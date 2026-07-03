//! Team coordination types for multi-agent collaborative execution.
//!
//! Defines roles, sessions, subtasks, and coordination log entries
//! used by the `TeamCoordinator` in `zn-exec`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::AgentDescriptor;

/// Organizational role of an agent within a team session.
///
/// Distinct from `AgentRole` (which describes task-level capability).
/// An agent with `AgentRole::Executor` might take `TeamRole::Worker`
/// or even `TeamRole::Leader` depending on the session context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamRole {
    /// Plans, delegates subtasks, aggregates results.
    Leader,
    /// Executes one or more assigned subtasks.
    Worker,
    /// Reviews completed work before the session closes.
    Reviewer,
    /// Read-only participant (logging, monitoring).
    Observer,
}

impl Default for TeamRole {
    fn default() -> Self {
        Self::Worker
    }
}

/// A team member entry inside a `TeamSession`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    /// Unique agent identifier.
    pub agent_id: String,
    /// Capability descriptor (capabilities, trust_score, etc.).
    pub agent_descriptor: AgentDescriptor,
    /// Role this agent plays in the team.
    pub team_role: TeamRole,
    /// ID of the subtask currently assigned to this agent, if any.
    #[serde(default)]
    pub assigned_subtask: Option<String>,
    /// When the agent joined this team session.
    #[serde(default = "chrono::Utc::now")]
    pub joined_at: DateTime<Utc>,
}

/// Lifecycle status of a single subtask.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubtaskStatus {
    /// Not yet assigned.
    Pending,
    /// Assigned to a worker but not started.
    Assigned,
    /// Worker is actively executing.
    InProgress,
    /// Worker finished successfully.
    Completed,
    /// Worker encountered an unrecoverable error.
    Failed,
    /// Blocked by an unresolved dependency.
    Blocked,
}

impl Default for SubtaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// Result produced by a worker after completing a subtask.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskResult {
    pub success: bool,
    pub summary: String,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub evidence_keys: Vec<String>,
}

/// A unit of work within a `TeamSession`, produced by the leader's decomposition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
    /// Unique subtask ID (UUID).
    pub id: String,
    /// ID of the parent task this subtask belongs to.
    pub parent_task_id: String,
    /// Short title.
    pub title: String,
    /// Full description of what the worker should do.
    pub description: String,
    /// Agent ID of the assigned worker, once assigned.
    #[serde(default)]
    pub assigned_to: Option<String>,
    /// Current lifecycle status.
    #[serde(default)]
    pub status: SubtaskStatus,
    /// IDs of subtasks that must complete before this one can start.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Result produced by the worker.
    #[serde(default)]
    pub result: Option<SubtaskResult>,
    #[serde(default = "chrono::Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "chrono::Utc::now")]
    pub updated_at: DateTime<Utc>,
}

/// Overall lifecycle status of a `TeamSession`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamSessionStatus {
    /// Assembling team members.
    Forming,
    /// Leader is decomposing the goal into subtasks.
    Planning,
    /// Workers are executing assigned subtasks.
    Executing,
    /// Reviewer is checking completed work.
    Reviewing,
    /// All subtasks complete; session closed successfully.
    Completed,
    /// Session aborted due to unrecoverable failure.
    Failed,
}

impl Default for TeamSessionStatus {
    fn default() -> Self {
        Self::Forming
    }
}

/// A timestamped entry in the team coordination log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationEntry {
    pub timestamp: DateTime<Utc>,
    pub from_agent: String,
    /// Short action label (e.g. "subtask_assigned", "review_requested").
    pub action: String,
    pub details: String,
    #[serde(default)]
    pub subtask_id: Option<String>,
}

/// A complete multi-agent team session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSession {
    /// Unique session ID (UUID).
    pub id: String,
    /// The proposal this session belongs to.
    pub proposal_id: String,
    /// The top-level task being decomposed.
    pub task_id: String,
    /// Goal / objective statement for the team.
    pub objective: String,
    /// All team members (leader + workers + optional reviewer).
    pub members: Vec<TeamMember>,
    /// Subtasks produced by the leader's decomposition.
    pub subtasks: Vec<Subtask>,
    /// Overall session status.
    #[serde(default)]
    pub status: TeamSessionStatus,
    /// Chronological log of coordination events.
    #[serde(default)]
    pub coordination_log: Vec<CoordinationEntry>,
    #[serde(default = "chrono::Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "chrono::Utc::now")]
    pub updated_at: DateTime<Utc>,
}

impl TeamSession {
    /// Return the leader's agent_id, if any leader is registered.
    pub fn leader_id(&self) -> Option<&str> {
        self.members
            .iter()
            .find(|m| m.team_role == TeamRole::Leader)
            .map(|m| m.agent_id.as_str())
    }

    /// Return agent IDs of all workers.
    pub fn worker_ids(&self) -> Vec<&str> {
        self.members
            .iter()
            .filter(|m| m.team_role == TeamRole::Worker)
            .map(|m| m.agent_id.as_str())
            .collect()
    }

    /// Check if all subtasks are in a terminal state (Completed or Failed).
    /// Returns false if there are no subtasks (nothing to complete yet).
    pub fn all_subtasks_terminal(&self) -> bool {
        !self.subtasks.is_empty()
            && self.subtasks.iter().all(|s| {
                matches!(s.status, SubtaskStatus::Completed | SubtaskStatus::Failed)
            })
    }

    /// Count subtasks by status.
    pub fn subtask_count(&self, status: &SubtaskStatus) -> usize {
        self.subtasks.iter().filter(|s| &s.status == status).count()
    }

    /// Append a coordination log entry.
    pub fn log(&mut self, from: &str, action: &str, details: &str, subtask_id: Option<&str>) {
        self.coordination_log.push(CoordinationEntry {
            timestamp: Utc::now(),
            from_agent: from.to_string(),
            action: action.to_string(),
            details: details.to_string(),
            subtask_id: subtask_id.map(|s| s.to_string()),
        });
        self.updated_at = Utc::now();
    }
}

/// Request to form a new team for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamFormationRequest {
    pub task_id: String,
    pub proposal_id: String,
    pub objective: String,
    /// Which roles are required (must include at least one Leader).
    #[serde(default)]
    pub required_roles: Vec<TeamRole>,
    /// Maximum number of worker agents to enlist.
    #[serde(default = "default_max_workers")]
    pub max_workers: u8,
    #[serde(default)]
    pub constraints: Vec<String>,
}

fn default_max_workers() -> u8 {
    3
}

/// Result of a successful team formation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamFormationResult {
    /// The newly created session.
    pub session: TeamSession,
    /// agent_id → initial subtask_id assignment (may be empty before decomposition).
    #[serde(default)]
    pub assignments: HashMap<String, String>,
}

/// Record of a conflict that was detected and resolved during a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResolution {
    pub conflict_id: String,
    pub session_id: String,
    pub description: String,
    pub involved_agents: Vec<String>,
    pub resolution: String,
    pub resolved_by: String,
    #[serde(default = "chrono::Utc::now")]
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session() -> TeamSession {
        let make_desc = |id: &str| crate::core::AgentDescriptor {
            agent_id: id.to_string(),
            name: id.to_string(),
            agent_type: crate::core::AgentType::BuiltIn,
            capabilities: vec![],
            trust_score: 0.5,
            created_at: Utc::now(),
        };
        TeamSession {
            id: "sess-1".to_string(),
            proposal_id: "prop-1".to_string(),
            task_id: "task-1".to_string(),
            objective: "build feature X".to_string(),
            members: vec![
                TeamMember {
                    agent_id: "leader".to_string(),
                    agent_descriptor: make_desc("leader"),
                    team_role: TeamRole::Leader,
                    assigned_subtask: None,
                    joined_at: Utc::now(),
                },
                TeamMember {
                    agent_id: "worker-1".to_string(),
                    agent_descriptor: make_desc("worker-1"),
                    team_role: TeamRole::Worker,
                    assigned_subtask: None,
                    joined_at: Utc::now(),
                },
            ],
            subtasks: vec![
                Subtask {
                    id: "sub-1".to_string(),
                    parent_task_id: "task-1".to_string(),
                    title: "Write tests".to_string(),
                    description: "TDD cycle".to_string(),
                    assigned_to: None,
                    status: SubtaskStatus::Completed,
                    depends_on: vec![],
                    result: Some(SubtaskResult {
                        success: true,
                        summary: "tests written".to_string(),
                        artifacts: vec![],
                        evidence_keys: vec![],
                    }),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
                Subtask {
                    id: "sub-2".to_string(),
                    parent_task_id: "task-1".to_string(),
                    title: "Implement feature".to_string(),
                    description: "code".to_string(),
                    assigned_to: Some("worker-1".to_string()),
                    status: SubtaskStatus::Failed,
                    depends_on: vec!["sub-1".to_string()],
                    result: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
            ],
            status: TeamSessionStatus::Executing,
            coordination_log: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_leader_id() {
        let sess = make_session();
        assert_eq!(sess.leader_id(), Some("leader"));
    }

    #[test]
    fn test_worker_ids() {
        let sess = make_session();
        assert_eq!(sess.worker_ids(), vec!["worker-1"]);
    }

    #[test]
    fn test_all_subtasks_terminal() {
        let sess = make_session();
        assert!(sess.all_subtasks_terminal());
    }

    #[test]
    fn test_subtask_count() {
        let sess = make_session();
        assert_eq!(sess.subtask_count(&SubtaskStatus::Completed), 1);
        assert_eq!(sess.subtask_count(&SubtaskStatus::Failed), 1);
        assert_eq!(sess.subtask_count(&SubtaskStatus::Pending), 0);
    }

    #[test]
    fn test_log_entry() {
        let mut sess = make_session();
        sess.log("leader", "subtask_assigned", "sub-1 → worker-1", Some("sub-1"));
        assert_eq!(sess.coordination_log.len(), 1);
        assert_eq!(sess.coordination_log[0].action, "subtask_assigned");
    }

    #[test]
    fn test_session_roundtrip() {
        let sess = make_session();
        let json = serde_json::to_string(&sess).unwrap();
        let restored: TeamSession = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "sess-1");
        assert_eq!(restored.members.len(), 2);
        assert_eq!(restored.subtasks.len(), 2);
    }

    #[test]
    fn test_team_role_default() {
        assert_eq!(TeamRole::default(), TeamRole::Worker);
    }

    #[test]
    fn test_session_status_default() {
        assert_eq!(TeamSessionStatus::default(), TeamSessionStatus::Forming);
    }
}
