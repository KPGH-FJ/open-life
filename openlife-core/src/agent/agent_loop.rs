use crate::agent::action_executor::{
    ActionExecutionContext, ActionExecutionStatus, AgentActionRequest,
};
use crate::agent::runtime::{AgentRuntime, AgentRuntimeOutput};
use crate::agent::tool_gateway::ToolGateway;
use crate::agent::types::{AgentObservation, AgentRun, AgentRunError, AgentRunStatus, AgentTask};
use crate::agent::{RuntimeInput, RuntimeOutput, RuntimePolicyContext};
use crate::layer::Layer;
#[cfg(test)]
use crate::llm::ProviderInvocationStatus;
use crate::llm::{
    BoundedContextBlock, ChatMessage, ContextManifest, PreparedProviderRequest,
    ProviderLocalOnlyReason, ProviderPayloadPurpose, ProviderPolicyAuthorization,
};
use crate::privacy::PrivacyEngine;
use crate::scheduler::{
    InferenceScheduler, PreparedProviderStream, PreparedProviderStreamEvent,
    PreparedProviderStreamTerminal, ProviderInvocationProgress, ScheduledInferenceScheduler,
};
use anyhow::Result;
use futures::StreamExt;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;

/// Configuration for the agent execution loop.
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub timeout_seconds: u64,
    pub allow_writes: bool,
    pub allow_cloud: bool,
    pub shutdown_notify: Option<Arc<tokio::sync::Notify>>,
    /// Specialized role for tool selection and system prompt tuning
    pub role: AgentRole,
    /// Optional restrict to specific tools (empty = all allowed)
    pub toolset_allowlist: Vec<String>,
    /// Optional restrict to exact governed action/target candidate pairs.
    /// When set, this is stricter than toolset_allowlist and is evaluated as
    /// action_type + target together.
    pub tool_action_allowlist: Vec<AgentLoopAllowedToolAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentLoopAllowedToolAction {
    pub action_type: String,
    pub target: String,
    /// Governed executor input for this exact candidate pair. When the model
    /// selects the pair, this replaces model-supplied arguments before
    /// ActionExecutor invocation.
    pub input: Value,
}

/// Specialization role for the agent loop.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AgentRole {
    /// Full tool access, standard conversation
    #[default]
    Generalist,
    /// Goal decomposition and weekly review focus
    Planner,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_steps: 4,
            max_tool_calls: 6,
            timeout_seconds: 90,
            allow_writes: true,
            allow_cloud: true,
            shutdown_notify: None,
            role: AgentRole::default(),
            toolset_allowlist: Vec::new(),
            tool_action_allowlist: Vec::new(),
        }
    }
}

impl AgentLoopConfig {
    /// Get planner-specific system instruction appended to tools prompt.
    pub fn role_system_instruction(&self) -> Option<&'static str> {
        match self.role {
            AgentRole::Generalist => None,
            AgentRole::Planner => Some(
                "You are in Planner mode. Focus on:\n\
                 - Decomposing goals into actionable steps\n\
                 - Identifying blockers and dependencies\n\
                 - Proposing schedule adjustments\n\
                 - Reading current short-lived state and Agent Memory before suggesting changes\n\
                 Use state.read, memory.search, and proposal.create tools.",
            ),
        }
    }
}

fn intersect_provider_authorizations(
    explicit: Option<ProviderPolicyAuthorization>,
    runtime_policy: ProviderPolicyAuthorization,
) -> ProviderPolicyAuthorization {
    match explicit {
        Some(explicit) if explicit.data_route() == crate::llm::ProviderDataRoute::LocalOnly => {
            explicit
        }
        Some(explicit)
            if runtime_policy.data_route() == crate::llm::ProviderDataRoute::LocalOnly =>
        {
            explicit.restrict_to_local(ProviderLocalOnlyReason::CanonicalRouteIntersection)
        }
        Some(explicit) => explicit,
        None => runtime_policy,
    }
}

/// Bundles the correlated runtime and policy inputs that flow through the
/// entire agent loop, reducing argument counts below clippy limits while
/// keeping canonical policy subject truth separate from compiled prompt text.
struct AgentLoopContext<'a> {
    pub task: &'a AgentTask,
    /// Canonical current-user text bound by the policy authority before any
    /// prompt-hardening transform. `task.user_text` may be wrapped or otherwise
    /// compiled for model consumption and must never be reused as policy truth.
    pub provider_subject_text: &'a str,
    pub tools_prompt: &'a str,
    pub memory_context: Option<String>,
    pub privacy_engine: PrivacyEngine,
    pub policy_context: RuntimePolicyContext,
    pub provider_authorization: Option<ProviderPolicyAuthorization>,
    pub network_policy: crate::config::NetworkPolicy,
}

/// Typed execution boundary for one AgentLoop invocation. Keeping these
/// correlated inputs together prevents observer-enabled product paths from
/// growing a parallel positional API as runtime facts are added.
pub struct AgentLoopRunRequest<'a> {
    task: &'a AgentTask,
    tools_prompt: &'a str,
    memory_context: Option<String>,
    privacy_engine: PrivacyEngine,
    action_ctx: &'a ActionExecutionContext<'a>,
    policy_context: RuntimePolicyContext,
    provider_authorization: Option<ProviderPolicyAuthorization>,
}

impl<'a> AgentLoopRunRequest<'a> {
    pub fn new(
        task: &'a AgentTask,
        tools_prompt: &'a str,
        memory_context: Option<String>,
        privacy_engine: PrivacyEngine,
        action_ctx: &'a ActionExecutionContext<'a>,
        policy_context: RuntimePolicyContext,
    ) -> Self {
        Self {
            task,
            tools_prompt,
            memory_context,
            privacy_engine,
            action_ctx,
            policy_context,
            provider_authorization: None,
        }
    }

    pub fn with_provider_authorization(
        mut self,
        authorization: ProviderPolicyAuthorization,
    ) -> Self {
        self.provider_authorization = Some(authorization);
        self
    }
}

/// Boundary markers for prompt injection defense.
/// Wrapped around user content to clearly delimit untrusted input from system instructions.
const USER_REQUEST_START: &str = "[BEGIN USER REQUEST]";
const USER_REQUEST_END: &str = "[END USER REQUEST]";

/// Wrap user content in boundary markers to mitigate prompt injection.
/// Affects messages with role == "user" and the standalone user_text field.
fn wrap_user_content(task: &mut AgentTask) {
    for msg in task.messages.iter_mut().filter(|m| m.role == "user") {
        msg.content = format!(
            "{}\n{}\n{}",
            USER_REQUEST_START, msg.content, USER_REQUEST_END
        );
    }
    if !task.user_text.is_empty() && !task.user_text.starts_with(USER_REQUEST_START) {
        task.user_text = format!(
            "{}\n{}\n{}",
            USER_REQUEST_START, task.user_text, USER_REQUEST_END
        );
    }
}

/// Callback trait for streaming agent loop execution.
/// Allows callers (e.g., Tauri shell) to receive real-time token chunks,
/// tool execution notifications, and status updates during AgentLoop execution.
#[async_trait::async_trait]
pub trait StreamingCallback: Send + Sync {
    /// A single token chunk from the model.
    async fn on_chunk(&self, chunk: &str, step: u32, phase: &str);
    /// A tool is about to be executed.
    async fn on_tool_start(&self, tool_name: &str, step: u32);
    /// A tool execution completed.
    async fn on_tool_result(&self, tool_name: &str, success: bool, step: u32);
    /// A proposal was generated.
    async fn on_proposal(&self, proposal_type: &str, proposal_id: &str);
    /// Status phase change.
    async fn on_status(&self, status: &str, message: &str, step: u32);
}

/// Result of running the agent loop.
#[derive(Clone)]
pub struct AgentLoopResult {
    pub run: AgentRun,
    pub final_response: String,
    pub stop_reason: String,
    pub terminal_disposition: AgentLoopTerminalDisposition,
    pub tool_call_count: u32,
    pub step_count: u32,
    pub status_updates: Vec<crate::agent::types::AgentLoopStatusUpdate>,
}

impl std::fmt::Debug for AgentLoopResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentLoopResult")
            .field("run_id", &self.run.id)
            .field("terminal_disposition", &self.terminal_disposition)
            .field("tool_call_count", &self.tool_call_count)
            .field("step_count", &self.step_count)
            .field("status_update_count", &self.status_updates.len())
            .field("final_response", &"[REDACTED]")
            .field("stop_reason", &"[REDACTED]")
            .finish()
    }
}

/// The one terminal classification consumed by persistence and product
/// projections. A tool failure must never be reinterpreted as a completed run
/// by a downstream adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentLoopTerminalDisposition {
    Succeeded,
    WaitingPermission,
    Failed,
    RemoteUnknown,
    Cancelled,
}

impl AgentLoopTerminalDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::WaitingPermission => "waiting_permission",
            Self::Failed => "failed",
            Self::RemoteUnknown => "remote_unknown",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_success(self) -> bool {
        self == Self::Succeeded
    }
}

fn classify_agent_loop_terminal(
    run_status: AgentRunStatus,
    stop_reason: &str,
) -> AgentLoopTerminalDisposition {
    match run_status {
        AgentRunStatus::Failed => AgentLoopTerminalDisposition::Failed,
        AgentRunStatus::RemoteUnknown => AgentLoopTerminalDisposition::RemoteUnknown,
        AgentRunStatus::Cancelled => AgentLoopTerminalDisposition::Cancelled,
        AgentRunStatus::WaitingPermission => AgentLoopTerminalDisposition::WaitingPermission,
        AgentRunStatus::Completed if matches!(stop_reason, "no_tools" | "no_observations") => {
            AgentLoopTerminalDisposition::Succeeded
        }
        AgentRunStatus::Running if matches!(stop_reason, "no_tools" | "no_observations") => {
            AgentLoopTerminalDisposition::Succeeded
        }
        AgentRunStatus::Running | AgentRunStatus::Completed => AgentLoopTerminalDisposition::Failed,
    }
}

/// Result of a single step in the agent loop.
#[derive(Debug, Clone)]
struct StepResult {
    pub stop_reason: String,
    pub final_response: String,
    pub should_continue: bool,
    pub tool_call_count_delta: u32,
    pub observations: Vec<AgentObservation>,
    pub status_updates: Vec<crate::agent::types::AgentLoopStatusUpdate>,
}

/// Context for executing a single step of the agent loop.
struct StepContext<'a> {
    pub task: &'a AgentTask,
    pub provider_subject_text: &'a str,
    pub tools_prompt: &'a str,
    pub memory_context: Option<String>,
    pub privacy_engine: PrivacyEngine,
    pub policy_context: RuntimePolicyContext,
    pub provider_authorization: Option<ProviderPolicyAuthorization>,
    pub action_ctx: &'a ActionExecutionContext<'a>,
    pub run: &'a mut AgentRun,
    pub tool_call_count: u32,
}

/// The AgentLoop executes a task with an iterative ReAct loop:
///
/// Each step: model generates response (with optional tool calls) →
///            tools are executed → observations are fed back →
///            model generates follow-up response.
///
/// The loop continues until stop_reason indicates completion, permission
/// required, an error, or max_steps/max_tool_calls are exhausted.
/// Configurable via AgentLoopConfig (max_steps default: 4, max_tool_calls default: 6).
enum AgentLoopProviderScheduler {
    General(InferenceScheduler),
    Scheduled(ScheduledInferenceScheduler),
}

#[cfg(test)]
impl AgentLoopProviderScheduler {
    fn provider_receipts_snapshot(&self) -> Vec<crate::llm::ProviderInvocationReceipt> {
        match self {
            Self::General(scheduler) => scheduler.provider_receipts_snapshot(),
            Self::Scheduled(scheduler) => scheduler.inner_provider_receipts_snapshot_for_test(),
        }
    }
}

pub struct AgentLoop {
    runtime: AgentRuntime,
    tool_gateway: ToolGateway,
    scheduler: AgentLoopProviderScheduler,
    config: AgentLoopConfig,
    scripted_replies: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
}

impl AgentLoop {
    pub fn new(
        runtime: AgentRuntime,
        tool_gateway: ToolGateway,
        scheduler: InferenceScheduler,
        config: AgentLoopConfig,
    ) -> Self {
        Self {
            runtime,
            tool_gateway,
            scheduler: AgentLoopProviderScheduler::General(scheduler),
            config,
            scripted_replies: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
        }
    }

    pub fn new_scheduled(
        runtime: AgentRuntime,
        tool_gateway: ToolGateway,
        scheduler: ScheduledInferenceScheduler,
        config: AgentLoopConfig,
    ) -> Self {
        Self {
            runtime,
            tool_gateway,
            scheduler: AgentLoopProviderScheduler::Scheduled(scheduler),
            config,
            scripted_replies: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
        }
    }

    pub(crate) fn with_scripted_replies(self, replies: Vec<String>) -> Self {
        Self {
            scripted_replies: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::VecDeque::from(replies),
            )),
            ..self
        }
    }

    async fn prepare_provider_request(
        &self,
        actx: &AgentLoopContext<'_>,
        messages: Vec<ChatMessage>,
    ) -> Result<PreparedProviderRequest> {
        let provider_authorization = intersect_provider_authorizations(
            actx.provider_authorization.clone(),
            actx.policy_context.provider_authorization().clone(),
        );
        let provider_authorization = if self.config.allow_cloud {
            provider_authorization
        } else {
            provider_authorization.restrict_to_local(ProviderLocalOnlyReason::CloudDisabled)
        };
        let context_blocks = (!actx.tools_prompt.trim().is_empty())
            .then(|| BoundedContextBlock {
                source_ref: "tool_gateway.manifest".into(),
                category: "typed_tool_contract".into(),
                content: actx.tools_prompt.to_string(),
            })
            .into_iter()
            .collect::<Vec<_>>();
        let selected_context_refs = context_blocks
            .iter()
            .map(|block| block.source_ref.clone())
            .collect::<Vec<_>>();
        let included_context_categories = context_blocks
            .iter()
            .map(|block| block.category.clone())
            .collect::<Vec<_>>();
        let policy_provenance_refs = actx.policy_context.policy_provenance_refs().to_vec();
        let provider_authorization = provider_authorization.authorize_derived_payload(
            ProviderPayloadPurpose::AgentLoopStep,
            actx.provider_subject_text,
            &messages,
            &context_blocks,
        )?;

        let context_manifest = ContextManifest {
            request_id: uuid::Uuid::new_v4().to_string(),
            privacy_decision_id: provider_authorization.decision_id().to_string(),
            selected_context_refs,
            included_context_categories,
            declared_payload_categories: vec![
                crate::llm::ProviderPayloadCategory::RuntimeCompiledMessages,
            ],
            policy_provenance_refs,
            raw_life_model_included: false,
            raw_unbounded_memory_included: false,
        };
        match &self.scheduler {
            AgentLoopProviderScheduler::General(scheduler) => {
                scheduler
                    .prepare_chat_request_with_authorization(
                        messages,
                        context_blocks,
                        context_manifest,
                        provider_authorization,
                        actx.network_policy.clone(),
                        !actx.tools_prompt.trim().is_empty(),
                    )
                    .await
            }
            AgentLoopProviderScheduler::Scheduled(scheduler) => {
                scheduler
                    .prepare_scheduled_chat_request(
                        messages,
                        context_blocks,
                        context_manifest,
                        provider_authorization,
                        actx.network_policy.clone(),
                        !actx.tools_prompt.trim().is_empty(),
                    )
                    .await
            }
        }
    }

    fn next_scripted_reply(&self) -> Option<String> {
        self.scripted_replies
            .lock()
            .ok()
            .and_then(|mut replies| replies.pop_front())
    }

    fn emit_status(
        &self,
        updates: &mut Vec<crate::agent::types::AgentLoopStatusUpdate>,
        phase: crate::agent::types::AgentLoopPhase,
        message: impl Into<String>,
        step_index: u32,
        tool_call_index: Option<u32>,
    ) {
        updates.push(crate::agent::types::AgentLoopStatusUpdate {
            phase,
            message: message.into(),
            step_index,
            tool_call_index,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Attempt a one-shot JSON self-repair when the model produces malformed JSON.
    /// Injects a bilingual, schema-first correction prompt and regenerates once.
    /// Records warnings on the run for observability.
    /// Returns a fresh ParsedAgentReply from the regenerated response, or the
    /// original `failed_parsed` (with `json_parse_failed: true`) if repair also fails.
    async fn try_json_self_repair(
        &self,
        actx: &AgentLoopContext<'_>,
        action_ctx: &ActionExecutionContext<'_>,
        run: &mut AgentRun,
        tool_call_count: &mut u32,
        provider_progress: &mut (dyn FnMut(ProviderInvocationProgress) -> Result<()> + Send),
    ) -> Result<ParsedAgentReply> {
        let mut repair_task = actx.task.clone();
        repair_task.messages.push(ChatMessage {
            role: "system".into(),
            content: format!("{}{}", SELF_REPAIR_PROMPT, actx.task.user_text),
        });

        let repair_actx = AgentLoopContext {
            task: &repair_task,
            provider_subject_text: actx.provider_subject_text,
            tools_prompt: actx.tools_prompt,
            memory_context: actx.memory_context.clone(),
            privacy_engine: actx.privacy_engine.clone(),
            policy_context: actx.policy_context.clone(),
            provider_authorization: actx.provider_authorization.clone(),
            network_policy: actx.network_policy.clone(),
        };

        match self
            .generate_response(&repair_actx, provider_progress)
            .await
        {
            Ok(repaired_gen) => {
                let parsed =
                    self.parse_agent_reply(&repaired_gen.reply, action_ctx, run, tool_call_count)?;
                if !parsed.json_parse_failed {
                    run.warnings
                        .push("JSON format self-repair succeeded".into());
                } else {
                    run.warnings.push(
                        "JSON format self-repair also failed, continuing with raw reply".into(),
                    );
                }
                Ok(parsed)
            }
            Err(e) => {
                run.warnings
                    .push(format!("JSON format self-repair generation failed: {}", e));
                Ok(ParsedAgentReply {
                    final_text: "[self-repair failed]".into(),
                    actions: Vec::new(),
                    json_parse_failed: true,
                })
            }
        }
    }

    /// Core agent loop implementation shared by `run` and `run_streaming`.
    /// Differences are handled via the optional `callback`.
    async fn run_loop_core(
        &self,
        actx: &AgentLoopContext<'_>,
        action_ctx: &ActionExecutionContext<'_>,
        mut run: AgentRun,
        callback: Option<Arc<dyn StreamingCallback>>,
        config: &AgentLoopConfig,
        provider_progress: &mut (dyn FnMut(ProviderInvocationProgress) -> Result<()> + Send),
    ) -> Result<AgentLoopResult> {
        let start_time = Instant::now();
        run.user_input = Some(actx.task.user_text.clone());
        let mut step_count: u32 = 0;
        let mut tool_call_count: u32 = 0;
        let mut final_response = String::new();
        let mut current_task = actx.task.clone();
        wrap_user_content(&mut current_task);
        let mut current_tools_prompt = actx.tools_prompt.to_string();
        let current_memory_context = actx.memory_context.clone();
        // Append role-specific instruction if applicable
        if let Some(role_instruction) = config.role_system_instruction() {
            if !current_tools_prompt.is_empty() {
                current_tools_prompt.push_str("\n\n");
            }
            current_tools_prompt.push_str(role_instruction);
        }
        let current_privacy_engine = actx.privacy_engine.clone();
        let mut status_updates: Vec<crate::agent::types::AgentLoopStatusUpdate> = Vec::new();

        // Set reasoning strategy
        run.reasoning_strategy = Some(if actx.task.layer == Layer::L3 {
            "layered".into()
        } else {
            "direct".into()
        });

        #[allow(unused_assignments)]
        let mut stop_reason = String::new();

        loop {
            // Emit thinking status at start of each step
            self.emit_status(
                &mut status_updates,
                crate::agent::types::AgentLoopPhase::Thinking,
                format!(
                    "Step {}: analyzing task and planning next action",
                    step_count + 1
                ),
                step_count,
                None,
            );

            // Check step budget
            if step_count >= config.max_steps {
                stop_reason = "max_steps_reached".into();
                if final_response.is_empty() {
                    final_response = format!(
                        "已达到最大执行步数 ({})。当前结果：{}",
                        config.max_steps, final_response
                    );
                }
                break;
            }

            // Check timeout
            if start_time.elapsed().as_secs() >= config.timeout_seconds {
                run.status = AgentRunStatus::Failed;
                run.error = Some(AgentRunError {
                    message: "Agent loop timeout exceeded".into(),
                    phase: "execution".into(),
                    recoverable: false,
                });
                stop_reason = "timeout".into();
                final_response = "执行超时，请稍后重试。".into();
                break;
            }

            // Search memory for relevant context
            let memory_context = match action_ctx.memory_store {
                Some(_) => match search_memory_for_context(
                    action_ctx,
                    &current_task.user_text,
                    &actx.task.session_id,
                ) {
                    Ok(context) => context,
                    Err(error) => {
                        eprintln!("[AgentLoop] canonical Memory retrieval degraded: {error}");
                        run.status = AgentRunStatus::Failed;
                        run.error = Some(AgentRunError {
                            message: "memory_retrieval_degraded".into(),
                            phase: "context_retrieval".into(),
                            recoverable: true,
                        });
                        run.warnings.push("memory_retrieval_degraded".into());
                        stop_reason = "memory_retrieval_degraded".into();
                        final_response =
                            "记忆检索状态暂时无法确认，本次执行已停止，以避免将未知状态当作空结果。".into();
                        break;
                    }
                },
                None if current_tools_prompt.contains("memory.search") => {
                    run.status = AgentRunStatus::Failed;
                    run.error = Some(AgentRunError {
                        message: "memory_store_unavailable".into(),
                        phase: "context_retrieval".into(),
                        recoverable: true,
                    });
                    run.warnings.push("memory_retrieval_degraded".into());
                    stop_reason = "memory_retrieval_degraded".into();
                    final_response = "记忆存储状态暂时无法确认，本次执行已停止。".into();
                    break;
                }
                None => current_memory_context.clone(),
            };

            // Execute single step (catch parse errors to preserve run.actions)
            let step_result = match self
                .run_single_step(
                    StepContext {
                        task: &current_task,
                        provider_subject_text: actx.provider_subject_text,
                        tools_prompt: &current_tools_prompt,
                        memory_context,
                        privacy_engine: current_privacy_engine.clone(),
                        policy_context: actx.policy_context.clone(),
                        provider_authorization: actx.provider_authorization.clone(),
                        action_ctx,
                        run: &mut run,
                        tool_call_count,
                    },
                    callback.clone(),
                    config,
                    provider_progress,
                )
                .await
            {
                Ok(sr) => sr,
                Err(e) => {
                    run.status = AgentRunStatus::Failed;
                    // Always overwrite — latest error is most relevant to the user.
                    run.error = Some(AgentRunError {
                        message: e.to_string(),
                        phase: "parse".into(),
                        recoverable: false,
                    });
                    stop_reason = "parse_error".into();
                    if final_response.is_empty() {
                        final_response = format!("内部执行错误：{}", e);
                    }
                    break;
                }
            };

            step_count += 1;
            tool_call_count += step_result.tool_call_count_delta;
            final_response = step_result.final_response;
            stop_reason = step_result.stop_reason;
            status_updates.extend(step_result.status_updates);

            if !step_result.should_continue {
                break;
            }

            // Prepare for next iteration
            let follow_up_messages = self.build_follow_up_messages(
                &current_task,
                &final_response,
                &step_result.observations,
                &current_tools_prompt,
            );
            current_task = AgentTask {
                messages: follow_up_messages,
                ..current_task.clone()
            };
            // Keep tools_prompt for next iteration so the model retains tool awareness
            // current_tools_prompt.clear(); // REMOVED: was causing step 2+ to lose tools
        }

        let terminal_disposition = classify_agent_loop_terminal(run.status, &stop_reason);
        let (terminal_phase, terminal_message, terminal_status) = match terminal_disposition {
            AgentLoopTerminalDisposition::Succeeded => {
                run.status = AgentRunStatus::Completed;
                (
                    crate::agent::types::AgentLoopPhase::Completed,
                    format!("Execution completed: {}", stop_reason),
                    "completed",
                )
            }
            AgentLoopTerminalDisposition::WaitingPermission => {
                run.status = AgentRunStatus::WaitingPermission;
                (
                    crate::agent::types::AgentLoopPhase::WaitingPermission,
                    "Execution paused for permission".into(),
                    "waiting_permission",
                )
            }
            AgentLoopTerminalDisposition::Failed => {
                run.status = AgentRunStatus::Failed;
                if run.error.is_none() {
                    run.error = Some(AgentRunError {
                        message: format!("agent_loop_terminal:{stop_reason}"),
                        phase: match stop_reason.as_str() {
                            "tool_execution_failed" => "tool_execution",
                            "tool_allowlist_blocked" => "policy",
                            "max_steps_reached" | "max_tool_calls_reached" => "budget",
                            _ => "execution",
                        }
                        .into(),
                        recoverable: stop_reason == "tool_execution_failed",
                    });
                }
                (
                    crate::agent::types::AgentLoopPhase::Failed,
                    format!("Execution failed: {}", stop_reason),
                    "failed",
                )
            }
            AgentLoopTerminalDisposition::RemoteUnknown => {
                run.status = AgentRunStatus::RemoteUnknown;
                if run.error.is_none() {
                    run.error = Some(AgentRunError {
                        message: "agent_loop_remote_terminal_unknown".into(),
                        phase: "interrupted".into(),
                        recoverable: false,
                    });
                }
                (
                    crate::agent::types::AgentLoopPhase::Failed,
                    "Execution stopped locally; remote terminal state is unknown".into(),
                    "remote_unknown",
                )
            }
            AgentLoopTerminalDisposition::Cancelled => {
                run.status = AgentRunStatus::Cancelled;
                (
                    crate::agent::types::AgentLoopPhase::Failed,
                    "Execution cancelled locally".into(),
                    "cancelled",
                )
            }
        };
        self.emit_status(
            &mut status_updates,
            terminal_phase,
            terminal_message.clone(),
            step_count,
            None,
        );
        if let Some(ref callback) = callback {
            callback
                .on_status(terminal_status, &terminal_message, step_count)
                .await;
        }
        run.output_preview = Some(preview_text(&final_response, 200));
        run.finished_at = Some(chrono::Utc::now());

        Ok(self.build_result(
            run,
            final_response,
            stop_reason,
            terminal_disposition,
            tool_call_count,
            step_count,
            status_updates,
        ))
    }

    /// Run the iterative agent loop for a given task.
    pub async fn run(
        &self,
        task: &AgentTask,
        tools_prompt: &str,
        memory_context: Option<String>,
        privacy_engine: PrivacyEngine,
        action_ctx: &ActionExecutionContext<'_>,
        policy_context: RuntimePolicyContext,
    ) -> Result<AgentLoopResult> {
        let mut ignore_provider_progress = |_: ProviderInvocationProgress| Ok(());
        self.run_with_provider_observer(
            AgentLoopRunRequest::new(
                task,
                tools_prompt,
                memory_context,
                privacy_engine,
                action_ctx,
                policy_context,
            ),
            &mut ignore_provider_progress,
        )
        .await
    }

    /// Run the AgentLoop while exposing adapter-edge provider lifecycle facts
    /// synchronously to the owning runtime.
    pub async fn run_with_provider_observer(
        &self,
        request: AgentLoopRunRequest<'_>,
        provider_progress: &mut (dyn FnMut(ProviderInvocationProgress) -> Result<()> + Send),
    ) -> Result<AgentLoopResult> {
        let AgentLoopRunRequest {
            task,
            tools_prompt,
            memory_context,
            privacy_engine,
            action_ctx,
            policy_context,
            provider_authorization,
        } = request;
        let actx = AgentLoopContext {
            task,
            provider_subject_text: &task.user_text,
            tools_prompt,
            memory_context,
            privacy_engine,
            policy_context,
            provider_authorization,
            network_policy: action_ctx.network_policy.cloned().unwrap_or_default(),
        };
        self.run_loop_core(
            &actx,
            action_ctx,
            AgentRun::new_chat_run(&actx.task.session_id, &actx.task.user_text),
            None,
            &self.config,
            provider_progress,
        )
        .await
    }

    /// Runs the loop as a subordinate of an already-persisted canonical run.
    /// Product runtimes must use this path so tool/proposal/trace facts cannot
    /// acquire a second randomly generated execution identity.
    pub async fn run_existing_with_provider_observer(
        &self,
        request: AgentLoopRunRequest<'_>,
        canonical_run: AgentRun,
        provider_progress: &mut (dyn FnMut(ProviderInvocationProgress) -> Result<()> + Send),
    ) -> Result<AgentLoopResult> {
        let AgentLoopRunRequest {
            task,
            tools_prompt,
            memory_context,
            privacy_engine,
            action_ctx,
            policy_context,
            provider_authorization,
        } = request;
        if canonical_run.id.trim().is_empty() || canonical_run.task_id.trim().is_empty() {
            anyhow::bail!("agent_loop_canonical_run_identity_missing");
        }
        if canonical_run.session_id.as_deref() != Some(task.session_id.as_str()) {
            anyhow::bail!("agent_loop_canonical_run_session_mismatch");
        }
        if canonical_run.status != AgentRunStatus::Running {
            anyhow::bail!("agent_loop_canonical_run_not_running");
        }
        let actx = AgentLoopContext {
            task,
            provider_subject_text: &task.user_text,
            tools_prompt,
            memory_context,
            privacy_engine,
            policy_context,
            provider_authorization,
            network_policy: action_ctx.network_policy.cloned().unwrap_or_default(),
        };
        // Keep the product AgentLoop seam stack-bounded. `run_loop_core`
        // contains the full iterative provider/tool state machine; inlining
        // that future into every upstream Main Chat poll chain can exhaust a
        // normal Tokio test/runtime thread before this async body is entered.
        // Boxing the single subordinate future preserves one runtime owner and
        // changes neither execution nor cancellation semantics.
        Box::pin(self.run_loop_core(
            &actx,
            action_ctx,
            canonical_run,
            None,
            &self.config,
            provider_progress,
        ))
        .await
    }

    fn config_with_runtime_budget(&self, input: &RuntimeInput) -> AgentLoopConfig {
        let mut config = self.config.clone();
        let runtime_config = input.agent_loop_config();
        config.max_steps = runtime_config.max_steps;
        config.max_tool_calls = runtime_config.max_tool_calls;
        config.timeout_seconds = runtime_config.timeout_seconds;
        config.allow_writes = runtime_config.allow_writes;
        config.allow_cloud = runtime_config.allow_cloud;
        config
    }

    /// Run the iterative loop through the RuntimeInput contract while preserving
    /// the existing AgentLoopResult surface for legacy callers.
    pub async fn run_with_runtime_input(
        &self,
        input: &RuntimeInput,
        privacy_engine: PrivacyEngine,
        action_ctx: &ActionExecutionContext<'_>,
    ) -> Result<AgentLoopResult> {
        let runtime_action_ctx = ActionExecutionContext {
            registry: action_ctx.registry,
            permission_store: action_ctx.permission_store,
            audit_store: action_ctx.audit_store,
            privacy_engine: action_ctx.privacy_engine,
            safe_paths: action_ctx.safe_paths,
            life_model: action_ctx.life_model,
            canonical_state: action_ctx.canonical_state,
            memory_store: action_ctx.memory_store,
            memory_lifecycle_retrieval_reader: action_ctx.memory_lifecycle_retrieval_reader,
            resource_store: action_ctx.resource_store,
            proposal_store: action_ctx.proposal_store,
            agent_run_store: action_ctx.agent_run_store,
            bound_content_receipt_issuer: action_ctx.bound_content_receipt_issuer,
            network_policy: action_ctx.network_policy,
            external_write_requires_proposal: input
                .policy_context
                .external_write_requires_proposal(),
            tool_dispatch_observer: action_ctx.tool_dispatch_observer,
            tool_started_transition_observer: action_ctx.tool_started_transition_observer,
            tool_audit_persistence_observer: action_ctx.tool_audit_persistence_observer,
            durable_store_failure_observer: action_ctx.durable_store_failure_observer,
            a2a_outbound_authorization: action_ctx.a2a_outbound_authorization,
            canonical_write_admission: action_ctx.canonical_write_admission,
            action_bound_tool_permission: action_ctx.action_bound_tool_permission,
            calendar_ics_paths: action_ctx.calendar_ics_paths,
            web_search_fixture_output: action_ctx.web_search_fixture_output,
        };
        let actx = AgentLoopContext {
            task: &input.task,
            provider_subject_text: &input.task.user_text,
            tools_prompt: &input.tools_prompt,
            memory_context: input.memory_context.clone(),
            privacy_engine,
            policy_context: input.policy_context.clone(),
            provider_authorization: None,
            network_policy: runtime_action_ctx
                .network_policy
                .cloned()
                .unwrap_or_default(),
        };
        let config = self.config_with_runtime_budget(input);

        let mut ignore_provider_progress = |_: ProviderInvocationProgress| Ok(());
        self.run_loop_core(
            &actx,
            &runtime_action_ctx,
            AgentRun::new_chat_run(&actx.task.session_id, &actx.task.user_text),
            None,
            &config,
            &mut ignore_provider_progress,
        )
        .await
    }

    /// Run the iterative loop through RuntimeInput and return the converged
    /// RuntimeOutput contract.
    pub async fn run_runtime_input(
        &self,
        input: RuntimeInput,
        privacy_engine: PrivacyEngine,
        action_ctx: &ActionExecutionContext<'_>,
    ) -> Result<RuntimeOutput> {
        let result = self
            .run_with_runtime_input(&input, privacy_engine, action_ctx)
            .await?;
        Ok(RuntimeOutput::from_agent_loop_result(result))
    }

    /// Streaming variant of run(). Same logic but forwards token chunks
    /// through the callback as they arrive from the model.
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub async fn run_streaming(
        &self,
        task: &AgentTask,
        tools_prompt: &str,
        memory_context: Option<String>,
        privacy_engine: PrivacyEngine,
        action_ctx: &ActionExecutionContext<'_>,
        policy_context: RuntimePolicyContext,
        callback: Arc<dyn StreamingCallback>,
    ) -> Result<AgentLoopResult> {
        let actx = AgentLoopContext {
            task,
            provider_subject_text: &task.user_text,
            tools_prompt,
            memory_context,
            privacy_engine,
            policy_context,
            provider_authorization: None,
            network_policy: action_ctx.network_policy.cloned().unwrap_or_default(),
        };
        let mut ignore_provider_progress = |_: ProviderInvocationProgress| Ok(());
        self.run_loop_core(
            &actx,
            action_ctx,
            AgentRun::new_chat_run(&actx.task.session_id, &actx.task.user_text),
            Some(callback),
            &self.config,
            &mut ignore_provider_progress,
        )
        .await
    }

    /// Execute a single step of the agent loop.
    /// If `callback` is provided, uses streaming generation and emits tool events.
    async fn run_single_step(
        &self,
        mut ctx: StepContext<'_>,
        callback: Option<Arc<dyn StreamingCallback>>,
        config: &AgentLoopConfig,
        provider_progress: &mut (dyn FnMut(ProviderInvocationProgress) -> Result<()> + Send),
    ) -> Result<StepResult> {
        let mut status_updates: Vec<crate::agent::types::AgentLoopStatusUpdate> = Vec::new();

        // Clone values that will be consumed by generate_response so we can
        // re-use them in a one-shot JSON repair round.
        let memory_ctx = ctx.memory_context.clone();
        let privacy = ctx.privacy_engine.clone();

        // Generate model response (streaming if callback provided)
        let generated = {
            let actx = AgentLoopContext {
                task: ctx.task,
                provider_subject_text: ctx.provider_subject_text,
                tools_prompt: ctx.tools_prompt,
                memory_context: ctx.memory_context.clone(),
                privacy_engine: ctx.privacy_engine.clone(),
                policy_context: ctx.policy_context.clone(),
                provider_authorization: ctx.provider_authorization.clone(),
                network_policy: ctx.action_ctx.network_policy.cloned().unwrap_or_default(),
            };
            if let Some(ref cb) = callback {
                self.generate_response_streaming(&actx, cb.clone(), provider_progress)
                    .await
            } else {
                self.generate_response(&actx, provider_progress).await
            }
        };

        match generated {
            Ok(gen) => {
                if ctx.run.context_summary.is_none() {
                    ctx.run.context_summary = Some(gen.runtime_output.context_summary.clone());
                }
                if ctx.run.reasoning_trace.is_none() {
                    ctx.run.reasoning_trace = Some(gen.runtime_output.reasoning_trace.clone());
                }
                let reply = gen.reply;

                // Check for tool calls in the reply
                let mut parsed = self.parse_agent_reply(
                    &reply,
                    ctx.action_ctx,
                    ctx.run,
                    &mut ctx.tool_call_count,
                )?;

                // One-shot JSON self-repair
                if parsed.json_parse_failed {
                    self.emit_status(
                        &mut status_updates,
                        crate::agent::types::AgentLoopPhase::Thinking,
                        "JSON parse failed, attempting one-shot repair...",
                        0,
                        None,
                    );
                    if let Some(ref cb) = callback {
                        cb.on_status(
                            "thinking",
                            "JSON parse failed, attempting one-shot repair...",
                            0,
                        )
                        .await;
                    }
                    parsed = self
                        .try_json_self_repair(
                            &AgentLoopContext {
                                task: ctx.task,
                                provider_subject_text: ctx.provider_subject_text,
                                tools_prompt: ctx.tools_prompt,
                                memory_context: memory_ctx.clone(),
                                privacy_engine: privacy.clone(),
                                policy_context: ctx.policy_context.clone(),
                                provider_authorization: ctx.provider_authorization.clone(),
                                network_policy: ctx
                                    .action_ctx
                                    .network_policy
                                    .cloned()
                                    .unwrap_or_default(),
                            },
                            ctx.action_ctx,
                            ctx.run,
                            &mut ctx.tool_call_count,
                            provider_progress,
                        )
                        .await?;
                }

                let final_text = parsed.final_text;
                let (tool_actions, rejected_tool_count) =
                    self.partition_tools_by_allowlist(parsed.actions, config);
                if rejected_tool_count > 0 {
                    ctx.run.warnings.push(format!(
                        "Tool selection blocked: disallowed_tool_count={rejected_tool_count}"
                    ));
                    return Ok(StepResult {
                        stop_reason: "tool_allowlist_blocked".into(),
                        final_response: final_text,
                        should_continue: false,
                        tool_call_count_delta: 0,
                        observations: vec![],
                        status_updates,
                    });
                }

                if tool_actions.is_empty() {
                    self.emit_status(
                        &mut status_updates,
                        crate::agent::types::AgentLoopPhase::GeneratingFinal,
                        "No tools needed, generating final answer",
                        0,
                        None,
                    );
                    if let Some(ref cb) = callback {
                        cb.on_status(
                            "generating_final",
                            "No tools needed, generating final answer",
                            0,
                        )
                        .await;
                    }
                    return Ok(StepResult {
                        stop_reason: "no_tools".into(),
                        final_response: final_text,
                        should_continue: false,
                        tool_call_count_delta: 0,
                        observations: vec![],
                        status_updates,
                    });
                }

                // Model wants to use tools
                self.emit_status(
                    &mut status_updates,
                    crate::agent::types::AgentLoopPhase::PlanningTool,
                    format!("Planning to execute {} tool(s)", tool_actions.len()),
                    0,
                    None,
                );
                if let Some(ref cb) = callback {
                    cb.on_status(
                        "planning_tool",
                        &format!("Planning to execute {} tool(s)", tool_actions.len()),
                        0,
                    )
                    .await;
                }

                let (all_succeeded, executed_this_step, budget_exceeded, observations) = self
                    .execute_tool_batch(
                        &tool_actions,
                        ctx.action_ctx,
                        ctx.run,
                        &mut ctx.tool_call_count,
                        &callback,
                        &mut status_updates,
                        config,
                    )
                    .await?;

                Ok(self.handle_step_completion(
                    budget_exceeded,
                    all_succeeded,
                    observations,
                    executed_this_step,
                    final_text,
                    ctx.run,
                    &mut status_updates,
                    config,
                ))
            }
            Err(e) => {
                ctx.run.status = AgentRunStatus::Failed;
                ctx.run.error = Some(AgentRunError {
                    message: e.to_string(),
                    phase: "model".into(),
                    recoverable: false,
                });
                Ok(StepResult {
                    stop_reason: "model_error".into(),
                    final_response: format!("模型生成失败: {}", e),
                    should_continue: false,
                    tool_call_count_delta: 0,
                    observations: vec![],
                    status_updates,
                })
            }
        }
    }

    async fn generate_response(
        &self,
        actx: &AgentLoopContext<'_>,
        provider_progress: &mut (dyn FnMut(ProviderInvocationProgress) -> Result<()> + Send),
    ) -> Result<GeneratedAgentResponse> {
        let memory_hits = Vec::new();
        let runtime_output = self
            .runtime
            .execute_task(
                actx.task,
                actx.tools_prompt,
                actx.memory_context.clone(),
                memory_hits,
                actx.privacy_engine.clone(),
                actx.policy_context.clone(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("runtime execution failed: {}", e))?;

        if let Some(reply) = self.next_scripted_reply() {
            return Ok(GeneratedAgentResponse {
                runtime_output,
                reply,
            });
        }

        let prepared = self
            .prepare_provider_request(actx, runtime_output.final_messages.clone())
            .await?;
        let provider_outcome = match &self.scheduler {
            AgentLoopProviderScheduler::General(scheduler) => {
                let outcome = scheduler
                    .execute_prepared_with_observer(prepared, provider_progress)
                    .await;
                scheduler.verify_prepared_outcome_receipt(&outcome)?;
                outcome
            }
            AgentLoopProviderScheduler::Scheduled(scheduler) => {
                let outcome = scheduler.execute_scheduled_provider_request(prepared).await;
                scheduler.verify_scheduled_outcome(&outcome)?;
                outcome
            }
        };
        let reply = provider_outcome
            .result
            .map_err(|e| anyhow::anyhow!("model generation failed: {e}"))?;

        Ok(GeneratedAgentResponse {
            runtime_output,
            reply,
        })
    }

    /// Streaming variant of generate_response. It forwards transient token
    /// events and the scheduler-owned typed terminal without reconstructing
    /// provider receipts in the loop.
    async fn generate_response_streaming(
        &self,
        actx: &AgentLoopContext<'_>,
        callback: Arc<dyn StreamingCallback>,
        provider_progress: &mut (dyn FnMut(ProviderInvocationProgress) -> Result<()> + Send),
    ) -> Result<GeneratedAgentResponse> {
        let memory_hits = Vec::new();
        let runtime_output = self
            .runtime
            .execute_task(
                actx.task,
                actx.tools_prompt,
                actx.memory_context.clone(),
                memory_hits,
                actx.privacy_engine.clone(),
                actx.policy_context.clone(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("runtime execution failed: {}", e))?;

        if let Some(reply) = self.next_scripted_reply() {
            callback.on_chunk(&reply, 0, "generating").await;
            return Ok(GeneratedAgentResponse {
                runtime_output,
                reply,
            });
        }

        let prepared = self
            .prepare_provider_request(actx, runtime_output.final_messages.clone())
            .await?;
        let AgentLoopProviderScheduler::General(scheduler) = &self.scheduler else {
            anyhow::bail!("scheduled provider execution does not expose a streaming bypass");
        };
        let stream = scheduler
            .generate_prepared_stream_with_start_observer(
                prepared,
                |request_id, provider, model, observed_at, observed_policy_evidence| {
                    provider_progress(ProviderInvocationProgress::Started {
                        request_id: request_id.to_string(),
                        provider: provider.to_string(),
                        model: model.to_string(),
                        started_at: observed_at,
                        policy_evidence: observed_policy_evidence.clone(),
                    })?;
                    Ok(())
                },
            )
            .await
            .map_err(|error| anyhow::anyhow!("stream generation failed: {error}"))?;
        let reply = self
            .consume_provider_stream(stream, callback, provider_progress)
            .await?;

        Ok(GeneratedAgentResponse {
            runtime_output,
            reply,
        })
    }

    async fn consume_provider_stream(
        &self,
        mut stream: PreparedProviderStream,
        callback: Arc<dyn StreamingCallback>,
        provider_progress: &mut (dyn FnMut(ProviderInvocationProgress) -> Result<()> + Send),
    ) -> Result<String> {
        let mut reply = String::new();
        while let Some(event) = stream.next().await {
            match event {
                PreparedProviderStreamEvent::Token(chunk) => {
                    callback.on_chunk(&chunk, 0, "generating").await;
                    reply.push_str(&chunk);
                }
                PreparedProviderStreamEvent::Terminal(
                    PreparedProviderStreamTerminal::NotAttempted,
                ) => return Ok(reply),
                PreparedProviderStreamEvent::Terminal(
                    PreparedProviderStreamTerminal::Completed(receipt),
                ) => {
                    provider_progress(ProviderInvocationProgress::Completed(*receipt))?;
                    return Ok(reply);
                }
                PreparedProviderStreamEvent::Terminal(PreparedProviderStreamTerminal::Failed {
                    receipt,
                    error,
                }) => {
                    provider_progress(ProviderInvocationProgress::Failed(*receipt))?;
                    return Err(anyhow::anyhow!("stream generation failed: {error}"));
                }
                PreparedProviderStreamEvent::Terminal(
                    PreparedProviderStreamTerminal::RemoteUnknown { receipt, error },
                ) => {
                    provider_progress(ProviderInvocationProgress::RemoteUnknown(*receipt))?;
                    return Err(anyhow::anyhow!("stream generation failed: {error}"));
                }
            }
        }
        anyhow::bail!("prepared provider stream ended without its typed terminal event")
    }

    /// Parse model response for JSON envelope.
    /// Supports format: {"final": "...", "actions": [...], "thought_summary": "...", "warnings": [...]}
    /// Fail-soft: malformed JSON or missing envelope returns empty actions (treat as final).
    #[cfg(test)]
    pub fn parse_tool_calls(
        &self,
        reply: &str,
        _action_ctx: &ActionExecutionContext<'_>,
        run: &mut AgentRun,
        tool_call_count: &mut u32,
    ) -> Result<Vec<AgentActionRequest>> {
        Ok(self
            .parse_agent_reply(reply, _action_ctx, run, tool_call_count)?
            .actions)
    }

    pub(crate) fn parse_agent_reply(
        &self,
        reply: &str,
        action_ctx: &ActionExecutionContext<'_>,
        run: &mut AgentRun,
        tool_call_count: &mut u32,
    ) -> Result<ParsedAgentReply> {
        let json_str = try_extract_json(reply);
        let json_str = if let Some(s) = json_str {
            s
        } else if reply.contains('{') {
            // Found '{' but no valid JSON object - try parsing anyway for error recording
            reply
        } else {
            // No JSON found - treat entire response as final answer
            return Ok(ParsedAgentReply {
                final_text: reply.to_string(),
                actions: Vec::new(),
                json_parse_failed: false,
            });
        };

        let v: Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => {
                // Fail-soft: malformed JSON, record warning and treat as final.
                // Signal json_parse_failed so caller can attempt a one-shot repair.
                run.warnings.push(format!(
                    "Parse warning: invalid JSON in model response: {}",
                    e
                ));
                return Ok(ParsedAgentReply {
                    final_text: reply.to_string(),
                    actions: Vec::new(),
                    json_parse_failed: true,
                });
            }
        };

        // Check for thought_summary and warnings
        if let Some(thought) = v.get("thought_summary").and_then(|t| t.as_str()) {
            run.warnings.push(format!(
                "Model thought: {}",
                metadata_safe_model_note(thought)
            ));
        }
        if let Some(warnings) = v.get("warnings").and_then(|w| w.as_array()) {
            for warning in warnings {
                if let Some(w) = warning.as_str() {
                    run.warnings
                        .push(format!("Model warning: {}", metadata_safe_model_note(w)));
                }
            }
        }

        let final_text = v
            .get("final")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| reply.to_string());

        // Parse actions (new format) or tool_calls (legacy format)
        // If both "final" and "actions"/"tool_calls" are present,
        // execute the actions and treat "final" as a pre-execution note.
        let calls = if let Some(actions) = v.get("actions").and_then(|a| a.as_array()) {
            if actions.is_empty() {
                return Ok(ParsedAgentReply {
                    final_text,
                    actions: Vec::new(),
                    json_parse_failed: false,
                });
            }
            actions
        } else if let Some(tool_calls) = v.get("tool_calls").and_then(|c| c.as_array()) {
            if tool_calls.is_empty() {
                return Ok(ParsedAgentReply {
                    final_text,
                    actions: Vec::new(),
                    json_parse_failed: false,
                });
            }
            tool_calls
        } else {
            // No actions or tool_calls - treat as final answer
            return Ok(ParsedAgentReply {
                final_text,
                actions: Vec::new(),
                json_parse_failed: false,
            });
        };

        let mut requests = Vec::new();
        for (idx, call) in calls.iter().enumerate() {
            let Some(name) = call
                .get("name")
                .or_else(|| call.get("tool"))
                .and_then(|n| n.as_str())
            else {
                run.warnings.push(format!(
                    "Parse warning: missing_tool_name at action_index={idx}"
                ));
                continue;
            };
            let args = match call
                .get("arguments")
                .or_else(|| call.get("args"))
                .or_else(|| call.get("input"))
            {
                None => serde_json::json!({}),
                Some(value) if value.is_object() => value.clone(),
                Some(Value::Null) => serde_json::json!({}),
                Some(_) => {
                    run.warnings.push(format!(
                        "Parse warning: invalid_arguments_defaulted_empty for action_index={idx}"
                    ));
                    serde_json::json!({})
                }
            };

            let explicit_action_type = call.get("action_type").and_then(|t| t.as_str());
            let registered_tool_like = action_ctx
                .registry
                .list_manifests()
                .iter()
                .any(|manifest| manifest.name == name || manifest.id == name);
            let action_type = explicit_action_type
                .map(ToString::to_string)
                .unwrap_or_else(|| {
                    if !registered_tool_like {
                        run.warnings.push(format!(
                            "Parse warning: unregistered_tool_defaulted_mcp_tool at action_index={idx}"
                        ));
                    }
                    "mcp_tool".to_string()
                });
            let input = match action_type.as_str() {
                "memory_search" => args.clone(),
                "session_search" => {
                    let mut governed = args.clone();
                    if let (Some(object), Some(current_session_id)) =
                        (governed.as_object_mut(), run.session_id.as_deref())
                    {
                        // Current conversation identity is canonical run
                        // state, never a model-authorized search target. A
                        // model-supplied session id cannot narrow the search
                        // back to the triggering conversation and manufacture
                        // its own "prior" evidence.
                        object.remove("session_id");
                        object.insert(
                            "exclude_session_id".into(),
                            Value::String(current_session_id.to_string()),
                        );
                    }
                    governed
                }
                _ => serde_json::json!({ "arguments": args }),
            };

            requests.push(AgentActionRequest {
                action_type,
                target: name.to_string(),
                input,
                source_run_id: Some(run.id.clone()),
                step_index: *tool_call_count + idx as u32,
            });
        }

        Ok(ParsedAgentReply {
            final_text,
            actions: requests,
            json_parse_failed: false,
        })
    }

    pub(crate) fn build_follow_up_messages(
        &self,
        task: &AgentTask,
        assistant_reply: &str,
        observations: &[AgentObservation],
        tools_prompt: &str,
    ) -> Vec<ChatMessage> {
        let mut messages = task.messages.clone();
        messages.push(ChatMessage {
            role: "assistant".into(),
            content: assistant_reply.into(),
        });

        // Build structured follow-up with: task goal, available tools, observations
        let mut follow_up = String::new();

        // Remind the model of the original task (strip boundary markers if present)
        let clean_user_text = task
            .user_text
            .replace(USER_REQUEST_START, "")
            .replace(USER_REQUEST_END, "")
            .trim()
            .to_string();
        follow_up.push_str(&format!(
            "[系统] 继续完成用户的原始请求：\"{}\"\n\n",
            clean_user_text
        ));

        // Include observations from tool executions
        if !observations.is_empty() {
            follow_up.push_str("工具执行结果：\n");
            for (idx, obs) in observations.iter().enumerate() {
                follow_up.push_str(&format!("[{}] {}\n", idx + 1, obs.content));
            }
            follow_up.push('\n');
        }

        // Remind available tools for next step
        if !tools_prompt.is_empty() {
            follow_up.push_str("下一步可用工具：\n");
            follow_up.push_str(tools_prompt);
            follow_up.push('\n');
        }

        follow_up.push_str("请继续使用工具或提供最终回答。");

        messages.push(ChatMessage {
            role: "user".into(),
            content: follow_up,
        });

        messages
    }

    /// Partition tool actions by the configured allowlist.
    /// Returns all actions and zero rejected actions if allowlist is not configured.
    fn partition_tools_by_allowlist(
        &self,
        actions: Vec<AgentActionRequest>,
        config: &AgentLoopConfig,
    ) -> (Vec<AgentActionRequest>, usize) {
        if config.tool_action_allowlist.is_empty() && config.toolset_allowlist.is_empty() {
            return (actions, 0);
        }
        let mut allowed_actions = Vec::new();
        let mut rejected_count = 0;
        for mut action in actions {
            let matched_allowed_action = if config.tool_action_allowlist.is_empty() {
                None
            } else {
                config.tool_action_allowlist.iter().find(|allowed| {
                    action.action_type == allowed.action_type && action.target == allowed.target
                })
            };
            let target_allowed = if config.tool_action_allowlist.is_empty() {
                config.toolset_allowlist.contains(&action.target)
            } else {
                matched_allowed_action.is_some()
            };
            let action_type_allowed =
                config.allow_writes || agent_loop_read_only_action_type(&action.action_type);
            if target_allowed && action_type_allowed {
                if let Some(allowed) = matched_allowed_action {
                    action.input = allowed.input.clone();
                }
                allowed_actions.push(action);
            } else {
                rejected_count += 1;
            }
        }
        (allowed_actions, rejected_count)
    }

    /// Execute a batch of tool actions, collecting observations and status updates.
    /// Returns (all_succeeded, executed_count, budget_exceeded, observations).
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    async fn execute_tool_batch(
        &self,
        tool_actions: &[AgentActionRequest],
        action_ctx: &ActionExecutionContext<'_>,
        run: &mut AgentRun,
        tool_call_count: &mut u32,
        callback: &Option<Arc<dyn StreamingCallback>>,
        status_updates: &mut Vec<crate::agent::types::AgentLoopStatusUpdate>,
        config: &AgentLoopConfig,
    ) -> Result<(bool, u32, bool, Vec<AgentObservation>)> {
        let mut observations = Vec::new();
        let mut all_succeeded = true;
        let mut executed_this_step: u32 = 0;
        let mut budget_exceeded = false;

        for (idx, action_request) in tool_actions.iter().enumerate() {
            // `tool_call_count` is advanced after every dispatched action below.
            // Adding `executed_this_step` here counted the current batch twice and
            // incorrectly rejected the second action when the budget was exactly 2.
            if *tool_call_count >= config.max_tool_calls {
                let obs = self.create_budget_exceeded_observation(
                    run,
                    *tool_call_count,
                    config.max_tool_calls,
                );
                observations.push(obs.clone());
                run.observations.push(obs);
                all_succeeded = false;
                budget_exceeded = true;
                break;
            }

            self.emit_status(
                status_updates,
                crate::agent::types::AgentLoopPhase::ExecutingTool,
                format!("Executing tool: {}", action_request.target),
                0,
                Some(idx as u32),
            );
            if let Some(ref cb) = callback {
                cb.on_status(
                    "executing_tool",
                    &format!("Executing tool: {}", action_request.target),
                    0,
                )
                .await;
            }

            if let Some(ref cb) = callback {
                cb.on_tool_start(&action_request.target, 0).await;
            }

            let exec_result = match self
                .tool_gateway
                .execute(action_request.clone(), action_ctx)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let now = chrono::Utc::now();
                    let fail_action = crate::agent::types::AgentAction {
                        id: format!(
                            "action-fail-{}",
                            now.timestamp_nanos_opt().unwrap_or_default()
                        ),
                        action_type: action_request.action_type.clone(),
                        target: Some(action_request.target.clone()),
                        input: action_request.input.clone(),
                        output: None,
                        status: "failed".into(),
                        error: Some(e.to_string()),
                        permission_decision: None,
                        started_at: None,
                        finished_at: Some(now),
                        timestamp: now,
                        tool_scope: None,
                        react_trace: None,
                        runtime_execution_receipt: None,
                    };
                    let obs = AgentObservation {
                        id: format!("obs-fail-{}", now.timestamp_nanos_opt().unwrap_or_default()),
                        action_id: Some(fail_action.id.clone()),
                        content: format!("工具 {} 执行失败: {}", action_request.target, e),
                        source: "action_executor".into(),
                        structured_result: Some(serde_json::json!({"error": e.to_string()})),
                        timestamp: now,
                        react_trace: None,
                    };
                    run.actions.push(fail_action.clone());
                    observations.push(obs.clone());
                    run.observations.push(obs);
                    all_succeeded = false;
                    *tool_call_count += 1;
                    executed_this_step += 1;
                    continue;
                }
            };

            // Collect proposal_id from action output if present
            if let Some(ref output) = exec_result.action.output {
                let proposal_id = output
                    .get("proposal_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        output
                            .get("text")
                            .and_then(|v| v.as_str())
                            .and_then(|text| {
                                serde_json::from_str::<serde_json::Value>(text)
                                    .ok()
                                    .and_then(|json| {
                                        json.get("proposal_id")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string())
                                    })
                            })
                    });
                if let Some(id) = proposal_id {
                    if let Some(ref cb) = callback {
                        cb.on_proposal("external_write_action", &id).await;
                    }
                    run.add_generated_proposal(&id);
                }
            }

            run.actions.push(exec_result.action.clone());
            observations.push(exec_result.observation.clone());
            run.observations.push(exec_result.observation.clone());

            if let Some(ref cb) = callback {
                cb.on_tool_result(
                    &action_request.target,
                    exec_result.status == ActionExecutionStatus::Succeeded,
                    0,
                )
                .await;
            }

            self.emit_status(
                status_updates,
                crate::agent::types::AgentLoopPhase::Observing,
                format!(
                    "Tool {} result: {}",
                    action_request.target,
                    if exec_result.status == ActionExecutionStatus::Succeeded {
                        "success"
                    } else {
                        "failed"
                    }
                ),
                0,
                Some(idx as u32),
            );
            if let Some(ref cb) = callback {
                let result_str = if exec_result.status == ActionExecutionStatus::Succeeded {
                    "success"
                } else {
                    "failed"
                };
                cb.on_status(
                    "observing",
                    &format!("Tool {} result: {}", action_request.target, result_str),
                    0,
                )
                .await;
            }

            if exec_result.status != ActionExecutionStatus::Succeeded {
                all_succeeded = false;
            }

            *tool_call_count += 1;
            executed_this_step += 1;
        }

        Ok((
            all_succeeded,
            executed_this_step,
            budget_exceeded,
            observations,
        ))
    }

    /// Handle step completion after tool batch execution:
    /// budget exceeded / partial failure / no observations / continue.
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    fn handle_step_completion(
        &self,
        budget_exceeded: bool,
        all_succeeded: bool,
        observations: Vec<AgentObservation>,
        executed_this_step: u32,
        final_text: String,
        run: &mut AgentRun,
        status_updates: &mut Vec<crate::agent::types::AgentLoopStatusUpdate>,
        config: &AgentLoopConfig,
    ) -> StepResult {
        if budget_exceeded {
            run.status = AgentRunStatus::Failed;
            run.error = Some(AgentRunError {
                message: "agent_loop_terminal:max_tool_calls_reached".into(),
                phase: "budget".into(),
                recoverable: false,
            });
            let final_response = format!(
                "已达到最大工具调用次数 ({})。已完成的观察结果：\n{}",
                config.max_tool_calls,
                observations
                    .iter()
                    .map(|o| format!("- {}", o.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            return StepResult {
                stop_reason: "max_tool_calls_reached".into(),
                final_response,
                should_continue: false,
                tool_call_count_delta: executed_this_step,
                observations,
                status_updates: std::mem::take(status_updates),
            };
        }

        if !all_succeeded {
            // Classify only the actions produced by this batch. Historical pending
            // actions must not mask a current failure, and a mixed pending+failed
            // batch is a failure because at least one terminal failure was observed.
            let current_actions = run
                .actions
                .iter()
                .rev()
                .take(executed_this_step as usize)
                .collect::<Vec<_>>();
            let pending_count = current_actions
                .iter()
                .filter(|action| action.status == "needs_confirmation")
                .count();
            let has_failure = current_actions.iter().any(|action| {
                action.status != "succeeded" && action.status != "needs_confirmation"
            });
            let waiting_only = pending_count > 0 && !has_failure;
            let final_response = if waiting_only {
                run.status = AgentRunStatus::WaitingPermission;
                "我需要先执行一些高风险或含敏感参数的工具操作，确认后才能继续给你结果。".into()
            } else {
                run.status = AgentRunStatus::Failed;
                run.error = Some(AgentRunError {
                    message: "agent_loop_terminal:tool_execution_failed".into(),
                    phase: "tool_execution".into(),
                    recoverable: true,
                });
                "工具执行过程中出现错误，请检查配置或稍后重试。".into()
            };
            return StepResult {
                stop_reason: if waiting_only {
                    "needs_confirmation".into()
                } else {
                    "tool_execution_failed".into()
                },
                final_response,
                should_continue: false,
                tool_call_count_delta: executed_this_step,
                observations,
                status_updates: std::mem::take(status_updates),
            };
        }

        if observations.is_empty() {
            return StepResult {
                stop_reason: "no_observations".into(),
                final_response: final_text,
                should_continue: false,
                tool_call_count_delta: 0,
                observations: vec![],
                status_updates: std::mem::take(status_updates),
            };
        }

        // Continue to next iteration
        StepResult {
            stop_reason: String::new(),
            final_response: final_text,
            should_continue: true,
            tool_call_count_delta: executed_this_step,
            observations,
            status_updates: std::mem::take(status_updates),
        }
    }

    fn create_budget_exceeded_observation(
        &self,
        _run: &AgentRun,
        tool_call_count: u32,
        max_tool_calls: u32,
    ) -> AgentObservation {
        let now = chrono::Utc::now();
        AgentObservation {
            id: format!(
                "observation-budget-{}",
                now.timestamp_nanos_opt().unwrap_or_default()
            ),
            action_id: None,
            content: format!("工具调用预算已耗尽 (max_tool_calls={})", max_tool_calls),
            source: "agent_loop".into(),
            structured_result: Some(serde_json::json!({
                "error": "max_tool_calls exceeded",
                "max_tool_calls": max_tool_calls,
                "current_count": tool_call_count,
            })),
            timestamp: now,
            react_trace: None,
        }
    }

    // Assemble independently audited terminal facts without a second mutable accumulator.
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    fn build_result(
        &self,
        mut run: AgentRun,
        final_response: String,
        stop_reason: String,
        terminal_disposition: AgentLoopTerminalDisposition,
        tool_call_count: u32,
        step_count: u32,
        status_updates: Vec<crate::agent::types::AgentLoopStatusUpdate>,
    ) -> AgentLoopResult {
        run.step_count = step_count;
        run.tool_call_count = tool_call_count;
        run.status_updates = status_updates.clone();
        AgentLoopResult {
            run,
            final_response,
            stop_reason,
            terminal_disposition,
            tool_call_count,
            step_count,
            status_updates,
        }
    }
}

fn agent_loop_read_only_action_type(action_type: &str) -> bool {
    matches!(
        action_type,
        "mcp_tool" | "builtin_tool" | "plugin_tool" | "memory_search" | "session_search"
    )
}

struct GeneratedAgentResponse {
    runtime_output: AgentRuntimeOutput,
    reply: String,
}

/// One-shot JSON self-repair prompt sent to the model when its previous
/// response was not valid JSON. Bilingual + schema-first for best results
/// across different models.
const SELF_REPAIR_PROMPT: &str = r#"Your previous response was not valid JSON for tool calling.
请只输出一个合法 JSON object，不要 markdown，不要解释。

Allowed shape:
{"final": "reply to user", "actions": [{"name": "tool_name", "arguments": {}}], "thought_summary": "brief reasoning", "warnings": []}
If no tools needed: {"final": "reply to user"}

Original request: "#;

pub(crate) struct ParsedAgentReply {
    pub(crate) final_text: String,
    pub(crate) actions: Vec<AgentActionRequest>,
    /// True if the model generated a JSON-like response that failed to parse.
    /// When true, the caller should attempt a one-shot repair round.
    pub(crate) json_parse_failed: bool,
}

fn metadata_safe_model_note(note: &str) -> String {
    let lower = note.to_ascii_lowercase();
    let looks_sensitive = lower.contains("raw")
        || lower.contains("secret")
        || note.contains('@')
        || lower.contains("prompt")
        || lower.contains("assistant output")
        || lower.contains("tool payload")
        || lower.contains("memory context")
        || lower.contains("lifemodel");
    if looks_sensitive {
        let (byte_count, hash) = crate::agent::metadata_safe::metadata_safe_text_digest(note);
        format!("{byte_count} bytes redacted ({hash})")
    } else {
        note.to_string()
    }
}

fn preview_text(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        format!("{}...", text.chars().take(max_len).collect::<String>())
    }
}

/// Search memory store for relevant context and format as a string.
fn search_memory_for_context(
    action_ctx: &ActionExecutionContext<'_>,
    query: &str,
    session_id: &str,
) -> Result<Option<String>> {
    if query.trim().is_empty() {
        return Ok(None);
    }

    let memory_store = action_ctx
        .memory_store
        .ok_or_else(|| anyhow::anyhow!("MemoryStore unavailable for AgentLoop context"))?;
    let hits = action_ctx.filter_retrievable_memory_hits(memory_store.search_text_memories(
        Some(session_id),
        query,
        5,
    )?)?;
    if hits.is_empty() {
        return Ok(None);
    }

    let mut context = String::from("以下是与当前话题相关的历史记忆：\n\n");
    for (idx, hit) in hits.iter().enumerate() {
        context.push_str(&format!(
            "[记忆 {}] {} (相关度: {:.2})\n{}\n\n",
            idx + 1,
            hit.chunk.source,
            hit.relevance_score,
            hit.chunk.content
        ));
    }

    Ok(Some(context))
}

/// Extract JSON object from text.
fn try_extract_json(text: &str) -> Option<&str> {
    crate::json_utils::extract_first_json_object(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::action_executor::ActionExecutorConfig;
    use crate::agent::runtime::AgentRuntime;
    use crate::agent::tool_gateway::ToolGateway;
    use crate::agent::types::{AgentObservation, AgentRun, AgentTaskKind};
    use crate::config::AppConfig;
    use crate::llm::{ChatMessage, ProviderInvocationReceipt};
    use crate::mcp::McpRegistry;
    use crate::mcp_audit::McpAuditStore;
    use crate::privacy::PrivacyEngine;
    use crate::scheduler::InferenceScheduler;
    use crate::tool_permissions::ToolPermissionStore;

    /// Creates a minimal AgentLoop for testing parse_agent_reply and build_follow_up_messages.
    /// Uses dummy scheduler credentials (no actual LLM calls are made).
    fn make_test_agent_loop() -> AgentLoop {
        let scheduler = InferenceScheduler::new(
            "llama3".into(),
            false,
            "openrouter".into(),
            "https://test.example.com/v1".into(),
            "sk-test".into(),
            "gpt-3.5-turbo".into(),
            "text-embedding-ada-002".into(),
            false,
        );
        let app_config = AppConfig::default();
        let runtime = AgentRuntime::new(scheduler.clone(), &app_config);
        let gateway = ToolGateway::from_executor_config(ActionExecutorConfig::default());
        let config = AgentLoopConfig::default();
        AgentLoop::new(runtime, gateway, scheduler, config)
    }

    /// Creates a provider-preparation fixture that never depends on a live
    /// local Ollama process. The scripted scheduler still exercises payload
    /// authorization and provider selection without crossing an adapter edge.
    fn make_scripted_provider_test_agent_loop() -> AgentLoop {
        let scheduler = InferenceScheduler::new(
            "llama3".into(),
            false,
            "openrouter".into(),
            "https://test.example.com/v1".into(),
            "sk-test".into(),
            "gpt-3.5-turbo".into(),
            "text-embedding-ada-002".into(),
            false,
        )
        .with_scripted_generation_response("provider preparation fixture");
        let app_config = AppConfig::default();
        let runtime = AgentRuntime::new(scheduler.clone(), &app_config);
        let gateway = ToolGateway::from_executor_config(ActionExecutorConfig::default());
        let config = AgentLoopConfig::default();
        AgentLoop::new(runtime, gateway, scheduler, config)
    }

    fn terminal_test_action(id: &str, status: &str) -> crate::agent::types::AgentAction {
        let now = chrono::Utc::now();
        crate::agent::types::AgentAction {
            id: id.to_string(),
            action_type: "mcp_tool".into(),
            target: Some("fixture.tool".into()),
            input: serde_json::json!({}),
            output: None,
            status: status.into(),
            error: None,
            permission_decision: None,
            started_at: Some(now),
            finished_at: Some(now),
            timestamp: now,
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: None,
        }
    }

    #[test]
    fn mixed_permission_and_failure_batch_fails_instead_of_masking_failure() {
        let agent = make_test_agent_loop();
        let mut run = AgentRun::new_chat_run("session-mixed", "run mixed actions");
        run.actions
            .push(terminal_test_action("pending-action", "needs_confirmation"));
        run.actions
            .push(terminal_test_action("failed-action", "failed"));
        let mut status_updates = Vec::new();

        let result = agent.handle_step_completion(
            false,
            false,
            Vec::new(),
            2,
            String::new(),
            &mut run,
            &mut status_updates,
            &AgentLoopConfig::default(),
        );

        assert_eq!(result.stop_reason, "tool_execution_failed");
        assert_eq!(run.status, AgentRunStatus::Failed);
        assert_eq!(
            run.error.as_ref().map(|error| error.message.as_str()),
            Some("agent_loop_terminal:tool_execution_failed")
        );
    }

    #[test]
    fn historical_pending_action_does_not_mask_current_batch_failure() {
        let agent = make_test_agent_loop();
        let mut run = AgentRun::new_chat_run("session-history", "run failing action");
        run.actions
            .push(terminal_test_action("old-pending", "needs_confirmation"));
        run.actions
            .push(terminal_test_action("current-failure", "failed"));
        let mut status_updates = Vec::new();

        let result = agent.handle_step_completion(
            false,
            false,
            Vec::new(),
            1,
            String::new(),
            &mut run,
            &mut status_updates,
            &AgentLoopConfig::default(),
        );

        assert_eq!(result.stop_reason, "tool_execution_failed");
        assert_eq!(run.status, AgentRunStatus::Failed);
    }

    #[test]
    fn main_chat_cloud_authorization_cannot_override_hs_local_only_policy() {
        let ingress = crate::agent::main_chat_agent_v1::AgentIngress::default().decide(
            "provider-intersection-session",
            "Explain focused work.",
            None,
            AgentTaskKind::Conversation,
        );
        let main_chat = ProviderPolicyAuthorization::from_main_chat_ingress(&ingress).unwrap();
        let main_chat_decision_id = main_chat.decision_id().to_string();
        assert_eq!(
            main_chat.data_route(),
            crate::llm::ProviderDataRoute::PolicyAllowed
        );

        let hs_decision = crate::agent::PolicyStore::mvp_builtin().evaluate_context_policy(
            crate::agent::PolicyEvaluationRequest {
                topic: crate::agent::PolicyTopic::Health,
                requested_route: crate::agent::ModelRoutePolicy::CloudAllowed,
            },
        );
        let policy_store_local = ProviderPolicyAuthorization::from_policy_store_context_decision(
            &hs_decision,
            "policy-store-local-intersection-decision",
        )
        .unwrap();

        let intersected = intersect_provider_authorizations(Some(main_chat), policy_store_local);
        assert_eq!(
            intersected.data_route(),
            crate::llm::ProviderDataRoute::LocalOnly
        );
        assert_eq!(
            intersected.authority(),
            crate::llm::ProviderPolicyAuthority::MainChatPolicyRouter
        );
        assert_eq!(intersected.decision_id(), main_chat_decision_id);
        assert_eq!(
            intersected.effective_local_restriction(),
            Some(crate::llm::ProviderLocalOnlyReason::CanonicalRouteIntersection)
        );
    }

    #[tokio::test]
    async fn prompt_wrapping_preserves_the_canonical_provider_policy_subject() {
        let user_text = "Use the governed read-only tool.";
        let ingress = crate::agent::main_chat_agent_v1::AgentIngress::default().decide(
            "provider-subject-session",
            user_text,
            None,
            AgentTaskKind::Conversation,
        );
        let authorization = ProviderPolicyAuthorization::from_main_chat_ingress(&ingress).unwrap();
        let mut task = AgentTask {
            kind: AgentTaskKind::Conversation,
            session_id: "provider-subject-session".into(),
            user_text: user_text.into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: user_text.into(),
            }],
            layer: crate::layer::Layer::L2,
        };
        wrap_user_content(&mut task);
        assert_ne!(task.user_text, user_text);

        let agent = make_scripted_provider_test_agent_loop();
        let network_policy = crate::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        let context = AgentLoopContext {
            task: &task,
            provider_subject_text: user_text,
            tools_prompt: "",
            memory_context: None,
            privacy_engine: PrivacyEngine::new(),
            provider_authorization: Some(authorization.clone()),
            network_policy: network_policy.clone(),
            policy_context: RuntimePolicyContext::fail_closed(),
        };

        let prepared = agent
            .prepare_provider_request(&context, task.messages.clone())
            .await
            .expect("compiled prompt remains derived from the canonical policy subject");
        assert!(prepared.messages.iter().any(|message| {
            message.role == "user" && message.content.starts_with(USER_REQUEST_START)
        }));

        let rebound_context = AgentLoopContext {
            provider_subject_text: &task.user_text,
            provider_authorization: Some(authorization),
            ..context
        };
        let error = agent
            .prepare_provider_request(&rebound_context, task.messages.clone())
            .await
            .expect_err("compiled prompt text cannot rebind the canonical policy subject");
        assert!(error.to_string().contains("subject mismatch"));
    }

    /// Create a minimal ActionExecutionContext backed by tempfile-based stores.
    struct TestCtx {
        registry: McpRegistry,
        permission_store: ToolPermissionStore,
        audit_store: McpAuditStore,
        privacy_engine: PrivacyEngine,
        safe_paths: Vec<String>,
    }

    impl TestCtx {
        fn new() -> Self {
            let tmp = tempfile::tempdir().unwrap();
            Self {
                registry: McpRegistry::new(),
                permission_store: ToolPermissionStore::new_in_memory().unwrap(),
                audit_store: McpAuditStore::new(tmp.path().join("audit.db")),
                privacy_engine: PrivacyEngine::new(),
                safe_paths: vec!["/tmp/openlife-test".into()],
            }
        }

        fn as_ctx(&self) -> ActionExecutionContext<'_> {
            ActionExecutionContext {
                registry: &self.registry,
                permission_store: &self.permission_store,
                audit_store: &self.audit_store,
                privacy_engine: &self.privacy_engine,
                safe_paths: &self.safe_paths,
                life_model: None,
                canonical_state: None,
                memory_store: None,
                memory_lifecycle_retrieval_reader: None,
                resource_store: None,
                proposal_store: None,
                agent_run_store: None,
                bound_content_receipt_issuer: None,
                network_policy: None,
                web_search_fixture_output: None,
                external_write_requires_proposal: true,
                tool_dispatch_observer: None,
                tool_started_transition_observer: None,
                tool_audit_persistence_observer: None,
                durable_store_failure_observer: None,
                a2a_outbound_authorization: None,
                canonical_write_admission: Some(
                    &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
                ),
                action_bound_tool_permission: None,
                calendar_ics_paths: &[],
            }
        }
    }

    #[derive(Default)]
    struct RecordingStreamingCallback {
        chunks: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl StreamingCallback for RecordingStreamingCallback {
        async fn on_chunk(&self, chunk: &str, _step: u32, _phase: &str) {
            self.chunks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(chunk.to_string());
        }

        async fn on_tool_start(&self, _tool_name: &str, _step: u32) {}

        async fn on_tool_result(&self, _tool_name: &str, _success: bool, _step: u32) {}

        async fn on_proposal(&self, _proposal_type: &str, _proposal_id: &str) {}

        async fn on_status(&self, _status: &str, _message: &str, _step: u32) {}
    }

    fn observed_provider_receipt(status: ProviderInvocationStatus) -> ProviderInvocationReceipt {
        ProviderInvocationReceipt {
            request_id: "stream-request-1".into(),
            provider: "openai".into(),
            model: "gpt-test".into(),
            started_at: chrono::Utc::now(),
            finished_at: chrono::Utc::now(),
            status,
            error_digest: (status != ProviderInvocationStatus::Completed)
                .then(|| format!("sha256:{}", "5".repeat(64))),
            simulated: false,
            policy_evidence: Some(crate::llm::ProviderPolicyReceiptEvidence {
                decision_id: "test-stream-policy".into(),
                policy_version: "test-policy-v1".into(),
                issuing_authority: crate::llm::ProviderPolicyAuthority::LocalOnlyFailClosed,
                effective_data_route: crate::llm::ProviderDataRoute::LocalOnly,
                effective_local_restriction: Some(crate::llm::ProviderLocalOnlyReason::TestFixture),
                subject_scope_digest: format!("sha256:{}", "0".repeat(64)),
                payload_purpose: Some(crate::llm::ProviderPayloadPurpose::AgentLoopStep),
                unfiltered_payload_digest: Some(format!("sha256:{}", "2".repeat(64))),
                context_manifest_digest: format!("sha256:{}", "1".repeat(64)),
                prepared_envelope_digest: Some(format!("sha256:{}", "3".repeat(64))),
                provider_config_generation: "test-provider-generation".into(),
                network_policy_decision_digest: format!("sha256:{}", "4".repeat(64)),
                selected_context_refs: Vec::new(),
                included_context_categories: Vec::new(),
                declared_payload_categories: vec![
                    crate::llm::ProviderPayloadCategory::FrozenEvaluationInput,
                ],
                policy_provenance_refs: Vec::new(),
                raw_life_model_included: false,
                raw_unbounded_memory_included: false,
            }),
        }
    }

    #[test]
    fn streaming_consumer_forwards_terminal_seam_truth_without_receipt_synthesis() {
        let source = include_str!("agent_loop.rs");
        let consumer = source
            .split("async fn consume_provider_stream")
            .nth(1)
            .and_then(|tail| tail.split("/// Parse model response").next())
            .expect("stream consumer source slice");

        assert!(!consumer.contains("ProviderInvocationReceipt {"));
        assert!(!consumer.contains("provider_error_terminal_status"));
        assert!(!consumer.contains("chrono::Utc::now"));
        for removed in [
            ["emit_stream_", "terminal_receipt"].concat(),
            ["StartedProvider", "Stream"].concat(),
            ["retain_stream_", "terminal_receipt"].concat(),
        ] {
            assert!(
                !source.contains(&removed),
                "old synthesis route remains: {removed}"
            );
        }
    }

    #[tokio::test]
    async fn scripted_stream_terminal_remains_not_attempted_without_provider_progress() {
        let agent = make_test_agent_loop();
        let callback = Arc::new(RecordingStreamingCallback::default());
        let stream: PreparedProviderStream = Box::pin(futures::stream::iter(vec![
            PreparedProviderStreamEvent::Token("scripted".into()),
            PreparedProviderStreamEvent::Terminal(PreparedProviderStreamTerminal::NotAttempted),
        ]));
        let mut progress = Vec::new();
        let mut observer = |event| {
            progress.push(event);
            Ok(())
        };

        let reply = agent
            .consume_provider_stream(stream, callback, &mut observer)
            .await
            .expect("scripted not-attempted stream");

        assert_eq!(reply, "scripted");
        assert!(progress.is_empty());
        assert!(agent.scheduler.provider_receipts_snapshot().is_empty());
    }

    #[tokio::test]
    async fn reasoning_only_stream_error_is_failed_receipt_not_success_reply() {
        let agent = make_test_agent_loop();
        let callback = Arc::new(RecordingStreamingCallback::default());
        let stream: PreparedProviderStream = Box::pin(futures::stream::iter(vec![
            PreparedProviderStreamEvent::Terminal(PreparedProviderStreamTerminal::Failed {
                receipt: Box::new(observed_provider_receipt(ProviderInvocationStatus::Failed)),
                error: "provider_reasoning_without_final_content".into(),
            }),
        ]));
        let mut progress = Vec::new();
        let mut observer = |event| {
            progress.push(event);
            Ok(())
        };

        let error = agent
            .consume_provider_stream(stream, callback, &mut observer)
            .await
            .expect_err("reasoning-only stream must fail");

        assert!(error
            .to_string()
            .contains("provider_reasoning_without_final_content"));
        assert!(matches!(
            progress.as_slice(),
            [ProviderInvocationProgress::Failed(receipt)]
                if receipt.status == ProviderInvocationStatus::Failed
                    && receipt.error_digest.is_some()
        ));
        assert!(
            agent.scheduler.provider_receipts_snapshot().is_empty(),
            "a manually supplied stream terminal must not fabricate a second scheduler receipt"
        );
    }

    #[tokio::test]
    async fn mid_stream_error_never_promotes_partial_reply_to_success() {
        let agent = make_test_agent_loop();
        let callback = Arc::new(RecordingStreamingCallback::default());
        let stream: PreparedProviderStream = Box::pin(futures::stream::iter(vec![
            PreparedProviderStreamEvent::Token("partial".to_string()),
            PreparedProviderStreamEvent::Terminal(PreparedProviderStreamTerminal::RemoteUnknown {
                receipt: Box::new(observed_provider_receipt(
                    ProviderInvocationStatus::RemoteUnknown,
                )),
                error: "provider transport reset".into(),
            }),
        ]));
        let mut progress = Vec::new();
        let mut observer = |event| {
            progress.push(event);
            Ok(())
        };

        let result = agent
            .consume_provider_stream(stream, callback.clone(), &mut observer)
            .await;

        assert!(result.is_err(), "partial output cannot become an Ok reply");
        assert_eq!(
            callback
                .chunks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_slice(),
            ["partial"]
        );
        assert!(matches!(
            progress.as_slice(),
            [ProviderInvocationProgress::RemoteUnknown(receipt)]
                if receipt.status == ProviderInvocationStatus::RemoteUnknown
        ));
        assert!(!progress
            .iter()
            .any(|event| matches!(event, ProviderInvocationProgress::Completed(_))));
    }

    #[tokio::test]
    async fn successful_stream_retains_completed_terminal_receipt() {
        let agent = make_test_agent_loop();
        let callback = Arc::new(RecordingStreamingCallback::default());
        let expected_receipt = observed_provider_receipt(ProviderInvocationStatus::Completed);
        let stream: PreparedProviderStream = Box::pin(futures::stream::iter(vec![
            PreparedProviderStreamEvent::Token("hello ".to_string()),
            PreparedProviderStreamEvent::Token("world".to_string()),
            PreparedProviderStreamEvent::Terminal(PreparedProviderStreamTerminal::Completed(
                Box::new(expected_receipt.clone()),
            )),
        ]));
        let mut progress = Vec::new();
        let mut observer = |event| {
            progress.push(event);
            Ok(())
        };

        let reply = agent
            .consume_provider_stream(stream, callback, &mut observer)
            .await
            .expect("successful stream");

        assert_eq!(reply, "hello world");
        assert!(matches!(
            progress.as_slice(),
            [ProviderInvocationProgress::Completed(receipt)]
                if receipt == &expected_receipt
        ));
    }

    #[tokio::test]
    async fn existing_canonical_run_identity_is_reused_by_agent_loop() {
        let agent = make_test_agent_loop()
            .with_scripted_replies(vec![r#"{"final":"canonical result"}"#.into()]);
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let task = AgentTask {
            kind: AgentTaskKind::Conversation,
            session_id: "canonical-session".into(),
            user_text: "hello".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "hello".into(),
            }],
            layer: crate::layer::Layer::L2,
        };
        let mut canonical_run = AgentRun::new_chat_run(&task.session_id, &task.user_text);
        canonical_run.id = "canonical-persisted-run".into();
        canonical_run.task_id = "canonical-persisted-task".into();
        let mut progress = Vec::new();
        let mut observer = |event| {
            progress.push(event);
            Ok(())
        };

        let result = agent
            .run_existing_with_provider_observer(
                AgentLoopRunRequest::new(
                    &task,
                    "",
                    None,
                    PrivacyEngine::new(),
                    &action_ctx,
                    RuntimePolicyContext::fail_closed(),
                ),
                canonical_run,
                &mut observer,
            )
            .await
            .expect("canonical run should drive the AgentLoop");

        assert_eq!(result.run.id, "canonical-persisted-run");
        assert_eq!(result.run.task_id, "canonical-persisted-task");
        assert!(
            progress.is_empty(),
            "scripted reply performs no provider I/O"
        );
    }

    #[test]
    fn existing_canonical_run_future_remains_stack_bounded() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let task = AgentTask {
            kind: AgentTaskKind::Conversation,
            session_id: "bounded-agent-loop-session".into(),
            user_text: "hello".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "hello".into(),
            }],
            layer: crate::layer::Layer::L2,
        };
        let canonical_run = AgentRun::new_chat_run(&task.session_id, &task.user_text);
        let mut observer = |_: ProviderInvocationProgress| Ok(());
        let future = agent.run_existing_with_provider_observer(
            AgentLoopRunRequest::new(
                &task,
                "",
                None,
                PrivacyEngine::new(),
                &action_ctx,
                RuntimePolicyContext::fail_closed(),
            ),
            canonical_run,
            &mut observer,
        );
        let future_size = std::mem::size_of_val(&future);

        assert!(
            future_size <= 8 * 1024,
            "canonical AgentLoop seam regressed to an oversized inline future: {future_size} bytes"
        );
    }

    // ── parse_agent_reply tests ──────────────────────────────────────────

    #[test]
    fn parse_final_only_no_json() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 0;

        let result = agent
            .parse_agent_reply("Hello, how can I help?", &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(!result.json_parse_failed);
        assert_eq!(result.final_text, "Hello, how can I help?");
        assert!(result.actions.is_empty());
    }

    #[test]
    fn parse_json_plain_final() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 0;

        let reply = r#"{"final": "Here is my answer"}"#;
        let result = agent
            .parse_agent_reply(reply, &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(!result.json_parse_failed);
        assert_eq!(result.final_text, "Here is my answer");
        assert!(result.actions.is_empty());
    }

    #[test]
    fn parse_json_with_actions() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 0;

        let reply = r#"{
            "final": "Let me check that for you",
            "actions": [
                {"name": "web.search", "arguments": {"query": "Rust async"}},
                {"name": "file.read", "arguments": {"path": "/tmp/test.txt"}}
            ]
        }"#;
        let result = agent
            .parse_agent_reply(reply, &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(!result.json_parse_failed);
        assert_eq!(result.final_text, "Let me check that for you");
        assert_eq!(result.actions.len(), 2);
        assert_eq!(result.actions[0].target, "web.search");
        assert_eq!(result.actions[1].target, "file.read");
        // step_index should start from tool_call_count (0)
        assert_eq!(result.actions[0].step_index, 0);
        assert_eq!(result.actions[1].step_index, 1);
    }

    #[test]
    fn parse_session_search_binds_current_conversation_as_excluded_owner() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("current-conversation", "find prior context");
        let mut tool_call_count = 0;

        let result = agent
            .parse_agent_reply(
                r#"{
                    "final": "I will search prior conversations.",
                    "actions": [{
                        "name": "session.search",
                        "action_type": "session_search",
                        "arguments": {
                            "query": "Agent memory",
                            "session_id": "model-selected-session",
                            "limit": 5
                        }
                    }]
                }"#,
                &action_ctx,
                &mut run,
                &mut tool_call_count,
            )
            .unwrap();

        assert_eq!(result.actions.len(), 1);
        assert_eq!(
            result.actions[0]
                .input
                .get("exclude_session_id")
                .and_then(Value::as_str),
            Some("current-conversation")
        );
        assert!(result.actions[0].input.get("session_id").is_none());
    }

    #[test]
    fn toolset_allowlist_rejects_prefixed_target_aliases() {
        let agent = make_test_agent_loop();
        let config = AgentLoopConfig {
            allow_writes: false,
            toolset_allowlist: vec!["memory.search".into()],
            ..Default::default()
        };
        let exact_action = AgentActionRequest {
            action_type: "memory_search".into(),
            target: "memory.search".into(),
            input: serde_json::json!({ "query": "safe" }),
            source_run_id: None,
            step_index: 0,
        };
        let prefixed_alias_action = AgentActionRequest {
            action_type: "memory_search".into(),
            target: "memory.search.exfiltrate".into(),
            input: serde_json::json!({ "query": "unsafe" }),
            source_run_id: None,
            step_index: 1,
        };

        let (allowed, rejected_count) =
            agent.partition_tools_by_allowlist(vec![exact_action, prefixed_alias_action], &config);

        assert_eq!(allowed.len(), 1);
        assert_eq!(allowed[0].target, "memory.search");
        assert_eq!(
            rejected_count, 1,
            "toolset allowlist must reject prefixed aliases instead of treating them as governed targets"
        );
    }

    #[test]
    fn parse_json_legacy_tool_calls() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 5;

        let reply = r#"{
            "final": "Done",
            "tool_calls": [
                {"name": "echo", "args": {"msg": "hi"}}
            ]
        }"#;
        let result = agent
            .parse_agent_reply(reply, &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(!result.json_parse_failed);
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].target, "echo");
        assert_eq!(result.actions[0].step_index, 5);
    }

    #[test]
    fn parse_json_markdown_wrapped() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 0;

        let reply = r#"```json
{"final": "Answer from markdown", "actions": []}
```"#;
        let result = agent
            .parse_agent_reply(reply, &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(!result.json_parse_failed);
        assert_eq!(result.final_text, "Answer from markdown");
        assert!(result.actions.is_empty());
    }

    #[test]
    fn parse_malformed_json_signals_repair() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 0;

        // Missing closing brace
        let reply = r#"{"final": "oops", "actions": [{"name": "x", "arguments": {}]"#;
        let result = agent
            .parse_agent_reply(reply, &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(result.json_parse_failed, "should signal repair needed");
        assert!(result.actions.is_empty());
        assert!(!run.warnings.is_empty(), "should have recorded warning");
    }

    #[test]
    fn react_beta_parse_missing_tool_name_fails_soft_without_raw_reply() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 0;

        let reply = r#"{
            "final": "checking",
            "actions": [
                {"arguments": {"query": "secret@example.com raw prompt"}}
            ]
        }"#;
        let result = agent
            .parse_agent_reply(reply, &action_ctx, &mut run, &mut tc)
            .unwrap();

        assert!(!result.json_parse_failed);
        assert!(result.actions.is_empty());
        let warnings = serde_json::to_string(&run.warnings).unwrap();
        assert!(warnings.contains("missing_tool_name"));
        assert!(!warnings.contains("secret@example.com"));
        assert!(!warnings.contains("raw prompt"));
    }

    #[test]
    fn react_beta_parse_invalid_arguments_defaults_empty_and_records_warning() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 2;

        let reply = r#"{
            "final": "checking",
            "actions": [
                {"name": "memory.search", "arguments": "not an object"}
            ]
        }"#;
        let result = agent
            .parse_agent_reply(reply, &action_ctx, &mut run, &mut tc)
            .unwrap();

        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].step_index, 2);
        assert_eq!(
            result.actions[0].input,
            serde_json::json!({ "arguments": {} })
        );
        assert!(run
            .warnings
            .iter()
            .any(|warning| warning.contains("invalid_arguments_defaulted_empty")));
    }

    #[test]
    fn react_beta_broad_tools_prompt_text_alone_does_not_create_actions() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 0;

        let result = agent
            .parse_agent_reply(
                "Available tools: memory.search, file.write_proposal. Use them when useful.",
                &action_ctx,
                &mut run,
                &mut tc,
            )
            .unwrap();

        assert!(result.actions.is_empty());
        assert_eq!(
            result.final_text,
            "Available tools: memory.search, file.write_proposal. Use them when useful."
        );
    }

    #[test]
    fn parse_empty_actions_array_yields_final_only() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 0;

        let reply = r#"{"final": "done", "actions": []}"#;
        let result = agent
            .parse_agent_reply(reply, &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(!result.json_parse_failed);
        assert!(result.actions.is_empty());
    }

    #[test]
    fn parse_thought_summary_and_warnings_recorded() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 0;

        let reply = r#"{
            "final": "ok",
            "thought_summary": "simple task",
            "warnings": ["low confidence"]
        }"#;
        let result = agent
            .parse_agent_reply(reply, &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(!result.json_parse_failed);
        assert!(run.warnings.iter().any(|w| w.contains("thought")));
        assert!(run.warnings.iter().any(|w| w.contains("low confidence")));
    }

    #[test]
    fn parse_action_with_alternative_field_names() {
        let agent = make_test_agent_loop();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("s1", "hello");
        let mut tc: u32 = 0;

        // Uses "tool" instead of "name" and "input" instead of "arguments"
        let reply = r#"{"final":"ok","actions":[{"tool":"test_tool","input":{"key":"val"}}]}"#;
        let result = agent
            .parse_agent_reply(reply, &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(!result.json_parse_failed);
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].target, "test_tool");
    }

    // ── build_follow_up_messages tests ───────────────────────────────────

    #[test]
    fn build_follow_up_with_observations() {
        let agent = make_test_agent_loop();
        let task = crate::agent::types::AgentTask {
            kind: crate::agent::types::AgentTaskKind::Conversation,
            session_id: "s1".into(),
            user_text: "帮我查天气".into(),
            messages: vec![],
            layer: crate::layer::Layer::L2,
        };
        let obs = vec![AgentObservation {
            id: "obs-1".into(),
            action_id: Some("act-1".into()),
            content: "北京今天晴，25°C".into(),
            source: "web.search".into(),
            structured_result: None,
            timestamp: chrono::Utc::now(),
            react_trace: None,
        }];
        let tools_prompt = "可用工具: web.search, file.read";

        let messages = agent.build_follow_up_messages(&task, "正在查询...", &obs, tools_prompt);

        assert_eq!(messages.len(), 2); // assistant + user (follow-up)
        assert_eq!(messages[0].role, "assistant");
        assert_eq!(messages[0].content, "正在查询...");
        assert_eq!(messages[1].role, "user");
        assert!(messages[1].content.contains("帮我查天气"));
        assert!(messages[1].content.contains("北京今天晴"));
        assert!(messages[1].content.contains("web.search"));
    }

    #[test]
    fn build_follow_up_no_observations() {
        let agent = make_test_agent_loop();
        let task = crate::agent::types::AgentTask {
            kind: crate::agent::types::AgentTaskKind::Conversation,
            session_id: "s1".into(),
            user_text: "hello".into(),
            messages: vec![],
            layer: crate::layer::Layer::L2,
        };

        let messages = agent.build_follow_up_messages(&task, "Hi there!", &[], "可用工具: echo");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, "user");
        assert!(!messages[1].content.contains("工具执行结果"));
        assert!(messages[1].content.contains("可用工具"));
    }

    #[test]
    fn build_follow_up_preserves_existing_messages() {
        let agent = make_test_agent_loop();
        let task = crate::agent::types::AgentTask {
            kind: crate::agent::types::AgentTaskKind::Conversation,
            session_id: "s1".into(),
            user_text: "天气".into(),
            messages: vec![
                ChatMessage {
                    role: "user".into(),
                    content: "你好".into(),
                },
                ChatMessage {
                    role: "assistant".into(),
                    content: "你好！有什么可以帮你的？".into(),
                },
            ],
            layer: crate::layer::Layer::L2,
        };

        let obs = vec![AgentObservation {
            id: "obs-1".into(),
            action_id: None,
            content: "上海25°C".into(),
            source: "web.search".into(),
            structured_result: None,
            timestamp: chrono::Utc::now(),
            react_trace: None,
        }];

        let messages = agent.build_follow_up_messages(&task, "查询天气中...", &obs, "工具: web");

        // Original 2 + assistant + follow-up = 4
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "你好");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[3].role, "user");
    }

    // ── preview_text tests (existing) ────────────────────────────────────

    #[test]
    fn preview_text_truncates_on_char_boundary() {
        let text = format!("{}星", "a".repeat(199));
        assert_eq!(preview_text(&text, 200), text);

        let text = format!("{}星期几", "a".repeat(199));
        let preview = preview_text(&text, 200);
        assert!(preview.ends_with("星..."));
    }

    #[test]
    fn preview_text_handles_emoji_without_panic() {
        let text = format!("{}😀more", "a".repeat(199));
        let preview = preview_text(&text, 200);
        assert!(preview.ends_with("😀..."));
    }
}
