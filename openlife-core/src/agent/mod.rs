pub mod accepted_guidance;
pub mod action_executor;
pub mod agent_loop;
pub mod backend_contract_freeze;
pub mod canonical_write_admission;
pub mod context_assembler;
pub mod conversation_context;
pub mod evidence_graph;
pub mod evidence_store;
pub mod golden_paths;
pub mod governor;
pub mod heuristic_store;
pub mod hs_asset_authority;
pub mod hs_selector;
pub mod life_model_explicit_read;
pub mod life_model_learning;
pub mod life_model_runtime_context;
pub mod life_model_view_model;
pub mod lifemodel_backend_completion;
pub mod main_chat_agent_v1;
pub mod main_chat_governance_intent;
pub mod main_chat_memory_candidate;
pub mod main_chat_runtime_contract;
pub mod maturation;
mod maturation_domain;
pub mod memory_lifecycle;
pub mod memory_service;
pub mod memory_view_model;
pub mod metadata_safe;
pub mod metrics;
pub mod model_router;
pub mod plan_execute;
pub mod policy_store;
pub mod product_read_model;
mod proposal_outcome;
pub mod proposal_store;
pub mod provider_privacy_boundary;
pub mod reasoning;
pub mod regression_suite;
pub mod review_decision_context;
pub mod review_item;
pub mod review_workflow;
pub mod runtime;
pub mod runtime_contract;
mod runtime_strategy_contract;
pub mod store;
pub mod strategy_runtime;
pub mod tasks_view_model;
pub mod tool_execution_owner;
pub mod tool_gateway;
pub mod types;

#[cfg(test)]
mod tests;

pub use crate::tool_execution_receipt::{
    ToolActionEffect, ToolAuditPersistenceStatus, ToolDispatchKind, ToolEffectStatus,
    ToolExecutionOutcome, ToolExecutionReceipt, ToolExecutionReceiptRegistration,
    ToolTransportStatus,
};
pub use accepted_guidance::{
    build_lifemodel_version_read_model, create_accepted_guidance_from_maturation_candidate,
    deactivate_accepted_guidance, AcceptedGuidanceLifecycleInput, AcceptedGuidanceLifecycleReport,
    AcceptedGuidanceRollbackPath, AcceptedGuidanceVersionRef, LifeModelRollbackReadModelRef,
    LifeModelVersionAssetDiffRef, LifeModelVersionReadModel,
};
pub use action_executor::{
    A2AOutboundAuthorization, ActionExecutionContext, ActionExecutionResult, ActionExecutionStatus,
    ActionExecutorConfig, AgentActionRequest, CanonicalStateSnapshot, DurableStoreFailureObserver,
    DurableToolExecutionOwner, ToolAuditPersistenceObserver, ToolDispatchAttempt,
    ToolDispatchObserver, ToolStartedTransitionObserver,
};
pub use agent_loop::apply_react_guidance_to_config;
pub use agent_loop::{
    AgentLoop, AgentLoopAllowedToolAction, AgentLoopConfig, AgentLoopResult, AgentLoopRunRequest,
    AgentLoopTerminalDisposition, StreamingCallback,
};
pub use backend_contract_freeze::{
    evaluate_final_backend_completion_gate, freeze_pre_ui_backend_read_model_contracts,
    BackendCompletionAcceptanceGateStatus, BackendCompletionGateEvidence,
    DefaultChatIsolationProof, FinalBackendCompletionGateInput, FinalBackendCompletionGateReport,
    GoldenPathCoverageProof, LearningInboxItem, LearningInboxReadModel, LifeModelOverviewReadModel,
    LocalOnlyPrivacyProof, PreUiBackendContractFreezeInput, PreUiBackendContractFreezeReport,
    PrivacyControlsReadModel, PrivacyDecisionSummary, ProposalFirstBoundaryProof,
    ProposalReviewItem, ProposalReviewReadModel, RawContentExclusionProof, RuntimeTraceHsInfluence,
    RuntimeTraceReadModel, RuntimeTraceRunItem, ToolGovernanceProof, UiReadModelGateProof,
    UiReadModelSurfaceContract,
};
pub use canonical_write_admission::{
    CanonicalWriteAdmission, CanonicalWriteAdmissionRejection, CanonicalWriteAdmissionRequest,
    CanonicalWritePermit,
};
pub use context_assembler::{
    AssembleInput, AssembleOutput, CompositeAssembler, ContextAssembler, LifeModelAssembler,
    MemoryAssembler, MemoryHit, PrivacyAssembler, ToolsAssembler,
};
pub use evidence_graph::{
    build_evidence_timeline, evaluate_evidence_graph, EvidenceClusterSummary,
    EvidenceConflictState, EvidenceCooldownState, EvidenceDecayState, EvidenceGraphInput,
    EvidenceGraphLink, EvidenceGraphLinkKind, EvidenceGraphReport, EvidencePolarity,
    EvidenceSourceWeightSummary, EvidenceTimelineItem, EvidenceTimelineReadModel,
};
pub use evidence_store::{
    EvidenceDraft, EvidencePrivacyLevel, EvidenceQuery, EvidenceRecord, EvidenceSourceRef,
    EvidenceSourceType, EvidenceStatus, EvidenceStore, EvidenceTombstone, EvidenceType,
};
pub use golden_paths::{
    run_low_energy_support_golden_path, run_preference_correction_golden_path,
    run_weekly_planning_golden_path, LowEnergySupportGoldenPathInput,
    LowEnergySupportGoldenPathReport, PreferenceCorrectionGoldenPathInput,
    PreferenceCorrectionGoldenPathReport, WeeklyPlanningGoldenPathInput,
    WeeklyPlanningGoldenPathReport,
};
pub use governor::{
    ExternalWriteGovernanceInput, GovernanceDecision, GovernanceDecisionClassification,
    GovernanceDecisionKind, GovernanceSubject, GovernorDecisionReport, LifeModelGovernor,
    MemoryWriteGovernanceInput, ModelRouteGovernanceInput, ToolGovernanceInput,
};
pub use heuristic_store::{
    DomainCapDiagnostic, HeuristicActivationAuthority, HeuristicConstraintSet, HeuristicDraft,
    HeuristicLifecycleStatus, HeuristicLineage, HeuristicQuery, HeuristicRecord, HeuristicStore,
    HeuristicUsageMetadata, HeuristicValidationState,
};
pub use hs_asset_authority::{
    build_collaboration_guidance_projection, complete_collaboration_guidance_cutover,
    digest_string, reconcile_collaboration_guidance_authority, CollaborationGuidanceCutoverReport,
    CollaborationGuidanceCutoverStatus, CollaborationGuidanceProjection, HSAssetAuthorityRecord,
    HSAssetAuthorityRegistry, HSAssetCategory, HSAssetOwner, HSAssetWriteKind, HSAssetWriteRequest,
    ProductScenarioReceipt, RollbackRehearsalReceipt, ShadowParityReceipt,
};
pub use hs_selector::{
    behavior_checks_for_packet, build_guidance_impact_read_model, build_runtime_hs_packet,
    GuidanceAffectedSurface, GuidanceImpactReadModel, GuidanceImpactRef,
    GuidancePolicyBoundarySummary, HSAssetExclusion, HSAssetKind, HSExclusionReason,
    HSSelectionAudit, HSSelector, HSSelectorInput, RuntimeHSPacket, RuntimeHSPacketBuildInput,
    SelectedGuidanceRef, SelectedHeuristic, SelectedPolicyRef,
};
pub use life_model_explicit_read::{
    is_explicit_lifemodel_read_intent, LifeModelExplicitReadAnswer, LifeModelExplicitReadFact,
};
pub use life_model_learning::{
    life_model_learning_candidate_snapshot_digest, LifeModelLearningCandidate,
    LifeModelLearningCandidateStatus, LifeModelLearningCapture, LifeModelLearningCaptureReceipt,
    LifeModelLearningDecisionReceipt, LifeModelLearningEvidencePolarity,
    LifeModelLearningExplicitness, LifeModelLearningMaterializationEvidence,
    LifeModelLearningObservation, LifeModelLearningReviewDecisionReceipt,
    LifeModelLearningSensitivity, LifeModelLearningSourceKind, LifeModelLearningStore,
    LifeModelLearningSuppressionKind,
};
pub use life_model_runtime_context::{LifeModelRuntimeContextV2, LifeModelRuntimeFactV2};
pub use life_model_view_model::{
    build_life_model_view_model_envelope, LifeModelCandidateChange,
    LifeModelCandidateDecisionStatus, LifeModelCanonicalSummary, LifeModelCanonicalV2Input,
    LifeModelChangeKind, LifeModelLearningSummary, LifeModelManualOverrideState,
    LifeModelMemoryLinkageStatus, LifeModelMemoryLinkageSummary, LifeModelMemoryTierStatsInput,
    LifeModelOwnerStatus, LifeModelPendingUpdateCounts, LifeModelProjectionInput,
    LifeModelReadiness, LifeModelTierSummary, LifeModelTrustQualityState, LifeModelTruthMode,
    LifeModelViewModel, LifeModelViewModelBuildInput,
};
pub use lifemodel_backend_completion::{
    bridge_life_signal_to_evidence, evaluate_lifemodel_backend_completion_readiness,
    extract_life_signals, CanonicalLifeEventOwnerKind, CanonicalLifeEventOwnerRef,
    CanonicalLifeEventSourceProof, DroppedLifeSignal, LifeDomain, LifeEvent, LifeEventPrivacyLevel,
    LifeEventSourceRef, LifeEventSourceType, LifeEventSourceVerification, LifeEventStore,
    LifeModelBackendCompletionReadinessReport, LifeModelBackendGateBlocker,
    LifeModelBackendGovernanceReadiness, LifeModelBackendPrerequisites, LifeSignal,
    LifeSignalBridgeInput, LifeSignalEvidenceBridgeReport, LifeSignalExtractorInput,
    LifeSignalExtractorReport, LifeSignalPolarity, LifeSignalType,
};
pub use main_chat_governance_intent::{
    extract_main_chat_intent_signals, MainChatBlockerRequirement, MainChatDurableWriteRequirement,
    MainChatExternalReadRequirement, MainChatIntentSignals,
};
pub use main_chat_memory_candidate::{
    extract_main_chat_memory_candidates, plan_main_chat_memory_routing, route_memory_candidates,
    MainChatMemoryCandidate, MainChatMemoryRoutingResult, MemoryCandidateKind, MemoryDestination,
};
pub use maturation::{
    ensure_accepted_low_energy_rule_selection, ensure_lifemodel_maturation_non_default_invocation,
    ensure_lifemodel_maturation_readiness, ensure_low_energy_rule_trace_visibility,
    evaluate_accepted_low_energy_rule_selection, evaluate_lifemodel_maturation_readiness,
    evaluate_low_energy_collaboration_rule_candidate, evaluate_low_energy_rule_trace_visibility,
    evaluate_maturation_engine_v1, propose_low_energy_collaboration_rule_candidate,
    run_lifemodel_maturation_non_default_invocation,
    AcceptedLowEnergyRuleSelectionHSPacketAuditProof, AcceptedLowEnergyRuleSelectionInput,
    AcceptedLowEnergyRuleSelectionReport, LifeModelMaturationNonDefaultInvocationInput,
    LifeModelMaturationNonDefaultInvocationReport, LifeModelMaturationReadinessInput,
    LifeModelMaturationReadinessReport, LifeModelMaturationReadinessSideEffectBudget,
    LifeModelMaturationService, LowEnergyCollaborationRuleCandidateInput,
    LowEnergyCollaborationRuleCandidateReport, LowEnergyRuleTraceLineageItem,
    LowEnergyRuleTraceLineageSummary, LowEnergyRuleTraceMetadata,
    LowEnergyRuleTraceVisibilityInput, LowEnergyRuleTraceVisibilityReport,
    MaturationCandidateDomain, MaturationCandidateSuppressionReport, MaturationDropReason,
    MaturationEngineCandidate, MaturationEngineV1Input, MaturationEngineV1Report,
    MaturationGovernanceAudit, MaturationGovernanceSummary, MaturationInput, MaturationOutput,
    MaturationProposalCandidate, MaturationReport, MaturationService,
};
pub use memory_lifecycle::{
    bind_memory_fact_scope_owner, memory_lifecycle_category_for_candidate_kind,
    memory_scope_owner_ref, CanonicalMemoryFactDescriptor, ExplicitMemoryWriteInput,
    ExplicitMemoryWriteReceipt, MemoryAdmissionOutcome, MemoryLifecycleAcceptanceInput,
    MemoryLifecycleAcceptanceReport, MemoryLifecycleCategory, MemoryLifecycleEvent,
    MemoryLifecycleRecord, MemoryLifecycleRetrievalReader, MemoryLifecycleRiskLevel,
    MemoryLifecycleScope, MemoryLifecycleSensitivity, MemoryLifecycleStatus, MemoryLifecycleStore,
    MemoryMaterializationStatus, MemoryMaterializedView, MemoryPrivacyEraseReport,
    MemoryRollbackEvent, MemoryRollbackReport,
};
pub use memory_service::{EmbeddingConfig, MemoryContext, MemoryService};
pub use memory_view_model::{
    build_memory_view_model, MemoryItemView, MemoryLaneSummary, MemoryLifeModelLinkageStatus,
    MemoryLifeModelLinkageSummary, MemoryLifecycleSummary, MemoryTierSummary, MemoryViewModel,
    MemoryViewModelBuildInput, MemoryViewModelSummary,
};
pub use metadata_safe::{
    metadata_safe_text_digest, metadata_safe_text_preview, metadata_safe_value_digest,
    metadata_safe_value_preview,
};
pub use metrics::{RolloutMetric, RolloutMetricsStore, RolloutSummary};
pub use model_router::{
    ModelRouteDecision, ModelRouteScore, ModelRouter, PrivacyRequirement, ProviderAvailability,
    TaskType,
};
pub use plan_execute::{
    PlanDraft, PlanExecuteCancelResult, PlanExecuteInput, PlanExecuteLifeModelHint,
    PlanExecuteProductAuthorityReport, PlanExecuteProductContract,
    PlanExecuteProductContractReport, PlanExecuteProductScenario, PlanExecuteReport,
    PlanExecuteReviewItem, PlanExecuteReviewSummary, PlanExecuteService, PlanExecuteSession,
    PlanExecuteSessionStatus, PlanExecuteSessionStore, PlanExecuteStepEdit,
    PlanExecuteStepExecutionResult, PlanExecuteStepRecord, PlanExecutionOutput,
    PlanGovernanceDecisionSummary, PlanObservationSummary, PlanStep, PlanStepStatus, PlanStepTrace,
};
pub use policy_store::{
    ContextPolicyDecision, HeuristicPolicyEffect, ModelRoutePolicy, PolicyConflictAudit,
    PolicyEvaluationRequest, PolicyRecord, PolicyStore, PolicyTopic, ToolPolicyDecision,
    BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING, BUILTIN_HEURISTIC_REJECTED_REMINDER_DELAY,
    BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST, BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY,
};
pub use product_read_model::{
    BackendEntityKind, BackendEntityRef, DebugAction, DebugActionKind, EvidenceRef,
    EvidenceSensitivity, EvidenceSource, ExternalTransmissionStatus, ProductAction,
    ProductActionKind, ProductReadModelContractError, ProductRiskLevel,
    ProviderPrivacyBoundarySummary, ProviderRouteType, ReviewAction, ReviewActionEffect,
    ReviewActionKind, ReviewItemMaterializationStatus, ViewModelActions, ViewModelEnvelope,
    ViewModelSource, ViewModelStatus, ViewModelWarning, ViewModelWarningSeverity,
};
#[cfg(test)]
pub use proposal_outcome::{
    evaluate_maturation_proposal_outcome_evidence, record_maturation_proposal_outcome_evidence,
    MaturationProposalOutcome, MaturationProposalOutcomeEvidenceReport,
};
pub use proposal_store::{
    ArtifactEffectRecord, ArtifactEffectState, ProposalStore, ProposalTerminalRelationKind,
    ProposalTerminalRelationProjectionProof, TerminalOwnerOriginBinding,
};
pub use provider_privacy_boundary::{
    build_provider_privacy_boundary_summary, ProviderPrivacyBoundaryBuildInput,
};
pub use reasoning::layered::{SafetyCheckResult, SafetyChecker};
pub use reasoning::{
    DirectReasoner, LayeredReasoner, ReasoningConfig, ReasoningError, ReasoningInput,
    ReasoningOutput, ReasoningPhaseKind, ReasoningStrategy, ReasoningTrace,
};
pub use regression_suite::{
    RegressionResult, RegressionScenario, RegressionSuite, RegressionVerdict,
};
pub use review_decision_context::{
    build_review_decision_context, GovernedActionReviewContract, PermissionDecisionContext,
    PermissionDecisionContextStatus, PermissionPolicyKind, PermissionRequestDigestKind,
    PermissionScopeKind, PermissionTransmissionBoundary, ReviewDecisionContext,
    ReviewReadableValue, ReviewReadableValueKind,
};
pub use review_item::{
    build_review_center_view_model, build_review_item, ReviewBatch, ReviewBatchDomain,
    ReviewCenterBuildInput, ReviewCenterSummary, ReviewCenterViewModel, ReviewItem,
    ReviewItemArtifactEvidence, ReviewItemDecisionStatus, ReviewItemSource, ReviewItemSourceKind,
    ReviewItemTaskResumeRelation, ReviewItemType,
};
pub use review_workflow::{
    proposal_status_semantics, DurableWriteDecision, DurableWriteDecisionKind, DurableWriteRequest,
    DurableWriteSource, DurableWriteSubject, FinalDeliveryWordingContract,
    MaterializedReviewAcceptanceSnapshot, ReviewWorkflow, ReviewWorkflowOutcome,
    TerminalOwnerReviewOriginProof, TerminalOwnerReviewSubmission,
};
pub use runtime::{AgentRuntime, AgentRuntimeConfig, AgentRuntimeError, AgentRuntimeOutput};
pub use runtime_contract::{
    AgentRuntimeParams, LifeEventDraft, RuntimeGuidanceConsumptionMode, RuntimeInput, RuntimeOutput,
};
pub use runtime_strategy_contract::{
    RuntimeStrategyKind, StrategyCandidateEvaluation, StrategySelection, StrategySelectionInput,
    StrategySelectionReport,
};
pub use store::{
    issue_agent_run_review_relation_projection_lane, AgentRunReviewRelationProjectionLane,
    AgentRunReviewRelationProjectionLaneAdmission, AgentRunReviewRelationProjectionOutcome,
    AgentRunStore, AgentRunTerminalRelationTargetIntentAdmission,
};
pub use strategy_runtime::{
    PlanExecuteRuntimeStrategy, ReActRuntimeStrategy, RuntimeStrategy,
    RuntimeStrategyDeclarativeDescriptor, RuntimeStrategyDescriptor,
    RuntimeStrategyExecutionReport, RuntimeStrategyInput, RuntimeStrategyOutput,
    RuntimeStrategyPayload, RuntimeStrategyPayloadKind, RuntimeStrategyRegistry,
    RuntimeStrategyRegistryReadinessReport, RuntimeStrategySideEffectBudget,
};
pub use tasks_view_model::{
    build_tasks_view_model, build_workspace_view_model, TaskControl, TaskControlEffect,
    TaskControlKind, TaskLatestResultPreview, TaskLifecycleStatus, TaskTerminalDeliveryStatus,
    TaskViewModelContractError, TaskViewModelItem, TaskViewModelRunInput, TaskViewModelTaskInput,
    TasksViewModel, TasksViewModelBuildInput, TasksViewModelSummary, WorkspaceActivityItem,
    WorkspaceActivityKind, WorkspaceActivityStatus, WorkspaceViewModel,
    WorkspaceViewModelBuildInput,
};
#[cfg(any(test, feature = "test-utils"))]
pub use tool_execution_owner::AgentRunToolExecutionFaultPoint;
pub use tool_execution_owner::{
    AgentRunA2AToolExecutionOwner, AgentRunToolExecutionRecord, AgentRunToolExecutionState,
};
pub use tool_gateway::{
    validate_manifest_execution_contract, ToolGateway, ToolGatewayContractEvidence,
};
pub use types::*;
