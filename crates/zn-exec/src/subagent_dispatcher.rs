//! Subagent Dispatcher - Dispatch tasks to subagents and collect results
//!
//! This module provides:
//! - Subagent dispatch protocol
//! - Context preparation for each role
//! - Result collection and aggregation
//! - Recovery ledger for interrupted dispatches

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
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
}
