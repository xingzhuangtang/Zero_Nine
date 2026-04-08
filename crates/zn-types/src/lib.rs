use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    ClaudeCode,
    OpenCode,
    Terminal,
}

impl Default for HostKind {
    fn default() -> Self {
        Self::Terminal
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub max_retries: u8,
    pub verify_before_complete: bool,
    pub auto_evolve: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            verify_before_complete: true,
            auto_evolve: true,
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
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            version: "0.1.0".to_string(),
            name: "Zero_Nine".to_string(),
            default_host: HostKind::Terminal,
            skill_dirs: vec![".claude/skills".to_string(), ".opencode/skills".to_string()],
            policy: Policy::default(),
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
pub enum LoopStage {
    Idle,
    SpecDrafting,
    Ready,
    RunningTask,
    Verifying,
    Retrying,
    Escalated,
    Archived,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceStrategy {
    InPlace,
    GitWorktree,
    Sandboxed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskContract {
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub deliverables: Vec<String>,
    #[serde(default)]
    pub verification_points: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub contract: TaskContract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    #[serde(default = "default_spec_schema_version")]
    pub schema_version: String,
    pub id: String,
    pub title: String,
    pub goal: String,
    pub status: ProposalStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub design_summary: Option<String>,
    #[serde(default)]
    pub source_brainstorm_session_id: Option<String>,

    // M1: Structured Spec Contract Fields (蓝图 M1-1)
    #[serde(default)]
    pub problem_statement: Option<String>,
    #[serde(default)]
    pub scope_in: Vec<String>,
    #[serde(default)]
    pub scope_out: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    #[serde(default)]
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    #[serde(default)]
    pub risks: Vec<Risk>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub non_goals: Vec<String>,

    pub tasks: Vec<TaskItem>,
}

/// M1-1: Constraint 结构 (蓝图 M1-1)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Constraint {
    pub id: String,
    pub category: ConstraintCategory,
    pub description: String,
    pub rationale: Option<String>,
    #[serde(default)]
    pub enforced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintCategory {
    Technical,
    Business,
    Regulatory,
    Performance,
    Security,
    Timeline,
    Resource,
}

/// M1-3: AcceptanceCriterion 结构 (蓝图 M1-3)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcceptanceCriterion {
    pub id: String,
    pub description: String,
    pub verification_method: VerificationMethod,
    pub priority: Priority,
    #[serde(default)]
    pub status: CriterionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    AutomatedTest,
    ManualInspection,
    PerformanceBenchmark,
    SecurityAudit,
    UserAcceptance,
    DocumentationReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CriterionStatus {
    Pending,
    Verified,
    Failed,
    Blocked,
}

impl Default for CriterionStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

/// M1-1: Risk 结构 (蓝图 M1-1)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Risk {
    pub id: String,
    pub description: String,
    pub probability: RiskProbability,
    pub impact: RiskImpact,
    pub mitigation: Option<String>,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskProbability {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskImpact {
    Low,
    Medium,
    High,
    Critical,
}

/// M1-1: Dependency 结构
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dependency {
    pub id: String,
    pub description: String,
    pub kind: DependencyKind,
    pub status: DependencyStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Internal,
    External,
    ThirdParty,
    Infrastructure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyStatus {
    Satisfied,
    Pending,
    Blocked,
    AtRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDependencyEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    #[serde(default = "default_spec_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub proposal_id: String,
    pub tasks: Vec<TaskItem>,
    #[serde(default)]
    pub edges: Vec<TaskDependencyEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopState {
    pub proposal_id: String,
    pub current_task: Option<String>,
    pub iteration: u32,
    pub retry_count: u8,
    pub stage: LoopStage,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    Info,
    Warning,
    Blocking,
    Dangerous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriftResponse {
    Continue,
    Replan,
    Confirm,
    Halt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesiredProjectState {
    #[serde(default = "default_spec_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub proposal_id: Option<String>,
    #[serde(default)]
    pub expected_branch: Option<String>,
    #[serde(default)]
    pub required_files: Vec<String>,
    #[serde(default)]
    pub expected_test_command: Option<String>,
    #[serde(default)]
    pub required_toolchains: Vec<String>,
    #[serde(default)]
    pub required_remote_capabilities: Vec<String>,
    #[serde(default)]
    pub require_clean_worktree: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActualProjectState {
    #[serde(default = "default_spec_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub proposal_id: Option<String>,
    #[serde(default)]
    pub current_branch: Option<String>,
    pub worktree_clean: bool,
    #[serde(default)]
    pub present_files: Vec<String>,
    #[serde(default)]
    pub available_toolchains: Vec<String>,
    #[serde(default)]
    pub detected_test_command: Option<String>,
    #[serde(default)]
    pub remote_capabilities: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    pub field: String,
    pub severity: DriftSeverity,
    pub expected: String,
    pub actual: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    #[serde(default = "default_spec_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub proposal_id: Option<String>,
    pub desired: DesiredProjectState,
    pub actual: ActualProjectState,
    #[serde(default)]
    pub diffs: Vec<StateDiff>,
    pub response: DriftResponse,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftCheckResult {
    pub report: DriftReport,
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteReadiness {
    pub git_remote_configured: bool,
    pub gh_available: bool,
    pub gh_authenticated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteReadinessCheck {
    pub required: bool,
    pub readiness: RemoteReadiness,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftPolicyDecision {
    pub max_allowed_severity: DriftSeverity,
    pub response: DriftResponse,
    pub rationale: String,
}

impl Default for DesiredProjectState {
    fn default() -> Self {
        Self {
            schema_version: default_spec_schema_version(),
            proposal_id: None,
            expected_branch: None,
            required_files: Vec::new(),
            expected_test_command: None,
            required_toolchains: Vec::new(),
            required_remote_capabilities: Vec::new(),
            require_clean_worktree: false,
        }
    }
}

impl Default for ActualProjectState {
    fn default() -> Self {
        Self {
            schema_version: default_spec_schema_version(),
            proposal_id: None,
            current_branch: None,
            worktree_clean: true,
            present_files: Vec::new(),
            available_toolchains: Vec::new(),
            detected_test_command: None,
            remote_capabilities: Vec::new(),
            notes: Vec::new(),
        }
    }
}

impl Default for RemoteReadiness {
    fn default() -> Self {
        Self {
            git_remote_configured: false,
            gh_available: false,
            gh_authenticated: false,
        }
    }
}

impl Default for DriftPolicyDecision {
    fn default() -> Self {
        Self {
            max_allowed_severity: DriftSeverity::Warning,
            response: DriftResponse::Continue,
            rationale: "Default policy allows only informational and warning-level drift to continue automatically.".to_string(),
        }
    }
}

impl DriftSeverity {
    pub fn rank(&self) -> u8 {
        match self {
            Self::Info => 0,
            Self::Warning => 1,
            Self::Blocking => 2,
            Self::Dangerous => 3,
        }
    }
}

impl DriftReport {
    pub fn highest_severity(&self) -> Option<DriftSeverity> {
        self.diffs
            .iter()
            .map(|diff| diff.severity.clone())
            .max_by_key(|severity| severity.rank())
    }
}

impl DriftCheckResult {
    pub fn from_report(report: DriftReport) -> Self {
        let blocking = matches!(
            report.highest_severity(),
            Some(DriftSeverity::Blocking | DriftSeverity::Dangerous)
        ) || matches!(
            report.response,
            DriftResponse::Confirm | DriftResponse::Halt
        );
        Self { report, blocking }
    }
}

impl RemoteReadiness {
    pub fn capabilities(&self) -> Vec<String> {
        let mut capabilities = Vec::new();
        if self.git_remote_configured {
            capabilities.push("git_remote".to_string());
        }
        if self.gh_available {
            capabilities.push("gh_cli".to_string());
        }
        if self.gh_authenticated {
            capabilities.push("gh_auth".to_string());
        }
        capabilities
    }
}

impl RemoteReadinessCheck {
    pub fn to_notes(&self) -> Vec<String> {
        let mut notes = vec![self.summary.clone()];
        if !self.readiness.git_remote_configured {
            notes.push("Git remote is not configured for this repository.".to_string());
        }
        if !self.readiness.gh_available {
            notes.push("GitHub CLI is not available in the current environment.".to_string());
        }
        if self.required && !self.readiness.gh_authenticated {
            notes.push(
                "GitHub CLI is available but not authenticated for remote PR or merge actions."
                    .to_string(),
            );
        }
        notes
    }
}

impl DriftPolicyDecision {
    pub fn allows(&self, severity: &DriftSeverity) -> bool {
        severity.rank() <= self.max_allowed_severity.rank()
    }
}

impl std::fmt::Display for DriftSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Blocking => "blocking",
            Self::Dangerous => "dangerous",
        };
        write!(f, "{}", value)
    }
}

impl std::fmt::Display for DriftResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Continue => "continue",
            Self::Replan => "replan",
            Self::Confirm => "confirm",
            Self::Halt => "halt",
        };
        write!(f, "{}", value)
    }
}

impl std::fmt::Display for StateDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} expected `{}` but found `{}` ({})",
            self.severity, self.field, self.expected, self.actual, self.message
        )
    }
}

impl std::fmt::Display for DriftReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (response: {})", self.summary, self.response)
    }
}

impl std::fmt::Display for DriftPolicyDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "allow up to {} then {}: {}",
            self.max_allowed_severity, self.response, self.rationale
        )
    }
}

impl std::fmt::Display for RemoteReadinessCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.summary)
    }
}

impl std::fmt::Display for RemoteReadiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "git_remote_configured={}, gh_available={}, gh_authenticated={}",
            self.git_remote_configured, self.gh_available, self.gh_authenticated
        )
    }
}

impl std::fmt::Display for DriftCheckResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.report)
    }
}

impl std::fmt::Display for DesiredProjectState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "expected_branch={:?}, expected_test_command={:?}, require_clean_worktree={}",
            self.expected_branch, self.expected_test_command, self.require_clean_worktree
        )
    }
}

impl std::fmt::Display for ActualProjectState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "current_branch={:?}, detected_test_command={:?}, worktree_clean={}",
            self.current_branch, self.detected_test_command, self.worktree_clean
        )
    }
}

pub fn default_drift_policy_for_required_remote(required: bool) -> DriftPolicyDecision {
    if required {
        DriftPolicyDecision {
            max_allowed_severity: DriftSeverity::Warning,
            response: DriftResponse::Confirm,
            rationale: "Remote actions need explicit confirmation and ready remote capabilities before continuing.".to_string(),
        }
    } else {
        DriftPolicyDecision::default()
    }
}

pub fn default_desired_project_state(proposal_id: Option<String>) -> DesiredProjectState {
    DesiredProjectState {
        proposal_id,
        require_clean_worktree: true,
        ..DesiredProjectState::default()
    }
}

pub fn empty_drift_report(
    proposal_id: Option<String>,
    desired: DesiredProjectState,
    actual: ActualProjectState,
) -> DriftReport {
    DriftReport {
        schema_version: default_spec_schema_version(),
        proposal_id,
        desired,
        actual,
        diffs: Vec::new(),
        response: DriftResponse::Continue,
        summary: "No project drift detected against the current expected state.".to_string(),
    }
}

pub fn blocking_drift_report(
    proposal_id: Option<String>,
    desired: DesiredProjectState,
    actual: ActualProjectState,
    diffs: Vec<StateDiff>,
    response: DriftResponse,
    summary: String,
) -> DriftReport {
    DriftReport {
        schema_version: default_spec_schema_version(),
        proposal_id,
        desired,
        actual,
        diffs,
        response,
        summary,
    }
}

pub fn summarize_state_diffs(diffs: &[StateDiff]) -> String {
    if diffs.is_empty() {
        "No project drift detected against the current expected state.".to_string()
    } else {
        diffs
            .iter()
            .map(|diff| diff.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

pub fn response_for_highest_severity(severity: Option<DriftSeverity>) -> DriftResponse {
    match severity {
        Some(DriftSeverity::Info | DriftSeverity::Warning) | None => DriftResponse::Continue,
        Some(DriftSeverity::Blocking) => DriftResponse::Replan,
        Some(DriftSeverity::Dangerous) => DriftResponse::Halt,
    }
}

pub fn has_blocking_drift(diffs: &[StateDiff]) -> bool {
    diffs.iter().any(|diff| {
        matches!(
            diff.severity,
            DriftSeverity::Blocking | DriftSeverity::Dangerous
        )
    })
}

pub fn highest_drift_severity(diffs: &[StateDiff]) -> Option<DriftSeverity> {
    diffs
        .iter()
        .map(|diff| diff.severity.clone())
        .max_by_key(|severity| severity.rank())
}

pub fn make_state_diff(
    field: impl Into<String>,
    severity: DriftSeverity,
    expected: impl Into<String>,
    actual: impl Into<String>,
    message: impl Into<String>,
) -> StateDiff {
    StateDiff {
        field: field.into(),
        severity,
        expected: expected.into(),
        actual: actual.into(),
        message: message.into(),
    }
}

pub fn make_remote_readiness_summary(readiness: &RemoteReadiness, required: bool) -> String {
    if required {
        format!(
            "Remote finish requires git remote + GitHub CLI auth readiness ({})",
            readiness
        )
    } else {
        format!("Remote readiness snapshot captured ({})", readiness)
    }
}

pub fn normalize_state_entries(values: &[String]) -> Vec<String> {
    let mut normalized = values
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

pub fn string_list_to_display(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

pub fn option_to_display(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| "-".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementPacket {
    #[serde(default = "default_spec_schema_version")]
    pub schema_version: String,
    pub user_goal: String,
    pub problem_statement: String,
    pub scope_in: Vec<String>,
    pub scope_out: Vec<String>,
    pub constraints: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub risks: Vec<String>,
    pub next_questions: Vec<String>,
    #[serde(default)]
    pub source_brainstorm_session_id: Option<String>,
    #[serde(default)]
    pub clarified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecBundle {
    pub proposal_path: String,
    pub requirements_path: String,
    pub acceptance_path: String,
    pub design_path: String,
    pub tasks_path: String,
    pub dag_path: String,
    pub progress_path: String,
    pub verification_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub title: String,
    pub rationale: String,
    pub expected_output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGate {
    pub name: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentBrief {
    pub role: String,
    pub goal: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreePlan {
    pub branch_name: String,
    pub worktree_path: String,
    pub strategy: WorkspaceStrategy,
    pub cleanup_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceStatus {
    Planned,
    Prepared,
    Active,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRecord {
    pub strategy: WorkspaceStrategy,
    pub status: WorkspaceStatus,
    pub branch_name: String,
    pub worktree_path: String,
    pub base_branch: Option<String>,
    pub head_branch: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FinishBranchAction {
    Merge,
    PullRequest,
    Discard,
    Keep,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FinishBranchStatus {
    Planned,
    Completed,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinishBranchResult {
    pub action: FinishBranchAction,
    pub status: FinishBranchStatus,
    pub branch_name: String,
    pub worktree_path: Option<String>,
    pub summary: String,
    pub follow_ups: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunRecord {
    pub role: String,
    pub status: String,
    pub summary: String,
    pub outputs: Vec<String>,
    #[serde(default)]
    pub evidence_paths: Vec<String>,
    #[serde(default)]
    pub failure_summary: Option<String>,
    #[serde(default)]
    pub state_transitions: Vec<String>,
    #[serde(default)]
    pub recovery_path: Option<String>,
    #[serde(default)]
    pub evidence_archive_path: Option<String>,
    #[serde(default)]
    pub replay_ready: bool,
    #[serde(default)]
    pub replay_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubagentRunStatus {
    Planned,
    Dispatched,
    Recovered,
    Failed,
    Blocked,
}

impl std::fmt::Display for SubagentRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Planned => "planned",
            Self::Dispatched => "dispatched",
            Self::Recovered => "recovered",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
        };
        write!(f, "{}", value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentRecoveryRecord {
    pub role: String,
    pub status: SubagentRunStatus,
    pub summary: String,
    #[serde(default)]
    pub expected_outputs: Vec<String>,
    #[serde(default)]
    pub actual_outputs: Vec<String>,
    #[serde(default)]
    pub evidence_paths: Vec<String>,
    #[serde(default)]
    pub failure_summary: Option<String>,
    #[serde(default)]
    pub state_transitions: Vec<String>,
    #[serde(default)]
    pub evidence_archive_path: Option<String>,
    #[serde(default)]
    pub replay_ready: bool,
    #[serde(default)]
    pub replay_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentRecoveryLedger {
    pub task_id: String,
    pub records: Vec<SubagentRecoveryRecord>,
    #[serde(default)]
    pub replay_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentExecutionRuntime {
    pub runbook_path: String,
    #[serde(default)]
    pub dispatch_paths: Vec<String>,
    #[serde(default)]
    pub recovery_paths: Vec<String>,
    #[serde(default)]
    pub replay_paths: Vec<String>,
    pub ledger_path: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Collected,
    Missing,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub key: String,
    pub label: String,
    pub kind: EvidenceKind,
    pub status: EvidenceStatus,
    pub required: bool,
    pub summary: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewVerdict {
    pub approved: bool,
    pub status: VerdictStatus,
    pub summary: String,
    pub risks: Vec<String>,
    pub evidence_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationVerdict {
    pub passed: bool,
    pub status: VerdictStatus,
    pub summary: String,
    pub evidence: Vec<String>,
    pub evidence_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainstormSession {
    pub id: String,
    pub goal: String,
    pub host: HostKind,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub questions: Vec<ClarificationQuestion>,
    pub answers: Vec<ClarificationAnswer>,
    pub verdict: BrainstormVerdict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarificationQuestion {
    pub id: String,
    pub question: String,
    pub rationale: String,
    pub priority: u8,
    pub answered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarificationAnswer {
    pub question_id: String,
    pub answer: String,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrainstormVerdict {
    Continue,
    Ready,
    Escalate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkCapability {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkManifest {
    pub version: String,
    pub capabilities: Vec<SdkCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentDispatch {
    pub role: String,
    pub command_hint: String,
    pub context_files: Vec<String>,
    pub expected_outputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentRunBook {
    pub task_id: String,
    pub dispatches: Vec<SubagentDispatch>,
    #[serde(default)]
    pub runtime: Option<SubagentExecutionRuntime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePreparationResult {
    pub success: bool,
    pub summary: String,
    pub record: Option<WorkspaceRecord>,
    pub created_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchFinishRequest {
    pub action: FinishBranchAction,
    pub branch_name: String,
    pub worktree_path: Option<String>,
    pub verify_clean: bool,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchFinishPreview {
    pub request: BranchFinishRequest,
    pub warnings: Vec<String>,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroNineSdkConfig {
    pub project_root: String,
    pub host: HostKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroNineRunRequest {
    pub goal: String,
    pub host: HostKind,
    pub project_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroNineRunResponse {
    pub proposal_id: Option<String>,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroNineStatusResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroNineExportResponse {
    pub exported_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroNineBrainstormRequest {
    pub goal: String,
    pub host: HostKind,
    pub resume: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroNineBrainstormResponse {
    pub session_id: String,
    pub verdict: BrainstormVerdict,
    pub question_count: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroNineWorkspaceRequest {
    pub task_id: String,
    pub proposal_id: String,
    pub strategy: WorkspaceStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroNineWorkspaceResponse {
    pub success: bool,
    pub summary: String,
    pub branch_name: Option<String>,
    pub worktree_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroNineFinishBranchResponse {
    pub success: bool,
    pub summary: String,
    pub action: FinishBranchAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroNineSubagentResponse {
    pub task_id: String,
    pub generated_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextArtifact {
    pub path: String,
    pub role: String,
    pub required: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextInjectionProtocol {
    pub version: String,
    pub host: HostKind,
    pub mode: ExecutionMode,
    pub objective: String,
    pub artifacts: Vec<ContextArtifact>,
    pub instructions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationAction {
    pub name: String,
    pub command: String,
    pub required: bool,
    pub expected_evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationActionResult {
    pub name: String,
    pub status: String,
    pub summary: String,
    pub evidence_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOutcome {
    Completed,
    RetryableFailure,
    Blocked,
    Escalated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinishBranchAutomation {
    pub default_action: FinishBranchAction,
    pub available_actions: Vec<FinishBranchAction>,
    pub requires_clean_tree: bool,
    pub preview_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEnvelope {
    pub proposal_id: String,
    pub task_id: String,
    pub task_title: String,
    pub execution_mode: ExecutionMode,
    pub workspace_strategy: WorkspaceStrategy,
    pub context_files: Vec<String>,
    pub context_protocol: Option<ContextInjectionProtocol>,
    pub context_protocol_path: Option<String>,
    pub quality_gates: Vec<QualityGate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedArtifact {
    pub path: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub task_id: String,
    pub objective: String,
    pub mode: ExecutionMode,
    pub workspace_strategy: WorkspaceStrategy,
    pub steps: Vec<PlanStep>,
    pub validation: Vec<String>,
    pub quality_gates: Vec<QualityGate>,
    pub skill_chain: Vec<String>,
    pub deliverables: Vec<String>,
    pub risks: Vec<String>,
    pub subagents: Vec<SubagentBrief>,
    pub worktree_plan: Option<WorktreePlan>,
    pub workspace_record: Option<WorkspaceRecord>,
    pub verification_actions: Vec<VerificationAction>,
    pub finish_branch_automation: Option<FinishBranchAutomation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub task_id: String,
    pub success: bool,
    pub outcome: ExecutionOutcome,
    pub summary: String,
    pub details: Vec<String>,
    pub tests_passed: bool,
    pub review_passed: bool,
    pub artifacts: Vec<String>,
    pub generated_artifacts: Vec<GeneratedArtifact>,
    pub evidence: Vec<EvidenceRecord>,
    pub follow_ups: Vec<String>,
    pub workspace_record: Option<WorkspaceRecord>,
    pub finish_branch_result: Option<FinishBranchResult>,
    pub finish_branch_automation: Option<FinishBranchAutomation>,
    pub agent_runs: Vec<AgentRunRecord>,
    pub review_verdict: Option<ReviewVerdict>,
    pub verification_verdict: Option<VerificationVerdict>,
    pub verification_actions: Vec<VerificationAction>,
    pub verification_action_results: Vec<VerificationActionResult>,
    pub failure_summary: Option<String>,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpecValidationSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecValidationIssue {
    pub severity: SpecValidationSeverity,
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecValidationReport {
    #[serde(default = "default_spec_schema_version")]
    pub schema_version: String,
    pub proposal_id: String,
    pub valid: bool,
    pub issues: Vec<SpecValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressRecord {
    pub proposal_id: String,
    pub completed: Vec<String>,
    pub pending: Vec<String>,
    pub blocked: Vec<String>,
    #[serde(default)]
    pub runnable: Vec<String>,
    #[serde(default)]
    pub blocked_details: Vec<String>,
    #[serde(default)]
    pub scheduler_summary: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEvaluation {
    pub skill_name: String,
    pub task_type: String,
    pub latency_ms: u64,
    pub token_cost: u64,
    pub score: f32,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionKind {
    AutoFix,
    AutoImprove,
    AutoLearn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionSignal {
    pub task_id: String,
    pub score: f32,
    pub decision: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionCandidate {
    pub source_skill: String,
    pub kind: EvolutionKind,
    pub reason: String,
    pub patch: String,
    pub confidence: f32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEvent {
    pub ts: DateTime<Utc>,
    pub event: String,
    pub proposal_id: Option<String>,
    pub task_id: Option<String>,
    pub payload: Option<Value>,
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

// ==================== M1: Default Implementations ====================

impl Default for Constraint {
    fn default() -> Self {
        Self {
            id: String::new(),
            category: ConstraintCategory::Technical,
            description: String::new(),
            rationale: None,
            enforced: false,
        }
    }
}

impl Default for AcceptanceCriterion {
    fn default() -> Self {
        Self {
            id: String::new(),
            description: String::new(),
            verification_method: VerificationMethod::AutomatedTest,
            priority: Priority::Medium,
            status: CriterionStatus::Pending,
        }
    }
}

impl Default for Risk {
    fn default() -> Self {
        Self {
            id: String::new(),
            description: String::new(),
            probability: RiskProbability::Medium,
            impact: RiskImpact::Medium,
            mitigation: None,
            owner: None,
        }
    }
}

impl Default for Dependency {
    fn default() -> Self {
        Self {
            id: String::new(),
            description: String::new(),
            kind: DependencyKind::Internal,
            status: DependencyStatus::Pending,
        }
    }
}

impl Default for Proposal {
    fn default() -> Self {
        Self {
            schema_version: default_spec_schema_version(),
            id: String::new(),
            title: String::new(),
            goal: String::new(),
            status: ProposalStatus::Draft,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            design_summary: None,
            source_brainstorm_session_id: None,
            problem_statement: None,
            scope_in: Vec::new(),
            scope_out: Vec::new(),
            constraints: Vec::new(),
            acceptance_criteria: Vec::new(),
            risks: Vec::new(),
            dependencies: Vec::new(),
            non_goals: Vec::new(),
            tasks: Vec::new(),
        }
    }
}

// ==================== M1-2: DAG Validation ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DagValidationError {
    pub error_code: DagErrorCode,
    pub message: String,
    pub involved_task_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DagErrorCode {
    CircularDependency,
    MissingDependency,
    SelfReference,
    DuplicateTaskId,
    EmptyTaskGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagValidationResult {
    pub valid: bool,
    pub errors: Vec<DagValidationError>,
    pub warnings: Vec<String>,
    pub critical_path: Vec<String>,
    pub max_depth: u32,
}

impl TaskGraph {
    /// M1-2: Validate DAG for cycles, missing dependencies, self-references
    pub fn validate_dag(&self) -> DagValidationResult {
        let mut errors = Vec::new();
        let warnings = Vec::new();

        // Check for empty graph
        if self.tasks.is_empty() {
            errors.push(DagValidationError {
                error_code: DagErrorCode::EmptyTaskGraph,
                message: "Task graph contains no tasks".to_string(),
                involved_task_ids: Vec::new(),
            });
            return DagValidationResult {
                valid: false,
                errors,
                warnings,
                critical_path: Vec::new(),
                max_depth: 0,
            };
        }

        // Check for duplicate task IDs
        let mut task_ids = std::collections::HashMap::new();
        for task in &self.tasks {
            if task_ids.contains_key(&task.id) {
                errors.push(DagValidationError {
                    error_code: DagErrorCode::DuplicateTaskId,
                    message: format!("Duplicate task ID: {}", task.id),
                    involved_task_ids: vec![task.id.clone()],
                });
            } else {
                task_ids.insert(task.id.clone(), task);
            }
        }

        // Check for self-references and missing dependencies
        for task in &self.tasks {
            for dep in &task.depends_on {
                if dep == &task.id {
                    errors.push(DagValidationError {
                        error_code: DagErrorCode::SelfReference,
                        message: format!("Task {} references itself", task.id),
                        involved_task_ids: vec![task.id.clone()],
                    });
                } else if !task_ids.contains_key(dep) {
                    errors.push(DagValidationError {
                        error_code: DagErrorCode::MissingDependency,
                        message: format!("Task {} depends on non-existent task {}", task.id, dep),
                        involved_task_ids: vec![task.id.clone(), dep.clone()],
                    });
                }
            }
        }

        // Check for circular dependencies using DFS
        let cycles = self.detect_cycles();
        if !cycles.is_empty() {
            let involved: Vec<String> = cycles.iter().flatten().cloned().collect();
            errors.push(DagValidationError {
                error_code: DagErrorCode::CircularDependency,
                message: format!("Circular dependency detected: {}", cycles[0].join(" -> ")),
                involved_task_ids: involved,
            });
        }

        let critical_path = self.compute_critical_path();
        let max_depth = self.compute_max_depth();

        DagValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
            critical_path,
            max_depth,
        }
    }

    fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut rec_stack = std::collections::HashSet::new();
        let mut path = Vec::new();

        for task in &self.tasks {
            if !visited.contains(&task.id) {
                self.dfs_cycle_check(task, &mut visited, &mut rec_stack, &mut path, &mut cycles);
            }
        }

        cycles
    }

    fn dfs_cycle_check(
        &self,
        task: &TaskItem,
        visited: &mut std::collections::HashSet<String>,
        rec_stack: &mut std::collections::HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(task.id.clone());
        rec_stack.insert(task.id.clone());
        path.push(task.id.clone());

        for dep_id in &task.depends_on {
            if !visited.contains(dep_id) {
                if let Some(dep_task) = self.tasks.iter().find(|t| &t.id == dep_id) {
                    self.dfs_cycle_check(dep_task, visited, rec_stack, path, cycles);
                }
            } else if rec_stack.contains(dep_id) {
                let cycle_start = path.iter().position(|x| x == dep_id).unwrap_or(0);
                let mut cycle: Vec<String> = path[cycle_start..].to_vec();
                cycle.push(dep_id.clone());
                cycles.push(cycle);
            }
        }

        path.pop();
        rec_stack.remove(&task.id);
    }

    fn compute_critical_path(&self) -> Vec<String> {
        // Simple implementation: find longest path in DAG
        let mut dist: std::collections::HashMap<String, (u32, String)> =
            std::collections::HashMap::new();

        // Initialize
        for task in &self.tasks {
            dist.insert(task.id.clone(), (0, String::new()));
        }

        // Topological sort and longest path
        let mut result = Vec::new();
        let mut in_degree: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for task in &self.tasks {
            in_degree.insert(task.id.clone(), task.depends_on.len());
        }

        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        while let Some(task_id) = queue.pop() {
            result.push(task_id.clone());
            if let Some(&(d, _)) = dist.get(&task_id) {
                for task in &self.tasks {
                    if task.depends_on.contains(&task_id) {
                        let new_dist = d + 1;
                        if let Some(entry) = dist.get_mut(&task.id) {
                            if new_dist > entry.0 {
                                *entry = (new_dist, task_id.clone());
                            }
                        }
                        if let Some(deg) = in_degree.get_mut(&task.id) {
                            *deg -= 1;
                            if *deg == 0 {
                                queue.push(task.id.clone());
                            }
                        }
                    }
                }
            }
        }

        // Backtrack to find critical path
        if result.is_empty() {
            return Vec::new();
        }

        let mut max_task = result[0].clone();
        let mut max_dist = 0;
        for (task_id, &(d, _)) in &dist {
            if d > max_dist {
                max_dist = d;
                max_task = task_id.clone();
            }
        }

        let mut critical_path = Vec::new();
        let mut current = max_task;
        while !current.is_empty() {
            critical_path.push(current.clone());
            if let Some((_, prev)) = dist.get(&current) {
                current = prev.clone();
            } else {
                break;
            }
        }
        critical_path.reverse();
        critical_path
    }

    fn compute_max_depth(&self) -> u32 {
        let mut depths: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

        fn compute_depth(
            task_id: &str,
            tasks: &[TaskItem],
            depths: &mut std::collections::HashMap<String, u32>,
        ) -> u32 {
            if let Some(&d) = depths.get(task_id) {
                return d;
            }

            let task = match tasks.iter().find(|t| &t.id == task_id) {
                Some(t) => t,
                None => return 0,
            };

            let depth = if task.depends_on.is_empty() {
                1
            } else {
                task.depends_on
                    .iter()
                    .map(|dep| compute_depth(dep, tasks, depths))
                    .max()
                    .unwrap_or(0)
                    + 1
            };

            depths.insert(task_id.to_string(), depth);
            depth
        }

        self.tasks
            .iter()
            .map(|t| compute_depth(&t.id, &self.tasks, &mut depths))
            .max()
            .unwrap_or(0)
    }
}
