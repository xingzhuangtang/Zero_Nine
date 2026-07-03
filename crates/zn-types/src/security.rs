//! Security types — sandbox levels, tool permissions, and permission matrices.
//!
//! Extends the existing governance system with per-agent permission granularity.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Sandbox isolation level for agent execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SandboxLevel {
    /// No isolation — full system access.
    None,
    /// Restricted filesystem, no network access.
    Restricted,
    /// Container-level isolation with resource limits.
    Container,
    /// Full sandbox with strict resource limits and network isolation.
    Full,
}

impl Default for SandboxLevel {
    fn default() -> Self {
        Self::Restricted
    }
}

/// Permission for a specific tool/command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermission {
    /// Tool name (e.g. "read_file", "execute_command").
    pub tool_name: String,
    /// Whether the tool is allowed at all.
    pub allowed: bool,
    /// Maximum risk level this tool can operate at.
    pub max_risk_level: crate::governance::ActionRiskLevel,
    /// Whether the tool requires human confirmation.
    #[serde(default)]
    pub requires_confirmation: bool,
    /// Glob patterns for allowed arguments.
    #[serde(default)]
    pub allowed_args_patterns: Vec<String>,
    /// Glob patterns for denied arguments.
    #[serde(default)]
    pub denied_args_patterns: Vec<String>,
}

/// Per-agent permission matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionMatrix {
    /// Agent this matrix applies to.
    pub agent_id: String,
    /// Sandbox isolation level.
    pub sandbox_level: SandboxLevel,
    /// Tool-specific permissions.
    pub tool_permissions: HashMap<String, ToolPermission>,
    /// Paths the agent can access.
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    /// Paths the agent cannot access.
    #[serde(default)]
    pub denied_paths: Vec<String>,
    /// Whether the agent can access the network.
    #[serde(default)]
    pub network_allowed: bool,
    /// Maximum execution time in seconds.
    #[serde(default = "default_max_exec_time")]
    pub max_execution_time_secs: u64,
}

fn default_max_exec_time() -> u64 {
    300
}

/// Result of a safe command validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeCommandResult {
    /// The command that was checked.
    pub command: String,
    /// Whether the command is allowed.
    pub allowed: bool,
    /// Reason if denied.
    #[serde(default)]
    pub reason: Option<String>,
    /// Exit code (only set after execution).
    #[serde(default)]
    pub exit_code: Option<i32>,
    /// Standard output (only set after execution).
    #[serde(default)]
    pub stdout: Option<String>,
    /// Standard error (only set after execution).
    #[serde(default)]
    pub stderr: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_level_ordering() {
        assert!(SandboxLevel::None < SandboxLevel::Restricted);
        assert!(SandboxLevel::Restricted < SandboxLevel::Container);
        assert!(SandboxLevel::Container < SandboxLevel::Full);
    }

    #[test]
    fn test_sandbox_level_default() {
        assert_eq!(SandboxLevel::default(), SandboxLevel::Restricted);
    }

    #[test]
    fn test_tool_permission_serialization() {
        let tp = ToolPermission {
            tool_name: "read_file".to_string(),
            allowed: true,
            max_risk_level: crate::governance::ActionRiskLevel::Low,
            requires_confirmation: false,
            allowed_args_patterns: vec!["**/*.rs".to_string()],
            denied_args_patterns: vec!["**/*.env".to_string()],
        };
        let json = serde_json::to_string(&tp).unwrap();
        let restored: ToolPermission = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.tool_name, "read_file");
    }

    #[test]
    fn test_permission_matrix_defaults() {
        let pm = PermissionMatrix {
            agent_id: "a1".to_string(),
            sandbox_level: SandboxLevel::default(),
            tool_permissions: HashMap::new(),
            allowed_paths: vec![],
            denied_paths: vec![],
            network_allowed: false,
            max_execution_time_secs: 300,
        };
        assert_eq!(pm.sandbox_level, SandboxLevel::Restricted);
        assert_eq!(pm.max_execution_time_secs, 300);
    }
}
