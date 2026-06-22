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
        "send_message_missing_workspace_file_source_blocks_before_queue_execution",
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
}

#[tokio::test]
async fn send_message_missing_workspace_file_source_blocks_before_queue_execution() {
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
    assert!(response["reply"]
        .as_str()
        .is_some_and(|reply| reply.contains("missing or unreadable")));
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
        vec!["workspace_file_read_source_missing".to_string()]
    );

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list missing file actions")
    };
    assert!(
        actions.is_empty(),
        "missing file plan preparation should block before queue execution: {actions:?}"
    );

    let transcript = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .expect("list missing file transcript")
    };
    let error_entry = transcript
        .iter()
        .find(|entry| entry.summary == "ReAct tool action was blocked before execution.")
        .expect("missing file blocker transcript entry");
    assert_eq!(
        error_entry
            .metadata
            .get("blockerReason")
            .and_then(serde_json::Value::as_str),
        Some("workspace_file_read_source_missing")
    );
    assert_eq!(
        error_entry
            .metadata
            .get("sourceMissing")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
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
            && proposal.source_detail.as_deref()
                == Some(format!("main_chat_agent_task_session:{task_session_id}").as_str())
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
            && proposal.source_detail.as_deref()
                == Some(format!("main_chat_agent_task_session:{task_session_id}").as_str())
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

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load stream web blocker task session")
            .expect("stream web blocker task session exists")
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
        .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
        .expect("mcp AgentLoop completion transcript entry");
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
        .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
        .expect("stream mcp AgentLoop completion transcript entry");
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
            .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
            .expect("mcp multi-candidate AgentLoop completion transcript entry")
    };
    let metadata = completed_entry.metadata;
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
        .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
        .expect("web AgentLoop completion transcript entry");
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
        .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
        .expect("stream web AgentLoop completion transcript entry");
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
    assert_eq!(
        observation["directWritesExecuted"],
        serde_json::json!(false)
    );
}
