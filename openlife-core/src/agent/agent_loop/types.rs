use crate::agent::action_executor::{ActionContext, AgentActionRequest};
use crate::agent::prompt_stack::PromptBlockRegistry;
use crate::agent::runtime::AgentRuntimeOutput;
use crate::agent::types::{AgentLoopStatusUpdate, AgentObservation, AgentRun};
use crate::agent::types::{AgentSpec, AgentTask, PrivacyPolicy};
use crate::life_model::LifeModel;
use crate::privacy::PrivacyEngine;

/// Result of running the agent loop.
#[derive(Debug, Clone)]
pub struct AgentLoopResult {
    pub run: AgentRun,
    pub final_response: String,
    pub stop_reason: String,
    pub tool_call_count: u32,
    pub step_count: u32,
    pub status_updates: Vec<AgentLoopStatusUpdate>,
}

/// Result of a single step in the agent loop.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub stop_reason: String,
    pub final_response: String,
    pub should_continue: bool,
    pub tool_call_count_delta: u32,
    pub observations: Vec<AgentObservation>,
    pub status_updates: Vec<AgentLoopStatusUpdate>,
}

/// Context for executing a single step of the agent loop.
pub struct StepContext<'a> {
    pub task: &'a AgentTask,
    pub life_model: &'a LifeModel,
    pub tools_prompt: &'a str,
    pub memory_context: Option<String>,
    pub privacy_engine: PrivacyEngine,
    pub privacy_policy: PrivacyPolicy,
    pub agent_spec: &'a AgentSpec,
    pub prompt_registry: &'a PromptBlockRegistry,
    pub action_ctx: &'a ActionContext,
    pub run: &'a mut AgentRun,
    pub tool_call_count: u32,
}

pub(crate) struct GeneratedAgentResponse {
    pub runtime_output: AgentRuntimeOutput,
    pub reply: String,
    pub model_route: Option<crate::agent::types::ModelRouteTrace>,
}

pub struct ParsedAgentReply {
    pub final_text: String,
    pub actions: Vec<AgentActionRequest>,
    /// True if the model generated a JSON-like response that failed to parse.
    /// When true, the caller should attempt a one-shot repair round.
    pub json_parse_failed: bool,
}

pub fn preview_text(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        format!("{}...", text.chars().take(max_len).collect::<String>())
    }
}
