//! Multi-Dimensional Reward Model - RLHF-style reward learning
//!
//! This module provides:
//! - Multi-dimensional reward calculation
//! - Pairwise comparison recording
//! - Weight learning from user preferences
//! - Code quality, test coverage, user satisfaction metrics

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use zn_types::{ExecutionReport, MultiDimensionalReward, PairwiseComparison};

/// Maximum comparisons to keep in memory
const MAX_COMPARISONS: usize = 500;

/// Reward model that learns user preferences
pub struct RewardModel {
    reward: MultiDimensionalReward,
    comparisons_file: PathBuf,
}

impl RewardModel {
    /// Create a new RewardModel
    pub fn new(comparisons_file: PathBuf) -> Result<Self> {
        let mut model = Self {
            reward: MultiDimensionalReward::default(),
            comparisons_file,
        };
        model.load_existing_comparisons()?;
        Ok(model)
    }

    /// Load existing comparisons from file
    pub fn load_existing_comparisons(&mut self) -> Result<()> {
        if !self.comparisons_file.exists() {
            return Ok(());
        }

        let file = fs::File::open(&self.comparisons_file)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(comp) = serde_json::from_str::<PairwiseComparison>(&line) {
                self.reward.record_comparison(comp);
            }
        }

        // Trim to max size
        if self.reward.pairwise_comparisons.len() > MAX_COMPARISONS {
            let drain_idx = self.reward.pairwise_comparisons.len() - MAX_COMPARISONS;
            self.reward.pairwise_comparisons.drain(0..drain_idx);
        }

        Ok(())
    }

    /// Record a new execution outcome with multi-dimensional scoring
    pub fn record_execution(
        &mut self,
        task_id: &str,
        code_quality: f32,
        test_coverage: f32,
        execution_time_ms: u64,
        token_count: u64,
        user_satisfaction: Option<f32>,
    ) {
        // Normalize execution speed (assuming < 10s is good)
        let execution_speed = (1.0 - (execution_time_ms as f32 / 10000.0)).max(0.0);

        // Normalize token efficiency (assuming < 10k tokens is good)
        let token_efficiency = (1.0 - (token_count as f32 / 10000.0)).max(0.0);

        self.reward.code_quality = self.smooth_update(self.reward.code_quality, code_quality);
        self.reward.test_coverage = self.smooth_update(self.reward.test_coverage, test_coverage);
        self.reward.execution_speed = self.smooth_update(self.reward.execution_speed, execution_speed);
        self.reward.token_efficiency = self.smooth_update(self.reward.token_efficiency, token_efficiency);

        if let Some(sat) = user_satisfaction {
            self.reward.user_satisfaction = self.smooth_update(self.reward.user_satisfaction, sat);
        }

        // Store comparison for this execution
        let comparison = PairwiseComparison {
            task_id: task_id.to_string(),
            option_a: format!("quality={:.2}, coverage={:.2}", code_quality, test_coverage),
            option_b: String::new(),
            chosen: "A".to_string(),
            preferred_reason: Some(format!("task: {}", task_id)),
            timestamp: Utc::now(),
        };
        self.reward.record_comparison(comparison);
    }

    /// Smooth update with exponential moving average
    fn smooth_update(&self, current: f32, new: f32) -> f32 {
        current * 0.7 + new * 0.3
    }

    /// Record a pairwise comparison (A vs B choice)
    pub fn record_pairwise(
        &mut self,
        task_id: &str,
        option_a: &str,
        option_b: &str,
        chosen: &str,
        reason: Option<&str>,
    ) {
        let comparison = PairwiseComparison {
            task_id: task_id.to_string(),
            option_a: option_a.to_string(),
            option_b: option_b.to_string(),
            chosen: chosen.to_string(),
            preferred_reason: reason.map(|s| s.to_string()),
            timestamp: Utc::now(),
        };
        self.reward.record_comparison(comparison);
    }

    /// Record reward from execution report automatically
    /// Extracts all multi-dimensional metrics from the report
    pub fn record_from_report(&mut self, report: &ExecutionReport) {
        // Extract user satisfaction from feedback if available
        let user_satisfaction = report.user_feedback.as_ref().map(|feedback| {
            (feedback.rating as f32) / 5.0 // Normalize 1-5 to 0-1
        });

        self.record_execution(
            &report.task_id,
            report.code_quality_score,
            report.test_coverage,
            report.execution_time_ms,
            report.token_count,
            user_satisfaction,
        );
    }

    /// Get the weighted reward score
    pub fn get_weighted_reward(&self) -> f32 {
        self.reward.weighted_reward()
    }

    /// Get learned weights
    pub fn get_learned_weights(&self) -> &HashMap<String, f32> {
        &self.reward.learned_weights
    }

    /// Update weights manually
    pub fn set_weight(&mut self, dimension: &str, weight: f32) {
        self.reward.learned_weights.insert(dimension.to_string(), weight);
    }

    /// Get reward breakdown
    pub fn get_breakdown(&self) -> RewardBreakdown {
        RewardBreakdown {
            code_quality: self.reward.code_quality,
            test_coverage: self.reward.test_coverage,
            user_satisfaction: self.reward.user_satisfaction,
            execution_speed: self.reward.execution_speed,
            token_efficiency: self.reward.token_efficiency,
            weighted_score: self.reward.weighted_reward(),
            comparison_count: self.reward.pairwise_comparisons.len() as u32,
        }
    }

    /// Get current reward model state
    pub fn get_reward_state(&self) -> RewardState {
        let breakdown = self.get_breakdown();
        RewardState {
            code_quality: breakdown.code_quality,
            test_coverage: breakdown.test_coverage,
            user_satisfaction: breakdown.user_satisfaction,
            execution_speed: breakdown.execution_speed,
            token_efficiency: breakdown.token_efficiency,
            weighted_score: breakdown.weighted_score,
            learned_weights: self.reward.learned_weights.clone(),
        }
    }

    /// Save comparisons to file
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.comparisons_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.comparisons_file)?;

        let mut writer = std::io::BufWriter::new(file);

        for comp in &self.reward.pairwise_comparisons {
            let line = serde_json::to_string(comp)?;
            writeln!(writer, "{}", line)?;
        }

        writer.flush()?;
        Ok(())
    }
}

/// Reward breakdown summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardBreakdown {
    pub code_quality: f32,
    pub test_coverage: f32,
    pub user_satisfaction: f32,
    pub execution_speed: f32,
    pub token_efficiency: f32,
    pub weighted_score: f32,
    pub comparison_count: u32,
}

/// Current reward model state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardState {
    pub code_quality: f32,
    pub test_coverage: f32,
    pub user_satisfaction: f32,
    pub execution_speed: f32,
    pub token_efficiency: f32,
    pub weighted_score: f32,
    pub learned_weights: std::collections::HashMap<String, f32>,
}

/// Create default reward model in .zero_nine/evolve directory
pub fn create_default_reward_model(project_root: &Path) -> Result<RewardModel> {
    let comparisons_file = project_root
        .join(".zero_nine/evolve/pairwise_comparisons.ndjson");
    RewardModel::new(comparisons_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_reward_model_lifecycle() {
        let tmp_file = temp_dir().join("test_reward.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut model = RewardModel::new(tmp_file.clone()).unwrap();

        // Record some executions
        model.record_execution("task-1", 0.9, 0.8, 1500, 5000, Some(0.95));
        model.record_execution("task-2", 0.7, 0.6, 3000, 8000, Some(0.7));

        let breakdown = model.get_breakdown();
        assert!(breakdown.code_quality > 0.0);
        assert!(breakdown.weighted_score > 0.0);

        // Save and reload
        model.save().unwrap();

        let model2 = RewardModel::new(tmp_file.clone()).unwrap();
        let breakdown2 = model2.get_breakdown();
        assert_eq!(breakdown2.comparison_count, 2);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_pairwise_comparison() {
        let tmp_file = temp_dir().join("test_pairwise.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut model = RewardModel::new(tmp_file.clone()).unwrap();

        model.record_pairwise(
            "task-1",
            "Clean code, no tests",
            "Messy code, with tests",
            "B",
            Some("Test coverage is more important"),
        );

        let weights = model.get_learned_weights();
        assert!(!weights.is_empty());

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_weighted_reward() {
        let tmp_file = temp_dir().join("test_weighted.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut model = RewardModel::new(tmp_file.clone()).unwrap();

        // Set custom weights
        model.set_weight("code_quality", 0.4);
        model.set_weight("test_coverage", 0.3);
        model.set_weight("user_satisfaction", 0.3);

        model.record_execution("task-1", 0.8, 0.9, 1000, 2000, Some(0.95));

        let breakdown = model.get_breakdown();
        assert!(breakdown.weighted_score > 0.5);

        let _ = fs::remove_file(&tmp_file);
    }
}
