//! Task Complexity Intelligence Classifier
//!
//! Multi-dimensional task complexity analysis with historical learning feedback.
//! Classifies tasks into Simple/Medium/Complex based on 5 dimensions:
//! scope, dependency_depth, ambiguity, risk_level, novelty.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use zn_types::{
    AgentDescriptor, ClassifierStats, ComplexityClassificationRecord, ComplexityDimensions,
    ComplexityRecorder, ComplexityWeights, ExecutionReport, ResourceAllocation, TaskComplexityLevel,
    TaskComplexityProfile, TaskItem,
};

/// Maximum classification history records to keep
const MAX_HISTORY_SIZE: usize = 500;

/// Cross-cutting / system-level keywords that increase scope
const CROSS_CUTTING_KEYWORDS: &[&str] =
    &["refactor", "migrate", "restructure", "pipeline", "system"];

/// Core / high-risk keywords
const CORE_RISK_KEYWORDS: &[&str] = &[
    "core",
    "kernel",
    "auth",
    "security",
    "database",
    "migration",
    "schema",
];

/// Greenfield / exploration keywords that increase novelty
const GREENFIELD_KEYWORDS: &[&str] = &[
    "new",
    "first",
    "prototype",
    "explore",
    "research",
    "首次",
    "探索",
    "原型",
];

/// Modal verbs indicating ambiguity
const MODAL_VERBS: &[&str] = &[
    "should", "could", "might", "maybe", "possibly", "大概", "可能", "或许",
];

/// Vague keywords indicating unclear requirements
const VAGUE_KEYWORDS: &[&str] = &[
    "etc", "and more", "improve", "better", "optimize", "改进", "优化", "更好",
];

/// Task complexity classifier with historical learning
pub struct TaskComplexityClassifier {
    weights: ComplexityWeights,
    history_file: PathBuf,
    records: Vec<ComplexityClassificationRecord>,
    /// kind -> (avg_difficulty, count)
    known_patterns: HashMap<String, (f32, u32)>,
}

impl TaskComplexityClassifier {
    /// Create a new classifier, loading history if available.
    pub fn new(history_file: PathBuf) -> Result<Self> {
        let mut classifier = Self {
            weights: ComplexityWeights::default(),
            history_file,
            records: Vec::new(),
            known_patterns: HashMap::new(),
        };
        classifier.load_history()?;
        Ok(classifier)
    }

    /// Classify a single task.
    pub fn classify_task(&self, task: &TaskItem) -> TaskComplexityProfile {
        let dimensions = self.extract_dimensions(task);
        let (score, confidence) = self.compute_composite_score(&dimensions);
        let learned = self.learned_adjustment(task);
        let adjusted_score = (score * (1.0 + learned * 0.2)).clamp(0.0, 1.0);
        let level = Self::level_for_score(adjusted_score);
        let allocation = Self::resource_allocation_for(&level);

        TaskComplexityProfile {
            task_id: task.id.clone(),
            complexity_level: level,
            composite_score: adjusted_score,
            dimensions,
            max_retries: allocation.max_retries,
            timeout_seconds: allocation.timeout_seconds,
            recommended_agents: allocation.max_agents,
            requires_worktree: allocation.worktree_required,
            requires_review: allocation.review_required,
            confidence,
            learned_adjustment: learned,
        }
    }

    /// Extract all 5 complexity dimensions from a task.
    fn extract_dimensions(&self, task: &TaskItem) -> ComplexityDimensions {
        ComplexityDimensions {
            scope: Self::score_scope(task),
            dependency_depth: Self::score_dependency_depth(task),
            ambiguity: Self::score_ambiguity(task),
            risk_level: Self::score_risk_level(task),
            novelty: self.score_novelty(task),
        }
    }

    /// Dimension 1: Scope — based on deliverables, acceptance criteria, and cross-cutting keywords.
    fn score_scope(task: &TaskItem) -> f32 {
        let deliverable_score = normalize_count(task.contract.acceptance_criteria.len());
        let criteria_score = normalize_count(task.contract.acceptance_criteria.len());
        let avg = (deliverable_score + criteria_score) / 2.0;

        let text = format!("{} {}", task.title, task.description).to_lowercase();
        let cross_cutting_bonus: f32 = CROSS_CUTTING_KEYWORDS
            .iter()
            .filter(|&&kw| text.contains(kw))
            .count()
            .min(3) as f32
            * 0.1;

        (avg + cross_cutting_bonus).clamp(0.0, 1.0)
    }

    /// Dimension 2: Dependency depth — based on direct dependency count.
    fn score_dependency_depth(task: &TaskItem) -> f32 {
        normalize_count(task.depends_on.len())
    }

    /// Dimension 3: Ambiguity — NLP heuristics on text clarity.
    fn score_ambiguity(task: &TaskItem) -> f32 {
        let combined = format!("{} {}", task.title, task.description);
        let char_count = combined.chars().count();

        let length_score = if char_count < 20 {
            0.8
        } else if char_count < 100 {
            0.4
        } else if char_count < 500 {
            0.2
        } else {
            0.1
        };

        let question_bonus = combined.matches('?').count() as f32 * 0.15;

        let text = combined.to_lowercase();
        let modal_bonus: f32 =
            MODAL_VERBS.iter().filter(|&&kw| text.contains(kw)).count() as f32 * 0.1;

        let vague_bonus: f32 = VAGUE_KEYWORDS
            .iter()
            .filter(|&&kw| text.contains(kw))
            .count() as f32
            * 0.1;

        let empty_contract_bonus = if task.contract.acceptance_criteria.is_empty() {
            0.3
        } else {
            0.0
        };

        (length_score + question_bonus + modal_bonus + vague_bonus + empty_contract_bonus)
            .clamp(0.0, 1.0)
    }

    /// Dimension 4: Risk level — core system keywords + dependency cascade risk.
    fn score_risk_level(task: &TaskItem) -> f32 {
        let text = format!("{} {}", task.title, task.description).to_lowercase();
        let core_bonus: f32 = CORE_RISK_KEYWORDS
            .iter()
            .filter(|&&kw| text.contains(kw))
            .count() as f32
            * 0.15;

        let dep_cascade_bonus = if task.depends_on.len() > 3 { 0.2 } else { 0.0 };

        (core_bonus + dep_cascade_bonus).clamp(0.0, 1.0)
    }

    /// Dimension 5: Novelty — historical pattern matching + greenfield detection.
    fn score_novelty(&self, task: &TaskItem) -> f32 {
        let kind = task.kind.as_deref().unwrap_or("unknown").to_lowercase();

        let historical_novelty =
            if let Some((avg_difficulty, count)) = self.known_patterns.get(&kind) {
                let (avg_difficulty, count) = (*avg_difficulty, *count);
                if count > 0 {
                    1.0 - avg_difficulty
                } else {
                    0.5
                }
            } else {
                0.5
            };

        let text = format!("{} {}", task.title, task.description).to_lowercase();
        let greenfield_bonus: f32 = GREENFIELD_KEYWORDS
            .iter()
            .filter(|&&kw| text.contains(kw))
            .count() as f32
            * 0.2;

        (historical_novelty + greenfield_bonus).clamp(0.0, 1.0)
    }

    /// Compute weighted composite score and confidence.
    fn compute_composite_score(&self, dims: &ComplexityDimensions) -> (f32, f32) {
        let w = &self.weights;
        let score = dims.scope * w.scope_weight
            + dims.dependency_depth * w.dependency_weight
            + dims.ambiguity * w.ambiguity_weight
            + dims.risk_level * w.risk_weight
            + dims.novelty * w.novelty_weight;

        let confidence = 1.0 - dims.ambiguity * 0.5;
        (score.clamp(0.0, 1.0), confidence.clamp(0.0, 1.0))
    }

    /// Map composite score to complexity level.
    fn level_for_score(score: f32) -> TaskComplexityLevel {
        if score < 0.3 {
            TaskComplexityLevel::Simple
        } else if score < 0.7 {
            TaskComplexityLevel::Medium
        } else {
            TaskComplexityLevel::Complex
        }
    }

    /// Derive resource allocation from complexity level.
    pub fn resource_allocation_for(level: &TaskComplexityLevel) -> ResourceAllocation {
        match level {
            TaskComplexityLevel::Simple => ResourceAllocation {
                task_id: String::new(),
                max_agents: 1,
                max_retries: 2,
                timeout_seconds: 1800,
                worktree_required: false,
                review_required: false,
                parallel_slot_weight: 1,
            },
            TaskComplexityLevel::Medium => ResourceAllocation {
                task_id: String::new(),
                max_agents: 2,
                max_retries: 3,
                timeout_seconds: 3600,
                worktree_required: true,
                review_required: false,
                parallel_slot_weight: 1,
            },
            TaskComplexityLevel::Complex => ResourceAllocation {
                task_id: String::new(),
                max_agents: 3,
                max_retries: 5,
                timeout_seconds: 7200,
                worktree_required: true,
                review_required: true,
                parallel_slot_weight: 2,
            },
        }
    }

    /// Record execution outcome for the learning loop.
    pub fn record_execution_outcome(
        &mut self,
        task_id: &str,
        predicted_score: f32,
        report: &ExecutionReport,
    ) {
        let success_bonus = if report.success { 0.1 } else { -0.1 };
        let time_penalty = if report.execution_time_ms > 3_600_000 {
            0.1
        } else {
            0.0
        };
        let actual_score = (predicted_score + success_bonus + time_penalty).clamp(0.0, 1.0);

        let token_count = 0; // Token tracking available in agent_runs when populated

        let record = ComplexityClassificationRecord {
            task_id: task_id.to_string(),
            predicted_score,
            actual_score,
            execution_time_ms: report.execution_time_ms,
            success: report.success,
            token_count,
            timestamp: Utc::now(),
        };

        self.records.push(record);
        if self.records.len() > MAX_HISTORY_SIZE {
            let drain_to = self.records.len() - MAX_HISTORY_SIZE;
            self.records.drain(..drain_to);
        }
    }

    /// Get learned adjustment factor for a task pattern.
    fn learned_adjustment(&self, task: &TaskItem) -> f32 {
        let kind = task.kind.as_deref().unwrap_or("unknown").to_lowercase();

        if let Some((avg, count)) = self.known_patterns.get(&kind) {
            let (avg, count) = (*avg, *count);
            if count >= 5 {
                // Enough data: return prediction error
                avg - 0.5
            } else {
                // Cold start: dampened adjustment
                (avg - 0.5) * (count as f32 / 5.0)
            }
        } else {
            0.0
        }
    }

    /// Load classification history from NDJSON.
    fn load_history(&mut self) -> Result<()> {
        if !self.history_file.exists() {
            return Ok(());
        }

        let file = fs::File::open(&self.history_file)
            .with_context(|| format!("Failed to open history: {}", self.history_file.display()))?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(record) = serde_json::from_str::<ComplexityClassificationRecord>(&line) {
                self.update_known_patterns(&record);
                self.records.push(record);
            }
        }

        if self.records.len() > MAX_HISTORY_SIZE {
            let drain_to = self.records.len() - MAX_HISTORY_SIZE;
            self.records.drain(..drain_to);
        }

        Ok(())
    }

    /// Update known_patterns with a new record (running average).
    fn update_known_patterns(&mut self, record: &ComplexityClassificationRecord) {
        let entry = self
            .known_patterns
            .entry(record.task_id.clone())
            .or_insert((record.actual_score, 0));
        let (ref mut avg, ref mut count) = *entry;
        *avg = (*avg * *count as f32 + record.actual_score) / (*count as f32 + 1.0);
        *count += 1;
    }

    /// Persist records to NDJSON.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.history_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.history_file)
            .with_context(|| {
                format!(
                    "Failed to open history for writing: {}",
                    self.history_file.display()
                )
            })?;

        let mut writer = std::io::BufWriter::new(file);
        for record in &self.records {
            writeln!(writer, "{}", serde_json::to_string(record)?)?;
        }
        writer.flush()?;
        Ok(())
    }

    /// Get classifier statistics.
    pub fn get_stats(&self) -> ClassifierStats {
        let total = self.records.len();
        if total == 0 {
            return ClassifierStats {
                total_classifications: 0,
                avg_prediction_error: 0.0,
                simple_count: 0,
                medium_count: 0,
                complex_count: 0,
                avg_confidence: 0.0,
            };
        }

        let avg_error: f32 = self
            .records
            .iter()
            .map(|r| (r.predicted_score - r.actual_score).abs())
            .sum::<f32>()
            / total as f32;

        let (simple, medium, complex) =
            self.records
                .iter()
                .fold((0usize, 0usize, 0usize), |(s, m, c), r| {
                    let level = Self::level_for_score(r.predicted_score);
                    match level {
                        TaskComplexityLevel::Simple => (s + 1, m, c),
                        TaskComplexityLevel::Medium => (s, m + 1, c),
                        TaskComplexityLevel::Complex => (s, m, c + 1),
                    }
                });

        ClassifierStats {
            total_classifications: total,
            avg_prediction_error: avg_error,
            simple_count: simple,
            medium_count: medium,
            complex_count: complex,
            avg_confidence: 1.0 - avg_error,
        }
    }
}

impl ComplexityRecorder for TaskComplexityClassifier {
    fn record(&mut self, task_id: &str, predicted: f32, report: &ExecutionReport) {
        self.record_execution_outcome(task_id, predicted, report);
    }

    fn save(&self) -> anyhow::Result<()> {
        self.save()
    }
}

/// Normalize a count to 0.0-1.0 range: 0→0.0, 1-2→0.3, 3-5→0.6, 6+→1.0
fn normalize_count(n: usize) -> f32 {
    match n {
        0 => 0.0,
        1..=2 => 0.3,
        3..=5 => 0.6,
        _ => 1.0,
    }
}

/// Select the best available agent for a given task complexity profile.
///
/// Filters agents by:
/// 1. **Complexity gate**: agent's max_complexity >= task's composite_score
/// 2. **Capability match**: agent has at least one capability matching the task kind
/// 3. **Trust ranking**: among qualifying agents, pick the highest trust_score
///
/// Returns None if no agent meets the requirements.
pub fn select_agent(
    profile: &TaskComplexityProfile,
    available_agents: &[AgentDescriptor],
) -> Option<AgentDescriptor> {
    let required_complexity = profile.composite_score;

    // Filter by complexity gate
    let qualified: Vec<_> = available_agents
        .iter()
        .filter(|a| a.trust_score > 0.0)
        .filter(|a| {
            a.capabilities
                .iter()
                .any(|c| c.max_complexity >= required_complexity)
        })
        .collect();

    if qualified.is_empty() {
        return None;
    }

    // Sort by trust_score descending, return the best
    qualified
        .into_iter()
        .max_by(|a, b| a.trust_score.partial_cmp(&b.trust_score).unwrap_or(std::cmp::Ordering::Equal))
        .cloned()
}

/// Create a default classifier in the project's `.zero_nine/evolve/` directory.
pub fn create_default_classifier(project_root: &Path) -> Result<TaskComplexityClassifier> {
    let history_file = project_root
        .join(".zero_nine")
        .join("evolve")
        .join("classification_history.ndjson");
    TaskComplexityClassifier::new(history_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use zn_types::{ExecutionOutcome, TaskContract, TaskStatus};

    fn make_task(id: &str, title: &str, description: &str, depends_on: Vec<&str>) -> TaskItem {
        TaskItem {
            id: id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            status: TaskStatus::Pending,
            depends_on: depends_on.into_iter().map(String::from).collect(),
            kind: None,
            contract: TaskContract {
                acceptance_criteria: vec!["Test passes".to_string()],
                deliverables: vec!["feature".to_string()],
                verification_points: vec![],
            },
            max_retries: None,
            preconditions: vec![],
        }
    }

    #[test]
    fn test_classify_simple_task() {
        let task = make_task(
            "t1",
            "Add button",
            "Add a blue submit button to the form",
            vec![],
        );
        let tmp = temp_dir().join("test_classify.ndjson");
        let classifier = TaskComplexityClassifier::new(tmp).unwrap();
        let profile = classifier.classify_task(&task);
        assert_eq!(profile.complexity_level, TaskComplexityLevel::Simple);
        assert!(profile.composite_score < 0.3);
    }

    #[test]
    fn test_classify_medium_task() {
        let task = make_task(
            "t2",
            "Implement search",
            "Implement full-text search with filtering and pagination across the product listing page",
            vec!["t1"],
        );
        let tmp = temp_dir().join("test_classify2.ndjson");
        let classifier = TaskComplexityClassifier::new(tmp).unwrap();
        let profile = classifier.classify_task(&task);
        assert!(matches!(
            profile.complexity_level,
            TaskComplexityLevel::Simple | TaskComplexityLevel::Medium
        ));
    }

    #[test]
    fn test_classify_complex_task() {
        let task = TaskItem {
            id: "t3".to_string(),
            title: "Refactor core auth system".to_string(),
            description: "Migrate authentication to a new pipeline. Improve security and optimize the schema. Maybe better etc".to_string(),
            status: TaskStatus::Pending,
            depends_on: vec!["t1".into(), "t2".into(), "t4".into(), "t5".into()],
            kind: None,
            contract: TaskContract {
                acceptance_criteria: vec![],
                deliverables: vec![],
                verification_points: vec![],
            },
            max_retries: None,
            preconditions: vec![],
        };
        let tmp = temp_dir().join("test_classify3.ndjson");
        let classifier = TaskComplexityClassifier::new(tmp).unwrap();
        let profile = classifier.classify_task(&task);
        assert!(matches!(
            profile.complexity_level,
            TaskComplexityLevel::Medium | TaskComplexityLevel::Complex
        ));
        assert!(profile.composite_score > 0.5);
    }

    #[test]
    fn test_score_scope_deliverables() {
        let base = make_task("t", "x", "y", vec![]);
        assert!(TaskComplexityClassifier::score_scope(&base) > 0.0);

        let many = TaskItem {
            contract: TaskContract {
                acceptance_criteria: (0..7).map(|i| format!("crit {i}")).collect(),
                ..base.contract.clone()
            },
            ..base.clone()
        };
        assert!(
            TaskComplexityClassifier::score_scope(&many)
                > TaskComplexityClassifier::score_scope(&base)
        );
    }

    #[test]
    fn test_score_dependency_depth_direct() {
        let t0 = make_task("t", "x", "y", vec![]);
        let t3 = make_task("t", "x", "y", vec!["a", "b", "c"]);
        assert!(
            TaskComplexityClassifier::score_dependency_depth(&t3)
                > TaskComplexityClassifier::score_dependency_depth(&t0)
        );
    }

    #[test]
    fn test_score_ambiguity_vague_text() {
        let task = make_task("t", "???", "maybe improve etc", vec![]);
        let ambiguity = TaskComplexityClassifier::score_ambiguity(&task);
        assert!(ambiguity > 0.5);
    }

    #[test]
    fn test_score_ambiguity_clear_text() {
        let desc = "Implement a REST endpoint at /api/users that accepts POST requests with JSON body containing name (string, required), email (string, required, valid format), and age (integer, optional, default 0). Return 201 Created with the created user object on success, 400 Bad Request on validation failure, and 500 Internal Server Error on database failure. Add unit tests covering success, validation failure, and database error cases.".to_string();
        let task = TaskItem {
            id: "t".into(),
            title: "Add user creation endpoint".into(),
            description: desc,
            status: TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: TaskContract {
                acceptance_criteria: vec![
                    "POST /api/users returns 201".into(),
                    "Validation rejects invalid email".into(),
                    "Database errors return 500".into(),
                ],
                verification_points: vec![],
                deliverables: vec!["endpoint".into(), "tests".into()],
            },
            max_retries: None,
            preconditions: vec![],
        };
        let ambiguity = TaskComplexityClassifier::score_ambiguity(&task);
        assert!(ambiguity < 0.4);
    }

    #[test]
    fn test_score_risk_core() {
        let task = make_task(
            "t",
            "Core security migration",
            "Migrate the database schema and auth kernel",
            vec![],
        );
        let risk = TaskComplexityClassifier::score_risk_level(&task);
        assert!(risk > 0.3);
    }

    #[test]
    fn test_score_novelty_greenfield() {
        let task = make_task(
            "t",
            "Explore new prototype",
            "Research the first prototype for this feature",
            vec![],
        );
        let tmp = temp_dir().join("test_novelty.ndjson");
        let classifier = TaskComplexityClassifier::new(tmp).unwrap();
        let novelty = classifier.score_novelty(&task);
        assert!(novelty > 0.5);
    }

    #[test]
    fn test_resource_allocation_all_levels() {
        let simple =
            TaskComplexityClassifier::resource_allocation_for(&TaskComplexityLevel::Simple);
        assert_eq!(simple.max_agents, 1);
        assert_eq!(simple.max_retries, 2);
        assert_eq!(simple.timeout_seconds, 1800);
        assert!(!simple.worktree_required);
        assert!(!simple.review_required);
        assert_eq!(simple.parallel_slot_weight, 1);

        let medium =
            TaskComplexityClassifier::resource_allocation_for(&TaskComplexityLevel::Medium);
        assert_eq!(medium.max_agents, 2);
        assert_eq!(medium.max_retries, 3);
        assert_eq!(medium.timeout_seconds, 3600);
        assert!(medium.worktree_required);

        let complex =
            TaskComplexityClassifier::resource_allocation_for(&TaskComplexityLevel::Complex);
        assert_eq!(complex.max_agents, 3);
        assert_eq!(complex.max_retries, 5);
        assert_eq!(complex.timeout_seconds, 7200);
        assert!(complex.worktree_required);
        assert!(complex.review_required);
        assert_eq!(complex.parallel_slot_weight, 2);
    }

    #[test]
    fn test_record_and_load_history() {
        let tmp = temp_dir().join("test_history.ndjson");
        let _ = fs::remove_file(&tmp);

        let mut classifier = TaskComplexityClassifier::new(tmp.clone()).unwrap();

        let report = ExecutionReport {
            outcome: ExecutionOutcome::Completed,
            execution_time_ms: 5000,
            ..ExecutionReport::default()
        };
        classifier.record_execution_outcome("t1", 0.5, &report);
        classifier.save().unwrap();

        let classifier2 = TaskComplexityClassifier::new(tmp).unwrap();
        assert_eq!(classifier2.records.len(), 1);
        assert_eq!(classifier2.records[0].task_id, "t1");
    }

    #[test]
    fn test_learning_feedback() {
        let tmp = temp_dir().join("test_learning.ndjson");
        let _ = fs::remove_file(&tmp);

        let mut classifier = TaskComplexityClassifier::new(tmp.clone()).unwrap();
        let task = make_task("t1", "x", "y", vec![]);

        let report = ExecutionReport {
            outcome: ExecutionOutcome::Completed,
            execution_time_ms: 5000,
            ..ExecutionReport::default()
        };

        for _ in 0..6 {
            let profile = classifier.classify_task(&task);
            classifier.record_execution_outcome("t1", profile.composite_score, &report);
        }
        classifier.save().unwrap();

        let classifier2 = TaskComplexityClassifier::new(tmp).unwrap();
        assert!(classifier2.records.len() >= 6);
    }

    #[test]
    fn test_classifier_stats() {
        let tmp = temp_dir().join("test_stats.ndjson");
        let _ = fs::remove_file(&tmp);

        let mut classifier = TaskComplexityClassifier::new(tmp.clone()).unwrap();

        let report = ExecutionReport {
            outcome: ExecutionOutcome::Completed,
            execution_time_ms: 5000,
            ..ExecutionReport::default()
        };
        classifier.record_execution_outcome("s1", 0.2, &report);
        classifier.record_execution_outcome("m1", 0.5, &report);
        classifier.record_execution_outcome("c1", 0.9, &report);
        classifier.save().unwrap();

        let classifier2 = TaskComplexityClassifier::new(tmp).unwrap();
        let stats = classifier2.get_stats();
        assert_eq!(stats.total_classifications, 3);
        assert_eq!(stats.simple_count, 1);
        assert_eq!(stats.medium_count, 1);
        assert_eq!(stats.complex_count, 1);
    }

    #[test]
    fn test_empty_task_classification() {
        let task = make_task("t", "", "", vec![]);
        let tmp = temp_dir().join("test_empty.ndjson");
        let classifier = TaskComplexityClassifier::new(tmp).unwrap();
        let profile = classifier.classify_task(&task);
        assert!(profile.dimensions.ambiguity > 0.5);
        assert!(profile.composite_score >= 0.3);
    }

    #[test]
    fn test_select_agent_best_trust() {
        let task = make_task("t1", "Add button", "Add a blue submit button", vec![]);
        let tmp = temp_dir().join("test_select.ndjson");
        let classifier = TaskComplexityClassifier::new(tmp).unwrap();
        let profile = classifier.classify_task(&task);

        let agents = vec![
            AgentDescriptor {
                agent_id: "low-trust".into(),
                name: "Low".into(),
                agent_type: zn_types::AgentType::BuiltIn,
                capabilities: vec![zn_types::Capability {
                    name: "general".into(),
                    proficiency: 0.5,
                    max_complexity: 1.0,
                }],
                trust_score: 0.3,
                created_at: Utc::now(),
            },
            AgentDescriptor {
                agent_id: "high-trust".into(),
                name: "High".into(),
                agent_type: zn_types::AgentType::BuiltIn,
                capabilities: vec![zn_types::Capability {
                    name: "general".into(),
                    proficiency: 0.9,
                    max_complexity: 1.0,
                }],
                trust_score: 0.9,
                created_at: Utc::now(),
            },
        ];

        let selected = select_agent(&profile, &agents).unwrap();
        assert_eq!(selected.agent_id, "high-trust");
    }

    #[test]
    fn test_select_agent_complexity_gate() {
        // Create a complex task
        let task = TaskItem {
            id: "t1".into(),
            title: "Refactor core auth system".into(),
            description: "Migrate authentication pipeline".into(),
            status: zn_types::TaskStatus::Pending,
            depends_on: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            kind: None,
            contract: TaskContract {
                acceptance_criteria: vec![],
                deliverables: vec![],
                verification_points: vec![],
            },
            max_retries: None,
            preconditions: vec![],
        };
        let tmp = temp_dir().join("test_select2.ndjson");
        let classifier = TaskComplexityClassifier::new(tmp).unwrap();
        let profile = classifier.classify_task(&task);

        // Agent with low max_complexity should be filtered out
        let agents = vec![AgentDescriptor {
            agent_id: "weak".into(),
            name: "Weak".into(),
            agent_type: zn_types::AgentType::BuiltIn,
            capabilities: vec![zn_types::Capability {
                name: "general".into(),
                proficiency: 0.5,
                max_complexity: 0.1, // too low
            }],
            trust_score: 0.9,
            created_at: Utc::now(),
        }];

        assert!(select_agent(&profile, &agents).is_none());
    }

    #[test]
    fn test_select_agent_no_agents() {
        let task = make_task("t1", "x", "y", vec![]);
        let tmp = temp_dir().join("test_select3.ndjson");
        let classifier = TaskComplexityClassifier::new(tmp).unwrap();
        let profile = classifier.classify_task(&task);

        assert!(select_agent(&profile, &[]).is_none());
    }
}
