//! Bridge Handler — Local CLI Execution
//!
//! Implements gRPC handler traits for the BridgeServer, delegating
//! task dispatch to the local `SubagentDispatcher`. This enables
//! `BridgeServer` to run as an independent service that executes
//! tasks via the local `claude` CLI.

use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tonic::Status;
use tracing::info;

use zn_bridge::proto;
use zn_bridge::{DispatchHandler, EvidenceHandler, StatusHandler};

use zn_types::{
    ExecutionMode, ExecutionOutcome, ExecutionPlan, HostKind, QualityGate, TaskItem,
    WorkspaceStrategy,
};

use crate::{execute_subagent_dispatch, SubagentExecutionOutcome};

/// Tracks running task state for status/ evidence queries
#[derive(Debug, Default)]
struct TaskTracker {
    agent_task_id: String,
    status: proto::TaskState,
    summary: String,
    artifacts: Vec<String>,
    progress_percent: i32,
    evidence_records: Vec<proto::EvidenceRecord>,
}

/// Local CLI handler for the gRPC bridge server.
/// Receives dispatched tasks and executes them via the local `claude` CLI.
pub struct LocalCliHandler {
    project_root: PathBuf,
    trackers: Arc<Mutex<std::collections::HashMap<String, TaskTracker>>>,
}

impl LocalCliHandler {
    pub fn new(project_root: &std::path::Path) -> Arc<Self> {
        Arc::new(Self {
            project_root: project_root.to_path_buf(),
            trackers: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
    }

    fn build_task_and_plan(req: &proto::DispatchRequest) -> (TaskItem, ExecutionPlan) {
        let mode = match req.mode {
            1 => ExecutionMode::Brainstorming,
            2 => ExecutionMode::SpecCapture,
            3 => ExecutionMode::WritingPlans,
            4 => ExecutionMode::WorkspacePrepare,
            5 => ExecutionMode::SubagentDev,
            6 => ExecutionMode::SubagentReview,
            7 => ExecutionMode::TddCycle,
            8 => ExecutionMode::Verification,
            9 => ExecutionMode::FinishBranch,
            _ => ExecutionMode::SubagentDev,
        };

        let strategy = match req.workspace_strategy {
            1 => WorkspaceStrategy::InPlace,
            2 => WorkspaceStrategy::GitWorktree,
            3 => WorkspaceStrategy::Sandboxed,
            _ => WorkspaceStrategy::InPlace,
        };

        let quality_gates: Vec<QualityGate> = req
            .quality_gates
            .iter()
            .map(|qg| QualityGate {
                name: qg.name.clone(),
                required: qg.required,
                description: qg.description.clone(),
            })
            .collect();

        let _host_kind: HostKind = if req.host_kind.is_empty() {
            HostKind::Terminal
        } else {
            serde_json::from_str(&format!("\"{}\"", req.host_kind)).unwrap_or(HostKind::Terminal)
        };

        let task = TaskItem {
            id: req.task_id.clone(),
            title: req.task_title.clone(),
            description: req.task_description.clone(),
            status: zn_types::TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: zn_types::TaskContract::default(),
            max_retries: Some(3),
            preconditions: vec![],
        };

        let plan = ExecutionPlan {
            task_id: req.task_id.clone(),
            objective: req.task_description.clone(),
            mode,
            workspace_strategy: strategy,
            steps: vec![],
            validation: vec![],
            quality_gates,
            skill_chain: vec![],
            deliverables: vec![],
            risks: vec![],
            subagents: vec![],
            worktree_plan: None,
            workspace_record: None,
            verification_actions: vec![],
            finish_branch_automation: None,
            execution_path: zn_types::SubagentExecutionPath::Cli,
            bridge_address: None,
        };

        (task, plan)
    }
}

#[async_trait]
impl DispatchHandler for LocalCliHandler {
    async fn dispatch_task(
        &self,
        request: proto::DispatchRequest,
    ) -> Result<proto::DispatchResponse> {
        let agent_task_id = format!("agent-{}", uuid::Uuid::new_v4().simple());
        let task_id = request.task_id.clone();

        info!(
            "LocalCliHandler: dispatching task {} as {}",
            task_id, agent_task_id
        );

        // Track the task
        {
            let mut trackers = self.trackers.lock().await;
            trackers.insert(
                task_id.clone(),
                TaskTracker {
                    agent_task_id: agent_task_id.clone(),
                    status: proto::TaskState::Running,
                    summary: "Dispatched to local CLI".to_string(),
                    artifacts: vec![],
                    progress_percent: 10,
                    evidence_records: vec![],
                },
            );
        }

        // Spawn execution in background (fire-and-forget, updates tracker on completion)
        let project_root = self.project_root.clone();
        let trackers = Arc::clone(&self.trackers);
        let tracking_task_id = task_id.clone();
        let dispatch_request = request.clone();

        tokio::task::spawn(async move {
            let (_task, plan) = LocalCliHandler::build_task_and_plan(&dispatch_request);

            // Run the subagent dispatch synchronously in a blocking thread
            let project_root = project_root.clone();
            let outcome = tokio::task::spawn_blocking(move || {
                execute_subagent_dispatch(&project_root, &plan)
            })
            .await;

            let mut trackers = trackers.lock().await;
            if let Ok(outcome) = outcome {
                let outcome: SubagentExecutionOutcome = outcome;
                let tracker = trackers.get_mut(&tracking_task_id);
                if let Some(tracker) = tracker {
                    let verdict = outcome.tri_role_verdict.clone();
                    if outcome.all_succeeded {
                        tracker.status = proto::TaskState::Completed;
                        tracker.summary = "All subagent dispatches succeeded".to_string();
                        tracker.progress_percent = 100;
                    } else {
                        tracker.status = proto::TaskState::Failed;
                        tracker.summary = outcome
                            .tri_role_verdict
                            .unwrap_or_else(|| "Subagent execution failed".to_string());
                        tracker.progress_percent = 0;
                    }
                    tracker.artifacts = outcome.artifact_paths.clone();

                    // Build evidence records from artifacts and verdict
                    for path in &outcome.artifact_paths {
                        tracker.evidence_records.push(proto::EvidenceRecord {
                            id: format!("evidence-{}", uuid::Uuid::new_v4().simple()),
                            task_id: tracking_task_id.clone(),
                            kind: "artifact".to_string(),
                            content: String::new(),
                            file_path: path.clone(),
                            timestamp: chrono::Utc::now().timestamp(),
                        });
                    }
                    if let Some(verdict_str) = verdict {
                        tracker.evidence_records.push(proto::EvidenceRecord {
                            id: format!("verdict-{}", uuid::Uuid::new_v4().simple()),
                            task_id: tracking_task_id.clone(),
                            kind: "verdict".to_string(),
                            content: verdict_str,
                            file_path: String::new(),
                            timestamp: chrono::Utc::now().timestamp(),
                        });
                    }
                }
            } else {
                if let Some(tracker) = trackers.get_mut(&tracking_task_id) {
                    tracker.status = proto::TaskState::Failed;
                    tracker.summary = "Execution runtime error".to_string();
                }
            }
        });

        Ok(proto::DispatchResponse {
            agent_task_id,
            status: proto::DispatchStatus::Accepted as i32,
            message: "Task dispatched to local CLI executor".to_string(),
        })
    }

    async fn cancel_task(&self, request: proto::CancelRequest) -> Result<proto::CancelResponse> {
        info!(
            "LocalCliHandler: cancel task {} ({})",
            request.task_id, request.reason
        );

        {
            let mut trackers = self.trackers.lock().await;
            if let Some(tracker) = trackers.get_mut(&request.task_id) {
                tracker.status = proto::TaskState::Cancelled;
                tracker.summary = format!("Cancelled: {}", request.reason);
            }
        }

        Ok(proto::CancelResponse {
            success: true,
            message: "Task cancellation requested".to_string(),
        })
    }
}

#[async_trait]
impl StatusHandler for LocalCliHandler {
    async fn get_status(&self, request: proto::StatusRequest) -> Result<proto::StatusResponse> {
        let trackers = self.trackers.lock().await;
        if let Some(tracker) = trackers.get(&request.task_id) {
            Ok(proto::StatusResponse {
                task_id: request.task_id,
                state: tracker.status as i32,
                summary: tracker.summary.clone(),
                artifacts: tracker.artifacts.clone(),
                progress_percent: tracker.progress_percent,
                error_message: String::new(),
            })
        } else {
            Err(anyhow::anyhow!("Task {} not found", request.task_id).into())
        }
    }

    async fn stream_status(
        &self,
        request: proto::StatusRequest,
    ) -> Result<mpsc::Receiver<Result<proto::StatusUpdate, Status>>> {
        let (tx, rx) = mpsc::channel(32);

        let trackers = self.trackers.lock().await;
        if let Some(tracker) = trackers.get(&request.task_id) {
            let _ = tx
                .send(Ok(proto::StatusUpdate {
                    state: tracker.status as i32,
                    summary: tracker.summary.clone(),
                    progress_percent: tracker.progress_percent,
                    new_artifacts: tracker.artifacts.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                }))
                .await;
        }

        Ok(rx)
    }
}

#[async_trait]
impl EvidenceHandler for LocalCliHandler {
    async fn stream_evidence(
        &self,
        request: proto::EvidenceRequest,
    ) -> Result<mpsc::Receiver<Result<proto::EvidenceRecord, Status>>> {
        let (tx, rx) = mpsc::channel(32);

        let trackers = self.trackers.lock().await;
        if let Some(tracker) = trackers.get(&request.task_id) {
            for record in &tracker.evidence_records {
                let _ = tx.send(Ok(record.clone())).await;
            }
        }
        // Channel closes after sending all buffered evidence
        Ok(rx)
    }

    async fn submit_evidence(
        &self,
        request: proto::SubmitEvidenceRequest,
    ) -> Result<proto::SubmitEvidenceResponse> {
        let evidence_paths: Vec<String> = request
            .evidence
            .iter()
            .filter_map(|e| {
                if !e.file_path.is_empty() {
                    Some(e.file_path.clone())
                } else {
                    None
                }
            })
            .collect();

        // Update task state
        {
            let mut trackers = self.trackers.lock().await;
            if let Some(tracker) = trackers.get_mut(&request.task_id) {
                tracker.status = proto::TaskState::try_from(request.final_state)
                    .unwrap_or(proto::TaskState::Unknown);
                tracker.summary = request.summary.clone();
            }
        }

        Ok(proto::SubmitEvidenceResponse {
            success: true,
            evidence_paths,
            message: "Evidence received".to_string(),
        })
    }
}
