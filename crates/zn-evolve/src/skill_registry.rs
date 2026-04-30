//! Skill Version Registry — tracks multiple versions per skill with performance metrics
//!
//! Enables comparing skill versions, auto-promoting better versions, and rollback.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use zn_types::SkillVersion;

/// Performance record for a specific skill version
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillVersionRecord {
    pub version: SkillVersion,
    pub content_hash: String,
    /// Number of times this version was used
    pub usage_count: u32,
    /// Number of successful uses
    pub success_count: u32,
    /// Average confidence score when this version was distilled
    pub avg_confidence: f32,
    /// Timestamp when this version was created (RFC3339)
    pub created_at: String,
    /// Whether this version is currently active
    pub active: bool,
    /// Path to the SKILL.md file on disk
    pub file_path: String,
}

impl SkillVersionRecord {
    pub fn success_rate(&self) -> f32 {
        if self.usage_count == 0 {
            return 0.0;
        }
        self.success_count as f32 / self.usage_count as f32
    }
}

/// Registry of all skill versions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillRegistryData {
    /// skill_name -> list of version records
    pub versions: HashMap<String, Vec<SkillVersionRecord>>,
}

/// Skill version registry with disk persistence
pub struct SkillRegistry {
    data: SkillRegistryData,
    registry_file: PathBuf,
}

impl SkillRegistry {
    pub fn new(registry_file: PathBuf) -> Result<Self> {
        let mut registry = Self {
            data: SkillRegistryData::default(),
            registry_file,
        };
        registry.load()?;
        Ok(registry)
    }

    fn load(&mut self) -> Result<()> {
        let content = match fs::read_to_string(&self.registry_file) {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        self.data = serde_json::from_str(&content).with_context(|| {
            format!("Failed to parse registry: {}", self.registry_file.display())
        })?;
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.registry_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.data)?;
        fs::write(&self.registry_file, content).with_context(|| {
            format!("Failed to write registry: {}", self.registry_file.display())
        })?;
        Ok(())
    }

    /// Register a new version of a skill
    pub fn register_version(
        &mut self,
        skill_name: &str,
        version: SkillVersion,
        content_hash: &str,
        confidence: f32,
        file_path: &str,
    ) -> Result<()> {
        // Deactivate previous version
        if let Some(records) = self.data.versions.get_mut(skill_name) {
            for rec in records.iter_mut() {
                rec.active = false;
            }
        }

        let record = SkillVersionRecord {
            version,
            content_hash: content_hash.to_string(),
            usage_count: 0,
            success_count: 0,
            avg_confidence: confidence,
            created_at: chrono::Utc::now().to_rfc3339(),
            active: true,
            file_path: file_path.to_string(),
        };

        self.data
            .versions
            .entry(skill_name.to_string())
            .or_default()
            .push(record);

        self.save()
    }

    /// Record a usage outcome for a skill version
    pub fn record_usage(
        &mut self,
        skill_name: &str,
        version: &SkillVersion,
        success: bool,
    ) -> Result<()> {
        let records = self
            .data
            .versions
            .entry(skill_name.to_string())
            .or_default();
        if let Some(rec) = records.iter_mut().find(|r| r.version == *version) {
            rec.usage_count += 1;
            if success {
                rec.success_count += 1;
            }
            // Update confidence as moving average
            let old_avg = rec.avg_confidence;
            let new_score = if success { 1.0 } else { 0.0 };
            rec.avg_confidence = old_avg + (new_score - old_avg) / (rec.usage_count as f32);
            return self.save();
        }
        Ok(())
    }

    /// Get all versions of a skill, newest first
    pub fn get_versions(&self, skill_name: &str) -> Vec<&SkillVersionRecord> {
        let mut records: Vec<_> = self
            .data
            .versions
            .get(skill_name)
            .map(|v| v.iter().collect())
            .unwrap_or_default();
        records.sort_by(|a, b| {
            b.version
                .major
                .cmp(&a.version.major)
                .then(b.version.minor.cmp(&a.version.minor))
                .then(b.version.patch.cmp(&a.version.patch))
        });
        records
    }

    /// Get the currently active version
    pub fn get_active_version(&self, skill_name: &str) -> Option<&SkillVersionRecord> {
        self.data
            .versions
            .get(skill_name)
            .and_then(|v| v.iter().find(|r| r.active))
    }

    /// Compare two versions of a skill by success rate
    pub fn compare_versions(&self, skill_name: &str) -> Option<VersionComparison> {
        let records = self.data.versions.get(skill_name)?;
        if records.len() < 2 {
            return None;
        }

        let active = records.iter().find(|r| r.active)?;
        let best = records
            .iter()
            .max_by(|a, b| a.success_rate().total_cmp(&b.success_rate()))?;

        Some(VersionComparison {
            skill_name: skill_name.to_string(),
            active_version: active.version.clone(),
            active_success_rate: active.success_rate(),
            active_usage: active.usage_count,
            best_version: best.version.clone(),
            best_success_rate: best.success_rate(),
            best_usage: best.usage_count,
            should_promote: best != active
                && best.usage_count >= 3
                && best.success_rate() > active.success_rate(),
        })
    }

    /// Roll back to a previous version
    pub fn rollback_to(&mut self, skill_name: &str, version: &SkillVersion) -> Result<()> {
        let records = self
            .data
            .versions
            .get_mut(skill_name)
            .with_context(|| format!("No versions found for skill '{}'", skill_name))?;

        let target_index = records
            .iter()
            .position(|r| r.version == *version)
            .with_context(|| format!("Version {} not found for skill '{}'", version, skill_name))?;

        for r in records.iter_mut() {
            r.active = false;
        }
        records[target_index].active = true;

        self.save()
    }

    /// Promote a better version to active
    pub fn promote(&mut self, skill_name: &str, version: &SkillVersion) -> Result<()> {
        self.rollback_to(skill_name, version) // Same operation: set as active
    }

    /// Get summary stats for all skills
    pub fn summary(&self) -> RegistrySummary {
        let mut total_skills = 0;
        let mut total_versions = 0;
        let mut active_count = 0;

        for (name, records) in &self.data.versions {
            total_skills += 1;
            total_versions += records.len();
            if records.iter().any(|r| r.active) {
                active_count += 1;
            }
        }

        RegistrySummary {
            total_skills,
            total_versions,
            active_count,
        }
    }
}

/// Comparison result between skill versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionComparison {
    pub skill_name: String,
    pub active_version: SkillVersion,
    pub active_success_rate: f32,
    pub active_usage: u32,
    pub best_version: SkillVersion,
    pub best_success_rate: f32,
    pub best_usage: u32,
    pub should_promote: bool,
}

/// Registry summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySummary {
    pub total_skills: usize,
    pub total_versions: usize,
    pub active_count: usize,
}

/// Compute a content hash for version comparison
pub fn content_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    fn temp_registry() -> PathBuf {
        temp_dir().join(format!("skill_registry_{}.json", std::process::id()))
    }

    #[test]
    fn test_register_and_get_versions() {
        let path = temp_registry();
        let mut registry = SkillRegistry::new(path).unwrap();

        let v1 = SkillVersion {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let v2 = SkillVersion {
            major: 1,
            minor: 1,
            patch: 0,
        };

        registry
            .register_version("test-skill", v1.clone(), "hash1", 0.8, "/path/to/skill.md")
            .unwrap();
        registry
            .register_version("test-skill", v2.clone(), "hash2", 0.9, "/path/to/skill.md")
            .unwrap();

        let versions = registry.get_versions("test-skill");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version.major, 1);
        assert_eq!(versions[0].version.minor, 1); // newest first
        assert!(versions[0].active);
        assert!(!versions[1].active);

        let _ = fs::remove_file(&registry.registry_file);
    }

    #[test]
    fn test_record_usage_updates_stats() {
        let path = temp_registry();
        let mut registry = SkillRegistry::new(path.clone()).unwrap();
        let v1 = SkillVersion {
            major: 1,
            minor: 0,
            patch: 0,
        };
        registry
            .register_version("test-skill", v1.clone(), "hash1", 0.5, "/path")
            .unwrap();

        registry.record_usage("test-skill", &v1, true).unwrap();
        registry.record_usage("test-skill", &v1, true).unwrap();
        registry.record_usage("test-skill", &v1, false).unwrap();

        let rec = registry.get_versions("test-skill")[0];
        assert_eq!(rec.usage_count, 3);
        assert_eq!(rec.success_count, 2);
        assert!((rec.success_rate() - 0.666).abs() < 0.01);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_rollback_sets_active() {
        let path = temp_registry();
        let mut registry = SkillRegistry::new(path.clone()).unwrap();
        let v1 = SkillVersion {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let v2 = SkillVersion {
            major: 1,
            minor: 1,
            patch: 0,
        };

        registry
            .register_version("test-skill", v1.clone(), "hash1", 0.8, "/path")
            .unwrap();
        registry
            .register_version("test-skill", v2.clone(), "hash2", 0.9, "/path")
            .unwrap();

        // v2 is active, rollback to v1
        registry.rollback_to("test-skill", &v1).unwrap();

        let versions = registry.get_versions("test-skill");
        let v1_rec = versions.iter().find(|r| r.version == v1).unwrap();
        let v2_rec = versions.iter().find(|r| r.version == v2).unwrap();
        assert!(v1_rec.active);
        assert!(!v2_rec.active);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_compare_versions() {
        let path = temp_registry();
        let mut registry = SkillRegistry::new(path.clone()).unwrap();
        let v1 = SkillVersion {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let v2 = SkillVersion {
            major: 1,
            minor: 1,
            patch: 0,
        };

        registry
            .register_version("test-skill", v1.clone(), "hash1", 0.5, "/path")
            .unwrap();
        registry
            .register_version("test-skill", v2.clone(), "hash2", 0.5, "/path")
            .unwrap();

        // v1: 3 uses, 3 successes (100%)
        for _ in 0..3 {
            registry.record_usage("test-skill", &v1, true).unwrap();
        }
        // v2: 3 uses, 1 success (33%)
        registry.record_usage("test-skill", &v2, true).unwrap();
        registry.record_usage("test-skill", &v2, false).unwrap();
        registry.record_usage("test-skill", &v2, false).unwrap();

        let cmp = registry.compare_versions("test-skill").unwrap();
        assert_eq!(cmp.active_version.major, 1); // v2 is active (last registered)
        assert_eq!(cmp.active_version.minor, 1);
        assert!(cmp.should_promote); // v1 has better success rate

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        let h3 = content_hash("hello worlD");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_registry_summary() {
        let path = temp_registry();
        let mut registry = SkillRegistry::new(path.clone()).unwrap();

        let v1 = SkillVersion {
            major: 1,
            minor: 0,
            patch: 0,
        };
        registry
            .register_version("skill-a", v1.clone(), "h1", 0.8, "/a")
            .unwrap();
        registry
            .register_version(
                "skill-a",
                SkillVersion {
                    major: 1,
                    minor: 1,
                    patch: 0,
                },
                "h2",
                0.9,
                "/a",
            )
            .unwrap();
        registry
            .register_version("skill-b", v1, "h3", 0.7, "/b")
            .unwrap();

        let summary = registry.summary();
        assert_eq!(summary.total_skills, 2);
        assert_eq!(summary.total_versions, 3);
        assert_eq!(summary.active_count, 2);

        let _ = fs::remove_file(&path);
    }
}
