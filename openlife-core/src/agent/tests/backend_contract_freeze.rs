use crate::agent::{
    evaluate_final_backend_completion_gate, freeze_pre_ui_backend_read_model_contracts, AgentRun,
    AgentRunStatus, AgentTaskKind, BackendCompletionGateEvidence, ContextSummary,
    EvidenceConflictState, EvidenceCooldownState, EvidenceDecayState, EvidencePolarity,
    EvidenceTimelineItem, EvidenceTimelineReadModel, FinalBackendCompletionGateInput,
    GovernorDecisionReport, GuidanceAffectedSurface, GuidanceImpactReadModel, GuidanceImpactRef,
    HeuristicLifecycleStatus, LifeModelVersionReadModel, PreUiBackendContractFreezeInput,
    ProposalSource, ProposalStatus, ProposalType, RedactionLevel, RiskLevel,
};
use crate::life_model::LifeModelMaterializedViewProvenance;
use chrono::{TimeZone, Utc};

fn frozen_now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 5, 12, 0, 0).unwrap()
}

fn evidence_timeline() -> EvidenceTimelineReadModel {
    let now = frozen_now();
    EvidenceTimelineReadModel {
        report_kind: "evidence_timeline_v1".into(),
        metadata_safe: true,
        contains_raw_content: false,
        generated_at: now,
        item_count: 1,
        items: vec![EvidenceTimelineItem {
            evidence_id: "ev-w147-low-energy".into(),
            evidence_type: "preference".into(),
            affected_path: "/preferences/planning/low_energy_intensity".into(),
            status: "candidate".into(),
            confidence: 0.84,
            risk_level: "low".into(),
            privacy_level: "internal".into(),
            polarity: EvidencePolarity::Supporting,
            source_ref_count: 2,
            support_link_count: 1,
            opposition_link_count: 0,
            linked_proposal_ids: vec!["proposal-w147-learning".into()],
            linked_agent_run_ids: vec!["run-w147-trace".into()],
            cluster_id: "egc-w147".into(),
            cluster_hash: "sha256:cluster-w147".into(),
            conflict_state: EvidenceConflictState {
                conflicted: false,
                reasons: Vec::new(),
                opposing_evidence_ids: Vec::new(),
                contradicted: false,
                rejected_opposition: false,
                same_affected_path_cluster_opposition: false,
            },
            decay_state: EvidenceDecayState {
                generated_at: now,
                last_observed_at: now,
                age_days: 0,
                grace_days: 30,
                half_life_days: 90.0,
                decay_factor: 1.0,
                effective_confidence: 0.84,
                decayed: false,
            },
            cooldown_state: EvidenceCooldownState {
                active: false,
                reason: None,
                similar_cluster_id: None,
                cooldown_days: 14,
                cooldown_until: None,
                days_remaining: 0,
                rejected_evidence_ids: Vec::new(),
                rejected_proposal_ids: Vec::new(),
            },
            created_at: now,
            updated_at: now,
            last_observed_at: now,
        }],
    }
}

fn proposal_with_raw_payload() -> crate::agent::AgentProposal {
    let mut proposal = crate::agent::AgentProposal::new(
        ProposalType::PreferenceUpdate,
        "/preferences/planning/low_energy_intensity",
        serde_json::json!({
            "rawUserText": "RAW_USER_TEXT_SECRET alice@example.com",
            "rawAssistantOutput": "RAW_ASSISTANT_OUTPUT_SECRET",
        }),
        "RAW_REASON_SECRET should not appear in read models",
        0.81,
        RiskLevel::Low,
        ProposalSource::PlanningSession,
    );
    proposal.id = "proposal-w147-learning".into();
    proposal.run_id = Some("run-w147-trace".into());
    proposal.status = ProposalStatus::Pending;
    proposal
}

fn runtime_run_with_raw_fields() -> AgentRun {
    let mut run = AgentRun::new_chat_run(
        "session-w147",
        "RAW_USER_TEXT_SECRET alice@example.com must not leak",
    );
    run.id = "run-w147-trace".into();
    run.status = AgentRunStatus::Completed;
    run.kind = AgentTaskKind::Planning;
    run.output_preview = Some("RAW_ASSISTANT_OUTPUT_SECRET must not leak".into());
    run.reasoning_strategy = Some("plan_execute".into());
    run.generated_proposals = vec!["proposal-w147-learning".into()];
    run.context_summary = Some(ContextSummary {
        life_model_empty: false,
        included_life_model_sections: vec!["state".into(), "preferences".into()],
        memory_hit_count: 2,
        memory_sources: vec!["RAW_MEMORY_SECRET".into()],
        used_tools_prompt: true,
        redaction_applied: true,
        redaction_level: RedactionLevel::Strict,
    });
    run
}

fn guidance_impact() -> GuidanceImpactReadModel {
    GuidanceImpactReadModel {
        report_kind: "w140.guidanceImpactReadModel.v1".into(),
        metadata_safe: true,
        contains_raw_content: false,
        run_id: Some("run-w147-trace".into()),
        strategy_kind: "plan_execute".into(),
        selected_guidance_count: 1,
        selected_policy_count: 1,
        guidance_refs: vec![GuidanceImpactRef {
            guidance_id: "accepted_guidance_w147".into(),
            guidance_digest: "sha256:guidance-w147".into(),
            guidance_type: "accepted_guidance".into(),
            lifecycle_status: HeuristicLifecycleStatus::Trial,
            domain: "planning".into(),
            impact_kind: "gentle_planning".into(),
            selected_reason: "task_domain_and_trigger_match".into(),
            source_proposal_id: Some("proposal-w147-learning".into()),
            source_evidence_count: 1,
            source_lineage_digest: "sha256:lineage-w147".into(),
            affected_run_count: 1,
            affected_surfaces: vec![GuidanceAffectedSurface::PlanExecuteTrace],
        }],
        selected_policy_ids: vec!["policy.sensitive_topics.local_only".into()],
        affected_surfaces: vec![GuidanceAffectedSurface::PlanExecuteTrace],
        behavior_check_count: 1,
        read_model_digest: "sha256:impact-w147".into(),
        raw_prompt_included: false,
        raw_user_text_included: false,
        raw_assistant_output_included: false,
        raw_memory_included: false,
        raw_life_model_included: false,
        raw_tool_payload_included: false,
        raw_guidance_included: false,
    }
}

fn lifemodel_version_read_model() -> LifeModelVersionReadModel {
    let provenance = LifeModelMaterializedViewProvenance {
        compatibility_materialized_view: true,
        accepted_source_of_truth: false,
        durable_truth_materialized: false,
        proposal_first_required_for_truth: true,
        source_proposal_ids: vec!["proposal-w147-learning".into()],
        source_evidence_ids: vec!["ev-w147-low-energy".into()],
        source_patch_ids: vec!["patch-w147".into()],
        source_heuristic_ids: vec!["accepted_guidance_w147".into()],
        proposal_source_digests: vec!["sha256:proposal-digest".into()],
        evidence_source_digests: vec!["sha256:evidence-digest".into()],
        patch_source_digests: vec!["sha256:patch-digest".into()],
        heuristic_source_digests: vec!["sha256:heuristic-digest".into()],
        provenance_digest: "sha256:provenance-w147".into(),
    };

    LifeModelVersionReadModel {
        report_kind: "w136.lifeModelVersionReadModel.v1".into(),
        metadata_safe: true,
        contains_raw_content: false,
        from_version_id: "version-before".into(),
        to_version_id: "version-after".into(),
        materialized_view_source_digest: "sha256:lifemodel-view".into(),
        materialized_view_provenance_digest: "sha256:provenance-w147".into(),
        provenance,
        accepted_guidance_refs: Vec::new(),
        changed_asset_refs: Vec::new(),
        rollback_reference: None,
        diff_reference_digest: "sha256:diff-w147".into(),
        rollback_reference_digest: None,
        raw_content_included: false,
    }
}

fn gate_evidence() -> BackendCompletionGateEvidence {
    BackendCompletionGateEvidence {
        lifemodel_maturity_gate_passed: true,
        runtime_driven_gate_passed: true,
        governance_privacy_gate_passed: true,
        ui_read_model_gate_passed: true,
        default_chat_isolated: true,
        ordinary_chat_route_unchanged: true,
        proposal_first_boundaries_preserved: true,
        raw_content_excluded: true,
        local_only_privacy_enforced: true,
        tool_governance_enforced: true,
        golden_paths_ready: true,
        materialized_lifemodel_provenance_traceable: true,
        high_risk_auto_materialization_blocked: true,
        remaining_beta_blockers: vec!["skill_runtime_beta_not_complete".into()],
    }
}

#[test]
fn w147_freezes_all_pre_ui_read_model_contracts_without_raw_payloads() {
    let report = freeze_pre_ui_backend_read_model_contracts(PreUiBackendContractFreezeInput {
        generated_at: frozen_now(),
        evidence_timeline: evidence_timeline(),
        proposals: vec![proposal_with_raw_payload()],
        agent_runs: vec![runtime_run_with_raw_fields()],
        guidance_impact: guidance_impact(),
        governor_decisions: vec![GovernorDecisionReport {
            report_kind: "governor_decision_report".into(),
            metadata_safe: true,
            contains_raw_content: false,
            subject: crate::agent::GovernanceSubject::ModelRoute,
            decision_kind: crate::agent::GovernanceDecisionKind::RequireLocalOnly,
            classification: crate::agent::GovernanceDecisionClassification::LocalOnly,
            allowed: false,
            blocked: false,
            requires_confirmation: false,
            requires_proposal: false,
            requires_local_only: true,
            risk_level: RiskLevel::High,
            policy_reason_code: "sensitive_local_only".into(),
            proposal_type: None,
            source_run_id: Some("run-w147-trace".into()),
            selected_policy_ids: vec!["policy.sensitive_topics.local_only".into()],
            metadata_safe_summary: serde_json::json!({
                "policyReasonCode": "sensitive_local_only"
            }),
            warning_count: 0,
            decision_digest: "sha256:decision-w147".into(),
            raw_prompt_included: false,
            raw_user_text_included: false,
            raw_assistant_output_included: false,
            raw_memory_included: false,
            raw_life_model_included: false,
            raw_tool_payload_included: false,
        }],
        lifemodel_version: lifemodel_version_read_model(),
    });

    assert!(report.contract_frozen);
    assert_eq!(report.report_kind, "w147.preUiBackendContractFreeze.v1");
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(report.read_only);
    assert!(!report.command_surface_added);
    assert!(report.default_chat_unchanged);
    assert!(!report.migration_permission);
    assert_eq!(report.surface_count, 7);
    assert!(report.learning_inbox.item_count >= 2);
    assert_eq!(report.evidence_timeline.item_count, 1);
    assert_eq!(report.proposal_review.pending_count, 1);
    assert!(report.runtime_trace.hs_influence.included);
    assert_eq!(report.guidance_impact.selected_guidance_count, 1);
    assert!(report.privacy_controls.local_only_policy_visible);
    assert!(report.lifemodel_overview.provenance_traceable);
    assert!(report.blockers.is_empty());

    let serialized = serde_json::to_string(&report).unwrap();
    for raw in [
        "RAW_USER_TEXT_SECRET",
        "RAW_ASSISTANT_OUTPUT_SECRET",
        "RAW_MEMORY_SECRET",
        "RAW_REASON_SECRET",
        "alice@example.com",
    ] {
        assert!(
            !serialized.contains(raw),
            "W147 contract report leaked raw marker {raw}: {serialized}"
        );
    }
}

#[test]
fn w148_final_backend_completion_gate_passes_with_explicit_beta_blockers_only() {
    let contracts = freeze_pre_ui_backend_read_model_contracts(PreUiBackendContractFreezeInput {
        generated_at: frozen_now(),
        evidence_timeline: evidence_timeline(),
        proposals: vec![proposal_with_raw_payload()],
        agent_runs: vec![runtime_run_with_raw_fields()],
        guidance_impact: guidance_impact(),
        governor_decisions: Vec::new(),
        lifemodel_version: lifemodel_version_read_model(),
    });
    let report = evaluate_final_backend_completion_gate(FinalBackendCompletionGateInput {
        generated_at: frozen_now(),
        contract_freeze: contracts,
        evidence: gate_evidence(),
    });

    assert!(report.gate_ready);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(report.default_chat_isolation.default_chat_isolated);
    assert_eq!(
        report.default_chat_isolation.selected_adapter_path,
        "main_chat_kernel"
    );
    assert!(!report.default_chat_isolation.migration_permission);
    assert!(report.proposal_first_boundaries.proposal_first_preserved);
    assert!(report.raw_content_exclusion.raw_content_excluded);
    assert!(report.local_only_privacy.local_only_enforced);
    assert!(report.tool_governance.tool_governance_enforced);
    assert!(report.golden_path_coverage.weekly_planning_ready);
    assert!(report.golden_path_coverage.low_energy_support_ready);
    assert!(report.golden_path_coverage.preference_correction_ready);
    assert_eq!(
        report.remaining_beta_blockers,
        vec!["skill_runtime_beta_not_complete"]
    );
    assert!(report
        .blockers_by_gate
        .iter()
        .all(|gate| gate.blockers.is_empty()));
    assert_eq!(report.business_write_count, 0);
    assert!(!report.tauri_command_added);
}

#[test]
fn w148_final_backend_completion_gate_lists_blockers_by_acceptance_gate() {
    let contracts = freeze_pre_ui_backend_read_model_contracts(PreUiBackendContractFreezeInput {
        generated_at: frozen_now(),
        evidence_timeline: evidence_timeline(),
        proposals: vec![proposal_with_raw_payload()],
        agent_runs: vec![runtime_run_with_raw_fields()],
        guidance_impact: guidance_impact(),
        governor_decisions: Vec::new(),
        lifemodel_version: lifemodel_version_read_model(),
    });
    let mut evidence = gate_evidence();
    evidence.runtime_driven_gate_passed = false;
    evidence.local_only_privacy_enforced = false;
    evidence.tool_governance_enforced = false;
    evidence.raw_content_excluded = false;

    let report = evaluate_final_backend_completion_gate(FinalBackendCompletionGateInput {
        generated_at: frozen_now(),
        contract_freeze: contracts,
        evidence,
    });

    assert!(!report.gate_ready);
    let runtime_gate = report
        .blockers_by_gate
        .iter()
        .find(|gate| gate.gate == "runtime_driven_gate")
        .unwrap();
    assert!(runtime_gate
        .blockers
        .contains(&"runtime_driven_gate_not_proven".to_string()));
    assert!(runtime_gate
        .blockers
        .contains(&"local_only_privacy_not_enforced".to_string()));
    assert!(runtime_gate
        .blockers
        .contains(&"tool_governance_not_enforced".to_string()));

    let governance_gate = report
        .blockers_by_gate
        .iter()
        .find(|gate| gate.gate == "governance_privacy_gate")
        .unwrap();
    assert!(governance_gate
        .blockers
        .contains(&"raw_content_not_excluded".to_string()));
}
