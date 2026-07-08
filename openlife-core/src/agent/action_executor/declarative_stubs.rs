use crate::agent::review_workflow::{
    DurableWriteRequest, DurableWriteSource, DurableWriteSubject, ReviewWorkflow,
};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use serde_json::Value;

use super::ActionExecutionContext;
use super::ActionExecutionResult;
use super::AgentActionRequest;

impl super::ActionExecutor {
    /// For manifest-only provider-gated tools, create a Proposal instead of blocking.
    #[allow(clippy::too_many_arguments)]
    pub fn create_declarative_stub_proposal(
        &self,
        request: &AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        tool_name: &str,
        args: &Value,
        proposal_type: ProposalType,
        category: &str,
        reason: &str,
    ) -> Option<anyhow::Result<ActionExecutionResult>> {
        let proposal_store = ctx.proposal_store?;

        // Build after payload from tool arguments
        let after = match tool_name {
            "calendar.propose_event" => serde_json::json!({
                "title": args.get("title").and_then(Value::as_str).unwrap_or("Untitled Event"),
                "scheduled_at": args.get("scheduled_at").or_else(|| args.get("date")).and_then(Value::as_str).unwrap_or(""),
                "description": args.get("description").and_then(Value::as_str).unwrap_or(""),
                "tool": tool_name,
                "raw_args": args,
            }),
            "email.propose_draft" => serde_json::json!({
                "to": args.get("to").and_then(Value::as_str).unwrap_or(""),
                "subject": args.get("subject").and_then(Value::as_str).unwrap_or(""),
                "body": args.get("body").and_then(Value::as_str).unwrap_or(""),
                "tool": tool_name,
                "raw_args": args,
            }),
            _ => args.clone(),
        };

        let affected_path = format!("{}.{}", category, tool_name);
        let mut proposal = AgentProposal::new(
            proposal_type,
            &affected_path,
            after,
            reason,
            0.8,
            RiskLevel::High,
            ProposalSource::Manual,
        );

        if let Some(ref run_id) = request.source_run_id {
            proposal.run_id = Some(run_id.clone());
        }

        let outcome = match ReviewWorkflow::new(proposal_store).submit(
            DurableWriteRequest::from_agent_proposal(
                DurableWriteSource::ToolPermission,
                DurableWriteSubject::from_proposal_type(proposal.proposal_type),
                proposal,
                "Tool proposal is pending Review Center approval.",
            ),
        ) {
            Ok(outcome) => outcome,
            Err(e) => {
                eprintln!(
                    "[warn] Failed to create {} Proposal for {}: {}",
                    proposal_type, tool_name, e
                );
                return None;
            }
        };

        let result = self.build_proposal_required_action(
            request.clone(),
            &format!(
                "{}: created {} Proposal (id: {})",
                tool_name,
                proposal_type,
                outcome.proposal_id()
            ),
        );

        Some(Ok(result))
    }
}
