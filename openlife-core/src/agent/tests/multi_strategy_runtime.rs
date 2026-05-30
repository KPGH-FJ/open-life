use crate::agent::policy_store::{ModelRoutePolicy, BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY};
use crate::agent::{
    AgentExecutionBudget, AgentTask, AgentTaskKind, EvidenceQuery, EvidenceStore, HSSelectionAudit,
    HeuristicQuery, HeuristicStore, ModelRouter, MultiStrategyRuntime, MultiStrategyRuntimeInput,
    MultiStrategyRuntimePayload, PlanStepStatus, ProposalStore, ProviderAvailability,
    RuntimeHSPacket, RuntimeInput, RuntimeStrategyKind, SelectedPolicyRef, StrategySelectionInput,
    StrategySelector,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::memory::MemoryStore;
use crate::scheduler::InferenceScheduler;

fn no_network_scheduler() -> InferenceScheduler {
    let mut router = ModelRouter::new();
    router.providers.insert(
        "deepseek".into(),
        ProviderAvailability {
            provider: "deepseek".into(),
            available: true,
            latency_ms: Some(50),
            models: vec!["deepseek-chat".into()],
            last_checked: chrono::Utc::now(),
            last_error: None,
            health_is_estimated: false,
        },
    );

    InferenceScheduler {
        prefer_local: false,
        provider: "contract-test".into(),
        openai_key: String::new(),
        model_router: Some(router),
        ..InferenceScheduler::default()
    }
}

fn test_runtime() -> MultiStrategyRuntime {
    let runtime = crate::agent::AgentRuntime::with_config(
        LifeModel::default(),
        no_network_scheduler(),
        crate::agent::AgentRuntimeConfig::default(),
    );
    MultiStrategyRuntime::new(runtime)
}

fn runtime_input(user_text: &str, tools_prompt: &str) -> RuntimeInput {
    runtime_input_with_life_model(user_text, tools_prompt, LifeModel::default(), None)
}

fn runtime_input_with_life_model(
    user_text: &str,
    tools_prompt: &str,
    life_model: LifeModel,
    hs_packet: Option<RuntimeHSPacket>,
) -> RuntimeInput {
    RuntimeInput::from_agent_task(
        AgentTask {
            kind: AgentTaskKind::Conversation,
            session_id: "session-multi-strategy".into(),
            user_text: user_text.into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: user_text.into(),
            }],
            layer: Layer::L2,
        },
        life_model,
        Some("memory context must stay out of runtime metadata".into()),
        tools_prompt,
        hs_packet,
        AgentExecutionBudget::default(),
    )
}

fn multi_input(
    runtime_input: RuntimeInput,
    allow_planning: bool,
    local_model_available: bool,
) -> MultiStrategyRuntimeInput {
    MultiStrategyRuntimeInput {
        runtime_input,
        allow_planning,
        local_model_available,
    }
}

fn sensitive_packet() -> RuntimeHSPacket {
    RuntimeHSPacket {
        selected_policies: vec![SelectedPolicyRef {
            policy_id: BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
            reason: "sensitive_topic_route".into(),
            route: Some(ModelRoutePolicy::LocalOnly),
            digest: "digest-sensitive".into(),
        }],
        selected_heuristics: Vec::new(),
        estimated_tokens: 12,
        audit: HSSelectionAudit {
            agent_task_id: Some("task-multi-strategy".into()),
            agent_run_id: Some("run-multi-strategy".into()),
            input_digest: "digest-input".into(),
            selected_policy_ids: vec![BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into()],
            selected_heuristic_ids: Vec::new(),
            excluded_assets: Vec::new(),
            estimated_tokens: 12,
            token_budget: 128,
        },
    }
}

#[tokio::test]
async fn simple_chat_orchestrates_react_path() {
    let output = test_runtime()
        .execute(multi_input(
            runtime_input(
                "What should I focus on today?",
                "Available tools: memory.search",
            ),
            true,
            true,
        ))
        .await
        .unwrap();

    assert_eq!(output.selection.kind, RuntimeStrategyKind::ReAct);
    match output.payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output) => {
            assert!(runtime_output.run_id.is_some());
            assert!(!runtime_output.user_output.trim().is_empty());
            assert!(runtime_output.actions.is_empty());
            assert!(runtime_output.observations.is_empty());
            assert!(runtime_output.proposal_ids.is_empty());
            assert!(runtime_output.life_event_candidates.is_empty());
        }
        other => panic!("expected ReAct payload, got {other:?}"),
    }
}

#[tokio::test]
async fn planning_intent_orchestrates_plan_execute_path() {
    let output = test_runtime()
        .execute(multi_input(
            runtime_input("Plan steps for my afternoon.", ""),
            true,
            true,
        ))
        .await
        .unwrap();

    assert_eq!(output.selection.kind, RuntimeStrategyKind::PlanExecute);
    match output.payload {
        MultiStrategyRuntimePayload::PlanExecute(plan_output) => {
            assert_eq!(plan_output.plan.steps.len(), 1);
            assert_eq!(plan_output.traces.len(), 1);
            assert_eq!(plan_output.traces[0].status, PlanStepStatus::Executed);
            assert_eq!(plan_output.report.step_count, 1);
            assert_eq!(plan_output.report.executed_read_only_step_count, 1);
            assert_eq!(
                plan_output.report.metadata_safe_summary["reportKind"],
                "plan_execute_v1"
            );
            assert!(plan_output.runtime_outputs.is_empty());
        }
        other => panic!("expected PlanExecute payload, got {other:?}"),
    }
}

#[tokio::test]
async fn write_like_plan_execute_does_not_execute_external_write() {
    let output = test_runtime()
        .execute(multi_input(
            runtime_input("Send Alice the draft.", ""),
            true,
            true,
        ))
        .await
        .unwrap();

    assert_eq!(output.selection.kind, RuntimeStrategyKind::PlanExecute);
    match output.payload {
        MultiStrategyRuntimePayload::PlanExecute(plan_output) => {
            assert_eq!(plan_output.traces.len(), 1);
            assert_eq!(
                plan_output.traces[0].status,
                PlanStepStatus::RequiresProposal
            );
            assert!(plan_output.runtime_outputs.is_empty());
        }
        other => panic!("expected PlanExecute payload, got {other:?}"),
    }
}

#[tokio::test]
async fn blocked_local_only_selection_does_not_execute_any_runtime() {
    let input = runtime_input_with_life_model(
        "Talk through a sensitive health topic.",
        "Available tools: memory.search",
        LifeModel::default(),
        Some(sensitive_packet()),
    );

    let output = test_runtime()
        .execute(multi_input(input, true, false))
        .await
        .unwrap();

    assert_eq!(output.selection.kind, RuntimeStrategyKind::ReAct);
    assert_eq!(
        output
            .selection
            .metadata_safe_summary
            .get("governanceDecisionKind")
            .and_then(|value| value.as_str()),
        Some("block")
    );
    assert!(matches!(
        output.payload,
        MultiStrategyRuntimePayload::Blocked
    ));
}

#[tokio::test]
async fn orchestrator_preserves_strategy_selection_summary() {
    let input = runtime_input("Plan steps for a quiet work block.", "");
    let expected = StrategySelector::default().select(StrategySelectionInput {
        runtime_input: input.clone(),
        allow_planning: true,
        local_model_available: true,
    });

    let output = test_runtime()
        .execute(multi_input(input, true, true))
        .await
        .unwrap();

    assert_eq!(output.selection.kind, expected.kind);
    assert_eq!(
        output.selection.metadata_safe_summary,
        expected.metadata_safe_summary
    );
}

#[tokio::test]
async fn orchestrator_does_not_write_lifemodel_memory_or_proposal_store() {
    let mut life_model = LifeModel::default();
    life_model.metadata.version = "before-orchestrator".into();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let memory_store = MemoryStore::new_in_memory().unwrap();
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let heuristic_store = HeuristicStore::new_in_memory().unwrap();

    let output = test_runtime()
        .execute(multi_input(
            runtime_input_with_life_model(
                "Create a reminder for tomorrow.",
                "",
                life_model.clone(),
                None,
            ),
            true,
            true,
        ))
        .await
        .unwrap();

    assert!(matches!(
        output.payload,
        MultiStrategyRuntimePayload::PlanExecute(_)
    ));
    assert_eq!(life_model.metadata.version, "before-orchestrator");
    assert!(proposal_store
        .list_pending_proposals(10)
        .unwrap()
        .is_empty());
    assert!(memory_store.export_all_messages().unwrap().is_empty());
    assert!(evidence_store
        .query(EvidenceQuery::default())
        .unwrap()
        .is_empty());
    assert!(heuristic_store
        .query(HeuristicQuery::default())
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn broad_tools_prompt_does_not_force_plan_execute_or_write() {
    let output = test_runtime()
        .execute(multi_input(
            runtime_input(
                "What should I focus on today?",
                "Available tools: file.write, calendar.create_event, email.send",
            ),
            true,
            true,
        ))
        .await
        .unwrap();

    assert_eq!(output.selection.kind, RuntimeStrategyKind::ReAct);
    match output.payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output) => {
            assert!(runtime_output.proposal_ids.is_empty());
            assert!(runtime_output.actions.is_empty());
        }
        other => panic!("expected ReAct payload, got {other:?}"),
    }
    let serialized_summary = output.selection.metadata_safe_summary.to_string();
    assert!(!serialized_summary.contains("calendar.create_event"));
    assert!(!serialized_summary.contains("email.send"));
}

#[tokio::test]
async fn orchestrator_output_is_metadata_safe() {
    let output = test_runtime()
        .execute(multi_input(
            runtime_input(
                "Plan steps for Alice and alice@example.com before sending the full draft.",
                "Available tools: email.send with body payloads and file.update",
            ),
            true,
            true,
        ))
        .await
        .unwrap();

    let serialized_output = serde_json::to_string(&output).unwrap();

    assert!(!serialized_output.contains("Alice"));
    assert!(!serialized_output.contains("alice@example.com"));
    assert!(!serialized_output.contains("full draft"));
    assert!(!serialized_output.contains("email.send"));
    assert!(!serialized_output.contains("file.update"));
    assert!(!serialized_output.contains("memory context must stay out"));
}
