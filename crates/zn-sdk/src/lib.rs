//! Zero_Nine SDK — unified facade for the orchestration kernel.
//!
//! The SDK exposes the core workflow as a single `ZeroNine` struct.
//! CLI parameter parsing and output rendering belong in zn-cli, not here.

use std::path::Path;

use anyhow::Result;
use zn_types::{HostKind, ZeroNineSdkConfig, ZeroNineStatusResponse};

/// Unified entry point for all Zero_Nine operations.
pub struct ZeroNine {
    config: ZeroNineSdkConfig,
}

impl ZeroNine {
    /// Create a new SDK instance with the given configuration.
    pub fn new(config: ZeroNineSdkConfig) -> Self {
        Self { config }
    }

    fn project_root(&self) -> &Path {
        Path::new(&self.config.project_root)
    }

    /// Get the configured host kind.
    pub fn host(&self) -> &HostKind {
        &self.config.host
    }

    /// Initialize the project layout and manifest.
    pub fn init(&self) -> Result<()> {
        zn_loop::initialize_project(self.project_root(), self.config.host.clone())
    }

    /// Start or resume a brainstorming session.
    pub fn brainstorm(&self, goal: Option<&str>, resume: bool) -> Result<String> {
        zn_loop::brainstorm(self.project_root(), goal, self.config.host.clone(), resume)
    }

    /// Execute a goal through the full orchestration loop.
    pub fn run_goal(&self, goal: &str, allow_remote_finish: bool) -> Result<String> {
        zn_loop::run_goal(
            self.project_root(),
            goal,
            self.config.host.clone(),
            allow_remote_finish,
        )
    }

    /// Resume an interrupted workflow.
    pub fn resume(&self, allow_remote_finish: bool) -> Result<String> {
        zn_loop::resume(
            self.project_root(),
            self.config.host.clone(),
            allow_remote_finish,
        )
    }

    /// Query current project status.
    pub fn status(&self) -> Result<ZeroNineStatusResponse> {
        let raw = zn_loop::status(self.project_root())?;
        Ok(ZeroNineStatusResponse {
            status: "ready".to_string(),
            message: raw,
        })
    }

    /// Export host adapter files.
    pub fn export(&self) -> Result<String> {
        zn_loop::export(self.project_root())
    }

    /// Validate the latest proposal spec.
    pub fn validate_spec(&self) -> Result<String> {
        zn_loop::validate_spec(self.project_root())
    }

    /// Handle a brainstorming turn from a host agent.
    pub fn brainstorm_host_turn(&self, input: &str) -> Result<String> {
        zn_loop::brainstorm_host_turn(self.project_root(), input, self.config.host.clone())
    }
}

/// Convenience constructor from a project root and host kind.
pub fn from_project(project_root: &str, host: HostKind) -> ZeroNine {
    ZeroNine::new(ZeroNineSdkConfig {
        project_root: project_root.to_string(),
        host,
    })
}
