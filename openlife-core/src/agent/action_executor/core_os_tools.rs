use anyhow::Result;
use serde_json::Value;

use super::helpers::ToolCallInternalResult;
use crate::agent::review_workflow::{DurableWriteRequest, DurableWriteSource, DurableWriteSubject};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use crate::agent::{
    memory_lifecycle_category_for_candidate_kind, CanonicalMemoryFactDescriptor,
    MemoryCandidateKind, MemoryLifecycleRiskLevel, MemoryLifecycleScope,
    MemoryLifecycleSensitivity,
};
use crate::tool_execution_receipt::ToolExecutionReceiptTracker;
use crate::tool_manifest::ToolManifest;

use super::{ActionExecutionContext, AgentActionRequest};

impl super::ActionExecutor {
    /// Execute a Core OS tool with real data from LifeModel.
    pub(crate) async fn execute_core_os_tool(
        &self,
        tool_name: &str,
        args: &Value,
        outer_request: &AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        manifest: &ToolManifest,
        receipt_tracker: ToolExecutionReceiptTracker,
    ) -> Result<ToolCallInternalResult> {
        const SUPPORTED_CORE_OS_TOOLS: &[&str] = &[
            "life_model.read",
            "goal.read",
            "state.read",
            "memory.search",
            "tool.list_available",
            "proposal.list",
            "agent_run.lookup",
            "life_model.propose_patch",
            "memory.propose_write",
            "memory.propose_archive",
            "permission.check",
            "permission.request",
        ];
        if !SUPPORTED_CORE_OS_TOOLS.contains(&tool_name) {
            return Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!("Unknown core_os tool: {}", tool_name)),
            });
        }
        if matches!(tool_name, "life_model.read" | "goal.read") && ctx.life_model.is_none() {
            return Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!(
                    "LifeModel not available in execution context for core_os tool '{}'",
                    tool_name
                )),
            });
        }
        if matches!(tool_name, "goal.read" | "state.read") && ctx.canonical_state.is_none() {
            return Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!(
                    "canonical StateStore snapshot not available for core_os tool '{}'",
                    tool_name
                )),
            });
        }
        ctx.authorize_tool_dispatch(manifest, outer_request, args, &receipt_tracker)
            .await?
            .observe_local()
            .await?;
        let operation = (|| -> Result<ToolCallInternalResult> {
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
                    let canonical_state = ctx.canonical_state.ok_or_else(|| {
                        anyhow::anyhow!(
                            "canonical StateStore snapshot not available for core_os tool '{}'",
                            tool_name
                        )
                    })?;
                    serde_json::to_string_pretty(&serde_json::json!({
                        "canonicalOwners": {
                            "longTermGoals": "life_model",
                            "dailyTasks": "state_store"
                        },
                        "shortTerm": &life_model.goals.short_term,
                        "mediumTerm": &life_model.goals.medium_term,
                        "longTerm": &life_model.goals.long_term,
                        "lifeGoals": &life_model.goals.life_goals,
                        "dailyTasks": &canonical_state.daily_tasks,
                    }))
                    .unwrap_or_else(|_| "{\"error\": \"serialization failed\"}".to_string())
                }
                "state.read" => {
                    let canonical_state = ctx.canonical_state.ok_or_else(|| {
                        anyhow::anyhow!(
                            "canonical StateStore snapshot not available for core_os tool '{}'",
                            tool_name
                        )
                    })?;
                    serde_json::to_string_pretty(&serde_json::json!({
                        "canonicalOwner": "state_store",
                        "dailyTasks": &canonical_state.daily_tasks,
                        "observations": &canonical_state.observations,
                    }))
                    .unwrap_or_else(|_| "{\"error\": \"serialization failed\"}".to_string())
                }
                "memory.search" => {
                    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                    let memory_store = ctx.memory_store.ok_or_else(|| {
                        anyhow::anyhow!("memory_retrieval_degraded:memory_store_unavailable")
                    })?;
                    let hits =
                        memory_store
                            .search_text_memories(None, query, 10)
                            .map_err(|error| {
                                anyhow::anyhow!(
                                    "memory_retrieval_degraded:memory_store_query_failed:{error}"
                                )
                            })?;
                    let hits = ctx.filter_retrievable_memory_hits(hits)?;
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
                        let run = match store.get_run(run_id) {
                            Ok(run) => run,
                            Err(error) => {
                                ctx.observe_durable_store_failure("AgentRunStore", &error);
                                return Err(anyhow::anyhow!(
                                    "Failed to lookup agent run: {}",
                                    error
                                ));
                            }
                        };
                        match run {
                            Some(run) => serde_json::to_string(&run).unwrap_or_else(|_| {
                                "{\"error\":\"serialization failed\"}".to_string()
                            }),
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
                "memory.propose_write" => {
                    let (reviewed_payload, risk) = validated_memory_write_proposal(args)?;
                    self.create_core_os_proposal(
                        ctx,
                        ProposalType::MemoryWrite,
                        "memory.candidates",
                        reviewed_payload,
                        "Agent proposed a MemoryWrite via Core OS tool.",
                        risk,
                    )?
                    .to_string()
                }
                "memory.propose_archive" => self
                    .create_core_os_proposal(
                        ctx,
                        ProposalType::MemoryArchive,
                        "memory.archive",
                        validated_memory_archive_proposal(args)?,
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
                        match ctx.permission_store.peek(
                            target,
                            source,
                            risk_level,
                            action_type,
                            &caps,
                        ) {
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
                            "permission_scope_kind": "manifest_policy",
                            "tool_name": target,
                            "source": source,
                            "risk_level": risk,
                            "action_type": manifest.as_ref().map(|m| m.action_type.as_str()).unwrap_or("read"),
                            "policy": "allow_until_revoked",
                            "reason": args.get("reason").and_then(|v| v.as_str()).unwrap_or("Agent requested permission via Core OS tool"),
                        }),
                        "Agent requested tool permission via Core OS tool.",
                        if risk == "high" { RiskLevel::High } else { RiskLevel::Medium },
                    )?
                    .to_string()
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
        })();
        receipt_tracker.mark_response_observed();
        super::tool_executor::record_effect_outcome(
            &receipt_tracker,
            operation.as_ref().is_ok_and(|result| result.success),
        );
        operation
    }

    fn create_core_os_proposal(
        &self,
        ctx: &ActionExecutionContext<'_>,
        proposal_type: ProposalType,
        affected_path: &str,
        after: Value,
        reason: &str,
        risk: RiskLevel,
    ) -> anyhow::Result<Value> {
        ctx.proposal_store
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
        let outcome = ctx.submit_review_proposal(DurableWriteRequest::from_agent_proposal(
            DurableWriteSource::ToolPermission,
            DurableWriteSubject::from_proposal_type(proposal.proposal_type),
            proposal,
            "Core OS proposal is pending Review Center approval.",
        ))?;
        Ok(serde_json::json!({
            "status": "proposal_created",
            "proposal_id": outcome.proposal_id(),
            "proposal_type": outcome.proposal.proposal_type.to_string(),
            "affected_path": outcome.proposal.affected_path,
        }))
    }
}

fn validated_memory_write_proposal(args: &Value) -> anyhow::Result<(Value, RiskLevel)> {
    const MAX_MEMORY_PROPOSAL_CONTENT_BYTES: usize = 64 * 1024;

    let object = args
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("memory.propose_write requires an object payload"))?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "content" | "scope" | "category" | "candidateKind" | "candidate_kind"
        )
    }) {
        anyhow::bail!("memory.propose_write contains an unsupported field");
    }
    let content = object
        .get("content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|content| !content.is_empty() && content.len() <= MAX_MEMORY_PROPOSAL_CONTENT_BYTES)
        .ok_or_else(|| {
            anyhow::anyhow!("memory.propose_write content must contain 1..=65536 bytes")
        })?;
    let reviewed_category = object.get("category").and_then(Value::as_str);
    let candidate_kind = object
        .get("candidateKind")
        .or_else(|| object.get("candidate_kind"))
        .and_then(Value::as_str)
        .map(parse_memory_candidate_kind)
        .transpose()?
        .or_else(|| reviewed_category.and_then(candidate_kind_for_category))
        .unwrap_or(MemoryCandidateKind::SemanticUserFact);
    let expected_category = memory_lifecycle_category_for_candidate_kind(candidate_kind);
    if reviewed_category.is_some_and(|category| category != expected_category.as_str()) {
        anyhow::bail!("memory.propose_write candidate kind and category disagree");
    }
    let scope = match object.get("scope").and_then(Value::as_str) {
        None | Some("global") => MemoryLifecycleScope::Global,
        Some("workspace") => MemoryLifecycleScope::Workspace,
        Some("conversation") => MemoryLifecycleScope::Conversation,
        Some("project") => MemoryLifecycleScope::Project,
        Some(other) => anyhow::bail!("memory.propose_write has unknown scope: {other}"),
    };
    let (proposal_risk, lifecycle_risk) = if candidate_kind == MemoryCandidateKind::IdentityOrRole {
        (RiskLevel::High, MemoryLifecycleRiskLevel::IdentityValue)
    } else {
        (RiskLevel::Medium, MemoryLifecycleRiskLevel::Medium)
    };
    // Tool-generated candidates cannot prove that arbitrary model-supplied
    // content is non-sensitive. Conservatively keep them local until review.
    let fact = CanonicalMemoryFactDescriptor::from_candidate(
        content,
        candidate_kind,
        scope,
        lifecycle_risk,
        MemoryLifecycleSensitivity::Sensitive,
    )?;
    Ok((
        serde_json::json!({
            "content": fact.canonical_body,
            "scope": fact.scope,
            "category": fact.category,
            "candidateKind": candidate_kind,
            "riskLevel": fact.risk_level,
            "sensitivity": fact.sensitivity,
            "source": "core_os_tool",
        }),
        proposal_risk,
    ))
}

fn parse_memory_candidate_kind(value: &str) -> anyhow::Result<MemoryCandidateKind> {
    match value {
        "episodic_life_event" => Ok(MemoryCandidateKind::EpisodicLifeEvent),
        "semantic_user_fact" => Ok(MemoryCandidateKind::SemanticUserFact),
        "procedural_rule" => Ok(MemoryCandidateKind::ProceduralRule),
        "preference" => Ok(MemoryCandidateKind::Preference),
        "identity_or_role" => Ok(MemoryCandidateKind::IdentityOrRole),
        other => anyhow::bail!("memory.propose_write has unknown candidate kind: {other}"),
    }
}

fn candidate_kind_for_category(value: &str) -> Option<MemoryCandidateKind> {
    match value {
        "fact" => Some(MemoryCandidateKind::SemanticUserFact),
        "workflow" => Some(MemoryCandidateKind::ProceduralRule),
        "preference" => Some(MemoryCandidateKind::Preference),
        "boundary" => Some(MemoryCandidateKind::IdentityOrRole),
        _ => None,
    }
}

#[cfg(test)]
mod memory_write_contract_tests {
    use super::validated_memory_write_proposal;
    use crate::agent::types::RiskLevel;

    #[test]
    fn write_proposal_builds_one_conservative_reviewed_memory_contract() {
        let (preference, risk) = validated_memory_write_proposal(&serde_json::json!({
            "content": "User prefers concise updates.",
            "category": "preference",
            "scope": "project"
        }))
        .unwrap();
        assert_eq!(risk, RiskLevel::Medium);
        assert_eq!(preference["candidateKind"], "preference");
        assert_eq!(preference["category"], "preference");
        assert_eq!(preference["riskLevel"], "medium");
        assert_eq!(preference["sensitivity"], "sensitive");
        assert_eq!(preference["scope"], "project");

        let (identity, risk) = validated_memory_write_proposal(&serde_json::json!({
            "content": "User identifies as a founder.",
            "candidateKind": "identity_or_role",
            "category": "boundary"
        }))
        .unwrap();
        assert_eq!(risk, RiskLevel::High);
        assert_eq!(identity["riskLevel"], "identity_value");
        assert_eq!(identity["sensitivity"], "sensitive");

        for invalid in [
            serde_json::json!({"content": "", "category": "fact"}),
            serde_json::json!({
                "content": "mismatch",
                "candidateKind": "preference",
                "category": "fact"
            }),
            serde_json::json!({"content": "unknown", "category": "unknown"}),
            serde_json::json!({"content": "extra", "unreviewed": true}),
        ] {
            assert!(validated_memory_write_proposal(&invalid).is_err());
        }
    }
}

fn validated_memory_archive_proposal(args: &Value) -> anyhow::Result<Value> {
    let object = args
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("memory.propose_archive requires an object payload"))?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "owner" | "owners" | "reason"))
    {
        anyhow::bail!("memory.propose_archive contains an unsupported field");
    }
    let owner = object.get("owner");
    let owners = object.get("owners");
    if owner.is_some() == owners.is_some() {
        anyhow::bail!("memory.propose_archive requires exactly one of owner or owners");
    }
    let values = if let Some(owner) = owner {
        vec![owner]
    } else {
        let owners = owners
            .and_then(Value::as_array)
            .filter(|owners| !owners.is_empty() && owners.len() <= 200)
            .ok_or_else(|| {
                anyhow::anyhow!("memory.propose_archive owners must contain 1..=200 owners")
            })?;
        owners.iter().collect::<Vec<_>>()
    };
    let mut unique = std::collections::HashSet::new();
    let mut lifecycle_owned = 0_usize;
    let owner_count = values.len();
    for value in values {
        let owner = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("memory.propose_archive owner must be an object"))?;
        if owner.len() != 2 || !owner.contains_key("ownerKind") || !owner.contains_key("ownerId") {
            anyhow::bail!("memory.propose_archive owner requires only ownerKind and ownerId");
        }
        let owner_kind = owner
            .get("ownerKind")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("memory.propose_archive ownerKind must be a string"))?;
        let owner_id = owner
            .get("ownerId")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("memory.propose_archive ownerId must be a string"))?;
        let canonical = crate::vectors::CanonicalVectorOwnerRef::new(owner_kind, owner_id)?;
        lifecycle_owned += usize::from(canonical.kind() == "memory_lifecycle");
        if !unique.insert(canonical.source()) {
            anyhow::bail!("memory.propose_archive contains duplicate canonical owners");
        }
    }
    if lifecycle_owned != 0 && lifecycle_owned != owner_count {
        anyhow::bail!(
            "memory.propose_archive cannot mix lifecycle and MemoryStore canonical owners"
        );
    }
    if object
        .get("reason")
        .is_some_and(|reason| reason.as_str().is_none_or(|reason| reason.len() > 512))
    {
        anyhow::bail!("memory.propose_archive reason must be a bounded string");
    }
    Ok(args.clone())
}

#[cfg(test)]
mod memory_archive_contract_tests {
    use super::validated_memory_archive_proposal;

    #[test]
    fn archive_proposal_accepts_only_stable_canonical_owner_contract() {
        let valid = serde_json::json!({
            "owners": [
                { "ownerKind": "memory_lifecycle", "ownerId": "memory:one" },
                { "ownerKind": "memory_lifecycle", "ownerId": "memory:two" }
            ],
            "reason": "user reviewed archive"
        });
        assert_eq!(validated_memory_archive_proposal(&valid).unwrap(), valid);

        for invalid in [
            serde_json::json!({ "chunkIds": [1] }),
            serde_json::json!({
                "owner": { "ownerKind": "memory_lifecycle", "ownerId": "memory:one" },
                "owners": [{ "ownerKind": "knowledge_note", "ownerId": "42" }]
            }),
            serde_json::json!({
                "owner": { "ownerKind": "unsupported", "ownerId": "one" }
            }),
            serde_json::json!({
                "owners": [
                    { "ownerKind": "memory_lifecycle", "ownerId": "memory:one" },
                    { "ownerKind": "knowledge_note", "ownerId": "42" }
                ]
            }),
            serde_json::json!({
                "owner": { "ownerKind": "memory_lifecycle", "ownerId": "memory:one", "rowId": 7 }
            }),
        ] {
            assert!(validated_memory_archive_proposal(&invalid).is_err());
        }
    }
}
