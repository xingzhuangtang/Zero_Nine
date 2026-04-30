//! Skill Manager - Create, patch, edit, delete, list, and view skills
//!
//! This module provides programmatic access to skill management operations.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use zn_types::SkillError;

use crate::skill_format::{SkillFile, SkillSummary};

/// Manager for skill operations
pub struct SkillManager {
    skills_dir: PathBuf,
}

impl SkillManager {
    /// Create a new SkillManager with the given skills directory
    pub fn new(skills_dir: PathBuf) -> Self {
        Self { skills_dir }
    }

    /// Create a new skill
    ///
    /// # Arguments
    /// * `name` - Skill name (e.g., "zero-nine-tdd-cycle")
    /// * `content` - Skill markdown content (without frontmatter)
    /// * `category` - Skill category (brainstorming, spec, execution, verification, evolution)
    /// * `description` - One-line description
    /// * `version` - Semantic version (e.g., "1.0.0")
    pub fn create(
        &self,
        name: &str,
        content: &str,
        category: &str,
        description: &str,
        version: &str,
    ) -> Result<PathBuf> {
        let skill_dir = self.skills_dir.join(name);
        fs::create_dir_all(&skill_dir).with_context(|| {
            format!("Failed to create skill directory: {}", skill_dir.display())
        })?;

        let frontmatter = format!(
            r#"---
name: {}
description: {}
version: {}
category: {}
platforms: [claude-code, opencode]
---
"#,
            name, description, version, category
        );

        let skill_file = skill_dir.join("SKILL.md");
        let full_content = format!("{}\n{}", frontmatter, content);
        fs::write(&skill_file, &full_content)
            .with_context(|| format!("Failed to write skill file: {}", skill_file.display()))?;

        Ok(skill_file)
    }

    /// Patch an existing skill (replace specific text)
    pub fn patch(&self, name: &str, old_string: &str, new_string: &str) -> Result<PathBuf> {
        let skill_file = self.get_skill_file_path(name)?;
        let content = fs::read_to_string(&skill_file)
            .with_context(|| format!("Failed to read skill file: {}", skill_file.display()))?;

        if !content.contains(old_string) {
            return Err(SkillError::PatchTargetNotFound.into());
        }

        let patched = content.replace(old_string, new_string);
        fs::write(&skill_file, &patched).with_context(|| {
            format!(
                "Failed to write patched skill file: {}",
                skill_file.display()
            )
        })?;

        Ok(skill_file)
    }

    /// Edit an entire skill (replace content after frontmatter)
    pub fn edit(&self, name: &str, new_content: &str) -> Result<PathBuf> {
        let skill_file = self.get_skill_file_path(name)?;
        let raw = fs::read_to_string(&skill_file)
            .with_context(|| format!("Failed to read skill file: {}", skill_file.display()))?;

        // Parse to get frontmatter
        let skill = SkillFile::parse(&raw)?;

        // Rebuild with new content
        let frontmatter_yaml = serde_yaml::to_string(&skill.frontmatter)
            .unwrap_or_else(|_| "name: invalid\n".to_string());
        let full_content = format!("---\n{}\n---\n{}\n", frontmatter_yaml, new_content);

        fs::write(&skill_file, &full_content).with_context(|| {
            format!(
                "Failed to write edited skill file: {}",
                skill_file.display()
            )
        })?;

        Ok(skill_file)
    }

    /// Delete a skill
    pub fn delete(&self, name: &str) -> Result<()> {
        let skill_dir = self.skills_dir.join(name);
        if !skill_dir.exists() {
            return Err(SkillError::NotFound { name: name.to_string() }.into());
        }
        fs::remove_dir_all(&skill_dir).with_context(|| {
            format!("Failed to delete skill directory: {}", skill_dir.display())
        })?;
        Ok(())
    }

    /// List all available skills
    pub fn list(&self) -> Result<Vec<SkillSummary>> {
        let mut skills = Vec::new();

        if !self.skills_dir.exists() {
            return Ok(skills);
        }

        for entry in fs::read_dir(&self.skills_dir).with_context(|| {
            format!(
                "Failed to read skills directory: {}",
                self.skills_dir.display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let skill_file = path.join("SKILL.md");
            if !skill_file.exists() {
                continue;
            }

            match fs::read_to_string(&skill_file) {
                Ok(content) => {
                    if let Ok(skill) = SkillFile::parse(&content) {
                        skills.push(SkillSummary::from(&skill));
                    }
                }
                Err(_) => {
                    // Skip invalid files
                    continue;
                }
            }
        }

        // Sort by name
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }

    /// View a skill file
    pub fn view(&self, name: &str) -> Result<SkillFile> {
        let skill_file = self.get_skill_file_path(name)?;
        let content = fs::read_to_string(&skill_file)
            .with_context(|| format!("Failed to read skill file: {}", skill_file.display()))?;
        SkillFile::parse(&content)
    }

    /// Validate a skill file
    pub fn validate(&self, name: &str) -> Result<Vec<crate::skill_format::SkillValidationIssue>> {
        let _skill_file = self.get_skill_file_path(name)?;
        crate::skill_format::validate_skill_dir(&self.skills_dir.join(name))
    }

    /// Get the path to a skill file
    fn get_skill_file_path(&self, name: &str) -> Result<PathBuf> {
        let skill_file = self.skills_dir.join(name).join("SKILL.md");
        if !skill_file.exists() {
            return Err(SkillError::NotFound { name: name.to_string() }.into());
        }
        Ok(skill_file)
    }
}

/// Create a SkillManager for the default Zero_Nine skills directory
pub fn create_default_manager(project_root: &Path) -> SkillManager {
    let skills_dir = project_root
        .join(".zero_nine")
        .join("evolve")
        .join("skills");
    SkillManager::new(skills_dir)
}

/// Create a SkillManager for adapter skills directory
pub fn create_adapter_manager(adapter_root: &Path) -> SkillManager {
    let skills_dir = adapter_root.join(".claude").join("skills");
    SkillManager::new(skills_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    const SAMPLE_CONTENT: &str = r#"
# Sample Skill

## When to Use
- When you need to do something

## Procedure
1. Do the thing
2. Verify the result

## Pitfalls
- Don't skip step 1
"#;

    #[test]
    fn test_create_skill() {
        let tmp_dir = temp_dir().join("zn_skill_test");
        let _ = fs::remove_dir_all(&tmp_dir);

        let manager = SkillManager::new(tmp_dir.clone());
        let result = manager.create(
            "zero-nine-test-skill",
            SAMPLE_CONTENT,
            "execution",
            "A test skill",
            "1.0.0",
        );

        assert!(result.is_ok());
        let skill_file = result.unwrap();
        assert!(skill_file.exists());

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_list_skills() {
        let tmp_dir = temp_dir().join("zn_skill_list_test");
        let _ = fs::remove_dir_all(&tmp_dir);

        let manager = SkillManager::new(tmp_dir.clone());
        manager
            .create(
                "zero-nine-test-1",
                SAMPLE_CONTENT,
                "execution",
                "Test skill 1",
                "1.0.0",
            )
            .unwrap();
        manager
            .create(
                "zero-nine-test-2",
                SAMPLE_CONTENT,
                "brainstorming",
                "Test skill 2",
                "1.0.0",
            )
            .unwrap();

        let skills = manager.list().unwrap();
        assert_eq!(skills.len(), 2);

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_view_skill() {
        let tmp_dir = temp_dir().join("zn_skill_view_test");
        let _ = fs::remove_dir_all(&tmp_dir);

        let manager = SkillManager::new(tmp_dir.clone());
        manager
            .create(
                "zero-nine-view-test",
                SAMPLE_CONTENT,
                "execution",
                "View test",
                "1.0.0",
            )
            .unwrap();

        let skill = manager.view("zero-nine-view-test").unwrap();
        assert_eq!(skill.frontmatter.name, "zero-nine-view-test");
        assert_eq!(skill.frontmatter.category, "execution");

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_patch_skill() {
        let tmp_dir = temp_dir().join("zn_skill_patch_test");
        let _ = fs::remove_dir_all(&tmp_dir);

        let manager = SkillManager::new(tmp_dir.clone());
        manager
            .create(
                "zero-nine-patch-test",
                SAMPLE_CONTENT,
                "execution",
                "Patch test",
                "1.0.0",
            )
            .unwrap();

        let result = manager.patch(
            "zero-nine-patch-test",
            "Do the thing",
            "Do the modified thing",
        );
        assert!(result.is_ok());

        let skill = manager.view("zero-nine-patch-test").unwrap();
        assert!(skill.content.contains("Do the modified thing"));

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_delete_skill() {
        let tmp_dir = temp_dir().join("zn_skill_delete_test");
        let _ = fs::remove_dir_all(&tmp_dir);

        let manager = SkillManager::new(tmp_dir.clone());
        let skill_path = manager
            .create(
                "zero-nine-delete-test",
                SAMPLE_CONTENT,
                "execution",
                "Delete test",
                "1.0.0",
            )
            .unwrap();

        assert!(skill_path.exists());
        let result = manager.delete("zero-nine-delete-test");
        assert!(result.is_ok());
        assert!(!skill_path.exists());

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
