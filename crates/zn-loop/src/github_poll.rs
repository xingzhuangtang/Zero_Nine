//! GitHub Issue Polling and CI Failure Scanning — auto-discover work from external sources.
//!
//! Scans for new/open GitHub issues and recent CI failures, converting them into
//! `CronJob` entries that the cron scheduler can pick up on subsequent loop iterations.

use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::path::Path;
use std::process::Command;
use tracing::{info, warn};

use crate::cron_scheduler::{create_recurring_job, CronScheduler};

/// GitHub issue fetched via gh CLI.
#[derive(Debug, Clone, Deserialize)]
struct GitHubIssue {
    number: u64,
    title: String,
    #[serde(default)]
    body: Option<String>,
}

/// CI workflow run from gh CLI.
#[derive(Debug, Clone, Deserialize)]
struct CIBuildRun {
    database_id: u64,
    name: String,
    conclusion: String,
    head_branch: String,
}

/// Poll GitHub for open issues and register them as cron jobs.
///
/// Returns the number of new issues discovered.
pub fn poll_github_issues(project_root: &Path, repo: Option<&str>) -> Result<usize> {
    let manifest = zn_spec::load_manifest(project_root)?.unwrap_or_default();
    let effective_repo = repo.or(manifest.github_repo.as_deref());

    let Some(repo_str) = effective_repo else {
        info!("No GitHub repo configured, skipping issue poll");
        return Ok(0);
    };

    let issues = fetch_open_issues(repo_str)?;
    if issues.is_empty() {
        info!("No open GitHub issues found in {}", repo_str);
        return Ok(0);
    }

    let mut scheduler = CronScheduler::new(project_root)?;
    let mut new_count = 0;

    for issue in &issues {
        let job_id = format!("gh-issue-{}", issue.number);

        // Skip if already registered
        if scheduler.get_job(&job_id).is_some() {
            continue;
        }

        // Skip if already has a proposal for this issue
        if has_proposal_for_issue(project_root, issue.number) {
            continue;
        }

        let body_preview = issue
            .body
            .as_ref()
            .map(|b| b.chars().take(200).collect::<String>())
            .unwrap_or_default();

        let payload = json!({
            "goal": format!("{}: {}", issue.title, body_preview),
            "source": "github",
            "issue_number": issue.number,
            "repo": repo_str,
        });

        let job = create_recurring_job(
            &job_id,
            "0 */6 * * *", // check every 6 hours
            &format!("GitHub issue #{}: {}", issue.number, issue.title),
            payload,
            Some(30),
        );

        scheduler.schedule(job)?;
        new_count += 1;
        info!(
            "Discovered GitHub issue #{} as cron job: {}",
            issue.number, issue.title
        );
    }

    scheduler.save_state()?;
    Ok(new_count)
}

/// Scan for recent CI failures and register remediation jobs.
///
/// Returns the number of new CI failure jobs registered.
pub fn scan_ci_failures(project_root: &Path, repo: Option<&str>) -> Result<usize> {
    let manifest = zn_spec::load_manifest(project_root)?.unwrap_or_default();
    let effective_repo = repo.or(manifest.github_repo.as_deref());

    let Some(repo_str) = effective_repo else {
        info!("No GitHub repo configured, skipping CI scan");
        return Ok(0);
    };

    let runs = fetch_failed_runs(repo_str)?;
    if runs.is_empty() {
        info!("No recent CI failures in {}", repo_str);
        return Ok(0);
    }

    let mut scheduler = CronScheduler::new(project_root)?;
    let mut new_count = 0;

    for run in &runs {
        let job_id = format!("ci-failure-{}", run.database_id);

        // Skip if already registered
        if scheduler.get_job(&job_id).is_some() {
            continue;
        }

        let payload = json!({
            "goal": format!(
                "Investigate and fix CI failure in run #{} ({}) on branch {}",
                run.database_id, run.name, run.head_branch
            ),
            "source": "ci_failure",
            "run_id": run.database_id,
            "repo": repo_str,
        });

        let job = create_recurring_job(
            &job_id,
            "0 */4 * * *",
            &format!("CI failure: run #{} ({})", run.database_id, run.name),
            payload,
            Some(7),
        );

        scheduler.schedule(job)?;
        new_count += 1;
        info!(
            "Registered CI failure remediation for run #{} ({})",
            run.database_id, run.name
        );
    }

    scheduler.save_state()?;
    Ok(new_count)
}

// ==================== Internal helpers ====================

fn fetch_open_issues(repo: &str) -> Result<Vec<GitHubIssue>> {
    let output = Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--limit",
            "50",
            "--json",
            "number,title,body",
        ])
        .output()?;

    if !output.status.success() {
        warn!(
            "gh issue list failed, skipping issue poll: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(Vec::new());
    }

    let raw: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    Ok(raw
        .into_iter()
        .filter_map(|v| {
            Some(GitHubIssue {
                number: v["number"].as_u64()?,
                title: v["title"].as_str()?.to_string(),
                body: v["body"].as_str().map(String::from),
            })
        })
        .collect())
}

fn fetch_failed_runs(repo: &str) -> Result<Vec<CIBuildRun>> {
    let output = Command::new("gh")
        .args([
            "run",
            "list",
            "--repo",
            repo,
            "--limit",
            "10",
            "--json",
            "databaseId,name,conclusion,headBranch",
        ])
        .output()?;

    if !output.status.success() {
        warn!(
            "gh run list failed, skipping CI scan: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(Vec::new());
    }

    let raw: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    Ok(raw
        .into_iter()
        .filter_map(|v| {
            Some(CIBuildRun {
                database_id: v["databaseId"].as_u64()?,
                name: v["name"].as_str()?.to_string(),
                conclusion: v["conclusion"].as_str()?.to_string(),
                head_branch: v["headBranch"].as_str()?.to_string(),
            })
        })
        .filter(|r| r.conclusion == "failure")
        .collect())
}

fn has_proposal_for_issue(project_root: &Path, issue_number: u64) -> bool {
    // Check .zero_nine/proposals/ for any proposal referencing this issue
    let proposals_dir = project_root.join(".zero_nine/proposals");
    if !proposals_dir.exists() {
        return false;
    }

    if let Ok(entries) = std::fs::read_dir(&proposals_dir) {
        for entry in entries.flatten() {
            let path = entry.path().join("proposal.json");
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if val.get("source_issue_number").and_then(|n| n.as_u64())
                            == Some(issue_number)
                        {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_poll_github_issues_no_repo_returns_zero() {
        let tmp_dir = temp_dir().join(format!("zn_poll_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        // No manifest, so no repo configured
        let result = poll_github_issues(&tmp_dir, None).unwrap();
        assert_eq!(result, 0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_has_proposal_for_issue_empty_dir_returns_false() {
        let tmp_dir = temp_dir().join(format!("zn_proposal_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        assert!(!has_proposal_for_issue(&tmp_dir, 42));

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_has_proposal_for_issue_matching_proposal() {
        let tmp_dir = temp_dir().join(format!("zn_proposal_match_{}", uuid::Uuid::new_v4()));
        let proposals_dir = tmp_dir.join(".zero_nine/proposals/test-001");
        std::fs::create_dir_all(&proposals_dir).unwrap();

        let proposal = json!({
            "id": "test-001",
            "title": "Test Proposal",
            "source_issue_number": 42
        });
        std::fs::write(
            proposals_dir.join("proposal.json"),
            serde_json::to_string_pretty(&proposal).unwrap(),
        )
        .unwrap();

        assert!(has_proposal_for_issue(&tmp_dir, 42));
        assert!(!has_proposal_for_issue(&tmp_dir, 99));

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
