//! A2A Bus — in-memory agent-to-agent message bus.
//!
//! Provides two communication patterns:
//!
//! 1. **Broadcast** (pub/sub): `tokio::sync::broadcast` per `A2AChannel`.
//!    Any agent publishes; all subscribers on that channel receive.
//!
//! 2. **Direct** (unicast): `tokio::sync::mpsc` per `agent_id`.
//!    Messages addressed to a specific agent go to its mailbox.
//!
//! The bus is `Clone + Send + Sync`-friendly via `Arc` sharing.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use zn_types::{A2AChannel, A2AMessage, A2APayload};

/// Default broadcast channel capacity (messages buffered per subscriber lag).
const DEFAULT_BROADCAST_CAP: usize = 128;
/// Default mailbox capacity per agent.
const DEFAULT_MAILBOX_CAP: usize = 64;

/// A2A Bus — in-memory agent-to-agent communication hub.
///
/// Clone the `Arc`-wrapped bus to share it across tasks.
#[derive(Clone)]
pub struct A2ABus {
    inner: Arc<A2ABusInner>,
}

struct A2ABusInner {
    /// Broadcast sender per A2AChannel (created lazily on first subscribe/broadcast).
    channels: RwLock<HashMap<A2AChannel, broadcast::Sender<A2AMessage>>>,
    /// Direct mailboxes: agent_id → mpsc sender.
    mailboxes: RwLock<HashMap<String, mpsc::Sender<A2AMessage>>>,
    /// Dead-letter queue for messages addressed to unknown agents.
    dead_letters: RwLock<Vec<A2AMessage>>,
    /// Broadcast capacity for new channels.
    broadcast_cap: usize,
    /// Mailbox capacity for new agents.
    mailbox_cap: usize,
}

impl A2ABus {
    /// Create a new bus with default capacity settings.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_BROADCAST_CAP, DEFAULT_MAILBOX_CAP)
    }

    /// Create a bus with explicit capacity settings.
    pub fn with_capacity(broadcast_cap: usize, mailbox_cap: usize) -> Self {
        Self {
            inner: Arc::new(A2ABusInner {
                channels: RwLock::new(HashMap::new()),
                mailboxes: RwLock::new(HashMap::new()),
                dead_letters: RwLock::new(Vec::new()),
                broadcast_cap,
                mailbox_cap,
            }),
        }
    }

    /// Register an agent and return its dedicated mailbox receiver.
    ///
    /// Calling this again for the same `agent_id` replaces the old mailbox.
    pub async fn register_agent(&self, agent_id: &str) -> mpsc::Receiver<A2AMessage> {
        let (tx, rx) = mpsc::channel(self.inner.mailbox_cap);
        self.inner
            .mailboxes
            .write()
            .await
            .insert(agent_id.to_string(), tx);
        rx
    }

    /// Deregister an agent and remove its mailbox.
    pub async fn deregister_agent(&self, agent_id: &str) {
        self.inner.mailboxes.write().await.remove(agent_id);
    }

    /// Subscribe to a broadcast channel and get a `Receiver`.
    ///
    /// Creates the broadcast channel if it doesn't exist yet.
    pub async fn subscribe(&self, channel: &A2AChannel) -> broadcast::Receiver<A2AMessage> {
        let mut channels = self.inner.channels.write().await;
        channels
            .entry(channel.clone())
            .or_insert_with(|| broadcast::channel(self.inner.broadcast_cap).0)
            .subscribe()
    }

    /// Send a message.
    ///
    /// - If `to_agent` is `Some`, routes to that agent's mailbox (falls back to dead-letter if unknown).
    /// - If `to_agent` is `None`, broadcasts on `message.channel`.
    pub async fn send(&self, message: A2AMessage) -> Result<()> {
        if let Some(ref target) = message.to_agent.clone() {
            // Unicast: deliver to the agent's mailbox.
            let mailboxes = self.inner.mailboxes.read().await;
            if let Some(tx) = mailboxes.get(target) {
                tx.send(message).await.map_err(|_| {
                    anyhow!("mailbox for agent '{}' is closed", target)
                })?;
            } else {
                drop(mailboxes);
                // Unknown agent → dead-letter queue.
                self.inner.dead_letters.write().await.push(message);
            }
        } else {
            // Broadcast on the message's channel.
            let channel = message.channel.clone();
            self.broadcast_on(&channel, message).await?;
        }
        Ok(())
    }

    /// Broadcast a message on a specific channel regardless of `to_agent`.
    pub async fn broadcast_on(&self, channel: &A2AChannel, message: A2AMessage) -> Result<()> {
        let mut channels = self.inner.channels.write().await;
        let tx = channels
            .entry(channel.clone())
            .or_insert_with(|| broadcast::channel(self.inner.broadcast_cap).0);
        // broadcast::send errors only when there are no receivers — that's OK.
        let _ = tx.send(message);
        Ok(())
    }

    /// Send a request and await a single response (req/rep pattern).
    ///
    /// Creates a temporary reply-to identity and waits up to `timeout` for a
    /// response message addressed back to that identity.
    pub async fn request(
        &self,
        mut message: A2AMessage,
        timeout: Duration,
    ) -> Result<A2AMessage> {
        // Create a one-shot reply mailbox.
        let reply_id = format!("__reply_{}", uuid::Uuid::new_v4());
        let mut reply_rx = self.register_agent(&reply_id).await;

        // Tag the message so the responder knows where to reply.
        let trace = reply_id.clone();
        message.trace_id = Some(trace);

        self.send(message).await?;

        // Wait for the reply or timeout.
        let result = tokio::time::timeout(timeout, reply_rx.recv())
            .await
            .map_err(|_| anyhow!("A2A request timed out after {:?}", timeout))?
            .ok_or_else(|| anyhow!("reply mailbox closed before response arrived"))?;

        // Clean up the ephemeral mailbox.
        self.deregister_agent(&reply_id).await;
        Ok(result)
    }

    /// Return the number of currently registered agents.
    pub async fn agent_count(&self) -> usize {
        self.inner.mailboxes.read().await.len()
    }

    /// Drain and return all messages in the dead-letter queue.
    pub async fn drain_dead_letters(&self) -> Vec<A2AMessage> {
        let mut dl = self.inner.dead_letters.write().await;
        std::mem::take(&mut *dl)
    }

    /// Check if an agent is registered.
    pub async fn is_registered(&self, agent_id: &str) -> bool {
        self.inner.mailboxes.read().await.contains_key(agent_id)
    }
}

impl Default for A2ABus {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for `A2ABus` with custom configuration.
pub struct A2ABusBuilder {
    broadcast_cap: usize,
    mailbox_cap: usize,
}

impl A2ABusBuilder {
    pub fn new() -> Self {
        Self {
            broadcast_cap: DEFAULT_BROADCAST_CAP,
            mailbox_cap: DEFAULT_MAILBOX_CAP,
        }
    }

    pub fn broadcast_capacity(mut self, cap: usize) -> Self {
        self.broadcast_cap = cap;
        self
    }

    pub fn mailbox_capacity(mut self, cap: usize) -> Self {
        self.mailbox_cap = cap;
        self
    }

    pub fn build(self) -> A2ABus {
        A2ABus::with_capacity(self.broadcast_cap, self.mailbox_cap)
    }
}

impl Default for A2ABusBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;
    use zn_types::A2AChannel;

    fn make_text_msg(from: &str, to: Option<&str>, content: &str) -> A2AMessage {
        A2AMessage {
            id: uuid::Uuid::new_v4().to_string(),
            from_agent: from.to_string(),
            to_agent: to.map(|s| s.to_string()),
            channel: A2AChannel::Coordination,
            payload: A2APayload::Text {
                content: content.to_string(),
            },
            trace_id: None,
            timestamp: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_unicast_delivery() {
        let bus = A2ABus::new();
        let mut rx_a = bus.register_agent("agent-a").await;
        let mut rx_b = bus.register_agent("agent-b").await;

        let msg = make_text_msg("sender", Some("agent-a"), "hello agent-a");
        bus.send(msg).await.unwrap();

        // agent-a receives the message
        let received = timeout(Duration::from_millis(100), rx_a.recv())
            .await
            .expect("timed out")
            .expect("channel closed");
        assert_eq!(received.from_agent, "sender");

        // agent-b receives nothing
        let nothing = timeout(Duration::from_millis(10), rx_b.recv()).await;
        assert!(nothing.is_err(), "agent-b should not receive the message");
    }

    #[tokio::test]
    async fn test_broadcast_delivery() {
        let bus = A2ABus::new();

        let mut sub1 = bus.subscribe(&A2AChannel::Evolution).await;
        let mut sub2 = bus.subscribe(&A2AChannel::Evolution).await;

        let msg = A2AMessage::broadcast(
            "evolution-engine",
            A2AChannel::Evolution,
            A2APayload::Text {
                content: "evolution triggered".to_string(),
            },
        );
        bus.send(msg).await.unwrap();

        let r1 = timeout(Duration::from_millis(100), sub1.recv())
            .await
            .expect("sub1 timed out")
            .expect("sub1 lagged");
        let r2 = timeout(Duration::from_millis(100), sub2.recv())
            .await
            .expect("sub2 timed out")
            .expect("sub2 lagged");

        assert_eq!(r1.from_agent, "evolution-engine");
        assert_eq!(r2.from_agent, "evolution-engine");
    }

    #[tokio::test]
    async fn test_dead_letter_for_unknown_agent() {
        let bus = A2ABus::new();
        let msg = make_text_msg("sender", Some("ghost-agent"), "lost message");
        bus.send(msg).await.unwrap();

        let dead = bus.drain_dead_letters().await;
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].to_agent, Some("ghost-agent".to_string()));
    }

    #[tokio::test]
    async fn test_agent_count() {
        let bus = A2ABus::new();
        assert_eq!(bus.agent_count().await, 0);

        let _rx1 = bus.register_agent("a1").await;
        let _rx2 = bus.register_agent("a2").await;
        assert_eq!(bus.agent_count().await, 2);

        bus.deregister_agent("a1").await;
        assert_eq!(bus.agent_count().await, 1);
    }

    #[tokio::test]
    async fn test_is_registered() {
        let bus = A2ABus::new();
        assert!(!bus.is_registered("agent-x").await);
        let _rx = bus.register_agent("agent-x").await;
        assert!(bus.is_registered("agent-x").await);
    }

    #[tokio::test]
    async fn test_multi_channel_subscription() {
        let bus = A2ABus::new();
        let mut coord_sub = bus.subscribe(&A2AChannel::Coordination).await;
        let mut evol_sub = bus.subscribe(&A2AChannel::Evolution).await;

        // Send to Coordination
        let coord_msg = A2AMessage::broadcast(
            "leader",
            A2AChannel::Coordination,
            A2APayload::Text {
                content: "coord".to_string(),
            },
        );
        bus.send(coord_msg).await.unwrap();

        // Only coord_sub should receive
        let received = timeout(Duration::from_millis(100), coord_sub.recv())
            .await
            .expect("timed out")
            .expect("lagged");
        assert_eq!(received.channel, A2AChannel::Coordination);

        // evol_sub receives nothing
        let nothing = timeout(Duration::from_millis(10), evol_sub.recv()).await;
        assert!(nothing.is_err(), "evolution subscriber got a coordination message");
    }

    #[tokio::test]
    async fn test_builder() {
        let bus = A2ABusBuilder::new()
            .broadcast_capacity(32)
            .mailbox_capacity(16)
            .build();
        let _rx = bus.register_agent("x").await;
        assert!(bus.is_registered("x").await);
    }
}
