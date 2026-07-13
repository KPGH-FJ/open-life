#![cfg(feature = "test-utils")]

use openlife_core::agent::ProposalStatus;
use openlife_tauri_lib::d055_test_support::{
    D055TypedOriginTerminalSealHarness, TerminalSealAcceptanceDispositionV1,
};

#[tokio::test]
async fn typed_origin_proposal_defers_during_seal_then_commits_one_recoverable_successor() {
    let mut harness = D055TypedOriginTerminalSealHarness::new(
        "Explain one practical way to stay focused this afternoon.",
        "D055 typed-origin post-seal focus",
    )
    .await
    .expect("build isolated real-store/provider-capture harness");

    let staged = harness
        .stage_proposal_through_review_workflow()
        .await
        .expect("the test-utils seam uses the production typed origin staging path");
    assert!(
        staged
            .proposal
            .source_detail
            .as_deref()
            .map_or(true, |value| {
                !value.contains("main_chat_agent_task_session:")
                    && !value.contains(staged.operation_id.as_str())
            }),
        "free-text source_detail must not carry turn authority"
    );
    let after_text =
        serde_json::to_string(&staged.proposal.after).expect("serialize Proposal body");
    assert!(
        !after_text.contains("originatingTaskSessionId")
            && !after_text.contains("originating_task_session_id")
            && !after_text.contains("operationId")
            && !after_text.contains("operation_id")
            && !after_text.contains("epochGeneration")
            && !after_text.contains("epoch_generation"),
        "caller-shaped Proposal JSON must not carry turn authority"
    );
    assert_eq!(staged.origin_binding.operation_id(), staged.operation_id);
    assert_eq!(
        staged.origin_binding.epoch_generation(),
        staged.epoch_generation
    );

    harness
        .start_turn_and_wait_after_final_owner_snapshot()
        .await
        .expect("real TurnRuntime reaches the owner-snapshot barrier");
    let deferred = harness
        .accept_staged_proposal()
        .await
        .expect("sealing admission returns typed product truth");
    assert_eq!(
        deferred,
        TerminalSealAcceptanceDispositionV1::DeferredWhileOriginTurnSealing
    );
    let during = harness
        .proposal_and_owner_snapshot()
        .await
        .expect("read canonical stores while sealing");
    assert_eq!(during.proposal_status, ProposalStatus::Pending);
    assert_eq!(during.dispatch_state, "unclaimed");
    assert_eq!(during.applied_patch_count, 0);
    assert_eq!(during.canonical_focus, during.focus_before);

    harness
        .release_final_seal_and_wait_for_turn()
        .await
        .expect("origin turn commits one sealed final");
    let accepted = harness
        .accept_staged_proposal()
        .await
        .expect("the exact deferred Proposal remains retryable after seal");
    assert_eq!(
        accepted,
        TerminalSealAcceptanceDispositionV1::EffectConfirmed
    );
    let after = harness
        .proposal_and_owner_snapshot()
        .await
        .expect("read canonical stores after typed successor");
    assert_eq!(after.proposal_status, ProposalStatus::Accepted);
    assert_eq!(after.dispatch_state, "confirmed");
    assert_eq!(after.applied_patch_count, 1);
    assert_eq!(after.canonical_focus, after.proposed_focus);

    harness
        .recover_same_operation()
        .await
        .expect("historical final plus exact successor receipt recovers");
    let facts = harness
        .durable_turn_facts()
        .await
        .expect("read durable event/provider facts");
    assert_eq!(facts.provider_request_count, 1);
    assert_eq!(facts.provider_completed_event_count, 1);
    assert_eq!(facts.final_event_count, 1);
    assert_eq!(facts.confirmed_successor_count, 1);
}
