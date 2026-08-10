use crate::agent::action_executor::ActionExecutionContext;
use crate::agent::agent_loop::{AgentLoopConfig, AgentLoopResult};
use crate::agent::types::{AgentAction, AgentExecutionBudget, AgentObservation, AgentTask};
use crate::llm::ChatMessage;
use crate::llm::{ProviderPolicyAuthorization, ProviderPolicyProvenanceRef};
use serde::{Deserialize, Serialize};

/// Narrow, typed policy result consumed by generic Agent runtime paths.
///
/// It deliberately contains no heuristic, guidance, LifeModel, or prompt
/// content. Product owners evaluate policy before entering the runtime and
/// pass only the provider capability, its provenance, and the one tool-policy
/// fact the action boundary needs.
#[derive(Debug, Clone)]
pub struct RuntimePolicyContext {
    provider_authorization: ProviderPolicyAuthorization,
    policy_provenance_refs: Vec<ProviderPolicyProvenanceRef>,
    external_write_requires_proposal: bool,
}

impl RuntimePolicyContext {
    pub fn new(
        provider_authorization: ProviderPolicyAuthorization,
        mut policy_provenance_refs: Vec<ProviderPolicyProvenanceRef>,
        external_write_requires_proposal: bool,
    ) -> Self {
        policy_provenance_refs.sort();
        policy_provenance_refs.dedup();
        Self {
            provider_authorization,
            policy_provenance_refs,
            external_write_requires_proposal,
        }
    }

    pub fn fail_closed() -> Self {
        let authorization = ProviderPolicyAuthorization::local_only_fail_closed(
            crate::llm::ProviderLocalOnlyReason::MissingCanonicalPolicy,
        );
        let route_digest = crate::agent::metadata_safe::metadata_safe_text_digest(&format!(
            "{}:{}:{:?}",
            authorization.decision_id(),
            authorization.policy_version(),
            authorization.data_route(),
        ))
        .1;
        let provenance = vec![ProviderPolicyProvenanceRef::new(
            crate::llm::ProviderPolicyProvenanceKind::FailClosedRouteDecision,
            authorization.decision_id(),
            route_digest,
        )];
        Self::new(authorization, provenance, true)
    }

    pub fn from_scheduled_claim(claim: &crate::tasks::ScheduledTaskClaim) -> anyhow::Result<Self> {
        let authorization = ProviderPolicyAuthorization::from_scheduled_claim(claim)?;
        let provenance = vec![ProviderPolicyProvenanceRef::new(
            crate::llm::ProviderPolicyProvenanceKind::ScheduledRouteDecision,
            authorization.decision_id(),
            &claim.provider_grant().policy_decision_digest,
        )];
        Ok(Self::new(authorization, provenance, true))
    }

    pub fn provider_authorization(&self) -> &ProviderPolicyAuthorization {
        &self.provider_authorization
    }

    pub fn policy_provenance_refs(&self) -> &[ProviderPolicyProvenanceRef] {
        &self.policy_provenance_refs
    }

    pub fn external_write_requires_proposal(&self) -> bool {
        self.external_write_requires_proposal
    }
}

/// Thin input boundary shared by current Direct/Layered/ReAct adapters.
///
/// This is intentionally a contract layer, not a RuntimeStrategy abstraction.
/// The contract carries task, Agent Memory context, tools, and an evaluated
/// Policy result. LifeModel personalization is owned by explicit canonical-v2
/// product adapters and is never inferred from a legacy YAML compatibility
/// model here.
#[derive(Debug, Clone)]
pub struct RuntimeInput {
    pub task: AgentTask,
    pub source_run_id: Option<String>,
    pub memory_context: Option<String>,
    pub tools_prompt: String,
    pub policy_context: RuntimePolicyContext,
    pub execution_budget: AgentExecutionBudget,
}

impl RuntimeInput {
    pub fn from_agent_task(
        task: AgentTask,
        memory_context: Option<String>,
        tools_prompt: impl Into<String>,
        policy_context: RuntimePolicyContext,
        execution_budget: AgentExecutionBudget,
    ) -> Self {
        Self {
            task,
            source_run_id: None,
            memory_context,
            tools_prompt: tools_prompt.into(),
            policy_context,
            execution_budget,
        }
    }

    pub fn with_source_run_id(mut self, source_run_id: impl Into<String>) -> Self {
        let source_run_id = source_run_id.into();
        if !source_run_id.trim().is_empty() {
            self.source_run_id = Some(source_run_id);
        }
        self
    }

    pub fn new_chat(
        session_id: impl Into<String>,
        user_text: impl Into<String>,
        tools_prompt: impl Into<String>,
        policy_context: RuntimePolicyContext,
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
                layer: crate::layer::Layer::L2,
            },
            None,
            tools_prompt,
            policy_context,
            AgentExecutionBudget::default(),
        )
    }

    pub fn agent_runtime_params(&self) -> AgentRuntimeParams<'_> {
        AgentRuntimeParams {
            task: &self.task,
            tools_prompt: &self.tools_prompt,
            memory_context: self.memory_context.clone(),
            policy_context: self.policy_context.clone(),
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

    pub fn attach_policy_to_action_context<'a>(
        &'a self,
        mut context: ActionExecutionContext<'a>,
    ) -> ActionExecutionContext<'a> {
        context.external_write_requires_proposal =
            self.policy_context.external_write_requires_proposal();
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
    pub tools_prompt: &'a str,
    pub memory_context: Option<String>,
    pub policy_context: RuntimePolicyContext,
}

/// Candidate event shape for the future maturation loop.
///
/// RuntimeOutput only carries these drafts. Persisting them into Agent Memory
/// or a canonical LifeModel proposal must happen through its owning governed
/// product path.
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
