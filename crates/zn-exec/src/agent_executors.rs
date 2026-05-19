//! Built-in Agent Executor Implementations
//!
//! Provides concrete `AgentExecutor` implementations for the three built-in host types:
//! - `ClaudeCodeAgent` — invokes the `claude` CLI
//! - `OpenCodeAgent` — invokes the `opencode` CLI
//! - `TerminalAgent` — executes commands directly in the terminal

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::process::Command;
use zn_types::{
    AgentDescriptor, AgentType, Capability, ExecutionPlan, ExecutionReport, ExecutionOutcome,
    HostKind,
};

use crate::agent_protocol::AgentExecutor;

/// Common helper: run a CLI command with a prompt and capture output.
fn run_cli_command(binary: &str, prompt: &str, extra_args: &[&str]) -> Result<CommandOutput> {
    let mut cmd = Command::new(binary);
    cmd.arg("--verbose").arg("--prompt").arg(prompt);
    for arg in extra_args {
        cmd.arg(arg);
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to spawn `{}` CLI", binary))?;

    Ok(CommandOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

struct CommandOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

/// Builds a minimal ExecutionReport from CLI output.
fn report_from_output(plan: &ExecutionPlan, output: &CommandOutput) -> ExecutionReport {
    ExecutionReport {
        task_id: plan.task_id.clone(),
        success: output.success,
        outcome: if output.success {
            ExecutionOutcome::Completed
        } else {
            ExecutionOutcome::RetryableFailure
        },
        summary: if output.success {
            "Task completed successfully".to_string()
        } else {
            format!("Task failed: {}", output.stderr)
        },
        ..Default::default()
    }
}

// ==================== ClaudeCodeAgent ====================

/// Agent executor for Claude Code (`claude` CLI).
pub struct ClaudeCodeAgent {
    descriptor: AgentDescriptor,
}

impl ClaudeCodeAgent {
    pub fn new() -> Self {
        Self {
            descriptor: AgentDescriptor::from_host_kind(&HostKind::ClaudeCode, "claude-code-001".to_string()),
        }
    }
}

impl Default for ClaudeCodeAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentExecutor for ClaudeCodeAgent {
    fn descriptor(&self) -> &AgentDescriptor {
        &self.descriptor
    }

    async fn execute(&self, plan: &ExecutionPlan) -> Result<ExecutionReport> {
        let prompt = build_plan_prompt(plan);
        let output = run_cli_command("claude", &prompt, &[])?;
        Ok(report_from_output(plan, &output))
    }

    async fn cancel(&self, _task_id: &str) -> Result<()> {
        // claude CLI has no cancel signal; best-effort no-op
        Ok(())
    }
}

// ==================== OpenCodeAgent ====================

/// Agent executor for OpenCode (`opencode` CLI).
pub struct OpenCodeAgent {
    descriptor: AgentDescriptor,
}

impl OpenCodeAgent {
    pub fn new() -> Self {
        Self {
            descriptor: AgentDescriptor::from_host_kind(&HostKind::OpenCode, "opencode-001".to_string()),
        }
    }
}

impl Default for OpenCodeAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentExecutor for OpenCodeAgent {
    fn descriptor(&self) -> &AgentDescriptor {
        &self.descriptor
    }

    async fn execute(&self, plan: &ExecutionPlan) -> Result<ExecutionReport> {
        let prompt = build_plan_prompt(plan);
        let output = run_cli_command("opencode", &prompt, &[])?;
        Ok(report_from_output(plan, &output))
    }

    async fn cancel(&self, _task_id: &str) -> Result<()> {
        Ok(())
    }
}

// ==================== TerminalAgent ====================

/// Agent executor for direct terminal execution.
pub struct TerminalAgent {
    descriptor: AgentDescriptor,
}

impl TerminalAgent {
    pub fn new() -> Self {
        Self {
            descriptor: AgentDescriptor {
                agent_id: "terminal-agent-001".to_string(),
                name: "Terminal".to_string(),
                agent_type: AgentType::BuiltIn,
                capabilities: vec![Capability {
                    name: "shell-execution".to_string(),
                    proficiency: 0.9,
                    max_complexity: 0.7,
                }],
                trust_score: 0.8,
                created_at: chrono::Utc::now(),
            },
        }
    }
}

impl Default for TerminalAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentExecutor for TerminalAgent {
    fn descriptor(&self) -> &AgentDescriptor {
        &self.descriptor
    }

    async fn execute(&self, plan: &ExecutionPlan) -> Result<ExecutionReport> {
        // TerminalAgent executes the first validation command from the plan
        let cmd = plan.validation.first().ok_or_else(|| {
            anyhow!("TerminalAgent requires at least one validation command in the plan")
        })?;

        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .with_context(|| format!("Failed to execute terminal command: {}", cmd))?;

        let command_output = CommandOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        };

        let report = report_from_output(plan, &command_output);
        Ok(report)
    }

    async fn cancel(&self, _task_id: &str) -> Result<()> {
        Ok(())
    }
}

// ==================== Helpers ====================

/// Build a prompt string from an ExecutionPlan.
fn build_plan_prompt(plan: &ExecutionPlan) -> String {
    let skills = if plan.skill_chain.is_empty() {
        String::new()
    } else {
        format!(
            "\n## Active Skills\n{}\n",
            plan.skill_chain
                .iter()
                .map(|s| format!("- {}", s))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    let steps = if plan.steps.is_empty() {
        String::new()
    } else {
        format!(
            "\n## Plan Steps\n{}\n",
            plan.steps
                .iter()
                .enumerate()
                .map(|(i, s)| format!("{}. {}", i + 1, s.title))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    let deliverables = if plan.deliverables.is_empty() {
        String::new()
    } else {
        format!(
            "\n## Deliverables\n{}\n",
            plan.deliverables
                .iter()
                .map(|d| format!("- {}", d))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    format!(
        r#"# Execution Task

## Objective: {}
## Mode: {:?}
## Workspace Strategy: {:?}
{}
{}
{}
## Validation Criteria
{}

Please execute this task according to the plan above."#,
        plan.objective,
        plan.mode,
        plan.workspace_strategy,
        skills,
        steps,
        deliverables,
        plan.validation.join("\n"),
    )
}
