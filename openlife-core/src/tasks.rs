use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub source_run_id: Option<String>,
}

impl ScheduledTask {
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        due_date: Option<String>,
        priority: impl Into<String>,
    ) -> Self {
        let priority: String = priority.into();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            description: description.into(),
            due_date,
            priority: if priority.is_empty() {
                "medium".into()
            } else {
                priority
            },
            status: "pending".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            source_run_id: None,
        }
    }
}

pub struct TaskStore {
    conn: Mutex<Connection>,
}

impl TaskStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open tasks db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory tasks db")?;
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
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                due_date TEXT,
                priority TEXT NOT NULL DEFAULT 'medium',
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                completed_at TEXT,
                source_run_id TEXT
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tasks_due_date ON tasks(due_date) WHERE status != 'completed' AND status != 'cancelled'",
            [],
        )?;
        Ok(())
    }

    pub fn create_task(&self, task: &ScheduledTask) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO tasks (id, title, description, due_date, priority, status, created_at, completed_at, source_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                task.id,
                task.title,
                task.description,
                task.due_date,
                task.priority,
                task.status,
                task.created_at,
                task.completed_at,
                task.source_run_id,
            ],
        )?;
        Ok(())
    }

    pub fn list_tasks(&self, status: Option<&str>) -> Result<Vec<ScheduledTask>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let query = if let Some(_s) = status {
            "SELECT id, title, description, due_date, priority, status, created_at, completed_at, source_run_id FROM tasks WHERE status = ?1 ORDER BY created_at DESC"
        } else {
            "SELECT id, title, description, due_date, priority, status, created_at, completed_at, source_run_id FROM tasks ORDER BY created_at DESC"
        };
        let mut stmt = conn.prepare(query)?;
        let rows = if let Some(s) = status {
            stmt.query_map(params![s], map_row)?
        } else {
            stmt.query_map([], map_row)?
        };
        let tasks: Result<Vec<_>> = rows.map(|r| r.map_err(anyhow::Error::from)).collect();
        tasks
    }

    pub fn complete_task(&self, id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE tasks SET status = 'completed', completed_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn cancel_task(&self, id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE tasks SET status = 'cancelled', completed_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduledTask> {
    Ok(ScheduledTask {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        due_date: row.get(3)?,
        priority: row.get(4)?,
        status: row.get(5)?,
        created_at: row.get(6)?,
        completed_at: row.get(7)?,
        source_run_id: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_roundtrip() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = ScheduledTask::new("测试任务", "描述内容", None, "high");
        store.create_task(&task).unwrap();
        let tasks = store.list_tasks(Some("pending")).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "测试任务");
        assert_eq!(tasks[0].priority, "high");
    }

    #[test]
    fn test_complete_task() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = ScheduledTask::new("可完成任务", "", None, "medium");
        let id = task.id.clone();
        store.create_task(&task).unwrap();
        store.complete_task(&id).unwrap();
        let completed = store.list_tasks(Some("completed")).unwrap();
        assert_eq!(completed.len(), 1);
    }

    #[test]
    fn test_default_priority_is_medium() {
        let task = ScheduledTask::new("默认优先级", "", None, "");
        assert_eq!(task.priority, "medium");
    }
}
