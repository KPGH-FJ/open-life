pub mod accepted_guidance;
pub mod action_executor;
pub mod agent_loop;
pub mod backend_contract_freeze;
pub mod context_assembler;
pub mod evidence_graph;
pub mod evidence_store;
pub mod golden_paths;
pub mod governor;
pub mod heuristic_store;
pub mod hs_selector;
pub mod lifemodel_backend_completion;
pub mod main_chat_agent_productization_v1;
pub mod main_chat_agent_v1;
pub mod maturation;
mod maturation_domain;
pub mod memory_lifecycle;
pub mod memory_service;
pub mod metrics;
pub mod model_router;
pub mod multi_strategy_runtime;
pub mod plan_execute;
pub mod policy_store;
pub mod proposal_engine;
pub mod proposal_generators;
pub mod proposal_outcome;
pub mod proposal_store;
pub mod react_beta;
pub mod reasoning;
pub mod regression_suite;
pub mod runtime;
pub mod runtime_contract;
pub mod runtime_migration_gate;
pub mod store;
pub mod strategy;
pub mod strategy_runtime;
pub mod types;

#[cfg(test)]
mod tests;

pub use accepted_guidance::{
    build_lifemodel_version_read_model, create_accepted_guidance_from_maturation_candidate,
    deactivate_accepted_guidance, AcceptedGuidanceLifecycleInput, AcceptedGuidanceLifecycleReport,
    AcceptedGuidanceRollbackPath, AcceptedGuidanceVersionRef, LifeModelRollbackReadModelRef,
    LifeModelVersionAssetDiffRef, LifeModelVersionReadModel,
};
pub use action_executor::{
    ActionExecutionContext, ActionExecutionResult, ActionExecutionStatus, ActionExecutor,
    ActionExecutorConfig, AgentActionRequest,
};
pub use agent_loop::apply_react_guidance_to_config;
pub use agent_loop::{
    AgentLoop, AgentLoopAllowedToolAction, AgentLoopConfig, AgentLoopResult, StreamingCallback,
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
pub use hs_selector::{
    behavior_checks_for_packet, build_guidance_impact_read_model, build_runtime_hs_packet,
    GuidanceAffectedSurface, GuidanceImpactReadModel, GuidanceImpactRef,
    GuidancePolicyBoundarySummary, HSAssetExclusion, HSAssetKind, HSExclusionReason,
    HSSelectionAudit, HSSelector, HSSelectorInput, RuntimeHSPacket, RuntimeHSPacketBuildInput,
    SelectedGuidanceRef, SelectedHeuristic, SelectedPolicyRef,
};
pub use lifemodel_backend_completion::{
    bridge_life_signal_to_evidence, evaluate_lifemodel_backend_completion_readiness,
    extract_life_signals, DroppedLifeSignal, LifeDomain, LifeEvent, LifeEventPrivacyLevel,
    LifeEventSourceRef, LifeEventSourceType, LifeEventStore,
    LifeModelBackendCompletionReadinessReport, LifeModelBackendGateBlocker,
    LifeModelBackendGovernanceReadiness, LifeModelBackendPrerequisites, LifeSignal,
    LifeSignalBridgeInput, LifeSignalEvidenceBridgeReport, LifeSignalExtractorInput,
    LifeSignalExtractorReport, LifeSignalPolarity, LifeSignalType,
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
    MemoryLifecycleAcceptanceInput, MemoryLifecycleAcceptanceReport, MemoryLifecycleCategory,
    MemoryLifecycleEvent, MemoryLifecycleRecord, MemoryLifecycleRiskLevel, MemoryLifecycleScope,
    MemoryLifecycleStatus, MemoryLifecycleStore, MemoryMaterializationStatus,
    MemoryMaterializedView, MemoryRollbackEvent, MemoryRollbackReport,
};
pub use memory_service::{EmbeddingConfig, MemoryContext, MemoryService};
pub use metrics::{RolloutMetric, RolloutMetricsStore, RolloutSummary};
pub use model_router::{
    ModelRouteDecision, ModelRouteScore, ModelRouter, PrivacyRequirement, ProviderAvailability,
    ProviderHealth, TaskType,
};
pub use multi_strategy_runtime::{
    MultiStrategyRuntime, MultiStrategyRuntimeInput, MultiStrategyRuntimeOutput,
    MultiStrategyRuntimePayload,
};
pub use plan_execute::{
    PlanDraft, PlanExecuteInput, PlanExecuteProductAuthorityReport, PlanExecuteProductContract,
    PlanExecuteProductContractReport, PlanExecuteProductScenario, PlanExecuteReport,
    PlanExecuteService, PlanExecuteSession, PlanExecuteSessionStatus, PlanExecuteSessionStore,
    PlanExecuteStepEdit, PlanExecuteStepExecutionResult, PlanExecuteStepRecord,
    PlanExecutionOutput, PlanGovernanceDecisionSummary, PlanObservationSummary, PlanStep,
    PlanStepStatus, PlanStepTrace,
};
pub use policy_store::{
    ContextPolicyDecision, HeuristicPolicyEffect, ModelRoutePolicy, PolicyConflictAudit,
    PolicyEvaluationRequest, PolicyRecord, PolicyStore, PolicyTopic, ToolPolicyDecision,
    BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING, BUILTIN_HEURISTIC_REJECTED_REMINDER_DELAY,
    BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST, BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY,
};
pub use proposal_engine::{
    BuilderProposalGenerator, CalibrationProposalGenerator, ChatProposalGeneratorAdapter,
    FeedbackProposalGenerator, MemoryProposalGenerator, ProposalEngine, ProposalGenerator,
    ToolProposalGenerator,
};
pub use proposal_generators::ChatProposalGenerator;
pub use proposal_outcome::{
    evaluate_maturation_proposal_outcome_evidence, record_maturation_proposal_outcome_evidence,
    MaturationProposalOutcome, MaturationProposalOutcomeEvidenceReport,
};
pub use proposal_store::ProposalStore;
pub use react_beta::{
    evaluate_react_beta_execution_readiness, evaluate_react_beta_execution_readiness_for_input,
    evaluate_tool_registry_beta_readiness, metadata_safe_value_digest, metadata_safe_value_preview,
    ReactBetaExecutionReadinessInput, ReactBetaExecutionReadinessReport,
    ReactBetaReadinessComponentOverride, ToolRegistryBetaReadinessReport,
    ToolRegistryBetaToolReport,
};
pub use reasoning::layered::{SafetyCheckResult, SafetyChecker};
pub use reasoning::{
    DirectReasoner, LayeredReasoner, ReasoningConfig, ReasoningError, ReasoningInput,
    ReasoningOutput, ReasoningPhaseKind, ReasoningStrategy, ReasoningTrace,
};
pub use regression_suite::{
    RegressionResult, RegressionScenario, RegressionSuite, RegressionVerdict,
};
pub use runtime::{AgentRuntime, AgentRuntimeConfig, AgentRuntimeError, AgentRuntimeOutput};
pub use runtime_contract::{
    AgentRuntimeParams, LifeEventDraft, RuntimeGuidanceConsumptionMode, RuntimeInput, RuntimeOutput,
};
pub use runtime_migration_gate::{
    evaluate_controlled_chat_pilot_eligibility, evaluate_runtime_migration_gate,
    ControlledChatPilotEligibilityInput, ControlledChatPilotEligibilityReport,
    RuntimeMigrationGateInput, RuntimeMigrationGateReport,
    DEFAULT_CONTROLLED_CHAT_PILOT_REQUIRED_CLEAN_RUNS,
};
pub use store::AgentRunStore;
pub use strategy::{
    RuntimeStrategyKind, StrategyCandidateEvaluation, StrategySelection, StrategySelectionInput,
    StrategySelectionReport, StrategySelector,
};
pub use strategy_runtime::{
    MultiStrategyRuntimeMaturityReport, PlanExecuteRuntimeStrategy, ReActRuntimeStrategy,
    RuntimeStrategy, RuntimeStrategyDeclarativeDescriptor, RuntimeStrategyDescriptor,
    RuntimeStrategyExecutionReport, RuntimeStrategyInput, RuntimeStrategyOutput,
    RuntimeStrategyPayload, RuntimeStrategyPayloadKind, RuntimeStrategyRegistry,
    RuntimeStrategyRegistryReadinessReport, RuntimeStrategySideEffectBudget,
};
pub use types::*;
