//! Zero_Nine SDK — unified facade for the orchestration kernel.
//!
//! The SDK exposes the core workflow as a single `ZeroNine` struct.
//! CLI parameter parsing and output rendering belong in zn-cli, not here.

use std::path::Path;

use anyhow::{anyhow, Result};
use zn_loop::TerminalInput;
use zn_types::{HostKind, ZeroNineSdkConfig, ZeroNineStatusResponse};

/// No-op terminal input — used when the host is not Terminal (no readline needed).
pub struct NoopInput;

impl TerminalInput for NoopInput {
    fn readline(&mut self, _prompt: &str) -> Result<String> {
        Err(anyhow!("NoopInput: readline not available — use a TerminalInput implementation for HostKind::Terminal"))
    }
}

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
    pub fn brainstorm<T: TerminalInput>(
        &self,
        goal: Option<&str>,
        resume: bool,
        input: &mut T,
    ) -> Result<String> {
        zn_loop::brainstorm(
            self.project_root(),
            goal,
            self.config.host.clone(),
            resume,
            input,
        )
    }

    /// Brainstorming for non-terminal hosts (no input needed).
    pub fn brainstorm_headless(&self, goal: Option<&str>, resume: bool) -> Result<String> {
        zn_loop::brainstorm(
            self.project_root(),
            goal,
            self.config.host.clone(),
            resume,
            &mut NoopInput,
        )
    }

    /// Execute a goal through the full orchestration loop.
    pub fn run_goal<T: TerminalInput>(
        &self,
        goal: &str,
        allow_remote_finish: bool,
        input: &mut T,
    ) -> Result<String> {
        zn_loop::run_goal(
            self.project_root(),
            goal,
            self.config.host.clone(),
            allow_remote_finish,
            input,
        )
    }

    /// Run goal for non-terminal hosts (no input needed).
    pub fn run_goal_headless(&self, goal: &str, allow_remote_finish: bool) -> Result<String> {
        zn_loop::run_goal(
            self.project_root(),
            goal,
            self.config.host.clone(),
            allow_remote_finish,
            &mut NoopInput,
        )
    }

    /// Resume an interrupted workflow.
    pub fn resume<T: TerminalInput>(
        &self,
        allow_remote_finish: bool,
        input: &mut T,
    ) -> Result<String> {
        zn_loop::resume(
            self.project_root(),
            self.config.host.clone(),
            allow_remote_finish,
            input,
        )
    }

    /// Resume for non-terminal hosts (no input needed).
    pub fn resume_headless(&self, allow_remote_finish: bool) -> Result<String> {
        zn_loop::resume(
            self.project_root(),
            self.config.host.clone(),
            allow_remote_finish,
            &mut NoopInput,
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

    /// Dry-run: generate an execution plan for a goal without executing it.
    /// Returns a human-readable summary of what would happen.
    pub fn run_dry(&self, goal: &str) -> Result<String> {
        zn_loop::plan_only(self.project_root(), goal, self.config.host.clone())
    }

    /// Dry-run: preview resume plan without executing.
    pub fn resume_dry(&self) -> Result<String> {
        zn_loop::resume_plan(self.project_root(), self.config.host.clone())
    }
}

/// Convenience constructor from a project root and host kind.
pub fn from_project(project_root: &str, host: HostKind) -> ZeroNine {
    ZeroNine::new(ZeroNineSdkConfig {
        project_root: project_root.to_string(),
        host,
    })
}
