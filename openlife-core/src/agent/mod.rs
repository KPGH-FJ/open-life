pub mod action_executor;
pub mod agent_loop;
pub mod agent_spec_store;
pub mod compaction;
pub mod context_assembler;
pub mod event_store;
pub mod execution_facade;
pub mod execution_sandbox;
pub mod memory_evidence;
pub mod memory_service;
pub mod metrics;
pub mod model_router;
pub mod plan_executor;
pub mod plan_mode;
pub mod plan_store;
pub mod prompt_stack;
pub mod proposal_engine;
pub mod proposal_generators;
pub mod proposal_store;
pub mod reasoning;
pub mod runtime;
pub mod store;
pub mod sub_agent;
pub mod types;

#[cfg(test)]
mod tests;

pub use compaction::{
    build_safe_compacted_observation, compact_messages_for_agent_loop, should_compact,
    CompactionConfig, CompactionDecision, CompactionInput, CompactionResult,
    CompactionSummaryBuilder, estimate_message_tokens,
};
pub use action_executor::{
    ActionExecutionContext, ActionExecutionResult, ActionExecutionStatus, ActionExecutor,
    ActionExecutorConfig, AgentActionRequest,
};
pub use agent_loop::{AgentLoop, AgentLoopConfig, AgentLoopResult, StreamingCallback};
pub use agent_spec_store::AgentSpecStore;
pub use context_assembler::{
    AssembleInput, AssembleOutput, CompositeAssembler, ContextAssembler, LifeModelAssembler,
    MemoryAssembler, MemoryHit, PrivacyAssembler, ToolsAssembler,
};
pub use memory_service::{EmbeddingConfig, MemoryContext, MemoryService};
pub use metrics::{RolloutMetric, RolloutMetricsStore, RolloutSummary};
pub use model_router::{
    ModelRouteDecision, ModelRouteScore, ModelRouter, PrivacyRequirement, ProviderAvailability,
    ProviderHealth, TaskType,
};
pub use plan_executor::{
    DefaultPlanReviewGate, PlanExecutionError, PlanExecutor, PlanReviewGate, SubAgentReviewGate,
};
pub use plan_mode::{
    check_confirmation_required, is_plan_mode_read_only, record_confirmation_requested,
    PlanConfirmation, PlanModeConfig, PlanModeRunner, PlanModeToolClass,
};
pub use plan_store::PlanStore;
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
pub use runtime::{AgentRuntime, AgentRuntimeConfig, AgentRuntimeError, AgentRuntimeOutput};
pub use store::AgentRunStore;
pub use sub_agent::{
    ReviewAgentOutput, ReviewIssue, ReviewVerdict, SubAgentExecutionOutcome, SubAgentResult,
    SubAgentRuntime,
};
pub use types::AgentSpecStoreError;
pub use types::PlanOperationResult;
pub use types::*;
