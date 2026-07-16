use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use openlife_core::agent::main_chat_agent_v1::{
    AgentTaskSessionStore, VerifiedTerminalOwnerTransitionReceipt,
};
use openlife_core::agent::review_workflow::ClaimedReviewAcceptanceSnapshot;
use openlife_core::agent::{
    MemoryLifecycleAcceptanceInput, MemoryLifecycleStore, ProposalStore, ProposalType,
    ReviewWorkflow,
};

use crate::main_chat_event_stream::{
    MainChatAgentDurableEvent, MainChatAgentEventStore, TerminalOwnerSealState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalOwnerCrashPoint {
    AfterClaimPersistedBeforeEffect,
    AfterMemoryCommittedBeforeTaskOwner,
    AfterTaskOwnerReceiptBeforeProposalCheckpoint,
    AfterProposalCheckpointBeforeSuccessor,
    AfterSuccessorBeforeProposalProjection,
}

impl TerminalOwnerCrashPoint {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AfterClaimPersistedBeforeEffect => "after_claim_persisted_before_effect",
            Self::AfterMemoryCommittedBeforeTaskOwner => "after_memory_committed_before_task_owner",
            Self::AfterTaskOwnerReceiptBeforeProposalCheckpoint => {
                "after_task_owner_receipt_before_proposal_checkpoint"
            }
            Self::AfterProposalCheckpointBeforeSuccessor => {
                "after_proposal_checkpoint_before_successor"
            }
            Self::AfterSuccessorBeforeProposalProjection => {
                "after_successor_before_proposal_projection"
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedTerminalOwnerExternalDispatch {
    proposal_id: String,
}

impl PreparedTerminalOwnerExternalDispatch {
    pub(crate) fn proposal_id(&self) -> &str {
        &self.proposal_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExternalDispatchOutcome {
    Confirmed,
    FailedBeforeEffect,
    RemoteUnknown,
}

#[async_trait::async_trait]
pub(crate) trait TerminalOwnerExternalDispatchAdapter: Send + Sync {
    async fn dispatch(
        &self,
        request: &PreparedTerminalOwnerExternalDispatch,
    ) -> Result<ExternalDispatchOutcome, String>;
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TerminalOwnerExecutionCapture {
    counts: Arc<Mutex<HashMap<(String, &'static str), usize>>>,
}

impl TerminalOwnerExecutionCapture {
    fn record(&self, proposal_id: &str, stage: &'static str) {
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *counts.entry((proposal_id.to_string(), stage)).or_default() += 1;
    }

    fn count(&self, proposal_id: &str, stage: &'static str) -> usize {
        self.counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&(proposal_id.to_string(), stage))
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn memory_effect_invocations(&self, proposal_id: &str) -> usize {
        self.count(proposal_id, "memory_effect")
    }

    pub(crate) fn task_owner_transition_invocations(&self, proposal_id: &str) -> usize {
        self.count(proposal_id, "task_owner_transition")
    }

    pub(crate) fn successor_confirmation_invocations(&self, proposal_id: &str) -> usize {
        self.count(proposal_id, "successor_confirmation")
    }

    pub(crate) fn proposal_projection_invocations(&self, proposal_id: &str) -> usize {
        self.count(proposal_id, "proposal_projection")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalOwnerReviewTransition {
    pub(crate) before_owner_revision: u64,
    pub(crate) after_owner_revision: u64,
    pub(crate) before_owner_digest: String,
    pub(crate) after_owner_digest: String,
    pub(crate) local_transition_receipt_ref: String,
    pub(crate) local_transition_receipt_digest: String,
    pub(crate) successor_event_id: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TerminalOwnerReconciliationReport {
    pub(crate) canonical_effects_executed: usize,
    pub(crate) task_owner_transitions_executed: usize,
    pub(crate) successors_confirmed: usize,
    pub(crate) proposals_projected: usize,
    pub(crate) unknown_external_effects_retried: usize,
}

pub(crate) struct TerminalOwnerWriteGateway {
    event_store: MainChatAgentEventStore,
    task_store: AgentTaskSessionStore,
    proposal_store: ProposalStore,
    memory_store: MemoryLifecycleStore,
    external_dispatch: Option<Arc<dyn TerminalOwnerExternalDispatchAdapter>>,
    execution_capture: TerminalOwnerExecutionCapture,
    crash_points: Mutex<HashMap<String, TerminalOwnerCrashPoint>>,
}

impl TerminalOwnerWriteGateway {
    pub(crate) fn new(
        event_store: &MainChatAgentEventStore,
        task_store: &AgentTaskSessionStore,
        proposal_store: &ProposalStore,
        memory_store: &MemoryLifecycleStore,
    ) -> Self {
        Self {
            event_store: event_store.clone(),
            task_store: task_store.clone(),
            proposal_store: proposal_store.clone(),
            memory_store: memory_store.clone(),
            external_dispatch: None,
            execution_capture: TerminalOwnerExecutionCapture::default(),
            crash_points: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn with_external_dispatch_adapter(
        mut self,
        adapter: Arc<dyn TerminalOwnerExternalDispatchAdapter>,
    ) -> Self {
        self.external_dispatch = Some(adapter);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_execution_capture_for_test(
        mut self,
        capture: TerminalOwnerExecutionCapture,
    ) -> Self {
        self.execution_capture = capture;
        self
    }

    #[cfg(test)]
    pub(crate) fn install_crash_point_for_test(
        &self,
        proposal_id: &str,
        crash_point: TerminalOwnerCrashPoint,
    ) -> anyhow::Result<()> {
        self.crash_points
            .lock()
            .map_err(|error| anyhow::anyhow!("terminal crash point mutex: {error}"))?
            .insert(proposal_id.to_string(), crash_point);
        Ok(())
    }

    fn crash_if_selected(
        &self,
        proposal_id: &str,
        crash_point: TerminalOwnerCrashPoint,
    ) -> anyhow::Result<()> {
        let selected = self
            .crash_points
            .lock()
            .map_err(|error| anyhow::anyhow!("terminal crash point mutex: {error}"))?
            .get(proposal_id)
            .copied();
        if selected == Some(crash_point) {
            anyhow::bail!("injected_terminal_owner_crash:{}", crash_point.as_str());
        }
        Ok(())
    }

    pub(crate) async fn apply_claimed_review_acceptance(
        &self,
        acceptance: ClaimedReviewAcceptanceSnapshot,
    ) -> anyhow::Result<TerminalOwnerReviewTransition> {
        acceptance.validate()?;
        let proposal = acceptance.proposal().clone();
        let proposal_id = proposal.id.clone();
        let claim_id = self
            .proposal_store
            .dispatch_claim_id(&proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_dispatch_claim_missing"))?;
        self.crash_if_selected(
            &proposal_id,
            TerminalOwnerCrashPoint::AfterClaimPersistedBeforeEffect,
        )?;
        let origin = acceptance
            .terminal_owner_origin()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_origin_binding_missing"))?;
        let epoch = self
            .event_store
            .terminal_owner_epoch(origin.task_session_id())?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_epoch_missing"))?;
        if epoch.run_id() != origin.run_id() {
            anyhow::bail!("terminal_owner_epoch_run_mismatch");
        }
        match epoch.state() {
            TerminalOwnerSealState::Open => {
                anyhow::bail!("terminal_owner_origin_turn_open");
            }
            TerminalOwnerSealState::Sealing => {
                anyhow::bail!("terminal_owner_origin_turn_sealing");
            }
            TerminalOwnerSealState::Sealed => {}
        }

        if proposal.proposal_type == ProposalType::ExternalWriteAction {
            let adapter = self
                .external_dispatch
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("terminal_owner_external_dispatch_unavailable"))?;
            let outcome = adapter
                .dispatch(&PreparedTerminalOwnerExternalDispatch {
                    proposal_id: proposal_id.clone(),
                })
                .await
                .map_err(anyhow::Error::msg)?;
            match outcome {
                ExternalDispatchOutcome::RemoteUnknown => {
                    self.proposal_store.mark_dispatch_unknown(
                        &proposal_id,
                        &claim_id,
                        "terminal_owner_external_effect_remote_unknown",
                    )?;
                    anyhow::bail!("terminal_owner_external_effect_remote_unknown")
                }
                ExternalDispatchOutcome::FailedBeforeEffect => {
                    self.proposal_store.mark_dispatch_failed_before_effect(
                        &proposal_id,
                        &claim_id,
                        "terminal_owner_external_effect_failed_before_effect",
                    )?;
                    anyhow::bail!("terminal_owner_external_effect_failed_before_effect")
                }
                ExternalDispatchOutcome::Confirmed => {
                    anyhow::bail!("terminal_owner_external_confirmed_adapter_not_integrated")
                }
            }
        }
        if proposal.proposal_type != ProposalType::MemoryWrite {
            anyhow::bail!("terminal_owner_proposal_type_not_supported");
        }

        if self
            .memory_store
            .get_record_by_proposal_id(&proposal_id)?
            .is_none()
        {
            self.execution_capture.record(&proposal_id, "memory_effect");
            let content = proposal
                .after
                .get("content")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("terminal_owner_memory_content_missing"))?;
            let input = MemoryLifecycleAcceptanceInput::from_memory_proposal_with_terminal_origin(
                &proposal,
                content.to_string(),
                origin.task_session_id(),
                origin.run_id(),
                origin.canonical_user_message_ref(),
                origin.canonical_user_message_digest(),
            )?;
            self.memory_store.accept_memory_proposal(input)?;
        }
        self.crash_if_selected(
            &proposal_id,
            TerminalOwnerCrashPoint::AfterMemoryCommittedBeforeTaskOwner,
        )?;

        let final_event_id = epoch
            .final_event_id()
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_final_event_missing"))?;
        let final_event = self
            .event_store
            .get_immutable_event(
                origin.task_session_id(),
                "final_delivery.created",
                &format!("delivery:{}:{}", origin.task_session_id(), origin.run_id()),
            )?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_final_event_missing"))?;
        if final_event.event_id != final_event_id {
            anyhow::bail!("terminal_owner_final_event_identity_mismatch");
        }
        let before_revision = final_event
            .payload
            .get("taskOwnerRevision")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_final_revision_missing"))?;
        let before_digest = final_event
            .payload
            .get("taskOwnerDigest")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_final_digest_missing"))?;
        let existing_receipt = self
            .task_store
            .terminal_owner_transition_receipt_for_claim(&proposal_id, &claim_id)?;
        let receipt = if let Some(receipt) = existing_receipt {
            receipt
        } else {
            self.execution_capture
                .record(&proposal_id, "task_owner_transition");
            self.task_store.apply_terminal_owner_review_transition(
                &proposal_id,
                &claim_id,
                origin.task_session_id(),
                before_revision,
                before_digest,
            )?
        };
        self.crash_if_selected(
            &proposal_id,
            TerminalOwnerCrashPoint::AfterTaskOwnerReceiptBeforeProposalCheckpoint,
        )?;

        let dispatch_state = self
            .proposal_store
            .dispatch_state(&proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_dispatch_state_missing"))?;
        if dispatch_state == "claimed"
            && !self
                .proposal_store
                .mark_effect_confirmed_projection_pending(&proposal_id, &claim_id)?
        {
            anyhow::bail!("terminal_owner_proposal_checkpoint_cas_lost");
        }
        self.crash_if_selected(
            &proposal_id,
            TerminalOwnerCrashPoint::AfterProposalCheckpointBeforeSuccessor,
        )?;

        let successor_existed = self
            .event_store
            .get_immutable_event(
                origin.task_session_id(),
                "terminal_owner.successor_confirmed",
                &format!("successor:{proposal_id}"),
            )?
            .is_some();
        let successor = self.event_store.append_terminal_owner_successor(
            origin.task_session_id(),
            origin.run_id(),
            &proposal_id,
            &receipt,
        )?;
        if !successor_existed {
            self.execution_capture
                .record(&proposal_id, "successor_confirmation");
        }
        self.crash_if_selected(
            &proposal_id,
            TerminalOwnerCrashPoint::AfterSuccessorBeforeProposalProjection,
        )?;

        if self.proposal_store.dispatch_state(&proposal_id)?.as_deref()
            == Some("confirmed_projection_pending")
        {
            let mut accepted = proposal;
            accepted.accept();
            self.execution_capture
                .record(&proposal_id, "proposal_projection");
            if !self
                .proposal_store
                .project_confirmed_effect(&accepted, &claim_id)?
            {
                anyhow::bail!("terminal_owner_proposal_projection_cas_lost");
            }
        }
        Ok(transition_from_receipt(receipt, successor))
    }

    pub(crate) async fn reconcile_pending_terminal_owner_successors(
        &self,
        limit: usize,
    ) -> anyhow::Result<TerminalOwnerReconciliationReport> {
        let candidates = self
            .proposal_store
            .list_terminal_owner_reconciliation_candidates(limit)?;
        let mut before_memory = 0;
        let mut before_task = 0;
        for (proposal, claim_id, _) in &candidates {
            if self
                .memory_store
                .get_record_by_proposal_id(&proposal.id)?
                .is_none()
            {
                before_memory += 1;
            }
            if self
                .task_store
                .terminal_owner_transition_receipt_for_claim(&proposal.id, claim_id)?
                .is_none()
            {
                before_task += 1;
            }
        }
        let mut before_successors = 0;
        for (proposal, _, _) in &candidates {
            let origin = self
                .proposal_store
                .terminal_owner_origin_binding(&proposal.id)?
                .ok_or_else(|| anyhow::anyhow!("terminal_owner_origin_binding_missing"))?;
            if self
                .event_store
                .get_immutable_event(
                    origin.task_session_id(),
                    "terminal_owner.successor_confirmed",
                    &format!("successor:{}", proposal.id),
                )?
                .is_none()
            {
                before_successors += 1;
            }
        }
        for (proposal, claim_id, _) in &candidates {
            let acceptance = ReviewWorkflow::new(&self.proposal_store)
                .claimed_acceptance_snapshot(&proposal.id, claim_id)?;
            self.apply_claimed_review_acceptance(acceptance).await?;
        }
        Ok(TerminalOwnerReconciliationReport {
            canonical_effects_executed: before_memory,
            task_owner_transitions_executed: before_task,
            successors_confirmed: before_successors,
            proposals_projected: candidates.len(),
            unknown_external_effects_retried: 0,
        })
    }
}

fn transition_from_receipt(
    receipt: VerifiedTerminalOwnerTransitionReceipt,
    successor: MainChatAgentDurableEvent,
) -> TerminalOwnerReviewTransition {
    TerminalOwnerReviewTransition {
        before_owner_revision: receipt.before_revision(),
        after_owner_revision: receipt.after_revision(),
        before_owner_digest: receipt.before_digest().to_string(),
        after_owner_digest: receipt.after_digest().to_string(),
        local_transition_receipt_ref: receipt.receipt_ref().to_string(),
        local_transition_receipt_digest: receipt.receipt_digest().to_string(),
        successor_event_id: successor.event_id,
    }
}
