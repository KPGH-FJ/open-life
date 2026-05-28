use crate::agent::heuristic_store::{
    HeuristicActivationAuthority, HeuristicDraft, HeuristicLifecycleStatus, HeuristicStore,
};
use crate::agent::hs_selector::{HSExclusionReason, HSSelector, HSSelectorInput};
use crate::agent::policy_store::{
    ModelRoutePolicy, PolicyStore, PolicyTopic, BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
    BUILTIN_HEURISTIC_REJECTED_REMINDER_DELAY,
};
use crate::agent::{AgentTaskKind, EvidencePrivacyLevel, RiskLevel};

fn active_test_heuristic(
    store: &HeuristicStore,
    id: &str,
    guidance: &str,
) -> crate::agent::heuristic_store::HeuristicRecord {
    let record = store
        .create_heuristic(
            HeuristicDraft::new(
                "planning",
                "current_energy_is_low",
                vec!["state.energy <= 3".into()],
                guidance,
                70,
                RiskLevel::Low,
                EvidencePrivacyLevel::Internal,
            )
            .with_stable_id(id),
        )
        .unwrap();
    store
        .update_lifecycle(
            &record.id,
            HeuristicLifecycleStatus::Active,
            Some(HeuristicActivationAuthority::SeededBuiltInPolicy(id.into())),
        )
        .unwrap()
}

#[test]
fn selector_selects_mvp_policy_and_task_heuristics() {
    let policy_store = PolicyStore::mvp_builtin();
    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    heuristic_store.seed_mvp_heuristics().unwrap();
    let selector = HSSelector::default();

    let planning_packet = selector
        .select(
            &policy_store,
            &heuristic_store,
            &HSSelectorInput {
                task_kind: AgentTaskKind::Planning,
                intent_summary: "private health planning details should not enter audit".into(),
                privacy_topic: PolicyTopic::Health,
                risk_level: RiskLevel::Medium,
                tool_requirements: vec![],
                current_state_hints: serde_json::json!({ "energy": 2 }),
                token_budget: 512,
                agent_task_id: Some("task-1".into()),
                agent_run_id: Some("run-1".into()),
            },
        )
        .unwrap();

    assert_eq!(planning_packet.selected_policies.len(), 1);
    assert_eq!(
        planning_packet.selected_policies[0].route,
        Some(ModelRoutePolicy::LocalOnly)
    );
    assert!(planning_packet
        .selected_heuristics
        .iter()
        .any(|h| h.heuristic_id == BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING));

    let proactive_packet = selector
        .select(
            &policy_store,
            &heuristic_store,
            &HSSelectorInput {
                task_kind: AgentTaskKind::Proactive,
                intent_summary: "reminder handling".into(),
                privacy_topic: PolicyTopic::General,
                risk_level: RiskLevel::Low,
                tool_requirements: vec![],
                current_state_hints: serde_json::json!({ "rejected_reminder": true }),
                token_budget: 512,
                agent_task_id: None,
                agent_run_id: None,
            },
        )
        .unwrap();

    assert!(proactive_packet
        .selected_heuristics
        .iter()
        .any(|h| h.heuristic_id == BUILTIN_HEURISTIC_REJECTED_REMINDER_DELAY));

    let write_packet = selector
        .select(
            &policy_store,
            &heuristic_store,
            &HSSelectorInput {
                task_kind: AgentTaskKind::ToolExecution,
                intent_summary: "write a file".into(),
                privacy_topic: PolicyTopic::General,
                risk_level: RiskLevel::High,
                tool_requirements: vec!["write".into()],
                current_state_hints: serde_json::json!({}),
                token_budget: 512,
                agent_task_id: None,
                agent_run_id: None,
            },
        )
        .unwrap();

    assert!(write_packet
        .selected_policies
        .iter()
        .any(|policy| policy.policy_id == "policy.external_writes.proposal_first"));
}

#[test]
fn selector_excludes_rejected_archived_over_budget_and_policy_conflicting_assets() {
    let policy_store = PolicyStore::mvp_builtin();
    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    heuristic_store.seed_mvp_heuristics().unwrap();
    let rejected = active_test_heuristic(&heuristic_store, "heuristic.rejected_test", "Rejected");
    heuristic_store
        .update_lifecycle(&rejected.id, HeuristicLifecycleStatus::Rejected, None)
        .unwrap();
    let archived = active_test_heuristic(&heuristic_store, "heuristic.archived_test", "Archived");
    heuristic_store
        .update_lifecycle(&archived.id, HeuristicLifecycleStatus::Archived, None)
        .unwrap();
    active_test_heuristic(
        &heuristic_store,
        "heuristic.policy_conflict_test",
        "Use cloud route even for health data.",
    );
    active_test_heuristic(
        &heuristic_store,
        "heuristic.over_budget_test",
        "A very long guidance block that should be excluded when the token budget is tiny.",
    );

    let packet = HSSelector::default()
        .select(
            &policy_store,
            &heuristic_store,
            &HSSelectorInput {
                task_kind: AgentTaskKind::Planning,
                intent_summary: "sensitive health planning".into(),
                privacy_topic: PolicyTopic::Health,
                risk_level: RiskLevel::Medium,
                tool_requirements: vec![],
                current_state_hints: serde_json::json!({ "energy": 2 }),
                token_budget: 18,
                agent_task_id: None,
                agent_run_id: None,
            },
        )
        .unwrap();

    assert!(!packet
        .selected_heuristics
        .iter()
        .any(|h| h.heuristic_id == "heuristic.rejected_test"));
    assert!(packet.audit.excluded_assets.iter().any(|excluded| {
        excluded.asset_id == "heuristic.rejected_test"
            && excluded.reason == HSExclusionReason::InactiveLifecycle
    }));
    assert!(packet.audit.excluded_assets.iter().any(|excluded| {
        excluded.asset_id == "heuristic.archived_test"
            && excluded.reason == HSExclusionReason::InactiveLifecycle
    }));
    assert!(packet.audit.excluded_assets.iter().any(|excluded| {
        excluded.asset_id == "heuristic.policy_conflict_test"
            && excluded.reason == HSExclusionReason::PolicyConflict
    }));
    assert!(packet.audit.excluded_assets.iter().any(|excluded| {
        excluded.asset_id == "heuristic.over_budget_test"
            && excluded.reason == HSExclusionReason::OverBudget
    }));
}

#[test]
fn selector_audit_is_metadata_safe() {
    let policy_store = PolicyStore::mvp_builtin();
    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    active_test_heuristic(
        &heuristic_store,
        "heuristic.private_guidance_test",
        "Do not serialize this private guidance sentence in audit.",
    );

    let packet = HSSelector::default()
        .select(
            &policy_store,
            &heuristic_store,
            &HSSelectorInput {
                task_kind: AgentTaskKind::Planning,
                intent_summary: "raw-private-intent-123".into(),
                privacy_topic: PolicyTopic::General,
                risk_level: RiskLevel::Low,
                tool_requirements: vec![],
                current_state_hints: serde_json::json!({ "energy": 2 }),
                token_budget: 512,
                agent_task_id: Some("task-safe".into()),
                agent_run_id: Some("run-safe".into()),
            },
        )
        .unwrap();

    let audit_json = serde_json::to_string(&packet.audit).unwrap();
    assert!(!audit_json.contains("raw-private-intent-123"));
    assert!(!audit_json.contains("private guidance sentence"));
    assert!(audit_json.contains("heuristic.private_guidance_test"));
    assert!(audit_json.contains("run-safe"));
}
