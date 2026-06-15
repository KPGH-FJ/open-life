use openlife_core::llm::ChatMessage;

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
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "gpt-react-mcp-loop-disallowed-tool".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response(
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
        );
    }

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

    let transcript = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("main chat session store");
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .expect("list disallowed tool transcript")
    };
    let blocked_entry = transcript
        .iter()
        .find(|entry| {
            entry
                .summary
                .contains("blocked a disallowed model-selected tool")
        })
        .expect("disallowed tool blocker transcript entry");
    assert_eq!(
        blocked_entry
            .metadata
            .get("modelSelectedAllowedTool")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        blocked_entry
            .metadata
            .get("singleStepFallbackUsed")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        blocked_entry
            .metadata
            .get("blockerReason")
            .and_then(serde_json::Value::as_str),
        Some("model_selected_disallowed_tool")
    );

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
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "gpt-react-mcp-loop-policy-validation".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response(
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
        );
    }

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
        .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
        .expect("selected policy completion transcript entry");
    assert_selected_candidate_policy_metadata(&completed_entry.metadata);

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
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
    );
    assert_selected_candidate_policy_metadata(
        mcp_action
            .observation_metadata
            .as_ref()
            .expect("selected policy observation metadata"),
    );
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
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "gpt-react-mcp-loop-governed-arguments".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response(
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
        );
    }

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
        metadata["modelSelectedArgumentsSource"],
        serde_json::json!("governed_candidate_contract")
    );
    assert_eq!(
        metadata["modelSelectedAllowedTool"],
        serde_json::json!(true)
    );
    assert_eq!(metadata["directWritesExecuted"], serde_json::json!(false));
}

fn assert_selected_candidate_policy_metadata(metadata: &serde_json::Value) {
    assert_eq!(
        metadata
            .get("toolSelectionCandidateRank")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "selected candidate metadata must preserve deterministic rank evidence"
    );
    assert!(
        metadata
            .get("toolSelectionCandidateSource")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|source| !source.trim().is_empty()),
        "selected candidate metadata must preserve a metadata-safe manifest source"
    );
    assert!(
        metadata
            .get("toolSelectionCandidateCapabilitiesDigest")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|digest| digest.starts_with("bytes:")),
        "selected candidate metadata must preserve metadata-safe capability digest evidence"
    );
    assert!(
        metadata
            .get("toolSelectionCandidateMatchReason")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|reason| !reason.trim().is_empty()),
        "selected candidate metadata must preserve a bounded match reason"
    );
    assert_eq!(
        metadata
            .get("modelSelectedExecutionPolicyValidated")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        metadata
            .get("modelSelectedExecutionAllowed")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        metadata
            .get("modelSelectedExecutionPolicyLevel")
            .and_then(serde_json::Value::as_str),
        Some("l1_read_only_auto")
    );
    assert_eq!(
        metadata
            .get("modelSelectedExecutionPolicyReasonCode")
            .and_then(serde_json::Value::as_str),
        Some("read_only_action_allowed")
    );
    assert_eq!(
        metadata
            .get("modelSelectedSilentWriteAllowed")
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
