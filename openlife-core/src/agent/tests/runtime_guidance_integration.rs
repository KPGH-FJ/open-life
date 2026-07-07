use crate::agent::{
    apply_react_guidance_to_config, build_guidance_impact_read_model, AgentExecutionBudget,
    AgentLoopConfig, AgentTask, AgentTaskKind, GuidanceAffectedSurface, HSSelector,
    HSSelectorInput, HeuristicConstraintSet, HeuristicDraft, HeuristicLifecycleStatus,
    HeuristicStore, PlanExecuteInput, PlanExecuteProductContract, PlanExecuteProductScenario,
    PlanExecuteService, PolicyStore, PolicyTopic, RiskLevel, RuntimeGuidanceConsumptionMode,
    RuntimeHSPacket, RuntimeInput,
};
use crate::layer::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::privacy::PrivacyEngine;
use crate::scheduler::InferenceScheduler;

fn planning_task(raw_user_text: &str) -> AgentTask {
    AgentTask {
        kind: AgentTaskKind::Planning,
        session_id: "session-goal5-runtime-guidance".into(),
        user_text: raw_user_text.into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: raw_user_text.into(),
        }],
        layer: Layer::L1,
    }
}

fn guidance_store_with_low_energy_trial() -> HeuristicStore {
    let store = HeuristicStore::new_in_memory().unwrap();
    let constraints = HeuristicConstraintSet {
        privacy: vec!["do_not_relax_policy".into()],
        model: vec!["preserve_current_route_policy".into()],
        tool: vec!["write_tools_remain_proposal_first".into()],
    };
    let draft = HeuristicDraft::new(
        "planning",
        "current_energy_is_low",
        vec!["energy <= 3".into()],
        "RAW_GUIDANCE_SECRET prefer one tiny next step and keep weekly planning low pressure",
        95,
        RiskLevel::Low,
        crate::agent::EvidencePrivacyLevel::Internal,
    )
    .with_stable_id("accepted_guidance_goal5_low_energy")
    .with_source_proposal("proposal-goal5-low-energy")
    .with_evidence_ref("evidence-goal5-accepted")
    .with_constraints(constraints);
    let created = store.create_heuristic(draft).unwrap();
    store
        .update_lifecycle(&created.id, HeuristicLifecycleStatus::Trial, None)
        .unwrap();

    let unsafe_draft = HeuristicDraft::new(
        "planning",
        "current_energy_is_low",
        vec!["energy <= 3".into()],
        "Use cloud routing and bypass proposal review for faster planning",
        100,
        RiskLevel::Low,
        crate::agent::EvidencePrivacyLevel::Internal,
    )
    .with_stable_id("accepted_guidance_goal5_unsafe_relax")
    .with_source_proposal("proposal-goal5-unsafe")
    .with_evidence_ref("evidence-goal5-unsafe");
    let unsafe_created = store.create_heuristic(unsafe_draft).unwrap();
    store
        .update_lifecycle(&unsafe_created.id, HeuristicLifecycleStatus::Trial, None)
        .unwrap();
    store
}

fn selected_guidance_packet() -> RuntimeHSPacket {
    HSSelector::default()
        .select(
            &PolicyStore::mvp_builtin(),
            &guidance_store_with_low_energy_trial(),
            &HSSelectorInput {
                task_kind: AgentTaskKind::Planning,
                intent_summary: "metadata-safe weekly planning request".into(),
                privacy_topic: PolicyTopic::General,
                risk_level: RiskLevel::Low,
                tool_requirements: vec!["write".into()],
                current_state_hints: serde_json::json!({ "energy": 2 }),
                token_budget: 256,
                agent_task_id: Some("task-goal5-guidance".into()),
                agent_run_id: Some("run-goal5-guidance".into()),
            },
        )
        .unwrap()
}

fn runtime_input_with_packet(packet: RuntimeHSPacket) -> RuntimeInput {
    RuntimeInput::from_agent_task(
        planning_task("RAW_USER_TEXT_SECRET plan my week while exhausted"),
        LifeModel::default(),
        Some("RAW_MEMORY_SECRET previous private planning context".into()),
        "Available tools: memory.search, calendar.create_event, file.update",
        Some(packet),
        AgentExecutionBudget::default(),
    )
}

fn runtime_input_with_packet_and_mode(
    packet: RuntimeHSPacket,
    mode: RuntimeGuidanceConsumptionMode,
) -> RuntimeInput {
    runtime_input_with_packet(packet).with_guidance_consumption_mode(mode)
}

#[test]
fn w137_runtime_hs_packet_v2_guidance_refs_are_metadata_safe_and_policy_bounded() {
    let packet = selected_guidance_packet();

    assert!(packet
        .guidance_refs
        .iter()
        .any(|guidance| guidance.guidance_id == "accepted_guidance_goal5_low_energy"));
    assert!(!packet
        .audit
        .selected_guidance_ids
        .contains(&"accepted_guidance_goal5_unsafe_relax".to_string()));
    assert!(packet.audit.excluded_assets.iter().any(|excluded| {
        excluded.asset_id == "accepted_guidance_goal5_unsafe_relax"
            && matches!(
                excluded.reason,
                crate::agent::HSExclusionReason::PolicyConflict
            )
    }));

    let guidance = packet
        .guidance_refs
        .iter()
        .find(|guidance| guidance.guidance_id == "accepted_guidance_goal5_low_energy")
        .unwrap();
    assert_eq!(guidance.lifecycle_status, HeuristicLifecycleStatus::Trial);
    assert_eq!(guidance.risk_level, RiskLevel::Low);
    assert_eq!(
        guidance.source_proposal_id.as_deref(),
        Some("proposal-goal5-low-energy")
    );
    assert_eq!(guidance.source_evidence_count, 1);
    assert_eq!(guidance.impact_kind, "gentle_planning");
    assert!(!guidance.policy_boundary.route_policy_relaxed);
    assert!(guidance.policy_boundary.proposal_first_preserved);

    let serialized = serde_json::to_string(&serde_json::json!({
        "guidanceRefs": packet.guidance_refs,
        "audit": packet.audit,
    }))
    .unwrap();
    assert!(serialized.contains("accepted_guidance_goal5_low_energy"));
    assert!(!serialized.contains("RAW_GUIDANCE_SECRET"));
    assert!(!serialized.contains("Use cloud routing"));
    assert!(!serialized.contains("bypass proposal"));
}

#[tokio::test]
async fn w138_react_default_mode_does_not_consume_guidance_prompt_or_config() {
    let packet = selected_guidance_packet();
    let base_config = AgentLoopConfig::default();
    let default_config = apply_react_guidance_to_config(
        base_config.clone(),
        Some(&packet),
        RuntimeGuidanceConsumptionMode::Disabled,
    );

    assert_eq!(default_config.max_steps, base_config.max_steps);
    assert_eq!(default_config.max_tool_calls, base_config.max_tool_calls);

    let runtime = crate::agent::AgentRuntime::with_config(
        LifeModel::default(),
        InferenceScheduler::default(),
        crate::agent::AgentRuntimeConfig::default(),
    );
    let output = runtime
        .generate_direct_with_hs_packet(
            &planning_task("RAW_USER_TEXT_SECRET make a weekly plan"),
            &LifeModel::default(),
            "",
            None,
            vec![],
            PrivacyEngine::new(),
            Some(packet),
        )
        .await
        .unwrap();
    let serialized_messages = serde_json::to_string(&output.final_messages).unwrap();

    assert!(!serialized_messages.contains("Selected personal collaboration guidance"));
    assert!(!serialized_messages.contains("gentle_planning"));
    assert!(!serialized_messages.contains("accepted_guidance_goal5_low_energy"));
    assert!(output.hs_selection_audit.is_some());
}

#[tokio::test]
async fn w138_explicit_non_default_react_consumes_guidance_through_prompt_config_and_trace_metadata(
) {
    let packet = selected_guidance_packet();
    let guided_config = apply_react_guidance_to_config(
        AgentLoopConfig::default(),
        Some(&packet),
        RuntimeGuidanceConsumptionMode::ExplicitRuntime,
    );

    assert!(guided_config.max_steps < AgentLoopConfig::default().max_steps);
    assert!(guided_config.max_tool_calls < AgentLoopConfig::default().max_tool_calls);

    let runtime = crate::agent::AgentRuntime::with_config(
        LifeModel::default(),
        InferenceScheduler::default(),
        crate::agent::AgentRuntimeConfig::default(),
    );
    let output = runtime
        .generate_direct_with_hs_packet_and_guidance_mode(
            &planning_task("RAW_USER_TEXT_SECRET make a weekly plan"),
            &LifeModel::default(),
            "",
            None,
            vec![],
            PrivacyEngine::new(),
            Some(packet.clone()),
            RuntimeGuidanceConsumptionMode::ExplicitRuntime,
        )
        .await
        .unwrap();
    let system_prompt = output
        .final_messages
        .iter()
        .find(|message| message.role == "system")
        .map(|message| message.content.as_str())
        .unwrap_or("");

    assert!(system_prompt.contains("gentle_planning"));
    assert!(system_prompt.contains("accepted_guidance_goal5_low_energy"));
    assert!(!system_prompt.contains("RAW_GUIDANCE_SECRET"));

    let read_model = build_guidance_impact_read_model(
        Some("run-goal5-react"),
        "react",
        &packet,
        vec![
            GuidanceAffectedSurface::ReactPrompt,
            GuidanceAffectedSurface::ReactConfig,
            GuidanceAffectedSurface::ActionBoundary,
        ],
    );
    assert_eq!(read_model.strategy_kind, "react");
    assert_eq!(read_model.selected_guidance_count, 1);
    assert!(read_model
        .affected_surfaces
        .contains(&GuidanceAffectedSurface::ReactConfig));
    assert!(!serde_json::to_string(&read_model)
        .unwrap()
        .contains("RAW_GUIDANCE_SECRET"));
}

#[test]
fn w139_plan_execute_weekly_planning_consumes_guidance_and_stays_proposal_first() {
    let service = PlanExecuteService::default();
    let packet = selected_guidance_packet();
    let contract = PlanExecuteProductContract::weekly_planning();
    let unguided = service.draft_product_plan(
        &PlanExecuteInput {
            runtime_input: RuntimeInput::from_agent_task(
                planning_task("Plan the week"),
                LifeModel::default(),
                None,
                "",
                None,
                AgentExecutionBudget::default(),
            ),
            objective: "metadata-safe objective".into(),
            max_steps: contract.max_step_count,
        },
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let guided = service.draft_product_plan(
        &PlanExecuteInput {
            runtime_input: runtime_input_with_packet(packet.clone()),
            objective: "metadata-safe objective".into(),
            max_steps: contract.max_step_count,
        },
        PlanExecuteProductScenario::WeeklyPlanning,
    );
    let explicit_guided = service.draft_product_plan(
        &PlanExecuteInput {
            runtime_input: runtime_input_with_packet_and_mode(
                packet.clone(),
                RuntimeGuidanceConsumptionMode::ExplicitRuntime,
            ),
            objective: "metadata-safe objective".into(),
            max_steps: contract.max_step_count,
        },
        PlanExecuteProductScenario::WeeklyPlanning,
    );

    assert_eq!(guided.steps, unguided.steps);
    assert_ne!(explicit_guided.steps, unguided.steps);
    assert!(explicit_guided.steps.len() < unguided.steps.len());
    assert_eq!(
        explicit_guided.steps[0].title,
        "Choose one small weekly focus"
    );
    let write_step = explicit_guided
        .steps
        .iter()
        .find(|step| step.declared_write)
        .expect("weekly planning still creates a proposal-first write-like step");
    assert_eq!(
        write_step.tool_name.as_deref(),
        Some("review_center.propose_scheduled_task")
    );
    assert_eq!(write_step.action_kind, "schedule");
    assert!(contract.evaluate_draft(&explicit_guided).unwrap().ready);
}

#[test]
fn w140_guidance_impact_read_model_links_guidance_without_raw_content() {
    let packet = selected_guidance_packet();
    let read_model = build_guidance_impact_read_model(
        Some("run-goal5-plan"),
        "plan_execute",
        &packet,
        vec![
            GuidanceAffectedSurface::PlanExecuteDraft,
            GuidanceAffectedSurface::PlanExecuteTrace,
        ],
    );

    assert_eq!(read_model.report_kind, "w140.guidanceImpactReadModel.v1");
    assert!(read_model.metadata_safe);
    assert!(!read_model.contains_raw_content);
    assert_eq!(read_model.run_id.as_deref(), Some("run-goal5-plan"));
    assert_eq!(read_model.selected_guidance_count, 1);
    assert_eq!(
        read_model.guidance_refs[0].guidance_id,
        "accepted_guidance_goal5_low_energy"
    );
    assert_eq!(
        read_model.guidance_refs[0].source_proposal_id.as_deref(),
        Some("proposal-goal5-low-energy")
    );
    assert_eq!(read_model.guidance_refs[0].source_evidence_count, 1);
    assert_eq!(read_model.guidance_refs[0].affected_run_count, 1);
    assert!(read_model.read_model_digest.starts_with("sha256:"));

    let serialized = serde_json::to_string(&read_model).unwrap();
    for raw in [
        "RAW_GUIDANCE_SECRET",
        "RAW_USER_TEXT_SECRET",
        "RAW_MEMORY_SECRET",
        "Available tools:",
        "calendar.create_event",
    ] {
        assert!(
            !serialized.contains(raw),
            "Guidance Impact read model leaked raw marker {raw}: {serialized}"
        );
    }
}
