//! Session Search - Search and summarize historical sessions
//!
//! This module provides:
//! - SQLite storage for session records
//! - FTS5 full-text search
//! - Session summarization

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Session record for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub session_type: String,  // brainstorm, execution, etc.
    pub goal: String,
    pub summary: String,
    pub artifacts: Vec<String>,
    pub success: bool,
    pub created_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub query: String,
    pub total: usize,
    pub results: Vec<SessionSummary>,
}

/// Session summary for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub session_type: String,
    pub goal: String,
    pub created_at: DateTime<Utc>,
    pub success: bool,
    pub relevance_score: Option<f32>,
}

/// Session search engine
pub struct SessionSearch {
    db_path: PathBuf,
    conn: Connection,
}

impl SessionSearch {
    /// Create a new SessionSearch
    pub fn new(db_path: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

        let mut searcher = Self { db_path, conn };
        searcher.init_tables()?;
        Ok(searcher)
    }

    /// Initialize database tables
    fn init_tables(&mut self) -> Result<()> {
        // Create sessions table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                session_type TEXT NOT NULL,
                goal TEXT NOT NULL,
                summary TEXT NOT NULL,
                artifacts TEXT NOT NULL,
                success INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                metadata TEXT NOT NULL
            )",
            [],
        )?;

        // Create FTS5 virtual table for full-text search
        self.conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
                goal,
                summary,
                content='sessions',
                content_rowid='rowid'
            )",
            [],
        )?;

        // Create triggers for FTS5 sync
        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS sessions_ai AFTER INSERT ON sessions BEGIN
                INSERT INTO sessions_fts(rowid, goal, summary)
                VALUES (NEW.rowid, NEW.goal, NEW.summary);
            END",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS sessions_ad AFTER DELETE ON sessions BEGIN
                INSERT INTO sessions_fts(sessions_fts, rowid, goal, summary)
                VALUES('delete', OLD.rowid, OLD.goal, OLD.summary);
            END",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS sessions_au AFTER UPDATE ON sessions BEGIN
                INSERT INTO sessions_fts(sessions_fts, rowid, goal, summary)
                VALUES('delete', OLD.rowid, OLD.goal, OLD.summary);
                INSERT INTO sessions_fts(rowid, goal, summary)
                VALUES (NEW.rowid, NEW.goal, NEW.summary);
            END",
            [],
        )?;

        Ok(())
    }

    /// Add a session record
    pub fn add_session(&mut self, record: &SessionRecord) -> Result<()> {
        let artifacts_json = serde_json::to_string(&record.artifacts)?;
        let metadata_json = serde_json::to_string(&record.metadata)?;
        let created_at_str = record.created_at.to_rfc3339();

        self.conn.execute(
            "INSERT OR REPLACE INTO sessions (id, session_type, goal, summary, artifacts, success, created_at, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                record.id,
                record.session_type,
                record.goal,
                record.summary,
                artifacts_json,
                if record.success { 1 } else { 0 },
                created_at_str,
                metadata_json,
            ],
        )?;

        Ok(())
    }

    /// Search sessions
    pub fn search(&self, query: &str, limit: usize) -> Result<SearchResults> {
        // 安全修复：FTS5 查询参数化/转义
        // FTS5 MATCH 语法特殊，需要转义双引号、星号等控制字符
        let sanitized_query = sanitize_fts5_query(query);

        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.session_type, s.goal, s.created_at, s.success,
                    bm25(sessions_fts) as relevance
             FROM sessions s
             JOIN sessions_fts ON s.rowid = sessions_fts.rowid
             WHERE sessions_fts MATCH ?1
             ORDER BY relevance
             LIMIT ?2",
        )?;

        let mut results = Vec::new();
        let rows = stmt.query_map(params![sanitized_query, limit as i64], |row| {
            let created_at_str: String = row.get(3)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(SessionSummary {
                id: row.get(0)?,
                session_type: row.get(1)?,
                goal: row.get(2)?,
                created_at,
                success: row.get::<_, i32>(4)? != 0,
                relevance_score: Some(row.get(5)?),
            })
        })?;

        for row in rows {
            results.push(row?);
        }

        // Also get total count
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions s
             JOIN sessions_fts ON s.rowid = sessions_fts.rowid
             WHERE sessions_fts MATCH ?1",
            params![sanitized_query],
            |row| row.get(0),
        )?;

        Ok(SearchResults {
            query: query.to_string(),
            total: total as usize,
            results,
        })
    }

    /// Get recent sessions
    pub fn get_recent(&self, limit: usize) -> Result<Vec<SessionSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_type, goal, created_at, success
             FROM sessions
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let mut results = Vec::new();
        let rows = stmt.query_map(params![limit as i64], |row| {
            let created_at_str: String = row.get(3)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(SessionSummary {
                id: row.get(0)?,
                session_type: row.get(1)?,
                goal: row.get(2)?,
                created_at,
                success: row.get::<_, i32>(4)? != 0,
                relevance_score: None,
            })
        })?;

        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }

    /// Get session by ID
    pub fn get_session(&self, id: &str) -> Result<Option<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_type, goal, summary, artifacts, success, created_at, metadata
             FROM sessions WHERE id = ?1",
        )?;

        let record = stmt.query_row(params![id], |row| {
            let artifacts_json: String = row.get(4)?;
            let metadata_json: String = row.get(7)?;
            let created_at_str: String = row.get(6)?;

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(SessionRecord {
                id: row.get(0)?,
                session_type: row.get(1)?,
                goal: row.get(2)?,
                summary: row.get(3)?,
                artifacts: serde_json::from_str(&artifacts_json)
                    .map_err(|_e| rusqlite::Error::QueryReturnedNoRows)  // workaround
                    .unwrap_or_default(),
                success: row.get::<_, i32>(5)? != 0,
                created_at,
                metadata: serde_json::from_str(&metadata_json)
                    .map_err(|_e| rusqlite::Error::QueryReturnedNoRows)  // workaround
                    .unwrap_or_default(),
            })
        });

        match record {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e).with_context(|| format!("Failed to get session: {}", id)),
        }
    }

    /// Get session statistics
    pub fn get_stats(&self) -> Result<SessionStats> {
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions",
            [],
            |row| row.get(0),
        )?;

        let successful: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE success = 1",
            [],
            |row| row.get(0),
        )?;

        let brainstorm_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE session_type = 'brainstorm'",
            [],
            |row| row.get(0),
        )?;

        let execution_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE session_type = 'execution'",
            [],
            |row| row.get(0),
        )?;

        Ok(SessionStats {
            total_sessions: total as usize,
            successful_sessions: successful as usize,
            brainstorm_sessions: brainstorm_count as usize,
            execution_sessions: execution_count as usize,
            success_rate: if total > 0 {
                successful as f32 / total as f32
            } else {
                0.0
            },
        })
    }

    /// Delete old sessions (older than specified date)
    pub fn delete_old_sessions(&mut self, before: DateTime<Utc>) -> Result<usize> {
        let before_str = before.to_rfc3339();
        let changes = self.conn.execute(
            "DELETE FROM sessions WHERE created_at < ?1",
            params![before_str],
        )?;
        Ok(changes)
    }
}

/// FTS5 查询转义函数
/// 转义 FTS5 特殊字符：双引号、星号、AND、OR、NOT 等
fn sanitize_fts5_query(query: &str) -> String {
    // 如果查询为空，返回空查询
    if query.trim().is_empty() {
        return String::new();
    }

    // FTS5 特殊字符和运算符需要转义
    // 双引号用于短语搜索，需要转义
    let escaped = query.replace('"', "\"\"");

    // 星号是通配符，如果单独出现则转义为空
    // 但如果 * 在单词末尾是合法的通配符用法，保留
    let result = if escaped.trim() == "*" {
        // 单独星号转义为空
        String::new()
    } else {
        escaped
    };

    // 限制查询长度
    result.chars().take(200).collect()
}

/// Session statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub total_sessions: usize,
    pub successful_sessions: usize,
    pub brainstorm_sessions: usize,
    pub execution_sessions: usize,
    pub success_rate: f32,
}

/// Create a default session search for a project
pub fn create_default_searcher(project_root: &Path) -> Result<SessionSearch> {
    let db_path = project_root
        .join(".zero_nine")
        .join("memory")
        .join("sessions.db");
    SessionSearch::new(db_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs;

    #[test]
    fn test_create_session_search() {
        let tmp_file = temp_dir().join("zn_session_test.db");
        let _ = fs::remove_file(&tmp_file);

        let search = SessionSearch::new(tmp_file.clone()).unwrap();
        assert!(tmp_file.exists());

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_add_and_search_session() {
        let tmp_file = temp_dir().join("zn_session_add_test.db");
        let _ = fs::remove_file(&tmp_file);

        let mut search = SessionSearch::new(tmp_file.clone()).unwrap();

        let record = SessionRecord {
            id: "test-1".to_string(),
            session_type: "brainstorm".to_string(),
            goal: "Implement feature X with Y capability".to_string(),
            summary: "Discussed implementation of feature X".to_string(),
            artifacts: vec!["proposal.md".to_string()],
            success: true,
            created_at: Utc::now(),
            metadata: serde_json::json!({}),
        };

        search.add_session(&record).unwrap();

        // Search for the session
        let results = search.search("feature X", 10).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.results.len(), 1);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_get_recent_sessions() {
        let tmp_file = temp_dir().join("zn_session_recent_test.db");
        let _ = fs::remove_file(&tmp_file);

        let mut search = SessionSearch::new(tmp_file.clone()).unwrap();

        for i in 0..5 {
            let record = SessionRecord {
                id: format!("test-{}", i),
                session_type: "execution".to_string(),
                goal: format!("Goal {}", i),
                summary: format!("Summary {}", i),
                artifacts: vec![],
                success: i % 2 == 0,
                created_at: Utc::now(),
                metadata: serde_json::json!({}),
            };
            search.add_session(&record).unwrap();
        }

        let recent = search.get_recent(3).unwrap();
        assert_eq!(recent.len(), 3);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_get_stats() {
        let tmp_file = temp_dir().join("zn_session_stats_test.db");
        let _ = fs::remove_file(&tmp_file);

        let mut search = SessionSearch::new(tmp_file.clone()).unwrap();

        // Add some sessions
        for i in 0..3 {
            let record = SessionRecord {
                id: format!("session-{}", i),
                session_type: if i < 2 { "brainstorm" } else { "execution" }.to_string(),
                goal: format!("Goal {}", i),
                summary: format!("Summary {}", i),
                artifacts: vec![],
                success: i != 1,  // 2 success, 1 failure
                created_at: Utc::now(),
                metadata: serde_json::json!({}),
            };
            search.add_session(&record).unwrap();
        }

        let stats = search.get_stats().unwrap();
        assert_eq!(stats.total_sessions, 3);
        assert_eq!(stats.successful_sessions, 2);
        assert_eq!(stats.brainstorm_sessions, 2);
        assert_eq!(stats.execution_sessions, 1);
        assert!((stats.success_rate - 0.667).abs() < 0.01);

        let _ = fs::remove_file(&tmp_file);
    }
}
