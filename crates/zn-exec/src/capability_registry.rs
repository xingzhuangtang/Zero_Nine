//! Capability Registry — agent capability discovery and trust management.
//!
//! Provides:
//! - `CapabilityRegistry` — central register of available agents and their capabilities
//! - `register()` / `deregister()` — agent lifecycle
//! - `find_by_capability()` / `find_by_complexity()` — capability-based lookup
//! - `update_trust()` — trust score adjustment based on execution results

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zn_types::AgentDescriptor;
#[allow(unused_imports)]
use zn_types::Capability;

/// History entry for a trust score change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustHistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub old_score: f32,
    pub new_score: f32,
    pub reason: String,
    pub task_id: Option<String>,
}

/// Agent entry in the capability registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub descriptor: AgentDescriptor,
    pub trust_history: Vec<TrustHistoryEntry>,
    pub total_tasks: u64,
    pub successful_tasks: u64,
    #[serde(default)]
    pub registered_at: DateTime<Utc>,
}

impl RegistryEntry {
    fn success_rate(&self) -> f32 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        self.successful_tasks as f32 / self.total_tasks as f32
    }
}

/// Central capability registry for multi-agent discovery and trust management.
pub struct CapabilityRegistry {
    agents: HashMap<String, RegistryEntry>,
    /// capability_name -> list of agent_ids
    capability_index: HashMap<String, Vec<String>>,
}

impl CapabilityRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            capability_index: HashMap::new(),
        }
    }

    /// Register an agent in the registry.
    /// If the agent already exists, it is updated (re-register).
    pub fn register(&mut self, descriptor: AgentDescriptor) {
        let agent_id = descriptor.agent_id.clone();

        let entry = if let Some(mut existing) = self.agents.remove(&agent_id) {
            // Update descriptor but preserve trust history
            existing.descriptor = descriptor;
            existing
        } else {
            RegistryEntry {
                descriptor,
                trust_history: Vec::new(),
                total_tasks: 0,
                successful_tasks: 0,
                registered_at: Utc::now(),
            }
        };

        // Update capability index
        for cap in &entry.descriptor.capabilities {
            self.capability_index
                .entry(cap.name.clone())
                .or_default()
                .push(agent_id.clone());
        }

        self.agents.insert(agent_id, entry);
    }

    /// Deregister an agent from the registry.
    pub fn deregister(&mut self, agent_id: &str) -> Option<AgentDescriptor> {
        if let Some(entry) = self.agents.remove(agent_id) {
            // Remove from capability index
            for cap in &entry.descriptor.capabilities {
                if let Some(ids) = self.capability_index.get_mut(&cap.name) {
                    ids.retain(|id| id != agent_id);
                }
            }
            Some(entry.descriptor)
        } else {
            None
        }
    }

    /// Find all agents that have a specific capability.
    pub fn find_by_capability(&self, capability: &str) -> Vec<AgentDescriptor> {
        self.capability_index
            .get(capability)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.agents.get(id).map(|e| e.descriptor.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find agents capable of handling a given complexity level.
    /// Returns agents sorted by trust_score (highest first).
    pub fn find_by_complexity(&self, min_complexity: f32) -> Vec<AgentDescriptor> {
        let mut candidates: Vec<_> = self
            .agents
            .values()
            .filter(|e| {
                e.descriptor
                    .capabilities
                    .iter()
                    .any(|c| c.max_complexity >= min_complexity)
            })
            .map(|e| e.descriptor.clone())
            .collect();

        candidates.sort_by(|a, b| {
            b.trust_score
                .partial_cmp(&a.trust_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates
    }

    /// Update an agent's trust score based on execution results.
    ///
    /// Trust adjustment:
    /// - Success: +delta * quality (0.0-1.0)
    /// - Failure: -delta * (1.0 - quality)
    /// - Delta is clamped to keep trust within 0.0-1.0
    pub fn update_trust(
        &mut self,
        agent_id: &str,
        success: bool,
        quality: f32, // 0.0-1.0 quality of the result
        task_id: Option<&str>,
    ) -> Option<f32> {
        let entry = self.agents.get_mut(agent_id)?;

        let delta = 0.05; // base adjustment step
        let adjustment = if success {
            delta * quality
        } else {
            -delta * (1.0 - quality)
        };

        let old_score = entry.descriptor.trust_score;
        let new_score = (old_score + adjustment).clamp(0.0, 1.0);
        entry.descriptor.trust_score = new_score;

        entry.total_tasks += 1;
        if success {
            entry.successful_tasks += 1;
        }

        entry.trust_history.push(TrustHistoryEntry {
            timestamp: Utc::now(),
            old_score,
            new_score,
            reason: if success {
                "Successful execution".to_string()
            } else {
                "Failed execution".to_string()
            },
            task_id: task_id.map(|s| s.to_string()),
        });

        Some(new_score)
    }

    /// Get a registered agent by ID.
    pub fn get(&self, agent_id: &str) -> Option<&AgentDescriptor> {
        self.agents.get(agent_id).map(|e| &e.descriptor)
    }

    /// Get all registered agents.
    pub fn list_agents(&self) -> Vec<AgentDescriptor> {
        self.agents
            .values()
            .map(|e| e.descriptor.clone())
            .collect()
    }

    /// Get the success rate for an agent.
    pub fn success_rate(&self, agent_id: &str) -> Option<f32> {
        self.agents.get(agent_id).map(|e| e.success_rate())
    }

    /// Get the number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zn_types::AgentType;

    fn make_agent(id: &str, trust: f32) -> AgentDescriptor {
        AgentDescriptor {
            agent_id: id.to_string(),
            name: id.to_string(),
            agent_type: AgentType::BuiltIn,
            capabilities: vec![Capability {
                name: "general".to_string(),
                proficiency: 0.8,
                max_complexity: 0.9,
            }],
            trust_score: trust,
            created_at: Utc::now(),
        }
    }

    fn make_specialized_agent(id: &str, cap_name: &str) -> AgentDescriptor {
        AgentDescriptor {
            agent_id: id.to_string(),
            name: id.to_string(),
            agent_type: AgentType::BuiltIn,
            capabilities: vec![Capability {
                name: cap_name.to_string(),
                proficiency: 0.9,
                max_complexity: 1.0,
            }],
            trust_score: 0.7,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_register_and_list() {
        let mut registry = CapabilityRegistry::new();
        registry.register(make_agent("a1", 0.5));
        registry.register(make_agent("a2", 0.8));
        assert_eq!(registry.agent_count(), 2);

        let agents = registry.list_agents();
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn test_deregister() {
        let mut registry = CapabilityRegistry::new();
        registry.register(make_agent("a1", 0.5));
        let removed = registry.deregister("a1");
        assert!(removed.is_some());
        assert_eq!(registry.agent_count(), 0);
        assert!(registry.deregister("a1").is_none());
    }

    #[test]
    fn test_find_by_capability() {
        let mut registry = CapabilityRegistry::new();
        registry.register(make_agent("generalist", 0.5));
        registry.register(make_specialized_agent("coder", "coding"));
        registry.register(make_specialized_agent("reviewer", "review"));

        let generalists = registry.find_by_capability("general");
        assert_eq!(generalists.len(), 1);
        assert_eq!(generalists[0].agent_id, "generalist");

        let coders = registry.find_by_capability("coding");
        assert_eq!(coders.len(), 1);
        assert_eq!(coders[0].agent_id, "coder");

        let nonexistent = registry.find_by_capability("nonexistent");
        assert!(nonexistent.is_empty());
    }

    #[test]
    fn test_find_by_complexity() {
        let mut registry = CapabilityRegistry::new();
        registry.register(make_agent("low", 0.3));
        registry.register(make_agent("high", 0.9));

        // All agents can handle low complexity
        let low = registry.find_by_complexity(0.1);
        assert_eq!(low.len(), 2);
        // Highest trust first
        assert_eq!(low[0].agent_id, "high");

        // Only high-complexity agents
        let high = registry.find_by_complexity(0.95);
        assert!(high.is_empty()); // max_complexity is 0.9 for both
    }

    #[test]
    fn test_update_trust_success() {
        let mut registry = CapabilityRegistry::new();
        registry.register(make_agent("a1", 0.5));

        let new_score = registry.update_trust("a1", true, 0.9, Some("task-1"));
        assert!(new_score.is_some());
        let score = new_score.unwrap();
        assert!(score > 0.5); // trust increased

        let desc = registry.get("a1").unwrap();
        assert_eq!(desc.trust_score, score);
    }

    #[test]
    fn test_update_trust_failure() {
        let mut registry = CapabilityRegistry::new();
        registry.register(make_agent("a1", 0.5));

        let new_score = registry.update_trust("a1", false, 0.1, Some("task-1"));
        assert!(new_score.is_some());
        let score = new_score.unwrap();
        assert!(score < 0.5); // trust decreased
    }

    #[test]
    fn test_trust_bounds() {
        let mut registry = CapabilityRegistry::new();
        registry.register(make_agent("a1", 0.01));

        // Many failures should not go below 0.0
        for _ in 0..100 {
            registry.update_trust("a1", false, 0.0, None);
        }
        let score = registry.get("a1").unwrap().trust_score;
        assert!(score >= 0.0);
    }

    #[test]
    fn test_success_rate() {
        let mut registry = CapabilityRegistry::new();
        registry.register(make_agent("a1", 0.5));

        registry.update_trust("a1", true, 0.9, None);
        registry.update_trust("a1", true, 0.8, None);
        registry.update_trust("a1", false, 0.2, None);

        let rate = registry.success_rate("a1").unwrap();
        assert!((rate - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_reregister_preserves_history() {
        let mut registry = CapabilityRegistry::new();
        registry.register(make_agent("a1", 0.5));
        registry.update_trust("a1", true, 0.9, None);

        let updated = make_agent("a1", 0.6);
        registry.register(updated);

        // History should be preserved
        let entry = registry.agents.get("a1").unwrap();
        assert!(!entry.trust_history.is_empty());
        assert_eq!(entry.total_tasks, 1);
        assert_eq!(entry.descriptor.trust_score, 0.6); // descriptor updated
    }
}
