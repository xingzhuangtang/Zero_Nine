//! Drift detection types: desired/actual project state, diffs, reports, and policy decisions.

use serde::{Deserialize, Serialize};

use crate::core::default_spec_schema_version;

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
#[derive(Default)]
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

// ==================== Default Implementations ====================

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


impl Default for DriftPolicyDecision {
    fn default() -> Self {
        Self {
            max_allowed_severity: DriftSeverity::Warning,
            response: DriftResponse::Continue,
            rationale: "Default policy allows only informational and warning-level drift to continue automatically.".to_string(),
        }
    }
}

// ==================== Impl Methods ====================

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

// ==================== Display Implementations ====================

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

// ==================== Helper Functions ====================

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
