//! Safe Command Executor — validates commands against permission matrices before execution.
//!
//! Wraps raw command execution with:
//! - Command whitelist/blacklist validation
//! - Path restriction checks (canonicalize + project root)
//! - Argument pattern matching (glob-based)
//! - Sandbox level enforcement

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use zn_types::{PermissionMatrix, SafeCommandResult, SandboxLevel};

use crate::governance::{AuthorizationMatrix, AuthorizationCheckResult, RiskLevel};

/// Safe command executor — validates commands before running them.
pub struct SafeCommandExecutor {
    permission_matrix: PermissionMatrix,
    auth_matrix: AuthorizationMatrix,
    /// Whitelist of allowed command prefixes.
    command_whitelist: Vec<String>,
    /// Blacklist of dangerous command patterns.
    command_blacklist: Vec<String>,
    /// Project root for path restriction.
    project_root: PathBuf,
}

impl SafeCommandExecutor {
    /// Create a new safe command executor.
    pub fn new(
        permission_matrix: PermissionMatrix,
        auth_matrix: AuthorizationMatrix,
        project_root: &Path,
    ) -> Self {
        Self {
            permission_matrix,
            auth_matrix,
            command_whitelist: default_whitelist(),
            command_blacklist: default_blacklist(),
            project_root: project_root.to_path_buf(),
        }
    }

    /// Check if a command is allowed without executing it.
    pub fn validate(&self, command: &str, args: &[&str]) -> SafeCommandResult {
        // Check blacklist first.
        let full = format!("{} {}", command, args.join(" "));
        for pattern in &self.command_blacklist {
            if pattern_matches(pattern, &full) {
                return SafeCommandResult {
                    command: full,
                    allowed: false,
                    reason: Some(format!("blocked by blacklist pattern: {pattern}")),
                    exit_code: None,
                    stdout: None,
                    stderr: None,
                };
            }
        }

        // Check whitelist.
        if !self.command_whitelist.iter().any(|w| command.starts_with(w)) {
            return SafeCommandResult {
                command: full.clone(),
                allowed: false,
                reason: Some(format!(
                    "command not in whitelist (allowed: {:?})",
                    self.command_whitelist
                )),
                exit_code: None,
                stdout: None,
                stderr: None,
            };
        }

        // Check tool-specific permissions.
        if let Some(tp) = self.permission_matrix.tool_permissions.get(command) {
            if !tp.allowed {
                return SafeCommandResult {
                    command: full.clone(),
                    allowed: false,
                    reason: Some(format!("tool '{}' is not allowed", command)),
                    exit_code: None,
                    stdout: None,
                    stderr: None,
                };
            }
            // Check denied argument patterns.
            for arg in args {
                for pattern in &tp.denied_args_patterns {
                    if pattern_matches(pattern, arg) {
                        return SafeCommandResult {
                            command: full.clone(),
                            allowed: false,
                            reason: Some(format!(
                                "argument '{}' matches denied pattern '{}' for tool '{command}'",
                                arg, pattern
                            )),
                            exit_code: None,
                            stdout: None,
                            stderr: None,
                        };
                    }
                }
            }
        }

        SafeCommandResult {
            command: full,
            allowed: true,
            reason: None,
            exit_code: None,
            stdout: None,
            stderr: None,
        }
    }

    /// Validate and execute a command safely.
    pub fn execute(&self, command: &str, args: &[&str]) -> Result<SafeCommandResult> {
        let validation = self.validate(command, args);
        if !validation.allowed {
            return Ok(validation);
        }

        // Execute the command.
        let output = std::process::Command::new(command)
            .args(args)
            .output()?;

        Ok(SafeCommandResult {
            command: format!("{} {}", command, args.join(" ")),
            allowed: true,
            reason: None,
            exit_code: Some(output.status.code().unwrap_or(-1)),
            stdout: Some(String::from_utf8_lossy(&output.stdout).to_string()),
            stderr: Some(String::from_utf8_lossy(&output.stderr).to_string()),
        })
    }

    /// Get the required sandbox level for a given command.
    pub fn required_sandbox_level(&self, command: &str) -> SandboxLevel {
        if self.command_blacklist.iter().any(|p| command.starts_with(p)) {
            return SandboxLevel::Full;
        }
        if self.command_whitelist.iter().any(|w| command.starts_with(w)) {
            return SandboxLevel::Restricted;
        }
        SandboxLevel::Container
    }

    /// Resolve a path and verify it's within the project root.
    pub fn verify_path(&self, path: &Path) -> Result<PathBuf> {
        let canonical = path.canonicalize().or_else(|_| {
            // If the path doesn't exist yet, canonicalize the parent and append the filename.
            if let Some(parent) = path.parent() {
                let canon_parent = parent.canonicalize()?;
                Ok(canon_parent.join(path.file_name().unwrap_or_default()))
            } else {
                Err(anyhow!("cannot resolve path: {:?}", path))
            }
        })?;

        let project_canonical = self.project_root.canonicalize()?;
        if canonical.starts_with(&project_canonical) {
            Ok(canonical)
        } else {
            Err(anyhow!(
                "path {:?} is outside project root {:?}",
                canonical,
                project_canonical
            ))
        }
    }
}

fn default_whitelist() -> Vec<String> {
    vec![
        "cargo".to_string(),
        "rustc".to_string(),
        "git".to_string(),
        "ls".to_string(),
        "cat".to_string(),
        "echo".to_string(),
        "grep".to_string(),
        "find".to_string(),
        "diff".to_string(),
        "head".to_string(),
        "tail".to_string(),
        "wc".to_string(),
        "sort".to_string(),
        "npm".to_string(),
        "node".to_string(),
        "python".to_string(),
        "python3".to_string(),
        "pytest".to_string(),
    ]
}

fn default_blacklist() -> Vec<String> {
    vec![
        "rm -rf".to_string(),
        "sudo".to_string(),
        "curl | bash".to_string(),
        "wget | sh".to_string(),
        "nc ".to_string(),
        "netcat".to_string(),
        "/dev/tcp".to_string(),
        "/dev/udp".to_string(),
        "chmod 777".to_string(),
        "dd if=".to_string(),
        "mkfs".to_string(),
        "fdisk".to_string(),
        "shutdown".to_string(),
        "reboot".to_string(),
        "kill -9".to_string(),
        ":(){:|:&};:".to_string(), // fork bomb
    ]
}

/// Simple glob-like pattern matching (supports `*` and `**`).
fn pattern_matches(pattern: &str, text: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    // Exact prefix match for simple patterns (no wildcards).
    if !pattern.contains('*') {
        return text.starts_with(pattern);
    }
    // `**/` prefix means match anywhere in path.
    if pattern.starts_with("**/") {
        let suffix = &pattern[3..];
        return text.ends_with(suffix) || text.contains(suffix);
    }
    // `*` suffix means prefix match.
    if pattern.ends_with("*") {
        return text.starts_with(&pattern[..pattern.len() - 1]);
    }
    text.starts_with(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zn_types::{ActionRiskLevel, ToolPermission};

    fn make_executor() -> SafeCommandExecutor {
        let pm = PermissionMatrix {
            agent_id: "test".to_string(),
            sandbox_level: SandboxLevel::Restricted,
            tool_permissions: HashMap::new(),
            allowed_paths: vec!["**/*".to_string()],
            denied_paths: vec!["**/*.env".to_string(), ".zero_nine/**".to_string()],
            network_allowed: false,
            max_execution_time_secs: 60,
        };
        let auth_matrix = AuthorizationMatrix::default();
        SafeCommandExecutor::new(pm, auth_matrix, Path::new("/tmp"))
    }

    #[test]
    fn test_validate_whitelisted_command() {
        let ex = make_executor();
        let result = ex.validate("cargo", &["build"]);
        assert!(result.allowed);
    }

    #[test]
    fn test_validate_blacklisted_command() {
        let ex = make_executor();
        let result = ex.validate("rm", &["-rf", "/"]);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("blacklist"));
    }

    #[test]
    fn test_validate_non_whitelisted_command() {
        let ex = make_executor();
        let result = ex.validate("evil_command", &[]);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("whitelist"));
    }

    #[test]
    fn test_tool_permission_denied() {
        let mut pm = PermissionMatrix {
            agent_id: "test".to_string(),
            sandbox_level: SandboxLevel::Restricted,
            tool_permissions: HashMap::new(),
            allowed_paths: vec![],
            denied_paths: vec![],
            network_allowed: false,
            max_execution_time_secs: 60,
        };
        pm.tool_permissions.insert(
            "cargo".to_string(),
            ToolPermission {
                tool_name: "cargo".to_string(),
                allowed: false,
                max_risk_level: ActionRiskLevel::Low,
                requires_confirmation: false,
                allowed_args_patterns: vec![],
                denied_args_patterns: vec![],
            },
        );
        let auth_matrix = AuthorizationMatrix::default();
        let ex = SafeCommandExecutor::new(pm, auth_matrix, Path::new("/tmp"));
        let result = ex.validate("cargo", &["build"]);
        assert!(!result.allowed);
    }

    #[test]
    fn test_required_sandbox_level() {
        let ex = make_executor();
        assert_eq!(ex.required_sandbox_level("sudo"), SandboxLevel::Full);
        assert_eq!(ex.required_sandbox_level("cargo"), SandboxLevel::Restricted);
    }

    #[test]
    fn test_pattern_matching() {
        assert!(pattern_matches("rm -rf", "rm -rf /"));
        assert!(pattern_matches("cargo*", "cargo build"));
        assert!(pattern_matches("**/.env", "project/.env"));
        assert!(!pattern_matches("cargo*", "rustc build"));
    }
}
