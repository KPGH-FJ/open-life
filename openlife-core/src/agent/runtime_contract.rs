use crate::agent::action_executor::ActionExecutionContext;
use crate::agent::agent_loop::{AgentLoopConfig, AgentLoopResult};
use crate::agent::types::{AgentAction, AgentExecutionBudget, AgentObservation, AgentTask};
use crate::agent::RuntimeHSPacket;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGuidanceConsumptionMode {
    #[default]
    Disabled,
    ExplicitRuntime,
}

impl RuntimeGuidanceConsumptionMode {
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::ExplicitRuntime)
    }
}

/// Thin input boundary shared by current Direct/Layered/ReAct adapters.
///
/// This is intentionally a contract layer, not a RuntimeStrategy abstraction.
/// Future maturation work can derive LifeEvent candidates from the same input
/// without making raw task data accepted HS truth.
#[derive(Debug, Clone)]
pub struct RuntimeInput {
    pub task: AgentTask,
    pub life_model_compat: LifeModel,
    pub memory_context: Option<String>,
    pub tools_prompt: String,
    pub hs_packet: Option<RuntimeHSPacket>,
    pub guidance_consumption_mode: RuntimeGuidanceConsumptionMode,
    pub execution_budget: AgentExecutionBudget,
}

impl RuntimeInput {
    pub fn from_agent_task(
        task: AgentTask,
        life_model_compat: LifeModel,
        memory_context: Option<String>,
        tools_prompt: impl Into<String>,
        hs_packet: Option<RuntimeHSPacket>,
        execution_budget: AgentExecutionBudget,
    ) -> Self {
        Self {
            task,
            life_model_compat,
            memory_context,
            tools_prompt: tools_prompt.into(),
            hs_packet,
            guidance_consumption_mode: RuntimeGuidanceConsumptionMode::Disabled,
            execution_budget,
        }
    }

    pub fn with_guidance_consumption_mode(mut self, mode: RuntimeGuidanceConsumptionMode) -> Self {
        self.guidance_consumption_mode = mode;
        self
    }

    pub fn new_chat(
        session_id: impl Into<String>,
        user_text: impl Into<String>,
        life_model_compat: LifeModel,
        tools_prompt: impl Into<String>,
    ) -> Self {
        let user_text = user_text.into();
        Self::from_agent_task(
            AgentTask {
                kind: crate::agent::AgentTaskKind::Conversation,
                session_id: session_id.into(),
                messages: vec![ChatMessage {
                    role: "user".into(),
                    content: user_text.clone(),
                }],
                user_text,
                layer: crate::layer_router::Layer::L2,
            },
            life_model_compat,
            None,
            tools_prompt,
            None,
            AgentExecutionBudget::default(),
        )
    }

    pub fn agent_runtime_params(&self) -> AgentRuntimeParams<'_> {
        AgentRuntimeParams {
            task: &self.task,
            life_model: &self.life_model_compat,
            tools_prompt: &self.tools_prompt,
            memory_context: self.memory_context.clone(),
            hs_packet: self.hs_packet.clone(),
            guidance_consumption_mode: self.guidance_consumption_mode,
        }
    }

    pub fn agent_loop_config(&self) -> AgentLoopConfig {
        AgentLoopConfig {
            max_steps: self.execution_budget.max_steps,
            max_tool_calls: self.execution_budget.max_tool_calls,
            timeout_seconds: self.execution_budget.timeout_seconds,
            allow_writes: self.execution_budget.allow_writes,
            allow_cloud: self.execution_budget.allow_cloud,
            ..AgentLoopConfig::default()
        }
    }

    pub fn attach_hs_packet_to_action_context<'a>(
        &'a self,
        mut context: ActionExecutionContext<'a>,
    ) -> ActionExecutionContext<'a> {
        if let Some(packet) = self.hs_packet.as_ref() {
            context.hs_runtime_packet = Some(packet);
        }
        context
    }

    /// Contract-level tool intent is explicit only. A broad catalog in
    /// tools_prompt is a capability surface and must not imply writes.
    pub fn inferred_tool_requirements_from_contract(&self) -> Vec<String> {
        Vec::new()
    }
}

pub struct AgentRuntimeParams<'a> {
    pub task: &'a AgentTask,
    pub life_model: &'a LifeModel,
    pub tools_prompt: &'a str,
    pub memory_context: Option<String>,
    pub hs_packet: Option<RuntimeHSPacket>,
    pub guidance_consumption_mode: RuntimeGuidanceConsumptionMode,
}

/// Candidate event shape for the future maturation loop.
///
/// RuntimeOutput only carries these drafts. Persisting them into EvidenceStore,
/// LifeModel-HS assets, or the compatibility LifeModel must happen in a later
/// governed maturation path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeEventDraft {
    pub event_type: String,
    pub summary: String,
    #[serde(default)]
    pub source_run_id: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl LifeEventDraft {
    pub fn new(event_type: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            summary: summary.into(),
            source_run_id: None,
            metadata: serde_json::Value::Null,
        }
    }

    pub fn with_source_run_id(mut self, source_run_id: impl Into<String>) -> Self {
        self.source_run_id = Some(source_run_id.into());
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOutput {
    #[serde(default)]
    pub run_id: Option<String>,
    pub user_output: String,
    #[serde(default)]
    pub actions: Vec<AgentAction>,
    #[serde(default)]
    pub observations: Vec<AgentObservation>,
    #[serde(default)]
    pub proposal_ids: Vec<String>,
    #[serde(default)]
    pub life_event_candidates: Vec<LifeEventDraft>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl RuntimeOutput {
    pub fn empty(user_output: impl Into<String>) -> Self {
        Self {
            user_output: user_output.into(),
            ..Self::default()
        }
    }

    pub fn from_agent_loop_result(result: AgentLoopResult) -> Self {
        Self {
            run_id: Some(result.run.id.clone()),
            user_output: result.final_response,
            actions: result.run.actions,
            observations: result.run.observations,
            proposal_ids: result.run.generated_proposals,
            life_event_candidates: Vec::new(),
            warnings: result.run.warnings,
        }
    }
}
