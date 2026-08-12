//! Canonical Task/Item/Artifact metadata for the general Agent runtime.
//!
//! This store is introduced vertically on the generated-report path. It owns
//! stable Task identity, Run membership, typed Items, and Artifact versions.
//! Existing AgentRun persistence remains the execution/receipt owner while the
//! migration proceeds; this module does not copy AgentRun status or bodies.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

const TASK_RUNTIME_SCHEMA_VERSION: i64 = 2;

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
    ProviderGeneration,
    ArtifactDraft,
    ReviewCheckpoint,
    ArtifactMaterialized,
}

impl CanonicalTaskItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Instruction => "instruction",
            Self::ProviderGeneration => "provider_generation",
            Self::ArtifactDraft => "artifact_draft",
            Self::ReviewCheckpoint => "review_checkpoint",
            Self::ArtifactMaterialized => "artifact_materialized",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "instruction" => Ok(Self::Instruction),
            "provider_generation" => Ok(Self::ProviderGeneration),
            "artifact_draft" => Ok(Self::ArtifactDraft),
            "review_checkpoint" => Ok(Self::ReviewCheckpoint),
            "artifact_materialized" => Ok(Self::ArtifactMaterialized),
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
    pub created_at: DateTime<Utc>,
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
    pub provider_request_id: &'a str,
    pub provider_receipt_digest: &'a str,
    pub target_reference: &'a str,
    pub content_digest: &'a str,
    pub media_type: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedReportArtifact {
    pub task_id: String,
    pub artifact_draft_item_id: String,
    pub artifact_id: String,
    pub version: u64,
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
                task_kind TEXT NOT NULL CHECK(task_kind = 'report'),
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
                execution_facts_version INTEGER NOT NULL DEFAULT 2
                    CHECK(execution_facts_version IN (1, 2)),
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
             VALUES ('schema_version', '2')
             ON CONFLICT(key) DO NOTHING;",
        )?;
        if Self::schema_version(&conn)? == 1 {
            Self::migrate_v1_to_v2(&mut conn)?;
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

    fn validate_schema(conn: &Connection) -> Result<()> {
        let version = Self::schema_version(conn)?;
        if version != TASK_RUNTIME_SCHEMA_VERSION {
            anyhow::bail!("canonical_task_runtime_schema_version_unsupported:{version}");
        }
        Ok(())
    }

    pub fn prepare_report_artifact(
        &self,
        input: ReportArtifactDraftInput<'_>,
    ) -> Result<PreparedReportArtifact> {
        validate_nonempty("conversation_id", input.conversation_id, 512)?;
        validate_nonempty("execution_session_id", input.execution_session_id, 512)?;
        validate_nonempty("run_id", input.run_id, 512)?;
        validate_digest("outcome_digest", input.outcome_digest)?;
        validate_nonempty("provider_request_id", input.provider_request_id, 512)?;
        validate_digest("provider_receipt_digest", input.provider_receipt_digest)?;
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
                    execution_facts_version, created_at
                 ) VALUES (?1, ?2, ?3, ?4, 2, ?5)",
                params![
                    task_id,
                    input.run_id,
                    input.execution_session_id,
                    ordinal,
                    now_text
                ],
            )?;
            2
        };

        if execution_facts_version == 2 {
            if !is_new_run {
                let existing_instruction_id = tx
                    .query_row(
                        "SELECT id FROM canonical_task_items
                         WHERE task_id = ?1 AND run_id = ?2 AND kind = 'instruction'",
                        params![task_id, input.run_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                if existing_instruction_id.as_deref() != Some(instruction_item_id.as_str()) {
                    anyhow::bail!("canonical_report_instruction_item_missing_or_conflicting");
                }
                let existing_generation_id = tx
                    .query_row(
                        "SELECT id FROM canonical_task_items
                         WHERE task_id = ?1 AND run_id = ?2
                           AND kind = 'provider_generation'",
                        params![task_id, input.run_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                if existing_generation_id.as_deref() != Some(provider_generation_item_id.as_str()) {
                    anyhow::bail!(
                        "canonical_report_provider_generation_item_missing_or_conflicting"
                    );
                }
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
                    item_id: &provider_generation_item_id,
                    task_id: &task_id,
                    run_id: input.run_id,
                    kind: CanonicalTaskItemKind::ProviderGeneration,
                    summary_code: "report_provider_generation_completed",
                    payload_digest: input.provider_receipt_digest,
                    now: &now_text,
                },
            )?;
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
            if execution_facts_version != 2 {
                anyhow::bail!("canonical_report_legacy_run_cannot_add_artifact");
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
               AND (observed_content_digest IS NULL OR observed_content_digest = ?4)",
            params![
                artifact_id,
                expected_digest,
                materialized_reference,
                observed_content_digest,
                now
            ],
        )?;
        if version_changed != 1 {
            anyhow::bail!("canonical_report_artifact_version_confirm_cas_failed");
        }
        let artifact_changed = tx.execute(
            "UPDATE canonical_artifacts
             SET status = 'materialized', materialized_reference = ?2, updated_at = ?3
             WHERE id = ?1 AND status IN ('waiting_review', 'materialized')",
            params![artifact_id, materialized_reference, now],
        )?;
        if artifact_changed != 1 {
            anyhow::bail!("canonical_report_artifact_confirm_cas_failed");
        }
        let checkpoint_item_id = stable_id("item", &["review_checkpoint", &artifact_id]);
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'completed', summary_code = 'report_artifact_review_accepted',
                 updated_at = ?2
             WHERE id = ?1 AND kind = 'review_checkpoint'
               AND status IN ('waiting', 'completed')",
            params![checkpoint_item_id, now],
        )?;
        let materialized_item_id = stable_id("item", &["artifact_materialized", &artifact_id]);
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM canonical_task_items
             WHERE task_id = ?1",
            [&task_id],
            |row| row.get(0),
        )?;
        let run_id: String = tx.query_row(
            "SELECT run_id FROM canonical_task_items WHERE id = ?1",
            [&source_item_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO canonical_task_items (
                id, task_id, run_id, sequence, kind, status, summary_code,
                payload_digest, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'artifact_materialized', 'completed',
                       'report_artifact_materialized', ?5, ?6, ?6)
             ON CONFLICT(id) DO NOTHING",
            params![
                materialized_item_id,
                task_id,
                run_id,
                sequence,
                sha256_text(&format!(
                    "{}\0{}",
                    materialized_reference, observed_content_digest
                )),
                now
            ],
        )?;
        let remaining: i64 = tx.query_row(
            "SELECT COUNT(*) FROM canonical_artifacts
             WHERE task_id = ?1 AND status != 'materialized'",
            [&task_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![
                task_id,
                if remaining == 0 {
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
                            execution_facts_version, created_at
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
    Ok(CanonicalTaskRunRecord {
        task_id: row.get(0)?,
        run_id: row.get(1)?,
        execution_session_id: row.get(2)?,
        ordinal,
        execution_facts_version,
        created_at: parse_timestamp(row.get(5)?, "task_run_created_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, error.into())
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
                provider_request_id: &format!("provider-request-{run}"),
                provider_receipt_digest: &digest_of(&format!("provider-receipt-{run}")),
                target_reference: "/tmp/openlife/report.md",
                content_digest: &content_digest,
                media_type: "text/markdown; charset=utf-8",
            })
            .unwrap()
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
            provider_request_id,
            provider_receipt_digest,
            target_reference,
            content_digest,
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
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].kind, CanonicalTaskItemKind::Instruction);
        assert_eq!(items[1].kind, CanonicalTaskItemKind::ProviderGeneration);
        assert_eq!(items[2].kind, CanonicalTaskItemKind::ArtifactDraft);
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
        assert_eq!(store.list_items(&first.task_id).unwrap().len(), 4);
        assert_eq!(
            store
                .load_artifact(&first.artifact_id)
                .unwrap()
                .unwrap()
                .status,
            CanonicalArtifactStatus::WaitingReview
        );
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
        assert_eq!(items.len(), 6);
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
        assert_eq!(items.len(), 4);
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
        assert_eq!(migrated[0].runs[1].execution_facts_version, 2);
        assert_eq!(migrated[0].items.len(), 4);
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
                provider_request_id: "provider-request-run-2",
                provider_receipt_digest: &digest_of("provider-receipt-run-2"),
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
        assert_eq!(snapshot.items.len(), 8);
        assert_eq!(snapshot.artifacts.len(), 2);
        assert!(snapshot.artifacts.iter().all(|artifact| {
            artifact.artifact.current_version == artifact.current_version.version
                && artifact.artifact.id == artifact.current_version.artifact_id
                && artifact.artifact.content_digest == artifact.current_version.content_digest
        }));
    }
}
