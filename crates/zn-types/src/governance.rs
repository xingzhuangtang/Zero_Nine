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
    pub fn from_report(
        report: &crate::execution::ExecutionReport,
        proposal_id: &str,
    ) -> Option<Self> {
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

impl ActionRiskLevel {
    /// Numeric ranking for risk level comparison: Low=0, Medium=1, High=2, Critical=3
    pub fn rank(&self) -> u8 {
        match self {
            ActionRiskLevel::Low => 0,
            ActionRiskLevel::Medium => 1,
            ActionRiskLevel::High => 2,
            ActionRiskLevel::Critical => 3,
        }
    }
}

impl PolicyEngine {
    /// Evaluate whether an action is allowed based on matching rules.
    /// Returns the decision of the highest-risk matching rule, or Allow if no rule matches.
    pub fn evaluate_action(&self, action: &str, conditions_met: &[&str]) -> PolicyDecision {
        let mut matching_rules: Vec<&PolicyRule> = self
            .rules
            .iter()
            .filter(|rule| rule.action_pattern.is_empty() || action.contains(&rule.action_pattern))
            .collect();
        matching_rules.sort_by_key(|r| r.risk_level.rank());
        matching_rules.reverse();

        for rule in matching_rules {
            let is_exception = rule.exceptions.iter().any(|e| action.contains(e));
            if is_exception {
                continue;
            }
            let all_conditions_met = rule.conditions.is_empty()
                || rule
                    .conditions
                    .iter()
                    .all(|c| conditions_met.contains(&c.as_str()));
            if all_conditions_met {
                return rule.default_decision.clone();
            } else {
                // Highest-risk rule matched but conditions not met → deny
                return PolicyDecision::Deny;
            }
        }
        PolicyDecision::Allow
    }

    /// Check if a specific permission is granted for an action.
    pub fn check_permission(&self, action: &str, conditions_met: &[&str]) -> bool {
        matches!(
            self.evaluate_action(action, conditions_met),
            PolicyDecision::Allow
        )
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

// ==================== RBAC ====================

/// RBAC role definition for governance access control.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceRole {
    Admin,    // Full permissions
    Approver, // Can approve high/critical-risk actions
    Executor, // Can execute low/medium-risk actions
    Reviewer, // Read-only + audit review
    #[default]
    Observer, // Pure audit viewer
}

/// Role permission configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolePermission {
    pub role: GovernanceRole,
    /// Maximum risk level (numeric rank from ActionRiskLevel::rank)
    pub max_risk_level: u8,
    pub can_approve: bool,
    pub can_modify_policy: bool,
    pub allowed_actions: Vec<String>, // ActionType names
}

impl Default for RolePermission {
    fn default() -> Self {
        Self {
            role: GovernanceRole::Observer,
            max_risk_level: 0,
            can_approve: false,
            can_modify_policy: false,
            allowed_actions: Vec::new(),
        }
    }
}

/// RBAC store mapping users to roles and permissions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RBACStore {
    pub roles: std::collections::HashMap<String, GovernanceRole>, // user_id -> role
    pub permissions: Vec<RolePermission>,
}

// ==================== Audit Log ====================

/// Audit log entry with hash-chain integrity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String, // UUID
    pub timestamp: DateTime<Utc>,
    pub action: String,     // ActionType name
    pub risk_level: String, // "Low" | "Medium" | "High" | "Critical"
    pub decision: String,   // "Allow" | "Deny" | "RequireApproval" | "Block"
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>, // Which agent performed this action
    #[serde(default)]
    pub task_id: Option<String>,
    pub details: String,
    pub entry_hash: String, // SHA256 of this entry
    pub prev_hash: String,  // Hash of previous entry
}

impl Default for AuditEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            timestamp: Utc::now(),
            action: String::new(),
            risk_level: String::new(),
            decision: String::new(),
            user_id: None,
            agent_id: None,
            task_id: None,
            details: String::new(),
            entry_hash: String::new(),
            prev_hash: String::new(),
        }
    }
}

/// Audit query filter.
#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    pub action: Option<String>,
    pub user_id: Option<String>,
    pub risk_level: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: usize,
}

// ==================== Compliance ====================

/// Policy violation record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyViolation {
    pub action: String,
    pub risk_level: String,
    pub decision: String,
    pub timestamp: DateTime<Utc>,
    pub description: String,
}

/// Compliance check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceCheckResult {
    pub name: String,
    pub passed: bool,
    pub severity: String,
    pub description: String,
}

/// Compliance report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub compliance_score: f32, // 0-100
    pub total_actions: usize,
    pub policy_violations: Vec<PolicyViolation>,
    pub compliance_checks: Vec<ComplianceCheckResult>,
    pub recommendations: Vec<String>,
}

// ==================== Ticket Resolution ====================

/// Approval ticket resolution record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketResolution {
    pub ticket_id: String,
    pub resolved_by: String,
    pub decision: String, // "Approved" | "Rejected"
    pub rationale: String,
    pub resolved_at: DateTime<Utc>,
}

// ==================== Rate Limiting ====================

/// Rate limit configuration for action throttling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub max_actions_per_window: u32,
    pub window_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_actions_per_window: 100,
            window_seconds: 3600,
        }
    }
}

// ==================== Multi-Agent Trust Framework ====================

/// Resource quota limits for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u64,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,
    #[serde(default)]
    pub max_storage: u64,
    #[serde(default)]
    pub allowed_strategies: Vec<String>,
}

fn default_max_tokens() -> u64 {
    32000
}

fn default_max_concurrent() -> u32 {
    3
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            max_tokens: default_max_tokens(),
            max_concurrent: default_max_concurrent(),
            max_storage: 0,
            allowed_strategies: Vec::new(),
        }
    }
}

/// Isolation rules defining what resources an agent can access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIsolationRule {
    #[serde(default)]
    pub allowed_resource_prefixes: Vec<String>,
    #[serde(default)]
    pub deny_patterns: Vec<String>,
    #[serde(default)]
    pub network_access: bool,
    #[serde(default)]
    pub filesystem_access: bool,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
}

impl Default for AgentIsolationRule {
    fn default() -> Self {
        Self {
            allowed_resource_prefixes: Vec::new(),
            deny_patterns: Vec::new(),
            network_access: true,
            filesystem_access: true,
            allowed_paths: Vec::new(),
        }
    }
}

/// Permission profile for an agent in the governance system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPermission {
    pub agent_id: String,
    #[serde(default)]
    pub role: String,
    #[serde(default = "default_max_risk")]
    pub max_risk_level: f32,
    #[serde(default)]
    pub can_dispatch_to: Vec<String>,
    #[serde(default)]
    pub resource_quotas: ResourceQuota,
    #[serde(default)]
    pub isolation_rules: AgentIsolationRule,
}

fn default_max_risk() -> f32 {
    0.5
}

impl Default for AgentPermission {
    fn default() -> Self {
        Self {
            agent_id: String::new(),
            role: String::new(),
            max_risk_level: default_max_risk(),
            can_dispatch_to: Vec::new(),
            resource_quotas: ResourceQuota::default(),
            isolation_rules: AgentIsolationRule::default(),
        }
    }
}
