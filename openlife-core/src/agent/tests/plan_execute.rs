use crate::agent::{
    AgentExecutionBudget, AgentTask, AgentTaskKind, GovernanceDecisionKind, HSSelectionAudit,
    LifeModelGovernor, PlanDraft, PlanExecuteInput, PlanExecuteProductContract,
    PlanExecuteProductScenario, PlanExecuteService, PlanExecuteSession, PlanExecuteSessionStatus,
    PlanExecuteSessionStore, PlanExecuteStepEdit, PlanStep, PlanStepStatus, ProposalStore,
    RiskLevel, RuntimeHSPacket, RuntimeInput,
};
use crate::layer::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;

fn runtime_input(user_text: &str) -> RuntimeInput {
    runtime_input_with_source_run(user_text, None)
}

fn runtime_input_with_source_run(user_text: &str, source_run_id: Option<&str>) -> RuntimeInput {
    RuntimeInput::from_agent_task(
        AgentTask {
            kind: AgentTaskKind::Planning,
            session_id: "session-plan-execute".into(),
            user_text: user_text.into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: user_text.into(),
            }],
            layer: Layer::L2,
        },
        LifeModel::default(),
        Some("memory context should not be copied into trace".into()),
        "Available tools: memory.search, calendar.create_event, file.update",
        source_run_id.map(test_hs_packet),
        AgentExecutionBudget::default(),
    )
}

fn test_hs_packet(source_run_id: &str) -> RuntimeHSPacket {
    RuntimeHSPacket {
        selected_policies: Vec::new(),
        selected_heuristics: Vec::new(),
        guidance_refs: Vec::new(),
        estimated_tokens: 8,
        audit: HSSelectionAudit {
            agent_task_id: Some("task-plan-execute".into()),
            agent_run_id: Some(source_run_id.into()),
            input_digest: "digest-input".into(),
            selected_policy_ids: Vec::new(),
            selected_heuristic_ids: Vec::new(),
            selected_guidance_ids: Vec::new(),
            selected_guidance_refs: Vec::new(),
            excluded_assets: Vec::new(),
            estimated_tokens: 8,
            token_budget: 128,
        },
        provider_authorization: crate::llm::ProviderPolicyAuthorization::local_only_fail_closed(
            crate::llm::ProviderLocalOnlyReason::TestFixture,
        ),
    }
}

fn plan_input(user_text: &str, max_steps: usize) -> PlanExecuteInput {
    PlanExecuteInput {
        runtime_input: runtime_input(user_text),
        objective: "metadata-safe objective".into(),
        max_steps,
    }
}

fn plan_input_with_source_run(
    user_text: &str,
    max_steps: usize,
    source_run_id: &str,
) -> PlanExecuteInput {
    PlanExecuteInput {
        runtime_input: runtime_input_with_source_run(user_text, Some(source_run_id)),
        objective: "metadata-safe objective".into(),
        max_steps,
    }
}

#[test]
fn draft_plan_creates_read_only_step_for_simple_question() {
    let service = PlanExecuteService::default();
    let plan = service.draft_plan(&plan_input("What should I focus on today?", 4));

    assert_eq!(plan.steps.len(), 1);
    let step = &plan.steps[0];
    assert_eq!(step.id, "step-1");
    assert_eq!(step.action_kind, "reason");
    assert_eq!(step.risk_level, RiskLevel::Low);
    assert!(!step.declared_write);
}

#[test]
fn draft_plan_marks_write_like_intent_without_executing() {
    let service = PlanExecuteService::default();
    let plan = service.draft_plan(&plan_input("Create a calendar event for tomorrow.", 4));

    assert_eq!(plan.steps.len(), 1);
    let step = &plan.steps[0];
    assert_eq!(step.action_kind, "create");
    assert_eq!(step.risk_level, RiskLevel::Medium);
    assert!(step.declared_write);
    assert_eq!(step.tool_name.as_deref(), Some("external.write_proposal"));
}

#[test]
fn plan_step_uses_governor_before_execution() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(plan_input("Update my calendar.", 4), &governor);

    assert_eq!(output.traces.len(), 1);
    assert_eq!(
        output.traces[0].decision.kind,
        GovernanceDecisionKind::RequireProposal
    );
    assert_eq!(output.traces[0].status, PlanStepStatus::RequiresProposal);
    assert!(output.runtime_outputs.is_empty());
}

#[test]
fn write_like_step_requires_proposal_and_is_not_executed() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(plan_input("Send an email to Alice.", 4), &governor);

    assert_eq!(output.traces.len(), 1);
    assert_eq!(output.traces[0].status, PlanStepStatus::RequiresProposal);
    assert_ne!(output.traces[0].status, PlanStepStatus::Executed);
    assert!(output.runtime_outputs.is_empty());
}

#[test]
fn read_only_step_is_allowed() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(
        plan_input("Search my notes for project context.", 4),
        &governor,
    );

    assert_eq!(output.traces.len(), 1);
    assert_eq!(
        output.traces[0].decision.kind,
        GovernanceDecisionKind::Allow
    );
    assert_eq!(output.traces[0].status, PlanStepStatus::Executed);
}

#[test]
fn plan_execute_v1_report_summarizes_governed_vertical_slice() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(
        plan_input_with_source_run(
            "Search notes for Alice and then update the calendar with alice@example.com.",
            4,
            "run-source-123",
        ),
        &governor,
    );

    assert!(output.report.plan_id.starts_with("plan-"));
    assert_eq!(
        output.report.source_run_id.as_deref(),
        Some("run-source-123")
    );
    assert_eq!(output.report.step_count, 2);
    assert_eq!(output.report.executed_read_only_step_count, 1);
    assert_eq!(output.report.blocked_or_proposal_required_step_count, 1);
    assert_eq!(output.report.governance_decisions.len(), 2);
    assert_eq!(output.report.observation_summaries.len(), 1);
    assert_eq!(
        output.report.metadata_safe_summary["reportKind"],
        "plan_execute_v1"
    );
}

#[test]
fn read_only_step_execution_produces_metadata_safe_observation_summary() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(
        plan_input(
            "Search memory for Alice's private note alice@example.com and raw draft text.",
            4,
        ),
        &governor,
    );

    assert_eq!(output.report.executed_read_only_step_count, 1);
    let observation = &output.report.observation_summaries[0];
    assert_eq!(observation.step_id, "step-1");
    assert_eq!(observation.source, "internal_read_only");
    assert!(observation.summary.contains("read-only"));
    assert!(!observation.summary.contains("Alice"));
    assert!(!observation.summary.contains("alice@example.com"));
    assert!(!observation.summary.contains("raw draft"));
}

#[test]
fn plan_execution_respects_max_steps() {
    let service = PlanExecuteService::default();
    let input = plan_input("Search my notes and then create a calendar event.", 1);

    let plan = service.draft_plan(&input);
    let output = service.execute_plan(input, &LifeModelGovernor::default());

    assert_eq!(plan.steps.len(), 1);
    assert_eq!(output.plan.steps.len(), 1);
    assert_eq!(output.traces.len(), 1);
}

#[test]
fn plan_trace_is_metadata_safe() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(
        plan_input(
            "Search for Alice's note, alice@example.com, and send her the full draft.",
            4,
        ),
        &governor,
    );

    let serialized_traces = serde_json::to_string(&output).unwrap();

    assert!(serialized_traces.contains("policyReasonCode"));
    assert!(!serialized_traces.contains("Alice"));
    assert!(!serialized_traces.contains("alice@example.com"));
    assert!(!serialized_traces.contains("full draft"));
    assert!(!serialized_traces.contains("memory context should not be copied"));
}

#[test]
fn plan_execute_does_not_write_lifemodel_memory_or_proposal_store() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let proposal_store = ProposalStore::new_in_memory().unwrap();

    let output = service.execute_plan(plan_input("Create a new reminder.", 4), &governor);

    assert!(output.runtime_outputs.is_empty());
    assert!(output.warnings.is_empty());
    assert!(proposal_store
        .list_pending_proposals(10)
        .unwrap()
        .is_empty());
}

#[test]
fn write_intents_for_sensitive_surfaces_are_proposal_required_without_direct_apply() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let proposal_store = ProposalStore::new_in_memory().unwrap();

    for user_text in [
        "Update my LifeModel with a new preference.",
        "Write this detail into memory.",
        "Update a file with this draft.",
        "Create a calendar event.",
        "Send an email with this draft.",
    ] {
        let output = service.execute_plan(plan_input(user_text, 4), &governor);

        assert_eq!(
            output.traces[0].status,
            PlanStepStatus::RequiresProposal,
            "{user_text}"
        );
        assert!(output.runtime_outputs.is_empty(), "{user_text}");
        assert!(
            output.report.observation_summaries.is_empty(),
            "{user_text}"
        );
    }

    assert!(proposal_store
        .list_pending_proposals(10)
        .unwrap()
        .is_empty());
}

#[test]
fn weekly_planning_product_contract_is_ready_for_clean_plan_draft() {
    let service = PlanExecuteService::default();
    let contract = PlanExecuteProductContract::weekly_planning();
    let draft = service.draft_product_plan(
        &plan_input(
            "Use my LifeModel to plan this week.",
            contract.max_step_count,
        ),
        PlanExecuteProductScenario::WeeklyPlanning,
    );

    let report = contract.evaluate_draft(&draft).unwrap();

    assert_eq!(
        contract.scenario,
        PlanExecuteProductScenario::WeeklyPlanning
    );
    assert!(draft.steps.len() >= 2);
    assert!(draft.steps.len() <= contract.max_step_count);
    assert!(report.ready);
    assert_eq!(report.scenario_id, "weekly_planning");
    assert_eq!(
        report.metadata_safe_summary["proposalFirstWriteBoundary"],
        true
    );
}

#[test]
fn weekly_planning_contract_rejects_unsupported_scenario_ids() {
    let err = PlanExecuteProductScenario::try_from_id("quarterly_planning").unwrap_err();

    assert_eq!(err.reason_code, "unsupported_scenario");
}

#[test]
fn weekly_planning_contract_rejects_excessive_step_count() {
    let contract = PlanExecuteProductContract::weekly_planning();
    let mut draft = PlanDraft {
        objective: "metadata-safe objective".into(),
        steps: Vec::new(),
    };
    for index in 0..=contract.max_step_count {
        draft.steps.push(PlanStep {
            id: format!("step-{}", index + 1),
            title: "Bounded weekly planning step".into(),
            intent: "read_only_reasoning".into(),
            tool_name: None,
            action_kind: "reason".into(),
            risk_level: RiskLevel::Low,
            declared_write: false,
        });
    }

    let report = contract.evaluate_draft(&draft).unwrap_err();

    assert_eq!(report.reason_code, "step_count_exceeds_contract");
}

#[test]
fn weekly_planning_contract_rejects_high_or_critical_direct_write_steps() {
    let contract = PlanExecuteProductContract::weekly_planning();
    for risk_level in [RiskLevel::High, RiskLevel::Critical] {
        let draft = PlanDraft {
            objective: "metadata-safe objective".into(),
            steps: vec![PlanStep {
                id: "step-unsafe".into(),
                title: "Unsafe write".into(),
                intent: "write_like_external_action".into(),
                tool_name: Some("external.write_proposal".into()),
                action_kind: "update".into(),
                risk_level,
                declared_write: true,
            }],
        };

        let report = contract.evaluate_draft(&draft).unwrap_err();

        assert_eq!(report.reason_code, "direct_write_risk_exceeds_contract");
    }
}

#[test]
fn broad_tools_prompt_does_not_grant_write_or_external_side_effect_authority() {
    let input = runtime_input_with_source_run(
        "Use my LifeModel to plan this week with Alice and alice@example.com.",
        Some("run-weekly-raw"),
    );
    let contract = PlanExecuteProductContract::weekly_planning();

    let report = contract.tools_authority_report(&input);
    let serialized = serde_json::to_string(&report).unwrap();

    assert_eq!(
        report.metadata_safe_summary["externalSideEffectsAllowed"],
        false
    );
    assert_eq!(report.metadata_safe_summary["directWritesAllowed"], false);
    assert!(!serialized.contains("Alice"));
    assert!(!serialized.contains("alice@example.com"));
    assert!(!serialized.contains("Available tools"));
    assert!(!serialized.contains("memory context should not be copied"));
}

#[test]
fn plan_execute_product_contract_debug_output_excludes_raw_content() {
    let input = runtime_input_with_source_run(
        "Plan my week around Alice's private note alice@example.com and raw draft text.",
        Some("run-weekly-raw"),
    );
    let contract = PlanExecuteProductContract::weekly_planning();

    let report = contract.metadata_safe_report(&input);
    let serialized = serde_json::to_string(&report).unwrap();

    assert!(serialized.contains("weekly_planning"));
    assert!(!serialized.contains("Alice"));
    assert!(!serialized.contains("alice@example.com"));
    assert!(!serialized.contains("raw draft"));
    assert!(!serialized.contains("memory context should not be copied"));
}

#[test]
fn plan_execute_session_store_creates_gets_and_lists_draft_sessions() {
    let store = PlanExecuteSessionStore::new_in_memory().unwrap();
    let service = PlanExecuteService::default();
    let contract = PlanExecuteProductContract::weekly_planning();
    let draft = service.draft_product_plan(
        &plan_input(
            "Use my LifeModel to plan this week.",
            contract.max_step_count,
        ),
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let session = PlanExecuteSession::new_draft(
        Some("chat-weekly".into()),
        Some("run-weekly".into()),
        contract,
        draft,
    )
    .unwrap();
    let session_id = session.session_id.clone();

    store.create_session(&session).unwrap();

    let fetched = store.get_session(&session_id).unwrap().unwrap();
    let sessions = store.list_sessions(10).unwrap();
    assert_eq!(fetched.status, PlanExecuteSessionStatus::Draft);
    assert_eq!(fetched.scenario, PlanExecuteProductScenario::WeeklyPlanning);
    assert_eq!(fetched.source_agent_run_id.as_deref(), Some("run-weekly"));
    assert_eq!(
        fetched.source_chat_session_id.as_deref(),
        Some("chat-weekly")
    );
    assert_eq!(sessions.len(), 1);
}

#[test]
fn draft_session_can_be_edited_and_finalized_but_not_after_finalization() {
    let service = PlanExecuteService::default();
    let contract = PlanExecuteProductContract::weekly_planning();
    let draft = service.draft_product_plan(
        &plan_input(
            "Use my LifeModel to plan this week.",
            contract.max_step_count,
        ),
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let mut session =
        PlanExecuteSession::new_draft(None, Some("run-weekly".into()), contract, draft).unwrap();
    let first_step_id = session.steps[0].step_id.clone();

    session
        .apply_draft_edits(vec![PlanExecuteStepEdit {
            step_id: first_step_id.clone(),
            title: Some("Review the week before choosing focus".into()),
            intent: Some("read_only_reasoning".into()),
            action_kind: Some("reason".into()),
            tool_name: None,
            declared_write: Some(false),
            risk_level: Some(RiskLevel::Low),
        }])
        .unwrap();
    session.finalize().unwrap();

    assert_eq!(session.status, PlanExecuteSessionStatus::Finalized);
    assert_eq!(
        session.steps[0].title,
        "Review the week before choosing focus"
    );
    assert!(session
        .apply_draft_edits(vec![PlanExecuteStepEdit {
            step_id: first_step_id,
            title: Some("Too late".into()),
            intent: None,
            action_kind: None,
            tool_name: None,
            declared_write: None,
            risk_level: None,
        }])
        .is_err());
}

#[test]
fn finalized_session_executes_read_only_steps_and_creates_proposals_for_write_like_steps() {
    let service = PlanExecuteService::default();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let contract = PlanExecuteProductContract::weekly_planning();
    let draft = service.draft_product_plan(
        &plan_input(
            "Use my LifeModel to plan this week.",
            contract.max_step_count,
        ),
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let mut session =
        PlanExecuteSession::new_draft(None, Some("run-weekly".into()), contract, draft).unwrap();
    session.finalize().unwrap();
    let read_step_id = session
        .steps
        .iter()
        .find(|step| !step.declared_write)
        .unwrap()
        .step_id
        .clone();
    let write_step_id = session
        .steps
        .iter()
        .find(|step| step.declared_write)
        .unwrap()
        .step_id
        .clone();

    let read_result = session
        .execute_step(
            &read_step_id,
            &LifeModelGovernor::default(),
            &proposal_store,
        )
        .unwrap();
    let write_result = session
        .execute_step(
            &write_step_id,
            &LifeModelGovernor::default(),
            &proposal_store,
        )
        .unwrap();
    let duplicate_write_result = session
        .execute_step(
            &write_step_id,
            &LifeModelGovernor::default(),
            &proposal_store,
        )
        .unwrap();

    assert_eq!(read_result.step_status, PlanStepStatus::Executed);
    assert!(read_result.linked_proposal_id.is_none());
    assert_eq!(write_result.step_status, PlanStepStatus::RequiresProposal);
    assert!(write_result.linked_proposal_id.is_some());
    assert_eq!(
        duplicate_write_result.linked_proposal_id,
        write_result.linked_proposal_id
    );
    assert_eq!(proposal_store.list_pending_proposals(10).unwrap().len(), 1);
    assert_eq!(session.linked_proposal_ids.len(), 1);
}

#[test]
fn phase4_plan_execute_returns_reused_review_workflow_outcome_id() {
    let service = PlanExecuteService::default();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let fixed_session_id = "phase4-plan-outcome-session";

    let build_finalized_session = |source_run_id: &str| {
        let contract = PlanExecuteProductContract::weekly_planning();
        let draft = service.draft_product_plan(
            &plan_input(
                "Use my LifeModel to plan this week.",
                contract.max_step_count,
            ),
            PlanExecuteProductScenario::WeeklyPlanning,
        );
        let mut session =
            PlanExecuteSession::new_draft(None, Some(source_run_id.into()), contract, draft)
                .unwrap();
        session.session_id = fixed_session_id.into();
        session.plan_id = format!("plan:{fixed_session_id}");
        for step in &mut session.steps {
            step.plan_id = session.plan_id.clone();
        }
        session.finalize().unwrap();
        session
    };

    let mut first_session = build_finalized_session("phase4-plan-run-a");
    let write_step_id = first_session
        .steps
        .iter()
        .find(|step| step.declared_write)
        .unwrap()
        .step_id
        .clone();
    let first_result = first_session
        .execute_step(
            &write_step_id,
            &LifeModelGovernor::default(),
            &proposal_store,
        )
        .unwrap();
    let reused_id = first_result
        .linked_proposal_id
        .clone()
        .expect("first write step proposal id");

    let mut second_session = build_finalized_session("phase4-plan-run-b");
    let second_result = second_session
        .execute_step(
            &write_step_id,
            &LifeModelGovernor::default(),
            &proposal_store,
        )
        .unwrap();

    assert_eq!(second_result.linked_proposal_id, Some(reused_id.clone()));
    assert_eq!(second_session.linked_proposal_ids, vec![reused_id.clone()]);
    assert!(
        second_result.evidence_ids.contains(&reused_id),
        "PlanExecute evidence must use the authoritative ReviewWorkflowOutcome id"
    );
    assert_eq!(proposal_store.list_pending_proposals(10).unwrap().len(), 1);
}

#[test]
fn phase_c_write_like_steps_are_proposal_first_without_direct_writes() {
    let service = PlanExecuteService::default();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let contract = PlanExecuteProductContract::weekly_planning();
    let draft = service.draft_product_plan(
        &plan_input(
            "Use my LifeModel to plan this week.",
            contract.max_step_count,
        ),
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let mut session =
        PlanExecuteSession::new_draft(None, Some("run-weekly".into()), contract, draft).unwrap();
    session.finalize().unwrap();
    let write_step_id = session
        .steps
        .iter()
        .find(|step| step.declared_write)
        .unwrap()
        .step_id
        .clone();

    let write_result = session
        .execute_step(
            &write_step_id,
            &LifeModelGovernor::default(),
            &proposal_store,
        )
        .unwrap();

    assert_eq!(write_result.step_status, PlanStepStatus::RequiresProposal);
    assert!(write_result.linked_action_ids.is_empty());
    assert!(write_result.linked_observation_ids.is_empty());
    assert!(!write_result.linked_proposal_ids.is_empty());
    assert_eq!(
        write_result.status_reason.as_deref(),
        Some("proposal_first_write_boundary")
    );
    assert_eq!(
        write_result.metadata_safe_summary["directLifeModelWrites"],
        false
    );
    assert_eq!(write_result.metadata_safe_summary["memoryWrites"], false);
    assert_eq!(
        write_result.metadata_safe_summary["externalWritesExecuted"],
        false
    );
    let proposals = proposal_store.list_pending_proposals(10).unwrap();
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].after["externalWriteExecuted"], false);
    assert_eq!(
        session.linked_proposal_ids,
        write_result.linked_proposal_ids
    );
}

#[test]
fn phase_c_cancel_marks_remaining_steps_cancelled_with_evidence() {
    let service = PlanExecuteService::default();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let contract = PlanExecuteProductContract::weekly_planning();
    let draft = service.draft_product_plan(
        &plan_input(
            "Use my LifeModel to plan this week.",
            contract.max_step_count,
        ),
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let mut session =
        PlanExecuteSession::new_draft(None, Some("run-weekly".into()), contract, draft).unwrap();
    session.finalize().unwrap();
    let read_step_id = session
        .steps
        .iter()
        .find(|step| !step.declared_write)
        .unwrap()
        .step_id
        .clone();
    session
        .execute_step(
            &read_step_id,
            &LifeModelGovernor::default(),
            &proposal_store,
        )
        .unwrap();
    let cancel_revision = session.revision;

    let cancel_result = session.cancel_at_revision(cancel_revision).unwrap();

    assert_eq!(session.status, PlanExecuteSessionStatus::Cancelled);
    assert_eq!(cancel_result.base_plan_revision, cancel_revision);
    assert!(!cancel_result.cancelled_step_ids.is_empty());
    for step in session
        .steps
        .iter()
        .filter(|step| cancel_result.cancelled_step_ids.contains(&step.step_id))
    {
        assert_eq!(step.status, PlanStepStatus::Cancelled);
        assert_eq!(step.status_reason.as_deref(), Some("cancelled_by_user"));
        assert!(step
            .evidence_ids
            .iter()
            .any(|id| id.contains("plan-step-cancel")));
    }
    assert!(session.steps.iter().all(|step| {
        matches!(
            step.status,
            PlanStepStatus::Executed
                | PlanStepStatus::RequiresProposal
                | PlanStepStatus::Blocked
                | PlanStepStatus::Skipped
                | PlanStepStatus::Cancelled
        )
    }));
}

#[test]
fn phase_c_review_summary_separates_evidence_backed_outcomes() {
    let service = PlanExecuteService::default();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let contract = PlanExecuteProductContract::weekly_planning();
    let draft = service.draft_product_plan(
        &plan_input(
            "Use my LifeModel to plan this week.",
            contract.max_step_count,
        ),
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let mut session =
        PlanExecuteSession::new_draft(None, Some("run-weekly".into()), contract, draft).unwrap();
    session.finalize().unwrap();
    let read_step_id = session
        .steps
        .iter()
        .find(|step| !step.declared_write)
        .unwrap()
        .step_id
        .clone();
    session
        .execute_step(
            &read_step_id,
            &LifeModelGovernor::default(),
            &proposal_store,
        )
        .unwrap();
    let write_step_id = session
        .steps
        .iter()
        .find(|step| step.declared_write)
        .unwrap()
        .step_id
        .clone();
    session
        .execute_step(
            &write_step_id,
            &LifeModelGovernor::default(),
            &proposal_store,
        )
        .unwrap();
    let remaining_step_id = session
        .steps
        .iter()
        .find(|step| step.status == PlanStepStatus::Planned)
        .unwrap()
        .step_id
        .clone();
    let skip_revision = session.revision;
    session
        .skip_step_at_revision(
            &remaining_step_id,
            skip_revision,
            "not needed for this review",
        )
        .unwrap();
    let review_revision = session.revision;

    let summary = session.review_at_revision(review_revision).unwrap();

    assert_eq!(summary.base_plan_revision, review_revision);
    assert_eq!(
        session.review_id.as_deref(),
        Some(summary.review_id.as_str())
    );
    assert_eq!(session.review_summary.as_ref(), Some(&summary));
    assert!(!summary.completed_steps.is_empty());
    assert!(!summary.skipped_steps.is_empty());
    assert!(summary.blocked_steps.is_empty());
    assert!(!summary.proposals_created.is_empty());
    assert!(!summary.observations_used.is_empty());
    assert!(summary.unresolved.is_empty());
    assert!(!summary.recommended_next_action.is_empty());
    assert!(summary.completed_steps.iter().all(|item| {
        !item.linked_action_ids.is_empty() || !item.linked_observation_ids.is_empty()
    }));
    assert!(summary
        .proposals_created
        .iter()
        .all(|item| { !item.linked_proposal_ids.is_empty() && item.linked_action_ids.is_empty() }));
}

#[test]
fn phase_c_plan_session_tracks_revisions_step_links_and_skip_evidence() {
    let service = PlanExecuteService::default();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let contract = PlanExecuteProductContract::weekly_planning();
    let draft = service.draft_product_plan(
        &plan_input(
            "Use my LifeModel to plan this week.",
            contract.max_step_count,
        ),
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let mut session =
        PlanExecuteSession::new_draft(None, Some("run-weekly".into()), contract, draft).unwrap();
    let plan_id = session.plan_id.clone();
    let initial_revision = session.revision;
    let first_step_id = session.steps[0].step_id.clone();
    let second_step_id = session.steps[1].step_id.clone();

    assert!(!plan_id.is_empty());
    assert_eq!(session.revision_id, "rev-1");
    assert_eq!(session.steps[0].plan_id, plan_id);
    assert_eq!(session.steps[0].base_plan_revision, initial_revision);
    assert_eq!(session.steps[0].linked_action_ids.len(), 0);

    let stale_edit = session.apply_draft_edits_at_revision(
        initial_revision + 1,
        vec![PlanExecuteStepEdit {
            step_id: first_step_id.clone(),
            title: Some("Stale edit".into()),
            intent: None,
            action_kind: None,
            tool_name: None,
            declared_write: None,
            risk_level: None,
        }],
    );
    assert!(stale_edit.unwrap_err().to_string().contains("stale"));

    session
        .apply_draft_edits_at_revision(
            initial_revision,
            vec![PlanExecuteStepEdit {
                step_id: first_step_id.clone(),
                title: Some("Review priorities with explicit evidence".into()),
                intent: Some("read_only_reasoning".into()),
                action_kind: Some("reason".into()),
                tool_name: None,
                declared_write: Some(false),
                risk_level: Some(RiskLevel::Low),
            }],
        )
        .unwrap();
    assert!(session.revision > initial_revision);
    assert_eq!(session.steps[0].revision, session.revision);
    assert_eq!(
        session.steps[0].status_reason.as_deref(),
        Some("edited_by_user")
    );

    let confirmed_revision = session.revision;
    session.finalize_at_revision(confirmed_revision).unwrap();
    assert!(session.confirmed_at.is_some());

    let stale_execute = session.execute_step_at_revision(
        &first_step_id,
        confirmed_revision - 1,
        &LifeModelGovernor::default(),
        &proposal_store,
    );
    assert!(stale_execute.unwrap_err().to_string().contains("stale"));

    let executed = session
        .execute_step_at_revision(
            &first_step_id,
            confirmed_revision,
            &LifeModelGovernor::default(),
            &proposal_store,
        )
        .unwrap();
    assert_eq!(executed.step_status, PlanStepStatus::Executed);
    assert!(!executed.linked_action_ids.is_empty());
    assert!(!executed.linked_observation_ids.is_empty());
    assert!(executed.linked_proposal_ids.is_empty());

    let skip_revision = session.revision;
    let skipped = session
        .skip_step_at_revision(
            &second_step_id,
            skip_revision,
            "unsupported in this phase; user requested skip",
        )
        .unwrap();
    assert_eq!(skipped.step_status, PlanStepStatus::Skipped);
    assert_eq!(
        skipped.skip_reason.as_deref(),
        Some("unsupported in this phase; user requested skip")
    );
    let skipped_step = session
        .steps
        .iter()
        .find(|step| step.step_id == second_step_id)
        .unwrap();
    assert_eq!(skipped_step.status, PlanStepStatus::Skipped);
    assert_eq!(
        skipped_step.skip_reason.as_deref(),
        Some("unsupported in this phase; user requested skip")
    );
    assert!(skipped_step
        .evidence_ids
        .iter()
        .any(|id| id.contains("skip")));
}
