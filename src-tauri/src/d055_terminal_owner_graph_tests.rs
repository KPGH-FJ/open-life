use std::sync::Arc;
use std::time::Duration;

use openlife_core::agent::{
    AgentProposal, ProposalSource, ProposalStatus, ProposalType, RiskLevel,
};
use openlife_core::life_model::patch::PatchStatus;
use openlife_core::llm::ChatMessage;
use serde::Deserialize;

const TERMINAL_SEAL_DEFERRED_ERROR_CODE: &str =
    "proposal_accept_deferred:origin_turn_terminal_sealing";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TerminalSealAcceptanceDispositionV1 {
    DeferredWhileOriginTurnSealing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TerminalSealAcceptanceReasonV1 {
    OriginTurnTerminalSealing,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalSealDeferredContractV1 {
    schema_version: String,
    disposition: TerminalSealAcceptanceDispositionV1,
    reason_code: TerminalSealAcceptanceReasonV1,
    retryable_after_seal: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalSealDeferredAcceptResponseV1 {
    success: bool,
    #[serde(alias = "effect_status")]
    effect_status: String,
    #[serde(alias = "proposal_projection_status")]
    proposal_projection_status: String,
    #[serde(alias = "blocked_action")]
    blocked_action: TerminalSealDeferredContractV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservedTerminalSealDisposition {
    TypedPendingResponse,
    StableDeferredErrorCode,
}

fn observe_terminal_seal_deferred_contract(
    result: &Result<serde_json::Value, String>,
) -> Result<ObservedTerminalSealDisposition, String> {
    match result {
        Err(error) if error == TERMINAL_SEAL_DEFERRED_ERROR_CODE => {
            Ok(ObservedTerminalSealDisposition::StableDeferredErrorCode)
        }
        Err(error) => Err(format!(
            "unexpected Proposal accept error while the origin turn was sealing: {error}"
        )),
        Ok(value) => {
            let response = serde_json::from_value::<TerminalSealDeferredAcceptResponseV1>(
                value.clone(),
            )
            .map_err(|error| {
                format!(
                    "Proposal accept did not return the typed terminal-seal pending contract: {error}"
                )
            })?;
            let expected_contract = TerminalSealDeferredContractV1 {
                schema_version: "terminal_seal_acceptance_v1".into(),
                disposition: TerminalSealAcceptanceDispositionV1::DeferredWhileOriginTurnSealing,
                reason_code: TerminalSealAcceptanceReasonV1::OriginTurnTerminalSealing,
                retryable_after_seal: true,
            };
            if response.success
                || response.effect_status != "not_dispatched"
                || response.proposal_projection_status != "pending"
                || response.blocked_action != expected_contract
            {
                return Err(format!(
                    "typed terminal-seal response claimed the wrong truth: {response:?}"
                ));
            }
            Ok(ObservedTerminalSealDisposition::TypedPendingResponse)
        }
    }
}

async fn create_linked_lifemodel_proposal(
    state: &Arc<crate::AppState>,
    operation_id: &str,
    proposed_focus: &str,
) -> (String, String) {
    let previous_focus = state
        .life_model_manager
        .lock()
        .await
        .load()
        .expect("load canonical LifeModel before D055 proposal")
        .state
        .current_focus;
    let mut proposal = AgentProposal::new(
        ProposalType::LifeModelUpdate,
        "state.current_focus",
        serde_json::json!(proposed_focus),
        "D055 legacy root-reproduction fixture linked to one Main Chat operation",
        1.0,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    // Legacy root reproduction only: current task synchronization parses this
    // free-text field and thereby exposes the owner-drift race. The target
    // terminal admission authority must ignore it; the separate Core contract
    // RED requires a typed immutable ProposalStore origin binding instead.
    proposal.source_detail = Some(format!("main_chat_agent_task_session:{operation_id}"));
    crate::life_model_write_gateway::stamp_lifemodel_proposal_base_hash_with_state(
        state,
        &mut proposal,
    )
    .await
    .expect("stamp proposal against the real canonical LifeModel owner");
    let proposal_id = proposal.id.clone();
    state
        .proposal_store
        .as_ref()
        .expect("D055 requires the real ProposalStore")
        .lock()
        .await
        .create_proposal(&proposal)
        .expect("persist the pending Proposal fixture");
    (proposal_id, previous_focus)
}

async fn set_linked_proposal_blocker(
    state: &Arc<crate::AppState>,
    operation_id: &str,
    proposal_id: &str,
) {
    state
        .main_chat_agent_session_store
        .as_ref()
        .expect("D055 requires the real TaskSession store")
        .lock()
        .await
        .set_pending_blockers(operation_id, vec![format!("proposal:{proposal_id}")])
        .expect("link the real pending Proposal to the canonical task before final");
}

async fn stored_proposal_status(state: &Arc<crate::AppState>, proposal_id: &str) -> ProposalStatus {
    state
        .proposal_store
        .as_ref()
        .expect("D055 requires the real ProposalStore")
        .lock()
        .await
        .get_proposal(proposal_id)
        .expect("load the real Proposal")
        .expect("linked Proposal remains present")
        .status
}

async fn stored_proposal_dispatch_state(
    state: &Arc<crate::AppState>,
    proposal_id: &str,
) -> Option<String> {
    state
        .proposal_store
        .as_ref()
        .expect("D055 requires the real ProposalStore")
        .lock()
        .await
        .dispatch_state(proposal_id)
        .expect("load the real Proposal dispatch state")
}

async fn applied_patch_count_for_proposal(
    state: &Arc<crate::AppState>,
    proposal_id: &str,
) -> usize {
    state
        .patch_store
        .as_ref()
        .expect("D055 requires the real PatchStore")
        .lock()
        .await
        .list_patches_by_proposal(proposal_id)
        .expect("list canonical patches for the exact Proposal")
        .into_iter()
        .filter(|patch| patch.status == PatchStatus::Applied)
        .count()
}

async fn canonical_focus(state: &Arc<crate::AppState>) -> String {
    state
        .life_model_manager
        .lock()
        .await
        .load()
        .expect("load the real canonical LifeModel")
        .state
        .current_focus
}

// Legacy root reproduction only. This test preserves the observed race but
// cannot earn D055 GREEN because its fixture intentionally uses source_detail.
// The independently compiled test-utils contract is the target behavior gate.
#[tokio::test]
async fn legacy_source_detail_root_reproduction_accept_during_final_seal_commits_stale_owner_graph()
{
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let captured_requests = crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_captured_local_http_provider(
        &state,
        "D055 final-seal provider reply",
    )
    .await;
    let operation_id = uuid::Uuid::new_v4().to_string();
    let session_id = "d055-proposal-races-final-seal";
    let body = "Give one concise focus tip for this afternoon.";
    let proposed_focus = "D055 focus must not commit while origin turn is sealing";
    let (proposal_id, focus_before) =
        create_linked_lifemodel_proposal(&state, &operation_id, proposed_focus).await;
    let (_pre_guard, pre_reached, pre_release, _kernel_poll_count) =
        crate::main_chat_turn_runtime::install_main_chat_pre_registration_barrier_for_test(
            session_id,
        );
    let (_final_guard, final_reached, final_release) =
        crate::main_chat_turn_runtime::install_main_chat_final_owner_snapshot_barrier_for_test(
            &operation_id,
        );

    let turn_state = Arc::clone(&state);
    let turn_operation = operation_id.clone();
    let turn = tokio::spawn(async move {
        crate::main_chat_send::send_message_with_operation_state(
            turn_operation,
            session_id.into(),
            vec![ChatMessage {
                role: "user".into(),
                content: body.into(),
            }],
            None,
            &turn_state,
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(5), pre_reached.wait())
        .await
        .expect("turn reaches the real task-created boundary");
    set_linked_proposal_blocker(&state, &operation_id, &proposal_id).await;
    pre_release.wait().await;
    tokio::time::timeout(Duration::from_secs(10), final_reached.wait())
        .await
        .expect("turn reaches the final owner snapshot after all real owner reads");

    // The fixture was inserted directly into ProposalStore. This call is the
    // real product acceptance path: it enters the ReviewWorkflow acceptance
    // snapshot and LifeModelWriteGateway before any canonical effect.
    let accept_while_sealing = tokio::time::timeout(
        Duration::from_secs(10),
        crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &state),
    )
    .await;
    let status_while_sealing = stored_proposal_status(&state, &proposal_id).await;
    let dispatch_while_sealing = stored_proposal_dispatch_state(&state, &proposal_id).await;
    let focus_while_sealing = canonical_focus(&state).await;
    let applied_patches_while_sealing =
        applied_patch_count_for_proposal(&state, &proposal_id).await;

    // Release the product turn before any RED assertion. This makes a failure
    // incapable of leaving the spawned TurnRuntime parked on the test barrier.
    final_release.wait().await;
    let first_result = tokio::time::timeout(Duration::from_secs(10), turn)
        .await
        .expect("TurnRuntime leaves the final snapshot barrier")
        .expect("spawned TurnRuntime task joins");

    let deferred_contract = accept_while_sealing
        .as_ref()
        .map_err(|_| {
            "Proposal accept timed out instead of returning a terminal-seal disposition".to_string()
        })
        .and_then(observe_terminal_seal_deferred_contract);

    let retry_after_seal = tokio::time::timeout(
        Duration::from_secs(10),
        crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &state),
    )
    .await
    .unwrap_or_else(|_| Err("d055_accept_retry_timed_out_after_origin_sealed".into()));
    let status_after_retry = stored_proposal_status(&state, &proposal_id).await;
    let dispatch_after_retry = stored_proposal_dispatch_state(&state, &proposal_id).await;
    let focus_after_retry = canonical_focus(&state).await;
    let applied_patches_after_retry = applied_patch_count_for_proposal(&state, &proposal_id).await;

    let recovery_result = tokio::time::timeout(
        Duration::from_secs(10),
        crate::main_chat_send::send_message_with_operation_state(
            operation_id.clone(),
            session_id.into(),
            vec![ChatMessage {
                role: "user".into(),
                content: body.into(),
            }],
            None,
            &state,
        ),
    )
    .await
    .unwrap_or_else(|_| Err("d055_recovery_timed_out_after_final_seal_race".into()));
    let events = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("D055 requires the real TurnEventStore")
        .lock()
        .await
        .list(&operation_id, 0, 250)
        .expect("list real durable turn events");
    let final_count = events
        .iter()
        .filter(|event| event.event_type == "final_delivery.created")
        .count();
    let provider_event_count = events
        .iter()
        .filter(|event| event.event_type == "provider.completed")
        .count();
    let provider_count = captured_requests.lock().unwrap().len();

    assert!(
        deferred_contract.is_ok(),
        "D055 RED: sealing admission returned neither the frozen typed pending response nor the exact stable deferred reason code: {deferred_contract:?}; raw_accept={accept_while_sealing:?}; status_while_sealing={status_while_sealing:?}; dispatch_while_sealing={dispatch_while_sealing:?}; focus_changed_while_sealing={}; applied_patches_while_sealing={applied_patches_while_sealing}; retry_after_seal={retry_after_seal:?}; status_after_retry={status_after_retry:?}; dispatch_after_retry={dispatch_after_retry:?}; applied_patches_after_retry={applied_patches_after_retry}; recovery_result={recovery_result:?}; provider_requests={provider_count}; provider_events={provider_event_count}; final_events={final_count}",
        focus_while_sealing != focus_before,
    );
    assert_eq!(status_while_sealing, ProposalStatus::Pending);
    assert_eq!(dispatch_while_sealing.as_deref(), Some("unclaimed"));
    assert_eq!(focus_while_sealing, focus_before);
    assert_eq!(applied_patches_while_sealing, 0);
    assert!(first_result.is_ok(), "origin turn failed: {first_result:?}");
    assert!(
        retry_after_seal
            .as_ref()
            .is_ok_and(|value| value.get("success").and_then(serde_json::Value::as_bool) == Some(true)),
        "the exact deferred Proposal must remain retryable after the origin final is sealed: {retry_after_seal:?}"
    );
    assert_eq!(status_after_retry, ProposalStatus::Accepted);
    assert_eq!(dispatch_after_retry.as_deref(), Some("confirmed"));
    assert_eq!(focus_after_retry, proposed_focus);
    assert_eq!(
        applied_patches_after_retry, 1,
        "the deferred-then-retried Proposal effect must materialize exactly once"
    );
    assert!(
        recovery_result.is_ok(),
        "the sealed final plus its authorized successor must recover: {recovery_result:?}"
    );
    assert_eq!(provider_count, 1);
    assert_eq!(provider_event_count, 1);
    assert_eq!(final_count, 1);
}

// Same legacy fixture boundary as the race reproduction above: useful RED
// evidence for the existing drift, never typed-origin completion credit.
#[tokio::test]
async fn legacy_source_detail_root_reproduction_post_terminal_accept_breaks_historical_recovery() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let captured_requests = crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_captured_local_http_provider(
        &state,
        "D055 post-final successor provider reply",
    )
    .await;
    let operation_id = uuid::Uuid::new_v4().to_string();
    let session_id = "d055-proven-post-final-successor";
    // This exact ordinary-answer shape already has provider-backed recovery
    // coverage elsewhere. Avoid tool/plan keywords so this test isolates the
    // owner-successor contract rather than a separate tool receipt contract.
    let body = "Explain one practical way to stay focused this afternoon.";
    let proposed_focus = "D055 accepted post-final focus";
    let (proposal_id, _focus_before) =
        create_linked_lifemodel_proposal(&state, &operation_id, proposed_focus).await;
    let (_pre_guard, pre_reached, pre_release, _kernel_poll_count) =
        crate::main_chat_turn_runtime::install_main_chat_pre_registration_barrier_for_test(
            session_id,
        );

    let turn_state = Arc::clone(&state);
    let turn_operation = operation_id.clone();
    let turn = tokio::spawn(async move {
        crate::main_chat_send::send_message_with_operation_state(
            turn_operation,
            session_id.into(),
            vec![ChatMessage {
                role: "user".into(),
                content: body.into(),
            }],
            None,
            &turn_state,
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), pre_reached.wait())
        .await
        .expect("turn reaches the real task-created boundary");
    set_linked_proposal_blocker(&state, &operation_id, &proposal_id).await;
    pre_release.wait().await;
    let first_result = tokio::time::timeout(Duration::from_secs(10), turn)
        .await
        .expect("initial turn completes")
        .expect("spawned initial turn joins");
    assert!(
        first_result.is_ok(),
        "initial real turn failed: {first_result:?}"
    );

    let before_accept_events = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("D055 requires the real TurnEventStore")
        .lock()
        .await
        .list(&operation_id, 0, 250)
        .expect("list final before proposal accept");
    let _final_event = before_accept_events
        .iter()
        .find(|event| event.event_type == "final_delivery.created")
        .expect("one real durable final exists before the legal successor");

    // The fixture was inserted directly into ProposalStore. Acceptance itself
    // crosses the real ReviewWorkflow snapshot and LifeModelWriteGateway.
    let accept_result = tokio::time::timeout(
        Duration::from_secs(10),
        crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &state),
    )
    .await
    .unwrap_or_else(|_| Err("d055_post_final_successor_accept_timed_out".into()));
    let proposal_status = stored_proposal_status(&state, &proposal_id).await;
    let focus_after_accept = canonical_focus(&state).await;

    let recovery_result = tokio::time::timeout(
        Duration::from_secs(10),
        crate::main_chat_send::send_message_with_operation_state(
            operation_id.clone(),
            session_id.into(),
            vec![ChatMessage {
                role: "user".into(),
                content: body.into(),
            }],
            None,
            &state,
        ),
    )
    .await
    .unwrap_or_else(|_| Err("d055_post_final_successor_recovery_timed_out".into()));
    let after_events = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("D055 requires the real TurnEventStore")
        .lock()
        .await
        .list(&operation_id, 0, 250)
        .expect("list durable facts after legal successor");
    let applied_patch_count = applied_patch_count_for_proposal(&state, &proposal_id).await;
    // This is a local capture boundary used only to disprove redispatch. It is
    // not external live-provider credit.
    let provider_count = captured_requests.lock().unwrap().len();
    let final_count = after_events
        .iter()
        .filter(|event| event.event_type == "final_delivery.created")
        .count();
    assert!(accept_result.is_ok(), "accept failed: {accept_result:?}");
    assert_eq!(proposal_status, ProposalStatus::Accepted);
    assert_eq!(focus_after_accept, proposed_focus);
    assert_eq!(applied_patch_count, 1);
    assert!(
        recovery_result.is_ok(),
        "D055 RED: a legal post-final ReviewWorkflow effect must preserve historical final recovery without whitelisting unrelated drift: {recovery_result:?}"
    );
    assert_eq!(provider_count, 1);
    assert_eq!(final_count, 1);
}
