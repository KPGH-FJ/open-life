use std::sync::Arc;

use openlife_core::agent::{
    AgentExecutionBudget, AgentTask, AgentTaskKind, PlanDraft, PlanExecuteInput,
    PlanExecuteProductContract, PlanExecuteProductScenario, PlanExecuteService, RuntimeInput,
};
use openlife_core::layer::Layer;
use openlife_core::llm::ChatMessage;

use crate::main_chat_policy_runtime::build_chat_runtime_policy_context;
use crate::AppState;

/// Produces bounded plan content for the canonical Task Plan item. This helper
/// has no lifecycle store and never materializes a separate PlanExecute task.
pub(crate) async fn draft_plan_for_main_chat(
    state: &Arc<AppState>,
    source_run_id: &str,
    source_chat_session_id: &str,
    task_text: &str,
    life_model_hints: Vec<openlife_core::agent::PlanExecuteLifeModelHint>,
) -> Result<PlanDraft, String> {
    let scenario = PlanExecuteProductScenario::WeeklyPlanning;
    let contract = PlanExecuteProductContract::weekly_planning();
    let tools_prompt = state.mcp_registry.lock().await.tools_prompt();
    let task_text = if task_text.trim().is_empty() {
        "Plan this week using confirmed context."
    } else {
        task_text
    };
    let task = AgentTask {
        kind: AgentTaskKind::Planning,
        session_id: source_chat_session_id.to_string(),
        user_text: task_text.into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: task_text.into(),
        }],
        layer: Layer::L2,
    };
    let policy_context = build_chat_runtime_policy_context(state, &task, &tools_prompt)?;
    let max_steps = contract.max_step_count;
    let runtime_input = RuntimeInput::from_agent_task(
        task,
        None,
        tools_prompt,
        policy_context,
        AgentExecutionBudget {
            max_steps: max_steps as u32,
            max_tool_calls: 0,
            timeout_seconds: 30,
            allow_cloud: false,
            allow_writes: false,
        },
    )
    .with_source_run_id(source_run_id.to_string());
    let plan_input = PlanExecuteInput::from_runtime_input(
        runtime_input,
        "scenario=weekly_planning product=workspace",
        max_steps,
    )
    .with_life_model_hints(life_model_hints);
    let draft = PlanExecuteService.draft_product_plan(&plan_input, scenario);
    contract
        .evaluate_draft(&draft)
        .map_err(|report| format!("Plan draft contract blocked: {}", report.reason_code))?;
    Ok(draft)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::RiskLevel;

    #[tokio::test]
    async fn plan_draft_applies_bounded_lifemodel_goal_hint_without_session_write() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let session = draft_plan_for_main_chat(
            &state,
            "run-lifemodel-plan",
            "lifemodel-plan-hint",
            "Plan this week around OpenLife.",
            vec![openlife_core::agent::PlanExecuteLifeModelHint {
                item_id: "goal-openlife".into(),
                section: openlife_core::life_model::v2::LifeModelSectionV2::LongTermGoals,
                value: "完成 OpenLife: 让个人 Agent OS 真正可用".into(),
                selected_reason: "task keyword matches: 1".into(),
            }],
        )
        .await
        .expect("LifeModel-aware plan draft");

        assert!(session.steps[0].title.contains("OpenLife"));
        assert_eq!(session.steps[0].intent, "lifemodel_goal_alignment");
        assert!(!session.steps[0].declared_write);
        assert!(session.steps[0].tool_name.is_none());
        assert_eq!(session.steps[0].risk_level, RiskLevel::Low);
    }
}
