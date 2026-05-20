//! Core enums, configuration types, and project manifest.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum HostKind {
    ClaudeCode,
    OpenCode,
    #[default]
    Terminal,
}

impl std::fmt::Display for HostKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            HostKind::ClaudeCode => "claude-code",
            HostKind::OpenCode => "opencode",
            HostKind::Terminal => "terminal",
        };
        write!(f, "{}", value)
    }
}

// ============================================================================
// Agent Descriptor — open, extensible agent identity (multi-agent foundation)
// ============================================================================

/// What kind of agent this is.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    /// Built-in agent (maps to a HostKind variant)
    #[default]
    BuiltIn,
    /// External agent registered via SDK or gRPC bridge
    External,
    /// Human-in-the-loop agent
    Human,
}

/// A single capability that an agent possesses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Capability {
    /// Capability name, e.g. "code-generation", "review", "test", "security-audit"
    pub name: String,
    /// Proficiency level 0.0–1.0
    pub proficiency: f32,
    /// Maximum task complexity this agent handles well (0.0–1.0)
    pub max_complexity: f32,
}

/// Open agent descriptor — replaces the closed HostKind enum for multi-agent scenarios.
/// HostKind remains for backward compatibility with existing projects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDescriptor {
    /// UUID-format unique identifier
    pub agent_id: String,
    /// Human-readable name
    pub name: String,
    pub agent_type: AgentType,
    pub capabilities: Vec<Capability>,
    /// Trust score 0.0–1.0, starts at 0.5
    pub trust_score: f32,
    pub created_at: DateTime<Utc>,
}

impl AgentDescriptor {
    /// Convert existing HostKind to AgentDescriptor for migration.
    pub fn from_host_kind(host: &HostKind, agent_id: String) -> Self {
        let name = host.to_string();
        Self {
            agent_id,
            name,
            agent_type: AgentType::BuiltIn,
            capabilities: vec![Capability {
                name: "general".to_string(),
                proficiency: 0.7,
                max_complexity: 0.5,
            }],
            trust_score: 0.5,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub max_retries: u8,
    pub verify_before_complete: bool,
    pub auto_evolve: bool,
    #[serde(default)]
    pub max_total_iterations: Option<u32>,
    #[serde(default)]
    pub max_elapsed_seconds: Option<u64>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            verify_before_complete: true,
            auto_evolve: true,
            max_total_iterations: Some(50),
            max_elapsed_seconds: Some(3600),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub version: String,
    pub name: String,
    pub default_host: HostKind,
    pub skill_dirs: Vec<String>,
    pub policy: Policy,
    #[serde(default)]
    pub github_repo: Option<String>,
    #[serde(default)]
    pub bridge_address: Option<String>,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            version: "0.1.0".to_string(),
            name: "Zero_Nine".to_string(),
            default_host: HostKind::Terminal,
            skill_dirs: vec![".claude/skills".to_string(), ".opencode/skills".to_string()],
            policy: Policy::default(),
            github_repo: None,
            bridge_address: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Draft,
    Ready,
    Running,
    Completed,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Brainstorming,
    SpecCapture,
    WritingPlans,
    WorkspacePrepare,
    SubagentDev,
    SubagentReview,
    TddCycle,
    Verification,
    FinishBranch,
}

/// Subagent 执行路径选择 — 为独立部署预留双路径模式
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubagentExecutionPath {
    /// 直接 CLI 执行（默认）
    #[default]
    Cli,
    /// gRPC 桥接远程 agent
    Bridge,
    /// CLI 优先，失败时回退到桥接
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStrategy {
    LinearSequential,
    ParallelBatch,
    RiskGated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceStrategy {
    InPlace,
    GitWorktree,
    Sandboxed,
}

// ==================== Container Sandbox Types ====================

/// Network policy for container sandbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    None,
    Internal,
    #[default]
    Full,
}

/// Resource limits for container sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    #[serde(default)]
    pub memory_bytes: u64,
    #[serde(default)]
    pub cpu_cores: f32,
    #[serde(default)]
    pub disk_bytes: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            memory_bytes: 0,
            cpu_cores: 0.0,
            disk_bytes: 0,
        }
    }
}

/// Mount specification for container sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountSpec {
    pub host_path: String,
    pub container_path: String,
    #[serde(default)]
    pub read_only: bool,
}

/// Environment specification for container sandbox execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvironmentSpec {
    #[serde(default)]
    pub base_image: String,
    #[serde(default)]
    pub working_dir: String,
    #[serde(default)]
    pub env_vars: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub mount_points: Vec<MountSpec>,
    #[serde(default)]
    pub network_policy: NetworkPolicy,
    #[serde(default)]
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerdictStatus {
    Passed,
    Failed,
    Warning,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    CommandOutput,
    GeneratedArtifact,
    Review,
    Verification,
    Workspace,
    BranchAutomation,
    Subagent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Collected,
    Missing,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOutcome {
    Completed,
    RetryableFailure,
    Blocked,
    Escalated,
}

/// Brainstorm verdict
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrainstormVerdict {
    Continue,
    Ready,
    Escalate,
}

pub fn default_spec_schema_version() -> String {
    "zero_nine.stage1.v1".to_string()
}

pub fn slugify_goal(goal: &str) -> String {
    let lowered = goal.to_lowercase();
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in lowered.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let normalized: String = slug.trim_matches('-').chars().take(48).collect();
    if normalized.is_empty() {
        format!("goal-{:x}", goal.chars().count())
    } else {
        normalized
    }
}
