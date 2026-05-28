use crate::agent::heuristic_store::{HeuristicDraft, HeuristicStore};
use crate::agent::policy_store::{PolicyStore, BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING};
use crate::agent::regression_suite::{RegressionSuite, RegressionVerdict};
use crate::agent::{EvidencePrivacyLevel, RiskLevel};

#[test]
fn regression_suite_mvp_scenarios_pass_with_seeded_assets() {
    let policy_store = PolicyStore::mvp_builtin();
    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    heuristic_store.seed_mvp_heuristics().unwrap();

    let results = RegressionSuite::mvp()
        .run_all(&policy_store, &heuristic_store)
        .unwrap();

    assert_eq!(results.len(), 5);
    assert!(results
        .iter()
        .all(|result| result.verdict == RegressionVerdict::Pass));
    assert!(results.iter().any(|result| result
        .asset_ids
        .contains(&"policy.sensitive_topics.local_only".to_string())));
    assert!(results.iter().any(|result| result
        .asset_ids
        .contains(&BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING.to_string())));
}

#[test]
fn regression_candidate_that_violates_local_only_fails() {
    let candidate = HeuristicDraft::new(
        "planning",
        "current_energy_is_low",
        vec!["state.energy <= 3".into()],
        "Use cloud route for this sensitive health planning case.",
        90,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_stable_id("heuristic.bad_cloud_candidate");

    let result = RegressionSuite::mvp().run_candidate_heuristic(&candidate);

    assert_eq!(result.verdict, RegressionVerdict::Fail);
    assert_eq!(result.scenario_id, "regression.local_only_candidate_guard");
    assert!(result
        .asset_ids
        .contains(&"heuristic.bad_cloud_candidate".to_string()));
}

#[test]
fn regression_result_serialization_is_metadata_safe() {
    let raw_prompt = "raw-sensitive-health-prompt-456";
    let raw_guidance = "Use cloud route for this sensitive health planning case.";
    let candidate = HeuristicDraft::new(
        "planning",
        "current_energy_is_low",
        vec![raw_prompt.into()],
        raw_guidance,
        90,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_stable_id("heuristic.metadata_safe_candidate");

    let result = RegressionSuite::mvp().run_candidate_heuristic(&candidate);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(!serialized.contains(raw_prompt));
    assert!(!serialized.contains(raw_guidance));
    assert!(serialized.contains("heuristic.metadata_safe_candidate"));
    assert!(serialized.contains("detailsDigest"));
}
