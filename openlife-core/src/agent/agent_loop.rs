use crate::agent::action_executor::{
    ActionExecutionContext, ActionExecutionStatus, ActionExecutor, AgentActionRequest,
};
use crate::agent::runtime::{AgentRuntime, AgentRuntimeOutput};
use crate::agent::types::{AgentObservation, AgentRun, AgentRunError, AgentRunStatus, AgentTask};
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
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_steps: 4,
            max_tool_calls: 6,
            timeout_seconds: 90,
            allow_writes: true,
            allow_cloud: true,
        }
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
    pub action_ctx: &'a ActionExecutionContext<'a>,
    pub run: &'a mut AgentRun,
    pub tool_call_count: u32,
}

/// The AgentLoop executes a task with a fixed 2-step pattern:
///
/// 1. Model response (with optional tool calls)
/// 2. Follow-up response (after tool observations)
///
/// This is the Beta MVP. Future versions will support iterative multi-step loops.
pub struct AgentLoop {
    runtime: AgentRuntime,
    action_executor: ActionExecutor,
    scheduler: InferenceScheduler,
    config: AgentLoopConfig,
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
        }
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

    /// Run the iterative agent loop for a given task.
    /// Supports multi-step ReAct: generate -> parse tools -> execute -> observe -> repeat.
    pub async fn run(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        privacy_engine: PrivacyEngine,
        action_ctx: &ActionExecutionContext<'_>,
    ) -> Result<AgentLoopResult> {
        let start_time = Instant::now();
        let mut run = AgentRun::new_chat_run(&task.session_id, &task.user_text);
        run.user_input = Some(task.user_text.clone());

        let mut step_count: u32 = 0;
        let mut tool_call_count: u32 = 0;
        let mut final_response = String::new();
        let mut current_task = task.clone();
        let current_tools_prompt = tools_prompt.to_string();
        let current_memory_context = memory_context;
        let current_privacy_engine = privacy_engine;
        let mut status_updates: Vec<crate::agent::types::AgentLoopStatusUpdate> = Vec::new();

        // Set reasoning strategy
        run.reasoning_strategy = Some(if task.layer == Layer::L3 {
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
                search_memory_for_context(memory_store, &current_task.user_text, &task.session_id)
                    .unwrap_or_else(|e| {
                        eprintln!("[AgentLoop] Memory search failed: {}", e);
                        current_memory_context.clone()
                    })
            } else {
                current_memory_context.clone()
            };

            // Execute single step
            let step_result = self
                .run_single_step(StepContext {
                    task: &current_task,
                    life_model,
                    tools_prompt: &current_tools_prompt,
                    memory_context,
                    privacy_engine: current_privacy_engine.clone(),
                    action_ctx,
                    run: &mut run,
                    tool_call_count,
                })
                .await?;

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

        if run.status != AgentRunStatus::Failed {
            run.status = AgentRunStatus::Completed;
        }
        run.output_preview = Some(preview_text(&final_response, 200));
        run.finished_at = Some(chrono::Utc::now());

        Ok(self.build_result(
            run,
            final_response,
            stop_reason,
            tool_call_count,
            step_count,
            status_updates,
        ))
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
        action_ctx: &ActionExecutionContext<'_>,
        callback: Arc<dyn StreamingCallback>,
    ) -> Result<AgentLoopResult> {
        let start_time = Instant::now();
        let mut run = AgentRun::new_chat_run(&task.session_id, &task.user_text);
        run.user_input = Some(task.user_text.clone());

        let mut step_count: u32 = 0;
        let mut tool_call_count: u32 = 0;
        let mut final_response = String::new();
        let mut current_task = task.clone();
        let current_tools_prompt = tools_prompt.to_string();
        let current_memory_context = memory_context;
        let current_privacy_engine = privacy_engine;
        let mut status_updates: Vec<crate::agent::types::AgentLoopStatusUpdate> = Vec::new();

        run.reasoning_strategy = Some(if task.layer == Layer::L3 {
            "layered".into()
        } else {
            "direct".into()
        });

        #[allow(unused_assignments)]
        let mut stop_reason = String::new();

        loop {
            callback
                .on_status(
                    "thinking",
                    &format!("Step {}: analyzing task", step_count + 1),
                    step_count,
                )
                .await;

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

            let memory_context = if let Some(memory_store) = action_ctx.memory_store {
                search_memory_for_context(memory_store, &current_task.user_text, &task.session_id)
                    .unwrap_or_else(|e| {
                        eprintln!("[AgentLoop] Memory search failed: {}", e);
                        current_memory_context.clone()
                    })
            } else {
                current_memory_context.clone()
            };

            // streaming step: generate with real token streaming
            let step_result = self
                .run_single_step_streaming(
                    StepContext {
                        task: &current_task,
                        life_model,
                        tools_prompt: &current_tools_prompt,
                        memory_context,
                        privacy_engine: current_privacy_engine.clone(),
                        action_ctx,
                        run: &mut run,
                        tool_call_count,
                    },
                    callback.clone(),
                )
                .await?;

            step_count += 1;
            tool_call_count += step_result.tool_call_count_delta;
            final_response = step_result.final_response;
            stop_reason = step_result.stop_reason;
            status_updates.extend(step_result.status_updates);

            if !step_result.should_continue {
                break;
            }

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
        }

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

        callback
            .on_status("completed", &format!("Done: {}", stop_reason), step_count)
            .await;

        if run.status != AgentRunStatus::Failed {
            run.status = AgentRunStatus::Completed;
        }
        run.output_preview = Some(preview_text(&final_response, 200));
        run.finished_at = Some(chrono::Utc::now());

        Ok(self.build_result(
            run,
            final_response,
            stop_reason,
            tool_call_count,
            step_count,
            status_updates,
        ))
    }

    /// Execute a single step of the agent loop with streaming output.
    async fn run_single_step(&self, mut ctx: StepContext<'_>) -> Result<StepResult> {
        let mut status_updates: Vec<crate::agent::types::AgentLoopStatusUpdate> = Vec::new();

        // Generate model response
        let generated = self
            .generate_response(
                ctx.task,
                ctx.life_model,
                ctx.tools_prompt,
                ctx.memory_context,
                ctx.privacy_engine,
            )
            .await;

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
                let parsed = self.parse_agent_reply(
                    &reply,
                    ctx.action_ctx,
                    ctx.run,
                    &mut ctx.tool_call_count,
                )?;
                let final_text = parsed.final_text;
                let tool_actions = parsed.actions;

                if tool_actions.is_empty() {
                    // No tool calls - this is the final answer
                    self.emit_status(
                        &mut status_updates,
                        crate::agent::types::AgentLoopPhase::GeneratingFinal,
                        "No tools needed, generating final answer",
                        0,
                        None,
                    );
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

                // Execute tools and build observations
                let mut observations = Vec::new();
                let mut all_succeeded = true;
                let mut executed_this_step = 0;
                let mut budget_exceeded = false;

                for (idx, action_request) in tool_actions.iter().enumerate() {
                    if ctx.tool_call_count >= self.config.max_tool_calls {
                        let obs =
                            self.create_budget_exceeded_observation(ctx.run, ctx.tool_call_count);
                        observations.push(obs.clone());
                        ctx.run.observations.push(obs);
                        all_succeeded = false;
                        budget_exceeded = true;
                        break;
                    }

                    self.emit_status(
                        &mut status_updates,
                        crate::agent::types::AgentLoopPhase::ExecutingTool,
                        format!("Executing tool: {}", action_request.target),
                        0,
                        Some(idx as u32),
                    );

                    let exec_result = self
                        .action_executor
                        .execute(action_request.clone(), ctx.action_ctx)?;

                    // Collect proposal_id from action output if present
                    if let Some(ref output) = exec_result.action.output {
                        let proposal_id = output
                            .get("proposal_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| {
                                // Action output may be wrapped as { "text": "...json string..." }
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
                            ctx.run.add_generated_proposal(&id);
                        }
                    }

                    ctx.run.actions.push(exec_result.action.clone());
                    observations.push(exec_result.observation.clone());
                    ctx.run.observations.push(exec_result.observation.clone());

                    self.emit_status(
                        &mut status_updates,
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

                    if exec_result.status != ActionExecutionStatus::Succeeded {
                        all_succeeded = false;
                    }

                    ctx.tool_call_count += 1;
                    executed_this_step += 1;
                }

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
                    return Ok(StepResult {
                        stop_reason: "max_tool_calls_reached".into(),
                        final_response,
                        should_continue: false,
                        tool_call_count_delta: executed_this_step,
                        observations,
                        status_updates,
                    });
                }

                if !all_succeeded {
                    // Some tools failed or need confirmation
                    let pending_count = ctx
                        .run
                        .actions
                        .iter()
                        .filter(|a| a.status == "needs_confirmation")
                        .count();
                    let final_response = if pending_count > 0 {
                        ctx.run.status = AgentRunStatus::WaitingPermission;
                        self.emit_status(
                            &mut status_updates,
                            crate::agent::types::AgentLoopPhase::WaitingPermission,
                            "Waiting for user permission to continue",
                            0,
                            None,
                        );
                        "我需要先执行一些高风险或含敏感参数的工具操作，确认后才能继续给你结果。"
                            .into()
                    } else {
                        "工具执行过程中出现错误，请检查配置或稍后重试。".into()
                    };
                    return Ok(StepResult {
                        stop_reason: if pending_count > 0 {
                            "needs_confirmation".into()
                        } else {
                            "tool_execution_failed".into()
                        },
                        final_response,
                        should_continue: false,
                        tool_call_count_delta: executed_this_step,
                        observations,
                        status_updates,
                    });
                }

                if observations.is_empty() {
                    return Ok(StepResult {
                        stop_reason: "no_observations".into(),
                        final_response: final_text,
                        should_continue: false,
                        tool_call_count_delta: 0,
                        observations: vec![],
                        status_updates,
                    });
                }

                // Continue to next iteration
                Ok(StepResult {
                    stop_reason: String::new(), // Will be set by caller if this is the last step
                    final_response: final_text,
                    should_continue: true,
                    tool_call_count_delta: executed_this_step,
                    observations,
                    status_updates,
                })
            }
            Err(e) => {
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

    /// Streaming variant of run_single_step. Forwards token chunks through the callback.
    async fn run_single_step_streaming(
        &self,
        mut ctx: StepContext<'_>,
        callback: Arc<dyn StreamingCallback>,
    ) -> Result<StepResult> {
        let mut status_updates: Vec<crate::agent::types::AgentLoopStatusUpdate> = Vec::new();

        // Generate model response with streaming
        let generated = self
            .generate_response_streaming(
                ctx.task,
                ctx.life_model,
                ctx.tools_prompt,
                ctx.memory_context,
                ctx.privacy_engine,
                callback.clone(),
            )
            .await;

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
                let parsed = self.parse_agent_reply(
                    &reply,
                    ctx.action_ctx,
                    ctx.run,
                    &mut ctx.tool_call_count,
                )?;

                let final_text = parsed.final_text;
                if parsed.actions.is_empty() {
                    return Ok(StepResult {
                        stop_reason: "done".into(),
                        final_response: final_text,
                        should_continue: false,
                        tool_call_count_delta: 0,
                        observations: vec![],
                        status_updates,
                    });
                }

                // Execute actions
                let mut observations = Vec::new();
                let mut all_succeeded = true;
                let mut budget_exceeded = false;
                let mut executed_this_step: u32 = 0;

                for (idx, action_request) in parsed.actions.iter().enumerate() {
                    if ctx.tool_call_count + executed_this_step >= self.config.max_tool_calls {
                        let obs =
                            self.create_budget_exceeded_observation(ctx.run, ctx.tool_call_count);
                        ctx.run.observations.push(obs.clone());
                        observations.push(obs);
                        budget_exceeded = true;
                        break;
                    }

                    callback.on_tool_start(&action_request.target, 0).await;

                    let exec_result = self
                        .action_executor
                        .execute(action_request.clone(), ctx.action_ctx)?;

                    // Collect proposal_id from action output
                    if let Some(ref output) = exec_result.action.output {
                        let proposal_id = output
                            .get("proposal_id")
                            .and_then(|v: &Value| v.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| {
                                output
                                    .get("text")
                                    .and_then(|v: &Value| v.as_str())
                                    .and_then(|text| {
                                        serde_json::from_str::<serde_json::Value>(text)
                                            .ok()
                                            .and_then(|json| {
                                                json.get("proposal_id")
                                                    .and_then(|v: &Value| v.as_str())
                                                    .map(|s| s.to_string())
                                            })
                                    })
                            });
                        if let Some(id) = proposal_id {
                            callback.on_proposal("external_write_action", &id).await;
                            ctx.run.add_generated_proposal(&id);
                        }
                    }

                    callback
                        .on_tool_result(
                            &action_request.target,
                            exec_result.status == ActionExecutionStatus::Succeeded,
                            0,
                        )
                        .await;

                    ctx.run.actions.push(exec_result.action.clone());
                    observations.push(exec_result.observation.clone());
                    ctx.run.observations.push(exec_result.observation.clone());

                    self.emit_status(
                        &mut status_updates,
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

                    if exec_result.status != ActionExecutionStatus::Succeeded {
                        all_succeeded = false;
                    }

                    ctx.tool_call_count += 1;
                    executed_this_step += 1;
                }

                // (Same budget/confirmation/no_observations logic as run_single_step)
                if budget_exceeded {
                    let final_response =
                        format!("已达到最大工具调用次数 ({})。", self.config.max_tool_calls);
                    return Ok(StepResult {
                        stop_reason: "max_tool_calls_reached".into(),
                        final_response,
                        should_continue: false,
                        tool_call_count_delta: executed_this_step,
                        observations,
                        status_updates,
                    });
                }

                if !all_succeeded {
                    let pending_count = ctx
                        .run
                        .actions
                        .iter()
                        .filter(|a| a.status == "needs_confirmation")
                        .count();
                    let final_response = if pending_count > 0 {
                        ctx.run.status = AgentRunStatus::WaitingPermission;
                        "需要用户确认部分操作才能继续。".into()
                    } else {
                        "部分工具执行出错。".into()
                    };
                    return Ok(StepResult {
                        stop_reason: if pending_count > 0 {
                            "needs_confirmation".into()
                        } else {
                            "tool_execution_failed".into()
                        },
                        final_response,
                        should_continue: false,
                        tool_call_count_delta: executed_this_step,
                        observations,
                        status_updates,
                    });
                }

                if observations.is_empty() {
                    return Ok(StepResult {
                        stop_reason: "no_observations".into(),
                        final_response: final_text,
                        should_continue: false,
                        tool_call_count_delta: 0,
                        observations: vec![],
                        status_updates,
                    });
                }

                Ok(StepResult {
                    stop_reason: String::new(),
                    final_response: final_text,
                    should_continue: true,
                    tool_call_count_delta: executed_this_step,
                    observations,
                    status_updates,
                })
            }
            Err(e) => {
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
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        privacy_engine: PrivacyEngine,
    ) -> Result<GeneratedAgentResponse> {
        let memory_hits = Vec::new(); // Simplified for Beta MVP
        let runtime_output = self
            .runtime
            .execute_task(
                task,
                life_model,
                tools_prompt,
                memory_context,
                memory_hits,
                privacy_engine,
            )
            .await
            .map_err(|e| anyhow::anyhow!("runtime execution failed: {}", e))?;

        let tools_prompt = if tools_prompt.trim().is_empty() {
            None
        } else {
            Some(tools_prompt)
        };
        let reply = self
            .scheduler
            .generate(
                runtime_output.final_messages.clone(),
                life_model,
                tools_prompt,
            )
            .await
            .map_err(|e| anyhow::anyhow!("model generation failed: {}", e))?;

        Ok(GeneratedAgentResponse {
            runtime_output,
            reply,
        })
    }

    /// Streaming variant of generate_response. Uses scheduler.generate_stream()
    /// and forwards each chunk through the callback.
    async fn generate_response_streaming(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        privacy_engine: PrivacyEngine,
        callback: Arc<dyn StreamingCallback>,
    ) -> Result<GeneratedAgentResponse> {
        let memory_hits = Vec::new();
        let runtime_output = self
            .runtime
            .execute_task(
                task,
                life_model,
                tools_prompt,
                memory_context,
                memory_hits,
                privacy_engine,
            )
            .await
            .map_err(|e| anyhow::anyhow!("runtime execution failed: {}", e))?;

        let tools_prompt = if tools_prompt.trim().is_empty() {
            None
        } else {
            Some(tools_prompt)
        };

        // Use streaming generation
        let mut stream = self
            .scheduler
            .generate_stream(
                runtime_output.final_messages.clone(),
                life_model,
                tools_prompt,
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
                    // Fallback: use partial reply, don't fail entirely
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
            });
        };

        let v: Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => {
                // Fail-soft: malformed JSON, record warning and treat as final
                run.warnings.push(format!(
                    "Parse warning: invalid JSON in model response: {}",
                    e
                ));
                return Ok(ParsedAgentReply {
                    final_text: reply.to_string(),
                    actions: Vec::new(),
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
                });
            }
            actions
        } else if let Some(tool_calls) = v.get("tool_calls").and_then(|c| c.as_array()) {
            if tool_calls.is_empty() {
                return Ok(ParsedAgentReply {
                    final_text,
                    actions: Vec::new(),
                });
            }
            tool_calls
        } else {
            // No actions or tool_calls - treat as final answer
            return Ok(ParsedAgentReply {
                final_text,
                actions: Vec::new(),
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

        // Remind the model of the original task
        follow_up.push_str(&format!(
            "[系统] 继续完成用户的原始请求：\"{}\"\n\n",
            task.user_text
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

pub(crate) struct ParsedAgentReply {
    pub(crate) final_text: String,
    pub(crate) actions: Vec<AgentActionRequest>,
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
    use super::preview_text;

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
    if let Some(start) = text.find('{') {
        let mut depth = 0;
        let mut in_string = false;
        let mut escape = false;
        for (idx, b) in text[start..].bytes().enumerate() {
            if escape {
                escape = false;
                continue;
            }
            if in_string {
                if b == b'\\' {
                    escape = true;
                    continue;
                }
                if b == b'"' {
                    in_string = false;
                }
                continue;
            }
            if b == b'"' {
                in_string = true;
                continue;
            }
            if b == b'{' {
                depth += 1;
            } else if b == b'}' {
                if depth == 0 {
                    continue;
                }
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..=start + idx]);
                }
            }
        }
    }
    None
}
