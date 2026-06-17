use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

macro_rules! record_select_sql {
    ($suffix:expr) => {
        concat!(
            "SELECT memory_id, proposal_id, source_task_session_id, source_run_id, content, scope, category, risk_level, status, materialization_status, materialization_error_code, created_by, accepted_by, accepted_at, materialized_view_id, materialized_view_version, evidence_ids_json, confidence, conflict_ids_json, supersedes_memory_id, replacement_memory_id, rolled_back_by_event_id, runtime_context_excluded_at FROM memory_lifecycle_records ",
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

    fn from_str(value: &str) -> Self {
        match value {
            "workspace" => Self::Workspace,
            "conversation" => Self::Conversation,
            "project" => Self::Project,
            _ => Self::Global,
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

    fn from_str(value: &str) -> Self {
        match value {
            "fact" => Self::Fact,
            "workflow" => Self::Workflow,
            "correction" => Self::Correction,
            "boundary" => Self::Boundary,
            _ => Self::Preference,
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
            "medium" => Self::Medium,
            "high" => Self::High,
            "identity_value" => Self::IdentityValue,
            _ => Self::Low,
        }
    }
}

impl std::fmt::Display for MemoryLifecycleRiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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
    pub category: MemoryLifecycleCategory,
    pub risk_level: MemoryLifecycleRiskLevel,
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
    pub content: String,
    pub scope: MemoryLifecycleScope,
    pub category: MemoryLifecycleCategory,
    pub risk_level: MemoryLifecycleRiskLevel,
    pub created_by: String,
    pub accepted_by: String,
    pub evidence_ids: Vec<String>,
    pub confidence: String,
    pub conflict_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLifecycleAcceptanceReport {
    pub record: MemoryLifecycleRecord,
    pub materialized_view: MemoryMaterializedView,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRollbackReport {
    pub record: MemoryLifecycleRecord,
    pub rollback_event: MemoryRollbackEvent,
    pub materialized_view: MemoryMaterializedView,
}

pub struct MemoryLifecycleStore {
    conn: Mutex<Connection>,
}

impl MemoryLifecycleStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open memory lifecycle db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("failed to open in-memory memory lifecycle db")?;
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
            "CREATE TABLE IF NOT EXISTS memory_lifecycle_records (
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
            )",
            [],
        )?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_lifecycle_proposal ON memory_lifecycle_records(proposal_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_lifecycle_status_scope ON memory_lifecycle_records(status, materialization_status, scope)",
            [],
        )?;
        conn.execute(
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
        conn.execute(
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
        Ok(())
    }

    pub fn accept_memory_proposal(
        &self,
        input: MemoryLifecycleAcceptanceInput,
    ) -> Result<MemoryLifecycleAcceptanceReport> {
        let now = Utc::now();
        let memory_id = format!("memory:{}", Uuid::new_v4());
        let mut record = MemoryLifecycleRecord {
            memory_id,
            proposal_id: input.proposal_id,
            source_task_session_id: input.source_task_session_id,
            source_run_id: input.source_run_id,
            content: input.content,
            scope: input.scope,
            category: input.category,
            risk_level: input.risk_level,
            status: MemoryLifecycleStatus::Accepted,
            materialization_status: MemoryMaterializationStatus::Pending,
            materialization_error_code: None,
            created_by: input.created_by,
            accepted_by: Some(input.accepted_by),
            accepted_at: Some(now),
            materialized_view_id: None,
            materialized_view_version: None,
            evidence_ids: input.evidence_ids,
            confidence: input.confidence.parse::<f32>().unwrap_or(0.0),
            conflict_ids: input.conflict_ids,
            supersedes_memory_id: None,
            replacement_memory_id: None,
            rolled_back_by_event_id: None,
            runtime_context_excluded_at: None,
        };
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        insert_record_tx(&tx, &record, now, now)?;
        let view = rebuild_view_tx(&tx, Some(record.scope), Some(&record.memory_id), None)?;
        record.status = MemoryLifecycleStatus::Materialized;
        record.materialization_status = MemoryMaterializationStatus::Materialized;
        record.materialized_view_id = Some(view.materialized_view_id.clone());
        record.materialized_view_version = Some(view.version);
        update_record_tx(&tx, &record, Utc::now())?;
        tx.commit()?;
        Ok(MemoryLifecycleAcceptanceReport {
            record,
            materialized_view: view,
        })
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
            record_select_sql!("WHERE proposal_id = ?1"),
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
        tx.commit()?;
        Ok(MemoryRollbackReport {
            record,
            rollback_event,
            materialized_view: view,
        })
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
        self.list_records(scope, Some(MemoryLifecycleStatus::Materialized), limit, 0)
            .map(|records| {
                records
                    .into_iter()
                    .filter(|record| {
                        record.materialization_status == MemoryMaterializationStatus::Materialized
                            && record.runtime_context_excluded_at.is_none()
                    })
                    .collect()
            })
    }

    pub fn is_memory_active(&self, memory_id: &str) -> Result<bool> {
        Ok(self.get_record(memory_id)?.is_some_and(|record| {
            record.status.is_runtime_active()
                && record.materialization_status == MemoryMaterializationStatus::Materialized
                && record.runtime_context_excluded_at.is_none()
        }))
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

impl MemoryLifecycleAcceptanceInput {
    pub fn from_memory_proposal(proposal: &AgentProposal, content: String) -> Self {
        Self {
            proposal_id: proposal.id.clone(),
            source_task_session_id: proposal.source_detail.clone(),
            source_run_id: proposal.run_id.clone(),
            content,
            scope: scope_from_proposal(proposal),
            category: category_from_proposal(proposal),
            risk_level: risk_from_proposal(proposal),
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
        }
    }
}

fn scope_from_proposal(proposal: &AgentProposal) -> MemoryLifecycleScope {
    let after_scope = proposal
        .after
        .get("scope")
        .or_else(|| proposal.after.get("memoryScope"))
        .and_then(serde_json::Value::as_str);
    if let Some(scope) = after_scope {
        return MemoryLifecycleScope::from_str(scope);
    }
    let path = proposal.affected_path.to_ascii_lowercase();
    if path.contains("project") {
        MemoryLifecycleScope::Project
    } else if path.contains("workspace") {
        MemoryLifecycleScope::Workspace
    } else if path.contains("conversation") {
        MemoryLifecycleScope::Conversation
    } else {
        MemoryLifecycleScope::Global
    }
}

fn category_from_proposal(proposal: &AgentProposal) -> MemoryLifecycleCategory {
    if let Some(category) = proposal
        .after
        .get("category")
        .and_then(serde_json::Value::as_str)
    {
        return MemoryLifecycleCategory::from_str(category);
    }
    match proposal.proposal_type {
        ProposalType::PreferenceUpdate | ProposalType::MemoryWrite => {
            MemoryLifecycleCategory::Preference
        }
        ProposalType::MemoryArchive => MemoryLifecycleCategory::Correction,
        _ => MemoryLifecycleCategory::Fact,
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
        RiskLevel::High | RiskLevel::Critical => MemoryLifecycleRiskLevel::High,
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

fn insert_record_tx(
    tx: &rusqlite::Transaction<'_>,
    record: &MemoryLifecycleRecord,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> Result<()> {
    let params = record_params(record, created_at, updated_at);
    tx.execute(
        "INSERT INTO memory_lifecycle_records (memory_id, proposal_id, source_task_session_id, source_run_id, content, scope, category, risk_level, status, materialization_status, materialization_error_code, created_by, accepted_by, accepted_at, materialized_view_id, materialized_view_version, evidence_ids_json, confidence, conflict_ids_json, supersedes_memory_id, replacement_memory_id, rolled_back_by_event_id, runtime_context_excluded_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
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
        "UPDATE memory_lifecycle_records SET proposal_id = ?2, source_task_session_id = ?3, source_run_id = ?4, content = ?5, scope = ?6, category = ?7, risk_level = ?8, status = ?9, materialization_status = ?10, materialization_error_code = ?11, created_by = ?12, accepted_by = ?13, accepted_at = ?14, materialized_view_id = ?15, materialized_view_version = ?16, evidence_ids_json = ?17, confidence = ?18, conflict_ids_json = ?19, supersedes_memory_id = ?20, replacement_memory_id = ?21, rolled_back_by_event_id = ?22, runtime_context_excluded_at = ?23, updated_at = ?24 WHERE memory_id = ?1",
        rusqlite::params_from_iter(params.iter().map(|param| param.as_ref())),
    )?;
    Ok(())
}

fn record_params(
    record: &MemoryLifecycleRecord,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> [Box<dyn rusqlite::ToSql>; 25] {
    [
        Box::new(record.memory_id.clone()),
        Box::new(record.proposal_id.clone()),
        Box::new(record.source_task_session_id.clone()),
        Box::new(record.source_run_id.clone()),
        Box::new(record.content.clone()),
        Box::new(record.scope.to_string()),
        Box::new(record.category.to_string()),
        Box::new(record.risk_level.to_string()),
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
) -> [Box<dyn rusqlite::ToSql>; 24] {
    [
        Box::new(record.memory_id.clone()),
        Box::new(record.proposal_id.clone()),
        Box::new(record.source_task_session_id.clone()),
        Box::new(record.source_run_id.clone()),
        Box::new(record.content.clone()),
        Box::new(record.scope.to_string()),
        Box::new(record.category.to_string()),
        Box::new(record.risk_level.to_string()),
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

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryLifecycleRecord> {
    let evidence_json: String = row.get(16)?;
    let conflict_json: String = row.get(18)?;
    Ok(MemoryLifecycleRecord {
        memory_id: row.get(0)?,
        proposal_id: row.get(1)?,
        source_task_session_id: row.get(2)?,
        source_run_id: row.get(3)?,
        content: row.get(4)?,
        scope: MemoryLifecycleScope::from_str(&row.get::<_, String>(5)?),
        category: MemoryLifecycleCategory::from_str(&row.get::<_, String>(6)?),
        risk_level: MemoryLifecycleRiskLevel::from_str(&row.get::<_, String>(7)?),
        status: MemoryLifecycleStatus::from_str(&row.get::<_, String>(8)?),
        materialization_status: MemoryMaterializationStatus::from_str(&row.get::<_, String>(9)?),
        materialization_error_code: row.get(10)?,
        created_by: row.get(11)?,
        accepted_by: row.get(12)?,
        accepted_at: parse_optional_time(row.get::<_, Option<String>>(13)?),
        materialized_view_id: row.get(14)?,
        materialized_view_version: row.get(15)?,
        evidence_ids: serde_json::from_str(&evidence_json).unwrap_or_default(),
        confidence: row.get(17)?,
        conflict_ids: serde_json::from_str(&conflict_json).unwrap_or_default(),
        supersedes_memory_id: row.get(19)?,
        replacement_memory_id: row.get(20)?,
        rolled_back_by_event_id: row.get(21)?,
        runtime_context_excluded_at: parse_optional_time(row.get::<_, Option<String>>(22)?),
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

    #[test]
    fn accepted_memory_materializes_and_rollback_excludes_active_context() {
        let store = MemoryLifecycleStore::new_in_memory().unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.records",
            json!({
                "content": "User prefers execution-first agents.",
                "scope": "project",
                "category": "preference"
            }),
            "User accepted a memory proposal.",
            0.82,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        proposal.id = "proposal-memory-lifecycle-test".into();

        let accepted = store
            .accept_memory_proposal(MemoryLifecycleAcceptanceInput::from_memory_proposal(
                &proposal,
                "User prefers execution-first agents.".into(),
            ))
            .unwrap();

        assert_eq!(accepted.record.proposal_id, proposal.id);
        assert_eq!(accepted.record.status, MemoryLifecycleStatus::Materialized);
        assert_eq!(
            accepted.record.materialization_status,
            MemoryMaterializationStatus::Materialized
        );
        assert!(accepted
            .materialized_view
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
                "category": "preference"
            }),
            "User accepted a sensitive memory proposal.",
            0.82,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        proposal.id = "proposal-memory-high-risk-rollback-test".into();

        let accepted = store
            .accept_memory_proposal(MemoryLifecycleAcceptanceInput::from_memory_proposal(
                &proposal,
                "User has an identity-level memory.".into(),
            ))
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
}
