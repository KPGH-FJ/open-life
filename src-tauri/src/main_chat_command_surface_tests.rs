use crate::main_chat_final_acceptance_tests::run_main_chat_command_surface_eval_gate;

#[test]
fn main_chat_command_surface_ipc_tests_are_not_concentrated_in_lib_rs() {
    let lib_rs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");

    for forbidden in [
        "send_message_command_surface_runs_governed_proposal_path",
        "start_stream_message_command_surface_runs_governed_proposal_path",
        "send_message_direct_answer_records_main_chat_run_and_completes_task",
        "send_message_l2_direct_answer_records_scheduler_provider_generation_trace",
        "send_message_runtime_clock_weekday_uses_kernel_direct_reply_without_provider",
        "send_message_runtime_clock_does_not_capture_planning_question",
        "start_stream_message_direct_answer_records_main_chat_run_and_completes_task",
        "start_stream_message_l2_direct_answer_records_scheduler_provider_generation_trace",
        "send_message_command_surface_preserves_web_policy_blocker",
        "start_stream_message_command_surface_preserves_web_policy_blocker",
        "send_message_command_surface_preserves_missing_mcp_blocker",
        "start_stream_message_command_surface_preserves_missing_mcp_blocker",
        "send_message_command_surface_preserves_registered_mcp_read_success",
        "start_stream_message_command_surface_preserves_registered_mcp_read_success",
        "send_message_registered_mcp_read_completes_through_agent_loop_not_fallback",
        "start_stream_message_registered_mcp_read_completes_through_agent_loop_not_fallback",
        "send_message_web_policy_blocker_completes_through_agent_loop_not_fallback",
        "start_stream_message_web_policy_blocker_completes_through_agent_loop_not_fallback",
        "send_message_registered_mcp_multi_candidate_agent_loop_selects_allowed_manifest",
        "send_message_missing_workspace_file_source_records_kernel_blocked_read_evidence",
        "main_chat_kernel_goal_3_review_maturation_send_stream_returns_governed_blocker_without_legacy",
        "main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix",
    ] {
        assert!(
            !source.contains(&format!("\n    async fn {forbidden}(")),
            "command-surface IPC test {forbidden} should live outside src/lib.rs"
        );
    }
}

fn main_chat_invoke_request(cmd: &str, body: serde_json::Value) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: cmd.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body: tauri::ipc::InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: tauri::test::INVOKE_KEY.to_string(),
    }
}

fn main_chat_command_surface_test_context() -> tauri::Context<tauri::test::MockRuntime> {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    let mock_ipc_origin = tauri::utils::acl::ExecutionContext::Remote {
        url: "http://tauri.localhost"
            .parse()
            .expect("valid mock IPC origin pattern"),
    };
    context
        .runtime_authority_mut()
        .__allow_command("send_message".into(), mock_ipc_origin.clone());
    context
        .runtime_authority_mut()
        .__allow_command("start_stream_message".into(), mock_ipc_origin);
    context
}

async fn set_command_surface_scripted_generation_response(
    state: &std::sync::Arc<crate::AppState>,
    model: &str,
    response: serde_json::Value,
) {
    let mut scheduler = state.scheduler.lock().await;
    *scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        "https://example.invalid/v1".into(),
        "test-key".into(),
        model.into(),
        "text-embedding-test".into(),
        false,
    )
    .with_scripted_generation_response(response.to_string());
}

async fn invoke_send_message_for_kernel_goal_3(
    state: std::sync::Arc<crate::AppState>,
    session_id: &str,
    user_text: &str,
) -> serde_json::Value {
    let app = tauri::test::mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![crate::send_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");

    tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "send_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": [{ "role": "user", "content": user_text }]
            }),
        ),
    )
    .expect("send_message kernel Goal 3 response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize kernel Goal 3 send response")
}

async fn invoke_start_stream_message_for_kernel_goal_3(
    state: std::sync::Arc<crate::AppState>,
    session_id: &str,
    user_text: &str,
) {
    let app = tauri::test::mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![crate::start_stream_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "start_stream_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": messages,
                "args": {
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages
                }
            }),
        ),
    );
    assert!(
        response.is_ok(),
        "start_stream_message kernel Goal 3 failed: {response:?}"
    );
}

fn expected_task_session_id(session_id: &str, user_text: &str) -> String {
    openlife_core::agent::main_chat_agent_v1::AgentIngress::default()
        .decide(
            session_id,
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        )
        .agent_task_session_id
        .expect("expected task session id")
}

async fn load_command_surface_session(
    state: &std::sync::Arc<crate::AppState>,
    task_session_id: &str,
) -> openlife_core::agent::main_chat_agent_v1::AgentTaskSession {
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("main chat session store");
    let store = store_arc.lock().await;
    store
        .load_session(task_session_id)
        .expect("load task session")
        .expect("task session exists")
}

async fn list_command_surface_transcript(
    state: &std::sync::Arc<crate::AppState>,
    task_session_id: &str,
) -> Vec<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry> {
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("main chat session store");
    let store = store_arc.lock().await;
    store
        .list_transcript_entries(task_session_id)
        .expect("list task transcript")
}

async fn list_command_surface_actions(
    state: &std::sync::Arc<crate::AppState>,
    task_session_id: &str,
) -> Vec<openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction> {
    let queue_arc = state
        .main_chat_action_queue_store
        .as_ref()
        .expect("main chat action queue store");
    let queue = queue_arc.lock().await;
    queue
        .list_for_session(task_session_id)
        .expect("list task actions")
}

async fn wait_command_surface_session_blocker(
    state: &std::sync::Arc<crate::AppState>,
    task_session_id: &str,
    blocker_substring: &str,
) -> openlife_core::agent::main_chat_agent_v1::AgentTaskSession {
    for _ in 0..80 {
        let session = load_command_surface_session(state, task_session_id).await;
        if session
            .pending_blockers
            .iter()
            .any(|blocker| blocker.contains(blocker_substring))
        {
            return session;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    load_command_surface_session(state, task_session_id).await
}

async fn list_command_surface_proposals(
    state: &std::sync::Arc<crate::AppState>,
) -> Vec<openlife_core::agent::AgentProposal> {
    let proposal_arc = state.proposal_store.as_ref().expect("proposal store");
    let store = proposal_arc.lock().await;
    store
        .list_all_proposals(100, 0)
        .expect("list command-surface proposals")
}

async fn find_command_surface_proposal_for_task(
    state: &std::sync::Arc<crate::AppState>,
    task_session_id: &str,
    proposal_type: openlife_core::agent::ProposalType,
) -> openlife_core::agent::AgentProposal {
    list_command_surface_proposals(state)
        .await
        .into_iter()
        .find(|proposal| {
            proposal.source_detail.as_deref() == Some(task_session_id)
                && proposal.proposal_type == proposal_type
        })
        .expect("find task-linked proposal")
}

async fn active_memory_record_count(state: &std::sync::Arc<crate::AppState>) -> usize {
    let lifecycle_store = state
        .memory_lifecycle_store
        .as_ref()
        .expect("memory lifecycle store");
    let store = lifecycle_store.lock().await;
    store
        .list_active_records(None, 100)
        .expect("list active memory records")
        .len()
}

async fn seed_command_surface_message(
    state: &std::sync::Arc<crate::AppState>,
    session_id: &str,
    content: &str,
) {
    let memory_store = state.memory_store.lock().await;
    memory_store
        .save_message(
            session_id,
            &openlife_core::llm::ChatMessage {
                role: "user".into(),
                content: content.into(),
            },
        )
        .expect("seed command-surface message");
}

fn assert_kernel_goal_3_read_action_metadata(
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
    expected_source_kind: &str,
    expected_evidence_kind: &str,
    expected_status: openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus,
) {
    assert_eq!(action.status, expected_status);
    let metadata = action
        .observation_metadata
        .as_ref()
        .expect("kernel Goal 3 observation metadata");
    assert_eq!(
        metadata
            .get("kernelBackedReadOnlyToolLoop")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        metadata
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        metadata
            .get("sourceKind")
            .and_then(serde_json::Value::as_str),
        Some(expected_source_kind)
    );
    let read_evidence = metadata
        .get("structuredResult")
        .and_then(|value| value.get("readExecutionEvidence"))
        .expect("kernel Goal 3 read evidence");
    assert_eq!(
        read_evidence
            .get("kind")
            .and_then(serde_json::Value::as_str),
        Some(expected_evidence_kind)
    );
    assert_eq!(
        read_evidence
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
}

fn assert_kernel_read_loop_final_metadata(metadata: &serde_json::Value) {
    assert_eq!(
        metadata
            .get("kernelBackedReadOnlyToolLoop")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        metadata
            .get("toolCallCount")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        metadata
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
}

fn assert_kernel_mcp_read_selection_metadata(
    metadata: &serde_json::Value,
    min_candidate_count: usize,
) {
    assert_eq!(
        metadata
            .get("kernelBackedReadOnlyToolLoop")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        metadata
            .get("mcpReadTargetResolved")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        metadata
            .get("strictManifestIdentity")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        metadata
            .get("fuzzyNameMatchingUsed")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        metadata
            .get("toolSelectionModelRanked")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        metadata
            .get("toolSelectionRankingSource")
            .and_then(serde_json::Value::as_str),
        Some("deterministic_local")
    );
    assert_eq!(
        metadata
            .get("toolSelectionDeterministicFallbackReady")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        metadata
            .get("toolSelectionProviderRankingRequiredForLocalCompletion")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        metadata
            .get("selectedCandidateId")
            .and_then(serde_json::Value::as_str),
        Some("builtin_echo")
    );
    assert_eq!(
        metadata
            .get("selectedCandidateTarget")
            .and_then(serde_json::Value::as_str),
        Some("builtin_echo")
    );
    assert_eq!(
        metadata
            .get("selectedCandidateActionType")
            .and_then(serde_json::Value::as_str),
        Some("mcp_tool")
    );
    assert_eq!(
        metadata
            .get("manifestId")
            .and_then(serde_json::Value::as_str),
        Some("builtin_echo")
    );
    assert_eq!(
        metadata.get("target").and_then(serde_json::Value::as_str),
        Some("builtin_echo")
    );
    assert_eq!(
        metadata
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    let candidate_count = metadata
        .get("toolSelectionCandidateCount")
        .and_then(serde_json::Value::as_u64)
        .expect("kernel MCP candidate count") as usize;
    assert!(
        candidate_count >= min_candidate_count,
        "expected at least {min_candidate_count} candidates, got {candidate_count}"
    );
    let candidate_ids = metadata
        .get("boundedCandidateIds")
        .and_then(serde_json::Value::as_array)
        .expect("kernel MCP bounded candidate ids");
    assert_eq!(candidate_ids.len(), candidate_count);
    let selected_index = candidate_ids
        .iter()
        .position(|candidate| candidate == "builtin_echo")
        .expect("bounded candidates include builtin_echo");
    assert_eq!(
        metadata
            .get("selectedCandidateRank")
            .and_then(serde_json::Value::as_u64),
        Some((selected_index + 1) as u64)
    );
    let target_allowlist = metadata
        .get("targetAllowlist")
        .and_then(serde_json::Value::as_array)
        .expect("kernel MCP target allowlist");
    assert_eq!(target_allowlist.len(), candidate_count);
    assert!(target_allowlist
        .iter()
        .any(|target| target == "builtin_echo"));
    let action_target_allowlist = metadata
        .get("actionTargetAllowlist")
        .and_then(serde_json::Value::as_array)
        .expect("kernel MCP action-target allowlist");
    assert_eq!(action_target_allowlist.len(), candidate_count);
    assert!(action_target_allowlist.iter().all(|entry| {
        entry.as_object().is_some_and(|object| object.len() == 2)
            && entry.get("actionType").and_then(serde_json::Value::as_str) == Some("mcp_tool")
            && entry
                .get("target")
                .and_then(serde_json::Value::as_str)
                .is_some()
    }));
    assert!(action_target_allowlist.iter().any(|entry| {
        entry.get("actionType").and_then(serde_json::Value::as_str) == Some("mcp_tool")
            && entry.get("target").and_then(serde_json::Value::as_str) == Some("builtin_echo")
    }));
}

fn assert_kernel_web_network_blocker_metadata(metadata: &serde_json::Value) {
    assert_eq!(
        metadata
            .get("kernelBackedReadOnlyToolLoop")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        metadata
            .get("executorStatus")
            .and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    assert_eq!(
        metadata
            .get("blockerReason")
            .and_then(serde_json::Value::as_str),
        Some("network_policy_blocked")
    );
    assert_eq!(
        metadata
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
}

#[tokio::test]
async fn main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix() {
    let report = run_main_chat_command_surface_eval_gate().await;

    assert_eq!(report.failed_cases, 0, "{:?}", report.failures);
    assert!(report.total_cases >= 38);
    let two_case_coverage = 2.0 / report.total_cases as f32;
    assert!(report.send_coverage >= 0.45);
    assert!(report.stream_coverage >= 0.45);
    assert!(report.provider_generation_coverage >= two_case_coverage);
    assert!(report.file_read_coverage >= two_case_coverage);
    assert!(report.plan_execute_coverage >= two_case_coverage);
    assert!(report.proposal_coverage >= two_case_coverage);
    assert!(report.web_policy_blocker_coverage >= two_case_coverage);
    assert!(report.web_agent_loop_blocker_coverage >= two_case_coverage);
    assert!(report.web_agent_loop_success_coverage >= two_case_coverage);
    assert!(report.mcp_missing_read_target_blocker_coverage >= two_case_coverage);
    assert!(report.mcp_registered_read_success_coverage >= two_case_coverage);
    assert!(report.mcp_agent_loop_success_coverage >= two_case_coverage);
    assert!(report.mcp_tool_permission_proposal_coverage >= two_case_coverage);
    assert!(report.mcp_agent_loop_tool_permission_proposal_coverage >= two_case_coverage);
    assert_eq!(report.live_provider_generation_coverage, 0.0);
    assert_eq!(report.live_provider_web_mcp_agent_loop_coverage, 0.0);
    assert_eq!(report.live_provider_web_agent_loop_coverage, 0.0);
    assert_eq!(report.live_provider_mcp_agent_loop_coverage, 0.0);
    assert_eq!(report.live_provider_proposal_permission_coverage, 0.0);
    assert!(!report.final_completion_ready);
    assert!(report
        .final_completion_blockers
        .contains(&"live_provider_generation_not_executed".to_string()));
    assert!(report
        .final_completion_blockers
        .contains(&"provider_backed_web_mcp_agent_loop_not_executed".to_string()));
    assert!(report
        .final_completion_blockers
        .contains(&"provider_backed_web_agent_loop_not_executed".to_string()));
    assert!(report
        .final_completion_blockers
        .contains(&"provider_backed_mcp_agent_loop_not_executed".to_string()));
    assert!(report
        .final_completion_blockers
        .contains(&"provider_live_proposal_permission_not_executed".to_string()));
    assert_eq!(report.legacy_fallback_count, 0);
    assert_eq!(report.silent_write_count, 0);
    let missing_kernel_cases = report
        .case_evidence
        .iter()
        .filter(|case| !case.kernel_backed)
        .map(|case| {
            format!(
                "{}:{}",
                case.entry_point.as_label(),
                case.scenario.as_label()
            )
        })
        .collect::<Vec<_>>();
    assert!(
        missing_kernel_cases.is_empty(),
        "all command-surface eval cases must be MainChatKernel-backed: {:?}",
        missing_kernel_cases
    );
    assert_eq!(report.kernel_backed_case_count, report.total_cases);
    assert!(report.kernel_direct_answer_case_count > 0);
    assert!(report.kernel_read_only_tool_case_count > 0);
    assert!(report.kernel_proposal_write_case_count > 0);
    assert!(report.kernel_plan_execute_case_count > 0);
    assert!(report.kernel_blocker_case_count > 0);
    assert!(report.kernel_hs_context_case_count > 0);
    assert!(report.kernel_web_tool_case_count > 0);
    assert!(report.kernel_mcp_tool_case_count > 0);
    assert_eq!(
        report.acceptance_evidence().send_stream_matrix_coverage,
        1.0
    );
}

#[tokio::test]
async fn main_chat_kernel_goal_3_workspace_file_read_send_stream_records_observation() {
    let user_text = "Please read file `Cargo.toml`.";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let send_response =
        invoke_send_message_for_kernel_goal_3(send_state.clone(), "k3-send-file-read", user_text)
            .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(
        send_response["agent_ingress"]["selectedStrategy"],
        "re_act_tool_execution"
    );
    assert_eq!(
        send_response["tool_calls"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(send_response["tool_calls"][0]["name"], "file.read");
    assert_eq!(send_response["tool_calls"][0]["success"], true);
    assert!(send_response["reply"]
        .as_str()
        .is_some_and(|reply| reply.contains("openlife-core")));
    let generation = &send_response["reasoning_trace"]["generation_result"];
    assert_eq!(generation["selectedStrategy"], "react_tool_execution");
    assert_eq!(generation["kernelBackedReadOnlyToolLoop"], true);
    assert_eq!(generation["directWritesExecuted"], false);
    assert_eq!(generation["legacyFallbackUsed"], false);

    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send file task session id");
    let send_session = load_command_surface_session(&send_state, send_task_session_id).await;
    assert_eq!(
        send_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
    );
    assert!(send_session.pending_blockers.is_empty());
    let send_actions = list_command_surface_actions(&send_state, send_task_session_id).await;
    let send_file_action = send_actions
        .iter()
        .find(|action| action.action.action_type == "file.read")
        .expect("send file.read action");
    assert_kernel_goal_3_read_action_metadata(
        send_file_action,
        "file",
        "file_system_read",
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed,
    );

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k3-stream-file-read",
        user_text,
    )
    .await;
    let stream_task_session_id = expected_task_session_id("k3-stream-file-read", user_text);
    let stream_session = load_command_surface_session(&stream_state, &stream_task_session_id).await;
    assert_eq!(
        stream_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
    );
    let stream_actions = list_command_surface_actions(&stream_state, &stream_task_session_id).await;
    let stream_file_action = stream_actions
        .iter()
        .find(|action| action.action.action_type == "file.read")
        .expect("stream file.read action");
    assert_kernel_goal_3_read_action_metadata(
        stream_file_action,
        "file",
        "file_system_read",
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed,
    );
}

#[tokio::test]
async fn main_chat_kernel_goal_3_path_traversal_send_stream_blocks_filesystem_read() {
    let user_text = "Please read file `../AGENTS.md`.";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let send_response =
        invoke_send_message_for_kernel_goal_3(send_state.clone(), "k3-send-traversal", user_text)
            .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(
        send_response["tool_calls"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(send_response["tool_calls"][0]["name"], "file.read");
    assert_eq!(send_response["tool_calls"][0]["status"], "blocked");
    assert_eq!(
        send_response["tool_calls"][0]["error"],
        "filesystem_path_traversal_blocked"
    );
    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send traversal task session id");
    let send_session = load_command_surface_session(&send_state, send_task_session_id).await;
    assert_eq!(
        send_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(send_session
        .pending_blockers
        .contains(&"filesystem_path_traversal_blocked".to_string()));
    let send_actions = list_command_surface_actions(&send_state, send_task_session_id).await;
    let send_file_action = send_actions
        .iter()
        .find(|action| action.action.action_type == "file.read")
        .expect("send traversal file.read action");
    assert_kernel_goal_3_read_action_metadata(
        send_file_action,
        "file",
        "file_system_read",
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed,
    );
    assert_eq!(
        send_file_action
            .observation_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("stopReason"))
            .and_then(serde_json::Value::as_str),
        Some("filesystem_path_traversal_blocked")
    );

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k3-stream-traversal",
        user_text,
    )
    .await;
    let stream_task_session_id = expected_task_session_id("k3-stream-traversal", user_text);
    let stream_session = load_command_surface_session(&stream_state, &stream_task_session_id).await;
    assert_eq!(
        stream_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(stream_session
        .pending_blockers
        .contains(&"filesystem_path_traversal_blocked".to_string()));
    let stream_actions = list_command_surface_actions(&stream_state, &stream_task_session_id).await;
    assert!(stream_actions
        .iter()
        .any(|action| action.action.action_type == "file.read"
            && action.status
                == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed));
}

#[tokio::test]
async fn main_chat_kernel_goal_3_session_search_send_stream_uses_bounded_prior_context() {
    let user_text = "Find what we discussed about Agent memory.";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    seed_command_surface_message(
        &send_state,
        "prior-k3-session",
        "We discussed Agent memory needing source citations and bounded session search.",
    )
    .await;
    let send_response = invoke_send_message_for_kernel_goal_3(
        send_state.clone(),
        "k3-send-session-search",
        user_text,
    )
    .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(send_response["tool_calls"][0]["name"], "session.search");
    assert!(send_response["reply"]
        .as_str()
        .is_some_and(|reply| reply.contains("source citations")));
    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send session search task session id");
    let send_session = load_command_surface_session(&send_state, send_task_session_id).await;
    assert_eq!(
        send_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
    );
    let send_actions = list_command_surface_actions(&send_state, send_task_session_id).await;
    let send_session_action = send_actions
        .iter()
        .find(|action| action.action.action_type == "session.search")
        .expect("send session.search action");
    assert_kernel_goal_3_read_action_metadata(
        send_session_action,
        "session",
        "session_read",
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed,
    );
    let structured = send_session_action
        .observation_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("structuredResult"))
        .expect("session structured result");
    assert!(structured["hitCount"].as_u64().unwrap_or_default() > 0);
    assert_eq!(structured["promotedToMemory"], false);

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    seed_command_surface_message(
        &stream_state,
        "prior-k3-stream-session",
        "We discussed Agent memory needing source citations and bounded session search.",
    )
    .await;
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k3-stream-session-search",
        user_text,
    )
    .await;
    let stream_task_session_id = expected_task_session_id("k3-stream-session-search", user_text);
    let stream_session = load_command_surface_session(&stream_state, &stream_task_session_id).await;
    assert_eq!(
        stream_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
    );
    let stream_actions = list_command_surface_actions(&stream_state, &stream_task_session_id).await;
    let stream_session_action = stream_actions
        .iter()
        .find(|action| action.action.action_type == "session.search")
        .expect("stream session.search action");
    assert_kernel_goal_3_read_action_metadata(
        stream_session_action,
        "session",
        "session_read",
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed,
    );
}

#[tokio::test]
async fn main_chat_kernel_goal_3_memory_search_send_stream_is_read_only() {
    let user_text = "memory.search energy planning notes";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    seed_command_surface_message(
        &send_state,
        "prior-k3-memory-session",
        "Energy planning works best when tasks are batched before lunch.",
    )
    .await;
    let active_records_before = {
        let lifecycle_store = send_state
            .memory_lifecycle_store
            .as_ref()
            .expect("memory lifecycle store");
        let store = lifecycle_store.lock().await;
        store
            .list_active_records(None, 20)
            .expect("list active memory records")
            .len()
    };
    let send_response = invoke_send_message_for_kernel_goal_3(
        send_state.clone(),
        "k3-send-memory-search",
        user_text,
    )
    .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(send_response["tool_calls"][0]["name"], "memory.search");
    assert!(send_response["reply"]
        .as_str()
        .is_some_and(|reply| reply.contains("Energy planning")));
    let active_records_after = {
        let lifecycle_store = send_state
            .memory_lifecycle_store
            .as_ref()
            .expect("memory lifecycle store");
        let store = lifecycle_store.lock().await;
        store
            .list_active_records(None, 20)
            .expect("list active memory records")
            .len()
    };
    assert_eq!(active_records_before, active_records_after);
    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send memory search task session id");
    let send_actions = list_command_surface_actions(&send_state, send_task_session_id).await;
    let send_memory_action = send_actions
        .iter()
        .find(|action| action.action.action_type == "memory.search")
        .expect("send memory.search action");
    assert_kernel_goal_3_read_action_metadata(
        send_memory_action,
        "memory",
        "memory_read",
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed,
    );

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    seed_command_surface_message(
        &stream_state,
        "prior-k3-stream-memory-session",
        "Energy planning works best when tasks are batched before lunch.",
    )
    .await;
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k3-stream-memory-search",
        user_text,
    )
    .await;
    let stream_task_session_id = expected_task_session_id("k3-stream-memory-search", user_text);
    let stream_actions = list_command_surface_actions(&stream_state, &stream_task_session_id).await;
    let stream_memory_action = stream_actions
        .iter()
        .find(|action| action.action.action_type == "memory.search")
        .expect("stream memory.search action");
    assert_kernel_goal_3_read_action_metadata(
        stream_memory_action,
        "memory",
        "memory_read",
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed,
    );
}

#[tokio::test]
async fn main_chat_kernel_goal_4_remember_this_send_stream_creates_memory_proposal_only() {
    let user_text = "Remember this: I prefer short planning summaries.";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let memory_records_before = active_memory_record_count(&send_state).await;
    let send_response = invoke_send_message_for_kernel_goal_3(
        send_state.clone(),
        "k4-send-memory-proposal",
        user_text,
    )
    .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(
        send_response["agent_ingress"]["selectedStrategy"],
        "memory_proposal"
    );
    let generation = &send_response["reasoning_trace"]["generation_result"];
    assert_eq!(generation["kernelBackedProposalOnlyWrite"], true);
    assert_eq!(generation["writeOutcomeKind"], "memory_proposal");
    assert_eq!(generation["directWritesExecuted"], false);
    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send memory proposal task session id");
    let send_session = load_command_surface_session(&send_state, send_task_session_id).await;
    assert_eq!(
        send_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
    );
    assert!(send_session
        .pending_blockers
        .iter()
        .any(|blocker| blocker.starts_with("proposal:")));
    let proposal = find_command_surface_proposal_for_task(
        &send_state,
        send_task_session_id,
        openlife_core::agent::ProposalType::MemoryWrite,
    )
    .await;
    assert_eq!(
        proposal.status,
        openlife_core::agent::ProposalStatus::Pending
    );
    assert_eq!(
        proposal
            .after
            .get("directMemoryWrite")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        proposal
            .after
            .get("acceptedDurableTruthWritten")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        active_memory_record_count(&send_state).await,
        memory_records_before
    );
    let send_actions = list_command_surface_actions(&send_state, send_task_session_id).await;
    assert!(send_actions.iter().any(|action| {
        action.action.action_type == "proposal.create"
            && action.status
                == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
            && action
                .observation_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("proposalId"))
                .and_then(serde_json::Value::as_str)
                == Some(proposal.id.as_str())
    }));

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let stream_memory_records_before = active_memory_record_count(&stream_state).await;
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k4-stream-memory-proposal",
        user_text,
    )
    .await;
    let stream_task_session_id = expected_task_session_id("k4-stream-memory-proposal", user_text);
    let stream_session = load_command_surface_session(&stream_state, &stream_task_session_id).await;
    assert_eq!(
        stream_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
    );
    let stream_proposal = find_command_surface_proposal_for_task(
        &stream_state,
        &stream_task_session_id,
        openlife_core::agent::ProposalType::MemoryWrite,
    )
    .await;
    assert_eq!(
        stream_proposal.status,
        openlife_core::agent::ProposalStatus::Pending
    );
    assert_eq!(
        active_memory_record_count(&stream_state).await,
        stream_memory_records_before
    );
}

#[tokio::test]
async fn main_chat_kernel_goal_4_lifemodel_update_send_stream_creates_proposal_only() {
    let user_text = "Update my life model: I am switching careers toward design lead.";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let model_before = {
        let manager = send_state.life_model_manager.lock().await;
        manager.load().expect("load life model before")
    };
    let send_response = invoke_send_message_for_kernel_goal_3(
        send_state.clone(),
        "k4-send-lifemodel-proposal",
        user_text,
    )
    .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(
        send_response["agent_ingress"]["selectedStrategy"],
        "life_model_proposal"
    );
    assert_eq!(
        send_response["reasoning_trace"]["generation_result"]["writeOutcomeKind"],
        "lifemodel_proposal"
    );
    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send lifemodel proposal task session id");
    let proposal = find_command_surface_proposal_for_task(
        &send_state,
        send_task_session_id,
        openlife_core::agent::ProposalType::LifeModelUpdate,
    )
    .await;
    assert_eq!(
        proposal.status,
        openlife_core::agent::ProposalStatus::Pending
    );
    assert_eq!(
        proposal
            .after
            .get("directLifeModelWrite")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        proposal
            .after
            .get("acceptedDurableTruthWritten")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    let model_after = {
        let manager = send_state.life_model_manager.lock().await;
        manager.load().expect("load life model after")
    };
    assert_eq!(
        serde_json::to_value(&model_after).expect("serialize after"),
        serde_json::to_value(&model_before).expect("serialize before")
    );

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k4-stream-lifemodel-proposal",
        user_text,
    )
    .await;
    let stream_task_session_id =
        expected_task_session_id("k4-stream-lifemodel-proposal", user_text);
    let stream_session = load_command_surface_session(&stream_state, &stream_task_session_id).await;
    assert_eq!(
        stream_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
    );
    let stream_proposal = find_command_surface_proposal_for_task(
        &stream_state,
        &stream_task_session_id,
        openlife_core::agent::ProposalType::LifeModelUpdate,
    )
    .await;
    assert_eq!(
        stream_proposal.status,
        openlife_core::agent::ProposalStatus::Pending
    );
}

#[tokio::test]
async fn main_chat_kernel_goal_4_file_write_send_stream_creates_proposal_without_writing_file() {
    let proposed_path = std::env::temp_dir().join(format!(
        "openlife-k4-file-write-{}.txt",
        uuid::Uuid::new_v4()
    ));
    let proposed_path_text = proposed_path.display().to_string();
    let user_text = format!(
        "Write file `{}` with content `hello from k4`.",
        proposed_path_text
    );

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let send_response = invoke_send_message_for_kernel_goal_3(
        send_state.clone(),
        "k4-send-file-write-proposal",
        &user_text,
    )
    .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(
        send_response["reasoning_trace"]["generation_result"]["writeOutcomeKind"],
        "file_write_proposal"
    );
    assert!(
        !proposed_path.exists(),
        "kernel must not write proposed file"
    );
    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send file write task session id");
    let proposal = find_command_surface_proposal_for_task(
        &send_state,
        send_task_session_id,
        openlife_core::agent::ProposalType::ExternalWriteAction,
    )
    .await;
    assert_eq!(
        proposal.status,
        openlife_core::agent::ProposalStatus::Pending
    );
    assert_eq!(
        proposal
            .after
            .get("path")
            .and_then(serde_json::Value::as_str),
        Some(proposed_path_text.as_str())
    );
    assert_eq!(
        proposal
            .after
            .get("fileWritten")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        proposal
            .after
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k4-stream-file-write-proposal",
        &user_text,
    )
    .await;
    let stream_task_session_id =
        expected_task_session_id("k4-stream-file-write-proposal", &user_text);
    let stream_proposal = find_command_surface_proposal_for_task(
        &stream_state,
        &stream_task_session_id,
        openlife_core::agent::ProposalType::ExternalWriteAction,
    )
    .await;
    assert_eq!(
        stream_proposal.status,
        openlife_core::agent::ProposalStatus::Pending
    );
    assert!(
        !proposed_path.exists(),
        "stream kernel must not write proposed file"
    );
}

#[tokio::test]
async fn main_chat_kernel_goal_4_external_write_send_stream_requires_confirmation_only() {
    let user_text = "Send email to my coworker with this private update.";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let send_response = invoke_send_message_for_kernel_goal_3(
        send_state.clone(),
        "k4-send-external-confirmation",
        user_text,
    )
    .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(
        send_response["reasoning_trace"]["generation_result"]["writeOutcomeKind"],
        "external_confirmation_blocker"
    );
    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send external task session id");
    let send_session = load_command_surface_session(&send_state, send_task_session_id).await;
    assert_eq!(
        send_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
    );
    assert!(send_session
        .pending_blockers
        .contains(&"external_write_requires_confirmation".to_string()));
    assert!(list_command_surface_proposals(&send_state).await.is_empty());
    let send_actions = list_command_surface_actions(&send_state, send_task_session_id).await;
    let email_action = send_actions
        .iter()
        .find(|action| action.action.action_type == "email.send")
        .expect("email confirmation action");
    assert_eq!(
        email_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
    );
    assert!(email_action.policy.requires_confirmation);
    assert!(!email_action.policy.silent_write_allowed);

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k4-stream-external-confirmation",
        user_text,
    )
    .await;
    let stream_task_session_id =
        expected_task_session_id("k4-stream-external-confirmation", user_text);
    let stream_session = load_command_surface_session(&stream_state, &stream_task_session_id).await;
    assert_eq!(
        stream_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
    );
    assert!(list_command_surface_proposals(&stream_state)
        .await
        .is_empty());
}

#[tokio::test]
async fn main_chat_kernel_goal_4_calendar_and_generic_external_write_send_stream_require_confirmation_only(
) {
    for (suffix, user_text, expected_action_type) in [
        (
            "calendar",
            "Add calendar event for tomorrow with private planning notes.",
            "calendar.real_write",
        ),
        (
            "generic",
            "Post to provider workspace with this private update.",
            "external.write",
        ),
    ] {
        let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let send_session_id = format!("k4-send-{suffix}-confirmation");
        let send_response =
            invoke_send_message_for_kernel_goal_3(send_state.clone(), &send_session_id, user_text)
                .await;
        assert_eq!(send_response["legacy_fallback_used"], false);
        assert_eq!(
            send_response["reasoning_trace"]["generation_result"]["writeOutcomeKind"],
            "external_confirmation_blocker"
        );
        let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
            .as_str()
            .expect("send external task session id");
        let send_session = load_command_surface_session(&send_state, send_task_session_id).await;
        assert_eq!(
            send_session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        );
        assert!(send_session
            .pending_blockers
            .contains(&"external_write_requires_confirmation".to_string()));
        assert!(list_command_surface_proposals(&send_state).await.is_empty());
        let send_actions = list_command_surface_actions(&send_state, send_task_session_id).await;
        let external_action = send_actions
            .iter()
            .find(|action| action.action.action_type == expected_action_type)
            .expect("external confirmation action");
        assert_eq!(
            external_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
        );
        assert!(external_action.policy.requires_confirmation);
        assert!(!external_action.policy.silent_write_allowed);

        let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let stream_session_id = format!("k4-stream-{suffix}-confirmation");
        invoke_start_stream_message_for_kernel_goal_3(
            stream_state.clone(),
            &stream_session_id,
            user_text,
        )
        .await;
        let stream_task_session_id = expected_task_session_id(&stream_session_id, user_text);
        let stream_session =
            load_command_surface_session(&stream_state, &stream_task_session_id).await;
        assert_eq!(
            stream_session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        );
        assert!(stream_session
            .pending_blockers
            .contains(&"external_write_requires_confirmation".to_string()));
        assert!(list_command_surface_proposals(&stream_state)
            .await
            .is_empty());
        let stream_actions =
            list_command_surface_actions(&stream_state, &stream_task_session_id).await;
        assert!(stream_actions
            .iter()
            .any(|action| action.action.action_type == expected_action_type
                && action.status
                    == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission));
    }
}

#[tokio::test]
async fn main_chat_kernel_goal_4_dangerous_shell_send_stream_hard_blocks_without_proposal() {
    let user_text = "Run shell.destructive rm -rf to delete project files.";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let send_response = invoke_send_message_for_kernel_goal_3(
        send_state.clone(),
        "k4-send-dangerous-shell",
        user_text,
    )
    .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(
        send_response["reasoning_trace"]["generation_result"]["writeOutcomeKind"],
        "dangerous_hard_block"
    );
    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send dangerous task session id");
    let send_session = load_command_surface_session(&send_state, send_task_session_id).await;
    assert_eq!(
        send_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(send_session
        .pending_blockers
        .contains(&"dangerous_action_hard_block".to_string()));
    assert!(list_command_surface_proposals(&send_state).await.is_empty());
    let send_actions = list_command_surface_actions(&send_state, send_task_session_id).await;
    let shell_action = send_actions
        .iter()
        .find(|action| action.action.action_type == "shell.destructive")
        .expect("dangerous shell action");
    assert_eq!(
        shell_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
    );
    let metadata = shell_action
        .observation_metadata
        .as_ref()
        .expect("hard block metadata");
    assert_eq!(
        metadata
            .get("replayable")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        metadata
            .get("proposalCreated")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k4-stream-dangerous-shell",
        user_text,
    )
    .await;
    let stream_task_session_id = expected_task_session_id("k4-stream-dangerous-shell", user_text);
    let stream_session = load_command_surface_session(&stream_state, &stream_task_session_id).await;
    assert_eq!(
        stream_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(list_command_surface_proposals(&stream_state)
        .await
        .is_empty());
}

#[tokio::test]
async fn main_chat_kernel_goal_4_ordinary_auto_checkin_does_not_materialize_truth() {
    let user_text = "我今天完成了写周报";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    set_command_surface_scripted_generation_response(
        &send_state,
        "k4-direct-answer-model",
        serde_json::json!("已收到。"),
    )
    .await;
    {
        let mut model = openlife_core::life_model::LifeModel::default_model();
        model
            .goals
            .daily
            .push(openlife_core::life_model::DailyGoal {
                name: "写周报".into(),
                done: false,
                time_block: None,
            });
        let manager = send_state.life_model_manager.lock().await;
        manager.save(&model).expect("seed daily goal");
    }
    let response = invoke_send_message_for_kernel_goal_3(
        send_state.clone(),
        "k4-send-auto-checkin-isolation",
        user_text,
    )
    .await;
    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "direct_answer"
    );
    assert_eq!(
        response["reasoning_trace"]["generation_result"]["kernelBackedDirectAnswer"],
        true
    );
    let model_after = {
        let manager = send_state.life_model_manager.lock().await;
        manager.load().expect("load daily goal after")
    };
    assert_eq!(model_after.goals.daily.len(), 1);
    assert!(!model_after.goals.daily[0].done);
    assert!(list_command_surface_proposals(&send_state).await.is_empty());
}

#[tokio::test]
async fn main_chat_kernel_goal_3_web_read_unavailable_send_stream_blocks_without_fake_success() {
    let user_text = "Please run web.read unavailable for OpenLife release notes.";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = send_state.config.lock().await;
        config.system.network_policy.enabled = false;
    }
    let send_response = invoke_send_message_for_kernel_goal_3(
        send_state.clone(),
        "k3-send-web-unavailable",
        user_text,
    )
    .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(send_response["tool_calls"][0]["name"], "web.search");
    assert_eq!(send_response["tool_calls"][0]["status"], "blocked");
    assert_eq!(
        send_response["tool_calls"][0]["error"],
        "network_policy_blocked"
    );
    assert!(send_response["reply"]
        .as_str()
        .is_some_and(|reply| reply.contains("network_policy_blocked")));
    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send web task session id");
    let send_session = load_command_surface_session(&send_state, send_task_session_id).await;
    assert_eq!(
        send_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    let send_actions = list_command_surface_actions(&send_state, send_task_session_id).await;
    let send_web_action = send_actions
        .iter()
        .find(|action| action.action.action_type == "web.search")
        .expect("send web.search action");
    assert_kernel_goal_3_read_action_metadata(
        send_web_action,
        "web",
        "web_search_network",
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed,
    );

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = stream_state.config.lock().await;
        config.system.network_policy.enabled = false;
    }
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k3-stream-web-unavailable",
        user_text,
    )
    .await;
    let stream_task_session_id = expected_task_session_id("k3-stream-web-unavailable", user_text);
    let stream_session = load_command_surface_session(&stream_state, &stream_task_session_id).await;
    assert_eq!(
        stream_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(stream_session
        .pending_blockers
        .contains(&"network_policy_blocked".to_string()));
    let stream_actions = list_command_surface_actions(&stream_state, &stream_task_session_id).await;
    assert!(stream_actions
        .iter()
        .any(|action| action.action.action_type == "web.search"
            && action.status
                == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed));
}

#[tokio::test]
async fn main_chat_kernel_goal_3_unknown_tool_send_stream_blocks_without_fallback() {
    let user_text = "Please use unknown tool for this task.";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let send_response = invoke_send_message_for_kernel_goal_3(
        send_state.clone(),
        "k3-send-unknown-tool",
        user_text,
    )
    .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(
        send_response["agent_ingress"]["selectedStrategy"],
        "direct_answer"
    );
    assert_eq!(send_response["tool_calls"][0]["name"], "unsupported.tool");
    assert_eq!(send_response["tool_calls"][0]["status"], "blocked");
    assert_eq!(send_response["tool_calls"][0]["error"], "unsupported_tool");
    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send unknown task session id");
    let send_session = load_command_surface_session(&send_state, send_task_session_id).await;
    assert_eq!(
        send_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(send_session
        .pending_blockers
        .contains(&"unsupported_tool".to_string()));

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k3-stream-unknown-tool",
        user_text,
    )
    .await;
    let stream_task_session_id = expected_task_session_id("k3-stream-unknown-tool", user_text);
    let stream_session = load_command_surface_session(&stream_state, &stream_task_session_id).await;
    assert_eq!(
        stream_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(stream_session
        .pending_blockers
        .contains(&"unsupported_tool".to_string()));
    let stream_actions = list_command_surface_actions(&stream_state, &stream_task_session_id).await;
    assert!(stream_actions
        .iter()
        .any(|action| action.action.action_type == "unsupported.tool"
            && action.status
                == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed));
}

#[tokio::test]
async fn main_chat_kernel_goal_3_review_maturation_send_stream_returns_governed_blocker_without_legacy(
) {
    let user_text = "Review what changed in my working style this month.";
    let expected_blocker = "review_maturation_kernel_executor_unavailable";

    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let send_response = invoke_send_message_for_kernel_goal_3(
        send_state.clone(),
        "k3-send-review-maturation",
        user_text,
    )
    .await;
    assert_eq!(send_response["legacy_fallback_used"], false);
    assert_eq!(
        send_response["agent_ingress"]["selectedStrategy"],
        "review_maturation"
    );
    assert!(send_response["tool_calls"]
        .as_array()
        .is_some_and(Vec::is_empty));
    assert!(send_response["reply"]
        .as_str()
        .is_some_and(|reply| reply.contains(expected_blocker)));
    let generation = &send_response["reasoning_trace"]["generation_result"];
    assert_eq!(generation["selectedStrategy"], "review_maturation");
    assert_eq!(generation["legacyFallbackUsed"], false);
    assert_eq!(generation["directWritesExecuted"], false);
    assert_eq!(generation["kernelBackedGovernedBlocker"], true);
    assert_eq!(generation["kernelSupportDisposition"], "governed_blocker");
    assert!(generation["blockers"]
        .as_array()
        .is_some_and(|blockers| blockers.iter().any(|blocker| blocker == expected_blocker)));
    let send_task_session_id = send_response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("send review task session id");
    let send_session = load_command_surface_session(&send_state, send_task_session_id).await;
    assert_eq!(
        send_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(send_session
        .pending_blockers
        .contains(&expected_blocker.to_string()));
    let send_actions = list_command_surface_actions(&send_state, send_task_session_id).await;
    assert!(send_actions.is_empty());
    let send_transcript = list_command_surface_transcript(&send_state, send_task_session_id).await;
    assert!(send_transcript.iter().any(|entry| {
        entry
            .metadata
            .get("kernelBackedGovernedBlocker")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
            && entry
                .metadata
                .get("kernelSupportDisposition")
                .and_then(serde_json::Value::as_str)
                == Some("governed_blocker")
    }));
    assert!(!send_transcript.iter().any(|entry| {
        entry.kind
            == openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Fallback
    }));

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    invoke_start_stream_message_for_kernel_goal_3(
        stream_state.clone(),
        "k3-stream-review-maturation",
        user_text,
    )
    .await;
    let stream_task_session_id = expected_task_session_id("k3-stream-review-maturation", user_text);
    let stream_session = load_command_surface_session(&stream_state, &stream_task_session_id).await;
    assert_eq!(
        stream_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(stream_session
        .pending_blockers
        .contains(&expected_blocker.to_string()));
    let stream_actions = list_command_surface_actions(&stream_state, &stream_task_session_id).await;
    assert!(stream_actions.is_empty());
    let stream_transcript =
        list_command_surface_transcript(&stream_state, &stream_task_session_id).await;
    assert!(stream_transcript.iter().any(|entry| {
        entry
            .metadata
            .get("kernelBackedGovernedBlocker")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
            && entry
                .metadata
                .get("kernelSupportDisposition")
                .and_then(serde_json::Value::as_str)
                == Some("governed_blocker")
    }));
    assert!(!stream_transcript.iter().any(|entry| {
        entry.kind
            == openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Fallback
    }));
}

#[tokio::test]
async fn send_message_missing_workspace_file_source_records_kernel_blocked_read_evidence() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::send_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-missing-workspace-file-source";

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "send_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": [{
                    "role": "user",
                    "content": "Read frontend/definitely-missing-stage2-file.md before answering."
                }]
            }),
        ),
    )
    .expect("send_message missing file response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize missing file response");

    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "re_act_tool_execution"
    );
    assert!(
        response["reply"]
            .as_str()
            .is_some_and(|reply| reply.contains("filesystem_read_blocked")),
        "missing file response: {response:#}"
    );
    assert_eq!(
        response["reasoning_trace"]["generation_result"]["kernelBackedReadOnlyToolLoop"],
        true
    );
    assert_eq!(
        response["reasoning_trace"]["generation_result"]["legacyFallbackUsed"],
        false
    );
    let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("missing file task session id");

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load missing file task session")
            .expect("missing file task session exists")
    };
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert_eq!(
        session.pending_blockers,
        vec!["filesystem_read_blocked".to_string()]
    );

    let actions = list_command_surface_actions(&state, task_session_id).await;
    let file_action = actions
        .iter()
        .find(|action| action.action.action_type == "file.read")
        .expect("missing file.read blocked action");
    assert_eq!(
        file_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
    );
    let action_metadata = file_action
        .observation_metadata
        .as_ref()
        .expect("missing file blocked observation metadata");
    assert_eq!(
        action_metadata
            .get("kernelBackedReadOnlyToolLoop")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        action_metadata
            .get("blockerReason")
            .and_then(serde_json::Value::as_str),
        Some("filesystem_read_blocked")
    );
    assert_eq!(
        action_metadata
            .get("legacyFallbackUsed")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    let transcript = list_command_surface_transcript(&state, task_session_id).await;
    let error_entry = transcript
        .iter()
        .find(|entry| {
            entry
                .summary
                .contains("MainChatKernel read-only tool loop returned a blocker")
        })
        .expect("missing file blocker transcript entry");
    assert_eq!(
        error_entry
            .metadata
            .get("kernelBackedReadOnlyToolLoop")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(error_entry
        .metadata
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|blockers| blockers
            .iter()
            .any(|blocker| blocker == "filesystem_read_blocked")));
    assert_eq!(
        error_entry
            .metadata
            .get("legacyFallbackUsed")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
}

#[tokio::test]
async fn send_message_command_surface_runs_governed_proposal_path() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::send_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-send-proposal";

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "send_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": [
                    {
                        "role": "user",
                        "content": "Please remember that I prefer morning writing blocks."
                    }
                ]
            }),
        ),
    )
    .expect("send_message response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize send_message response");

    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "memory_proposal"
    );
    let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("agent task session id");
    assert!(response["execution_transcript"]
        .as_array()
        .expect("transcript array")
        .iter()
        .any(|entry| entry["kind"] == "proposal_request"));

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load task session")
            .expect("task session exists")
    };
    assert_eq!(session.chat_session_id, session_id);
    assert_eq!(session.selected_strategy.as_str(), "memory_proposal");
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
    );
    assert!(session
        .pending_blockers
        .iter()
        .any(|blocker| blocker.starts_with("proposal:")));

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list command actions")
    };
    let proposal_action = actions
        .iter()
        .find(|action| action.action.action_type == "proposal.create")
        .expect("proposal create action");
    assert_eq!(
        proposal_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
    );
    assert_eq!(
        proposal_action.policy.level,
        openlife_core::agent::main_chat_agent_v1::MainChatPolicyLevel::L1GovernedProposalCreate
    );
    assert!(proposal_action.policy.execution_allowed);
    assert!(!proposal_action.policy.requires_proposal);
    assert!(!proposal_action.policy.silent_write_allowed);

    let proposals = {
        let proposal_arc = state.proposal_store.as_ref().expect("proposal store");
        let proposal_store = proposal_arc.lock().await;
        proposal_store
            .list_pending_proposals(10)
            .expect("list pending proposals")
    };
    assert!(proposals.iter().any(|proposal| {
        proposal.source == openlife_core::agent::ProposalSource::ChatConversation
            && matches!(
                proposal.source_detail.as_deref(),
                Some(detail)
                    if detail == task_session_id
                        || detail == format!("main_chat_agent_task_session:{task_session_id}")
            )
    }));
}

#[tokio::test]
async fn start_stream_message_command_surface_runs_governed_proposal_path() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::start_stream_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-stream-proposal";
    let messages = serde_json::json!([
        {
            "role": "user",
            "content": "Please remember that I prefer async writing review on Fridays."
        }
    ]);

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "start_stream_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": messages,
                "args": {
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages
                }
            }),
        ),
    );
    assert!(response.is_ok(), "stream command failed: {response:?}");

    let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
    let decision = ingress.decide(
        session_id,
        "Please remember that I prefer async writing review on Fridays.",
        None,
        openlife_core::agent::AgentTaskKind::Conversation,
    );
    let task_session_id = decision
        .agent_task_session_id
        .as_deref()
        .expect("expected stream task session id");

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load stream task session")
            .expect("stream task session exists")
    };
    assert_eq!(session.chat_session_id, session_id);
    assert_eq!(session.selected_strategy.as_str(), "memory_proposal");
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
    );

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list stream command actions")
    };
    let proposal_action = actions
        .iter()
        .find(|action| action.action.action_type == "proposal.create")
        .expect("stream proposal create action");
    assert_eq!(
        proposal_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
    );
    assert_eq!(
        proposal_action.policy.level,
        openlife_core::agent::main_chat_agent_v1::MainChatPolicyLevel::L1GovernedProposalCreate
    );
    assert!(!proposal_action.policy.silent_write_allowed);

    let proposals = {
        let proposal_arc = state.proposal_store.as_ref().expect("proposal store");
        let proposal_store = proposal_arc.lock().await;
        proposal_store
            .list_pending_proposals(10)
            .expect("list stream pending proposals")
    };
    assert!(proposals.iter().any(|proposal| {
        proposal.source == openlife_core::agent::ProposalSource::ChatConversation
            && matches!(
                proposal.source_detail.as_deref(),
                Some(detail)
                    if detail == task_session_id
                        || detail == format!("main_chat_agent_task_session:{task_session_id}")
            )
    }));
}

#[tokio::test]
async fn send_message_direct_answer_records_main_chat_run_and_completes_task() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::send_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-direct-answer";
    let user_text = "hello";

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "send_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": [{ "role": "user", "content": user_text }]
            }),
        ),
    )
    .expect("send_message direct response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize direct response");

    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(response["tool_calls"].as_array().map(Vec::len), Some(0));
    let generation = response["reasoning_trace"]["generation_result"]
        .as_object()
        .expect("direct answer generation result");
    assert_eq!(
        generation
            .get("kernelBackedDirectAnswer")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        generation
            .get("kernelEventSink")
            .and_then(serde_json::Value::as_str),
        Some("buffered")
    );
    assert!(
        generation
            .get("kernelEventCount")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default()
            > 0
    );
    assert_eq!(
        generation
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "direct_answer"
    );
    let run_id = response["run_id"].as_str().expect("direct answer run id");
    let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("direct answer task session id");

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load direct answer task session")
            .expect("direct answer task session exists")
    };
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
    );
    assert_eq!(session.selected_strategy.as_str(), "direct_answer");
    assert!(session.pending_blockers.is_empty());

    let run = {
        let run_store_arc = state.agent_run_store.as_ref().expect("agent run store");
        let run_store = run_store_arc.lock().await;
        run_store
            .get_run(run_id)
            .expect("get direct answer run")
            .expect("direct answer run exists")
    };
    assert_eq!(run.status, openlife_core::agent::AgentRunStatus::Completed);
    assert_eq!(
        run.reasoning_strategy.as_deref(),
        Some("main_chat_agent_v1_direct_answer")
    );
    assert_eq!(
        run.model_route
            .as_ref()
            .map(|route| route.route_type.as_str()),
        Some("direct")
    );
    assert_eq!(run.tool_call_count, 0);

    let transcript = response["execution_transcript"]
        .as_array()
        .expect("direct answer transcript");
    assert!(transcript.iter().any(|entry| {
        entry["summary"]
            .as_str()
            .is_some_and(|summary| summary.contains("DirectAnswer prompt contract"))
    }));
    assert!(transcript.iter().any(|entry| {
        entry["summary"]
            .as_str()
            .is_some_and(|summary| summary.contains("Bounded context"))
    }));
    assert!(transcript.iter().any(|entry| {
        entry["summary"]
            .as_str()
            .is_some_and(|summary| summary.contains("DirectAnswer completed"))
    }));
}

#[tokio::test]
async fn send_message_l2_direct_answer_records_scheduler_provider_generation_trace() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "gpt-provider-trace".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("scripted provider-backed direct answer");
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::send_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-direct-answer-provider-trace";
    let user_text = "Explain focused work in one concise paragraph for a teammate.";

    let ingress_decision = openlife_core::agent::main_chat_agent_v1::AgentIngress::default()
        .decide(
            session_id,
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
    assert_eq!(
        ingress_decision.selected_strategy,
        openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer
    );

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "send_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": [{ "role": "user", "content": user_text }]
            }),
        ),
    )
    .expect("send_message provider-backed direct response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize provider-backed direct response");

    assert_eq!(response["reply"], "scripted provider-backed direct answer");
    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "direct_answer"
    );
    let run_id = response["run_id"]
        .as_str()
        .expect("provider-backed direct answer run id");

    let generation = response["reasoning_trace"]["generation_result"]
        .as_object()
        .expect("generation result metadata");
    assert_eq!(
        generation
            .get("providerGenerationPath")
            .and_then(serde_json::Value::as_str),
        Some("main_chat_direct_answer_scheduler")
    );
    assert_eq!(
        generation
            .get("kernelBackedDirectAnswer")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        generation
            .get("kernelEventSink")
            .and_then(serde_json::Value::as_str),
        Some("buffered")
    );
    assert_eq!(
        generation
            .get("modelGenerated")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        generation
            .get("provider")
            .and_then(serde_json::Value::as_str),
        Some("openai")
    );
    assert_eq!(
        generation.get("model").and_then(serde_json::Value::as_str),
        Some("gpt-provider-trace")
    );
    assert_eq!(
        generation
            .get("routeType")
            .and_then(serde_json::Value::as_str),
        Some("cloud")
    );
    assert_eq!(
        generation
            .get("legacyFallbackUsed")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    let run = {
        let run_store_arc = state.agent_run_store.as_ref().expect("agent run store");
        let run_store = run_store_arc.lock().await;
        run_store
            .get_run(run_id)
            .expect("get provider-backed direct answer run")
            .expect("provider-backed direct answer run exists")
    };
    let model_route = run
        .model_route
        .as_ref()
        .expect("provider-backed model route");
    assert_eq!(model_route.provider, "openai");
    assert_eq!(model_route.model, "gpt-provider-trace");
    assert_eq!(model_route.route_type, "cloud");
    assert_eq!(
        run.reasoning_strategy.as_deref(),
        Some("main_chat_agent_v1_direct_answer")
    );

    let transcript = response["execution_transcript"]
        .as_array()
        .expect("provider-backed direct answer transcript");
    let generation_entry = transcript
        .iter()
        .find(|entry| {
            entry["summary"]
                .as_str()
                .is_some_and(|summary| summary.contains("DirectAnswer generated a model response"))
        })
        .expect("provider generation transcript entry");
    assert_eq!(
        generation_entry["metadata"]["providerGenerationPath"].as_str(),
        Some("main_chat_direct_answer_scheduler")
    );
    assert_eq!(
        generation_entry["metadata"]["provider"].as_str(),
        Some("openai")
    );
    assert_eq!(
        generation_entry["metadata"]["model"].as_str(),
        Some("gpt-provider-trace")
    );
    assert_eq!(
        generation_entry["metadata"]["routeType"].as_str(),
        Some("cloud")
    );
    assert_eq!(
        generation_entry["metadata"]["directWritesExecuted"].as_bool(),
        Some(false)
    );
}

#[tokio::test]
async fn send_message_runtime_clock_weekday_uses_kernel_direct_reply_without_provider() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    set_command_surface_scripted_generation_response(
        &state,
        "provider-should-not-answer-clock",
        serde_json::json!("provider should not be used for runtime clock"),
    )
    .await;

    let response = invoke_send_message_for_kernel_goal_3(
        state,
        "command-surface-runtime-clock-weekday",
        "今天星期几",
    )
    .await;

    let reply = response["reply"].as_str().expect("runtime clock reply");
    assert!(reply.contains("根据本机运行时钟"));
    assert!(reply.contains("星期"));
    assert_ne!(reply, "provider should not be used for runtime clock");
    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "direct_answer"
    );

    let generation = response["reasoning_trace"]["generation_result"]
        .as_object()
        .expect("runtime clock generation metadata");
    assert_eq!(
        generation
            .get("kernelBackedDirectAnswer")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        generation
            .get("modelGenerated")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        generation
            .get("schedulerGenerationCalled")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        generation
            .get("providerGenerationPath")
            .and_then(serde_json::Value::as_str),
        Some(crate::main_chat_runtime_facts::RUNTIME_FACT_PROVIDER_GENERATION_PATH)
    );
    assert_eq!(
        generation
            .get("sourceType")
            .and_then(serde_json::Value::as_str),
        Some(crate::main_chat_runtime_facts::RUNTIME_FACT_SOURCE_TYPE)
    );
    assert!(generation
        .get("runtimeFactKeys")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|keys| keys
            .iter()
            .any(|key| key.as_str()
                == Some(crate::main_chat_runtime_facts::RUNTIME_FACT_KEY_WEEKDAY))));
    assert_eq!(
        generation
            .get("runtimeFactAuthority")
            .and_then(serde_json::Value::as_str),
        Some("runtime")
    );
    assert_eq!(
        generation
            .get("toolCalled")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    let transcript = response["execution_transcript"]
        .as_array()
        .expect("runtime clock transcript");
    assert!(transcript.iter().any(|entry| {
        entry["summary"].as_str().is_some_and(|summary| {
            summary.contains("local deterministic response without provider generation")
        })
    }));
}

#[tokio::test]
async fn send_message_runtime_clock_does_not_capture_planning_question() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    set_command_surface_scripted_generation_response(
        &state,
        "provider-should-answer-planning",
        serde_json::json!("provider handled the planning question"),
    )
    .await;

    let response = invoke_send_message_for_kernel_goal_3(
        state,
        "command-surface-runtime-clock-negative-planning",
        "What time should I leave tomorrow?",
    )
    .await;

    let reply = response["reply"].as_str().expect("planning reply");
    assert!(reply.contains("provider handled the planning question"));
    assert!(!reply.contains("根据本机运行时钟"));
    assert_eq!(response["legacy_fallback_used"], false);

    let generation = response["reasoning_trace"]["generation_result"]
        .as_object()
        .expect("planning generation metadata");
    assert_eq!(
        generation
            .get("modelGenerated")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        generation
            .get("schedulerGenerationCalled")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        generation
            .get("providerGenerationPath")
            .and_then(serde_json::Value::as_str),
        Some("main_chat_direct_answer_scheduler")
    );
}

#[tokio::test]
async fn start_stream_message_direct_answer_records_main_chat_run_and_completes_task() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::start_stream_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-stream-direct-answer";
    let user_text = "hello";
    let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "start_stream_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": messages,
                "args": {
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages
                }
            }),
        ),
    );
    assert!(
        response.is_ok(),
        "stream direct answer failed: {response:?}"
    );

    let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
    let decision = ingress.decide(
        session_id,
        user_text,
        None,
        openlife_core::agent::AgentTaskKind::Conversation,
    );
    let task_session_id = decision
        .agent_task_session_id
        .as_deref()
        .expect("expected stream direct answer task session id");

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load stream direct answer task session")
            .expect("stream direct answer task session exists")
    };
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
    );
    assert_eq!(session.selected_strategy.as_str(), "direct_answer");
    assert!(session.pending_blockers.is_empty());

    let runs = {
        let run_store_arc = state.agent_run_store.as_ref().expect("agent run store");
        let run_store = run_store_arc.lock().await;
        run_store
            .list_runs_for_session(session_id, 10)
            .expect("list stream direct answer runs")
    };
    let run = runs
        .iter()
        .find(|run| run.reasoning_strategy.as_deref() == Some("main_chat_agent_v1_direct_answer"))
        .expect("stream direct answer main chat run");
    assert_eq!(run.status, openlife_core::agent::AgentRunStatus::Completed);
    assert_eq!(
        run.model_route
            .as_ref()
            .map(|route| route.route_type.as_str()),
        Some("direct")
    );
    assert_eq!(run.tool_call_count, 0);
    let generation = run
        .reasoning_trace
        .as_ref()
        .and_then(|trace| trace.generation_result.as_ref())
        .and_then(serde_json::Value::as_object)
        .expect("stream direct answer generation metadata");
    assert_eq!(
        generation
            .get("kernelBackedDirectAnswer")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        generation
            .get("kernelEventSink")
            .and_then(serde_json::Value::as_str),
        Some("streaming")
    );
    assert!(
        generation
            .get("kernelEventCount")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default()
            > 0
    );
    assert_eq!(
        generation
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    let transcript = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .expect("list stream direct answer transcript")
    };
    assert!(transcript
        .iter()
        .any(|entry| entry.summary.contains("DirectAnswer prompt contract")));
    assert!(transcript
        .iter()
        .any(|entry| entry.summary.contains("Bounded context")));
    assert!(transcript
        .iter()
        .any(|entry| entry.summary.contains("DirectAnswer completed")));
}

#[tokio::test]
async fn start_stream_message_l2_direct_answer_records_scheduler_provider_generation_trace() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "gpt-stream-provider-trace".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("scripted stream provider direct answer");
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::start_stream_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-stream-direct-answer-provider-trace";
    let user_text = "Explain focused work in one concise paragraph for a teammate.";
    let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "start_stream_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": messages,
                "args": {
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages
                }
            }),
        ),
    );
    assert!(
        response.is_ok(),
        "stream provider-backed direct answer failed: {response:?}"
    );

    let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
    let decision = ingress.decide(
        session_id,
        user_text,
        None,
        openlife_core::agent::AgentTaskKind::Conversation,
    );
    assert_eq!(
        decision.selected_strategy,
        openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer
    );
    let task_session_id = decision
        .agent_task_session_id
        .as_deref()
        .expect("expected stream provider direct answer task session id");

    let run = {
        let run_store_arc = state.agent_run_store.as_ref().expect("agent run store");
        let run_store = run_store_arc.lock().await;
        run_store
            .list_runs_for_session(session_id, 10)
            .expect("list stream provider direct answer runs")
            .into_iter()
            .find(|run| {
                run.reasoning_strategy.as_deref() == Some("main_chat_agent_v1_direct_answer")
            })
            .expect("stream provider direct answer main chat run")
    };
    assert_eq!(run.status, openlife_core::agent::AgentRunStatus::Completed);
    let model_route = run
        .model_route
        .as_ref()
        .expect("stream provider-backed model route");
    assert_eq!(model_route.provider, "openai");
    assert_eq!(model_route.model, "gpt-stream-provider-trace");
    assert_eq!(model_route.route_type, "cloud");
    let generation = run
        .reasoning_trace
        .as_ref()
        .and_then(|trace| trace.generation_result.as_ref())
        .and_then(serde_json::Value::as_object)
        .expect("stream provider generation trace");
    assert_eq!(
        generation
            .get("providerGenerationPath")
            .and_then(serde_json::Value::as_str),
        Some("main_chat_direct_answer_scheduler")
    );
    assert_eq!(
        generation
            .get("kernelBackedDirectAnswer")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        generation
            .get("kernelEventSink")
            .and_then(serde_json::Value::as_str),
        Some("streaming")
    );
    assert_eq!(
        generation
            .get("provider")
            .and_then(serde_json::Value::as_str),
        Some("openai")
    );
    assert_eq!(
        generation.get("model").and_then(serde_json::Value::as_str),
        Some("gpt-stream-provider-trace")
    );
    assert_eq!(
        generation
            .get("routeType")
            .and_then(serde_json::Value::as_str),
        Some("cloud")
    );
    assert_eq!(
        generation
            .get("legacyFallbackUsed")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    let transcript = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .expect("list stream provider direct answer transcript")
    };
    let generation_entry = transcript
        .iter()
        .find(|entry| {
            entry
                .summary
                .contains("DirectAnswer generated a model response")
        })
        .expect("stream provider generation transcript entry");
    assert_eq!(
        generation_entry
            .metadata
            .get("providerGenerationPath")
            .and_then(serde_json::Value::as_str),
        Some("main_chat_direct_answer_scheduler")
    );
    assert_eq!(
        generation_entry
            .metadata
            .get("provider")
            .and_then(serde_json::Value::as_str),
        Some("openai")
    );
    assert_eq!(
        generation_entry
            .metadata
            .get("model")
            .and_then(serde_json::Value::as_str),
        Some("gpt-stream-provider-trace")
    );
    assert_eq!(
        generation_entry
            .metadata
            .get("routeType")
            .and_then(serde_json::Value::as_str),
        Some("cloud")
    );
    assert_eq!(
        generation_entry
            .metadata
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
}

#[tokio::test]
async fn main_chat_kernel_direct_answer_send_stream_success_metadata_parity() {
    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    for state in [&send_state, &stream_state] {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "gpt-kernel-parity".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("kernel parity direct answer");
    }
    let messages = vec![openlife_core::llm::ChatMessage {
        role: "user".into(),
        content: "Explain focused work in one concise paragraph for a teammate.".into(),
    }];

    let send_result = crate::main_chat_send::send_message_with_state(
        "command-surface-kernel-parity-send".into(),
        messages.clone(),
        None,
        &send_state,
    )
    .await
    .expect("send kernel parity result");
    let send_value = serde_json::to_value(&send_result).expect("serialize send parity result");

    let mut emitted_events = Vec::<(String, serde_json::Value)>::new();
    crate::main_chat_streaming::start_stream_message_with_state(
        "command-surface-kernel-parity-stream".into(),
        messages,
        None,
        &stream_state,
        |event, payload| emitted_events.push((event.to_string(), payload)),
    )
    .await
    .expect("stream kernel parity result");
    let stream_done = emitted_events
        .iter()
        .rev()
        .find(|(event, _)| event == "stream-message-done")
        .map(|(_, payload)| payload)
        .expect("stream parity done event");

    assert_eq!(send_value["reply"], stream_done["reply"]);
    assert_eq!(send_value["legacy_fallback_used"], false);
    assert_eq!(stream_done["legacy_fallback_used"], false);
    let send_generation = &send_value["reasoning_trace"]["generation_result"];
    let stream_generation = &stream_done["reasoning_trace"]["generation_result"];
    for key in [
        "providerGenerationPath",
        "provider",
        "model",
        "routeType",
        "kernelBackedDirectAnswer",
        "directWritesExecuted",
        "legacyFallbackUsed",
        "modelGenerated",
        "schedulerGenerationCalled",
    ] {
        assert_eq!(
            send_generation.get(key),
            stream_generation.get(key),
            "send/stream direct-answer metadata mismatch for {key}"
        );
    }
    assert_eq!(send_generation["provider"], "openai");
    assert_eq!(send_generation["model"], "gpt-kernel-parity");
    assert_eq!(send_generation["routeType"], "cloud");
    assert_eq!(send_generation["kernelBackedDirectAnswer"], true);
    assert_eq!(send_generation["directWritesExecuted"], false);
}

#[tokio::test]
async fn main_chat_kernel_direct_answer_invalid_input_blocks_send_and_stream_with_same_metadata() {
    let send_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let send_result = crate::main_chat_send::send_message_with_state(
        "   ".into(),
        vec![openlife_core::llm::ChatMessage {
            role: "user".into(),
            content: "Hello from an invalid session.".into(),
        }],
        None,
        &send_state,
    )
    .await
    .expect("send invalid direct answer result");
    let send_generation = send_result
        .reasoning_trace
        .generation_result
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .expect("send invalid generation metadata");
    assert_eq!(
        send_generation
            .get("kernelBackedDirectAnswer")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        send_generation
            .get("kernelEventSink")
            .and_then(serde_json::Value::as_str),
        Some("buffered")
    );
    assert_eq!(
        send_generation
            .get("modelGenerated")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        send_generation
            .get("schedulerGenerationCalled")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        send_generation
            .get("blockers")
            .and_then(serde_json::Value::as_array)
            .and_then(|blockers| blockers.first())
            .and_then(serde_json::Value::as_str),
        Some("invalid_session_id")
    );
    assert_eq!(send_result.legacy_fallback_used, false);
    let send_task_session_id = send_result
        .agent_ingress
        .as_ref()
        .and_then(|decision| decision.agent_task_session_id.as_deref())
        .expect("send invalid task session id");
    let send_session = {
        let store_arc = send_state
            .main_chat_agent_session_store
            .as_ref()
            .expect("send invalid session store");
        let store = store_arc.lock().await;
        store
            .load_session(send_task_session_id)
            .expect("load send invalid session")
            .expect("send invalid session exists")
    };
    assert_eq!(
        send_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert_eq!(send_session.pending_blockers, vec!["invalid_session_id"]);

    let stream_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let mut emitted_events = Vec::<(String, serde_json::Value)>::new();
    crate::main_chat_streaming::start_stream_message_with_state(
        "   ".into(),
        vec![openlife_core::llm::ChatMessage {
            role: "user".into(),
            content: "Hello from an invalid session.".into(),
        }],
        None,
        &stream_state,
        |event, payload| emitted_events.push((event.to_string(), payload)),
    )
    .await
    .expect("stream invalid direct answer result");
    assert!(emitted_events.iter().any(|(event, payload)| {
        event == "main-chat-kernel-event"
            && payload["type"] == "blocker"
            && payload["code"] == "invalid_session_id"
    }));
    let stream_done = emitted_events
        .iter()
        .rev()
        .find(|(event, _)| event == "stream-message-done")
        .map(|(_, payload)| payload)
        .expect("stream invalid done event");
    let stream_generation = stream_done["reasoning_trace"]["generation_result"]
        .as_object()
        .expect("stream invalid generation metadata");
    assert_eq!(
        stream_generation
            .get("kernelBackedDirectAnswer")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        stream_generation
            .get("kernelEventSink")
            .and_then(serde_json::Value::as_str),
        Some("streaming")
    );
    assert_eq!(
        stream_generation
            .get("blockers")
            .and_then(serde_json::Value::as_array)
            .and_then(|blockers| blockers.first())
            .and_then(serde_json::Value::as_str),
        Some("invalid_session_id")
    );
    assert_eq!(stream_done["legacy_fallback_used"], false);
    let stream_task_session_id = stream_done["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("stream invalid task session id");
    let stream_session = {
        let store_arc = stream_state
            .main_chat_agent_session_store
            .as_ref()
            .expect("stream invalid session store");
        let store = store_arc.lock().await;
        store
            .load_session(stream_task_session_id)
            .expect("load stream invalid session")
            .expect("stream invalid session exists")
    };
    assert_eq!(
        stream_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert_eq!(stream_session.pending_blockers, vec!["invalid_session_id"]);
}

#[tokio::test]
async fn send_message_command_surface_preserves_web_policy_blocker() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        config.system.network_policy.enabled = false;
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::send_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-web-blocker";
    let user_text = "Please web search OpenLife release notes.";

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "send_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": [{ "role": "user", "content": user_text }]
            }),
        ),
    )
    .expect("send_message web blocker response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize web blocker response");

    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "re_act_tool_execution"
    );
    let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("web blocker task session id");

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load web blocker task session")
            .expect("web blocker task session exists")
    };
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(session
        .pending_blockers
        .iter()
        .any(|blocker| blocker.contains("network_policy_blocked")));

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list web blocker actions")
    };
    let web_action = actions
        .iter()
        .find(|action| action.action.action_type == "web.search")
        .expect("web search action");
    assert_eq!(
        web_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
    );
    assert_eq!(
        web_action
            .observation_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("structuredResult"))
            .and_then(|value| value.get("network_policy_blocked"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[tokio::test]
async fn start_stream_message_command_surface_preserves_web_policy_blocker() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        config.system.network_policy.enabled = false;
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::start_stream_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-stream-web-blocker";
    let user_text = "Please web search OpenLife release notes.";
    let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "start_stream_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": messages,
                "args": {
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages
                }
            }),
        ),
    );
    assert!(response.is_ok(), "stream web blocker failed: {response:?}");

    let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
    let decision = ingress.decide(
        session_id,
        user_text,
        None,
        openlife_core::agent::AgentTaskKind::Conversation,
    );
    let task_session_id = decision
        .agent_task_session_id
        .as_deref()
        .expect("expected stream web blocker task session id");

    let session =
        wait_command_surface_session_blocker(&state, task_session_id, "network_policy_blocked")
            .await;
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(session
        .pending_blockers
        .iter()
        .any(|blocker| blocker.contains("network_policy_blocked")));

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list stream web blocker actions")
    };
    let web_action = actions
        .iter()
        .find(|action| action.action.action_type == "web.search")
        .expect("stream web search action");
    assert_eq!(
        web_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
    );
    assert_eq!(
        web_action
            .observation_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("structuredResult"))
            .and_then(|value| value.get("network_policy_blocked"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[tokio::test]
async fn send_message_command_surface_preserves_missing_mcp_blocker() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    set_command_surface_scripted_generation_response(
        &state,
        "gpt-command-surface-mcp-missing-fallback",
        serde_json::json!({
            "final": "I cannot complete the requested MCP read without a governed observation.",
            "actions": [],
            "thought_summary": "No governed observation was executed.",
            "warnings": []
        }),
    )
    .await;
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::send_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-mcp-blocker";
    let user_text = "Use mcp missing.status read-only now.";

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "send_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": [{ "role": "user", "content": user_text }]
            }),
        ),
    )
    .expect("send_message mcp blocker response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize mcp blocker response");

    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "re_act_tool_execution"
    );
    let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("mcp blocker task session id");

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load mcp blocker task session")
            .expect("mcp blocker task session exists")
    };
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(session
        .pending_blockers
        .iter()
        .any(|blocker| blocker.contains("mcp_read_tool_not_registered")));

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list mcp blocker actions")
    };
    let mcp_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .expect("mcp read action");
    assert_eq!(
        mcp_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
    );
    assert_eq!(
        mcp_action
            .observation_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("blockerReason"))
            .and_then(serde_json::Value::as_str),
        Some("mcp_read_tool_not_registered")
    );
}

#[tokio::test]
async fn start_stream_message_command_surface_preserves_missing_mcp_blocker() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    set_command_surface_scripted_generation_response(
        &state,
        "gpt-command-surface-stream-mcp-missing-fallback",
        serde_json::json!({
            "final": "I cannot complete the requested MCP read without a governed observation.",
            "actions": [],
            "thought_summary": "No governed observation was executed.",
            "warnings": []
        }),
    )
    .await;
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::start_stream_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-stream-mcp-blocker";
    let user_text = "Use mcp missing.status read-only now.";
    let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "start_stream_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": messages,
                "args": {
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages
                }
            }),
        ),
    );
    assert!(response.is_ok(), "stream mcp blocker failed: {response:?}");

    let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
    let decision = ingress.decide(
        session_id,
        user_text,
        None,
        openlife_core::agent::AgentTaskKind::Conversation,
    );
    let task_session_id = decision
        .agent_task_session_id
        .as_deref()
        .expect("expected stream mcp blocker task session id");

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load stream mcp blocker task session")
            .expect("stream mcp blocker task session exists")
    };
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(session
        .pending_blockers
        .iter()
        .any(|blocker| blocker.contains("mcp_read_tool_not_registered")));

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list stream mcp blocker actions")
    };
    let mcp_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .expect("stream mcp read action");
    assert_eq!(
        mcp_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
    );
    assert_eq!(
        mcp_action
            .observation_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("blockerReason"))
            .and_then(serde_json::Value::as_str),
        Some("mcp_read_tool_not_registered")
    );
}

#[tokio::test]
async fn send_message_command_surface_preserves_registered_mcp_read_success() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    set_command_surface_scripted_generation_response(
        &state,
        "gpt-command-surface-mcp-read-fallback",
        serde_json::json!({
            "final": "I can answer without a tool.",
            "actions": [],
            "thought_summary": "No governed observation yet.",
            "warnings": []
        }),
    )
    .await;
    {
        let store = state.tool_permission_store.lock().await;
        store
            .grant(
                "builtin_echo",
                "builtin",
                "low",
                "read",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                None,
            )
            .expect("grant builtin echo permission");
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::send_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-mcp-success";
    let user_text = "Use mcp builtin_echo read-only now.";

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "send_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": [{ "role": "user", "content": user_text }]
            }),
        ),
    )
    .expect("send_message mcp success response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize mcp success response");

    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "re_act_tool_execution"
    );
    let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("mcp success task session id");

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load mcp success task session")
            .expect("mcp success task session exists")
    };
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
    );
    assert!(session.pending_blockers.is_empty());

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list mcp success actions")
    };
    let mcp_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .expect("mcp read action");
    assert_eq!(
        mcp_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
    );
    let metadata = mcp_action
        .observation_metadata
        .as_ref()
        .expect("mcp read observation metadata");
    assert_eq!(metadata["target"], serde_json::json!("builtin_echo"));
    assert_eq!(
        metadata["requestedTarget"],
        serde_json::json!("mcp.call_tool")
    );
    assert_eq!(metadata["mcpReadTargetResolved"], serde_json::json!(true));
    assert_eq!(metadata["executorStatus"], serde_json::json!("succeeded"));
    assert_eq!(metadata["directWritesExecuted"], serde_json::json!(false));
    assert_eq!(
        metadata["structuredResult"]["directWritesExecuted"],
        serde_json::json!(false)
    );
}

#[tokio::test]
async fn start_stream_message_command_surface_preserves_registered_mcp_read_success() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    set_command_surface_scripted_generation_response(
        &state,
        "gpt-command-surface-stream-mcp-read-fallback",
        serde_json::json!({
            "final": "I can answer without a tool.",
            "actions": [],
            "thought_summary": "No governed observation yet.",
            "warnings": []
        }),
    )
    .await;
    {
        let store = state.tool_permission_store.lock().await;
        store
            .grant(
                "builtin_echo",
                "builtin",
                "low",
                "read",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                None,
            )
            .expect("grant builtin echo permission");
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::start_stream_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-stream-mcp-success";
    let user_text = "Use mcp builtin_echo read-only now.";
    let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "start_stream_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": messages,
                "args": {
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages
                }
            }),
        ),
    );
    assert!(response.is_ok(), "stream mcp success failed: {response:?}");

    let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
    let decision = ingress.decide(
        session_id,
        user_text,
        None,
        openlife_core::agent::AgentTaskKind::Conversation,
    );
    let task_session_id = decision
        .agent_task_session_id
        .as_deref()
        .expect("expected stream mcp success task session id");

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load stream mcp success task session")
            .expect("stream mcp success task session exists")
    };
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
    );
    assert!(session.pending_blockers.is_empty());

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list stream mcp success actions")
    };
    let mcp_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .expect("stream mcp read action");
    assert_eq!(
        mcp_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
    );
    let metadata = mcp_action
        .observation_metadata
        .as_ref()
        .expect("stream mcp read observation metadata");
    assert_eq!(metadata["target"], serde_json::json!("builtin_echo"));
    assert_eq!(
        metadata["requestedTarget"],
        serde_json::json!("mcp.call_tool")
    );
    assert_eq!(metadata["mcpReadTargetResolved"], serde_json::json!(true));
    assert_eq!(metadata["executorStatus"], serde_json::json!("succeeded"));
    assert_eq!(metadata["directWritesExecuted"], serde_json::json!(false));
    assert_eq!(
        metadata["structuredResult"]["directWritesExecuted"],
        serde_json::json!(false)
    );
}

#[tokio::test]
async fn send_message_registered_mcp_read_completes_through_agent_loop_not_fallback() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let store = state.tool_permission_store.lock().await;
        store
            .grant(
                "builtin_echo",
                "builtin",
                "low",
                "read",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                None,
            )
            .expect("grant builtin echo permission");
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "gpt-react-mcp-loop".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response(
            serde_json::json!({
                "final": "I will run the registered MCP read first.",
                "actions": [{
                    "name": "builtin_echo",
                    "action_type": "mcp_tool",
                    "arguments": {}
                }],
                "thought_summary": "Need a governed read-only MCP observation.",
                "warnings": []
            })
            .to_string(),
        );
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::send_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-mcp-agent-loop-success";
    let user_text = "Use mcp builtin_echo read-only now.";

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "send_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": [{ "role": "user", "content": user_text }]
            }),
        ),
    )
    .expect("send_message mcp AgentLoop response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize mcp AgentLoop response");

    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "re_act_tool_execution"
    );
    let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("mcp AgentLoop task session id");

    let transcript = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .expect("list mcp AgentLoop transcript")
    };
    let completed_entry = transcript
        .iter()
        .find(|entry| {
            entry.summary.contains("Governed ReAct AgentLoop completed")
                || entry
                    .summary
                    .contains("MainChatKernel read-only tool loop completed")
        })
        .expect("mcp AgentLoop completion transcript entry");
    if completed_entry
        .metadata
        .get("kernelBackedReadOnlyToolLoop")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        assert_kernel_read_loop_final_metadata(&completed_entry.metadata);
    } else {
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("plannedActionObserved")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("mcpReadTargetResolved")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("resolvedTarget")
                .and_then(serde_json::Value::as_str),
            Some("builtin_echo")
        );
    }

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list mcp AgentLoop actions")
    };
    let mcp_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .expect("mcp read action");
    assert_eq!(
        mcp_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
    );
    let observation = mcp_action
        .observation_metadata
        .as_ref()
        .expect("mcp AgentLoop observation metadata");
    if observation
        .get("kernelBackedReadOnlyToolLoop")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        assert_eq!(
            observation["executorStatus"],
            serde_json::json!("succeeded")
        );
        assert_kernel_mcp_read_selection_metadata(observation, 1);
    } else {
        assert_eq!(observation["agentLoopSucceeded"], serde_json::json!(true));
        assert_eq!(
            observation["singleStepFallbackUsed"],
            serde_json::json!(false)
        );
        assert_eq!(
            observation["mcpReadTargetResolved"],
            serde_json::json!(true)
        );
        assert_eq!(
            observation["resolvedTarget"],
            serde_json::json!("builtin_echo")
        );
    }
    assert_eq!(
        observation["directWritesExecuted"],
        serde_json::json!(false)
    );
}

#[tokio::test]
async fn start_stream_message_registered_mcp_read_completes_through_agent_loop_not_fallback() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let store = state.tool_permission_store.lock().await;
        store
            .grant(
                "builtin_echo",
                "builtin",
                "low",
                "read",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                None,
            )
            .expect("grant builtin echo permission");
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "gpt-react-mcp-loop-stream".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response(
            serde_json::json!({
                "final": "I will run the registered MCP read first.",
                "actions": [{
                    "name": "builtin_echo",
                    "action_type": "mcp_tool",
                    "arguments": {}
                }],
                "thought_summary": "Need a governed read-only MCP observation.",
                "warnings": []
            })
            .to_string(),
        );
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::start_stream_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-stream-mcp-agent-loop-success";
    let user_text = "Use mcp builtin_echo read-only now.";
    let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "start_stream_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": messages,
                "args": {
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages
                }
            }),
        ),
    );
    assert!(
        response.is_ok(),
        "stream mcp AgentLoop success failed: {response:?}"
    );

    let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
    let decision = ingress.decide(
        session_id,
        user_text,
        None,
        openlife_core::agent::AgentTaskKind::Conversation,
    );
    let task_session_id = decision
        .agent_task_session_id
        .as_deref()
        .expect("expected stream mcp AgentLoop task session id");

    let transcript = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .expect("list stream mcp AgentLoop transcript")
    };
    let completed_entry = transcript
        .iter()
        .find(|entry| {
            entry.summary.contains("Governed ReAct AgentLoop completed")
                || entry
                    .summary
                    .contains("MainChatKernel read-only tool loop completed")
        })
        .expect("stream mcp AgentLoop completion transcript entry");
    if completed_entry
        .metadata
        .get("kernelBackedReadOnlyToolLoop")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        assert_kernel_read_loop_final_metadata(&completed_entry.metadata);
    } else {
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("mcpReadTargetResolved")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("resolvedTarget")
                .and_then(serde_json::Value::as_str),
            Some("builtin_echo")
        );
    }

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list stream mcp AgentLoop actions")
    };
    let mcp_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .expect("stream mcp read action");
    assert_eq!(
        mcp_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
    );
    let observation = mcp_action
        .observation_metadata
        .as_ref()
        .expect("stream mcp AgentLoop observation metadata");
    if observation
        .get("kernelBackedReadOnlyToolLoop")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        assert_eq!(
            observation["executorStatus"],
            serde_json::json!("succeeded")
        );
        assert_kernel_mcp_read_selection_metadata(observation, 1);
    } else {
        assert_eq!(observation["agentLoopSucceeded"], serde_json::json!(true));
        assert_eq!(
            observation["singleStepFallbackUsed"],
            serde_json::json!(false)
        );
        assert_eq!(
            observation["mcpReadTargetResolved"],
            serde_json::json!(true)
        );
        assert_eq!(
            observation["resolvedTarget"],
            serde_json::json!("builtin_echo")
        );
    }
    assert_eq!(
        observation["directWritesExecuted"],
        serde_json::json!(false)
    );
}

#[tokio::test]
async fn send_message_registered_mcp_multi_candidate_agent_loop_selects_allowed_manifest() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let store = state.tool_permission_store.lock().await;
        store
            .grant(
                "builtin_echo",
                "builtin",
                "low",
                "read",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                None,
            )
            .expect("grant builtin echo permission");
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "gpt-react-mcp-loop-multi-candidate".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response(
            serde_json::json!({
                "final": "I will run one allowed registered MCP read first.",
                "actions": [{
                    "name": "builtin_echo",
                    "action_type": "mcp_tool",
                    "arguments": {
                        "text": "multi candidate selected"
                    }
                }],
                "thought_summary": "Select one governed read-only manifest from the candidate set.",
                "warnings": []
            })
            .to_string(),
        );
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::send_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-mcp-agent-loop-multi-candidate";
    let user_text = "Use an mcp read-only utility tool now.";

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "send_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": [{ "role": "user", "content": user_text }]
            }),
        ),
    )
    .expect("send_message mcp multi-candidate AgentLoop response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize mcp multi-candidate AgentLoop response");

    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "re_act_tool_execution"
    );
    let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("mcp multi-candidate AgentLoop task session id");

    let completed_entry = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .expect("list mcp multi-candidate AgentLoop transcript")
            .into_iter()
            .find(|entry| {
                entry.summary.contains("Governed ReAct AgentLoop completed")
                    || entry
                        .summary
                        .contains("MainChatKernel read-only tool loop completed")
            })
            .expect("mcp multi-candidate AgentLoop completion transcript entry")
    };
    let metadata = completed_entry.metadata;
    if metadata
        .get("kernelBackedReadOnlyToolLoop")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        assert_kernel_read_loop_final_metadata(&metadata);
    } else {
        let candidate_count = metadata
            .get("toolSelectionCandidateCount")
            .and_then(serde_json::Value::as_u64)
            .expect("candidate count metadata");
        assert!(
            candidate_count >= 2,
            "AgentLoop completion metadata must preserve the multi-candidate contract"
        );
        let candidate_ids = metadata
            .get("toolSelectionCandidateIds")
            .and_then(serde_json::Value::as_array)
            .expect("candidate ids metadata");
        assert!(candidate_ids
            .iter()
            .any(|candidate| candidate == "builtin_echo"));
        assert_eq!(
            metadata
                .get("toolSelectionCandidateId")
                .and_then(serde_json::Value::as_str),
            Some("builtin_echo")
        );
        assert_eq!(
            metadata
                .get("toolSelectionCandidateTarget")
                .and_then(serde_json::Value::as_str),
            Some("builtin_echo")
        );
        assert_eq!(
            metadata
                .get("modelSelectedAllowedTool")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            metadata
                .get("directWritesExecuted")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
    }

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list mcp multi-candidate AgentLoop actions")
    };
    let mcp_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .expect("mcp read action");
    assert_eq!(
        mcp_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
    );
    let observation = mcp_action
        .observation_metadata
        .as_ref()
        .expect("mcp multi-candidate AgentLoop observation metadata");
    if observation
        .get("kernelBackedReadOnlyToolLoop")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        assert_eq!(
            observation["executorStatus"],
            serde_json::json!("succeeded")
        );
        assert_kernel_mcp_read_selection_metadata(observation, 2);
    } else {
        assert_eq!(observation["agentLoopSucceeded"], serde_json::json!(true));
        assert_eq!(
            observation["toolSelectionCandidateId"],
            serde_json::json!("builtin_echo")
        );
        assert_eq!(
            observation["toolSelectionCandidateTarget"],
            serde_json::json!("builtin_echo")
        );
        assert_eq!(
            observation["singleStepFallbackUsed"],
            serde_json::json!(false)
        );
    }
    assert_eq!(
        observation["directWritesExecuted"],
        serde_json::json!(false)
    );
}

#[tokio::test]
async fn send_message_web_policy_blocker_completes_through_agent_loop_not_fallback() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        config.system.network_policy.enabled = false;
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "gpt-react-web-blocker-loop".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response(
            serde_json::json!({
                "final": "I will run the governed web read first.",
                "actions": [{
                    "name": "web.search",
                    "action_type": "mcp_tool",
                    "arguments": {
                        "query": "OpenLife release notes",
                        "max_results": 3
                    }
                }],
                "thought_summary": "Need a governed network-policy checked web observation.",
                "warnings": []
            })
            .to_string(),
        );
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::send_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-web-agent-loop-blocker";
    let user_text = "Please web search OpenLife release notes.";

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "send_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": [{ "role": "user", "content": user_text }]
            }),
        ),
    )
    .expect("send_message web AgentLoop blocker response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize web AgentLoop blocker response");

    assert_eq!(response["legacy_fallback_used"], false);
    assert_eq!(
        response["agent_ingress"]["selectedStrategy"],
        "re_act_tool_execution"
    );
    let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
        .as_str()
        .expect("web AgentLoop blocker task session id");

    let transcript = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .expect("list web AgentLoop transcript")
    };
    let completed_entry = transcript
        .iter()
        .find(|entry| {
            entry.summary.contains("Governed ReAct AgentLoop completed")
                || entry
                    .summary
                    .contains("MainChatKernel read-only tool loop returned a blocker")
        })
        .expect("web AgentLoop completion transcript entry");
    if completed_entry
        .metadata
        .get("kernelBackedReadOnlyToolLoop")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        assert_kernel_read_loop_final_metadata(&completed_entry.metadata);
    } else {
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopActionStatus")
                .and_then(serde_json::Value::as_str),
            Some("blocked")
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("permissionDecision")
                .and_then(serde_json::Value::as_str),
            Some("network_policy_blocked")
        );
    }

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load web AgentLoop blocker task session")
            .expect("web AgentLoop blocker task session exists")
    };
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(session
        .pending_blockers
        .iter()
        .any(|blocker| blocker.contains("network_policy_blocked")));

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list web AgentLoop blocker actions")
    };
    let web_action = actions
        .iter()
        .find(|action| action.action.action_type == "web.search")
        .expect("web search action");
    assert_eq!(
        web_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
    );
    let observation = web_action
        .observation_metadata
        .as_ref()
        .expect("web AgentLoop observation metadata");
    if observation
        .get("kernelBackedReadOnlyToolLoop")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        assert_kernel_web_network_blocker_metadata(observation);
    } else {
        assert_eq!(observation["agentLoopSucceeded"], serde_json::json!(true));
        assert_eq!(
            observation["singleStepFallbackUsed"],
            serde_json::json!(false)
        );
        assert_eq!(
            observation["agentLoopActionStatus"],
            serde_json::json!("blocked")
        );
        assert_eq!(
            observation["permissionDecision"],
            serde_json::json!("network_policy_blocked")
        );
    }
    assert_eq!(
        observation["directWritesExecuted"],
        serde_json::json!(false)
    );
}

#[tokio::test]
async fn start_stream_message_web_policy_blocker_completes_through_agent_loop_not_fallback() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        config.system.network_policy.enabled = false;
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "gpt-react-web-blocker-loop-stream".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response(
            serde_json::json!({
                "final": "I will run the governed web read first.",
                "actions": [{
                    "name": "web.search",
                    "action_type": "mcp_tool",
                    "arguments": {
                        "query": "OpenLife release notes",
                        "max_results": 3
                    }
                }],
                "thought_summary": "Need a governed network-policy checked web observation.",
                "warnings": []
            })
            .to_string(),
        );
    }
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![crate::start_stream_message])
        .build(main_chat_command_surface_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    let session_id = "command-surface-stream-web-agent-loop-blocker";
    let user_text = "Please web search OpenLife release notes.";
    let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

    let response = tauri::test::get_ipc_response(
        &webview,
        main_chat_invoke_request(
            "start_stream_message",
            serde_json::json!({
                "sessionId": session_id,
                "session_id": session_id,
                "messages": messages,
                "args": {
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages
                }
            }),
        ),
    );
    assert!(
        response.is_ok(),
        "stream web AgentLoop blocker failed: {response:?}"
    );

    let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
    let decision = ingress.decide(
        session_id,
        user_text,
        None,
        openlife_core::agent::AgentTaskKind::Conversation,
    );
    let task_session_id = decision
        .agent_task_session_id
        .as_deref()
        .expect("expected stream web AgentLoop blocker task session id");

    let transcript = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .expect("list stream web AgentLoop transcript")
    };
    let completed_entry = transcript
        .iter()
        .find(|entry| {
            entry.summary.contains("Governed ReAct AgentLoop completed")
                || entry
                    .summary
                    .contains("MainChatKernel read-only tool loop returned a blocker")
        })
        .expect("stream web AgentLoop completion transcript entry");
    if completed_entry
        .metadata
        .get("kernelBackedReadOnlyToolLoop")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        assert_kernel_read_loop_final_metadata(&completed_entry.metadata);
    } else {
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopActionStatus")
                .and_then(serde_json::Value::as_str),
            Some("blocked")
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("permissionDecision")
                .and_then(serde_json::Value::as_str),
            Some("network_policy_blocked")
        );
    }

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load stream web AgentLoop blocker task session")
            .expect("stream web AgentLoop blocker task session exists")
    };
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(session
        .pending_blockers
        .iter()
        .any(|blocker| blocker.contains("network_policy_blocked")));

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list stream web AgentLoop blocker actions")
    };
    let web_action = actions
        .iter()
        .find(|action| action.action.action_type == "web.search")
        .expect("stream web search action");
    assert_eq!(
        web_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
    );
    let observation = web_action
        .observation_metadata
        .as_ref()
        .expect("stream web AgentLoop observation metadata");
    if observation
        .get("kernelBackedReadOnlyToolLoop")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        assert_kernel_web_network_blocker_metadata(observation);
    } else {
        assert_eq!(observation["agentLoopSucceeded"], serde_json::json!(true));
        assert_eq!(
            observation["singleStepFallbackUsed"],
            serde_json::json!(false)
        );
        assert_eq!(
            observation["agentLoopActionStatus"],
            serde_json::json!("blocked")
        );
        assert_eq!(
            observation["permissionDecision"],
            serde_json::json!("network_policy_blocked")
        );
    }
    assert_eq!(
        observation["directWritesExecuted"],
        serde_json::json!(false)
    );
}
