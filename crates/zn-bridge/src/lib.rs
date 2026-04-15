//! Zero_Nine Bridge - gRPC communication layer between Rust kernel and agents
//!
//! This crate provides:
//! - gRPC server that agents can connect to
//! - Task dispatching and status tracking
//! - Evidence streaming and submission
//! - MCP client and server for tool integration
//!
//! ## Example
//!
//! ```rust,no_run
//! use zn_bridge::{BridgeServer, BridgeConfig};
//! use std::net::SocketAddr;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let addr: SocketAddr = "127.0.0.1:50051".parse()?;
//!     let config = BridgeConfig {
//!         bind_addr: addr,
//!         ..Default::default()
//!     };
//!     let server = BridgeServer::new(config);
//!     server.run().await?;
//!     Ok(())
//! }
//! ```

pub mod proto {
    tonic::include_proto!("zero_nine.bridge.v1");
}

pub mod server;
pub mod service;
pub mod types;
pub mod mcp_client;
pub mod mcp_server;

pub use server::BridgeServer;
pub use service::{DispatchHandler, EvidenceHandler, StatusHandler};
pub use types::BridgeConfig;
pub use mcp_client::{McpClient, McpConfig, McpTool, load_or_create_mcp_config};
pub use mcp_server::ZeroNineMcpServer;

// Re-export Stream for convenience
pub use futures_core::Stream;
