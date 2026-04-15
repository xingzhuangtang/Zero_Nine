//! gRPC Bridge Client for dispatching tasks to agents

use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::time::Duration;
use tonic::transport::Channel;
use tracing::{debug, warn};

use zn_bridge::proto::{
    task_dispatch_client::TaskDispatchClient,
    task_status_client::TaskStatusClient,
    evidence_stream_client::EvidenceStreamClient,
    DispatchRequest, TaskState,
};
use zn_bridge::types::{zn_execution_mode_to_proto, zn_workspace_strategy_to_proto, zn_quality_gate_to_proto};
use zn_types::{ExecutionPlan, TaskItem};

/// gRPC Bridge Client for communicating with agents
pub struct BridgeClient {
    dispatch_client: TaskDispatchClient<Channel>,
    status_client: TaskStatusClient<Channel>,
    evidence_client: EvidenceStreamClient<Channel>,
}

impl BridgeClient {
    /// Connect to a gRPC bridge server at the specified address
    pub async fn connect(addr: SocketAddr) -> Result<Self> {
        let channel = Channel::from_shared(format!("http://{}", addr))
            .context("failed to create gRPC channel")?
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .connect()
            .await
            .context("failed to connect to gRPC server")?;

        Ok(Self {
            dispatch_client: TaskDispatchClient::new(channel.clone()),
            status_client: TaskStatusClient::new(channel.clone()),
            evidence_client: EvidenceStreamClient::new(channel),
        })
    }

    /// Dispatch a task to an agent via gRPC
    pub async fn dispatch_task(
        &mut self,
        task: &TaskItem,
        proposal_id: &str,
        plan: &ExecutionPlan,
        context_files: Vec<String>,
    ) -> Result<String> {
        let mode = plan.mode.clone();
        let workspace_strategy = plan.workspace_strategy.clone();
        let quality_gates = plan.quality_gates.iter().map(|g| {
            zn_quality_gate_to_proto(g)
        }).collect::<Vec<_>>();

        let request = DispatchRequest {
            task_id: task.id.clone(),
            proposal_id: proposal_id.to_string(),
            task_title: task.title.clone(),
            task_description: task.description.clone(),
            context_files,
            mode: zn_execution_mode_to_proto(mode) as i32,
            workspace_strategy: zn_workspace_strategy_to_proto(workspace_strategy) as i32,
            quality_gates,
            host_kind: "claude_code".to_string(),
        };

        let response = self.dispatch_client
            .dispatch_task(request)
            .await
            .context("failed to dispatch task")?
            .into_inner();

        debug!("Dispatched task {} -> agent_task_id={}", task.id, response.agent_task_id);
        Ok(response.agent_task_id)
    }

    /// Wait for task completion and collect results
    pub async fn wait_for_task(
        &mut self,
        task_id: &str,
        agent_task_id: &str,
        timeout_secs: u64,
    ) -> Result<TaskResult> {
        use tokio::time::{timeout, interval};
        use tokio_stream::StreamExt;

        let mut poll_interval = interval(Duration::from_secs(2));
        let deadline = Duration::from_secs(timeout_secs);

        let mut status_rx = {
            let request = zn_bridge::proto::StatusRequest {
                task_id: task_id.to_string(),
                agent_task_id: agent_task_id.to_string(),
            };
            let response = self.status_client
                .stream_status(request)
                .await
                .context("failed to stream status")?
                .into_inner();
            response
        };

        let mut final_state = TaskState::Unknown as i32;
        let mut summary = String::new();
        let mut artifacts = Vec::new();

        let result = timeout(deadline, async {
            loop {
                tokio::select! {
                    status_msg = status_rx.next() => {
                        match status_msg {
                            Some(Ok(status)) => {
                                debug!("Task {} status: {:?} ({}%)", task_id, status.state, status.progress_percent);
                                final_state = status.state;
                                summary = status.summary;
                                artifacts = status.new_artifacts;

                                if matches!(
                                    status.state,
                                    x if x == TaskState::Completed as i32
                                        || x == TaskState::Failed as i32
                                        || x == TaskState::Cancelled as i32
                                ) {
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                warn!("Status stream error: {}", e);
                            }
                            None => {
                                debug!("Status stream ended");
                                break;
                            }
                        }
                    }
                }
                poll_interval.tick().await;
            }

            TaskResult {
                task_id: task_id.to_string(),
                state: final_state,
                summary,
                artifacts,
                evidence: Vec::new(),
            }
        }).await;

        match result {
            Ok(task_result) => Ok(task_result),
            Err(_) => Err(anyhow::anyhow!("Task {} timed out after {} seconds", task_id, timeout_secs)),
        }
    }

    /// Collect evidence for a completed task
    pub async fn collect_evidence(
        &mut self,
        task_id: &str,
        agent_task_id: &str,
    ) -> Result<Vec<zn_bridge::proto::EvidenceRecord>> {
        use tokio_stream::StreamExt;

        let request = zn_bridge::proto::EvidenceRequest {
            task_id: task_id.to_string(),
            agent_task_id: agent_task_id.to_string(),
        };

        let mut stream = self.evidence_client
            .stream_evidence(request)
            .await
            .context("failed to stream evidence")?
            .into_inner();

        let mut evidence = Vec::new();
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(record) => {
                    debug!("Collected evidence: {} ({})", record.id, record.kind);
                    evidence.push(record);
                }
                Err(e) => {
                    warn!("Evidence stream error: {}", e);
                }
            }
        }

        Ok(evidence)
    }
}

/// Result of a dispatched task
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub state: i32,  // TaskState enum value
    pub summary: String,
    pub artifacts: Vec<String>,
    pub evidence: Vec<zn_bridge::proto::EvidenceRecord>,
}

/// Check if a TaskState i32 value represents success
pub fn task_state_is_success(state: i32) -> bool {
    state == TaskState::Completed as i32
}

/// Check if a TaskState i32 value represents failure
pub fn task_state_is_failure(state: i32) -> bool {
    state == TaskState::Failed as i32 || state == TaskState::Cancelled as i32
}
