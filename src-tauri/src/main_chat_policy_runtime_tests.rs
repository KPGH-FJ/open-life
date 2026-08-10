use openlife_core::{layer::Layer, llm::ChatMessage};

use crate::main_chat_policy_runtime::{
    build_chat_runtime_policy_context, classify_main_chat_policy_topic,
    main_chat_policy_tool_requirements,
};

fn task(session_id: &str, user_text: &str) -> openlife_core::agent::AgentTask {
    openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.into(),
        user_text: user_text.into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: user_text.into(),
        }],
        layer: Layer::L2,
    }
}

#[test]
fn typed_policy_context_preserves_sensitive_local_only_without_personalization() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let task = task(
        "session-sensitive-policy",
        "最近用药、债务、身份证和分手这些事情让我压力很大",
    );

    let context = build_chat_runtime_policy_context(&state, &task, "").unwrap();

    assert_eq!(
        context.provider_authorization().data_route(),
        openlife_core::llm::ProviderDataRoute::LocalOnly
    );
    assert_eq!(
        context.provider_authorization().authority(),
        openlife_core::llm::ProviderPolicyAuthority::PolicyStore
    );
    assert_eq!(
        context.provider_authorization().policy_version(),
        "policy_store_v1"
    );
    let provenance = context.policy_provenance_refs();
    assert!(provenance.iter().any(|reference| {
        reference.kind()
            == openlife_core::llm::ProviderPolicyProvenanceKind::PolicyStoreRouteDecision
    }));
    assert!(provenance.iter().any(|reference| {
        reference.kind() == openlife_core::llm::ProviderPolicyProvenanceKind::PolicyStorePolicy
    }));
}

#[test]
fn typed_policy_context_preserves_proposal_first_without_personalization() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let task = task(
        "session-write-policy",
        "Please send this email and save the result.",
    );

    let context =
        build_chat_runtime_policy_context(&state, &task, "file.write(path, content)").unwrap();

    assert!(context.external_write_requires_proposal());
    assert!(context.policy_provenance_refs().iter().any(|reference| {
        reference.kind() == openlife_core::llm::ProviderPolicyProvenanceKind::PolicyStorePolicy
    }));
}

#[test]
fn tools_catalog_alone_does_not_create_a_write_policy_requirement() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let user_text = "Summarize my current goals without changing anything.";
    let task = task("session-read-only-policy", user_text);
    let tools_prompt = "file.write email.propose_draft calendar.propose_event";

    assert!(main_chat_policy_tool_requirements(user_text, tools_prompt).is_empty());
    let context = build_chat_runtime_policy_context(&state, &task, tools_prompt).unwrap();

    assert!(!context.external_write_requires_proposal());
}

#[test]
fn policy_provenance_does_not_persist_raw_current_user_text() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let raw_marker = "raw-health-secret-999";
    let task = task(
        "session-policy-audit",
        &format!("{raw_marker} please write a health plan"),
    );

    let context = build_chat_runtime_policy_context(&state, &task, "").unwrap();
    let provenance_json = serde_json::to_string(context.policy_provenance_refs()).unwrap();

    assert!(!provenance_json.contains(raw_marker));
    assert_ne!(
        classify_main_chat_policy_topic(&task.user_text, ""),
        openlife_core::agent::PolicyTopic::General
    );
}
