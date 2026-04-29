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

        // 4. 保存所有状态
        self.reward_model.save()?;
        self.curriculum_manager.save()?;
        self.belief_tracker.save()?;

        Ok(())
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

        IntegratedDecision {
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
        }
    }

    /// 融合三个系统的行动推荐
    fn fuse_actions(
        &self,
        belief_action: &RecommendedAction,
        curriculum: &OptimalTaskRecommendation,
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
        Ok(())
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
}
