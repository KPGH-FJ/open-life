use crate::commands::agent_runtime::{
    cancel_plan_execute_session_with_state, create_plan_execute_session_with_state,
    execute_plan_execute_step_with_state, finalize_plan_execute_session_with_state,
    review_plan_execute_session_with_state, skip_plan_execute_step_with_state,
    update_plan_execute_session_draft_with_state, CancelPlanExecuteSessionInput,
    CreatePlanExecuteSessionInput, ExecutePlanExecuteStepInput, FinalizePlanExecuteSessionInput,
    PlanExecuteStepEditInput, ReviewPlanExecuteSessionInput, SkipPlanExecuteStepInput,
    UpdatePlanExecuteSessionDraftInput,
};
use crate::main_chat_event_stream::list_main_chat_agent_events_with_state;
use openlife_core::agent::PlanStepStatus;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2PlanScenario {
    pub id: String,
    pub capability_group: String,
    pub prompt: String,
    pub preconditions: Vec<String>,
    pub expected_route: String,
    pub required_runtime_evidence: Vec<String>,
    pub required_ui_state: Vec<String>,
    pub required_controls: Vec<String>,
    pub negative_assertions: Vec<String>,
    pub expected_outcome: String,
    pub default_gate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2PlanProof {
    pub scenario_id: String,
    pub passed: bool,
    pub expected_blocker: bool,
    pub plan_id: Option<String>,
    pub revision: Option<u64>,
    pub step_ids: Vec<String>,
    pub event_types: Vec<String>,
    pub linked_action_ids: Vec<String>,
    pub linked_observation_ids: Vec<String>,
    pub linked_proposal_ids: Vec<String>,
    pub blocker_ids: Vec<String>,
    pub controls: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2PlanGateReport {
    pub scenario_count: usize,
    pub default_gate_scenario_count: usize,
    pub passed_scenario_count: usize,
    pub expected_blocker_count: usize,
    pub ready: bool,
    pub blockers: Vec<String>,
    pub scenarios: Vec<MainChatProductMaturityV2PlanScenario>,
    pub proofs: Vec<MainChatProductMaturityV2PlanProof>,
}

pub(crate) fn main_chat_product_maturity_v2_plan_scenarios(
) -> Vec<MainChatProductMaturityV2PlanScenario> {
    vec![
        plan_scenario(
            "PI-01",
            "Plan this work before executing.",
            &["none"],
            &[
                "plan_id",
                "revision",
                "stable_step_ids",
                "plan.created",
                "step.created",
            ],
            &["plan_draft_visible"],
            &["confirm_plan", "edit_plan", "skip_step"],
            &["no_frontend_only_plan"],
            "pass",
        ),
        plan_scenario(
            "PI-02",
            "Confirm this plan.",
            &["plan_id", "base_revision"],
            &["plan.confirmed", "confirmed_at"],
            &["plan_confirmed_visible"],
            &["execute_step", "skip_step", "cancel_task"],
            &["no_stale_revision_execution"],
            "pass",
        ),
        plan_scenario(
            "PI-03",
            "Edit step 2 before running.",
            &["plan_id", "base_revision", "step_id"],
            &["plan.updated", "step.updated", "revision_incremented"],
            &["edited_step_visible"],
            &["confirm_plan", "edit_plan"],
            &["no_freeform_plan_patch"],
            "pass",
        ),
        plan_scenario(
            "PI-04",
            "Run the first read-only step.",
            &["plan_id", "base_revision", "read_step_id"],
            &["step.updated", "action.completed", "observation.created"],
            &["step_execution_links_visible"],
            &["execute_step"],
            &["no_write_execution"],
            "pass",
        ),
        plan_scenario(
            "PI-05",
            "Skip the unsupported step.",
            &["plan_id", "base_revision", "step_id", "skip_reason"],
            &["step.skipped", "skip_reason"],
            &["skipped_step_visible"],
            &["skip_step"],
            &["no_silent_skip"],
            "pass",
        ),
        plan_scenario(
            "PI-06",
            "Run a write-like step.",
            &["plan_id", "base_revision", "write_like_step_id"],
            &[
                "step.updated",
                "proposal.created",
                "linked_proposal_or_blocker",
            ],
            &["proposal_first_write_boundary_visible"],
            &["open_review_center"],
            &[
                "no_direct_lifemodel_write",
                "no_direct_memory_write",
                "no_file_or_external_write",
            ],
            "expected_blocker",
        ),
        plan_scenario(
            "PI-07",
            "Cancel remaining steps.",
            &["plan_id", "base_revision", "remaining_step_ids"],
            &["plan.updated", "step.updated", "step.cancelled"],
            &["cancelled_step_visible"],
            &["open_trace"],
            &["no_execute_or_skip_control_for_cancelled_step"],
            "pass",
        ),
        plan_scenario(
            "PI-08",
            "Review what happened.",
            &["plan_id", "base_revision", "reviewable_runtime_evidence"],
            &["plan.reviewed", "review_sections"],
            &["review_summary_visible"],
            &["open_trace"],
            &["no_completion_claim_without_linked_evidence"],
            "pass",
        ),
        plan_scenario(
            "PI-STALE-01",
            "Run a step against an old plan revision.",
            &["plan_id", "stale_base_revision", "step_id"],
            &["blocker.created", "stale_plan_revision"],
            &["stale_revision_blocker_visible"],
            &[],
            &["no_stale_revision_execution", "no_silent_action"],
            "expected_blocker",
        ),
        plan_scenario(
            "PI-INVALID-01",
            "Run a step id that does not belong to the plan.",
            &["plan_id", "base_revision", "invalid_step_id"],
            &["blocker.created", "invalid_plan_step"],
            &["invalid_step_blocker_visible"],
            &[],
            &["no_invalid_step_execution", "no_silent_action"],
            "expected_blocker",
        ),
    ]
}

pub(crate) async fn run_main_chat_agent_product_maturity_v2_plan_gate(
) -> MainChatProductMaturityV2PlanGateReport {
    let scenarios = main_chat_product_maturity_v2_plan_scenarios();
    let mut proofs = Vec::new();
    match run_plan_runtime_proofs().await {
        Ok(runtime_proofs) => proofs = runtime_proofs,
        Err(error) => proofs.push(MainChatProductMaturityV2PlanProof {
            scenario_id: "phase_c_runtime".into(),
            passed: false,
            expected_blocker: false,
            plan_id: None,
            revision: None,
            step_ids: Vec::new(),
            event_types: Vec::new(),
            linked_action_ids: Vec::new(),
            linked_observation_ids: Vec::new(),
            linked_proposal_ids: Vec::new(),
            blocker_ids: Vec::new(),
            controls: Vec::new(),
            diagnostics: vec![error],
        }),
    }
    let passed_scenario_count = proofs.iter().filter(|proof| proof.passed).count();
    let expected_blocker_count = proofs.iter().filter(|proof| proof.expected_blocker).count();
    let mut blockers = Vec::new();
    if passed_scenario_count != scenarios.len() {
        blockers.push("phase_c_plan_scenarios_not_ready".into());
    }
    MainChatProductMaturityV2PlanGateReport {
        scenario_count: scenarios.len(),
        default_gate_scenario_count: scenarios.len(),
        passed_scenario_count,
        expected_blocker_count,
        ready: blockers.is_empty(),
        blockers,
        scenarios,
        proofs,
    }
}

async fn run_plan_runtime_proofs() -> Result<Vec<MainChatProductMaturityV2PlanProof>, String> {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = create_plan_execute_session_with_state(
        CreatePlanExecuteSessionInput {
            scenario_id: Some("weekly_planning".into()),
            source_chat_session_id: Some("phase-c-plan-gate".into()),
            max_steps: Some(5),
        },
        &state,
    )
    .await?;
    let mut proofs = vec![proof_for_events(
        "PI-01",
        &session,
        events_for(&state, &session.session_id).await?,
        &["plan.created", "step.created"],
        vec![
            "confirm_plan".into(),
            "edit_plan".into(),
            "skip_step".into(),
        ],
        false,
    )];

    let second_step_id = session
        .steps
        .get(1)
        .map(|step| step.step_id.clone())
        .ok_or_else(|| "phase C plan fixture missing step 2".to_string())?;
    let edited = update_plan_execute_session_draft_with_state(
        UpdatePlanExecuteSessionDraftInput {
            session_id: session.session_id.clone(),
            base_revision: Some(session.revision),
            steps: vec![PlanExecuteStepEditInput {
                step_id: second_step_id,
                title: Some("Edited Phase C step".into()),
                intent: None,
                action_kind: None,
                tool_name: None,
                declared_write: None,
                risk_level: None,
            }],
        },
        &state,
    )
    .await?;
    proofs.push(proof_for_events(
        "PI-03",
        &edited,
        events_for(&state, &edited.session_id).await?,
        &["plan.updated", "step.updated"],
        vec!["confirm_plan".into(), "edit_plan".into()],
        false,
    ));

    let confirmed = finalize_plan_execute_session_with_state(
        FinalizePlanExecuteSessionInput {
            session_id: edited.session_id.clone(),
            base_revision: Some(edited.revision),
        },
        &state,
    )
    .await?;
    proofs.push(proof_for_events(
        "PI-02",
        &confirmed,
        events_for(&state, &confirmed.session_id).await?,
        &["plan.confirmed"],
        vec![
            "execute_step".into(),
            "skip_step".into(),
            "cancel_task".into(),
        ],
        false,
    ));

    let read_step_id = confirmed
        .steps
        .iter()
        .find(|step| !step.declared_write)
        .map(|step| step.step_id.clone())
        .ok_or_else(|| "phase C plan fixture missing read step".to_string())?;
    let stale = execute_plan_execute_step_with_state(
        ExecutePlanExecuteStepInput {
            session_id: confirmed.session_id.clone(),
            step_id: Some(read_step_id.clone()),
            base_revision: Some(confirmed.revision - 1),
        },
        &state,
    )
    .await;
    let stale_events = events_for(&state, &confirmed.session_id).await?;
    let stale_passed = stale.is_err() && has_blocker_reason(&stale_events, "stale_plan_revision");
    proofs.push(MainChatProductMaturityV2PlanProof {
        scenario_id: "PI-STALE-01".into(),
        passed: stale_passed,
        expected_blocker: true,
        plan_id: Some(confirmed.plan_id.clone()),
        revision: Some(confirmed.revision),
        step_ids: confirmed
            .steps
            .iter()
            .map(|step| step.step_id.clone())
            .collect(),
        event_types: stale_events
            .iter()
            .map(|event| event.event_type.clone())
            .collect(),
        linked_action_ids: Vec::new(),
        linked_observation_ids: Vec::new(),
        linked_proposal_ids: Vec::new(),
        blocker_ids: stale_events
            .iter()
            .filter(|event| event.event_type == "blocker.created")
            .map(|event| event.object_id.clone())
            .collect(),
        controls: Vec::new(),
        diagnostics: if stale_passed {
            Vec::new()
        } else {
            vec!["stale_revision_did_not_block".into()]
        },
    });

    let invalid = execute_plan_execute_step_with_state(
        ExecutePlanExecuteStepInput {
            session_id: confirmed.session_id.clone(),
            step_id: Some("phase-c-missing-step".into()),
            base_revision: Some(confirmed.revision),
        },
        &state,
    )
    .await;
    let invalid_events = events_for(&state, &confirmed.session_id).await?;
    let invalid_passed =
        invalid.is_err() && has_blocker_reason(&invalid_events, "invalid_plan_step");
    proofs.push(MainChatProductMaturityV2PlanProof {
        scenario_id: "PI-INVALID-01".into(),
        passed: invalid_passed,
        expected_blocker: true,
        plan_id: Some(confirmed.plan_id.clone()),
        revision: Some(confirmed.revision),
        step_ids: confirmed
            .steps
            .iter()
            .map(|step| step.step_id.clone())
            .collect(),
        event_types: invalid_events
            .iter()
            .map(|event| event.event_type.clone())
            .collect(),
        linked_action_ids: Vec::new(),
        linked_observation_ids: Vec::new(),
        linked_proposal_ids: Vec::new(),
        blocker_ids: invalid_events
            .iter()
            .filter(|event| event.event_type == "blocker.created")
            .map(|event| event.object_id.clone())
            .collect(),
        controls: Vec::new(),
        diagnostics: if invalid_passed {
            Vec::new()
        } else {
            vec!["invalid_step_did_not_block".into()]
        },
    });

    let executed = execute_plan_execute_step_with_state(
        ExecutePlanExecuteStepInput {
            session_id: confirmed.session_id.clone(),
            step_id: Some(read_step_id),
            base_revision: Some(confirmed.revision),
        },
        &state,
    )
    .await?;
    let mut pi04 = proof_for_events(
        "PI-04",
        &executed.session,
        events_for(&state, &executed.session.session_id).await?,
        &["step.updated", "action.completed", "observation.created"],
        vec!["execute_step".into()],
        false,
    );
    pi04.linked_action_ids = executed.executed_step.linked_action_ids.clone();
    pi04.linked_observation_ids = executed.executed_step.linked_observation_ids.clone();
    pi04.passed = pi04.passed
        && executed.executed_step.step_status == PlanStepStatus::Executed
        && !pi04.linked_action_ids.is_empty()
        && !pi04.linked_observation_ids.is_empty();
    proofs.push(pi04);

    let skip_step_id = executed
        .session
        .steps
        .iter()
        .find(|step| step.status == PlanStepStatus::Planned)
        .map(|step| step.step_id.clone())
        .ok_or_else(|| "phase C plan fixture missing skippable step".to_string())?;
    let skipped = skip_plan_execute_step_with_state(
        SkipPlanExecuteStepInput {
            session_id: executed.session.session_id.clone(),
            step_id: skip_step_id,
            base_revision: executed.session.revision,
            reason: "unsupported in Phase C deterministic gate".into(),
        },
        &state,
    )
    .await?;
    let mut pi05 = proof_for_events(
        "PI-05",
        &skipped.session,
        events_for(&state, &skipped.session.session_id).await?,
        &["step.skipped"],
        vec!["skip_step".into()],
        false,
    );
    pi05.passed = pi05.passed
        && skipped.skipped_step.step_status == PlanStepStatus::Skipped
        && skipped.skipped_step.skip_reason.is_some();
    proofs.push(pi05);

    let write_step_id = skipped
        .session
        .steps
        .iter()
        .find(|step| step.declared_write && step.status == PlanStepStatus::Planned)
        .map(|step| step.step_id.clone())
        .ok_or_else(|| "phase C plan fixture missing write-like step".to_string())?;
    let write_like = execute_plan_execute_step_with_state(
        ExecutePlanExecuteStepInput {
            session_id: skipped.session.session_id.clone(),
            step_id: Some(write_step_id),
            base_revision: Some(skipped.session.revision),
        },
        &state,
    )
    .await?;
    let write_events = events_for(&state, &write_like.session.session_id).await?;
    let mut pi06 = proof_for_events(
        "PI-06",
        &write_like.session,
        write_events.clone(),
        &["step.updated", "proposal.created"],
        vec!["open_review_center".into()],
        true,
    );
    pi06.linked_proposal_ids = write_like.executed_step.linked_proposal_ids.clone();
    pi06.blocker_ids = write_like.executed_step.blocker_ids.clone();
    pi06.linked_action_ids = write_like.executed_step.linked_action_ids.clone();
    pi06.linked_observation_ids = write_like.executed_step.linked_observation_ids.clone();
    pi06.passed = pi06.passed
        && write_like.executed_step.step_status == PlanStepStatus::RequiresProposal
        && (!pi06.linked_proposal_ids.is_empty() || !pi06.blocker_ids.is_empty())
        && pi06.linked_action_ids.is_empty()
        && pi06.linked_observation_ids.is_empty()
        && events_prove_no_direct_writes(&write_events);
    if !events_prove_no_direct_writes(&write_events) {
        pi06.diagnostics
            .push("write_like_direct_write_detected".into());
    }
    proofs.push(pi06);

    let reviewed = review_plan_execute_session_with_state(
        ReviewPlanExecuteSessionInput {
            session_id: write_like.session.session_id.clone(),
            base_revision: Some(write_like.session.revision),
        },
        &state,
    )
    .await?;
    let review_events = events_for(&state, &reviewed.session.session_id).await?;
    let mut pi08 = proof_for_events(
        "PI-08",
        &reviewed.session,
        review_events.clone(),
        &["plan.reviewed"],
        vec!["open_trace".into()],
        false,
    );
    let required_sections_present = !reviewed.summary.completed_steps.is_empty()
        && !reviewed.summary.skipped_steps.is_empty()
        && !reviewed.summary.proposals_created.is_empty()
        && !reviewed.summary.observations_used.is_empty()
        && !reviewed.summary.recommended_next_action.is_empty()
        && reviewed.summary.unresolved.is_empty()
        && reviewed.summary.completion_claimed;
    pi08.passed = pi08.passed
        && required_sections_present
        && review_events.iter().any(|event| {
            event.event_type == "plan.reviewed" && event.object_id == reviewed.summary.review_id
        });
    if !required_sections_present {
        pi08.diagnostics
            .push("review_summary_sections_missing_or_unbacked".into());
    }
    proofs.push(pi08);

    let cancel_session = create_plan_execute_session_with_state(
        CreatePlanExecuteSessionInput {
            scenario_id: Some("weekly_planning".into()),
            source_chat_session_id: Some("phase-c-plan-cancel-gate".into()),
            max_steps: Some(5),
        },
        &state,
    )
    .await?;
    let cancel_confirmed = finalize_plan_execute_session_with_state(
        FinalizePlanExecuteSessionInput {
            session_id: cancel_session.session_id.clone(),
            base_revision: Some(cancel_session.revision),
        },
        &state,
    )
    .await?;
    let cancel_read_step_id = cancel_confirmed
        .steps
        .iter()
        .find(|step| !step.declared_write)
        .map(|step| step.step_id.clone())
        .ok_or_else(|| "phase C cancel fixture missing read step".to_string())?;
    let cancel_executed = execute_plan_execute_step_with_state(
        ExecutePlanExecuteStepInput {
            session_id: cancel_confirmed.session_id.clone(),
            step_id: Some(cancel_read_step_id),
            base_revision: Some(cancel_confirmed.revision),
        },
        &state,
    )
    .await?;
    let cancelled = cancel_plan_execute_session_with_state(
        CancelPlanExecuteSessionInput {
            session_id: cancel_executed.session.session_id.clone(),
            base_revision: Some(cancel_executed.session.revision),
        },
        &state,
    )
    .await?;
    let cancel_events = events_for(&state, &cancelled.session_id).await?;
    let mut pi07 = proof_for_events(
        "PI-07",
        &cancelled,
        cancel_events,
        &["plan.updated", "step.updated", "step.cancelled"],
        vec!["open_trace".into()],
        false,
    );
    pi07.passed = pi07.passed
        && cancelled
            .steps
            .iter()
            .filter(|step| step.status == PlanStepStatus::Cancelled)
            .all(|step| {
                step.status_reason.as_deref() == Some("cancelled_by_user")
                    && step
                        .evidence_ids
                        .iter()
                        .any(|id| id.contains("plan-step-cancel"))
            })
        && cancelled
            .steps
            .iter()
            .any(|step| step.status == PlanStepStatus::Cancelled);
    proofs.push(pi07);

    Ok(proofs)
}

async fn events_for(
    state: &std::sync::Arc<crate::AppState>,
    task_session_id: &str,
) -> Result<Vec<crate::main_chat_event_stream::MainChatAgentDurableEvent>, String> {
    list_main_chat_agent_events_with_state(state, task_session_id.to_string(), Some(0), Some(250))
        .await
}

fn proof_for_events(
    scenario_id: &str,
    session: &openlife_core::agent::PlanExecuteSession,
    events: Vec<crate::main_chat_event_stream::MainChatAgentDurableEvent>,
    required_events: &[&str],
    controls: Vec<String>,
    expected_blocker: bool,
) -> MainChatProductMaturityV2PlanProof {
    let event_types = events
        .iter()
        .map(|event| event.event_type.clone())
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    for required in required_events {
        if !event_types.iter().any(|event_type| event_type == required) {
            diagnostics.push(format!("missing_event:{required}"));
        }
    }
    MainChatProductMaturityV2PlanProof {
        scenario_id: scenario_id.into(),
        passed: diagnostics.is_empty(),
        expected_blocker,
        plan_id: Some(session.plan_id.clone()),
        revision: Some(session.revision),
        step_ids: session
            .steps
            .iter()
            .map(|step| step.step_id.clone())
            .collect(),
        event_types,
        linked_action_ids: Vec::new(),
        linked_observation_ids: Vec::new(),
        linked_proposal_ids: session.linked_proposal_ids.clone(),
        blocker_ids: events
            .iter()
            .filter(|event| event.event_type == "blocker.created")
            .map(|event| event.object_id.clone())
            .collect(),
        controls,
        diagnostics,
    }
}

fn has_blocker_reason(
    events: &[crate::main_chat_event_stream::MainChatAgentDurableEvent],
    reason_code: &str,
) -> bool {
    events.iter().any(|event| {
        event.event_type == "blocker.created"
            && event
                .payload
                .get("reasonCode")
                .and_then(|value| value.as_str())
                == Some(reason_code)
    })
}

fn events_prove_no_direct_writes(
    events: &[crate::main_chat_event_stream::MainChatAgentDurableEvent],
) -> bool {
    events.iter().all(|event| {
        event
            .payload
            .get("directLifeModelWrites")
            .and_then(|value| value.as_bool())
            != Some(true)
            && event
                .payload
                .get("memoryWrites")
                .and_then(|value| value.as_bool())
                != Some(true)
            && event
                .payload
                .get("externalWritesExecuted")
                .and_then(|value| value.as_bool())
                != Some(true)
            && event
                .payload
                .get("directWritesExecuted")
                .and_then(|value| value.as_bool())
                != Some(true)
    })
}

#[allow(clippy::too_many_arguments)]
fn plan_scenario(
    id: &str,
    prompt: &str,
    preconditions: &[&str],
    required_runtime_evidence: &[&str],
    required_ui_state: &[&str],
    required_controls: &[&str],
    negative_assertions: &[&str],
    expected_outcome: &str,
) -> MainChatProductMaturityV2PlanScenario {
    MainChatProductMaturityV2PlanScenario {
        id: id.into(),
        capability_group: "plan_interaction".into(),
        prompt: prompt.into(),
        preconditions: preconditions.iter().map(|value| (*value).into()).collect(),
        expected_route: "plan_execute".into(),
        required_runtime_evidence: required_runtime_evidence
            .iter()
            .map(|value| (*value).into())
            .collect(),
        required_ui_state: required_ui_state
            .iter()
            .map(|value| (*value).into())
            .collect(),
        required_controls: required_controls
            .iter()
            .map(|value| (*value).into())
            .collect(),
        negative_assertions: negative_assertions
            .iter()
            .map(|value| (*value).into())
            .collect(),
        expected_outcome: expected_outcome.into(),
        default_gate: true,
    }
}
