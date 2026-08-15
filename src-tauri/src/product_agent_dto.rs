//! Explicit product-safe Work run and Main Chat trace projection.
//!
//! Core receipt structs intentionally contain canonical owner identities and
//! keyed verification material. No shipped IPC surface may serialize those
//! structs directly. This module is the single adapter that emits the six
//! public receipt facts understood by the frontend.

use openlife_core::agent::{ContentReceipt, ToolActionTraceEnvelope};
use serde::Serialize;

fn contains_internal_authority(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    normalized.contains("hmac-sha256:")
        || normalized.contains("canonicalstoreidentity")
        || normalized.contains("bindingreceipt")
        || normalized.contains("bodyreceipt")
        || normalized.contains("authoritytag")
}

fn public_text(value: String, max_bytes: usize) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()
        && trimmed.len() <= max_bytes
        && !trimmed.chars().any(char::is_control)
        && !contains_internal_authority(trimmed))
    .then(|| trimmed.to_string())
}

fn public_code(value: String) -> Option<String> {
    public_text(value, 128).filter(|candidate| {
        candidate
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._:/-".contains(character))
    })
}

fn required_public_code(value: String, unknown: &'static str) -> String {
    public_code(value).unwrap_or_else(|| unknown.into())
}

fn product_redacted_byte_count_preview(value: String) -> Option<String> {
    let trimmed = value.trim();
    let digits = trimmed.strip_suffix(" bytes redacted")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let byte_count = digits.parse::<usize>().ok()?;
    (byte_count.to_string() == digits).then(|| format!("{byte_count} bytes redacted"))
}

fn strict_uuid_ref(value: &str, unknown: &'static str) -> String {
    uuid::Uuid::parse_str(value)
        .map(|_| value.to_string())
        .unwrap_or_else(|_| unknown.into())
}

fn strict_opaque_sha256(value: &str) -> Option<String> {
    let digest = value.strip_prefix("sha256:")?;
    (digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')))
    .then(|| value.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductContentReceipt {
    version: u8,
    kind: openlife_core::agent::ContentReceiptKind,
    provenance: openlife_core::agent::ContentReceiptProvenance,
    byte_count: usize,
    digest: String,
    verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductToolReference {
    id: String,
    source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductToolExecutionReceipt {
    receipt_ref: String,
    request_digest: String,
    action_effect: openlife_core::tool_execution_receipt::ToolActionEffect,
    idempotency_contract: openlife_core::tool_manifest::ToolIdempotencyContract,
    dispatch_kind: openlife_core::tool_execution_receipt::ToolDispatchKind,
    dispatch_attempt_count: u32,
    dispatch_observed: bool,
    transport_status: openlife_core::tool_execution_receipt::ToolTransportStatus,
    effect_status: openlife_core::tool_execution_receipt::ToolEffectStatus,
    outcome: openlife_core::tool_execution_receipt::ToolExecutionOutcome,
    audit_persistence_status: openlife_core::tool_execution_receipt::ToolAuditPersistenceStatus,
    verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
// Tool prefixes are serialized product contract values, not redundant local names.
#[expect(
    clippy::enum_variant_names,
    reason = "owner=backend-contracts; expires=2026-10-01; preserve serialized or recovery vocabulary"
)]
pub enum ProductToolFailureCode {
    ToolFailed,
    ToolEffectUnknown,
    ToolRemoteStateUnknown,
    ToolLocallyAborted,
    ToolNotDispatched,
    ToolStateUnknown,
    ToolEvidenceUnverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductToolCallStatus {
    Success,
    Failed,
    EffectUnknown,
    NotDispatched,
    LocallyAborted,
    RemoteUnknown,
    Unknown,
}

/// Runtime-only evidence that an IPC projection was derived from the exact
/// live ToolGateway receipt bound to the exact AgentAction. Its fields are
/// private and it is never serialized as authorization material.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VerifiedProductToolCallProjection {
    bound_action_id: String,
    bound_run_id: String,
    bound_receipt: openlife_core::tool_execution_receipt::ToolExecutionReceipt,
    action_ref: String,
    run_ref: String,
    tool_ref: ProductToolReference,
    receipt: ProductToolExecutionReceipt,
    status: ProductToolCallStatus,
    failure_code: Option<ProductToolFailureCode>,
}

impl VerifiedProductToolCallProjection {
    pub(crate) fn from_bound_action(
        action: &openlife_core::agent::AgentAction,
        receipt: &openlife_core::tool_execution_receipt::ToolExecutionReceipt,
        run_id: &str,
    ) -> Option<Self> {
        if receipt.mechanically_valid_terminal().is_err()
            || !receipt.is_runtime_bound_to_action(
                run_id,
                &action.id,
                &action.action_type,
                action.target.as_deref(),
                &action.input,
            )
        {
            return None;
        }

        let action_ref = strict_uuid_ref(&action.id, "unknown_action");
        let run_ref = strict_uuid_ref(run_id, "unknown_run");
        let source = match receipt.dispatch_kind {
            openlife_core::tool_execution_receipt::ToolDispatchKind::Local => "local",
            openlife_core::tool_execution_receipt::ToolDispatchKind::Network => "network",
            openlife_core::tool_execution_receipt::ToolDispatchKind::McpStdio => "mcp",
            openlife_core::tool_execution_receipt::ToolDispatchKind::Simulated => "simulated",
            openlife_core::tool_execution_receipt::ToolDispatchKind::NotAttempted
            | openlife_core::tool_execution_receipt::ToolDispatchKind::Unknown => "unknown",
        }
        .into();
        let (status, failure_code) = if receipt.proves_success() {
            (ProductToolCallStatus::Success, None)
        } else {
            let (status, code) = match (
                receipt.transport_status,
                receipt.effect_status,
                receipt.execution_outcome,
            ) {
                (
                    openlife_core::tool_execution_receipt::ToolTransportStatus::ResponseObserved,
                    openlife_core::tool_execution_receipt::ToolEffectStatus::Unknown,
                    _,
                ) => (
                    ProductToolCallStatus::EffectUnknown,
                    ProductToolFailureCode::ToolEffectUnknown,
                ),
                (
                    openlife_core::tool_execution_receipt::ToolTransportStatus::RemoteUnknown,
                    _,
                    _,
                ) => (
                    ProductToolCallStatus::RemoteUnknown,
                    ProductToolFailureCode::ToolRemoteStateUnknown,
                ),
                (
                    openlife_core::tool_execution_receipt::ToolTransportStatus::LocalAborted,
                    _,
                    _,
                ) => (
                    ProductToolCallStatus::LocallyAborted,
                    ProductToolFailureCode::ToolLocallyAborted,
                ),
                (
                    openlife_core::tool_execution_receipt::ToolTransportStatus::NotAttempted,
                    _,
                    _,
                ) => (
                    ProductToolCallStatus::NotDispatched,
                    ProductToolFailureCode::ToolNotDispatched,
                ),
                (
                    openlife_core::tool_execution_receipt::ToolTransportStatus::ResponseObserved,
                    _,
                    _,
                ) => (
                    ProductToolCallStatus::Failed,
                    ProductToolFailureCode::ToolFailed,
                ),
                (openlife_core::tool_execution_receipt::ToolTransportStatus::Dispatched, _, _) => (
                    ProductToolCallStatus::Unknown,
                    ProductToolFailureCode::ToolStateUnknown,
                ),
            };
            (status, Some(code))
        };

        Some(Self {
            bound_action_id: action.id.clone(),
            bound_run_id: run_id.to_string(),
            bound_receipt: receipt.clone(),
            action_ref,
            run_ref,
            tool_ref: ProductToolReference {
                // A manifest id is an execution identifier, not a product-safe
                // label. Until ToolGateway provides a registry-attested public
                // label, fail closed instead of echoing code-shaped content.
                id: "unknown_tool".into(),
                source,
            },
            receipt: ProductToolExecutionReceipt {
                receipt_ref: strict_uuid_ref(&receipt.receipt_id, "unknown_receipt"),
                request_digest: strict_opaque_sha256(&receipt.request_digest)
                    .unwrap_or_else(|| "unknown".into()),
                action_effect: receipt.action_effect,
                idempotency_contract: receipt.idempotency_contract,
                dispatch_kind: receipt.dispatch_kind,
                dispatch_attempt_count: receipt.dispatch_attempt_count,
                dispatch_observed: receipt.dispatch_observed,
                transport_status: receipt.transport_status,
                effect_status: receipt.effect_status,
                outcome: receipt.execution_outcome,
                audit_persistence_status: receipt.audit_persistence_status,
                verified: true,
            },
            status,
            failure_code,
        })
    }

    pub(crate) fn bound_action_id(&self) -> &str {
        &self.bound_action_id
    }

    fn matches_current_envelope(&self, call: &crate::ToolCallResult) -> bool {
        call.action_id.as_deref() == Some(self.bound_action_id.as_str())
            && call.run_id.as_deref() == Some(self.bound_run_id.as_str())
            && call
                .execution_receipt
                .as_ref()
                .is_some_and(|receipt| receipt == &self.bound_receipt)
    }

    fn product_result(&self) -> ProductToolCallResult {
        ProductToolCallResult {
            tool_ref: self.tool_ref.clone(),
            action_ref: self.action_ref.clone(),
            run_ref: (self.run_ref != "unknown_run").then(|| self.run_ref.clone()),
            status: self.status,
            requires_confirmation: false,
            failure_code: self.failure_code,
            privacy_warning_count: 0,
            proposal_ref: None,
            execution_receipt: Some(self.receipt.clone()),
            // Transient trace receipts and proposal ids have no exact binding
            // to this immutable execution projection. They remain absent until
            // a canonical store projection can prove that binding.
            output_receipt: None,
        }
    }
}

/// Product-safe projection of the legacy broad AgentState read model.
///
/// This is intentionally an explicit allow-list DTO instead of a transparent
/// wrapper around the canonical snapshot. Adding a field to the canonical
/// state therefore cannot silently expand the shipped IPC privacy boundary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductToolCallResult {
    tool_ref: ProductToolReference,
    action_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_ref: Option<String>,
    status: ProductToolCallStatus,
    requires_confirmation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_code: Option<ProductToolFailureCode>,
    privacy_warning_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    proposal_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_receipt: Option<ProductToolExecutionReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_receipt: Option<ProductContentReceipt>,
}

impl ProductToolCallResult {
    pub(crate) fn from_internal(call: &crate::ToolCallResult) -> Self {
        if let Some(verified) = call
            .product_projection
            .as_ref()
            .filter(|projection| projection.matches_current_envelope(call))
        {
            return verified.product_result();
        }
        Self {
            tool_ref: ProductToolReference {
                id: "unknown_tool".into(),
                source: "unknown".into(),
            },
            action_ref: "unknown_action".into(),
            run_ref: None,
            status: ProductToolCallStatus::Unknown,
            requires_confirmation: false,
            failure_code: Some(ProductToolFailureCode::ToolEvidenceUnverified),
            privacy_warning_count: 0,
            proposal_ref: None,
            execution_receipt: None,
            output_receipt: None,
        }
    }
}

impl ProductContentReceipt {
    fn from_receipt(receipt: ContentReceipt, verified_by_store: bool) -> Self {
        Self {
            version: receipt.version(),
            kind: receipt.kind(),
            provenance: receipt.provenance(),
            byte_count: receipt.byte_count(),
            digest: receipt.public_digest(),
            verified: verified_by_store && !receipt.is_legacy_unverified(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductToolActionTrace {
    action_id: String,
    step_index: u32,
    tool_call_index: u32,
    action_type: String,
    tool_name: String,
    tool_source: String,
    action_category: String,
    risk_level: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_decision: Option<String>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    proposal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_receipt: Option<ProductContentReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
    metadata_safe: bool,
}

impl ProductToolActionTrace {
    fn from_trace(trace: ToolActionTraceEnvelope, verified_by_store: bool) -> Self {
        let metadata_safe = trace.metadata_safe;
        let tool_name = if verified_by_store {
            required_public_code(trace.tool_name, "unknown_tool")
        } else {
            "unknown_tool".into()
        };
        Self {
            action_id: required_public_code(trace.action_id, "unknown_action"),
            step_index: trace.step_index,
            tool_call_index: trace.tool_call_index,
            action_type: required_public_code(trace.action_type, "unknown_action_type"),
            tool_name,
            tool_source: required_public_code(trace.tool_source, "unknown_source"),
            action_category: required_public_code(trace.action_category, "unknown_action_category"),
            risk_level: required_public_code(trace.risk_level, "unknown_risk"),
            permission_decision: trace.permission_decision.and_then(public_code),
            status: required_public_code(trace.status, "unknown_status"),
            proposal_id: trace.proposal_id.and_then(public_code),
            observation_id: trace.observation_id.and_then(public_code),
            output_preview: metadata_safe
                .then(|| {
                    trace
                        .output_preview
                        .and_then(product_redacted_byte_count_preview)
                })
                .flatten(),
            output_receipt: trace
                .output_receipt
                .map(|receipt| ProductContentReceipt::from_receipt(receipt, verified_by_store)),
            started_at: trace.started_at,
            finished_at: trace.finished_at,
            metadata_safe,
        }
    }

    /// A live ToolGateway result has not yet crossed the sealed canonical
    /// canonical Work-run reload boundary. The trace's `metadata_safe` flag and
    /// code-shaped `tool_name` are not source authority: the transient product
    /// view keeps the name unknown and every receipt explicitly unverified.
    pub(crate) fn from_transient_trace(trace: ToolActionTraceEnvelope) -> Self {
        Self::from_trace(trace, false)
    }
}
