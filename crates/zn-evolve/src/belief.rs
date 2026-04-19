//! Belief Tracker - Online belief state management
//!
//! This module provides:
//! - Belief state creation and updates
//! - Confidence tracking
//! - Evidence collection
//! - Question resolution

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use zn_types::{BeliefState, WeightedEvidence};

/// Belief tracker for managing belief states over time
pub struct BeliefTracker {
    states: Vec<BeliefState>,
    belief_file: PathBuf,
}

impl BeliefTracker {
    /// Create a new BeliefTracker
    pub fn new(belief_file: PathBuf) -> Result<Self> {
        let mut tracker = Self {
            states: Vec::new(),
            belief_file,
        };
        tracker.load_existing_beliefs()?;
        Ok(tracker)
    }

    /// Load existing beliefs from file
    pub fn load_existing_beliefs(&mut self) -> Result<()> {
        if !self.belief_file.exists() {
            return Ok(());
        }

        let file = fs::File::open(&self.belief_file)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(state) = serde_json::from_str::<BeliefState>(&line) {
                self.states.push(state);
            }
        }

        Ok(())
    }

    /// Create a new belief state from a goal
    pub fn create_belief(&mut self, goal: &str, initial_hypothesis: &str) -> BeliefState {
        let state = BeliefState {
            goal: goal.to_string(),
            current_hypothesis: initial_hypothesis.to_string(),
            confidence: 0.5, // Start with neutral confidence
            evidence_for: Vec::new(),
            evidence_against: Vec::new(),
            open_questions: Vec::new(),
            next_experiment: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            confidence_history: Vec::new(),
            hypothesis_history: Vec::new(),
        };

        self.states.push(state.clone());
        state
    }

    /// Update the latest belief state with Bayesian update
    pub fn update_belief(&mut self, success: bool, evidence: &str, new_confidence: Option<f32>) -> Option<&mut BeliefState> {
        if let Some(state) = self.states.last_mut() {
            // Record confidence before update
            state.confidence_history.push(state.confidence);
            if state.confidence_history.len() > 20 {
                state.confidence_history.remove(0);
            }

            // Add evidence with weight based on outcome
            let weight = if success { 0.7 } else { 0.5 };
            let credibility = 0.8; // Default credibility for execution results

            let weighted_evidence = WeightedEvidence::new(evidence, weight, credibility);

            if success {
                state.evidence_for.push(weighted_evidence);
            } else {
                state.evidence_against.push(weighted_evidence);
            }

            // Bayesian update or use provided confidence
            if let Some(conf) = new_confidence {
                state.confidence = conf;
            } else {
                // Inline bayesian update to avoid borrow checker issue
                let prior = state.confidence;
                let likelihood_success = 0.8;
                let likelihood_failure = 0.3;

                state.confidence = if success {
                    let numerator = prior * likelihood_success;
                    let denominator = numerator + (1.0 - prior) * (1.0 - likelihood_success);
                    if denominator > 0.001 {
                        (numerator / denominator).clamp(0.0, 1.0)
                    } else {
                        prior
                    }
                } else {
                    let numerator = prior * likelihood_failure;
                    let denominator = numerator + (1.0 - prior) * (1.0 - likelihood_failure);
                    if denominator > 0.001 {
                        (numerator / denominator).clamp(0.0, 1.0)
                    } else {
                        prior
                    }
                };
            }

            state.updated_at = Utc::now();
            Some(state)
        } else {
            None
        }
    }

    /// Update belief with weighted evidence
    pub fn update_belief_with_weighted_evidence(
        &mut self,
        evidence: &WeightedEvidence,
        is_supporting: bool,
    ) -> Option<&mut BeliefState> {
        if let Some(state) = self.states.last_mut() {
            state.confidence_history.push(state.confidence);

            if is_supporting {
                state.evidence_for.push(evidence.clone());
            } else {
                state.evidence_against.push(evidence.clone());
            }

            // Calculate net confidence shift from evidence
            let adjusted_weight = evidence.adjusted_weight();
            let shift = if is_supporting {
                adjusted_weight * 0.1
            } else {
                -adjusted_weight * 0.1
            };

            state.confidence = (state.confidence + shift).clamp(0.0, 1.0);
            state.updated_at = Utc::now();
            Some(state)
        } else {
            None
        }
    }

    /// Add a question to the current belief state
    pub fn add_question(&mut self, question: &str) -> Result<()> {
        if let Some(state) = self.states.last_mut() {
            state.add_question(question.to_string());
            self.save()?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("No active belief state"))
        }
    }

    /// Resolve a question from the current belief state
    pub fn resolve_question(&mut self, question: &str) -> Result<()> {
        if let Some(state) = self.states.last_mut() {
            state.resolve_question(question);
            self.save()?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("No active belief state"))
        }
    }

    /// Set the next experiment to run
    pub fn set_next_experiment(&mut self, experiment: &str) -> Result<()> {
        if let Some(state) = self.states.last_mut() {
            state.next_experiment = experiment.to_string();
            self.save()?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("No active belief state"))
        }
    }

    /// Update hypothesis based on new information
    pub fn update_hypothesis(&mut self, new_hypothesis: &str) -> Result<()> {
        if let Some(state) = self.states.last_mut() {
            // Record old hypothesis in history
            state.hypothesis_history.push(state.current_hypothesis.clone());
            if state.hypothesis_history.len() > 10 {
                state.hypothesis_history.remove(0);
            }

            state.current_hypothesis = new_hypothesis.to_string();
            state.updated_at = Utc::now();
            self.save()?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("No active belief state"))
        }
    }

    /// Get belief-driven decision recommendation
    /// Returns (should_continue, should_change_hypothesis, should_run_experiment)
    pub fn get_decision(&self) -> BeliefDecision {
        let Some(state) = self.states.last() else {
            return BeliefDecision::default();
        };

        let confidence = state.confidence;
        let evidence_for_count = state.evidence_for.len();
        let evidence_against_count = state.evidence_against.len();
        let open_questions_count = state.open_questions.len();

        // Calculate evidence balance
        let for_weight: f32 = state.evidence_for.iter().map(|e| e.adjusted_weight()).sum();
        let against_weight: f32 = state.evidence_against.iter().map(|e| e.adjusted_weight()).sum();
        let evidence_balance = for_weight - against_weight;

        // Confidence trend
        let confidence_trend = self.get_confidence_trend();
        let is_confidence_increasing = confidence_trend.len() >= 2 && confidence_trend[0] > confidence_trend[confidence_trend.len() - 1];

        // Decision logic based on Harness Engineering principles
        let should_continue = confidence > 0.7 && evidence_balance > 0.3;
        let should_change_hypothesis = confidence < 0.3 || evidence_against_count > evidence_for_count * 2;
        let should_run_experiment = confidence > 0.4 && confidence < 0.8 && open_questions_count > 0;
        let should_escalate = confidence < 0.2 || (evidence_against_count > 3 && evidence_for_count < 2);

        BeliefDecision {
            confidence,
            evidence_balance,
            is_confidence_increasing,
            should_continue,
            should_change_hypothesis,
            should_run_experiment,
            should_escalate,
            recommended_action: self.recommend_action(confidence, evidence_balance, open_questions_count),
        }
    }

    /// Recommend action based on belief state
    fn recommend_action(&self, confidence: f32, evidence_balance: f32, open_questions: usize) -> RecommendedAction {
        if confidence > 0.85 && evidence_balance > 0.5 {
            RecommendedAction::ProceedToExecution
        } else if confidence < 0.3 {
            RecommendedAction::ReconsiderHypothesis
        } else if open_questions > 2 {
            RecommendedAction::AnswerQuestions
        } else if confidence > 0.6 {
            RecommendedAction::RunVerification
        } else {
            RecommendedAction::GatherMoreEvidence
        }
    }

    /// Get the current belief state
    pub fn get_current_belief(&self) -> Option<&BeliefState> {
        self.states.last()
    }

    /// Get belief history
    pub fn get_history(&self) -> &[BeliefState] {
        &self.states
    }

    /// Get confidence trend (last 5 states)
    pub fn get_confidence_trend(&self) -> Vec<f32> {
        self.states
            .iter()
            .rev()
            .take(5)
            .map(|s| s.confidence)
            .collect()
    }

    /// Check if confidence is increasing
    pub fn is_confidence_increasing(&self) -> bool {
        let trend = self.get_confidence_trend();
        if trend.len() < 2 {
            return true;
        }
        trend[0] >= trend[trend.len() - 1]
    }

    /// Get summary of current belief
    pub fn get_summary(&self) -> Option<BeliefSummary> {
        self.states.last().map(|state| BeliefSummary {
            goal: state.goal.clone(),
            hypothesis: state.current_hypothesis.clone(),
            confidence: state.confidence,
            evidence_for_count: state.evidence_for.len() as u32,
            evidence_against_count: state.evidence_against.len() as u32,
            open_questions_count: state.open_questions.len() as u32,
            is_confident: state.confidence > 0.8,
            is_confused: state.confidence < 0.3,
        })
    }

    /// Save all beliefs to file
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.belief_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut content = String::new();
        for state in &self.states {
            let line = serde_json::to_string(state)?;
            content.push_str(&line);
            content.push('\n');
        }

        // Use fs::write which handles creation and writing atomically
        fs::write(&self.belief_file, &content)?;

        Ok(())
    }

    /// Clear old beliefs (keep only last 10)
    pub fn prune_old(&mut self) {
        if self.states.len() > 10 {
            let drain_idx = self.states.len() - 10;
            self.states.drain(0..drain_idx);
        }
    }
}

/// Summary of current belief state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefSummary {
    pub goal: String,
    pub hypothesis: String,
    pub confidence: f32,
    pub evidence_for_count: u32,
    pub evidence_against_count: u32,
    pub open_questions_count: u32,
    pub is_confident: bool,
    pub is_confused: bool,
}

/// Belief-driven decision recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefDecision {
    pub confidence: f32,
    pub evidence_balance: f32,
    pub is_confidence_increasing: bool,
    pub should_continue: bool,
    pub should_change_hypothesis: bool,
    pub should_run_experiment: bool,
    pub should_escalate: bool,
    pub recommended_action: RecommendedAction,
}

impl Default for BeliefDecision {
    fn default() -> Self {
        Self {
            confidence: 0.5,
            evidence_balance: 0.0,
            is_confidence_increasing: true,
            should_continue: false,
            should_change_hypothesis: false,
            should_run_experiment: false,
            should_escalate: false,
            recommended_action: RecommendedAction::GatherMoreEvidence,
        }
    }
}

/// Recommended action from belief decision
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RecommendedAction {
    /// Proceed to execution phase
    ProceedToExecution,
    /// Reconsider the current hypothesis
    ReconsiderHypothesis,
    /// Answer open questions first
    AnswerQuestions,
    /// Run verification experiment
    RunVerification,
    /// Gather more evidence
    GatherMoreEvidence,
    /// Escalate to human
    EscalateToHuman,
}

/// Create default belief tracker in .zero_nine/evolve directory
pub fn create_default_belief_tracker(project_root: &Path) -> Result<BeliefTracker> {
    let belief_file = project_root
        .join(".zero_nine/evolve/belief_states.ndjson");
    BeliefTracker::new(belief_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_belief_tracker_lifecycle() {
        let tmp_file = temp_dir().join("test_belief.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut tracker = BeliefTracker::new(tmp_file.clone()).unwrap();

        // Create initial belief
        tracker.create_belief(
            "Build a search feature",
            "Users need fuzzy search",
        );

        // Update with positive evidence
        tracker.update_belief(true, "User test passed", None);
        tracker.update_belief(true, "Performance benchmark met", None);

        // Update with negative evidence
        tracker.update_belief(false, "Edge case failed on empty input", None);

        let summary = tracker.get_summary().unwrap();
        assert_eq!(summary.evidence_for_count, 2);
        assert_eq!(summary.evidence_against_count, 1);
        assert!(summary.confidence > 0.4);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_question_management() {
        let tmp_file = temp_dir().join("test_questions.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut tracker = BeliefTracker::new(tmp_file.clone()).unwrap();

        tracker.create_belief("Goal", "Hypothesis");

        tracker.add_question("What about edge cases?").unwrap();
        tracker.add_question("How to handle errors?").unwrap();

        let summary = tracker.get_summary().unwrap();
        assert_eq!(summary.open_questions_count, 2);

        tracker.resolve_question("What about edge cases?").unwrap();

        let summary = tracker.get_summary().unwrap();
        assert_eq!(summary.open_questions_count, 1);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_confidence_trend() {
        let tmp_file = temp_dir().join("test_trend.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut tracker = BeliefTracker::new(tmp_file.clone()).unwrap();

        tracker.create_belief("Goal", "Hypothesis");

        // Increase confidence
        tracker.update_belief(true, "Evidence 1", Some(0.6));
        tracker.update_belief(true, "Evidence 2", Some(0.7));
        tracker.update_belief(true, "Evidence 3", Some(0.8));

        assert!(tracker.is_confidence_increasing());

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_hypothesis_update() {
        let tmp_file = temp_dir().join("test_hypothesis.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut tracker = BeliefTracker::new(tmp_file.clone()).unwrap();

        tracker.create_belief("Goal", "Initial hypothesis");

        tracker.update_hypothesis("Updated hypothesis based on evidence").unwrap();

        let belief = tracker.get_current_belief().unwrap();
        assert_eq!(belief.current_hypothesis, "Updated hypothesis based on evidence");

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_save_load() {
        // Use unique filename to avoid race conditions with other tests
        let unique_id = std::process::id();
        let tmp_file = temp_dir().join(format!("test_save_load_{}.ndjson", unique_id));
        let _ = fs::remove_file(&tmp_file);

        {
            let mut tracker = BeliefTracker::new(tmp_file.clone()).unwrap();
            tracker.create_belief("Goal", "Hypothesis");
            tracker.update_belief(true, "Evidence", None);
            tracker.save().unwrap();
        }

        // Reopen to ensure data persists
        let tracker = BeliefTracker::new(tmp_file.clone()).unwrap();
        assert_eq!(tracker.states.len(), 1);

        let _ = fs::remove_file(&tmp_file);
    }
}
