use chrono::Utc;
use zn_types::{
    EvidenceStatus, EvolutionCandidate, EvolutionKind, ExecutionOutcome, ExecutionReport,
    SkillEvaluation, VerdictStatus,
};

pub fn evaluate(report: &ExecutionReport) -> SkillEvaluation {
    let collected_required = report
        .evidence
        .iter()
        .filter(|item| item.required && item.status == EvidenceStatus::Collected)
        .count();
    let missing_required = report
        .evidence
        .iter()
        .filter(|item| item.required && item.status != EvidenceStatus::Collected)
        .count();
    let review_status = report
        .review_verdict
        .as_ref()
        .map(|item| item.status.clone())
        .unwrap_or(VerdictStatus::Warning);
    let verification_status = report
        .verification_verdict
        .as_ref()
        .map(|item| item.status.clone())
        .unwrap_or(VerdictStatus::Warning);

    let score = match report.outcome {
        ExecutionOutcome::Completed
            if report.success
                && review_status == VerdictStatus::Passed
                && verification_status == VerdictStatus::Passed
                && missing_required == 0 => 0.97,
        ExecutionOutcome::Completed if report.success => 0.84,
        ExecutionOutcome::Completed => 0.61,
        ExecutionOutcome::RetryableFailure => 0.56,
        ExecutionOutcome::Blocked => 0.42,
        ExecutionOutcome::Escalated => 0.33,
    };

    let notes = format!(
        "{} | outcome={:?} | required_evidence_collected={} | required_evidence_missing={} | failure_summary={}",
        report.summary,
        report.outcome,
        collected_required,
        missing_required,
        report
            .failure_summary
            .clone()
            .unwrap_or_else(|| "none".to_string())
    );

    SkillEvaluation {
        skill_name: if report.tests_passed && missing_required == 0 {
            "guarded-execution".to_string()
        } else {
            "evidence-driven-verification".to_string()
        },
        task_type: "task_execution".to_string(),
        latency_ms: 150,
        token_cost: 0,
        score,
        notes,
    }
}

pub fn propose_candidate(report: &ExecutionReport) -> Option<EvolutionCandidate> {
    let missing_keys = report
        .evidence
        .iter()
        .filter(|item| item.required && item.status != EvidenceStatus::Collected)
        .map(|item| item.key.clone())
        .collect::<Vec<_>>();

    if report.outcome == ExecutionOutcome::Completed && report.success {
        Some(EvolutionCandidate {
            source_skill: "guarded-execution".to_string(),
            kind: EvolutionKind::AutoImprove,
            reason: "Successful execution with structured verdicts and evidence should be promoted as a reusable pattern.".to_string(),
            patch: format!(
                "Promote task {} evidence bundle and verdict rubric into the shared skill library. Evidence keys: {}.",
                report.task_id,
                if missing_keys.is_empty() {
                    "none".to_string()
                } else {
                    missing_keys.join(", ")
                }
            ),
            confidence: 0.76,
            created_at: Utc::now(),
        })
    } else {
        Some(EvolutionCandidate {
            source_skill: "evidence-driven-verification".to_string(),
            kind: match report.outcome {
                ExecutionOutcome::RetryableFailure => EvolutionKind::AutoImprove,
                _ => EvolutionKind::AutoFix,
            },
            reason: format!(
                "Execution for task {} did not fully pass the structured gates and needs a corrective skill patch.",
                report.task_id
            ),
            patch: format!(
                "Add remediation guidance for outcome {:?} on task {}. Missing required evidence keys: {}. Failure summary: {}.",
                report.outcome,
                report.task_id,
                if missing_keys.is_empty() {
                    "none".to_string()
                } else {
                    missing_keys.join(", ")
                },
                report
                    .failure_summary
                    .clone()
                    .unwrap_or_else(|| "none".to_string())
            ),
            confidence: 0.83,
            created_at: Utc::now(),
        })
    }
}
