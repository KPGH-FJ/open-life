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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn main_chat_live_provider_eval_harness_executes_local_http_provider_without_external_live_credit(
) {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_live_provider_eval_state_with_local_http_provider(
        &state,
        "local provider harness direct answer",
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

    let evidence = main_chat_live_provider_acceptance_evidence(&[report]);
    assert!(!evidence.generation_eval_executed);
    assert!(evidence.no_silent_writes);
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
        assert_eq!(
            report.model_selected_candidate_rank,
            Some(1),
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
                .is_some_and(|digest| digest.starts_with("bytes:")),
            "{scenario:?} must preserve selected candidate capability digest evidence"
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
            }
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => {
                assert_eq!(
                    report.agent_loop_action_status.as_deref(),
                    Some("needs_confirmation")
                );
                assert!(report.tool_permission_proposal_created);
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
