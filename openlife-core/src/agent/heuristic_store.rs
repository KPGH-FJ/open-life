use crate::agent::evidence_store::EvidencePrivacyLevel;
use crate::agent::policy_store::{
    BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING, BUILTIN_HEURISTIC_REJECTED_REMINDER_DELAY,
};
use crate::agent::types::RiskLevel;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

const DEFAULT_ACTIVE_CAP_PER_DOMAIN: usize = 5;
const DEFAULT_ACTIVE_OR_TRIAL_CAP_PER_DOMAIN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeuristicLifecycleStatus {
    Candidate,
    Trial,
    Active,
    Weakened,
    Archived,
    Rejected,
}

impl std::fmt::Display for HeuristicLifecycleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeuristicLifecycleStatus::Candidate => write!(f, "candidate"),
            HeuristicLifecycleStatus::Trial => write!(f, "trial"),
            HeuristicLifecycleStatus::Active => write!(f, "active"),
            HeuristicLifecycleStatus::Weakened => write!(f, "weakened"),
            HeuristicLifecycleStatus::Archived => write!(f, "archived"),
            HeuristicLifecycleStatus::Rejected => write!(f, "rejected"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeuristicValidationState {
    Untested,
    Pending,
    Passed,
    Failed,
    NeedsReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HeuristicActivationAuthority {
    AcceptedProposal(String),
    SeededBuiltInPolicy(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeuristicUsageMetadata {
    pub usage_count: u64,
    pub last_used_at: Option<DateTime<Utc>>,
    pub task_kinds: Vec<String>,
    pub last_task_kind: Option<String>,
    pub last_task_metadata: Value,
}

impl Default for HeuristicUsageMetadata {
    fn default() -> Self {
        Self {
            usage_count: 0,
            last_used_at: None,
            task_kinds: Vec::new(),
            last_task_kind: None,
            last_task_metadata: serde_json::json!({}),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeuristicRecord {
    pub id: String,
    pub domain: String,
    pub trigger: String,
    pub conditions: Vec<String>,
    pub guidance: String,
    pub priority: i32,
    pub risk_level: RiskLevel,
    pub privacy_level: EvidencePrivacyLevel,
    pub status: HeuristicLifecycleStatus,
    pub evidence_refs: Vec<String>,
    pub opposing_evidence_refs: Vec<String>,
    pub validation_state: HeuristicValidationState,
    pub source_proposal_id: Option<String>,
    pub version: u32,
    pub usage: HeuristicUsageMetadata,
    pub activation_authority: Option<HeuristicActivationAuthority>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeuristicDraft {
    pub stable_id: Option<String>,
    pub domain: String,
    pub trigger: String,
    pub conditions: Vec<String>,
    pub guidance: String,
    pub priority: i32,
    pub risk_level: RiskLevel,
    pub privacy_level: EvidencePrivacyLevel,
    pub evidence_refs: Vec<String>,
    pub opposing_evidence_refs: Vec<String>,
    pub validation_state: HeuristicValidationState,
    pub source_proposal_id: Option<String>,
}

impl HeuristicDraft {
    pub fn new(
        domain: impl Into<String>,
        trigger: impl Into<String>,
        conditions: Vec<String>,
        guidance: impl Into<String>,
        priority: i32,
        risk_level: RiskLevel,
        privacy_level: EvidencePrivacyLevel,
    ) -> Self {
        Self {
            stable_id: None,
            domain: domain.into(),
            trigger: trigger.into(),
            conditions,
            guidance: guidance.into(),
            priority,
            risk_level,
            privacy_level,
            evidence_refs: Vec::new(),
            opposing_evidence_refs: Vec::new(),
            validation_state: HeuristicValidationState::Untested,
            source_proposal_id: None,
        }
    }

    pub fn with_trigger(mut self, trigger: impl Into<String>) -> Self {
        self.trigger = trigger.into();
        self
    }

    pub fn with_stable_id(mut self, stable_id: impl Into<String>) -> Self {
        self.stable_id = Some(stable_id.into());
        self
    }

    pub fn with_evidence_ref(mut self, evidence_ref: impl Into<String>) -> Self {
        append_unique(&mut self.evidence_refs, evidence_ref.into());
        self
    }

    pub fn with_opposing_evidence_ref(mut self, evidence_ref: impl Into<String>) -> Self {
        append_unique(&mut self.opposing_evidence_refs, evidence_ref.into());
        self
    }

    pub fn with_source_proposal(mut self, proposal_id: impl Into<String>) -> Self {
        self.source_proposal_id = Some(proposal_id.into());
        self
    }

    pub fn with_validation_state(mut self, validation_state: HeuristicValidationState) -> Self {
        self.validation_state = validation_state;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct HeuristicQuery {
    pub domain: Option<String>,
    pub status: Option<HeuristicLifecycleStatus>,
    pub task_kind: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeuristicLineage {
    pub heuristic_id: String,
    pub evidence_refs: Vec<String>,
    pub opposing_evidence_refs: Vec<String>,
    pub source_proposal_id: Option<String>,
    pub activation_authority: Option<HeuristicActivationAuthority>,
    pub version: u32,
    pub validation_state: HeuristicValidationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainCapDiagnostic {
    pub domain: String,
    pub active_count: usize,
    pub active_cap: usize,
    pub active_cap_exceeded: bool,
    pub active_or_trial_count: usize,
    pub active_or_trial_cap: usize,
    pub active_or_trial_cap_exceeded: bool,
}

pub struct HeuristicStore {
    conn: Mutex<Connection>,
    active_cap_per_domain: usize,
    active_or_trial_cap_per_domain: usize,
}

impl HeuristicStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open heuristic db at {:?}", db_path))?;
        let store = Self::from_connection(conn);
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory heuristic db")?;
        let store = Self::from_connection(conn);
        store.init_tables()?;
        Ok(store)
    }

    fn from_connection(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
            active_cap_per_domain: DEFAULT_ACTIVE_CAP_PER_DOMAIN,
            active_or_trial_cap_per_domain: DEFAULT_ACTIVE_OR_TRIAL_CAP_PER_DOMAIN,
        }
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS heuristics (
                id TEXT PRIMARY KEY,
                domain TEXT NOT NULL,
                status TEXT NOT NULL,
                risk_level TEXT NOT NULL,
                priority INTEGER NOT NULL,
                source_proposal_id TEXT,
                record_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_heuristics_domain_status ON heuristics(domain, status, priority DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_heuristics_source_proposal ON heuristics(source_proposal_id)",
            [],
        )?;
        Ok(())
    }

    pub fn create_heuristic(&self, draft: HeuristicDraft) -> Result<HeuristicRecord> {
        let now = Utc::now();
        let record = HeuristicRecord {
            id: draft
                .stable_id
                .unwrap_or_else(|| format!("hr_{}", Uuid::new_v4().simple())),
            domain: draft.domain,
            trigger: draft.trigger,
            conditions: draft.conditions,
            guidance: draft.guidance,
            priority: draft.priority,
            risk_level: draft.risk_level,
            privacy_level: draft.privacy_level,
            status: HeuristicLifecycleStatus::Candidate,
            evidence_refs: draft.evidence_refs,
            opposing_evidence_refs: draft.opposing_evidence_refs,
            validation_state: draft.validation_state,
            source_proposal_id: draft.source_proposal_id,
            version: 1,
            usage: HeuristicUsageMetadata::default(),
            activation_authority: None,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO heuristics (
                id, domain, status, risk_level, priority, source_proposal_id,
                record_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id,
                record.domain,
                record.status.to_string(),
                record.risk_level.to_string(),
                record.priority,
                record.source_proposal_id,
                serde_json::to_string(&record)?,
                record.created_at.to_rfc3339(),
                record.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(record)
    }

    pub fn seed_mvp_heuristics(&self) -> Result<Vec<HeuristicRecord>> {
        let mut seeded = Vec::new();
        seeded.push(
            self.seed_mvp_heuristic(
                BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
                HeuristicDraft::new(
                    "planning",
                    "current_energy_is_low",
                    vec!["state.energy <= 3".into()],
                    "Reduce planning intensity, step count, and pressure.",
                    90,
                    RiskLevel::Low,
                    EvidencePrivacyLevel::Internal,
                )
                .with_stable_id(BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING),
            )?,
        );
        seeded.push(
            self.seed_mvp_heuristic(
                BUILTIN_HEURISTIC_REJECTED_REMINDER_DELAY,
                HeuristicDraft::new(
                    "proactive",
                    "similar_reminder_was_rejected",
                    vec!["recent.proactive_reminder.status == rejected".into()],
                    "Weaken or delay similar proactive reminders after rejection.",
                    80,
                    RiskLevel::Low,
                    EvidencePrivacyLevel::Internal,
                )
                .with_stable_id(BUILTIN_HEURISTIC_REJECTED_REMINDER_DELAY),
            )?,
        );
        Ok(seeded)
    }

    fn seed_mvp_heuristic(&self, id: &str, draft: HeuristicDraft) -> Result<HeuristicRecord> {
        if let Some(existing) = self.get_heuristic(id)? {
            return Ok(existing);
        }
        let record = self.create_heuristic(draft)?;
        self.update_lifecycle(
            &record.id,
            HeuristicLifecycleStatus::Active,
            Some(HeuristicActivationAuthority::SeededBuiltInPolicy(id.into())),
        )
    }

    pub fn get_heuristic(&self, id: &str) -> Result<Option<HeuristicRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare("SELECT record_json FROM heuristics WHERE id = ?1")?;
        match stmt.query_row([id], row_to_record) {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn query(&self, query: HeuristicQuery) -> Result<Vec<HeuristicRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(domain) = &query.domain {
            conditions.push("domain = ?".to_string());
            params.push(Box::new(domain.clone()));
        }
        if let Some(status) = query.status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(status.to_string()));
        }
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        let sql = format!(
            "SELECT record_json FROM heuristics {} ORDER BY priority DESC, updated_at DESC",
            where_clause
        );
        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), row_to_record)?;
        let mut records = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        if let Some(task_kind) = query.task_kind.as_deref() {
            records.retain(|record| record.usage.task_kinds.iter().any(|kind| kind == task_kind));
        }
        if let Some(limit) = query.limit {
            records.truncate(limit);
        }
        Ok(records)
    }

    pub fn update_lifecycle(
        &self,
        id: &str,
        status: HeuristicLifecycleStatus,
        authority: Option<HeuristicActivationAuthority>,
    ) -> Result<HeuristicRecord> {
        let mut record = self.require_heuristic(id)?;
        validate_lifecycle_transition(&record, status, authority.as_ref())?;

        record.status = status;
        record.version += 1;
        if matches!(status, HeuristicLifecycleStatus::Archived) {
            record.archived_at = Some(Utc::now());
        }
        if matches!(status, HeuristicLifecycleStatus::Active) {
            if let Some(HeuristicActivationAuthority::AcceptedProposal(proposal_id)) = &authority {
                record.source_proposal_id = Some(proposal_id.clone());
            }
            record.activation_authority = authority;
        }
        self.save_record(record)
    }

    pub fn record_usage(
        &self,
        id: &str,
        task_kind: impl Into<String>,
        task_metadata: Value,
    ) -> Result<HeuristicRecord> {
        let mut record = self.require_heuristic(id)?;
        let task_kind = task_kind.into();
        record.usage.usage_count += 1;
        record.usage.last_used_at = Some(Utc::now());
        append_unique(&mut record.usage.task_kinds, task_kind.clone());
        record.usage.last_task_kind = Some(task_kind);
        record.usage.last_task_metadata = task_metadata;
        self.save_record(record)
    }

    pub fn fetch_lineage(&self, id: &str) -> Result<HeuristicLineage> {
        let record = self.require_heuristic(id)?;
        Ok(HeuristicLineage {
            heuristic_id: record.id,
            evidence_refs: record.evidence_refs,
            opposing_evidence_refs: record.opposing_evidence_refs,
            source_proposal_id: record.source_proposal_id,
            activation_authority: record.activation_authority,
            version: record.version,
            validation_state: record.validation_state,
        })
    }

    pub fn diagnose_domain_caps(&self, domain: &str) -> Result<DomainCapDiagnostic> {
        let records = self.query(HeuristicQuery {
            domain: Some(domain.to_string()),
            ..HeuristicQuery::default()
        })?;
        let active_count = records
            .iter()
            .filter(|record| record.status == HeuristicLifecycleStatus::Active)
            .count();
        let active_or_trial_count = records
            .iter()
            .filter(|record| {
                matches!(
                    record.status,
                    HeuristicLifecycleStatus::Active | HeuristicLifecycleStatus::Trial
                )
            })
            .count();

        Ok(DomainCapDiagnostic {
            domain: domain.to_string(),
            active_count,
            active_cap: self.active_cap_per_domain,
            active_cap_exceeded: active_count > self.active_cap_per_domain,
            active_or_trial_count,
            active_or_trial_cap: self.active_or_trial_cap_per_domain,
            active_or_trial_cap_exceeded: active_or_trial_count
                > self.active_or_trial_cap_per_domain,
        })
    }

    fn require_heuristic(&self, id: &str) -> Result<HeuristicRecord> {
        self.get_heuristic(id)?
            .ok_or_else(|| anyhow::anyhow!("heuristic record not found: {}", id))
    }

    fn save_record(&self, mut record: HeuristicRecord) -> Result<HeuristicRecord> {
        record.updated_at = Utc::now();
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "UPDATE heuristics SET
                domain = ?2,
                status = ?3,
                risk_level = ?4,
                priority = ?5,
                source_proposal_id = ?6,
                record_json = ?7,
                updated_at = ?8
            WHERE id = ?1",
            params![
                record.id,
                record.domain,
                record.status.to_string(),
                record.risk_level.to_string(),
                record.priority,
                record.source_proposal_id,
                serde_json::to_string(&record)?,
                record.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(record)
    }
}

fn validate_lifecycle_transition(
    record: &HeuristicRecord,
    target: HeuristicLifecycleStatus,
    authority: Option<&HeuristicActivationAuthority>,
) -> Result<()> {
    if matches!(
        record.status,
        HeuristicLifecycleStatus::Archived | HeuristicLifecycleStatus::Rejected
    ) && matches!(
        target,
        HeuristicLifecycleStatus::Active | HeuristicLifecycleStatus::Trial
    ) {
        return Err(anyhow::anyhow!(
            "archived or rejected heuristics cannot be promoted"
        ));
    }

    if target == HeuristicLifecycleStatus::Active {
        let Some(authority) = authority else {
            return Err(anyhow::anyhow!(
                "active heuristic promotion requires accepted governance metadata"
            ));
        };
        match authority {
            HeuristicActivationAuthority::AcceptedProposal(proposal_id)
            | HeuristicActivationAuthority::SeededBuiltInPolicy(proposal_id)
                if proposal_id.trim().is_empty() =>
            {
                return Err(anyhow::anyhow!(
                    "active heuristic promotion authority cannot be empty"
                ));
            }
            _ => {}
        }
    }

    Ok(())
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<HeuristicRecord> {
    let raw: String = row.get(0)?;
    serde_json::from_str(&raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn append_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
