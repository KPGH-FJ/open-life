use crate::main_chat_runtime_support::{finalize_main_chat_task_failure, MainChatTaskFailureKind};
use crate::main_chat_task_controls::{
    cancel_main_chat_agent_task, get_main_chat_agent_task_detail, get_main_chat_agent_task_state,
    list_main_chat_agent_tasks, refresh_main_chat_agent_task_context, resume_main_chat_agent_task,
    MainChatAgentTaskFilter, MainChatAgentTaskState,
};
use tauri::Manager;

async fn create_task_bound_agent_run_for_test(
    state: &std::sync::Arc<crate::AppState>,
    task_session_id: &str,
    chat_session_id: &str,
    user_input: &str,
) -> String {
    create_task_bound_agent_run_with_status_for_test(
        state,
        task_session_id,
        chat_session_id,
        user_input,
        openlife_core::agent::AgentRunStatus::Completed,
    )
    .await
}

async fn create_task_bound_agent_run_with_status_for_test(
    state: &std::sync::Arc<crate::AppState>,
    task_session_id: &str,
    chat_session_id: &str,
    user_input: &str,
    status: openlife_core::agent::AgentRunStatus,
) -> String {
    let mut run = openlife_core::agent::AgentRun::new_chat_run(chat_session_id, user_input);
    run.task_id = task_session_id.to_string();
    run.status = status;
    run.finished_at = matches!(
        status,
        openlife_core::agent::AgentRunStatus::Completed
            | openlife_core::agent::AgentRunStatus::Failed
            | openlife_core::agent::AgentRunStatus::Cancelled
    )
    .then(chrono::Utc::now);
    let run_id = run.id.clone();
    let store = state
        .agent_run_store
        .as_ref()
        .expect("agent run store")
        .lock()
        .await;
    store.create_run(&run).expect("create task-bound AgentRun");
    run_id
}

async fn replay_execution_envelope_for_test(
    state: &std::sync::Arc<crate::AppState>,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    queued: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
    run_id: &str,
) -> crate::main_chat_replay_contract::DurableMainChatReplayExecutionEnvelope {
    let plan = crate::main_chat_react_tool_selection::build_main_chat_react_action_plan(
        &session.chat_session_id,
        &session.user_goal,
    )
    .expect("build replay fixture plan");
    let (resolution, manifest) = {
        let registry = state.mcp_registry.lock().await;
        let resolution = crate::main_chat_react_tool_selection::resolve_main_chat_mcp_read_target(
            &registry, &plan,
        );
        assert!(
            resolution.blocker_reason.is_none(),
            "replay fixture target must resolve: {:?}",
            resolution.blocker_reason
        );
        let manifests = registry
            .list_manifests()
            .into_iter()
            .filter(|manifest| {
                manifest.id == resolution.target || manifest.name == resolution.target
            })
            .collect::<Vec<_>>();
        let [manifest] = manifests.as_slice() else {
            panic!("replay fixture manifest identity must be unique");
        };
        (resolution, manifest.clone())
    };
    let executor_action_id = format!("fixture-executor:{}", queued.id);
    crate::main_chat_replay_contract::DurableMainChatReplayExecutionEnvelope::new(
        crate::main_chat_replay_contract::DurableMainChatReplayExecutionInput {
            task_session_id: &session.id,
            run_id,
            queue_action_id: &queued.id,
            executor_action_id: &executor_action_id,
            queue_action_type: &plan.queue_action_type,
            executor_action_type: &plan.executor_action_type,
            requested_target: &plan.target,
            resolved_target: &resolution.target,
            manifest: &manifest,
            input: &resolution.arguments,
        },
    )
    .expect("build replay fixture durable envelope")
}

fn metadata_with_replay_envelope(
    mut metadata: serde_json::Value,
    envelope: &crate::main_chat_replay_contract::DurableMainChatReplayExecutionEnvelope,
) -> serde_json::Value {
    envelope
        .attach_to_metadata(&mut metadata)
        .expect("attach replay fixture envelope");
    metadata
}

fn bind_tool_permission_proposal_to_replay_for_test(
    proposal: &mut openlife_core::agent::AgentProposal,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    envelope: &crate::main_chat_replay_contract::DurableMainChatReplayExecutionEnvelope,
) {
    proposal.run_id = Some(envelope.run_id.clone());
    proposal.source = openlife_core::agent::ProposalSource::ChatConversation;
    proposal.source_detail = Some(format!("main_chat_agent_task_session:{}", session.id));
    let after = proposal
        .after
        .as_object_mut()
        .expect("ToolPermission fixture after object");
    after.insert(
        "permission_scope_kind".into(),
        serde_json::json!("action_bound"),
    );
    after.insert("permission".into(), serde_json::json!("allow_once"));
    after.insert("policy".into(), serde_json::json!("allow_once"));
    after.insert(
        "blocked_action".into(),
        serde_json::json!({
            "action_type": envelope.queue_action_type,
            "target": envelope.requested_target,
            "resolved_target": envelope.resolved_target,
            "queue_action_id": envelope.queue_action_id,
            "executor_action_id": envelope.executor_action_id,
            "source_run_id": envelope.run_id,
            "source_task_session_id": envelope.task_session_id,
            "input_hash": envelope.input_hash,
            "input_length_bytes": envelope.input_length_bytes,
            "directWritesExecuted": false,
        }),
    );
    after.insert(
        "pending_action_identity".into(),
        serde_json::json!({
            "taskSessionId": envelope.task_session_id,
            "runId": envelope.run_id,
            "queueActionId": envelope.queue_action_id,
            "executorActionId": envelope.executor_action_id,
            "queueActionType": envelope.queue_action_type,
            "executorActionType": envelope.executor_action_type,
            "requestedTarget": envelope.requested_target,
            "resolvedTarget": envelope.resolved_target,
            "manifestId": envelope.manifest_id,
            "manifestName": envelope.manifest_name,
            "manifestSource": envelope.manifest_source,
            "manifestContractDigest": envelope.manifest_contract_digest,
            "inputHash": envelope.input_hash,
            "inputLengthBytes": envelope.input_length_bytes,
            "directWritesExecuted": false,
        }),
    );
}

fn project_test_read_receipt(
    queue: &openlife_core::agent::main_chat_agent_v1::ActionQueueStore,
    queued: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
    execution_status: openlife_core::agent::ActionExecutionStatus,
    metadata: serde_json::Value,
    error: Option<&str>,
) -> openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction {
    let envelope =
        crate::main_chat_replay_contract::DurableMainChatReplayExecutionEnvelope::from_action_metadata(
            &metadata,
        )
        .ok();
    let source_run_id = envelope
        .as_ref()
        .map(|envelope| envelope.run_id.clone())
        .unwrap_or_else(|| format!("test-run:{}", queued.session_id));
    let manifest_id = envelope
        .as_ref()
        .map(|envelope| envelope.manifest_id.clone())
        .unwrap_or_else(|| format!("test-manifest:{}", queued.action.action_type));
    let receipt = if execution_status == openlife_core::agent::ActionExecutionStatus::Succeeded {
        openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_observed_local_read(
            Some(source_run_id.clone()),
            Some(manifest_id.clone()),
            format!("sha256:test-request:{}", queued.id),
            true,
        )
    } else {
        openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_gateway_failed_before_dispatch(
            Some(source_run_id),
            Some(manifest_id),
            format!("sha256:test-request:{}", queued.id),
            openlife_core::tool_execution_receipt::ToolActionEffect::ReadOnly,
            openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent,
        )
    };
    if let Some(envelope) = envelope.as_ref() {
        assert!(receipt.test_bind_to_action_metadata(
            &envelope.run_id,
            &envelope.executor_action_id,
            &envelope.executor_action_type,
            Some(&envelope.resolved_target),
            &envelope.input_hash,
            envelope.input_length_bytes,
        ));
    }
    queue
        .project_initial_tool_execution_receipt(
            &queued.id,
            queued.status,
            queued.revision,
            openlife_core::agent::main_chat_agent_v1::InitialToolExecutionProjection {
                execution_status,
                receipt: &receipt,
                observation_metadata: Some(metadata),
                error: error.map(str::to_string),
            },
        )
        .expect("project typed test ToolExecutionReceipt")
}

async fn create_failed_replay_task_for_test(
    state: &std::sync::Arc<crate::AppState>,
    chat_session_id: &str,
    user_goal: &str,
) -> (
    openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
    String,
) {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, ExecutionAction, ExecutionPolicy, MainChatAgentStrategy,
    };
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: chat_session_id.into(),
                user_goal: user_goal.into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create replay fixture task")
    };
    let run_id = create_task_bound_agent_run_with_status_for_test(
        state,
        &session.id,
        &session.chat_session_id,
        &session.user_goal,
        openlife_core::agent::AgentRunStatus::Failed,
    )
    .await;
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        let action = ExecutionAction::new("mcp.read_only", "Replay governed MCP read.");
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue replay fixture action");
        queued
    };
    let envelope = replay_execution_envelope_for_test(state, &session, &queued, &run_id).await;
    let failed = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        project_test_read_receipt(
            &queue,
            &queued,
            openlife_core::agent::ActionExecutionStatus::Failed,
            metadata_with_replay_envelope(
                serde_json::json!({"directWritesExecuted": false}),
                &envelope,
            ),
            Some("fixture failed before dispatch"),
        )
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &failed.id)
            .expect("link replay fixture action");
        store
            .fail_session(&session.id, "Fixture failed before dispatch.")
            .expect("mark replay fixture failed");
    }
    crate::main_chat_event_stream::append_main_chat_agent_runtime_event_batch(
        state,
        &session.id,
        &run_id,
        vec![
            crate::main_chat_event_stream::MainChatAgentRuntimeEventInput::new(
                "failed",
                "turn",
                format!("fixture-terminal:{run_id}"),
                "main_chat_task_control_tests",
                serde_json::json!({
                    "status": "failed",
                    "kind": "tool_error",
                }),
            ),
        ],
    )
    .await
    .expect("persist replay fixture terminal receipt");
    (session, failed, run_id)
}

#[test]
fn main_chat_task_control_tests_are_not_concentrated_in_lib_rs() {
    let lib_rs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");

    for forbidden in [
        "retry_main_chat_action_enters_manual_blocker_when_not_replayable",
        "retry_main_chat_action_replays_replayable_action_instead_of_state_only",
        "resume_main_chat_task_preserves_pending_permission_blocker_instead_of_state_only",
        "resume_main_chat_task_replays_pending_action_after_tool_permission_acceptance",
        "cancel_main_chat_task_cancels_nonterminal_queued_actions",
        "main_chat_task_controls_are_not_concentrated_in_lib_rs",
    ] {
        assert!(
            !source.contains(&format!("\n    fn {forbidden}("))
                && !source.contains(&format!("\n    async fn {forbidden}(")),
            "Main Chat task-control test {forbidden} should live outside src/lib.rs"
        );
    }
}

#[test]
fn retry_main_chat_action_requires_backend_target_and_typed_receipt() {
    let module_path = format!(
        "{}/src/main_chat_task_controls.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(module_path).expect("read task-control module");
    let retry_body =
        extract_rust_function_body(&source, "pub(crate) async fn retry_main_chat_agent_action(");

    assert!(
        retry_body.contains("action_not_current_backend_retry_target"),
        "retry command must reject caller-selected stale or unsafe actions"
    );
    assert!(
        retry_body.contains("typed_retry_receipt_required"),
        "retry command must require typed pre-dispatch evidence"
    );
    assert!(
        !retry_body.contains("manual_retry_replay_required"),
        "legacy manual replay path must not survive behind the typed retry target"
    );
}

#[test]
fn retry_main_chat_action_replays_replayable_action_instead_of_state_only() {
    let module_path = format!(
        "{}/src/main_chat_task_controls.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(module_path).expect("read task-control module");
    let retry_body =
        extract_rust_function_body(&source, "pub(crate) async fn retry_main_chat_agent_action(");

    assert!(
        retry_body.contains(".run_replay("),
        "replayable Main Chat retries must enter OpenLifeTurnRuntime instead of only changing queue state"
    );
    assert!(
        std::fs::read_to_string(format!(
            "{}/src/main_chat_turn_runtime.rs",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("read turn runtime")
        .contains("automaticReplayCompleted"),
        "automatic retry replay completion must be visible in task state/transcript metadata"
    );
}

#[tokio::test]
async fn retry_main_chat_action_claims_before_dispatch_and_confirms_one_execution() {
    use openlife_core::agent::main_chat_agent_v1::{
        ActionReplayClaimState, ActionReplayEffectCertainty, AgentTaskSessionDraft,
        AgentTaskSessionStatus, ExecutionAction, ExecutionPolicy, ExecutionQueueStatus,
        MainChatAgentStrategy,
    };
    use openlife_core::tool_permissions::ToolPermissionPolicy;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "retry-claim-product-path".into(),
                user_goal: "Use mcp builtin_echo read-only now.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create replay task")
    };
    let run_id = create_task_bound_agent_run_with_status_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        &session.user_goal,
        openlife_core::agent::AgentRunStatus::Failed,
    )
    .await;
    let manifest = {
        let registry = state.mcp_registry.lock().await;
        registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.name == "builtin_echo")
            .expect("builtin echo manifest")
    };
    {
        let permission_store = state.tool_permission_store.lock().await;
        permission_store
            .grant(
                &manifest.name,
                &openlife_core::agent::action_executor::helpers::canonical_tool_source(&manifest),
                &manifest.risk_level,
                &manifest.action_type,
                ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .expect("grant replay read permission");
    }
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        let action = ExecutionAction::new("mcp.read_only", "Retry governed builtin echo read.");
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue replay action");
        queued
    };
    let envelope = replay_execution_envelope_for_test(&state, &session, &queued, &run_id).await;
    let failed_action = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        project_test_read_receipt(
            &queue,
            &queued,
            openlife_core::agent::ActionExecutionStatus::Failed,
            metadata_with_replay_envelope(
                serde_json::json!({"directWritesExecuted": false}),
                &envelope,
            ),
            Some("fixture failed before dispatch"),
        )
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &failed_action.id)
            .expect("link action to task");
        store
            .fail_session(&session.id, "Fixture action failed before dispatch.")
            .expect("mark task failed");
    }
    crate::main_chat_event_stream::append_main_chat_agent_runtime_event_batch(
        &state,
        &session.id,
        &run_id,
        vec![
            crate::main_chat_event_stream::MainChatAgentRuntimeEventInput::new(
                "failed",
                "turn",
                format!("fixture-terminal:{run_id}"),
                "main_chat_task_control_tests",
                serde_json::json!({"status": "failed"}),
            ),
        ],
    )
    .await
    .expect("persist retry fixture terminal receipt");

    crate::main_chat_task_controls::retry_main_chat_agent_action(
        session.id.clone(),
        failed_action.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("claim-aware product retry succeeds");

    let replayed = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        queue
            .load(&failed_action.id)
            .expect("load replayed action")
            .expect("replayed action exists")
    };
    assert_eq!(replayed.status, ExecutionQueueStatus::Completed);
    assert_eq!(
        replayed.replay_effect_certainty,
        ActionReplayEffectCertainty::Confirmed
    );
    assert!(matches!(
        replayed.replay_claim,
        ActionReplayClaimState::Claimed { .. }
    ));
    assert_eq!(replayed.attempts, 1);
    let task = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .load_session(&session.id)
            .expect("load replay task")
            .expect("replay task exists")
    };
    assert_eq!(task.status, AgentTaskSessionStatus::Completed);
}

#[tokio::test]
async fn expired_owner_reclaim_after_durable_prepared_cannot_reach_real_adapter() {
    use openlife_core::agent::main_chat_agent_v1::{ActionReplayClaimState, ExecutionQueueStatus};
    use openlife_core::tool_manifest::ToolSource;
    use openlife_core::tool_permissions::ToolPermissionPolicy;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock app");
    let (session, failed, _run_id) = create_failed_replay_task_for_test(
        &state,
        "replay-owner-generation-race",
        "Use mcp builtin_echo read-only now.",
    )
    .await;
    let dispatch_count = std::sync::Arc::new(AtomicUsize::new(0));
    let manifest = {
        let mut registry = state.mcp_registry.lock().await;
        let manifest = registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.name == "builtin_echo")
            .expect("builtin echo manifest");
        registry.remove_builtins_by_source(|source| matches!(source, ToolSource::BuiltIn));
        let count = std::sync::Arc::clone(&dispatch_count);
        registry.register_builtin(
            manifest.clone(),
            Box::new(move |_arguments| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok("real adapter dispatch".into())
            }),
        );
        manifest
    };
    state
        .tool_permission_store
        .lock()
        .await
        .grant(
            &manifest.name,
            &openlife_core::agent::action_executor::helpers::canonical_tool_source(&manifest),
            &manifest.risk_level,
            &manifest.action_type,
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .expect("grant exact replay read permission");

    let (_barrier_guard, reached, release) =
        crate::main_chat_turn_runtime::install_main_chat_replay_prepared_fence_barrier_for_test(
            &session.id,
        );
    let replay = crate::main_chat_task_controls::retry_main_chat_agent_action(
        session.id.clone(),
        failed.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    );
    tokio::pin!(replay);
    tokio::select! {
        _ = reached.wait() => {}
        result = &mut replay => panic!("replay exited before durable prepared barrier: {result:?}"),
        _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {
            panic!("replay did not reach durable prepared barrier")
        }
    }

    let new_claim_id = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        let old_owner = queue
            .load(&failed.id)
            .expect("load old replay owner")
            .expect("old replay owner exists");
        assert_eq!(old_owner.status, ExecutionQueueStatus::Executing);
        let lease_expires_at = old_owner
            .replay_claim_lease_expires_at
            .expect("old replay lease");
        let report = queue
            .reconcile_expired_replay_claims_at_for_test(
                lease_expires_at + chrono::Duration::seconds(1),
            )
            .expect("expire old owner at the real prepared/adapter barrier");
        assert_eq!(report.released_expired_before_dispatch, 1);
        assert_eq!(report.quarantined_expired_unknown, 0);
        let released = queue
            .load(&failed.id)
            .expect("load released action")
            .expect("released action exists");
        assert_eq!(released.status, ExecutionQueueStatus::Failed);
        let new_claim = queue
            .claim_replay_for_test_fixture(
                &released.id,
                released.status,
                released.revision,
                &uuid::Uuid::new_v4().to_string(),
            )
            .expect("a new owner claims after exact safe lease release");
        new_claim.claim_id
    };

    let (_, replay_result) = tokio::join!(
        release.wait(),
        tokio::time::timeout(std::time::Duration::from_secs(3), &mut replay)
    );
    let replay_error = replay_result
        .expect("stale replay owner terminates promptly")
        .expect_err("stale owner must fail its last claim fence");
    assert!(
        replay_error.contains("replay_dispatch_preflight_claim_not_owned")
            || replay_error.contains("replay_claim"),
        "unexpected stale-owner error: {replay_error}"
    );
    assert_eq!(dispatch_count.load(Ordering::SeqCst), 0);
    let persisted = state
        .main_chat_action_queue_store
        .as_ref()
        .expect("action queue")
        .lock()
        .await
        .load(&failed.id)
        .expect("load replacement owner")
        .expect("replacement owner exists");
    assert!(matches!(
        persisted.replay_claim,
        ActionReplayClaimState::Claimed { ref claim_id } if claim_id == &new_claim_id
    ));
    assert!(persisted.replay_dispatch_started_at.is_none());
}

#[tokio::test]
async fn registry_revocation_after_prepared_fails_before_dispatch_without_unknown_effect() {
    use openlife_core::agent::main_chat_agent_v1::{
        ActionReplayClaimState, ActionReplayEffectCertainty, ExecutionQueueStatus,
    };
    use openlife_core::tool_manifest::ToolSource;
    use openlife_core::tool_permissions::ToolPermissionPolicy;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock app");
    let (session, failed, _run_id) = create_failed_replay_task_for_test(
        &state,
        "replay-final-registry-revocation",
        "Use mcp builtin_echo read-only now.",
    )
    .await;
    let dispatch_count = std::sync::Arc::new(AtomicUsize::new(0));
    let manifest = {
        let mut registry = state.mcp_registry.lock().await;
        let manifest = registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.name == "builtin_echo")
            .expect("builtin echo manifest");
        registry.remove_builtins_by_source(|source| matches!(source, ToolSource::BuiltIn));
        let count = std::sync::Arc::clone(&dispatch_count);
        registry.register_builtin(
            manifest.clone(),
            Box::new(move |_arguments| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok("stale adapter dispatch".into())
            }),
        );
        manifest
    };
    state
        .tool_permission_store
        .lock()
        .await
        .grant(
            &manifest.name,
            &openlife_core::agent::action_executor::helpers::canonical_tool_source(&manifest),
            &manifest.risk_level,
            &manifest.action_type,
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .expect("grant exact replay read permission");

    let (_barrier_guard, reached, release) =
        crate::main_chat_turn_runtime::install_main_chat_replay_prepared_fence_barrier_for_test(
            &session.id,
        );
    let replay = crate::main_chat_task_controls::retry_main_chat_agent_action(
        session.id.clone(),
        failed.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    );
    tokio::pin!(replay);
    tokio::select! {
        _ = reached.wait() => {}
        result = &mut replay => panic!("replay exited before durable prepared barrier: {result:?}"),
        _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {
            panic!("replay did not reach durable prepared barrier")
        }
    }
    {
        let mut registry = state.mcp_registry.lock().await;
        registry.remove_builtins_by_source(|source| matches!(source, ToolSource::BuiltIn));
    }
    let (_, replay_result) = tokio::join!(
        release.wait(),
        tokio::time::timeout(std::time::Duration::from_secs(3), &mut replay)
    );
    replay_result
        .expect("revoked replay terminates promptly")
        .expect("runtime handles a governed pre-dispatch failure");
    assert_eq!(dispatch_count.load(Ordering::SeqCst), 0);
    let persisted = state
        .main_chat_action_queue_store
        .as_ref()
        .expect("action queue")
        .lock()
        .await
        .load(&failed.id)
        .expect("load failed replay")
        .expect("failed replay exists");
    assert_eq!(persisted.status, ExecutionQueueStatus::Failed);
    assert_eq!(
        persisted.replay_effect_certainty,
        ActionReplayEffectCertainty::FailedBeforeDispatch
    );
    assert_eq!(persisted.replay_claim, ActionReplayClaimState::Unclaimed);
    assert!(persisted.replay_dispatch_started_at.is_none());
    assert_eq!(
        persisted
            .observation_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("adapterEdgeCrossed"))
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    let lifecycle_events = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .list(&session.id, 0, 100)
        .expect("list registry-revocation tool lifecycle")
        .into_iter()
        .filter(|event| {
            event.object_id == persisted.id || event.object_type == "tool_execution_receipt"
        })
        .collect::<Vec<_>>();
    assert!(
        lifecycle_events
            .iter()
            .any(|event| event.event_type == "tool.not_dispatched"),
        "the live sealed receipt must close its prepared fact explicitly"
    );
    assert!(lifecycle_events.iter().all(|event| !matches!(
        event.event_type.as_str(),
        "tool.started" | "tool.dispatch_ambiguous" | "tool.remote_unknown" | "tool.effect_unknown"
    )));
}

#[tokio::test]
async fn same_contract_registry_replacement_cannot_dispatch_stale_executor_snapshot() {
    use openlife_core::tool_manifest::ToolSource;
    use openlife_core::tool_permissions::ToolPermissionPolicy;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock app");
    let (session, failed, _run_id) = create_failed_replay_task_for_test(
        &state,
        "replay-registry-instance-replacement",
        "Use mcp builtin_echo read-only now.",
    )
    .await;
    let stale_dispatches = std::sync::Arc::new(AtomicUsize::new(0));
    let replacement_dispatches = std::sync::Arc::new(AtomicUsize::new(0));
    let manifest = {
        let mut registry = state.mcp_registry.lock().await;
        let manifest = registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.name == "builtin_echo")
            .expect("builtin echo manifest");
        registry.remove_builtins_by_source(|source| matches!(source, ToolSource::BuiltIn));
        let count = std::sync::Arc::clone(&stale_dispatches);
        registry.register_builtin(
            manifest.clone(),
            Box::new(move |_arguments| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok("stale adapter dispatch".into())
            }),
        );
        manifest
    };
    state
        .tool_permission_store
        .lock()
        .await
        .grant(
            &manifest.name,
            &openlife_core::agent::action_executor::helpers::canonical_tool_source(&manifest),
            &manifest.risk_level,
            &manifest.action_type,
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .expect("grant exact replay read permission");

    let (_barrier_guard, reached, release) =
        crate::main_chat_turn_runtime::install_main_chat_replay_prepared_fence_barrier_for_test(
            &session.id,
        );
    let replay = crate::main_chat_task_controls::retry_main_chat_agent_action(
        session.id.clone(),
        failed.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    );
    tokio::pin!(replay);
    tokio::select! {
        _ = reached.wait() => {}
        result = &mut replay => panic!("replay exited before durable prepared barrier: {result:?}"),
        _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {
            panic!("replay did not reach durable prepared barrier")
        }
    }
    {
        let mut registry = state.mcp_registry.lock().await;
        registry.remove_builtins_by_source(|source| matches!(source, ToolSource::BuiltIn));
        let count = std::sync::Arc::clone(&replacement_dispatches);
        registry.register_builtin(
            manifest,
            Box::new(move |_arguments| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok("replacement adapter dispatch".into())
            }),
        );
    }
    let (_, replay_result) = tokio::join!(
        release.wait(),
        tokio::time::timeout(std::time::Duration::from_secs(3), &mut replay)
    );
    replay_result
        .expect("replaced replay terminates promptly")
        .expect("runtime handles a governed stale-instance failure");
    assert_eq!(stale_dispatches.load(Ordering::SeqCst), 0);
    assert_eq!(replacement_dispatches.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn startup_projects_prepared_replay_unknown_before_claim_recovery() {
    use openlife_core::agent::main_chat_agent_v1::{
        ActionReplayClaimState, ActionReplayEffectCertainty, ExecutionQueueStatus,
    };

    #[derive(Default)]
    struct StartupSecretStore {
        values: std::sync::Mutex<std::collections::HashMap<String, String>>,
    }

    impl crate::secret_store::SecretStore for StartupSecretStore {
        fn get(&self, secret_ref: &str) -> anyhow::Result<Option<String>> {
            Ok(self.values.lock().unwrap().get(secret_ref).cloned())
        }

        fn set(&self, secret_ref: &str, value: &str) -> anyhow::Result<()> {
            self.values
                .lock()
                .unwrap()
                .insert(secret_ref.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, secret_ref: &str) -> anyhow::Result<()> {
            self.values.lock().unwrap().remove(secret_ref);
            Ok(())
        }
    }

    let directory = tempfile::tempdir().expect("temporary release-bootstrap state");
    let secrets = StartupSecretStore::default();
    let bootstrap = crate::bootstrap::bootstrap_with_secret_store_for_test(
        directory.path().to_path_buf(),
        &secrets,
    );
    let state = bootstrap.state;
    assert!(
        state
            .persistence_coordinator
            .startup_reconciliation_mutations_safe(),
        "fixture must exercise the real unsealed release-bootstrap admission"
    );
    {
        let mut registry = state.mcp_registry.lock().await;
        assert!(
            registry
                .list_manifests()
                .iter()
                .all(|manifest| manifest.id != "builtin_echo"),
            "release bootstrap must not provide the development echo utility"
        );
        let mut remote_manifest = registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.id == "web.search")
            .expect("release registry retains a typed read manifest for the crash fixture");
        remote_manifest.id = "builtin_echo".into();
        remote_manifest.name = "builtin_echo".into();
        remote_manifest.description = "Test-only remote crash fixture".into();
        remote_manifest.parameters = serde_json::json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"]
        });
        remote_manifest.permission_level = "low".into();
        remote_manifest.risk_level = "low".into();
        remote_manifest.capabilities = vec!["read".into()];
        remote_manifest.action_type = "read".into();
        remote_manifest.source = openlife_core::tool_manifest::ToolSource::Mcp {
            server_name: "startup-prepared-remote-fixture".into(),
        };
        registry.register_builtin(
            remote_manifest,
            Box::new(|_arguments| {
                anyhow::bail!("startup prepared crash fixture must never enter an adapter")
            }),
        );
    }
    let (session, failed, run_id) = create_failed_replay_task_for_test(
        &state,
        "startup-prepared-replay-order",
        "Use mcp builtin_echo read-only now.",
    )
    .await;
    let (claim, executing) = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        let claim = queue
            .claim_replay_for_test_fixture(
                &failed.id,
                failed.status,
                failed.revision,
                &uuid::Uuid::new_v4().to_string(),
            )
            .expect("claim replay before simulated crash");
        let retrying = queue
            .transition_claimed_replay(
                &failed.id,
                &claim.claim_id,
                failed.status,
                claim.revision,
                ExecutionQueueStatus::Retrying,
                None,
            )
            .expect("enter retrying before simulated crash");
        let executing = queue
            .transition_claimed_replay(
                &failed.id,
                &claim.claim_id,
                retrying.status,
                retrying.revision,
                ExecutionQueueStatus::Executing,
                None,
            )
            .expect("enter executing before simulated crash");
        (claim, executing)
    };
    let authority = executing
        .replay_authority
        .as_ref()
        .expect("canonical replay authority");
    let receipt_id = "receipt-startup-prepared-replay";
    let attempt = openlife_core::agent::ToolDispatchAttempt {
        receipt_id: receipt_id.into(),
        manifest_id: authority.manifest_id().into(),
        tool_name: authority.manifest_name().into(),
        manifest_contract_digest: authority.manifest_contract_digest().into(),
        input_hash: authority.input_hash().into(),
        input_length_bytes: authority.input_length_bytes(),
        source_run_id: Some(run_id.clone()),
        request_digest: format!("sha256:{}", "a".repeat(64)),
        action_effect: authority.action_effect(),
        idempotency_contract: authority.idempotency_contract(),
        process_risk:
            openlife_core::agent::action_executor::ToolDispatchProcessRisk::MayOutliveLocalProcess,
        effect_may_survive_local_process: false,
    };
    let observer = crate::main_chat_event_stream::MainChatToolLifecycleObserver::new(
        std::sync::Arc::clone(&state),
        &session.id,
        &run_id,
    )
    .with_replay_claim(&failed.id, &claim.claim_id, claim.owner_generation)
    .expect("bind exact replay claim generation to prepared observer");
    openlife_core::agent::ToolDispatchObserver::before_dispatch(&observer, &attempt)
        .await
        .expect("persist exact signed write-ahead prepared replay fact");

    crate::bootstrap::reconcile_startup_orphaned_main_chat_runs(&state)
        .await
        .expect("startup reconciliation applies event outbox before claim recovery");

    let recovered = state
        .main_chat_action_queue_store
        .as_ref()
        .expect("action queue")
        .lock()
        .await
        .load(&failed.id)
        .expect("load startup-reconciled action")
        .expect("startup-reconciled action exists");
    assert_eq!(recovered.status, ExecutionQueueStatus::Failed);
    assert_eq!(
        recovered.replay_effect_certainty,
        ActionReplayEffectCertainty::DispatchedUnknown
    );
    assert!(matches!(
        recovered.replay_claim,
        ActionReplayClaimState::Claimed { ref claim_id } if claim_id == &claim.claim_id
    ));
    assert!(
        recovered.replay_dispatch_started_at.is_none(),
        "prepared-only restart must not invent physical dispatch time"
    );
    let pending = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .pending_tool_queue_reconciliation_projections(10)
        .expect("list applied tool queue outbox");
    assert!(pending.items.is_empty());
}

#[tokio::test]
async fn durable_restart_projects_live_not_dispatched_once_before_safe_claim_recovery() {
    use openlife_core::agent::main_chat_agent_v1::{
        ActionReplayClaimState, ActionReplayEffectCertainty, ExecutionQueueStatus,
    };

    #[derive(Default)]
    struct RestartSecretStore {
        values: std::sync::Mutex<std::collections::HashMap<String, String>>,
    }

    impl crate::secret_store::SecretStore for RestartSecretStore {
        fn get(&self, secret_ref: &str) -> anyhow::Result<Option<String>> {
            Ok(self.values.lock().unwrap().get(secret_ref).cloned())
        }

        fn set(&self, secret_ref: &str, value: &str) -> anyhow::Result<()> {
            self.values
                .lock()
                .unwrap()
                .insert(secret_ref.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, secret_ref: &str) -> anyhow::Result<()> {
            self.values.lock().unwrap().remove(secret_ref);
            Ok(())
        }
    }

    let directory = tempfile::tempdir().expect("temporary durable restart state");
    let secrets = RestartSecretStore::default();
    let first = crate::bootstrap::bootstrap_with_secret_store_for_test(
        directory.path().to_path_buf(),
        &secrets,
    );
    let first_state = first.state;
    assert!(
        first_state
            .persistence_coordinator
            .startup_reconciliation_mutations_safe(),
        "fixture must use durable release stores"
    );
    {
        let mut registry = first_state.mcp_registry.lock().await;
        let mut remote_manifest = registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.id == "builtin_echo")
            .expect("typed echo manifest for remote no-dispatch fixture");
        remote_manifest.source = openlife_core::tool_manifest::ToolSource::Mcp {
            server_name: "durable-live-not-dispatched-remote".into(),
        };
        registry.register_builtin(
            remote_manifest,
            Box::new(|_arguments| {
                anyhow::bail!("remote no-dispatch fixture must never enter an adapter")
            }),
        );
    }
    let (session, failed, run_id) = create_failed_replay_task_for_test(
        &first_state,
        "durable-live-not-dispatched-restart",
        "Use mcp builtin_echo read-only now.",
    )
    .await;
    let (claim, executing) = {
        let queue = first_state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        let claim = queue
            .claim_replay_for_test_fixture(
                &failed.id,
                failed.status,
                failed.revision,
                &uuid::Uuid::new_v4().to_string(),
            )
            .expect("claim replay before simulated crash");
        let retrying = queue
            .transition_claimed_replay(
                &failed.id,
                &claim.claim_id,
                failed.status,
                claim.revision,
                ExecutionQueueStatus::Retrying,
                None,
            )
            .expect("enter retrying before simulated crash");
        let executing = queue
            .transition_claimed_replay(
                &failed.id,
                &claim.claim_id,
                retrying.status,
                retrying.revision,
                ExecutionQueueStatus::Executing,
                None,
            )
            .expect("enter executing before simulated crash");
        (claim, executing)
    };
    let authority = executing
        .replay_authority
        .as_ref()
        .expect("canonical replay authority");
    let registration = openlife_core::tool_execution_receipt::ToolExecutionReceiptRegistration::test_never_dispatched_read(
        Some(run_id.clone()),
        Some(authority.manifest_id().to_string()),
        format!("request-durable-not-dispatched-{}", failed.id),
    );
    let prepared_receipt = registration.snapshot();
    let attempt = openlife_core::agent::ToolDispatchAttempt {
        receipt_id: prepared_receipt.receipt_id.clone(),
        manifest_id: authority.manifest_id().into(),
        tool_name: authority.manifest_name().into(),
        manifest_contract_digest: authority.manifest_contract_digest().into(),
        input_hash: authority.input_hash().into(),
        input_length_bytes: authority.input_length_bytes(),
        source_run_id: Some(run_id.clone()),
        request_digest: prepared_receipt.request_digest.clone(),
        action_effect: authority.action_effect(),
        idempotency_contract: authority.idempotency_contract(),
        process_risk:
            openlife_core::agent::action_executor::ToolDispatchProcessRisk::MayOutliveLocalProcess,
        effect_may_survive_local_process: false,
    };
    let observer = crate::main_chat_event_stream::MainChatToolLifecycleObserver::new(
        std::sync::Arc::clone(&first_state),
        &session.id,
        &run_id,
    )
    .with_replay_claim(&failed.id, &claim.claim_id, claim.owner_generation)
    .expect("bind exact replay claim generation to prepared observer");
    openlife_core::agent::ToolDispatchObserver::before_dispatch(&observer, &attempt)
        .await
        .expect("persist prepared fact before the simulated crash");
    let receipt = registration.settle_after_runtime_failure();
    let closures =
        crate::main_chat_event_stream::append_main_chat_live_not_dispatched_tool_receipts(
            &first_state,
            &session.id,
            &run_id,
            std::slice::from_ref(&receipt),
        )
        .await
        .expect("persist live not-dispatched closure");
    assert_eq!(closures.len(), 1);
    assert_eq!(closures[0].event_type, "tool.not_dispatched");
    {
        let event_store = first_state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await;
        assert_eq!(
            event_store
                .pending_tool_queue_reconciliation_projections(10)
                .expect("durable outbox before restart")
                .items
                .len(),
            1
        );
    }
    let before_restart = first_state
        .main_chat_action_queue_store
        .as_ref()
        .expect("action queue")
        .lock()
        .await
        .load(&failed.id)
        .expect("load pre-restart action")
        .expect("pre-restart action exists");
    assert_eq!(before_restart.status, ExecutionQueueStatus::Executing);
    assert_eq!(
        before_restart.replay_effect_certainty,
        ActionReplayEffectCertainty::EffectNotAttempted
    );

    drop(observer);
    drop(first_state);

    let second = crate::bootstrap::bootstrap_with_secret_store_for_test(
        directory.path().to_path_buf(),
        &secrets,
    );
    let applied_before_ack_state = second.state;
    let projection_before_ack = applied_before_ack_state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store after first restart")
        .lock()
        .await
        .pending_tool_queue_reconciliation_projections(10)
        .expect("load durable projection before first apply")
        .items
        .into_iter()
        .next()
        .expect("projection remains pending before apply");
    {
        let queue = applied_before_ack_state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue after first restart")
            .lock()
            .await;
        crate::bootstrap::apply_tool_queue_reconciliation_projection(
            &queue,
            &projection_before_ack,
        )
        .expect("apply EventStore projection before simulated ack crash");
    }
    let applied_before_ack = applied_before_ack_state
        .main_chat_action_queue_store
        .as_ref()
        .expect("action queue after first apply")
        .lock()
        .await
        .load(&failed.id)
        .expect("load action after first apply")
        .expect("action after first apply exists");
    assert_eq!(applied_before_ack.status, ExecutionQueueStatus::Executing);
    assert_eq!(
        applied_before_ack.replay_effect_certainty,
        ActionReplayEffectCertainty::EffectNotAttempted
    );
    let applied_before_ack_revision = applied_before_ack.revision;
    assert_eq!(
        applied_before_ack_state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store before simulated ack crash")
            .lock()
            .await
            .pending_tool_queue_reconciliation_projections(10)
            .expect("projection remains pending without ack")
            .items
            .len(),
        1
    );
    drop(applied_before_ack_state);

    let third = crate::bootstrap::bootstrap_with_secret_store_for_test(
        directory.path().to_path_buf(),
        &secrets,
    );
    let restarted_state = third.state;
    let projection_after_ack_crash = restarted_state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store after ack crash")
        .lock()
        .await
        .pending_tool_queue_reconciliation_projections(10)
        .expect("reload projection after ack crash")
        .items
        .into_iter()
        .next()
        .expect("unacknowledged projection survives restart");
    {
        let queue = restarted_state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue after ack crash")
            .lock()
            .await;
        crate::bootstrap::apply_tool_queue_reconciliation_projection(
            &queue,
            &projection_after_ack_crash,
        )
        .expect("reapplying the exact projection is idempotent");
        let reapplied = queue
            .load(&failed.id)
            .expect("load reapplied action")
            .expect("reapplied action exists");
        assert_eq!(reapplied.revision, applied_before_ack_revision);
        assert_eq!(
            reapplied.replay_effect_certainty,
            ActionReplayEffectCertainty::EffectNotAttempted
        );
    }
    restarted_state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store after exact reapply")
        .lock()
        .await
        .mark_tool_queue_reconciliation_projection_applied(&projection_after_ack_crash)
        .expect("acknowledge the exact reapplied projection");
    crate::bootstrap::reconcile_startup_orphaned_main_chat_runs(&restarted_state)
        .await
        .expect("safe claim recovery follows the acknowledged projection");
    let recovered = restarted_state
        .main_chat_action_queue_store
        .as_ref()
        .expect("restarted action queue")
        .lock()
        .await
        .load(&failed.id)
        .expect("load recovered action")
        .expect("recovered action exists");
    assert_eq!(recovered.status, ExecutionQueueStatus::Failed);
    assert_eq!(recovered.replay_claim, ActionReplayClaimState::Unclaimed);
    assert_eq!(
        recovered.replay_effect_certainty,
        ActionReplayEffectCertainty::FailedBeforeDispatch
    );
    assert!(recovered.replay_dispatch_started_at.is_none());
    let recovered_revision = recovered.revision;
    {
        let event_store = restarted_state
            .main_chat_agent_event_store
            .as_ref()
            .expect("restarted event store")
            .lock()
            .await;
        assert!(event_store
            .pending_tool_queue_reconciliation_projections(10)
            .expect("outbox acknowledged after recovery")
            .items
            .is_empty());
        let receipt_events = event_store
            .list(&session.id, 0, 100)
            .expect("list durable restart events")
            .into_iter()
            .filter(|event| event.object_id == receipt.receipt_id)
            .collect::<Vec<_>>();
        assert_eq!(
            receipt_events
                .iter()
                .filter(|event| event.event_type == "tool.dispatch_prepared")
                .count(),
            1
        );
        assert_eq!(
            receipt_events
                .iter()
                .filter(|event| event.event_type == "tool.not_dispatched")
                .count(),
            1
        );
        assert!(receipt_events.iter().all(|event| !matches!(
            event.event_type.as_str(),
            "tool.dispatch_ambiguous" | "tool.remote_unknown" | "tool.effect_unknown"
        )));
    }

    crate::bootstrap::reconcile_startup_orphaned_main_chat_runs(&restarted_state)
        .await
        .expect("repeated startup reconciliation is idempotent");
    let after_second_pass = restarted_state
        .main_chat_action_queue_store
        .as_ref()
        .expect("restarted action queue")
        .lock()
        .await
        .load(&failed.id)
        .expect("reload recovered action")
        .expect("recovered action still exists");
    assert_eq!(after_second_pass.revision, recovered_revision);
    assert_eq!(
        after_second_pass.replay_effect_certainty,
        ActionReplayEffectCertainty::FailedBeforeDispatch
    );
}

#[tokio::test]
async fn multi_action_retry_preserves_remaining_failure_and_backend_target_until_all_complete() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionStatus, ExecutionAction, ExecutionPolicy, ExecutionQueueStatus,
    };
    use openlife_core::agent::AgentRunStatus;
    use openlife_core::tool_permissions::ToolPermissionPolicy;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock app");
    let (session, first_failed, run_id) = create_failed_replay_task_for_test(
        &state,
        "multi-action-retry",
        "Use mcp builtin_echo read-only now.",
    )
    .await;
    let second_queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        let action = ExecutionAction::new("mcp.read_only", "Replay second governed MCP read.");
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue second failed action");
        queued
    };
    let second_envelope =
        replay_execution_envelope_for_test(&state, &session, &second_queued, &run_id).await;
    let second_failed = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        project_test_read_receipt(
            &queue,
            &second_queued,
            openlife_core::agent::ActionExecutionStatus::Failed,
            metadata_with_replay_envelope(
                serde_json::json!({"directWritesExecuted": false}),
                &second_envelope,
            ),
            Some("second fixture failed before dispatch"),
        )
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &second_failed.id)
            .expect("link second failed action");
        store
            .set_pending_blockers(&session.id, vec!["two_actions_failed".into()])
            .expect("seed aggregate blocker");
    }
    let manifest = state
        .mcp_registry
        .lock()
        .await
        .list_manifests()
        .into_iter()
        .find(|manifest| manifest.name == "builtin_echo")
        .expect("builtin echo manifest");
    state
        .tool_permission_store
        .lock()
        .await
        .grant(
            &manifest.name,
            &openlife_core::agent::action_executor::helpers::canonical_tool_source(&manifest),
            &manifest.risk_level,
            &manifest.action_type,
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .expect("grant replay permission");

    let stale_error = crate::main_chat_task_controls::retry_main_chat_agent_action(
        session.id.clone(),
        first_failed.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect_err("caller cannot choose a stale non-projected retry target");
    assert!(stale_error.contains("action_not_current_backend_retry_target"));

    crate::main_chat_task_controls::retry_main_chat_agent_action(
        session.id.clone(),
        second_failed.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("retry current backend target");

    let (first_after, second_after) = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        (
            queue.load(&first_failed.id).unwrap().unwrap(),
            queue.load(&second_failed.id).unwrap().unwrap(),
        )
    };
    assert_eq!(first_after.status, ExecutionQueueStatus::Failed);
    assert_eq!(second_after.status, ExecutionQueueStatus::Completed);
    let detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &session.id,
        &state,
    )
    .await
    .expect("load aggregate task detail");
    assert_eq!(detail.task_session.status, AgentTaskSessionStatus::Failed);
    assert_eq!(
        detail.retry_target_action_id.as_deref(),
        Some(first_failed.id.as_str())
    );
    assert!(detail
        .task_session
        .pending_blockers
        .iter()
        .any(|blocker| blocker == &format!("action:{}:failed", first_failed.id)));
    let run = state
        .agent_run_store
        .as_ref()
        .expect("run store")
        .lock()
        .await
        .get_run(&run_id)
        .unwrap()
        .unwrap();
    assert_eq!(run.status, AgentRunStatus::Failed);

    crate::main_chat_task_controls::retry_main_chat_agent_action(
        session.id.clone(),
        first_failed.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("retry final remaining action");
    let final_detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &session.id,
        &state,
    )
    .await
    .expect("load completed aggregate detail");
    assert_eq!(
        final_detail.task_session.status,
        AgentTaskSessionStatus::Completed
    );
    assert!(final_detail.task_session.pending_blockers.is_empty());
    assert!(final_detail.retry_target_action_id.is_none());
}

#[tokio::test]
async fn cancel_before_retry_registration_prevents_claim_and_tool_dispatch() {
    use openlife_core::agent::main_chat_agent_v1::{
        ActionReplayClaimState, ActionReplayEffectCertainty, AgentTaskSessionDraft,
        ExecutionAction, ExecutionPolicy, ExecutionQueueStatus, MainChatAgentStrategy,
    };

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "retry-cancel-before-registration".into(),
                user_goal: "Use mcp builtin_echo read-only now.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create replay task")
    };
    let run_id = create_task_bound_agent_run_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        &session.user_goal,
    )
    .await;
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        let action = ExecutionAction::new("mcp.read_only", "Cancelled replay must not dispatch.");
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue replay action");
        queued
    };
    let envelope = replay_execution_envelope_for_test(&state, &session, &queued, &run_id).await;
    let failed_action = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        project_test_read_receipt(
            &queue,
            &queued,
            openlife_core::agent::ActionExecutionStatus::Failed,
            metadata_with_replay_envelope(
                serde_json::json!({"directWritesExecuted": false}),
                &envelope,
            ),
            Some("fixture failed before dispatch"),
        )
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &failed_action.id)
            .expect("link action to task");
        store
            .fail_session(&session.id, "Fixture action failed before dispatch.")
            .expect("mark task failed");
    }
    crate::main_chat_event_stream::append_main_chat_agent_runtime_event_batch(
        &state,
        &session.id,
        &run_id,
        vec![
            crate::main_chat_event_stream::MainChatAgentRuntimeEventInput::new(
                "failed",
                "turn",
                format!("fixture-terminal:{run_id}"),
                "main_chat_task_control_tests",
                serde_json::json!({"status": "failed"}),
            ),
        ],
    )
    .await
    .expect("persist cancellation fixture terminal receipt");

    let cancellation_registry = {
        state
            .main_chat_runtime_state
            .lock()
            .await
            .cancellation_registry
            .clone()
    };
    let cancel_request = cancellation_registry.request_cancel(&session.id);
    assert!(!cancel_request.outcome.active_turn_found);

    let error = crate::main_chat_task_controls::retry_main_chat_agent_action(
        session.id.clone(),
        failed_action.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect_err("pre-registration cancellation must reject retry before claiming");
    assert!(error.contains("main_chat_replay_locally_aborted:before_claim"));

    let persisted = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        queue
            .load(&failed_action.id)
            .expect("load failed action")
            .expect("failed action exists")
    };
    assert_eq!(persisted.status, ExecutionQueueStatus::Cancelled);
    assert_eq!(persisted.replay_claim, ActionReplayClaimState::Unclaimed);
    assert_eq!(
        persisted.replay_effect_certainty,
        ActionReplayEffectCertainty::EffectNotAttempted
    );
    assert!(persisted.replay_dispatch_started_at.is_none());
    assert_eq!(persisted.attempts, 0);
    let lifecycle = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .list(&session.id, 0, 100)
        .expect("list pre-registration cancellation events")
        .into_iter()
        .filter(|event| {
            matches!(
                event.event_type.as_str(),
                "cancel_requested" | "local_aborted"
            )
        })
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert_eq!(lifecycle, vec!["cancel_requested", "local_aborted"]);
}

#[tokio::test]
async fn shipped_task_state_detail_and_refresh_project_transcript_without_internal_authority() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, ExecutionTranscriptEntryDraft, ExecutionTranscriptEntryKind,
        MainChatAgentStrategy,
    };

    const BODY_SENTINEL: &str = "D010_PRIVATE_TASK_TRANSCRIPT_BODY_MUST_NOT_SHIP";
    const AUTHORITY_SENTINEL: &str =
        "hmac-sha256:D010_PRIVATE_TASK_TRANSCRIPT_AUTHORITY_MUST_NOT_SHIP";

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        let session = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "d010-task-transcript-product-boundary".into(),
                user_goal: "Inspect the task transcript product boundary.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create task transcript fixture");
        store
            .append_transcript_entry(ExecutionTranscriptEntryDraft {
                session_id: session.id.clone(),
                kind: ExecutionTranscriptEntryKind::Observation,
                summary: BODY_SENTINEL.into(),
                metadata: serde_json::json!({
                    "authorityTag": AUTHORITY_SENTINEL,
                    "canonicalStoreIdentity": AUTHORITY_SENTINEL,
                    "bindingReceipt": AUTHORITY_SENTINEL,
                    "bodyReceipt": AUTHORITY_SENTINEL,
                    "sourceRef": AUTHORITY_SENTINEL,
                }),
            })
            .expect("append hostile transcript fixture");
        session
    };

    let managed = app.state::<std::sync::Arc<crate::AppState>>();
    let state_payload = get_main_chat_agent_task_state(session.id.clone(), managed)
        .await
        .expect("load shipped task state");
    let detail_payload = get_main_chat_agent_task_detail(
        session.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("load shipped task detail");
    let refreshed_payload = refresh_main_chat_agent_task_context(
        session.id,
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("load shipped refreshed task detail");

    for (surface, payload) in [
        (
            "task_state",
            serde_json::to_value(state_payload).expect("serialize task state"),
        ),
        (
            "task_detail",
            serde_json::to_value(detail_payload).expect("serialize task detail"),
        ),
        (
            "task_refresh",
            serde_json::to_value(refreshed_payload).expect("serialize task refresh"),
        ),
    ] {
        let encoded = serde_json::to_string(&payload).expect("encode shipped task payload");
        assert!(
            !encoded.contains(BODY_SENTINEL)
                && !encoded.contains(AUTHORITY_SENTINEL)
                && !encoded.contains("hmac-sha256:")
                && !encoded.contains("authorityTag")
                && !encoded.contains("canonicalStoreIdentity")
                && !encoded.contains("bindingReceipt")
                && !encoded.contains("bodyReceipt"),
            "{surface} leaked canonical transcript body or keyed authority: {encoded}"
        );
        let transcript = payload["transcript"]
            .as_array()
            .expect("shipped task transcript array");
        assert!(
            !transcript.is_empty(),
            "{surface} must preserve transcript capability"
        );
        assert_eq!(
            transcript[0]["summary"],
            serde_json::json!("observation_state_recorded")
        );
        let mut keys = transcript[0]
            .as_object()
            .expect("product transcript object")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["createdAt", "id", "kind", "sessionId", "summary"],
            "{surface} must expose the exact ProductExecutionTranscriptEntry contract"
        );
    }
}

#[tokio::test]
async fn shipped_task_evidence_subtrees_reject_hostile_body_authority_and_untyped_refs() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, ExecutionAction, ExecutionPolicy, ExecutionTranscriptEntryDraft,
        ExecutionTranscriptEntryKind, MainChatAgentStrategy,
    };
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

    const BODY_SENTINEL: &str = "D010_PRIVATE_TASK_EVIDENCE_BODY_MUST_NOT_SHIP";
    const AUTHORITY_SENTINEL: &str =
        "hmac-sha256:D010_PRIVATE_TASK_EVIDENCE_AUTHORITY_MUST_NOT_SHIP";
    let legal_plan_ref = format!("plan-session-{}", uuid::Uuid::new_v4());
    let legal_context_ref = "mainchat_ctx_1234abcd";

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "d010-hostile-task-evidence-boundary".into(),
                user_goal: format!("{BODY_SENTINEL}:{AUTHORITY_SENTINEL}"),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some(BODY_SENTINEL.into()),
                context_snapshot_refs: vec![
                    BODY_SENTINEL.into(),
                    AUTHORITY_SENTINEL.into(),
                    legal_context_ref.into(),
                ],
            })
            .expect("create hostile evidence fixture")
    };
    let run_id = create_task_bound_agent_run_with_status_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        &session.user_goal,
        openlife_core::agent::AgentRunStatus::Failed,
    )
    .await;

    let mut hostile_proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        "tool_permission.hostile",
        serde_json::json!({"body": BODY_SENTINEL, "authority": AUTHORITY_SENTINEL}),
        BODY_SENTINEL,
        0.5,
        RiskLevel::Medium,
        ProposalSource::ChatConversation,
    );
    hostile_proposal.id = BODY_SENTINEL.into();
    let legal_proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        "tool_permission.legal",
        serde_json::json!({"permission": "allow_once"}),
        "Review the legal fixture proposal.",
        0.8,
        RiskLevel::Medium,
        ProposalSource::ChatConversation,
    );
    let legal_proposal_id = legal_proposal.id.clone();
    {
        let proposal_store = state.proposal_store.as_ref().expect("proposal store");
        let proposal_store = proposal_store.lock().await;
        proposal_store
            .create_proposal(&hostile_proposal)
            .expect("create hostile proposal identity");
        proposal_store
            .create_proposal(&legal_proposal)
            .expect("create legal proposal identity");
    }

    let action = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        let action = ExecutionAction::new("mcp.read_only", "Hostile error evidence fixture.");
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue hostile evidence action");
        project_test_read_receipt(
            &queue,
            &queued,
            openlife_core::agent::ActionExecutionStatus::Failed,
            serde_json::json!({
                "proposalId": BODY_SENTINEL,
                "planExecuteSessionId": BODY_SENTINEL,
                "contextSnapshotRef": AUTHORITY_SENTINEL,
                "directWritesExecuted": false,
            }),
            Some(BODY_SENTINEL),
        )
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &action.id)
            .expect("link hostile action");
        for (proposal_id, plan_ref, context_ref) in [
            (BODY_SENTINEL, BODY_SENTINEL, AUTHORITY_SENTINEL),
            (
                legal_proposal_id.as_str(),
                legal_plan_ref.as_str(),
                legal_context_ref,
            ),
        ] {
            store
                .append_transcript_entry(ExecutionTranscriptEntryDraft {
                    session_id: session.id.clone(),
                    kind: ExecutionTranscriptEntryKind::ProposalRequest,
                    summary: BODY_SENTINEL.into(),
                    metadata: serde_json::json!({
                        "proposalId": proposal_id,
                        "planExecuteSessionId": plan_ref,
                        "contextSnapshotRef": context_ref,
                        "directWritesExecuted": false,
                    }),
                })
                .expect("append proposal and plan reference fixture");
        }
        store
            .append_transcript_entry(ExecutionTranscriptEntryDraft {
                session_id: session.id.clone(),
                kind: ExecutionTranscriptEntryKind::FinalResult,
                summary: BODY_SENTINEL.into(),
                metadata: serde_json::json!({
                    "final_delivery_status": "failed",
                    "planExecuteSessionId": BODY_SENTINEL,
                    "proposalId": BODY_SENTINEL,
                    "directWritesExecuted": false,
                }),
            })
            .expect("append hostile final delivery fixture");
        store
            .set_pending_blockers(
                &session.id,
                vec![
                    BODY_SENTINEL.into(),
                    AUTHORITY_SENTINEL.into(),
                    "tool_permission_required".into(),
                ],
            )
            .expect("set hostile blocker fixture");
        store
            .fail_session(&session.id, BODY_SENTINEL)
            .expect("set hostile final summary fixture");
    }
    crate::main_chat_event_stream::append_main_chat_agent_runtime_event_batch(
        &state,
        &session.id,
        &run_id,
        vec![
            crate::main_chat_event_stream::MainChatAgentRuntimeEventInput::new(
                "failed",
                "turn",
                "d010-hostile-evidence-terminal",
                "main_chat_task_control_tests",
                serde_json::json!({"status": "failed", "failureKind": "tool_error"}),
            ),
        ],
    )
    .await
    .expect("persist legal durable terminal identity");

    let state_payload = get_main_chat_agent_task_state(
        session.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("load task state");
    let detail_payload = get_main_chat_agent_task_detail(
        session.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("load task detail");
    let refresh_payload = refresh_main_chat_agent_task_context(
        session.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("load refreshed task detail");
    let list_payload = list_main_chat_agent_tasks(
        None,
        Some(100),
        Some(0),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("load task list");

    let state_json = serde_json::to_value(state_payload).expect("serialize task state");
    let detail_json = serde_json::to_value(detail_payload).expect("serialize task detail");
    let refresh_json = serde_json::to_value(refresh_payload).expect("serialize task refresh");
    let list_json = serde_json::to_value(list_payload).expect("serialize task list");
    let listed = list_json
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["taskSessionId"] == serde_json::json!(session.id))
        })
        .expect("listed hostile fixture");
    let evidence_subtrees = [
        ("state_transcript", &state_json["transcript"]),
        ("detail_evidence", &detail_json["evidenceView"]),
        ("detail_final", &detail_json["finalDelivery"]),
        ("refresh_evidence", &refresh_json["evidenceView"]),
        ("refresh_final", &refresh_json["finalDelivery"]),
        ("list_evidence", &listed["evidenceView"]),
        ("list_route", &listed["routeEvidence"]),
    ];
    for (surface, subtree) in evidence_subtrees {
        let encoded = serde_json::to_string(subtree).expect("encode evidence subtree");
        assert!(
            !encoded.contains(BODY_SENTINEL)
                && !encoded.contains(AUTHORITY_SENTINEL)
                && !encoded.contains("hmac-sha256:"),
            "{surface} leaked hostile task evidence: {encoded}"
        );
    }

    let evidence = &detail_json["evidenceView"];
    assert!(
        evidence["blockers"]
            .as_array()
            .is_some_and(|items| items.contains(&serde_json::json!("tool_permission_required"))),
        "legal blocker reason code must survive strict projection: {evidence}"
    );
    assert!(
        evidence["proposals"]
            .as_array()
            .is_some_and(|items| items.contains(&serde_json::json!(legal_proposal_id))),
        "legal proposal reference must survive strict projection: {evidence}"
    );
    assert!(
        evidence["planRefs"]
            .as_array()
            .is_some_and(|items| items.contains(&serde_json::json!(legal_plan_ref))),
        "legal plan reference must survive strict projection: {evidence}"
    );
}

#[test]
fn product_route_evidence_projects_typed_minimal_source_refs_and_rejects_hostile_values() {
    const BODY_SENTINEL: &str = "D010_PRIVATE_ROUTE_SOURCE_BODY_MUST_NOT_SHIP";
    const AUTHORITY_SENTINEL: &str =
        "hmac-sha256:D010_PRIVATE_ROUTE_SOURCE_AUTHORITY_MUST_NOT_SHIP";
    let legal_source_ref = uuid::Uuid::new_v4().to_string();
    let route_evidence = serde_json::json!({
        "evidence_id": BODY_SENTINEL,
        "generated_at": "2026-07-13T00:00:00Z",
        "conversation_id": BODY_SENTINEL,
        "run_id": AUTHORITY_SENTINEL,
        "task_session_id": BODY_SENTINEL,
        "answer_scope": "current_turn",
        "planned_route": {
            "provider": "openai",
            "model": "gpt-5",
            "route_type": "cloud",
            "privacy_level": "filtered",
            "reason": "policy_allowed_route",
            "provider_health_is_estimated": false
        },
        "actual_route": null,
        "last_completed_route": null,
        "provider_readiness": {
            "configured": true,
            "credential_present": true,
            "validated": true,
            "validation_status": "validated",
            "preferred": "cloud",
            "actually_used": "openai",
            "stale": false,
            "failed": false,
            "last_checked_at": "2026-07-13T00:00:00Z"
        },
        "fallback": {
            "from_route": null,
            "to_route": null,
            "reason": BODY_SENTINEL,
            "blocker_codes": [BODY_SENTINEL, "provider_unavailable"]
        },
        "external_transmission": "not_sent",
        "source_refs": [
            {
                "source": BODY_SENTINEL,
                "refId": BODY_SENTINEL,
                "status": AUTHORITY_SENTINEL,
                "routeType": BODY_SENTINEL,
                "payload": BODY_SENTINEL
            },
            {
                "source": "provider_validation",
                "refId": legal_source_ref,
                "status": "validated",
                "routeType": "cloud",
                "payload": BODY_SENTINEL
            }
        ],
        "truth_confidence": "verified"
    });
    let mut run = openlife_core::agent::AgentRun::new_chat_run(
        "d010-route-evidence-product-boundary",
        "route evidence fixture",
    );
    run.reasoning_trace = Some(openlife_core::agent::reasoning::ReasoningTrace {
        generation_result: Some(serde_json::json!({
            "runtimeRouteEvidence": route_evidence,
        })),
        ..Default::default()
    });

    let projected =
        crate::main_chat_task_controls::serialized_route_evidence_from_agent_run_for_test(&run)
            .expect("project route evidence");
    let encoded = serde_json::to_string(&projected).expect("encode product route evidence");
    assert!(
        !encoded.contains(BODY_SENTINEL)
            && !encoded.contains(AUTHORITY_SENTINEL)
            && !encoded.contains("hmac-sha256:")
            && !encoded.contains("payload"),
        "ProductRouteEvidence leaked an untyped core source ref: {encoded}"
    );
    assert!(projected["source_refs"].as_array().is_some_and(|refs| {
        refs.iter().any(|reference| {
            reference["source"] == serde_json::json!("provider_validation")
                && reference["ref_id"] == serde_json::json!(legal_source_ref)
                && reference["status"] == serde_json::json!("validated")
                && reference["route_type"] == serde_json::json!("cloud")
        })
    }));
}

#[test]
fn product_final_delivery_identifiers_are_strict_refs_or_unknown() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSession, AgentTaskSessionStatus, ExecutionTranscriptEntry,
        ExecutionTranscriptEntryKind, MainChatAgentStrategy,
    };

    const BODY_SENTINEL: &str = "D010_PRIVATE_FINAL_DELIVERY_ID_MUST_NOT_SHIP";
    const AUTHORITY_SENTINEL: &str =
        "hmac-sha256:D010_PRIVATE_FINAL_DELIVERY_AUTHORITY_MUST_NOT_SHIP";
    let now = chrono::Utc::now();
    let session = AgentTaskSession {
        id: uuid::Uuid::new_v4().to_string(),
        chat_session_id: uuid::Uuid::new_v4().to_string(),
        user_goal: String::new(),
        selected_strategy: MainChatAgentStrategy::DirectAnswer,
        status: AgentTaskSessionStatus::Completed,
        current_plan_summary: None,
        action_queue_ids: vec![],
        pending_blockers: vec![],
        context_snapshot_refs: vec![],
        created_at: now,
        updated_at: now,
        final_summary: None,
    };
    let hostile = vec![
        ExecutionTranscriptEntry {
            id: BODY_SENTINEL.into(),
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::FinalResult,
            summary: "final_result_state_recorded".into(),
            metadata: serde_json::json!({"status": "completed"}),
            created_at: now,
        },
        ExecutionTranscriptEntry {
            id: AUTHORITY_SENTINEL.into(),
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::Observation,
            summary: "observation_state_recorded".into(),
            metadata: serde_json::json!({"finalDeliveryStatus": "completed"}),
            created_at: now,
        },
    ];
    let hostile_projection =
        crate::main_chat_task_controls::final_delivery_from_task_for_test(&session, &hostile)
            .expect("project hostile final delivery");
    let encoded =
        serde_json::to_string(&hostile_projection).expect("encode hostile final delivery");
    assert!(
        !encoded.contains(BODY_SENTINEL)
            && !encoded.contains(AUTHORITY_SENTINEL)
            && !encoded.contains("hmac-sha256:"),
        "final delivery leaked hostile identifiers: {encoded}"
    );
    assert_eq!(
        hostile_projection["transcriptEntryId"],
        serde_json::json!("unknown")
    );
    assert_eq!(
        hostile_projection["deliveryStatusEvidenceId"],
        serde_json::json!("unknown")
    );

    let legal_final_ref = "mainchat_transcript_1234abcd";
    let legal_status_ref = "mainchat_transcript_8765dcba";
    let legal = vec![
        ExecutionTranscriptEntry {
            id: legal_final_ref.into(),
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::FinalResult,
            summary: "final_result_state_recorded".into(),
            metadata: serde_json::json!({"status": "completed"}),
            created_at: now,
        },
        ExecutionTranscriptEntry {
            id: legal_status_ref.into(),
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::Observation,
            summary: "observation_state_recorded".into(),
            metadata: serde_json::json!({"finalDeliveryStatus": "completed"}),
            created_at: now,
        },
    ];
    let legal_projection =
        crate::main_chat_task_controls::final_delivery_from_task_for_test(&session, &legal)
            .expect("project legal final delivery");
    assert_eq!(
        legal_projection["transcriptEntryId"],
        serde_json::json!(legal_final_ref)
    );
    assert_eq!(
        legal_projection["deliveryStatusEvidenceId"],
        serde_json::json!(legal_status_ref)
    );
}

#[test]
fn shipped_task_state_never_serializes_a_hostile_transient_core_transcript() {
    use openlife_core::agent::main_chat_agent_v1::{
        ExecutionTranscriptEntry, ExecutionTranscriptEntryKind,
    };

    const BODY_SENTINEL: &str = "D010_PRIVATE_TRANSIENT_TASK_BODY_MUST_NOT_SHIP";
    const AUTHORITY_SENTINEL: &str =
        "hmac-sha256:D010_PRIVATE_TRANSIENT_TASK_AUTHORITY_MUST_NOT_SHIP";
    let payload = MainChatAgentTaskState {
        session: None,
        actions: vec![],
        transcript: crate::product_agent_dto::project_execution_transcript(vec![
            ExecutionTranscriptEntry {
                id: "transient-hostile-entry".into(),
                session_id: "transient-hostile-session".into(),
                kind: ExecutionTranscriptEntryKind::Observation,
                summary: BODY_SENTINEL.into(),
                metadata: serde_json::json!({
                    "authorityTag": AUTHORITY_SENTINEL,
                    "canonicalStoreIdentity": AUTHORITY_SENTINEL,
                    "bindingReceipt": AUTHORITY_SENTINEL,
                    "bodyReceipt": AUTHORITY_SENTINEL,
                }),
                created_at: chrono::Utc::now(),
            },
        ]),
        pending_approval_count: 0,
        active_tool_count: 0,
        can_resume: false,
        can_cancel: false,
        can_retry: false,
        cancellation_pending: false,
    };

    let encoded = serde_json::to_string(&payload).expect("serialize transient task state");
    assert!(
        !encoded.contains(BODY_SENTINEL)
            && !encoded.contains(AUTHORITY_SENTINEL)
            && !encoded.contains("metadata"),
        "transient task transcript crossed the shipped boundary without Product projection: {encoded}"
    );
    let value = serde_json::to_value(payload).expect("serialize transient task state value");
    assert_eq!(value["transcript"][0]["id"], serde_json::json!("unknown"));
    assert_eq!(
        value["transcript"][0]["sessionId"],
        serde_json::json!("unknown")
    );
}

#[tokio::test]
async fn main_chat_task_continuity_list_detail_and_refresh_are_evidence_backed() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, ExecutionAction, ExecutionPolicy,
        ExecutionQueueStatus, ExecutionTranscriptEntryDraft, ExecutionTranscriptEntryKind,
        MainChatAgentStrategy,
    };

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");

    let blocked = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "continuity-list-chat".into(),
                user_goal: "Read the continuity fixture and wait for review.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some("Read before continuing.".into()),
                context_snapshot_refs: vec!["continuity-context:v1".into()],
            })
            .expect("create blocked task")
    };
    let (unsafe_action, action) = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        let unsafe_execution_action =
            ExecutionAction::new("file.read", "Legacy failure without a typed receipt.");
        let unsafe_queued = queue
            .enqueue(
                &blocked.id,
                unsafe_execution_action.clone(),
                ExecutionPolicy.classify(&unsafe_execution_action),
            )
            .expect("enqueue unsafe read action");
        let unsafe_failed = queue
            .fail(
                &unsafe_queued.id,
                "legacy failure without typed no-dispatch evidence",
                Some(serde_json::json!({"directWritesExecuted": false})),
            )
            .expect("fail unsafe read action");
        let execution_action = ExecutionAction::new("file.read", "Read a safe workspace file.");
        let queued = queue
            .enqueue(
                &blocked.id,
                execution_action.clone(),
                ExecutionPolicy.classify(&execution_action),
            )
            .expect("enqueue read action");
        let safe_failed = project_test_read_receipt(
            &queue,
            &queued,
            openlife_core::agent::ActionExecutionStatus::Failed,
            serde_json::json!({
                "target": "AGENTS.md",
                "directWritesExecuted": false,
            }),
            Some("safe read failed before dispatch"),
        );
        (unsafe_failed, safe_failed)
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .record_action_queue_id(&blocked.id, &unsafe_action.id)
            .expect("record unsafe action id");
        store
            .record_action_queue_id(&blocked.id, &action.id)
            .expect("record action id");
        store
            .append_transcript_entry(ExecutionTranscriptEntryDraft {
                session_id: blocked.id.clone(),
                kind: ExecutionTranscriptEntryKind::Observation,
                summary: "Last observation came from the action queue evidence.".into(),
                metadata: serde_json::json!({
                    "actionId": action.id,
                    "contextSnapshotRef": "continuity-context:v1",
                    "directWritesExecuted": false,
                }),
            })
            .expect("append observation");
        store
            .block_session(
                &blocked.id,
                "The failed read requires an exact context refresh before retry.",
            )
            .expect("block task");
    }
    create_task_bound_agent_run_with_status_for_test(
        &state,
        &blocked.id,
        &blocked.chat_session_id,
        &blocked.user_goal,
        openlife_core::agent::AgentRunStatus::WaitingPermission,
    )
    .await;

    let completed = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        let completed = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "continuity-list-chat".into(),
                user_goal: "Already completed continuity task.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create completed task");
        store
            .complete_session(&completed.id, "Done.")
            .expect("complete task")
    };
    create_task_bound_agent_run_for_test(
        &state,
        &completed.id,
        &completed.chat_session_id,
        &completed.user_goal,
    )
    .await;

    let managed_state = app.state::<std::sync::Arc<crate::AppState>>();
    let summaries = list_main_chat_agent_tasks(None, Some(10), Some(0), managed_state)
        .await
        .expect("list continuity tasks");
    assert!(
        summaries
            .iter()
            .any(|summary| summary.task_session_id == blocked.id
                && summary.status == AgentTaskSessionStatus::Blocked
                && summary.last_observation_preview == "observation_state_recorded"
                && summary.next_recommended_control == "refresh_context"
                && summary.pending_blocker_count > 0
                && summary.resume_safety_digest.starts_with("bytes:")),
        "blocked task summary should be evidence-backed: {summaries:?}"
    );
    assert!(
        summaries
            .iter()
            .any(|summary| summary.task_session_id == completed.id
                && summary.status == AgentTaskSessionStatus::Completed
                && summary.next_recommended_control == "wait_for_projection_reconciliation"),
        "completed task summary should remain discoverable: {summaries:?}"
    );

    let blocked_only = list_main_chat_agent_tasks(
        Some(MainChatAgentTaskFilter {
            statuses: vec![AgentTaskSessionStatus::Blocked],
            conversation_id: None,
            include_terminal: true,
            include_stale: true,
        }),
        Some(10),
        Some(0),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("filter blocked tasks");
    assert_eq!(
        blocked_only
            .iter()
            .map(|summary| summary.task_session_id.as_str())
            .collect::<Vec<_>>(),
        vec![blocked.id.as_str()]
    );

    let detail = get_main_chat_agent_task_detail(
        blocked.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("get blocked detail");
    assert_eq!(detail.task_session.id, blocked.id);
    assert_eq!(detail.actions.len(), 2);
    assert_eq!(detail.transcript.len(), 1);
    assert!(
        !detail.blockers.is_empty()
            && detail
                .blockers
                .iter()
                .all(|blocker| blocker.starts_with("action_failed:")),
        "product blockers must use action references, not final-summary or action-error bodies: {:?}",
        detail.blockers
    );
    assert!(!detail.allowed_controls.contains(&"retry".to_string()));
    assert!(detail
        .allowed_controls
        .contains(&"refresh_context".to_string()));
    assert!(detail.allowed_controls.contains(&"cancel".to_string()));
    assert!(!detail.allowed_controls.contains(&"resume".to_string()));
    assert!(detail.last_safe_resume_point.is_none());
    assert!(
        detail.retry_target_action_id.is_none(),
        "a typed receipt without a durable execution envelope is not a retry target"
    );
    assert_ne!(
        detail.retry_target_action_id.as_deref(),
        Some(unsafe_action.id.as_str()),
        "an earlier failed row without a typed retry-safe receipt must not become the UI target"
    );
    assert!(!detail.continuity_diagnostics.missing_action_evidence);

    let tasks_view = crate::read_models::tasks::get_tasks_view_model_with_state(&state)
        .await
        .expect("build backend TasksViewModel");
    let task_item = tasks_view
        .data
        .as_ref()
        .expect("TasksViewModel data")
        .items
        .iter()
        .find(|item| item.task_session_id.as_deref() == Some(blocked.id.as_str()))
        .expect("projected task item");
    assert!(
        task_item
            .allowed_controls
            .iter()
            .all(|control| !control.id.ends_with(":retry")),
        "TasksViewModel must not invent a retry control without a durable execution envelope"
    );

    let refreshed = refresh_main_chat_agent_task_context(
        blocked.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("refresh context");
    assert_eq!(refreshed.task_session.id, blocked.id);
    assert!(!refreshed.allowed_controls.contains(&"retry".to_string()));
    assert!(refreshed.retry_target_action_id.is_none());
    let action_after_refresh = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        queue
            .load(&action.id)
            .expect("load action")
            .expect("action exists")
    };
    assert_eq!(
        action_after_refresh.status,
        ExecutionQueueStatus::Failed,
        "refresh must not automatically replay failed actions"
    );
}

#[tokio::test]
async fn dispatched_unknown_failed_action_is_not_advertised_as_automatically_replayable() {
    use openlife_core::agent::main_chat_agent_v1::{
        ActionReplayEffectCertainty, AgentTaskSessionDraft, ExecutionAction, ExecutionPolicy,
        ExecutionQueueStatus, MainChatAgentStrategy,
    };

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "unknown-effect-continuity-chat".into(),
                user_goal: "Read one resource without duplicating an uncertain effect.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create unknown-effect task")
    };
    let failed = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        let action = ExecutionAction::new("file.read", "Read one governed resource.");
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue action");
        let initially_failed = project_test_read_receipt(
            &queue,
            &queued,
            openlife_core::agent::ActionExecutionStatus::Failed,
            serde_json::json!({"directWritesExecuted": false}),
            Some("pre-dispatch setup failure"),
        );
        let replay_execution_id = uuid::Uuid::new_v4().to_string();
        let claim = queue
            .claim_replay_for_test_fixture(
                &queued.id,
                initially_failed.status,
                initially_failed.revision,
                &replay_execution_id,
            )
            .expect("claim replay");
        let retrying = queue
            .transition_claimed_replay(
                &queued.id,
                &claim.claim_id,
                initially_failed.status,
                claim.revision,
                ExecutionQueueStatus::Retrying,
                None,
            )
            .expect("enter retrying");
        let executing = queue
            .transition_claimed_replay(
                &queued.id,
                &claim.claim_id,
                retrying.status,
                retrying.revision,
                ExecutionQueueStatus::Executing,
                None,
            )
            .expect("enter executing");
        let fenced = queue
            .fence_replay_dispatch_commit(
                &queued.id,
                &claim.claim_id,
                claim.owner_generation,
                executing.revision,
            )
            .expect("persist replay pre-edge dispatch fence");
        let dispatched = queue
            .record_replay_dispatch_started(&queued.id, &claim.claim_id, fenced.revision)
            .expect("record physical dispatch boundary");
        queue
            .fail_claimed_replay(
                &queued.id,
                &claim.claim_id,
                dispatched.status,
                dispatched.revision,
                "remote result unknown",
                Some(serde_json::json!({"retryReplayable": true})),
            )
            .expect("persist unknown effect")
    };
    assert_eq!(
        failed.replay_effect_certainty,
        ActionReplayEffectCertainty::DispatchedUnknown
    );
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &failed.id)
            .expect("bind action");
        store
            .block_session(&session.id, "Remote effect is unknown.")
            .expect("block task");
    }

    let detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &session.id,
        &state,
    )
    .await
    .expect("load task detail");
    assert!(!detail.continuity_diagnostics.automatic_replay_allowed);
    assert!(!detail
        .allowed_controls
        .iter()
        .any(|control| control == "retry"));
    assert_ne!(detail.next_recommended_control, "retry");
}

#[tokio::test]
async fn cancelling_an_unknown_task_does_not_leave_a_pre_registration_tombstone() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let registry = {
        state
            .main_chat_runtime_state
            .lock()
            .await
            .cancellation_registry
            .clone()
    };

    let error = crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(
        "unknown-task-id",
        &state,
    )
    .await
    .unwrap_err();
    assert!(error.contains("Main Chat task not found"));

    let registration = registry.register("unknown-task-id");
    assert!(
        !registration.token.is_cancelled(),
        "an invalid cancellation request must not poison a future task registration"
    );
}

#[tokio::test]
async fn degraded_action_queue_cancellation_recovers_from_durable_projection_delivery() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, ExecutionAction, ExecutionPolicy, ExecutionQueueStatus,
        MainChatAgentStrategy,
    };
    use openlife_core::agent::AgentRunStatus;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "cancel-projection-recovery".into(),
                user_goal: "Cancel the queued replay projection.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create cancellation projection task")
    };
    let run_id = create_task_bound_agent_run_with_status_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        &session.user_goal,
        AgentRunStatus::Running,
    )
    .await;
    let action_id = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        let action = ExecutionAction::new("mcp.read_only", "Queued cancellation projection.");
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue cancellation projection action");
        queue
            .install_cancel_session_failure_for_test()
            .expect("install action projection fault");
        queued.id
    };

    let error =
        crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(&session.id, &state)
            .await
            .expect_err("action queue projection failure must be reported as degraded");
    assert!(error.contains("cancellation_projection_degraded"));
    let events = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .list(&session.id, 0, 100)
        .expect("list durable cancellation events");
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.event_type.as_str(),
                "cancel_requested" | "local_aborted"
            ))
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>(),
        vec!["cancel_requested", "local_aborted"]
    );
    assert!(events.iter().all(|event| event.run_id == run_id));
    let before_recovery = state
        .main_chat_action_queue_store
        .as_ref()
        .expect("action queue")
        .lock()
        .await
        .load(&action_id)
        .expect("load stale action")
        .expect("stale action exists");
    assert_eq!(before_recovery.status, ExecutionQueueStatus::Planned);
    {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        queue
            .remove_cancel_session_failure_for_test()
            .expect("remove action projection fault");
    }
    let applied = crate::main_chat_task_controls::reconcile_main_chat_cancellation_projections(
        &state,
        Some(&session.id),
    )
    .await
    .expect("reconcile degraded cancellation projection");
    assert_eq!(applied, 1);
    let recovered = state
        .main_chat_action_queue_store
        .as_ref()
        .expect("action queue")
        .lock()
        .await
        .load(&action_id)
        .expect("load recovered action")
        .expect("recovered action exists");
    assert_eq!(recovered.status, ExecutionQueueStatus::Cancelled);
    let pending = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .list_cancellation_projection_deliveries(Some(&session.id), 10)
        .expect("list remaining projections");
    assert!(pending.is_empty());
}

#[tokio::test]
async fn cancellation_at_replay_commit_barrier_prevents_late_completion() {
    use openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus;
    use openlife_core::agent::AgentRunStatus;
    use openlife_core::tool_permissions::ToolPermissionPolicy;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock app");
    let (session, failed, run_id) = create_failed_replay_task_for_test(
        &state,
        "replay-commit-barrier",
        "Use mcp builtin_echo read-only now.",
    )
    .await;
    let manifest = state
        .mcp_registry
        .lock()
        .await
        .list_manifests()
        .into_iter()
        .find(|manifest| manifest.name == "builtin_echo")
        .expect("builtin echo manifest");
    state
        .tool_permission_store
        .lock()
        .await
        .grant(
            &manifest.name,
            &openlife_core::agent::action_executor::helpers::canonical_tool_source(&manifest),
            &manifest.risk_level,
            &manifest.action_type,
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .expect("grant builtin replay permission");
    let (_barrier_guard, reached, release) =
        crate::main_chat_task_controls::install_main_chat_replay_commit_barrier_for_test(
            &session.id,
        );
    let retry = crate::main_chat_task_controls::retry_main_chat_agent_action(
        session.id.clone(),
        failed.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    );
    tokio::pin!(retry);
    tokio::select! {
        result = tokio::time::timeout(std::time::Duration::from_secs(2), reached.wait()) => {
            result.expect("replay must reach the pre-commit barrier");
        }
        result = &mut retry => panic!("replay exited before commit barrier: {result:?}"),
    }

    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(&session.id, &state),
    )
    .await
    .expect("cancel command must not wait for the replay commit barrier")
    .expect("request cancellation at commit barrier");
    // Release is one-way because cancellation may already have dropped the
    // paused replay future. Requiring a second barrier participant here would
    // test Tokio Barrier cancellation behavior instead of OpenLife's commit
    // fence.
    release.add_permits(1);
    let retry_result = tokio::time::timeout(std::time::Duration::from_secs(2), &mut retry)
        .await
        .expect("replay must stop promptly after commit barrier cancellation");
    let error = retry_result.expect_err("cancel must win before replay projection commits");
    assert!(
        error.contains("main_chat_replay_locally_aborted"),
        "unexpected replay cancellation error: {error}"
    );

    let action = state
        .main_chat_action_queue_store
        .as_ref()
        .expect("action queue")
        .lock()
        .await
        .load(&failed.id)
        .expect("load commit barrier action")
        .expect("commit barrier action exists");
    assert_eq!(action.status, ExecutionQueueStatus::Cancelled);
    assert_ne!(action.status, ExecutionQueueStatus::Completed);
    let run = state
        .agent_run_store
        .as_ref()
        .expect("run store")
        .lock()
        .await
        .get_run(&run_id)
        .expect("load commit barrier run")
        .expect("commit barrier run exists");
    assert_eq!(run.status, AgentRunStatus::Cancelled);
    let events = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .list(&session.id, 0, 100)
        .expect("list commit barrier events");
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "tool.started")
            .count(),
        1,
        "one ToolGateway receipt must have one durable start fact"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "tool.completed")
            .count(),
        1,
        "cancellation finalization must idempotently reuse the terminal receipt"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.event_type.as_str(),
                "cancel_requested" | "local_aborted"
            ))
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>(),
        vec!["cancel_requested", "local_aborted"]
    );
}

#[tokio::test]
async fn hanging_real_mcp_replay_aborts_promptly_and_never_late_commits() {
    use openlife_core::agent::main_chat_agent_v1::{
        ActionReplayEffectCertainty, ExecutionQueueStatus,
    };
    use openlife_core::tool_manifest::{ToolIdempotencyContract, ToolManifest, ToolSource};
    use openlife_core::tool_permissions::ToolPermissionPolicy;
    use std::collections::HashMap;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock app");
    let marker_dir = tempfile::tempdir().expect("create MCP marker dir");
    let marker_path = marker_dir.path().join("call-dispatched.marker");
    let release_path = marker_dir.path().join("release-late-response.marker");
    let script = r#"
import json, os, sys, time
for line in sys.stdin:
    message = json.loads(line)
    method = message.get('method')
    if method == 'initialize':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'protocolVersion':'2024-11-05','capabilities':{}}}), flush=True)
    elif method == 'tools/list':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'tools':[{'name':'hang_tool','description':'bounded hang test','parameters':{'type':'object','properties':{}}}]}}), flush=True)
    elif method == 'tools/call':
        with open(os.environ['OPENLIFE_MCP_DISPATCH_MARKER'], 'w', encoding='utf-8') as marker:
            marker.write('dispatched')
            marker.flush()
            os.fsync(marker.fileno())
        while not os.path.exists(os.environ['OPENLIFE_MCP_RELEASE_MARKER']):
            time.sleep(0.005)
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'content':[{'type':'text','text':'late response'}],'isError':False}}), flush=True)
"#;
    let manifest = ToolManifest {
        id: "mcp:hanging-replay:hang_tool".into(),
        name: "hang_tool".into(),
        description: "Read through a deliberately hanging MCP fixture.".into(),
        parameters: serde_json::json!({"type":"object","properties":{}}),
        permission_level: "low".into(),
        risk_level: "low".into(),
        version: "1.0.0".into(),
        source: ToolSource::Mcp {
            server_name: "hanging-replay".into(),
        },
        capabilities: vec!["read".into()],
        requires_confirmation: false,
        enabled: true,
        declarative_only: false,
        action_type: "read".into(),
        idempotency_contract: ToolIdempotencyContract::Idempotent,
        tags: vec!["typed_contract".into(), "test".into()],
    };
    let mut env = HashMap::new();
    env.insert(
        "OPENLIFE_MCP_DISPATCH_MARKER".to_string(),
        marker_path.to_string_lossy().to_string(),
    );
    env.insert(
        "OPENLIFE_MCP_RELEASE_MARKER".to_string(),
        release_path.to_string_lossy().to_string(),
    );
    let args = ["-u", "-c", script];
    let prepared = openlife_core::mcp::McpRegistry::prepare_registration(
        "hanging-replay",
        "python3",
        &args,
        &env,
        vec![manifest.clone()],
    )
    .await
    .expect("prepare real hanging MCP registration");
    state
        .mcp_registry
        .lock()
        .await
        .commit_prepared_registration(prepared)
        .expect("commit hanging MCP registration");
    state
        .tool_permission_store
        .lock()
        .await
        .grant(
            &manifest.name,
            &openlife_core::agent::action_executor::helpers::canonical_tool_source(&manifest),
            &manifest.risk_level,
            &manifest.action_type,
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .expect("grant hanging MCP read permission");
    let (session, failed, run_id) = create_failed_replay_task_for_test(
        &state,
        "hanging-real-mcp-replay",
        "Use mcp hang_tool read-only now.",
    )
    .await;

    let retry = crate::main_chat_task_controls::retry_main_chat_agent_action(
        session.id.clone(),
        failed.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    );
    tokio::pin!(retry);
    let wait_for_dispatch = async {
        loop {
            if marker_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    };
    tokio::select! {
        _ = tokio::time::timeout(std::time::Duration::from_secs(2), wait_for_dispatch) => {}
        result = &mut retry => panic!("hanging MCP replay exited before dispatch: {result:?}"),
    }
    assert!(
        marker_path.exists(),
        "real MCP server must observe tools/call"
    );
    let lock_probe_timeout = std::time::Duration::from_millis(500);
    tokio::time::timeout(lock_probe_timeout, async {
        state.mcp_registry.lock().await.list_manifests()
    })
    .await
    .expect("hanging MCP transport must not retain the MCP registry guard");
    tokio::time::timeout(lock_probe_timeout, async {
        state.tool_permission_store.lock().await.list()
    })
    .await
    .expect("hanging MCP transport must not retain the permission-store guard")
    .expect("read permission store while MCP call is in flight");
    tokio::time::timeout(lock_probe_timeout, async {
        state.mcp_audit_store.lock().await.list_logs(1)
    })
    .await
    .expect("hanging MCP transport must not retain the audit-store guard")
    .expect("read audit store while MCP call is in flight");
    tokio::time::timeout(lock_probe_timeout, async {
        state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await
            .load_session(&session.id)
    })
    .await
    .expect("hanging MCP transport must not retain the task-session guard")
    .expect("read task session while MCP call is in flight")
    .expect("hanging replay task exists");
    tokio::time::timeout(lock_probe_timeout, async {
        state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await
            .load(&failed.id)
    })
    .await
    .expect("hanging MCP transport must not retain the action-queue guard")
    .expect("read action queue while MCP call is in flight")
    .expect("hanging replay action exists");
    let cancel_started = std::time::Instant::now();
    crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(&session.id, &state)
        .await
        .expect("request hanging MCP cancellation");
    let error = tokio::time::timeout(std::time::Duration::from_secs(1), &mut retry)
        .await
        .expect("hanging replay must stop locally within one second")
        .expect_err("hanging replay cancellation cannot report success");
    assert!(cancel_started.elapsed() < std::time::Duration::from_secs(1));
    assert_eq!(
        error, "main_chat_replay_locally_aborted:during_tool_execution",
        "hanging-MCP cancellation reason is a stable contract"
    );
    let cancelled = state
        .main_chat_action_queue_store
        .as_ref()
        .expect("action queue")
        .lock()
        .await
        .load(&failed.id)
        .expect("load hanging replay action")
        .expect("hanging replay action exists");
    assert_eq!(cancelled.status, ExecutionQueueStatus::Cancelled);
    assert_eq!(
        cancelled.replay_effect_certainty,
        ActionReplayEffectCertainty::DispatchedUnknown
    );
    let cancelled_revision = cancelled.revision;
    let cancelled_run = state
        .agent_run_store
        .as_ref()
        .expect("AgentRun store")
        .lock()
        .await
        .get_run(&run_id)
        .expect("load cancelled hanging replay run")
        .expect("cancelled hanging replay run exists");
    let cancelled_session = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("task session store")
        .lock()
        .await
        .load_session(&session.id)
        .expect("load cancelled hanging replay task")
        .expect("cancelled hanging replay task exists");
    assert_eq!(
        cancelled_run.status,
        openlife_core::agent::AgentRunStatus::Cancelled
    );
    assert_eq!(
        cancelled_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Cancelled
    );
    std::fs::write(&release_path, b"release").expect("release the late MCP response");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let still_cancelled = state
        .main_chat_action_queue_store
        .as_ref()
        .expect("action queue")
        .lock()
        .await
        .load(&failed.id)
        .expect("reload hanging replay action")
        .expect("hanging replay action exists");
    assert_eq!(still_cancelled.status, ExecutionQueueStatus::Cancelled);
    assert_eq!(
        still_cancelled.revision, cancelled_revision,
        "a late MCP response must not mutate the cancelled ActionQueue fact"
    );
    assert_eq!(
        still_cancelled.replay_effect_certainty,
        ActionReplayEffectCertainty::DispatchedUnknown
    );
    let run_after_late_response = state
        .agent_run_store
        .as_ref()
        .expect("AgentRun store")
        .lock()
        .await
        .get_run(&run_id)
        .expect("reload cancelled hanging replay run")
        .expect("cancelled hanging replay run still exists");
    assert_eq!(run_after_late_response.status, cancelled_run.status);
    assert_eq!(
        run_after_late_response.finished_at, cancelled_run.finished_at,
        "a late MCP response must not rewrite the AgentRun terminal"
    );
    let session_after_late_response = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("task session store")
        .lock()
        .await
        .load_session(&session.id)
        .expect("reload cancelled hanging replay task")
        .expect("cancelled hanging replay task still exists");
    assert_eq!(session_after_late_response.status, cancelled_session.status);
    assert_eq!(
        session_after_late_response.updated_at, cancelled_session.updated_at,
        "a late MCP response must not rewrite the task-session terminal"
    );
    let events = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .list(&session.id, 0, 100)
        .expect("list hanging replay events");
    assert!(events
        .iter()
        .any(|event| event.event_type == "tool.started"));
    assert!(events
        .iter()
        .any(|event| event.event_type == "tool.remote_unknown"
            && event.payload.get("remoteCancellationConfirmed").is_none()
            && event.payload.get("localWaitAborted").is_none()));
    assert!(events
        .iter()
        .any(|event| event.event_type == "cancel_requested"
            && event.payload.get("remoteCancellationConfirmed")
                == Some(&serde_json::Value::Bool(false))
            && event.payload.get("localWaitAborted") == Some(&serde_json::Value::Bool(true))));
    assert!(events
        .iter()
        .all(|event| event.event_type != "tool.completed"));
}

#[tokio::test]
async fn cancelling_a_task_before_its_canonical_run_returns_pending_without_false_terminal() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, MainChatAgentStrategy,
    };

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "cancel-missing-run".into(),
                user_goal: "This fixture intentionally has no AgentRun.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create task without run")
    };

    let pending =
        crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(&session.id, &state)
            .await
            .expect("the task-to-run race must accept a truthful pending cancellation");
    assert!(pending.cancellation_pending);
    assert!(!pending.can_cancel);
    assert_eq!(
        pending.session.as_ref().map(|session| session.status),
        Some(AgentTaskSessionStatus::Running)
    );

    let unchanged = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .load_session(&session.id)
            .expect("load task")
            .expect("task remains")
    };
    assert_eq!(unchanged.status, AgentTaskSessionStatus::Running);
    assert!(state
        .agent_run_store
        .as_ref()
        .expect("AgentRun store")
        .lock()
        .await
        .get_run_for_task_id(&session.id)
        .expect("load absent AgentRun")
        .is_none());
    assert!(state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .list(&session.id, 0, 100)
        .expect("list absent pre-run terminal facts")
        .is_empty());
    let registry = state
        .main_chat_runtime_state
        .lock()
        .await
        .cancellation_registry
        .clone();
    assert!(registry.is_cancellation_requested(&session.id));
}

#[tokio::test]
async fn main_chat_task_detail_final_delivery_requires_status_evidence() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, ExecutionTranscriptEntryDraft, ExecutionTranscriptEntryKind,
        MainChatAgentStrategy,
    };

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");

    let with_final_result = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        let session = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "final-delivery-status-chat".into(),
                user_goal: "Return an evidence-backed final result.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create final-result task");
        store
            .append_transcript_entry(ExecutionTranscriptEntryDraft {
                session_id: session.id.clone(),
                kind: ExecutionTranscriptEntryKind::FinalResult,
                summary: "Final result transcript entry exists.".into(),
                metadata: serde_json::json!({
                    "runId": "run-final-delivery-status-1",
                    "directWritesExecuted": false,
                }),
            })
            .expect("append final result");
        store
            .complete_session(&session.id, "Done.")
            .expect("complete final-result task")
    };
    let summary_only = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        let session = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "final-delivery-status-chat".into(),
                user_goal: "Only a stored final summary exists.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create summary-only task");
        store
            .complete_session(
                &session.id,
                "Stored final summary without transcript evidence.",
            )
            .expect("complete summary-only task")
    };

    let with_final_result_detail = get_main_chat_agent_task_detail(
        with_final_result.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("load final-result detail");
    assert_eq!(
        with_final_result_detail
            .final_delivery
            .as_ref()
            .and_then(|value| value.get("status"))
            .and_then(|value| value.as_str()),
        Some("completed"),
        "TaskDetail.final_delivery must carry explicit status evidence"
    );
    assert!(
        with_final_result_detail
            .final_delivery
            .as_ref()
            .is_some_and(|delivery| delivery.get("metadata").is_none()),
        "TaskDetail.final_delivery must not re-export canonical transcript metadata"
    );

    let summary_only_detail = get_main_chat_agent_task_detail(
        summary_only.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("load summary-only detail");
    assert!(
        summary_only_detail.final_delivery.is_none(),
        "stored final_summary alone must not become final_delivery evidence"
    );
}

#[tokio::test]
async fn failure_finalizer_records_timeout_run_session_and_transcript_evidence() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, MainChatAgentStrategy,
    };
    use openlife_core::agent::{AgentRun, AgentRunStatus};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "timeout-finalizer-chat".into(),
                user_goal: "Provider should time out in the harness.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec!["timeout-context".into()],
            })
            .expect("create session")
    };
    let mut run = AgentRun::new_chat_run(&session.chat_session_id, "provider timeout fixture");
    run.task_id = session.id.clone();
    {
        let run_store = state
            .agent_run_store
            .as_ref()
            .expect("run store")
            .lock()
            .await;
        run_store.create_run(&run).expect("create run");
    }

    let finalized = finalize_main_chat_task_failure(
        &state,
        Some(&run.id),
        Some(&session.id),
        MainChatTaskFailureKind::Timeout,
        "Provider timed out after the configured eval deadline.",
        "turn_runtime",
    )
    .await
    .expect("finalize timeout");
    assert_eq!(finalized.lifecycle_state, "timed_out");

    let stored_run = {
        let run_store = state
            .agent_run_store
            .as_ref()
            .expect("run store")
            .lock()
            .await;
        run_store
            .get_run(&run.id)
            .expect("load run")
            .expect("run exists")
    };
    assert_eq!(stored_run.status, AgentRunStatus::Failed);
    assert_eq!(
        stored_run.error.as_ref().map(|error| error.phase.as_str()),
        Some("timeout")
    );

    let stored_session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .load_session(&session.id)
            .expect("load session")
            .expect("session exists")
    };
    assert_eq!(stored_session.status, AgentTaskSessionStatus::Failed);

    let detail = get_main_chat_agent_task_detail(
        session.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("detail");
    assert_eq!(detail.evidence_view.lifecycle_state, "timed_out");
    assert_eq!(detail.evidence_view.projection_state, "consistent");
    let receipt = detail
        .evidence_view
        .durable_lifecycle_receipt
        .as_ref()
        .expect("durable timeout receipt");
    assert_eq!(receipt.event_type, "failed");
    assert_eq!(receipt.run_id, run.id);
    assert_eq!(receipt.failure_kind.as_deref(), Some("timeout"));
    assert_eq!(receipt.source_ref, "turn_runtime");
    assert!(detail.evidence_view.event_timeline.iter().any(|entry| {
        entry.failure_kind.as_deref() == Some("timeout")
            && entry.normalized_lifecycle_state.as_deref() == Some("timed_out")
            && entry.source_ref.as_deref() == Some("turn_runtime")
    }));
    let canonical_transcript = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("main chat session store")
        .lock()
        .await
        .list_transcript_entries(&session.id)
        .expect("load canonical timeout transcript");
    let timeout_entry = canonical_transcript
        .iter()
        .find(|entry| {
            entry
                .metadata
                .get("runId")
                .and_then(serde_json::Value::as_str)
                == Some(run.id.as_str())
        })
        .unwrap_or_else(|| {
            panic!(
                "timeout transcript entry missing from canonical transcript: {canonical_transcript:#?}"
            )
        });
    assert_eq!(timeout_entry.summary, "error_state_recorded");
    for forbidden_key in [
        "failureKind",
        "failure_kind",
        "safeReason",
        "safe_reason",
        "sourceRef",
        "source_ref",
        "routeEvidenceRef",
        "routeEvidence",
    ] {
        assert!(
            timeout_entry.metadata.get(forbidden_key).is_none(),
            "transcript must not duplicate terminal truth from the durable receipt: {forbidden_key}"
        );
    }
    assert!(timeout_entry
        .metadata
        .get("defaultDeniedMetadataReceipt")
        .and_then(serde_json::Value::as_str)
        .is_some());
    assert_eq!(
        detail.evidence_view.allowed_controls,
        vec!["open_trace".to_string()]
    );
}

#[tokio::test]
async fn failure_finalizer_rejects_an_unbound_synthetic_terminal() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let error = finalize_main_chat_task_failure(
        &state,
        None,
        None,
        MainChatTaskFailureKind::UnknownError,
        "Synthetic failure without canonical identity.",
        "test.synthetic_unbound_failure",
    )
    .await
    .expect_err("a failure receipt cannot exist without a canonical task/run binding");
    assert_eq!(error, "canonical_task_session_id_required_for_failure");
}

#[tokio::test]
async fn failure_finalizer_does_not_mutate_projections_when_durable_receipt_fails() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, MainChatAgentStrategy,
    };
    use openlife_core::agent::{AgentRun, AgentRunStatus};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "failure-receipt-injection".into(),
                user_goal: "A durable receipt failure must stop projection mutation.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create task")
    };
    let mut run = AgentRun::new_chat_run(&session.chat_session_id, &session.user_goal);
    run.task_id = session.id.clone();
    state
        .agent_run_store
        .as_ref()
        .expect("agent run store")
        .lock()
        .await
        .create_run(&run)
        .expect("create AgentRun");
    state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .install_failed_insert_failure_for_test()
        .expect("install failed-event fault");

    let error = finalize_main_chat_task_failure(
        &state,
        Some(&run.id),
        Some(&session.id),
        MainChatTaskFailureKind::ProviderError,
        "Injected provider failure.",
        "test.failure_receipt_injection",
    )
    .await
    .expect_err("durable receipt failure must abort the finalizer");
    assert!(error.contains("persist failure terminal receipt before projection failed"));

    let stored_run = state
        .agent_run_store
        .as_ref()
        .expect("agent run store")
        .lock()
        .await
        .get_run(&run.id)
        .expect("load AgentRun")
        .expect("AgentRun exists");
    assert_eq!(stored_run.status, AgentRunStatus::Running);
    let stored_session = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("task session store")
        .lock()
        .await
        .load_session(&session.id)
        .expect("load task")
        .expect("task exists");
    assert_eq!(stored_session.status, AgentTaskSessionStatus::Running);
    let transcript = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("task session store")
        .lock()
        .await
        .list_transcript_entries(&session.id)
        .expect("load transcript");
    assert!(transcript.is_empty());
}

#[tokio::test]
async fn failure_finalizer_does_not_display_non_timeout_failure_as_timed_out() {
    use openlife_core::agent::main_chat_agent_v1::{AgentTaskSessionDraft, MainChatAgentStrategy};
    use openlife_core::agent::AgentRun;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "provider-error-finalizer-chat".into(),
                user_goal: "Provider error should not become timed out.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create session")
    };
    let mut run = AgentRun::new_chat_run(&session.chat_session_id, "provider error fixture");
    run.task_id = session.id.clone();
    {
        let run_store = state
            .agent_run_store
            .as_ref()
            .expect("run store")
            .lock()
            .await;
        run_store.create_run(&run).expect("create run");
    }

    finalize_main_chat_task_failure(
        &state,
        Some(&run.id),
        Some(&session.id),
        MainChatTaskFailureKind::ProviderError,
        "Provider returned an error.",
        "v6.provider_error_replay",
    )
    .await
    .expect("finalize provider error");

    let detail = get_main_chat_agent_task_detail(
        session.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("detail");
    assert_eq!(detail.evidence_view.lifecycle_state, "failed");
    assert_eq!(detail.evidence_view.projection_state, "consistent");
    assert!(detail.evidence_view.durable_lifecycle_receipt.is_some());
    assert_ne!(detail.evidence_view.lifecycle_state, "timed_out");
    assert!(detail
        .evidence_view
        .event_timeline
        .iter()
        .any(|entry| entry.failure_kind.as_deref() == Some("provider_error")));
}

#[tokio::test]
async fn policy_blocker_finalizer_creates_auditable_detail_event_without_tool_call() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, MainChatAgentStrategy,
    };
    use openlife_core::agent::AgentRun;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "blocker-finalizer-chat".into(),
                user_goal: "Read a web or MCP target that policy blocks.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create session")
    };
    let mut run = AgentRun::new_chat_run(&session.chat_session_id, "blocked read fixture");
    run.task_id = session.id.clone();
    {
        let run_store = state
            .agent_run_store
            .as_ref()
            .expect("run store")
            .lock()
            .await;
        run_store.create_run(&run).expect("create run");
    }

    finalize_main_chat_task_failure(
        &state,
        Some(&run.id),
        Some(&session.id),
        MainChatTaskFailureKind::PolicyBlocker,
        "web_network_policy_blocked",
        "v4.web_mcp_blocker_replay",
    )
    .await
    .expect("finalize blocker");

    let stored_session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .load_session(&session.id)
            .expect("load session")
            .expect("session exists")
    };
    assert_eq!(stored_session.status, AgentTaskSessionStatus::Blocked);

    let detail = get_main_chat_agent_task_detail(
        session.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("detail");
    assert_eq!(detail.evidence_view.lifecycle_state, "blocked");
    assert_eq!(detail.evidence_view.projection_state, "consistent");
    assert!(detail.evidence_view.durable_lifecycle_receipt.is_some());
    assert_eq!(detail.evidence_view.action_count, 0);
    assert!(detail
        .evidence_view
        .blockers
        .contains(&"web_network_policy_blocked".to_string()));
    assert!(detail.evidence_view.event_timeline.iter().any(|entry| {
        entry.failure_kind.as_deref() == Some("policy_blocker")
            && entry.normalized_lifecycle_state.as_deref() == Some("blocked")
            && entry.summary == "durable_lifecycle_state_recorded"
    }));
}

#[tokio::test]
async fn main_chat_task_continuity_blocks_stale_terminal_and_changed_target_resume() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, ExecutionAction, ExecutionPolicy,
        ExecutionQueueStatus, ExecutionTranscriptEntryDraft, ExecutionTranscriptEntryKind,
        MainChatAgentStrategy,
    };
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");

    let stale = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        let stale = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "continuity-stale-chat".into(),
                user_goal: "Continue after context changed.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec!["context-now".into()],
            })
            .expect("create stale task");
        store
            .append_transcript_entry(ExecutionTranscriptEntryDraft {
                session_id: stale.id.clone(),
                kind: ExecutionTranscriptEntryKind::Observation,
                summary: "Original context digest was recorded before a context change.".into(),
                metadata: serde_json::json!({
                    "continuityContextDigest": "bytes:12 hash:sha256:old-context",
                    "contextSnapshotRef": "context-then",
                    "directWritesExecuted": false,
                }),
            })
            .expect("stale transcript");
        store
            .block_session(&stale.id, "Context requires review.")
            .expect("block stale task")
    };
    create_task_bound_agent_run_with_status_for_test(
        &state,
        &stale.id,
        &stale.chat_session_id,
        &stale.user_goal,
        openlife_core::agent::AgentRunStatus::WaitingPermission,
    )
    .await;

    let terminal = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        let terminal = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "continuity-terminal-chat".into(),
                user_goal: "Completed task should not resume.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create terminal task");
        store
            .complete_session(&terminal.id, "Already complete.")
            .expect("complete terminal task")
    };
    create_task_bound_agent_run_for_test(
        &state,
        &terminal.id,
        &terminal.chat_session_id,
        &terminal.user_goal,
    )
    .await;

    let (_, mismatched_input_hash) =
        openlife_core::agent::metadata_safe::metadata_safe_value_digest(
            &serde_json::json!({"not": "the current governed input"}),
        );
    let proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        "tool_permission.builtin.builtin_echo",
        serde_json::json!({
            "permission_scope_kind": "action_bound",
            "tool_name": "builtin_echo",
            "source": "builtin",
            "risk_level": "low",
            "action_type": "read",
            "permission": "allow_once",
            "mainChatAgentV1": true,
            "blocked_action": {
                "action_type": "mcp.read_only",
                "target": "mcp.call_tool",
                "resolved_target": "changed_builtin_echo_target",
                "input_hash": mismatched_input_hash,
                "input_length_bytes": 1
            }
        }),
        "Allow mismatched target.",
        0.7,
        RiskLevel::Medium,
        ProposalSource::ChatConversation,
    );
    let proposal_id = proposal.id.clone();
    {
        let proposal_store = state.proposal_store.as_ref().expect("proposal store");
        proposal_store
            .lock()
            .await
            .create_proposal(&proposal)
            .expect("create proposal");
        crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .expect("accept mismatched proposal");
    }
    let changed_target = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "continuity-scope-chat".into(),
                user_goal: "Use mcp builtin_echo read-only now.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some("Waiting for exact ToolPermission scope.".into()),
                context_snapshot_refs: vec!["scope-context".into()],
            })
            .expect("create changed target task")
    };
    let pending = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        let action = ExecutionAction::new(
            "mcp.read_only",
            "Pending registered MCP read action blocked on ToolPermission.",
        );
        let pending = queue
            .enqueue(
                &changed_target.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue pending action");
        project_test_read_receipt(
            &queue,
            &pending,
            openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
            serde_json::json!({
                "proposalId": proposal_id,
                "toolName": "builtin_echo",
                "directWritesExecuted": false,
            }),
            Some("tool_permission_required"),
        )
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .record_action_queue_id(&changed_target.id, &pending.id)
            .expect("record pending action id");
        store
            .set_pending_blockers(&changed_target.id, vec!["tool_permission_required".into()])
            .expect("set blocker");
        store
            .mark_waiting_permission(&changed_target.id)
            .expect("waiting permission");
    }
    create_task_bound_agent_run_with_status_for_test(
        &state,
        &changed_target.id,
        &changed_target.chat_session_id,
        &changed_target.user_goal,
        openlife_core::agent::AgentRunStatus::WaitingPermission,
    )
    .await;

    let stale_detail = get_main_chat_agent_task_detail(
        stale.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("stale detail");
    assert!(stale_detail.continuity_diagnostics.stale_context);
    assert!(stale_detail
        .allowed_controls
        .contains(&"refresh_context".to_string()));
    assert!(!stale_detail
        .allowed_controls
        .contains(&"resume".to_string()));
    assert!(resume_main_chat_agent_task(
        stale.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>()
    )
    .await
    .expect_err("stale task resume must fail closed")
    .contains("stale_context"));

    let terminal_detail = get_main_chat_agent_task_detail(
        terminal.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("terminal detail");
    assert_eq!(
        terminal_detail.task_session.status,
        AgentTaskSessionStatus::Completed
    );
    assert!(terminal_detail.continuity_diagnostics.terminal_no_resume);
    assert_eq!(
        terminal_detail.next_recommended_control,
        "wait_for_projection_reconciliation"
    );
    assert!(!terminal_detail
        .allowed_controls
        .contains(&"resume".to_string()));

    let changed_detail = get_main_chat_agent_task_detail(
        changed_target.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("changed target detail");
    assert!(
        changed_detail
            .continuity_diagnostics
            .permission_scope_mismatch
    );
    assert!(!changed_detail
        .allowed_controls
        .contains(&"resume".to_string()));
    let pending_after_detail = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        queue
            .load(&pending.id)
            .expect("load pending")
            .expect("pending exists")
    };
    assert_eq!(
        pending_after_detail.status,
        ExecutionQueueStatus::PendingPermission,
        "changed-target diagnostics must not replay pending actions"
    );
}

#[test]
fn resume_main_chat_task_preserves_pending_permission_blocker_instead_of_state_only() {
    let module_path = format!(
        "{}/src/main_chat_task_controls.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(module_path).expect("read task-control module");
    let resume_body = extract_rust_function_body(
        &source,
        "pub(crate) async fn resume_main_chat_agent_task_with_state(",
    );

    assert!(
        resume_body.contains("evaluate_main_chat_task_resume("),
        "resume command must evaluate blockers/actions before changing task state"
    );
    assert!(
        resume_body.contains("remain_waiting_permission"),
        "resume command must preserve pending permission state when blockers remain"
    );
    assert!(
        resume_body.contains("resumeBlockedByPendingPermission"),
        "resume command must expose permission-preserving resume metadata"
    );
}

#[tokio::test]
async fn resume_main_chat_task_replays_pending_action_after_tool_permission_acceptance() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, ExecutionAction, ExecutionPolicy,
        ExecutionQueueStatus, MainChatAgentStrategy,
    };
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut scheduler = state.scheduler.lock().await;
        scheduler.provider = "none".into();
        scheduler.openai_key.clear();
        scheduler.local_model.clear();
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");

    let user_goal = "Use mcp builtin_echo read-only now.";
    let plan = crate::main_chat_react_tool_selection::build_main_chat_react_action_plan(
        "resume-permission-command-surface",
        user_goal,
    )
    .expect("build exact pending action plan");
    let resolution = {
        let registry = state.mcp_registry.lock().await;
        crate::main_chat_react_tool_selection::resolve_main_chat_mcp_read_target(&registry, &plan)
    };
    assert!(resolution.blocker_reason.is_none());
    let (input_length_bytes, input_hash) =
        openlife_core::agent::metadata_safe::metadata_safe_value_digest(&resolution.arguments);

    let mut proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        "tool_permission.builtin.builtin_echo",
        serde_json::json!({
            "tool_name": "builtin_echo",
            "source": "builtin",
            "risk_level": "low",
            "action_type": "read",
            "permission": "allow_once",
            "blocked_action": {
                "action_type": plan.queue_action_type,
                "target": plan.target,
                "resolved_target": resolution.target,
                "input_hash": input_hash,
                "input_length_bytes": input_length_bytes,
            },
        }),
        "Allow the pending Main Chat MCP read action to continue.",
        0.7,
        RiskLevel::Medium,
        ProposalSource::ChatConversation,
    );
    let proposal_id = proposal.id.clone();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "resume-permission-command-surface".into(),
                user_goal: user_goal.into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some(
                    "Waiting for ToolPermission acceptance before replaying MCP read.".into(),
                ),
                context_snapshot_refs: vec!["resume-permission-context".into()],
            })
            .expect("create main chat task session")
    };
    let run_id = create_task_bound_agent_run_with_status_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        user_goal,
        openlife_core::agent::AgentRunStatus::WaitingPermission,
    )
    .await;
    let action = ExecutionAction::new(
        "mcp.read_only",
        "Pending registered MCP read action blocked on ToolPermission.",
    );
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue pending mcp action");
        queued
    };
    let envelope = replay_execution_envelope_for_test(&state, &session, &queued, &run_id).await;
    bind_tool_permission_proposal_to_replay_for_test(&mut proposal, &session, &envelope);
    {
        let proposal_store = state.proposal_store.as_ref().expect("proposal store");
        proposal_store
            .lock()
            .await
            .create_proposal(&proposal)
            .expect("create tool permission proposal");
    }
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        project_test_read_receipt(
            &queue,
            &queued,
            openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
            metadata_with_replay_envelope(
                serde_json::json!({
                    "proposalId": proposal_id,
                    "toolName": "builtin_echo",
                    "directWritesExecuted": false,
                }),
                &envelope,
            ),
            Some("tool_permission_required"),
        )
    };
    let parsed_receipt =
        serde_json::from_value::<openlife_core::tool_execution_receipt::ToolExecutionReceipt>(
            queued
                .observation_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("toolExecutionReceipt"))
                .cloned()
                .expect("typed receipt metadata"),
        );
    assert!(
        parsed_receipt.is_ok(),
        "stored typed receipt must round-trip: {:?}",
        parsed_receipt.err()
    );
    assert!(
        openlife_core::agent::main_chat_agent_v1::typed_tool_receipt_allows_automatic_retry(
            &queued
        ),
        "pending read replay eligibility must come from its typed pre-dispatch receipt: {queued:#?}"
    );
    assert!(
        openlife_core::agent::main_chat_agent_v1::action_replay_effect_is_safe_to_claim(&queued),
        "pending read must remain unclaimed and effect-not-attempted before resume"
    );
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &queued.id)
            .expect("record action id");
        store
            .set_pending_blockers(&session.id, vec!["tool_permission_required".into()])
            .expect("set pending blocker");
        store
            .mark_waiting_permission(&session.id)
            .expect("mark waiting permission");
    }

    crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &state)
        .await
        .expect("accept exact action-bound tool permission proposal");
    let accepted_scope =
        openlife_core::tool_permissions::ActionBoundToolPermissionScope::from_proposal_after(
            &proposal.after,
        )
        .expect("parse exact action-bound scope");
    assert!(state
        .tool_permission_store
        .lock()
        .await
        .peek_action_bound(&proposal_id, &accepted_scope)
        .expect("peek action-bound grant before replay")
        .is_some());

    let managed_state = app.state::<std::sync::Arc<crate::AppState>>();
    resume_main_chat_agent_task(session.id.clone(), managed_state)
        .await
        .expect("resume command should replay accepted ToolPermission action");

    let resumed = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .load_session(&session.id)
            .expect("load resumed session")
            .expect("resumed session exists")
    };
    assert_eq!(resumed.status, AgentTaskSessionStatus::Completed);
    assert!(resumed.pending_blockers.is_empty());

    let replayed = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        queue
            .load(&queued.id)
            .expect("load replayed action")
            .expect("replayed action exists")
    };
    assert_eq!(replayed.status, ExecutionQueueStatus::Completed);
    assert_eq!(
        replayed.replay_effect_certainty,
        openlife_core::agent::main_chat_agent_v1::ActionReplayEffectCertainty::Confirmed
    );
    assert!(matches!(
        replayed.replay_claim,
        openlife_core::agent::main_chat_agent_v1::ActionReplayClaimState::Claimed { .. }
    ));
    let metadata = replayed
        .observation_metadata
        .as_ref()
        .expect("replay observation metadata");
    assert_eq!(
        metadata["automaticReplayCompleted"],
        serde_json::json!(true)
    );
    assert_eq!(metadata["directWritesExecuted"], serde_json::json!(false));
    let permission_store = state.tool_permission_store.lock().await;
    assert!(
        permission_store
            .list()
            .expect("list manifest policies")
            .is_empty(),
        "action-bound replay must not create a global manifest permission"
    );
    assert!(permission_store
        .peek_action_bound(&proposal_id, &accepted_scope)
        .expect("peek consumed action-bound grant")
        .is_none());
    drop(permission_store);
    let second = resume_main_chat_agent_task(
        session.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect_err("completed action cannot consume or dispatch the same permission twice");
    assert!(second.contains("terminal_no_resume"));
}

#[tokio::test]
async fn replay_envelope_from_foreign_run_is_not_projected_or_dispatched() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, ExecutionAction, ExecutionPolicy,
        ExecutionQueueStatus, MainChatAgentStrategy,
    };
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");
    let user_goal = "Use mcp builtin_echo read-only now.";
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "foreign-run-envelope-chat".into(),
                user_goal: user_goal.into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some("Wait for exact permission.".into()),
                context_snapshot_refs: Vec::new(),
            })
            .expect("create task")
    };
    let canonical_run_id = create_task_bound_agent_run_with_status_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        user_goal,
        openlife_core::agent::AgentRunStatus::WaitingPermission,
    )
    .await;
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        let action = ExecutionAction::new(
            "mcp.read_only",
            "Pending registered MCP read action blocked on ToolPermission.",
        );
        queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue action")
    };
    let foreign_run_id = uuid::Uuid::new_v4().to_string();
    assert_ne!(foreign_run_id, canonical_run_id);
    let envelope =
        replay_execution_envelope_for_test(&state, &session, &queued, &foreign_run_id).await;
    let mut proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        "tool_permission.builtin.builtin_echo",
        serde_json::json!({
            "tool_name": "builtin_echo",
            "source": "builtin",
            "risk_level": "low",
            "action_type": "read",
            "permission": "allow_once",
        }),
        "This proposal is deliberately bound to a foreign run.",
        1.0,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    bind_tool_permission_proposal_to_replay_for_test(&mut proposal, &session, &envelope);
    let proposal_id = proposal.id.clone();
    {
        let proposal_store = state.proposal_store.as_ref().expect("proposal store");
        proposal_store
            .lock()
            .await
            .create_proposal(&proposal)
            .expect("create foreign-run proposal");
    }
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        project_test_read_receipt(
            &queue,
            &queued,
            openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
            metadata_with_replay_envelope(
                serde_json::json!({
                    "proposalId": proposal_id,
                    "toolName": "builtin_echo",
                    "directWritesExecuted": false,
                }),
                &envelope,
            ),
            Some("tool_permission_required"),
        )
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &queued.id)
            .expect("record action id");
        store
            .set_pending_blockers(&session.id, vec!["tool_permission_required".into()])
            .expect("set pending blocker");
        store
            .mark_waiting_permission(&session.id)
            .expect("mark waiting permission");
    }
    crate::commands::proposal::accept_proposal_with_state(proposal_id, &state)
        .await
        .expect("accept exact but foreign-run permission");

    let detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &session.id,
        &state,
    )
    .await
    .expect("load task detail");
    assert!(
        detail.continuity_diagnostics.permission_scope_mismatch,
        "foreign-run accepted permission must be diagnosed; detail={} raw_metadata={}",
        serde_json::to_string_pretty(&detail).expect("serialize foreign-run detail"),
        serde_json::to_string_pretty(&queued.observation_metadata)
            .expect("serialize foreign-run raw metadata")
    );
    assert_eq!(detail.retry_target_action_id, None);
    assert!(!detail
        .allowed_controls
        .iter()
        .any(|control| control == "retry"));
    assert!(!detail
        .allowed_controls
        .iter()
        .any(|control| control == "resume"));

    let retry_error = crate::main_chat_task_controls::retry_main_chat_agent_action(
        session.id.clone(),
        queued.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect_err("foreign-run envelope must not be a backend retry target");
    assert!(retry_error.contains("action_not_current_backend_retry_target"));
    let resumed =
        crate::main_chat_task_controls::resume_main_chat_agent_task_with_state(&session.id, &state)
            .await
            .expect("resume preserves a permission mismatch without dispatch");
    assert_eq!(
        resumed.session.expect("task state session").status,
        AgentTaskSessionStatus::WaitingPermission
    );
    let unchanged = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        queue
            .load(&queued.id)
            .expect("load unchanged action")
            .expect("unchanged action exists")
    };
    assert_eq!(unchanged.status, ExecutionQueueStatus::PendingPermission);
    assert!(matches!(
        unchanged.replay_claim,
        openlife_core::agent::main_chat_agent_v1::ActionReplayClaimState::Unclaimed
    ));
    assert_eq!(
        unchanged.replay_effect_certainty,
        openlife_core::agent::main_chat_agent_v1::ActionReplayEffectCertainty::EffectNotAttempted
    );
}

#[tokio::test]
async fn manifest_drift_after_snapshot_before_live_dispatch_fence_has_zero_dispatches() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, ExecutionAction, ExecutionPolicy, ExecutionQueueStatus,
        MainChatAgentStrategy,
    };
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
    use openlife_core::tool_manifest::{ToolIdempotencyContract, ToolSource};
    use std::sync::atomic::{AtomicUsize, Ordering};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let user_goal = "Use mcp builtin_echo read-only now.";
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "dispatch-manifest-drift-chat".into(),
                user_goal: user_goal.into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some("Wait for exact permission.".into()),
                context_snapshot_refs: Vec::new(),
            })
            .expect("create task")
    };
    let run_id = create_task_bound_agent_run_with_status_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        user_goal,
        openlife_core::agent::AgentRunStatus::WaitingPermission,
    )
    .await;
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        let action = ExecutionAction::new(
            "mcp.read_only",
            "Pending registered MCP read action blocked on ToolPermission.",
        );
        queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue action")
    };
    let envelope = replay_execution_envelope_for_test(&state, &session, &queued, &run_id).await;
    let mut proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        "tool_permission.builtin.builtin_echo",
        serde_json::json!({
            "tool_name": "builtin_echo",
            "source": "builtin",
            "risk_level": "low",
            "action_type": "read",
            "permission": "allow_once",
        }),
        "Allow the exact pending action once.",
        1.0,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    bind_tool_permission_proposal_to_replay_for_test(&mut proposal, &session, &envelope);
    let proposal_id = proposal.id.clone();
    {
        let proposal_store = state.proposal_store.as_ref().expect("proposal store");
        proposal_store
            .lock()
            .await
            .create_proposal(&proposal)
            .expect("create exact proposal");
    }
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        project_test_read_receipt(
            &queue,
            &queued,
            openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
            metadata_with_replay_envelope(
                serde_json::json!({
                    "proposalId": proposal_id,
                    "toolName": "builtin_echo",
                    "directWritesExecuted": false,
                }),
                &envelope,
            ),
            Some("tool_permission_required"),
        )
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &queued.id)
            .expect("record action id");
        store
            .set_pending_blockers(&session.id, vec!["tool_permission_required".into()])
            .expect("set blocker");
        store
            .mark_waiting_permission(&session.id)
            .expect("mark waiting permission");
    }
    crate::commands::proposal::accept_proposal_with_state(proposal_id, &state)
        .await
        .expect("accept exact permission");

    let actual_dispatch_count = std::sync::Arc::new(AtomicUsize::new(0));
    {
        let mut registry = state.mcp_registry.lock().await;
        let original = registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.id == "builtin_echo")
            .expect("original builtin_echo manifest");
        assert_eq!(
            original.execution_contract_digest(),
            envelope.manifest_contract_digest
        );
        registry.remove_builtins_by_source(|source| matches!(source, ToolSource::BuiltIn));
        let count = std::sync::Arc::clone(&actual_dispatch_count);
        registry.register_builtin(
            original,
            Box::new(move |_arguments| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok("stale snapshot adapter must not dispatch".into())
            }),
        );
    }
    let (_dispatch_fence_barrier, reached, release) =
        crate::main_chat_turn_runtime::install_main_chat_replay_dispatch_fence_barrier_for_test(
            &session.id,
        );
    let replay_state = std::sync::Arc::clone(&state);
    let replay_task_id = session.id.clone();
    let mut replay = tokio::spawn(async move {
        crate::main_chat_task_controls::resume_main_chat_agent_task_with_state(
            &replay_task_id,
            &replay_state,
        )
        .await
    });
    tokio::select! {
        _ = reached.wait() => {}
        result = &mut replay => {
            panic!("replay terminated before the live dispatch fence: {result:?}");
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
            panic!("replay did not reach or terminate before the live dispatch fence");
        }
    }

    {
        let mut registry = state.mcp_registry.lock().await;
        let mut drifted = registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.id == "builtin_echo")
            .expect("original builtin_echo manifest");
        assert_eq!(
            drifted.execution_contract_digest(),
            envelope.manifest_contract_digest
        );
        drifted.version = "2.0.0-drifted-after-precheck".into();
        drifted.action_type = "write".into();
        drifted.capabilities = vec!["write".into()];
        drifted.idempotency_contract = ToolIdempotencyContract::NonIdempotent;
        assert_ne!(
            drifted.execution_contract_digest(),
            envelope.manifest_contract_digest
        );
        let drifted_contract =
            openlife_core::agent::validate_manifest_execution_contract(&drifted).unwrap();
        assert_ne!(drifted_contract.action_effect, envelope.action_effect);
        assert_ne!(
            drifted_contract.idempotency_contract,
            envelope.idempotency_contract
        );
        registry.remove_builtins_by_source(|source| matches!(source, ToolSource::BuiltIn));
        let count = std::sync::Arc::clone(&actual_dispatch_count);
        registry.register_builtin(
            drifted,
            Box::new(move |_arguments| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok("must not dispatch drifted tool".into())
            }),
        );
    }
    release.wait().await;
    tokio::time::timeout(std::time::Duration::from_secs(3), replay)
        .await
        .expect("drifted replay terminates promptly")
        .expect("join drifted replay")
        .expect("drift is persisted as a governed failure");

    assert_eq!(actual_dispatch_count.load(Ordering::SeqCst), 0);
    let failed_before_dispatch = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await;
        queue
            .load(&queued.id)
            .expect("load drifted action")
            .expect("drifted action exists")
    };
    assert_eq!(failed_before_dispatch.status, ExecutionQueueStatus::Failed);
    assert_eq!(
        failed_before_dispatch.replay_effect_certainty,
        openlife_core::agent::main_chat_agent_v1::ActionReplayEffectCertainty::FailedBeforeDispatch
    );
    assert!(failed_before_dispatch.replay_dispatch_started_at.is_none());
    assert!(matches!(
        failed_before_dispatch.replay_claim,
        openlife_core::agent::main_chat_agent_v1::ActionReplayClaimState::Unclaimed
    ));
}

#[tokio::test]
async fn resume_main_chat_task_reaches_executor_for_native_web_tool_permission_scope() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, ExecutionAction, ExecutionPolicy,
        ExecutionQueueStatus, ExecutionTranscriptEntryKind, MainChatAgentStrategy,
    };
    use openlife_core::agent::metadata_safe::metadata_safe_value_digest;
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");

    let user_goal =
        "请告诉我今天旧金山的天气。必须使用可审计的 web/weather 读取证据；如果当前没有可用外部读取工具，请明确 fail closed，不要猜。";
    let plan = crate::main_chat_react_tool_selection::build_main_chat_react_action_plan(
        "resume-native-web-permission-chat",
        user_goal,
    )
    .expect("build web.search action plan");
    assert_eq!(plan.queue_action_type, "web.search");
    assert_eq!(plan.executor_action_type, "mcp_tool");
    let native_governed_input = serde_json::json!({
        "governedInputSource": "kernel_external_fact_query_from_governance_intent",
        "query": user_goal,
        "max_results": 5,
    });
    let (input_length_bytes, input_hash) = metadata_safe_value_digest(&native_governed_input);

    let mut proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        "tool_permission.builtin.web.search",
        serde_json::json!({
            "tool_name": "web.search",
            "source": "builtin",
            "risk_level": "medium",
            "permission_action": "grant",
            "policy": "allow_once",
            "canonical_scope": {
                "tool_name": "web.search",
                "source": "builtin",
                "risk_level": "medium",
                "action_type": "read",
                "input_hash": input_hash,
                "input_length_bytes": input_length_bytes
            },
            "blocked_action": {
                "action_type": "mcp_tool",
                "target": "web.search",
                "resolved_target": "web.search"
            },
            "auto_generated": true,
            "mainChatAgentV1": true,
            "directWritesExecuted": false
        }),
        "Allow the pending Main Chat web.search read action to continue.",
        0.7,
        RiskLevel::Medium,
        ProposalSource::ChatConversation,
    );
    let proposal_id = proposal.id.clone();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "resume-native-web-permission-chat".into(),
                user_goal: user_goal.into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some(
                    "Waiting for native ToolPermission acceptance before replaying web.search."
                        .into(),
                ),
                context_snapshot_refs: vec!["resume-native-web-permission-context".into()],
            })
            .expect("create main chat task session")
    };
    let run_id = create_task_bound_agent_run_with_status_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        user_goal,
        openlife_core::agent::AgentRunStatus::WaitingPermission,
    )
    .await;
    let action = ExecutionAction::new(
        &plan.queue_action_type,
        "Pending web.search action blocked on ToolPermission.",
    );
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue pending web action");
        queued
    };
    let envelope = replay_execution_envelope_for_test(&state, &session, &queued, &run_id).await;
    bind_tool_permission_proposal_to_replay_for_test(&mut proposal, &session, &envelope);
    {
        let proposal_store = state.proposal_store.as_ref().expect("proposal store");
        proposal_store
            .lock()
            .await
            .create_proposal(&proposal)
            .expect("create native web ToolPermission proposal");
    }
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        project_test_read_receipt(
            &queue,
            &queued,
            openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
            metadata_with_replay_envelope(
                serde_json::json!({
                    "proposalId": proposal_id,
                    "toolName": "web.search",
                    "governedInput": native_governed_input,
                    "governedInputDigest": [input_length_bytes, input_hash],
                    "queueActionType": "web.search",
                    "executorActionType": "mcp_tool",
                    "selectedCandidateTarget": "web.search",
                    "target": "web.search",
                    "directWritesExecuted": false,
                }),
                &envelope,
            ),
            Some("tool_permission_required"),
        )
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &queued.id)
            .expect("record action id");
        store
            .set_pending_blockers(&session.id, vec!["tool_permission_required".into()])
            .expect("set pending blocker");
        store
            .mark_waiting_permission(&session.id)
            .expect("mark waiting permission");
    }

    crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &state)
        .await
        .expect("accept native web ToolPermission proposal");
    let accepted_scope =
        openlife_core::tool_permissions::ActionBoundToolPermissionScope::from_proposal_after(
            &proposal.after,
        )
        .expect("parse native web action-bound scope");
    {
        let permissions = state.tool_permission_store.lock().await;
        assert!(
            permissions
                .list()
                .expect("list global manifest permissions")
                .is_empty(),
            "an action-bound web permission must not create a global manifest grant"
        );
        assert!(permissions
            .peek_action_bound(&proposal_id, &accepted_scope)
            .expect("peek accepted native web action-bound permission")
            .is_some());
    }
    let pre_resume_detail =
        crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
            &session.id,
            &state,
        )
        .await
        .expect("load pre-resume native web task detail");
    assert!(
        !pre_resume_detail
            .continuity_diagnostics
            .permission_scope_mismatch,
        "accepted native web ToolPermission scope must match the pending action before resume: {:?}",
        pre_resume_detail.continuity_diagnostics
    );
    assert!(
        pre_resume_detail
            .allowed_controls
            .iter()
            .any(|control| control == "resume"),
        "accepted native web ToolPermission task should expose resume control: {:?}",
        pre_resume_detail.allowed_controls
    );

    let managed_state = app.state::<std::sync::Arc<crate::AppState>>();
    resume_main_chat_agent_task(session.id.clone(), managed_state)
        .await
        .expect("resume command should replay accepted native web ToolPermission action");

    let post_resume_detail =
        crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
            &session.id,
            &state,
        )
        .await
        .expect("load post-resume native web task detail");
    let resumed = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .load_session(&session.id)
            .expect("load resumed session")
            .expect("resumed session exists")
    };
    assert_eq!(
        resumed.status,
        AgentTaskSessionStatus::WaitingPermission,
        "post-resume native web detail: {}",
        serde_json::to_string_pretty(&post_resume_detail).expect("serialize post-resume detail")
    );
    assert!(
        resumed
            .pending_blockers
            .iter()
            .any(|blocker| blocker == "network_policy_consent_required"),
        "native web resume must stay fail-closed on the distinct network-consent boundary: {:?}",
        resumed.pending_blockers
    );

    let replayed = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        queue
            .load(&queued.id)
            .expect("load replayed action")
            .expect("replayed action exists")
    };
    assert_eq!(replayed.status, ExecutionQueueStatus::PendingPermission);
    assert_eq!(
        replayed.replay_claim,
        openlife_core::agent::main_chat_agent_v1::ActionReplayClaimState::Unclaimed,
        "network consent is evaluated before ToolGateway's dispatch observer, so the replay claim must be safely released"
    );
    assert_eq!(
        replayed.replay_effect_certainty,
        openlife_core::agent::main_chat_agent_v1::ActionReplayEffectCertainty::EffectNotAttempted,
        "a network-consent blocker must not be overreported as dispatched or remote-unknown"
    );
    let metadata = replayed
        .observation_metadata
        .as_ref()
        .expect("replay observation metadata");
    assert!(metadata.get("automaticReplayCompleted").is_none());
    assert_eq!(
        metadata["automaticReplayNeedsPermission"],
        serde_json::json!(true)
    );
    assert_eq!(metadata["directWritesExecuted"], serde_json::json!(false));
    assert!(
        state
            .tool_permission_store
            .lock()
            .await
            .peek_action_bound(&proposal_id, &accepted_scope)
            .expect("peek web permission after independent network blocker")
            .is_some(),
        "a pre-dispatch network-consent blocker must not consume the accepted action permission"
    );
    let canonical_transcript = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("main chat session store")
        .lock()
        .await
        .list_transcript_entries(&session.id)
        .expect("load canonical replay blocker transcript");
    assert!(canonical_transcript.iter().any(|entry| {
        entry.kind == ExecutionTranscriptEntryKind::PermissionRequest
            && entry.summary == "permission_request_recorded"
    }));
}

#[tokio::test]
async fn resume_main_chat_task_does_not_replay_tool_permission_without_exact_action_scope() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, ExecutionAction, ExecutionPolicy,
        ExecutionQueueStatus, MainChatAgentStrategy,
    };
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");

    let proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        "tool_permission.builtin.builtin_echo",
        serde_json::json!({
            "tool_name": "builtin_echo",
            "source": "builtin",
            "risk_level": "low",
            "action_type": "read",
            "permission": "allow_once"
        }),
        "Allow the pending Main Chat MCP read action to continue.",
        0.7,
        RiskLevel::Medium,
        ProposalSource::ChatConversation,
    );
    let proposal_id = proposal.id.clone();
    {
        let proposal_store = state.proposal_store.as_ref().expect("proposal store");
        proposal_store
            .lock()
            .await
            .create_proposal(&proposal)
            .expect("create generic tool permission proposal");
    }

    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "resume-permission-changed-target".into(),
                user_goal: "Use mcp builtin_echo read-only now.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some(
                    "Waiting for ToolPermission acceptance before replaying MCP read.".into(),
                ),
                context_snapshot_refs: vec!["resume-permission-context".into()],
            })
            .expect("create main chat task session")
    };
    let action = ExecutionAction::new(
        "mcp.read_only",
        "Pending registered MCP read action blocked on ToolPermission.",
    );
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue pending mcp action");
        project_test_read_receipt(
            &queue,
            &queued,
            openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
            serde_json::json!({
                "proposalId": proposal_id,
                "toolName": "builtin_echo",
                "directWritesExecuted": false,
            }),
            Some("tool_permission_required"),
        )
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &queued.id)
            .expect("record action id");
        store
            .set_pending_blockers(&session.id, vec!["tool_permission_required".into()])
            .expect("set pending blocker");
        store
            .mark_waiting_permission(&session.id)
            .expect("mark waiting permission");
    }

    let acceptance_error =
        crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .expect_err("unscoped AllowOnce ToolPermission must stay pending");
    assert!(acceptance_error.contains("permission_scope_kind"));
    let permissions = state.tool_permission_store.lock().await;
    assert!(permissions
        .list()
        .expect("list manifest permissions")
        .is_empty());
    assert_eq!(permissions.action_bound_permission_count().unwrap(), 0);
    drop(permissions);

    let managed_state = app.state::<std::sync::Arc<crate::AppState>>();
    let task_state = resume_main_chat_agent_task(session.id.clone(), managed_state)
        .await
        .expect("scope mismatch should preserve the permission blocker");

    assert_eq!(
        task_state.session.as_ref().map(|session| &session.status),
        Some(&AgentTaskSessionStatus::WaitingPermission)
    );
    assert!(task_state.pending_approval_count >= 1);

    let not_replayed = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        queue
            .load(&queued.id)
            .expect("load pending action")
            .expect("pending action exists")
    };
    assert_eq!(not_replayed.status, ExecutionQueueStatus::PendingPermission);
    assert_eq!(not_replayed.attempts, 0);
    assert!(matches!(
        not_replayed.replay_claim,
        openlife_core::agent::main_chat_agent_v1::ActionReplayClaimState::Unclaimed
    ));
    let events = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .list(&session.id, 0, 100)
        .expect("list task events");
    assert!(
        events
            .iter()
            .all(|event| event.event_type != "tool.started"),
        "missing exact action scope must fail before ToolGateway dispatch"
    );
    let canonical_transcript = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("main chat session store")
        .lock()
        .await
        .list_transcript_entries(&session.id)
        .expect("load canonical pending permission transcript");
    let transcript = canonical_transcript
        .iter()
        .find(|entry| {
            entry
                .metadata
                .get("resumeBlockedByPendingPermission")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        })
        .expect("resume blocker transcript");
    assert_eq!(
        transcript.metadata["directWritesExecuted"],
        serde_json::json!(false)
    );
}

#[tokio::test]
async fn cancel_main_chat_task_cancels_nonterminal_queued_actions() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, ExecutionAction, ExecutionPolicy,
        ExecutionQueueStatus, MainChatAgentStrategy,
    };
    use openlife_core::agent::{AgentRun, AgentRunStatus};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");

    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "cancel-command-surface".into(),
                user_goal: "Search memory then request external write confirmation.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some("Cancel should stop queued work.".into()),
                context_snapshot_refs: vec!["cancel-context".into()],
            })
            .expect("create main chat task session")
    };
    let mut run = AgentRun::new_chat_run(&session.chat_session_id, "cancel running task fixture");
    run.task_id = session.id.clone();
    {
        let run_store = state
            .agent_run_store
            .as_ref()
            .expect("run store")
            .lock()
            .await;
        run_store.create_run(&run).expect("create cancel run");
    }
    let planned_action = ExecutionAction::new("memory.search", "Queued read action.");
    let permission_action = ExecutionAction::new("external.write", "Queued external write.");
    let (planned_id, permission_id) = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        let planned = queue
            .enqueue(
                &session.id,
                planned_action.clone(),
                ExecutionPolicy.classify(&planned_action),
            )
            .expect("enqueue planned action");
        let pending = queue
            .enqueue(
                &session.id,
                permission_action.clone(),
                ExecutionPolicy.classify(&permission_action),
            )
            .expect("enqueue pending action");
        (planned.id, pending.id)
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &planned_id)
            .expect("record planned action");
        store
            .record_action_queue_id(&session.id, &permission_id)
            .expect("record pending action");
        store
            .set_pending_blockers(
                &session.id,
                vec!["external_write_requires_confirmation".into()],
            )
            .expect("set pending blocker");
        store
            .mark_waiting_permission(&session.id)
            .expect("mark waiting permission");
    }

    let managed_state = app.state::<std::sync::Arc<crate::AppState>>();
    cancel_main_chat_agent_task(session.id.clone(), managed_state)
        .await
        .expect("cancel command should cancel nonterminal actions");
    let terminal_state = get_main_chat_agent_task_state(
        session.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("load terminal cancel state");
    assert!(
        !terminal_state.cancellation_pending,
        "durable Cancelled terminal state must not remain cancellation-pending"
    );

    let cancelled_session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store")
            .lock()
            .await;
        store
            .load_session(&session.id)
            .expect("load cancelled session")
            .expect("cancelled session exists")
    };
    assert_eq!(cancelled_session.status, AgentTaskSessionStatus::Cancelled);
    let cancelled_run = {
        let run_store = state
            .agent_run_store
            .as_ref()
            .expect("run store")
            .lock()
            .await;
        run_store
            .get_run(&run.id)
            .expect("load cancel run")
            .expect("cancel run exists")
    };
    assert_eq!(cancelled_run.status, AgentRunStatus::Cancelled);

    let actions = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue")
            .lock()
            .await;
        queue
            .list_for_session(&session.id)
            .expect("list cancelled actions")
    };
    for action in actions {
        assert_eq!(
            action.status,
            ExecutionQueueStatus::Cancelled,
            "cancel must stop queued action {}",
            action.id
        );
        assert_eq!(
            action
                .observation_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("cancelRequested"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }

    let detail = get_main_chat_agent_task_detail(
        session.id.clone(),
        app.state::<std::sync::Arc<crate::AppState>>(),
    )
    .await
    .expect("cancel detail");
    assert_eq!(detail.evidence_view.lifecycle_state, "cancelled");
    assert!(detail.evidence_view.event_timeline.iter().any(|entry| {
        entry.failure_kind.as_deref() == Some("cancelled")
            && entry.normalized_lifecycle_state.as_deref() == Some("cancelled")
    }));
    let lifecycle_events = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .list(&session.id, 0, 100)
        .expect("list cancellation receipts")
        .into_iter()
        .filter(|event| {
            matches!(
                event.event_type.as_str(),
                "cancel_requested" | "local_aborted"
            )
        })
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert_eq!(lifecycle_events, vec!["cancel_requested", "local_aborted"]);
}

#[tokio::test]
async fn active_turn_cancel_command_never_preempts_runtime_terminalization() {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionDraft, AgentTaskSessionStatus, MainChatAgentStrategy,
    };
    use openlife_core::agent::{AgentRun, AgentRunStatus};

    for settled_outcome in ["committed", "unknown"] {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let session = {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("task session store")
                .lock()
                .await;
            store
                .create_session(AgentTaskSessionDraft {
                    chat_session_id: format!("active-cancel-{settled_outcome}"),
                    user_goal: "An active runtime must own terminalization.".into(),
                    selected_strategy: MainChatAgentStrategy::DirectAnswer,
                    current_plan_summary: None,
                    context_snapshot_refs: vec![],
                })
                .expect("create active task")
        };
        let mut run = AgentRun::new_chat_run(&session.chat_session_id, &session.user_goal);
        run.task_id = session.id.clone();
        state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await
            .create_run(&run)
            .expect("create active AgentRun");
        let registry = state
            .main_chat_runtime_state
            .lock()
            .await
            .cancellation_registry
            .clone();
        let registration = registry
            .try_register(&session.id)
            .expect("register active runtime owner");
        let permit = registration
            .execution_epoch()
            .begin_canonical_commit("memory", format!("memory:{settled_outcome}"))
            .expect("begin canonical commit fact");
        if settled_outcome == "committed" {
            permit.finish_committed();
        } else {
            drop(permit);
        }

        crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(&session.id, &state)
            .await
            .expect("request active cancellation");

        let stored_session = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await
            .load_session(&session.id)
            .expect("load active task")
            .expect("active task exists");
        assert_eq!(stored_session.status, AgentTaskSessionStatus::Running);
        let stored_run = state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await
            .get_run(&run.id)
            .expect("load active run")
            .expect("active run exists");
        assert_eq!(stored_run.status, AgentRunStatus::Running);
        let durable_events = state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await
            .list(&session.id, 0, 100)
            .expect("list durable events");
        assert!(durable_events.iter().all(|event| {
            !matches!(
                event.event_type.as_str(),
                "local_aborted" | "interrupted" | "failed" | "final_delivery.created"
            )
        }));
        let cancellation_transcript = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await
            .list_transcript_entries(&session.id)
            .expect("load cancellation transcript")
            .into_iter()
            .rev()
            .find(|entry| entry.metadata["cancelRequested"] == serde_json::json!(true))
            .expect("cancellation request transcript");
        assert_eq!(
            cancellation_transcript.metadata["canonicalEffectState"],
            serde_json::json!(settled_outcome)
        );
        assert_eq!(
            cancellation_transcript.metadata["directWritesExecuted"],
            if settled_outcome == "committed" {
                serde_json::json!(true)
            } else {
                serde_json::Value::Null
            }
        );
        assert_eq!(
            cancellation_transcript.metadata["terminalDispositionPending"],
            serde_json::json!(true)
        );
        let detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
            &session.id,
            &state,
        )
        .await
        .expect("active task detail");
        assert_eq!(detail.evidence_view.lifecycle_state, "running");
        assert_eq!(detail.allowed_controls, vec!["cancel", "open_trace"]);

        drop(registration);
    }
}

#[tokio::test]
async fn durable_terminal_event_overrides_stale_projection_and_disables_controls() {
    use openlife_core::agent::main_chat_agent_v1::{AgentTaskSessionDraft, MainChatAgentStrategy};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "durable-terminal-overrides-projection".into(),
                user_goal: "Keep stale task projection running for this fixture.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create stale projected task")
    };
    let run_id = create_task_bound_agent_run_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        &session.user_goal,
    )
    .await;
    crate::main_chat_event_stream::append_main_chat_agent_runtime_event_batch(
        &state,
        &session.id,
        &run_id,
        vec![
            crate::main_chat_event_stream::MainChatAgentRuntimeEventInput::new(
                "local_aborted",
                "turn",
                "durable-terminal-cancel",
                "test_runtime",
                serde_json::json!({
                    "status": "local_aborted",
                    "directWritesExecuted": false,
                }),
            ),
        ],
    )
    .await
    .expect("append durable cancellation fact");

    let detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &session.id,
        &state,
    )
    .await
    .expect("build evidence view from durable terminal fact");
    assert_eq!(
        detail.task_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Running
    );
    assert_eq!(detail.evidence_view.lifecycle_state, "cancelled");
    assert_eq!(detail.evidence_view.projection_state, "pending");
    assert_eq!(detail.allowed_controls, vec!["open_trace"]);
    assert_eq!(detail.evidence_view.allowed_controls, vec!["open_trace"]);
    assert_eq!(
        detail.next_recommended_control,
        "wait_for_projection_reconciliation"
    );
    let receipt = detail
        .evidence_view
        .durable_lifecycle_receipt
        .expect("durable lifecycle receipt");
    assert_eq!(receipt.event_type, "local_aborted");
    assert_eq!(receipt.run_id, run_id);
    assert_eq!(receipt.lifecycle_state, "cancelled");
}

#[test]
fn durable_terminal_projection_maps_event_type_and_typed_kind_without_reinterpreting_status() {
    for (event_type, durable_status, kind, lifecycle, failure_kind) in [
        ("failed", "failed", "timeout", "timed_out", "timeout"),
        (
            "local_aborted",
            "local_aborted",
            "cancelled",
            "cancelled",
            "cancelled",
        ),
        (
            "interrupted",
            "interrupted",
            "interrupted",
            "interrupted",
            "interrupted",
        ),
        (
            "failed",
            "failed",
            "provider_error",
            "failed",
            "provider_error",
        ),
        ("failed", "failed", "tool_error", "failed", "tool_error"),
        (
            "failed",
            "failed",
            "policy_blocker",
            "blocked",
            "policy_blocker",
        ),
        (
            "failed",
            "failed",
            "unknown_error",
            "failed",
            "unknown_error",
        ),
    ] {
        let projected = crate::main_chat_task_controls::durable_terminal_projection_for_test(
            event_type,
            serde_json::json!({
                "status": durable_status,
                "kind": kind,
            }),
        );
        assert_eq!(projected.0, lifecycle, "lifecycle mapping for {kind}");
        assert_eq!(
            projected.1.as_deref(),
            Some(failure_kind),
            "failure kind mapping for {kind}"
        );
    }
}

#[tokio::test]
async fn task_list_terminal_filter_uses_durable_lifecycle_not_stale_task_status() {
    use openlife_core::agent::main_chat_agent_v1::{AgentTaskSessionDraft, MainChatAgentStrategy};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "terminal-filter-durable-truth".into(),
                user_goal: "Exclude a durably terminal task even while projection is running."
                    .into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create stale running task")
    };
    let run_id = create_task_bound_agent_run_with_status_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        &session.user_goal,
        openlife_core::agent::AgentRunStatus::Running,
    )
    .await;
    crate::main_chat_event_stream::append_main_chat_agent_runtime_event_batch(
        &state,
        &session.id,
        &run_id,
        vec![
            crate::main_chat_event_stream::MainChatAgentRuntimeEventInput::new(
                "local_aborted",
                "turn",
                "terminal-filter-cancel",
                "test_runtime",
                serde_json::json!({"status": "local_aborted"}),
            ),
        ],
    )
    .await
    .expect("append durable terminal fact");

    let nonterminal = crate::main_chat_task_controls::list_main_chat_agent_tasks_with_state(
        Some(crate::main_chat_task_controls::MainChatAgentTaskFilter {
            statuses: Vec::new(),
            conversation_id: None,
            include_terminal: false,
            include_stale: true,
        }),
        Some(10),
        Some(0),
        &state,
    )
    .await
    .expect("list nonterminal tasks");
    assert!(nonterminal
        .iter()
        .all(|summary| summary.task_session_id != session.id));
}

#[tokio::test]
async fn run_evidence_resolves_agent_run_by_exact_task_not_same_conversation() {
    use openlife_core::agent::main_chat_agent_v1::{AgentTaskSessionDraft, MainChatAgentStrategy};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let (first, second) = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        let first = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "shared-conversation-exact-task-run".into(),
                user_goal: "First task in one conversation.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create first task");
        let second = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "shared-conversation-exact-task-run".into(),
                user_goal: "Second task in one conversation.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create second task");
        (first, second)
    };
    let first_run_id = create_task_bound_agent_run_for_test(
        &state,
        &first.id,
        &first.chat_session_id,
        &first.user_goal,
    )
    .await;
    let second_run_id = create_task_bound_agent_run_for_test(
        &state,
        &second.id,
        &second.chat_session_id,
        &second.user_goal,
    )
    .await;

    let first_detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &first.id, &state,
    )
    .await
    .expect("first task detail");
    let second_detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &second.id, &state,
    )
    .await
    .expect("second task detail");

    assert_eq!(
        first_detail.evidence_view.run_id.as_deref(),
        Some(first_run_id.as_str())
    );
    assert_eq!(
        second_detail.evidence_view.run_id.as_deref(),
        Some(second_run_id.as_str())
    );
    assert_ne!(
        first_detail.evidence_view.run_id,
        second_detail.evidence_view.run_id
    );
}

#[tokio::test]
async fn durable_receipt_run_mismatch_degrades_evidence_and_disables_effectful_controls() {
    use openlife_core::agent::main_chat_agent_v1::{AgentTaskSessionDraft, MainChatAgentStrategy};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "durable-receipt-run-mismatch".into(),
                user_goal: "Do not join a receipt to the wrong AgentRun.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create task")
    };
    let canonical_run_id = create_task_bound_agent_run_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        &session.user_goal,
    )
    .await;
    let mismatched_run_id = uuid::Uuid::new_v4().to_string();
    crate::main_chat_event_stream::append_main_chat_agent_runtime_event_batch(
        &state,
        &session.id,
        &mismatched_run_id,
        vec![
            crate::main_chat_event_stream::MainChatAgentRuntimeEventInput::new(
                "failed",
                "turn",
                "mismatched-run-terminal",
                "test_runtime",
                serde_json::json!({"status": "failed"}),
            ),
        ],
    )
    .await
    .expect("append mismatched durable terminal receipt");

    let detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &session.id,
        &state,
    )
    .await
    .expect("build fail-closed task detail");

    assert_eq!(
        detail.evidence_view.run_id.as_deref(),
        Some(canonical_run_id.as_str())
    );
    assert_eq!(detail.evidence_view.lifecycle_state, "unknown");
    assert_eq!(detail.evidence_view.projection_state, "degraded");
    assert_eq!(detail.evidence_view.identity_state, "conflict");
    assert_eq!(detail.allowed_controls, vec!["open_trace"]);
    assert_eq!(
        detail.next_recommended_control,
        "wait_for_projection_reconciliation"
    );
}

#[tokio::test]
async fn durable_interrupted_lifecycle_is_never_downgraded_to_cancelled() {
    use openlife_core::agent::main_chat_agent_v1::{AgentTaskSessionDraft, MainChatAgentStrategy};

    for (event_type, durable_status, expected_lifecycle) in
        [("interrupted", "interrupted", "interrupted")]
    {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let session = {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("task session store")
                .lock()
                .await;
            let session = store
                .create_session(AgentTaskSessionDraft {
                    chat_session_id: format!("durable-{expected_lifecycle}-lifecycle"),
                    user_goal: "Preserve an uncertain terminal lifecycle.".into(),
                    selected_strategy: MainChatAgentStrategy::DirectAnswer,
                    current_plan_summary: None,
                    context_snapshot_refs: vec![],
                })
                .expect("create task");
            store
                .fail_session(&session.id, "Projected failure awaiting durable receipt.")
                .expect("fail projected task")
        };
        let run_id = create_task_bound_agent_run_with_status_for_test(
            &state,
            &session.id,
            &session.chat_session_id,
            &session.user_goal,
            openlife_core::agent::AgentRunStatus::Failed,
        )
        .await;
        crate::main_chat_event_stream::append_main_chat_agent_runtime_event_batch(
            &state,
            &session.id,
            &run_id,
            vec![
                crate::main_chat_event_stream::MainChatAgentRuntimeEventInput::new(
                    event_type,
                    if event_type == "final_delivery.created" {
                        "final_delivery"
                    } else {
                        "turn"
                    },
                    format!("terminal-{expected_lifecycle}"),
                    "test_runtime",
                    serde_json::json!({
                        "status": durable_status,
                        "reasonCode": "test_uncertain_terminal",
                    }),
                ),
            ],
        )
        .await
        .expect("append uncertain terminal receipt");

        let detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
            &session.id,
            &state,
        )
        .await
        .expect("build uncertain terminal detail");
        assert_eq!(detail.evidence_view.lifecycle_state, expected_lifecycle);
        assert_ne!(detail.evidence_view.lifecycle_state, "cancelled");
        assert_eq!(detail.evidence_view.identity_state, "consistent");
        assert_eq!(detail.allowed_controls, vec!["open_trace"]);
        assert_eq!(
            detail.next_recommended_control,
            "wait_for_projection_reconciliation"
        );
    }
}

#[tokio::test]
async fn projected_terminal_without_durable_receipt_is_pending_not_completed() {
    use openlife_core::agent::main_chat_agent_v1::{AgentTaskSessionDraft, MainChatAgentStrategy};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        let session = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "terminal-projection-missing-receipt".into(),
                user_goal: "A projection alone must not prove completion.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create task");
        store
            .complete_session(&session.id, "Projected completion only.")
            .expect("complete projected task")
    };
    create_task_bound_agent_run_for_test(
        &state,
        &session.id,
        &session.chat_session_id,
        &session.user_goal,
    )
    .await;

    let detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &session.id,
        &state,
    )
    .await
    .expect("build task detail");

    assert_eq!(detail.evidence_view.lifecycle_state, "unknown");
    assert_eq!(detail.evidence_view.projection_state, "pending");
    assert_eq!(detail.evidence_view.identity_state, "consistent");
    assert_eq!(detail.evidence_view.snapshot_state, "stable");
    assert_eq!(detail.evidence_view.durable_sequence_before, Some(0));
    assert_eq!(detail.evidence_view.durable_sequence_after, Some(0));
    assert!(detail.evidence_view.durable_lifecycle_receipt.is_none());
    assert_eq!(detail.allowed_controls, vec!["open_trace"]);
    assert_eq!(
        detail.next_recommended_control,
        "wait_for_projection_reconciliation"
    );
}

#[tokio::test]
async fn missing_canonical_agent_run_degrades_evidence_instead_of_borrowing_chat_history() {
    use openlife_core::agent::main_chat_agent_v1::{AgentTaskSessionDraft, MainChatAgentStrategy};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "missing-canonical-run-chat".into(),
                user_goal: "Do not borrow another task's AgentRun.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create task without AgentRun")
    };

    let detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &session.id,
        &state,
    )
    .await
    .expect("build fail-closed missing-run detail");

    assert!(detail.evidence_view.run_id.is_none());
    assert_eq!(detail.evidence_view.identity_state, "missing");
    assert_eq!(detail.evidence_view.lifecycle_state, "unknown");
    assert_eq!(detail.evidence_view.projection_state, "degraded");
    assert_eq!(detail.allowed_controls, vec!["open_trace"]);
}

#[tokio::test]
async fn active_task_without_terminal_receipt_only_allows_cancel_and_trace() {
    use openlife_core::agent::main_chat_agent_v1::{AgentTaskSessionDraft, MainChatAgentStrategy};
    use openlife_core::agent::AgentRun;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "active-no-terminal-receipt".into(),
                user_goal: "Keep the active task controls minimal.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create running task")
    };
    {
        let mut run = AgentRun::new_chat_run(&session.chat_session_id, &session.user_goal);
        run.task_id = session.id.clone();
        state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await
            .create_run(&run)
            .expect("create running AgentRun");
    }

    let detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &session.id,
        &state,
    )
    .await
    .expect("build active detail");

    assert_eq!(detail.evidence_view.lifecycle_state, "running");
    assert_eq!(detail.evidence_view.projection_state, "active");
    assert_eq!(detail.evidence_view.identity_state, "consistent");
    assert_eq!(detail.evidence_view.snapshot_state, "stable");
    assert_eq!(detail.evidence_view.durable_sequence_before, Some(0));
    assert_eq!(detail.evidence_view.durable_sequence_after, Some(0));
    assert_eq!(detail.allowed_controls, vec!["cancel", "open_trace"]);
}

#[test]
fn main_chat_task_controls_are_not_concentrated_in_lib_rs() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_task_controls.rs");

    assert!(
        source.contains("pub(crate) mod main_chat_task_controls;"),
        "Main Chat task-control commands must live in a focused non-test module"
    );
    assert!(
        module_path.is_file(),
        "Main Chat task-control module file must exist outside #[cfg(test)]"
    );
    let module_source = std::fs::read_to_string(&module_path).expect("read task-control module");
    assert!(
        module_source.contains("pub struct MainChatAgentTaskState"),
        "task-state response shape must move with the task-control commands"
    );
    assert!(
        module_source.contains("pub(crate) async fn resume_main_chat_agent_task("),
        "resume command implementation must be reusable outside src/lib.rs"
    );
    assert!(
        module_source.contains("pub(crate) async fn cancel_main_chat_agent_task("),
        "cancel command implementation must be reusable outside src/lib.rs"
    );
    assert!(
        module_source.contains("pub(crate) async fn retry_main_chat_agent_action("),
        "retry command implementation must be reusable outside src/lib.rs"
    );
    assert!(
        !source.contains("\nasync fn resume_main_chat_agent_task("),
        "resume command body must not remain concentrated in src/lib.rs"
    );
    assert!(
        !source.contains("\nasync fn cancel_main_chat_agent_task("),
        "cancel command body must not remain concentrated in src/lib.rs"
    );
    assert!(
        !source.contains("\nasync fn retry_main_chat_agent_action("),
        "retry command body must not remain concentrated in src/lib.rs"
    );
}

fn extract_rust_function_body(source: &str, signature: &str) -> String {
    let signature_start = source.find(signature).expect("function signature exists");
    let brace_start = source[signature_start..]
        .find('{')
        .map(|index| signature_start + index)
        .expect("function body starts");
    let mut depth = 0usize;

    for (offset, ch) in source[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = brace_start + offset + ch.len_utf8();
                    return source[brace_start..end].to_string();
                }
            }
            _ => {}
        }
    }

    panic!("function body closes");
}
