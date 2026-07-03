//! Concrete Channel implementations for each host kind.
//!
//! Provides:
//! - `ClaudeCodeChannel` — delegates to zn-exec execute_plan via Claude Code host
//! - `OpenCodeChannel` — delegates to zn-exec execute_plan via OpenCode host
//! - `TerminalChannel` — delegates to zn-exec execute_plan via local terminal

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use zn_types::{ExecutionPlan, ExecutionReport, ExecutionOutcome, HostKind, TaskItem, TaskStatus};

use crate::{Channel, ChannelConfig};

/// Shared state for channel execution.
#[derive(Clone)]
struct ChannelState {
    project_root: Arc<std::path::PathBuf>,
    config: ChannelConfig,
}

/// Channel for Claude Code host execution.
pub struct ClaudeCodeChannel {
    state: ChannelState,
}

impl ClaudeCodeChannel {
    /// Create a new Claude Code channel.
    pub fn new(project_root: &Path, config: Option<ChannelConfig>) -> Self {
        Self {
            state: ChannelState {
                project_root: Arc::new(project_root.to_path_buf()),
                config: config.unwrap_or_default(),
            },
        }
    }
}

#[async_trait::async_trait]
impl Channel for ClaudeCodeChannel {
    fn host_kind(&self) -> HostKind {
        HostKind::ClaudeCode
    }

    async fn execute(&self, plan: &ExecutionPlan) -> Result<ExecutionReport> {
        // Claude Code channel delegates to zn-exec execute_plan.
        // In production this would invoke the Claude Code subprocess.
        // For now, use the local execute_plan as the backend.
        let task = TaskItem {
            id: plan.task_id.clone(),
            title: plan.objective.clone(),
            description: plan.objective.clone(),
            status: TaskStatus::Running,
            depends_on: vec![],
            kind: None,
            contract: Default::default(),
            max_retries: None,
            preconditions: vec![],
        };

        let report = zn_exec::execute_plan(
            &self.state.project_root,
            &task,
            plan,
            None, // workspace_record
            false, // allow_remote_finish
        )?;

        Ok(report)
    }

    fn is_available(&self) -> bool {
        // Check if claude CLI is available
        std::process::Command::new("which")
            .arg("claude")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn config(&self) -> ChannelConfig {
        self.state.config.clone()
    }
}

/// Channel for OpenCode host execution.
pub struct OpenCodeChannel {
    state: ChannelState,
}

impl OpenCodeChannel {
    /// Create a new OpenCode channel.
    pub fn new(project_root: &Path, config: Option<ChannelConfig>) -> Self {
        Self {
            state: ChannelState {
                project_root: Arc::new(project_root.to_path_buf()),
                config: config.unwrap_or_default(),
            },
        }
    }
}

#[async_trait::async_trait]
impl Channel for OpenCodeChannel {
    fn host_kind(&self) -> HostKind {
        HostKind::OpenCode
    }

    async fn execute(&self, plan: &ExecutionPlan) -> Result<ExecutionReport> {
        let task = TaskItem {
            id: plan.task_id.clone(),
            title: plan.objective.clone(),
            description: plan.objective.clone(),
            status: TaskStatus::Running,
            depends_on: vec![],
            kind: None,
            contract: Default::default(),
            max_retries: None,
            preconditions: vec![],
        };

        let report = zn_exec::execute_plan(
            &self.state.project_root,
            &task,
            plan,
            None,
            false,
        )?;

        Ok(report)
    }

    fn is_available(&self) -> bool {
        // Check if opencode CLI is available
        std::process::Command::new("which")
            .arg("opencode")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn config(&self) -> ChannelConfig {
        self.state.config.clone()
    }
}

/// Channel for Terminal (local) execution.
pub struct TerminalChannel {
    state: ChannelState,
}

impl TerminalChannel {
    /// Create a new Terminal channel.
    pub fn new(project_root: &Path, config: Option<ChannelConfig>) -> Self {
        Self {
            state: ChannelState {
                project_root: Arc::new(project_root.to_path_buf()),
                config: config.unwrap_or_default(),
            },
        }
    }
}

#[async_trait::async_trait]
impl Channel for TerminalChannel {
    fn host_kind(&self) -> HostKind {
        HostKind::Terminal
    }

    async fn execute(&self, plan: &ExecutionPlan) -> Result<ExecutionReport> {
        let task = TaskItem {
            id: plan.task_id.clone(),
            title: plan.objective.clone(),
            description: plan.objective.clone(),
            status: TaskStatus::Running,
            depends_on: vec![],
            kind: None,
            contract: Default::default(),
            max_retries: None,
            preconditions: vec![],
        };

        let report = zn_exec::execute_plan(
            &self.state.project_root,
            &task,
            plan,
            None,
            false,
        )?;

        Ok(report)
    }

    fn is_available(&self) -> bool {
        // Terminal is always available
        true
    }

    fn config(&self) -> ChannelConfig {
        self.state.config.clone()
    }
}

/// Helper to build a default report for fallback scenarios.
#[allow(dead_code)]
fn fallback_report(plan: &ExecutionPlan, error: &str) -> ExecutionReport {
    ExecutionReport {
        task_id: plan.task_id.clone(),
        success: false,
        outcome: ExecutionOutcome::RetryableFailure,
        summary: format!("Channel execution failed: {error}"),
        details: vec![error.to_string()],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_channel_host_kind() {
        let ch = ClaudeCodeChannel::new(Path::new("/tmp"), None);
        assert_eq!(ch.host_kind(), HostKind::ClaudeCode);
    }

    #[test]
    fn test_opencode_channel_host_kind() {
        let ch = OpenCodeChannel::new(Path::new("/tmp"), None);
        assert_eq!(ch.host_kind(), HostKind::OpenCode);
    }

    #[test]
    fn test_terminal_channel_host_kind() {
        let ch = TerminalChannel::new(Path::new("/tmp"), None);
        assert_eq!(ch.host_kind(), HostKind::Terminal);
    }

    #[test]
    fn test_terminal_always_available() {
        let ch = TerminalChannel::new(Path::new("/tmp"), None);
        assert!(ch.is_available());
    }

    #[test]
    fn test_channel_config_override() {
        let config = ChannelConfig {
            max_concurrent: 5,
            timeout_secs: 120,
            ..Default::default()
        };
        let ch = TerminalChannel::new(Path::new("/tmp"), Some(config.clone()));
        assert_eq!(ch.config().max_concurrent, 5);
        assert_eq!(ch.config().timeout_secs, 120);
    }

    #[test]
    fn test_fallback_report() {
        let plan = ExecutionPlan {
            task_id: "task-1".to_string(),
            objective: "Test objective".to_string(),
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
            execution_path: zn_types::SubagentExecutionPath::Cli,
            bridge_address: None,
            max_retries: None,
        };
        let report = fallback_report(&plan, "connection refused");
        assert!(!report.success);
        assert_eq!(report.task_id, "task-1");
        assert!(report.summary.contains("connection refused"));
    }
}
