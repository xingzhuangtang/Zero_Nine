//! Subagent Dispatcher - Dispatch tasks to subagents and collect results
//!
//! This module provides:
//! - Subagent dispatch protocol
//! - Context preparation for each role
//! - Result collection and aggregation
//! - Recovery ledger for interrupted dispatches
//! - Actual subagent execution via Claude Code CLI

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;
use zn_types::{
    SubagentBrief, SubagentDispatch, SubagentRecoveryRecord, SubagentRecoveryLedger,
    SubagentRunBook, SubagentRunStatus,
};

/// Subagent dispatcher for managing multi-role execution
pub struct SubagentDispatcher {
    project_root: PathBuf,
    proposal_id: String,
    task_id: String,
    recovery_ledger_path: PathBuf,
}

/// Dispatch result from a subagent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResult {
    /// Role that executed
    pub role: String,
    /// Whether execution succeeded
    pub success: bool,
    /// Generated output files
    pub output_files: Vec<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Raw output content
    pub raw_output: Option<String>,
}

/// Context bundle for subagent dispatch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentContext {
    /// Target role
    pub role: String,
    /// Context files to inject
    pub context_files: HashMap<String, String>,
    /// Expected outputs
    pub expected_outputs: Vec<String>,
    /// Task objective
    pub objective: String,
}

/// Check if the Claude CLI is available on PATH
pub fn is_claude_available() -> bool {
    Command::new("claude")
        .arg("--version")
        .output()
        .is_ok()
}

/// Report from batch subagent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentExecutionReport {
    /// Individual dispatch results
    pub results: Vec<DispatchResult>,
    /// Whether all dispatches succeeded
    pub all_succeeded: bool,
    /// Total number of dispatches
    pub total: usize,
    /// Number of successful dispatches
    pub succeeded: usize,
    /// Number of failed dispatches
    pub failed: usize,
}

impl SubagentDispatcher {
    /// Create a new subagent dispatcher
    pub fn new(project_root: &Path, proposal_id: &str, task_id: &str) -> Result<Self> {
        let recovery_dir = project_root
            .join(".zero_nine/runtime/subagents")
            .join(proposal_id);
        fs::create_dir_all(&recovery_dir)?;

        let recovery_ledger_path = recovery_dir.join(format!("{}-recovery.json", task_id));

        Ok(Self {
            project_root: project_root.to_path_buf(),
            proposal_id: proposal_id.to_string(),
            task_id: task_id.to_string(),
            recovery_ledger_path,
        })
    }

    /// Load existing recovery ledger
    pub fn load_recovery_ledger(&self) -> Option<SubagentRecoveryLedger> {
        if !self.recovery_ledger_path.exists() {
            return None;
        }

        let content = fs::read_to_string(&self.recovery_ledger_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save recovery ledger
    pub fn save_recovery_ledger(&self, ledger: &SubagentRecoveryLedger) -> Result<()> {
        if let Some(parent) = self.recovery_ledger_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(ledger)?;
        fs::write(&self.recovery_ledger_path, content)?;
        Ok(())
    }

    /// Create a runbook for subagent dispatch
    pub fn create_runbook(&self, briefs: &[SubagentBrief], _objective: &str) -> SubagentRunBook {
        let dispatches: Vec<SubagentDispatch> = briefs
            .iter()
            .map(|brief| SubagentDispatch {
                role: brief.role.clone(),
                command_hint: format!("Dispatch {} to: {}", brief.role, brief.goal),
                context_files: brief.inputs.clone(),
                expected_outputs: brief.outputs.clone(),
            })
            .collect();

        SubagentRunBook {
            task_id: self.task_id.clone(),
            dispatches,
            runtime: None,
        }
    }

    /// Save runbook to disk
    pub fn save_runbook(&self, runbook: &SubagentRunBook) -> Result<String> {
        let runbooks_dir = self
            .project_root
            .join(".zero_nine/runtime/subagents/runbooks");
        fs::create_dir_all(&runbooks_dir)?;

        let runbook_path = runbooks_dir.join(format!("{}-runbook.json", self.task_id));
        let content = serde_json::to_string_pretty(runbook)?;
        fs::write(&runbook_path, &content)?;

        Ok(runbook_path.display().to_string())
    }

    /// Prepare context for a specific role
    pub fn prepare_context(&self, context: &SubagentContext) -> Result<SubagentContextBundle> {
        let bundle_dir = self
            .project_root
            .join(".zero_nine/runtime/subagents/context")
            .join(&self.task_id)
            .join(&context.role);
        fs::create_dir_all(&bundle_dir)?;

        let mut loaded_files = HashMap::new();
        for (name, content) in &context.context_files {
            let file_path = bundle_dir.join(format!("{}.md", name.replace('/', "_")));
            fs::write(&file_path, content)?;
            loaded_files.insert(name.clone(), file_path.display().to_string());
        }

        // Write context manifest
        let manifest_path = bundle_dir.join("context_manifest.json");
        let manifest = serde_json::json!({
            "role": context.role,
            "objective": context.objective,
            "expected_outputs": context.expected_outputs,
            "files": loaded_files,
            "prepared_at": Utc::now().to_rfc3339(),
        });
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

        Ok(SubagentContextBundle {
            role: context.role.clone(),
            bundle_dir,
            manifest_path,
            expected_outputs: context.expected_outputs.clone(),
        })
    }

    /// Record dispatch result
    pub fn record_dispatch(&mut self, result: &DispatchResult) -> Result<()> {
        let mut ledger = self.load_recovery_ledger().unwrap_or_else(|| SubagentRecoveryLedger {
            task_id: self.task_id.clone(),
            records: Vec::new(),
            replay_summary: String::new(),
        });

        let status = if result.success {
            SubagentRunStatus::Recovered
        } else {
            SubagentRunStatus::Failed
        };

        let record = SubagentRecoveryRecord {
            role: result.role.clone(),
            status,
            summary: result.raw_output.clone().unwrap_or_default(),
            expected_outputs: Vec::new(),
            actual_outputs: result.output_files.clone(),
            failure_summary: result.error.clone(),
            evidence_paths: result.output_files.clone(),
            evidence_archive_path: None,
            replay_ready: result.success,
            replay_command: None,
            state_transitions: Vec::new(),
        };

        ledger.records.push(record);
        self.save_recovery_ledger(&ledger)?;

        info!(
            "Recorded dispatch result for role: {} (success: {})",
            result.role, result.success
        );
        Ok(())
    }

    /// Check if all dispatches are complete
    pub fn all_dispatches_complete(&self) -> bool {
        if let Some(ledger) = self.load_recovery_ledger() {
            ledger.records.iter().all(|r| {
                matches!(
                    r.status,
                    SubagentRunStatus::Recovered | SubagentRunStatus::Failed
                )
            })
        } else {
            false
        }
    }

    /// Aggregate results from all subagent runs
    pub fn aggregate_results(&self) -> Result<AggregatedResults> {
        let ledger = self
            .load_recovery_ledger()
            .context("No recovery ledger found")?;

        let mut outputs = HashMap::new();
        let mut evidence_files = Vec::new();
        let mut errors = Vec::new();

        for record in &ledger.records {
            if matches!(record.status, SubagentRunStatus::Recovered) {
                outputs.insert(record.role.clone(), record.summary.clone());
                evidence_files.extend(record.evidence_paths.clone());
            } else if let Some(err) = &record.failure_summary {
                errors.push(format!("{}: {}", record.role, err));
            }
        }

        Ok(AggregatedResults {
            task_id: self.task_id.clone(),
            outputs,
            evidence_files,
            errors: errors.clone(),
            all_success: errors.len() == 0,
        })
    }

    /// Aggregate results from a list of dispatch results (for testing and direct use)
    pub fn aggregate_results_from_list(&self, results: &[DispatchResult]) -> AggregatedResults {
        let mut outputs = HashMap::new();
        let mut evidence_files = Vec::new();
        let mut errors = Vec::new();

        for result in results {
            if result.success {
                outputs.insert(
                    result.role.clone(),
                    result.raw_output.clone().unwrap_or_default(),
                );
                evidence_files.extend(result.output_files.clone());
            } else {
                if let Some(err) = &result.error {
                    errors.push(format!("{}: {}", result.role, err));
                }
            }
        }

        AggregatedResults {
            task_id: self.task_id.clone(),
            outputs,
            evidence_files,
            errors: errors.clone(),
            all_success: errors.is_empty(),
        }
    }

    /// Execute all dispatches sequentially, individual failures don't stop the overall run
    pub fn try_execute_all(&mut self, runbook: &SubagentRunBook) -> SubagentExecutionReport {
        let mut results = Vec::new();

        for dispatch in &runbook.dispatches {
            match self.execute_dispatch(dispatch) {
                Ok(result) => {
                    // Record to recovery ledger regardless of outcome
                    let _ = self.record_dispatch(&result);
                    results.push(result);
                }
                Err(e) => {
                    let error_result = DispatchResult {
                        role: dispatch.role.clone(),
                        success: false,
                        output_files: Vec::new(),
                        error: Some(e.to_string()),
                        raw_output: None,
                    };
                    let _ = self.record_dispatch(&error_result);
                    results.push(error_result);
                }
            }
        }

        let succeeded = results.iter().filter(|r| r.success).count();
        let total = results.len();
        let failed = total - succeeded;

        SubagentExecutionReport {
            results,
            all_succeeded: failed == 0,
            total,
            succeeded,
            failed,
        }
    }

    /// Generate dispatch command for external agent
    pub fn generate_dispatch_command(&self, dispatch: &SubagentDispatch) -> String {
        format!(
            r#"# Dispatch to: {}
# Context files: {:?}
# Expected outputs: {:?}

zero-nine subagent-dispatch \
  --proposal {} \
  --task {} \
  --role {} \
  --context {:?}"#,
            dispatch.role,
            dispatch.context_files,
            dispatch.expected_outputs,
            self.proposal_id,
            self.task_id,
            dispatch.role,
            dispatch.context_files.join(","),
        )
    }

    /// Execute a subagent dispatch using Claude Code CLI
    /// This is the actual implementation that calls external AI
    pub fn execute_dispatch(&self, dispatch: &SubagentDispatch) -> Result<DispatchResult> {
        info!("Executing subagent dispatch for role: {}", dispatch.role);

        // Prepare the prompt for the subagent
        let prompt = self.build_subagent_prompt(dispatch);

        // Build the Claude Code command
        // Uses 'claude' CLI with the prepared context
        let mut cmd = Command::new("claude");
        cmd.arg("--verbose")
            .arg("--prompt")
            .arg(&prompt);

        // Add context files as additional context
        let context_dir = self
            .project_root
            .join(".zero_nine/runtime/subagents/context")
            .join(&self.task_id)
            .join(&dispatch.role);

        if context_dir.exists() {
            // Add context by reading files and appending to prompt
            for entry in fs::read_dir(&context_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "md") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        cmd.arg("--context").arg(&content);
                    }
                }
            }
        }

        // Execute the command
        let output = cmd.output();

        match output {
            Ok(output) => {
                let success = output.status.success();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                // Parse output files from the response
                let output_files = self.extract_output_files(&stdout);

                Ok(DispatchResult {
                    role: dispatch.role.clone(),
                    success,
                    output_files,
                    error: if success { None } else { Some(stderr) },
                    raw_output: Some(stdout),
                })
            }
            Err(e) => Err(anyhow::anyhow!("Failed to execute subagent: {}", e)),
        }
    }

    /// Build a prompt for subagent execution
    fn build_subagent_prompt(&self, dispatch: &SubagentDispatch) -> String {
        format!(
            r#"# Subagent Task

## Role: {}
## Objective: Execute the assigned task as a {} agent

## Context Files: {:?}
## Expected Outputs: {:?}

## Instructions
You are acting as a {} subagent in the Zero_Nine orchestration system.
Your task is to: {}

Please complete the task and produce the expected outputs: {:?}

## Command Hint
{}

## Execution Protocol
1. Read all context files carefully
2. Understand the task objective
3. Execute the required actions
4. Produce the expected output files
5. Report success or failure with evidence
"#,
            dispatch.role,
            dispatch.role,
            dispatch.context_files,
            dispatch.expected_outputs,
            dispatch.role,
            dispatch.role,
            dispatch.expected_outputs,
            dispatch.command_hint,
        )
    }

    /// Extract output file paths from subagent response
    fn extract_output_files(&self, output: &str) -> Vec<String> {
        // Simple heuristic: look for file paths in the output
        // In production, this would parse a structured response
        let mut files = Vec::new();

        for line in output.lines() {
            if line.contains("Created:") || line.contains("Wrote:") || line.contains("Saved:") {
                // Extract path after the keyword
                if let Some(idx) = line.find(':') {
                    let path = line[idx + 1..].trim();
                    if !path.is_empty() {
                        files.push(path.to_string());
                    }
                }
            }
        }

        files
    }
}

/// Context bundle for subagent
pub struct SubagentContextBundle {
    pub role: String,
    pub bundle_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub expected_outputs: Vec<String>,
}

/// Aggregated results from all subagents
#[derive(Debug, Clone)]
pub struct AggregatedResults {
    pub task_id: String,
    pub outputs: HashMap<String, String>,
    pub evidence_files: Vec<String>,
    pub errors: Vec<String>,
    pub all_success: bool,
}

/// Verdict from tri-role workflow (developer/reviewer/verifier)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriRoleVerdict {
    /// All three roles passed successfully
    Pass,
    /// Developer failed to produce implementation
    DevelopmentFailed,
    /// Reviewer rejected the implementation
    ReviewRejected,
    /// Verifier found acceptance criteria not met
    VerificationFailed,
}

/// Compute tri-role verdict from dispatch results
pub fn compute_tri_role_verdict(results: &[DispatchResult]) -> TriRoleVerdict {
    if results.is_empty() {
        return TriRoleVerdict::DevelopmentFailed;
    }

    // Check developer result
    if let Some(dev_result) = results.iter().find(|r| r.role == "developer") {
        if !dev_result.success {
            return TriRoleVerdict::DevelopmentFailed;
        }
    } else {
        return TriRoleVerdict::DevelopmentFailed;
    }

    // Check reviewer result (if present)
    if let Some(review_result) = results.iter().find(|r| r.role == "reviewer") {
        if !review_result.success {
            return TriRoleVerdict::ReviewRejected;
        }
    }

    // Check verifier result (if present)
    if let Some(verify_result) = results.iter().find(|r| r.role == "verifier") {
        if !verify_result.success {
            return TriRoleVerdict::VerificationFailed;
        }
    }

    TriRoleVerdict::Pass
}

/// Create default subagent dispatcher
pub fn create_dispatcher(project_root: &Path, proposal_id: &str, task_id: &str) -> Result<SubagentDispatcher> {
    SubagentDispatcher::new(project_root, proposal_id, task_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_create_dispatcher() {
        let tmp_dir = temp_dir().join("subagent_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let dispatcher = SubagentDispatcher::new(&tmp_dir, "test-proposal", "test-task").unwrap();
        assert_eq!(dispatcher.task_id, "test-task");
        assert!(!dispatcher.recovery_ledger_path.exists());

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_recovery_ledger_lifecycle() {
        let tmp_dir = temp_dir().join("subagent_ledger_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let mut dispatcher = SubagentDispatcher::new(&tmp_dir, "test-proposal", "test-task").unwrap();

        // Initial load should return None
        assert!(dispatcher.load_recovery_ledger().is_none());

        // Create and save ledger
        let ledger = SubagentRecoveryLedger {
            task_id: "test-task".to_string(),
            records: Vec::new(),
            replay_summary: String::new(),
        };
        dispatcher.save_recovery_ledger(&ledger).unwrap();
        assert!(dispatcher.recovery_ledger_path.exists());

        // Load should now succeed
        let loaded = dispatcher.load_recovery_ledger().unwrap();
        assert_eq!(loaded.task_id, "test-task");

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_create_runbook() {
        let tmp_dir = temp_dir().join("subagent_runbook_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let dispatcher = SubagentDispatcher::new(&tmp_dir, "test-proposal", "test-task").unwrap();

        let briefs = vec![
            SubagentBrief {
                role: "developer".to_string(),
                goal: "Implement the feature".to_string(),
                inputs: vec!["design.md".to_string()],
                outputs: vec!["code".to_string()],
            },
            SubagentBrief {
                role: "reviewer".to_string(),
                goal: "Review the implementation".to_string(),
                inputs: vec!["code".to_string()],
                outputs: vec!["review verdict".to_string()],
            },
        ];

        let runbook = dispatcher.create_runbook(&briefs, "Test objective");
        assert_eq!(runbook.task_id, "test-task");
        assert_eq!(runbook.dispatches.len(), 2);

        let runbook_path = dispatcher.save_runbook(&runbook).unwrap();
        assert!(Path::new(&runbook_path).exists());

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_record_dispatch() {
        let tmp_dir = temp_dir().join("subagent_dispatch_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let mut dispatcher = SubagentDispatcher::new(&tmp_dir, "test-proposal", "test-task").unwrap();

        let result = DispatchResult {
            role: "developer".to_string(),
            success: true,
            output_files: vec!["code.diff".to_string()],
            error: None,
            raw_output: Some("Implementation complete".to_string()),
        };

        dispatcher.record_dispatch(&result).unwrap();

        let ledger = dispatcher.load_recovery_ledger().unwrap();
        assert_eq!(ledger.records.len(), 1);
        assert_eq!(ledger.records[0].role, "developer");

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_tri_role_workflow_lifecycle() {
        let tmp_dir = temp_dir().join("subagent_tri_role_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let dispatcher = SubagentDispatcher::new(&tmp_dir, "test-proposal", "test-task").unwrap();

        let briefs = vec![
            SubagentBrief {
                role: "developer".to_string(),
                goal: "Implement feature".to_string(),
                inputs: vec!["design.md".to_string()],
                outputs: vec!["implementation.diff".to_string()],
            },
            SubagentBrief {
                role: "reviewer".to_string(),
                goal: "Review implementation".to_string(),
                inputs: vec!["implementation.diff".to_string()],
                outputs: vec!["review-verdict.md".to_string()],
            },
            SubagentBrief {
                role: "verifier".to_string(),
                goal: "Verify acceptance criteria".to_string(),
                inputs: vec!["review-verdict.md".to_string()],
                outputs: vec!["verification-report.md".to_string()],
            },
        ];

        let runbook = dispatcher.create_runbook(&briefs, "Test tri-role workflow");
        assert_eq!(runbook.dispatches.len(), 3);
        assert_eq!(runbook.dispatches[0].role, "developer");
        assert_eq!(runbook.dispatches[1].role, "reviewer");
        assert_eq!(runbook.dispatches[2].role, "verifier");

        let runbook_path = dispatcher.save_runbook(&runbook).unwrap();
        assert!(Path::new(&runbook_path).exists());

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_aggregate_results_all_success() {
        let tmp_dir = temp_dir().join("subagent_aggregate_success_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let mut dispatcher = SubagentDispatcher::new(&tmp_dir, "test-proposal", "test-task").unwrap();

        let results = vec![
            DispatchResult {
                role: "developer".to_string(),
                success: true,
                output_files: vec!["code.diff".to_string()],
                error: None,
                raw_output: Some("Implementation complete".to_string()),
            },
            DispatchResult {
                role: "reviewer".to_string(),
                success: true,
                output_files: vec!["review.md".to_string()],
                error: None,
                raw_output: Some("Review passed".to_string()),
            },
            DispatchResult {
                role: "verifier".to_string(),
                success: true,
                output_files: vec!["verification.md".to_string()],
                error: None,
                raw_output: Some("Verification passed".to_string()),
            },
        ];

        for result in &results {
            dispatcher.record_dispatch(result).unwrap();
        }

        let ledger = dispatcher.load_recovery_ledger().unwrap();
        assert_eq!(ledger.records.len(), 3);

        let aggregated = dispatcher.aggregate_results_from_list(&results);
        assert!(aggregated.all_success);
        assert_eq!(aggregated.outputs.len(), 3);
        assert_eq!(aggregated.errors.len(), 0);

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_aggregate_results_with_failure() {
        let tmp_dir = temp_dir().join("subagent_aggregate_failure_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let mut dispatcher = SubagentDispatcher::new(&tmp_dir, "test-proposal", "test-task").unwrap();

        let results = vec![
            DispatchResult {
                role: "developer".to_string(),
                success: true,
                output_files: vec!["code.diff".to_string()],
                error: None,
                raw_output: Some("Implementation complete".to_string()),
            },
            DispatchResult {
                role: "reviewer".to_string(),
                success: false,
                output_files: vec![],
                error: Some("Review failed: quality issues".to_string()),
                raw_output: Some("Review rejected".to_string()),
            },
        ];

        for result in &results {
            dispatcher.record_dispatch(result).unwrap();
        }

        let aggregated = dispatcher.aggregate_results_from_list(&results);
        assert!(!aggregated.all_success);
        assert_eq!(aggregated.errors.len(), 1);
        assert!(aggregated.errors[0].contains("quality issues"));

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_compute_verdict_all_pass() {
        let results = vec![
            DispatchResult {
                role: "developer".to_string(),
                success: true,
                output_files: vec!["code.diff".to_string()],
                error: None,
                raw_output: None,
            },
            DispatchResult {
                role: "reviewer".to_string(),
                success: true,
                output_files: vec!["review.md".to_string()],
                error: None,
                raw_output: None,
            },
            DispatchResult {
                role: "verifier".to_string(),
                success: true,
                output_files: vec!["verification.md".to_string()],
                error: None,
                raw_output: None,
            },
        ];

        let verdict = compute_tri_role_verdict(&results);
        assert_eq!(verdict, TriRoleVerdict::Pass);
    }

    #[test]
    fn test_compute_verdict_review_reject() {
        let results = vec![
            DispatchResult {
                role: "developer".to_string(),
                success: true,
                output_files: vec!["code.diff".to_string()],
                error: None,
                raw_output: None,
            },
            DispatchResult {
                role: "reviewer".to_string(),
                success: false,
                output_files: vec![],
                error: Some("Quality issues found".to_string()),
                raw_output: None,
            },
        ];

        let verdict = compute_tri_role_verdict(&results);
        assert_eq!(verdict, TriRoleVerdict::ReviewRejected);
    }

    #[test]
    fn test_compute_verdict_verify_fail() {
        let results = vec![
            DispatchResult {
                role: "developer".to_string(),
                success: true,
                output_files: vec!["code.diff".to_string()],
                error: None,
                raw_output: None,
            },
            DispatchResult {
                role: "reviewer".to_string(),
                success: true,
                output_files: vec!["review.md".to_string()],
                error: None,
                raw_output: None,
            },
            DispatchResult {
                role: "verifier".to_string(),
                success: false,
                output_files: vec![],
                error: Some("Acceptance criteria not met".to_string()),
                raw_output: None,
            },
        ];

        let verdict = compute_tri_role_verdict(&results);
        assert_eq!(verdict, TriRoleVerdict::VerificationFailed);
    }

    #[test]
    fn test_compute_verdict_developer_fail() {
        let results = vec![
            DispatchResult {
                role: "developer".to_string(),
                success: false,
                output_files: vec![],
                error: Some("Implementation failed".to_string()),
                raw_output: None,
            },
        ];

        let verdict = compute_tri_role_verdict(&results);
        assert_eq!(verdict, TriRoleVerdict::DevelopmentFailed);
    }
}
