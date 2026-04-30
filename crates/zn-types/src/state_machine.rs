//! Lifecycle state machine: LoopStage transitions, validation, and history.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoopStage {
    Idle,
    SpecDrafting,
    Ready,
    RunningTask,
    Verifying,
    Retrying,
    Escalated,
    Archived,
    Completed,
}

/// 状态转换记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    pub stage_from: LoopStage,
    pub stage_to: LoopStage,
    pub triggered_at: DateTime<Utc>,
    pub reason: String,
    #[serde(default)]
    pub task_id: Option<String>,
}

/// 非法转换错误
#[derive(Debug)]
pub struct IllegalTransitionError {
    pub from: LoopStage,
    pub to: LoopStage,
    pub allowed: Vec<LoopStage>,
}

impl std::fmt::Display for IllegalTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "illegal transition from {:?} to {:?}; allowed: {:?}",
            self.from, self.to, self.allowed
        )
    }
}

impl std::error::Error for IllegalTransitionError {}

impl LoopStage {
    /// 返回允许转换到的目标状态
    pub fn allowed_transitions(&self) -> Vec<LoopStage> {
        match self {
            Self::Idle => vec![Self::SpecDrafting, Self::Archived],
            Self::SpecDrafting => vec![Self::Ready, Self::Archived],
            Self::Ready => vec![Self::RunningTask, Self::Archived],
            Self::RunningTask => vec![
                Self::Verifying,
                Self::Retrying,
                Self::Escalated,
                Self::Archived,
            ],
            Self::Verifying => vec![Self::RunningTask, Self::Completed],
            Self::Retrying => vec![Self::RunningTask, Self::Escalated],
            Self::Escalated => vec![Self::RunningTask, Self::Archived],
            Self::Archived => vec![Self::Ready],
            Self::Completed => vec![],
        }
    }

    /// 检查转换是否合法
    pub fn can_transition_to(&self, target: LoopStage) -> bool {
        self.allowed_transitions().contains(&target)
    }

    /// 执行转换并记录转换事件
    pub fn transition_to(
        &self,
        target: LoopStage,
        reason: &str,
        task_id: Option<&str>,
    ) -> Result<StateTransition, IllegalTransitionError> {
        if !self.can_transition_to(target.clone()) {
            return Err(IllegalTransitionError {
                from: self.clone(),
                to: target.clone(),
                allowed: self.allowed_transitions(),
            });
        }
        Ok(StateTransition {
            from: format!("{:?}", self),
            to: format!("{:?}", target),
            stage_from: self.clone(),
            stage_to: target,
            triggered_at: Utc::now(),
            reason: reason.to_string(),
            task_id: task_id.map(String::from),
        })
    }
}
