//! Memory Tool - Manage Zero_Nine agent memory
//!
//! This module provides:
//! - MEMORY.md management (agent notes, environment facts, project conventions)
//! - USER.md management (user preferences, communication style, code style)
//! - Memory action execution (add, replace, remove)

use anyhow::{Context, Result};
use zn_types::MemoryToolError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Memory target for operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTarget {
    Memory, // MEMORY.md
    User,   // USER.md
}

impl std::fmt::Display for MemoryTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryTarget::Memory => write!(f, "MEMORY.md"),
            MemoryTarget::User => write!(f, "USER.md"),
        }
    }
}

/// Memory action to perform
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum MemoryAction {
    /// Add new content to memory
    Add {
        target: MemoryTarget,
        content: String,
        section: Option<String>,
    },
    /// Replace existing content
    Replace {
        target: MemoryTarget,
        old_text: String,
        content: String,
    },
    /// Remove content
    Remove {
        target: MemoryTarget,
        old_text: String,
    },
}

/// Memory action result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryResult {
    pub success: bool,
    pub target: String,
    pub action: String,
    pub message: String,
}

/// Memory manager for Zero_Nine
pub struct MemoryManager {
    memory_dir: PathBuf,
    memory_file: PathBuf,
    user_file: PathBuf,
}

impl MemoryManager {
    /// Create a new MemoryManager
    pub fn new(memory_dir: PathBuf) -> Result<Self> {
        let manager = Self {
            memory_dir: memory_dir.clone(),
            memory_file: memory_dir.join("MEMORY.md"),
            user_file: memory_dir.join("USER.md"),
        };
        manager.ensure_files_exist()?;
        Ok(manager)
    }

    /// Ensure memory files exist
    pub fn ensure_files_exist(&self) -> Result<()> {
        fs::create_dir_all(&self.memory_dir)?;

        if !self.memory_file.exists() {
            fs::write(&self.memory_file, Self::default_memory_content())?;
        }

        if !self.user_file.exists() {
            fs::write(&self.user_file, Self::default_user_content())?;
        }

        Ok(())
    }

    fn default_memory_content() -> &'static str {
        r#"---
name: Project Memory
description: Environment facts, project conventions, and learned patterns
created: YYYY-MM-DD
updated: YYYY-MM-DD
---

# Project Memory

## Environment Facts
<!-- Facts about the development environment -->

## Project Conventions
<!-- Coding conventions and patterns specific to this project -->

## Known Issues
<!-- Known issues and their workarounds -->

## Learnings
<!-- Lessons learned from past incidents -->
"#
    }

    fn default_user_content() -> &'static str {
        r#"---
name: User Profile
description: User preferences, communication style, and code style preferences
created: YYYY-MM-DD
updated: YYYY-MM-DD
---

# User Profile

## Communication Preferences
<!-- How the user prefers to receive information -->

## Code Style Preferences
<!-- Naming conventions, formatting preferences, etc. -->

## Quality Requirements
<!-- Testing requirements, documentation standards, etc. -->

## Tool Preferences
<!-- Preferred tools and workflows -->
"#
    }

    /// Get the path to a memory file
    pub fn get_file(&self, target: &MemoryTarget) -> &PathBuf {
        match target {
            MemoryTarget::Memory => &self.memory_file,
            MemoryTarget::User => &self.user_file,
        }
    }

    /// Read memory content
    pub fn read(&self, target: &MemoryTarget) -> Result<String> {
        let path = self.get_file(target);
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", target))
    }

    /// Execute a memory action
    pub fn execute(&mut self, action: &MemoryAction) -> Result<MemoryResult> {
        match action {
            MemoryAction::Add {
                target,
                content,
                section,
            } => {
                self.add_content(target, content, section.as_deref())?;
                Ok(MemoryResult {
                    success: true,
                    target: target.to_string(),
                    action: "add".to_string(),
                    message: "Content added successfully".to_string(),
                })
            }
            MemoryAction::Replace {
                target,
                old_text,
                content,
            } => {
                self.replace_content(target, old_text, content)?;
                Ok(MemoryResult {
                    success: true,
                    target: target.to_string(),
                    action: "replace".to_string(),
                    message: "Content replaced successfully".to_string(),
                })
            }
            MemoryAction::Remove { target, old_text } => {
                self.remove_content(target, old_text)?;
                Ok(MemoryResult {
                    success: true,
                    target: target.to_string(),
                    action: "remove".to_string(),
                    message: "Content removed successfully".to_string(),
                })
            }
        }
    }

    /// Add content to memory
    fn add_content(
        &self,
        target: &MemoryTarget,
        content: &str,
        section: Option<&str>,
    ) -> Result<()> {
        let path = self.get_file(target);
        let mut current =
            fs::read_to_string(path).with_context(|| format!("Failed to read {}", target))?;

        // Update the frontmatter timestamp
        current = update_frontmatter_timestamp(&current);

        if let Some(section_name) = section {
            // Add to specific section
            let section_header = format!("## {}", section_name);
            if current.contains(&section_header) {
                // Find the section and append after it
                let lines: Vec<&str> = current.lines().collect();
                let mut new_lines = Vec::new();
                let mut in_section = false;
                let mut added = false;

                for line in lines {
                    new_lines.push(line);
                    if line == &section_header {
                        in_section = true;
                    } else if in_section && line.starts_with("##") && !added {
                        // Next section, insert content before it
                        new_lines.push("");
                        new_lines.push(content);
                        added = true;
                        in_section = false;
                    }
                }

                // If we're still in section at end, append
                if in_section && !added {
                    new_lines.push("");
                    new_lines.push(content);
                }

                current = new_lines.join("\n");
            } else {
                // Section doesn't exist, create it
                current.push_str(&format!("\n{}\n\n{}\n", section_header, content));
            }
        } else {
            // Append to end
            current.push_str(&format!("\n{}\n", content));
        }

        fs::write(path, &current)?;
        Ok(())
    }

    /// Replace content in memory
    fn replace_content(&self, target: &MemoryTarget, old_text: &str, new_text: &str) -> Result<()> {
        let path = self.get_file(target);
        let mut current =
            fs::read_to_string(path).with_context(|| format!("Failed to read {}", target))?;

        if !current.contains(old_text) {
            return Err(MemoryToolError::OldTextNotFound {
                target: target.to_string(),
            }
            .into());
        }

        current = current.replace(old_text, new_text);
        current = update_frontmatter_timestamp(&current);

        fs::write(path, &current)?;
        Ok(())
    }

    /// Remove content from memory
    fn remove_content(&self, target: &MemoryTarget, old_text: &str) -> Result<()> {
        let path = self.get_file(target);
        let mut current =
            fs::read_to_string(path).with_context(|| format!("Failed to read {}", target))?;

        if !current.contains(old_text) {
            return Err(MemoryToolError::RemoveTargetNotFound {
                target: target.to_string(),
            }
            .into());
        }

        current = current.replace(old_text, "");
        current = update_frontmatter_timestamp(&current);

        fs::write(path, &current)?;
        Ok(())
    }

    /// Get memory summary
    pub fn get_summary(&self) -> Result<MemorySummary> {
        let memory_content = self.read(&MemoryTarget::Memory)?;
        let user_content = self.read(&MemoryTarget::User)?;

        Ok(MemorySummary {
            memory_sections: extract_sections(&memory_content),
            user_sections: extract_sections(&user_content),
            memory_size: memory_content.len(),
            user_size: user_content.len(),
            last_updated: Utc::now(),
        })
    }
}

/// Memory summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySummary {
    pub memory_sections: Vec<String>,
    pub user_sections: Vec<String>,
    pub memory_size: usize,
    pub user_size: usize,
    pub last_updated: chrono::DateTime<Utc>,
}

/// Update the frontmatter timestamp
fn update_frontmatter_timestamp(content: &str) -> String {
    let now = Utc::now().format("%Y-%m-%d").to_string();
    let lines: Vec<&str> = content.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let mut in_frontmatter = false;
    let mut updated = false;
    let updated_line = format!("updated: {}", now);

    for line in lines {
        if line.trim() == "---" {
            in_frontmatter = !in_frontmatter;
            result.push(line.to_string());
            continue;
        }

        if in_frontmatter && line.starts_with("updated:") {
            result.push(updated_line.clone());
            updated = true;
        } else {
            result.push(line.to_string());
        }
    }

    // If no frontmatter timestamp found, add one after created
    if !updated {
        let mut final_result = Vec::new();
        for line in result {
            if line.starts_with("created:") {
                final_result.push(line);
                final_result.push(updated_line.clone());
            } else {
                final_result.push(line);
            }
        }
        final_result.join("\n")
    } else {
        result.join("\n")
    }
}

/// Extract section headers from markdown content
fn extract_sections(content: &str) -> Vec<String> {
    content
        .lines()
        .filter(|line| line.starts_with("## "))
        .map(|line| line.trim_start_matches("## ").trim().to_string())
        .collect()
}

/// Create a default memory manager for a project
pub fn create_default_manager(project_root: &Path) -> Result<MemoryManager> {
    let memory_dir = project_root.join(".zero_nine").join("memory");
    MemoryManager::new(memory_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_create_memory_manager() {
        let tmp_dir = temp_dir().join("zn_memory_test");
        let _ = fs::remove_dir_all(&tmp_dir);

        let manager = MemoryManager::new(tmp_dir.clone()).unwrap();
        assert!(manager.memory_file.exists());
        assert!(manager.user_file.exists());

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_add_content() {
        let tmp_dir = temp_dir().join("zn_memory_add_test");
        let _ = fs::remove_dir_all(&tmp_dir);

        let mut manager = MemoryManager::new(tmp_dir.clone()).unwrap();
        let action = MemoryAction::Add {
            target: MemoryTarget::Memory,
            content: "Test content".to_string(),
            section: None,
        };

        let result = manager.execute(&action).unwrap();
        assert!(result.success);

        let content = manager.read(&MemoryTarget::Memory).unwrap();
        assert!(content.contains("Test content"));

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_replace_content() {
        let tmp_dir = temp_dir().join("zn_memory_replace_test");
        let _ = fs::remove_dir_all(&tmp_dir);

        let mut manager = MemoryManager::new(tmp_dir.clone()).unwrap();

        // First add some content
        let add_action = MemoryAction::Add {
            target: MemoryTarget::Memory,
            content: "Old content".to_string(),
            section: None,
        };
        manager.execute(&add_action).unwrap();

        // Then replace it
        let replace_action = MemoryAction::Replace {
            target: MemoryTarget::Memory,
            old_text: "Old content".to_string(),
            content: "New content".to_string(),
        };
        let result = manager.execute(&replace_action).unwrap();
        assert!(result.success);

        let content = manager.read(&MemoryTarget::Memory).unwrap();
        assert!(content.contains("New content"));
        assert!(!content.contains("Old content"));

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_get_summary() {
        let tmp_dir = temp_dir().join("zn_memory_summary_test");
        let _ = fs::remove_dir_all(&tmp_dir);

        let manager = MemoryManager::new(tmp_dir.clone()).unwrap();
        let summary = manager.get_summary().unwrap();

        assert!(!summary.memory_sections.is_empty());
        assert!(!summary.user_sections.is_empty());

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
