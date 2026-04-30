//! Execution plan, reports, workspace management, brainstorm sessions, and SDK types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::*;
use crate::governance::FailureClassification;

// ==================== Spec & Requirement Types ====================

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

// ==================== Plan & Quality Types ====================

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
    #[serde(default)]
    pub depends_on: Vec<String>,
}

// ==================== Workspace Types ====================

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

// ==================== Branch Finish Types ====================

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
    #[serde(default)]
    pub pr_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinishBranchAutomation {
    pub default_action: FinishBranchAction,
    pub available_actions: Vec<FinishBranchAction>,
    pub requires_clean_tree: bool,
    pub preview_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchFinishRequest {
    pub action: FinishBranchAction,
    pub branch_name: String,
    pub worktree_path: Option<String>,
    pub verify_clean: bool,
    pub confirmed: bool,
    #[serde(default)]
    pub pr_title: Option<String>,
    #[serde(default)]
    pub pr_body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchFinishPreview {
    pub request: BranchFinishRequest,
    pub warnings: Vec<String>,
    pub commands: Vec<String>,
}

// ==================== Subagent Types ====================

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentDispatch {
    pub role: String,
    pub command_hint: String,
    pub context_files: Vec<String>,
    pub expected_outputs: Vec<String>,
    #[serde(default)]
    pub depends_on_roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentRunBook {
    pub task_id: String,
    pub dispatches: Vec<SubagentDispatch>,
    #[serde(default)]
    pub runtime: Option<SubagentExecutionRuntime>,
}

// ==================== Evidence & Verification Types ====================

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

// ==================== Context Injection ====================

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

// ==================== Execution Envelope & Plan ====================

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
    #[serde(default)]
    pub bridge_address: Option<String>,
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
    #[serde(default)]
    pub execution_path: SubagentExecutionPath,
    #[serde(default)]
    pub bridge_address: Option<String>,
}

// ==================== Execution Report ====================

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
    #[serde(default)]
    pub execution_time_ms: u64,
    #[serde(default)]
    pub token_count: u64,
    #[serde(default)]
    pub code_quality_score: f32,
    #[serde(default)]
    pub test_coverage: f32,
    #[serde(default)]
    pub user_feedback: Option<UserFeedback>,
    #[serde(default)]
    pub failure_classification: Option<FailureClassification>,
    #[serde(default)]
    pub tri_role_verdict: Option<String>,
    #[serde(default)]
    pub authorization_ticket_id: Option<String>,
    #[serde(default)]
    pub authorized_by: Option<String>,
}

// ==================== Spec Validation ====================

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
    #[serde(default)]
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecValidationReport {
    #[serde(default = "default_spec_schema_version")]
    pub schema_version: String,
    pub proposal_id: String,
    pub valid: bool,
    pub issues: Vec<SpecValidationIssue>,
}

// ==================== Progress & Feedback ====================

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

/// User feedback for a task execution (added in v1.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFeedback {
    pub task_id: String,
    pub rating: u8,
    pub comment: Option<String>,
    #[serde(default)]
    pub preferred_aspects: Vec<String>,
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
}

impl Default for UserFeedback {
    fn default() -> Self {
        Self {
            task_id: String::new(),
            rating: 3,
            comment: None,
            preferred_aspects: Vec::new(),
            timestamp: Utc::now(),
        }
    }
}

/// User feedback summary (added in v1.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFeedbackSummary {
    pub total_feedback: u32,
    pub avg_rating: f32,
    pub common_positive_aspects: Vec<String>,
    pub common_negative_aspects: Vec<String>,
}

// ==================== Workspace Preparation ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePreparationResult {
    pub success: bool,
    pub summary: String,
    pub record: Option<WorkspaceRecord>,
    pub created_paths: Vec<String>,
}

// ==================== Brainstorm Session ====================

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

// ==================== Runtime Events & Observability ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEvent {
    pub ts: DateTime<Utc>,
    pub event: String,
    pub proposal_id: Option<String>,
    pub task_id: Option<String>,
    pub payload: Option<Value>,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub span_id: Option<String>,
    #[serde(default)]
    pub parent_span_id: Option<String>,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    #[serde(default)]
    pub metadata: Option<serde_json::Map<String, Value>>,
}

impl RuntimeEvent {
    pub fn new(event: String, payload: Option<Value>) -> Self {
        Self {
            ts: Utc::now(),
            event,
            proposal_id: None,
            task_id: None,
            payload,
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            latency_ms: None,
            metadata: None,
        }
    }

    pub fn with_context(mut self, proposal_id: Option<String>, task_id: Option<String>) -> Self {
        self.proposal_id = proposal_id;
        self.task_id = task_id;
        self
    }

    pub fn with_trace(
        mut self,
        trace_id: Option<String>,
        span_id: Option<String>,
        parent_span_id: Option<String>,
    ) -> Self {
        self.trace_id = trace_id;
        self.span_id = span_id;
        self.parent_span_id = parent_span_id;
        self
    }
}

/// Trace context for cross-cutting observability (Layer 13)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    #[serde(default)]
    pub attributes: std::collections::HashMap<String, String>,
}

impl TraceContext {
    pub fn new() -> Self {
        let uuid = format!("{}", uuid::Uuid::new_v4().simple());
        Self {
            trace_id: uuid.clone(),
            span_id: format!("{}-001", &uuid[..8]),
            parent_span_id: None,
            attributes: std::collections::HashMap::new(),
        }
    }

    pub fn child(&self, span_suffix: &str) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: format!("{}-{}", &self.span_id[..8], span_suffix),
            parent_span_id: Some(self.span_id.clone()),
            attributes: self.attributes.clone(),
        }
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics snapshot for aggregation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsSnapshot {
    pub task_id: String,
    pub proposal_id: Option<String>,
    pub start_ts: DateTime<Utc>,
    pub end_ts: Option<DateTime<Utc>>,
    pub latency_ms: u64,
    pub token_usage: u64,
    pub subagent_count: u32,
    pub evidence_count: u32,
    pub success: bool,
    #[serde(default)]
    pub custom_metrics: std::collections::HashMap<String, Value>,
}

// ==================== SDK Types ====================

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


// ==================== Evolution Signal Types ====================

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
