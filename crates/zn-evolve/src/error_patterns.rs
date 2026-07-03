//! Error Pattern Detector — identify recurring error patterns from signals.
//!
//! Aggregates evolution signals (from both ExecutionReports and ExternalEvents)
//! and detects recurring patterns that warrant auto-remediation.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use zn_types::{EvolutionAction, EvolutionCandidate, EvolutionKind, EvolutionSignal};

/// A detected error pattern with frequency and remediation info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPattern {
    pub id: String,
    /// Normalized error signature
    pub signature: String,
    /// How many times this pattern has been observed
    pub occurrence_count: u32,
    /// First occurrence
    pub first_seen: DateTime<Utc>,
    /// Most recent occurrence
    pub last_seen: DateTime<Utc>,
    /// Affected task IDs
    pub affected_tasks: Vec<String>,
    /// Suggested remediation action
    pub suggested_action: EvolutionAction,
    /// Confidence in this pattern being actionable
    pub confidence: f32,
}

/// Pattern detector state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PatternState {
    pub patterns: HashMap<String, ErrorPattern>,
    pub signal_count: u64,
}

/// Detects recurring error patterns from evolution signals.
pub struct ErrorPatternDetector {
    state: PatternState,
    state_path: PathBuf,
    /// Minimum occurrences before a pattern is considered actionable
    min_occurrences: u32,
}

impl ErrorPatternDetector {
    pub fn new(project_root: &Path, min_occurrences: u32) -> Result<Self> {
        let state_path = project_root.join(".zero_nine/evolve/error_patterns.json");
        let mut detector = Self {
            state: PatternState::default(),
            state_path,
            min_occurrences,
        };
        detector.load_state()?;
        Ok(detector)
    }

    fn load_state(&mut self) -> Result<()> {
        if self.state_path.exists() {
            let content = std::fs::read_to_string(&self.state_path)?;
            self.state = serde_json::from_str(&content)?;
        }
        Ok(())
    }

    pub fn save_state(&self) -> Result<()> {
        if let Some(parent) = self.state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.state_path, serde_json::to_string_pretty(&self.state)?)?;
        Ok(())
    }

    /// Feed a signal into the pattern detector.
    ///
    /// Returns patterns that have crossed the actionability threshold.
    pub fn ingest_signal(&mut self, signal: &EvolutionSignal) -> Result<Vec<ErrorPattern>> {
        // Only ingest failure-related signals
        if !matches!(
            signal.proposed_action,
            EvolutionAction::AutoFix | EvolutionAction::AutoImprove
        ) {
            return Ok(Vec::new());
        }

        self.state.signal_count += 1;

        let signature = self.compute_signature(signal);

        let pattern = self
            .state
            .patterns
            .entry(signature.clone())
            .or_insert_with(|| ErrorPattern {
                id: format!(
                    "pattern-{}",
                    uuid::Uuid::new_v4()
                        .to_string()
                        .chars()
                        .take(8)
                        .collect::<String>()
                ),
                signature,
                occurrence_count: 0,
                first_seen: signal.detected_at,
                last_seen: signal.detected_at,
                affected_tasks: Vec::new(),
                suggested_action: signal.proposed_action.clone(),
                confidence: signal.confidence,
            });

        pattern.occurrence_count += 1;
        pattern.last_seen = Utc::now();
        if !pattern.affected_tasks.contains(&signal.task_id) {
            pattern.affected_tasks.push(signal.task_id.clone());
        }
        // Update confidence: running average
        pattern.confidence = (pattern.confidence + signal.confidence) / 2.0;

        self.save_state()?;

        Ok(self.actionable_patterns())
    }

    /// Get all patterns that have exceeded the minimum occurrence threshold.
    pub fn actionable_patterns(&self) -> Vec<ErrorPattern> {
        self.state
            .patterns
            .values()
            .filter(|p| p.occurrence_count >= self.min_occurrences)
            .cloned()
            .collect()
    }

    /// Compute a normalized signature for a signal.
    fn compute_signature(&self, signal: &EvolutionSignal) -> String {
        let raw = signal.notes.first().cloned().unwrap_or_default();
        let normalized: String = raw
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        normalized.chars().take(120).collect()
    }

    /// Generate a remediation candidate from an actionable pattern.
    pub fn generate_remediation(&self, pattern: &ErrorPattern) -> Option<EvolutionCandidate> {
        if pattern.occurrence_count < self.min_occurrences {
            return None;
        }

        Some(EvolutionCandidate {
            source_skill: pattern.signature.clone(),
            kind: match pattern.suggested_action {
                EvolutionAction::AutoFix => EvolutionKind::AutoFix,
                EvolutionAction::AutoImprove => EvolutionKind::AutoImprove,
                _ => EvolutionKind::AutoLearn,
            },
            reason: format!(
                "Recurring error pattern ({} occurrences across {} tasks): {}",
                pattern.occurrence_count,
                pattern.affected_tasks.len(),
                pattern.signature
            ),
            patch: format!(
                "Auto-remediation for pattern: {}\nAffected tasks: {}\nConfidence: {:.2}",
                pattern.signature,
                pattern.affected_tasks.join(", "),
                pattern.confidence
            ),
            confidence: pattern.confidence,
            created_at: Utc::now(),
        })
    }

    /// Get all patterns (for CLI inspection).
    pub fn all_patterns(&self) -> Vec<&ErrorPattern> {
        self.state.patterns.values().collect()
    }

    /// Get scheduler statistics
    pub fn get_stats(&self) -> PatternStats {
        let total = self.state.patterns.len();
        let actionable = self.actionable_patterns().len();
        let total_signals = self.state.signal_count;
        let total_occurrences: u32 = self
            .state
            .patterns
            .values()
            .map(|p| p.occurrence_count)
            .sum();

        PatternStats {
            total,
            actionable,
            total_signals,
            total_occurrences,
        }
    }
}

/// Pattern statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternStats {
    pub total: usize,
    pub actionable: usize,
    pub total_signals: u64,
    pub total_occurrences: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::env::temp_dir;
    use zn_types::{EvolutionSignal, EvolutionSignalSource};

    fn make_signal(task_id: &str, notes: &str, action: EvolutionAction) -> EvolutionSignal {
        EvolutionSignal {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: task_id.to_string(),
            score: 0.3,
            decision: "fix".to_string(),
            notes: vec![notes.to_string()],
            source: EvolutionSignalSource::ExecutionReport,
            detected_at: Utc::now(),
            confidence: 0.80,
            proposed_action: action,
        }
    }

    #[test]
    fn test_ingest_single_signal_no_pattern() {
        let tmp_dir = temp_dir().join(format!("zn_pattern_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut detector = ErrorPatternDetector::new(&tmp_dir, 2).unwrap();
        let signal = make_signal("task-1", "compilation error", EvolutionAction::AutoFix);
        let actionable = detector.ingest_signal(&signal).unwrap();
        assert!(actionable.is_empty()); // Only 1 occurrence, threshold is 2

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_ingest_recurring_signal_creates_pattern() {
        let tmp_dir = temp_dir().join(format!("zn_pattern_recurring_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut detector = ErrorPatternDetector::new(&tmp_dir, 2).unwrap();

        let s1 = make_signal("task-1", "compilation error", EvolutionAction::AutoFix);
        let s2 = make_signal("task-2", "compilation error", EvolutionAction::AutoFix);

        detector.ingest_signal(&s1).unwrap();
        let actionable = detector.ingest_signal(&s2).unwrap();

        assert_eq!(actionable.len(), 1);
        assert_eq!(actionable[0].occurrence_count, 2);
        assert_eq!(actionable[0].affected_tasks.len(), 2);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_actionable_patterns_threshold() {
        let tmp_dir = temp_dir().join(format!("zn_pattern_thresh_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut detector = ErrorPatternDetector::new(&tmp_dir, 3).unwrap();

        for i in 1..=2 {
            let s = make_signal(
                &format!("task-{}", i),
                "test failure in module X",
                EvolutionAction::AutoFix,
            );
            detector.ingest_signal(&s).unwrap();
        }

        assert!(detector.actionable_patterns().is_empty()); // 2 < 3

        let s3 = make_signal(
            "task-3",
            "test failure in module X",
            EvolutionAction::AutoFix,
        );
        detector.ingest_signal(&s3).unwrap();

        assert_eq!(detector.actionable_patterns().len(), 1);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_generate_remediation() {
        let tmp_dir = temp_dir().join(format!("zn_pattern_remed_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut detector = ErrorPatternDetector::new(&tmp_dir, 2).unwrap();

        let s1 = make_signal("task-1", "panic in tokio runtime", EvolutionAction::AutoFix);
        let s2 = make_signal("task-2", "panic in tokio runtime", EvolutionAction::AutoFix);

        detector.ingest_signal(&s1).unwrap();
        detector.ingest_signal(&s2).unwrap();

        let patterns = detector.actionable_patterns();
        assert_eq!(patterns.len(), 1);

        let remediation = detector.generate_remediation(&patterns[0]);
        assert!(remediation.is_some());
        let cand = remediation.unwrap();
        assert_eq!(cand.kind, EvolutionKind::AutoFix);
        assert!(cand.confidence > 0.0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_state_persistence() {
        let tmp_dir = temp_dir().join(format!("zn_pattern_persist_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        {
            let mut detector = ErrorPatternDetector::new(&tmp_dir, 1).unwrap();
            let s = make_signal("task-1", "timeout error", EvolutionAction::AutoImprove);
            detector.ingest_signal(&s).unwrap();
        } // dropped, state saved

        {
            let detector = ErrorPatternDetector::new(&tmp_dir, 1).unwrap();
            assert_eq!(detector.all_patterns().len(), 1);
            assert_eq!(detector.get_stats().total_signals, 1);
        }

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_promote_skill_signals_not_ingested() {
        let tmp_dir = temp_dir().join(format!("zn_pattern_promote_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut detector = ErrorPatternDetector::new(&tmp_dir, 1).unwrap();
        let s = make_signal("task-1", "high score", EvolutionAction::PromoteSkill);
        let actionable = detector.ingest_signal(&s).unwrap();
        assert!(actionable.is_empty()); // PromoteSkill is not a failure signal

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
