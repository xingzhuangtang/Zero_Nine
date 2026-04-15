//! GitHub Issue Reader - Import issues as Zero_Nine Proposals
//!
//! This module provides:
//! - Read GitHub Issues via gh CLI
//! - Parse issue template fields into Proposal structure
//! - Support local .issues/ directory as fallback

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::process::Command;
use uuid::Uuid;
use zn_types::{
    Proposal, ProposalStatus, TaskItem, TaskStatus,
    Constraint, ConstraintCategory, AcceptanceCriterion,
    Priority, VerificationMethod, CriterionStatus, Risk,
    RiskProbability, RiskImpact,
};

/// GitHub Issue structure from gh CLI
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubIssue {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub assignees: Vec<Assignee>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Label {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Assignee {
    pub login: String,
}

/// Local issue structure for .issues/ directory
#[derive(Debug, Clone, Deserialize)]
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

/// Result of importing an issue
#[derive(Debug, Clone)]
pub struct ImportedIssue {
    pub source: String,
    pub issue_id: String,
    pub proposal: Proposal,
    pub import_path: String,
}

/// Read GitHub Issues and convert to Proposals
pub fn read_github_issues(
    project_root: &Path,
    repo: Option<&str>,
    issue_numbers: Option<Vec<u64>>,
) -> Result<Vec<ImportedIssue>> {
    let mut imported = Vec::new();

    // Try gh CLI first
    if let Ok(issues) = fetch_github_issues_gh_cli(repo, issue_numbers.as_ref()) {
        for issue in issues {
            let imported_issue = convert_github_issue_to_proposal(project_root, &issue)?;
            imported.push(imported_issue);
        }
    } else {
        // Fallback to local .issues/ directory
        imported = fetch_local_issues(project_root, issue_numbers.as_ref())?;
    }

    Ok(imported)
}

/// Fetch issues using gh CLI
fn fetch_github_issues_gh_cli(
    repo: Option<&str>,
    issue_numbers: Option<&Vec<u64>>,
) -> Result<Vec<GitHubIssue>> {
    let mut cmd = Command::new("gh");
    cmd.arg("issue").arg("list").arg("--json")
        .arg("number,title,body,state,labels,assignees,createdAt,updatedAt")
        .arg("--limit").arg("100");

    if let Some(repo_str) = repo {
        cmd.arg("--repo").arg(repo_str);
    }

    if let Some(numbers) = issue_numbers {
        // For specific issues, fetch individually
        let mut issues = Vec::new();
        for num in numbers {
            let mut single_cmd = Command::new("gh");
            single_cmd.arg("issue").arg("view").arg(num.to_string())
                .arg("--json").arg("number,title,body,state,labels,assignees,createdAt,updatedAt");
            if let Some(repo_str) = repo {
                single_cmd.arg("--repo").arg(repo_str);
            }

            let output = single_cmd.output()?;
            if output.status.success() {
                let issue: GitHubIssue = serde_json::from_slice(&output.stdout)?;
                issues.push(issue);
            }
        }
        return Ok(issues);
    }

    let output = cmd.output()?;
    if !output.status.success() {
        return Err(anyhow!("gh CLI failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let issues: Vec<GitHubIssue> = serde_json::from_slice(&output.stdout)?;
    Ok(issues)
}

/// Fetch issues from local .issues/ directory
fn fetch_local_issues(
    project_root: &Path,
    issue_numbers: Option<&Vec<u64>>,
) -> Result<Vec<ImportedIssue>> {
    let issues_dir = project_root.join(".issues");
    if !issues_dir.exists() {
        return Err(anyhow!("No .issues/ directory found and gh CLI unavailable"));
    }

    let mut imported = Vec::new();
    for entry in fs::read_dir(&issues_dir)? {
        let entry = entry?;
        let path = entry.path();

        // 安全修复：验证路径是否在 issues_dir 内，防止路径遍历
        let canonicalized_path = path.canonicalize().ok();
        let canonicalized_issues_dir = issues_dir.canonicalize().ok();

        if let (Some(real_path), Some(real_issues_dir)) = (canonicalized_path, canonicalized_issues_dir) {
            if !real_path.starts_with(&real_issues_dir) {
                // 跳过不在 issues_dir 内的文件（可能是符号链接攻击）
                continue;
            }
        }

        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        // Check if specific issue numbers were requested
        if let Some(numbers) = issue_numbers {
            let file_num = path.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse::<u64>().ok());
            if let Some(num) = file_num {
                if !numbers.contains(&num) {
                    continue;
                }
            }
        }

        let content = fs::read_to_string(&path)?;
        let local_issue = parse_local_issue_markdown(&content)?;
        let imported_issue = convert_local_issue_to_proposal(project_root, &local_issue, &path)?;
        imported.push(imported_issue);
    }

    Ok(imported)
}

/// Parse local issue markdown format
fn parse_local_issue_markdown(content: &str) -> Result<LocalIssue> {
    let mut title = String::new();
    let mut description = String::new();
    let mut acceptance_criteria = Vec::new();
    let mut constraints = Vec::new();
    let mut labels = Vec::new();

    let mut current_section: Option<&str> = None;
    let mut description_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("# ") {
            title = trimmed.trim_start_matches("# ").trim().to_string();
            current_section = Some("description");
        } else if trimmed.starts_with("## Labels") {
            current_section = Some("labels");
        } else if trimmed.starts_with("## Acceptance Criteria") || trimmed.starts_with("## 验收标准") {
            current_section = Some("acceptance");
        } else if trimmed.starts_with("## Constraints") || trimmed.starts_with("## 约束") {
            current_section = Some("constraints");
        } else if trimmed.starts_with("## ") {
            current_section = Some("description");
        } else if let Some(section) = current_section {
            match section {
                "labels" => {
                    if trimmed.starts_with("- ") {
                        labels.push(trimmed.trim_start_matches("- ").trim().to_string());
                    }
                }
                "acceptance" => {
                    if trimmed.starts_with("- ") {
                        acceptance_criteria.push(trimmed.trim_start_matches("- ").trim().to_string());
                    }
                }
                "constraints" => {
                    if trimmed.starts_with("- ") {
                        constraints.push(trimmed.trim_start_matches("- ").trim().to_string());
                    }
                }
                "description" => {
                    description_lines.push(line);
                }
                _ => {}
            }
        }
    }

    description = description_lines.join("\n").trim().to_string();

    Ok(LocalIssue {
        title,
        description,
        acceptance_criteria,
        constraints,
        labels,
    })
}

/// Convert GitHub Issue to Proposal
fn convert_github_issue_to_proposal(
    project_root: &Path,
    issue: &GitHubIssue,
) -> Result<ImportedIssue> {
    let issue_id = format!("gh-{}", issue.number);
    let uuid_str = format!("{}", Uuid::new_v4().simple());
    let proposal_id = format!("proposal-gh-{}-{}", issue.number, &uuid_str[..8]);

    let mut proposal = Proposal::default();
    proposal.id = proposal_id;
    proposal.title = issue.title.clone();
    proposal.goal = issue.body.clone();
    proposal.status = if issue.state == "open" {
        ProposalStatus::Draft
    } else {
        ProposalStatus::Archived
    };
    proposal.created_at = issue.created_at;
    proposal.updated_at = issue.updated_at;

    // Parse body for structured fields
    parse_issue_body_into_proposal(&issue.body, &mut proposal);

    // Create initial task
    let task = TaskItem {
        id: "task-1".to_string(),
        title: format!("Implement: {}", issue.title),
        description: issue.body.clone(),
        status: TaskStatus::Pending,
        depends_on: vec![],
        kind: Some("execution".to_string()),
        contract: zn_types::TaskContract {
            acceptance_criteria: proposal.acceptance_criteria.iter()
                .map(|ac| ac.description.clone()).collect(),
            deliverables: vec!["Working implementation".to_string()],
            verification_points: vec!["All acceptance criteria met".to_string()],
        },
    };
    proposal.tasks = vec![task];

    // Save proposal
    let proposal_path = save_proposal(project_root, &proposal)?;

    Ok(ImportedIssue {
        source: format!("github:{}", issue.number),
        issue_id,
        proposal,
        import_path: proposal_path,
    })
}

/// Convert local issue to Proposal
fn convert_local_issue_to_proposal(
    project_root: &Path,
    issue: &LocalIssue,
    source_path: &Path,
) -> Result<ImportedIssue> {
    let issue_id = source_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let uuid_str = format!("{}", Uuid::new_v4().simple());
    let proposal_id = format!("proposal-{}-{}", issue_id, &uuid_str[..8]);

    let mut proposal = Proposal::default();
    proposal.id = proposal_id;
    proposal.title = issue.title.clone();
    proposal.goal = issue.description.clone();
    proposal.status = ProposalStatus::Draft;
    proposal.created_at = Utc::now();
    proposal.updated_at = Utc::now();

    // Add acceptance criteria
    for (i, ac_text) in issue.acceptance_criteria.iter().enumerate() {
        proposal.acceptance_criteria.push(AcceptanceCriterion {
            id: format!("ac-{}", i + 1),
            description: ac_text.clone(),
            verification_method: VerificationMethod::AutomatedTest,
            priority: Priority::High,
            status: CriterionStatus::Pending,
        });
    }

    // Add constraints
    for constraint_text in &issue.constraints {
        proposal.constraints.push(Constraint {
            id: format!("constraint-{}", proposal.constraints.len() + 1),
            category: ConstraintCategory::Technical,
            description: constraint_text.clone(),
            rationale: None,
            enforced: true,
        });
    }

    // Add labels as risks (if they indicate complexity)
    for label in &issue.labels {
        if label.to_lowercase().contains("complex") || label.to_lowercase().contains("risk") {
            proposal.risks.push(Risk {
                id: format!("risk-{}", proposal.risks.len() + 1),
                description: format!("Label {}: {}", label, issue.title),
                probability: RiskProbability::Medium,
                impact: RiskImpact::Medium,
                mitigation: None,
                owner: None,
            });
        }
    }

    // Create initial task
    let task = TaskItem {
        id: "task-1".to_string(),
        title: format!("Implement: {}", issue.title),
        description: issue.description.clone(),
        status: TaskStatus::Pending,
        depends_on: vec![],
        kind: Some("execution".to_string()),
        contract: zn_types::TaskContract {
            acceptance_criteria: issue.acceptance_criteria.clone(),
            deliverables: vec!["Working implementation".to_string()],
            verification_points: vec!["All acceptance criteria met".to_string()],
        },
    };
    proposal.tasks = vec![task];

    // Save proposal
    let proposal_path = save_proposal(project_root, &proposal)?;

    Ok(ImportedIssue {
        source: format!("local:{}", issue_id),
        issue_id,
        proposal,
        import_path: proposal_path,
    })
}

/// Parse issue body for structured fields (simple markdown parsing)
fn parse_issue_body_into_proposal(body: &str, proposal: &mut Proposal) {
    // Simple parsing: look for common section headers
    let lines: Vec<&str> = body.lines().collect();
    let mut current_section: Option<&str> = None;

    for line in lines {
        let trimmed = line.trim();

        if trimmed.starts_with("## Acceptance Criteria") || trimmed.starts_with("## 验收标准") {
            current_section = Some("acceptance");
        } else if trimmed.starts_with("## Constraints") || trimmed.starts_with("## 约束") {
            current_section = Some("constraints");
        } else if trimmed.starts_with("## Risks") || trimmed.starts_with("## 风险") {
            current_section = Some("risks");
        } else if trimmed.starts_with("## ") {
            current_section = None;
        } else if let Some(section) = current_section {
            match section {
                "acceptance" => {
                    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                        let text = trimmed.trim_start_matches("- ").trim_start_matches("* ").trim();
                        if !text.is_empty() {
                            proposal.acceptance_criteria.push(AcceptanceCriterion {
                                id: format!("ac-{}", proposal.acceptance_criteria.len() + 1),
                                description: text.to_string(),
                                verification_method: VerificationMethod::AutomatedTest,
                                priority: Priority::Medium,
                                status: CriterionStatus::Pending,
                            });
                        }
                    }
                }
                "constraints" => {
                    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                        let text = trimmed.trim_start_matches("- ").trim_start_matches("* ").trim();
                        if !text.is_empty() {
                            proposal.constraints.push(Constraint {
                                id: format!("constraint-{}", proposal.constraints.len() + 1),
                                category: ConstraintCategory::Technical,
                                description: text.to_string(),
                                rationale: None,
                                enforced: true,
                            });
                        }
                    }
                }
                "risks" => {
                    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                        let text = trimmed.trim_start_matches("- ").trim_start_matches("* ").trim();
                        if !text.is_empty() {
                            proposal.risks.push(Risk {
                                id: format!("risk-{}", proposal.risks.len() + 1),
                                description: text.to_string(),
                                probability: RiskProbability::Medium,
                                impact: RiskImpact::Medium,
                                mitigation: None,
                                owner: None,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Save proposal to disk
fn save_proposal(project_root: &Path, proposal: &Proposal) -> Result<String> {
    use zn_spec::save_proposal as spec_save_proposal;

    spec_save_proposal(project_root, proposal)?;

    let proposal_dir = project_root.join(".zero_nine/proposals").join(&proposal.id);
    Ok(proposal_dir.display().to_string())
}

/// Create a pull request via gh CLI
pub fn create_pull_request(
    repo: Option<&str>,
    branch: &str,
    title: &str,
    body: &str,
    base: Option<&str>,
) -> Result<PrResult> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr").arg("create")
        .arg("--title").arg(title)
        .arg("--body").arg(body)
        .arg("--head").arg(branch);

    if let Some(base_branch) = base {
        cmd.arg("--base").arg(base_branch);
    }

    if let Some(repo_str) = repo {
        cmd.arg("--repo").arg(repo_str);
    }

    let output = cmd.output()?;
    if !output.status.success() {
        return Err(anyhow!("gh pr create failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let pr_url = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Ok(PrResult {
        success: true,
        pr_url,
        message: "PR created successfully".to_string(),
    })
}

/// Write comment to GitHub issue/PR
pub fn write_issue_comment(
    repo: Option<&str>,
    issue_number: u64,
    comment: &str,
) -> Result<CommentResult> {
    let mut cmd = Command::new("gh");
    cmd.arg("issue").arg("comment").arg(issue_number.to_string())
        .arg("--body").arg(comment);

    if let Some(repo_str) = repo {
        cmd.arg("--repo").arg(repo_str);
    }

    let output = cmd.output()?;
    if !output.status.success() {
        return Err(anyhow!("gh issue comment failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    Ok(CommentResult {
        success: true,
        message: "Comment posted successfully".to_string(),
    })
}

/// PR creation result
#[derive(Debug, Clone)]
pub struct PrResult {
    pub success: bool,
    pub pr_url: String,
    pub message: String,
}

/// Comment result
#[derive(Debug, Clone)]
pub struct CommentResult {
    pub success: bool,
    pub message: String,
}

/// Write execution summary back to GitHub
pub fn write_execution_summary(
    repo: Option<&str>,
    issue_number: u64,
    proposal: &Proposal,
    summary: &str,
) -> Result<CommentResult> {
    let mut comment = String::new();
    comment.push_str("## Zero_Nine Execution Summary\n\n");
    comment.push_str(&format!("**Proposal**: {}\n\n", proposal.id));
    comment.push_str(&format!("**Status**: {:?}\n\n", proposal.status));

    comment.push_str("### Tasks\n\n");
    for task in &proposal.tasks {
        comment.push_str(&format!("- [{}] **{}**: {:?}\n",
            match task.status {
                TaskStatus::Completed => "x",
                TaskStatus::Failed => "!",
                TaskStatus::Blocked => "-",
                _ => " ",
            },
            task.title,
            task.status
        ));
    }

    comment.push_str("\n### Summary\n\n");
    comment.push_str(summary);

    write_issue_comment(repo, issue_number, &comment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_parse_local_issue_markdown() {
        let content = r#"# Implement Feature X

This is the description of feature X.

## Labels
- feature
- high-priority

## Acceptance Criteria
- Must handle 1000 requests/sec
- Must have 99.9% uptime

## Constraints
- Must use existing database schema
- Must be backwards compatible
"#;
        let issue = parse_local_issue_markdown(content).unwrap();
        assert_eq!(issue.title, "Implement Feature X");
        assert_eq!(issue.acceptance_criteria.len(), 2);
        assert_eq!(issue.constraints.len(), 2);
        assert_eq!(issue.labels.len(), 2);
    }

    #[test]
    fn test_convert_local_issue_to_proposal() {
        let tmp_dir = temp_dir().join("issue_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let issue = LocalIssue {
            title: "Test Issue".to_string(),
            description: "Test description".to_string(),
            acceptance_criteria: vec!["Criterion 1".to_string()],
            constraints: vec!["Constraint 1".to_string()],
            labels: vec!["feature".to_string()],
        };

        let source_path = tmp_dir.join("1.md");
        let result = convert_local_issue_to_proposal(&tmp_dir, &issue, &source_path);

        assert!(result.is_ok());
        let imported = result.unwrap();
        assert_eq!(imported.proposal.title, "Test Issue");
        assert_eq!(imported.proposal.acceptance_criteria.len(), 1);

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
