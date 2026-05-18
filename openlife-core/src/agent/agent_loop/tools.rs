use anyhow::Result;
use std::sync::Arc;

use super::streaming::StreamingCallback;
use super::types::AgentLoopResult;
use super::types::StepResult;
use super::AgentLoop;
use crate::agent::action_executor::{ActionContext, ActionExecutionStatus, AgentActionRequest};
use crate::agent::trace_payloads;
use crate::agent::types::AgentAction;
use crate::agent::types::{
    AgentEventActor, AgentLoopPhase, AgentLoopStatusUpdate, AgentObservation, AgentRun,
    AgentRunEventType, AgentRunStatus,
};

impl AgentLoop {
    /// Filter tool actions by the configured allowlist.
    /// Returns the filtered list (empty if allowlist is not configured).
    pub(crate) fn filter_tools_by_allowlist(
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
    pub(crate) async fn execute_tool_batch(
        &self,
        tool_actions: &[AgentActionRequest],
        action_ctx: &ActionContext,
        run: &mut AgentRun,
        tool_call_count: &mut u32,
        callback: &Option<Arc<dyn StreamingCallback>>,
        status_updates: &mut Vec<AgentLoopStatusUpdate>,
    ) -> Result<(bool, u32, bool, Vec<AgentObservation>)> {
        let mut observations = Vec::new();
        let mut all_succeeded = true;
        let mut executed_this_step: u32 = 0;
        let mut budget_exceeded = false;

        for (idx, action_request) in tool_actions.iter().enumerate() {
            if *tool_call_count + executed_this_step >= self.config.max_tool_calls {
                let blocked_tool_name = action_request.target.clone();
                self.try_record_event(
                    &run.id,
                    AgentRunEventType::ToolCallBlocked,
                    AgentEventActor::Runtime,
                    "Tool call budget exceeded",
                    trace_payloads::build_tool_call_blocked_payload(
                        "blocked",
                        &blocked_tool_name,
                        "runtime",
                        None::<&str>,
                        Some("invalid_arguments"),
                        None::<&str>,
                        None::<&str>,
                        Some(serde_json::json!({
                            "max_tool_calls": self.config.max_tool_calls,
                            "current_count": *tool_call_count + executed_this_step,
                        })),
                    ),
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
                AgentLoopPhase::ExecutingTool,
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
                .await
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
                    let fail_action = AgentAction {
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
                AgentLoopPhase::Observing,
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
    pub(crate) fn handle_step_completion(
        &self,
        budget_exceeded: bool,
        all_succeeded: bool,
        observations: Vec<AgentObservation>,
        executed_this_step: u32,
        final_text: String,
        run: &mut AgentRun,
        status_updates: &mut Vec<AgentLoopStatusUpdate>,
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
                    AgentLoopPhase::WaitingPermission,
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

    pub(crate) fn create_budget_exceeded_observation(
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

    pub(crate) fn build_result(
        &self,
        mut run: AgentRun,
        final_response: String,
        stop_reason: String,
        tool_call_count: u32,
        step_count: u32,
        status_updates: Vec<AgentLoopStatusUpdate>,
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
