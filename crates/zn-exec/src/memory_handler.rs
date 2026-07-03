//! Memory A2A Handler — bridges A2A bus memory queries to the MemoryStore backend.
//!
//! Listens on `A2AChannel::Memory` for `MemoryQuery` payloads and responds
//! with `MemoryResponse` payloads routed back to the requesting agent.

use anyhow::Result;
use std::sync::Arc;
use zn_types::{A2AChannel, A2AMessage, A2APayload, MemoryEntryPayload, MemoryQuery};

use crate::a2a_bus::A2ABus;

/// Bridges A2A bus memory queries to a `MemoryStore` backend.
///
/// Spawn `run()` as a background tokio task.
pub struct MemoryA2AHandler {
    bus: Arc<A2ABus>,
    store: Arc<dyn zn_spec::MemoryStore>,
}

impl MemoryA2AHandler {
    pub fn new(bus: Arc<A2ABus>, store: Arc<dyn zn_spec::MemoryStore>) -> Self {
        Self { bus, store }
    }

    /// Subscribe to the Memory channel and serve queries until the receiver is closed.
    pub async fn run(&self) -> Result<()> {
        let mut sub = self.bus.subscribe(&A2AChannel::Memory).await;
        loop {
            match sub.recv().await {
                Ok(msg) => {
                    if let Err(e) = self.handle(&msg).await {
                        tracing::warn!("memory handler error: {e}");
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("memory handler lagged by {n} messages");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
        Ok(())
    }

    async fn handle(&self, msg: &A2AMessage) -> Result<()> {
        let A2APayload::MemoryQuery {
            ref query,
            max_results,
        } = msg.payload
        else {
            return Ok(()); // not a memory query — ignore
        };

        let mq = MemoryQuery {
            query: query.clone(),
            levels: vec![],
            tags: vec![],
            max_results,
            min_relevance: 0.0,
        };

        let result = self.store.search(&mq).unwrap_or(zn_types::MemorySearchResult {
            entries: vec![],
            total: 0,
        });

        let payloads: Vec<MemoryEntryPayload> = result
            .entries
            .into_iter()
            .map(|e| MemoryEntryPayload {
                key: e.key,
                content: e.content,
                relevance: e.relevance_score,
            })
            .collect();

        // Route reply back to the requesting agent (use trace_id as reply-to if set).
        let reply_to = msg
            .trace_id
            .as_deref()
            .unwrap_or(&msg.from_agent)
            .to_string();

        let reply = A2AMessage::unicast(
            "memory-handler",
            reply_to.as_str(),
            A2AChannel::Memory,
            A2APayload::MemoryResponse { results: payloads },
        );
        self.bus.send(reply).await?;
        Ok(())
    }
}
