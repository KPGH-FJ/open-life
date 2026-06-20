use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::vectors::MemoryChunk;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

const FTS_QUERY_MAX_CHARS: usize = 256;
const FTS_QUERY_MAX_TOKENS: usize = 8;

pub struct MemoryStore {
    conn: Mutex<Connection>,
}

fn normalize_fts_query(query: &str) -> Option<String> {
    let limited = query
        .trim()
        .chars()
        .take(FTS_QUERY_MAX_CHARS)
        .collect::<String>();
    let tokens = limited
        .split_whitespace()
        .filter_map(|part| {
            let trimmed = part.trim_matches(|c: char| {
                !c.is_alphanumeric() && !('\u{4e00}'..='\u{9fff}').contains(&c)
            });
            if trimmed.is_empty() {
                None
            } else {
                Some(format!("\"{}\"", trimmed.replace('"', "\"\"")))
            }
        })
        .take(FTS_QUERY_MAX_TOKENS)
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" OR "))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: i64,
    pub session_id: String,
    pub content: String,
    pub content_type: String,
    pub source: String,
    pub role: Option<String>,
    pub created_at: String,
    pub importance_score: f32,
    pub access_count: i64,
    pub last_accessed_at: Option<String>,
    pub tags: Vec<String>,
    pub privacy_level: String,
    pub embedding_id: Option<i64>,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchHit {
    pub chunk: MemoryChunk,
    pub relevance_score: f32,
    pub source_tier: String,
}

impl MemoryStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open sqlite db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory sqlite db")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                embedding_id INTEGER
            )",
            [],
        )?;
        Self::ensure_column_exists(&conn, "messages", "embedding_id", "INTEGER")?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, created_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_embedding_id ON messages(embedding_id)",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                content TEXT NOT NULL,
                content_type TEXT NOT NULL,
                source TEXT NOT NULL,
                role TEXT,
                created_at TEXT NOT NULL,
                importance_score REAL NOT NULL DEFAULT 0.5,
                access_count INTEGER NOT NULL DEFAULT 0,
                last_accessed_at TEXT,
                tags_json TEXT NOT NULL DEFAULT '[]',
                privacy_level TEXT NOT NULL DEFAULT 'private',
                embedding_id INTEGER,
                archived INTEGER NOT NULL DEFAULT 0,
                archived_at TEXT,
                checksum TEXT NOT NULL
            )",
            [],
        )?;
        Self::ensure_column_exists(&conn, "memories", "embedding_id", "INTEGER")?;
        Self::ensure_column_exists(&conn, "memories", "archived", "INTEGER NOT NULL DEFAULT 0")?;
        Self::ensure_column_exists(&conn, "memories", "archived_at", "TEXT")?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_session_created ON memories(session_id, created_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_content_type ON memories(content_type)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_embedding_id ON memories(embedding_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_archived ON memories(archived)",
            [],
        )?;
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                content,
                tags,
                content='memories',
                content_rowid='id'
            )",
            [],
        )?;
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, content, tags) VALUES (new.id, new.content, new.tags_json);
            END",
            [],
        )?;
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content, tags) VALUES('delete', old.id, old.content, old.tags_json);
            END",
            [],
        )?;
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content, tags) VALUES('delete', old.id, old.content, old.tags_json);
                INSERT INTO memories_fts(rowid, content, tags) VALUES (new.id, new.content, new.tags_json);
            END",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                life_model_yaml TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_snapshots_session ON snapshots(session_id, created_at)",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chat_sessions (
                session_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_chat_sessions_updated ON chat_sessions(updated_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS state_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                dimension_name TEXT NOT NULL,
                value REAL NOT NULL,
                unit TEXT NOT NULL,
                recorded_at TEXT NOT NULL,
                note TEXT
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_state_history_dimension ON state_history(dimension_name, recorded_at DESC)",
            [],
        )?;
        Ok(())
    }

    fn ensure_column_exists(
        conn: &Connection,
        table_name: &str,
        column_name: &str,
        column_definition: &str,
    ) -> Result<()> {
        let pragma = format!("PRAGMA table_info({table_name})");
        let mut stmt = conn.prepare(&pragma)?;
        let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let has_column = columns
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|name| name == column_name);
        if !has_column {
            let sql =
                format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {column_definition}");
            conn.execute(&sql, [])?;
        }
        Ok(())
    }

    fn message_tags(msg: &ChatMessage) -> Vec<String> {
        vec![format!("role:{}", msg.role), "chat".into()]
    }

    fn checksum_for(content: &str, session_id: &str, created_at: &str) -> String {
        let data = format!("{}:{}:{}", session_id, created_at, content);
        let hash = digest(&SHA256, data.as_bytes());
        let bytes = hash.as_ref();
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            out.push_str(&format!("{:02x}", b));
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_memory_row(
        conn: &Connection,
        session_id: &str,
        content: &str,
        content_type: &str,
        source: &str,
        role: Option<&str>,
        created_at: &str,
        tags: &[String],
        privacy_level: &str,
        embedding_id: Option<i64>,
    ) -> Result<i64> {
        let checksum = Self::checksum_for(content, session_id, created_at);
        let tags_json = serde_json::to_string(tags)?;
        conn.execute(
            "INSERT INTO memories (
                session_id, content, content_type, source, role, created_at,
                importance_score, access_count, last_accessed_at, tags_json,
                privacy_level, embedding_id, archived, archived_at, checksum
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                session_id,
                content,
                content_type,
                source,
                role,
                created_at,
                0.5_f32,
                0_i64,
                Option::<String>::None,
                tags_json,
                privacy_level,
                embedding_id,
                0_i64,
                Option::<String>::None,
                checksum
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn save_message(&self, session_id: &str, msg: &ChatMessage) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let created_at = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO messages (session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, msg.role, msg.content, &created_at],
        )?;
        let _ = Self::insert_memory_row(
            &conn,
            session_id,
            &msg.content,
            "chat_message",
            &format!("chat:{}", msg.role),
            Some(&msg.role),
            &created_at,
            &Self::message_tags(msg),
            "private",
            None,
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn load_recent_messages(&self, session_id: &str, limit: usize) -> Result<Vec<ChatMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT role, content FROM messages WHERE session_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit as i64], |row| {
            Ok(ChatMessage {
                role: row.get(0)?,
                content: row.get(1)?,
            })
        })?;
        let mut messages: Vec<ChatMessage> = rows.collect::<Result<Vec<_>, _>>()?;
        messages.reverse();
        Ok(messages)
    }

    pub fn save_snapshot(&self, session_id: &str, model: &LifeModel) -> Result<i64> {
        let yaml = serde_yaml::to_string(model)?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO snapshots (session_id, life_model_yaml, created_at) VALUES (?1, ?2, ?3)",
            params![session_id, yaml, Utc::now().to_rfc3339()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn load_latest_snapshot(&self, session_id: &str) -> Result<Option<LifeModel>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT life_model_yaml FROM snapshots WHERE session_id = ?1 ORDER BY created_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![session_id])?;
        if let Some(row) = rows.next()? {
            let yaml: String = row.get(0)?;
            let model: LifeModel = serde_yaml::from_str(&yaml)?;
            Ok(Some(model))
        } else {
            Ok(None)
        }
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<(String, DateTime<Utc>)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT session_id, MAX(created_at) as last_at FROM messages GROUP BY session_id ORDER BY last_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let sid: String = row.get(0)?;
            let last_at: String = row.get(1)?;
            let dt = DateTime::parse_from_rfc3339(&last_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            Ok((sid, dt))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("failed to collect sessions")
    }

    pub fn export_all_messages(&self) -> Result<Vec<ExportedMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT session_id, role, content, created_at FROM messages ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ExportedMessage {
                session_id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("failed to export messages")
    }

    pub fn clear_all_messages(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute("DELETE FROM messages", [])?;
        conn.execute(
            "DELETE FROM memories WHERE content_type = 'chat_message'",
            [],
        )?;
        Ok(())
    }

    pub fn import_messages(&self, messages: &[ExportedMessage]) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        for msg in messages {
            let tags = Self::message_tags(&ChatMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            });
            tx.execute(
                "INSERT INTO messages (session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![msg.session_id, msg.role, msg.content, msg.created_at],
            )?;
            let _ = Self::insert_memory_row(
                &tx,
                &msg.session_id,
                &msg.content,
                "chat_message",
                &format!("chat:{}", msg.role),
                Some(&msg.role),
                &msg.created_at,
                &tags,
                "private",
                None,
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace_all_messages(&self, messages: &[ExportedMessage]) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM messages", [])?;
        tx.execute(
            "DELETE FROM memories WHERE content_type = 'chat_message'",
            [],
        )?;
        for msg in messages {
            let tags = Self::message_tags(&ChatMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            });
            tx.execute(
                "INSERT INTO messages (session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![msg.session_id, msg.role, msg.content, msg.created_at],
            )?;
            let _ = Self::insert_memory_row(
                &tx,
                &msg.session_id,
                &msg.content,
                "chat_message",
                &format!("chat:{}", msg.role),
                Some(&msg.role),
                &msg.created_at,
                &tags,
                "private",
                None,
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn create_chat_session(&self, session_id: &str, title: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO chat_sessions (session_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, title, &now, &now],
        )?;
        Ok(())
    }

    pub fn list_chat_sessions(&self, limit: usize) -> Result<Vec<ChatSession>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT session_id, title, created_at, updated_at FROM chat_sessions ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ChatSession {
                session_id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("failed to list chat sessions")
    }

    pub fn rename_chat_session(&self, session_id: &str, title: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE chat_sessions SET title = ?1, updated_at = ?2 WHERE session_id = ?3",
            params![title, &now, session_id],
        )?;
        Ok(())
    }

    pub fn delete_chat_session(&self, session_id: &str) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![session_id],
        )?;
        tx.execute(
            "DELETE FROM memories WHERE session_id = ?1",
            params![session_id],
        )?;
        tx.execute(
            "DELETE FROM snapshots WHERE session_id = ?1",
            params![session_id],
        )?;
        tx.execute(
            "DELETE FROM chat_sessions WHERE session_id = ?1",
            params![session_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn touch_chat_session(&self, session_id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let now = Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE chat_sessions SET updated_at = ?1 WHERE session_id = ?2",
            params![&now, session_id],
        )?;
        if updated == 0 {
            conn.execute(
                "INSERT INTO chat_sessions (session_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                params![session_id, "新会话", &now, &now],
            )?;
        }
        Ok(())
    }

    pub fn record_state_entry(
        &self,
        dimension_name: &str,
        value: f64,
        unit: &str,
        note: Option<&str>,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO state_history (dimension_name, value, unit, recorded_at, note) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![dimension_name, value, unit, Utc::now().to_rfc3339(), note],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_state_history(
        &self,
        dimension_name: &str,
        limit: usize,
    ) -> Result<Vec<StateHistoryEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, dimension_name, value, unit, recorded_at, note FROM state_history WHERE dimension_name = ?1 ORDER BY recorded_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![dimension_name, limit as i64], |row| {
            Ok(StateHistoryEntry {
                id: row.get(0)?,
                dimension_name: row.get(1)?,
                value: row.get(2)?,
                unit: row.get(3)?,
                recorded_at: row.get(4)?,
                note: row.get(5)?,
            })
        })?;
        let mut entries: Vec<StateHistoryEntry> = rows.collect::<Result<Vec<_>, _>>()?;
        entries.reverse();
        Ok(entries)
    }

    pub fn get_latest_state_entries(&self, limit: usize) -> Result<Vec<StateHistoryEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, dimension_name, value, unit, recorded_at, note FROM state_history ORDER BY recorded_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(StateHistoryEntry {
                id: row.get(0)?,
                dimension_name: row.get(1)?,
                value: row.get(2)?,
                unit: row.get(3)?,
                recorded_at: row.get(4)?,
                note: row.get(5)?,
            })
        })?;
        let mut entries: Vec<StateHistoryEntry> = rows.collect::<Result<Vec<_>, _>>()?;
        entries.reverse();
        Ok(entries)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_memory_record(
        &self,
        session_id: &str,
        content: &str,
        content_type: &str,
        source: &str,
        tags: &[String],
        privacy_level: &str,
        embedding_id: Option<i64>,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let created_at = Utc::now().to_rfc3339();
        Self::insert_memory_row(
            &conn,
            session_id,
            content,
            content_type,
            source,
            None,
            &created_at,
            tags,
            privacy_level,
            embedding_id,
        )
    }

    pub fn search_text_memories(
        &self,
        session_id: Option<&str>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchHit>> {
        if query.trim().is_empty() {
            return Ok(vec![]);
        }

        let Some(normalized_query) = normalize_fts_query(query) else {
            return Ok(vec![]);
        };

        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut results = if let Some(session_id) = session_id {
            let mut stmt = match conn.prepare(
                "SELECT m.id, m.session_id, m.content, m.source, m.created_at, m.access_count, m.last_accessed_at, bm25(memories_fts) AS rank
                 FROM memories_fts
                 JOIN memories m ON m.id = memories_fts.rowid
                 WHERE memories_fts MATCH ?1 AND m.session_id = ?2 AND m.archived = 0
                 ORDER BY rank ASC, m.created_at DESC
                 LIMIT ?3",
            ) {
                Ok(stmt) => stmt,
                Err(_) => return self.search_text_memories_fallback(&conn, session_id, query, limit),
            };
            let rows =
                stmt.query_map(params![normalized_query, session_id, limit as i64], |row| {
                    let rank: f32 = row.get(7)?;
                    Ok(Self::row_to_search_hit(row, rank, "fts"))
                })?;
            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            let mut stmt = match conn.prepare(
                "SELECT m.id, m.session_id, m.content, m.source, m.created_at, m.access_count, m.last_accessed_at, bm25(memories_fts) AS rank
                 FROM memories_fts
                 JOIN memories m ON m.id = memories_fts.rowid
                 WHERE memories_fts MATCH ?1 AND m.archived = 0
                 ORDER BY rank ASC, m.created_at DESC
                 LIMIT ?2",
            ) {
                Ok(stmt) => stmt,
                Err(_) => return self.search_text_memories_fallback(&conn, "", query, limit),
            };
            let rows = stmt.query_map(params![normalized_query, limit as i64], |row| {
                let rank: f32 = row.get(7)?;
                Ok(Self::row_to_search_hit(row, rank, "fts"))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        if results.is_empty() {
            results =
                self.search_text_memories_fallback(&conn, session_id.unwrap_or(""), query, limit)?;
        }

        let now = Utc::now().to_rfc3339();
        let ids: Vec<i64> = results.iter().map(|h| h.chunk.id).collect();
        if !ids.is_empty() {
            let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "UPDATE memories SET access_count = access_count + 1, last_accessed_at = ?1 WHERE archived = 0 AND id IN ({})",
                placeholders
            );
            let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&now];
            params_vec.extend(ids.iter().map(|id| id as &dyn rusqlite::ToSql));
            let _ = conn.execute(&sql, &*params_vec);
        }
        results.truncate(limit);
        Ok(results)
    }

    fn search_text_memories_fallback(
        &self,
        conn: &Connection,
        session_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchHit>> {
        let like_query = format!("%{}%", query.trim());
        let sql = if session_id.is_empty() {
            "SELECT id, session_id, content, source, created_at, access_count, last_accessed_at
             FROM memories
             WHERE archived = 0 AND content LIKE ?1
             ORDER BY created_at DESC
             LIMIT ?2"
        } else {
            "SELECT id, session_id, content, source, created_at, access_count, last_accessed_at
             FROM memories
             WHERE archived = 0 AND session_id = ?1 AND content LIKE ?2
             ORDER BY created_at DESC
             LIMIT ?3"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = if session_id.is_empty() {
            stmt.query_map(params![like_query, limit as i64], |row| {
                Ok(Self::row_to_search_hit_fallback(row, query, "keyword"))
            })?
            .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![session_id, like_query, limit as i64], |row| {
                Ok(Self::row_to_search_hit_fallback(row, query, "keyword"))
            })?
            .collect::<Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    fn row_to_search_hit(row: &rusqlite::Row, rank: f32, source_tier: &str) -> MemorySearchHit {
        MemorySearchHit {
            chunk: MemoryChunk {
                id: row.get(0).unwrap_or_default(),
                session_id: row.get(1).unwrap_or_default(),
                content: row.get(2).unwrap_or_default(),
                source: row.get(3).unwrap_or_else(|_| "memory".to_string()),
                created_at: row.get(4).unwrap_or_default(),
                tier: 2,
                access_count: row.get(5).unwrap_or_default(),
                last_accessed_at: row
                    .get::<_, Option<String>>(6)
                    .unwrap_or_default()
                    .unwrap_or_default(),
                importance_score: 0.0,
                archived: false,
                archived_at: None,
                summary: None,
            },
            relevance_score: 1.0 / (1.0 + rank.max(0.0)),
            source_tier: source_tier.to_string(),
        }
    }

    fn row_to_search_hit_fallback(
        row: &rusqlite::Row,
        query: &str,
        source_tier: &str,
    ) -> MemorySearchHit {
        let content: String = row.get(2).unwrap_or_default();
        let score = if content.contains(query) { 0.72 } else { 0.45 };
        MemorySearchHit {
            chunk: MemoryChunk {
                id: row.get(0).unwrap_or_default(),
                session_id: row.get(1).unwrap_or_default(),
                content,
                source: row.get(3).unwrap_or_else(|_| "memory".to_string()),
                created_at: row.get(4).unwrap_or_default(),
                tier: 2,
                access_count: row.get(5).unwrap_or_default(),
                last_accessed_at: row
                    .get::<_, Option<String>>(6)
                    .unwrap_or_default()
                    .unwrap_or_default(),
                importance_score: 0.0,
                archived: false,
                archived_at: None,
                summary: None,
            },
            relevance_score: score,
            source_tier: source_tier.to_string(),
        }
    }

    pub fn archive_lifecycle_memory_records(&self, memory_id: &str) -> Result<usize> {
        let lifecycle_source = format!("memory_lifecycle:{memory_id}");
        let tag_match = format!("%memory_id:{memory_id}%");
        let now = Utc::now().to_rfc3339();
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let archived = conn.execute(
            "UPDATE memories
             SET archived = 1, archived_at = ?1
             WHERE archived = 0 AND (source = ?2 OR tags_json LIKE ?3)",
            params![now, lifecycle_source, tag_match],
        )?;
        Ok(archived)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportedMessage {
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatSession {
    pub session_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StateHistoryEntry {
    pub id: i64,
    pub dimension_name: String,
    pub value: f64,
    pub unit: String,
    pub recorded_at: String,
    pub note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ChatMessage;
    use rusqlite::Connection;

    #[test]
    fn memory_store_save_and_load_message() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        let session_id = "s1";
        let msg = ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        };
        store.save_message(session_id, &msg).unwrap();
        let loaded = store.load_recent_messages(session_id, 10).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].role, "user");
        assert_eq!(loaded[0].content, "hello");
    }

    #[test]
    fn memory_store_save_and_load_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        let session_id = "s1";
        let model = LifeModel::default_model();
        store.save_snapshot(session_id, &model).unwrap();
        let loaded = store.load_latest_snapshot(session_id).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().metadata.version, model.metadata.version);
    }

    #[test]
    fn memory_store_chat_session_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        store.create_chat_session("sess1", "Test Session").unwrap();
        let sessions = store.list_chat_sessions(10).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "sess1");
        assert_eq!(sessions[0].title, "Test Session");

        store.rename_chat_session("sess1", "Renamed").unwrap();
        let sessions = store.list_chat_sessions(10).unwrap();
        assert_eq!(sessions[0].title, "Renamed");

        store.touch_chat_session("sess1").unwrap();
        store.delete_chat_session("sess1").unwrap();
        let sessions = store.list_chat_sessions(10).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn memory_store_state_history_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        store
            .record_state_entry("energy", 75.0, "%", Some("good"))
            .unwrap();
        store.record_state_entry("energy", 80.0, "%", None).unwrap();
        let history = store.get_state_history("energy", 10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].value, 75.0);
        assert_eq!(history[1].value, 80.0);

        let latest = store.get_latest_state_entries(5).unwrap();
        assert_eq!(latest.len(), 2);
    }

    #[test]
    fn fts_query_escape_handles_special_syntax_without_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        store
            .save_memory_record(
                "s1",
                "alpha quoted value with 中文 token",
                "note",
                "test",
                &[],
                "private",
                None,
            )
            .unwrap();

        let long_query = "alpha ".repeat(100);
        for query in [
            "\"quoted\"",
            "alpha OR NEAR * ()",
            "foo\"bar",
            long_query.as_str(),
        ] {
            store.search_text_memories(Some("s1"), query, 5).unwrap();
        }
    }

    #[test]
    fn fts_query_escape_preserves_normal_search() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        store
            .save_memory_record(
                "s1",
                "deep work 深度工作 planning",
                "note",
                "test",
                &[],
                "private",
                None,
            )
            .unwrap();

        let english = store.search_text_memories(Some("s1"), "deep", 5).unwrap();
        assert_eq!(english.len(), 1);

        let chinese = store
            .search_text_memories(Some("s1"), "深度工作", 5)
            .unwrap();
        assert_eq!(chinese.len(), 1);
    }

    #[test]
    fn memory_store_export_import_messages() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        let msg = ChatMessage {
            role: "user".into(),
            content: "hi".into(),
        };
        store.save_message("s1", &msg).unwrap();
        let exported = store.export_all_messages().unwrap();
        assert_eq!(exported.len(), 1);

        store.clear_all_messages().unwrap();
        let loaded = store.load_recent_messages("s1", 10).unwrap();
        assert!(loaded.is_empty());

        store.import_messages(&exported).unwrap();
        let loaded = store.load_recent_messages("s1", 10).unwrap();
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn memory_store_syncs_messages_into_memories() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        let msg = ChatMessage {
            role: "assistant".into(),
            content: "Rust 异步任务需要注意取消和所有权".into(),
        };
        store.save_message("s1", &msg).unwrap();
        let hits = store.search_text_memories(Some("s1"), "Rust", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].chunk.content.contains("Rust"));
    }

    #[test]
    fn memory_store_can_save_manual_memory_records() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        store
            .save_memory_record(
                "manual",
                "用户偏好在上午处理复杂任务",
                "note",
                "manual_index",
                &["preference".into(), "energy".into()],
                "private",
                None,
            )
            .unwrap();
        let hits = store.search_text_memories(None, "上午", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(matches!(hits[0].source_tier.as_str(), "fts" | "keyword"));
    }

    #[test]
    fn memory_store_migrates_legacy_messages_table_before_creating_embedding_index() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("legacy-memory.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
        drop(conn);

        let store = MemoryStore::new(&db_path).unwrap();
        let msg = ChatMessage {
            role: "user".into(),
            content: "legacy schema still loads".into(),
        };
        store.save_message("legacy", &msg).unwrap();
        let loaded = store.load_recent_messages("legacy", 10).unwrap();
        assert_eq!(loaded.len(), 1);

        let conn = Connection::open(&db_path).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(messages)").unwrap();
        let column_names = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(column_names.iter().any(|name| name == "embedding_id"));
    }
}
