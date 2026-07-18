use crate::agent::heuristic_store::{HeuristicLifecycleStatus, HeuristicQuery, HeuristicStore};
use crate::agent::policy_store::{
    HeuristicPolicyEffect, ModelRoutePolicy, PolicyEvaluationRequest, PolicyStore, PolicyTopic,
    BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
};
use crate::tool_manifest::{ToolManifest, ToolSource};

#[test]
fn policy_privacy_route_cannot_be_relaxed_by_selected_heuristic() {
    let store = PolicyStore::mvp_builtin();

    let decision = store.evaluate_context_policy(PolicyEvaluationRequest {
        topic: PolicyTopic::Health,
        requested_route: ModelRoutePolicy::CloudAllowed,
        heuristic_effect: Some(HeuristicPolicyEffect {
            heuristic_id: "hr_relax_health".into(),
            requested_route: Some(ModelRoutePolicy::CloudAllowed),
        }),
    });

    assert_eq!(decision.route(), ModelRoutePolicy::LocalOnly);
    assert!(decision.hard_boundary());
    assert_eq!(decision.policy_id(), "policy.sensitive_topics.local_only");
    assert_eq!(decision.conflicts().len(), 1);
    assert_eq!(
        decision.conflicts()[0].heuristic_id.as_deref(),
        Some("hr_relax_health")
    );
    assert!(decision.conflicts()[0].policy_won);
}

#[test]
fn policy_external_writes_remain_proposal_first_until_confirmed() {
    let store = PolicyStore::mvp_builtin();
    let mut manifest = ToolManifest::new(
        "file.write",
        "Direct file write",
        serde_json::json!({}),
        "high",
        "1.0.0",
        ToolSource::BuiltIn,
    )
    .with_capabilities(vec!["filesystem".into(), "write".into()]);
    manifest.action_type = "write".into();

    let unconfirmed = store.evaluate_tool_action(&manifest, false);
    assert!(!unconfirmed.allowed_direct);
    assert!(unconfirmed.proposal_first_required);
    assert_eq!(
        unconfirmed.policy_id,
        "policy.external_writes.proposal_first"
    );

    let confirmed = store.evaluate_tool_action(&manifest, true);
    assert!(confirmed.allowed_direct);
    assert!(!confirmed.proposal_first_required);
}

#[test]
fn low_energy_planning_is_seeded_as_soft_heuristic_not_hard_policy() {
    let policy_store = PolicyStore::mvp_builtin();
    assert!(!policy_store.is_hard_policy_id(BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING));

    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    heuristic_store.seed_mvp_heuristics().unwrap();

    let heuristics = heuristic_store
        .query(HeuristicQuery {
            domain: Some("planning".into()),
            status: Some(HeuristicLifecycleStatus::Active),
            ..HeuristicQuery::default()
        })
        .unwrap();

    let low_energy = heuristics
        .iter()
        .find(|heuristic| heuristic.id == BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING)
        .expect("low-energy planning heuristic should be seeded");
    assert_eq!(low_energy.risk_level.to_string(), "low");
    assert!(low_energy.guidance.contains("Reduce planning intensity"));
}
