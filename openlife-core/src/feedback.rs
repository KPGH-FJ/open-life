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
            &["analytics"],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_store_creates_only_the_current_audit_event_table() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("feedback.db");
        let _store = FeedbackStore::new(&path).unwrap();
        let connection = Connection::open(path).unwrap();
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        let names = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert!(names.contains(&"analytics".to_string()));
        assert!(!names.contains(&"feedback".to_string()));
        assert!(!names.contains(&"conversation_inferences".to_string()));
    }

    #[test]
    fn existing_legacy_tables_remain_inert_and_untouched() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("feedback.db");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE feedback (id INTEGER PRIMARY KEY, marker TEXT NOT NULL);
                     CREATE TABLE conversation_inferences (id INTEGER PRIMARY KEY, marker TEXT NOT NULL);
                     INSERT INTO feedback (id, marker) VALUES (1, 'legacy-feedback');
                     INSERT INTO conversation_inferences (id, marker) VALUES (1, 'legacy-inference');",
                )
                .unwrap();
        }

        let store = FeedbackStore::new(&path).unwrap();
        store.log_event("current-audit", None, None).unwrap();
        drop(store);

        let connection = Connection::open(path).unwrap();
        let feedback_marker: String = connection
            .query_row("SELECT marker FROM feedback WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        let inference_marker: String = connection
            .query_row(
                "SELECT marker FROM conversation_inferences WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let audit_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM analytics", [], |row| row.get(0))
            .unwrap();

        assert_eq!(feedback_marker, "legacy-feedback");
        assert_eq!(inference_marker, "legacy-inference");
        assert_eq!(audit_count, 1);
    }
}
