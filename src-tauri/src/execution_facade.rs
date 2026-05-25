//! Tauri-side execution facade skeleton.
//!
//! This module is the convergence point for Tauri runtime assembly: mode
//! selection, AgentLoop configuration, PromptStack registry creation, and
//! governed ActionContext construction. Scheduled, replay, builder, and
//! calibration callers still use assembly helpers only; chat and stream chat
//! run through the facade outcome protocol.

use crate::types::{ToolCallResult, ToolCallStatus};
use crate::{execution_deps, AppState};
use openlife_core::agent::action_executor::{ActionContext, ActionExecutionResult};
use openlife_core::agent::agent_loop::{AgentLoopConfig, AgentRole, StreamingCallback};
use openlife_core::agent::execution_sandbox::ExecutionSandbox;
use openlife_core::agent::prompt_stack::PromptBlockRegistry;
use openlife_core::agent::types::{AgentLoopStatusUpdate, AgentRunStatus, AgentSpec};
use openlife_core::agent::{
    ActionExecutionStatus, AgentActionRequest, AgentLoop, AgentLoopResult, AgentRun, AgentTask,
};
use openlife_core::config::{AppConfig, NetworkPolicy};
use openlife_core::life_model::LifeModel;
use openlife_core::privacy::PrivacyEngine;
use openlife_core::scheduler::InferenceScheduler;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TauriAgentExecutionMode {
    /// Non-streaming conversational AgentLoop execution.
    Chat,
    /// Streaming conversational execution.
    StreamChat,
    /// Background scheduled/proactive task execution with planner restrictions.
    Scheduled,
    /// Direct tool execution without a model planning loop.
    ToolExecution,
    /// Replay of a previously blocked action.
    Replay,
    /// Interactive LifeModel builder path; not migrated in Batch B.
    Builder,
    /// LifeModel calibration path; not migrated in Batch B.
    Calibration,
}

pub struct TauriAgentExecutionInput {
    pub mode: TauriAgentExecutionMode,
    pub task: AgentTask,
    pub life_model: LifeModel,
    pub tools_prompt: String,
    pub privacy_engine: PrivacyEngine,
    /// Required for formal Agent execution modes. `None` fails closed before
    /// any model or tool call. Future non-Agent utility modes must document
    /// their exception explicitly before using `None`.
    pub agent_spec: Option<AgentSpec>,
    /// Required for Chat/Scheduled formal model calls. `None` fails closed so
    /// callers cannot silently fall back to an ad hoc or built-in prompt stack.
    pub prompt_registry: Option<PromptBlockRegistry>,
    /// Required for StreamChat. Kept explicit so non-streaming callers cannot
    /// accidentally carry a callback through unrelated modes.
    pub streaming_callback: Option<Arc<dyn StreamingCallback>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TauriExecutionFacadeErrorKind {
    Governance,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TauriExecutionFacadeError {
    pub kind: TauriExecutionFacadeErrorKind,
    pub message: String,
    pub run_id: Option<String>,
}

impl TauriExecutionFacadeError {
    pub fn governance(message: impl Into<String>) -> Self {
        Self {
            kind: TauriExecutionFacadeErrorKind::Governance,
            message: message.into(),
            run_id: None,
        }
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        Self {
            kind: TauriExecutionFacadeErrorKind::Runtime,
            message: message.into(),
            run_id: None,
        }
    }

    pub fn runtime_with_run_id(message: impl Into<String>, run_id: impl Into<String>) -> Self {
        Self {
            kind: TauriExecutionFacadeErrorKind::Runtime,
            message: message.into(),
            run_id: Some(run_id.into()),
        }
    }

    pub fn is_runtime(&self) -> bool {
        self.kind == TauriExecutionFacadeErrorKind::Runtime
    }
}

impl std::fmt::Display for TauriExecutionFacadeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TauriExecutionFacadeError {}

#[derive(Debug)]
pub struct TauriAgentExecutionOutcome {
    pub reply: String,
    pub run: AgentRun,
    pub fallback_used: bool,
    pub fallback_reason: Option<String>,
    pub warnings: Vec<String>,
    pub status_updates: Vec<AgentLoopStatusUpdate>,
}

pub struct TauriDirectToolExecutionInput {
    pub name: String,
    pub arguments: serde_json::Value,
    pub action_ctx: ActionContext,
    pub agent_spec: AgentSpec,
    pub network_policy: NetworkPolicy,
}

pub struct TauriDirectToolExecutionOutcome {
    pub tool_result: ToolCallResult,
    pub run_id: String,
}

pub struct TauriScheduledExecutionInput {
    pub task: AgentTask,
    pub app_state: Arc<AppState>,
    pub config: AppConfig,
    pub life_model: LifeModel,
    pub scheduler: InferenceScheduler,
    pub privacy_engine: PrivacyEngine,
    pub agent_spec: Option<AgentSpec>,
    pub network_policy: Option<NetworkPolicy>,
    pub prompt_registry: Option<PromptBlockRegistry>,
}

#[derive(Debug)]
pub struct TauriScheduledExecutionOutcome {
    pub run_id: String,
    pub output: String,
    pub result_preview: String,
    pub run: AgentRun,
    pub status_updates: Vec<AgentLoopStatusUpdate>,
}

impl TauriAgentExecutionOutcome {
    pub fn from_agent_loop_result(result: AgentLoopResult) -> Self {
        Self {
            reply: result.final_response,
            run: result.run,
            fallback_used: false,
            fallback_reason: None,
            warnings: Vec::new(),
            status_updates: result.status_updates,
        }
    }

    pub fn with_fallback(
        reply: String,
        run: AgentRun,
        reason: impl Into<String>,
        warnings: Vec<String>,
    ) -> Self {
        let reason = reason.into();
        Self {
            reply,
            run,
            fallback_used: true,
            fallback_reason: Some(reason),
            warnings,
            status_updates: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TauriRuntimeAssemblyConfig {
    pub mode: TauriAgentExecutionMode,
    pub loop_config: AgentLoopConfig,
    pub safe_paths: Vec<String>,
    pub calendar_ics_paths: Vec<String>,
    pub network_policy: NetworkPolicy,
    pub execution_sandbox: ExecutionSandbox,
}

pub fn build_prompt_registry() -> PromptBlockRegistry {
    PromptBlockRegistry::built_in()
}

pub async fn resolve_default_agent_spec_fail_closed(
    agent_spec_store: &Arc<tokio::sync::Mutex<openlife_core::agent::AgentSpecStore>>,
) -> Result<AgentSpec, String> {
    crate::commands::agent_spec::resolve_required_agent_spec(agent_spec_store, None)
        .await
        .map_err(|e| format!("AgentSpec resolution failed: {}", e))
}

pub fn build_runtime_assembly_config(
    cfg: &AppConfig,
    mode: TauriAgentExecutionMode,
    shutdown_notify: Arc<tokio::sync::Notify>,
) -> TauriRuntimeAssemblyConfig {
    let loop_config = match mode {
        TauriAgentExecutionMode::Scheduled => scheduled_loop_config(shutdown_notify),
        _ => execution_deps::build_loop_config(cfg, shutdown_notify),
    };
    let safe_paths = cfg.system.safe_paths.clone();
    let execution_sandbox =
        ExecutionSandbox::from_config(&cfg.system.execution_sandbox, &safe_paths);

    TauriRuntimeAssemblyConfig {
        mode,
        loop_config,
        safe_paths,
        calendar_ics_paths: cfg.system.calendar_ics_paths.clone(),
        network_policy: cfg.system.network_policy.clone(),
        execution_sandbox,
    }
}

pub fn build_governed_agent_loop(
    life_model: LifeModel,
    scheduler: InferenceScheduler,
    cfg: &AppConfig,
    assembly: &TauriRuntimeAssemblyConfig,
    event_store: &Option<Arc<openlife_core::agent::event_store::AgentRunEventStore>>,
) -> AgentLoop {
    let agent_runtime = openlife_core::agent::AgentRuntime::new(life_model, scheduler.clone(), cfg);
    let action_executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    execution_deps::build_agent_loop(
        agent_runtime,
        action_executor,
        &scheduler,
        assembly.loop_config.clone(),
        event_store,
    )
}

pub fn build_governed_action_context(
    state: &Arc<AppState>,
    assembly: &TauriRuntimeAssemblyConfig,
    life_model: Option<LifeModel>,
    memory_store: Option<Arc<tokio::sync::Mutex<openlife_core::memory::MemoryStore>>>,
    agent_spec: AgentSpec,
) -> ActionContext {
    execution_deps::assemble_action_context(
        state.mcp_registry.clone(),
        state.tool_permission_store.clone(),
        state.mcp_audit_store.clone(),
        state.privacy_engine.clone(),
        assembly.safe_paths.clone(),
        life_model,
        memory_store,
        assembly.calendar_ics_paths.clone(),
        assembly.network_policy.clone(),
        assembly.execution_sandbox.clone(),
        agent_spec,
        state.proposal_store.clone(),
        state.agent_run_store.clone(),
        state
            .agent_run_event_store
            .as_ref()
            .map(|es| (**es).clone()),
    )
}

pub async fn run_tauri_agent_task(
    agent_loop: &AgentLoop,
    action_ctx: &ActionContext,
    input: TauriAgentExecutionInput,
) -> Result<TauriAgentExecutionOutcome, TauriExecutionFacadeError> {
    match input.mode {
        TauriAgentExecutionMode::Chat => run_chat_mode(agent_loop, action_ctx, input).await,
        TauriAgentExecutionMode::StreamChat => {
            run_stream_chat_mode(agent_loop, action_ctx, input).await
        }
        mode => Err(TauriExecutionFacadeError::governance(format!(
            "ExecutionFacade mode {:?} is not migrated to run_tauri_agent_task yet",
            mode
        ))),
    }
}

pub async fn run_tauri_direct_tool_execution(
    input: TauriDirectToolExecutionInput,
) -> Result<TauriDirectToolExecutionOutcome, TauriExecutionFacadeError> {
    validate_direct_tool_execution(&input)?;

    let mut run = AgentRun::new_tool_execution_run(&input.name);
    let run_id = run.id.clone();
    let executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let request = AgentActionRequest {
        action_type: "mcp_tool".to_string(),
        target: input.name.clone(),
        input: serde_json::json!({ "arguments": input.arguments.clone() }),
        source_run_id: Some(run_id.clone()),
        step_index: 0,
    };

    let result = executor
        .execute(request, &input.action_ctx)
        .await
        .map_err(|e| TauriExecutionFacadeError::runtime(e.to_string()))?;

    run.actions.push(result.action.clone());
    run.observations.push(result.observation.clone());
    run.status = match result.status {
        ActionExecutionStatus::Succeeded => AgentRunStatus::Completed,
        _ => AgentRunStatus::Failed,
    };
    run.finished_at = Some(chrono::Utc::now());

    if let Some(ref store_arc) = input.action_ctx.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.create_run(&run) {
            log::error!("[AgentRun] 创建运行记录失败: {}", e);
        }
    }

    let tool_result =
        direct_tool_result_from_action_result(input.name, input.arguments, result, run_id.clone());

    Ok(TauriDirectToolExecutionOutcome {
        tool_result,
        run_id,
    })
}

pub async fn run_tauri_scheduled_execution(
    input: TauriScheduledExecutionInput,
) -> Result<TauriScheduledExecutionOutcome, TauriExecutionFacadeError> {
    let agent_spec = input.agent_spec.as_ref().ok_or_else(|| {
        TauriExecutionFacadeError::governance("AgentSpec is required for Scheduled execution")
    })?;
    let prompt_registry = input.prompt_registry.as_ref().ok_or_else(|| {
        TauriExecutionFacadeError::governance(
            "PromptBlockRegistry is required for Scheduled execution",
        )
    })?;
    let network_policy = input.network_policy.as_ref().ok_or_else(|| {
        TauriExecutionFacadeError::governance("NetworkPolicy is required for Scheduled execution")
    })?;

    let assembly = build_runtime_assembly_config(
        &input.config,
        TauriAgentExecutionMode::Scheduled,
        input.app_state.shutdown_notify.clone(),
    );
    if !network_policies_match(&assembly.network_policy, network_policy) {
        return Err(TauriExecutionFacadeError::governance(
            "NetworkPolicy mismatch for Scheduled execution",
        ));
    }

    let agent_loop = build_governed_agent_loop(
        input.life_model.clone(),
        input.scheduler.clone(),
        &input.config,
        &assembly,
        &input.app_state.agent_run_event_store,
    );
    let action_ctx = build_governed_action_context(
        &input.app_state,
        &assembly,
        Some(input.life_model.clone()),
        Some(input.app_state.memory_store.clone()),
        agent_spec.clone(),
    );

    let action_agent_spec = action_ctx.agent_spec.as_ref().ok_or_else(|| {
        TauriExecutionFacadeError::governance(
            "ActionContext AgentSpec is required for Scheduled execution",
        )
    })?;
    if action_agent_spec.id != agent_spec.id {
        return Err(TauriExecutionFacadeError::governance(format!(
            "AgentSpec mismatch: input={} action_context={}",
            agent_spec.id, action_agent_spec.id
        )));
    }
    if action_ctx.network_policy.is_none() {
        return Err(TauriExecutionFacadeError::governance(
            "NetworkPolicy is required for Scheduled execution",
        ));
    }

    let result = agent_loop
        .run(
            &input.task,
            &input.life_model,
            "",
            None,
            input.privacy_engine.clone(),
            agent_spec.privacy_policy,
            agent_spec,
            prompt_registry,
            &action_ctx,
        )
        .await
        .map_err(|e| TauriExecutionFacadeError::runtime(e.to_string()))?;

    persist_scheduled_agent_run(&input.app_state, &result.run).await;
    let result = scheduled_outcome_from_agent_loop_result(result)?;

    Ok(result)
}

async fn persist_scheduled_agent_run(state: &Arc<AppState>, run: &AgentRun) {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.create_run(run) {
            log::error!(
                "[ExecutionFacade] Failed to persist scheduled AgentRun {}: {}",
                run.id,
                e
            );
        }
    }
}

fn validate_direct_tool_execution(
    input: &TauriDirectToolExecutionInput,
) -> Result<(), TauriExecutionFacadeError> {
    let action_agent_spec = input.action_ctx.agent_spec.as_ref().ok_or_else(|| {
        TauriExecutionFacadeError::governance(
            "ActionContext AgentSpec is required for Direct Tool execution",
        )
    })?;
    if input.agent_spec.id != action_agent_spec.id {
        return Err(TauriExecutionFacadeError::governance(format!(
            "AgentSpec mismatch: input={} action_context={}",
            input.agent_spec.id, action_agent_spec.id
        )));
    }
    let action_network_policy = input.action_ctx.network_policy.as_ref().ok_or_else(|| {
        TauriExecutionFacadeError::governance("NetworkPolicy is required for Direct Tool execution")
    })?;
    if action_network_policy.enabled != input.network_policy.enabled
        || action_network_policy.default_decision != input.network_policy.default_decision
        || action_network_policy.domain_allowlist != input.network_policy.domain_allowlist
        || action_network_policy.domain_denylist != input.network_policy.domain_denylist
        || action_network_policy.tool_overrides != input.network_policy.tool_overrides
    {
        return Err(TauriExecutionFacadeError::governance(
            "NetworkPolicy mismatch for Direct Tool execution",
        ));
    }

    Ok(())
}

fn network_policies_match(left: &NetworkPolicy, right: &NetworkPolicy) -> bool {
    left.enabled == right.enabled
        && left.default_decision == right.default_decision
        && left.domain_allowlist == right.domain_allowlist
        && left.domain_denylist == right.domain_denylist
        && left.tool_overrides == right.tool_overrides
}

fn direct_tool_result_from_action_result(
    name: String,
    arguments: serde_json::Value,
    result: ActionExecutionResult,
    run_id: String,
) -> ToolCallResult {
    ToolCallResult {
        name,
        arguments: arguments.clone(),
        sanitized_arguments: Some(arguments),
        success: result.status == ActionExecutionStatus::Succeeded,
        output: result
            .action
            .output
            .as_ref()
            .and_then(|o| o.get("text").and_then(|t| t.as_str()).map(String::from)),
        error: result.action.error.clone(),
        permission_level: result
            .action
            .tool_scope
            .as_ref()
            .map(|s| s.risk_level.clone())
            .unwrap_or_else(|| "medium".into()),
        status: match result.status {
            ActionExecutionStatus::Succeeded => ToolCallStatus::Success,
            ActionExecutionStatus::Failed => ToolCallStatus::Error,
            ActionExecutionStatus::Blocked => ToolCallStatus::Blocked,
            ActionExecutionStatus::NeedsConfirmation => ToolCallStatus::NeedsConfirmation,
        },
        requires_confirmation: result.status == ActionExecutionStatus::NeedsConfirmation,
        pii_found: false,
        privacy_warnings: Vec::new(),
        action_id: Some(result.action.id),
        run_id: Some(run_id),
        permission_decision: result.action.permission_decision,
    }
}

async fn run_chat_mode(
    agent_loop: &AgentLoop,
    action_ctx: &ActionContext,
    input: TauriAgentExecutionInput,
) -> Result<TauriAgentExecutionOutcome, TauriExecutionFacadeError> {
    let (agent_spec, prompt_registry) =
        validate_formal_agent_execution("Chat", action_ctx, &input)?;

    let result = agent_loop
        .run(
            &input.task,
            &input.life_model,
            &input.tools_prompt,
            None,
            input.privacy_engine.clone(),
            agent_spec.privacy_policy,
            agent_spec,
            prompt_registry,
            action_ctx,
        )
        .await
        .map_err(|e| TauriExecutionFacadeError::runtime(e.to_string()))?;

    outcome_from_agent_loop_result(result)
}

async fn run_stream_chat_mode(
    agent_loop: &AgentLoop,
    action_ctx: &ActionContext,
    input: TauriAgentExecutionInput,
) -> Result<TauriAgentExecutionOutcome, TauriExecutionFacadeError> {
    let callback = input
        .streaming_callback
        .as_ref()
        .ok_or_else(|| {
            TauriExecutionFacadeError::governance(
                "StreamingCallback is required for StreamChat execution",
            )
        })?
        .clone();
    let (agent_spec, prompt_registry) =
        validate_formal_agent_execution("StreamChat", action_ctx, &input)?;

    let result = agent_loop
        .run_streaming(
            &input.task,
            &input.life_model,
            &input.tools_prompt,
            None,
            input.privacy_engine.clone(),
            agent_spec.privacy_policy,
            agent_spec,
            prompt_registry,
            action_ctx,
            callback,
        )
        .await
        .map_err(|e| TauriExecutionFacadeError::runtime(e.to_string()))?;

    outcome_from_agent_loop_result(result)
}

fn validate_formal_agent_execution<'a>(
    mode_name: &str,
    action_ctx: &ActionContext,
    input: &'a TauriAgentExecutionInput,
) -> Result<(&'a AgentSpec, &'a PromptBlockRegistry), TauriExecutionFacadeError> {
    let agent_spec = input.agent_spec.as_ref().ok_or_else(|| {
        TauriExecutionFacadeError::governance(format!(
            "AgentSpec is required for {} execution",
            mode_name
        ))
    })?;
    let action_agent_spec = action_ctx.agent_spec.as_ref().ok_or_else(|| {
        TauriExecutionFacadeError::governance(format!(
            "ActionContext AgentSpec is required for {} execution",
            mode_name
        ))
    })?;
    if agent_spec.id != action_agent_spec.id {
        return Err(TauriExecutionFacadeError::governance(format!(
            "AgentSpec mismatch: input={} action_context={}",
            agent_spec.id, action_agent_spec.id
        )));
    }
    let prompt_registry = input.prompt_registry.as_ref().ok_or_else(|| {
        TauriExecutionFacadeError::governance(format!(
            "PromptBlockRegistry is required for {} execution",
            mode_name
        ))
    })?;
    if action_ctx.network_policy.is_none() {
        return Err(TauriExecutionFacadeError::governance(format!(
            "NetworkPolicy is required for {} execution",
            mode_name
        )));
    }

    Ok((agent_spec, prompt_registry))
}

fn outcome_from_agent_loop_result(
    result: AgentLoopResult,
) -> Result<TauriAgentExecutionOutcome, TauriExecutionFacadeError> {
    if result.run.status == AgentRunStatus::Failed {
        let message = result
            .run
            .error
            .as_ref()
            .map(|e| e.message.clone())
            .unwrap_or_else(|| "AgentLoop execution failed".to_string());
        return Err(TauriExecutionFacadeError::runtime(message));
    }

    Ok(TauriAgentExecutionOutcome::from_agent_loop_result(result))
}

fn scheduled_outcome_from_agent_loop_result(
    result: AgentLoopResult,
) -> Result<TauriScheduledExecutionOutcome, TauriExecutionFacadeError> {
    if result.run.status == AgentRunStatus::Failed {
        let run_id = result.run.id.clone();
        let message = result
            .run
            .error
            .as_ref()
            .map(|e| e.message.clone())
            .unwrap_or_else(|| "Scheduled AgentLoop execution failed".to_string());
        return Err(TauriExecutionFacadeError::runtime_with_run_id(
            message, run_id,
        ));
    }

    let run_id = result.run.id.clone();
    let output = result.final_response;
    let result_preview = result
        .run
        .output_preview
        .clone()
        .unwrap_or_else(|| output.chars().take(500).collect());

    Ok(TauriScheduledExecutionOutcome {
        run_id,
        output,
        result_preview,
        run: result.run,
        status_updates: result.status_updates,
    })
}

fn scheduled_loop_config(shutdown_notify: Arc<tokio::sync::Notify>) -> AgentLoopConfig {
    AgentLoopConfig {
        max_steps: 2,
        max_tool_calls: 4,
        timeout_seconds: 60,
        allow_writes: false,
        allow_cloud: true,
        shutdown_notify: Some(shutdown_notify),
        role: AgentRole::Planner,
        toolset_allowlist: vec![
            "goal.read".into(),
            "life_model.read".into(),
            "state.read".into(),
            "memory.search".into(),
            "proposal.create".into(),
        ],
        compaction_config: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use openlife_core::agent::types::AgentRunEventType;
    use openlife_core::agent::types::AgentSpec;
    use openlife_core::agent::AgentTaskKind;
    use openlife_core::config::NetworkPolicy;
    use openlife_core::layer_router::Layer;
    use openlife_core::llm::ChatMessage;
    use openlife_core::tool_permissions::ToolPermissionPolicy;

    struct TestStreamingCallback;

    #[async_trait]
    impl StreamingCallback for TestStreamingCallback {
        async fn on_chunk(&self, _chunk: &str, _step: u32, _phase: &str) {}
        async fn on_tool_start(&self, _tool_name: &str, _step: u32) {}
        async fn on_tool_result(&self, _tool_name: &str, _success: bool, _step: u32) {}
        async fn on_proposal(&self, _proposal_type: &str, _proposal_id: &str) {}
        async fn on_status(&self, _status: &str, _message: &str, _step: u32) {}
    }

    async fn chat_test_parts(
        max_steps: u32,
        prompt_registry: Option<PromptBlockRegistry>,
        agent_spec: Option<AgentSpec>,
    ) -> (
        Arc<AppState>,
        AgentLoop,
        ActionContext,
        TauriAgentExecutionInput,
    ) {
        let state = crate::test_utils::test_app_state();
        let mut cfg = state.config.lock().await.clone();
        cfg.system.agent_loop_max_steps = max_steps;
        let assembly = build_runtime_assembly_config(
            &cfg,
            TauriAgentExecutionMode::Chat,
            state.shutdown_notify.clone(),
        );
        let life_model = LifeModel::default();
        let scheduler = state.scheduler.lock().await.clone();
        let agent_loop = build_governed_agent_loop(
            life_model.clone(),
            scheduler,
            &cfg,
            &assembly,
            &state.agent_run_event_store,
        );
        let context_spec = agent_spec
            .clone()
            .unwrap_or_else(AgentSpec::default_main_spec);
        let action_ctx = build_governed_action_context(
            &state,
            &assembly,
            Some(life_model.clone()),
            Some(state.memory_store.clone()),
            context_spec,
        );
        let task = execution_deps::build_agent_task(
            AgentTaskKind::Conversation,
            "facade-chat-session".into(),
            "hello".into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "hello".into(),
            }],
            Layer::L2,
        );
        let input = TauriAgentExecutionInput {
            mode: TauriAgentExecutionMode::Chat,
            task,
            life_model,
            tools_prompt: String::new(),
            privacy_engine: PrivacyEngine::new(),
            agent_spec,
            prompt_registry,
            streaming_callback: None,
        };

        (state, agent_loop, action_ctx, input)
    }

    async fn scheduled_test_input(
        prompt_registry: Option<PromptBlockRegistry>,
        agent_spec: Option<AgentSpec>,
        network_policy: Option<NetworkPolicy>,
    ) -> TauriScheduledExecutionInput {
        scheduled_test_parts(prompt_registry, agent_spec, network_policy)
            .await
            .1
    }

    async fn scheduled_test_parts(
        prompt_registry: Option<PromptBlockRegistry>,
        agent_spec: Option<AgentSpec>,
        network_policy: Option<NetworkPolicy>,
    ) -> (Arc<AppState>, TauriScheduledExecutionInput) {
        let state = crate::test_utils::test_app_state();
        let mut cfg = state.config.lock().await.clone();
        if let Some(policy) = network_policy.clone() {
            cfg.system.network_policy = policy;
        }
        let life_model = LifeModel::default();
        let scheduler = state.scheduler.lock().await.clone();
        let task = execution_deps::build_agent_task(
            AgentTaskKind::Proactive,
            "scheduled-facade-session".into(),
            "scheduled prompt".into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "scheduled prompt".into(),
            }],
            Layer::L2,
        );

        let input = TauriScheduledExecutionInput {
            task,
            app_state: state.clone(),
            config: cfg,
            life_model,
            scheduler,
            privacy_engine: PrivacyEngine::new(),
            agent_spec,
            network_policy,
            prompt_registry,
        };

        (state, input)
    }

    #[tokio::test]
    async fn execution_facade_builds_action_context_with_agent_spec() {
        let state = crate::test_utils::test_app_state();
        let cfg = state.config.lock().await.clone();
        let assembly = build_runtime_assembly_config(
            &cfg,
            TauriAgentExecutionMode::Chat,
            state.shutdown_notify.clone(),
        );
        let spec = AgentSpec::default_main_spec();

        let ctx = build_governed_action_context(&state, &assembly, None, None, spec.clone());

        assert_eq!(ctx.agent_spec.as_ref().map(|s| &s.id), Some(&spec.id));
    }

    #[tokio::test]
    async fn execution_facade_builds_action_context_with_sandbox_from_config() {
        let state = crate::test_utils::test_app_state();
        let mut cfg = state.config.lock().await.clone();
        cfg.system.safe_paths = vec!["/system-safe".into()];
        cfg.system.execution_sandbox.safe_paths = vec!["/sandbox-safe".into()];
        cfg.system.execution_sandbox.bash_enabled = true;
        cfg.system.execution_sandbox.command_allowlist = vec!["pwd".into()];
        let assembly = build_runtime_assembly_config(
            &cfg,
            TauriAgentExecutionMode::Chat,
            state.shutdown_notify.clone(),
        );

        let ctx = build_governed_action_context(
            &state,
            &assembly,
            None,
            None,
            AgentSpec::default_main_spec(),
        );

        assert_eq!(ctx.execution_sandbox.safe_paths, vec!["/sandbox-safe"]);
        assert!(ctx.execution_sandbox.bash_enabled);
        assert_eq!(ctx.execution_sandbox.command_allowlist, vec!["pwd"]);
    }

    #[tokio::test]
    async fn execution_facade_non_stream_chat_matches_existing_agentloop_config() {
        let state = crate::test_utils::test_app_state();
        let mut cfg = state.config.lock().await.clone();
        cfg.system.agent_loop_max_steps = 7;
        cfg.system.agent_loop_max_tool_calls = 8;
        cfg.system.agent_loop_timeout_seconds = 99;
        let shutdown = state.shutdown_notify.clone();
        let existing = execution_deps::build_loop_config(&cfg, shutdown.clone());
        let assembly = build_runtime_assembly_config(&cfg, TauriAgentExecutionMode::Chat, shutdown);

        assert_eq!(assembly.loop_config.max_steps, existing.max_steps);
        assert_eq!(assembly.loop_config.max_tool_calls, existing.max_tool_calls);
        assert_eq!(
            assembly.loop_config.timeout_seconds,
            existing.timeout_seconds
        );
        assert_eq!(assembly.loop_config.allow_writes, existing.allow_writes);
        assert_eq!(assembly.loop_config.allow_cloud, existing.allow_cloud);
        assert_eq!(assembly.loop_config.role, existing.role);
        assert_eq!(
            assembly.loop_config.toolset_allowlist,
            existing.toolset_allowlist
        );
    }

    #[test]
    fn execution_facade_chat_path_uses_facade_entrypoint() {
        let source = include_str!("lib.rs");
        let start = source
            .find("async fn send_message_with_agent_loop_inner")
            .expect("chat inner entrypoint should exist");
        let end = source[start..]
            .find("pub(crate) async fn handle_agent_loop_fallback")
            .map(|offset| start + offset)
            .expect("fallback helper should follow chat inner entrypoint");
        let chat_path = &source[start..end];
        let direct_run_call = [".", "run("].concat();

        assert!(
            chat_path.contains("run_tauri_agent_task"),
            "non-stream Chat path must call the Tauri ExecutionFacade entrypoint"
        );
        assert!(
            !chat_path.contains(&direct_run_call),
            "non-stream Chat path must not call AgentLoop::run directly"
        );
    }

    #[test]
    fn execution_facade_stream_chat_path_uses_facade_entrypoint() {
        let source = include_str!("streaming.rs");
        let start = source
            .find("async fn start_stream_message_with_agent_loop")
            .expect("streaming AgentLoop entrypoint should exist");
        let end = source[start..]
            .find("fn should_fallback_from_execution_facade_error")
            .map(|offset| start + offset)
            .expect("streaming fallback helper should follow entrypoint");
        let stream_path = &source[start..end];
        let direct_streaming_call = [".", "run_streaming("].concat();

        assert!(
            stream_path.contains("run_tauri_agent_task"),
            "StreamChat path must call the Tauri ExecutionFacade entrypoint"
        );
        assert!(
            !stream_path.contains(&direct_streaming_call),
            "StreamChat path must not call AgentLoop::run_streaming directly"
        );
    }

    #[tokio::test]
    async fn scheduled_facade_preserves_restricted_toolset() {
        let state = crate::test_utils::test_app_state();
        let cfg = state.config.lock().await.clone();
        let assembly = build_runtime_assembly_config(
            &cfg,
            TauriAgentExecutionMode::Scheduled,
            state.shutdown_notify.clone(),
        );

        assert_eq!(assembly.loop_config.role, AgentRole::Planner);
        assert!(!assembly.loop_config.allow_writes);
        assert_eq!(assembly.loop_config.max_steps, 2);
        assert_eq!(assembly.loop_config.max_tool_calls, 4);
        assert_eq!(
            assembly.loop_config.toolset_allowlist,
            vec![
                "goal.read",
                "life_model.read",
                "state.read",
                "memory.search",
                "proposal.create",
            ]
        );
    }

    #[test]
    fn execution_facade_scheduled_path_uses_scheduled_wrapper() {
        let source = include_str!("scheduler_runner.rs");
        let start = source
            .find("async fn execute_scheduled_task")
            .expect("scheduled execution helper should exist");
        let end = source[start..]
            .find("// ── Tests")
            .map(|offset| start + offset)
            .expect("tests section should follow scheduled helper");
        let scheduled_path = &source[start..end];
        let direct_run_call = [".", "run("].concat();

        assert!(scheduled_path.contains("run_tauri_scheduled_execution"));
        assert!(!scheduled_path.contains(&direct_run_call));
        assert!(!scheduled_path.contains("run_tauri_agent_task"));
        assert!(!scheduled_path.contains("handle_agent_loop_fallback"));
    }

    #[tokio::test]
    async fn execution_facade_scheduled_assembly_carries_network_policy_and_sandbox() {
        let state = crate::test_utils::test_app_state();
        let mut cfg = state.config.lock().await.clone();
        cfg.system.network_policy = NetworkPolicy {
            enabled: true,
            default_decision: "deny".into(),
            domain_allowlist: vec!["example.com".into()],
            domain_denylist: vec!["blocked.example".into()],
            tool_overrides: std::collections::HashMap::new(),
        };
        cfg.system.safe_paths = vec!["/system-safe".into()];
        cfg.system.execution_sandbox.safe_paths = vec!["/sandbox-safe".into()];
        cfg.system.execution_sandbox.bash_enabled = false;
        cfg.system.execution_sandbox.command_allowlist = vec!["pwd".into()];

        let assembly = build_runtime_assembly_config(
            &cfg,
            TauriAgentExecutionMode::Scheduled,
            state.shutdown_notify.clone(),
        );
        let ctx = build_governed_action_context(
            &state,
            &assembly,
            None,
            None,
            AgentSpec::default_main_spec(),
        );

        assert_eq!(assembly.network_policy.default_decision, "deny");
        assert_eq!(
            ctx.network_policy
                .as_ref()
                .map(|policy| policy.default_decision.as_str()),
            Some("deny")
        );
        assert_eq!(
            ctx.network_policy
                .as_ref()
                .map(|policy| policy.domain_allowlist.as_slice()),
            Some(&["example.com".to_string()][..])
        );
        assert_eq!(ctx.execution_sandbox.safe_paths, vec!["/sandbox-safe"]);
        assert!(!ctx.execution_sandbox.bash_enabled);
        assert_eq!(ctx.execution_sandbox.command_allowlist, vec!["pwd"]);
    }

    #[tokio::test]
    async fn execution_facade_scheduled_mode_is_not_migrated_to_chat_task_entrypoint() {
        let (_state, agent_loop, action_ctx, mut input) = chat_test_parts(
            0,
            Some(build_prompt_registry()),
            Some(AgentSpec::default_main_spec()),
        )
        .await;
        input.mode = TauriAgentExecutionMode::Scheduled;

        let err = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();

        assert_eq!(err.kind, TauriExecutionFacadeErrorKind::Governance);
        assert!(
            err.to_string()
                .contains("Scheduled is not migrated to run_tauri_agent_task yet"),
            "Scheduled must remain assembly-only in this phase: {}",
            err
        );
    }

    #[test]
    fn execution_facade_replay_path_is_not_migrated_to_chat_task_entrypoint() {
        let source = include_str!("commands/agent.rs");
        let start = source
            .find("pub(crate) async fn replay_action_internal")
            .expect("replay internal entrypoint should exist");
        let end = source[start..]
            .find("#[tauri::command]\npub async fn list_agent_run_events")
            .map(|offset| start + offset)
            .expect("event listing command should follow replay command");
        let replay_path = &source[start..end];

        assert!(
            replay_path.contains("ActionExecutor::new"),
            "Replay remains on the direct ActionExecutor path during hardening preparation"
        );
        assert!(
            !replay_path.contains("run_tauri_agent_task"),
            "Replay must not use the Chat/StreamChat facade entrypoint"
        );
        assert!(
            !replay_path.contains("handle_agent_loop_fallback")
                && !replay_path.contains("FallbackStarted")
                && !replay_path.contains("FallbackCompleted"),
            "Replay must not inherit Chat fallback behavior"
        );
    }

    #[tokio::test]
    async fn scheduled_facade_requires_agent_spec() {
        let input = scheduled_test_input(
            Some(build_prompt_registry()),
            None,
            Some(NetworkPolicy::default()),
        )
        .await;

        let err = run_tauri_scheduled_execution(input).await.unwrap_err();

        assert_eq!(err.kind, TauriExecutionFacadeErrorKind::Governance);
        assert!(err.to_string().contains("AgentSpec is required"));
    }

    #[tokio::test]
    async fn scheduled_facade_requires_network_policy() {
        let input = scheduled_test_input(
            Some(build_prompt_registry()),
            Some(AgentSpec::default_main_spec()),
            None,
        )
        .await;

        let err = run_tauri_scheduled_execution(input).await.unwrap_err();

        assert_eq!(err.kind, TauriExecutionFacadeErrorKind::Governance);
        assert!(err.to_string().contains("NetworkPolicy is required"));
    }

    #[tokio::test]
    async fn scheduled_facade_governance_error_kind() {
        let mut mismatch = NetworkPolicy::default();
        mismatch.default_decision = "deny".into();
        let mut input = scheduled_test_input(
            Some(build_prompt_registry()),
            Some(AgentSpec::default_main_spec()),
            Some(NetworkPolicy::default()),
        )
        .await;
        input.config.system.network_policy = mismatch;

        let err = run_tauri_scheduled_execution(input).await.unwrap_err();

        assert_eq!(err.kind, TauriExecutionFacadeErrorKind::Governance);
        assert!(err.to_string().contains("NetworkPolicy mismatch"));
    }

    #[tokio::test]
    async fn scheduled_facade_runtime_error_kind() {
        let input = scheduled_test_input(
            Some(PromptBlockRegistry::new()),
            Some(AgentSpec::default_main_spec()),
            Some(NetworkPolicy::default()),
        )
        .await;

        let err = run_tauri_scheduled_execution(input).await.unwrap_err();

        assert_eq!(err.kind, TauriExecutionFacadeErrorKind::Runtime);
        assert!(err.to_string().contains("prompt stack error"));
        assert!(err.run_id.is_some());
    }

    #[tokio::test]
    async fn scheduled_facade_failed_run_persistence_on_runtime_failure() {
        let (state, input) = scheduled_test_parts(
            Some(PromptBlockRegistry::new()),
            Some(AgentSpec::default_main_spec()),
            Some(NetworkPolicy::default()),
        )
        .await;

        let err = run_tauri_scheduled_execution(input).await.unwrap_err();

        assert_eq!(err.kind, TauriExecutionFacadeErrorKind::Runtime);
        assert!(err.to_string().contains("prompt stack error"));
        let run_id = err
            .run_id
            .as_ref()
            .expect("Scheduled runtime failure should carry failed run id");

        let runs = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.list_runs(10, 0).unwrap()
        };
        let run = runs
            .iter()
            .find(|run| &run.id == run_id)
            .expect("failed scheduled run should be persisted");
        assert_eq!(run.status, AgentRunStatus::Failed);
        assert!(
            run.error
                .as_ref()
                .map(|err| err.message.contains("prompt stack error"))
                .unwrap_or(false),
            "failed run should keep readable error: {:?}",
            run.error
        );

        let event_store = state.agent_run_event_store.as_ref().unwrap();
        assert_eq!(
            event_store
                .count_events_by_type(AgentRunEventType::FallbackStarted)
                .unwrap(),
            0
        );
        assert_eq!(
            event_store
                .count_events_by_type(AgentRunEventType::FallbackCompleted)
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn scheduled_facade_prompt_stack_runtime_error_does_not_fallback() {
        let (state, input) = scheduled_test_parts(
            Some(PromptBlockRegistry::new()),
            Some(AgentSpec::default_main_spec()),
            Some(NetworkPolicy::default()),
        )
        .await;

        let err = run_tauri_scheduled_execution(input).await.unwrap_err();

        assert_eq!(err.kind, TauriExecutionFacadeErrorKind::Runtime);
        assert!(err.to_string().contains("prompt stack error"));
        let event_store = state.agent_run_event_store.as_ref().unwrap();
        assert_eq!(
            event_store
                .count_events_by_type(AgentRunEventType::FallbackStarted)
                .unwrap(),
            0
        );
        assert_eq!(
            event_store
                .count_events_by_type(AgentRunEventType::FallbackCompleted)
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn execution_facade_chat_returns_outcome() {
        let (_state, agent_loop, action_ctx, input) = chat_test_parts(
            0,
            Some(build_prompt_registry()),
            Some(AgentSpec::default_main_spec()),
        )
        .await;

        let outcome = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap();

        assert!(outcome.reply.contains("已达到最大执行步数"));
        assert_eq!(
            outcome.run.session_id.as_deref(),
            Some("facade-chat-session")
        );
        assert!(!outcome.fallback_used);
        assert!(outcome.fallback_reason.is_none());
        assert!(outcome.warnings.is_empty());
        assert!(!outcome.status_updates.is_empty());
    }

    #[tokio::test]
    async fn execution_facade_chat_requires_agent_spec() {
        let (_state, agent_loop, action_ctx, input) =
            chat_test_parts(0, Some(build_prompt_registry()), None).await;

        let err = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("AgentSpec is required"));
    }

    #[tokio::test]
    async fn execution_facade_chat_governance_error_kind() {
        let (_state, agent_loop, action_ctx, input) =
            chat_test_parts(0, Some(build_prompt_registry()), None).await;

        let missing_spec = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();
        assert_eq!(missing_spec.kind, TauriExecutionFacadeErrorKind::Governance);
        assert!(missing_spec.to_string().contains("AgentSpec is required"));

        let (_state, agent_loop, mut action_ctx, input) = chat_test_parts(
            1,
            Some(build_prompt_registry()),
            Some(AgentSpec::default_main_spec()),
        )
        .await;
        action_ctx.network_policy = None;
        let missing_policy = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();
        assert_eq!(
            missing_policy.kind,
            TauriExecutionFacadeErrorKind::Governance
        );
        assert!(missing_policy
            .to_string()
            .contains("NetworkPolicy is required"));
    }

    #[tokio::test]
    async fn execution_facade_chat_rejects_mismatched_agent_spec() {
        let mut input_spec = AgentSpec::default_main_spec();
        input_spec.id = "main.input".into();
        let mut context_spec = AgentSpec::default_main_spec();
        context_spec.id = "main.context".into();
        let (_state, agent_loop, mut action_ctx, input) =
            chat_test_parts(1, Some(PromptBlockRegistry::new()), Some(input_spec)).await;
        action_ctx.agent_spec = Some(context_spec);

        let err = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("AgentSpec mismatch"));
    }

    #[tokio::test]
    async fn execution_facade_chat_requires_network_policy() {
        let (_state, agent_loop, mut action_ctx, input) = chat_test_parts(
            1,
            Some(build_prompt_registry()),
            Some(AgentSpec::default_main_spec()),
        )
        .await;
        action_ctx.network_policy = None;

        let err = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("NetworkPolicy is required"));
    }

    #[tokio::test]
    async fn execution_facade_chat_uses_prompt_registry() {
        let (_state, agent_loop, action_ctx, input) = chat_test_parts(
            1,
            Some(PromptBlockRegistry::new()),
            Some(AgentSpec::default_main_spec()),
        )
        .await;

        let err = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("prompt stack error"),
            "unexpected error: {}",
            err
        );
        assert!(
            err.to_string().contains("unknown prompt block"),
            "unexpected error: {}",
            err
        );
    }

    #[tokio::test]
    async fn execution_facade_chat_runtime_error_kind() {
        let (_state, agent_loop, action_ctx, input) = chat_test_parts(
            1,
            Some(PromptBlockRegistry::new()),
            Some(AgentSpec::default_main_spec()),
        )
        .await;

        let err = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();

        assert_eq!(err.kind, TauriExecutionFacadeErrorKind::Runtime);
        assert!(err.to_string().contains("prompt stack error"));
    }

    #[tokio::test]
    async fn execution_facade_stream_chat_returns_outcome() {
        let (_state, agent_loop, action_ctx, mut input) = chat_test_parts(
            0,
            Some(build_prompt_registry()),
            Some(AgentSpec::default_main_spec()),
        )
        .await;
        input.mode = TauriAgentExecutionMode::StreamChat;
        input.streaming_callback = Some(Arc::new(TestStreamingCallback));

        let outcome = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap();

        assert!(outcome.reply.contains("已达到最大执行步数"));
        assert_eq!(
            outcome.run.session_id.as_deref(),
            Some("facade-chat-session")
        );
        assert!(!outcome.fallback_used);
        assert!(outcome.fallback_reason.is_none());
        assert!(outcome.warnings.is_empty());
        assert!(!outcome.status_updates.is_empty());
    }

    #[tokio::test]
    async fn execution_facade_stream_chat_rejects_mismatched_agent_spec() {
        let mut input_spec = AgentSpec::default_main_spec();
        input_spec.id = "stream.input".into();
        let mut context_spec = AgentSpec::default_main_spec();
        context_spec.id = "stream.context".into();
        let (_state, agent_loop, mut action_ctx, mut input) =
            chat_test_parts(1, Some(build_prompt_registry()), Some(input_spec)).await;
        input.mode = TauriAgentExecutionMode::StreamChat;
        input.streaming_callback = Some(Arc::new(TestStreamingCallback));
        action_ctx.agent_spec = Some(context_spec);

        let err = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("AgentSpec mismatch"));
    }

    #[tokio::test]
    async fn execution_facade_stream_chat_governance_error_kind() {
        let (_state, agent_loop, action_ctx, mut input) = chat_test_parts(
            1,
            Some(build_prompt_registry()),
            Some(AgentSpec::default_main_spec()),
        )
        .await;
        input.mode = TauriAgentExecutionMode::StreamChat;

        let missing_callback = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();
        assert_eq!(
            missing_callback.kind,
            TauriExecutionFacadeErrorKind::Governance
        );
        assert!(missing_callback
            .to_string()
            .contains("StreamingCallback is required"));

        let mut input_spec = AgentSpec::default_main_spec();
        input_spec.id = "stream.input".into();
        let mut context_spec = AgentSpec::default_main_spec();
        context_spec.id = "stream.context".into();
        let (_state, agent_loop, mut action_ctx, mut input) =
            chat_test_parts(1, Some(build_prompt_registry()), Some(input_spec)).await;
        input.mode = TauriAgentExecutionMode::StreamChat;
        input.streaming_callback = Some(Arc::new(TestStreamingCallback));
        action_ctx.agent_spec = Some(context_spec);

        let mismatch = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();
        assert_eq!(mismatch.kind, TauriExecutionFacadeErrorKind::Governance);
        assert!(mismatch.to_string().contains("AgentSpec mismatch"));
    }

    #[tokio::test]
    async fn execution_facade_stream_chat_requires_network_policy() {
        let (_state, agent_loop, mut action_ctx, mut input) = chat_test_parts(
            1,
            Some(build_prompt_registry()),
            Some(AgentSpec::default_main_spec()),
        )
        .await;
        input.mode = TauriAgentExecutionMode::StreamChat;
        input.streaming_callback = Some(Arc::new(TestStreamingCallback));
        action_ctx.network_policy = None;

        let err = run_tauri_agent_task(&agent_loop, &action_ctx, input)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("NetworkPolicy is required"));
    }

    #[tokio::test]
    async fn execution_facade_scheduled_mode_still_restricted() {
        let state = crate::test_utils::test_app_state();
        let mut cfg = state.config.lock().await.clone();
        cfg.system.agent_loop_max_steps = 99;
        cfg.system.agent_loop_max_tool_calls = 99;
        let assembly = build_runtime_assembly_config(
            &cfg,
            TauriAgentExecutionMode::Scheduled,
            state.shutdown_notify.clone(),
        );

        assert_eq!(assembly.loop_config.max_steps, 2);
        assert_eq!(assembly.loop_config.max_tool_calls, 4);
        assert!(!assembly.loop_config.allow_writes);
        assert!(assembly.loop_config.allow_cloud);
        assert_eq!(assembly.loop_config.role, AgentRole::Planner);
        assert_eq!(
            assembly.loop_config.toolset_allowlist,
            vec![
                "goal.read",
                "life_model.read",
                "state.read",
                "memory.search",
                "proposal.create",
            ]
        );
    }

    async fn direct_tool_test_context(
        state: &Arc<AppState>,
        network_policy: NetworkPolicy,
        spec: AgentSpec,
        life_model: Option<LifeModel>,
    ) -> ActionContext {
        let mut cfg = state.config.lock().await.clone();
        cfg.system.network_policy = network_policy;
        let assembly = build_runtime_assembly_config(
            &cfg,
            TauriAgentExecutionMode::ToolExecution,
            state.shutdown_notify.clone(),
        );
        build_governed_action_context(
            state,
            &assembly,
            life_model,
            Some(state.memory_store.clone()),
            spec,
        )
    }

    #[tokio::test]
    async fn direct_tool_facade_preserves_tool_result_shape() {
        let state = crate::test_utils::test_app_state();
        {
            let permissions = state.tool_permission_store.lock().await;
            permissions
                .grant(
                    "goal.read",
                    "builtin",
                    "low",
                    "read",
                    ToolPermissionPolicy::AllowUntilRevoked,
                    None,
                )
                .unwrap();
        }
        let spec = AgentSpec::default_main_spec();
        let network_policy = NetworkPolicy::default();
        let action_ctx = direct_tool_test_context(
            &state,
            network_policy.clone(),
            spec.clone(),
            Some(LifeModel::default()),
        )
        .await;

        let outcome = run_tauri_direct_tool_execution(TauriDirectToolExecutionInput {
            name: "goal.read".into(),
            arguments: serde_json::json!({}),
            action_ctx,
            agent_spec: spec,
            network_policy,
        })
        .await
        .unwrap();

        assert_eq!(outcome.tool_result.name, "goal.read");
        assert_eq!(outcome.tool_result.arguments, serde_json::json!({}));
        assert_eq!(
            outcome.tool_result.sanitized_arguments,
            Some(serde_json::json!({}))
        );
        assert!(outcome.tool_result.success);
        assert!(outcome.tool_result.output.is_some());
        assert!(outcome.tool_result.error.is_none());
        assert_eq!(outcome.tool_result.permission_level, "low");
        assert!(!outcome.tool_result.requires_confirmation);
        assert!(!outcome.tool_result.pii_found);
        assert!(outcome.tool_result.privacy_warnings.is_empty());
        assert!(outcome.tool_result.action_id.is_some());
        assert_eq!(outcome.tool_result.run_id, Some(outcome.run_id));
        assert!(outcome.tool_result.permission_decision.is_none());
    }

    #[tokio::test]
    async fn direct_tool_facade_sandbox_denial_does_not_fallback() {
        let state = crate::test_utils::test_app_state();
        {
            let mut registry = state.mcp_registry.lock().await;
            registry.set_builtin_manifest_enabled("shell.run", true);
        }
        let spec = AgentSpec::default_main_spec();
        let network_policy = NetworkPolicy::default();
        let action_ctx =
            direct_tool_test_context(&state, network_policy.clone(), spec.clone(), None).await;

        let outcome = run_tauri_direct_tool_execution(TauriDirectToolExecutionInput {
            name: "shell.run".into(),
            arguments: serde_json::json!({"command": "echo hi"}),
            action_ctx,
            agent_spec: spec,
            network_policy,
        })
        .await
        .unwrap();

        assert!(!outcome.tool_result.success);
        assert!(matches!(
            outcome.tool_result.status,
            crate::types::ToolCallStatus::Blocked
        ));
        assert!(
            outcome
                .tool_result
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("sandbox.bash_enabled = false"),
            "unexpected error: {:?}",
            outcome.tool_result.error
        );

        let runs = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.list_runs(10, 0).unwrap()
        };
        let run = runs
            .iter()
            .find(|run| run.id == outcome.run_id)
            .expect("direct tool run should be persisted");
        assert!(
            !run.warnings
                .iter()
                .any(|warning| warning.to_lowercase().contains("fallback")),
            "direct tool execution must not add fallback warnings: {:?}",
            run.warnings
        );
        let events = state
            .agent_run_event_store
            .as_ref()
            .unwrap()
            .list_events_by_run(&outcome.run_id)
            .unwrap();
        assert!(!events.iter().any(|event| {
            matches!(
                event.event_type,
                AgentRunEventType::FallbackStarted | AgentRunEventType::FallbackCompleted
            )
        }));
    }

    #[tokio::test]
    async fn direct_tool_facade_network_policy_denial_does_not_fallback() {
        let state = crate::test_utils::test_app_state();
        let policy = NetworkPolicy {
            default_decision: "deny".into(),
            ..NetworkPolicy::default()
        };
        let spec = AgentSpec::default_main_spec();
        let action_ctx = direct_tool_test_context(&state, policy.clone(), spec.clone(), None).await;

        let outcome = run_tauri_direct_tool_execution(TauriDirectToolExecutionInput {
            name: "web.search".into(),
            arguments: serde_json::json!({"query": "openlife"}),
            action_ctx,
            agent_spec: spec,
            network_policy: policy,
        })
        .await
        .unwrap();

        assert!(!outcome.tool_result.success);
        assert!(matches!(
            outcome.tool_result.status,
            crate::types::ToolCallStatus::Blocked
        ));
        assert!(
            outcome
                .tool_result
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("default_decision=deny"),
            "unexpected error: {:?}",
            outcome.tool_result.error
        );

        let runs = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.list_runs(10, 0).unwrap()
        };
        let run = runs
            .iter()
            .find(|run| run.id == outcome.run_id)
            .expect("direct tool run should be persisted");
        assert!(
            !run.warnings
                .iter()
                .any(|warning| warning.to_lowercase().contains("fallback")),
            "direct tool execution must not add fallback warnings: {:?}",
            run.warnings
        );
        let events = state
            .agent_run_event_store
            .as_ref()
            .unwrap()
            .list_events_by_run(&outcome.run_id)
            .unwrap();
        assert!(!events.iter().any(|event| {
            matches!(
                event.event_type,
                AgentRunEventType::FallbackStarted | AgentRunEventType::FallbackCompleted
            )
        }));
    }
}
