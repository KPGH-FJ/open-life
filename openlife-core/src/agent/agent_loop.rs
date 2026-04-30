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
use serde_json::Value;
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
            max_steps: 5,
            max_tool_calls: 3,
            timeout_seconds: 120,
            allow_writes: true,
            allow_cloud: true,
        }
    }
}

/// Result of running the agent loop.
#[derive(Debug, Clone)]
pub struct AgentLoopResult {
    pub run: AgentRun,
    pub final_response: String,
    pub stop_reason: String,
    pub tool_call_count: u32,
    pub step_count: u32,
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
    _scheduler: InferenceScheduler,
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
            _scheduler: scheduler,
            config,
        }
    }

    /// Run the 2-step agent loop for a given task.
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
        let mut _final_response = String::new();
        let mut _stop_reason = String::new();

        // Step 1: Generate initial model response
        let runtime_output = self
            .generate_response(
                task,
                life_model,
                tools_prompt,
                memory_context.clone(),
                privacy_engine.clone(),
            )
            .await
            .inspect_err(|e| {
                run.status = AgentRunStatus::Failed;
                run.error = Some(AgentRunError {
                    message: e.to_string(),
                    phase: "model".into(),
                    recoverable: false,
                });
            })?;

        step_count += 1;
        run.context_summary = Some(runtime_output.context_summary.clone());
        run.reasoning_trace = Some(runtime_output.reasoning_trace.clone());
        run.reasoning_strategy = Some(if task.layer == Layer::L3 {
            "layered".into()
        } else {
            "direct".into()
        });

        // Extract the assistant's reply text
        let first_reply = runtime_output
            .final_messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Check for tool calls in the reply
        let tool_actions =
            self.parse_tool_calls(&first_reply, action_ctx, &mut run, &mut tool_call_count)?;

        let has_tool_calls = !tool_actions.is_empty();

        if has_tool_calls {
            // Execute tools and build observations
            let mut observations = Vec::new();
            let mut all_succeeded = true;

            for action_request in tool_actions {
                if tool_call_count >= self.config.max_tool_calls {
                    let obs = self.create_budget_exceeded_observation(&run, tool_call_count);
                    observations.push(obs.clone());
                    run.observations.push(obs);
                    all_succeeded = false;
                    break;
                }

                let exec_result = self.action_executor.execute(action_request, action_ctx)?;
                run.actions.push(exec_result.action.clone());

                observations.push(exec_result.observation.clone());
                run.observations.push(exec_result.observation);
                if exec_result.status != ActionExecutionStatus::Succeeded {
                    all_succeeded = false;
                }

                tool_call_count += 1;
            }

            // Check timeout
            if start_time.elapsed().as_secs() >= self.config.timeout_seconds {
                run.status = AgentRunStatus::Failed;
                run.error = Some(AgentRunError {
                    message: "Agent loop timeout exceeded".into(),
                    phase: "execution".into(),
                    recoverable: false,
                });
                _stop_reason = "timeout".into();
                _final_response = "执行超时，请稍后重试。".into();
                return Ok(self.build_result(
                    run,
                    _final_response,
                    _stop_reason,
                    tool_call_count,
                    step_count,
                ));
            }

            // Step 2: Follow-up response with tool results
            if all_succeeded && !observations.is_empty() {
                let follow_up_messages =
                    self.build_follow_up_messages(task, &first_reply, &observations);
                let follow_up_task = AgentTask {
                    messages: follow_up_messages,
                    ..task.clone()
                };

                let follow_up_output = self
                    .generate_response(
                        &follow_up_task,
                        life_model,
                        "",
                        memory_context,
                        privacy_engine,
                    )
                    .await
                    .inspect_err(|e| {
                        run.status = AgentRunStatus::Failed;
                        run.error = Some(AgentRunError {
                            message: e.to_string(),
                            phase: "follow_up".into(),
                            recoverable: false,
                        });
                    })?;

                step_count += 1;
                _final_response = follow_up_output
                    .final_messages
                    .last()
                    .map(|m| m.content.clone())
                    .unwrap_or_else(|| first_reply.clone());
                _stop_reason = "completed_with_follow_up".into();
            } else if !all_succeeded {
                // Some tools failed or need confirmation
                let pending_count = run
                    .actions
                    .iter()
                    .filter(|a| a.status == "needs_confirmation")
                    .count();
                if pending_count > 0 {
                    _final_response =
                        "我需要先执行一些高风险或含敏感参数的工具操作，确认后才能继续给你结果。"
                            .into();
                    _stop_reason = "needs_confirmation".into();
                } else {
                    _final_response = "工具执行过程中出现错误，请检查配置或稍后重试。".into();
                    _stop_reason = "tool_execution_failed".into();
                }
            } else {
                _final_response = first_reply.clone();
                _stop_reason = "no_observations".into();
            }
        } else {
            // No tool calls, use first reply directly
            _final_response = first_reply;
            _stop_reason = "no_tools".into();
        }

        run.status = AgentRunStatus::Completed;
        run.output_preview = Some(preview_text(&_final_response, 200));
        run.finished_at = Some(chrono::Utc::now());

        Ok(self.build_result(
            run,
            _final_response,
            _stop_reason,
            tool_call_count,
            step_count,
        ))
    }

    async fn generate_response(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        privacy_engine: PrivacyEngine,
    ) -> Result<AgentRuntimeOutput> {
        let memory_hits = Vec::new(); // Simplified for Beta MVP
        self.runtime
            .execute_task(
                task,
                life_model,
                tools_prompt,
                memory_context,
                memory_hits,
                privacy_engine,
            )
            .await
            .map_err(|e| anyhow::anyhow!("runtime execution failed: {}", e))
    }

    fn parse_tool_calls(
        &self,
        reply: &str,
        _action_ctx: &ActionExecutionContext<'_>,
        run: &mut AgentRun,
        tool_call_count: &mut u32,
    ) -> Result<Vec<AgentActionRequest>> {
        let json_str = try_extract_json(reply);
        let Some(json_str) = json_str else {
            return Ok(Vec::new());
        };

        let v: Value =
            serde_json::from_str(json_str).map_err(|e| anyhow::anyhow!("invalid JSON: {}", e))?;
        let calls = match v.get("tool_calls").and_then(|c| c.as_array()) {
            Some(calls) if !calls.is_empty() => calls,
            _ => return Ok(Vec::new()),
        };

        let mut requests = Vec::new();
        for (idx, call) in calls.iter().enumerate() {
            let name = call
                .get("name")
                .and_then(|n| n.as_str())
                .context("tool call missing name")?;
            let args = call
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            requests.push(AgentActionRequest {
                action_type: "mcp_tool".to_string(),
                target: name.to_string(),
                input: serde_json::json!({ "arguments": args }),
                source_run_id: Some(run.id.clone()),
                step_index: *tool_call_count + idx as u32,
            });
        }

        Ok(requests)
    }

    fn build_follow_up_messages(
        &self,
        task: &AgentTask,
        assistant_reply: &str,
        observations: &[AgentObservation],
    ) -> Vec<ChatMessage> {
        let mut messages = task.messages.clone();
        messages.push(ChatMessage {
            role: "assistant".into(),
            content: assistant_reply.into(),
        });

        let results_text = observations
            .iter()
            .map(|obs| format!("工具结果: {}", obs.content))
            .collect::<Vec<_>>()
            .join("\n");

        messages.push(ChatMessage {
            role: "user".into(),
            content: format!("[系统] 工具执行结果:\n{}", results_text),
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
        run: AgentRun,
        final_response: String,
        stop_reason: String,
        tool_call_count: u32,
        step_count: u32,
    ) -> AgentLoopResult {
        AgentLoopResult {
            run,
            final_response,
            stop_reason,
            tool_call_count,
            step_count,
        }
    }
}

fn preview_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
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
