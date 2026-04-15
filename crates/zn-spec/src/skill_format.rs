//! SKILL.md Format Definition
//!
//! This module defines the standard format for Zero_Nine skill files.
//! Skills are markdown files with YAML frontmatter that describe reusable workflows.
//!
//! ## Format Specification
//!
//! ```markdown
//! ---
//! name: zero-nine-tdd-cycle
//! description: Test-Driven Development cycle with TDD-first implementation
//! version: 1.0.0
//! category: execution
//! platforms: [claude-code, opencode]
//! metadata:
//!   zero-nine:
//!     layer: execution
//!     requires: [zero-nine-spec-capture]
//!     triggers: [task.tdd_cycle]
//! ---
//!
//! # Skill Name
//!
//! ## When to Use
//! - Condition 1
//! - Condition 2
//!
//! ## Procedure
//! 1. Step 1
//! 2. Step 2
//!
//! ## Pitfalls
//! - Common mistake 1
//! - Common mistake 2
//!
//! ## Verification
//! - How to verify success
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Schema version for SKILL.md format
pub const SKILL_SCHEMA_VERSION: &str = "1.0.0";

/// SKILL.md frontmatter structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    /// Unique skill identifier (e.g., "zero-nine-tdd-cycle")
    pub name: String,
    /// One-line description of what the skill does
    pub description: String,
    /// Semantic version (e.g., "1.0.0")
    pub version: String,
    /// Skill category: brainstorming, spec, execution, verification, evolution
    pub category: String,
    /// Supported platforms
    pub platforms: Vec<String>,
    /// Optional metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SkillMetadata>,
}

/// Skill metadata for Zero_Nine integration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zero_nine: Option<ZeroNineMetadata>,
}

/// Zero_Nine specific metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ZeroNineMetadata {
    /// Which layer this skill belongs to
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// Required skills that must be loaded
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Vec<String>>,
    /// Triggers that activate this skill
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triggers: Option<Vec<String>>,
}

/// Parsed skill file with frontmatter and content
#[derive(Debug, Clone)]
pub struct SkillFile {
    pub frontmatter: SkillFrontmatter,
    pub content: String,
    pub raw: String,
}

impl SkillFile {
    /// Parse a skill file from raw content
    pub fn parse(content: &str) -> Result<Self> {
        // Find the frontmatter boundaries
        if !content.starts_with("---") {
            anyhow::bail!("Skill file must start with YAML frontmatter delimiter '---'");
        }

        let mut lines = content.lines();
        lines.next(); // Skip opening ---

        let mut frontmatter_lines = Vec::new();
        let mut content_started = false;
        let mut content_lines = Vec::new();

        for line in lines {
            if !content_started {
                if line.trim() == "---" {
                    content_started = true;
                    continue;
                }
                frontmatter_lines.push(line);
            } else {
                content_lines.push(line);
            }
        }

        let frontmatter_str = frontmatter_lines.join("\n");
        let frontmatter: SkillFrontmatter = serde_yaml::from_str(&frontmatter_str)
            .with_context(|| "Failed to parse YAML frontmatter")?;

        let content = content_lines.join("\n");

        Ok(Self {
            frontmatter,
            content: content.clone(),
            raw: content,
        })
    }

    /// Validate the skill file
    pub fn validate(&self) -> Vec<SkillValidationIssue> {
        let mut issues = Vec::new();

        // Check name format
        if !self.frontmatter.name.starts_with("zero-nine-") {
            issues.push(SkillValidationIssue {
                severity: SkillValidationSeverity::Warning,
                code: "name_prefix".to_string(),
                message: "Skill name should start with 'zero-nine-'".to_string(),
            });
        }

        // Check version format
        if !is_valid_semver(&self.frontmatter.version) {
            issues.push(SkillValidationIssue {
                severity: SkillValidationSeverity::Error,
                code: "invalid_version".to_string(),
                message: format!(
                    "Version '{}' is not valid semver (expected X.Y.Z)",
                    self.frontmatter.version
                ),
            });
        }

        // Check category
        let valid_categories = ["brainstorming", "spec", "execution", "verification", "evolution"];
        if !valid_categories.contains(&self.frontmatter.category.as_str()) {
            issues.push(SkillValidationIssue {
                severity: SkillValidationSeverity::Warning,
                code: "unknown_category".to_string(),
                message: format!(
                    "Unknown category '{}'. Valid categories: {:?}",
                    self.frontmatter.category, valid_categories
                ),
            });
        }

        // Check content sections
        let content_lower = self.content.to_lowercase();
        let required_sections = ["when to use", "procedure"];
        for section in required_sections {
            if !content_lower.contains(section) {
                issues.push(SkillValidationIssue {
                    severity: SkillValidationSeverity::Warning,
                    code: "missing_section".to_string(),
                    message: format!("Missing recommended section: '{}'", section),
                });
            }
        }

        issues
    }

    /// Render the skill file back to markdown
    pub fn render(&self) -> String {
        let frontmatter_yaml = serde_yaml::to_string(&self.frontmatter)
            .unwrap_or_else(|_| "name: invalid\n".to_string());
        format!("---\n{}\n---\n{}\n", frontmatter_yaml, self.content)
    }
}

/// Validation issue for skill files
#[derive(Debug, Clone)]
pub struct SkillValidationIssue {
    pub severity: SkillValidationSeverity,
    pub code: String,
    pub message: String,
}

/// Validation severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillValidationSeverity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for SkillValidationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => write!(f, "Error"),
            Self::Warning => write!(f, "Warning"),
            Self::Info => write!(f, "Info"),
        }
    }
}

/// Skill summary for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub version: String,
    pub category: String,
    pub valid: bool,
}

impl From<&SkillFile> for SkillSummary {
    fn from(skill: &SkillFile) -> Self {
        let issues = skill.validate();
        let has_errors = issues
            .iter()
            .any(|i| matches!(i.severity, SkillValidationSeverity::Error));

        Self {
            name: skill.frontmatter.name.clone(),
            description: skill.frontmatter.description.clone(),
            version: skill.frontmatter.version.clone(),
            category: skill.frontmatter.category.clone(),
            valid: !has_errors,
        }
    }
}

/// Check if a version string is valid semver
fn is_valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts.iter().all(|p| p.parse::<u32>().is_ok())
}

/// Extract skill name from a directory path
pub fn extract_skill_name(dir: &Path) -> Option<String> {
    dir.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// Validate skill directory structure
pub fn validate_skill_dir(dir: &Path) -> Result<Vec<SkillValidationIssue>> {
    let mut all_issues = Vec::new();

    let skill_file = dir.join("SKILL.md");
    if !skill_file.exists() {
        all_issues.push(SkillValidationIssue {
            severity: SkillValidationSeverity::Error,
            code: "missing_skill_file".to_string(),
            message: "SKILL.md not found in directory".to_string(),
        });
        return Ok(all_issues);
    }

    let content = std::fs::read_to_string(&skill_file)
        .with_context(|| format!("Failed to read {}", skill_file.display()))?;

    let skill = SkillFile::parse(&content)?;
    let mut issues = skill.validate();

    // Check if directory name matches skill name
    if let Some(dir_name) = extract_skill_name(dir) {
        if dir_name != skill.frontmatter.name {
            issues.push(SkillValidationIssue {
                severity: SkillValidationSeverity::Warning,
                code: "name_mismatch".to_string(),
                message: format!(
                    "Directory name '{}' doesn't match skill name '{}'",
                    dir_name, skill.frontmatter.name
                ),
            });
        }
    }

    all_issues.extend(issues);
    Ok(all_issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_SKILL: &str = r#"---
name: zero-nine-tdd-cycle
description: Test-Driven Development cycle
version: 1.0.0
category: execution
platforms: [claude-code, opencode]
metadata:
  zero-nine:
    layer: execution
    requires: [zero-nine-spec-capture]
---

# TDD Cycle Skill

## When to Use
- Task mode is `tdd_cycle`
- Implementation requires test coverage

## Procedure
1. Read task contract
2. Generate failing test first
3. Implement minimum code to pass
4. Run verification

## Pitfalls
- Don't write tests after implementation

## Verification
- `cargo test --all-targets` must pass
"#;

    #[test]
    fn test_parse_valid_skill() {
        let skill = SkillFile::parse(VALID_SKILL).unwrap();
        assert_eq!(skill.frontmatter.name, "zero-nine-tdd-cycle");
        assert_eq!(skill.frontmatter.version, "1.0.0");
        assert_eq!(skill.frontmatter.category, "execution");
    }

    #[test]
    fn test_validate_valid_skill() {
        let skill = SkillFile::parse(VALID_SKILL).unwrap();
        let issues = skill.validate();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_parse_invalid_skill_no_frontmatter() {
        let result = SkillFile::parse("# Just content");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invalid_version() {
        let skill_content = r#"---
name: zero-nine-test
description: Test skill
version: invalid
category: execution
platforms: [claude-code]
---

## When to Use
- Always

## Procedure
1. Do something
"#;
        let skill = SkillFile::parse(skill_content).unwrap();
        let issues = skill.validate();
        assert!(issues.iter().any(|i| i.code == "invalid_version"));
    }

    #[test]
    fn test_render_skill() {
        let skill = SkillFile::parse(VALID_SKILL).unwrap();
        let rendered = skill.render();
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("name: zero-nine-tdd-cycle"));
    }

    #[test]
    fn test_is_valid_semver() {
        assert!(is_valid_semver("1.0.0"));
        assert!(is_valid_semver("2.1.3"));
        assert!(!is_valid_semver("1.0"));
        assert!(!is_valid_semver("1"));
        assert!(!is_valid_semver("invalid"));
    }
}
