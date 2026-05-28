use crate::agent::heuristic_store::{
    HeuristicActivationAuthority, HeuristicDraft, HeuristicLifecycleStatus, HeuristicQuery,
    HeuristicStore, HeuristicValidationState,
};
use crate::agent::EvidencePrivacyLevel;
use crate::agent::RiskLevel;

fn planning_draft() -> HeuristicDraft {
    HeuristicDraft::new(
        "planning",
        "current_energy_is_low",
        vec!["state.energy <= 3".into()],
        "Reduce planning intensity, step count, and pressure.",
        80,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_evidence_ref("ev_low_energy")
    .with_source_proposal("proposal-low-energy")
}

#[test]
fn heuristic_create_query_record_usage_and_fetch_lineage() {
    let store = HeuristicStore::new_in_memory().unwrap();

    let record = store.create_heuristic(planning_draft()).unwrap();

    assert!(record.id.starts_with("hr_"));
    assert_eq!(record.domain, "planning");
    assert_eq!(record.status, HeuristicLifecycleStatus::Candidate);
    assert_eq!(record.version, 1);
    assert_eq!(record.validation_state, HeuristicValidationState::Untested);

    let queried = store
        .query(HeuristicQuery {
            domain: Some("planning".into()),
            status: Some(HeuristicLifecycleStatus::Candidate),
            limit: Some(10),
            ..HeuristicQuery::default()
        })
        .unwrap();
    assert_eq!(queried.len(), 1);
    assert_eq!(queried[0].id, record.id);

    let used = store
        .record_usage(
            &record.id,
            "planning",
            serde_json::json!({ "task_kind": "planning", "energy": "low" }),
        )
        .unwrap();
    assert_eq!(used.usage.usage_count, 1);

    let queried_by_task = store
        .query(HeuristicQuery {
            task_kind: Some("planning".into()),
            ..HeuristicQuery::default()
        })
        .unwrap();
    assert_eq!(queried_by_task.len(), 1);

    let lineage = store.fetch_lineage(&record.id).unwrap();
    assert_eq!(lineage.heuristic_id, record.id);
    assert_eq!(lineage.evidence_refs, vec!["ev_low_energy"]);
    assert_eq!(
        lineage.source_proposal_id.as_deref(),
        Some("proposal-low-energy")
    );
}

#[test]
fn heuristic_lifecycle_requires_governance_for_active_and_rejects_invalid_transitions() {
    let store = HeuristicStore::new_in_memory().unwrap();
    let record = store.create_heuristic(planning_draft()).unwrap();

    let blocked = store.update_lifecycle(&record.id, HeuristicLifecycleStatus::Active, None);
    assert!(blocked.is_err());

    let trial = store
        .update_lifecycle(&record.id, HeuristicLifecycleStatus::Trial, None)
        .unwrap();
    assert_eq!(trial.status, HeuristicLifecycleStatus::Trial);

    let active = store
        .update_lifecycle(
            &record.id,
            HeuristicLifecycleStatus::Active,
            Some(HeuristicActivationAuthority::AcceptedProposal(
                "proposal-low-energy".into(),
            )),
        )
        .unwrap();
    assert_eq!(active.status, HeuristicLifecycleStatus::Active);
    assert!(active.version > trial.version);

    let archived = store
        .update_lifecycle(&record.id, HeuristicLifecycleStatus::Archived, None)
        .unwrap();
    assert_eq!(archived.status, HeuristicLifecycleStatus::Archived);
    let invalid = store.update_lifecycle(
        &record.id,
        HeuristicLifecycleStatus::Active,
        Some(HeuristicActivationAuthority::AcceptedProposal(
            "proposal-low-energy".into(),
        )),
    );
    assert!(invalid.is_err());
}

#[test]
fn high_risk_active_promotion_requires_accepted_governance_metadata() {
    let store = HeuristicStore::new_in_memory().unwrap();
    let high_risk = store
        .create_heuristic(HeuristicDraft::new(
            "identity",
            "user_identity_claim",
            vec!["candidate extracted from conversation".into()],
            "Treat the identity claim as stable.",
            90,
            RiskLevel::High,
            EvidencePrivacyLevel::StrictlyLocal,
        ))
        .unwrap();

    assert!(store
        .update_lifecycle(&high_risk.id, HeuristicLifecycleStatus::Active, None)
        .is_err());

    let active = store
        .update_lifecycle(
            &high_risk.id,
            HeuristicLifecycleStatus::Active,
            Some(HeuristicActivationAuthority::SeededBuiltInPolicy(
                "builtin.identity.reviewed_fixture".into(),
            )),
        )
        .unwrap();
    assert_eq!(active.status, HeuristicLifecycleStatus::Active);
}

#[test]
fn domain_cap_diagnostics_report_default_active_and_trial_caps() {
    let store = HeuristicStore::new_in_memory().unwrap();

    for index in 0..6 {
        let record = store
            .create_heuristic(
                planning_draft()
                    .with_trigger(format!("active_trigger_{index}"))
                    .with_source_proposal(format!("proposal-active-{index}")),
            )
            .unwrap();
        store
            .update_lifecycle(
                &record.id,
                HeuristicLifecycleStatus::Active,
                Some(HeuristicActivationAuthority::AcceptedProposal(format!(
                    "proposal-active-{index}"
                ))),
            )
            .unwrap();
    }

    for index in 0..3 {
        let record = store
            .create_heuristic(
                planning_draft()
                    .with_trigger(format!("trial_trigger_{index}"))
                    .with_source_proposal(format!("proposal-trial-{index}")),
            )
            .unwrap();
        store
            .update_lifecycle(&record.id, HeuristicLifecycleStatus::Trial, None)
            .unwrap();
    }

    let diagnostic = store.diagnose_domain_caps("planning").unwrap();
    assert_eq!(diagnostic.active_count, 6);
    assert_eq!(diagnostic.active_cap, 5);
    assert!(diagnostic.active_cap_exceeded);
    assert_eq!(diagnostic.active_or_trial_count, 9);
    assert_eq!(diagnostic.active_or_trial_cap, 8);
    assert!(diagnostic.active_or_trial_cap_exceeded);
}
