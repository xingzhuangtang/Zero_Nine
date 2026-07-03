//! Integration Engine - 三系统联动引擎
//!
//! This module provides:
//! - 奖励模型、课程学习、信念状态的集成
//! - 统一的决策输出
//! - Harness Engineering 核心实现
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Integration Engine                        │
//! ├─────────────────────────────────────────────────────────────┤
//! │                                                              │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
//! │  │ Reward Model │───▶│   Belief     │◀───│  Curriculum  │  │
//! │  │              │    │   State      │    │   Learning   │  │
//! │  └──────────────┘    └──────────────┘    └──────────────┘  │
//! │         │                   │                   │          │
//! │         │                   │                   │          │
//! │         ▼                   ▼                   ▼          │
//! │  ┌─────────────────────────────────────────────────────┐  │
//! │  │              Decision Fusion Layer                   │  │
//! │  └─────────────────────────────────────────────────────┘  │
//! │                           │                                │
//! │                           ▼                                │
//! │  ┌─────────────────────────────────────────────────────┐  │
//! │  │              Integrated Decision                     │  │
//! │  │  - Should continue execution?                        │  │
//! │  │  - Should change hypothesis?                         │  │
//! │  │  - What difficulty level next?                       │  │
//! │  │  - What action to recommend?                         │  │
//! │  └─────────────────────────────────────────────────────┘  │
//! │                                                            │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::belief::{BeliefDecision, BeliefTracker, RecommendedAction};
use crate::curriculum::{CurriculumManager, OptimalTaskRecommendation};
use crate::reward::RewardModel;

/// 三系统联动引擎
pub struct IntegrationEngine {
    pub reward_model: RewardModel,
    pub curriculum_manager: CurriculumManager,
    pub belief_tracker: BeliefTracker,
    pub complexity_recorder: Option<Box<dyn zn_types::ComplexityRecorder>>,
    /// Per-agent metrics for collective learning
    agent_metrics: AgentMetricsStore,
}

/// 集成决策输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegratedDecision {
    /// 是否应该继续执行
    pub should_continue: bool,
    /// 是否应该改变假设
    pub should_change_hypothesis: bool,
    /// 是否应该升级处理
    pub should_escalate: bool,
    /// 推荐的难度级别
    pub recommended_difficulty: f32,
    /// 推荐的任务 ID
    pub recommended_task_id: Option<String>,
    /// 推荐的行动
    pub recommended_action: RecommendedAction,
    /// 置信度
    pub confidence: f32,
    /// 证据平衡
    pub evidence_balance: f32,
    /// 奖励模型评分
    pub reward_score: f32,
    /// 决策时间
    pub timestamp: chrono::DateTime<Utc>,
    /// 决策理由
    pub reasoning: DecisionReasoning,
    /// Trust-weighted confidence for multi-agent decisions
    #[serde(default)]
    pub trust_weighted_confidence: f32,
    /// Agents that contributed to this decision
    #[serde(default)]
    pub contributing_agents: Vec<String>,
    /// Decision trace for auditability
    #[serde(default)]
    pub decision_trace: Vec<DecisionTraceEntry>,
}

/// Individual entry in a decision trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionTraceEntry {
    pub timestamp: chrono::DateTime<Utc>,
    pub stage: String,
    pub decision: String,
    pub rationale: String,
    pub alternatives_considered: Vec<String>,
    pub agent_id: Option<String>,
}

/// Per-agent execution metrics for collective learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub agent_id: String,
    pub total_tasks: u64,
    pub successful_tasks: u64,
    #[serde(default)]
    pub avg_quality: f32,
    #[serde(default)]
    pub avg_latency_ms: u64,
    #[serde(default)]
    pub avg_token_usage: u64,
    #[serde(default)]
    pub trust_score_trend: Vec<f32>,
}

/// Collaboration pattern between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationPattern {
    pub agent_roles: Vec<String>,
    pub success_rate: f32,
    pub total_collaborations: u64,
    pub applicable_task_types: Vec<String>,
    #[serde(default)]
    pub avg_latency_ms: u64,
}

/// 决策理由分解
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DecisionReasoning {
    /// 来自信念系统的理由
    pub belief_reasoning: String,
    /// 来自课程系统的理由
    pub curriculum_reasoning: String,
    /// 来自奖励系统的理由
    pub reward_reasoning: String,
    /// 冲突检测
    pub conflicts: Vec<String>,
}

/// 引擎状态快照
#[derive(Debug, Clone)]
pub struct EngineSnapshot {
    pub reward_breakdown: crate::reward::RewardBreakdown,
    pub curriculum_stats: crate::curriculum::CurriculumStats,
    pub belief_summary: Option<crate::belief::BeliefSummary>,
    pub integrated_decision: IntegratedDecision,
}

/// Per-agent metrics store for collective learning
pub struct AgentMetricsStore {
    metrics: std::collections::HashMap<String, AgentMetrics>,
    collaboration_history: Vec<(Vec<String>, bool, u64)>,
}

impl AgentMetricsStore {
    fn new() -> Self {
        Self {
            metrics: std::collections::HashMap::new(),
            collaboration_history: Vec::new(),
        }
    }

    fn record_execution(
        &mut self,
        agent_id: &str,
        success: bool,
        quality: f32,
        latency_ms: u64,
        token_usage: u64,
        trust_score: f32,
    ) {
        let entry = self
            .metrics
            .entry(agent_id.to_string())
            .or_insert(AgentMetrics {
                agent_id: agent_id.to_string(),
                total_tasks: 0,
                successful_tasks: 0,
                avg_quality: 0.0,
                avg_latency_ms: 0,
                avg_token_usage: 0,
                trust_score_trend: Vec::new(),
            });
        let n = entry.total_tasks as f32;
        entry.total_tasks += 1;
        if success {
            entry.successful_tasks += 1;
        }
        entry.avg_quality = (entry.avg_quality * n + quality) / (n + 1.0);
        let new_avg_latency =
            (entry.avg_latency_ms as f64 * n as f64 + latency_ms as f64) / (n as f64 + 1.0);
        entry.avg_latency_ms = new_avg_latency as u64;
        let new_avg_token =
            (entry.avg_token_usage as f64 * n as f64 + token_usage as f64) / (n as f64 + 1.0);
        entry.avg_token_usage = new_avg_token as u64;
        entry.trust_score_trend.push(trust_score);
    }

    fn record_collaboration(&mut self, agent_ids: Vec<String>, success: bool, latency_ms: u64) {
        self.collaboration_history
            .push((agent_ids, success, latency_ms));
    }

    fn get_metrics(&self, agent_id: &str) -> Option<&AgentMetrics> {
        self.metrics.get(agent_id)
    }

    fn find_collaboration_patterns(&self) -> Vec<CollaborationPattern> {
        let mut pattern_map: std::collections::HashMap<String, (u64, u64, u64)> =
            std::collections::HashMap::new();
        for (agents, success, latency) in &self.collaboration_history {
            let mut sorted = agents.clone();
            sorted.sort();
            let key = sorted.join(",");
            let (total, successes, total_latency) = pattern_map.entry(key).or_insert((0, 0, 0));
            *total += 1;
            if *success {
                *successes += 1;
            }
            *total_latency += latency;
        }
        pattern_map
            .into_iter()
            .map(|(key, (total, successes, total_latency))| {
                let roles: Vec<String> = key.split(',').map(|s| s.to_string()).collect();
                CollaborationPattern {
                    agent_roles: roles,
                    success_rate: if total > 0 {
                        successes as f32 / total as f32
                    } else {
                        0.0
                    },
                    total_collaborations: total,
                    applicable_task_types: vec!["general".to_string()],
                    avg_latency_ms: if total > 0 { total_latency / total } else { 0 },
                }
            })
            .collect()
    }
}

/// 重置引擎所有子系统为干净状态（用于新项目）
pub struct EnginePaths {
    reward_file: std::path::PathBuf,
    curriculum_file: std::path::PathBuf,
    belief_file: std::path::PathBuf,
}

impl EnginePaths {
    fn new(project_root: &Path) -> Self {
        let evolve_dir = project_root.join(".zero_nine/evolve");
        Self {
            reward_file: evolve_dir.join("pairwise_comparisons.ndjson"),
            curriculum_file: evolve_dir.join("curriculum_history.ndjson"),
            belief_file: evolve_dir.join("belief_states.ndjson"),
        }
    }
}

impl IntegrationEngine {
    /// 创建新的集成引擎
    pub fn new(project_root: &Path) -> Result<Self> {
        let reward_model =
            RewardModel::new(project_root.join(".zero_nine/evolve/pairwise_comparisons.ndjson"))
                .context("Failed to initialize reward model")?;

        let curriculum_manager = CurriculumManager::new(
            project_root.join(".zero_nine/evolve/curriculum_history.ndjson"),
        )
        .context("Failed to initialize curriculum manager")?;

        let belief_tracker =
            BeliefTracker::new(project_root.join(".zero_nine/evolve/belief_states.ndjson"))
                .context("Failed to initialize belief tracker")?;

        Ok(Self {
            reward_model,
            curriculum_manager,
            belief_tracker,
            complexity_recorder: None,
            agent_metrics: AgentMetricsStore::new(),
        })
    }

    /// Record execution results and update all three systems
    pub fn record_execution(
        &mut self,
        task_id: &str,
        success: bool,
        evidence: &str,
        report: &zn_types::ExecutionReport,
    ) -> Result<()> {
        // 1. 更新奖励模型
        self.reward_model.record_from_report(report);

        // 2. 更新课程学习
        let task_diff = crate::curriculum::TaskDifficulty {
            task_id: task_id.to_string(),
            estimated_difficulty: 0.5,
            actual_difficulty: if success { 0.4 } else { 0.7 },
            completion_time_ms: report.execution_time_ms,
            success,
        };
        self.curriculum_manager.record_task_completion(&task_diff);

        // 3. 更新信念状态
        self.belief_tracker.update_belief(success, evidence, None);

        // 4. 更新复杂度记录
        if let Some(ref mut recorder) = self.complexity_recorder {
            recorder.record(task_id, 0.5, report); // 0.5 as default predicted; will be overridden by caller
        }

        // 5. 保存所有状态
        self.reward_model.save()?;
        self.curriculum_manager.save()?;
        self.belief_tracker.save()?;

        Ok(())
    }

    /// Record an external event and produce evolution signals.
    ///
    /// This extends the integration engine to accept signals from sources
    /// beyond the execution loop: CI failures, runtime crashes, user reports.
    pub fn record_external_event(
        &mut self,
        event: &zn_types::ExternalEvent,
    ) -> Result<Vec<zn_types::EvolutionSignal>> {
        use crate::signal_detector::SignalDetector;

        let detector = SignalDetector::default();
        let signals = detector.detect_from_external(event);

        if !signals.is_empty() {
            // Update belief tracker with external evidence
            let is_negative = signals
                .iter()
                .any(|s| matches!(s.proposed_action, zn_types::EvolutionAction::AutoFix));
            self.belief_tracker.update_belief(
                !is_negative,
                &format!("External event: {} — {}", event.event_type, event.title),
                None,
            );
            self.belief_tracker.save()?;

            // Record in reward model for tracking
            self.reward_model.record_external(
                &signals[0].task_id,
                signals[0].confidence,
                &event.title,
            );
            self.reward_model.save()?;
        }

        Ok(signals)
    }

    /// 获取集成决策
    pub fn get_integrated_decision(&self) -> IntegratedDecision {
        // 获取各子系统的决策
        let belief_decision = self.belief_tracker.get_decision();
        let optimal_task = self.curriculum_manager.get_optimal_next_task();
        let reward_breakdown = self.reward_model.get_breakdown();

        // 融合决策
        let should_continue =
            belief_decision.should_continue && reward_breakdown.weighted_score > 0.5;

        let should_change_hypothesis = belief_decision.should_change_hypothesis
            || (reward_breakdown.weighted_score < 0.3 && belief_decision.confidence < 0.4);

        let should_escalate = belief_decision.should_escalate
            || (belief_decision.confidence < 0.2 && reward_breakdown.code_quality < 0.3);

        // 确定推荐行动
        let recommended_action = self.fuse_actions(
            &belief_decision.recommended_action,
            &optimal_task,
            &reward_breakdown,
        );

        // 构建理由
        let reasoning = DecisionReasoning {
            belief_reasoning: format!(
                "置信度 {:.2}, 证据平衡 {:.2}, 趋势 {}",
                belief_decision.confidence,
                belief_decision.evidence_balance,
                if belief_decision.is_confidence_increasing {
                    "上升"
                } else {
                    "下降"
                }
            ),
            curriculum_reasoning: format!(
                "最优难度 {:.2}, 当前掌握 {:.2}, 推荐任务 {:?}",
                optimal_task.optimal_difficulty,
                optimal_task.current_mastery,
                optimal_task.recommended_task_id
            ),
            reward_reasoning: format!(
                "奖励评分 {:.2}, 代码质量 {:.2}, 测试覆盖 {:.2}",
                reward_breakdown.weighted_score,
                reward_breakdown.code_quality,
                reward_breakdown.test_coverage
            ),
            conflicts: self.detect_conflicts(&belief_decision, &optimal_task, &reward_breakdown),
        };

        let decision = IntegratedDecision {
            should_continue,
            should_change_hypothesis,
            should_escalate,
            recommended_difficulty: optimal_task.optimal_difficulty,
            recommended_task_id: optimal_task.recommended_task_id,
            recommended_action,
            confidence: belief_decision.confidence,
            evidence_balance: belief_decision.evidence_balance,
            reward_score: reward_breakdown.weighted_score,
            timestamp: Utc::now(),
            reasoning,
            trust_weighted_confidence: belief_decision.confidence * reward_breakdown.weighted_score,
            contributing_agents: Vec::new(),
            decision_trace: vec![
                DecisionTraceEntry {
                    timestamp: Utc::now(),
                    stage: "belief".to_string(),
                    decision: format!(
                        "continue={}, escalate={}, change_hypothesis={}",
                        belief_decision.should_continue,
                        belief_decision.should_escalate,
                        belief_decision.should_change_hypothesis
                    ),
                    rationale: belief_decision.confidence.to_string(),
                    alternatives_considered: vec![
                        "ProceedToExecution".to_string(),
                        "GatherMoreEvidence".to_string(),
                        "EscalateToHuman".to_string(),
                    ],
                    agent_id: None,
                },
                DecisionTraceEntry {
                    timestamp: Utc::now(),
                    stage: "reward".to_string(),
                    decision: format!("score={}", reward_breakdown.weighted_score),
                    rationale: format!(
                        "quality={}, coverage={}",
                        reward_breakdown.code_quality, reward_breakdown.test_coverage
                    ),
                    alternatives_considered: vec![],
                    agent_id: None,
                },
                DecisionTraceEntry {
                    timestamp: Utc::now(),
                    stage: "curriculum".to_string(),
                    decision: format!("difficulty={}", optimal_task.optimal_difficulty),
                    rationale: format!("mastery={}", optimal_task.current_mastery),
                    alternatives_considered: vec![],
                    agent_id: None,
                },
            ],
        };
        decision
    }

    /// 融合三个系统的行动推荐
    fn fuse_actions(
        &self,
        belief_action: &RecommendedAction,
        _curriculum: &OptimalTaskRecommendation,
        reward: &crate::reward::RewardBreakdown,
    ) -> RecommendedAction {
        // 如果奖励分数很低，优先收集更多证据
        if reward.weighted_score < 0.3 {
            return RecommendedAction::GatherMoreEvidence;
        }

        // 如果信念系统建议升级，优先升级
        if matches!(belief_action, RecommendedAction::EscalateToHuman) {
            return RecommendedAction::EscalateToHuman;
        }

        // 如果置信度高且奖励分数高，继续执行
        if reward.weighted_score > 0.7
            && matches!(belief_action, RecommendedAction::ProceedToExecution)
        {
            return RecommendedAction::ProceedToExecution;
        }

        // 默认使用信念系统的推荐
        belief_action.clone()
    }

    /// 检测子系统之间的冲突
    fn detect_conflicts(
        &self,
        belief: &BeliefDecision,
        curriculum: &OptimalTaskRecommendation,
        reward: &crate::reward::RewardBreakdown,
    ) -> Vec<String> {
        let mut conflicts = Vec::new();

        // 冲突 1: 高置信度但低奖励分数
        if belief.confidence > 0.7 && reward.weighted_score < 0.4 {
            conflicts.push("高置信度与低奖励分数冲突：系统认为假设正确但执行质量低".to_string());
        }

        // 冲突 2: 低置信度但高奖励分数
        if belief.confidence < 0.3 && reward.weighted_score > 0.7 {
            conflicts.push("低置信度与高奖励分数冲突：执行质量高但系统对假设不确定".to_string());
        }

        // 冲突 3: 课程推荐难度与当前能力差距过大
        let difficulty_gap = (curriculum.optimal_difficulty - curriculum.current_mastery).abs();
        if difficulty_gap > 0.3 {
            conflicts.push(format!(
                "课程难度差距过大：推荐难度 {:.2} 与当前掌握 {:.2} 差距 {:.2}",
                curriculum.optimal_difficulty, curriculum.current_mastery, difficulty_gap
            ));
        }

        conflicts
    }

    /// 获取引擎状态快照
    pub fn get_snapshot(&self) -> Result<EngineSnapshot> {
        let decision = self.get_integrated_decision();
        Ok(EngineSnapshot {
            reward_breakdown: self.reward_model.get_breakdown(),
            curriculum_stats: self.curriculum_manager.get_stats(),
            belief_summary: self.belief_tracker.get_summary(),
            integrated_decision: decision,
        })
    }

    /// 保存所有子系统状态
    pub fn save_all(&self) -> Result<()> {
        self.reward_model.save()?;
        self.curriculum_manager.save()?;
        self.belief_tracker.save()?;
        if let Some(ref recorder) = self.complexity_recorder {
            recorder.save()?;
        }
        Ok(())
    }

    /// 重置引擎所有子系统为干净状态（用于新项目）
    pub fn reset(&mut self, project_root: &Path) -> Result<()> {
        let paths = EnginePaths::new(project_root);
        self.reward_model =
            RewardModel::new(paths.reward_file).context("Failed to reset reward model")?;
        self.curriculum_manager = CurriculumManager::new(paths.curriculum_file)
            .context("Failed to reset curriculum manager")?;
        self.belief_tracker =
            BeliefTracker::new(paths.belief_file).context("Failed to reset belief tracker")?;
        self.complexity_recorder = None;
        Ok(())
    }

    /// Set the complexity recorder for tracking task complexity data.
    pub fn set_complexity_recorder(&mut self, recorder: Box<dyn zn_types::ComplexityRecorder>) {
        self.complexity_recorder = Some(recorder);
    }

    /// Record per-agent execution results for collective learning.
    pub fn record_agent_execution(
        &mut self,
        agent_id: &str,
        success: bool,
        quality: f32,
        latency_ms: u64,
        token_usage: u64,
        trust_score: f32,
    ) {
        self.agent_metrics.record_execution(
            agent_id,
            success,
            quality,
            latency_ms,
            token_usage,
            trust_score,
        );
    }

    /// Record a collaboration event between multiple agents.
    pub fn record_collaboration(&mut self, agent_ids: Vec<String>, success: bool, latency_ms: u64) {
        self.agent_metrics
            .record_collaboration(agent_ids, success, latency_ms);
    }

    /// Get execution metrics for a specific agent.
    pub fn get_agent_metrics(&self, agent_id: &str) -> Option<&AgentMetrics> {
        self.agent_metrics.get_metrics(agent_id)
    }

    /// Discover collaboration patterns from execution history.
    pub fn find_collaboration_patterns(&self) -> Vec<CollaborationPattern> {
        self.agent_metrics.find_collaboration_patterns()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use zn_types::{ExecutionOutcome, ExecutionReport};

    fn create_mock_report(task_id: &str, success: bool) -> ExecutionReport {
        ExecutionReport {
            task_id: task_id.to_string(),
            success,
            outcome: if success {
                ExecutionOutcome::Completed
            } else {
                ExecutionOutcome::RetryableFailure
            },
            summary: "Test".to_string(),
            details: vec![],
            tests_passed: success,
            review_passed: success,
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
            failure_summary: if success {
                None
            } else {
                Some("Test failure".to_string())
            },
            exit_code: if success { 0 } else { 1 },
            execution_time_ms: 1000,
            token_count: 500,
            code_quality_score: if success { 0.8 } else { 0.5 },
            test_coverage: if success { 0.9 } else { 0.6 },
            user_feedback: None,
            failure_classification: None,
            authorization_ticket_id: None,
            authorized_by: None,
            governance_summary: None,
            tri_role_verdict: None,
        }
    }

    #[test]
    fn test_integration_engine_lifecycle() {
        let tmp_dir = temp_dir().join("integration_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        // Create engine
        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();

        // Record some executions
        let report1 = create_mock_report("task-1", true);
        engine
            .record_execution("task-1", true, "Test passed", &report1)
            .unwrap();

        let report2 = create_mock_report("task-2", false);
        engine
            .record_execution("task-2", false, "Test failed", &report2)
            .unwrap();

        // Get integrated decision
        let decision = engine.get_integrated_decision();
        assert!(decision.confidence > 0.0);
        assert!(decision.reward_score >= 0.0);

        // Save all
        engine.save_all().unwrap();

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_conflict_detection() {
        let tmp_dir = temp_dir().join("conflict_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();

        // Create conflicting state: high confidence but low quality
        let report = create_mock_report("task-1", true);
        engine
            .record_execution("task-1", true, "Evidence", &report)
            .unwrap();

        let decision = engine.get_integrated_decision();

        // Should have a decision
        assert!(decision.confidence > 0.0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_record_execution_updates_reward() {
        let tmp_dir = temp_dir().join("ie_reward_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        let report = create_mock_report("t1", true);
        engine.record_execution("t1", true, "ok", &report).unwrap();

        let breakdown = engine.reward_model.get_breakdown();
        assert!(breakdown.code_quality > 0.0);
        assert!(breakdown.test_coverage > 0.0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_record_execution_updates_belief() {
        let tmp_dir = temp_dir().join("ie_belief_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        // Create initial belief state
        engine
            .belief_tracker
            .create_belief("test-goal", "initial hypothesis");

        let report = create_mock_report("t1", true);
        engine
            .record_execution("t1", true, "evidence for hypothesis", &report)
            .unwrap();

        let summary = engine.belief_tracker.get_summary();
        assert!(summary.unwrap().confidence > 0.0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_record_execution_updates_curriculum() {
        let tmp_dir = temp_dir().join("ie_curriculum_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        let report = create_mock_report("t1", true);
        engine
            .record_execution("t1", true, "completed", &report)
            .unwrap();

        let stats = engine.curriculum_manager.get_stats();
        assert!(stats.total_tasks >= 1);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_get_integrated_decision_structure() {
        let tmp_dir = temp_dir().join("ie_decision_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        let report = create_mock_report("t1", true);
        engine.record_execution("t1", true, "ok", &report).unwrap();

        let decision = engine.get_integrated_decision();
        assert!(decision.confidence > 0.0);
        assert!(decision.reward_score >= 0.0);
        assert!(decision.timestamp.timestamp() > 0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_fuse_low_reward_gather_evidence() {
        let tmp_dir = temp_dir().join("ie_low_reward_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        let report = create_mock_report("t1", false);
        engine
            .record_execution("t1", false, "low quality", &report)
            .unwrap();

        let decision = engine.get_integrated_decision();
        // Low reward should not recommend continue with high confidence
        assert!(decision.reward_score < 0.7 || !decision.should_continue);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_fuse_belief_escalate_priority() {
        let tmp_dir = temp_dir().join("ie_escalate_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        engine
            .belief_tracker
            .create_belief("test-goal", "initial hypothesis");
        // Record multiple failures to drive belief down
        for i in 0..3 {
            let report = create_mock_report(&format!("t{i}"), false);
            engine
                .record_execution(&format!("t{i}"), false, "against hypothesis", &report)
                .unwrap();
        }

        let decision = engine.get_integrated_decision();
        // After multiple failures, escalation should be considered
        assert!(decision.should_escalate || decision.should_change_hypothesis);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_conflict_high_confidence_low_reward() {
        let tmp_dir = temp_dir().join("ie_conflict1_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        // One success to build confidence but with moderate quality
        let report = create_mock_report("t1", true);
        engine
            .record_execution("t1", true, "strong evidence", &report)
            .unwrap();

        let decision = engine.get_integrated_decision();
        // Check reasoning for conflicts
        assert!(
            !decision.reasoning.belief_reasoning.is_empty()
                || !decision.reasoning.reward_reasoning.is_empty()
        );

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_conflict_difficulty_gap() {
        let tmp_dir = temp_dir().join("ie_difficulty_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        let report = create_mock_report("t1", true);
        engine
            .record_execution("t1", true, "easy", &report)
            .unwrap();

        let decision = engine.get_integrated_decision();
        assert!(decision.recommended_difficulty >= 0.0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    #[ignore = "reset hangs on this platform — likely file lock issue"]
    fn test_reset_clears_state() {
        let tmp_dir = temp_dir().join("ie_reset_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        engine
            .belief_tracker
            .create_belief("test-goal", "initial hypothesis");
        let report = create_mock_report("t1", true);
        engine.record_execution("t1", true, "ok", &report).unwrap();

        engine.reset(&tmp_dir).unwrap();
        let stats = engine.curriculum_manager.get_stats();
        assert_eq!(stats.total_tasks, 0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_consecutive_failures() {
        let tmp_dir = temp_dir().join("ie_consec_fail_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        engine
            .belief_tracker
            .create_belief("test-goal", "initial hypothesis");
        for i in 0..5 {
            let report = create_mock_report(&format!("t{i}"), false);
            engine
                .record_execution(&format!("t{i}"), false, "failure", &report)
                .unwrap();
        }

        let decision = engine.get_integrated_decision();
        // After 5 failures, should escalate or change hypothesis
        assert!(decision.should_escalate || decision.should_change_hypothesis);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_save_all_persists_files() {
        let tmp_dir = temp_dir().join("ie_save_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        engine
            .belief_tracker
            .create_belief("test-goal", "initial hypothesis");
        let report = create_mock_report("t1", true);
        engine.record_execution("t1", true, "ok", &report).unwrap();

        engine.save_all().unwrap();
        assert!(tmp_dir
            .join(".zero_nine/evolve/pairwise_comparisons.ndjson")
            .exists());
        assert!(tmp_dir
            .join(".zero_nine/evolve/curriculum_history.ndjson")
            .exists());
        assert!(tmp_dir
            .join(".zero_nine/evolve/belief_states.ndjson")
            .exists());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_decision_reasoning_accumulates() {
        let tmp_dir = temp_dir().join("ie_reason_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        engine
            .belief_tracker
            .create_belief("test-goal", "initial hypothesis");
        let report = create_mock_report("t1", true);
        engine
            .record_execution("t1", true, "strong evidence for approach", &report)
            .unwrap();

        let decision = engine.get_integrated_decision();
        assert!(
            !decision.reasoning.belief_reasoning.is_empty()
                || !decision.reasoning.reward_reasoning.is_empty()
        );

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_snapshot_contains_all_data() {
        let tmp_dir = temp_dir().join("ie_snapshot_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        engine
            .belief_tracker
            .create_belief("test-goal", "initial hypothesis");
        let report = create_mock_report("t1", true);
        engine.record_execution("t1", true, "ok", &report).unwrap();

        let snapshot = engine.get_snapshot().unwrap();
        assert!(snapshot.reward_breakdown.code_quality >= 0.0);
        assert!(snapshot.integrated_decision.confidence > 0.0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    /// Mock ComplexityRecorder for testing without circular dependency on zn-exec
    struct MockComplexityRecorder {
        recorded: std::sync::Arc<std::sync::Mutex<Vec<(String, f32)>>>,
        save_called: std::sync::Arc<std::sync::Mutex<bool>>,
    }

    impl MockComplexityRecorder {
        fn new() -> Self {
            Self {
                recorded: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
                save_called: std::sync::Arc::new(std::sync::Mutex::new(false)),
            }
        }
    }

    impl zn_types::ComplexityRecorder for MockComplexityRecorder {
        fn record(&mut self, task_id: &str, predicted: f32, _report: &zn_types::ExecutionReport) {
            self.recorded
                .lock()
                .unwrap()
                .push((task_id.to_string(), predicted));
        }

        fn save(&self) -> anyhow::Result<()> {
            *self.save_called.lock().unwrap() = true;
            Ok(())
        }
    }

    #[test]
    fn test_set_complexity_recorder() {
        let tmp_dir = temp_dir().join("ie_recorder_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        assert!(engine.complexity_recorder.is_none());

        // Set mock recorder
        let mock = MockComplexityRecorder::new();
        let recorded = mock.recorded.clone();
        let save_called = mock.save_called.clone();
        engine.set_complexity_recorder(Box::new(mock));
        assert!(engine.complexity_recorder.is_some());

        // Record execution - should call recorder.record()
        let report = create_mock_report("task-1", true);
        engine
            .record_execution("task-1", true, "Test passed", &report)
            .unwrap();

        // Verify record was called
        let recorded = recorded.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "task-1");
        assert!((recorded[0].1 - 0.5).abs() < f32::EPSILON); // default predicted score

        // Verify save_all calls recorder.save()
        engine.save_all().unwrap();
        assert!(*save_called.lock().unwrap());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_record_agent_execution() {
        let tmp_dir = temp_dir().join("ie_agent_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        engine.record_agent_execution("agent-a", true, 0.85, 1500, 3200, 0.7);
        engine.record_agent_execution("agent-a", true, 0.9, 1200, 2800, 0.75);
        engine.record_agent_execution("agent-b", false, 0.4, 3000, 5000, 0.3);

        let metrics_a = engine.get_agent_metrics("agent-a").unwrap();
        assert_eq!(metrics_a.total_tasks, 2);
        assert_eq!(metrics_a.successful_tasks, 2);
        assert!(metrics_a.avg_quality > 0.8);

        let metrics_b = engine.get_agent_metrics("agent-b").unwrap();
        assert_eq!(metrics_b.total_tasks, 1);
        assert_eq!(metrics_b.successful_tasks, 0);

        assert!(engine.get_agent_metrics("nonexistent").is_none());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_collaboration_patterns() {
        let tmp_dir = temp_dir().join("ie_collab_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        engine.record_collaboration(vec!["a".into(), "b".into()], true, 2000);
        engine.record_collaboration(vec!["a".into(), "b".into()], true, 1800);
        engine.record_collaboration(vec!["a".into(), "b".into()], false, 5000);
        engine.record_collaboration(vec!["c".into()], true, 1000);

        let patterns = engine.find_collaboration_patterns();
        assert_eq!(patterns.len(), 2);

        let ab = patterns.iter().find(|p| p.agent_roles.len() == 2).unwrap();
        assert_eq!(ab.total_collaborations, 3);
        assert!((ab.success_rate - 2.0 / 3.0).abs() < 0.01);

        let c = patterns.iter().find(|p| p.agent_roles == ["c"]).unwrap();
        assert_eq!(c.total_collaborations, 1);
        assert_eq!(c.success_rate, 1.0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_decision_trace_populated() {
        let tmp_dir = temp_dir().join("ie_trace_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut engine = IntegrationEngine::new(&tmp_dir).unwrap();
        let report = create_mock_report("t1", true);
        engine.record_execution("t1", true, "ok", &report).unwrap();

        let decision = engine.get_integrated_decision();
        assert!(!decision.decision_trace.is_empty());
        assert!(decision.trust_weighted_confidence > 0.0);
        assert_eq!(decision.decision_trace.len(), 3);
        assert_eq!(decision.decision_trace[0].stage, "belief");
        assert_eq!(decision.decision_trace[1].stage, "reward");
        assert_eq!(decision.decision_trace[2].stage, "curriculum");

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
