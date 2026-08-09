use crate::agent::runtime_contract::LifeEventDraft;
use crate::agent::types::{AgentRunReceiptKey, RiskLevel};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, types::Type, Connection, OptionalExtension, Row, TransactionBehavior};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const LIFE_EVENT_PAYLOAD_VERSION: i64 = 3;
const LIFE_EVENT_V2_PHYSICAL_PURGE_MARKER: &str = "life_event_v2_physical_purge_complete";
#[cfg(any(test, feature = "test-utils"))]
const MAX_LIFE_EVENT_METADATA_BYTES: usize = 1024 * 1024;
const MAX_LIFE_EVENT_SOURCE_REFS: usize = 512;

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
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "low_energy_planning" => Some(LifeDomain::LowEnergyPlanning),
            "planning_preference" => Some(LifeDomain::PlanningPreference),
            "state_energy" => Some(LifeDomain::StateEnergy),
            "work_style" => Some(LifeDomain::WorkStyle),
            "communication_preference" => Some(LifeDomain::CommunicationPreference),
            "tool_use" => Some(LifeDomain::ToolUse),
            "identity" => Some(LifeDomain::Identity),
            "values" => Some(LifeDomain::Values),
            "relationships" => Some(LifeDomain::Relationships),
            "health" => Some(LifeDomain::Health),
            "finance" => Some(LifeDomain::Finance),
            "privacy" => Some(LifeDomain::Privacy),
            "other" => Some(LifeDomain::Other),
            _ => None,
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
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "public" => Some(LifeEventPrivacyLevel::Public),
            "internal" => Some(LifeEventPrivacyLevel::Internal),
            "sensitive" => Some(LifeEventPrivacyLevel::Sensitive),
            "strictly_local" => Some(LifeEventPrivacyLevel::StrictlyLocal),
            _ => None,
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
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "chat_message" => Some(LifeEventSourceType::ChatMessage),
            "agent_run" => Some(LifeEventSourceType::AgentRun),
            "runtime_observation" => Some(LifeEventSourceType::RuntimeObservation),
            "proposal" => Some(LifeEventSourceType::Proposal),
            "tool_result" => Some(LifeEventSourceType::ToolResult),
            "feedback" => Some(LifeEventSourceType::Feedback),
            "user_edit" => Some(LifeEventSourceType::UserEdit),
            "manual_correction" => Some(LifeEventSourceType::ManualCorrection),
            "other" => Some(LifeEventSourceType::Other),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeEventSourceRef {
    pub source_type: LifeEventSourceType,
    pub source_id: String,
    pub source_detail: Option<String>,
    pub digest: String,
    pub observed_at: DateTime<Utc>,
    #[serde(default)]
    pub canonical_owner: Option<CanonicalLifeEventOwnerRef>,
    #[serde(default)]
    pub verification: LifeEventSourceVerification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalLifeEventOwnerKind {
    ConversationMessage,
    AgentRun,
}

impl CanonicalLifeEventOwnerKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ConversationMessage => "conversation_message",
            Self::AgentRun => "agent_run",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalLifeEventOwnerRef {
    pub owner_kind: CanonicalLifeEventOwnerKind,
    pub canonical_store_identity: String,
    pub canonical_ref: String,
    pub canonical_content_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_revision: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeEventSourceVerification {
    CanonicalOwnerVerified,
    #[default]
    LegacyUnverified,
}

impl LifeEventSourceVerification {
    fn as_str(self) -> &'static str {
        match self {
            Self::CanonicalOwnerVerified => "canonical_owner_verified",
            Self::LegacyUnverified => "legacy_unverified",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifeEventSensitivity {
    Low,
    #[cfg(test)]
    Medium,
    #[cfg(test)]
    High,
}

impl LifeEventSensitivity {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            #[cfg(test)]
            Self::Medium => "medium",
            #[cfg(test)]
            Self::High => "high",
        }
    }
}

/// One non-transferable runtime permit for one exact LifeEvent mutation.
///
/// The type deliberately implements neither `Clone` nor serde traits. Product
/// policy integration must issue a fresh permit for an exact retry rather than
/// serializing authority across process or IPC boundaries.
pub(crate) struct LifeEventCreatePermit {
    current_user_message_id: String,
    current_user_message_store_identity: String,
    current_user_message_digest: String,
    candidate_id: String,
    draft_digest: String,
    task_id: String,
    run_id: String,
    execution_id: String,
    canonical_store_identity: String,
    canonical_ref: String,
    canonical_content_digest: String,
    owner_revision: u64,
    domain: LifeDomain,
    risk_level: RiskLevel,
    sensitivity: LifeEventSensitivity,
    privacy_level: LifeEventPrivacyLevel,
    policy_version: String,
    policy_reason: String,
    operation_id: String,
    observed_at: DateTime<Utc>,
    operation_binding_digest: String,
    runtime_binding_digest: String,
    runtime_nonce: Uuid,
}

impl LifeEventCreatePermit {
    fn from_verified_authorities(
        message_proof: &crate::memory::CanonicalConversationMessageProof,
        policy_proof: &crate::agent::main_chat_memory_candidate::DeterministicLifeEventPolicyProof,
        execution_proof: &crate::agent::store::CanonicalAgentRunLifeEventExecutionProof,
        operation_id: String,
        draft_digest: String,
    ) -> Self {
        let mut permit = Self {
            current_user_message_id: format!("message-{}", message_proof.message_id()),
            current_user_message_store_identity: message_proof
                .canonical_store_identity()
                .to_string(),
            current_user_message_digest: message_proof.content_digest().to_string(),
            candidate_id: policy_proof.candidate_id().to_string(),
            draft_digest,
            task_id: execution_proof.task_id().to_string(),
            run_id: execution_proof.run_id().to_string(),
            execution_id: execution_proof.execution_id().to_string(),
            canonical_store_identity: execution_proof.canonical_store_identity().to_string(),
            canonical_ref: execution_proof.canonical_ref().to_string(),
            canonical_content_digest: execution_proof.canonical_content_digest().to_string(),
            owner_revision: execution_proof.owner_revision(),
            domain: LifeDomain::Other,
            risk_level: policy_proof.risk_level(),
            sensitivity: policy_proof.sensitivity(),
            privacy_level: LifeEventPrivacyLevel::Internal,
            policy_version: "life_event_policy_v2".into(),
            policy_reason: "deterministic_current_user_low_risk_life_event".into(),
            operation_id,
            observed_at: Utc::now(),
            operation_binding_digest: String::new(),
            runtime_binding_digest: String::new(),
            runtime_nonce: Uuid::new_v4(),
        };
        permit.operation_binding_digest = sha256_hex(permit.operation_material().as_bytes());
        permit.runtime_binding_digest = sha256_hex(permit.runtime_material().as_bytes());
        permit
    }

    fn operation_material(&self) -> String {
        format!(
            "authorization_source\0current_authenticated_user_message\0message_id\0{}:{}\0message_store\0{}:{}\0message_digest\0{}\0candidate_id\0{}:{}\0draft_digest\0{}\0task_id\0{}:{}\0run_id\0{}:{}\0execution_id\0{}:{}\0owner_store\0{}:{}\0owner_ref\0{}:{}\0owner_digest\0{}\0owner_revision\0{}\0domain\0{}\0risk\0{}\0sensitivity\0{}\0privacy\0{}\0policy_version\0{}:{}\0policy_reason\0{}:{}\0operation_id\0{}",
            self.current_user_message_id.len(),
            self.current_user_message_id,
            self.current_user_message_store_identity.len(),
            self.current_user_message_store_identity,
            self.current_user_message_digest,
            self.candidate_id.len(),
            self.candidate_id,
            self.draft_digest,
            self.task_id.len(),
            self.task_id,
            self.run_id.len(),
            self.run_id,
            self.execution_id.len(),
            self.execution_id,
            self.canonical_store_identity.len(),
            self.canonical_store_identity,
            self.canonical_ref.len(),
            self.canonical_ref,
            self.canonical_content_digest,
            self.owner_revision,
            self.domain,
            self.risk_level,
            self.sensitivity.as_str(),
            self.privacy_level,
            self.policy_version.len(),
            self.policy_version,
            self.policy_reason.len(),
            self.policy_reason,
            self.operation_id,
        )
    }

    fn runtime_material(&self) -> String {
        format!(
            "operation_binding\0{}\0runtime_nonce\0{}",
            self.operation_binding_digest, self.runtime_nonce
        )
    }

    pub(crate) fn runtime_seal_is_valid(&self) -> bool {
        self.operation_binding_digest == sha256_hex(self.operation_material().as_bytes())
            && self.runtime_binding_digest == sha256_hex(self.runtime_material().as_bytes())
    }

    pub(crate) fn matches_draft(&self, draft: &LifeEventDraft) -> bool {
        self.draft_digest == life_event_draft_digest(draft)
            && draft.source_run_id.as_deref() == Some(self.run_id.as_str())
    }

    pub(crate) fn matches_current_agent_run_owner(
        &self,
        canonical_store_identity: &str,
        canonical_ref: &str,
        canonical_content_digest: &str,
        owner_revision: u64,
        task_id: &str,
        run_id: &str,
    ) -> Result<()> {
        if self.task_id != task_id || self.run_id != run_id {
            anyhow::bail!("life_event_create_permit_execution_owner_mismatch");
        }
        if self.owner_revision != owner_revision {
            anyhow::bail!("life_event_create_permit_owner_revision_stale");
        }
        if self.canonical_store_identity != canonical_store_identity
            || self.canonical_ref != canonical_ref
            || self.canonical_content_digest != canonical_content_digest
        {
            anyhow::bail!("life_event_create_permit_owner_digest_stale");
        }
        Ok(())
    }
}

impl std::fmt::Debug for LifeEventCreatePermit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LifeEventCreatePermit")
            .field("domain", &self.domain)
            .field("risk_level", &self.risk_level)
            .field("sensitivity", &self.sensitivity)
            .field("privacy_level", &self.privacy_level)
            .field("owner_revision", &self.owner_revision)
            .field("operation_id", &self.operation_id)
            .field("authority", &"[REDACTED]")
            .finish()
    }
}

pub(crate) fn issue_life_event_create_permit(
    message_proof: &crate::memory::CanonicalConversationMessageProof,
    policy_proof: crate::agent::main_chat_memory_candidate::DeterministicLifeEventPolicyProof,
    execution_proof: &crate::agent::store::CanonicalAgentRunLifeEventExecutionProof,
    operation_id: &str,
) -> Result<(LifeEventCreatePermit, LifeEventDraft)> {
    if message_proof.role() != "user" || !policy_proof.matches_message(message_proof) {
        anyhow::bail!("life_event_create_current_authenticated_user_message_required");
    }
    if !execution_proof.runtime_seal_is_valid() {
        anyhow::bail!("life_event_create_execution_owner_proof_invalid");
    }
    if execution_proof.input_message_ref() != message_proof.canonical_ref() {
        anyhow::bail!("life_event_create_execution_message_ref_mismatch");
    }
    if execution_proof.input_message_store_identity() != message_proof.canonical_store_identity() {
        anyhow::bail!("life_event_create_execution_message_store_mismatch");
    }
    if policy_proof.risk_level() != RiskLevel::Low {
        anyhow::bail!("life_event_create_risk_requires_review");
    }
    if policy_proof.sensitivity() != LifeEventSensitivity::Low {
        anyhow::bail!("life_event_create_sensitivity_requires_review");
    }
    let parsed_operation =
        Uuid::parse_str(operation_id).context("life_event_create_operation_id_invalid")?;
    if parsed_operation.get_version() != Some(uuid::Version::Random)
        || parsed_operation.hyphenated().to_string() != operation_id
    {
        anyhow::bail!("life_event_create_operation_id_requires_uuid_v4");
    }
    let draft = LifeEventDraft::new(
        "main_chat.episodic_life_event",
        policy_proof.normalized_claim(),
    )
    .with_source_run_id(execution_proof.run_id())
    .with_metadata(json!({
        "confidence": policy_proof.confidence(),
        "proposal_only": false
    }));
    let permit = LifeEventCreatePermit::from_verified_authorities(
        message_proof,
        &policy_proof,
        execution_proof,
        operation_id.to_string(),
        life_event_draft_digest(&draft),
    );
    if !permit.runtime_seal_is_valid() {
        anyhow::bail!("life_event_create_permit_runtime_seal_invalid");
    }
    Ok((permit, draft))
}

pub(crate) fn life_event_draft_digest(draft: &LifeEventDraft) -> String {
    let metadata = canonical_life_event_json_material(&draft.metadata);
    sha256_hex(
        format!(
            "event_type\0{}:{}\0summary\0{}:{}\0source_run_id\0{}:{}\0metadata\0{}:{}",
            draft.event_type.len(),
            draft.event_type,
            draft.summary.len(),
            draft.summary,
            draft.source_run_id.as_deref().unwrap_or("").len(),
            draft.source_run_id.as_deref().unwrap_or(""),
            metadata.len(),
            metadata,
        )
        .as_bytes(),
    )
}

fn canonical_life_event_json_material(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into()),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_life_event_json_material)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(values) => {
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by_key(|(left, _)| *left);
            format!(
                "{{{}}}",
                entries
                    .into_iter()
                    .map(|(key, value)| format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_else(|_| "\"\"".into()),
                        canonical_life_event_json_material(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

/// Legacy/read-model lineage evidence issued while a canonical owner row is
/// under lookup. This is deliberately not a product write authorization:
/// release builds reject the legacy source-proof create path, and the new
/// gateway accepts only `LifeEventCreatePermit`.
#[derive(Debug, PartialEq, Eq)]
#[cfg(any(test, feature = "test-utils"))]
pub struct CanonicalLifeEventSourceProof {
    source_type: LifeEventSourceType,
    source_id: String,
    source_detail: Option<String>,
    observed_at: DateTime<Utc>,
    canonical_owner: CanonicalLifeEventOwnerRef,
    runtime_binding_digest: String,
    _runtime_nonce: Uuid,
}

#[cfg(any(test, feature = "test-utils"))]
impl CanonicalLifeEventSourceProof {
    pub(crate) fn from_agent_run_lookup(
        seal: crate::agent::store::CanonicalAgentRunLifeEventSourceSeal,
        source_detail: Option<&str>,
    ) -> Self {
        let mut proof = Self {
            source_type: LifeEventSourceType::AgentRun,
            source_id: seal.run_id().to_string(),
            source_detail: source_detail.map(str::to_string),
            observed_at: Utc::now(),
            canonical_owner: CanonicalLifeEventOwnerRef {
                owner_kind: CanonicalLifeEventOwnerKind::AgentRun,
                canonical_store_identity: seal.canonical_store_identity().to_string(),
                canonical_ref: seal.canonical_ref().to_string(),
                canonical_content_digest: seal.content_digest().to_string(),
                canonical_revision: None,
            },
            runtime_binding_digest: String::new(),
            _runtime_nonce: Uuid::new_v4(),
        };
        proof.runtime_binding_digest = sha256_hex(proof.runtime_material().as_bytes());
        proof
    }

    fn runtime_material(&self) -> String {
        format!(
            "source_type\0{}\0source_id\0{}:{}\0source_detail\0{}:{}\0owner_kind\0{}\0store_identity\0{}:{}\0canonical_ref\0{}:{}\0content_digest\0{}:{}",
            self.source_type,
            self.source_id.len(),
            self.source_id,
            self.source_detail.as_deref().unwrap_or("").len(),
            self.source_detail.as_deref().unwrap_or(""),
            self.canonical_owner.owner_kind.as_str(),
            self.canonical_owner.canonical_store_identity.len(),
            self.canonical_owner.canonical_store_identity,
            self.canonical_owner.canonical_ref.len(),
            self.canonical_owner.canonical_ref,
            self.canonical_owner.canonical_content_digest.len(),
            self.canonical_owner.canonical_content_digest,
        )
    }

    fn runtime_seal_is_valid(&self) -> bool {
        self.runtime_binding_digest == sha256_hex(self.runtime_material().as_bytes())
    }

    pub fn source_type(&self) -> LifeEventSourceType {
        self.source_type
    }

    pub fn source_id(&self) -> &str {
        &self.source_id
    }
}

impl LifeEventSourceRef {
    #[cfg(any(test, feature = "test-utils"))]
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
            canonical_owner: None,
            verification: LifeEventSourceVerification::LegacyUnverified,
        }
    }

    #[cfg(any(test, feature = "test-utils"))]
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

impl LifeEvent {
    pub fn has_canonical_source_authority(&self) -> bool {
        !self.source_refs.is_empty()
            && self.source_refs.iter().all(|source| {
                source.verification == LifeEventSourceVerification::CanonicalOwnerVerified
                    && source.canonical_owner.is_some()
            })
    }
}

pub struct LifeEventStore {
    conn: Mutex<Connection>,
    receipt_key: Arc<AgentRunReceiptKey>,
}

struct LifeEventCreateOperation {
    operation_id: String,
    binding_digest: String,
}

impl LifeEventStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        #[cfg(any(test, feature = "test-utils"))]
        {
            Self::new_with_receipt_key(db_path, AgentRunReceiptKey::test_key())
        }
        #[cfg(not(any(test, feature = "test-utils")))]
        {
            let _ = db_path.into();
            anyhow::bail!("life_event_receipt_key_required");
        }
    }

    pub fn new_with_receipt_key(
        db_path: impl Into<PathBuf>,
        receipt_key: AgentRunReceiptKey,
    ) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open life event db at {:?}", db_path))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA secure_delete = ON;")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        let store = Self {
            conn: Mutex::new(conn),
            receipt_key: Arc::new(receipt_key),
        };
        store.init_tables()?;
        store.validate_current_rows()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        #[cfg(any(test, feature = "test-utils"))]
        {
            Self::new_in_memory_with_receipt_key(AgentRunReceiptKey::test_key())
        }
        #[cfg(not(any(test, feature = "test-utils")))]
        {
            anyhow::bail!("life_event_receipt_key_required");
        }
    }

    pub fn new_in_memory_with_receipt_key(receipt_key: AgentRunReceiptKey) -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory event db")?;
        conn.execute_batch("PRAGMA secure_delete = ON;")?;
        let store = Self {
            conn: Mutex::new(conn),
            receipt_key: Arc::new(receipt_key),
        };
        store.init_tables()?;
        store.validate_current_rows()?;
        Ok(store)
    }

    pub fn open_read_only_existing(db_path: impl Into<PathBuf>) -> Result<Self> {
        #[cfg(any(test, feature = "test-utils"))]
        {
            Self::open_read_only_existing_with_receipt_key(db_path, AgentRunReceiptKey::test_key())
        }
        #[cfg(not(any(test, feature = "test-utils")))]
        {
            let _ = db_path.into();
            anyhow::bail!("life_event_receipt_key_required");
        }
    }

    pub fn open_read_only_existing_with_receipt_key(
        db_path: impl Into<PathBuf>,
        receipt_key: AgentRunReceiptKey,
    ) -> Result<Self> {
        let db_path = db_path.into();
        let conn = crate::sqlite_migration::open_existing_read_only(
            &db_path,
            "life_event_store",
            &["life_events"],
        )?;
        let store = Self {
            conn: Mutex::new(conn),
            receipt_key: Arc::new(receipt_key),
        };
        if !store.physical_purge_complete()? {
            anyhow::bail!("life_event_v2_physical_purge_incomplete");
        }
        store.validate_current_rows()?;
        Ok(store)
    }

    pub fn bind_canonical_memory_store(
        &self,
        memory_store: &crate::memory::MemoryStore,
    ) -> Result<()> {
        self.bind_canonical_source_owner(
            CanonicalLifeEventOwnerKind::ConversationMessage,
            memory_store.canonical_store_identity(),
        )
    }

    pub fn bind_canonical_agent_run_store(
        &self,
        agent_run_store: &crate::agent::store::AgentRunStore,
    ) -> Result<()> {
        self.bind_canonical_source_owner(
            CanonicalLifeEventOwnerKind::AgentRun,
            &agent_run_store.canonical_store_identity()?,
        )
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub fn create_canonical_agent_run_event_for_test(
        &self,
        agent_run_store: &crate::agent::store::AgentRunStore,
        run_id: &str,
        source_detail: Option<&str>,
        draft: LifeEventDraft,
        domain: LifeDomain,
        risk_level: RiskLevel,
        privacy_level: LifeEventPrivacyLevel,
    ) -> Result<LifeEvent> {
        self.bind_canonical_agent_run_store(agent_run_store)?;
        agent_run_store.create_life_event_from_active_run(
            self,
            run_id,
            source_detail,
            draft,
            domain,
            risk_level,
            privacy_level,
        )
    }

    fn bind_canonical_source_owner(
        &self,
        owner_kind: CanonicalLifeEventOwnerKind,
        canonical_store_identity: &str,
    ) -> Result<()> {
        if !is_canonical_owner_store_identity(owner_kind, canonical_store_identity) {
            anyhow::bail!("life_event_canonical_source_owner_identity_invalid");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow!("mutex poison: {error}"))?;
        let key = life_event_source_owner_metadata_key(owner_kind);
        let existing = conn
            .query_row(
                "SELECT value FROM life_event_store_metadata WHERE key = ?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match existing {
            Some(existing) if existing != canonical_store_identity => {
                anyhow::bail!("life_event_canonical_source_owner_identity_conflict")
            }
            Some(_) => Ok(()),
            None => {
                conn.execute(
                    "INSERT INTO life_event_store_metadata(key, value) VALUES (?1, ?2)",
                    params![key, canonical_store_identity],
                )?;
                Ok(())
            }
        }
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
                created_at TEXT NOT NULL,
                payload_minimized_version INTEGER NOT NULL DEFAULT 2
            )",
            [],
        )?;
        add_life_event_column_if_missing(
            &conn,
            "payload_minimized_version",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS life_event_store_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             ) WITHOUT ROWID",
            [],
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS life_event_create_operations (
                operation_id TEXT PRIMARY KEY,
                binding_digest TEXT NOT NULL,
                binding_receipt TEXT NOT NULL,
                event_id TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                FOREIGN KEY(event_id) REFERENCES life_events(id) ON DELETE RESTRICT
             ) WITHOUT ROWID;",
        )?;
        let legacy_rows = {
            let mut statement = conn.prepare(
                "SELECT id, event_type, source_refs_json, summary, payload_digest,
                        metadata_json, domain, risk_level, privacy_level,
                        payload_minimized_version
                 FROM life_events WHERE payload_minimized_version < ?1",
            )?;
            let rows = statement
                .query_map([LIFE_EVENT_PAYLOAD_VERSION], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, i64>(9)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        if !legacy_rows.is_empty() {
            let tx = conn.unchecked_transaction()?;
            for (
                id,
                event_type,
                refs_json,
                summary,
                old_payload_digest,
                metadata_json,
                domain,
                risk_level,
                privacy_level,
                payload_version,
            ) in legacy_rows
            {
                let refs = serde_json::from_str::<Vec<LifeEventSourceRef>>(&refs_json)
                    .context("invalid legacy LifeEvent source refs")?
                    .into_iter()
                    .map(|source| self.persisted_source_ref_for_migration(source))
                    .collect::<Result<Vec<_>>>()?;
                if refs.len() > MAX_LIFE_EVENT_SOURCE_REFS {
                    anyhow::bail!("legacy LifeEvent source lineage limit exceeded");
                }
                let primary = refs
                    .first()
                    .context("legacy LifeEvent source lineage missing")?;
                let (event_type, legacy_event_type_receipt) =
                    normalize_legacy_life_event_type(self.receipt_key.as_ref(), &event_type);
                let summary_receipt = if payload_version >= 1 && is_exact_life_event_hmac(&summary)
                {
                    summary
                } else {
                    self.receipt_key.sign(
                        "life_event_summary",
                        &format!(
                            "event_type\0{}\0source_type\0{}\0source_id\0{}\0summary\0{}:{}",
                            event_type,
                            primary.source_type,
                            primary.source_id,
                            summary.len(),
                            summary,
                        ),
                    )
                };
                let metadata = serde_json::from_str::<Value>(&metadata_json)
                    .context("invalid legacy LifeEvent metadata")?;
                let mut metadata =
                    sanitize_event_metadata(&metadata, self.receipt_key.as_ref(), primary);
                if let Some(receipt) = legacy_event_type_receipt {
                    if let Some(object) = metadata.as_object_mut() {
                        object.insert("legacyEventTypeReceipt".into(), receipt.into());
                    }
                }
                let payload_receipt = self.receipt_key.sign(
                    "life_event_payload",
                    &life_event_payload_material(
                        &event_type,
                        &summary_receipt,
                        &domain,
                        &risk_level,
                        &privacy_level,
                        &metadata,
                        &refs,
                    ),
                );
                let _ = old_payload_digest;
                let metadata_safe_summary = json!({
                    "eventType": "metadata_safe_life_event",
                    "summaryReceipt": summary_receipt.clone(),
                    "sourceCount": refs.len(),
                    "payloadReceipt": payload_receipt.clone(),
                    "sourceAuthority": "legacy_unverified",
                });
                let dedupe_key = format!(
                    "life_event:{}:{}:{}",
                    domain,
                    event_type,
                    short_hash(&primary.digest)
                );
                tx.execute(
                    "UPDATE life_events
                     SET event_type = ?2, source_type = ?3, source_id = ?4,
                         source_refs_json = ?5, summary = ?6, payload_digest = ?7,
                         metadata_json = ?8, metadata_safe_summary_json = ?9,
                         contains_raw_content = 0, dedupe_key = ?10,
                         payload_minimized_version = ?11
                     WHERE id = ?1 AND payload_minimized_version < ?11",
                    params![
                        id,
                        event_type,
                        primary.source_type.to_string(),
                        primary.source_id.clone(),
                        serde_json::to_string(&refs)?,
                        summary_receipt,
                        payload_receipt,
                        serde_json::to_string(&metadata)?,
                        serde_json::to_string(&metadata_safe_summary)?,
                        dedupe_key,
                        LIFE_EVENT_PAYLOAD_VERSION,
                    ],
                )?;
            }
            tx.execute(
                "INSERT INTO life_event_store_metadata(key, value)
                 VALUES (?1, 'pending')
                 ON CONFLICT(key) DO UPDATE SET value = 'pending'",
                [LIFE_EVENT_V2_PHYSICAL_PURGE_MARKER],
            )?;
            tx.commit()?;
        }
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_life_events_domain_time ON life_events(domain, occurred_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_life_events_dedupe_key ON life_events(dedupe_key)",
            [],
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS life_event_tombstone_projections (
                canonical_tombstone_id TEXT NOT NULL,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0, 1)),
                applied_event_id TEXT NOT NULL,
                applied_at TEXT NOT NULL,
                PRIMARY KEY(canonical_tombstone_id, source_type, source_id)
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_life_event_active_tombstone_source
             ON life_event_tombstone_projections(source_type, source_id, active);
             CREATE TABLE IF NOT EXISTS life_event_agent_run_projection_heads (
                source_id TEXT PRIMARY KEY,
                canonical_revision INTEGER NOT NULL,
                canonical_event_id TEXT NOT NULL,
                hidden INTEGER NOT NULL CHECK(hidden IN (0, 1)),
                canonical_tombstone_id TEXT,
                applied_at TEXT NOT NULL
             );",
        )?;
        if !Self::physical_purge_complete_conn(&conn)? {
            Self::complete_physical_purge(&conn)?;
        }
        Ok(())
    }

    fn physical_purge_complete(&self) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow!("mutex poison: {error}"))?;
        Self::physical_purge_complete_conn(&conn)
    }

    fn physical_purge_complete_conn(conn: &Connection) -> Result<bool> {
        Ok(conn
            .query_row(
                "SELECT value FROM life_event_store_metadata WHERE key = ?1",
                [LIFE_EVENT_V2_PHYSICAL_PURGE_MARKER],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .as_deref()
            == Some("complete"))
    }

    fn checkpoint_wal(conn: &Connection) -> Result<()> {
        let (busy, log_frames, checkpointed_frames): (i64, i64, i64) =
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
        if busy != 0 || log_frames != 0 || checkpointed_frames != 0 {
            anyhow::bail!(
                "life_event_v2_wal_checkpoint_incomplete:{busy}:{log_frames}:{checkpointed_frames}"
            );
        }
        Ok(())
    }

    fn complete_physical_purge(conn: &Connection) -> Result<()> {
        let database_path: String = conn.query_row(
            "SELECT file FROM pragma_database_list WHERE name = 'main'",
            [],
            |row| row.get(0),
        )?;
        if database_path.is_empty() {
            conn.execute_batch("VACUUM;")?;
        } else {
            Self::checkpoint_wal(conn)?;
            conn.execute_batch("VACUUM;")?;
            Self::checkpoint_wal(conn)?;
            let wal_path = PathBuf::from(format!("{database_path}-wal"));
            if wal_path.exists() && std::fs::metadata(wal_path)?.len() != 0 {
                anyhow::bail!("life_event_v2_wal_not_truncated");
            }
        }
        let freelist_count: i64 = conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
        if freelist_count != 0 {
            anyhow::bail!("life_event_v2_freelist_not_reclaimed");
        }
        conn.execute(
            "INSERT INTO life_event_store_metadata(key, value)
             VALUES (?1, 'complete')
             ON CONFLICT(key) DO UPDATE SET value = 'complete'",
            [LIFE_EVENT_V2_PHYSICAL_PURGE_MARKER],
        )?;
        Ok(())
    }

    #[cfg(any(test, feature = "test-utils"))]
    fn persisted_source_ref(
        &self,
        proof: CanonicalLifeEventSourceProof,
    ) -> Result<LifeEventSourceRef> {
        if !proof.runtime_seal_is_valid()
            || !is_typed_life_event_source_id(proof.source_type, &proof.source_id)
        {
            anyhow::bail!("life_event_source_id_invalid");
        }
        if proof
            .source_detail
            .as_deref()
            .is_some_and(|detail| !is_typed_life_event_source_detail(detail))
        {
            anyhow::bail!("life_event_source_detail_invalid");
        }
        let owner_matches_type = matches!(
            (proof.source_type, proof.canonical_owner.owner_kind),
            (
                LifeEventSourceType::ChatMessage,
                CanonicalLifeEventOwnerKind::ConversationMessage
            ) | (
                LifeEventSourceType::AgentRun,
                CanonicalLifeEventOwnerKind::AgentRun
            )
        );
        if !owner_matches_type
            || !is_canonical_owner_store_identity(
                proof.canonical_owner.owner_kind,
                &proof.canonical_owner.canonical_store_identity,
            )
            || !is_canonical_owner_ref(
                proof.canonical_owner.owner_kind,
                &proof.canonical_owner.canonical_ref,
                &proof.source_id,
            )
            || !is_exact_sha256_digest(&proof.canonical_owner.canonical_content_digest)
        {
            anyhow::bail!("life_event_canonical_source_proof_invalid");
        }
        let mut source = LifeEventSourceRef {
            source_type: proof.source_type,
            source_id: proof.source_id,
            source_detail: proof.source_detail,
            digest: String::new(),
            observed_at: proof.observed_at,
            canonical_owner: Some(proof.canonical_owner),
            verification: LifeEventSourceVerification::CanonicalOwnerVerified,
        };
        source.digest = self.receipt_key.sign(
            "life_event_canonical_source_ref",
            &life_event_source_material(&source),
        );
        Ok(source)
    }

    fn persisted_source_ref_from_permit(
        &self,
        permit: &LifeEventCreatePermit,
    ) -> Result<LifeEventSourceRef> {
        if !permit.runtime_seal_is_valid()
            || !is_typed_life_event_source_id(LifeEventSourceType::AgentRun, &permit.run_id)
            || !is_typed_life_event_source_detail(&permit.candidate_id)
            || !is_canonical_owner_store_identity(
                CanonicalLifeEventOwnerKind::AgentRun,
                &permit.canonical_store_identity,
            )
            || !is_canonical_owner_ref(
                CanonicalLifeEventOwnerKind::AgentRun,
                &permit.canonical_ref,
                &permit.run_id,
            )
            || !is_exact_sha256_digest(&permit.canonical_content_digest)
            || permit.owner_revision == 0
        {
            anyhow::bail!("life_event_create_permit_source_binding_invalid");
        }
        let mut source = LifeEventSourceRef {
            source_type: LifeEventSourceType::AgentRun,
            source_id: permit.run_id.clone(),
            source_detail: Some(permit.candidate_id.clone()),
            digest: String::new(),
            observed_at: permit.observed_at,
            canonical_owner: Some(CanonicalLifeEventOwnerRef {
                owner_kind: CanonicalLifeEventOwnerKind::AgentRun,
                canonical_store_identity: permit.canonical_store_identity.clone(),
                canonical_ref: permit.canonical_ref.clone(),
                canonical_content_digest: permit.canonical_content_digest.clone(),
                canonical_revision: Some(permit.owner_revision),
            }),
            verification: LifeEventSourceVerification::CanonicalOwnerVerified,
        };
        source.digest = self.receipt_key.sign(
            "life_event_canonical_source_ref",
            &life_event_source_material(&source),
        );
        Ok(source)
    }

    fn persisted_source_ref_for_migration(
        &self,
        source: LifeEventSourceRef,
    ) -> Result<LifeEventSourceRef> {
        let mut source = source;
        if !is_typed_life_event_source_id(source.source_type, &source.source_id) {
            source.source_id = format!(
                "legacy-source:{}",
                self.receipt_key.sign(
                    "life_event_legacy_source_id",
                    &format!(
                        "source_type\0{}\0source_id\0{}:{}",
                        source.source_type,
                        source.source_id.len(),
                        source.source_id
                    ),
                )
            );
        }
        if source
            .source_detail
            .as_deref()
            .is_some_and(|detail| !is_typed_life_event_source_detail(detail))
        {
            source.source_detail = source.source_detail.as_deref().map(|detail| {
                life_event_text_receipt(self.receipt_key.as_ref(), "source_detail", detail)
            });
        }
        source.canonical_owner = None;
        source.verification = LifeEventSourceVerification::LegacyUnverified;
        source.digest = self.receipt_key.sign(
            "life_event_legacy_unverified_source_ref",
            &life_event_source_material(&source),
        );
        Ok(source)
    }

    fn validate_current_rows(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow!("mutex poison: {error}"))?;
        let mut statement = conn.prepare(&format!(
            "SELECT {} FROM life_events ORDER BY id",
            Self::columns()
        ))?;
        let rows = statement.query_map([], |row| self.row_to_event(row))?;
        let events = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        for event in &events {
            ensure_bound_life_event_source_owners(&conn, &event.source_refs)?;
        }
        self.validate_current_create_operations(&conn)?;
        Ok(())
    }

    fn validate_current_create_operations(&self, conn: &Connection) -> Result<()> {
        let mut statement = conn.prepare(
            "SELECT operation_id, binding_digest, binding_receipt, event_id, created_at
             FROM life_event_create_operations ORDER BY operation_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        for row in rows {
            let (operation_id, binding_digest, binding_receipt, event_id, created_at) = row?;
            let parsed =
                Uuid::parse_str(&operation_id).context("life_event_create_operation_id_invalid")?;
            if parsed.get_version() != Some(uuid::Version::Random)
                || parsed.hyphenated().to_string() != operation_id
                || !is_exact_sha256_digest(&binding_digest)
                || !event_id.strip_prefix("le_").is_some_and(|suffix| {
                    suffix.len() == 32
                        && suffix
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                })
                || DateTime::parse_from_rfc3339(&created_at).is_err()
                || !self.receipt_key.verify(
                    "life_event_create_operation_binding",
                    &life_event_create_operation_material(
                        &operation_id,
                        &binding_digest,
                        &event_id,
                    ),
                    &binding_receipt,
                )
            {
                anyhow::bail!("life_event_create_operation_row_invalid");
            }
            let event_exists = conn
                .query_row(
                    "SELECT 1 FROM life_events WHERE id = ?1",
                    [&event_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !event_exists {
                anyhow::bail!("life_event_create_operation_event_missing");
            }
        }
        Ok(())
    }

    pub(crate) fn create_event_with_permit(
        &self,
        permit: LifeEventCreatePermit,
        draft: LifeEventDraft,
    ) -> Result<LifeEvent> {
        if !permit.runtime_seal_is_valid() {
            anyhow::bail!("life_event_create_permit_runtime_seal_invalid");
        }
        if !permit.matches_draft(&draft) {
            anyhow::bail!("life_event_create_permit_draft_mismatch");
        }
        if permit.sensitivity != LifeEventSensitivity::Low
            || permit.risk_level != RiskLevel::Low
            || matches!(
                permit.privacy_level,
                LifeEventPrivacyLevel::Sensitive | LifeEventPrivacyLevel::StrictlyLocal
            )
            || permit.policy_version != "life_event_policy_v2"
            || permit.policy_reason != "deterministic_current_user_low_risk_life_event"
        {
            anyhow::bail!("life_event_create_permit_policy_binding_invalid");
        }
        let source_ref = self.persisted_source_ref_from_permit(&permit)?;
        let operation = LifeEventCreateOperation {
            operation_id: permit.operation_id.clone(),
            binding_digest: permit.operation_binding_digest.clone(),
        };
        self.create_event_from_verified_source_refs(
            draft,
            vec![source_ref],
            permit.domain,
            permit.risk_level,
            permit.privacy_level,
            Some(operation),
        )
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub(crate) fn create_event_from_canonical_sources(
        &self,
        draft: LifeEventDraft,
        source_proofs: Vec<CanonicalLifeEventSourceProof>,
        domain: LifeDomain,
        risk_level: RiskLevel,
        privacy_level: LifeEventPrivacyLevel,
    ) -> Result<LifeEvent> {
        if !cfg!(any(test, feature = "test-utils")) {
            anyhow::bail!("legacy_life_event_source_proof_write_retired");
        }
        let mut blockers = Vec::new();
        if !is_registered_life_event_type(&draft.event_type) {
            push_unique(&mut blockers, "event_type_unregistered");
        }
        if source_proofs.is_empty() {
            push_unique(&mut blockers, "source_lineage_missing");
        }
        if source_proofs.len() > MAX_LIFE_EVENT_SOURCE_REFS {
            push_unique(&mut blockers, "source_lineage_limit_exceeded");
        }
        if draft.summary.trim().is_empty() {
            push_unique(&mut blockers, "metadata_safe_summary_missing");
        }
        if draft.summary.len() > MAX_LIFE_EVENT_METADATA_BYTES
            || serde_json::to_vec(&draft.metadata)
                .map(|payload| payload.len() > MAX_LIFE_EVENT_METADATA_BYTES)
                .unwrap_or(true)
        {
            push_unique(&mut blockers, "life_event_transient_payload_limit_exceeded");
        }
        if !blockers.is_empty() {
            return Err(anyhow!("life event blocked: {}", blockers.join(",")));
        }

        let source_refs = source_proofs
            .into_iter()
            .map(|source| self.persisted_source_ref(source))
            .collect::<Result<Vec<_>>>()?;
        self.create_event_from_verified_source_refs(
            draft,
            source_refs,
            domain,
            risk_level,
            privacy_level,
            None,
        )
    }

    fn create_event_from_verified_source_refs(
        &self,
        draft: LifeEventDraft,
        source_refs: Vec<LifeEventSourceRef>,
        domain: LifeDomain,
        risk_level: RiskLevel,
        privacy_level: LifeEventPrivacyLevel,
        operation: Option<LifeEventCreateOperation>,
    ) -> Result<LifeEvent> {
        if draft.source_run_id.as_deref().is_some_and(|source_run_id| {
            !source_refs.iter().any(|source| {
                source.verification == LifeEventSourceVerification::CanonicalOwnerVerified
                    && source.source_type == LifeEventSourceType::AgentRun
                    && source.source_id == source_run_id
            })
        }) {
            anyhow::bail!("life_event_source_run_proof_mismatch");
        }
        let primary_source = source_refs
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("life event blocked: source_lineage_missing"))?;
        let now = Utc::now();
        let metadata =
            sanitize_event_metadata(&draft.metadata, self.receipt_key.as_ref(), &primary_source);
        let summary_receipt = self.receipt_key.sign(
            "life_event_summary",
            &format!(
                "event_type\0{}\0source_type\0{}\0source_id\0{}\0summary\0{}:{}",
                draft.event_type,
                primary_source.source_type,
                primary_source.source_id,
                draft.summary.len(),
                draft.summary,
            ),
        );
        let payload_material = life_event_payload_material(
            &draft.event_type,
            &summary_receipt,
            &domain.to_string(),
            &risk_level.to_string(),
            &privacy_level.to_string(),
            &metadata,
            &source_refs,
        );
        let payload_digest = self
            .receipt_key
            .sign("life_event_payload", &payload_material);
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
            summary: summary_receipt.clone(),
            payload_digest: payload_digest.clone(),
            metadata,
            metadata_safe_summary: Some(json!({
                "eventType": "metadata_safe_life_event",
                "domain": domain.to_string(),
                "summaryReceipt": summary_receipt,
                "sourceCount": source_count,
                "payloadReceipt": payload_digest,
                "sourceAuthority": "canonical_owner_verified"
            })),
            contains_raw_content: false,
            dedupe_key,
            created_at: now,
        };
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(operation) = operation.as_ref() {
            let existing = tx
                .query_row(
                    "SELECT binding_digest, binding_receipt, event_id
                     FROM life_event_create_operations WHERE operation_id = ?1",
                    [&operation.operation_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((binding_digest, binding_receipt, event_id)) = existing {
                if binding_digest != operation.binding_digest {
                    anyhow::bail!("life_event_create_operation_binding_conflict");
                }
                if !self.receipt_key.verify(
                    "life_event_create_operation_binding",
                    &life_event_create_operation_material(
                        &operation.operation_id,
                        &binding_digest,
                        &event_id,
                    ),
                    &binding_receipt,
                ) {
                    anyhow::bail!("life_event_create_operation_receipt_invalid");
                }
                let query = format!("SELECT {} FROM life_events WHERE id = ?1", Self::columns());
                let replay = tx
                    .query_row(&query, [&event_id], |row| self.row_to_event(row))
                    .optional()?
                    .context("life_event_create_operation_event_missing")?;
                tx.commit()?;
                return Ok(replay);
            }
        }
        ensure_bound_life_event_source_owners(&tx, &event.source_refs)?;
        if life_event_hidden(&active_life_event_tombstones(&tx)?, &event) {
            anyhow::bail!("life_event_canonical_source_tombstoned");
        }
        tx.execute(
            "INSERT INTO life_events (
                id, event_type, source_type, source_id, source_refs_json,
                occurred_at, domain, risk_level, privacy_level, summary,
                payload_digest, metadata_json, metadata_safe_summary_json,
                contains_raw_content, dedupe_key, created_at,
                payload_minimized_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
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
                LIFE_EVENT_PAYLOAD_VERSION,
            ],
        )?;
        if let Some(operation) = operation {
            let binding_receipt = self.receipt_key.sign(
                "life_event_create_operation_binding",
                &life_event_create_operation_material(
                    &operation.operation_id,
                    &operation.binding_digest,
                    &event.id,
                ),
            );
            tx.execute(
                "INSERT INTO life_event_create_operations (
                    operation_id, binding_digest, binding_receipt, event_id, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    operation.operation_id,
                    operation.binding_digest,
                    binding_receipt,
                    event.id,
                    event.created_at.to_rfc3339(),
                ],
            )?;
        }
        tx.commit()?;
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
            let rows = stmt.query_map([domain.to_string()], |row| self.row_to_event(row))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            let mut stmt = conn.prepare(&format!(
                "SELECT {} FROM life_events ORDER BY occurred_at DESC",
                Self::columns()
            ))?;
            let rows = stmt.query_map([], |row| self.row_to_event(row))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        for event in &records {
            ensure_bound_life_event_source_owners(&conn, &event.source_refs)?;
        }
        let tombstones = active_life_event_tombstones(&conn)?;
        records.retain(|event| !life_event_hidden(&tombstones, event));
        if let Some(limit) = limit {
            records.truncate(limit);
        }
        Ok(records)
    }

    /// Hide LifeEvent projections linked to a canonical source tombstone. The
    /// operation is metadata-only and idempotent; no source content is copied
    /// into the projection marker.
    pub fn project_source_tombstone(
        &self,
        event_id: &str,
        tombstone_id: &str,
        source_type: LifeEventSourceType,
        source_ids: &[String],
    ) -> Result<usize> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        let source_type = source_type.to_string();
        let mut applied = 0usize;
        for source_id in source_ids {
            if source_id.trim().is_empty() {
                continue;
            }
            applied += tx.execute(
                "INSERT INTO life_event_tombstone_projections (
                    canonical_tombstone_id, source_type, source_id, active,
                    applied_event_id, applied_at
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5)
                 ON CONFLICT(canonical_tombstone_id, source_type, source_id)
                 DO NOTHING",
                params![tombstone_id, source_type, source_id, event_id, now],
            )?;
        }
        tx.commit()?;
        Ok(applied)
    }

    pub fn project_agent_run_canonical_head(
        &self,
        event_id: &str,
        canonical_revision: u64,
        source_id: &str,
        current_tombstone_id: Option<&str>,
        known_tombstone_ids: &[String],
    ) -> Result<usize> {
        if event_id.trim().is_empty()
            || source_id.trim().is_empty()
            || canonical_revision == 0
            || known_tombstone_ids
                .iter()
                .any(|tombstone_id| tombstone_id.trim().is_empty())
            || current_tombstone_id.is_some_and(|id| id.trim().is_empty())
            || current_tombstone_id
                .is_some_and(|id| !known_tombstone_ids.iter().any(|known| known == id))
        {
            anyhow::bail!("invalid LifeEvent AgentRun canonical projection head");
        }
        let canonical_revision = i64::try_from(canonical_revision)
            .context("LifeEvent AgentRun projection revision overflow")?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current_head = tx
            .query_row(
                "SELECT canonical_revision, canonical_event_id, hidden,
                        canonical_tombstone_id
                 FROM life_event_agent_run_projection_heads WHERE source_id = ?1",
                [source_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?;
        if let Some((revision, current_event_id, hidden, tombstone_id)) = current_head {
            if revision > canonical_revision {
                anyhow::bail!(
                    "LifeEvent AgentRun projection ahead of canonical source: target_revision={revision}, canonical_revision={canonical_revision}"
                );
            }
            if revision == canonical_revision
                && (current_event_id != event_id
                    || hidden != i64::from(current_tombstone_id.is_some())
                    || tombstone_id.as_deref() != current_tombstone_id)
            {
                anyhow::bail!("LifeEvent AgentRun projection revision identity conflict");
            }
            if revision == canonical_revision {
                tx.commit()?;
                return Ok(0);
            }
        }
        let source_type = LifeEventSourceType::AgentRun.to_string();
        let now = Utc::now().to_rfc3339();
        let mut changed = 0usize;
        for tombstone_id in known_tombstone_ids {
            changed += tx.execute(
                "INSERT INTO life_event_tombstone_projections (
                    canonical_tombstone_id, source_type, source_id, active,
                    applied_event_id, applied_at
                 ) VALUES (?1, ?2, ?3, 0, ?4, ?5)
                 ON CONFLICT(canonical_tombstone_id, source_type, source_id)
                 DO UPDATE SET active = 0,
                               applied_event_id = excluded.applied_event_id,
                               applied_at = excluded.applied_at",
                params![tombstone_id, source_type, source_id, event_id, now],
            )?;
        }
        if let Some(tombstone_id) = current_tombstone_id {
            changed += tx.execute(
                "UPDATE life_event_tombstone_projections
                 SET active = 1, applied_event_id = ?4, applied_at = ?5
                 WHERE canonical_tombstone_id = ?1
                   AND source_type = ?2 AND source_id = ?3",
                params![tombstone_id, source_type, source_id, event_id, now],
            )?;
        }
        tx.execute(
            "INSERT INTO life_event_agent_run_projection_heads (
                source_id, canonical_revision, canonical_event_id, hidden,
                canonical_tombstone_id, applied_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(source_id) DO UPDATE SET
                canonical_revision = excluded.canonical_revision,
                canonical_event_id = excluded.canonical_event_id,
                hidden = excluded.hidden,
                canonical_tombstone_id = excluded.canonical_tombstone_id,
                applied_at = excluded.applied_at
             WHERE excluded.canonical_revision >= canonical_revision",
            params![
                source_id,
                canonical_revision,
                event_id,
                i64::from(current_tombstone_id.is_some()),
                current_tombstone_id,
                now,
            ],
        )?;
        tx.commit()?;
        Ok(changed)
    }

    pub fn agent_run_projection_head_for_test(
        &self,
        source_id: &str,
    ) -> Result<Option<(u64, bool, Option<String>)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT canonical_revision, hidden, canonical_tombstone_id
             FROM life_event_agent_run_projection_heads WHERE source_id = ?1",
            [source_id],
            |row| {
                let revision: i64 = row.get(0)?;
                let revision = u64::try_from(revision).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(0, Type::Integer, Box::new(error))
                })?;
                Ok((revision, row.get::<_, i64>(1)? != 0, row.get(2)?))
            },
        )
        .optional()
        .map_err(Into::into)
    }

    fn columns() -> &'static str {
        "id, event_type, source_type, source_id, source_refs_json, occurred_at,
         domain, risk_level, privacy_level, summary, payload_digest, metadata_json,
         metadata_safe_summary_json, contains_raw_content, dedupe_key, created_at,
         payload_minimized_version"
    }

    fn row_to_event(&self, row: &Row<'_>) -> rusqlite::Result<LifeEvent> {
        let source_type: String = row.get(2)?;
        let occurred_at: String = row.get(5)?;
        let domain: String = row.get(6)?;
        let risk_level: String = row.get(7)?;
        let privacy_level: String = row.get(8)?;
        let created_at: String = row.get(15)?;
        let payload_version: i64 = row.get(16)?;
        let parsed_source_type = LifeEventSourceType::from_str(&source_type)
            .ok_or_else(|| life_event_row_fault(2, "source_type_enum_not_exact"))?;
        let parsed_domain = LifeDomain::from_str(&domain)
            .ok_or_else(|| life_event_row_fault(6, "domain_enum_not_exact"))?;
        let parsed_risk_level = risk_level_from_str(&risk_level)
            .ok_or_else(|| life_event_row_fault(7, "risk_level_enum_not_exact"))?;
        let parsed_privacy_level = LifeEventPrivacyLevel::from_str(&privacy_level)
            .ok_or_else(|| life_event_row_fault(8, "privacy_level_enum_not_exact"))?;
        let event = LifeEvent {
            id: row.get(0)?,
            event_type: row.get(1)?,
            source_type: parsed_source_type,
            source_id: row.get(3)?,
            source_refs: canonical_json_column(row, 4, "source_refs_noncanonical")?,
            occurred_at: parse_time(&occurred_at, 5)?,
            domain: parsed_domain,
            risk_level: parsed_risk_level,
            privacy_level: parsed_privacy_level,
            summary: row.get(9)?,
            payload_digest: row.get(10)?,
            metadata: canonical_json_column(row, 11, "metadata_noncanonical")?,
            metadata_safe_summary: optional_canonical_json_column(
                row,
                12,
                "metadata_safe_summary_noncanonical",
            )?,
            contains_raw_content: row.get::<_, i64>(13)? != 0,
            dedupe_key: row.get(14)?,
            created_at: parse_time(&created_at, 15)?,
        };
        if payload_version != LIFE_EVENT_PAYLOAD_VERSION
            || !is_registered_life_event_type(&event.event_type)
            || !is_exact_life_event_hmac(&event.summary)
            || !is_exact_life_event_hmac(&event.payload_digest)
            || event.contains_raw_content
            || !life_event_metadata_v2_is_valid(&event.metadata)
            || event.source_refs.is_empty()
            || event.source_refs.len() > MAX_LIFE_EVENT_SOURCE_REFS
            || event
                .source_refs
                .iter()
                .any(|source| !life_event_source_ref_is_valid(self.receipt_key.as_ref(), source))
            || event.source_refs.first().is_some_and(|primary| {
                primary.source_type != event.source_type || primary.source_id != event.source_id
            })
            || !self.receipt_key.verify(
                "life_event_payload",
                &life_event_payload_material(
                    &event.event_type,
                    &event.summary,
                    &domain,
                    &risk_level,
                    &privacy_level,
                    &event.metadata,
                    &event.source_refs,
                ),
                &event.payload_digest,
            )
        {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                9,
                Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid minimized LifeEvent receipt",
                )),
            ));
        }
        Ok(event)
    }
}

fn active_life_event_tombstones(conn: &Connection) -> Result<HashSet<(String, String)>> {
    let mut statement = conn.prepare(
        "SELECT source_type, source_id FROM life_event_tombstone_projections
         WHERE active = 1",
    )?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<std::result::Result<HashSet<_>, _>>()
        .map_err(Into::into)
}

fn life_event_hidden(tombstones: &HashSet<(String, String)>, event: &LifeEvent) -> bool {
    tombstones.contains(&(event.source_type.to_string(), event.source_id.clone()))
        || event.source_refs.iter().any(|source| {
            tombstones.contains(&(source.source_type.to_string(), source.source_id.clone()))
        })
}

fn sanitize_event_metadata(
    value: &Value,
    receipt_key: &AgentRunReceiptKey,
    source: &LifeEventSourceRef,
) -> Value {
    let mut sanitized = serde_json::Map::new();
    let mut default_denied = !matches!(value, Value::Null | Value::Object(_));
    if let Some(map) = value.as_object() {
        for (field, candidate) in map {
            if field == "rawEvidencePreview" {
                let serialized = serde_json::to_string(candidate).unwrap_or_else(|_| "null".into());
                sanitized.insert(
                    "rawEvidenceReceipt".into(),
                    receipt_key
                        .sign(
                            "life_event_raw_evidence",
                            &format!(
                                "source_type\0{}\0source_id\0{}\0evidence\0{}:{}",
                                source.source_type,
                                source.source_id,
                                serialized.len(),
                                serialized
                            ),
                        )
                        .into(),
                );
                sanitized.insert("rawEvidencePresent".into(), Value::Bool(true));
            } else if life_event_metadata_field_value_allowed(field, candidate) {
                sanitized.insert(field.clone(), candidate.clone());
            } else {
                default_denied = true;
            }
        }
    }
    if default_denied {
        let serialized = serde_json::to_string(value).unwrap_or_else(|_| "null".into());
        sanitized.insert(
            "defaultDeniedMetadataReceipt".into(),
            receipt_key
                .sign(
                    "life_event_default_denied_metadata",
                    &format!(
                        "source_type\0{}\0source_id\0{}:{}\0metadata\0{}:{}",
                        source.source_type,
                        source.source_id.len(),
                        source.source_id,
                        serialized.len(),
                        serialized
                    ),
                )
                .into(),
        );
    }
    Value::Object(sanitized)
}

fn life_event_metadata_field_value_allowed(field: &str, value: &Value) -> bool {
    match field {
        "confidence" => value
            .as_f64()
            .is_some_and(|confidence| (0.0..=1.0).contains(&confidence)),
        "proposal_only"
        | "rawEvidencePresent"
        | "localOnly"
        | "proposalRequired"
        | "directLifeModelWrite"
        | "acceptedDurableTruthWritten" => value.is_boolean(),
        "domain" => value.as_str().is_some_and(|domain| {
            matches!(
                domain,
                "identity"
                    | "values"
                    | "goals"
                    | "preferences"
                    | "constraints"
                    | "relationships"
                    | "routines"
                    | "health"
                    | "work_style"
                    | "state_energy"
                    | "low_energy_planning"
                    | "other"
            )
        }),
        "sourceDigest" | "source_digest" | "eventDigest" => {
            value.as_str().is_some_and(is_exact_life_event_digest)
        }
        "candidateId" | "sourceTaskSessionId" | "sourceRunId" => {
            value.as_str().is_some_and(safe_typed_life_event_identifier)
        }
        "rawEvidenceSourceRef" => value.as_str().is_some_and(is_typed_raw_evidence_source_ref),
        _ => false,
    }
}

fn life_event_metadata_v2_is_valid(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        object.iter().all(|(field, value)| match field.as_str() {
            "rawEvidenceReceipt" | "defaultDeniedMetadataReceipt" | "legacyEventTypeReceipt" => {
                value.as_str().is_some_and(is_exact_life_event_hmac)
            }
            _ => life_event_metadata_field_value_allowed(field, value),
        })
    })
}

fn safe_typed_life_event_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 192
        && value.trim() == value
        && !value.contains("://")
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':')
        })
}

fn is_typed_raw_evidence_source_ref(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("task-session://") else {
        return false;
    };
    let Some((session_id, candidate_id)) = rest.split_once("/memory-candidate/") else {
        return false;
    };
    uuid::Uuid::parse_str(session_id).is_ok()
        && safe_typed_life_event_identifier(candidate_id)
        && !candidate_id.contains('/')
}

fn is_exact_life_event_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) || is_exact_life_event_hmac(value)
}

fn is_registered_life_event_type(value: &str) -> bool {
    matches!(
        value,
        "agent_run_projection_seed"
            | "chat_interaction"
            | "goal.short_term"
            | "identity.preference.corrected"
            | "identity.values"
            | "legacy.unregistered"
            | "main_chat.episodic_life_event"
            | "main_chat.life_event"
            | "memory.write"
            | "preference.communication"
            | "preference.planning.low_energy"
            | "receipt.body.binding"
            | "receipt.key.binding"
            | "state.current_focus"
    )
}

fn normalize_legacy_life_event_type(
    key: &AgentRunReceiptKey,
    event_type: &str,
) -> (String, Option<String>) {
    if is_registered_life_event_type(event_type) {
        return (event_type.to_string(), None);
    }
    (
        "legacy.unregistered".into(),
        Some(key.sign(
            "life_event_legacy_event_type",
            &format!("event_type\0{}:{}", event_type.len(), event_type),
        )),
    )
}

fn is_typed_life_event_source_id(source_type: LifeEventSourceType, value: &str) -> bool {
    if uuid::Uuid::parse_str(value).is_ok()
        || value
            .strip_prefix("legacy-source:")
            .is_some_and(is_exact_life_event_hmac)
    {
        return true;
    }
    if !safe_typed_life_event_identifier(value) {
        return false;
    }
    match source_type {
        LifeEventSourceType::AgentRun => value.starts_with("run-"),
        LifeEventSourceType::RuntimeObservation => value.starts_with("observation-"),
        LifeEventSourceType::Proposal => value.starts_with("proposal-"),
        LifeEventSourceType::ToolResult => {
            value.starts_with("tool-") || value.starts_with("action-")
        }
        LifeEventSourceType::ChatMessage => {
            value.starts_with("message-") || value.starts_with("task-")
        }
        LifeEventSourceType::Feedback => value.starts_with("feedback-"),
        LifeEventSourceType::UserEdit | LifeEventSourceType::ManualCorrection => {
            value.starts_with("edit-") || value.starts_with("correction-")
        }
        LifeEventSourceType::Other => value.starts_with("source-"),
    }
}

fn is_typed_life_event_source_detail(value: &str) -> bool {
    matches!(
        value,
        "plan_execute:weekly" | "w145.low_energy_support_golden_path" | "cross_store_sentinel_test"
    ) || value.strip_prefix("mc_").is_some_and(|digest| {
        !digest.is_empty() && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) || value
        .strip_prefix("candidate:test:")
        .is_some_and(safe_typed_life_event_identifier)
        || value
            .strip_prefix("candidate-")
            .is_some_and(safe_typed_life_event_identifier)
        || value
            .strip_prefix("source_detail:bytes=")
            .is_some_and(|rest| {
                rest.split_once(':').is_some_and(|(count, digest)| {
                    count.parse::<usize>().is_ok() && is_exact_life_event_hmac(digest)
                })
            })
}

fn life_event_text_receipt(key: &AgentRunReceiptKey, field: &str, value: &str) -> String {
    format!(
        "{field}:bytes={}:{}",
        value.len(),
        key.sign(
            "life_event_free_text_field",
            &format!(
                "field\0{}:{}\0value\0{}:{}",
                field.len(),
                field,
                value.len(),
                value
            ),
        )
    )
}

fn is_exact_life_event_hmac(value: &str) -> bool {
    value.strip_prefix("hmac-sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}

fn life_event_row_fault(index: usize, reason: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid_current_life_event:{reason}"),
        )),
    )
}

fn life_event_source_ref_is_valid(key: &AgentRunReceiptKey, source: &LifeEventSourceRef) -> bool {
    if !is_typed_life_event_source_id(source.source_type, &source.source_id)
        || source
            .source_detail
            .as_deref()
            .is_some_and(|detail| !is_typed_life_event_source_detail(detail))
    {
        return false;
    }
    match source.verification {
        LifeEventSourceVerification::CanonicalOwnerVerified => {
            let Some(owner) = source.canonical_owner.as_ref() else {
                return false;
            };
            matches!(
                (source.source_type, owner.owner_kind),
                (
                    LifeEventSourceType::ChatMessage,
                    CanonicalLifeEventOwnerKind::ConversationMessage
                ) | (
                    LifeEventSourceType::AgentRun,
                    CanonicalLifeEventOwnerKind::AgentRun
                )
            ) && is_canonical_owner_store_identity(
                owner.owner_kind,
                &owner.canonical_store_identity,
            ) && is_canonical_owner_ref(owner.owner_kind, &owner.canonical_ref, &source.source_id)
                && is_exact_sha256_digest(&owner.canonical_content_digest)
                && key.verify(
                    "life_event_canonical_source_ref",
                    &life_event_source_material(source),
                    &source.digest,
                )
        }
        LifeEventSourceVerification::LegacyUnverified => {
            source.canonical_owner.is_none()
                && key.verify(
                    "life_event_legacy_unverified_source_ref",
                    &life_event_source_material(source),
                    &source.digest,
                )
        }
    }
}

fn is_exact_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}

fn life_event_source_owner_metadata_key(owner_kind: CanonicalLifeEventOwnerKind) -> &'static str {
    match owner_kind {
        CanonicalLifeEventOwnerKind::ConversationMessage => {
            "canonical_source_owner_identity:conversation_message"
        }
        CanonicalLifeEventOwnerKind::AgentRun => "canonical_source_owner_identity:agent_run",
    }
}

fn is_canonical_owner_store_identity(owner_kind: CanonicalLifeEventOwnerKind, value: &str) -> bool {
    let prefix = match owner_kind {
        CanonicalLifeEventOwnerKind::ConversationMessage => "memory_store:v1:",
        CanonicalLifeEventOwnerKind::AgentRun => "agent_run_store:",
    };
    value
        .strip_prefix(prefix)
        .and_then(|identity| uuid::Uuid::parse_str(identity).ok())
        .is_some()
}

fn is_canonical_owner_ref(
    owner_kind: CanonicalLifeEventOwnerKind,
    canonical_ref: &str,
    source_id: &str,
) -> bool {
    match owner_kind {
        CanonicalLifeEventOwnerKind::ConversationMessage => {
            let Some(message_id) = source_id.strip_prefix("message-") else {
                return false;
            };
            message_id.parse::<i64>().ok().is_some_and(|id| id > 0)
                && canonical_ref
                    .rsplit_once("/message/")
                    .is_some_and(|(_, canonical_id)| canonical_id == message_id)
                && canonical_ref.starts_with("conversation://")
        }
        CanonicalLifeEventOwnerKind::AgentRun => {
            canonical_ref == format!("agent-run://{source_id}")
        }
    }
}

fn life_event_source_material(source: &LifeEventSourceRef) -> String {
    let owner = source
        .canonical_owner
        .as_ref()
        .map(|owner| {
            let legacy_compatible = format!(
                "owner_kind\0{}\0store_identity\0{}:{}\0canonical_ref\0{}:{}\0content_digest\0{}:{}",
                owner.owner_kind.as_str(),
                owner.canonical_store_identity.len(),
                owner.canonical_store_identity,
                owner.canonical_ref.len(),
                owner.canonical_ref,
                owner.canonical_content_digest.len(),
                owner.canonical_content_digest,
            );
            owner
                .canonical_revision
                .map(|revision| format!("{legacy_compatible}\0canonical_revision\0{revision}"))
                .unwrap_or(legacy_compatible)
        })
        .unwrap_or_else(|| "owner_kind\0legacy_unverified".into());
    format!(
        "source_type\0{}\0source_id\0{}:{}\0source_detail\0{}:{}\0observed_at\0{}\0verification\0{}\0{}",
        source.source_type,
        source.source_id.len(),
        source.source_id,
        source.source_detail.as_deref().unwrap_or("").len(),
        source.source_detail.as_deref().unwrap_or(""),
        source.observed_at.to_rfc3339(),
        source.verification.as_str(),
        owner,
    )
}

fn ensure_bound_life_event_source_owners(
    conn: &Connection,
    source_refs: &[LifeEventSourceRef],
) -> Result<()> {
    for source in source_refs {
        if source.verification == LifeEventSourceVerification::LegacyUnverified {
            continue;
        }
        let owner = source
            .canonical_owner
            .as_ref()
            .context("life_event_canonical_source_owner_missing")?;
        let key = life_event_source_owner_metadata_key(owner.owner_kind);
        let bound = conn
            .query_row(
                "SELECT value FROM life_event_store_metadata WHERE key = ?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if bound.as_deref() != Some(owner.canonical_store_identity.as_str()) {
            anyhow::bail!("life_event_canonical_source_owner_not_bound");
        }
    }
    Ok(())
}

fn life_event_payload_material(
    event_type: &str,
    summary_receipt: &str,
    domain: &str,
    risk_level: &str,
    privacy_level: &str,
    metadata: &Value,
    source_refs: &[LifeEventSourceRef],
) -> String {
    let metadata = serde_json::to_string(metadata).unwrap_or_else(|_| "null".into());
    let source_receipts = source_refs
        .iter()
        .map(|source| {
            let material = life_event_source_material(source);
            format!(
                "{}:{}:{}:{}:{}:{}",
                source.source_type,
                source.source_id.len(),
                source.source_id,
                source.digest,
                material.len(),
                material,
            )
        })
        .collect::<Vec<_>>()
        .join("\0");
    format!(
        "event_type\0{}:{}\0summary_receipt\0{}:{}\0domain\0{}\0risk\0{}\0privacy\0{}\0metadata\0{}:{}\0sources\0{}:{}",
        event_type.len(),
        event_type,
        summary_receipt.len(),
        summary_receipt,
        domain,
        risk_level,
        privacy_level,
        metadata.len(),
        metadata,
        source_receipts.len(),
        source_receipts,
    )
}

fn life_event_create_operation_material(
    operation_id: &str,
    binding_digest: &str,
    event_id: &str,
) -> String {
    format!(
        "operation_id\0{}\0binding_digest\0{}\0event_id\0{}",
        operation_id, binding_digest, event_id
    )
}

fn add_life_event_column_if_missing(
    conn: &Connection,
    column: &str,
    definition: &str,
) -> Result<()> {
    if !column
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || definition.trim().is_empty()
    {
        anyhow::bail!("invalid LifeEvent SQLite migration definition");
    }
    let mut statement = conn.prepare("PRAGMA table_info(life_events)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(());
        }
    }
    conn.execute(
        &format!("ALTER TABLE life_events ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

fn canonical_json_column<T>(row: &Row<'_>, idx: usize, reason: &str) -> rusqlite::Result<T>
where
    T: DeserializeOwned + Serialize,
{
    let raw: String = row.get(idx)?;
    let decoded: T = serde_json::from_str(&raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(error))
    })?;
    let canonical = serde_json::to_string(&decoded)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    if canonical != raw {
        return Err(life_event_row_fault(idx, reason));
    }
    Ok(decoded)
}

fn optional_canonical_json_column<T>(
    row: &Row<'_>,
    idx: usize,
    reason: &str,
) -> rusqlite::Result<Option<T>>
where
    T: DeserializeOwned + Serialize,
{
    let raw: Option<String> = row.get(idx)?;
    match raw {
        Some(raw) => {
            let decoded: T = serde_json::from_str(&raw).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(error))
            })?;
            let canonical = serde_json::to_string(&decoded)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            if canonical != raw {
                return Err(life_event_row_fault(idx, reason));
            }
            Ok(Some(decoded))
        }
        None => Ok(None),
    }
}

fn parse_time(value: &str, idx: usize) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(e)))
}

fn risk_level_from_str(value: &str) -> Option<RiskLevel> {
    match value {
        "low" => Some(RiskLevel::Low),
        "medium" => Some(RiskLevel::Medium),
        "high" => Some(RiskLevel::High),
        "critical" => Some(RiskLevel::Critical),
        _ => None,
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

#[cfg(any(test, feature = "test-utils"))]
fn push_unique(reasons: &mut Vec<String>, reason: impl Into<String>) {
    let reason = reason.into();
    if !reasons.iter().any(|existing| existing == &reason) {
        reasons.push(reason);
    }
}
