use chrono::Utc;
use zn_types::{EvidenceStatus, EvolutionCandidate, EvolutionKind, ExecutionOutcome, ExecutionReport, SkillEvaluation, VerdictStatus};

// Re-export scorer module
pub mod scorer;
pub use scorer::SkillScorer;

// Re-export distiller module
pub mod distiller;
pub use distiller::SkillDistiller;

// Re-export reward model module
pub mod reward;
pub use reward::{RewardModel, RewardBreakdown};

// Re-export curriculum module
pub mod curriculum;
pub use curriculum::{CurriculumManager, CurriculumStats, OptimalTaskRecommendation};

// Re-export belief module
pub mod belief;
pub use belief::{BeliefTracker, BeliefDecision, RecommendedAction, update_belief_from_report};

// Integration engine - 三系统联动
pub mod integration_engine;
pub use integration_engine::{IntegrationEngine, IntegratedDecision, EngineSnapshot, DecisionReasoning};

// AI Client - 外部 AI API 客户端
pub mod ai_client;
pub use ai_client::{
    AIClient, AIClientConfig, AIProvider, AIRequest, AIResponse, AIMessage, MessageRole, TokenUsage,
    UserFeedbackCollector, UserFeedbackEntry, FeedbackStats, create_feedback_collector,
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
        latency_ms: report.execution_time_ms,
        token_cost: report.token_count,
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
        // Confidence based on evidence completeness and verdict strength
        let evidence_total = report.evidence.iter().filter(|e| e.required).count() as f32;
        let evidence_collected = report
            .evidence
            .iter()
            .filter(|e| e.required && e.status == EvidenceStatus::Collected)
            .count() as f32;
        let evidence_ratio = if evidence_total > 0.0 {
            evidence_collected / evidence_total
        } else {
            1.0
        };
        let verdict_bonus = match (&report.review_verdict, &report.verification_verdict) {
            (Some(r), Some(v))
                if r.status == VerdictStatus::Passed && v.status == VerdictStatus::Passed =>
            {
                0.15
            }
            _ => 0.0,
        };
        let confidence: f32 = (0.60f32 + evidence_ratio * 0.25 + verdict_bonus).min(0.95);

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
            confidence,
            created_at: Utc::now(),
        })
    } else {
        // Confidence based on diagnostic richness
        let has_failure_summary = report.failure_summary.as_ref().map_or(false, |s| !s.is_empty());
        let has_agent_runs = !report.agent_runs.is_empty();
        let diagnostic_bonus = if has_failure_summary { 0.10 } else { 0.0 }
            + if has_agent_runs { 0.05 } else { 0.0 };
        let confidence: f32 = (0.55f32 + diagnostic_bonus).min(0.85);

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
            confidence,
            created_at: Utc::now(),
        })
    }
}

/// Structured remediation guidance extracted from consumed evolution candidates
#[derive(Debug, Clone)]
pub struct RemediationGuidance {
    pub fix_instructions: Vec<String>,
    pub improve_patterns: Vec<String>,
    pub confidence: f32,
}

/// Consume pending evolution candidates for a task and extract actionable guidance
pub fn consume_candidates(project_root: &std::path::Path, task_id: &str) -> Option<RemediationGuidance> {
    use std::fs;
    let candidates_dir = project_root.join(".zero_nine/evolve/candidates");
    if !candidates_dir.exists() {
        return None;
    }

    let mut fix_instructions = Vec::new();
    let mut improve_patterns = Vec::new();
    let mut max_confidence = 0.0f32;
    let mut consumed_any = false;

    for entry in fs::read_dir(&candidates_dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let filename = path.file_stem()?.to_string_lossy();
        if !filename.starts_with(&format!("{}-", task_id)) {
            continue;
        }
        // Already consumed
        if filename.ends_with(".consumed") {
            continue;
        }

        let content = fs::read_to_string(&path).ok()?;
        let candidate: EvolutionCandidate = serde_json::from_str(&content).ok()?;
        max_confidence = max_confidence.max(candidate.confidence);

        match &candidate.kind {
            EvolutionKind::AutoFix => {
                fix_instructions.push(candidate.patch.clone());
            }
            EvolutionKind::AutoImprove | EvolutionKind::AutoLearn => {
                improve_patterns.push(candidate.patch.clone());
            }
        }

        // Mark as consumed by renaming (skip if target already exists to avoid races)
        let new_name = format!("{}.consumed.json", filename);
        let consumed_path = path.parent()?.join(&new_name);
        if consumed_path.exists() {
            continue;
        }
        let _ = fs::rename(&path, &consumed_path);
        consumed_any = true;
    }

    if !consumed_any {
        return None;
    }

    Some(RemediationGuidance {
        fix_instructions,
        improve_patterns,
        confidence: max_confidence,
    })
}
