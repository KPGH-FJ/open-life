use crate::agent::types::{AgentProposal, ProposalType, RiskLevel};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, types::Type, Connection, Row};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    Preference,
    Goal,
    State,
    Capability,
    Memory,
    Policy,
    RuntimeBehavior,
    ProposalOutcome,
    Contradiction,
    Other,
}

impl std::fmt::Display for EvidenceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvidenceType::Preference => write!(f, "preference"),
            EvidenceType::Goal => write!(f, "goal"),
            EvidenceType::State => write!(f, "state"),
            EvidenceType::Capability => write!(f, "capability"),
            EvidenceType::Memory => write!(f, "memory"),
            EvidenceType::Policy => write!(f, "policy"),
            EvidenceType::RuntimeBehavior => write!(f, "runtime_behavior"),
            EvidenceType::ProposalOutcome => write!(f, "proposal_outcome"),
            EvidenceType::Contradiction => write!(f, "contradiction"),
            EvidenceType::Other => write!(f, "other"),
        }
    }
}

impl EvidenceType {
    fn from_str(value: &str) -> Self {
        match value {
            "preference" => EvidenceType::Preference,
            "goal" => EvidenceType::Goal,
            "state" => EvidenceType::State,
            "capability" => EvidenceType::Capability,
            "memory" => EvidenceType::Memory,
            "policy" => EvidenceType::Policy,
            "runtime_behavior" => EvidenceType::RuntimeBehavior,
            "proposal_outcome" => EvidenceType::ProposalOutcome,
            "contradiction" => EvidenceType::Contradiction,
            _ => EvidenceType::Other,
        }
    }

    fn from_proposal_type(proposal_type: ProposalType) -> Self {
        match proposal_type {
            ProposalType::GoalUpdate => EvidenceType::Goal,
            ProposalType::StateUpdate => EvidenceType::State,
            ProposalType::PreferenceUpdate => EvidenceType::Preference,
            ProposalType::CapabilityUpdate => EvidenceType::Capability,
            ProposalType::MemoryWrite | ProposalType::MemoryArchive => EvidenceType::Memory,
            ProposalType::ToolPermission
            | ProposalType::ModelPolicyChange
            | ProposalType::DataExport => EvidenceType::Policy,
            ProposalType::ScheduledTask | ProposalType::ExternalWriteAction => {
                EvidenceType::RuntimeBehavior
            }
            ProposalType::Unsupported | ProposalType::LifeModelUpdate => EvidenceType::Other,
            ProposalType::ScheduleCheckin => EvidenceType::State,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePrivacyLevel {
    Public,
    Internal,
    Sensitive,
    StrictlyLocal,
}

impl std::fmt::Display for EvidencePrivacyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvidencePrivacyLevel::Public => write!(f, "public"),
            EvidencePrivacyLevel::Internal => write!(f, "internal"),
            EvidencePrivacyLevel::Sensitive => write!(f, "sensitive"),
            EvidencePrivacyLevel::StrictlyLocal => write!(f, "strictly_local"),
        }
    }
}

impl EvidencePrivacyLevel {
    fn from_str(value: &str) -> Self {
        match value {
            "public" => EvidencePrivacyLevel::Public,
            "sensitive" => EvidencePrivacyLevel::Sensitive,
            "strictly_local" => EvidencePrivacyLevel::StrictlyLocal,
            _ => EvidencePrivacyLevel::Internal,
        }
    }

    fn from_risk(risk: RiskLevel) -> Self {
        match risk {
            RiskLevel::Low => EvidencePrivacyLevel::Internal,
            RiskLevel::Medium => EvidencePrivacyLevel::Sensitive,
            RiskLevel::High | RiskLevel::Critical => EvidencePrivacyLevel::StrictlyLocal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Candidate,
    Active,
    Weakened,
    Archived,
    Contradicted,
    Tombstoned,
}

impl std::fmt::Display for EvidenceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvidenceStatus::Candidate => write!(f, "candidate"),
            EvidenceStatus::Active => write!(f, "active"),
            EvidenceStatus::Weakened => write!(f, "weakened"),
            EvidenceStatus::Archived => write!(f, "archived"),
            EvidenceStatus::Contradicted => write!(f, "contradicted"),
            EvidenceStatus::Tombstoned => write!(f, "tombstoned"),
        }
    }
}

impl EvidenceStatus {
    fn from_str(value: &str) -> Self {
        match value {
            "active" => EvidenceStatus::Active,
            "weakened" => EvidenceStatus::Weakened,
            "archived" => EvidenceStatus::Archived,
            "contradicted" => EvidenceStatus::Contradicted,
            "tombstoned" => EvidenceStatus::Tombstoned,
            _ => EvidenceStatus::Candidate,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSourceType {
    ChatMessage,
    MemoryRecord,
    VectorChunk,
    Proposal,
    WorkRun,
    RunMetadata,
    Feedback,
    UserEdit,
    Other,
}

impl std::fmt::Display for EvidenceSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvidenceSourceType::ChatMessage => write!(f, "chat_message"),
            EvidenceSourceType::MemoryRecord => write!(f, "memory_record"),
            EvidenceSourceType::VectorChunk => write!(f, "vector_chunk"),
            EvidenceSourceType::Proposal => write!(f, "proposal"),
            EvidenceSourceType::WorkRun => write!(f, "work_run"),
            EvidenceSourceType::RunMetadata => write!(f, "run_metadata"),
            EvidenceSourceType::Feedback => write!(f, "feedback"),
            EvidenceSourceType::UserEdit => write!(f, "user_edit"),
            EvidenceSourceType::Other => write!(f, "other"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSourceRef {
    pub source_type: EvidenceSourceType,
    pub source_id: String,
    pub source_detail: Option<String>,
    pub digest: String,
    pub observed_at: DateTime<Utc>,
}

impl EvidenceSourceRef {
    pub fn from_payload(
        source_type: EvidenceSourceType,
        source_id: impl Into<String>,
        source_detail: Option<&str>,
        payload: &str,
    ) -> Self {
        Self {
            source_type,
            source_id: source_id.into(),
            source_detail: source_detail.map(str::to_string),
            digest: sha256_hex(payload.as_bytes()),
            observed_at: Utc::now(),
        }
    }

    pub fn from_digest(
        source_type: EvidenceSourceType,
        source_id: impl Into<String>,
        source_detail: Option<&str>,
        digest: impl Into<String>,
    ) -> Self {
        Self {
            source_type,
            source_id: source_id.into(),
            source_detail: source_detail.map(str::to_string),
            digest: digest.into(),
            observed_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceTombstone {
    pub reason_digest: String,
    pub prevent_relearning_digest: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRecord {
    pub id: String,
    pub evidence_type: EvidenceType,
    pub affected_path: String,
    pub summary: Option<String>,
    pub confidence: f32,
    pub risk_level: RiskLevel,
    pub privacy_level: EvidencePrivacyLevel,
    pub status: EvidenceStatus,
    pub source_refs: Vec<EvidenceSourceRef>,
    pub support_count: u32,
    pub opposing_refs: Vec<String>,
    pub linked_proposal_ids: Vec<String>,
    pub linked_work_run_ids: Vec<String>,
    pub run_metadata: Value,
    pub tombstone: Option<EvidenceTombstone>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_observed_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceDraft {
    pub evidence_type: EvidenceType,
    pub affected_path: String,
    pub summary: Option<String>,
    pub confidence: f32,
    pub risk_level: RiskLevel,
    pub privacy_level: EvidencePrivacyLevel,
    pub source_refs: Vec<EvidenceSourceRef>,
    pub opposing_refs: Vec<String>,
    pub linked_proposal_ids: Vec<String>,
    pub linked_work_run_ids: Vec<String>,
    pub run_metadata: Value,
}

impl EvidenceDraft {
    pub fn new(
        evidence_type: EvidenceType,
        affected_path: impl Into<String>,
        confidence: f32,
        risk_level: RiskLevel,
        privacy_level: EvidencePrivacyLevel,
    ) -> Self {
        Self {
            evidence_type,
            affected_path: affected_path.into(),
            summary: None,
            confidence: confidence.clamp(0.0, 1.0),
            risk_level,
            privacy_level,
            source_refs: Vec::new(),
            opposing_refs: Vec::new(),
            linked_proposal_ids: Vec::new(),
            linked_work_run_ids: Vec::new(),
            run_metadata: serde_json::json!({}),
        }
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    pub fn with_source_ref(mut self, source_ref: EvidenceSourceRef) -> Self {
        self.source_refs.push(source_ref);
        self
    }

    pub fn with_linked_proposal(mut self, proposal_id: impl Into<String>) -> Self {
        append_unique(&mut self.linked_proposal_ids, proposal_id.into());
        self
    }

    pub fn with_linked_work_run(mut self, run_id: impl Into<String>) -> Self {
        append_unique(&mut self.linked_work_run_ids, run_id.into());
        self
    }

    pub fn from_proposal_candidate(proposal: &AgentProposal) -> Self {
        let payload_digest_input = serde_json::json!({
            "proposal_type": proposal.proposal_type.to_string(),
            "affected_path": proposal.affected_path,
            "after": proposal.after,
            "reason": proposal.reason,
            "source": proposal.source.to_string(),
            "source_detail": proposal.source_detail,
        })
        .to_string();
        let source_ref = EvidenceSourceRef::from_payload(
            EvidenceSourceType::Proposal,
            &proposal.id,
            proposal.source_detail.as_deref(),
            &payload_digest_input,
        );

        EvidenceDraft::new(
            EvidenceType::from_proposal_type(proposal.proposal_type),
            proposal.affected_path.clone(),
            proposal.confidence,
            proposal.risk_level,
            EvidencePrivacyLevel::from_risk(proposal.risk_level),
        )
        .with_summary(format!(
            "{} candidate from {}",
            proposal.proposal_type, proposal.source
        ))
        .with_source_ref(source_ref)
        .with_linked_proposal(proposal.id.clone())
    }
}

#[derive(Debug, Clone, Default)]
pub struct EvidenceQuery {
    pub affected_path: Option<String>,
    pub evidence_type: Option<EvidenceType>,
    pub status: Option<EvidenceStatus>,
    pub privacy_level: Option<EvidencePrivacyLevel>,
    pub linked_proposal_id: Option<String>,
    pub linked_work_run_id: Option<String>,
    pub limit: Option<usize>,
}

pub struct EvidenceStore {
    conn: Mutex<Connection>,
}

impl EvidenceStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open evidence db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory evidence db")?;
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
            "evidence_store",
            &["evidence_records"],
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn unavailable_sentinel() -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(crate::sqlite_migration::unavailable_read_only_sentinel(
                "evidence_store",
            )?),
        })
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS evidence_records (
                id TEXT PRIMARY KEY,
                evidence_type TEXT NOT NULL,
                affected_path TEXT NOT NULL,
                summary TEXT,
                confidence REAL NOT NULL,
                risk_level TEXT NOT NULL,
                privacy_level TEXT NOT NULL,
                status TEXT NOT NULL,
                source_refs_json TEXT NOT NULL,
                support_count INTEGER NOT NULL,
                opposing_refs_json TEXT NOT NULL,
                linked_proposal_ids_json TEXT NOT NULL,
                linked_work_run_ids_json TEXT NOT NULL,
                run_metadata_json TEXT NOT NULL,
                tombstone_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_observed_at TEXT NOT NULL,
                archived_at TEXT
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_evidence_path_status ON evidence_records(affected_path, status, last_observed_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_evidence_type_status ON evidence_records(evidence_type, status, last_observed_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_evidence_privacy ON evidence_records(privacy_level, status)",
            [],
        )?;
        Ok(())
    }

    pub fn create_evidence(&self, draft: EvidenceDraft) -> Result<EvidenceRecord> {
        let now = Utc::now();
        let last_observed_at = draft
            .source_refs
            .iter()
            .map(|source| source.observed_at)
            .max()
            .unwrap_or(now);
        let record = EvidenceRecord {
            id: format!("ev_{}", Uuid::new_v4().simple()),
            evidence_type: draft.evidence_type,
            affected_path: draft.affected_path,
            summary: draft.summary,
            confidence: draft.confidence.clamp(0.0, 1.0),
            risk_level: draft.risk_level,
            privacy_level: draft.privacy_level,
            status: EvidenceStatus::Candidate,
            support_count: draft.source_refs.len() as u32,
            source_refs: draft.source_refs,
            opposing_refs: draft.opposing_refs,
            linked_proposal_ids: draft.linked_proposal_ids,
            linked_work_run_ids: draft.linked_work_run_ids,
            run_metadata: draft.run_metadata,
            tombstone: None,
            created_at: now,
            updated_at: now,
            last_observed_at,
            archived_at: None,
        };

        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO evidence_records (
                id, evidence_type, affected_path, summary, confidence, risk_level,
                privacy_level, status, source_refs_json, support_count,
                opposing_refs_json, linked_proposal_ids_json, linked_work_run_ids_json,
                run_metadata_json, tombstone_json, created_at, updated_at,
                last_observed_at, archived_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                record.id,
                record.evidence_type.to_string(),
                record.affected_path,
                record.summary,
                record.confidence,
                record.risk_level.to_string(),
                record.privacy_level.to_string(),
                record.status.to_string(),
                serde_json::to_string(&record.source_refs)?,
                record.support_count,
                serde_json::to_string(&record.opposing_refs)?,
                serde_json::to_string(&record.linked_proposal_ids)?,
                serde_json::to_string(&record.linked_work_run_ids)?,
                serde_json::to_string(&record.run_metadata)?,
                record
                    .tombstone
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                record.created_at.to_rfc3339(),
                record.updated_at.to_rfc3339(),
                record.last_observed_at.to_rfc3339(),
                record.archived_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(record)
    }

    pub fn get_evidence(&self, id: &str) -> Result<Option<EvidenceRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM evidence_records WHERE id = ?1",
            Self::columns()
        ))?;
        match stmt.query_row([id], Self::row_to_record) {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn query(&self, query: EvidenceQuery) -> Result<Vec<EvidenceRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(path) = &query.affected_path {
            conditions.push("affected_path = ?".to_string());
            params.push(Box::new(path.clone()));
        }
        if let Some(evidence_type) = query.evidence_type {
            conditions.push("evidence_type = ?".to_string());
            params.push(Box::new(evidence_type.to_string()));
        }
        if let Some(status) = query.status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(status.to_string()));
        }
        if let Some(privacy_level) = query.privacy_level {
            conditions.push("privacy_level = ?".to_string());
            params.push(Box::new(privacy_level.to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        let sql = format!(
            "SELECT {} FROM evidence_records {} ORDER BY last_observed_at DESC, updated_at DESC",
            Self::columns(),
            where_clause
        );
        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), Self::row_to_record)?;
        let mut records = rows.collect::<std::result::Result<Vec<_>, _>>()?;

        if let Some(proposal_id) = query.linked_proposal_id.as_deref() {
            records.retain(|record| {
                record
                    .linked_proposal_ids
                    .iter()
                    .any(|linked_id| linked_id == proposal_id)
            });
        }
        if let Some(run_id) = query.linked_work_run_id.as_deref() {
            records.retain(|record| {
                record
                    .linked_work_run_ids
                    .iter()
                    .any(|linked_id| linked_id == run_id)
            });
        }
        if let Some(limit) = query.limit {
            records.truncate(limit);
        }
        Ok(records)
    }

    pub fn weaken_evidence(
        &self,
        id: &str,
        confidence_delta: f32,
        reason: Option<&str>,
    ) -> Result<EvidenceRecord> {
        let mut record = self.require_evidence(id)?;
        record.confidence = (record.confidence - confidence_delta.max(0.0)).max(0.0);
        record.status = EvidenceStatus::Weakened;
        if let Some(reason) = reason {
            merge_metadata(
                &mut record.run_metadata,
                serde_json::json!({
                    "last_weaken_reason_digest": sha256_hex(reason.as_bytes())
                }),
            );
        }
        self.save_record(record)
    }

    pub fn archive_evidence(&self, id: &str, reason: Option<&str>) -> Result<EvidenceRecord> {
        let mut record = self.require_evidence(id)?;
        let now = Utc::now();
        record.status = EvidenceStatus::Archived;
        record.archived_at = Some(now);
        if let Some(reason) = reason {
            merge_metadata(
                &mut record.run_metadata,
                serde_json::json!({
                    "last_archive_reason_digest": sha256_hex(reason.as_bytes())
                }),
            );
        }
        self.save_record(record)
    }

    pub fn contradict_evidence(
        &self,
        id: &str,
        opposing_ref: impl Into<String>,
        reason: Option<&str>,
    ) -> Result<EvidenceRecord> {
        let mut record = self.require_evidence(id)?;
        append_unique(&mut record.opposing_refs, opposing_ref.into());
        record.status = EvidenceStatus::Contradicted;
        if let Some(reason) = reason {
            merge_metadata(
                &mut record.run_metadata,
                serde_json::json!({
                    "last_contradiction_reason_digest": sha256_hex(reason.as_bytes())
                }),
            );
        }
        self.save_record(record)
    }

    pub fn tombstone_evidence(
        &self,
        id: &str,
        reason: impl Into<String>,
        prevent_relearning_digest: Option<&str>,
    ) -> Result<EvidenceRecord> {
        let mut record = self.require_evidence(id)?;
        record.status = EvidenceStatus::Tombstoned;
        record.tombstone = Some(EvidenceTombstone {
            reason_digest: sha256_hex(reason.into().as_bytes()),
            prevent_relearning_digest: prevent_relearning_digest.map(str::to_string),
            created_at: Utc::now(),
        });
        self.save_record(record)
    }

    pub fn link_proposal(
        &self,
        id: &str,
        proposal_id: impl Into<String>,
    ) -> Result<EvidenceRecord> {
        let mut record = self.require_evidence(id)?;
        append_unique(&mut record.linked_proposal_ids, proposal_id.into());
        self.save_record(record)
    }

    pub fn link_work_run(&self, id: &str, run_id: impl Into<String>) -> Result<EvidenceRecord> {
        let mut record = self.require_evidence(id)?;
        append_unique(&mut record.linked_work_run_ids, run_id.into());
        self.save_record(record)
    }

    pub fn merge_run_metadata(&self, id: &str, metadata: Value) -> Result<EvidenceRecord> {
        let mut record = self.require_evidence(id)?;
        merge_metadata(&mut record.run_metadata, metadata);
        self.save_record(record)
    }

    fn require_evidence(&self, id: &str) -> Result<EvidenceRecord> {
        self.get_evidence(id)?
            .ok_or_else(|| anyhow::anyhow!("evidence record not found: {}", id))
    }

    fn save_record(&self, mut record: EvidenceRecord) -> Result<EvidenceRecord> {
        record.updated_at = Utc::now();
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "UPDATE evidence_records SET
                evidence_type = ?2,
                affected_path = ?3,
                summary = ?4,
                confidence = ?5,
                risk_level = ?6,
                privacy_level = ?7,
                status = ?8,
                source_refs_json = ?9,
                support_count = ?10,
                opposing_refs_json = ?11,
                linked_proposal_ids_json = ?12,
                linked_work_run_ids_json = ?13,
                run_metadata_json = ?14,
                tombstone_json = ?15,
                updated_at = ?16,
                last_observed_at = ?17,
                archived_at = ?18
            WHERE id = ?1",
            params![
                record.id,
                record.evidence_type.to_string(),
                record.affected_path,
                record.summary,
                record.confidence,
                record.risk_level.to_string(),
                record.privacy_level.to_string(),
                record.status.to_string(),
                serde_json::to_string(&record.source_refs)?,
                record.support_count,
                serde_json::to_string(&record.opposing_refs)?,
                serde_json::to_string(&record.linked_proposal_ids)?,
                serde_json::to_string(&record.linked_work_run_ids)?,
                serde_json::to_string(&record.run_metadata)?,
                record
                    .tombstone
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                record.updated_at.to_rfc3339(),
                record.last_observed_at.to_rfc3339(),
                record.archived_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(record)
    }

    fn columns() -> &'static str {
        "id, evidence_type, affected_path, summary, confidence, risk_level,
         privacy_level, status, source_refs_json, support_count,
         opposing_refs_json, linked_proposal_ids_json, linked_work_run_ids_json,
         run_metadata_json, tombstone_json, created_at, updated_at,
         last_observed_at, archived_at"
    }

    fn row_to_record(row: &Row<'_>) -> rusqlite::Result<EvidenceRecord> {
        let evidence_type: String = row.get(1)?;
        let risk_level: String = row.get(5)?;
        let privacy_level: String = row.get(6)?;
        let status: String = row.get(7)?;
        Ok(EvidenceRecord {
            id: row.get(0)?,
            evidence_type: EvidenceType::from_str(&evidence_type),
            affected_path: row.get(2)?,
            summary: row.get(3)?,
            confidence: row.get(4)?,
            risk_level: risk_level_from_str(&risk_level),
            privacy_level: EvidencePrivacyLevel::from_str(&privacy_level),
            status: EvidenceStatus::from_str(&status),
            source_refs: json_column(row, 8)?,
            support_count: row.get::<_, i64>(9)?.max(0) as u32,
            opposing_refs: json_column(row, 10)?,
            linked_proposal_ids: json_column(row, 11)?,
            linked_work_run_ids: json_column(row, 12)?,
            run_metadata: json_column(row, 13)?,
            tombstone: optional_json_column(row, 14)?,
            created_at: datetime_column(row, 15)?,
            updated_at: datetime_column(row, 16)?,
            last_observed_at: datetime_column(row, 17)?,
            archived_at: optional_datetime_column(row, 18)?,
        })
    }
}

fn append_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn merge_metadata(target: &mut Value, patch: Value) {
    if !target.is_object() {
        *target = serde_json::json!({});
    }
    let Some(target_obj) = target.as_object_mut() else {
        return;
    };
    if let Some(patch_obj) = patch.as_object() {
        for (key, value) in patch_obj {
            target_obj.insert(key.clone(), value.clone());
        }
    }
}

fn risk_level_from_str(value: &str) -> RiskLevel {
    match value {
        "low" => RiskLevel::Low,
        "high" => RiskLevel::High,
        "critical" => RiskLevel::Critical,
        _ => RiskLevel::Medium,
    }
}

fn json_column<T: DeserializeOwned>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
    let raw: String = row.get(index)?;
    serde_json::from_str(&raw)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(e)))
}

fn optional_json_column<T: DeserializeOwned>(
    row: &Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<T>> {
    let raw: Option<String> = row.get(index)?;
    raw.map(|value| {
        serde_json::from_str(&value)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(e)))
    })
    .transpose()
}

fn datetime_column(row: &Row<'_>, index: usize) -> rusqlite::Result<DateTime<Utc>> {
    let raw: String = row.get(index)?;
    parse_datetime(&raw, index)
}

fn optional_datetime_column(
    row: &Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<DateTime<Utc>>> {
    let raw: Option<String> = row.get(index)?;
    raw.map(|value| parse_datetime(&value, index)).transpose()
}

fn parse_datetime(value: &str, index: usize) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(e)))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let hash = digest(&SHA256, bytes);
    let bytes = hash.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}
