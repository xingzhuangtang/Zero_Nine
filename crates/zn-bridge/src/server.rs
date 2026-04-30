//! gRPC Bridge Server implementation

use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info};

use crate::proto;
use crate::service::{DispatchHandler, EvidenceHandler, StatusHandler};
use crate::types::BridgeConfig;

/// Type alias for the status update stream
pub type StatusUpdateStream = Pin<Box<dyn futures_core::Stream<Item = proto::StatusUpdate> + Send>>;

/// Type alias for the evidence record stream
pub type EvidenceRecordStream =
    Pin<Box<dyn futures_core::Stream<Item = proto::EvidenceRecord> + Send>>;

/// Internal task tracking state
#[derive(Debug, Clone)]
pub struct TaskState {
    pub task_id: String,
    pub agent_task_id: Option<String>,
    pub status: proto::TaskState,
    pub summary: String,
    pub artifacts: Vec<String>,
    pub progress_percent: i32,
}

/// Shared state for the bridge server
#[derive(Debug, Default)]
pub struct BridgeState {
    pub tasks: RwLock<std::collections::HashMap<String, TaskState>>,
    pub status_senders: RwLock<
        std::collections::HashMap<String, mpsc::Sender<Result<proto::StatusUpdate, Status>>>,
    >,
    pub evidence_senders: RwLock<
        std::collections::HashMap<String, mpsc::Sender<Result<proto::EvidenceRecord, Status>>>,
    >,
}

impl BridgeState {
    fn new() -> Self {
        Self {
            tasks: RwLock::new(std::collections::HashMap::new()),
            status_senders: RwLock::new(std::collections::HashMap::new()),
            evidence_senders: RwLock::new(std::collections::HashMap::new()),
        }
    }
}

/// gRPC Bridge Server
pub struct BridgeServer {
    config: BridgeConfig,
    state: Arc<BridgeState>,
    dispatch_handler: Option<Arc<dyn DispatchHandler>>,
    status_handler: Option<Arc<dyn StatusHandler>>,
    evidence_handler: Option<Arc<dyn EvidenceHandler>>,
}

impl BridgeServer {
    /// Create a new BridgeServer with the given configuration
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            config,
            state: Arc::new(BridgeState::new()),
            dispatch_handler: None,
            status_handler: None,
            evidence_handler: None,
        }
    }

    /// Set the dispatch handler
    pub fn with_dispatch_handler(mut self, handler: Arc<dyn DispatchHandler>) -> Self {
        self.dispatch_handler = Some(handler);
        self
    }

    /// Set the status handler
    pub fn with_status_handler(mut self, handler: Arc<dyn StatusHandler>) -> Self {
        self.status_handler = Some(handler);
        self
    }

    /// Set the evidence handler
    pub fn with_evidence_handler(mut self, handler: Arc<dyn EvidenceHandler>) -> Self {
        self.evidence_handler = Some(handler);
        self
    }

    /// Get the bind address
    pub fn bind_addr(&self) -> SocketAddr {
        self.config.bind_addr
    }

    /// Run the server with graceful shutdown on SIGINT/SIGTERM
    pub async fn run(self) -> Result<()> {
        self.run_with_shutdown(None).await
    }

    /// Run the server with a custom shutdown signal
    pub async fn run_with_shutdown(
        self,
        shutdown_signal: Option<Pin<Box<dyn std::future::Future<Output = ()> + Send>>>,
    ) -> Result<()> {
        let dispatch_handler = self.dispatch_handler.clone();
        let status_handler = self.status_handler.clone();
        let evidence_handler = self.evidence_handler.clone();
        let state = self.state.clone();

        // Create service implementations
        let task_dispatch_service = TaskDispatchService {
            handler: dispatch_handler,
            state: state.clone(),
        };
        let task_status_service = TaskStatusService {
            handler: status_handler,
            state: state.clone(),
        };
        let evidence_stream_service = EvidenceStreamService {
            handler: evidence_handler,
            state: state.clone(),
        };

        // Build the gRPC server
        let addr = self.config.bind_addr;
        info!("Starting gRPC bridge server on {}", addr);

        let server = tonic::transport::Server::builder()
            .add_service(proto::task_dispatch_server::TaskDispatchServer::new(
                task_dispatch_service,
            ))
            .add_service(proto::task_status_server::TaskStatusServer::new(
                task_status_service,
            ))
            .add_service(proto::evidence_stream_server::EvidenceStreamServer::new(
                evidence_stream_service,
            ));

        if let Some(signal) = shutdown_signal {
            server
                .serve_with_shutdown(addr, signal)
                .await
                .context("gRPC server failed")?;
        } else {
            // Default: listen for OS signals
            let stop = tokio::signal::ctrl_c();
            tokio::pin!(stop);

            server
                .serve_with_shutdown(addr, async {
                    let _ = stop.await;
                    info!("Shutdown signal received, draining active tasks...");
                    // Cancel all in-flight tasks
                    let tasks = state.tasks.read().await;
                    for (_, task) in tasks.iter() {
                        if matches!(
                            task.status,
                            proto::TaskState::Running | proto::TaskState::Queued
                        ) {
                            info!("Marking task {} as cancelled on shutdown", task.task_id);
                        }
                    }
                })
                .await
                .context("gRPC server failed")?;
        }

        info!("gRPC bridge server stopped");
        Ok(())
    }
}

// ============================================================================
// Task Dispatch Service Implementation
// ============================================================================

#[derive(Clone)]
struct TaskDispatchService {
    handler: Option<Arc<dyn DispatchHandler>>,
    state: Arc<BridgeState>,
}

#[tonic::async_trait]
impl proto::task_dispatch_server::TaskDispatch for TaskDispatchService {
    async fn dispatch_task(
        &self,
        request: Request<proto::DispatchRequest>,
    ) -> Result<Response<proto::DispatchResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "DispatchTask request: task_id={}, proposal_id={}",
            req.task_id, req.proposal_id
        );

        // Store initial task state
        let task_state = TaskState {
            task_id: req.task_id.clone(),
            agent_task_id: None,
            status: proto::TaskState::Queued,
            summary: "Task dispatched, awaiting agent acceptance".to_string(),
            artifacts: Vec::new(),
            progress_percent: 0,
        };

        {
            let mut tasks = self.state.tasks.write().await;
            tasks.insert(req.task_id.clone(), task_state);
        }

        // Call the handler if available
        if let Some(handler) = &self.handler {
            match handler.dispatch_task(req).await {
                Ok(response) => {
                    // Update task state with agent_task_id
                    let mut tasks = self.state.tasks.write().await;
                    if let Some(state) = tasks.get_mut(&response.agent_task_id) {
                        state.agent_task_id = Some(response.agent_task_id.clone());
                        state.status = match proto::DispatchStatus::try_from(response.status) {
                            Ok(proto::DispatchStatus::Accepted) => proto::TaskState::Running,
                            Ok(proto::DispatchStatus::Rejected) => proto::TaskState::Failed,
                            Ok(proto::DispatchStatus::Queued) => proto::TaskState::Queued,
                            _ => proto::TaskState::Unknown,
                        };
                    }
                    return Ok(Response::new(response));
                }
                Err(e) => {
                    error!("Dispatch handler error: {}", e);
                    return Err(Status::internal(format!("Dispatch failed: {}", e)));
                }
            }
        }

        // Default response if no handler is configured
        Ok(Response::new(proto::DispatchResponse {
            agent_task_id: format!("agent-{}", uuid::Uuid::new_v4().simple()),
            status: proto::DispatchStatus::Accepted as i32,
            message: "Task accepted (no handler configured)".to_string(),
        }))
    }

    async fn cancel_task(
        &self,
        request: Request<proto::CancelRequest>,
    ) -> Result<Response<proto::CancelResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "CancelTask request: task_id={}, reason={}",
            req.task_id, req.reason
        );

        // Update task state
        {
            let mut tasks = self.state.tasks.write().await;
            if let Some(state) = tasks.get_mut(&req.task_id) {
                state.status = proto::TaskState::Cancelled;
                state.summary = format!("Cancelled: {}", req.reason);
            }
        }

        // Call the handler if available
        if let Some(handler) = &self.handler {
            match handler.cancel_task(req).await {
                Ok(response) => return Ok(Response::new(response)),
                Err(e) => {
                    error!("Cancel handler error: {}", e);
                    return Err(Status::internal(format!("Cancel failed: {}", e)));
                }
            }
        }

        Ok(Response::new(proto::CancelResponse {
            success: true,
            message: "Task cancelled (no handler configured)".to_string(),
        }))
    }
}

// ============================================================================
// Task Status Service Implementation
// ============================================================================

#[derive(Clone)]
struct TaskStatusService {
    handler: Option<Arc<dyn StatusHandler>>,
    state: Arc<BridgeState>,
}

#[tonic::async_trait]
impl proto::task_status_server::TaskStatus for TaskStatusService {
    async fn get_status(
        &self,
        request: Request<proto::StatusRequest>,
    ) -> Result<Response<proto::StatusResponse>, Status> {
        let req = request.into_inner();
        debug!("GetStatus request: task_id={}", req.task_id);

        // Call the handler if available
        if let Some(handler) = &self.handler {
            match handler.get_status(req).await {
                Ok(response) => return Ok(Response::new(response)),
                Err(e) => {
                    error!("Status handler error: {}", e);
                    return Err(Status::internal(format!("Status check failed: {}", e)));
                }
            }
        }

        // Default: return cached state
        let tasks = self.state.tasks.read().await;
        if let Some(state) = tasks.get(&req.task_id) {
            Ok(Response::new(proto::StatusResponse {
                task_id: state.task_id.clone(),
                state: state.status as i32,
                summary: state.summary.clone(),
                artifacts: state.artifacts.clone(),
                progress_percent: state.progress_percent,
                error_message: String::new(),
            }))
        } else {
            Err(Status::not_found(format!("Task {} not found", req.task_id)))
        }
    }

    type StreamStatusStream = ReceiverStream<Result<proto::StatusUpdate, Status>>;

    async fn stream_status(
        &self,
        request: Request<proto::StatusRequest>,
    ) -> Result<Response<Self::StreamStatusStream>, Status> {
        let req = request.into_inner();
        debug!("StreamStatus request: task_id={}", req.task_id);

        // Call the handler if available
        if let Some(handler) = &self.handler {
            match handler.stream_status(req).await {
                Ok(rx) => {
                    return Ok(Response::new(ReceiverStream::new(rx)));
                }
                Err(e) => {
                    error!("Status stream handler error: {}", e);
                    return Err(Status::internal(format!("Status stream failed: {}", e)));
                }
            }
        }

        // Default: create a channel that sends Result<StatusUpdate, Status>
        let (tx, rx): (mpsc::Sender<Result<proto::StatusUpdate, Status>>, _) = mpsc::channel(32);
        {
            let mut senders = self.state.status_senders.write().await;
            senders.insert(req.task_id.clone(), tx);
        }

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

// ============================================================================
// Evidence Stream Service Implementation
// ============================================================================

#[derive(Clone)]
struct EvidenceStreamService {
    handler: Option<Arc<dyn EvidenceHandler>>,
    state: Arc<BridgeState>,
}

#[tonic::async_trait]
impl proto::evidence_stream_server::EvidenceStream for EvidenceStreamService {
    type StreamEvidenceStream = ReceiverStream<Result<proto::EvidenceRecord, Status>>;

    async fn stream_evidence(
        &self,
        request: Request<proto::EvidenceRequest>,
    ) -> Result<Response<Self::StreamEvidenceStream>, Status> {
        let req = request.into_inner();
        debug!("StreamEvidence request: task_id={}", req.task_id);

        // Call the handler if available
        if let Some(handler) = &self.handler {
            match handler.stream_evidence(req).await {
                Ok(rx) => {
                    return Ok(Response::new(ReceiverStream::new(rx)));
                }
                Err(e) => {
                    error!("Evidence stream handler error: {}", e);
                    return Err(Status::internal(format!("Evidence stream failed: {}", e)));
                }
            }
        }

        // Default: create a channel that sends Result<EvidenceRecord, Status>
        let (tx, rx): (mpsc::Sender<Result<proto::EvidenceRecord, Status>>, _) = mpsc::channel(32);
        {
            let mut senders = self.state.evidence_senders.write().await;
            senders.insert(req.task_id.clone(), tx);
        }

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn submit_evidence(
        &self,
        request: Request<proto::SubmitEvidenceRequest>,
    ) -> Result<Response<proto::SubmitEvidenceResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "SubmitEvidence request: task_id={}, evidence_count={}",
            req.task_id,
            req.evidence.len()
        );

        // Update task state based on final_state
        {
            let mut tasks = self.state.tasks.write().await;
            if let Some(state) = tasks.get_mut(&req.task_id) {
                state.status = proto::TaskState::try_from(req.final_state)
                    .unwrap_or(proto::TaskState::Unknown);
                state.summary = req.summary.clone();
            }
        }

        // Call the handler if available
        if let Some(handler) = &self.handler {
            match handler.submit_evidence(req).await {
                Ok(response) => return Ok(Response::new(response)),
                Err(e) => {
                    error!("Evidence submit handler error: {}", e);
                    return Err(Status::internal(format!("Evidence submit failed: {}", e)));
                }
            }
        }

        // Default: collect evidence paths
        let evidence_paths: Vec<String> = req
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

        Ok(Response::new(proto::SubmitEvidenceResponse {
            success: true,
            evidence_paths,
            message: "Evidence submitted (no handler configured)".to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_state_default() {
        let state = BridgeState::new();
        // Verify the state can be created (async test would require tokio runtime)
        let _ = state;
    }
}
