use crate::agent::{
    ensure_lifemodel_maturation_readiness, evaluate_lifemodel_maturation_readiness, LifeEventDraft,
    LifeModelMaturationReadinessInput,
};

fn low_energy_planning_candidate() -> LifeEventDraft {
    LifeEventDraft::new(
        "preference.planning.low_energy",
        "User prefers low-energy planning with low-pressure next steps.",
    )
    .with_source_run_id("run-readiness")
    .with_metadata(serde_json::json!({
        "confidence": 0.86,
        "proposal_only": true,
        "domain": "low_energy_planning",
        "sourceDigest": "sha256:readiness-source",
    }))
}

fn readiness_input(candidate: LifeEventDraft) -> LifeModelMaturationReadinessInput {
    LifeModelMaturationReadinessInput::for_candidate(candidate)
}

#[test]
fn lifemodel_maturation_readiness_clean_candidate_is_ready() {
    let report =
        evaluate_lifemodel_maturation_readiness(readiness_input(low_energy_planning_candidate()));

    assert!(report.readiness_ready);
    assert!(report.ready);
    assert!(report.default_chat_unchanged);
    assert!(report.ordinary_chat_entrypoint_unchanged);
    assert!(report.runtime_output_candidate_shape_present);
    assert!(report.maturation_service_present);
    assert!(report.evidence_store_present);
    assert!(report.proposal_store_present);
    assert!(report.governor_present);
    assert!(report.proposal_first_required);
    assert!(!report.direct_life_model_write_allowed);
    assert!(!report.direct_memory_write_allowed);
    assert!(!report.heuristic_activation_allowed);
    assert!(report.low_energy_planning_domain_only);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(report.source_lineage_required);
    assert!(report.negative_evidence_required_for_rejection);
    assert!(report.accepted_rule_runtime_packet_future_only);
    assert_eq!(
        report.next_allowed_step,
        "non_default_maturation_invocation"
    );
    assert!(report.blocking_reasons.is_empty());

    ensure_lifemodel_maturation_readiness(readiness_input(low_energy_planning_candidate()))
        .expect("clean readiness should allow the next non-default slice");
}

#[test]
fn lifemodel_maturation_readiness_fails_closed_for_raw_candidate_metadata() {
    let candidate = low_energy_planning_candidate().with_metadata(serde_json::json!({
        "confidence": 0.91,
        "proposal_only": true,
        "domain": "low_energy_planning",
        "rawPrompt": "raw prompt: email jane@example.com with SECRET-123",
        "rawAssistantOutput": "assistant output includes tool result",
        "rawMemoryContext": "memory context: private file body",
        "toolPayload": { "body": "tool payload sk-test-secret" }
    }));

    let report = evaluate_lifemodel_maturation_readiness(readiness_input(candidate));

    assert!(!report.ready);
    assert!(report.metadata_safe);
    assert!(report.contains_raw_content);
    assert!(report
        .blocking_reasons
        .contains(&"candidate_metadata_contains_raw_content".to_string()));
}

#[test]
fn lifemodel_maturation_readiness_fails_closed_for_unsupported_domain() {
    let candidate = LifeEventDraft::new(
        "identity.values",
        "User says independence is a core life value.",
    )
    .with_source_run_id("run-identity")
    .with_metadata(serde_json::json!({
        "confidence": 0.9,
        "proposal_only": true
    }));

    let report = evaluate_lifemodel_maturation_readiness(readiness_input(candidate));

    assert!(!report.ready);
    assert!(report.low_energy_planning_domain_only);
    assert!(report
        .blocking_reasons
        .contains(&"candidate_type_outside_low_energy_planning_domain".to_string()));
}

#[test]
fn lifemodel_maturation_readiness_fails_closed_for_low_confidence() {
    let candidate = low_energy_planning_candidate().with_metadata(serde_json::json!({
        "confidence": 0.2,
        "proposal_only": true,
        "domain": "low_energy_planning"
    }));

    let report = evaluate_lifemodel_maturation_readiness(readiness_input(candidate));

    assert!(!report.ready);
    assert!(report
        .blocking_reasons
        .contains(&"candidate_confidence_too_low".to_string()));
}

#[test]
fn lifemodel_maturation_readiness_fails_closed_for_proposal_only_false() {
    let candidate = low_energy_planning_candidate().with_metadata(serde_json::json!({
        "confidence": 0.86,
        "proposal_only": false,
        "domain": "low_energy_planning"
    }));

    let report = evaluate_lifemodel_maturation_readiness(readiness_input(candidate));

    assert!(!report.ready);
    assert!(report.proposal_first_required);
    assert!(report
        .blocking_reasons
        .contains(&"proposal_only_false".to_string()));
}

#[test]
fn lifemodel_maturation_readiness_reports_no_direct_lifemodel_memory_or_heuristic_writes() {
    let mut input = readiness_input(low_energy_planning_candidate());
    input.require_direct_life_model_write = true;
    input.require_direct_memory_write = true;
    input.require_heuristic_activation = true;

    let report = evaluate_lifemodel_maturation_readiness(input);

    assert!(!report.ready);
    assert!(!report.direct_life_model_write_allowed);
    assert!(!report.direct_memory_write_allowed);
    assert!(!report.heuristic_activation_allowed);
    assert!(report.side_effect_budget_zero);
    assert_eq!(report.side_effect_budget.life_model_writes, 0);
    assert_eq!(report.side_effect_budget.memory_writes, 0);
    assert_eq!(report.side_effect_budget.heuristic_writes, 0);
    assert!(report
        .blocking_reasons
        .contains(&"direct_lifemodel_write_required".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"direct_memory_write_required".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"heuristic_activation_required".to_string()));
}

#[test]
fn lifemodel_maturation_readiness_fails_closed_for_default_chat_migration_or_auto_maturation() {
    let mut input = readiness_input(low_energy_planning_candidate());
    input.default_chat_selected_adapter_path = "controlled_adapter".into();
    input.ordinary_chat_auto_maturation_enabled = true;

    let report = evaluate_lifemodel_maturation_readiness(input);

    assert!(!report.ready);
    assert!(!report.default_chat_unchanged);
    assert!(!report.ordinary_chat_entrypoint_unchanged);
    assert!(report
        .blocking_reasons
        .contains(&"default_chat_route_migration_assumed".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"ordinary_chat_auto_maturation_assumed".to_string()));
}

#[test]
fn lifemodel_maturation_readiness_serialization_omits_raw_prompt_output_memory_tool_and_secrets() {
    let candidate = low_energy_planning_candidate().with_metadata(serde_json::json!({
        "confidence": 0.93,
        "proposal_only": true,
        "domain": "low_energy_planning",
        "rawPrompt": "raw prompt: contact jane@example.com",
        "rawAssistantOutput": "assistant output: send SECRET-123",
        "rawMemoryContext": "memory context: private file body",
        "toolPayload": "tool payload with sk-test-secret"
    }));

    let report = evaluate_lifemodel_maturation_readiness(readiness_input(candidate));
    let serialized = serde_json::to_string(&report).unwrap();
    let debug_dump = format!("{report:?}");

    for forbidden in [
        "raw prompt",
        "assistant output",
        "memory context",
        "tool payload",
        "jane@example.com",
        "SECRET-123",
        "sk-test-secret",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "serialized readiness report leaked {forbidden}"
        );
        assert!(
            !debug_dump.contains(forbidden),
            "debug readiness report leaked {forbidden}"
        );
    }
}
