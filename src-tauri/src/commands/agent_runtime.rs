use crate::AppState;
use openlife_core::agent::ReasoningTrace;
use openlife_core::agent::{
    behavior_checks_for_packet, AgentExecutionBudget, AgentRun, AgentRunError, AgentRunStatus,
    AgentRuntime, AgentTask, AgentTaskKind, ContextSummary, GovernanceDecisionKind,
    HSBehaviorCheckSummary, HSSelectionAudit, MultiStrategyRuntime, MultiStrategyRuntimeInput,
    MultiStrategyRuntimeOutput, MultiStrategyRuntimePayload, PlanExecutionOutput, PlanStepStatus,
    RedactionLevel, RuntimeInput, RuntimeStrategyKind,
};
use openlife_core::layer_router::Layer;
use openlife_core::llm::ChatMessage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiStrategyAgentPreviewInput {
    pub session_id: String,
    pub user_text: String,
    #[serde(default)]
    pub tools_prompt: String,
    #[serde(default)]
    pub allow_planning: bool,
    #[serde(default)]
    pub local_model_available: bool,
    #[serde(default)]
    pub layer: Option<String>,
    #[serde(default)]
    pub execution_budget: Option<MultiStrategyAgentPreviewExecutionBudgetInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiStrategyAgentPreviewExecutionBudgetInput {
    pub max_steps: Option<u32>,
    pub max_tool_calls: Option<u32>,
    pub timeout_seconds: Option<u64>,
    pub allow_cloud: Option<bool>,
    pub allow_writes: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiStrategyAgentPreviewOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub strategy_kind: String,
    pub payload_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<Value>,
    pub proposal_ids: Vec<String>,
    pub warnings: Vec<String>,
    pub metadata_safe_summary: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub governance_decision_kind: Option<String>,
}

#[tauri::command]
pub async fn run_multi_strategy_agent_preview(
    input: MultiStrategyAgentPreviewInput,
    state: State<'_, Arc<AppState>>,
) -> Result<MultiStrategyAgentPreviewOutput, String> {
    run_multi_strategy_agent_preview_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn run_multi_strategy_agent_preview_with_state(
    input: MultiStrategyAgentPreviewInput,
    state: &Arc<AppState>,
) -> Result<MultiStrategyAgentPreviewOutput, String> {
    let mut preview_run = new_preview_agent_run(&input.session_id);
    let preview_run_id = preview_run.id.clone();
    create_preview_run(state, &preview_run).await?;

    let result = execute_multi_strategy_agent_preview(input, state, &preview_run_id).await;

    let result = match result {
        Ok(result) => result,
        Err(error) => {
            fail_preview_run(state, &mut preview_run, &error).await;
            return Err(metadata_safe_preview_error(&error));
        }
    };

    let final_warnings = preview_output_warnings(&result.output, &result.warnings);
    let audit = preview_audit_summary(&result.output, &final_warnings);
    let mut output = map_preview_output(result.output, result.warnings);
    output.run_id = Some(preview_run_id);

    complete_preview_run(
        state,
        &mut preview_run,
        PreviewRunCompletion {
            audit,
            warnings: final_warnings,
            proposal_ids: output.proposal_ids.clone(),
            context_summary: result.context_summary,
            hs_selection_audit: result.hs_selection_audit,
            behavior_checks: result.behavior_checks,
        },
    )
    .await?;

    Ok(output)
}

struct PreviewExecutionResult {
    output: MultiStrategyRuntimeOutput,
    warnings: Vec<String>,
    context_summary: ContextSummary,
    hs_selection_audit: Option<HSSelectionAudit>,
    behavior_checks: Vec<HSBehaviorCheckSummary>,
}

struct PreviewRunCompletion {
    audit: Value,
    warnings: Vec<String>,
    proposal_ids: Vec<String>,
    context_summary: ContextSummary,
    hs_selection_audit: Option<HSSelectionAudit>,
    behavior_checks: Vec<HSBehaviorCheckSummary>,
}

async fn execute_multi_strategy_agent_preview(
    input: MultiStrategyAgentPreviewInput,
    state: &Arc<AppState>,
    preview_run_id: &str,
) -> Result<PreviewExecutionResult, String> {
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager
            .load()
            .map_err(|e| format!("failed to load LifeModel for preview runtime: {e}"))?
    };
    let scheduler = state.scheduler.lock().await.clone();
    let config = state.config.lock().await.clone();
    let layer = parse_preview_layer(input.layer.as_deref())?;
    let tools_prompt = if input.tools_prompt.trim().is_empty() {
        let registry = state.mcp_registry.lock().await;
        registry.tools_prompt()
    } else {
        input.tools_prompt.clone()
    };
    let (execution_budget, mut adapter_warnings) =
        preview_execution_budget(input.execution_budget.as_ref());
    let life_model_empty = life_model.is_effectively_empty();
    let used_tools_prompt = !tools_prompt.trim().is_empty();

    let task = AgentTask {
        kind: AgentTaskKind::Conversation,
        session_id: input.session_id.clone(),
        user_text: input.user_text.clone(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: input.user_text.clone(),
        }],
        layer,
    };
    let hs_packet = crate::build_chat_runtime_hs_packet(
        state,
        &task,
        &life_model,
        &tools_prompt,
        Some(preview_run_id.to_string()),
    )
    .await?;
    let hs_selection_audit = hs_packet.as_ref().map(|packet| packet.audit.clone());
    let behavior_checks = hs_packet
        .as_ref()
        .map(behavior_checks_for_packet)
        .unwrap_or_default();
    let runtime_input = RuntimeInput::from_agent_task(
        task,
        life_model.clone(),
        None,
        tools_prompt,
        hs_packet,
        execution_budget,
    );
    let runtime = AgentRuntime::new(life_model, scheduler, &config);
    let multi_strategy_runtime = MultiStrategyRuntime::new(runtime);
    let output = multi_strategy_runtime
        .execute(MultiStrategyRuntimeInput {
            runtime_input,
            allow_planning: input.allow_planning,
            local_model_available: input.local_model_available,
        })
        .await
        .map_err(|e| format!("multi-strategy preview runtime failed: {e}"))?;

    adapter_warnings.extend(output.warnings.clone());
    Ok(PreviewExecutionResult {
        output,
        warnings: adapter_warnings,
        context_summary: ContextSummary {
            life_model_empty,
            included_life_model_sections: Vec::new(),
            memory_hit_count: 0,
            memory_sources: Vec::new(),
            used_tools_prompt,
            redaction_applied: true,
            redaction_level: RedactionLevel::Strict,
        },
        hs_selection_audit,
        behavior_checks,
    })
}

fn preview_execution_budget(
    input: Option<&MultiStrategyAgentPreviewExecutionBudgetInput>,
) -> (AgentExecutionBudget, Vec<String>) {
    let mut budget = AgentExecutionBudget::default();
    let mut warnings = Vec::new();

    if let Some(input) = input {
        if let Some(max_steps) = input.max_steps {
            budget.max_steps = max_steps;
        }
        if let Some(max_tool_calls) = input.max_tool_calls {
            budget.max_tool_calls = max_tool_calls;
        }
        if let Some(timeout_seconds) = input.timeout_seconds {
            budget.timeout_seconds = timeout_seconds;
        }
        if let Some(allow_cloud) = input.allow_cloud {
            budget.allow_cloud = allow_cloud;
        }
        if input.allow_writes == Some(true) {
            warnings.push("preview runtime forces allowWrites=false".into());
        }
    }

    budget.allow_writes = false;
    (budget, warnings)
}

fn new_preview_agent_run(session_id: &str) -> AgentRun {
    let mut run = AgentRun::new_chat_run(session_id, "");
    run.user_input = None;
    run.reasoning_strategy = Some("multi_strategy_preview".into());
    run.output_preview = Some("Multi-strategy preview started".into());
    run.context_summary = Some(ContextSummary {
        life_model_empty: false,
        included_life_model_sections: Vec::new(),
        memory_hit_count: 0,
        memory_sources: Vec::new(),
        used_tools_prompt: false,
        redaction_applied: true,
        redaction_level: RedactionLevel::Strict,
    });
    run
}

async fn create_preview_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for preview runtime".to_string())?;
    let store = store_arc.lock().await;
    store
        .create_run(run)
        .map_err(|e| format!("failed to create preview AgentRun: {e}"))
}

async fn complete_preview_run(
    state: &Arc<AppState>,
    run: &mut AgentRun,
    completion: PreviewRunCompletion,
) -> Result<(), String> {
    run.status = AgentRunStatus::Completed;
    run.finished_at = Some(chrono::Utc::now());
    run.generated_proposals = completion.proposal_ids;
    run.warnings = completion.warnings;
    run.hs_selection_audit = completion.hs_selection_audit;
    run.behavior_checks = completion.behavior_checks;
    run.output_preview = Some(preview_output_label(&completion.audit));
    run.step_count = completion
        .audit
        .get("planStepCount")
        .and_then(|value| value.as_u64())
        .unwrap_or_default() as u32;
    run.tool_call_count = 0;
    run.context_summary = Some(completion.context_summary);
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(completion.audit),
        output: Some("multi_strategy_preview".into()),
        stable_steps: vec![
            "strategy_selection".into(),
            "governance_check".into(),
            "preview_payload".into(),
        ],
        ..ReasoningTrace::default()
    });

    update_preview_run(state, run).await
}

async fn fail_preview_run(state: &Arc<AppState>, run: &mut AgentRun, error: &str) {
    run.fail(AgentRunError {
        message: metadata_safe_preview_error(error),
        phase: "preview_runtime_failed".into(),
        recoverable: false,
    });
    run.user_input = None;
    run.reasoning_strategy = Some("multi_strategy_preview".into());
    let audit = json!({
        "previewRuntime": "multi_strategy",
        "status": "failed",
        "errorCode": preview_error_code(error),
        "metadataSafe": true,
    });
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(audit),
        output: Some("multi_strategy_preview_failed".into()),
        ..ReasoningTrace::default()
    });
    run.output_preview = Some("Multi-strategy preview failed".into());

    if let Err(e) = update_preview_run(state, run).await {
        log::warn!("[AgentRun] failed to update preview run after error: {}", e);
    }
}

async fn update_preview_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for preview runtime".to_string())?;
    let store = store_arc.lock().await;
    store
        .update_run(run)
        .map_err(|e| format!("failed to update preview AgentRun: {e}"))
}

fn metadata_safe_preview_error(error: &str) -> String {
    format!(
        "multi-strategy preview runtime failed: {}",
        preview_error_code(error)
    )
}

fn preview_error_code(error: &str) -> &'static str {
    if error.contains("unsupported preview runtime layer") {
        "invalid_preview_layer"
    } else if error.contains("failed to load LifeModel") {
        "lifemodel_load_failed"
    } else if error.contains("HS runtime packet build failed") {
        "hs_packet_build_failed"
    } else if error.contains("multi-strategy preview runtime failed") {
        "multi_strategy_runtime_failed"
    } else {
        "preview_runtime_failed"
    }
}

fn preview_output_label(audit: &Value) -> String {
    let strategy = audit
        .get("strategyKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let governance = audit
        .get("governanceDecisionKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    if audit
        .get("blocked")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        format!("Multi-strategy preview blocked: {strategy} / {governance}")
    } else {
        format!("Multi-strategy preview: {strategy} / {governance}")
    }
}

fn map_preview_output(
    output: openlife_core::agent::MultiStrategyRuntimeOutput,
    warnings: Vec<String>,
) -> MultiStrategyAgentPreviewOutput {
    let governance_decision_kind = output
        .selection
        .governance_decision
        .as_ref()
        .map(|decision| preview_governance_decision_kind(decision.kind).to_string());
    let strategy_kind = preview_strategy_kind(output.selection.kind).to_string();
    let metadata_safe_summary = output.selection.metadata_safe_summary.clone();

    match output.payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output) => MultiStrategyAgentPreviewOutput {
            run_id: runtime_output.run_id,
            strategy_kind,
            payload_kind: "react".into(),
            user_output: Some(runtime_output.user_output),
            plan: None,
            proposal_ids: runtime_output.proposal_ids,
            warnings: merge_warnings(warnings, runtime_output.warnings),
            metadata_safe_summary,
            governance_decision_kind,
        },
        MultiStrategyRuntimePayload::PlanExecute(plan_output) => MultiStrategyAgentPreviewOutput {
            run_id: None,
            strategy_kind,
            payload_kind: "planExecute".into(),
            user_output: None,
            plan: Some(metadata_safe_plan(&plan_output)),
            proposal_ids: Vec::new(),
            warnings,
            metadata_safe_summary,
            governance_decision_kind,
        },
        MultiStrategyRuntimePayload::Blocked => MultiStrategyAgentPreviewOutput {
            run_id: None,
            strategy_kind,
            payload_kind: "blocked".into(),
            user_output: None,
            plan: None,
            proposal_ids: Vec::new(),
            warnings,
            metadata_safe_summary,
            governance_decision_kind,
        },
    }
}

fn metadata_safe_plan(plan_output: &PlanExecutionOutput) -> Value {
    json!({
        "objective": plan_output.plan.objective,
        "steps": plan_output.plan.steps.iter().map(|step| {
            json!({
                "id": step.id,
                "title": step.title,
                "intent": step.intent,
                "toolName": step.tool_name,
                "actionKind": step.action_kind,
                "riskLevel": step.risk_level,
                "declaredWrite": step.declared_write,
            })
        }).collect::<Vec<_>>(),
        "traces": plan_output.traces.iter().map(|trace| {
            let policy_reason_code = trace
                .decision
                .metadata_safe_summary
                .get("policyReasonCode")
                .and_then(|value| value.as_str());
            json!({
                "stepId": trace.step_id,
                "status": trace.status,
                "decisionKind": trace.decision.kind,
                "riskLevel": trace.decision.risk_level,
                "policyReasonCode": policy_reason_code,
            })
        }).collect::<Vec<_>>(),
        "warnings": plan_output.warnings,
    })
}

fn preview_output_warnings(
    output: &MultiStrategyRuntimeOutput,
    adapter_warnings: &[String],
) -> Vec<String> {
    let mut warnings = adapter_warnings.to_vec();
    if let MultiStrategyRuntimePayload::ReAct(runtime_output) = &output.payload {
        warnings.extend(runtime_output.warnings.clone());
    }
    warnings
}

fn preview_audit_summary(output: &MultiStrategyRuntimeOutput, warnings: &[String]) -> Value {
    let strategy_kind = preview_strategy_kind(output.selection.kind);
    let payload_kind = preview_payload_kind(&output.payload);
    let metadata = &output.selection.metadata_safe_summary;
    let task_kind = metadata
        .get("taskKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let reason_code = metadata
        .get("reasonCode")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let risk_level = metadata
        .get("riskLevel")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let has_hs_packet = metadata
        .get("hasHsPacket")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let governance_policy_kind = output
        .selection
        .governance_decision
        .as_ref()
        .map(|decision| preview_governance_policy_kind(decision.kind))
        .unwrap_or("unknown");
    let governance_decision_kind = output
        .selection
        .governance_decision
        .as_ref()
        .map(|decision| preview_governance_decision_kind(decision.kind))
        .unwrap_or("unknown");
    let proposal_ids = preview_proposal_ids(&output.payload);
    let inner_run_id = match &output.payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output) => runtime_output.run_id.clone(),
        MultiStrategyRuntimePayload::PlanExecute(_) | MultiStrategyRuntimePayload::Blocked => None,
    };
    let plan_step_count = preview_plan_step_count(&output.payload);
    let plan_step_statuses = preview_plan_step_statuses(&output.payload);
    let write_control = preview_write_control(&output.payload);
    let blocked = matches!(output.payload, MultiStrategyRuntimePayload::Blocked);

    json!({
        "previewRuntime": "multi_strategy",
        "taskKind": task_kind,
        "strategyKind": strategy_kind,
        "payloadKind": payload_kind,
        "governanceDecisionKind": governance_decision_kind,
        "governancePolicyKind": governance_policy_kind,
        "reasonCode": reason_code,
        "riskLevel": risk_level,
        "hasHsPacket": has_hs_packet,
        "warnings": warnings,
        "proposalIds": proposal_ids,
        "planStepCount": plan_step_count,
        "planStepStatuses": plan_step_statuses,
        "blocked": blocked,
        "metadataSafe": true,
        "innerRunId": inner_run_id,
        "writeControl": write_control,
    })
}

fn preview_payload_kind(payload: &MultiStrategyRuntimePayload) -> &'static str {
    match payload {
        MultiStrategyRuntimePayload::ReAct(_) => "react",
        MultiStrategyRuntimePayload::PlanExecute(_) => "planExecute",
        MultiStrategyRuntimePayload::Blocked => "blocked",
    }
}

fn preview_proposal_ids(payload: &MultiStrategyRuntimePayload) -> Vec<String> {
    match payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output) => runtime_output.proposal_ids.clone(),
        MultiStrategyRuntimePayload::PlanExecute(_) | MultiStrategyRuntimePayload::Blocked => {
            Vec::new()
        }
    }
}

fn preview_plan_step_count(payload: &MultiStrategyRuntimePayload) -> usize {
    match payload {
        MultiStrategyRuntimePayload::PlanExecute(plan_output) => plan_output.plan.steps.len(),
        MultiStrategyRuntimePayload::ReAct(_) | MultiStrategyRuntimePayload::Blocked => 0,
    }
}

fn preview_plan_step_statuses(payload: &MultiStrategyRuntimePayload) -> Vec<String> {
    match payload {
        MultiStrategyRuntimePayload::PlanExecute(plan_output) => plan_output
            .traces
            .iter()
            .map(|trace| preview_plan_step_status(trace.status))
            .collect(),
        MultiStrategyRuntimePayload::ReAct(_) | MultiStrategyRuntimePayload::Blocked => Vec::new(),
    }
}

fn preview_write_control(payload: &MultiStrategyRuntimePayload) -> Value {
    match payload {
        MultiStrategyRuntimePayload::PlanExecute(plan_output) => {
            let declared_write_step_count = plan_output
                .plan
                .steps
                .iter()
                .filter(|step| step.declared_write)
                .count();
            let proposal_required_step_count = plan_output
                .traces
                .iter()
                .filter(|trace| trace.status == PlanStepStatus::RequiresProposal)
                .count();
            let blocked_step_count = plan_output
                .traces
                .iter()
                .filter(|trace| trace.status == PlanStepStatus::Blocked)
                .count();
            json!({
                "declaredWriteStepCount": declared_write_step_count,
                "proposalRequiredStepCount": proposal_required_step_count,
                "blockedStepCount": blocked_step_count,
            })
        }
        MultiStrategyRuntimePayload::ReAct(_) | MultiStrategyRuntimePayload::Blocked => json!({
            "declaredWriteStepCount": 0,
            "proposalRequiredStepCount": 0,
            "blockedStepCount": 0,
        }),
    }
}

fn preview_plan_step_status(status: PlanStepStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

fn merge_warnings(mut left: Vec<String>, right: Vec<String>) -> Vec<String> {
    left.extend(right);
    left
}

fn parse_preview_layer(layer: Option<&str>) -> Result<Layer, String> {
    match layer.map(str::trim).filter(|layer| !layer.is_empty()) {
        None => Ok(Layer::L2),
        Some("L1" | "l1" | "1") => Ok(Layer::L1),
        Some("L2" | "l2" | "2") => Ok(Layer::L2),
        Some("L3" | "l3" | "3") => Ok(Layer::L3),
        Some(other) => Err(format!("unsupported preview runtime layer: {other}")),
    }
}

fn preview_strategy_kind(kind: RuntimeStrategyKind) -> &'static str {
    match kind {
        RuntimeStrategyKind::ReAct => "react",
        RuntimeStrategyKind::PlanExecute => "planExecute",
    }
}

fn preview_governance_decision_kind(kind: GovernanceDecisionKind) -> &'static str {
    match kind {
        GovernanceDecisionKind::Allow => "allow",
        GovernanceDecisionKind::Block => "block",
        GovernanceDecisionKind::RequireProposal
        | GovernanceDecisionKind::RequireConfirmation
        | GovernanceDecisionKind::RequireLocalOnly => "warn",
    }
}

fn preview_governance_policy_kind(kind: GovernanceDecisionKind) -> &'static str {
    match kind {
        GovernanceDecisionKind::Allow => "allow",
        GovernanceDecisionKind::RequireProposal => "require_proposal",
        GovernanceDecisionKind::RequireConfirmation => "require_confirmation",
        GovernanceDecisionKind::RequireLocalOnly => "require_local_only",
        GovernanceDecisionKind::Block => "block",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::{AgentRun, AgentRunStatus, ProposalStore};
    use openlife_core::life_model::LifeModel;

    async fn preview_state() -> std::sync::Arc<crate::AppState> {
        let state = crate::test_utils::test_app_state();
        {
            let manager = state.life_model_manager.lock().await;
            manager.save(&LifeModel::default()).unwrap();
        }
        state
    }

    fn base_input(user_text: &str) -> MultiStrategyAgentPreviewInput {
        MultiStrategyAgentPreviewInput {
            session_id: "session-preview".into(),
            user_text: user_text.into(),
            tools_prompt: "Available tools: memory.search".into(),
            allow_planning: true,
            local_model_available: true,
            layer: None,
            execution_budget: None,
        }
    }

    async fn stored_preview_run(state: &Arc<crate::AppState>, run_id: &str) -> AgentRun {
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        store
            .get_run(run_id)
            .unwrap()
            .unwrap_or_else(|| panic!("missing preview run {run_id}"))
    }

    fn preview_audit(run: &AgentRun) -> &Value {
        run.reasoning_trace
            .as_ref()
            .and_then(|trace| trace.strategy_result.as_ref())
            .expect("preview run should persist metadata-safe audit")
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_executes_react_path() {
        let state = preview_state().await;

        let output = run_multi_strategy_agent_preview_with_state(
            base_input("What should I focus on today?"),
            &state,
        )
        .await
        .unwrap();

        assert_eq!(output.strategy_kind, "react");
        assert_eq!(output.payload_kind, "react");
        assert!(output.run_id.is_some());
        assert!(output.user_output.is_some());
        assert!(output.proposal_ids.is_empty());
        assert_eq!(output.governance_decision_kind.as_deref(), Some("allow"));

        let run = stored_preview_run(&state, output.run_id.as_deref().unwrap()).await;
        assert_eq!(run.status, AgentRunStatus::Completed);
        assert_eq!(run.user_input, None);
        assert_eq!(
            run.reasoning_strategy.as_deref(),
            Some("multi_strategy_preview")
        );
        let audit = preview_audit(&run);
        assert_eq!(audit["strategyKind"], "react");
        assert_eq!(audit["payloadKind"], "react");
        assert_eq!(audit["blocked"], false);
        assert_eq!(audit["metadataSafe"], true);
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_returns_plan_execute_payload_for_planning_intent() {
        let state = preview_state().await;

        let output = run_multi_strategy_agent_preview_with_state(
            base_input("Plan steps for my afternoon."),
            &state,
        )
        .await
        .unwrap();

        assert_eq!(output.strategy_kind, "planExecute");
        assert_eq!(output.payload_kind, "planExecute");
        assert!(output.run_id.is_some());
        assert!(output.user_output.is_none());
        assert!(output.plan.is_some());
        assert!(output.proposal_ids.is_empty());

        let run = stored_preview_run(&state, output.run_id.as_deref().unwrap()).await;
        assert_eq!(run.status, AgentRunStatus::Completed);
        assert_eq!(run.user_input, None);
        let audit = preview_audit(&run);
        assert_eq!(audit["strategyKind"], "planExecute");
        assert_eq!(audit["payloadKind"], "planExecute");
        assert_eq!(audit["planStepCount"], 1);
        assert_eq!(audit["planStepStatuses"][0], "executed");
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_returns_blocked_for_sensitive_local_only_without_local_model(
    ) {
        let state = preview_state().await;
        let mut input = base_input("Talk through a sensitive health topic about medication.");
        input.local_model_available = false;

        let output = run_multi_strategy_agent_preview_with_state(input, &state)
            .await
            .unwrap();

        assert_eq!(output.payload_kind, "blocked");
        assert!(output.run_id.is_some());
        assert!(output.user_output.is_none());
        assert_eq!(output.governance_decision_kind.as_deref(), Some("block"));

        let run = stored_preview_run(&state, output.run_id.as_deref().unwrap()).await;
        assert_eq!(run.status, AgentRunStatus::Completed);
        assert_eq!(run.user_input, None);
        let audit = preview_audit(&run);
        assert_eq!(audit["payloadKind"], "blocked");
        assert_eq!(audit["governanceDecisionKind"], "block");
        assert_eq!(audit["blocked"], true);
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_does_not_treat_broad_tools_prompt_as_write_intent() {
        let state = preview_state().await;
        let mut input = base_input("What should I focus on today?");
        input.tools_prompt =
            "Available tools: file.write, calendar.create_event, email.send".into();

        let output = run_multi_strategy_agent_preview_with_state(input, &state)
            .await
            .unwrap();

        assert_eq!(output.strategy_kind, "react");
        assert_eq!(output.payload_kind, "react");
        assert!(output.proposal_ids.is_empty());
        assert!(!output
            .metadata_safe_summary
            .to_string()
            .contains("calendar.create_event"));

        let run = stored_preview_run(&state, output.run_id.as_deref().unwrap()).await;
        let persisted = serde_json::to_string(&run).unwrap();
        assert!(!persisted.contains("calendar.create_event"));
        assert!(!persisted.contains("email.send"));
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_output_is_metadata_safe() {
        let state = preview_state().await;
        let mut input =
            base_input("Plan steps for Alice and alice@example.com before sending the full draft.");
        input.tools_prompt = "Available tools: email.send body payload and file.update".into();

        let output = run_multi_strategy_agent_preview_with_state(input, &state)
            .await
            .unwrap();
        let serialized = serde_json::to_string(&output).unwrap();

        assert!(!serialized.contains("Alice"));
        assert!(!serialized.contains("alice@example.com"));
        assert!(!serialized.contains("full draft"));
        assert!(!serialized.contains("email.send"));
        assert!(!serialized.contains("file.update"));

        let run = stored_preview_run(&state, output.run_id.as_deref().unwrap()).await;
        let persisted = serde_json::to_string(&run).unwrap();
        assert!(!persisted.contains("Alice"));
        assert!(!persisted.contains("alice@example.com"));
        assert!(!persisted.contains("full draft"));
        assert!(!persisted.contains("email.send"));
        assert!(!persisted.contains("file.update"));
        assert_eq!(run.user_input, None);
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_persists_failed_run_with_sanitized_error() {
        let state = preview_state().await;
        let mut input = base_input("raw user text for Alice alice@example.com");
        input.layer = Some("not-a-layer".into());

        let err = run_multi_strategy_agent_preview_with_state(input, &state)
            .await
            .unwrap_err();

        assert!(!err.contains("Alice"));
        assert!(!err.contains("alice@example.com"));

        let runs = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.list_runs_for_session("session-preview", 10).unwrap()
        };
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(run.status, AgentRunStatus::Failed);
        assert_eq!(run.user_input, None);
        let persisted = serde_json::to_string(run).unwrap();
        assert!(!persisted.contains("Alice"));
        assert!(!persisted.contains("alice@example.com"));
        assert!(persisted.contains("preview_runtime_failed"));
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_does_not_write_lifemodel_memory_or_proposals() {
        let state = preview_state().await;
        let proposal_store = ProposalStore::new_in_memory().unwrap();
        assert!(proposal_store
            .list_pending_proposals(10)
            .unwrap()
            .is_empty());

        let before_model = {
            let manager = state.life_model_manager.lock().await;
            manager.load().unwrap()
        };
        let before_messages = {
            let store = state.memory_store.lock().await;
            store.export_all_messages().unwrap()
        };

        let _ = run_multi_strategy_agent_preview_with_state(
            base_input("Create a reminder for tomorrow."),
            &state,
        )
        .await
        .unwrap();

        let after_model = {
            let manager = state.life_model_manager.lock().await;
            manager.load().unwrap()
        };
        let after_messages = {
            let store = state.memory_store.lock().await;
            store.export_all_messages().unwrap()
        };
        let pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();

        assert_eq!(before_model.metadata.version, after_model.metadata.version);
        assert_eq!(
            serde_json::to_string(&before_messages).unwrap(),
            serde_json::to_string(&after_messages).unwrap()
        );
        assert!(pending_proposals.is_empty());
    }
}
