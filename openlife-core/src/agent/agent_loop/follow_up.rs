use super::context::{USER_REQUEST_END, USER_REQUEST_START};
use super::AgentLoop;
use crate::agent::types::{AgentObservation, AgentTask};
use crate::llm::ChatMessage;

impl AgentLoop {
    pub(crate) fn build_follow_up_messages(
        &self,
        task: &AgentTask,
        assistant_reply: &str,
        observations: &[AgentObservation],
        tools_prompt: &str,
    ) -> Vec<ChatMessage> {
        let mut messages = task.messages.clone();
        messages.push(ChatMessage {
            role: "assistant".into(),
            content: assistant_reply.into(),
        });

        // Build structured follow-up with: task goal, available tools, observations
        let mut follow_up = String::new();

        // Remind the model of the original task (strip boundary markers if present)
        let clean_user_text = task
            .user_text
            .replace(USER_REQUEST_START, "")
            .replace(USER_REQUEST_END, "")
            .trim()
            .to_string();
        follow_up.push_str(&format!(
            "[系统] 继续完成用户的原始请求：\"{}\"\n\n",
            clean_user_text
        ));

        // Include observations from tool executions
        if !observations.is_empty() {
            follow_up.push_str("工具执行结果：\n");
            for (idx, obs) in observations.iter().enumerate() {
                follow_up.push_str(&format!("[{}] {}\n", idx + 1, obs.content));
            }
            follow_up.push('\n');
        }

        // Remind available tools for next step
        if !tools_prompt.is_empty() {
            follow_up.push_str("下一步可用工具：\n");
            follow_up.push_str(tools_prompt);
            follow_up.push('\n');
        }

        follow_up.push_str("请继续使用工具或提供最终回答。");

        messages.push(ChatMessage {
            role: "user".into(),
            content: follow_up,
        });

        messages
    }
}
