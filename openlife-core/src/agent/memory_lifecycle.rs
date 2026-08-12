use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use crate::agent::{
    main_chat_agent_v1::{IntentRiskLevel, PolicyMemoryAdmissionProof, PolicySensitivity},
    main_chat_memory_candidate::MemoryCandidateKind,
};
use crate::memory::{
    CanonicalMemoryRetrievalMutation, CanonicalMemoryRetrievalState, MemoryRetrievalDisposition,
};
use crate::persistence_outbox::{
    self, CanonicalMutationReceipt, ProjectionDelivery, ProjectionDeliveryState, ProjectionSummary,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

const MEMORY_LIFECYCLE_AGGREGATE_KIND: &str = "memory_lifecycle";
const MEMORY_LIFECYCLE_PROJECTION_TARGETS: [&str; 2] = ["memory_store", "vector_store"];
const MEMORY_LIFECYCLE_RETRIEVAL_AGGREGATE_KIND: &str = "memory_retrieval";
const MEMORY_LIFECYCLE_RETRIEVAL_OWNER_KIND: &str = "memory_lifecycle";

macro_rules! record_select_sql {
    ($suffix:expr) => {
        concat!(
            "SELECT memory_id, proposal_id, source_task_session_id, source_run_id, content, scope, scope_owner_ref, category, risk_level, sensitivity, audit_digest, status, materialization_status, materialization_error_code, created_by, accepted_by, accepted_at, materialized_view_id, materialized_view_version, evidence_ids_json, confidence, conflict_ids_json, supersedes_memory_id, replacement_memory_id, rolled_back_by_event_id, runtime_context_excluded_at FROM memory_lifecycle_records ",
            $suffix
        )
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLifecycleStatus {
    Candidate,
    PendingReview,
    EditedPendingReview,
    Accepted,
    PendingMaterialization,
    Materialized,
    MaterializationFailed,
    Rejected,
    Deferred,
    Superseded,
    RolledBack,
}

impl MemoryLifecycleStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::PendingReview => "pending_review",
            Self::EditedPendingReview => "edited_pending_review",
            Self::Accepted => "accepted",
            Self::PendingMaterialization => "pending_materialization",
            Self::Materialized => "materialized",
            Self::MaterializationFailed => "materialization_failed",
            Self::Rejected => "rejected",
            Self::Deferred => "deferred",
            Self::Superseded => "superseded",
            Self::RolledBack => "rolled_back",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "candidate" => Self::Candidate,
            "pending_review" => Self::PendingReview,
            "edited_pending_review" => Self::EditedPendingReview,
            "accepted" => Self::Accepted,
            "pending_materialization" => Self::PendingMaterialization,
            "materialized" => Self::Materialized,
            "materialization_failed" => Self::MaterializationFailed,
            "rejected" => Self::Rejected,
            "deferred" => Self::Deferred,
            "superseded" => Self::Superseded,
            "rolled_back" => Self::RolledBack,
            _ => Self::Candidate,
        }
    }

    pub fn is_runtime_active(self) -> bool {
        self == Self::Materialized
    }

    pub fn is_terminal_historical(self) -> bool {
        matches!(
            self,
            Self::Rejected | Self::Deferred | Self::Superseded | Self::RolledBack
        )
    }
}

impl std::fmt::Display for MemoryLifecycleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMaterializationStatus {
    NotRequired,
    Pending,
    Materialized,
    Failed,
}

impl MemoryMaterializationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Pending => "pending",
            Self::Materialized => "materialized",
            Self::Failed => "failed",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "not_required" => Self::NotRequired,
            "pending" => Self::Pending,
            "materialized" => Self::Materialized,
            "failed" => Self::Failed,
            _ => Self::NotRequired,
        }
    }
}

impl std::fmt::Display for MemoryMaterializationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLifecycleScope {
    Global,
    Workspace,
    Conversation,
    Project,
}

impl MemoryLifecycleScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Workspace => "workspace",
            Self::Conversation => "conversation",
            Self::Project => "project",
        }
    }

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "global" => Ok(Self::Global),
            "workspace" => Ok(Self::Workspace),
            "conversation" => Ok(Self::Conversation),
            "project" => Ok(Self::Project),
            _ => anyhow::bail!("unknown Memory lifecycle scope: {value}"),
        }
    }
}

impl std::fmt::Display for MemoryLifecycleScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLifecycleCategory {
    Preference,
    Fact,
    Workflow,
    Correction,
    Boundary,
}

pub fn memory_lifecycle_category_for_candidate_kind(
    kind: MemoryCandidateKind,
) -> MemoryLifecycleCategory {
    match kind {
        MemoryCandidateKind::EpisodicLifeEvent | MemoryCandidateKind::SemanticUserFact => {
            MemoryLifecycleCategory::Fact
        }
        MemoryCandidateKind::ProceduralRule => MemoryLifecycleCategory::Workflow,
        MemoryCandidateKind::Preference => MemoryLifecycleCategory::Preference,
        MemoryCandidateKind::IdentityOrRole => MemoryLifecycleCategory::Boundary,
    }
}

impl MemoryLifecycleCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preference => "preference",
            Self::Fact => "fact",
            Self::Workflow => "workflow",
            Self::Correction => "correction",
            Self::Boundary => "boundary",
        }
    }

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "preference" => Ok(Self::Preference),
            "fact" => Ok(Self::Fact),
            "workflow" => Ok(Self::Workflow),
            "correction" => Ok(Self::Correction),
            "boundary" => Ok(Self::Boundary),
            _ => anyhow::bail!("unknown Memory lifecycle category: {value}"),
        }
    }
}

impl std::fmt::Display for MemoryLifecycleCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLifecycleRiskLevel {
    Low,
    Medium,
    High,
    IdentityValue,
}

impl MemoryLifecycleRiskLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::IdentityValue => "identity_value",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "low" => Self::Low,
            "medium" => Self::Medium,
            "high" => Self::High,
            "identity_value" => Self::IdentityValue,
            _ => Self::High,
        }
    }

    pub fn from_intent_risk(value: IntentRiskLevel) -> Self {
        match value {
            IntentRiskLevel::Low => Self::Low,
            IntentRiskLevel::Medium => Self::Medium,
            IntentRiskLevel::High => Self::High,
            IntentRiskLevel::Critical => Self::IdentityValue,
        }
    }

    fn conservative_max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::IdentityValue => 4,
        }
    }
}

impl std::fmt::Display for MemoryLifecycleRiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLifecycleSensitivity {
    Internal,
    Sensitive,
}

impl MemoryLifecycleSensitivity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "internal",
            Self::Sensitive => "sensitive",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "internal" => Self::Internal,
            _ => Self::Sensitive,
        }
    }

    pub fn from_policy_and_candidate(
        policy: PolicySensitivity,
        candidate_sensitivity: &str,
    ) -> Self {
        if policy == PolicySensitivity::Sensitive
            || Self::from_candidate_label(candidate_sensitivity) == Self::Sensitive
        {
            Self::Sensitive
        } else {
            Self::Internal
        }
    }

    pub fn from_candidate_label(candidate_sensitivity: &str) -> Self {
        Self::from_str(candidate_sensitivity)
    }

    fn conservative_max(self, other: Self) -> Self {
        if self == Self::Sensitive || other == Self::Sensitive {
            Self::Sensitive
        } else {
            Self::Internal
        }
    }
}

impl std::fmt::Display for MemoryLifecycleSensitivity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalMemoryFactDescriptor {
    /// The exact canonical body supplied by the authorized admission. Identity
    /// normalization never rewrites this value.
    pub canonical_body: String,
    pub scope: MemoryLifecycleScope,
    /// Opaque identity of the scope selected by the user. Global facts do not
    /// carry an owner; non-global facts without one are legacy/unbound and are
    /// never eligible for normal runtime retrieval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_owner_ref: Option<String>,
    pub category: MemoryLifecycleCategory,
    pub risk_level: MemoryLifecycleRiskLevel,
    pub sensitivity: MemoryLifecycleSensitivity,
}

impl CanonicalMemoryFactDescriptor {
    pub fn new(
        canonical_body: impl Into<String>,
        scope: MemoryLifecycleScope,
        category: MemoryLifecycleCategory,
        risk_level: MemoryLifecycleRiskLevel,
        sensitivity: MemoryLifecycleSensitivity,
    ) -> Result<Self> {
        let canonical_body = canonical_body.into();
        if canonical_body.trim().is_empty() {
            anyhow::bail!("canonical Memory fact content is empty");
        }
        Ok(Self {
            canonical_body,
            scope,
            scope_owner_ref: None,
            category,
            risk_level,
            sensitivity,
        })
    }

    pub fn with_scope_owner_ref(mut self, scope_owner_ref: impl Into<String>) -> Result<Self> {
        let scope_owner_ref = scope_owner_ref.into();
        validate_scope_owner_ref(self.scope, Some(&scope_owner_ref))?;
        self.scope_owner_ref = Some(scope_owner_ref);
        Ok(self)
    }

    pub fn from_candidate(
        canonical_body: impl Into<String>,
        kind: MemoryCandidateKind,
        scope: MemoryLifecycleScope,
        risk_level: MemoryLifecycleRiskLevel,
        sensitivity: MemoryLifecycleSensitivity,
    ) -> Result<Self> {
        Self::new(
            canonical_body,
            scope,
            memory_lifecycle_category_for_candidate_kind(kind),
            risk_level,
            sensitivity,
        )
    }

    /// Stable semantic identity shared by proposal de-duplication and the
    /// canonical Memory owner. Source, confidence, evidence and run metadata
    /// are intentionally excluded by the lifecycle identity contract.
    pub fn fact_key(&self) -> Result<String> {
        canonical_memory_fact_identity(
            self.scope,
            self.scope_owner_ref.as_deref(),
            self.category,
            &self.canonical_body,
        )
        .map(|(_, fact_key)| fact_key)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLifecycleRecord {
    pub memory_id: String,
    pub proposal_id: String,
    pub source_task_session_id: Option<String>,
    pub source_run_id: Option<String>,
    pub content: String,
    pub scope: MemoryLifecycleScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_owner_ref: Option<String>,
    pub category: MemoryLifecycleCategory,
    pub risk_level: MemoryLifecycleRiskLevel,
    pub sensitivity: MemoryLifecycleSensitivity,
    pub audit_digest: String,
    pub status: MemoryLifecycleStatus,
    pub materialization_status: MemoryMaterializationStatus,
    pub materialization_error_code: Option<String>,
    pub created_by: String,
    pub accepted_by: Option<String>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub materialized_view_id: Option<String>,
    pub materialized_view_version: Option<i64>,
    pub evidence_ids: Vec<String>,
    pub confidence: f32,
    pub conflict_ids: Vec<String>,
    pub supersedes_memory_id: Option<String>,
    pub replacement_memory_id: Option<String>,
    pub rolled_back_by_event_id: Option<String>,
    pub runtime_context_excluded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRollbackEvent {
    pub rollback_event_id: String,
    pub memory_id: String,
    pub proposal_id: String,
    pub requested_by: String,
    pub reason: String,
    pub previous_status: MemoryLifecycleStatus,
    pub next_status: MemoryLifecycleStatus,
    pub affected_materialized_view_ids: Vec<String>,
    pub affected_runtime_surface_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub audit_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryMaterializedView {
    pub materialized_view_id: String,
    pub scope: Option<MemoryLifecycleScope>,
    pub version: i64,
    pub active_memory_ids: Vec<String>,
    pub runtime_surface_ids: Vec<String>,
    pub updated_at: DateTime<Utc>,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLifecycleEvent {
    pub event_id: String,
    pub memory_id: String,
    pub event_type: String,
    pub status: MemoryLifecycleStatus,
    pub materialization_status: MemoryMaterializationStatus,
    pub created_at: DateTime<Utc>,
    pub rollback_event: Option<MemoryRollbackEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryLifecycleAcceptanceInput {
    pub proposal_id: String,
    pub source_task_session_id: Option<String>,
    pub source_run_id: Option<String>,
    pub fact: CanonicalMemoryFactDescriptor,
    pub created_by: String,
    pub accepted_by: String,
    pub evidence_ids: Vec<String>,
    pub confidence: String,
    pub conflict_ids: Vec<String>,
    /// Exact canonical Memory owner replaced by this reviewed correction.
    /// Ordinary writes leave this empty; a caller cannot infer a target from
    /// similar text or vector search.
    pub supersedes_memory_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAdmissionOutcome {
    ExactReplay,
    AliasLinked,
    GovernanceUpgraded,
    OwnerCreated,
    TerminalHistorical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitMemoryWriteInput {
    pub source_task_session_id: String,
    pub source_run_id: String,
    pub source_message_id: String,
    pub source_message_digest: String,
    pub authorized_candidate_id: String,
    pub fact: CanonicalMemoryFactDescriptor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplicitMemoryWriteReceipt {
    pub receipt_id: String,
    pub memory_id: String,
    pub fact_key: String,
    pub source_message_id: String,
    pub content_digest: String,
    pub sensitivity: MemoryLifecycleSensitivity,
    pub audit_digest: String,
    pub admission_outcome: MemoryAdmissionOutcome,
    pub admission_at: DateTime<Utc>,
    pub owner_accepted_at: Option<DateTime<Utc>>,
    /// Compatibility alias for `admission_at`. This is never the owner's
    /// acceptance time unless this admission created that owner.
    pub created_at: DateTime<Utc>,
    pub newly_committed: bool,
    pub undo_available: bool,
    pub canonical_committed: bool,
    /// Metadata-only canonical outbox reference. Duplicate explicit writes
    /// reuse the existing canonical record and therefore may reuse its event.
    #[serde(default)]
    pub outbox_event_id: Option<String>,
    #[serde(default)]
    pub projection_state: ProjectionDeliveryState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_error_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLifecycleAcceptanceReport {
    pub record: MemoryLifecycleRecord,
    pub materialized_view: Option<MemoryMaterializedView>,
    /// Canonical mutations that must project before the primary mutation can
    /// truthfully be reported as fully applied. A reviewed correction uses
    /// this for the superseded owner's tombstone.
    #[serde(default)]
    pub preceding_canonical_mutations: Vec<CanonicalMutationReceipt>,
    pub canonical_mutation: Option<CanonicalMutationReceipt>,
    pub canonical_committed: bool,
    pub canonical_fact_key: String,
    pub newly_committed: bool,
    pub admission_outcome: MemoryAdmissionOutcome,
    pub admission_at: DateTime<Utc>,
    pub owner_accepted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub projection_state: ProjectionDeliveryState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRollbackReport {
    pub record: MemoryLifecycleRecord,
    pub rollback_event: MemoryRollbackEvent,
    pub materialized_view: MemoryMaterializedView,
    pub canonical_mutation: CanonicalMutationReceipt,
    pub canonical_committed: bool,
    #[serde(default)]
    pub projection_state: ProjectionDeliveryState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_error_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPrivacyEraseReport {
    pub memory_id: String,
    pub erased_at: DateTime<Utc>,
    pub materialized_view: MemoryMaterializedView,
    pub canonical_mutation: CanonicalMutationReceipt,
    pub canonical_committed: bool,
    #[serde(default)]
    pub projection_state: ProjectionDeliveryState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_error_digest: Option<String>,
}

#[derive(Clone)]
pub struct MemoryLifecycleStore {
    conn: Arc<Mutex<Connection>>,
}

/// Narrow, cloneable read authority for runtime retrieval admission. Tool and
/// provider-context code can consult current lifecycle truth without receiving
/// a canonical write interface.
#[derive(Clone)]
pub struct MemoryLifecycleRetrievalReader {
    conn: Arc<Mutex<Connection>>,
}

impl MemoryLifecycleRetrievalReader {
    /// Prove that canonical lifecycle retrieval truth is readable before a
    /// caller is allowed to report a healthy empty result.
    pub fn ensure_available(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let _ = conn
            .query_row("SELECT 1 FROM memory_lifecycle_records LIMIT 1", [], |_| {
                Ok(())
            })
            .optional()?;
        let _ = conn
            .query_row(
                "SELECT 1 FROM memory_lifecycle_retrieval_states LIMIT 1",
                [],
                |_| Ok(()),
            )
            .optional()?;
        Ok(())
    }

    pub fn is_memory_retrievable(&self, memory_id: &str) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        lifecycle_memory_is_retrievable_from_conn(&conn, memory_id)
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn install_query_failure_for_test(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute_batch(
            "ALTER TABLE memory_lifecycle_retrieval_states
             RENAME TO memory_lifecycle_retrieval_states_unavailable_for_test;",
        )?;
        Ok(())
    }
}

impl MemoryLifecycleStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open memory lifecycle db at {:?}", db_path))?;
        configure_memory_lifecycle_connection(&conn, true)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("failed to open in-memory memory lifecycle db")?;
        configure_memory_lifecycle_connection(&conn, false)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn open_read_only_existing(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = crate::sqlite_migration::open_existing_read_only(
            &db_path,
            "memory_lifecycle_store",
            &["memory_lifecycle_records"],
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn retrieval_reader(&self) -> MemoryLifecycleRetrievalReader {
        MemoryLifecycleRetrievalReader {
            conn: Arc::clone(&self.conn),
        }
    }

    fn init_tables(&self) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS memory_lifecycle_records (
                memory_id TEXT PRIMARY KEY,
                proposal_id TEXT NOT NULL,
                fact_key TEXT NOT NULL CHECK(TRIM(fact_key) != ''),
                source_task_session_id TEXT,
                source_run_id TEXT,
                content TEXT NOT NULL,
                scope TEXT NOT NULL CHECK(scope IN ('global', 'workspace', 'conversation', 'project')),
                scope_owner_ref TEXT,
                category TEXT NOT NULL CHECK(category IN ('preference', 'fact', 'workflow', 'correction', 'boundary')),
                risk_level TEXT NOT NULL CHECK(risk_level IN ('low', 'medium', 'high', 'identity_value')),
                sensitivity TEXT NOT NULL CHECK(sensitivity IN ('internal', 'sensitive')),
                audit_digest TEXT NOT NULL CHECK(TRIM(audit_digest) != ''),
                status TEXT NOT NULL CHECK(status IN ('candidate', 'pending_review', 'edited_pending_review', 'accepted', 'pending_materialization', 'materialized', 'materialization_failed', 'rejected', 'deferred', 'superseded', 'rolled_back')),
                materialization_status TEXT NOT NULL CHECK(materialization_status IN ('not_required', 'pending', 'materialized', 'failed')),
                materialization_error_code TEXT,
                created_by TEXT NOT NULL,
                accepted_by TEXT,
                accepted_at TEXT,
                materialized_view_id TEXT,
                materialized_view_version INTEGER,
                evidence_ids_json TEXT NOT NULL,
                confidence REAL NOT NULL,
                conflict_ids_json TEXT NOT NULL,
                supersedes_memory_id TEXT,
                replacement_memory_id TEXT,
                rolled_back_by_event_id TEXT,
                runtime_context_excluded_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;
        tx.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_lifecycle_proposal ON memory_lifecycle_records(proposal_id)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_lifecycle_status_scope ON memory_lifecycle_records(status, materialization_status, scope)",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS memory_rollback_events (
                rollback_event_id TEXT PRIMARY KEY,
                memory_id TEXT NOT NULL,
                proposal_id TEXT NOT NULL,
                requested_by TEXT NOT NULL,
                reason TEXT NOT NULL,
                previous_status TEXT NOT NULL,
                next_status TEXT NOT NULL,
                affected_materialized_view_ids_json TEXT NOT NULL,
                affected_runtime_surface_ids_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                audit_digest TEXT NOT NULL
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS memory_materialized_views (
                materialized_view_id TEXT PRIMARY KEY,
                scope TEXT,
                version INTEGER NOT NULL,
                active_memory_ids_json TEXT NOT NULL,
                runtime_surface_ids_json TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                content_digest TEXT NOT NULL
            )",
            [],
        )?;
        persistence_outbox::init_schema(&tx)?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "memory_lifecycle_records",
            "fact_key",
            "TEXT",
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "memory_lifecycle_records",
            "sensitivity",
            "TEXT",
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "memory_lifecycle_records",
            "audit_digest",
            "TEXT",
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "memory_lifecycle_records",
            "scope_owner_ref",
            "TEXT",
        )?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory_lifecycle_proposal_links (
                proposal_id TEXT PRIMARY KEY,
                memory_id TEXT NOT NULL,
                admitted_memory_id TEXT NOT NULL,
                fact_key TEXT NOT NULL CHECK(TRIM(fact_key) != ''),
                admission_digest TEXT NOT NULL CHECK(TRIM(admission_digest) != ''),
                risk_level TEXT NOT NULL CHECK(risk_level IN ('low', 'medium', 'high', 'identity_value')),
                sensitivity TEXT NOT NULL CHECK(sensitivity IN ('internal', 'sensitive')),
                linked_at TEXT NOT NULL,
                FOREIGN KEY(memory_id) REFERENCES memory_lifecycle_records(memory_id),
                FOREIGN KEY(admitted_memory_id) REFERENCES memory_lifecycle_records(memory_id)
             );
             CREATE INDEX IF NOT EXISTS idx_memory_lifecycle_proposal_links_memory
             ON memory_lifecycle_proposal_links(memory_id);",
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "memory_lifecycle_proposal_links",
            "admitted_memory_id",
            "TEXT",
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "memory_lifecycle_proposal_links",
            "admission_digest",
            "TEXT",
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "memory_lifecycle_proposal_links",
            "risk_level",
            "TEXT",
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "memory_lifecycle_proposal_links",
            "sensitivity",
            "TEXT",
        )?;
        migrate_memory_fact_identity_tx(&tx)?;
        rebuild_memory_lifecycle_tables_if_needed_tx(&tx)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory_lifecycle_retrieval_states (
                memory_id TEXT PRIMARY KEY,
                disposition TEXT NOT NULL CHECK(disposition IN ('active', 'paused', 'archived')),
                revision INTEGER NOT NULL CHECK(revision > 0),
                last_event_id TEXT NOT NULL,
                reason_digest TEXT NOT NULL CHECK(TRIM(reason_digest) != ''),
                changed_at TEXT NOT NULL,
                FOREIGN KEY(memory_id) REFERENCES memory_lifecycle_records(memory_id),
                FOREIGN KEY(last_event_id) REFERENCES canonical_outbox_events(event_id)
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_memory_lifecycle_retrieval_disposition
             ON memory_lifecycle_retrieval_states(disposition, changed_at DESC);",
        )?;
        rebuild_memory_lifecycle_retrieval_table_if_needed_tx(&tx)?;
        tx.execute_batch(
            "DROP INDEX IF EXISTS idx_memory_lifecycle_active_fact_key;
             CREATE UNIQUE INDEX idx_memory_lifecycle_active_fact_key
             ON memory_lifecycle_records(fact_key)
             WHERE runtime_context_excluded_at IS NULL
               AND status IN (
                    'accepted', 'pending_materialization', 'materialized',
                    'materialization_failed'
               );",
        )?;
        crate::sqlite_migration::record_schema_version(&tx, "memory_lifecycle_store", 7)?;
        tx.commit()?;
        Ok(())
    }

    pub fn accept_memory_proposal(
        &self,
        input: MemoryLifecycleAcceptanceInput,
    ) -> Result<MemoryLifecycleAcceptanceReport> {
        if input.proposal_id.trim().is_empty() {
            anyhow::bail!("memory proposal id is empty");
        }
        let (_, fact_key) = canonical_memory_fact_identity(
            input.fact.scope,
            input.fact.scope_owner_ref.as_deref(),
            input.fact.category,
            &input.fact.canonical_body,
        )?;
        let admission_digest = memory_fact_admission_digest(&fact_key, &input.fact);
        let confidence = input
            .confidence
            .parse::<f32>()
            .context("memory proposal confidence is not numeric")?;
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            anyhow::bail!("memory proposal confidence must be finite and within 0..=1");
        }
        let now = Utc::now();
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        if let Some(link) = proposal_link_tx(&tx, &input.proposal_id)? {
            ensure_admission_link_matches(&link, &fact_key, &admission_digest, &input.fact)?;
            let admitted_record = record_by_memory_id_tx(&tx, &link.admitted_memory_id)?
                .context("memory admission link points to a missing admitted owner")?;
            if admitted_record.status.is_terminal_historical() {
                let report = terminal_historical_acceptance_report(
                    admitted_record,
                    fact_key,
                    link.linked_at,
                );
                tx.commit()?;
                return Ok(report);
            }
            let record = record_by_memory_id_tx(&tx, &link.memory_id)?
                .context("memory proposal link points to a missing canonical owner")?;
            if !record.status.is_runtime_active() {
                anyhow::bail!("memory proposal owner is not materialized");
            }
            let (record, upgraded) = merge_effective_governance_tx(
                &tx,
                record,
                &fact_key,
                input.fact.risk_level,
                input.fact.sensitivity,
            )?;
            let report = existing_acceptance_report_tx(
                &tx,
                record,
                fact_key,
                if upgraded {
                    MemoryAdmissionOutcome::GovernanceUpgraded
                } else {
                    MemoryAdmissionOutcome::ExactReplay
                },
                link.linked_at,
            )?;
            tx.commit()?;
            return Ok(report);
        }

        if let Some(supersedes_memory_id) = input.supersedes_memory_id.as_deref() {
            if supersedes_memory_id.trim() != supersedes_memory_id
                || !supersedes_memory_id.starts_with("memory:")
                || supersedes_memory_id.len() > 256
            {
                anyhow::bail!("reviewed Memory correction target id is invalid");
            }
            let mut previous = record_by_memory_id_tx(&tx, supersedes_memory_id)?
                .context("reviewed Memory correction target is missing")?;
            if !previous.status.is_runtime_active()
                || previous.materialization_status != MemoryMaterializationStatus::Materialized
                || previous.runtime_context_excluded_at.is_some()
            {
                anyhow::bail!("reviewed Memory correction target is stale or inactive");
            }
            if previous.scope != input.fact.scope {
                anyhow::bail!("reviewed Memory correction cannot change scope implicitly");
            }
            if previous.scope_owner_ref != input.fact.scope_owner_ref {
                anyhow::bail!("reviewed Memory correction cannot change scope owner implicitly");
            }
            if previous.content == input.fact.canonical_body {
                anyhow::bail!("reviewed Memory correction must change the canonical content");
            }
            if let Some(conflict) = active_record_by_fact_key_tx(&tx, &fact_key)? {
                anyhow::bail!(
                    "reviewed Memory correction collides with active owner {}",
                    conflict.memory_id
                );
            }

            let memory_id = format!("memory:{}", Uuid::new_v4());
            let mut replacement = MemoryLifecycleRecord {
                memory_id: memory_id.clone(),
                proposal_id: input.proposal_id.clone(),
                source_task_session_id: input.source_task_session_id,
                source_run_id: input.source_run_id,
                content: input.fact.canonical_body,
                scope: input.fact.scope,
                scope_owner_ref: input.fact.scope_owner_ref,
                category: input.fact.category,
                risk_level: input.fact.risk_level,
                sensitivity: input.fact.sensitivity,
                audit_digest: String::new(),
                status: MemoryLifecycleStatus::Accepted,
                materialization_status: MemoryMaterializationStatus::Pending,
                materialization_error_code: None,
                created_by: input.created_by,
                accepted_by: Some(input.accepted_by),
                accepted_at: Some(now),
                materialized_view_id: None,
                materialized_view_version: None,
                evidence_ids: input.evidence_ids,
                confidence,
                conflict_ids: input.conflict_ids,
                supersedes_memory_id: Some(previous.memory_id.clone()),
                replacement_memory_id: None,
                rolled_back_by_event_id: None,
                runtime_context_excluded_at: None,
            };
            replacement.audit_digest = memory_record_audit_digest(&replacement, &fact_key);
            insert_record_tx(&tx, &replacement, &fact_key, now, now)?;
            link_proposal_to_memory_tx(
                &tx,
                &input.proposal_id,
                &replacement.memory_id,
                &fact_key,
                &admission_digest,
                input.fact.risk_level,
                input.fact.sensitivity,
                now,
            )?;

            previous.status = MemoryLifecycleStatus::Superseded;
            previous.materialization_status = MemoryMaterializationStatus::NotRequired;
            previous.replacement_memory_id = Some(replacement.memory_id.clone());
            previous.runtime_context_excluded_at = Some(now);
            let (_, previous_fact_key) = canonical_memory_fact_identity(
                previous.scope,
                previous.scope_owner_ref.as_deref(),
                previous.category,
                &previous.content,
            )?;
            previous.audit_digest = memory_record_audit_digest(&previous, &previous_fact_key);
            update_record_tx(&tx, &previous, now)?;

            let view = rebuild_view_tx(
                &tx,
                Some(replacement.scope),
                Some(&replacement.memory_id),
                Some(&previous.memory_id),
            )?;
            replacement.status = MemoryLifecycleStatus::Materialized;
            replacement.materialization_status = MemoryMaterializationStatus::Materialized;
            replacement.materialized_view_id = Some(view.materialized_view_id.clone());
            replacement.materialized_view_version = Some(view.version);
            update_record_tx(&tx, &replacement, Utc::now())?;
            let superseded_canonical_mutation = persistence_outbox::enqueue_tombstone(
                &tx,
                MEMORY_LIFECYCLE_AGGREGATE_KIND,
                &previous.memory_id,
                Some("reviewed_memory_correction_superseded"),
                &MEMORY_LIFECYCLE_PROJECTION_TARGETS,
            )?;
            let canonical_mutation = persistence_outbox::enqueue_mutation(
                &tx,
                MEMORY_LIFECYCLE_AGGREGATE_KIND,
                &replacement.memory_id,
                "materialized",
                &memory_lifecycle_projection_token(&replacement),
                &MEMORY_LIFECYCLE_PROJECTION_TARGETS,
            )?;
            tx.commit()?;
            return Ok(MemoryLifecycleAcceptanceReport {
                record: replacement,
                materialized_view: Some(view),
                preceding_canonical_mutations: vec![superseded_canonical_mutation],
                canonical_mutation: Some(canonical_mutation),
                canonical_committed: true,
                canonical_fact_key: fact_key,
                newly_committed: true,
                admission_outcome: MemoryAdmissionOutcome::OwnerCreated,
                admission_at: now,
                owner_accepted_at: Some(now),
                projection_state: ProjectionDeliveryState::Pending,
            });
        }

        if let Some(record) = active_record_by_fact_key_tx(&tx, &fact_key)? {
            if !record.status.is_runtime_active() {
                anyhow::bail!("existing Memory fact owner is not materialized");
            }
            link_proposal_to_memory_tx(
                &tx,
                &input.proposal_id,
                &record.memory_id,
                &fact_key,
                &admission_digest,
                input.fact.risk_level,
                input.fact.sensitivity,
                now,
            )?;
            let (record, upgraded) = merge_effective_governance_tx(
                &tx,
                record,
                &fact_key,
                input.fact.risk_level,
                input.fact.sensitivity,
            )?;
            let report = existing_acceptance_report_tx(
                &tx,
                record,
                fact_key,
                if upgraded {
                    MemoryAdmissionOutcome::GovernanceUpgraded
                } else {
                    MemoryAdmissionOutcome::AliasLinked
                },
                now,
            )?;
            tx.commit()?;
            return Ok(report);
        }

        let memory_id = format!("memory:{}", Uuid::new_v4());
        let mut record = MemoryLifecycleRecord {
            memory_id,
            proposal_id: input.proposal_id.clone(),
            source_task_session_id: input.source_task_session_id,
            source_run_id: input.source_run_id,
            content: input.fact.canonical_body,
            scope: input.fact.scope,
            scope_owner_ref: input.fact.scope_owner_ref,
            category: input.fact.category,
            risk_level: input.fact.risk_level,
            sensitivity: input.fact.sensitivity,
            audit_digest: String::new(),
            status: MemoryLifecycleStatus::Accepted,
            materialization_status: MemoryMaterializationStatus::Pending,
            materialization_error_code: None,
            created_by: input.created_by,
            accepted_by: Some(input.accepted_by),
            accepted_at: Some(now),
            materialized_view_id: None,
            materialized_view_version: None,
            evidence_ids: input.evidence_ids,
            confidence,
            conflict_ids: input.conflict_ids,
            supersedes_memory_id: None,
            replacement_memory_id: None,
            rolled_back_by_event_id: None,
            runtime_context_excluded_at: None,
        };
        record.audit_digest = memory_record_audit_digest(&record, &fact_key);
        insert_record_tx(&tx, &record, &fact_key, now, now)?;
        link_proposal_to_memory_tx(
            &tx,
            &input.proposal_id,
            &record.memory_id,
            &fact_key,
            &admission_digest,
            input.fact.risk_level,
            input.fact.sensitivity,
            now,
        )?;
        let view = rebuild_view_tx(&tx, Some(record.scope), Some(&record.memory_id), None)?;
        record.status = MemoryLifecycleStatus::Materialized;
        record.materialization_status = MemoryMaterializationStatus::Materialized;
        record.materialized_view_id = Some(view.materialized_view_id.clone());
        record.materialized_view_version = Some(view.version);
        update_record_tx(&tx, &record, Utc::now())?;
        let canonical_mutation = persistence_outbox::enqueue_mutation(
            &tx,
            MEMORY_LIFECYCLE_AGGREGATE_KIND,
            &record.memory_id,
            "materialized",
            &memory_lifecycle_projection_token(&record),
            &MEMORY_LIFECYCLE_PROJECTION_TARGETS,
        )?;
        tx.commit()?;
        Ok(MemoryLifecycleAcceptanceReport {
            record,
            materialized_view: Some(view),
            preceding_canonical_mutations: Vec::new(),
            canonical_mutation: Some(canonical_mutation),
            canonical_committed: true,
            canonical_fact_key: fact_key,
            newly_committed: true,
            admission_outcome: MemoryAdmissionOutcome::OwnerCreated,
            admission_at: now,
            owner_accepted_at: Some(now),
            projection_state: ProjectionDeliveryState::Pending,
        })
    }

    pub fn commit_explicit_user_memory(
        &self,
        input: ExplicitMemoryWriteInput,
        admission_proof: PolicyMemoryAdmissionProof,
    ) -> Result<ExplicitMemoryWriteReceipt> {
        admission_proof.consume_for_explicit_input(&input)?;
        if !matches!(
            input.fact.risk_level,
            MemoryLifecycleRiskLevel::Low | MemoryLifecycleRiskLevel::Medium
        ) {
            anyhow::bail!("explicit Memory lane rejects high-risk or identity/value content");
        }
        if input.fact.sensitivity == MemoryLifecycleSensitivity::Sensitive {
            anyhow::bail!("explicit Memory lane rejects sensitive content");
        }
        if input.fact.category == MemoryLifecycleCategory::Boundary {
            anyhow::bail!("explicit Memory lane rejects identity or boundary content");
        }
        if input.source_message_id.trim().is_empty() {
            anyhow::bail!("explicit Memory source message id is empty");
        }
        let (_, fact_key) = canonical_memory_fact_identity(
            input.fact.scope,
            input.fact.scope_owner_ref.as_deref(),
            input.fact.category,
            &input.fact.canonical_body,
        )?;
        let content_digest = digest_label(&input.fact.canonical_body);
        let admission_digest = memory_fact_admission_digest(&fact_key, &input.fact);
        let explicit_admission_id = digest_length_delimited_values(
            "openlife_explicit_memory_admission_id_v1",
            &[&input.source_message_id, &fact_key],
        );
        let proposal_id = format!("explicit_memory:{explicit_admission_id}");
        let now = Utc::now();
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(link) = proposal_link_tx(&tx, &proposal_id)? {
            ensure_admission_link_matches(&link, &fact_key, &admission_digest, &input.fact)?;
            let admitted_record = record_by_memory_id_tx(&tx, &link.admitted_memory_id)?
                .context("explicit Memory admission link lost its admitted owner")?;
            if admitted_record.status.is_terminal_historical() {
                tx.commit()?;
                return Ok(ExplicitMemoryWriteReceipt {
                    receipt_id: admitted_record.memory_id.clone(),
                    memory_id: admitted_record.memory_id,
                    fact_key,
                    source_message_id: input.source_message_id,
                    content_digest,
                    sensitivity: admitted_record.sensitivity,
                    audit_digest: admitted_record.audit_digest,
                    admission_outcome: MemoryAdmissionOutcome::TerminalHistorical,
                    admission_at: link.linked_at,
                    owner_accepted_at: admitted_record.accepted_at,
                    created_at: link.linked_at,
                    newly_committed: false,
                    undo_available: false,
                    canonical_committed: false,
                    outbox_event_id: None,
                    projection_state: terminal_projection_state(admitted_record.status),
                    projection_error_digest: None,
                });
            }
            let record = record_by_memory_id_tx(&tx, &link.memory_id)?
                .context("explicit Memory admission link lost its canonical owner")?;
            if !record.status.is_runtime_active() {
                anyhow::bail!("explicit Memory canonical owner is not materialized");
            }
            let (record, upgraded) = merge_effective_governance_tx(
                &tx,
                record,
                &fact_key,
                input.fact.risk_level,
                input.fact.sensitivity,
            )?;
            let canonical_mutation = ensure_materialized_mutation_tx(&tx, &record)?;
            let projection_state =
                persistence_outbox::projection_summary(&tx, &canonical_mutation.event_id)?.state();
            tx.commit()?;
            return Ok(ExplicitMemoryWriteReceipt {
                receipt_id: record.memory_id.clone(),
                memory_id: record.memory_id,
                fact_key,
                source_message_id: input.source_message_id,
                content_digest,
                sensitivity: record.sensitivity,
                audit_digest: record.audit_digest,
                admission_outcome: if upgraded {
                    MemoryAdmissionOutcome::GovernanceUpgraded
                } else {
                    MemoryAdmissionOutcome::ExactReplay
                },
                admission_at: link.linked_at,
                owner_accepted_at: record.accepted_at,
                created_at: link.linked_at,
                newly_committed: upgraded,
                undo_available: true,
                canonical_committed: true,
                outbox_event_id: Some(canonical_mutation.event_id),
                projection_state,
                projection_error_digest: None,
            });
        }
        let existing = active_record_by_fact_key_tx(&tx, &fact_key)?;
        if let Some(record) = existing {
            if !record.status.is_runtime_active() {
                anyhow::bail!("existing explicit Memory owner is not materialized");
            }
            link_proposal_to_memory_tx(
                &tx,
                &proposal_id,
                &record.memory_id,
                &fact_key,
                &admission_digest,
                input.fact.risk_level,
                input.fact.sensitivity,
                now,
            )?;
            let (record, upgraded) = merge_effective_governance_tx(
                &tx,
                record,
                &fact_key,
                input.fact.risk_level,
                input.fact.sensitivity,
            )?;
            let canonical_mutation = ensure_materialized_mutation_tx(&tx, &record)?;
            let projection_state =
                persistence_outbox::projection_summary(&tx, &canonical_mutation.event_id)?.state();
            tx.commit()?;
            return Ok(ExplicitMemoryWriteReceipt {
                receipt_id: record.memory_id.clone(),
                memory_id: record.memory_id,
                fact_key,
                source_message_id: input.source_message_id,
                content_digest,
                sensitivity: record.sensitivity,
                audit_digest: record.audit_digest,
                admission_outcome: if upgraded {
                    MemoryAdmissionOutcome::GovernanceUpgraded
                } else {
                    MemoryAdmissionOutcome::AliasLinked
                },
                admission_at: now,
                owner_accepted_at: record.accepted_at,
                created_at: now,
                newly_committed: upgraded,
                undo_available: true,
                canonical_committed: true,
                outbox_event_id: Some(canonical_mutation.event_id),
                projection_state,
                projection_error_digest: None,
            });
        }

        let memory_id = format!("memory:{}", Uuid::new_v4());
        let mut record = MemoryLifecycleRecord {
            memory_id: memory_id.clone(),
            proposal_id: proposal_id.clone(),
            source_task_session_id: Some(input.source_task_session_id),
            source_run_id: Some(input.source_run_id),
            content: input.fact.canonical_body,
            scope: input.fact.scope,
            scope_owner_ref: input.fact.scope_owner_ref,
            category: input.fact.category,
            risk_level: input.fact.risk_level,
            sensitivity: input.fact.sensitivity,
            audit_digest: String::new(),
            status: MemoryLifecycleStatus::Accepted,
            materialization_status: MemoryMaterializationStatus::Pending,
            materialization_error_code: None,
            created_by: "current_authenticated_user_message".into(),
            accepted_by: Some("user_explicit_instruction".into()),
            accepted_at: Some(now),
            materialized_view_id: None,
            materialized_view_version: None,
            evidence_ids: vec![input.source_message_id.clone()],
            confidence: 1.0,
            conflict_ids: Vec::new(),
            supersedes_memory_id: None,
            replacement_memory_id: None,
            rolled_back_by_event_id: None,
            runtime_context_excluded_at: None,
        };
        record.audit_digest = memory_record_audit_digest(&record, &fact_key);
        insert_record_tx(&tx, &record, &fact_key, now, now)?;
        link_proposal_to_memory_tx(
            &tx,
            &proposal_id,
            &record.memory_id,
            &fact_key,
            &admission_digest,
            input.fact.risk_level,
            input.fact.sensitivity,
            now,
        )?;
        let view = rebuild_view_tx(&tx, Some(record.scope), Some(&record.memory_id), None)?;
        record.status = MemoryLifecycleStatus::Materialized;
        record.materialization_status = MemoryMaterializationStatus::Materialized;
        record.materialized_view_id = Some(view.materialized_view_id);
        record.materialized_view_version = Some(view.version);
        update_record_tx(&tx, &record, Utc::now())?;
        let canonical_mutation = persistence_outbox::enqueue_mutation(
            &tx,
            MEMORY_LIFECYCLE_AGGREGATE_KIND,
            &record.memory_id,
            "materialized",
            &memory_lifecycle_projection_token(&record),
            &MEMORY_LIFECYCLE_PROJECTION_TARGETS,
        )?;
        tx.commit()?;

        Ok(ExplicitMemoryWriteReceipt {
            receipt_id: memory_id.clone(),
            memory_id,
            fact_key,
            source_message_id: input.source_message_id,
            content_digest,
            sensitivity: record.sensitivity,
            audit_digest: record.audit_digest,
            admission_outcome: MemoryAdmissionOutcome::OwnerCreated,
            admission_at: now,
            owner_accepted_at: record.accepted_at,
            created_at: now,
            newly_committed: true,
            undo_available: true,
            canonical_committed: true,
            outbox_event_id: Some(canonical_mutation.event_id),
            projection_state: ProjectionDeliveryState::Pending,
            projection_error_digest: None,
        })
    }

    #[cfg(test)]
    fn commit_test_explicit_user_memory(
        &self,
        input: ExplicitMemoryWriteInput,
    ) -> Result<ExplicitMemoryWriteReceipt> {
        let admission_proof = PolicyMemoryAdmissionProof::test_fixture_for_explicit_input(&input);
        self.commit_explicit_user_memory(input, admission_proof)
    }

    pub fn get_record(&self, memory_id: &str) -> Result<Option<MemoryLifecycleRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            record_select_sql!("WHERE memory_id = ?1"),
            [memory_id],
            row_to_record,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_record_by_proposal_id(
        &self,
        proposal_id: &str,
    ) -> Result<Option<MemoryLifecycleRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            record_select_sql!(
                "WHERE memory_id = (
                    SELECT admitted_memory_id FROM memory_lifecycle_proposal_links
                    WHERE proposal_id = ?1
                 )"
            ),
            [proposal_id],
            row_to_record,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn rollback_memory_asset(
        &self,
        memory_id: &str,
        requested_by: &str,
        reason: &str,
    ) -> Result<MemoryRollbackReport> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let mut record = tx
            .query_row(
                record_select_sql!("WHERE memory_id = ?1"),
                [memory_id],
                row_to_record,
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("accepted memory id not found: {}", memory_id))?;
        if matches!(
            record.status,
            MemoryLifecycleStatus::RolledBack | MemoryLifecycleStatus::Superseded
        ) {
            return Err(anyhow::anyhow!(
                "memory asset is already terminal: {}",
                record.status
            ));
        }
        if !matches!(
            record.status,
            MemoryLifecycleStatus::Accepted
                | MemoryLifecycleStatus::PendingMaterialization
                | MemoryLifecycleStatus::Materialized
                | MemoryLifecycleStatus::MaterializationFailed
        ) {
            return Err(anyhow::anyhow!(
                "memory asset is not an accepted memory lifecycle record: {}",
                record.status
            ));
        }
        if matches!(
            record.risk_level,
            MemoryLifecycleRiskLevel::High | MemoryLifecycleRiskLevel::IdentityValue
        ) {
            return Err(anyhow::anyhow!(
                "memory rollback requires explicit confirmation for high-risk or identity/value memory"
            ));
        }

        let previous_status = record.status;
        let now = Utc::now();
        record.status = MemoryLifecycleStatus::RolledBack;
        record.materialization_status = MemoryMaterializationStatus::NotRequired;
        record.runtime_context_excluded_at = Some(now);
        update_record_tx(&tx, &record, now)?;
        let view = rebuild_view_tx(&tx, Some(record.scope), None, Some(&record.memory_id))?;
        let rollback_event_id = format!("memory_rollback:{}", Uuid::new_v4());
        let affected_materialized_view_ids = vec![view.materialized_view_id.clone()];
        let affected_runtime_surface_ids = view.runtime_surface_ids.clone();
        let audit_digest = digest_label(&format!(
            "{}:{}:{}:{}:{}",
            rollback_event_id, record.memory_id, record.proposal_id, previous_status, view.version
        ));
        let rollback_event = MemoryRollbackEvent {
            rollback_event_id: rollback_event_id.clone(),
            memory_id: record.memory_id.clone(),
            proposal_id: record.proposal_id.clone(),
            requested_by: requested_by.to_string(),
            reason: reason.to_string(),
            previous_status,
            next_status: MemoryLifecycleStatus::RolledBack,
            affected_materialized_view_ids,
            affected_runtime_surface_ids,
            created_at: now,
            audit_digest,
        };
        insert_rollback_event_tx(&tx, &rollback_event)?;
        record.rolled_back_by_event_id = Some(rollback_event_id);
        record.materialized_view_id = Some(view.materialized_view_id.clone());
        record.materialized_view_version = Some(view.version);
        update_record_tx(&tx, &record, now)?;
        let canonical_mutation = persistence_outbox::enqueue_tombstone(
            &tx,
            MEMORY_LIFECYCLE_AGGREGATE_KIND,
            &record.memory_id,
            Some(reason),
            &MEMORY_LIFECYCLE_PROJECTION_TARGETS,
        )?;
        tx.commit()?;
        Ok(MemoryRollbackReport {
            record,
            rollback_event,
            materialized_view: view,
            canonical_mutation,
            canonical_committed: true,
            projection_state: ProjectionDeliveryState::Pending,
            projection_error_digest: None,
        })
    }

    /// Irreversibly remove the canonical Memory body and all content-bearing
    /// lifecycle metadata. The row remains only as a body-free tombstone so
    /// outbox projections can delete derived MemoryStore/vector content and a
    /// later replay cannot silently resurrect the erased owner.
    pub fn privacy_erase_memory_asset(&self, memory_id: &str) -> Result<MemoryPrivacyEraseReport> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut record = tx
            .query_row(
                record_select_sql!("WHERE memory_id = ?1"),
                [memory_id],
                row_to_record,
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("Memory privacy erase target not found"))?;
        if record.content.is_empty() {
            anyhow::bail!("Memory privacy erase target is already erased");
        }
        let erased_at = Utc::now();
        let scope = record.scope;
        record.content.clear();
        record.status = MemoryLifecycleStatus::RolledBack;
        record.materialization_status = MemoryMaterializationStatus::NotRequired;
        record.materialization_error_code = None;
        record.source_task_session_id = None;
        record.source_run_id = None;
        record.created_by = "privacy_erasure_tombstone".into();
        record.accepted_by = None;
        record.accepted_at = None;
        record.materialized_view_id = None;
        record.materialized_view_version = None;
        record.evidence_ids.clear();
        record.conflict_ids.clear();
        record.supersedes_memory_id = None;
        record.replacement_memory_id = None;
        record.rolled_back_by_event_id = None;
        record.runtime_context_excluded_at = Some(erased_at);
        record.audit_digest =
            memory_record_audit_digest(&record, &format!("privacy_erased:{}", record.memory_id));
        tx.execute(
            "DELETE FROM memory_lifecycle_retrieval_states WHERE memory_id = ?1",
            [memory_id],
        )?;
        tx.execute(
            "DELETE FROM memory_rollback_events WHERE memory_id = ?1",
            [memory_id],
        )?;
        tx.execute(
            "DELETE FROM memory_lifecycle_proposal_links
             WHERE memory_id = ?1 OR admitted_memory_id = ?1",
            [memory_id],
        )?;
        update_record_tx(&tx, &record, erased_at)?;
        let materialized_view = rebuild_view_tx(&tx, Some(scope), None, Some(memory_id))?;
        let canonical_mutation = persistence_outbox::enqueue_tombstone(
            &tx,
            MEMORY_LIFECYCLE_AGGREGATE_KIND,
            memory_id,
            Some("user_confirmed_privacy_erase"),
            &MEMORY_LIFECYCLE_PROJECTION_TARGETS,
        )?;
        tx.commit()?;
        Ok(MemoryPrivacyEraseReport {
            memory_id: memory_id.to_string(),
            erased_at,
            materialized_view,
            canonical_mutation,
            canonical_committed: true,
            projection_state: ProjectionDeliveryState::Pending,
            projection_error_digest: None,
        })
    }

    pub fn list_replayable_projection_deliveries(
        &self,
        limit: usize,
    ) -> Result<Vec<ProjectionDelivery>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::list_replayable_deliveries(&conn, limit)
    }

    pub fn list_replayable_projection_deliveries_for_event(
        &self,
        event_id: &str,
    ) -> Result<Vec<ProjectionDelivery>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::list_replayable_deliveries_for_event(&conn, event_id)
    }

    pub fn mark_projection_applied(&self, event_id: &str, projection_target: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_applied(&conn, event_id, projection_target)
    }

    pub fn mark_projection_degraded(
        &self,
        event_id: &str,
        projection_target: &str,
        error: &str,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_degraded(&conn, event_id, projection_target, error)
    }

    pub fn projection_summary(&self, event_id: &str) -> Result<ProjectionSummary> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::projection_summary(&conn, event_id)
    }

    pub fn latest_projection_event_id(&self, memory_id: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT event_id FROM canonical_outbox_events
             WHERE aggregate_kind = ?1 AND aggregate_id = ?2
             ORDER BY created_at DESC, event_id DESC LIMIT 1",
            params![MEMORY_LIFECYCLE_AGGREGATE_KIND, memory_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn install_outbox_insert_failure_for_test(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute_batch(
            "CREATE TRIGGER fail_memory_lifecycle_outbox_insert_for_test
             BEFORE INSERT ON canonical_outbox_events
             BEGIN
                 SELECT RAISE(ABORT, 'injected memory lifecycle outbox insert failure');
             END;",
        )?;
        Ok(())
    }

    pub fn list_records(
        &self,
        scope: Option<MemoryLifecycleScope>,
        status: Option<MemoryLifecycleStatus>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<MemoryLifecycleRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let limit = limit.clamp(1, 200);
        match (scope, status) {
            (Some(scope), Some(status)) => {
                let mut stmt = conn.prepare(record_select_sql!(
                    "WHERE scope = ?1 AND status = ?2 ORDER BY accepted_at DESC, memory_id DESC LIMIT ?3 OFFSET ?4"
                ))?;
                let rows = stmt.query_map(
                    params![scope.to_string(), status.to_string(), limit, offset],
                    row_to_record,
                )?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(Into::into)
            }
            (Some(scope), None) => {
                let mut stmt = conn.prepare(record_select_sql!(
                    "WHERE scope = ?1 ORDER BY accepted_at DESC, memory_id DESC LIMIT ?2 OFFSET ?3"
                ))?;
                let rows =
                    stmt.query_map(params![scope.to_string(), limit, offset], row_to_record)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(Into::into)
            }
            (None, Some(status)) => {
                let mut stmt = conn.prepare(record_select_sql!(
                    "WHERE status = ?1 ORDER BY accepted_at DESC, memory_id DESC LIMIT ?2 OFFSET ?3"
                ))?;
                let rows =
                    stmt.query_map(params![status.to_string(), limit, offset], row_to_record)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(Into::into)
            }
            (None, None) => {
                let mut stmt = conn.prepare(record_select_sql!(
                    "ORDER BY accepted_at DESC, memory_id DESC LIMIT ?1 OFFSET ?2"
                ))?;
                let rows = stmt.query_map(params![limit, offset], row_to_record)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(Into::into)
            }
        }
    }

    pub fn list_active_records(
        &self,
        scope: Option<MemoryLifecycleScope>,
        limit: i64,
    ) -> Result<Vec<MemoryLifecycleRecord>> {
        self.list_retrievable_records(scope, limit)
    }

    /// Runtime-safe personal Memory context. The lifecycle row and retrieval
    /// disposition live in this one canonical database, so archive/restore,
    /// rollback and context selection are serialized by the same SQLite
    /// transaction authority. A lagging MemoryStore/VectorStore projection can
    /// therefore never make an archived lifecycle body eligible for context.
    pub fn list_retrievable_records(
        &self,
        scope: Option<MemoryLifecycleScope>,
        limit: i64,
    ) -> Result<Vec<MemoryLifecycleRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let limit = limit.clamp(1, 200);
        let predicate = "status = 'materialized'
             AND materialization_status = 'materialized'
             AND runtime_context_excluded_at IS NULL
             AND NOT EXISTS (
                 SELECT 1 FROM memory_lifecycle_retrieval_states retrieval
                 WHERE retrieval.memory_id = memory_lifecycle_records.memory_id
                   AND retrieval.disposition != 'active'
             )";
        if let Some(scope) = scope {
            let sql = format!(
                "{} WHERE scope = ?1 AND {predicate}
                 ORDER BY accepted_at DESC, memory_id DESC LIMIT ?2",
                record_select_sql!("")
            );
            let mut statement = conn.prepare(&sql)?;
            let rows = statement.query_map(params![scope.as_str(), limit], row_to_record)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Into::into)
        } else {
            let sql = format!(
                "{} WHERE {predicate}
                 ORDER BY accepted_at DESC, memory_id DESC LIMIT ?1",
                record_select_sql!("")
            );
            let mut statement = conn.prepare(&sql)?;
            let rows = statement.query_map([limit], row_to_record)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Into::into)
        }
    }

    /// Canonical asset liveness independent of retrieval disposition. This is
    /// used to authorize both archive and restore. Runtime context must use
    /// `is_memory_retrievable` instead.
    pub fn is_memory_active(&self, memory_id: &str) -> Result<bool> {
        Ok(self.get_record(memory_id)?.is_some_and(|record| {
            record.status.is_runtime_active()
                && record.materialization_status == MemoryMaterializationStatus::Materialized
                && record.runtime_context_excluded_at.is_none()
        }))
    }

    pub fn is_memory_retrievable(&self, memory_id: &str) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        lifecycle_memory_is_retrievable_from_conn(&conn, memory_id)
    }

    pub fn set_memory_retrieval_disposition(
        &self,
        memory_id: &str,
        disposition: MemoryRetrievalDisposition,
        reason_code: &str,
    ) -> Result<CanonicalMemoryRetrievalMutation> {
        self.set_memory_retrieval_dispositions(
            std::slice::from_ref(&memory_id.to_string()),
            disposition,
            reason_code,
        )?
        .into_iter()
        .next()
        .context("canonical lifecycle Memory retrieval single mutation is missing")
    }

    /// Mutate retrieval visibility under the same canonical transaction that
    /// owns lifecycle liveness. Every owner is validated before the first
    /// outbox event, so a rollback/delete race has one serial outcome and a
    /// mixed valid/stale batch cannot partially commit.
    pub fn set_memory_retrieval_dispositions(
        &self,
        memory_ids: &[String],
        disposition: MemoryRetrievalDisposition,
        reason_code: &str,
    ) -> Result<Vec<CanonicalMemoryRetrievalMutation>> {
        ensure_lifecycle_retrieval_reason_code(reason_code)?;
        if memory_ids.is_empty() || memory_ids.len() > 200 {
            anyhow::bail!("canonical lifecycle Memory retrieval batch must contain 1..=200 owners");
        }
        let unique = memory_ids.iter().collect::<std::collections::HashSet<_>>();
        if unique.len() != memory_ids.len() {
            anyhow::bail!("canonical lifecycle Memory retrieval batch contains duplicate owners");
        }
        if memory_ids.iter().any(|memory_id| {
            memory_id.trim() != memory_id || memory_id.is_empty() || memory_id.len() > 256
        }) {
            anyhow::bail!("canonical lifecycle Memory owner id is invalid");
        }

        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for memory_id in memory_ids {
            let active = tx
                .query_row(
                    "SELECT 1 FROM memory_lifecycle_records
                     WHERE memory_id = ?1
                       AND status = 'materialized'
                       AND materialization_status = 'materialized'
                       AND runtime_context_excluded_at IS NULL",
                    [memory_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !active {
                anyhow::bail!(
                    "canonical lifecycle Memory owner proof is missing, stale, or inactive"
                );
            }
        }
        let mutations = memory_ids
            .iter()
            .map(|memory_id| {
                Self::set_memory_retrieval_disposition_in_transaction(
                    &tx,
                    memory_id,
                    disposition,
                    reason_code,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(mutations)
    }

    fn set_memory_retrieval_disposition_in_transaction(
        tx: &rusqlite::Transaction<'_>,
        memory_id: &str,
        disposition: MemoryRetrievalDisposition,
        reason_code: &str,
    ) -> Result<CanonicalMemoryRetrievalMutation> {
        let existing = tx
            .query_row(
                "SELECT disposition, revision, last_event_id, changed_at
                 FROM memory_lifecycle_retrieval_states WHERE memory_id = ?1",
                [memory_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        if let Some((stored, revision, last_event_id, changed_at)) = existing.as_ref() {
            let stored_disposition = MemoryRetrievalDisposition::parse(stored)?;
            if stored_disposition == disposition {
                let revision = u64::try_from(*revision)
                    .context("canonical lifecycle Memory retrieval revision is invalid")?;
                let canonical_mutation =
                    persistence_outbox::mutation_by_event_id(tx, last_event_id)?.context(
                        "canonical lifecycle Memory retrieval state lost its outbox event",
                    )?;
                return Ok(CanonicalMemoryRetrievalMutation {
                    changed: false,
                    state: Some(CanonicalMemoryRetrievalState {
                        owner_kind: MEMORY_LIFECYCLE_RETRIEVAL_OWNER_KIND.into(),
                        owner_id: memory_id.to_string(),
                        disposition,
                        revision,
                        last_event_id: last_event_id.clone(),
                        changed_at: changed_at.clone(),
                    }),
                    canonical_mutation: Some(canonical_mutation),
                });
            }
        } else if disposition == MemoryRetrievalDisposition::Active {
            return Ok(CanonicalMemoryRetrievalMutation {
                changed: false,
                state: None,
                canonical_mutation: None,
            });
        }

        let payload_digest = persistence_outbox::metadata_digest(&format!(
            "memory_lifecycle_retrieval:{}:{}:{}",
            memory_id,
            disposition.as_str(),
            Uuid::new_v4()
        ));
        let canonical_mutation = persistence_outbox::enqueue_mutation(
            tx,
            MEMORY_LIFECYCLE_RETRIEVAL_AGGREGATE_KIND,
            &lifecycle_retrieval_aggregate_id(memory_id),
            disposition.as_str(),
            &payload_digest,
            &["vector_store"],
        )?;
        let revision = i64::try_from(canonical_mutation.aggregate_revision)
            .context("canonical lifecycle Memory retrieval revision exceeds SQLite range")?;
        let changed_at = canonical_mutation.created_at.to_rfc3339();
        tx.execute(
            "INSERT INTO memory_lifecycle_retrieval_states (
                memory_id, disposition, revision, last_event_id, reason_digest, changed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(memory_id) DO UPDATE SET
                disposition = excluded.disposition,
                revision = excluded.revision,
                last_event_id = excluded.last_event_id,
                reason_digest = excluded.reason_digest,
                changed_at = excluded.changed_at",
            params![
                memory_id,
                disposition.as_str(),
                revision,
                canonical_mutation.event_id,
                persistence_outbox::metadata_digest(reason_code),
                changed_at,
            ],
        )?;
        Ok(CanonicalMemoryRetrievalMutation {
            changed: true,
            state: Some(CanonicalMemoryRetrievalState {
                owner_kind: MEMORY_LIFECYCLE_RETRIEVAL_OWNER_KIND.into(),
                owner_id: memory_id.to_string(),
                disposition,
                revision: canonical_mutation.aggregate_revision,
                last_event_id: canonical_mutation.event_id.clone(),
                changed_at,
            }),
            canonical_mutation: Some(canonical_mutation),
        })
    }

    pub fn memory_retrieval_state(
        &self,
        memory_id: &str,
    ) -> Result<Option<CanonicalMemoryRetrievalState>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        lifecycle_retrieval_state_from_conn(&conn, memory_id)
    }

    pub fn list_archived_memory_retrieval_states(
        &self,
        limit: usize,
    ) -> Result<Vec<CanonicalMemoryRetrievalState>> {
        self.list_archived_memory_retrieval_states_page(limit, 0)
    }

    pub fn list_archived_memory_retrieval_states_page(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<CanonicalMemoryRetrievalState>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT retrieval.memory_id, retrieval.disposition, retrieval.revision,
                    retrieval.last_event_id, retrieval.changed_at
             FROM memory_lifecycle_retrieval_states retrieval
             JOIN memory_lifecycle_records records
               ON records.memory_id = retrieval.memory_id
             WHERE retrieval.disposition = 'archived'
               AND records.status = 'materialized'
               AND records.materialization_status = 'materialized'
               AND records.runtime_context_excluded_at IS NULL
             ORDER BY retrieval.changed_at DESC, retrieval.memory_id
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = statement.query_map(
            [
                i64::try_from(limit).context("archived lifecycle Memory limit exceeds i64")?,
                i64::try_from(offset).context("archived lifecycle Memory offset exceeds i64")?,
            ],
            lifecycle_retrieval_state_from_row,
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_active_record_for_fact(
        &self,
        fact: &CanonicalMemoryFactDescriptor,
    ) -> Result<Option<MemoryLifecycleRecord>> {
        let fact_key = fact.fact_key()?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            record_select_sql!(
                "WHERE fact_key = ?1
                   AND runtime_context_excluded_at IS NULL
                   AND status IN (
                        'accepted', 'pending_materialization', 'materialized',
                        'materialization_failed'
                   )
                 LIMIT 1"
            ),
            [fact_key],
            row_to_record,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn count_archived_memory_retrieval_states(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let count = conn.query_row(
            "SELECT COUNT(*)
             FROM memory_lifecycle_retrieval_states retrieval
             JOIN memory_lifecycle_records records
               ON records.memory_id = retrieval.memory_id
             WHERE retrieval.disposition = 'archived'
               AND records.status = 'materialized'
               AND records.materialization_status = 'materialized'
               AND records.runtime_context_excluded_at IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        usize::try_from(count).context("archived lifecycle Memory count exceeds usize")
    }

    pub fn load_memory_retrieval_state_for_projection(
        &self,
        event_id: &str,
    ) -> Result<CanonicalMemoryRetrievalState> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let (state, aggregate_id, mutation_kind, event_revision) = conn
            .query_row(
                "SELECT states.memory_id, states.disposition, states.revision,
                        states.last_event_id, states.changed_at,
                        events.aggregate_id, events.mutation_kind,
                        events.aggregate_revision
                 FROM memory_lifecycle_retrieval_states states
                 JOIN canonical_outbox_events events
                   ON events.event_id = states.last_event_id
                 WHERE states.last_event_id = ?1
                   AND events.aggregate_kind = 'memory_retrieval'",
                [event_id],
                |row| {
                    let state = lifecycle_retrieval_state_from_row(row)?;
                    let event_revision_raw = row.get::<_, i64>(7)?;
                    let event_revision = u64::try_from(event_revision_raw).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })?;
                    Ok((
                        state,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        event_revision,
                    ))
                },
            )
            .optional()?
            .context("canonical lifecycle Memory retrieval projection is stale or missing")?;
        if aggregate_id != lifecycle_retrieval_aggregate_id(&state.owner_id)
            || mutation_kind != state.disposition.as_str()
            || event_revision != state.revision
        {
            anyhow::bail!(
                "canonical lifecycle Memory retrieval projection identity is inconsistent"
            );
        }
        Ok(state)
    }

    pub fn load_memory_retrieval_head_for_event(
        &self,
        event_id: &str,
    ) -> Result<CanonicalMemoryRetrievalState> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let state = conn
            .query_row(
                "SELECT states.memory_id, states.disposition, states.revision,
                        states.last_event_id, states.changed_at
                 FROM canonical_outbox_events stale_events
                 JOIN canonical_outbox_events head_events
                   ON head_events.aggregate_kind = stale_events.aggregate_kind
                  AND head_events.aggregate_id = stale_events.aggregate_id
                 JOIN memory_lifecycle_retrieval_states states
                   ON states.last_event_id = head_events.event_id
                 WHERE stale_events.event_id = ?1
                   AND stale_events.aggregate_kind = 'memory_retrieval'",
                [event_id],
                lifecycle_retrieval_state_from_row,
            )
            .optional()?
            .context("canonical lifecycle Memory retrieval event has no current head")?;
        let head = lifecycle_retrieval_state_from_conn(&conn, &state.owner_id)?
            .context("canonical lifecycle Memory retrieval head is missing")?;
        if head != state {
            anyhow::bail!("canonical lifecycle Memory retrieval head changed while loading");
        }
        Ok(state)
    }

    pub fn mark_memory_retrieval_projection_applied_if_head(
        &self,
        event_id: &str,
        revision: u64,
        projection_target: &str,
    ) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_applied_if_canonical_head(
            &mut conn,
            event_id,
            revision,
            projection_target,
        )
    }

    pub fn mark_memory_retrieval_projection_compensated_to_head(
        &self,
        stale_event_id: &str,
        head_event_id: &str,
        head_revision: u64,
        projection_target: &str,
    ) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_compensated_to_head(
            &mut conn,
            stale_event_id,
            head_event_id,
            head_revision,
            projection_target,
        )
    }

    pub fn lifecycle_events(&self, memory_id: &str) -> Result<Vec<MemoryLifecycleEvent>> {
        let Some(record) = self.get_record(memory_id)? else {
            return Ok(Vec::new());
        };
        let mut events = vec![MemoryLifecycleEvent {
            event_id: format!("memory_lifecycle:{}:{}", record.memory_id, record.status),
            memory_id: record.memory_id.clone(),
            event_type: "memory.lifecycle_status".into(),
            status: record.status,
            materialization_status: record.materialization_status,
            created_at: record.accepted_at.unwrap_or_else(Utc::now),
            rollback_event: None,
        }];
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT rollback_event_id, memory_id, proposal_id, requested_by, reason, previous_status, next_status, affected_materialized_view_ids_json, affected_runtime_surface_ids_json, created_at, audit_digest
             FROM memory_rollback_events WHERE memory_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([memory_id], row_to_rollback_event)?;
        for row in rows {
            let event = row?;
            events.push(MemoryLifecycleEvent {
                event_id: event.rollback_event_id.clone(),
                memory_id: event.memory_id.clone(),
                event_type: "memory.rolled_back".into(),
                status: event.next_status,
                materialization_status: MemoryMaterializationStatus::NotRequired,
                created_at: event.created_at,
                rollback_event: Some(event),
            });
        }
        Ok(events)
    }

    pub fn rebuild_materialized_view(
        &self,
        scope: Option<MemoryLifecycleScope>,
    ) -> Result<MemoryMaterializedView> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let view = rebuild_view_tx(&tx, scope, None, None)?;
        tx.commit()?;
        Ok(view)
    }
}

fn ensure_lifecycle_retrieval_reason_code(reason_code: &str) -> Result<()> {
    if reason_code.is_empty()
        || reason_code.len() > 96
        || !reason_code.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
    {
        anyhow::bail!("invalid canonical lifecycle Memory retrieval reason code");
    }
    Ok(())
}

fn lifecycle_retrieval_aggregate_id(memory_id: &str) -> String {
    persistence_outbox::metadata_digest(&format!(
        "memory_retrieval_owner:{MEMORY_LIFECYCLE_RETRIEVAL_OWNER_KIND}:{memory_id}"
    ))
}

fn lifecycle_memory_is_retrievable_from_conn(conn: &Connection, memory_id: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1
             FROM memory_lifecycle_records records
             WHERE records.memory_id = ?1
               AND records.status = 'materialized'
               AND records.materialization_status = 'materialized'
               AND records.runtime_context_excluded_at IS NULL
               AND NOT EXISTS (
                   SELECT 1 FROM memory_lifecycle_retrieval_states retrieval
                   WHERE retrieval.memory_id = records.memory_id
                     AND retrieval.disposition != 'active'
               )",
            [memory_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn lifecycle_retrieval_state_from_conn(
    conn: &Connection,
    memory_id: &str,
) -> Result<Option<CanonicalMemoryRetrievalState>> {
    conn.query_row(
        "SELECT memory_id, disposition, revision, last_event_id, changed_at
         FROM memory_lifecycle_retrieval_states WHERE memory_id = ?1",
        [memory_id],
        lifecycle_retrieval_state_from_row,
    )
    .optional()
    .map_err(Into::into)
}

fn lifecycle_retrieval_state_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CanonicalMemoryRetrievalState> {
    let disposition_raw = row.get::<_, String>(1)?;
    let disposition = MemoryRetrievalDisposition::parse(&disposition_raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })?;
    let revision_raw = row.get::<_, i64>(2)?;
    let revision = u64::try_from(revision_raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    Ok(CanonicalMemoryRetrievalState {
        owner_kind: MEMORY_LIFECYCLE_RETRIEVAL_OWNER_KIND.into(),
        owner_id: row.get(0)?,
        disposition,
        revision,
        last_event_id: row.get(3)?,
        changed_at: row.get(4)?,
    })
}

impl MemoryLifecycleAcceptanceInput {
    pub fn from_memory_proposal_with_terminal_origin(
        proposal: &AgentProposal,
        content: String,
        task_session_id: &str,
        conversation_owner_id: &str,
        run_id: &str,
        canonical_user_message_ref: &str,
        canonical_user_message_digest: &str,
    ) -> Result<Self> {
        let mut input = Self::from_memory_proposal(proposal, content)?;
        if task_session_id.trim().is_empty()
            || conversation_owner_id.trim().is_empty()
            || run_id.trim().is_empty()
            || canonical_user_message_ref.trim().is_empty()
            || canonical_user_message_digest.trim().is_empty()
        {
            anyhow::bail!("terminal owner Memory origin is incomplete");
        }
        input.source_task_session_id = Some(task_session_id.to_string());
        input.source_run_id = Some(run_id.to_string());
        if input.fact.scope == MemoryLifecycleScope::Conversation
            && input.fact.scope_owner_ref.is_none()
        {
            input.fact.scope_owner_ref = Some(memory_scope_owner_ref(
                MemoryLifecycleScope::Conversation,
                conversation_owner_id,
            )?);
        }
        input.evidence_ids = vec![
            proposal.id.clone(),
            canonical_user_message_ref.to_string(),
            canonical_user_message_digest.to_string(),
        ];
        Ok(input)
    }

    pub fn from_memory_proposal(proposal: &AgentProposal, content: String) -> Result<Self> {
        let reviewed_content = proposal
            .after
            .get("content")
            .and_then(serde_json::Value::as_str)
            .context("Memory proposal reviewed content is missing or not a string")?;
        if reviewed_content != content {
            anyhow::bail!("Memory proposal materialization content differs from reviewed content");
        }
        let risk_level = risk_from_proposal(proposal);
        let reviewed_risk = proposal
            .after
            .get("riskLevel")
            .or_else(|| proposal.after.get("risk_level"))
            .and_then(serde_json::Value::as_str)
            .context("Memory proposal reviewed risk level is missing or not a string")?;
        let reviewed_risk = strict_risk_level(reviewed_risk)?;
        if reviewed_risk != risk_level {
            anyhow::bail!("Memory proposal reviewed risk disagrees with proposal risk");
        }
        let sensitivity = sensitivity_from_proposal(proposal)?;
        if matches!(
            risk_level,
            MemoryLifecycleRiskLevel::High | MemoryLifecycleRiskLevel::IdentityValue
        ) && sensitivity != MemoryLifecycleSensitivity::Sensitive
        {
            anyhow::bail!("high-risk Memory proposal must be marked sensitive");
        }
        let mut fact = CanonicalMemoryFactDescriptor::new(
            content,
            scope_from_proposal(proposal)?,
            category_from_proposal(proposal)?,
            risk_level,
            sensitivity,
        )?;
        if let Some(scope_owner_ref) = proposal
            .after
            .get("scopeOwnerRef")
            .or_else(|| proposal.after.get("scope_owner_ref"))
            .and_then(serde_json::Value::as_str)
        {
            fact = fact.with_scope_owner_ref(scope_owner_ref)?;
        }
        Ok(Self {
            proposal_id: proposal.id.clone(),
            // Untyped Proposal fields are never terminal-owner authority.
            // Origin-bound Review acceptance must use
            // `from_memory_proposal_with_terminal_origin`.
            source_task_session_id: None,
            source_run_id: proposal.run_id.clone(),
            fact,
            created_by: created_by_from_source(proposal.source).into(),
            accepted_by: "user".into(),
            evidence_ids: proposal
                .source_detail
                .iter()
                .cloned()
                .chain(proposal.run_id.iter().cloned())
                .chain(std::iter::once(proposal.id.clone()))
                .collect(),
            confidence: format!("{:.3}", proposal.confidence),
            conflict_ids: conflict_ids_from_proposal(proposal),
            supersedes_memory_id: proposal
                .after
                .get("supersedesMemoryId")
                .or_else(|| proposal.after.get("supersedes_memory_id"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        })
    }
}

fn scope_from_proposal(proposal: &AgentProposal) -> Result<MemoryLifecycleScope> {
    let after_scope = proposal
        .after
        .get("scope")
        .or_else(|| proposal.after.get("memoryScope"));
    if let Some(scope) = after_scope {
        return MemoryLifecycleScope::from_str(
            scope
                .as_str()
                .context("Memory lifecycle scope must be a string")?,
        );
    }
    let path = proposal.affected_path.to_ascii_lowercase();
    Ok(if path.contains("project") {
        MemoryLifecycleScope::Project
    } else if path.contains("workspace") {
        MemoryLifecycleScope::Workspace
    } else if path.contains("conversation") {
        MemoryLifecycleScope::Conversation
    } else {
        MemoryLifecycleScope::Global
    })
}

fn category_from_proposal(proposal: &AgentProposal) -> Result<MemoryLifecycleCategory> {
    let kind = proposal
        .after
        .get("candidateKind")
        .or_else(|| proposal.after.get("candidate_kind"));
    let category = proposal.after.get("category");
    if proposal.proposal_type == ProposalType::MemoryWrite && (kind.is_none() || category.is_none())
    {
        anyhow::bail!("Memory write proposal requires candidateKind and category");
    }
    if let Some(kind) = kind {
        let kind = kind
            .as_str()
            .context("Memory candidate kind must be a string")?;
        let kind = memory_candidate_kind_from_str(kind)
            .with_context(|| format!("unknown Memory candidate kind: {kind}"))?;
        let expected = memory_lifecycle_category_for_candidate_kind(kind);
        let reviewed = MemoryLifecycleCategory::from_str(
            category
                .and_then(serde_json::Value::as_str)
                .context("Memory lifecycle category must be a string")?,
        )?;
        if reviewed != expected {
            anyhow::bail!("Memory candidate kind and reviewed category disagree");
        }
        return Ok(expected);
    }
    if let Some(category) = category {
        return MemoryLifecycleCategory::from_str(
            category
                .as_str()
                .context("Memory lifecycle category must be a string")?,
        );
    }
    Ok(match proposal.proposal_type {
        ProposalType::PreferenceUpdate | ProposalType::MemoryWrite => {
            MemoryLifecycleCategory::Preference
        }
        ProposalType::MemoryArchive => MemoryLifecycleCategory::Correction,
        _ => MemoryLifecycleCategory::Fact,
    })
}

fn strict_risk_level(value: &str) -> Result<MemoryLifecycleRiskLevel> {
    match value {
        "low" => Ok(MemoryLifecycleRiskLevel::Low),
        "medium" => Ok(MemoryLifecycleRiskLevel::Medium),
        "high" => Ok(MemoryLifecycleRiskLevel::High),
        "identity_value" => Ok(MemoryLifecycleRiskLevel::IdentityValue),
        _ => anyhow::bail!("unknown Memory lifecycle risk level: {value}"),
    }
}

fn memory_candidate_kind_from_str(value: &str) -> Option<MemoryCandidateKind> {
    match value {
        "episodic_life_event" => Some(MemoryCandidateKind::EpisodicLifeEvent),
        "semantic_user_fact" => Some(MemoryCandidateKind::SemanticUserFact),
        "procedural_rule" => Some(MemoryCandidateKind::ProceduralRule),
        "preference" => Some(MemoryCandidateKind::Preference),
        "identity_or_role" => Some(MemoryCandidateKind::IdentityOrRole),
        _ => None,
    }
}

fn risk_from_proposal(proposal: &AgentProposal) -> MemoryLifecycleRiskLevel {
    let path = proposal.affected_path.to_ascii_lowercase();
    if path.contains("identity") || path.contains("value") {
        return MemoryLifecycleRiskLevel::IdentityValue;
    }
    match proposal.risk_level {
        RiskLevel::Low => MemoryLifecycleRiskLevel::Low,
        RiskLevel::Medium => MemoryLifecycleRiskLevel::Medium,
        RiskLevel::High => MemoryLifecycleRiskLevel::High,
        RiskLevel::Critical => MemoryLifecycleRiskLevel::IdentityValue,
    }
}

fn sensitivity_from_proposal(proposal: &AgentProposal) -> Result<MemoryLifecycleSensitivity> {
    let value = proposal
        .after
        .get("sensitivity")
        .context("Memory proposal reviewed sensitivity is missing")?;
    match value
        .as_str()
        .context("Memory lifecycle sensitivity must be a string")?
    {
        "internal" => Ok(MemoryLifecycleSensitivity::Internal),
        "sensitive" => Ok(MemoryLifecycleSensitivity::Sensitive),
        value => anyhow::bail!("unknown Memory lifecycle sensitivity: {value}"),
    }
}

fn legacy_default_sensitivity(risk_level: MemoryLifecycleRiskLevel) -> MemoryLifecycleSensitivity {
    match risk_level {
        MemoryLifecycleRiskLevel::Low => MemoryLifecycleSensitivity::Internal,
        MemoryLifecycleRiskLevel::Medium
        | MemoryLifecycleRiskLevel::High
        | MemoryLifecycleRiskLevel::IdentityValue => MemoryLifecycleSensitivity::Sensitive,
    }
}

fn created_by_from_source(source: ProposalSource) -> &'static str {
    match source {
        ProposalSource::Manual => "user",
        _ => "agent",
    }
}

fn conflict_ids_from_proposal(proposal: &AgentProposal) -> Vec<String> {
    proposal
        .after
        .get("conflictIds")
        .or_else(|| proposal.after.get("conflict_ids"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn configure_memory_lifecycle_connection(conn: &Connection, file_backed: bool) -> Result<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    if file_backed {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
    }
    Ok(())
}

/// Canonical identity for one active Memory fact. Governance metadata that can
/// legitimately vary between proposals (source, evidence and confidence) is
/// deliberately excluded; scope owner, scope, category and normalized content
/// define the semantic identity without rewriting the canonical body. Each admission
/// retains its exact risk and sensitivity, while the shared owner only moves
/// toward the more conservative effective governance level.
fn canonical_memory_fact_identity(
    scope: MemoryLifecycleScope,
    scope_owner_ref: Option<&str>,
    category: MemoryLifecycleCategory,
    content: &str,
) -> Result<(String, String)> {
    validate_scope_owner_ref(scope, scope_owner_ref)?;
    let compatibility_normalized = content.nfkc().collect::<String>();
    let normalized = compatibility_normalized
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if normalized.is_empty() {
        anyhow::bail!("canonical Memory fact content is empty");
    }
    let mut material = Vec::new();
    for value in [
        "openlife_memory_fact_v2",
        scope.as_str(),
        scope_owner_ref.unwrap_or(""),
        category.as_str(),
        normalized.as_str(),
    ] {
        let length = u64::try_from(value.len()).context("Memory fact identity is too large")?;
        material.extend_from_slice(&length.to_be_bytes());
        material.extend_from_slice(value.as_bytes());
    }
    let digest = digest(&SHA256, &material);
    let encoded = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok((normalized, format!("memory_fact_v2:sha256:{encoded}")))
}

fn validate_scope_owner_ref(
    scope: MemoryLifecycleScope,
    scope_owner_ref: Option<&str>,
) -> Result<()> {
    match (scope, scope_owner_ref) {
        (MemoryLifecycleScope::Global, None) => Ok(()),
        (MemoryLifecycleScope::Global, Some(_)) => {
            anyhow::bail!("global Memory must not carry a scope owner")
        }
        (_, Some(value))
            if value.trim() == value
                && (1..=160).contains(&value.len())
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.')
                }) =>
        {
            Ok(())
        }
        // Older rows and callers may still produce an unbound non-global
        // fact. It remains canonical and user-visible, but the runtime scope
        // filter treats it as ineligible instead of guessing an owner.
        (_, None) => Ok(()),
        _ => anyhow::bail!("non-global Memory scope owner ref is invalid"),
    }
}

fn digest_length_delimited_values(namespace: &str, values: &[&str]) -> String {
    let mut material = Vec::new();
    for value in std::iter::once(namespace).chain(values.iter().copied()) {
        material.extend_from_slice(&(value.len() as u64).to_be_bytes());
        material.extend_from_slice(value.as_bytes());
    }
    let digest = digest(&SHA256, &material);
    let encoded = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{encoded}")
}

/// Builds a stable, non-path-bearing identity for one user-selected runtime
/// scope. The raw conversation id or filesystem path never enters Memory
/// records, prompt explanations, or audit payloads.
pub fn memory_scope_owner_ref(
    scope: MemoryLifecycleScope,
    canonical_identity: &str,
) -> Result<String> {
    if scope == MemoryLifecycleScope::Global {
        anyhow::bail!("global Memory has no scope owner");
    }
    let canonical_identity = canonical_identity.trim();
    if canonical_identity.is_empty() {
        anyhow::bail!("Memory scope owner identity is empty");
    }
    let digest = digest_length_delimited_values(
        "openlife_memory_scope_owner_v1",
        &[scope.as_str(), canonical_identity],
    );
    Ok(format!("{}:{}", scope.as_str(), digest))
}

/// Binds an accepted fact to runtime-owned scope identity. Proposal JSON may
/// describe the scope, but it cannot choose a different project/workspace
/// owner than the one supplied by the trusted product runtime.
pub fn bind_memory_fact_scope_owner(
    fact: &mut CanonicalMemoryFactDescriptor,
    conversation_identity: Option<&str>,
    workspace_identity: Option<&str>,
    project_identity: Option<&str>,
) -> Result<()> {
    let identity = match fact.scope {
        MemoryLifecycleScope::Global => {
            if fact.scope_owner_ref.is_some() {
                anyhow::bail!("global Memory must not carry a scope owner");
            }
            return Ok(());
        }
        MemoryLifecycleScope::Conversation => conversation_identity,
        MemoryLifecycleScope::Workspace => workspace_identity,
        MemoryLifecycleScope::Project => project_identity,
    }
    .context("selected Memory scope owner is unavailable")?;
    let expected = memory_scope_owner_ref(fact.scope, identity)?;
    if fact
        .scope_owner_ref
        .as_ref()
        .is_some_and(|provided| provided != &expected)
    {
        anyhow::bail!("Memory proposal scope owner does not match the active selected scope");
    }
    fact.scope_owner_ref = Some(expected);
    Ok(())
}

fn memory_fact_admission_digest(fact_key: &str, fact: &CanonicalMemoryFactDescriptor) -> String {
    digest_length_delimited_values(
        "openlife_memory_admission_v1",
        &[
            fact_key,
            &fact.canonical_body,
            fact.scope.as_str(),
            fact.scope_owner_ref.as_deref().unwrap_or(""),
            fact.category.as_str(),
            fact.risk_level.as_str(),
            fact.sensitivity.as_str(),
        ],
    )
}

fn memory_record_audit_digest(record: &MemoryLifecycleRecord, fact_key: &str) -> String {
    digest_length_delimited_values(
        "openlife_memory_record_audit_v1",
        &[
            &record.memory_id,
            &record.proposal_id,
            fact_key,
            &record.content,
            record.scope.as_str(),
            record.scope_owner_ref.as_deref().unwrap_or(""),
            record.category.as_str(),
            record.risk_level.as_str(),
            record.sensitivity.as_str(),
            &record.created_by,
            record.accepted_by.as_deref().unwrap_or(""),
        ],
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemoryAdmissionLink {
    memory_id: String,
    admitted_memory_id: String,
    fact_key: String,
    admission_digest: String,
    risk_level: MemoryLifecycleRiskLevel,
    sensitivity: MemoryLifecycleSensitivity,
    linked_at: DateTime<Utc>,
}

fn proposal_link_tx(
    tx: &rusqlite::Transaction<'_>,
    proposal_id: &str,
) -> Result<Option<MemoryAdmissionLink>> {
    tx.query_row(
        "SELECT memory_id, admitted_memory_id, fact_key, admission_digest,
                risk_level, sensitivity, linked_at
         FROM memory_lifecycle_proposal_links
         WHERE proposal_id = ?1",
        [proposal_id],
        |row| {
            Ok(MemoryAdmissionLink {
                memory_id: row.get(0)?,
                admitted_memory_id: row.get(1)?,
                fact_key: row.get(2)?,
                admission_digest: row.get(3)?,
                risk_level: MemoryLifecycleRiskLevel::from_str(&row.get::<_, String>(4)?),
                sensitivity: MemoryLifecycleSensitivity::from_str(&row.get::<_, String>(5)?),
                linked_at: parse_time(&row.get::<_, String>(6)?),
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

// The canonical proposal-memory relation is one transactional row whose
// identity and audit fields remain explicit.
#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
fn link_proposal_to_memory_tx(
    tx: &rusqlite::Transaction<'_>,
    proposal_id: &str,
    memory_id: &str,
    fact_key: &str,
    admission_digest: &str,
    risk_level: MemoryLifecycleRiskLevel,
    sensitivity: MemoryLifecycleSensitivity,
    linked_at: DateTime<Utc>,
) -> Result<()> {
    if proposal_id.trim().is_empty()
        || memory_id.trim().is_empty()
        || fact_key.trim().is_empty()
        || admission_digest.trim().is_empty()
    {
        anyhow::bail!("Memory proposal link identity is incomplete");
    }
    if let Some(existing) = proposal_link_tx(tx, proposal_id)? {
        if existing.memory_id == memory_id
            && existing.admitted_memory_id == memory_id
            && existing.fact_key == fact_key
            && existing.admission_digest == admission_digest
            && existing.risk_level == risk_level
            && existing.sensitivity == sensitivity
        {
            return Ok(());
        }
        anyhow::bail!("Memory proposal link conflicts with its canonical owner");
    }
    tx.execute(
        "INSERT INTO memory_lifecycle_proposal_links (
            proposal_id, memory_id, admitted_memory_id, fact_key,
            admission_digest, risk_level, sensitivity, linked_at
         ) VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            proposal_id,
            memory_id,
            fact_key,
            admission_digest,
            risk_level.to_string(),
            sensitivity.to_string(),
            linked_at.to_rfc3339()
        ],
    )?;
    Ok(())
}

fn ensure_admission_link_matches(
    link: &MemoryAdmissionLink,
    fact_key: &str,
    admission_digest: &str,
    fact: &CanonicalMemoryFactDescriptor,
) -> Result<()> {
    if link.fact_key != fact_key
        || link.admission_digest != admission_digest
        || link.risk_level != fact.risk_level
        || link.sensitivity != fact.sensitivity
    {
        anyhow::bail!(
            "memory proposal idempotency key was reused with a different canonical admission"
        );
    }
    Ok(())
}

fn record_by_memory_id_tx(
    tx: &rusqlite::Transaction<'_>,
    memory_id: &str,
) -> Result<Option<MemoryLifecycleRecord>> {
    tx.query_row(
        record_select_sql!("WHERE memory_id = ?1"),
        [memory_id],
        row_to_record,
    )
    .optional()
    .map_err(Into::into)
}

fn active_record_by_fact_key_tx(
    tx: &rusqlite::Transaction<'_>,
    fact_key: &str,
) -> Result<Option<MemoryLifecycleRecord>> {
    tx.query_row(
        record_select_sql!(
            "WHERE fact_key = ?1
               AND runtime_context_excluded_at IS NULL
               AND status IN (
                    'accepted', 'pending_materialization', 'materialized',
                    'materialization_failed'
               )
             LIMIT 1"
        ),
        [fact_key],
        row_to_record,
    )
    .optional()
    .map_err(Into::into)
}

fn merge_effective_governance_tx(
    tx: &rusqlite::Transaction<'_>,
    mut record: MemoryLifecycleRecord,
    fact_key: &str,
    admitted_risk: MemoryLifecycleRiskLevel,
    admitted_sensitivity: MemoryLifecycleSensitivity,
) -> Result<(MemoryLifecycleRecord, bool)> {
    let effective_risk = record.risk_level.conservative_max(admitted_risk);
    let effective_sensitivity = record.sensitivity.conservative_max(admitted_sensitivity);
    if effective_risk == record.risk_level && effective_sensitivity == record.sensitivity {
        return Ok((record, false));
    }
    record.risk_level = effective_risk;
    record.sensitivity = effective_sensitivity;
    record.audit_digest = memory_record_audit_digest(&record, fact_key);
    update_record_tx(tx, &record, Utc::now())?;
    if record.status.is_runtime_active()
        && record.materialization_status == MemoryMaterializationStatus::Materialized
    {
        persistence_outbox::enqueue_mutation(
            tx,
            MEMORY_LIFECYCLE_AGGREGATE_KIND,
            &record.memory_id,
            "materialized",
            &memory_lifecycle_projection_token(&record),
            &MEMORY_LIFECYCLE_PROJECTION_TARGETS,
        )?;
    }
    Ok((record, true))
}

fn ensure_materialized_mutation_tx(
    tx: &rusqlite::Transaction<'_>,
    record: &MemoryLifecycleRecord,
) -> Result<CanonicalMutationReceipt> {
    let event_id = tx
        .query_row(
            "SELECT event_id FROM canonical_outbox_events
             WHERE aggregate_kind = ?1 AND aggregate_id = ?2
               AND mutation_kind = 'materialized'
             ORDER BY aggregate_revision DESC LIMIT 1",
            params![MEMORY_LIFECYCLE_AGGREGATE_KIND, record.memory_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match event_id {
        Some(event_id) => persistence_outbox::mutation_by_event_id(tx, &event_id)?
            .context("MemoryLifecycle materialized outbox event is missing"),
        None => persistence_outbox::enqueue_mutation(
            tx,
            MEMORY_LIFECYCLE_AGGREGATE_KIND,
            &record.memory_id,
            "materialized",
            &memory_lifecycle_projection_token(record),
            &MEMORY_LIFECYCLE_PROJECTION_TARGETS,
        ),
    }
}

fn existing_acceptance_report_tx(
    tx: &rusqlite::Transaction<'_>,
    mut record: MemoryLifecycleRecord,
    fact_key: String,
    admission_outcome: MemoryAdmissionOutcome,
    admission_at: DateTime<Utc>,
) -> Result<MemoryLifecycleAcceptanceReport> {
    let materialized_view = match record
        .materialized_view_id
        .as_deref()
        .map(|view_id| load_materialized_view_tx(tx, view_id))
        .transpose()?
        .flatten()
    {
        Some(view) => view,
        None => {
            let view = rebuild_view_tx(
                tx,
                Some(record.scope),
                record
                    .status
                    .is_runtime_active()
                    .then_some(record.memory_id.as_str()),
                (!record.status.is_runtime_active()).then_some(record.memory_id.as_str()),
            )?;
            record.materialized_view_id = Some(view.materialized_view_id.clone());
            record.materialized_view_version = Some(view.version);
            update_record_tx(tx, &record, Utc::now())?;
            view
        }
    };
    let canonical_mutation = ensure_materialized_mutation_tx(tx, &record)?;
    let projection_state =
        persistence_outbox::projection_summary(tx, &canonical_mutation.event_id)?.state();
    let owner_accepted_at = record.accepted_at;
    Ok(MemoryLifecycleAcceptanceReport {
        record,
        materialized_view: Some(materialized_view),
        preceding_canonical_mutations: Vec::new(),
        canonical_mutation: Some(canonical_mutation),
        canonical_committed: true,
        canonical_fact_key: fact_key,
        newly_committed: admission_outcome == MemoryAdmissionOutcome::GovernanceUpgraded,
        admission_outcome,
        admission_at,
        owner_accepted_at,
        projection_state,
    })
}

fn terminal_historical_acceptance_report(
    record: MemoryLifecycleRecord,
    fact_key: String,
    admission_at: DateTime<Utc>,
) -> MemoryLifecycleAcceptanceReport {
    let projection_state = terminal_projection_state(record.status);
    MemoryLifecycleAcceptanceReport {
        owner_accepted_at: record.accepted_at,
        record,
        materialized_view: None,
        preceding_canonical_mutations: Vec::new(),
        canonical_mutation: None,
        canonical_committed: false,
        canonical_fact_key: fact_key,
        newly_committed: false,
        admission_outcome: MemoryAdmissionOutcome::TerminalHistorical,
        admission_at,
        projection_state,
    }
}

fn terminal_projection_state(status: MemoryLifecycleStatus) -> ProjectionDeliveryState {
    if status == MemoryLifecycleStatus::RolledBack {
        ProjectionDeliveryState::Compensated
    } else {
        ProjectionDeliveryState::Superseded
    }
}

fn load_materialized_view_tx(
    tx: &rusqlite::Transaction<'_>,
    view_id: &str,
) -> Result<Option<MemoryMaterializedView>> {
    tx.query_row(
        "SELECT materialized_view_id, scope, version, active_memory_ids_json,
                runtime_surface_ids_json, updated_at, content_digest
         FROM memory_materialized_views WHERE materialized_view_id = ?1",
        [view_id],
        |row| {
            let scope = row
                .get::<_, Option<String>>(1)?
                .map(|scope| {
                    MemoryLifecycleScope::from_str(&scope)
                        .map_err(|_| invalid_db_enum_error(1, "scope", &scope))
                })
                .transpose()?;
            let active_ids_json: String = row.get(3)?;
            let runtime_ids_json: String = row.get(4)?;
            let updated_at: String = row.get(5)?;
            Ok(MemoryMaterializedView {
                materialized_view_id: row.get(0)?,
                scope,
                version: row.get(2)?,
                active_memory_ids: serde_json::from_str(&active_ids_json).unwrap_or_default(),
                runtime_surface_ids: serde_json::from_str(&runtime_ids_json).unwrap_or_default(),
                updated_at: parse_time(&updated_at),
                content_digest: row.get(6)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn rebuild_memory_lifecycle_retrieval_table_if_needed_tx(
    tx: &rusqlite::Transaction<'_>,
) -> Result<()> {
    let sql = tx
        .query_row(
            "SELECT sql FROM sqlite_master
             WHERE type = 'table' AND name = 'memory_lifecycle_retrieval_states'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if sql.as_deref().is_some_and(|sql| sql.contains("'paused'")) {
        return Ok(());
    }
    tx.execute_batch(
        "DROP INDEX IF EXISTS idx_memory_lifecycle_retrieval_disposition;
         ALTER TABLE memory_lifecycle_retrieval_states
             RENAME TO memory_lifecycle_retrieval_states_v5;
         CREATE TABLE memory_lifecycle_retrieval_states (
            memory_id TEXT PRIMARY KEY,
            disposition TEXT NOT NULL CHECK(disposition IN ('active', 'paused', 'archived')),
            revision INTEGER NOT NULL CHECK(revision > 0),
            last_event_id TEXT NOT NULL,
            reason_digest TEXT NOT NULL CHECK(TRIM(reason_digest) != ''),
            changed_at TEXT NOT NULL,
            FOREIGN KEY(memory_id) REFERENCES memory_lifecycle_records(memory_id),
            FOREIGN KEY(last_event_id) REFERENCES canonical_outbox_events(event_id)
         ) WITHOUT ROWID;
         INSERT INTO memory_lifecycle_retrieval_states (
            memory_id, disposition, revision, last_event_id, reason_digest, changed_at
         )
         SELECT memory_id, disposition, revision, last_event_id, reason_digest, changed_at
         FROM memory_lifecycle_retrieval_states_v5;
         DROP TABLE memory_lifecycle_retrieval_states_v5;
         CREATE INDEX idx_memory_lifecycle_retrieval_disposition
         ON memory_lifecycle_retrieval_states(disposition, changed_at DESC);",
    )?;
    Ok(())
}

fn migrate_memory_fact_identity_tx(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    let records = {
        let mut statement = tx.prepare(
            "SELECT memory_id, proposal_id, content, scope, scope_owner_ref, category, risk_level,
                    COALESCE(sensitivity, ''), COALESCE(fact_key, ''), status
             FROM memory_lifecycle_records ORDER BY memory_id ASC",
        )?;
        let loaded = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        loaded
    };
    let now = Utc::now();
    for (
        memory_id,
        proposal_id,
        content,
        scope,
        scope_owner_ref,
        category,
        risk_level,
        sensitivity,
        existing_fact_key,
        status,
    ) in records
    {
        let scope = MemoryLifecycleScope::from_str(&scope)?;
        let category = MemoryLifecycleCategory::from_str(&category)?;
        let risk_level = MemoryLifecycleRiskLevel::from_str(&risk_level);
        let sensitivity = if sensitivity.trim().is_empty() {
            legacy_default_sensitivity(risk_level)
        } else {
            MemoryLifecycleSensitivity::from_str(&sensitivity)
        };
        if content.is_empty() && status == "rolled_back" && !existing_fact_key.trim().is_empty() {
            // Privacy-erased rows intentionally retain only a one-way fact-key
            // digest. Reopening must not require or reconstruct the removed
            // canonical body, proposal link, or provenance.
            continue;
        }
        let mut fact =
            CanonicalMemoryFactDescriptor::new(content, scope, category, risk_level, sensitivity)?;
        fact.scope_owner_ref = scope_owner_ref;
        let (_, fact_key) = canonical_memory_fact_identity(
            fact.scope,
            fact.scope_owner_ref.as_deref(),
            fact.category,
            &fact.canonical_body,
        )?;
        let admission_digest = memory_fact_admission_digest(&fact_key, &fact);
        tx.execute(
            "UPDATE memory_lifecycle_records
             SET fact_key = ?2, risk_level = ?3, sensitivity = ?4,
                 audit_digest = COALESCE(audit_digest, '')
             WHERE memory_id = ?1",
            params![
                memory_id,
                fact_key,
                risk_level.to_string(),
                sensitivity.to_string()
            ],
        )?;
        let record = record_by_memory_id_tx(tx, &memory_id)?
            .context("Memory migration lost a canonical lifecycle record")?;
        let audit_digest = memory_record_audit_digest(&record, &fact_key);
        tx.execute(
            "UPDATE memory_lifecycle_records SET audit_digest = ?2 WHERE memory_id = ?1",
            params![memory_id, audit_digest],
        )?;
        tx.execute(
            "INSERT INTO memory_lifecycle_proposal_links (
                proposal_id, memory_id, admitted_memory_id, fact_key,
                admission_digest, risk_level, sensitivity, linked_at
             ) VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(proposal_id) DO UPDATE SET
                admitted_memory_id = CASE
                    WHEN memory_lifecycle_proposal_links.admitted_memory_id IS NULL
                      OR TRIM(memory_lifecycle_proposal_links.admitted_memory_id) = ''
                    THEN excluded.admitted_memory_id
                    ELSE memory_lifecycle_proposal_links.admitted_memory_id
                END,
                fact_key = CASE
                    WHEN memory_lifecycle_proposal_links.fact_key IS NULL
                      OR TRIM(memory_lifecycle_proposal_links.fact_key) = ''
                    THEN excluded.fact_key
                    ELSE memory_lifecycle_proposal_links.fact_key
                END,
                admission_digest = CASE
                    WHEN memory_lifecycle_proposal_links.admission_digest IS NULL
                      OR TRIM(memory_lifecycle_proposal_links.admission_digest) = ''
                    THEN excluded.admission_digest
                    ELSE memory_lifecycle_proposal_links.admission_digest
                END,
                risk_level = CASE
                    WHEN memory_lifecycle_proposal_links.risk_level IS NULL
                      OR TRIM(memory_lifecycle_proposal_links.risk_level) = ''
                    THEN excluded.risk_level
                    ELSE memory_lifecycle_proposal_links.risk_level
                END,
                sensitivity = CASE
                    WHEN memory_lifecycle_proposal_links.sensitivity IS NULL
                      OR TRIM(memory_lifecycle_proposal_links.sensitivity) = ''
                    THEN excluded.sensitivity
                    ELSE memory_lifecycle_proposal_links.sensitivity
                END",
            params![
                proposal_id,
                memory_id,
                fact_key,
                admission_digest,
                risk_level.to_string(),
                sensitivity.to_string(),
                now.to_rfc3339()
            ],
        )?;
    }
    tx.execute(
        "UPDATE memory_lifecycle_proposal_links
         SET admitted_memory_id = memory_id
         WHERE admitted_memory_id IS NULL OR TRIM(admitted_memory_id) = ''",
        [],
    )?;
    tx.execute(
        "UPDATE memory_lifecycle_proposal_links
         SET risk_level = 'high'
         WHERE risk_level IS NULL
            OR risk_level NOT IN ('low', 'medium', 'high', 'identity_value')",
        [],
    )?;
    tx.execute(
        "UPDATE memory_lifecycle_proposal_links
         SET sensitivity = 'sensitive'
         WHERE sensitivity IS NULL
            OR sensitivity NOT IN ('internal', 'sensitive')",
        [],
    )?;

    let active_records = {
        let mut statement = tx.prepare(
            "SELECT memory_id, fact_key, scope, risk_level, sensitivity
             FROM memory_lifecycle_records
             WHERE runtime_context_excluded_at IS NULL
               AND status IN (
                    'accepted', 'pending_materialization', 'materialized',
                    'materialization_failed'
             )
             ORDER BY fact_key ASC,
                      CASE
                        WHEN status = 'materialized'
                         AND materialization_status = 'materialized' THEN 5
                        WHEN materialization_status = 'pending' THEN 4
                        WHEN status = 'accepted' THEN 3
                        WHEN status = 'pending_materialization' THEN 2
                        ELSE 1
                      END DESC,
                      CASE risk_level
                        WHEN 'identity_value' THEN 4
                        WHEN 'high' THEN 3
                        WHEN 'medium' THEN 2
                        ELSE 1
                      END DESC,
                      CASE sensitivity WHEN 'sensitive' THEN 2 ELSE 1 END DESC,
                      COALESCE(accepted_at, created_at) ASC,
                      memory_id ASC",
        )?;
        let loaded = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        loaded
    };
    let mut owner_by_fact_key = BTreeMap::<String, (String, MemoryLifecycleScope)>::new();
    let mut loser_tombstones = Vec::<String>::new();
    for (memory_id, fact_key, scope, risk_level, sensitivity) in active_records {
        let scope = MemoryLifecycleScope::from_str(&scope)?;
        let risk_level = MemoryLifecycleRiskLevel::from_str(&risk_level);
        let sensitivity = MemoryLifecycleSensitivity::from_str(&sensitivity);
        let Some((owner_memory_id, _)) = owner_by_fact_key.get(&fact_key) else {
            owner_by_fact_key.insert(fact_key, (memory_id, scope));
            continue;
        };
        let owner = record_by_memory_id_tx(tx, owner_memory_id)?
            .context("Memory migration canonical fact owner is missing")?;
        merge_effective_governance_tx(tx, owner, &fact_key, risk_level, sensitivity)?;
        tx.execute(
            "UPDATE memory_lifecycle_records
             SET status = 'superseded', materialization_status = 'not_required',
                 replacement_memory_id = ?2,
                 runtime_context_excluded_at = COALESCE(runtime_context_excluded_at, ?3),
                 updated_at = ?3
             WHERE memory_id = ?1",
            params![memory_id, owner_memory_id, now.to_rfc3339()],
        )?;
        tx.execute(
            "UPDATE memory_lifecycle_proposal_links
             SET memory_id = ?2, fact_key = ?3
             WHERE memory_id = ?1",
            params![memory_id, owner_memory_id, fact_key],
        )?;
        loser_tombstones.push(memory_id);
    }

    let mut materialized_owner_seen = false;
    for (fact_key, (memory_id, scope)) in &owner_by_fact_key {
        let mut owner = record_by_memory_id_tx(tx, memory_id)?
            .context("Memory migration canonical fact owner disappeared")?;
        if !owner.status.is_runtime_active()
            || owner.materialization_status != MemoryMaterializationStatus::Materialized
        {
            continue;
        }
        materialized_owner_seen = true;
        let view = ensure_materialized_view_for_scope_tx(tx, Some(*scope))?;
        if owner.materialized_view_id.as_deref() != Some(&view.materialized_view_id)
            || owner.materialized_view_version != Some(view.version)
        {
            owner.materialized_view_id = Some(view.materialized_view_id.clone());
            owner.materialized_view_version = Some(view.version);
            owner.audit_digest = memory_record_audit_digest(&owner, fact_key);
            update_record_tx(tx, &owner, Utc::now())?;
        }
        ensure_materialized_mutation_tx(tx, &owner)?;
    }
    if materialized_owner_seen {
        ensure_materialized_view_for_scope_tx(tx, None)?;
    }

    for memory_id in loser_tombstones {
        persistence_outbox::enqueue_tombstone(
            tx,
            MEMORY_LIFECYCLE_AGGREGATE_KIND,
            &memory_id,
            Some("legacy_duplicate_memory_fact_superseded"),
            &MEMORY_LIFECYCLE_PROJECTION_TARGETS,
        )?;
    }
    Ok(())
}

fn rebuild_memory_lifecycle_tables_if_needed_tx(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    let records_sql = tx
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'memory_lifecycle_records'",
            [],
            |row| row.get::<_, String>(0),
        )
        .context("memory lifecycle record schema is missing")?;
    let links_sql = tx
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'memory_lifecycle_proposal_links'",
            [],
            |row| row.get::<_, String>(0),
        )
        .context("memory lifecycle admission-link schema is missing")?;
    let records_strict = records_sql.contains("fact_key TEXT NOT NULL CHECK")
        && records_sql.contains("CHECK(risk_level IN")
        && records_sql.contains("CHECK(sensitivity IN")
        && records_sql.contains("CHECK(scope IN")
        && records_sql.contains("CHECK(category IN");
    let links_strict = links_sql.contains("admitted_memory_id TEXT NOT NULL")
        && links_sql.contains("fact_key TEXT NOT NULL CHECK")
        && links_sql.contains("CHECK(risk_level IN")
        && links_sql.contains("CHECK(sensitivity IN")
        && links_sql.contains("FOREIGN KEY(admitted_memory_id)");
    if records_strict && links_strict {
        return Ok(());
    }

    tx.execute_batch(
        "DROP TABLE IF EXISTS memory_lifecycle_proposal_links_v4;
         DROP TABLE IF EXISTS memory_lifecycle_records_v4;
         CREATE TABLE memory_lifecycle_records_v4 (
            memory_id TEXT PRIMARY KEY,
            proposal_id TEXT NOT NULL,
            fact_key TEXT NOT NULL CHECK(TRIM(fact_key) != ''),
            source_task_session_id TEXT,
            source_run_id TEXT,
            content TEXT NOT NULL,
            scope TEXT NOT NULL CHECK(scope IN ('global', 'workspace', 'conversation', 'project')),
            scope_owner_ref TEXT,
            category TEXT NOT NULL CHECK(category IN ('preference', 'fact', 'workflow', 'correction', 'boundary')),
            risk_level TEXT NOT NULL CHECK(risk_level IN ('low', 'medium', 'high', 'identity_value')),
            sensitivity TEXT NOT NULL CHECK(sensitivity IN ('internal', 'sensitive')),
            audit_digest TEXT NOT NULL CHECK(TRIM(audit_digest) != ''),
            status TEXT NOT NULL CHECK(status IN ('candidate', 'pending_review', 'edited_pending_review', 'accepted', 'pending_materialization', 'materialized', 'materialization_failed', 'rejected', 'deferred', 'superseded', 'rolled_back')),
            materialization_status TEXT NOT NULL CHECK(materialization_status IN ('not_required', 'pending', 'materialized', 'failed')),
            materialization_error_code TEXT,
            created_by TEXT NOT NULL,
            accepted_by TEXT,
            accepted_at TEXT,
            materialized_view_id TEXT,
            materialized_view_version INTEGER,
            evidence_ids_json TEXT NOT NULL,
            confidence REAL NOT NULL,
            conflict_ids_json TEXT NOT NULL,
            supersedes_memory_id TEXT,
            replacement_memory_id TEXT,
            rolled_back_by_event_id TEXT,
            runtime_context_excluded_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );
         INSERT INTO memory_lifecycle_records_v4 (
            memory_id, proposal_id, fact_key, source_task_session_id, source_run_id,
            content, scope, scope_owner_ref, category, risk_level, sensitivity, audit_digest, status,
            materialization_status, materialization_error_code, created_by, accepted_by,
            accepted_at, materialized_view_id, materialized_view_version, evidence_ids_json,
            confidence, conflict_ids_json, supersedes_memory_id, replacement_memory_id,
            rolled_back_by_event_id, runtime_context_excluded_at, created_at, updated_at
         )
         SELECT memory_id, proposal_id, fact_key, source_task_session_id, source_run_id,
            content, scope, scope_owner_ref, category, risk_level, sensitivity, audit_digest, status,
            materialization_status, materialization_error_code, created_by, accepted_by,
            accepted_at, materialized_view_id, materialized_view_version, evidence_ids_json,
            confidence, conflict_ids_json, supersedes_memory_id, replacement_memory_id,
            rolled_back_by_event_id, runtime_context_excluded_at, created_at, updated_at
         FROM memory_lifecycle_records;
         CREATE TABLE memory_lifecycle_proposal_links_v4 (
            proposal_id TEXT PRIMARY KEY,
            memory_id TEXT NOT NULL,
            admitted_memory_id TEXT NOT NULL,
            fact_key TEXT NOT NULL CHECK(TRIM(fact_key) != ''),
            admission_digest TEXT NOT NULL CHECK(TRIM(admission_digest) != ''),
            risk_level TEXT NOT NULL CHECK(risk_level IN ('low', 'medium', 'high', 'identity_value')),
            sensitivity TEXT NOT NULL CHECK(sensitivity IN ('internal', 'sensitive')),
            linked_at TEXT NOT NULL,
            FOREIGN KEY(memory_id) REFERENCES memory_lifecycle_records_v4(memory_id),
            FOREIGN KEY(admitted_memory_id) REFERENCES memory_lifecycle_records_v4(memory_id)
         );
         INSERT INTO memory_lifecycle_proposal_links_v4 (
            proposal_id, memory_id, admitted_memory_id, fact_key,
            admission_digest, risk_level, sensitivity, linked_at
         )
         SELECT proposal_id, memory_id, admitted_memory_id, fact_key,
            admission_digest, risk_level, sensitivity, linked_at
         FROM memory_lifecycle_proposal_links;
         DROP TABLE memory_lifecycle_proposal_links;
         DROP TABLE memory_lifecycle_records;
         ALTER TABLE memory_lifecycle_records_v4 RENAME TO memory_lifecycle_records;
         ALTER TABLE memory_lifecycle_proposal_links_v4 RENAME TO memory_lifecycle_proposal_links;
         CREATE UNIQUE INDEX idx_memory_lifecycle_proposal
         ON memory_lifecycle_records(proposal_id);
         CREATE INDEX idx_memory_lifecycle_status_scope
         ON memory_lifecycle_records(status, materialization_status, scope);
         CREATE INDEX idx_memory_lifecycle_proposal_links_memory
         ON memory_lifecycle_proposal_links(memory_id);",
    )?;
    Ok(())
}

fn ensure_materialized_view_for_scope_tx(
    tx: &rusqlite::Transaction<'_>,
    scope: Option<MemoryLifecycleScope>,
) -> Result<MemoryMaterializedView> {
    let expected_active_ids = active_materialized_ids_tx(tx, scope)?;
    let view_id = view_id_for_scope(scope);
    if let Some(existing) = load_materialized_view_tx(tx, &view_id)? {
        if existing.active_memory_ids == expected_active_ids {
            return Ok(existing);
        }
    }
    rebuild_view_tx(tx, scope, None, None)
}

fn insert_record_tx(
    tx: &rusqlite::Transaction<'_>,
    record: &MemoryLifecycleRecord,
    fact_key: &str,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> Result<()> {
    let mut params = record_params(record, created_at, updated_at)
        .into_iter()
        .collect::<Vec<_>>();
    params.push(Box::new(fact_key.to_string()));
    tx.execute(
        "INSERT INTO memory_lifecycle_records (memory_id, proposal_id, fact_key, source_task_session_id, source_run_id, content, scope, scope_owner_ref, category, risk_level, sensitivity, audit_digest, status, materialization_status, materialization_error_code, created_by, accepted_by, accepted_at, materialized_view_id, materialized_view_version, evidence_ids_json, confidence, conflict_ids_json, supersedes_memory_id, replacement_memory_id, rolled_back_by_event_id, runtime_context_excluded_at, created_at, updated_at)
         VALUES (?1, ?2, ?29, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)",
        rusqlite::params_from_iter(params.iter().map(|param| param.as_ref())),
    )?;
    Ok(())
}

fn update_record_tx(
    tx: &rusqlite::Transaction<'_>,
    record: &MemoryLifecycleRecord,
    updated_at: DateTime<Utc>,
) -> Result<()> {
    let params = record_update_params(record, updated_at);
    tx.execute(
        "UPDATE memory_lifecycle_records SET proposal_id = ?2, source_task_session_id = ?3, source_run_id = ?4, content = ?5, scope = ?6, scope_owner_ref = ?7, category = ?8, risk_level = ?9, sensitivity = ?10, audit_digest = ?11, status = ?12, materialization_status = ?13, materialization_error_code = ?14, created_by = ?15, accepted_by = ?16, accepted_at = ?17, materialized_view_id = ?18, materialized_view_version = ?19, evidence_ids_json = ?20, confidence = ?21, conflict_ids_json = ?22, supersedes_memory_id = ?23, replacement_memory_id = ?24, rolled_back_by_event_id = ?25, runtime_context_excluded_at = ?26, updated_at = ?27 WHERE memory_id = ?1",
        rusqlite::params_from_iter(params.iter().map(|param| param.as_ref())),
    )?;
    Ok(())
}

fn record_params(
    record: &MemoryLifecycleRecord,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> [Box<dyn rusqlite::ToSql>; 28] {
    [
        Box::new(record.memory_id.clone()),
        Box::new(record.proposal_id.clone()),
        Box::new(record.source_task_session_id.clone()),
        Box::new(record.source_run_id.clone()),
        Box::new(record.content.clone()),
        Box::new(record.scope.to_string()),
        Box::new(record.scope_owner_ref.clone()),
        Box::new(record.category.to_string()),
        Box::new(record.risk_level.to_string()),
        Box::new(record.sensitivity.to_string()),
        Box::new(record.audit_digest.clone()),
        Box::new(record.status.to_string()),
        Box::new(record.materialization_status.to_string()),
        Box::new(record.materialization_error_code.clone()),
        Box::new(record.created_by.clone()),
        Box::new(record.accepted_by.clone()),
        Box::new(record.accepted_at.map(|time| time.to_rfc3339())),
        Box::new(record.materialized_view_id.clone()),
        Box::new(record.materialized_view_version),
        Box::new(serde_json::to_string(&record.evidence_ids).unwrap_or_else(|_| "[]".into())),
        Box::new(record.confidence),
        Box::new(serde_json::to_string(&record.conflict_ids).unwrap_or_else(|_| "[]".into())),
        Box::new(record.supersedes_memory_id.clone()),
        Box::new(record.replacement_memory_id.clone()),
        Box::new(record.rolled_back_by_event_id.clone()),
        Box::new(
            record
                .runtime_context_excluded_at
                .map(|time| time.to_rfc3339()),
        ),
        Box::new(created_at.to_rfc3339()),
        Box::new(updated_at.to_rfc3339()),
    ]
}

fn record_update_params(
    record: &MemoryLifecycleRecord,
    updated_at: DateTime<Utc>,
) -> [Box<dyn rusqlite::ToSql>; 27] {
    [
        Box::new(record.memory_id.clone()),
        Box::new(record.proposal_id.clone()),
        Box::new(record.source_task_session_id.clone()),
        Box::new(record.source_run_id.clone()),
        Box::new(record.content.clone()),
        Box::new(record.scope.to_string()),
        Box::new(record.scope_owner_ref.clone()),
        Box::new(record.category.to_string()),
        Box::new(record.risk_level.to_string()),
        Box::new(record.sensitivity.to_string()),
        Box::new(record.audit_digest.clone()),
        Box::new(record.status.to_string()),
        Box::new(record.materialization_status.to_string()),
        Box::new(record.materialization_error_code.clone()),
        Box::new(record.created_by.clone()),
        Box::new(record.accepted_by.clone()),
        Box::new(record.accepted_at.map(|time| time.to_rfc3339())),
        Box::new(record.materialized_view_id.clone()),
        Box::new(record.materialized_view_version),
        Box::new(serde_json::to_string(&record.evidence_ids).unwrap_or_else(|_| "[]".into())),
        Box::new(record.confidence),
        Box::new(serde_json::to_string(&record.conflict_ids).unwrap_or_else(|_| "[]".into())),
        Box::new(record.supersedes_memory_id.clone()),
        Box::new(record.replacement_memory_id.clone()),
        Box::new(record.rolled_back_by_event_id.clone()),
        Box::new(
            record
                .runtime_context_excluded_at
                .map(|time| time.to_rfc3339()),
        ),
        Box::new(updated_at.to_rfc3339()),
    ]
}

fn rebuild_view_tx(
    tx: &rusqlite::Transaction<'_>,
    scope: Option<MemoryLifecycleScope>,
    pending_memory_id: Option<&str>,
    excluded_memory_id: Option<&str>,
) -> Result<MemoryMaterializedView> {
    let view_id = view_id_for_scope(scope);
    let previous_version = tx
        .query_row(
            "SELECT version FROM memory_materialized_views WHERE materialized_view_id = ?1",
            [&view_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let mut active_ids = active_materialized_ids_tx(tx, scope)?;
    if let Some(memory_id) = pending_memory_id {
        if !active_ids.iter().any(|active| active == memory_id) {
            active_ids.push(memory_id.to_string());
        }
    }
    if let Some(memory_id) = excluded_memory_id {
        active_ids.retain(|active| active != memory_id);
    }
    active_ids.sort();
    active_ids.dedup();
    let runtime_surface_ids = vec![runtime_surface_id_for_scope(scope)];
    let digest_input = format!(
        "{}:{}:{}",
        view_id,
        active_ids.join("|"),
        runtime_surface_ids.join("|")
    );
    let view = MemoryMaterializedView {
        materialized_view_id: view_id.clone(),
        scope,
        version: previous_version + 1,
        active_memory_ids: active_ids,
        runtime_surface_ids,
        updated_at: Utc::now(),
        content_digest: digest_label(&digest_input),
    };
    tx.execute(
        "INSERT INTO memory_materialized_views (materialized_view_id, scope, version, active_memory_ids_json, runtime_surface_ids_json, updated_at, content_digest)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(materialized_view_id) DO UPDATE SET scope = excluded.scope, version = excluded.version, active_memory_ids_json = excluded.active_memory_ids_json, runtime_surface_ids_json = excluded.runtime_surface_ids_json, updated_at = excluded.updated_at, content_digest = excluded.content_digest",
        params![
            view.materialized_view_id,
            view.scope.map(|scope| scope.to_string()),
            view.version,
            serde_json::to_string(&view.active_memory_ids)?,
            serde_json::to_string(&view.runtime_surface_ids)?,
            view.updated_at.to_rfc3339(),
            view.content_digest,
        ],
    )?;
    Ok(view)
}

fn active_materialized_ids_tx(
    tx: &rusqlite::Transaction<'_>,
    scope: Option<MemoryLifecycleScope>,
) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    if let Some(scope) = scope {
        let mut stmt = tx.prepare(
            "SELECT memory_id FROM memory_lifecycle_records
             WHERE scope = ?1 AND status = 'materialized' AND materialization_status = 'materialized' AND runtime_context_excluded_at IS NULL
             ORDER BY accepted_at ASC, memory_id ASC",
        )?;
        let rows = stmt.query_map([scope.to_string()], |row| row.get::<_, String>(0))?;
        for row in rows {
            ids.push(row?);
        }
    } else {
        let mut stmt = tx.prepare(
            "SELECT memory_id FROM memory_lifecycle_records
             WHERE status = 'materialized' AND materialization_status = 'materialized' AND runtime_context_excluded_at IS NULL
             ORDER BY accepted_at ASC, memory_id ASC",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            ids.push(row?);
        }
    }
    Ok(ids)
}

fn insert_rollback_event_tx(
    tx: &rusqlite::Transaction<'_>,
    event: &MemoryRollbackEvent,
) -> Result<()> {
    tx.execute(
        "INSERT INTO memory_rollback_events (rollback_event_id, memory_id, proposal_id, requested_by, reason, previous_status, next_status, affected_materialized_view_ids_json, affected_runtime_surface_ids_json, created_at, audit_digest)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            event.rollback_event_id,
            event.memory_id,
            event.proposal_id,
            event.requested_by,
            event.reason,
            event.previous_status.to_string(),
            event.next_status.to_string(),
            serde_json::to_string(&event.affected_materialized_view_ids)?,
            serde_json::to_string(&event.affected_runtime_surface_ids)?,
            event.created_at.to_rfc3339(),
            event.audit_digest,
        ],
    )?;
    Ok(())
}

fn invalid_db_enum_error(column: usize, kind: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unknown Memory lifecycle {kind}: {value}"),
        )),
    )
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryLifecycleRecord> {
    let evidence_json: String = row.get(19)?;
    let conflict_json: String = row.get(21)?;
    let scope_raw = row.get::<_, String>(5)?;
    let category_raw = row.get::<_, String>(7)?;
    let scope = MemoryLifecycleScope::from_str(&scope_raw)
        .map_err(|_| invalid_db_enum_error(5, "scope", &scope_raw))?;
    let category = MemoryLifecycleCategory::from_str(&category_raw)
        .map_err(|_| invalid_db_enum_error(7, "category", &category_raw))?;
    Ok(MemoryLifecycleRecord {
        memory_id: row.get(0)?,
        proposal_id: row.get(1)?,
        source_task_session_id: row.get(2)?,
        source_run_id: row.get(3)?,
        content: row.get(4)?,
        scope,
        scope_owner_ref: row.get(6)?,
        category,
        risk_level: MemoryLifecycleRiskLevel::from_str(&row.get::<_, String>(8)?),
        sensitivity: MemoryLifecycleSensitivity::from_str(&row.get::<_, String>(9)?),
        audit_digest: row.get(10)?,
        status: MemoryLifecycleStatus::from_str(&row.get::<_, String>(11)?),
        materialization_status: MemoryMaterializationStatus::from_str(&row.get::<_, String>(12)?),
        materialization_error_code: row.get(13)?,
        created_by: row.get(14)?,
        accepted_by: row.get(15)?,
        accepted_at: parse_optional_time(row.get::<_, Option<String>>(16)?),
        materialized_view_id: row.get(17)?,
        materialized_view_version: row.get(18)?,
        evidence_ids: serde_json::from_str(&evidence_json).unwrap_or_default(),
        confidence: row.get(20)?,
        conflict_ids: serde_json::from_str(&conflict_json).unwrap_or_default(),
        supersedes_memory_id: row.get(22)?,
        replacement_memory_id: row.get(23)?,
        rolled_back_by_event_id: row.get(24)?,
        runtime_context_excluded_at: parse_optional_time(row.get::<_, Option<String>>(25)?),
    })
}

fn row_to_rollback_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRollbackEvent> {
    let view_ids_json: String = row.get(7)?;
    let surface_ids_json: String = row.get(8)?;
    let created_at: String = row.get(9)?;
    Ok(MemoryRollbackEvent {
        rollback_event_id: row.get(0)?,
        memory_id: row.get(1)?,
        proposal_id: row.get(2)?,
        requested_by: row.get(3)?,
        reason: row.get(4)?,
        previous_status: MemoryLifecycleStatus::from_str(&row.get::<_, String>(5)?),
        next_status: MemoryLifecycleStatus::from_str(&row.get::<_, String>(6)?),
        affected_materialized_view_ids: serde_json::from_str(&view_ids_json).unwrap_or_default(),
        affected_runtime_surface_ids: serde_json::from_str(&surface_ids_json).unwrap_or_default(),
        created_at: parse_time(&created_at),
        audit_digest: row.get(10)?,
    })
}

fn parse_optional_time(value: Option<String>) -> Option<DateTime<Utc>> {
    value.as_deref().map(parse_time)
}

fn parse_time(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn view_id_for_scope(scope: Option<MemoryLifecycleScope>) -> String {
    match scope {
        Some(scope) => format!("main_chat_memory_materialized_view:{}", scope),
        None => "main_chat_memory_materialized_view:all".into(),
    }
}

fn runtime_surface_id_for_scope(scope: Option<MemoryLifecycleScope>) -> String {
    match scope {
        Some(scope) => format!("main_chat_runtime_memory_context:{}", scope),
        None => "main_chat_runtime_memory_context:all".into(),
    }
}

/// Projection triggers deliberately contain only refs and an opaque receipt
/// token. The random nonce is consumed before persistence, so low-entropy
/// Memory content cannot be tested against a deterministic outbox digest. The
/// body remains owned by `memory_lifecycle_records` and is loaded by the
/// replaceable materializer when a delivery is applied.
fn memory_lifecycle_projection_token(record: &MemoryLifecycleRecord) -> String {
    let one_time_nonce = Uuid::new_v4();
    persistence_outbox::metadata_digest(&format!(
        "{}:{}:{}:{}:{}",
        record.memory_id,
        record.status,
        record.materialization_status,
        record.scope,
        one_time_nonce
    ))
}

fn digest_label(input: &str) -> String {
    let bytes = input.as_bytes();
    let digest = digest(&SHA256, bytes);
    format!(
        "bytes:{} hash:sha256:{}",
        bytes.len(),
        digest
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{AgentProposal, ProposalSource, ProposalType};
    use serde_json::json;
    use std::sync::Arc;

    fn fact_descriptor(
        content: &str,
        risk_level: MemoryLifecycleRiskLevel,
        sensitivity: MemoryLifecycleSensitivity,
    ) -> CanonicalMemoryFactDescriptor {
        CanonicalMemoryFactDescriptor::from_candidate(
            content,
            MemoryCandidateKind::SemanticUserFact,
            MemoryLifecycleScope::Global,
            risk_level,
            sensitivity,
        )
        .unwrap()
    }

    fn explicit_input(message_id: &str, content: &str) -> ExplicitMemoryWriteInput {
        ExplicitMemoryWriteInput {
            source_task_session_id: "task-session".into(),
            source_run_id: "run".into(),
            source_message_id: message_id.into(),
            source_message_digest: digest_label(content),
            authorized_candidate_id: format!("candidate:test:{message_id}"),
            fact: fact_descriptor(
                content,
                MemoryLifecycleRiskLevel::Low,
                MemoryLifecycleSensitivity::Internal,
            ),
        }
    }

    fn acceptance_input(proposal_id: &str, content: &str) -> MemoryLifecycleAcceptanceInput {
        MemoryLifecycleAcceptanceInput {
            proposal_id: proposal_id.into(),
            source_task_session_id: Some(format!("task:{proposal_id}")),
            source_run_id: Some(format!("run:{proposal_id}")),
            fact: fact_descriptor(
                content,
                MemoryLifecycleRiskLevel::Low,
                MemoryLifecycleSensitivity::Internal,
            ),
            created_by: "agent".into(),
            accepted_by: "user".into(),
            evidence_ids: vec![format!("evidence:{proposal_id}")],
            confidence: "0.900".into(),
            conflict_ids: Vec::new(),
            supersedes_memory_id: None,
        }
    }

    fn create_pre_d045_database(path: &std::path::Path, scope: &str) {
        let now = Utc::now().to_rfc3339();
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE memory_lifecycle_records (
                memory_id TEXT PRIMARY KEY,
                proposal_id TEXT NOT NULL,
                source_task_session_id TEXT,
                source_run_id TEXT,
                content TEXT NOT NULL,
                scope TEXT NOT NULL,
                category TEXT NOT NULL,
                risk_level TEXT NOT NULL,
                status TEXT NOT NULL,
                materialization_status TEXT NOT NULL,
                materialization_error_code TEXT,
                created_by TEXT NOT NULL,
                accepted_by TEXT,
                accepted_at TEXT,
                materialized_view_id TEXT,
                materialized_view_version INTEGER,
                evidence_ids_json TEXT NOT NULL,
                confidence REAL NOT NULL,
                conflict_ids_json TEXT NOT NULL,
                supersedes_memory_id TEXT,
                replacement_memory_id TEXT,
                rolled_back_by_event_id TEXT,
                runtime_context_excluded_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memory_lifecycle_records (
                memory_id, proposal_id, content, scope, category, risk_level,
                status, materialization_status, created_by, accepted_by,
                accepted_at, evidence_ids_json, confidence, conflict_ids_json,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'fact', 'low', 'materialized',
                'materialized', 'legacy', 'user', ?5, '[]', 0.9, '[]', ?5, ?5)",
            params![
                "memory:pre-d045",
                "proposal-pre-d045",
                "Pre D045 fact",
                scope,
                now
            ],
        )
        .unwrap();
    }

    #[test]
    fn explicit_and_proposal_lanes_share_one_typed_canonical_owner_in_both_orders() {
        let explicit_first = MemoryLifecycleStore::new_in_memory().unwrap();
        let mut explicit = explicit_input("message-explicit-first", "Line one\nLine two");
        explicit.fact.risk_level = MemoryLifecycleRiskLevel::Medium;
        let explicit_receipt = explicit_first
            .commit_test_explicit_user_memory(explicit)
            .unwrap();
        let accepted = explicit_first
            .accept_memory_proposal(acceptance_input(
                "proposal-after-explicit",
                "Line one Line two",
            ))
            .unwrap();

        assert_eq!(accepted.record.memory_id, explicit_receipt.memory_id);
        assert_eq!(
            accepted.admission_outcome,
            MemoryAdmissionOutcome::AliasLinked
        );
        assert_eq!(accepted.record.content, "Line one\nLine two");
        assert_eq!(accepted.record.risk_level, MemoryLifecycleRiskLevel::Medium);
        assert_eq!(
            accepted.record.sensitivity,
            MemoryLifecycleSensitivity::Internal
        );
        assert_eq!(accepted.record.audit_digest, explicit_receipt.audit_digest);
        assert_eq!(
            explicit_first.list_active_records(None, 10).unwrap().len(),
            1
        );

        let proposal_first = MemoryLifecycleStore::new_in_memory().unwrap();
        let proposed = proposal_first
            .accept_memory_proposal(acceptance_input(
                "proposal-before-explicit",
                "Stable canonical fact",
            ))
            .unwrap();
        let mut explicit = explicit_input("message-after-proposal", "Stable canonical fact");
        explicit.fact.risk_level = MemoryLifecycleRiskLevel::Medium;
        let explicit_receipt = proposal_first
            .commit_test_explicit_user_memory(explicit)
            .unwrap();

        assert_eq!(explicit_receipt.memory_id, proposed.record.memory_id);
        assert_eq!(
            explicit_receipt.admission_outcome,
            MemoryAdmissionOutcome::GovernanceUpgraded
        );
        let owner = proposal_first
            .get_record(&proposed.record.memory_id)
            .unwrap()
            .unwrap();
        assert_eq!(owner.risk_level, MemoryLifecycleRiskLevel::Medium);
        assert_eq!(owner.sensitivity, MemoryLifecycleSensitivity::Internal);
        assert_eq!(owner.audit_digest, explicit_receipt.audit_digest);
        assert_eq!(
            proposal_first.list_active_records(None, 10).unwrap().len(),
            1
        );
    }

    #[test]
    fn explicit_and_proposal_cross_connection_race_keeps_one_conservative_owner() {
        let directory = tempfile::tempdir().expect("temporary lifecycle directory");
        let path = directory.path().join("mixed-lane-race.db");
        let stores = (0..8)
            .map(|_| MemoryLifecycleStore::new(&path).expect("open lifecycle connection"))
            .collect::<Vec<_>>();
        let barrier = Arc::new(std::sync::Barrier::new(stores.len()));
        let handles = stores
            .into_iter()
            .enumerate()
            .map(|(index, store)| {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    if index % 2 == 0 {
                        let mut input = explicit_input(
                            &format!("mixed-message-{index}"),
                            "One mixed lane fact",
                        );
                        input.fact.risk_level = MemoryLifecycleRiskLevel::Medium;
                        store
                            .commit_test_explicit_user_memory(input)
                            .map(|receipt| receipt.memory_id)
                    } else {
                        store
                            .accept_memory_proposal(acceptance_input(
                                &format!("mixed-proposal-{index}"),
                                "One mixed lane fact",
                            ))
                            .map(|report| report.record.memory_id)
                    }
                })
            })
            .collect::<Vec<_>>();
        let memory_ids = handles
            .into_iter()
            .map(|handle| handle.join().unwrap().unwrap())
            .collect::<Vec<_>>();

        assert!(memory_ids
            .iter()
            .all(|memory_id| memory_id == &memory_ids[0]));
        let verifier = MemoryLifecycleStore::new(&path).unwrap();
        let active = verifier.list_active_records(None, 10).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].risk_level, MemoryLifecycleRiskLevel::Medium);
        assert_eq!(active[0].sensitivity, MemoryLifecycleSensitivity::Internal);
        assert!(active[0].audit_digest.starts_with("sha256:"));
    }

    #[test]
    fn explicit_medium_memory_persists_sensitivity_and_audit_receipt() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let mut input = explicit_input("message-medium", "A medium-risk exact fact");
        input.fact.risk_level = MemoryLifecycleRiskLevel::Medium;
        let receipt = store.commit_test_explicit_user_memory(input).unwrap();
        let owner = store.get_record(&receipt.memory_id).unwrap().unwrap();

        assert_eq!(owner.risk_level, MemoryLifecycleRiskLevel::Medium);
        assert_eq!(owner.sensitivity, MemoryLifecycleSensitivity::Internal);
        assert_eq!(receipt.sensitivity, MemoryLifecycleSensitivity::Internal);
        assert_eq!(receipt.audit_digest, owner.audit_digest);
        assert!(receipt.audit_digest.starts_with("sha256:"));
    }

    #[test]
    fn proposal_candidate_kind_uses_the_same_typed_fact_descriptor_as_explicit_memory() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.records",
            json!({
                "content": "User prefers focused work before lunch.",
                "scope": "global",
                "category": "fact",
                "candidateKind": "semantic_user_fact",
                "riskLevel": "medium",
                "sensitivity": "internal"
            }),
            "The user approved a semantic Memory fact.",
            0.9,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        proposal.id = "proposal-typed-semantic-fact".into();
        let proposal_input = MemoryLifecycleAcceptanceInput::from_memory_proposal(
            &proposal,
            "User prefers focused work before lunch.".into(),
        )
        .unwrap();

        assert_eq!(proposal_input.fact.category, MemoryLifecycleCategory::Fact);
        assert_eq!(
            proposal_input.fact.risk_level,
            MemoryLifecycleRiskLevel::Medium
        );
        assert_eq!(
            proposal_input.fact.sensitivity,
            MemoryLifecycleSensitivity::Internal
        );
        let proposed = store.accept_memory_proposal(proposal_input).unwrap();
        let explicit = store
            .commit_test_explicit_user_memory(ExplicitMemoryWriteInput {
                source_task_session_id: "task-session".into(),
                source_run_id: "run".into(),
                source_message_id: "message-typed-semantic-fact".into(),
                source_message_digest: digest_label("User prefers focused work before lunch."),
                authorized_candidate_id: "candidate:test:message-typed-semantic-fact".into(),
                fact: CanonicalMemoryFactDescriptor::from_candidate(
                    "User prefers focused work before lunch.",
                    MemoryCandidateKind::SemanticUserFact,
                    MemoryLifecycleScope::Global,
                    MemoryLifecycleRiskLevel::Medium,
                    MemoryLifecycleSensitivity::Internal,
                )
                .unwrap(),
            })
            .unwrap();

        assert_eq!(explicit.memory_id, proposed.record.memory_id);
        assert!(!explicit.newly_committed);
        assert_eq!(store.list_active_records(None, 10).unwrap().len(), 1);
    }

    #[test]
    fn restart_preserves_original_admission_after_owner_governance_upgrade() {
        let directory = tempfile::tempdir().expect("temporary lifecycle directory");
        let path = directory.path().join("admission-history.db");
        let store = MemoryLifecycleStore::new(&path).unwrap();
        let explicit_input = explicit_input(
            "message-stable-admission",
            "A stable admission survives governance upgrades.",
        );
        let explicit = store
            .commit_test_explicit_user_memory(explicit_input.clone())
            .unwrap();
        let mut proposal = acceptance_input(
            "proposal-governance-upgrade",
            "A stable admission survives governance upgrades.",
        );
        proposal.fact.risk_level = MemoryLifecycleRiskLevel::Medium;
        let upgraded = store.accept_memory_proposal(proposal).unwrap();
        assert_eq!(upgraded.record.memory_id, explicit.memory_id);
        assert_eq!(upgraded.record.risk_level, MemoryLifecycleRiskLevel::Medium);
        drop(store);

        let reopened = MemoryLifecycleStore::new(&path).unwrap();
        let replay = reopened
            .commit_test_explicit_user_memory(explicit_input)
            .expect("restart must preserve the explicit lane's original admission");
        assert!(!replay.newly_committed);
        assert_eq!(replay.memory_id, explicit.memory_id);
        let owner = reopened.get_record(&replay.memory_id).unwrap().unwrap();
        assert_eq!(owner.risk_level, MemoryLifecycleRiskLevel::Medium);
    }

    #[test]
    fn distinct_equivalent_proposals_concurrently_share_one_canonical_fact() {
        let directory = tempfile::tempdir().expect("temporary lifecycle directory");
        let path = directory.path().join("memory-lifecycle.db");
        let stores = (0..8)
            .map(|_| MemoryLifecycleStore::new(&path).expect("open lifecycle connection"))
            .collect::<Vec<_>>();
        let barrier = Arc::new(std::sync::Barrier::new(stores.len()));
        let handles = stores
            .into_iter()
            .enumerate()
            .map(|(index, store)| {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .accept_memory_proposal(acceptance_input(
                            &format!("proposal-concurrent-{index}"),
                            if index % 2 == 0 {
                                "User prefers focused work in the morning."
                            } else {
                                "  User prefers focused work   in the morning.  "
                            },
                        ))
                        .expect("accept equivalent proposal")
                })
            })
            .collect::<Vec<_>>();
        let reports = handles
            .into_iter()
            .map(|handle| handle.join().expect("join acceptance"))
            .collect::<Vec<_>>();

        assert_eq!(
            reports
                .iter()
                .filter(|report| report.newly_committed)
                .count(),
            1
        );
        assert!(reports
            .iter()
            .all(|report| report.record.memory_id == reports[0].record.memory_id));
        assert!(reports
            .iter()
            .all(|report| report.canonical_fact_key == reports[0].canonical_fact_key));
        assert!(reports.iter().all(
            |report| report.canonical_mutation.as_ref().unwrap().event_id
                == reports[0].canonical_mutation.as_ref().unwrap().event_id
        ));

        let verifier = MemoryLifecycleStore::new(&path).expect("reopen lifecycle store");
        assert_eq!(verifier.list_active_records(None, 20).unwrap().len(), 1);
        for index in 0..8 {
            let linked = verifier
                .get_record_by_proposal_id(&format!("proposal-concurrent-{index}"))
                .unwrap()
                .expect("proposal alias resolves");
            assert_eq!(linked.memory_id, reports[0].record.memory_id);
        }
        let conn = verifier.conn.lock().unwrap();
        let canonical_event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM canonical_outbox_events
                 WHERE aggregate_kind = 'memory_lifecycle'
                   AND mutation_kind = 'materialized'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(canonical_event_count, 1);
    }

    #[test]
    fn proposal_id_reuse_with_different_fact_fails_closed() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let first = store
            .accept_memory_proposal(acceptance_input(
                "proposal-idempotency-conflict",
                "First canonical fact.",
            ))
            .unwrap();
        let error = store
            .accept_memory_proposal(acceptance_input(
                "proposal-idempotency-conflict",
                "Different canonical fact.",
            ))
            .expect_err("proposal id cannot be rebound");

        assert!(error.to_string().contains("idempotency key"));
        assert_eq!(store.list_active_records(None, 10).unwrap().len(), 1);
        assert_eq!(
            store
                .get_record_by_proposal_id("proposal-idempotency-conflict")
                .unwrap()
                .unwrap()
                .memory_id,
            first.record.memory_id
        );
    }

    #[test]
    fn fact_identity_keeps_scope_and_category_distinct() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let global = store
            .accept_memory_proposal(acceptance_input("proposal-global-fact", "Shared words"))
            .unwrap();
        let mut project_preference =
            acceptance_input("proposal-project-preference", "Shared words");
        project_preference.fact.scope = MemoryLifecycleScope::Project;
        project_preference.fact.category = MemoryLifecycleCategory::Preference;
        let project = store.accept_memory_proposal(project_preference).unwrap();

        assert!(global.newly_committed);
        assert!(project.newly_committed);
        assert_ne!(global.canonical_fact_key, project.canonical_fact_key);
        assert_ne!(global.record.memory_id, project.record.memory_id);
    }

    #[test]
    fn fact_identity_keeps_same_project_fact_distinct_across_scope_owners() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let mut project_a = acceptance_input("proposal-project-a", "Use the release checklist");
        project_a.fact.scope = MemoryLifecycleScope::Project;
        project_a.fact.scope_owner_ref =
            Some(memory_scope_owner_ref(MemoryLifecycleScope::Project, "/tmp/project-a").unwrap());
        let mut project_b = acceptance_input("proposal-project-b", "Use the release checklist");
        project_b.fact.scope = MemoryLifecycleScope::Project;
        project_b.fact.scope_owner_ref =
            Some(memory_scope_owner_ref(MemoryLifecycleScope::Project, "/tmp/project-b").unwrap());

        let accepted_a = store.accept_memory_proposal(project_a).unwrap();
        let accepted_b = store.accept_memory_proposal(project_b).unwrap();

        assert_ne!(accepted_a.canonical_fact_key, accepted_b.canonical_fact_key);
        assert_ne!(accepted_a.record.memory_id, accepted_b.record.memory_id);
        assert_ne!(
            accepted_a.record.scope_owner_ref,
            accepted_b.record.scope_owner_ref
        );
    }

    #[test]
    fn trusted_scope_binding_rejects_a_forged_project_owner() {
        let mut fact = fact_descriptor(
            "Use the release checklist",
            MemoryLifecycleRiskLevel::Low,
            MemoryLifecycleSensitivity::Internal,
        );
        fact.scope = MemoryLifecycleScope::Project;
        bind_memory_fact_scope_owner(&mut fact, None, None, Some("/tmp/project-a")).unwrap();
        let bound = fact.scope_owner_ref.clone().expect("project owner");

        let mut forged = fact.clone();
        forged.scope_owner_ref =
            Some(memory_scope_owner_ref(MemoryLifecycleScope::Project, "/tmp/project-b").unwrap());
        assert!(
            bind_memory_fact_scope_owner(&mut forged, None, None, Some("/tmp/project-a")).is_err()
        );
        assert_eq!(fact.scope_owner_ref.as_deref(), Some(bound.as_str()));
    }

    #[test]
    fn public_fact_identity_finds_the_existing_active_canonical_owner() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let accepted = store
            .accept_memory_proposal(acceptance_input(
                "proposal-active-fact-lookup",
                "I prefer quiet focus time.",
            ))
            .unwrap();
        let equivalent = fact_descriptor(
            "  I   prefer quiet focus time.  ",
            MemoryLifecycleRiskLevel::Medium,
            MemoryLifecycleSensitivity::Sensitive,
        );

        assert_eq!(equivalent.fact_key().unwrap(), accepted.canonical_fact_key);
        let existing = store
            .get_active_record_for_fact(&equivalent)
            .unwrap()
            .expect("active canonical owner");
        assert_eq!(existing.memory_id, accepted.record.memory_id);
    }

    #[test]
    fn fact_identity_nfkc_unifies_unicode_equivalents_without_rewriting_canonical_body() {
        let original_body = "Ｃａｆｅ\u{301} １２３";
        let (normalized_original, original_key) = canonical_memory_fact_identity(
            MemoryLifecycleScope::Global,
            None,
            MemoryLifecycleCategory::Fact,
            original_body,
        )
        .unwrap();
        let (normalized_equivalent, equivalent_key) = canonical_memory_fact_identity(
            MemoryLifecycleScope::Global,
            None,
            MemoryLifecycleCategory::Fact,
            "Café 123",
        )
        .unwrap();
        let (_, decomposed_key) = canonical_memory_fact_identity(
            MemoryLifecycleScope::Global,
            None,
            MemoryLifecycleCategory::Fact,
            "Cafe\u{301} 123",
        )
        .unwrap();

        assert_eq!(normalized_original, "Café 123");
        assert_eq!(normalized_equivalent, "Café 123");
        assert_eq!(original_key, equivalent_key);
        assert_eq!(original_key, decomposed_key);

        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let first = store
            .commit_test_explicit_user_memory(explicit_input(
                "message-unicode-original",
                original_body,
            ))
            .unwrap();
        let equivalent = store
            .commit_test_explicit_user_memory(explicit_input(
                "message-unicode-equivalent",
                "Café 123",
            ))
            .unwrap();

        assert_eq!(equivalent.memory_id, first.memory_id);
        assert!(!equivalent.newly_committed);
        assert_eq!(
            store.get_record(&first.memory_id).unwrap().unwrap().content,
            original_body
        );
    }

    #[test]
    fn malformed_risk_and_sensitivity_are_never_downgraded() {
        assert_eq!(
            MemoryLifecycleRiskLevel::from_str("low"),
            MemoryLifecycleRiskLevel::Low
        );
        assert_eq!(
            MemoryLifecycleRiskLevel::from_str("LOW"),
            MemoryLifecycleRiskLevel::High
        );
        assert_eq!(
            MemoryLifecycleRiskLevel::from_str("unexpected"),
            MemoryLifecycleRiskLevel::High
        );
        assert_eq!(
            MemoryLifecycleSensitivity::from_str("internal"),
            MemoryLifecycleSensitivity::Internal
        );
        assert_eq!(
            MemoryLifecycleSensitivity::from_candidate_label("INTERNAL"),
            MemoryLifecycleSensitivity::Sensitive
        );
        assert_eq!(
            MemoryLifecycleSensitivity::from_candidate_label("unexpected"),
            MemoryLifecycleSensitivity::Sensitive
        );
    }

    #[test]
    fn current_schema_rejects_unknown_governance_enums_without_mutating_owner() {
        let directory = tempfile::tempdir().expect("temporary lifecycle directory");
        let path = directory.path().join("conservative-enum-repair.db");
        let store = MemoryLifecycleStore::new(&path).unwrap();
        let receipt = store
            .commit_test_explicit_user_memory(explicit_input(
                "message-enum-repair",
                "Unknown governance metadata must not lower protection.",
            ))
            .unwrap();
        let before = store.get_record(&receipt.memory_id).unwrap().unwrap();
        for (column, invalid_value) in [
            ("risk_level", "unexpected-risk"),
            ("sensitivity", "unexpected-sensitivity"),
        ] {
            let conn = store.conn.lock().unwrap();
            let result = conn.execute(
                &format!("UPDATE memory_lifecycle_records SET {column} = ?2 WHERE memory_id = ?1"),
                params![receipt.memory_id, invalid_value],
            );
            assert!(
                result.is_err(),
                "{column} CHECK constraint must reject corruption"
            );
        }
        let after = store.get_record(&receipt.memory_id).unwrap().unwrap();
        assert_eq!(
            after, before,
            "rejected corruption must not mutate canonical owner"
        );
    }

    #[test]
    fn malformed_scope_category_and_candidate_kind_fail_admission() {
        let proposal = |id: &str, after: serde_json::Value| {
            let mut proposal = AgentProposal::new(
                ProposalType::MemoryWrite,
                "memory.records",
                after,
                "Malformed typed metadata must fail closed.",
                0.9,
                RiskLevel::Medium,
                ProposalSource::Manual,
            );
            proposal.id = id.into();
            proposal
        };
        let invalid_scope = proposal(
            "proposal-invalid-scope",
            json!({"content": "Typed fact", "scope": "planet", "category": "fact", "candidateKind": "semantic_user_fact", "riskLevel": "medium", "sensitivity": "internal"}),
        );
        let invalid_category = proposal(
            "proposal-invalid-category",
            json!({"content": "Typed fact", "scope": "global", "category": "guess", "candidateKind": "semantic_user_fact", "riskLevel": "medium", "sensitivity": "internal"}),
        );
        let invalid_kind = proposal(
            "proposal-invalid-kind",
            json!({"content": "Typed fact", "scope": "global", "category": "fact", "candidateKind": "maybe_fact", "riskLevel": "medium", "sensitivity": "internal"}),
        );
        let invalid_sensitivity = proposal(
            "proposal-invalid-sensitivity",
            json!({"content": "Typed fact", "scope": "global", "category": "fact", "candidateKind": "semantic_user_fact", "riskLevel": "medium", "sensitivity": 7}),
        );

        for proposal in [
            invalid_scope,
            invalid_category,
            invalid_kind,
            invalid_sensitivity,
        ] {
            assert!(MemoryLifecycleAcceptanceInput::from_memory_proposal(
                &proposal,
                "Typed fact".into(),
            )
            .is_err());
        }
    }

    #[test]
    fn proposal_admission_binds_reviewed_content_risk_sensitivity_and_candidate_category() {
        let proposal = |id: &str, risk: RiskLevel, after: serde_json::Value| {
            let mut proposal = AgentProposal::new(
                ProposalType::MemoryWrite,
                "memory.records",
                after,
                "Reviewed Memory admission must remain exact.",
                0.9,
                risk,
                ProposalSource::Manual,
            );
            proposal.id = id.into();
            proposal
        };
        let risk_mismatch = proposal(
            "proposal-risk-mismatch",
            RiskLevel::Medium,
            json!({"content": "Exact reviewed fact", "scope": "global", "category": "fact", "candidateKind": "semantic_user_fact", "riskLevel": "low", "sensitivity": "internal"}),
        );
        let unsafe_high = proposal(
            "proposal-high-internal",
            RiskLevel::High,
            json!({"content": "Exact reviewed fact", "scope": "global", "category": "fact", "candidateKind": "semantic_user_fact", "riskLevel": "high", "sensitivity": "internal"}),
        );
        let category_mismatch = proposal(
            "proposal-category-mismatch",
            RiskLevel::Medium,
            json!({"content": "Exact reviewed fact", "scope": "global", "category": "preference", "candidateKind": "semantic_user_fact", "riskLevel": "medium", "sensitivity": "internal"}),
        );
        let content_mismatch = proposal(
            "proposal-content-mismatch",
            RiskLevel::Medium,
            json!({"content": "Exact reviewed fact", "scope": "global", "category": "fact", "candidateKind": "semantic_user_fact", "riskLevel": "medium", "sensitivity": "internal"}),
        );

        let risk_error = MemoryLifecycleAcceptanceInput::from_memory_proposal(
            &risk_mismatch,
            "Exact reviewed fact".into(),
        )
        .unwrap_err();
        assert!(risk_error.to_string().contains("reviewed risk disagrees"));
        let sensitivity_error = MemoryLifecycleAcceptanceInput::from_memory_proposal(
            &unsafe_high,
            "Exact reviewed fact".into(),
        )
        .unwrap_err();
        assert!(sensitivity_error
            .to_string()
            .contains("must be marked sensitive"));
        let category_error = MemoryLifecycleAcceptanceInput::from_memory_proposal(
            &category_mismatch,
            "Exact reviewed fact".into(),
        )
        .unwrap_err();
        assert!(category_error.to_string().contains("category disagree"));
        let content_error = MemoryLifecycleAcceptanceInput::from_memory_proposal(
            &content_mismatch,
            "Changed after review".into(),
        )
        .unwrap_err();
        assert!(content_error
            .to_string()
            .contains("differs from reviewed content"));
    }

    #[test]
    fn proposal_admission_ignores_untyped_session_fields_and_uses_terminal_origin() {
        let proposal = |after: serde_json::Value, source_detail: Option<&str>| {
            let mut proposal = AgentProposal::new(
                ProposalType::MemoryWrite,
                "memory.records",
                after,
                "Task session ownership must remain exact.",
                0.9,
                RiskLevel::Medium,
                ProposalSource::Manual,
            );
            proposal.source_detail = source_detail.map(str::to_string);
            proposal
        };
        let valid_after = json!({
            "content": "Task-bound reviewed fact",
            "scope": "global",
            "category": "fact",
            "candidateKind": "semantic_user_fact",
            "riskLevel": "medium",
            "sensitivity": "internal",
            "originatingTaskSessionId": "task-session-1"
        });
        let valid = proposal(
            valid_after.clone(),
            Some("main_chat_agent_task_session:task-session-1;candidate:candidate-1"),
        );
        let input = MemoryLifecycleAcceptanceInput::from_memory_proposal(
            &valid,
            "Task-bound reviewed fact".into(),
        )
        .unwrap();
        assert_eq!(input.source_task_session_id, None);

        let terminal_bound =
            MemoryLifecycleAcceptanceInput::from_memory_proposal_with_terminal_origin(
                &valid,
                "Task-bound reviewed fact".into(),
                "task-session-1",
                "chat-session-1",
                "run-1",
                "conversation-message:1",
                "sha256:canonical-user-message",
            )
            .unwrap();
        assert_eq!(
            terminal_bound.source_task_session_id.as_deref(),
            Some("task-session-1")
        );
        assert_eq!(terminal_bound.source_run_id.as_deref(), Some("run-1"));
        assert_eq!(
            terminal_bound.fact.scope_owner_ref, None,
            "global Memory remains ownerless"
        );

        let conversation = proposal(
            json!({
                "content": "Conversation reviewed fact",
                "scope": "conversation",
                "category": "fact",
                "candidateKind": "semantic_user_fact",
                "riskLevel": "medium",
                "sensitivity": "internal"
            }),
            Some("main_chat_agent_task_session:task-session-1;candidate:candidate-2"),
        );
        let conversation_bound =
            MemoryLifecycleAcceptanceInput::from_memory_proposal_with_terminal_origin(
                &conversation,
                "Conversation reviewed fact".into(),
                "task-session-1",
                "chat-session-1",
                "run-1",
                "conversation-message:1",
                "sha256:canonical-user-message",
            )
            .unwrap();
        assert_eq!(
            conversation_bound.fact.scope_owner_ref.as_deref(),
            Some(
                memory_scope_owner_ref(MemoryLifecycleScope::Conversation, "chat-session-1")
                    .unwrap()
                    .as_str()
            )
        );
        assert_ne!(
            conversation_bound.fact.scope_owner_ref.as_deref(),
            Some(
                memory_scope_owner_ref(MemoryLifecycleScope::Conversation, "task-session-1")
                    .unwrap()
                    .as_str()
            )
        );

        let drift = proposal(
            valid_after.clone(),
            Some("main_chat_agent_task_session:task-session-2;candidate:candidate-1"),
        );
        assert_eq!(
            MemoryLifecycleAcceptanceInput::from_memory_proposal(
                &drift,
                "Task-bound reviewed fact".into(),
            )
            .unwrap()
            .source_task_session_id,
            None,
            "source_detail cannot authorize terminal ownership"
        );

        let mut alias_drift = valid_after;
        alias_drift["session_id"] = json!("task-session-2");
        assert_eq!(
            MemoryLifecycleAcceptanceInput::from_memory_proposal(
                &proposal(alias_drift, None),
                "Task-bound reviewed fact".into(),
            )
            .unwrap()
            .source_task_session_id,
            None,
            "proposal JSON aliases cannot authorize terminal ownership"
        );

        let unbound = proposal(
            json!({
                "content": "Global reviewed fact",
                "scope": "global",
                "category": "fact",
                "candidateKind": "semantic_user_fact",
                "riskLevel": "medium",
                "sensitivity": "internal"
            }),
            Some("maturation:preference.communication"),
        );
        assert_eq!(
            MemoryLifecycleAcceptanceInput::from_memory_proposal(
                &unbound,
                "Global reviewed fact".into(),
            )
            .unwrap()
            .source_task_session_id,
            None,
            "non-session source metadata must not become a MemoryStore session owner"
        );
    }

    #[test]
    fn current_schema_rejects_invalid_scope_or_category_without_mutating_owner() {
        for (column, corrupt_value) in [("scope", "planet"), ("category", "guess")] {
            let directory = tempfile::tempdir().expect("temporary lifecycle directory");
            let path = directory.path().join(format!("corrupt-{column}.db"));
            let store = MemoryLifecycleStore::new(&path).unwrap();
            let receipt = store
                .commit_test_explicit_user_memory(explicit_input(
                    &format!("message-corrupt-{column}"),
                    &format!("Corrupted {column} must not be guessed."),
                ))
                .unwrap();
            let before = store.get_record(&receipt.memory_id).unwrap().unwrap();
            {
                let conn = store.conn.lock().unwrap();
                let result = conn.execute(
                    &format!(
                        "UPDATE memory_lifecycle_records SET {column} = ?2 WHERE memory_id = ?1"
                    ),
                    params![receipt.memory_id, corrupt_value],
                );
                assert!(
                    result.is_err(),
                    "{column} CHECK constraint must reject corruption"
                );
            }
            let after = store.get_record(&receipt.memory_id).unwrap().unwrap();
            assert_eq!(
                after, before,
                "rejected corruption must preserve canonical row"
            );
        }
    }

    #[test]
    fn pre_d045_schema_is_transactionally_rebuilt_with_non_null_and_check_constraints() {
        let directory = tempfile::tempdir().expect("temporary lifecycle directory");
        let path = directory.path().join("pre-d045-schema.db");
        create_pre_d045_database(&path, "global");

        let store = MemoryLifecycleStore::new(&path).expect("strict D045 rebuild");
        assert_eq!(store.list_active_records(None, 10).unwrap().len(), 1);
        let conn = store.conn.lock().unwrap();
        let mut table_info = conn
            .prepare("PRAGMA table_info(memory_lifecycle_records)")
            .unwrap();
        let columns = table_info
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, i64>(3)?))
            })
            .unwrap()
            .collect::<std::result::Result<BTreeMap<_, _>, _>>()
            .unwrap();
        for column in ["fact_key", "risk_level", "sensitivity", "audit_digest"] {
            assert_eq!(columns.get(column), Some(&1), "{column} must be NOT NULL");
        }
        let records_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'memory_lifecycle_records'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(records_sql.contains("CHECK(risk_level IN"));
        assert!(records_sql.contains("CHECK(sensitivity IN"));
        let active_index_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'idx_memory_lifecycle_active_fact_key'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!active_index_sql.contains("fact_key IS NOT NULL"));
        let foreign_key_violation_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(foreign_key_violation_count, 0);
        assert!(conn
            .execute(
                "UPDATE memory_lifecycle_records SET fact_key = NULL WHERE memory_id = 'memory:pre-d045'",
                [],
            )
            .is_err());
        assert!(conn
            .execute(
                "UPDATE memory_lifecycle_records SET risk_level = 'unknown' WHERE memory_id = 'memory:pre-d045'",
                [],
            )
            .is_err());
    }

    #[test]
    fn failed_pre_d045_migration_rolls_back_all_schema_changes() {
        let directory = tempfile::tempdir().expect("temporary lifecycle directory");
        let path = directory.path().join("pre-d045-failed-schema.db");
        create_pre_d045_database(&path, "unknown-scope");

        let error = match MemoryLifecycleStore::new(&path) {
            Ok(_) => panic!("invalid legacy scope must fail migration"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("unknown Memory lifecycle scope"));

        let conn = Connection::open(&path).unwrap();
        let mut statement = conn
            .prepare("PRAGMA table_info(memory_lifecycle_records)")
            .unwrap();
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(!columns.iter().any(|column| column == "fact_key"));
        let added_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN (
                    'memory_lifecycle_proposal_links', 'canonical_outbox_events',
                    'canonical_outbox_deliveries', 'canonical_tombstones'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(added_table_count, 0);
    }

    #[test]
    fn legacy_duplicate_facts_migrate_to_one_conservative_owner() {
        let directory = tempfile::tempdir().expect("temporary lifecycle directory");
        let path = directory.path().join("legacy-duplicates.db");
        let store = MemoryLifecycleStore::new(&path).unwrap();
        let low = store
            .accept_memory_proposal(acceptance_input(
                "proposal-legacy-low",
                "Legacy duplicate fact",
            ))
            .unwrap();
        let (_, fact_key) = canonical_memory_fact_identity(
            MemoryLifecycleScope::Global,
            None,
            MemoryLifecycleCategory::Fact,
            "Legacy duplicate fact",
        )
        .unwrap();
        let mut high_record = low.record.clone();
        high_record.memory_id = "memory:legacy-high-risk-owner".into();
        high_record.proposal_id = "proposal-legacy-high".into();
        high_record.risk_level = MemoryLifecycleRiskLevel::High;
        high_record.sensitivity = MemoryLifecycleSensitivity::Sensitive;
        high_record.accepted_at = low
            .record
            .accepted_at
            .map(|accepted| accepted + chrono::Duration::seconds(1));
        high_record.audit_digest = memory_record_audit_digest(&high_record, &fact_key);
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            tx.execute("DROP INDEX idx_memory_lifecycle_active_fact_key", [])
                .unwrap();
            insert_record_tx(&tx, &high_record, &fact_key, Utc::now(), Utc::now()).unwrap();
            let high_fact = CanonicalMemoryFactDescriptor::new(
                high_record.content.clone(),
                high_record.scope,
                high_record.category,
                high_record.risk_level,
                high_record.sensitivity,
            )
            .unwrap();
            link_proposal_to_memory_tx(
                &tx,
                &high_record.proposal_id,
                &high_record.memory_id,
                &fact_key,
                &memory_fact_admission_digest(&fact_key, &high_fact),
                high_record.risk_level,
                high_record.sensitivity,
                Utc::now(),
            )
            .unwrap();
            tx.commit().unwrap();
        }
        drop(store);

        let migrated = MemoryLifecycleStore::new(&path).expect("migrate duplicate facts");
        let active = migrated.list_active_records(None, 10).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].memory_id, high_record.memory_id);
        assert_eq!(active[0].risk_level, MemoryLifecycleRiskLevel::High);
        assert_eq!(active[0].sensitivity, MemoryLifecycleSensitivity::Sensitive);
        assert_eq!(
            migrated
                .get_record_by_proposal_id("proposal-legacy-low")
                .unwrap()
                .unwrap()
                .memory_id,
            low.record.memory_id
        );
        assert_eq!(
            migrated
                .get_record_by_proposal_id("proposal-legacy-high")
                .unwrap()
                .unwrap()
                .memory_id,
            high_record.memory_id
        );
        let superseded = migrated.get_record(&low.record.memory_id).unwrap().unwrap();
        assert_eq!(superseded.status, MemoryLifecycleStatus::Superseded);
        assert_eq!(
            superseded.replacement_memory_id.as_deref(),
            Some(high_record.memory_id.as_str())
        );
        let conn = migrated.conn.lock().unwrap();
        let winner_materialized_events: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM canonical_outbox_events
                 WHERE aggregate_kind = 'memory_lifecycle' AND aggregate_id = ?1
                   AND mutation_kind = 'materialized'",
                [&high_record.memory_id],
                |row| row.get(0),
            )
            .unwrap();
        let winner_deliveries: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM canonical_outbox_deliveries deliveries
                 JOIN canonical_outbox_events events ON events.event_id = deliveries.event_id
                 WHERE events.aggregate_kind = 'memory_lifecycle'
                   AND events.aggregate_id = ?1 AND events.mutation_kind = 'materialized'",
                [&high_record.memory_id],
                |row| row.get(0),
            )
            .unwrap();
        let tombstone_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM canonical_tombstones
                 WHERE aggregate_kind = 'memory_lifecycle' AND aggregate_id = ?1
                   AND superseded_at IS NULL",
                [&low.record.memory_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(winner_materialized_events, 1);
        assert_eq!(winner_deliveries, 2);
        assert_eq!(tombstone_count, 1);
        drop(conn);

        let replay = migrated
            .accept_memory_proposal(acceptance_input(
                "proposal-legacy-low",
                "Legacy duplicate fact",
            ))
            .expect("legacy losing alias must replay against its original admission");
        assert!(!replay.newly_committed);
        assert_eq!(
            replay.admission_outcome,
            MemoryAdmissionOutcome::TerminalHistorical
        );
        assert!(!replay.canonical_committed);
        assert!(replay.canonical_mutation.is_none());
        assert_eq!(replay.record.memory_id, low.record.memory_id);
    }

    #[test]
    fn legacy_migration_preserves_materialized_capability_and_is_reopen_idempotent() {
        let directory = tempfile::tempdir().expect("temporary lifecycle directory");
        let path = directory.path().join("legacy-capability-owner.db");
        let store = MemoryLifecycleStore::new(&path).unwrap();
        let low_materialized = store
            .accept_memory_proposal(acceptance_input(
                "proposal-capability-low",
                "Capability must survive conservative migration",
            ))
            .unwrap();
        let fact_key = low_materialized.canonical_fact_key.clone();
        let mut failed_high = low_materialized.record.clone();
        failed_high.memory_id = "memory:legacy-failed-high".into();
        failed_high.proposal_id = "proposal-capability-failed-high".into();
        failed_high.risk_level = MemoryLifecycleRiskLevel::High;
        failed_high.sensitivity = MemoryLifecycleSensitivity::Sensitive;
        failed_high.status = MemoryLifecycleStatus::MaterializationFailed;
        failed_high.materialization_status = MemoryMaterializationStatus::Failed;
        failed_high.materialization_error_code = Some("legacy_failure".into());
        failed_high.materialized_view_id = None;
        failed_high.materialized_view_version = None;
        failed_high.audit_digest = memory_record_audit_digest(&failed_high, &fact_key);
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            tx.execute("DROP INDEX idx_memory_lifecycle_active_fact_key", [])
                .unwrap();
            insert_record_tx(&tx, &failed_high, &fact_key, Utc::now(), Utc::now()).unwrap();
            let failed_fact = CanonicalMemoryFactDescriptor::new(
                failed_high.content.clone(),
                failed_high.scope,
                failed_high.category,
                failed_high.risk_level,
                failed_high.sensitivity,
            )
            .unwrap();
            link_proposal_to_memory_tx(
                &tx,
                &failed_high.proposal_id,
                &failed_high.memory_id,
                &fact_key,
                &memory_fact_admission_digest(&fact_key, &failed_fact),
                failed_high.risk_level,
                failed_high.sensitivity,
                Utc::now(),
            )
            .unwrap();
            tx.commit().unwrap();
        }
        drop(store);

        let migrated = MemoryLifecycleStore::new(&path).unwrap();
        let active = migrated.list_active_records(None, 10).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].memory_id, low_materialized.record.memory_id);
        assert_eq!(active[0].status, MemoryLifecycleStatus::Materialized);
        assert_eq!(
            active[0].materialization_status,
            MemoryMaterializationStatus::Materialized
        );
        assert_eq!(active[0].risk_level, MemoryLifecycleRiskLevel::High);
        assert_eq!(active[0].sensitivity, MemoryLifecycleSensitivity::Sensitive);
        let failed_history = migrated
            .get_record(&failed_high.memory_id)
            .unwrap()
            .unwrap();
        assert_eq!(failed_history.status, MemoryLifecycleStatus::Superseded);
        assert_eq!(
            failed_history.replacement_memory_id.as_deref(),
            Some(low_materialized.record.memory_id.as_str())
        );
        let event_count_before_reopen: i64 = migrated
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM canonical_outbox_events
                 WHERE aggregate_kind = 'memory_lifecycle' AND aggregate_id = ?1
                   AND mutation_kind = 'materialized'",
                [&low_materialized.record.memory_id],
                |row| row.get(0),
            )
            .unwrap();
        drop(migrated);

        let reopened = MemoryLifecycleStore::new(&path).unwrap();
        let event_count_after_reopen: i64 = reopened
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM canonical_outbox_events
                 WHERE aggregate_kind = 'memory_lifecycle' AND aggregate_id = ?1
                   AND mutation_kind = 'materialized'",
                [&low_materialized.record.memory_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(event_count_after_reopen, event_count_before_reopen);
        assert_eq!(reopened.list_active_records(None, 10).unwrap().len(), 1);
        let historical = reopened
            .accept_memory_proposal(MemoryLifecycleAcceptanceInput {
                proposal_id: failed_high.proposal_id,
                source_task_session_id: None,
                source_run_id: None,
                fact: CanonicalMemoryFactDescriptor::new(
                    failed_high.content,
                    failed_high.scope,
                    failed_high.category,
                    failed_high.risk_level,
                    failed_high.sensitivity,
                )
                .unwrap(),
                created_by: "agent".into(),
                accepted_by: "user".into(),
                evidence_ids: Vec::new(),
                confidence: "0.900".into(),
                conflict_ids: Vec::new(),
                supersedes_memory_id: None,
            })
            .unwrap();
        assert_eq!(
            historical.admission_outcome,
            MemoryAdmissionOutcome::TerminalHistorical
        );
    }

    #[test]
    fn reviewed_correction_replaces_one_exact_owner_and_survives_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("correction.db");
        let store = MemoryLifecycleStore::new(&path).unwrap();
        let original = store
            .accept_memory_proposal(acceptance_input(
                "proposal-original",
                "User prefers meetings in the afternoon.",
            ))
            .unwrap();
        let mut correction =
            acceptance_input("proposal-correction", "User prefers meetings before noon.");
        correction.supersedes_memory_id = Some(original.record.memory_id.clone());
        let replacement = store.accept_memory_proposal(correction).unwrap();

        assert_eq!(replacement.preceding_canonical_mutations.len(), 1);
        assert_eq!(
            replacement.preceding_canonical_mutations[0].aggregate_id,
            original.record.memory_id
        );
        assert_eq!(
            replacement.preceding_canonical_mutations[0].mutation_kind,
            "deleted"
        );
        assert_eq!(
            replacement.record.supersedes_memory_id.as_deref(),
            Some(original.record.memory_id.as_str())
        );
        let historical = store
            .get_record(&original.record.memory_id)
            .unwrap()
            .unwrap();
        assert_eq!(historical.status, MemoryLifecycleStatus::Superseded);
        assert_eq!(
            historical.replacement_memory_id.as_deref(),
            Some(replacement.record.memory_id.as_str())
        );
        assert!(!store.is_memory_retrievable(&historical.memory_id).unwrap());
        assert!(store
            .is_memory_retrievable(&replacement.record.memory_id)
            .unwrap());
        drop(store);

        let reopened = MemoryLifecycleStore::new(&path).unwrap();
        assert_eq!(
            reopened
                .list_retrievable_records(None, 10)
                .unwrap()
                .into_iter()
                .map(|record| record.content)
                .collect::<Vec<_>>(),
            vec!["User prefers meetings before noon.".to_string()]
        );
        let mut stale = acceptance_input("proposal-stale", "A third preference");
        stale.supersedes_memory_id = Some(original.record.memory_id);
        assert!(reopened.accept_memory_proposal(stale).is_err());
    }

    #[test]
    fn privacy_erase_removes_body_and_provenance_but_keeps_metadata_tombstone() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("privacy-erase.db");
        let store = MemoryLifecycleStore::new(&path).unwrap();
        let accepted = store
            .accept_memory_proposal(acceptance_input(
                "proposal-private",
                "Private medical detail that must be erased.",
            ))
            .unwrap();
        store
            .set_memory_retrieval_disposition(
                &accepted.record.memory_id,
                MemoryRetrievalDisposition::Archived,
                "test_archive_before_erase",
            )
            .unwrap();

        let report = store
            .privacy_erase_memory_asset(&accepted.record.memory_id)
            .unwrap();
        assert!(report.canonical_committed);
        assert_eq!(report.canonical_mutation.mutation_kind, "deleted");
        let tombstone = store
            .get_record(&accepted.record.memory_id)
            .unwrap()
            .unwrap();
        assert!(tombstone.content.is_empty());
        assert!(tombstone.evidence_ids.is_empty());
        assert!(tombstone.source_task_session_id.is_none());
        assert!(tombstone.source_run_id.is_none());
        assert!(tombstone.accepted_by.is_none());
        assert!(tombstone.runtime_context_excluded_at.is_some());
        assert!(!store.is_memory_retrievable(&tombstone.memory_id).unwrap());
        assert!(store
            .memory_retrieval_state(&tombstone.memory_id)
            .unwrap()
            .is_none());
        drop(store);

        let reopened = MemoryLifecycleStore::new(&path).unwrap();
        let tombstone = reopened
            .get_record(&accepted.record.memory_id)
            .unwrap()
            .unwrap();
        assert!(tombstone.content.is_empty());
        assert!(reopened
            .list_retrievable_records(None, 10)
            .unwrap()
            .is_empty());
        assert!(reopened
            .accept_memory_proposal(acceptance_input(
                "proposal-private",
                "Private medical detail that must be erased.",
            ))
            .is_err());
    }

    #[test]
    fn paused_recall_is_distinct_from_recoverable_archive_across_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("paused-recall.db");
        let store = MemoryLifecycleStore::new(&path).unwrap();
        let accepted = store
            .accept_memory_proposal(acceptance_input(
                "proposal-paused",
                "A memory that can leave normal recall without being archived.",
            ))
            .unwrap();
        store
            .set_memory_retrieval_disposition(
                &accepted.record.memory_id,
                MemoryRetrievalDisposition::Paused,
                "user_reviewed_stop_recall",
            )
            .unwrap();
        assert!(!store
            .is_memory_retrievable(&accepted.record.memory_id)
            .unwrap());
        assert_eq!(store.count_archived_memory_retrieval_states().unwrap(), 0);
        assert!(store
            .list_archived_memory_retrieval_states(10)
            .unwrap()
            .is_empty());
        drop(store);

        let reopened = MemoryLifecycleStore::new(&path).unwrap();
        assert_eq!(
            reopened
                .memory_retrieval_state(&accepted.record.memory_id)
                .unwrap()
                .unwrap()
                .disposition,
            MemoryRetrievalDisposition::Paused
        );
        reopened
            .set_memory_retrieval_disposition(
                &accepted.record.memory_id,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .unwrap();
        assert_eq!(
            reopened.count_archived_memory_retrieval_states().unwrap(),
            1
        );
        reopened
            .set_memory_retrieval_disposition(
                &accepted.record.memory_id,
                MemoryRetrievalDisposition::Active,
                "user_reviewed_restore",
            )
            .unwrap();
        assert!(reopened
            .is_memory_retrievable(&accepted.record.memory_id)
            .unwrap());
    }

    #[test]
    fn accepted_memory_materializes_and_rollback_excludes_active_context() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.records",
            json!({
                "content": "User prefers execution-first agents.",
                "scope": "project",
                "category": "preference",
                "candidateKind": "preference",
                "riskLevel": "low",
                "sensitivity": "internal"
            }),
            "User accepted a memory proposal.",
            0.82,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        proposal.id = "proposal-memory-lifecycle-test".into();

        let accepted = store
            .accept_memory_proposal(
                MemoryLifecycleAcceptanceInput::from_memory_proposal(
                    &proposal,
                    "User prefers execution-first agents.".into(),
                )
                .unwrap(),
            )
            .unwrap();

        assert_eq!(accepted.record.proposal_id, proposal.id);
        assert_eq!(accepted.record.status, MemoryLifecycleStatus::Materialized);
        assert_eq!(
            accepted.record.materialization_status,
            MemoryMaterializationStatus::Materialized
        );
        assert!(accepted
            .materialized_view
            .as_ref()
            .unwrap()
            .active_memory_ids
            .contains(&accepted.record.memory_id));
        assert_eq!(
            store
                .list_active_records(Some(MemoryLifecycleScope::Project), 10)
                .unwrap()
                .len(),
            1
        );

        let rolled_back = store
            .rollback_memory_asset(&accepted.record.memory_id, "user", "not needed")
            .unwrap();

        assert_eq!(
            rolled_back.rollback_event.memory_id,
            accepted.record.memory_id
        );
        assert_eq!(rolled_back.record.status, MemoryLifecycleStatus::RolledBack);
        assert!(rolled_back.record.runtime_context_excluded_at.is_some());
        assert!(!rolled_back
            .materialized_view
            .active_memory_ids
            .contains(&accepted.record.memory_id));
        assert_eq!(
            store
                .list_active_records(Some(MemoryLifecycleScope::Project), 10)
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn high_risk_memory_rollback_requires_confirmation_blocker() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.identity.value",
            json!({
                "content": "User has an identity-level memory.",
                "scope": "global",
                "category": "boundary",
                "candidateKind": "identity_or_role",
                "riskLevel": "identity_value",
                "sensitivity": "sensitive"
            }),
            "User accepted a sensitive memory proposal.",
            0.82,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        proposal.id = "proposal-memory-high-risk-rollback-test".into();

        let accepted = store
            .accept_memory_proposal(
                MemoryLifecycleAcceptanceInput::from_memory_proposal(
                    &proposal,
                    "User has an identity-level memory.".into(),
                )
                .unwrap(),
            )
            .unwrap();

        let blocked = store
            .rollback_memory_asset(&accepted.record.memory_id, "user", "not needed")
            .unwrap_err();

        assert!(
            blocked
                .to_string()
                .contains("requires explicit confirmation"),
            "high-risk rollback must become a confirmation/blocker path: {blocked}"
        );
        assert!(store.is_memory_active(&accepted.record.memory_id).unwrap());
    }

    #[test]
    fn explicit_memory_uses_the_canonical_lifecycle_owner_and_is_reversible() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let first = store
            .commit_test_explicit_user_memory(explicit_input("message-1", "我早餐喜欢咖啡和面包"))
            .unwrap();
        let duplicate = store
            .commit_test_explicit_user_memory(explicit_input("message-2", "我早餐喜欢咖啡和面包"))
            .unwrap();

        assert!(first.newly_committed);
        assert!(!duplicate.newly_committed);
        assert_eq!(duplicate.memory_id, first.memory_id);
        assert_eq!(store.list_active_records(None, 10).unwrap().len(), 1);
        let record = store.get_record(&first.memory_id).unwrap().unwrap();
        assert_eq!(record.created_by, "current_authenticated_user_message");
        assert!(record.proposal_id.starts_with("explicit_memory:"));

        store
            .rollback_memory_asset(&first.memory_id, "user", "explicit memory undo")
            .unwrap();
        assert!(store.list_active_records(None, 10).unwrap().is_empty());

        let recreated = store
            .commit_test_explicit_user_memory(explicit_input("message-3", "我早餐喜欢咖啡和面包"))
            .unwrap();
        assert!(recreated.newly_committed);
        assert_ne!(recreated.memory_id, first.memory_id);
    }

    #[test]
    fn admission_outcomes_and_timestamps_distinguish_owner_from_each_admission() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let first_input = explicit_input("message-outcome-owner", "One admission outcome fact");
        let owner = store
            .commit_test_explicit_user_memory(first_input.clone())
            .unwrap();
        let replay = store.commit_test_explicit_user_memory(first_input).unwrap();
        let alias = store
            .commit_test_explicit_user_memory(explicit_input(
                "message-outcome-alias",
                "One admission outcome fact",
            ))
            .unwrap();

        assert_eq!(
            owner.admission_outcome,
            MemoryAdmissionOutcome::OwnerCreated
        );
        assert_eq!(
            replay.admission_outcome,
            MemoryAdmissionOutcome::ExactReplay
        );
        assert_eq!(alias.admission_outcome, MemoryAdmissionOutcome::AliasLinked);
        assert_eq!(owner.admission_at, owner.owner_accepted_at.unwrap());
        assert_eq!(replay.admission_at, owner.admission_at);
        assert_eq!(alias.owner_accepted_at, owner.owner_accepted_at);
        assert_eq!(alias.memory_id, owner.memory_id);
        assert!(!replay.newly_committed);
        assert!(!alias.newly_committed);
    }

    #[test]
    fn terminal_admission_replay_never_reactivates_but_new_admission_rebuilds_same_fact() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let original_input = explicit_input("message-terminal-original", "Rebuildable fact");
        let original = store
            .commit_test_explicit_user_memory(original_input.clone())
            .unwrap();
        store
            .rollback_memory_asset(&original.memory_id, "user", "remove old admission")
            .unwrap();

        let historical = store
            .commit_test_explicit_user_memory(original_input.clone())
            .unwrap();
        assert_eq!(
            historical.admission_outcome,
            MemoryAdmissionOutcome::TerminalHistorical
        );
        assert!(!historical.canonical_committed);
        assert!(!historical.undo_available);
        assert!(historical.outbox_event_id.is_none());
        assert!(store.list_active_records(None, 10).unwrap().is_empty());

        let rebuilt = store
            .commit_test_explicit_user_memory(explicit_input(
                "message-terminal-new-admission",
                "Rebuildable fact",
            ))
            .unwrap();
        assert_eq!(
            rebuilt.admission_outcome,
            MemoryAdmissionOutcome::OwnerCreated
        );
        assert_ne!(rebuilt.memory_id, original.memory_id);
        assert_eq!(store.list_active_records(None, 10).unwrap().len(), 1);

        let historical_again = store
            .commit_test_explicit_user_memory(original_input)
            .unwrap();
        assert_eq!(
            historical_again.admission_outcome,
            MemoryAdmissionOutcome::TerminalHistorical
        );
        assert_eq!(historical_again.memory_id, original.memory_id);
        assert_eq!(
            store.list_active_records(None, 10).unwrap()[0].memory_id,
            rebuilt.memory_id
        );
    }

    #[test]
    fn rolled_back_proposal_replay_is_historical_and_new_proposal_creates_a_new_owner() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let original_input = acceptance_input("proposal-terminal-original", "Proposal fact");
        let original = store
            .accept_memory_proposal(original_input.clone())
            .unwrap();
        store
            .rollback_memory_asset(&original.record.memory_id, "user", "remove proposal fact")
            .unwrap();

        let historical = store.accept_memory_proposal(original_input).unwrap();
        assert_eq!(
            historical.admission_outcome,
            MemoryAdmissionOutcome::TerminalHistorical
        );
        assert!(!historical.canonical_committed);
        assert!(historical.canonical_mutation.is_none());

        let rebuilt = store
            .accept_memory_proposal(acceptance_input(
                "proposal-terminal-new-admission",
                "Proposal fact",
            ))
            .unwrap();
        assert_eq!(
            rebuilt.admission_outcome,
            MemoryAdmissionOutcome::OwnerCreated
        );
        assert_ne!(rebuilt.record.memory_id, original.record.memory_id);
        assert_eq!(store.list_active_records(None, 10).unwrap().len(), 1);
    }

    #[test]
    fn direct_lane_rejects_boundary_identity_high_and_sensitive_memory() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let boundary = ExplicitMemoryWriteInput {
            source_task_session_id: "task-boundary".into(),
            source_run_id: "run-boundary".into(),
            source_message_id: "message-boundary".into(),
            source_message_digest: digest_label("I am the finance administrator."),
            authorized_candidate_id: "candidate:test:message-boundary".into(),
            fact: CanonicalMemoryFactDescriptor::new(
                "I am the finance administrator.",
                MemoryLifecycleScope::Global,
                MemoryLifecycleCategory::Boundary,
                MemoryLifecycleRiskLevel::Low,
                MemoryLifecycleSensitivity::Internal,
            )
            .unwrap(),
        };
        let identity = ExplicitMemoryWriteInput {
            source_task_session_id: "task-identity".into(),
            source_run_id: "run-identity".into(),
            source_message_id: "message-identity".into(),
            source_message_digest: digest_label("I am the finance administrator."),
            authorized_candidate_id: "candidate:test:message-identity".into(),
            fact: CanonicalMemoryFactDescriptor::from_candidate(
                "I am the finance administrator.",
                MemoryCandidateKind::IdentityOrRole,
                MemoryLifecycleScope::Global,
                MemoryLifecycleRiskLevel::Low,
                MemoryLifecycleSensitivity::Internal,
            )
            .unwrap(),
        };
        let mut high = explicit_input("message-high", "High-risk direct Memory");
        high.fact.risk_level = MemoryLifecycleRiskLevel::High;
        let mut sensitive = explicit_input("message-sensitive", "Sensitive direct Memory");
        sensitive.fact.sensitivity = MemoryLifecycleSensitivity::Sensitive;

        assert!(store.commit_test_explicit_user_memory(boundary).is_err());
        assert!(store.commit_test_explicit_user_memory(identity).is_err());
        assert!(store.commit_test_explicit_user_memory(high).is_err());
        assert!(store.commit_test_explicit_user_memory(sensitive).is_err());
        assert!(store.list_active_records(None, 10).unwrap().is_empty());
    }

    #[test]
    fn canonical_store_rejects_proof_rebound_to_different_message_candidate_or_fact() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        for mutation in ["message", "candidate", "fact"] {
            let mut input = explicit_input(
                &format!("message-proof-{mutation}"),
                "One exact proof-bound Memory fact",
            );
            let proof = PolicyMemoryAdmissionProof::test_fixture_for_explicit_input(&input);
            match mutation {
                "message" => input.source_message_digest = digest_label("different message"),
                "candidate" => input.authorized_candidate_id.push_str(":forged"),
                "fact" => input.fact.canonical_body.push_str(" changed"),
                _ => unreachable!(),
            }

            let error = store
                .commit_explicit_user_memory(input, proof)
                .expect_err("rebound admission proof must fail before canonical mutation");
            assert!(error
                .to_string()
                .contains("proof does not match canonical write input"));
        }
        assert!(store.list_active_records(None, 10).unwrap().is_empty());
    }

    #[test]
    fn concurrent_explicit_memory_writes_materialize_one_canonical_fact() {
        let store = Arc::new(MemoryLifecycleStore::new_in_memory().unwrap());
        let barrier = Arc::new(std::sync::Barrier::new(16));
        let handles = (0..16)
            .map(|index| {
                let store = store.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .commit_test_explicit_user_memory(explicit_input(
                            &format!("message-{index}"),
                            "我喜欢上午进行深度工作",
                        ))
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let receipts = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            receipts
                .iter()
                .filter(|receipt| receipt.newly_committed)
                .count(),
            1
        );
        assert!(receipts
            .iter()
            .all(|receipt| receipt.memory_id == receipts[0].memory_id));
        assert_eq!(store.list_active_records(None, 10).unwrap().len(), 1);
    }

    #[test]
    fn explicit_memory_and_outbox_roll_back_as_one_transaction() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        store.install_outbox_insert_failure_for_test().unwrap();

        let error = store
            .commit_test_explicit_user_memory(explicit_input("message-atomic", "低熵偏好：咖啡"))
            .expect_err("canonical record must not survive a failed outbox insert");

        assert!(error
            .to_string()
            .contains("injected memory lifecycle outbox"));
        assert!(store.list_active_records(None, 10).unwrap().is_empty());
        assert!(store
            .list_replayable_projection_deliveries(10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn rollback_tombstone_failure_preserves_the_active_canonical_memory() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let receipt = store
            .commit_test_explicit_user_memory(explicit_input("message-rollback", "需要保留的事实"))
            .unwrap();
        store.install_outbox_insert_failure_for_test().unwrap();

        store
            .rollback_memory_asset(&receipt.memory_id, "user", "forget")
            .expect_err("rollback and tombstone must share one transaction");

        assert!(store.is_memory_active(&receipt.memory_id).unwrap());
        assert_eq!(store.lifecycle_events(&receipt.memory_id).unwrap().len(), 1);
    }

    #[test]
    fn outbox_tokens_are_opaque_random_and_never_copy_low_entropy_content() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let sentinel = "咖啡";
        let first = store
            .commit_test_explicit_user_memory(explicit_input("message-token-1", sentinel))
            .unwrap();
        store
            .rollback_memory_asset(&first.memory_id, "user", "replace")
            .unwrap();
        let second = store
            .commit_test_explicit_user_memory(explicit_input("message-token-2", sentinel))
            .unwrap();

        let conn = store.conn.lock().unwrap();
        let first_digest: String = conn
            .query_row(
                "SELECT payload_digest FROM canonical_outbox_events WHERE event_id = ?1",
                [first.outbox_event_id.as_deref().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        let second_digest: String = conn
            .query_row(
                "SELECT payload_digest FROM canonical_outbox_events WHERE event_id = ?1",
                [second.outbox_event_id.as_deref().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        let persisted_metadata: String = conn
            .query_row(
                "SELECT COALESCE(group_concat(
                    event_id || aggregate_kind || aggregate_id || mutation_kind || payload_digest,
                    '|'
                 ), '') FROM canonical_outbox_events",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(conn);

        assert_ne!(first_digest, second_digest);
        assert!(!persisted_metadata.contains(sentinel));
        assert!(
            !serde_json::to_string(&store.list_replayable_projection_deliveries(100).unwrap())
                .unwrap()
                .contains(sentinel)
        );
    }

    #[test]
    fn duplicate_legacy_memory_without_outbox_is_repaired_as_pending_not_applied() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let first = store
            .commit_test_explicit_user_memory(explicit_input("message-legacy-1", "legacy fact"))
            .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "DELETE FROM canonical_outbox_deliveries WHERE event_id = ?1",
                [first.outbox_event_id.as_deref().unwrap()],
            )
            .unwrap();
            conn.execute(
                "DELETE FROM canonical_outbox_events WHERE event_id = ?1",
                [first.outbox_event_id.as_deref().unwrap()],
            )
            .unwrap();
        }

        let repaired = store
            .commit_test_explicit_user_memory(explicit_input("message-legacy-2", "legacy fact"))
            .unwrap();

        assert!(!repaired.newly_committed);
        assert_eq!(repaired.memory_id, first.memory_id);
        assert_eq!(repaired.projection_state, ProjectionDeliveryState::Pending);
        assert!(repaired.outbox_event_id.is_some());
        assert_eq!(
            store
                .projection_summary(repaired.outbox_event_id.as_deref().unwrap())
                .unwrap()
                .pending,
            2
        );
    }

    #[test]
    fn lifecycle_archive_is_canonical_before_derived_projection_and_restore_is_revisioned() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let receipt = store
            .commit_test_explicit_user_memory(explicit_input(
                "message-retrieval-owner",
                "lifecycle retrieval sentinel",
            ))
            .unwrap();
        assert!(store.is_memory_retrievable(&receipt.memory_id).unwrap());
        assert_eq!(store.list_retrievable_records(None, 10).unwrap().len(), 1);

        let archived = store
            .set_memory_retrieval_disposition(
                &receipt.memory_id,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .unwrap();
        let archived_event = archived.canonical_mutation.as_ref().unwrap();
        assert!(archived.changed);
        assert_eq!(archived.state.as_ref().unwrap().revision, 1);
        assert_eq!(
            store
                .projection_summary(&archived_event.event_id)
                .unwrap()
                .pending,
            1,
            "derived vector projection intentionally remains behind canonical archive"
        );
        assert!(!store.is_memory_retrievable(&receipt.memory_id).unwrap());
        assert!(store.list_retrievable_records(None, 10).unwrap().is_empty());
        assert!(
            store.is_memory_active(&receipt.memory_id).unwrap(),
            "archive changes retrieval eligibility, not canonical asset liveness"
        );

        let restored = store
            .set_memory_retrieval_disposition(
                &receipt.memory_id,
                MemoryRetrievalDisposition::Active,
                "user_reviewed_restore",
            )
            .unwrap();
        assert!(restored.changed);
        assert_eq!(restored.state.as_ref().unwrap().revision, 2);
        assert!(store.is_memory_retrievable(&receipt.memory_id).unwrap());
        assert_eq!(store.list_retrievable_records(None, 10).unwrap().len(), 1);
    }

    #[test]
    fn lifecycle_retrieval_mutation_and_owner_validation_are_one_transaction() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let first = store
            .commit_test_explicit_user_memory(explicit_input(
                "message-retrieval-batch-one",
                "first lifecycle retrieval owner",
            ))
            .unwrap();
        let second = store
            .commit_test_explicit_user_memory(explicit_input(
                "message-retrieval-batch-two",
                "second lifecycle retrieval owner",
            ))
            .unwrap();
        let owners = vec![first.memory_id.clone(), second.memory_id.clone()];
        let mut invalid = owners.clone();
        invalid.push("memory:missing-owner".into());

        assert!(store
            .set_memory_retrieval_dispositions(
                &invalid,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .is_err());
        assert!(owners
            .iter()
            .all(|owner| store.memory_retrieval_state(owner).unwrap().is_none()));

        store.install_outbox_insert_failure_for_test().unwrap();
        assert!(store
            .set_memory_retrieval_dispositions(
                &owners,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .is_err());
        assert!(owners
            .iter()
            .all(|owner| store.memory_retrieval_state(owner).unwrap().is_none()));
    }

    #[test]
    fn lifecycle_rollback_cannot_race_a_separate_retrieval_owner_check() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let receipt = store
            .commit_test_explicit_user_memory(explicit_input(
                "message-retrieval-rollback",
                "rollback serialization sentinel",
            ))
            .unwrap();
        store
            .rollback_memory_asset(&receipt.memory_id, "user", "remove canonical owner")
            .unwrap();

        let error = store
            .set_memory_retrieval_disposition(
                &receipt.memory_id,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .expect_err("terminal lifecycle owner must not acquire retrieval state");
        assert!(error.to_string().contains("owner proof"));
        assert!(store
            .memory_retrieval_state(&receipt.memory_id)
            .unwrap()
            .is_none());
        assert!(!store.is_memory_retrievable(&receipt.memory_id).unwrap());
    }

    #[test]
    fn archived_lifecycle_count_and_pages_exceed_legacy_five_hundred_cap() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let mut owners = Vec::with_capacity(501);
        for index in 0..501 {
            let receipt = store
                .commit_test_explicit_user_memory(explicit_input(
                    &format!("message-archive-page-{index}"),
                    &format!("archived lifecycle page fact {index}"),
                ))
                .unwrap();
            owners.push(receipt.memory_id);
        }
        for batch in owners.chunks(200) {
            store
                .set_memory_retrieval_dispositions(
                    batch,
                    MemoryRetrievalDisposition::Archived,
                    "user_reviewed_archive",
                )
                .unwrap();
        }

        assert_eq!(store.count_archived_memory_retrieval_states().unwrap(), 501);
        assert_eq!(
            store
                .list_archived_memory_retrieval_states_page(500, 0)
                .unwrap()
                .len(),
            500
        );
        assert_eq!(
            store
                .list_archived_memory_retrieval_states_page(10, 500)
                .unwrap()
                .len(),
            1
        );
    }
}
