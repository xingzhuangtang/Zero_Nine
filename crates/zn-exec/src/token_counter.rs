//! Token Counter - Estimate token usage for text content
//!
//! This module provides:
//! - Simple token estimation based on character/word counts
//! - Output truncation with head/tail preservation
//! - Smart filtering (progress bars, ANSI codes, duplicate lines)

use std::collections::HashSet;

/// Token counter for estimating LLM token usage
pub struct TokenCounter {
    /// Characters per token approximation (average for English/Chinese mixed)
    chars_per_token: f64,
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self {
            // Average: ~4 chars/token for English, ~1.5 chars/token for Chinese
            // Using 3.5 as a reasonable middle ground
            chars_per_token: 3.5,
        }
    }
}

impl TokenCounter {
    /// Create a new TokenCounter
    pub fn new(chars_per_token: f64) -> Self {
        Self { chars_per_token }
    }

    /// Count tokens in text using character-based estimation
    pub fn count_tokens(&self, text: &str) -> u64 {
        if text.is_empty() {
            return 0;
        }

        // More accurate: count words for English, characters for Chinese
        let mut token_count = 0.0;
        let mut in_word = false;
        let mut word_len = 0;

        for c in text.chars() {
            if c.is_ascii_alphanumeric() {
                if !in_word {
                    // Start of new word
                    if in_word {
                        token_count += (word_len as f64 / self.chars_per_token).max(1.0);
                    }
                    in_word = true;
                    word_len = 0;
                }
                word_len += 1;
            } else if c.is_whitespace() {
                if in_word {
                    token_count += (word_len as f64 / self.chars_per_token).max(1.0);
                    in_word = false;
                    word_len = 0;
                }
            } else {
                // Non-ASCII character (likely Chinese/Japanese/etc.)
                if in_word {
                    token_count += (word_len as f64 / self.chars_per_token).max(1.0);
                    in_word = false;
                }
                // Each CJK character is roughly 1 token
                token_count += 1.0;
            }
        }

        // Handle last word
        if in_word {
            token_count += (word_len as f64 / self.chars_per_token).max(1.0);
        }

        token_count.ceil() as u64
    }

    /// Count tokens with metadata (includes breakdown)
    pub fn count_tokens_detailed(&self, text: &str) -> TokenCountResult {
        let total = self.count_tokens(text);
        let char_count = text.chars().count();
        let byte_count = text.len();
        let line_count = text.lines().count();

        TokenCountResult {
            tokens: total,
            char_count,
            byte_count,
            line_count,
        }
    }
}

/// Detailed token count result
#[derive(Debug, Clone)]
pub struct TokenCountResult {
    pub tokens: u64,
    pub char_count: usize,
    pub byte_count: usize,
    pub line_count: usize,
}

/// Output optimizer for reducing token usage
pub struct OutputOptimizer {
    /// Maximum lines to keep (0 = unlimited)
    max_lines: usize,
    /// Maximum characters to keep (0 = unlimited)
    max_chars: usize,
    /// Enable smart filtering
    smart_filter: bool,
}

impl Default for OutputOptimizer {
    fn default() -> Self {
        Self {
            max_lines: 200,
            max_chars: 10000,
            smart_filter: true,
        }
    }
}

impl OutputOptimizer {
    /// Create a new OutputOptimizer
    pub fn new(max_lines: usize, max_chars: usize, smart_filter: bool) -> Self {
        Self {
            max_lines,
            max_chars,
            smart_filter,
        }
    }

    /// Optimize command output for token efficiency
    pub fn optimize(&self, output: &str) -> String {
        let mut result = output.to_string();

        // Step 1: Smart filtering (remove noise)
        if self.smart_filter {
            result = self.filter_noise(&result);
        }

        // Step 2: Truncate if needed
        result = self.truncate(&result);

        result
    }

    /// Filter out common noise patterns
    fn filter_noise(&self, input: &str) -> String {
        let mut lines: Vec<String> = Vec::new();
        let mut seen_lines: HashSet<String> = HashSet::new();
        let mut duplicate_count = 0;

        for line in input.lines() {
            // Skip ANSI escape codes
            if line.contains('\x1b') || line.contains("[2J") || line.contains("[H") {
                continue;
            }

            // Skip progress bars (lines with repeated = or - characters)
            if line.contains("====")
                || line.contains("----")
                || line.contains("[====")
                || line.contains("[----")
            {
                continue;
            }

            // Skip loading bars (e.g., "Loading... [00:00:15]")
            if line.contains("Loading...") || line.contains("[00:00") {
                continue;
            }

            // Skip cargo build progress
            if line.contains("Compiling ") && line.contains("] ") {
                // Keep only every 10th compilation line
                if seen_lines.contains(line) {
                    duplicate_count += 1;
                    continue;
                }
            }

            // Skip npm/yarn progress output
            if line.contains("[2K") || line.contains("[1G") {
                continue;
            }

            // Deduplicate identical consecutive lines
            let line_hash = line.trim().to_string();
            if seen_lines.contains(&line_hash) {
                duplicate_count += 1;
                continue;
            }

            seen_lines.insert(line_hash);
            lines.push(line.to_string());
        }

        if duplicate_count > 0 {
            lines.push(format!("({} duplicate lines removed)", duplicate_count));
        }

        lines.join("\n")
    }

    /// Truncate output while preserving head and tail
    fn truncate(&self, input: &str) -> String {
        let lines: Vec<&str> = input.lines().collect();
        let total_lines = lines.len();

        // Check line limit
        if self.max_lines > 0 && total_lines > self.max_lines {
            let head_lines = self.max_lines / 3;
            let tail_lines = self.max_lines - head_lines;

            let mut result: Vec<String> = lines[..head_lines.min(total_lines)]
                .iter()
                .map(|s| s.to_string())
                .collect();
            result.push(String::new());
            let truncation_msg = format!(
                "--- {} lines truncated ({} total) ---",
                total_lines - head_lines - tail_lines,
                total_lines
            );
            result.push(truncation_msg);
            result.push(String::new());
            result.extend(
                lines[tail_lines.min(total_lines)..]
                    .iter()
                    .map(|s| s.to_string()),
            );
            return result.join("\n");
        }

        let result = input.to_string();

        // Check character limit
        if self.max_chars > 0 && result.len() > self.max_chars {
            let head_chars = self.max_chars * 40 / 100;
            let tail_chars = self.max_chars * 60 / 100;

            let head = &result[..head_chars.min(result.len())];
            let tail = &result[(result.len() - tail_chars).min(result.len() - 1)..];

            return format!(
                "{}\n\n--- {} characters truncated ({} total) ---\n\n{}",
                head,
                result.len() - head_chars - tail_chars,
                result.len(),
                tail
            );
        }

        result
    }
}

/// Token budget for execution
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Maximum tokens allowed
    pub max_tokens: u64,
    /// Current usage
    pub used_tokens: u64,
    /// Warning threshold (percentage)
    pub warning_threshold: f64,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            max_tokens: 100000, // 100K tokens default
            used_tokens: 0,
            warning_threshold: 0.8, // 80% threshold
        }
    }
}

impl TokenBudget {
    /// Create a new TokenBudget
    pub fn new(max_tokens: u64) -> Self {
        Self {
            max_tokens,
            used_tokens: 0,
            warning_threshold: 0.8,
        }
    }

    /// Check if adding more tokens would exceed budget
    pub fn can_add(&self, tokens: u64) -> bool {
        self.used_tokens + tokens <= self.max_tokens
    }

    /// Add tokens to usage
    pub fn add(&mut self, tokens: u64) {
        self.used_tokens += tokens;
    }

    /// Check if over warning threshold
    pub fn is_near_limit(&self) -> bool {
        self.used_tokens as f64 / self.max_tokens as f64 >= self.warning_threshold
    }

    /// Get remaining tokens
    pub fn remaining(&self) -> u64 {
        self.max_tokens.saturating_sub(self.used_tokens)
    }

    /// Get usage percentage
    pub fn usage_percent(&self) -> f64 {
        self.used_tokens as f64 / self.max_tokens as f64 * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_counter_english() {
        let counter = TokenCounter::default();
        let text = "Hello world, this is a test.";
        let tokens = counter.count_tokens(text);
        assert!(tokens > 0);
    }

    #[test]
    fn test_token_counter_chinese() {
        let counter = TokenCounter::default();
        let text = "你好世界，这是一个测试。";
        let tokens = counter.count_tokens(text);
        assert!(tokens > 0);
    }

    #[test]
    fn test_output_optimizer_filters_ansi() {
        let optimizer = OutputOptimizer::new(200, 10000, true);
        let input = "Hello\x1b[2J\x1b[HWorld";
        let result = optimizer.optimize(input);
        assert!(!result.contains('\x1b'));
    }

    #[test]
    fn test_output_optimizer_truncates() {
        let optimizer = OutputOptimizer::new(10, 0, false);
        let input =
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12";
        let result = optimizer.optimize(input);
        assert!(result.contains("truncated"));
    }

    #[test]
    fn test_token_budget() {
        let mut budget = TokenBudget::new(1000);
        assert!(budget.can_add(500));
        budget.add(500);
        assert!(budget.can_add(400));
        assert!(!budget.can_add(600));
        assert!(!budget.is_near_limit());
        budget.add(300);
        assert!(budget.is_near_limit());
    }
}
