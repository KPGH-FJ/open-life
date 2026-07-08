use std::sync::Arc;

use crate::main_chat_react_tool_selection::{
    resolve_main_chat_mcp_read_target, MainChatReactActionPlan,
};
use crate::main_chat_runtime_support::{
    append_main_chat_agent_transcript, enqueue_main_chat_agent_action, transition_main_chat_action,
};
use crate::AppState;
use openlife_core::agent::{
    DurableWriteRequest, DurableWriteSource, DurableWriteSubject, ReviewWorkflow,
};

pub(crate) async fn create_main_chat_agent_proposal(
    state: &Arc<AppState>,
    task_session_id: &str,
    strategy: openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy,
    user_text: &str,
) -> Result<openlife_core::agent::AgentProposal, String> {
    use openlife_core::agent::main_chat_agent_v1::{
        ExecutionQueueStatus, ExecutionTranscriptEntryKind, MainChatAgentStrategy,
    };
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

    let knowledge_asset_edit = strategy == MainChatAgentStrategy::LifeModelProposal
        && main_chat_knowledge_asset_edit_target(user_text).is_some();
    let (proposal_type, affected_path, reason, risk_level, after) = if knowledge_asset_edit {
        let target = main_chat_knowledge_asset_edit_target(user_text).unwrap_or("AGENTS.md");
        (
            ProposalType::LifeModelUpdate,
            format!("knowledge_asset.{target}"),
            "User requested a proposal-first knowledge asset edit from Main Chat.".to_string(),
            RiskLevel::Medium,
            serde_json::json!({
                "assetId": target,
                "assetKind": "knowledge_markdown",
                "source": "main_chat_agent_v1",
                "originatingTaskSessionId": task_session_id,
                "proposedDiff": {
                    "operation": "append_note",
                    "target": target,
                    "summary": "Add bounded capability evidence note to the knowledge asset.",
                    "unifiedDiff": format!("--- {target}\n+++ {target}\n@@\n+Bounded capability evidence note: keep knowledge assets as context, not policy override."),
                },
                "directKnowledgeFileWrite": false,
                "requiresReviewCenterApproval": true,
            }),
        )
    } else {
        match strategy {
            MainChatAgentStrategy::MemoryProposal => (
                ProposalType::MemoryWrite,
                "memory.pending.chat_conversation".to_string(),
                "User explicitly requested a memory update from Main Chat.".to_string(),
                RiskLevel::Medium,
                serde_json::json!({
                    "content": user_text,
                    "source": "main_chat_agent_v1",
                    "originatingTaskSessionId": task_session_id,
                    "directMemoryWrite": false,
                }),
            ),
            _ => (
                ProposalType::LifeModelUpdate,
                "lifemodel.pending.chat_conversation".to_string(),
                "User explicitly requested a LifeModel-affecting update from Main Chat."
                    .to_string(),
                RiskLevel::High,
                serde_json::json!({
                    "requestedChange": user_text,
                    "source": "main_chat_agent_v1",
                    "originatingTaskSessionId": task_session_id,
                    "directLifeModelWrite": false,
                }),
            ),
        }
    };
    let mut proposal = AgentProposal::new(
        proposal_type,
        &affected_path,
        after,
        &reason,
        0.86,
        risk_level,
        ProposalSource::ChatConversation,
    );
    proposal.source_detail = Some(format!("main_chat_agent_task_session:{task_session_id}"));
    crate::life_model_write_gateway::stamp_lifemodel_proposal_base_hash_with_state(
        state,
        &mut proposal,
    )
    .await?;

    let mut internal_transcript = Vec::new();
    let queue_action_type = if knowledge_asset_edit {
        "knowledge.propose_edit"
    } else {
        "proposal.create"
    };
    let queue_description = if knowledge_asset_edit {
        "Create a Mailbox proposal for a knowledge asset edit."
    } else {
        "Create a Mailbox proposal from Main Chat."
    };
    let queued = enqueue_main_chat_agent_action(
        state,
        task_session_id,
        queue_action_type,
        queue_description,
        &mut internal_transcript,
    )
    .await?;
    let store_arc = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "Proposal store not available".to_string())?;
    let store = store_arc.lock().await;
    let outcome = ReviewWorkflow::new(&store)
        .submit(
            DurableWriteRequest::from_agent_proposal(
                DurableWriteSource::MainChat,
                DurableWriteSubject::from_proposal_type(proposal.proposal_type),
                proposal,
                "Main Chat proposal is pending Review Center approval.",
            )
            .with_evidence_refs(vec![format!("main_chat_task_session:{task_session_id}")]),
        )
        .map_err(|err| format!("create proposal failed: {err}"))?;
    let proposal = outcome.proposal;
    drop(store);
    transition_main_chat_action(state, &queued.id, ExecutionQueueStatus::Executing, None).await?;
    transition_main_chat_action(
        state,
        &queued.id,
        ExecutionQueueStatus::Observed,
        Some(serde_json::json!({
            "proposalId": proposal.id,
            "proposalType": proposal.proposal_type,
            "knowledgeAssetId": if knowledge_asset_edit {
                proposal.after.get("assetId").and_then(serde_json::Value::as_str)
            } else {
                None
            },
            "proposedDiffPresent": proposal.after.get("proposedDiff").is_some(),
            "directKnowledgeFileWrite": proposal.after.get("directKnowledgeFileWrite").and_then(serde_json::Value::as_bool).unwrap_or(false),
            "directWritesExecuted": false,
        })),
    )
    .await?;
    transition_main_chat_action(state, &queued.id, ExecutionQueueStatus::Completed, None).await?;
    let _ = append_main_chat_agent_transcript(
        state,
        Some(task_session_id),
        ExecutionTranscriptEntryKind::Action,
        "Proposal create action observed; proposal remains pending Review Center approval.",
        serde_json::json!({
            "actionId": queued.id,
            "proposalId": proposal.id,
            "actionType": queue_action_type,
            "knowledgeAssetId": if knowledge_asset_edit {
                proposal.after.get("assetId").and_then(serde_json::Value::as_str)
            } else {
                None
            },
            "proposedDiffPresent": proposal.after.get("proposedDiff").is_some(),
            "directKnowledgeFileWrite": proposal.after.get("directKnowledgeFileWrite").and_then(serde_json::Value::as_bool).unwrap_or(false),
            "directWritesExecuted": false,
        }),
    )
    .await;
    Ok(proposal)
}

fn main_chat_knowledge_asset_edit_target(user_text: &str) -> Option<&'static str> {
    let lower = user_text.to_ascii_lowercase();
    if !lower.contains("knowledge asset") && !lower.contains(".md") {
        return None;
    }
    if lower.contains("agents.md") {
        Some("AGENTS.md")
    } else if lower.contains("soul.md") {
        Some("SOUL.md")
    } else if lower.contains("user.md") {
        Some("USER.md")
    } else if lower.contains("memory.md") {
        Some("MEMORY.md")
    } else {
        Some("AGENTS.md")
    }
}

pub(crate) async fn attach_main_chat_tool_permission_proposal_metadata(
    state: &Arc<AppState>,
    task_session_id: &str,
    plan: &MainChatReactActionPlan,
    blocker_reason: Option<&str>,
    mut metadata: serde_json::Value,
) -> Result<
    (
        serde_json::Value,
        Vec<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>,
    ),
    String,
> {
    use openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind;
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

    if plan.queue_action_type != "mcp.read_only" {
        return Ok((metadata, Vec::new()));
    }

    let (manifest, target_arguments) = {
        let registry = state.mcp_registry.lock().await;
        let resolution = resolve_main_chat_mcp_read_target(&registry, plan);
        if !resolution.resolved {
            return Ok((metadata, Vec::new()));
        }
        let manifest = registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.name == resolution.target || manifest.id == resolution.target)
            .ok_or_else(|| {
                format!(
                    "resolved MCP read target missing from registry: {}",
                    resolution.target
                )
            })?;
        (manifest, resolution.arguments)
    };

    let source = openlife_core::agent::action_executor::helpers::canonical_tool_source(&manifest);
    let (input_length_bytes, input_hash) =
        openlife_core::agent::metadata_safe::metadata_safe_value_digest(&target_arguments);
    let risk_level = match manifest.risk_level.to_ascii_lowercase().as_str() {
        "high" => RiskLevel::High,
        "low" => RiskLevel::Low,
        _ => RiskLevel::Medium,
    };
    let affected_path = format!("tool_permission.{}.{}", source, manifest.name);
    let after = serde_json::json!({
        "permission_action": "grant",
        "permission": "allow_once",
        "tool_name": manifest.name.clone(),
        "source": source,
        "risk_level": manifest.risk_level.clone(),
        "action_type": manifest.action_type.clone(),
        "capabilities": manifest.capabilities.clone(),
        "originatingTaskSessionId": task_session_id,
        "blocked_action": {
            "action_type": plan.queue_action_type,
            "target": plan.target,
            "resolved_target": manifest.name.clone(),
            "input_hash": input_hash,
            "input_length_bytes": input_length_bytes,
        },
        "reason": blocker_reason.unwrap_or("tool_permission_required"),
        "auto_generated": true,
        "mainChatAgentV1": true,
        "directWritesExecuted": false,
    });
    let proposal_store_arc = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "Proposal store not available".to_string())?;
    let existing_proposal_id = metadata
        .get("proposalId")
        .or_else(|| metadata.get("proposal_id"))
        .or_else(|| {
            metadata
                .get("structuredResult")
                .and_then(|structured| structured.get("proposalId"))
        })
        .or_else(|| {
            metadata
                .get("structuredResult")
                .and_then(|structured| structured.get("proposal_id"))
        })
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let proposal = {
        let proposal_store = proposal_store_arc.lock().await;
        let reusable_existing_id = if let Some(existing_proposal_id) = existing_proposal_id {
            proposal_store
                .get_proposal(&existing_proposal_id)
                .map_err(|err| format!("load existing ToolPermission proposal failed: {err}"))?
                .filter(|proposal| {
                    proposal.proposal_type == ProposalType::ToolPermission
                        && proposal.status == openlife_core::agent::ProposalStatus::Pending
                })
                .map(|proposal| proposal.id)
        } else {
            None
        };
        let mut proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            &affected_path,
            after.clone(),
            "Allow the pending Main Chat MCP read action to continue after explicit review.",
            0.72,
            risk_level,
            ProposalSource::ChatConversation,
        );
        proposal.source_detail = Some(format!("main_chat_agent_task_session:{task_session_id}"));
        let outcome = ReviewWorkflow::new(&proposal_store)
            .submit(
                DurableWriteRequest::from_agent_proposal(
                    DurableWriteSource::MainChat,
                    DurableWriteSubject::ToolPermission,
                    proposal,
                    "Main Chat tool permission proposal is pending Review Center approval.",
                )
                .with_existing_proposal_id(reusable_existing_id)
                .with_evidence_refs(vec![format!("main_chat_task_session:{task_session_id}")]),
            )
            .map_err(|err| format!("create Main Chat ToolPermission proposal failed: {err}"))?;
        outcome.proposal
    };

    if let Some(object) = metadata.as_object_mut() {
        object.insert("proposalId".into(), serde_json::json!(proposal.id.clone()));
        object.insert(
            "proposalType".into(),
            serde_json::json!(proposal.proposal_type.to_string()),
        );
        object.insert("permissionProposalCreated".into(), serde_json::json!(true));
        object.insert("toolName".into(), serde_json::json!(manifest.name.clone()));
        object.insert("resumeReplayable".into(), serde_json::json!(true));
        object.insert("directWritesExecuted".into(), serde_json::json!(false));
    }
    if let Some(structured) = metadata
        .get_mut("structuredResult")
        .and_then(serde_json::Value::as_object_mut)
    {
        structured.insert("proposalId".into(), serde_json::json!(proposal.id.clone()));
        structured.insert("permissionProposalCreated".into(), serde_json::json!(true));
        structured.insert("directWritesExecuted".into(), serde_json::json!(false));
    }

    let transcript_entries = append_main_chat_agent_transcript(
        state,
        Some(task_session_id),
        ExecutionTranscriptEntryKind::PermissionRequest,
        "ToolPermission proposal created for pending Main Chat action.",
        serde_json::json!({
            "proposalId": proposal.id,
            "proposalType": proposal.proposal_type,
            "affectedPath": proposal.affected_path,
            "toolName": proposal.after.get("tool_name").cloned().unwrap_or(serde_json::Value::Null),
            "resumeReplayable": true,
            "directWritesExecuted": false,
        }),
    )
    .await;

    Ok((metadata, transcript_entries))
}
