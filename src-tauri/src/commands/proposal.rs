use crate::{persist_life_model, AppState};
use openlife_core::agent::{AgentProposal, ProposalStatus, ProposalType, RiskLevel};
use openlife_core::life_model::LifeModel;
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

fn proposal_store_missing() -> String {
    "Proposal store is unavailable. Please check Settings > 试用就绪检查.".to_string()
}

fn check_safe_mode(state: &Arc<AppState>) -> Result<(), String> {
    if !state.startup_warnings.is_empty() {
        return Err(format!(
            "系统处于 Safe Mode，无法应用 Proposal：{}",
            state.startup_warnings.join("；")
        ));
    }
    Ok(())
}

fn ensure_pending_or_postponed(proposal: &AgentProposal) -> Result<(), String> {
    match proposal.status {
        ProposalStatus::Pending | ProposalStatus::Postponed => Ok(()),
        ProposalStatus::Accepted => Err("该 Proposal 已经被接受，不能重复处理。".to_string()),
        ProposalStatus::Rejected => Err("该 Proposal 已经被拒绝，不能再次处理。".to_string()),
        ProposalStatus::Edited => Err("该 Proposal 已经被编辑并应用，不能重复处理。".to_string()),
    }
}

fn patch_result_for_proposal(
    proposal: &AgentProposal,
    success: bool,
    operation: &str,
    error: Option<String>,
) -> openlife_core::life_model::patch::PatchApplyResult {
    openlife_core::life_model::patch::PatchApplyResult {
        patch_id: proposal.id.clone(),
        success,
        path: proposal.affected_path.clone(),
        operation: operation.to_string(),
        error,
    }
}

fn memory_session_id(after: &Value) -> String {
    after
        .get("session_id")
        .or_else(|| after.get("sessionId"))
        .and_then(Value::as_str)
        .unwrap_or("proposal")
        .to_string()
}

fn memory_source(after: &Value) -> String {
    after
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("proposal")
        .to_string()
}

fn memory_content(after: &Value) -> Result<String, String> {
    if let Some(content) = after.get("content").and_then(Value::as_str) {
        let content = content.trim();
        if !content.is_empty() {
            return Ok(content.to_string());
        }
    }
    if let Some(content) = after.as_str() {
        let content = content.trim();
        if !content.is_empty() {
            return Ok(content.to_string());
        }
    }
    Err("MemoryWrite Proposal 缺少 after.content。".to_string())
}

fn memory_archive_ids(after: &Value) -> Result<Vec<i64>, String> {
    let value = after
        .get("chunk_ids")
        .or_else(|| after.get("chunkIds"))
        .or_else(|| after.get("ids"))
        .unwrap_or(after);

    if let Some(id) = value.as_i64() {
        return Ok(vec![id]);
    }

    if let Some(ids) = value.as_array() {
        let parsed: Vec<i64> = ids.iter().filter_map(Value::as_i64).collect();
        if !parsed.is_empty() {
            return Ok(parsed);
        }
    }

    Err("MemoryArchive Proposal 缺少 after.chunk_ids。".to_string())
}

#[allow(dead_code)]
fn set_path_value(root: &mut Value, path: &str, value: Value) -> Result<(), String> {
    let mut current = root;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            let object = current
                .as_object_mut()
                .ok_or_else(|| format!("路径 `{}` 的父节点不是对象。", path))?;
            if !object.contains_key(part) {
                return Err(format!("人生模型不包含字段路径 `{}`。", path));
            }
            object.insert(part.to_string(), value);
            return Ok(());
        }

        current = current
            .get_mut(part)
            .ok_or_else(|| format!("人生模型不包含字段路径 `{}`。", path))?;
    }
    Err("Proposal affected_path 不能为空。".to_string())
}

#[allow(dead_code)]
fn apply_life_model_value(
    model: &LifeModel,
    path: &str,
    after: Value,
) -> Result<LifeModel, String> {
    let mut value = serde_json::to_value(model).map_err(|e| e.to_string())?;
    set_path_value(&mut value, path, after)?;
    serde_json::from_value(value).map_err(|e| format!("Proposal 值无法转换为 LifeModel：{}", e))
}

async fn apply_proposal_to_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    after: Value,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    match proposal.proposal_type {
        ProposalType::LifeModelUpdate
        | ProposalType::GoalUpdate
        | ProposalType::StateUpdate
        | ProposalType::PreferenceUpdate
        | ProposalType::CapabilityUpdate => {
            let mut model = {
                let manager = state.life_model_manager.lock().await;
                manager.load().map_err(|e| e.to_string())?
            };

            // 1. Create Before Snapshot
            let _before_snapshot = {
                let vm = state.version_manager.lock().await;
                vm.snapshot_for_patch(&model, &proposal.id, "before")
                    .map_err(|e| e.to_string())?
            };

            // 2. Generate Patch from Proposal
            let path_pointer =
                openlife_core::life_model::patch::dot_to_pointer(&proposal.affected_path);
            let path_display =
                openlife_core::life_model::patch::pointer_to_display(&path_pointer, &model);

            let patch = openlife_core::life_model::patch::LifeModelPatch::from_proposal(
                &proposal.id,
                &path_pointer,
                &path_display,
                openlife_core::life_model::patch::PatchOp::Replace,
                proposal.before.clone(),
                after.clone(),
                &proposal.reason,
                proposal.confidence,
                proposal.risk_level,
                openlife_core::life_model::patch::PatchSource::BuilderReview,
            );

            // 3. Apply Patch using new engine
            let result = model.apply_patch(&patch).map_err(|e| e.to_string())?;

            if !result.success {
                return Ok(result);
            }

            // 4. Persist updated model
            persist_life_model(state, model.clone(), true).await?;

            // 5. Create After Snapshot
            let _after_snapshot = {
                let vm = state.version_manager.lock().await;
                vm.snapshot_for_patch(&model, &proposal.id, "after")
                    .map_err(|e| e.to_string())?
            };

            // 6. Save Patch to PatchStore
            if let Some(ref patch_store_arc) = state.patch_store {
                let patch_store = patch_store_arc.lock().await;
                let mut patch_to_save = patch.clone();
                patch_to_save.mark_applied();
                let _ = patch_store.create_patch(&patch_to_save);
            }

            Ok(result)
        }
        ProposalType::MemoryWrite | ProposalType::MemoryArchive => match proposal.proposal_type {
            ProposalType::MemoryWrite => {
                let content = memory_content(&after)?;
                let session_id = memory_session_id(&after);
                let source = memory_source(&after);
                let embedding_id = {
                    let (provider, openai_base, openai_key, embedding_model, embedding_enabled) = {
                        let cfg = state.config.lock().await;
                        (
                            cfg.llm.provider.clone(),
                            cfg.llm.openai_base.clone(),
                            cfg.llm.openai_key.clone(),
                            cfg.llm.embedding_model.clone(),
                            cfg.llm.embedding_enabled,
                        )
                    };
                    match openlife_core::vectors::embed_text_with_config(
                        &content,
                        &provider,
                        &openai_base,
                        &openai_key,
                        &embedding_model,
                        embedding_enabled,
                    )
                    .await
                    {
                        Ok(embedding) if !embedding.is_empty() => {
                            let store = state.vector_store.lock().await;
                            store
                                .insert(&session_id, &content, &embedding, &source)
                                .map_err(|e| e.to_string())
                                .ok()
                        }
                        Ok(_) | Err(_) => None,
                    }
                };
                {
                    let store = state.memory_store.lock().await;
                    let tags = vec![
                        "proposal".to_string(),
                        format!("proposal_id:{}", proposal.id),
                        format!("source:{}", source),
                    ];
                    store
                        .save_memory_record(
                            &session_id,
                            &content,
                            "proposal_memory",
                            &source,
                            &tags,
                            "private",
                            embedding_id,
                        )
                        .map_err(|e| e.to_string())?;
                }
                Ok(patch_result_for_proposal(
                    proposal,
                    true,
                    "memory_write",
                    None,
                ))
            }
            ProposalType::MemoryArchive => {
                let ids = memory_archive_ids(&after)?;
                let archived = {
                    let store = state.vector_store.lock().await;
                    store.archive_chunks(&ids).map_err(|e| e.to_string())?
                };
                if archived == 0 {
                    return Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "memory_archive",
                        Some("没有匹配到可归档的 active memory chunk。".to_string()),
                    ));
                }
                Ok(patch_result_for_proposal(
                    proposal,
                    true,
                    "memory_archive",
                    None,
                ))
            }
            _ => unreachable!(),
        },
        ProposalType::ToolPermission => {
            let tool_name = after
                .get("tool_name")
                .or_else(|| after.get("toolName"))
                .or_else(|| after.get("name"))
                .and_then(Value::as_str)
                .ok_or_else(|| "ToolPermission Proposal 缺少 after.tool_name。".to_string())?;
            let permission = after
                .get("permission")
                .or_else(|| after.get("level"))
                .and_then(Value::as_str)
                .unwrap_or("allow_until_revoked");
            let policy = match permission {
                "allowed" | "allow" => {
                    openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked
                }
                "deny" => openlife_core::tool_permissions::ToolPermissionPolicy::Deny,
                "ask_every_time" => {
                    openlife_core::tool_permissions::ToolPermissionPolicy::AskEveryTime
                }
                "allow_once" => openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                "allow_until_revoked" => {
                    openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked
                }
                other => return Err(format!("未知 ToolPermission policy: {}", other)),
            };
            let source = after.get("source").and_then(Value::as_str).unwrap_or("*");
            let risk_level = after
                .get("risk_level")
                .or_else(|| after.get("riskLevel"))
                .and_then(Value::as_str)
                .unwrap_or("*");
            let action_type = after
                .get("action_type")
                .or_else(|| after.get("actionType"))
                .and_then(Value::as_str)
                .unwrap_or("*");
            {
                let permission_store = state.tool_permission_store.lock().await;
                permission_store
                    .grant(tool_name, source, risk_level, action_type, policy, None)
                    .map_err(|e| e.to_string())?;
            }
            {
                let feedback = state.feedback_store.lock().await;
                let detail = serde_json::json!({
                    "proposal_id": proposal.id,
                    "tool_name": tool_name,
                    "permission": permission,
                    "source_detail": proposal.source_detail,
                });
                let detail_text = detail.to_string();
                feedback
                    .log_event(
                        "tool_permission_accepted",
                        proposal.run_id.as_deref(),
                        Some(&detail_text),
                    )
                    .map_err(|e| e.to_string())?;
            }
            Ok(patch_result_for_proposal(
                proposal,
                true,
                "tool_permission",
                None,
            ))
        }
        ProposalType::PluginPermission
        | ProposalType::ScheduledTask
        | ProposalType::ExternalWriteAction
        | ProposalType::ModelPolicyChange
        | ProposalType::DataExport
        | ProposalType::ScheduleCheckin => Err(format!(
            "{} Proposal 尚未接入应用器，已保持 pending。",
            proposal.proposal_type
        )),
    }
}

async fn get_proposal_with_state(
    state: &Arc<AppState>,
    proposal_id: &str,
) -> Result<AgentProposal, String> {
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?;
    let store = store.lock().await;
    store
        .get_proposal(proposal_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Proposal 不存在：{}", proposal_id))
}

async fn update_proposal_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<(), String> {
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?;
    let store = store.lock().await;
    store.update_proposal(proposal).map_err(|e| e.to_string())
}

pub(crate) async fn get_pending_proposals_with_state(
    limit: i64,
    state: &Arc<AppState>,
) -> Result<Vec<AgentProposal>, String> {
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?;
    let store = store.lock().await;
    store
        .list_pending_proposals(limit.clamp(1, 200))
        .map_err(|e| e.to_string())
}

pub(crate) async fn accept_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    check_safe_mode(state)?;
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    ensure_pending_or_postponed(&proposal)?;
    let result = apply_proposal_to_state(state, &proposal, proposal.after.clone()).await?;
    if !result.success {
        return Err(format!(
            "Patch 应用失败: {}",
            result.error.unwrap_or_default()
        ));
    }
    proposal.accept();
    update_proposal_with_state(state, &proposal).await?;
    Ok(serde_json::json!({
        "success": true,
        "patch_result": result,
    }))
}

pub(crate) async fn reject_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    ensure_pending_or_postponed(&proposal)?;
    proposal.reject();
    update_proposal_with_state(state, &proposal).await
}

pub(crate) async fn edit_proposal_with_state(
    proposal_id: String,
    new_after: Value,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    check_safe_mode(state)?;
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    ensure_pending_or_postponed(&proposal)?;
    let result = apply_proposal_to_state(state, &proposal, new_after.clone()).await?;
    if !result.success {
        return Err(format!(
            "Patch 应用失败: {}",
            result.error.unwrap_or_default()
        ));
    }
    proposal.edit(new_after);
    update_proposal_with_state(state, &proposal).await?;
    Ok(serde_json::json!({
        "success": true,
        "patch_result": result,
    }))
}

pub(crate) async fn postpone_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    ensure_pending_or_postponed(&proposal)?;
    proposal.postpone();
    update_proposal_with_state(state, &proposal).await
}

#[tauri::command]
pub async fn get_pending_proposals(
    limit: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentProposal>, String> {
    get_pending_proposals_with_state(limit, state.inner()).await
}

#[tauri::command]
pub async fn list_proposals(
    status: Option<String>,
    proposal_type: Option<String>,
    risk_level: Option<String>,
    limit: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentProposal>, String> {
    let status_filter = status.and_then(|s| match s.as_str() {
        "pending" => Some(ProposalStatus::Pending),
        "accepted" => Some(ProposalStatus::Accepted),
        "rejected" => Some(ProposalStatus::Rejected),
        "edited" => Some(ProposalStatus::Edited),
        "postponed" => Some(ProposalStatus::Postponed),
        _ => None,
    });

    let type_filter = proposal_type.and_then(|t| match t.as_str() {
        "life_model_update" => Some(ProposalType::LifeModelUpdate),
        "goal_update" => Some(ProposalType::GoalUpdate),
        "state_update" => Some(ProposalType::StateUpdate),
        "preference_update" => Some(ProposalType::PreferenceUpdate),
        "capability_update" => Some(ProposalType::CapabilityUpdate),
        "memory_write" => Some(ProposalType::MemoryWrite),
        "memory_archive" => Some(ProposalType::MemoryArchive),
        "tool_permission" => Some(ProposalType::ToolPermission),
        "plugin_permission" => Some(ProposalType::PluginPermission),
        "scheduled_task" => Some(ProposalType::ScheduledTask),
        "external_write_action" => Some(ProposalType::ExternalWriteAction),
        "model_policy_change" => Some(ProposalType::ModelPolicyChange),
        "data_export" => Some(ProposalType::DataExport),
        "schedule_checkin" => Some(ProposalType::ScheduleCheckin),
        _ => None,
    });

    let risk_filter = risk_level.and_then(|r| match r.as_str() {
        "low" => Some(RiskLevel::Low),
        "medium" => Some(RiskLevel::Medium),
        "high" => Some(RiskLevel::High),
        "critical" => Some(RiskLevel::Critical),
        _ => None,
    });

    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?;
    let store = store.lock().await;
    store
        .list_proposals_filtered(status_filter, type_filter, risk_filter, limit.clamp(1, 200))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn batch_accept_low_risk_proposals(
    state: State<'_, Arc<AppState>>,
) -> Result<i64, String> {
    check_safe_mode(state.inner())?;
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?;
    let store = store.lock().await;

    // Get all low risk pending proposals
    let proposals = store
        .list_proposals_filtered(
            Some(ProposalStatus::Pending),
            None,
            Some(RiskLevel::Low),
            200,
        )
        .map_err(|e| e.to_string())?;

    let mut accepted_count = 0i64;
    for proposal in proposals {
        match accept_proposal_with_state(proposal.id.clone(), state.inner()).await {
            Ok(_) => accepted_count += 1,
            Err(e) => eprintln!("Batch accept failed for proposal {}: {}", proposal.id, e),
        }
    }

    Ok(accepted_count)
}

#[tauri::command]
pub async fn accept_proposal(
    proposal_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    accept_proposal_with_state(proposal_id, state.inner()).await
}

#[tauri::command]
pub async fn reject_proposal(
    proposal_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    reject_proposal_with_state(proposal_id, state.inner()).await
}

#[tauri::command]
pub async fn edit_proposal(
    proposal_id: String,
    new_after: Value,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    edit_proposal_with_state(proposal_id, new_after, state.inner()).await
}

#[tauri::command]
pub async fn postpone_proposal(
    proposal_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    postpone_proposal_with_state(proposal_id, state.inner()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{a2a_sidecar::A2ASidecar, HotMemoryCache, PrivacyEngine, SharedHotCache};
    use openlife_core::{
        agent::{
            AgentProposal, ProposalEngine, ProposalSource, ProposalStore, ProposalType, RiskLevel,
        },
        builder::BuilderSessionStore,
        config::AppConfig,
        feedback::FeedbackStore,
        layer_router::LayerRouter,
        life_model::LifeModelManager,
        mcp::McpRegistry,
        mcp_audit::McpAuditStore,
        memory::MemoryStore,
        router::IntentRouter,
        scheduler::InferenceScheduler,
        vectors::VectorStore,
        versioning::VersionManager,
    };
    use std::collections::HashMap;
    use tokio::sync::Mutex;

    fn test_app_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
        let config = AppConfig::default();
        let hot_cache: SharedHotCache =
            Arc::new(tokio::sync::RwLock::new(HotMemoryCache::default()));
        Arc::new(AppState {
            config: Arc::new(Mutex::new(config.clone())),
            life_model_manager: Arc::new(Mutex::new(LifeModelManager::new(
                temp_dir.path().join("life-model").join("current"),
            ))),
            memory_store: Arc::new(Mutex::new(MemoryStore::new_in_memory().unwrap())),
            mcp_registry: Arc::new(Mutex::new(McpRegistry::new())),
            intent_router: Arc::new(Mutex::new(IntentRouter::new())),
            layer_router: Arc::new(Mutex::new(LayerRouter::new())),
            scheduler: Arc::new(Mutex::new(InferenceScheduler::new(
                config.local_model.clone(),
                config.prefer_local_model,
                config.llm.provider.clone(),
                config.llm.openai_base.clone(),
                config.llm.openai_key.clone(),
                config.llm.chat_model.clone(),
                config.llm.embedding_model.clone(),
                config.llm.embedding_enabled,
            ))),
            privacy_engine: Arc::new(Mutex::new(PrivacyEngine::new())),
            version_manager: Arc::new(Mutex::new(VersionManager::new(
                temp_dir.path().join("life-model").join("versions"),
            ))),
            feedback_store: Arc::new(Mutex::new(FeedbackStore::new_in_memory().unwrap())),
            vector_store: Arc::new(Mutex::new(VectorStore::new_in_memory().unwrap())),
            builder_sessions: Arc::new(Mutex::new(HashMap::new())),
            builder_session_store: Arc::new(Mutex::new(BuilderSessionStore::new(
                temp_dir.path().join("builder_sessions.json"),
            ))),
            a2a_sidecar: Arc::new(Mutex::new(A2ASidecar::new(8765))),
            last_snapshot_date: Arc::new(Mutex::new(None)),
            mcp_audit_store: Arc::new(Mutex::new(McpAuditStore::new(
                temp_dir.path().join("mcp_audit.db"),
            ))),
            agent_run_store: None,
            proposal_store: Some(Arc::new(Mutex::new(
                ProposalStore::new_in_memory().unwrap(),
            ))),
            patch_store: Some(Arc::new(Mutex::new(
                openlife_core::life_model::patch_store::PatchStore::new_in_memory().unwrap(),
            ))),
            rollout_metrics_store: None,
            tool_permission_store: Arc::new(Mutex::new(
                openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            )),
            skill_registry: Arc::new(Mutex::new(openlife_core::skills::SkillRegistry::built_in())),
            plugin_registry: Arc::new(Mutex::new(openlife_core::plugins::PluginRegistry::new(
                temp_dir.path().join("plugins"),
            ))),
            hot_cache,
            proposal_engine: Arc::new(tokio::sync::Mutex::new(ProposalEngine::new())),
            startup_warnings: vec![],
        })
    }

    #[tokio::test]
    async fn accept_life_model_proposal_updates_model_and_marks_accepted() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("Fujing"),
            "用户确认的新称呼",
            0.9,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(model.identity.name, "Fujing");
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);
    }

    #[tokio::test]
    async fn edit_life_model_proposal_applies_edited_value() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            "state.current_focus",
            serde_json::json!("旧焦点"),
            "用户状态更新",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        edit_proposal_with_state(id.clone(), serde_json::json!("新焦点"), &state)
            .await
            .unwrap();

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(model.state.current_focus, "新焦点");
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Edited);
    }

    #[tokio::test]
    async fn accept_memory_write_proposal_records_memory_without_life_model_patch() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.records",
            serde_json::json!({
                "session_id": "proposal-session",
                "content": "用户偏好早上做深度工作",
                "source": "review_center"
            }),
            "用户确认写入长期记忆",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let result = accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(result["success"], true);

        let hits = state
            .memory_store
            .lock()
            .await
            .search_text_memories(Some("proposal-session"), "深度工作", 10)
            .unwrap();
        assert_eq!(hits.len(), 1);

        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);
    }

    #[tokio::test]
    async fn accept_memory_archive_proposal_archives_specific_chunk() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let chunk_id = state
            .vector_store
            .lock()
            .await
            .insert("s1", "temporary memory", &[0.1, 0.2, 0.3, 0.4], "test")
            .unwrap();
        let proposal = AgentProposal::new(
            ProposalType::MemoryArchive,
            "memory.chunks",
            serde_json::json!({ "chunk_ids": [chunk_id] }),
            "用户确认归档低价值记忆",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();

        let archived = state.vector_store.lock().await.list_archived(10).unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, chunk_id);
    }

    #[tokio::test]
    async fn accept_memory_archive_without_chunk_ids_keeps_pending() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::MemoryArchive,
            "memory.chunks",
            serde_json::json!({ "reason": "missing ids" }),
            "无效归档请求",
            0.5,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let err = accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap_err();
        assert!(err.contains("chunk_ids"));
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Pending);
    }

    #[tokio::test]
    async fn accept_tool_permission_proposal_records_permission_event() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            "tools.filesystem.write",
            serde_json::json!({
                "tool_name": "filesystem.write",
                "permission": "allowed"
            }),
            "用户确认工具权限",
            0.7,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();

        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);
    }

    #[tokio::test]
    async fn accept_invalid_life_model_path_keeps_proposal_pending() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            "identity.no_such_field",
            serde_json::json!("bad"),
            "无效字段",
            0.5,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let err = accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap_err();
        assert!(err.contains("Invalid path") || err.contains("no_such_field"));
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Pending);
    }

    #[test]
    fn proposal_serializes_for_frontend_contract() {
        let proposal = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("Fujing"),
            "test",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let value = serde_json::to_value(proposal).unwrap();
        assert!(value.get("proposalType").is_some());
        assert_eq!(value.get("proposalType").unwrap(), "goal_update");
        assert_eq!(value.get("riskLevel").unwrap(), "low");
        assert_eq!(value.get("status").unwrap(), "pending");
    }
}
