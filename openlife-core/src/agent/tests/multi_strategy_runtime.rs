use crate::agent::policy_store::{ModelRoutePolicy, BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY};
use crate::agent::{
    AgentExecutionBudget, AgentTask, AgentTaskKind, EvidenceQuery, EvidenceStore, HSSelectionAudit,
    HeuristicQuery, HeuristicStore, ModelRouter, MultiStrategyRuntime, MultiStrategyRuntimeInput,
    MultiStrategyRuntimePayload, PlanExecuteRuntimeStrategy, PlanStepStatus, ProposalStore,
    ProviderAvailability, RuntimeHSPacket, RuntimeInput, RuntimeOutput, RuntimeStrategy,
    RuntimeStrategyDescriptor, RuntimeStrategyInput, RuntimeStrategyKind, RuntimeStrategyOutput,
    RuntimeStrategyPayload, RuntimeStrategyPayloadKind, RuntimeStrategyRegistry, SelectedPolicyRef,
    StrategySelectionInput, StrategySelector,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::memory::MemoryStore;
use crate::scheduler::InferenceScheduler;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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
        guidance_refs: Vec::new(),
        estimated_tokens: 12,
        audit: HSSelectionAudit {
            agent_task_id: Some("task-multi-strategy".into()),
            agent_run_id: Some("run-multi-strategy".into()),
            input_digest: "digest-input".into(),
            selected_policy_ids: vec![BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into()],
            selected_heuristic_ids: Vec::new(),
            selected_guidance_ids: Vec::new(),
            selected_guidance_refs: Vec::new(),
            excluded_assets: Vec::new(),
            estimated_tokens: 12,
            token_budget: 128,
        },
    }
}

#[derive(Clone)]
struct CountingRuntimeStrategy {
    kind: RuntimeStrategyKind,
    payload_kind: RuntimeStrategyPayloadKind,
    metadata_safe_id: &'static str,
    metadata_safe_name: &'static str,
    payload: RuntimeStrategyPayload,
    execution_count: Arc<AtomicUsize>,
    seen_summaries: Arc<Mutex<Vec<Value>>>,
}

impl CountingRuntimeStrategy {
    fn react(execution_count: Arc<AtomicUsize>, seen_summaries: Arc<Mutex<Vec<Value>>>) -> Self {
        Self {
            kind: RuntimeStrategyKind::ReAct,
            payload_kind: RuntimeStrategyPayloadKind::ReAct,
            metadata_safe_id: "test_react",
            metadata_safe_name: "Test ReAct",
            payload: RuntimeStrategyPayload::ReAct(RuntimeOutput {
                run_id: Some("run-test-react".into()),
                user_output: "react adapter output".into(),
                actions: Vec::new(),
                observations: Vec::new(),
                proposal_ids: Vec::new(),
                life_event_candidates: Vec::new(),
                warnings: Vec::new(),
            }),
            execution_count,
            seen_summaries,
        }
    }

    fn plan_execute(
        execution_count: Arc<AtomicUsize>,
        seen_summaries: Arc<Mutex<Vec<Value>>>,
    ) -> Self {
        Self {
            kind: RuntimeStrategyKind::PlanExecute,
            payload_kind: RuntimeStrategyPayloadKind::PlanExecute,
            metadata_safe_id: "test_plan_execute",
            metadata_safe_name: "Test PlanExecute",
            payload: RuntimeStrategyPayload::PlanExecute(crate::agent::PlanExecutionOutput {
                report: crate::agent::PlanExecuteReport {
                    plan_id: "plan-test".into(),
                    source_run_id: Some("source-run-test".into()),
                    step_count: 1,
                    executed_read_only_step_count: 1,
                    blocked_or_proposal_required_step_count: 0,
                    governance_decisions: Vec::new(),
                    observation_summaries: Vec::new(),
                    warnings: Vec::new(),
                    guidance_impact: None,
                    metadata_safe_summary: json!({
                        "reportKind": "plan_execute_v1",
                        "planId": "plan-test",
                        "sourceRunId": "source-run-test",
                        "stepCount": 1,
                    }),
                },
                plan: crate::agent::PlanDraft {
                    objective: "selected_strategy=plan_execute task_kind=conversation reason_code=planning_intent_allowed".into(),
                    steps: Vec::new(),
                },
                traces: Vec::new(),
                runtime_outputs: Vec::new(),
                warnings: Vec::new(),
            }),
            execution_count,
            seen_summaries,
        }
    }
}

#[async_trait::async_trait]
impl RuntimeStrategy for CountingRuntimeStrategy {
    fn kind(&self) -> RuntimeStrategyKind {
        self.kind
    }

    fn metadata_safe_id(&self) -> &'static str {
        self.metadata_safe_id
    }

    fn metadata_safe_name(&self) -> &'static str {
        self.metadata_safe_name
    }

    fn payload_kind(&self) -> RuntimeStrategyPayloadKind {
        self.payload_kind
    }

    async fn execute(
        &self,
        input: RuntimeStrategyInput,
    ) -> Result<RuntimeStrategyOutput, crate::agent::AgentRuntimeError> {
        self.execution_count.fetch_add(1, Ordering::SeqCst);
        self.seen_summaries
            .lock()
            .unwrap()
            .push(input.selection.metadata_safe_summary);

        Ok(RuntimeStrategyOutput {
            payload: self.payload.clone(),
            metadata_safe_summary: json!({
                "strategyId": self.metadata_safe_id,
                "strategyName": self.metadata_safe_name,
                "payloadKind": self.payload_kind.as_str(),
            }),
            warnings: Vec::new(),
        })
    }
}

#[derive(Clone)]
struct DefaultChatMigrationRuntimeStrategy {
    inner: CountingRuntimeStrategy,
}

#[async_trait::async_trait]
impl RuntimeStrategy for DefaultChatMigrationRuntimeStrategy {
    fn kind(&self) -> RuntimeStrategyKind {
        self.inner.kind()
    }

    fn metadata_safe_id(&self) -> &'static str {
        self.inner.metadata_safe_id()
    }

    fn metadata_safe_name(&self) -> &'static str {
        self.inner.metadata_safe_name()
    }

    fn payload_kind(&self) -> RuntimeStrategyPayloadKind {
        self.inner.payload_kind()
    }

    fn descriptor(&self) -> RuntimeStrategyDescriptor {
        let mut descriptor = self.inner.descriptor();
        descriptor.default_chat_migration_permission = true;
        descriptor
    }

    async fn execute(
        &self,
        input: RuntimeStrategyInput,
    ) -> Result<RuntimeStrategyOutput, crate::agent::AgentRuntimeError> {
        self.inner.execute(input).await
    }
}

fn counting_runtime(
    react_count: Arc<AtomicUsize>,
    plan_count: Arc<AtomicUsize>,
    seen_summaries: Arc<Mutex<Vec<Value>>>,
) -> MultiStrategyRuntime {
    MultiStrategyRuntime::with_strategy_registry(
        StrategySelector::default(),
        RuntimeStrategyRegistry::new()
            .with_strategy(Box::new(CountingRuntimeStrategy::react(
                react_count,
                Arc::clone(&seen_summaries),
            )))
            .with_strategy(Box::new(CountingRuntimeStrategy::plan_execute(
                plan_count,
                seen_summaries,
            ))),
    )
}

#[test]
fn runtime_strategy_registry_readiness_passes_for_react_and_plan_execute_descriptors() {
    let react_count = Arc::new(AtomicUsize::new(0));
    let plan_count = Arc::new(AtomicUsize::new(0));
    let seen_summaries = Arc::new(Mutex::new(Vec::new()));
    let registry = RuntimeStrategyRegistry::new()
        .with_strategy(Box::new(CountingRuntimeStrategy::react(
            Arc::clone(&react_count),
            Arc::clone(&seen_summaries),
        )))
        .with_strategy(Box::new(CountingRuntimeStrategy::plan_execute(
            Arc::clone(&plan_count),
            seen_summaries,
        )));

    let report = registry.readiness_report();

    assert!(report.ready);
    assert!(report.metadata_safe);
    assert_eq!(report.executable_strategy_count, 2);
    assert!(report.blocking_reasons.is_empty());
    assert!(report
        .executable_descriptors
        .iter()
        .any(
            |descriptor| descriptor.strategy_kind == RuntimeStrategyKind::ReAct
                && descriptor.payload_kind == RuntimeStrategyPayloadKind::ReAct
                && !descriptor.default_chat_migration_permission
        ));
    assert!(report
        .executable_descriptors
        .iter()
        .any(
            |descriptor| descriptor.strategy_kind == RuntimeStrategyKind::PlanExecute
                && descriptor.payload_kind == RuntimeStrategyPayloadKind::PlanExecute
                && descriptor.proposal_first_required
        ));
    assert!(report
        .future_strategy_descriptors
        .iter()
        .any(|descriptor| descriptor.strategy_kind == "workflow"
            && descriptor.declarative_only
            && !descriptor.executable));
    assert_eq!(react_count.load(Ordering::SeqCst), 0);
    assert_eq!(plan_count.load(Ordering::SeqCst), 0);
}

#[test]
fn runtime_strategy_registry_readiness_fails_closed_for_missing_duplicate_and_migration_grants() {
    let count = Arc::new(AtomicUsize::new(0));
    let seen_summaries = Arc::new(Mutex::new(Vec::new()));

    let missing_report = RuntimeStrategyRegistry::new()
        .with_strategy(Box::new(CountingRuntimeStrategy::react(
            Arc::clone(&count),
            Arc::clone(&seen_summaries),
        )))
        .readiness_report();
    assert!(!missing_report.ready);
    assert!(missing_report
        .blocking_reasons
        .iter()
        .any(|reason| reason == "missing_required_strategy:plan_execute"));

    let duplicate_report = RuntimeStrategyRegistry::new()
        .with_strategy(Box::new(CountingRuntimeStrategy::react(
            Arc::clone(&count),
            Arc::clone(&seen_summaries),
        )))
        .with_strategy(Box::new(CountingRuntimeStrategy::react(
            Arc::clone(&count),
            Arc::clone(&seen_summaries),
        )))
        .with_strategy(Box::new(CountingRuntimeStrategy::plan_execute(
            Arc::clone(&count),
            Arc::clone(&seen_summaries),
        )))
        .readiness_report();
    assert!(!duplicate_report.ready);
    assert!(duplicate_report
        .blocking_reasons
        .iter()
        .any(|reason| reason == "duplicate_strategy_kind:react"));

    let migration_report = RuntimeStrategyRegistry::new()
        .with_strategy(Box::new(DefaultChatMigrationRuntimeStrategy {
            inner: CountingRuntimeStrategy::react(Arc::clone(&count), Arc::clone(&seen_summaries)),
        }))
        .with_strategy(Box::new(CountingRuntimeStrategy::plan_execute(
            Arc::clone(&count),
            seen_summaries,
        )))
        .readiness_report();
    assert!(!migration_report.ready);
    assert!(migration_report
        .blocking_reasons
        .iter()
        .any(|reason| reason == "default_chat_migration_permission_granted:react"));
    assert_eq!(count.load(Ordering::SeqCst), 0);
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
async fn simple_chat_executes_selected_react_adapter() {
    let react_count = Arc::new(AtomicUsize::new(0));
    let plan_count = Arc::new(AtomicUsize::new(0));
    let seen_summaries = Arc::new(Mutex::new(Vec::new()));

    let output = counting_runtime(
        Arc::clone(&react_count),
        Arc::clone(&plan_count),
        Arc::clone(&seen_summaries),
    )
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
    assert_eq!(
        output.execution_report.report_kind,
        "runtime_strategy_execution_report"
    );
    assert_eq!(
        output.execution_report.selected_strategy_kind,
        RuntimeStrategyKind::ReAct
    );
    assert_eq!(
        output.execution_report.payload_kind,
        RuntimeStrategyPayloadKind::ReAct
    );
    assert_eq!(output.execution_report.strategy_descriptor_id, "test_react");
    assert!(output.execution_report.registry_ready);
    assert!(output.execution_report.default_chat_unchanged);
    assert_eq!(
        output.execution_report.strategy_output_summary["strategyId"],
        "test_react"
    );
    assert_eq!(react_count.load(Ordering::SeqCst), 1);
    assert_eq!(plan_count.load(Ordering::SeqCst), 0);
    match output.payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output) => {
            assert_eq!(runtime_output.run_id.as_deref(), Some("run-test-react"));
            assert_eq!(runtime_output.user_output, "react adapter output");
        }
        other => panic!("expected ReAct payload, got {other:?}"),
    }

    let summaries = seen_summaries.lock().unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0]["selectedStrategyKind"], "react");
    assert!(!summaries[0].to_string().contains("memory.search"));
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
    match &output.payload {
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
async fn planning_intent_executes_selected_plan_execute_adapter_and_keeps_report() {
    let react_count = Arc::new(AtomicUsize::new(0));
    let plan_count = Arc::new(AtomicUsize::new(0));
    let seen_summaries = Arc::new(Mutex::new(Vec::new()));

    let output = counting_runtime(
        Arc::clone(&react_count),
        Arc::clone(&plan_count),
        Arc::clone(&seen_summaries),
    )
    .execute(multi_input(
        runtime_input(
            "Plan steps for Alice and alice@example.com without leaking the raw prompt.",
            "",
        ),
        true,
        true,
    ))
    .await
    .unwrap();

    assert_eq!(output.selection.kind, RuntimeStrategyKind::PlanExecute);
    assert_eq!(
        output.execution_report.selected_strategy_kind,
        RuntimeStrategyKind::PlanExecute
    );
    assert_eq!(
        output.execution_report.payload_kind,
        RuntimeStrategyPayloadKind::PlanExecute
    );
    assert_eq!(
        output.execution_report.strategy_descriptor_id,
        "test_plan_execute"
    );
    assert_eq!(
        output.execution_report.selection_reason_code,
        "planning_intent_allowed"
    );
    assert_eq!(react_count.load(Ordering::SeqCst), 0);
    assert_eq!(plan_count.load(Ordering::SeqCst), 1);
    match &output.payload {
        MultiStrategyRuntimePayload::PlanExecute(plan_output) => {
            assert_eq!(plan_output.report.plan_id, "plan-test");
            assert_eq!(plan_output.report.step_count, 1);
            assert_eq!(
                plan_output.report.metadata_safe_summary["reportKind"],
                "plan_execute_v1"
            );
        }
        other => panic!("expected PlanExecute payload, got {other:?}"),
    }

    let serialized_output = serde_json::to_string(&output).unwrap();
    assert!(!serialized_output.contains("Alice"));
    assert!(!serialized_output.contains("alice@example.com"));
    assert!(!serialized_output.contains("raw prompt"));
}

#[tokio::test]
async fn plan_execute_strategy_output_and_metadata_summary_are_metadata_safe() {
    let runtime_input = runtime_input(
        "Plan steps for Alice and alice@example.com using raw memory context.",
        "Available tools: email.send with body payloads and file.update",
    );
    let selection = StrategySelector::default().select(StrategySelectionInput {
        runtime_input: runtime_input.clone(),
        allow_planning: true,
        local_model_available: true,
    });

    let output = PlanExecuteRuntimeStrategy::default()
        .execute(RuntimeStrategyInput {
            runtime_input,
            selection,
        })
        .await
        .unwrap();

    assert_eq!(output.metadata_safe_summary["strategyId"], "plan_execute");
    assert_eq!(output.metadata_safe_summary["payloadKind"], "plan_execute");

    let serialized_output = serde_json::to_string(&output).unwrap();
    assert!(!serialized_output.contains("Alice"));
    assert!(!serialized_output.contains("alice@example.com"));
    assert!(!serialized_output.contains("raw memory context"));
    assert!(!serialized_output.contains("email.send"));
    assert!(!serialized_output.contains("file.update"));

    match output.payload {
        RuntimeStrategyPayload::PlanExecute(plan_output) => {
            assert!(plan_output.plan.objective.contains("selected_strategy="));
            assert!(!plan_output.plan.objective.contains("Alice"));
            assert!(!plan_output.plan.objective.contains("alice@example.com"));
        }
        other => panic!("expected PlanExecute strategy payload, got {other:?}"),
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
async fn blocked_local_only_selection_does_not_execute_any_strategy_adapter() {
    let react_count = Arc::new(AtomicUsize::new(0));
    let plan_count = Arc::new(AtomicUsize::new(0));
    let seen_summaries = Arc::new(Mutex::new(Vec::new()));
    let input = runtime_input_with_life_model(
        "Talk through a sensitive health topic.",
        "Available tools: memory.search",
        LifeModel::default(),
        Some(sensitive_packet()),
    );

    let output = counting_runtime(
        Arc::clone(&react_count),
        Arc::clone(&plan_count),
        Arc::clone(&seen_summaries),
    )
    .execute(multi_input(input, true, false))
    .await
    .unwrap();

    assert!(matches!(
        output.payload,
        MultiStrategyRuntimePayload::Blocked
    ));
    assert!(output.execution_report.blocked);
    assert_eq!(
        output.execution_report.payload_kind,
        RuntimeStrategyPayloadKind::Blocked
    );
    assert_eq!(
        output.execution_report.selection_reason_code,
        "governance_blocked"
    );
    assert_eq!(react_count.load(Ordering::SeqCst), 0);
    assert_eq!(plan_count.load(Ordering::SeqCst), 0);
    assert!(seen_summaries.lock().unwrap().is_empty());
}

#[tokio::test]
async fn runtime_strategy_missing_selected_adapter_fails_closed_without_raw_input() {
    let react_count = Arc::new(AtomicUsize::new(0));
    let plan_count = Arc::new(AtomicUsize::new(0));
    let seen_summaries = Arc::new(Mutex::new(Vec::new()));
    let runtime = MultiStrategyRuntime::with_strategy_registry(
        StrategySelector::default(),
        RuntimeStrategyRegistry::new().with_strategy(Box::new(
            CountingRuntimeStrategy::plan_execute(plan_count, seen_summaries),
        )),
    );

    let error = runtime
        .execute(multi_input(
            runtime_input(
                "Simple chat for Alice and alice@example.com must not leak.",
                "Available tools: email.send with body payloads",
            ),
            true,
            true,
        ))
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("runtime_strategy_missing:react"));
    assert!(!error.contains("Alice"));
    assert!(!error.contains("alice@example.com"));
    assert!(!error.contains("email.send"));
    assert_eq!(react_count.load(Ordering::SeqCst), 0);
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
