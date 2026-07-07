use crate::agent::action_executor::helpers::{is_proposal_generation_tool, normalize_tool_name};
use crate::agent::types::{AgentAction, AgentObservation};
use crate::agent::{
    ActionExecutionContext, ActionExecutionResult, ActionExecutionStatus, ActionExecutor,
    ActionExecutorConfig, AgentActionRequest,
};
use crate::tool_manifest::{ToolManifest, ToolSource};
use anyhow::Result;
use serde_json::Value;

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
}

pub struct ToolGateway {
    executor: ActionExecutor,
}

impl ToolGateway {
    pub fn new(executor: ActionExecutor) -> Self {
        Self { executor }
    }

    pub fn from_executor_config(config: ActionExecutorConfig) -> Self {
        Self::new(ActionExecutor::new(config))
    }

    pub fn execute(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
    ) -> Result<ActionExecutionResult> {
        let contract = match validate_gateway_request_contract(&request, ctx) {
            Ok(contract) => contract,
            Err(reason) => return Ok(blocked_gateway_result(request, &reason)),
        };

        let mut result = self.executor.execute(request, ctx)?;
        attach_gateway_contract_evidence(&mut result, &contract);
        Ok(result)
    }
}

fn validate_gateway_request_contract(
    request: &AgentActionRequest,
    ctx: &ActionExecutionContext<'_>,
) -> std::result::Result<ToolGatewayContractEvidence, String> {
    match request.action_type.as_str() {
        "memory_search" | "session_search" => Ok(internal_read_contract(&request.action_type)),
        "memory_write" | "memory_archive" | "life_model_patch" => {
            Ok(internal_proposal_contract(&request.action_type))
        }
        "mcp_tool" | "builtin_tool" | "plugin_tool" => {
            let tool_name = normalize_tool_name(&request.target, ctx.registry);
            let manifest = find_manifest(ctx, &tool_name)
                .ok_or_else(|| "tool_gateway_manifest_not_found".to_string())?;
            let contract = validate_manifest_execution_contract(&manifest)?;

            if manifest.name == "mcp.call_tool" {
                validate_mcp_target_contract(request, ctx)?;
            }

            Ok(contract)
        }
        _ => Err("tool_gateway_unsupported_action_type".into()),
    }
}

fn validate_mcp_target_contract(
    request: &AgentActionRequest,
    ctx: &ActionExecutionContext<'_>,
) -> std::result::Result<(), String> {
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
    validate_manifest_execution_contract(&target_manifest)?;
    Ok(())
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

    Ok(ToolGatewayContractEvidence {
        tool_name: manifest.name.clone(),
        manifest_id: manifest.id.clone(),
        source: manifest.source.to_string(),
        permission_level: manifest.permission_level.clone(),
        risk_level: manifest.risk_level.clone(),
        action_type: manifest.action_type.clone(),
        capabilities: manifest.capabilities.clone(),
        evidence_contract: evidence_contract_for_manifest(manifest),
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

fn internal_read_contract(action_type: &str) -> ToolGatewayContractEvidence {
    ToolGatewayContractEvidence {
        tool_name: action_type.into(),
        manifest_id: action_type.into(),
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
    }
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

fn blocked_gateway_result(request: AgentActionRequest, reason: &str) -> ActionExecutionResult {
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
            "status": "blocked",
            "toolGatewayAuthority": true,
            "blockerReason": reason,
            "directWritesExecuted": false,
        })),
        status: "blocked".into(),
        error: Some(reason.into()),
        permission_decision: Some(reason.into()),
        started_at: Some(now),
        finished_at: Some(now),
        timestamp: now,
        tool_scope: None,
        react_trace: None,
    };
    let observation = AgentObservation {
        id: observation_id,
        action_id: Some(action_id),
        content: format!("ToolGateway blocked execution: {reason}"),
        source: "tool_gateway".into(),
        structured_result: Some(serde_json::json!({
            "success": false,
            "status": "blocked",
            "toolGatewayAuthority": true,
            "blockerReason": reason,
            "directWritesExecuted": false,
        })),
        timestamp: now,
        react_trace: None,
    };
    ActionExecutionResult {
        action,
        observation,
        status: ActionExecutionStatus::Blocked,
        stop_reason: Some(reason.into()),
        governance_report: None,
    }
}

fn attach_gateway_contract_evidence(
    result: &mut ActionExecutionResult,
    contract: &ToolGatewayContractEvidence,
) {
    let evidence = serde_json::json!({
        "toolGatewayAuthority": true,
        "manifestId": contract.manifest_id,
        "toolName": contract.tool_name,
        "source": contract.source,
        "permissionLevel": contract.permission_level,
        "riskLevel": contract.risk_level,
        "actionType": contract.action_type,
        "capabilities": contract.capabilities,
        "evidenceContract": contract.evidence_contract,
        "inferredNameContractCredit": false,
    });
    if let Some(output) = result.action.output.as_mut() {
        if let Some(object) = output.as_object_mut() {
            object.insert("toolGateway".into(), evidence.clone());
        }
    }
    if let Some(structured) = result.observation.structured_result.as_mut() {
        if let Some(object) = structured.as_object_mut() {
            object.insert("toolGateway".into(), evidence);
        }
    } else {
        result.observation.structured_result = Some(serde_json::json!({
            "toolGateway": evidence,
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
            tags: vec![],
        }
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

    #[test]
    fn declarative_only_manifest_is_blocked_by_gateway_before_executor() {
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
        let ctx = ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &safe_paths,
        );
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
            .expect("gateway returns blocked result");

        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            result.stop_reason.as_deref(),
            Some("tool_gateway_manifest_declarative_only")
        );
        assert_eq!(result.observation.source, "tool_gateway");
    }
}
