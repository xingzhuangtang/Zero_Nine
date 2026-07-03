//! Learning Memory Manager — bridges execution outcomes to the memory system
//! for cross-proposal pattern learning.

use anyhow::Result;
use chrono::Utc;
use std::path::Path;
use std::sync::Arc;

use crate::memory_store::{MemoryStore, SqliteMemoryStore};
use zn_types::{
    ExecutionOutcome, ExecutionReport, MemoryEntry, MemoryLevel, MemoryQuery, MemorySearchResult,
};

/// Learning memory manager — records outcomes and retrieves learned patterns.
pub struct LearningMemoryManager {
    store: Arc<dyn MemoryStore>,
}

impl LearningMemoryManager {
    pub fn new(project_root: &Path) -> Result<Self> {
        let db_dir = project_root.join(".zero_nine/memory");
        std::fs::create_dir_all(&db_dir)?;
        let db_path = db_dir.join("learning.db");
        let store = Arc::new(SqliteMemoryStore::new(&db_path)?);
        Ok(Self { store })
    }

    /// Record a learning entry from an execution report.
    pub fn record_outcome(&self, report: &ExecutionReport) -> Result<()> {
        let is_success = matches!(report.outcome, ExecutionOutcome::Completed);
        let key = format!("outcome:{}", report.task_id);
        let content = format!(
            "Task {} — outcome: {:?} — summary: {}{}",
            report.task_id,
            report.outcome,
            report.summary,
            report
                .failure_summary
                .as_ref()
                .map(|s| format!("\nFailure: {}", s))
                .unwrap_or_default()
        );

        let entry = MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            level: MemoryLevel::Task,
            key,
            content,
            tags: vec![
                format!("outcome:{:?}", report.outcome),
                "execution-result".to_string(),
            ],
            session_id: None,
            task_id: Some(report.task_id.clone()),
            relevance_score: if is_success { 0.8 } else { 0.3 },
            access_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.store.store(&entry)?;

        // For failures, also store as coding-level pattern for cross-proposal learning
        if !is_success {
            if let Some(ref failure_class) = report.failure_classification {
                let pattern_entry = MemoryEntry::new_coding(
                    format!("failure-pattern:{:?}", failure_class.category),
                    format!(
                        "Category: {:?} — severity: {:?} — root_cause: {}{}",
                        failure_class.category,
                        failure_class.severity,
                        failure_class.root_cause.as_deref().unwrap_or("unknown"),
                        failure_class
                            .suggested_fix
                            .as_ref()
                            .map(|s| format!("\nFix: {}", s))
                            .unwrap_or_default()
                    ),
                    vec![
                        format!("category:{:?}", failure_class.category),
                        format!("severity:{:?}", failure_class.severity),
                    ],
                );
                self.store.store(&pattern_entry)?;
            }
        }

        Ok(())
    }

    /// Search for relevant learned patterns given a task description.
    pub fn find_relevant_patterns(&self, task_description: &str) -> Result<MemorySearchResult> {
        let query = MemoryQuery {
            query: task_description.to_string(),
            levels: vec![MemoryLevel::Coding, MemoryLevel::Task],
            tags: vec![
                "failure-pattern".to_string(),
                "execution-result".to_string(),
            ],
            max_results: 5,
            min_relevance: 0.2,
        };
        self.store.search(&query)
    }

    /// Get failure statistics across all proposals.
    pub fn get_failure_stats(&self) -> Result<FailureStats> {
        let all_task = self.store.list_by_level(&MemoryLevel::Task)?;
        let all_coding = self.store.list_by_level(&MemoryLevel::Coding)?;

        let total = all_task.len();
        let failures = all_task
            .iter()
            .filter(|e| {
                e.tags.iter().any(|t| {
                    t.starts_with("outcome:RetryableFailure") || t.starts_with("outcome:Escalated")
                })
            })
            .count();

        let pattern_count = all_coding
            .iter()
            .filter(|e| e.key.starts_with("failure-pattern:"))
            .count();

        Ok(FailureStats {
            total_executions: total,
            failures,
            unique_failure_patterns: pattern_count,
            success_rate: if total > 0 {
                1.0 - (failures as f32 / total as f32)
            } else {
                1.0
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct FailureStats {
    pub total_executions: usize,
    pub failures: usize,
    pub unique_failure_patterns: usize,
    pub success_rate: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_record_success_outcome() {
        let tmp_dir = temp_dir().join(format!("zn_learn_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mgr = LearningMemoryManager::new(&tmp_dir).unwrap();
        let report = ExecutionReport {
            task_id: "task-success-1".to_string(),
            success: true,
            outcome: ExecutionOutcome::Completed,
            summary: "All tests passed".to_string(),
            ..Default::default()
        };

        mgr.record_outcome(&report).unwrap();
        let stats = mgr.get_failure_stats().unwrap();
        assert_eq!(stats.total_executions, 1);
        assert_eq!(stats.failures, 0);
        assert_eq!(stats.success_rate, 1.0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_record_failure_stores_coding_pattern() {
        let tmp_dir = temp_dir().join(format!("zn_learn_fail_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mgr = LearningMemoryManager::new(&tmp_dir).unwrap();

        // First record a success to have baseline
        let success_report = ExecutionReport {
            task_id: "task-ok".to_string(),
            success: true,
            outcome: ExecutionOutcome::Completed,
            summary: "OK".to_string(),
            ..Default::default()
        };
        mgr.record_outcome(&success_report).unwrap();

        // Then record a failure
        let fail_report = ExecutionReport {
            task_id: "task-fail-1".to_string(),
            success: false,
            outcome: ExecutionOutcome::RetryableFailure,
            summary: "Build failed".to_string(),
            failure_summary: Some("cargo test failed".to_string()),
            ..Default::default()
        };
        mgr.record_outcome(&fail_report).unwrap();

        let stats = mgr.get_failure_stats().unwrap();
        assert_eq!(stats.total_executions, 2);
        assert_eq!(stats.failures, 1);
        assert!((stats.success_rate - 0.5).abs() < 0.01);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_failure_stats_empty() {
        let tmp_dir = temp_dir().join(format!("zn_learn_empty_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mgr = LearningMemoryManager::new(&tmp_dir).unwrap();
        let stats = mgr.get_failure_stats().unwrap();
        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.failures, 0);
        assert_eq!(stats.success_rate, 1.0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
