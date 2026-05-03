use super::ActionExecutionResult;
use super::AgentActionRequest;

impl super::ActionExecutor {
    pub fn execute_life_model_patch(
        &self,
        request: AgentActionRequest,
    ) -> anyhow::Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "life_model_patch must be submitted as a LifeModel proposal before persistence",
        ))
    }
}
