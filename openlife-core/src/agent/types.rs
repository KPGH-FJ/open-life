use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
fn metadata_digest(value: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, value.as_bytes());
    let hex = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskKind {
    Conversation,
    /// 构建/编辑 LifeModel（用户交互式构建）
    Builder,
    Calibration,
    Evolution,
    ToolExecution,
    Proactive,
    Planning,
    Review,
    Writing,
    MemoryGovernance,
    Skill,
    Plugin,
}

impl std::fmt::Display for AgentTaskKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentTaskKind::Conversation => write!(f, "conversation"),
            AgentTaskKind::Builder => write!(f, "builder"),
            AgentTaskKind::Calibration => write!(f, "calibration"),
            AgentTaskKind::Evolution => write!(f, "evolution"),
            AgentTaskKind::ToolExecution => write!(f, "tool_execution"),
            AgentTaskKind::Proactive => write!(f, "proactive"),
            AgentTaskKind::Planning => write!(f, "planning"),
            AgentTaskKind::Review => write!(f, "review"),
            AgentTaskKind::Writing => write!(f, "writing"),
            AgentTaskKind::MemoryGovernance => write!(f, "memory_governance"),
            AgentTaskKind::Skill => write!(f, "skill"),
            AgentTaskKind::Plugin => write!(f, "plugin"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentExecutionBudget {
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub timeout_seconds: u64,
    pub allow_cloud: bool,
    pub allow_writes: bool,
}

impl Default for AgentExecutionBudget {
    fn default() -> Self {
        Self {
            max_steps: 5,
            max_tool_calls: 3,
            timeout_seconds: 60,
            allow_cloud: true,
            allow_writes: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for AgentTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentTaskStatus::Pending => write!(f, "pending"),
            AgentTaskStatus::Running => write!(f, "running"),
            AgentTaskStatus::Completed => write!(f, "completed"),
            AgentTaskStatus::Failed => write!(f, "failed"),
            AgentTaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Typed task input used by policy classification and scheduled work.
#[derive(Debug, Clone)]
pub struct AgentTask {
    pub kind: AgentTaskKind,
    pub session_id: String,
    pub user_text: String,
    pub messages: Vec<crate::llm::ChatMessage>,
    pub layer: crate::layer::Layer,
}

/// Trace of which model was chosen and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionLevel {
    None,
    Light,
    Summary,
    Strict,
    LocalOnly,
}

impl std::fmt::Display for RedactionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RedactionLevel::None => write!(f, "none"),
            RedactionLevel::Light => write!(f, "light"),
            RedactionLevel::Summary => write!(f, "summary"),
            RedactionLevel::Strict => write!(f, "strict"),
            RedactionLevel::LocalOnly => write!(f, "local_only"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRouteTrace {
    pub provider: String,
    pub model: String,
    pub route_type: String, // "local" | "cloud" | "fallback" | "direct"
    pub prefer_local: bool,
    pub local_model: String,
    pub reason: String,
    pub privacy_level: RedactionLevel,
    pub latency_ms: Option<u64>,
    pub retry_count: u32,
    #[serde(default)]
    pub fallback_reason: Option<String>,
    #[serde(default)]
    pub provider_health_is_estimated: Option<bool>,
}

/// Summary of what context was included in the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSummary {
    pub life_model_empty: bool,
    pub included_life_model_sections: Vec<String>,
    pub memory_hit_count: i64,
    pub memory_sources: Vec<String>,
    pub used_tools_prompt: bool,
    pub redaction_applied: bool,
    pub redaction_level: RedactionLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolActionScope {
    pub tool_id: String,
    pub tool_name: String,
    pub source: String,
    pub risk_level: String,
    pub capabilities: Vec<String>,
    pub action_type: String,
    pub requires_confirmation: bool,
    pub allowed: bool,
}

pub(crate) const MAX_OBSERVED_CONTENT_RECEIPT_BYTES: usize = 16 * 1024 * 1024;
pub const CANONICAL_TASK_RECEIPT_KEY_BYTES: usize = 32;

/// Purpose-scoped receipt authority for the canonical Task/Run/Item store.
/// The key is deliberately non-serializable and its debug representation is
/// redacted. Production callers load stable random bytes from the secret store
/// and bind them to the canonical runtime database slot.
#[derive(Clone)]
pub struct CanonicalTaskReceiptKey([u8; CANONICAL_TASK_RECEIPT_KEY_BYTES]);

pub(crate) trait ContentReceiptAuthorityKey {
    fn sign_receipt(&self, purpose: &str, material: &str) -> String;
    fn verify_receipt(&self, purpose: &str, material: &str, encoded_tag: &str) -> bool;
}

impl CanonicalTaskReceiptKey {
    pub fn from_bytes(bytes: [u8; CANONICAL_TASK_RECEIPT_KEY_BYTES]) -> anyhow::Result<Self> {
        if bytes.iter().all(|byte| *byte == 0) {
            anyhow::bail!("canonical_task_receipt_key_must_not_be_all_zero");
        }
        Ok(Self(bytes))
    }

    pub(crate) fn derive_for_canonical_database_slot(
        &self,
        canonical_path: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let mut material = b"openlife-canonical-task-database-slot-key-v1\0".to_vec();
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            material.extend_from_slice(b"unix\0");
            material.extend_from_slice(canonical_path.as_os_str().as_bytes());
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            material.extend_from_slice(b"windows-utf16le\0");
            for unit in canonical_path.as_os_str().encode_wide() {
                material.extend_from_slice(&unit.to_le_bytes());
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            material.extend_from_slice(b"portable-lossy\0");
            material.extend_from_slice(canonical_path.to_string_lossy().as_bytes());
        }
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &self.0);
        let tag = ring::hmac::sign(&key, &material);
        let bytes: [u8; CANONICAL_TASK_RECEIPT_KEY_BYTES] = tag
            .as_ref()
            .try_into()
            .map_err(|_| anyhow::anyhow!("canonical_task_database_slot_key_derivation_failed"))?;
        Self::from_bytes(bytes)
    }

    fn signing_message(purpose: &str, material: &str) -> String {
        format!(
            "openlife-canonical-task-receipt-v1\0purpose\0{}:{}\0material\0{}:{}",
            purpose.len(),
            purpose,
            material.len(),
            material
        )
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn test_key() -> Self {
        Self([0xC7; CANONICAL_TASK_RECEIPT_KEY_BYTES])
    }
}

impl ContentReceiptAuthorityKey for CanonicalTaskReceiptKey {
    fn sign_receipt(&self, purpose: &str, material: &str) -> String {
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &self.0);
        let message = Self::signing_message(purpose, material);
        let tag = ring::hmac::sign(&key, message.as_bytes());
        let hex = tag
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        format!("hmac-sha256:{hex}")
    }

    fn verify_receipt(&self, purpose: &str, material: &str, encoded_tag: &str) -> bool {
        let Some(hex) = encoded_tag.strip_prefix("hmac-sha256:") else {
            return false;
        };
        if hex.len() != 64 {
            return false;
        }
        let mut tag = [0u8; 32];
        for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
            let Ok(pair) = std::str::from_utf8(pair) else {
                return false;
            };
            let Ok(byte) = u8::from_str_radix(pair, 16) else {
                return false;
            };
            tag[index] = byte;
        }
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &self.0);
        ring::hmac::verify(
            &key,
            Self::signing_message(purpose, material).as_bytes(),
            &tag,
        )
        .is_ok()
    }
}

impl Drop for CanonicalTaskReceiptKey {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

impl std::fmt::Debug for CanonicalTaskReceiptKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CanonicalTaskReceiptKey([REDACTED])")
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ContentReceiptBinding {
    canonical_store_identity: Option<String>,
    run_id: String,
    action_id: String,
    observation_id: String,
    field: BoundContentField,
    semantic_material: String,
}

impl ContentReceiptBinding {
    pub(crate) fn from_action_graph(
        run_id: &str,
        action: &AgentAction,
        observation: &AgentObservation,
        field: BoundContentField,
    ) -> anyhow::Result<Self> {
        Self::from_action_graph_internal(None, run_id, action, observation, field)
    }

    pub(crate) fn from_canonical_action_graph(
        canonical_store_identity: &str,
        run_id: &str,
        action: &AgentAction,
        observation: &AgentObservation,
        field: BoundContentField,
    ) -> anyhow::Result<Self> {
        if canonical_store_identity
            .strip_prefix("canonical_task_runtime_store:")
            .and_then(|value| Uuid::parse_str(value).ok())
            .is_none()
        {
            anyhow::bail!("bound_content_binding_store_identity_invalid");
        }
        Self::from_action_graph_internal(
            Some(canonical_store_identity),
            run_id,
            action,
            observation,
            field,
        )
    }

    fn from_action_graph_internal(
        canonical_store_identity: Option<&str>,
        run_id: &str,
        action: &AgentAction,
        observation: &AgentObservation,
        field: BoundContentField,
    ) -> anyhow::Result<Self> {
        let trace = action
            .tool_trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("bound_content_binding_action_trace_missing"))?;
        let scope = action
            .tool_scope
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("bound_content_binding_manifest_scope_missing"))?;
        let target = action
            .target
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("bound_content_binding_action_target_missing"))?;
        let invalid_identity = if run_id.trim().is_empty() {
            Some("run_id_empty")
        } else if action.id.trim().is_empty() {
            Some("action_id_empty")
        } else if observation.id.trim().is_empty() {
            Some("observation_id_empty")
        } else if observation.action_id.as_deref() != Some(action.id.as_str()) {
            Some("observation_action_id_mismatch")
        } else if trace.run_id.as_deref() != Some(run_id) {
            Some("trace_run_id_mismatch")
        } else if trace.action_id != action.id {
            Some("trace_action_id_mismatch")
        } else if trace.observation_id.as_deref() != Some(observation.id.as_str()) {
            Some("trace_observation_id_mismatch")
        } else if trace.action_type != action.action_type {
            Some("trace_action_type_mismatch")
        } else if trace.tool_name != scope.tool_name {
            Some("trace_tool_name_mismatch")
        } else if trace.tool_id != scope.tool_id {
            Some("trace_tool_id_mismatch")
        } else if trace.tool_source != scope.source {
            Some("trace_tool_source_mismatch")
        } else if trace.risk_level != scope.risk_level {
            Some("trace_risk_level_mismatch")
        } else if [
            action.action_type.as_str(),
            target,
            trace.tool_name.as_str(),
            trace.tool_id.as_str(),
            trace.tool_source.as_str(),
            scope.action_type.as_str(),
            scope.risk_level.as_str(),
        ]
        .into_iter()
        .any(|value| value.is_empty() || value.trim() != value)
        {
            Some("owner_field_not_canonical")
        } else {
            None
        };
        if let Some(reason) = invalid_identity {
            anyhow::bail!("bound_content_binding_owner_graph_invalid:{reason}");
        }
        let mut trace_without_receipt = trace.clone();
        trace_without_receipt.output_receipt = None;
        let semantic_material = serde_json::to_string(&serde_json::json!({
            "action": {
                "id": action.id,
                "actionType": action.action_type,
                "target": action.target,
                "input": action.input,
                "status": action.status,
                "permissionDecision": action.permission_decision,
                "startedAt": action.started_at,
                "finishedAt": action.finished_at,
                "timestamp": action.timestamp,
                "toolScope": action.tool_scope,
                "toolTrace": trace_without_receipt,
            },
            "observation": {
                "id": observation.id,
                "actionId": observation.action_id,
                "source": observation.source,
                "structuredResult": observation.structured_result,
                "timestamp": observation.timestamp,
            },
            "boundField": field.as_str(),
        }))
        .map_err(|error| anyhow::anyhow!("bound_content_binding_serialization_failed:{error}"))?;
        Ok(Self {
            canonical_store_identity: canonical_store_identity.map(str::to_string),
            run_id: run_id.to_string(),
            action_id: action.id.clone(),
            observation_id: observation.id.clone(),
            field,
            semantic_material,
        })
    }

    fn material(&self) -> String {
        let canonical_store_identity = self.canonical_store_identity.as_deref().unwrap_or("");
        [
            ("canonical_store_identity", canonical_store_identity),
            ("run_id", self.run_id.as_str()),
            ("action_id", self.action_id.as_str()),
            ("observation_id", self.observation_id.as_str()),
            ("field", self.field.as_str()),
            ("semantic_material", self.semantic_material.as_str()),
        ]
        .into_iter()
        .map(|(name, value)| format!("{}:{}={}:{}", name.len(), name, value.len(), value))
        .collect::<Vec<_>>()
        .join("\0")
    }

    fn owner_anchor_material(&self) -> String {
        [
            ("run_id", self.run_id.as_str()),
            ("action_id", self.action_id.as_str()),
            ("observation_id", self.observation_id.as_str()),
            ("field", self.field.as_str()),
        ]
        .into_iter()
        .map(|(name, value)| format!("{}:{}={}:{}", name.len(), name, value.len(), value))
        .collect::<Vec<_>>()
        .join("\0")
    }

    pub(crate) fn owner_anchor_digest(&self) -> String {
        metadata_digest(&self.owner_anchor_material())
    }

    pub(crate) fn field(&self) -> BoundContentField {
        self.field
    }

    fn same_graph_owner_identity(&self, receipt: &BoundContentReceipt) -> bool {
        self.run_id == receipt.run_id
            && self.action_id == receipt.action_id
            && self.observation_id == receipt.observation_id
            && self.field == receipt.field
    }

    fn same_canonical_owner_identity(&self, receipt: &BoundContentReceipt) -> bool {
        self.same_graph_owner_identity(receipt)
            && self.canonical_store_identity.as_deref()
                == Some(receipt.canonical_store_identity.as_str())
    }

    fn canonical_store_identity(&self) -> Option<&str> {
        self.canonical_store_identity.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundContentField {
    ActionOutputObservationContent,
    ActionErrorObservationContent,
}

impl BoundContentField {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActionOutputObservationContent => "action_output_observation_content",
            Self::ActionErrorObservationContent => "action_error_observation_content",
        }
    }

    pub(crate) fn for_kind(kind: ContentReceiptKind) -> Self {
        match kind {
            ContentReceiptKind::ToolOutput => Self::ActionOutputObservationContent,
            ContentReceiptKind::ToolError => Self::ActionErrorObservationContent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentReceiptKind {
    ToolOutput,
    ToolError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentReceiptProvenance {
    ObservedToolAdapterBody,
}

const BOUND_CONTENT_RECEIPT_VERSION_LEGACY_UNVERIFIED: u8 = 1;
const BOUND_CONTENT_RECEIPT_VERSION_CURRENT: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentReceiptIssuanceError {
    ObservedBodyTooLarge {
        observed_bytes: usize,
        max_bytes: usize,
    },
    FieldKindMismatch,
}

impl std::fmt::Display for ContentReceiptIssuanceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ObservedBodyTooLarge {
                observed_bytes,
                max_bytes,
            } => write!(
                formatter,
                "bound_content_receipt_observed_body_too_large:{observed_bytes}:{max_bytes}"
            ),
            Self::FieldKindMismatch => {
                formatter.write_str("bound_content_receipt_field_kind_mismatch")
            }
        }
    }
}

impl std::error::Error for ContentReceiptIssuanceError {}

/// One durable, owner-bound receipt for the adapter body shared by an
/// AgentAction and its AgentObservation. Only the narrow receipt issuer may
/// consume a ToolExecutor admission and construct this metadata. Raw bodies
/// are never serialized or copied into the issuance ledger.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundContentReceipt {
    version: u8,
    issuance_id: String,
    receipt_id: String,
    canonical_store_identity: String,
    run_id: String,
    action_id: String,
    observation_id: String,
    field: BoundContentField,
    kind: ContentReceiptKind,
    provenance: ContentReceiptProvenance,
    byte_count: usize,
    /// Recomputable from the canonical persisted action/observation semantics.
    binding_receipt: String,
    /// Keyed receipt over the original adapter binding and body. The body is
    /// not retained by the canonical Work runtime store.
    body_receipt: String,
    authority_tag: String,
}

impl std::fmt::Debug for BoundContentReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoundContentReceipt")
            .field("version", &self.version)
            .field("kind", &self.kind)
            .field("provenance", &self.provenance)
            .field("byte_count", &self.byte_count)
            .field("public_digest", &self.public_digest())
            .field("authority", &"[REDACTED]")
            .finish()
    }
}

impl BoundContentReceipt {
    pub(crate) fn issue_durable<K: ContentReceiptAuthorityKey>(
        key: &K,
        evidence: crate::agent::action_executor::tool_executor::ObservedToolBodyIssueEvidence,
        observed_binding: &ContentReceiptBinding,
        canonical_binding: &ContentReceiptBinding,
    ) -> anyhow::Result<Self> {
        let issuance_id = evidence.issuance_id().to_string();
        let kind = evidence.kind();
        let observed_body = evidence.body();
        let canonical_store_identity = canonical_binding
            .canonical_store_identity()
            .ok_or_else(|| anyhow::anyhow!("bound_content_receipt_store_identity_missing"))?;
        if observed_body.len() > MAX_OBSERVED_CONTENT_RECEIPT_BYTES {
            anyhow::bail!("bound_content_receipt_observed_body_too_large");
        }
        if observed_binding.field != BoundContentField::for_kind(kind)
            || canonical_binding.field != observed_binding.field
        {
            anyhow::bail!("bound_content_receipt_field_kind_mismatch");
        }
        if observed_binding.owner_anchor_digest() != evidence.owner_anchor_digest() {
            anyhow::bail!("bound_content_receipt_owner_anchor_mismatch");
        }
        if Uuid::parse_str(&issuance_id).is_err() {
            anyhow::bail!("bound_content_receipt_issuance_identity_invalid");
        }
        let mut durable = Self {
            version: BOUND_CONTENT_RECEIPT_VERSION_CURRENT,
            issuance_id,
            receipt_id: Uuid::new_v4().to_string(),
            canonical_store_identity: canonical_store_identity.to_string(),
            run_id: observed_binding.run_id.clone(),
            action_id: observed_binding.action_id.clone(),
            observation_id: observed_binding.observation_id.clone(),
            field: observed_binding.field,
            kind,
            provenance: ContentReceiptProvenance::ObservedToolAdapterBody,
            byte_count: observed_body.len(),
            binding_receipt: String::new(),
            body_receipt: String::new(),
            authority_tag: String::new(),
        };
        durable.binding_receipt = key.sign_receipt(
            "canonical_task_bound_content_binding_v2",
            &canonical_binding.material(),
        );
        let body_material = durable.body_material(observed_binding, observed_body);
        durable.body_receipt =
            key.sign_receipt("canonical_task_bound_content_body_v2", &body_material);
        if !key.verify_receipt(
            "canonical_task_bound_content_body_v2",
            &body_material,
            &durable.body_receipt,
        ) {
            anyhow::bail!("bound_content_receipt_body_issue_failed");
        }
        durable.authority_tag = key.sign_receipt(
            "canonical_task_bound_content_authority_v2",
            &durable.canonical_material(),
        );
        if !durable.verify_durable(key, canonical_binding) {
            anyhow::bail!("bound_content_receipt_durable_issue_failed");
        }
        Ok(durable)
    }

    fn body_material(
        &self,
        observed_binding: &ContentReceiptBinding,
        observed_body: &str,
    ) -> String {
        let observed_owner_anchor = observed_binding.owner_anchor_material();
        format!(
            "version\0{}\0issuance_id\0{}:{}\0owner_anchor\0{}:{}\0body\0{}:{}",
            BOUND_CONTENT_RECEIPT_VERSION_CURRENT,
            self.issuance_id.len(),
            self.issuance_id,
            observed_owner_anchor.len(),
            observed_owner_anchor,
            observed_body.len(),
            observed_body,
        )
    }

    pub(crate) fn verify_durable<K: ContentReceiptAuthorityKey>(
        &self,
        key: &K,
        canonical_binding: &ContentReceiptBinding,
    ) -> bool {
        self.version == BOUND_CONTENT_RECEIPT_VERSION_CURRENT
            && canonical_binding.same_canonical_owner_identity(self)
            && Uuid::parse_str(&self.issuance_id).is_ok()
            && self.field == BoundContentField::for_kind(self.kind)
            && is_exact_hmac_sha256(&self.binding_receipt)
            && is_exact_hmac_sha256(&self.body_receipt)
            && is_exact_hmac_sha256(&self.authority_tag)
            && key.verify_receipt(
                "canonical_task_bound_content_binding_v2",
                &canonical_binding.material(),
                &self.binding_receipt,
            )
            && key.verify_receipt(
                "canonical_task_bound_content_authority_v2",
                &self.canonical_material(),
                &self.authority_tag,
            )
    }

    pub fn is_legacy_unverified(&self) -> bool {
        self.version == BOUND_CONTENT_RECEIPT_VERSION_LEGACY_UNVERIFIED
    }

    fn canonical_material(&self) -> String {
        let version = self.version.to_string();
        let byte_count = self.byte_count.to_string();
        [
            ("version", version.as_str()),
            ("issuance_id", self.issuance_id.as_str()),
            ("receipt_id", self.receipt_id.as_str()),
            (
                "canonical_store_identity",
                self.canonical_store_identity.as_str(),
            ),
            ("run_id", self.run_id.as_str()),
            ("action_id", self.action_id.as_str()),
            ("observation_id", self.observation_id.as_str()),
            ("field", self.field.as_str()),
            (
                "kind",
                match self.kind {
                    ContentReceiptKind::ToolOutput => "tool_output",
                    ContentReceiptKind::ToolError => "tool_error",
                },
            ),
            (
                "provenance",
                match self.provenance {
                    ContentReceiptProvenance::ObservedToolAdapterBody => {
                        "observed_tool_adapter_body"
                    }
                },
            ),
            ("byte_count", byte_count.as_str()),
            ("binding_receipt", self.binding_receipt.as_str()),
            ("body_receipt", self.body_receipt.as_str()),
        ]
        .into_iter()
        .map(|(name, value)| format!("{}:{}={}:{}", name.len(), name, value.len(), value))
        .collect::<Vec<_>>()
        .join("\0")
    }

    pub fn public_digest(&self) -> String {
        metadata_digest(&self.authority_tag)
    }

    pub fn kind(&self) -> ContentReceiptKind {
        self.kind
    }

    pub fn provenance(&self) -> ContentReceiptProvenance {
        self.provenance
    }

    pub fn byte_count(&self) -> usize {
        self.byte_count
    }

    pub fn version(&self) -> u8 {
        self.version
    }
}

impl PartialEq for BoundContentReceipt {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.issuance_id == other.issuance_id
            && self.kind == other.kind
            && self.provenance == other.provenance
            && self.byte_count == other.byte_count
            && self.canonical_store_identity == other.canonical_store_identity
            && self.run_id == other.run_id
            && self.action_id == other.action_id
            && self.observation_id == other.observation_id
            && self.field == other.field
            && self.binding_receipt == other.binding_receipt
            && self.body_receipt == other.body_receipt
            && self.authority_tag == other.authority_tag
            && self.receipt_id == other.receipt_id
    }
}

impl Eq for BoundContentReceipt {}

impl<'de> Deserialize<'de> for BoundContentReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            #[serde(default)]
            version: Option<u8>,
            #[serde(default)]
            issuance_id: Option<String>,
            receipt_id: String,
            #[serde(default)]
            canonical_store_identity: Option<String>,
            run_id: String,
            action_id: String,
            observation_id: String,
            field: BoundContentField,
            kind: ContentReceiptKind,
            provenance: ContentReceiptProvenance,
            byte_count: usize,
            #[serde(default)]
            binding_receipt: Option<String>,
            #[serde(default)]
            body_receipt: Option<String>,
            #[serde(default)]
            opaque_body_receipt: Option<String>,
            authority_tag: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        let version = wire
            .version
            .unwrap_or(BOUND_CONTENT_RECEIPT_VERSION_LEGACY_UNVERIFIED);
        let (issuance_id, canonical_store_identity, binding_receipt, body_receipt) = match version {
            BOUND_CONTENT_RECEIPT_VERSION_CURRENT => {
                let issuance_id = wire
                    .issuance_id
                    .filter(|value| Uuid::parse_str(value).is_ok())
                    .ok_or_else(|| serde::de::Error::custom("content_receipt_issuance_missing"))?;
                let binding_receipt = wire
                    .binding_receipt
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| serde::de::Error::custom("content_receipt_binding_missing"))?;
                let body_receipt = wire
                    .body_receipt
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| serde::de::Error::custom("content_receipt_body_missing"))?;
                let canonical_store_identity = wire
                    .canonical_store_identity
                    .filter(|value| {
                        value
                            .strip_prefix("canonical_task_runtime_store:")
                            .and_then(|identity| Uuid::parse_str(identity).ok())
                            .is_some()
                    })
                    .ok_or_else(|| {
                        serde::de::Error::custom("content_receipt_store_identity_missing")
                    })?;
                if wire.opaque_body_receipt.is_some()
                    || !is_exact_hmac_sha256(&binding_receipt)
                    || !is_exact_hmac_sha256(&body_receipt)
                {
                    return Err(serde::de::Error::custom("invalid_content_receipt_v2"));
                }
                (
                    issuance_id,
                    canonical_store_identity,
                    binding_receipt,
                    body_receipt,
                )
            }
            BOUND_CONTENT_RECEIPT_VERSION_LEGACY_UNVERIFIED => {
                let issuance_id = wire
                    .issuance_id
                    .filter(|value| Uuid::parse_str(value).is_ok())
                    .unwrap_or_else(|| wire.receipt_id.clone());
                let binding_receipt = wire.binding_receipt.unwrap_or_default();
                if !binding_receipt.is_empty() {
                    return Err(serde::de::Error::custom(
                        "legacy_content_receipt_cannot_claim_binding",
                    ));
                }
                let body_receipt =
                    wire.opaque_body_receipt
                        .or(wire.body_receipt)
                        .ok_or_else(|| {
                            serde::de::Error::custom("legacy_content_receipt_body_missing")
                        })?;
                if !is_exact_hmac_sha256(&body_receipt) {
                    return Err(serde::de::Error::custom(
                        "invalid_legacy_content_receipt_body",
                    ));
                }
                (
                    issuance_id,
                    wire.canonical_store_identity.unwrap_or_default(),
                    String::new(),
                    body_receipt,
                )
            }
            _ => {
                return Err(serde::de::Error::custom(
                    "unsupported_content_receipt_version",
                ))
            }
        };
        if wire.byte_count > MAX_OBSERVED_CONTENT_RECEIPT_BYTES
            || wire.field != BoundContentField::for_kind(wire.kind)
            || !is_exact_hmac_sha256(&wire.authority_tag)
            || Uuid::parse_str(&wire.receipt_id).is_err()
            || [
                wire.run_id.as_str(),
                wire.action_id.as_str(),
                wire.observation_id.as_str(),
            ]
            .into_iter()
            .any(|value| value.is_empty() || value.trim() != value || value.len() > 384)
        {
            return Err(serde::de::Error::custom("invalid_content_receipt"));
        }
        Ok(Self {
            version,
            issuance_id,
            receipt_id: wire.receipt_id,
            canonical_store_identity,
            run_id: wire.run_id,
            action_id: wire.action_id,
            observation_id: wire.observation_id,
            field: wire.field,
            kind: wire.kind,
            provenance: wire.provenance,
            byte_count: wire.byte_count,
            binding_receipt,
            body_receipt,
            authority_tag: wire.authority_tag,
        })
    }
}

pub type ContentReceipt = BoundContentReceipt;

fn is_exact_hmac_sha256(value: &str) -> bool {
    value.strip_prefix("hmac-sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolActionTraceEnvelope {
    #[serde(default)]
    pub run_id: Option<String>,
    pub action_id: String,
    pub step_index: u32,
    pub tool_call_index: u32,
    pub action_type: String,
    pub tool_id: String,
    pub tool_name: String,
    pub tool_source: String,
    pub action_category: String,
    pub risk_level: String,
    #[serde(default)]
    pub permission_decision: Option<String>,
    pub status: String,
    #[serde(default)]
    pub proposal_id: Option<String>,
    #[serde(default)]
    pub observation_id: Option<String>,
    #[serde(default)]
    pub observation_status: Option<String>,
    #[serde(default)]
    pub output_preview: Option<String>,
    #[serde(default)]
    pub output_receipt: Option<ContentReceipt>,
    #[serde(default)]
    pub output_item_count: Option<usize>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
    pub metadata_safe: bool,
}

impl std::fmt::Debug for ToolActionTraceEnvelope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToolActionTraceEnvelope")
            .field("action_id", &self.action_id)
            .field("step_index", &self.step_index)
            .field("tool_call_index", &self.tool_call_index)
            .field("action_type", &self.action_type)
            .field("tool_id", &self.tool_id)
            .field("status", &self.status)
            .field("output_receipt_present", &self.output_receipt.is_some())
            .field("output_item_count", &self.output_item_count)
            .field("output_preview", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAction {
    pub id: String,
    pub action_type: String,
    #[serde(default)]
    pub target: Option<String>,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub status: String,
    #[serde(default)]
    pub permission_decision: Option<String>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub tool_scope: Option<ToolActionScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_trace: Option<ToolActionTraceEnvelope>,
    /// In-process-only execution authority. JSON/SQLite projections may carry
    /// receipt-shaped metadata, but deserialization must never recreate this
    /// runtime sidecar or its authenticity seal.
    #[serde(skip)]
    pub runtime_execution_receipt: Option<crate::tool_execution_receipt::ToolExecutionReceipt>,
}

impl std::fmt::Debug for AgentAction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentAction")
            .field("id", &self.id)
            .field("action_type", &self.action_type)
            .field("status", &self.status)
            .field("input", &"[REDACTED]")
            .field("output", &"[REDACTED]")
            .field("error", &self.error.as_ref().map(|_| "[REDACTED]"))
            .field("tool_scope_present", &self.tool_scope.is_some())
            .field("tool_trace_present", &self.tool_trace.is_some())
            .field(
                "runtime_execution_receipt_present",
                &self.runtime_execution_receipt.is_some(),
            )
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentObservation {
    pub id: String,
    #[serde(default)]
    pub action_id: Option<String>,
    pub content: String,
    pub source: String,
    #[serde(default)]
    pub structured_result: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_trace: Option<ToolActionTraceEnvelope>,
}

impl std::fmt::Debug for AgentObservation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentObservation")
            .field("id", &self.id)
            .field("action_id", &self.action_id)
            .field("source", &self.source)
            .field("content_bytes", &self.content.len())
            .field("content", &"[REDACTED]")
            .field(
                "structured_result_present",
                &self.structured_result.is_some(),
            )
            .field("tool_trace_present", &self.tool_trace.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Pending,
    Accepted,
    Rejected,
    Edited,
    Postponed,
    Expired,
}

impl std::fmt::Display for ProposalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalStatus::Pending => write!(f, "pending"),
            ProposalStatus::Accepted => write!(f, "accepted"),
            ProposalStatus::Rejected => write!(f, "rejected"),
            ProposalStatus::Edited => write!(f, "edited"),
            ProposalStatus::Postponed => write!(f, "postponed"),
            ProposalStatus::Expired => write!(f, "expired"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalType {
    GoalUpdate,
    StateUpdate,
    PreferenceUpdate,
    CapabilityUpdate,
    MemoryWrite,
    MemoryArchive,
    ToolPermission,
    PluginPermission,
    ScheduledTask,
    ExternalWriteAction,
    ModelPolicyChange,
    DataExport,
    ScheduleCheckin,
    /// Unknown or future proposal type that this build cannot safely apply.
    Unsupported,
    /// 兼容旧数据
    #[serde(alias = "life_model_update")]
    LifeModelUpdate,
}

impl std::fmt::Display for ProposalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalType::GoalUpdate => write!(f, "goal_update"),
            ProposalType::StateUpdate => write!(f, "state_update"),
            ProposalType::PreferenceUpdate => write!(f, "preference_update"),
            ProposalType::CapabilityUpdate => write!(f, "capability_update"),
            ProposalType::MemoryWrite => write!(f, "memory_write"),
            ProposalType::MemoryArchive => write!(f, "memory_archive"),
            ProposalType::ToolPermission => write!(f, "tool_permission"),
            ProposalType::PluginPermission => write!(f, "plugin_permission"),
            ProposalType::ScheduledTask => write!(f, "scheduled_task"),
            ProposalType::ExternalWriteAction => write!(f, "external_write_action"),
            ProposalType::ModelPolicyChange => write!(f, "model_policy_change"),
            ProposalType::DataExport => write!(f, "data_export"),
            ProposalType::ScheduleCheckin => write!(f, "schedule_checkin"),
            ProposalType::Unsupported => write!(f, "unsupported"),
            ProposalType::LifeModelUpdate => write!(f, "life_model_update"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalSource {
    BuilderReview,
    CalibrationRun,
    FeedbackEvolution,
    MemoryGovernance,
    SkillRuntime,
    Plugin,
    NetworkConsent,
    Manual,
    ChatConversation,
    /// Agent 主动发起的提案（如定期检查、触发式建议）
    ProactiveAgent,
    PlanningSession,
}

impl std::fmt::Display for ProposalSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalSource::BuilderReview => write!(f, "builder_review"),
            ProposalSource::CalibrationRun => write!(f, "calibration_run"),
            ProposalSource::FeedbackEvolution => write!(f, "feedback_evolution"),
            ProposalSource::MemoryGovernance => write!(f, "memory_governance"),
            ProposalSource::SkillRuntime => write!(f, "skill_runtime"),
            ProposalSource::Plugin => write!(f, "plugin"),
            ProposalSource::NetworkConsent => write!(f, "network_consent"),
            ProposalSource::Manual => write!(f, "manual"),
            ProposalSource::ChatConversation => write!(f, "chat_conversation"),
            ProposalSource::ProactiveAgent => write!(f, "proactive_agent"),
            ProposalSource::PlanningSession => write!(f, "planning_session"),
        }
    }
}

impl rusqlite::types::ToSql for ProposalSource {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

impl rusqlite::types::FromSql for ProposalSource {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().and_then(|s| match s {
            "builder_review" => Ok(ProposalSource::BuilderReview),
            "calibration_run" => Ok(ProposalSource::CalibrationRun),
            "feedback_evolution" => Ok(ProposalSource::FeedbackEvolution),
            "memory_governance" => Ok(ProposalSource::MemoryGovernance),
            "skill_runtime" => Ok(ProposalSource::SkillRuntime),
            "plugin" => Ok(ProposalSource::Plugin),
            "network_consent" => Ok(ProposalSource::NetworkConsent),
            "manual" => Ok(ProposalSource::Manual),
            "chat_conversation" => Ok(ProposalSource::ChatConversation),
            "proactive_agent" => Ok(ProposalSource::ProactiveAgent),
            "planning_session" => Ok(ProposalSource::PlanningSession),
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProposal {
    pub id: String,
    pub run_id: Option<String>,
    pub proposal_type: ProposalType,
    pub source: ProposalSource,
    pub source_detail: Option<String>,
    pub base_hash: Option<String>,
    pub affected_path: String,
    pub before: Option<serde_json::Value>,
    pub after: serde_json::Value,
    pub reason: String,
    pub confidence: f32,
    pub risk_level: RiskLevel,
    pub status: ProposalStatus,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl AgentProposal {
    pub fn new(
        proposal_type: ProposalType,
        affected_path: &str,
        after: serde_json::Value,
        reason: &str,
        confidence: f32,
        risk_level: RiskLevel,
        source: ProposalSource,
    ) -> Self {
        let expires_at = Self::calculate_expires_at(source);
        Self {
            id: Uuid::new_v4().to_string(),
            run_id: None,
            proposal_type,
            source,
            source_detail: None,
            base_hash: None,
            affected_path: affected_path.to_string(),
            before: None,
            after,
            reason: reason.to_string(),
            confidence,
            risk_level,
            status: ProposalStatus::Pending,
            created_at: Utc::now(),
            resolved_at: None,
            expires_at,
        }
    }

    /// Calculate expiration time based on source
    fn calculate_expires_at(source: ProposalSource) -> Option<DateTime<Utc>> {
        let duration = match source {
            ProposalSource::BuilderReview => chrono::Duration::days(30),
            ProposalSource::CalibrationRun => chrono::Duration::days(14),
            ProposalSource::FeedbackEvolution => chrono::Duration::days(7),
            ProposalSource::MemoryGovernance => chrono::Duration::days(7),
            ProposalSource::SkillRuntime => chrono::Duration::days(14),
            ProposalSource::Plugin => chrono::Duration::days(14),
            ProposalSource::NetworkConsent => chrono::Duration::days(3),
            ProposalSource::Manual => chrono::Duration::days(365),
            ProposalSource::ChatConversation => chrono::Duration::days(3),
            ProposalSource::ProactiveAgent => chrono::Duration::days(7),
            ProposalSource::PlanningSession => chrono::Duration::days(14),
        };
        Some(Utc::now() + duration)
    }

    /// Check if proposal is expired
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires) => Utc::now() > expires,
            None => false,
        }
    }

    /// Get days until expiration (negative if expired)
    pub fn days_until_expiration(&self) -> Option<i64> {
        self.expires_at.map(|expires| {
            let now = Utc::now();
            let duration = expires.signed_duration_since(now);
            duration.num_days()
        })
    }

    /// Backward compatibility: get run_id
    pub fn get_run_id(&self) -> Option<&str> {
        self.run_id.as_deref()
    }

    pub fn accept(&mut self) {
        self.status = ProposalStatus::Accepted;
        self.resolved_at = Some(Utc::now());
    }

    pub fn reject(&mut self) {
        self.status = ProposalStatus::Rejected;
        self.resolved_at = Some(Utc::now());
    }

    pub fn edit(&mut self, new_after: serde_json::Value) {
        self.after = new_after;
        self.status = ProposalStatus::Edited;
        self.resolved_at = None;
    }

    pub fn postpone(&mut self) {
        self.status = ProposalStatus::Postponed;
        self.resolved_at = Some(Utc::now());
    }
}
