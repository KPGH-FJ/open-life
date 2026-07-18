use crate::commands::settings::{
    require_danger_action_confirmation, DangerActionConfirmationReference,
    DangerActionConfirmationRequest,
};
use crate::errors::AppError;
use crate::main_chat_runtime_facts::{
    provider_transmission_history_from_runs_with_state, ProviderTransmissionHistoryItem,
};
use crate::product_agent_dto::ProductAgentRun;
#[cfg(test)]
use crate::product_agent_dto::ProductContentReceipt;
use crate::AppState;
use openlife_core::agent::AgentRun;
use openlife_core::persistence_outbox::ProjectionDeliveryState;
use std::sync::Arc;
use tauri::State;

/// Sealed evidence that a product projection starts from an AgentRun returned
/// by the current canonical store read path. Only this module can construct
/// the token; other Tauri siblings cannot label a transient trace verified.
pub(crate) struct VerifiedAgentRunProjectionAuthority {
    _sealed: (),
}

impl VerifiedAgentRunProjectionAuthority {
    fn new() -> Self {
        Self { _sealed: () }
    }
}

pub(crate) fn project_verified_agent_run(run: AgentRun) -> ProductAgentRun {
    let authority = VerifiedAgentRunProjectionAuthority::new();
    ProductAgentRun::from_verified_store_run(run, &authority)
}

#[tauri::command]
pub async fn get_agent_run(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<ProductAgentRun>, AppError> {
    get_agent_run_with_state(&run_id, state.inner()).await
}

async fn get_agent_run_with_state(
    run_id: &str,
    state: &Arc<AppState>,
) -> Result<Option<ProductAgentRun>, AppError> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| AppError::internal("agent_run_store_unavailable"))?;
    let store = store_arc.lock().await;
    crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store
            .get_live_run(run_id)
            .map_err(|error| error.to_string()),
    )
    .map(|run| run.map(project_verified_agent_run))
    .map_err(AppError::internal)
}

#[tauri::command]
pub async fn list_agent_runs(
    limit: i64,
    offset: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProductAgentRun>, AppError> {
    list_agent_runs_with_state(limit, offset, state.inner()).await
}

async fn list_agent_runs_with_state(
    limit: i64,
    offset: i64,
    state: &Arc<AppState>,
) -> Result<Vec<ProductAgentRun>, AppError> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| AppError::internal("agent_run_store_unavailable"))?;
    let store = store_arc.lock().await;
    crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store
            .list_runs(limit, offset)
            .map_err(|error| error.to_string()),
    )
    .map(|runs| runs.into_iter().map(project_verified_agent_run).collect())
    .map_err(AppError::internal)
}

#[tauri::command]
pub async fn list_provider_transmission_history(
    limit: Option<i64>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProviderTransmissionHistoryItem>, AppError> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| AppError::internal("agent_run_store_unavailable"))?;
    let store = store_arc.lock().await;
    let runs = crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state.inner(),
        store.list_runs(limit, 0).map_err(|error| error.to_string()),
    )
    .map_err(AppError::internal)?;
    drop(store);
    provider_transmission_history_from_runs_with_state(state.inner(), &runs)
        .await
        .map_err(AppError::internal)
}

#[tauri::command]
pub async fn list_agent_runs_for_session(
    session_id: String,
    limit: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProductAgentRun>, AppError> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| AppError::internal("agent_run_store_unavailable"))?;
    let store = store_arc.lock().await;
    crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state.inner(),
        store
            .list_runs_for_session(&session_id, limit)
            .map_err(|error| error.to_string()),
    )
    .map(|runs| runs.into_iter().map(project_verified_agent_run).collect())
    .map_err(AppError::internal)
}

#[tauri::command]
pub async fn delete_agent_run(
    run_id: String,
    reason: Option<String>,
    confirmation_evidence: Option<DangerActionConfirmationReference>,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    require_agent_run_effects_allowed(state.inner())?;
    let (action_type, scoped_targets) = match confirmation_evidence.as_ref() {
        Some(reference) if reference.action_type == "agent_run_bulk_delete" => {
            ("agent_run_bulk_delete", reference.target_ids.clone())
        }
        _ => ("agent_run_delete", vec![run_id.clone()]),
    };
    let affected_count = scoped_targets.len().max(1);
    let confirmation_arguments = serde_json::json!({
        "delete_reason": reason.as_deref(),
    });
    require_danger_action_confirmation(
        DangerActionConfirmationRequest {
            action_type,
            target_ids_for_new_challenge: &scoped_targets,
            requested_target: Some(run_id.as_str()),
            affected_count: Some(affected_count),
            reference: confirmation_evidence.as_ref(),
            arguments: &confirmation_arguments,
            arguments_summary:
                "删除 AgentRun 运行记录并写入删除原因；批量范围中的每个目标使用独立 single-use grant。",
            governed_data_import_recovery: None,
        },
        &window,
        state.inner(),
    )
    .await?;
    delete_agent_run_after_confirmation_with_state(&run_id, reason.as_deref(), state.inner()).await
}

fn require_agent_run_effects_allowed(state: &Arc<AppState>) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))
}

fn admit_agent_run_write(
    state: &Arc<AppState>,
) -> Result<crate::persistence_coordinator::AgentRunCanonicalWriteAdmission, AppError> {
    state
        .persistence_coordinator
        .admit_agent_run_write()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))
}

async fn delete_agent_run_after_confirmation_with_state(
    run_id: &str,
    reason: Option<&str>,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    // Confirmation can await user input while persistence health degrades.
    // Capture a generation-bound AgentRun admission after confirmation; the
    // gateway is the sole per-run causal-lock owner and revalidates this
    // admission after its task/event fences at the commit point.
    let admission = admit_agent_run_write(state)?;
    let receipt = crate::terminal_owner_write_gateway::delete_agent_run_with_tombstone(
        state, run_id, reason, &admission,
    )
    .await
    .map_err(AppError::internal)?;
    crate::memory_gateway::reconcile_agent_run_blocking_outbox_event_with_state(
        state,
        &receipt.event_id,
    )
    .await
    .map_err(AppError::external)?;
    ensure_agent_run_projection_applied(state, &receipt.event_id, "agent_run_delete").await
}

#[tauri::command]
pub async fn restore_agent_run(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ProductAgentRun, AppError> {
    restore_agent_run_with_state(&run_id, state.inner())
        .await
        .map(project_verified_agent_run)
}

pub(crate) async fn restore_agent_run_with_state(
    run_id: &str,
    state: &Arc<AppState>,
) -> Result<AgentRun, AppError> {
    // The gateway owns causal serialization. This command captures only a
    // generation-bound admission after the user-visible restore request.
    let admission = admit_agent_run_write(state)?;
    // 1. Restore the run in store
    let restore_event_id = crate::terminal_owner_write_gateway::restore_agent_run_with_receipt(
        state, run_id, &admission,
    )
    .await
    .map_err(AppError::internal)?
    .event_id;
    crate::memory_gateway::reconcile_agent_run_blocking_outbox_event_with_state(
        state,
        &restore_event_id,
    )
    .await
    .map_err(AppError::external)?;
    ensure_agent_run_projection_applied(state, &restore_event_id, "agent_run_restore").await?;

    // 2. Retrieve and return the restored run
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        crate::terminal_owner_write_gateway::register_agent_run_store_result(
            state,
            store
                .get_live_run(run_id)
                .map_err(|error| error.to_string()),
        )
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("Run not found after restore"))
    } else {
        Err(AppError::internal("AgentRun store not available"))
    }
}

async fn ensure_agent_run_projection_applied(
    state: &Arc<AppState>,
    event_id: &str,
    operation: &str,
) -> Result<(), AppError> {
    let store = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| AppError::internal("agent_run_store_unavailable"))?
        .lock()
        .await;
    let summary = crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store
            .projection_summary(event_id)
            .map_err(|error| error.to_string()),
    )
    .map_err(AppError::internal)?;
    if summary.state() == ProjectionDeliveryState::Applied {
        return Ok(());
    }
    Err(AppError::external(
        serde_json::json!({
            "operation": operation,
            "canonicalCommitted": true,
            "outboxEventId": event_id,
            "projectionState": summary.state(),
            "pending": summary.pending,
            "degraded": summary.degraded,
            "superseded": summary.superseded,
            "compensated": summary.compensated,
        })
        .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detached_test_execution_epoch(
        task_session_id: &str,
    ) -> crate::main_chat_cancellation::MainChatExecutionEpoch {
        let registry = crate::main_chat_cancellation::MainChatCancellationRegistry::default();
        let registration = registry.register(task_session_id);
        registration.execution_epoch()
    }

    #[test]
    fn product_content_receipt_contract_is_minimal_and_uses_a_public_digest() {
        let receipt: openlife_core::agent::ContentReceipt =
            serde_json::from_value(serde_json::json!({
            "receiptId": uuid::Uuid::new_v4().to_string(),
            "runId": uuid::Uuid::new_v4().to_string(),
            "actionId": "action-contract",
            "observationId": "observation-contract",
            "field": "action_output_observation_content",
            "kind": "tool_output",
            "provenance": "observed_tool_adapter_body",
            "byteCount": 7,
            "opaqueBodyReceipt": format!("hmac-sha256:{}", "b".repeat(64)),
            "authorityTag": format!("hmac-sha256:{}", "c".repeat(64)),
            }))
            .unwrap();
        let value =
            serde_json::to_value(ProductContentReceipt::from_unverified_receipt(receipt)).unwrap();
        let object = value.as_object().unwrap();
        let keys = object
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            keys,
            [
                "byteCount",
                "digest",
                "kind",
                "provenance",
                "verified",
                "version",
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(value["verified"], false);
        assert!(value["digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:")));
        let encoded = serde_json::to_string(&value).unwrap();
        for forbidden in [
            "receiptId",
            "issuanceId",
            "runId",
            "actionId",
            "observationId",
            "canonicalStoreIdentity",
            "bindingReceipt",
            "bodyReceipt",
            "authorityTag",
            "hmac-sha256:",
        ] {
            assert!(
                !encoded.contains(forbidden),
                "leaked {forbidden}: {encoded}"
            );
        }
        let typescript = include_str!("../../../frontend/src/tauri.ts");
        let contract = typescript
            .split("export interface ContentReceipt {")
            .nth(1)
            .and_then(|tail| tail.split("\n}").next())
            .expect("TypeScript ContentReceipt contract");
        for required in [
            "version: number",
            "kind:",
            "provenance:",
            "byteCount: number",
            "digest: string",
            "verified: boolean",
        ] {
            assert!(contract.contains(required), "missing TS field {required}");
        }
        for forbidden in [
            "receiptId",
            "issuanceId",
            "runId",
            "actionId",
            "observationId",
            "canonicalStoreIdentity",
            "bindingReceipt",
            "bodyReceipt",
            "authorityTag",
        ] {
            assert!(
                !contract.contains(forbidden),
                "TS contract leaked {forbidden}"
            );
        }
    }

    #[test]
    fn product_agent_run_contract_drops_transient_input_and_reasoning_body() {
        let run = AgentRun::new_chat_run("product-contract", "private transient body");
        let encoded = serde_json::to_string(&project_verified_agent_run(run)).unwrap();
        assert!(!encoded.contains("private transient body"));
        assert!(!encoded.contains("userInput"));
        assert!(!encoded.contains("reasoningTrace\""));
    }

    #[tokio::test]
    async fn shipped_agent_run_dto_marks_only_real_v2_store_reload_verified() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let store = state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await
            .clone();
        let mut registry = openlife_core::mcp::McpRegistry::new();
        let mut manifest = openlife_core::tool_manifest::ToolManifest::new(
            "product_receipt_fixture",
            "Product receipt fixture",
            serde_json::json!({"type": "object"}),
            "low",
            "1",
            openlife_core::tool_manifest::ToolSource::BuiltIn,
        );
        manifest.id = "builtin.product_receipt_fixture".into();
        manifest.capabilities = vec!["read".into()];
        manifest.action_type = "read".into();
        manifest.idempotency_contract =
            openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent;
        registry.register_builtin(
            manifest,
            Box::new(|_| Ok("D010_PRODUCT_DTO_ADAPTER_BODY".into())),
        );
        let permission_store =
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let audit_dir = tempfile::tempdir().unwrap();
        let audit_store =
            openlife_core::mcp_audit::McpAuditStore::new(audit_dir.path().join("audit.db"));
        let privacy_engine = openlife_core::privacy::PrivacyEngine::new();
        let mut run = AgentRun::new_chat_run("product-receipt-reload", "transient input");
        store.create_run(&run).unwrap();
        let context = openlife_core::agent::ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_agent_run_store(&store);
        let result = openlife_core::agent::ToolGateway::from_executor_config(
            openlife_core::agent::ActionExecutorConfig::default(),
        )
        .execute(
            openlife_core::agent::AgentActionRequest {
                action_type: "builtin_tool".into(),
                target: "product_receipt_fixture".into(),
                input: serde_json::json!({"arguments": {}}),
                source_run_id: Some(run.id.clone()),
                step_index: 1,
            },
            &context,
        )
        .await
        .unwrap();
        run.actions.push(result.action);
        run.observations.push(result.observation);
        store.update_run(&run).unwrap();

        let canonical = store
            .get_run(&run.id)
            .unwrap()
            .expect("canonical AgentRun reload");
        let canonical_encoded = serde_json::to_string(&canonical).unwrap();
        assert!(
            canonical_encoded.contains("receiptId") && canonical_encoded.contains("hmac-sha256:"),
            "canonical owner lost internal verification authority: {canonical_encoded}"
        );

        const NESTED_SENTINEL: &str = "D010_PRODUCT_NESTED_DYNAMIC_SENTINEL";
        let mut projection_counterfactual = canonical.clone();
        projection_counterfactual.actions[0].input = serde_json::json!({
            "nested": [{"secret": NESTED_SENTINEL}],
        });
        projection_counterfactual.actions[0].output = Some(serde_json::json!({
            "nested": {"receiptId": NESTED_SENTINEL},
        }));
        projection_counterfactual.observations[0].structured_result =
            Some(serde_json::json!({"deep": [{"body": NESTED_SENTINEL}]}));
        let counterfactual_product =
            serde_json::to_value(project_verified_agent_run(projection_counterfactual)).unwrap();
        let counterfactual_encoded = serde_json::to_string(&counterfactual_product).unwrap();
        assert!(!counterfactual_encoded.contains(NESTED_SENTINEL));
        assert!(counterfactual_product["actions"][0].get("input").is_none());
        assert!(counterfactual_product["actions"][0].get("output").is_none());
        assert!(counterfactual_product["observations"][0]
            .get("structuredResult")
            .is_none());
        let action_keys = counterfactual_product["actions"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        let allowed_action_keys = [
            "id",
            "actionType",
            "target",
            "status",
            "permissionDecision",
            "startedAt",
            "finishedAt",
            "error",
            "timestamp",
            "toolScope",
            "reactTrace",
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
        assert!(
            action_keys.is_subset(&allowed_action_keys),
            "ProductAgentAction gained non-whitelisted keys: {action_keys:?}"
        );
        let observation_keys = counterfactual_product["observations"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        let allowed_observation_keys = [
            "id",
            "actionId",
            "content",
            "source",
            "timestamp",
            "reactTrace",
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
        assert!(
            observation_keys.is_subset(&allowed_observation_keys),
            "ProductAgentObservation gained non-whitelisted keys: {observation_keys:?}"
        );
        assert_eq!(
            serde_json::to_string(&store.get_run(&run.id).unwrap().unwrap()).unwrap(),
            canonical_encoded,
            "product projection mutated canonical AgentRun truth"
        );

        let product = get_agent_run_with_state(&run.id, &state)
            .await
            .unwrap()
            .expect("shipped DTO reload");
        let value = serde_json::to_value(product).unwrap();
        let receipt = &value["actions"][0]["reactTrace"]["outputReceipt"];
        assert_eq!(
            value["actions"][0]["reactTrace"]["toolName"], "unknown_tool",
            "the canonical store does not currently issue separate tool-name authority"
        );
        assert_eq!(receipt["version"], 2);
        assert_eq!(receipt["verified"], true);
        assert_eq!(receipt.as_object().unwrap().len(), 6);
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(!encoded.contains("D010_PRODUCT_DTO_ADAPTER_BODY"));
        for forbidden in [
            "receiptId",
            "issuanceId",
            "canonicalStoreIdentity",
            "bindingReceipt",
            "bodyReceipt",
            "authorityTag",
            "hmac-sha256:",
        ] {
            assert!(
                !encoded.contains(forbidden),
                "leaked {forbidden}: {encoded}"
            );
        }
    }

    fn install_global_read_only_degradation(state: &mut Arc<AppState>) {
        let coordinator = Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap(),
        );
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        coordinator.seal();
        coordinator.degrade_globally("injected_global_read_only");
        assert!(coordinator.require_effects_allowed().is_err());
        Arc::get_mut(state)
            .expect("test state must have one outer owner")
            .persistence_coordinator = coordinator;
    }

    fn install_release_like_persistence_coordinator(state: &mut Arc<AppState>) {
        let coordinator = Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap(),
        );
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        coordinator.seal();
        coordinator
            .require_effects_allowed()
            .expect("complete sealed manifest enables canonical writes");
        Arc::get_mut(state)
            .expect("test state must have one outer owner")
            .persistence_coordinator = coordinator;
    }

    fn typed_generation_projection_fixture(
        now: chrono::DateTime<chrono::Utc>,
    ) -> crate::terminal_owner_write_gateway::MainChatGenerationProjection {
        crate::terminal_owner_write_gateway::MainChatGenerationProjection {
            context_summary: openlife_core::agent::ContextSummary {
                life_model_empty: false,
                included_life_model_sections: vec!["goal_priority".into()],
                memory_hit_count: 1,
                memory_sources: vec!["memory-ref".into()],
                used_tools_prompt: true,
                redaction_applied: true,
                redaction_level: openlife_core::agent::RedactionLevel::Strict,
            },
            model_route: openlife_core::agent::ModelRouteTrace {
                provider: "local".into(),
                model: "typed-fixture".into(),
                route_type: "local".into(),
                prefer_local: true,
                local_model: "typed-fixture".into(),
                reason: "typed_projection_test".into(),
                privacy_level: openlife_core::agent::RedactionLevel::Strict,
                latency_ms: Some(1),
                retry_count: 0,
                fallback_reason: None,
                provider_health_is_estimated: Some(false),
            },
            output_preview: "typed projection output".into(),
            reasoning_strategy: Some("react".into()),
            reasoning_trace: openlife_core::agent::ReasoningTrace {
                generation_result: Some(serde_json::json!({"typedProjection": true})),
                ..Default::default()
            },
            terminal_owner_generation: 1,
            actions: vec![openlife_core::agent::AgentAction {
                id: "typed-action".into(),
                action_type: "read".into(),
                target: Some("local-resource".into()),
                input: serde_json::json!({"metadataOnly": true}),
                output: Some(serde_json::json!({"observed": true})),
                status: "completed".into(),
                permission_decision: None,
                started_at: Some(now),
                finished_at: Some(now),
                error: None,
                timestamp: now,
                tool_scope: None,
                react_trace: None,
                runtime_execution_receipt: None,
            }],
            observations: vec![openlife_core::agent::AgentObservation {
                id: "typed-observation".into(),
                action_id: Some("typed-action".into()),
                content: "metadata safe observed result".into(),
                source: "typed_projection_test".into(),
                structured_result: Some(serde_json::json!({"itemCount": 1})),
                timestamp: now,
                react_trace: None,
            }],
            hs_selection_audit: None,
            behavior_checks: Vec::new(),
            step_count: 2,
            tool_call_count: 1,
        }
    }

    fn completed_agent_run_with_generation_projection(
        session_id: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AgentRun {
        let projection = typed_generation_projection_fixture(now);
        let mut run = AgentRun::new_chat_run(session_id, "metadata safe input");
        run.context_summary = Some(projection.context_summary);
        run.model_route = Some(projection.model_route);
        run.output_preview = Some(projection.output_preview);
        run.reasoning_strategy = projection.reasoning_strategy;
        run.reasoning_trace = Some(projection.reasoning_trace);
        run.actions = projection.actions;
        run.observations = projection.observations;
        run.hs_selection_audit = projection.hs_selection_audit;
        run.behavior_checks = projection.behavior_checks;
        run.step_count = projection.step_count;
        run.tool_call_count = projection.tool_call_count;
        run.status = openlife_core::agent::AgentRunStatus::Completed;
        run.finished_at = Some(now);
        run
    }

    fn typed_blocked_projection_fixture(
        now: chrono::DateTime<chrono::Utc>,
        disposition: crate::terminal_owner_write_gateway::MainChatBlockedDisposition,
    ) -> crate::terminal_owner_write_gateway::MainChatBlockedProjection {
        let generation = typed_generation_projection_fixture(now);
        crate::terminal_owner_write_gateway::MainChatBlockedProjection {
            reasoning_strategy: generation.reasoning_strategy,
            reasoning_trace: generation.reasoning_trace,
            actions: generation.actions,
            observations: generation.observations,
            step_count: generation.step_count,
            tool_call_count: generation.tool_call_count,
            disposition,
        }
    }

    fn install_startup_like_persistence_coordinator(state: &mut Arc<AppState>) {
        let coordinator = Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap(),
        );
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        assert!(coordinator.startup_reconciliation_mutations_safe());
        assert!(coordinator.require_effects_allowed().is_err());
        Arc::get_mut(state)
            .expect("test state must have one outer owner")
            .persistence_coordinator = coordinator;
    }

    async fn startup_task_owner_fixture(
        status: openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus,
    ) -> (
        Arc<AppState>,
        AgentRun,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    ) {
        use openlife_core::agent::main_chat_agent_v1::{
            AgentTaskSessionDraft, AgentTaskSessionStatus, MainChatAgentStrategy,
        };

        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let operation_id = uuid::Uuid::new_v4().to_string();
        let chat_session_id = format!("startup-task-owner:{operation_id}");
        state
            .memory_store
            .lock()
            .await
            .create_chat_session(&chat_session_id, "Startup task owner fixture")
            .unwrap();
        let task = {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            let task = store
                .create_session_with_id(
                    operation_id.clone(),
                    AgentTaskSessionDraft {
                        chat_session_id: chat_session_id.clone(),
                        user_goal: "Project exact startup lifecycle".into(),
                        selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                        current_plan_summary: None,
                        context_snapshot_refs: vec![],
                    },
                )
                .unwrap();
            match status {
                AgentTaskSessionStatus::Running => task,
                AgentTaskSessionStatus::WaitingPermission => {
                    store.mark_waiting_permission(&task.id).unwrap()
                }
                AgentTaskSessionStatus::Completed => store
                    .complete_session(&task.id, "completed fixture")
                    .unwrap(),
                AgentTaskSessionStatus::Cancelled => {
                    store.cancel_session(&task.id, "cancelled fixture").unwrap()
                }
                AgentTaskSessionStatus::Blocked => {
                    store.block_session(&task.id, "blocked fixture").unwrap()
                }
                AgentTaskSessionStatus::Failed => {
                    store.fail_session(&task.id, "failed fixture").unwrap()
                }
            }
        };
        let mut run = AgentRun::new_chat_run(&chat_session_id, "metadata safe input");
        run.id = operation_id;
        run.task_id = task.id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_startup_like_persistence_coordinator(&mut state);
        (state, run, task)
    }

    async fn create_sealing_agent_run_fixture(
        state: &Arc<AppState>,
        initially_deleted: bool,
    ) -> (
        AgentRun,
        Option<openlife_core::persistence_outbox::CanonicalMutationReceipt>,
    ) {
        use openlife_core::agent::main_chat_agent_v1::{
            AgentTaskSessionDraft, MainChatAgentStrategy,
        };

        let operation_id = uuid::Uuid::new_v4().to_string();
        let chat_session_id = format!("sealing-agent-run:{operation_id}");
        let user_goal = "AgentRun lifecycle must not cross terminal sealing";
        {
            let memory = state.memory_store.lock().await;
            memory
                .create_chat_session(&chat_session_id, "Sealing fixture")
                .unwrap();
        }
        let session = {
            let task_store = state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            task_store
                .create_session_with_id(
                    operation_id.clone(),
                    AgentTaskSessionDraft {
                        chat_session_id: chat_session_id.clone(),
                        user_goal: user_goal.into(),
                        selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                        current_plan_summary: None,
                        context_snapshot_refs: vec![],
                    },
                )
                .unwrap()
        };
        let canonical_message = {
            let memory = state.memory_store.lock().await;
            memory
                .save_message_idempotent_with_proof(
                    &chat_session_id,
                    &openlife_core::llm::ChatMessage {
                        role: "user".into(),
                        content: user_goal.into(),
                    },
                    &operation_id,
                )
                .unwrap()
        };
        let admission = {
            let task_store = state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            task_store
                .bind_session_canonical_user_message(
                    &session.id,
                    &canonical_message.receipt().canonical_ref,
                    user_goal,
                )
                .unwrap();
            task_store
                .issue_terminal_owner_epoch_admission(&session.id, &operation_id, canonical_message)
                .unwrap()
        };
        let mut run = AgentRun::new_chat_run(&chat_session_id, user_goal);
        run.id = operation_id.clone();
        run.task_id = operation_id;
        let deleted = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
            initially_deleted.then(|| {
                store
                    .delete_run_with_tombstone(&run.id, Some("sealing restore precondition"))
                    .unwrap()
            })
        };
        let event_store = state
            .main_chat_agent_event_store
            .as_ref()
            .unwrap()
            .lock()
            .await;
        let epoch = event_store
            .open_terminal_owner_epoch_from_admission(admission)
            .unwrap();
        event_store
            .begin_terminal_owner_seal(&session.id, &run.id, epoch.generation())
            .unwrap();
        (run, deleted)
    }

    async fn create_open_terminal_owner_fixture(
        state: &Arc<AppState>,
    ) -> (
        AgentRun,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
        u64,
    ) {
        use openlife_core::agent::main_chat_agent_v1::{
            AgentTaskSessionDraft, MainChatAgentStrategy,
        };

        let operation_id = uuid::Uuid::new_v4().to_string();
        let chat_session_id = format!("startup-durable-event:{operation_id}");
        let user_goal = "Project only the latest durable lifecycle event";
        state
            .memory_store
            .lock()
            .await
            .create_chat_session(&chat_session_id, "Startup durable event fixture")
            .unwrap();
        let session = state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_session_with_id(
                operation_id.clone(),
                AgentTaskSessionDraft {
                    chat_session_id: chat_session_id.clone(),
                    user_goal: user_goal.into(),
                    selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                    current_plan_summary: None,
                    context_snapshot_refs: vec![],
                },
            )
            .unwrap();
        let canonical_message = state
            .memory_store
            .lock()
            .await
            .save_message_idempotent_with_proof(
                &chat_session_id,
                &openlife_core::llm::ChatMessage {
                    role: "user".into(),
                    content: user_goal.into(),
                },
                &operation_id,
            )
            .unwrap();
        let admission = {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            store
                .bind_session_canonical_user_message(
                    &session.id,
                    &canonical_message.receipt().canonical_ref,
                    user_goal,
                )
                .unwrap();
            store
                .issue_terminal_owner_epoch_admission(&session.id, &operation_id, canonical_message)
                .unwrap()
        };
        let mut run = AgentRun::new_chat_run(&chat_session_id, user_goal);
        run.id = operation_id;
        run.task_id = session.id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        let epoch = state
            .main_chat_agent_event_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .open_terminal_owner_epoch_from_admission(admission)
            .unwrap();
        (run, session, epoch.generation())
    }

    async fn append_terminal_final_fixture(
        state: &Arc<AppState>,
        session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
        run: &AgentRun,
        generation: u64,
        status: &str,
    ) -> crate::main_chat_event_stream::MainChatAgentDurableEvent {
        let head = state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_owner_head(&session.id)
            .unwrap()
            .unwrap();
        let event_store = state
            .main_chat_agent_event_store
            .as_ref()
            .unwrap()
            .lock()
            .await;
        event_store
            .begin_terminal_owner_seal(&session.id, &run.id, generation)
            .unwrap();
        event_store
            .append_terminal_final_and_seal(
                crate::main_chat_event_stream::MainChatTerminalFinalizationInput {
                    task_session_id: session.id.clone(),
                    run_id: run.id.clone(),
                    epoch_generation: generation,
                    delivery_id: format!("startup-final:{}:{status}", session.id),
                    expected_task_owner_revision: head.revision(),
                    expected_task_owner_digest: head.digest().to_string(),
                    status: status.to_string(),
                },
            )
            .unwrap()
    }

    #[tokio::test]
    async fn shipped_agent_run_read_hides_canonical_delete_before_projection_finishes() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let run = AgentRun::new_chat_run("deleted-product-read", "metadata safe input");
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
            store
                .delete_run_with_tombstone(&run.id, Some("product read fence"))
                .unwrap();
            assert!(store.get_run_including_deleted(&run.id).unwrap().is_some());
        }

        assert!(get_agent_run_with_state(&run.id, &state)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn global_degradation_blocks_agent_run_delete_before_canonical_or_outbox_change() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let run = AgentRun::new_chat_run("degraded-delete-task", "metadata safe input");
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_global_read_only_degradation(&mut state);

        let error = delete_agent_run_after_confirmation_with_state(
            &run.id,
            Some("must remain unapplied"),
            &state,
        )
        .await
        .unwrap_err();
        assert!(error
            .to_string()
            .contains(crate::persistence_coordinator::PERSISTENCE_EFFECTS_BLOCKED));
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        assert!(store
            .get_live_run(&run.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_none());
        assert!(store
            .list_replayable_projection_deliveries(20)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn global_degradation_blocks_agent_run_restore_before_canonical_or_outbox_change() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let run = AgentRun::new_chat_run("degraded-restore-task", "metadata safe input");
        let deleted = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
            store
                .delete_run_with_tombstone(&run.id, Some("precondition delete"))
                .unwrap()
        };
        install_global_read_only_degradation(&mut state);

        let error = restore_agent_run_with_state(&run.id, &state)
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains(crate::persistence_coordinator::PERSISTENCE_EFFECTS_BLOCKED));
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        assert!(store
            .get_run_including_deleted(&run.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some());
        let deliveries = store.list_replayable_projection_deliveries(20).unwrap();
        assert_eq!(deliveries.len(), 3);
        assert!(deliveries
            .iter()
            .all(|delivery| delivery.event_id == deleted.event_id));
        assert!(store
            .superseded_tombstone_ids_for_restore_event(&deleted.event_id)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn canonical_conversation_delete_blocks_agent_run_restore_before_projection() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let session_id = "deleted-parent-before-agent-run-projection";
        let run = AgentRun::new_chat_run(session_id, "metadata safe input");
        {
            let memory = state.memory_store.lock().await;
            memory
                .create_chat_session(session_id, "Deleted parent")
                .unwrap();
        }
        let deleted_run = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
            store
                .delete_run_with_tombstone(&run.id, Some("precondition delete"))
                .unwrap()
        };
        let deleted_parent = {
            let memory = state.memory_store.lock().await;
            memory
                .delete_chat_session_with_tombstone(session_id, Some("canonical parent delete"))
                .unwrap()
        };
        install_release_like_persistence_coordinator(&mut state);

        let error = restore_agent_run_with_state(&run.id, &state)
            .await
            .expect_err("canonical parent tombstone must block restore before projection");
        assert!(
            error
                .to_string()
                .contains("agent_run_restore_blocked_by_conversation_tombstone"),
            "wrong canonical parent blocker: {error}"
        );
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        assert!(store.get_live_run(&run.id).unwrap().is_none());
        assert_eq!(
            store
                .projection_summary(&deleted_run.event_id)
                .unwrap()
                .pending,
            3,
            "failed restore must not supersede the AgentRun tombstone"
        );
        drop(store);
        assert_eq!(
            state
                .memory_store
                .lock()
                .await
                .projection_summary(&deleted_parent.event_id)
                .unwrap()
                .pending,
            5,
            "the test must exercise the canonical-before-projection window"
        );
    }

    #[tokio::test]
    async fn terminal_owner_sealing_rejects_agent_run_delete_without_owner_or_outbox_change() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        install_release_like_persistence_coordinator(&mut state);
        let (run, deleted) = create_sealing_agent_run_fixture(&state, false).await;
        assert!(deleted.is_none());

        let error = delete_agent_run_after_confirmation_with_state(
            &run.id,
            Some("must not cross sealing"),
            &state,
        )
        .await
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("agent_run_lifecycle_mutation_rejected_while_terminal_owner_sealing"));
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        assert!(store.get_live_run(&run.id).unwrap().is_some());
        assert!(store
            .list_replayable_projection_deliveries(20)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn terminal_owner_sealing_rejects_agent_run_restore_without_owner_or_outbox_change() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        install_release_like_persistence_coordinator(&mut state);
        let (run, deleted) = create_sealing_agent_run_fixture(&state, true).await;
        let deleted = deleted.unwrap();

        let error = restore_agent_run_with_state(&run.id, &state)
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("agent_run_lifecycle_mutation_rejected_while_terminal_owner_sealing"));
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        assert!(store.get_live_run(&run.id).unwrap().is_none());
        let deliveries = store.list_replayable_projection_deliveries(20).unwrap();
        assert_eq!(deliveries.len(), 3);
        assert!(deliveries
            .iter()
            .all(|delivery| delivery.event_id == deleted.event_id));
        assert!(store
            .superseded_tombstone_ids_for_restore_event(&deleted.event_id)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn degradation_after_delete_admission_wins_before_agent_run_and_outbox_commit() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let run = AgentRun::new_chat_run("late-degraded-delete-task", "metadata safe input");
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_release_like_persistence_coordinator(&mut state);
        let (reached, release) =
            crate::terminal_owner_write_gateway::install_agent_run_lifecycle_commit_test_barrier(
                &run.id,
            );

        let operation_state = Arc::clone(&state);
        let run_id = run.id.clone();
        let operation = tokio::spawn(async move {
            delete_agent_run_after_confirmation_with_state(
                &run_id,
                Some("must lose to canonical degradation"),
                &operation_state,
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), reached)
            .await
            .expect("delete must not self-deadlock before the commit barrier")
            .expect("delete reached the post-task-fence pre-transaction barrier");
        state
            .persistence_coordinator
            .degrade_globally("injected_after_agent_run_delete_admission");
        release.send(()).unwrap();

        let error = operation.await.unwrap().unwrap_err();
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        assert!(store
            .get_live_run(&run.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_none());
        assert!(store
            .list_replayable_projection_deliveries(20)
            .unwrap()
            .is_empty());
        assert!(
            error
                .to_string()
                .contains(crate::persistence_coordinator::PERSISTENCE_ADMISSION_INVALIDATED),
            "late degradation returned the wrong causal error: {error}"
        );
    }

    #[tokio::test]
    async fn degradation_after_restore_admission_wins_before_agent_run_and_outbox_commit() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let run = AgentRun::new_chat_run("late-degraded-restore-task", "metadata safe input");
        let deleted = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
            store
                .delete_run_with_tombstone(&run.id, Some("precondition delete"))
                .unwrap()
        };
        install_release_like_persistence_coordinator(&mut state);
        let (reached, release) =
            crate::terminal_owner_write_gateway::install_agent_run_lifecycle_commit_test_barrier(
                &run.id,
            );

        let operation_state = Arc::clone(&state);
        let run_id = run.id.clone();
        let operation =
            tokio::spawn(
                async move { restore_agent_run_with_state(&run_id, &operation_state).await },
            );
        tokio::time::timeout(std::time::Duration::from_secs(2), reached)
            .await
            .expect("restore must not self-deadlock before the commit barrier")
            .expect("restore reached the post-task-fence pre-transaction barrier");
        state
            .persistence_coordinator
            .degrade_globally("injected_after_agent_run_restore_admission");
        release.send(()).unwrap();

        let error = operation.await.unwrap().unwrap_err();
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        assert_eq!(
            store.lifecycle_task_id(&run.id).unwrap().as_deref(),
            Some(run.task_id.as_str()),
            "the tombstoned canonical AgentRun row must still exist"
        );
        assert!(
            store.get_live_run(&run.id).unwrap().is_none(),
            "late degradation must leave the AgentRun tombstoned"
        );
        let deliveries = store.list_replayable_projection_deliveries(20).unwrap();
        assert_eq!(deliveries.len(), 3);
        assert!(deliveries
            .iter()
            .all(|delivery| delivery.event_id == deleted.event_id));
        assert!(store
            .superseded_tombstone_ids_for_restore_event(&deleted.event_id)
            .unwrap()
            .is_empty());
        assert!(
            error
                .to_string()
                .contains(crate::persistence_coordinator::PERSISTENCE_ADMISSION_INVALIDATED),
            "late degradation returned the wrong causal error: {error}"
        );
    }

    #[tokio::test]
    async fn degradation_after_normal_agent_run_update_admission_wins_before_owner_commit() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let run = AgentRun::new_builder_run("late-degraded-normal-update");
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_release_like_persistence_coordinator(&mut state);
        let mut projected = run.clone();
        projected.status = openlife_core::agent::AgentRunStatus::Completed;
        projected.finished_at = Some(chrono::Utc::now());
        let (reached, release) =
            crate::terminal_owner_write_gateway::install_agent_run_lifecycle_commit_test_barrier(
                &run.id,
            );
        let operation_state = Arc::clone(&state);
        let operation = tokio::spawn(async move {
            crate::terminal_owner_write_gateway::replace_agent_run_for_test(
                &operation_state,
                &projected,
            )
            .await
        });
        reached.await.expect("normal update reached commit barrier");
        state
            .persistence_coordinator
            .degrade_globally("injected_after_normal_agent_run_update_admission");
        release.send(()).unwrap();

        let error = operation.await.unwrap().unwrap_err();
        assert!(error.contains(crate::persistence_coordinator::PERSISTENCE_ADMISSION_INVALIDATED));
        let stored = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run.id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, openlife_core::agent::AgentRunStatus::Running);
        assert!(stored.finished_at.is_none());
    }

    #[tokio::test]
    async fn readonly_agent_run_update_failure_degrades_runtime_before_more_effects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-runs.db");
        let run = AgentRun::new_builder_run("readonly-agent-run-update");
        {
            let writable = openlife_core::agent::AgentRunStore::new(&path).unwrap();
            writable.create_run(&run).unwrap();
        }
        let readonly = openlife_core::agent::AgentRunStore::open_read_only_existing(&path).unwrap();
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(readonly)));
        install_release_like_persistence_coordinator(&mut state);
        let mut projected = run.clone();
        projected.status = openlife_core::agent::AgentRunStatus::Completed;
        projected.finished_at = Some(chrono::Utc::now());

        let error =
            crate::terminal_owner_write_gateway::replace_agent_run_for_test(&state, &projected)
                .await
                .unwrap_err();

        assert!(
            error.to_ascii_lowercase().contains("readonly"),
            "the fixture must exercise a real SQLite readonly write error: {error}"
        );
        let health = state.persistence_coordinator.snapshot();
        assert_eq!(
            health.mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded
        );
        let agent_run_health = health
            .stores
            .iter()
            .find(|store| store.store == "AgentRunStore")
            .expect("AgentRunStore health entry");
        assert_eq!(
            agent_run_health.mode,
            crate::persistence_coordinator::PersistenceStoreMode::Unavailable
        );
        assert_eq!(
            agent_run_health.reason_code.as_deref(),
            Some("runtime_durable_store_failure")
        );
        assert!(state
            .persistence_coordinator
            .require_effects_allowed()
            .is_err());
    }

    #[tokio::test]
    async fn readonly_agent_run_create_failure_degrades_runtime_before_more_effects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-runs.db");
        {
            let _writable = openlife_core::agent::AgentRunStore::new(&path).unwrap();
        }
        let readonly = openlife_core::agent::AgentRunStore::open_read_only_existing(&path).unwrap();
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(readonly)));
        install_release_like_persistence_coordinator(&mut state);
        let run = AgentRun::new_builder_run("readonly-agent-run-create");

        let error = crate::terminal_owner_write_gateway::create_agent_run(&state, &run)
            .await
            .unwrap_err();

        assert!(
            error.to_ascii_lowercase().contains("readonly"),
            "the fixture must exercise a real SQLite readonly write error: {error}"
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded
        );
        assert!(state
            .persistence_coordinator
            .require_effects_allowed()
            .is_err());
    }

    #[tokio::test]
    async fn readonly_agent_run_delete_failure_degrades_runtime_before_more_effects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-runs.db");
        let run = AgentRun::new_calibration_run();
        {
            let writable = openlife_core::agent::AgentRunStore::new(&path).unwrap();
            writable.create_run(&run).unwrap();
        }
        let readonly = openlife_core::agent::AgentRunStore::open_read_only_existing(&path).unwrap();
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(readonly)));
        install_release_like_persistence_coordinator(&mut state);

        let error = delete_agent_run_after_confirmation_with_state(
            &run.id,
            Some("readonly delete fixture"),
            &state,
        )
        .await
        .unwrap_err();

        assert!(
            error.to_string().to_ascii_lowercase().contains("readonly"),
            "the fixture must exercise a real SQLite readonly write error: {error}"
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded
        );
    }

    #[tokio::test]
    async fn readonly_agent_run_restore_failure_degrades_runtime_before_more_effects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-runs.db");
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let memory_store = state.memory_store.lock().await.clone();
        let run = AgentRun::new_calibration_run();
        {
            let writable = openlife_core::agent::AgentRunStore::new(&path).unwrap();
            writable.bind_canonical_memory_store(&memory_store).unwrap();
            writable.create_run(&run).unwrap();
            writable
                .delete_run_with_tombstone(&run.id, Some("readonly restore fixture"))
                .unwrap();
        }
        let readonly = openlife_core::agent::AgentRunStore::open_read_only_existing(&path).unwrap();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(readonly)));
        install_release_like_persistence_coordinator(&mut state);

        let error = restore_agent_run_with_state(&run.id, &state)
            .await
            .unwrap_err();

        assert!(
            error.to_string().to_ascii_lowercase().contains("readonly"),
            "the fixture must exercise a real SQLite readonly write error: {error}"
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded
        );
    }

    #[tokio::test]
    async fn logical_create_delete_restore_conflicts_do_not_degrade_runtime() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        install_release_like_persistence_coordinator(&mut state);
        let run = AgentRun::new_calibration_run();
        crate::terminal_owner_write_gateway::create_agent_run(&state, &run)
            .await
            .unwrap();

        let duplicate = crate::terminal_owner_write_gateway::create_agent_run(&state, &run)
            .await
            .unwrap_err();
        assert!(
            duplicate.to_ascii_lowercase().contains("unique constraint"),
            "{duplicate}"
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadWrite
        );

        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            delete_agent_run_after_confirmation_with_state(
                &run.id,
                Some("logical conflict"),
                &state,
            ),
        )
        .await
        .expect("healthy AgentRun delete must not self-deadlock")
        .unwrap();
        let duplicate_delete =
            delete_agent_run_after_confirmation_with_state(&run.id, None, &state)
                .await
                .unwrap_err();
        assert!(
            duplicate_delete
                .to_string()
                .contains("canonical_agent_run_missing"),
            "{duplicate_delete}"
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadWrite
        );

        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            restore_agent_run_with_state(&run.id, &state),
        )
        .await
        .expect("healthy AgentRun restore must not self-deadlock")
        .unwrap();
        let duplicate_restore = restore_agent_run_with_state(&run.id, &state)
            .await
            .unwrap_err();
        assert!(
            duplicate_restore
                .to_string()
                .contains("agent_run_restore_requires_active_canonical_tombstone"),
            "{duplicate_restore}"
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadWrite
        );
    }

    #[tokio::test]
    async fn agent_run_preflight_read_failure_degrades_runtime_before_effects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-runs.db");
        let store = openlife_core::agent::AgentRunStore::new(&path).unwrap();
        let run = AgentRun::new_builder_run("agent-run-preflight-read-failure");
        store.create_run(&run).unwrap();

        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(store)));
        install_release_like_persistence_coordinator(&mut state);

        let fault = rusqlite::Connection::open(&path).unwrap();
        fault.execute_batch("DROP TABLE agent_runs;").unwrap();
        drop(fault);

        let error = crate::terminal_owner_write_gateway::project_agent_run_from_proposal_staging(
            &state,
            &run.id,
            &[],
            crate::terminal_owner_write_gateway::AgentRunProposalStagingReceipt {
                kind: crate::terminal_owner_write_gateway::AgentRunProposalStagingKind::Builder,
                requested_count: 0,
                failed_count: 0,
            },
        )
        .await
        .unwrap_err();

        assert!(
            error.to_ascii_lowercase().contains("no such table"),
            "{error}"
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded,
            "a canonical owner read failure must fail closed before more effects"
        );
    }

    #[tokio::test]
    async fn shipped_get_and_list_classify_durable_read_failure_without_degrading_not_found() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-run-shipped-reads.db");
        let store = openlife_core::agent::AgentRunStore::new(&path).unwrap();
        let run = AgentRun::new_builder_run("agent-run-shipped-read-failure");
        store.create_run(&run).unwrap();

        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(store)));
        install_release_like_persistence_coordinator(&mut state);

        assert!(get_agent_run_with_state("missing-logical-run", &state)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadWrite,
            "normal not-found is an Option result and must not degrade persistence"
        );

        let fault = rusqlite::Connection::open(&path).unwrap();
        fault.execute_batch("DROP TABLE agent_runs;").unwrap();
        drop(fault);

        let get_error = get_agent_run_with_state(&run.id, &state)
            .await
            .expect_err("corrupt canonical get must fail closed")
            .to_string();
        assert!(get_error.to_ascii_lowercase().contains("no such table"));
        let list_error = list_agent_runs_with_state(20, 0, &state)
            .await
            .expect_err("corrupt canonical list must fail closed")
            .to_string();
        assert!(list_error.to_ascii_lowercase().contains("no such table"));
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded
        );
        assert!(state
            .persistence_coordinator
            .admit_agent_run_write()
            .is_err());
    }

    #[tokio::test]
    async fn typed_generation_rejects_changed_terminal_runs_without_revision_or_field_write() {
        for terminal_status in [
            openlife_core::agent::AgentRunStatus::Cancelled,
            openlife_core::agent::AgentRunStatus::Completed,
            openlife_core::agent::AgentRunStatus::Failed,
        ] {
            let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
            let mut run = AgentRun::new_chat_run(
                &format!("typed-generation-terminal-{terminal_status}"),
                "metadata safe input",
            );
            match terminal_status {
                openlife_core::agent::AgentRunStatus::Cancelled => run.cancel(),
                openlife_core::agent::AgentRunStatus::Completed => {
                    run.status = openlife_core::agent::AgentRunStatus::Completed;
                    run.finished_at = Some(chrono::Utc::now());
                }
                openlife_core::agent::AgentRunStatus::Failed => {
                    run.fail(openlife_core::agent::AgentRunError {
                        message: "terminal before generation projection".into(),
                        phase: "tool_error".into(),
                        recoverable: true,
                    });
                }
                _ => unreachable!(),
            }
            let run_id = run.id.clone();
            let task_id = run.task_id.clone();
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .create_run(&run)
                .unwrap();
            install_release_like_persistence_coordinator(&mut state);
            let (before, before_revision) = {
                let store = state.agent_run_store.as_ref().unwrap().lock().await;
                (
                    serde_json::to_value(store.get_run(&run_id).unwrap().unwrap()).unwrap(),
                    store.canonical_revision(&run_id).unwrap(),
                )
            };

            let error = crate::terminal_owner_write_gateway::project_main_chat_generation_result(
                &state,
                &run_id,
                &task_id,
                &detached_test_execution_epoch(&task_id),
                typed_generation_projection_fixture(chrono::Utc::now()),
            )
            .await
            .expect_err("a changed generation delta must not resurrect a terminal AgentRun");
            assert_eq!(error, "main_chat_agent_run_terminal_delta_conflict");
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            assert_eq!(
                serde_json::to_value(store.get_run(&run_id).unwrap().unwrap()).unwrap(),
                before,
                "terminal generation conflict must not write any projected field"
            );
            assert_eq!(store.canonical_revision(&run_id).unwrap(), before_revision);
        }
    }

    #[tokio::test]
    async fn exact_completed_generation_replay_is_a_revision_preserving_noop() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let now = chrono::Utc::now();
        let run = completed_agent_run_with_generation_projection(
            "typed-generation-exact-terminal-replay",
            now,
        );
        let run_id = run.id.clone();
        let task_id = run.task_id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_release_like_persistence_coordinator(&mut state);
        let before_revision = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_revision(&run_id)
            .unwrap();

        crate::terminal_owner_write_gateway::project_main_chat_generation_result(
            &state,
            &run_id,
            &task_id,
            &detached_test_execution_epoch(&task_id),
            typed_generation_projection_fixture(now),
        )
        .await
        .expect("an exact terminal generation replay must be accepted as a no-op");

        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .canonical_revision(&run_id)
                .unwrap(),
            before_revision
        );
    }

    #[tokio::test]
    async fn typed_blocked_projection_rejects_terminal_resurrection_and_exact_replay_is_noop() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let now = chrono::Utc::now();
        let run = completed_agent_run_with_generation_projection(
            "typed-blocked-terminal-projection",
            now,
        );
        let run_id = run.id.clone();
        let task_id = run.task_id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_release_like_persistence_coordinator(&mut state);
        let before_revision = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_revision(&run_id)
            .unwrap();

        crate::terminal_owner_write_gateway::project_main_chat_kernel_evidence(
            &state,
            &run_id,
            &task_id,
            &detached_test_execution_epoch(&task_id),
            typed_blocked_projection_fixture(
                now,
                crate::terminal_owner_write_gateway::MainChatBlockedDisposition::TerminalFailurePendingDurableReceipt,
            ),
        )
        .await
        .expect("an exact terminal blocked projection replay must be a no-op");
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .canonical_revision(&run_id)
                .unwrap(),
            before_revision
        );

        let error = crate::terminal_owner_write_gateway::project_main_chat_kernel_evidence(
            &state,
            &run_id,
            &task_id,
            &detached_test_execution_epoch(&task_id),
            typed_blocked_projection_fixture(
                chrono::Utc::now(),
                crate::terminal_owner_write_gateway::MainChatBlockedDisposition::WaitingPermission,
            ),
        )
        .await
        .expect_err("a blocked delta must not move a terminal AgentRun back to waiting");
        assert_eq!(error, "main_chat_agent_run_terminal_delta_conflict");
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .canonical_revision(&run_id)
                .unwrap(),
            before_revision
        );
    }

    #[tokio::test]
    async fn cancel_before_typed_generation_commit_rejects_without_store_write() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let run = AgentRun::new_chat_run(
            "typed-generation-cancel-before-commit",
            "metadata safe input",
        );
        let run_id = run.id.clone();
        let task_id = run.task_id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_release_like_persistence_coordinator(&mut state);
        let cancellation_registry =
            crate::main_chat_cancellation::MainChatCancellationRegistry::default();
        let registration = cancellation_registry.register(&task_id);
        let execution_epoch = registration.execution_epoch();
        let before_revision = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_revision(&run_id)
            .unwrap();
        let (reached, release) =
            crate::terminal_owner_write_gateway::install_agent_run_lifecycle_commit_test_barrier(
                &run_id,
            );
        let projection_state = Arc::clone(&state);
        let projection_run_id = run_id.clone();
        let projection_task_id = task_id.clone();
        let projection_execution_epoch = execution_epoch.clone();
        let projection = tokio::spawn(async move {
            crate::terminal_owner_write_gateway::project_main_chat_generation_result(
                &projection_state,
                &projection_run_id,
                &projection_task_id,
                &projection_execution_epoch,
                typed_generation_projection_fixture(chrono::Utc::now()),
            )
            .await
        });
        reached.await.unwrap();
        cancellation_registry.request_cancel(&task_id);
        release.send(()).unwrap();

        let error = projection
            .await
            .unwrap()
            .expect_err("cancel-first must reject the physical AgentRun update");
        assert_eq!(
            error,
            "main_chat_agent_run_commit_rejected:cancel_requested"
        );
        assert!(execution_epoch.snapshot().cancel_requested);
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .canonical_revision(&run_id)
                .unwrap(),
            before_revision
        );
    }

    #[tokio::test]
    async fn typed_generation_commit_is_visible_to_execution_epoch_terminalization() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let run = AgentRun::new_chat_run("typed-generation-epoch-commit", "metadata safe input");
        let run_id = run.id.clone();
        let task_id = run.task_id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_release_like_persistence_coordinator(&mut state);
        let cancellation_registry =
            crate::main_chat_cancellation::MainChatCancellationRegistry::default();
        let registration = cancellation_registry.register(&task_id);
        let execution_epoch = registration.execution_epoch();

        crate::terminal_owner_write_gateway::project_main_chat_generation_result(
            &state,
            &run_id,
            &task_id,
            &execution_epoch,
            typed_generation_projection_fixture(chrono::Utc::now()),
        )
        .await
        .unwrap();

        let snapshot = execution_epoch.wait_for_inflight_commits().await;
        assert_eq!(snapshot.committed_fact_count(), 1);
        assert!(snapshot.commit_facts.iter().any(|fact| {
            fact.domain == "agent_run"
                && fact.object_ref == run_id
                && fact.outcome
                    == crate::main_chat_cancellation::MainChatCanonicalCommitOutcome::Committed
        }));
    }

    #[tokio::test]
    async fn typed_generation_precommit_error_closes_commit_barrier_and_preserves_error() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let run = AgentRun::new_chat_run("typed-generation-precommit-error", "metadata safe input");
        let run_id = run.id.clone();
        let task_id = run.task_id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_release_like_persistence_coordinator(&mut state);

        let mut projection = typed_generation_projection_fixture(chrono::Utc::now());
        projection.actions.clear();
        projection.observations[0].action_id = Some("missing-action".into());
        let (reached, _release) =
            crate::terminal_owner_write_gateway::install_agent_run_lifecycle_commit_test_barrier(
                &run_id,
            );
        let projection_state = Arc::clone(&state);
        let projection_run_id = run_id.clone();
        let projection_task = tokio::spawn(async move {
            crate::terminal_owner_write_gateway::project_main_chat_generation_result(
                &projection_state,
                &projection_run_id,
                &task_id,
                &detached_test_execution_epoch(&task_id),
                projection,
            )
            .await
        });

        let error = projection_task
            .await
            .expect("generation task join")
            .expect_err("orphan observation must fail before commit");
        assert_eq!(
            error,
            "main_chat_agent_run_observation_action_owner_missing"
        );
        assert!(
            reached.await.is_err(),
            "pre-commit failure must remove the barrier and close reached"
        );
    }

    #[tokio::test]
    async fn cancelled_waiting_typed_delta_cannot_remove_causal_owner_commit_barrier() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let run = AgentRun::new_chat_run(
            "typed-generation-cancelled-contender",
            "metadata safe input",
        );
        let run_id = run.id.clone();
        let task_id = run.task_id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_release_like_persistence_coordinator(&mut state);

        let causal_lock = state.persistence_coordinator.agent_run_causal_lock(&run_id);
        let causal_owner = causal_lock.lock().await;
        let (reached, release) =
            crate::terminal_owner_write_gateway::install_agent_run_lifecycle_commit_test_barrier(
                &run_id,
            );
        let contender_state = Arc::clone(&state);
        let contender_run_id = run_id.clone();
        let contender_task_id = task_id.clone();
        let contender = tokio::spawn(async move {
            crate::terminal_owner_write_gateway::project_main_chat_generation_result(
                &contender_state,
                &contender_run_id,
                &contender_task_id,
                &detached_test_execution_epoch(&contender_task_id),
                typed_generation_projection_fixture(chrono::Utc::now()),
            )
            .await
        });
        for _ in 0..100 {
            if Arc::strong_count(&causal_lock) >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(
            Arc::strong_count(&causal_lock) >= 2,
            "contending typed delta did not reach the causal lock"
        );
        contender.abort();
        assert!(contender.await.unwrap_err().is_cancelled());
        drop(causal_owner);

        let owner_state = Arc::clone(&state);
        let owner_run_id = run_id.clone();
        let owner = tokio::spawn(async move {
            crate::terminal_owner_write_gateway::project_main_chat_generation_result(
                &owner_state,
                &owner_run_id,
                &task_id,
                &detached_test_execution_epoch(&task_id),
                typed_generation_projection_fixture(chrono::Utc::now()),
            )
            .await
        });
        reached
            .await
            .expect("cancelled contender must not close the causal owner's barrier");
        release.send(()).unwrap();
        owner.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn typed_blocked_projection_preserves_tool_delta_and_waiting_truth() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let run = AgentRun::new_chat_run("typed-blocked-projection", "metadata safe input");
        let run_id = run.id.clone();
        let task_id = run.task_id.clone();
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
            store
                .add_generated_proposal(&run_id, "preexisting-proposal-link")
                .unwrap();
        }
        install_release_like_persistence_coordinator(&mut state);
        let now = chrono::Utc::now();
        crate::terminal_owner_write_gateway::project_main_chat_kernel_evidence(
            &state,
            &run_id,
            &task_id,
            &detached_test_execution_epoch(&task_id),
            crate::terminal_owner_write_gateway::MainChatBlockedProjection {
                reasoning_strategy: Some("react".into()),
                reasoning_trace: openlife_core::agent::ReasoningTrace {
                    generation_result: Some(serde_json::json!({"blocked": true})),
                    ..Default::default()
                },
                actions: vec![openlife_core::agent::AgentAction {
                    id: "blocked-action".into(),
                    action_type: "read".into(),
                    target: Some("local-resource".into()),
                    input: serde_json::json!({"metadataOnly": true}),
                    output: None,
                    status: "blocked".into(),
                    permission_decision: Some("permission_required".into()),
                    started_at: Some(now),
                    finished_at: Some(now),
                    error: Some("permission_required".into()),
                    timestamp: now,
                    tool_scope: None,
                    react_trace: None,
                    runtime_execution_receipt: None,
                }],
                observations: vec![openlife_core::agent::AgentObservation {
                    id: "blocked-observation".into(),
                    action_id: Some("blocked-action".into()),
                    content: "permission required".into(),
                    source: "typed_blocked_projection_test".into(),
                    structured_result: Some(serde_json::json!({"blocked": true})),
                    timestamp: now,
                    react_trace: None,
                }],
                step_count: 1,
                tool_call_count: 1,
                disposition:
                    crate::terminal_owner_write_gateway::MainChatBlockedDisposition::WaitingPermission,
            },
        )
        .await
        .unwrap();

        let canonical = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            canonical.status,
            openlife_core::agent::AgentRunStatus::WaitingPermission
        );
        assert!(canonical.finished_at.is_none());
        assert!(canonical.error.is_none());
        assert!(canonical.reasoning_strategy.is_some());
        assert!(canonical.reasoning_trace_digest.is_some());
        assert_eq!(canonical.actions.len(), 1);
        assert_eq!(canonical.observations.len(), 1);
        assert_eq!(canonical.step_count, 1);
        assert_eq!(canonical.tool_call_count, 1);
        assert!(canonical
            .generated_proposals
            .iter()
            .any(|proposal_id| proposal_id == "preexisting-proposal-link"));
    }

    #[tokio::test]
    async fn typed_tool_delta_rejects_same_identity_conflicts_and_orphan_observations() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let now = chrono::Utc::now();
        let mut run = AgentRun::new_chat_run("typed-tool-conflict", "metadata safe input");
        run.actions.push(openlife_core::agent::AgentAction {
            id: "conflict-action".into(),
            action_type: "read".into(),
            target: Some("canonical-target".into()),
            input: serde_json::json!({"canonical": true}),
            output: None,
            status: "completed".into(),
            permission_decision: None,
            started_at: Some(now),
            finished_at: Some(now),
            error: None,
            timestamp: now,
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: None,
        });
        run.observations
            .push(openlife_core::agent::AgentObservation {
                id: "conflict-observation".into(),
                action_id: Some("conflict-action".into()),
                content: "canonical observation".into(),
                source: "typed_tool_conflict_test".into(),
                structured_result: None,
                timestamp: now,
                react_trace: None,
            });
        let run_id = run.id.clone();
        let task_id = run.task_id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_release_like_persistence_coordinator(&mut state);

        crate::terminal_owner_write_gateway::project_main_chat_kernel_evidence(
            &state,
            &run_id,
            &task_id,
            &detached_test_execution_epoch(&task_id),
            crate::terminal_owner_write_gateway::MainChatBlockedProjection {
                reasoning_strategy: Some("react".into()),
                reasoning_trace: Default::default(),
                actions: run.actions.clone(),
                observations: run.observations.clone(),
                step_count: 1,
                tool_call_count: 1,
                disposition: crate::terminal_owner_write_gateway::MainChatBlockedDisposition::TerminalFailurePendingDurableReceipt,
            },
        )
        .await
        .expect("an exact typed tool delta replay is idempotent");

        let conflict = crate::terminal_owner_write_gateway::project_main_chat_kernel_evidence(
            &state,
            &run_id,
            &task_id,
            &detached_test_execution_epoch(&task_id),
            crate::terminal_owner_write_gateway::MainChatBlockedProjection {
                reasoning_strategy: Some("react".into()),
                reasoning_trace: Default::default(),
                actions: vec![openlife_core::agent::AgentAction {
                    id: "conflict-action".into(),
                    action_type: "read".into(),
                    target: Some("different-target".into()),
                    input: serde_json::json!({"canonical": false}),
                    output: None,
                    status: "failed".into(),
                    permission_decision: None,
                    started_at: Some(now),
                    finished_at: Some(now),
                    error: Some("different".into()),
                    timestamp: now,
                    tool_scope: None,
                    react_trace: None,
                    runtime_execution_receipt: None,
                }],
                observations: Vec::new(),
                step_count: 1,
                tool_call_count: 1,
                disposition: crate::terminal_owner_write_gateway::MainChatBlockedDisposition::TerminalFailurePendingDurableReceipt,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(conflict, "main_chat_agent_run_action_identity_conflict");

        let orphan = crate::terminal_owner_write_gateway::project_main_chat_kernel_evidence(
            &state,
            &run_id,
            &task_id,
            &detached_test_execution_epoch(&task_id),
            crate::terminal_owner_write_gateway::MainChatBlockedProjection {
                reasoning_strategy: Some("react".into()),
                reasoning_trace: Default::default(),
                actions: Vec::new(),
                observations: vec![openlife_core::agent::AgentObservation {
                    id: "orphan-observation".into(),
                    action_id: Some("missing-action".into()),
                    content: "orphan".into(),
                    source: "typed_tool_conflict_test".into(),
                    structured_result: None,
                    timestamp: now,
                    react_trace: None,
                }],
                step_count: 1,
                tool_call_count: 1,
                disposition: crate::terminal_owner_write_gateway::MainChatBlockedDisposition::TerminalFailurePendingDurableReceipt,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(
            orphan,
            "main_chat_agent_run_observation_action_owner_missing"
        );

        let invalid_supplemental =
            crate::terminal_owner_write_gateway::project_main_chat_kernel_evidence(
                &state,
                &run_id,
                &task_id,
                &detached_test_execution_epoch(&task_id),
                crate::terminal_owner_write_gateway::MainChatBlockedProjection {
                    reasoning_strategy: Some("react".into()),
                    reasoning_trace: Default::default(),
                    actions: Vec::new(),
                    observations: vec![openlife_core::agent::AgentObservation {
                        id: "invalid-supplemental".into(),
                        action_id: None,
                        content: "unbound".into(),
                        source: "untrusted-source".into(),
                        structured_result: None,
                        timestamp: now,
                        react_trace: None,
                    }],
                    step_count: 1,
                    tool_call_count: 1,
                    disposition: crate::terminal_owner_write_gateway::MainChatBlockedDisposition::TerminalFailurePendingDurableReceipt,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(
            invalid_supplemental,
            "main_chat_agent_run_supplemental_observation_contract_invalid"
        );

        let exact_duplicate_run =
            AgentRun::new_chat_run("typed-tool-exact-duplicate", "metadata safe input");
        let exact_duplicate_run_id = exact_duplicate_run.id.clone();
        let exact_duplicate_task_id = exact_duplicate_run.task_id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&exact_duplicate_run)
            .unwrap();
        let duplicate_action = run.actions[0].clone();
        let duplicate_observation = run.observations[0].clone();
        crate::terminal_owner_write_gateway::project_main_chat_kernel_evidence(
            &state,
            &exact_duplicate_run_id,
            &exact_duplicate_task_id,
            &detached_test_execution_epoch(&exact_duplicate_task_id),
            crate::terminal_owner_write_gateway::MainChatBlockedProjection {
                reasoning_strategy: Some("react".into()),
                reasoning_trace: Default::default(),
                actions: vec![duplicate_action.clone(), duplicate_action.clone()],
                observations: vec![
                    duplicate_observation.clone(),
                    duplicate_observation.clone(),
                ],
                step_count: 1,
                tool_call_count: 1,
                disposition: crate::terminal_owner_write_gateway::MainChatBlockedDisposition::TerminalFailurePendingDurableReceipt,
            },
        )
        .await
        .expect("an exact duplicate inside one typed delta must collapse idempotently");
        let exact_duplicate_canonical = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&exact_duplicate_run_id)
            .unwrap()
            .unwrap();
        assert_eq!(exact_duplicate_canonical.actions.len(), 1);
        assert_eq!(exact_duplicate_canonical.observations.len(), 1);

        let conflicting_duplicate_run =
            AgentRun::new_chat_run("typed-tool-conflicting-duplicate", "metadata safe input");
        let conflicting_duplicate_run_id = conflicting_duplicate_run.id.clone();
        let conflicting_duplicate_task_id = conflicting_duplicate_run.task_id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&conflicting_duplicate_run)
            .unwrap();
        let mut conflicting_duplicate_action = duplicate_action.clone();
        conflicting_duplicate_action.target = Some("different-target-in-same-delta".into());
        let same_delta_conflict =
            crate::terminal_owner_write_gateway::project_main_chat_kernel_evidence(
                &state,
                &conflicting_duplicate_run_id,
                &conflicting_duplicate_task_id,
                &detached_test_execution_epoch(&conflicting_duplicate_task_id),
                crate::terminal_owner_write_gateway::MainChatBlockedProjection {
                    reasoning_strategy: Some("react".into()),
                    reasoning_trace: Default::default(),
                    actions: vec![duplicate_action, conflicting_duplicate_action],
                    observations: Vec::new(),
                    step_count: 1,
                    tool_call_count: 1,
                    disposition: crate::terminal_owner_write_gateway::MainChatBlockedDisposition::TerminalFailurePendingDurableReceipt,
                },
            )
            .await
            .expect_err("same identity with a different body in one delta must fail closed");
        assert_eq!(
            same_delta_conflict,
            "main_chat_agent_run_action_identity_conflict"
        );
    }

    #[tokio::test]
    async fn canonical_reload_tool_delta_replay_is_noop_and_changed_body_conflicts() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let now = chrono::Utc::now();
        let mut run = AgentRun::new_chat_run("canonical-reload-tool-replay", "metadata safe input");
        run.reasoning_strategy = Some("react".into());
        run.reasoning_trace = Some(Default::default());
        run.step_count = 1;
        run.tool_call_count = 1;
        run.actions.push(openlife_core::agent::AgentAction {
            // Deliberately not an authoritative `action-*` reference. The
            // store must replace this producer identity with its HMAC receipt.
            id: "producer-action".into(),
            action_type: "read".into(),
            target: Some("canonical-target".into()),
            input: serde_json::json!({"metadataOnly": true}),
            output: None,
            status: "completed".into(),
            permission_decision: None,
            started_at: Some(now),
            finished_at: Some(now),
            error: None,
            timestamp: now,
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: None,
        });
        run.observations
            .push(openlife_core::agent::AgentObservation {
                id: "producer-observation".into(),
                action_id: Some("producer-action".into()),
                content: "canonical observation".into(),
                source: "canonical_reload_replay_test".into(),
                structured_result: Some(serde_json::json!({"metadataOnly": true})),
                timestamp: now,
                react_trace: None,
            });
        let run_id = run.id.clone();
        let task_id = run.task_id.clone();
        let canonical = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
            store.get_run(&run_id).unwrap().unwrap()
        };
        assert_ne!(canonical.actions[0].id, "producer-action");
        assert_ne!(canonical.observations[0].id, "producer-observation");
        assert_eq!(
            canonical.observations[0].action_id.as_deref(),
            Some(canonical.actions[0].id.as_str()),
            "reload must preserve the canonical action-observation owner graph"
        );
        install_release_like_persistence_coordinator(&mut state);
        let revision_before = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_revision(&run_id)
            .unwrap();

        crate::terminal_owner_write_gateway::project_main_chat_kernel_evidence(
            &state,
            &run_id,
            &task_id,
            &detached_test_execution_epoch(&task_id),
            crate::terminal_owner_write_gateway::MainChatBlockedProjection {
                reasoning_strategy: canonical.reasoning_strategy.clone(),
                reasoning_trace: Default::default(),
                actions: canonical.actions.clone(),
                observations: canonical.observations.clone(),
                step_count: canonical.step_count,
                tool_call_count: canonical.tool_call_count,
                disposition: crate::terminal_owner_write_gateway::MainChatBlockedDisposition::TerminalFailurePendingDurableReceipt,
            },
        )
        .await
        .expect("an exact create-reload typed replay must be a canonical no-op");
        let revision_after_replay = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_revision(&run_id)
            .unwrap();
        assert_eq!(revision_after_replay, revision_before);

        let mut changed_action = canonical.actions[0].clone();
        changed_action.target = Some("different-target".into());
        let conflict = crate::terminal_owner_write_gateway::project_main_chat_kernel_evidence(
            &state,
            &run_id,
            &task_id,
            &detached_test_execution_epoch(&task_id),
            crate::terminal_owner_write_gateway::MainChatBlockedProjection {
                reasoning_strategy: canonical.reasoning_strategy,
                reasoning_trace: Default::default(),
                actions: vec![changed_action],
                observations: Vec::new(),
                step_count: canonical.step_count,
                tool_call_count: canonical.tool_call_count,
                disposition: crate::terminal_owner_write_gateway::MainChatBlockedDisposition::TerminalFailurePendingDurableReceipt,
            },
        )
        .await
        .expect_err("the same canonical identity with a changed body must fail closed");
        assert_eq!(conflict, "main_chat_agent_run_action_identity_conflict");
        let revision_after_conflict = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_revision(&run_id)
            .unwrap();
        assert_eq!(revision_after_conflict, revision_before);
    }

    #[tokio::test]
    async fn immutable_agent_run_evidence_conflict_does_not_degrade_runtime() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut run = AgentRun::new_builder_run("immutable-evidence-conflict");
        run.reasoning_trace = Some(openlife_core::agent::ReasoningTrace {
            strategy_result: Some(serde_json::json!({"fixture": "original"})),
            ..openlife_core::agent::ReasoningTrace::default()
        });
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        install_release_like_persistence_coordinator(&mut state);
        let mut conflicting = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run.id)
            .unwrap()
            .unwrap();
        conflicting.reasoning_trace = Some(openlife_core::agent::ReasoningTrace {
            strategy_result: Some(serde_json::json!({"fixture": "counterfactual"})),
            ..openlife_core::agent::ReasoningTrace::default()
        });
        conflicting.reasoning_trace_digest = None;

        let error =
            crate::terminal_owner_write_gateway::replace_agent_run_for_test(&state, &conflicting)
                .await
                .unwrap_err();

        assert!(error.contains("agent_run_immutable_evidence_update_conflict"));
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadWrite
        );
        state
            .persistence_coordinator
            .require_effects_allowed()
            .expect("validation conflict cannot force global safe mode");
    }

    #[tokio::test]
    async fn degradation_after_startup_agent_run_update_admission_wins_before_owner_commit() {
        let (state, run, task) = startup_task_owner_fixture(
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission,
        )
        .await;
        let (reached, release) =
            crate::terminal_owner_write_gateway::install_agent_run_lifecycle_commit_test_barrier(
                &run.id,
            );
        let operation_state = Arc::clone(&state);
        let task_id = task.id.clone();
        let operation_task_id = task_id.clone();
        let operation = tokio::spawn(async move {
            crate::terminal_owner_write_gateway::project_agent_run_from_startup_task_owner(
                &operation_state,
                &run.id,
                &operation_task_id,
            )
            .await
        });
        reached
            .await
            .expect("startup update reached commit barrier");
        state
            .persistence_coordinator
            .degrade_globally("injected_after_startup_agent_run_update_admission");
        release.send(()).unwrap();

        let error = operation.await.unwrap().unwrap_err();
        assert!(error.contains(crate::persistence_coordinator::PERSISTENCE_ADMISSION_INVALIDATED));
        let stored = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run_for_task_id(&task_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, openlife_core::agent::AgentRunStatus::Running);
    }

    #[tokio::test]
    async fn startup_task_owner_rejects_terminal_agent_run_projection_without_durable_event() {
        use openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus;

        for status in [
            AgentTaskSessionStatus::Completed,
            AgentTaskSessionStatus::Cancelled,
            AgentTaskSessionStatus::Blocked,
            AgentTaskSessionStatus::Failed,
        ] {
            let (state, run, task) = startup_task_owner_fixture(status).await;
            let error =
                crate::terminal_owner_write_gateway::project_agent_run_from_startup_task_owner(
                    &state, &run.id, &task.id,
                )
                .await
                .unwrap_err();
            assert_eq!(
                error,
                "startup_agent_run_task_owner_requires_waiting_permission"
            );
            let stored = state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_run(&run.id)
                .unwrap()
                .unwrap();
            assert_eq!(stored.status, openlife_core::agent::AgentRunStatus::Running);
            assert!(stored.finished_at.is_none());
        }
    }

    #[tokio::test]
    async fn startup_interrupted_final_delivery_projects_failed_with_event_time() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let (run, session, generation) = create_open_terminal_owner_fixture(&state).await;
        let final_event =
            append_terminal_final_fixture(&state, &session, &run, generation, "interrupted").await;
        install_startup_like_persistence_coordinator(&mut state);

        crate::terminal_owner_write_gateway::project_agent_run_from_startup_durable_event(
            &state,
            &final_event,
        )
        .await
        .unwrap();
        let stored = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run.id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, openlife_core::agent::AgentRunStatus::Failed);
        assert_eq!(stored.finished_at, Some(final_event.created_at));
    }

    #[tokio::test]
    async fn startup_durable_event_rejects_stale_terminal_before_newer_final_head() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let (run, session, generation) = create_open_terminal_owner_fixture(&state).await;
        let stale = crate::terminal_owner_write_gateway::append_runtime_event(
            &state,
            &session.id,
            &run.id,
            "failed",
            "turn",
            format!("stale-terminal:{}", run.id),
            "test.stale_terminal",
            serde_json::json!({
                "status": "failed",
                "kind": "unknown_error",
                "errorDigest": format!("sha256:{}", "a".repeat(64)),
                "durableCommitAllowedAfterFailure": false,
            }),
        )
        .await
        .unwrap();
        let latest =
            append_terminal_final_fixture(&state, &session, &run, generation, "completed").await;
        assert!(latest.sequence > stale.sequence);
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            let mut completed = store.get_run(&run.id).unwrap().unwrap();
            completed.status = openlife_core::agent::AgentRunStatus::Completed;
            completed.finished_at = Some(latest.created_at);
            store.update_run(&completed).unwrap();
        }
        install_startup_like_persistence_coordinator(&mut state);

        let error =
            crate::terminal_owner_write_gateway::project_agent_run_from_startup_durable_event(
                &state, &stale,
            )
            .await
            .unwrap_err();
        assert_eq!(error, "startup_agent_run_durable_evidence_stale");
        let stored = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            stored.status,
            openlife_core::agent::AgentRunStatus::Completed
        );
        assert_eq!(stored.finished_at, Some(latest.created_at));
    }

    #[tokio::test]
    async fn degradation_after_generic_agent_run_create_admission_wins_before_commit() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        install_release_like_persistence_coordinator(&mut state);
        let run = AgentRun::new_builder_run("late-degraded-generic-create");
        let (reached, release) =
            crate::terminal_owner_write_gateway::install_agent_run_lifecycle_commit_test_barrier(
                &run.id,
            );

        let operation_state = Arc::clone(&state);
        let operation_run = run.clone();
        let operation = tokio::spawn(async move {
            crate::terminal_owner_write_gateway::create_agent_run(&operation_state, &operation_run)
                .await
        });
        reached
            .await
            .expect("generic create reached its admitted pre-transaction barrier");
        state
            .persistence_coordinator
            .degrade_globally("injected_after_generic_agent_run_create_admission");
        release.send(()).unwrap();

        let error = operation.await.unwrap().unwrap_err();
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        assert!(store.get_run_including_deleted(&run.id).unwrap().is_none());
        assert!(store
            .list_replayable_projection_deliveries(20)
            .unwrap()
            .is_empty());
        assert!(
            error.contains(crate::persistence_coordinator::PERSISTENCE_ADMISSION_INVALIDATED),
            "late degradation returned the wrong generic-create error: {error}"
        );
    }

    #[tokio::test]
    async fn degradation_after_conversation_bound_agent_run_create_admission_wins_before_commit() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        install_release_like_persistence_coordinator(&mut state);
        let operation_id = uuid::Uuid::new_v4().to_string();
        let session_id = format!("late-degraded-conversation-create:{operation_id}");
        let message_commit = {
            let memory = state.memory_store.lock().await;
            memory
                .create_chat_session(&session_id, "Late create")
                .unwrap();
            memory
                .save_message_idempotent_with_proof(
                    &session_id,
                    &openlife_core::llm::ChatMessage {
                        role: "user".into(),
                        content: "Do not persist the run after degradation".into(),
                    },
                    &operation_id,
                )
                .unwrap()
        };
        let mut run =
            AgentRun::new_chat_run(&session_id, "Do not persist the run after degradation");
        run.id = operation_id.clone();
        run.task_id = operation_id;
        run.input_ref = Some(message_commit.receipt().canonical_ref.clone());
        let (reached, release) =
            crate::terminal_owner_write_gateway::install_agent_run_lifecycle_commit_test_barrier(
                &run.id,
            );

        let operation_state = Arc::clone(&state);
        let operation_run = run.clone();
        let operation = tokio::spawn(async move {
            crate::terminal_owner_write_gateway::create_conversation_bound_agent_run(
                &operation_state,
                &operation_run,
                &message_commit,
            )
            .await
        });
        reached
            .await
            .expect("conversation-bound create reached its admitted pre-transaction barrier");
        state
            .persistence_coordinator
            .degrade_globally("injected_after_conversation_agent_run_create_admission");
        release.send(()).unwrap();

        let error = operation.await.unwrap().unwrap_err();
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        assert!(store.get_run_including_deleted(&run.id).unwrap().is_none());
        assert!(store
            .list_replayable_projection_deliveries(20)
            .unwrap()
            .is_empty());
        assert!(
            error.contains(crate::persistence_coordinator::PERSISTENCE_ADMISSION_INVALIDATED),
            "late degradation returned the wrong conversation-create error: {error}"
        );
    }
}
