use super::ActionExecutionResult;
use super::AgentActionRequest;

impl super::ActionExecutor {
    pub fn execute_memory_write(
        &self,
        request: AgentActionRequest,
    ) -> anyhow::Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "memory_write must be submitted as a MemoryWrite proposal before persistence",
        ))
    }

    pub fn execute_memory_archive(
        &self,
        request: AgentActionRequest,
    ) -> anyhow::Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "memory_archive must be submitted as a MemoryArchive proposal before persistence",
        ))
    }
}
