use anyhow::Result;
use futures::StreamExt;
use std::sync::Arc;

use super::context::AgentLoopContext;
use super::streaming::StreamingCallback;
use super::types::{GeneratedAgentResponse, StepContext, StepResult};
use super::AgentLoop;
use crate::agent::types::{
    AgentEventActor, AgentLoopPhase, AgentLoopStatusUpdate, AgentRunError, AgentRunEventType,
    AgentRunStatus,
};

impl AgentLoop {
    pub(crate) async fn generate_response(
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

        self.record_runtime_governance_events(
            run_id,
            actx.agent_spec,
            &runtime_output,
            actx.privacy_policy,
        );

        let tools_prompt = if actx.tools_prompt.trim().is_empty() {
            None
        } else {
            Some(actx.tools_prompt)
        };

        let route = self.scheduler.preview_chat_route(tools_prompt).await;
        self.try_record_event(
            run_id,
            AgentRunEventType::ModelRouteSelected,
            AgentEventActor::Agent,
            format!(
                "Model route selected: {} ({}) via {}",
                route.model, route.provider, route.route_type
            ),
            serde_json::json!({
                "provider": route.provider,
                "model": route.model,
                "route_type": route.route_type,
                "prefer_local": route.prefer_local,
                "privacy_level": route.privacy_level.to_string(),
                "reason": route.reason,
            }),
        );

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
            model_route: Some(route),
        })
    }

    /// Streaming variant of generate_response. Uses governed runtime and
    /// forwards each chunk through the callback.
    pub(crate) async fn generate_response_streaming(
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

        self.record_runtime_governance_events(
            run_id,
            actx.agent_spec,
            &runtime_output,
            actx.privacy_policy,
        );

        let tools_prompt = if actx.tools_prompt.trim().is_empty() {
            None
        } else {
            Some(actx.tools_prompt)
        };

        let route = self.scheduler.preview_chat_route(tools_prompt).await;
        self.try_record_event(
            run_id,
            AgentRunEventType::ModelRouteSelected,
            AgentEventActor::Agent,
            format!(
                "Model route selected: {} ({}) via {}",
                route.model, route.provider, route.route_type
            ),
            serde_json::json!({
                "provider": route.provider,
                "model": route.model,
                "route_type": route.route_type,
                "prefer_local": route.prefer_local,
                "privacy_level": route.privacy_level.to_string(),
                "reason": route.reason,
            }),
        );

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
        let mut pending_visible_reply = String::new();
        let mut visible_stream_started = false;
        loop {
            match stream.next().await {
                Some(Ok(chunk)) => {
                    reply.push_str(&chunk);
                    if visible_stream_started {
                        callback.on_chunk(&chunk, 0, "generating").await;
                    } else {
                        pending_visible_reply.push_str(&chunk);
                        if !super::context::should_hold_streaming_reply(&pending_visible_reply) {
                            visible_stream_started = true;
                            callback
                                .on_chunk(&pending_visible_reply, 0, "generating")
                                .await;
                            pending_visible_reply.clear();
                        }
                    }
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
            model_route: Some(route),
        })
    }

    /// Execute a single step of the agent loop.
    /// If `callback` is provided, uses streaming generation and emits tool events.
    pub(crate) async fn run_single_step(
        &self,
        mut ctx: StepContext<'_>,
        callback: Option<Arc<dyn StreamingCallback>>,
    ) -> Result<StepResult> {
        let mut status_updates: Vec<AgentLoopStatusUpdate> = Vec::new();

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
                if ctx.run.model_route.is_none() {
                    ctx.run.model_route = gen.model_route.clone();
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
                        AgentLoopPhase::Thinking,
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
                        AgentLoopPhase::GeneratingFinal,
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
                    AgentLoopPhase::PlanningTool,
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
                    AgentLoopPhase::Failed,
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
}
