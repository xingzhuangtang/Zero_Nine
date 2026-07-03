//! Skill Evolver — closed-loop evolution from signal to promotion.
//!
//! Processes `EvolutionSignal` entries through a three-stage pipeline:
//! 1. **Evolve**: generate a candidate patch from the signal.
//! 2. **Verify**: dry-run the candidate against historical data.
//! 3. **Promote**: inject the verified candidate into the skill library.

use anyhow::{anyhow, Result};
use chrono::Utc;
use zn_types::{
    EvolutionAction, EvolutionCandidate, EvolutionKind, EvolutionSignal, SkillBundle, SkillVersion,
};

use crate::skill_registry::SkillRegistry;

/// Generates and promotes skill patches from evolution signals.
pub struct SkillEvolver {
    registry: SkillRegistry,
}

impl SkillEvolver {
    /// Create a new evolver with the given skill registry.
    pub fn new(registry: SkillRegistry) -> Self {
        Self { registry }
    }

    /// Process an evolution signal and produce a candidate patch.
    ///
    /// Only signals with actionable `proposed_action` (AutoFix, AutoImprove,
    /// PromoteSkill) generate candidates.
    pub fn evolve(&self, signal: &EvolutionSignal) -> Result<Option<EvolutionCandidate>> {
        let (kind, reason) = match &signal.proposed_action {
            EvolutionAction::AutoFix => (
                EvolutionKind::AutoFix,
                format!(
                    "Signal from {} (confidence {:.2}): {}",
                    signal.source.name(),
                    signal.confidence,
                    signal.notes.first().map(|s| s.as_str()).unwrap_or("")
                ),
            ),
            EvolutionAction::AutoImprove => (
                EvolutionKind::AutoImprove,
                format!(
                    "Improvement signal (score {:.2}): {}",
                    signal.score,
                    signal.notes.first().map(|s| s.as_str()).unwrap_or("")
                ),
            ),
            EvolutionAction::PromoteSkill => (
                EvolutionKind::AutoLearn,
                format!(
                    "Promotion signal (score {:.2}): high quality execution",
                    signal.score
                ),
            ),
            _ => return Ok(None), // No actionable evolution for this signal.
        };

        // The patch is a structured improvement instruction.
        let patch = format!(
            "TASK: {}\nSIGNAL: {:?}\nSCORE: {:.2}\nDECISION: {}\nNOTES: {}",
            signal.task_id,
            signal.proposed_action,
            signal.score,
            signal.decision,
            signal.notes.join("; ")
        );

        let confidence = signal.confidence;
        Ok(Some(EvolutionCandidate {
            source_skill: signal.task_id.clone(),
            kind,
            reason,
            patch,
            confidence,
            created_at: Utc::now(),
        }))
    }

    /// Verify a candidate by checking it has sufficient confidence
    /// and a non-empty patch.
    ///
    /// In a full system this would dry-run the candidate in a sandbox.
    /// For now, we validate confidence >= 0.50 and patch is non-empty.
    pub fn verify_candidate(&self, candidate: &EvolutionCandidate) -> Result<bool> {
        if candidate.confidence < 0.50 {
            return Ok(false);
        }
        if candidate.patch.is_empty() {
            return Err(anyhow!("candidate patch is empty"));
        }
        Ok(true)
    }

    /// Promote a verified candidate to the skill library.
    ///
    /// Registers a new skill version in the registry and returns the
    /// resulting skill bundle.
    pub fn promote(&mut self, candidate: &EvolutionCandidate) -> Result<SkillBundle> {
        let bundle = SkillBundle {
            id: format!("evolved-{}", &candidate.source_skill),
            name: format!("Evolved from {}", &candidate.source_skill),
            version: SkillVersion::default(),
            description: candidate.reason.clone(),
            applicable_scenarios: vec![candidate.source_skill.clone()],
            preconditions: vec![],
            disabled_conditions: vec![],
            risk_level: zn_types::ActionRiskLevel::Medium,
            skill_chain: vec![],
            artifacts: vec![],
            usage_count: 0,
            success_rate: 0.0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Register in the skill registry for version tracking.
        self.registry.register_version(
            &bundle.name,
            bundle.version.clone(),
            "evolved",
            bundle.success_rate,
            &bundle.id,
        )?;

        Ok(bundle)
    }

    /// Run the full closed loop: detect → evolve → verify → promote.
    pub fn run_closed_loop(&mut self, signal: &EvolutionSignal) -> Result<Option<SkillBundle>> {
        let candidate = self.evolve(signal)?;
        let candidate = match candidate {
            Some(c) => c,
            None => return Ok(None),
        };

        if !self.verify_candidate(&candidate)? {
            return Ok(None);
        }

        let bundle = self.promote(&candidate)?;
        Ok(Some(bundle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use zn_types::{EvolutionAction, EvolutionSignal, EvolutionSignalSource};

    fn make_registry() -> SkillRegistry {
        let path = std::env::temp_dir().join(format!("test-registry-{}", uuid::Uuid::new_v4()));
        SkillRegistry::new(path).unwrap()
    }

    fn make_signal(action: EvolutionAction, confidence: f32) -> EvolutionSignal {
        EvolutionSignal {
            id: "sig-1".to_string(),
            task_id: "task-1".to_string(),
            score: 0.75,
            decision: "improve".to_string(),
            notes: vec!["need better tests".to_string()],
            source: EvolutionSignalSource::ExecutionReport,
            detected_at: Utc::now(),
            confidence,
            proposed_action: action,
        }
    }

    #[test]
    fn test_evolve_auto_fix() {
        let evolver = SkillEvolver::new(make_registry());
        let signal = make_signal(EvolutionAction::AutoFix, 0.85);
        let candidate = evolver.evolve(&signal).unwrap();
        assert!(candidate.is_some());
        let c = candidate.unwrap();
        assert_eq!(c.kind, EvolutionKind::AutoFix);
        assert_eq!(c.confidence, 0.85);
    }

    #[test]
    fn test_evolve_auto_improve() {
        let evolver = SkillEvolver::new(make_registry());
        let signal = make_signal(EvolutionAction::AutoImprove, 0.70);
        let candidate = evolver.evolve(&signal).unwrap();
        assert!(candidate.is_some());
        assert_eq!(candidate.unwrap().kind, EvolutionKind::AutoImprove);
    }

    #[test]
    fn test_evolve_promote_skill() {
        let evolver = SkillEvolver::new(make_registry());
        let signal = make_signal(EvolutionAction::PromoteSkill, 0.90);
        let candidate = evolver.evolve(&signal).unwrap();
        assert!(candidate.is_some());
        assert_eq!(candidate.unwrap().kind, EvolutionKind::AutoLearn);
    }

    #[test]
    fn test_evolve_no_action() {
        let evolver = SkillEvolver::new(make_registry());
        let signal = make_signal(EvolutionAction::NoAction, 0.50);
        let candidate = evolver.evolve(&signal).unwrap();
        assert!(candidate.is_none());
    }

    #[test]
    fn test_verify_candidate_low_confidence() {
        let evolver = SkillEvolver::new(make_registry());
        let signal = make_signal(EvolutionAction::AutoFix, 0.30);
        let candidate = evolver.evolve(&signal).unwrap().unwrap();
        assert!(!evolver.verify_candidate(&candidate).unwrap());
    }

    #[test]
    fn test_run_closed_loop_success() {
        let mut evolver = SkillEvolver::new(make_registry());
        let signal = make_signal(EvolutionAction::AutoImprove, 0.75);
        let bundle = evolver.run_closed_loop(&signal).unwrap();
        assert!(bundle.is_some());
        assert!(bundle.unwrap().name.starts_with("Evolved from"));
    }

    #[test]
    fn test_run_closed_loop_no_action() {
        let mut evolver = SkillEvolver::new(make_registry());
        let signal = make_signal(EvolutionAction::NoAction, 0.50);
        let bundle = evolver.run_closed_loop(&signal).unwrap();
        assert!(bundle.is_none());
    }

    #[test]
    fn test_run_closed_loop_low_confidence() {
        let mut evolver = SkillEvolver::new(make_registry());
        let signal = make_signal(EvolutionAction::AutoFix, 0.30);
        let bundle = evolver.run_closed_loop(&signal).unwrap();
        assert!(bundle.is_none());
    }
}
