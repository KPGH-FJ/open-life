use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for AgentRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRunStatus::Running => write!(f, "running"),
            AgentRunStatus::Completed => write!(f, "completed"),
            AgentRunStatus::Failed => write!(f, "failed"),
            AgentRunStatus::Cancelled => write!(f, "cancelled"),
        }
    }
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
pub struct AgentAction {
    pub id: String,
    pub action_type: String,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentObservation {
    pub id: String,
    pub content: String,
    pub source: String,
    pub timestamp: DateTime<Utc>,
}

/// Error information when a run fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunError {
    pub message: String,
    pub phase: String, // "preprocess" | "model" | "stream" | "fallback" | "hermes"
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
    pub generated_proposals: Vec<String>,
    pub actions: Vec<AgentAction>,
    pub observations: Vec<AgentObservation>,
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
    LifeModelUpdate,
    MemoryUpdate,
    ToolPermission,
    GoalUpdate,
}

impl std::fmt::Display for ProposalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalType::LifeModelUpdate => write!(f, "life_model_update"),
            ProposalType::MemoryUpdate => write!(f, "memory_update"),
            ProposalType::ToolPermission => write!(f, "tool_permission"),
            ProposalType::GoalUpdate => write!(f, "goal_update"),
        }
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
    pub proposal_type: ProposalType,
    pub affected_path: String,
    pub before: Option<serde_json::Value>,
    pub after: serde_json::Value,
    pub reason: String,
    pub confidence: f32,
    pub risk_level: RiskLevel,
    pub status: ProposalStatus,
    pub source: String,
    pub source_run_id: Option<String>,
    pub source_kind: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl AgentProposal {
    pub fn new(
        proposal_type: ProposalType,
        affected_path: &str,
        after: serde_json::Value,
        reason: &str,
        confidence: f32,
        risk_level: RiskLevel,
        source: &str,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            proposal_type,
            affected_path: affected_path.to_string(),
            before: None,
            after,
            reason: reason.to_string(),
            confidence,
            risk_level,
            status: ProposalStatus::Pending,
            source: source.to_string(),
            source_run_id: None,
            source_kind: None,
            created_at: Utc::now(),
            resolved_at: None,
        }
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
