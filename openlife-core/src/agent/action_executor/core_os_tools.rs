use anyhow::Result;
use serde_json::Value;

use super::helpers::ToolCallInternalResult;
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

use super::ActionExecutionContext;

impl super::ActionExecutor {
    /// Execute a Core OS tool with real data from LifeModel.
    pub fn execute_core_os_tool(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ActionExecutionContext<'_>,
    ) -> Result<ToolCallInternalResult> {
        let output = match tool_name {
            "life_model.read" => {
                let life_model = ctx.life_model.ok_or_else(|| {
                    anyhow::anyhow!(
                        "LifeModel not available in execution context for core_os tool '{}'",
                        tool_name
                    )
                })?;
                serde_json::to_string_pretty(&life_model)
                    .unwrap_or_else(|_| "{\"error\": \"serialization failed\"}".to_string())
            }
            "goal.read" => {
                let life_model = ctx.life_model.ok_or_else(|| {
                    anyhow::anyhow!(
                        "LifeModel not available in execution context for core_os tool '{}'",
                        tool_name
                    )
                })?;
                serde_json::to_string_pretty(&life_model.goals)
                    .unwrap_or_else(|_| "{\"error\": \"serialization failed\"}".to_string())
            }
            "state.read" => {
                let life_model = ctx.life_model.ok_or_else(|| {
                    anyhow::anyhow!(
                        "LifeModel not available in execution context for core_os tool '{}'",
                        tool_name
                    )
                })?;
                serde_json::to_string_pretty(&life_model.state)
                    .unwrap_or_else(|_| "{\"error\": \"serialization failed\"}".to_string())
            }
            "memory.search" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");

                if let Some(memory_store) = ctx.memory_store {
                    match memory_store.search_text_memories(None, query, 10) {
                        Ok(hits) => {
                            let results: Vec<_> = hits
                                .into_iter()
                                .map(|hit| {
                                    serde_json::json!({
                                        "content": hit.chunk.content,
                                        "source": hit.chunk.source,
                                        "relevance": hit.relevance_score,
                                        "tier": hit.chunk.tier,
                                    })
                                })
                                .collect();
                            serde_json::json!({
                                "status": "success",
                                "query": query,
                                "hits": results,
                                "count": results.len()
                            })
                            .to_string()
                        }
                        Err(e) => serde_json::json!({
                            "status": "error",
                            "reason": format!("Search failed: {}", e),
                            "hits": []
                        })
                        .to_string(),
                    }
                } else {
                    serde_json::json!({
                        "status": "unavailable",
                        "reason": "MemoryStore not available in execution context",
                        "hits": []
                    })
                    .to_string()
                }
            }
            "tool.list_available" => {
                let manifests = ctx.registry.list_manifests();
                let tools: Vec<_> = manifests
                    .into_iter()
                    .map(|m| {
                        // Determine execution status
                        let execution_status = if !m.enabled {
                            "disabled"
                        } else if m.declarative_only {
                            "declarative_only"
                        } else if m.requires_confirmation
                            || m.risk_level == "high"
                            || m.capabilities.iter().any(|c| {
                                matches!(
                                    c.as_str(),
                                    "write"
                                        | "filesystem"
                                        | "memory"
                                        | "lifemodel"
                                        | "external_side_effect"
                                )
                            })
                        {
                            "needs_permission"
                        } else {
                            "executable"
                        };

                        serde_json::json!({
                            "name": m.name,
                            "description": m.description,
                            "source": m.source.to_string(),
                            "action_type": m.action_type,
                            "risk_level": m.risk_level,
                            "capabilities": m.capabilities,
                            "execution_status": execution_status,
                            "enabled": m.enabled,
                            "declarative_only": m.declarative_only,
                            "requires_confirmation": m.requires_confirmation,
                        })
                    })
                    .collect();
                serde_json::json!({ "tools": tools }).to_string()
            }
            "proposal.list" => {
                if let Some(store) = ctx.proposal_store {
                    let proposals = store
                        .list_pending_proposals(20)
                        .map_err(|e| anyhow::anyhow!("Failed to list proposals: {}", e))?;
                    serde_json::to_string(&proposals)
                        .unwrap_or_else(|_| "{\"error\":\"serialization failed\"}".to_string())
                } else {
                    serde_json::json!({
                        "status": "unavailable",
                        "reason": "ProposalStore not available in execution context",
                        "proposals": []
                    })
                    .to_string()
                }
            }
            "agent_run.lookup" => {
                let run_id = args
                    .get("run_id")
                    .or_else(|| args.get("runId"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if run_id.is_empty() {
                    serde_json::json!({
                        "status": "error",
                        "reason": "agent_run.lookup requires run_id"
                    })
                    .to_string()
                } else if let Some(store) = ctx.agent_run_store {
                    match store
                        .get_run(run_id)
                        .map_err(|e| anyhow::anyhow!("Failed to lookup agent run: {}", e))?
                    {
                        Some(run) => serde_json::to_string(&run)
                            .unwrap_or_else(|_| "{\"error\":\"serialization failed\"}".to_string()),
                        None => serde_json::json!({
                            "status": "not_found",
                            "run_id": run_id
                        })
                        .to_string(),
                    }
                } else {
                    serde_json::json!({
                        "status": "unavailable",
                        "reason": "AgentRunStore not available in execution context"
                    })
                    .to_string()
                }
            }
            "life_model.propose_patch" => self
                .create_core_os_proposal(
                    ctx,
                    ProposalType::LifeModelUpdate,
                    args.get("path")
                        .and_then(Value::as_str)
                        .unwrap_or("life_model"),
                    args.clone(),
                    "Agent proposed a LifeModel patch via Core OS tool.",
                    RiskLevel::High,
                )?
                .to_string(),
            "memory.propose_write" => self
                .create_core_os_proposal(
                    ctx,
                    ProposalType::MemoryWrite,
                    "memory.candidates",
                    args.clone(),
                    "Agent proposed a MemoryWrite via Core OS tool.",
                    RiskLevel::Medium,
                )?
                .to_string(),
            "memory.propose_archive" => self
                .create_core_os_proposal(
                    ctx,
                    ProposalType::MemoryArchive,
                    "memory.archive",
                    args.clone(),
                    "Agent proposed a MemoryArchive via Core OS tool.",
                    RiskLevel::Medium,
                )?
                .to_string(),
            _ => {
                return Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(format!("Unknown core_os tool: {}", tool_name)),
                });
            }
        };

        Ok(ToolCallInternalResult {
            success: true,
            output: Some(output),
            error: None,
        })
    }

    pub fn create_core_os_proposal(
        &self,
        ctx: &ActionExecutionContext<'_>,
        proposal_type: ProposalType,
        affected_path: &str,
        after: Value,
        reason: &str,
        risk: RiskLevel,
    ) -> anyhow::Result<Value> {
        let store = ctx
            .proposal_store
            .ok_or_else(|| anyhow::anyhow!("ProposalStore not available in execution context"))?;
        let proposal = AgentProposal::new(
            proposal_type,
            affected_path,
            after,
            reason,
            0.8,
            risk,
            ProposalSource::Manual,
        );
        let proposal_id = proposal.id.clone();
        store.create_proposal(&proposal)?;
        Ok(serde_json::json!({
            "status": "proposal_created",
            "proposal_id": proposal_id,
            "proposal_type": proposal.proposal_type.to_string(),
            "affected_path": proposal.affected_path,
        }))
    }
}
