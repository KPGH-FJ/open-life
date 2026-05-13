use super::AgentLoop;
use crate::agent::compaction::{build_safe_compacted_observation, compact_messages_for_agent_loop};
use crate::agent::types::{
    AgentEventActor, AgentRun, AgentRunEventType, AgentTask, CompactionEventPayload, PrivacyPolicy,
};

impl AgentLoop {
    /// P8: Check compaction policy and, if triggered, replace the current
    /// task messages with compacted context. Records `compaction.created`
    /// event and returns true if compaction was applied.
    pub(crate) fn try_compact_context(
        &self,
        task: &mut AgentTask,
        run: &mut AgentRun,
        privacy_policy: PrivacyPolicy,
    ) -> bool {
        let compaction_cfg = match &self.config.compaction_config {
            Some(cfg) if cfg.enabled => cfg,
            _ => return false,
        };

        let decision = crate::agent::compaction::should_compact(&task.messages, compaction_cfg);
        if !decision.should_compact {
            return false;
        }

        let unresolved_obs: Vec<crate::agent::types::CompactedObservation> = run
            .observations
            .iter()
            .map(|obs| build_safe_compacted_observation(&obs.source, &obs.content))
            .collect();
        let unresolved_count = unresolved_obs.len();

        if let Some(result) = compact_messages_for_agent_loop(
            &task.messages,
            &run.id,
            compaction_cfg,
            privacy_policy,
            run.generated_proposals.clone(),
            unresolved_obs,
        ) {
            let reason = decision
                .reason
                .unwrap_or_else(|| "triggered by policy".into());
            let payload = CompactionEventPayload {
                compaction_id: result.summary.id.clone(),
                run_id: run.id.clone(),
                reason,
                original_token_estimate: decision.original_token_estimate,
                compacted_token_estimate: result.summary.compacted_token_estimate,
                source_message_count: decision.message_count,
                active_proposal_count: run.generated_proposals.len(),
                unresolved_observation_count: unresolved_count as u32,
                redacted_fields: result.summary.redacted_fields.clone(),
                privacy_policy: privacy_policy.to_string(),
            };
            self.try_record_event(
                &run.id,
                AgentRunEventType::CompactionCreated,
                AgentEventActor::Runtime,
                format!(
                    "Context compacted: {} -> {} messages (~{} -> ~{} tokens)",
                    payload.source_message_count,
                    result.compacted_messages.len(),
                    payload.original_token_estimate,
                    payload.compacted_token_estimate,
                ),
                serde_json::to_value(&payload).unwrap_or_default(),
            );

            task.messages = result.compacted_messages;
            return true;
        }

        false
    }

    /// P8 helper exposed for tests: public visibility for compaction tests.
    #[doc(hidden)]
    pub fn _test_compact_context(
        &self,
        task: &mut AgentTask,
        run: &mut AgentRun,
        privacy_policy: PrivacyPolicy,
    ) -> bool {
        self.try_compact_context(task, run, privacy_policy)
    }
}
