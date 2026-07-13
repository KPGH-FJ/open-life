use std::sync::Arc;
use std::task::Poll;

use crate::main_chat_generation_support::preview_text;
use crate::main_chat_react_runtime::{
    attach_main_chat_read_observation_metadata, blocked_main_chat_observation, MainChatObservation,
};
use crate::main_chat_react_tool_selection::{
    resolve_main_chat_mcp_read_target, MainChatReactActionPlan,
};
use crate::AppState;

#[derive(Debug)]
struct LocalToolAbort {
    receipt: Option<openlife_core::tool_execution_receipt::ToolExecutionReceipt>,
}

impl std::fmt::Display for LocalToolAbort {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Some(receipt) = &self.receipt else {
            return formatter.write_str("transport=not_attempted,effect=not_attempted");
        };
        write!(
            formatter,
            "receipt={},transport={:?},effect={:?}",
            receipt.receipt_id, receipt.transport_status, receipt.effect_status
        )
    }
}

type SharedToolReceiptRegistration = Arc<
    std::sync::Mutex<
        Option<openlife_core::tool_execution_receipt::ToolExecutionReceiptRegistration>,
    >,
>;

fn observed_tool_receipt_registration(
    registration: &SharedToolReceiptRegistration,
) -> Option<openlife_core::tool_execution_receipt::ToolExecutionReceiptRegistration> {
    registration
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

async fn await_or_local_abort<F, T>(
    future: F,
    cancellation_token: Option<&tokio_util::sync::CancellationToken>,
    receipt_registration: &SharedToolReceiptRegistration,
) -> Result<T, LocalToolAbort>
where
    F: std::future::Future<Output = T>,
{
    tokio::pin!(future);
    if let Some(cancellation_token) = cancellation_token {
        if cancellation_token.is_cancelled() {
            return Err(LocalToolAbort {
                receipt: observed_tool_receipt_registration(receipt_registration)
                    .map(|registration| registration.settle_after_local_abort()),
            });
        }
        tokio::select! {
            biased;
            result = &mut future => Ok(result),
            _ = cancellation_token.cancelled() => {
                let registration = observed_tool_receipt_registration(receipt_registration);
                let receipt_before_abort = registration
                    .as_ref()
                    .map(|registration| registration.snapshot());
                let response_or_effect_was_confirmed = receipt_before_abort.as_ref().is_some_and(|receipt| {
                    receipt.transport_status
                        == openlife_core::tool_execution_receipt::ToolTransportStatus::ResponseObserved
                        || receipt.effect_status
                            == openlife_core::tool_execution_receipt::ToolEffectStatus::Confirmed
                });

                if response_or_effect_was_confirmed {
                    // The transport boundary has already observed the response.
                    // Poll the gateway future once more: after that observation
                    // the gateway has no external await left, so a simultaneously
                    // ready successful result must beat local cancellation.
                    let immediate = std::future::poll_fn(|context| {
                        Poll::Ready(std::future::Future::poll(future.as_mut(), context))
                    })
                    .await;
                    if let Poll::Ready(result) = immediate {
                        return Ok(result);
                    }
                }

                if let Some(registration) = registration {
                    Err(LocalToolAbort {
                        receipt: Some(registration.settle_after_local_abort()),
                    })
                } else {
                    Err(LocalToolAbort { receipt: None })
                }
            },
        }
    } else {
        Ok(future.await)
    }
}

pub(crate) async fn execute_main_chat_react_action_with_tool_gateway(
    state: &Arc<AppState>,
    plan: &MainChatReactActionPlan,
    local_only_required: bool,
    dispatch_observer: Option<&dyn openlife_core::agent::ToolDispatchObserver>,
    started_observer: Option<&dyn openlife_core::agent::ToolStartedTransitionObserver>,
    source_run_id: Option<&str>,
    cancellation_token: Option<&tokio_util::sync::CancellationToken>,
    execution_epoch: Option<&crate::main_chat_cancellation::MainChatExecutionEpoch>,
    action_bound_permission: Option<
        &openlife_core::tool_permissions::ActionBoundToolPermissionAuthorization,
    >,
) -> Result<MainChatObservation, String> {
    if local_only_required && plan.requires_network {
        return Err(
            "local-only policy blocks governed network reads for this Main Chat request".into(),
        );
    }

    let resources =
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_execution(
            state,
        )
        .await?;
    let (safe_paths, calendar_ics_paths, network_policy) = {
        let governed = &resources.governed;
        let mut safe_paths = governed.shared.safe_paths.clone();
        if let Ok(workspace) =
            crate::workspace_file_resolver::resolve_workspace_root().or_else(|_| {
                std::env::current_dir()
                    .and_then(|dir| dir.canonicalize())
                    .map_err(|err| err.to_string())
            })
        {
            let workspace = workspace.to_string_lossy().to_string();
            if !safe_paths.iter().any(|path| path == &workspace) {
                safe_paths.push(workspace);
            }
        }
        (
            safe_paths,
            governed.calendar_ics_paths.clone(),
            governed.network_policy.clone(),
        )
    };
    let web_search_fixture_output = state.web_search_fixture_output.lock().await.clone();
    let local_permission_store = if plan.uses_ephemeral_file_permission {
        let store = openlife_core::tool_permissions::ToolPermissionStore::new_in_memory()
            .map_err(|err| format!("create ephemeral tool permission store failed: {err}"))?;
        store
            .grant(
                "file.read",
                "builtin",
                "low",
                "read",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                None,
            )
            .map_err(|err| format!("grant ephemeral file.read permission failed: {err}"))?;
        Some(store)
    } else {
        None
    };
    let permission_store = if let Some(store) = local_permission_store {
        store
    } else {
        resources.governed.shared.permission_store.clone()
    };
    let mcp_read_resolution =
        resolve_main_chat_mcp_read_target(&resources.governed.shared.registry, plan);
    if let Some(ref blocker_reason) = mcp_read_resolution.blocker_reason {
        return Ok(blocked_main_chat_observation(
            plan,
            blocker_reason,
            serde_json::json!({
                "mcpReadTargetResolved": mcp_read_resolution.resolved,
                "mcpReadTarget": mcp_read_resolution.target,
            }),
        ));
    }

    let mut action_ctx = openlife_core::agent::ActionExecutionContext::new(
        &resources.governed.shared.registry,
        &permission_store,
        &resources.governed.shared.audit_store,
        &resources.governed.shared.privacy_engine,
        &safe_paths,
    )
    .with_memory_store(&resources.governed.memory_store)
    .with_network_policy(&network_policy)
    .with_calendar_ics_paths(&calendar_ics_paths);
    if let Some(retrieval_reader) = resources
        .governed
        .memory_lifecycle_retrieval_reader
        .as_ref()
    {
        action_ctx = action_ctx.with_memory_lifecycle_retrieval_reader(retrieval_reader);
    }
    action_ctx = action_ctx.with_agent_run_store(&resources.agent_run_store);
    let bound_action_permission = action_bound_permission
        .cloned()
        .map(|authorization| {
            authorization.bind_execution(
                openlife_core::tool_permissions::ActionBoundToolExecutionBinding {
                    queue_action_type: plan.queue_action_type.clone(),
                    requested_target: plan.target.clone(),
                },
            )
        })
        .transpose()
        .map_err(|error| format!("bind action-bound ToolPermission failed: {error}"))?;
    if let Some(action_bound_permission) = bound_action_permission.as_ref() {
        action_ctx = action_ctx.with_action_bound_tool_permission(action_bound_permission);
    }
    if let Some(ref fixture_output) = web_search_fixture_output {
        action_ctx = action_ctx.with_web_search_fixture_output(fixture_output);
    }
    if let Some(dispatch_observer) = dispatch_observer {
        action_ctx = action_ctx.with_tool_dispatch_observer(dispatch_observer);
    }
    if let Some(started_observer) = started_observer {
        action_ctx = action_ctx.with_tool_started_transition_observer(started_observer);
    }

    let request_input = if plan.executor_action_type == "mcp_tool" {
        serde_json::json!({ "arguments": mcp_read_resolution.arguments.clone() })
    } else {
        mcp_read_resolution.arguments.clone()
    };
    let request = openlife_core::agent::AgentActionRequest {
        action_type: plan.executor_action_type.clone(),
        target: mcp_read_resolution.target.clone(),
        input: request_input,
        source_run_id: source_run_id.map(str::to_string),
        step_index: 0,
    };
    let gateway = openlife_core::agent::ToolGateway::from_executor_config(
        openlife_core::agent::ActionExecutorConfig {
            allow_writes: false,
            ..Default::default()
        },
    );
    let receipt_registration: SharedToolReceiptRegistration = Arc::new(std::sync::Mutex::new(None));
    let receipt_registration_observer = Arc::clone(&receipt_registration);
    let execution_epoch = execution_epoch.cloned();
    let execution =
        gateway.execute_with_receipt_registration_sink(request, &action_ctx, move |registration| {
            if let Some(execution_epoch) = execution_epoch.as_ref() {
                execution_epoch.observe_tool_execution(registration.clone());
            }
            *receipt_registration_observer
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(registration);
        });
    let result = await_or_local_abort(execution, cancellation_token, &receipt_registration)
        .await
        .map_err(|abort| format!("ToolGateway locally aborted by Main Chat cancellation: {abort}"))?
        .map_err(|err| format!("ToolGateway failed: {err}"))?;

    let executor_status = result.status.clone();
    let status_label = match executor_status {
        openlife_core::agent::ActionExecutionStatus::Succeeded => "succeeded",
        openlife_core::agent::ActionExecutionStatus::Failed => "failed",
        openlife_core::agent::ActionExecutionStatus::Blocked => "blocked",
        openlife_core::agent::ActionExecutionStatus::NeedsConfirmation => "needs_confirmation",
    };
    let blocker_reason = result
        .stop_reason
        .clone()
        .or_else(|| result.action.error.clone())
        .or_else(|| {
            result
                .observation
                .structured_result
                .as_ref()
                .and_then(|value| value.get("permission_decision"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
    let output_preview = preview_text(&result.observation.content, 500);
    let tool_execution_receipt = result.execution_receipt.clone();
    let proposal_id = result
        .action
        .react_trace
        .as_ref()
        .and_then(|trace| trace.proposal_id.clone())
        .or_else(|| {
            result
                .observation
                .react_trace
                .as_ref()
                .and_then(|trace| trace.proposal_id.clone())
        })
        .or_else(|| {
            result
                .observation
                .structured_result
                .as_ref()
                .and_then(|structured| {
                    structured
                        .get("proposalId")
                        .or_else(|| structured.get("proposal_id"))
                })
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
    let mut metadata = serde_json::json!({
        "actionType": plan.queue_action_type.clone(),
        "executorActionType": plan.executor_action_type.clone(),
        "target": mcp_read_resolution.target.clone(),
        "requestedTarget": plan.target.clone(),
        "argumentsDigest": openlife_core::agent::metadata_safe::metadata_safe_value_digest(&mcp_read_resolution.arguments),
        "toolGatewayAuthority": true,
        "actionExecutorBacked": true,
        "mcpReadTargetResolved": mcp_read_resolution.resolved,
        "executorStatus": status_label,
        "actionId": result.action.id,
        "observationId": result.observation.id,
        "stopReason": result.stop_reason,
        "structuredResult": result.observation.structured_result,
        "toolExecutionReceipt": tool_execution_receipt,
        "directWritesExecuted": false,
    });
    attach_main_chat_read_observation_metadata(
        &mut metadata,
        &plan.queue_action_type,
        &mcp_read_resolution.target,
        &mcp_read_resolution.arguments,
        &output_preview,
        result.observation.structured_result.clone(),
        web_search_fixture_output.is_some() && plan.queue_action_type == "web.search",
        executor_status == openlife_core::agent::ActionExecutionStatus::Succeeded,
    );
    if let Some(ref proposal_id) = proposal_id {
        if let Some(object) = metadata.as_object_mut() {
            object.insert("proposalId".into(), serde_json::json!(proposal_id));
        }
    }

    Ok(MainChatObservation {
        metadata,
        executor_status,
        blocker_reason,
        tool_execution_receipt: Some(tool_execution_receipt),
    })
}

#[cfg(test)]
mod cancellation_tests {
    use super::{await_or_local_abort, SharedToolReceiptRegistration};
    use openlife_core::tool_execution_receipt::{
        ToolEffectStatus, ToolExecutionReceiptRegistration, ToolTransportStatus,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct DropProbe(Arc<AtomicBool>);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn local_cancel_drops_the_inflight_tool_future_without_waiting_for_remote_completion() {
        let dropped = Arc::new(AtomicBool::new(false));
        let future_dropped = Arc::clone(&dropped);
        let cancellation = tokio_util::sync::CancellationToken::new();
        let cancellation_for_task = cancellation.clone();
        let registration = ToolExecutionReceiptRegistration::test_never_dispatched_read(
            None,
            Some("test-read".into()),
            "sha256:not-attempted".into(),
        );
        let shared_tracker: SharedToolReceiptRegistration =
            Arc::new(std::sync::Mutex::new(Some(registration)));
        let tracker_for_task = Arc::clone(&shared_tracker);
        let task = tokio::spawn(async move {
            await_or_local_abort(
                async move {
                    let _probe = DropProbe(future_dropped);
                    std::future::pending::<()>().await;
                },
                Some(&cancellation_for_task),
                &tracker_for_task,
            )
            .await
        });

        tokio::task::yield_now().await;
        cancellation.cancel();
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .expect("local cancellation must wake within one second")
            .expect("join cancellation task");
        let abort = result.expect_err("local cancellation must abort a pending tool future");
        let receipt = abort
            .receipt
            .expect("gateway tracker must remain observable");
        assert_eq!(receipt.transport_status, ToolTransportStatus::NotAttempted);
        assert_eq!(receipt.effect_status, ToolEffectStatus::NotAttempted);
        assert!(
            dropped.load(Ordering::SeqCst),
            "the in-flight tool future must be dropped after local cancellation"
        );
    }

    #[tokio::test]
    async fn response_observed_result_wins_when_result_and_cancel_are_ready_together() {
        let cancellation = tokio_util::sync::CancellationToken::new();
        let cancellation_for_task = cancellation.clone();
        let registration = ToolExecutionReceiptRegistration::test_observed_external_mutation(
            None,
            Some("test-external-effect".into()),
            "sha256:confirmed".into(),
        );
        let shared_tracker: SharedToolReceiptRegistration =
            Arc::new(std::sync::Mutex::new(Some(registration.clone())));
        let tracker_for_task = Arc::clone(&shared_tracker);
        let (send_result, receive_result) = tokio::sync::oneshot::channel::<&'static str>();
        let task = tokio::spawn(async move {
            await_or_local_abort(
                async move { receive_result.await.expect("send gateway result") },
                Some(&cancellation_for_task),
                &tracker_for_task,
            )
            .await
        });

        tokio::task::yield_now().await;
        send_result.send("confirmed-result").unwrap();
        cancellation.cancel();

        let result = task.await.expect("join simultaneous completion task");
        assert_eq!(
            result.expect("confirmed response must win"),
            "confirmed-result"
        );
        assert_eq!(
            registration.snapshot().effect_status,
            ToolEffectStatus::Confirmed
        );
    }

    #[tokio::test]
    async fn dispatched_without_response_becomes_remote_unknown_with_unknown_effect() {
        let cancellation = tokio_util::sync::CancellationToken::new();
        let cancellation_for_task = cancellation.clone();
        let registration = ToolExecutionReceiptRegistration::test_inflight_network_mutation(
            None,
            Some("test-external-effect".into()),
            "sha256:unknown".into(),
        );
        let shared_tracker: SharedToolReceiptRegistration =
            Arc::new(std::sync::Mutex::new(Some(registration)));
        let tracker_for_task = Arc::clone(&shared_tracker);
        let task = tokio::spawn(async move {
            await_or_local_abort(
                std::future::pending::<()>(),
                Some(&cancellation_for_task),
                &tracker_for_task,
            )
            .await
        });

        tokio::task::yield_now().await;
        cancellation.cancel();

        let abort = task
            .await
            .expect("join dispatched cancellation task")
            .expect_err("dispatched request without response cannot report completion");
        let receipt = abort.receipt.expect("receipt must survive dropped future");
        assert_eq!(receipt.transport_status, ToolTransportStatus::RemoteUnknown);
        assert_eq!(receipt.effect_status, ToolEffectStatus::Unknown);
        assert!(receipt.response_observed_at.is_none());
    }
}
