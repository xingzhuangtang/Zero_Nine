//! Agent Executor Protocol
//!
//! Defines the `AgentExecutor` trait that abstracts over different agent implementations,
//! replacing hardcoded CLI invocations with a pluggable executor model.

use anyhow::Result;
use async_trait::async_trait;
use zn_types::{AgentDescriptor, ExecutionPlan, ExecutionReport};

/// Trait for any agent that can execute a plan within Zero_Nine.
///
/// Implementors represent concrete agent backends (built-in CLI agents,
/// external agents via bridge, human-in-the-loop, etc.).
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Returns the agent's descriptor (identity, capabilities, trust score).
    fn descriptor(&self) -> &AgentDescriptor;

    /// Execute the given plan and return a structured report.
    async fn execute(&self, plan: &ExecutionPlan) -> Result<ExecutionReport>;

    /// Cancel a running task by ID.
    async fn cancel(&self, task_id: &str) -> Result<()>;
}
