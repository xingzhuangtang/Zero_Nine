//! Zero_Nine shared types library.
//!
//! This crate defines all data models used across the Zero_Nine workspace.
//! Types are organized into thematic modules but re-exported at the crate root
//! to maintain backward compatibility with existing `use zn_types::*` imports.

pub mod core;
pub mod error;
pub mod state_machine;
pub mod proposal;
pub mod drift;
pub mod execution;
pub mod governance;
pub mod evolution;
pub mod github;

// ==================== Re-exports ====================
// All types are re-exported at the crate root for backward compatibility.
// Downstream crates can continue using `use zn_types::TypeName;` without changes.

pub use core::*;
pub use error::*;
pub use state_machine::*;
pub use proposal::*;
pub use drift::*;
pub use execution::*;
pub use governance::*;
pub use evolution::*;
pub use github::*;

// ==================== Tests ====================
// Tests remain in lib.rs to validate cross-module integration.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_dag_passes_validation() {
        let graph = TaskGraph {
            schema_version: default_spec_schema_version(),
            proposal_id: "test-1".to_string(),
            tasks: vec![
                TaskItem {
                    id: "1".to_string(),
                    title: "Task 1".to_string(),
                    description: "First task".to_string(),
                    status: TaskStatus::Pending,
                    depends_on: vec![],
                    kind: None,
                    contract: TaskContract::default(),
                    max_retries: None,
                    preconditions: vec![],
                },
                TaskItem {
                    id: "2".to_string(),
                    title: "Task 2".to_string(),
                    description: "Second task".to_string(),
                    status: TaskStatus::Pending,
                    depends_on: vec!["1".to_string()],
                    kind: None,
                    contract: TaskContract::default(),
                    max_retries: None,
                    preconditions: vec![],
                },
            ],
            edges: vec![],
        };

        let result = graph.validate_dag();
        assert!(result.valid);
        assert!(result.errors.is_empty());
        assert_eq!(result.max_depth, 2);
    }

    #[test]
    fn test_circular_dependency_detected() {
        let graph = TaskGraph {
            schema_version: default_spec_schema_version(),
            proposal_id: "test-2".to_string(),
            tasks: vec![
                TaskItem {
                    id: "1".to_string(),
                    title: "Task 1".to_string(),
                    description: "First task".to_string(),
                    status: TaskStatus::Pending,
                    depends_on: vec!["3".to_string()],
                    kind: None,
                    contract: TaskContract::default(),
                    max_retries: None,
                    preconditions: vec![],
                },
                TaskItem {
                    id: "2".to_string(),
                    title: "Task 2".to_string(),
                    description: "Second task".to_string(),
                    status: TaskStatus::Pending,
                    depends_on: vec!["1".to_string()],
                    kind: None,
                    contract: TaskContract::default(),
                    max_retries: None,
                    preconditions: vec![],
                },
                TaskItem {
                    id: "3".to_string(),
                    title: "Task 3".to_string(),
                    description: "Third task".to_string(),
                    status: TaskStatus::Pending,
                    depends_on: vec!["2".to_string()],
                    kind: None,
                    contract: TaskContract::default(),
                    max_retries: None,
                    preconditions: vec![],
                },
            ],
            edges: vec![],
        };

        let result = graph.validate_dag();
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e.error_code, DagErrorCode::CircularDependency)));
    }

    #[test]
    fn test_missing_dependency_detected() {
        let graph = TaskGraph {
            schema_version: default_spec_schema_version(),
            proposal_id: "test-3".to_string(),
            tasks: vec![TaskItem {
                id: "1".to_string(),
                title: "Task 1".to_string(),
                description: "First task".to_string(),
                status: TaskStatus::Pending,
                depends_on: vec!["non-existent".to_string()],
                kind: None,
                contract: TaskContract::default(),
                max_retries: None,
                preconditions: vec![],
            }],
            edges: vec![],
        };

        let result = graph.validate_dag();
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e.error_code, DagErrorCode::MissingDependency)));
    }

    #[test]
    fn test_self_reference_detected() {
        let graph = TaskGraph {
            schema_version: default_spec_schema_version(),
            proposal_id: "test-4".to_string(),
            tasks: vec![TaskItem {
                id: "1".to_string(),
                title: "Task 1".to_string(),
                description: "First task".to_string(),
                status: TaskStatus::Pending,
                depends_on: vec!["1".to_string()],
                kind: None,
                contract: TaskContract::default(),
                max_retries: None,
                preconditions: vec![],
            }],
            edges: vec![],
        };

        let result = graph.validate_dag();
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e.error_code, DagErrorCode::SelfReference)));
    }

    #[test]
    fn test_empty_graph_detected() {
        let graph = TaskGraph {
            schema_version: default_spec_schema_version(),
            proposal_id: "test-5".to_string(),
            tasks: vec![],
            edges: vec![],
        };

        let result = graph.validate_dag();
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e.error_code, DagErrorCode::EmptyTaskGraph)));
    }

    #[test]
    fn test_duplicate_task_id_detected() {
        let graph = TaskGraph {
            schema_version: default_spec_schema_version(),
            proposal_id: "test-6".to_string(),
            tasks: vec![
                TaskItem {
                    id: "1".to_string(),
                    title: "Task 1".to_string(),
                    description: "First task".to_string(),
                    status: TaskStatus::Pending,
                    depends_on: vec![],
                    kind: None,
                    contract: TaskContract::default(),
                    max_retries: None,
                    preconditions: vec![],
                },
                TaskItem {
                    id: "1".to_string(),
                    title: "Duplicate Task".to_string(),
                    description: "Duplicate task".to_string(),
                    status: TaskStatus::Pending,
                    depends_on: vec![],
                    kind: None,
                    contract: TaskContract::default(),
                    max_retries: None,
                    preconditions: vec![],
                },
            ],
            edges: vec![],
        };

        let result = graph.validate_dag();
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e.error_code, DagErrorCode::DuplicateTaskId)));
    }

    #[test]
    fn test_critical_path_computation() {
        let graph = TaskGraph {
            schema_version: default_spec_schema_version(),
            proposal_id: "test-7".to_string(),
            tasks: vec![
                TaskItem {
                    id: "1".to_string(),
                    title: "Task 1".to_string(),
                    description: "First task".to_string(),
                    status: TaskStatus::Pending,
                    depends_on: vec![],
                    kind: None,
                    contract: TaskContract::default(),
                    max_retries: None,
                    preconditions: vec![],
                },
                TaskItem {
                    id: "2".to_string(),
                    title: "Task 2".to_string(),
                    description: "Second task".to_string(),
                    status: TaskStatus::Pending,
                    depends_on: vec!["1".to_string()],
                    kind: None,
                    contract: TaskContract::default(),
                    max_retries: None,
                    preconditions: vec![],
                },
                TaskItem {
                    id: "3".to_string(),
                    title: "Task 3".to_string(),
                    description: "Third task".to_string(),
                    status: TaskStatus::Pending,
                    depends_on: vec!["2".to_string()],
                    kind: None,
                    contract: TaskContract::default(),
                    max_retries: None,
                    preconditions: vec![],
                },
            ],
            edges: vec![],
        };

        let result = graph.validate_dag();
        assert!(result.valid);
        assert_eq!(result.critical_path, vec!["1", "2", "3"]);
        assert_eq!(result.max_depth, 3);
    }
}

#[cfg(test)]
mod blueprint_tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_failure_classification() {
        let f = FailureClassification::default();
        assert_eq!(f.category, FailureCategory::Unknown);
    }
    #[test]
    fn test_verdict() {
        let v = Verdict::default();
        assert_eq!(v.status, VerdictStatus::Warning);
    }
    #[test]
    fn test_policy_rule() {
        let r = PolicyRule::default();
        assert_eq!(r.default_decision, PolicyDecision::Ask);
    }
    #[test]
    fn test_human_intervention() {
        let h = HumanIntervention::default();
        assert_eq!(h.action, SupervisionAction::Approve);
    }
    #[test]
    fn test_approval_ticket() {
        let a = ApprovalTicket::default();
        assert_eq!(a.status, ApprovalStatus::Pending);
    }
    #[test]
    fn test_skill_version() {
        let v = SkillVersion::default();
        assert_eq!(v.major, 1);
    }
    #[test]
    fn test_skill_bundle() {
        let b = SkillBundle::default();
        assert_eq!(b.usage_count, 0);
    }
    #[test]
    fn test_agent_role() {
        let r = AgentRole::default();
        assert_eq!(r, AgentRole::Executor);
    }
    #[test]
    fn test_multi_agent_orchestration() {
        let o = MultiAgentOrchestration::default();
        assert_eq!(o.dispatches.len(), 0);
    }

    // M6: Issue source tracking
    #[test]
    fn test_issue_source_serialization() {
        let github = IssueSource::GitHub;
        let local = IssueSource::Local;
        let manual = IssueSource::Manual;

        assert_eq!(serde_json::to_string(&github).unwrap(), "\"github\"");
        assert_eq!(serde_json::to_string(&local).unwrap(), "\"local\"");
        assert_eq!(serde_json::to_string(&manual).unwrap(), "\"manual\"");
    }

    #[test]
    fn test_proposal_source_tracking() {
        let mut proposal = Proposal::default();
        assert!(proposal.source_issue_number.is_none());
        proposal.source_issue_number = Some(42);
        proposal.source_repo = Some("owner/repo".to_string());
        proposal.source_type = Some(IssueSource::GitHub);
        assert_eq!(proposal.source_issue_number, Some(42));
    }

    #[test]
    fn test_proposal_source_roundtrip() {
        let mut proposal = Proposal::default();
        proposal.id = "test".to_string();
        proposal.source_issue_number = Some(99);
        proposal.source_repo = Some("org/repo".to_string());
        proposal.source_type = Some(IssueSource::Local);
        let json = serde_json::to_string(&proposal).unwrap();
        let restored: Proposal = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.source_issue_number, Some(99));
    }

    #[test]
    fn test_issue_mapping_serialization() {
        let mapping = IssueMapping {
            issue_number: 123,
            repo: "owner/repo".to_string(),
            proposal_id: "proposal-abc".to_string(),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&mapping).unwrap();
        let restored: IssueMapping = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.issue_number, 123);
    }

    #[test]
    fn test_github_issue_parsing() {
        let json = r#"{"number":42,"title":"Test","body":"Desc","state":"open","labels":[{"name":"bug"}],"assignees":[{"login":"dev"}],"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}"#;
        let issue: GitHubIssue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.number, 42);
        assert_eq!(issue.labels[0].name, "bug");
    }

    // M7: State machine tests
    #[test]
    fn test_state_machine_allowed_transitions() {
        assert_eq!(
            LoopStage::Ready.allowed_transitions(),
            vec![LoopStage::RunningTask, LoopStage::Archived]
        );
        assert!(LoopStage::Completed.allowed_transitions().is_empty());
    }

    #[test]
    fn test_state_machine_can_transition() {
        assert!(LoopStage::Ready.can_transition_to(LoopStage::RunningTask));
        assert!(LoopStage::RunningTask.can_transition_to(LoopStage::Verifying));
        assert!(LoopStage::Verifying.can_transition_to(LoopStage::Completed));
        assert!(LoopStage::Archived.can_transition_to(LoopStage::Ready));
        assert!(!LoopStage::Completed.can_transition_to(LoopStage::RunningTask));
        assert!(!LoopStage::Ready.can_transition_to(LoopStage::Completed));
    }

    #[test]
    fn test_state_machine_transition_to_valid() {
        let t = LoopStage::Ready
            .transition_to(LoopStage::RunningTask, "task_started", Some("task-1"))
            .unwrap();
        assert_eq!(t.from, "Ready");
        assert_eq!(t.to, "RunningTask");
        assert_eq!(t.task_id, Some("task-1".to_string()));
    }

    #[test]
    fn test_state_machine_transition_to_invalid() {
        let result = LoopStage::Completed.transition_to(LoopStage::RunningTask, "resume", None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.from, LoopStage::Completed);
        assert!(err.allowed.is_empty());
    }

    #[test]
    fn test_state_transition_serialization() {
        let t = LoopStage::RunningTask
            .transition_to(LoopStage::Verifying, "task_completed", Some("task-1"))
            .unwrap();
        let json = serde_json::to_string(&t).unwrap();
        let restored: StateTransition = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.stage_from, LoopStage::RunningTask);
        assert_eq!(restored.stage_to, LoopStage::Verifying);
        assert_eq!(restored.reason, "task_completed");
    }

    #[test]
    fn test_loop_stage_completed_is_terminal() {
        assert!(!LoopStage::Completed.can_transition_to(LoopStage::RunningTask));
        assert!(!LoopStage::Completed.can_transition_to(LoopStage::Verifying));
        assert!(!LoopStage::Completed.can_transition_to(LoopStage::Archived));
    }

    #[test]
    fn test_subagent_execution_path_serialization() {
        // Test Cli
        let path = SubagentExecutionPath::Cli;
        let json = serde_json::to_string(&path).unwrap();
        assert_eq!(json, "\"cli\"");
        assert_eq!(
            serde_json::from_str::<SubagentExecutionPath>(&json).unwrap(),
            path
        );

        // Test Bridge
        let path = SubagentExecutionPath::Bridge;
        let json = serde_json::to_string(&path).unwrap();
        assert_eq!(json, "\"bridge\"");
        assert_eq!(
            serde_json::from_str::<SubagentExecutionPath>(&json).unwrap(),
            path
        );

        // Test Hybrid
        let path = SubagentExecutionPath::Hybrid;
        let json = serde_json::to_string(&path).unwrap();
        assert_eq!(json, "\"hybrid\"");
        assert_eq!(
            serde_json::from_str::<SubagentExecutionPath>(&json).unwrap(),
            path
        );

        // Test default
        let default = SubagentExecutionPath::default();
        assert_eq!(default, SubagentExecutionPath::Cli);
    }

    #[test]
    fn test_project_manifest_bridge_address() {
        let manifest = ProjectManifest {
            bridge_address: Some("127.0.0.1:50051".to_string()),
            ..Default::default()
        };
        assert_eq!(manifest.bridge_address, Some("127.0.0.1:50051".to_string()));

        // Test serialization roundtrip
        let json = serde_json::to_string(&manifest).unwrap();
        let restored: ProjectManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.bridge_address, Some("127.0.0.1:50051".to_string()));
    }

    #[test]
    fn test_execution_plan_bridge_fields() {
        let plan = ExecutionPlan {
            task_id: "test".to_string(),
            objective: "test".to_string(),
            mode: ExecutionMode::SubagentDev,
            workspace_strategy: WorkspaceStrategy::InPlace,
            steps: vec![],
            validation: vec![],
            quality_gates: vec![],
            skill_chain: vec![],
            deliverables: vec![],
            risks: vec![],
            subagents: vec![],
            worktree_plan: None,
            workspace_record: None,
            verification_actions: vec![],
            finish_branch_automation: None,
            execution_path: SubagentExecutionPath::Bridge,
            bridge_address: Some("127.0.0.1:50051".to_string()),
        };

        assert_eq!(plan.execution_path, SubagentExecutionPath::Bridge);
        assert_eq!(plan.bridge_address, Some("127.0.0.1:50051".to_string()));
    }
}
