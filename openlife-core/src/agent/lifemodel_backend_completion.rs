use crate::agent::evidence_graph::EvidenceGraphReport;
use crate::agent::evidence_store::{
    EvidenceDraft, EvidencePrivacyLevel, EvidenceSourceRef, EvidenceSourceType, EvidenceStore,
    EvidenceType,
};
use crate::agent::runtime_contract::LifeEventDraft;
use crate::agent::types::RiskLevel;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, types::Type, Connection, Row};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

const DEFAULT_CHAT_LEGACY_PATH: &str = "legacy_stream";
const BACKEND_COMPLETION_REPORT_KIND: &str = "lifemodel_governed_backend_completion_readiness";
const SIGNAL_EXTRACTOR_ID: &str = "deterministic.low_energy_planning";
const SIGNAL_EXTRACTOR_VERSION: &str = "1";
const MIN_SIGNAL_EVIDENCE_CONFIDENCE: f32 = 0.65;
const LOW_ENERGY_EVIDENCE_PATH: &str = "/preferences/planning/low_energy_intensity";
const LOW_ENERGY_SIGNAL_SUMMARY: &str =
    "User prefers low-pressure planning with small next steps when energy is low.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelBackendCompletionReadinessReport {
    pub report_kind: String,
    pub report_ready: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub default_chat_isolated: bool,
    pub default_chat_selected_adapter_path: String,
    pub ordinary_chat_route_unchanged: bool,
    pub migration_permission: bool,
    pub runtime_execution_allowed: bool,
    pub model_execution_allowed: bool,
    pub tool_execution_allowed: bool,
    pub business_writes_allowed: bool,
    pub tauri_command_required: bool,
    pub current_prerequisites: LifeModelBackendPrerequisites,
    pub governance_readiness: LifeModelBackendGovernanceReadiness,
    pub next_required_schemas: Vec<String>,
    pub blockers: Vec<String>,
    pub master_spec_gate_blockers: Vec<LifeModelBackendGateBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelBackendPrerequisites {
    pub w123_react_beta_execution_hardening_complete: bool,
    pub legacy_direct_write_convergence_complete: bool,
    pub default_chat_legacy_stream: bool,
    pub evidence_store_present: bool,
    pub evidence_graph_present: bool,
    pub heuristic_store_present: bool,
    pub policy_store_present: bool,
    pub proposal_store_present: bool,
    pub patch_store_present: bool,
    pub runtime_hs_packet_present: bool,
    pub react_present: bool,
    pub plan_execute_present: bool,
    pub model_router_present: bool,
    pub action_executor_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelBackendGovernanceReadiness {
    pub evidence_store_present: bool,
    pub evidence_graph_present: bool,
    pub heuristic_store_present: bool,
    pub policy_store_present: bool,
    pub proposal_store_present: bool,
    pub patch_store_present: bool,
    pub source_lineage_required: bool,
    pub metadata_safe_reports_required: bool,
    pub proposal_first_required_for_truth: bool,
    pub direct_lifemodel_truth_write_allowed: bool,
    pub raw_content_allowed_in_reports: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelBackendGateBlocker {
    pub gate: String,
    pub blockers: Vec<String>,
}

pub fn evaluate_lifemodel_backend_completion_readiness() -> LifeModelBackendCompletionReadinessReport
{
    let current_prerequisites = LifeModelBackendPrerequisites {
        w123_react_beta_execution_hardening_complete: true,
        legacy_direct_write_convergence_complete: true,
        default_chat_legacy_stream: true,
        evidence_store_present: type_available::<EvidenceStore>(),
        evidence_graph_present: type_available::<EvidenceGraphReport>(),
        heuristic_store_present: type_available::<crate::agent::heuristic_store::HeuristicStore>(),
        policy_store_present: type_available::<crate::agent::policy_store::PolicyStore>(),
        proposal_store_present: type_available::<crate::agent::proposal_store::ProposalStore>(),
        patch_store_present: type_available::<crate::life_model::patch_store::PatchStore>(),
        runtime_hs_packet_present: type_available::<crate::agent::hs_selector::RuntimeHSPacket>(),
        react_present: type_available::<crate::agent::agent_loop::AgentLoop>(),
        plan_execute_present: type_available::<crate::agent::plan_execute::PlanExecuteService>(),
        model_router_present: type_available::<crate::agent::model_router::ModelRouter>(),
        action_executor_present: type_available::<crate::agent::action_executor::ActionExecutor>(),
    };
    let governance_readiness = LifeModelBackendGovernanceReadiness {
        evidence_store_present: current_prerequisites.evidence_store_present,
        evidence_graph_present: current_prerequisites.evidence_graph_present,
        heuristic_store_present: current_prerequisites.heuristic_store_present,
        policy_store_present: current_prerequisites.policy_store_present,
        proposal_store_present: current_prerequisites.proposal_store_present,
        patch_store_present: current_prerequisites.patch_store_present,
        source_lineage_required: true,
        metadata_safe_reports_required: true,
        proposal_first_required_for_truth: true,
        direct_lifemodel_truth_write_allowed: false,
        raw_content_allowed_in_reports: false,
    };
    let next_required_schemas = Vec::new();
    let blockers = Vec::new();
    LifeModelBackendCompletionReadinessReport {
        report_kind: BACKEND_COMPLETION_REPORT_KIND.to_string(),
        report_ready: true,
        metadata_safe: true,
        contains_raw_content: false,
        default_chat_isolated: true,
        default_chat_selected_adapter_path: DEFAULT_CHAT_LEGACY_PATH.to_string(),
        ordinary_chat_route_unchanged: true,
        migration_permission: false,
        runtime_execution_allowed: false,
        model_execution_allowed: false,
        tool_execution_allowed: false,
        business_writes_allowed: false,
        tauri_command_required: false,
        current_prerequisites,
        governance_readiness,
        next_required_schemas,
        blockers,
        master_spec_gate_blockers: Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeDomain {
    LowEnergyPlanning,
    PlanningPreference,
    StateEnergy,
    WorkStyle,
    CommunicationPreference,
    ToolUse,
    Identity,
    Values,
    Relationships,
    Health,
    Finance,
    Privacy,
    Other,
}

impl std::fmt::Display for LifeDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifeDomain::LowEnergyPlanning => write!(f, "low_energy_planning"),
            LifeDomain::PlanningPreference => write!(f, "planning_preference"),
            LifeDomain::StateEnergy => write!(f, "state_energy"),
            LifeDomain::WorkStyle => write!(f, "work_style"),
            LifeDomain::CommunicationPreference => write!(f, "communication_preference"),
            LifeDomain::ToolUse => write!(f, "tool_use"),
            LifeDomain::Identity => write!(f, "identity"),
            LifeDomain::Values => write!(f, "values"),
            LifeDomain::Relationships => write!(f, "relationships"),
            LifeDomain::Health => write!(f, "health"),
            LifeDomain::Finance => write!(f, "finance"),
            LifeDomain::Privacy => write!(f, "privacy"),
            LifeDomain::Other => write!(f, "other"),
        }
    }
}

impl LifeDomain {
    fn from_str(value: &str) -> Self {
        match value {
            "low_energy_planning" => LifeDomain::LowEnergyPlanning,
            "planning_preference" => LifeDomain::PlanningPreference,
            "state_energy" => LifeDomain::StateEnergy,
            "work_style" => LifeDomain::WorkStyle,
            "communication_preference" => LifeDomain::CommunicationPreference,
            "tool_use" => LifeDomain::ToolUse,
            "identity" => LifeDomain::Identity,
            "values" => LifeDomain::Values,
            "relationships" => LifeDomain::Relationships,
            "health" => LifeDomain::Health,
            "finance" => LifeDomain::Finance,
            "privacy" => LifeDomain::Privacy,
            _ => LifeDomain::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeEventPrivacyLevel {
    Public,
    Internal,
    Sensitive,
    StrictlyLocal,
}

impl std::fmt::Display for LifeEventPrivacyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifeEventPrivacyLevel::Public => write!(f, "public"),
            LifeEventPrivacyLevel::Internal => write!(f, "internal"),
            LifeEventPrivacyLevel::Sensitive => write!(f, "sensitive"),
            LifeEventPrivacyLevel::StrictlyLocal => write!(f, "strictly_local"),
        }
    }
}

impl LifeEventPrivacyLevel {
    fn from_str(value: &str) -> Self {
        match value {
            "public" => LifeEventPrivacyLevel::Public,
            "sensitive" => LifeEventPrivacyLevel::Sensitive,
            "strictly_local" => LifeEventPrivacyLevel::StrictlyLocal,
            _ => LifeEventPrivacyLevel::Internal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeEventSourceType {
    ChatMessage,
    AgentRun,
    RuntimeObservation,
    Proposal,
    ToolResult,
    Feedback,
    UserEdit,
    ManualCorrection,
    Other,
}

impl std::fmt::Display for LifeEventSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifeEventSourceType::ChatMessage => write!(f, "chat_message"),
            LifeEventSourceType::AgentRun => write!(f, "agent_run"),
            LifeEventSourceType::RuntimeObservation => write!(f, "runtime_observation"),
            LifeEventSourceType::Proposal => write!(f, "proposal"),
            LifeEventSourceType::ToolResult => write!(f, "tool_result"),
            LifeEventSourceType::Feedback => write!(f, "feedback"),
            LifeEventSourceType::UserEdit => write!(f, "user_edit"),
            LifeEventSourceType::ManualCorrection => write!(f, "manual_correction"),
            LifeEventSourceType::Other => write!(f, "other"),
        }
    }
}

impl LifeEventSourceType {
    fn from_str(value: &str) -> Self {
        match value {
            "chat_message" => LifeEventSourceType::ChatMessage,
            "agent_run" => LifeEventSourceType::AgentRun,
            "runtime_observation" => LifeEventSourceType::RuntimeObservation,
            "proposal" => LifeEventSourceType::Proposal,
            "tool_result" => LifeEventSourceType::ToolResult,
            "feedback" => LifeEventSourceType::Feedback,
            "user_edit" => LifeEventSourceType::UserEdit,
            "manual_correction" => LifeEventSourceType::ManualCorrection,
            _ => LifeEventSourceType::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeEventSourceRef {
    pub source_type: LifeEventSourceType,
    pub source_id: String,
    pub source_detail: Option<String>,
    pub digest: String,
    pub observed_at: DateTime<Utc>,
}

impl LifeEventSourceRef {
    pub fn from_digest(
        source_type: LifeEventSourceType,
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

    pub fn from_payload(
        source_type: LifeEventSourceType,
        source_id: impl Into<String>,
        source_detail: Option<&str>,
        payload: &str,
    ) -> Self {
        Self::from_digest(
            source_type,
            source_id,
            source_detail,
            sha256_hex(payload.as_bytes()),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeEvent {
    pub id: String,
    pub event_type: String,
    pub source_type: LifeEventSourceType,
    pub source_id: String,
    pub source_refs: Vec<LifeEventSourceRef>,
    pub occurred_at: DateTime<Utc>,
    pub domain: LifeDomain,
    pub risk_level: RiskLevel,
    pub privacy_level: LifeEventPrivacyLevel,
    pub summary: String,
    pub payload_digest: String,
    pub metadata: Value,
    pub metadata_safe_summary: Option<Value>,
    pub contains_raw_content: bool,
    pub dedupe_key: String,
    pub created_at: DateTime<Utc>,
}

pub struct LifeEventStore {
    conn: Mutex<Connection>,
}

impl LifeEventStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open life event db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory event db")?;
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
            .map_err(|e| anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS life_events (
                id TEXT PRIMARY KEY,
                event_type TEXT NOT NULL,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                source_refs_json TEXT NOT NULL,
                occurred_at TEXT NOT NULL,
                domain TEXT NOT NULL,
                risk_level TEXT NOT NULL,
                privacy_level TEXT NOT NULL,
                summary TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                metadata_safe_summary_json TEXT,
                contains_raw_content INTEGER NOT NULL,
                dedupe_key TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_life_events_domain_time ON life_events(domain, occurred_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_life_events_dedupe_key ON life_events(dedupe_key)",
            [],
        )?;
        Ok(())
    }

    pub fn create_event(
        &self,
        draft: LifeEventDraft,
        source_refs: Vec<LifeEventSourceRef>,
        domain: LifeDomain,
        risk_level: RiskLevel,
        privacy_level: LifeEventPrivacyLevel,
    ) -> Result<LifeEvent> {
        let mut blockers = Vec::new();
        if source_refs.is_empty() {
            push_unique(&mut blockers, "source_lineage_missing");
        }
        if draft.summary.trim().is_empty() {
            push_unique(&mut blockers, "metadata_safe_summary_missing");
        }
        let draft_raw =
            value_contains_raw_content(&draft.metadata) || text_contains_raw_marker(&draft.summary);
        if draft_raw {
            push_unique(&mut blockers, "raw_content_present");
        }
        if !blockers.is_empty() {
            return Err(anyhow!("life event blocked: {}", blockers.join(",")));
        }

        let primary_source = source_refs
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("life event blocked: source_lineage_missing"))?;
        let now = Utc::now();
        let metadata = sanitize_event_metadata(&draft.metadata);
        let payload_digest = sha256_hex(
            json!({
                "eventType": draft.event_type,
                "summary": draft.summary,
                "domain": domain.to_string(),
                "riskLevel": risk_level.to_string(),
                "privacyLevel": privacy_level.to_string(),
                "metadata": metadata,
                "sourceDigests": source_refs.iter().map(|source| source.digest.as_str()).collect::<Vec<_>>()
            })
            .to_string()
            .as_bytes(),
        );
        let source_digest = primary_source.digest.clone();
        let dedupe_key = format!(
            "life_event:{}:{}:{}",
            domain,
            draft.event_type,
            short_hash(&source_digest)
        );
        let source_count = source_refs.len();
        let event = LifeEvent {
            id: format!("le_{}", Uuid::new_v4().simple()),
            event_type: draft.event_type,
            source_type: primary_source.source_type,
            source_id: primary_source.source_id,
            source_refs,
            occurred_at: now,
            domain,
            risk_level,
            privacy_level,
            summary: metadata_safe_summary_text(&draft.summary),
            payload_digest,
            metadata,
            metadata_safe_summary: Some(json!({
                "eventType": "metadata_safe_life_event",
                "domain": domain.to_string(),
                "summaryDigest": sha256_hex(draft.summary.as_bytes()),
                "sourceCount": source_count,
                "payloadDigest": short_hash(&dedupe_key)
            })),
            contains_raw_content: false,
            dedupe_key,
            created_at: now,
        };
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO life_events (
                id, event_type, source_type, source_id, source_refs_json,
                occurred_at, domain, risk_level, privacy_level, summary,
                payload_digest, metadata_json, metadata_safe_summary_json,
                contains_raw_content, dedupe_key, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                event.id,
                event.event_type,
                event.source_type.to_string(),
                event.source_id,
                serde_json::to_string(&event.source_refs)?,
                event.occurred_at.to_rfc3339(),
                event.domain.to_string(),
                event.risk_level.to_string(),
                event.privacy_level.to_string(),
                event.summary,
                event.payload_digest,
                serde_json::to_string(&event.metadata)?,
                event
                    .metadata_safe_summary
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                event.contains_raw_content as i64,
                event.dedupe_key,
                event.created_at.to_rfc3339(),
            ],
        )?;
        Ok(event)
    }

    pub fn query_events(
        &self,
        domain: Option<LifeDomain>,
        limit: Option<usize>,
    ) -> Result<Vec<LifeEvent>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow!("mutex poison: {}", e))?;
        let mut records = if let Some(domain) = domain {
            let mut stmt = conn.prepare(&format!(
                "SELECT {} FROM life_events WHERE domain = ?1 ORDER BY occurred_at DESC",
                Self::columns()
            ))?;
            let rows = stmt.query_map([domain.to_string()], Self::row_to_event)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            let mut stmt = conn.prepare(&format!(
                "SELECT {} FROM life_events ORDER BY occurred_at DESC",
                Self::columns()
            ))?;
            let rows = stmt.query_map([], Self::row_to_event)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        if let Some(limit) = limit {
            records.truncate(limit);
        }
        Ok(records)
    }

    fn columns() -> &'static str {
        "id, event_type, source_type, source_id, source_refs_json, occurred_at,
         domain, risk_level, privacy_level, summary, payload_digest, metadata_json,
         metadata_safe_summary_json, contains_raw_content, dedupe_key, created_at"
    }

    fn row_to_event(row: &Row<'_>) -> rusqlite::Result<LifeEvent> {
        let source_type: String = row.get(2)?;
        let occurred_at: String = row.get(5)?;
        let domain: String = row.get(6)?;
        let risk_level: String = row.get(7)?;
        let privacy_level: String = row.get(8)?;
        let created_at: String = row.get(15)?;
        Ok(LifeEvent {
            id: row.get(0)?,
            event_type: row.get(1)?,
            source_type: LifeEventSourceType::from_str(&source_type),
            source_id: row.get(3)?,
            source_refs: json_column(row, 4)?,
            occurred_at: parse_time(&occurred_at, 5)?,
            domain: LifeDomain::from_str(&domain),
            risk_level: risk_level_from_str(&risk_level),
            privacy_level: LifeEventPrivacyLevel::from_str(&privacy_level),
            summary: row.get(9)?,
            payload_digest: row.get(10)?,
            metadata: json_column(row, 11)?,
            metadata_safe_summary: optional_json_column(row, 12)?,
            contains_raw_content: row.get::<_, i64>(13)? != 0,
            dedupe_key: row.get(14)?,
            created_at: parse_time(&created_at, 15)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeSignalType {
    PlanningIntensityPreference,
    StateEnergyPattern,
    WorkStylePreference,
    CommunicationPreference,
    Unsupported,
}

impl std::fmt::Display for LifeSignalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifeSignalType::PlanningIntensityPreference => {
                write!(f, "planning_intensity_preference")
            }
            LifeSignalType::StateEnergyPattern => write!(f, "state_energy_pattern"),
            LifeSignalType::WorkStylePreference => write!(f, "work_style_preference"),
            LifeSignalType::CommunicationPreference => write!(f, "communication_preference"),
            LifeSignalType::Unsupported => write!(f, "unsupported"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeSignalPolarity {
    Supporting,
    Opposing,
    Corrective,
    Uncertain,
}

impl std::fmt::Display for LifeSignalPolarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifeSignalPolarity::Supporting => write!(f, "supporting"),
            LifeSignalPolarity::Opposing => write!(f, "opposing"),
            LifeSignalPolarity::Corrective => write!(f, "corrective"),
            LifeSignalPolarity::Uncertain => write!(f, "uncertain"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeSignal {
    pub id: String,
    pub signal_type: LifeSignalType,
    pub domain: LifeDomain,
    pub claim_summary: String,
    pub polarity: LifeSignalPolarity,
    pub confidence: f32,
    pub risk_level: RiskLevel,
    pub privacy_level: LifeEventPrivacyLevel,
    pub source_event_ids: Vec<String>,
    pub extractor_id: String,
    pub extractor_version: String,
    pub uncertainty_reasons: Vec<String>,
    pub dedupe_key: String,
    pub metadata: Value,
    pub contains_raw_content: bool,
}

#[derive(Debug, Clone)]
pub struct LifeSignalExtractorInput {
    pub events: Vec<LifeEvent>,
}

impl LifeSignalExtractorInput {
    pub fn new(events: Vec<LifeEvent>) -> Self {
        Self { events }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeSignalExtractorReport {
    pub extractor_id: String,
    pub extractor_version: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub ran_runtime: bool,
    pub ran_model: bool,
    pub ran_tool: bool,
    pub accepted_signals: Vec<LifeSignal>,
    pub dropped_signals: Vec<DroppedLifeSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedLifeSignal {
    pub event_id: String,
    pub event_digest: String,
    pub reasons: Vec<String>,
}

pub fn extract_life_signals(input: LifeSignalExtractorInput) -> LifeSignalExtractorReport {
    let mut accepted_signals = Vec::new();
    let mut dropped_signals = Vec::new();
    for event in input.events {
        let mut reasons = Vec::new();
        if event.contains_raw_content || value_contains_raw_content(&event.metadata) {
            push_unique(&mut reasons, "event_contains_raw_content");
        }
        if !matches!(event.risk_level, RiskLevel::Low) {
            push_unique(&mut reasons, "high_risk_event");
        }
        if !matches!(
            event.privacy_level,
            LifeEventPrivacyLevel::Public | LifeEventPrivacyLevel::Internal
        ) {
            push_unique(&mut reasons, "event_privacy_not_allowed");
        }
        if event.source_refs.is_empty() {
            push_unique(&mut reasons, "source_lineage_missing");
        }
        if event.domain != LifeDomain::LowEnergyPlanning {
            push_unique(&mut reasons, "unsupported_domain");
        }
        if !is_low_energy_planning_event(&event) {
            push_unique(&mut reasons, "unsupported_event_type");
        }

        if !reasons.is_empty() {
            dropped_signals.push(DroppedLifeSignal {
                event_id: event.id,
                event_digest: event.payload_digest,
                reasons,
            });
            continue;
        }

        let confidence = confidence_from_metadata(&event.metadata).unwrap_or(0.78);
        let mut uncertainty_reasons = Vec::new();
        if confidence < 0.75 {
            push_unique(&mut uncertainty_reasons, "below_preferred_confidence");
        }
        accepted_signals.push(LifeSignal {
            id: format!("ls_{}", Uuid::new_v4().simple()),
            signal_type: LifeSignalType::PlanningIntensityPreference,
            domain: LifeDomain::LowEnergyPlanning,
            claim_summary: LOW_ENERGY_SIGNAL_SUMMARY.to_string(),
            polarity: LifeSignalPolarity::Supporting,
            confidence: confidence.clamp(0.0, 1.0),
            risk_level: RiskLevel::Low,
            privacy_level: LifeEventPrivacyLevel::Internal,
            source_event_ids: vec![event.id.clone()],
            extractor_id: SIGNAL_EXTRACTOR_ID.to_string(),
            extractor_version: SIGNAL_EXTRACTOR_VERSION.to_string(),
            uncertainty_reasons,
            dedupe_key: format!(
                "signal:low_energy_planning:{}",
                short_hash(&event.dedupe_key)
            ),
            metadata: json!({
                "sourceEventDigest": event.payload_digest,
                "sourceEventDedupeKeyDigest": sha256_hex(event.dedupe_key.as_bytes()),
                "deterministic": true,
                "metadataSafe": true
            }),
            contains_raw_content: false,
        });
    }

    LifeSignalExtractorReport {
        extractor_id: SIGNAL_EXTRACTOR_ID.to_string(),
        extractor_version: SIGNAL_EXTRACTOR_VERSION.to_string(),
        metadata_safe: true,
        contains_raw_content: false,
        ran_runtime: false,
        ran_model: false,
        ran_tool: false,
        accepted_signals,
        dropped_signals,
    }
}

#[derive(Debug, Clone)]
pub struct LifeSignalBridgeInput {
    pub signal: LifeSignal,
    pub source_events: Vec<LifeEvent>,
}

impl LifeSignalBridgeInput {
    pub fn new(signal: LifeSignal, source_events: Vec<LifeEvent>) -> Self {
        Self {
            signal,
            source_events,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeSignalEvidenceBridgeReport {
    pub bridged: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub evidence_ids: Vec<String>,
    pub wrote_evidence_count: u32,
    pub wrote_life_model_count: u32,
    pub wrote_memory_count: u32,
    pub wrote_heuristic_count: u32,
    pub wrote_chat_message_count: u32,
    pub wrote_agent_run_count: u32,
    pub wrote_mcp_audit_count: u32,
    pub wrote_external_count: u32,
    pub ran_runtime: bool,
    pub ran_model: bool,
    pub ran_tool: bool,
    pub blocking_reasons: Vec<String>,
}

pub fn bridge_life_signal_to_evidence(
    input: LifeSignalBridgeInput,
    evidence_store: &EvidenceStore,
) -> Result<LifeSignalEvidenceBridgeReport> {
    let mut blocking_reasons = Vec::new();
    if input.signal.contains_raw_content || value_contains_raw_content(&input.signal.metadata) {
        push_unique(&mut blocking_reasons, "signal_contains_raw_content");
    }
    if input.signal.confidence < MIN_SIGNAL_EVIDENCE_CONFIDENCE {
        push_unique(&mut blocking_reasons, "signal_confidence_too_low");
    }
    if !matches!(input.signal.risk_level, RiskLevel::Low) {
        push_unique(&mut blocking_reasons, "signal_risk_not_allowed");
    }
    if !matches!(
        input.signal.privacy_level,
        LifeEventPrivacyLevel::Public | LifeEventPrivacyLevel::Internal
    ) {
        push_unique(&mut blocking_reasons, "signal_privacy_not_allowed");
    }
    if input.signal.domain != LifeDomain::LowEnergyPlanning {
        push_unique(&mut blocking_reasons, "unsupported_signal_domain");
    }
    if input.signal.signal_type != LifeSignalType::PlanningIntensityPreference {
        push_unique(&mut blocking_reasons, "unsupported_signal_type");
    }
    if input.signal.source_event_ids.is_empty() {
        push_unique(&mut blocking_reasons, "source_event_lineage_missing");
    }

    let event_by_id: HashMap<String, LifeEvent> = input
        .source_events
        .into_iter()
        .map(|event| (event.id.clone(), event))
        .collect();
    let mut lineage_events = Vec::new();
    for event_id in &input.signal.source_event_ids {
        match event_by_id.get(event_id) {
            Some(event) => {
                if event.contains_raw_content || value_contains_raw_content(&event.metadata) {
                    push_unique(&mut blocking_reasons, "source_event_contains_raw_content");
                }
                if event.source_refs.is_empty() {
                    push_unique(&mut blocking_reasons, "source_event_lineage_missing");
                }
                if !matches!(event.risk_level, RiskLevel::Low) {
                    push_unique(&mut blocking_reasons, "source_event_risk_not_allowed");
                }
                if event.domain != LifeDomain::LowEnergyPlanning {
                    push_unique(&mut blocking_reasons, "source_event_domain_not_allowed");
                }
                lineage_events.push(event.clone());
            }
            None => push_unique(&mut blocking_reasons, "source_event_lineage_missing"),
        }
    }

    if !blocking_reasons.is_empty() {
        return Ok(blocked_bridge_report(blocking_reasons));
    }

    let mut source_refs = vec![EvidenceSourceRef::from_digest(
        EvidenceSourceType::RunMetadata,
        input.signal.id.clone(),
        Some(SIGNAL_EXTRACTOR_ID),
        signal_digest(&input.signal),
    )];
    let mut linked_agent_run_ids = Vec::new();
    for event in &lineage_events {
        for source in &event.source_refs {
            let evidence_source_type = evidence_source_type_from_life_source(source.source_type);
            if evidence_source_type == EvidenceSourceType::AgentRun {
                push_unique(&mut linked_agent_run_ids, source.source_id.clone());
            }
            source_refs.push(EvidenceSourceRef::from_digest(
                evidence_source_type,
                source.source_id.clone(),
                Some(&format!(
                    "life_event:{}:{}",
                    event.id,
                    source.source_detail.as_deref().unwrap_or("source")
                )),
                source.digest.clone(),
            ));
        }
    }

    let mut draft = EvidenceDraft::new(
        EvidenceType::Preference,
        LOW_ENERGY_EVIDENCE_PATH,
        input.signal.confidence,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_summary(LOW_ENERGY_SIGNAL_SUMMARY);
    for source_ref in source_refs {
        draft = draft.with_source_ref(source_ref);
    }
    for run_id in linked_agent_run_ids {
        draft = draft.with_linked_agent_run(run_id);
    }
    draft.run_metadata = json!({
        "bridgeKind": "life_signal_to_evidence",
        "signalId": input.signal.id,
        "signalDedupeKeyDigest": sha256_hex(input.signal.dedupe_key.as_bytes()),
        "sourceEventIds": input.signal.source_event_ids,
        "extractorId": input.signal.extractor_id,
        "extractorVersion": input.signal.extractor_version,
        "metadataSafe": true
    });

    let evidence = evidence_store.create_evidence(draft)?;
    Ok(LifeSignalEvidenceBridgeReport {
        bridged: true,
        metadata_safe: true,
        contains_raw_content: false,
        evidence_ids: vec![evidence.id],
        wrote_evidence_count: 1,
        wrote_life_model_count: 0,
        wrote_memory_count: 0,
        wrote_heuristic_count: 0,
        wrote_chat_message_count: 0,
        wrote_agent_run_count: 0,
        wrote_mcp_audit_count: 0,
        wrote_external_count: 0,
        ran_runtime: false,
        ran_model: false,
        ran_tool: false,
        blocking_reasons: Vec::new(),
    })
}

fn blocked_bridge_report(blocking_reasons: Vec<String>) -> LifeSignalEvidenceBridgeReport {
    LifeSignalEvidenceBridgeReport {
        bridged: false,
        metadata_safe: true,
        contains_raw_content: false,
        evidence_ids: Vec::new(),
        wrote_evidence_count: 0,
        wrote_life_model_count: 0,
        wrote_memory_count: 0,
        wrote_heuristic_count: 0,
        wrote_chat_message_count: 0,
        wrote_agent_run_count: 0,
        wrote_mcp_audit_count: 0,
        wrote_external_count: 0,
        ran_runtime: false,
        ran_model: false,
        ran_tool: false,
        blocking_reasons,
    }
}

fn is_low_energy_planning_event(event: &LifeEvent) -> bool {
    event.domain == LifeDomain::LowEnergyPlanning
        && (event.event_type.contains("low_energy")
            || event
                .metadata
                .get("domain")
                .and_then(Value::as_str)
                .map(|domain| domain == "low_energy_planning")
                .unwrap_or(false))
}

fn sanitize_event_metadata(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sanitized = serde_json::Map::new();
            for key in [
                "confidence",
                "proposal_only",
                "domain",
                "sourceDigest",
                "source_digest",
                "eventDigest",
            ] {
                if let Some(value) = map.get(key) {
                    sanitized.insert(key.to_string(), value.clone());
                }
            }
            Value::Object(sanitized)
        }
        _ => json!({}),
    }
}

fn metadata_safe_summary_text(summary: &str) -> String {
    if summary.chars().count() > 180 {
        summary.chars().take(180).collect()
    } else {
        summary.to_string()
    }
}

fn confidence_from_metadata(metadata: &Value) -> Option<f32> {
    metadata
        .get("confidence")
        .and_then(Value::as_f64)
        .map(|value| (value as f32).clamp(0.0, 1.0))
}

fn value_contains_raw_content(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            raw_key(key)
                || value_contains_raw_content(value)
                || value
                    .as_str()
                    .map(text_contains_raw_marker)
                    .unwrap_or(false)
        }),
        Value::Array(values) => values.iter().any(value_contains_raw_content),
        Value::String(text) => text_contains_raw_marker(text),
        _ => false,
    }
}

fn raw_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("raw")
        || normalized.contains("prompt")
        || normalized.contains("assistantoutput")
        || normalized.contains("usertext")
        || normalized.contains("memorycontext")
        || normalized.contains("toolpayload")
        || normalized.contains("payload")
}

fn text_contains_raw_marker(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    normalized.contains("raw prompt")
        || normalized.contains("raw user")
        || normalized.contains("raw assistant")
        || normalized.contains("assistant output")
        || normalized.contains("tool payload")
        || normalized.contains("tool output")
        || normalized.contains("memory context")
        || normalized.contains("raw content")
        || normalized.contains("secret-")
        || normalized.contains("sk-test")
        || normalized.contains('@')
}

fn evidence_source_type_from_life_source(source_type: LifeEventSourceType) -> EvidenceSourceType {
    match source_type {
        LifeEventSourceType::ChatMessage => EvidenceSourceType::ChatMessage,
        LifeEventSourceType::AgentRun => EvidenceSourceType::AgentRun,
        LifeEventSourceType::Proposal => EvidenceSourceType::Proposal,
        LifeEventSourceType::Feedback => EvidenceSourceType::Feedback,
        LifeEventSourceType::UserEdit | LifeEventSourceType::ManualCorrection => {
            EvidenceSourceType::UserEdit
        }
        LifeEventSourceType::RuntimeObservation | LifeEventSourceType::ToolResult => {
            EvidenceSourceType::RunMetadata
        }
        LifeEventSourceType::Other => EvidenceSourceType::Other,
    }
}

fn signal_digest(signal: &LifeSignal) -> String {
    sha256_hex(
        json!({
            "signalType": signal.signal_type.to_string(),
            "domain": signal.domain.to_string(),
            "polarity": signal.polarity.to_string(),
            "confidence": signal.confidence,
            "sourceEventIds": signal.source_event_ids,
            "extractorId": signal.extractor_id,
            "extractorVersion": signal.extractor_version,
            "dedupeKey": signal.dedupe_key,
        })
        .to_string()
        .as_bytes(),
    )
}

fn type_available<T: 'static>() -> bool {
    let _ = std::any::TypeId::of::<T>();
    true
}

fn json_column<T: DeserializeOwned>(row: &Row<'_>, idx: usize) -> rusqlite::Result<T> {
    let raw: String = row.get(idx)?;
    serde_json::from_str(&raw)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(e)))
}

fn optional_json_column<T: DeserializeOwned>(
    row: &Row<'_>,
    idx: usize,
) -> rusqlite::Result<Option<T>> {
    let raw: Option<String> = row.get(idx)?;
    match raw {
        Some(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(e))),
        None => Ok(None),
    }
}

fn parse_time(value: &str, idx: usize) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(e)))
}

fn risk_level_from_str(value: &str) -> RiskLevel {
    match value {
        "low" => RiskLevel::Low,
        "high" => RiskLevel::High,
        "critical" => RiskLevel::Critical,
        _ => RiskLevel::Medium,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = digest(&SHA256, bytes);
    digest
        .as_ref()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn short_hash(value: &str) -> String {
    sha256_hex(value.as_bytes()).chars().take(16).collect()
}

fn push_unique(reasons: &mut Vec<String>, reason: impl Into<String>) {
    let reason = reason.into();
    if !reasons.iter().any(|existing| existing == &reason) {
        reasons.push(reason);
    }
}
