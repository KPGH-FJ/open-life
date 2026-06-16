use crate::main_chat_final_acceptance_tests::{
    configure_live_provider_eval_state, configure_live_provider_eval_state_with_local_http_provider,
};
use crate::main_chat_final_gate::{
    main_chat_live_provider_acceptance_evidence, MainChatLiveProviderEvalHarnessScenario,
};
use crate::main_chat_live_provider_harness::{
    run_main_chat_live_provider_eval_harness, MainChatLiveProviderEvalHarnessInput,
};

#[test]
fn main_chat_live_provider_command_surface_tests_are_not_concentrated_in_lib_rs() {
    let lib_rs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");

    for forbidden in [
        "main_chat_live_provider_eval_preflight_from_command_state_fails_closed",
        "main_chat_live_provider_eval_harness_blocks_before_command_invocation_when_preflight_fails",
        "main_chat_live_provider_eval_harness_blocks_react_cases_before_command_invocation_when_preflight_fails",
        "main_chat_live_provider_eval_harness_executes_local_http_provider_without_external_live_credit",
        "main_chat_live_provider_eval_harness_invokes_external_direct_answer_when_opted_in",
        "main_chat_live_provider_eval_harness_invokes_external_react_web_and_mcp_when_opted_in",
    ] {
        assert!(
            !source.contains(&format!("\n    async fn {forbidden}(")),
            "live-provider command-surface test {forbidden} should live outside src/lib.rs"
        );
    }
}

#[tokio::test]
async fn main_chat_live_provider_eval_preflight_from_command_state_fails_closed() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        config.llm.provider = "custom".into();
        config.llm.openai_base = "https://example.invalid/v1".into();
        config.llm.openai_key.clear();
        config.system.network_policy.enabled = false;
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "custom".into(),
            "https://example.invalid/v1".into(),
            String::new(),
            "gpt-command-surface-live-preflight".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("scripted response must block live eval");
    }

    let config = state.config.lock().await.clone();
    let scripted_provider_response_present = state
        .scheduler
        .lock()
        .await
        .scripted_generation_response
        .is_some();
    let report =
        openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_live_provider_eval_preflight_from_config(
            &config,
            false,
            scripted_provider_response_present,
            false,
        );

    assert!(!report.ready);
    assert_eq!(report.provider, "custom");
    assert!(!report.live_provider_invocation_allowed);
    assert!(!report.model_invoked);
    assert!(!report.direct_writes_executed);
    assert!(report
        .blockers
        .contains(&"explicit_live_eval_required".to_string()));
    assert!(report
        .blockers
        .contains(&"provider_api_key_missing".to_string()));
    assert!(report.blockers.contains(&"network_disabled".to_string()));
    assert!(report
        .blockers
        .contains(&"scripted_provider_response_not_allowed".to_string()));
    assert!(report
        .required_evidence
        .contains(&"live_provider_generation".to_string()));
    let serialized = serde_json::to_string(&report).expect("serialize preflight report");
    assert!(!serialized.contains("apiKey"));
    assert!(!serialized.contains("scripted response must block live eval"));
}

#[tokio::test]
async fn main_chat_live_provider_eval_harness_blocks_before_command_invocation_when_preflight_fails(
) {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        config.llm.provider = "custom".into();
        config.llm.openai_base = "https://example.invalid/v1".into();
        config.llm.openai_key.clear();
        config.system.network_policy.enabled = false;
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "custom".into(),
            "https://example.invalid/v1".into(),
            String::new(),
            "gpt-command-surface-live-preflight".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("scripted response must block live eval");
    }

    let report = run_main_chat_live_provider_eval_harness(
        state.clone(),
        MainChatLiveProviderEvalHarnessInput {
            scenario: MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            session_id: "live-provider-eval-blocked".into(),
            prompt: "Give a short live-provider DirectAnswer proof.".into(),
            explicit_live_eval_requested: false,
            local_only_required: false,
        },
    )
    .await
    .expect("live provider harness report");

    assert!(!report.ready);
    assert_eq!(report.status, "blocked");
    assert_eq!(report.provider, "custom");
    assert_eq!(report.provider_endpoint_kind, "scripted_scheduler_response");
    assert!(!report.main_chat_invoked);
    assert!(!report.model_invoked);
    assert!(!report.direct_writes_executed);
    assert!(!report.legacy_fallback_used);
    assert!(report.run_id.is_none());
    assert!(report.task_session_id.is_none());
    assert!(report
        .blockers
        .contains(&"explicit_live_eval_required".to_string()));
    assert!(report
        .blockers
        .contains(&"provider_api_key_missing".to_string()));
    assert!(report
        .blockers
        .contains(&"scripted_provider_response_not_allowed".to_string()));
    assert!(report
        .required_evidence
        .contains(&"live_provider_generation".to_string()));
    assert!(report
        .required_evidence
        .contains(&"provider_backed_web_mcp_agent_loop".to_string()));
    let run_count = state
        .agent_run_store
        .as_ref()
        .expect("agent run store")
        .lock()
        .await
        .run_count()
        .expect("run count");
    assert_eq!(run_count, 0);
}

#[tokio::test]
async fn main_chat_live_provider_eval_harness_blocks_react_cases_before_command_invocation_when_preflight_fails(
) {
    for scenario in [
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    ] {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut config = state.config.lock().await;
            config.llm.provider = "custom".into();
            config.llm.openai_base = "https://example.invalid/v1".into();
            config.llm.openai_key.clear();
            config.system.network_policy.enabled = false;
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                "unused-local-model".into(),
                false,
                "custom".into(),
                "https://example.invalid/v1".into(),
                String::new(),
                "gpt-command-surface-live-preflight".into(),
                "text-embedding-test".into(),
                false,
            )
            .with_scripted_generation_response("scripted response must block live eval");
        }

        let report = run_main_chat_live_provider_eval_harness(
            state.clone(),
            MainChatLiveProviderEvalHarnessInput {
                scenario,
                session_id: format!("live-provider-eval-blocked-{}", scenario.as_str()),
                prompt: scenario.prompt().into(),
                explicit_live_eval_requested: false,
                local_only_required: false,
            },
        )
        .await
        .expect("live provider harness report");

        assert!(!report.ready, "{scenario:?}");
        assert_eq!(report.status, "blocked");
        assert_eq!(report.provider_endpoint_kind, "scripted_scheduler_response");
        assert!(!report.main_chat_invoked);
        assert!(!report.model_invoked);
        assert!(!report.agent_loop_succeeded);
        assert!(!report.direct_writes_executed);
        assert!(report
            .required_evidence
            .contains(&"provider_backed_web_mcp_agent_loop".to_string()));
        let run_count = state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await
            .run_count()
            .expect("run count");
        assert_eq!(run_count, 0, "{scenario:?}");
    }
}

#[test]
fn live_provider_harness_preserves_agent_loop_attempt_metadata_when_not_completed() {
    let entries = vec![
        serde_json::json!({
            "summary": "Governed ReAct AgentLoop attempt started.",
            "metadata": {
                "agentLoopAttempted": true,
                "agentLoopSucceeded": false,
                "toolSelectionCandidateCount": 2,
                "toolSelectionCandidateIds": ["builtin_echo", "tool.list_available"]
            }
        }),
        serde_json::json!({
            "summary": "Governed ReAct AgentLoop did not observe the planned action; single-step fallback remains available.",
            "metadata": {
                "agentLoopAttempted": true,
                "agentLoopSucceeded": false,
                "singleStepFallbackUsed": true,
                "plannedActionObserved": false,
                "toolSelectionCandidateCount": 2,
                "toolSelectionCandidateIds": ["builtin_echo", "tool.list_available"],
                "toolSelectionAllowlist": ["builtin_echo", "tool.list_available"]
            }
        }),
    ];

    let metadata =
        crate::main_chat_live_provider_harness::main_chat_live_provider_agent_loop_metadata_from_entries(
            &entries,
        )
        .expect("agent loop attempted metadata");

    assert_eq!(
        metadata
            .get("toolSelectionCandidateCount")
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        metadata
            .get("singleStepFallbackUsed")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test]
fn live_provider_evidence_rejects_tool_permission_proposal_with_mcp_read_success_overlap() {
    let mut report =
        crate::main_chat_final_gate::completed_main_chat_live_provider_eval_harness_report(
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
            "openai",
            "external_provider",
            "run-proposal",
            "task-proposal",
            "proposal ready",
        );
    report.mcp_read_target_resolved = true;

    let evidence = main_chat_live_provider_acceptance_evidence(&[report]);

    assert!(!evidence.proposal_permission_eval_executed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn main_chat_live_provider_eval_harness_executes_local_http_provider_without_external_live_credit(
) {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_live_provider_eval_state_with_local_http_provider(
        &state,
        "local provider harness\ndirect answer",
    )
    .await;

    let report = run_main_chat_live_provider_eval_harness(
        state,
        MainChatLiveProviderEvalHarnessInput {
            scenario: MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            session_id: "local-http-provider-harness-direct".into(),
            prompt: "Answer in one short sentence: what is this local provider eval proving?"
                .into(),
            explicit_live_eval_requested: true,
            local_only_required: false,
        },
    )
    .await
    .expect("local provider harness report");

    assert!(
        report.ready,
        "local provider harness blocked: {:?}",
        report.blockers
    );
    assert_eq!(report.status, "completed");
    assert_eq!(report.provider_endpoint_kind, "local_test_http");
    assert!(report.live_provider_invocation_allowed);
    assert!(report.main_chat_invoked);
    assert!(report.model_invoked);
    assert!(!report.direct_writes_executed);
    assert!(!report.legacy_fallback_used);
    assert!(report
        .response_preview
        .as_ref()
        .is_some_and(|preview| preview.contains("local provider harness direct answer")));
    assert!(report
        .response_preview
        .as_ref()
        .is_some_and(|preview| preview.chars().all(|ch| !ch.is_control())));

    let evidence = main_chat_live_provider_acceptance_evidence(&[report]);
    assert!(!evidence.generation_eval_executed);
    assert!(evidence.no_silent_writes);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn main_chat_live_provider_eval_harness_preserves_raw_model_identity_for_final_audit() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_live_provider_eval_state_with_local_http_provider(
        &state,
        "local provider harness model identity",
    )
    .await;
    {
        let mut scheduler = state.scheduler.lock().await;
        scheduler.chat_model = "gpt-local-provider-harness ".into();
    }

    let report = run_main_chat_live_provider_eval_harness(
        state,
        MainChatLiveProviderEvalHarnessInput {
            scenario: MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            session_id: "local-http-provider-harness-raw-model".into(),
            prompt: "Answer in one short sentence: what is this local provider eval proving?"
                .into(),
            explicit_live_eval_requested: true,
            local_only_required: false,
        },
    )
    .await
    .expect("local provider harness report");

    assert_eq!(
        report.provider_model.as_deref(),
        Some("gpt-local-provider-harness "),
        "live harness must preserve the raw scheduler model identity so the final gate can reject labels that only become metadata-safe after trimming"
    );
    assert!(
        !report.ready,
        "live harness must not mark a report ready when the raw provider model identity is not metadata-safe"
    );
    assert_eq!(report.status, "failed");
    assert!(report
        .blockers
        .contains(&"live_provider_model_identity_missing".to_string()));
    let evidence = main_chat_live_provider_acceptance_evidence(&[report]);
    assert!(
        !evidence.generation_eval_executed,
        "trim-normalized provider model identity must not receive live generation credit"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn main_chat_live_provider_eval_harness_preserves_raw_provider_identity_for_final_audit() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_live_provider_eval_state_with_local_http_provider(
        &state,
        "local provider harness provider identity",
    )
    .await;
    {
        let mut scheduler = state.scheduler.lock().await;
        scheduler.provider = "openai ".into();
    }

    let report = run_main_chat_live_provider_eval_harness(
        state,
        MainChatLiveProviderEvalHarnessInput {
            scenario: MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            session_id: "local-http-provider-harness-raw-provider".into(),
            prompt: "Answer in one short sentence: what is this local provider eval proving?"
                .into(),
            explicit_live_eval_requested: true,
            local_only_required: false,
        },
    )
    .await
    .expect("local provider harness report");

    assert_eq!(
        report.provider, "openai ",
        "live harness must preserve the raw scheduler provider identity so the final gate can reject labels that only become metadata-safe after trimming"
    );
    assert!(
        !report.ready,
        "live harness must not mark a report ready when the raw provider identity is not metadata-safe"
    );
    assert_eq!(report.status, "failed");
    assert!(report
        .blockers
        .contains(&"live_provider_external_provider_missing".to_string()));
    let evidence = main_chat_live_provider_acceptance_evidence(&[report]);
    assert!(
        !evidence.generation_eval_executed,
        "trim-normalized provider identity must not receive live generation credit"
    );
}

#[tokio::test]
#[ignore = "requires OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1, network, and a real provider API key"]
async fn main_chat_live_provider_eval_harness_invokes_external_direct_answer_when_opted_in() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        config.llm.provider =
            std::env::var("OPENLIFE_LIVE_EVAL_PROVIDER").unwrap_or_else(|_| "openai".into());
        config.llm.openai_base = std::env::var("OPENLIFE_LIVE_EVAL_BASE")
            .unwrap_or_else(|_| "https://api.openai.com/v1".into());
        config.llm.chat_model =
            std::env::var("OPENLIFE_LIVE_EVAL_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
        config.llm.openai_key = std::env::var("OPENLIFE_LIVE_EVAL_API_KEY")
            .unwrap_or_else(|_| std::env::var("OPENAI_API_KEY").unwrap_or_default());
        config.system.network_policy.enabled = true;
    }
    {
        let config = state.config.lock().await.clone();
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            config.local_model.clone(),
            false,
            config.llm.provider.clone(),
            config.llm.openai_base.clone(),
            config.llm.openai_key.clone(),
            config.llm.chat_model.clone(),
            config.llm.embedding_model.clone(),
            false,
        );
    }

    let report = run_main_chat_live_provider_eval_harness(
        state,
        MainChatLiveProviderEvalHarnessInput {
            scenario: MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            session_id: "live-provider-eval-direct-answer".into(),
            prompt: "Answer in one short sentence: what is this live provider eval proving?".into(),
            explicit_live_eval_requested: true,
            local_only_required: false,
        },
    )
    .await
    .expect("live provider harness report");

    assert!(
        report.ready,
        "live provider harness blocked: {:?}",
        report.blockers
    );
    assert_eq!(report.status, "completed");
    assert_eq!(report.provider_endpoint_kind, "external_provider");
    assert!(report.live_provider_invocation_allowed);
    assert!(report.main_chat_invoked);
    assert!(report.model_invoked);
    assert!(!report.direct_writes_executed);
    assert!(!report.legacy_fallback_used);
    assert!(report.run_id.is_some());
    assert!(report.task_session_id.is_some());
    assert!(report
        .response_preview
        .as_ref()
        .is_some_and(|preview| !preview.trim().is_empty()));
    let evidence = main_chat_live_provider_acceptance_evidence(&[report]);
    assert!(evidence.generation_eval_executed);
    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);
    assert!(evidence.no_silent_writes);
}

#[tokio::test]
#[ignore = "requires OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1, network, and a real provider API key"]
async fn main_chat_live_provider_eval_harness_invokes_external_react_web_and_mcp_when_opted_in() {
    let mut reports = Vec::new();
    for scenario in [
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    ] {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        configure_live_provider_eval_state(&state).await;

        let report = run_main_chat_live_provider_eval_harness(
            state,
            MainChatLiveProviderEvalHarnessInput {
                scenario,
                session_id: format!("live-provider-eval-{}", scenario.as_str()),
                prompt: scenario.prompt().into(),
                explicit_live_eval_requested: true,
                local_only_required: false,
            },
        )
        .await
        .expect("live provider harness report");

        if !report.ready {
            eprintln!(
                "live provider harness failed summary: {}",
                serde_json::json!({
                    "scenario": scenario.as_str(),
                    "status": report.status,
                    "blockers": report.blockers,
                    "modelInvoked": report.model_invoked,
                    "agentLoopSucceeded": report.agent_loop_succeeded,
                    "singleStepFallbackUsed": report.single_step_fallback_used,
                    "agentLoopActionStatus": report.agent_loop_action_status,
                    "mcpReadTargetResolved": report.mcp_read_target_resolved,
                    "toolPermissionProposalCreated": report.tool_permission_proposal_created,
                    "toolPermissionProposalTarget": report.tool_permission_proposal_target,
                    "toolSelectionCandidateCount": report.tool_selection_candidate_count,
                    "toolSelectionCandidateIds": report.tool_selection_candidate_ids,
                    "toolSelectionAllowlist": report.tool_selection_allowlist,
                    "toolSelectionAllowedActions": report.tool_selection_allowed_actions,
                    "modelSelectedAllowedTool": report.model_selected_allowed_tool,
                    "modelSelectedExecutionPolicyValidated": report.model_selected_execution_policy_validated,
                    "modelSelectedExecutionAllowed": report.model_selected_execution_allowed,
                    "modelSelectedGovernedArguments": report.model_selected_governed_arguments,
                    "modelSelectedCandidateId": report.model_selected_candidate_id,
                    "modelSelectedCandidateTarget": report.model_selected_candidate_target,
                    "modelSelectedCandidateActionType": report.model_selected_candidate_action_type,
                    "modelSelectedCandidateRank": report.model_selected_candidate_rank,
                    "modelSelectedCandidateSource": report.model_selected_candidate_source,
                    "modelSelectedCandidateCapabilityLabels": report.model_selected_candidate_capability_labels,
                    "modelSelectedCandidateMatchReason": report.model_selected_candidate_match_reason,
                })
            );
        }
        assert!(report.ready, "{scenario:?} blocked: {:?}", report.blockers);
        assert_eq!(report.status, "completed");
        assert_eq!(report.provider_endpoint_kind, "external_provider");
        assert!(report.live_provider_invocation_allowed);
        assert!(report.main_chat_invoked);
        assert!(report.model_invoked);
        assert!(report.agent_loop_succeeded, "{scenario:?}");
        assert!(!report.single_step_fallback_used, "{scenario:?}");
        assert!(
            report.tool_selection_candidate_count > 0,
            "{scenario:?} must expose a bounded governed candidate set"
        );
        assert!(report.model_selected_allowed_tool, "{scenario:?}");
        assert!(
            report.model_selected_execution_policy_validated,
            "{scenario:?}"
        );
        assert!(report.model_selected_execution_allowed, "{scenario:?}");
        assert!(report.model_selected_governed_arguments, "{scenario:?}");
        assert!(
            report
                .model_selected_governed_arguments_digest
                .as_ref()
                .is_some_and(|digest| {
                    let digest = digest.trim();
                    digest.starts_with("bytes:") && digest.contains(" hash:")
                }),
            "{scenario:?} must preserve governed candidate arguments digest evidence"
        );
        assert!(
            report
                .model_selected_candidate_rank
                .is_some_and(|rank| rank > 0),
            "{scenario:?} must preserve selected candidate rank evidence"
        );
        assert!(
            report
                .model_selected_candidate_source
                .as_ref()
                .is_some_and(|source| !source.trim().is_empty()),
            "{scenario:?} must preserve selected candidate source evidence"
        );
        assert!(
            report
                .model_selected_candidate_capabilities_digest
                .as_ref()
                .is_some_and(|digest| {
                    let digest = digest.trim();
                    digest.starts_with("bytes:") && digest.contains(" hash:")
                }),
            "{scenario:?} must preserve selected candidate capability digest evidence"
        );
        assert!(
            report
                .model_selected_candidate_capability_labels
                .as_ref()
                .is_some_and(|labels| labels == "read" || labels.starts_with("read/")),
            "{scenario:?} must preserve bounded safe selected candidate capability labels"
        );
        assert!(
            report
                .model_selected_candidate_match_reason
                .as_ref()
                .is_some_and(|reason| !reason.trim().is_empty()),
            "{scenario:?} must preserve selected candidate match reason evidence"
        );
        let selected_id = report
            .model_selected_candidate_id
            .as_deref()
            .expect("selected candidate id");
        let selected_target = report
            .model_selected_candidate_target
            .as_deref()
            .expect("selected candidate target");
        let selected_action_type = report
            .model_selected_candidate_action_type
            .as_deref()
            .expect("selected candidate action type");
        assert_eq!(
            report.tool_selection_candidate_ids.len(),
            report.tool_selection_candidate_count,
            "{scenario:?} must preserve the bounded candidate id list"
        );
        assert!(
            report
                .tool_selection_candidate_ids
                .iter()
                .any(|candidate_id| candidate_id == selected_id),
            "{scenario:?} must select from the bounded candidate id list"
        );
        assert!(
            report
                .tool_selection_allowlist
                .iter()
                .any(|target| target == selected_target),
            "{scenario:?} must select a target from the exact toolset allowlist"
        );
        assert!(
            report.tool_selection_allowed_actions.iter().any(|action| {
                action.get("actionType").and_then(serde_json::Value::as_str)
                    == Some(selected_action_type)
                    && action.get("target").and_then(serde_json::Value::as_str)
                        == Some(selected_target)
            }),
            "{scenario:?} must select an exact allowed action-target pair"
        );
        assert!(!report.direct_writes_executed);
        assert!(!report.legacy_fallback_used);
        match scenario {
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop => {
                assert_eq!(
                    report.agent_loop_action_status.as_deref(),
                    Some("succeeded")
                );
                assert_eq!(
                    selected_id, selected_target,
                    "web AgentLoop live scenario must prove selected candidate id and target identify the same governed web tool"
                );
            }
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop => {
                assert_eq!(
                    report.agent_loop_action_status.as_deref(),
                    Some("succeeded")
                );
                assert!(report.mcp_read_target_resolved);
                assert!(
                    report.tool_selection_candidate_count >= 2,
                    "registered MCP live scenario must prove bounded multi-candidate model selection"
                );
                let distinct_candidate_ids = report
                    .tool_selection_candidate_ids
                    .iter()
                    .map(String::as_str)
                    .filter(|candidate_id| !candidate_id.trim().is_empty())
                    .collect::<std::collections::BTreeSet<_>>();
                let distinct_allowed_targets = report
                    .tool_selection_allowlist
                    .iter()
                    .map(String::as_str)
                    .filter(|target| !target.trim().is_empty())
                    .collect::<std::collections::BTreeSet<_>>();
                let distinct_allowed_action_pairs = report
                    .tool_selection_allowed_actions
                    .iter()
                    .filter_map(|action| {
                        let action_type = action.get("actionType")?.as_str()?.trim();
                        let target = action.get("target")?.as_str()?.trim();
                        if action_type.is_empty() || target.is_empty() {
                            return None;
                        }
                        Some((action_type, target))
                    })
                    .collect::<std::collections::BTreeSet<_>>();
                assert_eq!(
                    distinct_candidate_ids.len(),
                    report.tool_selection_candidate_count,
                    "registered MCP live scenario must preserve duplicate-free bounded candidate ids"
                );
                assert_eq!(
                    report.tool_selection_allowlist.len(),
                    report.tool_selection_candidate_count,
                    "registered MCP live scenario must preserve complete bounded target allowlist"
                );
                assert_eq!(
                    distinct_allowed_targets.len(),
                    report.tool_selection_candidate_count,
                    "registered MCP live scenario must preserve duplicate-free bounded target allowlist"
                );
                assert_eq!(
                    report.tool_selection_allowed_actions.len(),
                    report.tool_selection_candidate_count,
                    "registered MCP live scenario must preserve complete exact action-target allowlist"
                );
                assert_eq!(
                    distinct_allowed_action_pairs.len(),
                    report.tool_selection_candidate_count,
                    "registered MCP live scenario must preserve duplicate-free exact action-target pairs"
                );
                let action_targets = report
                    .tool_selection_allowed_actions
                    .iter()
                    .filter_map(|action| {
                        let action_type = action.get("actionType")?.as_str()?.trim();
                        let target = action.get("target")?.as_str()?.trim();
                        if action_type == "mcp_tool" && !target.is_empty() {
                            Some(target)
                        } else {
                            None
                        }
                    })
                    .collect::<std::collections::BTreeSet<_>>();
                let candidate_target_labels = report
                    .tool_selection_candidate_ids
                    .iter()
                    .map(String::as_str)
                    .collect::<std::collections::BTreeSet<_>>();
                let allowed_target_labels = report
                    .tool_selection_allowlist
                    .iter()
                    .map(String::as_str)
                    .collect::<std::collections::BTreeSet<_>>();
                assert_eq!(
                    candidate_target_labels, allowed_target_labels,
                    "registered MCP live scenario must preserve candidate ids as the exact target allowlist"
                );
                assert_eq!(
                    candidate_target_labels, action_targets,
                    "registered MCP live scenario must preserve candidate ids as exact MCP action targets"
                );
                assert_eq!(
                    selected_id, selected_target,
                    "registered MCP live scenario must prove selected candidate id and target identify the same MCP candidate"
                );
                assert_eq!(
                    selected_action_type, "mcp_tool",
                    "registered MCP live scenario must select the governed MCP action type"
                );
                assert!(
                    report.tool_selection_model_ranked,
                    "registered MCP live scenario must prove provider-ranked candidate ordering"
                );
                assert_eq!(
                    report.tool_selection_ranking_source.as_deref(),
                    Some("provider_model")
                );
                assert_eq!(
                    report.tool_selection_ranking_route_type.as_deref(),
                    Some("cloud")
                );
                assert_eq!(
                    report.tool_selection_ranking_provider.as_deref(),
                    Some(report.provider.as_str()),
                    "registered MCP live scenario must prove the ranking provider matches the live report provider"
                );
                assert!(
                    report
                        .provider_model
                        .as_ref()
                        .is_some_and(|model| !model.trim().is_empty()),
                    "registered MCP live scenario must preserve the live report model identity"
                );
                assert_eq!(
                    report.tool_selection_ranking_model.as_deref(),
                    report.provider_model.as_deref(),
                    "registered MCP live scenario must prove the ranking model matches the live report model"
                );
                assert!(
                    report.tool_selection_ranking_provider_backed,
                    "registered MCP live scenario must prove provider-backed ranking route"
                );
                assert!(!report.tool_selection_model_ranking_ignored);
                assert_eq!(
                    report.tool_selection_model_ranking_candidate_ids.len(),
                    report.tool_selection_candidate_count,
                    "registered MCP live scenario must preserve a complete provider-ranked candidate permutation"
                );
                assert!(
                    report.tool_selection_model_ranking_candidate_ids.len() >= 2,
                    "registered MCP live scenario must preserve provider-ranked candidate ids"
                );
                let selected_provider_rank = report
                    .tool_selection_model_ranking_candidate_ids
                    .iter()
                    .position(|candidate_id| candidate_id == selected_id)
                    .map(|index| index + 1);
                assert_eq!(
                    report.model_selected_candidate_rank,
                    selected_provider_rank,
                    "registered MCP live scenario must preserve selected candidate rank from the provider-ranked order"
                );
                assert!(
                    report
                        .tool_selection_model_ranking_response_digest
                        .as_ref()
                        .is_some_and(|digest| {
                            let digest = digest.trim();
                            digest.starts_with("bytes:") && digest.contains(" hash:")
                        }),
                    "registered MCP live scenario must preserve provider-ranking response digest"
                );
                assert_eq!(
                    report.model_selected_candidate_match_reason.as_deref(),
                    Some("provider_model_ranked"),
                    "registered MCP live scenario must select a provider-ranked candidate"
                );
            }
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => {
                assert_eq!(
                    report.agent_loop_action_status.as_deref(),
                    Some("needs_confirmation")
                );
                assert!(report.tool_permission_proposal_created);
                assert_eq!(
                    report.model_selected_candidate_id.as_deref(),
                    report.tool_permission_proposal_target.as_deref(),
                    "MCP ToolPermission proposal live scenario must select the governed proposal target candidate id"
                );
                assert_eq!(
                    report.model_selected_candidate_target.as_deref(),
                    report.tool_permission_proposal_target.as_deref(),
                    "MCP ToolPermission proposal live scenario must bind the selected target to the pending proposal target"
                );
                assert_eq!(
                    report.model_selected_candidate_action_type.as_deref(),
                    Some("mcp_tool"),
                    "MCP ToolPermission proposal live scenario must select an MCP tool action"
                );
            }
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer => unreachable!(),
        }
        reports.push(report);
    }

    let evidence = main_chat_live_provider_acceptance_evidence(&reports);
    assert!(!evidence.generation_eval_executed);
    assert!(evidence.web_agent_loop_eval_executed);
    assert!(evidence.mcp_agent_loop_eval_executed);
    assert!(evidence.web_mcp_agent_loop_eval_executed);
    assert!(evidence.proposal_permission_eval_executed);
    assert!(evidence.no_silent_writes);
}
