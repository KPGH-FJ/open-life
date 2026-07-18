use crate::agent::{
    complete_collaboration_guidance_cutover, digest_string,
    reconcile_collaboration_guidance_authority, CollaborationGuidanceCutoverStatus,
    EvidencePrivacyLevel, HSAssetAuthorityRegistry, HSAssetCategory, HSAssetOwner,
    HSAssetWriteKind, HSAssetWriteRequest, HeuristicDraft, HeuristicLifecycleStatus,
    HeuristicStore, PolicyStore, RiskLevel,
};
use crate::life_model::LifeModel;
use std::sync::Arc;

fn seeded_store() -> HeuristicStore {
    let store = HeuristicStore::new_in_memory().unwrap();
    store.seed_mvp_heuristics().unwrap();
    store
}

fn fixture_scenario(
    registry: &HSAssetAuthorityRegistry,
    revision: i64,
) -> crate::agent::ProductScenarioReceipt {
    registry
        .record_product_scenario(
            HSAssetCategory::CollaborationGuidance,
            revision,
            "test-fixture:receipt-shape-only-no-live-credit",
            HSAssetOwner::AcceptedHsStore,
            &["builtin.low_energy_planning".into()],
            digest_string("metadata-safe-runtime-audit"),
        )
        .unwrap()
}

fn proof_set(
    registry: &HSAssetAuthorityRegistry,
) -> (
    crate::agent::ShadowParityReceipt,
    crate::agent::RollbackRehearsalReceipt,
    crate::agent::ProductScenarioReceipt,
) {
    let revision = registry
        .authority(HSAssetCategory::CollaborationGuidance)
        .unwrap()
        .revision;
    let digest = digest_string("same-projection");
    let parity = registry
        .record_shadow_parity(
            HSAssetCategory::CollaborationGuidance,
            revision,
            digest.clone(),
            digest.clone(),
            digest,
        )
        .unwrap();
    let rollback = registry
        .rehearse_rollback(HSAssetCategory::CollaborationGuidance, revision)
        .unwrap();
    let scenario = fixture_scenario(registry, revision);
    (parity, rollback, scenario)
}

#[test]
fn authority_registry_starts_every_unmigrated_asset_category_on_yaml() {
    let registry = HSAssetAuthorityRegistry::new_in_memory().unwrap();
    let records = registry.list_authorities().unwrap();

    assert_eq!(records.len(), HSAssetCategory::ALL.len());
    assert!(records
        .iter()
        .all(|record| record.owner == HSAssetOwner::LifeModelYaml && record.revision == 1));
}

#[test]
fn shadow_preparation_does_not_promote_without_durable_runtime_evidence() {
    let registry = HSAssetAuthorityRegistry::new_in_memory().unwrap();
    let store = seeded_store();

    let report =
        reconcile_collaboration_guidance_authority(&registry, &LifeModel::default_model(), &store)
            .unwrap();

    assert_eq!(
        report.status,
        CollaborationGuidanceCutoverStatus::ShadowEvidencePending
    );
    assert_eq!(report.authority.owner, HSAssetOwner::LifeModelYaml);
    assert_eq!(
        registry
            .authority(HSAssetCategory::CollaborationGuidance)
            .unwrap()
            .owner,
        HSAssetOwner::LifeModelYaml
    );
}

#[test]
fn fixture_receipt_exercises_one_category_cutover_mechanics_without_live_credit() {
    let registry = HSAssetAuthorityRegistry::new_in_memory().unwrap();
    let store = seeded_store();
    let scenario = fixture_scenario(&registry, 1);

    let report = complete_collaboration_guidance_cutover(
        &registry,
        &LifeModel::default_model(),
        &store,
        &scenario,
    )
    .unwrap();

    assert_eq!(report.status, CollaborationGuidanceCutoverStatus::Promoted);
    assert_eq!(report.authority.owner, HSAssetOwner::AcceptedHsStore);
    for category in HSAssetCategory::ALL {
        let owner = registry.authority(category).unwrap().owner;
        if category == HSAssetCategory::CollaborationGuidance {
            assert_eq!(owner, HSAssetOwner::AcceptedHsStore);
        } else {
            assert_eq!(owner, HSAssetOwner::LifeModelYaml);
        }
    }
}

#[test]
fn promoted_authority_survives_registry_restart() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("hs_asset_authority.db");
    let store = seeded_store();
    {
        let registry = HSAssetAuthorityRegistry::new(&path).unwrap();
        let scenario = fixture_scenario(&registry, 1);
        complete_collaboration_guidance_cutover(
            &registry,
            &LifeModel::default_model(),
            &store,
            &scenario,
        )
        .unwrap();
    }

    let restarted = HSAssetAuthorityRegistry::new(&path).unwrap();
    let authority = restarted
        .authority(HSAssetCategory::CollaborationGuidance)
        .unwrap();
    assert_eq!(authority.owner, HSAssetOwner::AcceptedHsStore);
    assert_eq!(authority.revision, 2);
}

#[test]
fn stale_parity_evidence_cannot_promote_after_authority_revision_changes() {
    let registry = HSAssetAuthorityRegistry::new_in_memory().unwrap();
    let (parity, rollback, scenario) = proof_set(&registry);
    registry
        .invalidate_shadow_evidence(
            HSAssetCategory::CollaborationGuidance,
            1,
            &digest_string("canonical assets changed"),
        )
        .unwrap();

    let error = registry
        .promote_to_accepted_hs(
            HSAssetCategory::CollaborationGuidance,
            1,
            &parity,
            &rollback,
            &scenario,
        )
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("stale HS asset authority revision"));
    assert_eq!(
        registry
            .authority(HSAssetCategory::CollaborationGuidance)
            .unwrap()
            .owner,
        HSAssetOwner::LifeModelYaml
    );
}

#[test]
fn digest_mismatch_fails_closed_even_with_rollback_and_scenario_receipts() {
    let registry = HSAssetAuthorityRegistry::new_in_memory().unwrap();
    let parity = registry
        .record_shadow_parity(
            HSAssetCategory::CollaborationGuidance,
            1,
            digest_string("canonical"),
            digest_string("different compatibility"),
            digest_string("canonical"),
        )
        .unwrap();
    let rollback = registry
        .rehearse_rollback(HSAssetCategory::CollaborationGuidance, 1)
        .unwrap();
    let scenario = fixture_scenario(&registry, 1);

    let error = registry
        .promote_to_accepted_hs(
            HSAssetCategory::CollaborationGuidance,
            1,
            &parity,
            &rollback,
            &scenario,
        )
        .unwrap_err();
    assert!(error.to_string().contains("digest parity"));
}

#[test]
fn concurrent_promotion_has_exactly_one_cas_winner() {
    let registry = Arc::new(HSAssetAuthorityRegistry::new_in_memory().unwrap());
    let (parity, rollback, scenario) = proof_set(&registry);
    let mut threads = Vec::new();
    for _ in 0..16 {
        let registry = Arc::clone(&registry);
        let parity = parity.clone();
        let rollback = rollback.clone();
        let scenario = scenario.clone();
        threads.push(std::thread::spawn(move || {
            registry
                .promote_to_accepted_hs(
                    HSAssetCategory::CollaborationGuidance,
                    1,
                    &parity,
                    &rollback,
                    &scenario,
                )
                .is_ok()
        }));
    }
    let winner_count = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .filter(|won| *won)
        .count();

    assert_eq!(winner_count, 1);
    assert_eq!(
        registry
            .authority(HSAssetCategory::CollaborationGuidance)
            .unwrap()
            .revision,
        2
    );
}

#[test]
fn promoted_category_has_one_product_write_owner_and_yaml_is_projection_only() {
    let registry = HSAssetAuthorityRegistry::new_in_memory().unwrap();
    registry
        .authorize_write(HSAssetWriteRequest {
            category: HSAssetCategory::CollaborationGuidance,
            source_owner: HSAssetOwner::LifeModelYaml,
            target_owner: HSAssetOwner::LifeModelYaml,
            kind: HSAssetWriteKind::ProductMutation,
        })
        .unwrap();
    let (parity, rollback, scenario) = proof_set(&registry);
    registry
        .promote_to_accepted_hs(
            HSAssetCategory::CollaborationGuidance,
            1,
            &parity,
            &rollback,
            &scenario,
        )
        .unwrap();

    assert!(registry
        .authorize_write(HSAssetWriteRequest {
            category: HSAssetCategory::CollaborationGuidance,
            source_owner: HSAssetOwner::LifeModelYaml,
            target_owner: HSAssetOwner::LifeModelYaml,
            kind: HSAssetWriteKind::ProductMutation,
        })
        .is_err());
    registry
        .authorize_write(HSAssetWriteRequest {
            category: HSAssetCategory::CollaborationGuidance,
            source_owner: HSAssetOwner::AcceptedHsStore,
            target_owner: HSAssetOwner::AcceptedHsStore,
            kind: HSAssetWriteKind::ProductMutation,
        })
        .unwrap();
    registry
        .authorize_write(HSAssetWriteRequest {
            category: HSAssetCategory::CollaborationGuidance,
            source_owner: HSAssetOwner::AcceptedHsStore,
            target_owner: HSAssetOwner::LifeModelYaml,
            kind: HSAssetWriteKind::DerivedCompatibilityProjection,
        })
        .unwrap();
}

#[test]
fn rollback_rehearsal_supports_cas_restore_to_yaml() {
    let registry = HSAssetAuthorityRegistry::new_in_memory().unwrap();
    let (parity, rollback, scenario) = proof_set(&registry);
    registry
        .promote_to_accepted_hs(
            HSAssetCategory::CollaborationGuidance,
            1,
            &parity,
            &rollback,
            &scenario,
        )
        .unwrap();

    let restored = registry
        .rollback_to_yaml(
            HSAssetCategory::CollaborationGuidance,
            2,
            &rollback,
            &digest_string("operator rollback reason"),
        )
        .unwrap();
    assert_eq!(restored.owner, HSAssetOwner::LifeModelYaml);
    assert_eq!(restored.revision, 3);
    assert_eq!(restored.previous_owner, Some(HSAssetOwner::AcceptedHsStore));
}

#[test]
fn compatibility_yaml_contains_only_metadata_safe_guidance_refs_and_digests() {
    let store = seeded_store();
    let record = store
        .create_heuristic(
            HeuristicDraft::new(
                "planning",
                "RAW_TRIGGER_SECRET",
                vec!["RAW_CONDITION_SECRET".into()],
                "RAW_GUIDANCE_SECRET",
                70,
                RiskLevel::Low,
                EvidencePrivacyLevel::Internal,
            )
            .with_stable_id("accepted-guidance-safe-id"),
        )
        .unwrap();
    store
        .update_lifecycle(&record.id, HeuristicLifecycleStatus::Trial, None)
        .unwrap();

    let projection =
        crate::agent::build_collaboration_guidance_projection(&LifeModel::default_model(), &store)
            .unwrap();

    assert_eq!(projection.canonical_digest, projection.compatibility_digest);
    assert_eq!(
        projection.canonical_digest,
        projection.repeated_materialization_digest
    );
    for raw in [
        "RAW_TRIGGER_SECRET",
        "RAW_CONDITION_SECRET",
        "RAW_GUIDANCE_SECRET",
    ] {
        assert!(!projection.yaml.contains(raw));
    }
    assert!(projection.yaml.contains("accepted-guidance-safe-id"));
    assert!(projection.yaml.contains("heuristics.planning"));
}

#[test]
fn policy_store_is_not_accidentally_treated_as_a_migrated_yaml_asset() {
    let registry = HSAssetAuthorityRegistry::new_in_memory().unwrap();
    let _policy_store = PolicyStore::mvp_builtin();
    assert!(!HSAssetCategory::ALL
        .iter()
        .any(|category| category.to_string() == "policy"));
    assert_eq!(registry.list_authorities().unwrap().len(), 7);
}
