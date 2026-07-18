use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::agent::{AgentRunStore, DurableToolExecutionOwner};
use crate::tool_execution_receipt::{
    ToolActionEffect, ToolDispatchKind, ToolEffectStatus, ToolExecutionOutcome,
    ToolExecutionReceipt, ToolTransportStatus,
};
use crate::tool_manifest::ToolIdempotencyContract;

/// Durable state owned by the canonical AgentRun store. This is a child state
/// machine of AgentRun, not a second execution authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRunToolExecutionState {
    Prepared,
    DispatchAttempted,
    ResponseObserved,
    TerminalSucceeded,
    TerminalFailed,
    TerminalNotAttempted,
    TerminalRemoteUnknown,
}

impl AgentRunToolExecutionState {
    pub(crate) fn from_str(value: &str) -> Result<Self> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "dispatch_attempted" => Ok(Self::DispatchAttempted),
            "response_observed" => Ok(Self::ResponseObserved),
            "terminal_succeeded" => Ok(Self::TerminalSucceeded),
            "terminal_failed" => Ok(Self::TerminalFailed),
            "terminal_not_attempted" => Ok(Self::TerminalNotAttempted),
            "terminal_remote_unknown" => Ok(Self::TerminalRemoteUnknown),
            _ => anyhow::bail!("agent_run_tool_execution_state_invalid"),
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::TerminalSucceeded
                | Self::TerminalFailed
                | Self::TerminalNotAttempted
                | Self::TerminalRemoteUnknown
        )
    }
}

/// Minimal durable execution fact. It contains no endpoint, request body,
/// tool arguments, response body, prompt, token, or credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunToolExecutionRecord {
    pub run_id: String,
    pub receipt_id: String,
    pub manifest_id: String,
    pub request_digest: String,
    pub endpoint_digest: String,
    pub action_effect: ToolActionEffect,
    pub idempotency_contract: ToolIdempotencyContract,
    pub dispatch_kind: ToolDispatchKind,
    pub state: AgentRunToolExecutionState,
    pub revision: u64,
    pub dispatch_attempt_count: u32,
    pub transport_status: ToolTransportStatus,
    pub effect_status: ToolEffectStatus,
    pub execution_outcome: ToolExecutionOutcome,
    pub prepared_at: DateTime<Utc>,
    pub dispatch_attempted_at: Option<DateTime<Utc>>,
    pub response_observed_at: Option<DateTime<Utc>>,
    pub terminal_at: Option<DateTime<Utc>>,
}

impl AgentRunToolExecutionRecord {
    pub fn automatic_retry_safe(&self) -> bool {
        self.idempotency_contract == ToolIdempotencyContract::Idempotent
            && self.state == AgentRunToolExecutionState::TerminalNotAttempted
            && self.dispatch_attempt_count == 0
            && self.transport_status == ToolTransportStatus::NotAttempted
    }
}

/// The sole A2A implementation of the durable ToolGateway owner seam. It owns
/// only a cloned AgentRunStore handle, the canonical run id, and an endpoint
/// digest. The raw endpoint is validated then discarded during construction.
#[derive(Clone)]
pub struct AgentRunA2AToolExecutionOwner {
    store: AgentRunStore,
    run_id: String,
    endpoint_digest: String,
    unpersisted_run: Option<crate::agent::AgentRun>,
}

impl std::fmt::Debug for AgentRunA2AToolExecutionOwner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentRunA2AToolExecutionOwner")
            .field("run_id", &self.run_id)
            .field("endpoint", &"[DIGEST_ONLY]")
            .finish()
    }
}

impl AgentRunA2AToolExecutionOwner {
    pub fn new(store: AgentRunStore, run_id: impl Into<String>, base_url: &str) -> Result<Self> {
        let run_id = run_id.into();
        let parsed = uuid::Uuid::parse_str(&run_id)
            .map_err(|_| anyhow::anyhow!("a2a_tool_execution_owner_run_id_invalid"))?;
        if parsed.get_version() != Some(uuid::Version::Random)
            || parsed.hyphenated().to_string() != run_id
        {
            anyhow::bail!("a2a_tool_execution_owner_run_id_invalid");
        }
        let endpoint = crate::a2a::A2AClient::task_url(base_url)?;
        let (_, endpoint_digest) = crate::agent::metadata_safe::metadata_safe_value_digest(
            &serde_json::Value::String(endpoint),
        );
        Ok(Self {
            store,
            run_id,
            endpoint_digest,
            unpersisted_run: None,
        })
    }

    /// Construct the dedicated A2A command owner before either its parent run
    /// or prepared child exists. `prepare` commits both rows in one SQLite
    /// transaction, so a crash or injected child failure cannot leave a
    /// child-less Running AgentRun.
    pub fn new_for_unpersisted_run(
        store: AgentRunStore,
        run: crate::agent::AgentRun,
        base_url: &str,
    ) -> Result<Self> {
        let mut owner = Self::new(store, run.id.clone(), base_url)?;
        owner.unpersisted_run = Some(run);
        Ok(owner)
    }

    pub fn record_for_receipt(
        &self,
        receipt_id: &str,
    ) -> Result<Option<AgentRunToolExecutionRecord>> {
        self.store
            .get_agent_run_tool_execution(&self.run_id, receipt_id)
    }
}

impl DurableToolExecutionOwner for AgentRunA2AToolExecutionOwner {
    fn prepare(&self, receipt: &ToolExecutionReceipt) -> Result<()> {
        if let Some(run) = self.unpersisted_run.as_ref() {
            self.store
                .create_run_and_prepare_agent_run_a2a_tool_execution(
                    run,
                    &self.endpoint_digest,
                    receipt,
                )
        } else {
            self.store.prepare_agent_run_a2a_tool_execution(
                &self.run_id,
                &self.endpoint_digest,
                receipt,
            )
        }
    }

    fn before_dispatch_attempt(
        &self,
        receipt: &ToolExecutionReceipt,
        dispatch_kind: ToolDispatchKind,
    ) -> Result<()> {
        if dispatch_kind != ToolDispatchKind::A2a {
            anyhow::bail!("a2a_tool_execution_dispatch_kind_mismatch");
        }
        self.store
            .mark_agent_run_a2a_dispatch_attempted(&self.run_id, receipt)
    }

    fn response_observed(&self, receipt: &ToolExecutionReceipt) -> Result<()> {
        self.store
            .mark_agent_run_a2a_response_observed(&self.run_id, receipt)
    }

    fn terminal(&self, result: &crate::agent::ActionExecutionResult) -> Result<()> {
        self.store
            .commit_agent_run_a2a_tool_terminal(&self.run_id, result)
    }
}

#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRunToolExecutionFaultPoint {
    Prepare,
    DispatchAttempted,
    ResponseObserved,
    Terminal,
    BoundContentReceiptIssuance,
    AgentRunUpdate,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentRunReceiptKey;
    use crate::tool_execution_receipt::ToolExecutionReceiptRegistration;

    fn create_file_backed_owner(
        path: &std::path::Path,
    ) -> (
        AgentRunReceiptKey,
        AgentRunStore,
        AgentRunA2AToolExecutionOwner,
        ToolExecutionReceiptRegistration,
        String,
    ) {
        let key = AgentRunReceiptKey::test_key();
        let store = AgentRunStore::new_with_receipt_key(path, key.clone()).unwrap();
        let run = crate::agent::AgentRun::new_tool_execution_run("a2a.call_agent");
        let run_id = run.id.clone();
        store.create_run(&run).unwrap();
        let owner = AgentRunA2AToolExecutionOwner::new(
            store.clone(),
            run_id.clone(),
            "http://127.0.0.1:43123",
        )
        .unwrap();
        let registration =
            ToolExecutionReceiptRegistration::test_never_dispatched_external_mutation(
                Some(run_id.clone()),
                Some("builtin.a2a.call_agent".into()),
                "private request body that must not be stored".into(),
            );
        (key, store, owner, registration, run_id)
    }

    fn assert_recovered_parent(
        store: &AgentRunStore,
        run_id: &str,
        expected_parent_status: crate::agent::AgentRunStatus,
        expected_action_status: &str,
    ) {
        let parent = store.get_run(run_id).unwrap().unwrap();
        assert_eq!(parent.status, expected_parent_status);
        assert_eq!(parent.actions.len(), 1);
        assert_eq!(parent.observations.len(), 1);
        assert_eq!(parent.actions[0].status, expected_action_status);
        assert_eq!(
            parent.actions[0].id,
            parent.observations[0].action_id.as_deref().unwrap()
        );
        assert_eq!(
            parent.error.as_ref().map(|error| error.phase.as_str()),
            Some("startup_projection_recovery")
        );
    }

    #[test]
    fn dedicated_a2a_parent_and_prepared_child_are_one_atomic_create() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run = crate::agent::AgentRun::new_tool_execution_run("a2a.call_agent");
        let run_id = run.id.clone();
        let owner = AgentRunA2AToolExecutionOwner::new_for_unpersisted_run(
            store.clone(),
            run,
            "http://127.0.0.1:43123",
        )
        .unwrap();
        let registration =
            ToolExecutionReceiptRegistration::test_never_dispatched_external_mutation(
                Some(run_id.clone()),
                Some("builtin.a2a.call_agent".into()),
                "atomic prepare body".into(),
            );
        store
            .install_tool_execution_fault_for_test(AgentRunToolExecutionFaultPoint::Prepare)
            .unwrap();

        assert!(owner.prepare(&registration.snapshot()).is_err());
        assert!(store.get_run(&run_id).unwrap().is_none());
        assert!(store
            .list_agent_run_tool_executions(&run_id)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn canonical_delete_blocks_late_a2a_dispatch_child_transition() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("deleted-before-dispatch.db");
        let (_key, store, owner, registration, run_id) = create_file_backed_owner(&path);
        let receipt = registration.snapshot();
        owner.prepare(&receipt).unwrap();
        store
            .delete_run_with_tombstone(&run_id, Some("delete before late dispatch"))
            .unwrap();

        let error = owner
            .before_dispatch_attempt(&receipt, ToolDispatchKind::A2a)
            .expect_err("deleted parent must fence a late dispatch transition")
            .to_string();
        assert!(error.contains("agent_run_a2a_parent_inactive"), "{error}");
        assert_eq!(
            store
                .raw_agent_run_tool_execution_state_for_test(&run_id, &receipt.receipt_id)
                .unwrap(),
            Some(AgentRunToolExecutionState::Prepared)
        );
    }

    #[test]
    fn canonical_delete_blocks_late_a2a_response_child_transition() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("deleted-before-response.db");
        let (_key, store, owner, registration, run_id) = create_file_backed_owner(&path);
        let prepared_receipt = registration.snapshot();
        owner.prepare(&prepared_receipt).unwrap();
        owner
            .before_dispatch_attempt(&prepared_receipt, ToolDispatchKind::A2a)
            .unwrap();
        registration.test_mark_a2a_dispatch_attempted();
        registration.test_mark_a2a_response_observed();
        store
            .delete_run_with_tombstone(&run_id, Some("delete before late response"))
            .unwrap();

        let error = owner
            .response_observed(&registration.snapshot())
            .expect_err("deleted parent must fence a late response transition")
            .to_string();
        assert!(error.contains("agent_run_a2a_parent_inactive"), "{error}");
        assert_eq!(
            store
                .raw_agent_run_tool_execution_state_for_test(&run_id, &prepared_receipt.receipt_id)
                .unwrap(),
            Some(AgentRunToolExecutionState::DispatchAttempted)
        );
    }

    #[test]
    fn terminal_parent_blocks_late_a2a_dispatch_child_transition() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("terminal-before-dispatch.db");
        let (_key, store, owner, registration, run_id) = create_file_backed_owner(&path);
        let receipt = registration.snapshot();
        owner.prepare(&receipt).unwrap();

        let mut parent = store.get_run(&run_id).unwrap().unwrap();
        parent.status = crate::agent::AgentRunStatus::Completed;
        parent.finished_at = Some(chrono::Utc::now());
        store.update_run(&parent).unwrap();

        let error = owner
            .before_dispatch_attempt(&receipt, ToolDispatchKind::A2a)
            .expect_err("terminal parent must fence a late dispatch transition")
            .to_string();
        assert!(
            error.contains("agent_run_a2a_parent_not_running"),
            "{error}"
        );
        assert_eq!(
            store
                .raw_agent_run_tool_execution_state_for_test(&run_id, &receipt.receipt_id)
                .unwrap(),
            Some(AgentRunToolExecutionState::Prepared)
        );
    }

    #[test]
    fn terminal_parent_blocks_late_a2a_response_child_transition() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("terminal-before-response.db");
        let (_key, store, owner, registration, run_id) = create_file_backed_owner(&path);
        let prepared_receipt = registration.snapshot();
        owner.prepare(&prepared_receipt).unwrap();
        owner
            .before_dispatch_attempt(&prepared_receipt, ToolDispatchKind::A2a)
            .unwrap();
        registration.test_mark_a2a_dispatch_attempted();
        registration.test_mark_a2a_response_observed();

        let mut parent = store.get_run(&run_id).unwrap().unwrap();
        parent.status = crate::agent::AgentRunStatus::RemoteUnknown;
        parent.finished_at = Some(chrono::Utc::now());
        store.update_run(&parent).unwrap();

        let error = owner
            .response_observed(&registration.snapshot())
            .expect_err("terminal parent must fence a late response transition")
            .to_string();
        assert!(
            error.contains("agent_run_a2a_parent_not_running"),
            "{error}"
        );
        assert_eq!(
            store
                .raw_agent_run_tool_execution_state_for_test(&run_id, &prepared_receipt.receipt_id)
                .unwrap(),
            Some(AgentRunToolExecutionState::DispatchAttempted)
        );
    }

    #[test]
    fn startup_recovery_settles_prepared_a2a_as_not_attempted() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("prepared-a2a-owner.db");
        let (key, store, owner, registration, run_id) = create_file_backed_owner(&path);
        let receipt = registration.snapshot();
        owner.prepare(&receipt).unwrap();
        let prepared = owner
            .record_for_receipt(&receipt.receipt_id)
            .unwrap()
            .unwrap();
        assert_eq!(prepared.state, AgentRunToolExecutionState::Prepared);
        assert_eq!(prepared.dispatch_attempt_count, 0);
        assert!(!prepared.endpoint_digest.contains("127.0.0.1"));
        assert!(!prepared.request_digest.contains("private request body"));
        drop(owner);
        drop(store);

        let reopened = AgentRunStore::new_with_receipt_key(&path, key).unwrap();
        let recovered = reopened
            .list_agent_run_tool_executions(&run_id)
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(
            recovered.state,
            AgentRunToolExecutionState::TerminalNotAttempted
        );
        assert_eq!(
            recovered.transport_status,
            ToolTransportStatus::NotAttempted
        );
        assert_eq!(recovered.dispatch_attempt_count, 0);
        assert!(!recovered.automatic_retry_safe());
        assert_recovered_parent(
            &reopened,
            &run_id,
            crate::agent::AgentRunStatus::Failed,
            "failed",
        );
    }

    #[test]
    fn startup_recovery_settles_attempted_a2a_as_remote_unknown_without_retry() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("attempted-a2a-owner.db");
        let (key, store, owner, registration, run_id) = create_file_backed_owner(&path);
        let prepared_receipt = registration.snapshot();
        owner.prepare(&prepared_receipt).unwrap();
        owner
            .before_dispatch_attempt(&prepared_receipt, ToolDispatchKind::A2a)
            .unwrap();
        registration.test_mark_a2a_dispatch_attempted();
        drop(owner);
        drop(store);

        let reopened = AgentRunStore::new_with_receipt_key(&path, key).unwrap();
        let recovered = reopened
            .list_agent_run_tool_executions(&run_id)
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(
            recovered.state,
            AgentRunToolExecutionState::TerminalRemoteUnknown
        );
        assert_eq!(
            recovered.transport_status,
            ToolTransportStatus::RemoteUnknown
        );
        assert_eq!(recovered.effect_status, ToolEffectStatus::Unknown);
        assert_eq!(recovered.dispatch_attempt_count, 1);
        assert!(!recovered.automatic_retry_safe());
        assert_recovered_parent(
            &reopened,
            &run_id,
            crate::agent::AgentRunStatus::RemoteUnknown,
            "remote_unknown",
        );
    }

    #[test]
    fn startup_recovery_keeps_response_fact_but_terminal_remains_remote_unknown() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("response-a2a-owner.db");
        let (key, store, owner, registration, run_id) = create_file_backed_owner(&path);
        let prepared_receipt = registration.snapshot();
        owner.prepare(&prepared_receipt).unwrap();
        owner
            .before_dispatch_attempt(&prepared_receipt, ToolDispatchKind::A2a)
            .unwrap();
        registration.test_mark_a2a_dispatch_attempted();
        registration.test_mark_a2a_response_observed();
        owner.response_observed(&registration.snapshot()).unwrap();
        drop(owner);
        drop(store);

        let reopened = AgentRunStore::new_with_receipt_key(&path, key).unwrap();
        let recovered = reopened
            .list_agent_run_tool_executions(&run_id)
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(
            recovered.state,
            AgentRunToolExecutionState::TerminalRemoteUnknown
        );
        assert!(recovered.response_observed_at.is_some());
        assert_eq!(recovered.execution_outcome, ToolExecutionOutcome::Unknown);
        assert!(!recovered.automatic_retry_safe());
        assert_recovered_parent(
            &reopened,
            &run_id,
            crate::agent::AgentRunStatus::RemoteUnknown,
            "remote_unknown",
        );
    }

    #[test]
    fn startup_recovery_projects_terminal_success_as_succeeded_not_success() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("terminal-success-parent-drift.db");
        let (key, store, owner, registration, run_id) = create_file_backed_owner(&path);
        let receipt = registration.snapshot();
        owner.prepare(&receipt).unwrap();
        store
            .install_terminal_succeeded_parent_drift_for_test(&run_id, &receipt.receipt_id)
            .unwrap();
        drop(owner);
        drop(store);

        let reopened = AgentRunStore::new_with_receipt_key(&path, key).unwrap();
        let parent = reopened.get_run(&run_id).unwrap().unwrap();
        assert_eq!(parent.status, crate::agent::AgentRunStatus::Completed);
        assert_eq!(parent.actions.len(), 1);
        assert_eq!(parent.actions[0].status, "succeeded");
        assert_ne!(parent.actions[0].status, "success");
        let child = reopened
            .list_agent_run_tool_executions(&run_id)
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(child.state, AgentRunToolExecutionState::TerminalSucceeded);
    }
}
