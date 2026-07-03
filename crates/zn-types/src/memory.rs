//! Three-level memory system types.
//!
//! Memory is organized in three levels:
//! - **Session**: short-term, lives within a single execution session.
//! - **Task**: mid-term, scoped to a project's task history.
//! - **Coding**: long-term, cross-project patterns and user preferences.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The three levels of the memory hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLevel {
    /// Short-term: current session context. Cleared when session ends.
    Session,
    /// Mid-term: task-scoped knowledge, persisted per project.
    Task,
    /// Long-term: cross-project coding patterns and user preferences.
    Coding,
}

/// A single structured memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique entry ID (UUID).
    pub id: String,
    /// Memory level this entry belongs to.
    pub level: MemoryLevel,
    /// Short key / title for this memory (used in search).
    pub key: String,
    /// Full content of the memory.
    pub content: String,
    /// Searchable tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Session ID (for Session-level memories).
    #[serde(default)]
    pub session_id: Option<String>,
    /// Task ID (for Task-level memories).
    #[serde(default)]
    pub task_id: Option<String>,
    /// Relevance score 0.0-1.0 (set by search).
    #[serde(default)]
    pub relevance_score: f32,
    /// How many times this entry has been accessed.
    #[serde(default)]
    pub access_count: u32,
    #[serde(default = "chrono::Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "chrono::Utc::now")]
    pub updated_at: DateTime<Utc>,
}

impl MemoryEntry {
    /// Construct a new session-level memory entry.
    pub fn new_session(
        key: impl Into<String>,
        content: impl Into<String>,
        session_id: &str,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            level: MemoryLevel::Session,
            key: key.into(),
            content: content.into(),
            tags: vec![],
            session_id: Some(session_id.to_string()),
            task_id: None,
            relevance_score: 0.0,
            access_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Construct a new task-level memory entry.
    pub fn new_task(key: impl Into<String>, content: impl Into<String>, task_id: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            level: MemoryLevel::Task,
            key: key.into(),
            content: content.into(),
            tags: vec![],
            session_id: None,
            task_id: Some(task_id.to_string()),
            relevance_score: 0.0,
            access_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Construct a new coding-level (global) memory entry.
    pub fn new_coding(
        key: impl Into<String>,
        content: impl Into<String>,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            level: MemoryLevel::Coding,
            key: key.into(),
            content: content.into(),
            tags,
            session_id: None,
            task_id: None,
            relevance_score: 0.0,
            access_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// A query against the memory store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuery {
    /// Full-text search string.
    pub query: String,
    /// Which levels to search (empty = all levels).
    #[serde(default)]
    pub levels: Vec<MemoryLevel>,
    /// Filter by these tags (empty = no tag filter).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Maximum number of results to return.
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    /// Minimum relevance score for results (0.0 = no minimum).
    #[serde(default)]
    pub min_relevance: f32,
}

fn default_max_results() -> usize {
    10
}

impl MemoryQuery {
    /// Build a simple query with just a search string.
    pub fn simple(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            levels: vec![],
            tags: vec![],
            max_results: 10,
            min_relevance: 0.0,
        }
    }
}

/// The result of a memory search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    /// Matching entries, ordered by relevance (highest first).
    pub entries: Vec<MemoryEntry>,
    /// Total number of matches (before max_results truncation).
    pub total: usize,
}

/// Learning-focused memory entry with success/failure tracking.
///
/// Extends the base memory system to track which patterns lead to success
/// vs failure, enabling cross-proposal learning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningEntry {
    /// Base memory entry
    pub base: MemoryEntry,
    /// How many times this pattern led to success
    #[serde(default)]
    pub success_count: u32,
    /// How many times this pattern led to failure
    #[serde(default)]
    pub failure_count: u32,
    /// Associated error pattern IDs
    #[serde(default)]
    pub pattern_ids: Vec<String>,
    /// Applicable task kinds (e.g., "execution", "test", "verification")
    #[serde(default)]
    pub applicable_kinds: Vec<String>,
    /// Last applied timestamp
    #[serde(default)]
    pub last_applied: Option<DateTime<Utc>>,
}

impl LearningEntry {
    /// Success rate as a fraction.
    pub fn success_rate(&self) -> f32 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            0.5 // Unknown
        } else {
            self.success_count as f32 / total as f32
        }
    }

    /// Whether this pattern is considered reliable (>60% success, >2 data points).
    pub fn is_reliable(&self) -> bool {
        self.success_count + self.failure_count >= 3 && self.success_rate() > 0.6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_session_entry() {
        let e = MemoryEntry::new_session("key1", "some content", "sess-1");
        assert_eq!(e.level, MemoryLevel::Session);
        assert_eq!(e.session_id, Some("sess-1".to_string()));
        assert!(e.task_id.is_none());
    }

    #[test]
    fn test_new_task_entry() {
        let e = MemoryEntry::new_task("error pattern", "always check bounds", "task-42");
        assert_eq!(e.level, MemoryLevel::Task);
        assert_eq!(e.task_id, Some("task-42".to_string()));
    }

    #[test]
    fn test_new_coding_entry() {
        let e = MemoryEntry::new_coding("style", "use snake_case", vec!["rust".to_string()]);
        assert_eq!(e.level, MemoryLevel::Coding);
        assert_eq!(e.tags, vec!["rust".to_string()]);
    }

    #[test]
    fn test_memory_query_simple() {
        let q = MemoryQuery::simple("error handling");
        assert_eq!(q.query, "error handling");
        assert_eq!(q.max_results, 10);
        assert!(q.levels.is_empty());
    }

    #[test]
    fn test_memory_entry_roundtrip() {
        let e = MemoryEntry::new_coding("pref", "content", vec!["tag1".to_string()]);
        let json = serde_json::to_string(&e).unwrap();
        let restored: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.key, "pref");
        assert_eq!(restored.level, MemoryLevel::Coding);
    }

    #[test]
    fn test_memory_level_serialization() {
        assert_eq!(
            serde_json::to_string(&MemoryLevel::Session).unwrap(),
            "\"session\""
        );
        assert_eq!(
            serde_json::to_string(&MemoryLevel::Coding).unwrap(),
            "\"coding\""
        );
    }
}
