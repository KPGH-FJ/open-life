pub mod action_executor;
pub mod agent_loop;
pub mod context_assembler;
pub mod evidence_store;
pub mod heuristic_store;
pub mod hs_selector;
pub mod memory_service;
pub mod metrics;
pub mod model_router;
pub mod policy_store;
pub mod proposal_engine;
pub mod proposal_generators;
pub mod proposal_store;
pub mod reasoning;
pub mod regression_suite;
pub mod runtime;
pub mod store;
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
pub use heuristic_store::{
    DomainCapDiagnostic, HeuristicActivationAuthority, HeuristicDraft, HeuristicLifecycleStatus,
    HeuristicLineage, HeuristicQuery, HeuristicRecord, HeuristicStore, HeuristicUsageMetadata,
    HeuristicValidationState,
};
pub use hs_selector::{
    HSAssetExclusion, HSAssetKind, HSExclusionReason, HSSelectionAudit, HSSelector,
    HSSelectorInput, RuntimeHSPacket, SelectedHeuristic, SelectedPolicyRef,
};
pub use memory_service::{EmbeddingConfig, MemoryContext, MemoryService};
pub use metrics::{RolloutMetric, RolloutMetricsStore, RolloutSummary};
pub use model_router::{
    ModelRouteDecision, ModelRouteScore, ModelRouter, PrivacyRequirement, ProviderAvailability,
    ProviderHealth, TaskType,
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
pub use store::AgentRunStore;
pub use types::*;
