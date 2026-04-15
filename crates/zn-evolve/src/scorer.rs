//! Skill Scorer - Track and evaluate skill performance over time
//!
//! This module provides:
//! - Execution recording and scoring
//! - Aggregate score calculation
//! - Improvement suggestions based on patterns

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use zn_types::{ExecutionReport, SkillEvaluation};

/// Maximum evaluations to keep in memory per skill
const MAX_EVALUATIONS_PER_SKILL: usize = 100;

/// Skill score summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillScoreSummary {
    pub skill_name: String,
    pub average_score: f32,
    pub execution_count: u64,
    pub success_rate: f32,
    pub average_latency_ms: u64,
    pub last_updated: DateTime<Utc>,
}

/// Skill improvement suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillImprovement {
    pub priority: u8,  // 1-5, 1 is highest
    pub category: String,
    pub suggestion: String,
    pub expected_impact: String,
}

/// Skill scorer that tracks evaluations over time
pub struct SkillScorer {
    evaluations: HashMap<String, Vec<SkillEvaluation>>,
    evaluations_file: PathBuf,
}

impl SkillScorer {
    /// Create a new SkillScorer with the given evaluations file
    pub fn new(evaluations_file: PathBuf) -> Result<Self> {
        let mut scorer = Self {
            evaluations: HashMap::new(),
            evaluations_file,
        };
        scorer.load_existing_evaluations()?;
        Ok(scorer)
    }

    /// Load existing evaluations from file
    pub fn load_existing_evaluations(&mut self) -> Result<()> {
        if !self.evaluations_file.exists() {
            return Ok(());
        }

        let file = fs::File::open(&self.evaluations_file)
            .with_context(|| format!("Failed to open evaluations file: {}", self.evaluations_file.display()))?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(eval) = serde_json::from_str::<SkillEvaluation>(&line) {
                self.evaluations
                    .entry(eval.skill_name.clone())
                    .or_insert_with(Vec::new)
                    .push(eval);
            }
        }

        // Trim to max size
        for evaluations in self.evaluations.values_mut() {
            if evaluations.len() > MAX_EVALUATIONS_PER_SKILL {
                *evaluations = evaluations.split_off(evaluations.len() - MAX_EVALUATIONS_PER_SKILL);
            }
        }

        Ok(())
    }

    /// Record a new skill execution
    pub fn record_execution(&mut self, skill_name: &str, success: bool, latency_ms: u64, token_cost: u64, notes: &str) {
        let score = if success {
            0.8 + (latency_ms.max(100) as f32 / 10000.0).min(0.17)  // 0.8-0.97 for success
        } else {
            0.3 + (latency_ms.max(100) as f32 / 10000.0).min(0.3)  // 0.3-0.6 for failure
        };

        let evaluation = SkillEvaluation {
            skill_name: skill_name.to_string(),
            task_type: "task_execution".to_string(),
            latency_ms,
            token_cost,
            score,
            notes: notes.to_string(),
        };

        self.evaluations
            .entry(skill_name.to_string())
            .or_insert_with(Vec::new)
            .push(evaluation);

        // Trim if needed
        if let Some(evals) = self.evaluations.get_mut(skill_name) {
            if evals.len() > MAX_EVALUATIONS_PER_SKILL {
                *evals = evals.split_off(evals.len() - MAX_EVALUATIONS_PER_SKILL);
            }
        }
    }

    /// Record execution from ExecutionReport
    pub fn record_from_report(&mut self, report: &ExecutionReport) {
        let skill_name = if report.tests_passed {
            "guarded-execution"
        } else {
            "evidence-driven-verification"
        };

        // Use default values since ExecutionReport doesn't have timing/cost fields
        let latency_ms = 0u64;
        let token_cost = 0u64;

        self.record_execution(
            skill_name,
            report.success,
            latency_ms,
            token_cost,
            &report.summary,
        );
    }

    /// Get the average score for a skill
    pub fn get_score(&self, skill_name: &str) -> Option<f32> {
        self.evaluations.get(skill_name).and_then(|evals| {
            if evals.is_empty() {
                None
            } else {
                Some(evals.iter().map(|e| e.score).sum::<f32>() / evals.len() as f32)
            }
        })
    }

    /// Get score summary for a skill
    pub fn get_score_summary(&self, skill_name: &str) -> Option<SkillScoreSummary> {
        self.evaluations.get(skill_name).and_then(|evals| {
            if evals.is_empty() {
                None
            } else {
                let avg_score = evals.iter().map(|e| e.score).sum::<f32>() / evals.len() as f32;
                let success_count = evals.iter().filter(|e| e.score >= 0.7).count();
                let avg_latency = evals.iter().map(|e| e.latency_ms).sum::<u64>() / evals.len() as u64;
                let last_updated = Utc::now();

                Some(SkillScoreSummary {
                    skill_name: skill_name.to_string(),
                    average_score: avg_score,
                    execution_count: evals.len() as u64,
                    success_rate: success_count as f32 / evals.len() as f32,
                    average_latency_ms: avg_latency,
                    last_updated,
                })
            }
        })
    }

    /// Get all skill summaries
    pub fn get_all_summaries(&self) -> Vec<SkillScoreSummary> {
        self.evaluations
            .keys()
            .filter_map(|name| self.get_score_summary(name))
            .collect()
    }

    /// Generate improvement suggestions for a skill
    pub fn suggest_improvements(&self, skill_name: &str) -> Vec<SkillImprovement> {
        let mut suggestions = Vec::new();
        let evals = match self.evaluations.get(skill_name) {
            Some(e) => e,
            None => return suggestions,
        };

        if evals.len() < 3 {
            // Not enough data
            return suggestions;
        }

        let avg_score = evals.iter().map(|e| e.score).sum::<f32>() / evals.len() as f32;
        let recent_evals = evals.iter().rev().take(5).collect::<Vec<_>>();
        let recent_avg = recent_evals.iter().map(|e| e.score).sum::<f32>() / recent_evals.len() as f32;

        // Check for declining performance
        if recent_avg < avg_score - 0.1 {
            suggestions.push(SkillImprovement {
                priority: 1,
                category: "Performance Decline".to_string(),
                suggestion: "Recent executions show declining performance. Review recent failure patterns and update error handling.".to_string(),
                expected_impact: "Restore previous success rate, reduce failures by 20-30%".to_string(),
            });
        }

        // Check for low success rate
        let success_rate = evals.iter().filter(|e| e.score >= 0.7).count() as f32 / evals.len() as f32;
        if success_rate < 0.6 {
            suggestions.push(SkillImprovement {
                priority: 1,
                category: "Low Success Rate".to_string(),
                suggestion: "Success rate is below 60%. Consider rewriting core logic or adding more robust error recovery.".to_string(),
                expected_impact: "Increase success rate to 80%+, reduce retry frequency".to_string(),
            });
        }

        // Check for high latency
        let avg_latency = evals.iter().map(|e| e.latency_ms).sum::<u64>() / evals.len() as u64;
        if avg_latency > 5000 {
            suggestions.push(SkillImprovement {
                priority: 2,
                category: "High Latency".to_string(),
                suggestion: format!("Average latency is {}ms. Consider optimizing prompt structure or caching intermediate results.", avg_latency),
                expected_impact: "Reduce execution time by 30-50%".to_string(),
            });
        }

        // Check for high token cost
        let avg_tokens = evals.iter().map(|e| e.token_cost).sum::<u64>() / evals.len() as u64;
        if avg_tokens > 10000 {
            suggestions.push(SkillImprovement {
                priority: 3,
                category: "High Token Cost".to_string(),
                suggestion: format!("Average token cost is {} tokens. Consider prompt optimization or splitting into smaller skills.", avg_tokens),
                expected_impact: "Reduce token usage by 20-40%".to_string(),
            });
        }

        // Analyze failure patterns
        let failures: Vec<_> = evals.iter().filter(|e| e.score < 0.6).collect();
        if failures.len() >= 2 {
            let common_terms = find_common_terms(&failures.iter().map(|e| e.notes.as_str()).collect::<Vec<_>>());
            if !common_terms.is_empty() {
                suggestions.push(SkillImprovement {
                    priority: 2,
                    category: "Recurring Failure Pattern".to_string(),
                    suggestion: format!("Common failure pattern detected: {}. Add specific handling for this case.", common_terms.join(", ")),
                    expected_impact: "Eliminate recurring failure mode".to_string(),
                });
            }
        }

        suggestions.sort_by_key(|s| s.priority);
        suggestions
    }

    /// Save evaluations to file (NDJSON format)
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.evaluations_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.evaluations_file)?;

        use std::io::Write;
        let mut writer = std::io::BufWriter::new(file);

        for evals in self.evaluations.values() {
            for eval in evals {
                let line = serde_json::to_string(eval)?;
                writeln!(writer, "{}", line)?;
            }
        }

        writer.flush()?;
        Ok(())
    }

    /// Clear all evaluations for a skill
    pub fn clear_skill(&mut self, skill_name: &str) {
        self.evaluations.remove(skill_name);
    }

    /// Get recent evaluations for a skill
    pub fn get_recent_evaluations(&self, skill_name: &str, count: usize) -> Vec<&SkillEvaluation> {
        self.evaluations
            .get(skill_name)
            .map(|evals| {
                evals.iter()
                    .rev()
                    .take(count)
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Create a default scorer in the project's .zero_nine/evolve directory
pub fn create_default_scorer(project_root: &Path) -> Result<SkillScorer> {
    let evaluations_file = project_root
        .join(".zero_nine")
        .join("evolve")
        .join("evaluations.ndjson");
    SkillScorer::new(evaluations_file)
}

/// Find common terms in failure notes
fn find_common_terms(notes: &[&str]) -> Vec<String> {
    // Simple implementation: find words that appear in multiple failure notes
    let mut word_counts: HashMap<String, usize> = HashMap::new();

    for note in notes {
        let words: HashSet<_> = note
            .split_whitespace()
            .filter(|w| w.len() > 4)  // Skip short words
            .collect();

        for word in words {
            *word_counts.entry(word.to_lowercase().to_string())
                .or_insert(0) += 1;
        }
    }

    // Find words that appear in at least half the notes
    let threshold = notes.len() / 2;
    let mut common: Vec<_> = word_counts
        .into_iter()
        .filter(|(_, count)| *count >= threshold)
        .map(|(word, _)| word)
        .collect();

    common.sort_by(|a, b| b.len().cmp(&a.len()));  // Longer terms first
    common.truncate(3);  // Top 3 terms
    common
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_record_execution() {
        let tmp_file = temp_dir().join("test_eval.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut scorer = SkillScorer::new(tmp_file.clone()).unwrap();
        scorer.record_execution("test-skill", true, 100, 500, "Test execution");

        let score = scorer.get_score("test-skill");
        assert!(score.is_some());
        assert!(score.unwrap() >= 0.8);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_score_summary() {
        let tmp_file = temp_dir().join("test_summary.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut scorer = SkillScorer::new(tmp_file.clone()).unwrap();

        // Record multiple executions
        for i in 0..10 {
            scorer.record_execution("test-skill", i % 2 == 0, 100 + i * 10, 500, "Test");
        }

        let summary = scorer.get_score_summary("test-skill");
        assert!(summary.is_some());
        let summary = summary.unwrap();
        assert_eq!(summary.execution_count, 10);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_improvement_suggestions() {
        let tmp_file = temp_dir().join("test_improve.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut scorer = SkillScorer::new(tmp_file.clone()).unwrap();

        // Record failing executions
        for _ in 0..5 {
            scorer.record_execution("failing-skill", false, 6000, 15000, "Error: timeout failed");
        }

        let suggestions = scorer.suggest_improvements("failing-skill");
        assert!(!suggestions.is_empty());

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_save_load() {
        let tmp_file = temp_dir().join("test_save.ndjson");
        let _ = fs::remove_file(&tmp_file);

        {
            let mut scorer = SkillScorer::new(tmp_file.clone()).unwrap();
            scorer.record_execution("skill-a", true, 100, 500, "Test A");
            scorer.record_execution("skill-b", false, 200, 600, "Test B");
            scorer.save().unwrap();
        }

        {
            let mut scorer = SkillScorer::new(tmp_file.clone()).unwrap();
            scorer.load_existing_evaluations().unwrap();

            assert!(scorer.get_score("skill-a").is_some());
            assert!(scorer.get_score("skill-b").is_some());
        }

        let _ = fs::remove_file(&tmp_file);
    }
}
