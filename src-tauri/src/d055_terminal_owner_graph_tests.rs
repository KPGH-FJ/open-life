use std::sync::Arc;
use std::time::Duration;

use openlife_core::agent::main_chat_agent_v1::{
    ActionQueueStore, AgentTaskSessionStore, ExecutionTranscriptEntryDraft,
};
use openlife_core::agent::{
    AgentProposal, AgentRunStore, DurableWriteRequest, DurableWriteSource, DurableWriteSubject,
    MemoryLifecycleStore, ProposalSource, ProposalStatus, ProposalType, ReviewWorkflow, RiskLevel,
};
use openlife_core::llm::ChatMessage;
use openlife_core::memory::MemoryStore;

const SCENARIO_TIMEOUT: Duration = Duration::from_secs(45);
const STEP_TIMEOUT: Duration = Duration::from_secs(10);
const FINAL_EVENT_TYPE: &str = "final_delivery.created";
const PROVIDER_COMPLETED_EVENT_TYPE: &str = "provider.completed";
const SUCCESSOR_CONFIRMED_EVENT_TYPE: &str = "terminal_owner.successor_confirmed";

async fn stage_forged_unbound_memory_proposal(
    state: &Arc<crate::AppState>,
    operation_id: &str,
) -> String {
    let mut proposal = AgentProposal::new(
        ProposalType::MemoryWrite,
        "memory.pending.chat_conversation",
        serde_json::json!({
            "content": "D055 forged-origin memory body",
            "scope": "global",
            "category": "fact",
            "riskLevel": "low",
            "sensitivity": "internal",
            "candidateKind": "semantic_user_fact",
            "source": "chat_explicit",
            // These are deliberately attacker/caller-shaped legacy fields.
            // They are payload, never terminal-owner authority.
            "originatingTaskSessionId": operation_id,
            "originating_task_session_id": operation_id,
            "operationId": operation_id,
            "operation_id": operation_id,
            "epochGeneration": 999,
            "epoch_generation": 999,
        }),
        "D055 forged-origin Proposal remains pending until real ReviewWorkflow acceptance.",
        1.0,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    // Keep this independent review item unbound. A same-ID `run_id` has
    // legitimate AgentRun projection semantics and would introduce a second
    // variable unrelated to the legacy free-text origin forgery under test.
    proposal.run_id = None;
    proposal.source_detail = Some(format!(
        "main_chat_agent_task_session:{operation_id};operation:{operation_id};epoch:999"
    ));
    let proposal_id = proposal.id.clone();
    let store = state
        .proposal_store
        .as_ref()
        .expect("D055 requires the real ProposalStore")
        .lock()
        .await;
    let outcome = ReviewWorkflow::new(&store)
        .submit(DurableWriteRequest::from_agent_proposal(
            DurableWriteSource::MainChat,
            DurableWriteSubject::Memory,
            proposal,
            "Forged origin metadata does not bypass pending Review Center approval.",
        ))
        .expect("stage the counterfactual through the production ReviewWorkflow");
    assert_eq!(outcome.proposal.id, proposal_id);
    assert_eq!(outcome.proposal.status, ProposalStatus::Pending);
    proposal_id
}

async fn set_test_proposal_blocker(
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
        .expect("install the test-only owner-drift trigger before terminal snapshot");
}

async fn task_owner_digest(state: &Arc<crate::AppState>, operation_id: &str) -> String {
    let store = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("D055 requires the real TaskSession store")
        .lock()
        .await;
    let receipt = store
        .canonical_owner_receipt(operation_id)
        .expect("read the canonical TaskSession owner receipt")
        .expect("canonical TaskSession owner exists");
    // `receipt.version()` is the canonical receipt schema version. It is not
    // an owner revision and must never be used as successor-CAS evidence.
    receipt.digest().to_string()
}

async fn proposal_truth(
    state: &Arc<crate::AppState>,
    proposal_id: &str,
) -> (ProposalStatus, String) {
    let store = state
        .proposal_store
        .as_ref()
        .expect("D055 requires the real ProposalStore")
        .lock()
        .await;
    let proposal = store
        .get_proposal(proposal_id)
        .expect("read Proposal")
        .expect("Proposal remains present");
    let dispatch = store
        .dispatch_state(proposal_id)
        .expect("read Proposal dispatch state")
        .unwrap_or_else(|| "missing".into());
    (proposal.status, dispatch)
}

async fn active_memory_record_count(state: &Arc<crate::AppState>) -> usize {
    state
        .memory_lifecycle_store
        .as_ref()
        .expect("D055 requires the real MemoryLifecycleStore")
        .lock()
        .await
        .list_active_records(None, 100)
        .expect("read active canonical Memory records")
        .len()
}

async fn exact_pending_memory_proposal_from_task_blocker(
    state: &Arc<crate::AppState>,
    operation_id: &str,
) -> AgentProposal {
    let proposal_id = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("D055 requires the real TaskSession store")
            .lock()
            .await;
        let session = store
            .load_session(operation_id)
            .expect("read TaskSession")
            .expect("TaskSession exists at final snapshot");
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        );
        let proposal_blockers = session
            .pending_blockers
            .iter()
            .filter_map(|blocker| blocker.strip_prefix("proposal:"))
            .collect::<Vec<_>>();
        assert_eq!(
            proposal_blockers.len(),
            1,
            "the real sensitive-memory turn owns one exact Proposal blocker"
        );
        proposal_blockers[0].to_string()
    };
    let store = state
        .proposal_store
        .as_ref()
        .expect("D055 requires the real ProposalStore")
        .lock()
        .await;
    let proposal = store
        .get_proposal(&proposal_id)
        .expect("read task-blocking Proposal")
        .expect("task-blocking Proposal exists");
    assert_eq!(proposal.proposal_type, ProposalType::MemoryWrite);
    assert_eq!(proposal.status, ProposalStatus::Pending);
    assert_eq!(
        store
            .dispatch_state(&proposal_id)
            .expect("read Proposal dispatch")
            .as_deref(),
        Some("unclaimed")
    );
    proposal
}

#[derive(Debug, PartialEq, Eq)]
struct ExactDurableFacts {
    provider_request_id: String,
    provider_completed_event_id: String,
    final_event_id: String,
    final_task_owner_digest: String,
}

async fn exact_non_successor_durable_facts(
    state: &Arc<crate::AppState>,
    operation_id: &str,
) -> ExactDurableFacts {
    let events = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("D055 requires the real MainChatAgentEventStore")
        .lock()
        .await
        .list(operation_id, 0, 250)
        .expect("read exact durable EventStore facts");
    assert!(
        events
            .iter()
            .all(|event| { event.task_session_id == operation_id && event.run_id == operation_id }),
        "every D055 fact must be bound to the exact TaskSession/run identity"
    );

    let provider_started = events
        .iter()
        .filter(|event| event.event_type == "provider.started")
        .collect::<Vec<_>>();
    let provider_completed = events
        .iter()
        .filter(|event| event.event_type == PROVIDER_COMPLETED_EVENT_TYPE)
        .collect::<Vec<_>>();
    assert_eq!(provider_started.len(), 1, "exactly one provider start fact");
    assert_eq!(
        provider_completed.len(),
        1,
        "exactly one provider completion fact"
    );
    let provider_started = provider_started[0];
    let provider_completed = provider_completed[0];
    for event in [provider_started, provider_completed] {
        assert_eq!(event.object_type, "provider_request");
        assert_eq!(event.source, "provider_adapter");
        assert_eq!(
            event
                .payload
                .get("requestId")
                .and_then(serde_json::Value::as_str),
            Some(event.object_id.as_str())
        );
        assert_eq!(
            event
                .payload
                .get("provider")
                .and_then(serde_json::Value::as_str),
            Some("openai")
        );
        assert_eq!(
            event
                .payload
                .get("model")
                .and_then(serde_json::Value::as_str),
            Some("gpt-local-provider-harness")
        );
        assert!(!event.event_id.is_empty());
        assert!(!event.payload_digest.is_empty());
    }
    assert_eq!(provider_started.object_id, provider_completed.object_id);
    assert!(provider_started.sequence < provider_completed.sequence);
    assert_eq!(provider_started.payload["status"], "started");
    assert_eq!(provider_completed.payload["status"], "completed");

    let finals = events
        .iter()
        .filter(|event| event.event_type == FINAL_EVENT_TYPE)
        .collect::<Vec<_>>();
    assert_eq!(finals.len(), 1, "exactly one immutable final fact");
    let final_event = finals[0];
    let expected_delivery_id = format!("delivery:{operation_id}:{operation_id}");
    assert_eq!(final_event.object_type, "final_delivery");
    assert_eq!(final_event.object_id, expected_delivery_id);
    assert_eq!(
        final_event.source,
        "openlife_turn_runtime.final_delivery_owner"
    );
    assert_eq!(final_event.payload["deliveryId"], final_event.object_id);
    assert_eq!(final_event.payload["taskSessionId"], operation_id);
    assert_eq!(final_event.payload["runId"], operation_id);
    assert_eq!(final_event.payload["status"], "completed");
    let final_task_owner_digest = final_event
        .payload
        .get("taskOwnerDigest")
        .and_then(serde_json::Value::as_str)
        .filter(|digest| !digest.is_empty())
        .expect("final binds the canonical TaskSession owner digest")
        .to_string();
    assert!(!final_event.event_id.is_empty());
    assert!(!final_event.payload_digest.is_empty());
    assert!(provider_completed.sequence < final_event.sequence);

    let successors = events
        .iter()
        .filter(|event| event.event_type == SUCCESSOR_CONFIRMED_EVENT_TYPE)
        .collect::<Vec<_>>();
    assert!(
        successors.is_empty(),
        "an unbound Proposal or unproven drift must mint no successor fact: {successors:#?}"
    );

    ExactDurableFacts {
        provider_request_id: provider_completed.object_id.clone(),
        provider_completed_event_id: provider_completed.event_id.clone(),
        final_event_id: final_event.event_id.clone(),
        final_task_owner_digest,
    }
}

fn assert_single_provider_capture(
    captured_requests: &Arc<std::sync::Mutex<Vec<String>>>,
    expected_user_body: &str,
) {
    let requests = captured_requests
        .lock()
        .expect("read the real local HTTP provider capture");
    assert_eq!(requests.len(), 1, "recovery must not redispatch provider");
    let request = &requests[0];
    let (headers, body) = request
        .split_once("\r\n\r\n")
        .expect("captured provider request has HTTP headers and body");
    assert!(headers.starts_with("POST /v1/chat/completions HTTP/1.1"));
    let payload: serde_json::Value =
        serde_json::from_str(body).expect("captured provider body is exact JSON");
    assert_eq!(payload["model"], "gpt-local-provider-harness");
    assert!(
        payload["messages"]
            .as_array()
            .expect("provider messages array")
            .iter()
            .any(|message| message["role"] == "user" && message["content"] == expected_user_body),
        "the one captured provider dispatch must carry this scenario's exact user body"
    );
}

async fn buffered_recovery(
    state: &Arc<crate::AppState>,
    operation_id: &str,
    session_id: &str,
    body: &str,
) -> Result<crate::SendMessageResult, String> {
    crate::main_chat_send::send_message_with_operation_state(
        operation_id.to_string(),
        session_id.to_string(),
        vec![ChatMessage {
            role: "user".into(),
            content: body.into(),
        }],
        None,
        state,
    )
    .await
}

async fn streaming_recovery(
    state: &Arc<crate::AppState>,
    operation_id: &str,
    session_id: &str,
    body: &str,
) -> (
    Result<serde_json::Value, String>,
    Vec<(String, serde_json::Value)>,
) {
    let mut emitted = Vec::new();
    let result = crate::main_chat_streaming::start_stream_message_with_operation_state(
        operation_id.to_string(),
        session_id.to_string(),
        vec![ChatMessage {
            role: "user".into(),
            content: body.into(),
        }],
        None,
        state,
        |event, payload| emitted.push((event.to_string(), payload)),
    )
    .await;
    (result, emitted)
}

// Root runtime RED. This does not hand-build any origin metadata: the ordinary
// sensitive-Memory product path stages the Proposal through its real kernel +
// ReviewWorkflow route and attaches the TaskSession blocker. The target is an
// exact typed defer before dispatch claim while the origin epoch is SEALING,
// followed by one effect-blocking Memory commit and one durable TaskSession
// successor after SEALED. The original final remains immutable; acceptance
// advances the canonical owner exactly once so the approved task can finish.
#[tokio::test]
async fn real_sensitive_memory_accept_defers_at_sealing_then_commits_one_task_successor() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let operation_id = uuid::Uuid::new_v4().to_string();
        let session_id = "d055-real-sensitive-memory-sealing";
        let body = "Remember this private health fact: coffee on an empty stomach causes heart palpitations.";
        let memory_before = active_memory_record_count(&state).await;
        let (_guard, final_reached, final_release) =
            crate::main_chat_turn_runtime::install_main_chat_terminal_sealing_barrier_for_test(
                &operation_id,
            );

        let turn_state = Arc::clone(&state);
        let turn_operation = operation_id.clone();
        let turn = tokio::spawn(async move {
            buffered_recovery(&turn_state, &turn_operation, session_id, body).await
        });
        tokio::time::timeout(STEP_TIMEOUT, final_reached.wait())
            .await
            .expect("real sensitive-memory turn reaches the terminal SEALING barrier");

        let proposal = exact_pending_memory_proposal_from_task_blocker(&state, &operation_id).await;
        let owner_before = task_owner_digest(&state, &operation_id).await;
        let during_accept = tokio::time::timeout(
            STEP_TIMEOUT,
            crate::commands::proposal::accept_proposal_with_state(proposal.id.clone(), &state),
        )
        .await
        .expect("sealing acceptance decision must not hang");
        let during_truth = proposal_truth(&state, &proposal.id).await;
        let owner_during = task_owner_digest(&state, &operation_id).await;
        let memory_during = active_memory_record_count(&state).await;
        let successor_count_during = state
            .main_chat_agent_event_store
            .as_ref()
            .expect("real EventStore")
            .lock()
            .await
            .list(&operation_id, 0, 250)
            .expect("read EventStore during sealing")
            .iter()
            .filter(|event| event.event_type == SUCCESSOR_CONFIRMED_EVENT_TYPE)
            .count();

        // Always release the production TurnRuntime before evaluating RED
        // assertions. The complete scenario remains bounded by the outer
        // timeout, and a failing oracle cannot strand the shared barrier.
        final_release.wait().await;
        let first = tokio::time::timeout(STEP_TIMEOUT, turn)
            .await
            .expect("origin turn leaves final barrier")
            .expect("origin turn task joins");

        let after_seal_accept = tokio::time::timeout(
            STEP_TIMEOUT,
            crate::commands::proposal::accept_proposal_with_state(proposal.id.clone(), &state),
        )
        .await
        .expect("post-seal acceptance must not hang");
        let idempotent_accept = tokio::time::timeout(
            STEP_TIMEOUT,
            crate::commands::proposal::accept_proposal_with_state(proposal.id.clone(), &state),
        )
        .await
        .expect("post-seal replay must not hang");
        let after_truth = proposal_truth(&state, &proposal.id).await;
        let memory_after = active_memory_record_count(&state).await;
        let owner_after = task_owner_digest(&state, &operation_id).await;
        let buffered = buffered_recovery(&state, &operation_id, session_id, body).await;
        let (streaming, streaming_events) =
            streaming_recovery(&state, &operation_id, session_id, body).await;

        let events = state
            .main_chat_agent_event_store
            .as_ref()
            .expect("real EventStore")
            .lock()
            .await
            .list(&operation_id, 0, 250)
            .expect("read exact post-successor facts");
        let finals = events
            .iter()
            .filter(|event| event.event_type == FINAL_EVENT_TYPE)
            .collect::<Vec<_>>();
        let successors = events
            .iter()
            .filter(|event| event.event_type == SUCCESSOR_CONFIRMED_EVENT_TYPE)
            .collect::<Vec<_>>();

        let during = during_accept
            .expect("SEALING is a typed deferred result, not an arbitrary error");
        assert_eq!(during["success"], false);
        assert_eq!(during["status"], "deferred");
        assert_eq!(during["reasonCode"], "origin_turn_sealing");
        assert_eq!(during["proposalId"], proposal.id);
        assert_eq!(during["dispatchState"], "unclaimed");
        assert_eq!(during["durableWriteExecuted"], false);
        assert_eq!(during_truth.0, ProposalStatus::Pending);
        assert_eq!(during_truth.1, "unclaimed");
        assert_eq!(owner_during, owner_before);
        assert_eq!(memory_during, memory_before);
        assert_eq!(successor_count_during, 0);

        assert!(first.is_ok(), "origin turn failed: {first:?}");
        let after = after_seal_accept.expect("same Proposal succeeds after SEALED");
        assert_eq!(after["success"], true);
        assert_eq!(after["effect_status"], "confirmed");
        assert_eq!(after["proposal_projection_status"], "confirmed");
        let transition = after
            .get("terminalOwnerTransition")
            .expect("effect-blocking Memory acceptance returns its owner transition");
        let replay = idempotent_accept.expect("exact retry returns confirmed truth");
        assert_eq!(replay["success"], true);
        assert_eq!(after_truth.0, ProposalStatus::Accepted);
        assert_eq!(after_truth.1, "confirmed");
        assert_eq!(memory_after, memory_before + 1);
        assert_ne!(owner_after, owner_before);
        assert_eq!(transition["beforeOwnerDigest"], owner_before);
        assert_eq!(transition["afterOwnerDigest"], owner_after);
        assert!(
            buffered.is_ok(),
            "buffered replay must reuse the immutable sealed final after the legal successor: {buffered:?}"
        );
        assert!(
            streaming.is_ok(),
            "streaming replay must reuse the immutable sealed final after the legal successor: {streaming:?}"
        );
        assert_eq!(
            streaming_events
                .iter()
                .filter(|(event, _)| event == "stream-message-done")
                .count(),
            1
        );
        assert_eq!(finals.len(), 1, "one immutable original final");
        assert_eq!(successors.len(), 1, "one accepted blocking effect mints one successor");
        let final_event = finals[0];
        let successor = successors[0];
        assert_eq!(final_event.task_session_id, operation_id);
        assert_eq!(final_event.run_id, operation_id);
        assert_eq!(final_event.object_type, "final_delivery");
        assert_eq!(
            final_event.object_id,
            format!("delivery:{operation_id}:{operation_id}")
        );
        assert_eq!(
            final_event.source,
            "openlife_turn_runtime.final_delivery_owner"
        );
        assert_eq!(final_event.payload["taskOwnerDigest"], owner_before);
        assert_eq!(successor.event_id, transition["successorEventId"]);
        assert_eq!(successor.task_session_id, operation_id);
        assert_eq!(successor.run_id, operation_id);
        assert_eq!(successor.object_type, "terminal_owner_successor");
        assert_eq!(
            successor.source,
            "terminal_owner_write_gateway.review_successor"
        );
        assert_eq!(successor.payload["causeKind"], "proposal_review_acceptance");
        assert_eq!(successor.payload["causeRef"], proposal.id);
        assert_eq!(successor.payload["finalEventId"], final_event.event_id);
        assert_eq!(successor.payload["beforeOwnerDigest"], owner_before);
        assert_eq!(successor.payload["afterOwnerDigest"], owner_after);
        let completed_session = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("real TaskSession store")
            .lock()
            .await
            .load_session(&operation_id)
            .expect("load accepted sensitive-Memory task")
            .expect("accepted sensitive-Memory task remains present");
        assert_eq!(
            completed_session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
        );
        assert!(completed_session.pending_blockers.is_empty());
    })
    .await
    .expect("D055 real accept-at-sealing scenario exceeded its outer timeout");
}

// Runtime RED, not a typed-origin GREEN oracle. The Proposal is intentionally
// unbound and may be accepted as an independent review item. Its forged
// source_detail/after fields must not make it an origin-turn writer, defer it
// because this turn is sealing, clear this task's blocker, or mint a successor.
#[tokio::test]
async fn forged_source_detail_and_after_cannot_gain_terminal_owner_authority() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let captured_requests = crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_captured_local_http_provider(
            &state,
            "D055 forged-origin provider reply",
        )
        .await;
        let operation_id = uuid::Uuid::new_v4().to_string();
        let session_id = "d055-forged-origin-negative";
        let body = "Explain one practical way to stay focused this afternoon.";
        let proposal_id = stage_forged_unbound_memory_proposal(&state, &operation_id).await;
        let (_pre_guard, pre_reached, pre_release, _polls) =
            crate::main_chat_turn_runtime::install_main_chat_pre_registration_barrier_for_test(
                session_id,
            );
        let (_final_guard, final_reached, final_release) =
            crate::main_chat_turn_runtime::install_main_chat_terminal_sealing_barrier_for_test(
                &operation_id,
            );

        let turn_state = Arc::clone(&state);
        let turn_operation = operation_id.clone();
        let turn = tokio::spawn(async move {
            buffered_recovery(&turn_state, &turn_operation, session_id, body).await
        });
        tokio::time::timeout(STEP_TIMEOUT, pre_reached.wait())
            .await
            .expect("turn reaches the real task-created barrier");
        set_test_proposal_blocker(&state, &operation_id, &proposal_id).await;
        pre_release.wait().await;
        tokio::time::timeout(STEP_TIMEOUT, final_reached.wait())
            .await
            .expect("turn reaches the real terminal SEALING barrier");
        let owner_before_accept = task_owner_digest(&state, &operation_id).await;

        let accept = tokio::time::timeout(
            STEP_TIMEOUT,
            crate::commands::proposal::accept_proposal_with_state(
                proposal_id.clone(),
                &state,
            ),
        )
        .await
        .expect("real Proposal accept must not hang");
        let owner_after_accept = task_owner_digest(&state, &operation_id).await;
        let proposal_after_accept = proposal_truth(&state, &proposal_id).await;

        // Never leave the real TurnRuntime parked when an assertion later
        // reports the RED. The outer timeout bounds the complete scenario.
        final_release.wait().await;
        let first = tokio::time::timeout(STEP_TIMEOUT, turn)
            .await
            .expect("origin TurnRuntime leaves final barrier")
            .expect("origin TurnRuntime task joins");
        let buffered = buffered_recovery(&state, &operation_id, session_id, body).await;
        let (streaming, streaming_events) =
            streaming_recovery(&state, &operation_id, session_id, body).await;
        let facts = exact_non_successor_durable_facts(&state, &operation_id).await;

        assert!(accept.is_ok(), "independent Proposal accept failed: {accept:?}");
        assert_eq!(proposal_after_accept.0, ProposalStatus::Accepted);
        assert_eq!(proposal_after_accept.1, "confirmed");
        assert_eq!(
            owner_after_accept, owner_before_accept,
            "D055 RED: forged source_detail/after gained TaskSession authority during the terminal snapshot window"
        );
        assert!(first.is_ok(), "origin turn failed: {first:?}");
        assert!(
            buffered.is_ok(),
            "forged unbound metadata must not create post-final owner drift: {buffered:?}"
        );
        assert!(
            streaming.is_ok(),
            "streaming recovery must observe the same intact sealed final: {streaming:?}"
        );
        assert_eq!(
            streaming_events
                .iter()
                .filter(|(event, _)| event == "stream-message-done")
                .count(),
            1
        );
        assert_single_provider_capture(&captured_requests, body);
        assert_eq!(facts.final_task_owner_digest, owner_before_accept);
        assert!(!facts.provider_request_id.is_empty());
        assert!(!facts.provider_completed_event_id.is_empty());
        assert!(!facts.final_event_id.is_empty());
    })
    .await
    .expect("D055 forged-origin concurrency scenario exceeded its outer timeout");
}

// Counterfactual control: the future successor fold must remain strict. A raw
// owner mutation with no EventStore claim and no owner-local transition receipt
// is drift, not a legal successor, in both delivery modes.
#[tokio::test]
async fn unproven_post_final_owner_drift_stays_fail_closed_without_redispatch() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let captured_requests = crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_captured_local_http_provider(
            &state,
            "D055 unproven-drift provider reply",
        )
        .await;
        let operation_id = uuid::Uuid::new_v4().to_string();
        let session_id = "d055-unproven-post-final-drift";
        let body = "Explain one practical way to stay focused this afternoon.";
        let initial = buffered_recovery(&state, &operation_id, session_id, body).await;
        assert!(initial.is_ok(), "initial turn failed: {initial:?}");

        state
            .main_chat_agent_session_store
            .as_ref()
            .expect("real TaskSession store")
            .lock()
            .await
            .set_pending_blockers(&operation_id, vec!["unproven_post_final_drift".into()])
            .expect("inject raw post-final owner drift without a claim/receipt");

        let buffered = buffered_recovery(&state, &operation_id, session_id, body).await;
        let (streaming, emitted) =
            streaming_recovery(&state, &operation_id, session_id, body).await;
        let expected =
            "turn_operation_final_reconciliation_required:canonical_owner_digest_drift";
        assert_eq!(buffered.as_ref().expect_err("raw drift must fail"), expected);
        assert_eq!(streaming.as_ref().expect_err("raw stream drift must fail"), expected);
        assert!(
            emitted
                .iter()
                .all(|(event, _)| event != "stream-message-done"),
            "failed recovery may emit a structured error, but never a fake done event: {emitted:#?}"
        );
        assert_single_provider_capture(&captured_requests, body);
        let facts = exact_non_successor_durable_facts(&state, &operation_id).await;
        assert!(!facts.final_task_owner_digest.is_empty());
    })
    .await
    .expect("D055 unproven-drift counterfactual exceeded its outer timeout");
}

#[tokio::test]
async fn normal_buffered_and_streaming_recovery_controls_remain_idempotent() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        let body = "Explain one practical way to stay focused this afternoon.";

        let buffered_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let buffered_capture = crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_captured_local_http_provider(
            &buffered_state,
            "D055 buffered recovery control",
        )
        .await;
        let buffered_operation = uuid::Uuid::new_v4().to_string();
        let buffered_session = "d055-buffered-recovery-control";
        let first = buffered_recovery(
            &buffered_state,
            &buffered_operation,
            buffered_session,
            body,
        )
        .await;
        let replay = buffered_recovery(
            &buffered_state,
            &buffered_operation,
            buffered_session,
            body,
        )
        .await;
        let (stream_recovery, stream_recovery_events) = streaming_recovery(
            &buffered_state,
            &buffered_operation,
            buffered_session,
            body,
        )
        .await;
        assert!(
            first.is_ok() && replay.is_ok() && stream_recovery.is_ok(),
            "buffered/streaming recovery control failed: first={first:?}, buffered={replay:?}, streaming={stream_recovery:?}"
        );
        assert_eq!(
            stream_recovery_events
                .iter()
                .filter(|(event, _)| event == "stream-message-done")
                .count(),
            1
        );
        assert_single_provider_capture(&buffered_capture, body);
        let buffered_facts =
            exact_non_successor_durable_facts(&buffered_state, &buffered_operation).await;
        assert!(!buffered_facts.final_task_owner_digest.is_empty());
    })
    .await
    .expect("D055 buffered/streaming controls exceeded their outer timeout");
}

// This is an absence/deletion oracle, so source classification is appropriate;
// unlike the removed receipt oracle, it does not claim strings prove runtime
// behavior. Compile-time function-item bindings keep the matrix tied to the
// real current writer APIs, while the behavioral tests above prove outcomes.
#[test]
fn turn_bound_writer_matrix_has_no_direct_product_bypass_after_gateway_cutover() {
    // Concrete API inventory. If a store writer is renamed, this test must be
    // updated deliberately instead of silently ceasing to cover that owner.
    let _task_session_status = AgentTaskSessionStore::set_pending_blockers;
    let _task_session_complete = AgentTaskSessionStore::complete_session;
    let _task_session_block = AgentTaskSessionStore::block_session;
    let _task_session_fail = AgentTaskSessionStore::fail_session;
    let _task_transcript = AgentTaskSessionStore::append_transcript_entry;
    let _agent_run = AgentRunStore::update_run;
    let _agent_run_delete = AgentRunStore::delete_run_with_tombstone;
    let _agent_run_restore = AgentRunStore::restore_run_with_receipt;
    let _action_enqueue = ActionQueueStore::enqueue;
    let _action_claim = ActionQueueStore::claim_replay_with_automatic_retry_proof;
    let _tool_event = |state: Arc<crate::AppState>| async move {
        crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
            &state,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            serde_json::Value::Null,
        )
        .await
    };
    let _life_model =
        crate::life_model_write_gateway::materialize_accepted_lifemodel_proposal_with_state;
    let _memory = crate::memory_gateway::commit_explicit_user_memory_for_turn_with_state;
    let _memory_commit = MemoryLifecycleStore::commit_explicit_user_memory;
    let _memory_rollback = MemoryLifecycleStore::rollback_memory_asset;
    let _conversation_delete_store = MemoryStore::delete_chat_session_with_tombstone;
    let _conversation_delete_gateway = crate::memory_gateway::delete_chat_session_with_state;
    let _agent_run_restore_command = crate::commands::agent::restore_agent_run_with_state;
    let _life_model_restore = crate::life_model_write_gateway::restore_life_model_with_gateway;
    let _transcript_draft_type = std::any::TypeId::of::<ExecutionTranscriptEntryDraft>();

    // Scope is product callers that currently participate in a turn or its
    // Proposal reconciliation. Store definitions, projection/recovery jobs,
    // bootstrap, and test modules are intentionally excluded. The future
    // `terminal_owner_write_gateway.rs` is the sole allowed direct caller.
    // LifeModel/Memory domain gateways remain nested canonical writers; a
    // typed origin must reach them only through the terminal-owner gateway.
    const ALLOWED_EXCEPTIONS: &[&str] = &[
        "store API definitions in openlife-core",
        "domain-internal LifeModelWriteGateway and MemoryGateway transactions",
        "bootstrap and projection recovery without a turn origin",
        "test modules removed from each scanned production source range",
        "terminal_owner_write_gateway.rs after the D055 cutover",
    ];
    assert_eq!(ALLOWED_EXCEPTIONS.len(), 5);

    fn before_test_module<'a>(source: &'a str, marker: &str) -> &'a str {
        source
            .split_once(marker)
            .map(|(production, _)| production)
            .unwrap_or(source)
    }

    let surfaces = [
        (
            "main_chat_turn_runtime.rs",
            before_test_module(
                include_str!("main_chat_turn_runtime.rs"),
                "#[cfg(test)]\nmod turn_admission_tests",
            ),
        ),
        (
            "main_chat_runtime_support.rs",
            before_test_module(
                include_str!("main_chat_runtime_support.rs"),
                "#[cfg(test)]\nmod tests",
            ),
        ),
        (
            "main_chat_kernel.rs",
            before_test_module(
                include_str!("main_chat_kernel.rs"),
                "#[cfg(test)]\nmod tests",
            ),
        ),
        (
            "main_chat_task_controls.rs",
            before_test_module(
                include_str!("main_chat_task_controls.rs"),
                "#[cfg(test)]\nmod product_task_dto_tests",
            ),
        ),
        (
            "commands/proposal.rs",
            before_test_module(
                include_str!("commands/proposal.rs"),
                "#[cfg(test)]\nmod tests",
            ),
        ),
        (
            "commands/agent.rs",
            before_test_module(include_str!("commands/agent.rs"), "#[cfg(test)]\nmod tests"),
        ),
        (
            "main_chat_generation_support.rs",
            include_str!("main_chat_generation_support.rs"),
        ),
    ];
    let writer_apis = [
        (
            "TaskSession",
            "set_pending_blockers",
            ".set_pending_blockers(",
        ),
        (
            "TaskSession",
            "append_transcript_entry",
            ".append_transcript_entry(",
        ),
        ("TaskSession", "complete_session", ".complete_session("),
        ("TaskSession", "block_session", ".block_session("),
        ("TaskSession", "fail_session", ".fail_session("),
        ("AgentRun", "update_run", ".update_run("),
        ("ActionQueue", "enqueue", ".enqueue("),
        (
            "ActionQueue",
            "claim_replay_with_automatic_retry_proof",
            ".claim_replay_with_automatic_retry_proof(",
        ),
        (
            "ActionQueue",
            "transition_claimed_replay",
            ".transition_claimed_replay(",
        ),
        (
            "ActionQueue",
            "fail_and_release_replay_claim_before_dispatch",
            ".fail_and_release_replay_claim_before_dispatch(",
        ),
        (
            "ActionQueue",
            "fail_claimed_replay",
            ".fail_claimed_replay(",
        ),
        (
            "ActionQueue",
            "release_pending_permission_replay_claim_without_dispatch",
            ".release_pending_permission_replay_claim_without_dispatch(",
        ),
        (
            "ActionQueue",
            "fence_replay_dispatch_commit",
            ".fence_replay_dispatch_commit(",
        ),
        (
            "ActionQueue",
            "record_replay_dispatch_started",
            ".record_replay_dispatch_started(",
        ),
        (
            "ActionQueue",
            "complete_claimed_replay",
            ".complete_claimed_replay(",
        ),
        (
            "AgentRun",
            "delete_run_with_tombstone",
            ".delete_run_with_tombstone(",
        ),
        (
            "AgentRun",
            "restore_run_with_receipt",
            ".restore_run_with_receipt(",
        ),
        (
            "ToolEvent/Final",
            "append_main_chat_agent_runtime_event",
            "append_main_chat_agent_runtime_event(",
        ),
    ];
    let mut direct_hits = Vec::new();
    for (file, source) in surfaces {
        for (line_index, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for (owner, api, pattern) in writer_apis {
                if line.contains(pattern) {
                    direct_hits.push(format!("{file}:{}:{owner}:{api}:{pattern}", line_index + 1));
                }
            }
        }
    }

    assert!(
        direct_hits.is_empty(),
        "D055 RED: covered product writers still bypass one terminal-owner gateway: {direct_hits:#?}; allowed exceptions={ALLOWED_EXCEPTIONS:#?}"
    );
}

#[test]
fn terminal_origin_authority_surface_has_no_naked_id_or_string_minter_after_cutover() {
    // This is deliberately deletion/absence evidence only. Dynamic authority
    // behavior is proven by the file-backed compile target; source strings
    // cannot establish that a typed proof is valid or invalid at runtime.
    fn collect_rust_files(root: &std::path::Path, output: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(root).expect("read production authority source tree") {
            let entry = entry.expect("read production authority source entry");
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().and_then(|name| name.to_str()) != Some("tests") {
                    collect_rust_files(&path, output);
                }
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("");
                if !name.ends_with("_tests.rs")
                    && !matches!(
                        name,
                        "d055_terminal_owner_graph_tests.rs"
                            | "d055_terminal_owner_graph_compile_red.rs"
                    )
                {
                    output.push(path);
                }
            }
        }
    }

    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let roots = [manifest.join("src"), manifest.join("../openlife-core/src")];
    let forbidden = [
        "issue_review_origin_proof(",
        "TerminalOwnerOriginProof::new(",
        "TerminalOwnerOriginProof::from_ids(",
        "TerminalOwnerOriginProof::from_strings(",
        "TerminalOwnerOriginProof::from_source_detail(",
        "TerminalOwnerOriginProof::from_after(",
        "TerminalOwnerEpochAdmission::new(",
        "TerminalOwnerEpochAdmission::from_ids(",
        "TerminalOwnerEpochAdmission::from_strings(",
        "TerminalOwnerOriginBinding::new(",
        "fn mint_terminal_owner",
        "fn issue_terminal_owner_origin",
        "fn create_terminal_owner_origin",
        "fn terminal_owner_origin_from_",
        ".get(\"originatingTaskSessionId\")",
        ".get(\"originating_task_session_id\")",
        "\"originatingTaskSessionId\"",
        "\"originating_task_session_id\"",
        "main_chat_agent_task_session:",
        "proposal.source_detail.as_deref() == Some(task_session_id)",
        "source_detail == task_session_id",
    ];
    let mut files = Vec::new();
    for root in roots {
        collect_rust_files(&root, &mut files);
    }
    files.sort();
    let mut hits = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(&file).expect("read production authority source");
        // Repository convention places named unit-test modules at the end.
        // Test-only imports/helpers can appear earlier, so truncate only at a
        // cfg(test) module declaration rather than the first cfg(test) item.
        let production = source
            .split_once("#[cfg(test)]\nmod ")
            .map(|(before, _)| before)
            .unwrap_or(source.as_str());
        for (line_index, line) in production.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for pattern in forbidden {
                if line.contains(pattern) {
                    hits.push(format!(
                        "{}:{}:{pattern}",
                        file.strip_prefix(&manifest)
                            .unwrap_or(file.as_path())
                            .display(),
                        line_index + 1
                    ));
                }
            }
        }
    }

    assert!(
        hits.is_empty(),
        "D055 RED deletion evidence: production authority surfaces still contain caller-shaped terminal-origin minters/fields: {hits:#?}"
    );
}
