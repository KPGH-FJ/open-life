//! Canonical Task/Item/Artifact metadata for the general Agent runtime.
//!
//! This store owns stable Task identity, Run membership, typed Items, and
//! Artifact versions. Reports use the full Artifact lifecycle; ordinary plans
//! use Instruction + Plan Items without a parallel planning lifecycle.
//! Work execution identity and terminal state are owned here. Capability
//! adapters may retain their own typed receipts, but they do not own Task or
//! Run lifecycle state.

use crate::work_orchestration::{StructuredWorkPlan, WorkRunBudgetPolicy, WorkRunBudgetUsage};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

const TASK_RUNTIME_SCHEMA_VERSION: i64 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalTaskStatus {
    Running,
    WaitingReview,
    Completed,
    Blocked,
    Failed,
    Cancelled,
    Interrupted,
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
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
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
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
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
    Running,
    Completed,
    Blocked,
    Failed,
    Cancelled,
    Interrupted,
    EffectUnknown,
}

impl CanonicalTaskItemStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
            Self::EffectUnknown => "effect_unknown",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "waiting" => Ok(Self::Waiting),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "blocked" => Ok(Self::Blocked),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
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
    pub materialized_reference: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalArtifactReviewCheckpointRecord {
    pub artifact_id: String,
    pub version: u64,
    pub proposal_id: String,
    pub item_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalArtifactEffectState {
    Prepared,
    Staged,
    Confirmed,
    FailedBeforeEffect,
    EffectUnknown,
}

impl CanonicalArtifactEffectState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Staged => "staged",
            Self::Confirmed => "confirmed",
            Self::FailedBeforeEffect => "failed_before_effect",
            Self::EffectUnknown => "effect_unknown",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "staged" => Ok(Self::Staged),
            "confirmed" => Ok(Self::Confirmed),
            "failed_before_effect" => Ok(Self::FailedBeforeEffect),
            "effect_unknown" => Ok(Self::EffectUnknown),
            _ => anyhow::bail!("canonical_artifact_effect_state_invalid:{value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalArtifactEffectRecord {
    pub artifact_id: String,
    pub version: u64,
    pub proposal_id: String,
    pub attempt_id: String,
    pub dispatch_claim_id: String,
    pub target_reference_digest: String,
    pub content_digest: String,
    pub byte_size: u64,
    pub media_type: String,
    pub state: CanonicalArtifactEffectState,
    pub observed_content_digest: Option<String>,
    pub error_code: Option<String>,
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
    pub target_reference: Option<String>,
    pub draft_reference: Option<String>,
    pub expected_target_absent: Option<bool>,
    pub expected_target_digest: Option<String>,
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
    pub status: CanonicalTaskStatus,
    pub execution_facts_version: u64,
    pub plan_revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub project_id: Option<String>,
    pub project_revision: Option<u64>,
    pub scope_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalTaskItemAttemptRecord {
    pub attempt_id: String,
    pub task_id: String,
    pub run_id: String,
    pub item_id: String,
    pub ordinal: u64,
    pub status: CanonicalTaskItemStatus,
    pub executor_kind: String,
    pub provider_profile_id: Option<String>,
    pub provider_model_id: Option<String>,
    pub request_digest: String,
    pub receipt_digest: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalWorkPlanRecord {
    pub task_id: String,
    pub run_id: String,
    pub plan_revision: u64,
    pub plan: StructuredWorkPlan,
    pub plan_digest: String,
    pub budget_policy: WorkRunBudgetPolicy,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalFinalResultRecord {
    pub task_id: String,
    pub run_id: String,
    pub item_id: String,
    pub conversation_item_id: String,
    pub result_digest: String,
    pub summary_code: String,
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
pub struct CanonicalSteeringRecord {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalAttentionKind {
    ReviewRequired,
    Blocked,
    Failed,
    EffectUnknown,
    ScopeStale,
}

impl CanonicalAttentionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ReviewRequired => "review_required",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::EffectUnknown => "effect_unknown",
            Self::ScopeStale => "scope_stale",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "review_required" => Ok(Self::ReviewRequired),
            "blocked" => Ok(Self::Blocked),
            "failed" => Ok(Self::Failed),
            "effect_unknown" => Ok(Self::EffectUnknown),
            "scope_stale" => Ok(Self::ScopeStale),
            _ => anyhow::bail!("canonical_attention_kind_invalid:{value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalAttentionRecord {
    pub attention_id: String,
    pub task_id: String,
    pub run_id: String,
    pub kind: CanonicalAttentionKind,
    pub reason_code: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalArtifactSnapshot {
    pub artifact: CanonicalArtifactRecord,
    pub current_version: CanonicalArtifactVersionRecord,
    pub review_checkpoint: Option<CanonicalArtifactReviewCheckpointRecord>,
    pub undo: Option<CanonicalArtifactUndoRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalArtifactUndoRecord {
    pub artifact_id: String,
    pub version: u64,
    pub proposal_id: String,
    pub source_reference: String,
    pub target_reference: String,
    pub content_digest: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalTaskSnapshot {
    pub task: CanonicalTaskRecord,
    pub runs: Vec<CanonicalTaskRunRecord>,
    pub items: Vec<CanonicalTaskItemRecord>,
    pub attempts: Vec<CanonicalTaskItemAttemptRecord>,
    pub final_result: Option<CanonicalFinalResultRecord>,
    pub artifacts: Vec<CanonicalArtifactSnapshot>,
    pub attention: Vec<CanonicalAttentionRecord>,
}

#[derive(Clone, Copy)]
pub struct BeginGeneralTaskRunInput<'a> {
    pub task_id: &'a str,
    pub conversation_id: &'a str,
    pub run_id: &'a str,
    pub execution_session_id: &'a str,
    pub instruction_digest: &'a str,
    pub plan_digest: Option<&'a str>,
    pub project_id: Option<&'a str>,
    pub project_revision: Option<u64>,
    pub scope_digest: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BegunGeneralTaskRun {
    pub task_id: String,
    pub run_id: String,
    pub instruction_item_id: String,
    pub plan_item_id: Option<String>,
    pub ordinal: u64,
    pub plan_revision: u64,
}

pub struct BeginItemAttemptInput<'a> {
    pub attempt_id: &'a str,
    pub task_id: &'a str,
    pub run_id: &'a str,
    pub item_id: &'a str,
    pub executor_kind: &'a str,
    pub provider_profile_id: Option<&'a str>,
    pub provider_model_id: Option<&'a str>,
    pub request_digest: &'a str,
}

pub struct CompleteGeneralTaskInput<'a> {
    pub task_id: &'a str,
    pub run_id: &'a str,
    pub final_item_id: &'a str,
    pub conversation_item_id: &'a str,
    pub result_digest: &'a str,
    pub summary_code: &'a str,
}

pub struct DeferGeneralTaskResultInput<'a> {
    pub task_id: &'a str,
    pub run_id: &'a str,
    pub conversation_item_id: &'a str,
    pub result_digest: &'a str,
    pub summary_code: &'a str,
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

/// A user-visible file result prepared by the general Work runtime.
///
/// The caller supplies the exact Task/Run identities that already own the
/// provider and tool attempts. The Task store adds only Artifact metadata; the
/// ReviewWorkflow owns only the decision checkpoint. Artifact identity and
/// version state remain canonical here; file bytes are owned by the workspace
/// draft/materialization layer.
pub struct GeneralArtifactDraftInput<'a> {
    pub task_id: &'a str,
    pub run_id: &'a str,
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

pub struct SubmitSteeringInput<'a> {
    pub steering_id: &'a str,
    pub task_id: &'a str,
    pub run_id: &'a str,
    pub source_message_ref: &'a str,
    pub source_message_digest: &'a str,
    pub steering_digest: &'a str,
    pub base_plan_revision: u64,
    pub scope_expansion_blocked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalRunSteeringTarget {
    pub task_id: String,
    pub execution_session_id: String,
    pub plan_revision: u64,
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

pub type PreparedGeneralArtifact = PreparedReportArtifact;

pub fn general_artifact_id(task_id: &str, target_reference: &str, media_type: &str) -> String {
    stable_id(
        "artifact",
        &[task_id, &sha256_text(target_reference), media_type],
    )
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
    pub version: u64,
    pub checkpoint_item_id: String,
    pub proposal_id: String,
}

#[derive(Clone)]
pub struct CanonicalTaskRuntimeStore {
    conn: Arc<Mutex<Connection>>,
    db_path: Option<PathBuf>,
    receipt_key: Option<crate::agent::CanonicalTaskReceiptKey>,
    store_identity: String,
}

impl CanonicalTaskRuntimeStore {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        Self::new_internal(path.into(), None)
    }

    pub fn new_with_receipt_key(
        path: impl Into<PathBuf>,
        installation_key: crate::agent::CanonicalTaskReceiptKey,
    ) -> Result<Self> {
        let path = path.into();
        Self::new_internal(path, Some(installation_key))
    }

    fn new_internal(
        path: PathBuf,
        installation_key: Option<crate::agent::CanonicalTaskReceiptKey>,
    ) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        let receipt_key = installation_key
            .map(|key| {
                let canonical_path = crate::sqlite_migration::canonical_opened_main_database_path(
                    &conn,
                    "canonical_task_runtime_store",
                )?
                .ok_or_else(|| {
                    anyhow::anyhow!("canonical_task_runtime_persistent_database_path_missing")
                })?;
                key.derive_for_canonical_database_slot(&canonical_path)
            })
            .transpose()?;
        let mut store = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: Some(path),
            receipt_key,
            store_identity: String::new(),
        };
        store.initialize()?;
        store.store_identity = store.load_store_identity()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        Self::new_in_memory_internal(None)
    }

    pub fn new_in_memory_with_receipt_key(
        receipt_key: crate::agent::CanonicalTaskReceiptKey,
    ) -> Result<Self> {
        Self::new_in_memory_internal(Some(receipt_key))
    }

    fn new_in_memory_internal(
        receipt_key: Option<crate::agent::CanonicalTaskReceiptKey>,
    ) -> Result<Self> {
        let mut store = Self {
            conn: Arc::new(Mutex::new(Connection::open_in_memory()?)),
            db_path: None,
            receipt_key,
            store_identity: String::new(),
        };
        store.initialize()?;
        store.store_identity = store.load_store_identity()?;
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
                "canonical_task_item_attempts",
                "canonical_work_plans",
                "canonical_work_plan_revisions",
                "canonical_task_final_results",
                "canonical_task_deferred_results",
                "canonical_steering",
                "canonical_task_attention",
                "canonical_artifacts",
                "canonical_artifact_versions",
                "canonical_artifact_review_checkpoints",
                "canonical_artifact_effects",
                "canonical_artifact_undo",
            ],
        )?;
        Self::validate_schema(&conn)?;
        let store_identity: String = conn.query_row(
            "SELECT value FROM canonical_task_runtime_metadata WHERE key = 'store_identity'",
            [],
            |row| row.get(0),
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: Some(path.to_path_buf()),
            receipt_key: None,
            store_identity,
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
                conversation_id TEXT NOT NULL,
                task_kind TEXT NOT NULL CHECK(task_kind IN ('work', 'report', 'plan')),
                initial_outcome_digest TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN (
                    'running', 'waiting_review', 'completed', 'blocked',
                    'failed', 'cancelled', 'interrupted', 'effect_unknown'
                )),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS canonical_task_runs (
                task_id TEXT NOT NULL,
                run_id TEXT NOT NULL UNIQUE,
                execution_session_id TEXT NOT NULL,
                ordinal INTEGER NOT NULL CHECK(ordinal > 0),
                status TEXT NOT NULL DEFAULT 'running' CHECK(status IN (
                    'running', 'waiting_review', 'completed', 'blocked',
                    'failed', 'cancelled', 'interrupted', 'effect_unknown'
                )),
                execution_facts_version INTEGER NOT NULL DEFAULT 5
                    CHECK(execution_facts_version IN (1, 2, 3, 4, 5)),
                plan_revision INTEGER NOT NULL DEFAULT 1 CHECK(plan_revision > 0),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_at TEXT,
                project_id TEXT,
                project_revision INTEGER CHECK(project_revision > 0),
                scope_digest TEXT,
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
                    'waiting', 'running', 'completed', 'blocked', 'failed',
                    'cancelled', 'interrupted', 'effect_unknown'
                )),
                summary_code TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(task_id, sequence),
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             );
             CREATE TABLE IF NOT EXISTS canonical_steering (
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
             CREATE TABLE IF NOT EXISTS canonical_task_item_attempts (
                attempt_id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                item_id TEXT NOT NULL,
                ordinal INTEGER NOT NULL CHECK(ordinal > 0),
                status TEXT NOT NULL CHECK(status IN (
                    'running', 'completed', 'blocked', 'failed', 'cancelled',
                    'interrupted', 'effect_unknown'
                )),
                executor_kind TEXT NOT NULL CHECK(executor_kind IN (
                    'provider', 'tool', 'internal', 'review', 'materializer'
                )),
                provider_profile_id TEXT,
                provider_model_id TEXT,
                request_digest TEXT NOT NULL,
                receipt_digest TEXT,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                UNIQUE(item_id, ordinal),
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT,
                FOREIGN KEY(item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
             );
             CREATE TABLE IF NOT EXISTS canonical_work_plans (
                run_id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                plan_revision INTEGER NOT NULL CHECK(plan_revision > 0),
                schema_version TEXT NOT NULL,
                plan_json TEXT NOT NULL,
                plan_digest TEXT NOT NULL,
                max_plan_attempts INTEGER NOT NULL CHECK(max_plan_attempts > 0),
                max_provider_attempts INTEGER NOT NULL CHECK(max_provider_attempts > 0),
                max_tool_attempts INTEGER NOT NULL CHECK(max_tool_attempts > 0),
                max_total_items INTEGER NOT NULL CHECK(max_total_items > 0),
                created_at TEXT NOT NULL,
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS canonical_work_plan_revisions (
                run_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                plan_revision INTEGER NOT NULL CHECK(plan_revision > 0),
                schema_version TEXT NOT NULL,
                plan_json TEXT NOT NULL,
                plan_digest TEXT NOT NULL,
                max_plan_attempts INTEGER NOT NULL CHECK(max_plan_attempts > 0),
                max_provider_attempts INTEGER NOT NULL CHECK(max_provider_attempts > 0),
                max_tool_attempts INTEGER NOT NULL CHECK(max_tool_attempts > 0),
                max_total_items INTEGER NOT NULL CHECK(max_total_items > 0),
                created_at TEXT NOT NULL,
                PRIMARY KEY(run_id, plan_revision),
                UNIQUE(task_id, run_id, plan_revision),
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS canonical_task_final_results (
                task_id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                item_id TEXT NOT NULL UNIQUE,
                conversation_item_id TEXT NOT NULL UNIQUE,
                result_digest TEXT NOT NULL,
                summary_code TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT,
                FOREIGN KEY(item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
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
                materialized_reference TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT,
                FOREIGN KEY(source_item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
             );
             CREATE TABLE IF NOT EXISTS canonical_task_deferred_results (
                task_id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL UNIQUE,
                conversation_item_id TEXT NOT NULL,
                result_digest TEXT NOT NULL,
                summary_code TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS canonical_task_attention (
                attention_id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN (
                    'review_required','blocked','failed','effect_unknown','scope_stale'
                )),
                reason_code TEXT NOT NULL,
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                UNIQUE(task_id, run_id, kind, reason_code),
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
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
                target_reference TEXT,
                draft_reference TEXT,
                expected_target_absent INTEGER CHECK(expected_target_absent IN (0, 1)),
                expected_target_digest TEXT,
                PRIMARY KEY(artifact_id, version),
                FOREIGN KEY(artifact_id) REFERENCES canonical_artifacts(id) ON DELETE RESTRICT,
                FOREIGN KEY(source_item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS canonical_artifact_review_checkpoints (
                artifact_id TEXT NOT NULL,
                version INTEGER NOT NULL CHECK(version > 0),
                proposal_id TEXT NOT NULL UNIQUE,
                item_id TEXT NOT NULL UNIQUE,
                status TEXT NOT NULL CHECK(status IN (
                    'waiting', 'accepted', 'rejected', 'failed', 'effect_unknown'
                )),
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                PRIMARY KEY(artifact_id, version),
                FOREIGN KEY(artifact_id, version)
                    REFERENCES canonical_artifact_versions(artifact_id, version)
                    ON DELETE RESTRICT,
                FOREIGN KEY(item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS canonical_artifact_effects (
                proposal_id TEXT PRIMARY KEY,
                artifact_id TEXT NOT NULL,
                version INTEGER NOT NULL CHECK(version > 0),
                attempt_id TEXT NOT NULL UNIQUE,
                dispatch_claim_id TEXT NOT NULL,
                target_reference_digest TEXT NOT NULL,
                content_digest TEXT NOT NULL,
                byte_size INTEGER NOT NULL CHECK(byte_size >= 0),
                media_type TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN (
                    'prepared', 'staged', 'confirmed',
                    'failed_before_effect', 'effect_unknown'
                )),
                observed_content_digest TEXT,
                error_code TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(artifact_id, version)
                    REFERENCES canonical_artifact_versions(artifact_id, version)
                    ON DELETE RESTRICT,
                FOREIGN KEY(attempt_id)
                    REFERENCES canonical_task_item_attempts(attempt_id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS canonical_artifact_undo (
                artifact_id TEXT NOT NULL,
                version INTEGER NOT NULL CHECK(version > 0),
                proposal_id TEXT NOT NULL UNIQUE,
                source_reference TEXT NOT NULL,
                target_reference TEXT NOT NULL,
                content_digest TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN (
                    'waiting_review', 'undone', 'failed', 'effect_unknown'
                )),
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                PRIMARY KEY(artifact_id, version),
                FOREIGN KEY(artifact_id, version)
                    REFERENCES canonical_artifact_versions(artifact_id, version)
                    ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_canonical_task_items_run
                ON canonical_task_items(run_id, sequence);
             CREATE INDEX IF NOT EXISTS idx_canonical_tasks_conversation
                ON canonical_tasks(conversation_id, updated_at DESC, id);
             CREATE INDEX IF NOT EXISTS idx_canonical_task_attempts_run
                ON canonical_task_item_attempts(run_id, started_at, attempt_id);
             CREATE INDEX IF NOT EXISTS idx_canonical_artifacts_task
                ON canonical_artifacts(task_id, created_at, id);
             INSERT INTO canonical_task_runtime_metadata(key, value)
             VALUES ('schema_version', '16')
             ON CONFLICT(key) DO NOTHING;
             INSERT INTO canonical_task_runtime_metadata(key, value)
             VALUES ('store_identity', 'canonical_task_runtime_store:' || lower(hex(randomblob(16))))
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
        if Self::schema_version(&conn)? == 6 {
            Self::migrate_v6_to_v7(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 7 {
            Self::migrate_v7_to_v8(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 8 {
            Self::migrate_v8_to_v9(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 9 {
            Self::migrate_v9_to_v10(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 10 {
            Self::migrate_v10_to_v11(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 11 {
            Self::migrate_v11_to_v12(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 12 {
            Self::migrate_v12_to_v13(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 13 {
            Self::migrate_v13_to_v14(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 14 {
            Self::migrate_v14_to_v15(&mut conn)?;
        }
        if Self::schema_version(&conn)? == 15 {
            Self::migrate_v15_to_v16(&mut conn)?;
        }
        Self::validate_schema(&conn)
    }

    fn load_store_identity(&self) -> Result<String> {
        let identity: String = self.lock_conn()?.query_row(
            "SELECT value FROM canonical_task_runtime_metadata WHERE key = 'store_identity'",
            [],
            |row| row.get(0),
        )?;
        let suffix = identity
            .strip_prefix("canonical_task_runtime_store:")
            .ok_or_else(|| anyhow::anyhow!("canonical_task_runtime_store_identity_invalid"))?;
        if suffix.len() != 32 || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            anyhow::bail!("canonical_task_runtime_store_identity_invalid");
        }
        let uuid = format!(
            "{}-{}-{}-{}-{}",
            &suffix[0..8],
            &suffix[8..12],
            &suffix[12..16],
            &suffix[16..20],
            &suffix[20..32]
        );
        let parsed = uuid::Uuid::parse_str(&uuid)
            .map_err(|_| anyhow::anyhow!("canonical_task_runtime_store_identity_invalid"))?;
        Ok(format!("canonical_task_runtime_store:{parsed}"))
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

    fn migrate_v6_to_v7(conn: &mut Connection) -> Result<()> {
        conn.execute_batch("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = ON;")?;
        let migration = (|| -> Result<()> {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(
                "DROP TABLE IF EXISTS canonical_task_final_results;
                 DROP TABLE IF EXISTS canonical_task_item_attempts;
                 ALTER TABLE canonical_task_items RENAME TO canonical_task_items_v6;
                 ALTER TABLE canonical_task_runs RENAME TO canonical_task_runs_v6;
                 ALTER TABLE canonical_tasks RENAME TO canonical_tasks_v6;
                 CREATE TABLE canonical_tasks (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    task_kind TEXT NOT NULL CHECK(task_kind IN ('work', 'report', 'plan')),
                    initial_outcome_digest TEXT NOT NULL,
                    status TEXT NOT NULL CHECK(status IN (
                        'running', 'waiting_review', 'completed', 'blocked',
                        'failed', 'cancelled', 'interrupted', 'effect_unknown'
                    )),
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                 );
                 INSERT INTO canonical_tasks
                 SELECT * FROM canonical_tasks_v6;
                 CREATE TABLE canonical_task_runs (
                    task_id TEXT NOT NULL,
                    run_id TEXT NOT NULL UNIQUE,
                    execution_session_id TEXT NOT NULL,
                    ordinal INTEGER NOT NULL CHECK(ordinal > 0),
                    status TEXT NOT NULL CHECK(status IN (
                        'running', 'waiting_review', 'completed', 'blocked',
                        'failed', 'cancelled', 'interrupted', 'effect_unknown'
                    )),
                    execution_facts_version INTEGER NOT NULL DEFAULT 5
                        CHECK(execution_facts_version IN (1, 2, 3, 4, 5)),
                    plan_revision INTEGER NOT NULL DEFAULT 1 CHECK(plan_revision > 0),
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    completed_at TEXT,
                    PRIMARY KEY(task_id, run_id),
                    FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT
                 ) WITHOUT ROWID;
                 INSERT INTO canonical_task_runs (
                    task_id, run_id, execution_session_id, ordinal, status,
                    execution_facts_version, plan_revision, created_at, updated_at,
                    completed_at
                 ) SELECT run.task_id, run.run_id, run.execution_session_id, run.ordinal,
                          CASE task.status
                            WHEN 'completed' THEN 'completed'
                            WHEN 'failed' THEN 'failed'
                            WHEN 'blocked' THEN 'blocked'
                            WHEN 'effect_unknown' THEN 'effect_unknown'
                            ELSE 'running'
                          END,
                          run.execution_facts_version, run.plan_revision, run.created_at,
                          task.updated_at,
                          CASE WHEN task.status = 'completed' THEN task.updated_at ELSE NULL END
                   FROM canonical_task_runs_v6 run
                   JOIN canonical_tasks_v6 task ON task.id = run.task_id;
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
                        'waiting', 'running', 'completed', 'blocked', 'failed',
                        'cancelled', 'interrupted', 'effect_unknown'
                    )),
                    summary_code TEXT NOT NULL,
                    payload_digest TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    UNIQUE(task_id, sequence),
                    FOREIGN KEY(task_id, run_id)
                        REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
                 );
                 INSERT INTO canonical_task_items SELECT * FROM canonical_task_items_v6;
                 DROP TABLE canonical_task_items_v6;
                 DROP TABLE canonical_task_runs_v6;
                 DROP TABLE canonical_tasks_v6;
                 CREATE TABLE canonical_task_item_attempts (
                    attempt_id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    item_id TEXT NOT NULL,
                    ordinal INTEGER NOT NULL CHECK(ordinal > 0),
                    status TEXT NOT NULL CHECK(status IN (
                        'running', 'completed', 'blocked', 'failed', 'cancelled',
                        'interrupted', 'effect_unknown'
                    )),
                    executor_kind TEXT NOT NULL CHECK(executor_kind IN (
                        'provider', 'tool', 'internal', 'review', 'materializer'
                    )),
                    provider_profile_id TEXT,
                    provider_model_id TEXT,
                    request_digest TEXT NOT NULL,
                    receipt_digest TEXT,
                    started_at TEXT NOT NULL,
                    finished_at TEXT,
                    UNIQUE(item_id, ordinal),
                    FOREIGN KEY(task_id, run_id)
                        REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT,
                    FOREIGN KEY(item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
                 );
                 CREATE TABLE canonical_task_final_results (
                    task_id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    item_id TEXT NOT NULL UNIQUE,
                    conversation_item_id TEXT NOT NULL UNIQUE,
                    result_digest TEXT NOT NULL,
                    summary_code TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(task_id, run_id)
                        REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT,
                    FOREIGN KEY(item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
                 ) WITHOUT ROWID;
                 CREATE INDEX idx_canonical_tasks_conversation
                    ON canonical_tasks(conversation_id, updated_at DESC, id);
                 CREATE INDEX idx_canonical_task_items_run
                    ON canonical_task_items(run_id, sequence);
                 CREATE INDEX idx_canonical_task_attempts_run
                    ON canonical_task_item_attempts(run_id, started_at, attempt_id);",
            )?;
            let changed = tx.execute(
                "UPDATE canonical_task_runtime_metadata SET value = '7'
                 WHERE key = 'schema_version' AND value = '6'",
                [],
            )?;
            if changed != 1 {
                anyhow::bail!("canonical_task_runtime_v6_migration_version_conflict");
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
            anyhow::bail!("canonical_task_runtime_v6_migration_foreign_key_violation");
        }
        Ok(())
    }

    fn migrate_v7_to_v8(conn: &mut Connection) -> Result<()> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO canonical_task_runtime_metadata(key, value)
             VALUES ('store_identity', 'canonical_task_runtime_store:' || lower(hex(randomblob(16))))
             ON CONFLICT(key) DO NOTHING",
            [],
        )?;
        let changed = tx.execute(
            "UPDATE canonical_task_runtime_metadata SET value = '8'
             WHERE key = 'schema_version' AND value = '7'",
            [],
        )?;
        if changed != 1 {
            anyhow::bail!("canonical_task_runtime_v7_migration_version_conflict");
        }
        tx.commit()?;
        Ok(())
    }

    fn migrate_v8_to_v9(conn: &mut Connection) -> Result<()> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS canonical_artifact_undo (
                artifact_id TEXT PRIMARY KEY,
                proposal_id TEXT NOT NULL UNIQUE,
                source_reference TEXT NOT NULL,
                target_reference TEXT NOT NULL,
                content_digest TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN (
                    'waiting_review', 'undone', 'failed', 'effect_unknown'
                )),
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                FOREIGN KEY(artifact_id) REFERENCES canonical_artifacts(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             UPDATE canonical_task_runtime_metadata SET value = '9'
              WHERE key = 'schema_version' AND value = '8';",
        )?;
        tx.commit()?;
        Ok(())
    }

    fn migrate_v9_to_v10(conn: &mut Connection) -> Result<()> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS canonical_task_deferred_results (
                task_id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL UNIQUE,
                conversation_item_id TEXT NOT NULL,
                result_digest TEXT NOT NULL,
                summary_code TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             UPDATE canonical_task_runtime_metadata SET value = '10'
              WHERE key = 'schema_version' AND value = '9';",
        )?;
        tx.commit()?;
        Ok(())
    }

    fn migrate_v10_to_v11(conn: &mut Connection) -> Result<()> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old_table_exists = tx
            .query_row(
                "SELECT 1 FROM sqlite_master
                 WHERE type='table' AND name='canonical_report_steering'",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if old_table_exists {
            // initialize() creates the current empty table before applying old
            // migrations. Copy v10 rows into that owner instead of attempting
            // a rename that would conflict with it.
            tx.execute_batch(
                "INSERT INTO canonical_steering (
                    steering_id,item_id,task_id,run_id,source_message_ref,
                    source_message_digest,steering_digest,base_plan_revision,
                    status,created_at,consumed_at
                 ) SELECT steering_id,item_id,task_id,run_id,source_message_ref,
                          source_message_digest,steering_digest,base_plan_revision,
                          status,created_at,consumed_at
                   FROM canonical_report_steering;
                 DROP TABLE canonical_report_steering;",
            )?;
        }
        tx.execute_batch(
            "UPDATE canonical_task_items
                SET summary_code = CASE summary_code
                    WHEN 'report_steering_scope_expansion_blocked'
                        THEN 'work_steering_scope_expansion_blocked'
                    WHEN 'report_steering_pending' THEN 'work_steering_pending'
                    WHEN 'report_steering_consumed' THEN 'work_steering_consumed'
                    ELSE summary_code END
              WHERE kind = 'steering';
             UPDATE canonical_task_runtime_metadata SET value = '11'
              WHERE key = 'schema_version' AND value = '10';",
        )?;
        tx.commit()?;
        Ok(())
    }

    fn migrate_v11_to_v12(conn: &mut Connection) -> Result<()> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "ALTER TABLE canonical_task_runs ADD COLUMN project_id TEXT;
             ALTER TABLE canonical_task_runs ADD COLUMN project_revision INTEGER
                CHECK(project_revision > 0);
             ALTER TABLE canonical_task_runs ADD COLUMN scope_digest TEXT;
             CREATE TABLE IF NOT EXISTS canonical_task_attention (
                attention_id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN (
                    'review_required','blocked','failed','effect_unknown','scope_stale'
                )),
                reason_code TEXT NOT NULL,
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                UNIQUE(task_id, run_id, kind, reason_code),
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             );
             UPDATE canonical_task_runtime_metadata SET value = '12'
              WHERE key = 'schema_version' AND value = '11';",
        )?;
        tx.commit()?;
        Ok(())
    }

    fn migrate_v12_to_v13(conn: &mut Connection) -> Result<()> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS canonical_work_plans (
                run_id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                plan_revision INTEGER NOT NULL CHECK(plan_revision > 0),
                schema_version TEXT NOT NULL,
                plan_json TEXT NOT NULL,
                plan_digest TEXT NOT NULL,
                max_plan_attempts INTEGER NOT NULL CHECK(max_plan_attempts > 0),
                max_provider_attempts INTEGER NOT NULL CHECK(max_provider_attempts > 0),
                max_tool_attempts INTEGER NOT NULL CHECK(max_tool_attempts > 0),
                max_total_items INTEGER NOT NULL CHECK(max_total_items > 0),
                created_at TEXT NOT NULL,
                UNIQUE(task_id, plan_revision),
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             UPDATE canonical_task_runtime_metadata SET value = '13'
              WHERE key = 'schema_version' AND value = '12';",
        )?;
        tx.commit()?;
        Ok(())
    }

    fn migrate_v13_to_v14(conn: &mut Connection) -> Result<()> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS canonical_work_plan_revisions (
                run_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                plan_revision INTEGER NOT NULL CHECK(plan_revision > 0),
                schema_version TEXT NOT NULL,
                plan_json TEXT NOT NULL,
                plan_digest TEXT NOT NULL,
                max_plan_attempts INTEGER NOT NULL CHECK(max_plan_attempts > 0),
                max_provider_attempts INTEGER NOT NULL CHECK(max_provider_attempts > 0),
                max_tool_attempts INTEGER NOT NULL CHECK(max_tool_attempts > 0),
                max_total_items INTEGER NOT NULL CHECK(max_total_items > 0),
                created_at TEXT NOT NULL,
                PRIMARY KEY(run_id, plan_revision),
                UNIQUE(task_id, run_id, plan_revision),
                FOREIGN KEY(task_id, run_id)
                    REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             INSERT OR IGNORE INTO canonical_work_plan_revisions (
                run_id, task_id, plan_revision, schema_version, plan_json,
                plan_digest, max_plan_attempts, max_provider_attempts,
                max_tool_attempts, max_total_items, created_at
             ) SELECT run_id, task_id, plan_revision, schema_version, plan_json,
                      plan_digest, max_plan_attempts, max_provider_attempts,
                      max_tool_attempts, max_total_items, created_at
                 FROM canonical_work_plans;
             UPDATE canonical_task_runtime_metadata SET value = '14'
              WHERE key = 'schema_version' AND value = '13';",
        )?;
        tx.commit()?;
        Ok(())
    }

    fn migrate_v14_to_v15(conn: &mut Connection) -> Result<()> {
        let existing_columns = {
            let mut statement = conn.prepare("PRAGMA table_info(canonical_artifact_versions)")?;
            let columns = statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<std::result::Result<std::collections::HashSet<_>, _>>()?;
            columns
        };
        let undo_columns = {
            let mut statement = conn.prepare("PRAGMA table_info(canonical_artifact_undo)")?;
            let columns = statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<std::result::Result<std::collections::HashSet<_>, _>>()?;
            columns
        };
        let artifact_columns = {
            let mut statement = conn.prepare("PRAGMA table_info(canonical_artifacts)")?;
            let columns = statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<std::result::Result<std::collections::HashSet<_>, _>>()?;
            columns
        };
        let foreign_keys_enabled: bool =
            conn.pragma_query_value(None, "foreign_keys", |row| Ok(row.get::<_, i64>(0)? != 0))?;
        if foreign_keys_enabled {
            conn.pragma_update(None, "foreign_keys", "OFF")?;
        }
        let migration = (|| -> Result<()> {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            for (column, definition) in [
                ("target_reference", "TEXT"),
                ("draft_reference", "TEXT"),
                (
                    "expected_target_absent",
                    "INTEGER CHECK(expected_target_absent IN (0, 1))",
                ),
                ("expected_target_digest", "TEXT"),
            ] {
                if !existing_columns.contains(column) {
                    tx.execute(
                        &format!(
                            "ALTER TABLE canonical_artifact_versions ADD COLUMN {column} {definition}"
                        ),
                        [],
                    )?;
                }
            }
            if artifact_columns.contains("proposal_id") {
                tx.execute_batch(
                    "CREATE TABLE canonical_artifacts_v15 (
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
                        materialized_reference TEXT,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL,
                        FOREIGN KEY(task_id) REFERENCES canonical_tasks(id) ON DELETE RESTRICT,
                        FOREIGN KEY(source_item_id)
                            REFERENCES canonical_task_items(id) ON DELETE RESTRICT
                     );
                     INSERT INTO canonical_artifacts_v15 (
                        id, task_id, source_item_id, current_version, status,
                        media_type, target_reference_digest, content_digest,
                        materialized_reference, created_at, updated_at
                     ) SELECT id, task_id, source_item_id, current_version, status,
                              media_type, target_reference_digest, content_digest,
                              materialized_reference, created_at, updated_at
                         FROM canonical_artifacts;
                     DROP TABLE canonical_artifacts;
                     ALTER TABLE canonical_artifacts_v15 RENAME TO canonical_artifacts;",
                )?;
            }
            if !undo_columns.contains("version") {
                tx.execute_batch(
                    "ALTER TABLE canonical_artifact_undo RENAME TO canonical_artifact_undo_v14;
                 CREATE TABLE canonical_artifact_undo (
                    artifact_id TEXT NOT NULL,
                    version INTEGER NOT NULL CHECK(version > 0),
                    proposal_id TEXT NOT NULL UNIQUE,
                    source_reference TEXT NOT NULL,
                    target_reference TEXT NOT NULL,
                    content_digest TEXT NOT NULL,
                    status TEXT NOT NULL CHECK(status IN (
                        'waiting_review', 'undone', 'failed', 'effect_unknown'
                    )),
                    created_at TEXT NOT NULL,
                    resolved_at TEXT,
                    PRIMARY KEY(artifact_id, version),
                    FOREIGN KEY(artifact_id, version)
                        REFERENCES canonical_artifact_versions(artifact_id, version)
                        ON DELETE RESTRICT
                 ) WITHOUT ROWID;
                 INSERT INTO canonical_artifact_undo (
                    artifact_id, version, proposal_id, source_reference,
                    target_reference, content_digest, status, created_at, resolved_at
                 ) SELECT undo.artifact_id, artifact.current_version, undo.proposal_id,
                          undo.source_reference, undo.target_reference, undo.content_digest,
                          undo.status, undo.created_at, undo.resolved_at
                     FROM canonical_artifact_undo_v14 undo
                     JOIN canonical_artifacts artifact ON artifact.id = undo.artifact_id;
                 DROP TABLE canonical_artifact_undo_v14;",
                )?;
            }
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS canonical_artifact_review_checkpoints (
                artifact_id TEXT NOT NULL,
                version INTEGER NOT NULL CHECK(version > 0),
                proposal_id TEXT NOT NULL UNIQUE,
                item_id TEXT NOT NULL UNIQUE,
                status TEXT NOT NULL CHECK(status IN (
                    'waiting', 'accepted', 'rejected', 'failed', 'effect_unknown'
                )),
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                PRIMARY KEY(artifact_id, version),
                FOREIGN KEY(artifact_id, version)
                    REFERENCES canonical_artifact_versions(artifact_id, version)
                    ON DELETE RESTRICT,
                FOREIGN KEY(item_id) REFERENCES canonical_task_items(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS canonical_artifact_effects (
                proposal_id TEXT PRIMARY KEY,
                artifact_id TEXT NOT NULL,
                version INTEGER NOT NULL CHECK(version > 0),
                attempt_id TEXT NOT NULL UNIQUE,
                dispatch_claim_id TEXT NOT NULL,
                target_reference_digest TEXT NOT NULL,
                content_digest TEXT NOT NULL,
                byte_size INTEGER NOT NULL CHECK(byte_size >= 0),
                media_type TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN (
                    'prepared', 'staged', 'confirmed',
                    'failed_before_effect', 'effect_unknown'
                )),
                observed_content_digest TEXT,
                error_code TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(artifact_id, version)
                    REFERENCES canonical_artifact_versions(artifact_id, version)
                    ON DELETE RESTRICT,
                FOREIGN KEY(attempt_id)
                    REFERENCES canonical_task_item_attempts(attempt_id) ON DELETE RESTRICT
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_canonical_artifacts_task
                ON canonical_artifacts(task_id, created_at, id);
             UPDATE canonical_task_runtime_metadata SET value = '15'
              WHERE key = 'schema_version' AND value = '14';",
            )?;
            tx.commit()?;
            Ok(())
        })();
        if foreign_keys_enabled {
            conn.pragma_update(None, "foreign_keys", "ON")?;
        }
        migration?;
        let foreign_key_violation: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check LIMIT 1)",
            [],
            |row| row.get(0),
        )?;
        if foreign_key_violation {
            anyhow::bail!("canonical_task_runtime_v15_foreign_key_violation");
        }
        Ok(())
    }

    fn migrate_v15_to_v16(conn: &mut Connection) -> Result<()> {
        // Plan revision is scoped to one Run. The old task-wide UNIQUE
        // constraint made a second Run of the same Task collide at revision
        // one, so a user-visible Retry could never persist its own plan.
        let foreign_keys_enabled: bool =
            conn.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
        if foreign_keys_enabled {
            conn.pragma_update(None, "foreign_keys", "OFF")?;
        }
        let migration = (|| -> Result<()> {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(
                "CREATE TABLE canonical_work_plans_v16 (
                    run_id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    plan_revision INTEGER NOT NULL CHECK(plan_revision > 0),
                    schema_version TEXT NOT NULL,
                    plan_json TEXT NOT NULL,
                    plan_digest TEXT NOT NULL,
                    max_plan_attempts INTEGER NOT NULL CHECK(max_plan_attempts > 0),
                    max_provider_attempts INTEGER NOT NULL CHECK(max_provider_attempts > 0),
                    max_tool_attempts INTEGER NOT NULL CHECK(max_tool_attempts > 0),
                    max_total_items INTEGER NOT NULL CHECK(max_total_items > 0),
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(task_id, run_id)
                        REFERENCES canonical_task_runs(task_id, run_id) ON DELETE RESTRICT
                 ) WITHOUT ROWID;
                 INSERT INTO canonical_work_plans_v16 (
                    run_id, task_id, plan_revision, schema_version, plan_json,
                    plan_digest, max_plan_attempts, max_provider_attempts,
                    max_tool_attempts, max_total_items, created_at
                 ) SELECT run_id, task_id, plan_revision, schema_version, plan_json,
                          plan_digest, max_plan_attempts, max_provider_attempts,
                          max_tool_attempts, max_total_items, created_at
                     FROM canonical_work_plans;
                 DROP TABLE canonical_work_plans;
                 ALTER TABLE canonical_work_plans_v16 RENAME TO canonical_work_plans;
                 UPDATE canonical_task_runtime_metadata SET value = '16'
                  WHERE key = 'schema_version' AND value = '15';",
            )?;
            tx.commit()?;
            Ok(())
        })();
        if foreign_keys_enabled {
            conn.pragma_update(None, "foreign_keys", "ON")?;
        }
        migration?;
        let foreign_key_violation: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check LIMIT 1)",
            [],
            |row| row.get(0),
        )?;
        if foreign_key_violation {
            anyhow::bail!("canonical_task_runtime_v16_foreign_key_violation");
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

    pub fn begin_general_task_run(
        &self,
        input: BeginGeneralTaskRunInput<'_>,
    ) -> Result<BegunGeneralTaskRun> {
        validate_uuid("task_id", input.task_id)?;
        validate_uuid("conversation_id", input.conversation_id)?;
        validate_uuid("run_id", input.run_id)?;
        validate_nonempty("execution_session_id", input.execution_session_id, 512)?;
        validate_digest("instruction_digest", input.instruction_digest)?;
        if let Some(plan_digest) = input.plan_digest {
            validate_digest("plan_digest", plan_digest)?;
        }
        match (input.project_id, input.project_revision, input.scope_digest) {
            (None, None, None) => {}
            (Some(project_id), Some(project_revision), Some(scope_digest)) => {
                validate_uuid("project_id", project_id)?;
                if project_revision == 0 {
                    anyhow::bail!("canonical_project_revision_invalid");
                }
                validate_digest("scope_digest", scope_digest)?;
            }
            _ => anyhow::bail!("canonical_project_scope_incomplete"),
        }
        let instruction_item_id = stable_id("item", &["instruction", input.task_id, input.run_id]);
        let plan_item_id = input
            .plan_digest
            .map(|_| stable_id("item", &["plan", input.task_id, input.run_id]));
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted_task = tx.execute(
            "INSERT INTO canonical_tasks (
                id, conversation_id, task_kind, initial_outcome_digest,
                status, created_at, updated_at
             ) VALUES (?1, ?2, 'work', ?3, 'running', ?4, ?4)
             ON CONFLICT(id) DO NOTHING",
            params![
                input.task_id,
                input.conversation_id,
                input.instruction_digest,
                now
            ],
        )?;
        let task: (String, String, String, String) = tx.query_row(
            "SELECT conversation_id, task_kind, initial_outcome_digest, status
             FROM canonical_tasks WHERE id = ?1",
            [input.task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        if task.0 != input.conversation_id || task.1 != "work" || task.2 != input.instruction_digest
        {
            anyhow::bail!("canonical_general_task_identity_conflict");
        }
        let existing_run = tx
            .query_row(
                "SELECT task_id, execution_session_id, ordinal
                 FROM canonical_task_runs WHERE run_id = ?1",
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
        let run_existed = existing_run.is_some();
        if inserted_task == 0
            && existing_run.is_none()
            && !matches!(
                task.3.as_str(),
                "completed" | "failed" | "blocked" | "cancelled" | "interrupted"
            )
        {
            anyhow::bail!("canonical_general_task_not_retryable");
        }
        let ordinal: i64 = tx.query_row(
            "SELECT COALESCE(MAX(ordinal), 0) + 1 FROM canonical_task_runs WHERE task_id = ?1",
            [input.task_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO canonical_task_runs (
                task_id, run_id, execution_session_id, ordinal, status,
                execution_facts_version, plan_revision, created_at, updated_at,
                project_id, project_revision, scope_digest
             ) VALUES (?1, ?2, ?3, ?4, 'running', 5, 1, ?5, ?5, ?6, ?7, ?8)
             ON CONFLICT(run_id) DO NOTHING",
            params![
                input.task_id,
                input.run_id,
                input.execution_session_id,
                ordinal,
                now,
                input.project_id,
                input.project_revision.map(i64::try_from).transpose()?,
                input.scope_digest,
            ],
        )?;
        let run: (
            String,
            String,
            i64,
            i64,
            Option<String>,
            Option<i64>,
            Option<String>,
        ) = tx.query_row(
            "SELECT task_id, execution_session_id, ordinal, plan_revision,
                    project_id, project_revision, scope_digest
             FROM canonical_task_runs WHERE run_id = ?1",
            [input.run_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )?;
        if run.0 != input.task_id || run.1 != input.execution_session_id {
            anyhow::bail!("canonical_general_run_identity_conflict");
        }
        if run.4.as_deref() != input.project_id
            || run.5.map(u64::try_from).transpose()? != input.project_revision
            || run.6.as_deref() != input.scope_digest
        {
            anyhow::bail!("canonical_general_run_scope_conflict");
        }
        if run_existed {
            let stored_plan_digest = tx
                .query_row(
                    "SELECT payload_digest FROM canonical_task_items
                     WHERE task_id = ?1 AND run_id = ?2 AND kind = 'plan'",
                    params![input.task_id, input.run_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if stored_plan_digest.as_deref() != input.plan_digest {
                anyhow::bail!("canonical_general_run_plan_conflict");
            }
        }
        ensure_completed_item(
            &tx,
            CompletedItemInput {
                item_id: &instruction_item_id,
                task_id: input.task_id,
                run_id: input.run_id,
                kind: CanonicalTaskItemKind::Instruction,
                summary_code: "work_instruction_bound",
                payload_digest: input.instruction_digest,
                now: &now,
            },
        )?;
        if let (Some(item_id), Some(plan_digest)) = (&plan_item_id, input.plan_digest) {
            ensure_completed_item(
                &tx,
                CompletedItemInput {
                    item_id,
                    task_id: input.task_id,
                    run_id: input.run_id,
                    kind: CanonicalTaskItemKind::Plan,
                    summary_code: "work_plan_bound",
                    payload_digest: plan_digest,
                    now: &now,
                },
            )?;
        }
        if !run_existed {
            if task.3 == "completed" {
                tx.execute(
                    "DELETE FROM canonical_task_final_results WHERE task_id = ?1",
                    [input.task_id],
                )?;
                tx.execute(
                    "DELETE FROM canonical_task_deferred_results WHERE task_id = ?1",
                    [input.task_id],
                )?;
            }
            tx.execute(
                "UPDATE canonical_tasks SET status = 'running', updated_at = ?2 WHERE id = ?1",
                params![input.task_id, now],
            )?;
        }
        tx.commit()?;
        Ok(BegunGeneralTaskRun {
            task_id: input.task_id.to_string(),
            run_id: input.run_id.to_string(),
            instruction_item_id,
            plan_item_id,
            ordinal: u64::try_from(run.2)?,
            plan_revision: u64::try_from(run.3)?,
        })
    }

    pub fn begin_item_attempt(
        &self,
        input: BeginItemAttemptInput<'_>,
    ) -> Result<CanonicalTaskItemAttemptRecord> {
        validate_uuid("attempt_id", input.attempt_id)?;
        validate_uuid("task_id", input.task_id)?;
        validate_uuid("run_id", input.run_id)?;
        validate_nonempty("item_id", input.item_id, 512)?;
        validate_nonempty("executor_kind", input.executor_kind, 64)?;
        if !matches!(
            input.executor_kind,
            "provider" | "tool" | "internal" | "review" | "materializer"
        ) {
            anyhow::bail!("canonical_task_runtime_executor_kind_invalid");
        }
        validate_digest("request_digest", input.request_digest)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (item_task, item_run, item_status): (String, String, String) = tx.query_row(
            "SELECT task_id, run_id, status FROM canonical_task_items WHERE id = ?1",
            [input.item_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if item_task != input.task_id || item_run != input.run_id {
            anyhow::bail!("canonical_item_attempt_owner_conflict");
        }
        if matches!(
            item_status.as_str(),
            "completed" | "cancelled" | "effect_unknown"
        ) {
            anyhow::bail!("canonical_item_attempt_terminal_item");
        }
        let ordinal: i64 = tx.query_row(
            "SELECT COALESCE(MAX(ordinal), 0) + 1
             FROM canonical_task_item_attempts WHERE item_id = ?1",
            [input.item_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO canonical_task_item_attempts (
                attempt_id, task_id, run_id, item_id, ordinal, status,
                executor_kind, provider_profile_id, provider_model_id,
                request_digest, started_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(attempt_id) DO NOTHING",
            params![
                input.attempt_id,
                input.task_id,
                input.run_id,
                input.item_id,
                ordinal,
                input.executor_kind,
                input.provider_profile_id,
                input.provider_model_id,
                input.request_digest,
                now
            ],
        )?;
        tx.execute(
            "UPDATE canonical_task_items SET status = 'running', updated_at = ?2 WHERE id = ?1",
            params![input.item_id, now],
        )?;
        let attempt = load_attempt_in_tx(&tx, input.attempt_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_item_attempt_missing_after_begin"))?;
        if attempt.task_id != input.task_id
            || attempt.run_id != input.run_id
            || attempt.item_id != input.item_id
            || attempt.executor_kind != input.executor_kind
            || attempt.request_digest != input.request_digest
        {
            anyhow::bail!("canonical_item_attempt_identity_conflict");
        }
        tx.commit()?;
        Ok(attempt)
    }

    pub fn append_general_item(
        &self,
        task_id: &str,
        run_id: &str,
        item_id: &str,
        kind: CanonicalTaskItemKind,
        summary_code: &str,
        payload_digest: &str,
    ) -> Result<CanonicalTaskItemRecord> {
        validate_uuid("task_id", task_id)?;
        validate_uuid("run_id", run_id)?;
        validate_nonempty("item_id", item_id, 512)?;
        validate_nonempty("summary_code", summary_code, 128)?;
        validate_digest("payload_digest", payload_digest)?;
        if matches!(
            kind,
            CanonicalTaskItemKind::Instruction | CanonicalTaskItemKind::FinalResult
        ) {
            anyhow::bail!("canonical_general_item_kind_reserved");
        }
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run_status: String = tx.query_row(
            "SELECT status FROM canonical_task_runs WHERE task_id = ?1 AND run_id = ?2",
            params![task_id, run_id],
            |row| row.get(0),
        )?;
        if run_status != "running" {
            anyhow::bail!("canonical_general_item_run_not_running");
        }
        if let Some(existing) = tx
            .query_row(
                "SELECT id, task_id, run_id, sequence, kind, status, summary_code,
                        payload_digest, created_at, updated_at
                 FROM canonical_task_items WHERE id = ?1",
                [item_id],
                row_to_item,
            )
            .optional()?
        {
            if existing.task_id != task_id
                || existing.run_id != run_id
                || existing.kind != kind
                || existing.summary_code != summary_code
                || existing.payload_digest != payload_digest
            {
                anyhow::bail!("canonical_general_item_identity_conflict");
            }
            tx.commit()?;
            return Ok(existing);
        }
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM canonical_task_items WHERE task_id = ?1",
            [task_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO canonical_task_items (
                id, task_id, run_id, sequence, kind, status, summary_code,
                payload_digest, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'waiting', ?6, ?7, ?8, ?8)",
            params![
                item_id,
                task_id,
                run_id,
                sequence,
                kind.as_str(),
                summary_code,
                payload_digest,
                now
            ],
        )?;
        let item = tx.query_row(
            "SELECT id, task_id, run_id, sequence, kind, status, summary_code,
                    payload_digest, created_at, updated_at
             FROM canonical_task_items WHERE id = ?1",
            [item_id],
            row_to_item,
        )?;
        tx.commit()?;
        Ok(item)
    }

    /// Derive durable Work budget usage from canonical Items and Attempts.
    /// Limits are policy, while usage is reconstructed from the lifecycle
    /// owner so restart never resets an exhausted budget.
    pub fn work_run_budget_usage(&self, run_id: &str) -> Result<WorkRunBudgetUsage> {
        validate_uuid("run_id", run_id)?;
        let conn = self.lock_conn()?;
        let run_exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM canonical_task_runs WHERE run_id = ?1)",
            [run_id],
            |row| row.get(0),
        )?;
        if !run_exists {
            anyhow::bail!("canonical_work_budget_run_missing");
        }
        let total_items: i64 = conn.query_row(
            "SELECT COUNT(*) FROM canonical_task_items WHERE run_id = ?1",
            [run_id],
            |row| row.get(0),
        )?;
        let provider_attempts: i64 = conn.query_row(
            "SELECT COUNT(*) FROM canonical_task_item_attempts
             WHERE run_id = ?1 AND executor_kind = 'provider'",
            [run_id],
            |row| row.get(0),
        )?;
        let tool_attempts: i64 = conn.query_row(
            "SELECT COUNT(*) FROM canonical_task_item_attempts
             WHERE run_id = ?1 AND executor_kind = 'tool'",
            [run_id],
            |row| row.get(0),
        )?;
        let plan_attempts: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM canonical_task_item_attempts attempt
             JOIN canonical_task_items item ON item.id = attempt.item_id
             WHERE attempt.run_id = ?1
               AND attempt.executor_kind = 'provider'
               AND item.summary_code = 'work_plan_generation'",
            [run_id],
            |row| row.get(0),
        )?;
        Ok(WorkRunBudgetUsage {
            plan_attempts: u32::try_from(plan_attempts)?,
            provider_attempts: u32::try_from(provider_attempts)?,
            tool_attempts: u32::try_from(tool_attempts)?,
            total_items: u32::try_from(total_items)?,
        })
    }

    /// Append an execution observation that is already mechanically complete.
    ///
    /// This is intentionally narrower than a caller-selected status setter:
    /// live provider and tool work must still pass through `ItemAttempt`.
    /// It is used for bounded, post-execution facts such as a verified tool
    /// observation or selected Skill context receipt.
    pub fn append_completed_observation(
        &self,
        task_id: &str,
        run_id: &str,
        item_id: &str,
        summary_code: &str,
        payload_digest: &str,
    ) -> Result<CanonicalTaskItemRecord> {
        let item = self.append_general_item(
            task_id,
            run_id,
            item_id,
            CanonicalTaskItemKind::Observation,
            summary_code,
            payload_digest,
        )?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE canonical_task_items SET status = 'completed', updated_at = ?2
             WHERE id = ?1 AND status = 'waiting'",
            params![item_id, now],
        )?;
        let completed = tx.query_row(
            "SELECT id, task_id, run_id, sequence, kind, status, summary_code,
                    payload_digest, created_at, updated_at
             FROM canonical_task_items WHERE id = ?1",
            [item_id],
            row_to_item,
        )?;
        if changed == 0 && completed.status != CanonicalTaskItemStatus::Completed {
            anyhow::bail!("canonical_completed_observation_terminal_conflict");
        }
        if completed.task_id != item.task_id
            || completed.run_id != item.run_id
            || completed.kind != CanonicalTaskItemKind::Observation
            || completed.summary_code != summary_code
            || completed.payload_digest != payload_digest
        {
            anyhow::bail!("canonical_completed_observation_identity_conflict");
        }
        tx.commit()?;
        Ok(completed)
    }

    /// Persist one policy-validated structured plan declaration. Planning is
    /// an internal scheduler fact, not a provider/tool effect attempt; the
    /// provider call that proposed the plan has its own ProviderGeneration
    /// ItemAttempt when a real adapter was used.
    pub fn append_completed_plan_item(
        &self,
        task_id: &str,
        run_id: &str,
        item_id: &str,
        summary_code: &str,
        payload_digest: &str,
    ) -> Result<CanonicalTaskItemRecord> {
        let item = self.append_general_item(
            task_id,
            run_id,
            item_id,
            CanonicalTaskItemKind::Plan,
            summary_code,
            payload_digest,
        )?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE canonical_task_items SET status = 'completed', updated_at = ?2
             WHERE id = ?1 AND status = 'waiting'",
            params![item_id, now],
        )?;
        let completed = tx.query_row(
            "SELECT id, task_id, run_id, sequence, kind, status, summary_code,
                    payload_digest, created_at, updated_at
             FROM canonical_task_items WHERE id = ?1",
            [item_id],
            row_to_item,
        )?;
        if changed == 0 && completed.status != CanonicalTaskItemStatus::Completed {
            anyhow::bail!("canonical_completed_plan_item_terminal_conflict");
        }
        if completed.task_id != item.task_id
            || completed.run_id != item.run_id
            || completed.kind != CanonicalTaskItemKind::Plan
            || completed.summary_code != summary_code
            || completed.payload_digest != payload_digest
        {
            anyhow::bail!("canonical_completed_plan_item_identity_conflict");
        }
        tx.commit()?;
        Ok(completed)
    }

    /// Persist the complete policy-validated plan for one Run. The JSON holds
    /// only typed step ids, kinds, dependencies, and completion requirements;
    /// user text and tool payloads remain in their canonical owners. This
    /// makes scheduling and completion evaluation reconstructable after a
    /// restart without creating a second task runtime.
    pub fn persist_work_plan(
        &self,
        task_id: &str,
        run_id: &str,
        plan_revision: u64,
        plan: &StructuredWorkPlan,
        budget_policy: WorkRunBudgetPolicy,
    ) -> Result<CanonicalWorkPlanRecord> {
        validate_uuid("task_id", task_id)?;
        validate_uuid("run_id", run_id)?;
        if plan_revision == 0 {
            anyhow::bail!("canonical_work_plan_revision_invalid");
        }
        let allowed = plan.steps.iter().map(|step| step.kind).collect();
        let allowed_mcp_targets = plan
            .steps
            .iter()
            .filter_map(|step| step.target_id.clone())
            .collect();
        plan.validate(&allowed, &allowed_mcp_targets)
            .map_err(|code| anyhow::anyhow!(code))?;
        let plan_json = plan
            .canonical_json()
            .map_err(|code| anyhow::anyhow!(code))?;
        validate_nonempty("work_plan_json", &plan_json, 32_768)?;
        let plan_digest = sha256_text(&plan_json);
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (status, stored_revision): (String, i64) = tx.query_row(
            "SELECT status, plan_revision FROM canonical_task_runs
             WHERE task_id = ?1 AND run_id = ?2",
            params![task_id, run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if status != "running" {
            anyhow::bail!("canonical_work_plan_run_not_running");
        }
        if u64::try_from(stored_revision)? != plan_revision {
            anyhow::bail!("canonical_work_plan_revision_stale");
        }
        if let Some(existing) = load_work_plan_in_tx(&tx, run_id)? {
            if existing.task_id != task_id
                || existing.plan_revision != plan_revision
                || existing.plan_digest != plan_digest
                || existing.plan != *plan
                || existing.budget_policy != budget_policy
            {
                anyhow::bail!("canonical_work_plan_identity_conflict");
            }
            tx.commit()?;
            return Ok(existing);
        }
        tx.execute(
            "INSERT INTO canonical_work_plans (
                run_id, task_id, plan_revision, schema_version, plan_json,
                plan_digest, max_plan_attempts, max_provider_attempts,
                max_tool_attempts, max_total_items, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                run_id,
                task_id,
                i64::try_from(plan_revision)?,
                plan.schema_version,
                plan_json,
                plan_digest,
                i64::from(budget_policy.max_plan_attempts),
                i64::from(budget_policy.max_provider_attempts),
                i64::from(budget_policy.max_tool_attempts),
                i64::from(budget_policy.max_total_items),
                now
            ],
        )?;
        insert_work_plan_revision_in_tx(
            &tx,
            task_id,
            run_id,
            plan_revision,
            plan,
            &plan_json,
            &plan_digest,
            budget_policy,
            &now,
        )?;
        let persisted = load_work_plan_in_tx(&tx, run_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_work_plan_missing_after_insert"))?;
        tx.commit()?;
        Ok(persisted)
    }

    /// Atomically replace the current execution plan while retaining every
    /// admitted revision. The Run budget policy is immutable and the caller
    /// must present the exact current revision, so replanning cannot reset
    /// attempts or race a steering/terminal transition.
    pub fn revise_work_plan(
        &self,
        task_id: &str,
        run_id: &str,
        base_plan_revision: u64,
        plan: &StructuredWorkPlan,
    ) -> Result<CanonicalWorkPlanRecord> {
        validate_uuid("task_id", task_id)?;
        validate_uuid("run_id", run_id)?;
        if base_plan_revision == 0 {
            anyhow::bail!("canonical_work_replan_base_revision_invalid");
        }
        let allowed = plan.steps.iter().map(|step| step.kind).collect();
        let allowed_mcp_targets = plan
            .steps
            .iter()
            .filter_map(|step| step.target_id.clone())
            .collect();
        plan.validate(&allowed, &allowed_mcp_targets)
            .map_err(|code| anyhow::anyhow!(code))?;
        let plan_json = plan
            .canonical_json()
            .map_err(|code| anyhow::anyhow!(code))?;
        validate_nonempty("work_plan_json", &plan_json, 32_768)?;
        let plan_digest = sha256_text(&plan_json);
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (status, stored_revision): (String, i64) = tx.query_row(
            "SELECT status, plan_revision FROM canonical_task_runs
             WHERE task_id = ?1 AND run_id = ?2",
            params![task_id, run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if status != "running" {
            anyhow::bail!("canonical_work_replan_run_not_running");
        }
        if u64::try_from(stored_revision)? != base_plan_revision {
            anyhow::bail!("canonical_work_replan_revision_stale");
        }
        let current = load_work_plan_in_tx(&tx, run_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_work_replan_current_plan_missing"))?;
        if current.task_id != task_id || current.plan_revision != base_plan_revision {
            anyhow::bail!("canonical_work_replan_current_identity_conflict");
        }
        let next_revision = base_plan_revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("canonical_work_replan_revision_overflow"))?;
        let changed = tx.execute(
            "UPDATE canonical_task_runs SET plan_revision = ?3, updated_at = ?4
             WHERE task_id = ?1 AND run_id = ?2 AND status = 'running'
               AND plan_revision = ?5",
            params![
                task_id,
                run_id,
                i64::try_from(next_revision)?,
                now,
                i64::try_from(base_plan_revision)?
            ],
        )?;
        if changed != 1 {
            anyhow::bail!("canonical_work_replan_run_cas_conflict");
        }
        let plan_changed = tx.execute(
            "UPDATE canonical_work_plans
             SET plan_revision = ?3, schema_version = ?4, plan_json = ?5,
                 plan_digest = ?6, created_at = ?7
             WHERE task_id = ?1 AND run_id = ?2 AND plan_revision = ?8",
            params![
                task_id,
                run_id,
                i64::try_from(next_revision)?,
                plan.schema_version,
                plan_json,
                plan_digest,
                now,
                i64::try_from(base_plan_revision)?
            ],
        )?;
        if plan_changed != 1 {
            anyhow::bail!("canonical_work_replan_plan_cas_conflict");
        }
        insert_work_plan_revision_in_tx(
            &tx,
            task_id,
            run_id,
            next_revision,
            plan,
            &plan_json,
            &plan_digest,
            current.budget_policy,
            &now,
        )?;
        let revised = load_work_plan_in_tx(&tx, run_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_work_replan_missing_after_update"))?;
        tx.commit()?;
        Ok(revised)
    }

    pub fn list_work_plan_revisions(&self, run_id: &str) -> Result<Vec<CanonicalWorkPlanRecord>> {
        validate_uuid("run_id", run_id)?;
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let mut statement = tx.prepare(
            "SELECT task_id, run_id, plan_revision, schema_version, plan_json,
                    plan_digest, max_plan_attempts, max_provider_attempts,
                    max_tool_attempts, max_total_items, created_at
             FROM canonical_work_plan_revisions
             WHERE run_id = ?1 ORDER BY plan_revision ASC",
        )?;
        let records = statement
            .query_map([run_id], row_to_work_plan_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        tx.commit()?;
        Ok(records)
    }

    pub fn load_work_plan(&self, run_id: &str) -> Result<Option<CanonicalWorkPlanRecord>> {
        validate_uuid("run_id", run_id)?;
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let plan = load_work_plan_in_tx(&tx, run_id)?;
        tx.commit()?;
        Ok(plan)
    }

    /// Return the immutable policy stored with the Run plan. Planning itself
    /// starts before that record exists and therefore uses the same default;
    /// every later provider/tool admission reads the persisted snapshot.
    pub fn work_run_budget_policy(&self, run_id: &str) -> Result<WorkRunBudgetPolicy> {
        validate_uuid("run_id", run_id)?;
        Ok(self
            .load_work_plan(run_id)?
            .map(|record| record.budget_policy)
            .unwrap_or_default())
    }

    pub fn terminalize_item_attempt(
        &self,
        attempt_id: &str,
        status: CanonicalTaskItemStatus,
        receipt_digest: Option<&str>,
    ) -> Result<CanonicalTaskItemAttemptRecord> {
        validate_uuid("attempt_id", attempt_id)?;
        if !matches!(
            status,
            CanonicalTaskItemStatus::Completed
                | CanonicalTaskItemStatus::Blocked
                | CanonicalTaskItemStatus::Failed
                | CanonicalTaskItemStatus::Cancelled
                | CanonicalTaskItemStatus::Interrupted
                | CanonicalTaskItemStatus::EffectUnknown
        ) {
            anyhow::bail!("canonical_item_attempt_terminal_status_invalid");
        }
        if let Some(digest) = receipt_digest {
            validate_digest("receipt_digest", digest)?;
        }
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let attempt = load_attempt_in_tx(&tx, attempt_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_item_attempt_missing"))?;
        if attempt.status != CanonicalTaskItemStatus::Running {
            if attempt.status == status && attempt.receipt_digest.as_deref() == receipt_digest {
                tx.commit()?;
                return Ok(attempt);
            }
            anyhow::bail!("canonical_item_attempt_terminal_conflict");
        }
        tx.execute(
            "UPDATE canonical_task_item_attempts
             SET status = ?2, receipt_digest = ?3, finished_at = ?4
             WHERE attempt_id = ?1 AND status = 'running'",
            params![attempt_id, status.as_str(), receipt_digest, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_items SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![attempt.item_id, status.as_str(), now],
        )?;
        let result = load_attempt_in_tx(&tx, attempt_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_item_attempt_missing_after_terminal"))?;
        tx.commit()?;
        Ok(result)
    }

    pub fn complete_general_task(
        &self,
        input: CompleteGeneralTaskInput<'_>,
    ) -> Result<CanonicalFinalResultRecord> {
        validate_uuid("task_id", input.task_id)?;
        validate_uuid("run_id", input.run_id)?;
        validate_nonempty("final_item_id", input.final_item_id, 512)?;
        validate_nonempty("conversation_item_id", input.conversation_item_id, 512)?;
        validate_digest("result_digest", input.result_digest)?;
        validate_nonempty("summary_code", input.summary_code, 128)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run_status: String = tx.query_row(
            "SELECT status FROM canonical_task_runs WHERE task_id = ?1 AND run_id = ?2",
            params![input.task_id, input.run_id],
            |row| row.get(0),
        )?;
        if run_status != "running" {
            if let Some(existing) = load_final_result_in_tx(&tx, input.task_id)? {
                if existing.run_id == input.run_id
                    && existing.item_id == input.final_item_id
                    && existing.conversation_item_id == input.conversation_item_id
                    && existing.result_digest == input.result_digest
                {
                    tx.commit()?;
                    return Ok(existing);
                }
            }
            anyhow::bail!("canonical_general_task_run_not_running");
        }
        let incomplete_required_items: i64 = tx.query_row(
            "SELECT COUNT(*) FROM canonical_task_items
             WHERE task_id = ?1 AND run_id = ?2
               AND kind != 'final_result' AND status != 'completed'",
            params![input.task_id, input.run_id],
            |row| row.get(0),
        )?;
        if incomplete_required_items != 0 {
            anyhow::bail!("canonical_completion_required_item_incomplete");
        }
        ensure_completed_item(
            &tx,
            CompletedItemInput {
                item_id: input.final_item_id,
                task_id: input.task_id,
                run_id: input.run_id,
                kind: CanonicalTaskItemKind::FinalResult,
                summary_code: input.summary_code,
                payload_digest: input.result_digest,
                now: &now,
            },
        )?;
        tx.execute(
            "INSERT INTO canonical_task_final_results (
                task_id, run_id, item_id, conversation_item_id, result_digest,
                summary_code, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                input.task_id,
                input.run_id,
                input.final_item_id,
                input.conversation_item_id,
                input.result_digest,
                input.summary_code,
                now
            ],
        )?;
        tx.execute(
            "UPDATE canonical_task_runs SET status = 'completed', updated_at = ?3,
                    completed_at = ?3 WHERE task_id = ?1 AND run_id = ?2",
            params![input.task_id, input.run_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = 'completed', updated_at = ?2 WHERE id = ?1",
            params![input.task_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_attention SET resolved_at = ?3
             WHERE task_id = ?1 AND run_id = ?2 AND resolved_at IS NULL",
            params![input.task_id, input.run_id, now],
        )?;
        let result = load_final_result_in_tx(&tx, input.task_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_final_result_missing_after_commit"))?;
        tx.commit()?;
        Ok(result)
    }

    /// Persist the exact assistant result identity while a Work Task is paused
    /// for Artifact review. Approval can happen after restart, so this durable
    /// relation is the only source allowed to create the later FinalResult.
    pub fn defer_general_task_result(&self, input: DeferGeneralTaskResultInput<'_>) -> Result<()> {
        validate_uuid("task_id", input.task_id)?;
        validate_uuid("run_id", input.run_id)?;
        validate_nonempty("conversation_item_id", input.conversation_item_id, 512)?;
        validate_digest("result_digest", input.result_digest)?;
        validate_nonempty("summary_code", input.summary_code, 128)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run_status: String = tx.query_row(
            "SELECT status FROM canonical_task_runs WHERE task_id = ?1 AND run_id = ?2",
            params![input.task_id, input.run_id],
            |row| row.get(0),
        )?;
        if run_status != CanonicalTaskStatus::WaitingReview.as_str() {
            anyhow::bail!("canonical_deferred_result_run_not_waiting_review");
        }
        tx.execute(
            "INSERT INTO canonical_task_deferred_results (
                task_id, run_id, conversation_item_id, result_digest,
                summary_code, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(task_id) DO NOTHING",
            params![
                input.task_id,
                input.run_id,
                input.conversation_item_id,
                input.result_digest,
                input.summary_code,
                now
            ],
        )?;
        let existing: (String, String, String, String) = tx.query_row(
            "SELECT run_id, conversation_item_id, result_digest, summary_code
             FROM canonical_task_deferred_results WHERE task_id = ?1",
            [input.task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        if existing
            != (
                input.run_id.to_string(),
                input.conversation_item_id.to_string(),
                input.result_digest.to_string(),
                input.summary_code.to_string(),
            )
        {
            anyhow::bail!("canonical_deferred_result_identity_conflict");
        }
        tx.commit()?;
        Ok(())
    }

    pub fn terminalize_general_run(
        &self,
        task_id: &str,
        run_id: &str,
        status: CanonicalTaskStatus,
    ) -> Result<CanonicalTaskSnapshot> {
        validate_uuid("task_id", task_id)?;
        validate_uuid("run_id", run_id)?;
        if !matches!(
            status,
            CanonicalTaskStatus::Blocked
                | CanonicalTaskStatus::Failed
                | CanonicalTaskStatus::Cancelled
                | CanonicalTaskStatus::Interrupted
                | CanonicalTaskStatus::EffectUnknown
        ) {
            anyhow::bail!("canonical_general_run_terminal_status_invalid");
        }
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE canonical_task_runs SET status = ?3, updated_at = ?4,
                    completed_at = ?4 WHERE task_id = ?1 AND run_id = ?2
                      AND status = 'running'",
            params![task_id, run_id, status.as_str(), now],
        )?;
        if changed == 0 {
            let existing: String = tx.query_row(
                "SELECT status FROM canonical_task_runs WHERE task_id = ?1 AND run_id = ?2",
                params![task_id, run_id],
                |row| row.get(0),
            )?;
            if existing != status.as_str() {
                anyhow::bail!("canonical_general_run_terminal_conflict");
            }
        }
        tx.execute(
            "UPDATE canonical_task_items SET status = ?3, updated_at = ?4
             WHERE task_id = ?1 AND run_id = ?2 AND status IN ('waiting', 'running')",
            params![task_id, run_id, status.as_str(), now],
        )?;
        tx.execute(
            "UPDATE canonical_task_item_attempts SET status = ?3, finished_at = ?4
             WHERE task_id = ?1 AND run_id = ?2 AND status = 'running'",
            params![task_id, run_id, status.as_str(), now],
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![task_id, status.as_str(), now],
        )?;
        tx.commit()?;
        drop(conn);
        self.load_task_snapshot(task_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_general_task_missing_after_terminal"))
    }

    pub fn recover_interrupted_general_runs(&self) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut statement = tx.prepare(
            "SELECT run.task_id, run.run_id FROM canonical_task_runs run
             JOIN canonical_tasks task ON task.id = run.task_id
             WHERE run.status = 'running' AND run.execution_facts_version = 5
               AND task.task_kind = 'work'",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let running = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        drop(statement);
        for (task_id, run_id) in &running {
            tx.execute(
                "UPDATE canonical_task_runs SET status = 'interrupted', updated_at = ?3,
                        completed_at = ?3 WHERE task_id = ?1 AND run_id = ?2",
                params![task_id, run_id, now],
            )?;
            tx.execute(
                "UPDATE canonical_task_items SET status = 'interrupted', updated_at = ?3
                 WHERE task_id = ?1 AND run_id = ?2 AND status IN ('waiting', 'running')",
                params![task_id, run_id, now],
            )?;
            tx.execute(
                "UPDATE canonical_task_item_attempts
                 SET status = 'interrupted', finished_at = ?3
                 WHERE task_id = ?1 AND run_id = ?2 AND status = 'running'",
                params![task_id, run_id, now],
            )?;
            tx.execute(
                "UPDATE canonical_tasks SET status = 'interrupted', updated_at = ?2
                 WHERE id = ?1 AND status = 'running'",
                params![task_id, now],
            )?;
        }
        tx.commit()?;
        Ok(u64::try_from(running.len())?)
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
             ON CONFLICT(id) DO NOTHING",
            params![task_id, input.conversation_id, input.outcome_digest, now],
        )?;
        let stored_task: (String, String, String) = tx.query_row(
            "SELECT id, initial_outcome_digest, status FROM canonical_tasks
             WHERE id = ?1",
            [&task_id],
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
                    task_id, run_id, execution_session_id, ordinal, status,
                    execution_facts_version, plan_revision, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, 'running', 5, 1, ?5, ?5)",
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
             ON CONFLICT(id) DO NOTHING",
            params![
                task_id,
                input.conversation_id,
                input.instruction_digest,
                now
            ],
        )?;
        let stored: (String, String, String) = tx.query_row(
            "SELECT id, task_kind, initial_outcome_digest FROM canonical_tasks
             WHERE id = ?1",
            [&task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if stored.0 != task_id || stored.1 != "plan" || stored.2 != input.instruction_digest {
            anyhow::bail!("canonical_plan_task_identity_conflict");
        }
        tx.execute(
            "INSERT INTO canonical_task_runs (
                task_id, run_id, execution_session_id, ordinal, status,
                execution_facts_version, plan_revision, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 1, 'running', 5, 1, ?4, ?4)
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

    pub fn submit_steering(
        &self,
        input: SubmitSteeringInput<'_>,
    ) -> Result<CanonicalSteeringRecord> {
        validate_nonempty("steering_id", input.steering_id, 512)?;
        validate_nonempty("task_id", input.task_id, 512)?;
        validate_nonempty("run_id", input.run_id, 512)?;
        validate_nonempty("source_message_ref", input.source_message_ref, 1024)?;
        validate_digest("source_message_digest", input.source_message_digest)?;
        validate_digest("steering_digest", input.steering_digest)?;
        if input.base_plan_revision == 0 {
            anyhow::bail!("canonical_steering_plan_revision_invalid");
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
                anyhow::bail!("canonical_steering_identity_conflict");
            }
            tx.commit()?;
            return Ok(existing);
        }
        let (task_status, plan_revision, run_finalized, provider_started): (
            String,
            i64,
            bool,
            bool,
        ) = tx
            .query_row(
                "SELECT task.status, run.plan_revision,
                        EXISTS(SELECT 1 FROM canonical_task_items terminal
                               WHERE terminal.task_id = task.id
                                 AND terminal.run_id = run.run_id
                                 AND terminal.kind = 'final_result'
                                 AND terminal.status = 'completed'),
                        EXISTS(SELECT 1 FROM canonical_task_items provider
                               WHERE provider.task_id = task.id
                                 AND provider.run_id = run.run_id
                                 AND provider.kind = 'provider_generation'
                                 AND provider.status IN ('running','completed'))
                 FROM canonical_tasks task
                 JOIN canonical_task_runs run ON run.task_id = task.id
                 WHERE task.id = ?1 AND run.run_id = ?2",
                params![input.task_id, input.run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .with_context(|| "canonical_steering_target_missing")?;
        if run_finalized || !matches!(task_status.as_str(), "running" | "waiting_review") {
            anyhow::bail!("canonical_steering_target_terminal");
        }
        if u64::try_from(plan_revision)? != input.base_plan_revision {
            anyhow::bail!("canonical_steering_plan_revision_stale");
        }
        if !input.scope_expansion_blocked && provider_started {
            anyhow::bail!("canonical_steering_checkpoint_passed");
        }
        if !input.scope_expansion_blocked {
            let pending: Option<String> = tx
                .query_row(
                    "SELECT steering_id FROM canonical_steering
                     WHERE task_id = ?1 AND run_id = ?2 AND status = 'pending'",
                    params![input.task_id, input.run_id],
                    |row| row.get(0),
                )
                .optional()?;
            if pending.is_some() {
                anyhow::bail!("canonical_steering_pending_conflict");
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
                    "work_steering_scope_expansion_blocked"
                } else {
                    "work_steering_pending"
                },
                input.steering_digest,
                now
            ],
        )?;
        tx.execute(
            "INSERT INTO canonical_steering (
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
            .ok_or_else(|| anyhow::anyhow!("canonical_steering_missing_after_insert"))?;
        tx.commit()?;
        Ok(record)
    }

    pub fn consume_pending_steering(
        &self,
        task_id: &str,
        run_id: &str,
    ) -> Result<Option<CanonicalSteeringRecord>> {
        validate_nonempty("task_id", task_id, 512)?;
        validate_nonempty("run_id", run_id, 512)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let steering_id = tx
            .query_row(
                "SELECT steering_id FROM canonical_steering
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
            .ok_or_else(|| anyhow::anyhow!("canonical_steering_missing_before_consume"))?;
        let (task_status, plan_revision): (String, i64) = tx.query_row(
            "SELECT task.status, run.plan_revision
             FROM canonical_tasks task
             JOIN canonical_task_runs run ON run.task_id = task.id
             WHERE task.id = ?1 AND run.run_id = ?2",
            params![task_id, run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if task_status != "running" {
            anyhow::bail!("canonical_steering_consume_target_not_running");
        }
        if u64::try_from(plan_revision)? != record.base_plan_revision {
            anyhow::bail!("canonical_steering_consume_revision_conflict");
        }
        let changed = tx.execute(
            "UPDATE canonical_steering
             SET status = 'consumed', consumed_at = ?2
             WHERE steering_id = ?1 AND status = 'pending'",
            params![steering_id, now],
        )?;
        if changed != 1 {
            anyhow::bail!("canonical_steering_consume_conflict");
        }
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'completed', summary_code = 'work_steering_consumed', updated_at = ?2
             WHERE id = ?1 AND kind = 'steering' AND status = 'waiting'",
            params![record.item_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_runs SET plan_revision = plan_revision + 1
             WHERE task_id = ?1 AND run_id = ?2 AND plan_revision = ?3",
            params![task_id, run_id, plan_revision],
        )?;
        let consumed = load_steering_in_tx(&tx, &record.steering_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_steering_missing_after_consume"))?;
        tx.commit()?;
        Ok(Some(consumed))
    }

    pub fn load_steering(&self, steering_id: &str) -> Result<Option<CanonicalSteeringRecord>> {
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

    pub fn resolve_general_run_by_execution_session(
        &self,
        execution_session_id: &str,
    ) -> Result<Option<(String, String)>> {
        validate_uuid("execution_session_id", execution_session_id)?;
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT run.task_id, run.run_id
             FROM canonical_task_runs run
             JOIN canonical_tasks task ON task.id = run.task_id
             WHERE run.execution_session_id = ?1 AND task.task_kind = 'work'",
            [execution_session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn resolve_general_task_id_by_run(&self, run_id: &str) -> Result<Option<String>> {
        validate_uuid("run_id", run_id)?;
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT task_id FROM canonical_task_runs WHERE run_id = ?1",
            [run_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn resolve_general_run_target_for_conversation(
        &self,
        task_id: &str,
        run_id: &str,
        conversation_id: &str,
    ) -> Result<Option<CanonicalRunSteeringTarget>> {
        validate_uuid("task_id", task_id)?;
        validate_uuid("run_id", run_id)?;
        validate_uuid("conversation_id", conversation_id)?;
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT task.id, run.execution_session_id, run.plan_revision
             FROM canonical_tasks task
             JOIN canonical_task_runs run ON run.task_id = task.id
             WHERE task.id = ?1 AND run.run_id = ?2
               AND task.conversation_id = ?3 AND task.task_kind = 'work'",
            params![task_id, run_id, conversation_id],
            |row| {
                let revision = u64::try_from(row.get::<_, i64>(2)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Integer,
                        error.into(),
                    )
                })?;
                Ok(CanonicalRunSteeringTarget {
                    task_id: row.get(0)?,
                    execution_session_id: row.get(1)?,
                    plan_revision: revision,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn resolve_general_task_id_for_conversation(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<Option<String>> {
        // This is a compatibility-safe lookup used at the kernel checkpoint.
        // Pre-reconstruction Conversation labels can still reach test/migration
        // consumers, where the correct answer is "no canonical Work target",
        // not a blocker that replaces the actual terminal reason. Product Work
        // admission and steering commands enforce UUID identities before this
        // read-only lookup is reached.
        validate_nonempty("conversation_id", conversation_id, 512)?;
        validate_nonempty("run_id", run_id, 512)?;
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT task.id FROM canonical_tasks task
             JOIN canonical_task_runs run ON run.task_id = task.id
             WHERE task.conversation_id = ?1 AND run.run_id = ?2
               AND task.task_kind = 'work'",
            params![conversation_id, run_id],
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

    pub fn list_consumed_steering(
        &self,
        task_id: &str,
        run_id: &str,
    ) -> Result<Vec<CanonicalSteeringRecord>> {
        validate_nonempty("task_id", task_id, 512)?;
        validate_nonempty("run_id", run_id, 512)?;
        let conn = self.lock_conn()?;
        let ids = {
            let mut statement = conn.prepare(
                "SELECT steering_id FROM canonical_steering
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
                     FROM canonical_steering WHERE steering_id = ?1",
                    [steering_id],
                    row_to_steering,
                )
                .optional()?
                .ok_or_else(|| anyhow::anyhow!("canonical_steering_missing_during_list"))?;
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
             ON CONFLICT(id) DO UPDATE SET updated_at = excluded.updated_at",
            params![
                task_id,
                input.conversation_id,
                input.outcome_digest,
                now_text
            ],
        )?;
        let stored_task_id: String = tx.query_row(
            "SELECT id FROM canonical_tasks WHERE id = ?1",
            [&task_id],
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
                    task_id, run_id, execution_session_id, ordinal, status,
                    execution_facts_version, plan_revision, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, 'running', 5, 1, ?5, ?5)",
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
                    materialized_reference, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 1, 'draft', ?4, ?5, ?6,
                           NULL, ?7, ?7)",
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

    pub fn prepare_general_artifact(
        &self,
        input: GeneralArtifactDraftInput<'_>,
    ) -> Result<PreparedGeneralArtifact> {
        validate_uuid("task_id", input.task_id)?;
        validate_uuid("run_id", input.run_id)?;
        validate_nonempty("target_reference", input.target_reference, 4096)?;
        validate_digest("content_digest", input.content_digest)?;
        validate_nonempty("media_type", input.media_type, 256)?;

        let target_reference_digest = sha256_text(input.target_reference);
        let artifact_id =
            general_artifact_id(input.task_id, input.target_reference, input.media_type);
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (task_kind, task_status): (String, String) = tx.query_row(
            "SELECT task_kind, status FROM canonical_tasks WHERE id = ?1",
            [input.task_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if task_kind != "work" || task_status != CanonicalTaskStatus::Running.as_str() {
            anyhow::bail!("canonical_general_artifact_task_not_running");
        }
        let run_status: String = tx.query_row(
            "SELECT status FROM canonical_task_runs WHERE task_id = ?1 AND run_id = ?2",
            params![input.task_id, input.run_id],
            |row| row.get(0),
        )?;
        if run_status != CanonicalTaskStatus::Running.as_str() {
            anyhow::bail!("canonical_general_artifact_run_not_running");
        }
        let existing = tx
            .query_row(
                "SELECT task_id, source_item_id, current_version, status, media_type,
                        target_reference_digest, content_digest
                 FROM canonical_artifacts WHERE id = ?1",
                [&artifact_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing.0 != input.task_id
                || existing.4 != input.media_type
                || existing.5 != target_reference_digest
            {
                anyhow::bail!("canonical_general_artifact_identity_conflict");
            }
            let current_version = u64::try_from(existing.2)?;
            if existing.6 == input.content_digest {
                let source_run_id: String = tx.query_row(
                    "SELECT run_id FROM canonical_task_items WHERE id = ?1",
                    [&existing.1],
                    |row| row.get(0),
                )?;
                if source_run_id != input.run_id {
                    anyhow::bail!("canonical_general_artifact_replay_run_conflict");
                }
                tx.commit()?;
                return Ok(PreparedGeneralArtifact {
                    task_id: input.task_id.to_string(),
                    artifact_draft_item_id: existing.1,
                    artifact_id,
                    version: current_version,
                });
            }
            if existing.3 == "effect_unknown" {
                anyhow::bail!("canonical_general_artifact_effect_unknown_requires_reconciliation");
            }
            if matches!(existing.3.as_str(), "draft" | "waiting_review") {
                anyhow::bail!("canonical_general_artifact_previous_version_unresolved");
            }
            let next_version = current_version
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("canonical_general_artifact_version_overflow"))?;
            let item_id = stable_id(
                "item",
                &["artifact_draft", &artifact_id, &next_version.to_string()],
            );
            let sequence: i64 = tx.query_row(
                "SELECT COALESCE(MAX(sequence), 0) + 1 FROM canonical_task_items WHERE task_id = ?1",
                [input.task_id],
                |row| row.get(0),
            )?;
            let payload_digest = sha256_text(&format!(
                "{}\0{}\0{}\0{}",
                target_reference_digest, input.content_digest, input.media_type, next_version
            ));
            tx.execute(
                "INSERT INTO canonical_task_items (
                    id, task_id, run_id, sequence, kind, status, summary_code,
                    payload_digest, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, 'artifact_draft', 'completed',
                           'work_artifact_version_prepared', ?5, ?6, ?6)",
                params![
                    item_id,
                    input.task_id,
                    input.run_id,
                    sequence,
                    payload_digest,
                    now
                ],
            )?;
            tx.execute(
                "INSERT INTO canonical_artifact_versions (
                    artifact_id, version, source_item_id, content_digest,
                    materialized_reference, observed_content_digest,
                    created_at, materialized_at
                 ) VALUES (?1, ?2, ?3, ?4, NULL, NULL, ?5, NULL)",
                params![
                    artifact_id,
                    i64::try_from(next_version)?,
                    item_id,
                    input.content_digest,
                    now
                ],
            )?;
            let changed = tx.execute(
                "UPDATE canonical_artifacts
                 SET source_item_id = ?2, current_version = ?3, status = 'draft',
                     content_digest = ?4, materialized_reference = NULL, updated_at = ?5
                 WHERE id = ?1 AND current_version = ?6",
                params![
                    artifact_id,
                    item_id,
                    i64::try_from(next_version)?,
                    input.content_digest,
                    now,
                    i64::try_from(current_version)?
                ],
            )?;
            if changed != 1 {
                anyhow::bail!("canonical_general_artifact_version_cas_failed");
            }
            tx.commit()?;
            return Ok(PreparedGeneralArtifact {
                task_id: input.task_id.to_string(),
                artifact_draft_item_id: item_id,
                artifact_id,
                version: next_version,
            });
        }
        let item_id = stable_id("item", &["artifact_draft", &artifact_id, "1"]);
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM canonical_task_items WHERE task_id = ?1",
            [input.task_id],
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
                       'work_artifact_draft_prepared', ?5, ?6, ?6)",
            params![
                item_id,
                input.task_id,
                input.run_id,
                sequence,
                payload_digest,
                now
            ],
        )?;
        tx.execute(
            "INSERT INTO canonical_artifacts (
                id, task_id, source_item_id, current_version, status,
                media_type, target_reference_digest, content_digest,
                materialized_reference, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 1, 'draft', ?4, ?5, ?6,
                       NULL, ?7, ?7)",
            params![
                artifact_id,
                input.task_id,
                item_id,
                input.media_type,
                target_reference_digest,
                input.content_digest,
                now
            ],
        )?;
        tx.execute(
            "INSERT INTO canonical_artifact_versions (
                artifact_id, version, source_item_id, content_digest,
                materialized_reference, observed_content_digest,
                created_at, materialized_at
             ) VALUES (?1, 1, ?2, ?3, NULL, NULL, ?4, NULL)",
            params![artifact_id, item_id, input.content_digest, now],
        )?;
        tx.commit()?;
        Ok(PreparedGeneralArtifact {
            task_id: input.task_id.to_string(),
            artifact_draft_item_id: item_id,
            artifact_id,
            version: 1,
        })
    }

    pub fn bind_general_artifact_version_source(
        &self,
        artifact_id: &str,
        version: u64,
        target_reference: &str,
        draft_reference: &str,
        expected_target_absent: bool,
        expected_target_digest: Option<&str>,
    ) -> Result<CanonicalArtifactVersionRecord> {
        validate_nonempty("artifact_id", artifact_id, 512)?;
        validate_nonempty("target_reference", target_reference, 4096)?;
        validate_nonempty("draft_reference", draft_reference, 4096)?;
        if version == 0 {
            anyhow::bail!("canonical_artifact_version_invalid");
        }
        match (expected_target_absent, expected_target_digest) {
            (true, None) => {}
            (false, Some(digest)) => validate_digest("expected_target_digest", digest)?,
            _ => anyhow::bail!("canonical_artifact_target_precondition_invalid"),
        }
        let conn = self.lock_conn()?;
        let target_digest: String = conn.query_row(
            "SELECT target_reference_digest FROM canonical_artifacts
             WHERE id = ?1 AND current_version = ?2",
            params![artifact_id, i64::try_from(version)?],
            |row| row.get(0),
        )?;
        if target_digest != sha256_text(target_reference) {
            anyhow::bail!("canonical_artifact_target_reference_mismatch");
        }
        let changed = conn.execute(
            "UPDATE canonical_artifact_versions
             SET target_reference = ?3, draft_reference = ?4,
                 expected_target_absent = ?5, expected_target_digest = ?6
             WHERE artifact_id = ?1 AND version = ?2
               AND target_reference IS NULL AND draft_reference IS NULL
               AND expected_target_absent IS NULL AND expected_target_digest IS NULL",
            params![
                artifact_id,
                i64::try_from(version)?,
                target_reference,
                draft_reference,
                if expected_target_absent { 1 } else { 0 },
                expected_target_digest
            ],
        )?;
        if changed != 1 {
            drop(conn);
            let existing = self
                .load_artifact_version(artifact_id, version)?
                .ok_or_else(|| anyhow::anyhow!("canonical_artifact_version_missing"))?;
            if existing.target_reference.as_deref() != Some(target_reference)
                || existing.draft_reference.as_deref() != Some(draft_reference)
                || existing.expected_target_absent != Some(expected_target_absent)
                || existing.expected_target_digest.as_deref() != expected_target_digest
            {
                anyhow::bail!("canonical_artifact_version_source_conflict");
            }
            return Ok(existing);
        }
        drop(conn);
        self.load_artifact_version(artifact_id, version)?
            .ok_or_else(|| anyhow::anyhow!("canonical_artifact_version_missing_after_source_bind"))
    }

    pub fn bind_report_review(
        &self,
        artifact_id: &str,
        proposal_id: &str,
    ) -> Result<BoundReportReview> {
        self.bind_artifact_review(artifact_id, proposal_id)
    }

    pub fn bind_artifact_review(
        &self,
        artifact_id: &str,
        proposal_id: &str,
    ) -> Result<BoundReportReview> {
        validate_nonempty("artifact_id", artifact_id, 512)?;
        validate_nonempty("proposal_id", proposal_id, 512)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (task_id, source_run_id, current_version, artifact_status): (
            String,
            String,
            i64,
            String,
        ) = tx
            .query_row(
                "SELECT artifact.task_id, item.run_id, artifact.current_version, artifact.status
                 FROM canonical_artifacts artifact
                 JOIN canonical_task_items item ON item.id = artifact.source_item_id
                 WHERE artifact.id = ?1",
                [artifact_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .with_context(|| "canonical_report_artifact_missing_before_review")?;
        if !matches!(artifact_status.as_str(), "draft" | "waiting_review") {
            anyhow::bail!("canonical_artifact_version_not_reviewable");
        }
        let version = u64::try_from(current_version)?;
        let checkpoint_item_id = stable_id(
            "item",
            &["review_checkpoint", artifact_id, &version.to_string()],
        );
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
                       'artifact_review_required', ?5, ?6, ?6)
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
            "INSERT INTO canonical_artifact_review_checkpoints (
                artifact_id, version, proposal_id, item_id, status, created_at, resolved_at
             ) VALUES (?1, ?2, ?3, ?4, 'waiting', ?5, NULL)
             ON CONFLICT(artifact_id, version) DO NOTHING",
            params![
                artifact_id,
                current_version,
                proposal_id,
                checkpoint_item_id,
                now
            ],
        )?;
        let checkpoint: (String, String, String) = tx.query_row(
            "SELECT proposal_id, item_id, status
             FROM canonical_artifact_review_checkpoints
             WHERE artifact_id = ?1 AND version = ?2",
            params![artifact_id, current_version],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if checkpoint.0 != proposal_id
            || checkpoint.1 != checkpoint_item_id
            || checkpoint.2 != "waiting"
        {
            anyhow::bail!("canonical_artifact_review_checkpoint_conflict");
        }
        let materialized_item_id = stable_id(
            "item",
            &["artifact_materialized", artifact_id, &version.to_string()],
        );
        let materialized_sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM canonical_task_items
             WHERE task_id = ?1",
            [&task_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO canonical_task_items (
                id, task_id, run_id, sequence, kind, status, summary_code,
                payload_digest, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'artifact_materialized', 'waiting',
                       'artifact_materialization_waiting', ?5, ?6, ?6)
             ON CONFLICT(id) DO NOTHING",
            params![
                materialized_item_id,
                task_id,
                source_run_id,
                materialized_sequence,
                sha256_text(proposal_id),
                now
            ],
        )?;
        tx.execute(
            "UPDATE canonical_artifacts
             SET status = 'waiting_review', updated_at = ?2
             WHERE id = ?1 AND current_version = ?3
               AND status IN ('draft', 'waiting_review')",
            params![artifact_id, now, current_version],
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = 'waiting_review', updated_at = ?2
             WHERE id = ?1 AND status IN ('running', 'waiting_review')",
            params![task_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_runs SET status = 'waiting_review', updated_at = ?3
             WHERE task_id = ?1 AND run_id = ?2
               AND status IN ('running', 'waiting_review')",
            params![task_id, source_run_id, now],
        )?;
        let attention_id = stable_id(
            "attention",
            &[
                &task_id,
                &source_run_id,
                "review_required",
                "work_review_required",
            ],
        );
        tx.execute(
            "INSERT INTO canonical_task_attention(
                attention_id,task_id,run_id,kind,reason_code,created_at,resolved_at
             ) VALUES(?1,?2,?3,'review_required','work_review_required',?4,NULL)
             ON CONFLICT(task_id,run_id,kind,reason_code) DO NOTHING",
            params![attention_id, task_id, source_run_id, now],
        )?;
        tx.commit()?;
        Ok(BoundReportReview {
            task_id,
            artifact_id: artifact_id.to_string(),
            version,
            checkpoint_item_id,
            proposal_id: proposal_id.to_string(),
        })
    }

    pub fn begin_artifact_materialization_attempt(
        &self,
        proposal_id: &str,
        attempt_id: &str,
        request_digest: &str,
    ) -> Result<Option<CanonicalTaskItemAttemptRecord>> {
        validate_nonempty("proposal_id", proposal_id, 512)?;
        validate_uuid("attempt_id", attempt_id)?;
        validate_digest("request_digest", request_digest)?;
        let owner = {
            let conn = self.lock_conn()?;
            conn.query_row(
                "SELECT artifact.task_id, source.run_id, artifact.id, checkpoint.version
                 FROM canonical_artifact_review_checkpoints checkpoint
                 JOIN canonical_artifacts artifact ON artifact.id = checkpoint.artifact_id
                 JOIN canonical_task_items source ON source.id = artifact.source_item_id
                 WHERE checkpoint.proposal_id = ?1 AND checkpoint.status = 'waiting'
                   AND checkpoint.version = artifact.current_version
                   AND artifact.status = 'waiting_review'",
                [proposal_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?
        };
        let Some((task_id, run_id, artifact_id, version)) = owner else {
            return Ok(None);
        };
        let item_id = stable_id(
            "item",
            &["artifact_materialized", &artifact_id, &version.to_string()],
        );
        self.begin_item_attempt(BeginItemAttemptInput {
            attempt_id,
            task_id: &task_id,
            run_id: &run_id,
            item_id: &item_id,
            executor_kind: "materializer",
            provider_profile_id: None,
            provider_model_id: None,
            request_digest,
        })
        .map(Some)
    }

    pub fn prepare_artifact_effect(
        &self,
        proposal_id: &str,
        dispatch_claim_id: &str,
        target_reference_digest: &str,
        content_digest: &str,
        byte_size: u64,
        media_type: &str,
    ) -> Result<Option<CanonicalArtifactEffectRecord>> {
        for (label, value) in [
            ("proposal_id", proposal_id),
            ("dispatch_claim_id", dispatch_claim_id),
            ("target_reference_digest", target_reference_digest),
            ("content_digest", content_digest),
            ("media_type", media_type),
        ] {
            validate_nonempty(label, value, 512)?;
        }
        validate_digest("target_reference_digest", target_reference_digest)?;
        validate_digest("content_digest", content_digest)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut owner = tx
            .query_row(
                "SELECT checkpoint.artifact_id, checkpoint.version, attempt.attempt_id,
                        version.content_digest, artifact.target_reference_digest
                 FROM canonical_artifact_review_checkpoints checkpoint
                 JOIN canonical_artifacts artifact ON artifact.id = checkpoint.artifact_id
                 JOIN canonical_artifact_versions version
                   ON version.artifact_id = checkpoint.artifact_id
                  AND version.version = checkpoint.version
                 JOIN canonical_task_items item ON item.id = checkpoint.item_id
                 JOIN canonical_task_item_attempts attempt
                   ON attempt.task_id = item.task_id AND attempt.run_id = item.run_id
                  AND attempt.executor_kind = 'materializer'
                  AND attempt.status = 'running'
                  AND attempt.item_id = (
                    SELECT candidate.id FROM canonical_task_items candidate
                     WHERE candidate.task_id = item.task_id
                       AND candidate.run_id = item.run_id
                       AND candidate.kind = 'artifact_materialized'
                       AND candidate.sequence > item.sequence
                     ORDER BY candidate.sequence ASC LIMIT 1
                  )
                 WHERE checkpoint.proposal_id = ?1
                   AND checkpoint.status = 'waiting'
                   AND checkpoint.version = artifact.current_version",
                [proposal_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;
        if owner.is_none() {
            owner = tx
                .query_row(
                    "SELECT undo.artifact_id, undo.version, attempt.attempt_id,
                            undo.content_digest, undo.source_reference,
                            undo.target_reference
                     FROM canonical_artifact_undo undo
                     JOIN canonical_artifacts artifact ON artifact.id = undo.artifact_id
                     JOIN canonical_task_items item
                       ON item.task_id = artifact.task_id
                      AND item.kind = 'review_checkpoint'
                      AND item.summary_code = 'artifact_undo_review_required'
                     JOIN canonical_task_item_attempts attempt
                       ON attempt.item_id = item.id
                      AND attempt.executor_kind = 'materializer'
                      AND attempt.status = 'running'
                     WHERE undo.proposal_id = ?1 AND undo.status = 'waiting_review'
                       AND undo.version = artifact.current_version",
                    [proposal_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            sha256_text(&format!(
                                "{} -> {}",
                                row.get::<_, String>(4)?,
                                row.get::<_, String>(5)?
                            )),
                        ))
                    },
                )
                .optional()?;
        }
        let Some((artifact_id, version, attempt_id, expected_content, expected_target)) = owner
        else {
            tx.commit()?;
            return Ok(None);
        };
        if expected_content != content_digest {
            anyhow::bail!("canonical_artifact_effect_content_mismatch");
        }
        if expected_target != target_reference_digest {
            anyhow::bail!("canonical_artifact_effect_target_mismatch");
        }
        tx.execute(
            "INSERT INTO canonical_artifact_effects (
                proposal_id, artifact_id, version, attempt_id, dispatch_claim_id,
                target_reference_digest, content_digest, byte_size, media_type,
                state, observed_content_digest, error_code, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                       'prepared', NULL, NULL, ?10, ?10)
             ON CONFLICT(proposal_id) DO NOTHING",
            params![
                proposal_id,
                artifact_id,
                version,
                attempt_id,
                dispatch_claim_id,
                target_reference_digest,
                content_digest,
                i64::try_from(byte_size)?,
                media_type,
                now
            ],
        )?;
        let record = load_artifact_effect_in_tx(&tx, proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("canonical_artifact_effect_missing_after_prepare"))?;
        if record.artifact_id != artifact_id
            || record.version != u64::try_from(version)?
            || record.attempt_id != attempt_id
            || record.dispatch_claim_id != dispatch_claim_id
            || record.target_reference_digest != target_reference_digest
            || record.content_digest != content_digest
            || record.byte_size != byte_size
            || record.media_type != media_type
        {
            anyhow::bail!("canonical_artifact_effect_identity_conflict");
        }
        tx.commit()?;
        Ok(Some(record))
    }

    pub fn mark_artifact_effect_staged(
        &self,
        proposal_id: &str,
        dispatch_claim_id: &str,
    ) -> Result<bool> {
        self.transition_artifact_effect(
            proposal_id,
            dispatch_claim_id,
            CanonicalArtifactEffectState::Staged,
            None,
            None,
        )
    }

    pub fn finish_artifact_effect_confirmed(
        &self,
        proposal_id: &str,
        dispatch_claim_id: &str,
        observed_content_digest: &str,
    ) -> Result<bool> {
        validate_digest("observed_content_digest", observed_content_digest)?;
        self.transition_artifact_effect(
            proposal_id,
            dispatch_claim_id,
            CanonicalArtifactEffectState::Confirmed,
            Some(observed_content_digest),
            None,
        )
    }

    pub fn finish_artifact_effect_failed_before_effect(
        &self,
        proposal_id: &str,
        dispatch_claim_id: &str,
        error_code: &str,
    ) -> Result<bool> {
        self.transition_artifact_effect(
            proposal_id,
            dispatch_claim_id,
            CanonicalArtifactEffectState::FailedBeforeEffect,
            None,
            Some(error_code),
        )
    }

    pub fn finish_artifact_effect_unknown(
        &self,
        proposal_id: &str,
        dispatch_claim_id: &str,
        error_code: &str,
    ) -> Result<bool> {
        self.transition_artifact_effect(
            proposal_id,
            dispatch_claim_id,
            CanonicalArtifactEffectState::EffectUnknown,
            None,
            Some(error_code),
        )
    }

    fn transition_artifact_effect(
        &self,
        proposal_id: &str,
        dispatch_claim_id: &str,
        next: CanonicalArtifactEffectState,
        observed_content_digest: Option<&str>,
        error_code: Option<&str>,
    ) -> Result<bool> {
        validate_nonempty("proposal_id", proposal_id, 512)?;
        validate_nonempty("dispatch_claim_id", dispatch_claim_id, 512)?;
        if let Some(error_code) = error_code {
            validate_nonempty("artifact_effect_error_code", error_code, 256)?;
        }
        let now = Utc::now().to_rfc3339();
        let conn = self.lock_conn()?;
        let allowed_prior = match next {
            CanonicalArtifactEffectState::Staged => "prepared",
            CanonicalArtifactEffectState::Confirmed => "prepared','staged",
            CanonicalArtifactEffectState::FailedBeforeEffect => "prepared",
            CanonicalArtifactEffectState::EffectUnknown => "prepared','staged",
            CanonicalArtifactEffectState::Prepared => {
                anyhow::bail!("canonical_artifact_effect_transition_invalid")
            }
        };
        let sql = format!(
            "UPDATE canonical_artifact_effects
             SET state = ?3, observed_content_digest = ?4, error_code = ?5, updated_at = ?6
             WHERE proposal_id = ?1 AND dispatch_claim_id = ?2
               AND state IN ('{allowed_prior}')"
        );
        let changed = conn.execute(
            &sql,
            params![
                proposal_id,
                dispatch_claim_id,
                next.as_str(),
                observed_content_digest,
                error_code,
                now
            ],
        )?;
        if changed == 1 {
            return Ok(true);
        }
        let existing = load_artifact_effect_in_conn(&conn, proposal_id)?;
        Ok(existing.is_some_and(|record| {
            record.dispatch_claim_id == dispatch_claim_id
                && record.state == next
                && record.observed_content_digest.as_deref() == observed_content_digest
                && record.error_code.as_deref() == error_code
        }))
    }

    pub fn load_artifact_effect(
        &self,
        proposal_id: &str,
    ) -> Result<Option<CanonicalArtifactEffectRecord>> {
        let conn = self.lock_conn()?;
        load_artifact_effect_in_conn(&conn, proposal_id)
    }

    pub fn list_artifact_effects_for_reconciliation(
        &self,
        limit: u64,
    ) -> Result<Vec<CanonicalArtifactEffectRecord>> {
        let conn = self.lock_conn()?;
        let mut statement = conn.prepare(
            "SELECT artifact_id, version, proposal_id, attempt_id, dispatch_claim_id,
                    target_reference_digest, content_digest, byte_size, media_type,
                    state, observed_content_digest, error_code, created_at, updated_at
             FROM canonical_artifact_effects
             WHERE state IN ('prepared', 'staged')
             ORDER BY created_at ASC, proposal_id ASC LIMIT ?1",
        )?;
        let rows = statement.query_map(
            [i64::try_from(limit.clamp(1, 200))?],
            row_to_artifact_effect,
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
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
        let (artifact_id, task_id, source_item_id, current_version, expected_digest): (
            String,
            String,
            String,
            i64,
            String,
        ) = tx
            .query_row(
                "SELECT artifact.id, artifact.task_id, artifact.source_item_id,
                        artifact.current_version, artifact.content_digest
                 FROM canonical_artifact_review_checkpoints checkpoint
                 JOIN canonical_artifacts artifact ON artifact.id = checkpoint.artifact_id
                 WHERE checkpoint.proposal_id = ?1
                   AND checkpoint.version = artifact.current_version",
                [proposal_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .with_context(|| "canonical_report_artifact_missing_for_confirmed_proposal")?;
        if expected_digest != observed_content_digest {
            anyhow::bail!("canonical_report_artifact_observed_digest_mismatch");
        }
        let version_changed = tx.execute(
            "UPDATE canonical_artifact_versions
             SET materialized_reference = ?3, observed_content_digest = ?4,
                 materialized_at = ?5
                 WHERE artifact_id = ?1
                   AND version = (SELECT current_version FROM canonical_artifacts WHERE id = ?1)
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
                 WHERE artifact_id = ?1
                   AND version = (SELECT current_version FROM canonical_artifacts WHERE id = ?1)
                   AND content_digest = ?2",
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
             WHERE id = ?1 AND current_version = ?4 AND status = 'waiting_review'",
            params![artifact_id, materialized_reference, now, current_version],
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
        let checkpoint_item_id = stable_id(
            "item",
            &[
                "review_checkpoint",
                &artifact_id,
                &current_version.to_string(),
            ],
        );
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'completed', summary_code = 'artifact_review_accepted',
                 updated_at = ?2
             WHERE id = ?1 AND kind = 'review_checkpoint'
               AND status = 'waiting'",
            params![checkpoint_item_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_artifact_review_checkpoints
             SET status = 'accepted', resolved_at = ?2
             WHERE proposal_id = ?1 AND status = 'waiting'",
            params![proposal_id, now],
        )?;
        let materialized_item_id = stable_id(
            "item",
            &[
                "artifact_materialized",
                &artifact_id,
                &current_version.to_string(),
            ],
        );
        let run_id: String = tx.query_row(
            "SELECT run_id FROM canonical_task_items WHERE id = ?1",
            [&source_item_id],
            |row| row.get(0),
        )?;
        let materialized_payload_digest = sha256_text(&format!(
            "{}\0{}",
            materialized_reference, observed_content_digest
        ));
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'completed', summary_code = 'artifact_materialized',
                 payload_digest = ?2, updated_at = ?3
             WHERE id = ?1 AND kind = 'artifact_materialized'
               AND status IN ('waiting', 'running')",
            params![materialized_item_id, materialized_payload_digest, now],
        )?;
        let materializer_receipt_digest = sha256_text(&format!(
            "{}\0{}\0{}",
            proposal_id, materialized_reference, observed_content_digest
        ));
        tx.execute(
            "UPDATE canonical_task_item_attempts
             SET status = 'completed', receipt_digest = ?2, finished_at = ?3
             WHERE item_id = ?1 AND executor_kind = 'materializer' AND status = 'running'",
            params![materialized_item_id, materializer_receipt_digest, now],
        )?;

        let verification_item_id = report_verification_item_id(
            &artifact_id,
            u64::try_from(current_version)?,
            observed_content_digest,
        );
        let verification_payload_digest = sha256_text(&format!(
            "{}\0{}\0{}\0{}\0{}",
            artifact_id,
            current_version,
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
                summary_code: "artifact_version_verified",
                payload_digest: &verification_payload_digest,
                now: &now,
            },
        )?;

        let artifact_facts = {
            let mut statement = tx.prepare(
                "SELECT artifact.id, artifact.current_version, artifact.content_digest,
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
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut all_verified = !artifact_facts.is_empty();
        let mut final_result_facts = Vec::with_capacity(artifact_facts.len());
        for (candidate_id, candidate_version, content_digest, reference, observed_digest) in
            &artifact_facts
        {
            let (Some(reference), Some(observed_digest)) = (reference, observed_digest) else {
                all_verified = false;
                continue;
            };
            if observed_digest != content_digest {
                all_verified = false;
                continue;
            }
            let candidate_verification_id = report_verification_item_id(
                candidate_id,
                u64::try_from(*candidate_version)?,
                observed_digest,
            );
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
                "{}\0{}\0{}\0{}\0{}",
                candidate_id,
                candidate_version,
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
                    summary_code: "work_artifact_final_result_verified",
                    payload_digest: &final_result_payload_digest,
                    now: &now,
                },
            )?;
            if let Some((conversation_item_id, result_digest, summary_code)) = tx
                .query_row(
                    "SELECT conversation_item_id, result_digest, summary_code
                     FROM canonical_task_deferred_results
                     WHERE task_id = ?1 AND run_id = ?2",
                    params![task_id, run_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?
            {
                tx.execute(
                    "INSERT INTO canonical_task_final_results (
                        task_id, run_id, item_id, conversation_item_id,
                        result_digest, summary_code, created_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(task_id) DO NOTHING",
                    params![
                        task_id,
                        run_id,
                        final_result_item_id,
                        conversation_item_id,
                        result_digest,
                        summary_code,
                        now
                    ],
                )?;
            }
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
        tx.execute(
            "UPDATE canonical_task_runs SET status = ?3, updated_at = ?4,
                    completed_at = CASE WHEN ?3 = 'completed' THEN ?4 ELSE completed_at END
             WHERE task_id = ?1 AND run_id = ?2",
            params![
                task_id,
                run_id,
                if all_verified {
                    CanonicalTaskStatus::Completed.as_str()
                } else {
                    CanonicalTaskStatus::WaitingReview.as_str()
                },
                now
            ],
        )?;
        if all_verified {
            tx.execute(
                "UPDATE canonical_task_attention SET resolved_at = ?3
                 WHERE task_id = ?1 AND run_id = ?2 AND resolved_at IS NULL",
                params![task_id, run_id, now],
            )?;
        }
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
        let owner = tx
            .query_row(
                "SELECT artifact.task_id, item.run_id, artifact.id, checkpoint.version
                 FROM canonical_artifact_review_checkpoints checkpoint
                 JOIN canonical_artifacts artifact ON artifact.id = checkpoint.artifact_id
                 JOIN canonical_task_items item ON item.id = artifact.source_item_id
                 WHERE checkpoint.proposal_id = ?1
                   AND checkpoint.version = artifact.current_version",
                [proposal_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((task_id, run_id, artifact_id, version)) = owner else {
            tx.commit()?;
            return Ok(());
        };
        tx.execute(
            "UPDATE canonical_artifacts SET status = 'effect_unknown', updated_at = ?2
             WHERE id = ?1 AND current_version = ?3 AND status != 'materialized'",
            params![artifact_id, now, version],
        )?;
        tx.execute(
            "UPDATE canonical_artifact_review_checkpoints
             SET status = 'effect_unknown', resolved_at = ?2
             WHERE proposal_id = ?1 AND status = 'waiting'",
            params![proposal_id, now],
        )?;
        let checkpoint_item_id = stable_id(
            "item",
            &["review_checkpoint", &artifact_id, &version.to_string()],
        );
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'effect_unknown', summary_code = ?2, updated_at = ?3
             WHERE id = ?1 AND kind = 'review_checkpoint' AND status = 'waiting'",
            params![checkpoint_item_id, reason_code, now],
        )?;
        let materialized_item_id = stable_id(
            "item",
            &["artifact_materialized", &artifact_id, &version.to_string()],
        );
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'effect_unknown', summary_code = ?2, updated_at = ?3
             WHERE id = ?1 AND kind = 'artifact_materialized'
               AND status IN ('waiting', 'running')",
            params![materialized_item_id, reason_code, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_item_attempts
             SET status = 'effect_unknown', finished_at = ?2
             WHERE item_id = ?1 AND executor_kind = 'materializer' AND status = 'running'",
            params![materialized_item_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = 'effect_unknown', updated_at = ?2
             WHERE id = ?1 AND status != 'completed'",
            params![task_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_runs SET status = 'effect_unknown', updated_at = ?3,
                    completed_at = ?3
             WHERE task_id = ?1 AND run_id = ?2 AND status != 'completed'",
            params![task_id, run_id, now],
        )?;
        let attention_id = stable_id(
            "attention",
            &[&task_id, &run_id, "effect_unknown", reason_code],
        );
        tx.execute(
            "INSERT INTO canonical_task_attention(
                attention_id,task_id,run_id,kind,reason_code,created_at,resolved_at
             ) VALUES(?1,?2,?3,'effect_unknown',?4,?5,NULL)
             ON CONFLICT(task_id,run_id,kind,reason_code) DO NOTHING",
            params![attention_id, task_id, run_id, reason_code, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_attention(
        &self,
        task_id: &str,
        run_id: &str,
        kind: CanonicalAttentionKind,
        reason_code: &str,
    ) -> Result<CanonicalAttentionRecord> {
        validate_uuid("task_id", task_id)?;
        validate_uuid("run_id", run_id)?;
        validate_nonempty("attention_reason_code", reason_code, 256)?;
        let attention_id = stable_id("attention", &[task_id, run_id, kind.as_str(), reason_code]);
        let now = Utc::now().to_rfc3339();
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO canonical_task_attention(
                attention_id,task_id,run_id,kind,reason_code,created_at,resolved_at
             ) VALUES(?1,?2,?3,?4,?5,?6,NULL)
             ON CONFLICT(task_id,run_id,kind,reason_code) DO NOTHING",
            params![
                attention_id,
                task_id,
                run_id,
                kind.as_str(),
                reason_code,
                now
            ],
        )?;
        conn.query_row(
            "SELECT attention_id,task_id,run_id,kind,reason_code,created_at,resolved_at
             FROM canonical_task_attention WHERE attention_id=?1",
            [&attention_id],
            row_to_attention,
        )
        .map_err(Into::into)
    }

    pub fn resolve_attention_for_run(&self, task_id: &str, run_id: &str) -> Result<usize> {
        validate_uuid("task_id", task_id)?;
        validate_uuid("run_id", run_id)?;
        self.lock_conn()?
            .execute(
                "UPDATE canonical_task_attention SET resolved_at=?3
                 WHERE task_id=?1 AND run_id=?2 AND resolved_at IS NULL",
                params![task_id, run_id, Utc::now().to_rfc3339()],
            )
            .map_err(Into::into)
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
        let owner = tx
            .query_row(
                "SELECT artifact.task_id, item.run_id, artifact.id, checkpoint.version
                 FROM canonical_artifact_review_checkpoints checkpoint
                 JOIN canonical_artifacts artifact ON artifact.id = checkpoint.artifact_id
                 JOIN canonical_task_items item ON item.id = artifact.source_item_id
                 WHERE checkpoint.proposal_id = ?1
                   AND checkpoint.version = artifact.current_version",
                [proposal_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((task_id, run_id, artifact_id, version)) = owner else {
            tx.commit()?;
            return Ok(());
        };
        tx.execute(
            "UPDATE canonical_artifacts SET status = 'failed', updated_at = ?2
             WHERE id = ?1 AND current_version = ?3 AND status != 'materialized'",
            params![artifact_id, now, version],
        )?;
        tx.execute(
            "UPDATE canonical_artifact_review_checkpoints
             SET status = 'failed', resolved_at = ?2
             WHERE proposal_id = ?1 AND status = 'waiting'",
            params![proposal_id, now],
        )?;
        let checkpoint_item_id = stable_id(
            "item",
            &["review_checkpoint", &artifact_id, &version.to_string()],
        );
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'failed', summary_code = ?2, updated_at = ?3
             WHERE id = ?1 AND kind = 'review_checkpoint' AND status = 'waiting'",
            params![checkpoint_item_id, reason_code, now],
        )?;
        let materialized_item_id = stable_id(
            "item",
            &["artifact_materialized", &artifact_id, &version.to_string()],
        );
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'failed', summary_code = ?2, updated_at = ?3
             WHERE id = ?1 AND kind = 'artifact_materialized'
               AND status IN ('waiting', 'running')",
            params![materialized_item_id, reason_code, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_item_attempts SET status = 'failed', finished_at = ?2
             WHERE item_id = ?1 AND executor_kind = 'materializer' AND status = 'running'",
            params![materialized_item_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = 'failed', updated_at = ?2
             WHERE id = ?1 AND status != 'completed'",
            params![task_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_runs SET status = 'failed', updated_at = ?3,
                    completed_at = ?3
             WHERE task_id = ?1 AND run_id = ?2 AND status != 'completed'",
            params![task_id, run_id, now],
        )?;
        let attention_id = stable_id("attention", &[&task_id, &run_id, "failed", reason_code]);
        tx.execute(
            "INSERT INTO canonical_task_attention(
                attention_id,task_id,run_id,kind,reason_code,created_at,resolved_at
             ) VALUES(?1,?2,?3,'failed',?4,?5,NULL)
             ON CONFLICT(task_id,run_id,kind,reason_code) DO NOTHING",
            params![attention_id, task_id, run_id, reason_code, now],
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
        let (artifact_id, task_id, run_id, version, status): (String, String, String, i64, String) =
            tx.query_row(
                "SELECT artifact.id, artifact.task_id, item.run_id,
                        checkpoint.version, artifact.status
                 FROM canonical_artifact_review_checkpoints checkpoint
                 JOIN canonical_artifacts artifact ON artifact.id = checkpoint.artifact_id
                 JOIN canonical_task_items item ON item.id = artifact.source_item_id
                 WHERE checkpoint.proposal_id = ?1
                   AND checkpoint.version = artifact.current_version",
                [proposal_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
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
        tx.execute(
            "UPDATE canonical_artifact_review_checkpoints
             SET status = 'rejected', resolved_at = ?2
             WHERE proposal_id = ?1 AND status = 'waiting'",
            params![proposal_id, now],
        )?;
        let checkpoint_item_id = stable_id(
            "item",
            &["review_checkpoint", &artifact_id, &version.to_string()],
        );
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'blocked', summary_code = 'artifact_review_rejected',
                 updated_at = ?2
             WHERE id = ?1 AND kind = 'review_checkpoint'
               AND status = 'waiting'",
            params![checkpoint_item_id, now],
        )?;
        let materialized_item_id = stable_id(
            "item",
            &["artifact_materialized", &artifact_id, &version.to_string()],
        );
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'cancelled', summary_code = 'artifact_review_rejected', updated_at = ?2
             WHERE id = ?1 AND kind = 'artifact_materialized'
               AND status IN ('waiting', 'running')",
            params![materialized_item_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_tasks SET status = 'blocked', updated_at = ?2
             WHERE id = ?1 AND status != 'blocked'",
            params![task_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_runs SET status = 'blocked', updated_at = ?3,
                    completed_at = ?3
             WHERE task_id = ?1 AND run_id = ?2 AND status != 'blocked'",
            params![task_id, run_id, now],
        )?;
        let attention_id = stable_id(
            "attention",
            &[&task_id, &run_id, "blocked", "artifact_review_rejected"],
        );
        tx.execute(
            "INSERT INTO canonical_task_attention(
                attention_id,task_id,run_id,kind,reason_code,created_at,resolved_at
             ) VALUES(?1,?2,?3,'blocked','artifact_review_rejected',?4,NULL)
             ON CONFLICT(task_id,run_id,kind,reason_code) DO NOTHING",
            params![attention_id, task_id, run_id, now],
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

    pub fn load_task_snapshot(&self, task_id: &str) -> Result<Option<CanonicalTaskSnapshot>> {
        let conn = self.lock_conn()?;
        let task = conn
            .query_row(
                "SELECT id, conversation_id, task_kind, initial_outcome_digest,
                        status, created_at, updated_at
                 FROM canonical_tasks WHERE id = ?1",
                [task_id],
                row_to_task,
            )
            .optional()?;
        let Some(task) = task else {
            return Ok(None);
        };
        Ok(Some(load_snapshot_for_task(&conn, task)?))
    }

    pub fn load_artifact(&self, artifact_id: &str) -> Result<Option<CanonicalArtifactRecord>> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, task_id, source_item_id, current_version, status,
                    media_type, target_reference_digest, content_digest,
                    materialized_reference, created_at, updated_at
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
            "SELECT artifact.id, artifact.task_id, artifact.source_item_id,
                    artifact.current_version, artifact.status, artifact.media_type,
                    artifact.target_reference_digest, artifact.content_digest,
                    artifact.materialized_reference,
                    artifact.created_at, artifact.updated_at
             FROM canonical_artifact_review_checkpoints checkpoint
             JOIN canonical_artifacts artifact ON artifact.id = checkpoint.artifact_id
             WHERE checkpoint.proposal_id = ?1",
            [proposal_id],
            row_to_artifact,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn bind_artifact_undo(
        &self,
        artifact_id: &str,
        proposal_id: &str,
        source_reference: &str,
        target_reference: &str,
        content_digest: &str,
    ) -> Result<CanonicalArtifactUndoRecord> {
        validate_nonempty("artifact_id", artifact_id, 512)?;
        validate_nonempty("proposal_id", proposal_id, 512)?;
        validate_nonempty("source_reference", source_reference, 4096)?;
        validate_nonempty("target_reference", target_reference, 4096)?;
        validate_digest("content_digest", content_digest)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let artifact: (String, String, Option<String>, String, String, i64) = tx.query_row(
            "SELECT artifact.status, artifact.content_digest, artifact.materialized_reference,
                    artifact.task_id, source.run_id, artifact.current_version
             FROM canonical_artifacts artifact
             JOIN canonical_task_items source ON source.id = artifact.source_item_id
             WHERE artifact.id = ?1",
            [artifact_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )?;
        if artifact.0 != "materialized"
            || artifact.1 != content_digest
            || artifact.2.as_deref() != Some(source_reference)
        {
            anyhow::bail!("canonical_artifact_undo_source_not_verified");
        }
        tx.execute(
            "INSERT INTO canonical_artifact_undo (
                artifact_id, version, proposal_id, source_reference, target_reference,
                content_digest, status, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'waiting_review', ?7)
             ON CONFLICT(artifact_id, version) DO NOTHING",
            params![
                artifact_id,
                artifact.5,
                proposal_id,
                source_reference,
                target_reference,
                content_digest,
                now
            ],
        )?;
        let item_id = stable_id(
            "item",
            &["artifact_undo", artifact_id, &artifact.5.to_string()],
        );
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM canonical_task_items WHERE task_id = ?1",
            [&artifact.3],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO canonical_task_items (
                id, task_id, run_id, sequence, kind, status, summary_code,
                payload_digest, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'review_checkpoint', 'waiting',
                       'artifact_undo_review_required', ?5, ?6, ?6)
             ON CONFLICT(id) DO NOTHING",
            params![
                item_id,
                artifact.3,
                artifact.4,
                sequence,
                sha256_text(proposal_id),
                now
            ],
        )?;
        let existing = load_artifact_undo_in_tx(&tx, artifact_id, u64::try_from(artifact.5)?)?
            .ok_or_else(|| anyhow::anyhow!("canonical_artifact_undo_missing_after_bind"))?;
        if existing.version != u64::try_from(artifact.5)?
            || existing.proposal_id != proposal_id
            || existing.source_reference != source_reference
            || existing.target_reference != target_reference
            || existing.content_digest != content_digest
        {
            anyhow::bail!("canonical_artifact_undo_identity_conflict");
        }
        tx.commit()?;
        Ok(existing)
    }

    pub fn begin_artifact_undo_attempt(
        &self,
        proposal_id: &str,
        attempt_id: &str,
        request_digest: &str,
    ) -> Result<Option<CanonicalTaskItemAttemptRecord>> {
        validate_uuid("attempt_id", attempt_id)?;
        validate_digest("request_digest", request_digest)?;
        let owner = {
            let conn = self.lock_conn()?;
            conn.query_row(
                "SELECT artifact.task_id, source.run_id, undo.artifact_id, undo.version
                 FROM canonical_artifact_undo undo
                 JOIN canonical_artifacts artifact ON artifact.id = undo.artifact_id
                 JOIN canonical_task_items source ON source.id = artifact.source_item_id
                 WHERE undo.proposal_id = ?1 AND undo.status = 'waiting_review'",
                [proposal_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?
        };
        let Some((task_id, run_id, artifact_id, version)) = owner else {
            return Ok(None);
        };
        let item_id = stable_id(
            "item",
            &["artifact_undo", &artifact_id, &version.to_string()],
        );
        self.begin_item_attempt(BeginItemAttemptInput {
            attempt_id,
            task_id: &task_id,
            run_id: &run_id,
            item_id: &item_id,
            executor_kind: "materializer",
            provider_profile_id: None,
            provider_model_id: None,
            request_digest,
        })
        .map(Some)
    }

    pub fn confirm_artifact_undone(
        &self,
        proposal_id: &str,
        target_reference: &str,
        observed_content_digest: &str,
    ) -> Result<CanonicalArtifactUndoRecord> {
        validate_nonempty("proposal_id", proposal_id, 512)?;
        validate_nonempty("target_reference", target_reference, 4096)?;
        validate_digest("observed_content_digest", observed_content_digest)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let undo = tx
            .query_row(
                "SELECT artifact_id, version, proposal_id, source_reference, target_reference,
                        content_digest, status, created_at, resolved_at
                 FROM canonical_artifact_undo WHERE proposal_id = ?1",
                [proposal_id],
                row_to_artifact_undo,
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("canonical_artifact_undo_missing"))?;
        if undo.target_reference != target_reference
            || undo.content_digest != observed_content_digest
        {
            anyhow::bail!("canonical_artifact_undo_receipt_mismatch");
        }
        tx.execute(
            "UPDATE canonical_artifact_undo SET status = 'undone', resolved_at = ?2
             WHERE proposal_id = ?1 AND status = 'waiting_review'",
            params![proposal_id, now],
        )?;
        let item_id = stable_id(
            "item",
            &[
                "artifact_undo",
                &undo.artifact_id,
                &undo.version.to_string(),
            ],
        );
        let receipt_digest = sha256_text(&format!(
            "{}\0{}\0{}",
            proposal_id, target_reference, observed_content_digest
        ));
        tx.execute(
            "UPDATE canonical_task_items
             SET status = 'completed', summary_code = 'artifact_undo_confirmed',
                 payload_digest = ?2, updated_at = ?3
             WHERE id = ?1 AND kind = 'review_checkpoint'
               AND status IN ('waiting', 'running')",
            params![item_id, receipt_digest, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_item_attempts
             SET status = 'completed', receipt_digest = ?2, finished_at = ?3
             WHERE item_id = ?1 AND executor_kind = 'materializer' AND status = 'running'",
            params![item_id, receipt_digest, now],
        )?;
        tx.commit()?;
        drop(conn);
        self.load_artifact_undo_version(&undo.artifact_id, undo.version)?
            .ok_or_else(|| anyhow::anyhow!("canonical_artifact_undo_missing_after_confirm"))
    }

    pub fn load_artifact_undo(
        &self,
        artifact_id: &str,
    ) -> Result<Option<CanonicalArtifactUndoRecord>> {
        let version = self
            .load_artifact(artifact_id)?
            .map(|artifact| artifact.current_version);
        let Some(version) = version else {
            return Ok(None);
        };
        self.load_artifact_undo_version(artifact_id, version)
    }

    pub fn load_artifact_undo_version(
        &self,
        artifact_id: &str,
        version: u64,
    ) -> Result<Option<CanonicalArtifactUndoRecord>> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT artifact_id, version, proposal_id, source_reference, target_reference,
                    content_digest, status, created_at, resolved_at
             FROM canonical_artifact_undo WHERE artifact_id = ?1 AND version = ?2",
            params![artifact_id, i64::try_from(version)?],
            row_to_artifact_undo,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn mark_artifact_undo_terminal(
        &self,
        proposal_id: &str,
        status: &str,
        reason_code: &str,
    ) -> Result<bool> {
        if !matches!(status, "failed" | "effect_unknown") {
            anyhow::bail!("canonical_artifact_undo_terminal_status_invalid");
        }
        validate_nonempty("reason_code", reason_code, 256)?;
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let owner = tx
            .query_row(
                "SELECT artifact_id, version FROM canonical_artifact_undo WHERE proposal_id = ?1",
                [proposal_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        let Some((artifact_id, version)) = owner else {
            tx.commit()?;
            return Ok(false);
        };
        tx.execute(
            "UPDATE canonical_artifact_undo SET status = ?2, resolved_at = ?3
             WHERE proposal_id = ?1 AND status = 'waiting_review'",
            params![proposal_id, status, now],
        )?;
        let item_id = stable_id(
            "item",
            &["artifact_undo", &artifact_id, &version.to_string()],
        );
        tx.execute(
            "UPDATE canonical_task_items SET status = ?2, summary_code = ?3, updated_at = ?4
             WHERE id = ?1 AND kind = 'review_checkpoint'
               AND status IN ('waiting', 'running')",
            params![item_id, status, reason_code, now],
        )?;
        tx.execute(
            "UPDATE canonical_task_item_attempts SET status = ?2, finished_at = ?3
             WHERE item_id = ?1 AND executor_kind = 'materializer' AND status = 'running'",
            params![item_id, status, now],
        )?;
        tx.commit()?;
        Ok(true)
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
                    created_at, materialized_at, target_reference, draft_reference,
                    expected_target_absent, expected_target_digest
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
                            status, execution_facts_version, plan_revision, created_at,
                            updated_at, completed_at, project_id, project_revision, scope_digest
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
            let attempts = {
                let mut statement = tx.prepare(
                    "SELECT attempt_id, task_id, run_id, item_id, ordinal, status,
                            executor_kind, provider_profile_id, provider_model_id,
                            request_digest, receipt_digest, started_at, finished_at
                     FROM canonical_task_item_attempts WHERE task_id = ?1
                     ORDER BY started_at ASC, attempt_id ASC",
                )?;
                let rows = statement.query_map([&task.id], row_to_item_attempt)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            };
            let final_result = load_final_result_in_tx(&tx, &task.id)?;
            let artifacts = {
                let mut statement = tx.prepare(
                    "SELECT artifact.id, artifact.task_id, artifact.source_item_id,
                            artifact.current_version, artifact.status, artifact.media_type,
                            artifact.target_reference_digest, artifact.content_digest,
                            artifact.materialized_reference,
                            artifact.created_at, artifact.updated_at,
                            version.artifact_id, version.version, version.source_item_id,
                            version.content_digest, version.materialized_reference,
                            version.observed_content_digest, version.created_at,
                            version.materialized_at, version.target_reference,
                            version.draft_reference, version.expected_target_absent,
                            version.expected_target_digest
                     FROM canonical_artifacts artifact
                     JOIN canonical_artifact_versions version
                       ON version.artifact_id = artifact.id
                      AND version.version = artifact.current_version
                     WHERE artifact.task_id = ?1
                     ORDER BY artifact.created_at ASC, artifact.id ASC",
                )?;
                let rows = statement.query_map([&task.id], row_to_artifact_snapshot)?;
                let mut snapshots = rows.collect::<std::result::Result<Vec<_>, _>>()?;
                for snapshot in &mut snapshots {
                    snapshot.review_checkpoint = load_artifact_review_checkpoint_in_tx(
                        &tx,
                        &snapshot.artifact.id,
                        snapshot.current_version.version,
                    )?;
                    snapshot.undo = load_artifact_undo_in_tx(
                        &tx,
                        &snapshot.artifact.id,
                        snapshot.current_version.version,
                    )?;
                }
                snapshots
            };
            let attention = load_attention_for_task(&tx, &task.id)?;
            snapshots.push(CanonicalTaskSnapshot {
                task,
                runs,
                items,
                attempts,
                final_result,
                artifacts,
                attention,
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

impl crate::agent::action_executor::BoundContentReceiptIssuer for CanonicalTaskRuntimeStore {
    fn issue_bound_content_receipt(
        &self,
        admission: crate::agent::action_executor::tool_executor::ObservedToolBodyAdmission,
        action: &crate::agent::AgentAction,
        observation: &crate::agent::AgentObservation,
    ) -> Result<crate::agent::ContentReceipt> {
        let key = self
            .receipt_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("canonical_task_content_receipt_key_unavailable"))?;
        let evidence = admission.into_issue_evidence();
        let field = crate::agent::types::BoundContentField::for_kind(evidence.kind());
        let run_id = action
            .tool_trace
            .as_ref()
            .and_then(|trace| trace.run_id.as_deref())
            .ok_or_else(|| anyhow::anyhow!("bound_content_receipt_run_identity_missing"))?;
        let run_status: String = self.lock_conn()?.query_row(
            "SELECT status FROM canonical_task_runs WHERE run_id = ?1",
            [run_id],
            |row| row.get(0),
        )?;
        if run_status != CanonicalTaskStatus::Running.as_str() {
            anyhow::bail!("canonical_task_content_receipt_run_not_running");
        }
        let observed_binding = crate::agent::types::ContentReceiptBinding::from_action_graph(
            run_id,
            action,
            observation,
            field,
        )?;
        let body = match field {
            crate::agent::types::BoundContentField::ActionOutputObservationContent => action
                .output
                .as_ref()
                .and_then(|value| value.get("text"))
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("bound_content_receipt_action_output_missing"))?,
            crate::agent::types::BoundContentField::ActionErrorObservationContent => action
                .error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("bound_content_receipt_action_error_missing"))?,
        };
        if observation.content != body || evidence.body() != body {
            anyhow::bail!("bound_content_receipt_observed_body_mismatch");
        }
        let canonical_binding =
            crate::agent::types::ContentReceiptBinding::from_canonical_action_graph(
                &self.store_identity,
                run_id,
                action,
                observation,
                field,
            )?;
        crate::agent::ContentReceipt::issue_durable(
            key,
            evidence,
            &observed_binding,
            &canonical_binding,
        )
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
) -> Result<Option<CanonicalSteeringRecord>> {
    tx.query_row(
        "SELECT steering_id, item_id, task_id, run_id, source_message_ref,
                source_message_digest, steering_digest, base_plan_revision,
                status, created_at, consumed_at
         FROM canonical_steering WHERE steering_id = ?1",
        [steering_id],
        row_to_steering,
    )
    .optional()
    .map_err(Into::into)
}

fn load_attention_for_task(
    conn: &Connection,
    task_id: &str,
) -> Result<Vec<CanonicalAttentionRecord>> {
    let mut statement = conn.prepare(
        "SELECT attention_id,task_id,run_id,kind,reason_code,created_at,resolved_at
         FROM canonical_task_attention WHERE task_id=?1
         ORDER BY created_at ASC,attention_id ASC",
    )?;
    let rows = statement.query_map([task_id], row_to_attention)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn row_to_attention(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalAttentionRecord> {
    let kind = CanonicalAttentionKind::from_db(&row.get::<_, String>(3)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, error.into())
    })?;
    Ok(CanonicalAttentionRecord {
        attention_id: row.get(0)?,
        task_id: row.get(1)?,
        run_id: row.get(2)?,
        kind,
        reason_code: row.get(4)?,
        created_at: parse_timestamp(row.get(5)?, "attention_created_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, error.into())
        })?,
        resolved_at: row
            .get::<_, Option<String>>(6)?
            .map(|value| parse_timestamp(value, "attention_resolved_at"))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            })?,
    })
}

fn load_attempt_in_tx(
    tx: &rusqlite::Transaction<'_>,
    attempt_id: &str,
) -> Result<Option<CanonicalTaskItemAttemptRecord>> {
    tx.query_row(
        "SELECT attempt_id, task_id, run_id, item_id, ordinal, status,
                executor_kind, provider_profile_id, provider_model_id,
                request_digest, receipt_digest, started_at, finished_at
         FROM canonical_task_item_attempts WHERE attempt_id = ?1",
        [attempt_id],
        row_to_item_attempt,
    )
    .optional()
    .map_err(Into::into)
}

fn load_final_result_in_tx(
    tx: &rusqlite::Transaction<'_>,
    task_id: &str,
) -> Result<Option<CanonicalFinalResultRecord>> {
    tx.query_row(
        "SELECT task_id, run_id, item_id, conversation_item_id, result_digest,
                summary_code, created_at
         FROM canonical_task_final_results WHERE task_id = ?1",
        [task_id],
        row_to_final_result,
    )
    .optional()
    .map_err(Into::into)
}

fn load_work_plan_in_tx(
    tx: &rusqlite::Transaction<'_>,
    run_id: &str,
) -> Result<Option<CanonicalWorkPlanRecord>> {
    tx.query_row(
        "SELECT task_id, run_id, plan_revision, schema_version, plan_json,
                plan_digest, max_plan_attempts, max_provider_attempts,
                max_tool_attempts, max_total_items, created_at
         FROM canonical_work_plans WHERE run_id = ?1",
        [run_id],
        row_to_work_plan_record,
    )
    .optional()
    .map_err(Into::into)
}

#[expect(
    clippy::too_many_arguments,
    reason = "canonical plan history insert mirrors one bounded SQLite row"
)]
fn insert_work_plan_revision_in_tx(
    tx: &rusqlite::Transaction<'_>,
    task_id: &str,
    run_id: &str,
    plan_revision: u64,
    plan: &StructuredWorkPlan,
    plan_json: &str,
    plan_digest: &str,
    budget_policy: WorkRunBudgetPolicy,
    created_at: &str,
) -> Result<()> {
    tx.execute(
        "INSERT INTO canonical_work_plan_revisions (
            run_id, task_id, plan_revision, schema_version, plan_json,
            plan_digest, max_plan_attempts, max_provider_attempts,
            max_tool_attempts, max_total_items, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            run_id,
            task_id,
            i64::try_from(plan_revision)?,
            plan.schema_version,
            plan_json,
            plan_digest,
            i64::from(budget_policy.max_plan_attempts),
            i64::from(budget_policy.max_provider_attempts),
            i64::from(budget_policy.max_tool_attempts),
            i64::from(budget_policy.max_total_items),
            created_at,
        ],
    )?;
    Ok(())
}

fn row_to_work_plan_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalWorkPlanRecord> {
    let plan_revision = u64::try_from(row.get::<_, i64>(2)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Integer, error.into())
    })?;
    let schema_version: String = row.get(3)?;
    let plan_json: String = row.get(4)?;
    let plan: StructuredWorkPlan = serde_json::from_str(&plan_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, error.into())
    })?;
    if plan.schema_version != schema_version {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            anyhow::anyhow!("canonical_work_plan_schema_projection_mismatch").into(),
        ));
    }
    let plan_digest: String = row.get(5)?;
    if sha256_text(&plan_json) != plan_digest {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            anyhow::anyhow!("canonical_work_plan_digest_mismatch").into(),
        ));
    }
    Ok(CanonicalWorkPlanRecord {
        task_id: row.get(0)?,
        run_id: row.get(1)?,
        plan_revision,
        plan,
        plan_digest,
        budget_policy: WorkRunBudgetPolicy {
            max_plan_attempts: u32::try_from(row.get::<_, i64>(6)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Integer,
                    error.into(),
                )
            })?,
            max_provider_attempts: u32::try_from(row.get::<_, i64>(7)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Integer,
                    error.into(),
                )
            })?,
            max_tool_attempts: u32::try_from(row.get::<_, i64>(8)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Integer,
                    error.into(),
                )
            })?,
            max_total_items: u32::try_from(row.get::<_, i64>(9)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    9,
                    rusqlite::types::Type::Integer,
                    error.into(),
                )
            })?,
        },
        created_at: parse_timestamp(row.get(10)?, "work_plan_created_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, error.into())
        })?,
    })
}

fn load_snapshot_for_task(
    conn: &Connection,
    task: CanonicalTaskRecord,
) -> Result<CanonicalTaskSnapshot> {
    let runs = {
        let mut statement = conn.prepare(
            "SELECT task_id, run_id, execution_session_id, ordinal,
                    status, execution_facts_version, plan_revision, created_at,
                    updated_at, completed_at, project_id, project_revision, scope_digest
             FROM canonical_task_runs WHERE task_id = ?1
             ORDER BY ordinal ASC, run_id ASC",
        )?;
        let rows = statement.query_map([&task.id], row_to_task_run)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    let items = {
        let mut statement = conn.prepare(
            "SELECT id, task_id, run_id, sequence, kind, status, summary_code,
                    payload_digest, created_at, updated_at
             FROM canonical_task_items WHERE task_id = ?1
             ORDER BY sequence ASC, id ASC",
        )?;
        let rows = statement.query_map([&task.id], row_to_item)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    let attempts = {
        let mut statement = conn.prepare(
            "SELECT attempt_id, task_id, run_id, item_id, ordinal, status,
                    executor_kind, provider_profile_id, provider_model_id,
                    request_digest, receipt_digest, started_at, finished_at
             FROM canonical_task_item_attempts WHERE task_id = ?1
             ORDER BY started_at ASC, attempt_id ASC",
        )?;
        let rows = statement.query_map([&task.id], row_to_item_attempt)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    let final_result = conn
        .query_row(
            "SELECT task_id, run_id, item_id, conversation_item_id, result_digest,
                    summary_code, created_at
             FROM canonical_task_final_results WHERE task_id = ?1",
            [&task.id],
            row_to_final_result,
        )
        .optional()?;
    let artifacts =
        {
            let mut statement = conn.prepare(
                "SELECT artifact.id, artifact.task_id, artifact.source_item_id,
                    artifact.current_version, artifact.status, artifact.media_type,
                    artifact.target_reference_digest, artifact.content_digest,
                    artifact.materialized_reference,
                    artifact.created_at, artifact.updated_at,
                    version.artifact_id, version.version, version.source_item_id,
                    version.content_digest, version.materialized_reference,
                    version.observed_content_digest, version.created_at,
                    version.materialized_at, version.target_reference,
                    version.draft_reference, version.expected_target_absent,
                    version.expected_target_digest
             FROM canonical_artifacts artifact
             JOIN canonical_artifact_versions version
               ON version.artifact_id = artifact.id
              AND version.version = artifact.current_version
             WHERE artifact.task_id = ?1
             ORDER BY artifact.created_at ASC, artifact.id ASC",
            )?;
            let rows = statement.query_map([&task.id], row_to_artifact_snapshot)?;
            let mut snapshots = rows.collect::<std::result::Result<Vec<_>, _>>()?;
            for snapshot in &mut snapshots {
                snapshot.review_checkpoint = conn
                    .query_row(
                        "SELECT artifact_id, version, proposal_id, item_id, status,
                            created_at, resolved_at
                     FROM canonical_artifact_review_checkpoints
                     WHERE artifact_id = ?1 AND version = ?2",
                        params![
                            snapshot.artifact.id,
                            i64::try_from(snapshot.current_version.version)?
                        ],
                        row_to_artifact_review_checkpoint,
                    )
                    .optional()?;
                snapshot.undo = conn
                .query_row(
                    "SELECT artifact_id, version, proposal_id, source_reference, target_reference,
                            content_digest, status, created_at, resolved_at
                     FROM canonical_artifact_undo WHERE artifact_id = ?1 AND version = ?2",
                    params![snapshot.artifact.id, i64::try_from(snapshot.current_version.version)?],
                    row_to_artifact_undo,
                )
                .optional()?;
            }
            snapshots
        };
    let attention = load_attention_for_task(conn, &task.id)?;
    Ok(CanonicalTaskSnapshot {
        task,
        runs,
        items,
        attempts,
        final_result,
        artifacts,
        attention,
    })
}

fn row_to_steering(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalSteeringRecord> {
    let status = CanonicalSteeringStatus::from_db(&row.get::<_, String>(8)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, error.into())
    })?;
    let base_plan_revision = u64::try_from(row.get::<_, i64>(7)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Integer, error.into())
    })?;
    Ok(CanonicalSteeringRecord {
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

fn validate_uuid(field: &str, value: &str) -> Result<()> {
    let parsed = uuid::Uuid::parse_str(value)
        .with_context(|| format!("canonical_task_runtime_{field}_invalid"))?;
    if parsed.get_version_num() != 4 || parsed.is_nil() || parsed.to_string() != value {
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
        materialized_reference: row.get(8)?,
        created_at: parse_timestamp(row.get(9)?, "artifact_created_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, error.into())
        })?,
        updated_at: parse_timestamp(row.get(10)?, "artifact_updated_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, error.into())
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
        target_reference: row.get(8)?,
        draft_reference: row.get(9)?,
        expected_target_absent: row.get::<_, Option<i64>>(10)?.map(|value| value != 0),
        expected_target_digest: row.get(11)?,
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
    let status = CanonicalTaskStatus::from_db(&row.get::<_, String>(4)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, error.into())
    })?;
    let execution_facts_version = u64::try_from(row.get::<_, i64>(5)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Integer, error.into())
    })?;
    let plan_revision = u64::try_from(row.get::<_, i64>(6)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Integer, error.into())
    })?;
    Ok(CanonicalTaskRunRecord {
        task_id: row.get(0)?,
        run_id: row.get(1)?,
        execution_session_id: row.get(2)?,
        ordinal,
        status,
        execution_facts_version,
        plan_revision,
        created_at: parse_timestamp(row.get(7)?, "task_run_created_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, error.into())
        })?,
        updated_at: parse_timestamp(row.get(8)?, "task_run_updated_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, error.into())
        })?,
        completed_at: row
            .get::<_, Option<String>>(9)?
            .map(|value| parse_timestamp(value, "task_run_completed_at"))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    9,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            })?,
        project_id: row.get(10)?,
        project_revision: row
            .get::<_, Option<i64>>(11)?
            .map(u64::try_from)
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    11,
                    rusqlite::types::Type::Integer,
                    error.into(),
                )
            })?,
        scope_digest: row.get(12)?,
    })
}

fn row_to_item_attempt(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CanonicalTaskItemAttemptRecord> {
    let ordinal = u64::try_from(row.get::<_, i64>(4)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Integer, error.into())
    })?;
    let status = CanonicalTaskItemStatus::from_db(&row.get::<_, String>(5)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, error.into())
    })?;
    Ok(CanonicalTaskItemAttemptRecord {
        attempt_id: row.get(0)?,
        task_id: row.get(1)?,
        run_id: row.get(2)?,
        item_id: row.get(3)?,
        ordinal,
        status,
        executor_kind: row.get(6)?,
        provider_profile_id: row.get(7)?,
        provider_model_id: row.get(8)?,
        request_digest: row.get(9)?,
        receipt_digest: row.get(10)?,
        started_at: parse_timestamp(row.get(11)?, "item_attempt_started_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, error.into())
        })?,
        finished_at: row
            .get::<_, Option<String>>(12)?
            .map(|value| parse_timestamp(value, "item_attempt_finished_at"))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    12,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            })?,
    })
}

fn row_to_final_result(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalFinalResultRecord> {
    Ok(CanonicalFinalResultRecord {
        task_id: row.get(0)?,
        run_id: row.get(1)?,
        item_id: row.get(2)?,
        conversation_item_id: row.get(3)?,
        result_digest: row.get(4)?,
        summary_code: row.get(5)?,
        created_at: parse_timestamp(row.get(6)?, "final_result_created_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, error.into())
        })?,
    })
}

fn row_to_artifact_snapshot(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CanonicalArtifactSnapshot> {
    let artifact = row_to_artifact(row)?;
    let version = u64::try_from(row.get::<_, i64>(12)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(12, rusqlite::types::Type::Integer, error.into())
    })?;
    let current_version = CanonicalArtifactVersionRecord {
        artifact_id: row.get(11)?,
        version,
        source_item_id: row.get(13)?,
        content_digest: row.get(14)?,
        materialized_reference: row.get(15)?,
        observed_content_digest: row.get(16)?,
        target_reference: row.get(19)?,
        draft_reference: row.get(20)?,
        expected_target_absent: row.get::<_, Option<i64>>(21)?.map(|value| value != 0),
        expected_target_digest: row.get(22)?,
        created_at: parse_timestamp(row.get(17)?, "artifact_version_created_at").map_err(
            |error| {
                rusqlite::Error::FromSqlConversionFailure(
                    17,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            },
        )?,
        materialized_at: row
            .get::<_, Option<String>>(18)?
            .map(|value| parse_timestamp(value, "artifact_version_materialized_at"))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    18,
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
        review_checkpoint: None,
        undo: None,
    })
}

fn row_to_artifact_effect(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CanonicalArtifactEffectRecord> {
    let version = u64::try_from(row.get::<_, i64>(1)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Integer, error.into())
    })?;
    let byte_size = u64::try_from(row.get::<_, i64>(7)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Integer, error.into())
    })?;
    let state =
        CanonicalArtifactEffectState::from_db(&row.get::<_, String>(9)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, error.into())
        })?;
    Ok(CanonicalArtifactEffectRecord {
        artifact_id: row.get(0)?,
        version,
        proposal_id: row.get(2)?,
        attempt_id: row.get(3)?,
        dispatch_claim_id: row.get(4)?,
        target_reference_digest: row.get(5)?,
        content_digest: row.get(6)?,
        byte_size,
        media_type: row.get(8)?,
        state,
        observed_content_digest: row.get(10)?,
        error_code: row.get(11)?,
        created_at: parse_timestamp(row.get(12)?, "artifact_effect_created_at").map_err(
            |error| {
                rusqlite::Error::FromSqlConversionFailure(
                    12,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            },
        )?,
        updated_at: parse_timestamp(row.get(13)?, "artifact_effect_updated_at").map_err(
            |error| {
                rusqlite::Error::FromSqlConversionFailure(
                    13,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            },
        )?,
    })
}

fn load_artifact_effect_in_conn(
    conn: &Connection,
    proposal_id: &str,
) -> Result<Option<CanonicalArtifactEffectRecord>> {
    conn.query_row(
        "SELECT artifact_id, version, proposal_id, attempt_id, dispatch_claim_id,
                target_reference_digest, content_digest, byte_size, media_type,
                state, observed_content_digest, error_code, created_at, updated_at
         FROM canonical_artifact_effects WHERE proposal_id = ?1",
        [proposal_id],
        row_to_artifact_effect,
    )
    .optional()
    .map_err(Into::into)
}

fn load_artifact_effect_in_tx(
    tx: &rusqlite::Transaction<'_>,
    proposal_id: &str,
) -> Result<Option<CanonicalArtifactEffectRecord>> {
    tx.query_row(
        "SELECT artifact_id, version, proposal_id, attempt_id, dispatch_claim_id,
                target_reference_digest, content_digest, byte_size, media_type,
                state, observed_content_digest, error_code, created_at, updated_at
         FROM canonical_artifact_effects WHERE proposal_id = ?1",
        [proposal_id],
        row_to_artifact_effect,
    )
    .optional()
    .map_err(Into::into)
}

fn row_to_artifact_undo(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalArtifactUndoRecord> {
    let version = u64::try_from(row.get::<_, i64>(1)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Integer, error.into())
    })?;
    Ok(CanonicalArtifactUndoRecord {
        artifact_id: row.get(0)?,
        version,
        proposal_id: row.get(2)?,
        source_reference: row.get(3)?,
        target_reference: row.get(4)?,
        content_digest: row.get(5)?,
        status: row.get(6)?,
        created_at: parse_timestamp(row.get(7)?, "artifact_undo_created_at").map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, error.into())
        })?,
        resolved_at: row
            .get::<_, Option<String>>(8)?
            .map(|value| parse_timestamp(value, "artifact_undo_resolved_at"))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            })?,
    })
}

fn row_to_artifact_review_checkpoint(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CanonicalArtifactReviewCheckpointRecord> {
    let version = u64::try_from(row.get::<_, i64>(1)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Integer, error.into())
    })?;
    Ok(CanonicalArtifactReviewCheckpointRecord {
        artifact_id: row.get(0)?,
        version,
        proposal_id: row.get(2)?,
        item_id: row.get(3)?,
        status: row.get(4)?,
        created_at: parse_timestamp(row.get(5)?, "artifact_review_checkpoint_created_at").map_err(
            |error| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            },
        )?,
        resolved_at: row
            .get::<_, Option<String>>(6)?
            .map(|value| parse_timestamp(value, "artifact_review_checkpoint_resolved_at"))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            })?,
    })
}

fn load_artifact_review_checkpoint_in_tx(
    tx: &rusqlite::Transaction<'_>,
    artifact_id: &str,
    version: u64,
) -> Result<Option<CanonicalArtifactReviewCheckpointRecord>> {
    tx.query_row(
        "SELECT artifact_id, version, proposal_id, item_id, status,
                created_at, resolved_at
         FROM canonical_artifact_review_checkpoints
         WHERE artifact_id = ?1 AND version = ?2",
        params![artifact_id, i64::try_from(version)?],
        row_to_artifact_review_checkpoint,
    )
    .optional()
    .map_err(Into::into)
}

fn load_artifact_undo_in_tx(
    tx: &rusqlite::Transaction<'_>,
    artifact_id: &str,
    version: u64,
) -> Result<Option<CanonicalArtifactUndoRecord>> {
    tx.query_row(
        "SELECT artifact_id, version, proposal_id, source_reference, target_reference,
                content_digest, status, created_at, resolved_at
         FROM canonical_artifact_undo WHERE artifact_id = ?1 AND version = ?2",
        params![artifact_id, i64::try_from(version)?],
        row_to_artifact_undo,
    )
    .optional()
    .map_err(Into::into)
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

    #[expect(
        clippy::too_many_arguments,
        reason = "test fixture names each persisted artifact fact explicitly"
    )]
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
        assert_eq!(store.list_items(&first.task_id).unwrap().len(), 6);
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
            .submit_steering(SubmitSteeringInput {
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
            .consume_pending_steering(&begun.task_id, &begun.run_id)
            .unwrap()
            .unwrap();
        assert_eq!(consumed.status, CanonicalSteeringStatus::Consumed);
        assert!(consumed.consumed_at.is_some());
        assert!(store
            .consume_pending_steering(&begun.task_id, &begun.run_id)
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
        let input = || SubmitSteeringInput {
            steering_id: "steering-replay",
            task_id: &begun.task_id,
            run_id: &begun.run_id,
            source_message_ref: "conversation://conversation-steering/message/2",
            source_message_digest: &steering_digest,
            steering_digest: &steering_digest,
            base_plan_revision: 1,
            scope_expansion_blocked: false,
        };
        let first = store.submit_steering(input()).unwrap();
        assert_eq!(store.submit_steering(input()).unwrap(), first);
        let changed_digest = digest_of("longer summary");
        let error = store
            .submit_steering(SubmitSteeringInput {
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
            .contains("canonical_steering_identity_conflict"));
        assert_eq!(store.list_items(&begun.task_id).unwrap().len(), 3);
    }

    #[test]
    fn steering_after_provider_start_is_rejected_without_an_extra_item() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: &uuid::Uuid::new_v4().to_string(),
                instruction_digest: &digest_of("write a report"),
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        store
            .append_general_item(
                &task_id,
                &run_id,
                "provider-item",
                CanonicalTaskItemKind::ProviderGeneration,
                "work_provider_generation",
                &digest_of("provider request"),
            )
            .unwrap();
        store
            .begin_item_attempt(BeginItemAttemptInput {
                attempt_id: &uuid::Uuid::new_v4().to_string(),
                task_id: &task_id,
                run_id: &run_id,
                item_id: "provider-item",
                executor_kind: "provider",
                provider_profile_id: None,
                provider_model_id: None,
                request_digest: &digest_of("provider request"),
            })
            .unwrap();
        let before = store.list_items(&task_id).unwrap().len();
        let steering_digest = digest_of("change the conclusion");
        let error = store
            .submit_steering(SubmitSteeringInput {
                steering_id: "late-steering",
                task_id: &task_id,
                run_id: &run_id,
                source_message_ref: "conversation://late",
                source_message_digest: &steering_digest,
                steering_digest: &steering_digest,
                base_plan_revision: 1,
                scope_expansion_blocked: false,
            })
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("canonical_steering_checkpoint_passed"));
        assert_eq!(store.list_items(&task_id).unwrap().len(), before);
    }

    #[test]
    fn compatibility_conversation_label_is_not_misreported_as_a_work_steering_failure() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        assert_eq!(
            store
                .resolve_general_task_id_for_conversation(
                    "pre-reconstruction-conversation-label",
                    "pre-reconstruction-run-label",
                )
                .unwrap(),
            None
        );
    }

    #[test]
    fn project_scope_and_attention_survive_restart_and_completion_resolves_attention() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("task-runtime-test.db");
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let project_id = uuid::Uuid::new_v4().to_string();
        {
            let store = CanonicalTaskRuntimeStore::new(&path).unwrap();
            store
                .begin_general_task_run(BeginGeneralTaskRunInput {
                    task_id: &task_id,
                    conversation_id: &uuid::Uuid::new_v4().to_string(),
                    run_id: &run_id,
                    execution_session_id: &uuid::Uuid::new_v4().to_string(),
                    instruction_digest: &digest_of("scoped task"),
                    plan_digest: None,
                    project_id: Some(&project_id),
                    project_revision: Some(3),
                    scope_digest: Some(&digest_of("scope revision 3")),
                })
                .unwrap();
            store
                .record_attention(
                    &task_id,
                    &run_id,
                    CanonicalAttentionKind::ReviewRequired,
                    "work_review_required",
                )
                .unwrap();
        }
        let reopened = CanonicalTaskRuntimeStore::new(&path).unwrap();
        let snapshot = reopened.load_task_snapshot(&task_id).unwrap().unwrap();
        assert_eq!(
            snapshot.runs[0].project_id.as_deref(),
            Some(project_id.as_str())
        );
        assert_eq!(snapshot.runs[0].project_revision, Some(3));
        assert_eq!(snapshot.attention.len(), 1);
        let final_item_id = stable_id("item", &["final_result", &task_id, &run_id]);
        reopened
            .complete_general_task(CompleteGeneralTaskInput {
                task_id: &task_id,
                run_id: &run_id,
                final_item_id: &final_item_id,
                conversation_item_id: "conversation-item:r5",
                result_digest: &digest_of("done"),
                summary_code: "work_completed",
            })
            .unwrap();
        assert!(reopened
            .load_task_snapshot(&task_id)
            .unwrap()
            .unwrap()
            .attention
            .iter()
            .all(|attention| attention.resolved_at.is_some()));
    }

    #[test]
    fn scope_expanding_steering_is_recorded_blocked_and_never_consumed() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let begun = begin_steerable_report(&store);
        let steering_digest = digest_of("use another workspace");
        let steering = store
            .submit_steering(SubmitSteeringInput {
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
            .consume_pending_steering(&begun.task_id, &begun.run_id)
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
                .submit_steering(SubmitSteeringInput {
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
                .consume_pending_steering(&task_id, &run_id)
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
            .submit_steering(SubmitSteeringInput {
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
            .contains("canonical_steering_target_terminal"));
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
            .submit_steering(SubmitSteeringInput {
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
            .contains("canonical_steering_target_terminal"));
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
        assert_eq!(checkpoint.summary_code, "artifact_review_rejected");
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
    fn governed_artifact_undo_is_independent_and_receipt_bound() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: "turn-undo",
                instruction_digest: &sha256_text("generate artifact"),
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        let prepared = store
            .prepare_general_artifact(GeneralArtifactDraftInput {
                task_id: &task_id,
                run_id: &run_id,
                target_reference: "/tmp/openlife/generated.md",
                content_digest: &sha256_text("# Generated"),
                media_type: "text/markdown; charset=utf-8",
            })
            .unwrap();
        store
            .bind_report_review(&prepared.artifact_id, "proposal-write")
            .unwrap();
        let artifact = store.load_artifact(&prepared.artifact_id).unwrap().unwrap();
        store
            .confirm_artifact_materialized(
                "proposal-write",
                "/tmp/openlife/generated.md",
                &artifact.content_digest,
            )
            .unwrap();
        let undo = store
            .bind_artifact_undo(
                &prepared.artifact_id,
                "proposal-undo",
                "/tmp/openlife/generated.md",
                "/tmp/openlife/.openlife-trash-generated.md",
                &artifact.content_digest,
            )
            .unwrap();
        assert_eq!(undo.status, "waiting_review");
        let attempt_id = uuid::Uuid::new_v4().to_string();
        store
            .begin_artifact_undo_attempt("proposal-undo", &attempt_id, &sha256_text("undo request"))
            .unwrap()
            .unwrap();
        let confirmed = store
            .confirm_artifact_undone(
                "proposal-undo",
                "/tmp/openlife/.openlife-trash-generated.md",
                &artifact.content_digest,
            )
            .unwrap();
        assert_eq!(confirmed.status, "undone");
        let snapshot = store
            .load_task_snapshot(&prepared.task_id)
            .unwrap()
            .unwrap();
        let undo_attempt = snapshot
            .attempts
            .iter()
            .find(|attempt| attempt.attempt_id == attempt_id)
            .unwrap();
        assert_eq!(undo_attempt.status, CanonicalTaskItemStatus::Completed);
        assert!(undo_attempt.receipt_digest.is_some());
        assert_eq!(snapshot.task.status, CanonicalTaskStatus::Completed);
        assert_eq!(
            snapshot.artifacts[0].undo.as_ref().unwrap().status,
            "undone"
        );
        let listed = store.list_task_snapshots(10).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            listed[0].artifacts[0].undo.as_ref().unwrap().status,
            "undone"
        );
    }

    #[test]
    fn general_artifact_keeps_stable_identity_across_exact_versions() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let first_run_id = uuid::Uuid::new_v4().to_string();
        store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &first_run_id,
                execution_session_id: "turn-artifact-v1",
                instruction_digest: &sha256_text("create a durable result"),
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        let target = "/safe/stable-result.md";
        let media_type = "text/markdown; charset=utf-8";
        let first_digest = sha256_text("# Version 1");
        let first = store
            .prepare_general_artifact(GeneralArtifactDraftInput {
                task_id: &task_id,
                run_id: &first_run_id,
                target_reference: target,
                content_digest: &first_digest,
                media_type,
            })
            .unwrap();
        assert_eq!(first.version, 1);
        assert_eq!(
            first.artifact_id,
            general_artifact_id(&task_id, target, media_type)
        );
        store
            .bind_general_artifact_version_source(
                &first.artifact_id,
                first.version,
                target,
                "/managed-drafts/stable-result-v1.draft",
                true,
                None,
            )
            .unwrap();
        store
            .bind_artifact_review(&first.artifact_id, "proposal-artifact-v1")
            .unwrap();
        let materializer_attempt_id = uuid::Uuid::new_v4().to_string();
        store
            .begin_artifact_materialization_attempt(
                "proposal-artifact-v1",
                &materializer_attempt_id,
                &sha256_text("materialize stable artifact v1"),
            )
            .unwrap()
            .unwrap();
        let effect = store
            .prepare_artifact_effect(
                "proposal-artifact-v1",
                "dispatch-artifact-v1",
                &sha256_text(target),
                &first_digest,
                "# Version 1".len() as u64,
                media_type,
            )
            .unwrap()
            .unwrap();
        assert_eq!(effect.version, 1);
        assert!(store
            .mark_artifact_effect_staged("proposal-artifact-v1", "dispatch-artifact-v1")
            .unwrap());
        assert!(store
            .finish_artifact_effect_confirmed(
                "proposal-artifact-v1",
                "dispatch-artifact-v1",
                &first_digest,
            )
            .unwrap());
        assert!(store
            .list_artifact_effects_for_reconciliation(10)
            .unwrap()
            .is_empty());
        store
            .confirm_artifact_materialized("proposal-artifact-v1", target, &first_digest)
            .unwrap();

        let second_run_id = uuid::Uuid::new_v4().to_string();
        store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &second_run_id,
                execution_session_id: "turn-artifact-v2",
                instruction_digest: &sha256_text("create a durable result"),
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        let second_digest = sha256_text("# Version 2");
        let second = store
            .prepare_general_artifact(GeneralArtifactDraftInput {
                task_id: &task_id,
                run_id: &second_run_id,
                target_reference: target,
                content_digest: &second_digest,
                media_type,
            })
            .unwrap();
        assert_eq!(second.artifact_id, first.artifact_id);
        assert_eq!(second.version, 2);
        store
            .bind_general_artifact_version_source(
                &second.artifact_id,
                second.version,
                target,
                "/managed-drafts/stable-result-v2.draft",
                false,
                Some(&first_digest),
            )
            .unwrap();
        assert_eq!(
            store
                .load_artifact_version(&first.artifact_id, 1)
                .unwrap()
                .unwrap()
                .content_digest,
            first_digest
        );
        assert_eq!(
            store
                .load_artifact_version(&first.artifact_id, 2)
                .unwrap()
                .unwrap()
                .content_digest,
            second_digest
        );
        let stale_confirmation = store
            .confirm_artifact_materialized("proposal-artifact-v1", target, &second_digest)
            .unwrap_err();
        assert!(stale_confirmation
            .to_string()
            .contains("canonical_report_artifact_missing_for_confirmed_proposal"));
        let second_review = store
            .bind_artifact_review(&second.artifact_id, "proposal-artifact-v2")
            .unwrap();
        assert_eq!(second_review.version, 2);
        store
            .confirm_artifact_materialized("proposal-artifact-v2", target, &second_digest)
            .unwrap();
        let snapshot = store.load_task_snapshot(&task_id).unwrap().unwrap();
        assert_eq!(snapshot.artifacts.len(), 1);
        assert_eq!(snapshot.artifacts[0].current_version.version, 2);
        assert_eq!(
            snapshot.artifacts[0]
                .current_version
                .expected_target_digest
                .as_deref(),
            Some(first_digest.as_str())
        );
        assert_eq!(
            snapshot.artifacts[0]
                .review_checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.proposal_id.as_str()),
            Some("proposal-artifact-v2")
        );
    }

    #[test]
    fn canonical_artifact_effect_journal_survives_restart_without_second_owner() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("task-runtime.db");
        let task_id = uuid::Uuid::new_v4().to_string();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let proposal_id = "proposal-restart-effect";
        let dispatch_claim_id = "dispatch-restart-effect";
        let target = "/safe/restart-effect.md";
        let content_digest = sha256_text("# Restart effect");
        let attempt_id = uuid::Uuid::new_v4().to_string();
        {
            let store = CanonicalTaskRuntimeStore::new(&path).unwrap();
            store
                .begin_general_task_run(BeginGeneralTaskRunInput {
                    task_id: &task_id,
                    conversation_id: &conversation_id,
                    run_id: &run_id,
                    execution_session_id: "turn-restart-effect",
                    instruction_digest: &sha256_text("create restart effect"),
                    plan_digest: None,
                    project_id: None,
                    project_revision: None,
                    scope_digest: None,
                })
                .unwrap();
            let prepared = store
                .prepare_general_artifact(GeneralArtifactDraftInput {
                    task_id: &task_id,
                    run_id: &run_id,
                    target_reference: target,
                    content_digest: &content_digest,
                    media_type: "text/markdown; charset=utf-8",
                })
                .unwrap();
            store
                .bind_artifact_review(&prepared.artifact_id, proposal_id)
                .unwrap();
            store
                .begin_artifact_materialization_attempt(
                    proposal_id,
                    &attempt_id,
                    &sha256_text("restart effect request"),
                )
                .unwrap()
                .unwrap();
            store
                .prepare_artifact_effect(
                    proposal_id,
                    dispatch_claim_id,
                    &sha256_text(target),
                    &content_digest,
                    "# Restart effect".len() as u64,
                    "text/markdown; charset=utf-8",
                )
                .unwrap()
                .unwrap();
        }
        let reopened = CanonicalTaskRuntimeStore::new(&path).unwrap();
        let prepared = reopened
            .list_artifact_effects_for_reconciliation(10)
            .unwrap();
        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].state, CanonicalArtifactEffectState::Prepared);
        reopened
            .mark_artifact_effect_staged(proposal_id, dispatch_claim_id)
            .unwrap();
        drop(reopened);
        let reopened = CanonicalTaskRuntimeStore::new(&path).unwrap();
        assert_eq!(
            reopened
                .list_artifact_effects_for_reconciliation(10)
                .unwrap()[0]
                .state,
            CanonicalArtifactEffectState::Staged
        );
        reopened
            .finish_artifact_effect_confirmed(proposal_id, dispatch_claim_id, &content_digest)
            .unwrap();
        assert!(reopened
            .list_artifact_effects_for_reconciliation(10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn deferred_artifact_result_survives_restart_and_becomes_final_result() {
        let directory = tempfile::tempdir().unwrap();
        let db_path = directory.path().join("task-runtime.db");
        let task_id = uuid::Uuid::new_v4().to_string();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let digest = sha256_text("# Restart-safe result");
        let proposal_id = "proposal-restart-artifact";
        let artifact_id = {
            let store = CanonicalTaskRuntimeStore::new(&db_path).unwrap();
            store
                .begin_general_task_run(BeginGeneralTaskRunInput {
                    task_id: &task_id,
                    conversation_id: &conversation_id,
                    run_id: &run_id,
                    execution_session_id: "turn-restart-artifact",
                    instruction_digest: &sha256_text("create restart-safe artifact"),
                    plan_digest: None,
                    project_id: None,
                    project_revision: None,
                    scope_digest: None,
                })
                .unwrap();
            let prepared = store
                .prepare_general_artifact(GeneralArtifactDraftInput {
                    task_id: &task_id,
                    run_id: &run_id,
                    target_reference: "/safe/restart-artifact.md",
                    content_digest: &digest,
                    media_type: "text/markdown; charset=utf-8",
                })
                .unwrap();
            store
                .bind_artifact_review(&prepared.artifact_id, proposal_id)
                .unwrap();
            store
                .defer_general_task_result(DeferGeneralTaskResultInput {
                    task_id: &task_id,
                    run_id: &run_id,
                    conversation_item_id: "conversation-item-restart-artifact",
                    result_digest: &sha256_text("waiting review reply"),
                    summary_code: "work_artifact_completed",
                })
                .unwrap();
            prepared.artifact_id
        };

        let reopened = CanonicalTaskRuntimeStore::new(&db_path).unwrap();
        reopened
            .begin_artifact_materialization_attempt(
                proposal_id,
                &uuid::Uuid::new_v4().to_string(),
                &sha256_text("materialize after restart"),
            )
            .unwrap();
        reopened
            .confirm_artifact_materialized(proposal_id, "/safe/restart-artifact.md", &digest)
            .unwrap();
        let snapshot = reopened.load_task_snapshot(&task_id).unwrap().unwrap();
        assert_eq!(snapshot.task.status, CanonicalTaskStatus::Completed);
        assert_eq!(snapshot.artifacts[0].artifact.id, artifact_id);
        assert_eq!(
            snapshot
                .final_result
                .as_ref()
                .map(|result| result.conversation_item_id.as_str()),
            Some("conversation-item-restart-artifact")
        );
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
                .load_task_snapshot(&prepared.task_id)
                .unwrap()
                .unwrap()
                .artifacts[0]
                .review_checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.proposal_id.as_str()),
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
        let conn = store.lock_conn().unwrap();
        let artifact_columns = {
            let mut statement = conn
                .prepare("PRAGMA table_info(canonical_artifacts)")
                .unwrap();
            let columns = statement
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap();
            columns
        };
        assert!(!artifact_columns
            .iter()
            .any(|column| column == "proposal_id"));
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_canonical_artifacts_task'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
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
        assert_eq!(snapshot.items.len(), 12);
        assert_eq!(snapshot.artifacts.len(), 2);
        assert!(snapshot.artifacts.iter().all(|artifact| {
            artifact.artifact.current_version == artifact.current_version.version
                && artifact.artifact.id == artifact.current_version.artifact_id
                && artifact.artifact.content_digest == artifact.current_version.content_digest
        }));
    }

    #[test]
    fn general_tasks_are_distinct_inside_one_conversation_and_runs_are_canonical() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let first_task = uuid::Uuid::new_v4().to_string();
        let second_task = uuid::Uuid::new_v4().to_string();
        let first_run = uuid::Uuid::new_v4().to_string();
        let second_run = uuid::Uuid::new_v4().to_string();
        let instruction = digest_of("prepare a bounded result");
        let first = store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &first_task,
                conversation_id: &conversation_id,
                run_id: &first_run,
                execution_session_id: &first_run,
                instruction_digest: &instruction,
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        let second = store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &second_task,
                conversation_id: &conversation_id,
                run_id: &second_run,
                execution_session_id: &second_run,
                instruction_digest: &digest_of("prepare another bounded result"),
                plan_digest: Some(&digest_of("one step")),
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        assert_ne!(first.task_id, second.task_id);
        let snapshots = store.list_task_snapshots(100).unwrap();
        assert_eq!(snapshots.len(), 2);
        assert!(snapshots
            .iter()
            .all(|snapshot| snapshot.task.conversation_id == conversation_id));
        assert!(snapshots
            .iter()
            .all(|snapshot| snapshot.task.task_kind == "work"));
    }

    #[test]
    fn general_item_attempt_and_final_result_have_one_terminal_owner() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: &run_id,
                instruction_digest: &digest_of("answer with evidence"),
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        let provider_item_id = stable_id("item", &["provider_generation", &task_id, &run_id]);
        store
            .append_general_item(
                &task_id,
                &run_id,
                &provider_item_id,
                CanonicalTaskItemKind::ProviderGeneration,
                "work_provider_generation",
                &digest_of("provider request item"),
            )
            .unwrap();
        let attempt_id = uuid::Uuid::new_v4().to_string();
        let request_digest = digest_of("provider request");
        let attempt = store
            .begin_item_attempt(BeginItemAttemptInput {
                attempt_id: &attempt_id,
                task_id: &task_id,
                run_id: &run_id,
                item_id: &provider_item_id,
                executor_kind: "provider",
                provider_profile_id: None,
                provider_model_id: None,
                request_digest: &request_digest,
            })
            .unwrap();
        assert_eq!(attempt.status, CanonicalTaskItemStatus::Running);
        store
            .terminalize_item_attempt(
                &attempt_id,
                CanonicalTaskItemStatus::Completed,
                Some(&digest_of("internal receipt")),
            )
            .unwrap();
        let final_item_id = stable_id("item", &["final_result", &task_id, &run_id]);
        let result = store
            .complete_general_task(CompleteGeneralTaskInput {
                task_id: &task_id,
                run_id: &run_id,
                final_item_id: &final_item_id,
                conversation_item_id: "conversation-item:assistant",
                result_digest: &digest_of("final answer"),
                summary_code: "work_completed",
            })
            .unwrap();
        let snapshot = store.load_task_snapshot(&task_id).unwrap().unwrap();
        assert_eq!(snapshot.task.status, CanonicalTaskStatus::Completed);
        assert_eq!(snapshot.runs[0].status, CanonicalTaskStatus::Completed);
        assert_eq!(snapshot.final_result, Some(result));
        assert!(store
            .terminalize_general_run(&task_id, &run_id, CanonicalTaskStatus::Failed)
            .is_err());
    }

    #[test]
    fn completed_observation_is_idempotent_and_rejects_payload_drift() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: &run_id,
                instruction_digest: &digest_of("review bounded evidence"),
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        let item_id = stable_id("item", &["observation", &task_id, &run_id]);
        let digest = digest_of("bounded observation");
        let first = store
            .append_completed_observation(
                &task_id,
                &run_id,
                &item_id,
                "work_bounded_observation",
                &digest,
            )
            .unwrap();
        let replay = store
            .append_completed_observation(
                &task_id,
                &run_id,
                &item_id,
                "work_bounded_observation",
                &digest,
            )
            .unwrap();
        assert_eq!(first.status, CanonicalTaskItemStatus::Completed);
        assert_eq!(replay, first);
        assert!(store
            .append_completed_observation(
                &task_id,
                &run_id,
                &item_id,
                "work_bounded_observation",
                &digest_of("drifted observation"),
            )
            .unwrap_err()
            .to_string()
            .contains("canonical_general_item_identity_conflict"));
    }

    #[test]
    fn recovery_interrupts_open_general_run_and_retry_adds_a_new_run() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let first_run = uuid::Uuid::new_v4().to_string();
        let instruction_digest = digest_of("continue after restart");
        store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &first_run,
                execution_session_id: &first_run,
                instruction_digest: &instruction_digest,
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        assert_eq!(store.recover_interrupted_general_runs().unwrap(), 1);
        let interrupted = store.load_task_snapshot(&task_id).unwrap().unwrap();
        assert_eq!(interrupted.task.status, CanonicalTaskStatus::Interrupted);
        assert_eq!(interrupted.runs[0].status, CanonicalTaskStatus::Interrupted);
        let retry_run = uuid::Uuid::new_v4().to_string();
        let retry = store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &retry_run,
                execution_session_id: &retry_run,
                instruction_digest: &instruction_digest,
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        assert_eq!(retry.ordinal, 2);
        let current = store.load_task_snapshot(&task_id).unwrap().unwrap();
        assert_eq!(current.task.status, CanonicalTaskStatus::Running);
        assert_eq!(current.runs.len(), 2);
    }

    #[test]
    fn general_task_allows_exact_replay_but_rejects_a_parallel_run() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let instruction_digest = digest_of("one active outcome");
        let input = BeginGeneralTaskRunInput {
            task_id: &task_id,
            conversation_id: &conversation_id,
            run_id: &run_id,
            execution_session_id: &run_id,
            instruction_digest: &instruction_digest,
            plan_digest: None,
            project_id: None,
            project_revision: None,
            scope_digest: None,
        };
        let first = store.begin_general_task_run(input).unwrap();
        let replay = store.begin_general_task_run(input).unwrap();
        assert_eq!(replay.run_id, first.run_id);

        let parallel_run = uuid::Uuid::new_v4().to_string();
        let error = store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &parallel_run,
                execution_session_id: &parallel_run,
                instruction_digest: &instruction_digest,
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("canonical_general_task_not_retryable"));
        assert_eq!(
            store
                .load_task_snapshot(&task_id)
                .unwrap()
                .unwrap()
                .runs
                .len(),
            1
        );

        let plan_drift = store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: &run_id,
                instruction_digest: &instruction_digest,
                plan_digest: Some(&digest_of("changed plan presence")),
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap_err();
        assert!(plan_drift
            .to_string()
            .contains("canonical_general_run_plan_conflict"));
    }

    #[test]
    fn cancelled_general_task_can_retry_with_a_new_run() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let instruction_digest = digest_of("retry cancelled outcome");
        store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: &run_id,
                instruction_digest: &instruction_digest,
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        store
            .terminalize_general_run(&task_id, &run_id, CanonicalTaskStatus::Cancelled)
            .unwrap();
        let retry_run = uuid::Uuid::new_v4().to_string();
        let retried = store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &retry_run,
                execution_session_id: &retry_run,
                instruction_digest: &instruction_digest,
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        assert_eq!(retried.ordinal, 2);
    }

    #[test]
    fn retry_runs_each_own_plan_revision_one() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let instruction_digest = digest_of("retry a planned Work task");
        let plan = StructuredWorkPlan {
            schema_version: crate::work_orchestration::WORK_PLAN_SCHEMA_VERSION.into(),
            steps: vec![crate::work_orchestration::WorkPlanStep {
                id: "deliver".into(),
                kind: crate::work_orchestration::WorkPlanStepKind::DeliverResult,
                required: true,
                depends_on: Vec::new(),
                target_id: None,
                target_contract_digest: None,
            }],
            completion: crate::work_orchestration::WorkCompletionContract {
                result_kind: crate::work_orchestration::WorkResultKind::Answer,
                requires_verification: false,
            },
        };

        let first_run = uuid::Uuid::new_v4().to_string();
        let first = store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &first_run,
                execution_session_id: &first_run,
                instruction_digest: &instruction_digest,
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        store
            .persist_work_plan(
                &task_id,
                &first_run,
                first.plan_revision,
                &plan,
                WorkRunBudgetPolicy::default(),
            )
            .unwrap();
        store
            .terminalize_general_run(&task_id, &first_run, CanonicalTaskStatus::Blocked)
            .unwrap();

        let retry_run = uuid::Uuid::new_v4().to_string();
        let retry = store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &retry_run,
                execution_session_id: &retry_run,
                instruction_digest: &instruction_digest,
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        assert_eq!(retry.ordinal, 2);
        assert_eq!(retry.plan_revision, 1);
        store
            .persist_work_plan(
                &task_id,
                &retry_run,
                retry.plan_revision,
                &plan,
                WorkRunBudgetPolicy::default(),
            )
            .unwrap();
        assert_eq!(
            store
                .load_work_plan(&first_run)
                .unwrap()
                .unwrap()
                .plan_revision,
            1
        );
        assert_eq!(
            store
                .load_work_plan(&retry_run)
                .unwrap()
                .unwrap()
                .plan_revision,
            1
        );
    }

    #[test]
    fn work_budget_usage_and_plan_declarations_survive_as_canonical_facts() {
        let store = CanonicalTaskRuntimeStore::new_in_memory().unwrap();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let begun = store
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: &run_id,
                instruction_digest: &digest_of("orchestrate this work"),
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        let structured_plan = StructuredWorkPlan {
            schema_version: crate::work_orchestration::WORK_PLAN_SCHEMA_VERSION.into(),
            steps: vec![crate::work_orchestration::WorkPlanStep {
                id: "deliver".into(),
                kind: crate::work_orchestration::WorkPlanStepKind::DeliverResult,
                required: true,
                depends_on: Vec::new(),
                target_id: None,
                target_contract_digest: None,
            }],
            completion: crate::work_orchestration::WorkCompletionContract {
                result_kind: crate::work_orchestration::WorkResultKind::Answer,
                requires_verification: false,
            },
        };
        let persisted = store
            .persist_work_plan(
                &task_id,
                &run_id,
                begun.plan_revision,
                &structured_plan,
                WorkRunBudgetPolicy::default(),
            )
            .unwrap();
        assert_eq!(persisted.plan, structured_plan);
        assert_eq!(persisted.budget_policy, WorkRunBudgetPolicy::default());
        assert_eq!(store.load_work_plan(&run_id).unwrap(), Some(persisted));
        assert_eq!(
            store.work_run_budget_policy(&run_id).unwrap(),
            WorkRunBudgetPolicy::default()
        );
        let revised_plan = StructuredWorkPlan {
            schema_version: crate::work_orchestration::WORK_PLAN_SCHEMA_VERSION.into(),
            steps: vec![
                crate::work_orchestration::WorkPlanStep {
                    id: "verify".into(),
                    kind: crate::work_orchestration::WorkPlanStepKind::Verify,
                    required: true,
                    depends_on: Vec::new(),
                    target_id: None,
                    target_contract_digest: None,
                },
                crate::work_orchestration::WorkPlanStep {
                    id: "deliver".into(),
                    kind: crate::work_orchestration::WorkPlanStepKind::DeliverResult,
                    required: true,
                    depends_on: vec!["verify".into()],
                    target_id: None,
                    target_contract_digest: None,
                },
            ],
            completion: crate::work_orchestration::WorkCompletionContract {
                result_kind: crate::work_orchestration::WorkResultKind::Answer,
                requires_verification: true,
            },
        };
        let revised = store
            .revise_work_plan(&task_id, &run_id, begun.plan_revision, &revised_plan)
            .unwrap();
        assert_eq!(revised.plan_revision, begun.plan_revision + 1);
        assert_eq!(revised.budget_policy, WorkRunBudgetPolicy::default());
        let revisions = store.list_work_plan_revisions(&run_id).unwrap();
        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].plan, structured_plan);
        assert_eq!(revisions[1].plan, revised_plan);
        assert!(store
            .revise_work_plan(&task_id, &run_id, begun.plan_revision, &revisions[0].plan)
            .unwrap_err()
            .to_string()
            .contains("canonical_work_replan_revision_stale"));
        let plan_item_id = format!("item:plan-step:{run_id}:deliver");
        store
            .append_completed_plan_item(
                &task_id,
                &run_id,
                &plan_item_id,
                "work_plan_step_declared:deliver_result",
                &digest_of("deliver step"),
            )
            .unwrap();
        let provider_item_id = format!("item:provider:{run_id}:1");
        store
            .append_general_item(
                &task_id,
                &run_id,
                &provider_item_id,
                CanonicalTaskItemKind::ProviderGeneration,
                "work_plan_generation",
                &digest_of("planner request"),
            )
            .unwrap();
        let attempt_id = uuid::Uuid::new_v4().to_string();
        store
            .begin_item_attempt(BeginItemAttemptInput {
                attempt_id: &attempt_id,
                task_id: &task_id,
                run_id: &run_id,
                item_id: &provider_item_id,
                executor_kind: "provider",
                provider_profile_id: Some("profile"),
                provider_model_id: Some("model"),
                request_digest: &digest_of("planner request"),
            })
            .unwrap();
        let usage = store.work_run_budget_usage(&run_id).unwrap();
        assert_eq!(usage.plan_attempts, 1);
        assert_eq!(usage.provider_attempts, 1);
        assert_eq!(usage.tool_attempts, 0);
        assert_eq!(usage.total_items, 3); // instruction + plan + provider
        let snapshot = store.load_task_snapshot(&task_id).unwrap().unwrap();
        assert!(snapshot.items.iter().any(|item| {
            item.id == plan_item_id && item.status == CanonicalTaskItemStatus::Completed
        }));
    }
}
