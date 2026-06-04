use crate::agent::action_executor::helpers::is_proposal_generation_tool;
use crate::agent::strategy_runtime::RuntimeStrategyRegistry;
use crate::mcp::McpRegistry;
use crate::tool_manifest::{ToolManifest, ToolSource};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactBetaReadinessComponentOverride {
    #[default]
    Current,
    Ready,
    Blocked(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactBetaExecutionReadinessInput {
    pub react_loop: ReactBetaReadinessComponentOverride,
    pub action_schema: ReactBetaReadinessComponentOverride,
    pub tool_registry: ReactBetaReadinessComponentOverride,
    pub action_executor_manifest_authority: ReactBetaReadinessComponentOverride,
    pub agent_run_trace: ReactBetaReadinessComponentOverride,
    pub permission_replay: ReactBetaReadinessComponentOverride,
    pub proposal_first_writes: ReactBetaReadinessComponentOverride,
    pub runs_trace_surface: ReactBetaReadinessComponentOverride,
    pub runtime_strategy: ReactBetaReadinessComponentOverride,
    pub default_chat_isolation: ReactBetaReadinessComponentOverride,
}

impl ReactBetaExecutionReadinessInput {
    pub fn current() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactBetaExecutionReadinessReport {
    pub report_kind: String,
    pub ready: bool,
    pub react_loop_present: bool,
    pub action_schema_ready: bool,
    pub tool_registry_ready: bool,
    pub action_executor_manifest_authority_ready: bool,
    pub agent_run_trace_ready: bool,
    pub permission_replay_ready: bool,
    pub proposal_first_writes_ready: bool,
    pub runs_trace_surface_ready: bool,
    pub default_chat_unchanged: bool,
    pub migration_permission: bool,
    pub runtime_strategy_ready: bool,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe: bool,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRegistryBetaToolReport {
    pub tool_id: String,
    pub required_state: String,
    pub actual_state: String,
    pub ready: bool,
    pub executable: bool,
    pub source: Option<String>,
    pub risk_level: Option<String>,
    pub action_type: Option<String>,
    pub capabilities: Vec<String>,
    pub proposal_type: Option<String>,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRegistryBetaReadinessReport {
    pub report_kind: String,
    pub ready: bool,
    pub metadata_safe: bool,
    pub required_tool_ids: Vec<String>,
    pub tools: Vec<ToolRegistryBetaToolReport>,
    pub executable_read_tools: Vec<String>,
    pub proposal_only_tools: Vec<String>,
    pub permission_gated_tools: Vec<String>,
    pub disabled_or_declarative_only_tools: Vec<String>,
    pub unsupported_or_missing_tools: Vec<String>,
    pub unknown_tools_blocked: bool,
    pub plugin_tools_executable_without_executor: Vec<String>,
    pub calendar_email_proposal_tools_avoid_external_write_fallback: bool,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
}

impl ToolRegistryBetaReadinessReport {
    pub fn tool(&self, tool_id: &str) -> Option<&ToolRegistryBetaToolReport> {
        self.tools.iter().find(|tool| tool.tool_id == tool_id)
    }
}

pub fn evaluate_react_beta_execution_readiness() -> ReactBetaExecutionReadinessReport {
    evaluate_react_beta_execution_readiness_for_input(ReactBetaExecutionReadinessInput::current())
}

pub fn evaluate_react_beta_execution_readiness_for_input(
    input: ReactBetaExecutionReadinessInput,
) -> ReactBetaExecutionReadinessReport {
    let registry = McpRegistry::new();
    let registry_report = evaluate_tool_registry_beta_readiness(&registry);
    let runtime_report = RuntimeStrategyRegistry::fixed_readiness_report();
    let mut blocking_reasons = Vec::new();

    let react_loop_present = component_ready(
        "react_loop_missing",
        &input.react_loop,
        true,
        &mut blocking_reasons,
    );
    let action_schema_ready = component_ready(
        "action_schema_not_ready",
        &input.action_schema,
        true,
        &mut blocking_reasons,
    );
    let tool_registry_ready = component_ready(
        "tool_registry_not_ready",
        &input.tool_registry,
        registry_report.ready,
        &mut blocking_reasons,
    );
    if !registry_report.ready
        && matches!(
            input.tool_registry,
            ReactBetaReadinessComponentOverride::Current
        )
    {
        blocking_reasons.extend(registry_report.blocking_reasons.clone());
    }
    let action_executor_manifest_authority_ready = component_ready(
        "action_executor_manifest_authority_not_ready",
        &input.action_executor_manifest_authority,
        true,
        &mut blocking_reasons,
    );
    let agent_run_trace_ready = component_ready(
        "agent_run_trace_not_ready",
        &input.agent_run_trace,
        true,
        &mut blocking_reasons,
    );
    let permission_replay_ready = component_ready(
        "permission_replay_not_ready",
        &input.permission_replay,
        true,
        &mut blocking_reasons,
    );
    let proposal_first_writes_ready = component_ready(
        "proposal_first_writes_not_ready",
        &input.proposal_first_writes,
        true,
        &mut blocking_reasons,
    );
    let runs_trace_surface_ready = component_ready(
        "runs_trace_surface_not_ready",
        &input.runs_trace_surface,
        true,
        &mut blocking_reasons,
    );
    let runtime_strategy_ready = component_ready(
        "runtime_strategy_not_ready",
        &input.runtime_strategy,
        runtime_report.ready,
        &mut blocking_reasons,
    );
    if !runtime_report.ready
        && matches!(
            input.runtime_strategy,
            ReactBetaReadinessComponentOverride::Current
        )
    {
        blocking_reasons.extend(runtime_report.blocking_reasons.clone());
    }
    let default_chat_unchanged = component_ready(
        "default_chat_isolation_not_ready",
        &input.default_chat_isolation,
        true,
        &mut blocking_reasons,
    );

    blocking_reasons.sort();
    blocking_reasons.dedup();
    let ready = blocking_reasons.is_empty()
        && react_loop_present
        && action_schema_ready
        && tool_registry_ready
        && action_executor_manifest_authority_ready
        && agent_run_trace_ready
        && permission_replay_ready
        && proposal_first_writes_ready
        && runs_trace_surface_ready
        && runtime_strategy_ready
        && default_chat_unchanged;

    let metadata_safe_summary = json!({
        "reportKind": "react_beta_execution_readiness",
        "ready": ready,
        "toolRegistryReady": tool_registry_ready,
        "runtimeStrategyReady": runtime_strategy_ready,
        "defaultChatUnchanged": default_chat_unchanged,
        "migrationPermission": false,
        "blockingReasonCount": blocking_reasons.len(),
        "metadataSafe": true,
        "runtimeModelToolExecuted": false,
        "businessWrites": false,
    });

    ReactBetaExecutionReadinessReport {
        report_kind: "react_beta_execution_readiness".into(),
        ready,
        react_loop_present,
        action_schema_ready,
        tool_registry_ready,
        action_executor_manifest_authority_ready,
        agent_run_trace_ready,
        permission_replay_ready,
        proposal_first_writes_ready,
        runs_trace_surface_ready,
        default_chat_unchanged,
        migration_permission: false,
        runtime_strategy_ready,
        blocking_reasons,
        metadata_safe: true,
        metadata_safe_summary,
    }
}

pub fn evaluate_tool_registry_beta_readiness(
    registry: &McpRegistry,
) -> ToolRegistryBetaReadinessReport {
    let manifests = registry.list_manifests();
    let manifest_by_name = manifests
        .iter()
        .map(|manifest| (manifest.name.clone(), manifest))
        .collect::<BTreeMap<_, _>>();

    let required = required_beta_tools();
    let mut tools = Vec::new();
    let mut blocking_reasons = Vec::new();
    let mut executable_read_tools = BTreeSet::new();
    let mut proposal_only_tools = BTreeSet::new();
    let mut permission_gated_tools = BTreeSet::new();
    let mut disabled_or_declarative_only_tools = BTreeSet::new();
    let mut unsupported_or_missing_tools = BTreeSet::new();

    for (tool_id, required_state, proposal_type) in &required {
        let manifest = manifest_by_name.get(*tool_id).copied();
        let mut report = classify_tool(tool_id, required_state, *proposal_type, manifest);
        if !report.ready {
            blocking_reasons.extend(report.blocking_reasons.clone());
        }
        match report.actual_state.as_str() {
            "executable_read"
            | "safe_path_read"
            | "network_policy_gated_read"
            | "configured_read" => {
                executable_read_tools.insert(report.tool_id.clone());
            }
            "proposal_only" => {
                proposal_only_tools.insert(report.tool_id.clone());
            }
            "permission_gated" => {
                permission_gated_tools.insert(report.tool_id.clone());
            }
            "disabled" | "declarative_only" => {
                disabled_or_declarative_only_tools.insert(report.tool_id.clone());
            }
            "missing" | "unsupported" => {
                unsupported_or_missing_tools.insert(report.tool_id.clone());
            }
            _ => {}
        }
        report.blocking_reasons.sort();
        report.blocking_reasons.dedup();
        tools.push(report);
    }

    let plugin_tools_executable_without_executor = manifests
        .iter()
        .filter(|manifest| {
            matches!(manifest.source, ToolSource::Plugin { .. })
                && manifest.enabled
                && !manifest.declarative_only
        })
        .map(|manifest| manifest.name.clone())
        .collect::<Vec<_>>();
    for tool_id in &plugin_tools_executable_without_executor {
        blocking_reasons.push(format!("plugin_tool_executable_without_executor:{tool_id}"));
    }

    let calendar_email_proposal_tools_avoid_external_write_fallback = tools.iter().all(|tool| {
        !matches!(
            tool.tool_id.as_str(),
            "calendar.propose_event" | "email.propose_draft"
        ) || (tool.actual_state == "proposal_only"
            && tool.proposal_type.as_deref() != Some("external_write_action"))
    });
    if !calendar_email_proposal_tools_avoid_external_write_fallback {
        blocking_reasons.push("calendar_email_proposal_tools_external_write_fallback".into());
    }

    blocking_reasons.sort();
    blocking_reasons.dedup();
    let ready = blocking_reasons.is_empty();
    let required_tool_ids = required
        .iter()
        .map(|(tool_id, _, _)| (*tool_id).to_string())
        .collect::<Vec<_>>();
    let metadata_safe_summary = json!({
        "reportKind": "tool_registry_beta_readiness",
        "ready": ready,
        "requiredToolCount": required_tool_ids.len(),
        "executableReadToolCount": executable_read_tools.len(),
        "proposalOnlyToolCount": proposal_only_tools.len(),
        "permissionGatedToolCount": permission_gated_tools.len(),
        "disabledOrDeclarativeOnlyToolCount": disabled_or_declarative_only_tools.len(),
        "unsupportedOrMissingToolCount": unsupported_or_missing_tools.len(),
        "unknownToolsBlocked": true,
        "pluginToolsExecutableWithoutExecutorCount": plugin_tools_executable_without_executor.len(),
        "blockingReasonCount": blocking_reasons.len(),
        "metadataSafe": true,
    });

    ToolRegistryBetaReadinessReport {
        report_kind: "tool_registry_beta_readiness".into(),
        ready,
        metadata_safe: true,
        required_tool_ids,
        tools,
        executable_read_tools: executable_read_tools.into_iter().collect(),
        proposal_only_tools: proposal_only_tools.into_iter().collect(),
        permission_gated_tools: permission_gated_tools.into_iter().collect(),
        disabled_or_declarative_only_tools: disabled_or_declarative_only_tools
            .into_iter()
            .collect(),
        unsupported_or_missing_tools: unsupported_or_missing_tools.into_iter().collect(),
        unknown_tools_blocked: true,
        plugin_tools_executable_without_executor,
        calendar_email_proposal_tools_avoid_external_write_fallback,
        blocking_reasons,
        metadata_safe_summary,
    }
}

pub fn metadata_safe_value_digest(value: &Value) -> (usize, String) {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    metadata_safe_text_digest(&serialized)
}

pub fn metadata_safe_text_digest(text: &str) -> (usize, String) {
    let bytes = text.as_bytes();
    let hash = digest(&SHA256, bytes);
    let hex = hash
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    (bytes.len(), format!("sha256:{hex}"))
}

pub fn metadata_safe_value_preview(value: &Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    metadata_safe_text_preview(&serialized)
}

pub fn metadata_safe_text_preview(text: &str) -> String {
    format!("{} bytes redacted", text.len())
}

fn component_ready(
    default_blocker: &str,
    override_value: &ReactBetaReadinessComponentOverride,
    current_ready: bool,
    blocking_reasons: &mut Vec<String>,
) -> bool {
    match override_value {
        ReactBetaReadinessComponentOverride::Ready => true,
        ReactBetaReadinessComponentOverride::Blocked(reason) => {
            blocking_reasons.push(reason.clone());
            false
        }
        ReactBetaReadinessComponentOverride::Current => {
            if !current_ready {
                blocking_reasons.push(default_blocker.to_string());
            }
            current_ready
        }
    }
}

fn required_beta_tools() -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    vec![
        ("life_model.read", "executable_read", None),
        (
            "life_model.propose_patch",
            "proposal_only",
            Some("life_model_update"),
        ),
        ("goal.read", "executable_read", None),
        ("memory.search", "executable_read", None),
        (
            "memory.propose_write",
            "proposal_only",
            Some("memory_write"),
        ),
        (
            "memory.propose_archive",
            "proposal_only",
            Some("memory_archive"),
        ),
        ("proposal.list", "executable_read", None),
        ("agent_run.lookup", "executable_read", None),
        ("permission.check", "executable_read", None),
        (
            "permission.request",
            "proposal_only",
            Some("tool_permission"),
        ),
        ("permission.replay_action", "permission_gated", None),
        ("mcp.call_tool", "permission_gated", None),
        ("a2a.call_agent", "permission_gated", None),
        ("file.read", "safe_path_read", None),
        (
            "file.write_proposal",
            "proposal_only",
            Some("external_write_action"),
        ),
        ("web.search", "network_policy_gated_read", None),
        ("web.fetch", "network_policy_gated_read", None),
        ("calendar.read", "configured_read", None),
        (
            "calendar.propose_event",
            "proposal_only",
            Some("scheduled_task"),
        ),
        ("email.read", "declarative_only", None),
        ("email.propose_draft", "proposal_only", Some("data_export")),
        (
            "task.create_proposal",
            "proposal_only",
            Some("scheduled_task"),
        ),
    ]
}

fn classify_tool(
    tool_id: &str,
    required_state: &str,
    proposal_type: Option<&str>,
    manifest: Option<&ToolManifest>,
) -> ToolRegistryBetaToolReport {
    let Some(manifest) = manifest else {
        return ToolRegistryBetaToolReport {
            tool_id: tool_id.into(),
            required_state: required_state.into(),
            actual_state: "missing".into(),
            ready: false,
            executable: false,
            source: None,
            risk_level: None,
            action_type: None,
            capabilities: Vec::new(),
            proposal_type: proposal_type.map(str::to_string),
            blocking_reasons: vec![format!("required_tool_missing:{tool_id}")],
        };
    };

    let executable = manifest.enabled && !manifest.declarative_only;
    let actual_state = if !manifest.enabled {
        "disabled"
    } else if manifest.declarative_only {
        "declarative_only"
    } else if is_proposal_generation_tool(&manifest.name) || manifest.name == "permission.request" {
        "proposal_only"
    } else if matches!(
        manifest.name.as_str(),
        "mcp.call_tool" | "a2a.call_agent" | "permission.replay_action"
    ) {
        "permission_gated"
    } else if manifest.name == "file.read" {
        "safe_path_read"
    } else if matches!(manifest.name.as_str(), "web.search" | "web.fetch") {
        "network_policy_gated_read"
    } else if manifest.name == "calendar.read" {
        "configured_read"
    } else if manifest.action_type == "read" && manifest.risk_level == "low" {
        "executable_read"
    } else {
        "unsupported"
    };

    let mut blocking_reasons = Vec::new();
    let ready = actual_state == required_state;
    if !ready {
        blocking_reasons.push(format!(
            "tool_state_mismatch:{}:{}:{}",
            tool_id, required_state, actual_state
        ));
    }
    if required_state == "proposal_only"
        && matches!(manifest.action_type.as_str(), "external_side_effect")
    {
        blocking_reasons.push(format!("proposal_tool_direct_external_write:{tool_id}"));
    }
    if required_state == "declarative_only" && executable {
        blocking_reasons.push(format!("declarative_tool_executable:{tool_id}"));
    }

    ToolRegistryBetaToolReport {
        tool_id: tool_id.into(),
        required_state: required_state.into(),
        actual_state: actual_state.into(),
        ready: ready && blocking_reasons.is_empty(),
        executable,
        source: Some(manifest.source.to_string()),
        risk_level: Some(manifest.risk_level.clone()),
        action_type: Some(manifest.action_type.clone()),
        capabilities: manifest.capabilities.clone(),
        proposal_type: proposal_type.map(str::to_string),
        blocking_reasons,
    }
}
