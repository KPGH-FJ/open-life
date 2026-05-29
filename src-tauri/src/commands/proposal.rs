use crate::{persist_life_model, storage::app_data_dir, AppState};
use openlife_core::agent::{AgentProposal, ProposalStatus, ProposalType, RiskLevel};
use openlife_core::life_model::LifeModel;
use serde_json::Value;
use std::io::Write;
use std::sync::Arc;
use tauri::State;

/// Maximum content size for ExternalWriteAction (100 KB)
const EXTERNAL_WRITE_MAX_SIZE: usize = 100 * 1024;

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

/// Check if any component of the path is a symlink.
/// This includes the final target and any parent directory.
/// Returns true if any symlink is found.
fn path_contains_symlink(path: &std::path::Path) -> bool {
    // Check each existing component in the path
    for component in path.ancestors() {
        if let Ok(meta) = component.symlink_metadata() {
            if meta.file_type().is_symlink() {
                return true;
            }
        }
    }
    false
}

fn canonical_safe_paths(safe_paths: &[String]) -> Vec<std::path::PathBuf> {
    safe_paths
        .iter()
        .filter_map(|safe| {
            let path = std::path::Path::new(safe);
            if path_contains_symlink(path) {
                return None;
            }
            path.canonicalize().ok()
        })
        .collect()
}

fn canonical_parent_in_safe_paths(
    target: &std::path::Path,
    safe_paths: &[std::path::PathBuf],
) -> Result<std::path::PathBuf, String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("Path '{}' has no parent directory.", target.display()))?;
    if path_contains_symlink(parent) {
        return Err(format!(
            "Path '{}' contains a symbolic link. Symbolic links are not allowed in safe paths.",
            parent.display()
        ));
    }
    let canonical_parent = parent
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize parent directory: {}", e))?;
    if !safe_paths
        .iter()
        .any(|safe| canonical_parent.starts_with(safe))
    {
        return Err(format!(
            "Path '{}' is not in safe paths list",
            target.display()
        ));
    }
    Ok(canonical_parent)
}

/// Write content to a file atomically within a safe directory.
/// 1. Verifies no symlinks exist in the path or its parents.
/// 2. Writes to a temp file in the same directory.
/// 3. Renames the temp file to the target (atomic on Unix).
fn safe_write_utf8(path: &str, content: &str, safe_paths: &[String]) -> Result<(), String> {
    let target = std::path::Path::new(path);
    let valid_safe_paths = canonical_safe_paths(safe_paths);
    if valid_safe_paths.is_empty() {
        return Err("No valid safe paths configured for filesystem access".to_string());
    }

    // 1. Strict symlink check: reject any symlink in the path
    if path_contains_symlink(target) {
        return Err(format!(
            "Path '{}' contains a symbolic link. Symbolic links are not allowed in safe paths.",
            path
        ));
    }

    let canonical_parent = canonical_parent_in_safe_paths(target, &valid_safe_paths)?;
    let file_name = target
        .file_name()
        .ok_or_else(|| format!("Path '{}' has no filename.", path))?;
    let canonical_target_path = canonical_parent.join(file_name);

    // 2. Create temp file in the same directory (same filesystem for atomic rename)
    let temp_path = canonical_parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));

    // Write to a newly-created temp file and flush it before rename.
    let mut temp_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|e| format!("Failed to create temporary file: {}", e))?;
    temp_file
        .write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write temporary file: {}", e))?;
    temp_file
        .sync_all()
        .map_err(|e| format!("Failed to sync temporary file: {}", e))?;
    drop(temp_file);

    // Re-check immediately before rename: parent may have changed, and target may
    // have become a symlink after the initial validation.
    let pre_rename_parent = match canonical_parent_in_safe_paths(target, &valid_safe_paths) {
        Ok(parent) => parent,
        Err(e) => {
            let _ = std::fs::remove_file(&temp_path);
            return Err(e);
        }
    };
    if pre_rename_parent != canonical_parent || path_contains_symlink(target) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Path '{}' changed during safe write validation.",
            path
        ));
    }

    // 3. Atomic rename (Unix: atomic; Windows: best-effort)
    match std::fs::rename(&temp_path, &canonical_target_path) {
        Ok(_) => {
            let canonical_target = canonical_target_path
                .canonicalize()
                .map_err(|e| format!("Failed to canonicalize written file: {}", e))?;
            if valid_safe_paths
                .iter()
                .any(|safe| canonical_target.starts_with(safe))
                && !path_contains_symlink(&canonical_target_path)
            {
                Ok(())
            } else {
                Err(format!(
                    "Path '{}' left safe paths during write.",
                    target.display()
                ))
            }
        }
        Err(e) => {
            // Clean up temp file on failure
            let _ = std::fs::remove_file(&temp_path);
            Err(format!("Failed to rename temporary file to target: {}", e))
        }
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

/// Validate that a DataExport filename is a single plain filename.
/// Rejects path traversal, absolute paths, and empty names.
fn validate_export_filename(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." || name == ".." {
        return Err("Filename cannot be empty, '.', or '..'.".to_string());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("Filename cannot contain path separators.".to_string());
    }
    if name.contains("..") {
        return Err("Filename cannot contain parent directory references.".to_string());
    }
    // Ensure it parses as a single normal filename component
    let path = std::path::Path::new(name);
    if path.components().count() != 1 {
        return Err("Filename must be a single component.".to_string());
    }
    if !matches!(
        path.components().next(),
        Some(std::path::Component::Normal(_))
    ) {
        return Err("Filename must be a normal file name.".to_string());
    }
    Ok(())
}

/// Minimal URL-encoding (only encodes space, newline, and special chars).
fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '\n' => "%0A".to_string(),
            '\r' => "%0D".to_string(),
            '&' => "%26".to_string(),
            '=' => "%3D".to_string(),
            '+' => "%2B".to_string(),
            '%' => "%25".to_string(),
            '#' => "%23".to_string(),
            c if c.is_ascii_alphanumeric()
                || c == '-'
                || c == '_'
                || c == '.'
                || c == '!'
                || c == '~'
                || c == '*'
                || c == '\''
                || c == '('
                || c == ')' =>
            {
                c.to_string()
            }
            c => format!("%{:02X}", c as u8),
        })
        .collect()
}

/// Build a minimal ICS (iCalendar) VEVENT string from proposal after data.
fn build_ics_event(after: &Value) -> String {
    let now = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let uid = uuid::Uuid::new_v4().to_string();
    let title = after
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Untitled Event");
    let description = after
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("");
    let scheduled_at = after
        .get("scheduled_at")
        .or_else(|| after.get("date"))
        .and_then(Value::as_str)
        .unwrap_or("");
    // Use scheduled_at as DTSTART; default end to +1h
    let dtend = if !scheduled_at.is_empty() {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(scheduled_at, "%Y-%m-%dT%H:%M:%S") {
            (dt + chrono::Duration::hours(1))
                .format("%Y%m%dT%H%M%SZ")
                .to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    format!(
        "BEGIN:VCALENDAR\r\n\
         VERSION:2.0\r\n\
         PRODID:-//OpenLife//Calendar//EN\r\n\
         BEGIN:VEVENT\r\n\
         DTSTAMP:{now}\r\n\
         UID:{uid}\r\n\
         DTSTART:{scheduled_at}\r\n\
         DTEND:{dtend}\r\n\
         SUMMARY:{title}\r\n\
         DESCRIPTION:{description}\r\n\
         END:VEVENT\r\n\
         END:VCALENDAR\r\n"
    )
}

/// Replace path-unsafe characters in a filename.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
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

fn validate_proposal_payload(proposal_type: ProposalType, after: &Value) -> Result<(), String> {
    match proposal_type {
        ProposalType::LifeModelUpdate
        | ProposalType::GoalUpdate
        | ProposalType::StateUpdate
        | ProposalType::PreferenceUpdate
        | ProposalType::CapabilityUpdate => {
            // LifeModel proposals require after to be a non-null value
            if after.is_null() {
                return Err("LifeModel Proposal 的 after 值不能为 null。".to_string());
            }
            Ok(())
        }
        ProposalType::MemoryWrite => {
            let content = after
                .get("content")
                .and_then(Value::as_str)
                .or_else(|| after.as_str());
            match content {
                Some(c) if !c.trim().is_empty() => Ok(()),
                _ => Err("MemoryWrite Proposal 缺少 after.content（非空字符串）。".to_string()),
            }
        }
        ProposalType::MemoryArchive => {
            let has_ids = after.get("chunk_ids").is_some()
                || after.get("chunkIds").is_some()
                || after.get("ids").is_some()
                || after.as_i64().is_some()
                || after.as_array().map(|a| !a.is_empty()).unwrap_or(false);
            if !has_ids {
                return Err(
                    "MemoryArchive Proposal 缺少 after.chunk_ids（整数或整数数组）。".to_string(),
                );
            }
            Ok(())
        }
        ProposalType::ToolPermission => {
            let tool_name = after
                .get("tool_name")
                .or_else(|| after.get("toolName"))
                .or_else(|| after.get("name"))
                .and_then(Value::as_str);
            match tool_name {
                Some(name) if !name.is_empty() => {
                    let permission = after
                        .get("permission")
                        .or_else(|| after.get("level"))
                        .and_then(Value::as_str)
                        .unwrap_or("allow_until_revoked");
                    let valid_permissions = [
                        "allow",
                        "allowed",
                        "deny",
                        "ask_every_time",
                        "allow_once",
                        "allow_until_revoked",
                    ];
                    if !valid_permissions.contains(&permission) {
                        return Err(format!(
                            "ToolPermission Proposal 的 permission 值 '{}' 无效。有效值: allow, deny, ask_every_time, allow_once, allow_until_revoked",
                            permission
                        ));
                    }
                    Ok(())
                }
                _ => {
                    Err("ToolPermission Proposal 缺少 after.tool_name（非空字符串）。".to_string())
                }
            }
        }
        ProposalType::ExternalWriteAction => {
            let path = after
                .get("path")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty());
            if path.is_none() {
                return Err(
                    "ExternalWriteAction Proposal 缺少 after.path（非空字符串）。".to_string(),
                );
            }
            Ok(())
        }
        ProposalType::ScheduledTask => {
            let title = after
                .get("title")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty());
            if title.is_none() {
                return Err("ScheduledTask Proposal 缺少 after.title（非空字符串）。".to_string());
            }
            Ok(())
        }
        ProposalType::DataExport => {
            let content = after.get("content").and_then(Value::as_str);
            if content.is_none() {
                return Err("DataExport Proposal 缺少 after.content（字符串）。".to_string());
            }
            Ok(())
        }
        ProposalType::PluginPermission
        | ProposalType::ModelPolicyChange
        | ProposalType::ScheduleCheckin
        | ProposalType::Unsupported => {
            // These types are not yet implemented; validation passes but apply will fail
            Ok(())
        }
    }
}

async fn apply_proposal_to_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    after: Value,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    // Validate payload schema before applying
    if let Err(e) = validate_proposal_payload(proposal.proposal_type, &after) {
        return Ok(openlife_core::life_model::patch::PatchApplyResult {
            patch_id: proposal.id.clone(),
            success: false,
            path: proposal.affected_path.clone(),
            operation: "validation_failed".to_string(),
            error: Some(e),
        });
    }

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

                // Check for duplicate content in memory store
                {
                    let store = state.memory_store.lock().await;
                    let hits = store
                        .search_text_memories(Some(&session_id), &content, 10)
                        .map_err(|e| e.to_string())?;
                    let is_duplicate = hits
                        .iter()
                        .any(|hit| hit.chunk.content.trim() == content.trim());
                    if is_duplicate {
                        return Ok(patch_result_for_proposal(
                            proposal,
                            false,
                            "memory_write",
                            Some("检测到重复内容，该记忆已存在。".to_string()),
                        ));
                    }
                }

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
                .or_else(|| after.get("permission_action"))
                .or_else(|| after.get("policy"))
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
            // Check for blocked_action payload from auto-generated proposals
            // so the frontend can offer a "continue" or replay option.
            let blocked_action = after.get("blocked_action").cloned();
            Ok(patch_result_for_proposal(
                proposal,
                true,
                "tool_permission",
                blocked_action.map(|ba| format!("__blocked_action__:{ba}")),
            ))
        }
        ProposalType::ExternalWriteAction => {
            let path = after
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "ExternalWriteAction Proposal 缺少 after.path。".to_string())?;
            let content = after.get("content").and_then(Value::as_str).unwrap_or("");

            // Load safe_paths from config
            let safe_paths = {
                let cfg = state.config.lock().await;
                cfg.system.safe_paths.clone()
            };

            // Re-validate path is within safe_paths using strict canonical parent strategy.
            // This is a defense-in-depth check: the path was already validated at proposal
            // creation time, but the filesystem state may have changed.
            if !openlife_core::agent::action_executor::is_path_in_safe_paths(path, &safe_paths) {
                return Ok(patch_result_for_proposal(
                    proposal,
                    false,
                    "external_write",
                    Some(
                        openlife_core::agent::action_executor::filesystem_access_error(
                            path,
                            &safe_paths,
                        ),
                    ),
                ));
            }

            // Validate content is valid UTF-8 (defense-in-depth: JSON strings are UTF-8,
            // but we enforce it explicitly for audit clarity)
            if std::str::from_utf8(content.as_bytes()).is_err() {
                return Ok(patch_result_for_proposal(
                    proposal,
                    false,
                    "external_write",
                    Some("Content is not valid UTF-8.".to_string()),
                ));
            }

            // Check content size limit
            let max_size = EXTERNAL_WRITE_MAX_SIZE;
            if content.len() > max_size {
                return Ok(patch_result_for_proposal(
                    proposal,
                    false,
                    "external_write",
                    Some(format!(
                        "Content size ({} bytes) exceeds maximum allowed ({} bytes)",
                        content.len(),
                        max_size
                    )),
                ));
            }

            // Validate content hash if present
            if let Some(expected_hash) = after.get("content_hash").and_then(Value::as_str) {
                if !expected_hash.is_empty() {
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(content.as_bytes());
                    let actual_hash = format!("{:x}", hasher.finalize());
                    if actual_hash != expected_hash {
                        return Ok(patch_result_for_proposal(
                            proposal,
                            false,
                            "external_write",
                            Some(format!(
                                "Content hash mismatch: expected {}, got {}",
                                expected_hash, actual_hash
                            )),
                        ));
                    }
                }
            }

            // Beta policy: do NOT auto-create parent directories.
            // is_path_in_safe_paths already requires the parent to exist.
            // If the parent was removed between proposal creation and acceptance,
            // std::fs::write will fail with a clear error and the Proposal stays pending.

            // Execute file write with symlink defense and atomic temp+rename
            match safe_write_utf8(path, content, &safe_paths) {
                Ok(_) => Ok(patch_result_for_proposal(
                    proposal,
                    true,
                    "external_write",
                    None,
                )),
                Err(e) => Ok(patch_result_for_proposal(
                    proposal,
                    false,
                    "external_write",
                    Some(e),
                )),
            }
        }
        ProposalType::ScheduledTask => {
            let title = after
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Untitled Task");
            let scheduled_at = after
                .get("scheduled_at")
                .or_else(|| after.get("date"))
                .and_then(Value::as_str)
                .unwrap_or("");

            let task = serde_json::json!({
                "id": proposal.id,
                "title": title,
                "prompt": after.get("description").and_then(Value::as_str).unwrap_or(""),
                "action_type": after.get("tool").and_then(Value::as_str).unwrap_or("scheduled_task"),
                "scheduled_at": scheduled_at,
                "status": "pending",
                "created_at": chrono::Utc::now().to_rfc3339(),
                "source_run_id": proposal.run_id,
                "source_proposal_id": proposal.id,
            });

            // Atomic append to scheduled_tasks.json under mutex guard
            let tasks_path = app_data_dir().join("scheduled_tasks.json");
            let _guard = state.scheduled_task_mutex.lock().await;

            let mut tasks = if tasks_path.exists() {
                std::fs::read_to_string(&tasks_path)
                    .ok()
                    .and_then(|text| serde_json::from_str::<Vec<Value>>(&text).ok())
                    .unwrap_or_default()
            } else {
                vec![]
            };
            tasks.push(task.clone());

            let temp_path = tasks_path.with_extension("tmp");
            if let Some(parent) = tasks_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "scheduled_task",
                        Some(format!("Failed to create scheduled task directory: {}", e)),
                    ));
                }
            }
            if let Err(e) = std::fs::write(
                &temp_path,
                serde_json::to_string_pretty(&tasks).map_err(|e| e.to_string())?,
            ) {
                return Ok(patch_result_for_proposal(
                    proposal,
                    false,
                    "scheduled_task",
                    Some(format!("Failed to write scheduled task temp file: {}", e)),
                ));
            }
            if let Err(e) = std::fs::rename(&temp_path, &tasks_path) {
                let _ = std::fs::remove_file(&temp_path);
                return Ok(patch_result_for_proposal(
                    proposal,
                    false,
                    "scheduled_task",
                    Some(format!("Failed to atomically save scheduled tasks: {}", e)),
                ));
            }

            // For calendar.propose_event, also write an .ics file if safe_paths allow
            let tool = after.get("tool").and_then(Value::as_str).unwrap_or("");
            if tool == "calendar.propose_event" {
                let safe_paths = {
                    let cfg = state.config.lock().await;
                    cfg.system.safe_paths.clone()
                };
                if !safe_paths.is_empty() {
                    let ics_content = build_ics_event(&after);
                    let ics_filename = format!("{}.ics", sanitize_filename(title));
                    let ics_path = std::path::PathBuf::from(&safe_paths[0]).join(&ics_filename);
                    if let Err(e) = std::fs::write(&ics_path, &ics_content) {
                        log::warn!(
                            "[proposal] Failed to write ICS file '{}': {}",
                            ics_path.display(),
                            e
                        );
                    }
                }
            }

            Ok(patch_result_for_proposal(
                proposal,
                true,
                "scheduled_task",
                None,
            ))
        }
        ProposalType::DataExport => {
            let content = after.get("content").and_then(Value::as_str).unwrap_or("");
            let filename = after
                .get("filename")
                .and_then(Value::as_str)
                .unwrap_or("export.txt");
            let tool = after.get("tool").and_then(Value::as_str).unwrap_or("");

            // email.propose_draft: open system mail client via mailto: URI
            if tool == "email.propose_draft" {
                let to = after.get("to").and_then(Value::as_str).unwrap_or("");
                let subject = after.get("subject").and_then(Value::as_str).unwrap_or("");
                let body = after.get("body").and_then(Value::as_str).unwrap_or(content);
                let mailto = format!(
                    "mailto:{}?subject={}&body={}",
                    to,
                    urlencoding(subject),
                    urlencoding(body)
                );
                match open::that(&mailto) {
                    Ok(_) => Ok(patch_result_for_proposal(
                        proposal,
                        true,
                        "data_export",
                        None,
                    )),
                    Err(e) => Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "data_export",
                        Some(format!("Failed to open mail client: {}", e)),
                    )),
                }
            } else {
                // Default: write to file
                if let Err(e) = validate_export_filename(filename) {
                    return Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "data_export",
                        Some(e),
                    ));
                }
                let safe_paths = {
                    let cfg = state.config.lock().await;
                    cfg.system.safe_paths.clone()
                };
                let export_dir = if !safe_paths.is_empty() {
                    std::path::PathBuf::from(&safe_paths[0])
                } else {
                    app_data_dir().join("exports")
                };

                if let Err(e) = std::fs::create_dir_all(&export_dir) {
                    return Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "data_export",
                        Some(format!("Failed to create export directory: {}", e)),
                    ));
                }

                let export_path = export_dir.join(filename);
                match std::fs::write(&export_path, content) {
                    Ok(_) => Ok(patch_result_for_proposal(
                        proposal,
                        true,
                        "data_export",
                        None,
                    )),
                    Err(e) => Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "data_export",
                        Some(format!(
                            "Failed to write export file '{}': {}",
                            export_path.display(),
                            e
                        )),
                    )),
                }
            } // end else (non-email DataExport)
        }
        ProposalType::PluginPermission
        | ProposalType::ModelPolicyChange
        | ProposalType::ScheduleCheckin
        | ProposalType::Unsupported => Err(format!(
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
    // Check for blocked_action in the patch result error field
    let blocked_action_info = if let Some(ref err) = result.error {
        if err.starts_with("__blocked_action__:") {
            err.strip_prefix("__blocked_action__:")
                .map(|s| s.to_string())
        } else {
            None
        }
    } else {
        None
    };
    let mut response = serde_json::json!({
        "success": true,
        "patch_result": result,
    });
    if let Some(blocked) = blocked_action_info {
        if let Ok(parsed) = serde_json::from_str::<Value>(&blocked) {
            response["blocked_action"] = parsed;
            response["can_continue"] = serde_json::Value::Bool(true);
        }
    }
    Ok(response)
}

pub(crate) async fn reject_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    ensure_pending_or_postponed(&proposal)?;
    proposal.reject();
    update_proposal_with_state(state, &proposal).await?;
    record_rejected_proactive_reminder_evidence(state, &proposal).await;
    Ok(())
}

async fn record_rejected_proactive_reminder_evidence(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) {
    let evidence_store = state.evidence_store.lock().await;
    if let Err(e) = openlife_core::proactive::ProactiveEngine::default()
        .record_rejected_reminder_proposal(&evidence_store, proposal)
    {
        log::warn!(
            "[LifeModel-HS] failed to record rejected reminder evidence for proposal {}: {}",
            proposal.id,
            e
        );
    }
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
        "unsupported" => Some(ProposalType::Unsupported),
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
    proposal_ids: Option<Vec<String>>,
    state: State<'_, Arc<AppState>>,
) -> Result<i64, String> {
    check_safe_mode(state.inner())?;
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?;
    let store = store.lock().await;

    // If specific IDs provided, use those; otherwise fall back to all low-risk pending
    let proposals = if let Some(ids) = proposal_ids {
        let mut proposals = Vec::new();
        for id in ids {
            if let Ok(Some(p)) = store.get_proposal(&id) {
                if p.status == ProposalStatus::Pending && p.risk_level == RiskLevel::Low {
                    proposals.push(p);
                }
            }
        }
        proposals
    } else {
        store
            .list_proposals_filtered(
                Some(ProposalStatus::Pending),
                None,
                Some(RiskLevel::Low),
                200,
            )
            .map_err(|e| e.to_string())?
    };

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
            evidence_store: Arc::new(Mutex::new(
                openlife_core::agent::EvidenceStore::new_in_memory().unwrap(),
            )),
            heuristic_store: Arc::new(Mutex::new({
                let store = openlife_core::agent::HeuristicStore::new_in_memory().unwrap();
                store.seed_mvp_heuristics().unwrap();
                store
            })),
            policy_store: Arc::new(openlife_core::agent::PolicyStore::mvp_builtin()),
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
            provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
            scheduled_task_mutex: Arc::new(tokio::sync::Mutex::new(())),
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
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

    #[tokio::test]
    async fn accept_external_write_action_writes_file_to_safe_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let safe_path = temp_dir.path().join("safe");
        std::fs::create_dir_all(&safe_path).unwrap();
        let safe_path_canonical = safe_path.canonicalize().unwrap();
        {
            let mut cfg = state.config.lock().await;
            cfg.system.safe_paths = vec![safe_path_canonical.to_string_lossy().to_string()];
        }

        let file_path = safe_path_canonical.join("test.txt");
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("filesystem.{}", file_path.display()),
            serde_json::json!({
                "path": file_path.to_string_lossy().to_string(),
                "content": "Hello from test",
                "content_hash": "",
                "size_bytes": 15,
                "operation": "create",
            }),
            "测试写入文件",
            0.8,
            RiskLevel::High,
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
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello from test");
    }

    #[tokio::test]
    async fn proposal_accepts_hs_external_write_payload_and_verifies_hash() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let safe_path = temp_dir.path().join("safe");
        std::fs::create_dir_all(&safe_path).unwrap();
        let safe_path_canonical = safe_path.canonicalize().unwrap();
        {
            let mut cfg = state.config.lock().await;
            cfg.system.safe_paths = vec![safe_path_canonical.to_string_lossy().to_string()];
        }

        let file_path = safe_path_canonical.join("hs-payload.txt");
        let content = "真实 content 应由 HS ExternalWriteAction payload 写入";
        let content_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            format!("{:x}", hasher.finalize())
        };
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("builtin.{}", file_path.display()),
            serde_json::json!({
                "tool_name": "file.write",
                "tool_id": "file.write",
                "source": "builtin",
                "arguments": {
                    "path": file_path.to_string_lossy().to_string(),
                    "content": content
                },
                "path": file_path.to_string_lossy().to_string(),
                "content": content,
                "content_preview": content,
                "content_hash": content_hash,
                "size_bytes": content.len(),
                "operation": "create",
                "requires_confirmation": true,
                "hs_policy_id": openlife_core::agent::BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST,
            }),
            "HS proposal-first 写入文件",
            0.9,
            RiskLevel::High,
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
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), content);
    }

    #[tokio::test]
    async fn accept_external_write_action_blocks_outside_safe_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let safe_path = temp_dir.path().join("safe");
        std::fs::create_dir_all(&safe_path).unwrap();
        let safe_path_canonical = safe_path.canonicalize().unwrap();
        {
            let mut cfg = state.config.lock().await;
            cfg.system.safe_paths = vec![safe_path_canonical.to_string_lossy().to_string()];
        }

        let file_path = temp_dir.path().join("unsafe.txt");
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("filesystem.{}", file_path.display()),
            serde_json::json!({
                "path": file_path.to_string_lossy().to_string(),
                "content": "should not write",
            }),
            "测试安全路径拦截",
            0.8,
            RiskLevel::High,
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
        assert!(err.contains("not in safe paths"));
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn rejecting_proactive_reminder_records_negative_evidence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "proactive.reminder.pending_proposal",
            serde_json::json!({
                "proactive_reminder_category": "pending_proposal",
                "prompt_digest": "digest-only",
            }),
            "raw reminder rejection text should not be stored as evidence",
            0.7,
            RiskLevel::Low,
            ProposalSource::ProactiveAgent,
        );
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        reject_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let records = state
            .evidence_store
            .lock()
            .await
            .query(openlife_core::agent::EvidenceQuery {
                affected_path: Some("proactive.reminder.pending_proposal".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].linked_proposal_ids.contains(&proposal_id));
        let serialized = serde_json::to_string(&records[0]).unwrap();
        assert!(!serialized.contains("raw reminder rejection text"));
    }

    #[test]
    fn safe_write_utf8_creates_and_overwrites_inside_safe_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let safe_path = temp_dir.path().join("safe");
        std::fs::create_dir_all(&safe_path).unwrap();
        let safe_path = safe_path.canonicalize().unwrap();
        let safe_paths = vec![safe_path.to_string_lossy().to_string()];
        let file_path = safe_path.join("write.txt");

        safe_write_utf8(&file_path.to_string_lossy(), "first", &safe_paths).unwrap();
        safe_write_utf8(&file_path.to_string_lossy(), "second", &safe_paths).unwrap();

        assert_eq!(std::fs::read_to_string(file_path).unwrap(), "second");
    }

    #[cfg(unix)]
    #[test]
    fn safe_write_utf8_rejects_target_symlink() {
        let temp_dir = tempfile::tempdir().unwrap();
        let safe_path = temp_dir.path().join("safe");
        let outside_path = temp_dir.path().join("outside");
        std::fs::create_dir_all(&safe_path).unwrap();
        std::fs::create_dir_all(&outside_path).unwrap();
        let target = safe_path.join("link.txt");
        let outside_file = outside_path.join("outside.txt");
        std::fs::write(&outside_file, "outside").unwrap();
        std::os::unix::fs::symlink(&outside_file, &target).unwrap();
        let safe_path = safe_path.canonicalize().unwrap();
        let safe_paths = vec![safe_path.to_string_lossy().to_string()];

        let err = safe_write_utf8(&target.to_string_lossy(), "new", &safe_paths).unwrap_err();
        assert!(err.contains("symbolic link"));
        assert_eq!(std::fs::read_to_string(outside_file).unwrap(), "outside");
    }

    #[cfg(unix)]
    #[test]
    fn safe_write_utf8_rejects_parent_symlink() {
        let temp_dir = tempfile::tempdir().unwrap();
        let safe_path = temp_dir.path().join("safe");
        let outside_path = temp_dir.path().join("outside");
        std::fs::create_dir_all(&safe_path).unwrap();
        std::fs::create_dir_all(&outside_path).unwrap();
        let link_dir = safe_path.join("linked-dir");
        std::os::unix::fs::symlink(&outside_path, &link_dir).unwrap();
        let target = link_dir.join("write.txt");
        let safe_path = safe_path.canonicalize().unwrap();
        let safe_paths = vec![safe_path.to_string_lossy().to_string()];

        let err = safe_write_utf8(&target.to_string_lossy(), "new", &safe_paths).unwrap_err();
        assert!(err.contains("symbolic link") || err.contains("safe paths"));
        assert!(!outside_path.join("write.txt").exists());
    }

    #[tokio::test]
    async fn accept_scheduled_task_returns_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "calendar.event",
            serde_json::json!({
                "title": "Team Meeting",
                "scheduled_at": "2026-05-10T10:00:00Z",
                "description": "Weekly sync",
            }),
            "测试创建计划任务",
            0.8,
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
    async fn accept_data_export_returns_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let safe_path = temp_dir.path().join("safe");
        std::fs::create_dir_all(&safe_path).unwrap();
        {
            let mut cfg = state.config.lock().await;
            cfg.system.safe_paths = vec![safe_path.to_string_lossy().to_string()];
        }

        let proposal = AgentProposal::new(
            ProposalType::DataExport,
            "export.file",
            serde_json::json!({
                "content": "exported data",
                "filename": "export.txt",
            }),
            "测试数据导出",
            0.8,
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
