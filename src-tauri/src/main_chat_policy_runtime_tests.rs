use openlife_core::{layer::Layer, llm::ChatMessage};

use crate::main_chat_policy_runtime::{
    build_chat_runtime_policy_packet, classify_main_chat_policy_topic,
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
fn policy_packet_preserves_sensitive_local_only_without_personalization() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let task = task(
        "session-sensitive-policy",
        "最近用药、债务、身份证和分手这些事情让我压力很大",
    );

    let packet = build_chat_runtime_policy_packet(&state, &task, "", None).unwrap();

    assert!(packet
        .selected_policies
        .iter()
        .any(|policy| policy.route == Some(openlife_core::agent::ModelRoutePolicy::LocalOnly)));
    assert!(packet.selected_heuristics.is_empty());
    assert!(packet.guidance_refs.is_empty());
    assert!(packet.audit.selected_heuristic_ids.is_empty());
    assert!(packet.audit.selected_guidance_ids.is_empty());
    assert_eq!(
        packet.provider_authorization().authority(),
        openlife_core::llm::ProviderPolicyAuthority::PolicyStore
    );
    assert_eq!(
        packet.provider_authorization().policy_version(),
        "policy_store_v1"
    );
    let provenance = packet.provider_policy_provenance_refs();
    assert!(provenance.iter().any(|reference| {
        reference.kind()
            == openlife_core::llm::ProviderPolicyProvenanceKind::PolicyStoreRouteDecision
    }));
    assert!(provenance.iter().any(|reference| {
        reference.kind() == openlife_core::llm::ProviderPolicyProvenanceKind::PolicyStorePolicy
    }));
}

#[test]
fn policy_packet_preserves_proposal_first_without_personalization() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let task = task(
        "session-write-policy",
        "Please send this email and save the result.",
    );

    let packet = build_chat_runtime_policy_packet(
        &state,
        &task,
        "file.write(path, content)",
        Some("run-write-policy".into()),
    )
    .unwrap();

    assert!(packet.selected_policies.iter().any(|policy| {
        policy.policy_id == openlife_core::agent::BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST
    }));
    assert_eq!(
        packet.audit.agent_run_id.as_deref(),
        Some("run-write-policy")
    );
    assert!(packet.selected_heuristics.is_empty());
    assert!(packet.guidance_refs.is_empty());
}

#[test]
fn tools_catalog_alone_does_not_create_a_write_policy_requirement() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let user_text = "Summarize my current goals without changing anything.";
    let task = task("session-read-only-policy", user_text);
    let tools_prompt = "file.write email.propose_draft calendar.propose_event";

    assert!(main_chat_policy_tool_requirements(user_text, tools_prompt).is_empty());
    let packet = build_chat_runtime_policy_packet(&state, &task, tools_prompt, None).unwrap();

    assert!(!packet.selected_policies.iter().any(|policy| {
        policy.policy_id == openlife_core::agent::BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST
    }));
    assert!(packet.selected_heuristics.is_empty());
    assert!(packet.guidance_refs.is_empty());
}

#[test]
fn policy_audit_does_not_persist_raw_current_user_text() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let raw_marker = "raw-health-secret-999";
    let task = task(
        "session-policy-audit",
        &format!("{raw_marker} please write a health plan"),
    );

    let packet = build_chat_runtime_policy_packet(&state, &task, "", None).unwrap();
    let audit_json = serde_json::to_string(&packet.audit).unwrap();

    assert!(!audit_json.contains(raw_marker));
    assert_ne!(
        classify_main_chat_policy_topic(&task.user_text, ""),
        openlife_core::agent::PolicyTopic::General
    );
}
