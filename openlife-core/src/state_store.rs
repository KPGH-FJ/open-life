//! Canonical transient-state owner for the ADR 0015 command lane.
//!
//! State content is stored once in this SQLite owner. Operation receipts and
//! the shared transactional outbox contain only identity, digest, lifecycle,
//! and projection state; they never copy the task title or user message body.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;
use uuid::{Uuid, Version};

const STATE_STORE_SCHEMA_VERSION: i64 = 3;
const STATE_ASSET_AGGREGATE_KIND: &str = "transient_state_asset";
pub const LIFEMODEL_YAML_PROJECTION_TARGET: &str = "lifemodel_yaml_compat_v1";
const PROJECTION_TARGETS: &[&str] = &[LIFEMODEL_YAML_PROJECTION_TARGET];
const MAX_TASK_TITLE_CHARS: usize = 512;
const MAX_SOURCE_MESSAGE_REF_CHARS: usize = 256;
const MAX_RESOURCE_TASK_BATCH_ITEMS: usize = 8;

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
pub struct ResourceDailyTaskDraft {
    pub title: String,
    pub resource_id: String,
    pub chunk_ordinal: u32,
    pub content_digest: String,
}

#[derive(Debug, Clone)]
pub struct CreateResourceDailyTaskBatchCommand {
    pub operation_id: String,
    pub request_digest: String,
    pub source_message_ref: String,
    pub tasks: Vec<ResourceDailyTaskDraft>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateBatchAssetReceipt {
    pub receipt_id: String,
    pub asset_id: String,
    pub asset_version: u64,
    pub payload_digest: String,
    pub outbox_event_id: String,
    pub projection_status: StateProjectionStatus,
    pub resource_id: String,
    pub chunk_ordinal: u32,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateBatchExecutionReceipt {
    pub schema: String,
    pub receipt_id: String,
    pub operation_id: String,
    pub payload_digest: String,
    pub canonical_status: String,
    pub assets: Vec<StateBatchAssetReceipt>,
    pub committed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct CreateDailyTaskCommand {
    pub operation_id: String,
    /// Sealed user-request identity. Gateway callers provide the policy-bound
    /// digest; lower-level storage callers fall back to the canonical payload
    /// digest for backwards-compatible idempotency.
    pub request_digest: Option<String>,
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
    pub request_digest: Option<String>,
    pub source_message_ref: String,
    pub asset_id: String,
    pub expected_version: u64,
    pub mutation_kind: StateMutationKind,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StateGatewayExecutionContext {
    pub occurred_at: DateTime<Utc>,
    /// Time-zone resolution is owned by the application boundary. The gateway
    /// accepts it only when the sealed intent carried a corresponding due hint.
    pub resolved_due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateCommandOutcome {
    pub command_kind: crate::agent::main_chat_agent_v1::TransientStateCommandKind,
    pub receipt: Option<StateExecutionReceipt>,
    pub tasks: Vec<StateAsset>,
    pub source_message_ref: String,
    pub source_message_digest: String,
    pub policy_contract_digest: String,
}

#[derive(Clone)]
pub struct StateGateway {
    store: StateStore,
}

impl StateGateway {
    pub fn new(store: StateStore) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &StateStore {
        &self.store
    }

    pub fn execute(
        &self,
        grant: crate::agent::main_chat_agent_v1::PolicyTransientStateGrant,
        context: StateGatewayExecutionContext,
    ) -> Result<StateCommandOutcome> {
        self.execute_guarded(grant, context, || Result::<()>::Ok(()))
    }

    pub fn execute_with_admission(
        &self,
        grant: crate::agent::main_chat_agent_v1::PolicyTransientStateGrant,
        context: StateGatewayExecutionContext,
        admission: &dyn crate::agent::CanonicalWriteAdmission,
    ) -> Result<StateCommandOutcome> {
        if !grant.intent().command_kind.is_mutation() {
            return self.execute(grant, context);
        }
        let object_ref = grant.operation_id().to_string();
        let mut permit: Option<Box<dyn crate::agent::CanonicalWritePermit>> = None;
        let outcome = self.execute_guarded(grant, context, || {
            permit = Some(
                admission
                    .acquire(crate::agent::CanonicalWriteAdmissionRequest::new(
                        "state_store.transient",
                        object_ref,
                    ))
                    .map_err(anyhow::Error::from)?,
            );
            Ok(())
        });
        match outcome {
            Ok(outcome) => {
                if outcome
                    .receipt
                    .as_ref()
                    .is_some_and(|receipt| receipt.replayed)
                    && permit.is_none()
                {
                    // A committed operation can be recovered without opening
                    // a second write-admission window. The durable receipt is
                    // read-only truth, even if cancellation arrived after the
                    // original commit.
                    return Ok(outcome);
                }
                let permit = permit.context("state_gateway_commit_permit_missing")?;
                if outcome
                    .receipt
                    .as_ref()
                    .is_some_and(|receipt| receipt.replayed)
                {
                    permit.finish_noop();
                } else {
                    permit.finish_committed();
                }
                Ok(outcome)
            }
            Err(error) => {
                if let Some(permit) = permit {
                    permit.finish_failed();
                }
                Err(error)
            }
        }
    }

    pub fn replay_resource_task_batch(
        &self,
        grant: &crate::agent::main_chat_agent_v1::PolicyTransientStateGrant,
    ) -> Result<Option<StateBatchExecutionReceipt>> {
        validate_resource_task_batch_grant(grant)?;
        let Some(receipt) = self.store.resource_task_batch_receipt_for_request(
            grant.operation_id(),
            grant.intent_digest(),
            true,
        )?
        else {
            return Ok(None);
        };
        Ok(Some(receipt))
    }

    pub fn execute_resource_task_batch_with_admission(
        &self,
        grant: crate::agent::main_chat_agent_v1::PolicyTransientStateGrant,
        tasks: Vec<ResourceDailyTaskDraft>,
        context: StateGatewayExecutionContext,
        admission: &dyn crate::agent::CanonicalWriteAdmission,
    ) -> Result<StateBatchExecutionReceipt> {
        validate_resource_task_batch_grant(&grant)?;
        if context.resolved_due_at.is_some() {
            anyhow::bail!("state_resource_task_batch_due_time_unsupported");
        }
        let operation_id = grant.operation_id().to_string();
        let mut permit: Option<Box<dyn crate::agent::CanonicalWritePermit>> = None;
        let receipt = self.store.create_resource_task_batch_guarded(
            CreateResourceDailyTaskBatchCommand {
                operation_id: operation_id.clone(),
                request_digest: grant.intent_digest().to_string(),
                source_message_ref: grant.source_user_message_id().to_string(),
                tasks,
                created_at: context.occurred_at,
                expires_at: context.occurred_at
                    + Duration::days(i64::from(grant.intent().expiry_days)),
            },
            || {
                permit = Some(
                    admission
                        .acquire(crate::agent::CanonicalWriteAdmissionRequest::new(
                            "state_store.resource_task_batch",
                            operation_id.clone(),
                        ))
                        .map_err(anyhow::Error::from)?,
                );
                Ok(())
            },
        );
        match receipt {
            Ok(receipt) => {
                if receipt.replayed && permit.is_none() {
                    return Ok(receipt);
                }
                let permit = permit.context("state_resource_task_batch_commit_permit_missing")?;
                if receipt.replayed {
                    permit.finish_noop();
                } else {
                    permit.finish_committed();
                }
                Ok(receipt)
            }
            Err(error) => {
                if let Some(permit) = permit {
                    permit.finish_failed();
                }
                Err(error)
            }
        }
    }

    pub fn execute_guarded<F>(
        &self,
        grant: crate::agent::main_chat_agent_v1::PolicyTransientStateGrant,
        context: StateGatewayExecutionContext,
        before_commit: F,
    ) -> Result<StateCommandOutcome>
    where
        F: FnOnce() -> Result<()>,
    {
        use crate::agent::main_chat_agent_v1::TransientStateCommandKind;

        let operation_id = grant.operation_id().to_string();
        let source_message_ref = grant.source_user_message_id().to_string();
        let source_message_digest = grant.source_user_message_digest().to_string();
        let policy_contract_digest = grant.policy_contract_digest().to_string();
        let request_digest = grant.intent_digest().to_string();
        if grant.intent_digest().trim().is_empty()
            || !is_sha256_digest(&source_message_digest)
            || !is_sha256_digest(&policy_contract_digest)
        {
            anyhow::bail!("state_gateway_policy_grant_invalid");
        }
        let intent = grant.intent().clone();
        if intent.expiry_days == 0 || intent.expiry_days > 7 {
            anyhow::bail!("state_gateway_expiry_days_invalid");
        }
        if intent.due_hint.is_some() != context.resolved_due_at.is_some() {
            anyhow::bail!("state_gateway_due_resolution_mismatch");
        }
        if intent.command_kind.is_mutation() {
            if let Some(receipt) = self.store.replay_request(&operation_id, &request_digest)? {
                return Ok(StateCommandOutcome {
                    command_kind: intent.command_kind,
                    receipt: Some(receipt),
                    tasks: self.store.list_daily_tasks(false)?,
                    source_message_ref,
                    source_message_digest,
                    policy_contract_digest,
                });
            }
        }
        let receipt = match intent.command_kind {
            TransientStateCommandKind::ListDailyTasks => {
                if intent.command_kind.is_mutation() {
                    anyhow::bail!("state_gateway_list_contract_invalid");
                }
                before_commit()?;
                None
            }
            TransientStateCommandKind::CreateDailyTask => {
                Some(self.store.create_daily_task_guarded(
                    CreateDailyTaskCommand {
                        operation_id,
                        request_digest: Some(request_digest),
                        source_message_ref: source_message_ref.clone(),
                        title: intent.target,
                        due_at: context.resolved_due_at,
                        created_at: context.occurred_at,
                        expires_at: context.occurred_at
                            + Duration::days(i64::from(intent.expiry_days)),
                        risk: StateRisk::Low,
                        sensitivity: StateSensitivity::Internal,
                        source_kind: StateSourceKind::CurrentAuthenticatedUserMessage,
                        confidence: 1.0,
                        privacy_class: StatePrivacyClass::Private,
                    },
                    before_commit,
                )?)
            }
            TransientStateCommandKind::CompleteDailyTask
            | TransientStateCommandKind::UndoDailyTask => {
                let asset = resolve_active_task_target(&self.store, &intent.target)?;
                let mutation_kind =
                    if intent.command_kind == TransientStateCommandKind::CompleteDailyTask {
                        StateMutationKind::Complete
                    } else {
                        StateMutationKind::Undo
                    };
                Some(self.store.transition_daily_task_guarded(
                    TransitionDailyTaskCommand {
                        operation_id,
                        request_digest: Some(request_digest),
                        source_message_ref: source_message_ref.clone(),
                        asset_id: asset.asset_id,
                        expected_version: asset.version,
                        mutation_kind,
                        occurred_at: context.occurred_at,
                    },
                    before_commit,
                )?)
            }
        };
        Ok(StateCommandOutcome {
            command_kind: intent.command_kind,
            receipt,
            tasks: self.store.list_daily_tasks(false)?,
            source_message_ref,
            source_message_digest,
            policy_contract_digest,
        })
    }
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
                request_digest TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                receipt_id TEXT NOT NULL UNIQUE,
                asset_id TEXT NOT NULL,
                asset_version INTEGER NOT NULL CHECK(asset_version > 0),
                mutation_kind TEXT NOT NULL CHECK(mutation_kind IN ('create', 'complete', 'undo', 'expire')),
                outbox_event_id TEXT NOT NULL UNIQUE,
                committed_at TEXT NOT NULL,
                FOREIGN KEY(asset_id) REFERENCES state_assets(asset_id),
                FOREIGN KEY(outbox_event_id) REFERENCES canonical_outbox_events(event_id)
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS state_resource_task_batch_operations (
                operation_id TEXT PRIMARY KEY,
                request_digest TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                receipt_id TEXT NOT NULL UNIQUE,
                item_count INTEGER NOT NULL CHECK(item_count > 0 AND item_count <= 8),
                committed_at TEXT NOT NULL,
                expires_at TEXT NOT NULL
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS state_resource_task_batch_items (
                operation_id TEXT NOT NULL,
                ordinal INTEGER NOT NULL CHECK(ordinal >= 0 AND ordinal < 8),
                receipt_id TEXT NOT NULL UNIQUE,
                asset_id TEXT NOT NULL UNIQUE,
                asset_version INTEGER NOT NULL CHECK(asset_version > 0),
                payload_digest TEXT NOT NULL,
                outbox_event_id TEXT NOT NULL UNIQUE,
                resource_id TEXT NOT NULL,
                chunk_ordinal INTEGER NOT NULL CHECK(chunk_ordinal >= 0),
                content_digest TEXT NOT NULL,
                PRIMARY KEY(operation_id, ordinal),
                FOREIGN KEY(operation_id) REFERENCES state_resource_task_batch_operations(operation_id),
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
            Some("1") => {
                tx.execute_batch(
                    "ALTER TABLE state_operations ADD COLUMN request_digest TEXT;
                     UPDATE state_operations
                     SET request_digest = payload_digest
                     WHERE request_digest IS NULL;
                     UPDATE state_store_metadata
                     SET value = '3'
                     WHERE key = 'schema_version';",
                )?;
            }
            Some("2") => {
                tx.execute(
                    "UPDATE state_store_metadata SET value = '3' WHERE key = 'schema_version'",
                    [],
                )?;
            }
            Some("3") => {}
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

    pub fn create_resource_task_batch(
        &self,
        command: CreateResourceDailyTaskBatchCommand,
    ) -> Result<StateBatchExecutionReceipt> {
        self.create_resource_task_batch_guarded(command, || Result::<()>::Ok(()))
    }

    pub fn create_resource_task_batch_guarded<F>(
        &self,
        command: CreateResourceDailyTaskBatchCommand,
        before_commit: F,
    ) -> Result<StateBatchExecutionReceipt>
    where
        F: FnOnce() -> Result<()>,
    {
        let prepared = PreparedResourceTaskBatch::validate(command)?;
        let mut before_commit = Some(before_commit);
        let mut conn = self.lock_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((existing_request_digest, existing_payload_digest)) = tx
            .query_row(
                "SELECT request_digest, payload_digest
                 FROM state_resource_task_batch_operations
                 WHERE operation_id = ?1",
                [&prepared.operation_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            if existing_request_digest != prepared.request_digest {
                anyhow::bail!("state_resource_task_batch_request_drift");
            }
            if existing_payload_digest != prepared.payload_digest {
                anyhow::bail!("state_resource_task_batch_payload_drift");
            }
            tx.rollback()?;
            drop(conn);
            return self
                .resource_task_batch_receipt_for_operation(&prepared.operation_id, true)?
                .context("state_resource_task_batch_replay_receipt_missing");
        }
        prepared.validate_new_effect_timing()?;
        let batch_receipt_id = format!("state_batch_receipt:{}", Uuid::new_v4());
        tx.execute(
            "INSERT INTO state_resource_task_batch_operations (
                operation_id, request_digest, payload_digest, receipt_id,
                item_count, committed_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                prepared.operation_id,
                prepared.request_digest,
                prepared.payload_digest,
                batch_receipt_id,
                i64::try_from(prepared.tasks.len())?,
                prepared.created_at.to_rfc3339(),
                prepared.expires_at.to_rfc3339(),
            ],
        )?;
        for (ordinal, task) in prepared.tasks.iter().enumerate() {
            let asset_id = Uuid::new_v4().hyphenated().to_string();
            let asset_receipt_id = format!("state_receipt:{}", Uuid::new_v4());
            let asset_operation_id = Uuid::new_v4().hyphenated().to_string();
            tx.execute(
                "INSERT INTO state_assets (
                    asset_id, kind, version, title, status, due_at,
                    source_message_ref, risk, sensitivity, source_kind, confidence,
                    privacy_class, created_at, updated_at, expires_at,
                    tombstoned_at, tombstone_reason
                 ) VALUES (?1, 'daily_task', 1, ?2, 'pending', NULL, ?3,
                           'low', 'internal', 'current_authenticated_user_message', 1.0,
                           'private', ?4, ?4, ?5, NULL, NULL)",
                params![
                    asset_id,
                    task.title,
                    prepared.source_message_ref,
                    prepared.created_at.to_rfc3339(),
                    prepared.expires_at.to_rfc3339(),
                ],
            )?;
            tx.execute(
                "INSERT INTO state_asset_versions (
                    asset_id, version, operation_id, payload_digest, mutation_kind,
                    status, title, due_at, source_message_ref, source_kind,
                    created_at, expires_at, tombstone_reason
                 ) VALUES (?1, 1, ?2, ?3, 'create', 'pending', ?4, NULL, ?5,
                           'current_authenticated_user_message', ?6, ?7, NULL)",
                params![
                    asset_id,
                    asset_operation_id,
                    task.payload_digest,
                    task.title,
                    prepared.source_message_ref,
                    prepared.created_at.to_rfc3339(),
                    prepared.expires_at.to_rfc3339(),
                ],
            )?;
            let outbox = crate::persistence_outbox::enqueue_mutation(
                &tx,
                STATE_ASSET_AGGREGATE_KIND,
                &asset_id,
                StateMutationKind::Create.as_str(),
                &task.payload_digest,
                PROJECTION_TARGETS,
            )?;
            tx.execute(
                "INSERT INTO state_resource_task_batch_items (
                    operation_id, ordinal, receipt_id, asset_id, asset_version,
                    payload_digest, outbox_event_id, resource_id, chunk_ordinal,
                    content_digest
                 ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8, ?9)",
                params![
                    prepared.operation_id,
                    i64::try_from(ordinal)?,
                    asset_receipt_id,
                    asset_id,
                    task.payload_digest,
                    outbox.event_id,
                    task.resource_id,
                    i64::from(task.chunk_ordinal),
                    task.content_digest,
                ],
            )?;
        }
        let _commit_guard = before_commit
            .take()
            .context("state_resource_task_batch_commit_guard_missing")?(
        )?;
        tx.commit()?;
        drop(conn);
        self.resource_task_batch_receipt_for_operation(&prepared.operation_id, false)?
            .context("state_resource_task_batch_receipt_missing_after_commit")
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
        let mut before_commit = Some(before_commit);
        let mut conn = self.lock_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(receipt) =
            replay_operation(&tx, &prepared.operation_id, &prepared.payload_digest)?
        {
            let _commit_guard = before_commit
                .take()
                .context("state_create_commit_guard_missing")?()?;
            tx.rollback()?;
            return Ok(receipt);
        }
        prepared.validate_new_effect_timing()?;

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
            &prepared.request_digest,
            &prepared.payload_digest,
            &receipt_id,
            &asset_id,
            1,
            StateMutationKind::Create,
            &outbox.event_id,
            prepared.created_at,
        )?;
        let _commit_guard = before_commit
            .take()
            .context("state_create_commit_guard_missing")?()?;
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
                    request_digest: None,
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
        let mut before_commit = Some(before_commit);
        let mut conn = self.lock_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(receipt) =
            replay_operation(&tx, &prepared.operation_id, &prepared.payload_digest)?
        {
            let _commit_guard = before_commit
                .take()
                .context("state_transition_commit_guard_missing")?(
            )?;
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
            &prepared.request_digest,
            &prepared.payload_digest,
            &receipt_id,
            &prepared.asset_id,
            next_version,
            prepared.mutation_kind,
            &outbox.event_id,
            prepared.occurred_at,
        )?;
        let _commit_guard = before_commit
            .take()
            .context("state_transition_commit_guard_missing")?()?;
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
            "SELECT assets.asset_id, assets.kind, assets.version, assets.title,
                    assets.status, assets.due_at, assets.source_message_ref,
                    assets.risk, assets.sensitivity, assets.source_kind,
                    assets.confidence, assets.privacy_class, assets.created_at,
                    assets.updated_at, assets.expires_at, assets.tombstoned_at,
                    assets.tombstone_reason
             FROM state_assets assets
             LEFT JOIN state_resource_task_batch_items batch
               ON batch.asset_id = assets.asset_id
             WHERE assets.kind = 'daily_task'
             ORDER BY assets.created_at ASC,
                      batch.operation_id ASC,
                      batch.ordinal ASC,
                      assets.asset_id ASC"
        } else {
            "SELECT assets.asset_id, assets.kind, assets.version, assets.title,
                    assets.status, assets.due_at, assets.source_message_ref,
                    assets.risk, assets.sensitivity, assets.source_kind,
                    assets.confidence, assets.privacy_class, assets.created_at,
                    assets.updated_at, assets.expires_at, assets.tombstoned_at,
                    assets.tombstone_reason
             FROM state_assets assets
             LEFT JOIN state_resource_task_batch_items batch
               ON batch.asset_id = assets.asset_id
             WHERE assets.kind = 'daily_task' AND assets.status != 'tombstoned'
             ORDER BY assets.created_at ASC,
                      batch.operation_id ASC,
                      batch.ordinal ASC,
                      assets.asset_id ASC"
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

    pub fn resource_task_batch_receipt_for_operation(
        &self,
        operation_id: &str,
        replayed: bool,
    ) -> Result<Option<StateBatchExecutionReceipt>> {
        validate_uuid_v4("state_resource_task_batch_operation_id", operation_id)?;
        let conn = self.lock_connection()?;
        resource_task_batch_receipt(&conn, operation_id, replayed)
    }

    pub fn resource_task_batch_receipt_for_request(
        &self,
        operation_id: &str,
        request_digest: &str,
        replayed: bool,
    ) -> Result<Option<StateBatchExecutionReceipt>> {
        validate_uuid_v4("state_resource_task_batch_operation_id", operation_id)?;
        if !is_sha256_digest(request_digest) {
            anyhow::bail!("state_resource_task_batch_request_digest_invalid");
        }
        let conn = self.lock_connection()?;
        let existing = conn
            .query_row(
                "SELECT request_digest FROM state_resource_task_batch_operations
                 WHERE operation_id = ?1",
                [operation_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(existing) = existing else {
            return Ok(None);
        };
        if existing != request_digest {
            anyhow::bail!("state_resource_task_batch_request_drift");
        }
        resource_task_batch_receipt(&conn, operation_id, replayed)
    }

    fn replay_request(
        &self,
        operation_id: &str,
        request_digest: &str,
    ) -> Result<Option<StateExecutionReceipt>> {
        validate_uuid_v4("state_operation_id", operation_id)?;
        if !is_sha256_digest(request_digest) {
            anyhow::bail!("state_operation_request_digest_invalid");
        }
        let conn = self.lock_connection()?;
        replay_operation_request(&conn, operation_id, request_digest)
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

struct PreparedResourceTaskBatch {
    operation_id: String,
    request_digest: String,
    source_message_ref: String,
    tasks: Vec<PreparedResourceTask>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    payload_digest: String,
}

struct PreparedResourceTask {
    title: String,
    resource_id: String,
    chunk_ordinal: u32,
    content_digest: String,
    payload_digest: String,
}

impl PreparedResourceTaskBatch {
    fn validate(command: CreateResourceDailyTaskBatchCommand) -> Result<Self> {
        validate_uuid_v4(
            "state_resource_task_batch_operation_id",
            &command.operation_id,
        )?;
        validate_bounded_ref("state_source_message_ref", &command.source_message_ref)?;
        if !is_sha256_digest(&command.request_digest) {
            anyhow::bail!("state_resource_task_batch_request_digest_invalid");
        }
        if command.tasks.is_empty() || command.tasks.len() > MAX_RESOURCE_TASK_BATCH_ITEMS {
            anyhow::bail!("state_resource_task_batch_size_invalid");
        }
        let ttl = command.expires_at - command.created_at;
        if ttl < Duration::hours(24) || ttl > Duration::days(7) {
            anyhow::bail!("state_resource_task_batch_ttl_out_of_range");
        }
        let mut normalized_titles = BTreeSet::new();
        let mut tasks = Vec::with_capacity(command.tasks.len());
        for (ordinal, task) in command.tasks.into_iter().enumerate() {
            validate_uuid_v4("state_resource_task_resource_id", &task.resource_id)?;
            if !is_sha256_digest(&task.content_digest) {
                anyhow::bail!("state_resource_task_content_digest_invalid");
            }
            let title = task.title.trim().to_string();
            if title.is_empty()
                || title.chars().count() > MAX_TASK_TITLE_CHARS
                || title.chars().any(char::is_control)
            {
                anyhow::bail!("state_resource_task_title_invalid");
            }
            if !normalized_titles.insert(title.to_lowercase()) {
                anyhow::bail!("state_resource_task_title_duplicate");
            }
            let payload_digest = digest_json(&serde_json::json!({
                "schema": "openlife.state-resource-task-item.v1",
                "batchOperationId": command.operation_id,
                "ordinal": ordinal,
                "sourceMessageRef": command.source_message_ref,
                "title": title,
                "resourceId": task.resource_id,
                "chunkOrdinal": task.chunk_ordinal,
                "contentDigest": task.content_digest,
                "ttlSeconds": ttl.num_seconds(),
            }))?;
            tasks.push(PreparedResourceTask {
                title,
                resource_id: task.resource_id,
                chunk_ordinal: task.chunk_ordinal,
                content_digest: task.content_digest,
                payload_digest,
            });
        }
        let payload_digest = digest_json(&serde_json::json!({
            "schema": "openlife.state-resource-task-batch.v1",
            "operationId": command.operation_id,
            "sourceMessageRef": command.source_message_ref,
            "tasks": tasks.iter().map(|task| serde_json::json!({
                "payloadDigest": task.payload_digest,
                "resourceId": task.resource_id,
                "chunkOrdinal": task.chunk_ordinal,
                "contentDigest": task.content_digest,
            })).collect::<Vec<_>>(),
            "ttlSeconds": ttl.num_seconds(),
        }))?;
        Ok(Self {
            operation_id: command.operation_id,
            request_digest: command.request_digest,
            source_message_ref: command.source_message_ref,
            tasks,
            created_at: command.created_at,
            expires_at: command.expires_at,
            payload_digest,
        })
    }

    fn validate_new_effect_timing(&self) -> Result<()> {
        if self.expires_at <= self.created_at {
            anyhow::bail!("state_resource_task_batch_expiry_invalid");
        }
        Ok(())
    }
}

struct PreparedCreate {
    operation_id: String,
    request_digest: String,
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
        if !command.confidence.is_finite() || !(0.0..=1.0).contains(&command.confidence) {
            anyhow::bail!("state_asset_confidence_invalid");
        }
        let payload_digest = digest_json(&serde_json::json!({
            "schema": "openlife.state-create-payload.v1",
            "operationId": command.operation_id,
            "sourceMessageRef": command.source_message_ref,
            "title": title,
            "dueAt": command.due_at.map(|value| value.to_rfc3339()),
            // Wall-clock observation is server-owned execution metadata, not
            // user payload. Bind the requested TTL instead so an exact retry
            // cannot drift merely because it resumed later.
            "ttlSeconds": ttl.num_seconds(),
            "risk": command.risk,
            "sensitivity": command.sensitivity,
            "sourceKind": command.source_kind,
            "confidence": command.confidence,
            "privacyClass": command.privacy_class,
        }))?;
        let request_digest = command
            .request_digest
            .unwrap_or_else(|| payload_digest.clone());
        if !is_sha256_digest(&request_digest) {
            anyhow::bail!("state_operation_request_digest_invalid");
        }
        Ok(Self {
            operation_id: command.operation_id,
            request_digest,
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

    fn validate_new_effect_timing(&self) -> Result<()> {
        if self
            .due_at
            .is_some_and(|due_at| due_at < self.created_at || due_at > self.expires_at)
        {
            anyhow::bail!("state_daily_task_due_at_out_of_range");
        }
        Ok(())
    }
}

struct PreparedTransition {
    operation_id: String,
    request_digest: String,
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
            // occurredAt records when the winning commit happened. It must
            // not turn an otherwise exact operation replay into payload drift.
            "sourceKind": source_kind,
        }))?;
        let request_digest = command
            .request_digest
            .unwrap_or_else(|| payload_digest.clone());
        if !is_sha256_digest(&request_digest) {
            anyhow::bail!("state_operation_request_digest_invalid");
        }
        Ok(Self {
            operation_id: command.operation_id,
            request_digest,
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
    request_digest: &str,
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
            operation_id, request_digest, payload_digest, receipt_id, asset_id, asset_version,
            mutation_kind, outbox_event_id, committed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            operation_id,
            request_digest,
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

fn replay_operation_request(
    conn: &Connection,
    operation_id: &str,
    request_digest: &str,
) -> Result<Option<StateExecutionReceipt>> {
    let existing_digest = conn
        .query_row(
            "SELECT request_digest FROM state_operations WHERE operation_id = ?1",
            [operation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(existing_digest) = existing_digest else {
        return Ok(None);
    };
    if existing_digest != request_digest {
        anyhow::bail!("state_operation_request_drift");
    }
    operation_receipt(conn, operation_id, true)
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

fn resource_task_batch_receipt(
    conn: &Connection,
    operation_id: &str,
    replayed: bool,
) -> Result<Option<StateBatchExecutionReceipt>> {
    let batch = conn
        .query_row(
            "SELECT receipt_id, payload_digest, item_count, committed_at, expires_at
             FROM state_resource_task_batch_operations WHERE operation_id = ?1",
            [operation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((receipt_id, payload_digest, item_count, committed_at, expires_at)) = batch else {
        return Ok(None);
    };
    let rows = {
        let mut statement = conn.prepare(
            "SELECT receipt_id, asset_id, asset_version, payload_digest,
                    outbox_event_id, resource_id, chunk_ordinal, content_digest
             FROM state_resource_task_batch_items
             WHERE operation_id = ?1 ORDER BY ordinal ASC",
        )?;
        let rows = statement
            .query_map([operation_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows
    };
    if rows.len() != usize::try_from(item_count).context("state_batch_item_count_invalid")? {
        anyhow::bail!("state_resource_task_batch_item_count_mismatch");
    }
    let mut assets = Vec::with_capacity(rows.len());
    for (
        receipt_id,
        asset_id,
        asset_version,
        payload_digest,
        outbox_event_id,
        resource_id,
        chunk_ordinal,
        content_digest,
    ) in rows
    {
        let projection = crate::persistence_outbox::projection_summary(conn, &outbox_event_id)?;
        let projection_status = match projection.state() {
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
        };
        assets.push(StateBatchAssetReceipt {
            receipt_id,
            asset_id,
            asset_version: u64::try_from(asset_version)
                .context("state_batch_asset_version_invalid")?,
            payload_digest,
            outbox_event_id,
            projection_status,
            resource_id,
            chunk_ordinal: u32::try_from(chunk_ordinal)
                .context("state_batch_chunk_ordinal_invalid")?,
            content_digest,
        });
    }
    Ok(Some(StateBatchExecutionReceipt {
        schema: "openlife.state-batch-execution-receipt.v1".into(),
        receipt_id,
        operation_id: operation_id.to_string(),
        payload_digest,
        canonical_status: "committed".into(),
        assets,
        committed_at: parse_time(&committed_at)?,
        expires_at: parse_time(&expires_at)?,
        replayed,
    }))
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

fn validate_resource_task_batch_grant(
    grant: &crate::agent::main_chat_agent_v1::PolicyTransientStateGrant,
) -> Result<()> {
    use crate::agent::main_chat_agent_v1::TransientStateCommandKind;

    let intent = grant.intent();
    if intent.command_kind != TransientStateCommandKind::CreateDailyTask
        || intent.reason_code != "explicit_resource_daily_task_batch"
        || !intent.target.is_empty()
        || intent.due_hint.is_some()
        || intent.expiry_days != 1
    {
        anyhow::bail!("state_resource_task_batch_policy_grant_invalid");
    }
    if !is_sha256_digest(grant.intent_digest())
        || !is_sha256_digest(grant.source_user_message_digest())
        || !is_sha256_digest(grant.policy_contract_digest())
    {
        anyhow::bail!("state_resource_task_batch_policy_digest_invalid");
    }
    Ok(())
}

fn resolve_active_task_target(store: &StateStore, target: &str) -> Result<StateAsset> {
    let normalized = target.trim().to_lowercase();
    if normalized.is_empty() {
        anyhow::bail!("state_gateway_task_target_missing");
    }
    let tasks = store.list_daily_tasks(false)?;
    let mut exact = tasks
        .iter()
        .filter(|task| task.title.trim().to_lowercase() == normalized)
        .cloned()
        .collect::<Vec<_>>();
    if exact.len() == 1 {
        return Ok(exact.remove(0));
    }
    if exact.len() > 1 {
        anyhow::bail!("state_gateway_task_target_ambiguous");
    }
    let mut partial = tasks
        .into_iter()
        .filter(|task| {
            let title = task.title.trim().to_lowercase();
            title.contains(&normalized) || normalized.contains(&title)
        })
        .collect::<Vec<_>>();
    if partial.len() == 1 {
        return Ok(partial.remove(0));
    }
    if partial.is_empty() {
        anyhow::bail!("state_gateway_task_target_not_found");
    }
    anyhow::bail!("state_gateway_task_target_ambiguous")
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
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
    use crate::agent::main_chat_agent_v1::{IntentFrame, PolicyRouter};
    use std::sync::atomic::{AtomicUsize, Ordering};
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
            request_digest: None,
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

    fn resource_task_batch_command(
        operation_id: String,
        titles: &[&str],
    ) -> CreateResourceDailyTaskBatchCommand {
        CreateResourceDailyTaskBatchCommand {
            operation_id,
            request_digest: digest_json(&serde_json::json!({
                "prompt": "create bounded tasks from current resource"
            }))
            .unwrap(),
            source_message_ref: format!("conversation://state-tests/message/{}", Uuid::new_v4()),
            tasks: titles
                .iter()
                .enumerate()
                .map(|(ordinal, title)| ResourceDailyTaskDraft {
                    title: (*title).into(),
                    resource_id: Uuid::new_v4().hyphenated().to_string(),
                    chunk_ordinal: u32::try_from(ordinal).unwrap(),
                    content_digest: digest_json(&serde_json::json!({"content": title})).unwrap(),
                })
                .collect(),
            created_at: at(9),
            expires_at: at(9) + Duration::days(1),
        }
    }

    fn transition(
        asset: &StateAsset,
        mutation_kind: StateMutationKind,
        occurred_at: DateTime<Utc>,
    ) -> TransitionDailyTaskCommand {
        TransitionDailyTaskCommand {
            operation_id: Uuid::new_v4().hyphenated().to_string(),
            request_digest: None,
            source_message_ref: Uuid::new_v4().hyphenated().to_string(),
            asset_id: asset.asset_id.clone(),
            expected_version: asset.version,
            mutation_kind,
            occurred_at,
        }
    }

    fn state_grant(
        operation_id: &str,
        prompt: &str,
    ) -> crate::agent::main_chat_agent_v1::PolicyTransientStateGrant {
        let mut intent = IntentFrame::from_user_message(prompt);
        intent.current_user_message_id =
            Some(format!("conversation://state-tests/message/{operation_id}"));
        let route = PolicyRouter.route(intent);
        let state_intent = route
            .intent_frame
            .transient_state_intent
            .as_ref()
            .expect("transient state intent");
        route
            .policy_decision
            .authorize_transient_state_command(operation_id, state_intent)
            .expect("transient state grant")
    }

    struct RejectingWriteAdmission {
        calls: AtomicUsize,
    }

    impl crate::agent::CanonicalWriteAdmission for RejectingWriteAdmission {
        fn acquire(
            &self,
            _request: crate::agent::CanonicalWriteAdmissionRequest,
        ) -> std::result::Result<
            Box<dyn crate::agent::CanonicalWritePermit>,
            crate::agent::CanonicalWriteAdmissionRejection,
        > {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(crate::agent::CanonicalWriteAdmissionRejection::new(
                "cancelled_after_original_commit",
            ))
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
    fn exact_create_replay_ignores_server_clock_drift_but_keeps_semantic_binding() {
        let store = StateStore::new_in_memory().unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let source_message_ref = Uuid::new_v4().hyphenated().to_string();
        let mut first_command = create_command(operation_id.clone(), "时间漂移重试");
        first_command.source_message_ref = source_message_ref.clone();
        first_command.due_at = None;
        let first = store.create_daily_task(first_command).unwrap();

        let mut retry_command = create_command(operation_id, "时间漂移重试");
        retry_command.source_message_ref = source_message_ref;
        retry_command.due_at = None;
        retry_command.created_at = at(10);
        retry_command.expires_at = at(10) + Duration::days(1);
        let retry = store.create_daily_task(retry_command).unwrap();

        assert!(retry.replayed);
        assert_eq!(first.asset_id, retry.asset_id);
        assert_eq!(first.committed_at, retry.committed_at);
        assert_eq!(store.list_daily_tasks(false).unwrap().len(), 1);
    }

    #[test]
    fn exact_transition_replay_ignores_server_clock_drift() {
        let store = StateStore::new_in_memory().unwrap();
        let created = store
            .create_daily_task(create_command(
                Uuid::new_v4().hyphenated().to_string(),
                "完成时间漂移重试",
            ))
            .unwrap();
        let asset = store.get_asset(&created.asset_id).unwrap().unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let source_message_ref = Uuid::new_v4().hyphenated().to_string();
        let mut first_command = transition(&asset, StateMutationKind::Complete, at(16));
        first_command.operation_id = operation_id.clone();
        first_command.source_message_ref = source_message_ref.clone();
        let first = store.complete_daily_task(first_command).unwrap();

        let mut retry_command = transition(&asset, StateMutationKind::Complete, at(17));
        retry_command.operation_id = operation_id;
        retry_command.source_message_ref = source_message_ref;
        let retry = store.complete_daily_task(retry_command).unwrap();

        assert!(retry.replayed);
        assert_eq!(first.asset_version, retry.asset_version);
        assert_eq!(first.committed_at, retry.committed_at);
        assert_eq!(
            store.get_asset(&asset.asset_id).unwrap().unwrap().version,
            2
        );
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

    #[test]
    fn state_gateway_consumes_policy_grant_and_reuses_same_operation_receipt() {
        let store = StateStore::new_in_memory().unwrap();
        let gateway = StateGateway::new(store.clone());
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let prompt = "今天下午三点前提醒我完成路演设备检查，完成后我还要能撤销。";
        let context = StateGatewayExecutionContext {
            occurred_at: at(9),
            resolved_due_at: Some(at(15)),
        };
        let first = gateway
            .execute(state_grant(&operation_id, prompt), context.clone())
            .unwrap();
        let replay = gateway
            .execute(
                state_grant(&operation_id, prompt),
                StateGatewayExecutionContext {
                    occurred_at: at(16),
                    resolved_due_at: context.resolved_due_at,
                },
            )
            .unwrap();
        assert_eq!(first.tasks.len(), 1);
        assert_eq!(first.tasks[0].title, "完成路演设备检查");
        assert!(!first.receipt.as_ref().unwrap().replayed);
        assert!(replay.receipt.as_ref().unwrap().replayed);
        assert_eq!(
            first.receipt.as_ref().unwrap().asset_id,
            replay.receipt.as_ref().unwrap().asset_id
        );

        let late_error = gateway
            .execute(
                state_grant(&Uuid::new_v4().hyphenated().to_string(), prompt),
                StateGatewayExecutionContext {
                    occurred_at: at(16),
                    resolved_due_at: Some(at(15)),
                },
            )
            .unwrap_err();
        assert!(late_error
            .to_string()
            .contains("state_daily_task_due_at_out_of_range"));
    }

    #[test]
    fn resource_task_batch_commits_atomically_replays_and_rejects_drift() {
        let store = StateStore::new_in_memory().unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let command = resource_task_batch_command(
            operation_id.clone(),
            &["检查投影器", "检查离线演示账号", "验证撤销与过期"],
        );
        let first = store.create_resource_task_batch(command.clone()).unwrap();
        let replay = store.create_resource_task_batch(command.clone()).unwrap();

        assert_eq!(first.assets.len(), 3);
        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(first.receipt_id, replay.receipt_id);
        assert_eq!(first.assets, replay.assets);
        let tasks = store.list_daily_tasks(false).unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(
            tasks
                .iter()
                .map(|task| task.title.as_str())
                .collect::<Vec<_>>(),
            ["检查投影器", "检查离线演示账号", "验证撤销与过期"]
        );
        let encoded_receipt = serde_json::to_string(&first).unwrap();
        for title in ["检查投影器", "检查离线演示账号", "验证撤销与过期"] {
            assert!(!encoded_receipt.contains(title));
        }

        let mut payload_drift = command.clone();
        payload_drift.tasks[0].title = "不同任务".into();
        assert!(store
            .create_resource_task_batch(payload_drift)
            .unwrap_err()
            .to_string()
            .contains("state_resource_task_batch_payload_drift"));

        let mut request_drift = command;
        request_drift.request_digest = digest_json(&serde_json::json!({
            "prompt": "different current-user request"
        }))
        .unwrap();
        assert!(store
            .create_resource_task_batch(request_drift)
            .unwrap_err()
            .to_string()
            .contains("state_resource_task_batch_request_drift"));
    }

    #[test]
    fn concurrent_same_resource_task_batch_has_one_canonical_commit() {
        let store = StateStore::new_in_memory().unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let command = resource_task_batch_command(
            operation_id.clone(),
            &["检查投影器", "检查离线演示账号", "验证撤销与过期"],
        );
        let barrier = Arc::new(Barrier::new(3));
        let handles = (0..2)
            .map(|_| {
                let store = store.clone();
                let command = command.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    store.create_resource_task_batch(command).unwrap()
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let receipts = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(receipts[0].receipt_id, receipts[1].receipt_id);
        assert_eq!(
            receipts.iter().filter(|receipt| !receipt.replayed).count(),
            1
        );
        assert_eq!(
            receipts.iter().filter(|receipt| receipt.replayed).count(),
            1
        );
        assert_eq!(store.list_daily_tasks(false).unwrap().len(), 3);
        assert_eq!(
            store
                .resource_task_batch_receipt_for_operation(&operation_id, false)
                .unwrap()
                .unwrap()
                .assets
                .len(),
            3
        );
    }

    #[test]
    fn resource_task_batch_guard_failure_leaves_zero_partial_assets_or_receipt() {
        let store = StateStore::new_in_memory().unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let command =
            resource_task_batch_command(operation_id.clone(), &["任务 A", "任务 B", "任务 C"]);

        let error = store
            .create_resource_task_batch_guarded(command, || {
                anyhow::bail!("resource_task_batch_cancelled_before_commit")
            })
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("resource_task_batch_cancelled_before_commit"));
        assert!(store.list_daily_tasks(true).unwrap().is_empty());
        assert!(store
            .resource_task_batch_receipt_for_operation(&operation_id, false)
            .unwrap()
            .is_none());
        assert!(store
            .list_replayable_projection_deliveries(10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn state_gateway_replays_complete_and_undo_after_current_state_changes() {
        let store = StateStore::new_in_memory().unwrap();
        let gateway = StateGateway::new(store.clone());
        store
            .create_daily_task(create_command(
                Uuid::new_v4().hyphenated().to_string(),
                "完成路演设备检查",
            ))
            .unwrap();
        let context = StateGatewayExecutionContext {
            occurred_at: at(16),
            resolved_due_at: None,
        };

        let complete_operation = Uuid::new_v4().hyphenated().to_string();
        let complete_prompt = "/goal done 完成路演设备检查";
        let completed = gateway
            .execute(
                state_grant(&complete_operation, complete_prompt),
                context.clone(),
            )
            .unwrap();
        let completed_replay = gateway
            .execute(
                state_grant(&complete_operation, complete_prompt),
                StateGatewayExecutionContext {
                    occurred_at: at(17),
                    resolved_due_at: None,
                },
            )
            .unwrap();
        assert_eq!(completed.receipt.as_ref().unwrap().asset_version, 2);
        assert!(completed_replay.receipt.as_ref().unwrap().replayed);
        assert_eq!(
            completed.receipt.as_ref().unwrap().receipt_id,
            completed_replay.receipt.as_ref().unwrap().receipt_id
        );

        let undo_operation = Uuid::new_v4().hyphenated().to_string();
        let undo_prompt = "/goal undo 完成路演设备检查";
        let undone = gateway
            .execute(state_grant(&undo_operation, undo_prompt), context.clone())
            .unwrap();
        assert!(store.list_daily_tasks(false).unwrap().is_empty());
        let undone_replay = gateway
            .execute(
                state_grant(&undo_operation, undo_prompt),
                StateGatewayExecutionContext {
                    occurred_at: at(18),
                    resolved_due_at: None,
                },
            )
            .unwrap();
        assert_eq!(undone.receipt.as_ref().unwrap().asset_version, 3);
        assert!(undone_replay.receipt.as_ref().unwrap().replayed);
        assert_eq!(
            undone.receipt.as_ref().unwrap().receipt_id,
            undone_replay.receipt.as_ref().unwrap().receipt_id
        );

        let drift = gateway
            .execute(
                state_grant(&undo_operation, "/goal undo 另一项任务"),
                context,
            )
            .unwrap_err();
        assert!(drift.to_string().contains("state_operation_request_drift"));
    }

    #[test]
    fn committed_receipt_recovery_opens_no_second_write_admission_window() {
        let store = StateStore::new_in_memory().unwrap();
        let gateway = StateGateway::new(store);
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let prompt = "/goal add 已经提交的任务";
        let context = StateGatewayExecutionContext {
            occurred_at: at(9),
            resolved_due_at: None,
        };
        let committed = gateway
            .execute(state_grant(&operation_id, prompt), context.clone())
            .unwrap();
        let rejecting_admission = RejectingWriteAdmission {
            calls: AtomicUsize::new(0),
        };

        let recovered = gateway
            .execute_with_admission(
                state_grant(&operation_id, prompt),
                context,
                &rejecting_admission,
            )
            .unwrap();
        assert!(recovered.receipt.as_ref().unwrap().replayed);
        assert_eq!(
            recovered.receipt.as_ref().unwrap().receipt_id,
            committed.receipt.as_ref().unwrap().receipt_id
        );
        assert_eq!(rejecting_admission.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn schema_v1_migrates_request_digest_without_losing_operation_truth() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state-v1.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE state_store_metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                 ) WITHOUT ROWID;
                 INSERT INTO state_store_metadata (key, value)
                 VALUES ('schema_version', '1');
                 CREATE TABLE state_operations (
                    operation_id TEXT PRIMARY KEY,
                    payload_digest TEXT NOT NULL,
                    receipt_id TEXT NOT NULL UNIQUE,
                    asset_id TEXT NOT NULL,
                    asset_version INTEGER NOT NULL,
                    mutation_kind TEXT NOT NULL,
                    outbox_event_id TEXT NOT NULL UNIQUE,
                    committed_at TEXT NOT NULL
                 ) WITHOUT ROWID;",
            )
            .unwrap();
        }

        let store = StateStore::new(&path).unwrap();
        let conn = store.lock_connection().unwrap();
        let version: String = conn
            .query_row(
                "SELECT value FROM state_store_metadata WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let request_digest_column_exists = conn
            .prepare("PRAGMA table_info(state_operations)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
            .iter()
            .any(|column| column == "request_digest");
        assert_eq!(version, "3");
        assert!(request_digest_column_exists);
    }

    #[test]
    fn schema_v2_adds_resource_task_batch_tables_before_advancing_version() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state-v2.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE state_store_metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                 ) WITHOUT ROWID;
                 INSERT INTO state_store_metadata (key, value)
                 VALUES ('schema_version', '2');",
            )
            .unwrap();
        }

        let store = StateStore::new(&path).unwrap();
        let conn = store.lock_connection().unwrap();
        let version: String = conn
            .query_row(
                "SELECT value FROM state_store_metadata WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let tables = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type = 'table' AND name LIKE 'state_resource_task_batch_%'
                 ORDER BY name ASC",
            )
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(version, "3");
        assert_eq!(
            tables,
            [
                "state_resource_task_batch_items",
                "state_resource_task_batch_operations"
            ]
        );
    }

    #[test]
    fn state_gateway_commit_guard_failure_leaves_no_canonical_effect() {
        let store = StateStore::new_in_memory().unwrap();
        let gateway = StateGateway::new(store.clone());
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let error = gateway
            .execute_guarded(
                state_grant(&operation_id, "/goal add 不应提交"),
                StateGatewayExecutionContext {
                    occurred_at: at(9),
                    resolved_due_at: None,
                },
                || anyhow::bail!("state_gateway_cancelled_before_commit"),
            )
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("state_gateway_cancelled_before_commit"));
        assert!(store.list_daily_tasks(true).unwrap().is_empty());
        assert!(store
            .receipt_for_operation(&operation_id, false)
            .unwrap()
            .is_none());
    }
}
