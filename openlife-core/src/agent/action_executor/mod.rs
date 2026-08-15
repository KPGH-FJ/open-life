pub mod core_os_tools;
pub mod declarative_stubs;
pub mod execution_tools;
pub mod helpers;
pub mod tool_executor;

// Re-export commonly used helpers
pub use helpers::{filesystem_access_error, is_path_in_safe_paths};

use crate::agent::types::{AgentAction, AgentObservation, ContentReceipt};
use crate::agent::GovernorDecisionReport;
use crate::agent::{
    CanonicalWriteAdmission, DurableWriteRequest, ReviewWorkflow, ReviewWorkflowOutcome,
};
use crate::mcp::McpRegistry;
use crate::mcp_audit::McpAuditStore;
use crate::privacy::PrivacyEngine;
use crate::tool_execution_receipt::{
    ToolActionEffect, ToolExecutionReceipt, ToolExecutionReceiptTracker,
};
use crate::tool_permissions::ToolPermissionStore;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Configuration for action execution.
#[derive(Debug, Clone)]
pub struct ActionExecutorConfig {
    pub allow_writes: bool,
    pub allow_cloud: bool,
    pub timeout_seconds: u64,
    /// Whether to consume `allow_once` policies during permission check.
    /// Default is `true`. Set to `false` for replay paths to avoid
    /// consuming one-time permissions.
    pub consume_allow_once: bool,
    /// Immutable per-execution projection of the canonical product config.
    /// Keeping it on ToolGateway prevents process-global search state from
    /// drifting away from the AppState generation that authorized the turn.
    pub search_provider: helpers::SearchProviderConfig,
}

impl Default for ActionExecutorConfig {
    fn default() -> Self {
        Self {
            allow_writes: true,
            allow_cloud: true,
            timeout_seconds: 120,
            consume_allow_once: true,
            search_provider: helpers::SearchProviderConfig::default(),
        }
    }
}

/// Request to execute a single action.
#[derive(Debug, Clone)]
pub struct AgentActionRequest {
    pub action_type: String,
    pub target: String,
    pub input: Value,
    pub source_run_id: Option<String>,
    pub step_index: u32,
}

/// Result of executing an action. Adapter body admission is deliberately not
/// inspectable or movable outside this module.
///
/// ```compile_fail
/// use openlife_core::agent::ActionExecutionResult;
///
/// fn transplant(result: ActionExecutionResult) {
///     let _admission = result.observed_body_admission;
/// }
/// ```
pub struct ActionExecutionResult {
    pub action: AgentAction,
    pub observation: AgentObservation,
    pub status: ActionExecutionStatus,
    pub stop_reason: Option<String>,
    pub governance_report: Option<GovernorDecisionReport>,
    pub execution_receipt: ToolExecutionReceipt,
    /// One-shot adapter body evidence. It never crosses serde/IPC and is
    /// consumed at the outermost semantic boundary before the result returns.
    observed_body_admission: Option<tool_executor::ObservedToolBodyAdmission>,
}

impl ActionExecutionResult {
    pub(crate) fn without_observed_body(
        action: AgentAction,
        observation: AgentObservation,
        status: ActionExecutionStatus,
        stop_reason: Option<String>,
        governance_report: Option<GovernorDecisionReport>,
        execution_receipt: ToolExecutionReceipt,
    ) -> Self {
        Self {
            action,
            observation,
            status,
            stop_reason,
            governance_report,
            execution_receipt,
            observed_body_admission: None,
        }
    }

    fn finalize_bound_content_receipt(&mut self, ctx: &ActionExecutionContext<'_>) -> Result<()> {
        let Some(admission) = self.observed_body_admission.take() else {
            return Ok(());
        };
        let Some(issuer) = ctx.bound_content_receipt_issuer else {
            anyhow::bail!("bound_content_receipt_issuer_unavailable");
        };
        let receipt =
            match issuer.issue_bound_content_receipt(admission, &self.action, &self.observation) {
                Ok(receipt) => receipt,
                Err(error) => {
                    // Preserve the concrete canonical owner failure before the
                    // gateway converts it into a typed tool result.
                    ctx.observe_durable_store_failure("CanonicalTaskRuntimeStore", &error);
                    return Err(error);
                }
            };
        self.action
            .tool_trace
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("bound_content_receipt_action_trace_missing"))?
            .output_receipt = Some(receipt);
        self.observation.tool_trace = None;
        Ok(())
    }
}

impl std::fmt::Debug for ActionExecutionResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ActionExecutionResult")
            .field("action_id", &self.action.id)
            .field("observation_id", &self.observation.id)
            .field("status", &self.status)
            .field("stop_reason", &self.stop_reason)
            .field(
                "governance_report_present",
                &self.governance_report.is_some(),
            )
            .field("execution_receipt_id", &self.execution_receipt.receipt_id)
            .field(
                "observed_body_admission_present",
                &self.observed_body_admission.is_some(),
            )
            .finish()
    }
}

/// Opaque result between the concrete adapter boundary and ToolGateway's
/// final semantic augmentation. ToolGateway can inspect only terminal status;
/// the action/observation graph and one-shot body admission remain sealed
/// until the gateway-owned authority consumes this wrapper and signs the final
/// graph.
pub(crate) struct PendingToolGatewayActionExecution {
    result: ActionExecutionResult,
}

impl PendingToolGatewayActionExecution {
    pub(crate) fn from_terminal_without_admission(
        result: ActionExecutionResult,
        _authority: &crate::agent::tool_gateway::ToolGatewayFinalizationAuthority,
    ) -> Result<Self> {
        if result.observed_body_admission.is_some() {
            anyhow::bail!("tool_gateway_terminal_result_contains_body_admission");
        }
        Ok(Self { result })
    }

    pub(crate) fn status(&self) -> &ActionExecutionStatus {
        &self.result.status
    }

    pub(crate) fn replace_with_terminal(
        &mut self,
        result: ActionExecutionResult,
        authority: &crate::agent::tool_gateway::ToolGatewayFinalizationAuthority,
    ) -> Result<()> {
        *self = Self::from_terminal_without_admission(result, authority)?;
        Ok(())
    }

    pub(crate) fn finalize_gateway_semantics(
        mut self,
        ctx: &ActionExecutionContext<'_>,
        receipt_tracker: &ToolExecutionReceiptTracker,
        gateway_contract_evidence: Value,
        _authority: crate::agent::tool_gateway::ToolGatewayFinalizationAuthority,
    ) -> Result<ActionExecutionResult> {
        receipt_tracker
            .bind_action_identity(
                &self.result.action.id,
                &self.result.action.action_type,
                self.result.action.target.as_deref(),
                &self.result.action.input,
            )
            .map_err(|reason| {
                anyhow::anyhow!("tool_gateway_action_receipt_binding_failed:{reason}")
            })?;
        self.result.execution_receipt = receipt_tracker.snapshot();
        if let Some(output) = self.result.action.output.as_mut() {
            if let Some(object) = output.as_object_mut() {
                object.insert("toolGateway".into(), gateway_contract_evidence.clone());
            }
        }
        if let Some(structured) = self.result.observation.structured_result.as_mut() {
            if let Some(object) = structured.as_object_mut() {
                object.insert("toolGateway".into(), gateway_contract_evidence);
            }
        } else {
            self.result.observation.structured_result = Some(serde_json::json!({
                "toolGateway": gateway_contract_evidence,
            }));
        }

        self.result.action.runtime_execution_receipt = Some(self.result.execution_receipt.clone());
        let receipt = serde_json::to_value(&self.result.execution_receipt).unwrap_or_else(|_| {
            serde_json::json!({
                "receiptId": self.result.execution_receipt.receipt_id,
                "transportStatus": crate::tool_execution_receipt::ToolTransportStatus::NotAttempted,
                "effectStatus": crate::tool_execution_receipt::ToolEffectStatus::Unknown,
            })
        });
        if let Some(output) = self.result.action.output.as_mut() {
            if let Some(object) = output.as_object_mut() {
                object.insert("toolExecutionReceipt".into(), receipt.clone());
            }
        }
        if let Some(structured) = self.result.observation.structured_result.as_mut() {
            if let Some(object) = structured.as_object_mut() {
                object.insert("toolExecutionReceipt".into(), receipt);
            }
        } else {
            self.result.observation.structured_result = Some(serde_json::json!({
                "toolExecutionReceipt": receipt,
            }));
        }
        self.result.finalize_bound_content_receipt(ctx)?;
        Ok(self.result)
    }
}

/// Narrow authority for turning one adapter-issued, transient body admission
/// into durable receipt metadata. The concrete owner may persist only
/// minimized issuance facts; raw adapter bodies must never enter its ledger.
pub(crate) trait BoundContentReceiptIssuer: Send + Sync {
    fn issue_bound_content_receipt(
        &self,
        admission: tool_executor::ObservedToolBodyAdmission,
        action: &AgentAction,
        observation: &AgentObservation,
    ) -> Result<ContentReceipt>;
}

/// Status of action execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionExecutionStatus {
    Succeeded,
    Failed,
    Blocked,
    NeedsConfirmation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDispatchAttempt {
    pub receipt_id: String,
    pub manifest_id: String,
    pub tool_name: String,
    pub manifest_contract_digest: String,
    pub input_hash: String,
    pub input_length_bytes: u64,
    pub source_run_id: Option<String>,
    pub request_digest: String,
    pub action_effect: ToolActionEffect,
    pub idempotency_contract: crate::tool_manifest::ToolIdempotencyContract,
    pub process_risk: ToolDispatchProcessRisk,
    pub effect_may_survive_local_process: bool,
}

/// Conservative, manifest-derived process-lifetime contract captured before
/// an adapter is entered. This is not a claim that dispatch happened. It lets
/// restart reconciliation distinguish a process-bound local read from a
/// network/MCP/plugin attempt whose peer may have observed the request
/// before the local process died.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolDispatchProcessRisk {
    ProcessBound,
    MayOutliveLocalProcess,
}

impl ToolDispatchProcessRisk {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProcessBound => "process_bound",
            Self::MayOutliveLocalProcess => "may_outlive_local_process",
        }
    }

    pub fn may_outlive_local_process(self) -> bool {
        self == Self::MayOutliveLocalProcess
    }
}

pub fn tool_dispatch_process_risk_for_manifest(
    manifest: &crate::tool_manifest::ToolManifest,
) -> ToolDispatchProcessRisk {
    let contract_declares_remote_or_external = matches!(
        &manifest.source,
        crate::tool_manifest::ToolSource::Mcp { .. }
            | crate::tool_manifest::ToolSource::Plugin { .. }
    ) || matches!(
        manifest.action_type.as_str(),
        "network" | "external_side_effect"
    ) || manifest
        .capabilities
        .iter()
        .any(|capability| matches!(capability.as_str(), "network" | "external_side_effect"));
    if contract_declares_remote_or_external {
        ToolDispatchProcessRisk::MayOutliveLocalProcess
    } else {
        ToolDispatchProcessRisk::ProcessBound
    }
}

fn effect_may_survive_local_process(action_effect: ToolActionEffect) -> bool {
    matches!(
        action_effect,
        ToolActionEffect::LocalMutation
            | ToolActionEffect::ExternalMutation
            | ToolActionEffect::ProposalOnly
            | ToolActionEffect::Unknown
    )
}

#[async_trait]
pub trait ToolDispatchObserver: Send + Sync {
    async fn before_dispatch(&self, attempt: &ToolDispatchAttempt) -> Result<()>;

    async fn before_registry_dispatch(
        &self,
        attempt: &ToolDispatchAttempt,
        _binding: &crate::mcp::McpRegistryDispatchBinding,
    ) -> Result<()> {
        self.before_dispatch(attempt).await
    }
}

#[async_trait]
pub trait ToolStartedTransitionObserver: Send + Sync {
    /// Called immediately after a concrete adapter has crossed its dispatch
    /// boundary and before it continues to await/consume the response.
    async fn after_dispatch(&self, receipt: &ToolExecutionReceipt) -> Result<()>;
}

/// Canonical execution owner for a remote, non-idempotent tool attempt. The
/// owner receives only live ToolGateway receipts; implementations must reject
/// serde-shaped receipts and persist only minimized identity/state facts.
/// Transition methods are synchronous so no store guard can cross an external
/// network await.
pub trait DurableToolExecutionOwner: Send + Sync {
    fn prepare(&self, receipt: &ToolExecutionReceipt) -> Result<()>;

    /// CAS the durable attempt fence before the network client calls `send`.
    /// Returning an error must prevent the send.
    fn before_dispatch_attempt(
        &self,
        receipt: &ToolExecutionReceipt,
        dispatch_kind: crate::tool_execution_receipt::ToolDispatchKind,
    ) -> Result<()>;

    fn response_observed(&self, receipt: &ToolExecutionReceipt) -> Result<()>;

    /// Commit a mechanically valid terminal receipt. A failure here must
    /// leave the previous durable state intact for startup reconciliation.
    fn terminal(&self, result: &ActionExecutionResult) -> Result<()>;
}

/// Receives a metadata-only fact after the mandatory minimized audit insert
/// fails. Product runtimes use this to degrade their existing persistence
/// coordinator; the observer owns no tool payload and cannot rewrite the
/// execution receipt's transport or effect truth.
pub trait ToolAuditPersistenceObserver: Send + Sync {
    fn audit_persistence_failed(&self, receipt: &ToolExecutionReceipt);
}

/// Metadata-only bridge from synchronous canonical-store failures inside Core
/// execution back to the product's existing persistence-health authority.
/// Implementations receive only the store kind and raw store error; tool
/// arguments, observed bodies, and model/user payloads must never be copied
/// into this callback.
pub trait DurableStoreFailureObserver: Send + Sync {
    fn durable_store_failed(&self, store_kind: &'static str, raw_error: &str);
}

/// Project the immutable `tool.started` transition exactly once for a receipt.
///
/// A concrete idempotent adapter may perform more than one wire attempt. An
/// ambiguous attempt does not authorize `tool.started`; the first adapter-
/// observed edge claims this runtime-only gate even when it follows a failed
/// attempt. Later retries cannot emit a second immutable start.
pub(crate) async fn observe_first_tool_started_transition(
    receipt_tracker: &ToolExecutionReceiptTracker,
    observer: Option<&dyn ToolStartedTransitionObserver>,
) -> Result<()> {
    let Some(observer) = observer else {
        return Ok(());
    };
    if receipt_tracker.claim_first_concrete_dispatch_observation() {
        let receipt = receipt_tracker.snapshot();
        observer.after_dispatch(&receipt).await?;
    }
    Ok(())
}

/// Opaque, one-use proof that the execution-owner dispatch fence succeeded for
/// this receipt. Concrete adapters must consume this value at their dispatch
/// edge; a prepared receipt alone cannot be promoted into dispatch truth.
#[must_use = "dispatch admission must be consumed by exactly one concrete adapter edge"]
pub(crate) struct ToolDispatchAdmission<'a> {
    receipt_tracker: ToolExecutionReceiptTracker,
    started_observer: Option<&'a dyn ToolStartedTransitionObserver>,
}

impl<'a> ToolDispatchAdmission<'a> {
    fn new(
        receipt_tracker: ToolExecutionReceiptTracker,
        started_observer: Option<&'a dyn ToolStartedTransitionObserver>,
    ) -> Self {
        Self {
            receipt_tracker,
            started_observer,
        }
    }

    pub(crate) async fn observe_local(self) -> Result<()> {
        self.receipt_tracker.mark_local_dispatched();
        observe_first_tool_started_transition(&self.receipt_tracker, self.started_observer).await?;
        Ok(())
    }

    pub(crate) async fn observe_simulated(self) -> Result<()> {
        self.receipt_tracker.mark_simulated_dispatched();
        observe_first_tool_started_transition(&self.receipt_tracker, self.started_observer).await?;
        Ok(())
    }

    pub(crate) fn into_remote_parts(
        self,
    ) -> (
        ToolExecutionReceiptTracker,
        Option<&'a dyn ToolStartedTransitionObserver>,
    ) {
        (self.receipt_tracker, self.started_observer)
    }
}

/// Immutable read snapshot of the StateStore-owned product state used by a
/// single tool execution. It is a transport value, not another persistence
/// owner, and can always be rebuilt from StateStore.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalStateSnapshot {
    pub daily_tasks: Vec<crate::state_store::StateAsset>,
    pub observations: Vec<crate::state_store::StateObservation>,
}

/// Dependencies required for action execution.
///
/// Essential fields (registry, permission_store, audit_store, privacy_engine,
/// safe_paths) are set via the constructor. Optional fields are set via builder
/// methods.
pub struct ActionExecutionContext<'a> {
    pub registry: &'a McpRegistry,
    pub permission_store: &'a ToolPermissionStore,
    pub audit_store: &'a McpAuditStore,
    pub privacy_engine: &'a PrivacyEngine,
    pub safe_paths: &'a [String],
    pub life_model: Option<&'a crate::life_model::LifeModel>,
    /// Canonical short-lived state owned by StateStore. This snapshot is
    /// deliberately separate from the LifeModel YAML compatibility view so
    /// `goal.read` and `state.read` cannot revive the retired dual authority.
    pub canonical_state: Option<&'a CanonicalStateSnapshot>,
    pub memory_store: Option<&'a crate::memory::MemoryStore>,
    pub memory_lifecycle_retrieval_reader: Option<&'a crate::agent::MemoryLifecycleRetrievalReader>,
    /// Canonical owner for resources explicitly bound to the current task.
    /// `document.read` requires this store plus an exact message identity; it
    /// never scans arbitrary filesystem paths.
    pub resource_store: Option<&'a crate::resource::ResourceStore>,
    pub proposal_store: Option<&'a crate::agent::ProposalStore>,
    pub(crate) bound_content_receipt_issuer: Option<&'a dyn BoundContentReceiptIssuer>,
    pub network_policy: Option<&'a crate::config::NetworkPolicy>,
    pub web_search_fixture_output: Option<&'a str>,
    /// Evaluated PolicyStore result for the current task. This is intentionally
    /// a single action-policy fact rather than an HS/personalization envelope.
    pub external_write_requires_proposal: bool,
    pub tool_dispatch_observer: Option<&'a dyn ToolDispatchObserver>,
    pub tool_started_transition_observer: Option<&'a dyn ToolStartedTransitionObserver>,
    pub tool_audit_persistence_observer: Option<&'a dyn ToolAuditPersistenceObserver>,
    pub durable_store_failure_observer: Option<&'a dyn DurableStoreFailureObserver>,
    /// Execution-owner authority that linearizes canonical mutations against
    /// cancellation. Proposal writes fail closed when this authority is absent.
    pub canonical_write_admission: Option<&'a dyn CanonicalWriteAdmission>,
    /// Exact one-shot authorization for a reviewed action/input binding. This
    /// is intentionally separate from reusable manifest-level permissions.
    pub action_bound_tool_permission:
        Option<&'a crate::tool_permissions::ActionBoundToolPermissionAuthorization>,
    /// ICS calendar file paths for calendar.read tool
    pub calendar_ics_paths: &'a [String],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryRetrievalAuthorityError {
    LifecycleReaderUnavailable,
    LifecycleStoreQueryFailed(String),
}

impl MemoryRetrievalAuthorityError {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::LifecycleReaderUnavailable => "memory_lifecycle_reader_unavailable",
            Self::LifecycleStoreQueryFailed(_) => "memory_lifecycle_query_failed",
        }
    }
}

impl std::fmt::Display for MemoryRetrievalAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LifecycleReaderUnavailable => {
                formatter.write_str("canonical MemoryLifecycle retrieval reader unavailable")
            }
            Self::LifecycleStoreQueryFailed(error) => write!(
                formatter,
                "canonical MemoryLifecycle retrieval query failed: {error}"
            ),
        }
    }
}

impl std::error::Error for MemoryRetrievalAuthorityError {}

impl<'a> ActionExecutionContext<'a> {
    /// Create a context with the essential dependencies.
    /// Optional fields default to None / empty.
    pub fn new(
        registry: &'a McpRegistry,
        permission_store: &'a ToolPermissionStore,
        audit_store: &'a McpAuditStore,
        privacy_engine: &'a PrivacyEngine,
        safe_paths: &'a [String],
    ) -> Self {
        Self {
            registry,
            permission_store,
            audit_store,
            privacy_engine,
            safe_paths,
            life_model: None,
            canonical_state: None,
            memory_store: None,
            memory_lifecycle_retrieval_reader: None,
            resource_store: None,
            proposal_store: None,
            bound_content_receipt_issuer: None,
            network_policy: None,
            web_search_fixture_output: None,
            external_write_requires_proposal: false,
            tool_dispatch_observer: None,
            tool_started_transition_observer: None,
            tool_audit_persistence_observer: None,
            durable_store_failure_observer: None,
            canonical_write_admission: None,
            action_bound_tool_permission: None,
            calendar_ics_paths: &[],
        }
    }

    pub fn with_life_model(mut self, life_model: &'a crate::life_model::LifeModel) -> Self {
        self.life_model = Some(life_model);
        self
    }

    pub fn with_canonical_state(mut self, state: &'a CanonicalStateSnapshot) -> Self {
        self.canonical_state = Some(state);
        self
    }

    pub fn with_memory_store(mut self, memory_store: &'a crate::memory::MemoryStore) -> Self {
        self.memory_store = Some(memory_store);
        self
    }

    pub fn with_memory_lifecycle_retrieval_reader(
        mut self,
        memory_lifecycle_retrieval_reader: &'a crate::agent::MemoryLifecycleRetrievalReader,
    ) -> Self {
        self.memory_lifecycle_retrieval_reader = Some(memory_lifecycle_retrieval_reader);
        self
    }

    pub fn with_resource_store(
        mut self,
        resource_store: &'a crate::resource::ResourceStore,
    ) -> Self {
        self.resource_store = Some(resource_store);
        self
    }

    /// Fail-closed join between the replaceable MemoryStore projection and
    /// canonical lifecycle retrieval authority. Non-lifecycle rows have
    /// already passed MemoryStore's own retrieval predicate; lifecycle bodies
    /// additionally require a current canonical `is_memory_retrievable` fact.
    pub(crate) fn filter_retrievable_memory_hits(
        &self,
        hits: Vec<crate::memory::MemorySearchHit>,
    ) -> std::result::Result<Vec<crate::memory::MemorySearchHit>, MemoryRetrievalAuthorityError>
    {
        let reader = self
            .memory_lifecycle_retrieval_reader
            .ok_or(MemoryRetrievalAuthorityError::LifecycleReaderUnavailable)?;
        reader.ensure_available().map_err(|error| {
            MemoryRetrievalAuthorityError::LifecycleStoreQueryFailed(error.to_string())
        })?;
        let mut filtered = Vec::with_capacity(hits.len());
        for hit in hits {
            let Some(memory_id) = hit.chunk.source.strip_prefix("memory_lifecycle:") else {
                filtered.push(hit);
                continue;
            };
            if reader.is_memory_retrievable(memory_id).map_err(|error| {
                MemoryRetrievalAuthorityError::LifecycleStoreQueryFailed(error.to_string())
            })? {
                filtered.push(hit);
            }
        }
        Ok(filtered)
    }

    pub fn with_proposal_store(mut self, proposal_store: &'a crate::agent::ProposalStore) -> Self {
        self.proposal_store = Some(proposal_store);
        self
    }

    pub fn with_canonical_task_runtime_store(
        mut self,
        store: &'a crate::task_runtime::CanonicalTaskRuntimeStore,
    ) -> Self {
        self.bound_content_receipt_issuer = Some(store);
        self
    }

    pub fn with_action_bound_tool_permission(
        mut self,
        authorization: &'a crate::tool_permissions::ActionBoundToolPermissionAuthorization,
    ) -> Self {
        self.action_bound_tool_permission = Some(authorization);
        self
    }

    pub fn with_network_policy(mut self, network_policy: &'a crate::config::NetworkPolicy) -> Self {
        self.network_policy = Some(network_policy);
        self
    }

    pub fn with_web_search_fixture_output(mut self, output: &'a str) -> Self {
        self.web_search_fixture_output = Some(output);
        self
    }

    pub fn with_calendar_ics_paths(mut self, paths: &'a [String]) -> Self {
        self.calendar_ics_paths = paths;
        self
    }

    pub fn with_external_write_proposal_policy(mut self, required: bool) -> Self {
        self.external_write_requires_proposal = required;
        self
    }

    pub fn with_tool_dispatch_observer(mut self, observer: &'a dyn ToolDispatchObserver) -> Self {
        self.tool_dispatch_observer = Some(observer);
        self
    }

    pub fn with_tool_started_transition_observer(
        mut self,
        observer: &'a dyn ToolStartedTransitionObserver,
    ) -> Self {
        self.tool_started_transition_observer = Some(observer);
        self
    }

    pub fn with_tool_audit_persistence_observer(
        mut self,
        observer: &'a dyn ToolAuditPersistenceObserver,
    ) -> Self {
        self.tool_audit_persistence_observer = Some(observer);
        self
    }

    pub fn with_durable_store_failure_observer(
        mut self,
        observer: &'a dyn DurableStoreFailureObserver,
    ) -> Self {
        self.durable_store_failure_observer = Some(observer);
        self
    }

    pub(crate) fn observe_durable_store_failure(
        &self,
        store_kind: &'static str,
        raw_error: &impl std::fmt::Display,
    ) {
        if let Some(observer) = self.durable_store_failure_observer {
            observer.durable_store_failed(store_kind, &raw_error.to_string());
        }
    }

    pub fn with_canonical_write_admission(
        mut self,
        admission: &'a dyn CanonicalWriteAdmission,
    ) -> Self {
        self.canonical_write_admission = Some(admission);
        self
    }

    /// The only ToolGateway-owned entrypoint for creating, updating, or
    /// idempotently reusing a Review Center Proposal.
    pub(crate) fn submit_review_proposal(
        &self,
        request: DurableWriteRequest,
    ) -> Result<ReviewWorkflowOutcome> {
        let proposal_store = self
            .proposal_store
            .ok_or_else(|| anyhow::anyhow!("proposal_store_unavailable"))?;
        let admission = self
            .canonical_write_admission
            .ok_or_else(|| anyhow::anyhow!("canonical_write_admission_missing"))?;
        ReviewWorkflow::new(proposal_store).submit_with_admission(request, admission)
    }

    /// Run the fallible policy/claim fence immediately before a concrete
    /// adapter crosses its local or remote dispatch boundary. The opaque
    /// admission is the only value an adapter may consume to record that edge.
    pub(crate) async fn authorize_tool_dispatch(
        &self,
        manifest: &crate::tool_manifest::ToolManifest,
        request: &AgentActionRequest,
        args: &Value,
        receipt_tracker: &ToolExecutionReceiptTracker,
    ) -> Result<ToolDispatchAdmission<'a>> {
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_value_digest(args);
        let receipt = receipt_tracker.snapshot();
        let registry_binding = self.registry.dispatch_binding(manifest)?;
        if let Some(observer) = self.tool_dispatch_observer {
            observer
                .before_registry_dispatch(
                    &ToolDispatchAttempt {
                        receipt_id: receipt.receipt_id,
                        manifest_id: manifest.id.clone(),
                        tool_name: manifest.name.clone(),
                        manifest_contract_digest: manifest.execution_contract_digest(),
                        input_hash,
                        input_length_bytes: input_length_bytes as u64,
                        source_run_id: request.source_run_id.clone(),
                        request_digest: receipt.request_digest,
                        action_effect: receipt.action_effect,
                        idempotency_contract: receipt.idempotency_contract,
                        process_risk: tool_dispatch_process_risk_for_manifest(manifest),
                        effect_may_survive_local_process: effect_may_survive_local_process(
                            receipt.action_effect,
                        ),
                    },
                    &registry_binding,
                )
                .await?;
        }
        Ok(ToolDispatchAdmission::new(
            receipt_tracker.clone(),
            self.tool_started_transition_observer,
        ))
    }

    /// Run the same fallible execution-owner fence for a gateway-owned
    /// internal adapter. Internal reads do not have a registry manifest, but
    /// they still have a typed gateway contract and must therefore produce the
    /// same prepared -> adapter-edge transition as registry-backed tools.
    pub(crate) async fn authorize_internal_tool_dispatch(
        &self,
        request: &AgentActionRequest,
        receipt_tracker: &ToolExecutionReceiptTracker,
    ) -> Result<ToolDispatchAdmission<'a>> {
        let receipt = receipt_tracker.snapshot();
        let manifest_id = receipt
            .manifest_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("internal_tool_dispatch_contract_missing"))?;
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_value_digest(&request.input);
        let (_, manifest_contract_digest) =
            crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                "source": "tool_gateway_internal",
                "manifestId": &manifest_id,
                "actionEffect": receipt.action_effect.as_str(),
                "idempotencyContract": receipt.idempotency_contract,
            }));
        if let Some(observer) = self.tool_dispatch_observer {
            observer
                .before_dispatch(&ToolDispatchAttempt {
                    receipt_id: receipt.receipt_id,
                    manifest_id,
                    tool_name: request.action_type.clone(),
                    manifest_contract_digest,
                    input_hash,
                    input_length_bytes: input_length_bytes as u64,
                    source_run_id: request.source_run_id.clone(),
                    request_digest: receipt.request_digest,
                    action_effect: receipt.action_effect,
                    idempotency_contract: receipt.idempotency_contract,
                    process_risk: ToolDispatchProcessRisk::ProcessBound,
                    effect_may_survive_local_process: effect_may_survive_local_process(
                        receipt.action_effect,
                    ),
                })
                .await?;
        }
        Ok(ToolDispatchAdmission::new(
            receipt_tracker.clone(),
            self.tool_started_transition_observer,
        ))
    }
}

fn metadata_safe_preview(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

/// ToolGateway-owned implementation detail for action execution. Product and
/// external crate callers cannot construct or invoke it; ToolGateway is the
/// only execution authority.
pub(crate) struct ActionExecutor {
    config: ActionExecutorConfig,
}

impl ActionExecutor {
    pub(crate) fn new(config: ActionExecutorConfig) -> Self {
        Self { config }
    }

    async fn execute_with_receipt_tracker(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        receipt_tracker: ToolExecutionReceiptTracker,
        contract: &crate::agent::tool_gateway::ToolGatewayContractEvidence,
    ) -> Result<ActionExecutionResult> {
        let memory_read_can_dispatch = matches!(
            request.action_type.as_str(),
            "memory_search" | "session_search"
        ) && ctx.memory_store.is_some();
        if memory_read_can_dispatch {
            // This is the last fallible fence before the local MemoryStore
            // adapter is entered. A prepared attempt is not dispatch truth;
            // the receipt changes only at the concrete local adapter edge.
            ctx.authorize_internal_tool_dispatch(&request, &receipt_tracker)
                .await?
                .observe_local()
                .await?;
        }
        let mut result = match request.action_type.as_str() {
            "mcp_tool" | "builtin_tool" | "plugin_tool" => {
                self.execute_tool(request, ctx, receipt_tracker.clone())
                    .await
            }
            "memory_search" | "session_search" => {
                self.execute_memory_search(request, ctx, contract)
            }
            "memory_write" => self.execute_memory_write(request),
            "memory_archive" => self.execute_memory_archive(request),
            "life_model_patch" => self.execute_life_model_patch(request),
            _ => Err(anyhow::anyhow!(
                "unsupported action type: {}",
                request.action_type
            )),
        }?;
        if memory_read_can_dispatch {
            receipt_tracker.mark_response_observed();
            tool_executor::record_effect_outcome(
                &receipt_tracker,
                result.status == ActionExecutionStatus::Succeeded,
            );
        }
        receipt_tracker.finish();
        result.execution_receipt = receipt_tracker.snapshot();
        Ok(result)
    }

    pub(crate) async fn execute_for_tool_gateway(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        receipt_tracker: ToolExecutionReceiptTracker,
        contract: &crate::agent::tool_gateway::ToolGatewayContractEvidence,
        _authority: &crate::agent::tool_gateway::ToolGatewayFinalizationAuthority,
    ) -> Result<PendingToolGatewayActionExecution> {
        self.execute_with_receipt_tracker(request, ctx, receipt_tracker, contract)
            .await
            .map(|result| PendingToolGatewayActionExecution { result })
    }

    pub(crate) fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.config.timeout_seconds.max(1))
    }

    pub fn execute_life_model_patch(
        &self,
        request: AgentActionRequest,
    ) -> Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "life_model_patch must be submitted as a LifeModel proposal before persistence",
        ))
    }

    pub fn execute_memory_search(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        contract: &crate::agent::tool_gateway::ToolGatewayContractEvidence,
    ) -> Result<ActionExecutionResult> {
        let fallback_receipt = receipt_tracker_for_request(
            &request,
            Some(request.action_type.clone()),
            ToolActionEffect::ReadOnly,
            crate::tool_manifest::ToolIdempotencyContract::Idempotent,
        )
        .snapshot();
        let Some(memory_store) = ctx.memory_store else {
            return failed_memory_search_result(
                self,
                request,
                contract,
                "memory_store_unavailable",
                "memory store unavailable for read-only search",
                fallback_receipt,
            );
        };

        let query = request
            .input
            .get("query")
            .or_else(|| request.input.get("q"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let session_id = request.input.get("session_id").and_then(Value::as_str);
        let exclude_session_id = request
            .input
            .get("exclude_session_id")
            .and_then(Value::as_str);
        let limit = request
            .input
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(5)
            .clamp(1, 10) as usize;
        let is_session_search = request.action_type == "session_search";
        let raw_hits = match if is_session_search {
            memory_store.search_conversation_messages(session_id, exclude_session_id, query, limit)
        } else {
            memory_store.search_text_memories(session_id, query, limit)
        } {
            Ok(hits) => hits,
            Err(error) => {
                return failed_memory_search_result(
                    self,
                    request,
                    contract,
                    "memory_store_query_failed",
                    &error.to_string(),
                    fallback_receipt,
                );
            }
        };
        let hits = if is_session_search {
            // Conversation rows are already read from their canonical owner;
            // the MemoryLifecycle projection authority does not govern them.
            raw_hits
        } else {
            match ctx.filter_retrievable_memory_hits(raw_hits) {
                Ok(hits) => hits,
                Err(error) => {
                    return failed_memory_search_result(
                        self,
                        request,
                        contract,
                        error.reason_code(),
                        &error.to_string(),
                        fallback_receipt,
                    );
                }
            }
        };
        let hit_previews = hits
            .iter()
            .map(|hit| {
                serde_json::json!({
                    "sessionId": hit.chunk.session_id,
                    "source": hit.chunk.source,
                    "score": hit.relevance_score,
                    "preview": metadata_safe_preview(&hit.chunk.content, 160),
                    "createdAt": hit.chunk.created_at,
                })
            })
            .collect::<Vec<_>>();
        let content = if hits.is_empty() {
            format!("No memory/session hits found for query '{}'.", query)
        } else {
            let joined = hits
                .iter()
                .map(|hit| metadata_safe_preview(&hit.chunk.content, 180))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "Found {} memory/session hit(s) for query '{}':\n{}",
                hits.len(),
                query,
                joined
            )
        };
        let structured_result = serde_json::json!({
            "success": true,
            "status": "succeeded",
            "query": query,
            "sessionId": session_id,
            "hitCount": hits.len(),
            "hits": hit_previews,
            "directWritesExecuted": false,
            "promotedToMemory": false,
        });
        let (action, observation, observed_body_admission) = self
            .build_internal_read_action_observation(
                &request,
                contract,
                true,
                content,
                structured_result,
                None,
            )?;

        Ok(ActionExecutionResult {
            action,
            observation,
            status: ActionExecutionStatus::Succeeded,
            stop_reason: None,
            governance_report: None,
            execution_receipt: fallback_receipt,
            observed_body_admission,
        })
    }

    pub fn execute_memory_write(
        &self,
        request: AgentActionRequest,
    ) -> Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "memory_write must be submitted as a MemoryWrite proposal before persistence",
        ))
    }

    pub fn execute_memory_archive(
        &self,
        request: AgentActionRequest,
    ) -> Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "memory_archive must be submitted as a MemoryArchive proposal before persistence",
        ))
    }
}

fn failed_memory_search_result(
    executor: &ActionExecutor,
    request: AgentActionRequest,
    contract: &crate::agent::tool_gateway::ToolGatewayContractEvidence,
    reason_code: &str,
    detail: &str,
    execution_receipt: crate::tool_execution_receipt::ToolExecutionReceipt,
) -> Result<ActionExecutionResult> {
    let structured_result = serde_json::json!({
        "success": false,
        "status": "degraded",
        "reasonCode": reason_code,
        "errorDigest": crate::persistence_outbox::metadata_digest(detail),
        "hitCount": serde_json::Value::Null,
        "directWritesExecuted": false,
    });
    let (action, observation, observed_body_admission) = executor
        .build_internal_read_action_observation(
            &request,
            contract,
            false,
            format!("Memory search degraded: {reason_code}."),
            structured_result,
            Some(reason_code.to_string()),
        )?;
    Ok(ActionExecutionResult {
        action,
        observation,
        status: ActionExecutionStatus::Failed,
        stop_reason: Some(reason_code.to_string()),
        governance_report: None,
        execution_receipt,
        observed_body_admission,
    })
}

pub(crate) fn receipt_tracker_for_request(
    request: &AgentActionRequest,
    manifest_id: Option<String>,
    action_effect: ToolActionEffect,
    idempotency_contract: crate::tool_manifest::ToolIdempotencyContract,
) -> ToolExecutionReceiptTracker {
    let (_, request_digest) =
        crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
            "actionType": &request.action_type,
            "target": &request.target,
            "input": &request.input,
            "sourceRunId": &request.source_run_id,
            "stepIndex": request.step_index,
        }));
    ToolExecutionReceiptTracker::new(
        request.source_run_id.clone(),
        manifest_id,
        request_digest,
        action_effect,
        idempotency_contract,
    )
}
