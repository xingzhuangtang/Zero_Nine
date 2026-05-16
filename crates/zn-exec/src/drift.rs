use std::path::Path;
use std::process::Command;
use zn_types::{
    ActualProjectState, CompensationAction, CompensationType, DesiredProjectState,
    DriftCheckResult, DriftReport, DriftSeverity, ExecutionPlan, LoopStage, Proposal, StateDiff,
    WorkspaceStrategy,
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

// ============================================================================
// T2.3: State Machine Consistency Check
// ============================================================================

/// Verify that the LoopStage state machine is consistent with the actual Git worktree state.
///
/// Detects "ghost branches" (worktrees referencing deleted branches) and
/// uncommitted changes that could cause state machine misalignment.
pub fn check_state_machine_consistency(
    project_root: &Path,
    current_stage: &LoopStage,
) -> Vec<StateDiff> {
    let mut diffs = Vec::new();

    // Check: RunningTask/Verifying/Retrying stages require a clean worktree
    if matches!(
        current_stage,
        LoopStage::RunningTask | LoopStage::Verifying | LoopStage::Retrying
    ) {
        let dirty = run_git(project_root, &["status", "--porcelain"])
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if dirty {
            diffs.push(StateDiff {
                field: "stage:worktree_consistency".to_string(),
                severity: DriftSeverity::Dangerous,
                expected: "clean worktree during active execution".to_string(),
                actual: "dirty worktree".to_string(),
                message: format!(
                    "Stage {:?} requires clean worktree but uncommitted changes detected",
                    current_stage
                ),
            });
        }
    }

    // Check: Completed/Escalated stage should have no lingering worktrees for active tasks
    if matches!(current_stage, LoopStage::Completed | LoopStage::Escalated) {
        if let Some(output) = run_git(project_root, &["worktree", "list", "--porcelain"]) {
            for line in output.lines() {
                if line.starts_with("worktree ") && line.contains("zero-nine/task-") {
                    diffs.push(StateDiff {
                        field: "stage:lingering_worktree".to_string(),
                        severity: DriftSeverity::Warning,
                        expected: "no task worktrees after completion".to_string(),
                        actual: format!("found worktree: {}", line),
                        message: "Lingering task worktree after proposal completion".to_string(),
                    });
                }
            }
        }
    }

    // Check: Ghost branches — branches from zero-nine/ that no longer have proposals
    if let Some(branches_output) = run_git(project_root, &["branch", "--list", "zero-nine/*"]) {
        for branch in branches_output.lines() {
            let branch_name = branch.trim().trim_start_matches("* ").trim();
            if branch_name.is_empty() {
                continue;
            }
            // Check if corresponding proposal directory exists
            let task_id = branch_name.trim_start_matches("zero-nine/");
            let proposal_dir = project_root
                .join(".zero_nine/proposals")
                .read_dir()
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .any(|e| {
                    e.path()
                        .join(format!("task-{}-report.md", task_id))
                        .exists()
                        || e.path()
                            .join(format!("task-{}-envelope.json", task_id))
                            .exists()
                });
            if !proposal_dir {
                diffs.push(StateDiff {
                    field: format!("ghost_branch:{}", branch_name),
                    severity: DriftSeverity::Info,
                    expected: "branch should not exist".to_string(),
                    actual: branch_name.to_string(),
                    message: "Ghost branch with no corresponding proposal data".to_string(),
                });
            }
        }
    }

    diffs
}

// ============================================================================
// T2.3: Drift Compensation
// ============================================================================

/// Generate compensation actions for detected drift.
///
/// Returns a list of actions that can clean up the workspace state.
pub fn generate_compensation_actions(
    project_root: &Path,
    diffs: &[StateDiff],
) -> Vec<CompensationAction> {
    let mut actions = Vec::new();

    for diff in diffs {
        match diff.field.as_str() {
            "worktree_clean" | "stage:worktree_consistency" => {
                if matches!(
                    diff.severity,
                    DriftSeverity::Dangerous | DriftSeverity::Blocking
                ) {
                    actions.push(CompensationAction {
                        action_type: CompensationType::ResetWorkspace,
                        target: "HEAD".to_string(),
                        reason: format!("Drift compensation: {}", diff.message),
                        executed: false,
                    });
                }
            }
            f if f.starts_with("ghost_branch:") => {
                let branch = f.strip_prefix("ghost_branch:").unwrap_or(f);
                actions.push(CompensationAction {
                    action_type: CompensationType::DeleteBranch,
                    target: branch.to_string(),
                    reason: format!("Drift compensation: {}", diff.message),
                    executed: false,
                });
            }
            f if f.starts_with("stage:lingering_worktree") => {
                // Extract worktree path from the diff
                if let Some(path) = diff.actual.strip_prefix("found worktree: ") {
                    actions.push(CompensationAction {
                        action_type: CompensationType::DeleteWorktree,
                        target: path.to_string(),
                        reason: format!("Drift compensation: {}", diff.message),
                        executed: false,
                    });
                }
            }
            _ => {}
        }
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs;

    #[test]
    fn test_check_drift_no_drift() {
        let desired = DesiredProjectState {
            schema_version: "v1".into(),
            proposal_id: Some("p1".into()),
            expected_branch: Some("main".into()),
            required_files: vec!["Cargo.toml".into()],
            expected_test_command: None,
            required_toolchains: vec!["cargo".into()],
            required_remote_capabilities: vec![],
            require_clean_worktree: true,
        };
        let actual = ActualProjectState {
            schema_version: "v1".into(),
            proposal_id: None,
            current_branch: Some("main".into()),
            worktree_clean: true,
            present_files: vec!["Cargo.toml".into()],
            available_toolchains: vec!["cargo".into()],
            detected_test_command: None,
            remote_capabilities: vec![],
            notes: vec![],
        };
        let result = check_drift(&desired, &actual);
        assert!(result.report.diffs.is_empty());
        assert!(!result.blocking);
    }

    #[test]
    fn test_check_drift_branch_mismatch() {
        let desired = DesiredProjectState {
            schema_version: "v1".into(),
            proposal_id: Some("p1".into()),
            expected_branch: Some("feature".into()),
            required_files: vec![],
            expected_test_command: None,
            required_toolchains: vec![],
            required_remote_capabilities: vec![],
            require_clean_worktree: false,
        };
        let actual = ActualProjectState {
            schema_version: "v1".into(),
            proposal_id: None,
            current_branch: Some("main".into()),
            worktree_clean: true,
            present_files: vec![],
            available_toolchains: vec![],
            detected_test_command: None,
            remote_capabilities: vec![],
            notes: vec![],
        };
        let result = check_drift(&desired, &actual);
        assert!(!result.report.diffs.is_empty());
        assert_eq!(result.report.diffs[0].field, "branch");
    }

    #[test]
    fn test_check_drift_dirty_blocking() {
        let desired = DesiredProjectState {
            schema_version: "v1".into(),
            proposal_id: Some("p1".into()),
            expected_branch: None,
            required_files: vec![],
            expected_test_command: None,
            required_toolchains: vec![],
            required_remote_capabilities: vec![],
            require_clean_worktree: true,
        };
        let actual = ActualProjectState {
            schema_version: "v1".into(),
            proposal_id: None,
            current_branch: None,
            worktree_clean: false,
            present_files: vec![],
            available_toolchains: vec![],
            detected_test_command: None,
            remote_capabilities: vec![],
            notes: vec![],
        };
        let result = check_drift(&desired, &actual);
        assert!(!result.report.diffs.is_empty());
        let diff = &result.report.diffs[0];
        assert_eq!(diff.field, "worktree_clean");
        assert!(matches!(diff.severity, DriftSeverity::Blocking));
    }

    #[test]
    fn test_check_drift_missing_files() {
        let desired = DesiredProjectState {
            schema_version: "v1".into(),
            proposal_id: Some("p1".into()),
            expected_branch: None,
            required_files: vec!["README.md".into(), "Cargo.toml".into()],
            expected_test_command: None,
            required_toolchains: vec![],
            required_remote_capabilities: vec![],
            require_clean_worktree: false,
        };
        let actual = ActualProjectState {
            schema_version: "v1".into(),
            proposal_id: None,
            current_branch: None,
            worktree_clean: true,
            present_files: vec!["Cargo.toml".into()],
            available_toolchains: vec![],
            detected_test_command: None,
            remote_capabilities: vec![],
            notes: vec![],
        };
        let result = check_drift(&desired, &actual);
        assert!(!result.report.diffs.is_empty());
        // Missing files are reported as "file:<name>"
        assert!(result.report.diffs.iter().any(|d| d.field == "file:README.md"));
    }

    #[test]
    fn test_check_drift_toolchain_mismatch() {
        let desired = DesiredProjectState {
            schema_version: "v1".into(),
            proposal_id: Some("p1".into()),
            expected_branch: None,
            required_files: vec![],
            expected_test_command: None,
            required_toolchains: vec!["rustc".into(), "cargo".into()],
            required_remote_capabilities: vec![],
            require_clean_worktree: false,
        };
        let actual = ActualProjectState {
            schema_version: "v1".into(),
            proposal_id: None,
            current_branch: None,
            worktree_clean: true,
            present_files: vec![],
            available_toolchains: vec!["cargo".into()],
            detected_test_command: None,
            remote_capabilities: vec![],
            notes: vec![],
        };
        let result = check_drift(&desired, &actual);
        assert!(!result.report.diffs.is_empty());
        // Missing toolchains are reported as "toolchain:<name>"
        assert!(result.report.diffs.iter().any(|d| d.field == "toolchain:rustc"));
    }

    #[test]
    fn test_generate_compensation_reset() {
        let tmp = temp_dir().join("zn_drift_test_reset");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let diffs = vec![StateDiff {
            field: "worktree_clean".into(),
            severity: DriftSeverity::Blocking,
            expected: "clean".into(),
            actual: "dirty".into(),
            message: "dirty worktree".into(),
        }];
        let actions = generate_compensation_actions(&tmp, &diffs);
        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| matches!(
            a.action_type,
            CompensationType::ResetWorkspace
        )));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_generate_compensation_delete_branch() {
        let tmp = temp_dir().join("zn_drift_test_branch");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let diffs = vec![StateDiff {
            field: "ghost_branch:old-feature".into(),
            severity: DriftSeverity::Warning,
            expected: "no ghost branches".into(),
            actual: "old-feature".into(),
            message: "ghost branch detected".into(),
        }];
        let actions = generate_compensation_actions(&tmp, &diffs);
        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| matches!(
            a.action_type,
            CompensationType::DeleteBranch
        )));
        assert_eq!(actions[0].target, "old-feature");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_generate_compensation_delete_worktree() {
        let tmp = temp_dir().join("zn_drift_test_worktree");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let diffs = vec![StateDiff {
            field: "stage:lingering_worktree".into(),
            severity: DriftSeverity::Warning,
            expected: "no worktrees".into(),
            actual: "found worktree: /tmp/wt".into(),
            message: "lingering worktree".into(),
        }];
        let actions = generate_compensation_actions(&tmp, &diffs);
        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| matches!(
            a.action_type,
            CompensationType::DeleteWorktree
        )));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_test_command_cargo() {
        let tmp = temp_dir().join("zn_drift_test_cargo");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("Cargo.toml"), "").unwrap();
        assert_eq!(detect_test_command(&tmp), Some("cargo test".into()));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_test_command_npm() {
        let tmp = temp_dir().join("zn_drift_test_npm");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("package.json"), "").unwrap();
        assert_eq!(detect_test_command(&tmp), Some("npm test".into()));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_test_command_none() {
        let tmp = temp_dir().join("zn_drift_test_none");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        assert_eq!(detect_test_command(&tmp), None);
        let _ = fs::remove_dir_all(&tmp);
    }
}
