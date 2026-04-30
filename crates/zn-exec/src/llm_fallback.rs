//! LLM Fallback — direct API calls when the host CLI is unavailable.
//!
//! Provides a synchronous wrapper around zn-evolve's async `AIClient`,
//! enabling zn-exec to invoke LLMs without requiring an async runtime
//! at the top level.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use zn_evolve::{AIClient, AIClientConfig};

/// Maximum tokens for subagent dispatch prompts.
const SUBAGENT_MAX_TOKENS: u32 = 8192;

/// Temperature for subagent tasks (lower = more deterministic).
const SUBAGENT_TEMPERATURE: f32 = 0.3;

/// Parsed output from an LLM response.
pub struct ParsedLlmResponse {
    /// Non-code prose from the response.
    pub summary: String,
    /// File paths extracted from fenced code blocks.
    pub output_files: Vec<String>,
    /// (file_path, content) pairs from fenced code blocks.
    pub file_contents: Vec<(String, String)>,
    /// Whether the response looks meaningful.
    pub success: bool,
}

/// Call the LLM API synchronously with the given prompt.
pub fn call_llm_sync(prompt: &str, system: Option<&str>) -> Result<zn_evolve::AIResponse> {
    let config = AIClientConfig::default();
    let client = AIClient::new(config)
        .context("Failed to create AIClient — set ANTHROPIC_AUTH_TOKEN or ANTHROPIC_API_KEY")?;

    let rt =
        tokio::runtime::Runtime::new().context("Failed to create tokio runtime for LLM call")?;
    rt.block_on(async {
        client
            .send_message_with_config(
                prompt,
                system,
                Some(SUBAGENT_MAX_TOKENS),
                Some(SUBAGENT_TEMPERATURE),
            )
            .await
    })
}

/// Send a subagent dispatch prompt via the LLM API.
pub fn execute_dispatch_via_llm(role: &str, prompt: &str) -> Result<zn_evolve::AIResponse> {
    let system = format!(
        "You are a {} agent in the Zero_Nine orchestration system. \
         Produce concrete, actionable output. When creating or modifying files, \
         output them in markdown code blocks with a language tag and file path \
         on the opening fence (e.g. ```rust src/lib.rs).",
        role,
    );
    call_llm_sync(prompt, Some(&system))
}

/// Parse an LLM response into structured output.
///
/// Extracts file paths from fenced code blocks whose opening line
/// contains a path token (e.g. ````rust src/main.rs`), and collects
/// the body text of each such block as file content.
pub fn parse_llm_response(raw: &str) -> ParsedLlmResponse {
    let mut output_files = Vec::new();
    let mut file_contents = Vec::new();
    let mut summary_parts = Vec::new();

    let mut in_code_block = false;
    let mut current_path: Option<String> = None;
    let mut current_content = String::new();

    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_code_block {
                // Closing fence — save if we had a path.
                if let Some(path) = current_path.take() {
                    output_files.push(path.clone());
                    file_contents.push((path, std::mem::take(&mut current_content)));
                }
                in_code_block = false;
            } else {
                // Opening fence — try to extract a file path.
                let fence = trimmed.trim_start_matches('`').trim();
                current_path = extract_file_path_from_fence(fence);
                in_code_block = true;
            }
            continue;
        }
        if in_code_block {
            current_content.push_str(line);
            current_content.push('\n');
        } else {
            summary_parts.push(line);
        }
    }

    // Handle unclosed code block at end of input.
    if in_code_block {
        if let Some(path) = current_path {
            output_files.push(path.clone());
            file_contents.push((path, current_content));
        }
    }

    let summary = summary_parts.join("\n").trim().to_string();
    let success = !raw.trim().is_empty() && raw.trim().len() > 10;

    ParsedLlmResponse {
        summary: if summary.is_empty() {
            "LLM response produced successfully".to_string()
        } else {
            summary
        },
        output_files,
        file_contents,
        success,
    }
}

/// Write extracted file contents to disk under `project_root`.
pub fn write_parsed_files(
    project_root: &Path,
    file_contents: &[(String, String)],
) -> Result<Vec<String>> {
    let mut written = Vec::new();
    for (path, content) in file_contents {
        let full_path = if PathBuf::from(path).is_absolute() {
            PathBuf::from(path)
        } else {
            project_root.join(path)
        };
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, content)?;
        written.push(full_path.display().to_string());
    }
    Ok(written)
}

/// Extract a file path from a fenced code block header.
///
/// Accepted formats: ````rust src/main.rs`, ```path/to/file`,
/// ``` path/to/file`, or a single token that looks like a path.
fn extract_file_path_from_fence(fence: &str) -> Option<String> {
    let parts: Vec<&str> = fence.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    // Two+ tokens: second token is likely the path.
    if parts.len() >= 2 {
        let candidate = parts[1];
        if is_path_like(candidate) {
            return Some(candidate.to_string());
        }
    }
    // Single token that looks like a path.
    if parts.len() == 1 && is_path_like(parts[0]) {
        return Some(parts[0].to_string());
    }
    None
}

/// Heuristic: does this token look like a file path?
fn is_path_like(s: &str) -> bool {
    s.contains('/') || s.contains('.') || s.contains('\\')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_parse_with_code_blocks() {
        let raw = r#"Here is the implementation.

```rust src/lib.rs
pub fn hello() {
    println!("Hello");
}
```

```python scripts/run.py
import sys
print("Running")
```
"#;
        let parsed = parse_llm_response(raw);
        assert_eq!(parsed.output_files.len(), 2);
        assert!(parsed.output_files.contains(&"src/lib.rs".to_string()));
        assert!(parsed.output_files.contains(&"scripts/run.py".to_string()));
        assert_eq!(parsed.file_contents.len(), 2);
        assert!(parsed.summary.contains("Here is the implementation"));
    }

    #[test]
    fn test_parse_without_code_blocks() {
        let raw = "The task is complete. All acceptance criteria met.";
        let parsed = parse_llm_response(raw);
        assert!(parsed.output_files.is_empty());
        assert!(parsed.file_contents.is_empty());
        assert!(parsed.summary.contains("complete"));
        assert!(parsed.success);
    }

    #[test]
    fn test_parse_mixed_prose_and_code() {
        let raw = r#"I've analyzed the requirements.

```markdown docs/analysis.md
# Analysis

The feature requires three changes.
```

Additional notes below.
"#;
        let parsed = parse_llm_response(raw);
        assert_eq!(parsed.output_files, vec!["docs/analysis.md"]);
        assert!(parsed.summary.contains("Additional notes below"));
        assert!(parsed.file_contents[0].1.contains("# Analysis"));
    }

    #[test]
    fn test_parse_unclosed_code_block() {
        let raw = "```rust src/missing.rs\npub fn x() {}\n";
        let parsed = parse_llm_response(raw);
        assert_eq!(parsed.output_files, vec!["src/missing.rs"]);
        assert_eq!(parsed.file_contents.len(), 1);
    }

    #[test]
    fn test_extract_file_path_variants() {
        assert_eq!(
            extract_file_path_from_fence("rust src/main.rs"),
            Some("src/main.rs".to_string())
        );
        assert_eq!(
            extract_file_path_from_fence("path/to/file.md"),
            Some("path/to/file.md".to_string())
        );
        assert_eq!(extract_file_path_from_fence("rust"), None);
        assert_eq!(extract_file_path_from_fence(""), None);
    }

    #[test]
    fn test_write_parsed_files_creates_directories() {
        let tmp = temp_dir().join("llm_write_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let contents = vec![(
            "deep/nested/dir/file.txt".to_string(),
            "hello\n".to_string(),
        )];
        let written = write_parsed_files(&tmp, &contents).unwrap();
        assert_eq!(written.len(), 1);
        assert!(written[0].ends_with("deep/nested/dir/file.txt"));
        assert!(std::fs::read_to_string(&written[0])
            .unwrap()
            .contains("hello"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_empty_response_not_success() {
        let parsed = parse_llm_response("");
        assert!(!parsed.success);
    }

    #[test]
    fn test_short_response_not_success() {
        let parsed = parse_llm_response("ok");
        assert!(!parsed.success);
    }
}
