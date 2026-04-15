//! Example gRPC Bridge Server
//!
//! Run this example with:
//! `cargo run --package zn-bridge --example grpc-server`
//!
//! This starts a gRPC server on 127.0.0.1:50051 that accepts task dispatch,
//! status streaming, and evidence submission from agents.

use anyhow::Result;
use tokio::sync::mpsc;
use tonic::Request;
use zn_bridge::proto::{
    task_dispatch_server::TaskDispatchServer,
    task_status_server::TaskStatusServer,
    evidence_stream_server::EvidenceStreamServer,
    DispatchRequest, DispatchResponse, DispatchStatus,
    CancelRequest, CancelResponse,
    StatusRequest, StatusResponse, StatusUpdate,
    EvidenceRequest, EvidenceRecord, SubmitEvidenceRequest, SubmitEvidenceResponse,
    TaskState,
};
use zn_bridge::server::{BridgeState, TaskState as ServerTaskState};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tokio_stream::wrappers::ReceiverStream;

#[tokio::main]
async fn main() -> Result<()> {
    use tracing_subscriber::FmtSubscriber;
    use tracing::Level;

    // Initialize logging
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .init();

    let addr = "127.0.0.1:50051".parse()?;
    tracing::info!("Starting example gRPC bridge server on {}", addr);

    // Create a simple mock service
    let service = MockBridgeService::new();

    // Build the gRPC server
    tonic::transport::Server::builder()
        .add_service(TaskDispatchServer::new(service.clone()))
        .add_service(TaskStatusServer::new(service.clone()))
        .add_service(EvidenceStreamServer::new(service.clone()))
        .serve(addr)
        .await?;

    Ok(())
}

/// A mock bridge service that simulates task execution
#[derive(Clone, Debug)]
struct MockBridgeService {
    state: Arc<BridgeState>,
}

impl MockBridgeService {
    fn new() -> Self {
        Self {
            state: Arc::new(BridgeState::default()),
        }
    }
}

#[tonic::async_trait]
impl zn_bridge::proto::task_dispatch_server::TaskDispatch for MockBridgeService {
    async fn dispatch_task(
        &self,
        request: Request<DispatchRequest>,
    ) -> Result<tonic::Response<DispatchResponse>, tonic::Status> {
        let req = request.into_inner();
        tracing::info!("DispatchTask: task_id={}, proposal_id={}", req.task_id, req.proposal_id);

        let agent_task_id = format!("mock-agent-{}", uuid::Uuid::new_v4().simple());

        // Store task state
        {
            let mut tasks = self.state.tasks.write().await;
            tasks.insert(
                req.task_id.clone(),
                ServerTaskState {
                    task_id: req.task_id.clone(),
                    agent_task_id: Some(agent_task_id.clone()),
                    status: TaskState::Running,
                    summary: "Task dispatched to mock agent".to_string(),
                    artifacts: Vec::new(),
                    progress_percent: 0,
                },
            );
        }

        // Simulate task completion in background
        let state = self.state.clone();
        let task_id = req.task_id.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1));
            for progress in 0..=100 {
                interval.tick().await;

                let mut tasks = state.tasks.write().await;
                if let Some(task_state) = tasks.get_mut(&task_id) {
                    task_state.progress_percent = progress as i32;
                    if progress == 100 {
                        task_state.status = TaskState::Completed;
                        task_state.summary = "Task completed by mock agent".to_string();
                        task_state.artifacts = vec!["/tmp/mock-artifact.txt".to_string()];
                        break;
                    }
                }
            }
        });

        Ok(tonic::Response::new(DispatchResponse {
            agent_task_id: agent_task_id.clone(),
            status: DispatchStatus::Accepted as i32,
            message: "Task accepted by mock agent".to_string(),
        }))
    }

    async fn cancel_task(
        &self,
        request: Request<CancelRequest>,
    ) -> Result<tonic::Response<CancelResponse>, tonic::Status> {
        let req = request.into_inner();
        tracing::info!("CancelTask: task_id={}, reason={}", req.task_id, req.reason);

        Ok(tonic::Response::new(CancelResponse {
            success: true,
            message: "Task cancelled".to_string(),
        }))
    }
}

#[tonic::async_trait]
impl zn_bridge::proto::task_status_server::TaskStatus for MockBridgeService {
    async fn get_status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<tonic::Response<StatusResponse>, tonic::Status> {
        let req = request.into_inner();
        tracing::debug!("GetStatus: task_id={}", req.task_id);

        let tasks = self.state.tasks.read().await;
        if let Some(state) = tasks.get(&req.task_id) {
            Ok(tonic::Response::new(StatusResponse {
                task_id: state.task_id.clone(),
                state: state.status as i32,
                summary: state.summary.clone(),
                artifacts: state.artifacts.clone(),
                progress_percent: state.progress_percent,
                error_message: String::new(),
            }))
        } else {
            Err(tonic::Status::not_found(format!("Task {} not found", req.task_id)))
        }
    }

    type StreamStatusStream = ReceiverStream<Result<StatusUpdate, tonic::Status>>;

    async fn stream_status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<tonic::Response<Self::StreamStatusStream>, tonic::Status> {
        let req = request.into_inner();
        tracing::info!("StreamStatus: task_id={}", req.task_id);

        let (tx, rx) = mpsc::channel(32);

        // Store sender for later updates
        {
            let mut senders: std::collections::HashMap<_, mpsc::Sender<Result<StatusUpdate, tonic::Status>>> = self.state.status_senders.write().await.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            senders.insert(req.task_id.clone(), tx.clone());

            let mut actual_senders = self.state.status_senders.write().await;
            actual_senders.insert(req.task_id.clone(), tx.clone());
        }

        // Stream status updates
        let state = self.state.clone();
        let task_id = req.task_id.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(500));
            let mut last_progress = 0;

            while last_progress < 100 {
                interval.tick().await;

                let tasks = state.tasks.read().await;
                if let Some(task_state) = tasks.get(&task_id) {
                    let progress = task_state.progress_percent;
                    if progress > last_progress {
                        let _ = tx.send(Ok(StatusUpdate {
                            state: task_state.status as i32,
                            summary: task_state.summary.clone(),
                            progress_percent: progress,
                            new_artifacts: task_state.artifacts.clone(),
                            timestamp: chrono::Utc::now().timestamp(),
                        })).await;
                        last_progress = progress;
                    }

                    if task_state.status == TaskState::Completed
                        || task_state.status == TaskState::Failed
                        || task_state.status == TaskState::Cancelled
                    {
                        break;
                    }
                }
            }
        });

        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }
}

#[tonic::async_trait]
impl zn_bridge::proto::evidence_stream_server::EvidenceStream for MockBridgeService {
    type StreamEvidenceStream = ReceiverStream<Result<EvidenceRecord, tonic::Status>>;

    async fn stream_evidence(
        &self,
        request: Request<EvidenceRequest>,
    ) -> Result<tonic::Response<Self::StreamEvidenceStream>, tonic::Status> {
        let req = request.into_inner();
        tracing::info!("StreamEvidence: task_id={}", req.task_id);

        let (tx, rx) = mpsc::channel(32);

        // Send mock evidence
        tokio::spawn(async move {
            let evidence = vec![
                EvidenceRecord {
                    id: "evidence-1".to_string(),
                    kind: "test_output".to_string(),
                    content: "All tests passed".to_string(),
                    file_path: "/tmp/test-output.txt".to_string(),
                    timestamp: chrono::Utc::now().timestamp(),
                    task_id: req.task_id.clone(),
                },
                EvidenceRecord {
                    id: "evidence-2".to_string(),
                    kind: "code_diff".to_string(),
                    content: "Changes applied".to_string(),
                    file_path: "/tmp/code.diff".to_string(),
                    timestamp: chrono::Utc::now().timestamp(),
                    task_id: req.task_id.clone(),
                },
            ];

            for record in evidence {
                let _ = tx.send(Ok(record)).await;
            }
        });

        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }

    async fn submit_evidence(
        &self,
        request: Request<SubmitEvidenceRequest>,
    ) -> Result<tonic::Response<SubmitEvidenceResponse>, tonic::Status> {
        let req = request.into_inner();
        tracing::info!("SubmitEvidence: task_id={}, count={}", req.task_id, req.evidence.len());

        let evidence_paths: Vec<String> = req.evidence
            .iter()
            .filter(|e| !e.file_path.is_empty())
            .map(|e| e.file_path.clone())
            .collect();

        Ok(tonic::Response::new(SubmitEvidenceResponse {
            success: true,
            evidence_paths,
            message: "Evidence submitted".to_string(),
        }))
    }
}
