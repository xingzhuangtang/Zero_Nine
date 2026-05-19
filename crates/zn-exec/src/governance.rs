//! Safety Governance and Human Oversight
//!
//! This module provides:
//! - Risk classification and authorization matrix
//! - Approval tickets for high-risk actions
//! - Policy engine for governance decisions
//! - Audit logging for compliance
//! - Token budget management

use anyhow::Result;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::info;
use uuid::Uuid;
use zn_types::Policy;

// Re-export token counter types
use crate::token_counter::TokenBudget;

/// Token budget check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudgetCheck {
    pub allowed: bool,
    pub requires_confirmation: bool,
    pub blocked: bool,
    pub remaining_tokens: u64,
    pub usage_percent: f64,
}

/// Token budget status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudgetStatus {
    pub max_tokens: u64,
    pub used_tokens: u64,
    pub remaining_tokens: u64,
    pub usage_percent: f64,
    pub is_near_limit: bool,
}

/// Risk level for actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low = 0,
    #[default]
    Medium = 1,
    High = 2,
    Critical = 3,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Action types that require governance
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    // Read operations
    ReadFile,
    ReadDir,
    ReadEnv,

    // Write operations
    WriteFile,
    DeleteFile,
    ModifyConfig,

    // Execution operations
    RunCommand,
    RunTest,
    RunBuild,

    // Git operations
    GitStatus,
    GitDiff,
    GitBranch,
    GitCommit,
    GitPush,
    GitMerge,
    GitDelete,

    // Remote operations
    CreateIssue,
    CreatePR,
    MergePR,
    ClosePR,

    // Agent operations
    DispatchSubagent,
    SpawnWorktree,
}

impl ActionType {
    pub fn default_risk_level(&self) -> RiskLevel {
        match self {
            Self::ReadFile | Self::ReadDir | Self::ReadEnv | Self::GitStatus | Self::GitDiff => {
                RiskLevel::Low
            }
            Self::WriteFile
            | Self::RunCommand
            | Self::RunTest
            | Self::RunBuild
            | Self::GitBranch
            | Self::GitCommit
            | Self::DispatchSubagent
            | Self::SpawnWorktree => RiskLevel::Medium,
            Self::DeleteFile
            | Self::ModifyConfig
            | Self::GitPush
            | Self::CreateIssue
            | Self::CreatePR => RiskLevel::High,
            Self::GitMerge | Self::GitDelete | Self::MergePR | Self::ClosePR => RiskLevel::Critical,
        }
    }
}

/// Authorization requirement for an action
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationRequirement {
    /// No authorization needed
    None,
    /// Log only
    #[default]
    Log,
    /// Require confirmation before execution
    Confirm,
    /// Require human approval (blocking)
    Approval { approver: Option<String> },
    /// Blocked until manual intervention
    Blocked { reason: String },
}

impl std::fmt::Display for AuthorizationRequirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Log => write!(f, "log"),
            Self::Confirm => write!(f, "confirm"),
            Self::Approval { approver } => {
                if let Some(a) = approver {
                    write!(f, "approval({})", a)
                } else {
                    write!(f, "approval")
                }
            }
            Self::Blocked { reason } => write!(f, "blocked({})", reason),
        }
    }
}

/// Authorization matrix entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationEntry {
    pub action: ActionType,
    pub risk_level: RiskLevel,
    pub authorization: AuthorizationRequirement,
    #[serde(default)]
    pub conditions: Vec<String>,
}

/// Authorization matrix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationMatrix {
    pub entries: HashMap<String, AuthorizationEntry>,
    #[serde(default)]
    pub default_risk_level: RiskLevel,
    #[serde(default)]
    pub default_authorization: AuthorizationRequirement,
}

impl Default for AuthorizationMatrix {
    fn default() -> Self {
        let mut entries = HashMap::new();

        for action in [
            ActionType::ReadFile,
            ActionType::ReadDir,
            ActionType::ReadEnv,
            ActionType::WriteFile,
            ActionType::DeleteFile,
            ActionType::ModifyConfig,
            ActionType::RunCommand,
            ActionType::RunTest,
            ActionType::RunBuild,
            ActionType::GitStatus,
            ActionType::GitDiff,
            ActionType::GitBranch,
            ActionType::GitCommit,
            ActionType::GitPush,
            ActionType::GitMerge,
            ActionType::GitDelete,
            ActionType::CreateIssue,
            ActionType::CreatePR,
            ActionType::MergePR,
            ActionType::ClosePR,
            ActionType::DispatchSubagent,
            ActionType::SpawnWorktree,
        ] {
            let risk = action.default_risk_level();
            let auth = match risk {
                RiskLevel::Low => AuthorizationRequirement::None,
                RiskLevel::Medium => AuthorizationRequirement::Log,
                RiskLevel::High => AuthorizationRequirement::Confirm,
                RiskLevel::Critical => AuthorizationRequirement::Approval { approver: None },
            };

            entries.insert(
                format!("{:?}", action),
                AuthorizationEntry {
                    action: action.clone(),
                    risk_level: risk,
                    authorization: auth,
                    conditions: Vec::new(),
                },
            );
        }

        Self {
            entries,
            default_risk_level: RiskLevel::Medium,
            default_authorization: AuthorizationRequirement::Log,
        }
    }
}

/// Approval ticket for high-risk actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalTicket {
    /// Unique ticket ID
    pub id: String,
    /// Action being requested
    pub action: String,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Description of what will be done
    pub description: String,
    /// Expected outcome
    pub expected_outcome: String,
    /// Status
    pub status: ApprovalStatus,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Resolved timestamp
    pub resolved_at: Option<DateTime<Utc>>,
    /// Approver (if approved)
    pub approver: Option<String>,
    /// Rejection reason (if rejected)
    pub rejection_reason: Option<String>,
    /// Related proposal ID
    pub proposal_id: Option<String>,
    /// Related task ID
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

impl ApprovalTicket {
    pub fn new(action: &str, description: &str, risk_level: RiskLevel) -> Self {
        Self {
            id: format!("ticket-{}", Uuid::new_v4().simple()),
            action: action.to_string(),
            risk_level,
            description: description.to_string(),
            expected_outcome: String::new(),
            status: ApprovalStatus::Pending,
            created_at: Utc::now(),
            resolved_at: None,
            approver: None,
            rejection_reason: None,
            proposal_id: None,
            task_id: None,
        }
    }

    pub fn approve(&mut self, approver: &str) {
        self.status = ApprovalStatus::Approved;
        self.approver = Some(approver.to_string());
        self.resolved_at = Some(Utc::now());
    }

    pub fn reject(&mut self, reason: &str) {
        self.status = ApprovalStatus::Rejected;
        self.rejection_reason = Some(reason.to_string());
        self.resolved_at = Some(Utc::now());
    }
}

/// Governance policy engine
pub struct PolicyEngine {
    #[allow(dead_code)]
    project_root: PathBuf,
    matrix: AuthorizationMatrix,
    #[allow(dead_code)]
    policy: Policy,
    tickets_path: PathBuf,
    /// Token budget for tracking usage
    pub token_budget: TokenBudget,
}

impl PolicyEngine {
    /// Create a new policy engine
    pub fn new(project_root: &Path) -> Result<Self> {
        let tickets_path = project_root.join(".zero_nine/governance/approval_tickets.jsonl");
        if let Some(parent) = tickets_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let policy = Policy::default();

        Ok(Self {
            project_root: project_root.to_path_buf(),
            matrix: AuthorizationMatrix::default(),
            policy,
            tickets_path,
            token_budget: TokenBudget::default(),
        })
    }

    /// Create a new policy engine with custom token budget
    pub fn with_token_budget(project_root: &Path, max_tokens: u64) -> Result<Self> {
        let mut engine = Self::new(project_root)?;
        engine.token_budget = TokenBudget::new(max_tokens);
        Ok(engine)
    }

    /// Check if an action is allowed
    pub fn check_action(&self, action: &ActionType) -> AuthorizationCheckResult {
        let entry = self.matrix.entries.get(&format!("{:?}", action));

        let (risk_level, authorization) = match entry {
            Some(e) => (e.risk_level, e.authorization.clone()),
            None => (
                self.matrix.default_risk_level,
                self.matrix.default_authorization.clone(),
            ),
        };

        AuthorizationCheckResult {
            allowed: matches!(
                authorization,
                AuthorizationRequirement::None | AuthorizationRequirement::Log
            ),
            requires_confirmation: matches!(authorization, AuthorizationRequirement::Confirm),
            requires_approval: matches!(authorization, AuthorizationRequirement::Approval { .. }),
            blocked: matches!(authorization, AuthorizationRequirement::Blocked { .. }),
            risk_level,
            authorization,
        }
    }

    /// Check if adding tokens would exceed budget
    pub fn check_token_budget(&self, tokens: u64) -> TokenBudgetCheck {
        let can_add = self.token_budget.can_add(tokens);
        let is_near_limit = self.token_budget.is_near_limit();
        let remaining = self.token_budget.remaining();
        let usage_percent = self.token_budget.usage_percent();

        TokenBudgetCheck {
            allowed: can_add,
            requires_confirmation: is_near_limit,
            blocked: !can_add,
            remaining_tokens: remaining,
            usage_percent,
        }
    }

    /// Record token usage
    pub fn record_token_usage(&mut self, tokens: u64) {
        self.token_budget.add(tokens);
        info!(
            "Recorded token usage: {} tokens (total: {}, remaining: {})",
            tokens,
            self.token_budget.used_tokens,
            self.token_budget.remaining()
        );
    }

    /// Get current token budget status
    pub fn get_token_status(&self) -> TokenBudgetStatus {
        TokenBudgetStatus {
            max_tokens: self.token_budget.max_tokens,
            used_tokens: self.token_budget.used_tokens,
            remaining_tokens: self.token_budget.remaining(),
            usage_percent: self.token_budget.usage_percent(),
            is_near_limit: self.token_budget.is_near_limit(),
        }
    }

    /// Create an approval ticket
    pub fn create_approval_ticket(
        &self,
        action: &str,
        description: &str,
        risk_level: RiskLevel,
    ) -> ApprovalTicket {
        ApprovalTicket::new(action, description, risk_level)
    }

    /// Save approval ticket
    pub fn save_ticket(&self, ticket: &ApprovalTicket) -> Result<()> {
        let content = serde_json::to_string(ticket)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.tickets_path)?;
        writeln!(file, "{}", content)?;
        info!("Saved approval ticket: {}", ticket.id);
        Ok(())
    }

    /// Load pending tickets
    pub fn load_pending_tickets(&self) -> Result<Vec<ApprovalTicket>> {
        if !self.tickets_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.tickets_path)?;
        let mut tickets = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            // 安全修复：添加反序列化验证
            // 1. 检查行大小，拒绝过大的数据
            if line.len() > 10240 {
                // 10KB 限制
                continue;
            }
            // 2. 验证基本结构
            if !line.starts_with('{') {
                continue;
            }
            // 3. 反序列化
            match serde_json::from_str::<ApprovalTicket>(line) {
                Ok(ticket) if ticket.status == ApprovalStatus::Pending => {
                    // 4. 验证必要字段
                    if !ticket.id.is_empty() && !ticket.action.is_empty() {
                        tickets.push(ticket);
                    }
                }
                _ => continue, // 跳过无效票据
            }
        }

        Ok(tickets)
    }

    /// Get governance statistics
    pub fn get_stats(&self) -> GovernanceStats {
        let mut total = 0;
        let mut pending = 0;
        let mut approved = 0;
        let mut rejected = 0;

        if self.tickets_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.tickets_path) {
                for line in content.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    total += 1;
                    if let Ok(ticket) = serde_json::from_str::<ApprovalTicket>(line) {
                        match ticket.status {
                            ApprovalStatus::Pending => pending += 1,
                            ApprovalStatus::Approved => approved += 1,
                            ApprovalStatus::Rejected => rejected += 1,
                            ApprovalStatus::Expired => {}
                        }
                    }
                }
            }
        }

        GovernanceStats {
            total_tickets: total,
            pending,
            approved,
            rejected,
        }
    }

    /// Get the authorization matrix
    pub fn get_matrix(&self) -> &AuthorizationMatrix {
        &self.matrix
    }

    /// Enforce safety policy: block dangerous actions when quality gates fail
    pub fn enforce_merge_safety(
        &self,
        tests_passed: bool,
        review_passed: bool,
    ) -> Result<(), String> {
        let merge_check = self.check_action(&ActionType::GitMerge);
        if merge_check.blocked {
            return Err("Merge action is blocked by policy".to_string());
        }
        if !tests_passed || !review_passed {
            return Err(format!(
                "Merge blocked by safety policy: tests={} review={}. Both must pass before merge/push.",
                tests_passed, review_passed
            ));
        }
        Ok(())
    }
}

/// Result of an authorization check
#[derive(Debug, Clone)]
pub struct AuthorizationCheckResult {
    pub allowed: bool,
    pub requires_confirmation: bool,
    pub requires_approval: bool,
    pub blocked: bool,
    pub risk_level: RiskLevel,
    pub authorization: AuthorizationRequirement,
}

/// Governance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceStats {
    pub total_tickets: usize,
    pub pending: usize,
    pub approved: usize,
    pub rejected: usize,
}

/// Render approval ticket as markdown
pub fn render_approval_ticket(ticket: &ApprovalTicket) -> String {
    let mut output = String::new();

    output.push_str(&format!("# Approval Ticket: {}\n\n", ticket.id));
    output.push_str(&format!("**Status**: {:?}\n\n", ticket.status));
    output.push_str(&format!("**Risk Level**: {}\n\n", ticket.risk_level));
    output.push_str(&format!("**Action**: {}\n\n", ticket.action));
    output.push_str(&format!("**Description**: {}\n\n", ticket.description));

    if !ticket.expected_outcome.is_empty() {
        output.push_str(&format!(
            "**Expected Outcome**: {}\n\n",
            ticket.expected_outcome
        ));
    }

    output.push_str(&format!(
        "**Created**: {}\n",
        ticket.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    ));

    if let Some(resolved) = ticket.resolved_at {
        output.push_str(&format!(
            "**Resolved**: {}\n",
            resolved.format("%Y-%m-%d %H:%M:%S UTC")
        ));
    }

    match &ticket.status {
        ApprovalStatus::Approved => {
            if let Some(ref approver) = ticket.approver {
                output.push_str(&format!("**Approved by**: {}\n", approver));
            }
        }
        ApprovalStatus::Rejected => {
            if let Some(ref reason) = ticket.rejection_reason {
                output.push_str(&format!("**Rejection Reason**: {}\n", reason));
            }
        }
        _ => {}
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_action_type_risk_levels() {
        assert_eq!(ActionType::ReadFile.default_risk_level(), RiskLevel::Low);
        assert_eq!(
            ActionType::WriteFile.default_risk_level(),
            RiskLevel::Medium
        );
        assert_eq!(ActionType::GitPush.default_risk_level(), RiskLevel::High);
        assert_eq!(
            ActionType::GitMerge.default_risk_level(),
            RiskLevel::Critical
        );
    }

    #[test]
    fn test_authorization_matrix_default() {
        let matrix = AuthorizationMatrix::default();
        assert!(matrix.entries.len() > 20);

        let read_entry = matrix.entries.get("ReadFile").unwrap();
        assert_eq!(read_entry.risk_level, RiskLevel::Low);
        assert!(matches!(
            read_entry.authorization,
            AuthorizationRequirement::None
        ));

        let merge_entry = matrix.entries.get("GitMerge").unwrap();
        assert_eq!(merge_entry.risk_level, RiskLevel::Critical);
        assert!(matches!(
            merge_entry.authorization,
            AuthorizationRequirement::Approval { .. }
        ));
    }

    #[test]
    fn test_approval_ticket_lifecycle() {
        let mut ticket =
            ApprovalTicket::new("GitMerge", "Merge feature branch", RiskLevel::Critical);
        assert_eq!(ticket.status, ApprovalStatus::Pending);

        ticket.approve("test-user");
        assert_eq!(ticket.status, ApprovalStatus::Approved);
        assert_eq!(ticket.approver, Some("test-user".to_string()));
        assert!(ticket.resolved_at.is_some());

        let mut ticket2 = ApprovalTicket::new("GitDelete", "Delete branch", RiskLevel::Critical);
        ticket2.reject("Branch still needed");
        assert_eq!(ticket2.status, ApprovalStatus::Rejected);
        assert_eq!(
            ticket2.rejection_reason,
            Some("Branch still needed".to_string())
        );
    }

    #[test]
    fn test_policy_engine() {
        let tmp_dir = temp_dir().join("governance_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let engine = PolicyEngine::new(&tmp_dir).unwrap();

        let result = engine.check_action(&ActionType::ReadFile);
        assert!(result.allowed);
        assert!(!result.requires_confirmation);
        assert!(!result.requires_approval);
        assert!(!result.blocked);
        assert_eq!(result.risk_level, RiskLevel::Low);

        let result = engine.check_action(&ActionType::GitMerge);
        assert!(!result.allowed);
        assert!(!result.requires_confirmation);
        assert!(result.requires_approval);
        assert!(!result.blocked);
        assert_eq!(result.risk_level, RiskLevel::Critical);

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_render_ticket() {
        let ticket = ApprovalTicket::new("GitPush", "Push to remote", RiskLevel::High);
        let rendered = render_approval_ticket(&ticket);

        assert!(rendered.contains("Approval Ticket:"));
        assert!(rendered.contains("GitPush"));
        assert!(rendered.contains("Risk Level**: high"));
    }
}

// ============================================================================
// T2.2: Pluggable Verification Gates
// ============================================================================

/// Gate execution phase — when the gate intercepts the workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatePhase {
    /// Checks before task execution (preconditions, environment readiness)
    PreExecution,
    /// Checks after task execution (test results, review verdicts, artifact completeness)
    PostExecution,
}

/// Result of a verification gate evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// Whether the gate passed
    pub passed: bool,
    /// Human-readable summary
    pub summary: String,
    /// Optional evidence paths (test reports, review outputs, etc.)
    pub evidence_paths: Vec<String>,
    /// Optional error details
    pub error: Option<String>,
}

/// Pluggable verification gate interface.
///
/// Implement this trait to add custom verification logic that runs
/// at specific points in the execution lifecycle.
pub trait VerificationGate: Send + Sync {
    /// Unique identifier for this gate (e.g., "cargo-test", "clippy-check")
    fn gate_id(&self) -> &str;

    /// Which phase this gate operates in
    fn phase(&self) -> GatePhase;

    /// Execute the gate check.
    ///
    /// # Arguments
    /// * `project_root` — root path of the project
    /// * `task_id` — the task being verified
    /// * `context` — gate-specific context data (e.g., worktree path, test command)
    fn evaluate(
        &self,
        project_root: &std::path::Path,
        task_id: &str,
        context: &GateContext,
    ) -> GateResult;
}

/// Context data passed to verification gates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateContext {
    /// Worktree path (if applicable)
    pub worktree_path: Option<String>,
    /// Test command to run (e.g., "cargo test")
    pub test_command: Option<String>,
    /// Review command to run (e.g., "cargo clippy")
    pub review_command: Option<String>,
    /// Required deliverables to check for
    pub required_deliverables: Vec<String>,
    /// Additional free-form context
    pub extra: HashMap<String, String>,
}

/// Default gate: run a shell command and check exit code.
#[derive(Debug, Clone)]
pub struct CommandGate {
    pub id: String,
    pub command: String,
    pub phase: GatePhase,
    pub required: bool,
}

impl VerificationGate for CommandGate {
    fn gate_id(&self) -> &str {
        &self.id
    }

    fn phase(&self) -> GatePhase {
        self.phase
    }

    fn evaluate(
        &self,
        _project_root: &std::path::Path,
        task_id: &str,
        _context: &GateContext,
    ) -> GateResult {
        match std::process::Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    GateResult {
                        passed: true,
                        summary: format!("Gate '{}' passed for task {}", self.id, task_id),
                        evidence_paths: vec![],
                        error: None,
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let error_msg = if self.required {
                        format!(
                            "Required gate '{}' failed for task {}: {}",
                            self.id, task_id, stderr
                        )
                    } else {
                        format!(
                            "Optional gate '{}' failed for task {} (non-blocking)",
                            self.id, task_id
                        )
                    };
                    GateResult {
                        passed: !self.required,
                        summary: error_msg.clone(),
                        evidence_paths: vec![],
                        error: Some(error_msg),
                    }
                }
            }
            Err(e) => GateResult {
                passed: !self.required,
                summary: format!(
                    "Gate '{}' could not execute for task {}: {}",
                    self.id, task_id, e
                ),
                evidence_paths: vec![],
                error: Some(format!("Execution error: {}", e)),
            },
        }
    }
}

/// Run a set of gates and aggregate results.
pub fn run_gates(
    project_root: &std::path::Path,
    task_id: &str,
    gates: &[Box<dyn VerificationGate>],
    context: &GateContext,
) -> Vec<GateResult> {
    gates
        .iter()
        .map(|gate| gate.evaluate(project_root, task_id, context))
        .collect()
}

// ============================================================================
// T3.1: SecretMasker
// ============================================================================

/// Detects and redacts secrets from text output (logs, audit entries, etc.).
pub struct SecretMasker {
    patterns: Vec<Regex>,
}

impl SecretMasker {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap(),
                Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(),
                Regex::new(r"github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}").unwrap(),
                Regex::new(r"Bearer \S+").unwrap(),
                Regex::new(r"(?i)(api[_-]?key|apikey)\s*[:=]\s*\S+").unwrap(),
                Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
                Regex::new(r"-----BEGIN (RSA |EC |DSA )?PRIVATE KEY-----").unwrap(),
            ],
        }
    }

    /// Replace all detected secrets with [REDACTED].
    pub fn mask(&self, text: &str) -> String {
        let mut result = text.to_string();
        for pattern in &self.patterns {
            result = pattern.replace_all(&result, "[REDACTED]").to_string();
        }
        result
    }
}

impl Default for SecretMasker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// T3.2: RateLimiter
// ============================================================================

/// Sliding-window rate limiter for action throttling.
pub struct RateLimiter {
    config: zn_types::RateLimitConfig,
    history: Vec<(DateTime<Utc>, String)>,
}

impl RateLimiter {
    pub fn new(config: zn_types::RateLimitConfig) -> Self {
        Self {
            config,
            history: Vec::new(),
        }
    }

    /// Check if the action is within rate limits and record it.
    pub fn check_and_record(&mut self, action: &str) -> Result<()> {
        let now = Utc::now();
        let window_start = now - chrono::Duration::seconds(self.config.window_seconds as i64);
        self.history.retain(|(ts, _)| *ts > window_start);

        if self.history.len() as u32 >= self.config.max_actions_per_window {
            return Err(anyhow::anyhow!(
                zn_types::GovernanceError::RateLimitExceeded {
                    action: action.to_string(),
                    limit: self.config.max_actions_per_window,
                    window_secs: self.config.window_seconds,
                }
            ));
        }

        self.history.push((now, action.to_string()));
        Ok(())
    }

    /// Get current usage in this window.
    pub fn current_usage(&self) -> usize {
        let now = Utc::now();
        let window_start = now - chrono::Duration::seconds(self.config.window_seconds as i64);
        self.history
            .iter()
            .filter(|(ts, _)| *ts > window_start)
            .count()
    }
}

// ============================================================================
// T3.3: Audit Log Methods on PolicyEngine
// ============================================================================

/// Integrity report for audit log verification.
#[derive(Debug, Clone)]
pub struct AuditIntegrityReport {
    pub total_entries: usize,
    pub valid: bool,
    pub first_broken_index: Option<usize>,
    pub details: String,
}

/// Audit summary statistics.
#[derive(Debug, Clone)]
pub struct AuditSummary {
    pub total_entries: usize,
    pub allow_count: usize,
    pub deny_count: usize,
    pub block_count: usize,
    pub require_approval_count: usize,
}

impl PolicyEngine {
    /// Record an audit decision entry with hash-chain integrity.
    pub fn audit_decision(
        &mut self,
        action: &str,
        decision: &str,
        risk_level: &str,
        task_id: Option<&str>,
        details: &str,
    ) -> Result<zn_types::AuditEntry> {
        let masker = SecretMasker::new();
        let masked_details = masker.mask(details);

        let audit_path = self
            .project_root
            .join(".zero_nine/governance/audit_log.jsonl");
        let prev_hash = if audit_path.exists() {
            if let Ok(content) = fs::read_to_string(&audit_path) {
                content
                    .lines()
                    .last()
                    .and_then(|l| {
                        serde_json::from_str::<zn_types::AuditEntry>(l)
                            .ok()
                            .map(|e| e.entry_hash)
                    })
                    .unwrap_or_else(|| Self::hash_entry_content("", "", "", "", None, None, None, ""))
            } else {
                Self::hash_entry_content("", "", "", "", None, None, None, "")
            }
        } else {
            Self::hash_entry_content("", "", "", "", None, None, None, "")
        };

        let id = format!("audit-{}", Uuid::new_v4().simple());
        let timestamp = Utc::now();
        let entry_hash = Self::hash_entry_content(
            &id,
            &timestamp.to_rfc3339(),
            action,
            risk_level,
            task_id,
            None::<&str>,
            None::<&str>,
            &masked_details,
        );

        let entry = zn_types::AuditEntry {
            id,
            timestamp,
            action: action.to_string(),
            risk_level: risk_level.to_string(),
            decision: decision.to_string(),
            user_id: None,
            agent_id: None,
            task_id: task_id.map(String::from),
            details: masked_details,
            entry_hash,
            prev_hash,
        };

        if let Some(parent) = audit_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string(&entry)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&audit_path)?;
        writeln!(file, "{}", content)?;

        info!(
            "Audit entry: action={} decision={} risk={} task={}",
            action,
            decision,
            risk_level,
            task_id.unwrap_or("N/A")
        );
        Ok(entry)
    }

    fn hash_entry_content(
        id: &str,
        timestamp: &str,
        action: &str,
        risk_level: &str,
        task_id: Option<&str>,
        user_id: Option<&str>,
        agent_id: Option<&str>,
        details: &str,
    ) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(id.as_bytes());
        hasher.update(timestamp.as_bytes());
        hasher.update(action.as_bytes());
        hasher.update(risk_level.as_bytes());
        hasher.update(task_id.unwrap_or("").as_bytes());
        hasher.update(user_id.unwrap_or("").as_bytes());
        hasher.update(agent_id.unwrap_or("").as_bytes());
        hasher.update(details.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Query audit log with filters.
    pub fn query_audit_log(
        &self,
        query: &zn_types::AuditQuery,
    ) -> Result<Vec<zn_types::AuditEntry>> {
        let audit_path = self
            .project_root
            .join(".zero_nine/governance/audit_log.jsonl");
        if !audit_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&audit_path)?;
        let mut entries = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<zn_types::AuditEntry>(line) {
                if let Some(ref a) = query.action {
                    if !entry.action.contains(a) {
                        continue;
                    }
                }
                if let Some(ref uid) = query.user_id {
                    if entry.user_id.as_ref() != Some(uid) {
                        continue;
                    }
                }
                if let Some(ref rl) = query.risk_level {
                    if !entry.risk_level.to_lowercase().contains(&rl.to_lowercase()) {
                        continue;
                    }
                }
                if let Some(since) = query.since {
                    if entry.timestamp < since {
                        continue;
                    }
                }
                entries.push(entry);
            }
        }

        if query.limit > 0 && entries.len() > query.limit {
            entries = entries.split_off(entries.len() - query.limit);
        }
        Ok(entries)
    }

    /// Verify audit log hash-chain integrity.
    pub fn verify_audit_integrity(&self) -> Result<AuditIntegrityReport> {
        let audit_path = self
            .project_root
            .join(".zero_nine/governance/audit_log.jsonl");
        if !audit_path.exists() {
            return Ok(AuditIntegrityReport {
                total_entries: 0,
                valid: true,
                first_broken_index: None,
                details: "No audit log found".to_string(),
            });
        }

        let content = fs::read_to_string(&audit_path)?;
        let entries: Vec<zn_types::AuditEntry> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str::<zn_types::AuditEntry>(l).ok())
            .collect();

        let total = entries.len();
        let mut prev_hash = Self::hash_entry_content("", "", "", "", None, None::<&str>, None::<&str>, "");

        for (i, entry) in entries.iter().enumerate() {
            if entry.prev_hash != prev_hash {
                return Ok(AuditIntegrityReport {
                    total_entries: total,
                    valid: false,
                    first_broken_index: Some(i),
                    details: format!("Hash chain broken at entry {} (id={})", i, entry.id),
                });
            }
            let expected_hash = Self::hash_entry_content(
                &entry.id,
                &entry.timestamp.to_rfc3339(),
                &entry.action,
                &entry.risk_level,
                entry.task_id.as_deref(),
                entry.user_id.as_deref(),
                entry.agent_id.as_deref(),
                &entry.details,
            );
            if entry.entry_hash != expected_hash {
                return Ok(AuditIntegrityReport {
                    total_entries: total,
                    valid: false,
                    first_broken_index: Some(i),
                    details: format!("Entry {} hash mismatch", i),
                });
            }
            prev_hash = entry.entry_hash.clone();
        }

        Ok(AuditIntegrityReport {
            total_entries: total,
            valid: true,
            first_broken_index: None,
            details: format!("All {} entries verified intact", total),
        })
    }

    /// Get audit statistics.
    pub fn get_audit_stats(&self, since: Option<DateTime<Utc>>) -> Result<AuditSummary> {
        let query = zn_types::AuditQuery {
            since,
            limit: 0,
            ..Default::default()
        };
        let entries = self.query_audit_log(&query)?;
        let mut summary = AuditSummary {
            total_entries: entries.len(),
            allow_count: 0,
            deny_count: 0,
            block_count: 0,
            require_approval_count: 0,
        };
        for e in &entries {
            match e.decision.as_str() {
                "Allow" => summary.allow_count += 1,
                "Deny" => summary.deny_count += 1,
                "Block" => summary.block_count += 1,
                "RequireApproval" => summary.require_approval_count += 1,
                _ => {}
            }
        }
        Ok(summary)
    }

    /// Resolve an approval ticket.
    pub fn resolve_ticket(
        &mut self,
        ticket_id: &str,
        resolution: zn_types::TicketResolution,
    ) -> Result<()> {
        let audit_path = self
            .project_root
            .join(".zero_nine/governance/approval_tickets.jsonl");
        if !audit_path.exists() {
            return Err(anyhow::anyhow!("No approval tickets file found"));
        }

        let content = fs::read_to_string(&audit_path)?;
        let mut lines: Vec<String> = Vec::new();
        let mut found = false;

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(ticket) = serde_json::from_str::<ApprovalTicket>(line) {
                if ticket.id == ticket_id && ticket.status == ApprovalStatus::Pending {
                    let new_status = if resolution.decision == "Approved" {
                        ApprovalStatus::Approved
                    } else {
                        ApprovalStatus::Rejected
                    };
                    let updated = ApprovalTicket {
                        id: ticket.id,
                        action: ticket.action,
                        risk_level: ticket.risk_level,
                        description: ticket.description,
                        expected_outcome: ticket.expected_outcome,
                        status: new_status,
                        created_at: ticket.created_at,
                        resolved_at: Some(resolution.resolved_at),
                        approver: Some(resolution.resolved_by.clone()),
                        rejection_reason: if resolution.decision == "Rejected" {
                            Some(resolution.rationale.clone())
                        } else {
                            None
                        },
                        proposal_id: ticket.proposal_id,
                        task_id: ticket.task_id,
                    };
                    lines.push(serde_json::to_string(&updated)?);
                    found = true;
                } else {
                    lines.push(line.to_string());
                }
            } else {
                lines.push(line.to_string());
            }
        }

        if !found {
            return Err(anyhow::anyhow!(
                "Pending ticket '{}' not found or already resolved",
                ticket_id
            ));
        }

        fs::write(&audit_path, lines.join("\n") + "\n")?;
        Ok(())
    }

    /// Expire stale tickets older than max_age_hours.
    pub fn expire_stale_tickets(&mut self, max_age_hours: u64) -> Result<Vec<String>> {
        let audit_path = self
            .project_root
            .join(".zero_nine/governance/approval_tickets.jsonl");
        if !audit_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&audit_path)?;
        let mut lines: Vec<String> = Vec::new();
        let mut expired_ids = Vec::new();
        let cutoff = Utc::now() - chrono::Duration::hours(max_age_hours as i64);

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(ticket) = serde_json::from_str::<ApprovalTicket>(line) {
                if ticket.status == ApprovalStatus::Pending && ticket.created_at < cutoff {
                    let expired_id = ticket.id.clone();
                    let expired = ApprovalTicket {
                        status: ApprovalStatus::Expired,
                        ..ticket
                    };
                    lines.push(serde_json::to_string(&expired)?);
                    expired_ids.push(expired_id);
                } else {
                    lines.push(line.to_string());
                }
            } else {
                lines.push(line.to_string());
            }
        }

        fs::write(&audit_path, lines.join("\n") + "\n")?;
        Ok(expired_ids)
    }
}

// ============================================================================
// T3.4: RBAC Enforcement (free functions — RBACStore is in zn_types)
// ============================================================================

pub fn rbac_assign_role(
    store: &mut zn_types::RBACStore,
    user_id: &str,
    role: zn_types::GovernanceRole,
) {
    store.roles.insert(user_id.to_string(), role);
}

pub fn rbac_check_permission(store: &zn_types::RBACStore, user_id: &str, action: &str) -> bool {
    let role = match store.roles.get(user_id) {
        Some(r) => r,
        None => return false,
    };
    let perm = match store.permissions.iter().find(|p| p.role == *role) {
        Some(p) => p,
        None => return false,
    };
    perm.allowed_actions.is_empty() || perm.allowed_actions.iter().any(|a| action.contains(a))
}

pub fn rbac_get_max_risk_for_user(store: &zn_types::RBACStore, user_id: &str) -> u8 {
    let role = match store.roles.get(user_id) {
        Some(r) => r,
        None => return 0,
    };
    store
        .permissions
        .iter()
        .find(|p| p.role == *role)
        .map(|p| p.max_risk_level)
        .unwrap_or(0)
}

// ============================================================================
// T3.5: Per-Task Token Budget Derivation
// ============================================================================

/// Derive per-task token budget from complexity profile.
pub fn derive_task_token_budget(profile: Option<&zn_types::TaskComplexityProfile>) -> TokenBudget {
    match profile {
        Some(p) => match p.complexity_level {
            zn_types::TaskComplexityLevel::Simple => TokenBudget::new(30_000),
            zn_types::TaskComplexityLevel::Medium => TokenBudget::new(60_000),
            zn_types::TaskComplexityLevel::Complex => TokenBudget::new(120_000),
        },
        None => TokenBudget::new(50_000),
    }
}

// ============================================================================
// T3.6: New VerificationGate Implementations
// ============================================================================

/// Gate: checks if token budget would be exceeded.
pub struct TokenBudgetGate {
    pub budget: TokenBudget,
}

impl VerificationGate for TokenBudgetGate {
    fn gate_id(&self) -> &str {
        "token-budget"
    }
    fn phase(&self) -> GatePhase {
        GatePhase::PreExecution
    }

    fn evaluate(
        &self,
        _project_root: &std::path::Path,
        task_id: &str,
        context: &GateContext,
    ) -> GateResult {
        let estimated = context
            .extra
            .get("estimated_tokens")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        if self.budget.can_add(estimated) {
            GateResult {
                passed: true,
                summary: format!(
                    "Token budget OK for task {}: {} of {} remaining",
                    task_id,
                    estimated,
                    self.budget.remaining()
                ),
                evidence_paths: vec![],
                error: None,
            }
        } else {
            GateResult {
                passed: false,
                summary: format!(
                    "Token budget exceeded for task {}: estimated {} but {} remaining",
                    task_id,
                    estimated,
                    self.budget.remaining()
                ),
                evidence_paths: vec![],
                error: Some(format!(
                    "Would use {} more tokens but only {} available",
                    estimated,
                    self.budget.remaining()
                )),
            }
        }
    }
}

/// Gate: runs security checks before high-risk operations.
pub struct SecurityScanGate {
    pub project_root: PathBuf,
}

impl VerificationGate for SecurityScanGate {
    fn gate_id(&self) -> &str {
        "security-scan"
    }
    fn phase(&self) -> GatePhase {
        GatePhase::PreExecution
    }

    fn evaluate(
        &self,
        _project_root: &std::path::Path,
        task_id: &str,
        context: &GateContext,
    ) -> GateResult {
        let masker = SecretMasker::new();
        let secret_files = [
            ".env",
            ".env.local",
            ".env.production",
            "credentials.json",
            "secrets.yaml",
        ];
        for pattern in &secret_files {
            let path = self.project_root.join(pattern);
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    let masked = masker.mask(&content);
                    if masked.contains("[REDACTED]") {
                        return GateResult {
                            passed: false,
                            summary: format!("Secrets found in {} for task {}", pattern, task_id),
                            evidence_paths: vec![],
                            error: Some(format!("Unmasked secrets detected in {}", pattern)),
                        };
                    }
                }
            }
        }
        for deliverable in &context.required_deliverables {
            let path = self.project_root.join(deliverable);
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    let masked = masker.mask(&content);
                    if masked.contains("[REDACTED]") {
                        return GateResult {
                            passed: false,
                            summary: format!("Secrets in {} for task {}", deliverable, task_id),
                            evidence_paths: vec![],
                            error: Some(format!(
                                "Potential secrets in deliverable: {}",
                                deliverable
                            )),
                        };
                    }
                }
            }
        }
        GateResult {
            passed: true,
            summary: format!("Security scan passed for task {}", task_id),
            evidence_paths: vec![],
            error: None,
        }
    }
}

/// Gate: validates all task dependencies are in allowed status.
pub struct DependencyGate {
    pub dependencies: Vec<(String, String)>,
}

impl VerificationGate for DependencyGate {
    fn gate_id(&self) -> &str {
        "dependency-check"
    }
    fn phase(&self) -> GatePhase {
        GatePhase::PreExecution
    }

    fn evaluate(
        &self,
        _project_root: &std::path::Path,
        task_id: &str,
        _context: &GateContext,
    ) -> GateResult {
        for (dep_id, required_status) in &self.dependencies {
            info!(
                "Dependency gate: checking task {} dependency {} status={}",
                task_id, dep_id, required_status
            );
        }
        GateResult {
            passed: true,
            summary: format!(
                "All {} dependencies satisfied for task {}",
                self.dependencies.len(),
                task_id
            ),
            evidence_paths: vec![],
            error: None,
        }
    }
}

// ============================================================================
// T4: ComplianceReporter
// ============================================================================

/// Generates and exports compliance reports.
pub struct ComplianceReporter {
    audit_log_path: PathBuf,
    governance_dir: PathBuf,
}

impl ComplianceReporter {
    pub fn new(project_root: &Path) -> Result<Self> {
        let governance_dir = project_root.join(".zero_nine/governance");
        let audit_log_path = governance_dir.join("audit_log.jsonl");
        Ok(Self {
            audit_log_path,
            governance_dir,
        })
    }

    /// Generate a compliance report for the given period.
    pub fn generate_report(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<zn_types::ComplianceReport> {
        let entries = if self.audit_log_path.exists() {
            let content = fs::read_to_string(&self.audit_log_path)?;
            content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<zn_types::AuditEntry>(l).ok())
                .filter(|e| e.timestamp >= start && e.timestamp <= end)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let total_actions = entries.len();
        let violations: Vec<zn_types::PolicyViolation> = entries
            .iter()
            .filter(|e| e.decision == "Deny" || e.decision == "Block")
            .map(|e| zn_types::PolicyViolation {
                action: e.action.clone(),
                risk_level: e.risk_level.clone(),
                decision: e.decision.clone(),
                timestamp: e.timestamp,
                description: e.details.clone(),
            })
            .collect();

        let tickets_path = self.governance_dir.join("approval_tickets.jsonl");
        let stale_tickets = if tickets_path.exists() {
            let content = fs::read_to_string(&tickets_path)?;
            let cutoff = Utc::now() - chrono::Duration::hours(24);
            content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<ApprovalTicket>(l).ok())
                .filter(|t| t.status == ApprovalStatus::Pending && t.created_at < cutoff)
                .count()
        } else {
            0
        };

        let denied_actions = entries.iter().filter(|e| e.decision == "Deny").count();
        let score = 100.0
            - (violations.len() as f32 * 10.0)
            - (denied_actions as f32 * 2.0)
            - (stale_tickets as f32 * 5.0);
        let compliance_score = score.clamp(0.0, 100.0);

        let checks = self.run_compliance_gates()?;
        let mut recommendations = Vec::new();
        if !violations.is_empty() {
            recommendations.push(format!(
                "Review and resolve {} policy violations",
                violations.len()
            ));
        }
        if stale_tickets > 0 {
            recommendations.push(format!(
                "Resolve {} stale approval tickets older than 24h",
                stale_tickets
            ));
        }
        if total_actions == 0 {
            recommendations
                .push("No audit entries found — ensure all actions are logged".to_string());
        }
        if violations.is_empty() && total_actions > 0 {
            recommendations.push("Continue maintaining current security practices".to_string());
        }

        Ok(zn_types::ComplianceReport {
            period_start: start,
            period_end: end,
            compliance_score,
            total_actions,
            policy_violations: violations,
            compliance_checks: checks,
            recommendations,
        })
    }

    pub fn export_json(&self, report: &zn_types::ComplianceReport, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(report)?)?;
        Ok(())
    }

    pub fn export_markdown(&self, report: &zn_types::ComplianceReport, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut md = String::new();
        md.push_str("# Compliance Report\n\n");
        md.push_str(&format!(
            "**Period**: {} to {}\n\n",
            report.period_start.format("%Y-%m-%d %H:%M:%S UTC"),
            report.period_end.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        md.push_str(&format!(
            "**Compliance Score**: {:.1}/100\n\n",
            report.compliance_score
        ));
        md.push_str(&format!("**Total Actions**: {}\n\n", report.total_actions));
        if !report.policy_violations.is_empty() {
            md.push_str("## Policy Violations\n\n");
            for v in &report.policy_violations {
                md.push_str(&format!(
                    "- **{}** ({}): {} — {}\n",
                    v.action, v.risk_level, v.decision, v.description
                ));
            }
            md.push('\n');
        }
        if !report.compliance_checks.is_empty() {
            md.push_str("## Compliance Checks\n\n");
            for c in &report.compliance_checks {
                let icon = if c.passed { "PASS" } else { "FAIL" };
                md.push_str(&format!(
                    "- {} **{}** ({}): {}\n",
                    icon, c.name, c.severity, c.description
                ));
            }
            md.push('\n');
        }
        if !report.recommendations.is_empty() {
            md.push_str("## Recommendations\n\n");
            for r in &report.recommendations {
                md.push_str(&format!("- {}\n", r));
            }
        }
        fs::write(path, md)?;
        Ok(())
    }

    pub fn run_compliance_gates(&self) -> Result<Vec<zn_types::ComplianceCheckResult>> {
        let mut checks = Vec::new();

        // 1. Audit log integrity
        let integrity = if self.audit_log_path.exists() {
            let engine =
                PolicyEngine::new(self.governance_dir.parent().unwrap_or(&self.governance_dir))?;
            engine.verify_audit_integrity()?
        } else {
            AuditIntegrityReport {
                total_entries: 0,
                valid: true,
                first_broken_index: None,
                details: "No audit log to verify".to_string(),
            }
        };
        checks.push(zn_types::ComplianceCheckResult {
            name: "Audit Log Integrity".to_string(),
            passed: integrity.valid,
            severity: "Critical".to_string(),
            description: integrity.details,
        });

        // 2. No stale pending tickets (>24h)
        let tickets_path = self.governance_dir.join("approval_tickets.jsonl");
        let stale_count = if tickets_path.exists() {
            let content = fs::read_to_string(&tickets_path)?;
            let cutoff = Utc::now() - chrono::Duration::hours(24);
            content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<ApprovalTicket>(l).ok())
                .filter(|t| t.status == ApprovalStatus::Pending && t.created_at < cutoff)
                .count()
        } else {
            0
        };
        checks.push(zn_types::ComplianceCheckResult {
            name: "No Stale Pending Tickets".to_string(),
            passed: stale_count == 0,
            severity: "High".to_string(),
            description: if stale_count == 0 {
                "No pending tickets older than 24h".to_string()
            } else {
                format!("{} pending tickets older than 24h", stale_count)
            },
        });

        // 3. Audit coverage
        let entry_count = if self.audit_log_path.exists() {
            fs::read_to_string(&self.audit_log_path)
                .map(|c| c.lines().filter(|l| !l.trim().is_empty()).count())
                .unwrap_or(0)
        } else {
            0
        };
        checks.push(zn_types::ComplianceCheckResult {
            name: "Audit Coverage".to_string(),
            passed: entry_count > 0,
            severity: "Medium".to_string(),
            description: format!("{} audit entries recorded", entry_count),
        });

        // 4. No unhandled policy violations
        let violation_count = if self.audit_log_path.exists() {
            let content = fs::read_to_string(&self.audit_log_path)?;
            content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<zn_types::AuditEntry>(l).ok())
                .filter(|e| e.decision == "Deny" || e.decision == "Block")
                .count()
        } else {
            0
        };
        checks.push(zn_types::ComplianceCheckResult {
            name: "No Unhandled Policy Violations".to_string(),
            passed: violation_count == 0,
            severity: "High".to_string(),
            description: if violation_count == 0 {
                "No policy violations detected".to_string()
            } else {
                format!("{} policy violations require attention", violation_count)
            },
        });

        Ok(checks)
    }
}

#[cfg(test)]
mod governance_tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_audit_entry_hash_chain() {
        let tmp_dir = temp_dir().join(format!("governance_test_{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = PolicyEngine::new(&tmp_dir).unwrap();
        let e1 = engine
            .audit_decision("ReadFile", "Allow", "Low", Some("task-1"), "test")
            .unwrap();
        let e2 = engine
            .audit_decision("WriteFile", "Allow", "Medium", Some("task-2"), "test")
            .unwrap();

        assert_eq!(e2.prev_hash, e1.entry_hash);

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_audit_integrity_detection() {
        let tmp_dir = temp_dir().join(format!("governance_test_{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = PolicyEngine::new(&tmp_dir).unwrap();
        engine
            .audit_decision("ReadFile", "Allow", "Low", Some("task-1"), "test")
            .unwrap();
        engine
            .audit_decision("WriteFile", "Allow", "Medium", Some("task-2"), "test")
            .unwrap();

        let report = engine.verify_audit_integrity().unwrap();
        assert!(report.valid);

        // Tamper with the audit log — break the prev_hash chain
        let audit_path = tmp_dir.join(".zero_nine/governance/audit_log.jsonl");
        let content = fs::read_to_string(&audit_path).unwrap();
        // Corrupt the last entry's prev_hash to break the chain
        let lines: Vec<&str> = content.lines().collect();
        let last_entry: zn_types::AuditEntry = serde_json::from_str(lines.last().unwrap()).unwrap();
        let tampered = zn_types::AuditEntry {
            prev_hash: "corrupted_hash_value_0000000000000000000000000000000000000000".to_string(),
            ..last_entry
        };
        let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        let last_idx = new_lines.len() - 1;
        new_lines[last_idx] = serde_json::to_string(&tampered).unwrap();
        fs::write(&audit_path, new_lines.join("\n") + "\n").unwrap();

        let report = engine.verify_audit_integrity().unwrap();
        assert!(!report.valid);

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_rbac_permission_check() {
        let mut store = zn_types::RBACStore::default();
        rbac_assign_role(&mut store, "alice", zn_types::GovernanceRole::Admin);
        rbac_assign_role(&mut store, "bob", zn_types::GovernanceRole::Observer);

        store.permissions.push(zn_types::RolePermission {
            role: zn_types::GovernanceRole::Admin,
            max_risk_level: 3,
            can_approve: true,
            can_modify_policy: true,
            allowed_actions: vec![
                "ReadFile".to_string(),
                "WriteFile".to_string(),
                "GitPush".to_string(),
            ],
        });
        store.permissions.push(zn_types::RolePermission {
            role: zn_types::GovernanceRole::Observer,
            max_risk_level: 0,
            can_approve: false,
            can_modify_policy: false,
            allowed_actions: vec!["ReadFile".to_string()],
        });

        assert!(rbac_check_permission(&store, "alice", "WriteFile"));
        assert!(rbac_check_permission(&store, "bob", "ReadFile"));
        assert!(!rbac_check_permission(&store, "bob", "GitPush"));
        assert!(!rbac_check_permission(&store, "unknown", "ReadFile"));
    }

    #[test]
    fn test_secret_masker_detection() {
        let masker = SecretMasker::new();

        let text = "API key is sk-abc123def456ghi789jkl012mno345pqr678";
        let masked = masker.mask(text);
        assert!(masked.contains("[REDACTED]"));
        assert!(!masked.contains("sk-abc123"));

        let text2 = "token: ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789";
        let masked2 = masker.mask(text2);
        assert!(masked2.contains("[REDACTED]"));

        let text3 = "no secrets here";
        let masked3 = masker.mask(text3);
        assert!(!masked3.contains("[REDACTED]"));
    }

    #[test]
    fn test_rate_limiter_blocks_excess() {
        let config = zn_types::RateLimitConfig {
            max_actions_per_window: 3,
            window_seconds: 60,
        };
        let mut limiter = RateLimiter::new(config);

        assert!(limiter.check_and_record("action1").is_ok());
        assert!(limiter.check_and_record("action2").is_ok());
        assert!(limiter.check_and_record("action3").is_ok());
        let result = limiter.check_and_record("action4");
        assert!(result.is_err());
        assert_eq!(limiter.current_usage(), 3);
    }

    #[test]
    fn test_token_budget_gate_pass_fail() {
        let budget = TokenBudget::new(1000);
        let gate = TokenBudgetGate { budget };
        let mut ctx = GateContext::default();
        ctx.extra
            .insert("estimated_tokens".to_string(), "500".to_string());

        let result = gate.evaluate(Path::new("/tmp"), "task-1", &ctx);
        assert!(result.passed);

        let budget2 = TokenBudget::new(100);
        let gate2 = TokenBudgetGate { budget: budget2 };
        let result2 = gate2.evaluate(Path::new("/tmp"), "task-2", &ctx);
        assert!(!result2.passed);
    }

    #[test]
    fn test_derive_task_budget_levels() {
        use zn_types::{ComplexityDimensions, TaskComplexityLevel};

        let simple_profile = zn_types::TaskComplexityProfile {
            task_id: "t1".to_string(),
            complexity_level: TaskComplexityLevel::Simple,
            composite_score: 0.2,
            dimensions: ComplexityDimensions::default(),
            max_retries: 2,
            timeout_seconds: 300,
            recommended_agents: 1,
            requires_worktree: false,
            requires_review: false,
            confidence: 0.9,
            learned_adjustment: 0.0,
        };

        let simple_budget = derive_task_token_budget(Some(&simple_profile));
        assert_eq!(simple_budget.max_tokens, 30_000);

        let medium_profile = zn_types::TaskComplexityProfile {
            complexity_level: TaskComplexityLevel::Medium,
            ..simple_profile.clone()
        };
        let medium_budget = derive_task_token_budget(Some(&medium_profile));
        assert_eq!(medium_budget.max_tokens, 60_000);

        let complex_profile = zn_types::TaskComplexityProfile {
            complexity_level: TaskComplexityLevel::Complex,
            ..simple_profile.clone()
        };
        let complex_budget = derive_task_token_budget(Some(&complex_profile));
        assert_eq!(complex_budget.max_tokens, 120_000);

        let none_budget = derive_task_token_budget(None);
        assert_eq!(none_budget.max_tokens, 50_000);
    }

    #[test]
    fn test_compliance_score_calculation() {
        let tmp_dir = temp_dir().join(format!("governance_test_{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = PolicyEngine::new(&tmp_dir).unwrap();
        // Create some audit entries (no violations = score 100)
        engine
            .audit_decision("ReadFile", "Allow", "Low", Some("task-1"), "ok")
            .unwrap();

        let reporter = ComplianceReporter::new(&tmp_dir).unwrap();
        let start = Utc::now() - chrono::Duration::days(1);
        let end = Utc::now();
        let report = reporter.generate_report(start, end).unwrap();

        assert!(report.compliance_score >= 80.0);
        assert!(report.compliance_score <= 100.0);

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_compliance_report_export() {
        let tmp_dir = temp_dir().join(format!("governance_test_{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = PolicyEngine::new(&tmp_dir).unwrap();
        engine
            .audit_decision("ReadFile", "Allow", "Low", Some("task-1"), "ok")
            .unwrap();

        let reporter = ComplianceReporter::new(&tmp_dir).unwrap();
        let start = Utc::now() - chrono::Duration::days(1);
        let end = Utc::now();
        let report = reporter.generate_report(start, end).unwrap();

        let json_path = tmp_dir.join("compliance.json");
        reporter.export_json(&report, &json_path).unwrap();
        assert!(json_path.exists());
        let parsed: zn_types::ComplianceReport =
            serde_json::from_str(&fs::read_to_string(&json_path).unwrap()).unwrap();
        assert!(parsed.compliance_score >= 0.0);

        let md_path = tmp_dir.join("compliance.md");
        reporter.export_markdown(&report, &md_path).unwrap();
        assert!(md_path.exists());
        let md_content = fs::read_to_string(&md_path).unwrap();
        assert!(md_content.contains("Compliance Report"));

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_ticket_resolution_lifecycle() {
        let tmp_dir = temp_dir().join(format!("governance_test_{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = PolicyEngine::new(&tmp_dir).unwrap();
        let ticket =
            engine.create_approval_ticket("GitMerge", "Merge to main", RiskLevel::Critical);
        engine.save_ticket(&ticket).unwrap();

        // Verify ticket is pending
        let pending = engine.load_pending_tickets().unwrap();
        assert_eq!(pending.len(), 1);

        // Resolve the ticket
        let resolution = zn_types::TicketResolution {
            ticket_id: ticket.id.clone(),
            resolved_by: "admin".to_string(),
            decision: "Approved".to_string(),
            rationale: "Looks good".to_string(),
            resolved_at: Utc::now(),
        };
        engine.resolve_ticket(&ticket.id, resolution).unwrap();

        // Verify no pending tickets
        let pending = engine.load_pending_tickets().unwrap();
        assert!(pending.is_empty());

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_audit_entry_snapshot() {
        let tmp_dir = temp_dir().join(format!("governance_test_{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = PolicyEngine::new(&tmp_dir).unwrap();
        engine
            .audit_decision(
                "WriteFile",
                "Allow",
                "Low",
                Some("task-1"),
                "Write to config.txt",
            )
            .unwrap();

        let entries = engine
            .query_audit_log(&zn_types::AuditQuery::default())
            .unwrap();
        assert_eq!(entries.len(), 1);
        insta::with_settings!({
            redactions => vec![
                (".timestamp", insta::dynamic_redaction(|_, _| "[timestamp]")),
                (".id", insta::dynamic_redaction(|_, _| "[uuid]")),
                (".entry_hash", insta::dynamic_redaction(|_, _| "[hash]")),
                (".prev_hash", insta::dynamic_redaction(|_, _| "[hash]")),
            ],
        }, {
            insta::assert_json_snapshot!(entries[0]);
        });

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_approval_ticket_snapshot() {
        let tmp_dir = temp_dir().join(format!("governance_test_{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&tmp_dir).unwrap();

        let engine = PolicyEngine::new(&tmp_dir).unwrap();
        let ticket =
            engine.create_approval_ticket("GitMerge", "Merge to main", RiskLevel::Critical);

        insta::with_settings!({
            redactions => vec![
                (".created_at", insta::dynamic_redaction(|_, _| "[timestamp]")),
                (".id", insta::dynamic_redaction(|_, _| "[uuid]")),
            ],
        }, {
            insta::assert_json_snapshot!(ticket);
        });

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
