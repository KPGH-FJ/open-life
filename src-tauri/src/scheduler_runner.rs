//! Scheduled task runner: atomically claims due tasks from the canonical TaskStore
//! and triggers AgentLoop executions.
//! Runs as a background tokio task spawned during app bootstrap.

use crate::AppState;
use openlife_core::agent::agent_loop::{AgentLoopConfig, AgentRole};
use openlife_core::agent::{
    AgentLoop, AgentRunStatus, AgentTask, AgentTaskKind, RuntimePolicyContext, ToolDispatchAttempt,
    ToolDispatchObserver, ToolExecutionReceipt, ToolStartedTransitionObserver, ToolTransportStatus,
};
use openlife_core::layer::Layer;
use openlife_core::llm::{ChatMessage, ProviderDataRoute};
#[cfg(test)]
use openlife_core::llm::{ProviderPolicyAuthority, ProviderPolicyReceiptEvidence};
#[cfg(test)]
use openlife_core::scheduler::ProviderInvocationProgress;
use openlife_core::scheduler::{
    ScheduledProviderLocalAbortCause, ScheduledProviderTruthAdmissionHandle,
};
#[cfg(test)]
use openlife_core::tasks::ScheduledClaimSettlement;
use openlife_core::tasks::{ScheduledTaskClaim, TaskStore};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Check interval (seconds) for the scheduled task runner.
const CHECK_INTERVAL_SECONDS: u64 = 60;
const TASK_EXECUTION_TIMEOUT_SECONDS: u64 = 60;
const TASK_LEASE_SECONDS: i64 = 90;
const MAX_CONCURRENT_SCHEDULED_TASKS: usize = 4;

/// Start the scheduled task runner as a background loop.
pub fn start_scheduler_runner(state: Arc<AppState>) {
    if let Err(error) = state.persistence_coordinator.require_effects_allowed() {
        log::warn!("[scheduler_runner] disabled by persistence gate: {error}");
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(error) = run_scheduler_cycle(&state).await {
                log::warn!("[scheduler_runner] Cycle failed: {}", error);
            }
            tokio::time::sleep(std::time::Duration::from_secs(CHECK_INTERVAL_SECONDS)).await;
        }
    });
}

async fn run_scheduler_cycle(state: &Arc<AppState>) -> Result<(), String> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| error.to_string())?;
    let now = chrono::Utc::now();
    state
        .scheduled_task_store
        .reconcile_previous_process_claims(now)
        .map_err(|error| {
            state
                .persistence_coordinator
                .register_runtime_durable_failure("TaskStore", error.to_string());
            format!("scheduled previous-process reconciliation failed: {error}")
        })?;
    state
        .scheduled_task_store
        .reconcile_expired_claims(now)
        .map_err(|error| {
            state
                .persistence_coordinator
                .register_runtime_durable_failure("TaskStore", error.to_string());
            format!("scheduled task reconciliation failed: {error}")
        })?;

    let mut executions = tokio::task::JoinSet::new();
    let mut first_error = None;
    let mut claiming_enabled = true;
    loop {
        while claiming_enabled && executions.len() < MAX_CONCURRENT_SCHEDULED_TASKS {
            let claim = match state.scheduled_task_store.claim_next_due(
                chrono::Utc::now(),
                chrono::Duration::seconds(TASK_LEASE_SECONDS),
            ) {
                Ok(claim) => claim,
                Err(error) => {
                    state
                        .persistence_coordinator
                        .register_runtime_durable_failure("TaskStore", error.to_string());
                    let error = format!("scheduled task claim failed: {error}");
                    log::warn!("[scheduler_runner] {error}");
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    claiming_enabled = false;
                    break;
                }
            };
            let Some(claim) = claim else {
                claiming_enabled = false;
                break;
            };
            executions.spawn(run_claimed_task(Arc::clone(state), claim));
        }

        let Some(joined) = executions.join_next().await else {
            break;
        };
        let outcome = match joined {
            Ok(outcome) => outcome,
            Err(error) => Err(format!(
                "scheduled execution task aborted: {}",
                digest_text(&error.to_string())
            )),
        };
        if let Err(error) = outcome {
            log::warn!("[scheduler_runner] Claimed execution failed: {error}");
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

async fn run_claimed_task(state: Arc<AppState>, claim: ScheduledTaskClaim) -> Result<(), String> {
    let claim = Arc::new(claim);
    if !state
        .scheduled_task_store
        .begin_claim_execution(&claim)
        .map_err(|error| format!("scheduled execution boundary failed: {error}"))?
    {
        return Err(format!(
            "scheduled task '{}' lost its claim before execution",
            claim.task().id
        ));
    }

    let observer_state = Arc::new(SchedulerReceiptObserverState::default());
    log::info!(
        "[scheduler_runner] Executing task_ref={} attempt={} route={} grant={}",
        digest_text(&claim.task().id),
        claim.attempt_number(),
        provider_data_route_label(claim.provider_grant().data_route),
        claim.provider_grant().grant_id,
    );
    let execution = tokio::time::timeout(
        std::time::Duration::from_secs(TASK_EXECUTION_TIMEOUT_SECONDS),
        execute_scheduled_task(&state, Arc::clone(&claim), Arc::clone(&observer_state)),
    )
    .await;
    match execution {
        Ok(Ok(result)) => {
            match state.scheduled_task_store.complete_claim(
                &claim,
                &result.agent_run_id,
                &result.delivery_ref,
                &result.result_digest,
            ) {
                Ok(true) => {
                    log::info!(
                        "[scheduler_runner] Completed task_ref={} attempt={}",
                        digest_text(&claim.task().id),
                        claim.attempt_number(),
                    );
                }
                Ok(false) => {
                    quarantine_after_truth_failure(
                        &state.scheduled_task_store,
                        &claim,
                        "scheduled_completion_claim_lost",
                    )?;
                }
                Err(error) => {
                    quarantine_after_truth_failure(
                        &state.scheduled_task_store,
                        &claim,
                        "scheduled_completion_truth_rejected",
                    )?;
                    return Err(format!(
                        "scheduled task completion rejected: {}",
                        digest_text(&error.to_string())
                    ));
                }
            }
        }
        Ok(Err(failure)) => {
            persist_provider_local_abort_truth(
                &state.scheduled_task_store,
                &claim,
                &observer_state,
                ScheduledProviderLocalAbortCause::RuntimeFutureAborted,
            )?;
            let settlement = settle_claim_after_execution_error(
                &state.scheduled_task_store,
                &claim,
                &observer_state,
                &failure,
            )?;
            log::warn!(
                "[scheduler_runner] Task attempt failed: task_ref={} attempt={} reason={} error={} settlement={:?}",
                digest_text(&claim.task().id),
                claim.attempt_number(),
                failure.reason_code,
                failure.error_digest,
                settlement,
            );
        }
        Err(_) => {
            persist_provider_local_abort_truth(
                &state.scheduled_task_store,
                &claim,
                &observer_state,
                ScheduledProviderLocalAbortCause::ExecutionTimeout,
            )?;
            let settlement = settle_claim_after_execution_timeout(
                &state.scheduled_task_store,
                &claim,
                &observer_state,
            )?;
            log::warn!(
                "[scheduler_runner] Task attempt timed out: task_ref={} attempt={} settlement={:?}",
                digest_text(&claim.task().id),
                claim.attempt_number(),
                settlement,
            );
        }
    }
    Ok(())
}

fn persist_provider_local_abort_truth(
    store: &TaskStore,
    claim: &ScheduledTaskClaim,
    observer_state: &SchedulerReceiptObserverState,
    cause: ScheduledProviderLocalAbortCause,
) -> Result<usize, String> {
    let persist = || -> Result<usize, String> {
        let Some(handle) = observer_state
            .provider_truth_handle()
            .map_err(|error| format!("scheduled provider truth handle unavailable: {error}"))?
        else {
            return Ok(0);
        };
        let admissions = handle
            .take_remote_unknown_after_local_abort(cause)
            .map_err(|error| format!("scheduled provider local-abort truth rejected: {error}"))?;
        let admission_count = admissions.len();
        for admission in admissions {
            store
                .record_provider_truth(claim, admission)
                .map_err(|error| {
                    format!("scheduled provider local-abort truth persistence failed: {error}")
                })?;
        }
        Ok(admission_count)
    };
    match persist() {
        Ok(count) => Ok(count),
        Err(error) => {
            observer_state.record_persistence_failure(&error);
            quarantine_after_truth_failure(
                store,
                claim,
                "scheduled_provider_local_abort_truth_failed",
            )?;
            Err(error)
        }
    }
}

fn settle_claim_after_execution_error(
    store: &TaskStore,
    claim: &ScheduledTaskClaim,
    observer_state: &SchedulerReceiptObserverState,
    failure: &ScheduledExecutionFailure,
) -> Result<openlife_core::tasks::ScheduledClaimSettlement, String> {
    if observer_state.adapter_edge_crossed() {
        // The adapter owned the irreversible boundary before the fallible
        // durable-start callback ran. Even if identity validation or the
        // durable insert failed, absence of a TaskStore start row is no longer
        // evidence that dispatch did not happen.
        quarantine_after_truth_failure(store, claim, failure.reason_code)?;
        return Ok(openlife_core::tasks::ScheduledClaimSettlement::UnknownRequiresReconciliation);
    }
    store
        .settle_claim_after_error(claim, failure.reason_code, Some(&failure.error_digest))
        .map_err(|error| format!("scheduled task failure settlement failed: {error}"))
}

fn settle_claim_after_execution_timeout(
    store: &TaskStore,
    claim: &ScheduledTaskClaim,
    observer_state: &SchedulerReceiptObserverState,
) -> Result<openlife_core::tasks::ScheduledClaimSettlement, String> {
    if observer_state.adapter_edge_crossed() {
        quarantine_after_truth_failure(store, claim, "scheduled_timeout_after_adapter_edge")?;
        return Ok(openlife_core::tasks::ScheduledClaimSettlement::UnknownRequiresReconciliation);
    }
    store
        .settle_claim_after_timeout(claim)
        .map_err(|error| format!("scheduled task timeout settlement failed: {error}"))
}

fn quarantine_after_truth_failure(
    store: &TaskStore,
    claim: &ScheduledTaskClaim,
    reason_code: &str,
) -> Result<(), String> {
    let quarantined = store
        .quarantine_claim_unknown(claim, reason_code)
        .map_err(|error| format!("scheduled unknown quarantine failed: {error}"))?;
    if !quarantined {
        return Err("scheduled unknown quarantine lost its canonical claim".into());
    }
    Ok(())
}

#[derive(Debug)]
struct ScheduledExecutionResult {
    agent_run_id: String,
    delivery_ref: String,
    result_digest: String,
}

#[derive(Debug)]
struct ScheduledExecutionFailure {
    reason_code: &'static str,
    error_digest: String,
}

impl ScheduledExecutionFailure {
    fn from_error(reason_code: &'static str, error: impl std::fmt::Display) -> Self {
        Self {
            reason_code,
            error_digest: digest_text(&error.to_string()),
        }
    }
}

#[derive(Default)]
struct SchedulerReceiptObserverState {
    persistence_failed: AtomicBool,
    adapter_edge_crossed: AtomicBool,
    last_error_digest: Mutex<Option<String>>,
    prepared_tools: Mutex<std::collections::HashMap<String, ToolDispatchAttempt>>,
    provider_truth_handle: Mutex<Option<ScheduledProviderTruthAdmissionHandle>>,
}

impl SchedulerReceiptObserverState {
    fn record_persistence_failure(&self, error: impl std::fmt::Display) {
        self.persistence_failed.store(true, Ordering::SeqCst);
        let mut last_error = self
            .last_error_digest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *last_error = Some(digest_text(&error.to_string()));
    }

    fn persistence_failed(&self) -> bool {
        self.persistence_failed.load(Ordering::SeqCst)
    }

    fn mark_adapter_edge_crossed(&self) {
        self.adapter_edge_crossed.store(true, Ordering::SeqCst);
    }

    fn adapter_edge_crossed(&self) -> bool {
        self.adapter_edge_crossed.load(Ordering::SeqCst)
    }

    fn install_provider_truth_handle(
        &self,
        handle: ScheduledProviderTruthAdmissionHandle,
    ) -> anyhow::Result<()> {
        let mut slot = self
            .provider_truth_handle
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduler provider truth handle is poisoned"))?;
        if slot.is_some() {
            anyhow::bail!("scheduler provider truth handle cannot be rebound");
        }
        *slot = Some(handle);
        Ok(())
    }

    fn provider_truth_handle(
        &self,
    ) -> anyhow::Result<Option<ScheduledProviderTruthAdmissionHandle>> {
        self.provider_truth_handle
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduler provider truth handle is poisoned"))
            .map(|slot| slot.clone())
    }
}

struct SchedulerToolDispatchObserver {
    store: Arc<TaskStore>,
    claim: Arc<ScheduledTaskClaim>,
    observer_state: Arc<SchedulerReceiptObserverState>,
    persistence_coordinator: Arc<crate::persistence_coordinator::PersistenceCoordinator>,
}

struct SchedulerCanonicalWritePermit {
    observer_state: Arc<SchedulerReceiptObserverState>,
    finished: bool,
}

impl openlife_core::agent::CanonicalWritePermit for SchedulerCanonicalWritePermit {
    fn finish_committed(mut self: Box<Self>) {
        self.finished = true;
    }

    fn finish_failed(mut self: Box<Self>) {
        self.finished = true;
    }

    fn finish_noop(mut self: Box<Self>) {
        self.finished = true;
    }
}

impl Drop for SchedulerCanonicalWritePermit {
    fn drop(&mut self) {
        if !self.finished {
            self.observer_state
                .record_persistence_failure("scheduler_canonical_write_outcome_unknown");
        }
    }
}

impl openlife_core::agent::CanonicalWriteAdmission for SchedulerToolDispatchObserver {
    fn acquire(
        &self,
        request: openlife_core::agent::CanonicalWriteAdmissionRequest,
    ) -> Result<
        Box<dyn openlife_core::agent::CanonicalWritePermit>,
        openlife_core::agent::CanonicalWriteAdmissionRejection,
    > {
        if self
            .persistence_coordinator
            .require_effects_allowed()
            .is_err()
        {
            return Err(openlife_core::agent::CanonicalWriteAdmissionRejection::new(
                "persistence_effects_blocked",
            ));
        }
        if request.domain != "proposal" || !request.object_ref.starts_with("proposal:") {
            return Err(openlife_core::agent::CanonicalWriteAdmissionRejection::new(
                "scheduler_canonical_write_scope_invalid",
            ));
        }
        if self.observer_state.persistence_failed() {
            return Err(openlife_core::agent::CanonicalWriteAdmissionRejection::new(
                "scheduler_receipt_persistence_failed",
            ));
        }
        let owns_claim = self
            .store
            .owns_executing_claim(&self.claim)
            .map_err(|error| {
                self.observer_state.record_persistence_failure(&error);
                openlife_core::agent::CanonicalWriteAdmissionRejection::new(
                    "scheduler_claim_ownership_unknown",
                )
            })?;
        if !owns_claim {
            return Err(openlife_core::agent::CanonicalWriteAdmissionRejection::new(
                "scheduler_claim_not_owned",
            ));
        }
        Ok(Box::new(SchedulerCanonicalWritePermit {
            observer_state: Arc::clone(&self.observer_state),
            finished: false,
        }))
    }
}

#[async_trait::async_trait]
impl ToolDispatchObserver for SchedulerToolDispatchObserver {
    async fn before_dispatch(&self, attempt: &ToolDispatchAttempt) -> anyhow::Result<()> {
        self.persistence_coordinator
            .require_effects_allowed()
            .map_err(anyhow::Error::msg)?;
        if self.observer_state.persistence_failed() {
            return Err(anyhow::anyhow!(
                "scheduler_provider_receipt_persistence_failed"
            ));
        }
        if !self
            .store
            .owns_executing_claim(&self.claim)
            .map_err(|error| {
                self.observer_state.record_persistence_failure(&error);
                anyhow::anyhow!("scheduler_tool_dispatch_claim_validation_failed")
            })?
        {
            return Err(anyhow::anyhow!("scheduler_tool_dispatch_claim_not_owned"));
        }
        let mut prepared = self
            .observer_state
            .prepared_tools
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match prepared.get(&attempt.receipt_id) {
            Some(existing) if existing == attempt => Ok(()),
            Some(_) => Err(anyhow::anyhow!(
                "scheduler_tool_dispatch_prepared_identity_conflict"
            )),
            None => {
                prepared.insert(attempt.receipt_id.clone(), attempt.clone());
                Ok(())
            }
        }
    }
}

#[async_trait::async_trait]
impl ToolStartedTransitionObserver for SchedulerToolDispatchObserver {
    async fn after_dispatch(&self, receipt: &ToolExecutionReceipt) -> anyhow::Result<()> {
        // `after_dispatch` is invoked by the concrete adapter immediately
        // after it has crossed its local/network boundary. Record that
        // irreversible truth before any identity check or durable I/O can
        // fail. Settlement must never infer "pre-dispatch" from a missing row
        // after this point.
        self.observer_state.mark_adapter_edge_crossed();
        let attempt = {
            let prepared = self
                .observer_state
                .prepared_tools
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            prepared.get(&receipt.receipt_id).cloned().ok_or_else(|| {
                anyhow::anyhow!("scheduler_tool_dispatch_missing_prepared_identity")
            })?
        };
        self.store
            .record_tool_dispatch_started(&self.claim, &attempt, receipt)
            .map_err(|error| {
                self.observer_state.record_persistence_failure(&error);
                anyhow::anyhow!("scheduler_tool_dispatch_started_truth_rejected")
            })?;
        self.observer_state
            .prepared_tools
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&receipt.receipt_id);
        Ok(())
    }
}

async fn execute_scheduled_task(
    state: &Arc<AppState>,
    claim: Arc<ScheduledTaskClaim>,
    observer_state: Arc<SchedulerReceiptObserverState>,
) -> Result<ScheduledExecutionResult, ScheduledExecutionFailure> {
    let resources =
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_scheduler(state)
            .await
            .map_err(|error| {
                ScheduledExecutionFailure::from_error("scheduled_tool_resources_unavailable", error)
            })?;
    let provider_runtime = state.provider_runtime_snapshot().await;
    if !provider_runtime.coherent {
        return Err(ScheduledExecutionFailure::from_error(
            "scheduled_provider_runtime_generation_incoherent",
            "provider config and executable adapter belong to different generations",
        ));
    }
    let runtime_scheduler = provider_runtime.scheduler.clone();
    let (scheduler, provider_truth_handle) = provider_runtime
        .scheduler
        .bind_scheduled_provider_truth_scope(
            Arc::clone(&state.scheduled_task_store),
            Arc::clone(&claim),
        )
        .map_err(|error| {
            ScheduledExecutionFailure::from_error(
                "scheduled_provider_truth_scope_binding_failed",
                error,
            )
        })?;
    observer_state
        .install_provider_truth_handle(provider_truth_handle.clone())
        .map_err(|error| {
            ScheduledExecutionFailure::from_error(
                "scheduled_provider_truth_handle_install_failed",
                error,
            )
        })?;
    let safe_paths = resources.governed.shared.safe_paths.clone();
    let calendar_ics_paths = resources.governed.calendar_ics_paths.clone();
    let network_policy = provider_runtime.config.system.network_policy;

    let agent_runtime = openlife_core::agent::AgentRuntime::new_with_runtime_config(
        runtime_scheduler,
        network_policy.clone(),
        resources.agent_runtime_config.clone(),
    );
    let tool_gateway = openlife_core::agent::ToolGateway::from_executor_config(
        openlife_core::agent::ActionExecutorConfig {
            search_provider: resources.governed.search_provider.clone(),
            ..Default::default()
        },
    );
    let loop_config = AgentLoopConfig {
        max_steps: 2,
        max_tool_calls: 4,
        timeout_seconds: 60,
        allow_writes: false,
        allow_cloud: claim.provider_grant().allows_cloud(),
        shutdown_notify: Some(state.shutdown_notify.clone()),
        role: AgentRole::Planner,
        toolset_allowlist: vec![
            "state.read".into(),
            "memory.search".into(),
            "proposal.create".into(),
        ],
        tool_action_allowlist: Vec::new(),
    };
    let agent_loop = AgentLoop::new_scheduled(agent_runtime, tool_gateway, scheduler, loop_config);

    let task = AgentTask {
        kind: AgentTaskKind::Proactive,
        session_id: format!("scheduled:{}", claim.attempt_id()),
        user_text: claim.task().description.clone(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: claim.task().description.clone(),
        }],
        layer: Layer::L2,
    };
    let policy_context = RuntimePolicyContext::from_scheduled_claim(&claim).map_err(|error| {
        ScheduledExecutionFailure::from_error("scheduled_provider_policy_invalid", error)
    })?;
    let tools_prompt = String::new();
    let tool_observer = SchedulerToolDispatchObserver {
        store: Arc::clone(&state.scheduled_task_store),
        claim: Arc::clone(&claim),
        observer_state: Arc::clone(&observer_state),
        persistence_coordinator: Arc::clone(&state.persistence_coordinator),
    };

    let mut loop_result = {
        let mut action_ctx = openlife_core::agent::ActionExecutionContext::new(
            &resources.governed.shared.registry,
            &resources.governed.shared.permission_store,
            &resources.governed.shared.audit_store,
            &resources.governed.shared.privacy_engine,
            &safe_paths,
        )
        .with_tool_audit_persistence_observer(
            resources.governed.shared.persistence_coordinator.as_ref(),
        )
        .with_durable_store_failure_observer(
            resources.governed.shared.persistence_coordinator.as_ref(),
        )
        .with_calendar_ics_paths(&calendar_ics_paths)
        .with_memory_store(&resources.governed.memory_store)
        .with_proposal_store(&resources.proposal_store)
        .with_agent_run_store(&resources.agent_run_store)
        .with_network_policy(&network_policy)
        .with_external_write_proposal_policy(policy_context.external_write_requires_proposal())
        .with_tool_dispatch_observer(&tool_observer)
        .with_tool_started_transition_observer(&tool_observer)
        .with_canonical_write_admission(&tool_observer);
        if let Some(reader) = resources
            .governed
            .memory_lifecycle_retrieval_reader
            .as_ref()
        {
            action_ctx = action_ctx.with_memory_lifecycle_retrieval_reader(reader);
        }
        if let Some(canonical_state) = resources.governed.canonical_state.as_ref() {
            action_ctx = action_ctx.with_canonical_state(canonical_state);
        }
        agent_loop
            .run(
                &task,
                &tools_prompt,
                None,
                resources.governed.shared.privacy_engine.clone(),
                &action_ctx,
                policy_context,
            )
            .await
            .map_err(|error| {
                crate::terminal_owner_write_gateway::register_agent_run_store_error(state, &error);
                ScheduledExecutionFailure::from_error("scheduled_agent_loop_failed", error)
            })
    }?;

    if observer_state.persistence_failed() {
        return Err(ScheduledExecutionFailure::from_error(
            "scheduler_receipt_persistence_failed",
            "adapter edge receipt could not be persisted",
        ));
    }
    project_tool_terminal_receipts(&state.scheduled_task_store, &claim, &loop_result.run).map_err(
        |error| {
            ScheduledExecutionFailure::from_error("scheduled_tool_receipt_projection_failed", error)
        },
    )?;

    let response = validate_scheduled_task_terminal(
        loop_result.run.status,
        loop_result.run.error.is_some(),
        &loop_result.stop_reason,
        loop_result
            .run
            .actions
            .iter()
            .map(|action| action.status.as_str()),
        loop_result.final_response,
    )
    .map_err(|reason_code| {
        ScheduledExecutionFailure::from_error(reason_code, "terminal truth rejected")
    })?;
    let delivery = crate::memory_gateway::save_conversation_message_idempotent_with_state(
        &task.session_id,
        &ChatMessage {
            role: "assistant".into(),
            content: response,
        },
        &format!("scheduled:{}:final", claim.attempt_id()),
        state,
    )
    .await
    .map_err(|error| {
        ScheduledExecutionFailure::from_error("scheduled_final_delivery_persistence_failed", error)
    })?;
    state
        .scheduled_task_store
        .stage_claim_result_delivery(&claim, &delivery.canonical_ref, &delivery.content_digest)
        .map_err(|error| {
            ScheduledExecutionFailure::from_error("scheduled_result_reference_failed", error)
        })?;
    loop_result.run.output_preview = Some(format!(
        "canonical_delivery_ref={};digest={}",
        delivery.canonical_ref, delivery.content_digest
    ));
    crate::terminal_owner_write_gateway::create_agent_run(state, &loop_result.run)
        .await
        .map_err(|error| {
            ScheduledExecutionFailure::from_error("scheduled_agent_run_persistence_failed", error)
        })?;
    Ok(ScheduledExecutionResult {
        agent_run_id: loop_result.run.id,
        delivery_ref: delivery.canonical_ref,
        result_digest: delivery.content_digest,
    })
}

fn project_tool_terminal_receipts(
    store: &TaskStore,
    claim: &ScheduledTaskClaim,
    run: &openlife_core::agent::AgentRun,
) -> anyhow::Result<()> {
    for action in run.actions.iter().filter(|action| {
        matches!(
            action.action_type.as_str(),
            "mcp_tool" | "builtin_tool" | "plugin_tool" | "memory_search" | "session_search"
        )
    }) {
        let receipt_value = action
            .output
            .as_ref()
            .and_then(|output| output.get("toolExecutionReceipt"))
            .cloned()
            .or_else(|| {
                run.observations
                    .iter()
                    .find(|observation| observation.action_id.as_deref() == Some(&action.id))
                    .and_then(|observation| observation.structured_result.as_ref())
                    .and_then(|result| result.get("toolExecutionReceipt"))
                    .cloned()
            });
        let Some(receipt_value) = receipt_value else {
            anyhow::bail!("scheduled tool action is missing its typed execution receipt");
        };
        let receipt: ToolExecutionReceipt = serde_json::from_value(receipt_value)
            .map_err(|_| anyhow::anyhow!("scheduled tool execution receipt is malformed"))?;
        if receipt.transport_status == ToolTransportStatus::NotAttempted {
            continue;
        }
        store.record_tool_terminal(claim, &receipt)?;
    }
    Ok(())
}

fn validate_scheduled_task_terminal<'a>(
    status: AgentRunStatus,
    run_has_error: bool,
    stop_reason: &str,
    action_statuses: impl Iterator<Item = &'a str>,
    final_response: String,
) -> Result<String, &'static str> {
    if status != AgentRunStatus::Completed || run_has_error {
        return Err("scheduled_agent_loop_failed");
    }
    if stop_reason != "no_tools" {
        return Err("scheduled_agent_loop_incomplete");
    }
    if action_statuses
        .into_iter()
        .any(|status| status != "succeeded")
    {
        return Err("scheduled_tool_terminal_truth_rejected");
    }
    if final_response.trim().is_empty() {
        return Err("scheduled_final_response_missing");
    }
    Ok(final_response)
}

fn digest_text(value: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(value.as_bytes()))
}

fn provider_data_route_label(route: ProviderDataRoute) -> &'static str {
    match route {
        ProviderDataRoute::LocalOnly => "local_only",
        ProviderDataRoute::PolicyAllowed => "policy_allowed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::{AgentAction, AgentRun};
    use openlife_core::llm::{ProviderInvocationReceipt, ProviderInvocationStatus};
    use openlife_core::tasks::{ScheduledReconciliationTestResolution, ScheduledTask};

    fn due_task() -> ScheduledTask {
        let mut task = ScheduledTask::new(
            "Scheduled review",
            "Prepare a short review",
            Some((chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339()),
            "medium",
        );
        task.id = "scheduler-runner-test-task".into();
        task.source_proposal_id = Some("scheduler-runner-test-proposal".into());
        task.seal_deterministic_local_provider_grant();
        task
    }

    fn isolated_persistence_coordinator(
    ) -> Arc<crate::persistence_coordinator::PersistenceCoordinator> {
        Arc::new(crate::persistence_coordinator::PersistenceCoordinator::isolated_evaluation())
    }

    fn healthy_release_persistence_coordinator(
    ) -> Arc<crate::persistence_coordinator::PersistenceCoordinator> {
        let coordinator =
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap();
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        coordinator.seal();
        Arc::new(coordinator)
    }

    fn scheduled_provider_evidence(claim: &ScheduledTaskClaim) -> ProviderPolicyReceiptEvidence {
        ProviderPolicyReceiptEvidence {
            decision_id: claim.provider_grant().policy_decision_digest.clone(),
            policy_version: claim.provider_grant().policy_version.clone(),
            issuing_authority: ProviderPolicyAuthority::ScheduledPolicy,
            effective_data_route: claim.provider_grant().data_route,
            effective_local_restriction: None,
            subject_scope_digest: claim.test_policy_subject_scope_digest().unwrap(),
            payload_purpose: Some(openlife_core::llm::ProviderPayloadPurpose::AgentLoopStep),
            unfiltered_payload_digest: Some(digest_text("scheduled unfiltered payload")),
            context_manifest_digest: digest_text(claim.attempt_id()),
            prepared_envelope_digest: Some(digest_text("scheduled prepared envelope")),
            provider_config_generation: "test-scheduled-provider-generation".into(),
            network_policy_decision_digest: digest_text("scheduled network decision"),
            selected_context_refs: Vec::new(),
            included_context_categories: Vec::new(),
            declared_payload_categories: vec![
                openlife_core::llm::ProviderPayloadCategory::RuntimeCompiledMessages,
            ],
            policy_provenance_refs: Vec::new(),
            raw_life_model_included: false,
            raw_unbounded_memory_included: false,
        }
    }

    fn persist_test_provider_progress(
        store: &TaskStore,
        claim: &ScheduledTaskClaim,
        progress: ProviderInvocationProgress,
    ) -> anyhow::Result<()> {
        let admission = openlife_core::scheduler::issue_scheduled_provider_truth_test_admission(
            claim, &progress,
        )?;
        store.record_provider_truth(claim, admission)?;
        Ok(())
    }

    #[test]
    fn provider_failure_text_cannot_be_committed_as_scheduled_task_success() {
        let result = validate_scheduled_task_terminal(
            AgentRunStatus::Failed,
            true,
            "model_error",
            std::iter::empty(),
            "模型生成失败：this is prose, not success".into(),
        );

        assert_eq!(result.unwrap_err(), "scheduled_agent_loop_failed");
    }

    #[test]
    fn scheduled_completion_rejects_partial_and_failed_tool_terminals() {
        assert_eq!(
            validate_scheduled_task_terminal(
                AgentRunStatus::Completed,
                false,
                "max_steps_reached",
                std::iter::empty(),
                "partial".into(),
            )
            .unwrap_err(),
            "scheduled_agent_loop_incomplete"
        );
        assert_eq!(
            validate_scheduled_task_terminal(
                AgentRunStatus::Completed,
                false,
                "no_tools",
                ["failed"].into_iter(),
                "unproven".into(),
            )
            .unwrap_err(),
            "scheduled_tool_terminal_truth_rejected"
        );
        assert_eq!(
            validate_scheduled_task_terminal(
                AgentRunStatus::Completed,
                false,
                "no_tools",
                ["succeeded"].into_iter(),
                "confirmed output".into(),
            )
            .unwrap(),
            "confirmed output"
        );
    }

    #[test]
    fn typed_policy_context_binds_local_only_route_to_durable_decision_id() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = due_task();
        store.create_task_idempotent(&task).unwrap();
        let provider_grant = task.provider_grant.clone();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        let context =
            openlife_core::agent::RuntimePolicyContext::from_scheduled_claim(&claim).unwrap();

        assert_eq!(
            context.provider_authorization().data_route(),
            openlife_core::llm::ProviderDataRoute::LocalOnly
        );
        assert_eq!(
            context.provider_authorization().decision_id(),
            provider_grant.policy_decision_digest
        );
        assert!(context.policy_provenance_refs().iter().any(|reference| {
            reference.kind()
                == openlife_core::llm::ProviderPolicyProvenanceKind::ScheduledRouteDecision
        }));
    }

    #[test]
    fn provider_progress_is_persisted_by_request_and_attempt_without_bodies() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = due_task();
        store.create_task_idempotent(&task).unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        store.begin_claim_execution(&claim).unwrap();
        let started_at = chrono::Utc::now();
        persist_test_provider_progress(
            &store,
            &claim,
            ProviderInvocationProgress::Started {
                request_id: "scheduler-provider-request".into(),
                provider: "ollama".into(),
                model: "local-model".into(),
                started_at,
                policy_evidence: scheduled_provider_evidence(&claim),
            },
        )
        .unwrap();
        persist_test_provider_progress(
            &store,
            &claim,
            ProviderInvocationProgress::Completed(ProviderInvocationReceipt {
                request_id: "scheduler-provider-request".into(),
                provider: "ollama".into(),
                model: "local-model".into(),
                status: ProviderInvocationStatus::Completed,
                started_at,
                finished_at: started_at + chrono::Duration::milliseconds(5),
                error_digest: None,
                simulated: false,
                policy_evidence: Some(scheduled_provider_evidence(&claim)),
            }),
        )
        .unwrap();

        let receipts = store
            .provider_receipts_for_attempt(claim.attempt_id())
            .unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].request_id, "scheduler-provider-request");
        assert_eq!(receipts[0].attempt_id, claim.attempt_id());
        assert_eq!(
            receipts[0].provider_grant_id,
            claim.provider_grant().grant_id
        );
        assert_eq!(receipts[0].status, "completed");
        assert!(receipts[0]
            .prepared_request_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("sha256:")));
        let source = include_str!("scheduler_runner.rs");
        let removed_start = ["record_provider_", "started("].concat();
        let removed_terminal = ["record_provider_", "terminal("].concat();
        assert!(!source.contains(&removed_start));
        assert!(!source.contains(&removed_terminal));
        assert!(source.contains("take_for_progress(&progress)"));
    }

    #[test]
    fn local_abort_without_a_bound_in_flight_provider_cannot_fabricate_remote_unknown() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = due_task();
        store.create_task_idempotent(&task).unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        store.begin_claim_execution(&claim).unwrap();
        let observer_state = SchedulerReceiptObserverState::default();

        assert_eq!(
            persist_provider_local_abort_truth(
                &store,
                &claim,
                &observer_state,
                ScheduledProviderLocalAbortCause::ExecutionTimeout,
            )
            .unwrap(),
            0
        );
        assert!(store
            .provider_receipts_for_attempt(claim.attempt_id())
            .unwrap()
            .is_empty());
        assert_eq!(
            store.settle_claim_after_timeout(&claim).unwrap(),
            ScheduledClaimSettlement::ReclaimedBeforeDispatch
        );
    }

    #[tokio::test]
    async fn scheduler_preflight_is_not_dispatch_and_adapter_start_is_durable() {
        let preflight_store = Arc::new(TaskStore::new_in_memory().unwrap());
        let task = due_task();
        preflight_store.create_task_idempotent(&task).unwrap();
        let preflight_claim = preflight_store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        preflight_store
            .begin_claim_execution(&preflight_claim)
            .unwrap();
        let preflight_claim = Arc::new(preflight_claim);
        let preflight_state = Arc::new(SchedulerReceiptObserverState::default());
        let preflight_observer = SchedulerToolDispatchObserver {
            store: Arc::clone(&preflight_store),
            claim: Arc::clone(&preflight_claim),
            observer_state: Arc::clone(&preflight_state),
            persistence_coordinator: isolated_persistence_coordinator(),
        };
        let preflight_registration =
            openlife_core::agent::ToolExecutionReceiptRegistration::test_inflight_network_mutation(
                Some("scheduler-tool-run".into()),
                Some("manifest.scheduler-write".into()),
                "scheduled write".into(),
            );
        let preflight_receipt = preflight_registration.snapshot();
        let attempt = ToolDispatchAttempt {
            receipt_id: preflight_receipt.receipt_id.clone(),
            manifest_id: preflight_receipt.manifest_id.clone().unwrap(),
            tool_name: "calendar.write".into(),
            manifest_contract_digest: digest_text("scheduler manifest"),
            input_hash: digest_text("bounded input"),
            input_length_bytes: 13,
            source_run_id: preflight_receipt.source_run_id.clone(),
            request_digest: preflight_receipt.request_digest.clone(),
            action_effect: preflight_receipt.action_effect,
            idempotency_contract: preflight_receipt.idempotency_contract,
            process_risk: openlife_core::agent::action_executor::ToolDispatchProcessRisk::MayOutliveLocalProcess,
            effect_may_survive_local_process: true,
        };
        preflight_observer.before_dispatch(&attempt).await.unwrap();
        assert!(!preflight_state.adapter_edge_crossed());
        assert_eq!(
            preflight_store
                .settle_claim_after_error(
                    &preflight_claim,
                    "adapter_preflight_failed",
                    Some(&digest_text("preflight rejected")),
                )
                .unwrap(),
            ScheduledClaimSettlement::ReclaimedBeforeDispatch,
            "preflight cannot fabricate a wire-dispatch fact"
        );

        let started_store = Arc::new(TaskStore::new_in_memory().unwrap());
        started_store.create_task_idempotent(&task).unwrap();
        let started_claim = started_store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        started_store.begin_claim_execution(&started_claim).unwrap();
        let started_claim = Arc::new(started_claim);
        let started_state = Arc::new(SchedulerReceiptObserverState::default());
        let started_observer = SchedulerToolDispatchObserver {
            store: Arc::clone(&started_store),
            claim: Arc::clone(&started_claim),
            observer_state: started_state,
            persistence_coordinator: isolated_persistence_coordinator(),
        };
        started_observer.before_dispatch(&attempt).await.unwrap();
        started_observer
            .after_dispatch(&preflight_receipt)
            .await
            .unwrap();
        assert_eq!(
            started_store
                .settle_claim_after_error(
                    &started_claim,
                    "local_wait_lost",
                    Some(&digest_text("response not observed")),
                )
                .unwrap(),
            ScheduledClaimSettlement::UnknownRequiresReconciliation,
            "a real adapter-owned dispatch must survive as unknown"
        );
    }

    #[tokio::test]
    async fn d067_degraded_audit_store_blocks_next_scheduler_effect_in_same_loop() {
        let store = Arc::new(TaskStore::new_in_memory().unwrap());
        let task = due_task();
        store.create_task_idempotent(&task).unwrap();
        let claim = Arc::new(
            store
                .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
                .unwrap()
                .unwrap(),
        );
        store.begin_claim_execution(&claim).unwrap();
        let persistence = healthy_release_persistence_coordinator();
        let observer_state = Arc::new(SchedulerReceiptObserverState::default());
        let observer = SchedulerToolDispatchObserver {
            store,
            claim,
            observer_state: Arc::clone(&observer_state),
            persistence_coordinator: Arc::clone(&persistence),
        };
        let registration =
            openlife_core::agent::ToolExecutionReceiptRegistration::test_inflight_network_mutation(
                Some("d067-scheduler-run".into()),
                Some("d067.scheduler.effect".into()),
                "d067 scheduler effect".into(),
            );
        let receipt = registration.snapshot();
        let attempt = ToolDispatchAttempt {
            receipt_id: receipt.receipt_id,
            manifest_id: receipt.manifest_id.unwrap(),
            tool_name: "calendar.write".into(),
            manifest_contract_digest: digest_text("d067 scheduler manifest"),
            input_hash: digest_text("d067 scheduler input"),
            input_length_bytes: 21,
            source_run_id: receipt.source_run_id,
            request_digest: receipt.request_digest,
            action_effect: receipt.action_effect,
            idempotency_contract: receipt.idempotency_contract,
            process_risk:
                openlife_core::agent::action_executor::ToolDispatchProcessRisk::MayOutliveLocalProcess,
            effect_may_survive_local_process: true,
        };

        persistence.register_unavailable(
            "McpAuditStore",
            "runtime_audit_commit_failed",
            "d067 injected runtime audit failure",
        );
        let error = observer
            .before_dispatch(&attempt)
            .await
            .expect_err("the next tool effect must fail before dispatch");
        assert!(error.to_string().contains("persistence_effects_blocked"));
        assert!(observer_state.prepared_tools.lock().unwrap().is_empty());
        assert!(openlife_core::agent::CanonicalWriteAdmission::acquire(
            &observer,
            openlife_core::agent::CanonicalWriteAdmissionRequest {
                domain: "proposal".into(),
                object_ref: "proposal:d067".into(),
            },
        )
        .is_err());
    }

    #[tokio::test]
    async fn scheduler_memory_read_uses_gateway_prepared_and_real_local_adapter_edge() {
        let store = Arc::new(TaskStore::new_in_memory().unwrap());
        let task = due_task();
        store.create_task_idempotent(&task).unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        store.begin_claim_execution(&claim).unwrap();
        let claim = Arc::new(claim);
        let observer_state = Arc::new(SchedulerReceiptObserverState::default());
        let observer = SchedulerToolDispatchObserver {
            store: Arc::clone(&store),
            claim: Arc::clone(&claim),
            observer_state: Arc::clone(&observer_state),
            persistence_coordinator: isolated_persistence_coordinator(),
        };

        let registry = openlife_core::mcp::McpRegistry::new();
        let permission_store =
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = openlife_core::mcp_audit::McpAuditStore::new(audit_file.path());
        let privacy_engine = openlife_core::privacy::PrivacyEngine::new();
        let memory_store = openlife_core::memory::MemoryStore::new_in_memory().unwrap();
        let memory_lifecycle_store =
            openlife_core::agent::MemoryLifecycleStore::new_in_memory().unwrap();
        let memory_lifecycle_reader = memory_lifecycle_store.retrieval_reader();
        let owner_store = openlife_core::agent::AgentRunStore::new_in_memory().unwrap();
        let owner_run =
            openlife_core::agent::AgentRun::new_tool_execution_run("scheduler-memory-read");
        let owner_run_id = owner_run.id.clone();
        owner_store.create_run(&owner_run).unwrap();
        memory_store
            .save_message(
                "scheduler-memory-session",
                &ChatMessage {
                    role: "user".into(),
                    content: "A bounded scheduler memory read fixture.".into(),
                },
            )
            .unwrap();
        let ctx = openlife_core::agent::ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_memory_store(&memory_store)
        .with_memory_lifecycle_retrieval_reader(&memory_lifecycle_reader)
        .with_agent_run_store(&owner_store)
        .with_tool_dispatch_observer(&observer)
        .with_tool_started_transition_observer(&observer);
        let result = openlife_core::agent::ToolGateway::from_executor_config(
            openlife_core::agent::ActionExecutorConfig::default(),
        )
        .execute(
            openlife_core::agent::AgentActionRequest {
                action_type: "memory_search".into(),
                target: "memory.search".into(),
                input: serde_json::json!({
                    "query": "bounded scheduler",
                    "session_id": "scheduler-memory-session",
                    "limit": 3,
                }),
                source_run_id: Some(owner_run_id),
                step_index: 0,
            },
            &ctx,
        )
        .await
        .unwrap();

        assert_eq!(
            result.status,
            openlife_core::agent::ActionExecutionStatus::Succeeded,
            "unexpected scheduler memory-read blocker: {:?}",
            result.stop_reason
        );
        assert_eq!(
            result.execution_receipt.dispatch_kind,
            openlife_core::agent::ToolDispatchKind::Local
        );
        assert_eq!(
            result.execution_receipt.transport_status,
            ToolTransportStatus::ResponseObserved
        );
        assert!(observer_state.adapter_edge_crossed());
        assert!(observer_state
            .prepared_tools
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty());
        assert_eq!(
            store
                .settle_claim_after_error(
                    &claim,
                    "downstream_projection_failed",
                    Some(&digest_text("downstream failure")),
                )
                .unwrap(),
            ScheduledClaimSettlement::UnknownRequiresReconciliation,
            "the real MemoryStore adapter start must be durable before its read"
        );
        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn after_dispatch_identity_failure_is_quarantined_and_never_retried() {
        let store = Arc::new(TaskStore::new_in_memory().unwrap());
        let task = due_task();
        store.create_task_idempotent(&task).unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        store.begin_claim_execution(&claim).unwrap();
        let claim = Arc::new(claim);
        let observer_state = Arc::new(SchedulerReceiptObserverState::default());
        let observer = SchedulerToolDispatchObserver {
            store: Arc::clone(&store),
            claim: Arc::clone(&claim),
            observer_state: Arc::clone(&observer_state),
            persistence_coordinator: isolated_persistence_coordinator(),
        };
        let registration =
            openlife_core::agent::ToolExecutionReceiptRegistration::test_inflight_network_mutation(
                Some("scheduler-identity-run".into()),
                Some("manifest.scheduler-identity".into()),
                "scheduled identity-bound write".into(),
            );
        let mut receipt = registration.snapshot();
        let attempt = ToolDispatchAttempt {
            receipt_id: receipt.receipt_id.clone(),
            manifest_id: receipt.manifest_id.clone().unwrap(),
            tool_name: "calendar.write".into(),
            manifest_contract_digest: digest_text("scheduler identity manifest"),
            input_hash: digest_text("bounded identity input"),
            input_length_bytes: 22,
            source_run_id: receipt.source_run_id.clone(),
            request_digest: receipt.request_digest.clone(),
            action_effect: receipt.action_effect,
            idempotency_contract: receipt.idempotency_contract,
            process_risk: openlife_core::agent::action_executor::ToolDispatchProcessRisk::MayOutliveLocalProcess,
            effect_may_survive_local_process: true,
        };
        observer.before_dispatch(&attempt).await.unwrap();
        receipt.manifest_id = Some("manifest.drifted-after-edge".into());

        let error = observer.after_dispatch(&receipt).await.unwrap_err();
        assert!(error
            .to_string()
            .contains("scheduler_tool_dispatch_started_truth_rejected"));
        assert!(observer_state.adapter_edge_crossed());
        assert!(observer_state.persistence_failed());
        let failure = ScheduledExecutionFailure::from_error(
            "scheduled_tool_adapter_edge_truth_rejected",
            error,
        );
        assert_eq!(
            settle_claim_after_execution_error(&store, &claim, &observer_state, &failure).unwrap(),
            ScheduledClaimSettlement::UnknownRequiresReconciliation
        );
        assert!(
            store
                .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
                .unwrap()
                .is_none(),
            "an adapter-edge identity failure must not become an automatic retry"
        );
    }

    #[test]
    fn unknown_attempt_cannot_retry_until_explicit_reconciliation() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = due_task();
        store.create_task_idempotent(&task).unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        store.begin_claim_execution(&claim).unwrap();
        persist_test_provider_progress(
            &store,
            &claim,
            ProviderInvocationProgress::Started {
                request_id: "unknown-provider-request".into(),
                provider: "ollama".into(),
                model: "local-model".into(),
                started_at: chrono::Utc::now(),
                policy_evidence: scheduled_provider_evidence(&claim),
            },
        )
        .unwrap();
        assert_eq!(
            store.settle_claim_after_timeout(&claim).unwrap(),
            ScheduledClaimSettlement::UnknownRequiresReconciliation
        );
        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_none());
        let admission = store
            .issue_scheduled_reconciliation_test_admission(
                &claim.task().id,
                claim.attempt_id(),
                ScheduledReconciliationTestResolution::RetrySafe,
            )
            .unwrap();
        assert!(store.reconcile_unknown_attempt(admission).unwrap());
        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_some());
    }

    #[test]
    fn agent_run_typed_tool_receipt_resolves_scheduler_dispatch_truth() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = due_task();
        store.create_task_idempotent(&task).unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        store.begin_claim_execution(&claim).unwrap();
        let receipt = ToolExecutionReceipt::test_observed_mcp_read(
            Some("run-tool-projection".into()),
            Some("manifest-projection".into()),
            "tool request".into(),
        );
        let mut started_receipt = receipt.clone();
        started_receipt.transport_status = ToolTransportStatus::Dispatched;
        started_receipt.response_observed_at = None;
        started_receipt.finished_at = None;
        started_receipt.execution_outcome = openlife_core::agent::ToolExecutionOutcome::NotObserved;
        let attempt = ToolDispatchAttempt {
            receipt_id: started_receipt.receipt_id.clone(),
            manifest_id: started_receipt.manifest_id.clone().unwrap(),
            tool_name: "mcp.read".into(),
            manifest_contract_digest: digest_text("projection manifest contract"),
            input_hash: digest_text("projection bounded input"),
            input_length_bytes: 24,
            source_run_id: started_receipt.source_run_id.clone(),
            request_digest: started_receipt.request_digest.clone(),
            action_effect: started_receipt.action_effect,
            idempotency_contract: started_receipt.idempotency_contract,
            process_risk: openlife_core::agent::action_executor::ToolDispatchProcessRisk::MayOutliveLocalProcess,
            effect_may_survive_local_process: false,
        };
        store
            .record_tool_dispatch_started(&claim, &attempt, &started_receipt)
            .unwrap();
        let started_at = receipt.started_at;
        let observed_at = receipt
            .response_observed_at
            .expect("observed MCP read fixture has a response boundary");
        let mut run = AgentRun::new_chat_run("scheduled-test", "transient input");
        run.id = "run-tool-projection".into();
        run.actions.push(AgentAction {
            id: "action-tool-projection".into(),
            action_type: "mcp_tool".into(),
            target: Some("mcp.read".into()),
            input: serde_json::json!({}),
            output: Some(serde_json::json!({ "toolExecutionReceipt": receipt })),
            status: "succeeded".into(),
            permission_decision: None,
            started_at: Some(started_at),
            finished_at: Some(observed_at),
            error: None,
            timestamp: started_at,
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: None,
        });

        project_tool_terminal_receipts(&store, &claim, &run).unwrap();
        assert_eq!(
            store
                .settle_claim_after_error(
                    &claim,
                    "post_tool_local_failure",
                    Some(&digest_text("known local failure")),
                )
                .unwrap(),
            ScheduledClaimSettlement::FailedAfterObservedTerminal
        );
        assert!(store.list_tasks(Some("unknown")).unwrap().is_empty());
    }
}
