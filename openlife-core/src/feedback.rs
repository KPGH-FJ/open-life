use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

/// Narrow audit-event store retained for current LifeModel gateway and proposal
/// receipts. Historical feedback-evolution APIs no longer own product behavior.
pub struct FeedbackStore {
    conn: Mutex<Connection>,
}

impl FeedbackStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open feedback sqlite db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("failed to open in-memory feedback sqlite db")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn open_read_only_existing(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = crate::sqlite_migration::open_existing_read_only(
            &db_path,
            "feedback_store",
            &["feedback", "analytics", "conversation_inferences"],
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn unavailable_sentinel() -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(crate::sqlite_migration::unavailable_read_only_sentinel(
                "feedback_store",
            )?),
        })
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        // Keep legacy tables readable until 5.5E has classified real on-disk data.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                message_index INTEGER NOT NULL,
                feedback_type TEXT NOT NULL,
                content_preview TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS analytics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_name TEXT NOT NULL,
                session_id TEXT,
                detail TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS conversation_inferences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                dimension TEXT NOT NULL,
                target_name TEXT NOT NULL,
                suggested_delta REAL,
                confidence REAL,
                reason TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_feedback_session ON feedback(session_id, created_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_inference_created ON conversation_inferences(created_at)",
            [],
        )?;
        Ok(())
    }

    pub fn log_event(
        &self,
        event_name: &str,
        session_id: Option<&str>,
        detail: Option<&str>,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO analytics (event_name, session_id, detail, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![event_name, session_id, detail, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn count_event_today(&self, event_name: &str) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM analytics WHERE event_name = ?1 AND DATE(created_at, 'localtime') = ?2",
                params![event_name, today],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count)
    }

    pub fn analytics_details_for_event(&self, event_name: &str, limit: i64) -> Result<Vec<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT detail FROM analytics WHERE event_name = ?1 ORDER BY created_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![event_name, limit.max(1)], |row| {
            row.get::<_, Option<String>>(0)
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map(|items| items.into_iter().flatten().collect())
            .map_err(|e| e.into())
    }
}
