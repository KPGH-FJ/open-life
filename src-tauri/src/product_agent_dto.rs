//! Explicit product-safe AgentRun and Main Chat trace projection.
//!
//! Core receipt structs intentionally contain canonical owner identities and
//! keyed verification material. No shipped IPC surface may serialize those
//! structs directly. This module is the single adapter that emits the six
//! public receipt facts understood by the frontend.

use openlife_core::agent::{ContentReceipt, ReactActionTraceEnvelope};
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

pub(crate) fn product_transcript_summary(
    kind: openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind,
) -> &'static str {
    use openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind;

    match kind {
        ExecutionTranscriptEntryKind::UserInput => "user_input_recorded",
        ExecutionTranscriptEntryKind::RouteDecision => "route_decision_recorded",
        ExecutionTranscriptEntryKind::Plan => "plan_state_recorded",
        ExecutionTranscriptEntryKind::Action => "action_state_recorded",
        ExecutionTranscriptEntryKind::Observation => "observation_state_recorded",
        ExecutionTranscriptEntryKind::FollowUp => "follow_up_state_recorded",
        ExecutionTranscriptEntryKind::PermissionRequest => "permission_request_recorded",
        ExecutionTranscriptEntryKind::ProposalRequest => "proposal_request_recorded",
        ExecutionTranscriptEntryKind::Error => "error_state_recorded",
        ExecutionTranscriptEntryKind::Retry => "retry_state_recorded",
        ExecutionTranscriptEntryKind::FinalResult => "final_result_state_recorded",
        ExecutionTranscriptEntryKind::Fallback => "fallback_state_recorded",
    }
}

/// Product-only transcript view. Canonical transcript metadata contains keyed
/// integrity receipts used by Rust stores; those tags are authorization
/// material and have no frontend consumer. The shipped surface exposes only
/// the user-visible timeline fields.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProductExecutionTranscriptEntry {
    id: String,
    session_id: String,
    kind: openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind,
    summary: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl ProductExecutionTranscriptEntry {
    fn project(
        id: &str,
        session_id: &str,
        kind: openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            id: strict_product_transcript_id(id),
            session_id: strict_product_task_session_id(session_id),
            kind,
            summary: product_transcript_summary(kind).into(),
            created_at,
        }
    }
}

fn strict_product_transcript_id(value: &str) -> String {
    value
        .strip_prefix("mainchat_transcript_")
        .filter(|digest| {
            digest.len() == 8
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        })
        .map(|_| value.to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn strict_product_task_session_id(value: &str) -> String {
    uuid::Uuid::parse_str(value)
        .map(|_| value.to_string())
        .unwrap_or_else(|_| "unknown".into())
}

impl From<&openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>
    for ProductExecutionTranscriptEntry
{
    fn from(entry: &openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry) -> Self {
        Self::project(&entry.id, &entry.session_id, entry.kind, entry.created_at)
    }
}

impl From<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>
    for ProductExecutionTranscriptEntry
{
    fn from(entry: openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry) -> Self {
        Self::project(&entry.id, &entry.session_id, entry.kind, entry.created_at)
    }
}

pub(crate) fn project_execution_transcript(
    transcript: Vec<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>,
) -> Vec<ProductExecutionTranscriptEntry> {
    transcript.into_iter().map(Into::into).collect()
}

pub(crate) fn serialize_product_execution_transcript<S>(
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let product = transcript
        .iter()
        .map(ProductExecutionTranscriptEntry::from)
        .collect::<Vec<_>>();
    product.serialize(serializer)
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
    verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
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
            openlife_core::tool_execution_receipt::ToolDispatchKind::A2a => "a2a",
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
                verified: true,
            },
            status,
            failure_code,
        })
    }

    pub(crate) fn bound_action_id(&self) -> &str {
        &self.bound_action_id
    }

    pub(crate) fn authorizes_exact_current_envelope(
        &self,
        call: &crate::ToolCallResult,
        run_id: &str,
        action_id: &str,
        receipt: &openlife_core::tool_execution_receipt::ToolExecutionReceipt,
    ) -> bool {
        self.bound_run_id == run_id
            && self.bound_action_id == action_id
            && &self.bound_receipt == receipt
            && receipt.proves_success()
            && self.matches_current_envelope(call)
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
            status: self.status.clone(),
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

/// Body-free product projection for the terminal delivery envelope. The
/// canonical delivery remains available inside the runtime and event store;
/// shipped IPC receives only references, counts, and observed state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductFinalDeliveryView {
    delivery_ref: String,
    task_ref: String,
    run_ref: String,
    status: String,
    completed_action_count: usize,
    observation_count: usize,
    proposal_count: usize,
    blocker_count: usize,
    pending_user_action_count: usize,
    durable_change_count: usize,
    next_step_count: usize,
    trace_available: bool,
    kernel_event_count: Option<usize>,
    durable_event_count: usize,
    has_assistant_message: bool,
    tool_call_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductOpenLifeTurnTerminal {
    runtime_owner: String,
    status: String,
    state: String,
    final_delivery: ProductFinalDeliveryView,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_session_id: Option<String>,
    blockers: Vec<String>,
    proposals: Vec<String>,
    legacy_fallback_used: bool,
    legacy_runtime_invoked: bool,
    single_step_fallback_used: bool,
    direct_writes_executed: bool,
    provider_invocation_status: crate::main_chat_turn_runtime::ProviderInvocationState,
    model_invoked: bool,
    tool_invoked: bool,
}

impl From<&crate::main_chat_turn_runtime::CanonicalFinalDeliveryView> for ProductFinalDeliveryView {
    fn from(delivery: &crate::main_chat_turn_runtime::CanonicalFinalDeliveryView) -> Self {
        let (_, delivery_digest) =
            openlife_core::agent::metadata_safe::metadata_safe_text_digest(&delivery.delivery_id);
        let status = match delivery.status.as_str() {
            "completed"
            | "completed_with_pending_items"
            | "blocked"
            | "failed"
            | "cancelled"
            | "interrupted" => delivery.status.clone(),
            _ => "unknown".into(),
        };
        Self {
            delivery_ref: format!("delivery:{delivery_digest}"),
            task_ref: strict_uuid_ref(&delivery.task_id, "unknown_task"),
            run_ref: strict_uuid_ref(&delivery.run_id, "unknown_run"),
            status,
            completed_action_count: delivery.completed_actions.len(),
            observation_count: delivery.observations_used.len(),
            proposal_count: delivery.proposals_created.len(),
            blocker_count: delivery.blockers.len(),
            pending_user_action_count: delivery.pending_user_actions.len(),
            durable_change_count: delivery.durable_changes.len(),
            next_step_count: delivery.next_steps.len(),
            trace_available: delivery.trace_available,
            kernel_event_count: delivery.kernel_event_count,
            durable_event_count: delivery.durable_event_count,
            has_assistant_message: delivery.has_assistant_message,
            tool_call_count: delivery.tool_call_count,
        }
    }
}

fn product_terminal_status(value: &str) -> String {
    match value {
        "completed"
        | "completed_with_pending_items"
        | "blocked"
        | "failed"
        | "cancelled"
        | "interrupted" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_terminal_state(value: &str) -> String {
    match value {
        "DirectAnswer" | "direct" => "direct_answer",
        "ReadOnlyTool" => "read_only_tool",
        "WriteOutcome" => "write_outcome",
        "PlanExecute" => "plan_execute",
        "GovernedBlocker" => "governed_blocker",
        _ => "unknown",
    }
    .into()
}

fn product_terminal_blocker(value: &str) -> String {
    match value {
        "tool_permission_required"
        | "proposal_review_required"
        | "dangerous_action_hard_block"
        | "external_write_requires_confirmation"
        | "policy_confirmation_required"
        | "provider_network_consent_required"
        | "provider_unavailable"
        | "provider_failed"
        | "provider_error"
        | "tool_unavailable"
        | "tool_failed"
        | "tool_error"
        | "runtime_interrupted"
        | "interrupted"
        | "timeout"
        | "cancelled"
        | "unknown_error"
        | "policy_blocker"
        | "web_network_policy_blocked"
        | "memory_lifecycle_reader_unavailable"
        | "stale_context"
        | "missing_action_evidence"
        | "permission_scope_mismatch"
        | "terminal_no_resume"
        | "selected_skill_context_digest_mismatch"
        | "plan_revision_mismatch"
        | "requires_user_decision" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_terminal_proposal_ref(value: &str) -> String {
    let proposal_id = value.strip_prefix("proposal:").unwrap_or(value);
    let proposal_ref = strict_uuid_ref(proposal_id, "unknown_proposal");
    (proposal_ref != "unknown_proposal")
        .then(|| format!("proposal:{proposal_ref}"))
        .unwrap_or(proposal_ref)
}

impl From<&crate::main_chat_turn_runtime::OpenLifeTurnTerminal> for ProductOpenLifeTurnTerminal {
    fn from(terminal: &crate::main_chat_turn_runtime::OpenLifeTurnTerminal) -> Self {
        Self {
            runtime_owner: (terminal.runtime_owner
                == crate::main_chat_turn_runtime::OPENLIFE_TURN_RUNTIME_OWNER)
                .then(|| terminal.runtime_owner.clone())
                .unwrap_or_else(|| "unknown_runtime".into()),
            status: product_terminal_status(&terminal.status),
            state: product_terminal_state(&terminal.state),
            final_delivery: ProductFinalDeliveryView::from(&terminal.final_delivery),
            run_id: terminal
                .run_id
                .as_deref()
                .map(|value| strict_uuid_ref(value, "unknown_run")),
            task_session_id: terminal
                .task_session_id
                .as_deref()
                .map(|value| strict_uuid_ref(value, "unknown_task")),
            blockers: terminal
                .blockers
                .iter()
                .map(|value| product_terminal_blocker(value))
                .collect(),
            proposals: terminal
                .proposals
                .iter()
                .map(|value| product_terminal_proposal_ref(value))
                .collect(),
            legacy_fallback_used: terminal.legacy_fallback_used,
            legacy_runtime_invoked: terminal.legacy_runtime_invoked,
            single_step_fallback_used: terminal.single_step_fallback_used,
            direct_writes_executed: terminal.direct_writes_executed,
            provider_invocation_status: terminal.provider_invocation_status,
            model_invoked: terminal.model_invoked,
            tool_invoked: terminal.tool_invoked,
        }
    }
}

impl serde::Serialize for crate::main_chat_turn_runtime::OpenLifeTurnTerminal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ProductOpenLifeTurnTerminal::from(self).serialize(serializer)
    }
}

/// Product-safe projection of the legacy broad AgentState read model.
///
/// This is intentionally an explicit allow-list DTO instead of a transparent
/// wrapper around the canonical snapshot. Adding a field to the canonical
/// state therefore cannot silently expand the shipped IPC privacy boundary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProductMainChatAgentStateSnapshot {
    task: ProductStateTask,
    route: ProductStateRoute,
    context: Vec<ProductStateContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<ProductStateProvider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<ProductStatePlan>,
    actions: Vec<ProductStateAction>,
    observations: Vec<ProductStateObservation>,
    blockers: Vec<ProductStateBlocker>,
    proposals: Vec<ProductStateProposal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    final_delivery: Option<ProductStateFinalDelivery>,
    diagnostics: Vec<ProductStateDiagnostic>,
    sequence: u64,
    emitted_at: chrono::DateTime<chrono::Utc>,
    events: Vec<ProductStateEvent>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateTask {
    task_id: String,
    run_id: String,
    conversation_id: String,
    user_message_id: String,
    title: String,
    strategy: openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductStrategyRoute,
    status: openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductTaskStatus,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    trace_available: bool,
    controls: Vec<openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductControl>,
    action_ids: Vec<String>,
    observation_ids: Vec<String>,
    blocker_ids: Vec<String>,
    proposal_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    final_delivery_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateRoute {
    strategy: openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductStrategyRoute,
    reason: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateContext {
    context_id: String,
    source_kind: String,
    source_label: String,
    evidence_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateProvider {
    provider: String,
    model: String,
    route_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_config_generation: Option<String>,
    reason: &'static str,
    evidence_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStatePlan {
    plan_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<String>,
    status: String,
    summary: &'static str,
    editable: bool,
    source: String,
    evidence_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_id: Option<String>,
    source_evidence_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    superseded_by_plan_id: Option<String>,
    controls: Vec<String>,
    steps: Vec<ProductStatePlanStep>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStatePlanStep {
    step_id: String,
    plan_id: String,
    index: usize,
    title: &'static str,
    description: &'static str,
    kind: String,
    status: String,
    revision: u64,
    base_plan_revision: u64,
    linked_action_ids: Vec<String>,
    linked_observation_ids: Vec<String>,
    linked_proposal_ids: Vec<String>,
    blocker_ids: Vec<String>,
    linked_final_delivery_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    policy_decision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    evidence_ids: Vec<String>,
    controls: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateAction {
    action_id: String,
    action_type: String,
    target: String,
    label: String,
    status: String,
    risk_level: String,
    policy_decision_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
    observation_ids: Vec<String>,
    retryable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateReadExecution {
    kind: &'static str,
    source_kind: String,
    source_label: String,
    target: &'static str,
    real_read_only_execution: bool,
    fixture_backed: bool,
    network_read_attempted: bool,
    direct_writes_executed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateObservation {
    observation_id: String,
    action_id: String,
    source_kind: String,
    source_label: String,
    preview: &'static str,
    citation_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    read_execution: Option<ProductStateReadExecution>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateBlocker {
    blocker_id: String,
    reason_code: String,
    title: String,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    affected_action_id: Option<String>,
    recoverable: bool,
    controls: Vec<openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductControl>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateProposal {
    proposal_id: String,
    proposal_type: String,
    status: openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductProposalStatus,
    title: String,
    summary: &'static str,
    evidence_ids: Vec<String>,
    action_ids: Vec<String>,
    controls: Vec<openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductControl>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateCompletedAction {
    action_id: String,
    action_type: String,
    target: String,
    status: String,
    observation_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateObservationSummary {
    observation_id: String,
    source_kind: String,
    source_label: String,
    preview: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateProposalSummary {
    proposal_id: String,
    proposal_type: String,
    status: openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductProposalStatus,
    summary: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateBlockerSummary {
    blocker_id: String,
    reason_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    affected_action_id: Option<String>,
    recoverable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateSkippedWork {
    step_id: String,
    title: &'static str,
    reason: &'static str,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStatePendingAction {
    pending_id: String,
    kind: &'static str,
    controls: Vec<openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductControl>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateDurableChange {
    change_type: &'static str,
    target: String,
    provenance_id: String,
    rollback_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateFinalDelivery {
    delivery_id: String,
    task_id: String,
    run_id: String,
    status: openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductDeliveryStatus,
    headline: &'static str,
    answer: &'static str,
    completed_actions: Vec<ProductStateCompletedAction>,
    observations_used: Vec<ProductStateObservationSummary>,
    proposals_created: Vec<ProductStateProposalSummary>,
    blockers: Vec<ProductStateBlockerSummary>,
    skipped_work: Vec<ProductStateSkippedWork>,
    pending_user_actions: Vec<ProductStatePendingAction>,
    durable_changes: Vec<ProductStateDurableChange>,
    next_steps: Vec<&'static str>,
    trace_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateDiagnostic {
    gap_id: String,
    gap_code: String,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductStateEvent {
    event_type: openlife_core::agent::main_chat_runtime_contract::MainChatAgentStateEventType,
    sequence: u64,
    object_id: String,
    evidence_id: String,
}

fn product_state_ref(kind: &str, value: &str) -> String {
    if uuid::Uuid::parse_str(value).is_ok() || strict_product_transcript_id(value) != "unknown" {
        return value.to_string();
    }
    let (_, digest) = openlife_core::agent::metadata_safe::metadata_safe_text_digest(value);
    format!("{kind}:{digest}")
}

fn product_state_action_type(value: &str) -> String {
    match value {
        "direct.answer"
        | "memory.search"
        | "session.search"
        | "file.read"
        | "file.patch"
        | "web.search"
        | "web.fetch"
        | "mcp.read_only"
        | "proposal.create"
        | "memory.write"
        | "life_model.update"
        | "file.write.approved"
        | "calendar.real_write"
        | "email.send"
        | "shell.destructive"
        | "plan_execute.create_session"
        | "memory.governance.plan"
        | "builtin_tool" => value.into(),
        _ => "unknown_action_type".into(),
    }
}

fn product_state_proposal_type(value: &str) -> String {
    match value {
        "goal_update"
        | "state_update"
        | "preference_update"
        | "capability_update"
        | "memory_write"
        | "memory_archive"
        | "tool_permission"
        | "plugin_permission"
        | "scheduled_task"
        | "external_write_action"
        | "model_policy_change"
        | "data_export"
        | "schedule_checkin"
        | "unsupported"
        | "life_model_update" => value.into(),
        _ => "unknown_proposal_type".into(),
    }
}

fn product_state_source_kind(value: &str) -> String {
    match value {
        "context_snapshot" | "tool" | "system" | "web" | "mcp" | "file" | "memory" | "provider"
        | "conversation" | "workspace" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_state_control(value: &str) -> Option<String> {
    matches!(
        value,
        "confirm_plan"
            | "edit_plan"
            | "cancel_task"
            | "open_trace"
            | "execute_step"
            | "skip_step"
            | "review_plan"
            | "resume"
            | "retry"
            | "refresh_context"
            | "open_mailbox"
    )
    .then(|| value.to_string())
}

fn product_state_plan_status(value: &str) -> String {
    match value {
        "draft" | "review_pending" | "confirmed" | "queued" | "executing" | "blocked"
        | "completed" | "failed" | "cancelled" | "superseded" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_state_plan_source(value: &str) -> String {
    match value {
        "plan_execute" | "agent_loop" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_state_step_kind(value: &str) -> String {
    match value {
        "analysis" | "read" | "write" | "tool" | "review" | "governed_step" => value.into(),
        _ => "governed_step".into(),
    }
}

fn product_state_step_status(value: &str) -> String {
    match value {
        "draft" | "queued" | "executing" | "observing" | "blocked" | "completed" | "failed"
        | "cancelled" | "skipped" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_state_action_status(value: &str) -> String {
    match value {
        "queued" | "blocked" | "running" | "succeeded" | "failed" | "cancelled" | "unknown" => {
            value.into()
        }
        _ => "unknown".into(),
    }
}

fn product_state_risk_level(value: &str) -> String {
    match value {
        "safe_read" | "local_low_risk" | "proposal_first" | "external_confirm"
        | "dangerous_blocked" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_state_diagnostic_code(value: &str) -> String {
    match value {
        "missing_run_identity"
        | "agent_run_identity_mismatch"
        | "legacy_agent_run_payload_unverified"
        | "missing_action_evidence"
        | "missing_observation_evidence"
        | "missing_final_delivery"
        | "agent_state_session_not_found"
        | "agent_state_action_queue_store_unavailable" => value.into(),
        _ => "unknown_diagnostic".into(),
    }
}

impl From<openlife_core::agent::main_chat_runtime_contract::MainChatAgentStateSnapshot>
    for ProductMainChatAgentStateSnapshot
{
    fn from(
        snapshot: openlife_core::agent::main_chat_runtime_contract::MainChatAgentStateSnapshot,
    ) -> Self {
        use openlife_core::agent::main_chat_runtime_contract::MainChatAgentStateSnapshot;

        let MainChatAgentStateSnapshot {
            task,
            route,
            context,
            provider,
            plan,
            actions,
            observations,
            blockers,
            proposals,
            final_delivery,
            diagnostics,
            sequence,
            emitted_at,
            events,
        } = snapshot;

        let task = ProductStateTask {
            task_id: product_state_ref("task", &task.task_id),
            run_id: product_state_ref("run", &task.run_id),
            conversation_id: product_state_ref("conversation", &task.conversation_id),
            user_message_id: product_state_ref("message", &task.user_message_id),
            title: "main_chat_task".into(),
            strategy: task.strategy,
            status: task.status,
            created_at: task.created_at,
            updated_at: task.updated_at,
            trace_available: task.trace_available,
            controls: task.controls,
            action_ids: task
                .action_ids
                .iter()
                .map(|value| product_state_ref("action", value))
                .collect(),
            observation_ids: task
                .observation_ids
                .iter()
                .map(|value| product_state_ref("observation", value))
                .collect(),
            blocker_ids: task
                .blocker_ids
                .iter()
                .map(|value| product_state_ref("blocker", value))
                .collect(),
            proposal_ids: task
                .proposal_ids
                .iter()
                .map(|value| product_state_ref("proposal", value))
                .collect(),
            final_delivery_id: task
                .final_delivery_id
                .as_deref()
                .map(|value| product_state_ref("delivery", value)),
        };
        let route = ProductStateRoute {
            strategy: route.strategy,
            reason: "strategy_route_recorded",
            confidence: route.confidence,
        };
        let context = context
            .into_iter()
            .map(|context| {
                let source_kind = product_state_source_kind(&context.source_kind);
                ProductStateContext {
                    context_id: product_state_ref("context", &context.context_id),
                    source_label: source_kind.clone(),
                    source_kind,
                    evidence_id: product_state_ref("evidence", &context.evidence_id),
                }
            })
            .collect();
        let provider = provider.map(|provider| ProductStateProvider {
            provider: product_state_ref("provider", &provider.provider),
            model: product_state_ref("model", &provider.model),
            route_type: match provider.route_type.as_str() {
                "local" | "cloud" | "agent_runtime" | "scripted" => provider.route_type,
                _ => "unknown".into(),
            },
            provider_config_generation: provider
                .provider_config_generation
                .as_deref()
                .map(|value| product_state_ref("provider_config", value)),
            reason: "route_reason_recorded",
            evidence_id: product_state_ref("evidence", &provider.evidence_id),
        });
        let plan = plan.map(|plan| ProductStatePlan {
            plan_id: product_state_ref("plan", &plan.plan_id),
            plan_session_id: plan
                .plan_session_id
                .as_deref()
                .map(|value| product_state_ref("plan_session", value)),
            task_session_id: plan
                .task_session_id
                .as_deref()
                .map(|value| product_state_ref("task", value)),
            run_id: plan
                .run_id
                .as_deref()
                .map(|value| product_state_ref("run", value)),
            status: product_state_plan_status(&plan.status),
            summary: "plan_state_recorded",
            editable: plan.editable,
            source: product_state_plan_source(&plan.source),
            evidence_id: product_state_ref("evidence", &plan.evidence_id),
            revision: plan.revision,
            revision_id: plan
                .revision_id
                .as_deref()
                .map(|value| product_state_ref("plan_revision", value)),
            review_id: plan
                .review_id
                .as_deref()
                .map(|value| product_state_ref("review", value)),
            source_evidence_ids: plan
                .source_evidence_ids
                .iter()
                .map(|value| product_state_ref("evidence", value))
                .collect(),
            superseded_by_plan_id: plan
                .superseded_by_plan_id
                .as_deref()
                .map(|value| product_state_ref("plan", value)),
            controls: plan
                .controls
                .iter()
                .filter_map(|value| product_state_control(value))
                .collect(),
            steps: plan
                .steps
                .into_iter()
                .map(|step| ProductStatePlanStep {
                    step_id: product_state_ref("plan_step", &step.step_id),
                    plan_id: product_state_ref("plan", &step.plan_id),
                    index: step.index,
                    title: "plan_step",
                    description: "content_in_canonical_plan",
                    kind: product_state_step_kind(&step.kind),
                    status: product_state_step_status(&step.status),
                    revision: step.revision,
                    base_plan_revision: step.base_plan_revision,
                    linked_action_ids: step
                        .linked_action_ids
                        .iter()
                        .map(|value| product_state_ref("action", value))
                        .collect(),
                    linked_observation_ids: step
                        .linked_observation_ids
                        .iter()
                        .map(|value| product_state_ref("observation", value))
                        .collect(),
                    linked_proposal_ids: step
                        .linked_proposal_ids
                        .iter()
                        .map(|value| product_state_ref("proposal", value))
                        .collect(),
                    blocker_ids: step
                        .blocker_ids
                        .iter()
                        .map(|value| product_state_ref("blocker", value))
                        .collect(),
                    linked_final_delivery_ids: step
                        .linked_final_delivery_ids
                        .iter()
                        .map(|value| product_state_ref("delivery", value))
                        .collect(),
                    skip_reason: step.skip_reason.as_ref().map(|_| "step_skipped"),
                    policy_decision_id: step
                        .policy_decision_id
                        .as_deref()
                        .map(|value| product_state_ref("policy", value)),
                    reason: step.reason.as_ref().map(|_| "step_state_recorded"),
                    evidence_ids: step
                        .evidence_ids
                        .iter()
                        .map(|value| product_state_ref("evidence", value))
                        .collect(),
                    controls: step
                        .controls
                        .iter()
                        .filter_map(|value| product_state_control(value))
                        .collect(),
                })
                .collect(),
        });
        let actions = actions
            .into_iter()
            .map(|action| {
                let action_type = product_state_action_type(&action.action_type);
                ProductStateAction {
                    action_id: product_state_ref("action", &action.action_id),
                    target: action_type.clone(),
                    label: action_type.clone(),
                    action_type,
                    status: product_state_action_status(&action.status),
                    risk_level: product_state_risk_level(&action.risk_level),
                    policy_decision_id: product_state_ref("policy", &action.policy_decision_id),
                    started_at: action.started_at,
                    finished_at: action.finished_at,
                    observation_ids: action
                        .observation_ids
                        .iter()
                        .map(|value| product_state_ref("observation", value))
                        .collect(),
                    retryable: action.retryable,
                }
            })
            .collect();
        let observations = observations
            .into_iter()
            .map(|observation| {
                let source_kind = product_state_source_kind(&observation.source_kind);
                ProductStateObservation {
                    observation_id: product_state_ref("observation", &observation.observation_id),
                    action_id: product_state_ref("action", &observation.action_id),
                    source_label: source_kind.clone(),
                    source_kind,
                    preview: "observation_recorded",
                    citation_available: observation.citation_available,
                    read_execution: observation.read_execution.map(|read| {
                        let source_kind = product_state_source_kind(&read.source_kind);
                        ProductStateReadExecution {
                            kind: "read_execution",
                            source_label: source_kind.clone(),
                            source_kind,
                            target: "redacted_target",
                            real_read_only_execution: read.real_read_only_execution,
                            fixture_backed: read.fixture_backed,
                            network_read_attempted: read.network_read_attempted,
                            direct_writes_executed: read.direct_writes_executed,
                        }
                    }),
                    created_at: observation.created_at,
                }
            })
            .collect();
        let blockers = blockers
            .into_iter()
            .map(|blocker| {
                let reason_code = product_terminal_blocker(&blocker.reason_code);
                ProductStateBlocker {
                    blocker_id: product_state_ref("blocker", &blocker.blocker_id),
                    title: reason_code.clone(),
                    detail: reason_code.clone(),
                    reason_code,
                    affected_action_id: blocker
                        .affected_action_id
                        .as_deref()
                        .map(|value| product_state_ref("action", value)),
                    recoverable: blocker.recoverable,
                    controls: blocker.controls,
                }
            })
            .collect();
        let proposals = proposals
            .into_iter()
            .map(|proposal| {
                let proposal_type = product_state_proposal_type(&proposal.proposal_type);
                ProductStateProposal {
                    proposal_id: product_state_ref("proposal", &proposal.proposal_id),
                    title: proposal_type.clone(),
                    proposal_type,
                    status: proposal.status,
                    summary: "proposal_state_recorded",
                    evidence_ids: proposal
                        .evidence_ids
                        .iter()
                        .map(|value| product_state_ref("evidence", value))
                        .collect(),
                    action_ids: proposal
                        .action_ids
                        .iter()
                        .map(|value| product_state_ref("action", value))
                        .collect(),
                    controls: proposal.controls,
                }
            })
            .collect();
        let final_delivery = final_delivery.map(|delivery| ProductStateFinalDelivery {
            delivery_id: product_state_ref("delivery", &delivery.delivery_id),
            task_id: product_state_ref("task", &delivery.task_id),
            run_id: product_state_ref("run", &delivery.run_id),
            status: delivery.status,
            headline: "final_delivery_recorded",
            answer: "final_delivery_recorded",
            completed_actions: delivery
                .completed_actions
                .into_iter()
                .map(|action| {
                    let action_type = product_state_action_type(&action.action_type);
                    ProductStateCompletedAction {
                        action_id: product_state_ref("action", &action.action_id),
                        target: action_type.clone(),
                        action_type,
                        status: product_state_action_status(&action.status),
                        observation_ids: action
                            .observation_ids
                            .iter()
                            .map(|value| product_state_ref("observation", value))
                            .collect(),
                    }
                })
                .collect(),
            observations_used: delivery
                .observations_used
                .into_iter()
                .map(|observation| {
                    let source_kind = product_state_source_kind(&observation.source_kind);
                    ProductStateObservationSummary {
                        observation_id: product_state_ref(
                            "observation",
                            &observation.observation_id,
                        ),
                        source_label: source_kind.clone(),
                        source_kind,
                        preview: "observation_recorded",
                    }
                })
                .collect(),
            proposals_created: delivery
                .proposals_created
                .into_iter()
                .map(|proposal| ProductStateProposalSummary {
                    proposal_id: product_state_ref("proposal", &proposal.proposal_id),
                    proposal_type: product_state_proposal_type(&proposal.proposal_type),
                    status: proposal.status,
                    summary: "proposal_state_recorded",
                })
                .collect(),
            blockers: delivery
                .blockers
                .into_iter()
                .map(|blocker| ProductStateBlockerSummary {
                    blocker_id: product_state_ref("blocker", &blocker.blocker_id),
                    reason_code: product_terminal_blocker(&blocker.reason_code),
                    affected_action_id: blocker
                        .affected_action_id
                        .as_deref()
                        .map(|value| product_state_ref("action", value)),
                    recoverable: blocker.recoverable,
                })
                .collect(),
            skipped_work: delivery
                .skipped_work
                .into_iter()
                .map(|skipped| ProductStateSkippedWork {
                    step_id: product_state_ref("plan_step", &skipped.step_id),
                    title: "plan_step",
                    reason: "step_skipped",
                    status: product_state_step_status(&skipped.status),
                })
                .collect(),
            pending_user_actions: delivery
                .pending_user_actions
                .into_iter()
                .map(|pending| ProductStatePendingAction {
                    pending_id: product_state_ref("pending", &pending.pending_id),
                    kind: "review_required",
                    controls: pending.controls,
                })
                .collect(),
            durable_changes: delivery
                .durable_changes
                .into_iter()
                .map(|change| ProductStateDurableChange {
                    change_type: "canonical_change",
                    target: product_state_ref("canonical_object", &change.target),
                    provenance_id: product_state_ref("provenance", &change.provenance_id),
                    rollback_available: change.rollback_available,
                })
                .collect(),
            next_steps: delivery
                .next_steps
                .iter()
                .map(|_| "review_task_state")
                .collect(),
            trace_available: delivery.trace_available,
        });
        let diagnostics = diagnostics
            .into_iter()
            .map(|diagnostic| {
                let gap_code = product_state_diagnostic_code(&diagnostic.gap_code);
                ProductStateDiagnostic {
                    gap_id: product_state_ref("diagnostic", &diagnostic.gap_id),
                    detail: gap_code.clone(),
                    gap_code,
                    evidence_id: diagnostic
                        .evidence_id
                        .as_deref()
                        .map(|value| product_state_ref("evidence", value)),
                }
            })
            .collect();
        let events = events
            .into_iter()
            .map(|event| ProductStateEvent {
                event_type: event.event_type,
                sequence: event.sequence,
                object_id: product_state_ref("object", &event.object_id),
                evidence_id: product_state_ref("evidence", &event.evidence_id),
            })
            .collect();

        Self {
            task,
            route,
            context,
            provider,
            plan,
            actions,
            observations,
            blockers,
            proposals,
            final_delivery,
            diagnostics,
            sequence,
            emitted_at,
            events,
        }
    }
}

pub(crate) fn serialize_product_agent_state<S>(
    state: &Option<openlife_core::agent::main_chat_runtime_contract::MainChatAgentStateSnapshot>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    state
        .clone()
        .map(ProductMainChatAgentStateSnapshot::from)
        .serialize(serializer)
}

/// Minimal product projection for a tool call. Raw arguments, sanitized
/// arguments, adapter output, error bodies, privacy-warning bodies, and
/// replay authority intentionally remain inside the runtime.
#[derive(Debug, Clone, PartialEq, Serialize)]
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

    #[cfg(test)]
    pub(crate) fn from_unverified_receipt(receipt: ContentReceipt) -> Self {
        Self::from_receipt(receipt, false)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductReactActionTrace {
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

impl ProductReactActionTrace {
    fn from_trace(trace: ReactActionTraceEnvelope, verified_by_store: bool) -> Self {
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
    /// AgentRun reload boundary. The trace's `metadata_safe` flag and
    /// code-shaped `tool_name` are not source authority: the transient product
    /// view keeps the name unknown and every receipt explicitly unverified.
    pub(crate) fn from_transient_trace(trace: ReactActionTraceEnvelope) -> Self {
        Self::from_trace(trace, false)
    }

    /// AgentRunStore validates current store identity, semantic binding and
    /// keyed authority before returning a current row.
    pub(crate) fn from_verified_store_trace(
        trace: ReactActionTraceEnvelope,
        _authority: &crate::commands::agent::VerifiedAgentRunProjectionAuthority,
    ) -> Self {
        Self::from_trace(trace, true)
    }

    pub(crate) fn action_id(&self) -> &str {
        &self.action_id
    }

    pub(crate) fn action_type(&self) -> &str {
        &self.action_type
    }

    pub(crate) fn tool_name(&self) -> &str {
        &self.tool_name
    }

    pub(crate) fn tool_source(&self) -> &str {
        &self.tool_source
    }

    pub(crate) fn observation_id(&self) -> Option<&str> {
        self.observation_id.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductContextSummary {
    life_model_empty: bool,
    memory_hit_count: i64,
    used_tools_prompt: bool,
    redaction_applied: bool,
    redaction_level: openlife_core::agent::RedactionLevel,
}

impl From<openlife_core::agent::ContextSummary> for ProductContextSummary {
    fn from(summary: openlife_core::agent::ContextSummary) -> Self {
        Self {
            life_model_empty: summary.life_model_empty,
            memory_hit_count: summary.memory_hit_count,
            used_tools_prompt: summary.used_tools_prompt,
            redaction_applied: summary.redaction_applied,
            redaction_level: summary.redaction_level,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductModelRouteTrace {
    provider: String,
    model: String,
    route_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    privacy_level: openlife_core::agent::RedactionLevel,
    retry_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_health_is_estimated: Option<bool>,
}

impl From<openlife_core::agent::ModelRouteTrace> for ProductModelRouteTrace {
    fn from(route: openlife_core::agent::ModelRouteTrace) -> Self {
        Self {
            provider: required_public_code(route.provider, "unknown_provider"),
            model: public_text(route.model, 160).unwrap_or_else(|| "unknown_model".into()),
            route_type: required_public_code(route.route_type, "unknown_route"),
            reason: public_code(route.reason),
            privacy_level: route.privacy_level,
            retry_count: route.retry_count,
            fallback_reason: route.fallback_reason.and_then(public_code),
            provider_health_is_estimated: route.provider_health_is_estimated,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductToolActionScope {
    tool_name: String,
    source: String,
    risk_level: String,
    capabilities: Vec<String>,
}

impl From<openlife_core::agent::ToolActionScope> for ProductToolActionScope {
    fn from(scope: openlife_core::agent::ToolActionScope) -> Self {
        Self {
            tool_name: public_text(scope.tool_name, 160).unwrap_or_else(|| "unknown_tool".into()),
            source: required_public_code(scope.source, "unknown_source"),
            risk_level: required_public_code(scope.risk_level, "unknown_risk"),
            capabilities: scope
                .capabilities
                .into_iter()
                .filter_map(public_code)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductAgentAction {
    id: String,
    action_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_decision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_scope: Option<ProductToolActionScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    react_trace: Option<ProductReactActionTrace>,
}

impl ProductAgentAction {
    fn from_verified_store_action(
        action: openlife_core::agent::AgentAction,
        authority: &crate::commands::agent::VerifiedAgentRunProjectionAuthority,
    ) -> Self {
        let openlife_core::agent::AgentAction {
            id,
            action_type,
            target,
            input: _,
            output: _,
            status,
            permission_decision,
            started_at,
            finished_at,
            error,
            timestamp,
            tool_scope,
            react_trace,
            runtime_execution_receipt: _,
        } = action;
        Self {
            id: required_public_code(id, "unknown_action"),
            action_type: required_public_code(action_type, "unknown_action_type"),
            target: target.and_then(|value| public_text(value, 256)),
            status: required_public_code(status, "unknown_status"),
            permission_decision: permission_decision.and_then(public_code),
            started_at,
            finished_at,
            error: error.map(|_| "action_failed".into()),
            timestamp,
            tool_scope: tool_scope.map(Into::into),
            react_trace: react_trace
                .map(|trace| ProductReactActionTrace::from_verified_store_trace(trace, authority)),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductAgentObservation {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    action_id: Option<String>,
    content: String,
    source: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    react_trace: Option<ProductReactActionTrace>,
}

impl ProductAgentObservation {
    fn from_verified_store_observation(
        observation: openlife_core::agent::AgentObservation,
        authority: &crate::commands::agent::VerifiedAgentRunProjectionAuthority,
    ) -> Self {
        let openlife_core::agent::AgentObservation {
            id,
            action_id,
            content,
            source,
            structured_result: _,
            timestamp,
            react_trace,
        } = observation;
        let content = if react_trace.is_some() {
            "Tool result body is represented by the execution receipt.".into()
        } else {
            public_text(content, 512).unwrap_or_else(|| "observation_not_available".into())
        };
        Self {
            id: required_public_code(id, "unknown_observation"),
            action_id: action_id.and_then(public_code),
            content,
            source: required_public_code(source, "unknown_source"),
            timestamp,
            react_trace: react_trace
                .map(|trace| ProductReactActionTrace::from_verified_store_trace(trace, authority)),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductAgentStatusUpdate {
    phase: String,
    message: String,
    step_index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_index: Option<u32>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl From<openlife_core::agent::AgentLoopStatusUpdate> for ProductAgentStatusUpdate {
    fn from(update: openlife_core::agent::AgentLoopStatusUpdate) -> Self {
        let phase = update.phase.to_string();
        Self {
            message: public_text(update.message, 512).unwrap_or_else(|| phase.clone()),
            phase,
            step_index: update.step_index,
            tool_call_index: update.tool_call_index,
            timestamp: update.timestamp,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductAgentRunError {
    message: &'static str,
    phase: String,
    recoverable: bool,
}

impl From<openlife_core::agent::AgentRunError> for ProductAgentRunError {
    fn from(error: openlife_core::agent::AgentRunError) -> Self {
        Self::from_status(error, openlife_core::agent::AgentRunStatus::Failed)
    }
}

impl ProductAgentRunError {
    fn from_status(
        error: openlife_core::agent::AgentRunError,
        status: openlife_core::agent::AgentRunStatus,
    ) -> Self {
        Self {
            message: if status == openlife_core::agent::AgentRunStatus::RemoteUnknown {
                "remote_state_unknown"
            } else {
                "run_failed"
            },
            phase: required_public_code(error.phase, "unknown_phase"),
            recoverable: error.recoverable,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductHSSelectionAudit {
    selected_policy_ids: Vec<String>,
    selected_heuristic_ids: Vec<String>,
    estimated_tokens: usize,
    token_budget: usize,
}

impl From<openlife_core::agent::HSSelectionAudit> for ProductHSSelectionAudit {
    fn from(audit: openlife_core::agent::HSSelectionAudit) -> Self {
        Self {
            selected_policy_ids: audit
                .selected_policy_ids
                .into_iter()
                .filter_map(public_code)
                .collect(),
            selected_heuristic_ids: audit
                .selected_heuristic_ids
                .into_iter()
                .filter_map(public_code)
                .collect(),
            estimated_tokens: audit.estimated_tokens,
            token_budget: audit.token_budget,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductHSBehaviorCheckSummary {
    id: String,
    label: String,
    passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
}

impl From<openlife_core::agent::HSBehaviorCheckSummary> for ProductHSBehaviorCheckSummary {
    fn from(check: openlife_core::agent::HSBehaviorCheckSummary) -> Self {
        Self {
            id: required_public_code(check.id, "unknown_check"),
            label: public_text(check.label, 160).unwrap_or_else(|| "Behavior check".into()),
            passed: check.passed,
            summary: check.summary.and_then(|value| public_text(value, 256)),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductAgentRun {
    id: String,
    task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    status: openlife_core::agent::AgentRunStatus,
    kind: openlife_core::agent::AgentTaskKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_summary: Option<ProductContextSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_route: Option<ProductModelRouteTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ProductAgentRunError>,
    generated_proposals: Vec<String>,
    actions: Vec<ProductAgentAction>,
    observations: Vec<ProductAgentObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_strategy: Option<String>,
    legacy_payload_unverified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    hs_selection_audit: Option<ProductHSSelectionAudit>,
    behavior_checks: Vec<ProductHSBehaviorCheckSummary>,
    warnings: Vec<String>,
    status_updates: Vec<ProductAgentStatusUpdate>,
    step_count: u32,
    tool_call_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delete_reason: Option<String>,
    started_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ProductAgentRun {
    pub(crate) fn from_verified_store_run(
        run: openlife_core::agent::AgentRun,
        authority: &crate::commands::agent::VerifiedAgentRunProjectionAuthority,
    ) -> Self {
        let status = run.status;
        Self {
            id: required_public_code(run.id, "unknown_run"),
            task_id: required_public_code(run.task_id, "unknown_task"),
            session_id: run.session_id.and_then(public_code),
            status,
            kind: run.kind,
            context_summary: run.context_summary.map(Into::into),
            model_route: run.model_route.map(Into::into),
            output_preview: run.output_preview.and_then(|value| public_text(value, 512)),
            error: run
                .error
                .map(|error| ProductAgentRunError::from_status(error, status)),
            generated_proposals: run
                .generated_proposals
                .into_iter()
                .filter_map(public_code)
                .collect(),
            actions: run
                .actions
                .into_iter()
                .map(|action| ProductAgentAction::from_verified_store_action(action, authority))
                .collect(),
            observations: run
                .observations
                .into_iter()
                .map(|observation| {
                    ProductAgentObservation::from_verified_store_observation(observation, authority)
                })
                .collect(),
            reasoning_strategy: run.reasoning_strategy.and_then(public_code),
            legacy_payload_unverified: run.legacy_payload_unverified,
            hs_selection_audit: run.hs_selection_audit.map(Into::into),
            behavior_checks: run.behavior_checks.into_iter().map(Into::into).collect(),
            warnings: run.warnings.into_iter().filter_map(public_code).collect(),
            status_updates: run.status_updates.into_iter().map(Into::into).collect(),
            step_count: run.step_count,
            tool_call_count: run.tool_call_count,
            deleted_at: run.deleted_at,
            delete_reason: run.delete_reason.and_then(|value| public_text(value, 256)),
            started_at: run.started_at,
            finished_at: run.finished_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn canonical_remote_unknown_run_projects_unknown_not_failed() {
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("a2a.call_agent");
        run.status = openlife_core::agent::AgentRunStatus::RemoteUnknown;
        run.error = Some(openlife_core::agent::AgentRunError {
            message: "a2a_remote_state_unknown".into(),
            phase: "startup_projection_recovery".into(),
            recoverable: false,
        });
        run.finished_at = Some(chrono::Utc::now());

        let product = crate::commands::agent::project_verified_agent_run(run);
        let encoded = serde_json::to_value(product).unwrap();
        assert_eq!(encoded["status"], "remote_unknown");
        assert_ne!(encoded["status"], "failed");
        assert_eq!(encoded["error"]["message"], "remote_state_unknown");
        assert_ne!(encoded["error"]["message"], "run_failed");
    }

    fn typescript_interface_fields(name: &str) -> BTreeSet<String> {
        let typescript = include_str!("../../frontend/src/tauri.ts");
        let marker = format!("export interface {name} {{");
        typescript
            .split(&marker)
            .nth(1)
            .and_then(|tail| tail.split("\n}").next())
            .unwrap_or_else(|| panic!("TypeScript interface {name} is missing"))
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with("/**"))
            .filter_map(|line| line.split_once(':').map(|(field, _)| field))
            .map(|field| field.trim().trim_end_matches('?').to_string())
            .collect()
    }

    fn serialized_fields(value: impl Serialize) -> BTreeSet<String> {
        serde_json::to_value(value)
            .expect("serialize product DTO")
            .as_object()
            .expect("product DTO must serialize as an object")
            .keys()
            .cloned()
            .collect()
    }

    fn typescript_string_union_values(name: &str) -> BTreeSet<String> {
        let typescript = include_str!("../../frontend/src/tauri.ts");
        let marker = format!("export type {name} =");
        typescript
            .split(&marker)
            .nth(1)
            .and_then(|tail| tail.split(';').next())
            .unwrap_or_else(|| panic!("TypeScript union {name} is missing"))
            .split('|')
            .map(str::trim)
            .filter_map(|value| {
                value
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
            })
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn agent_state_product_projection_is_an_explicit_hostile_nested_allowlist() {
        use openlife_core::agent::main_chat_runtime_contract as state;

        const HOSTILE: &str = "hmac-sha256:D010_PRIVATE_NESTED_AGENT_STATE_AUTHORITY_MUST_NOT_SHIP";
        const CODE_SHAPED_HOSTILE: &str = "d010_private_code_shaped_marker";
        let now = chrono::Utc::now();
        let action = state::ActionEvidence {
            action_id: HOSTILE.into(),
            action_type: HOSTILE.into(),
            target: HOSTILE.into(),
            label: HOSTILE.into(),
            status: CODE_SHAPED_HOSTILE.into(),
            risk_level: CODE_SHAPED_HOSTILE.into(),
            policy_decision_id: HOSTILE.into(),
            started_at: Some(now),
            finished_at: Some(now),
            observation_ids: vec![HOSTILE.into()],
            retryable: true,
        };
        let observation = state::ObservationEvidence {
            observation_id: HOSTILE.into(),
            action_id: HOSTILE.into(),
            source_kind: HOSTILE.into(),
            source_label: HOSTILE.into(),
            preview: HOSTILE.into(),
            citation_available: true,
            read_execution: Some(state::ReadExecutionEvidence {
                kind: HOSTILE.into(),
                source_kind: HOSTILE.into(),
                source_label: HOSTILE.into(),
                target: HOSTILE.into(),
                real_read_only_execution: true,
                fixture_backed: false,
                network_read_attempted: true,
                direct_writes_executed: false,
            }),
            created_at: now,
        };
        let blocker = state::BlockerEvidence {
            blocker_id: HOSTILE.into(),
            reason_code: HOSTILE.into(),
            title: HOSTILE.into(),
            detail: HOSTILE.into(),
            affected_action_id: Some(HOSTILE.into()),
            recoverable: true,
            controls: vec![state::MainChatAgentProductControl::OpenTrace],
        };
        let proposal = state::ProposalEvidence {
            proposal_id: HOSTILE.into(),
            proposal_type: HOSTILE.into(),
            status: state::MainChatAgentProductProposalStatus::PendingReview,
            title: HOSTILE.into(),
            summary: HOSTILE.into(),
            evidence_ids: vec![HOSTILE.into()],
            action_ids: vec![HOSTILE.into()],
            controls: vec![state::MainChatAgentProductControl::OpenReviewCenter],
            memory_lifecycle: Some(
                openlife_core::agent::memory_lifecycle::MemoryLifecycleRecord {
                    memory_id: HOSTILE.into(),
                    proposal_id: HOSTILE.into(),
                    source_task_session_id: Some(HOSTILE.into()),
                    source_run_id: Some(HOSTILE.into()),
                    content: HOSTILE.into(),
                    scope: openlife_core::agent::memory_lifecycle::MemoryLifecycleScope::Global,
                    category:
                        openlife_core::agent::memory_lifecycle::MemoryLifecycleCategory::Fact,
                    risk_level:
                        openlife_core::agent::memory_lifecycle::MemoryLifecycleRiskLevel::High,
                    sensitivity:
                        openlife_core::agent::memory_lifecycle::MemoryLifecycleSensitivity::Sensitive,
                    audit_digest: HOSTILE.into(),
                    status:
                        openlife_core::agent::memory_lifecycle::MemoryLifecycleStatus::PendingReview,
                    materialization_status:
                        openlife_core::agent::memory_lifecycle::MemoryMaterializationStatus::Pending,
                    materialization_error_code: Some(HOSTILE.into()),
                    created_by: HOSTILE.into(),
                    accepted_by: Some(HOSTILE.into()),
                    accepted_at: Some(now),
                    materialized_view_id: Some(HOSTILE.into()),
                    materialized_view_version: Some(1),
                    evidence_ids: vec![HOSTILE.into()],
                    confidence: 0.5,
                    conflict_ids: vec![HOSTILE.into()],
                    supersedes_memory_id: Some(HOSTILE.into()),
                    replacement_memory_id: Some(HOSTILE.into()),
                    rolled_back_by_event_id: Some(HOSTILE.into()),
                    runtime_context_excluded_at: Some(now),
                },
            ),
        };
        let snapshot = state::MainChatAgentStateSnapshot {
            task: state::TaskSessionEvidence {
                task_id: HOSTILE.into(),
                run_id: HOSTILE.into(),
                conversation_id: HOSTILE.into(),
                user_message_id: HOSTILE.into(),
                title: HOSTILE.into(),
                strategy: state::MainChatAgentProductStrategyRoute::ReactToolExecution,
                status: state::MainChatAgentProductTaskStatus::Executing,
                created_at: now,
                updated_at: now,
                trace_available: true,
                controls: vec![state::MainChatAgentProductControl::Cancel],
                action_ids: vec![HOSTILE.into()],
                observation_ids: vec![HOSTILE.into()],
                blocker_ids: vec![HOSTILE.into()],
                proposal_ids: vec![HOSTILE.into()],
                final_delivery_id: Some(HOSTILE.into()),
            },
            route: state::StrategyEvidence {
                strategy: state::MainChatAgentProductStrategyRoute::ReactToolExecution,
                reason: HOSTILE.into(),
                confidence: Some(0.7),
            },
            context: vec![state::ContextEvidence {
                context_id: HOSTILE.into(),
                source_kind: HOSTILE.into(),
                source_label: HOSTILE.into(),
                evidence_id: HOSTILE.into(),
            }],
            provider: Some(state::ProviderRouteEvidence {
                provider: HOSTILE.into(),
                model: HOSTILE.into(),
                route_type: CODE_SHAPED_HOSTILE.into(),
                provider_config_generation: Some(HOSTILE.into()),
                reason: HOSTILE.into(),
                evidence_id: HOSTILE.into(),
            }),
            plan: Some(state::PlanEvidence {
                plan_id: HOSTILE.into(),
                plan_session_id: Some(HOSTILE.into()),
                task_session_id: Some(HOSTILE.into()),
                run_id: Some(HOSTILE.into()),
                status: CODE_SHAPED_HOSTILE.into(),
                summary: HOSTILE.into(),
                editable: true,
                source: CODE_SHAPED_HOSTILE.into(),
                evidence_id: HOSTILE.into(),
                revision: Some(4),
                revision_id: Some(HOSTILE.into()),
                confirmed_at: Some(HOSTILE.into()),
                review_id: Some(HOSTILE.into()),
                source_evidence_ids: vec![HOSTILE.into()],
                superseded_by_plan_id: Some(HOSTILE.into()),
                controls: vec!["open_trace".into(), HOSTILE.into()],
                steps: vec![state::PlanStepEvidence {
                    step_id: HOSTILE.into(),
                    plan_id: HOSTILE.into(),
                    index: 0,
                    title: HOSTILE.into(),
                    description: HOSTILE.into(),
                    kind: CODE_SHAPED_HOSTILE.into(),
                    status: CODE_SHAPED_HOSTILE.into(),
                    revision: 4,
                    base_plan_revision: 3,
                    linked_action_ids: vec![HOSTILE.into()],
                    linked_observation_ids: vec![HOSTILE.into()],
                    linked_proposal_ids: vec![HOSTILE.into()],
                    blocker_ids: vec![HOSTILE.into()],
                    linked_final_delivery_ids: vec![HOSTILE.into()],
                    skip_reason: Some(HOSTILE.into()),
                    policy_decision_id: Some(HOSTILE.into()),
                    reason: Some(HOSTILE.into()),
                    evidence_ids: vec![HOSTILE.into()],
                    controls: vec!["open_trace".into(), HOSTILE.into()],
                }],
                review_summary: None,
                artifact_view: None,
            }),
            actions: vec![action],
            observations: vec![observation],
            blockers: vec![blocker],
            proposals: vec![proposal],
            final_delivery: Some(state::FinalDeliveryEvidence {
                delivery_id: HOSTILE.into(),
                task_id: HOSTILE.into(),
                run_id: HOSTILE.into(),
                status: state::MainChatAgentProductDeliveryStatus::CompletedWithPendingItems,
                headline: HOSTILE.into(),
                answer: HOSTILE.into(),
                completed_actions: vec![state::CompletedActionSummary {
                    action_id: HOSTILE.into(),
                    action_type: HOSTILE.into(),
                    target: HOSTILE.into(),
                    status: CODE_SHAPED_HOSTILE.into(),
                    observation_ids: vec![HOSTILE.into()],
                }],
                observations_used: vec![state::ObservationSummary {
                    observation_id: HOSTILE.into(),
                    source_kind: HOSTILE.into(),
                    source_label: HOSTILE.into(),
                    preview: HOSTILE.into(),
                }],
                proposals_created: vec![state::ProposalSummary {
                    proposal_id: HOSTILE.into(),
                    proposal_type: HOSTILE.into(),
                    status: state::MainChatAgentProductProposalStatus::PendingReview,
                    summary: HOSTILE.into(),
                }],
                blockers: vec![state::BlockerSummary {
                    blocker_id: HOSTILE.into(),
                    reason_code: HOSTILE.into(),
                    affected_action_id: Some(HOSTILE.into()),
                    recoverable: true,
                }],
                skipped_work: vec![state::SkippedWorkSummary {
                    step_id: HOSTILE.into(),
                    title: HOSTILE.into(),
                    reason: HOSTILE.into(),
                    status: CODE_SHAPED_HOSTILE.into(),
                }],
                pending_user_actions: vec![state::PendingUserActionSummary {
                    pending_id: HOSTILE.into(),
                    kind: HOSTILE.into(),
                    controls: vec![state::MainChatAgentProductControl::OpenReviewCenter],
                }],
                durable_changes: vec![state::DurableChangeSummary {
                    change_type: HOSTILE.into(),
                    target: HOSTILE.into(),
                    provenance_id: HOSTILE.into(),
                    rollback_available: true,
                }],
                next_steps: vec![HOSTILE.into()],
                trace_available: true,
            }),
            diagnostics: vec![state::EvidenceGap {
                gap_id: HOSTILE.into(),
                gap_code: CODE_SHAPED_HOSTILE.into(),
                detail: HOSTILE.into(),
                evidence_id: Some(HOSTILE.into()),
            }],
            sequence: 9,
            emitted_at: now,
            events: vec![state::MainChatAgentStateEvent {
                event_type: state::MainChatAgentStateEventType::DiagnosticCreated,
                sequence: 9,
                object_id: HOSTILE.into(),
                evidence_id: HOSTILE.into(),
            }],
        };

        let product = serde_json::to_value(ProductMainChatAgentStateSnapshot::from(snapshot))
            .expect("serialize explicit product AgentState projection");
        let encoded = product.to_string();
        assert!(
            !encoded.contains(HOSTILE),
            "hostile authority leaked: {encoded}"
        );
        assert!(
            !encoded.contains(CODE_SHAPED_HOSTILE),
            "code-shaped hostile body leaked: {encoded}"
        );
        assert!(!encoded.contains("hmac-sha256:"), "HMAC authority leaked");
        assert_eq!(product["plan"]["status"], "unknown");
        assert_eq!(product["plan"]["source"], "unknown");
        assert_eq!(product["actions"][0]["status"], "unknown");
        assert_eq!(product["actions"][0]["riskLevel"], "unknown");
        assert_eq!(product["diagnostics"][0]["gapCode"], "unknown_diagnostic");
        assert_eq!(
            product["observations"][0]["preview"],
            "observation_recorded"
        );
        for forbidden in [
            "confirmedAt",
            "reviewSummary",
            "artifactView",
            "memoryLifecycle",
        ] {
            assert!(
                !encoded.contains(forbidden),
                "fail-open field escaped: {forbidden}"
            );
        }
    }

    #[test]
    fn typescript_agent_run_nested_contracts_match_exact_product_serde_keys() {
        let now = chrono::Utc::now();
        let receipt = ProductContentReceipt {
            version: 2,
            kind: openlife_core::agent::ContentReceiptKind::ToolOutput,
            provenance: openlife_core::agent::ContentReceiptProvenance::ObservedToolAdapterBody,
            byte_count: 1,
            digest: "sha256:product-contract".into(),
            verified: false,
        };
        let react_trace = ProductReactActionTrace {
            action_id: "action".into(),
            step_index: 1,
            tool_call_index: 1,
            action_type: "builtin_tool".into(),
            tool_name: "tool".into(),
            tool_source: "builtin".into(),
            action_category: "read".into(),
            risk_level: "low".into(),
            permission_decision: Some("allow".into()),
            status: "succeeded".into(),
            proposal_id: Some("proposal".into()),
            observation_id: Some("observation".into()),
            output_preview: Some("bounded_preview".into()),
            output_receipt: Some(receipt),
            started_at: Some(now),
            finished_at: Some(now),
            metadata_safe: true,
        };
        let tool_scope = ProductToolActionScope {
            tool_name: "tool".into(),
            source: "builtin".into(),
            risk_level: "low".into(),
            capabilities: vec!["read".into()],
        };
        let action = ProductAgentAction {
            id: "action".into(),
            action_type: "builtin_tool".into(),
            target: Some("tool".into()),
            status: "succeeded".into(),
            permission_decision: Some("allow".into()),
            started_at: Some(now),
            finished_at: Some(now),
            error: Some("action_failed".into()),
            timestamp: now,
            tool_scope: Some(tool_scope),
            react_trace: Some(react_trace),
        };
        let context = ProductContextSummary {
            life_model_empty: false,
            memory_hit_count: 1,
            used_tools_prompt: true,
            redaction_applied: true,
            redaction_level: openlife_core::agent::RedactionLevel::Strict,
        };
        let route = ProductModelRouteTrace {
            provider: "provider".into(),
            model: "model".into(),
            route_type: "cloud".into(),
            reason: Some("policy_allowed".into()),
            privacy_level: openlife_core::agent::RedactionLevel::Strict,
            retry_count: 0,
            fallback_reason: Some("none".into()),
            provider_health_is_estimated: Some(false),
        };
        let run_error = ProductAgentRunError {
            message: "run_failed",
            phase: "generation".into(),
            recoverable: true,
        };
        let hs_selection_audit = ProductHSSelectionAudit {
            selected_policy_ids: vec!["policy".into()],
            selected_heuristic_ids: vec!["heuristic".into()],
            estimated_tokens: 1,
            token_budget: 2,
        };
        let behavior_check = ProductHSBehaviorCheckSummary {
            id: "check".into(),
            label: "Behavior check".into(),
            passed: true,
            summary: Some("passed".into()),
        };
        let status_update = ProductAgentStatusUpdate {
            phase: "generation".into(),
            message: "generation".into(),
            step_index: 1,
            tool_call_index: Some(1),
            timestamp: now,
        };

        for (typescript_name, rust_fields) in [
            (
                "ProductReactActionTrace",
                serialized_fields(action.react_trace.as_ref().unwrap()),
            ),
            (
                "ProductToolActionScope",
                serialized_fields(action.tool_scope.as_ref().unwrap()),
            ),
            ("ProductAgentAction", serialized_fields(&action)),
            ("ProductContextSummary", serialized_fields(&context)),
            ("ProductModelRouteTrace", serialized_fields(&route)),
            ("ProductAgentRunError", serialized_fields(&run_error)),
            (
                "ProductHSSelectionAudit",
                serialized_fields(&hs_selection_audit),
            ),
            (
                "ProductHSBehaviorCheckSummary",
                serialized_fields(&behavior_check),
            ),
            (
                "ProductAgentStatusUpdate",
                serialized_fields(&status_update),
            ),
        ] {
            assert_eq!(
                typescript_interface_fields(typescript_name),
                rust_fields,
                "TypeScript {typescript_name} drifted from the shipped Rust product DTO"
            );
        }

        let product_run = ProductAgentRun {
            id: "run".into(),
            task_id: "task".into(),
            session_id: Some("session".into()),
            status: openlife_core::agent::AgentRunStatus::Completed,
            kind: openlife_core::agent::AgentTaskKind::Conversation,
            context_summary: Some(context),
            model_route: Some(route),
            output_preview: Some("bounded_preview".into()),
            error: Some(run_error),
            generated_proposals: vec!["proposal".into()],
            actions: vec![action],
            observations: Vec::new(),
            reasoning_strategy: Some("react".into()),
            legacy_payload_unverified: false,
            hs_selection_audit: Some(hs_selection_audit),
            behavior_checks: vec![behavior_check],
            warnings: vec!["warning".into()],
            status_updates: vec![status_update],
            step_count: 1,
            tool_call_count: 1,
            deleted_at: Some(now),
            delete_reason: Some("deleted".into()),
            started_at: now,
            finished_at: Some(now),
        };
        assert_eq!(
            typescript_interface_fields("ProductAgentRun"),
            serialized_fields(product_run),
            "TypeScript ProductAgentRun drifted from the shipped Rust product DTO"
        );
    }

    #[test]
    fn typescript_task_evidence_contracts_match_exact_product_serde_keys() {
        use crate::main_chat_task_controls::{
            DurableTurnLifecycleReceiptView, ProductFallbackEvidence, ProductProviderReadiness,
            ProductRouteEvidence, ProductRouteIdentity, ProductRouteSourceRef,
            ProductRunEvidenceView, RunEvidenceTimelineEvent,
        };

        let now = chrono::Utc::now();
        let route_identity = ProductRouteIdentity {
            provider: "local".into(),
            model_ref:
                "model:sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .into(),
            route_type: "local".into(),
            privacy_level: "strict".into(),
            reason_ref:
                "reason:sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .into(),
            provider_health_is_estimated: false,
        };
        let readiness = ProductProviderReadiness {
            configured: true,
            credential_present: true,
            validated: true,
            validation_status: "validated".into(),
            preferred: "local".into(),
            actually_used: Some("local".into()),
            stale: false,
            failed: false,
            last_checked_at: Some(now.to_rfc3339()),
        };
        let fallback = ProductFallbackEvidence {
            from_route: Some(route_identity.clone()),
            to_route: Some(route_identity.clone()),
            reason_ref:
                "reason:sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .into(),
            blocker_codes: vec![],
        };
        let source_ref = ProductRouteSourceRef {
            source: "provider_adapter".into(),
            ref_id: Some("mainchat_event:v2:sha256:0000000000000000000000000000000000000000000000000000000000000000".into()),
            status: Some("completed".into()),
            route_type: Some("local".into()),
        };
        let route_evidence = ProductRouteEvidence {
            evidence_id: "mainchat_route_evidence_00000000".into(),
            generated_at: now.to_rfc3339(),
            conversation_id: Some(uuid::Uuid::nil().to_string()),
            run_id: Some(uuid::Uuid::nil().to_string()),
            task_session_id: Some(uuid::Uuid::nil().to_string()),
            answer_scope: "current_turn".into(),
            planned_route: Some(route_identity.clone()),
            actual_route: Some(route_identity.clone()),
            last_completed_route: Some(route_identity.clone()),
            provider_readiness: readiness.clone(),
            fallback: Some(fallback.clone()),
            external_transmission: "not_sent".into(),
            source_refs: vec![source_ref.clone()],
            truth_confidence: "verified".into(),
        };
        let receipt = DurableTurnLifecycleReceiptView {
            event_id: "mainchat_event:v2:sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
            run_id: uuid::Uuid::nil().to_string(),
            sequence: 1,
            event_type: "failed".into(),
            source_ref: "provider_adapter".into(),
            lifecycle_state: "failed".into(),
            failure_kind: Some("provider_error".into()),
            created_at: now,
            payload_digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        };
        let timeline_event = RunEvidenceTimelineEvent {
            id: receipt.event_id.clone(),
            kind: receipt.event_type.clone(),
            summary: "durable_lifecycle_state_recorded".into(),
            created_at: Some(now),
            failure_kind: receipt.failure_kind.clone(),
            normalized_lifecycle_state: Some(receipt.lifecycle_state.clone()),
            source_ref: Some(receipt.source_ref.clone()),
        };
        let evidence_view = ProductRunEvidenceView {
            run_id: Some(receipt.run_id.clone()),
            task_session_id: uuid::Uuid::nil().to_string(),
            title: "main_chat_task".into(),
            lifecycle_state: "failed".into(),
            projection_state: "consistent".into(),
            identity_state: "verified".into(),
            snapshot_state: "available".into(),
            durable_sequence_before: Some(0),
            durable_sequence_after: Some(1),
            durable_lifecycle_receipt: Some(receipt.clone()),
            route_evidence: Some(route_evidence.clone()),
            event_timeline: vec![timeline_event.clone()],
            action_count: 0,
            observation_count: 1,
            blockers: vec![],
            proposals: vec![],
            plan_refs: vec![],
            allowed_controls: vec!["open_trace".into()],
            next_recommended_control: "open_trace".into(),
            redaction_state: "metadata_only".into(),
        };

        for (typescript_name, rust_fields) in [
            ("ProductRouteIdentity", serialized_fields(&route_identity)),
            ("ProductProviderReadiness", serialized_fields(&readiness)),
            ("ProductFallbackEvidence", serialized_fields(&fallback)),
            ("ProductRouteSourceRef", serialized_fields(&source_ref)),
            ("ProductRouteEvidence", serialized_fields(&route_evidence)),
            (
                "DurableTurnLifecycleReceiptView",
                serialized_fields(&receipt),
            ),
            (
                "RunEvidenceTimelineEvent",
                serialized_fields(&timeline_event),
            ),
            ("ProductRunEvidenceView", serialized_fields(&evidence_view)),
        ] {
            assert_eq!(
                typescript_interface_fields(typescript_name),
                rust_fields,
                "TypeScript {typescript_name} drifted from the shipped Rust product DTO"
            );
        }
    }

    #[test]
    fn typescript_terminal_delivery_contract_matches_body_free_product_projection() {
        let canonical = crate::main_chat_turn_runtime::CanonicalFinalDeliveryView {
            delivery_id: "delivery:D010_FINAL_SECRET".into(),
            task_id: uuid::Uuid::new_v4().to_string(),
            run_id: uuid::Uuid::new_v4().to_string(),
            status: "completed".into(),
            headline: "D010_FINAL_SECRET".into(),
            answer: "D010_FINAL_SECRET".into(),
            completed_actions: Vec::new(),
            observations_used: Vec::new(),
            proposals_created: Vec::new(),
            blockers: Vec::new(),
            pending_user_actions: Vec::new(),
            durable_changes: Vec::new(),
            next_steps: vec!["D010_FINAL_SECRET".into()],
            trace_available: true,
            kernel_event_count: Some(1),
            durable_event_count: 1,
            reply_preview: "D010_FINAL_SECRET".into(),
            has_assistant_message: true,
            tool_call_count: 0,
            blocker_count: 0,
            proposal_count: 0,
        };
        let product = ProductFinalDeliveryView::from(&canonical);
        assert_eq!(
            typescript_interface_fields("ProductFinalDeliveryView"),
            serialized_fields(&product),
            "TypeScript ProductFinalDeliveryView drifted from shipped Rust serde"
        );
        let encoded = serde_json::to_string(&product).unwrap();
        assert!(!encoded.contains("D010_FINAL_SECRET"));
        for forbidden in [
            "headline",
            "answer",
            "completedActions",
            "observationsUsed",
            "proposalsCreated",
            "pendingUserActions",
            "durableChanges",
            "nextSteps",
            "replyPreview",
        ] {
            assert!(
                !encoded.contains(forbidden),
                "leaked {forbidden}: {encoded}"
            );
        }

        let terminal = crate::main_chat_turn_runtime::OpenLifeTurnTerminal {
            runtime_owner: crate::main_chat_turn_runtime::OPENLIFE_TURN_RUNTIME_OWNER.into(),
            status: "completed".into(),
            state: "DirectAnswer".into(),
            final_delivery: canonical,
            run_id: Some(uuid::Uuid::new_v4().to_string()),
            task_session_id: Some(uuid::Uuid::new_v4().to_string()),
            blockers: vec!["D010_TERMINAL_SECRET".into()],
            proposals: vec!["D010_TERMINAL_SECRET".into()],
            legacy_fallback_used: false,
            legacy_runtime_invoked: false,
            single_step_fallback_used: false,
            direct_writes_executed: false,
            provider_invocation_status:
                crate::main_chat_turn_runtime::ProviderInvocationState::Completed,
            model_invoked: true,
            tool_invoked: false,
        };
        let product_terminal = ProductOpenLifeTurnTerminal::from(&terminal);
        assert_eq!(
            typescript_interface_fields("OpenLifeTurnTerminal"),
            serialized_fields(&product_terminal),
            "TypeScript OpenLifeTurnTerminal drifted from shipped Rust serde"
        );
        let terminal_encoded = serde_json::to_string(&terminal).unwrap();
        assert!(!terminal_encoded.contains("D010_FINAL_SECRET"));
        assert!(!terminal_encoded.contains("D010_TERMINAL_SECRET"));
    }

    #[test]
    fn product_tool_call_contract_excludes_raw_tool_payloads_and_runtime_authority() {
        const ARGUMENT_SECRET: &str = "D010_ARGUMENT_SECRET_7C3E";
        const OUTPUT_SECRET: &str = "D010_OUTPUT_SECRET_91AF";
        const ERROR_SECRET: &str = "D010_ERROR_SECRET_5B42";
        const WARNING_SECRET: &str = "D010_WARNING_SECRET_C826";
        let tool_call = crate::ToolCallResult {
            name: "tool".into(),
            arguments: serde_json::json!({"secret": ARGUMENT_SECRET}),
            sanitized_arguments: Some(serde_json::json!({"secret": ARGUMENT_SECRET})),
            success: false,
            output: Some(OUTPUT_SECRET.into()),
            error: Some(ERROR_SECRET.into()),
            permission_level: "low".into(),
            status: crate::ToolCallStatus::Error,
            requires_confirmation: false,
            pii_found: false,
            privacy_warnings: vec![WARNING_SECRET.into()],
            action_id: Some("action".into()),
            run_id: Some("run".into()),
            permission_decision: Some("allow".into()),
            react_trace: None,
            execution_receipt: None,
            product_projection: None,
        };

        let serialized = serde_json::to_value(&tool_call).expect("product tool call serializes");
        let body = serde_json::to_string(&serialized).expect("product tool call JSON");
        for forbidden_key in [
            "name",
            "arguments",
            "sanitized_arguments",
            "success",
            "output",
            "error",
            "permission_level",
            "pii_found",
            "privacy_warnings",
            "action_id",
            "run_id",
            "permission_decision",
            "execution_receipt",
        ] {
            assert!(
                serialized.get(forbidden_key).is_none(),
                "raw/internal ToolCallResult key escaped to product IPC: {forbidden_key}"
            );
        }
        for secret in [ARGUMENT_SECRET, OUTPUT_SECRET, ERROR_SECRET, WARNING_SECRET] {
            assert!(
                !body.contains(secret),
                "raw tool payload escaped to product IPC: {secret}"
            );
        }

        assert_eq!(serialized["toolRef"]["id"], "unknown_tool");
        assert_eq!(serialized["actionRef"], "unknown_action");
        assert_eq!(serialized["failureCode"], "tool_evidence_unverified");
        assert_eq!(serialized["privacyWarningCount"], 0);
    }

    #[test]
    fn typescript_product_tool_call_nested_contracts_match_exact_serde_keys() {
        let tool_ref = ProductToolReference {
            id: "builtin.file_read".into(),
            source: "local".into(),
        };
        let execution_receipt = ProductToolExecutionReceipt {
            receipt_ref: uuid::Uuid::new_v4().to_string(),
            request_digest: format!("sha256:{}", "a".repeat(64)),
            action_effect: openlife_core::tool_execution_receipt::ToolActionEffect::ReadOnly,
            idempotency_contract: openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent,
            dispatch_kind: openlife_core::tool_execution_receipt::ToolDispatchKind::Local,
            dispatch_attempt_count: 1,
            dispatch_observed: true,
            transport_status:
                openlife_core::tool_execution_receipt::ToolTransportStatus::ResponseObserved,
            effect_status: openlife_core::tool_execution_receipt::ToolEffectStatus::NotAttempted,
            outcome: openlife_core::tool_execution_receipt::ToolExecutionOutcome::Succeeded,
            verified: true,
        };
        let output_receipt = ProductContentReceipt {
            version: 2,
            kind: openlife_core::agent::ContentReceiptKind::ToolOutput,
            provenance: openlife_core::agent::ContentReceiptProvenance::ObservedToolAdapterBody,
            byte_count: 16,
            digest: format!("sha256:{}", "b".repeat(64)),
            verified: false,
        };
        let product = ProductToolCallResult {
            tool_ref: tool_ref.clone(),
            action_ref: uuid::Uuid::new_v4().to_string(),
            run_ref: Some(uuid::Uuid::new_v4().to_string()),
            status: ProductToolCallStatus::Success,
            requires_confirmation: false,
            failure_code: Some(ProductToolFailureCode::ToolFailed),
            privacy_warning_count: 1,
            proposal_ref: Some(uuid::Uuid::new_v4().to_string()),
            execution_receipt: Some(execution_receipt.clone()),
            output_receipt: Some(output_receipt),
        };

        for (typescript_name, rust_fields) in [
            ("ProductToolReference", serialized_fields(tool_ref)),
            (
                "ProductToolExecutionReceipt",
                serialized_fields(execution_receipt),
            ),
            ("ProductToolCallResult", serialized_fields(product)),
        ] {
            assert_eq!(
                typescript_interface_fields(typescript_name),
                rust_fields,
                "TypeScript {typescript_name} drifted from the shipped Rust product DTO"
            );
        }

        let rust_failure_codes = [
            ProductToolFailureCode::ToolFailed,
            ProductToolFailureCode::ToolEffectUnknown,
            ProductToolFailureCode::ToolRemoteStateUnknown,
            ProductToolFailureCode::ToolLocallyAborted,
            ProductToolFailureCode::ToolNotDispatched,
            ProductToolFailureCode::ToolStateUnknown,
            ProductToolFailureCode::ToolEvidenceUnverified,
        ]
        .into_iter()
        .map(|code| {
            serde_json::to_value(code)
                .expect("serialize product tool failure code")
                .as_str()
                .expect("product tool failure code is a string")
                .to_string()
        })
        .collect::<BTreeSet<_>>();
        assert_eq!(
            typescript_string_union_values("ProductToolFailureCode"),
            rust_failure_codes,
            "TypeScript ProductToolFailureCode drifted from Rust"
        );
        let rust_statuses = [
            ProductToolCallStatus::Success,
            ProductToolCallStatus::Failed,
            ProductToolCallStatus::EffectUnknown,
            ProductToolCallStatus::NotDispatched,
            ProductToolCallStatus::LocallyAborted,
            ProductToolCallStatus::RemoteUnknown,
            ProductToolCallStatus::Unknown,
        ]
        .into_iter()
        .map(|status| {
            serde_json::to_value(status)
                .expect("serialize product tool status")
                .as_str()
                .expect("product tool status is a string")
                .to_string()
        })
        .collect::<BTreeSet<_>>();
        assert_eq!(
            typescript_string_union_values("ProductToolCallStatus"),
            rust_statuses,
            "TypeScript ProductToolCallStatus drifted from Rust"
        );

        assert!(!serde_json::to_string(&ProductToolReference {
            id: "unknown_tool".into(),
            source: "unknown".into(),
        })
        .unwrap()
        .contains("d010secret123"));
    }

    #[test]
    fn verified_product_tool_projection_requires_live_exact_binding_and_filters_manifest_secret() {
        const CODE_SHAPED_SECRET: &str = "d010secret123";
        let run_id = uuid::Uuid::new_v4().to_string();
        let action_id = uuid::Uuid::new_v4().to_string();
        let input = serde_json::json!({"arguments": {"private": "body-never-ships"}});
        let action = openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".into(),
            target: Some("safe.read".into()),
            input: input.clone(),
            output: Some(serde_json::json!({"text": "adapter-body-never-ships"})),
            status: "succeeded".into(),
            permission_decision: Some("allow".into()),
            started_at: Some(chrono::Utc::now()),
            finished_at: Some(chrono::Utc::now()),
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: None,
        };
        let receipt =
            openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_observed_local_read(
                Some(run_id.clone()),
                Some(CODE_SHAPED_SECRET.into()),
                "private request digest material".into(),
                true,
            )
            .test_bound_to_action(
                &run_id,
                &action_id,
                &action.action_type,
                action.target.as_deref(),
                &input,
            );

        let exact =
            VerifiedProductToolCallProjection::from_bound_action(&action, &receipt, &run_id)
                .expect("the exact live action/receipt pair projects");
        assert_eq!(exact.tool_ref.id, "unknown_tool");
        assert!(exact.receipt.verified);
        assert!(!serde_json::to_string(&exact.receipt)
            .expect("serialize product receipt")
            .contains(CODE_SHAPED_SECRET));

        let mut wrong_action = action.clone();
        wrong_action.id = uuid::Uuid::new_v4().to_string();
        assert!(VerifiedProductToolCallProjection::from_bound_action(
            &wrong_action,
            &receipt,
            &run_id
        )
        .is_none());

        let persisted: openlife_core::tool_execution_receipt::ToolExecutionReceipt =
            serde_json::from_value(serde_json::to_value(&receipt).expect("serialize receipt"))
                .expect("deserialize receipt-shaped evidence");
        assert!(
            VerifiedProductToolCallProjection::from_bound_action(&action, &persisted, &run_id)
                .is_none()
        );
    }

    #[test]
    fn observed_failure_with_unknown_effect_remains_product_effect_unknown() {
        let run_id = uuid::Uuid::new_v4().to_string();
        let action_id = uuid::Uuid::new_v4().to_string();
        let input = serde_json::json!({"arguments": {"target": "external"}});
        let action = openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".into(),
            target: Some("external_mutation".into()),
            input: input.clone(),
            output: None,
            status: "failed".into(),
            permission_decision: Some("allow".into()),
            started_at: Some(chrono::Utc::now()),
            finished_at: Some(chrono::Utc::now()),
            error: Some("adapter reported failure".into()),
            timestamp: chrono::Utc::now(),
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: None,
        };
        let receipt = openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_observed_local_mutation_failure(
            Some(run_id.clone()),
            Some("external_mutation".into()),
            "external mutation request".into(),
        )
        .test_bound_to_action(
            &run_id,
            &action_id,
            &action.action_type,
            action.target.as_deref(),
            &input,
        );

        let projection =
            VerifiedProductToolCallProjection::from_bound_action(&action, &receipt, &run_id)
                .expect("exact live failed mutation projects");
        assert_eq!(projection.status, ProductToolCallStatus::EffectUnknown);
        assert_eq!(
            projection.failure_code,
            Some(ProductToolFailureCode::ToolEffectUnknown)
        );
        let product = projection.product_result();
        let serialized = serde_json::to_value(product).unwrap();
        assert_eq!(serialized["status"], "effect_unknown");
        assert_eq!(serialized["executionReceipt"]["effectStatus"], "unknown");
        assert_eq!(serialized["executionReceipt"]["outcome"], "failed");
    }

    #[test]
    fn observed_success_with_unknown_effect_remains_product_effect_unknown() {
        let run_id = uuid::Uuid::new_v4().to_string();
        let action_id = uuid::Uuid::new_v4().to_string();
        let input = serde_json::json!({"arguments": {"target": "unclassified"}});
        let action = openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".into(),
            target: Some("unclassified_effect".into()),
            input: input.clone(),
            output: Some(serde_json::json!({"result": "adapter-success-is-not-effect-proof"})),
            status: "succeeded".into(),
            permission_decision: Some("allow".into()),
            started_at: Some(chrono::Utc::now()),
            finished_at: Some(chrono::Utc::now()),
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: None,
        };
        let receipt = openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_observed_unknown_effect_succeeded(
            Some(run_id.clone()),
            Some("unclassified_effect".into()),
            "unknown effect request".into(),
        )
        .test_bound_to_action(
            &run_id,
            &action_id,
            &action.action_type,
            action.target.as_deref(),
            &input,
        );

        let projection =
            VerifiedProductToolCallProjection::from_bound_action(&action, &receipt, &run_id)
                .expect("exact live unknown-effect success projects");
        assert_eq!(projection.status, ProductToolCallStatus::EffectUnknown);
        assert_eq!(
            projection.failure_code,
            Some(ProductToolFailureCode::ToolEffectUnknown)
        );
        let serialized = serde_json::to_value(projection.product_result()).unwrap();
        assert_eq!(serialized["status"], "effect_unknown");
        assert_eq!(serialized["executionReceipt"]["effectStatus"], "unknown");
        assert_eq!(serialized["executionReceipt"]["outcome"], "succeeded");
    }

    #[test]
    fn verified_product_tool_projection_is_atomic_and_fails_closed_after_transplant() {
        let run_id = uuid::Uuid::new_v4().to_string();
        let action_id = uuid::Uuid::new_v4().to_string();
        let input = serde_json::json!({"arguments": {"private": "body-never-ships"}});
        let action = openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".into(),
            target: Some("safe.read".into()),
            input: input.clone(),
            output: None,
            status: "succeeded".into(),
            permission_decision: Some("allow".into()),
            started_at: Some(chrono::Utc::now()),
            finished_at: Some(chrono::Utc::now()),
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: None,
        };
        let receipt =
            openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_observed_local_read(
                Some(run_id.clone()),
                Some("d010secret123".into()),
                "private request digest material".into(),
                true,
            )
            .test_bound_to_action(
                &run_id,
                &action_id,
                &action.action_type,
                action.target.as_deref(),
                &input,
            );
        let projection =
            VerifiedProductToolCallProjection::from_bound_action(&action, &receipt, &run_id)
                .expect("exact live projection");
        let exact = crate::ToolCallResult {
            name: "d010secret123".into(),
            arguments: serde_json::json!({"private": "raw-argument"}),
            sanitized_arguments: None,
            success: true,
            output: Some("raw-output".into()),
            error: None,
            permission_level: "low".into(),
            status: crate::ToolCallStatus::Success,
            requires_confirmation: false,
            pii_found: false,
            privacy_warnings: Vec::new(),
            action_id: Some(action_id.clone()),
            run_id: Some(run_id.clone()),
            permission_decision: Some("allow".into()),
            react_trace: None,
            execution_receipt: Some(receipt.clone()),
            product_projection: Some(projection.clone()),
        };
        let exact_json = serde_json::to_value(&exact).expect("serialize exact projection");
        assert_eq!(exact_json["status"], "success");
        assert_eq!(exact_json["executionReceipt"]["verified"], true);
        assert_eq!(exact_json["toolRef"]["id"], "unknown_tool");
        assert!(!exact_json.to_string().contains("d010secret123"));

        let mut mutations = Vec::new();
        let mut wrong_identity = exact.clone();
        wrong_identity.action_id = Some(uuid::Uuid::new_v4().to_string());
        wrong_identity.run_id = Some(uuid::Uuid::new_v4().to_string());
        mutations.push(wrong_identity);
        let mut missing_receipt = exact.clone();
        missing_receipt.execution_receipt = None;
        mutations.push(missing_receipt);
        let mut replaced_receipt = exact.clone();
        replaced_receipt.execution_receipt = Some(
            openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_gateway_failed_before_dispatch(
                Some(run_id.clone()),
                Some("another-tool".into()),
                "another request".into(),
                openlife_core::tool_execution_receipt::ToolActionEffect::ReadOnly,
                openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent,
            ),
        );
        mutations.push(replaced_receipt);

        for mutated in mutations {
            let value = serde_json::to_value(mutated).expect("serialize transplanted projection");
            assert_eq!(value["actionRef"], "unknown_action");
            assert_eq!(value["failureCode"], "tool_evidence_unverified");
            assert!(value.get("executionReceipt").is_none());
            assert!(value.get("proposalRef").is_none());
            assert!(value.get("outputReceipt").is_none());
            assert!(!value.to_string().contains("d010secret123"));
        }

        let presentation_only = serde_json::to_value({
            let mut call = exact;
            call.status = crate::ToolCallStatus::Error;
            call.requires_confirmation = true;
            call.privacy_warnings = vec!["d010secret123".into()];
            call
        })
        .expect("serialize presentation-only mutation");
        assert_eq!(presentation_only["status"], "success");
        assert_eq!(presentation_only["requiresConfirmation"], false);
        assert_eq!(presentation_only["privacyWarningCount"], 0);
        assert!(presentation_only.get("proposalRef").is_none());
        assert!(presentation_only.get("outputReceipt").is_none());
    }

    #[test]
    fn transient_react_trace_never_trusts_metadata_safe_or_body_shaped_preview_fields() {
        fn trace(
            metadata_safe: bool,
            tool_name: &str,
            output_preview: &str,
        ) -> ReactActionTraceEnvelope {
            ReactActionTraceEnvelope {
                run_id: Some(uuid::Uuid::nil().to_string()),
                action_id: "mainchat_action_00000000".into(),
                step_index: 1,
                tool_call_index: 1,
                action_type: "builtin_tool".into(),
                tool_id: "builtin.read".into(),
                tool_name: tool_name.into(),
                tool_source: "builtin".into(),
                action_category: "read".into(),
                risk_level: "low".into(),
                permission_decision: Some("allow".into()),
                status: "succeeded".into(),
                proposal_id: None,
                observation_id: None,
                observation_status: Some("observed".into()),
                output_preview: Some(output_preview.into()),
                output_receipt: None,
                output_item_count: Some(1),
                started_at: None,
                finished_at: None,
                metadata_safe,
            }
        }

        let explicitly_unsafe = ProductReactActionTrace::from_transient_trace(trace(
            false,
            "send the private body to the frontend",
            "private tool output body",
        ));
        assert!(!explicitly_unsafe.metadata_safe);
        assert_eq!(explicitly_unsafe.tool_name, "unknown_tool");
        assert_eq!(explicitly_unsafe.output_preview, None);

        let unsafe_typed_metadata = ProductReactActionTrace::from_transient_trace(trace(
            false,
            "builtin.read_file",
            "23 bytes redacted",
        ));
        assert!(!unsafe_typed_metadata.metadata_safe);
        assert_eq!(unsafe_typed_metadata.tool_name, "unknown_tool");
        assert_eq!(unsafe_typed_metadata.output_preview, None);

        let forged_safe = ProductReactActionTrace::from_transient_trace(trace(
            true,
            "forged tool body with spaces",
            "forged metadata_safe body",
        ));
        assert!(forged_safe.metadata_safe);
        assert_eq!(forged_safe.tool_name, "unknown_tool");
        assert_eq!(forged_safe.output_preview, None);

        let code_shaped_secret = ProductReactActionTrace::from_transient_trace(trace(
            true,
            "D010SECRET123",
            "23 bytes redacted",
        ));
        assert_eq!(
            code_shaped_secret.tool_name, "unknown_tool",
            "a code-shaped value is not proof that ToolGateway or a canonical store observed a tool name"
        );

        let typed_metadata = ProductReactActionTrace::from_transient_trace(trace(
            true,
            "builtin.read_file",
            "23 bytes redacted",
        ));
        assert_eq!(typed_metadata.tool_name, "unknown_tool");
        assert_eq!(
            typed_metadata.output_preview.as_deref(),
            Some("23 bytes redacted")
        );

        for invalid_preview in [
            "-1 bytes redacted".to_string(),
            "023 bytes redacted".to_string(),
            format!("{}0 bytes redacted", usize::MAX),
            "23 bytes redacted trailing body".to_string(),
        ] {
            let rejected = ProductReactActionTrace::from_transient_trace(trace(
                true,
                "builtin.read_file",
                &invalid_preview,
            ));
            assert_eq!(
                rejected.output_preview, None,
                "only a canonical nonnegative usize byte-count receipt may cross product IPC"
            );
        }
    }
}
