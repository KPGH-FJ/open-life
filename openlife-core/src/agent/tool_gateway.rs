use crate::agent::action_executor::helpers::{is_proposal_generation_tool, normalize_tool_name};
use crate::agent::action_executor::ActionExecutor;
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
        Ok(result)
    }
}

fn validate_gateway_request_contract(
    request: &AgentActionRequest,
    ctx: &ActionExecutionContext<'_>,
) -> std::result::Result<ToolGatewayContractEvidence, String> {
    match request.action_type.as_str() {
        "memory_search" | "session_search" => {
            request
                .source_run_id
                .as_deref()
                .filter(|run_id| !run_id.trim().is_empty())
                .ok_or_else(|| "internal_read_canonical_run_identity_missing".to_string())?;
            let contract = internal_read_contract(&request.action_type, &request.target)?;
            if ctx.bound_content_receipt_issuer.is_none() {
                return Err("bound_content_receipt_issuer_unavailable".into());
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
    if matches!(manifest.source, ToolSource::Plugin { .. }) {
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
        tool_trace: None,
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
        tool_trace: None,
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
    // cross the canonical tool loop in-process. The JSON copies below are display/audit
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
