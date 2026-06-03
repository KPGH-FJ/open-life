pub mod action_executor;
pub mod agent_loop;
pub mod context_assembler;
pub mod evidence_store;
pub mod governor;
pub mod heuristic_store;
pub mod hs_selector;
pub mod maturation;
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

pub use action_executor::{
    ActionExecutionContext, ActionExecutionResult, ActionExecutionStatus, ActionExecutor,
    ActionExecutorConfig, AgentActionRequest,
};
pub use agent_loop::{AgentLoop, AgentLoopConfig, AgentLoopResult, StreamingCallback};
pub use context_assembler::{
    AssembleInput, AssembleOutput, CompositeAssembler, ContextAssembler, LifeModelAssembler,
    MemoryAssembler, MemoryHit, PrivacyAssembler, ToolsAssembler,
};
pub use evidence_store::{
    EvidenceDraft, EvidencePrivacyLevel, EvidenceQuery, EvidenceRecord, EvidenceSourceRef,
    EvidenceSourceType, EvidenceStatus, EvidenceStore, EvidenceTombstone, EvidenceType,
};
pub use governor::{
    GovernanceDecision, GovernanceDecisionKind, GovernanceSubject, LifeModelGovernor,
    ModelRouteGovernanceInput, ToolGovernanceInput,
};
pub use heuristic_store::{
    DomainCapDiagnostic, HeuristicActivationAuthority, HeuristicDraft, HeuristicLifecycleStatus,
    HeuristicLineage, HeuristicQuery, HeuristicRecord, HeuristicStore, HeuristicUsageMetadata,
    HeuristicValidationState,
};
pub use hs_selector::{
    behavior_checks_for_packet, build_runtime_hs_packet, HSAssetExclusion, HSAssetKind,
    HSExclusionReason, HSSelectionAudit, HSSelector, HSSelectorInput, RuntimeHSPacket,
    RuntimeHSPacketBuildInput, SelectedHeuristic, SelectedPolicyRef,
};
pub use maturation::{
    ensure_accepted_low_energy_rule_selection, ensure_lifemodel_maturation_non_default_invocation,
    ensure_lifemodel_maturation_readiness, ensure_low_energy_rule_trace_visibility,
    evaluate_accepted_low_energy_rule_selection, evaluate_lifemodel_maturation_readiness,
    evaluate_low_energy_collaboration_rule_candidate, evaluate_low_energy_rule_trace_visibility,
    propose_low_energy_collaboration_rule_candidate,
    run_lifemodel_maturation_non_default_invocation,
    AcceptedLowEnergyRuleSelectionHSPacketAuditProof, AcceptedLowEnergyRuleSelectionInput,
    AcceptedLowEnergyRuleSelectionReport, LifeModelMaturationNonDefaultInvocationInput,
    LifeModelMaturationNonDefaultInvocationReport, LifeModelMaturationReadinessInput,
    LifeModelMaturationReadinessReport, LifeModelMaturationReadinessSideEffectBudget,
    LifeModelMaturationService, LowEnergyCollaborationRuleCandidateInput,
    LowEnergyCollaborationRuleCandidateReport, LowEnergyRuleTraceLineageItem,
    LowEnergyRuleTraceLineageSummary, LowEnergyRuleTraceMetadata,
    LowEnergyRuleTraceVisibilityInput, LowEnergyRuleTraceVisibilityReport, MaturationDropReason,
    MaturationGovernanceAudit, MaturationGovernanceSummary, MaturationInput, MaturationOutput,
    MaturationProposalCandidate, MaturationReport, MaturationService,
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
pub use reasoning::layered::{SafetyCheckResult, SafetyChecker};
pub use reasoning::{
    DirectReasoner, LayeredReasoner, ReasoningConfig, ReasoningError, ReasoningInput,
    ReasoningOutput, ReasoningPhaseKind, ReasoningStrategy, ReasoningTrace,
};
pub use regression_suite::{
    RegressionResult, RegressionScenario, RegressionSuite, RegressionVerdict,
};
pub use runtime::{AgentRuntime, AgentRuntimeConfig, AgentRuntimeError, AgentRuntimeOutput};
pub use runtime_contract::{AgentRuntimeParams, LifeEventDraft, RuntimeInput, RuntimeOutput};
pub use runtime_migration_gate::{
    evaluate_controlled_chat_pilot_eligibility, evaluate_runtime_migration_gate,
    ControlledChatPilotEligibilityInput, ControlledChatPilotEligibilityReport,
    RuntimeMigrationGateInput, RuntimeMigrationGateReport,
    DEFAULT_CONTROLLED_CHAT_PILOT_REQUIRED_CLEAN_RUNS,
};
pub use store::AgentRunStore;
pub use strategy::{
    RuntimeStrategyKind, StrategySelection, StrategySelectionInput, StrategySelector,
};
pub use strategy_runtime::{
    PlanExecuteRuntimeStrategy, ReActRuntimeStrategy, RuntimeStrategy, RuntimeStrategyInput,
    RuntimeStrategyOutput, RuntimeStrategyPayload, RuntimeStrategyPayloadKind,
    RuntimeStrategyRegistry,
};
pub use types::*;
