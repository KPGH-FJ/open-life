use crate::agent::prompt_stack::PromptBlockRegistry;
use crate::agent::types::{AgentSpec, AgentTask, PrivacyPolicy};
use crate::life_model::LifeModel;
use crate::privacy::PrivacyEngine;

/// Bundles the shared task/life-model/tools/privacy/memory/AgentSpec parameters
/// that flow through the entire agent loop, reducing argument counts below clippy limits.
pub(crate) struct AgentLoopContext<'a> {
    pub task: &'a AgentTask,
    pub life_model: &'a LifeModel,
    pub tools_prompt: &'a str,
    pub memory_context: Option<String>,
    pub privacy_engine: PrivacyEngine,
    /// AgentSpec privacy policy governing cloud data exposure.
    pub privacy_policy: PrivacyPolicy,
    /// Resolved AgentSpec for governed execution (prompt blocks, context policy).
    pub agent_spec: &'a AgentSpec,
    /// PromptBlockRegistry for prompt block resolution.
    pub prompt_registry: &'a PromptBlockRegistry,
}

/// Boundary markers for prompt injection defense.
/// Wrapped around user content to clearly delimit untrusted input from system instructions.
pub(crate) const USER_REQUEST_START: &str = "[BEGIN USER REQUEST]";
pub(crate) const USER_REQUEST_END: &str = "[END USER REQUEST]";

pub(crate) fn should_hold_streaming_reply(pending: &str) -> bool {
    let trimmed = pending.trim_start();
    trimmed.is_empty() || trimmed.starts_with('{') || trimmed.starts_with("```")
}

/// Wrap user content in boundary markers to mitigate prompt injection.
/// Affects messages with role == "user" and the standalone user_text field.
pub(crate) fn wrap_user_content(task: &mut AgentTask) {
    for msg in task.messages.iter_mut().filter(|m| m.role == "user") {
        msg.content = format!(
            "{}\n{}\n{}",
            USER_REQUEST_START, msg.content, USER_REQUEST_END
        );
    }
    if !task.user_text.is_empty() && !task.user_text.starts_with(USER_REQUEST_START) {
        task.user_text = format!(
            "{}\n{}\n{}",
            USER_REQUEST_START, task.user_text, USER_REQUEST_END
        );
    }
}
