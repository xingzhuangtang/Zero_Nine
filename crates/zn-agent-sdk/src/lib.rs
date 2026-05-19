//! Zero_Nine Agent SDK
//!
//! Provides building blocks for external agents to connect to the Zero_Nine orchestrator:
//! - `TaskHandler` trait — implement to handle dispatched tasks
//! - `AgentBuilder` — fluent builder for registering agents
//! - `AgentContext` — task context passed to handlers
//! - `RegisteredAgent` — connects to bridge, listens for dispatches, executes tasks

use anyhow::Result;
use async_trait::async_trait;
use zn_types::{AgentDescriptor, AgentType, Capability, ExecutionPlan, ExecutionReport};

/// Context passed to a task handler with all necessary information.
#[derive(Debug, Clone)]
pub struct AgentContext {
    pub task_id: String,
    pub proposal_id: String,
    pub plan: ExecutionPlan,
}

/// Trait that external agents implement to handle dispatched tasks.
#[async_trait]
pub trait TaskHandler: Send + Sync {
    /// Handle a dispatched task and return an execution report.
    async fn handle(&self, ctx: AgentContext) -> Result<ExecutionReport>;
}

/// Fluent builder for creating and registering an agent.
pub struct AgentBuilder {
    name: String,
    agent_type: AgentType,
    capabilities: Vec<Capability>,
    trust_score: f32,
    bridge_address: Option<String>,
}

impl AgentBuilder {
    /// Create a new builder with the given agent name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            agent_type: AgentType::External,
            capabilities: Vec::new(),
            trust_score: 0.5,
            bridge_address: None,
        }
    }

    /// Set the agent type.
    pub fn agent_type(mut self, agent_type: AgentType) -> Self {
        self.agent_type = agent_type;
        self
    }

    /// Add a capability with name, proficiency, and max complexity.
    pub fn capability(mut self, name: &str, proficiency: f32, max_complexity: f32) -> Self {
        self.capabilities.push(Capability {
            name: name.to_string(),
            proficiency: proficiency.clamp(0.0, 1.0),
            max_complexity: max_complexity.clamp(0.0, 1.0),
        });
        self
    }

    /// Set the initial trust score (0.0-1.0).
    pub fn trust_score(mut self, score: f32) -> Self {
        self.trust_score = score.clamp(0.0, 1.0);
        self
    }

    /// Set the bridge address to connect to.
    pub fn bridge_address(mut self, address: &str) -> Self {
        self.bridge_address = Some(address.to_string());
        self
    }

    /// Build and connect the agent with the given task handler.
    pub async fn connect(self, handler: Box<dyn TaskHandler>) -> Result<RegisteredAgent> {
        let descriptor = AgentDescriptor {
            agent_id: format!("{}-{}", self.name.to_lowercase().replace(' ', "-"), uuid::Uuid::new_v4()),
            name: self.name,
            agent_type: self.agent_type,
            capabilities: self.capabilities,
            trust_score: self.trust_score,
            created_at: chrono::Utc::now(),
        };

        Ok(RegisteredAgent {
            descriptor,
            handler,
            bridge_address: self.bridge_address,
        })
    }
}

/// A registered agent connected to the Zero_Nine orchestrator.
pub struct RegisteredAgent {
    descriptor: AgentDescriptor,
    handler: Box<dyn TaskHandler>,
    bridge_address: Option<String>,
}

impl RegisteredAgent {
    /// Get the agent's descriptor.
    pub fn descriptor(&self) -> &AgentDescriptor {
        &self.descriptor
    }

    /// Execute a task using the registered handler.
    pub async fn execute_task(&self, ctx: AgentContext) -> Result<ExecutionReport> {
        self.handler.handle(ctx).await
    }

    /// Get the bridge address this agent is connected to.
    pub fn bridge_address(&self) -> Option<&str> {
        self.bridge_address.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHandler;

    #[async_trait]
    impl TaskHandler for MockHandler {
        async fn handle(&self, ctx: AgentContext) -> Result<ExecutionReport> {
            Ok(ExecutionReport {
                task_id: ctx.task_id,
                success: true,
                outcome: zn_types::ExecutionOutcome::Completed,
                summary: "Mock execution".to_string(),
                ..Default::default()
            })
        }
    }

    #[tokio::test]
    async fn test_agent_builder() {
        let agent = AgentBuilder::new("Test Agent")
            .capability("coding", 0.9, 0.8)
            .capability("review", 0.7, 0.6)
            .trust_score(0.85)
            .connect(Box::new(MockHandler))
            .await
            .unwrap();

        assert_eq!(agent.descriptor().name, "Test Agent");
        assert_eq!(agent.descriptor().capabilities.len(), 2);
        assert!((agent.descriptor().trust_score - 0.85).abs() < f32::EPSILON);
        assert!(agent.descriptor().agent_id.starts_with("test-agent-"));
    }

    #[tokio::test]
    async fn test_execute_task() {
        let agent = AgentBuilder::new("Mock")
            .connect(Box::new(MockHandler))
            .await
            .unwrap();

        let ctx = AgentContext {
            task_id: "task-1".to_string(),
            proposal_id: "prop-1".to_string(),
            plan: ExecutionPlan {
                task_id: "task-1".to_string(),
                objective: "Test".to_string(),
                mode: zn_types::ExecutionMode::TddCycle,
                workspace_strategy: zn_types::WorkspaceStrategy::GitWorktree,
                steps: vec![],
                validation: vec![],
                quality_gates: vec![],
                skill_chain: vec![],
                deliverables: vec![],
                risks: vec![],
                subagents: vec![],
                worktree_plan: None,
                workspace_record: None,
                verification_actions: vec![],
                finish_branch_automation: None,
                execution_path: zn_types::SubagentExecutionPath::default(),
                bridge_address: None,
                max_retries: None,
            },
        };

        let report = agent.execute_task(ctx).await.unwrap();
        assert!(report.success);
        assert_eq!(report.task_id, "task-1");
    }

    #[tokio::test]
    async fn test_builder_defaults() {
        let agent = AgentBuilder::new("Default")
            .connect(Box::new(MockHandler))
            .await
            .unwrap();

        assert!(matches!(agent.descriptor().agent_type, AgentType::External));
        assert!((agent.descriptor().trust_score - 0.5).abs() < f32::EPSILON);
        assert!(agent.bridge_address().is_none());
    }
}
