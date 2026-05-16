use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;

use super::config::AgentLoopConfig;
use super::context::{wrap_user_content, AgentLoopContext};
use super::streaming::StreamingCallback;
use super::types::{preview_text, AgentLoopResult, StepContext};
use crate::agent::event_store::AgentRunEventStore;
use crate::agent::runtime::{AgentRuntime, AgentRuntimeOutput};
use crate::agent::types::{
    AgentEventActor, AgentLoopPhase, AgentLoopStatusUpdate, AgentRun, AgentRunError, AgentRunEvent,
    AgentRunEventType, AgentRunStatus, AgentTask, PrivacyPolicy,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::privacy::PrivacyEngine;
use crate::scheduler::InferenceScheduler;

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
    pub(crate) runtime: AgentRuntime,
    pub(crate) action_executor: crate::agent::action_executor::ActionExecutor,
    pub(crate) scheduler: InferenceScheduler,
    pub(crate) config: AgentLoopConfig,
    /// Optional vNext event store for runtime trace recording.
    /// When None, no events are recorded (backward compatible).
    pub(crate) event_store: Option<AgentRunEventStore>,
}

impl AgentLoop {
    pub fn new(
        runtime: AgentRuntime,
        action_executor: crate::agent::action_executor::ActionExecutor,
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
    pub(crate) fn try_record_event(
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
    pub(crate) fn record_runtime_governance_events(
        &self,
        run_id: &str,
        agent_spec: &crate::agent::types::AgentSpec,
        runtime_output: &AgentRuntimeOutput,
        effective_privacy_policy: PrivacyPolicy,
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
                "agent_spec_privacy_policy": agent_spec.privacy_policy.to_string(),
                "effective_privacy_policy": effective_privacy_policy.to_string(),
            }),
        );
    }

    pub(crate) fn emit_status(
        &self,
        updates: &mut Vec<AgentLoopStatusUpdate>,
        phase: AgentLoopPhase,
        message: impl Into<String>,
        step_index: u32,
        tool_call_index: Option<u32>,
    ) {
        updates.push(AgentLoopStatusUpdate {
            phase,
            message: message.into(),
            step_index,
            tool_call_index,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Core agent loop implementation shared by `run` and `run_streaming`.
    /// Differences are handled via the optional `callback`.
    pub(crate) async fn run_loop_core(
        &self,
        actx: &AgentLoopContext<'_>,
        action_ctx: &crate::agent::action_executor::ActionContext,
        callback: Option<Arc<dyn StreamingCallback>>,
    ) -> Result<AgentLoopResult> {
        use crate::agent::types::AgentRunEventType;

        let start_time = Instant::now();
        let mut run = AgentRun::new_chat_run(&actx.task.session_id, &actx.task.user_text);
        run.user_input = Some(actx.task.user_text.clone());
        run.agent_spec_id = Some(actx.agent_spec.id.clone());

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

        let mut status_updates: Vec<AgentLoopStatusUpdate> = Vec::new();

        // Set reasoning strategy
        run.reasoning_strategy = Some(if actx.task.layer == Layer::L3 {
            "layered".into()
        } else {
            "direct".into()
        });

        #[allow(unused_assignments)]
        let mut stop_reason = String::new();

        // P8: re-compaction guard — only re-trigger when significant new
        // messages have accumulated since the last compaction.
        let mut last_compacted_message_count: usize = 0;
        let min_new_messages_for_recompact: usize = 5;

        loop {
            // Emit thinking status at start of each step
            self.emit_status(
                &mut status_updates,
                AgentLoopPhase::Thinking,
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
            let memory_context = if let Some(ref memory_store) = action_ctx.memory_store {
                let guard = memory_store.lock().await;
                super::memory::search_memory_for_context(
                    &guard,
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

            // P8: Compaction check before each model call.
            // Only re-trigger if enough new messages have accumulated since
            // the last compaction, to avoid infinite re-compaction.
            if (last_compacted_message_count == 0
                || current_task.messages.len()
                    >= last_compacted_message_count + min_new_messages_for_recompact)
                && self.try_compact_context(&mut current_task, &mut run, actx.privacy_policy)
            {
                last_compacted_message_count = current_task.messages.len();
            }

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
                AgentLoopPhase::Failed,
                format!("Execution failed: {}", stop_reason),
                step_count,
                None,
            );
        } else {
            self.emit_status(
                &mut status_updates,
                AgentLoopPhase::Completed,
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
        _privacy_policy: PrivacyPolicy,
        agent_spec: &crate::agent::types::AgentSpec,
        prompt_registry: &crate::agent::prompt_stack::PromptBlockRegistry,
        action_ctx: &crate::agent::action_executor::ActionContext,
    ) -> Result<AgentLoopResult> {
        let effective_policy = crate::agent::runtime::resolve_privacy_policy(task, agent_spec);
        let effective_policy = if !self.config.allow_cloud {
            PrivacyPolicy::LocalOnly
        } else {
            effective_policy
        };
        let actx = AgentLoopContext {
            task,
            life_model,
            tools_prompt,
            memory_context,
            privacy_engine,
            privacy_policy: effective_policy,
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
        _privacy_policy: PrivacyPolicy,
        agent_spec: &crate::agent::types::AgentSpec,
        prompt_registry: &crate::agent::prompt_stack::PromptBlockRegistry,
        action_ctx: &crate::agent::action_executor::ActionContext,
        callback: Arc<dyn StreamingCallback>,
    ) -> Result<AgentLoopResult> {
        let effective_policy = crate::agent::runtime::resolve_privacy_policy(task, agent_spec);
        let effective_policy = if !self.config.allow_cloud {
            PrivacyPolicy::LocalOnly
        } else {
            effective_policy
        };
        let actx = AgentLoopContext {
            task,
            life_model,
            tools_prompt,
            memory_context,
            privacy_engine,
            privacy_policy: effective_policy,
            agent_spec,
            prompt_registry,
        };
        self.run_loop_core(&actx, action_ctx, Some(callback)).await
    }
}
