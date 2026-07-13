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
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .get_live_run(run_id)
            .map(|run| run.map(project_verified_agent_run))
            .map_err(AppError::from)
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn list_agent_runs(
    limit: i64,
    offset: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProductAgentRun>, AppError> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .list_runs(limit, offset)
            .map(|runs| runs.into_iter().map(project_verified_agent_run).collect())
            .map_err(AppError::from)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn list_provider_transmission_history(
    limit: Option<i64>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProviderTransmissionHistoryItem>, AppError> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let runs = store.list_runs(limit, 0).map_err(AppError::from)?;
        drop(store);
        provider_transmission_history_from_runs_with_state(state.inner(), &runs)
            .await
            .map_err(AppError::internal)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn list_agent_runs_for_session(
    session_id: String,
    limit: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProductAgentRun>, AppError> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .list_runs_for_session(&session_id, limit)
            .map(|runs| runs.into_iter().map(project_verified_agent_run).collect())
            .map_err(AppError::from)
    } else {
        Ok(vec![])
    }
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
            preflight_scope_arguments: None,
            arguments: &confirmation_arguments,
            arguments_summary:
                "删除 AgentRun 运行记录并写入删除原因；批量范围中的每个目标使用独立 single-use grant。",
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

async fn delete_agent_run_after_confirmation_with_state(
    run_id: &str,
    reason: Option<&str>,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    // Confirmation can await user input while persistence health degrades.
    // Re-check immediately before the canonical transaction so a stale grant
    // can never bypass global read-only admission.
    require_agent_run_effects_allowed(state)?;
    let causal_lock = state.persistence_coordinator.agent_run_causal_lock(run_id);
    let causal_guard = causal_lock.lock().await;
    let receipt = if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        require_agent_run_effects_allowed(state)?;
        store
            .delete_run_with_tombstone(run_id, reason)
            .map_err(AppError::from)?
    } else {
        return Err(AppError::internal("AgentRun store not available"));
    };
    drop(causal_guard);
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
    require_agent_run_effects_allowed(state)?;
    let causal_lock = state.persistence_coordinator.agent_run_causal_lock(run_id);
    let causal_guard = causal_lock.lock().await;
    // 1. Restore the run in store
    let restore_event_id;
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        require_agent_run_effects_allowed(state)?;
        restore_event_id = store
            .restore_run_with_receipt(run_id)
            .map_err(AppError::from)?
            .event_id;
    } else {
        return Err(AppError::internal("AgentRun store not available"));
    }
    drop(causal_guard);
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
        store
            .get_live_run(run_id)
            .map_err(AppError::from)?
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
    let summary = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| AppError::internal("AgentRun store not available"))?
        .lock()
        .await
        .projection_summary(event_id)
        .map_err(AppError::from)?;
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
        store.create_run(&run).unwrap();

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
}
