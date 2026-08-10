use crate::agent::policy_store::{
    ModelRoutePolicy, PolicyEvaluationRequest, PolicyStore, PolicyTopic,
};
use crate::tool_manifest::{ToolManifest, ToolSource};

#[test]
fn sensitive_policy_ignores_a_requested_cloud_route() {
    let store = PolicyStore::mvp_builtin();

    let decision = store.evaluate_context_policy(PolicyEvaluationRequest {
        topic: PolicyTopic::Health,
        requested_route: ModelRoutePolicy::CloudAllowed,
    });

    assert_eq!(decision.route(), ModelRoutePolicy::LocalOnly);
    assert!(decision.hard_boundary());
    assert_eq!(decision.policy_id(), "policy.sensitive_topics.local_only");
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
