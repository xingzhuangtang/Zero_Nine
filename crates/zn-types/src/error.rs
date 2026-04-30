//! Structured error types for Zero_Nine.
//!
//! All public-facing errors are defined here using `thiserror` for ergonomic
//! error handling. Downstream crates should prefer these typed errors over
//! bare `anyhow::anyhow!("...")` strings for errors that cross API boundaries
//! or need to be matched programmatically.

use thiserror::Error;

// ==================== Orchestration Errors ====================

/// Errors that occur during the brainstorming phase.
#[derive(Debug, Error)]
pub enum BrainstormError {
    #[error("no brainstorm session found to resume")]
    NoSessionToResume,

    #[error("goal is required when starting a new brainstorm session")]
    GoalRequired,

    #[error("brainstorm input cannot be empty")]
    EmptyInput,

    #[error("unknown brainstorm question: {question_id}")]
    UnknownQuestion { question_id: String },

    #[error("brainstorm session {session_id} is not Ready (verdict: {verdict})")]
    SessionNotReady { session_id: String, verdict: String },
}

/// Errors that occur during proposal lifecycle management.
#[derive(Debug, Error)]
pub enum ProposalError {
    #[error("no proposal found")]
    NotFound,

    #[error(
        "cannot execute proposal {proposal_id}: brainstorm session {session_id} is not Ready"
    )]
    BrainstormNotReady {
        proposal_id: String,
        session_id: String,
    },

    #[error(
        "cannot execute proposal {proposal_id}: goal does not match the latest Ready brainstorm"
    )]
    GoalMismatch { proposal_id: String },

    #[error("cannot execute proposal {proposal_id}: status is {status:?} instead of Ready")]
    NotReadyStatus { proposal_id: String, status: String },

    #[error("proposal {proposal_id} spec validation failed: {reason}")]
    SpecValidationFailed { proposal_id: String, reason: String },
}

// ==================== Execution Errors ====================

/// Errors that occur during task execution.
#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("task {task_id} timed out after {timeout_secs} seconds")]
    TaskTimeout { task_id: String, timeout_secs: u64 },

    #[error("task {task_id} not found")]
    TaskNotFound { task_id: String },

    #[error("repository has uncommitted changes; aborting finish-branch")]
    UncommittedChanges,

    #[error("no git remote configured for pull-request automation")]
    NoGitRemote,

    #[error("missing worktree plan for git worktree strategy")]
    MissingWorktreePlan,

    #[error("bridge_address required for bridge execution path")]
    BridgeAddressRequired,

    #[error("subagent execution failed: {source}")]
    SubagentFailed {
        #[source]
        source: anyhow::Error,
    },

    #[error("{context}: {detail}")]
    ContextualFailure { context: String, detail: String },

    #[error("empty command is not allowed")]
    EmptyCommand,

    #[error("command not allowed by policy: {command}")]
    CommandNotAllowed { command: String },
}

// ==================== Skill / Spec Errors ====================

/// Errors that occur during skill and spec management.
#[derive(Debug, Error)]
pub enum SkillError {
    #[error("skill file must start with YAML frontmatter delimiter '---'")]
    MissingFrontmatter,

    #[error("skill '{name}' not found")]
    NotFound { name: String },

    #[error("old string not found in skill file")]
    PatchTargetNotFound,
}

/// Errors that occur during memory/file tool operations.
#[derive(Debug, Error)]
pub enum MemoryToolError {
    #[error("old text not found in {target}")]
    OldTextNotFound { target: String },

    #[error("text to remove not found in {target}")]
    RemoveTargetNotFound { target: String },
}

// ==================== Top-level ZnError ====================

/// The top-level error type for Zero_Nine operations.
///
/// This wraps all domain-specific errors and can be used as the `Err` variant
/// in `Result<T, ZnError>` for public API boundaries.
#[derive(Debug, Error)]
pub enum ZnError {
    #[error(transparent)]
    Brainstorm(#[from] BrainstormError),

    #[error(transparent)]
    Proposal(#[from] ProposalError),

    #[error(transparent)]
    Execution(#[from] ExecutionError),

    #[error(transparent)]
    Skill(#[from] SkillError),

    #[error(transparent)]
    MemoryTool(#[from] MemoryToolError),

    /// Catch-all for I/O and other infrastructure errors.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Catch-all for JSON serialization errors.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Catch-all for other errors (bridges to anyhow for gradual migration).
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brainstorm_error_display() {
        let e = BrainstormError::EmptyInput;
        assert_eq!(e.to_string(), "brainstorm input cannot be empty");
    }

    #[test]
    fn test_proposal_error_display() {
        let e = ProposalError::NotFound;
        assert_eq!(e.to_string(), "no proposal found");
    }

    #[test]
    fn test_proposal_goal_mismatch_display() {
        let e = ProposalError::GoalMismatch {
            proposal_id: "prop-123".to_string(),
        };
        assert!(e.to_string().contains("prop-123"));
        assert!(e.to_string().contains("goal does not match"));
    }

    #[test]
    fn test_execution_error_timeout_display() {
        let e = ExecutionError::TaskTimeout {
            task_id: "task-1".to_string(),
            timeout_secs: 30,
        };
        assert!(e.to_string().contains("task-1"));
        assert!(e.to_string().contains("30 seconds"));
    }

    #[test]
    fn test_skill_error_not_found_display() {
        let e = SkillError::NotFound {
            name: "my-skill".to_string(),
        };
        assert_eq!(e.to_string(), "skill 'my-skill' not found");
    }

    #[test]
    fn test_zn_error_from_brainstorm() {
        let inner = BrainstormError::GoalRequired;
        let e: ZnError = inner.into();
        assert!(e.to_string().contains("goal is required"));
    }

    #[test]
    fn test_zn_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let e: ZnError = io_err.into();
        assert!(e.to_string().contains("I/O error"));
    }

    #[test]
    fn test_memory_tool_error_display() {
        let e = MemoryToolError::OldTextNotFound {
            target: "README.md".to_string(),
        };
        assert!(e.to_string().contains("README.md"));
        assert!(e.to_string().contains("old text not found"));
    }
}
