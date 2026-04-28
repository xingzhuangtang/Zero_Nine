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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low = 0,
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

impl Default for RiskLevel {
    fn default() -> Self {
        Self::Medium
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationRequirement {
    /// No authorization needed
    None,
    /// Log only
    Log,
    /// Require confirmation before execution
    Confirm,
    /// Require human approval (blocking)
    Approval { approver: Option<String> },
    /// Blocked until manual intervention
    Blocked { reason: String },
}

impl Default for AuthorizationRequirement {
    fn default() -> Self {
        Self::Log
    }
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
    project_root: PathBuf,
    matrix: AuthorizationMatrix,
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
            Some(e) => (e.risk_level.clone(), e.authorization.clone()),
            None => (
                self.matrix.default_risk_level.clone(),
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
        let ticket = ApprovalTicket::new(action, description, risk_level);
        ticket
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
