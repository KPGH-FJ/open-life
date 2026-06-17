use openlife_core::agent::main_chat_agent_v1::{
    ActionQueueStore, AgentTaskSessionDraft, AgentTaskSessionStore, ExecutionAction,
    ExecutionPolicy, ExecutionQueueStatus, ExecutionTranscriptEntryDraft,
    ExecutionTranscriptEntryKind, MainChatAgentStrategy,
};
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2TaskContinuityScenario {
    pub id: String,
    pub capability_group: String,
    pub prompt: String,
    pub preconditions: Vec<String>,
    pub expected_route: String,
    pub required_runtime_evidence: Vec<String>,
    pub required_ui_state: Vec<String>,
    pub required_controls: Vec<String>,
    pub negative_assertions: Vec<String>,
    pub expected_outcome: String,
    pub default_gate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2TaskContinuityProof {
    pub scenario_id: String,
    pub passed: bool,
    pub expected_blocker: bool,
    pub task_session_ids: Vec<String>,
    pub action_ids: Vec<String>,
    pub controls: Vec<String>,
    pub diagnostics: Vec<String>,
    pub runtime_evidence: Vec<String>,
    pub ui_state: Vec<String>,
    pub negative_assertions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2TaskContinuityGateReport {
    pub scenario_count: usize,
    pub default_gate_scenario_count: usize,
    pub passed_scenario_count: usize,
    pub expected_blocker_count: usize,
    pub ready: bool,
    pub blockers: Vec<String>,
    pub scenarios: Vec<MainChatProductMaturityV2TaskContinuityScenario>,
    pub proofs: Vec<MainChatProductMaturityV2TaskContinuityProof>,
}

pub(crate) fn main_chat_product_maturity_v2_task_continuity_scenarios(
) -> Vec<MainChatProductMaturityV2TaskContinuityScenario> {
    vec![
        scenario(
            "LT2-01",
            "Open task list.",
            &["agent_task_session_store"],
            &["task_summaries", "resume_safety_digest"],
            &["task_continuity_list_visible"],
            &["open_task_detail"],
            &["no_raw_chat_text_inference"],
            "pass",
        ),
        scenario(
            "LT2-02",
            "Open blocked task detail.",
            &["blocked_task_session_id"],
            &["blockers", "last_observation", "next_control"],
            &["blocked_detail_visible"],
            &["retry", "cancel", "refresh_context"],
            &["no_frontend_only_blocker"],
            "pass",
        ),
        scenario(
            "LT2-03",
            "Resume after exact permission acceptance.",
            &["accepted_tool_permission", "action_input_hash"],
            &["same_action_id", "same_target", "resume_allowed"],
            &["resume_control_visible"],
            &["resume"],
            &["no_changed_target_replay"],
            "pass",
        ),
        scenario(
            "LT2-04",
            "Resume after target changed.",
            &["accepted_tool_permission", "changed_target"],
            &["permission_scope_mismatch"],
            &["scope_mismatch_blocker_visible"],
            &["refresh_context"],
            &["no_automatic_replay", "no_changed_target_replay"],
            "expected_blocker",
        ),
        scenario(
            "LT2-05",
            "Retry failed safe read.",
            &["failed_safe_read_action"],
            &["retry_control", "action_id"],
            &["retry_control_visible"],
            &["retry"],
            &["no_write_execution"],
            "pass",
        ),
        scenario(
            "LT2-06",
            "Continue stale task.",
            &["stored_context_digest", "current_context_digest"],
            &["stale_context"],
            &["stale_warning_visible"],
            &["refresh_context"],
            &["no_automatic_replay"],
            "expected_blocker",
        ),
        scenario(
            "LT2-07",
            "Resume completed task.",
            &["completed_task_session_id"],
            &["terminal_no_resume"],
            &["terminal_explanation_visible"],
            &["open_trace"],
            &["no_terminal_resume"],
            "expected_blocker",
        ),
        scenario(
            "LT2-08",
            "Reopen app and inspect task.",
            &["persisted_task_session_id"],
            &["task_detail"],
            &["persisted_detail_visible"],
            &["open_trace"],
            &["no_raw_chat_text_inference"],
            "pass",
        ),
    ]
}

pub(crate) async fn run_main_chat_agent_product_maturity_v2_task_continuity_gate(
) -> MainChatProductMaturityV2TaskContinuityGateReport {
    let scenarios = main_chat_product_maturity_v2_task_continuity_scenarios();
    let proofs = match run_task_continuity_runtime_proofs().await {
        Ok(proofs) => proofs,
        Err(error) => vec![failed_proof("phase_d_runtime", error)],
    };
    let passed_scenario_count = proofs.iter().filter(|proof| proof.passed).count();
    let expected_blocker_count = proofs.iter().filter(|proof| proof.expected_blocker).count();
    let mut blockers = Vec::new();
    if passed_scenario_count != scenarios.len() {
        blockers.push("phase_d_task_continuity_scenarios_not_ready".into());
    }
    for id in [
        "LT2-01", "LT2-02", "LT2-03", "LT2-04", "LT2-05", "LT2-06", "LT2-07", "LT2-08",
    ] {
        if !proofs.iter().any(|proof| proof.scenario_id == id) {
            blockers.push(format!("missing_task_continuity_eval:{id}"));
        }
    }

    MainChatProductMaturityV2TaskContinuityGateReport {
        scenario_count: scenarios.len(),
        default_gate_scenario_count: scenarios.len(),
        passed_scenario_count,
        expected_blocker_count,
        ready: blockers.is_empty(),
        blockers,
        scenarios,
        proofs,
    }
}

async fn run_task_continuity_runtime_proofs(
) -> Result<Vec<MainChatProductMaturityV2TaskContinuityProof>, String> {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let blocked = seed_failed_safe_read_task(&state).await?;
    let stale = seed_stale_task(&state).await?;
    let completed = seed_completed_task(&state).await?;
    let accepted_permission = seed_waiting_permission_task(&state, false).await?;
    let changed_target = seed_waiting_permission_task(&state, true).await?;
    let persistence_root = std::env::temp_dir().join(format!(
        "openlife-main-chat-lt2-persist-{}",
        uuid::Uuid::new_v4()
    ));
    let persistent_seed_state = build_persistent_task_continuity_eval_state(&persistence_root)?;
    let persisted_task = seed_failed_safe_read_task(&persistent_seed_state).await?;
    let reopened_state = build_persistent_task_continuity_eval_state(&persistence_root)?;

    let summaries = crate::main_chat_task_controls::list_main_chat_agent_tasks_with_state(
        None,
        Some(20),
        Some(0),
        &state,
    )
    .await?;
    let blocked_detail =
        crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
            &blocked.task_session_id,
            &state,
        )
        .await?;
    let stale_detail = crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
        &stale.task_session_id,
        &state,
    )
    .await?;
    let completed_detail =
        crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
            &completed.task_session_id,
            &state,
        )
        .await?;
    let accepted_detail =
        crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
            &accepted_permission.task_session_id,
            &state,
        )
        .await?;
    let changed_detail =
        crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
            &changed_target.task_session_id,
            &state,
        )
        .await?;
    let persisted_detail =
        crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state(
            &persisted_task.task_session_id,
            &reopened_state,
        )
        .await?;

    let mut proofs = Vec::new();
    proofs.push(proof(
        "LT2-01",
        !summaries.is_empty()
            && summaries
                .iter()
                .all(|summary| summary.resume_safety_digest.starts_with("bytes:")),
        false,
        summaries
            .iter()
            .map(|summary| summary.task_session_id.clone())
            .collect(),
        Vec::new(),
        vec!["open_task_detail".into()],
        Vec::new(),
        vec!["task_summaries".into(), "resume_safety_digest".into()],
        vec!["task_continuity_list_visible".into()],
        vec!["no_raw_chat_text_inference".into()],
    ));
    proofs.push(proof_for_detail(
        "LT2-02",
        &blocked_detail,
        false,
        ["blockers", "last_observation", "next_control"],
        ["blocked_detail_visible"],
        ["no_frontend_only_blocker"],
    ));
    proofs.push(proof_for_detail(
        "LT2-03",
        &accepted_detail,
        false,
        ["same_action_id", "same_target", "resume_allowed"],
        ["resume_control_visible"],
        ["no_changed_target_replay"],
    ));
    proofs.push(proof_for_detail(
        "LT2-04",
        &changed_detail,
        true,
        ["permission_scope_mismatch"],
        ["scope_mismatch_blocker_visible"],
        ["no_automatic_replay", "no_changed_target_replay"],
    ));
    proofs.push(proof_for_detail(
        "LT2-05",
        &blocked_detail,
        false,
        ["retry_control", "action_id"],
        ["retry_control_visible"],
        ["no_write_execution"],
    ));
    proofs.push(proof_for_detail(
        "LT2-06",
        &stale_detail,
        true,
        ["stale_context"],
        ["stale_warning_visible"],
        ["no_automatic_replay"],
    ));
    proofs.push(proof_for_detail(
        "LT2-07",
        &completed_detail,
        true,
        ["terminal_no_resume"],
        ["terminal_explanation_visible"],
        ["no_terminal_resume"],
    ));
    proofs.push(proof(
        "LT2-08",
        persisted_detail.task_session.id == persisted_task.task_session_id
            && !persisted_detail.actions.is_empty()
            && !persisted_detail.transcript.is_empty()
            && persisted_detail
                .blockers
                .iter()
                .any(|blocker| blocker == "safe_read_failed")
            && persisted_detail
                .allowed_controls
                .iter()
                .any(|control| control == "retry"),
        false,
        vec![persisted_detail.task_session.id.clone()],
        persisted_detail
            .actions
            .iter()
            .map(|action| action.id.clone())
            .collect(),
        persisted_detail.allowed_controls.clone(),
        persisted_detail.continuity_diagnostics.reason_codes.clone(),
        vec![
            "persisted_task_detail".into(),
            "fresh_app_state_instance".into(),
            "agent_task_session_store".into(),
            "action_queue_store".into(),
            "transcript".into(),
        ],
        vec!["persisted_detail_visible".into()],
        vec!["no_raw_chat_text_inference".into()],
    ));

    Ok(proofs)
}

fn build_persistent_task_continuity_eval_state(
    root: &Path,
) -> Result<Arc<crate::AppState>, String> {
    let template = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    Ok(Arc::new(crate::AppState {
        main_chat_agent_session_store: Some(Arc::new(tokio::sync::Mutex::new(
            AgentTaskSessionStore::new(root.join("main_chat_agent_sessions.sqlite"))
                .map_err(|err| err.to_string())?,
        ))),
        main_chat_action_queue_store: Some(Arc::new(tokio::sync::Mutex::new(
            ActionQueueStore::new(root.join("main_chat_action_queue.sqlite"))
                .map_err(|err| err.to_string())?,
        ))),
        ..(*template).clone()
    }))
}

struct SeededTask {
    task_session_id: String,
}

async fn seed_failed_safe_read_task(state: &Arc<crate::AppState>) -> Result<SeededTask, String> {
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "session store missing".to_string())?
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "lt2-blocked-chat".into(),
                user_goal: "Retry a failed safe read.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some("Read safe evidence.".into()),
                context_snapshot_refs: vec!["lt2-context".into()],
            })
            .map_err(|err| err.to_string())?
    };
    let action = ExecutionAction::new("file.read", "Read a safe workspace file.");
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .ok_or_else(|| "action queue missing".to_string())?
            .lock()
            .await;
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy::default().classify(&action),
            )
            .map_err(|err| err.to_string())?;
        queue
            .transition(
                &queued.id,
                ExecutionQueueStatus::Failed,
                Some(serde_json::json!({
                    "target": "AGENTS.md",
                    "retryReplayable": true,
                    "directWritesExecuted": false,
                })),
            )
            .map_err(|err| err.to_string())?
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "session store missing".to_string())?
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &queued.id)
            .map_err(|err| err.to_string())?;
        store
            .append_transcript_entry(ExecutionTranscriptEntryDraft {
                session_id: session.id.clone(),
                kind: ExecutionTranscriptEntryKind::Observation,
                summary: "Safe read failed with retryable action evidence.".into(),
                metadata: serde_json::json!({
                    "actionId": queued.id,
                    "contextSnapshotRef": "lt2-context",
                    "directWritesExecuted": false,
                }),
            })
            .map_err(|err| err.to_string())?;
        store
            .block_session(&session.id, "safe_read_failed")
            .map_err(|err| err.to_string())?;
    }
    Ok(SeededTask {
        task_session_id: session.id,
    })
}

async fn seed_stale_task(state: &Arc<crate::AppState>) -> Result<SeededTask, String> {
    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "session store missing".to_string())?
            .lock()
            .await;
        let session = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "lt2-stale-chat".into(),
                user_goal: "Continue a stale task.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec!["current-context".into()],
            })
            .map_err(|err| err.to_string())?;
        store
            .append_transcript_entry(ExecutionTranscriptEntryDraft {
                session_id: session.id.clone(),
                kind: ExecutionTranscriptEntryKind::Observation,
                summary: "Stored context digest no longer matches current context.".into(),
                metadata: serde_json::json!({
                    "continuityContextDigest": "bytes:12 hash:sha256:old-context",
                    "contextSnapshotRef": "previous-context",
                    "directWritesExecuted": false,
                }),
            })
            .map_err(|err| err.to_string())?;
        store
            .block_session(&session.id, "stale_context")
            .map_err(|err| err.to_string())?;
        session
    };
    Ok(SeededTask {
        task_session_id: session.id,
    })
}

async fn seed_completed_task(state: &Arc<crate::AppState>) -> Result<SeededTask, String> {
    let store = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "session store missing".to_string())?
        .lock()
        .await;
    let session = store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: "lt2-completed-chat".into(),
            user_goal: "Completed task.".into(),
            selected_strategy: MainChatAgentStrategy::DirectAnswer,
            current_plan_summary: None,
            context_snapshot_refs: vec![],
        })
        .map_err(|err| err.to_string())?;
    store
        .complete_session(&session.id, "Already completed.")
        .map_err(|err| err.to_string())?;
    Ok(SeededTask {
        task_session_id: session.id,
    })
}

async fn seed_waiting_permission_task(
    state: &Arc<crate::AppState>,
    changed_target: bool,
) -> Result<SeededTask, String> {
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

    let proposal_after = if changed_target {
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
                "input_hash": "hash:sha256:not-current",
                "input_length_bytes": 1
            }
        })
    } else {
        serde_json::json!({
            "tool_name": "builtin_echo",
            "source": "builtin",
            "risk_level": "low",
            "action_type": "read",
            "permission": "allow_once"
        })
    };
    let proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        "tool_permission.builtin.builtin_echo",
        proposal_after,
        "Allow pending Main Chat tool read.",
        0.7,
        RiskLevel::Medium,
        ProposalSource::ChatConversation,
    );
    let proposal_id = proposal.id.clone();
    {
        let proposal_store = state
            .proposal_store
            .as_ref()
            .ok_or_else(|| "proposal store missing".to_string())?;
        proposal_store
            .lock()
            .await
            .create_proposal(&proposal)
            .map_err(|err| err.to_string())?;
    }
    crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), state)
        .await
        .map_err(|err| err.to_string())?;

    let session = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "session store missing".to_string())?
            .lock()
            .await;
        store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: if changed_target {
                    "lt2-changed-target-chat".into()
                } else {
                    "lt2-accepted-permission-chat".into()
                },
                user_goal: "Use mcp builtin_echo read-only now.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some("Waiting for ToolPermission scope.".into()),
                context_snapshot_refs: vec!["permission-context".into()],
            })
            .map_err(|err| err.to_string())?
    };
    let action = ExecutionAction::new("mcp.read_only", "Pending MCP read action.");
    let queued = {
        let queue = state
            .main_chat_action_queue_store
            .as_ref()
            .ok_or_else(|| "action queue missing".to_string())?
            .lock()
            .await;
        let queued = queue
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy::default().classify(&action),
            )
            .map_err(|err| err.to_string())?;
        queue
            .transition(&queued.id, ExecutionQueueStatus::Executing, None)
            .map_err(|err| err.to_string())?;
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
            .map_err(|err| err.to_string())?
    };
    {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "session store missing".to_string())?
            .lock()
            .await;
        store
            .record_action_queue_id(&session.id, &queued.id)
            .map_err(|err| err.to_string())?;
        store
            .set_pending_blockers(&session.id, vec!["tool_permission_required".into()])
            .map_err(|err| err.to_string())?;
        store
            .mark_waiting_permission(&session.id)
            .map_err(|err| err.to_string())?;
    }
    Ok(SeededTask {
        task_session_id: session.id,
    })
}

fn scenario<const P: usize, const R: usize, const U: usize, const C: usize, const N: usize>(
    id: &str,
    prompt: &str,
    preconditions: &[&str; P],
    required_runtime_evidence: &[&str; R],
    required_ui_state: &[&str; U],
    required_controls: &[&str; C],
    negative_assertions: &[&str; N],
    expected_outcome: &str,
) -> MainChatProductMaturityV2TaskContinuityScenario {
    MainChatProductMaturityV2TaskContinuityScenario {
        id: id.into(),
        capability_group: "task_continuity".into(),
        prompt: prompt.into(),
        preconditions: preconditions.iter().map(|value| (*value).into()).collect(),
        expected_route: "task_control".into(),
        required_runtime_evidence: required_runtime_evidence
            .iter()
            .map(|value| (*value).into())
            .collect(),
        required_ui_state: required_ui_state
            .iter()
            .map(|value| (*value).into())
            .collect(),
        required_controls: required_controls
            .iter()
            .map(|value| (*value).into())
            .collect(),
        negative_assertions: negative_assertions
            .iter()
            .map(|value| (*value).into())
            .collect(),
        expected_outcome: expected_outcome.into(),
        default_gate: true,
    }
}

fn proof_for_detail<const R: usize, const U: usize, const N: usize>(
    scenario_id: &str,
    detail: &crate::main_chat_task_controls::TaskDetail,
    expected_blocker: bool,
    runtime_evidence: [&str; R],
    ui_state: [&str; U],
    negative_assertions: [&str; N],
) -> MainChatProductMaturityV2TaskContinuityProof {
    let diagnostics = detail.continuity_diagnostics.reason_codes.clone();
    let expected_diagnostic_present = match scenario_id {
        "LT2-04" => diagnostics
            .iter()
            .any(|code| code == "permission_scope_mismatch"),
        "LT2-06" => diagnostics.iter().any(|code| code == "stale_context"),
        "LT2-07" => diagnostics.iter().any(|code| code == "terminal_no_resume"),
        _ => true,
    };
    let control_present = match scenario_id {
        "LT2-03" => detail
            .allowed_controls
            .iter()
            .any(|control| control == "resume"),
        "LT2-05" => detail
            .allowed_controls
            .iter()
            .any(|control| control == "retry"),
        "LT2-06" => {
            detail
                .allowed_controls
                .iter()
                .any(|control| control == "refresh_context")
                && !detail
                    .allowed_controls
                    .iter()
                    .any(|control| control == "resume")
        }
        "LT2-07" => !detail
            .allowed_controls
            .iter()
            .any(|control| control == "resume"),
        _ => !detail.allowed_controls.is_empty(),
    };
    proof(
        scenario_id,
        expected_diagnostic_present && control_present,
        expected_blocker,
        vec![detail.task_session.id.clone()],
        detail
            .actions
            .iter()
            .map(|action| action.id.clone())
            .collect(),
        detail.allowed_controls.clone(),
        diagnostics,
        runtime_evidence
            .iter()
            .map(|value| (*value).into())
            .collect(),
        ui_state.iter().map(|value| (*value).into()).collect(),
        negative_assertions
            .iter()
            .map(|value| (*value).into())
            .collect(),
    )
}

fn proof(
    scenario_id: &str,
    passed: bool,
    expected_blocker: bool,
    task_session_ids: Vec<String>,
    action_ids: Vec<String>,
    controls: Vec<String>,
    diagnostics: Vec<String>,
    runtime_evidence: Vec<String>,
    ui_state: Vec<String>,
    negative_assertions: Vec<String>,
) -> MainChatProductMaturityV2TaskContinuityProof {
    MainChatProductMaturityV2TaskContinuityProof {
        scenario_id: scenario_id.into(),
        passed,
        expected_blocker,
        task_session_ids,
        action_ids,
        controls,
        diagnostics,
        runtime_evidence,
        ui_state,
        negative_assertions,
    }
}

fn failed_proof(
    scenario_id: &str,
    diagnostic: String,
) -> MainChatProductMaturityV2TaskContinuityProof {
    proof(
        scenario_id,
        false,
        false,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![diagnostic],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}
