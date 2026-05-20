//! Cloud synchronization for evolution data.
//!
//! Provides version-vector-based conflict resolution and HTTP sync
//! for distilled skills, skill bundles, and registry state.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::distiller::DistilledSkill;
use zn_types::SkillBundle;

// ============================================================================
// Version Vector
// ============================================================================

/// Logical clock per skill for conflict detection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionVector {
    pub node_id: String,
    pub clocks: HashMap<String, u64>,
}

impl VersionVector {
    pub fn new(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            clocks: HashMap::new(),
        }
    }

    pub fn increment(&mut self, skill_id: &str) {
        let clock = self.clocks.entry(skill_id.to_string()).or_insert(0);
        *clock += 1;
    }

    pub fn merge(&mut self, other: &VersionVector) {
        for (key, &val) in &other.clocks {
            let entry = self.clocks.entry(key.clone()).or_insert(0);
            *entry = (*entry).max(val);
        }
    }
}

// ============================================================================
// Cloud Sync State
// ============================================================================

/// Tier-1 data that is synchronized across nodes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CloudSyncState {
    pub distilled_skills: Vec<DistilledSkill>,
    pub skill_bundles: Vec<SkillBundle>,
    pub skill_count: usize,
    pub version_vectors: VersionVector,
    pub last_synced_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Merge Result
// ============================================================================

/// Summary of a state merge operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MergeResult {
    pub skills_uploaded: usize,
    pub skills_downloaded: usize,
    pub conflicts_resolved: usize,
    pub local_wins: usize,
    pub remote_wins: usize,
}

// ============================================================================
// Cloud Sync Config
// ============================================================================

/// Configuration for cloud synchronization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncConfig {
    pub endpoint_url: String,
    pub auth_token: String,
    pub node_id: String,
    pub auto_sync: bool,
}

impl CloudSyncConfig {
    /// Load config from a JSON file. Returns None if file doesn't exist.
    pub fn load(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("read cloud sync config: {}", path.display()))?;
        let config: Self = serde_json::from_str(&data)
            .with_context(|| format!("parse cloud sync config: {}", path.display()))?;
        Ok(Some(config))
    }

    /// Save config to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}

// ============================================================================
// CloudSyncClient
// ============================================================================

/// HTTP client for cloud sync operations.
pub struct CloudSyncClient {
    config: CloudSyncConfig,
    http: reqwest::Client,
}

impl CloudSyncClient {
    pub fn new(config: CloudSyncConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("build HTTP client")?;
        Ok(Self { config, http })
    }

    /// Upload evolution state to the cloud.
    pub async fn upload_state(&self, state: &CloudSyncState) -> Result<()> {
        let url = format!("{}/sync/upload", self.config.endpoint_url);
        self.http
            .post(&url)
            .bearer_auth(&self.config.auth_token)
            .json(state)
            .send()
            .await
            .context("upload sync state")?
            .error_for_status()
            .context("upload sync state: server error")?;
        Ok(())
    }

    /// Download evolution state from the cloud.
    pub async fn download_state(&self) -> Result<CloudSyncState> {
        let url = format!("{}/sync/download", self.config.endpoint_url);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.config.auth_token)
            .send()
            .await
            .context("download sync state")?;
        let state: CloudSyncState = resp
            .error_for_status()
            .context("download sync state: server error")?
            .json()
            .await
            .context("parse downloaded state")?;
        Ok(state)
    }

    /// Merge local and remote state using version vectors + last-write-wins.
    pub fn merge_state(local: &mut CloudSyncState, remote: CloudSyncState) -> MergeResult {
        let mut result = MergeResult::default();

        // Merge remote skills into local
        for remote_skill in remote.distilled_skills {
            let skill_id = &remote_skill.pattern_id;
            match local
                .distilled_skills
                .iter()
                .position(|s| s.pattern_id == *skill_id)
            {
                Some(idx) => {
                    // Conflict: same skill exists in both
                    let local_clock = local
                        .version_vectors
                        .clocks
                        .get(skill_id)
                        .copied()
                        .unwrap_or(0);
                    let remote_clock = remote
                        .version_vectors
                        .clocks
                        .get(skill_id)
                        .copied()
                        .unwrap_or(0);

                    if remote_clock > local_clock {
                        // Remote wins (higher clock)
                        local.distilled_skills[idx] = remote_skill;
                        result.remote_wins += 1;
                    }
                    // If equal or local higher, keep local (local-preferred LWW)
                    result.conflicts_resolved += 1;
                }
                None => {
                    // New remote skill, add to local
                    local.distilled_skills.push(remote_skill);
                    result.skills_downloaded += 1;
                }
            }
        }

        // Merge version vectors
        local.version_vectors.merge(&remote.version_vectors);
        local.last_synced_at = Some(Utc::now());

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs;

    #[test]
    fn test_version_vector_increment() {
        let mut vv = VersionVector::new("node-1");
        vv.increment("skill-a");
        assert_eq!(vv.clocks.get("skill-a"), Some(&1));
        vv.increment("skill-a");
        assert_eq!(vv.clocks.get("skill-a"), Some(&2));
        vv.increment("skill-b");
        assert_eq!(vv.clocks.get("skill-b"), Some(&1));
    }

    #[test]
    fn test_version_vector_merge() {
        let mut vv1 = VersionVector::new("node-1");
        vv1.clocks.insert("a".to_string(), 3);
        vv1.clocks.insert("b".to_string(), 1);

        let mut vv2 = VersionVector::new("node-2");
        vv2.clocks.insert("a".to_string(), 2);
        vv2.clocks.insert("c".to_string(), 5);

        vv1.merge(&vv2);
        assert_eq!(vv1.clocks.get("a"), Some(&3)); // max(3,2)
        assert_eq!(vv1.clocks.get("b"), Some(&1));
        assert_eq!(vv1.clocks.get("c"), Some(&5));
    }

    #[test]
    fn test_merge_local_wins_equal_clock() {
        let mut local = CloudSyncState::default();
        local.distilled_skills.push(DistilledSkill {
            pattern_id: "skill-a".to_string(),
            bundle: SkillBundle {
                id: "skill-a".to_string(),
                name: "Local Skill A".to_string(),
                version: zn_types::SkillVersion {
                    major: 1,
                    minor: 0,
                    patch: 0,
                },
                description: "Local version".to_string(),
                applicable_scenarios: vec![],
                preconditions: vec![],
                disabled_conditions: vec![],
                risk_level: zn_types::ActionRiskLevel::Low,
                skill_chain: vec![],
                artifacts: vec![],
                usage_count: 1,
                success_rate: 0.9,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            confidence_score: 0.8,
            supporting_evidence: vec![],
            usage_recommendations: vec![],
            anti_patterns: vec![],
        });
        local
            .version_vectors
            .clocks
            .insert("skill-a".to_string(), 2);

        let mut remote = CloudSyncState::default();
        remote.distilled_skills.push(DistilledSkill {
            pattern_id: "skill-a".to_string(),
            bundle: SkillBundle {
                id: "skill-a".to_string(),
                name: "Remote Skill A".to_string(),
                version: zn_types::SkillVersion {
                    major: 2,
                    minor: 0,
                    patch: 0,
                },
                description: "Remote version".to_string(),
                applicable_scenarios: vec![],
                preconditions: vec![],
                disabled_conditions: vec![],
                risk_level: zn_types::ActionRiskLevel::Low,
                skill_chain: vec![],
                artifacts: vec![],
                usage_count: 2,
                success_rate: 0.95,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            confidence_score: 0.9,
            supporting_evidence: vec![],
            usage_recommendations: vec![],
            anti_patterns: vec![],
        });
        remote
            .version_vectors
            .clocks
            .insert("skill-a".to_string(), 2);

        let result = CloudSyncClient::merge_state(&mut local, remote);
        assert_eq!(result.conflicts_resolved, 1);
        assert_eq!(result.local_wins, 0); // equal clock: local keeps, no win counted
        assert_eq!(result.remote_wins, 0); // equal: no remote win
                                           // Local skill should still be the original (clock equal -> local preferred)
        assert_eq!(local.distilled_skills[0].bundle.name, "Local Skill A");
    }

    #[test]
    fn test_merge_remote_wins_higher_clock() {
        let mut local = CloudSyncState::default();
        local.distilled_skills.push(DistilledSkill {
            pattern_id: "skill-x".to_string(),
            bundle: SkillBundle {
                id: "skill-x".to_string(),
                name: "Old".to_string(),
                version: zn_types::SkillVersion {
                    major: 1,
                    minor: 0,
                    patch: 0,
                },
                description: "Old version".to_string(),
                applicable_scenarios: vec![],
                preconditions: vec![],
                disabled_conditions: vec![],
                risk_level: zn_types::ActionRiskLevel::Low,
                skill_chain: vec![],
                artifacts: vec![],
                usage_count: 1,
                success_rate: 0.5,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            confidence_score: 0.5,
            supporting_evidence: vec![],
            usage_recommendations: vec![],
            anti_patterns: vec![],
        });
        local
            .version_vectors
            .clocks
            .insert("skill-x".to_string(), 1);

        let mut remote = CloudSyncState::default();
        remote.distilled_skills.push(DistilledSkill {
            pattern_id: "skill-x".to_string(),
            bundle: SkillBundle {
                id: "skill-x".to_string(),
                name: "New".to_string(),
                version: zn_types::SkillVersion {
                    major: 2,
                    minor: 0,
                    patch: 0,
                },
                description: "New version".to_string(),
                applicable_scenarios: vec![],
                preconditions: vec![],
                disabled_conditions: vec![],
                risk_level: zn_types::ActionRiskLevel::Low,
                skill_chain: vec![],
                artifacts: vec![],
                usage_count: 5,
                success_rate: 0.9,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            confidence_score: 0.9,
            supporting_evidence: vec![],
            usage_recommendations: vec![],
            anti_patterns: vec![],
        });
        remote
            .version_vectors
            .clocks
            .insert("skill-x".to_string(), 3);

        let result = CloudSyncClient::merge_state(&mut local, remote);
        assert_eq!(result.remote_wins, 1);
        assert_eq!(result.conflicts_resolved, 1);
        assert_eq!(local.distilled_skills[0].bundle.name, "New");
    }

    #[test]
    fn test_cloud_sync_state_serialization() {
        let state = CloudSyncState {
            distilled_skills: vec![],
            skill_bundles: vec![],
            skill_count: 0,
            version_vectors: VersionVector::new("node-1"),
            last_synced_at: Some(Utc::now()),
        };
        let json = serde_json::to_string_pretty(&state).unwrap();
        let restored: CloudSyncState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.version_vectors.node_id, "node-1");
        assert!(restored.last_synced_at.is_some());
    }

    #[test]
    fn test_config_load_missing() {
        let path = temp_dir().join("cloud_sync_missing.json");
        let _ = fs::remove_file(&path);
        let result = CloudSyncConfig::load(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_config_load_and_save() {
        let path = temp_dir().join("cloud_sync_config.json");
        let _ = fs::remove_file(&path);

        let config = CloudSyncConfig {
            endpoint_url: "https://api.example.com".to_string(),
            auth_token: "secret-token".to_string(),
            node_id: "test-node".to_string(),
            auto_sync: true,
        };
        config.save(&path).unwrap();
        assert!(path.exists());

        let loaded = CloudSyncConfig::load(&path).unwrap().unwrap();
        assert_eq!(loaded.endpoint_url, "https://api.example.com");
        assert_eq!(loaded.node_id, "test-node");
        assert!(loaded.auto_sync);

        let _ = fs::remove_file(&path);
    }
}
