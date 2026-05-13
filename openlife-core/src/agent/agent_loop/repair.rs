use anyhow::Result;

use super::context::AgentLoopContext;
use super::types::ParsedAgentReply;
use super::AgentLoop;
use crate::agent::action_executor::ActionContext;
use crate::agent::types::AgentRun;
use crate::llm::ChatMessage;

/// One-shot JSON self-repair prompt sent to the model when its previous
/// response was not valid JSON. Bilingual + schema-first for best results
/// across different models.
pub(crate) const SELF_REPAIR_PROMPT: &str = r#"Your previous response was not valid JSON for tool calling.
请只输出一个合法 JSON object，不要 markdown，不要解释。

Allowed shape:
{"final": "reply to user", "actions": [{"name": "tool_name", "arguments": {}}], "thought_summary": "brief reasoning", "warnings": []}
If no tools needed: {"final": "reply to user"}

Original request: "#;

impl AgentLoop {
    /// Attempt a one-shot JSON self-repair when the model produces malformed JSON.
    /// Injects a bilingual, schema-first correction prompt and regenerates once.
    /// Records warnings on the run for observability.
    /// Returns a fresh ParsedAgentReply from the regenerated response, or the
    /// original `failed_parsed` (with `json_parse_failed: true`) if repair also fails.
    pub(crate) async fn try_json_self_repair(
        &self,
        actx: &AgentLoopContext<'_>,
        action_ctx: &ActionContext,
        run: &mut AgentRun,
        tool_call_count: &mut u32,
    ) -> Result<ParsedAgentReply> {
        let mut repair_task = actx.task.clone();
        repair_task.messages.push(ChatMessage {
            role: "system".into(),
            content: format!("{}{}", SELF_REPAIR_PROMPT, actx.task.user_text),
        });

        let repair_actx = AgentLoopContext {
            task: &repair_task,
            life_model: actx.life_model,
            tools_prompt: actx.tools_prompt,
            memory_context: actx.memory_context.clone(),
            privacy_engine: actx.privacy_engine.clone(),
            privacy_policy: actx.privacy_policy,
            agent_spec: actx.agent_spec,
            prompt_registry: actx.prompt_registry,
        };

        match self.generate_response(&repair_actx, &run.id).await {
            Ok(repaired_gen) => {
                let parsed =
                    self.parse_agent_reply(&repaired_gen.reply, action_ctx, run, tool_call_count)?;
                if !parsed.json_parse_failed {
                    run.warnings
                        .push("JSON format self-repair succeeded".into());
                } else {
                    run.warnings.push(
                        "JSON format self-repair also failed, continuing with raw reply".into(),
                    );
                }
                Ok(parsed)
            }
            Err(e) => {
                run.warnings
                    .push(format!("JSON format self-repair generation failed: {}", e));
                Ok(ParsedAgentReply {
                    final_text: "[self-repair failed]".into(),
                    actions: Vec::new(),
                    json_parse_failed: true,
                })
            }
        }
    }
}
