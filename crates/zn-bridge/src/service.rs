//! Service trait definitions for gRPC handlers

use anyhow::Result;
use tokio::sync::mpsc;

use crate::proto;

/// Handler for task dispatch operations
#[async_trait::async_trait]
pub trait DispatchHandler: Send + Sync {
    async fn dispatch_task(&self, request: proto::DispatchRequest) -> Result<proto::DispatchResponse>;
    async fn cancel_task(&self, request: proto::CancelRequest) -> Result<proto::CancelResponse>;
}

/// Handler for task status operations
#[async_trait::async_trait]
pub trait StatusHandler: Send + Sync {
    async fn get_status(&self, request: proto::StatusRequest) -> Result<proto::StatusResponse>;
    async fn stream_status(
        &self,
        request: proto::StatusRequest,
    ) -> Result<mpsc::Receiver<Result<proto::StatusUpdate, tonic::Status>>>;
}

/// Handler for evidence stream operations
#[async_trait::async_trait]
pub trait EvidenceHandler: Send + Sync {
    async fn stream_evidence(
        &self,
        request: proto::EvidenceRequest,
    ) -> Result<mpsc::Receiver<Result<proto::EvidenceRecord, tonic::Status>>>;
    async fn submit_evidence(
        &self,
        request: proto::SubmitEvidenceRequest,
    ) -> Result<proto::SubmitEvidenceResponse>;
}
