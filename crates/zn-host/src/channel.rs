//! Channel Abstraction — pluggable host execution interface.
//!
//! Provides a trait-based abstraction over different host environments
//! (Claude Code, OpenCode, Terminal) so that execution logic can be
//! host-agnostic while each channel implements host-specific dispatch.

use anyhow::Result;
use zn_types::{ExecutionPlan, ExecutionReport, HostKind};

/// Configuration for a channel.
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    /// Maximum number of concurrent executions.
    pub max_concurrent: usize,
    /// Timeout in seconds for a single execution.
    pub timeout_secs: u64,
    /// Extra environment variables to inject.
    pub extra_env: std::collections::HashMap<String, String>,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 2,
            timeout_secs: 600,
            extra_env: std::collections::HashMap::new(),
        }
    }
}

/// A channel represents a host-specific execution backend.
///
/// Each channel knows how to:
/// - Identify which host kind it serves
/// - Execute an `ExecutionPlan` and return an `ExecutionReport`
/// - Report availability and configuration
#[async_trait::async_trait]
pub trait Channel: Send + Sync {
    /// Returns the `HostKind` this channel targets.
    fn host_kind(&self) -> HostKind;

    /// Execute the given plan and return a report.
    async fn execute(&self, plan: &ExecutionPlan) -> Result<ExecutionReport>;

    /// Whether this channel is currently available for execution.
    fn is_available(&self) -> bool;

    /// Returns the channel's configuration.
    fn config(&self) -> ChannelConfig;
}

/// Registry of named channels for lookup and dispatch.
pub struct ChannelRegistry {
    channels: std::collections::HashMap<String, Box<dyn Channel>>,
}

impl ChannelRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            channels: std::collections::HashMap::new(),
        }
    }

    /// Register a channel under the given name.
    pub fn register(&mut self, name: &str, channel: Box<dyn Channel>) {
        self.channels.insert(name.to_string(), channel);
    }

    /// Look up a channel by name.
    pub fn get(&self, name: &str) -> Option<&dyn Channel> {
        self.channels.get(name).map(|c| c.as_ref())
    }

    /// Look up a channel by `HostKind`.
    pub fn get_by_host(&self, host: &HostKind) -> Option<&dyn Channel> {
        self.channels.values().find(|c| c.host_kind() == *host).map(|c| c.as_ref())
    }

    /// List all registered channel names.
    pub fn list_names(&self) -> Vec<String> {
        self.channels.keys().cloned().collect()
    }

    /// Returns true if any channel matches the given host kind.
    pub fn has_host(&self, host: &HostKind) -> bool {
        self.channels.values().any(|c| c.host_kind() == *host)
    }
}

impl Default for ChannelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyChannel {
        kind: HostKind,
    }

    #[async_trait::async_trait]
    impl Channel for DummyChannel {
        fn host_kind(&self) -> HostKind {
            self.kind.clone()
        }

        async fn execute(&self, _plan: &ExecutionPlan) -> Result<ExecutionReport> {
            unimplemented!()
        }

        fn is_available(&self) -> bool {
            true
        }

        fn config(&self) -> ChannelConfig {
            ChannelConfig::default()
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ChannelRegistry::new();
        registry.register(
            "claude",
            Box::new(DummyChannel {
                kind: HostKind::ClaudeCode,
            }),
        );
        assert!(registry.get("claude").is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn test_registry_get_by_host() {
        let mut registry = ChannelRegistry::new();
        registry.register(
            "opencode",
            Box::new(DummyChannel {
                kind: HostKind::OpenCode,
            }),
        );
        let found = registry.get_by_host(&HostKind::OpenCode);
        assert!(found.is_some());
        assert_eq!(found.unwrap().host_kind(), HostKind::OpenCode);
    }

    #[test]
    fn test_registry_has_host() {
        let mut registry = ChannelRegistry::new();
        registry.register(
            "terminal",
            Box::new(DummyChannel {
                kind: HostKind::Terminal,
            }),
        );
        assert!(registry.has_host(&HostKind::Terminal));
        assert!(!registry.has_host(&HostKind::ClaudeCode));
    }

    #[test]
    fn test_channel_config_default() {
        let config = ChannelConfig::default();
        assert_eq!(config.max_concurrent, 2);
        assert_eq!(config.timeout_secs, 600);
        assert!(config.extra_env.is_empty());
    }
}
