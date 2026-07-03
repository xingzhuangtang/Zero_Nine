//! Signal Detector — extract evolution signals from execution reports.
//!
//! Analyzes `ExecutionReport` instances and produces `EvolutionSignal`
//! entries that drive the closed-loop skill evolution system.

use chrono::Utc;
use zn_types::{
    EvolutionAction, EvolutionSignal, EvolutionSignalSource, ExecutionOutcome, ExecutionReport,
    ExternalEvent,
};

/// Thresholds for signal detection.
pub struct SignalDetector {
    /// Signals with a score below this threshold are ignored.
    pub score_threshold: f32,
    /// Minimum confidence for a signal to be actionable.
    pub confidence_threshold: f32,
}

impl SignalDetector {
    pub fn new(score_threshold: f32, confidence_threshold: f32) -> Self {
        Self {
            score_threshold,
            confidence_threshold,
        }
    }

    /// Analyze an execution report and produce evolution signals.
    ///
    /// Detection rules:
    /// 1. Failed execution → `EvolutionAction::AutoFix` with high confidence.
    /// 2. Completed but low score → `EvolutionAction::AutoImprove`.
    /// 3. Successful with high score → `EvolutionAction::PromoteSkill`.
    pub fn detect(&self, report: &ExecutionReport) -> Vec<EvolutionSignal> {
        let mut signals = Vec::new();

        let score = self.compute_score(report);

        // Rule 1: Execution failure → AutoFix
        if let ExecutionOutcome::RetryableFailure | ExecutionOutcome::Escalated = report.outcome {
            signals.push(EvolutionSignal {
                id: uuid::Uuid::new_v4().to_string(),
                task_id: report.task_id.clone(),
                score,
                decision: "fix".to_string(),
                notes: vec![format!(
                    "Execution failed: {:?}",
                    report.failure_classification
                )],
                source: EvolutionSignalSource::ExecutionReport,
                detected_at: Utc::now(),
                confidence: 0.85,
                proposed_action: EvolutionAction::AutoFix,
            });
        }

        // Rule 2: Completed but score below threshold → AutoImprove
        if matches!(report.outcome, ExecutionOutcome::Completed) && score < self.score_threshold {
            signals.push(EvolutionSignal {
                id: uuid::Uuid::new_v4().to_string(),
                task_id: report.task_id.clone(),
                score,
                decision: "improve".to_string(),
                notes: vec![format!(
                    "Low execution score: {score:.2} < {}",
                    self.score_threshold
                )],
                source: EvolutionSignalSource::ExecutionReport,
                detected_at: Utc::now(),
                confidence: 0.70,
                proposed_action: EvolutionAction::AutoImprove,
            });
        }

        // Rule 3: High score → PromoteSkill
        if matches!(report.outcome, ExecutionOutcome::Completed) && score >= 0.90 && report.success
        {
            signals.push(EvolutionSignal {
                id: uuid::Uuid::new_v4().to_string(),
                task_id: report.task_id.clone(),
                score,
                decision: "promote".to_string(),
                notes: vec![format!("High execution score: {score:.2}")],
                source: EvolutionSignalSource::ExecutionReport,
                detected_at: Utc::now(),
                confidence: 0.90,
                proposed_action: EvolutionAction::PromoteSkill,
            });
        }

        // Apply confidence threshold filter.
        signals
            .into_iter()
            .filter(|s| s.confidence >= self.confidence_threshold)
            .collect()
    }

    /// Analyze an external event and produce evolution signals.
    ///
    /// Detection rules:
    /// 1. CI compilation/test failure → `AutoFix` (high confidence)
    /// 2. Runtime panic/crash → `AutoFix` (high confidence)
    /// 3. User-reported issue → `AutoImprove` (medium confidence)
    /// 4. Timeout → `AutoImprove`
    /// 5. Unknown → `AutoLearn`
    pub fn detect_from_external(&self, event: &ExternalEvent) -> Vec<EvolutionSignal> {
        let (confidence, action) = match event.event_type.as_str() {
            "compilation_error" | "test_failure" => (0.80, EvolutionAction::AutoFix),
            "panic" | "segfault" => (0.90, EvolutionAction::AutoFix),
            "timeout" => (0.70, EvolutionAction::AutoImprove),
            "user_report" => (0.55, EvolutionAction::AutoImprove),
            _ => (0.50, EvolutionAction::AutoLearn),
        };

        if confidence < self.confidence_threshold {
            return Vec::new();
        }

        let task_id = event
            .task_id
            .clone()
            .unwrap_or_else(|| format!("external-{}", &event.id[..8.min(event.id.len())]));

        let body_preview = event.body.chars().take(200).collect::<String>();
        vec![EvolutionSignal {
            id: uuid::Uuid::new_v4().to_string(),
            task_id,
            score: 1.0 - confidence,
            decision: event.event_type.clone(),
            notes: vec![format!("{}: {}", event.source, event.title), body_preview],
            source: EvolutionSignalSource::ExternalEvent,
            detected_at: Utc::now(),
            confidence,
            proposed_action: action,
        }]
    }

    /// Merge signals from multiple sources, keeping the highest-confidence
    /// signal per task_id.
    pub fn merge_signals(&self, signals: Vec<EvolutionSignal>) -> Vec<EvolutionSignal> {
        use std::collections::HashMap;
        let mut best: HashMap<String, EvolutionSignal> = HashMap::new();
        for signal in signals {
            let tid = signal.task_id.clone();
            best.entry(tid)
                .and_modify(|existing| {
                    if signal.confidence > existing.confidence {
                        *existing = signal.clone();
                    }
                })
                .or_insert(signal);
        }
        best.into_values().collect()
    }

    /// Compute a composite score from the execution report.
    fn compute_score(&self, report: &ExecutionReport) -> f32 {
        if report.success {
            0.80 + if report.tests_passed { 0.10 } else { 0.0 }
                + if report.review_passed { 0.10 } else { 0.0 }
        } else {
            0.33
        }
    }
}

impl Default for SignalDetector {
    fn default() -> Self {
        Self {
            score_threshold: 0.60,
            confidence_threshold: 0.50,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(
        outcome: ExecutionOutcome,
        success: bool,
        tests_passed: bool,
        review_passed: bool,
    ) -> ExecutionReport {
        ExecutionReport {
            task_id: "task-1".to_string(),
            success,
            outcome,
            summary: "test".to_string(),
            details: vec![],
            tests_passed,
            review_passed,
            artifacts: vec![],
            generated_artifacts: vec![],
            evidence: vec![],
            follow_ups: vec![],
            workspace_record: None,
            finish_branch_result: None,
            finish_branch_automation: None,
            agent_runs: vec![],
            review_verdict: None,
            verification_verdict: None,
            verification_actions: vec![],
            verification_action_results: vec![],
            failure_summary: None,
            exit_code: 0,
            execution_time_ms: 0,
            token_count: 0,
            code_quality_score: 0.0,
            test_coverage: 0.0,
            user_feedback: None,
            failure_classification: None,
            tri_role_verdict: None,
            authorization_ticket_id: None,
            authorized_by: None,
            governance_summary: None,
        }
    }

    #[test]
    fn test_detect_success_high_score() {
        let detector = SignalDetector::default();
        let report = make_report(ExecutionOutcome::Completed, true, true, true);
        let signals = detector.detect(&report);
        assert!(signals
            .iter()
            .any(|s| s.proposed_action == EvolutionAction::PromoteSkill));
    }

    #[test]
    fn test_detect_failure() {
        let detector = SignalDetector::default();
        let report = make_report(ExecutionOutcome::RetryableFailure, false, false, false);
        let signals = detector.detect(&report);
        assert!(signals
            .iter()
            .any(|s| s.proposed_action == EvolutionAction::AutoFix));
    }

    #[test]
    fn test_detect_no_signal_for_partial_success() {
        let detector = SignalDetector::default();
        let report = make_report(ExecutionOutcome::Completed, true, false, false);
        let signals = detector.detect(&report);
        assert!(signals.is_empty());
    }

    #[test]
    fn test_merge_signals_keeps_highest_confidence() {
        let detector = SignalDetector::default();
        let s1 = EvolutionSignal {
            id: "s1".to_string(),
            task_id: "t1".to_string(),
            score: 0.5,
            decision: "fix".to_string(),
            notes: vec![],
            source: EvolutionSignalSource::ExecutionReport,
            detected_at: Utc::now(),
            confidence: 0.70,
            proposed_action: EvolutionAction::AutoFix,
        };
        let s2 = EvolutionSignal {
            confidence: 0.90,
            id: "s2".to_string(),
            ..s1.clone()
        };
        let merged = detector.merge_signals(vec![s1, s2]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].confidence, 0.90);
    }

    #[test]
    fn test_confidence_threshold_filter() {
        let detector = SignalDetector {
            confidence_threshold: 0.95,
            ..SignalDetector::default()
        };
        let report = make_report(ExecutionOutcome::RetryableFailure, false, false, false);
        let signals = detector.detect(&report);
        assert!(signals.is_empty());
    }

    #[test]
    fn test_detect_external_ci_failure() {
        let detector = SignalDetector::default();
        let event = ExternalEvent {
            id: "ci-001".to_string(),
            source: "ci_failure".to_string(),
            event_type: "compilation_error".to_string(),
            task_id: None,
            title: "Build failed on main".to_string(),
            body: "error[E0432]: unresolved import".to_string(),
            metadata: serde_json::json!({}),
            detected_at: Utc::now(),
        };
        let signals = detector.detect_from_external(&event);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].proposed_action, EvolutionAction::AutoFix);
        assert_eq!(signals[0].confidence, 0.80);
    }

    #[test]
    fn test_detect_external_panic() {
        let detector = SignalDetector::default();
        let event = ExternalEvent {
            id: "crash-001".to_string(),
            source: "runtime".to_string(),
            event_type: "panic".to_string(),
            task_id: Some("task-42".to_string()),
            title: "Panic in module".to_string(),
            body: "thread panicked".to_string(),
            metadata: serde_json::json!({}),
            detected_at: Utc::now(),
        };
        let signals = detector.detect_from_external(&event);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].proposed_action, EvolutionAction::AutoFix);
        assert_eq!(signals[0].confidence, 0.90);
        assert_eq!(signals[0].task_id, "task-42");
    }

    #[test]
    fn test_detect_external_user_report_below_threshold() {
        let detector = SignalDetector {
            confidence_threshold: 0.60,
            ..SignalDetector::default()
        };
        let event = ExternalEvent {
            id: "issue-001".to_string(),
            source: "github".to_string(),
            event_type: "user_report".to_string(),
            task_id: None,
            title: "Feature request".to_string(),
            body: "".to_string(),
            metadata: serde_json::json!({}),
            detected_at: Utc::now(),
        };
        let signals = detector.detect_from_external(&event);
        assert!(signals.is_empty()); // 0.55 < 0.60 threshold
    }

    #[test]
    fn test_detect_external_unknown_type() {
        let detector = SignalDetector::default();
        let event = ExternalEvent {
            id: "unknown-001".to_string(),
            source: "mystery".to_string(),
            event_type: "mystery_event".to_string(),
            task_id: None,
            title: "Something happened".to_string(),
            body: "".to_string(),
            metadata: serde_json::json!({}),
            detected_at: Utc::now(),
        };
        let signals = detector.detect_from_external(&event);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].proposed_action, EvolutionAction::AutoLearn);
        assert_eq!(signals[0].confidence, 0.50);
    }
}
