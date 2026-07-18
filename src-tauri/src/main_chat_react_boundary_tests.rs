use openlife_core::llm::ChatMessage;

use crate::main_chat_react_tool_selection::{
    build_main_chat_react_action_plan, main_chat_react_agent_loop_execution_plan,
    rank_main_chat_react_tool_candidates_with_authorization,
    rank_main_chat_react_tool_candidates_with_model, MainChatReactActionPlan,
    MainChatReactToolCandidate,
};
use crate::main_chat_send::send_message_with_state;

#[tokio::test]
async fn main_chat_react_registered_mcp_agent_loop_blocks_disallowed_model_tool_without_fallback() {
    for case in [
        ("unknown_target", "file.write", "mcp_tool"),
        ("write_like_action_type", "builtin_echo", "memory_write"),
        ("unsupported_action_type", "builtin_echo", "shell_exec"),
        ("wrong_read_action_type", "builtin_echo", "session_search"),
    ] {
        assert_disallowed_model_tool_blocked(case.0, case.1, case.2).await;
    }
}

async fn assert_disallowed_model_tool_blocked(
    case_id: &str,
    model_tool_name: &str,
    model_action_type: &str,
) {
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
    configure_scripted_provider_scheduler(
        &state,
        "gpt-react-mcp-loop-disallowed-tool",
        serde_json::json!({
            "final": "I will try an unsafe tool.",
            "actions": [{
                "name": model_tool_name,
                "action_type": model_action_type,
                "arguments": {
                    "content": "do not write"
                }
            }],
            "thought_summary": "This attempts a disallowed tool selection outside the candidate contract.",
            "warnings": []
        })
        .to_string(),
    )
    .await;

    let session_id = format!("command-surface-mcp-agent-loop-disallowed-tool-{case_id}");
    let user_text = "Use mcp builtin_echo read-only now.";
    let response = send_message_with_state(
        session_id,
        vec![ChatMessage {
            role: "user".into(),
            content: user_text.into(),
        }],
        None,
        &state,
    )
    .await
    .expect("send_message disallowed model tool response");

    assert!(!response.legacy_fallback_used);
    let task_session_id = response
        .agent_ingress
        .as_ref()
        .and_then(|decision| decision.agent_task_session_id.as_deref())
        .expect("disallowed model tool task session id");

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load disallowed tool task session")
            .expect("disallowed tool task session exists")
    };
    assert_eq!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    );
    assert!(session
        .pending_blockers
        .iter()
        .any(|blocker| blocker == "model_selected_disallowed_tool"));

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list disallowed tool actions")
    };
    let mcp_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .expect("mcp read action");
    assert_eq!(
        mcp_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
    );
    let observation = mcp_action
        .observation_metadata
        .as_ref()
        .expect("disallowed tool observation metadata");
    assert_eq!(
        observation["blockerReason"],
        serde_json::json!("model_selected_disallowed_tool")
    );
    assert_eq!(
        observation["modelSelectedAllowedTool"],
        serde_json::json!(false)
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

async fn grant_mcp_read_tool(state: &std::sync::Arc<crate::AppState>, tool_name: &str) {
    let store = state.tool_permission_store.lock().await;
    store
        .grant(
            tool_name,
            "builtin",
            "low",
            "read",
            openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
            None,
        )
        .expect("grant MCP read permission");
}

async fn configure_http_provider_scheduler(
    state: &std::sync::Arc<crate::AppState>,
    provider_base: &str,
    chat_model: &str,
) {
    let mut config = state.config.lock().await.clone();
    config.local_model = "unused-local-model".into();
    config.prefer_local_model = false;
    config.llm.provider = "openai".into();
    config.llm.openai_base = provider_base.into();
    config.llm.openai_key = "test-key".into();
    config.llm.chat_model = chat_model.into();
    config.llm.embedding_model = "text-embedding-test".into();
    config.llm.embedding_enabled = false;
    config.system.network_policy.enabled = true;
    config.system.network_policy.default_decision = "allow".into();
    state.replace_provider_runtime_config(config).await;
}

async fn configure_scripted_provider_scheduler(
    state: &std::sync::Arc<crate::AppState>,
    chat_model: &str,
    response: String,
) {
    let mut config = state.config.lock().await.clone();
    config.local_model = "unused-local-model".into();
    config.prefer_local_model = false;
    config.llm.provider = "openai".into();
    config.llm.openai_base = "https://example.invalid/v1".into();
    config.llm.openai_key = "test-key".into();
    config.llm.chat_model = chat_model.into();
    config.llm.embedding_model = "text-embedding-test".into();
    config.llm.embedding_enabled = false;
    state.replace_provider_runtime_config(config).await;
    let mut scheduler = state.scheduler.lock().await;
    *scheduler = scheduler
        .clone()
        .with_scripted_generation_response(response);
}

async fn main_chat_transcript(
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
        .expect("list main chat transcript")
}

fn read_http_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut request_bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    let mut expected_request_len = None;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let bytes_read = match std::io::Read::read(stream, &mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => bytes_read,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                if std::time::Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            }
            Err(_) => break,
        };
        request_bytes.extend_from_slice(&buffer[..bytes_read]);
        let request_so_far = String::from_utf8_lossy(&request_bytes);
        if expected_request_len.is_none() {
            if let Some((headers, _)) = request_so_far.split_once("\r\n\r\n") {
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        if name.eq_ignore_ascii_case("content-length") {
                            value.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                expected_request_len = Some(headers.len() + "\r\n\r\n".len() + content_length);
            }
        }
        if expected_request_len.is_some_and(|expected| request_bytes.len() >= expected) {
            break;
        }
    }
    request_bytes
}

async fn fake_ordered_chat_provider_endpoint(replies: Vec<String>) -> String {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind local ordered chat provider");
    let addr = listener.local_addr().expect("local ordered provider addr");
    std::thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut handled = 0usize;
        while handled < replies.len().saturating_add(2) && std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
                    let _request_bytes = read_http_request(&mut stream);
                    let reply = replies
                        .get(handled)
                        .or_else(|| replies.last())
                        .cloned()
                        .unwrap_or_else(|| "{}".into());
                    handled += 1;
                    let body = serde_json::json!({
                        "id": "chatcmpl-main-chat-provider-ranked",
                        "object": "chat.completion",
                        "choices": [{
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": reply
                            },
                            "finish_reason": "stop"
                        }]
                    })
                    .to_string();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    format!("http://{addr}/v1")
}

async fn fake_ranked_then_ordered_chat_provider_endpoint(
    ranking_reply: String,
    generation_replies: Vec<String>,
) -> String {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind local ranked chat provider");
    let addr = listener.local_addr().expect("local ranked provider addr");
    std::thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut handled_generation = 0usize;
        let mut handled_total = 0usize;
        while handled_total < generation_replies.len().saturating_add(4)
            && std::time::Instant::now() < deadline
        {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
                    let request_bytes = read_http_request(&mut stream);
                    let request = String::from_utf8_lossy(&request_bytes);
                    let reply = if request.contains("Return ranked_candidate_ids now")
                        || request.contains("Metadata-safe candidate contract")
                        || request.contains("ranked_candidate_ids")
                    {
                        ranking_reply.clone()
                    } else {
                        let reply = generation_replies
                            .get(handled_generation)
                            .or_else(|| generation_replies.last())
                            .cloned()
                            .unwrap_or_else(|| "{}".into());
                        handled_generation += 1;
                        reply
                    };
                    handled_total += 1;
                    let body = serde_json::json!({
                        "id": "chatcmpl-main-chat-provider-ranked",
                        "object": "chat.completion",
                        "choices": [{
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": reply
                            },
                            "finish_reason": "stop"
                        }]
                    })
                    .to_string();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    format!("http://{addr}/v1")
}

async fn fake_capturing_chat_provider_endpoint(
    reply: String,
) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind local capturing chat provider");
    let addr = listener
        .local_addr()
        .expect("local capturing provider addr");
    let (request_tx, request_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
            let request_bytes = read_http_request(&mut stream);
            let request = String::from_utf8_lossy(&request_bytes).to_string();
            let _ = request_tx.send(request);
            let body = serde_json::json!({
                "id": "chatcmpl-main-chat-provider-ranking-capture",
                "object": "chat.completion",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": reply
                    },
                    "finish_reason": "stop"
                }]
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        }
    });
    (format!("http://{addr}/v1"), request_rx)
}

#[tokio::test]
async fn main_chat_react_registered_mcp_agent_loop_uses_provider_ranked_candidate_order() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    grant_mcp_read_tool(&state, "builtin_echo").await;
    grant_mcp_read_tool(&state, "tool.list_available").await;
    let user_text = "Use an mcp read-only utility tool now.";
    let provider_ranked_candidate_ids = {
        let registry = state.mcp_registry.lock().await;
        let plan = build_main_chat_react_action_plan(
            "command-surface-mcp-agent-loop-provider-ranked",
            user_text,
        )
        .expect("provider-ranked base plan");
        let execution_plan = main_chat_react_agent_loop_execution_plan(&registry, &plan);
        let mut candidate_ids = vec!["tool.list_available".to_string()];
        candidate_ids.extend(
            execution_plan
                .tool_candidate_ids()
                .into_iter()
                .filter(|candidate_id| candidate_id != "tool.list_available"),
        );
        candidate_ids
    };
    let provider_base = fake_ranked_then_ordered_chat_provider_endpoint(
        serde_json::json!({
            "ranked_candidate_ids": provider_ranked_candidate_ids
        })
        .to_string(),
        vec![
            serde_json::json!({
                "final": "I will use the provider-ranked read candidate.",
                "actions": [{
                    "name": "tool.list_available",
                    "action_type": "mcp_tool",
                    "arguments": {
                        "ignored": "model supplied arguments must not execute"
                    }
                }],
                "thought_summary": "Select the first provider-ranked governed read candidate.",
                "warnings": []
            })
            .to_string(),
            serde_json::json!({
                "final": "The provider-ranked read candidate completed.",
                "actions": [],
                "thought_summary": "Finish after observing the governed read result.",
                "warnings": []
            })
            .to_string(),
        ],
    )
    .await;
    configure_http_provider_scheduler(&state, &provider_base, "gpt-general-read-model").await;

    let response = send_message_with_state(
        "command-surface-mcp-agent-loop-provider-ranked".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: user_text.into(),
        }],
        None,
        &state,
    )
    .await
    .expect("send_message provider-ranked MCP response");

    assert!(!response.legacy_fallback_used);
    let task_session_id = response
        .agent_ingress
        .as_ref()
        .and_then(|decision| decision.agent_task_session_id.as_deref())
        .expect("provider-ranked task session id");
    let transcript = main_chat_transcript(&state, task_session_id).await;
    let plan_entry = transcript
        .iter()
        .find(|entry| {
            entry
                .metadata
                .get("agentLoopAttempted")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
                && entry
                    .metadata
                    .get("toolSelectionModelRanked")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                && entry
                    .metadata
                    .get("toolSelectionRankingProviderBacked")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
        })
        .expect("provider-ranked plan transcript entry");
    assert_eq!(
        plan_entry
            .metadata
            .get("toolSelectionModelRanked")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        plan_entry
            .metadata
            .get("toolSelectionRankingProviderBacked")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(
        plan_entry
            .metadata
            .get("toolSelectionRankingSource")
            .is_none()
            && plan_entry
                .metadata
                .get("toolSelectionRankingModel")
                .is_none(),
        "minimized transcript facts must not duplicate provider route identity"
    );
    assert!(
        plan_entry
            .metadata
            .get("toolSelectionCandidateIds")
            .is_none(),
        "minimized plan facts must not duplicate the governed candidate allowlist"
    );

    let completed_entry = transcript
        .iter()
        .find(|entry| {
            entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
                && entry
                    .metadata
                    .get("modelSelectedArgumentsSource")
                    .and_then(serde_json::Value::as_str)
                    == Some("governed_candidate_contract")
        })
        .expect("provider-ranked completion transcript entry");
    assert_eq!(
        completed_entry
            .metadata
            .get("modelSelectedArgumentsSource")
            .and_then(serde_json::Value::as_str),
        Some("governed_candidate_contract")
    );
    assert_eq!(
        completed_entry
            .metadata
            .get("liveProviderInvoked")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "live-provider truth must come from adapter-edge receipts"
    );

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list provider-ranked actions")
    };
    let selected_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .expect("provider-ranked read action");
    let selected_observation = selected_action
        .observation_metadata
        .as_ref()
        .expect("provider-ranked action observation");
    assert_eq!(
        selected_observation["toolSelectionCandidateId"],
        serde_json::json!("tool.list_available")
    );
    assert_eq!(
        selected_observation["toolSelectionCandidateRank"],
        serde_json::json!(1)
    );
    assert_eq!(
        selected_observation["toolSelectionCandidateMatchReason"],
        serde_json::json!("provider_model_ranked")
    );

    let durable_events = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id.to_string(),
        None,
        Some(200),
    )
    .await
    .expect("list provider-ranked durable events");
    let provider_started = durable_events
        .iter()
        .filter(|event| event.event_type == "provider.started")
        .collect::<Vec<_>>();
    let provider_completed = durable_events
        .iter()
        .filter(|event| event.event_type == "provider.completed")
        .collect::<Vec<_>>();
    assert_eq!(
        provider_started.len(),
        3,
        "candidate ranking and both AgentLoop generations must each have a durable start fact"
    );
    assert_eq!(provider_completed.len(), 3);
    assert!(durable_events
        .iter()
        .all(|event| event.event_type != "provider.failed"));
    let started_request_ids = provider_started
        .iter()
        .map(|event| event.object_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let completed_request_ids = provider_completed
        .iter()
        .map(|event| event.object_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(started_request_ids, completed_request_ids);
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_keeps_sensitive_context_local() {
    let (provider_base, request_rx) = fake_capturing_chat_provider_endpoint(
        serde_json::json!({
            "ranked_candidate_ids": ["candidate.beta", "candidate.alpha"]
        })
        .to_string(),
    )
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-privacy".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking privacy boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };
    let mut life_model = openlife_core::life_model::LifeModel::default();
    life_model.identity.name = "alice@example.com".into();
    life_model.identity.mission_statement = "Reach me at 13800138000".into();

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &life_model,
        &[ChatMessage {
            role: "user".into(),
            content:
                "Use this private context only for relevance: alice@example.com and 13800138000."
                    .into(),
        }],
        plan,
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "PolicyRouter local-only authority must take precedence over cloud ranking"
    );
    assert_eq!(ranking.ranking_source, "deterministic_local");
    assert_eq!(ranking.ranking_provider, None);
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
    assert!(
        request_rx
            .recv_timeout(std::time::Duration::from_millis(150))
            .is_err(),
        "sensitive current-user context must not cross the provider adapter edge, even masked"
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_uses_the_configured_privacy_policy() {
    let (provider_base, request_rx) = fake_capturing_chat_provider_endpoint(
        serde_json::json!({
            "ranked_candidate_ids": ["candidate.alpha", "candidate.beta"]
        })
        .to_string(),
    )
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-custom-privacy".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Custom privacy provider ranking boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };
    let privacy_engine =
        openlife_core::privacy::PrivacyEngine::with_policy(openlife_core::privacy::PrivacyPolicy {
            enabled: true,
            rules: vec![openlife_core::privacy::PrivacyRule {
                ptype: openlife_core::privacy::PrivacyType::Generic,
                enabled: true,
                action: openlife_core::privacy::PrivacyAction::Block,
                custom_pattern: Some("CUSTOM-SECRET-[0-9]+".into()),
            }],
        });
    let current_user_text = "Rank the governed read-only candidates for this request.";
    let authorization =
        crate::main_chat_kernel::MainChatProviderAuthorization::test_fixture_for_user_text(
            "custom-ranking-privacy",
            true,
            current_user_text,
        );

    let (_, ranking) = rank_main_chat_react_tool_candidates_with_authorization(
        &scheduler,
        &[
            ChatMessage {
                role: "user".into(),
                content: current_user_text.into(),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "Use CUSTOM-SECRET-4815162342 only as private relevance context.".into(),
            },
        ],
        plan,
        &authorization,
        &openlife_core::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..openlife_core::config::NetworkPolicy::default()
        },
        &privacy_engine,
    )
    .await;

    assert!(ranking.model_ranked);
    let captured_request = request_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("capture configured-policy ranking request");
    assert!(!captured_request.contains("CUSTOM-SECRET-4815162342"));
    assert!(captured_request.contains("<BLOCKED_SENSITIVE_0>"));
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_sends_safe_capability_labels_only() {
    let (provider_base, request_rx) = fake_capturing_chat_provider_endpoint(
        serde_json::json!({
            "ranked_candidate_ids": ["candidate.alpha", "candidate.beta"]
        })
        .to_string(),
    )
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-capabilities".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking capability-label boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec![
                    "read".into(),
                    "calendar".into(),
                    "project secret".into(),
                    "delete".into(),
                ],
                selection_rank: 1,
                match_reason: "capability_or_name_match".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into(), "utility".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan,
        true,
    )
    .await;

    assert!(ranking.model_ranked);
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
    let captured_request = request_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("capture provider ranking request");
    assert!(
        captured_request.contains("capabilityLabels=read/calendar"),
        "provider-ranking prompt should expose bounded safe capability labels for model-ranked manifest selection"
    );
    assert!(
        captured_request.contains("capabilityLabels=read/utility"),
        "provider-ranking prompt should expose safe capability labels for each candidate"
    );
    assert!(
        !captured_request.contains("project secret"),
        "provider-ranking prompt must not expose contract-unsafe capability text"
    );
    assert!(
        !captured_request.contains("delete"),
        "provider-ranking prompt must not expose write-like capability labels"
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_requires_route_identity_to_match_request_provider(
) {
    let provider_base = fake_ordered_chat_provider_endpoint(vec![serde_json::json!({
        "ranked_candidate_ids": ["candidate.beta", "candidate.alpha"]
    })
    .to_string()])
    .await;
    let mut router = openlife_core::agent::model_router::ModelRouter::new();
    router.providers.insert(
        "deepseek".into(),
        openlife_core::agent::model_router::ProviderAvailability {
            provider: "deepseek".into(),
            available: true,
            latency_ms: Some(50),
            models: vec!["deepseek-tool-ranker".into()],
            last_checked: chrono::Utc::now(),
            last_error: None,
            health_is_estimated: false,
        },
    );
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-openai-scheduler".into(),
        "text-embedding-test".into(),
        false,
    )
    .with_model_router(router);
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking route identity boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan,
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must fail soft when the routed identity does not match the configured request adapter"
    );
    assert_eq!(ranking.ranking_source, "deterministic_local");
    assert_eq!(
        ranking.ranking_provider, None,
        "a rejected preparation must not be reported as an observed provider invocation"
    );
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_rejects_wrapping_control_route_identity() {
    let provider_base = fake_ordered_chat_provider_endpoint(vec![serde_json::json!({
        "ranked_candidate_ids": ["candidate.beta", "candidate.alpha"]
    })
    .to_string()])
    .await;
    let mut router = openlife_core::agent::model_router::ModelRouter::new();
    router.providers.insert(
        "openai\n".into(),
        openlife_core::agent::model_router::ProviderAvailability {
            provider: "openai\n".into(),
            available: true,
            latency_ms: Some(50),
            models: vec!["gpt-openai-scheduler".into()],
            last_checked: chrono::Utc::now(),
            last_error: None,
            health_is_estimated: false,
        },
    );
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-openai-scheduler".into(),
        "text-embedding-test".into(),
        false,
    )
    .with_model_router(router);
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking raw route identity boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan,
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must fail soft when route identity only matches after trimming control characters"
    );
    assert_eq!(ranking.ranking_source, "deterministic_local");
    assert_eq!(
        ranking.ranking_provider, None,
        "an invalid route identity must not become an observed provider fact"
    );
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_rejects_wrapping_whitespace_route_identity() {
    let provider_base = fake_ordered_chat_provider_endpoint(vec![serde_json::json!({
        "ranked_candidate_ids": ["candidate.beta", "candidate.alpha"]
    })
    .to_string()])
    .await;
    let mut router = openlife_core::agent::model_router::ModelRouter::new();
    router.providers.insert(
        " openai".into(),
        openlife_core::agent::model_router::ProviderAvailability {
            provider: " openai".into(),
            available: true,
            latency_ms: Some(50),
            models: vec!["gpt-openai-scheduler".into()],
            last_checked: chrono::Utc::now(),
            last_error: None,
            health_is_estimated: false,
        },
    );
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        " openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-openai-scheduler".into(),
        "text-embedding-test".into(),
        false,
    )
    .with_model_router(router);
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking whitespace route identity boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan,
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must fail soft when raw route identity only becomes metadata-safe after trimming whitespace"
    );
    assert_eq!(ranking.ranking_source, "deterministic_local");
    assert_eq!(
        ranking.ranking_provider, None,
        "an invalid raw route identity must not become an observed provider fact"
    );
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_rejects_synthetic_or_local_provider_identity_before_call(
) {
    for provider in ["mock", "openai-127-0-0-1"] {
        let (provider_base, request_rx) = fake_capturing_chat_provider_endpoint(
            serde_json::json!({
                "ranked_candidate_ids": ["candidate.beta", "candidate.alpha"]
            })
            .to_string(),
        )
        .await;
        let mut router = openlife_core::agent::model_router::ModelRouter::new();
        router.providers.insert(
            provider.into(),
            openlife_core::agent::model_router::ProviderAvailability {
                provider: provider.into(),
                available: true,
                latency_ms: Some(50),
                models: vec!["gpt-mock-ranker".into()],
                last_checked: chrono::Utc::now(),
                last_error: None,
                health_is_estimated: false,
            },
        );
        let scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            provider.into(),
            provider_base,
            "test-key".into(),
            "gpt-mock-ranker".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_model_router(router);
        let plan = MainChatReactActionPlan {
            queue_action_type: "mcp.read_only".into(),
            executor_action_type: "mcp_tool".into(),
            target: "target.alpha".into(),
            arguments: serde_json::json!({}),
            description: "Synthetic provider ranking identity boundary.".into(),
            requires_network: false,
            uses_ephemeral_file_permission: false,
            uses_ephemeral_mcp_wrapper_permission: true,
            tool_candidates: vec![
                MainChatReactToolCandidate {
                    candidate_id: "candidate.alpha".into(),
                    executor_action_type: "mcp_tool".into(),
                    target: "target.alpha".into(),
                    arguments: serde_json::json!({}),
                    manifest_source: "boundary".into(),
                    capabilities: vec!["read".into()],
                    selection_rank: 1,
                    match_reason: "manifest_default_order".into(),
                },
                MainChatReactToolCandidate {
                    candidate_id: "candidate.beta".into(),
                    executor_action_type: "mcp_tool".into(),
                    target: "target.beta".into(),
                    arguments: serde_json::json!({}),
                    manifest_source: "boundary".into(),
                    capabilities: vec!["read".into()],
                    selection_rank: 2,
                    match_reason: "manifest_default_order".into(),
                },
            ],
        };

        let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
            &scheduler,
            &openlife_core::life_model::LifeModel::default(),
            &[ChatMessage {
                role: "user".into(),
                content: "Use the governed MCP read candidate.".into(),
            }],
            plan,
            true,
        )
        .await;

        assert!(
            !ranking.model_ranked,
            "provider ranking must fail soft for synthetic/local provider identities: {provider}"
        );
        assert_eq!(ranking.ranking_source, "deterministic_local");
        assert_eq!(ranking.ranking_provider.as_deref(), Some(provider));
        assert!(
            !ranking.provider_backed,
            "synthetic/local provider identities must not be reported as provider-backed ranking routes: {provider}"
        );
        assert_eq!(
            ranked_plan.tool_candidate_ids(),
            vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
        );
        assert!(
            request_rx
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_err(),
            "synthetic/local provider ranking routes must be rejected before sending a provider request: {provider}"
        );
    }
}

#[tokio::test]
async fn main_chat_react_registered_mcp_agent_loop_ignores_invalid_provider_candidate_ranking() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    grant_mcp_read_tool(&state, "builtin_echo").await;
    let provider_base = fake_ordered_chat_provider_endpoint(vec![
        serde_json::json!({
            "ranked_candidate_ids": ["file.write", "unknown.write"]
        })
        .to_string(),
        serde_json::json!({
            "final": "I will use the governed default read candidate.",
            "actions": [{
                "name": "builtin_echo",
                "action_type": "mcp_tool",
                "arguments": {
                    "ignored": "model supplied arguments must not execute"
                }
            }],
            "thought_summary": "Invalid ranking candidates must not change the governed set.",
            "warnings": []
        })
        .to_string(),
        serde_json::json!({
            "final": "The governed default read candidate completed.",
            "actions": [],
            "thought_summary": "Finish after observing the governed read result.",
            "warnings": []
        })
        .to_string(),
    ])
    .await;
    configure_http_provider_scheduler(&state, &provider_base, "gpt-invalid-provider-ranking").await;

    let response = send_message_with_state(
        "command-surface-mcp-agent-loop-invalid-provider-ranking".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "Use an mcp read-only utility tool now.".into(),
        }],
        None,
        &state,
    )
    .await
    .expect("send_message invalid provider ranking response");

    assert!(!response.legacy_fallback_used);
    let task_session_id = response
        .agent_ingress
        .as_ref()
        .and_then(|decision| decision.agent_task_session_id.as_deref())
        .expect("invalid provider ranking task session id");
    let transcript = main_chat_transcript(&state, task_session_id).await;
    let plan_entry = transcript
        .iter()
        .find(|entry| {
            entry
                .metadata
                .get("agentLoopAttempted")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
                && entry
                    .metadata
                    .get("toolSelectionModelRankingIgnored")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                && entry
                    .metadata
                    .get("toolSelectionRankingProviderBacked")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
        })
        .expect("invalid ranking plan transcript entry");
    assert_eq!(
        plan_entry
            .metadata
            .get("toolSelectionModelRanked")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        plan_entry
            .metadata
            .get("toolSelectionModelRankingIgnored")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(
        plan_entry
            .metadata
            .get("toolSelectionRankingSource")
            .is_none(),
        "minimized transcript facts must not duplicate ranking route identity"
    );
    assert!(
        plan_entry
            .metadata
            .get("toolSelectionModelRankingCandidateIds")
            .is_none()
            && plan_entry
                .metadata
                .get("toolSelectionModelRankingResponseDigest")
                .is_none()
            && plan_entry
                .metadata
                .get("toolSelectionCandidateIds")
                .is_none(),
        "minimized transcript facts must persist neither untrusted ranking output nor the governed allowlist"
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_ignores_target_aliases() {
    let provider_base = fake_ordered_chat_provider_endpoint(vec![serde_json::json!({
        "ranked_candidate_ids": ["target.beta"]
    })
    .to_string()])
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-target-alias".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking candidate-id boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan.clone(),
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must accept candidate ids only, not target aliases"
    );
    assert!(
        ranking.ignored,
        "target-alias provider ranking should be ignored fail-soft"
    );
    assert_eq!(ranked_plan.target, "target.alpha");
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_requires_complete_candidate_permutation() {
    let provider_base = fake_ordered_chat_provider_endpoint(vec![serde_json::json!({
        "ranked_candidate_ids": ["candidate.beta"]
    })
    .to_string()])
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-partial".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking complete-permutation boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan.clone(),
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must include every bounded candidate id before it can be credited"
    );
    assert!(
        ranking.ignored,
        "partial provider ranking should be ignored fail-soft"
    );
    assert_eq!(ranked_plan.target, "target.alpha");
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_rejects_duplicate_candidate_ids() {
    let provider_base = fake_ordered_chat_provider_endpoint(vec![serde_json::json!({
        "ranked_candidate_ids": ["candidate.beta", "candidate.alpha", "candidate.alpha"]
    })
    .to_string()])
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-duplicates".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking duplicate candidate boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan.clone(),
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must be a duplicate-free candidate-id permutation"
    );
    assert_eq!(ranked_plan.target, "target.alpha");
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_rejects_duplicate_source_candidate_ids() {
    let (provider_base, request_rx) = fake_capturing_chat_provider_endpoint(
        serde_json::json!({
            "ranked_candidate_ids": ["candidate.alpha", "candidate.alpha"]
        })
        .to_string(),
    )
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-source-duplicates".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking duplicate source candidate boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan.clone(),
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must fail soft before model invocation when the governed candidate set is not duplicate-free"
    );
    assert!(
        !ranking.ignored,
        "duplicate source candidate ids should be rejected before provider response handling"
    );
    assert!(
        request_rx
            .recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "duplicate source candidate ids must not be sent to the provider-ranked preselection prompt"
    );
    assert_eq!(ranked_plan.target, "target.alpha");
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.alpha".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_rejects_contract_unsafe_source_candidates_before_provider_call(
) {
    let (provider_base, request_rx) = fake_capturing_chat_provider_endpoint(
        serde_json::json!({
            "ranked_candidate_ids": ["candidate.beta", "candidate alpha"]
        })
        .to_string(),
    )
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-source-contract".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking contract-unsafe source candidate boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan.clone(),
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must fail soft before model invocation when source candidate labels are contract-unsafe"
    );
    assert_eq!(ranking.ranking_source, "deterministic_local");
    assert!(
        !ranking.ignored,
        "contract-unsafe source candidates should be rejected before provider response handling"
    );
    assert!(
        request_rx
            .recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "contract-unsafe source candidates must not be sent to the provider-ranked preselection prompt"
    );
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate alpha".to_string(), "candidate.beta".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_rejects_contract_unsafe_candidate_ids() {
    let provider_base = fake_ordered_chat_provider_endpoint(vec![serde_json::json!({
        "ranked_candidate_ids": ["candidate.beta", "candidate.alpha", "candidate beta"]
    })
    .to_string()])
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-contract-unsafe".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking contract-unsafe candidate boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan.clone(),
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must reject the whole response when any returned candidate id is contract-unsafe"
    );
    assert_eq!(ranked_plan.target, "target.alpha");
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_rejects_wrapping_whitespace_candidate_ids() {
    let provider_base = fake_ordered_chat_provider_endpoint(vec![serde_json::json!({
        "ranked_candidate_ids": [" candidate.beta", "candidate.alpha "]
    })
    .to_string()])
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-wrapping-whitespace".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking raw candidate-id boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan.clone(),
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must not normalize returned candidate ids before validation"
    );
    assert!(
        ranking.ignored,
        "wrapping-whitespace provider ranking should be ignored fail-soft"
    );
    assert_eq!(ranked_plan.target, "target.alpha");
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_rejects_extra_response_fields() {
    let provider_base = fake_ordered_chat_provider_endpoint(vec![serde_json::json!({
        "ranked_candidate_ids": ["candidate.beta", "candidate.alpha"],
        "arguments": {
            "tool_name": "file.write"
        }
    })
    .to_string()])
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-extra-fields".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking response-shape boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan.clone(),
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must reject responses that include fields beyond ranked_candidate_ids"
    );
    assert!(
        ranking.ignored,
        "extra-field provider ranking should be ignored fail-soft"
    );
    assert_eq!(ranked_plan.target, "target.alpha");
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_provider_candidate_ranking_rejects_markdown_fenced_response() {
    let provider_base = fake_ordered_chat_provider_endpoint(vec![
        "```json\n{\"ranked_candidate_ids\":[\"candidate.beta\",\"candidate.alpha\"]}\n```"
            .to_string(),
    ])
    .await;
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        provider_base,
        "test-key".into(),
        "gpt-provider-ranking-fenced-json".into(),
        "text-embedding-test".into(),
        false,
    );
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "target.alpha".into(),
        arguments: serde_json::json!({}),
        description: "Synthetic provider ranking response-format boundary.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: true,
        tool_candidates: vec![
            MainChatReactToolCandidate {
                candidate_id: "candidate.alpha".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.alpha".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 1,
                match_reason: "manifest_default_order".into(),
            },
            MainChatReactToolCandidate {
                candidate_id: "candidate.beta".into(),
                executor_action_type: "mcp_tool".into(),
                target: "target.beta".into(),
                arguments: serde_json::json!({}),
                manifest_source: "boundary".into(),
                capabilities: vec!["read".into()],
                selection_rank: 2,
                match_reason: "manifest_default_order".into(),
            },
        ],
    };

    let (ranked_plan, ranking) = rank_main_chat_react_tool_candidates_with_model(
        &scheduler,
        &openlife_core::life_model::LifeModel::default(),
        &[ChatMessage {
            role: "user".into(),
            content: "Use the governed MCP read candidate.".into(),
        }],
        plan.clone(),
        true,
    )
    .await;

    assert!(
        !ranking.model_ranked,
        "provider ranking must reject Markdown fenced JSON instead of normalizing it"
    );
    assert!(
        ranking.ignored,
        "fenced provider ranking should be ignored fail-soft"
    );
    assert_eq!(ranked_plan.target, "target.alpha");
    assert_eq!(
        ranked_plan.tool_candidate_ids(),
        vec!["candidate.alpha".to_string(), "candidate.beta".to_string()]
    );
}

#[tokio::test]
async fn main_chat_react_registered_mcp_agent_loop_records_selected_candidate_execution_policy() {
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
    let provider_base = fake_ordered_chat_provider_endpoint(vec![
        serde_json::json!({
            "ranked_candidate_ids": ["builtin_echo"]
        })
        .to_string(),
        serde_json::json!({
            "final": "I will run the selected read-only MCP candidate first.",
            "actions": [{
                "name": "builtin_echo",
                "action_type": "mcp_tool",
                "arguments": {}
            }],
            "thought_summary": "Select the governed read-only candidate.",
            "warnings": []
        })
        .to_string(),
        serde_json::json!({
            "final": "The selected read-only MCP candidate completed.",
            "actions": [],
            "thought_summary": "Finish after observing the governed read result.",
            "warnings": []
        })
        .to_string(),
    ])
    .await;
    configure_http_provider_scheduler(
        &state,
        &provider_base,
        "gpt-react-mcp-loop-policy-validation",
    )
    .await;

    let session_id = "command-surface-mcp-agent-loop-selected-policy";
    let user_text = "Use mcp builtin_echo read-only now.";
    let response = send_message_with_state(
        session_id.into(),
        vec![ChatMessage {
            role: "user".into(),
            content: user_text.into(),
        }],
        None,
        &state,
    )
    .await
    .expect("send_message selected candidate policy response");

    assert!(!response.legacy_fallback_used);
    let task_session_id = response
        .agent_ingress
        .as_ref()
        .and_then(|decision| decision.agent_task_session_id.as_deref())
        .expect("selected candidate policy task session id");

    let transcript = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .expect("list selected policy transcript")
    };
    let completed_entry = transcript
        .iter()
        .find(|entry| {
            entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        })
        .expect("selected policy completion transcript entry");
    assert_eq!(
        completed_entry
            .metadata
            .get("summaryCode")
            .and_then(serde_json::Value::as_str),
        Some("final_result_state_recorded")
    );

    let actions = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .expect("main chat action queue store");
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .expect("list selected policy actions")
    };
    let mcp_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .expect("mcp read action");
    assert_eq!(
        mcp_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed,
        "full command-surface success requires the live ToolGateway receipt sidecar to survive AgentLoop and ReAct"
    );
    let observation_metadata = mcp_action
        .observation_metadata
        .as_ref()
        .expect("selected policy observation metadata");
    assert_selected_candidate_gateway_metadata(observation_metadata);
    assert_eq!(
        observation_metadata
            .get("toolExecutionReceipt")
            .and_then(|receipt| receipt.get("transportStatus"))
            .and_then(serde_json::Value::as_str),
        Some("response_observed")
    );
    assert!(observation_metadata
        .get("receiptInvariantViolation")
        .is_none());
}

#[tokio::test]
async fn main_chat_react_registered_mcp_agent_loop_uses_governed_candidate_arguments() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut registry = state.mcp_registry.lock().await;
        registry.register_builtin(
            openlife_core::tool_manifest::ToolManifest {
                id: "argument_guard.read".into(),
                name: "argument_guard.read".into(),
                description: "Read-only argument guard for Main Chat candidate tests.".into(),
                parameters: serde_json::json!({ "type": "object" }),
                permission_level: "low".into(),
                risk_level: "low".into(),
                version: "1.0.0".into(),
                source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["read".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                idempotency_contract:
                    openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent,
                tags: vec!["argument_guard".into()],
            },
            Box::new(|args| {
                if args.get("content").is_some() {
                    Err(anyhow::anyhow!(
                        "model arguments reached governed read tool"
                    ))
                } else {
                    Ok(format!("governed arguments: {}", args))
                }
            }),
        );
    }
    {
        let store = state.tool_permission_store.lock().await;
        store
            .grant(
                "argument_guard.read",
                "builtin",
                "low",
                "read",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                None,
            )
            .expect("grant argument guard permission");
    }
    let provider_base = fake_ordered_chat_provider_endpoint(vec![
        serde_json::json!({
            "ranked_candidate_ids": ["argument_guard.read"]
        })
        .to_string(),
        serde_json::json!({
            "final": "I will run the selected read-only MCP candidate first.",
            "actions": [{
                "name": "argument_guard.read",
                "action_type": "mcp_tool",
                "arguments": {
                    "content": "model-supplied argument must not reach executor"
                }
            }],
            "thought_summary": "Select the governed read-only candidate.",
            "warnings": []
        })
        .to_string(),
        serde_json::json!({
            "final": "The governed argument read completed.",
            "actions": [],
            "thought_summary": "Finish after observing the governed read result.",
            "warnings": []
        })
        .to_string(),
    ])
    .await;
    configure_http_provider_scheduler(
        &state,
        &provider_base,
        "gpt-react-mcp-loop-governed-arguments",
    )
    .await;

    let session_id = "command-surface-mcp-agent-loop-governed-arguments";
    let user_text = "Use mcp argument_guard.read read-only now.";
    let response = send_message_with_state(
        session_id.into(),
        vec![ChatMessage {
            role: "user".into(),
            content: user_text.into(),
        }],
        None,
        &state,
    )
    .await
    .expect("send_message governed argument response");

    assert!(!response.legacy_fallback_used);
    let task_session_id = response
        .agent_ingress
        .as_ref()
        .and_then(|decision| decision.agent_task_session_id.as_deref())
        .expect("governed argument task session id");

    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .expect("load governed argument task session")
            .expect("governed argument task session exists")
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
            .expect("list governed argument actions")
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
        .expect("governed argument observation metadata");
    assert_eq!(
        metadata["governedArgumentsSource"],
        serde_json::json!("kernel_manifest_candidate_contract")
    );
    assert_eq!(metadata["governedInput"], serde_json::json!({}));
    assert!(
        metadata["governedInput"].get("content").is_none(),
        "model-supplied arguments must not reach the ToolGateway input"
    );
    assert_eq!(metadata["boundedArguments"], serde_json::json!(true));
    assert_eq!(metadata["strictManifestIdentity"], serde_json::json!(true));
    assert_eq!(metadata["directWritesExecuted"], serde_json::json!(false));
}

fn assert_selected_candidate_gateway_metadata(metadata: &serde_json::Value) {
    assert_eq!(
        metadata
            .get("selectedCandidateRank")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "selected candidate metadata must preserve deterministic rank evidence"
    );
    assert!(
        metadata
            .get("selectedCandidateSource")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|source| !source.trim().is_empty()),
        "selected candidate metadata must preserve a metadata-safe manifest source"
    );
    assert!(
        metadata
            .get("selectedCandidateCapabilityDigest")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|digest| digest.starts_with("bytes:")),
        "selected candidate metadata must preserve metadata-safe capability digest evidence"
    );
    assert!(
        metadata
            .get("selectedCandidateCapabilityLabels")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|labels| labels == "read" || labels.starts_with("read/")),
        "selected candidate metadata must preserve bounded safe capability labels"
    );
    assert!(
        metadata
            .get("selectedCandidateMatchReason")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|reason| !reason.trim().is_empty()),
        "selected candidate metadata must preserve a bounded match reason"
    );
    assert_eq!(metadata["strictManifestIdentity"], serde_json::json!(true));
    assert_eq!(metadata["fuzzyNameMatchingUsed"], serde_json::json!(false));
    assert_eq!(metadata["boundedArguments"], serde_json::json!(true));
    assert_eq!(
        metadata["governedArgumentsSource"],
        serde_json::json!("kernel_manifest_candidate_contract")
    );
    assert_eq!(
        metadata["structuredResult"]["toolGateway"]["toolGatewayAuthority"],
        serde_json::json!(true)
    );
    assert_eq!(
        metadata["structuredResult"]["toolGateway"]["actionEffect"],
        serde_json::json!("read_only")
    );
    assert_eq!(
        metadata["structuredResult"]["toolGateway"]["inferredNameContractCredit"],
        serde_json::json!(false)
    );
    assert_eq!(
        metadata
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
}
