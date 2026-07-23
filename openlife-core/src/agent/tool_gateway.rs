use crate::agent::action_executor::helpers::{is_proposal_generation_tool, normalize_tool_name};
use crate::agent::action_executor::ActionExecutor;
use crate::agent::main_chat_agent_v1::CanonicalToolReplayAuthority;
use crate::agent::types::{AgentAction, AgentObservation};
use crate::agent::{
    ActionExecutionContext, ActionExecutionResult, ActionExecutionStatus, ActionExecutorConfig,
    AgentActionRequest,
};
use crate::tool_execution_receipt::{
    ToolActionEffect, ToolEffectStatus, ToolExecutionReceipt, ToolExecutionReceiptRegistration,
    ToolTransportStatus,
};
use crate::tool_manifest::{ToolIdempotencyContract, ToolManifest, ToolSource};
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolGatewayContractEvidence {
    pub tool_name: String,
    pub manifest_id: String,
    pub source: String,
    pub permission_level: String,
    pub risk_level: String,
    pub action_type: String,
    pub capabilities: Vec<String>,
    pub evidence_contract: Vec<String>,
    pub action_effect: ToolActionEffect,
    pub idempotency_contract: ToolIdempotencyContract,
}

/// One-use authority whose constructor is private to ToolGateway. Crate
/// siblings may name the type for sealed method signatures, but cannot mint a
/// value, extract a pending adapter admission, or request final signing.
///
/// ```compile_fail
/// use openlife_core::agent::tool_gateway::ToolGatewayFinalizationAuthority;
///
/// let _forged = ToolGatewayFinalizationAuthority { _sealed: () };
/// ```
pub(crate) struct ToolGatewayFinalizationAuthority {
    _sealed: (),
}

impl ToolGatewayFinalizationAuthority {
    fn new() -> Self {
        Self { _sealed: () }
    }
}

/// Exact, current replay binding supplied to ToolGateway for validation. The
/// durable authority itself is not constructible from product serde surfaces.
pub struct ToolAutomaticRetryAuthorizationInput<'a> {
    pub authority: &'a CanonicalToolReplayAuthority,
    pub action_id: &'a str,
    pub task_session_id: &'a str,
    pub run_id: &'a str,
    pub queue_action_type: &'a str,
    pub executor_action_type: &'a str,
    pub requested_target: &'a str,
    pub resolved_target: &'a str,
    pub manifest: &'a ToolManifest,
    pub input: &'a Value,
    pub expected_action_status: &'a str,
    pub expected_action_revision: u64,
}

/// One-use, non-serde authorization that binds ToolGateway's current manifest
/// validation to the exact ActionQueue CAS snapshot. Private fields prevent a
/// command or UI adapter from fabricating replay credit.
///
/// ```compile_fail
/// use openlife_core::agent::tool_gateway::ToolAutomaticRetryProof;
///
/// fn inspect(proof: ToolAutomaticRetryProof) {
///     let ToolAutomaticRetryProof {
///         binding,
///         expected_action_status,
///         expected_action_revision,
///     } = proof;
///     let _ = (binding, expected_action_status, expected_action_revision);
/// }
/// ```
#[derive(Debug)]
pub struct ToolAutomaticRetryProof {
    binding: ToolAutomaticRetryClaimBinding,
    expected_action_status: String,
    expected_action_revision: u64,
}

/// Consumed claim material produced only by [`ToolAutomaticRetryProof`]. It is
/// crate-visible so ActionQueueStore can compare it with the authenticated row
/// inside the same `BEGIN IMMEDIATE` transaction; its fields and constructor
/// remain sealed in this module.
#[derive(Debug)]
pub(crate) struct ToolAutomaticRetryClaimBinding {
    authority_version: u32,
    store_id: String,
    action_id: String,
    task_session_id: String,
    run_id: String,
    queue_action_type: String,
    executor_action_id: String,
    executor_action_type: String,
    requested_target: String,
    resolved_target: String,
    manifest_id: String,
    manifest_name: String,
    manifest_source: String,
    manifest_contract_digest: String,
    input_hash: String,
    input_length_bytes: u64,
    receipt_id: String,
    receipt_request_digest: String,
    action_effect: ToolActionEffect,
    idempotency_contract: ToolIdempotencyContract,
    dispatch_kind: crate::tool_execution_receipt::ToolDispatchKind,
    dispatch_attempt_count: u32,
    transport_status: ToolTransportStatus,
    effect_status: ToolEffectStatus,
    execution_outcome: crate::tool_execution_receipt::ToolExecutionOutcome,
}

impl ToolAutomaticRetryClaimBinding {
    fn from_authenticated_authority(authority: &CanonicalToolReplayAuthority) -> Self {
        Self {
            authority_version: authority.version(),
            store_id: authority.store_id().to_string(),
            action_id: authority.action_id().to_string(),
            task_session_id: authority.task_session_id().to_string(),
            run_id: authority.run_id().to_string(),
            queue_action_type: authority.queue_action_type().to_string(),
            executor_action_id: authority.executor_action_id().to_string(),
            executor_action_type: authority.executor_action_type().to_string(),
            requested_target: authority.requested_target().to_string(),
            resolved_target: authority.resolved_target().to_string(),
            manifest_id: authority.manifest_id().to_string(),
            manifest_name: authority.manifest_name().to_string(),
            manifest_source: authority.manifest_source().to_string(),
            manifest_contract_digest: authority.manifest_contract_digest().to_string(),
            input_hash: authority.input_hash().to_string(),
            input_length_bytes: authority.input_length_bytes(),
            receipt_id: authority.receipt_id().to_string(),
            receipt_request_digest: authority.receipt_request_digest().to_string(),
            action_effect: authority.action_effect(),
            idempotency_contract: authority.idempotency_contract(),
            dispatch_kind: authority.dispatch_kind(),
            dispatch_attempt_count: authority.dispatch_attempt_count(),
            transport_status: authority.transport_status(),
            effect_status: authority.effect_status(),
            execution_outcome: authority.execution_outcome(),
        }
    }

    pub(crate) fn matches_authenticated_authority(
        &self,
        authority: &CanonicalToolReplayAuthority,
    ) -> bool {
        self.authority_version == authority.version()
            && self.store_id == authority.store_id()
            && self.action_id == authority.action_id()
            && self.task_session_id == authority.task_session_id()
            && self.run_id == authority.run_id()
            && self.queue_action_type == authority.queue_action_type()
            && self.executor_action_id == authority.executor_action_id()
            && self.executor_action_type == authority.executor_action_type()
            && self.requested_target == authority.requested_target()
            && self.resolved_target == authority.resolved_target()
            && self.manifest_id == authority.manifest_id()
            && self.manifest_name == authority.manifest_name()
            && self.manifest_source == authority.manifest_source()
            && self.manifest_contract_digest == authority.manifest_contract_digest()
            && self.input_hash == authority.input_hash()
            && self.input_length_bytes == authority.input_length_bytes()
            && self.receipt_id == authority.receipt_id()
            && self.receipt_request_digest == authority.receipt_request_digest()
            && self.action_effect == authority.action_effect()
            && self.idempotency_contract == authority.idempotency_contract()
            && self.dispatch_kind == authority.dispatch_kind()
            && self.dispatch_attempt_count == authority.dispatch_attempt_count()
            && self.transport_status == authority.transport_status()
            && self.effect_status == authority.effect_status()
            && self.execution_outcome == authority.execution_outcome()
    }
}

impl ToolAutomaticRetryProof {
    pub(crate) fn consume_for_queue_claim(
        self,
        store_id: &str,
        action_id: &str,
        expected_action_status: &str,
        expected_action_revision: u64,
    ) -> std::result::Result<ToolAutomaticRetryClaimBinding, String> {
        if self.binding.store_id != store_id {
            return Err("automatic_retry_proof_store_mismatch".into());
        }
        if self.binding.action_id != action_id {
            return Err("automatic_retry_proof_action_mismatch".into());
        }
        if self.expected_action_status != expected_action_status
            || self.expected_action_revision != expected_action_revision
        {
            return Err("automatic_retry_proof_action_snapshot_mismatch".into());
        }
        Ok(self.binding)
    }
}

pub struct ToolGateway {
    executor: ActionExecutor,
    receipt_registration_sink: Option<Arc<dyn Fn(ToolExecutionReceiptRegistration) + Send + Sync>>,
}

impl ToolGateway {
    fn new(executor: ActionExecutor) -> Self {
        Self {
            executor,
            receipt_registration_sink: None,
        }
    }

    pub fn from_executor_config(config: ActionExecutorConfig) -> Self {
        Self::new(ActionExecutor::new(config))
    }

    pub fn mint_automatic_retry_proof(
        input: ToolAutomaticRetryAuthorizationInput<'_>,
    ) -> std::result::Result<ToolAutomaticRetryProof, String> {
        let ToolAutomaticRetryAuthorizationInput {
            authority,
            action_id,
            task_session_id,
            run_id,
            queue_action_type,
            executor_action_type,
            requested_target,
            resolved_target,
            manifest,
            input,
            expected_action_status,
            expected_action_revision,
        } = input;
        let contract = validate_manifest_execution_contract(manifest)?;
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_value_digest(input);
        if !authority.automatic_retry_terminal_is_safe() {
            return Err("tool_gateway_retry_authority_terminal_not_safe".into());
        }
        if authority.action_id() != action_id
            || authority.task_session_id() != task_session_id
            || authority.run_id() != run_id
            || authority.queue_action_type() != queue_action_type
            || authority.executor_action_type() != executor_action_type
            || authority.requested_target() != requested_target
            || authority.resolved_target() != resolved_target
            || authority.manifest_id() != manifest.id
            || authority.manifest_name() != manifest.name
            || authority.manifest_source() != manifest.source.to_string()
            || authority.manifest_contract_digest() != manifest.execution_contract_digest()
            || authority.input_hash() != input_hash
            || authority.input_length_bytes() != input_length_bytes as u64
        {
            return Err("tool_gateway_retry_authority_execution_binding_mismatch".into());
        }
        if contract.manifest_id != authority.manifest_id()
            || contract.tool_name != authority.manifest_name()
            || contract.action_effect != authority.action_effect()
            || contract.idempotency_contract != authority.idempotency_contract()
            || contract.idempotency_contract != ToolIdempotencyContract::Idempotent
        {
            return Err("tool_gateway_retry_current_manifest_contract_mismatch".into());
        }

        Ok(ToolAutomaticRetryProof {
            binding: ToolAutomaticRetryClaimBinding::from_authenticated_authority(authority),
            expected_action_status: expected_action_status.to_string(),
            expected_action_revision,
        })
    }

    /// Registers an execution-owner observer that outlives an individual tool
    /// future. Product runtimes use this to retain the gateway-owned typed
    /// receipt when local cancellation drops the future after dispatch.
    pub fn with_receipt_registration_sink<F>(mut self, sink: F) -> Self
    where
        F: Fn(ToolExecutionReceiptRegistration) + Send + Sync + 'static,
    {
        self.receipt_registration_sink = Some(Arc::new(sink));
        self
    }

    pub async fn execute(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
    ) -> Result<ActionExecutionResult> {
        self.execute_with_receipt_registration_sink(request, ctx, |_| {})
            .await
    }

    /// Exposes the gateway-owned receipt tracker before transport execution.
    /// The observer receives only the minimal typed receipt state, never tool
    /// arguments or output. A cancellation coordinator can therefore classify
    /// a dropped future as not-attempted, dispatched/unknown, or response-
    /// observed without guessing from which select branch happened to win.
    pub async fn execute_with_receipt_registration_sink<F>(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        receipt_observer: F,
    ) -> Result<ActionExecutionResult>
    where
        F: FnOnce(ToolExecutionReceiptRegistration),
    {
        let contract = match validate_gateway_request_contract(&request, ctx) {
            Ok(contract) => contract,
            Err(reason) => {
                let tracker = crate::agent::action_executor::receipt_tracker_for_request(
                    &request,
                    None,
                    ToolActionEffect::Unknown,
                    ToolIdempotencyContract::Unspecified,
                );
                let registration = ToolExecutionReceiptRegistration::new(tracker.clone());
                if let Some(sink) = &self.receipt_registration_sink {
                    sink(registration.clone());
                }
                receipt_observer(registration);
                tracker.mark_execution_failed();
                tracker.finish();
                let mut result = blocked_gateway_result(request, &reason, tracker.snapshot());
                bind_receipt_to_gateway_action(&tracker, &mut result)?;
                attach_execution_receipt_evidence(&mut result);
                return Ok(result);
            }
        };

        let receipt_tracker = crate::agent::action_executor::receipt_tracker_for_request(
            &request,
            Some(contract.manifest_id.clone()),
            contract.action_effect,
            contract.idempotency_contract,
        );
        let registration = ToolExecutionReceiptRegistration::new(receipt_tracker.clone());
        if let Some(sink) = &self.receipt_registration_sink {
            sink(registration.clone());
        }
        receipt_observer(registration);
        let durable_owner = ctx
            .a2a_outbound_authorization
            .and_then(|authorization| authorization.durable_tool_execution_owner());
        if let Some(owner) = durable_owner {
            owner.prepare(&receipt_tracker.snapshot())?;
        }

        let finalization_authority = ToolGatewayFinalizationAuthority::new();
        let mut pending = match tokio::time::timeout(
            self.executor.timeout(),
            self.executor.execute_for_tool_gateway(
                request.clone(),
                ctx,
                receipt_tracker.clone(),
                &contract,
                &finalization_authority,
            ),
        )
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                // The executor returned an observed failure; it was not
                // necessarily locally aborted. Preserve a definitely-before-
                // dispatch failure as NotAttempted, and classify only a real
                // dispatched mutation as effect-unknown.
                receipt_tracker.settle_failed_terminal();
                crate::agent::action_executor::PendingToolGatewayActionExecution::from_terminal_without_admission(
                    failed_gateway_result(
                        request.clone(),
                        &format!("tool_gateway_executor_failed:{error}"),
                        receipt_tracker.snapshot(),
                    ),
                    &finalization_authority,
                )?
            }
            Err(_) => {
                if receipt_tracker.snapshot().transport_status != ToolTransportStatus::NotAttempted
                {
                    receipt_tracker.mark_local_aborted();
                }
                receipt_tracker.finish();
                crate::agent::action_executor::PendingToolGatewayActionExecution::from_terminal_without_admission(
                    failed_gateway_result(
                        request.clone(),
                        "tool_gateway_timeout",
                        receipt_tracker.snapshot(),
                    ),
                    &finalization_authority,
                )?
            }
        };
        if receipt_tracker.snapshot().transport_status == ToolTransportStatus::Dispatched {
            receipt_tracker.settle_failed_terminal();
        }
        // A timeout or dropped future can occur after a manifest-backed tool
        // became audit-eligible but before the synchronous insert returned.
        // Preserve that as unknown instead of exposing a pending terminal or
        // claiming that the audit row committed.
        receipt_tracker.mark_audit_persistence_unknown_if_pending();
        let receipt = receipt_tracker.snapshot();
        if let Err(reason) = receipt.mechanically_valid_terminal() {
            pending.replace_with_terminal(
                failed_gateway_result(
                    request.clone(),
                    &format!("tool_gateway_invalid_terminal_receipt:{reason}"),
                    receipt.clone(),
                ),
                &finalization_authority,
            )?;
        } else if pending.status() == &ActionExecutionStatus::Succeeded && !receipt.proves_success()
        {
            pending.replace_with_terminal(
                failed_gateway_result(
                    request,
                    "tool_gateway_success_without_mechanical_receipt_proof",
                    receipt.clone(),
                ),
                &finalization_authority,
            )?;
        }
        let result = pending.finalize_gateway_semantics(
            ctx,
            &receipt_tracker,
            gateway_contract_evidence(&contract),
            finalization_authority,
        )?;
        if let Some(owner) = durable_owner {
            owner.terminal(&result)?;
        }
        Ok(result)
    }

    pub(crate) fn execute_for_deterministic_eval(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
    ) -> Result<ActionExecutionResult> {
        static EVAL_RUNTIME: std::sync::OnceLock<std::sync::Mutex<tokio::runtime::Runtime>> =
            std::sync::OnceLock::new();
        let runtime = EVAL_RUNTIME.get_or_init(|| {
            std::sync::Mutex::new(
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build shared deterministic ToolGateway eval runtime"),
            )
        });
        runtime
            .lock()
            .expect("deterministic ToolGateway eval runtime mutex poisoned")
            .block_on(self.execute(request, ctx))
    }
}

fn validate_gateway_request_contract(
    request: &AgentActionRequest,
    ctx: &ActionExecutionContext<'_>,
) -> std::result::Result<ToolGatewayContractEvidence, String> {
    match request.action_type.as_str() {
        "memory_search" | "session_search" => {
            let run_id = request
                .source_run_id
                .as_deref()
                .filter(|run_id| !run_id.trim().is_empty())
                .ok_or_else(|| "internal_read_canonical_run_identity_missing".to_string())?;
            let contract = internal_read_contract(&request.action_type, &request.target)?;
            if ctx.bound_content_receipt_issuer.is_none() {
                return Err("bound_content_receipt_issuer_unavailable".into());
            }
            let owner = ctx.agent_run_store.ok_or_else(|| {
                "internal_read_canonical_run_owner_authority_unavailable".to_string()
            })?;
            let owner_is_active =
                owner
                    .has_active_bound_content_owner(run_id)
                    .map_err(|error| {
                        // This contract failure is intentionally returned as a typed
                        // blocker. Observe the raw durable error before that rewrite
                        // so product persistence health remains truthful.
                        ctx.observe_durable_store_failure("AgentRunStore", &error);
                        "internal_read_canonical_run_owner_authority_unavailable".to_string()
                    })?;
            if !owner_is_active {
                return Err("internal_read_canonical_run_owner_inactive".into());
            }
            Ok(contract)
        }
        "memory_write" | "memory_archive" | "life_model_patch" => {
            Ok(internal_proposal_contract(&request.action_type))
        }
        "mcp_tool" | "builtin_tool" | "plugin_tool" => {
            let tool_name = normalize_tool_name(&request.target, ctx.registry);
            let manifest = find_manifest(ctx, &tool_name)
                .ok_or_else(|| "tool_gateway_manifest_not_found".to_string())?;
            let contract = validate_manifest_execution_contract(&manifest)?;

            if manifest.name == "mcp.call_tool" {
                return validate_mcp_target_contract(request, ctx);
            }

            Ok(contract)
        }
        _ => Err("tool_gateway_unsupported_action_type".into()),
    }
}

fn validate_mcp_target_contract(
    request: &AgentActionRequest,
    ctx: &ActionExecutionContext<'_>,
) -> std::result::Result<ToolGatewayContractEvidence, String> {
    let args = request
        .input
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| request.input.clone());
    let Some(tool_name) = args
        .get("tool_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err("tool_gateway_mcp_target_missing".into());
    };
    let target_manifest = find_manifest(ctx, tool_name)
        .ok_or_else(|| "tool_gateway_mcp_target_manifest_not_found".to_string())?;
    validate_manifest_execution_contract(&target_manifest)
}

fn find_manifest(ctx: &ActionExecutionContext<'_>, tool_name: &str) -> Option<ToolManifest> {
    ctx.registry
        .list_manifests()
        .into_iter()
        .find(|manifest| manifest.name == tool_name || manifest.id == tool_name)
}

pub fn validate_manifest_execution_contract(
    manifest: &ToolManifest,
) -> std::result::Result<ToolGatewayContractEvidence, String> {
    if manifest
        .tags
        .iter()
        .any(|tag| tag.starts_with("migration:name_inferred_contract"))
    {
        return Err("tool_gateway_inferred_manifest_contract_no_execution_credit".into());
    }
    if manifest.name.trim().is_empty() || manifest.id.trim().is_empty() {
        return Err("tool_gateway_manifest_identity_missing".into());
    }
    if matches!(
        manifest.source,
        ToolSource::Plugin { .. } | ToolSource::A2A { .. }
    ) {
        return Err("tool_gateway_source_executor_unavailable".into());
    }
    if !manifest.enabled {
        return Err("tool_gateway_manifest_disabled".into());
    }
    if manifest.declarative_only {
        return Err("tool_gateway_manifest_declarative_only".into());
    }
    if manifest.permission_level.trim().is_empty() {
        return Err("tool_gateway_permission_contract_missing".into());
    }
    if manifest.risk_level.trim().is_empty() {
        return Err("tool_gateway_risk_contract_missing".into());
    }
    if manifest.action_type.trim().is_empty() {
        return Err("tool_gateway_action_type_contract_missing".into());
    }
    if manifest.capabilities.is_empty() {
        return Err("tool_gateway_capability_contract_missing".into());
    }
    if !manifest.parameters.is_object() {
        return Err("tool_gateway_parameter_contract_missing".into());
    }
    if !is_known_permission_level(&manifest.permission_level) {
        return Err("tool_gateway_permission_contract_unknown".into());
    }
    if !is_known_risk_level(&manifest.risk_level) {
        return Err("tool_gateway_risk_contract_unknown".into());
    }
    if !is_known_action_type(&manifest.action_type) {
        return Err("tool_gateway_action_type_contract_unknown".into());
    }
    if manifest
        .capabilities
        .iter()
        .any(|capability| capability.trim().is_empty())
    {
        return Err("tool_gateway_capability_contract_incomplete".into());
    }
    if manifest.idempotency_contract == ToolIdempotencyContract::Unspecified {
        return Err("tool_gateway_idempotency_contract_missing".into());
    }

    Ok(ToolGatewayContractEvidence {
        tool_name: manifest.name.clone(),
        manifest_id: manifest.id.clone(),
        source: manifest.source.to_string(),
        permission_level: manifest.permission_level.clone(),
        risk_level: manifest.risk_level.clone(),
        action_type: manifest.action_type.clone(),
        capabilities: manifest.capabilities.clone(),
        evidence_contract: evidence_contract_for_manifest(manifest),
        action_effect: ToolActionEffect::from_contract(
            &manifest.action_type,
            &manifest.capabilities,
        ),
        idempotency_contract: manifest.idempotency_contract,
    })
}

fn evidence_contract_for_manifest(manifest: &ToolManifest) -> Vec<String> {
    let mut evidence = vec![
        "tool_manifest_contract".into(),
        "permission_decision".into(),
        "action_record".into(),
        "observation_record".into(),
    ];
    if is_proposal_generation_tool(&manifest.name)
        || manifest.capabilities.iter().any(|capability| {
            matches!(
                capability.to_ascii_lowercase().as_str(),
                "write" | "memory" | "lifemodel" | "external_side_effect"
            )
        })
    {
        evidence.push("proposal_or_blocker_record".into());
    }
    evidence
}

fn internal_read_contract(
    executor_action_type: &str,
    requested_target: &str,
) -> std::result::Result<ToolGatewayContractEvidence, String> {
    let canonical_tool_name = match executor_action_type {
        "memory_search" => "memory.search",
        "session_search" => "session.search",
        _ => return Err("tool_gateway_internal_read_action_type_unknown".into()),
    };
    if requested_target != canonical_tool_name {
        return Err("tool_gateway_internal_read_target_mismatch".into());
    }
    Ok(ToolGatewayContractEvidence {
        tool_name: canonical_tool_name.into(),
        manifest_id: executor_action_type.into(),
        source: "tool_gateway_internal".into(),
        permission_level: "low".into(),
        risk_level: "low".into(),
        action_type: "read".into(),
        capabilities: vec!["read".into()],
        evidence_contract: vec![
            "gateway_internal_contract".into(),
            "action_record".into(),
            "observation_record".into(),
        ],
        action_effect: ToolActionEffect::ReadOnly,
        idempotency_contract: ToolIdempotencyContract::Idempotent,
    })
}

fn internal_proposal_contract(action_type: &str) -> ToolGatewayContractEvidence {
    ToolGatewayContractEvidence {
        tool_name: action_type.into(),
        manifest_id: action_type.into(),
        source: "tool_gateway_internal".into(),
        permission_level: "medium".into(),
        risk_level: "medium".into(),
        action_type: "proposal_only_write".into(),
        capabilities: vec!["write".into()],
        evidence_contract: vec![
            "gateway_internal_contract".into(),
            "proposal_or_blocker_record".into(),
            "action_record".into(),
            "observation_record".into(),
        ],
        action_effect: ToolActionEffect::ProposalOnly,
        idempotency_contract: ToolIdempotencyContract::NonIdempotent,
    }
}

fn is_known_permission_level(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "low" | "medium" | "high" | "critical"
    )
}

fn is_known_risk_level(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "low" | "medium" | "high" | "critical"
    )
}

fn is_known_action_type(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "read" | "write" | "network" | "external_side_effect" | "proposal_only_write"
    )
}

fn blocked_gateway_result(
    request: AgentActionRequest,
    reason: &str,
    execution_receipt: ToolExecutionReceipt,
) -> ActionExecutionResult {
    gateway_terminal_result(
        request,
        reason,
        "blocked",
        ActionExecutionStatus::Blocked,
        execution_receipt,
    )
}

fn failed_gateway_result(
    request: AgentActionRequest,
    reason: &str,
    execution_receipt: ToolExecutionReceipt,
) -> ActionExecutionResult {
    gateway_terminal_result(
        request,
        reason,
        "failed",
        ActionExecutionStatus::Failed,
        execution_receipt,
    )
}

fn gateway_terminal_result(
    request: AgentActionRequest,
    reason: &str,
    status_label: &str,
    status: ActionExecutionStatus,
    execution_receipt: ToolExecutionReceipt,
) -> ActionExecutionResult {
    let now = chrono::Utc::now();
    let action_id = format!(
        "action-tool-gateway-blocked-{}",
        now.timestamp_nanos_opt().unwrap_or_default()
    );
    let observation_id = format!(
        "obs-tool-gateway-blocked-{}",
        now.timestamp_nanos_opt().unwrap_or_default()
    );
    let action = AgentAction {
        id: action_id.clone(),
        action_type: request.action_type,
        target: Some(request.target),
        input: request.input,
        output: Some(serde_json::json!({
            "success": false,
            "status": status_label,
            "toolGatewayAuthority": true,
            "blockerReason": reason,
            "directWritesExecuted": false,
        })),
        status: status_label.into(),
        error: Some(reason.into()),
        permission_decision: Some(reason.into()),
        started_at: Some(now),
        finished_at: Some(now),
        timestamp: now,
        tool_scope: None,
        react_trace: None,
        runtime_execution_receipt: None,
    };
    let observation = AgentObservation {
        id: observation_id,
        action_id: Some(action_id),
        content: format!("ToolGateway {status_label} execution: {reason}"),
        source: "tool_gateway".into(),
        structured_result: Some(serde_json::json!({
            "success": false,
            "status": status_label,
            "toolGatewayAuthority": true,
            "blockerReason": reason,
            "directWritesExecuted": false,
        })),
        timestamp: now,
        react_trace: None,
    };
    ActionExecutionResult::without_observed_body(
        action,
        observation,
        status,
        Some(reason.into()),
        None,
        execution_receipt,
    )
}

fn gateway_contract_evidence(contract: &ToolGatewayContractEvidence) -> Value {
    serde_json::json!({
        "toolGatewayAuthority": true,
        "manifestId": contract.manifest_id,
        "toolName": contract.tool_name,
        "source": contract.source,
        "permissionLevel": contract.permission_level,
        "riskLevel": contract.risk_level,
        "actionType": contract.action_type,
        "capabilities": contract.capabilities,
        "evidenceContract": contract.evidence_contract,
        "actionEffect": contract.action_effect,
        "idempotencyContract": contract.idempotency_contract,
        "inferredNameContractCredit": false,
    })
}

fn bind_receipt_to_gateway_action(
    tracker: &crate::tool_execution_receipt::ToolExecutionReceiptTracker,
    result: &mut ActionExecutionResult,
) -> Result<()> {
    tracker
        .bind_action_identity(
            &result.action.id,
            &result.action.action_type,
            result.action.target.as_deref(),
            &result.action.input,
        )
        .map_err(|reason| anyhow::anyhow!("tool_gateway_action_receipt_binding_failed:{reason}"))?;
    result.execution_receipt = tracker.snapshot();
    Ok(())
}

fn attach_execution_receipt_evidence(result: &mut ActionExecutionResult) {
    // The live AgentAction sidecar is the only receipt authority that may
    // cross AgentLoop in-process. The JSON copies below are display/audit
    // mirrors; serde round-trips deliberately cannot recreate this value.
    result.action.runtime_execution_receipt = Some(result.execution_receipt.clone());
    let receipt = serde_json::to_value(&result.execution_receipt).unwrap_or_else(|_| {
        serde_json::json!({
            "receiptId": result.execution_receipt.receipt_id,
            "transportStatus": ToolTransportStatus::NotAttempted,
            "effectStatus": ToolEffectStatus::Unknown,
        })
    });
    if let Some(output) = result.action.output.as_mut() {
        if let Some(object) = output.as_object_mut() {
            object.insert("toolExecutionReceipt".into(), receipt.clone());
        }
    }
    if let Some(structured) = result.observation.structured_result.as_mut() {
        if let Some(object) = structured.as_object_mut() {
            object.insert("toolExecutionReceipt".into(), receipt);
        }
    } else {
        result.observation.structured_result = Some(serde_json::json!({
            "toolExecutionReceipt": receipt,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpRegistry;
    use crate::mcp_audit::McpAuditStore;
    use crate::privacy::PrivacyEngine;
    use crate::tool_manifest::{ToolManifest, ToolSource};
    use crate::tool_permissions::ToolPermissionStore;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct CountingDispatchObserver {
        count: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl crate::agent::ToolDispatchObserver for CountingDispatchObserver {
        async fn before_dispatch(
            &self,
            _attempt: &crate::agent::ToolDispatchAttempt,
        ) -> anyhow::Result<()> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingAuditPersistenceObserver {
        count: AtomicUsize,
        last_status: Mutex<Option<crate::tool_execution_receipt::ToolAuditPersistenceStatus>>,
    }

    impl crate::agent::ToolAuditPersistenceObserver for RecordingAuditPersistenceObserver {
        fn audit_persistence_failed(&self, receipt: &ToolExecutionReceipt) {
            self.count.fetch_add(1, Ordering::SeqCst);
            *self.last_status.lock().unwrap() = Some(receipt.audit_persistence_status);
        }
    }

    fn explicit_read_manifest() -> ToolManifest {
        ToolManifest {
            id: "notes.read".into(),
            name: "notes.read".into(),
            description: "Read notes.".into(),
            parameters: serde_json::json!({"type": "object"}),
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: ToolSource::BuiltIn,
            capabilities: vec!["read".into()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            idempotency_contract: ToolIdempotencyContract::Idempotent,
            tags: vec![],
        }
    }

    fn create_tool_execution_owner(
        tool_name: &str,
    ) -> (crate::agent::AgentRunStore, crate::agent::AgentRun) {
        let store = crate::agent::AgentRunStore::new_in_memory().unwrap();
        let run = crate::agent::AgentRun::new_tool_execution_run(tool_name);
        store
            .create_run(&run)
            .expect("create canonical ToolGateway execution owner");
        (store, run)
    }

    fn issuance_count_for_run(path: &std::path::Path, run_id: &str) -> i64 {
        rusqlite::Connection::open(path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM bound_content_issuance_ledger WHERE run_id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn attach_result_to_owner(
        store: &crate::agent::AgentRunStore,
        run: &mut crate::agent::AgentRun,
        result: &ActionExecutionResult,
    ) {
        run.actions.push(result.action.clone());
        run.observations.push(result.observation.clone());
        run.step_count = run.step_count.saturating_add(1);
        run.tool_call_count = run.tool_call_count.saturating_add(1);
        store
            .update_run(run)
            .expect("CAS-attach ToolGateway body receipt to canonical owner");
        let persisted = store
            .get_run(&run.id)
            .unwrap()
            .expect("reload canonical ToolGateway execution owner");
        let receipt = persisted
            .actions
            .last()
            .and_then(|action| action.react_trace.as_ref())
            .and_then(|trace| trace.output_receipt.as_ref())
            .expect("owner has one durable bound-content receipt");
        assert_eq!(receipt.version(), 2);
        assert!(persisted
            .observations
            .last()
            .is_some_and(|observation| observation.react_trace.is_none()));
    }

    fn internal_read_test_context<'a>(
        registry: &'a McpRegistry,
        permission_store: &'a ToolPermissionStore,
        audit_store: &'a McpAuditStore,
        privacy_engine: &'a PrivacyEngine,
        memory_store: &'a crate::memory::MemoryStore,
        lifecycle_reader: Option<&'a crate::agent::MemoryLifecycleRetrievalReader>,
        owner_store: Option<&'a crate::agent::AgentRunStore>,
    ) -> ActionExecutionContext<'a> {
        let mut context = ActionExecutionContext::new(
            registry,
            permission_store,
            audit_store,
            privacy_engine,
            &[],
        )
        .with_memory_store(memory_store);
        if let Some(reader) = lifecycle_reader {
            context = context.with_memory_lifecycle_retrieval_reader(reader);
        }
        if let Some(store) = owner_store {
            context = context.with_agent_run_store(store);
        }
        context
    }

    #[tokio::test]
    async fn internal_read_contract_rejects_executor_target_drift_before_dispatch() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let memory_store = crate::memory::MemoryStore::new_in_memory().unwrap();
        let owner_store = crate::agent::AgentRunStore::new_in_memory().unwrap();
        let observer = CountingDispatchObserver::default();
        let context = internal_read_test_context(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &memory_store,
            None,
            Some(&owner_store),
        )
        .with_tool_dispatch_observer(&observer);

        for (action_type, wrong_target) in [
            ("memory_search", "session.search"),
            ("session_search", "memory.search"),
        ] {
            let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
                .execute(
                    AgentActionRequest {
                        action_type: action_type.into(),
                        target: wrong_target.into(),
                        input: serde_json::json!({"query": "target drift"}),
                        source_run_id: Some(uuid::Uuid::new_v4().to_string()),
                        step_index: 0,
                    },
                    &context,
                )
                .await
                .expect("contract drift becomes a typed pre-dispatch blocker");

            assert_eq!(result.status, ActionExecutionStatus::Blocked);
            assert_eq!(
                result.stop_reason.as_deref(),
                Some("tool_gateway_internal_read_target_mismatch")
            );
            assert_eq!(
                result.execution_receipt.transport_status,
                crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
            );
        }
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn internal_memory_and_session_reads_attach_bound_receipts_without_body_persistence() {
        const MEMORY_BODY: &str = "INTERNAL_MEMORY_RECEIPT_BODY_4b1f8d";
        const SESSION_BODY: &str = "INTERNAL_SESSION_RECEIPT_BODY_9c70aa";

        let directory = tempfile::tempdir().unwrap();
        let agent_run_path = directory.path().join("agent-runs.db");
        let owner_store = crate::agent::AgentRunStore::new(&agent_run_path).unwrap();
        let mut owner_run = crate::agent::AgentRun::new_tool_execution_run("internal reads");
        owner_store.create_run(&owner_run).unwrap();

        let memory_store = crate::memory::MemoryStore::new_in_memory().unwrap();
        memory_store
            .save_memory_record(
                "memory-owner-session",
                MEMORY_BODY,
                "note",
                "manual_test",
                &[],
                "private",
                None,
            )
            .unwrap();
        memory_store
            .save_message(
                "conversation-owner-session",
                &crate::llm::ChatMessage {
                    role: "user".into(),
                    content: SESSION_BODY.into(),
                },
            )
            .unwrap();
        let lifecycle_store = crate::agent::MemoryLifecycleStore::new_in_memory().unwrap();
        let lifecycle_reader = lifecycle_store.retrieval_reader();
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        {
            let context = internal_read_test_context(
                &registry,
                &permission_store,
                &audit_store,
                &privacy_engine,
                &memory_store,
                Some(&lifecycle_reader),
                Some(&owner_store),
            );

            for (step_index, action_type, target, session_id, query, body) in [
                (
                    0,
                    "memory_search",
                    "memory.search",
                    "memory-owner-session",
                    "INTERNAL_MEMORY_RECEIPT_BODY",
                    MEMORY_BODY,
                ),
                (
                    1,
                    "session_search",
                    "session.search",
                    "conversation-owner-session",
                    "INTERNAL_SESSION_RECEIPT_BODY",
                    SESSION_BODY,
                ),
            ] {
                let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
                    .execute(
                        AgentActionRequest {
                            action_type: action_type.into(),
                            target: target.into(),
                            input: serde_json::json!({
                                "query": query,
                                "session_id": session_id,
                                "limit": 5,
                            }),
                            source_run_id: Some(owner_run.id.clone()),
                            step_index,
                        },
                        &context,
                    )
                    .await
                    .expect("internal read must produce a canonical evidence graph");

                assert_eq!(result.status, ActionExecutionStatus::Succeeded);
                assert_eq!(result.action.action_type, target);
                let scope = result
                    .action
                    .tool_scope
                    .as_ref()
                    .expect("internal read has a typed ToolGateway scope");
                assert_eq!(scope.tool_name, target);
                assert_eq!(scope.source, "tool_gateway_internal");
                assert_eq!(scope.action_type, "read");
                assert_eq!(scope.capabilities, vec!["read"]);
                assert!(scope.allowed);
                let trace = result
                    .action
                    .react_trace
                    .as_ref()
                    .expect("internal read has an owner-bound trace");
                assert_eq!(trace.run_id.as_deref(), Some(owner_run.id.as_str()));
                assert_eq!(trace.action_type, target);
                assert_eq!(trace.tool_name, target);
                assert_eq!(trace.tool_source, "tool_gateway_internal");
                let receipt = trace
                    .output_receipt
                    .as_ref()
                    .expect("adapter-observed internal body has a receipt");
                assert_eq!(receipt.version(), 2);
                assert_eq!(receipt.kind(), crate::agent::ContentReceiptKind::ToolOutput);
                assert!(result.observation.react_trace.is_none());
                assert!(result.observation.content.contains(body));

                owner_run.actions.push(result.action);
                owner_run.observations.push(result.observation);
            }
            owner_run.step_count = 2;
            owner_run.tool_call_count = 2;
            owner_store.update_run(&owner_run).unwrap();

            let stored = owner_store.get_run(&owner_run.id).unwrap().unwrap();
            assert_eq!(stored.actions.len(), 2);
            assert_eq!(stored.observations.len(), 2);
            assert!(stored.actions.iter().all(|action| action
                .react_trace
                .as_ref()
                .and_then(|trace| trace.output_receipt.as_ref())
                .is_some()));
            let serialized = serde_json::to_string(&stored).unwrap();
            assert!(!serialized.contains(MEMORY_BODY));
            assert!(!serialized.contains(SESSION_BODY));
        }

        drop(owner_store);
        let ledger = rusqlite::Connection::open(&agent_run_path).unwrap();
        let mut statement = ledger
            .prepare("SELECT receipt_json FROM bound_content_issuance_ledger ORDER BY issued_at")
            .unwrap();
        let receipt_rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(receipt_rows.len(), 2);
        for receipt_json in receipt_rows {
            assert!(!receipt_json.contains(MEMORY_BODY));
            assert!(!receipt_json.contains(SESSION_BODY));
        }
        drop(statement);
        drop(ledger);
        for candidate in [
            agent_run_path.clone(),
            std::path::PathBuf::from(format!("{}-wal", agent_run_path.display())),
            std::path::PathBuf::from(format!("{}-shm", agent_run_path.display())),
        ] {
            if candidate.exists() {
                let bytes = std::fs::read(&candidate).unwrap();
                for body in [MEMORY_BODY.as_bytes(), SESSION_BODY.as_bytes()] {
                    assert!(
                        !bytes.windows(body.len()).any(|window| window == body),
                        "internal read body leaked into {}",
                        candidate.display()
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn internal_read_success_requires_run_identity_and_receipt_issuer() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let memory_store = crate::memory::MemoryStore::new_in_memory().unwrap();
        memory_store
            .save_message(
                "identity-required-session",
                &crate::llm::ChatMessage {
                    role: "user".into(),
                    content: "identity-required canonical session body".into(),
                },
            )
            .unwrap();
        let owner_store = crate::agent::AgentRunStore::new_in_memory().unwrap();
        let owner_run = crate::agent::AgentRun::new_tool_execution_run("session.search");
        owner_store.create_run(&owner_run).unwrap();
        let request = |run_id: Option<String>| AgentActionRequest {
            action_type: "session_search".into(),
            target: "session.search".into(),
            input: serde_json::json!({
                "query": "identity-required",
                "session_id": "identity-required-session",
                "limit": 5,
            }),
            source_run_id: run_id,
            step_index: 0,
        };

        let no_run_context = internal_read_test_context(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &memory_store,
            None,
            Some(&owner_store),
        );
        let no_run_result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(request(None), &no_run_context)
            .await
            .expect("missing run identity is a typed pre-dispatch blocker");
        assert_eq!(no_run_result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            no_run_result.stop_reason.as_deref(),
            Some("internal_read_canonical_run_identity_missing")
        );
        assert_eq!(
            no_run_result.execution_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
        );

        let no_issuer_context = internal_read_test_context(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &memory_store,
            None,
            None,
        );
        let no_issuer_result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(request(Some(owner_run.id)), &no_issuer_context)
            .await
            .expect("missing receipt issuer is a typed pre-dispatch blocker");
        assert_eq!(no_issuer_result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            no_issuer_result.stop_reason.as_deref(),
            Some("bound_content_receipt_issuer_unavailable")
        );
        assert_eq!(
            no_issuer_result.execution_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
        );
    }

    #[tokio::test]
    async fn internal_read_requires_current_active_owner_before_dispatch_or_issuance() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("active-owner.db");
        let owner_store = crate::agent::AgentRunStore::new(&path).unwrap();
        let memory_store = crate::memory::MemoryStore::new_in_memory().unwrap();
        memory_store
            .save_message(
                "active-owner-session",
                &crate::llm::ChatMessage {
                    role: "user".into(),
                    content: "active owner session body".into(),
                },
            )
            .unwrap();
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let observer = CountingDispatchObserver::default();
        let context = internal_read_test_context(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &memory_store,
            None,
            Some(&owner_store),
        )
        .with_tool_dispatch_observer(&observer);
        let run_id = uuid::Uuid::new_v4().to_string();
        let request = AgentActionRequest {
            action_type: "session_search".into(),
            target: "session.search".into(),
            input: serde_json::json!({
                "query": "active owner",
                "session_id": "active-owner-session",
                "limit": 5,
            }),
            source_run_id: Some(run_id.clone()),
            step_index: 0,
        };

        let missing = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(request.clone(), &context)
            .await
            .expect("missing owner becomes a typed blocker");
        assert_eq!(missing.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            missing.stop_reason.as_deref(),
            Some("internal_read_canonical_run_owner_inactive")
        );
        assert_eq!(
            missing.execution_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
        );
        assert_eq!(
            missing.execution_receipt.effect_status,
            crate::tool_execution_receipt::ToolEffectStatus::NotAttempted
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
        assert_eq!(issuance_count_for_run(&path, &run_id), 0);

        let mut later_owner = crate::agent::AgentRun::new_tool_execution_run("session.search");
        later_owner.id = run_id.clone();
        owner_store.create_run(&later_owner).unwrap();
        assert_eq!(
            issuance_count_for_run(&path, &run_id),
            0,
            "creating the same id later must not revive a blocked issuance"
        );
        assert!(missing
            .action
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .is_none());

        let active = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(request, &context)
            .await
            .expect("the newly active owner may authorize a new read");
        assert_eq!(active.status, ActionExecutionStatus::Succeeded);
        assert_eq!(issuance_count_for_run(&path, &run_id), 1);

        let observer_count_before_tombstone = observer.count.load(Ordering::SeqCst);
        let tombstoned_id = uuid::Uuid::new_v4().to_string();
        let mut tombstoned = crate::agent::AgentRun::new_tool_execution_run("session.search");
        tombstoned.id = tombstoned_id.clone();
        owner_store.create_run(&tombstoned).unwrap();
        owner_store
            .delete_run_with_tombstone(&tombstoned_id, Some("owner removed"))
            .unwrap();
        let tombstoned_result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    source_run_id: Some(tombstoned_id.clone()),
                    ..AgentActionRequest {
                        action_type: "session_search".into(),
                        target: "session.search".into(),
                        input: serde_json::json!({
                            "query": "active owner",
                            "session_id": "active-owner-session",
                            "limit": 5,
                        }),
                        source_run_id: None,
                        step_index: 1,
                    }
                },
                &context,
            )
            .await
            .expect("tombstoned owner becomes a typed blocker");
        assert_eq!(tombstoned_result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            tombstoned_result.stop_reason.as_deref(),
            Some("internal_read_canonical_run_owner_inactive")
        );
        assert_eq!(
            tombstoned_result.execution_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
        );
        assert_eq!(
            tombstoned_result.execution_receipt.effect_status,
            crate::tool_execution_receipt::ToolEffectStatus::NotAttempted
        );
        assert_eq!(
            observer.count.load(Ordering::SeqCst),
            observer_count_before_tombstone,
            "tombstoned owner must block before adapter dispatch"
        );
        assert_eq!(issuance_count_for_run(&path, &tombstoned_id), 0);
    }

    #[tokio::test]
    async fn internal_read_rejects_every_non_running_canonical_owner_before_dispatch() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("non-running-owner.db");
        let owner_store = crate::agent::AgentRunStore::new(&path).unwrap();
        let memory_store = crate::memory::MemoryStore::new_in_memory().unwrap();
        memory_store
            .save_message(
                "non-running-owner-session",
                &crate::llm::ChatMessage {
                    role: "user".into(),
                    content: "non-running owner session body".into(),
                },
            )
            .unwrap();
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let observer = CountingDispatchObserver::default();
        let context = internal_read_test_context(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &memory_store,
            None,
            Some(&owner_store),
        )
        .with_tool_dispatch_observer(&observer);

        for status in [
            crate::agent::AgentRunStatus::WaitingPermission,
            crate::agent::AgentRunStatus::Completed,
            crate::agent::AgentRunStatus::Failed,
            crate::agent::AgentRunStatus::RemoteUnknown,
            crate::agent::AgentRunStatus::Cancelled,
        ] {
            let mut owner = crate::agent::AgentRun::new_tool_execution_run("session.search");
            owner.id = format!("non-running-owner-{status}");
            owner_store.create_run(&owner).unwrap();
            owner.status = status;
            if status != crate::agent::AgentRunStatus::WaitingPermission {
                owner.finished_at = Some(chrono::Utc::now());
            }
            owner_store.update_run(&owner).unwrap();

            let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
                .execute(
                    AgentActionRequest {
                        action_type: "session_search".into(),
                        target: "session.search".into(),
                        input: serde_json::json!({
                            "query": "non-running owner",
                            "session_id": "non-running-owner-session",
                            "limit": 5,
                        }),
                        source_run_id: Some(owner.id.clone()),
                        step_index: 0,
                    },
                    &context,
                )
                .await
                .expect("non-running owner becomes a typed pre-dispatch blocker");

            assert_eq!(result.status, ActionExecutionStatus::Blocked, "{status}");
            assert_eq!(
                result.stop_reason.as_deref(),
                Some("internal_read_canonical_run_owner_inactive"),
                "{status}"
            );
            assert_eq!(
                result.execution_receipt.transport_status,
                crate::tool_execution_receipt::ToolTransportStatus::NotAttempted,
                "{status}"
            );
            assert_eq!(
                result.execution_receipt.effect_status,
                crate::tool_execution_receipt::ToolEffectStatus::NotAttempted,
                "{status}"
            );
            assert_eq!(issuance_count_for_run(&path, &owner.id), 0, "{status}");
        }
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn failed_internal_memory_read_has_typed_graph_without_observed_body_receipt() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let memory_store = crate::memory::MemoryStore::new_in_memory().unwrap();
        memory_store
            .save_memory_record(
                "failure-session",
                "failure-path body",
                "note",
                "manual_test",
                &[],
                "private",
                None,
            )
            .unwrap();
        let owner_store = crate::agent::AgentRunStore::new_in_memory().unwrap();
        let mut owner_run = crate::agent::AgentRun::new_tool_execution_run("memory.search");
        owner_store.create_run(&owner_run).unwrap();
        let context = internal_read_test_context(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &memory_store,
            None,
            Some(&owner_store),
        );

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "memory_search".into(),
                    target: "memory.search".into(),
                    input: serde_json::json!({
                        "query": "failure-path",
                        "session_id": "failure-session",
                    }),
                    source_run_id: Some(owner_run.id.clone()),
                    step_index: 0,
                },
                &context,
            )
            .await
            .expect("typed lifecycle failure is an execution result");

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert_eq!(
            result.stop_reason.as_deref(),
            Some("memory_lifecycle_reader_unavailable")
        );
        assert!(result
            .action
            .tool_scope
            .as_ref()
            .is_some_and(|scope| scope.allowed));
        let trace = result
            .action
            .react_trace
            .as_ref()
            .expect("failed internal read retains a typed graph");
        assert!(trace.output_receipt.is_none());
        assert!(result.observation.react_trace.is_some());

        owner_run.actions.push(result.action);
        owner_run.observations.push(result.observation);
        owner_run.step_count = 1;
        owner_run.tool_call_count = 1;
        owner_store.update_run(&owner_run).unwrap();
        let stored = owner_store.get_run(&owner_run.id).unwrap().unwrap();
        assert_eq!(stored.actions.len(), 1);
        assert_eq!(stored.observations.len(), 1);
        assert!(stored.actions[0]
            .react_trace
            .as_ref()
            .is_some_and(|trace| trace.output_receipt.is_none()));
    }

    #[test]
    fn explicit_manifest_contract_receives_gateway_credit() {
        let evidence = validate_manifest_execution_contract(&explicit_read_manifest())
            .expect("explicit manifest accepted");

        assert_eq!(evidence.tool_name, "notes.read");
        assert!(evidence.evidence_contract.contains(&"action_record".into()));
        assert!(evidence
            .evidence_contract
            .contains(&"observation_record".into()));
        assert_eq!(
            evidence.idempotency_contract,
            ToolIdempotencyContract::Idempotent
        );
    }

    #[test]
    fn inferred_manifest_contract_is_warning_only() {
        let mut manifest = explicit_read_manifest();
        manifest.capabilities = vec![];
        manifest.tags = vec!["migration:name_inferred_contract_warning".into()];

        assert_eq!(
            validate_manifest_execution_contract(&manifest).unwrap_err(),
            "tool_gateway_inferred_manifest_contract_no_execution_credit"
        );
    }

    #[test]
    fn incomplete_manifest_fails_closed() {
        let mut manifest = explicit_read_manifest();
        manifest.risk_level.clear();

        assert_eq!(
            validate_manifest_execution_contract(&manifest).unwrap_err(),
            "tool_gateway_risk_contract_missing"
        );
    }

    #[test]
    fn executable_manifest_without_typed_idempotency_fails_closed() {
        let mut manifest = explicit_read_manifest();
        manifest.idempotency_contract = ToolIdempotencyContract::Unspecified;

        assert_eq!(
            validate_manifest_execution_contract(&manifest).unwrap_err(),
            "tool_gateway_idempotency_contract_missing"
        );
    }

    #[test]
    fn disabled_manifest_contract_fails_closed() {
        let mut manifest = explicit_read_manifest();
        manifest.enabled = false;

        assert_eq!(
            validate_manifest_execution_contract(&manifest).unwrap_err(),
            "tool_gateway_manifest_disabled"
        );
    }

    #[test]
    fn declarative_only_manifest_contract_fails_closed() {
        let mut manifest = explicit_read_manifest();
        manifest.declarative_only = true;

        assert_eq!(
            validate_manifest_execution_contract(&manifest).unwrap_err(),
            "tool_gateway_manifest_declarative_only"
        );
    }

    #[tokio::test]
    async fn declarative_only_manifest_is_blocked_by_gateway_before_executor() {
        let mut registry = McpRegistry::new();
        let mut manifest = explicit_read_manifest();
        manifest.name = "declarative.only.read".into();
        manifest.id = "declarative.only.read".into();
        manifest.declarative_only = true;
        registry.register_builtin(
            manifest,
            Box::new(|_| panic!("declarative-only manifest must not reach ActionExecutor")),
        );
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let safe_paths: Vec<String> = Vec::new();
        let observer = CountingDispatchObserver::default();
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &safe_paths,
        )
        .with_tool_dispatch_observer(&observer);
        let gateway = ToolGateway::from_executor_config(ActionExecutorConfig::default());

        let result = gateway
            .execute(
                AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "declarative.only.read".into(),
                    input: serde_json::json!({}),
                    source_run_id: None,
                    step_index: 0,
                },
                &ctx,
            )
            .await
            .expect("gateway returns blocked result");

        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            result.stop_reason.as_deref(),
            Some("tool_gateway_manifest_declarative_only")
        );
        assert_eq!(result.observation.source, "tool_gateway");
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
        assert_eq!(
            result.execution_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
        );
        assert_eq!(
            result.execution_receipt.effect_status,
            crate::tool_execution_receipt::ToolEffectStatus::NotAttempted
        );
        assert!(result.execution_receipt.dispatched_at.is_none());
    }

    #[tokio::test]
    async fn dispatch_observer_runs_once_after_policy_checks_and_before_executor() {
        let mut registry = McpRegistry::new();
        registry.register_builtin(
            explicit_read_manifest(),
            Box::new(|_| Ok(serde_json::json!({"ok": true}).to_string())),
        );
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let safe_paths: Vec<String> = Vec::new();
        let observer = CountingDispatchObserver::default();
        let (owner_store, mut owner_run) = create_tool_execution_owner("notes.read");
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &safe_paths,
        )
        .with_agent_run_store(&owner_store)
        .with_tool_dispatch_observer(&observer);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "notes.read".into(),
                    input: serde_json::json!({}),
                    source_run_id: Some(owner_run.id.clone()),
                    step_index: 0,
                },
                &ctx,
            )
            .await
            .expect("governed read executes");
        attach_result_to_owner(&owner_store, &mut owner_run, &result);

        assert_eq!(result.status, ActionExecutionStatus::Succeeded);
        assert_eq!(observer.count.load(Ordering::SeqCst), 1);
        assert_eq!(
            result.execution_receipt.action_effect,
            crate::tool_execution_receipt::ToolActionEffect::ReadOnly
        );
        assert_eq!(
            result.execution_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::ResponseObserved
        );
        assert_eq!(
            result.execution_receipt.dispatch_kind,
            crate::tool_execution_receipt::ToolDispatchKind::Local
        );
        assert!(result.execution_receipt.is_runtime_bound_to_action(
            &owner_run.id,
            &result.action.id,
            &result.action.action_type,
            result.action.target.as_deref(),
            &result.action.input,
        ));
        assert!(
            !result.execution_receipt.automatic_retry_safe(),
            "a completed dispatch is not an automatic-retry candidate"
        );
    }

    #[tokio::test]
    async fn d067_tool_success_plus_audit_failure_preserves_effect_and_reports_audit_failure() {
        let callback_count = Arc::new(AtomicUsize::new(0));
        let mut registry = McpRegistry::new();
        let callback_count_for_tool = Arc::clone(&callback_count);
        registry.register_builtin(
            explicit_read_manifest(),
            Box::new(move |_| {
                callback_count_for_tool.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::json!({"ok": true}).to_string())
            }),
        );
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_store = McpAuditStore::unavailable_sentinel("d067_injected_audit_failure");
        let privacy_engine = PrivacyEngine::new();
        let observer = RecordingAuditPersistenceObserver::default();
        let (owner_store, mut owner_run) = create_tool_execution_owner("notes.read");
        let context = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_agent_run_store(&owner_store)
        .with_tool_audit_persistence_observer(&observer);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "notes.read".into(),
                    input: serde_json::json!({}),
                    source_run_id: Some(owner_run.id.clone()),
                    step_index: 0,
                },
                &context,
            )
            .await
            .expect("audit failure remains a typed terminal result");
        attach_result_to_owner(&owner_store, &mut owner_run, &result);

        assert_eq!(result.status, ActionExecutionStatus::Succeeded);
        assert_eq!(
            result.execution_receipt.execution_outcome,
            crate::tool_execution_receipt::ToolExecutionOutcome::Succeeded
        );
        assert_eq!(
            result.execution_receipt.transport_status,
            ToolTransportStatus::ResponseObserved
        );
        assert_eq!(
            result.execution_receipt.audit_persistence_status,
            crate::tool_execution_receipt::ToolAuditPersistenceStatus::Failed
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 1);
        assert_eq!(
            *observer.last_status.lock().unwrap(),
            Some(crate::tool_execution_receipt::ToolAuditPersistenceStatus::Failed)
        );
        assert_eq!(
            callback_count.load(Ordering::SeqCst),
            1,
            "audit failure must never retry the tool"
        );
    }

    #[tokio::test]
    async fn d067_tool_failure_plus_audit_failure_preserves_both_outcomes() {
        const ORIGINAL_TOOL_FAILURE: &str = "d067_original_tool_failure";
        let callback_count = Arc::new(AtomicUsize::new(0));
        let mut registry = McpRegistry::new();
        let mut manifest = explicit_read_manifest();
        manifest.id = "notes.fail".into();
        manifest.name = "notes.fail".into();
        let callback_count_for_tool = Arc::clone(&callback_count);
        registry.register_builtin(
            manifest,
            Box::new(move |_| {
                callback_count_for_tool.fetch_add(1, Ordering::SeqCst);
                anyhow::bail!(ORIGINAL_TOOL_FAILURE)
            }),
        );
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_store = McpAuditStore::unavailable_sentinel("d067_injected_audit_failure");
        let privacy_engine = PrivacyEngine::new();
        let observer = RecordingAuditPersistenceObserver::default();
        let context = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_tool_audit_persistence_observer(&observer);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "notes.fail".into(),
                    input: serde_json::json!({}),
                    source_run_id: None,
                    step_index: 0,
                },
                &context,
            )
            .await
            .expect("dual failure remains a typed terminal result");

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert_eq!(result.action.error.as_deref(), Some(ORIGINAL_TOOL_FAILURE));
        assert_eq!(result.observation.content, ORIGINAL_TOOL_FAILURE);
        assert_eq!(
            result.execution_receipt.execution_outcome,
            crate::tool_execution_receipt::ToolExecutionOutcome::Failed
        );
        assert_eq!(
            result.execution_receipt.audit_persistence_status,
            crate::tool_execution_receipt::ToolAuditPersistenceStatus::Failed
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 1);
        assert_eq!(callback_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn d067_normal_tool_terminal_writes_exactly_one_minimized_audit_row() {
        let mut registry = McpRegistry::new();
        registry.register_builtin(
            explicit_read_manifest(),
            Box::new(|_| Ok(serde_json::json!({"ok": true}).to_string())),
        );
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let (owner_store, mut owner_run) = create_tool_execution_owner("notes.read");
        let context = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_agent_run_store(&owner_store);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "notes.read".into(),
                    input: serde_json::json!({"query": "metadata only"}),
                    source_run_id: Some(owner_run.id.clone()),
                    step_index: 0,
                },
                &context,
            )
            .await
            .expect("normal tool execution succeeds");
        attach_result_to_owner(&owner_store, &mut owner_run, &result);

        assert_eq!(
            result.execution_receipt.audit_persistence_status,
            crate::tool_execution_receipt::ToolAuditPersistenceStatus::Committed
        );
        let rows = audit_store
            .list_logs(10)
            .expect("read minimized audit rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tool_name, "notes.read");
        assert!(rows[0].success);
        assert!(!rows[0].arguments.contains("metadata only"));
        assert!(!rows[0].result.contains("\"ok\":true"));
    }

    #[test]
    fn d067_warning_and_continue_audit_route_is_absent() {
        let source = include_str!("action_executor/tool_executor.rs");
        assert!(!source.contains("audit log write failed"));
        assert!(!source.contains("eprintln!(\"[warn] audit"));
        assert!(source.contains("mark_audit_persistence_failed"));
        assert!(source.contains("audit_persistence_failed"));
    }

    #[tokio::test]
    async fn bound_body_receipt_rejects_wrong_store_and_tamper_before_exact_owner_attach() {
        let mut registry = McpRegistry::new();
        registry.register_builtin(
            explicit_read_manifest(),
            Box::new(|_| Ok(serde_json::json!({"ok": true}).to_string())),
        );
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let (owner_store, mut owner_run) = create_tool_execution_owner("notes.read");
        let wrong_store = crate::agent::AgentRunStore::new_in_memory().unwrap();
        wrong_store
            .create_run(&owner_run)
            .expect("create same-shaped run under a different canonical owner");
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_agent_run_store(&owner_store);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "notes.read".into(),
                    input: serde_json::json!({}),
                    source_run_id: Some(owner_run.id.clone()),
                    step_index: 0,
                },
                &ctx,
            )
            .await
            .expect("exact owner issues the transient receipt");

        let mut wrong_owner_update = owner_run.clone();
        wrong_owner_update.actions.push(result.action.clone());
        wrong_owner_update
            .observations
            .push(result.observation.clone());
        let wrong_owner_error = wrong_store
            .update_run(&wrong_owner_update)
            .unwrap_err()
            .to_string();
        assert!(
            wrong_owner_error.contains("bound_content_receipt"),
            "wrong canonical store must not attach another owner's receipt: {wrong_owner_error}"
        );

        let mut tampered_action_json = serde_json::to_value(&result.action).unwrap();
        tampered_action_json["reactTrace"]["outputReceipt"]["authorityTag"] =
            Value::String(format!("hmac-sha256:{}", "00".repeat(32)));
        let mut tampered_update = owner_run.clone();
        tampered_update.actions.push(
            serde_json::from_value(tampered_action_json)
                .expect("receipt-shaped tamper remains deserializable but unauthenticated"),
        );
        tampered_update
            .observations
            .push(result.observation.clone());
        let tamper_error = owner_store
            .update_run(&tampered_update)
            .unwrap_err()
            .to_string();
        assert!(
            tamper_error.contains("bound_content_receipt"),
            "tampered receipt must fail before CAS attach: {tamper_error}"
        );

        attach_result_to_owner(&owner_store, &mut owner_run, &result);
    }

    #[tokio::test]
    async fn file_read_without_permission_stays_in_ask_before_preflight_or_dispatch() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let observer = CountingDispatchObserver::default();
        let safe_root = tempfile::tempdir().unwrap();
        let safe_paths = vec![safe_root.path().to_string_lossy().into_owned()];
        let missing_path = safe_root.path().join("missing.txt");
        let run_id = uuid::Uuid::new_v4().to_string();
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &safe_paths,
        )
        .with_tool_dispatch_observer(&observer);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "file.read".into(),
                    input: serde_json::json!({
                        "arguments": {"path": missing_path.to_string_lossy()}
                    }),
                    source_run_id: Some(run_id),
                    step_index: 0,
                },
                &ctx,
            )
            .await
            .expect("permission ask is a typed product state");

        assert_eq!(result.status, ActionExecutionStatus::NeedsConfirmation);
        assert_eq!(result.stop_reason.as_deref(), Some("blocked_by_policy"));
        assert_eq!(
            result
                .observation
                .structured_result
                .as_ref()
                .and_then(|value| value.get("requires_confirmation"))
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
        assert_eq!(
            result.execution_receipt.dispatch_kind,
            crate::tool_execution_receipt::ToolDispatchKind::NotAttempted
        );
        assert_eq!(result.execution_receipt.dispatch_attempt_count, 0);
        assert_eq!(
            result.execution_receipt.transport_status,
            ToolTransportStatus::NotAttempted
        );
        assert!(result
            .execution_receipt
            .mechanically_valid_terminal()
            .is_ok());
    }

    #[tokio::test]
    async fn missing_file_is_one_gateway_owned_local_failed_attempt() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "file.read",
                "builtin",
                "low",
                "read",
                crate::tool_permissions::ToolPermissionPolicy::AllowOnce,
                None,
            )
            .unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let observer = CountingDispatchObserver::default();
        let safe_root = tempfile::tempdir().unwrap();
        let safe_paths = vec![safe_root.path().to_string_lossy().into_owned()];
        let missing_path = safe_root.path().join("missing.txt");
        let (owner_store, owner_run) = create_tool_execution_owner("file.read");
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &safe_paths,
        )
        .with_agent_run_store(&owner_store)
        .with_tool_dispatch_observer(&observer);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "file.read".into(),
                    input: serde_json::json!({
                        "arguments": {"path": missing_path.to_string_lossy()}
                    }),
                    source_run_id: Some(owner_run.id.clone()),
                    step_index: 0,
                },
                &ctx,
            )
            .await
            .expect("missing file is a typed ToolGateway failure");

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert_eq!(
            result.stop_reason.as_deref(),
            Some("filesystem_read_failed")
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 1);
        assert_eq!(
            result.execution_receipt.dispatch_kind,
            crate::tool_execution_receipt::ToolDispatchKind::Local
        );
        assert_eq!(
            result.execution_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::ResponseObserved
        );
        assert_eq!(
            result.execution_receipt.execution_outcome,
            crate::tool_execution_receipt::ToolExecutionOutcome::Failed
        );
        assert!(result.execution_receipt.is_runtime_bound_to_action(
            &owner_run.id,
            &result.action.id,
            &result.action.action_type,
            result.action.target.as_deref(),
            &result.action.input,
        ));
    }

    #[tokio::test]
    async fn life_model_read_without_permission_stays_in_ask_before_dependency_access() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let observer = CountingDispatchObserver::default();
        let run_id = uuid::Uuid::new_v4().to_string();
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_tool_dispatch_observer(&observer);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "life_model.read".into(),
                    input: serde_json::json!({"arguments": {}}),
                    source_run_id: Some(run_id),
                    step_index: 0,
                },
                &ctx,
            )
            .await
            .expect("permission ask is a typed product state");

        assert_eq!(result.status, ActionExecutionStatus::NeedsConfirmation);
        assert_eq!(result.stop_reason.as_deref(), Some("blocked_by_policy"));
        assert_eq!(
            result
                .observation
                .structured_result
                .as_ref()
                .and_then(|value| value.get("requires_confirmation"))
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
        assert_eq!(
            result.execution_receipt.dispatch_kind,
            crate::tool_execution_receipt::ToolDispatchKind::NotAttempted
        );
        assert_eq!(result.execution_receipt.dispatch_attempt_count, 0);
        assert!(result
            .execution_receipt
            .mechanically_valid_terminal()
            .is_ok());
    }

    #[tokio::test]
    async fn network_policy_ask_stages_consent_before_any_tool_dispatch() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let observer = CountingDispatchObserver::default();
        let network_policy = crate::config::NetworkPolicy::default();
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_network_policy(&network_policy)
        .with_web_search_fixture_output("fixture must remain behind network consent")
        .with_proposal_store(&proposal_store)
        .with_canonical_write_admission(
            &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
        )
        .with_tool_dispatch_observer(&observer);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "web.search".into(),
                    input: serde_json::json!({"arguments": {"query": "OpenLife"}}),
                    source_run_id: Some("run-network-consent".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .await
            .expect("network ask must become a governed consent result");

        assert_eq!(result.status, ActionExecutionStatus::NeedsConfirmation);
        assert_eq!(
            result.stop_reason.as_deref(),
            Some("network_policy_consent_required")
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
        assert_eq!(
            result.execution_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
        );
        let pending = proposal_store.list_pending_proposals(10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(
            pending[0].proposal_type,
            crate::agent::ProposalType::ToolPermission
        );
        assert_eq!(pending[0].after["permission"], "allow_once");
        assert!(
            pending[0].after["canonical_scope"]["network_policy_decision_id"]
                .as_str()
                .is_some_and(|id| id.starts_with("network-policy:sha256:"))
        );
    }

    #[tokio::test]
    async fn proposal_write_without_canonical_admission_fails_before_dispatch() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let network_policy = crate::config::NetworkPolicy::default();
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_network_policy(&network_policy)
        .with_web_search_fixture_output("must never dispatch")
        .with_proposal_store(&proposal_store);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "web.search".into(),
                    input: serde_json::json!({"arguments": {"query": "OpenLife"}}),
                    source_run_id: Some("run-missing-canonical-admission".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .await
            .expect("missing admission becomes a typed failed result");

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert!(result
            .stop_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("canonical_write_admission_missing")));
        assert_eq!(
            result.execution_receipt.transport_status,
            ToolTransportStatus::NotAttempted
        );
        assert_eq!(
            result.execution_receipt.effect_status,
            crate::tool_execution_receipt::ToolEffectStatus::NotAttempted
        );
        assert!(proposal_store
            .list_pending_proposals(10)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn network_policy_default_deny_blocks_without_consent_or_dispatch() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let observer = CountingDispatchObserver::default();
        let network_policy = crate::config::NetworkPolicy {
            default_decision: "deny".into(),
            ..Default::default()
        };
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_network_policy(&network_policy)
        .with_web_search_fixture_output("fixture must remain behind network policy")
        .with_proposal_store(&proposal_store)
        .with_canonical_write_admission(
            &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
        )
        .with_tool_dispatch_observer(&observer);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "web.search".into(),
                    input: serde_json::json!({"arguments": {"query": "OpenLife"}}),
                    source_run_id: Some("run-network-deny".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .await
            .expect("network deny must become a governed blocker");

        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            result.stop_reason.as_deref(),
            Some("network_policy_default_deny")
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
        assert!(proposal_store
            .list_pending_proposals(10)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn network_policy_explicit_allow_preserves_web_tool_capability() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let observer = CountingDispatchObserver::default();
        let (owner_store, mut owner_run) = create_tool_execution_owner("web.search");
        let network_policy = crate::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_network_policy(&network_policy)
        .with_web_search_fixture_output("governed search result")
        .with_proposal_store(&proposal_store)
        .with_canonical_write_admission(
            &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
        )
        .with_agent_run_store(&owner_store)
        .with_tool_dispatch_observer(&observer);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "web.search".into(),
                    input: serde_json::json!({"arguments": {"query": "OpenLife"}}),
                    source_run_id: Some(owner_run.id.clone()),
                    step_index: 0,
                },
                &ctx,
            )
            .await
            .expect("explicit network allow keeps web search available");
        attach_result_to_owner(&owner_store, &mut owner_run, &result);

        assert_eq!(result.status, ActionExecutionStatus::Succeeded);
        assert_eq!(observer.count.load(Ordering::SeqCst), 1);
        assert_eq!(
            result.execution_receipt.dispatch_kind,
            crate::tool_execution_receipt::ToolDispatchKind::Simulated
        );
        assert_eq!(result.execution_receipt.dispatch_attempt_count, 1);
        assert!(proposal_store
            .list_pending_proposals(10)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn configured_brave_without_credential_blocks_before_fixture_or_network_dispatch() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let observer = CountingDispatchObserver::default();
        let network_policy = crate::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_network_policy(&network_policy)
        .with_web_search_fixture_output("must not bypass missing provider requirements")
        .with_tool_dispatch_observer(&observer);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig {
            search_provider: crate::agent::action_executor::helpers::SearchProviderConfig {
                provider: "brave".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .execute(
            AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "web.search".into(),
                input: serde_json::json!({"arguments": {"query": "OpenLife"}}),
                source_run_id: Some("run-search-config-blocked".into()),
                step_index: 0,
            },
            &ctx,
        )
        .await
        .expect("missing search credential is a typed blocker");

        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            result.stop_reason.as_deref(),
            Some("web_search_brave_credential_unavailable")
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
        assert_eq!(
            result.execution_receipt.dispatch_kind,
            crate::tool_execution_receipt::ToolDispatchKind::NotAttempted
        );
        assert_eq!(result.execution_receipt.dispatch_attempt_count, 0);
    }

    #[tokio::test]
    async fn empty_web_query_fails_before_fixture_or_network_dispatch() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let observer = CountingDispatchObserver::default();
        let (owner_store, mut owner_run) = create_tool_execution_owner("web.search");
        let network_policy = crate::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_network_policy(&network_policy)
        .with_web_search_fixture_output("must not be observed")
        .with_agent_run_store(&owner_store)
        .with_tool_dispatch_observer(&observer);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "web.search".into(),
                    input: serde_json::json!({"arguments": {"query": "   "}}),
                    source_run_id: Some(owner_run.id.clone()),
                    step_index: 0,
                },
                &ctx,
            )
            .await
            .expect("empty query is a typed preflight failure");
        attach_result_to_owner(&owner_store, &mut owner_run, &result);

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
        assert_eq!(
            result.execution_receipt.dispatch_kind,
            crate::tool_execution_receipt::ToolDispatchKind::NotAttempted
        );
        assert_eq!(result.execution_receipt.dispatch_attempt_count, 0);
    }

    #[tokio::test]
    async fn network_policy_tool_ask_and_allow_once_are_exact_and_single_use() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let observer = CountingDispatchObserver::default();
        let (owner_store, mut owner_run) = create_tool_execution_owner("web.search");
        let network_policy = crate::config::NetworkPolicy {
            default_decision: "allow".into(),
            tool_overrides: std::collections::HashMap::from([("web.search".into(), "ask".into())]),
            ..Default::default()
        };
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_network_policy(&network_policy)
        .with_web_search_fixture_output("governed search result")
        .with_proposal_store(&proposal_store)
        .with_canonical_write_admission(
            &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
        )
        .with_agent_run_store(&owner_store)
        .with_tool_dispatch_observer(&observer);
        let gateway = ToolGateway::from_executor_config(ActionExecutorConfig::default());
        let request = AgentActionRequest {
            action_type: "mcp_tool".into(),
            target: "web.search".into(),
            input: serde_json::json!({"arguments": {"query": "OpenLife"}}),
            source_run_id: Some(owner_run.id.clone()),
            step_index: 0,
        };

        let pending_result = gateway.execute(request.clone(), &ctx).await.unwrap();
        assert_eq!(
            pending_result.status,
            ActionExecutionStatus::NeedsConfirmation
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
        let pending = proposal_store.list_pending_proposals(10).unwrap();
        let scope = pending[0].after["canonical_scope"]["tool_name"]
            .as_str()
            .expect("network consent scope")
            .to_string();
        permission_store
            .grant(
                &scope,
                "network_policy",
                "medium",
                "network",
                crate::tool_permissions::ToolPermissionPolicy::AllowOnce,
                None,
            )
            .unwrap();

        let mut different_request = request.clone();
        different_request.input = serde_json::json!({
            "arguments": {"query": "a different external transmission"}
        });
        let different_result = gateway.execute(different_request, &ctx).await.unwrap();
        assert_eq!(
            different_result.status,
            ActionExecutionStatus::NeedsConfirmation,
            "an allow-once grant must be bound to the reviewed action digest"
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);

        let allowed_result = gateway.execute(request.clone(), &ctx).await.unwrap();
        assert_eq!(allowed_result.status, ActionExecutionStatus::Succeeded);
        attach_result_to_owner(&owner_store, &mut owner_run, &allowed_result);
        assert_eq!(observer.count.load(Ordering::SeqCst), 1);

        let consumed_result = gateway.execute(request, &ctx).await.unwrap();
        assert_eq!(
            consumed_result.status,
            ActionExecutionStatus::NeedsConfirmation
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn receipt_observer_tracks_the_same_gateway_owned_receipt_to_completion() {
        let mut registry = McpRegistry::new();
        registry.register_builtin(
            explicit_read_manifest(),
            Box::new(|_| Ok(serde_json::json!({"ok": true}).to_string())),
        );
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let safe_paths: Vec<String> = Vec::new();
        let (owner_store, mut owner_run) = create_tool_execution_owner("notes.read");
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &safe_paths,
        )
        .with_agent_run_store(&owner_store);
        let observed_tracker = Arc::new(Mutex::new(None));
        let observer_slot = Arc::clone(&observed_tracker);

        let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
            .execute_with_receipt_registration_sink(
                AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "notes.read".into(),
                    input: serde_json::json!({}),
                    source_run_id: Some(owner_run.id.clone()),
                    step_index: 0,
                },
                &ctx,
                move |tracker| {
                    *observer_slot.lock().unwrap() = Some(tracker);
                },
            )
            .await
            .expect("observed gateway read executes");
        attach_result_to_owner(&owner_store, &mut owner_run, &result);

        let tracked = observed_tracker
            .lock()
            .unwrap()
            .as_ref()
            .expect("observer receives tracker before execution")
            .snapshot();
        assert_eq!(tracked, result.execution_receipt);
        assert_eq!(
            tracked.transport_status,
            ToolTransportStatus::ResponseObserved
        );
    }

    #[tokio::test]
    async fn timeout_after_mcp_flush_is_failed_with_remote_effect_unknown_receipt() {
        let script = r#"
import json, sys, time
for line in sys.stdin:
    message = json.loads(line)
    method = message.get('method')
    if method == 'initialize':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'protocolVersion':'2024-11-05','capabilities':{}}}), flush=True)
    elif method == 'tools/list':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'tools':[{'name':'slow.effect','description':'slow external effect','parameters':{'type':'object'}}]}}), flush=True)
    elif method == 'tools/call':
        time.sleep(5)
"#;
        let manifest = ToolManifest {
            id: "mcp:slow:slow.effect".into(),
            name: "slow.effect".into(),
            description: "Slow external effect.".into(),
            parameters: serde_json::json!({"type": "object"}),
            permission_level: "high".into(),
            risk_level: "high".into(),
            version: "1.0.0".into(),
            source: ToolSource::Mcp {
                server_name: "slow".into(),
            },
            capabilities: vec!["external_side_effect".into()],
            requires_confirmation: true,
            enabled: true,
            declarative_only: false,
            action_type: "external_side_effect".into(),
            idempotency_contract: ToolIdempotencyContract::NonIdempotent,
            tags: vec!["typed_contract".into()],
        };
        let mut registry = McpRegistry::new();
        registry
            .register_with_env_and_manifests(
                "slow",
                "python3",
                &["-u", "-c", script],
                &std::collections::HashMap::new(),
                vec![manifest],
            )
            .await
            .expect("register bounded slow MCP fixture");
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "slow.effect",
                "mcp:slow",
                "high",
                "external_side_effect",
                crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::new();
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        );

        let result = ToolGateway::from_executor_config(ActionExecutorConfig {
            timeout_seconds: 1,
            ..Default::default()
        })
        .execute(
            AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "slow.effect".into(),
                input: serde_json::json!({"arguments": {}}),
                source_run_id: Some("run-gateway-timeout".into()),
                step_index: 0,
            },
            &ctx,
        )
        .await
        .expect("gateway timeout is a typed terminal result");

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert_eq!(result.stop_reason.as_deref(), Some("tool_gateway_timeout"));
        assert_eq!(
            result.execution_receipt.transport_status,
            ToolTransportStatus::RemoteUnknown
        );
        assert_eq!(
            result.execution_receipt.dispatch_kind,
            crate::tool_execution_receipt::ToolDispatchKind::McpStdio
        );
        assert_eq!(result.execution_receipt.dispatch_attempt_count, 1);
        assert_eq!(
            result.execution_receipt.effect_status,
            ToolEffectStatus::Unknown
        );
        assert!(result.execution_receipt.dispatched_at.is_some());
        assert!(result.execution_receipt.response_observed_at.is_none());
        assert!(result
            .execution_receipt
            .mechanically_valid_terminal()
            .is_ok());
        assert!(!result.execution_receipt.automatic_retry_safe());
    }
}
