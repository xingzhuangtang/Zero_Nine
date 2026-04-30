//! Evolution types: belief state, curriculum learning, multi-dimensional reward,
//! skill bundles, and multi-agent orchestration.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::execution::SubagentDispatch;
use crate::governance::ActionRiskLevel;

// ==================== Weighted Evidence & Belief ====================

/// Evidence with weight and credibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedEvidence {
    pub content: String,
    pub weight: f32,
    pub credibility: f32,
    pub timestamp: DateTime<Utc>,
}

impl WeightedEvidence {
    pub fn new(content: &str, weight: f32, credibility: f32) -> Self {
        Self {
            content: content.to_string(),
            weight,
            credibility,
            timestamp: Utc::now(),
        }
    }

    pub fn adjusted_weight(&self) -> f32 {
        self.weight * self.credibility
    }
}

/// BeliefState - 在线信念状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefState {
    pub goal: String,
    pub current_hypothesis: String,
    pub confidence: f32,
    #[serde(default)]
    pub evidence_for: Vec<WeightedEvidence>,
    #[serde(default)]
    pub evidence_against: Vec<WeightedEvidence>,
    #[serde(default)]
    pub open_questions: Vec<String>,
    #[serde(default)]
    pub next_experiment: String,
    #[serde(default)]
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub confidence_history: Vec<f32>,
    #[serde(default)]
    pub hypothesis_history: Vec<String>,
}

impl Default for BeliefState {
    fn default() -> Self {
        Self {
            goal: String::new(),
            current_hypothesis: String::new(),
            confidence: 0.5,
            evidence_for: Vec::new(),
            evidence_against: Vec::new(),
            open_questions: Vec::new(),
            next_experiment: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            confidence_history: Vec::new(),
            hypothesis_history: Vec::new(),
        }
    }
}

impl BeliefState {
    pub fn update(&mut self, success: bool, evidence: &str, new_confidence: Option<f32>) {
        self.updated_at = Utc::now();

        let weight = if success { 0.7 } else { 0.5 };
        let credibility = 0.8;
        let weighted_evidence = WeightedEvidence::new(evidence, weight, credibility);

        if success {
            self.evidence_for.push(weighted_evidence);
            self.confidence = (self.confidence * 0.9 + 0.9 * 0.1).min(0.99);
        } else {
            self.evidence_against.push(weighted_evidence);
            self.confidence = (self.confidence * 0.8).max(0.1);
        }

        if let Some(conf) = new_confidence {
            self.confidence = conf.clamp(0.1, 0.99);
        }
    }

    pub fn add_question(&mut self, question: String) {
        if !self.open_questions.contains(&question) {
            self.open_questions.push(question);
        }
    }

    pub fn resolve_question(&mut self, question: &str) {
        self.open_questions.retain(|q| q != question);
    }
}

// ==================== Multi-Dimensional Reward ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairwiseComparison {
    pub task_id: String,
    pub option_a: String,
    pub option_b: String,
    pub chosen: String,
    #[serde(default)]
    pub preferred_reason: Option<String>,
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
}

impl Default for PairwiseComparison {
    fn default() -> Self {
        Self {
            task_id: String::new(),
            option_a: String::new(),
            option_b: String::new(),
            chosen: "A".to_string(),
            preferred_reason: None,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiDimensionalReward {
    pub code_quality: f32,
    pub test_coverage: f32,
    pub user_satisfaction: f32,
    pub execution_speed: f32,
    pub token_efficiency: f32,
    #[serde(default)]
    pub learned_weights: HashMap<String, f32>,
    #[serde(default)]
    pub pairwise_comparisons: Vec<PairwiseComparison>,
}

impl Default for MultiDimensionalReward {
    fn default() -> Self {
        Self {
            code_quality: 0.5,
            test_coverage: 0.5,
            user_satisfaction: 0.5,
            execution_speed: 0.5,
            token_efficiency: 0.5,
            learned_weights: HashMap::new(),
            pairwise_comparisons: Vec::new(),
        }
    }
}

impl MultiDimensionalReward {
    pub fn weighted_reward(&self) -> f32 {
        let weights = &self.learned_weights;
        let default_weight = 0.2;

        let w_code = *weights.get("code_quality").unwrap_or(&default_weight);
        let w_test = *weights.get("test_coverage").unwrap_or(&default_weight);
        let w_user = *weights.get("user_satisfaction").unwrap_or(&default_weight);
        let w_speed = *weights.get("execution_speed").unwrap_or(&default_weight);
        let w_token = *weights.get("token_efficiency").unwrap_or(&default_weight);

        self.code_quality * w_code
            + self.test_coverage * w_test
            + self.user_satisfaction * w_user
            + self.execution_speed * w_speed
            + self.token_efficiency * w_token
    }

    pub fn record_comparison(&mut self, comparison: PairwiseComparison) {
        self.pairwise_comparisons.push(comparison);
        self.update_weights_from_comparisons();
    }

    fn update_weights_from_comparisons(&mut self) {
        if self.pairwise_comparisons.is_empty() {
            return;
        }

        let mut preference_counts: HashMap<String, u32> = HashMap::new();
        for comp in &self.pairwise_comparisons {
            if let Some(pref) = &comp.preferred_reason {
                *preference_counts.entry(pref.clone()).or_insert(0) += 1;
            }
        }

        let total: u32 = preference_counts.values().sum();
        if total > 0 {
            for (key, count) in preference_counts {
                self.learned_weights
                    .insert(key, count as f32 / total as f32);
            }
        }
    }
}

// ==================== Curriculum Learning ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Curriculum {
    #[serde(default)]
    pub task_difficulty: HashMap<String, f32>,
    #[serde(default)]
    pub skill_prerequisites: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub mastery_level: HashMap<String, f32>,
    #[serde(default)]
    pub current_difficulty: f32,
    #[serde(default)]
    pub success_history: Vec<f32>,
}

impl Default for Curriculum {
    fn default() -> Self {
        Self {
            task_difficulty: HashMap::new(),
            skill_prerequisites: HashMap::new(),
            mastery_level: HashMap::new(),
            current_difficulty: 0.5,
            success_history: Vec::new(),
        }
    }
}

impl Curriculum {
    pub fn evaluate_task_difficulty(&mut self, task_id: &str, complexity: f32) -> f32 {
        let difficulty = complexity.clamp(0.1, 0.9);
        self.task_difficulty.insert(task_id.to_string(), difficulty);
        difficulty
    }

    pub fn record_completion(&mut self, task_id: &str, success: bool) {
        let _difficulty = self.task_difficulty.get(task_id).copied().unwrap_or(0.5);
        let success_rate = if success { 1.0 } else { 0.0 };

        self.success_history.push(success_rate);
        if self.success_history.len() > 10 {
            self.success_history.remove(0);
        }

        let mastery = self.mastery_level.entry(task_id.to_string()).or_insert(0.5);
        *mastery = (*mastery * 0.8 + success_rate * 0.2).clamp(0.0, 1.0);

        self.adapt_difficulty();
    }

    pub fn adapt_difficulty(&mut self) {
        if self.success_history.len() < 3 {
            return;
        }

        let recent_avg: f32 =
            self.success_history.iter().sum::<f32>() / self.success_history.len() as f32;

        if recent_avg > 0.8 {
            self.current_difficulty = (self.current_difficulty + 0.1).min(0.9);
        } else if recent_avg < 0.4 {
            self.current_difficulty = (self.current_difficulty - 0.1).max(0.1);
        }
    }

    pub fn get_mastery(&self, skill_id: &str) -> f32 {
        *self.mastery_level.get(skill_id).unwrap_or(&0.0)
    }

    pub fn check_prerequisites(&self, skill_id: &str) -> bool {
        let prereqs = self.skill_prerequisites.get(skill_id);
        match prereqs {
            None => true,
            Some(reqs) => reqs
                .iter()
                .all(|req| self.mastery_level.get(req).copied().unwrap_or(0.0) >= 0.7),
        }
    }

    pub fn add_prerequisite(&mut self, skill_id: &str, prerequisite: &str) {
        self.skill_prerequisites
            .entry(skill_id.to_string())
            .or_default()
            .push(prerequisite.to_string());
    }
}

// ==================== Skill Bundle & Versioning ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for SkillVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Default for SkillVersion {
    fn default() -> Self {
        Self {
            major: 1,
            minor: 0,
            patch: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBundle {
    pub id: String,
    pub name: String,
    pub version: SkillVersion,
    pub description: String,
    pub applicable_scenarios: Vec<String>,
    pub preconditions: Vec<String>,
    pub disabled_conditions: Vec<String>,
    pub risk_level: ActionRiskLevel,
    pub skill_chain: Vec<String>,
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub usage_count: u32,
    #[serde(default)]
    pub success_rate: f32,
    #[serde(default)]
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub updated_at: DateTime<Utc>,
}

impl Default for SkillBundle {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: SkillVersion::default(),
            description: String::new(),
            applicable_scenarios: Vec::new(),
            preconditions: Vec::new(),
            disabled_conditions: Vec::new(),
            risk_level: ActionRiskLevel::Medium,
            skill_chain: Vec::new(),
            artifacts: Vec::new(),
            usage_count: 0,
            success_rate: 0.0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct SkillLibrary {
    pub bundles: Vec<SkillBundle>,
    #[serde(default)]
    pub active_bundle_ids: Vec<String>,
}


// ==================== Multi-Agent Orchestration ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum AgentRole {
    Planner,
    #[default]
    Executor,
    Reviewer,
    Coordinator,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct MultiAgentOrchestration {
    pub proposal_id: String,
    pub dispatches: Vec<SubagentDispatch>,
    #[serde(default)]
    pub coordination_log: Vec<String>,
    #[serde(default)]
    pub conflict_resolutions: Vec<String>,
}

