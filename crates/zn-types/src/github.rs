//! GitHub integration types: issues, labels, PR results, and issue-to-proposal mapping.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::proposal::Proposal;

/// GitHub Issue 标签
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubLabel {
    pub name: String,
}

/// GitHub Issue 指派人
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubAssignee {
    pub login: String,
}

/// GitHub Issue 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubIssue {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    #[serde(default)]
    pub labels: Vec<GitHubLabel>,
    #[serde(default)]
    pub assignees: Vec<GitHubAssignee>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 本地 Issue 结构（.issues/ 目录）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalIssue {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub labels: Vec<String>,
}

/// PR 创建结果
#[derive(Debug, Clone)]
pub struct PrResult {
    pub success: bool,
    pub pr_url: String,
    pub message: String,
}

/// Issue/PR 评论结果
#[derive(Debug, Clone)]
pub struct CommentResult {
    pub success: bool,
    pub message: String,
}

/// 导入 Issue 的结果
#[derive(Debug, Clone)]
pub struct ImportedIssue {
    pub source: String,
    pub issue_id: String,
    pub proposal: Proposal,
    pub import_path: String,
}

/// Issue-to-Proposal 映射记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueMapping {
    pub issue_number: u64,
    pub repo: String,
    pub proposal_id: String,
    pub created_at: DateTime<Utc>,
}
