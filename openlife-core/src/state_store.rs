//! Canonical transient-state owner for the ADR 0015 command lane.
//!
//! State content is stored once in this SQLite owner. Operation receipts and
//! the shared transactional outbox contain only identity, digest, lifecycle,
//! and projection state; they never copy the task title or user message body.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;
use uuid::{Uuid, Version};

const STATE_STORE_SCHEMA_VERSION: i64 = 1;
const STATE_ASSET_AGGREGATE_KIND: &str = "transient_state_asset";
pub const LIFEMODEL_YAML_PROJECTION_TARGET: &str = "lifemodel_yaml_compat_v1";
const PROJECTION_TARGETS: &[&str] = &[LIFEMODEL_YAML_PROJECTION_TARGET];
const MAX_TASK_TITLE_CHARS: usize = 512;
const MAX_SOURCE_MESSAGE_REF_CHARS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateAssetKind {
    DailyTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DailyTaskStatus {
    Pending,
    Completed,
    Tombstoned,
}

impl DailyTaskStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Tombstoned => "tombstoned",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateMutationKind {
    Create,
    Complete,
    Undo,
    Expire,
}

impl StateMutationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Complete => "complete",
            Self::Undo => "undo",
            Self::Expire => "expire",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateRisk {
    Low,
}

impl StateRisk {
    fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateSensitivity {
    Internal,
}

impl StateSensitivity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateSourceKind {
    CurrentAuthenticatedUserMessage,
    SystemExpiry,
}

impl StateSourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::CurrentAuthenticatedUserMessage => "current_authenticated_user_message",
            Self::SystemExpiry => "system_expiry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatePrivacyClass {
    Private,
}

impl StatePrivacyClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateProjectionStatus {
    Pending,
    Degraded,
    Applied,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateAsset {
    pub asset_id: String,
    pub kind: StateAssetKind,
    pub version: u64,
    pub title: String,
    pub status: DailyTaskStatus,
    pub due_at: Option<DateTime<Utc>>,
    pub source_message_ref: String,
    pub risk: StateRisk,
    pub sensitivity: StateSensitivity,
    pub source_kind: StateSourceKind,
    pub confidence: f32,
    pub privacy_class: StatePrivacyClass,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub tombstoned_at: Option<DateTime<Utc>>,
    pub tombstone_reason: Option<StateMutationKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateExecutionReceipt {
    pub schema: String,
    pub receipt_id: String,
    pub operation_id: String,
    pub payload_digest: String,
    pub asset_id: String,
    pub asset_version: u64,
    pub mutation_kind: StateMutationKind,
    pub canonical_status: String,
    pub projection_status: StateProjectionStatus,
    pub outbox_event_id: String,
    pub tombstone_id: Option<String>,
    pub committed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct CreateDailyTaskCommand {
    pub operation_id: String,
    pub source_message_ref: String,
    pub title: String,
    pub due_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub risk: StateRisk,
    pub sensitivity: StateSensitivity,
    pub source_kind: StateSourceKind,
    pub confidence: f32,
    pub privacy_class: StatePrivacyClass,
}

#[derive(Debug, Clone)]
pub struct TransitionDailyTaskCommand {
    pub operation_id: String,
    pub source_message_ref: String,
    pub asset_id: String,
    pub expected_version: u64,
    pub mutation_kind: StateMutationKind,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct StateStore {
    db_path: Option<PathBuf>,
    conn: Arc<Mutex<Connection>>,
}

impl StateStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create StateStore directory {}", parent.display()))?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("open canonical StateStore {}", db_path.display()))?;
        configure_connection(&conn)?;
        let store = Self {
            db_path: Some(db_path),
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory StateStore")?;
        configure_connection(&conn)?;
        let store = Self {
            db_path: None,
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn db_path(&self) -> Option<&Path> {
        self.db_path.as_deref()
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.lock_connection()?;
        crate::persistence_outbox::init_schema(&conn)?;
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS state_store_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS state_assets (
                asset_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL CHECK(kind IN ('daily_task')),
                version INTEGER NOT NULL CHECK(version > 0),
                title TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('pending', 'completed', 'tombstoned')),
                due_at TEXT,
                source_message_ref TEXT NOT NULL,
                risk TEXT NOT NULL CHECK(risk IN ('low')),
                sensitivity TEXT NOT NULL CHECK(sensitivity IN ('internal')),
                source_kind TEXT NOT NULL CHECK(source_kind IN (
                    'current_authenticated_user_message', 'system_expiry'
                )),
                confidence REAL NOT NULL CHECK(confidence >= 0 AND confidence <= 1),
                privacy_class TEXT NOT NULL CHECK(privacy_class IN ('private')),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                tombstoned_at TEXT,
                tombstone_reason TEXT CHECK(tombstone_reason IS NULL OR tombstone_reason IN ('undo', 'expire'))
             );
             CREATE INDEX IF NOT EXISTS idx_state_assets_daily_status_expiry
             ON state_assets(kind, status, expires_at, updated_at);
             CREATE TABLE IF NOT EXISTS state_asset_versions (
                asset_id TEXT NOT NULL,
                version INTEGER NOT NULL CHECK(version > 0),
                operation_id TEXT NOT NULL UNIQUE,
                payload_digest TEXT NOT NULL,
                mutation_kind TEXT NOT NULL CHECK(mutation_kind IN ('create', 'complete', 'undo', 'expire')),
                status TEXT NOT NULL CHECK(status IN ('pending', 'completed', 'tombstoned')),
                title TEXT NOT NULL,
                due_at TEXT,
                source_message_ref TEXT NOT NULL,
                source_kind TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                tombstone_reason TEXT,
                PRIMARY KEY(asset_id, version),
                FOREIGN KEY(asset_id) REFERENCES state_assets(asset_id)
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS state_operations (
                operation_id TEXT PRIMARY KEY,
                payload_digest TEXT NOT NULL,
                receipt_id TEXT NOT NULL UNIQUE,
                asset_id TEXT NOT NULL,
                asset_version INTEGER NOT NULL CHECK(asset_version > 0),
                mutation_kind TEXT NOT NULL CHECK(mutation_kind IN ('create', 'complete', 'undo', 'expire')),
                outbox_event_id TEXT NOT NULL UNIQUE,
                committed_at TEXT NOT NULL,
                FOREIGN KEY(asset_id) REFERENCES state_assets(asset_id),
                FOREIGN KEY(outbox_event_id) REFERENCES canonical_outbox_events(event_id)
             ) WITHOUT ROWID;",
        )?;
        let existing_version = tx
            .query_row(
                "SELECT value FROM state_store_metadata WHERE key = 'schema_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match existing_version.as_deref() {
            None => {
                tx.execute(
                    "INSERT INTO state_store_metadata (key, value) VALUES ('schema_version', ?1)",
                    [STATE_STORE_SCHEMA_VERSION.to_string()],
                )?;
            }
            Some("1") => {}
            Some(other) => anyhow::bail!("state_store_schema_version_unsupported:{other}"),
        }
        tx.commit()?;
        Ok(())
    }

    pub fn create_daily_task(
        &self,
        command: CreateDailyTaskCommand,
    ) -> Result<StateExecutionReceipt> {
        self.create_daily_task_guarded(command, || Result::<()>::Ok(()))
    }

    pub fn create_daily_task_guarded<F>(
        &self,
        command: CreateDailyTaskCommand,
        before_commit: F,
    ) -> Result<StateExecutionReceipt>
    where
        F: FnOnce() -> Result<()>,
    {
        let prepared = PreparedCreate::validate(command)?;
        let mut conn = self.lock_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(receipt) =
            replay_operation(&tx, &prepared.operation_id, &prepared.payload_digest)?
        {
            tx.rollback()?;
            return Ok(receipt);
        }

        let asset_id = Uuid::new_v4().hyphenated().to_string();
        let receipt_id = format!("state_receipt:{}", Uuid::new_v4());
        tx.execute(
            "INSERT INTO state_assets (
                asset_id, kind, version, title, status, due_at,
                source_message_ref, risk, sensitivity, source_kind, confidence,
                privacy_class, created_at, updated_at, expires_at,
                tombstoned_at, tombstone_reason
             ) VALUES (?1, 'daily_task', 1, ?2, 'pending', ?3, ?4, ?5, ?6,
                       ?7, ?8, ?9, ?10, ?10, ?11, NULL, NULL)",
            params![
                asset_id,
                prepared.title,
                prepared.due_at.map(|value| value.to_rfc3339()),
                prepared.source_message_ref,
                prepared.risk.as_str(),
                prepared.sensitivity.as_str(),
                prepared.source_kind.as_str(),
                f64::from(prepared.confidence),
                prepared.privacy_class.as_str(),
                prepared.created_at.to_rfc3339(),
                prepared.expires_at.to_rfc3339(),
            ],
        )?;
        tx.execute(
            "INSERT INTO state_asset_versions (
                asset_id, version, operation_id, payload_digest, mutation_kind,
                status, title, due_at, source_message_ref, source_kind,
                created_at, expires_at, tombstone_reason
             ) VALUES (?1, 1, ?2, ?3, 'create', 'pending', ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
            params![
                asset_id,
                prepared.operation_id,
                prepared.payload_digest,
                prepared.title,
                prepared.due_at.map(|value| value.to_rfc3339()),
                prepared.source_message_ref,
                prepared.source_kind.as_str(),
                prepared.created_at.to_rfc3339(),
                prepared.expires_at.to_rfc3339(),
            ],
        )?;
        let outbox = crate::persistence_outbox::enqueue_mutation(
            &tx,
            STATE_ASSET_AGGREGATE_KIND,
            &asset_id,
            StateMutationKind::Create.as_str(),
            &prepared.payload_digest,
            PROJECTION_TARGETS,
        )?;
        insert_operation(
            &tx,
            &prepared.operation_id,
            &prepared.payload_digest,
            &receipt_id,
            &asset_id,
            1,
            StateMutationKind::Create,
            &outbox.event_id,
            prepared.created_at,
        )?;
        before_commit()?;
        tx.commit()?;
        drop(conn);
        self.receipt_for_operation(&prepared.operation_id, false)?
            .context("state_create_receipt_missing_after_commit")
    }

    pub fn complete_daily_task(
        &self,
        command: TransitionDailyTaskCommand,
    ) -> Result<StateExecutionReceipt> {
        if command.mutation_kind != StateMutationKind::Complete {
            anyhow::bail!("state_complete_mutation_kind_invalid");
        }
        self.transition_daily_task_guarded(command, || Result::<()>::Ok(()))
    }

    pub fn undo_daily_task(
        &self,
        command: TransitionDailyTaskCommand,
    ) -> Result<StateExecutionReceipt> {
        if command.mutation_kind != StateMutationKind::Undo {
            anyhow::bail!("state_undo_mutation_kind_invalid");
        }
        self.transition_daily_task_guarded(command, || Result::<()>::Ok(()))
    }

    pub fn transition_daily_task_guarded<F>(
        &self,
        command: TransitionDailyTaskCommand,
        before_commit: F,
    ) -> Result<StateExecutionReceipt>
    where
        F: FnOnce() -> Result<()>,
    {
        if !matches!(
            command.mutation_kind,
            StateMutationKind::Complete | StateMutationKind::Undo
        ) {
            anyhow::bail!("state_user_transition_kind_invalid");
        }
        let prepared = PreparedTransition::validate(
            command,
            StateSourceKind::CurrentAuthenticatedUserMessage,
        )?;
        self.commit_transition(prepared, before_commit)
    }

    pub fn expire_due(&self, now: DateTime<Utc>) -> Result<Vec<StateExecutionReceipt>> {
        let due = {
            let conn = self.lock_connection()?;
            let mut statement = conn.prepare(
                "SELECT asset_id, version FROM state_assets
                 WHERE status != 'tombstoned' AND expires_at <= ?1
                 ORDER BY expires_at ASC, asset_id ASC",
            )?;
            let rows = statement
                .query_map([now.to_rfc3339()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        let mut receipts = Vec::with_capacity(due.len());
        for (asset_id, version) in due {
            let prepared = PreparedTransition::validate(
                TransitionDailyTaskCommand {
                    operation_id: Uuid::new_v4().hyphenated().to_string(),
                    source_message_ref: format!("system_expiry:{asset_id}"),
                    asset_id,
                    expected_version: u64::try_from(version)
                        .context("state_expiry_version_invalid")?,
                    mutation_kind: StateMutationKind::Expire,
                    occurred_at: now,
                },
                StateSourceKind::SystemExpiry,
            )?;
            match self.commit_transition(prepared, || Result::<()>::Ok(())) {
                Ok(receipt) => receipts.push(receipt),
                Err(error) if error.to_string().contains("state_asset_version_conflict") => {}
                Err(error) => return Err(error),
            }
        }
        Ok(receipts)
    }

    fn commit_transition<F>(
        &self,
        prepared: PreparedTransition,
        before_commit: F,
    ) -> Result<StateExecutionReceipt>
    where
        F: FnOnce() -> Result<()>,
    {
        let mut conn = self.lock_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(receipt) =
            replay_operation(&tx, &prepared.operation_id, &prepared.payload_digest)?
        {
            tx.rollback()?;
            return Ok(receipt);
        }
        let current = load_asset(&tx, &prepared.asset_id)?
            .ok_or_else(|| anyhow::anyhow!("state_asset_not_found"))?;
        if current.version != prepared.expected_version {
            anyhow::bail!("state_asset_version_conflict");
        }
        if current.status == DailyTaskStatus::Tombstoned {
            anyhow::bail!("state_asset_tombstoned");
        }
        if prepared.occurred_at < current.created_at {
            anyhow::bail!("state_transition_precedes_creation");
        }
        if prepared.mutation_kind == StateMutationKind::Complete
            && current.status == DailyTaskStatus::Completed
        {
            anyhow::bail!("state_asset_already_completed");
        }
        let next_version = current
            .version
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("state_asset_version_overflow"))?;
        let (next_status, tombstoned_at, tombstone_reason) = match prepared.mutation_kind {
            StateMutationKind::Complete => (DailyTaskStatus::Completed, None, None),
            StateMutationKind::Undo | StateMutationKind::Expire => (
                DailyTaskStatus::Tombstoned,
                Some(prepared.occurred_at),
                Some(prepared.mutation_kind),
            ),
            StateMutationKind::Create => anyhow::bail!("state_transition_create_invalid"),
        };
        let changed = tx.execute(
            "UPDATE state_assets
             SET version = ?3, status = ?4, updated_at = ?5,
                 source_message_ref = ?6, source_kind = ?7,
                 tombstoned_at = ?8, tombstone_reason = ?9
             WHERE asset_id = ?1 AND version = ?2 AND status != 'tombstoned'",
            params![
                prepared.asset_id,
                i64::try_from(prepared.expected_version)?,
                i64::try_from(next_version)?,
                next_status.as_str(),
                prepared.occurred_at.to_rfc3339(),
                prepared.source_message_ref,
                prepared.source_kind.as_str(),
                tombstoned_at.map(|value| value.to_rfc3339()),
                tombstone_reason.map(StateMutationKind::as_str),
            ],
        )?;
        if changed != 1 {
            anyhow::bail!("state_asset_version_conflict");
        }
        tx.execute(
            "INSERT INTO state_asset_versions (
                asset_id, version, operation_id, payload_digest, mutation_kind,
                status, title, due_at, source_message_ref, source_kind,
                created_at, expires_at, tombstone_reason
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                prepared.asset_id,
                i64::try_from(next_version)?,
                prepared.operation_id,
                prepared.payload_digest,
                prepared.mutation_kind.as_str(),
                next_status.as_str(),
                current.title,
                current.due_at.map(|value| value.to_rfc3339()),
                prepared.source_message_ref,
                prepared.source_kind.as_str(),
                current.created_at.to_rfc3339(),
                current.expires_at.to_rfc3339(),
                tombstone_reason.map(StateMutationKind::as_str),
            ],
        )?;
        let outbox = if matches!(
            prepared.mutation_kind,
            StateMutationKind::Undo | StateMutationKind::Expire
        ) {
            crate::persistence_outbox::enqueue_tombstone(
                &tx,
                STATE_ASSET_AGGREGATE_KIND,
                &prepared.asset_id,
                Some(prepared.mutation_kind.as_str()),
                PROJECTION_TARGETS,
            )?
        } else {
            crate::persistence_outbox::enqueue_mutation(
                &tx,
                STATE_ASSET_AGGREGATE_KIND,
                &prepared.asset_id,
                prepared.mutation_kind.as_str(),
                &prepared.payload_digest,
                PROJECTION_TARGETS,
            )?
        };
        let receipt_id = format!("state_receipt:{}", Uuid::new_v4());
        insert_operation(
            &tx,
            &prepared.operation_id,
            &prepared.payload_digest,
            &receipt_id,
            &prepared.asset_id,
            next_version,
            prepared.mutation_kind,
            &outbox.event_id,
            prepared.occurred_at,
        )?;
        before_commit()?;
        tx.commit()?;
        drop(conn);
        self.receipt_for_operation(&prepared.operation_id, false)?
            .context("state_transition_receipt_missing_after_commit")
    }

    pub fn get_asset(&self, asset_id: &str) -> Result<Option<StateAsset>> {
        validate_bounded_ref("state_asset_id", asset_id)?;
        let conn = self.lock_connection()?;
        load_asset(&conn, asset_id)
    }

    pub fn list_daily_tasks(&self, include_tombstoned: bool) -> Result<Vec<StateAsset>> {
        let conn = self.lock_connection()?;
        let mut statement = conn.prepare(if include_tombstoned {
            "SELECT asset_id, kind, version, title, status, due_at,
                    source_message_ref, risk, sensitivity, source_kind, confidence,
                    privacy_class, created_at, updated_at, expires_at,
                    tombstoned_at, tombstone_reason
             FROM state_assets WHERE kind = 'daily_task'
             ORDER BY created_at ASC, asset_id ASC"
        } else {
            "SELECT asset_id, kind, version, title, status, due_at,
                    source_message_ref, risk, sensitivity, source_kind, confidence,
                    privacy_class, created_at, updated_at, expires_at,
                    tombstoned_at, tombstone_reason
             FROM state_assets WHERE kind = 'daily_task' AND status != 'tombstoned'
             ORDER BY created_at ASC, asset_id ASC"
        })?;
        let rows = statement
            .query_map([], state_asset_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn receipt_for_operation(
        &self,
        operation_id: &str,
        replayed: bool,
    ) -> Result<Option<StateExecutionReceipt>> {
        validate_uuid_v4("state_operation_id", operation_id)?;
        let conn = self.lock_connection()?;
        operation_receipt(&conn, operation_id, replayed)
    }

    pub fn list_replayable_projection_deliveries(
        &self,
        limit: usize,
    ) -> Result<Vec<crate::persistence_outbox::ProjectionDelivery>> {
        let conn = self.lock_connection()?;
        crate::persistence_outbox::list_replayable_deliveries(&conn, limit)
    }

    pub fn mark_projection_applied(&self, event_id: &str) -> Result<()> {
        let conn = self.lock_connection()?;
        crate::persistence_outbox::mark_delivery_applied(
            &conn,
            event_id,
            LIFEMODEL_YAML_PROJECTION_TARGET,
        )
    }

    pub fn mark_projection_degraded(&self, event_id: &str, error_code: &str) -> Result<()> {
        let conn = self.lock_connection()?;
        crate::persistence_outbox::mark_delivery_degraded(
            &conn,
            event_id,
            LIFEMODEL_YAML_PROJECTION_TARGET,
            error_code,
        )
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|error| anyhow::anyhow!("StateStore mutex poisoned: {error}"))
    }
}

struct PreparedCreate {
    operation_id: String,
    source_message_ref: String,
    title: String,
    due_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    risk: StateRisk,
    sensitivity: StateSensitivity,
    source_kind: StateSourceKind,
    confidence: f32,
    privacy_class: StatePrivacyClass,
    payload_digest: String,
}

impl PreparedCreate {
    fn validate(command: CreateDailyTaskCommand) -> Result<Self> {
        validate_uuid_v4("state_operation_id", &command.operation_id)?;
        validate_bounded_ref("state_source_message_ref", &command.source_message_ref)?;
        if command.source_kind != StateSourceKind::CurrentAuthenticatedUserMessage {
            anyhow::bail!("state_create_source_not_current_user");
        }
        let title = command.title.trim().to_string();
        if title.is_empty()
            || title.chars().count() > MAX_TASK_TITLE_CHARS
            || title.chars().any(char::is_control)
        {
            anyhow::bail!("state_daily_task_title_invalid");
        }
        let ttl = command.expires_at - command.created_at;
        if ttl < Duration::hours(24) || ttl > Duration::days(7) {
            anyhow::bail!("state_asset_ttl_out_of_range");
        }
        if command
            .due_at
            .is_some_and(|due_at| due_at < command.created_at || due_at > command.expires_at)
        {
            anyhow::bail!("state_daily_task_due_at_out_of_range");
        }
        if !command.confidence.is_finite() || !(0.0..=1.0).contains(&command.confidence) {
            anyhow::bail!("state_asset_confidence_invalid");
        }
        let payload_digest = digest_json(&serde_json::json!({
            "schema": "openlife.state-create-payload.v1",
            "operationId": command.operation_id,
            "sourceMessageRef": command.source_message_ref,
            "title": title,
            "dueAt": command.due_at.map(|value| value.to_rfc3339()),
            "createdAt": command.created_at.to_rfc3339(),
            "expiresAt": command.expires_at.to_rfc3339(),
            "risk": command.risk,
            "sensitivity": command.sensitivity,
            "sourceKind": command.source_kind,
            "confidence": command.confidence,
            "privacyClass": command.privacy_class,
        }))?;
        Ok(Self {
            operation_id: command.operation_id,
            source_message_ref: command.source_message_ref,
            title,
            due_at: command.due_at,
            created_at: command.created_at,
            expires_at: command.expires_at,
            risk: command.risk,
            sensitivity: command.sensitivity,
            source_kind: command.source_kind,
            confidence: command.confidence,
            privacy_class: command.privacy_class,
            payload_digest,
        })
    }
}

struct PreparedTransition {
    operation_id: String,
    source_message_ref: String,
    asset_id: String,
    expected_version: u64,
    mutation_kind: StateMutationKind,
    occurred_at: DateTime<Utc>,
    source_kind: StateSourceKind,
    payload_digest: String,
}

impl PreparedTransition {
    fn validate(command: TransitionDailyTaskCommand, source_kind: StateSourceKind) -> Result<Self> {
        validate_uuid_v4("state_operation_id", &command.operation_id)?;
        validate_bounded_ref("state_source_message_ref", &command.source_message_ref)?;
        validate_uuid_v4("state_asset_id", &command.asset_id)?;
        if command.expected_version == 0 {
            anyhow::bail!("state_expected_version_invalid");
        }
        match (source_kind, command.mutation_kind) {
            (StateSourceKind::CurrentAuthenticatedUserMessage, StateMutationKind::Complete)
            | (StateSourceKind::CurrentAuthenticatedUserMessage, StateMutationKind::Undo)
            | (StateSourceKind::SystemExpiry, StateMutationKind::Expire) => {}
            _ => anyhow::bail!("state_transition_source_kind_mismatch"),
        }
        let payload_digest = digest_json(&serde_json::json!({
            "schema": "openlife.state-transition-payload.v1",
            "operationId": command.operation_id,
            "sourceMessageRef": command.source_message_ref,
            "assetId": command.asset_id,
            "expectedVersion": command.expected_version,
            "mutationKind": command.mutation_kind,
            "occurredAt": command.occurred_at.to_rfc3339(),
            "sourceKind": source_kind,
        }))?;
        Ok(Self {
            operation_id: command.operation_id,
            source_message_ref: command.source_message_ref,
            asset_id: command.asset_id,
            expected_version: command.expected_version,
            mutation_kind: command.mutation_kind,
            occurred_at: command.occurred_at,
            source_kind,
            payload_digest,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_operation(
    tx: &Transaction<'_>,
    operation_id: &str,
    payload_digest: &str,
    receipt_id: &str,
    asset_id: &str,
    asset_version: u64,
    mutation_kind: StateMutationKind,
    outbox_event_id: &str,
    committed_at: DateTime<Utc>,
) -> Result<()> {
    tx.execute(
        "INSERT INTO state_operations (
            operation_id, payload_digest, receipt_id, asset_id, asset_version,
            mutation_kind, outbox_event_id, committed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            operation_id,
            payload_digest,
            receipt_id,
            asset_id,
            i64::try_from(asset_version)?,
            mutation_kind.as_str(),
            outbox_event_id,
            committed_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn replay_operation(
    conn: &Connection,
    operation_id: &str,
    payload_digest: &str,
) -> Result<Option<StateExecutionReceipt>> {
    let existing_digest = conn
        .query_row(
            "SELECT payload_digest FROM state_operations WHERE operation_id = ?1",
            [operation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(existing_digest) = existing_digest else {
        return Ok(None);
    };
    if existing_digest != payload_digest {
        anyhow::bail!("state_operation_payload_drift");
    }
    operation_receipt(conn, operation_id, true)
}

fn operation_receipt(
    conn: &Connection,
    operation_id: &str,
    replayed: bool,
) -> Result<Option<StateExecutionReceipt>> {
    let row = conn
        .query_row(
            "SELECT operations.receipt_id, operations.payload_digest,
                    operations.asset_id, operations.asset_version,
                    operations.mutation_kind, operations.outbox_event_id,
                    operations.committed_at, assets.expires_at
             FROM state_operations operations
             JOIN state_assets assets ON assets.asset_id = operations.asset_id
             WHERE operations.operation_id = ?1",
            [operation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .optional()?;
    let Some((
        receipt_id,
        payload_digest,
        asset_id,
        asset_version,
        mutation_kind,
        outbox_event_id,
        committed_at,
        expires_at,
    )) = row
    else {
        return Ok(None);
    };
    let outbox = crate::persistence_outbox::mutation_by_event_id(conn, &outbox_event_id)?
        .context("state_operation_outbox_event_missing")?;
    let projection = crate::persistence_outbox::projection_summary(conn, &outbox_event_id)?;
    Ok(Some(StateExecutionReceipt {
        schema: "openlife.state-execution-receipt.v1".into(),
        receipt_id,
        operation_id: operation_id.to_string(),
        payload_digest,
        asset_id,
        asset_version: u64::try_from(asset_version).context("state_receipt_version_invalid")?,
        mutation_kind: parse_mutation_kind(&mutation_kind)?,
        canonical_status: "committed".into(),
        projection_status: match projection.state() {
            crate::persistence_outbox::ProjectionDeliveryState::Pending => {
                StateProjectionStatus::Pending
            }
            crate::persistence_outbox::ProjectionDeliveryState::Degraded => {
                StateProjectionStatus::Degraded
            }
            crate::persistence_outbox::ProjectionDeliveryState::Applied
            | crate::persistence_outbox::ProjectionDeliveryState::Superseded
            | crate::persistence_outbox::ProjectionDeliveryState::Compensated => {
                StateProjectionStatus::Applied
            }
        },
        outbox_event_id,
        tombstone_id: outbox.tombstone_id,
        committed_at: parse_time(&committed_at)?,
        expires_at: parse_time(&expires_at)?,
        replayed,
    }))
}

fn load_asset(conn: &Connection, asset_id: &str) -> Result<Option<StateAsset>> {
    conn.query_row(
        "SELECT asset_id, kind, version, title, status, due_at,
                source_message_ref, risk, sensitivity, source_kind, confidence,
                privacy_class, created_at, updated_at, expires_at,
                tombstoned_at, tombstone_reason
         FROM state_assets WHERE asset_id = ?1",
        [asset_id],
        state_asset_from_row,
    )
    .optional()
    .map_err(Into::into)
}

fn state_asset_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StateAsset> {
    let version = row.get::<_, i64>(2)?;
    let confidence = row.get::<_, f64>(10)?;
    Ok(StateAsset {
        asset_id: row.get(0)?,
        kind: parse_asset_kind_sql(&row.get::<_, String>(1)?)?,
        version: u64::try_from(version).map_err(sql_conversion_error)?,
        title: row.get(3)?,
        status: parse_task_status_sql(&row.get::<_, String>(4)?)?,
        due_at: parse_optional_time_sql(row.get::<_, Option<String>>(5)?)?,
        source_message_ref: row.get(6)?,
        risk: parse_risk_sql(&row.get::<_, String>(7)?)?,
        sensitivity: parse_sensitivity_sql(&row.get::<_, String>(8)?)?,
        source_kind: parse_source_kind_sql(&row.get::<_, String>(9)?)?,
        confidence: confidence as f32,
        privacy_class: parse_privacy_sql(&row.get::<_, String>(11)?)?,
        created_at: parse_time_sql(row.get::<_, String>(12)?)?,
        updated_at: parse_time_sql(row.get::<_, String>(13)?)?,
        expires_at: parse_time_sql(row.get::<_, String>(14)?)?,
        tombstoned_at: parse_optional_time_sql(row.get::<_, Option<String>>(15)?)?,
        tombstone_reason: row
            .get::<_, Option<String>>(16)?
            .map(|value| parse_mutation_kind_sql(&value))
            .transpose()?,
    })
}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.busy_timeout(StdDuration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    if !conn.is_autocommit() {
        anyhow::bail!("StateStore connection unexpectedly opened in a transaction");
    }
    if conn.path().is_some() {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
    }
    Ok(())
}

fn validate_uuid_v4(label: &str, value: &str) -> Result<()> {
    let parsed = Uuid::parse_str(value).with_context(|| format!("{label}_invalid_uuid"))?;
    if parsed.get_version() != Some(Version::Random) || parsed.hyphenated().to_string() != value {
        anyhow::bail!("{label}_must_be_canonical_uuid_v4");
    }
    Ok(())
}

fn validate_bounded_ref(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty()
        || value.chars().count() > MAX_SOURCE_MESSAGE_REF_CHARS
        || value.chars().any(char::is_control)
    {
        anyhow::bail!("{label}_invalid");
    }
    Ok(())
}

fn digest_json(value: &serde_json::Value) -> Result<String> {
    Ok(crate::persistence_outbox::metadata_digest(
        &serde_json::to_string(value)?,
    ))
}

fn parse_time(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("state_timestamp_invalid:{value}"))?
        .with_timezone(&Utc))
}

fn parse_time_sql(value: String) -> rusqlite::Result<DateTime<Utc>> {
    parse_time(&value).map_err(|error| sql_conversion_error(error.to_string()))
}

fn parse_optional_time_sql(value: Option<String>) -> rusqlite::Result<Option<DateTime<Utc>>> {
    value.map(parse_time_sql).transpose()
}

fn sql_conversion_error(error: impl std::fmt::Display + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}

fn parse_asset_kind_sql(value: &str) -> rusqlite::Result<StateAssetKind> {
    match value {
        "daily_task" => Ok(StateAssetKind::DailyTask),
        other => Err(sql_conversion_error(format!(
            "state_asset_kind_invalid:{other}"
        ))),
    }
}

fn parse_task_status_sql(value: &str) -> rusqlite::Result<DailyTaskStatus> {
    match value {
        "pending" => Ok(DailyTaskStatus::Pending),
        "completed" => Ok(DailyTaskStatus::Completed),
        "tombstoned" => Ok(DailyTaskStatus::Tombstoned),
        other => Err(sql_conversion_error(format!(
            "state_task_status_invalid:{other}"
        ))),
    }
}

fn parse_mutation_kind(value: &str) -> Result<StateMutationKind> {
    match value {
        "create" => Ok(StateMutationKind::Create),
        "complete" => Ok(StateMutationKind::Complete),
        "undo" => Ok(StateMutationKind::Undo),
        "expire" => Ok(StateMutationKind::Expire),
        other => anyhow::bail!("state_mutation_kind_invalid:{other}"),
    }
}

fn parse_mutation_kind_sql(value: &str) -> rusqlite::Result<StateMutationKind> {
    parse_mutation_kind(value).map_err(|error| sql_conversion_error(error.to_string()))
}

fn parse_risk_sql(value: &str) -> rusqlite::Result<StateRisk> {
    match value {
        "low" => Ok(StateRisk::Low),
        other => Err(sql_conversion_error(format!("state_risk_invalid:{other}"))),
    }
}

fn parse_sensitivity_sql(value: &str) -> rusqlite::Result<StateSensitivity> {
    match value {
        "internal" => Ok(StateSensitivity::Internal),
        other => Err(sql_conversion_error(format!(
            "state_sensitivity_invalid:{other}"
        ))),
    }
}

fn parse_source_kind_sql(value: &str) -> rusqlite::Result<StateSourceKind> {
    match value {
        "current_authenticated_user_message" => {
            Ok(StateSourceKind::CurrentAuthenticatedUserMessage)
        }
        "system_expiry" => Ok(StateSourceKind::SystemExpiry),
        other => Err(sql_conversion_error(format!(
            "state_source_kind_invalid:{other}"
        ))),
    }
}

fn parse_privacy_sql(value: &str) -> rusqlite::Result<StatePrivacyClass> {
    match value {
        "private" => Ok(StatePrivacyClass::Private),
        other => Err(sql_conversion_error(format!(
            "state_privacy_class_invalid:{other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn at(hour: u32) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(&format!("2026-07-14T{hour:02}:00:00Z"))
            .unwrap()
            .with_timezone(&Utc)
    }

    fn create_command(operation_id: String, title: &str) -> CreateDailyTaskCommand {
        CreateDailyTaskCommand {
            operation_id,
            source_message_ref: Uuid::new_v4().hyphenated().to_string(),
            title: title.into(),
            due_at: Some(at(15)),
            created_at: at(9),
            expires_at: at(9) + Duration::days(1),
            risk: StateRisk::Low,
            sensitivity: StateSensitivity::Internal,
            source_kind: StateSourceKind::CurrentAuthenticatedUserMessage,
            confidence: 1.0,
            privacy_class: StatePrivacyClass::Private,
        }
    }

    fn transition(
        asset: &StateAsset,
        mutation_kind: StateMutationKind,
        occurred_at: DateTime<Utc>,
    ) -> TransitionDailyTaskCommand {
        TransitionDailyTaskCommand {
            operation_id: Uuid::new_v4().hyphenated().to_string(),
            source_message_ref: Uuid::new_v4().hyphenated().to_string(),
            asset_id: asset.asset_id.clone(),
            expected_version: asset.version,
            mutation_kind,
            occurred_at,
        }
    }

    #[test]
    fn create_receipt_is_minimal_and_exact_replay_reuses_canonical_effect() {
        let store = StateStore::new_in_memory().unwrap();
        let command = create_command(Uuid::new_v4().hyphenated().to_string(), "完成路演设备检查");
        let source_message_ref = command.source_message_ref.clone();
        let first = store.create_daily_task(command.clone()).unwrap();
        let replay = store.create_daily_task(command).unwrap();
        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(first.asset_id, replay.asset_id);
        assert_eq!(first.payload_digest, replay.payload_digest);
        assert_eq!(store.list_daily_tasks(false).unwrap().len(), 1);

        let serialized = serde_json::to_string(&first).unwrap();
        assert!(!serialized.contains("完成路演设备检查"));
        assert!(!serialized.contains(&source_message_ref));
        let outbox_metadata = {
            let conn = store.lock_connection().unwrap();
            conn.query_row(
                "SELECT aggregate_kind || ':' || mutation_kind || ':' || payload_digest
                 FROM canonical_outbox_events WHERE event_id = ?1",
                [&first.outbox_event_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
        };
        assert!(!outbox_metadata.contains("完成路演设备检查"));
        assert!(!outbox_metadata.contains(&source_message_ref));
    }

    #[test]
    fn operation_payload_drift_fails_without_second_asset() {
        let store = StateStore::new_in_memory().unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        store
            .create_daily_task(create_command(operation_id.clone(), "任务 A"))
            .unwrap();
        let error = store
            .create_daily_task(create_command(operation_id, "任务 B"))
            .unwrap_err();
        assert!(error.to_string().contains("state_operation_payload_drift"));
        assert_eq!(store.list_daily_tasks(true).unwrap().len(), 1);
    }

    #[test]
    fn transaction_fault_rolls_back_asset_operation_and_outbox() {
        let store = StateStore::new_in_memory().unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let error = store
            .create_daily_task_guarded(create_command(operation_id.clone(), "回滚任务"), || {
                anyhow::bail!("injected_state_commit_failure")
            })
            .unwrap_err();
        assert!(error.to_string().contains("injected_state_commit_failure"));
        assert!(store.list_daily_tasks(true).unwrap().is_empty());
        assert!(store
            .receipt_for_operation(&operation_id, false)
            .unwrap()
            .is_none());
        assert!(store
            .list_replayable_projection_deliveries(10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn complete_then_undo_creates_versions_and_durable_tombstone() {
        let store = StateStore::new_in_memory().unwrap();
        let created = store
            .create_daily_task(create_command(
                Uuid::new_v4().hyphenated().to_string(),
                "可撤销任务",
            ))
            .unwrap();
        let asset = store.get_asset(&created.asset_id).unwrap().unwrap();
        let completed = store
            .complete_daily_task(transition(&asset, StateMutationKind::Complete, at(16)))
            .unwrap();
        let completed_asset = store.get_asset(&created.asset_id).unwrap().unwrap();
        assert_eq!(completed.asset_version, 2);
        assert_eq!(completed_asset.status, DailyTaskStatus::Completed);

        let undone = store
            .undo_daily_task(transition(
                &completed_asset,
                StateMutationKind::Undo,
                at(17),
            ))
            .unwrap();
        let tombstoned = store.get_asset(&created.asset_id).unwrap().unwrap();
        assert_eq!(undone.asset_version, 3);
        assert!(undone.tombstone_id.is_some());
        assert_eq!(tombstoned.status, DailyTaskStatus::Tombstoned);
        assert_eq!(tombstoned.tombstone_reason, Some(StateMutationKind::Undo));
        assert!(store.list_daily_tasks(false).unwrap().is_empty());
    }

    #[test]
    fn expiry_tombstone_survives_restart_and_projection_failure_stays_degraded() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.db");
        let created = {
            let store = StateStore::new(&path).unwrap();
            let receipt = store
                .create_daily_task(create_command(
                    Uuid::new_v4().hyphenated().to_string(),
                    "过期任务",
                ))
                .unwrap();
            store.expire_due(at(9) + Duration::days(2)).unwrap();
            receipt
        };
        let restarted = StateStore::new(&path).unwrap();
        let asset = restarted.get_asset(&created.asset_id).unwrap().unwrap();
        assert_eq!(asset.status, DailyTaskStatus::Tombstoned);
        assert_eq!(asset.tombstone_reason, Some(StateMutationKind::Expire));
        let expiry = restarted
            .list_replayable_projection_deliveries(20)
            .unwrap()
            .into_iter()
            .find(|delivery| delivery.mutation_kind == "deleted")
            .unwrap();
        restarted
            .mark_projection_degraded(&expiry.event_id, "injected_yaml_projection_failure")
            .unwrap();
        let receipt_operation = {
            let conn = restarted.lock_connection().unwrap();
            conn.query_row(
                "SELECT operation_id FROM state_operations WHERE outbox_event_id = ?1",
                [&expiry.event_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
        };
        let receipt = restarted
            .receipt_for_operation(&receipt_operation, true)
            .unwrap()
            .unwrap();
        assert_eq!(receipt.canonical_status, "committed");
        assert_eq!(receipt.projection_status, StateProjectionStatus::Degraded);
    }

    #[test]
    fn concurrent_same_operation_has_one_canonical_winner() {
        let store = Arc::new(StateStore::new_in_memory().unwrap());
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let source_message_ref = Uuid::new_v4().hyphenated().to_string();
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let store = Arc::clone(&store);
            let operation_id = operation_id.clone();
            let source_message_ref = source_message_ref.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                let mut command = create_command(operation_id, "并发任务");
                command.source_message_ref = source_message_ref;
                barrier.wait();
                store.create_daily_task(command).unwrap()
            }));
        }
        barrier.wait();
        let receipts = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            receipts.iter().filter(|receipt| !receipt.replayed).count(),
            1
        );
        assert_eq!(
            receipts.iter().filter(|receipt| receipt.replayed).count(),
            1
        );
        assert_eq!(receipts[0].asset_id, receipts[1].asset_id);
        assert_eq!(store.list_daily_tasks(false).unwrap().len(), 1);
    }

    #[test]
    fn concurrent_distinct_completion_operations_enforce_version_cas() {
        let store = Arc::new(StateStore::new_in_memory().unwrap());
        let created = store
            .create_daily_task(create_command(
                Uuid::new_v4().hyphenated().to_string(),
                "CAS 任务",
            ))
            .unwrap();
        let asset = store.get_asset(&created.asset_id).unwrap().unwrap();
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let store = Arc::clone(&store);
            let command = transition(&asset, StateMutationKind::Complete, at(16));
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                store.complete_daily_task(command)
            }));
        }
        barrier.wait();
        let outcomes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        let error = outcomes
            .into_iter()
            .find_map(Result::err)
            .unwrap()
            .to_string();
        assert!(error.contains("state_asset_version_conflict"));
        assert_eq!(
            store.get_asset(&asset.asset_id).unwrap().unwrap().version,
            2
        );
    }

    #[test]
    fn ttl_and_source_boundaries_fail_closed() {
        let store = StateStore::new_in_memory().unwrap();
        let mut short = create_command(Uuid::new_v4().hyphenated().to_string(), "短 TTL");
        short.expires_at = short.created_at + Duration::hours(23);
        assert!(store
            .create_daily_task(short)
            .unwrap_err()
            .to_string()
            .contains("state_asset_ttl_out_of_range"));

        let mut wrong_source = create_command(Uuid::new_v4().hyphenated().to_string(), "错误来源");
        wrong_source.source_kind = StateSourceKind::SystemExpiry;
        assert!(store
            .create_daily_task(wrong_source)
            .unwrap_err()
            .to_string()
            .contains("state_create_source_not_current_user"));
    }
}
