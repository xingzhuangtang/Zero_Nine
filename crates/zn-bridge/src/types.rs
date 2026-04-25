//! Type conversions between gRPC protobuf types and zn-types

use crate::proto;
use std::path::Path;
use zn_types::HostKind;

/// Configuration for the gRPC bridge server
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub bind_addr: std::net::SocketAddr,
    pub project_root: std::path::PathBuf,
    pub max_concurrent_tasks: usize,
    pub evidence_buffer_size: usize,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:50051".parse().unwrap(),
            project_root: std::env::current_dir().unwrap(),
            max_concurrent_tasks: 4,
            evidence_buffer_size: 32,
        }
    }
}

// ============================================================================
// ExecutionMode conversion
// ============================================================================

pub fn zn_execution_mode_to_proto(mode: zn_types::ExecutionMode) -> proto::ExecutionMode {
    match mode {
        zn_types::ExecutionMode::Brainstorming => proto::ExecutionMode::Brainstorming,
        zn_types::ExecutionMode::SpecCapture => proto::ExecutionMode::SpecCapture,
        zn_types::ExecutionMode::WritingPlans => proto::ExecutionMode::WritingPlans,
        zn_types::ExecutionMode::WorkspacePrepare => proto::ExecutionMode::WorkspacePrepare,
        zn_types::ExecutionMode::SubagentDev => proto::ExecutionMode::SubagentDev,
        zn_types::ExecutionMode::SubagentReview => proto::ExecutionMode::SubagentReview,
        zn_types::ExecutionMode::TddCycle => proto::ExecutionMode::TddCycle,
        zn_types::ExecutionMode::Verification => proto::ExecutionMode::Verification,
        zn_types::ExecutionMode::FinishBranch => proto::ExecutionMode::FinishBranch,
    }
}

pub fn proto_execution_mode_to_zn(mode: proto::ExecutionMode) -> zn_types::ExecutionMode {
    match mode {
        proto::ExecutionMode::Unknown => zn_types::ExecutionMode::Brainstorming,
        proto::ExecutionMode::Brainstorming => zn_types::ExecutionMode::Brainstorming,
        proto::ExecutionMode::SpecCapture => zn_types::ExecutionMode::SpecCapture,
        proto::ExecutionMode::WritingPlans => zn_types::ExecutionMode::WritingPlans,
        proto::ExecutionMode::WorkspacePrepare => zn_types::ExecutionMode::WorkspacePrepare,
        proto::ExecutionMode::SubagentDev => zn_types::ExecutionMode::SubagentDev,
        proto::ExecutionMode::SubagentReview => zn_types::ExecutionMode::SubagentReview,
        proto::ExecutionMode::TddCycle => zn_types::ExecutionMode::TddCycle,
        proto::ExecutionMode::Verification => zn_types::ExecutionMode::Verification,
        proto::ExecutionMode::FinishBranch => zn_types::ExecutionMode::FinishBranch,
    }
}

// ============================================================================
// WorkspaceStrategy conversion
// ============================================================================

pub fn zn_workspace_strategy_to_proto(strategy: zn_types::WorkspaceStrategy) -> proto::WorkspaceStrategy {
    match strategy {
        zn_types::WorkspaceStrategy::InPlace => proto::WorkspaceStrategy::InPlace,
        zn_types::WorkspaceStrategy::GitWorktree => proto::WorkspaceStrategy::GitWorktree,
        zn_types::WorkspaceStrategy::Sandboxed => proto::WorkspaceStrategy::Sandboxed,
    }
}

pub fn proto_workspace_strategy_to_zn(strategy: proto::WorkspaceStrategy) -> zn_types::WorkspaceStrategy {
    match strategy {
        proto::WorkspaceStrategy::Unknown => zn_types::WorkspaceStrategy::InPlace,
        proto::WorkspaceStrategy::InPlace => zn_types::WorkspaceStrategy::InPlace,
        proto::WorkspaceStrategy::GitWorktree => zn_types::WorkspaceStrategy::GitWorktree,
        proto::WorkspaceStrategy::Sandboxed => zn_types::WorkspaceStrategy::Sandboxed,
    }
}

// ============================================================================
// QualityGate conversion
// ============================================================================

pub fn zn_quality_gate_to_proto(gate: &zn_types::QualityGate) -> proto::QualityGate {
    proto::QualityGate {
        name: gate.name.clone(),
        required: gate.required,
        description: gate.description.clone(),
    }
}

pub fn proto_quality_gate_to_zn(gate: proto::QualityGate) -> zn_types::QualityGate {
    zn_types::QualityGate {
        name: gate.name,
        required: gate.required,
        description: gate.description,
    }
}

// ============================================================================
// TaskState conversion
// ============================================================================

pub fn zn_task_status_to_proto(status: zn_types::TaskStatus) -> proto::TaskState {
    match status {
        zn_types::TaskStatus::Pending => proto::TaskState::Queued,
        zn_types::TaskStatus::Running => proto::TaskState::Running,
        zn_types::TaskStatus::Completed => proto::TaskState::Completed,
        zn_types::TaskStatus::Failed => proto::TaskState::Failed,
        zn_types::TaskStatus::Blocked => proto::TaskState::Failed,
    }
}

pub fn proto_task_state_to_zn(state: proto::TaskState) -> zn_types::TaskStatus {
    match state {
        proto::TaskState::Unknown => zn_types::TaskStatus::Pending,
        proto::TaskState::Queued => zn_types::TaskStatus::Pending,
        proto::TaskState::Running => zn_types::TaskStatus::Running,
        proto::TaskState::Completed => zn_types::TaskStatus::Completed,
        proto::TaskState::Failed => zn_types::TaskStatus::Failed,
        proto::TaskState::Cancelled => zn_types::TaskStatus::Failed,
    }
}

// ============================================================================
// HostKind conversion
// ============================================================================

pub fn zn_host_kind_to_string(host: HostKind) -> String {
    match host {
        HostKind::ClaudeCode => "claude_code".to_string(),
        HostKind::OpenCode => "opencode".to_string(),
        HostKind::Terminal => "terminal".to_string(),
    }
}

pub fn string_to_zn_host_kind(s: &str) -> HostKind {
    match s.to_lowercase().as_str() {
        "claude_code" | "claude" => HostKind::ClaudeCode,
        "opencode" | "open" => HostKind::OpenCode,
        _ => HostKind::Terminal,
    }
}

// ============================================================================
// EvidenceKind conversion
// ============================================================================

pub fn zn_evidence_kind_to_string(kind: zn_types::EvidenceKind) -> String {
    match kind {
        zn_types::EvidenceKind::CommandOutput => "command_output".to_string(),
        zn_types::EvidenceKind::GeneratedArtifact => "generated_artifact".to_string(),
        zn_types::EvidenceKind::Review => "review".to_string(),
        zn_types::EvidenceKind::Verification => "verification".to_string(),
        zn_types::EvidenceKind::Workspace => "workspace".to_string(),
        zn_types::EvidenceKind::BranchAutomation => "branch_automation".to_string(),
        zn_types::EvidenceKind::Subagent => "subagent".to_string(),
    }
}

pub fn string_to_zn_evidence_kind(s: &str) -> zn_types::EvidenceKind {
    match s {
        "command_output" => zn_types::EvidenceKind::CommandOutput,
        "generated_artifact" => zn_types::EvidenceKind::GeneratedArtifact,
        "review" => zn_types::EvidenceKind::Review,
        "verification" => zn_types::EvidenceKind::Verification,
        "workspace" => zn_types::EvidenceKind::Workspace,
        "branch_automation" => zn_types::EvidenceKind::BranchAutomation,
        "subagent" => zn_types::EvidenceKind::Subagent,
        _ => zn_types::EvidenceKind::GeneratedArtifact,
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Convert a file path to a URI-safe string
pub fn path_to_uri(path: &Path) -> String {
    path.display().to_string()
}

/// Parse a URI-safe string back to a PathBuf
pub fn uri_to_path(uri: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_mode_conversion() {
        let mode = zn_types::ExecutionMode::SubagentDev;
        let proto_mode = zn_execution_mode_to_proto(mode);
        assert_eq!(proto_mode as i32, proto::ExecutionMode::SubagentDev as i32);
    }

    #[test]
    fn test_workspace_strategy_conversion() {
        let strategy = zn_types::WorkspaceStrategy::GitWorktree;
        let proto_strategy = zn_workspace_strategy_to_proto(strategy);
        assert_eq!(proto_strategy as i32, proto::WorkspaceStrategy::GitWorktree as i32);
    }

    #[test]
    fn test_host_kind_conversion() {
        let host = HostKind::ClaudeCode;
        let s = zn_host_kind_to_string(host);
        assert_eq!(s, "claude_code");

        let host_back = string_to_zn_host_kind("claude_code");
        assert_eq!(host_back, HostKind::ClaudeCode);
    }
}
