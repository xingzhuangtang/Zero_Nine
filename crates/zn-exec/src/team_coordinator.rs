//! TeamCoordinator — orchestrates multi-agent teams via the A2A bus.
//!
//! Responsibilities:
//! - Form teams by assigning roles to registered agents.
//! - Leader decomposes a goal into subtasks and dispatches them via A2A.
//! - Workers report progress and completion over A2A.
//! - Reviewer submits a final verdict.
//! - Conflict resolution is logged in the session.

use anyhow::{anyhow, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use zn_types::{
    A2AChannel, A2AMessage, A2APayload, AgentDescriptor, CoordinationEntry, ConflictResolution,
    Subtask, SubtaskResult, SubtaskStatus, TeamFormationRequest, TeamFormationResult, TeamMember,
    TeamRole, TeamSession, TeamSessionStatus,
};

use crate::a2a_bus::A2ABus;
use crate::capability_registry::CapabilityRegistry;

/// TeamCoordinator — orchestrates multi-agent teams using A2A communication.
///
/// Wrap in an `Arc` to share across async tasks.
pub struct TeamCoordinator {
    bus: Arc<A2ABus>,
    registry: Arc<RwLock<CapabilityRegistry>>,
    sessions: Arc<RwLock<HashMap<String, TeamSession>>>,
    conflicts: Arc<RwLock<Vec<ConflictResolution>>>,
}

impl TeamCoordinator {
    /// Create a new coordinator with the given A2A bus and capability registry.
    pub fn new(bus: Arc<A2ABus>, registry: Arc<RwLock<CapabilityRegistry>>) -> Self {
        Self {
            bus,
            registry,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            conflicts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    // -------------------------------------------------------------------------
    // Team Formation
    // -------------------------------------------------------------------------

    /// Form a new team for the given request.
    ///
    /// Selects a leader (the first registered agent, or a synthetic stub if none
    /// are registered) plus up to `max_workers` additional workers from the
    /// capability registry.
    pub async fn form_team(
        &self,
        request: TeamFormationRequest,
    ) -> Result<TeamFormationResult> {
        let registry = self.registry.read().await;
        let all_agents: Vec<AgentDescriptor> = registry.list_agents();

        // Build members list: first agent is leader, rest are workers.
        let mut members: Vec<TeamMember> = Vec::new();
        let max_workers = request.max_workers as usize;

        for (i, desc) in all_agents.iter().enumerate() {
            let role = if i == 0 {
                TeamRole::Leader
            } else if i <= max_workers {
                TeamRole::Worker
            } else {
                break;
            };
            members.push(TeamMember {
                agent_id: desc.agent_id.clone(),
                agent_descriptor: desc.clone(),
                team_role: role,
                assigned_subtask: None,
                joined_at: Utc::now(),
            });
        }

        // If no agents registered, create a minimal synthetic team for testing.
        if members.is_empty() {
            let synthetic_id = format!("leader-{}", &Uuid::new_v4().to_string()[..8]);
            members.push(TeamMember {
                agent_id: synthetic_id.clone(),
                agent_descriptor: zn_types::AgentDescriptor {
                    agent_id: synthetic_id,
                    name: "synthetic-leader".to_string(),
                    agent_type: zn_types::AgentType::BuiltIn,
                    capabilities: vec![],
                    trust_score: 0.5,
                    created_at: Utc::now(),
                },
                team_role: TeamRole::Leader,
                assigned_subtask: None,
                joined_at: Utc::now(),
            });
        }

        let session = TeamSession {
            id: Uuid::new_v4().to_string(),
            proposal_id: request.proposal_id.clone(),
            task_id: request.task_id.clone(),
            objective: request.objective.clone(),
            members,
            subtasks: Vec::new(),
            status: TeamSessionStatus::Forming,
            coordination_log: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let session_id = session.id.clone();
        self.sessions
            .write()
            .await
            .insert(session_id.clone(), session.clone());

        Ok(TeamFormationResult {
            session,
            assignments: HashMap::new(),
        })
    }

    // -------------------------------------------------------------------------
    // Subtask Decomposition
    // -------------------------------------------------------------------------

    /// Leader: decompose the session goal into named subtasks and topologically
    /// assign them to workers (round-robin).
    ///
    /// `subtask_defs` is a list of `(title, description, depends_on_titles)`.
    pub async fn decompose_and_assign(
        &self,
        session_id: &str,
        subtask_defs: Vec<(String, String, Vec<String>)>,
    ) -> Result<Vec<Subtask>> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session '{}' not found", session_id))?;

        let worker_ids: Vec<String> = session
            .members
            .iter()
            .filter(|m| m.team_role == TeamRole::Worker)
            .map(|m| m.agent_id.clone())
            .collect();

        // Map title → id for dependency resolution.
        let mut title_to_id: HashMap<String, String> = HashMap::new();

        let subtasks: Vec<Subtask> = subtask_defs
            .iter()
            .enumerate()
            .map(|(i, (title, desc, dep_titles))| {
                let id = Uuid::new_v4().to_string();
                title_to_id.insert(title.clone(), id.clone());
                let assigned_to = if !worker_ids.is_empty() {
                    Some(worker_ids[i % worker_ids.len()].clone())
                } else {
                    None
                };
                let status = if assigned_to.is_some() {
                    SubtaskStatus::Assigned
                } else {
                    SubtaskStatus::Pending
                };
                Subtask {
                    id,
                    parent_task_id: session.task_id.clone(),
                    title: title.clone(),
                    description: desc.clone(),
                    assigned_to,
                    status,
                    depends_on: dep_titles
                        .iter()
                        .filter_map(|t| title_to_id.get(t).cloned())
                        .collect(),
                    result: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                }
            })
            .collect();

        session.subtasks = subtasks.clone();
        session.status = TeamSessionStatus::Planning;
        let leader_id = session
            .leader_id()
            .unwrap_or("coordinator")
            .to_string();
        session.log(
            &leader_id,
            "subtasks_decomposed",
            &format!("{} subtasks created", subtasks.len()),
            None,
        );

        Ok(subtasks)
    }

    // -------------------------------------------------------------------------
    // Dispatch
    // -------------------------------------------------------------------------

    /// Leader: dispatch a subtask to its assigned worker via A2A.
    pub async fn dispatch_subtask(&self, session_id: &str, subtask_id: &str) -> Result<()> {
        let (worker_id, objective, context_files) = {
            let sessions = self.sessions.read().await;
            let session = sessions
                .get(session_id)
                .ok_or_else(|| anyhow!("session '{}' not found", session_id))?;
            let subtask = session
                .subtasks
                .iter()
                .find(|s| s.id == subtask_id)
                .ok_or_else(|| anyhow!("subtask '{}' not found", subtask_id))?;
            let worker = subtask
                .assigned_to
                .clone()
                .ok_or_else(|| anyhow!("subtask '{}' has no assigned worker", subtask_id))?;
            (worker, subtask.description.clone(), vec![])
        };

        // Update status to InProgress.
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                if let Some(subtask) = session.subtasks.iter_mut().find(|s| s.id == subtask_id) {
                    subtask.status = SubtaskStatus::InProgress;
                    subtask.updated_at = Utc::now();
                }
                let leader_id = session.leader_id().unwrap_or("coordinator").to_string();
                session.log(
                    &leader_id,
                    "subtask_dispatched",
                    &format!("subtask {} → {}", subtask_id, worker_id),
                    Some(subtask_id),
                );
                session.status = TeamSessionStatus::Executing;
            }
        }

        let msg = A2AMessage::unicast(
            "coordinator",
            &worker_id,
            A2AChannel::Coordination,
            A2APayload::TaskDispatch {
                task_id: subtask_id.to_string(),
                objective,
                context: context_files,
            },
        );
        self.bus.send(msg).await?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Progress & Completion Reporting
    // -------------------------------------------------------------------------

    /// Worker: report progress on a subtask.
    pub async fn report_progress(
        &self,
        session_id: &str,
        subtask_id: &str,
        percent: u8,
        summary: &str,
        worker_id: &str,
    ) -> Result<()> {
        // Find the leader to notify.
        let leader_id = {
            let sessions = self.sessions.read().await;
            let session = sessions
                .get(session_id)
                .ok_or_else(|| anyhow!("session '{}' not found", session_id))?;
            session.leader_id().map(|s| s.to_string())
        };

        if let Some(ref lid) = leader_id {
            let msg = A2AMessage::unicast(
                worker_id,
                lid,
                A2AChannel::Progress,
                A2APayload::TaskProgress {
                    task_id: subtask_id.to_string(),
                    percent,
                    summary: summary.to_string(),
                },
            );
            self.bus.send(msg).await?;
        }
        Ok(())
    }

    /// Worker: report subtask completion.
    pub async fn report_completion(
        &self,
        session_id: &str,
        subtask_id: &str,
        success: bool,
        summary: &str,
        artifacts: Vec<String>,
        worker_id: &str,
    ) -> Result<()> {
        // Persist result.
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                if let Some(subtask) = session.subtasks.iter_mut().find(|s| s.id == subtask_id) {
                    subtask.status = if success {
                        SubtaskStatus::Completed
                    } else {
                        SubtaskStatus::Failed
                    };
                    subtask.result = Some(SubtaskResult {
                        success,
                        summary: summary.to_string(),
                        artifacts: artifacts.clone(),
                        evidence_keys: vec![],
                    });
                    subtask.updated_at = Utc::now();
                }
                session.log(
                    worker_id,
                    if success { "subtask_completed" } else { "subtask_failed" },
                    summary,
                    Some(subtask_id),
                );
            }
        }

        // Notify leader.
        let leader_id = {
            let sessions = self.sessions.read().await;
            sessions
                .get(session_id)
                .and_then(|s| s.leader_id().map(|l| l.to_string()))
        };

        if let Some(lid) = leader_id {
            let msg = A2AMessage::unicast(
                worker_id,
                &lid,
                A2AChannel::Progress,
                A2APayload::TaskCompleted {
                    task_id: subtask_id.to_string(),
                    success,
                    summary: summary.to_string(),
                    artifacts,
                },
            );
            self.bus.send(msg).await?;
        }
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Review
    // -------------------------------------------------------------------------

    /// Reviewer: submit a verdict on the overall session work.
    pub async fn submit_review(
        &self,
        session_id: &str,
        subtask_id: &str,
        approved: bool,
        notes: &str,
        reviewer_id: &str,
    ) -> Result<()> {
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.log(
                    reviewer_id,
                    if approved { "review_approved" } else { "review_rejected" },
                    notes,
                    Some(subtask_id),
                );
                if approved && session.all_subtasks_terminal() {
                    session.status = TeamSessionStatus::Completed;
                }
            }
        }

        // Broadcast verdict.
        let msg = A2AMessage::broadcast(
            reviewer_id,
            A2AChannel::Review,
            A2APayload::ReviewVerdict {
                task_id: subtask_id.to_string(),
                approved,
                notes: notes.to_string(),
            },
        );
        self.bus.send(msg).await?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Session Queries
    // -------------------------------------------------------------------------

    /// Get a snapshot of a session.
    pub async fn get_session(&self, session_id: &str) -> Option<TeamSession> {
        self.sessions.read().await.get(session_id).cloned()
    }

    /// List all active session IDs.
    pub async fn list_sessions(&self) -> Vec<String> {
        self.sessions.read().await.keys().cloned().collect()
    }

    /// Check if all subtasks in a session are complete.
    pub async fn is_session_complete(&self, session_id: &str) -> bool {
        self.sessions
            .read()
            .await
            .get(session_id)
            .map(|s| s.all_subtasks_terminal())
            .unwrap_or(false)
    }

    // -------------------------------------------------------------------------
    // Conflict Resolution
    // -------------------------------------------------------------------------

    /// Log a conflict resolution event in the session and the global conflict list.
    pub async fn resolve_conflict(
        &self,
        session_id: &str,
        description: &str,
        involved_agents: Vec<String>,
        resolution: &str,
        resolved_by: &str,
    ) -> Result<ConflictResolution> {
        let conflict = ConflictResolution {
            conflict_id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            description: description.to_string(),
            involved_agents: involved_agents.clone(),
            resolution: resolution.to_string(),
            resolved_by: resolved_by.to_string(),
            timestamp: Utc::now(),
        };

        // Append to session log.
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.log(
                    resolved_by,
                    "conflict_resolved",
                    &format!("{}: {}", description, resolution),
                    None,
                );
            }
        }

        self.conflicts.write().await.push(conflict.clone());
        Ok(conflict)
    }

    /// Return all conflict resolutions (all sessions).
    pub async fn all_conflicts(&self) -> Vec<ConflictResolution> {
        self.conflicts.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability_registry::CapabilityRegistry;

    fn make_coordinator() -> TeamCoordinator {
        let bus = Arc::new(A2ABus::new());
        let registry = Arc::new(RwLock::new(CapabilityRegistry::new()));
        TeamCoordinator::new(bus, registry)
    }

    fn make_request() -> TeamFormationRequest {
        TeamFormationRequest {
            task_id: "task-1".to_string(),
            proposal_id: "prop-1".to_string(),
            objective: "implement feature X".to_string(),
            required_roles: vec![TeamRole::Leader, TeamRole::Worker],
            max_workers: 2,
            constraints: vec![],
        }
    }

    #[tokio::test]
    async fn test_form_team_no_agents() {
        let coord = make_coordinator();
        let result = coord.form_team(make_request()).await.unwrap();
        // With no registered agents, a synthetic leader is created.
        assert!(!result.session.id.is_empty());
        assert_eq!(result.session.members.len(), 1);
        assert_eq!(result.session.members[0].team_role, TeamRole::Leader);
        assert_eq!(result.session.status, TeamSessionStatus::Forming);
    }

    #[tokio::test]
    async fn test_form_team_with_agents() {
        let bus = Arc::new(A2ABus::new());
        let registry = Arc::new(RwLock::new(CapabilityRegistry::new()));

        // Register two agents.
        let desc1 = zn_types::AgentDescriptor {
            agent_id: "a1".to_string(),
            name: "agent-1".to_string(),
            agent_type: zn_types::AgentType::BuiltIn,
            capabilities: vec![],
            trust_score: 0.5,
            created_at: chrono::Utc::now(),
        };
        let desc2 = zn_types::AgentDescriptor {
            agent_id: "a2".to_string(),
            name: "agent-2".to_string(),
            agent_type: zn_types::AgentType::BuiltIn,
            capabilities: vec![],
            trust_score: 0.5,
            created_at: chrono::Utc::now(),
        };

        {
            let mut reg = registry.write().await;
            reg.register(desc1);
            reg.register(desc2);
        }

        let coord = TeamCoordinator::new(bus, registry);
        let result = coord.form_team(make_request()).await.unwrap();

        assert_eq!(result.session.members.len(), 2);
        assert_eq!(result.session.members[0].team_role, TeamRole::Leader);
        assert_eq!(result.session.members[1].team_role, TeamRole::Worker);
    }

    #[tokio::test]
    async fn test_decompose_and_assign() {
        let coord = make_coordinator();
        let formation = coord.form_team(make_request()).await.unwrap();
        let session_id = formation.session.id.clone();

        let defs = vec![
            ("Write tests".to_string(), "TDD cycle".to_string(), vec![]),
            (
                "Implement".to_string(),
                "feature code".to_string(),
                vec!["Write tests".to_string()],
            ),
        ];
        let subtasks = coord.decompose_and_assign(&session_id, defs).await.unwrap();

        assert_eq!(subtasks.len(), 2);
        // Second subtask should depend on the first.
        assert_eq!(subtasks[1].depends_on, vec![subtasks[0].id.clone()]);

        let session = coord.get_session(&session_id).await.unwrap();
        assert_eq!(session.status, TeamSessionStatus::Planning);
        assert!(!session.coordination_log.is_empty());
    }

    #[tokio::test]
    async fn test_report_completion_updates_status() {
        let coord = make_coordinator();
        let formation = coord.form_team(make_request()).await.unwrap();
        let session_id = formation.session.id.clone();

        let defs = vec![("Sub A".to_string(), "do something".to_string(), vec![])];
        let subtasks = coord.decompose_and_assign(&session_id, defs).await.unwrap();
        let subtask_id = subtasks[0].id.clone();

        coord
            .report_completion(
                &session_id,
                &subtask_id,
                true,
                "done",
                vec![],
                "worker-1",
            )
            .await
            .unwrap();

        let session = coord.get_session(&session_id).await.unwrap();
        let sub = session.subtasks.iter().find(|s| s.id == subtask_id).unwrap();
        assert_eq!(sub.status, SubtaskStatus::Completed);
        assert!(sub.result.as_ref().unwrap().success);
    }

    #[tokio::test]
    async fn test_is_session_complete() {
        let coord = make_coordinator();
        let formation = coord.form_team(make_request()).await.unwrap();
        let session_id = formation.session.id.clone();

        assert!(!coord.is_session_complete(&session_id).await);

        let defs = vec![("Task".to_string(), "do it".to_string(), vec![])];
        let subtasks = coord.decompose_and_assign(&session_id, defs).await.unwrap();

        coord
            .report_completion(&session_id, &subtasks[0].id, true, "done", vec![], "w1")
            .await
            .unwrap();

        assert!(coord.is_session_complete(&session_id).await);
    }

    #[tokio::test]
    async fn test_conflict_resolution() {
        let coord = make_coordinator();
        let formation = coord.form_team(make_request()).await.unwrap();
        let session_id = formation.session.id.clone();

        let conflict = coord
            .resolve_conflict(
                &session_id,
                "workers disagree on approach",
                vec!["w1".to_string(), "w2".to_string()],
                "use approach A",
                "leader",
            )
            .await
            .unwrap();

        assert_eq!(conflict.session_id, session_id);
        let all = coord.all_conflicts().await;
        assert_eq!(all.len(), 1);

        let session = coord.get_session(&session_id).await.unwrap();
        assert!(session
            .coordination_log
            .iter()
            .any(|e| e.action == "conflict_resolved"));
    }
}
