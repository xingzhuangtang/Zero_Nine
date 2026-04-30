//! Governance types: failure classification, safety events, policy engine, human supervision.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::VerdictStatus;

// ==================== Failure Classification ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    EnvironmentDrift,
    ToolError,
    VerificationFailed,
    PolicyBlocked,
    HumanRejected,
    ResourceExhausted,
    Timeout,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureClassification {
    pub id: String,
    pub category: FailureCategory,
    pub severity: FailureSeverity,
    pub description: String,
    #[serde(default)]
    pub root_cause: Option<String>,
    #[serde(default)]
    pub retry_recommended: bool,
    #[serde(default)]
    pub human_intervention_required: bool,
    #[serde(default)]
    pub suggested_fix: Option<String>,
}

impl Default for FailureClassification {
    fn default() -> Self {
        Self {
            id: String::new(),
            category: FailureCategory::Unknown,
            severity: FailureSeverity::Medium,
            description: String::new(),
            root_cause: None,
            retry_recommended: false,
            human_intervention_required: false,
            suggested_fix: None,
        }
    }
}

// ==================== Safety Events ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SafetyEventType {
    PolicyDenied,
    MergeBlocked,
    HumanIntervention,
    DriftHalt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyEvent {
    pub event_id: String,
    pub task_id: String,
    pub proposal_id: String,
    pub event_type: SafetyEventType,
    pub severity: FailureSeverity,
    pub description: String,
    pub action_taken: String,
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
}

impl Default for SafetyEvent {
    fn default() -> Self {
        Self {
            event_id: String::new(),
            task_id: String::new(),
            proposal_id: String::new(),
            event_type: SafetyEventType::PolicyDenied,
            severity: FailureSeverity::Medium,
            description: String::new(),
            action_taken: String::new(),
            timestamp: Utc::now(),
        }
    }
}

impl SafetyEvent {
    /// Derive a safety event from an execution report if guardrails were triggered.
    /// Returns None for clean completions.
    pub fn from_report(report: &crate::execution::ExecutionReport, proposal_id: &str) -> Option<Self> {
        let classification = report.failure_classification.as_ref()?;
        let (event_type, description) = match classification.category {
            FailureCategory::PolicyBlocked => (
                SafetyEventType::PolicyDenied,
                format!(
                    "Policy blocked task {}: {}",
                    report.task_id,
                    report
                        .failure_summary
                        .as_deref()
                        .unwrap_or("policy violation")
                ),
            ),
            FailureCategory::HumanRejected => (
                SafetyEventType::HumanIntervention,
                format!(
                    "Human intervention for task {}: {}",
                    report.task_id,
                    report
                        .failure_summary
                        .as_deref()
                        .unwrap_or("human review rejected")
                ),
            ),
            FailureCategory::EnvironmentDrift => (
                SafetyEventType::DriftHalt,
                format!(
                    "Drift halted task {}: {}",
                    report.task_id,
                    report
                        .failure_summary
                        .as_deref()
                        .unwrap_or("environment drift detected")
                ),
            ),
            _ => return None,
        };

        let severity = classification.severity.clone();

        Some(Self {
            event_id: format!("safety-{}-{}", report.task_id, Utc::now().timestamp()),
            task_id: report.task_id.clone(),
            proposal_id: proposal_id.to_string(),
            event_type,
            severity,
            description,
            action_taken: classification
                .suggested_fix
                .clone()
                .unwrap_or_else(|| "escalated".to_string()),
            timestamp: Utc::now(),
        })
    }

    /// Emit MergeBlocked when quality gates prevent merge/push.
    pub fn merge_blocked(
        task_id: &str,
        proposal_id: &str,
        tests_passed: bool,
        review_passed: bool,
    ) -> Self {
        let reason = match (tests_passed, review_passed) {
            (false, false) => "both tests and review failed",
            (false, true) => "tests failed",
            (true, false) => "review failed",
            (true, true) => "unknown",
        };
        Self {
            event_id: format!("safety-merge-{}-{}", task_id, Utc::now().timestamp()),
            task_id: task_id.to_string(),
            proposal_id: proposal_id.to_string(),
            event_type: SafetyEventType::MergeBlocked,
            severity: FailureSeverity::Critical,
            description: format!("Merge blocked: quality gates not passed ({})", reason),
            action_taken: "merge_blocked_escalated".to_string(),
            timestamp: Utc::now(),
        }
    }
}

// ==================== Compensation Actions ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompensationType {
    DeleteWorktree,
    DeleteBranch,
    CleanupArtifacts,
    ResetWorkspace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensationAction {
    pub action_type: CompensationType,
    pub target: String,
    pub reason: String,
    #[serde(default)]
    pub executed: bool,
}

// ==================== Enhanced Verdict ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    pub status: VerdictStatus,
    pub rationale: String,
    pub evidence_ids: Vec<String>,
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub reviewer: Option<String>,
}

impl Default for Verdict {
    fn default() -> Self {
        Self {
            status: VerdictStatus::Warning,
            rationale: String::new(),
            evidence_ids: Vec::new(),
            timestamp: Utc::now(),
            reviewer: None,
        }
    }
}

// ==================== Policy Engine ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ActionRiskLevel {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow,
    Ask,
    Deny,
    Escalate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    pub name: String,
    pub action_pattern: String,
    pub risk_level: ActionRiskLevel,
    pub default_decision: PolicyDecision,
    #[serde(default)]
    pub conditions: Vec<String>,
    #[serde(default)]
    pub exceptions: Vec<String>,
}

impl Default for PolicyRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            action_pattern: String::new(),
            risk_level: ActionRiskLevel::Medium,
            default_decision: PolicyDecision::Ask,
            conditions: Vec::new(),
            exceptions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEngine {
    pub rules: Vec<PolicyRule>,
    #[serde(default)]
    pub max_allowed_risk: ActionRiskLevel,
    #[serde(default)]
    pub require_human_for_high_risk: bool,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            max_allowed_risk: ActionRiskLevel::High,
            require_human_for_high_risk: true,
        }
    }
}

// ==================== Human Supervision ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionAction {
    Approve,
    Reject,
    Modify,
    Takeover,
    Delegate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanIntervention {
    pub id: String,
    pub task_id: String,
    pub action: SupervisionAction,
    pub rationale: String,
    #[serde(default)]
    pub modifications: Option<String>,
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub human_id: Option<String>,
}

impl Default for HumanIntervention {
    fn default() -> Self {
        Self {
            id: String::new(),
            task_id: String::new(),
            action: SupervisionAction::Approve,
            rationale: String::new(),
            modifications: None,
            timestamp: Utc::now(),
            human_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalTicket {
    pub id: String,
    pub task_id: String,
    pub action_description: String,
    pub risk_level: ActionRiskLevel,
    pub status: ApprovalStatus,
    #[serde(default)]
    pub approved_by: Option<String>,
    #[serde(default)]
    pub approved_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub rejection_reason: Option<String>,
}

impl Default for ApprovalTicket {
    fn default() -> Self {
        Self {
            id: String::new(),
            task_id: String::new(),
            action_description: String::new(),
            risk_level: ActionRiskLevel::Medium,
            status: ApprovalStatus::Pending,
            approved_by: None,
            approved_at: None,
            rejection_reason: None,
        }
    }
}
