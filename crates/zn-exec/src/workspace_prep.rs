//! Workspace Preparation — git worktree, sandbox, and in-place strategies
//!
//! Extracted from zn-exec/lib.rs (T3.3 architecture refactor)

use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use zn_types::{
    ExecutionPlan, WorkspacePreparationResult, WorkspaceRecord, WorkspaceStatus, WorkspaceStrategy,
    WorktreePlan,
};

/// Prepare workspace according to the execution plan's strategy.
pub fn prepare_workspace(
    project_root: &Path,
    plan: &ExecutionPlan,
) -> Result<WorkspacePreparationResult> {
    match plan.workspace_strategy {
        WorkspaceStrategy::InPlace => prepare_in_place(project_root, plan),
        WorkspaceStrategy::GitWorktree => prepare_git_worktree(project_root, plan),
        WorkspaceStrategy::Sandboxed => prepare_sandbox(project_root, plan),
    }
}

fn prepare_in_place(
    project_root: &Path,
    plan: &ExecutionPlan,
) -> Result<WorkspacePreparationResult> {
    let now = Utc::now();
    let record = WorkspaceRecord {
        strategy: WorkspaceStrategy::InPlace,
        status: WorkspaceStatus::Active,
        branch_name: git_current_branch(project_root).unwrap_or_else(|_| "in-place".to_string()),
        worktree_path: project_root.display().to_string(),
        base_branch: git_current_branch(project_root).ok(),
        head_branch: git_current_branch(project_root).ok(),
        created_at: now,
        updated_at: now,
        notes: vec![
            "Task runs in the project root without creating a separate worktree.".to_string(),
        ],
    };
    let mut created_paths = vec![project_root.display().to_string()];
    created_paths.extend(persist_workspace_preparation_artifacts(
        project_root,
        plan,
        &record,
    )?);
    Ok(WorkspacePreparationResult {
        success: true,
        summary: "Workspace strategy is in-place; no new worktree was created.".to_string(),
        record: Some(record),
        created_paths,
    })
}

fn prepare_git_worktree(
    project_root: &Path,
    plan: &ExecutionPlan,
) -> Result<WorkspacePreparationResult> {
    let repo_root = git_toplevel(project_root)?;
    let worktree = plan
        .worktree_plan
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("ExecutionPlan missing WorktreePlan"))?;

    if !git_has_head(&repo_root)? {
        return fallback_to_in_place(project_root, plan, "Repository has no initial commit yet");
    }

    let abs_path = normalize_worktree_path(&repo_root, &worktree.worktree_path);
    if let Some(parent) = abs_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create worktree parent directory {}",
                parent.display()
            )
        })?;
    }

    // Reuse existing worktree if present
    if abs_path.exists() && (abs_path.join(".git").exists() || abs_path.join(".git").is_file()) {
        let now = Utc::now();
        let record = WorkspaceRecord {
            strategy: WorkspaceStrategy::GitWorktree,
            status: WorkspaceStatus::Active,
            branch_name: worktree.branch_name.clone(),
            worktree_path: abs_path.display().to_string(),
            base_branch: git_current_branch(&repo_root).ok(),
            head_branch: Some(worktree.branch_name.clone()),
            created_at: now,
            updated_at: now,
            notes: vec![
                "Reused an existing git worktree because the target path already exists."
                    .to_string(),
            ],
        };
        let mut created_paths = vec![abs_path.display().to_string()];
        created_paths.extend(persist_workspace_preparation_artifacts(
            project_root,
            plan,
            &record,
        )?);
        return Ok(WorkspacePreparationResult {
            success: true,
            summary: format!("Reused existing git worktree at {}", abs_path.display()),
            record: Some(record),
            created_paths,
        });
    }

    let branch_exists = git_branch_exists(&repo_root, &worktree.branch_name)?;
    let mut command = Command::new("git");
    command.arg("-C").arg(&repo_root).arg("worktree").arg("add");
    if !branch_exists {
        command.arg("-b").arg(&worktree.branch_name);
    }
    command.arg(&abs_path);
    if branch_exists {
        command.arg(&worktree.branch_name);
    } else {
        command.arg("HEAD");
    }
    run_command(&mut command, "failed to create git worktree")?;

    let now = Utc::now();
    let record = WorkspaceRecord {
        strategy: WorkspaceStrategy::GitWorktree,
        status: WorkspaceStatus::Prepared,
        branch_name: worktree.branch_name.clone(),
        worktree_path: abs_path.display().to_string(),
        base_branch: git_current_branch(&repo_root).ok(),
        head_branch: Some(worktree.branch_name.clone()),
        created_at: now,
        updated_at: now,
        notes: vec![format!("Prepared git worktree for task {}.", plan.task_id)],
    };
    let mut created_paths = vec![abs_path.display().to_string()];
    created_paths.extend(persist_workspace_preparation_artifacts(
        project_root,
        plan,
        &record,
    )?);
    Ok(WorkspacePreparationResult {
        success: true,
        summary: format!(
            "Prepared git worktree {} on branch {}",
            abs_path.display(),
            worktree.branch_name
        ),
        record: Some(record),
        created_paths,
    })
}

fn prepare_sandbox(
    project_root: &Path,
    plan: &ExecutionPlan,
) -> Result<WorkspacePreparationResult> {
    let sandbox_root = project_root
        .join(".zero_nine/sandboxes")
        .join(&plan.task_id);
    fs::create_dir_all(&sandbox_root).with_context(|| {
        format!(
            "failed to create sandbox directory {}",
            sandbox_root.display()
        )
    })?;

    // Write .gitignore to isolate sandbox from git tracking
    let gitignore = sandbox_root.join(".gitignore");
    fs::write(&gitignore, "*\n!.gitignore\n")
        .with_context(|| format!("failed to write {}", gitignore.display()))?;

    // Write sandbox metadata description
    let readme = sandbox_root.join("SANDBOX.md");
    let now = Utc::now();
    fs::write(
        &readme,
        format!(
            "# Sandbox for Task: {}\n\n\
             - **Objective**: {}\n\
             - **Created**: {}\n\
             - **Strategy**: Sandboxed (isolated directory)\n\
             - **Status**: Prepared\n\n\
             All files in this directory are isolated from the main repository\n\
             and will be cleaned up after task execution.\n",
            plan.task_id,
            plan.objective,
            now.to_rfc3339()
        ),
    )
    .with_context(|| format!("failed to write {}", readme.display()))?;

    let record = WorkspaceRecord {
        strategy: WorkspaceStrategy::Sandboxed,
        status: WorkspaceStatus::Prepared,
        branch_name: format!("sandbox-{}", plan.task_id),
        worktree_path: sandbox_root.display().to_string(),
        base_branch: None,
        head_branch: None,
        created_at: now,
        updated_at: now,
        notes: vec!["Prepared sandbox with .gitignore isolation and metadata.".to_string()],
    };
    let mut created_paths = vec![
        sandbox_root.display().to_string(),
        gitignore.display().to_string(),
        readme.display().to_string(),
    ];
    created_paths.extend(persist_workspace_preparation_artifacts(
        project_root,
        plan,
        &record,
    )?);
    Ok(WorkspacePreparationResult {
        success: true,
        summary: format!("Prepared sandbox at {}", sandbox_root.display()),
        record: Some(record),
        created_paths,
    })
}

fn fallback_to_in_place(
    project_root: &Path,
    plan: &ExecutionPlan,
    reason: &str,
) -> Result<WorkspacePreparationResult> {
    let now = Utc::now();
    let record = WorkspaceRecord {
        strategy: WorkspaceStrategy::InPlace,
        status: WorkspaceStatus::Active,
        branch_name: git_current_branch(project_root)
            .unwrap_or_else(|_| "pre-initial-commit".to_string()),
        worktree_path: project_root.display().to_string(),
        base_branch: None,
        head_branch: None,
        created_at: now,
        updated_at: now,
        notes: vec![format!(
            "Fell back to in-place execution because git worktree requires an existing HEAD commit, but {}.",
            reason
        )],
    };
    let mut created_paths = vec![project_root.display().to_string()];
    created_paths.extend(persist_workspace_preparation_artifacts(
        project_root,
        plan,
        &record,
    )?);
    Ok(WorkspacePreparationResult {
        success: true,
        summary: format!(
            "Zero_Nine skipped git worktree creation and continued in-place for this task: {}.",
            reason
        ),
        record: Some(record),
        created_paths,
    })
}

fn persist_workspace_preparation_artifacts(
    project_root: &Path,
    plan: &ExecutionPlan,
    record: &WorkspaceRecord,
) -> Result<Vec<String>> {
    let artifact_dir = project_root
        .join(".zero_nine/workspace")
        .join(format!("task-{}", plan.task_id));
    fs::create_dir_all(&artifact_dir)?;

    let json_path = artifact_dir.join("workspace-record.json");
    let markdown_path = artifact_dir.join("workspace-record.md");

    fs::write(&json_path, serde_json::to_vec_pretty(record)?)?;
    fs::write(
        &markdown_path,
        render_workspace_record_markdown(plan, record),
    )?;

    Ok(vec![
        json_path.display().to_string(),
        markdown_path.display().to_string(),
    ])
}

fn render_workspace_record_markdown(plan: &ExecutionPlan, record: &WorkspaceRecord) -> String {
    let base_branch = record.base_branch.as_deref().unwrap_or("none (new branch)");
    format!(
        "# Workspace Record — Task {task_id}\n\n\
         **Strategy**: {strategy:?}\n\
         **Status**: {status:?}\n\
         **Branch**: {branch}\n\
         **Base**: {base}\n\
         **Path**: {path}\n\
         **Created**: {created}\n\n\
         ## Notes\n\
         {notes}\n",
        task_id = plan.task_id,
        strategy = record.strategy,
        status = record.status,
        branch = record.branch_name,
        base = base_branch,
        path = record.worktree_path,
        created = record.created_at.to_rfc3339(),
        notes = record.notes.join("\n")
    )
}

// --- Git helpers ---

fn run_command(command: &mut Command, context: &str) -> Result<String> {
    let output = command
        .output()
        .with_context(|| format!("{}: failed to run command", context))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("{}: {}", context, stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_toplevel(project_root: &Path) -> Result<PathBuf> {
    let output = run_command(
        Command::new("git")
            .arg("-C")
            .arg(project_root)
            .arg("rev-parse")
            .arg("--show-toplevel"),
        "failed to find git toplevel",
    )?;
    Ok(PathBuf::from(output.trim()))
}

fn git_current_branch(project_root: &Path) -> Result<String> {
    let output = run_command(
        Command::new("git")
            .arg("-C")
            .arg(project_root)
            .arg("branch")
            .arg("--show-current"),
        "failed to get current branch",
    )?;
    Ok(output.trim().to_string())
}

fn git_has_head(repo_root: &Path) -> Result<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("--verify")
        .arg("HEAD")
        .output();
    Ok(output.map(|o| o.status.success()).unwrap_or(false))
}

fn git_branch_exists(repo_root: &Path, branch_name: &str) -> Result<bool> {
    let output = run_command(
        Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .arg("branch")
            .arg("--list")
            .arg(branch_name),
        "failed to list git branches",
    )?;
    Ok(!output.trim().is_empty())
}

fn git_is_clean(repo_root: &Path) -> Result<bool> {
    let output = run_command(
        Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .arg("status")
            .arg("--porcelain"),
        "failed to check git status",
    )?;
    Ok(output.trim().is_empty())
}

fn normalize_worktree_path(repo_root: &Path, worktree_path: &str) -> PathBuf {
    let path = PathBuf::from(worktree_path);
    if path.is_absolute() {
        path
    } else {
        repo_root.join(&path)
    }
}
