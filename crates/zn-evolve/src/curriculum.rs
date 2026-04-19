//! Curriculum Manager - Course learning for task difficulty adaptation
//!
//! This module provides:
//! - Task difficulty assessment
//! - Skill tree tracking
//! - Dynamic difficulty adjustment
//! - Prerequisite management

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use zn_types::Curriculum;

/// Maximum history size per task
const MAX_HISTORY_SIZE: usize = 50;

/// Curriculum manager for course learning
pub struct CurriculumManager {
    curriculum: Curriculum,
    history_file: PathBuf,
}

/// Task difficulty estimate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDifficulty {
    pub task_id: String,
    pub estimated_difficulty: f32,
    pub actual_difficulty: f32,
    pub completion_time_ms: u64,
    pub success: bool,
}

/// Skill tree node
#[derive(Debug, Clone)]
pub struct SkillNode {
    pub skill_id: String,
    pub mastery: f32,
    pub prerequisites: Vec<String>,
    pub dependents: Vec<String>,
}

impl CurriculumManager {
    /// Create a new CurriculumManager
    pub fn new(history_file: PathBuf) -> Result<Self> {
        let mut manager = Self {
            curriculum: Curriculum::default(),
            history_file,
        };
        manager.load_history()?;
        Ok(manager)
    }

    /// Load existing history from file
    pub fn load_history(&mut self) -> Result<()> {
        if !self.history_file.exists() {
            return Ok(());
        }

        let file = fs::File::open(&self.history_file)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(diff) = serde_json::from_str::<TaskDifficulty>(&line) {
                self.record_task_completion(&diff);
            }
        }

        Ok(())
    }

    /// Record task completion
    pub fn record_task_completion(&mut self, difficulty: &TaskDifficulty) {
        // Update task difficulty estimate
        let current = self.curriculum.task_difficulty
            .get(&difficulty.task_id)
            .copied()
            .unwrap_or(difficulty.estimated_difficulty);

        // Exponential moving average
        let new_estimate = current * 0.7 + difficulty.actual_difficulty * 0.3;
        self.curriculum.task_difficulty
            .insert(difficulty.task_id.clone(), new_estimate);

        // Update mastery level
        let mastery = self.curriculum.mastery_level
            .entry(difficulty.task_id.clone())
            .or_insert(0.5);

        let success_rate = if difficulty.success { 1.0 } else { 0.0 };
        *mastery = (*mastery * 0.8 + success_rate * 0.2).clamp(0.0, 1.0);

        // Update success history
        self.curriculum.success_history.push(success_rate);
        if self.curriculum.success_history.len() > MAX_HISTORY_SIZE {
            self.curriculum.success_history.remove(0);
        }

        // Adapt difficulty
        self.adapt_difficulty();

        // Save history
        self.append_to_history(difficulty).ok();
    }

    /// Adapt difficulty based on recent performance
    pub fn adapt_difficulty(&mut self) {
        if self.curriculum.success_history.len() < 3 {
            return;
        }

        let recent_avg: f32 = self.curriculum.success_history
            .iter()
            .rev()
            .take(5)
            .sum::<f32>() / 5.0;

        // Adjust current difficulty level
        if recent_avg > 0.8 {
            // Too easy, increase difficulty
            self.curriculum.current_difficulty = (self.curriculum.current_difficulty + 0.1).min(0.9);
        } else if recent_avg < 0.4 {
            // Too hard, decrease difficulty
            self.curriculum.current_difficulty = (self.curriculum.current_difficulty - 0.1).max(0.1);
        }
    }

    /// Advanced difficulty adaptation using ELO-style rating
    pub fn adapt_difficulty_elo(&mut self, task_id: &str, success: bool, actual_difficulty: f32) {
        let current_rating = self.curriculum.task_difficulty
            .get(task_id)
            .copied()
            .unwrap_or(0.5);

        // K-factor: how much ratings change (higher for new tasks, lower for established)
        let k_factor = if self.curriculum.mastery_level.get(task_id).is_some() {
            0.1 // Established task
        } else {
            0.2 // New task
        };

        // Expected success based on current rating
        let expected_success = 1.0 / (1.0 + (10.0_f32).powf((current_rating - self.curriculum.current_difficulty) / 0.4));

        // Actual outcome
        let actual_outcome = if success { 1.0 } else { 0.0 };

        // Update rating using ELO formula
        let new_rating = current_rating + k_factor * (actual_outcome - expected_success);

        self.curriculum.task_difficulty.insert(task_id.to_string(), new_rating.clamp(0.1, 0.9));

        // Also update global difficulty
        let adjustment = k_factor * (actual_outcome - expected_success) * 0.5;
        self.curriculum.current_difficulty = (self.curriculum.current_difficulty + adjustment).clamp(0.1, 0.9);
    }

    /// Get task difficulty with uncertainty estimate
    pub fn get_task_difficulty_with_uncertainty(&self, task_id: &str) -> (f32, f32) {
        let difficulty = self.curriculum.task_difficulty
            .get(task_id)
            .copied()
            .unwrap_or(0.5);

        // Uncertainty decreases with more attempts
        let attempts = self.curriculum.success_history.len();
        let uncertainty = 1.0 / (1.0 + attempts as f32 * 0.1);

        (difficulty, uncertainty)
    }

    /// Get optimal next task based on zone of proximal development
    pub fn get_optimal_next_task(&self) -> OptimalTaskRecommendation {
        let current_mastery = if self.curriculum.mastery_level.is_empty() {
            0.5
        } else {
            self.curriculum.mastery_level.values().sum::<f32>() / self.curriculum.mastery_level.len() as f32
        };

        // Zone of proximal development: slightly above current ability
        let optimal_difficulty = current_mastery + 0.1;

        // Find tasks in the optimal zone
        let mut candidates: Vec<_> = self.curriculum.task_difficulty
            .iter()
            .filter(|(_, &diff)| (diff - optimal_difficulty).abs() < 0.15)
            .collect();

        candidates.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

        OptimalTaskRecommendation {
            optimal_difficulty,
            current_mastery,
            recommended_task_id: candidates.first().map(|(id, _)| (*id).clone()),
            zone_range: (optimal_difficulty - 0.15, optimal_difficulty + 0.15),
        }
    }

    /// Get recommended next task difficulty
    pub fn get_recommended_difficulty(&self) -> f32 {
        // Optimal challenge point: slightly above current mastery
        self.curriculum.current_difficulty * 1.1
    }

    /// Check if task prerequisites are met
    pub fn check_prerequisites(&self, task_id: &str) -> bool {
        self.curriculum.check_prerequisites(task_id)
    }

    /// Add a prerequisite relationship
    pub fn add_prerequisite(&mut self, skill_id: &str, prerequisite: &str) {
        self.curriculum.add_prerequisite(skill_id, prerequisite);
    }

    /// Get mastery level for a skill
    pub fn get_mastery(&self, skill_id: &str) -> f32 {
        self.curriculum.get_mastery(skill_id)
    }

    /// Get skill tree visualization
    pub fn get_skill_tree(&self) -> Vec<SkillNode> {
        let mut nodes = Vec::new();

        for (skill_id, &mastery) in &self.curriculum.mastery_level {
            let prereqs = self.curriculum.skill_prerequisites
                .get(skill_id)
                .cloned()
                .unwrap_or_default();

            // Find dependents (skills that depend on this one)
            let dependents: Vec<_> = self.curriculum.skill_prerequisites
                .iter()
                .filter(|(_, reqs)| reqs.contains(skill_id))
                .map(|(id, _)| id.clone())
                .collect();

            nodes.push(SkillNode {
                skill_id: skill_id.clone(),
                mastery,
                prerequisites: prereqs,
                dependents,
            });
        }

        nodes.sort_by(|a, b| b.mastery.partial_cmp(&a.mastery).unwrap_or(std::cmp::Ordering::Equal));
        nodes
    }

    /// Get tasks sorted by difficulty
    pub fn get_tasks_by_difficulty(&self) -> Vec<(&String, &f32)> {
        let mut tasks: Vec<_> = self.curriculum.task_difficulty.iter().collect();
        tasks.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        tasks
    }

    /// Get recent success rate
    pub fn get_recent_success_rate(&self) -> f32 {
        if self.curriculum.success_history.is_empty() {
            return 0.5;
        }

        let recent: Vec<f32> = self.curriculum.success_history.iter().rev().take(10).copied().collect();
        recent.iter().sum::<f32>() / recent.len() as f32
    }

    /// Get curriculum statistics
    pub fn get_stats(&self) -> CurriculumStats {
        CurriculumStats {
            total_tasks: self.curriculum.task_difficulty.len() as u32,
            total_skills: self.curriculum.mastery_level.len() as u32,
            avg_mastery: if self.curriculum.mastery_level.is_empty() {
                0.0
            } else {
                self.curriculum.mastery_level.values().sum::<f32>()
                    / self.curriculum.mastery_level.len() as f32
            },
            current_difficulty: self.curriculum.current_difficulty,
            recent_success_rate: self.get_recent_success_rate(),
            prerequisite_count: self.curriculum.skill_prerequisites
                .values()
                .map(|v| v.len())
                .sum::<usize>() as u32,
        }
    }

    /// Append task difficulty to history file
    fn append_to_history(&self, difficulty: &TaskDifficulty) -> Result<()> {
        if let Some(parent) = self.history_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history_file)?;

        let mut writer = std::io::BufWriter::new(file);
        let line = serde_json::to_string(difficulty)?;
        writeln!(writer, "{}", line)?;
        writer.flush()?;

        Ok(())
    }

    /// Save full curriculum state
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.history_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let curriculum_file = self.history_file.with_extension("json");
        let content = serde_json::to_string_pretty(&self.curriculum)?;
        fs::write(curriculum_file, content)?;

        Ok(())
    }
}

/// Curriculum statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurriculumStats {
    pub total_tasks: u32,
    pub total_skills: u32,
    pub avg_mastery: f32,
    pub current_difficulty: f32,
    pub recent_success_rate: f32,
    pub prerequisite_count: u32,
}

/// Optimal task recommendation based on zone of proximal development
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimalTaskRecommendation {
    /// The optimal difficulty level for learning
    pub optimal_difficulty: f32,
    /// Current average mastery level
    pub current_mastery: f32,
    /// Recommended task ID (if available)
    pub recommended_task_id: Option<String>,
    /// The zone range (lower, upper)
    pub zone_range: (f32, f32),
}

/// Create default curriculum manager in .zero_nine/evolve directory
pub fn create_default_curriculum_manager(project_root: &Path) -> Result<CurriculumManager> {
    let history_file = project_root
        .join(".zero_nine/evolve/curriculum_history.ndjson");
    CurriculumManager::new(history_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_curriculum_lifecycle() {
        let tmp_file = temp_dir().join("test_curriculum.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut manager = CurriculumManager::new(tmp_file.clone()).unwrap();

        // Record some task completions
        manager.record_task_completion(&TaskDifficulty {
            task_id: "task-1".to_string(),
            estimated_difficulty: 0.5,
            actual_difficulty: 0.6,
            completion_time_ms: 5000,
            success: true,
        });

        manager.record_task_completion(&TaskDifficulty {
            task_id: "task-2".to_string(),
            estimated_difficulty: 0.7,
            actual_difficulty: 0.8,
            completion_time_ms: 8000,
            success: false,
        });

        let stats = manager.get_stats();
        assert_eq!(stats.total_tasks, 2);
        assert!(stats.avg_mastery > 0.0);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_prerequisites() {
        let tmp_file = temp_dir().join("test_prereq.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut manager = CurriculumManager::new(tmp_file.clone()).unwrap();

        // Add prerequisite: task-2 requires task-1
        manager.add_prerequisite("task-2", "task-1");

        // Initially task-1 not mastered, so task-2 should not be available
        assert!(!manager.check_prerequisites("task-2"));

        // Simulate mastering task-1
        manager.record_task_completion(&TaskDifficulty {
            task_id: "task-1".to_string(),
            estimated_difficulty: 0.5,
            actual_difficulty: 0.5,
            completion_time_ms: 1000,
            success: true,
        });

        // Check again - still might not pass due to mastery threshold
        let mastery = manager.get_mastery("task-1");
        assert!(mastery > 0.5);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_difficulty_adaptation() {
        let tmp_file = temp_dir().join("test_adapt.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut manager = CurriculumManager::new(tmp_file.clone()).unwrap();

        // Record successful completions
        for i in 0..5 {
            manager.record_task_completion(&TaskDifficulty {
                task_id: format!("task-{}", i),
                estimated_difficulty: 0.5,
                actual_difficulty: 0.5,
                completion_time_ms: 1000,
                success: true,
            });
        }

        let stats = manager.get_stats();
        assert!(stats.recent_success_rate > 0.8);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_skill_tree() {
        let tmp_file = temp_dir().join("test_tree.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut manager = CurriculumManager::new(tmp_file.clone()).unwrap();

        // Create skill tree
        manager.add_prerequisite("advanced", "basic");
        manager.record_task_completion(&TaskDifficulty {
            task_id: "basic".to_string(),
            estimated_difficulty: 0.3,
            actual_difficulty: 0.3,
            completion_time_ms: 500,
            success: true,
        });

        let tree = manager.get_skill_tree();
        assert!(!tree.is_empty());

        let _ = fs::remove_file(&tmp_file);
    }
}
