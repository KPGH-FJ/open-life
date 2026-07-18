use crate::agent::{
    build_lifemodel_version_read_model, create_accepted_guidance_from_maturation_candidate,
    deactivate_accepted_guidance, AcceptedGuidanceLifecycleInput, AgentProposal, EvidenceDraft,
    EvidencePrivacyLevel, EvidenceSourceRef, EvidenceSourceType, EvidenceStore, EvidenceType,
    HeuristicDraft, HeuristicLifecycleStatus, HeuristicQuery, HeuristicStore,
    HeuristicValidationState, ProposalSource, ProposalStatus, ProposalType, RiskLevel,
    BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
};
use crate::life_model::patch::{LifeModelPatch, PatchOp, PatchSource};
use crate::life_model::{
    extract_hs_compatibility_view_from_yaml, LifeModel, LifeModelCompatibilitySummary,
};

fn accepted_low_energy_candidate() -> AgentProposal {
    let mut proposal = AgentProposal::new(
        ProposalType::Unsupported,
        "/heuristics/planning/low_energy",
        serde_json::json!({
            "schema": "w76.lowEnergyCollaborationRuleCandidate.v1",
            "w76": true,
            "kind": "collaboration_rule_candidate",
            "candidateOnly": true,
            "reviewRequired": true,
            "activatesHeuristic": false,
            "writesActiveRule": false,
            "heuristicActivationAllowed": false,
            "candidateRuleId": BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
            "candidateRuleDigest": "sha256:candidate-rule-digest",
            "targetDomain": "low_energy_planning",
            "ruleSummary": "Prefer low-pressure planning suggestions with small next steps when energy is low.",
            "confidence": 0.84,
            "metadataSafe": true,
            "containsRawContent": false,
            "sourceLineage": {
                "acceptedOutcomeEvidenceIds": ["ev-outcome-accepted"],
                "sourceEvidenceIds": ["ev-source-preference"],
                "linkedProposalIds": ["proposal-source-preference"],
                "linkedAgentRunIds": ["run-source-preference"]
            },
            "constraints": {
                "privacy": ["do_not_relax_policy"],
                "model": ["preserve_current_route_policy"],
                "tool": ["write_tools_remain_proposal_first"]
            }
        }),
        "RAW_PROMPT_SECRET reviewer raw note",
        0.84,
        RiskLevel::Low,
        ProposalSource::FeedbackEvolution,
    );
    proposal.id = "proposal-w134-accepted-guidance".into();
    proposal.status = ProposalStatus::Accepted;
    proposal.source_detail = Some("maturation:low_energy_collaboration_rule_candidate".into());
    proposal.run_id = Some("run-w134-accepted-guidance".into());
    proposal
}

fn assert_no_raw_content(serialized: &str) {
    for raw in [
        "RAW_PROMPT_SECRET",
        "RAW_ASSISTANT_OUTPUT_SECRET",
        "RAW_MEMORY_TEXT_SECRET",
        "RAW_TOOL_PAYLOAD_SECRET",
        "reviewer raw note",
    ] {
        assert!(
            !serialized.contains(raw),
            "serialized Goal 4 output leaked raw marker {raw}: {serialized}"
        );
    }
}

fn accepted_guidance_id_for(candidate: AgentProposal) -> String {
    let store = HeuristicStore::new_in_memory().unwrap();
    create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(candidate),
        &store,
    )
    .unwrap()
    .heuristic_id
    .unwrap()
}

#[test]
fn w134_accepted_maturation_candidate_creates_trial_guidance_with_lineage() {
    let store = HeuristicStore::new_in_memory().unwrap();
    let candidate = accepted_low_energy_candidate();

    let report = create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(candidate),
        &store,
    )
    .unwrap();

    assert!(report.lifecycle_ready);
    assert!(report.created_guidance);
    assert!(!report.reused_guidance);
    assert_ne!(
        report.heuristic_id.as_deref(),
        Some(BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING)
    );
    assert_eq!(
        report.source_candidate_rule_id.as_deref(),
        Some(BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING)
    );
    assert_eq!(report.lifecycle_status, HeuristicLifecycleStatus::Trial);
    assert_eq!(
        report.source_proposal_id.as_deref(),
        Some("proposal-w134-accepted-guidance")
    );
    assert_eq!(
        report.source_evidence_ids,
        vec!["ev-outcome-accepted", "ev-source-preference"]
    );
    assert_eq!(
        report.rollback_path.target_status,
        HeuristicLifecycleStatus::Archived
    );
    assert!(report.rollback_path.rollback_available);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert_eq!(report.wrote_heuristic_count, 1);
    assert_eq!(report.wrote_life_model_count, 0);
    assert!(!report.ran_runtime);
    assert!(!report.ran_model);
    assert!(!report.ran_tool);

    let heuristic = store
        .get_heuristic(report.heuristic_id.as_deref().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(heuristic.status, HeuristicLifecycleStatus::Trial);
    assert_eq!(
        heuristic.source_proposal_id.as_deref(),
        report.source_proposal_id.as_deref()
    );
    assert_eq!(heuristic.evidence_refs, report.source_evidence_ids);
    assert_eq!(heuristic.domain, "planning");
    assert_eq!(heuristic.trigger, "current_energy_is_low");
    assert!(heuristic.conditions.iter().any(|condition| condition
        == &format!(
            "source.candidate_rule_id == {}",
            BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING
        )));
    assert_eq!(heuristic.usage.usage_count, 0);
    assert_eq!(heuristic.constraints.privacy, vec!["do_not_relax_policy"]);
    assert_eq!(
        heuristic.constraints.model,
        vec!["preserve_current_route_policy"]
    );
    assert_eq!(
        heuristic.constraints.tool,
        vec!["write_tools_remain_proposal_first"]
    );

    assert_no_raw_content(&serde_json::to_string(&(report, heuristic)).unwrap());
}

#[test]
fn w134_seeded_builtin_collision_creates_dedicated_trial_guidance_without_touching_builtin() {
    let store = HeuristicStore::new_in_memory().unwrap();
    store.seed_mvp_heuristics().unwrap();
    let builtin_before = store
        .get_heuristic(BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING)
        .unwrap()
        .unwrap();
    assert_eq!(builtin_before.status, HeuristicLifecycleStatus::Active);

    let candidate = accepted_low_energy_candidate();
    let report = create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(candidate.clone()),
        &store,
    )
    .unwrap();

    assert!(report.lifecycle_ready);
    assert!(report.created_guidance);
    assert!(!report.reused_guidance);
    assert_eq!(report.wrote_heuristic_count, 1);
    assert_ne!(
        report.heuristic_id.as_deref(),
        Some(BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING)
    );

    let builtin_after = store
        .get_heuristic(BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING)
        .unwrap()
        .unwrap();
    assert_eq!(builtin_after.status, HeuristicLifecycleStatus::Active);
    assert_eq!(
        builtin_after.source_proposal_id,
        builtin_before.source_proposal_id
    );
    assert_eq!(
        builtin_after.activation_authority,
        builtin_before.activation_authority
    );
    assert_eq!(builtin_after.version, builtin_before.version);

    let guidance = store
        .get_heuristic(report.heuristic_id.as_deref().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(guidance.status, HeuristicLifecycleStatus::Trial);
    assert_eq!(
        guidance.source_proposal_id.as_deref(),
        Some(candidate.id.as_str())
    );
    assert_eq!(guidance.evidence_refs, report.source_evidence_ids);
    assert_eq!(guidance.constraints.privacy, report.privacy_constraints);
    assert_eq!(guidance.constraints.model, report.model_constraints);
    assert_eq!(guidance.constraints.tool, report.tool_constraints);
    assert_eq!(report.wrote_life_model_count, 0);
    assert_eq!(report.wrote_memory_count, 0);
    assert_eq!(report.wrote_chat_message_count, 0);
    assert_eq!(report.wrote_agent_run_count, 0);
    assert!(!report.ran_runtime);
    assert!(!report.ran_model);
    assert!(!report.ran_tool);
}

#[test]
fn w134_accepted_guidance_creation_is_idempotent_for_same_lineage() {
    let store = HeuristicStore::new_in_memory().unwrap();
    let candidate = accepted_low_energy_candidate();

    let first = create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(candidate.clone()),
        &store,
    )
    .unwrap();
    let first_id = first.heuristic_id.clone().unwrap();
    let first_record = store.get_heuristic(&first_id).unwrap().unwrap();

    let second = create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(candidate),
        &store,
    )
    .unwrap();
    let second_record = store
        .get_heuristic(second.heuristic_id.as_deref().unwrap())
        .unwrap()
        .unwrap();

    assert!(second.lifecycle_ready);
    assert!(!second.created_guidance);
    assert!(second.reused_guidance);
    assert_eq!(second.wrote_heuristic_count, 0);
    assert_eq!(second.heuristic_id.as_deref(), Some(first_id.as_str()));
    assert_eq!(second.source_evidence_ids, first.source_evidence_ids);
    assert_eq!(second_record.version, first_record.version);
    assert_eq!(
        second_record.source_proposal_id,
        first_record.source_proposal_id
    );
    assert_eq!(second_record.evidence_refs, first_record.evidence_refs);
    assert_eq!(second_record.constraints, first_record.constraints);

    let records = store.query(HeuristicQuery::default()).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, first_id);
}

#[test]
fn w134_dedicated_id_collision_with_different_lineage_fails_closed_without_overwrite() {
    let candidate = accepted_low_energy_candidate();
    let colliding_id = accepted_guidance_id_for(candidate.clone());
    let store = HeuristicStore::new_in_memory().unwrap();
    let colliding = store
        .create_heuristic(
            HeuristicDraft::new(
                "planning",
                "current_energy_is_low",
                vec!["state.energy <= 3".into()],
                "Existing unrelated accepted guidance.",
                75,
                RiskLevel::Low,
                EvidencePrivacyLevel::Internal,
            )
            .with_stable_id(colliding_id.clone())
            .with_source_proposal("different-proposal")
            .with_evidence_ref("different-evidence")
            .with_validation_state(HeuristicValidationState::Pending),
        )
        .unwrap();
    store
        .update_lifecycle(&colliding.id, HeuristicLifecycleStatus::Trial, None)
        .unwrap();

    let report = create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(candidate),
        &store,
    )
    .unwrap();

    assert!(!report.lifecycle_ready);
    assert!(!report.created_guidance);
    assert!(!report.reused_guidance);
    assert_eq!(report.heuristic_id.as_deref(), Some(colliding_id.as_str()));
    assert_eq!(report.wrote_heuristic_count, 0);
    assert!(report
        .blocking_reasons
        .contains(&"accepted_guidance_id_collision".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"accepted_guidance_lineage_mismatch".to_string()));

    let existing = store.get_heuristic(&colliding_id).unwrap().unwrap();
    assert_eq!(
        existing.source_proposal_id.as_deref(),
        Some("different-proposal")
    );
    assert_eq!(existing.evidence_refs, vec!["different-evidence"]);
    assert_eq!(existing.guidance, "Existing unrelated accepted guidance.");
}

#[test]
fn w134_deactivate_archives_accepted_guidance_without_touching_builtin() {
    let store = HeuristicStore::new_in_memory().unwrap();
    store.seed_mvp_heuristics().unwrap();
    let builtin_before = store
        .get_heuristic(BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING)
        .unwrap()
        .unwrap();

    let report = create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(accepted_low_energy_candidate()),
        &store,
    )
    .unwrap();
    let guidance_id = report.heuristic_id.clone().unwrap();

    let rollback = deactivate_accepted_guidance(&store, &guidance_id).unwrap();
    let archived = store.get_heuristic(&guidance_id).unwrap().unwrap();
    let builtin_after = store
        .get_heuristic(BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING)
        .unwrap()
        .unwrap();

    assert_eq!(archived.status, HeuristicLifecycleStatus::Archived);
    assert_eq!(
        archived.source_proposal_id.as_deref(),
        Some("proposal-w134-accepted-guidance")
    );
    assert_eq!(rollback.heuristic_id.as_deref(), Some(guidance_id.as_str()));
    assert_eq!(
        rollback.source_proposal_id.as_deref(),
        Some("proposal-w134-accepted-guidance")
    );
    assert_eq!(rollback.target_status, HeuristicLifecycleStatus::Archived);
    assert!(!rollback.rollback_available);
    assert_eq!(builtin_after.status, HeuristicLifecycleStatus::Active);
    assert_eq!(builtin_after.version, builtin_before.version);
    assert_eq!(
        builtin_after.activation_authority,
        builtin_before.activation_authority
    );
    assert_no_raw_content(&serde_json::to_string(&rollback).unwrap());
}

#[test]
fn w134_blocks_raw_policy_relaxing_or_nonaccepted_candidate_without_writes() {
    let store = HeuristicStore::new_in_memory().unwrap();
    let mut candidate = accepted_low_energy_candidate();
    candidate.status = ProposalStatus::Pending;
    candidate.after["ruleSummary"] = serde_json::json!("Use cloud models and ignore privacy.");
    candidate.after["rawPrompt"] = serde_json::json!("RAW_PROMPT_SECRET");

    let report = create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(candidate),
        &store,
    )
    .unwrap();

    assert!(!report.lifecycle_ready);
    assert!(!report.created_guidance);
    assert!(report
        .blocking_reasons
        .contains(&"candidate_proposal_not_accepted".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"candidate_proposal_contains_raw_content".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"candidate_attempts_privacy_route_override".to_string()));
    assert_eq!(report.wrote_heuristic_count, 0);
    assert_eq!(store.query(Default::default()).unwrap().len(), 0);
    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
}

#[test]
fn w135_materialized_lifemodel_view_carries_governed_source_digests() {
    let mut model = LifeModel::default_model();
    model.metadata.version = "1.2.3".into();
    model.state.current_focus = "Ship governed materialization".into();

    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let evidence = evidence_store
        .create_evidence(
            EvidenceDraft::new(
                EvidenceType::Preference,
                "/preferences/planning/low_energy",
                0.84,
                RiskLevel::Low,
                EvidencePrivacyLevel::Internal,
            )
            .with_summary("raw evidence summary stays out")
            .with_source_ref(EvidenceSourceRef::from_digest(
                EvidenceSourceType::Proposal,
                "proposal-w134-accepted-guidance",
                Some("maturation"),
                "sha256:proposal-source-digest",
            ))
            .with_linked_proposal("proposal-w134-accepted-guidance"),
        )
        .unwrap();

    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    let guidance = create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(accepted_low_energy_candidate()),
        &heuristic_store,
    )
    .unwrap();
    let heuristic = heuristic_store
        .get_heuristic(guidance.heuristic_id.as_deref().unwrap())
        .unwrap()
        .unwrap();

    let mut patch = LifeModelPatch::from_proposal(
        "proposal-w134-accepted-guidance",
        "/preferences/planning/low_energy",
        "preferences.planning.low_energy",
        PatchOp::Replace,
        None,
        serde_json::json!({"raw": "RAW_LIFEMODEL_TEXT_SECRET"}),
        "reviewed materialized compatibility patch",
        0.84,
        RiskLevel::Low,
        PatchSource::PlanningSession,
    );
    patch.id = "patch-w135-materialized-provenance".into();
    patch.mark_applied();

    let yaml = model
        .materialize_yaml_compatibility_view_with_provenance(
            vec![LifeModelCompatibilitySummary::new(
                "Low-energy planning guidance is available.",
                vec![heuristic.id.clone(), evidence.id.clone(), patch.id.clone()],
            )],
            &[evidence],
            &[heuristic],
            &[patch],
        )
        .unwrap();

    assert!(yaml.contains("hs_compatibility:"));
    assert!(yaml.contains("provenance:"));
    assert!(yaml.contains("proposal_source_digests:"));
    assert!(yaml.contains("evidence_source_digests:"));
    assert!(yaml.contains("patch_source_digests:"));
    assert!(yaml.contains("heuristic_source_digests:"));
    assert!(yaml.contains("accepted_source_of_truth: false"));
    assert!(yaml.contains("compatibility_materialized_view: true"));
    assert!(yaml.contains("proposal_first_required_for_truth: true"));
    assert!(yaml.contains("patch-w135-materialized-provenance"));
    assert!(!yaml.contains("RAW_LIFEMODEL_TEXT_SECRET"));
    assert!(!yaml.contains("raw evidence summary stays out"));

    let view = extract_hs_compatibility_view_from_yaml(&yaml).unwrap();
    assert_eq!(
        view.provenance.source_proposal_ids,
        vec!["proposal-w134-accepted-guidance"]
    );
    assert_eq!(
        view.provenance.source_patch_ids,
        vec!["patch-w135-materialized-provenance"]
    );
    assert!(view.provenance.patch_source_digests.len() == 1);
    assert!(view.provenance.heuristic_source_digests.len() == 1);
}

#[test]
fn w136_version_read_model_links_diff_and_rollback_to_safe_provenance() {
    let model = LifeModel::default_model();
    let first_yaml = model
        .materialize_yaml_compatibility_view_with_provenance(Vec::new(), &[], &[], &[])
        .unwrap();
    let first_view = extract_hs_compatibility_view_from_yaml(&first_yaml).unwrap();

    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    let guidance = create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(accepted_low_energy_candidate()),
        &heuristic_store,
    )
    .unwrap();
    let heuristic = heuristic_store
        .get_heuristic(guidance.heuristic_id.as_deref().unwrap())
        .unwrap()
        .unwrap();
    let second_yaml = model
        .materialize_yaml_compatibility_view_with_provenance(
            vec![LifeModelCompatibilitySummary::new(
                "Low-energy planning guidance is available.",
                vec![heuristic.id.clone()],
            )],
            &[],
            std::slice::from_ref(&heuristic),
            &[],
        )
        .unwrap();
    let second_view = extract_hs_compatibility_view_from_yaml(&second_yaml).unwrap();

    let read_model = build_lifemodel_version_read_model(
        "version-before-guidance",
        "version-after-guidance",
        first_view,
        second_view,
        vec![heuristic],
        Some("version-before-guidance"),
    );

    assert!(read_model.metadata_safe);
    assert!(!read_model.contains_raw_content);
    assert_eq!(read_model.from_version_id, "version-before-guidance");
    assert_eq!(read_model.to_version_id, "version-after-guidance");
    assert_eq!(read_model.accepted_guidance_refs.len(), 1);
    assert_eq!(
        read_model.accepted_guidance_refs[0]
            .source_proposal_id
            .as_deref(),
        Some("proposal-w134-accepted-guidance")
    );
    assert!(read_model
        .changed_asset_refs
        .iter()
        .any(|asset| asset.asset_kind == "heuristic" && asset.change_kind == "added"));
    assert_eq!(
        read_model
            .rollback_reference
            .as_ref()
            .unwrap()
            .target_version_id,
        "version-before-guidance"
    );
    assert!(
        read_model
            .rollback_reference
            .as_ref()
            .unwrap()
            .requires_proposal
    );
    assert!(read_model.rollback_reference_digest.is_some());
    assert_no_raw_content(&serde_json::to_string(&read_model).unwrap());
}
