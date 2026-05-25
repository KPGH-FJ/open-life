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

fn tool_permission_policy_from_after(
    after: &Value,
) -> Result<openlife_core::tool_permissions::ToolPermissionPolicy, String> {
    let policy_value = after
        .get("policy")
        .or_else(|| after.get("permission"))
        .or_else(|| after.get("level"))
        .or_else(|| after.get("permission_action"))
        .and_then(Value::as_str)
        .unwrap_or("allow_until_revoked");

    match policy_value {
        "allowed" | "allow" | "grant" => {
            Ok(openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked)
        }
        "deny" | "revoke" => Ok(openlife_core::tool_permissions::ToolPermissionPolicy::Deny),
        "ask_every_time" => Ok(openlife_core::tool_permissions::ToolPermissionPolicy::AskEveryTime),
        "allow_once" => Ok(openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce),
        "allow_until_revoked" => {
            Ok(openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked)
        }
        other => Err(format!("未知 ToolPermission policy: {}", other)),
    }
}

async fn find_replayable_action_id_for_tool_permission(
    state: &Arc<AppState>,
    run_id: &str,
    after: &Value,
) -> Result<Option<String>, String> {
    let Some(store_arc) = state.agent_run_store.as_ref() else {
        return Ok(None);
    };
    let run = {
        let store = store_arc.lock().await;
        store
            .get_run(run_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("AgentRun 不存在：{}", run_id))?
    };

    let tool_name = after
        .get("tool_name")
        .or_else(|| after.get("toolName"))
        .or_else(|| after.get("name"))
        .and_then(Value::as_str);
    let source = after.get("source").and_then(Value::as_str);
    let risk_level = after
        .get("risk_level")
        .or_else(|| after.get("riskLevel"))
        .and_then(Value::as_str);
    let action_type = after
        .get("action_type")
        .or_else(|| after.get("actionType"))
        .and_then(Value::as_str);
    let step_index = after
        .get("blocked_action")
        .and_then(|v| v.get("step_index"))
        .and_then(Value::as_u64);

    let pending_actions = run
        .actions
        .iter()
        .filter(|action| action.status == "needs_confirmation");

    let action = pending_actions
        .filter(|action| {
            if let Some(step) = step_index {
                if !action.id.starts_with(&format!("action-{}-", step)) {
                    return false;
                }
            }
            if let Some(expected_tool) = tool_name {
                let action_tool = action
                    .tool_scope
                    .as_ref()
                    .map(|scope| scope.tool_name.as_str())
                    .or(action.target.as_deref());
                if action_tool != Some(expected_tool) {
                    return false;
                }
            }
            if let Some(expected_source) = source {
                if expected_source != "*" {
                    let action_source = action
                        .tool_scope
                        .as_ref()
                        .map(|scope| scope.source.as_str());
                    if action_source != Some(expected_source) {
                        return false;
                    }
                }
            }
            if let Some(expected_risk) = risk_level {
                if expected_risk != "*" {
                    let action_risk = action
                        .tool_scope
                        .as_ref()
                        .map(|scope| scope.risk_level.as_str());
                    if action_risk != Some(expected_risk) {
                        return false;
                    }
                }
            }
            if let Some(expected_action_type) = action_type {
                if expected_action_type != "*" {
                    let actual_action_type = action
                        .tool_scope
                        .as_ref()
                        .map(|scope| scope.action_type.as_str())
                        .unwrap_or(action.action_type.as_str());
                    if actual_action_type != expected_action_type {
                        return false;
                    }
                }
            }
            true
        })
        .min_by_key(|action| action.timestamp)
        .map(|action| action.id.clone());

    Ok(action)
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
        blocked_action: None,
    }
}

fn patch_result_with_blocked_action(
    proposal: &AgentProposal,
    success: bool,
    operation: &str,
    error: Option<String>,
    blocked_action: Option<serde_json::Value>,
) -> openlife_core::life_model::patch::PatchApplyResult {
    openlife_core::life_model::patch::PatchApplyResult {
        patch_id: proposal.id.clone(),
        success,
        path: proposal.affected_path.clone(),
        operation: operation.to_string(),
        error,
        blocked_action,
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
            c => {
                let mut buf = [0u8; 4];
                let encoded = c.encode_utf8(&mut buf);
                encoded
                    .bytes()
                    .map(|b| format!("%{:02X}", b))
                    .collect::<Vec<_>>()
                    .join("")
            }
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
         END:VCALENDAR\r\n",
        title = ics_escape_text(title),
        description = ics_escape_text(description),
    )
}

fn ics_escape_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
        .replace('\r', "")
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
                        .get("policy")
                        .or_else(|| after.get("permission_action"))
                        .or_else(|| after.get("permission"))
                        .or_else(|| after.get("level"))
                        .and_then(Value::as_str)
                        .unwrap_or("allow_until_revoked");
                    let valid_permissions = [
                        "allow",
                        "allowed",
                        "grant",
                        "deny",
                        "revoke",
                        "ask_every_time",
                        "allow_once",
                        "allow_until_revoked",
                    ];
                    if !valid_permissions.contains(&permission) {
                        return Err(format!(
                            "ToolPermission Proposal 的 policy 值 '{}' 无效。有效值: allow, grant, deny, ask_every_time, allow_once, allow_until_revoked",
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

async fn apply_life_model_patch(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    after: &Value,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    let mut model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    let _before_snapshot = {
        let vm = state.version_manager.lock().await;
        vm.snapshot_for_patch(&model, &proposal.id, "before")
            .map_err(|e| e.to_string())?
    };
    let path_pointer = openlife_core::life_model::patch::dot_to_pointer(&proposal.affected_path);
    let path_display = openlife_core::life_model::patch::pointer_to_display(&path_pointer, &model);
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
    let result = model.apply_patch(&patch).map_err(|e| e.to_string())?;
    if !result.success {
        return Ok(result);
    }
    persist_life_model(state, model.clone(), true).await?;
    let _after_snapshot = {
        let vm = state.version_manager.lock().await;
        vm.snapshot_for_patch(&model, &proposal.id, "after")
            .map_err(|e| e.to_string())?
    };
    if let Some(ref patch_store_arc) = state.patch_store {
        let patch_store = patch_store_arc.lock().await;
        let mut patch_to_save = patch.clone();
        patch_to_save.mark_applied();
        let _ = patch_store.create_patch(&patch_to_save);
    }
    Ok(result)
}

async fn apply_memory_write(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    after: &Value,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    let content = memory_content(after)?;
    let session_id = memory_session_id(after);
    let source = memory_source(after);
    // Duplicate check
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

async fn apply_memory_archive(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    after: &Value,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    let ids = memory_archive_ids(after)?;
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

async fn apply_tool_permission(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    after: &Value,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    let tool_name = after
        .get("tool_name")
        .or_else(|| after.get("toolName"))
        .or_else(|| after.get("name"))
        .and_then(Value::as_str)
        .ok_or_else(|| "ToolPermission Proposal 缺少 after.tool_name。".to_string())?;
    let policy = tool_permission_policy_from_after(after)?;
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
            "permission": policy.to_string(),
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
    let blocked_action = after.get("blocked_action").cloned();
    Ok(patch_result_with_blocked_action(
        proposal,
        true,
        "tool_permission",
        None,
        blocked_action,
    ))
}

async fn apply_external_write_action(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    after: &Value,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    let path = after
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "ExternalWriteAction Proposal 缺少 after.path。".to_string())?;
    let content = after.get("content").and_then(Value::as_str).unwrap_or("");
    let safe_paths = {
        let cfg = state.config.lock().await;
        cfg.system.safe_paths.clone()
    };
    if !openlife_core::agent::action_executor::is_path_in_safe_paths(path, &safe_paths) {
        return Ok(patch_result_for_proposal(
            proposal,
            false,
            "external_write",
            Some(openlife_core::agent::action_executor::filesystem_access_error(path, &safe_paths)),
        ));
    }
    if std::str::from_utf8(content.as_bytes()).is_err() {
        return Ok(patch_result_for_proposal(
            proposal,
            false,
            "external_write",
            Some("Content is not valid UTF-8.".to_string()),
        ));
    }
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

async fn apply_scheduled_task(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    after: &Value,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
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
    // For calendar.propose_event, also write an .ics file
    let tool = after.get("tool").and_then(Value::as_str).unwrap_or("");
    if tool == "calendar.propose_event" {
        let safe_paths = {
            let cfg = state.config.lock().await;
            cfg.system.safe_paths.clone()
        };
        if !safe_paths.is_empty() {
            let ics_content = build_ics_event(after);
            let ics_filename = format!("{}.ics", sanitize_filename(title));
            let ics_path = std::path::PathBuf::from(&safe_paths[0]).join(&ics_filename);
            if let Err(e) = safe_write_utf8(&ics_path.to_string_lossy(), &ics_content, &safe_paths)
            {
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

async fn apply_data_export(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    after: &Value,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    let content = after.get("content").and_then(Value::as_str).unwrap_or("");
    let filename = after
        .get("filename")
        .and_then(Value::as_str)
        .unwrap_or("export.txt");
    let tool = after.get("tool").and_then(Value::as_str).unwrap_or("");
    // email.propose_draft: open system mail client
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
        let path_lossy = export_path.to_string_lossy();
        match safe_write_utf8(path_lossy.as_ref(), content, &safe_paths) {
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
    }
}

async fn apply_proposal_to_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    after: Value,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    if let Err(e) = validate_proposal_payload(proposal.proposal_type, &after) {
        return Ok(openlife_core::life_model::patch::PatchApplyResult {
            patch_id: proposal.id.clone(),
            success: false,
            path: proposal.affected_path.clone(),
            operation: "validation_failed".to_string(),
            error: Some(e),
            blocked_action: None,
        });
    }

    match proposal.proposal_type {
        ProposalType::LifeModelUpdate
        | ProposalType::GoalUpdate
        | ProposalType::StateUpdate
        | ProposalType::PreferenceUpdate
        | ProposalType::CapabilityUpdate => apply_life_model_patch(state, proposal, &after).await,
        ProposalType::MemoryWrite => apply_memory_write(state, proposal, &after).await,
        ProposalType::MemoryArchive => apply_memory_archive(state, proposal, &after).await,
        ProposalType::ToolPermission => apply_tool_permission(state, proposal, &after).await,
        ProposalType::ExternalWriteAction => {
            apply_external_write_action(state, proposal, &after).await
        }
        ProposalType::ScheduledTask => apply_scheduled_task(state, proposal, &after).await,
        ProposalType::DataExport => apply_data_export(state, proposal, &after).await,
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
    // Use typed blocked_action field instead of __blocked_action__: string protocol.
    let blocked_action_info = result.blocked_action.clone();
    let mut response = serde_json::json!({
        "success": true,
        "patch_result": result,
    });
    if let Some(blocked) = blocked_action_info {
        response["blocked_action"] = blocked;
        response["can_continue"] = serde_json::Value::Bool(true);
    }
    if proposal.proposal_type == ProposalType::ToolPermission {
        if let Some(run_id) = proposal.run_id.as_deref() {
            if let Some(action_id) =
                find_replayable_action_id_for_tool_permission(state, run_id, &proposal.after)
                    .await?
            {
                response["can_continue"] = serde_json::Value::Bool(true);
                response["continue_run_id"] = serde_json::Value::String(run_id.to_string());
                response["continue_action_id"] = serde_json::Value::String(action_id);
            }
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
    batch_accept_low_risk_proposals_with_state(proposal_ids, state.inner()).await
}

pub(crate) async fn batch_accept_low_risk_proposals_with_state(
    proposal_ids: Option<Vec<String>>,
    state: &Arc<AppState>,
) -> Result<i64, String> {
    check_safe_mode(state)?;

    // Collect qualifying proposal IDs while holding the lock,
    // then release the lock before accepting to avoid deadlock.
    let qualifying_ids = {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?;
        let store = store.lock().await;

        if let Some(ids) = proposal_ids {
            let mut qualifying = Vec::new();
            for id in ids {
                if let Ok(Some(p)) = store.get_proposal(&id) {
                    if p.status == ProposalStatus::Pending && p.risk_level == RiskLevel::Low {
                        qualifying.push(p.id.clone());
                    }
                }
            }
            qualifying
        } else {
            store
                .list_proposals_filtered(
                    Some(ProposalStatus::Pending),
                    None,
                    Some(RiskLevel::Low),
                    200,
                )
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|p| p.id.clone())
                .collect()
        }
    };

    let mut accepted_count = 0i64;
    for id in qualifying_ids {
        let proposal_id = id.clone();
        match accept_proposal_with_state(id, state).await {
            Ok(_) => accepted_count += 1,
            Err(e) => eprintln!("Batch accept failed for proposal {}: {}", proposal_id, e),
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
        mcp::{McpRegistry, MockMcpClient},
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
            agent_run_event_store: None,
            plan_store: None,
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
            agent_spec_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::AgentSpecStore::new_in_memory().unwrap(),
            )),
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
    async fn accept_auto_tool_permission_proposal_uses_policy_not_grant_action() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.web.search",
            serde_json::json!({
                "permission_action": "grant",
                "tool_name": "web.search",
                "source": "builtin",
                "risk_level": "medium",
                "policy": "allow_until_revoked",
                "blocked_action": {
                    "action_type": "read",
                    "target": "web.search"
                },
                "reason": "low-risk read action allowed by default",
                "auto_generated": true
            }),
            "自动生成的工具权限确认",
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

        let permissions = state.tool_permission_store.lock().await.list().unwrap();
        assert_eq!(permissions.len(), 1);
        assert_eq!(permissions[0].tool_name, "web.search");
        assert_eq!(
            permissions[0].policy,
            openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked
        );
    }

    #[tokio::test]
    async fn tool_permission_replay_lookup_matches_pending_action_from_run() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));

        let mut run = openlife_core::agent::AgentRun::new_chat_run("session-1", "search");
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: "action-0-123".to_string(),
            action_type: "tool_call".to_string(),
            target: Some("web.search".to_string()),
            input: serde_json::json!({ "query": "万象城" }),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("ask_every_time".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "builtin.web.search".to_string(),
                tool_name: "web.search".to_string(),
                source: "builtin".to_string(),
                risk_level: "medium".to_string(),
                capabilities: vec!["web".to_string()],
                action_type: "read".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();

        let action_id = find_replayable_action_id_for_tool_permission(
            &state,
            &run.id,
            &serde_json::json!({
                "tool_name": "web.search",
                "source": "builtin",
                "risk_level": "medium",
                "action_type": "read",
                "blocked_action": { "step_index": 0 }
            }),
        )
        .await
        .unwrap();

        assert_eq!(action_id.as_deref(), Some("action-0-123"));
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
        let safe_path_canonical = safe_path.canonicalize().unwrap();
        {
            let mut cfg = state.config.lock().await;
            cfg.system.safe_paths = vec![safe_path_canonical.to_string_lossy().to_string()];
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

    #[tokio::test]
    async fn batch_accept_real_helper_with_timeout_and_only_accepts_pending_low() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let p1 = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("test1"),
            "low pending",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let p1_id = p1.id.clone();

        let mut p2 = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("test2"),
            "low but already accepted",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        p2.accept();
        let p2_id = p2.id.clone();

        let p3 = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("test3"),
            "high pending",
            0.9,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        let p3_id = p3.id.clone();

        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&p1).unwrap();
            store.create_proposal(&p2).unwrap();
            store.create_proposal(&p3).unwrap();
        }

        // Call the real helper with specific IDs
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            batch_accept_low_risk_proposals_with_state(
                Some(vec![p1_id.clone(), p2_id.clone(), p3_id.clone()]),
                &state,
            ),
        )
        .await
        .expect("batch accept should complete within 5s (no deadlock)");

        assert!(result.is_ok(), "batch accept should not error");
        assert_eq!(
            result.unwrap(),
            1,
            "only the low+pending proposal should be accepted"
        );

        let store = state.proposal_store.as_ref().unwrap().lock().await;
        assert_eq!(
            store.get_proposal(&p1_id).unwrap().unwrap().status,
            ProposalStatus::Accepted
        );
        assert_eq!(
            store.get_proposal(&p2_id).unwrap().unwrap().status,
            ProposalStatus::Accepted // unchanged — it was already accepted
        );
        assert_eq!(
            store.get_proposal(&p3_id).unwrap().unwrap().status,
            ProposalStatus::Pending // not accepted — high risk
        );
    }

    #[tokio::test]
    async fn batch_accept_real_helper_scan_all_pending_low_when_no_ids_provided() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let p_high = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("high"),
            "high risk pending",
            0.9,
            RiskLevel::High,
            ProposalSource::Manual,
        );

        let p_low1 = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("low1"),
            "low pending 1",
            0.5,
            RiskLevel::Low,
            ProposalSource::Manual,
        );

        let p_low2 = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("low2"),
            "low pending 2",
            0.5,
            RiskLevel::Low,
            ProposalSource::Manual,
        );

        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&p_high).unwrap();
            store.create_proposal(&p_low1).unwrap();
            store.create_proposal(&p_low2).unwrap();
        }

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            batch_accept_low_risk_proposals_with_state(None, &state),
        )
        .await
        .expect("batch accept should complete within 5s");

        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            2,
            "both low-risk pending should be accepted"
        );

        let store = state.proposal_store.as_ref().unwrap().lock().await;
        assert_eq!(
            store.get_proposal(&p_high.id).unwrap().unwrap().status,
            ProposalStatus::Pending
        );
        assert_eq!(
            store.get_proposal(&p_low1.id).unwrap().unwrap().status,
            ProposalStatus::Accepted
        );
        assert_eq!(
            store.get_proposal(&p_low2.id).unwrap().unwrap().status,
            ProposalStatus::Accepted
        );
    }

    /// Full MCP target ask → accept → replay closure.
    /// Uses real ActionExecutor.execute → auto-generated Proposal →
    /// real accept_proposal_with_state → real replay_action_internal.
    #[tokio::test]
    async fn mcp_network_ask_real_mcp_source_execute_accept_replay_closure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);

        // Inject agent_run_store (needed to persist run and for replay)
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));

        // Set NetworkPolicy: enabled=true, default_decision=ask
        {
            let mut config_guard = state.config.lock().await;
            config_guard.system.network_policy = openlife_core::config::NetworkPolicy {
                enabled: true,
                default_decision: "ask".to_string(),
                domain_allowlist: vec![],
                domain_denylist: vec![],
                tool_overrides: std::collections::HashMap::new(),
            };
        }

        // Build a fresh McpRegistry. The default builtin wrappers (including
        // mcp.call_tool) are already registered with correct metadata.
        let mut reg = openlife_core::mcp::McpRegistry::new();
        let run_id = "run-mcp-real-exec".to_string();
        let step_index: u32 = 0;

        // Register the real target with ToolSource::Mcp.
        // We do NOT re-register mcp.call_tool — the default builtin already exists.

        // 2. Register real target with ToolSource::Mcp
        // Register the manifest (dummy builtin — execution goes through the
        // mock MCP client below, not through this closure).
        let target_name = "test_mcp_network_tool";
        reg.register_builtin(
            openlife_core::tool_manifest::ToolManifest {
                name: target_name.to_string(),
                id: target_name.to_string(),
                description: "A mock MCP network tool".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "low".to_string(),
                risk_level: "low".to_string(),
                version: "1.0.0".to_string(),
                source: openlife_core::tool_manifest::ToolSource::Mcp {
                    server_name: "test-server".to_string(),
                },
                capabilities: vec!["network".to_string(), "read".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".to_string(),
                tags: vec!["execution".to_string()],
            },
            std::sync::Arc::new(|_args| panic!("MCP target must not execute via builtin fallback")),
        );
        // Register mock MCP client — this is the real execution path.
        reg.register_mock_mcp_client(
            "test-server",
            openlife_core::mcp::MockMcpClient::new(
                "test-server",
                |_name: String, _args: serde_json::Value| -> anyhow::Result<String> {
                    Ok("mock-mcp-ok".to_string())
                },
            ),
        );

        // Replace state registry with custom one
        Arc::get_mut(&mut state).unwrap().mcp_registry = Arc::new(tokio::sync::Mutex::new(reg));

        // 3. Grant permission to wrapper mcp.call_tool so only target gates
        {
            let perm = state.tool_permission_store.lock().await;
            perm.grant(
                "mcp.call_tool",
                "builtin",
                "medium",
                "external_side_effect",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        }

        // 4. Build ActionExecutor and ActionContext
        let executor = openlife_core::agent::action_executor::ActionExecutor::new(
            openlife_core::agent::action_executor::ActionExecutorConfig {
                allow_writes: true,
                allow_cloud: true,
                timeout_seconds: 120,
                consume_allow_once: true,
            },
        );

        let network_policy = {
            let config = state.config.lock().await;
            config.system.network_policy.clone()
        };

        let ac = openlife_core::agent::action_executor::ActionContext {
            registry: state.mcp_registry.clone(),
            permission_store: state.tool_permission_store.clone(),
            audit_store: state.mcp_audit_store.clone(),
            privacy_engine: state.privacy_engine.clone(),
            safe_paths: vec![],
            life_model: None,
            memory_store: None,
            proposal_store: state.proposal_store.clone(),
            agent_run_store: state.agent_run_store.clone(),
            event_store: None,
            network_policy: Some(network_policy),
            calendar_ics_paths: vec![],
            execution_sandbox: openlife_core::agent::execution_sandbox::ExecutionSandbox::default(),
            agent_spec: None,
        };

        // 5. Execute mcp.call_tool targeting the MCP tool with server argument
        let request = openlife_core::agent::action_executor::AgentActionRequest {
            action_type: "builtin_tool".to_string(),
            target: "mcp.call_tool".to_string(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": target_name,
                    "server": "test-server",
                    "arguments": {}
                }
            }),
            source_run_id: Some(run_id.clone()),
            step_index,
        };
        let exec_result = executor.execute(request, &ac).await.unwrap();

        // 6. Assert first execution: needs confirmation
        assert_eq!(
            exec_result.status,
            openlife_core::agent::action_executor::ActionExecutionStatus::NeedsConfirmation,
            "first execution must require confirmation"
        );
        assert_eq!(exec_result.action.status, "needs_confirmation");
        let scope = exec_result
            .action
            .tool_scope
            .as_ref()
            .expect("action must have tool_scope");
        assert_eq!(scope.tool_name, target_name);
        // Source must be the real MCP source, not "builtin"
        assert_eq!(scope.source, "mcp:test-server");
        assert_eq!(scope.risk_level, "low");
        assert_eq!(scope.action_type, "read");
        assert!(scope.capabilities.contains(&"network".to_string()));
        assert!(scope.capabilities.contains(&"read".to_string()));
        let action = exec_result.action.clone();

        // 7. Write the pending action into a real AgentRun
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("mcp.call_tool")
            .with_agent_spec_id("main.default");
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(action.clone());
        let action_id = action.id.clone();
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        // 8. Read auto-generated Proposal from ProposalStore
        let (proposal_id, _proposal_after) = {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            let pending = store.list_pending_proposals(20).unwrap();
            // Should have exactly one ToolPermission proposal
            let tp_proposals: Vec<_> = pending
                .into_iter()
                .filter(|p| {
                    p.proposal_type == ProposalType::ToolPermission
                        && p.run_id.as_deref() == Some(&run_id)
                })
                .collect();
            assert_eq!(
                tp_proposals.len(),
                1,
                "should have exactly one ToolPermission proposal for this run"
            );
            let p = &tp_proposals[0];
            assert_eq!(p.proposal_type, ProposalType::ToolPermission);
            assert_eq!(
                p.run_id.as_deref(),
                Some(run_id.as_str()),
                "proposal must be linked to the run"
            );

            // Assert proposal metadata uses TARGET manifest, not wrapper
            let after = &p.after;
            assert_eq!(
                after.get("tool_name").and_then(|v| v.as_str()),
                Some(target_name),
                "proposal after.tool_name must be target, not wrapper"
            );
            assert_eq!(
                after.get("source").and_then(|v| v.as_str()),
                Some("mcp:test-server"),
                "proposal after.source must be mcp:test-server, not builtin"
            );
            assert_eq!(
                after.get("risk_level").and_then(|v| v.as_str()),
                Some("low"),
                "proposal after.risk_level must be low, not medium"
            );
            assert_eq!(
                after.get("action_type").and_then(|v| v.as_str()),
                Some("read"),
                "proposal after.action_type must be read, not external_side_effect"
            );
            assert_eq!(
                after.get("network_policy_ask").and_then(|v| v.as_bool()),
                Some(true),
            );
            // Capabilities should include target capabilities
            let caps = after.get("capabilities").and_then(|v| v.as_array());
            assert!(
                caps.is_some_and(|arr| {
                    let strs: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                    strs.contains(&"network") && strs.contains(&"read")
                }),
                "proposal after.capabilities must contain network and read"
            );
            // blocked_action.target should be mcp.call_tool (the execution entry point)
            assert_eq!(
                after
                    .get("blocked_action")
                    .and_then(|v| v.get("target"))
                    .and_then(|v| v.as_str()),
                Some("mcp.call_tool"),
            );

            (p.id.clone(), p.after.clone())
        };

        // 9. Accept the Proposal via real accept_proposal_with_state
        let accept_result = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(
            accept_result.get("success").and_then(|v| v.as_bool()),
            Some(true),
        );
        assert_eq!(
            accept_result.get("can_continue").and_then(|v| v.as_bool()),
            Some(true),
            "accept should signal can_continue=true"
        );
        assert_eq!(
            accept_result
                .get("continue_run_id")
                .and_then(|v| v.as_str()),
            Some(run_id.as_str()),
        );
        assert_eq!(
            accept_result
                .get("continue_action_id")
                .and_then(|v| v.as_str()),
            Some(action_id.as_str()),
        );

        // 10. Assert ToolPermissionStore has the target permission
        {
            let perm = state.tool_permission_store.lock().await;
            let decision = perm
                .peek(
                    target_name,
                    "mcp:test-server",
                    "low",
                    "read",
                    &["network".to_string(), "read".to_string()],
                )
                .unwrap();
            assert!(
                decision.allowed,
                "target permission must be allowed after accept"
            );
        }

        // 11. Replay the blocked action — must go through mcp.call_tool wrapper
        let replayed = crate::commands::agent::replay_action_internal(&run_id, &action_id, &state)
            .await
            .unwrap();

        // 12. Assert replay: succeeded with mock output, not blocked
        assert_eq!(
            replayed.status, "succeeded",
            "replay must succeed, got status={} error={:?}",
            replayed.status, replayed.error
        );
        assert_eq!(
            replayed.target.as_deref(),
            Some("mcp.call_tool"),
            "replay target must be the wrapper mcp.call_tool, not the tool_scope tool"
        );
        // Output must contain the mock MCP target response (stored as {"text": "..."})
        let replay_output = replayed
            .output
            .as_ref()
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            replay_output.contains("mock-mcp-ok"),
            "replay output must contain 'mock-mcp-ok', got '{}'",
            replay_output
        );
        // No error at all
        assert!(
            replayed.error.is_none(),
            "replay must have no error, got '{}'",
            replayed.error.unwrap_or_default()
        );

        // 13. No new duplicate pending network-ask proposals
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            let pending = store.list_pending_proposals(20).unwrap();
            let tp_proposals: Vec<_> = pending
                .into_iter()
                .filter(|p| p.proposal_type == ProposalType::ToolPermission)
                .collect();
            assert!(
                tp_proposals.iter().all(|p| p.id != proposal_id),
                "original proposal must no longer be pending"
            );
            // No new network_policy_ask proposals should have been created
            assert!(
                tp_proposals.iter().all(|p| {
                    p.after.get("network_policy_ask").and_then(|v| v.as_bool()) != Some(true)
                }),
                "no pending network_policy_ask proposals after accept+replay"
            );
        }
    }

    // Keep the old test as a simpler unit-level verification
    #[tokio::test]
    async fn mcp_network_ask_proposal_accept_replay_real_closure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);

        // Inject agent_run_store so replay works
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));

        // Register mcp.call_tool and a mock network-capable target tool
        {
            let mut reg = state.mcp_registry.lock().await;
            // Register mcp.call_tool wrapper
            reg.register_builtin(
                openlife_core::tool_manifest::ToolManifest {
                    name: "mcp.call_tool".to_string(),
                    id: "mcp.call_tool".to_string(),
                    description: "MCP call tool".to_string(),
                    parameters: serde_json::json!({}),
                    permission_level: "medium".to_string(),
                    risk_level: "medium".to_string(),
                    version: "1.0.0".to_string(),
                    source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                    capabilities: vec!["network".to_string(), "external_side_effect".to_string()],
                    requires_confirmation: false,
                    enabled: true,
                    declarative_only: false,
                    action_type: "external_side_effect".to_string(),
                    tags: vec!["execution".to_string(), "mcp_wrapper".to_string()],
                },
                std::sync::Arc::new(|_args| Ok("ok".to_string())),
            );
            // Register the target tool
            let target_manifest = openlife_core::tool_manifest::ToolManifest {
                name: "test_network_tool".to_string(),
                id: "test_network_tool".to_string(),
                description: "A mock network MCP tool".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "low".to_string(),
                risk_level: "low".to_string(),
                version: "1.0.0".to_string(),
                source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["network".to_string(), "read".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".to_string(),
                tags: vec!["execution".to_string()],
            };
            reg.register_builtin(
                target_manifest,
                std::sync::Arc::new(|_args| Ok("mock-ok".to_string())),
            );
        }

        // Grant permission to mcp.call_tool so only target network ask gates
        {
            let perm = state.tool_permission_store.lock().await;
            perm.grant(
                "mcp.call_tool",
                "builtin",
                "medium",
                "external_side_effect",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        }

        // Build a pending run with a blocked action matching the target
        let run_id = "run-mcp-ask-trials".to_string();
        let action_id = "action-0-mcp-ask".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("mcp.call_tool")
            .with_agent_spec_id("main.default");
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("mcp.call_tool".to_string()),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": "test_network_tool",
                    "arguments": {}
                }
            }),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "test_network_tool".to_string(),
                tool_name: "test_network_tool".to_string(),
                source: "builtin".to_string(),
                risk_level: "low".to_string(),
                capabilities: vec!["network".to_string(), "read".to_string()],
                action_type: "read".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        // Create a Proposal as network_ask_proposal would (with target metadata)
        let proposal = openlife_core::agent::AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.test_network_tool",
            serde_json::json!({
                "permission_action": "grant",
                "tool_name": "test_network_tool",
                "source": "builtin",
                "risk_level": "low",
                "action_type": "read",
                "policy": "allow_until_revoked",
                "blocked_action": {
                    "action_type": "builtin_tool",
                    "target": "mcp.call_tool",
                    "input": { "tool_name": "test_network_tool", "arguments": {} },
                    "source_run_id": run_id,
                    "step_index": 0,
                },
                "reason": "network_policy ask",
                "auto_generated": true,
                "network_policy_ask": true,
            }),
            "[NetworkPolicy ask] MCP tool needs confirmation",
            0.7,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        let mut proposal = proposal;
        proposal.run_id = Some(run_id.clone());
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
        }

        // Accept the proposal
        let accept_result = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(
            accept_result.get("success").and_then(|v| v.as_bool()),
            Some(true),
        );
        assert_eq!(
            accept_result.get("can_continue").and_then(|v| v.as_bool()),
            Some(true),
            "accept should return can_continue=true for ToolPermission Proposal"
        );
        assert_eq!(
            accept_result
                .get("continue_run_id")
                .and_then(|v| v.as_str()),
            Some(run_id.as_str()),
        );
        assert_eq!(
            accept_result
                .get("continue_action_id")
                .and_then(|v| v.as_str()),
            Some(action_id.as_str()),
        );

        // Replay must not block again
        let replayed = crate::commands::agent::replay_action_internal(&run_id, &action_id, &state)
            .await
            .unwrap();
        assert!(
            replayed.status != "needs_confirmation",
            "replay must not be needs_confirmation again, got status={}",
            replayed.status,
        );

        // Proposal must still be accepted (no duplicate)
        let proposals = {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.list_pending_proposals(10).unwrap()
        };
        assert!(
            proposals.iter().all(|p| p.id != proposal_id),
            "accepted proposal should no longer be pending"
        );
    }

    // ── Batch 1: Governed Replay ───────────────────────────────────────

    /// GOV-001: Replay restores the original AgentSpec from the run.
    #[tokio::test]
    async fn replay_restores_original_agent_spec() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));

        // Register a simple test tool
        {
            let mut reg = state.mcp_registry.lock().await;
            reg.register_builtin(
                openlife_core::tool_manifest::ToolManifest {
                    name: "test.read_only_tool".to_string(),
                    id: "test.read_only_tool".to_string(),
                    description: "A read-only test tool".to_string(),
                    parameters: serde_json::json!({}),
                    permission_level: "low".to_string(),
                    risk_level: "low".to_string(),
                    version: "1.0.0".to_string(),
                    source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                    capabilities: vec!["read".to_string()],
                    requires_confirmation: false,
                    enabled: true,
                    declarative_only: false,
                    action_type: "read".to_string(),
                    tags: vec!["test".to_string()],
                },
                std::sync::Arc::new(|_args| Ok("test-read-only-ok".to_string())),
            );
        }

        let run_id = "run-gov-001".to_string();
        let action_id = "action-gov-001".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("test.read_only_tool")
            .with_agent_spec_id("main.default");
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.read_only_tool".to_string()),
            input: serde_json::json!({}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "test.read_only_tool".to_string(),
                tool_name: "test.read_only_tool".to_string(),
                source: "builtin".to_string(),
                risk_level: "low".to_string(),
                capabilities: vec!["read".to_string()],
                action_type: "read".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let proposal = openlife_core::agent::AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.test.read_only_tool",
            serde_json::json!({
                "permission_action": "grant",
                "tool_name": "test.read_only_tool",
                "source": "builtin",
                "risk_level": "low",
                "action_type": "read",
                "policy": "allow_until_revoked",
                "blocked_action": {
                    "action_type": "builtin_tool",
                    "target": "test.read_only_tool",
                    "input": {},
                    "source_run_id": run_id,
                    "step_index": 0,
                },
            }),
            "Grant permission for test.read_only_tool",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let mut proposal = proposal;
        proposal.run_id = Some(run_id.clone());
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
        }

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let replayed = crate::commands::agent::replay_action_internal(&run_id, &action_id, &state)
            .await
            .unwrap();

        assert_eq!(
            replayed.status, "succeeded",
            "replay must succeed when AgentSpec is restored and allows the tool, got status={} error={:?}",
            replayed.status, replayed.error
        );
        assert!(
            replayed.error.is_none(),
            "replay must have no error, got '{}'",
            replayed.error.unwrap_or_default()
        );
    }

    /// GOV-002: Replay with missing AgentSpec fails closed.
    #[tokio::test]
    async fn replay_missing_agent_spec_fails_closed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));
        let events_db = temp_dir.path().join("events_missing_spec_hardening.db");
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        Arc::get_mut(&mut state).unwrap().agent_run_event_store =
            Some(Arc::new(event_store.clone()));

        let call_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let call_count_for_tool = call_count.clone();
        {
            let mut reg = state.mcp_registry.lock().await;
            reg.register_builtin(
                openlife_core::tool_manifest::ToolManifest {
                    name: "test.read_only_tool".to_string(),
                    id: "test.read_only_tool".to_string(),
                    description: "A read-only test tool".to_string(),
                    parameters: serde_json::json!({}),
                    permission_level: "low".to_string(),
                    risk_level: "low".to_string(),
                    version: "1.0.0".to_string(),
                    source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                    capabilities: vec!["read".to_string()],
                    requires_confirmation: false,
                    enabled: true,
                    declarative_only: false,
                    action_type: "read".to_string(),
                    tags: vec!["test".to_string()],
                },
                std::sync::Arc::new(move |_args| {
                    call_count_for_tool.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok("test-read-only-ok".to_string())
                }),
            );
        }

        let run_id = "run-gov-002".to_string();
        let action_id = "action-gov-002".to_string();
        // Deliberately do NOT set agent_spec_id
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("test.read_only_tool");
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.read_only_tool".to_string()),
            input: serde_json::json!({}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "test.read_only_tool".to_string(),
                tool_name: "test.read_only_tool".to_string(),
                source: "builtin".to_string(),
                risk_level: "low".to_string(),
                capabilities: vec!["read".to_string()],
                action_type: "read".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let proposal = openlife_core::agent::AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.test.read_only_tool",
            serde_json::json!({
                "permission_action": "grant",
                "tool_name": "test.read_only_tool",
                "source": "builtin",
                "risk_level": "low",
                "action_type": "read",
                "policy": "allow_until_revoked",
                "blocked_action": {
                    "action_type": "builtin_tool",
                    "target": "test.read_only_tool",
                    "input": {},
                    "source_run_id": run_id,
                    "step_index": 0,
                },
            }),
            "Grant permission for test.read_only_tool",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let mut proposal = proposal;
        proposal.run_id = Some(run_id.clone());
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
        }

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let result =
            crate::commands::agent::replay_action_internal(&run_id, &action_id, &state).await;

        assert!(
            result.is_err(),
            "replay must fail when AgentSpec is missing, got Ok({:?})",
            result.ok()
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("AgentSpec") || err.contains("governance"),
            "error must mention missing AgentSpec governance: {}",
            err
        );

        assert_eq!(
            call_count.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "missing AgentSpec replay must fail before executing the action"
        );
        let stored_run = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.get_run(&run_id).unwrap().unwrap()
        };
        assert_eq!(
            stored_run.actions[0].status, "needs_confirmation",
            "missing AgentSpec replay must not mark the original action successful"
        );
        let stored_proposal = {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.get_proposal(&proposal_id).unwrap().unwrap()
        };
        assert_eq!(
            stored_proposal.status,
            ProposalStatus::Accepted,
            "accepting ToolPermission remains the Proposal source of truth; replay failure must not invent a success status"
        );
        let events = event_store.list_events_by_run(&run_id).unwrap();
        assert!(
            events.iter().any(|event| matches!(
                event.event_type,
                openlife_core::agent::AgentRunEventType::ReplayFailed
            )),
            "missing AgentSpec replay must record ReplayFailed"
        );
        assert!(
            events.iter().all(|event| !matches!(
                event.event_type,
                openlife_core::agent::AgentRunEventType::FallbackStarted
                    | openlife_core::agent::AgentRunEventType::FallbackCompleted
            )),
            "Replay must not emit Chat fallback events"
        );
    }

    /// GOV-003: Even after ToolPermission is accepted, AgentSpec deny
    /// must still block replay.
    #[tokio::test]
    async fn accepted_tool_permission_replay_still_checks_agent_spec() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));

        let deny_spec = openlife_core::agent::AgentSpec::new(
            openlife_core::agent::AgentRoleKind::Main,
            "Governed Replay Deny",
            "Deny spec for batch 1 test GOV-003",
        )
        .with_id("batch1.deny".to_string())
        .with_denied_tools(vec!["test.read_only_tool".to_string()])
        .with_lifemodel_access();
        {
            let store = state.agent_spec_store.lock().await;
            store.create_spec(&deny_spec).unwrap();
        }

        {
            let mut reg = state.mcp_registry.lock().await;
            reg.register_builtin(
                openlife_core::tool_manifest::ToolManifest {
                    name: "test.read_only_tool".to_string(),
                    id: "test.read_only_tool".to_string(),
                    description: "A read-only test tool".to_string(),
                    parameters: serde_json::json!({}),
                    permission_level: "low".to_string(),
                    risk_level: "low".to_string(),
                    version: "1.0.0".to_string(),
                    source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                    capabilities: vec!["read".to_string()],
                    requires_confirmation: false,
                    enabled: true,
                    declarative_only: false,
                    action_type: "read".to_string(),
                    tags: vec!["test".to_string()],
                },
                std::sync::Arc::new(|_args| Ok("test-read-only-ok".to_string())),
            );
        }

        let run_id = "run-gov-003".to_string();
        let action_id = "action-gov-003".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("test.read_only_tool")
            .with_agent_spec_id("batch1.deny");
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.read_only_tool".to_string()),
            input: serde_json::json!({}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "test.read_only_tool".to_string(),
                tool_name: "test.read_only_tool".to_string(),
                source: "builtin".to_string(),
                risk_level: "low".to_string(),
                capabilities: vec!["read".to_string()],
                action_type: "read".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let proposal = openlife_core::agent::AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.test.read_only_tool",
            serde_json::json!({
                "permission_action": "grant",
                "tool_name": "test.read_only_tool",
                "source": "builtin",
                "risk_level": "low",
                "action_type": "read",
                "policy": "allow_until_revoked",
                "blocked_action": {
                    "action_type": "builtin_tool",
                    "target": "test.read_only_tool",
                    "input": {},
                    "source_run_id": run_id,
                    "step_index": 0,
                },
            }),
            "Grant permission for test.read_only_tool",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let mut proposal = proposal;
        proposal.run_id = Some(run_id.clone());
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
        }

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        // Permission is granted, but AgentSpec denies the tool → replay must block.
        let replayed = crate::commands::agent::replay_action_internal(&run_id, &action_id, &state)
            .await
            .unwrap();

        assert_eq!(
            replayed.status, "blocked",
            "replay must be blocked by AgentSpec deny even after ToolPermission accept, got status={} error={:?}",
            replayed.status, replayed.error
        );
        assert!(
            replayed.error.is_some(),
            "blocked replay must have an error message"
        );
        let err_msg = replayed.error.unwrap();
        assert!(
            err_msg.contains("AgentSpec")
                || err_msg.contains("not allowed")
                || err_msg.contains("governance"),
            "block reason must mention AgentSpec governance: {}",
            err_msg
        );
    }

    // ── Batch 3: No Fake MCP Execution ─────────────────────────────
    //
    // network_ask_accept_replay_uses_real_mcp_client:
    // NetworkPolicy default_decision=ask blocks an MCP network-capable
    // target. A ToolPermission Proposal is created. After user accepts,
    // replay executes the blocked action through the mock MCP client
    // (not through a builtin closure). We prove this by checking the
    // mock client's call_count — it must be >= 1 after replay.
    #[tokio::test]
    async fn network_ask_accept_replay_uses_real_mcp_client() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);

        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));

        // Set NetworkPolicy: enabled=true, default_decision=ask
        {
            let mut config_guard = state.config.lock().await;
            config_guard.system.network_policy = openlife_core::config::NetworkPolicy {
                enabled: true,
                default_decision: "ask".to_string(),
                domain_allowlist: vec![],
                domain_denylist: vec![],
                tool_overrides: std::collections::HashMap::new(),
            };
        }

        // Build registry with mock MCP client
        let mut reg = openlife_core::mcp::McpRegistry::new();
        let run_id = "run-batch3-replay".to_string();
        let target_name = "batch3_mcp_target";

        // Register MCP-target manifest with dummy builtin (never called)
        reg.register_builtin(
            openlife_core::tool_manifest::ToolManifest {
                name: target_name.to_string(),
                id: target_name.to_string(),
                description: "Batch 3 MCP target".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "low".to_string(),
                risk_level: "low".to_string(),
                version: "1.0.0".to_string(),
                source: openlife_core::tool_manifest::ToolSource::Mcp {
                    server_name: "batch3-server".to_string(),
                },
                capabilities: vec!["network".to_string(), "read".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".to_string(),
                tags: vec!["execution".to_string()],
            },
            std::sync::Arc::new(|_args| {
                panic!("MCP target must not execute via builtin fallback in Batch 3")
            }),
        );

        // Register mock MCP client with shared call counter
        let mock_call_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let mock_counter = mock_call_count.clone();
        let mock_client = MockMcpClient::new(
            "batch3-server",
            move |_name: String, _args: serde_json::Value| -> anyhow::Result<String> {
                mock_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok("batch3-mock-mcp-result".to_string())
            },
        );
        reg.register_mock_mcp_client("batch3-server", mock_client);

        // Replace state registry
        Arc::get_mut(&mut state).unwrap().mcp_registry = Arc::new(tokio::sync::Mutex::new(reg));

        // Grant wrapper permission
        {
            let perm = state.tool_permission_store.lock().await;
            perm.grant(
                "mcp.call_tool",
                "builtin",
                "medium",
                "external_side_effect",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        }

        // Build ActionExecutor and execute
        let executor = openlife_core::agent::action_executor::ActionExecutor::new(
            openlife_core::agent::action_executor::ActionExecutorConfig {
                allow_writes: true,
                allow_cloud: true,
                timeout_seconds: 120,
                consume_allow_once: true,
            },
        );

        let network_policy = {
            let config = state.config.lock().await;
            config.system.network_policy.clone()
        };

        let ac = openlife_core::agent::action_executor::ActionContext {
            registry: state.mcp_registry.clone(),
            permission_store: state.tool_permission_store.clone(),
            audit_store: state.mcp_audit_store.clone(),
            privacy_engine: state.privacy_engine.clone(),
            safe_paths: vec![],
            life_model: None,
            memory_store: None,
            proposal_store: state.proposal_store.clone(),
            agent_run_store: state.agent_run_store.clone(),
            event_store: None,
            network_policy: Some(network_policy),
            calendar_ics_paths: vec![],
            execution_sandbox: openlife_core::agent::execution_sandbox::ExecutionSandbox::default(),
            agent_spec: None,
        };

        let request = openlife_core::agent::action_executor::AgentActionRequest {
            action_type: "builtin_tool".to_string(),
            target: "mcp.call_tool".to_string(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": target_name,
                    "server": "batch3-server",
                    "arguments": {}
                }
            }),
            source_run_id: Some(run_id.clone()),
            step_index: 0,
        };
        let exec_result = executor.execute(request, &ac).await.unwrap();

        // First execution: needs confirmation
        assert_eq!(
            exec_result.status,
            openlife_core::agent::action_executor::ActionExecutionStatus::NeedsConfirmation,
        );
        let action = exec_result.action.clone();

        // The mock client must NOT have been called yet (execution was blocked)
        assert_eq!(
            mock_call_count.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "mock MCP client must not have been called — execution was blocked by network ask"
        );

        // Write run with pending action
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("mcp.call_tool")
            .with_agent_spec_id("main.default");
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(action.clone());
        let action_id = action.id.clone();
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        // Read the auto-generated proposal
        let proposal_id = {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            let pending = store.list_pending_proposals(20).unwrap();
            let tp_proposals: Vec<_> = pending
                .into_iter()
                .filter(|p| {
                    p.proposal_type == ProposalType::ToolPermission
                        && p.run_id.as_deref() == Some(&run_id)
                })
                .collect();
            assert_eq!(tp_proposals.len(), 1);
            let p = &tp_proposals[0];
            // proposal must use real MCP target metadata
            assert_eq!(
                p.after.get("source").and_then(|v| v.as_str()),
                Some("mcp:batch3-server"),
                "proposal must use real MCP source, not builtin"
            );
            p.id.clone()
        };

        // Accept the proposal
        let accept_result = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(
            accept_result.get("success").and_then(|v| v.as_bool()),
            Some(true),
        );
        assert_eq!(
            accept_result.get("can_continue").and_then(|v| v.as_bool()),
            Some(true),
        );

        // Verify permission is granted on the real target scope
        {
            let perm = state.tool_permission_store.lock().await;
            let decision = perm
                .peek(
                    target_name,
                    "mcp:batch3-server",
                    "low",
                    "read",
                    &["network".to_string(), "read".to_string()],
                )
                .unwrap();
            assert!(decision.allowed);
        }

        // Replay the blocked action
        let replayed = crate::commands::agent::replay_action_internal(&run_id, &action_id, &state)
            .await
            .unwrap();

        assert_eq!(
            replayed.status, "succeeded",
            "replay must succeed, got status={} error={:?}",
            replayed.status, replayed.error
        );
        assert!(
            replayed.error.is_none(),
            "replay must have no error, got: {}",
            replayed.error.unwrap_or_default()
        );

        // Verify output comes from mock MCP client, not builtin
        let replay_output = replayed
            .output
            .as_ref()
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            replay_output.contains("batch3-mock-mcp-result"),
            "replay output must contain mock MCP result, got '{}'",
            replay_output
        );

        // Prove the mock MCP client was called during replay
        assert!(
            mock_call_count.load(std::sync::atomic::Ordering::SeqCst) >= 1,
            "mock MCP client must have been called at least once during replay"
        );
    }

    /// GOV-004: Replay does not escalate tool_scope beyond original.
    #[tokio::test]
    async fn replay_does_not_escalate_tool_scope() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));

        {
            let mut reg = state.mcp_registry.lock().await;
            reg.register_builtin(
                openlife_core::tool_manifest::ToolManifest {
                    name: "test.read_only_tool".to_string(),
                    id: "test.read_only_tool".to_string(),
                    description: "A read-only test tool".to_string(),
                    parameters: serde_json::json!({}),
                    permission_level: "low".to_string(),
                    risk_level: "low".to_string(),
                    version: "1.0.0".to_string(),
                    source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                    capabilities: vec!["read".to_string()],
                    requires_confirmation: false,
                    enabled: true,
                    declarative_only: false,
                    action_type: "read".to_string(),
                    tags: vec!["test".to_string()],
                },
                std::sync::Arc::new(|_args| Ok("test-read-only-ok".to_string())),
            );
        }

        let original_tool_scope = openlife_core::agent::ToolActionScope {
            tool_id: "test.read_only_tool".to_string(),
            tool_name: "test.read_only_tool".to_string(),
            source: "builtin".to_string(),
            risk_level: "low".to_string(),
            capabilities: vec!["read".to_string()],
            action_type: "read".to_string(),
            requires_confirmation: true,
            allowed: false,
        };

        let run_id = "run-gov-004".to_string();
        let action_id = "action-gov-004".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("test.read_only_tool")
            .with_agent_spec_id("main.default");
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.read_only_tool".to_string()),
            input: serde_json::json!({"key": "original-value"}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(original_tool_scope.clone()),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let proposal = openlife_core::agent::AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.test.read_only_tool",
            serde_json::json!({
                "permission_action": "grant",
                "tool_name": "test.read_only_tool",
                "source": "builtin",
                "risk_level": "low",
                "action_type": "read",
                "policy": "allow_until_revoked",
                "blocked_action": {
                    "action_type": "builtin_tool",
                    "target": "test.read_only_tool",
                    "input": {"key": "original-value"},
                    "source_run_id": run_id,
                    "step_index": 0,
                },
            }),
            "Grant permission for test.read_only_tool",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let mut proposal = proposal;
        proposal.run_id = Some(run_id.clone());
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
        }

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let replayed = crate::commands::agent::replay_action_internal(&run_id, &action_id, &state)
            .await
            .unwrap();

        assert_eq!(
            replayed.status, "succeeded",
            "replay must succeed, got status={}",
            replayed.status
        );

        // Tool_scope must not be escalated
        let replayed_scope = replayed
            .tool_scope
            .as_ref()
            .expect("replayed action must have tool_scope");
        assert_eq!(
            replayed_scope.tool_name, original_tool_scope.tool_name,
            "replay must preserve original tool_name"
        );
        assert_eq!(
            replayed_scope.source, original_tool_scope.source,
            "replay must preserve original source"
        );
        assert_eq!(
            replayed_scope.risk_level, original_tool_scope.risk_level,
            "replay must preserve original risk_level"
        );
        assert_eq!(
            replayed_scope.action_type, original_tool_scope.action_type,
            "replay must preserve original action_type"
        );
        // Capabilities must NOT be broader than original
        for cap in &replayed_scope.capabilities {
            assert!(
                original_tool_scope.capabilities.contains(cap),
                "replay must not gain capability '{}' not in original {:?}",
                cap,
                original_tool_scope.capabilities
            );
        }
        // Target must be the original target
        assert_eq!(
            replayed.target.as_deref(),
            Some("test.read_only_tool"),
            "replay target must match original"
        );
    }

    // ── Replay typed event tests ─────────────────────────────────────

    /// Replay when AgentSpec is missing must record ReplayFailed with
    /// block_reason = ReplaySpecMissing in event payload.
    #[tokio::test]
    async fn replay_missing_agent_spec_records_typed_replay_failed_event() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));

        // Set up event store
        let events_db = temp_dir.path().join("events_replay_fail.db");
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        Arc::get_mut(&mut state).unwrap().agent_run_event_store =
            Some(Arc::new(event_store.clone()));

        {
            let mut reg = state.mcp_registry.lock().await;
            reg.register_builtin(
                openlife_core::tool_manifest::ToolManifest {
                    name: "test.read_only_tool".to_string(),
                    id: "test.read_only_tool".to_string(),
                    description: "A read-only test tool".to_string(),
                    parameters: serde_json::json!({}),
                    permission_level: "low".to_string(),
                    risk_level: "low".to_string(),
                    version: "1.0.0".to_string(),
                    source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                    capabilities: vec!["read".to_string()],
                    requires_confirmation: false,
                    enabled: true,
                    declarative_only: false,
                    action_type: "read".to_string(),
                    tags: vec!["test".to_string()],
                },
                std::sync::Arc::new(|_args| Ok("test-read-only-ok".to_string())),
            );
        }

        let run_id = "run-replay-ev-001".to_string();
        let action_id = "action-replay-ev-001".to_string();
        // Do NOT set agent_spec_id
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("test.read_only_tool");
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.read_only_tool".to_string()),
            input: serde_json::json!({}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "test.read_only_tool".to_string(),
                tool_name: "test.read_only_tool".to_string(),
                source: "builtin".to_string(),
                risk_level: "low".to_string(),
                capabilities: vec!["read".to_string()],
                action_type: "read".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let proposal = openlife_core::agent::AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.test.read_only_tool",
            serde_json::json!({
                "permission_action": "grant",
                "tool_name": "test.read_only_tool",
                "source": "builtin",
                "risk_level": "low",
                "action_type": "read",
                "policy": "allow_until_revoked",
                "blocked_action": {
                    "action_type": "builtin_tool",
                    "target": "test.read_only_tool",
                    "input": {},
                    "source_run_id": run_id,
                    "step_index": 0,
                },
            }),
            "Grant permission",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let mut proposal = proposal;
        proposal.run_id = Some(run_id.clone());
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
        }

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let result =
            crate::commands::agent::replay_action_internal(&run_id, &action_id, &state).await;
        assert!(
            result.is_err(),
            "replay must fail when AgentSpec is missing"
        );

        // Verify ReplayFailed event with ReplaySpecMissing
        let events = event_store.list_events_by_run(&run_id).unwrap();
        let failed = events
            .iter()
            .find(|e| {
                matches!(
                    e.event_type,
                    openlife_core::agent::AgentRunEventType::ReplayFailed
                )
            })
            .expect("ReplayFailed event must be recorded");

        assert_eq!(failed.payload["status"], "failed");
        assert_eq!(
            failed.payload["block_reason"],
            openlife_core::agent::action_executor::ExecutionBlockReason::ReplaySpecMissing
                .to_string()
        );
        assert_eq!(failed.payload["run_id"], run_id);
        assert_eq!(failed.payload["action_id"], action_id);
        assert_eq!(failed.payload["replay_of_action_id"], action_id);
    }

    /// Successful replay must record ReplayStarted and ReplayCompleted events.
    #[tokio::test]
    async fn replay_success_records_typed_replay_completed_event() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));

        // Add AgentSpec to the store
        let spec_id = "replay-success-spec".to_string();
        {
            let spec_store = state.agent_spec_store.lock().await;
            let spec = openlife_core::agent::types::AgentSpec::default_main_spec()
                .with_id(spec_id.clone());
            spec_store.create_spec(&spec).unwrap();
        }

        // Set up event store
        let events_db = temp_dir.path().join("events_replay_ok.db");
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        Arc::get_mut(&mut state).unwrap().agent_run_event_store =
            Some(Arc::new(event_store.clone()));

        {
            let mut reg = state.mcp_registry.lock().await;
            reg.register_builtin(
                openlife_core::tool_manifest::ToolManifest {
                    name: "test.read_only_tool".to_string(),
                    id: "test.read_only_tool".to_string(),
                    description: "A read-only test tool".to_string(),
                    parameters: serde_json::json!({}),
                    permission_level: "low".to_string(),
                    risk_level: "low".to_string(),
                    version: "1.0.0".to_string(),
                    source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                    capabilities: vec!["read".to_string()],
                    requires_confirmation: false,
                    enabled: true,
                    declarative_only: false,
                    action_type: "read".to_string(),
                    tags: vec!["test".to_string()],
                },
                std::sync::Arc::new(|_args| Ok("test-read-only-ok".to_string())),
            );
        }

        let run_id = "run-replay-ev-002".to_string();
        let action_id = "action-replay-ev-002".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("test.read_only_tool");
        run.id = run_id.clone();
        run.agent_spec_id = Some(spec_id.clone());
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.read_only_tool".to_string()),
            input: serde_json::json!({}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "test.read_only_tool".to_string(),
                tool_name: "test.read_only_tool".to_string(),
                source: "builtin".to_string(),
                risk_level: "low".to_string(),
                capabilities: vec!["read".to_string()],
                action_type: "read".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let proposal = openlife_core::agent::AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.test.read_only_tool",
            serde_json::json!({
                "permission_action": "grant",
                "tool_name": "test.read_only_tool",
                "source": "builtin",
                "risk_level": "low",
                "action_type": "read",
                "policy": "allow_until_revoked",
                "blocked_action": {
                    "action_type": "builtin_tool",
                    "target": "test.read_only_tool",
                    "input": {},
                    "source_run_id": run_id,
                    "step_index": 0,
                },
            }),
            "Grant permission",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let mut proposal = proposal;
        proposal.run_id = Some(run_id.clone());
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
        }

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let result =
            crate::commands::agent::replay_action_internal(&run_id, &action_id, &state).await;
        assert!(result.is_ok(), "replay should succeed: {:?}", result.err());

        // Verify ReplayStarted and ReplayCompleted
        let events = event_store.list_events_by_run(&run_id).unwrap();
        let started = events
            .iter()
            .find(|e| {
                matches!(
                    e.event_type,
                    openlife_core::agent::AgentRunEventType::ReplayStarted
                )
            })
            .expect("ReplayStarted event must be recorded");
        assert_eq!(started.payload["status"], "started");
        assert_eq!(started.payload["run_id"], run_id);
        assert_eq!(started.payload["agent_spec_id"], spec_id);

        let completed = events
            .iter()
            .find(|e| {
                matches!(
                    e.event_type,
                    openlife_core::agent::AgentRunEventType::ReplayCompleted
                )
            })
            .expect("ReplayCompleted event must be recorded");
        assert_eq!(completed.payload["status"], "completed");
        assert_eq!(completed.payload["run_id"], run_id);
        assert_eq!(completed.payload["action_id"], action_id);
        assert_eq!(completed.payload["agent_spec_id"], spec_id);
    }

    /// Replay with restored AgentSpec that denies the tool records
    /// AgentSpecDenied in the AgentRunEvent (tool.call_blocked).
    #[tokio::test]
    async fn replay_agentspec_denied_records_typed_block_reason() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));

        // AgentSpec that denies the tool
        let spec_id = "replay-deny-spec".to_string();
        {
            let spec_store = state.agent_spec_store.lock().await;
            let spec = openlife_core::agent::types::AgentSpec::default_main_spec()
                .with_id(spec_id.clone())
                .with_denied_tools(vec!["test.read_only_tool".to_string()]);
            spec_store.create_spec(&spec).unwrap();
        }

        // Set up event store
        let events_db = temp_dir.path().join("events_replay_deny.db");
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        Arc::get_mut(&mut state).unwrap().agent_run_event_store =
            Some(Arc::new(event_store.clone()));

        {
            let mut reg = state.mcp_registry.lock().await;
            reg.register_builtin(
                openlife_core::tool_manifest::ToolManifest {
                    name: "test.read_only_tool".to_string(),
                    id: "test.read_only_tool".to_string(),
                    description: "A read-only test tool".to_string(),
                    parameters: serde_json::json!({}),
                    permission_level: "low".to_string(),
                    risk_level: "low".to_string(),
                    version: "1.0.0".to_string(),
                    source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                    capabilities: vec!["read".to_string()],
                    requires_confirmation: false,
                    enabled: true,
                    declarative_only: false,
                    action_type: "read".to_string(),
                    tags: vec!["test".to_string()],
                },
                std::sync::Arc::new(|_args| Ok("test-read-only-ok".to_string())),
            );
        }

        let run_id = "run-replay-ev-003".to_string();
        let action_id = "action-replay-ev-003".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("test.read_only_tool");
        run.id = run_id.clone();
        run.agent_spec_id = Some(spec_id.clone());
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.read_only_tool".to_string()),
            input: serde_json::json!({}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "test.read_only_tool".to_string(),
                tool_name: "test.read_only_tool".to_string(),
                source: "builtin".to_string(),
                risk_level: "low".to_string(),
                capabilities: vec!["read".to_string()],
                action_type: "read".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let proposal = openlife_core::agent::AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.test.read_only_tool",
            serde_json::json!({
                "permission_action": "grant",
                "tool_name": "test.read_only_tool",
                "source": "builtin",
                "risk_level": "low",
                "action_type": "read",
                "policy": "allow_until_revoked",
                "blocked_action": {
                    "action_type": "builtin_tool",
                    "target": "test.read_only_tool",
                    "input": {},
                    "source_run_id": run_id,
                    "step_index": 0,
                },
            }),
            "Grant permission",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let mut proposal = proposal;
        proposal.run_id = Some(run_id.clone());
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
        }

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let result =
            crate::commands::agent::replay_action_internal(&run_id, &action_id, &state).await;
        assert!(
            result.is_ok(),
            "replay should succeed but action blocked: {:?}",
            result.err()
        );

        // The action should be blocked by AgentSpec
        let replayed = result.unwrap();
        assert_eq!(
            replayed.status, "blocked",
            "restored AgentSpec must block the tool"
        );

        // Verify ToolCallBlocked event with AgentSpecDenied
        let events = event_store.list_events_by_run(&run_id).unwrap();
        let blocked = events
            .iter()
            .find(|e| {
                matches!(
                    e.event_type,
                    openlife_core::agent::AgentRunEventType::ToolCallBlocked
                )
            })
            .expect("ToolCallBlocked event must be recorded during replay");

        assert_eq!(blocked.payload["status"], "blocked");
        assert_eq!(
            blocked.payload["block_reason"],
            openlife_core::agent::action_executor::ExecutionBlockReason::AgentSpecDenied
                .to_string()
        );
        assert!(blocked.payload["agent_spec_id"].is_string());

        // Also verify ReplayCompleted
        let completed = events
            .iter()
            .find(|e| {
                matches!(
                    e.event_type,
                    openlife_core::agent::AgentRunEventType::ReplayCompleted
                )
            })
            .expect("ReplayCompleted event must be recorded");
        // AgentSpec denied the tool → replay outcome status is "blocked"
        assert_eq!(completed.payload["status"], "blocked");
        assert_eq!(
            completed.payload["block_reason"],
            openlife_core::agent::action_executor::ExecutionBlockReason::AgentSpecDenied
                .to_string()
        );
    }

    /// replay_non_confirmation_action_records_typed_replay_failed_event
    #[tokio::test]
    async fn replay_non_confirmation_action_records_typed_replay_failed_event() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));
        let events_db = temp_dir.path().join("events_nc.db");
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        Arc::get_mut(&mut state).unwrap().agent_run_event_store =
            Some(Arc::new(event_store.clone()));

        let run_id = "run-nc-001".to_string();
        let action_id = "action-nc-001".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("test.tool");
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.tool".to_string()),
            input: serde_json::json!({}),
            output: None,
            status: "succeeded".to_string(), // NOT needs_confirmation
            permission_decision: None,
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "test.tool".to_string(),
                tool_name: "test.tool".to_string(),
                source: "builtin".to_string(),
                risk_level: "low".to_string(),
                capabilities: vec![],
                action_type: "read".to_string(),
                requires_confirmation: false,
                allowed: true,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let result =
            crate::commands::agent::replay_action_internal(&run_id, &action_id, &state).await;
        assert!(result.is_err());

        let events = event_store.list_events_by_run(&run_id).unwrap();
        let failed = events
            .iter()
            .find(|e| {
                matches!(
                    e.event_type,
                    openlife_core::agent::AgentRunEventType::ReplayFailed
                )
            })
            .expect("ReplayFailed event must be recorded");
        assert_eq!(failed.payload["status"], "failed");
        assert!(
            failed.payload["block_reason"]
                .as_str()
                .unwrap_or_default()
                .contains("invalid_arguments"),
            "block_reason should be invalid_arguments"
        );
    }

    /// replay_missing_tool_scope_records_typed_replay_failed_event
    #[tokio::test]
    async fn replay_missing_tool_scope_records_typed_replay_failed_event() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));
        let events_db = temp_dir.path().join("events_ts.db");
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        Arc::get_mut(&mut state).unwrap().agent_run_event_store =
            Some(Arc::new(event_store.clone()));

        let run_id = "run-ts-001".to_string();
        let action_id = "action-ts-001".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("test.tool");
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        // Action has needs_confirmation but NO tool_scope
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.tool".to_string()),
            input: serde_json::json!({}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: None, // missing!
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let result =
            crate::commands::agent::replay_action_internal(&run_id, &action_id, &state).await;
        assert!(result.is_err());

        let events = event_store.list_events_by_run(&run_id).unwrap();
        let failed = events
            .iter()
            .find(|e| {
                matches!(
                    e.event_type,
                    openlife_core::agent::AgentRunEventType::ReplayFailed
                )
            })
            .expect("ReplayFailed event must be recorded");
        assert_eq!(failed.payload["status"], "failed");
        assert!(
            failed.payload["block_reason"]
                .as_str()
                .unwrap_or_default()
                .contains("invalid_arguments"),
            "block_reason should be invalid_arguments"
        );
    }

    /// replay_permission_not_authorized_records_typed_replay_failed_event
    #[tokio::test]
    async fn replay_permission_not_authorized_records_typed_replay_failed_event() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));
        let events_db = temp_dir.path().join("events_pd.db");
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        Arc::get_mut(&mut state).unwrap().agent_run_event_store =
            Some(Arc::new(event_store.clone()));

        let run_id = "run-pd-001".to_string();
        let action_id = "action-pd-001".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("test.tool");
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        // Action with needs_confirmation and tool_scope, but no permission granted
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.tool".to_string()),
            input: serde_json::json!({}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "test.tool".to_string(),
                tool_name: "test.tool".to_string(),
                source: "builtin".to_string(),
                risk_level: "high".to_string(),
                capabilities: vec!["write".to_string()],
                action_type: "write".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let result =
            crate::commands::agent::replay_action_internal(&run_id, &action_id, &state).await;
        assert!(result.is_err());

        let events = event_store.list_events_by_run(&run_id).unwrap();
        let failed = events
            .iter()
            .find(|e| {
                matches!(
                    e.event_type,
                    openlife_core::agent::AgentRunEventType::ReplayFailed
                )
            })
            .expect("ReplayFailed event must be recorded");
        assert_eq!(failed.payload["status"], "failed");
        assert_eq!(
            failed.payload["block_reason"],
            openlife_core::agent::action_executor::ExecutionBlockReason::ToolPermissionDenied
                .to_string()
        );
        assert!(
            events.iter().all(|event| !matches!(
                event.event_type,
                openlife_core::agent::AgentRunEventType::FallbackStarted
                    | openlife_core::agent::AgentRunEventType::FallbackCompleted
            )),
            "ToolPermission denied replay must not emit Chat fallback events"
        );
        let stored_run = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.get_run(&run_id).unwrap().unwrap()
        };
        assert_eq!(
            stored_run.actions[0].status, "needs_confirmation",
            "ToolPermission denied replay must not execute or rewrite the action"
        );
    }

    /// Replay typed payloads must keep a stable contract without raw prompt text.
    #[tokio::test]
    async fn replay_typed_event_payload_contract_is_stable_and_redacted() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));
        let events_db = temp_dir.path().join("events_payload_contract.db");
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        Arc::get_mut(&mut state).unwrap().agent_run_event_store =
            Some(Arc::new(event_store.clone()));

        let spec_id = "replay-payload-contract-spec".to_string();
        {
            let spec_store = state.agent_spec_store.lock().await;
            let spec = openlife_core::agent::types::AgentSpec::default_main_spec()
                .with_id(spec_id.clone());
            spec_store.create_spec(&spec).unwrap();
        }
        {
            let mut reg = state.mcp_registry.lock().await;
            reg.register_builtin(
                openlife_core::tool_manifest::ToolManifest {
                    name: "test.payload_contract".to_string(),
                    id: "test.payload_contract".to_string(),
                    description: "payload contract tool".to_string(),
                    parameters: serde_json::json!({}),
                    permission_level: "low".to_string(),
                    risk_level: "low".to_string(),
                    version: "1.0.0".to_string(),
                    source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                    capabilities: vec!["read".to_string()],
                    requires_confirmation: false,
                    enabled: true,
                    declarative_only: false,
                    action_type: "read".to_string(),
                    tags: vec![],
                },
                std::sync::Arc::new(|_args| Ok("payload-ok".to_string())),
            );
        }

        let run_id = "run-replay-payload-contract".to_string();
        let action_id = "action-replay-payload-contract".to_string();
        let secret_prompt = "RAW_SECRET_PROMPT_SHOULD_NOT_LEAK";
        let mut run =
            openlife_core::agent::AgentRun::new_tool_execution_run("test.payload_contract")
                .with_agent_spec_id(&spec_id);
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.payload_contract".to_string()),
            input: serde_json::json!({"prompt": secret_prompt}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "test.payload_contract".to_string(),
                tool_name: "test.payload_contract".to_string(),
                source: "builtin".to_string(),
                risk_level: "low".to_string(),
                capabilities: vec!["read".to_string()],
                action_type: "read".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }
        {
            let perm = state.tool_permission_store.lock().await;
            perm.grant(
                "test.payload_contract",
                "builtin",
                "low",
                "read",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        }

        crate::commands::agent::replay_action_internal(&run_id, &action_id, &state)
            .await
            .unwrap();

        let events = event_store.list_events_by_run(&run_id).unwrap();
        for event in events.iter().filter(|event| {
            matches!(
                event.event_type,
                openlife_core::agent::AgentRunEventType::ReplayStarted
                    | openlife_core::agent::AgentRunEventType::ReplayCompleted
                    | openlife_core::agent::AgentRunEventType::ReplayFailed
            )
        }) {
            assert_eq!(event.payload["run_id"], run_id);
            assert_eq!(event.payload["original_run_id"], run_id);
            assert_eq!(event.payload["action_id"], action_id);
            assert_eq!(event.payload["replay_of_action_id"], action_id);
            assert!(event.payload.get("proposal_id").is_some());
            assert!(
                event.payload["agent_spec_id"].is_string()
                    || event.payload["agent_spec_id"].is_null()
            );
            let serialized = serde_json::to_string(&event.payload).unwrap();
            assert!(
                !serialized.contains(secret_prompt),
                "Replay payload must not leak raw prompt/context: {}",
                serialized
            );
        }
    }

    /// NetworkPolicy hard deny must fail closed during replay without Chat fallback.
    #[tokio::test]
    async fn replay_network_policy_denied_fails_closed_without_fallback() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));
        let events_db = temp_dir.path().join("events_network_denied_replay.db");
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        Arc::get_mut(&mut state).unwrap().agent_run_event_store =
            Some(Arc::new(event_store.clone()));
        {
            let mut config = state.config.lock().await;
            config.system.network_policy = openlife_core::config::NetworkPolicy {
                enabled: false,
                default_decision: "allow".to_string(),
                domain_allowlist: vec![],
                domain_denylist: vec![],
                tool_overrides: std::collections::HashMap::new(),
            };
        }
        let spec_id = "replay-network-denied-spec".to_string();
        {
            let spec_store = state.agent_spec_store.lock().await;
            spec_store
                .create_spec(
                    &openlife_core::agent::types::AgentSpec::default_main_spec()
                        .with_id(spec_id.clone()),
                )
                .unwrap();
        }

        let run_id = "run-replay-network-denied".to_string();
        let action_id = "action-replay-network-denied".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("web.fetch")
            .with_agent_spec_id(&spec_id);
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("web.fetch".to_string()),
            input: serde_json::json!({"url": "https://example.com/private"}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "web.fetch".to_string(),
                tool_name: "web.fetch".to_string(),
                source: "builtin".to_string(),
                risk_level: "medium".to_string(),
                capabilities: vec!["network".to_string(), "read".to_string()],
                action_type: "read".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }
        {
            let perm = state.tool_permission_store.lock().await;
            perm.grant(
                "web.fetch",
                "builtin",
                "medium",
                "read",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        }

        let replayed = crate::commands::agent::replay_action_internal(&run_id, &action_id, &state)
            .await
            .unwrap();

        assert_eq!(replayed.status, "blocked");
        assert!(replayed.error.is_some());
        let events = event_store.list_events_by_run(&run_id).unwrap();
        let completed = events
            .iter()
            .find(|event| {
                matches!(
                    event.event_type,
                    openlife_core::agent::AgentRunEventType::ReplayCompleted
                )
            })
            .expect("ReplayCompleted blocked outcome must be recorded");
        assert_eq!(completed.payload["status"], "blocked");
        assert_eq!(
            completed.payload["block_reason"],
            openlife_core::agent::action_executor::ExecutionBlockReason::NetworkPolicyDenied
                .to_string()
        );
        assert!(
            events.iter().all(|event| !matches!(
                event.event_type,
                openlife_core::agent::AgentRunEventType::FallbackStarted
                    | openlife_core::agent::AgentRunEventType::FallbackCompleted
            )),
            "NetworkPolicy denied replay must not fallback to Chat"
        );
    }

    /// ExecutionSandbox hard deny must fail closed during replay without success outcome.
    #[tokio::test]
    async fn replay_execution_sandbox_denied_fails_closed_without_success_outcome() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));
        let events_db = temp_dir.path().join("events_sandbox_denied_replay.db");
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        Arc::get_mut(&mut state).unwrap().agent_run_event_store =
            Some(Arc::new(event_store.clone()));
        {
            let mut config = state.config.lock().await;
            config.system.execution_sandbox.bash_enabled = false;
        }
        {
            let mut registry = state.mcp_registry.lock().await;
            registry.register_builtin(
                openlife_core::tool_manifest::ToolManifest {
                    name: "shell.run".to_string(),
                    id: "shell.run".to_string(),
                    description: "Shell execution test manifest".to_string(),
                    parameters: serde_json::json!({}),
                    permission_level: "high".to_string(),
                    risk_level: "high".to_string(),
                    version: "1.0.0".to_string(),
                    source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                    capabilities: vec!["shell".to_string(), "filesystem".to_string()],
                    requires_confirmation: true,
                    enabled: true,
                    declarative_only: false,
                    action_type: "external_side_effect".to_string(),
                    tags: vec!["execution".to_string()],
                },
                std::sync::Arc::new(|_args| Ok("shell-should-not-execute".to_string())),
            );
            registry.set_builtin_manifest_enabled("shell.run", true);
        }
        let spec_id = "replay-sandbox-denied-spec".to_string();
        {
            let spec_store = state.agent_spec_store.lock().await;
            spec_store
                .create_spec(
                    &openlife_core::agent::types::AgentSpec::default_main_spec()
                        .with_id(spec_id.clone()),
                )
                .unwrap();
        }

        let run_id = "run-replay-sandbox-denied".to_string();
        let action_id = "action-replay-sandbox-denied".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("shell.run")
            .with_agent_spec_id(&spec_id);
        run.id = run_id.clone();
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("shell.run".to_string()),
            input: serde_json::json!({"command": "pwd"}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "shell.run".to_string(),
                tool_name: "shell.run".to_string(),
                source: "builtin".to_string(),
                risk_level: "high".to_string(),
                capabilities: vec!["shell".to_string(), "filesystem".to_string()],
                action_type: "external_side_effect".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }
        {
            let perm = state.tool_permission_store.lock().await;
            perm.grant(
                "shell.run",
                "builtin",
                "high",
                "external_side_effect",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        }

        let replayed = crate::commands::agent::replay_action_internal(&run_id, &action_id, &state)
            .await
            .unwrap();

        assert_eq!(replayed.status, "blocked");
        assert!(replayed.error.is_some());
        let events = event_store.list_events_by_run(&run_id).unwrap();
        let completed = events
            .iter()
            .find(|event| {
                matches!(
                    event.event_type,
                    openlife_core::agent::AgentRunEventType::ReplayCompleted
                )
            })
            .expect("ReplayCompleted blocked outcome must be recorded");
        assert_eq!(completed.payload["status"], "blocked");
        assert_eq!(
            completed.payload["block_reason"],
            openlife_core::agent::action_executor::ExecutionBlockReason::SandboxDenied.to_string()
        );
        assert_ne!(
            completed.payload["status"], "completed",
            "sandbox-denied replay must not write a successful outcome"
        );
        assert!(
            events.iter().all(|event| !matches!(
                event.event_type,
                openlife_core::agent::AgentRunEventType::FallbackStarted
                    | openlife_core::agent::AgentRunEventType::FallbackCompleted
            )),
            "ExecutionSandbox denied replay must not fallback to Chat"
        );
    }

    /// replay_blocked_outcome_records_typed_replay_outcome
    #[tokio::test]
    async fn replay_blocked_outcome_records_typed_replay_outcome() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state).unwrap().agent_run_store = Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        )));

        let spec_id = "replay-block-outcome".to_string();
        {
            let spec_store = state.agent_spec_store.lock().await;
            let spec = openlife_core::agent::types::AgentSpec::default_main_spec()
                .with_id(spec_id.clone())
                .with_denied_tools(vec!["test.tool".to_string()]); // will block on replay
            spec_store.create_spec(&spec).unwrap();
        }

        let events_db = temp_dir.path().join("events_bo.db");
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        Arc::get_mut(&mut state).unwrap().agent_run_event_store =
            Some(Arc::new(event_store.clone()));

        {
            let mut reg = state.mcp_registry.lock().await;
            reg.register_builtin(
                openlife_core::tool_manifest::ToolManifest {
                    name: "test.tool".to_string(),
                    id: "test.tool".to_string(),
                    description: "test".to_string(),
                    parameters: serde_json::json!({}),
                    permission_level: "low".to_string(),
                    risk_level: "low".to_string(),
                    version: "1.0.0".to_string(),
                    source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                    capabilities: vec!["read".to_string()],
                    requires_confirmation: false,
                    enabled: true,
                    declarative_only: false,
                    action_type: "read".to_string(),
                    tags: vec![],
                },
                std::sync::Arc::new(|_args| Ok("ok".to_string())),
            );
        }

        let run_id = "run-bo-001".to_string();
        let action_id = "action-bo-001".to_string();
        let mut run = openlife_core::agent::AgentRun::new_tool_execution_run("test.tool");
        run.id = run_id.clone();
        run.agent_spec_id = Some(spec_id.clone());
        run.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
        run.actions.push(openlife_core::agent::AgentAction {
            id: action_id.clone(),
            action_type: "builtin_tool".to_string(),
            target: Some("test.tool".to_string()),
            input: serde_json::json!({}),
            output: None,
            status: "needs_confirmation".to_string(),
            permission_decision: Some("proposal_required".to_string()),
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(openlife_core::agent::ToolActionScope {
                tool_id: "test.tool".to_string(),
                tool_name: "test.tool".to_string(),
                source: "builtin".to_string(),
                risk_level: "low".to_string(),
                capabilities: vec!["read".to_string()],
                action_type: "read".to_string(),
                requires_confirmation: true,
                allowed: false,
            }),
        });
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let proposal = openlife_core::agent::AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.test.tool",
            serde_json::json!({
                "permission_action": "grant",
                "tool_name": "test.tool",
                "source": "builtin",
                "risk_level": "low",
                "action_type": "read",
                "policy": "allow_until_revoked",
                "blocked_action": {
                    "action_type": "builtin_tool",
                    "target": "test.tool",
                    "input": {},
                    "source_run_id": run_id,
                    "step_index": 0,
                },
            }),
            "Grant permission",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let mut proposal = proposal;
        proposal.run_id = Some(run_id.clone());
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
        }

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let result =
            crate::commands::agent::replay_action_internal(&run_id, &action_id, &state).await;
        assert!(result.is_ok());

        let replayed = result.unwrap();
        assert_eq!(replayed.status, "blocked");

        // Check replay outcome event has block_reason
        let events = event_store.list_events_by_run(&run_id).unwrap();
        let completed = events
            .iter()
            .find(|e| {
                matches!(
                    e.event_type,
                    openlife_core::agent::AgentRunEventType::ReplayCompleted
                )
            })
            .expect("ReplayCompleted event must be recorded");
        assert_eq!(completed.payload["status"], "blocked");
        assert!(
            !completed.payload["block_reason"].is_null(),
            "blocked replay outcome must carry block_reason"
        );
        assert!(
            completed.payload["block_reason"].as_str().is_some(),
            "block_reason should be a string"
        );
    }
}
