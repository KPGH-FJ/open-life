use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::main_chat_kernel::{
    BufferedMainChatEventSink, MainChatKernel, MainChatKernelEvent, MainChatProviderAuthorization,
    MainChatTurnInput,
};
use openlife_core::agent::main_chat_agent_v1::AgentIngress;
use openlife_core::agent::AgentTaskKind;
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;

use crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_captured_local_http_provider;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a workspace parent")
        .to_path_buf()
}

#[test]
fn provider_projection_uses_one_durable_lifecycle_authority_and_deletes_the_combined_route() {
    let root = workspace_root();
    let contract =
        fs::read_to_string(root.join("openlife-core/src/agent/main_chat_runtime_contract.rs"))
            .expect("runtime contract source");
    assert!(!contract.contains("fn provider_from_run("));
    assert!(contract.contains("pub run_identity: Option<String>"));
    assert!(contract.contains("pub provider: Option<ProviderRouteEvidence>"));

    let payload = fs::read_to_string(root.join("src-tauri/src/main_chat_agent_state_payload.rs"))
        .expect("agent-state payload source");
    assert!(payload.contains("latest_provider_event_for_run(run_id)"));
    assert!(!payload.contains("latest_completed_provider_event_for_run(run_id)"));
    assert!(!payload.contains(".model_route"));
    assert!(payload.contains("provider_remote_unknown_has_runtime_cancel_contract(event)"));

    let event_stream = fs::read_to_string(root.join("src-tauri/src/main_chat_event_stream.rs"))
        .expect("event stream source");
    assert!(
        !event_stream.contains(
            "pub(crate) async fn materialize_optional_main_chat_agent_events_with_provider_receipts"
        ),
        "provider lifecycle and snapshot projection must not be recombined into a second materializer"
    );

    let kernel = fs::read_to_string(root.join("src-tauri/src/main_chat_kernel.rs"))
        .expect("Main Chat kernel source");
    let blocked_start = kernel
        .find("async fn build_blocked_kernel_command_surface_result(")
        .expect("blocked kernel result builder");
    let blocked_end = kernel[blocked_start..]
        .find("\nasync fn record_kernel_tool_call_evidence(")
        .map(|offset| blocked_start + offset)
        .expect("blocked kernel result builder boundary");
    let blocked = &kernel[blocked_start..blocked_end];
    let lifecycle_append = blocked
        .find("append_main_chat_provider_receipt_events(")
        .expect("blocked route persists provider lifecycle");
    let state_assembly = blocked
        .find("let preterminal_agent_state")
        .expect("blocked route assembles product state");
    assert!(
        lifecycle_append < state_assembly,
        "blocked route must durably append provider lifecycle before assembling its derived provider projection"
    );
    assert!(blocked.contains("materialize_optional_main_chat_agent_events("));
}

#[test]
fn ordinary_main_chat_uses_the_prepared_provider_seam() {
    let source = fs::read_to_string(workspace_root().join("src-tauri/src/main_chat_kernel.rs"))
        .expect("main_chat_kernel.rs should be readable");
    let client_start = source
        .find("pub struct SchedulerMainChatModelClient")
        .expect("scheduler-backed Main Chat client must exist");
    let client_end = source[client_start..]
        .find("#[derive(Clone)]\nstruct CommandSurfaceDirectReply")
        .map(|offset| client_start + offset)
        .expect("scheduler-backed Main Chat client boundary must remain inspectable");
    let client_source = &source[client_start..client_end];

    assert!(
        client_source.contains(".prepare_chat_request_with_authorized_filter("),
        "ordinary Main Chat must bind typed policy provenance after its final privacy filter"
    );
    assert!(
        client_source.contains(".authorize_derived_payload(")
            && client_source.contains("ProviderPayloadPurpose::MainChatDirectAnswer"),
        "ordinary Main Chat must scope the policy capability to its exact pre-filter payload"
    );
    assert!(
        client_source.contains(".execute_prepared_with_start_observer("),
        "ordinary Main Chat must execute the prepared request at the observable adapter edge"
    );
    assert!(
        !client_source.contains("life_model: LifeModel"),
        "the provider adapter owner must not retain a full LifeModel"
    );
    assert!(
        !client_source.contains(".generate(messages, &self.life_model"),
        "ordinary Main Chat must not call the LifeModel-injecting scheduler route"
    );
    assert!(
        !client_source.contains(".prepare_chat_request("),
        "ordinary Main Chat must not replace an explicit PolicyDecision with the scheduler default route"
    );
}

#[test]
fn prepared_provider_request_is_a_bounded_contract_without_life_model() {
    let source = fs::read_to_string(workspace_root().join("openlife-core/src/llm.rs"))
        .expect("llm.rs should be readable");
    let request_start = source
        .find("pub struct PreparedProviderRequest")
        .expect("PreparedProviderRequest must be defined");
    let request_end = source[request_start..]
        .find("\n}")
        .map(|offset| request_start + offset + 2)
        .expect("PreparedProviderRequest boundary must remain inspectable");
    let request_source = &source[request_start..request_end];

    assert!(request_source.contains("pub messages: Vec<ChatMessage>"));
    assert!(request_source.contains("pub context_manifest: ContextManifest"));
    assert!(request_source.contains("policy_authorization: ProviderPolicyAuthorization"));
    assert!(request_source.contains("pub network_policy: NetworkPolicy"));
    assert!(request_source.contains("pub network_policy_decision: NetworkPolicyDecision"));
    assert!(!request_source.contains("LifeModel"));
    assert!(!request_source.contains("api_key"));
}

#[test]
fn provider_prepare_seam_has_no_self_authorizing_route_api_or_forged_policy_fallback() {
    let root = workspace_root();
    let scheduler = fs::read_to_string(root.join("openlife-core/src/scheduler.rs"))
        .expect("scheduler source should be readable");
    assert!(!scheduler.contains("pub async fn prepare_chat_request("));
    assert!(!scheduler.contains("pub async fn prepare_chat_request_with_policy("));
    assert!(scheduler.contains("policy_authorization: ProviderPolicyAuthorization"));
    assert!(scheduler.contains("bind_prepared_envelope("));
    let scope_validation = scheduler
        .find("policy_authorization.validate_unfiltered_payload(&messages, &context_blocks)?")
        .expect("scheduler must validate exact pre-filter payload scope");
    let provider_selection = scheduler[scope_validation..]
        .find("let data_route = policy_authorization.data_route()")
        .map(|offset| scope_validation + offset)
        .expect("provider selection must remain inspectable");
    assert!(
        scope_validation < provider_selection,
        "payload scope must fail before provider selection or adapter dispatch"
    );

    let llm = fs::read_to_string(root.join("openlife-core/src/llm.rs"))
        .expect("provider policy source should be readable");
    assert!(llm.contains("provider policy exact payload scope cannot be rebound"));
    assert!(llm.contains("missing an exact unfiltered payload scope"));
    assert!(llm.contains("validate_context_truth(&self.context_blocks)?"));

    for relative in [
        "openlife-core/src/agent/runtime.rs",
        "openlife-core/src/agent/agent_loop.rs",
        "openlife-core/src/agent/reasoning/layered.rs",
        "src-tauri/src/main_chat_kernel.rs",
        "src-tauri/src/main_chat_react_tool_selection.rs",
    ] {
        let source = fs::read_to_string(root.join(relative)).expect("provider caller source");
        assert!(
            !source.contains(":policy_allowed\"")
                && !source.contains(":policy_allowed',")
                && !source.contains("format!(\"agent_runtime:{}:policy_allowed")
                && !source.contains("format!(\"agent_loop:{}:policy_allowed"),
            "{relative} must not mint cloud authorization from a caller string"
        );
        assert!(
            source.contains("authorize_derived_payload"),
            "{relative} must bind its typed authorization to the exact outbound payload"
        );
    }
}

#[test]
fn provider_execution_layers_have_no_life_model_accepting_generation_api() {
    let root = workspace_root();
    for relative in [
        "openlife-core/src/scheduler.rs",
        "openlife-core/src/llm.rs",
        "openlife-core/src/ollama.rs",
    ] {
        let source = fs::read_to_string(root.join(relative)).expect("provider source is readable");
        for (offset, _) in source.match_indices("pub ") {
            let signature = source[offset..]
                .split_once('{')
                .map(|(signature, _)| signature)
                .unwrap_or(&source[offset..]);
            assert!(
                !signature.contains("life_model: &LifeModel"),
                "{relative} must not expose a canonical LifeModel in a provider execution API: {signature}"
            );
        }
    }
}

#[test]
fn cloud_provider_http_cannot_bypass_the_bounded_network_reader() {
    let source = fs::read_to_string(workspace_root().join("openlife-core/src/llm.rs"))
        .expect("llm.rs should be readable");

    assert!(source.contains("post_json_text_with_decision_and_start_observer"));
    assert!(source.contains("post_json_stream_with_decision_and_start_observer"));
    assert!(!source.contains("reqwest::Client::builder"));
    assert!(!source.contains("res.text().await"));
}

#[test]
fn shipped_provider_status_probe_uses_the_same_bounded_policy_network_boundary() {
    let source = fs::read_to_string(workspace_root().join("src-tauri/src/commands/router.rs"))
        .expect("router.rs should be readable");

    assert!(source.contains("NetworkClient"));
    assert!(source.contains("resolve_network_policy_decision"));
    assert!(source.contains("network_policy_decision"));
    assert!(!source.contains("reqwest::Client::builder"));
    assert!(!source.contains("res.text().await"));
}

#[test]
fn async_tool_execution_has_no_synchronous_filesystem_calls() {
    let root = workspace_root();
    for relative in [
        "openlife-core/src/agent/action_executor/execution_tools.rs",
        "openlife-core/src/agent/action_executor/tool_executor.rs",
    ] {
        let source = fs::read_to_string(root.join(relative)).expect("tool source is readable");
        assert!(
            !source.contains("std::fs::"),
            "async product tool execution must not block Tokio workers with std::fs in {relative}"
        );
    }
}

#[tokio::test]
async fn provider_chat_status_probe_and_privacy_read_model_share_fail_closed_network_decisions() {
    let cases = [
        (
            "disabled",
            openlife_core::config::NetworkPolicy {
                enabled: false,
                default_decision: "allow".into(),
                ..openlife_core::config::NetworkPolicy::default()
            },
            "network_policy_disabled",
        ),
        (
            "default-deny",
            openlife_core::config::NetworkPolicy {
                default_decision: "deny".into(),
                ..openlife_core::config::NetworkPolicy::default()
            },
            "network_policy_default_deny",
        ),
        (
            "ask-without-consent",
            openlife_core::config::NetworkPolicy::default(),
            "network_policy_consent_required",
        ),
        (
            "denylisted",
            openlife_core::config::NetworkPolicy {
                default_decision: "allow".into(),
                domain_denylist: vec!["127.0.0.1".into()],
                ..openlife_core::config::NetworkPolicy::default()
            },
            "network_domain_denied",
        ),
    ];

    for (case_id, policy, expected_reason) in cases {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let captured =
            configure_live_provider_eval_state_with_captured_local_http_provider(&state, "unused")
                .await;
        {
            let mut config = state.config.lock().await;
            config.system.network_policy = policy.clone();
        }
        *state.provider_health_cache.lock().await = None;

        let endpoint = state.scheduler.lock().await.openai_base.clone();
        let decision = openlife_core::network_client::resolve_network_policy_decision(
            &policy,
            &openlife_core::llm::chat_completions_url("openai", &endpoint),
            "provider.openai",
        )
        .expect("network policy decision");
        assert_eq!(decision.reason_code, expected_reason, "{case_id}");

        let router_status = crate::commands::router::get_model_router_status_with_state(&state)
            .await
            .unwrap_or_else(|error| panic!("{case_id} router status failed: {error}"));
        let cloud = router_status
            .providers
            .iter()
            .find(|provider| provider.name == "openai")
            .expect("cloud provider status");
        assert!(!cloud.available, "{case_id}");
        assert_eq!(
            cloud.network_policy_reason_code.as_deref(),
            Some(expected_reason),
            "{case_id}"
        );

        let _ = crate::main_chat_send::send_message_with_state(
            format!("phase1-policy-{case_id}"),
            vec![ChatMessage {
                role: "user".into(),
                content: "Explain one ordinary fact about the sky in a complete sentence.".into(),
            }],
            None,
            &state,
        )
        .await;
        assert!(
            captured
                .lock()
                .expect("captured provider requests")
                .is_empty(),
            "{case_id} must produce zero provider HTTP dispatches"
        );

        let boundary =
            crate::read_models::provider_privacy::get_provider_privacy_boundary_summary_with_state(
                &state,
            )
            .await
            .expect("provider privacy read model");
        let summary = boundary.data.expect("provider privacy summary");
        assert!(
            summary
                .blocked_reason
                .as_deref()
                .is_some_and(|reason| reason.contains(expected_reason)
                    || (expected_reason == "network_policy_consent_required"
                        && reason.contains("consent"))),
            "{case_id} ProviderPrivacy must reflect the enforced decision: {:?}",
            summary.blocked_reason
        );
        assert!(boundary.evidence_refs.iter().any(|evidence| {
            evidence
                .id
                .contains(&format!("network_policy_decision:{}", decision.decision_id))
        }));
    }
}

#[test]
fn react_provider_ranking_cannot_bypass_the_prepared_provider_seam() {
    let source = fs::read_to_string(
        workspace_root().join("src-tauri/src/main_chat_react_tool_selection.rs"),
    )
    .expect("ReAct tool selection source should be readable");
    let start = source
        .find("pub(crate) async fn rank_main_chat_react_tool_candidates_with_authorization")
        .expect("provider ranking function");
    let end = source[start..]
        .find("\n#[cfg(test)]\npub(crate) async fn rank_main_chat_react_tool_candidates_with_model")
        .map(|offset| start + offset)
        .expect("provider ranking function boundary");
    let ranking_source = &source[start..end];

    assert!(!ranking_source.contains("chat_with_openrouter_raw"));
    assert!(ranking_source.contains("prepare_chat_request_with_authorization"));
    assert!(ranking_source.contains("authorize_derived_payload"));
    assert!(ranking_source.contains("ProviderPayloadPurpose::MainChatReactRanking"));
    assert!(ranking_source.contains("ranking_authorization"));
    assert!(ranking_source.contains("execute_prepared"));
}

#[test]
fn product_tool_execution_cannot_start_an_untracked_provider_call() {
    let root = workspace_root();
    for relative in [
        "openlife-core/src/agent/action_executor/helpers.rs",
        "openlife-core/src/agent/action_executor/execution_tools.rs",
    ] {
        let source = fs::read_to_string(root.join(relative)).expect("tool source is readable");
        for forbidden in [
            "chat_with_ollama_raw",
            "chat_with_openrouter_raw",
            "execute_prepared(",
            "execute_prepared_with_start_observer(",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} must return observations to the active TurnRuntime instead of starting a hidden provider call through {forbidden}"
            );
        }
    }
}

#[test]
fn builder_is_deterministic_candidate_staging_not_a_second_provider_authority() {
    let root = workspace_root();
    let engine = fs::read_to_string(root.join("openlife-core/src/builder/engine.rs"))
        .expect("Builder engine source");
    for forbidden in [
        "InferenceScheduler",
        "PreparedProviderRequest",
        "prepare_chat_request(",
        "prepare_chat_request_with_policy(",
        "generate_prepared(",
        "execute_prepared(",
        "generate_builder_phase",
        "draft_to_life_model",
    ] {
        assert!(
            !engine.contains(forbidden),
            "Builder guided-form candidate staging must not own provider execution symbol {forbidden}"
        );
    }

    let command = fs::read_to_string(root.join("src-tauri/src/commands/builder.rs"))
        .expect("Builder command source");
    assert!(!command.contains("pub async fn builder_apply_signals("));
    let handler = fs::read_to_string(root.join("src-tauri/src/lib.rs")).expect("Tauri handler");
    assert!(!handler.contains("            builder_apply_signals,"));
    let bridge = fs::read_to_string(root.join("frontend/src/tauri.ts")).expect("product bridge");
    assert!(!bridge.contains("builderApplySignals("));
    assert!(!bridge.contains("\"builder_apply_signals\""));
}

#[test]
fn generic_skill_runtime_second_provider_route_is_absent_from_product_surfaces() {
    let root = workspace_root();
    let execution = fs::read_to_string(root.join("src-tauri/src/commands/execution.rs"))
        .expect("execution command source");
    for forbidden in [
        "pub async fn run_skill(",
        "pub async fn get_skill_runtime_status(",
        "pub async fn get_skill_run_status(",
        "pub async fn list_skills(",
        "prepare_chat_request_with_policy(",
        "generate_prepared(",
    ] {
        assert!(
            !execution.contains(forbidden),
            "generic Skill Runtime parallel authority must stay deleted: {forbidden}"
        );
    }
    let handler = fs::read_to_string(root.join("src-tauri/src/lib.rs")).expect("Tauri handler");
    let bridge = fs::read_to_string(root.join("frontend/src/tauri.ts")).expect("product bridge");
    for command in [
        "run_skill",
        "get_skill_runtime_status",
        "get_skill_run_status",
        "list_skills",
    ] {
        assert!(!handler.contains(&format!("            {command},")));
        assert!(!bridge.contains(&format!("\"{command}\"")));
    }

    let bootstrap =
        fs::read_to_string(root.join("src-tauri/src/bootstrap.rs")).expect("bootstrap source");
    assert!(!bootstrap.contains("as_plugin_declarative_only"));
    assert!(!execution.contains("as_plugin_declarative_only"));
    let catalog = fs::read_to_string(root.join("src-tauri/src/main_chat_skills_tools.rs"))
        .expect("Main Chat skill catalog source");
    assert!(catalog.contains("main_chat_turn_runtime_native"));
    assert!(!catalog.contains("available: !manifest.execution_budget.allow_writes"));

    let core_catalog = fs::read_to_string(root.join("openlife-core/src/skills.rs"))
        .expect("core skill catalog source");
    for retained_catalog_symbol in [
        "pub struct SkillManifest",
        "pub struct SkillRegistry",
        "pub fn built_in()",
        "weekly_review",
        "goal_breakdown",
        "memory_consolidation",
    ] {
        assert!(
            core_catalog.contains(retained_catalog_symbol),
            "the product manifest/catalog consumer must remain: {retained_catalog_symbol}"
        );
    }
    for retired_runtime_symbol in [
        "pub struct SkillJsonEnvelope",
        "pub struct SkillProposalCandidate",
        "pub struct SkillRunResult",
        "pub struct SkillRuntimeDescriptor",
        "pub struct SkillRuntimeReadinessReport",
        "pub struct SkillContext",
        "pub struct NormalizedSkillOutput",
        "pub struct GovernedSkillProposalCandidate",
        "pub fn build_skill_prompt(",
        "pub fn parse_skill_json(",
        "pub fn validate_skill_envelope(",
        "pub fn evaluate_skill_runtime_readiness(",
        "pub fn assemble_skill_context(",
        "pub fn build_skill_context(",
        "pub fn normalize_skill_output(",
        "pub fn govern_skill_proposal_candidates(",
        "ProposalSource::SkillRuntime",
    ] {
        assert!(
            !core_catalog.contains(retired_runtime_symbol),
            "self-consumed generic Skill Runtime residue must stay deleted: {retired_runtime_symbol}"
        );
    }
    assert!(core_catalog.contains("catalog_only_unavailable"));
    assert!(!core_catalog.contains("你必须严格输出以下 JSON envelope"));
}

#[test]
fn provider_settings_test_cannot_reuse_a_masked_secret_across_endpoints() {
    let root = workspace_root();
    let settings = fs::read_to_string(root.join("src-tauri/src/commands/settings.rs"))
        .expect("settings source is readable");
    let handler = fs::read_to_string(root.join("src-tauri/src/lib.rs"))
        .expect("Tauri handler source is readable");

    assert!(!settings.contains("pub async fn test_api_key"));
    assert!(!handler.contains("            test_api_key,"));
    assert!(settings.contains("resolve_submitted_provider_api_key(&config, &current_config)"));
    assert!(settings.contains("effective_api_key_for_endpoint("));
    let llm = fs::read_to_string(root.join("openlife-core/src/llm.rs"))
        .expect("provider adapter source is readable");
    let secret_store = fs::read_to_string(root.join("src-tauri/src/secret_store.rs"))
        .expect("secret store source is readable");
    assert!(llm.contains("provider_endpoint_is_official"));
    assert!(!llm.contains("pub fn effective_api_key(provider:"));
    assert!(settings.contains("bind_explicit_provider_probe_scheduler(probe_scheduler)"));
    assert!(settings.contains("prepare_explicit_provider_probe(probe_grant)"));
    assert!(settings.contains("probe_scheduler.execute_prepared(prepared).await"));
    assert!(settings.contains("provider_validation_record_with_terminal_proof("));
    assert!(!settings.contains("provider_validation_record_with_receipt("));
    assert!(settings.contains("proof.receipt() == observed"));
    assert!(settings.contains("provider_invocation_receipt: receipt"));
    assert!(settings.contains("&backend_network_policy,"));
    assert!(settings.contains("&network_policy_decision,"));
    assert!(!settings.contains("post_json_text_with_decision_and_start_observer("));
    assert!(!settings.contains("provider_validation_response_has_content(&res.body)"));
    for binding_field in [
        "provider: String",
        "scheme: String",
        "host: String",
        "port: u16",
        "base_path: String",
        "credential_identity: String",
        "credential_version: u64",
    ] {
        assert!(
            secret_store.contains(binding_field),
            "provider keychain envelope lost binding field {binding_field}"
        );
    }
    assert!(secret_store.contains("hydrate_bound_provider_secret(config, &encoded)"));
    assert!(secret_store.contains("config.llm.openai_key_ref = None"));
}

#[tokio::test]
async fn provider_failure_has_a_failed_receipt_and_no_final_answer() {
    let scheduler = InferenceScheduler::new(
        "openlife-local-model-that-does-not-exist".into(),
        false,
        "openai".into(),
        "http://127.0.0.1:9/v1".into(),
        "test-key".into(),
        "test-model".into(),
        "test-embedding".into(),
        false,
    );
    let kernel = MainChatKernel::with_scheduler(scheduler);
    let mut events = BufferedMainChatEventSink::default();

    let result = kernel
        .run_turn(
            MainChatTurnInput {
                session_id: "phase1-provider-failure".into(),
                messages: vec![ChatMessage {
                    role: "user".into(),
                    content: "Give one ordinary sentence about the sky.".into(),
                }],
                provider_authorization: MainChatProviderAuthorization::test_fixture_for_user_text(
                    "phase1-provider-failure-policy",
                    true,
                    "Give one ordinary sentence about the sky.",
                ),
                selected_skill_id: None,
                policy_decision: AgentIngress::default()
                    .decide(
                        "phase1-provider-failure",
                        "Give one ordinary sentence about the sky.",
                        None,
                        AgentTaskKind::Conversation,
                    )
                    .policy_decision,
                model_supplied_tool_arguments: None,
                runtime_fact_direct_answer: false,
            },
            &mut events,
        )
        .await;

    assert!(
        result
            .blockers
            .contains(&"model_generation_failed".to_string()),
        "unexpected result blockers: {:?}; events: {:?}",
        result.blockers,
        events.events()
    );
    assert!(events
        .events()
        .iter()
        .any(|event| matches!(event, MainChatKernelEvent::ProviderStarted { .. })));
    assert!(events
        .events()
        .iter()
        .any(|event| matches!(event, MainChatKernelEvent::ProviderFailed { .. })));
    assert!(!events
        .events()
        .iter()
        .any(|event| matches!(event, MainChatKernelEvent::ProviderCompleted { .. })));
    assert!(!events
        .events()
        .iter()
        .any(|event| matches!(event, MainChatKernelEvent::FinalAnswer { .. })));
}

#[tokio::test]
async fn direct_answer_local_only_never_contacts_cloud_when_local_model_is_unavailable() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let captured_requests = configure_live_provider_eval_state_with_captured_local_http_provider(
        &state,
        "cloud response that policy must not request",
    )
    .await;
    {
        let mut config = state.config.lock().await;
        config.local_model = "openlife-local-model-that-does-not-exist".into();
    }
    {
        let config = state.config.lock().await.clone();
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = InferenceScheduler::new(
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

    let result = crate::main_chat_send::send_message_with_state(
        "phase1-direct-local-only".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "Help me understand this private medical concern in one paragraph.".into(),
        }],
        None,
        &state,
    )
    .await
    .expect("local-only failure returns structured turn state");

    assert!(
        result
            .agent_ingress
            .as_ref()
            .is_some_and(|decision| decision.privacy_risk.local_only_required),
        "the policy decision must require LocalOnly before provider routing"
    );
    assert!(
        captured_requests
            .lock()
            .expect("read captured provider requests")
            .is_empty(),
        "a LocalOnly direct answer must never fall back to an available cloud endpoint"
    );
    assert_ne!(result.status, "completed");
    assert!(!result.model_invoked);
}

#[tokio::test]
async fn direct_answer_rejects_trailing_non_user_message_before_cloud_dispatch() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let captured_requests = configure_live_provider_eval_state_with_captured_local_http_provider(
        &state,
        "cloud response that a malformed turn must not request",
    )
    .await;

    let result = crate::main_chat_send::send_message_with_state(
        "phase1-trailing-non-user".into(),
        vec![
            ChatMessage {
                role: "user".into(),
                content: "My private medical diagnosis is sentinel-diagnosis-4831.".into(),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "This trailing history item is not a current authenticated user message."
                    .into(),
            },
        ],
        None,
        &state,
    )
    .await;

    assert!(
        result.is_err(),
        "a turn whose final item is not the current user message must fail closed"
    );
    assert!(
        captured_requests
            .lock()
            .expect("read captured malformed-turn provider requests")
            .is_empty(),
        "sensitive history must not reach a cloud provider when the current-message boundary is malformed"
    );
}

#[tokio::test]
async fn sensitive_history_escalates_the_selected_provider_context_to_local_only() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let captured_requests = configure_live_provider_eval_state_with_captured_local_http_provider(
        &state,
        "cloud response that selected sensitive context must not request",
    )
    .await;
    {
        let mut config = state.config.lock().await;
        config.local_model = "openlife-local-model-that-does-not-exist".into();
    }
    {
        let config = state.config.lock().await.clone();
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = InferenceScheduler::new(
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

    let result = crate::main_chat_send::send_message_with_state(
        "phase1-sensitive-history".into(),
        vec![
            ChatMessage {
                role: "user".into(),
                content: "My private medical diagnosis needs careful explanation.".into(),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "I can help explain it carefully.".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: "Continue with one short sentence.".into(),
            },
        ],
        None,
        &state,
    )
    .await
    .expect("selected-context local-only failure returns structured state");

    assert!(
        result
            .blockers
            .iter()
            .any(|blocker| blocker == "model_generation_failed"),
        "the unavailable local route must fail closed instead of silently using cloud: {:?}",
        result.blockers
    );
    assert!(
        captured_requests
            .lock()
            .expect("read sensitive-history provider requests")
            .is_empty(),
        "a cloud request must not contain an earlier sensitive turn selected into context"
    );
}

#[tokio::test]
async fn cloud_boundary_reapplies_one_privacy_policy_to_all_message_roles_across_turns() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let captured_requests = configure_live_provider_eval_state_with_captured_local_http_provider(
        &state,
        "I noted <EMAIL_0>.",
    )
    .await;
    let private_email = "phase1-history-sentinel@example.com";
    let first_user = ChatMessage {
        role: "user".into(),
        content: format!("Contact me at {private_email}."),
    };
    let first = crate::main_chat_send::send_message_with_state(
        "phase1-all-role-privacy-first".into(),
        vec![first_user.clone()],
        None,
        &state,
    )
    .await
    .expect("first cloud turn succeeds through the capture adapter");
    assert!(
        first.reply.contains(private_email),
        "local reconstruction must remain usable for the user; reply={:?}",
        first.reply
    );

    crate::main_chat_send::send_message_with_state(
        "phase1-all-role-privacy-second".into(),
        vec![
            first_user,
            ChatMessage {
                role: "assistant".into(),
                content: first.reply,
            },
            ChatMessage {
                role: "user".into(),
                content: "Continue without repeating my contact details.".into(),
            },
        ],
        None,
        &state,
    )
    .await
    .expect("second cloud turn succeeds through the capture adapter");

    let requests = captured_requests
        .lock()
        .expect("read all-role privacy capture");
    assert_eq!(requests.len(), 2, "both cloud calls must be observed");
    for (index, request) in requests.iter().enumerate() {
        assert!(
            !request.contains(private_email),
            "cloud request {index} leaked reconstructed assistant/user history"
        );
    }
}

#[tokio::test]
async fn direct_answer_policy_allowed_still_uses_configured_cloud_provider() {
    const RAW_LIFEMODEL_SENTINEL: &str = "PHASE1_RAW_LIFEMODEL_WIRE_SENTINEL";
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let captured_requests = configure_live_provider_eval_state_with_captured_local_http_provider(
        &state,
        "bounded cloud answer",
    )
    .await;
    {
        let manager = state.life_model_manager.lock().await;
        let mut life_model = manager.load().expect("load isolated LifeModel");
        life_model.identity.name = RAW_LIFEMODEL_SENTINEL.into();
        life_model.identity.mission_statement = RAW_LIFEMODEL_SENTINEL.into();
        manager
            .save(&life_model)
            .expect("save wire-privacy LifeModel sentinel");
    }
    {
        let mut config = state.config.lock().await;
        config.local_model = "openlife-local-model-that-does-not-exist".into();
    }
    {
        let config = state.config.lock().await.clone();
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = InferenceScheduler::new(
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

    let result = crate::main_chat_send::send_message_with_state(
        "phase1-direct-policy-allowed".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "Explain focused work in one sentence.".into(),
        }],
        None,
        &state,
    )
    .await
    .expect("policy-allowed direct answer succeeds");

    assert!(result
        .agent_ingress
        .as_ref()
        .is_some_and(|decision| !decision.privacy_risk.local_only_required));
    assert_eq!(result.status, "completed");
    assert_eq!(result.reply, "bounded cloud answer");
    let request = captured_requests
        .lock()
        .expect("read captured provider requests")
        .first()
        .cloned()
        .expect("one captured cloud request");
    assert_eq!(
        captured_requests
            .lock()
            .expect("re-read captured provider requests")
            .len(),
        1,
        "a low-risk request must retain configured cloud capability"
    );
    assert!(
        !request.contains(RAW_LIFEMODEL_SENTINEL),
        "the actual HTTP request must not contain raw LifeModel fields"
    );
    let (headers, body) = request
        .split_once("\r\n\r\n")
        .expect("captured HTTP request has headers and body");
    let wire_request_id = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("x-openlife-request-id")
                .then(|| value.trim().to_string())
        })
        .expect("provider wire request carries its receipt correlation id");
    uuid::Uuid::parse_str(&wire_request_id).expect("wire request id is UUID");
    let body: serde_json::Value = serde_json::from_str(body).expect("provider body is JSON");

    let task_session_id = result
        .agent_ingress
        .as_ref()
        .and_then(|decision| decision.agent_task_session_id.as_deref())
        .expect("policy-allowed task session id");
    let durable_events = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id.to_string(),
        None,
        Some(100),
    )
    .await
    .expect("list policy-allowed provider facts");
    let started = durable_events
        .iter()
        .find(|event| event.event_type == "provider.started")
        .expect("durable provider start fact");
    let completed = durable_events
        .iter()
        .find(|event| event.event_type == "provider.completed")
        .expect("durable provider completion fact");
    assert_eq!(started.object_id, wire_request_id);
    assert_eq!(completed.object_id, wire_request_id);
    assert_eq!(
        started
            .payload
            .get("provider")
            .and_then(serde_json::Value::as_str),
        Some("openai")
    );
    assert_eq!(
        started
            .payload
            .get("model")
            .and_then(serde_json::Value::as_str),
        body.get("model").and_then(serde_json::Value::as_str)
    );
    let persisted_run = state
        .agent_run_store
        .as_ref()
        .expect("agent run store")
        .lock()
        .await
        .get_run(&started.run_id)
        .expect("load provider-correlated run")
        .expect("provider-correlated run exists");
    let actual_route = persisted_run
        .model_route
        .expect("completed provider run has actual route metadata");
    assert_eq!(actual_route.provider, "openai");
    assert_eq!(actual_route.model, body["model"].as_str().unwrap());
    assert_eq!(actual_route.route_type, "cloud");
    assert_eq!(actual_route.reason, "provider_adapter_receipt");
}

#[tokio::test]
async fn provider_dispatch_without_observed_response_is_durable_as_remote_unknown() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        config.local_model = "openlife-local-model-that-does-not-exist".into();
        config.llm.provider = "openai".into();
        config.llm.openai_base = "http://127.0.0.1:9/v1".into();
        config.llm.openai_key = "test-key".into();
        config.llm.chat_model = "gpt-unreachable".into();
        config.system.network_policy.default_decision = "allow".into();
    }
    {
        let config = state.config.lock().await.clone();
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = InferenceScheduler::new(
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

    let result = crate::main_chat_send::send_message_with_state(
        "phase1-durable-provider-failure".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "Explain one ordinary idea in one sentence.".into(),
        }],
        None,
        &state,
    )
    .await
    .expect("provider failure returns structured command result");
    assert_ne!(result.status, "completed");

    let task_session_id = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .list_sessions(None, 20, 0)
            .expect("list task sessions")
            .into_iter()
            .find(|session| session.chat_session_id == "phase1-durable-provider-failure")
            .expect("failed task session")
            .id
    };
    let events = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id,
        None,
        Some(100),
    )
    .await
    .expect("list durable provider facts");
    let event_types = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"provider.started"));
    assert!(event_types.contains(&"provider.remote_unknown"));
    assert!(!event_types.contains(&"provider.failed"));
    assert!(!event_types.contains(&"provider.completed"));
}

fn directory_contains_bytes(root: &std::path::Path, needle: &[u8]) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if directory_contains_bytes(&path, needle) {
                return true;
            }
        } else if fs::read(&path)
            .ok()
            .is_some_and(|bytes| bytes.windows(needle.len()).any(|window| window == needle))
        {
            return true;
        }
    }
    false
}

#[tokio::test]
async fn sensitive_body_exists_only_in_its_canonical_owner_across_execution_stores() {
    const SENTINEL: &str = "PRIVATE-CANONICAL-BODY-7Q9X-NOT-AN-EXECUTION-PAYLOAD";
    let root = tempfile::tempdir().expect("isolated cross-store sentinel root");
    let memory_dir = root.path().join("memory-owner");
    let run_dir = root.path().join("agent-run-projection");
    let session_dir = root.path().join("task-session-projection");
    let life_event_dir = root.path().join("life-event-projection");
    let event_dir = root.path().join("turn-event-projection");
    let audit_dir = root.path().join("mcp-audit-projection");
    for directory in [
        &memory_dir,
        &run_dir,
        &session_dir,
        &life_event_dir,
        &event_dir,
        &audit_dir,
    ] {
        fs::create_dir_all(directory).expect("create isolated store directory");
    }

    let memory_store = openlife_core::memory::MemoryStore::new(memory_dir.join("memory.db"))
        .expect("canonical conversation store");
    memory_store
        .save_message(
            "sentinel-session",
            &ChatMessage {
                role: "user".into(),
                content: SENTINEL.into(),
            },
        )
        .expect("save canonical conversation body");

    let agent_run_store = openlife_core::agent::AgentRunStore::new(run_dir.join("agent-runs.db"))
        .expect("AgentRun projection store");
    let mut run = openlife_core::agent::AgentRun::new_chat_run("sentinel-session", SENTINEL);
    run.context_summary = Some(openlife_core::agent::ContextSummary {
        life_model_empty: true,
        included_life_model_sections: Vec::new(),
        memory_hit_count: 1,
        memory_sources: vec![format!("memory://{SENTINEL}/forged")],
        used_tools_prompt: false,
        redaction_applied: true,
        redaction_level: openlife_core::agent::RedactionLevel::Strict,
    });
    agent_run_store
        .create_run(&run)
        .expect("create minimized AgentRun projection");

    let session_store = openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStore::new(
        session_dir.join("task-sessions.db"),
    )
    .expect("task session projection store");
    let task_session = session_store
        .create_session(
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionDraft {
                chat_session_id: "sentinel-session".into(),
                user_goal: SENTINEL.into(),
                selected_strategy:
                    openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::PlanExecute,
                current_plan_summary: Some(SENTINEL.into()),
                context_snapshot_refs: vec![format!("memory://{SENTINEL}/forged")],
            },
        )
        .expect("create minimized task-session projection");
    session_store
        .set_pending_blockers(&task_session.id, vec![SENTINEL.into()])
        .expect("minimize task-session blocker");
    session_store
        .append_transcript_entry(
            openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryDraft {
                session_id: task_session.id.clone(),
                kind: openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Plan,
                summary: SENTINEL.into(),
                metadata: serde_json::json!({
                    "unknownPrivateField": SENTINEL,
                    "rawUserTextPreview": SENTINEL
                }),
            },
        )
        .expect("create minimized transcript projection");

    let life_event_store =
        openlife_core::agent::LifeEventStore::new(life_event_dir.join("life-events.db"))
            .expect("LifeEvent projection store");
    life_event_store
        .create_canonical_agent_run_event_for_test(
            &agent_run_store,
            &run.id,
            Some("cross_store_sentinel_test"),
            openlife_core::agent::LifeEventDraft::new("preference.planning.low_energy", SENTINEL)
                .with_source_run_id(&run.id)
                .with_metadata(serde_json::json!({
                    "confidence": 0.8,
                    "proposal_only": true,
                    "unknownPrivateField": SENTINEL,
                    "rawEvidencePreview": SENTINEL
                })),
            openlife_core::agent::LifeDomain::LowEnergyPlanning,
            openlife_core::agent::RiskLevel::Low,
            openlife_core::agent::LifeEventPrivacyLevel::Internal,
        )
        .expect("create minimized LifeEvent projection");

    let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    Arc::get_mut(&mut state)
        .expect("isolated state has one owner")
        .main_chat_agent_event_store = Some(Arc::new(tokio::sync::Mutex::new(
        crate::main_chat_event_stream::MainChatAgentEventStore::new(
            event_dir.join("turn-events.db"),
        )
        .expect("durable event projection store"),
    )));
    crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
        &state,
        "sentinel-task",
        run.id.clone(),
        "turn_started",
        "turn",
        run.id.clone(),
        "cross_store_sentinel_test",
        serde_json::json!({
            "status": "started",
            "rawUserText": SENTINEL,
            SENTINEL: true,
            "rawUserTextStored": false,
        }),
    )
    .await
    .expect("append minimized turn fact");

    let audit_store = openlife_core::mcp_audit::McpAuditStore::with_key_materials(
        audit_dir.join("mcp-audit.db"),
        vec![openlife_core::mcp_audit::AuditKeyMaterial {
            config: openlife_core::mcp_audit::AuditKeyConfig {
                mode: openlife_core::mcp_audit::KeyMode::Keychain,
                salt_b64: None,
                env_var: None,
                key_ref: Some("keychain://cross-store-sentinel/epoch-1".into()),
                epoch: 1,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
            key: rand::random(),
        }],
    )
    .expect("random audit key material");
    audit_store
        .insert_log(
            "sentinel_tool",
            &serde_json::json!({"body": SENTINEL}),
            SENTINEL,
            false,
            true,
        )
        .expect("insert minimized encrypted audit receipt");

    let needle = SENTINEL.as_bytes();
    assert!(
        directory_contains_bytes(&memory_dir, needle),
        "the canonical conversation owner must retain the user body"
    );
    for projection in [
        &run_dir,
        &session_dir,
        &life_event_dir,
        &event_dir,
        &audit_dir,
    ] {
        assert!(
            !directory_contains_bytes(projection, needle),
            "execution projection copied canonical body into {}",
            projection.display()
        );
    }
}
