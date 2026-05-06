use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── vNext AgentRunEvent ──────────────────────────────────────────────

/// Append-only event kinds for every meaningful runtime transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRunEventType {
    RunCreated,
    ContextAssembled,
    ModelRouteSelected,
    ModelCallStarted,
    ModelCallCompleted,
    ModelCallFailed,
    ToolCallStarted,
    ToolCallBlocked,
    ToolCallCompleted,
    ToolCallFailed,
    ObservationCreated,
    ProposalCreated,
    FallbackStarted,
    FallbackCompleted,
    JsonRepairStarted,
    JsonRepairCompleted,
    RunCompleted,
    RunFailed,
    PlanCreated,
    PlanConfirmationRequested,
    PlanConfirmationResolved,
    PlanExecutionStarted,
    PlanStepStarted,
    PlanStepCompleted,
    PlanStepFailed,
    PlanDeviationRecorded,
    PlanExecutionCompleted,
    PlanExecutionFailed,
    PlanCancelRequested,
    PlanCancelled,
    PlanRetryRequested,
    PlanRetryStarted,
    PlanContinuationRequested,
    PlanActionReplayed,
    PlanActionReplayRequested,
    /// Unknown or future event type — preserved as-is in the trace.
    /// Older builds reading traces from newer builds use this variant.
    Unknown(String),
}

impl std::fmt::Display for AgentRunEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRunEventType::RunCreated => write!(f, "run.created"),
            AgentRunEventType::ContextAssembled => write!(f, "context.assembled"),
            AgentRunEventType::ModelRouteSelected => write!(f, "model.route_selected"),
            AgentRunEventType::ModelCallStarted => write!(f, "model.call_started"),
            AgentRunEventType::ModelCallCompleted => write!(f, "model.call_completed"),
            AgentRunEventType::ModelCallFailed => write!(f, "model.call_failed"),
            AgentRunEventType::ToolCallStarted => write!(f, "tool.call_started"),
            AgentRunEventType::ToolCallBlocked => write!(f, "tool.call_blocked"),
            AgentRunEventType::ToolCallCompleted => write!(f, "tool.call_completed"),
            AgentRunEventType::ToolCallFailed => write!(f, "tool.call_failed"),
            AgentRunEventType::ObservationCreated => write!(f, "observation.created"),
            AgentRunEventType::ProposalCreated => write!(f, "proposal.created"),
            AgentRunEventType::FallbackStarted => write!(f, "fallback.started"),
            AgentRunEventType::FallbackCompleted => write!(f, "fallback.completed"),
            AgentRunEventType::JsonRepairStarted => write!(f, "json_repair.started"),
            AgentRunEventType::JsonRepairCompleted => write!(f, "json_repair.completed"),
            AgentRunEventType::RunCompleted => write!(f, "run.completed"),
            AgentRunEventType::RunFailed => write!(f, "run.failed"),
            AgentRunEventType::PlanCreated => write!(f, "plan.created"),
            AgentRunEventType::PlanConfirmationRequested => {
                write!(f, "plan.confirmation_requested")
            }
            AgentRunEventType::PlanConfirmationResolved => {
                write!(f, "plan.confirmation_resolved")
            }
            AgentRunEventType::PlanExecutionStarted => write!(f, "plan.execution_started"),
            AgentRunEventType::PlanStepStarted => write!(f, "plan.step_started"),
            AgentRunEventType::PlanStepCompleted => write!(f, "plan.step_completed"),
            AgentRunEventType::PlanStepFailed => write!(f, "plan.step_failed"),
            AgentRunEventType::PlanDeviationRecorded => {
                write!(f, "plan.deviation_recorded")
            }
            AgentRunEventType::PlanExecutionCompleted => {
                write!(f, "plan.execution_completed")
            }
            AgentRunEventType::PlanExecutionFailed => write!(f, "plan.execution_failed"),
            AgentRunEventType::PlanCancelRequested => write!(f, "plan.cancel_requested"),
            AgentRunEventType::PlanCancelled => write!(f, "plan.cancelled"),
            AgentRunEventType::PlanRetryRequested => write!(f, "plan.retry_requested"),
            AgentRunEventType::PlanRetryStarted => write!(f, "plan.retry_started"),
            AgentRunEventType::PlanContinuationRequested => {
                write!(f, "plan.continuation_requested")
            }
            AgentRunEventType::PlanActionReplayed => write!(f, "plan.action_replayed"),
            AgentRunEventType::PlanActionReplayRequested => {
                write!(f, "plan.action_replay_requested")
            }
            AgentRunEventType::Unknown(raw) => write!(f, "{}", raw),
        }
    }
}

impl serde::Serialize for AgentRunEventType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for AgentRunEventType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Ok(match raw.as_str() {
            "run.created" => AgentRunEventType::RunCreated,
            "context.assembled" => AgentRunEventType::ContextAssembled,
            "model.route_selected" => AgentRunEventType::ModelRouteSelected,
            "model.call_started" => AgentRunEventType::ModelCallStarted,
            "model.call_completed" => AgentRunEventType::ModelCallCompleted,
            "model.call_failed" => AgentRunEventType::ModelCallFailed,
            "tool.call_started" => AgentRunEventType::ToolCallStarted,
            "tool.call_blocked" => AgentRunEventType::ToolCallBlocked,
            "tool.call_completed" => AgentRunEventType::ToolCallCompleted,
            "tool.call_failed" => AgentRunEventType::ToolCallFailed,
            "observation.created" => AgentRunEventType::ObservationCreated,
            "proposal.created" => AgentRunEventType::ProposalCreated,
            "fallback.started" => AgentRunEventType::FallbackStarted,
            "fallback.completed" => AgentRunEventType::FallbackCompleted,
            "json_repair.started" => AgentRunEventType::JsonRepairStarted,
            "json_repair.completed" => AgentRunEventType::JsonRepairCompleted,
            "plan.created" => AgentRunEventType::PlanCreated,
            "plan.confirmation_requested" => AgentRunEventType::PlanConfirmationRequested,
            "plan.confirmation_resolved" => AgentRunEventType::PlanConfirmationResolved,
            "plan.execution_started" => AgentRunEventType::PlanExecutionStarted,
            "plan.step_started" => AgentRunEventType::PlanStepStarted,
            "plan.step_completed" => AgentRunEventType::PlanStepCompleted,
            "plan.step_failed" => AgentRunEventType::PlanStepFailed,
            "plan.deviation_recorded" => AgentRunEventType::PlanDeviationRecorded,
            "plan.execution_completed" => AgentRunEventType::PlanExecutionCompleted,
            "plan.execution_failed" => AgentRunEventType::PlanExecutionFailed,
            "plan.cancel_requested" => AgentRunEventType::PlanCancelRequested,
            "plan.cancelled" => AgentRunEventType::PlanCancelled,
            "plan.retry_requested" => AgentRunEventType::PlanRetryRequested,
            "plan.retry_started" => AgentRunEventType::PlanRetryStarted,
            "plan.continuation_requested" => AgentRunEventType::PlanContinuationRequested,
            "plan.action_replayed" => AgentRunEventType::PlanActionReplayed,
            "plan.action_replay_requested" => AgentRunEventType::PlanActionReplayRequested,
            "run.completed" => AgentRunEventType::RunCompleted,
            "run.failed" => AgentRunEventType::RunFailed,
            other => AgentRunEventType::Unknown(other.to_string()),
        })
    }
}

/// Who or what originated an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEventActor {
    User,
    Agent,
    SubAgent(String),
    Tool(String),
    Runtime,
    System,
}

impl std::fmt::Display for AgentEventActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentEventActor::User => write!(f, "user"),
            AgentEventActor::Agent => write!(f, "agent"),
            AgentEventActor::SubAgent(name) => write!(f, "sub_agent:{}", name),
            AgentEventActor::Tool(name) => write!(f, "tool:{}", name),
            AgentEventActor::Runtime => write!(f, "runtime"),
            AgentEventActor::System => write!(f, "system"),
        }
    }
}

/// Optional redaction summary for events with sensitive payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactionSummary {
    pub redacted: bool,
    pub reason: String,
    pub fields_removed: Vec<String>,
}

/// A single append-only trace event belonging to an AgentRun.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunEvent {
    pub id: String,
    pub run_id: String,
    pub parent_event_id: Option<String>,
    pub event_type: AgentRunEventType,
    pub phase: Option<String>,
    pub actor: AgentEventActor,
    pub summary: String,
    pub payload: serde_json::Value,
    pub redaction: Option<RedactionSummary>,
    pub created_at: DateTime<Utc>,
}

impl AgentRunEvent {
    /// Create a new event with a UUID id and current timestamp.
    pub fn new(
        run_id: &str,
        event_type: AgentRunEventType,
        actor: AgentEventActor,
        summary: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            run_id: run_id.to_string(),
            parent_event_id: None,
            event_type,
            phase: None,
            actor,
            summary: summary.into(),
            payload,
            redaction: None,
            created_at: Utc::now(),
        }
    }

    /// Set parent event linkage (e.g., for JSON repair after model failure).
    pub fn with_parent(mut self, parent_event_id: &str) -> Self {
        self.parent_event_id = Some(parent_event_id.to_string());
        self
    }

    /// Set execution phase label.
    pub fn with_phase(mut self, phase: impl Into<String>) -> Self {
        self.phase = Some(phase.into());
        self
    }

    /// Mark this event as redacted with the given summary.
    pub fn with_redaction(
        mut self,
        reason: impl Into<String>,
        fields_removed: Vec<String>,
    ) -> Self {
        self.redaction = Some(RedactionSummary {
            redacted: true,
            reason: reason.into(),
            fields_removed,
        });
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskKind {
    Conversation,
    /// 构建/编辑 LifeModel（用户交互式构建）
    Builder,
    Calibration,
    Evolution,
    ToolExecution,
    Proactive,
    Planning,
    Review,
    Writing,
    MemoryGovernance,
    Skill,
    Plugin,
}

impl std::fmt::Display for AgentTaskKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentTaskKind::Conversation => write!(f, "conversation"),
            AgentTaskKind::Builder => write!(f, "builder"),
            AgentTaskKind::Calibration => write!(f, "calibration"),
            AgentTaskKind::Evolution => write!(f, "evolution"),
            AgentTaskKind::ToolExecution => write!(f, "tool_execution"),
            AgentTaskKind::Proactive => write!(f, "proactive"),
            AgentTaskKind::Planning => write!(f, "planning"),
            AgentTaskKind::Review => write!(f, "review"),
            AgentTaskKind::Writing => write!(f, "writing"),
            AgentTaskKind::MemoryGovernance => write!(f, "memory_governance"),
            AgentTaskKind::Skill => write!(f, "skill"),
            AgentTaskKind::Plugin => write!(f, "plugin"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentExecutionBudget {
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub timeout_seconds: u64,
    pub allow_cloud: bool,
    pub allow_writes: bool,
}

impl Default for AgentExecutionBudget {
    fn default() -> Self {
        Self {
            max_steps: 5,
            max_tool_calls: 3,
            timeout_seconds: 60,
            allow_cloud: true,
            allow_writes: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for AgentTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentTaskStatus::Pending => write!(f, "pending"),
            AgentTaskStatus::Running => write!(f, "running"),
            AgentTaskStatus::Completed => write!(f, "completed"),
            AgentTaskStatus::Failed => write!(f, "failed"),
            AgentTaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A formal task submitted for agent execution.
///
/// Separated from `AgentRun` so that intent, policy, and constraints are
/// captured before execution begins.  The runtime may use these fields to
/// select models, assemble context, and enforce governance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTask {
    /// Unique identifier for this task.
    pub id: String,
    /// Task category.
    pub kind: AgentTaskKind,
    pub session_id: String,
    /// User intent as raw text or structured description.
    pub user_text: String,
    pub messages: Vec<crate::llm::ChatMessage>,
    #[serde(default)]
    pub layer: crate::layer_router::Layer,
    /// Who or what initiated this task.
    pub initiator: String,
    /// AgentSpec id that governs this task's execution.
    pub agent_spec_id: Option<String>,
    /// Whether this task requires a plan before execution.
    #[serde(default)]
    pub requires_plan: bool,
    /// Expected output description (guides the agent).
    pub expected_output: Option<String>,
    /// Workspace scope for file/context access.
    pub workspace_scope: Option<String>,
    /// Privacy policy for this task (overrides AgentSpec default when set).
    pub privacy_policy: Option<PrivacyPolicy>,
    pub status: AgentTaskStatus,
}

impl Default for AgentTask {
    fn default() -> Self {
        Self::new(AgentTaskKind::Conversation, "default")
    }
}

impl AgentTask {
    pub fn new(kind: AgentTaskKind, session_id: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            session_id: session_id.into(),
            user_text: String::new(),
            messages: Vec::new(),
            layer: crate::layer_router::Layer::L3,
            initiator: "user".to_string(),
            agent_spec_id: None,
            requires_plan: false,
            expected_output: None,
            workspace_scope: None,
            privacy_policy: None,
            status: AgentTaskStatus::Pending,
        }
    }

    pub fn with_user_text(mut self, text: impl Into<String>) -> Self {
        self.user_text = text.into();
        self
    }

    pub fn with_agent_spec(mut self, spec_id: impl Into<String>) -> Self {
        self.agent_spec_id = Some(spec_id.into());
        self
    }

    pub fn with_requires_plan(mut self) -> Self {
        self.requires_plan = true;
        self
    }

    pub fn with_privacy(mut self, policy: PrivacyPolicy) -> Self {
        self.privacy_policy = Some(policy);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunStatus {
    Running,
    WaitingPermission,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for AgentRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRunStatus::Running => write!(f, "running"),
            AgentRunStatus::WaitingPermission => write!(f, "waiting_permission"),
            AgentRunStatus::Completed => write!(f, "completed"),
            AgentRunStatus::Failed => write!(f, "failed"),
            AgentRunStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Phase of the AgentLoop execution for real-time status streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLoopPhase {
    /// Understanding the task and planning next step
    Thinking,
    /// Model has decided to use a tool, preparing the call
    PlanningTool,
    /// Tool is being executed
    ExecutingTool,
    /// Tool result received, processing observation
    Observing,
    /// Waiting for user permission confirmation
    WaitingPermission,
    /// Generating the final answer
    GeneratingFinal,
    /// Execution completed successfully
    Completed,
    /// Execution failed
    Failed,
}

impl std::fmt::Display for AgentLoopPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentLoopPhase::Thinking => write!(f, "thinking"),
            AgentLoopPhase::PlanningTool => write!(f, "planning_tool"),
            AgentLoopPhase::ExecutingTool => write!(f, "executing_tool"),
            AgentLoopPhase::Observing => write!(f, "observing"),
            AgentLoopPhase::WaitingPermission => write!(f, "waiting_permission"),
            AgentLoopPhase::GeneratingFinal => write!(f, "generating_final"),
            AgentLoopPhase::Completed => write!(f, "completed"),
            AgentLoopPhase::Failed => write!(f, "failed"),
        }
    }
}

/// A single status update emitted during AgentLoop execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLoopStatusUpdate {
    pub phase: AgentLoopPhase,
    pub message: String,
    pub step_index: u32,
    pub tool_call_index: Option<u32>,
    pub timestamp: DateTime<Utc>,
}

/// Trace of which model was chosen and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionLevel {
    None,
    Light,
    Summary,
    Strict,
    LocalOnly,
}

impl std::fmt::Display for RedactionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RedactionLevel::None => write!(f, "none"),
            RedactionLevel::Light => write!(f, "light"),
            RedactionLevel::Summary => write!(f, "summary"),
            RedactionLevel::Strict => write!(f, "strict"),
            RedactionLevel::LocalOnly => write!(f, "local_only"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRouteTrace {
    pub provider: String,
    pub model: String,
    pub route_type: String, // "local" | "cloud" | "fallback" | "direct"
    pub prefer_local: bool,
    pub local_model: String,
    pub reason: String,
    pub privacy_level: RedactionLevel,
    pub latency_ms: Option<u64>,
    pub retry_count: u32,
    #[serde(default)]
    pub fallback_reason: Option<String>,
    #[serde(default)]
    pub provider_health_is_estimated: Option<bool>,
}

/// Summary of what context was included in the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSummary {
    pub life_model_empty: bool,
    pub included_life_model_sections: Vec<String>,
    pub memory_hit_count: i64,
    pub memory_sources: Vec<String>,
    pub used_tools_prompt: bool,
    pub redaction_applied: bool,
    pub redaction_level: RedactionLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolActionScope {
    pub tool_id: String,
    pub tool_name: String,
    pub source: String,
    pub risk_level: String,
    pub capabilities: Vec<String>,
    pub action_type: String,
    pub requires_confirmation: bool,
    pub allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAction {
    pub id: String,
    pub action_type: String,
    #[serde(default)]
    pub target: Option<String>,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub status: String,
    #[serde(default)]
    pub permission_decision: Option<String>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub tool_scope: Option<ToolActionScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentObservation {
    pub id: String,
    #[serde(default)]
    pub action_id: Option<String>,
    pub content: String,
    pub source: String,
    #[serde(default)]
    pub structured_result: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

/// Error information when a run fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunError {
    pub message: String,
    pub phase: String, // "preprocess" | "model" | "stream" | "fallback" | "reasoning"
    pub recoverable: bool,
}

/// A single traceable execution of an Agent task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRun {
    pub id: String,
    pub task_id: String,
    pub session_id: Option<String>,
    pub status: AgentRunStatus,
    pub kind: AgentTaskKind,
    pub user_input: Option<String>,
    pub context_summary: Option<ContextSummary>,
    pub model_route: Option<ModelRouteTrace>,
    pub output_preview: Option<String>,
    pub error: Option<AgentRunError>,
    #[serde(default)]
    pub generated_proposals: Vec<String>,
    #[serde(default)]
    pub actions: Vec<AgentAction>,
    #[serde(default)]
    pub observations: Vec<AgentObservation>,
    /// Reasoning strategy used (e.g., "layered", "direct")
    pub reasoning_strategy: Option<String>,
    /// Trace from the reasoning process (e.g., LayeredReasoner phases)
    pub reasoning_trace: Option<crate::agent::reasoning::ReasoningTrace>,
    /// Warnings generated during execution (e.g., parse warnings, budget warnings)
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Status updates emitted during AgentLoop execution
    #[serde(default)]
    pub status_updates: Vec<AgentLoopStatusUpdate>,
    /// Number of steps executed
    #[serde(default)]
    pub step_count: u32,
    /// Number of tool calls made
    #[serde(default)]
    pub tool_call_count: u32,
    pub deleted_at: Option<DateTime<Utc>>,
    pub delete_reason: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl AgentRun {
    pub fn new_chat_run(session_id: &str, user_input: &str) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            task_id: Uuid::new_v4().to_string(),
            session_id: Some(session_id.to_string()),
            status: AgentRunStatus::Running,
            kind: AgentTaskKind::Conversation,
            user_input: Some(user_input.to_string()),
            context_summary: None,
            model_route: None,
            output_preview: None,
            error: None,
            generated_proposals: Vec::new(),
            actions: Vec::new(),
            observations: Vec::new(),
            reasoning_strategy: None,
            reasoning_trace: None,
            warnings: Vec::new(),
            status_updates: Vec::new(),
            step_count: 0,
            tool_call_count: 0,
            deleted_at: None,
            delete_reason: None,
            started_at: now,
            finished_at: None,
        }
    }

    pub fn new_builder_run(session_id: &str) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            task_id: Uuid::new_v4().to_string(),
            session_id: Some(session_id.to_string()),
            status: AgentRunStatus::Running,
            kind: AgentTaskKind::Builder,
            user_input: None,
            context_summary: None,
            model_route: None,
            output_preview: None,
            error: None,
            generated_proposals: Vec::new(),
            actions: Vec::new(),
            observations: Vec::new(),
            reasoning_strategy: None,
            reasoning_trace: None,
            warnings: Vec::new(),
            status_updates: Vec::new(),
            step_count: 0,
            tool_call_count: 0,
            deleted_at: None,
            delete_reason: None,
            started_at: now,
            finished_at: None,
        }
    }

    pub fn new_calibration_run() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            task_id: Uuid::new_v4().to_string(),
            session_id: None,
            status: AgentRunStatus::Running,
            kind: AgentTaskKind::Calibration,
            user_input: None,
            context_summary: None,
            model_route: None,
            output_preview: None,
            error: None,
            generated_proposals: Vec::new(),
            actions: Vec::new(),
            observations: Vec::new(),
            reasoning_strategy: None,
            reasoning_trace: None,
            warnings: Vec::new(),
            status_updates: Vec::new(),
            step_count: 0,
            tool_call_count: 0,
            deleted_at: None,
            delete_reason: None,
            started_at: now,
            finished_at: None,
        }
    }

    pub fn new_tool_execution_run(tool_name: &str) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            task_id: Uuid::new_v4().to_string(),
            session_id: None,
            status: AgentRunStatus::Running,
            kind: AgentTaskKind::ToolExecution,
            user_input: Some(format!("Direct tool call: {}", tool_name)),
            context_summary: None,
            model_route: None,
            output_preview: None,
            error: None,
            generated_proposals: Vec::new(),
            actions: Vec::new(),
            observations: Vec::new(),
            reasoning_strategy: None,
            reasoning_trace: None,
            warnings: Vec::new(),
            status_updates: Vec::new(),
            step_count: 0,
            tool_call_count: 0,
            deleted_at: None,
            delete_reason: None,
            started_at: now,
            finished_at: None,
        }
    }

    pub fn complete(
        &mut self,
        output_preview: &str,
        model_route: ModelRouteTrace,
        context_summary: ContextSummary,
    ) {
        self.status = AgentRunStatus::Completed;
        self.output_preview = Some(output_preview.to_string());
        self.model_route = Some(model_route);
        self.context_summary = Some(context_summary);
        self.finished_at = Some(Utc::now());
    }

    pub fn fail(&mut self, error: AgentRunError) {
        self.status = AgentRunStatus::Failed;
        self.error = Some(error);
        self.finished_at = Some(Utc::now());
    }

    pub fn cancel(&mut self) {
        self.status = AgentRunStatus::Cancelled;
        self.finished_at = Some(Utc::now());
    }

    pub fn add_generated_proposal(&mut self, proposal_id: &str) {
        self.generated_proposals.push(proposal_id.to_string());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Pending,
    Accepted,
    Rejected,
    Edited,
    Postponed,
}

impl std::fmt::Display for ProposalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalStatus::Pending => write!(f, "pending"),
            ProposalStatus::Accepted => write!(f, "accepted"),
            ProposalStatus::Rejected => write!(f, "rejected"),
            ProposalStatus::Edited => write!(f, "edited"),
            ProposalStatus::Postponed => write!(f, "postponed"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalType {
    GoalUpdate,
    StateUpdate,
    PreferenceUpdate,
    CapabilityUpdate,
    MemoryWrite,
    MemoryArchive,
    ToolPermission,
    PluginPermission,
    ScheduledTask,
    ExternalWriteAction,
    ModelPolicyChange,
    DataExport,
    ScheduleCheckin,
    /// Unknown or future proposal type that this build cannot safely apply.
    Unsupported,
    /// 兼容旧数据
    #[serde(alias = "life_model_update")]
    LifeModelUpdate,
}

impl std::fmt::Display for ProposalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalType::GoalUpdate => write!(f, "goal_update"),
            ProposalType::StateUpdate => write!(f, "state_update"),
            ProposalType::PreferenceUpdate => write!(f, "preference_update"),
            ProposalType::CapabilityUpdate => write!(f, "capability_update"),
            ProposalType::MemoryWrite => write!(f, "memory_write"),
            ProposalType::MemoryArchive => write!(f, "memory_archive"),
            ProposalType::ToolPermission => write!(f, "tool_permission"),
            ProposalType::PluginPermission => write!(f, "plugin_permission"),
            ProposalType::ScheduledTask => write!(f, "scheduled_task"),
            ProposalType::ExternalWriteAction => write!(f, "external_write_action"),
            ProposalType::ModelPolicyChange => write!(f, "model_policy_change"),
            ProposalType::DataExport => write!(f, "data_export"),
            ProposalType::ScheduleCheckin => write!(f, "schedule_checkin"),
            ProposalType::Unsupported => write!(f, "unsupported"),
            ProposalType::LifeModelUpdate => write!(f, "life_model_update"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalSource {
    BuilderReview,
    CalibrationRun,
    FeedbackEvolution,
    MemoryGovernance,
    SkillRuntime,
    Plugin,
    Manual,
    ChatConversation,
    /// Agent 主动发起的提案（如定期检查、触发式建议）
    ProactiveAgent,
}

impl std::fmt::Display for ProposalSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalSource::BuilderReview => write!(f, "builder_review"),
            ProposalSource::CalibrationRun => write!(f, "calibration_run"),
            ProposalSource::FeedbackEvolution => write!(f, "feedback_evolution"),
            ProposalSource::MemoryGovernance => write!(f, "memory_governance"),
            ProposalSource::SkillRuntime => write!(f, "skill_runtime"),
            ProposalSource::Plugin => write!(f, "plugin"),
            ProposalSource::Manual => write!(f, "manual"),
            ProposalSource::ChatConversation => write!(f, "chat_conversation"),
            ProposalSource::ProactiveAgent => write!(f, "proactive_agent"),
        }
    }
}

impl rusqlite::types::ToSql for ProposalSource {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

impl rusqlite::types::FromSql for ProposalSource {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().and_then(|s| match s {
            "builder_review" => Ok(ProposalSource::BuilderReview),
            "calibration_run" => Ok(ProposalSource::CalibrationRun),
            "feedback_evolution" => Ok(ProposalSource::FeedbackEvolution),
            "memory_governance" => Ok(ProposalSource::MemoryGovernance),
            "skill_runtime" => Ok(ProposalSource::SkillRuntime),
            "plugin" => Ok(ProposalSource::Plugin),
            "manual" => Ok(ProposalSource::Manual),
            "proactive_agent" => Ok(ProposalSource::ProactiveAgent),
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProposal {
    pub id: String,
    pub run_id: Option<String>,
    pub proposal_type: ProposalType,
    pub source: ProposalSource,
    pub source_detail: Option<String>,
    pub affected_path: String,
    pub before: Option<serde_json::Value>,
    pub after: serde_json::Value,
    pub reason: String,
    pub confidence: f32,
    pub risk_level: RiskLevel,
    pub status: ProposalStatus,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl AgentProposal {
    pub fn new(
        proposal_type: ProposalType,
        affected_path: &str,
        after: serde_json::Value,
        reason: &str,
        confidence: f32,
        risk_level: RiskLevel,
        source: ProposalSource,
    ) -> Self {
        let expires_at = Self::calculate_expires_at(source);
        Self {
            id: Uuid::new_v4().to_string(),
            run_id: None,
            proposal_type,
            source,
            source_detail: None,
            affected_path: affected_path.to_string(),
            before: None,
            after,
            reason: reason.to_string(),
            confidence,
            risk_level,
            status: ProposalStatus::Pending,
            created_at: Utc::now(),
            resolved_at: None,
            expires_at,
        }
    }

    /// Calculate expiration time based on source
    fn calculate_expires_at(source: ProposalSource) -> Option<DateTime<Utc>> {
        let duration = match source {
            ProposalSource::BuilderReview => chrono::Duration::days(30),
            ProposalSource::CalibrationRun => chrono::Duration::days(14),
            ProposalSource::FeedbackEvolution => chrono::Duration::days(7),
            ProposalSource::MemoryGovernance => chrono::Duration::days(7),
            ProposalSource::SkillRuntime => chrono::Duration::days(14),
            ProposalSource::Plugin => chrono::Duration::days(14),
            ProposalSource::Manual => chrono::Duration::days(365),
            ProposalSource::ChatConversation => chrono::Duration::days(3),
            ProposalSource::ProactiveAgent => chrono::Duration::days(7),
        };
        Some(Utc::now() + duration)
    }

    /// Check if proposal is expired
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires) => Utc::now() > expires,
            None => false,
        }
    }

    /// Get days until expiration (negative if expired)
    pub fn days_until_expiration(&self) -> Option<i64> {
        self.expires_at.map(|expires| {
            let now = Utc::now();
            let duration = expires.signed_duration_since(now);
            duration.num_days()
        })
    }

    /// Backward compatibility: get run_id
    pub fn get_run_id(&self) -> Option<&str> {
        self.run_id.as_deref()
    }

    pub fn accept(&mut self) {
        self.status = ProposalStatus::Accepted;
        self.resolved_at = Some(Utc::now());
    }

    pub fn reject(&mut self) {
        self.status = ProposalStatus::Rejected;
        self.resolved_at = Some(Utc::now());
    }

    pub fn edit(&mut self, new_after: serde_json::Value) {
        self.after = new_after;
        self.status = ProposalStatus::Edited;
        self.resolved_at = Some(Utc::now());
    }

    pub fn postpone(&mut self) {
        self.status = ProposalStatus::Postponed;
        self.resolved_at = Some(Utc::now());
    }
}

// ── AgentPlan types ───────────────────────────────────────────────────

/// Status of an AgentPlan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Draft,
    /// Plan has been published and may need confirmation
    Published,
    /// User has confirmed the plan
    Confirmed,
    /// Plan is being executed
    Executing,
    /// Plan execution completed
    Completed,
    /// Plan was rejected by user
    Rejected,
    /// Plan was cancelled
    Cancelled,
    /// Plan execution failed at one or more steps.
    Failed,
    /// Plan execution completed but review found critical issues.
    FailedReview,
}

impl std::fmt::Display for PlanStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanStatus::Draft => write!(f, "draft"),
            PlanStatus::Published => write!(f, "published"),
            PlanStatus::Confirmed => write!(f, "confirmed"),
            PlanStatus::Executing => write!(f, "executing"),
            PlanStatus::Completed => write!(f, "completed"),
            PlanStatus::Rejected => write!(f, "rejected"),
            PlanStatus::Cancelled => write!(f, "cancelled"),
            PlanStatus::Failed => write!(f, "failed"),
            PlanStatus::FailedReview => write!(f, "failed_review"),
        }
    }
}

/// A single step in a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub index: u32,
    pub description: String,
    pub tool_intent: Option<String>,
    pub expected_output: Option<String>,
    pub depends_on: Vec<u32>,
}

/// Intent to use a specific tool during plan execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolIntent {
    pub tool_name: String,
    pub purpose: String,
    pub risk_level: RiskLevel,
    pub is_write: bool,
    pub parameters_summary: Option<String>,
}

/// Sub-agent assignment within a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentAssignment {
    pub agent_role: String,
    pub task: String,
    pub delegation_mode: String,
}

/// Permission requirement declared in a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequirement {
    pub target: String,
    pub reason: String,
    pub risk_level: RiskLevel,
}

/// A structured plan for complex or risky agent tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPlan {
    pub id: String,
    pub run_id: Option<String>,
    pub session_id: Option<String>,
    /// AgentSpec id that governs this plan's execution.
    /// When set, the plan executor resolves tools/context/prompts against this spec.
    #[serde(default)]
    pub agent_spec_id: Option<String>,
    pub goal: String,
    pub assumptions: Vec<String>,
    pub missing_context: Vec<String>,
    pub steps: Vec<PlanStep>,
    pub tool_intents: Vec<ToolIntent>,
    pub subagent_assignments: Vec<SubAgentAssignment>,
    pub permission_requirements: Vec<PermissionRequirement>,
    pub rollback_plan: Option<String>,
    pub success_criteria: Vec<String>,
    pub risk_level: RiskLevel,
    pub requires_confirmation: bool,
    pub status: PlanStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl AgentPlan {
    pub fn new(goal: impl Into<String>, risk_level: RiskLevel) -> Self {
        let now = Utc::now();
        let requires_confirmation = !matches!(risk_level, RiskLevel::Low);
        Self {
            id: Uuid::new_v4().to_string(),
            run_id: None,
            session_id: None,
            agent_spec_id: None,
            goal: goal.into(),
            assumptions: Vec::new(),
            missing_context: Vec::new(),
            steps: Vec::new(),
            tool_intents: Vec::new(),
            subagent_assignments: Vec::new(),
            permission_requirements: Vec::new(),
            rollback_plan: None,
            success_criteria: Vec::new(),
            risk_level,
            requires_confirmation,
            status: PlanStatus::Draft,
            created_at: now,
            updated_at: now,
            confirmed_at: None,
            completed_at: None,
        }
    }

    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_agent_spec(mut self, spec_id: impl Into<String>) -> Self {
        self.agent_spec_id = Some(spec_id.into());
        self
    }

    pub fn publish(&mut self) {
        self.status = PlanStatus::Published;
        self.updated_at = Utc::now();
    }

    pub fn confirm(&mut self) {
        self.status = PlanStatus::Confirmed;
        self.confirmed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    pub fn reject(&mut self) {
        self.status = PlanStatus::Rejected;
        self.updated_at = Utc::now();
    }

    pub fn start_execution(&mut self) {
        self.status = PlanStatus::Executing;
        self.updated_at = Utc::now();
    }

    pub fn complete(&mut self) {
        self.status = PlanStatus::Completed;
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Set plan status to Cancelled.
    pub fn cancel(&mut self) {
        self.status = PlanStatus::Cancelled;
        self.updated_at = Utc::now();
    }

    /// Reset a failed or failed-review plan back to Confirmed for retry.
    pub fn retry(&mut self) {
        self.status = PlanStatus::Confirmed;
        self.completed_at = None;
        self.updated_at = Utc::now();
    }

    /// Returns true if any tool intent involves write side effects.
    pub fn has_write_intents(&self) -> bool {
        self.tool_intents.iter().any(|t| t.is_write)
    }

    /// Returns true if any sub-agent assignment uses handoff mode.
    pub fn has_handoff_assignments(&self) -> bool {
        self.subagent_assignments
            .iter()
            .any(|a| a.delegation_mode == "handoff")
    }
}

// ── Plan Execution types ──────────────────────────────────────────────

/// Execution mode for a confirmed plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanExecutionMode {
    /// Execute steps sequentially, stopping on first failure.
    Sequential,
}

/// Result of executing a single plan step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStepExecutionResult {
    pub step_index: u32,
    pub tool_name: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
    /// Whether the executed action deviated from the plan's declared tool intent.
    pub deviation: Option<String>,
}

/// Outcome of a completed or failed plan execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecutionOutcome {
    pub plan_id: String,
    pub success: bool,
    pub steps_completed: u32,
    pub steps_failed: u32,
    pub deviations: Vec<String>,
    /// Whether the execution result requires review via ReviewAgent.
    pub review_required: bool,
}

// ── Plan Operation Result (stable frontend/backend contract) ──────────

/// Structured result for every plan lifecycle operation.
///
/// All Tauri plan commands (`execute_agent_plan`, `confirm_agent_plan`,
/// `reject_agent_plan`, and future `cancel` / `retry`) return this type
/// so that frontend callers receive a uniform, camelCase contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOperationResult {
    pub plan_id: String,
    pub run_id: Option<String>,
    /// Operation name, e.g. "execute", "confirm", "reject"
    pub operation: String,
    pub success: bool,
    /// Final plan status after the operation.
    pub status: PlanStatus,
    pub steps_completed: Option<u32>,
    pub steps_failed: Option<u32>,
    pub deviations: Vec<String>,
    /// Review verdict, if a review gate was applied.
    pub review_verdict: Option<String>,
    /// Human-readable message.
    pub message: Option<String>,
}

impl PlanOperationResult {
    pub fn from_execution(plan: &AgentPlan, outcome: &PlanExecutionOutcome) -> Self {
        Self {
            plan_id: outcome.plan_id.clone(),
            run_id: plan.run_id.clone(),
            operation: "execute".to_string(),
            success: outcome.success,
            status: plan.status,
            steps_completed: Some(outcome.steps_completed),
            steps_failed: Some(outcome.steps_failed),
            deviations: outcome.deviations.clone(),
            review_verdict: None,
            message: None,
        }
    }
}

// ── CompactionSummary ─────────────────────────────────────────────────

/// A compacted context summary used when conversation context grows beyond
/// the token budget. Preserves critical state: active proposals, unresolved
/// tool observations, and redaction metadata.
///
/// Sensitive content is summarized under the privacy policy and never stored
/// in plain text within a cloud-safe summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionSummary {
    /// Unique identifier for this compaction.
    pub id: String,
    /// The run this compaction belongs to.
    pub run_id: String,
    /// Compacted summary of the conversation so far.
    pub conversation_summary: String,
    /// IDs of proposals that were active (pending) at compaction time.
    pub active_proposal_ids: Vec<String>,
    /// Number of unresolved tool observations included.
    pub unresolved_observation_count: u32,
    /// Summarized tool observations that were unresolved.
    pub unresolved_observation_summaries: Vec<CompactedObservation>,
    /// Whether sensitive content was redacted or summarized.
    pub sensitive_content_redacted: bool,
    /// Which fields or categories were redacted/summarized.
    pub redacted_fields: Vec<String>,
    /// Human-readable description of the redaction policy applied.
    pub redaction_policy: String,
    /// Number of tokens in the original context before compaction.
    pub original_token_estimate: usize,
    /// Number of tokens in the compacted summary.
    pub compacted_token_estimate: usize,
    /// When the compaction was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// A compacted representation of an unresolved tool observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactedObservation {
    pub tool_name: String,
    pub summary: String,
    pub pending_action: String,
    pub risk_level: String,
}

impl CompactionSummary {
    pub fn new(
        run_id: impl Into<String>,
        conversation_summary: impl Into<String>,
        original_tokens: usize,
        compacted_tokens: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            run_id: run_id.into(),
            conversation_summary: conversation_summary.into(),
            active_proposal_ids: Vec::new(),
            unresolved_observation_count: 0,
            unresolved_observation_summaries: Vec::new(),
            sensitive_content_redacted: false,
            redacted_fields: Vec::new(),
            redaction_policy: String::new(),
            original_token_estimate: original_tokens,
            compacted_token_estimate: compacted_tokens,
            created_at: chrono::Utc::now(),
        }
    }

    /// Add an active proposal ID that was preserved through compaction.
    pub fn with_active_proposal(mut self, proposal_id: impl Into<String>) -> Self {
        self.active_proposal_ids.push(proposal_id.into());
        self
    }

    /// Add an unresolved observation summary.
    pub fn with_observation(mut self, obs: CompactedObservation) -> Self {
        self.unresolved_observation_count += 1;
        self.unresolved_observation_summaries.push(obs);
        self
    }

    /// Mark sensitive content as redacted with the given policy.
    pub fn with_redaction(mut self, policy: impl Into<String>, fields: Vec<String>) -> Self {
        self.sensitive_content_redacted = true;
        self.redaction_policy = policy.into();
        self.redacted_fields = fields;
        self
    }

    /// Check whether this summary preserves active proposals.
    pub fn has_active_proposals(&self) -> bool {
        !self.active_proposal_ids.is_empty()
    }

    /// Check whether this summary preserves unresolved observations.
    pub fn has_unresolved_observations(&self) -> bool {
        self.unresolved_observation_count > 0
    }
}

#[cfg(test)]
mod compaction_tests {
    use super::*;

    #[test]
    fn test_compaction_summary_preserves_active_proposals() {
        let summary = CompactionSummary::new("run-001", "Conversation so far", 5000, 800)
            .with_active_proposal("proposal-1")
            .with_active_proposal("proposal-2");

        assert!(summary.has_active_proposals());
        assert_eq!(summary.active_proposal_ids.len(), 2);
        assert_eq!(summary.active_proposal_ids[0], "proposal-1");
        assert_eq!(summary.active_proposal_ids[1], "proposal-2");
    }

    #[test]
    fn test_compaction_summary_preserves_unresolved_observations() {
        let summary = CompactionSummary::new("run-001", "Summary", 5000, 800)
            .with_observation(CompactedObservation {
                tool_name: "web.search".into(),
                summary: "Search returned 5 results".into(),
                pending_action: "Needs user to select result".into(),
                risk_level: "low".into(),
            })
            .with_observation(CompactedObservation {
                tool_name: "file.read".into(),
                summary: "File read partially".into(),
                pending_action: "File too large, needs chunking".into(),
                risk_level: "low".into(),
            });

        assert!(summary.has_unresolved_observations());
        assert_eq!(summary.unresolved_observation_count, 2);
        assert_eq!(summary.unresolved_observation_summaries.len(), 2);
        assert_eq!(
            summary.unresolved_observation_summaries[0].tool_name,
            "web.search"
        );
        assert_eq!(
            summary.unresolved_observation_summaries[1].tool_name,
            "file.read"
        );
    }

    #[test]
    fn test_compaction_summary_redacts_sensitive_fields() {
        let summary = CompactionSummary::new("run-001", "Summary with PII", 5000, 600)
            .with_redaction(
                "PII and LifeModel fields redacted",
                vec![
                    "life_model.identity.name".into(),
                    "life_model.identity.values".into(),
                    "memory.raw_content".into(),
                ],
            );

        assert!(summary.sensitive_content_redacted);
        assert_eq!(summary.redacted_fields.len(), 3);
        assert!(summary
            .redacted_fields
            .contains(&"life_model.identity.name".into()));
        assert!(summary.redaction_policy.contains("PII"));
    }

    #[test]
    fn test_compaction_summary_token_estimates() {
        let summary = CompactionSummary::new(
            "run-001",
            "Compacted context",
            10000, // original tokens
            1200,  // compacted tokens
        );

        assert_eq!(summary.original_token_estimate, 10000);
        assert_eq!(summary.compacted_token_estimate, 1200);
        assert!(
            summary.compacted_token_estimate < summary.original_token_estimate,
            "compacted should be smaller than original"
        );
    }

    #[test]
    fn test_compaction_summary_default_no_proposals_or_observations() {
        let summary = CompactionSummary::new("run-001", "Plain summary", 3000, 500);

        assert!(!summary.has_active_proposals());
        assert!(!summary.has_unresolved_observations());
        assert!(!summary.sensitive_content_redacted);
        assert!(summary.redacted_fields.is_empty());
    }

    #[test]
    fn test_compaction_summary_round_trip_serialization() {
        let summary = CompactionSummary::new("run-001", "Test summary", 5000, 800)
            .with_active_proposal("prop-1")
            .with_observation(CompactedObservation {
                tool_name: "web.search".into(),
                summary: "Search done".into(),
                pending_action: "Review results".into(),
                risk_level: "low".into(),
            })
            .with_redaction("redacted for cloud", vec!["life_model".into()]);

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("run-001"));
        assert!(json.contains("prop-1"));
        assert!(json.contains("web.search"));
        assert!(json.contains("life_model"));

        let deserialized: CompactionSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, summary.id);
        assert_eq!(
            deserialized.active_proposal_ids,
            summary.active_proposal_ids
        );
        assert_eq!(deserialized.unresolved_observation_count, 1);
        assert!(deserialized.sensitive_content_redacted);
    }
}

// ── AgentSpec & SubAgentSpec ───────────────────────────────────────────

/// An agent's role in the framework.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRoleKind {
    Main,
    Planner,
    CodebaseExplorer,
    MemoryCurator,
    LifeModelGuardian,
    Reviewer,
    Custom(String),
}

impl std::fmt::Display for AgentRoleKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRoleKind::Main => write!(f, "main"),
            AgentRoleKind::Planner => write!(f, "planner"),
            AgentRoleKind::CodebaseExplorer => write!(f, "codebase_explorer"),
            AgentRoleKind::MemoryCurator => write!(f, "memory_curator"),
            AgentRoleKind::LifeModelGuardian => write!(f, "lifemodel_guardian"),
            AgentRoleKind::Reviewer => write!(f, "reviewer"),
            AgentRoleKind::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// Delegation mode for sub-agent dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationMode {
    /// Sub-agent is invoked as a tool from the main agent while the main
    /// agent waits for the result. Tool policy applied.
    CallAsTool,
    /// Sub-agent reviews a plan/output/patch and returns structured feedback.
    Review,
    /// Multiple sub-agents dispatched in parallel (not in P3-1).
    Parallel,
    /// Sub-agent takes over control ownership (last to implement).
    Handoff,
}

impl std::fmt::Display for DelegationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DelegationMode::CallAsTool => write!(f, "call_as_tool"),
            DelegationMode::Review => write!(f, "review"),
            DelegationMode::Parallel => write!(f, "parallel"),
            DelegationMode::Handoff => write!(f, "handoff"),
        }
    }
}

/// AgentSpec privacy policy — governs what data may leave the local device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyPolicy {
    /// All data must stay local; no cloud calls permitted.
    LocalOnly,
    /// Summarized context may be sent to cloud providers.
    SummaryOnly,
    /// Full context may be sent to cloud providers under user consent.
    CloudAllowed,
}

impl std::fmt::Display for PrivacyPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrivacyPolicy::LocalOnly => write!(f, "local_only"),
            PrivacyPolicy::SummaryOnly => write!(f, "summary_only"),
            PrivacyPolicy::CloudAllowed => write!(f, "cloud_allowed"),
        }
    }
}

/// Canonical specification of an agent's identity, permissions, and runtime
/// constraints. Used for both the main agent and sub-agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpec {
    /// Unique identifier for this spec.
    pub id: String,
    /// The agent's role in the framework.
    pub role: AgentRoleKind,
    /// Human-readable name.
    pub name: String,
    /// What this agent is designed to do.
    pub purpose: String,
    /// PromptBlock IDs that this agent uses (referenced from PromptStack).
    pub prompt_block_ids: Vec<String>,
    /// Tool names this agent is allowed to call. Empty means use role defaults.
    pub allowed_tools: Vec<String>,
    /// Tool names this agent is explicitly forbidden from calling.
    pub denied_tools: Vec<String>,
    /// Whether this agent can touch LifeModel context.
    pub can_access_lifemodel: bool,
    /// Whether this agent can access MemoryEvidence.
    pub can_access_memory_evidence: bool,
    /// Whether this agent can generate proposals.
    pub can_generate_proposals: bool,
    /// Maximum steps in the agent loop.
    pub max_steps: u32,
    /// Maximum tool calls.
    pub max_tool_calls: u32,
    /// Timeout in seconds.
    pub timeout_seconds: u64,
    /// ID of the output schema to enforce (referenced from PromptStack).
    pub output_schema_id: Option<String>,
    /// Whether this spec describes a read-only agent.
    pub read_only: bool,
    /// Privacy policy governing cloud data exposure.
    #[serde(default = "default_privacy_policy")]
    pub privacy_policy: PrivacyPolicy,
    /// Whether this spec is active.
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

fn default_privacy_policy() -> PrivacyPolicy {
    PrivacyPolicy::LocalOnly
}

impl AgentSpec {
    pub fn new(role: AgentRoleKind, name: impl Into<String>, purpose: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            role,
            name: name.into(),
            purpose: purpose.into(),
            prompt_block_ids: Vec::new(),
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            can_access_lifemodel: false,
            can_access_memory_evidence: false,
            can_generate_proposals: false,
            max_steps: 5,
            max_tool_calls: 3,
            timeout_seconds: 60,
            output_schema_id: None,
            read_only: false,
            active: true,
            privacy_policy: PrivacyPolicy::LocalOnly,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }

    pub fn with_denied_tools(mut self, tools: Vec<String>) -> Self {
        self.denied_tools = tools;
        self
    }

    pub fn with_read_only(mut self) -> Self {
        self.read_only = true;
        self.can_generate_proposals = false;
        self.denied_tools.extend(vec![
            "memory.propose_write".into(),
            "memory.propose_archive".into(),
            "file.write_proposal".into(),
            "life_model.propose_patch".into(),
        ]);
        self
    }

    pub fn with_lifemodel_access(mut self) -> Self {
        self.can_access_lifemodel = true;
        self
    }

    pub fn with_privacy_policy(mut self, policy: PrivacyPolicy) -> Self {
        self.privacy_policy = policy;
        self
    }

    pub fn with_memory_evidence(mut self) -> Self {
        self.can_access_memory_evidence = true;
        self
    }

    pub fn with_output_schema(mut self, schema_id: impl Into<String>) -> Self {
        self.output_schema_id = Some(schema_id.into());
        self
    }

    /// Check if a tool is allowed by this spec.
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        if self.denied_tools.iter().any(|t| t == tool_name) {
            return false;
        }
        if self.allowed_tools.is_empty() {
            return true; // no allowlist = all non-denied tools allowed
        }
        self.allowed_tools.iter().any(|t| t == tool_name)
    }

    /// Check if a tool is denied by this spec.
    pub fn is_tool_denied(&self, tool_name: &str) -> bool {
        self.denied_tools.iter().any(|t| t == tool_name)
    }

    /// Create the default main AgentSpec with a stable id for store bootstrapping.
    pub fn default_main_spec() -> Self {
        Self::new(
            AgentRoleKind::Main,
            "OpenLife Main Agent",
            "LifeModel-governed personal agent",
        )
        .with_lifemodel_access()
        .with_memory_evidence()
        .with_id("main.default".to_string())
    }

    /// Set a specific id on this spec (useful for store bootstrapping).
    pub fn with_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }
}

/// Structured error returned by AgentSpecStore operations.
#[derive(Debug, Clone)]
pub enum AgentSpecStoreError {
    NotFound(String),
    AlreadyExists(String),
    /// The requested spec exists but its role kind does not match the
    /// operation's requirements (e.g. set_default_main_spec on a Planner spec).
    InvalidRole { spec_id: String, role: AgentRoleKind },
    Store(String),
}

impl std::fmt::Display for AgentSpecStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentSpecStoreError::NotFound(id) => write!(f, "AgentSpec not found: {}", id),
            AgentSpecStoreError::AlreadyExists(id) => write!(f, "AgentSpec already exists: {}", id),
            AgentSpecStoreError::InvalidRole { spec_id, role } => write!(
                f,
                "AgentSpec {} has role {:?}, not valid for this operation",
                spec_id, role
            ),
            AgentSpecStoreError::Store(msg) => write!(f, "AgentSpec store error: {}", msg),
        }
    }
}

impl std::error::Error for AgentSpecStoreError {}

impl From<anyhow::Error> for AgentSpecStoreError {
    fn from(e: anyhow::Error) -> Self {
        AgentSpecStoreError::Store(e.to_string())
    }
}

impl Default for AgentSpec {
    fn default() -> Self {
        Self::new(
            AgentRoleKind::Main,
            "OpenLife Main Agent",
            "LifeModel-governed personal agent",
        )
        .with_lifemodel_access()
        .with_memory_evidence()
    }
}

/// A sub-agent specification that wraps an AgentSpec with delegation
/// constraints and parent-child linkage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentSpec {
    /// Wrapped agent specification.
    pub spec: AgentSpec,
    /// How this sub-agent is invoked.
    pub delegation_mode: DelegationMode,
    /// ID of the parent run (for trace linkage).
    pub parent_run_id: Option<String>,
    /// Maximum time the parent will wait for this sub-agent.
    pub deadline_seconds: u64,
    /// Whether to inherit the parent's tool allowlist.
    pub inherit_tool_policy: bool,
    /// Whether to inherit the parent's context (LifeModel, memory, etc.).
    pub inherit_context: bool,
    /// Whether to apply isolated context policy (default: true).
    pub isolated_context: bool,
    /// Maximum output tokens for this sub-agent.
    pub max_output_tokens: Option<u32>,
}

impl SubAgentSpec {
    pub fn new(spec: AgentSpec, delegation_mode: DelegationMode) -> Self {
        Self {
            spec,
            delegation_mode,
            parent_run_id: None,
            deadline_seconds: 120,
            inherit_tool_policy: false,
            inherit_context: false,
            isolated_context: true,
            max_output_tokens: None,
        }
    }

    pub fn with_parent_run(mut self, run_id: impl Into<String>) -> Self {
        self.parent_run_id = Some(run_id.into());
        self
    }

    pub fn with_deadline(mut self, seconds: u64) -> Self {
        self.deadline_seconds = seconds;
        self
    }

    pub fn with_inherited_tools(mut self) -> Self {
        self.inherit_tool_policy = true;
        self
    }

    pub fn with_inherited_context(mut self) -> Self {
        self.inherit_context = true;
        self.isolated_context = false;
        self
    }
}

#[cfg(test)]
mod agent_spec_tests {
    use super::*;

    #[test]
    fn test_agent_spec_default_main() {
        let spec = AgentSpec::default();
        assert_eq!(spec.role, AgentRoleKind::Main);
        assert!(spec.can_access_lifemodel);
        assert!(spec.can_access_memory_evidence);
        assert!(spec.active);
        assert_eq!(spec.privacy_policy, PrivacyPolicy::LocalOnly);
    }

    #[test]
    fn test_privacy_policy_serde_round_trip() {
        let spec = AgentSpec::default().with_privacy_policy(PrivacyPolicy::CloudAllowed);
        let json = serde_json::to_string(&spec).unwrap();
        let parsed: AgentSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.privacy_policy, PrivacyPolicy::CloudAllowed);

        let summary = AgentSpec::default().with_privacy_policy(PrivacyPolicy::SummaryOnly);
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("summary_only"));
        let parsed2: AgentSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed2.privacy_policy, PrivacyPolicy::SummaryOnly);
    }

    #[test]
    fn test_privacy_policy_display() {
        assert_eq!(PrivacyPolicy::LocalOnly.to_string(), "local_only");
        assert_eq!(PrivacyPolicy::SummaryOnly.to_string(), "summary_only");
        assert_eq!(PrivacyPolicy::CloudAllowed.to_string(), "cloud_allowed");
    }

    // ── P6-4: AgentSpec tool policy tests ─────────────────────────────

    #[test]
    fn test_agentspec_allowed_read_tool_executes() {
        let spec = AgentSpec::default().with_allowed_tools(vec!["life_model.read".into()]);
        assert!(spec.is_tool_allowed("life_model.read"));
    }

    #[test]
    fn test_agentspec_denied_tool_blocked() {
        let spec = AgentSpec::default().with_denied_tools(vec!["web.search".into()]);
        assert!(!spec.is_tool_allowed("web.search"));
        // Other tools still allowed when allowed_tools is empty.
        assert!(spec.is_tool_allowed("life_model.read"));
    }

    #[test]
    fn test_agentspec_deny_overrides_allow() {
        let spec = AgentSpec::default()
            .with_allowed_tools(vec!["file.read".into()])
            .with_denied_tools(vec!["file.read".into()]);
        assert!(!spec.is_tool_allowed("file.read"));
    }

    #[test]
    fn test_agentspec_empty_allowlist_allows_all() {
        let spec = AgentSpec::default();
        assert!(spec.is_tool_allowed("anything.here"));
    }

    #[test]
    fn test_agent_spec_read_only_blocks_writes() {
        let spec = AgentSpec::new(AgentRoleKind::Planner, "Planner", "Task decomposition")
            .with_allowed_tools(vec![
                "life_model.read".into(),
                "web.search".into(),
                "file.read".into(),
            ])
            .with_read_only();

        assert!(spec.read_only);
        assert!(!spec.can_generate_proposals);
        assert!(spec.is_tool_allowed("life_model.read"));
        assert!(spec.is_tool_allowed("web.search"));
        assert!(spec.is_tool_denied("file.write_proposal"));
        assert!(!spec.is_tool_allowed("file.write_proposal"));
    }

    #[test]
    fn test_agent_spec_tool_allow_deny() {
        let spec = AgentSpec::new(AgentRoleKind::CodebaseExplorer, "Explorer", "Read files")
            .with_allowed_tools(vec!["file.read".into(), "web.search".into()])
            .with_denied_tools(vec!["life_model.propose_patch".into()]);

        assert!(spec.is_tool_allowed("file.read"));
        assert!(spec.is_tool_allowed("web.search"));
        assert!(!spec.is_tool_allowed("memory.search")); // not in allowlist
        assert!(spec.is_tool_denied("life_model.propose_patch"));
        assert!(!spec.is_tool_allowed("life_model.propose_patch")); // denied takes precedence
    }

    #[test]
    fn test_sub_agent_spec_serialization() {
        let spec = AgentSpec::new(AgentRoleKind::Planner, "Planner", "Plan complex tasks")
            .with_read_only()
            .with_output_schema("agent_plan_v1");

        let sub = SubAgentSpec::new(spec, DelegationMode::CallAsTool)
            .with_parent_run("parent-run-001")
            .with_deadline(60);

        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("call_as_tool"));
        assert!(json.contains("parent-run-001"));
        assert!(json.contains("Planner"));
        assert!(json.contains("agent_plan_v1"));

        let deserialized: SubAgentSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.delegation_mode, DelegationMode::CallAsTool);
        assert_eq!(deserialized.parent_run_id, Some("parent-run-001".into()));
        assert_eq!(deserialized.spec.role, AgentRoleKind::Planner);
        assert!(deserialized.isolated_context);
    }

    #[test]
    fn test_delegation_modes_serialize_deserialize() {
        let modes = [
            DelegationMode::CallAsTool,
            DelegationMode::Review,
            DelegationMode::Parallel,
            DelegationMode::Handoff,
        ];

        for mode in &modes {
            let json = serde_json::to_string(mode).unwrap();
            let parsed: DelegationMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, parsed);
        }
    }

    #[test]
    fn test_agent_role_kind_display() {
        assert_eq!(AgentRoleKind::Main.to_string(), "main");
        assert_eq!(AgentRoleKind::Planner.to_string(), "planner");
        assert_eq!(
            AgentRoleKind::CodebaseExplorer.to_string(),
            "codebase_explorer"
        );
        assert_eq!(AgentRoleKind::MemoryCurator.to_string(), "memory_curator");
        assert_eq!(
            AgentRoleKind::LifeModelGuardian.to_string(),
            "lifemodel_guardian"
        );
        assert_eq!(AgentRoleKind::Reviewer.to_string(), "reviewer");
        assert_eq!(
            AgentRoleKind::Custom("my-agent".into()).to_string(),
            "my-agent"
        );
    }

    #[test]
    fn test_sub_agent_default_isolated_context() {
        let spec = AgentSpec::new(AgentRoleKind::Reviewer, "Reviewer", "Review outputs");
        let sub = SubAgentSpec::new(spec, DelegationMode::Review);

        assert!(sub.isolated_context);
        assert!(!sub.inherit_context);
        assert!(!sub.inherit_tool_policy);
    }

    #[test]
    fn test_sub_agent_with_inherited_context() {
        let spec = AgentSpec::default();
        let sub = SubAgentSpec::new(spec, DelegationMode::CallAsTool)
            .with_inherited_context()
            .with_inherited_tools();

        assert!(!sub.isolated_context);
        assert!(sub.inherit_context);
    }

    // ── P6-2: AgentTask contract tests ─────────────────────────────────

    #[test]
    fn test_agent_task_round_trip_serde() {
        let task = AgentTask::new(AgentTaskKind::Conversation, "session-1")
            .with_user_text("hello")
            .with_agent_spec("spec-1")
            .with_requires_plan()
            .with_privacy(PrivacyPolicy::SummaryOnly);

        assert_eq!(task.agent_spec_id, Some("spec-1".to_string()));
        assert!(task.requires_plan);
        assert_eq!(task.privacy_policy, Some(PrivacyPolicy::SummaryOnly));
        assert_eq!(task.initiator, "user");
        assert_eq!(task.status, AgentTaskStatus::Pending);
        assert!(!task.id.is_empty());
    }

    #[test]
    fn test_agent_task_serializes_to_camelcase() {
        let task = AgentTask::new(AgentTaskKind::Planning, "sess-2").with_agent_spec("spec-2");
        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("agentSpecId"));
        assert!(json.contains("spec-2"));
        assert!(!json.contains("agent_spec_id"));
    }

    #[test]
    fn test_agent_task_round_trips_layer_l1() {
        let mut task = AgentTask::new(AgentTaskKind::Conversation, "sess-4");
        task.layer = crate::layer_router::Layer::L1;
        let json = serde_json::to_string(&task).unwrap();
        let parsed: AgentTask = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed.layer, crate::layer_router::Layer::L1));
    }

    #[test]
    fn test_agent_task_round_trips_layer_l2() {
        let mut task = AgentTask::new(AgentTaskKind::Conversation, "sess-5");
        task.layer = crate::layer_router::Layer::L2;
        let json = serde_json::to_string(&task).unwrap();
        let parsed: AgentTask = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed.layer, crate::layer_router::Layer::L2));
    }

    #[test]
    fn test_agent_task_deserializes_missing_layer_as_l3() {
        let json = r#"{"id":"t1","kind":"conversation","sessionId":"s",
            "userText":"","messages":[],"initiator":"user",
            "requiresPlan":false,"status":"pending"}"#;
        let task: AgentTask = serde_json::from_str(json).unwrap();
        assert!(matches!(task.layer, crate::layer_router::Layer::L3));
    }

    #[test]
    fn test_agent_task_workspace_and_privacy_optional() {
        let task = AgentTask::new(AgentTaskKind::Conversation, "sess-3");
        assert_eq!(task.workspace_scope, None);
        assert_eq!(task.privacy_policy, None);
        assert!(!task.requires_plan);
    }
}
