use openlife_core::{layer_router::Layer, life_model::LifeModel, llm::ChatMessage};

use crate::{
    main_chat_generation_support::generate_non_stream_fallback,
    main_chat_hs_runtime::{
        build_chat_runtime_hs_packet, classify_hs_policy_topic, hs_tool_requirements,
    },
};

#[test]
fn main_chat_hs_runtime_behavior_tests_are_not_concentrated_in_lib_rs() {
    let lib_rs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");

    for forbidden in [
        "chat_runtime_hs_packet_uses_sanitized_inputs_and_seeded_stores",
        "tools_prompt_catalog_alone_does_not_trigger_external_write_proposal_policy",
        "hs_runtime_fallback_local_only_does_not_fall_back_to_cloud_without_ollama",
        "hs_runtime_topic_keywords_select_sensitive_local_only_policy",
    ] {
        assert!(
            !source.contains(&format!("\n    async fn {forbidden}(")),
            "HS runtime behavior test {forbidden} should live outside src/lib.rs"
        );
    }

    let forbidden = "main_chat_hs_runtime_helpers_are_extracted_from_lib_rs";
    assert!(
        !source.contains(&format!("\n    fn {forbidden}(")),
        "HS runtime extraction guard {forbidden} should live outside src/lib.rs"
    );
}

#[test]
fn main_chat_hs_runtime_helpers_are_extracted_from_lib_rs() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_hs_runtime.rs");

    assert!(
        source.contains("pub(crate) mod main_chat_hs_runtime;"),
        "Main Chat HS runtime helpers must live in a focused module"
    );
    assert!(
        !source.contains("pub(crate) use main_chat_hs_runtime::"),
        "Main Chat HS runtime helpers should be imported from main_chat_hs_runtime directly, not re-exported through src/lib.rs"
    );
    assert!(
        module_path.is_file(),
        "Main Chat HS runtime module file must exist outside lib.rs"
    );
    let module_source =
        std::fs::read_to_string(&module_path).expect("read src/main_chat_hs_runtime.rs");
    assert!(
        module_source.contains("pub(crate) async fn build_chat_runtime_hs_packet("),
        "HS packet builder must be reusable outside src/lib.rs"
    );
    assert!(
        module_source.contains("pub(crate) fn classify_hs_policy_topic("),
        "HS topic classifier must be reusable outside src/lib.rs"
    );
    assert!(
        module_source.contains("pub(crate) fn hs_tool_requirements("),
        "HS tool requirement classifier must be reusable outside src/lib.rs"
    );
    assert!(
        module_source.contains("pub(crate) fn included_life_model_sections("),
        "LifeModel section metadata helper must be reusable outside src/lib.rs"
    );
    assert!(
        !source.contains("\npub(crate) async fn build_chat_runtime_hs_packet("),
        "HS packet builder should not stay concentrated in lib.rs"
    );
    assert!(
        !source.contains("\nfn classify_hs_policy_topic("),
        "HS topic classifier should not stay concentrated in lib.rs"
    );
    assert!(
        !source.contains("\nfn hs_tool_requirements("),
        "HS tool requirement classifier should not stay concentrated in lib.rs"
    );
    assert!(
        !source.contains("\nfn included_life_model_sections("),
        "LifeModel section metadata helper should not stay concentrated in lib.rs"
    );
}

#[tokio::test]
async fn chat_runtime_hs_packet_uses_sanitized_inputs_and_seeded_stores() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let mut life_model = LifeModel::default();
    life_model.state.health_status.energy_level = 2;

    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Planning,
        session_id: "session-chat-hs".into(),
        user_text: "raw-health-secret-999 please write a plan".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "raw-health-secret-999 please write a plan".into(),
        }],
        layer: Layer::L1,
    };

    let packet = build_chat_runtime_hs_packet(
        &state,
        &task,
        &life_model,
        "file.write(path, content)",
        Some("run-chat-hs".into()),
    )
    .await
    .unwrap()
    .expect("planning health write task should select HS assets");

    assert!(packet
        .selected_policies
        .iter()
        .any(|policy| policy.route == Some(openlife_core::agent::ModelRoutePolicy::LocalOnly)));
    assert!(packet
        .audit
        .selected_heuristic_ids
        .contains(&openlife_core::agent::BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING.to_string()));
    assert_eq!(packet.audit.agent_run_id.as_deref(), Some("run-chat-hs"));

    let audit_json = serde_json::to_string(&packet.audit).unwrap();
    assert!(!audit_json.contains("raw-health-secret-999"));
    assert!(!audit_json.contains("Reduce planning intensity"));
}

#[tokio::test]
async fn tools_prompt_catalog_alone_does_not_trigger_external_write_proposal_policy() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let life_model = LifeModel::default();
    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: "session-read-only-tools-catalog".into(),
        user_text: "Summarize what you know about my current goals without changing anything."
            .into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "Summarize what you know about my current goals without changing anything."
                .into(),
        }],
        layer: Layer::L1,
    };
    let tools_prompt = r#"
            Tools:
            file.write_proposal(path, content)
            email.propose_draft(to, subject, body)
            calendar.propose_event(title, scheduled_at)
            write external_side_effect
        "#;

    let requirements = hs_tool_requirements(&task.user_text, tools_prompt);
    assert!(!requirements
        .iter()
        .any(|requirement| requirement == "write"));
    assert!(!requirements
        .iter()
        .any(|requirement| requirement == "external_side_effect"));

    let packet = build_chat_runtime_hs_packet(&state, &task, &life_model, tools_prompt, None)
        .await
        .unwrap();
    if let Some(packet) = packet {
        assert!(!packet.selected_policies.iter().any(|policy| {
            policy.policy_id == openlife_core::agent::BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST
        }));
    }
}

#[tokio::test]
async fn hs_runtime_fallback_local_only_does_not_fall_back_to_cloud_without_ollama() {
    let mut router = openlife_core::agent::ModelRouter::new();
    router.providers.insert(
        "ollama".into(),
        openlife_core::agent::ProviderAvailability {
            provider: "ollama".into(),
            available: false,
            latency_ms: None,
            models: vec![],
            last_checked: chrono::Utc::now(),
            last_error: Some("not running".into()),
            health_is_estimated: false,
        },
    );
    router.providers.insert(
        "openai".into(),
        openlife_core::agent::ProviderAvailability {
            provider: "openai".into(),
            available: true,
            latency_ms: Some(120),
            models: vec!["gpt-4o-mini".into()],
            last_checked: chrono::Utc::now(),
            last_error: None,
            health_is_estimated: false,
        },
    );
    let scheduler = openlife_core::scheduler::InferenceScheduler::new(
        "openlife-test-local-model-that-should-not-exist".into(),
        false,
        "openai".into(),
        "https://api.openai.com/v1".into(),
        "sk-test-cloud-key-present".into(),
        "gpt-4o-mini".into(),
        "text-embedding-3-small".into(),
        true,
    )
    .with_model_router(router);

    let err = generate_non_stream_fallback(
        &scheduler,
        vec![ChatMessage {
            role: "user".into(),
            content: "请处理我的用药记录".into(),
        }],
        &LifeModel::default(),
        "",
        Some(local_only_test_packet()),
    )
    .await
    .unwrap_err();

    assert!(
        err.contains("LocalOnly") || err.contains("local") || err.contains("本地"),
        "unexpected fallback error: {}",
        err
    );
}

#[tokio::test]
async fn hs_runtime_topic_keywords_select_sensitive_local_only_policy() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let life_model = LifeModel::default();
    let text = "最近用药、债务、身份证和分手这些事情让我压力很大";
    let topic = classify_hs_policy_topic(text, "");
    assert_ne!(topic, openlife_core::agent::PolicyTopic::General);

    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: "session-sensitive-zh".into(),
        user_text: text.into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: text.into(),
        }],
        layer: Layer::L2,
    };

    let packet = build_chat_runtime_hs_packet(&state, &task, &life_model, "", None)
        .await
        .unwrap()
        .expect("Chinese sensitive keywords should select HS assets");

    assert!(packet
        .selected_policies
        .iter()
        .any(|policy| policy.route == Some(openlife_core::agent::ModelRoutePolicy::LocalOnly)));
}

fn local_only_test_packet() -> openlife_core::agent::RuntimeHSPacket {
    openlife_core::agent::RuntimeHSPacket {
        selected_policies: vec![openlife_core::agent::SelectedPolicyRef {
            policy_id: openlife_core::agent::BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
            reason: "test_sensitive_topic".into(),
            route: Some(openlife_core::agent::ModelRoutePolicy::LocalOnly),
            digest: "digest".into(),
        }],
        selected_heuristics: vec![],
        guidance_refs: vec![],
        estimated_tokens: 0,
        audit: openlife_core::agent::HSSelectionAudit {
            agent_task_id: None,
            agent_run_id: Some("run-fallback-hs".into()),
            input_digest: "input-digest".into(),
            selected_policy_ids: vec![
                openlife_core::agent::BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
            ],
            selected_heuristic_ids: vec![],
            selected_guidance_ids: vec![],
            selected_guidance_refs: vec![],
            excluded_assets: vec![],
            estimated_tokens: 0,
            token_budget: 128,
        },
    }
}
