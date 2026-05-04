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
            "permission.check" => {
                let target = args.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                if target.is_empty() {
                    serde_json::json!({
                        "status": "error",
                        "reason": "permission.check requires tool_name argument"
                    })
                    .to_string()
                } else {
                    let manifest = ctx
                        .registry
                        .list_manifests()
                        .into_iter()
                        .find(|m| m.name == target || m.id == target);
                    let source = args.get("source").and_then(|v| v.as_str()).unwrap_or("*");
                    let risk_level = manifest
                        .as_ref()
                        .map(|m| m.risk_level.as_str())
                        .unwrap_or("medium");
                    let action_type = manifest
                        .as_ref()
                        .map(|m| m.action_type.as_str())
                        .unwrap_or("read");
                    let caps: Vec<String> = manifest
                        .as_ref()
                        .map(|m| m.capabilities.clone())
                        .unwrap_or_default();
                    match ctx
                        .permission_store
                        .peek(target, source, risk_level, action_type, &caps)
                    {
                        Ok(decision) => serde_json::to_string(&decision).unwrap_or_default(),
                        Err(e) => serde_json::json!({
                            "status": "error",
                            "reason": format!("权限查询失败: {}", e)
                        })
                        .to_string(),
                    }
                }
            }
            "permission.request" => {
                let target = args.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                if target.is_empty() {
                    serde_json::json!({
                        "status": "error",
                        "reason": "permission.request requires tool_name argument"
                    })
                    .to_string()
                } else {
                    let manifest = ctx
                        .registry
                        .list_manifests()
                        .into_iter()
                        .find(|m| m.name == target || m.id == target);
                    let source = manifest
                        .as_ref()
                        .map(super::helpers::canonical_tool_source)
                        .unwrap_or_else(|| "builtin".to_string());
                    let risk = manifest
                        .as_ref()
                        .map(|m| m.risk_level.clone())
                        .unwrap_or_else(|| "medium".to_string());
                    self.create_core_os_proposal(
                        ctx,
                        ProposalType::ToolPermission,
                        &format!("tool_permission.{}.{}", source, target),
                        serde_json::json!({
                            "tool_name": target,
                            "source": source,
                            "risk_level": risk,
                            "policy": "allow_until_revoked",
                            "reason": args.get("reason").and_then(|v| v.as_str()).unwrap_or("Agent requested permission via Core OS tool"),
                        }),
                        "Agent requested tool permission via Core OS tool.",
                        if risk == "high" { RiskLevel::High } else { RiskLevel::Medium },
                    )?
                    .to_string()
                }
            }
            "permission.replay_action" => {
                let target = args.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                let action_input = args
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| args.clone());
                if target.is_empty() {
                    serde_json::json!({
                        "status": "error",
                        "reason": "permission.replay_action requires tool_name argument"
                    })
                    .to_string()
                } else if target == "permission.replay_action" {
                    serde_json::json!({
                        "status": "error",
                        "reason": "permission.replay_action cannot replay itself"
                    })
                    .to_string()
                } else {
                    // Replay the action through ActionExecutor
                    let action_request = super::AgentActionRequest {
                        action_type: "mcp_tool".into(),
                        target: target.to_string(),
                        input: serde_json::json!({ "arguments": action_input }),
                        source_run_id: None,
                        step_index: 0,
                    };
                    match self.execute(action_request, ctx) {
                        Ok(result) => serde_json::json!({
                            "status": if result.status == super::ActionExecutionStatus::Succeeded { "success" } else { "failed" },
                            "action_status": format!("{:?}", result.status),
                            "observation": result.observation.content,
                            "requires_confirmation": result.status == super::ActionExecutionStatus::NeedsConfirmation,
                        })
                        .to_string(),
                        Err(e) => serde_json::json!({
                            "status": "error",
                            "reason": format!("重放操作失败: {}", e)
                        })
                        .to_string(),
                    }
                }
            }
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
