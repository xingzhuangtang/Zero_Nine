//! Structured memory store — three-level (Session/Task/Coding) persistent memory.
//!
//! Provides a `MemoryStore` trait and a `SqliteMemoryStore` implementation
//! backed by rusqlite + FTS5 full-text search.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use zn_types::{MemoryEntry, MemoryLevel, MemoryQuery, MemorySearchResult};

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Backend-agnostic interface for structured memory storage.
pub trait MemoryStore: Send + Sync {
    /// Persist a new memory entry (or update if `id` already exists).
    fn store(&self, entry: &MemoryEntry) -> Result<()>;

    /// Retrieve an entry by its unique ID.
    fn get(&self, id: &str) -> Result<Option<MemoryEntry>>;

    /// Full-text search with optional level and tag filters.
    fn search(&self, query: &MemoryQuery) -> Result<MemorySearchResult>;

    /// Delete an entry by ID. Returns true if an entry was deleted.
    fn delete(&self, id: &str) -> Result<bool>;

    /// Update an existing entry (replaces content + tags, bumps updated_at).
    fn update(&self, entry: &MemoryEntry) -> Result<()>;

    /// List all entries at a given memory level.
    fn list_by_level(&self, level: &MemoryLevel) -> Result<Vec<MemoryEntry>>;

    /// List all session-level entries for a specific session.
    fn list_by_session(&self, session_id: &str) -> Result<Vec<MemoryEntry>>;

    /// List all task-level entries for a specific task.
    fn list_by_task(&self, task_id: &str) -> Result<Vec<MemoryEntry>>;

    /// Garbage-collect old entries.
    ///
    /// Removes entries older than `max_age_days` that have been accessed
    /// fewer than `min_access_count` times. Returns the number of deleted rows.
    fn gc(&self, max_age_days: u32, min_access_count: u32) -> Result<usize>;
}

// ---------------------------------------------------------------------------
// SQLite implementation
// ---------------------------------------------------------------------------

/// SQLite + FTS5 backed memory store.
///
/// The connection is wrapped in a `Mutex` so this type is `Send + Sync`.
pub struct SqliteMemoryStore {
    conn: Mutex<Connection>,
}

impl SqliteMemoryStore {
    /// Open (or create) a memory store at `db_path`.
    pub fn new(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create memory db dir {:?}", parent))?;
        }
        let conn =
            Connection::open(db_path).with_context(|| format!("open memory db {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// In-memory store for tests.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;

            CREATE TABLE IF NOT EXISTS memories (
                id           TEXT PRIMARY KEY,
                level        TEXT NOT NULL,
                key          TEXT NOT NULL,
                content      TEXT NOT NULL,
                tags         TEXT NOT NULL DEFAULT '[]',
                session_id   TEXT,
                task_id      TEXT,
                relevance    REAL NOT NULL DEFAULT 0.0,
                access_count INTEGER NOT NULL DEFAULT 0,
                created_at   TEXT NOT NULL,
                updated_at   TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_mem_level      ON memories(level);
            CREATE INDEX IF NOT EXISTS idx_mem_session_id ON memories(session_id);
            CREATE INDEX IF NOT EXISTS idx_mem_task_id    ON memories(task_id);

            CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts
            USING fts5(key, content, tags, content='memories', content_rowid='rowid');

            CREATE TRIGGER IF NOT EXISTS memories_ai
            AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, key, content, tags)
                VALUES (new.rowid, new.key, new.content, new.tags);
            END;

            CREATE TRIGGER IF NOT EXISTS memories_ad
            AFTER DELETE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, key, content, tags)
                VALUES ('delete', old.rowid, old.key, old.content, old.tags);
            END;

            CREATE TRIGGER IF NOT EXISTS memories_au
            AFTER UPDATE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, key, content, tags)
                VALUES ('delete', old.rowid, old.key, old.content, old.tags);
                INSERT INTO memories_fts(rowid, key, content, tags)
                VALUES (new.rowid, new.key, new.content, new.tags);
            END;
            ",
        )?;
        Ok(())
    }

    fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<MemoryEntry> {
        let level_str: String = row.get(1)?;
        let level = match level_str.as_str() {
            "session" => MemoryLevel::Session,
            "task" => MemoryLevel::Task,
            _ => MemoryLevel::Coding,
        };
        let tags_json: String = row.get(4)?;
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        let created_str: String = row.get(9)?;
        let updated_str: String = row.get(10)?;
        Ok(MemoryEntry {
            id: row.get(0)?,
            level,
            key: row.get(2)?,
            content: row.get(3)?,
            tags,
            session_id: row.get(5)?,
            task_id: row.get(6)?,
            relevance_score: row.get::<_, f64>(7)? as f32,
            access_count: row.get::<_, i64>(8)? as u32,
            created_at: created_str.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: updated_str.parse().unwrap_or_else(|_| Utc::now()),
        })
    }
}

impl MemoryStore for SqliteMemoryStore {
    fn store(&self, entry: &MemoryEntry) -> Result<()> {
        let level = match entry.level {
            MemoryLevel::Session => "session",
            MemoryLevel::Task => "task",
            MemoryLevel::Coding => "coding",
        };
        let tags_json = serde_json::to_string(&entry.tags)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO memories
             (id, level, key, content, tags, session_id, task_id,
              relevance, access_count, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                entry.id,
                level,
                entry.key,
                entry.content,
                tags_json,
                entry.session_id,
                entry.task_id,
                entry.relevance_score as f64,
                entry.access_count as i64,
                entry.created_at.to_rfc3339(),
                entry.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<MemoryEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,level,key,content,tags,session_id,task_id,
                    relevance,access_count,created_at,updated_at
             FROM memories WHERE id=?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let entry = Self::row_to_entry(row)?;
            conn.execute(
                "UPDATE memories SET access_count=access_count+1, updated_at=?1 WHERE id=?2",
                params![Utc::now().to_rfc3339(), id],
            )?;
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    fn search(&self, query: &MemoryQuery) -> Result<MemorySearchResult> {
        let fts_query = query.query.replace('"', "\"\"");
        let level_strings: Vec<String> = query
            .levels
            .iter()
            .map(|l| match l {
                MemoryLevel::Session => "session".to_string(),
                MemoryLevel::Task => "task".to_string(),
                MemoryLevel::Coding => "coding".to_string(),
            })
            .collect();

        let level_filter = if level_strings.is_empty() {
            String::new()
        } else {
            let placeholders: Vec<String> = (2..=level_strings.len() + 1)
                .map(|i| format!("?{i}"))
                .collect();
            format!(" AND m.level IN ({})", placeholders.join(","))
        };

        let sql = format!(
            "SELECT m.id,m.level,m.key,m.content,m.tags,
                    m.session_id,m.task_id,m.relevance,
                    m.access_count,m.created_at,m.updated_at
             FROM memories m
             JOIN memories_fts fts ON m.rowid = fts.rowid
             WHERE memories_fts MATCH ?1{}
             ORDER BY rank
             LIMIT ?{}",
            level_filter,
            level_strings.len() + 2
        );

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql)?;

        // Build a uniform params list: [fts_query, levels..., limit]
        let mut dyn_params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(fts_query)];
        for lvl in &level_strings {
            dyn_params.push(Box::new(lvl.clone()));
        }
        dyn_params.push(Box::new(query.max_results as i64));
        let refs: Vec<&dyn rusqlite::ToSql> = dyn_params.iter().map(|b| b.as_ref()).collect();

        let entries: Vec<MemoryEntry> = stmt
            .query_map(refs.as_slice(), Self::row_to_entry)?
            .filter_map(|r| r.ok())
            .filter(|e| e.relevance_score >= query.min_relevance)
            .collect();

        let total = entries.len();
        Ok(MemorySearchResult { entries, total })
    }

    fn delete(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM memories WHERE id=?1", params![id])?;
        Ok(n > 0)
    }

    fn update(&self, entry: &MemoryEntry) -> Result<()> {
        let tags_json = serde_json::to_string(&entry.tags)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE memories SET key=?1,content=?2,tags=?3,updated_at=?4 WHERE id=?5",
            params![
                entry.key,
                entry.content,
                tags_json,
                Utc::now().to_rfc3339(),
                entry.id
            ],
        )?;
        Ok(())
    }

    fn list_by_level(&self, level: &MemoryLevel) -> Result<Vec<MemoryEntry>> {
        let level_str = match level {
            MemoryLevel::Session => "session",
            MemoryLevel::Task => "task",
            MemoryLevel::Coding => "coding",
        };
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,level,key,content,tags,session_id,task_id,
                    relevance,access_count,created_at,updated_at
             FROM memories WHERE level=?1 ORDER BY updated_at DESC",
        )?;
        let entries = stmt
            .query_map(params![level_str], Self::row_to_entry)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(entries)
    }

    fn list_by_session(&self, session_id: &str) -> Result<Vec<MemoryEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,level,key,content,tags,session_id,task_id,
                    relevance,access_count,created_at,updated_at
             FROM memories WHERE session_id=?1 ORDER BY updated_at DESC",
        )?;
        let entries = stmt
            .query_map(params![session_id], Self::row_to_entry)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(entries)
    }

    fn list_by_task(&self, task_id: &str) -> Result<Vec<MemoryEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,level,key,content,tags,session_id,task_id,
                    relevance,access_count,created_at,updated_at
             FROM memories WHERE task_id=?1 ORDER BY updated_at DESC",
        )?;
        let entries = stmt
            .query_map(params![task_id], Self::row_to_entry)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(entries)
    }

    fn gc(&self, max_age_days: u32, min_access_count: u32) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "DELETE FROM memories WHERE created_at < ?1 AND access_count < ?2",
            params![cutoff.to_rfc3339(), min_access_count as i64],
        )?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zn_types::MemoryEntry;

    fn store() -> SqliteMemoryStore {
        SqliteMemoryStore::in_memory().unwrap()
    }

    #[test]
    fn test_store_and_get() {
        let s = store();
        let e = MemoryEntry::new_task("key1", "remember this", "task-1");
        let id = e.id.clone();
        s.store(&e).unwrap();
        let found = s.get(&id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().key, "key1");
    }

    #[test]
    fn test_get_missing() {
        let s = store();
        assert!(s.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_delete() {
        let s = store();
        let e = MemoryEntry::new_coding("style", "snake_case", vec![]);
        let id = e.id.clone();
        s.store(&e).unwrap();
        assert!(s.delete(&id).unwrap());
        assert!(s.get(&id).unwrap().is_none());
        assert!(!s.delete(&id).unwrap());
    }

    #[test]
    fn test_list_by_level() {
        let s = store();
        s.store(&MemoryEntry::new_task("t1", "content", "task-1"))
            .unwrap();
        s.store(&MemoryEntry::new_task("t2", "content", "task-2"))
            .unwrap();
        s.store(&MemoryEntry::new_coding("c1", "content", vec![]))
            .unwrap();
        assert_eq!(s.list_by_level(&MemoryLevel::Task).unwrap().len(), 2);
        assert_eq!(s.list_by_level(&MemoryLevel::Coding).unwrap().len(), 1);
    }

    #[test]
    fn test_list_by_session() {
        let s = store();
        s.store(&MemoryEntry::new_session("k1", "c1", "sess-A"))
            .unwrap();
        s.store(&MemoryEntry::new_session("k2", "c2", "sess-A"))
            .unwrap();
        s.store(&MemoryEntry::new_session("k3", "c3", "sess-B"))
            .unwrap();
        assert_eq!(s.list_by_session("sess-A").unwrap().len(), 2);
        assert_eq!(s.list_by_session("sess-B").unwrap().len(), 1);
    }

    #[test]
    fn test_list_by_task() {
        let s = store();
        s.store(&MemoryEntry::new_task("k1", "c1", "task-X"))
            .unwrap();
        s.store(&MemoryEntry::new_task("k2", "c2", "task-X"))
            .unwrap();
        s.store(&MemoryEntry::new_task("k3", "c3", "task-Y"))
            .unwrap();
        assert_eq!(s.list_by_task("task-X").unwrap().len(), 2);
    }

    #[test]
    fn test_update() {
        let s = store();
        let mut e = MemoryEntry::new_coding("old_key", "old_content", vec![]);
        s.store(&e).unwrap();
        e.key = "new_key".to_string();
        e.content = "new_content".to_string();
        s.update(&e).unwrap();
        let found = s.get(&e.id).unwrap().unwrap();
        assert_eq!(found.key, "new_key");
        assert_eq!(found.content, "new_content");
    }

    #[test]
    fn test_gc_removes_old_entries() {
        let s = store();
        let id = uuid::Uuid::new_v4().to_string();
        let old_date = (Utc::now() - chrono::Duration::days(100)).to_rfc3339();
        {
            let conn = s.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO memories
                 (id,level,key,content,tags,relevance,access_count,created_at,updated_at)
                 VALUES (?1,'coding','old','old content','[]',0.0,0,?2,?2)",
                params![id, old_date],
            )
            .unwrap();
        }
        s.store(&MemoryEntry::new_coding("recent", "content", vec![]))
            .unwrap();
        assert_eq!(s.gc(30, 1).unwrap(), 1);
        assert!(s.get(&id).unwrap().is_none());
    }
}
