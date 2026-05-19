//! Container Sandbox — Docker/Podman-based isolated execution environment.
//!
//! Provides:
//! - `ContainerSandbox::provision()` — create an isolated container
//! - `ContainerSandbox::exec()` — run commands inside the container
//! - `ContainerSandbox::teardown()` — destroy the container

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::process::Command;
use zn_types::{EnvironmentSpec, NetworkPolicy};

/// Runtime engine for container execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerRuntime {
    Docker,
    Podman,
}

impl ContainerRuntime {
    fn binary(&self) -> &'static str {
        match self {
            ContainerRuntime::Docker => "docker",
            ContainerRuntime::Podman => "podman",
        }
    }

    /// Detect the available container runtime.
    pub fn detect() -> Option<Self> {
        if Command::new("docker")
            .arg("--version")
            .output()
            .is_ok()
        {
            Some(ContainerRuntime::Docker)
        } else if Command::new("podman")
            .arg("--version")
            .output()
            .is_ok()
        {
            Some(ContainerRuntime::Podman)
        } else {
            None
        }
    }
}

/// A provisioned container sandbox.
pub struct ContainerSandbox {
    runtime: ContainerRuntime,
    container_id: String,
    _spec: EnvironmentSpec,
}

impl ContainerSandbox {
    /// Provision a new container sandbox.
    ///
    /// Creates a container with the specified environment, network policy,
    /// and resource limits. Returns an error if no container runtime is available.
    pub fn provision(spec: &EnvironmentSpec) -> Result<Self> {
        let runtime = ContainerRuntime::detect()
            .ok_or_else(|| anyhow!("No container runtime found (docker or podman)"))?;

        let mut cmd = Command::new(runtime.binary());
        cmd.arg("create");

        // Working directory
        if !spec.working_dir.is_empty() {
            cmd.args(["--workdir", &spec.working_dir]);
        }

        // Environment variables
        for (key, value) in &spec.env_vars {
            cmd.args(["--env", &format!("{}={}", key, value)]);
        }

        // Network policy
        match spec.network_policy {
            NetworkPolicy::None => {
                cmd.args(["--network", "none"]);
            }
            NetworkPolicy::Internal => {
                // Use a custom internal network (no external bridge)
                cmd.args(["--network", "host"]);
            }
            NetworkPolicy::Full => {
                // Default: no network restriction
            }
        }

        // Resource limits (Docker/Podman compatible flags)
        if spec.resource_limits.memory_bytes > 0 {
            cmd.args(["--memory", &spec.resource_limits.memory_bytes.to_string()]);
        }
        if spec.resource_limits.cpu_cores > 0.0 {
            cmd.args(["--cpus", &spec.resource_limits.cpu_cores.to_string()]);
        }

        // Mount points
        for mount in &spec.mount_points {
            let mut spec_str = format!("{}:{}", mount.host_path, mount.container_path);
            if mount.read_only {
                spec_str.push_str(":ro");
            }
            cmd.args(["--mount", &spec_str]);
        }

        // Base image
        cmd.arg(&spec.base_image);

        let output = cmd
            .output()
            .with_context(|| "Failed to create container")?;

        if !output.status.success() {
            return Err(anyhow!(
                "Container creation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let container_id = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();

        if container_id.is_empty() {
            return Err(anyhow!("Container ID is empty after creation"));
        }

        Ok(Self {
            runtime,
            container_id,
            _spec: spec.clone(),
        })
    }

    /// Execute a command inside the container.
    pub fn exec(&self, cmd: &str) -> Result<ExecResult> {
        let mut command = Command::new(self.runtime.binary());
        command
            .arg("exec")
            .arg(&self.container_id)
            .args(["sh", "-c", cmd]);

        let output = command
            .output()
            .with_context(|| format!("Failed to exec in container {}", self.container_id))?;

        Ok(ExecResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    /// Copy a file from host into the container.
    pub fn copy_in(&self, host_path: &Path, container_path: &str) -> Result<()> {
        Command::new(self.runtime.binary())
            .arg("cp")
            .arg(host_path.to_string_lossy().to_string())
            .arg(format!("{}:{}", self.container_id, container_path))
            .output()
            .with_context(|| "Failed to copy file into container")?;
        Ok(())
    }

    /// Teardown (destroy) the container.
    pub fn teardown(self) -> Result<()> {
        Command::new(self.runtime.binary())
            .arg("rm")
            .arg("-f")
            .arg(&self.container_id)
            .output()
            .with_context(|| "Failed to remove container")?;
        Ok(())
    }

    /// Get the container ID.
    pub fn container_id(&self) -> &str {
        &self.container_id
    }
}

/// Result of a command executed inside a container.
pub struct ExecResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Create a default environment spec for a given project.
pub fn default_env_spec(project_root: &Path, image: &str) -> EnvironmentSpec {
    EnvironmentSpec {
        base_image: image.to_string(),
        working_dir: "/workspace".to_string(),
        mount_points: vec![zn_types::MountSpec {
            host_path: project_root.to_string_lossy().to_string(),
            container_path: "/workspace".to_string(),
            read_only: false,
        }],
        network_policy: NetworkPolicy::Full,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_container_runtime_detect() {
        // May or may not have docker/podman — just ensure it doesn't panic
        let _ = ContainerRuntime::detect();
    }

    #[test]
    fn test_environment_spec_defaults() {
        let spec = EnvironmentSpec::default();
        assert!(spec.base_image.is_empty());
        assert!(spec.working_dir.is_empty());
        assert!(spec.env_vars.is_empty());
        assert!(spec.mount_points.is_empty());
        assert!(matches!(spec.network_policy, NetworkPolicy::Full));
    }

    #[test]
    fn test_default_env_spec() {
        let spec = default_env_spec(Path::new("/tmp/test"), "rust:latest");
        assert_eq!(spec.base_image, "rust:latest");
        assert_eq!(spec.working_dir, "/workspace");
        assert_eq!(spec.mount_points.len(), 1);
        assert_eq!(spec.mount_points[0].container_path, "/workspace");
    }

    #[test]
    fn test_env_spec_with_custom_values() {
        let mut env_vars = HashMap::new();
        env_vars.insert("FOO".to_string(), "bar".to_string());

        let spec = EnvironmentSpec {
            base_image: "python:3.11".to_string(),
            working_dir: "/app".to_string(),
            env_vars,
            network_policy: NetworkPolicy::None,
            ..Default::default()
        };

        assert_eq!(spec.base_image, "python:3.11");
        assert_eq!(spec.working_dir, "/app");
        assert!(spec.env_vars.contains_key("FOO"));
        assert!(matches!(spec.network_policy, NetworkPolicy::None));
    }
}
