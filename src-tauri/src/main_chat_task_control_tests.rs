use crate::main_chat_task_controls::{cancel_main_chat_agent_task, resume_main_chat_agent_task};
use tauri::Manager;

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
fn retry_main_chat_action_enters_manual_blocker_when_not_replayable() {
    let module_path = format!(
        "{}/src/main_chat_task_controls.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(module_path).expect("read task-control module");
    let retry_body =
        extract_rust_function_body(&source, "pub(crate) async fn retry_main_chat_agent_action(");

    assert!(
        retry_body.contains("manual_blocker_required"),
        "retry command must inspect whether the failed action can be replayed"
    );
    assert!(
        retry_body.contains("ExecutionQueueStatus::PendingPermission"),
        "non-replayable retries must become an explicit manual blocker"
    );
    assert!(
        retry_body.contains("manualReplayRequired"),
        "manual retry blocker metadata must be visible in task state/transcript"
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
        retry_body.contains("replay_main_chat_agent_action("),
        "replayable Main Chat retries must execute the failed action again instead of only changing queue state"
    );
    assert!(
        source.contains("automaticReplayCompleted"),
        "automatic retry replay completion must be visible in task state/transcript metadata"
    );
}

#[test]
fn resume_main_chat_task_preserves_pending_permission_blocker_instead_of_state_only() {
    let module_path = format!(
        "{}/src/main_chat_task_controls.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(module_path).expect("read task-control module");
    let resume_body =
        extract_rust_function_body(&source, "pub(crate) async fn resume_main_chat_agent_task(");

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
            "permission": "allow_until_revoked",
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
            .expect("create tool permission proposal");
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
                chat_session_id: "resume-permission-command-surface".into(),
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
                ExecutionPolicy::default().classify(&action),
            )
            .expect("enqueue pending mcp action");
        queue
            .transition(&queued.id, ExecutionQueueStatus::Executing, None)
            .expect("move action to executing");
        queue
            .transition(
                &queued.id,
                ExecutionQueueStatus::PendingPermission,
                Some(serde_json::json!({
                    "proposalId": proposal_id,
                    "toolName": "builtin_echo",
                    "resumeReplayable": true,
                    "directWritesExecuted": false,
                })),
            )
            .expect("move action to pending permission");
        queued
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
        .expect("accept tool permission proposal");

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
    let metadata = replayed
        .observation_metadata
        .as_ref()
        .expect("replay observation metadata");
    assert_eq!(
        metadata["automaticResumeReplayCompleted"],
        serde_json::json!(true)
    );
    assert_eq!(metadata["directWritesExecuted"], serde_json::json!(false));
}

#[tokio::test]
async fn resume_main_chat_task_does_not_replay_tool_permission_when_scope_target_changed() {
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
            "permission": "allow_once",
            "blocked_action": {
                "action_type": "mcp.read_only",
                "target": "mcp.call_tool",
                "resolved_target": "changed_builtin_echo_target",
                "input_hash": "hash:sha256:not-the-current-input",
                "input_length_bytes": 1
            }
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
            .expect("create scoped tool permission proposal");
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
                ExecutionPolicy::default().classify(&action),
            )
            .expect("enqueue pending mcp action");
        queue
            .transition(&queued.id, ExecutionQueueStatus::Executing, None)
            .expect("move action to executing");
        queue
            .transition(
                &queued.id,
                ExecutionQueueStatus::PendingPermission,
                Some(serde_json::json!({
                    "proposalId": proposal_id,
                    "toolName": "builtin_echo",
                    "resumeReplayable": true,
                    "directWritesExecuted": false,
                })),
            )
            .expect("move action to pending permission");
        queued
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
        .expect("accept tool permission proposal");

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
    let transcript = task_state
        .transcript
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
                ExecutionPolicy::default().classify(&planned_action),
            )
            .expect("enqueue planned action");
        let pending = queue
            .enqueue(
                &session.id,
                permission_action.clone(),
                ExecutionPolicy::default().classify(&permission_action),
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
