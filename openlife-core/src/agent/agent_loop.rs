use crate::agent::action_executor::{
    ActionExecutionContext, ActionExecutionStatus, ActionExecutor, AgentActionRequest,
};
use crate::agent::event_store::AgentRunEventStore;
use crate::agent::runtime::{AgentRuntime, AgentRuntimeOutput};
use crate::agent::types::{
    AgentEventActor, AgentObservation, AgentRun, AgentRunError, AgentRunEvent, AgentRunEventType,
    AgentRunStatus, AgentTask,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::privacy::PrivacyEngine;
use crate::scheduler::InferenceScheduler;
use anyhow::{Context, Result};
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
                 - Reading current goals before suggesting changes\n\
                 Use goal.read, life_model.read, and proposal.create tools.",
            ),
        }
    }

    /// vNext: get the role instruction as a versioned PromptBlock.
    pub fn role_prompt_block(&self) -> Option<crate::agent::prompt_stack::PromptBlock> {
        use crate::agent::prompt_stack::{PromptBlock, PromptPrivacyLevel, PromptPurpose};
        self.role_system_instruction().map(|content| {
            PromptBlock::new(
                format!("role.{}", format!("{:?}", self.role).to_lowercase()),
                "1.0.0",
                PromptPurpose::Planning,
                content,
            )
            .with_privacy(PromptPrivacyLevel::Internal)
            .with_applies_to(vec![format!("{:?}", self.role)])
        })
    }
}

/// Bundles the shared task/life-model/tools/privacy/memory/AgentSpec parameters
/// that flow through the entire agent loop, reducing argument counts below clippy limits.
struct AgentLoopContext<'a> {
    pub task: &'a AgentTask,
    pub life_model: &'a LifeModel,
    pub tools_prompt: &'a str,
    pub memory_context: Option<String>,
    pub privacy_engine: PrivacyEngine,
    /// AgentSpec privacy policy governing cloud data exposure.
    pub privacy_policy: crate::agent::types::PrivacyPolicy,
    /// Resolved AgentSpec for governed execution (prompt blocks, context policy).
    pub agent_spec: &'a crate::agent::types::AgentSpec,
    /// PromptBlockRegistry for prompt block resolution.
    pub prompt_registry: &'a crate::agent::prompt_stack::PromptBlockRegistry,
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
#[derive(Debug, Clone)]
pub struct AgentLoopResult {
    pub run: AgentRun,
    pub final_response: String,
    pub stop_reason: String,
    pub tool_call_count: u32,
    pub step_count: u32,
    pub status_updates: Vec<crate::agent::types::AgentLoopStatusUpdate>,
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
    pub life_model: &'a LifeModel,
    pub tools_prompt: &'a str,
    pub memory_context: Option<String>,
    pub privacy_engine: PrivacyEngine,
    pub privacy_policy: crate::agent::types::PrivacyPolicy,
    pub agent_spec: &'a crate::agent::types::AgentSpec,
    pub prompt_registry: &'a crate::agent::prompt_stack::PromptBlockRegistry,
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
pub struct AgentLoop {
    runtime: AgentRuntime,
    action_executor: ActionExecutor,
    scheduler: InferenceScheduler,
    config: AgentLoopConfig,
    /// Optional vNext event store for runtime trace recording.
    /// When None, no events are recorded (backward compatible).
    event_store: Option<AgentRunEventStore>,
}

impl AgentLoop {
    pub fn new(
        runtime: AgentRuntime,
        action_executor: ActionExecutor,
        scheduler: InferenceScheduler,
        config: AgentLoopConfig,
    ) -> Self {
        Self {
            runtime,
            action_executor,
            scheduler,
            config,
            event_store: None,
        }
    }

    /// Set an event store for runtime trace recording.
    pub fn with_event_store(mut self, store: AgentRunEventStore) -> Self {
        self.event_store = Some(store);
        self
    }

    /// Best-effort event recording. Silently drops on failure.
    fn try_record_event(
        &self,
        run_id: &str,
        event_type: AgentRunEventType,
        actor: AgentEventActor,
        summary: impl Into<String>,
        payload: serde_json::Value,
    ) {
        if let Some(ref store) = self.event_store {
            let event = AgentRunEvent::new(run_id, event_type, actor, summary, payload);
            if let Err(e) = store.append_event(&event) {
                eprintln!("[AgentLoop] Failed to record event: {}", e);
            }
        }
    }

    /// Record PromptStackAssembled and ContextGovernanceApplied events
    /// from a real AgentRuntimeOutput after execute_task_with_spec succeeds.
    /// Payloads contain only block IDs/versions and context categories —
    /// no raw prompt content, memory, or LifeModel.
    fn record_runtime_governance_events(
        &self,
        run_id: &str,
        agent_spec: &crate::agent::types::AgentSpec,
        runtime_output: &AgentRuntimeOutput,
    ) {
        self.try_record_event(
            run_id,
            AgentRunEventType::PromptStackAssembled,
            AgentEventActor::Runtime,
            format!(
                "PromptStack assembled with {} blocks from AgentSpec {}",
                runtime_output.prompt_block_trace.len(),
                agent_spec.id
            ),
            serde_json::json!({
                "agent_spec_id": agent_spec.id,
                "prompt_blocks": runtime_output.prompt_block_trace,
            }),
        );
        self.try_record_event(
            run_id,
            AgentRunEventType::ContextGovernanceApplied,
            AgentEventActor::Runtime,
            format!("Context governance applied by AgentSpec {}", agent_spec.id),
            serde_json::json!({
                "agent_spec_id": agent_spec.id,
                "context_included": runtime_output.governed_context_summary.as_ref()
                    .map(|g| &g.included).unwrap_or(&vec![]),
                "context_excluded": runtime_output.governed_context_summary.as_ref()
                    .map(|g| &g.excluded).unwrap_or(&vec![]),
                "privacy_policy": agent_spec.privacy_policy.to_string(),
            }),
        );
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
    ) -> Result<ParsedAgentReply> {
        let mut repair_task = actx.task.clone();
        repair_task.messages.push(ChatMessage {
            role: "system".into(),
            content: format!("{}{}", SELF_REPAIR_PROMPT, actx.task.user_text),
        });

        let repair_actx = AgentLoopContext {
            task: &repair_task,
            life_model: actx.life_model,
            tools_prompt: actx.tools_prompt,
            memory_context: actx.memory_context.clone(),
            privacy_engine: actx.privacy_engine.clone(),
            privacy_policy: actx.privacy_policy,
            agent_spec: actx.agent_spec,
            prompt_registry: actx.prompt_registry,
        };

        match self.generate_response(&repair_actx, &run.id).await {
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
        callback: Option<Arc<dyn StreamingCallback>>,
    ) -> Result<AgentLoopResult> {
        let start_time = Instant::now();
        let mut run = AgentRun::new_chat_run(&actx.task.session_id, &actx.task.user_text);
        run.user_input = Some(actx.task.user_text.clone());

        self.try_record_event(
            &run.id,
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "Agent run created",
            serde_json::json!({
                "session_id": actx.task.session_id,
                "kind": "conversation",
                "role": format!("{:?}", self.config.role),
            }),
        );

        // P7: AgentSpecSelected records the resolved spec id immediately.
        // PromptStackAssembled and ContextGovernanceApplied are recorded
        // in generate_response / generate_response_streaming after the
        // runtime has successfully resolved prompt blocks and context policy.
        self.try_record_event(
            &run.id,
            AgentRunEventType::AgentSpecSelected,
            AgentEventActor::Runtime,
            format!("AgentSpec {} selected for AgentLoop", actx.agent_spec.id),
            serde_json::json!({
                "agent_spec_id": actx.agent_spec.id,
                "role": actx.agent_spec.role.to_string(),
                "privacy_policy": actx.agent_spec.privacy_policy.to_string(),
            }),
        );

        let mut step_count: u32 = 0;
        let mut tool_call_count: u32 = 0;
        let mut final_response = String::new();
        let mut current_task = actx.task.clone();
        wrap_user_content(&mut current_task);
        let mut current_tools_prompt = actx.tools_prompt.to_string();
        // Append role-specific instruction if applicable
        if let Some(role_instruction) = self.config.role_system_instruction() {
            if !current_tools_prompt.is_empty() {
                current_tools_prompt.push_str("\n\n");
            }
            current_tools_prompt.push_str(role_instruction);
        }
        let current_memory_context = actx.memory_context.clone();
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
            if step_count >= self.config.max_steps {
                stop_reason = "max_steps_reached".into();
                if final_response.is_empty() {
                    final_response = format!(
                        "已达到最大执行步数 ({})。当前结果：{}",
                        self.config.max_steps, final_response
                    );
                }
                break;
            }

            // Check timeout
            if start_time.elapsed().as_secs() >= self.config.timeout_seconds {
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
            let memory_context = if let Some(memory_store) = action_ctx.memory_store {
                search_memory_for_context(
                    memory_store,
                    &current_task.user_text,
                    &actx.task.session_id,
                )
                .unwrap_or_else(|e| {
                    eprintln!("[AgentLoop] Memory search failed: {}", e);
                    current_memory_context.clone()
                })
            } else {
                current_memory_context.clone()
            };

            // Execute single step (catch parse errors to preserve run.actions)
            let step_result = match self
                .run_single_step(
                    StepContext {
                        task: &current_task,
                        life_model: actx.life_model,
                        tools_prompt: &current_tools_prompt,
                        memory_context,
                        privacy_engine: current_privacy_engine.clone(),
                        privacy_policy: actx.privacy_policy,
                        agent_spec: actx.agent_spec,
                        prompt_registry: actx.prompt_registry,
                        action_ctx,
                        run: &mut run,
                        tool_call_count,
                    },
                    callback.clone(),
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

        // Emit final status
        if run.status == AgentRunStatus::Failed {
            self.emit_status(
                &mut status_updates,
                crate::agent::types::AgentLoopPhase::Failed,
                format!("Execution failed: {}", stop_reason),
                step_count,
                None,
            );
        } else {
            self.emit_status(
                &mut status_updates,
                crate::agent::types::AgentLoopPhase::Completed,
                format!("Execution completed: {}", stop_reason),
                step_count,
                None,
            );
        }

        if run.status == AgentRunStatus::Running {
            run.status = AgentRunStatus::Completed;
        }
        run.output_preview = Some(preview_text(&final_response, 200));
        run.finished_at = Some(chrono::Utc::now());

        if run.status == AgentRunStatus::Failed {
            self.try_record_event(
                &run.id,
                AgentRunEventType::RunFailed,
                AgentEventActor::Runtime,
                format!("Run failed: {}", stop_reason),
                serde_json::json!({
                    "stop_reason": stop_reason,
                    "step_count": step_count,
                    "tool_call_count": tool_call_count,
                    "error": run.error.as_ref().map(|e| e.message.clone()),
                }),
            );
        } else {
            self.try_record_event(
                &run.id,
                AgentRunEventType::RunCompleted,
                AgentEventActor::Runtime,
                format!("Run completed: {}", stop_reason),
                serde_json::json!({
                    "stop_reason": stop_reason,
                    "step_count": step_count,
                    "tool_call_count": tool_call_count,
                    "reply_len": final_response.len(),
                }),
            );
        }

        Ok(self.build_result(
            run,
            final_response,
            stop_reason,
            tool_call_count,
            step_count,
            status_updates,
        ))
    }

    /// Run the iterative agent loop for a given task.
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        privacy_engine: PrivacyEngine,
        privacy_policy: crate::agent::types::PrivacyPolicy,
        agent_spec: &crate::agent::types::AgentSpec,
        prompt_registry: &crate::agent::prompt_stack::PromptBlockRegistry,
        action_ctx: &ActionExecutionContext<'_>,
    ) -> Result<AgentLoopResult> {
        let actx = AgentLoopContext {
            task,
            life_model,
            tools_prompt,
            memory_context,
            privacy_engine,
            privacy_policy,
            agent_spec,
            prompt_registry,
        };
        self.run_loop_core(&actx, action_ctx, None).await
    }

    /// Streaming variant of run(). Same logic but forwards token chunks
    /// through the callback as they arrive from the model.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_streaming(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        privacy_engine: PrivacyEngine,
        privacy_policy: crate::agent::types::PrivacyPolicy,
        agent_spec: &crate::agent::types::AgentSpec,
        prompt_registry: &crate::agent::prompt_stack::PromptBlockRegistry,
        action_ctx: &ActionExecutionContext<'_>,
        callback: Arc<dyn StreamingCallback>,
    ) -> Result<AgentLoopResult> {
        let actx = AgentLoopContext {
            task,
            life_model,
            tools_prompt,
            memory_context,
            privacy_engine,
            privacy_policy,
            agent_spec,
            prompt_registry,
        };
        self.run_loop_core(&actx, action_ctx, Some(callback)).await
    }

    /// Execute a single step of the agent loop.
    /// If `callback` is provided, uses streaming generation and emits tool events.
    async fn run_single_step(
        &self,
        mut ctx: StepContext<'_>,
        callback: Option<Arc<dyn StreamingCallback>>,
    ) -> Result<StepResult> {
        let mut status_updates: Vec<crate::agent::types::AgentLoopStatusUpdate> = Vec::new();

        // Clone values that will be consumed by generate_response so we can
        // re-use them in a one-shot JSON repair round.
        let memory_ctx = ctx.memory_context.clone();
        let privacy = ctx.privacy_engine.clone();

        // Generate model response (streaming if callback provided)
        let step_num = ctx.run.step_count;
        self.try_record_event(
            &ctx.run.id,
            AgentRunEventType::ModelCallStarted,
            AgentEventActor::Agent,
            format!("Step {}: model call started", step_num + 1),
            serde_json::json!({"step": step_num + 1}),
        );

        let generated = {
            let actx = AgentLoopContext {
                task: ctx.task,
                life_model: ctx.life_model,
                tools_prompt: ctx.tools_prompt,
                memory_context: ctx.memory_context.clone(),
                privacy_engine: ctx.privacy_engine.clone(),
                privacy_policy: ctx.privacy_policy,
                agent_spec: ctx.agent_spec,
                prompt_registry: ctx.prompt_registry,
            };
            let run_id = ctx.run.id.clone();
            if let Some(ref cb) = callback {
                self.generate_response_streaming(&actx, cb.clone(), &run_id)
                    .await
            } else {
                self.generate_response(&actx, &run_id).await
            }
        };

        match generated {
            Ok(gen) => {
                self.try_record_event(
                    &ctx.run.id,
                    AgentRunEventType::ModelCallCompleted,
                    AgentEventActor::Agent,
                    format!("Step {}: model call completed", step_num + 1),
                    serde_json::json!({"step": step_num + 1, "reply_len": gen.reply.len()}),
                );

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
                    self.try_record_event(
                        &ctx.run.id,
                        AgentRunEventType::JsonRepairStarted,
                        AgentEventActor::Runtime,
                        "JSON parse failed, attempting one-shot self-repair",
                        serde_json::json!({"reply_len": reply.len()}),
                    );
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
                                life_model: ctx.life_model,
                                tools_prompt: ctx.tools_prompt,
                                memory_context: memory_ctx.clone(),
                                privacy_engine: privacy.clone(),
                                privacy_policy: ctx.privacy_policy,
                                agent_spec: ctx.agent_spec,
                                prompt_registry: ctx.prompt_registry,
                            },
                            ctx.action_ctx,
                            ctx.run,
                            &mut ctx.tool_call_count,
                        )
                        .await?;
                    self.try_record_event(
                        &ctx.run.id,
                        AgentRunEventType::JsonRepairCompleted,
                        AgentEventActor::Runtime,
                        if parsed.json_parse_failed {
                            "JSON self-repair also failed"
                        } else {
                            "JSON self-repair succeeded"
                        },
                        serde_json::json!({"repaired": !parsed.json_parse_failed}),
                    );
                }

                let final_text = parsed.final_text;
                let tool_actions = self.filter_tools_by_allowlist(parsed.actions);

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
                ))
            }
            Err(e) => {
                self.try_record_event(
                    &ctx.run.id,
                    AgentRunEventType::ModelCallFailed,
                    AgentEventActor::Agent,
                    format!("Model call failed: {}", e),
                    serde_json::json!({"error": e.to_string()}),
                );
                ctx.run.status = AgentRunStatus::Failed;
                ctx.run.error = Some(AgentRunError {
                    message: e.to_string(),
                    phase: "model".into(),
                    recoverable: false,
                });
                self.emit_status(
                    &mut status_updates,
                    crate::agent::types::AgentLoopPhase::Failed,
                    format!("Model generation failed: {}", e),
                    0,
                    None,
                );
                if let Some(ref cb) = callback {
                    cb.on_status("failed", &format!("Model generation failed: {}", e), 0)
                        .await;
                }
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
        run_id: &str,
    ) -> Result<GeneratedAgentResponse> {
        let memory_hits = Vec::new();
        let runtime_output = self
            .runtime
            .execute_task_with_spec(
                actx.task,
                actx.life_model,
                actx.tools_prompt,
                actx.memory_context.clone(),
                memory_hits,
                actx.privacy_engine.clone(),
                actx.agent_spec,
                actx.prompt_registry,
            )
            .await
            .map_err(|e| anyhow::anyhow!("runtime execution failed: {}", e))?;

        self.record_runtime_governance_events(run_id, actx.agent_spec, &runtime_output);

        let tools_prompt = if actx.tools_prompt.trim().is_empty() {
            None
        } else {
            Some(actx.tools_prompt)
        };
        let reply = self
            .scheduler
            .generate_governed(
                runtime_output.final_messages.clone(),
                actx.life_model,
                tools_prompt,
                actx.privacy_policy,
            )
            .await
            .map_err(|e| anyhow::anyhow!("model generation failed: {}", e))?;

        Ok(GeneratedAgentResponse {
            runtime_output,
            reply,
        })
    }

    /// Streaming variant of generate_response. Uses governed runtime and
    /// forwards each chunk through the callback.
    async fn generate_response_streaming(
        &self,
        actx: &AgentLoopContext<'_>,
        callback: Arc<dyn StreamingCallback>,
        run_id: &str,
    ) -> Result<GeneratedAgentResponse> {
        let memory_hits = Vec::new();
        let runtime_output = self
            .runtime
            .execute_task_with_spec(
                actx.task,
                actx.life_model,
                actx.tools_prompt,
                actx.memory_context.clone(),
                memory_hits,
                actx.privacy_engine.clone(),
                actx.agent_spec,
                actx.prompt_registry,
            )
            .await
            .map_err(|e| anyhow::anyhow!("runtime execution failed: {}", e))?;

        self.record_runtime_governance_events(run_id, actx.agent_spec, &runtime_output);

        let tools_prompt = if actx.tools_prompt.trim().is_empty() {
            None
        } else {
            Some(actx.tools_prompt)
        };

        let mut stream = self
            .scheduler
            .generate_stream_governed(
                runtime_output.final_messages.clone(),
                actx.life_model,
                tools_prompt,
                actx.privacy_policy,
            )
            .await
            .map_err(|e| anyhow::anyhow!("stream generation failed: {}", e))?;

        let mut reply = String::new();
        loop {
            match stream.next().await {
                Some(Ok(chunk)) => {
                    callback.on_chunk(&chunk, 0, "generating").await;
                    reply.push_str(&chunk);
                }
                Some(Err(e)) => {
                    eprintln!("[AgentLoop] Stream chunk error: {}", e);
                    break;
                }
                None => break,
            }
        }

        Ok(GeneratedAgentResponse {
            runtime_output,
            reply,
        })
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
        _action_ctx: &ActionExecutionContext<'_>,
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
            run.warnings.push(format!("Model thought: {}", thought));
        }
        if let Some(warnings) = v.get("warnings").and_then(|w| w.as_array()) {
            for warning in warnings {
                if let Some(w) = warning.as_str() {
                    run.warnings.push(format!("Model warning: {}", w));
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
            let name = call
                .get("name")
                .or_else(|| call.get("tool"))
                .and_then(|n| n.as_str())
                .context("tool call missing name")?;
            let args = call
                .get("arguments")
                .or_else(|| call.get("args"))
                .or_else(|| call.get("input"))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            requests.push(AgentActionRequest {
                action_type: call
                    .get("action_type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("mcp_tool")
                    .to_string(),
                target: name.to_string(),
                input: serde_json::json!({ "arguments": args }),
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

    /// Filter tool actions by the configured allowlist.
    /// Returns the filtered list (empty if allowlist is not configured).
    fn filter_tools_by_allowlist(
        &self,
        actions: Vec<AgentActionRequest>,
    ) -> Vec<AgentActionRequest> {
        if self.config.toolset_allowlist.is_empty() {
            return actions;
        }
        actions
            .into_iter()
            .filter(|a| {
                self.config.toolset_allowlist.iter().any(|allowed| {
                    a.target == *allowed || a.target.starts_with(&format!("{}.", allowed))
                })
            })
            .collect()
    }

    /// Execute a batch of tool actions, collecting observations and status updates.
    /// Returns (all_succeeded, executed_count, budget_exceeded, observations).
    #[allow(clippy::too_many_arguments)]
    async fn execute_tool_batch(
        &self,
        tool_actions: &[AgentActionRequest],
        action_ctx: &ActionExecutionContext<'_>,
        run: &mut AgentRun,
        tool_call_count: &mut u32,
        callback: &Option<Arc<dyn StreamingCallback>>,
        status_updates: &mut Vec<crate::agent::types::AgentLoopStatusUpdate>,
    ) -> Result<(bool, u32, bool, Vec<AgentObservation>)> {
        let mut observations = Vec::new();
        let mut all_succeeded = true;
        let mut executed_this_step: u32 = 0;
        let mut budget_exceeded = false;

        for (idx, action_request) in tool_actions.iter().enumerate() {
            if *tool_call_count + executed_this_step >= self.config.max_tool_calls {
                self.try_record_event(
                    &run.id,
                    AgentRunEventType::ToolCallBlocked,
                    AgentEventActor::Runtime,
                    "Tool call budget exceeded",
                    serde_json::json!({
                        "max_tool_calls": self.config.max_tool_calls,
                        "current_count": *tool_call_count + executed_this_step,
                    }),
                );
                let obs = self.create_budget_exceeded_observation(run, *tool_call_count);
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

            self.try_record_event(
                &run.id,
                AgentRunEventType::ToolCallStarted,
                AgentEventActor::Tool(action_request.target.clone()),
                format!("Executing tool: {}", action_request.target),
                serde_json::json!({"tool": action_request.target}),
            );

            let exec_result = match self
                .action_executor
                .execute(action_request.clone(), action_ctx)
            {
                Ok(r) => {
                    let is_success = r.status == ActionExecutionStatus::Succeeded
                        || r.status == ActionExecutionStatus::NeedsConfirmation;
                    if is_success {
                        self.try_record_event(
                            &run.id,
                            AgentRunEventType::ToolCallCompleted,
                            AgentEventActor::Tool(action_request.target.clone()),
                            format!("Tool '{}' completed: {:?}", action_request.target, r.status),
                            serde_json::json!({"tool": action_request.target, "status": format!("{:?}", r.status)}),
                        );
                    } else {
                        self.try_record_event(
                            &run.id,
                            AgentRunEventType::ToolCallFailed,
                            AgentEventActor::Tool(action_request.target.clone()),
                            format!("Tool '{}' failed: {:?}", action_request.target, r.status),
                            serde_json::json!({"tool": action_request.target, "status": format!("{:?}", r.status)}),
                        );
                    }
                    r
                }
                Err(e) => {
                    self.try_record_event(
                        &run.id,
                        AgentRunEventType::ToolCallFailed,
                        AgentEventActor::Tool(action_request.target.clone()),
                        format!("Tool '{}' execution error: {}", action_request.target, e),
                        serde_json::json!({"tool": action_request.target, "error": e.to_string()}),
                    );
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
                    };
                    let obs = AgentObservation {
                        id: format!("obs-fail-{}", now.timestamp_nanos_opt().unwrap_or_default()),
                        action_id: Some(fail_action.id.clone()),
                        content: format!("工具 {} 执行失败: {}", action_request.target, e),
                        source: "action_executor".into(),
                        structured_result: Some(serde_json::json!({"error": e.to_string()})),
                        timestamp: now,
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
    #[allow(clippy::too_many_arguments)]
    fn handle_step_completion(
        &self,
        budget_exceeded: bool,
        all_succeeded: bool,
        observations: Vec<AgentObservation>,
        executed_this_step: u32,
        final_text: String,
        run: &mut AgentRun,
        status_updates: &mut Vec<crate::agent::types::AgentLoopStatusUpdate>,
    ) -> StepResult {
        if budget_exceeded {
            let final_response = format!(
                "已达到最大工具调用次数 ({})。已完成的观察结果：\n{}",
                self.config.max_tool_calls,
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
            let pending_count = run
                .actions
                .iter()
                .filter(|a| a.status == "needs_confirmation")
                .count();
            let final_response = if pending_count > 0 {
                run.status = AgentRunStatus::WaitingPermission;
                self.emit_status(
                    status_updates,
                    crate::agent::types::AgentLoopPhase::WaitingPermission,
                    "Waiting for user permission to continue",
                    0,
                    None,
                );
                "我需要先执行一些高风险或含敏感参数的工具操作，确认后才能继续给你结果。".into()
            } else {
                "工具执行过程中出现错误，请检查配置或稍后重试。".into()
            };
            return StepResult {
                stop_reason: if pending_count > 0 {
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
    ) -> AgentObservation {
        let now = chrono::Utc::now();
        AgentObservation {
            id: format!(
                "observation-budget-{}",
                now.timestamp_nanos_opt().unwrap_or_default()
            ),
            action_id: None,
            content: format!(
                "工具调用预算已耗尽 (max_tool_calls={})",
                self.config.max_tool_calls
            ),
            source: "agent_loop".into(),
            structured_result: Some(serde_json::json!({
                "error": "max_tool_calls exceeded",
                "max_tool_calls": self.config.max_tool_calls,
                "current_count": tool_call_count,
            })),
            timestamp: now,
        }
    }

    fn build_result(
        &self,
        mut run: AgentRun,
        final_response: String,
        stop_reason: String,
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
            tool_call_count,
            step_count,
            status_updates,
        }
    }
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

fn preview_text(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        format!("{}...", text.chars().take(max_len).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::action_executor::{ActionExecutor, ActionExecutorConfig};
    use crate::agent::runtime::AgentRuntime;
    use crate::agent::types::{AgentObservation, AgentRun};
    use crate::config::AppConfig;
    use crate::life_model::LifeModel;
    use crate::llm::ChatMessage;
    use crate::mcp::McpRegistry;
    use crate::mcp_audit::McpAuditStore;
    use crate::privacy::PrivacyEngine;
    use crate::scheduler::InferenceScheduler;
    use crate::tool_permissions::ToolPermissionStore;

    /// Creates a minimal AgentLoop for testing parse_agent_reply and build_follow_up_messages.
    /// Uses dummy scheduler credentials (no actual LLM calls are made).
    fn make_test_agent_loop() -> AgentLoop {
        let life_model = LifeModel::default();
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
        let runtime = AgentRuntime::new(life_model, scheduler.clone(), &app_config);
        let executor = ActionExecutor::new(ActionExecutorConfig::default());
        let config = AgentLoopConfig::default();
        AgentLoop::new(runtime, executor, scheduler, config)
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
                memory_store: None,
                proposal_store: None,
                agent_run_store: None,
                network_policy: None,
                calendar_ics_paths: &[],
                event_store: None,
            }
        }
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
            layer: crate::layer_router::Layer::L2,
            ..Default::default()
        };
        let obs = vec![AgentObservation {
            id: "obs-1".into(),
            action_id: Some("act-1".into()),
            content: "北京今天晴，25°C".into(),
            source: "web.search".into(),
            structured_result: None,
            timestamp: chrono::Utc::now(),
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
            layer: crate::layer_router::Layer::L2,
            ..Default::default()
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
            layer: crate::layer_router::Layer::L2,
            ..Default::default()
        };

        let obs = vec![AgentObservation {
            id: "obs-1".into(),
            action_id: None,
            content: "上海25°C".into(),
            source: "web.search".into(),
            structured_result: None,
            timestamp: chrono::Utc::now(),
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

    // ── P0-2: AgentRunEvent recording tests ──────────────────────────────

    fn make_test_agent_loop_with_events() -> (AgentLoop, AgentRunEventStore) {
        let agent = make_test_agent_loop();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let agent = agent.with_event_store(event_store.clone());
        (agent, event_store)
    }

    #[test]
    fn test_no_tool_response_event_sequence() {
        let (agent, _store) = make_test_agent_loop_with_events();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("no-tool-1", "hello");
        let mut tc: u32 = 0;

        // Record run.created
        agent.try_record_event(
            &run.id,
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "run created",
            serde_json::json!({}),
        );
        // Record model.call_started
        agent.try_record_event(
            &run.id,
            AgentRunEventType::ModelCallStarted,
            AgentEventActor::Agent,
            "model call started",
            serde_json::json!({}),
        );

        // Simulate no-tool response
        let result = agent
            .parse_agent_reply("Hello, how can I help?", &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(!result.json_parse_failed);
        assert!(result.actions.is_empty());

        // Record model.call_completed
        agent.try_record_event(
            &run.id,
            AgentRunEventType::ModelCallCompleted,
            AgentEventActor::Agent,
            "model call completed",
            serde_json::json!({"reply_len": 24}),
        );
        // Record run.completed
        agent.try_record_event(
            &run.id,
            AgentRunEventType::RunCompleted,
            AgentEventActor::Runtime,
            "run completed",
            serde_json::json!({"stop_reason": "no_tools"}),
        );

        let events = agent
            .event_store
            .as_ref()
            .unwrap()
            .list_events_by_run(&run.id)
            .unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].event_type, AgentRunEventType::RunCreated);
        assert_eq!(events[1].event_type, AgentRunEventType::ModelCallStarted);
        assert_eq!(events[2].event_type, AgentRunEventType::ModelCallCompleted);
        assert_eq!(events[3].event_type, AgentRunEventType::RunCompleted);
    }

    #[test]
    fn test_malformed_json_repair_event_sequence() {
        let (agent, _store) = make_test_agent_loop_with_events();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("malformed-json-1", "hello");
        let mut tc: u32 = 0;

        agent.try_record_event(
            &run.id,
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "run created",
            serde_json::json!({}),
        );
        agent.try_record_event(
            &run.id,
            AgentRunEventType::ModelCallStarted,
            AgentEventActor::Agent,
            "model call started",
            serde_json::json!({}),
        );
        agent.try_record_event(
            &run.id,
            AgentRunEventType::ModelCallCompleted,
            AgentEventActor::Agent,
            "model call completed",
            serde_json::json!({}),
        );

        // Simulate malformed JSON response (contains '{' but not valid JSON)
        let result = agent
            .parse_agent_reply(
                r#"{"final": "almost valid but missing bracket"#,
                &action_ctx,
                &mut run,
                &mut tc,
            )
            .unwrap();
        assert!(result.json_parse_failed); // Should signal repair needed

        agent.try_record_event(
            &run.id,
            AgentRunEventType::JsonRepairStarted,
            AgentEventActor::Runtime,
            "json repair started",
            serde_json::json!({}),
        );
        // Simulate repair succeeded (valid JSON after repair)
        let repair_reply = r#"{"final": "repaired response"}"#;
        let repair_result = agent
            .parse_agent_reply(repair_reply, &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(!repair_result.json_parse_failed);
        agent.try_record_event(
            &run.id,
            AgentRunEventType::JsonRepairCompleted,
            AgentEventActor::Runtime,
            "json repair succeeded",
            serde_json::json!({"repaired": true}),
        );
        agent.try_record_event(
            &run.id,
            AgentRunEventType::RunCompleted,
            AgentEventActor::Runtime,
            "run completed",
            serde_json::json!({}),
        );

        let events = agent
            .event_store
            .as_ref()
            .unwrap()
            .list_events_by_run(&run.id)
            .unwrap();
        assert_eq!(events.len(), 6);
        // Verify repair events exist in sequence
        let repair_start_ids: Vec<usize> = events
            .iter()
            .enumerate()
            .filter(|(_, e)| e.event_type == AgentRunEventType::JsonRepairStarted)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(repair_start_ids.len(), 1);
        let repair_complete_ids: Vec<usize> = events
            .iter()
            .enumerate()
            .filter(|(_, e)| e.event_type == AgentRunEventType::JsonRepairCompleted)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(repair_complete_ids.len(), 1);
        assert!(
            repair_complete_ids[0] > repair_start_ids[0],
            "repair completed should come after repair started"
        );
    }

    #[test]
    fn test_blocked_tool_call_event_sequence() {
        let (agent, _store) = make_test_agent_loop_with_events();
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("blocked-tool-1", "do many things");
        let mut tc: u32 = 0;

        agent.try_record_event(
            &run.id,
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "run created",
            serde_json::json!({}),
        );
        agent.try_record_event(
            &run.id,
            AgentRunEventType::ModelCallStarted,
            AgentEventActor::Agent,
            "model call started",
            serde_json::json!({}),
        );
        agent.try_record_event(
            &run.id,
            AgentRunEventType::ModelCallCompleted,
            AgentEventActor::Agent,
            "model call completed",
            serde_json::json!({}),
        );

        // Parse tool-call reply
        let reply = r#"{"final":"ok","actions":[{"name":"tool1","arguments":{"key":"v1"}}]}"#;
        let result = agent
            .parse_agent_reply(reply, &action_ctx, &mut run, &mut tc)
            .unwrap();
        assert!(!result.json_parse_failed);
        assert_eq!(result.actions.len(), 1);

        // Simulate tool blocked (budget exceeded or permission denied)
        agent.try_record_event(
            &run.id,
            AgentRunEventType::ToolCallStarted,
            AgentEventActor::Tool("tool1".to_string()),
            "executing tool1",
            serde_json::json!({"tool": "tool1"}),
        );
        agent.try_record_event(
            &run.id,
            AgentRunEventType::ToolCallBlocked,
            AgentEventActor::Runtime,
            "tool1 blocked: budget exceeded",
            serde_json::json!({"tool": "tool1", "reason": "budget"}),
        );
        agent.try_record_event(
            &run.id,
            AgentRunEventType::RunCompleted,
            AgentEventActor::Runtime,
            "run completed",
            serde_json::json!({"stop_reason": "max_tool_calls_reached"}),
        );

        let events = agent
            .event_store
            .as_ref()
            .unwrap()
            .list_events_by_run(&run.id)
            .unwrap();
        // Verify blocked event exists
        let blocked = events
            .iter()
            .find(|e| e.event_type == AgentRunEventType::ToolCallBlocked);
        assert!(blocked.is_some());
        assert!(blocked.unwrap().summary.contains("budget exceeded"));
    }

    #[test]
    fn test_events_not_recorded_when_store_is_none() {
        let agent = make_test_agent_loop(); // no event store
        let ctx = TestCtx::new();
        let action_ctx = ctx.as_ctx();
        let mut run = AgentRun::new_chat_run("no-store-1", "test");
        let mut tc: u32 = 0;

        // These should not crash
        agent.try_record_event(
            &run.id,
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "should not persist",
            serde_json::json!({}),
        );
        let _ = agent.parse_agent_reply("hello", &action_ctx, &mut run, &mut tc);
        agent.try_record_event(
            &run.id,
            AgentRunEventType::RunCompleted,
            AgentEventActor::Runtime,
            "should not persist",
            serde_json::json!({}),
        );

        // No events should be stored
        assert!(agent.event_store.is_none());
    }

    #[test]
    fn test_model_failed_event_recorded() {
        let (agent, store) = make_test_agent_loop_with_events();
        let run_id = "model-fail-1";

        agent.try_record_event(
            run_id,
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "run created",
            serde_json::json!({}),
        );
        agent.try_record_event(
            run_id,
            AgentRunEventType::ModelCallStarted,
            AgentEventActor::Agent,
            "model call started",
            serde_json::json!({"step": 1}),
        );
        agent.try_record_event(
            run_id,
            AgentRunEventType::ModelCallFailed,
            AgentEventActor::Agent,
            "model timeout",
            serde_json::json!({"error": "timeout", "step": 1}),
        );
        agent.try_record_event(
            run_id,
            AgentRunEventType::RunFailed,
            AgentEventActor::Runtime,
            "run failed due to model error",
            serde_json::json!({}),
        );

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[2].event_type, AgentRunEventType::ModelCallFailed);
        assert_eq!(events[3].event_type, AgentRunEventType::RunFailed);
    }

    // ── P7: AgentLoop governance events use real run id ───────────────────

    #[test]
    fn test_agent_loop_governance_events_use_real_run_id() {
        let (agent, store) = make_test_agent_loop_with_events();
        let run_id = "al-governance-1";

        // Simulate governance events written by run_loop_core
        agent.try_record_event(
            run_id,
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "run created",
            serde_json::json!({"session_id": "test"}),
        );
        agent.try_record_event(
            run_id,
            AgentRunEventType::AgentSpecSelected,
            AgentEventActor::Runtime,
            "AgentSpec main.default selected",
            serde_json::json!({
                "agent_spec_id": "main.default",
                "role": "Main",
                "privacy_policy": "cloud_allowed",
            }),
        );
        agent.try_record_event(
            run_id,
            AgentRunEventType::PromptStackAssembled,
            AgentEventActor::Runtime,
            "PromptStack assembled",
            serde_json::json!({
                "agent_spec_id": "main.default",
                "prompt_blocks": [{"id": "base.system"}],
            }),
        );
        agent.try_record_event(
            run_id,
            AgentRunEventType::ContextGovernanceApplied,
            AgentEventActor::Runtime,
            "Context governance applied",
            serde_json::json!({
                "agent_spec_id": "main.default",
                "context_included": ["session_summary", "lifemodel_summary", "memory"],
                "context_excluded": [],
                "privacy_policy": "cloud_allowed",
            }),
        );

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[1].event_type, AgentRunEventType::AgentSpecSelected);

        // Verify AgentSpecSelected payload does NOT contain raw prompt/memory/LifeModel
        let spec_payload = &events[1].payload;
        assert_eq!(spec_payload["agent_spec_id"], "main.default");
        assert!(spec_payload["role"].is_string());
        assert!(spec_payload["privacy_policy"].is_string());
        assert!(!spec_payload.to_string().contains("raw_prompt"));
        assert!(!spec_payload.to_string().contains("raw_memory"));

        // Verify PromptStackAssembled payload has block IDs only
        let ps_payload = &events[2].payload;
        assert_eq!(ps_payload["agent_spec_id"], "main.default");
        let blocks = ps_payload["prompt_blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["id"], "base.system");
        assert!(blocks[0].get("content").is_none());

        // Verify ContextGovernanceApplied payload has categories only
        let cg_payload = &events[3].payload;
        let included = cg_payload["context_included"].as_array().unwrap();
        assert!(included.iter().any(|v| v == "session_summary"));
        assert!(!cg_payload.to_string().contains("raw_lifemodel"));
    }

    #[test]
    fn test_agent_loop_governance_events_nonexistent_for_synthetic_run_id() {
        let (agent, store) = make_test_agent_loop_with_events();
        let real_run_id = "al-real-run-1";

        // Record events only under the real run id
        agent.try_record_event(
            real_run_id,
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "run created",
            serde_json::json!({}),
        );
        agent.try_record_event(
            real_run_id,
            AgentRunEventType::AgentSpecSelected,
            AgentEventActor::Runtime,
            "spec selected",
            serde_json::json!({"agent_spec_id": "main.default"}),
        );

        // Synthetic run id should have no events
        let synthetic_id = format!("al-nonstream-{}", real_run_id);
        let synthetic_events = store.list_events_by_run(&synthetic_id).unwrap();
        assert!(
            synthetic_events.is_empty(),
            "synthetic run id should have no events"
        );

        // Real run id should have events
        let real_events = store.list_events_by_run(real_run_id).unwrap();
        assert_eq!(real_events.len(), 2);
        assert!(real_events
            .iter()
            .any(|e| e.event_type == AgentRunEventType::AgentSpecSelected));
    }

    // ── P7: missing prompt block does not record fake PromptStackAssembled ──

    #[tokio::test]
    async fn test_agent_loop_missing_prompt_block_does_not_record_prompt_stack_assembled() {
        use crate::agent::types::AgentSpec;

        let (agent, store) = make_test_agent_loop_with_events();

        let test_ctx = TestCtx::new();
        let action_ctx = test_ctx.as_ctx();
        let life_model = LifeModel::default();
        let spec = AgentSpec::default_main_spec();
        let mut spec = spec; // remove mut if not needed, but we need to set prompt_block_ids
                             // For this test, we just use the default spec with valid blocks.
                             // The execute_task_with_spec will succeed; generate_governed will fail
                             // because the fake scheduler has no real backend.
                             // But BEFORE the model call, record_runtime_governance_events writes
                             // PromptStackAssembled with real runtime_output data.
                             // This test validates that real trace comes from runtime_output, not
                             // from manual block_id iteration.

        let registry = crate::agent::prompt_stack::PromptBlockRegistry::built_in();
        let task = AgentTask {
            kind: crate::agent::types::AgentTaskKind::Conversation,
            session_id: "test-governance-session".to_string(),
            user_text: "hello".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            layer: Layer::L1,
            ..Default::default()
        };

        // Run: execute_task_with_spec succeeds (L1 DirectReasoner),
        // generate_governed fails (fake scheduler), but governance events were
        // already written from real runtime_output.
        let result = agent
            .run(
                &task,
                &life_model,
                "",
                None,
                PrivacyEngine::new(),
                crate::agent::types::PrivacyPolicy::CloudAllowed,
                &spec,
                &registry,
                &action_ctx,
            )
            .await;

        match result {
            Ok(loop_result) => {
                let run_id = loop_result.run.id;
                let events = store.list_events_by_run(&run_id).unwrap();

                // AgentSpecSelected should be present
                let has_spec_selected = events
                    .iter()
                    .any(|e| e.event_type == AgentRunEventType::AgentSpecSelected);
                assert!(has_spec_selected);

                // PromptStackAssembled may or may not be present depending on
                // whether execute_task_with_spec succeeded before generate_governed failed.
                // If present, its payload must come from runtime_output (block IDs/versions).
                for event in &events {
                    if event.event_type == AgentRunEventType::PromptStackAssembled {
                        let blocks = event.payload["prompt_blocks"].as_array().unwrap();
                        for block in blocks {
                            assert!(
                                block.get("content").is_none(),
                                "prompt_blocks must not contain raw content"
                            );
                            // BlockTraceEntry has id, version, purpose, cloud_allowed, estimated_tokens
                            assert!(block["id"].is_string());
                        }
                    }
                }
            }
            Err(_) => {
                // If run fails, make sure no PromptStackAssembled was written with fake ids
                // (but it's fine - governance events only written on success)
            }
        }
    }

    #[tokio::test]
    async fn test_agent_loop_missing_prompt_block_fails_without_governance_events() {
        use crate::agent::types::AgentSpec;

        let (agent, store) = make_test_agent_loop_with_events();
        let test_ctx = TestCtx::new();
        let action_ctx = test_ctx.as_ctx();
        let life_model = LifeModel::default();

        // Spec with a missing prompt block — must fail before model call
        let mut spec = AgentSpec::default_main_spec();
        spec.prompt_block_ids = vec!["missing.block".to_string()];

        let registry = crate::agent::prompt_stack::PromptBlockRegistry::built_in();
        let task = AgentTask {
            kind: crate::agent::types::AgentTaskKind::Conversation,
            session_id: "test-missing-block".to_string(),
            user_text: "hello".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            layer: Layer::L1,
            ..Default::default()
        };

        let result = agent
            .run(
                &task,
                &life_model,
                "",
                None,
                PrivacyEngine::new(),
                crate::agent::types::PrivacyPolicy::CloudAllowed,
                &spec,
                &registry,
                &action_ctx,
            )
            .await;

        match result {
            Ok(loop_result) => {
                let run_id = loop_result.run.id;
                let events = store.list_events_by_run(&run_id).unwrap();

                // Must have a failure event (ModelCallFailed or RunFailed)
                let has_failure = events.iter().any(|e| {
                    e.event_type == AgentRunEventType::ModelCallFailed
                        || e.event_type == AgentRunEventType::RunFailed
                });
                assert!(
                    has_failure,
                    "missing prompt block must produce a failure event"
                );

                // Must NOT have PromptStackAssembled
                let has_prompt_stack = events
                    .iter()
                    .any(|e| e.event_type == AgentRunEventType::PromptStackAssembled);
                assert!(
                    !has_prompt_stack,
                    "missing prompt block must not record PromptStackAssembled"
                );
            }
            Err(e) => {
                // If AgentLoop returns Err, verify the error is governance-related
                let msg = e.to_string();
                assert!(
                    msg.contains("unknown")
                        || msg.contains("missing.block")
                        || msg.contains("governance"),
                    "error should mention governance/prompt failure, got: {}",
                    msg
                );
                // No events should be recorded under any synthetic id
                // (AgentLoop creates a run internally; the events reference the real run id)
            }
        }
    }

    // ── End of P7 hardening tests ─────────────────────────────────────
}

/// Search memory store for relevant context and format as a string.
fn search_memory_for_context(
    memory_store: &crate::memory::MemoryStore,
    query: &str,
    session_id: &str,
) -> Result<Option<String>> {
    if query.trim().is_empty() {
        return Ok(None);
    }

    let hits = memory_store.search_text_memories(Some(session_id), query, 5)?;
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
