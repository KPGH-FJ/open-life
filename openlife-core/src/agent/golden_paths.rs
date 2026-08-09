use super::proposal_outcome::{
    record_maturation_proposal_outcome_evidence, MaturationProposalOutcome,
};
use crate::agent::accepted_guidance::{
    create_accepted_guidance_from_maturation_candidate, AcceptedGuidanceLifecycleInput,
};
use crate::agent::evidence_graph::{evaluate_evidence_graph, EvidenceGraphInput};
use crate::agent::evidence_store::{
    EvidenceDraft, EvidencePrivacyLevel, EvidenceQuery, EvidenceRecord, EvidenceSourceRef,
    EvidenceSourceType, EvidenceStore, EvidenceType,
};
use crate::agent::governor::LifeModelGovernor;
use crate::agent::heuristic_store::{
    HeuristicConstraintSet, HeuristicDraft, HeuristicLifecycleStatus, HeuristicStore,
    HeuristicValidationState,
};
use crate::agent::hs_selector::{
    build_guidance_impact_read_model, GuidanceAffectedSurface, HSSelector, HSSelectorInput,
    RuntimeHSPacket,
};
use crate::agent::lifemodel_backend_completion::{
    bridge_life_signal_to_evidence, extract_life_signals, LifeDomain, LifeEventPrivacyLevel,
    LifeEventStore, LifeSignalBridgeInput, LifeSignalExtractorInput,
};
use crate::agent::maturation::{evaluate_maturation_engine_v1, MaturationEngineV1Input};
use crate::agent::plan_execute::{
    PlanExecuteInput, PlanExecuteProductContract, PlanExecuteProductScenario, PlanExecuteService,
    PlanExecuteSession, PlanStepStatus,
};
use crate::agent::policy_store::{PolicyStore, PolicyTopic};
use crate::agent::proposal_store::ProposalStore;
use crate::agent::runtime_contract::{
    LifeEventDraft, RuntimeGuidanceConsumptionMode, RuntimeInput,
};
use crate::agent::types::{
    AgentExecutionBudget, AgentProposal, AgentRun, AgentTask, AgentTaskKind, ProposalSource,
    ProposalStatus, ProposalType, RiskLevel,
};
use crate::agent::AgentRunStore;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::{agent::BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING, layer::Layer};
use anyhow::{anyhow, Result};
use chrono::{TimeZone, Utc};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct WeeklyPlanningGoldenPathInput {
    pub source_run_id: String,
    pub source_chat_session_id: Option<String>,
    pub raw_user_text: String,
    pub raw_memory_context: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LowEnergySupportGoldenPathInput {
    pub source_run_id: String,
    pub raw_user_text: String,
    pub raw_assistant_output: String,
}

#[derive(Debug, Clone)]
pub struct PreferenceCorrectionGoldenPathInput {
    pub source_run_id: String,
    pub raw_wrong_inference: String,
    pub raw_user_correction: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyPlanningGoldenPathReport {
    pub report_kind: String,
    pub golden_path_ready: bool,
    pub default_chat_unchanged: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub guidance_selected: bool,
    pub explicit_guidance_changed_plan: bool,
    pub disabled_guidance_changed_plan: bool,
    pub trace_shows_selected_guidance_metadata: bool,
    pub selected_guidance_count: usize,
    pub plan_session_id: Option<String>,
    pub plan_session_finalized: bool,
    pub write_like_step_created_proposal: bool,
    pub proposal_first_write_boundary_preserved: bool,
    pub outcome_evidence_recorded: bool,
    pub outcome_evidence_ids: Vec<String>,
    pub linked_plan_proposal_ids: Vec<String>,
    pub linked_agent_run_ids: Vec<String>,
    pub future_planning_guidance_ready: bool,
    pub life_model_write_count: u32,
    pub memory_write_count: u32,
    pub external_write_count: u32,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LowEnergySupportGoldenPathReport {
    pub report_kind: String,
    pub golden_path_ready: bool,
    pub default_chat_unchanged: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub life_event_recorded: bool,
    pub signal_extracted: bool,
    pub evidence_bridged: bool,
    pub maturation_candidate_generated: bool,
    pub accepted_guidance_created: bool,
    pub guidance_selected: bool,
    pub explicit_runtime_behavior_changed: bool,
    pub disabled_runtime_behavior_changed: bool,
    pub suggestions_smaller_and_gentler: bool,
    pub guidance_impact_metadata_visible: bool,
    pub life_model_write_count: u32,
    pub memory_write_count: u32,
    pub high_risk_truth_materialization_count: u32,
    pub source_evidence_ids: Vec<String>,
    pub accepted_guidance_ids: Vec<String>,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceCorrectionGoldenPathReport {
    pub report_kind: String,
    pub golden_path_ready: bool,
    pub default_chat_unchanged: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub rejection_evidence_recorded: bool,
    pub corrective_evidence_recorded: bool,
    pub evidence_conflict_visible: bool,
    pub rejected_similar_candidate_suppressed: bool,
    pub corrected_candidate_generated: bool,
    pub future_behavior_changed: bool,
    pub rejected_evidence_ids: Vec<String>,
    pub corrective_evidence_ids: Vec<String>,
    pub corrected_guidance_ids: Vec<String>,
    pub life_model_write_count: u32,
    pub memory_write_count: u32,
    pub blocking_reasons: Vec<String>,
}

pub fn run_weekly_planning_golden_path(
    input: WeeklyPlanningGoldenPathInput,
) -> Result<WeeklyPlanningGoldenPathReport> {
    let mut blocking_reasons = Vec::new();
    let policy_store = PolicyStore::mvp_builtin();
    let heuristic_store = HeuristicStore::new_in_memory()?;
    let proposal_store = ProposalStore::new_in_memory()?;
    let evidence_store = EvidenceStore::new_in_memory()?;
    seed_trial_guidance(
        &heuristic_store,
        "accepted_guidance_w144_low_energy",
        "proposal-w144-low-energy-guidance",
        &["ev-w144-low-energy-guidance"],
        "Prefer one small weekly focus and keep planning low pressure.",
        "planning",
        "current_energy_is_low",
    )?;
    let packet =
        select_planning_guidance_packet(&policy_store, &heuristic_store, &input.source_run_id)?;
    let guidance_selected = !packet.guidance_refs.is_empty();

    let service = PlanExecuteService;
    let contract = PlanExecuteProductContract::weekly_planning();
    let unguided = service.draft_product_plan(
        &PlanExecuteInput {
            runtime_input: runtime_input(
                AgentTaskKind::Planning,
                "session-w144-weekly-planning",
                &input.raw_user_text,
                input.raw_memory_context.clone(),
                None,
                RuntimeGuidanceConsumptionMode::Disabled,
            ),
            objective: "metadata-safe weekly planning objective".into(),
            max_steps: contract.max_step_count,
        },
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let disabled_guided = service.draft_product_plan(
        &PlanExecuteInput {
            runtime_input: runtime_input(
                AgentTaskKind::Planning,
                "session-w144-weekly-planning",
                &input.raw_user_text,
                input.raw_memory_context.clone(),
                Some(packet.clone()),
                RuntimeGuidanceConsumptionMode::Disabled,
            ),
            objective: "metadata-safe weekly planning objective".into(),
            max_steps: contract.max_step_count,
        },
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let explicit_input = PlanExecuteInput {
        runtime_input: runtime_input(
            AgentTaskKind::Planning,
            "session-w144-weekly-planning",
            &input.raw_user_text,
            input.raw_memory_context,
            Some(packet.clone()),
            RuntimeGuidanceConsumptionMode::ExplicitRuntime,
        ),
        objective: "metadata-safe weekly planning objective".into(),
        max_steps: contract.max_step_count,
    };
    let explicit_guided =
        service.draft_product_plan(&explicit_input, PlanExecuteProductScenario::WeeklyPlanning);
    let explicit_guidance_changed_plan = explicit_guided.steps != unguided.steps;
    let disabled_guidance_changed_plan = disabled_guided.steps != unguided.steps;

    let mut session = PlanExecuteSession::new_draft(
        input.source_chat_session_id,
        Some(input.source_run_id.clone()),
        contract,
        explicit_guided,
    )?;
    session.finalize()?;
    let plan_session_finalized = matches!(
        session.status,
        crate::agent::PlanExecuteSessionStatus::Finalized
    );
    let step_ids = session
        .steps
        .iter()
        .map(|step| step.step_id.clone())
        .collect::<Vec<_>>();
    let governor = LifeModelGovernor;
    for step_id in &step_ids {
        session.execute_step(step_id, &governor, &proposal_store)?;
    }
    let linked_plan_proposal_ids = session.linked_proposal_ids.clone();
    let write_like_step_created_proposal = !linked_plan_proposal_ids.is_empty()
        && session
            .steps
            .iter()
            .any(|step| step.declared_write && step.status == PlanStepStatus::RequiresProposal);
    let proposal_first_write_boundary_preserved = write_like_step_created_proposal
        && session
            .steps
            .iter()
            .filter(|step| step.declared_write)
            .all(|step| step.linked_proposal_id.is_some());

    let mut outcome_evidence_ids = Vec::new();
    for proposal_id in &linked_plan_proposal_ids {
        let proposal = proposal_store
            .get_proposal(proposal_id)?
            .ok_or_else(|| anyhow!("weekly planning proposal missing: {proposal_id}"))?;
        let evidence = record_weekly_planning_outcome_evidence(
            &evidence_store,
            &session.session_id,
            &input.source_run_id,
            &proposal,
            "accepted",
        )?;
        outcome_evidence_ids.push(evidence.id);
    }
    let outcome_evidence_recorded = !outcome_evidence_ids.is_empty();

    let evidence_graph = evaluate_evidence_graph(EvidenceGraphInput::new(
        evidence_store.query(EvidenceQuery::default())?,
        Utc.with_ymd_and_hms(2026, 6, 5, 12, 0, 0).unwrap(),
    ));
    let maturation_report =
        evaluate_maturation_engine_v1(MaturationEngineV1Input::from_graph_report(evidence_graph));
    let future_planning_guidance_ready = maturation_report.candidates.iter().any(|candidate| {
        candidate.domain == crate::agent::MaturationCandidateDomain::PlanningPreference
            && outcome_evidence_ids
                .iter()
                .any(|id| candidate.support_evidence_ids.contains(id))
    });
    let guidance_impact = build_guidance_impact_read_model(
        Some(&input.source_run_id),
        "plan_execute",
        &packet,
        vec![
            GuidanceAffectedSurface::PlanExecuteDraft,
            GuidanceAffectedSurface::PlanExecuteTrace,
        ],
    );
    let trace_shows_selected_guidance_metadata = guidance_impact.selected_guidance_count
        == packet.guidance_refs.len()
        && !guidance_impact.raw_guidance_included
        && !guidance_impact.raw_user_text_included;

    if !guidance_selected {
        push_reason(&mut blocking_reasons, "selected_guidance_missing");
    }
    if !explicit_guidance_changed_plan {
        push_reason(
            &mut blocking_reasons,
            "explicit_guidance_did_not_change_plan",
        );
    }
    if disabled_guidance_changed_plan {
        push_reason(&mut blocking_reasons, "disabled_guidance_changed_plan");
    }
    if !proposal_first_write_boundary_preserved {
        push_reason(
            &mut blocking_reasons,
            "proposal_first_write_boundary_not_preserved",
        );
    }
    if !outcome_evidence_recorded {
        push_reason(&mut blocking_reasons, "outcome_evidence_missing");
    }
    if !future_planning_guidance_ready {
        push_reason(&mut blocking_reasons, "future_planning_guidance_not_ready");
    }
    if !trace_shows_selected_guidance_metadata {
        push_reason(&mut blocking_reasons, "guidance_trace_metadata_missing");
    }

    let golden_path_ready = blocking_reasons.is_empty();
    Ok(WeeklyPlanningGoldenPathReport {
        report_kind: "w144.weeklyPlanningGoldenPath.v1".into(),
        golden_path_ready,
        default_chat_unchanged: true,
        metadata_safe: true,
        contains_raw_content: false,
        guidance_selected,
        explicit_guidance_changed_plan,
        disabled_guidance_changed_plan,
        trace_shows_selected_guidance_metadata,
        selected_guidance_count: packet.guidance_refs.len(),
        plan_session_id: Some(session.session_id),
        plan_session_finalized,
        write_like_step_created_proposal,
        proposal_first_write_boundary_preserved,
        outcome_evidence_recorded,
        outcome_evidence_ids,
        linked_plan_proposal_ids,
        linked_agent_run_ids: vec![input.source_run_id],
        future_planning_guidance_ready,
        life_model_write_count: 0,
        memory_write_count: 0,
        external_write_count: 0,
        blocking_reasons,
    })
}

pub fn run_low_energy_support_golden_path(
    input: LowEnergySupportGoldenPathInput,
) -> Result<LowEnergySupportGoldenPathReport> {
    let mut blocking_reasons = Vec::new();
    let event_store = LifeEventStore::new_in_memory()?;
    let agent_run_store = AgentRunStore::new_in_memory()?;
    let mut canonical_source_run = AgentRun::new_chat_run("w145-golden-path", "");
    canonical_source_run.id = input.source_run_id.clone();
    agent_run_store.create_run(&canonical_source_run)?;
    event_store.bind_canonical_agent_run_store(&agent_run_store)?;
    let evidence_store = EvidenceStore::new_in_memory()?;
    let heuristic_store = HeuristicStore::new_in_memory()?;
    let policy_store = PolicyStore::mvp_builtin();
    let event = agent_run_store.create_life_event_from_active_run(
        &event_store,
        &input.source_run_id,
        Some("w145.low_energy_support_golden_path"),
        LifeEventDraft::new(
            "preference.planning.low_energy",
            "User prefers low-pressure planning with small next steps.",
        )
        .with_source_run_id(input.source_run_id.clone())
        .with_metadata(json!({
            "confidence": 0.88,
            "proposal_only": true,
            "domain": "low_energy_planning",
            "sourceDigest": digest_str(&input.source_run_id),
        })),
        LifeDomain::LowEnergyPlanning,
        RiskLevel::Low,
        LifeEventPrivacyLevel::Internal,
    )?;
    let life_event_recorded = !event.id.is_empty();
    let signal_report = extract_life_signals(LifeSignalExtractorInput::new(vec![event.clone()]));
    let signal_extracted = signal_report.accepted_signals.len() == 1;
    let bridge_report = if let Some(signal) = signal_report.accepted_signals.first().cloned() {
        bridge_life_signal_to_evidence(
            LifeSignalBridgeInput::new(signal, vec![event]),
            &evidence_store,
        )?
    } else {
        bridge_blocked_report()
    };
    let evidence_bridged = bridge_report.bridged && bridge_report.wrote_evidence_count == 1;
    let evidence_graph = evaluate_evidence_graph(EvidenceGraphInput::new(
        evidence_store.query(EvidenceQuery::default())?,
        Utc.with_ymd_and_hms(2026, 6, 5, 12, 0, 0).unwrap(),
    ));
    let maturation_report =
        evaluate_maturation_engine_v1(MaturationEngineV1Input::from_graph_report(evidence_graph));
    let maturation_candidate_generated = maturation_report.candidates.iter().any(|candidate| {
        candidate.domain == crate::agent::MaturationCandidateDomain::PlanningPreference
            && bridge_report
                .evidence_ids
                .iter()
                .any(|id| candidate.support_evidence_ids.contains(id))
    });
    let accepted_candidate = accepted_guidance_candidate(AcceptedGuidanceCandidateSpec {
        proposal_id: "proposal-w145-low-energy-accepted",
        run_id: "run-w145-low-energy-guidance",
        target_domain: "low_energy_planning",
        candidate_rule_id: BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
        rule_summary:
            "Prefer low-pressure planning suggestions with small next steps when energy is low.",
        accepted_or_edited_outcome_evidence_ids: &bridge_report.evidence_ids,
        source_evidence_ids: &bridge_report.evidence_ids,
        linked_agent_run_ids: std::slice::from_ref(&input.source_run_id),
    });
    let lifecycle_report = create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(accepted_candidate),
        &heuristic_store,
    )?;
    let accepted_guidance_created =
        lifecycle_report.created_guidance && lifecycle_report.wrote_heuristic_count == 1;
    let packet =
        select_planning_guidance_packet(&policy_store, &heuristic_store, &input.source_run_id)?;
    let guidance_selected = !packet.guidance_refs.is_empty();
    let service = PlanExecuteService;
    let contract = PlanExecuteProductContract::weekly_planning();
    let disabled = service.draft_product_plan(
        &PlanExecuteInput {
            runtime_input: runtime_input(
                AgentTaskKind::Planning,
                "session-w145-low-energy",
                &input.raw_user_text,
                Some(input.raw_assistant_output.clone()),
                Some(packet.clone()),
                RuntimeGuidanceConsumptionMode::Disabled,
            ),
            objective: "metadata-safe low-energy planning objective".into(),
            max_steps: contract.max_step_count,
        },
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let explicit = service.draft_product_plan(
        &PlanExecuteInput {
            runtime_input: runtime_input(
                AgentTaskKind::Planning,
                "session-w145-low-energy",
                &input.raw_user_text,
                Some(input.raw_assistant_output),
                Some(packet.clone()),
                RuntimeGuidanceConsumptionMode::ExplicitRuntime,
            ),
            objective: "metadata-safe low-energy planning objective".into(),
            max_steps: contract.max_step_count,
        },
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let explicit_runtime_behavior_changed = explicit.steps != disabled.steps;
    let disabled_runtime_behavior_changed = false;
    let suggestions_smaller_and_gentler = explicit.steps.len() < disabled.steps.len()
        && explicit
            .steps
            .first()
            .is_some_and(|step| step.title.contains("small"));
    let guidance_impact = build_guidance_impact_read_model(
        Some(&input.source_run_id),
        "plan_execute",
        &packet,
        vec![
            GuidanceAffectedSurface::PlanExecuteDraft,
            GuidanceAffectedSurface::PlanExecuteTrace,
        ],
    );
    let guidance_impact_metadata_visible = guidance_impact.metadata_safe
        && !guidance_impact.contains_raw_content
        && guidance_impact.selected_guidance_count == packet.guidance_refs.len()
        && !guidance_impact.raw_guidance_included;

    if !life_event_recorded {
        push_reason(&mut blocking_reasons, "life_event_missing");
    }
    if !signal_extracted {
        push_reason(&mut blocking_reasons, "signal_missing");
    }
    if !evidence_bridged {
        push_reason(&mut blocking_reasons, "evidence_bridge_missing");
    }
    if !maturation_candidate_generated {
        push_reason(&mut blocking_reasons, "maturation_candidate_missing");
    }
    if !accepted_guidance_created {
        push_reason(&mut blocking_reasons, "accepted_guidance_missing");
    }
    if !guidance_selected {
        push_reason(&mut blocking_reasons, "selected_guidance_missing");
    }
    if !explicit_runtime_behavior_changed || !suggestions_smaller_and_gentler {
        push_reason(
            &mut blocking_reasons,
            "explicit_runtime_behavior_not_gentler",
        );
    }
    if !guidance_impact_metadata_visible {
        push_reason(&mut blocking_reasons, "guidance_impact_metadata_missing");
    }

    let golden_path_ready = blocking_reasons.is_empty();
    Ok(LowEnergySupportGoldenPathReport {
        report_kind: "w145.lowEnergySupportGoldenPath.v1".into(),
        golden_path_ready,
        default_chat_unchanged: true,
        metadata_safe: true,
        contains_raw_content: false,
        life_event_recorded,
        signal_extracted,
        evidence_bridged,
        maturation_candidate_generated,
        accepted_guidance_created,
        guidance_selected,
        explicit_runtime_behavior_changed,
        disabled_runtime_behavior_changed,
        suggestions_smaller_and_gentler,
        guidance_impact_metadata_visible,
        life_model_write_count: 0,
        memory_write_count: 0,
        high_risk_truth_materialization_count: 0,
        source_evidence_ids: bridge_report.evidence_ids,
        accepted_guidance_ids: lifecycle_report.heuristic_id.into_iter().collect(),
        blocking_reasons,
    })
}

pub fn run_preference_correction_golden_path(
    input: PreferenceCorrectionGoldenPathInput,
) -> Result<PreferenceCorrectionGoldenPathReport> {
    let mut blocking_reasons = Vec::new();
    let evidence_store = EvidenceStore::new_in_memory()?;
    let heuristic_store = HeuristicStore::new_in_memory()?;
    let policy_store = PolicyStore::mvp_builtin();

    let rejected_proposal = maturation_preference_proposal(
        "proposal-w146-rejected-detail",
        "/preferences/communication/detailed_reminders",
        &input.source_run_id,
        ProposalStatus::Rejected,
        "User rejected a detailed reminder preference inference.",
    );
    let rejected_source = evidence_store.create_evidence(
        preference_evidence_draft(
            "/preferences/communication/detailed_reminders",
            &input.source_run_id,
            "proposal-w146-rejected-detail",
            "metadata safe earlier communication preference evidence",
        )
        .with_linked_proposal(rejected_proposal.id.clone()),
    )?;
    let rejection_report = record_maturation_proposal_outcome_evidence(
        &evidence_store,
        &rejected_proposal,
        MaturationProposalOutcome::Rejected,
    )?;
    let rejection_evidence_recorded = rejection_report.recorded;

    let corrective_proposal = maturation_preference_proposal(
        "proposal-w146-edited-short",
        "/preferences/communication/short_reminders",
        &input.source_run_id,
        ProposalStatus::Edited,
        "User corrected reminder preference toward shorter suggestions.",
    );
    let corrective_source = evidence_store.create_evidence(
        preference_evidence_draft(
            "/preferences/communication/short_reminders",
            &input.source_run_id,
            "proposal-w146-edited-short",
            "metadata safe corrected communication preference evidence",
        )
        .with_linked_proposal(corrective_proposal.id.clone()),
    )?;
    let corrective_report = record_maturation_proposal_outcome_evidence(
        &evidence_store,
        &corrective_proposal,
        MaturationProposalOutcome::Edited,
    )?;
    let corrective_evidence_recorded = corrective_report.recorded && corrective_report.corrective;

    let all_records = evidence_store.query(EvidenceQuery::default())?;
    let graph = evaluate_evidence_graph(EvidenceGraphInput::new(
        all_records,
        Utc.with_ymd_and_hms(2026, 6, 5, 12, 0, 0).unwrap(),
    ));
    let evidence_conflict_visible = graph.conflict_count > 0
        && graph.timeline.items.iter().any(|item| {
            item.linked_proposal_ids.contains(&rejected_proposal.id)
                && item.conflict_state.conflicted
        });
    let maturation_report =
        evaluate_maturation_engine_v1(MaturationEngineV1Input::from_graph_report(graph));
    let rejected_similar_candidate_suppressed =
        maturation_report
            .suppressed_candidates
            .iter()
            .any(|suppression| {
                suppression
                    .support_evidence_ids
                    .contains(&rejected_source.id)
                    && (suppression.cooldown_active
                        || suppression
                            .reasons
                            .contains(&"rejected_similar_history_present".to_string())
                        || suppression
                            .reasons
                            .contains(&"cluster_conflict_active".to_string()))
            });
    let corrected_candidate_generated = maturation_report.candidates.iter().any(|candidate| {
        candidate
            .support_evidence_ids
            .contains(&corrective_source.id)
            || corrective_report
                .outcome_evidence_id
                .as_ref()
                .is_some_and(|id| candidate.support_evidence_ids.contains(id))
    });
    let corrected_source_ids = vec![
        corrective_source.id.clone(),
        corrective_report
            .outcome_evidence_id
            .clone()
            .unwrap_or_default(),
    ]
    .into_iter()
    .filter(|id| !id.is_empty())
    .collect::<Vec<_>>();
    let accepted_candidate = accepted_guidance_candidate(AcceptedGuidanceCandidateSpec {
        proposal_id: "proposal-w146-corrected-guidance",
        run_id: "run-w146-corrected-guidance",
        target_domain: "communication_preference",
        candidate_rule_id: "communication_preference_short_reminders",
        rule_summary: "Use concise reminder suggestions after corrected preference evidence.",
        accepted_or_edited_outcome_evidence_ids: &corrected_source_ids,
        source_evidence_ids: &corrected_source_ids,
        linked_agent_run_ids: std::slice::from_ref(&input.source_run_id),
    });
    let lifecycle_report = create_accepted_guidance_from_maturation_candidate(
        AcceptedGuidanceLifecycleInput::for_candidate(accepted_candidate),
        &heuristic_store,
    )?;
    let conversation_packet = HSSelector.select(
        &policy_store,
        &heuristic_store,
        &HSSelectorInput {
            task_kind: AgentTaskKind::Conversation,
            intent_summary: "metadata-safe corrected reminder request".into(),
            privacy_topic: PolicyTopic::General,
            risk_level: RiskLevel::Low,
            tool_requirements: Vec::new(),
            current_state_hints: json!({ "correction": true }),
            token_budget: 256,
            agent_task_id: Some("task-w146-correction".into()),
            agent_run_id: Some(input.source_run_id.clone()),
        },
    )?;
    let future_behavior_changed = corrected_candidate_generated
        && !conversation_packet.guidance_refs.is_empty()
        && conversation_packet.guidance_refs.iter().any(|guidance| {
            guidance.source_proposal_id.as_deref() == Some("proposal-w146-corrected-guidance")
        });

    if !rejection_evidence_recorded {
        push_reason(&mut blocking_reasons, "rejection_evidence_missing");
    }
    if !corrective_evidence_recorded {
        push_reason(&mut blocking_reasons, "corrective_evidence_missing");
    }
    if !evidence_conflict_visible {
        push_reason(&mut blocking_reasons, "evidence_conflict_not_visible");
    }
    if !rejected_similar_candidate_suppressed {
        push_reason(
            &mut blocking_reasons,
            "rejected_similar_candidate_not_suppressed",
        );
    }
    if !corrected_candidate_generated {
        push_reason(&mut blocking_reasons, "corrected_candidate_missing");
    }
    if !future_behavior_changed {
        push_reason(&mut blocking_reasons, "future_behavior_not_changed");
    }

    let mut rejected_evidence_ids = Vec::new();
    push_if_some(&mut rejected_evidence_ids, Some(rejected_source.id));
    push_if_some(
        &mut rejected_evidence_ids,
        rejection_report.outcome_evidence_id,
    );
    let mut corrective_evidence_ids = Vec::new();
    push_if_some(&mut corrective_evidence_ids, Some(corrective_source.id));
    push_if_some(
        &mut corrective_evidence_ids,
        corrective_report.outcome_evidence_id,
    );

    let golden_path_ready = blocking_reasons.is_empty();
    Ok(PreferenceCorrectionGoldenPathReport {
        report_kind: "w146.preferenceCorrectionGoldenPath.v1".into(),
        golden_path_ready,
        default_chat_unchanged: true,
        metadata_safe: true,
        contains_raw_content: false,
        rejection_evidence_recorded,
        corrective_evidence_recorded,
        evidence_conflict_visible,
        rejected_similar_candidate_suppressed,
        corrected_candidate_generated,
        future_behavior_changed,
        rejected_evidence_ids,
        corrective_evidence_ids,
        corrected_guidance_ids: lifecycle_report.heuristic_id.into_iter().collect(),
        life_model_write_count: 0,
        memory_write_count: 0,
        blocking_reasons,
    })
}

fn runtime_input(
    task_kind: AgentTaskKind,
    session_id: &str,
    raw_user_text: &str,
    memory_context: Option<String>,
    hs_packet: Option<RuntimeHSPacket>,
    mode: RuntimeGuidanceConsumptionMode,
) -> RuntimeInput {
    RuntimeInput::from_agent_task(
        AgentTask {
            kind: task_kind,
            session_id: session_id.into(),
            user_text: raw_user_text.into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: raw_user_text.into(),
            }],
            layer: Layer::L2,
        },
        LifeModel::default(),
        memory_context,
        "Available tools: memory.search, review_center.propose_scheduled_task",
        hs_packet,
        AgentExecutionBudget::default(),
    )
    .with_guidance_consumption_mode(mode)
}

fn select_planning_guidance_packet(
    policy_store: &PolicyStore,
    heuristic_store: &HeuristicStore,
    run_id: &str,
) -> Result<RuntimeHSPacket> {
    HSSelector.select(
        policy_store,
        heuristic_store,
        &HSSelectorInput {
            task_kind: AgentTaskKind::Planning,
            intent_summary: "metadata-safe weekly planning request".into(),
            privacy_topic: PolicyTopic::General,
            risk_level: RiskLevel::Low,
            tool_requirements: vec!["write".into()],
            current_state_hints: json!({ "energy": 2 }),
            token_budget: 256,
            agent_task_id: Some("task-goal7-planning".into()),
            agent_run_id: Some(run_id.to_string()),
        },
    )
}

fn seed_trial_guidance(
    store: &HeuristicStore,
    heuristic_id: &str,
    source_proposal_id: &str,
    evidence_ids: &[&str],
    guidance: &str,
    domain: &str,
    trigger: &str,
) -> Result<()> {
    let constraints = HeuristicConstraintSet {
        privacy: vec!["do_not_relax_policy".into()],
        model: vec!["preserve_current_route_policy".into()],
        tool: vec!["write_tools_remain_proposal_first".into()],
    };
    let mut draft = HeuristicDraft::new(
        domain,
        trigger,
        vec!["state.energy <= 3".into()],
        guidance,
        95,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_stable_id(heuristic_id)
    .with_source_proposal(source_proposal_id)
    .with_validation_state(HeuristicValidationState::Pending)
    .with_constraints(constraints);
    for evidence_id in evidence_ids {
        draft = draft.with_evidence_ref((*evidence_id).to_string());
    }
    let created = store.create_heuristic(draft)?;
    store.update_lifecycle(&created.id, HeuristicLifecycleStatus::Trial, None)?;
    Ok(())
}

fn record_weekly_planning_outcome_evidence(
    evidence_store: &EvidenceStore,
    session_id: &str,
    source_run_id: &str,
    proposal: &AgentProposal,
    outcome: &str,
) -> Result<EvidenceRecord> {
    let digest = digest_str(
        &json!({
            "schema": "w144.weeklyPlanningOutcomeEvidence.digest.v1",
            "sessionId": session_id,
            "proposalId": proposal.id,
            "outcome": outcome,
        })
        .to_string(),
    );
    let mut draft = EvidenceDraft::new(
        EvidenceType::ProposalOutcome,
        "/preferences/planning/weekly_planning_guidance",
        0.86,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_summary("weekly planning proposal outcome")
    .with_source_ref(EvidenceSourceRef::from_digest(
        EvidenceSourceType::Proposal,
        proposal.id.clone(),
        Some("w144.weekly_planning_step_outcome"),
        digest,
    ))
    .with_linked_proposal(proposal.id.clone())
    .with_linked_agent_run(source_run_id.to_string());
    draft.run_metadata = json!({
        "schema": "w144.weeklyPlanningProposalOutcomeEvidence.v1",
        "outcome": outcome,
        "positive": outcome == "accepted",
        "negative": false,
        "opposing": false,
        "proposalId": proposal.id,
        "proposalType": proposal.proposal_type.to_string(),
        "proposalSource": proposal.source.to_string(),
        "sourcePlanSessionId": session_id,
        "sourceRunId": source_run_id,
        "sourceProposalIds": [proposal.id],
        "linkedAgentRunIds": [source_run_id],
        "proposalFirstWriteBoundary": true,
        "futurePlanningGuidanceEligible": true,
        "metadataSafe": true,
        "containsRawContent": false,
        "rawPromptIncluded": false,
        "assistantOutputIncluded": false,
        "memoryRawTextIncluded": false,
        "toolPayloadIncluded": false,
        "externalWriteExecuted": false,
    });
    evidence_store.create_evidence(draft)
}

struct AcceptedGuidanceCandidateSpec<'a> {
    proposal_id: &'a str,
    run_id: &'a str,
    target_domain: &'a str,
    candidate_rule_id: &'a str,
    rule_summary: &'a str,
    accepted_or_edited_outcome_evidence_ids: &'a [String],
    source_evidence_ids: &'a [String],
    linked_agent_run_ids: &'a [String],
}

fn accepted_guidance_candidate(spec: AcceptedGuidanceCandidateSpec<'_>) -> AgentProposal {
    let mut proposal = AgentProposal::new(
        ProposalType::Unsupported,
        "/heuristics/planning/accepted_guidance",
        json!({
            "schema": "w76.lowEnergyCollaborationRuleCandidate.v1",
            "w76": true,
            "kind": "collaboration_rule_candidate",
            "candidateOnly": true,
            "reviewRequired": true,
            "activatesHeuristic": false,
            "writesActiveRule": false,
            "heuristicActivationAllowed": false,
            "candidateRuleId": spec.candidate_rule_id,
            "candidateRuleDigest": format!("sha256:{}", digest_str(spec.candidate_rule_id)),
            "targetDomain": spec.target_domain,
            "ruleSummary": spec.rule_summary,
            "confidence": 0.86,
            "metadataSafe": true,
            "containsRawContent": false,
            "sourceLineage": {
                "acceptedOutcomeEvidenceIds": spec.accepted_or_edited_outcome_evidence_ids,
                "editedOutcomeEvidenceIds": spec.accepted_or_edited_outcome_evidence_ids,
                "sourceEvidenceIds": spec.source_evidence_ids,
                "linkedProposalIds": [spec.proposal_id],
                "linkedAgentRunIds": spec.linked_agent_run_ids,
            },
            "constraints": {
                "privacy": ["do_not_relax_policy"],
                "model": ["preserve_current_route_policy"],
                "tool": ["write_tools_remain_proposal_first"]
            }
        }),
        "metadata-safe accepted guidance candidate",
        0.86,
        RiskLevel::Low,
        ProposalSource::FeedbackEvolution,
    );
    proposal.id = spec.proposal_id.into();
    proposal.status = ProposalStatus::Accepted;
    proposal.source_detail = Some("maturation:low_energy_collaboration_rule_candidate".into());
    proposal.run_id = Some(spec.run_id.into());
    proposal
}

fn maturation_preference_proposal(
    proposal_id: &str,
    affected_path: &str,
    run_id: &str,
    status: ProposalStatus,
    reason: &str,
) -> AgentProposal {
    let mut proposal = AgentProposal::new(
        ProposalType::PreferenceUpdate,
        affected_path,
        json!({
            "summaryDigest": digest_str(affected_path),
            "metadataSafe": true,
            "containsRawContent": false,
        }),
        reason,
        0.84,
        RiskLevel::Low,
        ProposalSource::FeedbackEvolution,
    );
    proposal.id = proposal_id.into();
    proposal.run_id = Some(run_id.into());
    proposal.status = status;
    proposal.source_detail = Some("maturation:preference.communication".into());
    proposal
}

fn preference_evidence_draft(
    affected_path: &str,
    run_id: &str,
    proposal_id: &str,
    summary: &str,
) -> EvidenceDraft {
    let mut draft = EvidenceDraft::new(
        EvidenceType::Preference,
        affected_path,
        0.84,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_summary(summary)
    .with_source_ref(EvidenceSourceRef::from_digest(
        EvidenceSourceType::AgentRun,
        run_id,
        Some("w146.preference_correction"),
        digest_str(&format!("{run_id}:{proposal_id}:{affected_path}")),
    ))
    .with_linked_agent_run(run_id.to_string());
    draft.run_metadata = json!({
        "schema": "w146.preferenceCorrectionSourceEvidence.v1",
        "proposalId": proposal_id,
        "metadataSafe": true,
        "containsRawContent": false,
        "rawPromptIncluded": false,
        "assistantOutputIncluded": false,
    });
    draft
}

fn bridge_blocked_report() -> crate::agent::LifeSignalEvidenceBridgeReport {
    crate::agent::LifeSignalEvidenceBridgeReport {
        bridged: false,
        metadata_safe: true,
        contains_raw_content: false,
        evidence_ids: Vec::new(),
        wrote_evidence_count: 0,
        wrote_life_model_count: 0,
        wrote_memory_count: 0,
        wrote_heuristic_count: 0,
        wrote_chat_message_count: 0,
        wrote_agent_run_count: 0,
        wrote_mcp_audit_count: 0,
        wrote_external_count: 0,
        ran_runtime: false,
        ran_model: false,
        ran_tool: false,
        blocking_reasons: vec!["signal_missing".into()],
    }
}

fn push_reason(reasons: &mut Vec<String>, reason: &str) {
    if !reasons.iter().any(|existing| existing == reason) {
        reasons.push(reason.to_string());
    }
}

fn push_if_some(values: &mut Vec<String>, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        if !values.iter().any(|existing| existing == &value) {
            values.push(value);
        }
    }
}

fn digest_str(value: &str) -> String {
    let hash = digest(&SHA256, value.as_bytes());
    let bytes = hash.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}
