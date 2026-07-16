//! D055 target-contract oracle.
//!
//! This module is compiled only by the explicit `d055_compile_red` command in
//! the D055 RED matrix. It intentionally calls the future production
//! terminal-owner APIs directly. The file itself must always exist; RED must
//! come from the missing production seam or a broken invariant, never from an
//! opaque integration harness or a missing test module.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use openlife_core::agent::main_chat_agent_v1::{
    AgentTaskSessionDraft, AgentTaskSessionStore, MainChatAgentStrategy,
};
use openlife_core::agent::{
    AgentProposal, AgentRunReceiptKey, DurableWriteRequest, DurableWriteSource,
    DurableWriteSubject, MemoryLifecycleStore, ProposalSource, ProposalStatus, ProposalStore,
    ProposalType, ReviewWorkflow, RiskLevel,
};
use openlife_core::llm::ChatMessage;
use openlife_core::memory::MemoryStore;

// Provisional contract names. The invariants and exact stored facts below are
// authoritative; implementation may rename these production types only by
// updating this compile oracle and the reviewed D055 matrix together.
use crate::main_chat_event_stream::{MainChatTerminalFinalizationInput, TerminalOwnerSealState};
use crate::terminal_owner_write_gateway::{
    ExternalDispatchOutcome, PreparedTerminalOwnerExternalDispatch, TerminalOwnerCrashPoint,
    TerminalOwnerExecutionCapture, TerminalOwnerExternalDispatchAdapter, TerminalOwnerWriteGateway,
};

const SCENARIO_TIMEOUT: Duration = Duration::from_secs(45);
const STEP_TIMEOUT: Duration = Duration::from_secs(10);
const SENSITIVE_MEMORY_BODY: &str =
    "Remember this private health fact: coffee on an empty stomach causes heart palpitations.";

fn receipt_key() -> AgentRunReceiptKey {
    AgentRunReceiptKey::from_bytes([0xD5; 32]).expect("non-zero D055 receipt key")
}

fn create_file_backed_task(
    task_store: &AgentTaskSessionStore,
    operation_id: &str,
    chat_session_id: &str,
    user_goal: &str,
) {
    task_store
        .create_session_with_id(
            operation_id.to_string(),
            AgentTaskSessionDraft {
                chat_session_id: chat_session_id.to_string(),
                user_goal: user_goal.into(),
                selected_strategy: MainChatAgentStrategy::MemoryProposal,
                current_plan_summary: None,
                context_snapshot_refs: Vec::new(),
            },
        )
        .expect("create a real file-backed TaskSession owner");
}

fn commit_file_backed_user_message(
    memory_store: &MemoryStore,
    operation_id: &str,
    chat_session_id: &str,
    body: &str,
) -> openlife_core::memory::CanonicalConversationMessageCommit {
    memory_store
        .create_chat_session(chat_session_id, "D055 canonical origin")
        .expect("create canonical Conversation owner");
    memory_store
        .save_message_idempotent_with_proof(
            chat_session_id,
            &ChatMessage {
                role: "user".into(),
                content: body.into(),
            },
            operation_id,
        )
        .expect("commit canonical user message and obtain opaque owner proof")
}

fn external_unknown_proposal() -> AgentProposal {
    AgentProposal::new(
        ProposalType::ExternalWriteAction,
        "external.unknown.d055",
        serde_json::json!({"action": "send", "target": "remote"}),
        "An unqueryable external effect remains unknown and is never automatically retried.",
        1.0,
        RiskLevel::High,
        ProposalSource::ChatConversation,
    )
}

fn external_unknown_review_request() -> DurableWriteRequest {
    DurableWriteRequest::from_agent_proposal(
        DurableWriteSource::MainChat,
        DurableWriteSubject::ExternalWrite,
        external_unknown_proposal(),
        "External effect requires Review Center approval and queryable reconciliation.",
    )
}

#[derive(Clone, Default)]
struct RecordingUnknownExternalDispatch {
    calls: Arc<AtomicUsize>,
    proposal_ids: Arc<Mutex<Vec<String>>>,
}

impl RecordingUnknownExternalDispatch {
    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn proposal_ids(&self) -> Vec<String> {
        self.proposal_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[async_trait::async_trait]
impl TerminalOwnerExternalDispatchAdapter for RecordingUnknownExternalDispatch {
    async fn dispatch(
        &self,
        request: &PreparedTerminalOwnerExternalDispatch,
    ) -> Result<ExternalDispatchOutcome, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.proposal_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request.proposal_id().to_string());
        Ok(ExternalDispatchOutcome::RemoteUnknown)
    }
}

struct SealedReviewScenario {
    _temp: tempfile::TempDir,
    event_path: std::path::PathBuf,
    task_path: std::path::PathBuf,
    proposal_path: std::path::PathBuf,
    memory_lifecycle_path: std::path::PathBuf,
    operation_id: String,
    proposal_id: String,
    proposal_blocker: String,
    final_event_id: String,
    owner_at_final_revision: u64,
    owner_at_final_digest: String,
    event_store: crate::main_chat_event_stream::MainChatAgentEventStore,
    task_store: AgentTaskSessionStore,
    proposal_store: ProposalStore,
    memory_store: MemoryStore,
    memory_lifecycle_store: MemoryLifecycleStore,
}

fn setup_sealed_review_scenario(label: &str, request: DurableWriteRequest) -> SealedReviewScenario {
    let temp = tempfile::tempdir().expect("D055 staged crash temp directory");
    let event_path = temp.path().join(format!("{label}-turn-events.sqlite"));
    let task_path = temp.path().join(format!("{label}-task-owner.sqlite"));
    let proposal_path = temp.path().join(format!("{label}-proposal-owner.sqlite"));
    let conversation_path = temp
        .path()
        .join(format!("{label}-conversation-owner.sqlite"));
    let memory_lifecycle_path = temp
        .path()
        .join(format!("{label}-memory-lifecycle-owner.sqlite"));
    let operation_id = uuid::Uuid::new_v4().to_string();
    let chat_session_id = format!("d055-{label}");
    let event_store = crate::main_chat_event_stream::MainChatAgentEventStore::new(&event_path)
        .expect("real file-backed EventStore");
    let task_store = AgentTaskSessionStore::new_with_receipt_key(&task_path, receipt_key())
        .expect("real file-backed TaskSession store");
    let proposal_store =
        ProposalStore::new(&proposal_path).expect("real file-backed ProposalStore");
    let memory_store =
        MemoryStore::new(&conversation_path).expect("real file-backed Conversation owner");
    let memory_lifecycle_store = MemoryLifecycleStore::new(&memory_lifecycle_path)
        .expect("real file-backed Memory lifecycle owner");
    let canonical_message = commit_file_backed_user_message(
        &memory_store,
        &operation_id,
        &chat_session_id,
        SENSITIVE_MEMORY_BODY,
    );
    let canonical_receipt = canonical_message.receipt().clone();
    create_file_backed_task(
        &task_store,
        &operation_id,
        &chat_session_id,
        SENSITIVE_MEMORY_BODY,
    );
    task_store
        .bind_canonical_memory_store(&memory_store)
        .expect("TaskSession store binds canonical Conversation owner");
    task_store
        .bind_session_canonical_user_message(
            &operation_id,
            &canonical_receipt.canonical_ref,
            SENSITIVE_MEMORY_BODY,
        )
        .expect("TaskSession binds exact canonical user message");
    let admission = task_store
        .issue_terminal_owner_epoch_admission(&operation_id, &operation_id, canonical_message)
        .expect("TaskSession consumes canonical-message authority exactly once");
    let epoch = event_store
        .open_terminal_owner_epoch_from_admission(admission)
        .expect("EventStore opens epoch only from non-Serde admission");
    assert_eq!(
        epoch.canonical_user_message_ref(),
        canonical_receipt.canonical_ref
    );
    assert_eq!(
        epoch.canonical_user_message_digest(),
        canonical_receipt.content_digest
    );
    let origin = epoch
        .review_origin_proof()
        .expect("message-bound epoch exposes immutable Review origin");
    assert_eq!(
        origin.canonical_user_message_ref(),
        canonical_receipt.canonical_ref
    );
    assert_eq!(
        origin.canonical_user_message_digest(),
        canonical_receipt.content_digest
    );
    let staged = ReviewWorkflow::new(&proposal_store)
        .submit_with_terminal_owner_origin(request, origin)
        .expect("persist Proposal with typed terminal origin");
    let proposal_id = staged.proposal.id.clone();
    let proposal_blocker = format!("proposal:{proposal_id}");
    task_store
        .set_pending_blockers(&operation_id, vec![proposal_blocker.clone()])
        .expect("attach exact Proposal blocker");
    task_store
        .mark_waiting_permission(&operation_id)
        .expect("enter WaitingPermission before terminalization");
    event_store
        .begin_terminal_owner_seal(&operation_id, &operation_id, epoch.generation())
        .expect("OPEN -> SEALING CAS");
    let owner_at_final = task_store
        .canonical_owner_head(&operation_id)
        .expect("read Task owner at final")
        .expect("Task owner exists");
    let owner_at_final_revision = owner_at_final.revision();
    let owner_at_final_digest = owner_at_final.digest().to_string();
    let final_fact = event_store
        .append_terminal_final_and_seal(MainChatTerminalFinalizationInput {
            task_session_id: operation_id.clone(),
            run_id: operation_id.clone(),
            epoch_generation: epoch.generation(),
            delivery_id: format!("delivery:{operation_id}:{operation_id}"),
            expected_task_owner_revision: owner_at_final_revision,
            expected_task_owner_digest: owner_at_final_digest.clone(),
            status: "waiting_permission".into(),
        })
        .expect("commit final and SEALED epoch atomically inside EventStore");

    SealedReviewScenario {
        _temp: temp,
        event_path,
        task_path,
        proposal_path,
        memory_lifecycle_path,
        operation_id,
        proposal_id,
        proposal_blocker,
        final_event_id: final_fact.event_id,
        owner_at_final_revision,
        owner_at_final_digest,
        event_store,
        task_store,
        proposal_store,
        memory_store,
        memory_lifecycle_store,
    }
}

fn assert_memory_review_converged(
    event_store: &crate::main_chat_event_stream::MainChatAgentEventStore,
    task_store: &AgentTaskSessionStore,
    proposal_store: &ProposalStore,
    memory_lifecycle_store: &MemoryLifecycleStore,
    operation_id: &str,
    proposal_id: &str,
    proposal_blocker: &str,
    final_event_id: &str,
    owner_at_final_revision: u64,
    owner_at_final_digest: &str,
) {
    let memory = memory_lifecycle_store
        .get_record_by_proposal_id(proposal_id)
        .expect("read converged Memory effect")
        .expect("Memory effect exists after reconciliation");
    assert_eq!(memory.proposal_id, proposal_id);
    assert_eq!(
        memory_lifecycle_store
            .list_active_records(None, 100)
            .expect("count converged Memory owners")
            .len(),
        1
    );
    let owner = task_store
        .canonical_owner_head(operation_id)
        .expect("read converged Task owner")
        .expect("converged Task owner exists");
    assert_eq!(owner.revision(), owner_at_final_revision + 1);
    assert_ne!(owner.digest(), owner_at_final_digest);
    let task = task_store
        .load_session(operation_id)
        .expect("read converged TaskSession")
        .expect("converged TaskSession exists");
    assert_eq!(
        task.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
    );
    assert!(!task
        .pending_blockers
        .iter()
        .any(|item| item == proposal_blocker));
    assert_eq!(
        proposal_store
            .get_proposal(proposal_id)
            .expect("read converged Proposal")
            .expect("converged Proposal exists")
            .status,
        ProposalStatus::Accepted
    );
    assert_eq!(
        proposal_store
            .dispatch_state(proposal_id)
            .expect("read converged Proposal dispatch")
            .as_deref(),
        Some("confirmed")
    );
    let events = event_store
        .list(operation_id, 0, 250)
        .expect("read converged terminal facts");
    let finals = events
        .iter()
        .filter(|event| event.event_type == "final_delivery.created")
        .collect::<Vec<_>>();
    let successors = events
        .iter()
        .filter(|event| event.event_type == "terminal_owner.successor_confirmed")
        .collect::<Vec<_>>();
    assert_eq!(finals.len(), 1);
    assert_eq!(finals[0].event_id, final_event_id);
    assert_eq!(successors.len(), 1);
    assert_eq!(successors[0].payload["causeRef"], proposal_id);
    assert_eq!(successors[0].payload["finalEventId"], final_event_id);
    assert_eq!(successors[0].payload["ownerId"], operation_id);
    assert_eq!(
        successors[0].payload["beforeOwnerRevision"],
        owner_at_final_revision
    );
    assert_eq!(
        successors[0].payload["afterOwnerRevision"],
        owner_at_final_revision + 1
    );
    for field in [
        "beforeOwnerDigest",
        "afterOwnerDigest",
        "localTransitionReceiptRef",
        "localTransitionReceiptDigest",
    ] {
        assert!(successors[0].payload[field]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
    }
}

async fn run_memory_crash_reconciliation_contract(
    label: &str,
    crash_point: TerminalOwnerCrashPoint,
    memory_committed_before_crash: bool,
    task_committed_before_crash: bool,
    proposal_checkpoint_before_crash: bool,
    successor_before_crash: bool,
) {
    let SealedReviewScenario {
        _temp,
        event_path,
        task_path,
        proposal_path,
        memory_lifecycle_path,
        operation_id,
        proposal_id,
        proposal_blocker,
        final_event_id,
        owner_at_final_revision,
        owner_at_final_digest,
        event_store,
        task_store,
        proposal_store,
        memory_store,
        memory_lifecycle_store,
    } = setup_sealed_review_scenario(label, memory_review_request());
    let claim_id = proposal_store
        .claim_dispatch(&proposal_id)
        .expect("claim exact Proposal")
        .expect("one durable Review claimant");
    assert_eq!(
        proposal_store
            .dispatch_claim_id(&proposal_id)
            .expect("read durable claim id")
            .as_deref(),
        Some(claim_id.as_str())
    );
    let acceptance = ReviewWorkflow::new(&proposal_store)
        .claimed_acceptance_snapshot(&proposal_id, &claim_id)
        .expect("claim yields non-Serde acceptance authority");
    let capture = TerminalOwnerExecutionCapture::default();
    let gateway = TerminalOwnerWriteGateway::new(
        &event_store,
        &task_store,
        &proposal_store,
        &memory_lifecycle_store,
    )
    .with_execution_capture_for_test(capture.clone());
    gateway
        .install_crash_point_for_test(&proposal_id, crash_point)
        .expect("install exact cross-store crash point");
    let error = gateway
        .apply_claimed_review_acceptance(acceptance)
        .await
        .expect_err("the selected boundary must interrupt convergence");
    assert_eq!(
        error.to_string(),
        format!("injected_terminal_owner_crash:{}", crash_point.as_str())
    );
    assert_eq!(
        capture.memory_effect_invocations(&proposal_id),
        usize::from(memory_committed_before_crash)
    );
    assert_eq!(
        capture.task_owner_transition_invocations(&proposal_id),
        usize::from(task_committed_before_crash)
    );
    assert_eq!(
        capture.successor_confirmation_invocations(&proposal_id),
        usize::from(successor_before_crash)
    );
    assert_eq!(capture.proposal_projection_invocations(&proposal_id), 0);

    let memory_before_restart = memory_lifecycle_store
        .get_record_by_proposal_id(&proposal_id)
        .expect("read Memory owner at injected boundary");
    assert_eq!(
        memory_before_restart.is_some(),
        memory_committed_before_crash
    );
    assert_eq!(
        memory_lifecycle_store
            .list_active_records(None, 100)
            .expect("count Memory owners at injected boundary")
            .len(),
        usize::from(memory_committed_before_crash)
    );
    let task_before_restart = task_store
        .canonical_owner_head(&operation_id)
        .expect("read Task owner at injected boundary")
        .expect("Task owner exists");
    assert_eq!(
        task_before_restart.revision(),
        owner_at_final_revision + u64::from(task_committed_before_crash)
    );
    if task_committed_before_crash {
        assert_ne!(task_before_restart.digest(), owner_at_final_digest);
    } else {
        assert_eq!(task_before_restart.digest(), owner_at_final_digest);
    }
    let task_session_before_restart = task_store
        .load_session(&operation_id)
        .expect("read TaskSession at injected boundary")
        .expect("TaskSession exists");
    assert_eq!(
        task_session_before_restart.status,
        if task_committed_before_crash {
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
        } else {
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        }
    );
    assert_eq!(
        task_session_before_restart
            .pending_blockers
            .iter()
            .any(|item| item == &proposal_blocker),
        !task_committed_before_crash
    );
    assert_eq!(
        task_store
            .terminal_owner_transition_receipt_for_claim(&proposal_id, &claim_id)
            .expect("read owner-local receipt at injected boundary")
            .is_some(),
        task_committed_before_crash
    );
    assert_eq!(
        proposal_store
            .get_proposal(&proposal_id)
            .expect("read Proposal at injected boundary")
            .expect("Proposal exists")
            .status,
        ProposalStatus::Pending
    );
    assert_eq!(
        proposal_store
            .dispatch_state(&proposal_id)
            .expect("read dispatch checkpoint at injected boundary")
            .as_deref(),
        Some(if proposal_checkpoint_before_crash {
            "confirmed_projection_pending"
        } else {
            "claimed"
        })
    );
    let successor_count_before_restart = event_store
        .list(&operation_id, 0, 250)
        .expect("read EventStore at injected boundary")
        .iter()
        .filter(|event| event.event_type == "terminal_owner.successor_confirmed")
        .count();
    assert_eq!(
        successor_count_before_restart,
        usize::from(successor_before_crash)
    );

    drop(gateway);
    drop(memory_lifecycle_store);
    drop(memory_store);
    drop(task_store);
    drop(proposal_store);
    drop(event_store);

    let reopened_events = crate::main_chat_event_stream::MainChatAgentEventStore::new(&event_path)
        .expect("reopen EventStore after injected crash");
    let reopened_tasks = AgentTaskSessionStore::new_with_receipt_key(&task_path, receipt_key())
        .expect("reopen TaskSession owner after injected crash");
    let reopened_proposals =
        ProposalStore::new(&proposal_path).expect("reopen ProposalStore after injected crash");
    let reopened_memory = MemoryLifecycleStore::new(&memory_lifecycle_path)
        .expect("reopen Memory lifecycle owner after injected crash");
    assert_eq!(
        reopened_proposals
            .dispatch_claim_id(&proposal_id)
            .expect("read claim after restart")
            .as_deref(),
        Some(claim_id.as_str()),
        "restart must resume the original claim, not mint a second dispatch"
    );
    let reopened_gateway = TerminalOwnerWriteGateway::new(
        &reopened_events,
        &reopened_tasks,
        &reopened_proposals,
        &reopened_memory,
    )
    .with_execution_capture_for_test(capture.clone());
    let reconciled = reopened_gateway
        .reconcile_pending_terminal_owner_successors(32)
        .await
        .expect("reconcile the exact durable boundary after restart");
    assert_eq!(
        reconciled.successors_confirmed,
        usize::from(!successor_before_crash)
    );
    assert_eq!(reconciled.proposals_projected, 1);
    assert_eq!(reconciled.unknown_external_effects_retried, 0);
    assert_eq!(capture.memory_effect_invocations(&proposal_id), 1);
    assert_eq!(capture.task_owner_transition_invocations(&proposal_id), 1);
    assert_eq!(capture.successor_confirmation_invocations(&proposal_id), 1);
    assert_eq!(capture.proposal_projection_invocations(&proposal_id), 1);
    assert_memory_review_converged(
        &reopened_events,
        &reopened_tasks,
        &reopened_proposals,
        &reopened_memory,
        &operation_id,
        &proposal_id,
        &proposal_blocker,
        &final_event_id,
        owner_at_final_revision,
        &owner_at_final_digest,
    );

    let replay = reopened_gateway
        .reconcile_pending_terminal_owner_successors(32)
        .await
        .expect("reconciliation replay remains idempotent");
    assert_eq!(replay.successors_confirmed, 0);
    assert_eq!(replay.proposals_projected, 0);
    assert_eq!(replay.unknown_external_effects_retried, 0);
    assert_eq!(capture.memory_effect_invocations(&proposal_id), 1);
    assert_eq!(capture.task_owner_transition_invocations(&proposal_id), 1);
    assert_eq!(capture.successor_confirmation_invocations(&proposal_id), 1);
    assert_eq!(capture.proposal_projection_invocations(&proposal_id), 1);
}

#[tokio::test]
async fn d055_target_reopen_after_claim_before_effect_executes_each_stage_once() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        run_memory_crash_reconciliation_contract(
            "claim-before-effect",
            TerminalOwnerCrashPoint::AfterClaimPersistedBeforeEffect,
            false,
            false,
            false,
            false,
        )
        .await;
    })
    .await
    .expect("claim-before-effect crash scenario exceeded its outer timeout");
}

#[tokio::test]
async fn d055_target_reopen_after_memory_before_task_does_not_repeat_memory() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        run_memory_crash_reconciliation_contract(
            "memory-before-task",
            TerminalOwnerCrashPoint::AfterMemoryCommittedBeforeTaskOwner,
            true,
            false,
            false,
            false,
        )
        .await;
    })
    .await
    .expect("memory-before-task crash scenario exceeded its outer timeout");
}

#[tokio::test]
async fn d055_target_reopen_after_task_receipt_before_proposal_checkpoint_does_not_repeat_owners() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        run_memory_crash_reconciliation_contract(
            "task-before-proposal-checkpoint",
            TerminalOwnerCrashPoint::AfterTaskOwnerReceiptBeforeProposalCheckpoint,
            true,
            true,
            false,
            false,
        )
        .await;
    })
    .await
    .expect("task-before-proposal-checkpoint scenario exceeded its outer timeout");
}

#[tokio::test]
async fn d055_target_reopen_after_proposal_checkpoint_before_successor_adds_one_successor() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        run_memory_crash_reconciliation_contract(
            "proposal-checkpoint-before-successor",
            TerminalOwnerCrashPoint::AfterProposalCheckpointBeforeSuccessor,
            true,
            true,
            true,
            false,
        )
        .await;
    })
    .await
    .expect("proposal-checkpoint-before-successor scenario exceeded its outer timeout");
}

#[tokio::test]
async fn d055_target_reopen_after_successor_before_projection_does_not_duplicate_successor() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        run_memory_crash_reconciliation_contract(
            "successor-before-projection",
            TerminalOwnerCrashPoint::AfterSuccessorBeforeProposalProjection,
            true,
            true,
            true,
            true,
        )
        .await;
    })
    .await
    .expect("successor-before-projection scenario exceeded its outer timeout");
}

#[tokio::test]
async fn d055_target_terminal_origin_rejects_mismatch_foreign_tombstone_and_rebind_without_minting_replay_epoch(
) {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        let temp = tempfile::tempdir().expect("D055 typed-origin negative temp directory");
        let task_store = AgentTaskSessionStore::new_with_receipt_key(
            temp.path().join("task-owner.sqlite"),
            receipt_key(),
        )
        .expect("real TaskSession owner");
        let canonical_store = MemoryStore::new(temp.path().join("conversation-owner.sqlite"))
            .expect("real canonical Conversation owner");
        let foreign_store = MemoryStore::new(temp.path().join("foreign-conversation-owner.sqlite"))
            .expect("independent canonical Conversation owner");
        let event_store = crate::main_chat_event_stream::MainChatAgentEventStore::new(
            temp.path().join("turn-events.sqlite"),
        )
        .expect("real EventStore");
        task_store
            .bind_canonical_memory_store(&canonical_store)
            .expect("TaskSession binds one canonical Conversation store identity");

        let operation_id = uuid::Uuid::new_v4().to_string();
        let session_id = "d055-origin-valid";
        let commit = commit_file_backed_user_message(
            &canonical_store,
            &operation_id,
            session_id,
            SENSITIVE_MEMORY_BODY,
        );
        let cloned_commit = commit.clone();
        create_file_backed_task(
            &task_store,
            &operation_id,
            session_id,
            SENSITIVE_MEMORY_BODY,
        );
        task_store
            .bind_session_canonical_user_message(
                &operation_id,
                &commit.receipt().canonical_ref,
                SENSITIVE_MEMORY_BODY,
            )
            .expect("bind valid canonical user message");

        let wrong_operation = task_store
            .issue_terminal_owner_epoch_admission(
                &operation_id,
                "different-run-operation",
                commit.clone(),
            )
            .expect_err("message operation cannot authorize a different run");
        assert_eq!(
            wrong_operation.to_string(),
            "terminal_origin_operation_mismatch"
        );

        let wrong_task_id = uuid::Uuid::new_v4().to_string();
        create_file_backed_task(
            &task_store,
            &wrong_task_id,
            session_id,
            SENSITIVE_MEMORY_BODY,
        );
        let wrong_task = task_store
            .issue_terminal_owner_epoch_admission(&wrong_task_id, &operation_id, commit.clone())
            .expect_err("canonical message cannot authorize a different TaskSession owner");
        assert_eq!(
            wrong_task.to_string(),
            "terminal_origin_task_owner_mismatch"
        );

        let session_mismatch_operation = uuid::Uuid::new_v4().to_string();
        let session_mismatch_commit = commit_file_backed_user_message(
            &canonical_store,
            &session_mismatch_operation,
            "d055-origin-message-session",
            SENSITIVE_MEMORY_BODY,
        );
        create_file_backed_task(
            &task_store,
            &session_mismatch_operation,
            "d055-origin-task-session",
            SENSITIVE_MEMORY_BODY,
        );
        let wrong_session = task_store
            .issue_terminal_owner_epoch_admission(
                &session_mismatch_operation,
                &session_mismatch_operation,
                session_mismatch_commit,
            )
            .expect_err("canonical message session must equal the TaskSession chat owner");
        assert_eq!(
            wrong_session.to_string(),
            "terminal_origin_session_mismatch"
        );

        let foreign_operation = uuid::Uuid::new_v4().to_string();
        let foreign_commit = commit_file_backed_user_message(
            &foreign_store,
            &foreign_operation,
            "d055-origin-foreign-store",
            SENSITIVE_MEMORY_BODY,
        );
        create_file_backed_task(
            &task_store,
            &foreign_operation,
            "d055-origin-foreign-store",
            SENSITIVE_MEMORY_BODY,
        );
        let foreign_identity = task_store
            .issue_terminal_owner_epoch_admission(
                &foreign_operation,
                &foreign_operation,
                foreign_commit,
            )
            .expect_err("proof from an unbound canonical store identity is rejected");
        assert_eq!(
            foreign_identity.to_string(),
            "terminal_origin_canonical_store_identity_mismatch"
        );

        let tombstoned_operation = uuid::Uuid::new_v4().to_string();
        let tombstoned_session = "d055-origin-tombstoned";
        let tombstoned_commit = commit_file_backed_user_message(
            &canonical_store,
            &tombstoned_operation,
            tombstoned_session,
            SENSITIVE_MEMORY_BODY,
        );
        create_file_backed_task(
            &task_store,
            &tombstoned_operation,
            tombstoned_session,
            SENSITIVE_MEMORY_BODY,
        );
        canonical_store
            .delete_chat_session_with_tombstone(tombstoned_session, Some("d055_stale_origin"))
            .expect("tombstone canonical Conversation before terminal admission");
        let stale = task_store
            .issue_terminal_owner_epoch_admission(
                &tombstoned_operation,
                &tombstoned_operation,
                tombstoned_commit,
            )
            .expect_err("tombstoned canonical message cannot authorize a terminal epoch");
        assert_eq!(
            stale.to_string(),
            "terminal_origin_canonical_message_inactive"
        );

        let admission = task_store
            .issue_terminal_owner_epoch_admission(&operation_id, &operation_id, commit)
            .expect("valid opaque canonical commit authorizes one TaskSession epoch");
        assert!(!admission.replayed());
        let admission_id = admission.admission_id().to_string();
        let epoch = event_store
            .open_terminal_owner_epoch_from_admission(admission)
            .expect("EventStore consumes valid non-Serde admission once");
        assert_eq!(epoch.task_session_id(), operation_id);
        assert_eq!(epoch.run_id(), operation_id);
        assert_eq!(epoch.state(), TerminalOwnerSealState::Open);
        assert!(!epoch.replayed());
        let epoch_id = epoch.epoch_id().to_string();
        let epoch_generation = epoch.generation();

        let cloned_replay_admission = task_store
            .issue_terminal_owner_epoch_admission(
                &operation_id,
                &operation_id,
                cloned_commit.clone(),
            )
            .expect("exact clone recovers the existing durable admission");
        assert!(cloned_replay_admission.replayed());
        assert_eq!(cloned_replay_admission.admission_id(), admission_id);
        let cloned_replay_epoch = event_store
            .open_terminal_owner_epoch_from_admission(cloned_replay_admission)
            .expect("exact clone recovers, rather than mints, one epoch");
        assert!(cloned_replay_epoch.replayed());
        assert_eq!(cloned_replay_epoch.epoch_id(), epoch_id);
        assert_eq!(cloned_replay_epoch.generation(), epoch_generation);

        let rebound_task = uuid::Uuid::new_v4().to_string();
        create_file_backed_task(
            &task_store,
            &rebound_task,
            session_id,
            SENSITIVE_MEMORY_BODY,
        );
        let rebound = task_store
            .issue_terminal_owner_epoch_admission(
                &rebound_task,
                &operation_id,
                cloned_commit.clone(),
            )
            .expect_err("consumed commit cannot be rebound to another TaskSession owner");
        assert_eq!(
            rebound.to_string(),
            "terminal_origin_commit_owner_rebind_forbidden"
        );

        event_store
            .begin_terminal_owner_seal(&operation_id, &operation_id, epoch_generation)
            .expect("seal the one valid epoch");
        let owner = task_store
            .canonical_owner_head(&operation_id)
            .expect("read valid Task owner")
            .expect("valid Task owner exists");
        let final_fact = event_store
            .append_terminal_final_and_seal(MainChatTerminalFinalizationInput {
                task_session_id: operation_id.clone(),
                run_id: operation_id.clone(),
                epoch_generation,
                delivery_id: format!("delivery:{operation_id}:{operation_id}"),
                expected_task_owner_revision: owner.revision(),
                expected_task_owner_digest: owner.digest().to_string(),
                status: "completed".into(),
            })
            .expect("commit final and SEALED state for the one epoch");
        let replayed_commit = canonical_store
            .save_message_idempotent_with_proof(
                session_id,
                &ChatMessage {
                    role: "user".into(),
                    content: SENSITIVE_MEMORY_BODY.into(),
                },
                &operation_id,
            )
            .expect("canonical owner returns an opaque exact replay");
        assert!(replayed_commit.receipt().replayed);
        let replayed_admission = task_store
            .issue_terminal_owner_epoch_admission(&operation_id, &operation_id, replayed_commit)
            .expect("canonical replay recovers the existing admission");
        assert!(replayed_admission.replayed());
        assert_eq!(replayed_admission.admission_id(), admission_id);
        let replayed_epoch = event_store
            .open_terminal_owner_epoch_from_admission(replayed_admission)
            .expect("sealed replay recovers existing epoch without a new generation");
        assert!(replayed_epoch.replayed());
        assert_eq!(replayed_epoch.epoch_id(), epoch_id);
        assert_eq!(replayed_epoch.generation(), epoch_generation);
        assert_eq!(replayed_epoch.state(), TerminalOwnerSealState::Sealed);
        assert_eq!(
            replayed_epoch.final_event_id(),
            Some(final_fact.event_id.as_str())
        );
    })
    .await
    .expect("D055 typed-origin negative scenario exceeded its outer timeout");
}

#[tokio::test]
async fn d055_target_unknown_external_dispatch_capture_is_not_called_by_restart_reconciliation() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        let SealedReviewScenario {
            _temp,
            event_path,
            task_path,
            proposal_path,
            memory_lifecycle_path,
            operation_id,
            proposal_id,
            proposal_blocker,
            final_event_id,
            owner_at_final_revision,
            owner_at_final_digest,
            event_store,
            task_store,
            proposal_store,
            memory_store,
            memory_lifecycle_store,
        } = setup_sealed_review_scenario(
            "unknown-external-capture",
            external_unknown_review_request(),
        );
        let claim_id = proposal_store
            .claim_dispatch(&proposal_id)
            .expect("claim exact external Proposal")
            .expect("one external Review claimant");
        let acceptance = ReviewWorkflow::new(&proposal_store)
            .claimed_acceptance_snapshot(&proposal_id, &claim_id)
            .expect("claim yields non-Serde external acceptance authority");
        let dispatch = RecordingUnknownExternalDispatch::default();
        let gateway = TerminalOwnerWriteGateway::new(
            &event_store,
            &task_store,
            &proposal_store,
            &memory_lifecycle_store,
        )
        .with_external_dispatch_adapter(Arc::new(dispatch.clone()));
        let error = gateway
            .apply_claimed_review_acceptance(acceptance)
            .await
            .expect_err("unqueryable remote result remains typed unknown");
        assert_eq!(
            error.to_string(),
            "terminal_owner_external_effect_remote_unknown"
        );
        assert_eq!(dispatch.call_count(), 1);
        assert_eq!(dispatch.proposal_ids(), vec![proposal_id.clone()]);
        assert_eq!(
            proposal_store
                .dispatch_claim_id(&proposal_id)
                .expect("read external claim")
                .as_deref(),
            Some(claim_id.as_str())
        );
        assert_eq!(
            proposal_store
                .dispatch_state(&proposal_id)
                .expect("read external dispatch truth")
                .as_deref(),
            Some("unknown")
        );
        assert_eq!(
            proposal_store
                .get_proposal(&proposal_id)
                .expect("read unknown Proposal")
                .expect("unknown Proposal exists")
                .status,
            ProposalStatus::Pending
        );
        assert!(memory_lifecycle_store
            .get_record_by_proposal_id(&proposal_id)
            .expect("read Memory owner for external counterexample")
            .is_none());
        let task_before_restart = task_store
            .canonical_owner_head(&operation_id)
            .expect("read Task owner after external unknown")
            .expect("Task owner exists");
        assert_eq!(task_before_restart.revision(), owner_at_final_revision);
        assert_eq!(task_before_restart.digest(), owner_at_final_digest);
        let task_session = task_store
            .load_session(&operation_id)
            .expect("read blocked TaskSession")
            .expect("blocked TaskSession exists");
        assert_eq!(
            task_session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        );
        assert!(task_session.pending_blockers.contains(&proposal_blocker));
        let events_before_restart = event_store
            .list(&operation_id, 0, 250)
            .expect("read EventStore before unknown restart");
        assert_eq!(
            events_before_restart
                .iter()
                .filter(|event| event.event_type == "final_delivery.created")
                .count(),
            1
        );
        assert_eq!(
            events_before_restart
                .iter()
                .filter(|event| event.event_type == "terminal_owner.successor_confirmed")
                .count(),
            0
        );

        drop(gateway);
        drop(memory_lifecycle_store);
        drop(memory_store);
        drop(task_store);
        drop(proposal_store);
        drop(event_store);

        let reopened_events =
            crate::main_chat_event_stream::MainChatAgentEventStore::new(&event_path)
                .expect("reopen EventStore after external unknown");
        let reopened_tasks = AgentTaskSessionStore::new_with_receipt_key(&task_path, receipt_key())
            .expect("reopen TaskSession store after external unknown");
        let reopened_proposals = ProposalStore::new(&proposal_path)
            .expect("reopen ProposalStore after external unknown");
        let reopened_memory = MemoryLifecycleStore::new(&memory_lifecycle_path)
            .expect("reopen Memory lifecycle store after external unknown");
        let reopened_gateway = TerminalOwnerWriteGateway::new(
            &reopened_events,
            &reopened_tasks,
            &reopened_proposals,
            &reopened_memory,
        )
        .with_external_dispatch_adapter(Arc::new(dispatch.clone()));
        for pass in 0..2 {
            let report = reopened_gateway
                .reconcile_pending_terminal_owner_successors(32)
                .await
                .expect("unknown reconciliation remains read-only toward the adapter");
            assert_eq!(report.unknown_external_effects_retried, 0, "pass={pass}");
            assert_eq!(report.successors_confirmed, 0, "pass={pass}");
            assert_eq!(report.proposals_projected, 0, "pass={pass}");
            assert_eq!(dispatch.call_count(), 1, "pass={pass}");
            assert_eq!(dispatch.proposal_ids(), vec![proposal_id.clone()]);
        }
        assert_eq!(
            reopened_proposals
                .dispatch_state(&proposal_id)
                .expect("read external unknown after restart")
                .as_deref(),
            Some("unknown")
        );
        assert_eq!(
            reopened_proposals
                .dispatch_claim_id(&proposal_id)
                .expect("read external claim after restart")
                .as_deref(),
            Some(claim_id.as_str())
        );
        let reopened_task_owner = reopened_tasks
            .canonical_owner_head(&operation_id)
            .expect("read Task owner after unknown reconciliation")
            .expect("Task owner still exists");
        assert_eq!(reopened_task_owner.revision(), owner_at_final_revision);
        assert_eq!(reopened_task_owner.digest(), owner_at_final_digest);
        let reopened_events_all = reopened_events
            .list(&operation_id, 0, 250)
            .expect("read terminal facts after unknown reconciliation");
        let finals = reopened_events_all
            .iter()
            .filter(|event| event.event_type == "final_delivery.created")
            .collect::<Vec<_>>();
        assert_eq!(finals.len(), 1);
        assert_eq!(finals[0].event_id, final_event_id);
        assert!(reopened_events_all
            .iter()
            .all(|event| event.event_type != "terminal_owner.successor_confirmed"));
    })
    .await
    .expect("D055 unknown external reconciliation scenario exceeded its outer timeout");
}

fn memory_review_request() -> DurableWriteRequest {
    let mut proposal = AgentProposal::new(
        ProposalType::MemoryWrite,
        "memory.pending.chat_conversation",
        serde_json::json!({
            "content": "Coffee on an empty stomach causes heart palpitations.",
            "scope": "global",
            "category": "fact",
            "riskLevel": "medium",
            "sensitivity": "sensitive",
            "candidateKind": "semantic_user_fact",
            "source": "chat_explicit"
        }),
        "Sensitive Memory remains pending until an exact Review Center acceptance.",
        1.0,
        RiskLevel::Medium,
        ProposalSource::ChatConversation,
    );
    // Origin authority is deliberately absent from caller-shaped fields.
    proposal.run_id = None;
    proposal.source_detail = None;
    DurableWriteRequest::from_agent_proposal(
        DurableWriteSource::MainChat,
        DurableWriteSubject::Memory,
        proposal,
        "Sensitive Memory remains pending Review Center approval.",
    )
}

#[tokio::test]
async fn d055_target_file_backed_successor_uses_verified_owner_local_receipt() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        let event_store_source = include_str!("main_chat_event_stream.rs");
        for forbidden_naked_minter in [
            "fn issue_review_origin_proof(",
            "fn open_terminal_owner_epoch(",
        ] {
            assert!(
                !event_store_source.contains(forbidden_naked_minter),
                "terminal origin authority must not be constructible from caller ids/free text: {forbidden_naked_minter}"
            );
        }
        let temp = tempfile::tempdir().expect("D055 temp directory");
        let event_path = temp.path().join("turn-events.sqlite");
        let task_path = temp.path().join("task-owner.sqlite");
        let proposal_path = temp.path().join("proposal-owner.sqlite");
        let conversation_path = temp.path().join("conversation-owner.sqlite");
        let memory_lifecycle_path = temp.path().join("memory-lifecycle-owner.sqlite");
        let operation_id = uuid::Uuid::new_v4().to_string();
        let chat_session_id = "d055-file-backed-successor";

        let event_store = crate::main_chat_event_stream::MainChatAgentEventStore::new(&event_path)
            .expect("real file-backed EventStore");
        let task_store = AgentTaskSessionStore::new_with_receipt_key(&task_path, receipt_key())
            .expect("real file-backed TaskSession store");
        let proposal_store =
            ProposalStore::new(&proposal_path).expect("real file-backed ProposalStore");
        let memory_store =
            MemoryStore::new(&conversation_path).expect("real file-backed Conversation owner");
        let memory_lifecycle_store = MemoryLifecycleStore::new(&memory_lifecycle_path)
            .expect("real file-backed Memory lifecycle owner");
        let canonical_message = commit_file_backed_user_message(
            &memory_store,
            &operation_id,
            chat_session_id,
            SENSITIVE_MEMORY_BODY,
        );
        let canonical_receipt = canonical_message.receipt().clone();
        create_file_backed_task(
            &task_store,
            &operation_id,
            chat_session_id,
            SENSITIVE_MEMORY_BODY,
        );
        task_store
            .bind_canonical_memory_store(&memory_store)
            .expect("TaskSession store binds the canonical Conversation owner");
        task_store
            .bind_session_canonical_user_message(
                &operation_id,
                &canonical_receipt.canonical_ref,
                SENSITIVE_MEMORY_BODY,
            )
            .expect("TaskSession binds the exact canonical user message");

        let terminal_admission = task_store
            .issue_terminal_owner_epoch_admission(
                &operation_id,
                &operation_id,
                canonical_message,
            )
            .expect("TaskSession owner consumes and verifies the opaque canonical-message commit");
        let epoch = event_store
            .open_terminal_owner_epoch_from_admission(terminal_admission)
            .expect("EventStore accepts only the non-Serde TaskSession admission");
        assert_eq!(
            epoch.canonical_user_message_ref(),
            canonical_receipt.canonical_ref
        );
        assert_eq!(
            epoch.canonical_user_message_digest(),
            canonical_receipt.content_digest
        );
        let origin_proof = epoch
            .review_origin_proof()
            .expect("bound epoch carries one non-Serde Review origin proof");
        assert_eq!(origin_proof.task_session_id(), operation_id);
        assert_eq!(origin_proof.run_id(), operation_id);
        assert_eq!(
            origin_proof.canonical_user_message_ref(),
            canonical_receipt.canonical_ref
        );
        assert_eq!(
            origin_proof.canonical_user_message_digest(),
            canonical_receipt.content_digest
        );
        let staged = ReviewWorkflow::new(&proposal_store)
            .submit_with_terminal_owner_origin(memory_review_request(), origin_proof)
            .expect("ReviewWorkflow persists Proposal and typed origin binding");
        let proposal_id = staged.proposal.id.clone();
        assert_eq!(staged.proposal.status, ProposalStatus::Pending);
        assert!(staged.proposal.source_detail.is_none());
        assert!(staged.proposal.run_id.is_none());
        assert!(staged
            .proposal
            .after
            .get("originatingTaskSessionId")
            .is_none());
        let proposal_blocker = format!("proposal:{proposal_id}");
        task_store
            .set_pending_blockers(&operation_id, vec![proposal_blocker.clone()])
            .expect("legitimate pre-terminal setup attaches the exact Proposal blocker");
        task_store
            .mark_waiting_permission(&operation_id)
            .expect("legitimate pre-terminal setup enters WaitingPermission");
        let blocked_owner = task_store
            .load_session(&operation_id)
            .expect("read blocked TaskSession")
            .expect("blocked TaskSession exists");
        assert_eq!(
            blocked_owner.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        );
        assert_eq!(
            blocked_owner.pending_blockers,
            vec![proposal_blocker.clone()]
        );

        let gateway = TerminalOwnerWriteGateway::new(
            &event_store,
            &task_store,
            &proposal_store,
            &memory_lifecycle_store,
        );
        let seal = event_store
            .begin_terminal_owner_seal(&operation_id, &operation_id, epoch.generation())
            .expect("OPEN -> SEALING CAS");
        assert_eq!(seal.state(), TerminalOwnerSealState::Sealing);
        let owner_at_final = task_store
            .canonical_owner_head(&operation_id)
            .expect("TaskSession owner head")
            .expect("TaskSession owner exists");
        assert!(owner_at_final.revision() > 0);
        let final_fact = event_store
            .append_terminal_final_and_seal(MainChatTerminalFinalizationInput {
                task_session_id: operation_id.clone(),
                run_id: operation_id.clone(),
                epoch_generation: epoch.generation(),
                delivery_id: format!("delivery:{operation_id}:{operation_id}"),
                expected_task_owner_revision: owner_at_final.revision(),
                expected_task_owner_digest: owner_at_final.digest().to_string(),
                status: "waiting_permission".into(),
            })
            .expect("final insert and SEALED CAS commit atomically");

        let dispatch_claim_id = proposal_store
            .claim_dispatch(&proposal_id)
            .expect("claim exact Proposal through the real ProposalStore")
            .expect("one Review Center claimant");
        let verified_acceptance = ReviewWorkflow::new(&proposal_store)
            .claimed_acceptance_snapshot(&proposal_id, &dispatch_claim_id)
            .expect("ReviewWorkflow issues non-Serde verified acceptance authority");
        let transition = gateway
            .apply_claimed_review_acceptance(verified_acceptance)
            .await
            .expect("post-seal review acceptance commits one legal successor");
        assert_eq!(transition.before_owner_revision, owner_at_final.revision());
        assert_eq!(
            transition.after_owner_revision,
            owner_at_final.revision() + 1
        );
        assert_eq!(transition.before_owner_digest, owner_at_final.digest());
        assert_ne!(
            transition.after_owner_digest,
            transition.before_owner_digest
        );
        assert!(!transition.local_transition_receipt_ref.is_empty());
        assert!(!transition.local_transition_receipt_digest.is_empty());
        assert!(!transition.successor_event_id.is_empty());
        assert_eq!(
            proposal_store
                .get_proposal(&proposal_id)
                .expect("read Proposal after gateway commit")
                .expect("Proposal remains present")
                .status,
            ProposalStatus::Accepted
        );
        assert_eq!(
            proposal_store
                .dispatch_state(&proposal_id)
                .expect("read dispatch after gateway commit")
                .as_deref(),
            Some("confirmed")
        );
        let committed_memory = memory_lifecycle_store
            .get_record_by_proposal_id(&proposal_id)
            .expect("read Memory effect")
            .expect("gateway committed the actual Memory effect");
        assert_eq!(committed_memory.proposal_id, proposal_id);

        drop(gateway);
        drop(memory_lifecycle_store);
        drop(memory_store);
        drop(task_store);
        drop(proposal_store);
        drop(event_store);

        let reopened_task = AgentTaskSessionStore::open_read_only_existing_with_receipt_key(
            &task_path,
            receipt_key(),
        )
        .expect("reopen the real TaskSession owner store");
        let local_receipt = reopened_task
            .verified_terminal_owner_transition_receipt(&transition.local_transition_receipt_ref)
            .expect("verify owner-local transition receipt")
            .expect("owner-local receipt persists across reopen");
        assert_eq!(local_receipt.owner_kind(), "agent_task_session");
        assert_eq!(local_receipt.owner_id(), operation_id);
        assert_eq!(
            local_receipt.before_revision(),
            transition.before_owner_revision
        );
        assert_eq!(
            local_receipt.after_revision(),
            transition.after_owner_revision
        );
        assert_eq!(
            local_receipt.before_digest(),
            transition.before_owner_digest
        );
        assert_eq!(local_receipt.after_digest(), transition.after_owner_digest);
        assert_eq!(
            local_receipt.receipt_ref(),
            transition.local_transition_receipt_ref
        );
        assert_eq!(
            local_receipt.receipt_digest(),
            transition.local_transition_receipt_digest
        );
        let reopened_session = reopened_task
            .load_session(&operation_id)
            .expect("read reopened TaskSession")
            .expect("reopened TaskSession exists");
        assert_eq!(
            reopened_session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
        );
        assert!(
            !reopened_session
                .pending_blockers
                .contains(&proposal_blocker),
            "the verified transition must remove the exact real Proposal blocker"
        );
        let reopened_head = reopened_task
            .canonical_owner_head(&operation_id)
            .expect("read reopened owner head")
            .expect("reopened owner head exists");
        assert_eq!(reopened_head.revision(), transition.after_owner_revision);
        assert_eq!(reopened_head.digest(), transition.after_owner_digest);
        assert_ne!(local_receipt.before_digest(), local_receipt.after_digest());
        let reopened_memory = MemoryLifecycleStore::open_read_only_existing(&memory_lifecycle_path)
            .expect("reopen real Memory lifecycle owner");
        let reopened_memory_record = reopened_memory
            .get_record_by_proposal_id(&proposal_id)
            .expect("read reopened Memory effect")
            .expect("Memory effect remains durable after reopen");
        assert_eq!(reopened_memory_record.memory_id, committed_memory.memory_id);
        assert_eq!(
            reopened_memory
                .list_active_records(None, 100)
                .expect("count active Memory records")
                .len(),
            1
        );

        let reopened_events =
            crate::main_chat_event_stream::MainChatAgentEventStore::new(&event_path)
                .expect("reopen real EventStore");
        let events = reopened_events
            .list(&operation_id, 0, 250)
            .expect("read exact durable facts after reopen");
        let finals = events
            .iter()
            .filter(|event| event.event_type == "final_delivery.created")
            .collect::<Vec<_>>();
        let successors = events
            .iter()
            .filter(|event| event.event_type == "terminal_owner.successor_confirmed")
            .collect::<Vec<_>>();
        assert_eq!(finals.len(), 1);
        assert_eq!(successors.len(), 1);
        assert_eq!(finals[0].event_id, final_fact.event_id);
        assert_eq!(successors[0].event_id, transition.successor_event_id);
        assert_eq!(successors[0].task_session_id, operation_id);
        assert_eq!(successors[0].run_id, operation_id);
        assert_eq!(successors[0].object_type, "terminal_owner_successor");
        assert_eq!(
            successors[0].source,
            "terminal_owner_write_gateway.review_successor"
        );
        assert_eq!(
            successors[0].payload["causeKind"],
            "proposal_review_acceptance"
        );
        assert_eq!(successors[0].payload["causeRef"], proposal_id);
        assert_eq!(successors[0].payload["finalEventId"], final_fact.event_id);
        assert_eq!(successors[0].payload["ownerKind"], "agent_task_session");
        assert_eq!(successors[0].payload["ownerId"], operation_id);
        assert_eq!(
            successors[0].payload["beforeOwnerRevision"],
            transition.before_owner_revision
        );
        assert_eq!(
            successors[0].payload["afterOwnerRevision"],
            transition.after_owner_revision
        );
        assert_eq!(
            successors[0].payload["beforeOwnerDigest"],
            transition.before_owner_digest
        );
        assert_eq!(
            successors[0].payload["afterOwnerDigest"],
            transition.after_owner_digest
        );
        assert_eq!(
            successors[0].payload["localTransitionReceiptRef"],
            transition.local_transition_receipt_ref
        );
        assert_eq!(
            successors[0].payload["localTransitionReceiptDigest"],
            transition.local_transition_receipt_digest
        );
    })
    .await
    .expect("D055 file-backed successor scenario exceeded its outer timeout");
}

#[tokio::test]
async fn d055_target_cross_store_owner_commit_reconciles_successor_exactly_once_after_restart() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        let temp = tempfile::tempdir().expect("D055 reconciliation temp directory");
        let event_path = temp.path().join("turn-events.sqlite");
        let task_path = temp.path().join("task-owner.sqlite");
        let proposal_path = temp.path().join("proposal-owner.sqlite");
        let conversation_path = temp.path().join("conversation-owner.sqlite");
        let memory_lifecycle_path = temp.path().join("memory-lifecycle-owner.sqlite");
        let operation_id = uuid::Uuid::new_v4().to_string();
        let chat_session_id = "d055-cross-store-reconcile";

        let event_store = crate::main_chat_event_stream::MainChatAgentEventStore::new(&event_path)
            .expect("real file-backed EventStore");
        let task_store = AgentTaskSessionStore::new_with_receipt_key(&task_path, receipt_key())
            .expect("real file-backed TaskSession store");
        let proposal_store =
            ProposalStore::new(&proposal_path).expect("real file-backed ProposalStore");
        let memory_store =
            MemoryStore::new(&conversation_path).expect("real file-backed Conversation owner");
        let memory_lifecycle_store = MemoryLifecycleStore::new(&memory_lifecycle_path)
            .expect("real file-backed Memory lifecycle owner");
        let canonical_message = commit_file_backed_user_message(
            &memory_store,
            &operation_id,
            chat_session_id,
            SENSITIVE_MEMORY_BODY,
        );
        let canonical_receipt = canonical_message.receipt().clone();
        create_file_backed_task(
            &task_store,
            &operation_id,
            chat_session_id,
            SENSITIVE_MEMORY_BODY,
        );
        task_store
            .bind_canonical_memory_store(&memory_store)
            .expect("TaskSession store binds canonical Conversation");
        task_store
            .bind_session_canonical_user_message(
                &operation_id,
                &canonical_receipt.canonical_ref,
                SENSITIVE_MEMORY_BODY,
            )
            .expect("TaskSession binds exact canonical user message");

        let terminal_admission = task_store
            .issue_terminal_owner_epoch_admission(&operation_id, &operation_id, canonical_message)
            .expect("TaskSession owner verifies canonical-message authority");
        let epoch = event_store
            .open_terminal_owner_epoch_from_admission(terminal_admission)
            .expect("open message-bound terminal epoch from non-Serde admission");
        assert_eq!(
            epoch.canonical_user_message_ref(),
            canonical_receipt.canonical_ref
        );
        assert_eq!(
            epoch.canonical_user_message_digest(),
            canonical_receipt.content_digest
        );
        let origin = epoch
            .review_origin_proof()
            .expect("epoch exposes only its bound non-Serde origin proof");
        assert_eq!(
            origin.canonical_user_message_ref(),
            canonical_receipt.canonical_ref
        );
        assert_eq!(
            origin.canonical_user_message_digest(),
            canonical_receipt.content_digest
        );
        let staged = ReviewWorkflow::new(&proposal_store)
            .submit_with_terminal_owner_origin(memory_review_request(), origin)
            .expect("persist Proposal with immutable typed origin");
        let proposal_id = staged.proposal.id.clone();
        let blocker = format!("proposal:{proposal_id}");
        task_store
            .set_pending_blockers(&operation_id, vec![blocker.clone()])
            .expect("attach exact Proposal blocker");
        task_store
            .mark_waiting_permission(&operation_id)
            .expect("enter WaitingPermission before terminalization");
        event_store
            .begin_terminal_owner_seal(&operation_id, &operation_id, epoch.generation())
            .expect("OPEN -> SEALING CAS");
        let owner_at_final = task_store
            .canonical_owner_head(&operation_id)
            .expect("read Task owner head")
            .expect("Task owner exists");
        let final_fact = event_store
            .append_terminal_final_and_seal(MainChatTerminalFinalizationInput {
                task_session_id: operation_id.clone(),
                run_id: operation_id.clone(),
                epoch_generation: epoch.generation(),
                delivery_id: format!("delivery:{operation_id}:{operation_id}"),
                expected_task_owner_revision: owner_at_final.revision(),
                expected_task_owner_digest: owner_at_final.digest().to_string(),
                status: "waiting_permission".into(),
            })
            .expect("commit final and SEALED epoch");

        let gateway = TerminalOwnerWriteGateway::new(
            &event_store,
            &task_store,
            &proposal_store,
            &memory_lifecycle_store,
        );
        let claim_id = proposal_store
            .claim_dispatch(&proposal_id)
            .expect("claim exact Proposal")
            .expect("one Review claimant");
        let acceptance = ReviewWorkflow::new(&proposal_store)
            .claimed_acceptance_snapshot(&proposal_id, &claim_id)
            .expect("claim yields non-Serde verified acceptance");
        gateway
            .install_crash_point_for_test(
                &proposal_id,
                TerminalOwnerCrashPoint::AfterProposalCheckpointBeforeSuccessor,
            )
            .expect("install the cross-SQLite crash failpoint");
        let error = gateway
            .apply_claimed_review_acceptance(acceptance)
            .await
            .expect_err("simulate crash after owner commit but before successor confirmation");
        assert_eq!(
            error.to_string(),
            "injected_terminal_owner_crash:after_proposal_checkpoint_before_successor"
        );

        let committed_memory = memory_lifecycle_store
            .get_record_by_proposal_id(&proposal_id)
            .expect("read Memory owner after injected crash")
            .expect("Memory effect committed exactly once before the crash");
        assert_eq!(
            memory_lifecycle_store
                .list_active_records(None, 100)
                .expect("count Memory owners after injected crash")
                .len(),
            1
        );
        let owner_after_crash = task_store
            .canonical_owner_head(&operation_id)
            .expect("read Task owner after injected crash")
            .expect("Task owner still exists");
        assert_eq!(owner_after_crash.revision(), owner_at_final.revision() + 1);
        assert_ne!(owner_after_crash.digest(), owner_at_final.digest());
        let session_after_crash = task_store
            .load_session(&operation_id)
            .expect("read TaskSession after injected crash")
            .expect("TaskSession exists");
        assert_eq!(
            session_after_crash.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
        );
        assert!(!session_after_crash.pending_blockers.contains(&blocker));
        let local_receipt = task_store
            .terminal_owner_transition_receipt_for_claim(&proposal_id, &claim_id)
            .expect("read owner-local receipt by durable Proposal claim")
            .expect("owner-local receipt committed before EventStore confirmation");
        assert_eq!(local_receipt.owner_id(), operation_id);
        assert_eq!(local_receipt.before_revision(), owner_at_final.revision());
        assert_eq!(local_receipt.after_revision(), owner_after_crash.revision());
        assert_eq!(local_receipt.before_digest(), owner_at_final.digest());
        assert_eq!(local_receipt.after_digest(), owner_after_crash.digest());
        assert_eq!(
            proposal_store
                .get_proposal(&proposal_id)
                .expect("read Proposal after injected crash")
                .expect("Proposal exists")
                .status,
            ProposalStatus::Pending,
            "Proposal read model cannot claim accepted before successor confirmation"
        );
        assert_eq!(
            proposal_store
                .dispatch_state(&proposal_id)
                .expect("read durable effect truth")
                .as_deref(),
            Some("confirmed_projection_pending")
        );
        assert!(event_store
            .list(&operation_id, 0, 250)
            .expect("read events before restart")
            .iter()
            .all(|event| event.event_type != "terminal_owner.successor_confirmed"));

        let local_receipt_ref = local_receipt.receipt_ref().to_string();
        let local_receipt_digest = local_receipt.receipt_digest().to_string();
        let memory_id = committed_memory.memory_id.clone();
        let owner_revision = owner_after_crash.revision();
        let owner_digest = owner_after_crash.digest().to_string();
        drop(gateway);
        drop(memory_lifecycle_store);
        drop(memory_store);
        drop(task_store);
        drop(proposal_store);
        drop(event_store);

        let reopened_events =
            crate::main_chat_event_stream::MainChatAgentEventStore::new(&event_path)
                .expect("reopen EventStore after simulated process crash");
        let reopened_tasks = AgentTaskSessionStore::new_with_receipt_key(&task_path, receipt_key())
            .expect("reopen TaskSession owner after simulated process crash");
        let reopened_proposals = ProposalStore::new(&proposal_path)
            .expect("reopen ProposalStore after simulated process crash");
        let reopened_memory = MemoryLifecycleStore::new(&memory_lifecycle_path)
            .expect("reopen Memory lifecycle owner after simulated process crash");
        let reopened_gateway = TerminalOwnerWriteGateway::new(
            &reopened_events,
            &reopened_tasks,
            &reopened_proposals,
            &reopened_memory,
        );
        let reconciled = reopened_gateway
            .reconcile_pending_terminal_owner_successors(32)
            .await
            .expect("reconcile owner-local receipts into EventStore successors");
        assert_eq!(reconciled.successors_confirmed, 1);
        assert_eq!(reconciled.canonical_effects_executed, 0);
        assert_eq!(reconciled.task_owner_transitions_executed, 0);
        assert_eq!(reconciled.unknown_external_effects_retried, 0);
        let replay = reopened_gateway
            .reconcile_pending_terminal_owner_successors(32)
            .await
            .expect("reconciliation replay is idempotent");
        assert_eq!(replay.successors_confirmed, 0);
        assert_eq!(replay.canonical_effects_executed, 0);
        assert_eq!(replay.task_owner_transitions_executed, 0);
        assert_eq!(replay.unknown_external_effects_retried, 0);

        let memory_after_reconcile = reopened_memory
            .get_record_by_proposal_id(&proposal_id)
            .expect("read Memory effect after reconciliation")
            .expect("Memory effect remains present");
        assert_eq!(memory_after_reconcile.memory_id, memory_id);
        assert_eq!(
            reopened_memory
                .list_active_records(None, 100)
                .expect("count Memory owners after reconciliation replay")
                .len(),
            1,
            "reconciliation must not re-execute the Memory effect"
        );
        let task_after_reconcile = reopened_tasks
            .canonical_owner_head(&operation_id)
            .expect("read Task owner after reconciliation")
            .expect("Task owner exists after reconciliation");
        assert_eq!(task_after_reconcile.revision(), owner_revision);
        assert_eq!(task_after_reconcile.digest(), owner_digest);
        let verified_local_receipt = reopened_tasks
            .verified_terminal_owner_transition_receipt(&local_receipt_ref)
            .expect("verify owner-local receipt after reopen")
            .expect("owner-local receipt remains durable");
        assert_eq!(
            verified_local_receipt.receipt_digest(),
            local_receipt_digest
        );
        assert_eq!(
            reopened_proposals
                .get_proposal(&proposal_id)
                .expect("read reconciled Proposal")
                .expect("reconciled Proposal exists")
                .status,
            ProposalStatus::Accepted
        );
        assert_eq!(
            reopened_proposals
                .dispatch_state(&proposal_id)
                .expect("read reconciled dispatch")
                .as_deref(),
            Some("confirmed")
        );
        let events = reopened_events
            .list(&operation_id, 0, 250)
            .expect("read terminal facts after reconciliation");
        let successors = events
            .iter()
            .filter(|event| event.event_type == "terminal_owner.successor_confirmed")
            .collect::<Vec<_>>();
        assert_eq!(successors.len(), 1);
        assert_eq!(successors[0].payload["causeRef"], proposal_id);
        assert_eq!(successors[0].payload["finalEventId"], final_fact.event_id);
        assert_eq!(
            successors[0].payload["localTransitionReceiptRef"],
            local_receipt_ref
        );
        assert_eq!(
            successors[0].payload["localTransitionReceiptDigest"],
            local_receipt_digest
        );
    })
    .await
    .expect("D055 cross-store reconciliation scenario exceeded its outer timeout");
}

#[tokio::test]
async fn d055_target_final_insert_and_sealed_cas_are_one_event_store_transaction() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        let temp = tempfile::tempdir().expect("D055 temp directory");
        let event_path = temp.path().join("turn-events.sqlite");
        let task_path = temp.path().join("task-owner.sqlite");
        let proposal_path = temp.path().join("proposal-owner.sqlite");
        let conversation_path = temp.path().join("conversation-owner.sqlite");
        let operation_id = uuid::Uuid::new_v4().to_string();
        let chat_session_id = "d055-final-seal-failpoint";
        let user_goal = "D055 final and SEALED CAS transaction";
        let task_store = AgentTaskSessionStore::new_with_receipt_key(&task_path, receipt_key())
            .expect("real file-backed TaskSession store");
        let memory_store =
            MemoryStore::new(&conversation_path).expect("real file-backed Conversation owner");
        let canonical_message = commit_file_backed_user_message(
            &memory_store,
            &operation_id,
            chat_session_id,
            user_goal,
        );
        let canonical_receipt = canonical_message.receipt().clone();
        create_file_backed_task(&task_store, &operation_id, chat_session_id, user_goal);
        task_store
            .bind_canonical_memory_store(&memory_store)
            .expect("TaskSession store binds the canonical Conversation owner");
        task_store
            .bind_session_canonical_user_message(
                &operation_id,
                &canonical_receipt.canonical_ref,
                user_goal,
            )
            .expect("TaskSession binds the exact canonical user message");
        let event_store = crate::main_chat_event_stream::MainChatAgentEventStore::new(&event_path)
            .expect("real file-backed EventStore");
        let proposal_store =
            ProposalStore::new(&proposal_path).expect("real file-backed ProposalStore");
        let memory_lifecycle_path = temp.path().join("memory-lifecycle-owner.sqlite");
        let memory_lifecycle_store = MemoryLifecycleStore::new(&memory_lifecycle_path)
            .expect("real file-backed Memory lifecycle owner");
        let _gateway = TerminalOwnerWriteGateway::new(
            &event_store,
            &task_store,
            &proposal_store,
            &memory_lifecycle_store,
        );
        let finalizer = include_str!("main_chat_turn_runtime.rs")
            .split_once("async fn persist_openlife_turn_final_delivery_receipt(")
            .and_then(|(_, rest)| rest.split_once("\nfn canonical_final_owner_digest("))
            .map(|(body, _)| body)
            .expect("production finalizer remains source-mappable");
        assert!(
            finalizer.contains("append_terminal_final_and_seal("),
            "OpenLifeTurnRuntime must call this exact EventStore transaction API"
        );
        let terminal_admission = task_store
            .issue_terminal_owner_epoch_admission(&operation_id, &operation_id, canonical_message)
            .expect("TaskSession owner verifies canonical-message authority");
        let epoch = event_store
            .open_terminal_owner_epoch_from_admission(terminal_admission)
            .expect("open epoch from non-Serde terminal admission");
        assert_eq!(
            epoch.canonical_user_message_ref(),
            canonical_receipt.canonical_ref
        );
        assert_eq!(
            epoch.canonical_user_message_digest(),
            canonical_receipt.content_digest
        );
        event_store
            .begin_terminal_owner_seal(&operation_id, &operation_id, epoch.generation())
            .expect("enter sealing");
        let owner = task_store
            .canonical_owner_head(&operation_id)
            .expect("owner head")
            .expect("owner exists");
        let input = MainChatTerminalFinalizationInput {
            task_session_id: operation_id.clone(),
            run_id: operation_id.clone(),
            epoch_generation: epoch.generation(),
            delivery_id: format!("delivery:{operation_id}:{operation_id}"),
            expected_task_owner_revision: owner.revision(),
            expected_task_owner_digest: owner.digest().to_string(),
            status: "completed".into(),
        };

        event_store
            .install_fail_after_final_insert_before_sealed_epoch_cas_for_test(&operation_id)
            .expect("install transaction failpoint");
        let error = event_store
            .append_terminal_final_and_seal(input.clone())
            .expect_err("failpoint interrupts the real production transaction");
        assert_eq!(
            error.to_string(),
            "injected_failure_after_final_insert_before_sealed_epoch_cas"
        );
        let rolled_back = event_store
            .list(&operation_id, 0, 250)
            .expect("read facts after rollback");
        assert!(rolled_back
            .iter()
            .all(|event| event.event_type != "final_delivery.created"));
        let still_sealing = event_store
            .terminal_owner_epoch(&operation_id)
            .expect("read epoch after rollback")
            .expect("epoch remains durable");
        assert_eq!(still_sealing.state(), TerminalOwnerSealState::Sealing);

        event_store.clear_fail_after_final_insert_before_sealed_epoch_cas_for_test(&operation_id);
        let committed = event_store
            .append_terminal_final_and_seal(input)
            .expect("retry commits final and sealed epoch together");
        let sealed = event_store
            .terminal_owner_epoch(&operation_id)
            .expect("read sealed epoch")
            .expect("sealed epoch exists");
        assert_eq!(sealed.state(), TerminalOwnerSealState::Sealed);
        assert_eq!(sealed.final_event_id(), Some(committed.event_id.as_str()));
        let committed_finals = event_store
            .list(&operation_id, 0, 250)
            .expect("read facts after retry")
            .into_iter()
            .filter(|event| event.event_type == "final_delivery.created")
            .collect::<Vec<_>>();
        assert_eq!(committed_finals.len(), 1);
        assert_eq!(committed_finals[0].event_id, committed.event_id);
    })
    .await
    .expect("D055 atomic final+seal scenario exceeded its outer timeout");
}

#[tokio::test]
async fn d055_target_real_sensitive_memory_accept_defers_during_seal_then_commits_once() {
    tokio::time::timeout(SCENARIO_TIMEOUT, async {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let operation_id = uuid::Uuid::new_v4().to_string();
        let session_id = "d055-target-real-sensitive-memory";
        let body = SENSITIVE_MEMORY_BODY;
        let (_barrier, reached, release) =
            crate::main_chat_turn_runtime::install_main_chat_terminal_sealing_barrier_for_test(
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
        tokio::time::timeout(STEP_TIMEOUT, reached.wait())
            .await
            .expect("real turn reaches SEALING barrier");

        let canonical_message = state
            .memory_store
            .lock()
            .await
            .load_active_conversation_message_by_operation(&operation_id)
            .expect("read canonical user-message owner")
            .expect("runtime committed a canonical user message before staging Review");
        assert_eq!(canonical_message.message.role, "user");
        assert_eq!(canonical_message.message.content, body);
        assert_eq!(canonical_message.receipt.operation_id, operation_id);

        let proposal = {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            let pending = store
                .list_pending_proposals(100)
                .expect("pending Proposals");
            let exact = pending
                .into_iter()
                .filter(|proposal| proposal.proposal_type == ProposalType::MemoryWrite)
                .collect::<Vec<_>>();
            assert_eq!(exact.len(), 1, "one real sensitive Memory Proposal");
            let proposal = exact.into_iter().next().unwrap();
            let origin = store
                .terminal_owner_origin_binding(&proposal.id)
                .expect("read typed origin")
                .expect("product staging persists typed immutable origin");
            assert_eq!(origin.task_session_id(), operation_id);
            assert_eq!(origin.run_id(), operation_id);
            assert!(origin.epoch_generation() > 0);
            assert_eq!(
                origin.canonical_user_message_ref(),
                canonical_message.receipt.canonical_ref
            );
            assert_eq!(
                origin.canonical_user_message_digest(),
                canonical_message.receipt.content_digest
            );
            assert!(proposal.source_detail.is_none());
            assert!(proposal.after.get("originatingTaskSessionId").is_none());
            proposal
        };
        let memory_before = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_active_records(None, 100)
            .unwrap()
            .len();
        let owner_before = state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_owner_head(&operation_id)
            .unwrap()
            .unwrap();
        let during =
            crate::commands::proposal::accept_proposal_with_state(proposal.id.clone(), &state)
                .await
                .expect("sealing conflict is a typed deferred response, not an arbitrary error");
        assert_eq!(during["success"], false);
        assert_eq!(during["status"], "deferred");
        assert_eq!(during["reasonCode"], "origin_turn_sealing");
        assert_eq!(during["proposalId"], proposal.id);
        assert_eq!(during["dispatchState"], "unclaimed");
        assert_eq!(during["durableWriteExecuted"], false);
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            assert_eq!(
                store.get_proposal(&proposal.id).unwrap().unwrap().status,
                ProposalStatus::Pending
            );
            assert_eq!(
                store.dispatch_state(&proposal.id).unwrap().as_deref(),
                Some("unclaimed")
            );
        }
        assert_eq!(
            state
                .memory_lifecycle_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_active_records(None, 100)
                .unwrap()
                .len(),
            memory_before
        );
        let owner_during = state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_owner_head(&operation_id)
            .unwrap()
            .unwrap();
        assert_eq!(owner_during.revision(), owner_before.revision());
        assert_eq!(owner_during.digest(), owner_before.digest());

        release.wait().await;
        let first = tokio::time::timeout(STEP_TIMEOUT, turn)
            .await
            .expect("origin turn exits barrier")
            .expect("origin turn joins")
            .expect("origin turn completes");
        assert_eq!(first.run_id.as_deref(), Some(operation_id.as_str()));

        let after =
            crate::commands::proposal::accept_proposal_with_state(proposal.id.clone(), &state)
                .await
                .expect("same Proposal commits once after SEALED");
        assert_eq!(after["success"], true);
        assert_eq!(after["effect_status"], "confirmed");
        assert_eq!(after["proposal_projection_status"], "confirmed");
        let replay =
            crate::commands::proposal::accept_proposal_with_state(proposal.id.clone(), &state)
                .await
                .expect("post-seal retry is idempotent");
        assert_eq!(replay["success"], true);
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            assert_eq!(
                store.get_proposal(&proposal.id).unwrap().unwrap().status,
                ProposalStatus::Accepted
            );
            assert_eq!(
                store.dispatch_state(&proposal.id).unwrap().as_deref(),
                Some("confirmed")
            );
        }
        assert_eq!(
            state
                .memory_lifecycle_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_active_records(None, 100)
                .unwrap()
                .len(),
            memory_before + 1
        );

        let buffered = crate::main_chat_send::send_message_with_operation_state(
            operation_id.clone(),
            session_id.into(),
            vec![ChatMessage {
                role: "user".into(),
                content: body.into(),
            }],
            None,
            &state,
        )
        .await;
        assert!(
            buffered.is_ok(),
            "buffered recovery folds successor: {buffered:?}"
        );
        let mut streamed = Vec::new();
        let streaming = crate::main_chat_streaming::start_stream_message_with_operation_state(
            operation_id.clone(),
            session_id.into(),
            vec![ChatMessage {
                role: "user".into(),
                content: body.into(),
            }],
            None,
            &state,
            |event, payload| streamed.push((event.to_string(), payload)),
        )
        .await;
        assert!(
            streaming.is_ok(),
            "streaming recovery folds successor: {streaming:?}"
        );
        assert_eq!(
            streamed
                .iter()
                .filter(|(event, _)| event == "stream-message-done")
                .count(),
            1
        );
        let events = state
            .main_chat_agent_event_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list(&operation_id, 0, 250)
            .unwrap();
        let final_event = events
            .iter()
            .filter(|event| event.event_type == "final_delivery.created")
            .collect::<Vec<_>>();
        let successor = events
            .iter()
            .filter(|event| event.event_type == "terminal_owner.successor_confirmed")
            .collect::<Vec<_>>();
        assert_eq!(final_event.len(), 1);
        assert_eq!(successor.len(), 1);
        let sealed_epoch = state
            .main_chat_agent_event_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .terminal_owner_epoch(&operation_id)
            .expect("read the runtime's production terminal epoch")
            .expect("runtime terminal epoch exists");
        assert_eq!(sealed_epoch.state(), TerminalOwnerSealState::Sealed);
        assert_eq!(
            sealed_epoch.final_event_id(),
            Some(final_event[0].event_id.as_str())
        );
        assert_eq!(
            sealed_epoch.final_event_payload_digest(),
            Some(final_event[0].payload_digest.as_str())
        );
        assert_eq!(successor[0].payload["causeRef"], proposal.id);
        assert_eq!(
            successor[0].payload["finalEventId"],
            final_event[0].event_id
        );
        assert_eq!(successor[0].payload["ownerKind"], "agent_task_session");
        assert_eq!(successor[0].payload["ownerId"], operation_id);
        assert!(
            successor[0].payload["beforeOwnerRevision"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert_eq!(
            successor[0].payload["afterOwnerRevision"].as_u64().unwrap(),
            successor[0].payload["beforeOwnerRevision"]
                .as_u64()
                .unwrap()
                + 1
        );
        for field in [
            "beforeOwnerDigest",
            "afterOwnerDigest",
            "localTransitionReceiptRef",
            "localTransitionReceiptDigest",
        ] {
            assert!(successor[0].payload[field]
                .as_str()
                .is_some_and(|value| !value.is_empty()));
        }
    })
    .await
    .expect("D055 real sensitive-memory scenario exceeded its outer timeout");
}
