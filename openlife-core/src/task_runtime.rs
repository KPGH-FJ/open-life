//! Canonical Task/Item/Artifact metadata for the general Agent runtime.
//!
//! This store owns stable Task identity, Run membership, typed Items, and
//! Artifact versions. Reports use the full Artifact lifecycle; ordinary plans
//! use Instruction + Plan Items without a parallel PlanExecute session.
//! Existing AgentRun persistence remains the execution/receipt owner while the
//! migration proceeds; this module does not copy AgentRun status or bodies.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

const TASK_RUNTIME_SCHEMA_VERSION: i64 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalTaskStatus {
    Running,
    WaitingReview,
    Completed,
    Blocked,
    Failed,
    EffectUnknown,
}

impl CanonicalTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::WaitingReview => "waiting_review",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::EffectUnknown => "effect_unknown",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "running" => Ok(Self::Running),
            "waiting_review" => Ok(Self::WaitingReview),
            "completed" => Ok(Self::Completed),
            "blocked" => Ok(Self::Blocked),
            "failed" => Ok(Self::Failed),
            "effect_unknown" => Ok(Self::EffectUnknown),
            _ => anyhow::bail!("canonical_task_status_invalid:{value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalTaskItemKind {
    Instruction,
    Plan,
    Steering,
    ToolCall,
    Observation,
    ProviderGeneration,
    ArtifactDraft,
    ReviewCheckpoint,
    ArtifactMaterialized,
    Verification,
    FinalResult,
}

impl CanonicalTaskItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Instruction => "instruction",
            Self::Plan => "plan",
            Self::Steering => "steering",
            Self::ToolCall => "tool_call",
            Self::Observation => "observation",
            Self::ProviderGeneration => "provider_generation",
            Self::ArtifactDraft => "artifact_draft",
            Self::ReviewCheckpoint => "review_checkpoint",
            Self::ArtifactMaterialized => "artifact_materialized",
            Self::Verification => "verification",
            Self::FinalResult => "final_result",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "instruction" => Ok(Self::Instruction),
            "plan" => Ok(Self::Plan),
            "steering" => Ok(Self::Steering),
            "tool_call" => Ok(Self::ToolCall),
            "observation" => Ok(Self::Observation),
            "provider_generation" => Ok(Self::ProviderGeneration),
            "artifact_draft" => Ok(Self::ArtifactDraft),
            "review_checkpoint" => Ok(Self::ReviewCheckpoint),
            "artifact_materialized" => Ok(Self::ArtifactMaterialized),
            "verification" => Ok(Self::Verification),
            "final_result" => Ok(Self::FinalResult),
            _ => anyhow::bail!("canonical_task_item_kind_invalid:{value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalTaskItemStatus {
    Waiting,
    Completed,
    Blocked,
    Failed,
    EffectUnknown,
}

impl CanonicalTaskItemStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::EffectUnknown => "effect_unknown",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "waiting" => Ok(Self::Waiting),
            "completed" => Ok(Self::Completed),
            "blocked" => Ok(Self::Blocked),
            "failed" => Ok(Self::Failed),
            "effect_unknown" => Ok(Self::EffectUnknown),
            _ => anyhow::bail!("canonical_task_item_status_invalid:{value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalArtifactStatus {
    Draft,
    WaitingReview,
    Materialized,
    Failed,
    EffectUnknown,
}

impl CanonicalArtifactStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::WaitingReview => "waiting_review",
            Self::Materialized => "materialized",
            Self::Failed => "failed",
            Self::EffectUnknown => "effect_unknown",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "draft" => Ok(Self::Draft),
            "waiting_review" => Ok(Self::WaitingReview),
            "materialized" => Ok(Self::Materialized),
            "failed" => Ok(Self::Failed),
            "effect_unknown" => Ok(Self::EffectUnknown),
            _ => anyhow::bail!("canonical_artifact_status_invalid:{value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalTaskRecord {
    pub id: String,
    pub conversation_id: String,
    pub task_kind: String,
    pub initial_outcome_digest: String,
    pub status: CanonicalTaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalTaskItemRecord {
    pub id: String,
    pub task_id: String,
    pub run_id: String,
    pub sequence: u64,
    pub kind: CanonicalTaskItemKind,
    pub status: CanonicalTaskItemStatus,
    pub summary_code: String,
    pub payload_digest: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalArtifactRecord {
    pub id: String,
    pub task_id: String,
    pub source_item_id: String,
    pub current_version: u64,
    pub status: CanonicalArtifactStatus,
    pub media_type: String,
    pub target_reference_digest: String,
    pub content_digest: String,
    pub proposal_id: Option<String>,
    pub materialized_reference: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalArtifactVersionRecord {
    pub artifact_id: String,
    pub version: u64,
    pub source_item_id: String,
    pub content_digest: String,
    pub materialized_reference: Option<String>,
    pub observed_content_digest: Option<String>,
    pub created_at: DateTime<Utc>,
    pub materialized_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalTaskRunRecord {
    pub task_id: String,
    pub run_id: String,
    pub execution_session_id: String,
    pub ordinal: u64,
    pub execution_facts_version: u64,
    pub plan_revision: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalSteeringStatus {
    Pending,
    Consumed,
    Blocked,
}

impl CanonicalSteeringStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Consumed => "consumed",
            Self::Blocked => "blocked",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "consumed" => Ok(Self::Consumed),
            "blocked" => Ok(Self::Blocked),
            _ => anyhow::bail!("canonical_steering_status_invalid:{value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalReportSteeringRecord {
    pub steering_id: String,
    pub item_id: String,
    pub task_id: String,
    pub run_id: String,
    pub source_message_ref: String,
    pub source_message_digest: String,
    pub steering_digest: String,
    pub base_plan_revision: u64,
    pub status: CanonicalSteeringStatus,
    pub created_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalArtifactSnapshot {
    pub artifact: CanonicalArtifactRecord,
    pub current_version: CanonicalArtifactVersionRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalTaskSnapshot {
    pub task: CanonicalTaskRecord,
    pub runs: Vec<CanonicalTaskRunRecord>,
    pub items: Vec<CanonicalTaskItemRecord>,
    pub artifacts: Vec<CanonicalArtifactSnapshot>,
}

pub struct ReportArtifactDraftInput<'a> {
    pub conversation_id: &'a str,
    pub execution_session_id: &'a str,
    pub run_id: &'a str,
    pub outcome_digest: &'a str,
    pub plan_digest: &'a str,
    pub provider_request_id: &'a str,
    pub provider_receipt_digest: &'a str,
    pub tool_observations: &'a [ReportToolObservationFactInput<'a>],
    pub target_reference: &'a str,
    pub content_digest: &'a str,
    pub media_type: &'a str,
}

pub struct BeginReportRunInput<'a> {
    pub conversation_id: &'a str,
    pub execution_session_id: &'a str,
    pub run_id: &'a str,
    pub outcome_digest: &'a str,
    pub plan_digest: &'a str,
}

pub struct BeginPlanRunInput<'a> {
    pub conversation_id: &'a str,
    pub execution_session_id: &'a str,
    pub run_id: &'a str,
    pub instruction_digest: &'a str,
    pub plan_digest: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BegunReportRun {
    pub task_id: String,
    pub run_id: String,
    pub plan_revision: u64,
}

pub struct SubmitReportSteeringInput<'a> {
    pub steering_id: &'a str,
    pub task_id: &'a str,
    pub run_id: &'a str,
    pub source_message_ref: &'a str,
    pub source_message_digest: &'a str,
    pub steering_digest: &'a str,
    pub base_plan_revision: u64,
    pub scope_expansion_blocked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportToolObservationFactInput<'a> {
    pub action_id: &'a str,
    pub tool_call_digest: &'a str,
    pub observation_id: &'a str,
    pub observation_digest: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedReportArtifact {
    pub task_id: String,
    pub artifact_draft_item_id: String,
    pub artifact_id: String,
    pub version: u64,
}

pub fn report_verification_item_id(
    artifact_id: &str,
    version: u64,
    observed_content_digest: &str,
) -> String {
    stable_id(
        "item",
        &[
            "verification",
            artifact_id,
            &version.to_string(),
            observed_content_digest,
        ],
    )
}

pub fn report_final_result_item_id(task_id: &str, run_id: &str) -> String {
    stable_id("item", &["final_result", task_id, run_id])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundReportReview {
    pub task_id: String,
    pub artifact_id: String,
    pub checkpoint_item_id: String,
    pub proposal_id: String,
}

#[derive(Clone)]
pub struct CanonicalTaskRuntimeStore {
    conn: Arc<Mutex<Connection>>,
    db_path: Option<PathBuf>,
}

impl CanonicalTaskRuntimeStore {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: Some(path),
        };
        store.initialize()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let store = Self {
            conn: Arc::new(Mutex::new(Connection::open_in_memory()?)),
            db_path: None,
        };
        store.initialize()?;
        Ok(store)
    }

    pub fn open_read_only_existing(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let conn = crate::sqlite_migration::open_existing_read_only(
            path,
            "canonical_task_runtime_store",
            &[
                "canonical_tasks",
                "canonical_task_runs",
                "canonical_task_items",
                "canonical_report_steering",
                "canonical_artifacts",
                "canonical_artifact_versions",
            ],
        )?;
        Self::validate_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: Some(path.to_path_buf()),
        })
    }

    pub fn db_path(&self) -> Option<&Path> {
        self.db_path.as_deref()
    }

    fn lock_conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|error| anyhow::anyhow!("canonical_task_runtime_mutex_poison:{error}"))
    }

    fn initialize(&self) -> Result<()> {
        let mut conn = self.lock_conn()?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS canonical_task_runtime_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS canonical_tasks (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL UNIQUE,
                task_kind TEXT NOT NULL CHECK(task_kind IN ('report', 'plan')),
                initial_outcome_digest TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN (
                    'running', 'waiting_review', 'completed', 'blocked',
                    'failed', 'effect_unknown'
                )),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS canonical_task_runs (
                task_id TEXT NOT NULL,
                run_id TEXT NOT NULL UNIQUE,
                execution_session_id TEXT NOT NULL,
                ordinal INTEGER NOT NULL CHECK(ordinal > 0),
                execution_facts_version INTEGER NOT NULL DEFAULT 5
                    CHECK(execution_facts_version IN (1, 2, 3, 4, 5)),
                plan_revision INTEGER NOT NULL DEFAULT 1 CHECK(plan_revision > 0),
                created_at TEXT NOT NULL,
                PRIMARY KEY(task_id, run_id),
                FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS canonical_task_items (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                sequence INTEGER NOT NULL CHECK(sequence > 0),
                kind TEXT NOT NULL CHECK(kind IN (
                    'instruction', 'plan', 'steering', 'tool_call', 'observation',
                    'provider_generation', 'artifact_draft',
                    'review_checkpoint', 'artifact_materialized',
                    'verification', 'final_result'
                )),
                status TEXT NOT NULL CHECK(status IN (
                    'waiting', 'completed', 'blocked', 'failed', 'effect_unknown'
                )),
                summary_code TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(task_id, sequence),
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             );
             CREATE TABLE IF NOT EXISTS canonical_report_steering (
                steering_id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL UNIQUE,
                task_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                source_message_ref TEXT NOT NULL UNIQUE,
                source_message_digest TEXT NOT NULL,
                steering_digest TEXT NOT NULL,
                base_plan_revision INTEGER NOT NULL CHECK(base_plan_revision > 0),
                status TEXT NOT NULL CHECK(status IN ('pending', 'consumed', 'blocked')),
                created_at TEXT NOT NULL,
                consumed_at TEXT,
                FOREIGN KEY(item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT,
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             );
             CREATE TABLE IF NOT EXISTS canonical_artifacts (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                source_item_id TEXT NOT NULL UNIQUE,
                current_version INTEGER NOT NULL CHECK(current_version > 0),
                status TEXT NOT NULL CHECK(status IN (
                    'draft', 'waiting_review', 'materialized', 'failed', 'effect_unknown'
                )),
                media_type TEXT NOT NULL,
                target_reference_digest TEXT NOT NULL,
                content_digest TEXT NOT NULL,
                proposal_id TEXT UNIQUE,
                materialized_reference TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT,
                FOREIGN KEY(source_item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
             );
             CREATE TABLE IF NOT EXISTS canonical_artifact_versions (
                artifact_id TEXT NOT NULL,
                version INTEGER NOT NULL CHECK(version > 0),
                source_item_id TEXT NOT NULL,
                content_digest TEXT NOT NULL,
                materialized_reference TEXT,
                observed_content_digest TEXT,
                created_at TEXT NOT NULL,
                materialized_at TEXT,
                PRIMARY KEY(artifact_id, version),
                FOREIGN KEY(artifact_id) REFERENCES canonical_artifacts(id) ON DELETE RESTRICT,
                FOREIGN KEY(source_item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_canonical_task_items_run
                ON canonical_task_items(run_id, sequence);
             CREATE INDEX IF NOT EXISTS idx_canonical_artifacts_task
                ON canonical_artifacts(task_id, created_at, id);
             INSERT INTO canonical_task_runtime_metadata(key, value)
             VALUES ('schema_version', '6')
             ON CONFLICT(key) DO NOTHING;",
        )?;
        if Self::schema_version(&conn)? == 1 {
            Self::migrate_v1_to_v2(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 2 {
            Self::migrate_v2_to_v3(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 3 {
            Self::migrate_v3_to_v4(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 4 {
            Self::migrate_v4_to_v5(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 5 {
            Self::migrate_v5_to_v6(&mut conn)?;
        }
        Self::validate_schema(&conn)
    }

    fn schema_version(conn: &Connection) -> Result<i64> {
        conn.query_row(
            "SELECT value FROM canonical_task_runtime_metadata
             WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("canonical_task_runtime_schema_version_missing"))?
        .parse::<i64>()
        .context("canonical_task_runtime_schema_version_invalid")
    }

    fn migrate_v1_to_v2(conn: &mut Connection) -> Result<()> {
        conn.execute_batch("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = ON;")?;
        let migration = (|| -> Result<()> {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(
                "ALTER TABLE canonical_task_runs
                     ADD COLUMN execution_facts_version INTEGER NOT NULL DEFAULT 1
                     CHECK(execution_facts_version IN (1, 2));
                 ALTER TABLE canonical_task_items RENAME TO canonical_task_items_v1;
                 CREATE TABLE canonical_task_items (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL CHECK(sequence > 0),
                    kind TEXT NOT NULL CHECK(kind IN (
                        'instruction', 'provider_generation', 'artifact_draft',
                        'review_checkpoint', 'artifact_materialized'
                    )),
                    status TEXT NOT NULL CHECK(status IN (
                        'waiting', 'completed', 'blocked', 'failed', 'effect_unknown'
                    )),
                    summary_code TEXT NOT NULL,
                    payload_digest TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    UNIQUE(task_id, sequence),
                    FOREIGN KEY(task_id, run_id)
                        REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
                 );
                 INSERT INTO canonical_task_items (
                    id, task_id, run_id, sequence, kind, status, summary_code,
                    payload_digest, created_at, updated_at
                 ) SELECT id, task_id, run_id, sequence, kind, status, summary_code,
                          payload_digest, created_at, updated_at
                   FROM canonical_task_items_v1;
                 DROP TABLE canonical_task_items_v1;
                 CREATE INDEX idx_canonical_task_items_run
                    ON canonical_task_items(run_id, sequence);",
            )?;
            let changed = tx.execute(
                "UPDATE canonical_task_runtime_metadata SET value = '2'
                 WHERE key = 'schema_version' AND value = '1'",
                [],
            )?;
            if changed != 1 {
                anyhow::bail!("canonical_task_runtime_v1_migration_version_conflict");
            }
            tx.commit()?;
            Ok(())
        })();
        let pragma_restore =
            conn.execute_batch("PRAGMA legacy_alter_table = OFF; PRAGMA foreign_keys = ON;");
        migration?;
        pragma_restore?;
        let violation = conn
            .prepare("PRAGMA foreign_key_check")?
            .query_row([], |_| Ok(()))
            .optional()?;
        if violation.is_some() {
            anyhow::bail!("canonical_task_runtime_v1_migration_foreign_key_violation");
        }
        Ok(())
    }

    fn migrate_v2_to_v3(conn: &mut Connection) -> Result<()> {
        conn.execute_batch("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = ON;")?;
        let migration = (|| -> Result<()> {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(
                "ALTER TABLE canonical_task_items RENAME TO canonical_task_items_v2;
                 ALTER TABLE canonical_task_runs RENAME TO canonical_task_runs_v2;
                 CREATE TABLE canonical_task_runs (
                    task_id TEXT NOT NULL,
                    run_id TEXT NOT NULL UNIQUE,
                    execution_session_id TEXT NOT NULL,
                    ordinal INTEGER NOT NULL CHECK(ordinal > 0),
                    execution_facts_version INTEGER NOT NULL DEFAULT 3
                        CHECK(execution_facts_version IN (1, 2, 3)),
                    created_at TEXT NOT NULL,
                    PRIMARY KEY(task_id, run_id),
                    FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT
                 ) WITHOUT ROWID;
                 INSERT INTO canonical_task_runs (
                    task_id, run_id, execution_session_id, ordinal,
                    execution_facts_version, created_at
                 ) SELECT task_id, run_id, execution_session_id, ordinal,
                          execution_facts_version, created_at
                   FROM canonical_task_runs_v2;
                 CREATE TABLE canonical_task_items (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL CHECK(sequence > 0),
                    kind TEXT NOT NULL CHECK(kind IN (
                        'instruction', 'tool_call', 'observation',
                        'provider_generation', 'artifact_draft',
                        'review_checkpoint', 'artifact_materialized'
                    )),
                    status TEXT NOT NULL CHECK(status IN (
                        'waiting', 'completed', 'blocked', 'failed', 'effect_unknown'
                    )),
                    summary_code TEXT NOT NULL,
                    payload_digest TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    UNIQUE(task_id, sequence),
                    FOREIGN KEY(task_id, run_id)
                        REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
                 );
                 INSERT INTO canonical_task_items (
                    id, task_id, run_id, sequence, kind, status, summary_code,
                    payload_digest, created_at, updated_at
                 ) SELECT id, task_id, run_id, sequence, kind, status, summary_code,
                          payload_digest, created_at, updated_at
                   FROM canonical_task_items_v2;
                 DROP TABLE canonical_task_items_v2;
                 DROP TABLE canonical_task_runs_v2;
                 CREATE INDEX idx_canonical_task_items_run
                    ON canonical_task_items(run_id, sequence);",
            )?;
            let changed = tx.execute(
                "UPDATE canonical_task_runtime_metadata SET value = '3'
                 WHERE key = 'schema_version' AND value = '2'",
                [],
            )?;
            if changed != 1 {
                anyhow::bail!("canonical_task_runtime_v2_migration_version_conflict");
            }
            tx.commit()?;
            Ok(())
        })();
        let pragma_restore =
            conn.execute_batch("PRAGMA legacy_alter_table = OFF; PRAGMA foreign_keys = ON;");
        migration?;
        pragma_restore?;
        let violation = conn
            .prepare("PRAGMA foreign_key_check")?
            .query_row([], |_| Ok(()))
            .optional()?;
        if violation.is_some() {
            anyhow::bail!("canonical_task_runtime_v2_migration_foreign_key_violation");
        }
        Ok(())
    }

    fn migrate_v3_to_v4(conn: &mut Connection) -> Result<()> {
        conn.execute_batch("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = ON;")?;
        let migration = (|| -> Result<()> {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(
                "ALTER TABLE canonical_task_items RENAME TO canonical_task_items_v3;
                 ALTER TABLE canonical_task_runs RENAME TO canonical_task_runs_v3;
                 CREATE TABLE canonical_task_runs (
                    task_id TEXT NOT NULL,
                    run_id TEXT NOT NULL UNIQUE,
                    execution_session_id TEXT NOT NULL,
                    ordinal INTEGER NOT NULL CHECK(ordinal > 0),
                    execution_facts_version INTEGER NOT NULL DEFAULT 4
                        CHECK(execution_facts_version IN (1, 2, 3, 4)),
                    created_at TEXT NOT NULL,
                    PRIMARY KEY(task_id, run_id),
                    FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT
                 ) WITHOUT ROWID;
                 INSERT INTO canonical_task_runs (
                    task_id, run_id, execution_session_id, ordinal,
                    execution_facts_version, created_at
                 ) SELECT task_id, run_id, execution_session_id, ordinal,
                          execution_facts_version, created_at
                   FROM canonical_task_runs_v3;
                 CREATE TABLE canonical_task_items (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL CHECK(sequence > 0),
                    kind TEXT NOT NULL CHECK(kind IN (
                        'instruction', 'plan', 'tool_call', 'observation',
                        'provider_generation', 'artifact_draft',
                        'review_checkpoint', 'artifact_materialized',
                        'verification', 'final_result'
                    )),
                    status TEXT NOT NULL CHECK(status IN (
                        'waiting', 'completed', 'blocked', 'failed', 'effect_unknown'
                    )),
                    summary_code TEXT NOT NULL,
                    payload_digest TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    UNIQUE(task_id, sequence),
                    FOREIGN KEY(task_id, run_id)
                        REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
                 );
                 INSERT INTO canonical_task_items (
                    id, task_id, run_id, sequence, kind, status, summary_code,
                    payload_digest, created_at, updated_at
                 ) SELECT id, task_id, run_id, sequence, kind, status, summary_code,
                          payload_digest, created_at, updated_at
                   FROM canonical_task_items_v3;
                 DROP TABLE canonical_task_items_v3;
                 DROP TABLE canonical_task_runs_v3;
                 CREATE INDEX idx_canonical_task_items_run
                    ON canonical_task_items(run_id, sequence);",
            )?;
            let changed = tx.execute(
                "UPDATE canonical_task_runtime_metadata SET value = '4'
                 WHERE key = 'schema_version' AND value = '3'",
                [],
            )?;
            if changed != 1 {
                anyhow::bail!("canonical_task_runtime_v3_migration_version_conflict");
            }
            tx.commit()?;
            Ok(())
        })();
        let pragma_restore =
            conn.execute_batch("PRAGMA legacy_alter_table = OFF; PRAGMA foreign_keys = ON;");
        migration?;
        pragma_restore?;
        let violation = conn
            .prepare("PRAGMA foreign_key_check")?
            .query_row([], |_| Ok(()))
            .optional()?;
        if violation.is_some() {
            anyhow::bail!("canonical_task_runtime_v3_migration_foreign_key_violation");
        }
        Ok(())
    }

    fn migrate_v4_to_v5(conn: &mut Connection) -> Result<()> {
        conn.execute_batch("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = ON;")?;
        let migration = (|| -> Result<()> {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(
                "DROP TABLE IF EXISTS canonical_report_steering;
                 ALTER TABLE canonical_task_items RENAME TO canonical_task_items_v4;
                 ALTER TABLE canonical_task_runs RENAME TO canonical_task_runs_v4;
                 CREATE TABLE canonical_task_runs (
                    task_id TEXT NOT NULL,
                    run_id TEXT NOT NULL UNIQUE,
                    execution_session_id TEXT NOT NULL,
                    ordinal INTEGER NOT NULL CHECK(ordinal > 0),
                    execution_facts_version INTEGER NOT NULL DEFAULT 5
                        CHECK(execution_facts_version IN (1, 2, 3, 4, 5)),
                    plan_revision INTEGER NOT NULL DEFAULT 1 CHECK(plan_revision > 0),
                    created_at TEXT NOT NULL,
                    PRIMARY KEY(task_id, run_id),
                    FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT
                 ) WITHOUT ROWID;
                 INSERT INTO canonical_task_runs (
                    task_id, run_id, execution_session_id, ordinal,
                    execution_facts_version, plan_revision, created_at
                 ) SELECT task_id, run_id, execution_session_id, ordinal,
                          execution_facts_version, 1, created_at
                   FROM canonical_task_runs_v4;
                 CREATE TABLE canonical_task_items (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL CHECK(sequence > 0),
                    kind TEXT NOT NULL CHECK(kind IN (
                        'instruction', 'plan', 'steering', 'tool_call', 'observation',
                        'provider_generation', 'artifact_draft',
                        'review_checkpoint', 'artifact_materialized',
                        'verification', 'final_result'
                    )),
                    status TEXT NOT NULL CHECK(status IN (
                        'waiting', 'completed', 'blocked', 'failed', 'effect_unknown'
                    )),
                    summary_code TEXT NOT NULL,
                    payload_digest TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    UNIQUE(task_id, sequence),
                    FOREIGN KEY(task_id, run_id)
                        REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
                 );
                 INSERT INTO canonical_task_items (
                    id, task_id, run_id, sequence, kind, status, summary_code,
                    payload_digest, created_at, updated_at
                 ) SELECT id, task_id, run_id, sequence, kind, status, summary_code,
                          payload_digest, created_at, updated_at
                   FROM canonical_task_items_v4;
                 DROP TABLE canonical_task_items_v4;
                 DROP TABLE canonical_task_runs_v4;
                 CREATE TABLE canonical_report_steering (
                    steering_id TEXT PRIMARY KEY,
                    item_id TEXT NOT NULL UNIQUE,
                    task_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    source_message_ref TEXT NOT NULL UNIQUE,
                    source_message_digest TEXT NOT NULL,
                    steering_digest TEXT NOT NULL,
                    base_plan_revision INTEGER NOT NULL CHECK(base_plan_revision > 0),
                    status TEXT NOT NULL CHECK(status IN ('pending', 'consumed', 'blocked')),
                    created_at TEXT NOT NULL,
                    consumed_at TEXT,
                    FOREIGN KEY(item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT,
                    FOREIGN KEY(task_id, run_id)
                        REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
                 );
                 CREATE INDEX idx_canonical_task_items_run
                    ON canonical_task_items(run_id, sequence);",
            )?;
            let changed = tx.execute(
                "UPDATE canonical_task_runtime_metadata SET value = '5'
                 WHERE key = 'schema_version' AND value = '4'",
                [],
            )?;
            if changed != 1 {
                anyhow::bail!("canonical_task_runtime_v4_migration_version_conflict");
            }
            tx.commit()?;
            Ok(())
        })();
        let pragma_restore =
            conn.execute_batch("PRAGMA legacy_alter_table = OFF; PRAGMA foreign_keys = ON;");
        migration?;
        pragma_restore?;
        let violation = conn
            .prepare("PRAGMA foreign_key_check")?
            .query_row([], |_| Ok(()))
            .optional()?;
        if violation.is_some() {
            anyhow::bail!("canonical_task_runtime_v4_migration_foreign_key_violation");
        }
        Ok(())
    }

    fn migrate_v5_to_v6(conn: &mut Connection) -> Result<()> {
        conn.execute_batch("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = ON;")?;
        let migration = (|| -> Result<()> {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(
                "ALTER TABLE canonical_tasks RENAME TO canonical_tasks_v5;
                 CREATE TABLE canonical_tasks (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL UNIQUE,
                    task_kind TEXT NOT NULL CHECK(task_kind IN ('report', 'plan')),
                    initial_outcome_digest TEXT NOT NULL,
                    status TEXT NOT NULL CHECK(status IN (
                        'running', 'waiting_review', 'completed', 'blocked',
                        'failed', 'effect_unknown'
                    )),
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                 );
                 INSERT INTO canonical_tasks (
                    id, conversation_id, task_kind, initial_outcome_digest,
                    status, created_at, updated_at
                 ) SELECT id, conversation_id, task_kind, initial_outcome_digest,
                          status, created_at, updated_at
                   FROM canonical_tasks_v5;
                 DROP TABLE canonical_tasks_v5;",
            )?;
            let changed = tx.execute(
                "UPDATE canonical_task_runtime_metadata SET value = '6'
                 WHERE key = 'schema_version' AND value = '5'",
                [],
            )?;
            if changed != 1 {
                anyhow::bail!("canonical_task_runtime_v5_migration_version_conflict");
            }
            tx.commit()?;
            Ok(())
        })();
        let pragma_restore =
            conn.execute_batch("PRAGMA legacy_alter_table = OFF; PRAGMA foreign_keys = ON;");
        migration?;
        pragma_restore?;
        let violation = conn
            .prepare("PRAGMA foreign_key_check")?
            .query_row([], |_| Ok(()))
            .optional()?;
        if violation.is_some() {
            anyhow::bail!("canonical_task_runtime_v5_migration_foreign_key_violation");
        }
        Ok(())
    }

    fn validate_schema(conn: &Connection) -> Result<()> {
        let version = Self::schema_version(conn)?;
        if version != TASK_RUNTIME_SCHEMA_VERSION {
            anyhow::bail!("canonical_task_runtime_schema_version_unsupported:{version}");
        }
        Ok(())
    }

    pub fn begin_report_run(&self, input: BeginReportRunInput<'_>) -> Result<BegunReportRun> {
        validate_nonempty("conversation_id", input.conversation_id, 512)?;
        validate_nonempty("execution_session_id", input.execution_session_id, 512)?;
        validate_nonempty("run_id", input.run_id, 512)?;
        validate_digest("outcome_digest", input.outcome_digest)?;
        validate_digest("plan_digest", input.plan_digest)?;
        let task_id = stable_id("task", &["report", input.conversation_id]);
        let instruction_item_id = stable_id("item", &["instruction", &task_id, input.run_id]);
        let plan_item_id = stable_id("item", &["plan", &task_id, input.run_id]);
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO canonical_tasks (
                id, conversation_id, task_kind, initial_outcome_digest,
                status, created_at, updated_at
             ) VALUES (?1, ?2, 'report', ?3, 'running', ?4, ?4)
             ON CONFLICT(conversation_id) DO NOTHING",
            params![task_id, input.conversation_id, input.outcome_digest, now],
        )?;
        let stored_task: (String, String, String) = tx.query_row(
            "SELECT id, initial_outcome_digest, status FROM canonical_tasks
             WHERE conversation_id = ?1",
            [input.conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if stored_task.0 != task_id {
            anyhow::bail!("canonical_report_task_identity_conflict");
        }
        let existing_run = tx
            .query_row(
                "SELECT task_id, execution_session_id, execution_facts_version, plan_revision
                 FROM canonical_task_runs WHERE run_id = ?1",
                [input.run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let plan_revision =
            if let Some((stored_task, stored_session, version, revision)) = existing_run {
                if stored_task != task_id
                    || stored_session != input.execution_session_id
                    || version != 5
                {
                    anyhow::bail!("canonical_report_run_membership_conflict");
                }
                revision
            } else {
                let ordinal: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(ordinal), 0) + 1 FROM canonical_task_runs
                 WHERE task_id = ?1",
                    [&task_id],
                    |row| row.get(0),
                )?;
                tx.execute(
                    "INSERT INTO canonical_task_runs (
                    task_id, run_id, execution_session_id, ordinal,
                    execution_facts_version, plan_revision, created_at
                 ) VALUES (?1, ?2, ?3, ?4, 5, 1, ?5)",
                    params![
                        task_id,
                        input.run_id,
                        input.execution_session_id,
                        ordinal,
                        now
                    ],
                )?;
                1
            };
        ensure_completed_item(
            &tx,
            CompletedItemInput {
                item_id: &instruction_item_id,
                task_id: &task_id,
                run_id: input.run_id,
                kind: CanonicalTaskItemKind::Instruction,
                summary_code: "report_instruction_bound",
                payload_digest: input.outcome_digest,
                now: &now,
            },
        )?;
        ensure_completed_item(
            &tx,
            CompletedItemInput {
                item_id: &plan_item_id,
                task_id: &task_id,
                run_id: input.run_id,
                kind: CanonicalTaskItemKind::Plan,
                summary_code: "report_execution_plan_bound",
                payload_digest: input.plan_digest,
                now: &now,
            },
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = 'running', updated_at = ?2 WHERE id = ?1",
            params![task_id, now],
        )?;
        tx.commit()?;
        Ok(BegunReportRun {
            task_id,
            run_id: input.run_id.to_string(),
            plan_revision: u64::try_from(plan_revision)?,
        })
    }

    pub fn begin_plan_run(&self, input: BeginPlanRunInput<'_>) -> Result<BegunReportRun> {
        validate_nonempty("conversation_id", input.conversation_id, 512)?;
        validate_nonempty("execution_session_id", input.execution_session_id, 512)?;
        validate_nonempty("run_id", input.run_id, 512)?;
        validate_digest("instruction_digest", input.instruction_digest)?;
        validate_digest("plan_digest", input.plan_digest)?;
        let task_id = stable_id("task", &["plan", input.conversation_id]);
        let instruction_item_id = stable_id("item", &["instruction", &task_id, input.run_id]);
        let plan_item_id = stable_id("item", &["plan", &task_id, input.run_id]);
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO canonical_tasks (
                id, conversation_id, task_kind, initial_outcome_digest,
                status, created_at, updated_at
             ) VALUES (?1, ?2, 'plan', ?3, 'running', ?4, ?4)
             ON CONFLICT(conversation_id) DO NOTHING",
            params![
                task_id,
                input.conversation_id,
                input.instruction_digest,
                now
            ],
        )?;
        let stored: (String, String, String) = tx.query_row(
            "SELECT id, task_kind, initial_outcome_digest FROM canonical_tasks
             WHERE conversation_id = ?1",
            [input.conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if stored.0 != task_id || stored.1 != "plan" || stored.2 != input.instruction_digest {
            anyhow::bail!("canonical_plan_task_identity_conflict");
        }
        tx.execute(
            "INSERT INTO canonical_task_runs (
                task_id, run_id, execution_session_id, ordinal,
                execution_facts_version, plan_revision, created_at
             ) VALUES (?1, ?2, ?3, 1, 5, 1, ?4)
             ON CONFLICT(run_id) DO NOTHING",
            params![task_id, input.run_id, input.execution_session_id, now],
        )?;
        let membership: (String, String) = tx.query_row(
            "SELECT task_id, execution_session_id FROM canonical_task_runs WHERE run_id = ?1",
            [input.run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if membership.0 != task_id || membership.1 != input.execution_session_id {
            anyhow::bail!("canonical_plan_run_membership_conflict");
        }
        ensure_completed_item(
            &tx,
            CompletedItemInput {
                item_id: &instruction_item_id,
                task_id: &task_id,
                run_id: input.run_id,
                kind: CanonicalTaskItemKind::Instruction,
                summary_code: "plan_instruction_bound",
                payload_digest: input.instruction_digest,
                now: &now,
            },
        )?;
        ensure_completed_item(
            &tx,
            CompletedItemInput {
                item_id: &plan_item_id,
                task_id: &task_id,
                run_id: input.run_id,
                kind: CanonicalTaskItemKind::Plan,
                summary_code: "plan_draft_bound",
                payload_digest: input.plan_digest,
                now: &now,
            },
        )?;
        tx.commit()?;
        Ok(BegunReportRun {
            task_id,
            run_id: input.run_id.to_string(),
            plan_revision: 1,
        })
    }

    pub fn submit_report_steering(
        &self,
        input: SubmitReportSteeringInput<'_>,
    ) -> Result<CanonicalReportSteeringRecord> {
        validate_nonempty("steering_id", input.steering_id, 512)?;
        validate_nonempty("task_id", input.task_id, 512)?;
        validate_nonempty("run_id", input.run_id, 512)?;
        validate_nonempty("source_message_ref", input.source_message_ref, 1024)?;
        validate_digest("source_message_digest", input.source_message_digest)?;
        validate_digest("steering_digest", input.steering_digest)?;
        if input.base_plan_revision == 0 {
            anyhow::bail!("canonical_report_steering_plan_revision_invalid");
        }
        let item_id = stable_id(
            "item",
            &["steering", input.task_id, input.run_id, input.steering_id],
        );
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = load_steering_in_tx(&tx, input.steering_id)? {
            if existing.item_id != item_id
                || existing.task_id != input.task_id
                || existing.run_id != input.run_id
                || existing.source_message_ref != input.source_message_ref
                || existing.source_message_digest != input.source_message_digest
                || existing.steering_digest != input.steering_digest
                || existing.base_plan_revision != input.base_plan_revision
                || (existing.status == CanonicalSteeringStatus::Blocked)
                    != input.scope_expansion_blocked
            {
                anyhow::bail!("canonical_report_steering_identity_conflict");
            }
            tx.commit()?;
            return Ok(existing);
        }
        let (task_status, plan_revision, run_finalized): (String, i64, bool) = tx
            .query_row(
                "SELECT task.status, run.plan_revision,
                        EXISTS(SELECT 1 FROM canonical_task_items terminal
                               WHERE terminal.task_id = task.id
                                 AND terminal.run_id = run.run_id
                                 AND terminal.kind = 'final_result'
                                 AND terminal.status = 'completed')
                 FROM canonical_tasks task
                 JOIN canonical_task_runs run ON run.task_id = task.id
                 WHERE task.id = ?1 AND run.run_id = ?2",
                params![input.task_id, input.run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .with_context(|| "canonical_report_steering_target_missing")?;
        if run_finalized || !matches!(task_status.as_str(), "running" | "waiting_review") {
            anyhow::bail!("canonical_report_steering_target_terminal");
        }
        if u64::try_from(plan_revision)? != input.base_plan_revision {
            anyhow::bail!("canonical_report_steering_plan_revision_stale");
        }
        if !input.scope_expansion_blocked {
            let pending: Option<String> = tx
                .query_row(
                    "SELECT steering_id FROM canonical_report_steering
                     WHERE task_id = ?1 AND run_id = ?2 AND status = 'pending'",
                    params![input.task_id, input.run_id],
                    |row| row.get(0),
                )
                .optional()?;
            if pending.is_some() {
                anyhow::bail!("canonical_report_steering_pending_conflict");
            }
        }
        let item_status = if input.scope_expansion_blocked {
            CanonicalTaskItemStatus::Blocked
        } else {
            CanonicalTaskItemStatus::Waiting
        };
        let steering_status = if input.scope_expansion_blocked {
            CanonicalSteeringStatus::Blocked
        } else {
            CanonicalSteeringStatus::Pending
        };
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM canonical_task_items WHERE task_id = ?1",
            [input.task_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO canonical_task_items (
                id, task_id, run_id, sequence, kind, status, summary_code,
                payload_digest, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'steering', ?5, ?6, ?7, ?8, ?8)",
            params![
                item_id,
                input.task_id,
                input.run_id,
                sequence,
                item_status.as_str(),
                if input.scope_expansion_blocked {
                    "report_steering_scope_expansion_blocked"
                } else {
                    "report_steering_pending"
                },
                input.steering_digest,
                now
            ],
        )?;
        tx.execute(
            "INSERT INTO canonical_report_steering (
                steering_id, item_id, task_id, run_id, source_message_ref,
                source_message_digest, steering_digest, base_plan_revision,
                status, created_at, consumed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL)",
            params![
                input.steering_id,
                item_id,
                input.task_id,
                input.run_id,
                input.source_message_ref,
                input.source_message_digest,
                input.steering_digest,
                i64::try_from(input.base_plan_revision)?,
                steering_status.as_str(),
                now
            ],
        )?;
        let record = load_steering_in_tx(&tx, input.steering_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_report_steering_missing_after_insert"))?;
        tx.commit()?;
        Ok(record)
    }

    pub fn consume_pending_report_steering(
        &self,
        task_id: &str,
        run_id: &str,
    ) -> Result<Option<CanonicalReportSteeringRecord>> {
        validate_nonempty("task_id", task_id, 512)?;
        validate_nonempty("run_id", run_id, 512)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let steering_id = tx
            .query_row(
                "SELECT steering_id FROM canonical_report_steering
                 WHERE task_id = ?1 AND run_id = ?2 AND status = 'pending'
                 ORDER BY created_at ASC, steering_id ASC LIMIT 1",
                params![task_id, run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(steering_id) = steering_id else {
            tx.commit()?;
            return Ok(None);
        };
        let record = load_steering_in_tx(&tx, &steering_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_report_steering_missing_before_consume"))?;
        let (task_status, plan_revision): (String, i64) = tx.query_row(
            "SELECT task.status, run.plan_revision
             FROM canonical_tasks task
             JOIN canonical_task_runs run ON run.task_id = task.id
             WHERE task.id = ?1 AND run.run_id = ?2",
            params![task_id, run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if task_status != "running" {
            anyhow::bail!("canonical_report_steering_consume_target_not_running");
        }
        if u64::try_from(plan_revision)? != record.base_plan_revision {
            anyhow::bail!("canonical_report_steering_consume_revision_conflict");
        }
        let changed = tx.execute(
            "UPDATE canonical_report_steering
             SET status = 'consumed', consumed_at = ?2
             WHERE steering_id = ?1 AND status = 'pending'",
            params![steering_id, now],
        )?;
        if changed != 1 {
            anyhow::bail!("canonical_report_steering_consume_conflict");
        }
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'completed', summary_code = 'report_steering_consumed', updated_at = ?2
             WHERE id = ?1 AND kind = 'steering' AND status = 'waiting'",
            params![record.item_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_runs SET plan_revision = plan_revision + 1
             WHERE task_id = ?1 AND run_id = ?2 AND plan_revision = ?3",
            params![task_id, run_id, plan_revision],
        )?;
        let consumed = load_steering_in_tx(&tx, &record.steering_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_report_steering_missing_after_consume"))?;
        tx.commit()?;
        Ok(Some(consumed))
    }

    pub fn load_report_steering(
        &self,
        steering_id: &str,
    ) -> Result<Option<CanonicalReportSteeringRecord>> {
        validate_nonempty("steering_id", steering_id, 512)?;
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let record = load_steering_in_tx(&tx, steering_id)?;
        tx.commit()?;
        Ok(record)
    }

    pub fn resolve_report_task_id(
        &self,
        execution_session_id: &str,
        run_id: &str,
    ) -> Result<Option<String>> {
        validate_nonempty("execution_session_id", execution_session_id, 512)?;
        validate_nonempty("run_id", run_id, 512)?;
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT task_id FROM canonical_task_runs
             WHERE execution_session_id = ?1 AND run_id = ?2",
            params![execution_session_id, run_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn resolve_report_run_target(
        &self,
        execution_session_id: &str,
        run_id: &str,
    ) -> Result<Option<(String, u64)>> {
        validate_nonempty("execution_session_id", execution_session_id, 512)?;
        validate_nonempty("run_id", run_id, 512)?;
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT task_id, plan_revision FROM canonical_task_runs
             WHERE execution_session_id = ?1 AND run_id = ?2",
            params![execution_session_id, run_id],
            |row| {
                let revision = u64::try_from(row.get::<_, i64>(1)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Integer,
                        error.into(),
                    )
                })?;
                Ok((row.get(0)?, revision))
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn resolve_report_run_target_for_conversation(
        &self,
        execution_session_id: &str,
        run_id: &str,
        conversation_id: &str,
    ) -> Result<Option<(String, u64)>> {
        validate_nonempty("execution_session_id", execution_session_id, 512)?;
        validate_nonempty("run_id", run_id, 512)?;
        validate_nonempty("conversation_id", conversation_id, 512)?;
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT run.task_id, run.plan_revision
             FROM canonical_task_runs run
             JOIN canonical_tasks task ON task.id = run.task_id
             WHERE run.execution_session_id = ?1
               AND run.run_id = ?2
               AND task.conversation_id = ?3",
            params![execution_session_id, run_id, conversation_id],
            |row| {
                let revision = u64::try_from(row.get::<_, i64>(1)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Integer,
                        error.into(),
                    )
                })?;
                Ok((row.get(0)?, revision))
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn resolve_report_task_id_for_conversation(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<Option<String>> {
        validate_nonempty("conversation_id", conversation_id, 512)?;
        validate_nonempty("run_id", run_id, 512)?;
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT task.id FROM canonical_tasks task
             JOIN canonical_task_runs run ON run.task_id = task.id
             WHERE task.conversation_id = ?1 AND run.run_id = ?2",
            params![conversation_id, run_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_consumed_report_steering(
        &self,
        task_id: &str,
        run_id: &str,
    ) -> Result<Vec<CanonicalReportSteeringRecord>> {
        validate_nonempty("task_id", task_id, 512)?;
        validate_nonempty("run_id", run_id, 512)?;
        let conn = self.lock_conn()?;
        let ids = {
            let mut statement = conn.prepare(
                "SELECT steering_id FROM canonical_report_steering
                 WHERE task_id = ?1 AND run_id = ?2 AND status = 'consumed'
                 ORDER BY consumed_at ASC, steering_id ASC",
            )?;
            let rows =
                statement.query_map(params![task_id, run_id], |row| row.get::<_, String>(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut records = Vec::with_capacity(ids.len());
        for steering_id in ids {
            let record = conn
                .query_row(
                    "SELECT steering_id, item_id, task_id, run_id, source_message_ref,
                            source_message_digest, steering_digest, base_plan_revision,
                            status, created_at, consumed_at
                     FROM canonical_report_steering WHERE steering_id = ?1",
                    [steering_id],
                    row_to_steering,
                )
                .optional()?
                .ok_or_else(|| anyhow::anyhow!("canonical_report_steering_missing_during_list"))?;
            records.push(record);
        }
        Ok(records)
    }

    pub fn prepare_report_artifact(
        &self,
        input: ReportArtifactDraftInput<'_>,
    ) -> Result<PreparedReportArtifact> {
        validate_nonempty("conversation_id", input.conversation_id, 512)?;
        validate_nonempty("execution_session_id", input.execution_session_id, 512)?;
        validate_nonempty("run_id", input.run_id, 512)?;
        validate_digest("outcome_digest", input.outcome_digest)?;
        validate_digest("plan_digest", input.plan_digest)?;
        validate_nonempty("provider_request_id", input.provider_request_id, 512)?;
        validate_digest("provider_receipt_digest", input.provider_receipt_digest)?;
        if input.tool_observations.len() > 64 {
            anyhow::bail!("canonical_report_tool_observation_count_invalid");
        }
        let mut action_ids = std::collections::HashSet::new();
        let mut observation_ids = std::collections::HashSet::new();
        for fact in input.tool_observations {
            validate_nonempty("tool_action_id", fact.action_id, 512)?;
            validate_digest("tool_call_digest", fact.tool_call_digest)?;
            validate_nonempty("tool_observation_id", fact.observation_id, 512)?;
            validate_digest("observation_digest", fact.observation_digest)?;
            if !action_ids.insert(fact.action_id) || !observation_ids.insert(fact.observation_id) {
                anyhow::bail!("canonical_report_tool_observation_identity_duplicate");
            }
        }
        validate_nonempty("target_reference", input.target_reference, 4096)?;
        validate_digest("content_digest", input.content_digest)?;
        validate_nonempty("media_type", input.media_type, 256)?;

        let task_id = stable_id("task", &["report", input.conversation_id]);
        let target_reference_digest = sha256_text(input.target_reference);
        let artifact_id = stable_id(
            "artifact",
            &[
                &task_id,
                input.run_id,
                &target_reference_digest,
                input.content_digest,
            ],
        );
        let item_id = stable_id("item", &["artifact_draft", &artifact_id]);
        let instruction_item_id = stable_id("item", &["instruction", &task_id, input.run_id]);
        let plan_item_id = stable_id("item", &["plan", &task_id, input.run_id]);
        let provider_generation_item_id = stable_id(
            "item",
            &[
                "provider_generation",
                &task_id,
                input.run_id,
                input.provider_request_id,
            ],
        );
        let now = Utc::now();
        let now_text = now.to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        tx.execute(
            "INSERT INTO canonical_tasks (
                id, conversation_id, task_kind, initial_outcome_digest,
                status, created_at, updated_at
             ) VALUES (?1, ?2, 'report', ?3, 'running', ?4, ?4)
             ON CONFLICT(conversation_id) DO UPDATE SET updated_at = excluded.updated_at",
            params![
                task_id,
                input.conversation_id,
                input.outcome_digest,
                now_text
            ],
        )?;
        let stored_task_id: String = tx.query_row(
            "SELECT id FROM canonical_tasks WHERE conversation_id = ?1",
            [input.conversation_id],
            |row| row.get(0),
        )?;
        if stored_task_id != task_id {
            anyhow::bail!("canonical_report_task_identity_conflict");
        }

        let existing_run = tx
            .query_row(
                "SELECT task_id, execution_session_id, execution_facts_version
                 FROM canonical_task_runs
                 WHERE run_id = ?1",
                [input.run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        let is_new_run = existing_run.is_none();
        let execution_facts_version = if let Some((
            existing_task,
            existing_session,
            execution_facts_version,
        )) = existing_run
        {
            if existing_task != task_id || existing_session != input.execution_session_id {
                anyhow::bail!("canonical_report_run_membership_conflict");
            }
            execution_facts_version
        } else {
            let ordinal: i64 = tx.query_row(
                "SELECT COALESCE(MAX(ordinal), 0) + 1 FROM canonical_task_runs
                 WHERE task_id = ?1",
                [&task_id],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT INTO canonical_task_runs (
                    task_id, run_id, execution_session_id, ordinal,
                    execution_facts_version, plan_revision, created_at
                 ) VALUES (?1, ?2, ?3, ?4, 5, 1, ?5)",
                params![
                    task_id,
                    input.run_id,
                    input.execution_session_id,
                    ordinal,
                    now_text
                ],
            )?;
            5
        };

        if matches!(execution_facts_version, 4 | 5) {
            if !is_new_run && execution_facts_version == 4 {
                validate_report_execution_items_v4(
                    &tx,
                    &task_id,
                    input.run_id,
                    &instruction_item_id,
                    &plan_item_id,
                    &provider_generation_item_id,
                    input.tool_observations,
                )?;
            }
            ensure_completed_item(
                &tx,
                CompletedItemInput {
                    item_id: &instruction_item_id,
                    task_id: &task_id,
                    run_id: input.run_id,
                    kind: CanonicalTaskItemKind::Instruction,
                    summary_code: "report_instruction_bound",
                    payload_digest: input.outcome_digest,
                    now: &now_text,
                },
            )?;
            ensure_completed_item(
                &tx,
                CompletedItemInput {
                    item_id: &plan_item_id,
                    task_id: &task_id,
                    run_id: input.run_id,
                    kind: CanonicalTaskItemKind::Plan,
                    summary_code: "report_execution_plan_bound",
                    payload_digest: input.plan_digest,
                    now: &now_text,
                },
            )?;
            for fact in input.tool_observations {
                let tool_call_item_id = stable_id(
                    "item",
                    &["tool_call", &task_id, input.run_id, fact.action_id],
                );
                ensure_completed_item(
                    &tx,
                    CompletedItemInput {
                        item_id: &tool_call_item_id,
                        task_id: &task_id,
                        run_id: input.run_id,
                        kind: CanonicalTaskItemKind::ToolCall,
                        summary_code: "report_governed_tool_call_completed",
                        payload_digest: fact.tool_call_digest,
                        now: &now_text,
                    },
                )?;
                let observation_item_id = stable_id(
                    "item",
                    &[
                        "observation",
                        &task_id,
                        input.run_id,
                        fact.action_id,
                        fact.observation_id,
                    ],
                );
                ensure_completed_item(
                    &tx,
                    CompletedItemInput {
                        item_id: &observation_item_id,
                        task_id: &task_id,
                        run_id: input.run_id,
                        kind: CanonicalTaskItemKind::Observation,
                        summary_code: "report_governed_observation_bound",
                        payload_digest: fact.observation_digest,
                        now: &now_text,
                    },
                )?;
            }
            ensure_completed_item(
                &tx,
                CompletedItemInput {
                    item_id: &provider_generation_item_id,
                    task_id: &task_id,
                    run_id: input.run_id,
                    kind: CanonicalTaskItemKind::ProviderGeneration,
                    summary_code: "report_provider_generation_completed",
                    payload_digest: input.provider_receipt_digest,
                    now: &now_text,
                },
            )?;
            if execution_facts_version == 5 {
                validate_report_execution_items_v4(
                    &tx,
                    &task_id,
                    input.run_id,
                    &instruction_item_id,
                    &plan_item_id,
                    &provider_generation_item_id,
                    input.tool_observations,
                )?;
            }
        } else if execution_facts_version == 3 {
            validate_report_execution_items_v3(
                &tx,
                &task_id,
                input.run_id,
                &instruction_item_id,
                &provider_generation_item_id,
                input.tool_observations,
            )?;
        } else if !input.tool_observations.is_empty() {
            anyhow::bail!("canonical_report_legacy_run_cannot_bind_tool_observations");
        }

        let existing_artifact = tx
            .query_row(
                "SELECT task_id, source_item_id, content_digest,
                        target_reference_digest, media_type
                 FROM canonical_artifacts WHERE id = ?1",
                [&artifact_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;
        if let Some((stored_task, stored_item, stored_content, stored_target, stored_media)) =
            existing_artifact
        {
            if stored_task != task_id
                || stored_item != item_id
                || stored_content != input.content_digest
                || stored_target != target_reference_digest
                || stored_media != input.media_type
            {
                anyhow::bail!("canonical_report_artifact_identity_conflict");
            }
        } else {
            if !matches!(execution_facts_version, 4 | 5) {
                anyhow::bail!("canonical_report_legacy_run_cannot_add_artifact");
            }
            let finalized_item_id = report_final_result_item_id(&task_id, input.run_id);
            if tx
                .query_row(
                    "SELECT 1 FROM canonical_task_items
                     WHERE id = ?1 AND kind = 'final_result' AND status = 'completed'",
                    [&finalized_item_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some()
            {
                anyhow::bail!("canonical_report_run_already_finalized");
            }
            tx.execute(
                "UPDATE canonical_tasks SET status = 'running', updated_at = ?2
                 WHERE id = ?1",
                params![task_id, now_text],
            )?;
            let sequence: i64 = tx.query_row(
                "SELECT COALESCE(MAX(sequence), 0) + 1 FROM canonical_task_items
                 WHERE task_id = ?1",
                [&task_id],
                |row| row.get(0),
            )?;
            let payload_digest = sha256_text(&format!(
                "{}\0{}\0{}",
                target_reference_digest, input.content_digest, input.media_type
            ));
            tx.execute(
                "INSERT INTO canonical_task_items (
                    id, task_id, run_id, sequence, kind, status, summary_code,
                    payload_digest, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, 'artifact_draft', 'completed',
                           'report_artifact_draft_prepared', ?5, ?6, ?6)",
                params![
                    item_id,
                    task_id,
                    input.run_id,
                    sequence,
                    payload_digest,
                    now_text
                ],
            )?;
            tx.execute(
                "INSERT INTO canonical_artifacts (
                    id, task_id, source_item_id, current_version, status,
                    media_type, target_reference_digest, content_digest,
                    proposal_id, materialized_reference, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 1, 'draft', ?4, ?5, ?6,
                           NULL, NULL, ?7, ?7)",
                params![
                    artifact_id,
                    task_id,
                    item_id,
                    input.media_type,
                    target_reference_digest,
                    input.content_digest,
                    now_text
                ],
            )?;
            tx.execute(
                "INSERT INTO canonical_artifact_versions (
                    artifact_id, version, source_item_id, content_digest,
                    materialized_reference, observed_content_digest,
                    created_at, materialized_at
                 ) VALUES (?1, 1, ?2, ?3, NULL, NULL, ?4, NULL)",
                params![artifact_id, item_id, input.content_digest, now_text],
            )?;
        }
        tx.commit()?;
        Ok(PreparedReportArtifact {
            task_id,
            artifact_draft_item_id: item_id,
            artifact_id,
            version: 1,
        })
    }

    pub fn bind_report_review(
        &self,
        artifact_id: &str,
        proposal_id: &str,
    ) -> Result<BoundReportReview> {
        validate_nonempty("artifact_id", artifact_id, 512)?;
        validate_nonempty("proposal_id", proposal_id, 512)?;
        let checkpoint_item_id = stable_id("item", &["review_checkpoint", artifact_id]);
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (task_id, source_run_id, current_proposal): (String, String, Option<String>) = tx
            .query_row(
                "SELECT artifact.task_id, item.run_id, artifact.proposal_id
                 FROM canonical_artifacts artifact
                 JOIN canonical_task_items item ON item.id = artifact.source_item_id
                 WHERE artifact.id = ?1",
                [artifact_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .with_context(|| "canonical_report_artifact_missing_before_review")?;
        if current_proposal
            .as_deref()
            .is_some_and(|current| current != proposal_id)
        {
            anyhow::bail!("canonical_report_artifact_proposal_conflict");
        }
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM canonical_task_items
             WHERE task_id = ?1",
            [&task_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO canonical_task_items (
                id, task_id, run_id, sequence, kind, status, summary_code,
                payload_digest, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'review_checkpoint', 'waiting',
                       'report_artifact_review_required', ?5, ?6, ?6)
             ON CONFLICT(id) DO NOTHING",
            params![
                checkpoint_item_id,
                task_id,
                source_run_id,
                sequence,
                sha256_text(proposal_id),
                now
            ],
        )?;
        tx.execute(
            "UPDATE canonical_artifacts
             SET status = 'waiting_review', proposal_id = ?2, updated_at = ?3
             WHERE id = ?1 AND status IN ('draft', 'waiting_review')",
            params![artifact_id, proposal_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = 'waiting_review', updated_at = ?2
             WHERE id = ?1 AND status IN ('running', 'waiting_review')",
            params![task_id, now],
        )?;
        tx.commit()?;
        Ok(BoundReportReview {
            task_id,
            artifact_id: artifact_id.to_string(),
            checkpoint_item_id,
            proposal_id: proposal_id.to_string(),
        })
    }

    pub fn confirm_artifact_materialized(
        &self,
        proposal_id: &str,
        materialized_reference: &str,
        observed_content_digest: &str,
    ) -> Result<CanonicalArtifactRecord> {
        validate_nonempty("proposal_id", proposal_id, 512)?;
        validate_nonempty("materialized_reference", materialized_reference, 4096)?;
        validate_digest("observed_content_digest", observed_content_digest)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (artifact_id, task_id, source_item_id, expected_digest): (
            String,
            String,
            String,
            String,
        ) = tx
            .query_row(
                "SELECT id, task_id, source_item_id, content_digest
                 FROM canonical_artifacts WHERE proposal_id = ?1",
                [proposal_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .with_context(|| "canonical_report_artifact_missing_for_confirmed_proposal")?;
        if expected_digest != observed_content_digest {
            anyhow::bail!("canonical_report_artifact_observed_digest_mismatch");
        }
        let version_changed = tx.execute(
            "UPDATE canonical_artifact_versions
             SET materialized_reference = ?3, observed_content_digest = ?4,
                 materialized_at = ?5
             WHERE artifact_id = ?1 AND version = 1
               AND content_digest = ?2
               AND observed_content_digest IS NULL
               AND materialized_reference IS NULL",
            params![
                artifact_id,
                expected_digest,
                materialized_reference,
                observed_content_digest,
                now
            ],
        )?;
        if version_changed != 1 {
            let existing: (Option<String>, Option<String>) = tx.query_row(
                "SELECT materialized_reference, observed_content_digest
                 FROM canonical_artifact_versions
                 WHERE artifact_id = ?1 AND version = 1 AND content_digest = ?2",
                params![artifact_id, expected_digest],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if existing.0.as_deref() != Some(materialized_reference)
                || existing.1.as_deref() != Some(observed_content_digest)
            {
                anyhow::bail!("canonical_report_artifact_version_confirm_cas_failed");
            }
        }
        let artifact_changed = tx.execute(
            "UPDATE canonical_artifacts
             SET status = 'materialized', materialized_reference = ?2, updated_at = ?3
             WHERE id = ?1 AND status = 'waiting_review'",
            params![artifact_id, materialized_reference, now],
        )?;
        if artifact_changed != 1 {
            let existing: (String, Option<String>) = tx.query_row(
                "SELECT status, materialized_reference FROM canonical_artifacts WHERE id = ?1",
                [&artifact_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if existing.0 != CanonicalArtifactStatus::Materialized.as_str()
                || existing.1.as_deref() != Some(materialized_reference)
            {
                anyhow::bail!("canonical_report_artifact_confirm_cas_failed");
            }
        }
        let checkpoint_item_id = stable_id("item", &["review_checkpoint", &artifact_id]);
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'completed', summary_code = 'report_artifact_review_accepted',
                 updated_at = ?2
             WHERE id = ?1 AND kind = 'review_checkpoint'
               AND status = 'waiting'",
            params![checkpoint_item_id, now],
        )?;
        let materialized_item_id = stable_id("item", &["artifact_materialized", &artifact_id]);
        let run_id: String = tx.query_row(
            "SELECT run_id FROM canonical_task_items WHERE id = ?1",
            [&source_item_id],
            |row| row.get(0),
        )?;
        let materialized_payload_digest = sha256_text(&format!(
            "{}\0{}",
            materialized_reference, observed_content_digest
        ));
        ensure_completed_item(
            &tx,
            CompletedItemInput {
                item_id: &materialized_item_id,
                task_id: &task_id,
                run_id: &run_id,
                kind: CanonicalTaskItemKind::ArtifactMaterialized,
                summary_code: "report_artifact_materialized",
                payload_digest: &materialized_payload_digest,
                now: &now,
            },
        )?;

        let verification_item_id =
            report_verification_item_id(&artifact_id, 1, observed_content_digest);
        let verification_payload_digest = sha256_text(&format!(
            "{}\01\0{}\0{}\0{}",
            artifact_id,
            sha256_text(materialized_reference),
            expected_digest,
            observed_content_digest
        ));
        ensure_completed_item(
            &tx,
            CompletedItemInput {
                item_id: &verification_item_id,
                task_id: &task_id,
                run_id: &run_id,
                kind: CanonicalTaskItemKind::Verification,
                summary_code: "report_artifact_version_verified",
                payload_digest: &verification_payload_digest,
                now: &now,
            },
        )?;

        let artifact_facts = {
            let mut statement = tx.prepare(
                "SELECT artifact.id, artifact.content_digest,
                        artifact.materialized_reference,
                        version.observed_content_digest
                 FROM canonical_artifacts artifact
                 JOIN canonical_artifact_versions version
                   ON version.artifact_id = artifact.id
                  AND version.version = artifact.current_version
                 WHERE artifact.task_id = ?1
                 ORDER BY artifact.id ASC",
            )?;
            let rows = statement.query_map([&task_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut all_verified = !artifact_facts.is_empty();
        let mut final_result_facts = Vec::with_capacity(artifact_facts.len());
        for (candidate_id, content_digest, reference, observed_digest) in &artifact_facts {
            let (Some(reference), Some(observed_digest)) = (reference, observed_digest) else {
                all_verified = false;
                continue;
            };
            if observed_digest != content_digest {
                all_verified = false;
                continue;
            }
            let candidate_verification_id =
                report_verification_item_id(candidate_id, 1, observed_digest);
            let verified = tx
                .query_row(
                    "SELECT 1 FROM canonical_task_items
                     WHERE id = ?1 AND task_id = ?2
                       AND kind = 'verification' AND status = 'completed'",
                    params![candidate_verification_id, task_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !verified {
                all_verified = false;
                continue;
            }
            final_result_facts.push(format!(
                "{}\01\0{}\0{}\0{}",
                candidate_id,
                content_digest,
                sha256_text(reference),
                candidate_verification_id
            ));
        }
        if all_verified {
            let final_result_item_id = report_final_result_item_id(&task_id, &run_id);
            let final_result_payload_digest = sha256_text(&final_result_facts.join("\u{001e}"));
            ensure_completed_item(
                &tx,
                CompletedItemInput {
                    item_id: &final_result_item_id,
                    task_id: &task_id,
                    run_id: &run_id,
                    kind: CanonicalTaskItemKind::FinalResult,
                    summary_code: "report_final_result_verified",
                    payload_digest: &final_result_payload_digest,
                    now: &now,
                },
            )?;
        }
        tx.execute(
            "UPDATE canonical_tasks SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![
                task_id,
                if all_verified {
                    CanonicalTaskStatus::Completed.as_str()
                } else {
                    CanonicalTaskStatus::WaitingReview.as_str()
                },
                now
            ],
        )?;
        tx.commit()?;
        drop(conn);
        self.load_artifact(&artifact_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_report_artifact_missing_after_confirm"))
    }

    pub fn mark_artifact_effect_unknown(&self, proposal_id: &str, reason_code: &str) -> Result<()> {
        validate_nonempty("proposal_id", proposal_id, 512)?;
        validate_nonempty("reason_code", reason_code, 256)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task_id = tx
            .query_row(
                "SELECT task_id FROM canonical_artifacts WHERE proposal_id = ?1",
                [proposal_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(task_id) = task_id else {
            tx.commit()?;
            return Ok(());
        };
        tx.execute(
            "UPDATE canonical_artifacts SET status = 'effect_unknown', updated_at = ?2
             WHERE proposal_id = ?1 AND status != 'materialized'",
            params![proposal_id, now],
        )?;
        let artifact_id: String = tx.query_row(
            "SELECT id FROM canonical_artifacts WHERE proposal_id = ?1",
            [proposal_id],
            |row| row.get(0),
        )?;
        let checkpoint_item_id = stable_id("item", &["review_checkpoint", &artifact_id]);
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'effect_unknown', summary_code = ?2, updated_at = ?3
             WHERE id = ?1 AND kind = 'review_checkpoint' AND status = 'waiting'",
            params![checkpoint_item_id, reason_code, now],
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = 'effect_unknown', updated_at = ?2
             WHERE id = ?1 AND status != 'completed'",
            params![task_id, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn mark_artifact_failed_before_effect(
        &self,
        proposal_id: &str,
        reason_code: &str,
    ) -> Result<()> {
        validate_nonempty("proposal_id", proposal_id, 512)?;
        validate_nonempty("reason_code", reason_code, 256)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task_id = tx
            .query_row(
                "SELECT task_id FROM canonical_artifacts WHERE proposal_id = ?1",
                [proposal_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(task_id) = task_id else {
            tx.commit()?;
            return Ok(());
        };
        tx.execute(
            "UPDATE canonical_artifacts SET status = 'failed', updated_at = ?2
             WHERE proposal_id = ?1 AND status != 'materialized'",
            params![proposal_id, now],
        )?;
        let artifact_id: String = tx.query_row(
            "SELECT id FROM canonical_artifacts WHERE proposal_id = ?1",
            [proposal_id],
            |row| row.get(0),
        )?;
        let checkpoint_item_id = stable_id("item", &["review_checkpoint", &artifact_id]);
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'failed', summary_code = ?2, updated_at = ?3
             WHERE id = ?1 AND kind = 'review_checkpoint' AND status = 'waiting'",
            params![checkpoint_item_id, reason_code, now],
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = 'failed', updated_at = ?2
             WHERE id = ?1 AND status != 'completed'",
            params![task_id, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn mark_artifact_review_rejected(
        &self,
        proposal_id: &str,
    ) -> Result<CanonicalArtifactRecord> {
        validate_nonempty("proposal_id", proposal_id, 512)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (artifact_id, task_id, status): (String, String, String) = tx
            .query_row(
                "SELECT id, task_id, status FROM canonical_artifacts
                 WHERE proposal_id = ?1",
                [proposal_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .with_context(|| "canonical_report_artifact_missing_for_rejected_proposal")?;
        if status == CanonicalArtifactStatus::Materialized.as_str() {
            anyhow::bail!("canonical_report_materialized_artifact_cannot_be_rejected");
        }
        tx.execute(
            "UPDATE canonical_artifacts SET status = 'failed', updated_at = ?2
             WHERE id = ?1 AND status IN ('draft', 'waiting_review')",
            params![artifact_id, now],
        )?;
        let checkpoint_item_id = stable_id("item", &["review_checkpoint", &artifact_id]);
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'blocked', summary_code = 'report_artifact_review_rejected',
                 updated_at = ?2
             WHERE id = ?1 AND kind = 'review_checkpoint'
               AND status = 'waiting'",
            params![checkpoint_item_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = 'blocked', updated_at = ?2
             WHERE id = ?1 AND status != 'blocked'",
            params![task_id, now],
        )?;
        tx.commit()?;
        drop(conn);
        self.load_artifact(&artifact_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_report_artifact_missing_after_rejection"))
    }

    pub fn load_task(&self, task_id: &str) -> Result<Option<CanonicalTaskRecord>> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, conversation_id, task_kind, initial_outcome_digest,
                    status, created_at, updated_at
             FROM canonical_tasks WHERE id = ?1",
            [task_id],
            row_to_task,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn load_artifact(&self, artifact_id: &str) -> Result<Option<CanonicalArtifactRecord>> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, task_id, source_item_id, current_version, status,
                    media_type, target_reference_digest, content_digest,
                    proposal_id, materialized_reference, created_at, updated_at
             FROM canonical_artifacts WHERE id = ?1",
            [artifact_id],
            row_to_artifact,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn load_artifact_by_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<CanonicalArtifactRecord>> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, task_id, source_item_id, current_version, status,
                    media_type, target_reference_digest, content_digest,
                    proposal_id, materialized_reference, created_at, updated_at
             FROM canonical_artifacts WHERE proposal_id = ?1",
            [proposal_id],
            row_to_artifact,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_items(&self, task_id: &str) -> Result<Vec<CanonicalTaskItemRecord>> {
        let conn = self.lock_conn()?;
        let mut statement = conn.prepare(
            "SELECT id, task_id, run_id, sequence, kind, status, summary_code,
                    payload_digest, created_at, updated_at
             FROM canonical_task_items WHERE task_id = ?1 ORDER BY sequence ASC",
        )?;
        let rows = statement.query_map([task_id], row_to_item)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn load_artifact_version(
        &self,
        artifact_id: &str,
        version: u64,
    ) -> Result<Option<CanonicalArtifactVersionRecord>> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT artifact_id, version, source_item_id, content_digest,
                    materialized_reference, observed_content_digest,
                    created_at, materialized_at
             FROM canonical_artifact_versions
             WHERE artifact_id = ?1 AND version = ?2",
            params![artifact_id, i64::try_from(version)?],
            row_to_artifact_version,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_task_snapshots(&self, limit: u64) -> Result<Vec<CanonicalTaskSnapshot>> {
        let bounded_limit = limit.clamp(1, 200);
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let tasks = {
            let mut statement = tx.prepare(
                "SELECT id, conversation_id, task_kind, initial_outcome_digest,
                        status, created_at, updated_at
                 FROM canonical_tasks
                 ORDER BY updated_at DESC, id ASC LIMIT ?1",
            )?;
            let rows = statement.query_map([i64::try_from(bounded_limit)?], row_to_task)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut snapshots = Vec::with_capacity(tasks.len());
        for task in tasks {
            let runs = {
                let mut statement = tx.prepare(
                    "SELECT task_id, run_id, execution_session_id, ordinal,
                            execution_facts_version, plan_revision, created_at
                     FROM canonical_task_runs WHERE task_id = ?1
                     ORDER BY ordinal ASC, run_id ASC",
                )?;
                let rows = statement.query_map([&task.id], row_to_task_run)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            };
            let items = {
                let mut statement = tx.prepare(
                    "SELECT id, task_id, run_id, sequence, kind, status, summary_code,
                            payload_digest, created_at, updated_at
                     FROM canonical_task_items WHERE task_id = ?1
                     ORDER BY sequence ASC, id ASC",
                )?;
                let rows = statement.query_map([&task.id], row_to_item)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            };
            let artifacts = {
                let mut statement = tx.prepare(
                    "SELECT artifact.id, artifact.task_id, artifact.source_item_id,
                            artifact.current_version, artifact.status, artifact.media_type,
                            artifact.target_reference_digest, artifact.content_digest,
                            artifact.proposal_id, artifact.materialized_reference,
                            artifact.created_at, artifact.updated_at,
                            version.artifact_id, version.version, version.source_item_id,
                            version.content_digest, version.materialized_reference,
                            version.observed_content_digest, version.created_at,
                            version.materialized_at
                     FROM canonical_artifacts artifact
                     JOIN canonical_artifact_versions version
                       ON version.artifact_id = artifact.id
                      AND version.version = artifact.current_version
                     WHERE artifact.task_id = ?1
                     ORDER BY artifact.created_at ASC, artifact.id ASC",
                )?;
                let rows = statement.query_map([&task.id], row_to_artifact_snapshot)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            };
            snapshots.push(CanonicalTaskSnapshot {
                task,
                runs,
                items,
                artifacts,
            });
        }
        tx.commit()?;
        Ok(snapshots)
    }

    pub fn run_count(&self, task_id: &str) -> Result<u64> {
        let conn = self.lock_conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM canonical_task_runs WHERE task_id = ?1",
            [task_id],
            |row| row.get(0),
        )?;
        Ok(u64::try_from(count)?)
    }

    pub fn is_writable(&self) -> bool {
        self.lock_conn()
            .and_then(|conn| {
                conn.query_row("PRAGMA query_only", [], |row| row.get::<_, i64>(0))
                    .map(|query_only| query_only == 0)
                    .map_err(Into::into)
            })
            .unwrap_or(false)
    }
}

fn validate_report_execution_items_v3(
    tx: &rusqlite::Transaction<'_>,
    task_id: &str,
    run_id: &str,
    instruction_item_id: &str,
    provider_generation_item_id: &str,
    tool_observations: &[ReportToolObservationFactInput<'_>],
) -> Result<()> {
    let mut expected = vec![(
        instruction_item_id.to_string(),
        CanonicalTaskItemKind::Instruction,
        "report_instruction_bound",
    )];
    for fact in tool_observations {
        expected.push((
            stable_id("item", &["tool_call", task_id, run_id, fact.action_id]),
            CanonicalTaskItemKind::ToolCall,
            "report_governed_tool_call_completed",
        ));
        expected.push((
            stable_id(
                "item",
                &[
                    "observation",
                    task_id,
                    run_id,
                    fact.action_id,
                    fact.observation_id,
                ],
            ),
            CanonicalTaskItemKind::Observation,
            "report_governed_observation_bound",
        ));
    }
    expected.push((
        provider_generation_item_id.to_string(),
        CanonicalTaskItemKind::ProviderGeneration,
        "report_provider_generation_completed",
    ));

    let actual = {
        let mut statement = tx.prepare(
            "SELECT id, kind, status, summary_code
             FROM canonical_task_items
             WHERE task_id = ?1 AND run_id = ?2
               AND kind IN ('instruction', 'tool_call', 'observation', 'provider_generation')
             ORDER BY sequence ASC",
        )?;
        let rows = statement.query_map(params![task_id, run_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    if actual.len() != expected.len()
        || actual.iter().zip(expected.iter()).any(
            |((id, kind, status, summary), (expected_id, expected_kind, expected_summary))| {
                id != expected_id
                    || kind != expected_kind.as_str()
                    || status != CanonicalTaskItemStatus::Completed.as_str()
                    || summary != expected_summary
            },
        )
    {
        anyhow::bail!("canonical_report_execution_items_missing_or_conflicting");
    }
    Ok(())
}

fn load_steering_in_tx(
    tx: &rusqlite::Transaction<'_>,
    steering_id: &str,
) -> Result<Option<CanonicalReportSteeringRecord>> {
    tx.query_row(
        "SELECT steering_id, item_id, task_id, run_id, source_message_ref,
                source_message_digest, steering_digest, base_plan_revision,
                status, created_at, consumed_at
         FROM canonical_report_steering WHERE steering_id = ?1",
        [steering_id],
        row_to_steering,
    )
    .optional()
    .map_err(Into::into)
}

fn row_to_steering(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalReportSteeringRecord> {
    let status = CanonicalSteeringStatus::from_db(&row.get::<_, String>(8)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, error.into())
    })?;
    let base_plan_revision = u64::try_from(row.get::<_, i64>(7)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Integer, error.into())
    })?;
    Ok(CanonicalReportSteeringRecord {
        steering_id: row.get(0)?,
        item_id: row.get(1)?,
        task_id: row.get(2)?,
        run_id: row.get(3)?,
        source_message_ref: row.get(4)?,
        source_message_digest: row.get(5)?,
        steering_digest: row.get(6)?,
        base_plan_revision,
        status,
        created_at: parse_timestamp(row.get(9)?, "steering_created_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, error.into())
        })?,
        consumed_at: row
            .get::<_, Option<String>>(10)?
            .map(|value| parse_timestamp(value, "steering_consumed_at"))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    10,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            })?,
    })
}

fn validate_report_execution_items_v4(
    tx: &rusqlite::Transaction<'_>,
    task_id: &str,
    run_id: &str,
    instruction_item_id: &str,
    plan_item_id: &str,
    provider_generation_item_id: &str,
    tool_observations: &[ReportToolObservationFactInput<'_>],
) -> Result<()> {
    let mut expected = vec![
        (
            instruction_item_id.to_string(),
            CanonicalTaskItemKind::Instruction,
            "report_instruction_bound",
        ),
        (
            plan_item_id.to_string(),
            CanonicalTaskItemKind::Plan,
            "report_execution_plan_bound",
        ),
    ];
    for fact in tool_observations {
        expected.push((
            stable_id("item", &["tool_call", task_id, run_id, fact.action_id]),
            CanonicalTaskItemKind::ToolCall,
            "report_governed_tool_call_completed",
        ));
        expected.push((
            stable_id(
                "item",
                &[
                    "observation",
                    task_id,
                    run_id,
                    fact.action_id,
                    fact.observation_id,
                ],
            ),
            CanonicalTaskItemKind::Observation,
            "report_governed_observation_bound",
        ));
    }
    expected.push((
        provider_generation_item_id.to_string(),
        CanonicalTaskItemKind::ProviderGeneration,
        "report_provider_generation_completed",
    ));

    validate_report_execution_item_sequence(tx, task_id, run_id, &expected)
}

fn validate_report_execution_item_sequence(
    tx: &rusqlite::Transaction<'_>,
    task_id: &str,
    run_id: &str,
    expected: &[(String, CanonicalTaskItemKind, &str)],
) -> Result<()> {
    let actual = {
        let mut statement = tx.prepare(
            "SELECT id, kind, status, summary_code
             FROM canonical_task_items
             WHERE task_id = ?1 AND run_id = ?2
               AND kind IN (
                   'instruction', 'plan', 'tool_call', 'observation',
                   'provider_generation'
               )
             ORDER BY sequence ASC",
        )?;
        let rows = statement.query_map(params![task_id, run_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    if actual.len() != expected.len()
        || actual.iter().zip(expected.iter()).any(
            |((id, kind, status, summary), (expected_id, expected_kind, expected_summary))| {
                id != expected_id
                    || kind != expected_kind.as_str()
                    || status != CanonicalTaskItemStatus::Completed.as_str()
                    || summary != expected_summary
            },
        )
    {
        anyhow::bail!("canonical_report_execution_items_missing_or_conflicting");
    }
    Ok(())
}

struct CompletedItemInput<'a> {
    item_id: &'a str,
    task_id: &'a str,
    run_id: &'a str,
    kind: CanonicalTaskItemKind,
    summary_code: &'a str,
    payload_digest: &'a str,
    now: &'a str,
}

fn ensure_completed_item(
    tx: &rusqlite::Transaction<'_>,
    input: CompletedItemInput<'_>,
) -> Result<()> {
    let existing = tx
        .query_row(
            "SELECT task_id, run_id, kind, status, summary_code, payload_digest
             FROM canonical_task_items WHERE id = ?1",
            [input.item_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?;
    if let Some((
        stored_task,
        stored_run,
        stored_kind,
        stored_status,
        stored_summary,
        stored_payload,
    )) = existing
    {
        if stored_task != input.task_id
            || stored_run != input.run_id
            || stored_kind != input.kind.as_str()
            || stored_status != CanonicalTaskItemStatus::Completed.as_str()
            || stored_summary != input.summary_code
            || stored_payload != input.payload_digest
        {
            anyhow::bail!("canonical_report_execution_item_conflict");
        }
        return Ok(());
    }
    let sequence: i64 = tx.query_row(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM canonical_task_items
         WHERE task_id = ?1",
        [input.task_id],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO canonical_task_items (
            id, task_id, run_id, sequence, kind, status, summary_code,
            payload_digest, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, 'completed', ?6, ?7, ?8, ?8)",
        params![
            input.item_id,
            input.task_id,
            input.run_id,
            sequence,
            input.kind.as_str(),
            input.summary_code,
            input.payload_digest,
            input.now
        ],
    )?;
    Ok(())
}

fn validate_nonempty(field: &str, value: &str, max_bytes: usize) -> Result<()> {
    if value.trim().is_empty() || value.len() > max_bytes || value.contains('\0') {
        anyhow::bail!("canonical_task_runtime_{field}_invalid");
    }
    Ok(())
}

fn validate_digest(field: &str, value: &str) -> Result<()> {
    validate_nonempty(field, value, 256)?;
    if !value.starts_with("sha256:") || value.len() != "sha256:".len() + 64 {
        anyhow::bail!("canonical_task_runtime_{field}_invalid");
    }
    Ok(())
}

fn sha256_text(value: &str) -> String {
    format!("sha256:{}", hex(digest(&SHA256, value.as_bytes()).as_ref()))
}

fn stable_id(namespace: &str, parts: &[&str]) -> String {
    let mut material = namespace.to_string();
    for part in parts {
        material.push('\0');
        material.push_str(&part.len().to_string());
        material.push(':');
        material.push_str(part);
    }
    let digest = sha256_text(&material);
    format!("{namespace}:{}", &digest["sha256:".len()..])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_timestamp(value: String, field: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value)
        .with_context(|| format!("canonical_task_runtime_{field}_invalid"))
        .map(|value| value.with_timezone(&Utc))
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalTaskRecord> {
    let status = CanonicalTaskStatus::from_db(&row.get::<_, String>(4)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, error.into())
    })?;
    let created_at = parse_timestamp(row.get(5)?, "task_created_at").map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, error.into())
    })?;
    let updated_at = parse_timestamp(row.get(6)?, "task_updated_at").map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, error.into())
    })?;
    Ok(CanonicalTaskRecord {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        task_kind: row.get(2)?,
        initial_outcome_digest: row.get(3)?,
        status,
        created_at,
        updated_at,
    })
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalTaskItemRecord> {
    let kind = CanonicalTaskItemKind::from_db(&row.get::<_, String>(4)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, error.into())
    })?;
    let status = CanonicalTaskItemStatus::from_db(&row.get::<_, String>(5)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, error.into())
    })?;
    let sequence = u64::try_from(row.get::<_, i64>(3)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Integer, error.into())
    })?;
    Ok(CanonicalTaskItemRecord {
        id: row.get(0)?,
        task_id: row.get(1)?,
        run_id: row.get(2)?,
        sequence,
        kind,
        status,
        summary_code: row.get(6)?,
        payload_digest: row.get(7)?,
        created_at: parse_timestamp(row.get(8)?, "item_created_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, error.into())
        })?,
        updated_at: parse_timestamp(row.get(9)?, "item_updated_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, error.into())
        })?,
    })
}

fn row_to_artifact(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalArtifactRecord> {
    let version = u64::try_from(row.get::<_, i64>(3)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Integer, error.into())
    })?;
    let status = CanonicalArtifactStatus::from_db(&row.get::<_, String>(4)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, error.into())
    })?;
    Ok(CanonicalArtifactRecord {
        id: row.get(0)?,
        task_id: row.get(1)?,
        source_item_id: row.get(2)?,
        current_version: version,
        status,
        media_type: row.get(5)?,
        target_reference_digest: row.get(6)?,
        content_digest: row.get(7)?,
        proposal_id: row.get(8)?,
        materialized_reference: row.get(9)?,
        created_at: parse_timestamp(row.get(10)?, "artifact_created_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, error.into())
        })?,
        updated_at: parse_timestamp(row.get(11)?, "artifact_updated_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, error.into())
        })?,
    })
}

fn row_to_artifact_version(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CanonicalArtifactVersionRecord> {
    let version = u64::try_from(row.get::<_, i64>(1)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Integer, error.into())
    })?;
    Ok(CanonicalArtifactVersionRecord {
        artifact_id: row.get(0)?,
        version,
        source_item_id: row.get(2)?,
        content_digest: row.get(3)?,
        materialized_reference: row.get(4)?,
        observed_content_digest: row.get(5)?,
        created_at: parse_timestamp(row.get(6)?, "artifact_version_created_at").map_err(
            |error| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            },
        )?,
        materialized_at: row
            .get::<_, Option<String>>(7)?
            .map(|value| parse_timestamp(value, "artifact_version_materialized_at"))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            })?,
    })
}

fn row_to_task_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalTaskRunRecord> {
    let ordinal = u64::try_from(row.get::<_, i64>(3)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Integer, error.into())
    })?;
    let execution_facts_version = u64::try_from(row.get::<_, i64>(4)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Integer, error.into())
    })?;
    let plan_revision = u64::try_from(row.get::<_, i64>(5)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Integer, error.into())
    })?;
    Ok(CanonicalTaskRunRecord {
        task_id: row.get(0)?,
        run_id: row.get(1)?,
        execution_session_id: row.get(2)?,
        ordinal,
        execution_facts_version,
        plan_revision,
        created_at: parse_timestamp(row.get(6)?, "task_run_created_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, error.into())
        })?,
    })
}

fn row_to_artifact_snapshot(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CanonicalArtifactSnapshot> {
    let artifact = row_to_artifact(row)?;
    let version = u64::try_from(row.get::<_, i64>(13)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(13, rusqlite::types::Type::Integer, error.into())
    })?;
    let current_version = CanonicalArtifactVersionRecord {
        artifact_id: row.get(12)?,
        version,
        source_item_id: row.get(14)?,
        content_digest: row.get(15)?,
        materialized_reference: row.get(16)?,
        observed_content_digest: row.get(17)?,
        created_at: parse_timestamp(row.get(18)?, "artifact_version_created_at").map_err(
            |error| {
                rusqlite::Error::FromSqlConversionFailure(
                    18,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            },
        )?,
        materialized_at: row
            .get::<_, Option<String>>(19)?
            .map(|value| parse_timestamp(value, "artifact_version_materialized_at"))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    19,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            })?,
    };
    if artifact.id != current_version.artifact_id
        || artifact.current_version != current_version.version
        || artifact.source_item_id != current_version.source_item_id
        || artifact.content_digest != current_version.content_digest
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(CanonicalArtifactSnapshot {
        artifact,
        current_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_of(value: &str) -> String {
        sha256_text(value)
    }

    fn prepare_test_artifact(
        store: &CanonicalTaskRuntimeStore,
        conversation: &str,
        run: &str,
    ) -> PreparedReportArtifact {
        let outcome_digest = digest_of("report outcome");
        let content_digest = digest_of("# Report");
        store
            .prepare_report_artifact(ReportArtifactDraftInput {
                conversation_id: conversation,
                execution_session_id: run,
                run_id: run,
                outcome_digest: &outcome_digest,
                plan_digest: &digest_of("report plan"),
                provider_request_id: &format!("provider-request-{run}"),
                provider_receipt_digest: &digest_of(&format!("provider-receipt-{run}")),
                tool_observations: &[],
                target_reference: "/tmp/openlife/report.md",
                content_digest: &content_digest,
                media_type: "text/markdown; charset=utf-8",
            })
            .unwrap()
    }

    #[test]
    fn canonical_plan_run_owns_instruction_and_plan_items_without_artifact() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let instruction_digest = digest_of("plan this week");
        let plan_digest = digest_of("bounded plan steps");
        let begun = store
            .begin_plan_run(BeginPlanRunInput {
                conversation_id: "conversation-plan",
                execution_session_id: "task-session-plan",
                run_id: "run-plan",
                instruction_digest: &instruction_digest,
                plan_digest: &plan_digest,
            })
            .unwrap();

        let task = store.load_task(&begun.task_id).unwrap().unwrap();
        assert_eq!(task.task_kind, "plan");
        let items = store.list_items(&begun.task_id).unwrap();
        assert_eq!(
            items.iter().map(|item| item.kind).collect::<Vec<_>>(),
            vec![
                CanonicalTaskItemKind::Instruction,
                CanonicalTaskItemKind::Plan
            ]
        );

        let replay = store
            .begin_plan_run(BeginPlanRunInput {
                conversation_id: "conversation-plan",
                execution_session_id: "task-session-plan",
                run_id: "run-plan",
                instruction_digest: &instruction_digest,
                plan_digest: &plan_digest,
            })
            .unwrap();
        assert_eq!(replay, begun);
        assert_eq!(store.list_items(&begun.task_id).unwrap().len(), 2);
    }

    fn prepare_artifact_with_facts(
        store: &CanonicalTaskRuntimeStore,
        conversation: &str,
        run: &str,
        outcome_digest: &str,
        provider_request_id: &str,
        provider_receipt_digest: &str,
        target_reference: &str,
        content_digest: &str,
    ) -> Result<PreparedReportArtifact> {
        store.prepare_report_artifact(ReportArtifactDraftInput {
            conversation_id: conversation,
            execution_session_id: run,
            run_id: run,
            outcome_digest,
            plan_digest: &digest_of("report plan"),
            provider_request_id,
            provider_receipt_digest,
            tool_observations: &[],
            target_reference,
            content_digest,
            media_type: "text/markdown; charset=utf-8",
        })
    }

    fn prepare_artifact_with_tools(
        store: &CanonicalTaskRuntimeStore,
        conversation: &str,
        run: &str,
        tool_observations: &[ReportToolObservationFactInput<'_>],
    ) -> Result<PreparedReportArtifact> {
        store.prepare_report_artifact(ReportArtifactDraftInput {
            conversation_id: conversation,
            execution_session_id: run,
            run_id: run,
            outcome_digest: &digest_of("report outcome"),
            plan_digest: &digest_of("report plan"),
            provider_request_id: &format!("provider-request-{run}"),
            provider_receipt_digest: &digest_of(&format!("provider-receipt-{run}")),
            tool_observations,
            target_reference: "/tmp/openlife/report.md",
            content_digest: &digest_of("# Report"),
            media_type: "text/markdown; charset=utf-8",
        })
    }

    fn create_v1_report_database(path: &Path) -> PreparedReportArtifact {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE canonical_task_runtime_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             ) WITHOUT ROWID;
             CREATE TABLE canonical_tasks (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL UNIQUE,
                task_kind TEXT NOT NULL CHECK(task_kind = 'report'),
                initial_outcome_digest TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN (
                    'running', 'waiting_review', 'completed', 'blocked',
                    'failed', 'effect_unknown'
                )),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE TABLE canonical_task_runs (
                task_id TEXT NOT NULL,
                run_id TEXT NOT NULL UNIQUE,
                execution_session_id TEXT NOT NULL,
                ordinal INTEGER NOT NULL CHECK(ordinal > 0),
                created_at TEXT NOT NULL,
                PRIMARY KEY(task_id, run_id),
                FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE TABLE canonical_task_items (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                sequence INTEGER NOT NULL CHECK(sequence > 0),
                kind TEXT NOT NULL CHECK(kind IN (
                    'artifact_draft', 'review_checkpoint', 'artifact_materialized'
                )),
                status TEXT NOT NULL CHECK(status IN (
                    'waiting', 'completed', 'blocked', 'failed', 'effect_unknown'
                )),
                summary_code TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(task_id, sequence),
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             );
             CREATE TABLE canonical_artifacts (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                source_item_id TEXT NOT NULL UNIQUE,
                current_version INTEGER NOT NULL CHECK(current_version > 0),
                status TEXT NOT NULL CHECK(status IN (
                    'draft', 'waiting_review', 'materialized', 'failed', 'effect_unknown'
                )),
                media_type TEXT NOT NULL,
                target_reference_digest TEXT NOT NULL,
                content_digest TEXT NOT NULL,
                proposal_id TEXT UNIQUE,
                materialized_reference TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT,
                FOREIGN KEY(source_item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
             );
             CREATE TABLE canonical_artifact_versions (
                artifact_id TEXT NOT NULL,
                version INTEGER NOT NULL CHECK(version > 0),
                source_item_id TEXT NOT NULL,
                content_digest TEXT NOT NULL,
                materialized_reference TEXT,
                observed_content_digest TEXT,
                created_at TEXT NOT NULL,
                materialized_at TEXT,
                PRIMARY KEY(artifact_id, version),
                FOREIGN KEY(artifact_id) REFERENCES canonical_artifacts(id) ON DELETE RESTRICT,
                FOREIGN KEY(source_item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE INDEX idx_canonical_task_items_run
                ON canonical_task_items(run_id, sequence);
             CREATE INDEX idx_canonical_artifacts_task
                ON canonical_artifacts(task_id, created_at, id);
             INSERT INTO canonical_task_runtime_metadata(key, value)
             VALUES ('schema_version', '1');",
        )
        .unwrap();

        let conversation_id = "conversation-v1";
        let run_id = "run-v1";
        let outcome_digest = digest_of("report outcome");
        let content_digest = digest_of("# Report");
        let target_reference = "/tmp/openlife/report.md";
        let task_id = stable_id("task", &["report", conversation_id]);
        let target_reference_digest = sha256_text(target_reference);
        let artifact_id = stable_id(
            "artifact",
            &[&task_id, run_id, &target_reference_digest, &content_digest],
        );
        let item_id = stable_id("item", &["artifact_draft", &artifact_id]);
        let payload_digest = sha256_text(&format!(
            "{}\0{}\0{}",
            target_reference_digest, content_digest, "text/markdown; charset=utf-8"
        ));
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO canonical_tasks (
                id, conversation_id, task_kind, initial_outcome_digest,
                status, created_at, updated_at
             ) VALUES (?1, ?2, 'report', ?3, 'running', ?4, ?4)",
            params![task_id, conversation_id, outcome_digest, now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO canonical_task_runs (
                task_id, run_id, execution_session_id, ordinal, created_at
             ) VALUES (?1, ?2, ?2, 1, ?3)",
            params![task_id, run_id, now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO canonical_task_items (
                id, task_id, run_id, sequence, kind, status, summary_code,
                payload_digest, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 1, 'artifact_draft', 'completed',
                       'report_artifact_draft_prepared', ?4, ?5, ?5)",
            params![item_id, task_id, run_id, payload_digest, now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO canonical_artifacts (
                id, task_id, source_item_id, current_version, status,
                media_type, target_reference_digest, content_digest,
                proposal_id, materialized_reference, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 1, 'draft', 'text/markdown; charset=utf-8',
                       ?4, ?5, NULL, NULL, ?6, ?6)",
            params![
                artifact_id,
                task_id,
                item_id,
                target_reference_digest,
                content_digest,
                now
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO canonical_artifact_versions (
                artifact_id, version, source_item_id, content_digest,
                materialized_reference, observed_content_digest,
                created_at, materialized_at
             ) VALUES (?1, 1, ?2, ?3, NULL, NULL, ?4, NULL)",
            params![artifact_id, item_id, content_digest, now],
        )
        .unwrap();

        PreparedReportArtifact {
            task_id,
            artifact_draft_item_id: item_id,
            artifact_id,
            version: 1,
        }
    }

    #[test]
    fn report_artifact_identity_is_stable_and_independent_of_proposal() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let first = prepare_test_artifact(&store, "conversation-1", "run-1");
        let replay = prepare_test_artifact(&store, "conversation-1", "run-1");
        assert_eq!(first, replay);
        assert!(first.artifact_id.starts_with("artifact:"));
        assert!(!first.artifact_id.contains("proposal"));
        let items = store.list_items(&first.task_id).unwrap();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].kind, CanonicalTaskItemKind::Instruction);
        assert_eq!(items[1].kind, CanonicalTaskItemKind::Plan);
        assert_eq!(items[2].kind, CanonicalTaskItemKind::ProviderGeneration);
        assert_eq!(items[3].kind, CanonicalTaskItemKind::ArtifactDraft);
        assert!(items
            .iter()
            .all(|item| item.status == CanonicalTaskItemStatus::Completed));

        let review = store
            .bind_report_review(&first.artifact_id, "proposal-1")
            .unwrap();
        let review_replay = store
            .bind_report_review(&first.artifact_id, "proposal-1")
            .unwrap();
        assert_eq!(review, review_replay);
        assert_eq!(store.list_items(&first.task_id).unwrap().len(), 5);
        assert_eq!(
            store
                .load_artifact(&first.artifact_id)
                .unwrap()
                .unwrap()
                .status,
            CanonicalArtifactStatus::WaitingReview
        );
    }

    fn begin_steerable_report(store: &CanonicalTaskRuntimeStore) -> BegunReportRun {
        store
            .begin_report_run(BeginReportRunInput {
                conversation_id: "conversation-steering",
                execution_session_id: "execution-steering",
                run_id: "run-steering",
                outcome_digest: &digest_of("report instruction"),
                plan_digest: &digest_of("report plan"),
            })
            .unwrap()
    }

    #[test]
    fn report_run_begins_before_provider_and_steering_consumes_once() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let begun = begin_steerable_report(&store);
        assert_eq!(
            store
                .resolve_report_run_target_for_conversation(
                    "execution-steering",
                    "run-steering",
                    "conversation-steering"
                )
                .unwrap(),
            Some((begun.task_id.clone(), 1))
        );
        assert!(store
            .resolve_report_run_target_for_conversation(
                "execution-steering",
                "run-steering",
                "another-conversation"
            )
            .unwrap()
            .is_none());
        assert_eq!(begun.plan_revision, 1);
        assert_eq!(store.list_items(&begun.task_id).unwrap().len(), 2);
        let steering_digest = digest_of("focus on privacy risks");
        let steering = store
            .submit_report_steering(SubmitReportSteeringInput {
                steering_id: "steering-1",
                task_id: &begun.task_id,
                run_id: &begun.run_id,
                source_message_ref: "conversation://conversation-steering/message/2",
                source_message_digest: &steering_digest,
                steering_digest: &steering_digest,
                base_plan_revision: 1,
                scope_expansion_blocked: false,
            })
            .unwrap();
        assert_eq!(steering.status, CanonicalSteeringStatus::Pending);
        let consumed = store
            .consume_pending_report_steering(&begun.task_id, &begun.run_id)
            .unwrap()
            .unwrap();
        assert_eq!(consumed.status, CanonicalSteeringStatus::Consumed);
        assert!(consumed.consumed_at.is_some());
        assert!(store
            .consume_pending_report_steering(&begun.task_id, &begun.run_id)
            .unwrap()
            .is_none());
        let snapshot = store.list_task_snapshots(10).unwrap().pop().unwrap();
        assert_eq!(snapshot.runs[0].plan_revision, 2);
        assert_eq!(snapshot.items[2].kind, CanonicalTaskItemKind::Steering);
        assert_eq!(snapshot.items[2].status, CanonicalTaskItemStatus::Completed);
    }

    #[test]
    fn report_steering_is_idempotent_and_conflicting_reuse_is_atomic() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let begun = begin_steerable_report(&store);
        let steering_digest = digest_of("shorter summary");
        let input = || SubmitReportSteeringInput {
            steering_id: "steering-replay",
            task_id: &begun.task_id,
            run_id: &begun.run_id,
            source_message_ref: "conversation://conversation-steering/message/2",
            source_message_digest: &steering_digest,
            steering_digest: &steering_digest,
            base_plan_revision: 1,
            scope_expansion_blocked: false,
        };
        let first = store.submit_report_steering(input()).unwrap();
        assert_eq!(store.submit_report_steering(input()).unwrap(), first);
        let changed_digest = digest_of("longer summary");
        let error = store
            .submit_report_steering(SubmitReportSteeringInput {
                steering_id: "steering-replay",
                task_id: &begun.task_id,
                run_id: &begun.run_id,
                source_message_ref: "conversation://conversation-steering/message/2",
                source_message_digest: &changed_digest,
                steering_digest: &changed_digest,
                base_plan_revision: 1,
                scope_expansion_blocked: false,
            })
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("canonical_report_steering_identity_conflict"));
        assert_eq!(store.list_items(&begun.task_id).unwrap().len(), 3);
    }

    #[test]
    fn scope_expanding_steering_is_recorded_blocked_and_never_consumed() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let begun = begin_steerable_report(&store);
        let steering_digest = digest_of("use another workspace");
        let steering = store
            .submit_report_steering(SubmitReportSteeringInput {
                steering_id: "steering-blocked",
                task_id: &begun.task_id,
                run_id: &begun.run_id,
                source_message_ref: "conversation://conversation-steering/message/2",
                source_message_digest: &steering_digest,
                steering_digest: &steering_digest,
                base_plan_revision: 1,
                scope_expansion_blocked: true,
            })
            .unwrap();
        assert_eq!(steering.status, CanonicalSteeringStatus::Blocked);
        assert!(store
            .consume_pending_report_steering(&begun.task_id, &begun.run_id)
            .unwrap()
            .is_none());
        let items = store.list_items(&begun.task_id).unwrap();
        assert_eq!(items[2].status, CanonicalTaskItemStatus::Blocked);
    }

    #[test]
    fn pending_steering_survives_restart_and_terminal_task_refuses_new_input() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("steering.db");
        let (task_id, run_id) = {
            let store = CanonicalTaskRuntimeStore::new(&path).unwrap();
            let begun = begin_steerable_report(&store);
            let digest = digest_of("use a table");
            store
                .submit_report_steering(SubmitReportSteeringInput {
                    steering_id: "steering-restart",
                    task_id: &begun.task_id,
                    run_id: &begun.run_id,
                    source_message_ref: "conversation://conversation-steering/message/2",
                    source_message_digest: &digest,
                    steering_digest: &digest,
                    base_plan_revision: 1,
                    scope_expansion_blocked: false,
                })
                .unwrap();
            (begun.task_id, begun.run_id)
        };
        let restarted = CanonicalTaskRuntimeStore::new(&path).unwrap();
        assert_eq!(
            restarted
                .consume_pending_report_steering(&task_id, &run_id)
                .unwrap()
                .unwrap()
                .status,
            CanonicalSteeringStatus::Consumed
        );
        restarted
            .lock_conn()
            .unwrap()
            .execute(
                "UPDATE canonical_tasks SET status = 'completed' WHERE id = ?1",
                [&task_id],
            )
            .unwrap();
        let digest = digest_of("late input");
        let error = restarted
            .submit_report_steering(SubmitReportSteeringInput {
                steering_id: "steering-late",
                task_id: &task_id,
                run_id: &run_id,
                source_message_ref: "conversation://conversation-steering/message/3",
                source_message_digest: &digest,
                steering_digest: &digest,
                base_plan_revision: 2,
                scope_expansion_blocked: false,
            })
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("canonical_report_steering_target_terminal"));
    }

    #[test]
    fn completed_report_task_can_begin_a_new_run_without_reopening_the_old_run() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let prepared = prepare_test_artifact(&store, "conversation-new-run", "run-old");
        store
            .bind_report_review(&prepared.artifact_id, "proposal-old")
            .unwrap();
        store
            .confirm_artifact_materialized(
                "proposal-old",
                "/tmp/openlife/report.md",
                &digest_of("# Report"),
            )
            .unwrap();
        let new_run = store
            .begin_report_run(BeginReportRunInput {
                conversation_id: "conversation-new-run",
                execution_session_id: "execution-new",
                run_id: "run-new",
                outcome_digest: &digest_of("new instruction"),
                plan_digest: &digest_of("new plan"),
            })
            .unwrap();
        assert_eq!(new_run.task_id, prepared.task_id);
        assert_eq!(store.run_count(&prepared.task_id).unwrap(), 2);
        let late_digest = digest_of("late old run steering");
        let error = store
            .submit_report_steering(SubmitReportSteeringInput {
                steering_id: "old-run-late",
                task_id: &prepared.task_id,
                run_id: "run-old",
                source_message_ref: "conversation://conversation-new-run/message/3",
                source_message_digest: &late_digest,
                steering_digest: &late_digest,
                base_plan_revision: 1,
                scope_expansion_blocked: false,
            })
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("canonical_report_steering_target_terminal"));
    }

    #[test]
    fn report_records_each_governed_tool_call_before_its_bound_observation() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let first_tool_digest = digest_of("tool-call-1");
        let first_observation_digest = digest_of("observation-1");
        let second_tool_digest = digest_of("tool-call-2");
        let second_observation_digest = digest_of("observation-2");
        let facts = [
            ReportToolObservationFactInput {
                action_id: "action-1",
                tool_call_digest: &first_tool_digest,
                observation_id: "observation-1",
                observation_digest: &first_observation_digest,
            },
            ReportToolObservationFactInput {
                action_id: "action-2",
                tool_call_digest: &second_tool_digest,
                observation_id: "observation-2",
                observation_digest: &second_observation_digest,
            },
        ];
        let prepared =
            prepare_artifact_with_tools(&store, "conversation-tools", "run-tools", &facts).unwrap();
        let items = store.list_items(&prepared.task_id).unwrap();
        assert_eq!(
            items.iter().map(|item| item.kind).collect::<Vec<_>>(),
            vec![
                CanonicalTaskItemKind::Instruction,
                CanonicalTaskItemKind::Plan,
                CanonicalTaskItemKind::ToolCall,
                CanonicalTaskItemKind::Observation,
                CanonicalTaskItemKind::ToolCall,
                CanonicalTaskItemKind::Observation,
                CanonicalTaskItemKind::ProviderGeneration,
                CanonicalTaskItemKind::ArtifactDraft,
            ]
        );
        assert_eq!(items[2].payload_digest, first_tool_digest);
        assert_eq!(items[3].payload_digest, first_observation_digest);
        assert_eq!(items[4].payload_digest, second_tool_digest);
        assert_eq!(items[5].payload_digest, second_observation_digest);
    }

    #[test]
    fn report_tool_observation_replay_rejects_missing_or_changed_facts_atomically() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let tool_digest = digest_of("tool-call-1");
        let observation_digest = digest_of("observation-1");
        let facts = [ReportToolObservationFactInput {
            action_id: "action-1",
            tool_call_digest: &tool_digest,
            observation_id: "observation-1",
            observation_digest: &observation_digest,
        }];
        let prepared =
            prepare_artifact_with_tools(&store, "conversation-tools", "run-tools", &facts).unwrap();
        let before = store.list_task_snapshots(100).unwrap();
        assert!(
            prepare_artifact_with_tools(&store, "conversation-tools", "run-tools", &[]).is_err()
        );
        let changed_tool_digest = digest_of("changed-tool-call");
        let changed = [ReportToolObservationFactInput {
            action_id: "action-1",
            tool_call_digest: &changed_tool_digest,
            observation_id: "observation-1",
            observation_digest: &observation_digest,
        }];
        assert!(
            prepare_artifact_with_tools(&store, "conversation-tools", "run-tools", &changed,)
                .is_err()
        );
        assert_eq!(store.list_task_snapshots(100).unwrap(), before);
        assert_eq!(store.list_items(&prepared.task_id).unwrap().len(), 6);
    }

    #[test]
    fn one_report_task_accepts_multiple_run_memberships() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let first = prepare_test_artifact(&store, "conversation-1", "run-1");
        let second = prepare_test_artifact(&store, "conversation-1", "run-2");
        assert_eq!(first.task_id, second.task_id);
        assert_ne!(first.artifact_id, second.artifact_id);
        assert_eq!(store.run_count(&first.task_id).unwrap(), 2);
        let items = store.list_items(&first.task_id).unwrap();
        assert_eq!(items.len(), 8);
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::Instruction)
                .count(),
            2
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::Plan)
                .count(),
            2
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::ProviderGeneration)
                .count(),
            2
        );
    }

    #[test]
    fn one_run_reuses_execution_facts_for_multiple_artifacts() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let outcome_digest = digest_of("report outcome");
        let provider_receipt_digest = digest_of("provider receipt");
        let first_content = digest_of("# First report");
        let second_content = digest_of("# Second report");
        let first = prepare_artifact_with_facts(
            &store,
            "conversation-1",
            "run-1",
            &outcome_digest,
            "provider-request-1",
            &provider_receipt_digest,
            "/tmp/openlife/first.md",
            &first_content,
        )
        .unwrap();
        let second = prepare_artifact_with_facts(
            &store,
            "conversation-1",
            "run-1",
            &outcome_digest,
            "provider-request-1",
            &provider_receipt_digest,
            "/tmp/openlife/second.md",
            &second_content,
        )
        .unwrap();

        assert_eq!(first.task_id, second.task_id);
        assert_ne!(first.artifact_id, second.artifact_id);
        let items = store.list_items(&first.task_id).unwrap();
        assert_eq!(items.len(), 5);
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::Instruction)
                .count(),
            1
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::Plan)
                .count(),
            1
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::ProviderGeneration)
                .count(),
            1
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::ArtifactDraft)
                .count(),
            2
        );
    }

    #[test]
    fn changed_execution_facts_fail_without_partial_writes() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let outcome_digest = digest_of("report outcome");
        let provider_receipt_digest = digest_of("provider receipt");
        let content_digest = digest_of("# Report");
        let prepared = prepare_artifact_with_facts(
            &store,
            "conversation-1",
            "run-1",
            &outcome_digest,
            "provider-request-1",
            &provider_receipt_digest,
            "/tmp/openlife/report.md",
            &content_digest,
        )
        .unwrap();
        let before = store.list_task_snapshots(100).unwrap();

        let changed_instruction = prepare_artifact_with_facts(
            &store,
            "conversation-1",
            "run-1",
            &digest_of("changed report outcome"),
            "provider-request-1",
            &provider_receipt_digest,
            "/tmp/openlife/report.md",
            &content_digest,
        )
        .unwrap_err();
        assert!(changed_instruction
            .to_string()
            .contains("canonical_report_execution_item_conflict"));
        assert_eq!(store.list_task_snapshots(100).unwrap(), before);

        let changed_plan = store
            .prepare_report_artifact(ReportArtifactDraftInput {
                conversation_id: "conversation-1",
                execution_session_id: "run-1",
                run_id: "run-1",
                outcome_digest: &outcome_digest,
                plan_digest: &digest_of("changed report plan"),
                provider_request_id: "provider-request-1",
                provider_receipt_digest: &provider_receipt_digest,
                tool_observations: &[],
                target_reference: "/tmp/openlife/report.md",
                content_digest: &content_digest,
                media_type: "text/markdown; charset=utf-8",
            })
            .unwrap_err();
        assert!(changed_plan
            .to_string()
            .contains("canonical_report_execution_item_conflict"));
        assert_eq!(store.list_task_snapshots(100).unwrap(), before);

        let changed_receipt = prepare_artifact_with_facts(
            &store,
            "conversation-1",
            "run-1",
            &outcome_digest,
            "provider-request-1",
            &digest_of("changed provider receipt"),
            "/tmp/openlife/report.md",
            &content_digest,
        )
        .unwrap_err();
        assert!(changed_receipt
            .to_string()
            .contains("canonical_report_execution_item_conflict"));
        assert_eq!(store.list_task_snapshots(100).unwrap(), before);
        assert_eq!(
            store
                .load_artifact(&prepared.artifact_id)
                .unwrap()
                .unwrap()
                .status,
            CanonicalArtifactStatus::Draft
        );
    }

    #[test]
    fn one_materialized_artifact_does_not_complete_a_multi_artifact_task() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let first = prepare_test_artifact(&store, "conversation-1", "run-1");
        let second = prepare_test_artifact(&store, "conversation-1", "run-2");
        let first_review = store
            .bind_report_review(&first.artifact_id, "proposal-1")
            .unwrap();
        let second_review = store
            .bind_report_review(&second.artifact_id, "proposal-2")
            .unwrap();

        store
            .confirm_artifact_materialized(
                "proposal-1",
                "/tmp/openlife/report-1.md",
                &digest_of("# Report"),
            )
            .unwrap();

        assert_eq!(
            store.load_task(&first.task_id).unwrap().unwrap().status,
            CanonicalTaskStatus::WaitingReview
        );
        let items = store.list_items(&first.task_id).unwrap();
        assert_eq!(
            items
                .iter()
                .find(|item| item.id == first_review.checkpoint_item_id)
                .unwrap()
                .status,
            CanonicalTaskItemStatus::Completed
        );
        assert_eq!(
            items
                .iter()
                .find(|item| item.id == second_review.checkpoint_item_id)
                .unwrap()
                .status,
            CanonicalTaskItemStatus::Waiting
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::Verification)
                .count(),
            1
        );
        assert!(items
            .iter()
            .all(|item| item.kind != CanonicalTaskItemKind::FinalResult));

        store
            .confirm_artifact_materialized(
                "proposal-2",
                "/tmp/openlife/report-2.md",
                &digest_of("# Report"),
            )
            .unwrap();
        assert_eq!(
            store.load_task(&first.task_id).unwrap().unwrap().status,
            CanonicalTaskStatus::Completed
        );
        let items = store.list_items(&first.task_id).unwrap();
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::Verification)
                .count(),
            2
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::FinalResult)
                .count(),
            1
        );
    }

    #[test]
    fn digest_mismatch_writes_no_materialization_verification_or_final_result() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let prepared = prepare_test_artifact(&store, "conversation-1", "run-1");
        store
            .bind_report_review(&prepared.artifact_id, "proposal-1")
            .unwrap();
        let before = store.list_task_snapshots(100).unwrap();

        let error = store
            .confirm_artifact_materialized(
                "proposal-1",
                "/tmp/openlife/report.md",
                &digest_of("tampered report"),
            )
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("canonical_report_artifact_observed_digest_mismatch"));
        assert_eq!(store.list_task_snapshots(100).unwrap(), before);
        assert_eq!(
            store.load_task(&prepared.task_id).unwrap().unwrap().status,
            CanonicalTaskStatus::WaitingReview
        );
    }

    #[test]
    fn a_new_run_reopens_a_completed_report_task() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let first = prepare_test_artifact(&store, "conversation-1", "run-1");
        store
            .bind_report_review(&first.artifact_id, "proposal-1")
            .unwrap();
        store
            .confirm_artifact_materialized(
                "proposal-1",
                "/tmp/openlife/report-1.md",
                &digest_of("# Report"),
            )
            .unwrap();
        assert_eq!(
            store.load_task(&first.task_id).unwrap().unwrap().status,
            CanonicalTaskStatus::Completed
        );

        let second = prepare_test_artifact(&store, "conversation-1", "run-2");
        assert_eq!(first.task_id, second.task_id);
        assert_eq!(
            store.load_task(&first.task_id).unwrap().unwrap().status,
            CanonicalTaskStatus::Running
        );
        store
            .bind_report_review(&second.artifact_id, "proposal-2")
            .unwrap();
        assert_eq!(
            store.load_task(&first.task_id).unwrap().unwrap().status,
            CanonicalTaskStatus::WaitingReview
        );
    }

    #[test]
    fn confirmed_materialization_updates_same_artifact_version_and_completes_task() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let prepared = prepare_test_artifact(&store, "conversation-1", "run-1");
        store
            .bind_report_review(&prepared.artifact_id, "proposal-1")
            .unwrap();
        let observed = digest_of("# Report");
        let artifact = store
            .confirm_artifact_materialized("proposal-1", "/tmp/openlife/report.md", &observed)
            .unwrap();
        assert_eq!(artifact.id, prepared.artifact_id);
        assert_eq!(artifact.status, CanonicalArtifactStatus::Materialized);
        let version = store
            .load_artifact_version(&prepared.artifact_id, 1)
            .unwrap()
            .unwrap();
        assert_eq!(
            version.observed_content_digest.as_deref(),
            Some(observed.as_str())
        );
        assert_eq!(
            store.load_task(&prepared.task_id).unwrap().unwrap().status,
            CanonicalTaskStatus::Completed
        );
        let items = store.list_items(&prepared.task_id).unwrap();
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::Verification)
                .count(),
            1
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == CanonicalTaskItemKind::FinalResult)
                .count(),
            1
        );
        let before_replay = items;
        store
            .confirm_artifact_materialized("proposal-1", "/tmp/openlife/report.md", &observed)
            .unwrap();
        assert_eq!(store.list_items(&prepared.task_id).unwrap(), before_replay);
    }

    #[test]
    fn effect_unknown_never_completes_task() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let prepared = prepare_test_artifact(&store, "conversation-1", "run-1");
        store
            .bind_report_review(&prepared.artifact_id, "proposal-1")
            .unwrap();
        store
            .mark_artifact_effect_unknown("proposal-1", "filesystem_effect_unknown")
            .unwrap();
        assert_eq!(
            store
                .load_artifact(&prepared.artifact_id)
                .unwrap()
                .unwrap()
                .status,
            CanonicalArtifactStatus::EffectUnknown
        );
        assert_eq!(
            store.load_task(&prepared.task_id).unwrap().unwrap().status,
            CanonicalTaskStatus::EffectUnknown
        );
        let items = store.list_items(&prepared.task_id).unwrap();
        assert!(items.iter().all(|item| {
            !matches!(
                item.kind,
                CanonicalTaskItemKind::Verification | CanonicalTaskItemKind::FinalResult
            )
        }));
    }

    #[test]
    fn rejected_review_blocks_the_same_task_and_checkpoint_without_delivery() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let prepared = prepare_test_artifact(&store, "conversation-1", "run-1");
        let review = store
            .bind_report_review(&prepared.artifact_id, "proposal-1")
            .unwrap();
        let artifact = store.mark_artifact_review_rejected("proposal-1").unwrap();
        assert_eq!(artifact.status, CanonicalArtifactStatus::Failed);
        assert!(artifact.materialized_reference.is_none());
        assert_eq!(
            store.load_task(&prepared.task_id).unwrap().unwrap().status,
            CanonicalTaskStatus::Blocked
        );
        let checkpoint = store
            .list_items(&prepared.task_id)
            .unwrap()
            .into_iter()
            .find(|item| item.id == review.checkpoint_item_id)
            .unwrap();
        assert_eq!(checkpoint.status, CanonicalTaskItemStatus::Blocked);
        assert_eq!(checkpoint.summary_code, "report_artifact_review_rejected");
        assert!(store
            .list_items(&prepared.task_id)
            .unwrap()
            .iter()
            .all(|item| {
                !matches!(
                    item.kind,
                    CanonicalTaskItemKind::Verification | CanonicalTaskItemKind::FinalResult
                )
            }));

        let replay = store.mark_artifact_review_rejected("proposal-1").unwrap();
        assert_eq!(replay, artifact);
    }

    #[test]
    fn file_backed_store_reopens_with_same_task_and_artifact() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("task_runtime.db");
        let prepared = {
            let store = CanonicalTaskRuntimeStore::new(&path).unwrap();
            let prepared = prepare_test_artifact(&store, "conversation-1", "run-1");
            store
                .bind_report_review(&prepared.artifact_id, "proposal-1")
                .unwrap();
            prepared
        };
        let reopened = CanonicalTaskRuntimeStore::new(&path).unwrap();
        assert_eq!(
            reopened
                .load_artifact(&prepared.artifact_id)
                .unwrap()
                .unwrap()
                .proposal_id
                .as_deref(),
            Some("proposal-1")
        );
        assert_eq!(reopened.run_count(&prepared.task_id).unwrap(), 1);
    }

    #[test]
    fn v5_runtime_migrates_report_references_and_admits_canonical_plan_tasks() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("task_runtime_v5.db");
        let prepared = {
            let store = CanonicalTaskRuntimeStore::new(&path).unwrap();
            prepare_test_artifact(&store, "conversation-v5", "run-v5")
        };
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             PRAGMA legacy_alter_table = ON;
             ALTER TABLE canonical_tasks RENAME TO canonical_tasks_v6;
             CREATE TABLE canonical_tasks (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL UNIQUE,
                task_kind TEXT NOT NULL CHECK(task_kind = 'report'),
                initial_outcome_digest TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN (
                    'running', 'waiting_review', 'completed', 'blocked',
                    'failed', 'effect_unknown'
                )),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             INSERT INTO canonical_tasks SELECT * FROM canonical_tasks_v6;
             DROP TABLE canonical_tasks_v6;
             UPDATE canonical_task_runtime_metadata
                SET value = '5' WHERE key = 'schema_version';
             PRAGMA legacy_alter_table = OFF;
             PRAGMA foreign_keys = ON;",
        )
        .unwrap();
        drop(conn);

        let store = CanonicalTaskRuntimeStore::new(&path).unwrap();
        let report_snapshot = store
            .list_task_snapshots(100)
            .unwrap()
            .into_iter()
            .find(|snapshot| snapshot.task.id == prepared.task_id)
            .expect("migrated report task");
        assert_eq!(report_snapshot.task.task_kind, "report");
        assert_eq!(report_snapshot.runs.len(), 1);
        assert_eq!(
            report_snapshot.artifacts[0].artifact.id,
            prepared.artifact_id
        );

        let plan = store
            .begin_plan_run(BeginPlanRunInput {
                conversation_id: "conversation-plan-after-v5",
                execution_session_id: "task-session-plan-after-v5",
                run_id: "run-plan-after-v5",
                instruction_digest: &digest_of("plan after migration"),
                plan_digest: &digest_of("bounded plan after migration"),
            })
            .unwrap();
        assert_eq!(
            store.load_task(&plan.task_id).unwrap().unwrap().task_kind,
            "plan"
        );
    }

    #[test]
    fn v1_runtime_migrates_without_rewriting_legacy_execution_facts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("task_runtime_v1.db");
        let legacy = create_v1_report_database(&path);

        let store = CanonicalTaskRuntimeStore::new(&path).unwrap();
        let snapshots = store.list_task_snapshots(100).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].task.id, legacy.task_id);
        assert_eq!(snapshots[0].runs.len(), 1);
        assert_eq!(snapshots[0].runs[0].execution_facts_version, 1);
        assert_eq!(snapshots[0].items.len(), 1);
        assert_eq!(
            snapshots[0].items[0].kind,
            CanonicalTaskItemKind::ArtifactDraft
        );
        assert_eq!(snapshots[0].artifacts[0].artifact.id, legacy.artifact_id);

        let replay = prepare_artifact_with_facts(
            &store,
            "conversation-v1",
            "run-v1",
            &digest_of("report outcome"),
            "provider-request-v1",
            &digest_of("provider receipt v1"),
            "/tmp/openlife/report.md",
            &digest_of("# Report"),
        )
        .unwrap();
        assert_eq!(replay, legacy);
        assert_eq!(store.list_items(&legacy.task_id).unwrap().len(), 1);

        let second = prepare_artifact_with_facts(
            &store,
            "conversation-v1",
            "run-v2",
            &digest_of("second report outcome"),
            "provider-request-v2",
            &digest_of("provider receipt v2"),
            "/tmp/openlife/second.md",
            &digest_of("# Second report"),
        )
        .unwrap();
        assert_eq!(second.task_id, legacy.task_id);
        let migrated = store.list_task_snapshots(100).unwrap();
        assert_eq!(migrated[0].runs.len(), 2);
        assert_eq!(migrated[0].runs[0].execution_facts_version, 1);
        assert_eq!(migrated[0].runs[1].execution_facts_version, 5);
        assert_eq!(migrated[0].items.len(), 5);
    }

    #[test]
    fn v2_runtime_migrates_and_keeps_legacy_runs_readable() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("task_runtime_v2.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE canonical_task_runtime_metadata (
                key TEXT PRIMARY KEY, value TEXT NOT NULL
             ) WITHOUT ROWID;
             INSERT INTO canonical_task_runtime_metadata VALUES ('schema_version', '2');
             CREATE TABLE canonical_tasks (
                id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL UNIQUE,
                task_kind TEXT NOT NULL CHECK(task_kind = 'report'),
                initial_outcome_digest TEXT NOT NULL, status TEXT NOT NULL,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL
             );
             CREATE TABLE canonical_task_runs (
                task_id TEXT NOT NULL, run_id TEXT NOT NULL UNIQUE,
                execution_session_id TEXT NOT NULL,
                ordinal INTEGER NOT NULL CHECK(ordinal > 0),
                execution_facts_version INTEGER NOT NULL DEFAULT 2
                    CHECK(execution_facts_version IN (1, 2)),
                created_at TEXT NOT NULL,
                PRIMARY KEY(task_id, run_id),
                FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE TABLE canonical_task_items (
                id TEXT PRIMARY KEY, task_id TEXT NOT NULL, run_id TEXT NOT NULL,
                sequence INTEGER NOT NULL CHECK(sequence > 0),
                kind TEXT NOT NULL CHECK(kind IN (
                    'instruction', 'provider_generation', 'artifact_draft',
                    'review_checkpoint', 'artifact_materialized'
                )),
                status TEXT NOT NULL, summary_code TEXT NOT NULL,
                payload_digest TEXT NOT NULL, created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL, UNIQUE(task_id, sequence),
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             );",
        )
        .unwrap();
        let now = Utc::now().to_rfc3339();
        let task_id = stable_id("task", &["report", "conversation-v2"]);
        conn.execute(
            "INSERT INTO canonical_tasks VALUES (?1, 'conversation-v2', 'report', ?2,
                                                   'running', ?3, ?3)",
            params![task_id, digest_of("legacy outcome"), now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO canonical_task_runs VALUES (?1, 'run-v2', 'run-v2', 1, 2, ?2)",
            params![task_id, now],
        )
        .unwrap();
        let instruction_id = stable_id("item", &["instruction", &task_id, "run-v2"]);
        conn.execute(
            "INSERT INTO canonical_task_items VALUES (
                ?1, ?2, 'run-v2', 1, 'instruction', 'completed',
                'report_instruction_bound', ?3, ?4, ?4
             )",
            params![instruction_id, task_id, digest_of("legacy outcome"), now],
        )
        .unwrap();
        drop(conn);

        let store = CanonicalTaskRuntimeStore::new(&path).unwrap();
        let snapshots = store.list_task_snapshots(100).unwrap();
        assert_eq!(snapshots[0].runs[0].execution_facts_version, 2);
        assert_eq!(
            snapshots[0].items[0].kind,
            CanonicalTaskItemKind::Instruction
        );
        let new_run = prepare_test_artifact(&store, "conversation-v2", "run-v3");
        let snapshots = store.list_task_snapshots(100).unwrap();
        assert_eq!(snapshots[0].runs[1].execution_facts_version, 5);
        assert_eq!(new_run.task_id, task_id);
    }

    #[test]
    fn v3_runtime_migrates_and_preserves_tool_execution_facts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("task_runtime_v3.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE canonical_task_runtime_metadata (
                key TEXT PRIMARY KEY, value TEXT NOT NULL
             ) WITHOUT ROWID;
             INSERT INTO canonical_task_runtime_metadata VALUES ('schema_version', '3');
             CREATE TABLE canonical_tasks (
                id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL UNIQUE,
                task_kind TEXT NOT NULL CHECK(task_kind = 'report'),
                initial_outcome_digest TEXT NOT NULL, status TEXT NOT NULL,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL
             );
             CREATE TABLE canonical_task_runs (
                task_id TEXT NOT NULL, run_id TEXT NOT NULL UNIQUE,
                execution_session_id TEXT NOT NULL,
                ordinal INTEGER NOT NULL CHECK(ordinal > 0),
                execution_facts_version INTEGER NOT NULL DEFAULT 3
                    CHECK(execution_facts_version IN (1, 2, 3)),
                created_at TEXT NOT NULL,
                PRIMARY KEY(task_id, run_id),
                FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE TABLE canonical_task_items (
                id TEXT PRIMARY KEY, task_id TEXT NOT NULL, run_id TEXT NOT NULL,
                sequence INTEGER NOT NULL CHECK(sequence > 0),
                kind TEXT NOT NULL CHECK(kind IN (
                    'instruction', 'tool_call', 'observation',
                    'provider_generation', 'artifact_draft',
                    'review_checkpoint', 'artifact_materialized'
                )),
                status TEXT NOT NULL, summary_code TEXT NOT NULL,
                payload_digest TEXT NOT NULL, created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL, UNIQUE(task_id, sequence),
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             );
             CREATE INDEX idx_canonical_task_items_run
                ON canonical_task_items(run_id, sequence);",
        )
        .unwrap();
        let now = Utc::now().to_rfc3339();
        let task_id = stable_id("task", &["report", "conversation-v3"]);
        conn.execute(
            "INSERT INTO canonical_tasks VALUES (?1, 'conversation-v3', 'report', ?2,
                                                   'running', ?3, ?3)",
            params![task_id, digest_of("legacy v3 outcome"), now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO canonical_task_runs VALUES (?1, 'run-v3', 'run-v3', 1, 3, ?2)",
            params![task_id, now],
        )
        .unwrap();
        let action_id = "action-v3";
        let observation_id = "observation-v3";
        let instruction_id = stable_id("item", &["instruction", &task_id, "run-v3"]);
        let tool_id = stable_id("item", &["tool_call", &task_id, "run-v3", action_id]);
        let observation_item_id = stable_id(
            "item",
            &["observation", &task_id, "run-v3", action_id, observation_id],
        );
        let provider_id = stable_id(
            "item",
            &[
                "provider_generation",
                &task_id,
                "run-v3",
                "provider-request-v3",
            ],
        );
        for (sequence, id, kind, summary, payload) in [
            (
                1,
                instruction_id,
                "instruction",
                "report_instruction_bound",
                digest_of("legacy v3 outcome"),
            ),
            (
                2,
                tool_id,
                "tool_call",
                "report_governed_tool_call_completed",
                digest_of("legacy v3 tool"),
            ),
            (
                3,
                observation_item_id,
                "observation",
                "report_governed_observation_bound",
                digest_of("legacy v3 observation"),
            ),
            (
                4,
                provider_id,
                "provider_generation",
                "report_provider_generation_completed",
                digest_of("legacy v3 provider"),
            ),
        ] {
            conn.execute(
                "INSERT INTO canonical_task_items VALUES (
                    ?1, ?2, 'run-v3', ?3, ?4, 'completed', ?5, ?6, ?7, ?7
                 )",
                params![id, task_id, sequence, kind, summary, payload, now],
            )
            .unwrap();
        }
        drop(conn);

        let store = CanonicalTaskRuntimeStore::new(&path).unwrap();
        let snapshots = store.list_task_snapshots(100).unwrap();
        assert_eq!(snapshots[0].runs[0].execution_facts_version, 3);
        assert_eq!(snapshots[0].items.len(), 4);
        assert_eq!(
            snapshots[0]
                .items
                .iter()
                .map(|item| item.kind)
                .collect::<Vec<_>>(),
            vec![
                CanonicalTaskItemKind::Instruction,
                CanonicalTaskItemKind::ToolCall,
                CanonicalTaskItemKind::Observation,
                CanonicalTaskItemKind::ProviderGeneration,
            ]
        );
        let new_run = prepare_test_artifact(&store, "conversation-v3", "run-v4");
        let snapshots = store.list_task_snapshots(100).unwrap();
        assert_eq!(snapshots[0].runs[1].execution_facts_version, 5);
        assert_eq!(new_run.task_id, task_id);
    }

    #[test]
    fn read_only_recovery_reopens_existing_runtime_without_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("task_runtime.db");
        let prepared = {
            let store = CanonicalTaskRuntimeStore::new(&path).unwrap();
            prepare_test_artifact(&store, "conversation-1", "run-1")
        };
        let recovered = CanonicalTaskRuntimeStore::open_read_only_existing(&path).unwrap();
        assert_eq!(
            recovered
                .load_artifact(&prepared.artifact_id)
                .unwrap()
                .unwrap()
                .id,
            prepared.artifact_id
        );
        let outcome_digest = digest_of("report outcome");
        let content_digest = digest_of("# Report");
        assert!(recovered
            .prepare_report_artifact(ReportArtifactDraftInput {
                conversation_id: "conversation-1",
                execution_session_id: "run-2",
                run_id: "run-2",
                outcome_digest: &outcome_digest,
                plan_digest: &digest_of("report plan run 2"),
                provider_request_id: "provider-request-run-2",
                provider_receipt_digest: &digest_of("provider-receipt-run-2"),
                tool_observations: &[],
                target_reference: "/tmp/openlife/report.md",
                content_digest: &content_digest,
                media_type: "text/markdown; charset=utf-8",
            })
            .is_err());
    }

    #[test]
    fn task_snapshot_joins_runs_items_and_current_artifact_versions() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let first = prepare_test_artifact(&store, "conversation-1", "run-1");
        let second = prepare_test_artifact(&store, "conversation-1", "run-2");
        store
            .bind_report_review(&first.artifact_id, "proposal-1")
            .unwrap();
        store
            .bind_report_review(&second.artifact_id, "proposal-2")
            .unwrap();

        let snapshots = store.list_task_snapshots(100).unwrap();
        assert_eq!(snapshots.len(), 1);
        let snapshot = &snapshots[0];
        assert_eq!(snapshot.task.id, first.task_id);
        assert_eq!(snapshot.runs.len(), 2);
        assert_eq!(snapshot.items.len(), 10);
        assert_eq!(snapshot.artifacts.len(), 2);
        assert!(snapshot.artifacts.iter().all(|artifact| {
            artifact.artifact.current_version == artifact.current_version.version
                && artifact.artifact.id == artifact.current_version.artifact_id
                && artifact.artifact.content_digest == artifact.current_version.content_digest
        }));
    }
}
