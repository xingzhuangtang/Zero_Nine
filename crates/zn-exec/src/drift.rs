use std::path::Path;
use std::process::Command;
use zn_types::{
    ActualProjectState, DesiredProjectState, DriftCheckResult, DriftReport, DriftSeverity,
    ExecutionPlan, Proposal, StateDiff, WorkspaceStrategy,
};

/// Capture the actual current state of the project workspace
pub fn capture_actual_state(project_root: &Path) -> ActualProjectState {
    let current_branch = run_git(project_root, &["branch", "--show-current"])
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string());

    let worktree_clean = run_git(project_root, &["status", "--porcelain"])
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);

    let present_files = list_project_files(project_root);

    let available_toolchains = detect_toolchains();

    let detected_test_command = detect_test_command(project_root);

    let remote_capabilities = detect_remote_capabilities(project_root);

    ActualProjectState {
        schema_version: zn_types::default_spec_schema_version(),
        proposal_id: None,
        current_branch,
        worktree_clean,
        present_files,
        available_toolchains,
        detected_test_command,
        remote_capabilities,
        notes: Vec::new(),
    }
}

/// Build the desired state from proposal and execution plan
pub fn build_desired_state(proposal: &Proposal, plan: &ExecutionPlan) -> DesiredProjectState {
    let expected_branch = plan.worktree_plan.as_ref().map(|wt| wt.branch_name.clone());

    let required_files = plan.deliverables.clone();

    let require_clean_worktree = matches!(
        plan.workspace_strategy,
        WorkspaceStrategy::GitWorktree | WorkspaceStrategy::Sandboxed
    );

    let required_toolchains = if cfg!(target_os = "macos") {
        vec!["cargo".to_string(), "rustc".to_string()]
    } else {
        Vec::new()
    };

    DesiredProjectState {
        schema_version: zn_types::default_spec_schema_version(),
        proposal_id: Some(proposal.id.clone()),
        expected_branch,
        required_files,
        expected_test_command: None,
        required_toolchains,
        required_remote_capabilities: Vec::new(),
        require_clean_worktree,
    }
}

/// Check for drift between desired and actual project state
pub fn check_drift(desired: &DesiredProjectState, actual: &ActualProjectState) -> DriftCheckResult {
    let mut diffs = Vec::new();

    // Branch comparison
    if let (Some(expected), Some(found)) = (&desired.expected_branch, &actual.current_branch) {
        if expected != found && desired.require_clean_worktree {
            diffs.push(StateDiff {
                field: "branch".to_string(),
                severity: DriftSeverity::Blocking,
                expected: expected.clone(),
                actual: found.clone(),
                message: "Worktree task requires a specific branch".to_string(),
            });
        } else if expected != found {
            diffs.push(StateDiff {
                field: "branch".to_string(),
                severity: DriftSeverity::Warning,
                expected: expected.clone(),
                actual: found.clone(),
                message: "Current branch differs from expected".to_string(),
            });
        }
    }

    // Worktree cleanliness
    if desired.require_clean_worktree && !actual.worktree_clean {
        diffs.push(StateDiff {
            field: "worktree_clean".to_string(),
            severity: DriftSeverity::Blocking,
            expected: "clean".to_string(),
            actual: "dirty".to_string(),
            message: "Workspace has uncommitted changes".to_string(),
        });
    }

    // Required files check
    for file in &desired.required_files {
        if !actual.present_files.contains(file) {
            diffs.push(StateDiff {
                field: format!("file:{file}"),
                severity: DriftSeverity::Warning,
                expected: file.clone(),
                actual: "missing".to_string(),
                message: "Required deliverable file not found".to_string(),
            });
        }
    }

    // Toolchain check
    for tool in &desired.required_toolchains {
        if !actual.available_toolchains.contains(tool) {
            diffs.push(StateDiff {
                field: format!("toolchain:{tool}"),
                severity: DriftSeverity::Warning,
                expected: tool.clone(),
                actual: "missing".to_string(),
                message: "Required toolchain not available".to_string(),
            });
        }
    }

    let summary = if diffs.is_empty() {
        "No project drift detected against the current expected state.".to_string()
    } else {
        diffs
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    };

    let response = zn_types::response_for_highest_severity(
        diffs
            .iter()
            .map(|d| d.severity.clone())
            .max_by_key(|s| s.rank()),
    );

    let report = DriftReport {
        schema_version: zn_types::default_spec_schema_version(),
        proposal_id: desired.proposal_id.clone(),
        desired: desired.clone(),
        actual: actual.clone(),
        diffs,
        response,
        summary,
    };

    DriftCheckResult::from_report(report)
}

// --- Helpers ---

fn run_git(project_root: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .current_dir(project_root)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).to_string())
}

fn list_project_files(project_root: &Path) -> Vec<String> {
    std::fs::read_dir(project_root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_string_lossy().to_string();
            // Skip hidden directories and common non-project dirs
            if name.starts_with('.') || name == "target" || name == ".git" || name == "node_modules"
            {
                return None;
            }
            Some(name)
        })
        .collect()
}

fn detect_toolchains() -> Vec<String> {
    let mut toolchains = Vec::new();
    if Command::new("cargo").arg("--version").output().is_ok() {
        toolchains.push("cargo".to_string());
    }
    if Command::new("rustc").arg("--version").output().is_ok() {
        toolchains.push("rustc".to_string());
    }
    if Command::new("node").arg("--version").output().is_ok() {
        toolchains.push("node".to_string());
    }
    if Command::new("python3").arg("--version").output().is_ok() {
        toolchains.push("python3".to_string());
    }
    toolchains
}

fn detect_test_command(project_root: &Path) -> Option<String> {
    if project_root.join("Cargo.toml").exists() {
        Some("cargo test".to_string())
    } else if project_root.join("package.json").exists() {
        Some("npm test".to_string())
    } else if project_root.join("pytest.ini").exists()
        || project_root.join("pyproject.toml").exists()
        || project_root.join("setup.py").exists()
    {
        Some("pytest".to_string())
    } else {
        None
    }
}

fn detect_remote_capabilities(project_root: &Path) -> Vec<String> {
    let mut caps = Vec::new();
    if run_git(project_root, &["remote", "-v"])
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        caps.push("git_remote".to_string());
    }
    if Command::new("gh").arg("--version").output().is_ok() {
        caps.push("gh_cli".to_string());
    }
    caps
}
