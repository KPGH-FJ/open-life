use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Metrics collected during gradual rollout of experimental features.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutMetric {
    pub id: Option<i64>,
    pub experiment: String,
    pub version: String,
    pub timestamp: String,
    pub duration_ms: i64,
    pub success: bool,
    pub error: Option<String>,
    pub metadata: Option<String>,
}

/// Store for rollout metrics during gradual feature rollout.
pub struct RolloutMetricsStore {
    conn: Connection,
}

impl RolloutMetricsStore {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS rollout_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                experiment TEXT NOT NULL,
                version TEXT NOT NULL,
                timestamp TEXT DEFAULT CURRENT_TIMESTAMP,
                duration_ms INTEGER NOT NULL,
                success BOOLEAN NOT NULL,
                error TEXT,
                metadata TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_rollout_experiment ON rollout_metrics(experiment)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_rollout_timestamp ON rollout_metrics(timestamp)",
            [],
        )?;

        Ok(())
    }

    pub fn record_metric(&self, metric: &RolloutMetric) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO rollout_metrics (experiment, version, timestamp, duration_ms, success, error, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &metric.experiment,
                &metric.version,
                &metric.timestamp,
                metric.duration_ms,
                metric.success,
                metric.error.as_ref(),
                metric.metadata.as_ref(),
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_metrics(
        &self,
        experiment: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RolloutMetric>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, experiment, version, timestamp, duration_ms, success, error, metadata
             FROM rollout_metrics
             WHERE experiment = ?1
             ORDER BY timestamp DESC
             LIMIT ?2 OFFSET ?3"
        )?;

        let metrics = stmt
            .query_map(params![experiment, limit, offset], |row| {
                Ok(RolloutMetric {
                    id: row.get(0)?,
                    experiment: row.get(1)?,
                    version: row.get(2)?,
                    timestamp: row.get(3)?,
                    duration_ms: row.get(4)?,
                    success: row.get(5)?,
                    error: row.get(6)?,
                    metadata: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(metrics)
    }

    pub fn get_summary(&self, experiment: &str) -> Result<RolloutSummary> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                COUNT(*) as total,
                SUM(CASE WHEN version = 'v2' THEN 1 ELSE 0 END) as v2_count,
                SUM(CASE WHEN version = 'v1' THEN 1 ELSE 0 END) as v1_count,
                SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) as success_count,
                AVG(CASE WHEN version = 'v2' THEN duration_ms END) as v2_avg_duration,
                AVG(CASE WHEN version = 'v1' THEN duration_ms END) as v1_avg_duration
             FROM rollout_metrics
             WHERE experiment = ?1"
        )?;

        let summary = stmt.query_row([experiment], |row| {
            Ok(RolloutSummary {
                total: row.get(0)?,
                v2_count: row.get(1)?,
                v1_count: row.get(2)?,
                success_count: row.get(3)?,
                v2_avg_duration_ms: row.get(4)?,
                v1_avg_duration_ms: row.get(5)?,
            })
        })?;

        Ok(summary)
    }

    pub fn get_recent_errors(&self, experiment: &str, limit: i64) -> Result<Vec<RolloutMetric>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, experiment, version, timestamp, duration_ms, success, error, metadata
             FROM rollout_metrics
             WHERE experiment = ?1 AND success = 0
             ORDER BY timestamp DESC
             LIMIT ?2"
        )?;

        let metrics = stmt
            .query_map(params![experiment, limit], |row| {
                Ok(RolloutMetric {
                    id: row.get(0)?,
                    experiment: row.get(1)?,
                    version: row.get(2)?,
                    timestamp: row.get(3)?,
                    duration_ms: row.get(4)?,
                    success: row.get(5)?,
                    error: row.get(6)?,
                    metadata: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(metrics)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutSummary {
    pub total: i64,
    pub v2_count: i64,
    pub v1_count: i64,
    pub success_count: i64,
    pub v2_avg_duration_ms: Option<f64>,
    pub v1_avg_duration_ms: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_store() -> RolloutMetricsStore {
        RolloutMetricsStore::new(":memory:").unwrap()
    }

    #[test]
    fn test_record_and_list() {
        let store = create_test_store();
        
        let metric = RolloutMetric {
            id: None,
            experiment: "context_assembler".into(),
            version: "v2".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_ms: 45,
            success: true,
            error: None,
            metadata: Some(r#"{"memory_hits": 3}"#.into()),
        };

        let id = store.record_metric(&metric).unwrap();
        assert!(id > 0);

        let metrics = store.list_metrics("context_assembler", 10, 0).unwrap();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].version, "v2");
    }

    #[test]
    fn test_summary() {
        let store = create_test_store();
        
        // Record 3 v2, 2 v1
        for i in 0..3 {
            store.record_metric(&RolloutMetric {
                id: None,
                experiment: "test".into(),
                version: "v2".into(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: 40 + i,
                success: true,
                error: None,
                metadata: None,
            }).unwrap();
        }
        
        for i in 0..2 {
            store.record_metric(&RolloutMetric {
                id: None,
                experiment: "test".into(),
                version: "v1".into(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: 50 + i,
                success: true,
                error: None,
                metadata: None,
            }).unwrap();
        }

        let summary = store.get_summary("test").unwrap();
        assert_eq!(summary.total, 5);
        assert_eq!(summary.v2_count, 3);
        assert_eq!(summary.v1_count, 2);
        assert!(summary.v2_avg_duration_ms.unwrap() > 0.0);
    }
}
