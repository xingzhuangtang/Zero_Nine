//! A2A (Agent-to-Agent) communication protocol types.
//!
//! Defines message envelopes, named channels, and typed payloads
//! for the in-memory A2A bus that coordinates local agents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Named logical channels for topic-based message routing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum A2AChannel {
    /// Leader ↔ Worker task dispatch and assignment.
    Coordination,
    /// Worker → Leader progress reports.
    Progress,
    /// Reviewer → Leader review verdicts.
    Review,
    /// Memory query/response between agents.
    Memory,
    /// Evolution signal broadcasts.
    Evolution,
    /// Policy enforcement and governance messages.
    Governance,
    /// Agent liveness heartbeats.
    Heartbeat,
}

/// The A2A message envelope. Every inter-agent message wraps one of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AMessage {
    /// Unique message ID (UUID).
    pub id: String,
    /// Sending agent ID.
    pub from_agent: String,
    /// Target agent ID. `None` means broadcast on `channel`.
    pub to_agent: Option<String>,
    /// Logical channel for routing.
    pub channel: A2AChannel,
    /// Typed message payload.
    pub payload: A2APayload,
    /// Optional distributed trace ID for cross-cutting observability.
    pub trace_id: Option<String>,
    /// Message creation time.
    pub timestamp: DateTime<Utc>,
}

impl A2AMessage {
    /// Create a new directed (unicast) message.
    pub fn unicast(
        from: impl Into<String>,
        to: impl Into<String>,
        channel: A2AChannel,
        payload: A2APayload,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            from_agent: from.into(),
            to_agent: Some(to.into()),
            channel,
            payload,
            trace_id: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a new broadcast message (no specific target).
    pub fn broadcast(
        from: impl Into<String>,
        channel: A2AChannel,
        payload: A2APayload,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            from_agent: from.into(),
            to_agent: None,
            channel,
            payload,
            trace_id: None,
            timestamp: Utc::now(),
        }
    }

    /// Attach a trace ID for distributed tracing.
    pub fn with_trace(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }
}

/// Typed payload variants carried by an A2AMessage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum A2APayload {
    /// Leader dispatches a subtask to a worker.
    TaskDispatch {
        task_id: String,
        objective: String,
        context: Vec<String>,
    },
    /// Worker reports progress on a subtask.
    TaskProgress {
        task_id: String,
        percent: u8,
        summary: String,
    },
    /// Worker reports subtask completion.
    TaskCompleted {
        task_id: String,
        success: bool,
        summary: String,
        artifacts: Vec<String>,
    },
    /// Leader requests a reviewer to review work.
    ReviewRequest {
        task_id: String,
        evidence_keys: Vec<String>,
    },
    /// Reviewer returns a verdict.
    ReviewVerdict {
        task_id: String,
        approved: bool,
        notes: String,
    },
    /// Evolution subsystem broadcasts a trigger signal.
    EvolutionTrigger {
        signal: EvolutionSignalPayload,
    },
    /// Agent queries the memory store.
    MemoryQuery {
        query: String,
        max_results: usize,
    },
    /// Memory store responds with matching entries.
    MemoryResponse {
        results: Vec<MemoryEntryPayload>,
    },
    /// Generic free-text message.
    Text {
        content: String,
    },
    /// Agent announces itself to the bus.
    AgentAnnounce {
        agent_id: String,
        capabilities: Vec<String>,
    },
    /// Liveness heartbeat.
    Heartbeat {
        agent_id: String,
        status: String,
    },
}

/// Compact evolution signal carried inside an A2A message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionSignalPayload {
    pub task_id: String,
    pub score: f32,
    pub decision: String,
    pub notes: Vec<String>,
}

/// Compact memory entry returned in MemoryResponse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntryPayload {
    pub key: String,
    pub content: String,
    pub relevance: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unicast_message_roundtrip() {
        let msg = A2AMessage::unicast(
            "leader",
            "worker-1",
            A2AChannel::Coordination,
            A2APayload::TaskDispatch {
                task_id: "t1".to_string(),
                objective: "implement feature".to_string(),
                context: vec!["src/main.rs".to_string()],
            },
        );
        assert_eq!(msg.from_agent, "leader");
        assert_eq!(msg.to_agent, Some("worker-1".to_string()));

        let json = serde_json::to_string(&msg).unwrap();
        let restored: A2AMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.from_agent, "leader");
    }

    #[test]
    fn test_broadcast_message() {
        let msg = A2AMessage::broadcast(
            "leader",
            A2AChannel::Evolution,
            A2APayload::EvolutionTrigger {
                signal: EvolutionSignalPayload {
                    task_id: "t2".to_string(),
                    score: 0.75,
                    decision: "improve".to_string(),
                    notes: vec![],
                },
            },
        );
        assert!(msg.to_agent.is_none());
    }

    #[test]
    fn test_channel_serialization() {
        let ch = A2AChannel::Coordination;
        assert_eq!(serde_json::to_string(&ch).unwrap(), "\"coordination\"");
        let ch2 = A2AChannel::Heartbeat;
        assert_eq!(serde_json::to_string(&ch2).unwrap(), "\"heartbeat\"");
    }

    #[test]
    fn test_payload_tagged_serialization() {
        let p = A2APayload::TaskProgress {
            task_id: "t3".to_string(),
            percent: 50,
            summary: "halfway".to_string(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"type\":\"task_progress\""));
    }
}
