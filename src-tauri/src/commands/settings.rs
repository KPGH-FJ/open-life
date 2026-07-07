use crate::errors::AppError;
use openlife_core::config::AppConfig;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::{
    chat_completions_url, default_base_for_provider, effective_api_key, provider_label,
};
use openlife_core::mcp_audit::{AuditExport, AuditKeyConfig, KeyMode};
use openlife_core::privacy::PrivacyPolicy;
use openlife_core::scheduler::InferenceScheduler;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri::State;

use crate::life_model_materializer_guard::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::storage::{
    app_data_dir, mcp_audit_keyring_path, privacy_policy_path, save_mcp_audit_keyring_to_path,
    save_privacy_policy_to_path,
};
use crate::AppState;
use crate::{memory_gateway, persist_life_model};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportRequest {
    pub purpose: String,
    pub explicit_user_intent: bool,
    pub create_pre_change_snapshot: bool,
    pub import_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DangerActionConfirmationEvidence {
    pub action_type: String,
    pub preflight_id: String,
    pub confirmation_phrase: String,
    pub confirmation_scope_digest: String,
    pub safe_mode: bool,
    #[serde(default)]
    pub target_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DangerActionPreflightView {
    pub action_type: String,
    pub risk_tier: String,
    pub scope_summary: String,
    pub data_categories: Vec<String>,
    pub writes_durable_state: bool,
    pub privacy_sensitive: bool,
    pub external_transmission: String,
    pub dry_run_available: bool,
    pub backup_status: String,
    pub requires_typed_confirmation: bool,
    pub confirmation_required: bool,
    pub confirmation_phrase: Option<String>,
    pub confirmation_scope_digest: String,
    pub preflight_id: String,
    pub affected_item_count: usize,
    pub affected_item_digest: String,
    pub final_action_enabled: bool,
    pub safe_mode_blocked: bool,
    pub blocking_reasons: Vec<String>,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DangerActionPreflightScope {
    target_ids: Vec<String>,
    affected_count: Option<usize>,
}

fn validate_scope_target_ids(target_ids: &[String]) -> Result<Vec<String>, AppError> {
    if target_ids.len() > 100 {
        return Err(AppError::permission(
            "danger action preflight target scope is too large",
        ));
    }
    let mut safe = Vec::with_capacity(target_ids.len());
    for target_id in target_ids {
        if target_id.is_empty()
            || target_id.len() > 128
            || target_id.trim() != target_id
            || target_id.chars().any(char::is_control)
        {
            return Err(AppError::permission(
                "danger action preflight target scope is not metadata-safe",
            ));
        }
        safe.push(target_id.clone());
    }
    safe.sort();
    safe.dedup();
    Ok(safe)
}

fn danger_action_confirmation_phrase(action_type: &str) -> Option<&'static str> {
    match action_type {
        "data_import_overwrite" => Some("IMPORT"),
        "mcp_audit_cleanup" => Some("CLEANUP"),
        "mcp_audit_key_rotation" => Some("ROTATE"),
        "agent_run_delete" => Some("DELETE RUN"),
        "agent_run_bulk_delete" => Some("DELETE RUNS"),
        "vector_rebuild" => Some("REBUILD"),
        _ => None,
    }
}

fn danger_action_scope_digest(
    action_type: &str,
    target_ids: &[String],
    affected_count: usize,
) -> Result<String, AppError> {
    let canonical = serde_json::json!({
        "action_type": action_type,
        "affected_count": affected_count,
        "target_id_count": target_ids.len(),
        "target_ids": target_ids,
    });
    let bytes = serde_json::to_vec(&canonical)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!(
        "bytes:{} hash:sha256:{:x}",
        bytes.len(),
        hasher.finalize()
    ))
}

fn danger_action_preflight_id(action_type: &str, scope_digest: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(action_type.as_bytes());
    hasher.update(b"\n");
    hasher.update(scope_digest.as_bytes());
    format!("danger-preflight:sha256:{:x}", hasher.finalize())
}

#[cfg(test)]
fn danger_action_preflight_for_action(
    action_type: &str,
    safe_mode: bool,
) -> Result<DangerActionPreflightView, AppError> {
    danger_action_preflight_for_action_scoped(
        action_type,
        safe_mode,
        DangerActionPreflightScope::default(),
    )
}

fn danger_action_preflight_for_action_scoped(
    action_type: &str,
    safe_mode: bool,
    scope: DangerActionPreflightScope,
) -> Result<DangerActionPreflightView, AppError> {
    let safe_target_ids = validate_scope_target_ids(&scope.target_ids)?;
    let affected_count = scope
        .affected_count
        .unwrap_or(safe_target_ids.len())
        .max(safe_target_ids.len());
    let scope_digest = danger_action_scope_digest(action_type, &safe_target_ids, affected_count)?;
    let confirmation_phrase = danger_action_confirmation_phrase(action_type).map(str::to_string);
    let confirmation_required = confirmation_phrase.is_some();
    let preflight_id = danger_action_preflight_id(action_type, &scope_digest);
    let mut view = match action_type {
        "data_export" => DangerActionPreflightView {
            action_type: "data_export".into(),
            risk_tier: "high".into(),
            scope_summary:
                "导出本地 LifeModel、聊天记录和向量记忆到用户选择的本地 JSON 文件。".into(),
            data_categories: vec!["life_model".into(), "messages".into(), "vectors".into()],
            writes_durable_state: false,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "not_required_read_only".into(),
            requires_typed_confirmation: confirmation_required,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:export_all_data".into(),
                "governance:slice5b_danger_action_preflight".into(),
            ],
        },
        "data_import_overwrite" => DangerActionPreflightView {
            action_type: "data_import_overwrite".into(),
            risk_tier: "critical".into(),
            scope_summary:
                "读取用户选择的 OpenLife JSON 备份，并覆盖当前 LifeModel、聊天记录和向量记忆。"
                    .into(),
            data_categories: vec!["life_model".into(), "messages".into(), "vectors".into()],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "will_create_on_execute".into(),
            requires_typed_confirmation: confirmation_required,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:import_all_data".into(),
                "governed_request:create_pre_change_snapshot_on_execute".into(),
                "governance:slice5b_danger_action_preflight".into(),
            ],
        },
        "mcp_audit_export" => DangerActionPreflightView {
            action_type: "mcp_audit_export".into(),
            risk_tier: "high".into(),
            scope_summary:
                "导出最近 MCP 审计日志到用户选择的本地 JSON 文件，可能包含工具名称、工具输入参数文本、工具执行结果文本、执行状态和审计元数据。"
                    .into(),
            data_categories: vec![
                "mcp_audit_metadata".into(),
                "tool_metadata".into(),
                "tool_input_text".into(),
                "tool_output_text".into(),
            ],
            writes_durable_state: false,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "not_required_read_only".into(),
            requires_typed_confirmation: confirmation_required,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:export_mcp_audit_logs".into(),
                "governance:slice5b_danger_action_preflight".into(),
            ],
        },
        "mcp_audit_cleanup" => DangerActionPreflightView {
            action_type: "mcp_audit_cleanup".into(),
            risk_tier: "high".into(),
            scope_summary: "删除超过保留期限的本地 MCP 审计日志。".into(),
            data_categories: vec!["mcp_audit_metadata".into(), "tool_metadata".into()],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "none".into(),
            requires_typed_confirmation: confirmation_required,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:cleanup_mcp_audit_logs".into(),
                "governance:slice5b_danger_action_preflight".into(),
            ],
        },
        "mcp_audit_key_rotation" => DangerActionPreflightView {
            action_type: "mcp_audit_key_rotation".into(),
            risk_tier: "critical".into(),
            scope_summary:
                "轮换本地 MCP 审计加密 epoch；历史 epoch 会保留以便旧审计日志继续可读。".into(),
            data_categories: vec!["mcp_audit_metadata".into(), "mcp_audit_key_epochs".into()],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "historical_key_epochs_retained".into(),
            requires_typed_confirmation: confirmation_required,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:rotate_mcp_audit_key".into(),
                "governance:slice5b_danger_action_preflight".into(),
            ],
        },
        "agent_run_delete" => DangerActionPreflightView {
            action_type: "agent_run_delete".into(),
            risk_tier: "high".into(),
            scope_summary:
                "删除选中的 AgentRun 运行记录；预检只保留数量和 id digest，不展开 transcript、tool input 或模型输出。"
                    .into(),
            data_categories: vec!["agent_run_metadata".into(), "run_trace_metadata".into()],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "soft_delete_trash_view".into(),
            requires_typed_confirmation: confirmation_required,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:delete_agent_run".into(),
                "governance:slice5c_danger_zone_consolidation".into(),
            ],
        },
        "agent_run_bulk_delete" => DangerActionPreflightView {
            action_type: "agent_run_bulk_delete".into(),
            risk_tier: "high".into(),
            scope_summary:
                "批量删除选中的 AgentRun 运行记录；预检只保留 bounded 数量和 id digest，不展开 transcript、tool input 或模型输出。"
                    .into(),
            data_categories: vec!["agent_run_metadata".into(), "run_trace_metadata".into()],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "soft_delete_trash_view".into(),
            requires_typed_confirmation: confirmation_required,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:delete_agent_run".into(),
                "governance:slice5c_danger_zone_consolidation".into(),
            ],
        },
        "vector_rebuild" => DangerActionPreflightView {
            action_type: "vector_rebuild".into(),
            risk_tier: "high".into(),
            scope_summary:
                "基于现有聊天消息重建本地向量索引；预检只展示消息数量和 scope digest，不展示原始消息或向量内容。"
                    .into(),
            data_categories: vec!["messages_metadata".into(), "vectors".into()],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "rollback_previous_vectors_on_failure".into(),
            requires_typed_confirmation: confirmation_required,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:rebuild_memory_index".into(),
                "governance:slice5c_danger_zone_consolidation".into(),
            ],
        },
        _ => {
            return Err(AppError::permission(
                "unsupported danger action preflight action type",
            ));
        }
    };

    if safe_mode && view.writes_durable_state {
        view.final_action_enabled = false;
        view.safe_mode_blocked = true;
        view.blocking_reasons
            .push("safe_mode_blocks_durable_write".into());
        view.source_refs.push("safe_mode:blocked".into());
    }
    view.source_refs
        .push(format!("scope_digest:{}", view.confirmation_scope_digest));

    Ok(view)
}

pub(crate) async fn danger_action_safe_mode_active(state: &Arc<AppState>) -> bool {
    if !state.startup_warnings.is_empty() {
        return true;
    }
    let store = state.vector_store.lock().await;
    store
        .integrity_report()
        .map(|report| report.corrupt_embedding_count > 0)
        .unwrap_or(true)
}

pub(crate) async fn require_danger_action_confirmation(
    action_type: &str,
    target_ids: &[String],
    affected_count: Option<usize>,
    evidence: Option<&DangerActionConfirmationEvidence>,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    if danger_action_safe_mode_active(state).await {
        return Err(AppError::permission(
            "danger action blocked because Safe Mode is active",
        ));
    }

    let scope = DangerActionPreflightScope {
        target_ids: target_ids.to_vec(),
        affected_count,
    };
    let expected = danger_action_preflight_for_action_scoped(action_type, false, scope)?;
    if !expected.confirmation_required {
        return Ok(());
    }

    let evidence = evidence.ok_or_else(|| {
        AppError::permission("danger action requires confirmed preflight evidence")
    })?;
    if evidence.safe_mode {
        return Err(AppError::permission(
            "danger action confirmation evidence was produced under Safe Mode",
        ));
    }
    if evidence.action_type != expected.action_type
        || evidence.preflight_id != expected.preflight_id
        || evidence.confirmation_scope_digest != expected.confirmation_scope_digest
        || Some(evidence.confirmation_phrase.as_str()) != expected.confirmation_phrase.as_deref()
    {
        return Err(AppError::permission(
            "danger action confirmation evidence does not match preflight scope",
        ));
    }

    let safe_evidence_targets = validate_scope_target_ids(&evidence.target_ids)?;
    let expected_targets = validate_scope_target_ids(target_ids)?;
    if safe_evidence_targets != expected_targets {
        return Err(AppError::permission(
            "danger action confirmation target scope does not match final action",
        ));
    }

    Ok(())
}

#[tauri::command]
pub async fn get_danger_action_preflight(
    action_type: String,
    safe_mode: Option<bool>,
    target_ids: Option<Vec<String>>,
    affected_count: Option<usize>,
    state: State<'_, Arc<AppState>>,
) -> Result<DangerActionPreflightView, AppError> {
    let mut effective_safe_mode = safe_mode.unwrap_or(false);
    if danger_action_safe_mode_active(state.inner()).await {
        effective_safe_mode = true;
    }
    let target_ids = target_ids.unwrap_or_default();
    let effective_affected_count = if action_type == "vector_rebuild" && affected_count.is_none() {
        let store = state.memory_store.lock().await;
        Some(store.export_all_messages().map_err(AppError::from)?.len())
    } else {
        affected_count
    };
    danger_action_preflight_for_action_scoped(
        &action_type,
        effective_safe_mode,
        DangerActionPreflightScope {
            target_ids,
            affected_count: effective_affected_count,
        },
    )
}

impl GovernedDataImportRequest {
    #[cfg(test)]
    fn manual_restore_all_targets() -> Self {
        Self {
            purpose: "manual_restore".into(),
            explicit_user_intent: true,
            create_pre_change_snapshot: true,
            import_targets: vec!["life_model".into(), "messages".into(), "vectors".into()],
        }
    }

    fn is_valid(&self) -> bool {
        self.explicit_user_intent
            && self.create_pre_change_snapshot
            && matches!(self.purpose.as_str(), "manual_restore" | "migration")
            && !self.import_targets.is_empty()
            && self
                .import_targets
                .iter()
                .all(|target| matches!(target.as_str(), "life_model" | "messages" | "vectors"))
    }
}

fn require_governed_data_import_request(
    import_request: Option<&GovernedDataImportRequest>,
) -> Result<&GovernedDataImportRequest, AppError> {
    if let Some(request) = import_request.filter(|request| request.is_valid()) {
        Ok(request)
    } else {
        Err(AppError::permission(
            "import_all_data requires an explicit governed import request with purpose manual_restore or migration, explicitUserIntent=true, createPreChangeSnapshot=true, and supported importTargets.",
        ))
    }
}

fn hash_json_value(value: &serde_json::Value) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn validate_import_payload_shape(payload: &serde_json::Value) -> Result<(), AppError> {
    let object = payload
        .as_object()
        .ok_or_else(|| AppError::external("导入 payload 必须是 JSON object"))?;
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "version" | "app_version" | "exported_at" | "life_model" | "messages" | "vectors"
        ) {
            return Err(AppError::permission(format!(
                "import_all_data received unsupported import target: {key}"
            )));
        }
    }
    if !object.contains_key("life_model") {
        return Err(AppError::external("导入 payload 缺少 life_model"));
    }
    Ok(())
}

fn validate_import_targets_cover_payload(
    payload: &serde_json::Value,
    request: &GovernedDataImportRequest,
) -> Result<(), AppError> {
    let object = payload
        .as_object()
        .ok_or_else(|| AppError::external("导入 payload 必须是 JSON object"))?;
    for target in ["life_model", "messages", "vectors"] {
        if object.contains_key(target) && !request.import_targets.iter().any(|item| item == target)
        {
            return Err(AppError::permission(format!(
                "import_all_data payload contains {target}, but the governed import request did not include that import target."
            )));
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct LastModelError {
    pub message: String,
    pub phase: String,
    pub timestamp: String,
}

#[tauri::command]
pub async fn get_last_model_error(
    state: State<'_, Arc<AppState>>,
) -> Result<Option<LastModelError>, AppError> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let runs = store.list_runs(10, 0).map_err(AppError::from)?;
        let last_error = runs
            .iter()
            .find(|r| r.error.is_some())
            .and_then(|r| r.error.as_ref())
            .map(|e| LastModelError {
                message: e.message.clone(),
                phase: e.phase.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        Ok(last_error)
    } else {
        Ok(None)
    }
}

/// Mask for sensitive API keys sent to the frontend.
const KEY_MASK: &str = "***";

fn resolve_masked_api_key(submitted_key: &str, current_key: &str) -> String {
    if submitted_key.trim().is_empty() || submitted_key == KEY_MASK {
        current_key.to_string()
    } else {
        submitted_key.to_string()
    }
}

#[tauri::command]
pub async fn get_config(state: State<'_, Arc<AppState>>) -> Result<AppConfig, AppError> {
    let mut cfg = state.config.lock().await.clone();
    // Sanitize API keys before sending to frontend
    if !cfg.llm.openai_key.is_empty() {
        cfg.llm.openai_key = KEY_MASK.to_string();
    }
    Ok(cfg)
}

#[tauri::command]
pub async fn save_config(
    mut config: AppConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    config.normalize_provider_from_base();
    let data_dir = app_data_dir();
    let config_path = data_dir.join("config.yaml");

    // Preserve existing API key if the submitted config has a mask or empty key
    let current_key = {
        let cfg = state.config.lock().await;
        cfg.llm.openai_key.clone()
    };
    if config.llm.openai_key.is_empty() || config.llm.openai_key == KEY_MASK {
        config.llm.openai_key = current_key;
    }

    config.save(&config_path).map_err(AppError::from)?;
    let mut cfg = state.config.lock().await;
    *cfg = config.clone();
    let mut scheduler = state.scheduler.lock().await;
    let mut new_scheduler = InferenceScheduler::new(
        config.local_model,
        config.prefer_local_model,
        config.llm.provider,
        config.llm.openai_base,
        config.llm.openai_key,
        config.llm.chat_model,
        config.llm.embedding_model,
        config.llm.embedding_enabled,
    );

    // ModelRouter is now on the graduated runtime path.
    let router = openlife_core::agent::ModelRouter::new();
    new_scheduler = new_scheduler.with_model_router(router);
    eprintln!("[Scheduler] ModelRouter enabled (graduated runtime path)");

    *scheduler = new_scheduler;
    Ok(())
}

#[tauri::command]
pub async fn export_all_data(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    export_all_data_with_state(state.inner()).await
}

async fn export_all_data_with_state(state: &Arc<AppState>) -> Result<serde_json::Value, AppError> {
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    let messages = {
        let store = state.memory_store.lock().await;
        store.export_all_messages().map_err(AppError::from)?
    };
    let vectors = {
        let store = state.vector_store.lock().await;
        store.export_all_chunks().map_err(AppError::from)?
    };
    Ok(serde_json::json!({
        "version": "1.0",
        "app_version": env!("CARGO_PKG_VERSION"),
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "life_model": life_model,
        "messages": messages,
        "vectors": vectors,
    }))
}

#[tauri::command]
pub async fn import_all_data(
    payload: serde_json::Value,
    import_request: Option<GovernedDataImportRequest>,
    confirmation_evidence: Option<DangerActionConfirmationEvidence>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    require_danger_action_confirmation(
        "data_import_overwrite",
        &[],
        None,
        confirmation_evidence.as_ref(),
        state.inner(),
    )
    .await?;
    import_all_data_with_state_gated(payload, state.inner(), import_request).await
}

#[cfg(test)]
async fn import_all_data_with_state(
    payload: serde_json::Value,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    import_all_data_with_state_gated(payload, state, None).await
}

#[cfg(test)]
async fn import_all_data_with_state_for_governed_import(
    payload: serde_json::Value,
    state: &Arc<AppState>,
    import_request: GovernedDataImportRequest,
) -> Result<serde_json::Value, AppError> {
    import_all_data_with_state_gated(payload, state, Some(import_request)).await
}

async fn import_all_data_with_state_gated(
    payload: serde_json::Value,
    state: &Arc<AppState>,
    import_request: Option<GovernedDataImportRequest>,
) -> Result<serde_json::Value, AppError> {
    let request = require_governed_data_import_request(import_request.as_ref())?;
    import_all_data_governed_operation(payload, state, request).await
}

async fn import_all_data_governed_operation(
    payload: serde_json::Value,
    state: &Arc<AppState>,
    request: &GovernedDataImportRequest,
) -> Result<serde_json::Value, AppError> {
    validate_import_payload_shape(&payload)?;
    validate_import_targets_cover_payload(&payload, request)?;
    let import_payload_hash = hash_json_value(&payload)?;
    let life_model: LifeModel = serde_json::from_value(
        payload
            .get("life_model")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(|e| AppError::external(format!("解析 life_model 失败: {}", e)))?;
    let messages: Vec<openlife_core::memory::ExportedMessage> = serde_json::from_value(
        payload
            .get("messages")
            .cloned()
            .unwrap_or(serde_json::Value::Array(vec![])),
    )
    .map_err(|e| AppError::external(format!("解析 messages 失败: {}", e)))?;
    let vectors: Vec<openlife_core::vectors::ExportedVectorChunk> = serde_json::from_value(
        payload
            .get("vectors")
            .cloned()
            .unwrap_or(serde_json::Value::Array(vec![])),
    )
    .map_err(|e| AppError::external(format!("解析 vectors 失败: {}", e)))?;

    let imported_message_count = messages.len();
    let imported_vector_count = vectors.len();
    let previous_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    let previous_model_hash = hash_json_value(&serde_json::to_value(&previous_model)?)?;
    let imported_model_hash = hash_json_value(&serde_json::to_value(&life_model)?)?;
    let pre_import_snapshot_version = {
        let vm = state.version_manager.lock().await;
        vm.snapshot(&previous_model, "auto:pre-import", "导入覆盖之前自动备份")
            .ok()
            .map(|snapshot| snapshot.version)
    };
    let previous_messages = {
        let store = state.memory_store.lock().await;
        store.export_all_messages().map_err(AppError::from)?
    };
    let previous_vectors = {
        let store = state.vector_store.lock().await;
        store.export_all_chunks().map_err(AppError::from)?
    };
    let durable_lifemodel_write = serde_json::to_value(&previous_model).map_err(AppError::from)?
        != serde_json::to_value(&life_model).map_err(AppError::from)?;

    if let Err(import_error) =
        apply_import_payload(state.clone(), life_model, messages, vectors).await
    {
        let rollback_error = apply_import_payload(
            state.clone(),
            previous_model,
            previous_messages,
            previous_vectors,
        )
        .await
        .err();
        if let Some(rollback_error) = rollback_error {
            return Err(AppError::internal(format!(
                "导入失败，且自动回滚失败。请不要继续操作，先备份数据目录。导入错误: {}; 回滚错误: {}",
                import_error, rollback_error
            )));
        }
        return Err(AppError::internal(format!(
            "导入失败，已自动回滚到导入前状态: {}",
            import_error
        )));
    }
    Ok(serde_json::json!({
        "success": true,
        "legacy": false,
        "governed_operation": true,
        "operation_kind": "data_import",
        "operation_purpose": request.purpose,
        "warning": "data import ran as an explicit governed restore/import operation.",
        "metadata_safe": true,
        "contains_raw_content": false,
        "durable_lifemodel_write": durable_lifemodel_write,
        "imported_message_count": imported_message_count,
        "imported_vector_count": imported_vector_count,
        "import_payload_hash": import_payload_hash,
        "previous_model_hash": previous_model_hash,
        "imported_model_hash": imported_model_hash,
        "pre_import_snapshot_created": pre_import_snapshot_version.is_some(),
        "pre_import_snapshot_version": pre_import_snapshot_version,
        "audit": {
            "source_kind": "data_import",
            "operation_purpose": request.purpose,
            "import_targets": request.import_targets,
            "import_payload_hash": import_payload_hash,
            "previous_model_hash": previous_model_hash,
            "imported_model_hash": imported_model_hash,
            "imported_message_count": imported_message_count,
            "imported_vector_count": imported_vector_count,
            "pre_change_snapshot_version": pre_import_snapshot_version,
            "metadata_safe": true,
            "contains_raw_content": false,
        },
    }))
}

async fn apply_import_payload(
    state: Arc<AppState>,
    life_model: LifeModel,
    messages: Vec<openlife_core::memory::ExportedMessage>,
    vectors: Vec<openlife_core::vectors::ExportedVectorChunk>,
) -> Result<(), AppError> {
    persist_life_model(
        &state,
        life_model,
        false,
        LifeModelMaterializerCallerContext::new(
            "data_import_governed_operation",
            LifeModelMaterializerCallerKind::GovernedRestoreImportOperation,
            LifeModelMaterializerCallerPurpose::GovernedRestoreImportOperation,
        ),
    )
    .await?;
    memory_gateway::replace_imported_memory_with_state(&state, &messages, &vectors).await?;
    Ok(())
}

#[tauri::command]
pub async fn test_api_key(state: State<'_, Arc<AppState>>) -> Result<bool, AppError> {
    let (base, key) = {
        let cfg = state.config.lock().await;
        (cfg.llm.openai_base.clone(), cfg.llm.openai_key.clone())
    };
    let api_key = if key.is_empty() {
        std::env::var("OPENROUTER_API_KEY").unwrap_or_default()
    } else {
        key
    };
    if api_key.is_empty() {
        return Ok(false);
    }
    let url = if base.is_empty() {
        "https://openrouter.ai/api/v1/models".to_string()
    } else {
        format!("{}/models", base.trim_end_matches('/'))
    };
    let client = reqwest::Client::new();
    let res = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| AppError::external(format!("API request failed: {}", e)))?;
    Ok(res.status().is_success())
}

#[derive(serde::Serialize)]
pub struct LlmConnectionTestResult {
    pub ok: bool,
    pub provider: String,
    pub message: String,
    pub validation_status: String,
}

#[tauri::command]
pub async fn test_llm_connection(
    mut config: AppConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<LlmConnectionTestResult, AppError> {
    config.normalize_provider_from_base();
    let provider = config.llm.provider.clone();
    let label = provider_label(&provider);

    let current_key = {
        let cfg = state.config.lock().await;
        cfg.llm.openai_key.clone()
    };
    let resolved_key = resolve_masked_api_key(&config.llm.openai_key, &current_key);
    config.llm.openai_key = resolved_key;

    let api_key = effective_api_key(&provider, &config.llm.openai_key);
    if api_key.trim().is_empty() {
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "missing_api_key",
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            &crate::provider_validation::provider_validation_path(),
            &record,
        )?;
        return Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: "未检测到 API Key，请填写后再测试。".to_string(),
            validation_status: "failed".into(),
        });
    }

    if !config.system.network_policy.enabled {
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "network_policy_disabled",
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            &crate::provider_validation::provider_validation_path(),
            &record,
        )?;
        return Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: "连接测试被当前网络策略阻止。请先启用网络访问后再验证 provider。".to_string(),
            validation_status: "failed".into(),
        });
    }

    let base = if config.llm.openai_base.trim().is_empty() {
        default_base_for_provider(&provider).to_string()
    } else {
        config.llm.openai_base.trim_end_matches('/').to_string()
    };
    let url = chat_completions_url(&provider, &base);
    let model = if config.llm.chat_model.trim().is_empty() {
        "deepseek-chat"
    } else {
        config.llm.chat_model.as_str()
    };
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 8,
        "temperature": 0.0
    });

    let client = reqwest::Client::new();
    let res = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let record = crate::provider_validation::failed_provider_validation_record(
                &config,
                "settings_manual_test",
                crate::provider_validation::reqwest_validation_error_label(&e),
                chrono::Utc::now(),
            );
            let _ = crate::provider_validation::save_provider_validation_record_to_path(
                &crate::provider_validation::provider_validation_path(),
                &record,
            );
            AppError::external(format!(
                "API request failed: {}",
                crate::provider_validation::reqwest_validation_error_label(&e)
            ))
        })?;
    let status = res.status();
    if status.is_success() {
        let record = crate::provider_validation::successful_provider_validation_record(
            &config,
            "settings_manual_test",
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            &crate::provider_validation::provider_validation_path(),
            &record,
        )?;
        let model_note = if model.to_lowercase().contains("reasoner") {
            " 当前选择的是推理模型，首次可见输出可能更慢；试用聊天建议优先使用 deepseek-chat 这类通用聊天模型。"
        } else {
            ""
        };
        Ok(LlmConnectionTestResult {
            ok: true,
            provider: label,
            message: format!("连接成功，云端模型可用。{}", model_note),
            validation_status: "validated".into(),
        })
    } else {
        let safe_error = format!("http_status:{}", status.as_u16());
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            &safe_error,
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            &crate::provider_validation::provider_validation_path(),
            &record,
        )?;
        Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: format!(
                "连接失败（HTTP {}）。请检查 provider、模型和 API Key。",
                status
            ),
            validation_status: "failed".into(),
        })
    }
}

#[tauri::command]
pub async fn export_mcp_audit_logs(
    days: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<AuditExport, AppError> {
    let store = state.mcp_audit_store.lock().await;
    store.export_logs(days).map_err(AppError::from)
}

#[tauri::command]
pub async fn cleanup_mcp_audit_logs(
    retention_days: i64,
    confirmation_evidence: Option<DangerActionConfirmationEvidence>,
    state: State<'_, Arc<AppState>>,
) -> Result<usize, AppError> {
    require_danger_action_confirmation(
        "mcp_audit_cleanup",
        &[],
        None,
        confirmation_evidence.as_ref(),
        state.inner(),
    )
    .await?;
    let store = state.mcp_audit_store.lock().await;
    store.cleanup(retention_days).map_err(AppError::from)
}

#[tauri::command]
pub async fn rotate_mcp_audit_key(
    confirmation_evidence: Option<DangerActionConfirmationEvidence>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    require_danger_action_confirmation(
        "mcp_audit_key_rotation",
        &[],
        None,
        confirmation_evidence.as_ref(),
        state.inner(),
    )
    .await?;
    let mut store = state.mcp_audit_store.lock().await;
    let new_config = AuditKeyConfig {
        mode: KeyMode::Derived,
        salt_b64: None,
        env_var: None,
        epoch: chrono::Utc::now().timestamp() as u64,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    store.rotate_key(new_config);
    save_mcp_audit_keyring_to_path(&mcp_audit_keyring_path(), store.key_configs())?;
    Ok(())
}

#[tauri::command]
pub async fn get_privacy_policy(
    state: State<'_, Arc<AppState>>,
) -> Result<PrivacyPolicy, AppError> {
    let engine = state.privacy_engine.lock().await;
    Ok(engine.policy().clone())
}

#[tauri::command]
pub async fn set_privacy_policy(
    policy: PrivacyPolicy,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    save_privacy_policy_to_path(&privacy_policy_path(), &policy)?;
    let mut engine = state.privacy_engine.lock().await;
    engine.set_policy(policy);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::llm::ChatMessage;

    const W84_IMPORT_CURRENT_NAME_SECRET: &str = "W84_IMPORT_CURRENT_LIFEMODEL_SECRET";
    const W84_IMPORT_PAYLOAD_NAME_SECRET: &str = "W84_IMPORT_PAYLOAD_LIFEMODEL_SECRET";
    const W84_IMPORT_CURRENT_MESSAGE_SECRET: &str = "W84_IMPORT_CURRENT_MESSAGE_SECRET";
    const W84_IMPORT_PAYLOAD_MESSAGE_SECRET: &str = "W84_IMPORT_PAYLOAD_MESSAGE_SECRET";
    const W84_IMPORT_CURRENT_VECTOR_SECRET: &str = "W84_IMPORT_CURRENT_VECTOR_SECRET";
    const W84_IMPORT_PAYLOAD_VECTOR_SECRET: &str = "W84_IMPORT_PAYLOAD_VECTOR_SECRET";

    #[test]
    fn resolve_masked_api_key_uses_current_key_for_mask_or_empty() {
        assert_eq!(resolve_masked_api_key(KEY_MASK, "sk-current"), "sk-current");
        assert_eq!(resolve_masked_api_key("", "sk-current"), "sk-current");
        assert_eq!(resolve_masked_api_key("   ", "sk-current"), "sk-current");
        assert_eq!(resolve_masked_api_key(KEY_MASK, ""), "");
    }

    #[test]
    fn resolve_masked_api_key_uses_submitted_new_key() {
        assert_eq!(resolve_masked_api_key("sk-new", "sk-current"), "sk-new");
    }

    #[test]
    fn danger_action_preflight_returns_safe_data_export_scope() {
        let view = danger_action_preflight_for_action("data_export", false).unwrap();

        assert_eq!(view.action_type, "data_export");
        assert_eq!(view.risk_tier, "high");
        assert_eq!(
            view.data_categories,
            vec!["life_model", "messages", "vectors"]
        );
        assert!(!view.writes_durable_state);
        assert!(view.privacy_sensitive);
        assert_eq!(view.external_transmission, "not_sent_externally");
        assert_eq!(view.backup_status, "not_required_read_only");
        assert!(view.final_action_enabled);
        assert!(!view.safe_mode_blocked);
        assert!(view
            .source_refs
            .iter()
            .any(|source| source == "final_command:export_all_data"));
    }

    #[test]
    fn danger_action_preflight_marks_import_overwrite_as_critical_without_claiming_existing_snapshot(
    ) {
        let view = danger_action_preflight_for_action("data_import_overwrite", false).unwrap();

        assert_eq!(view.action_type, "data_import_overwrite");
        assert_eq!(view.risk_tier, "critical");
        assert!(view.writes_durable_state);
        assert!(view.privacy_sensitive);
        assert_eq!(view.external_transmission, "not_sent_externally");
        assert_eq!(view.backup_status, "will_create_on_execute");
        assert!(view.final_action_enabled);
        assert!(view
            .source_refs
            .iter()
            .any(|source| source == "governed_request:create_pre_change_snapshot_on_execute"));

        let serialized = serde_json::to_string(&view).unwrap();
        for forbidden in [
            "snapshot_available",
            "snapshot_exists",
            "existing_snapshot",
            "already_created",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "import preflight must not claim existing snapshot via {forbidden}"
            );
        }
    }

    #[test]
    fn danger_action_preflight_marks_audit_export_as_sensitive_read_only() {
        let view = danger_action_preflight_for_action("mcp_audit_export", false).unwrap();

        assert_eq!(view.action_type, "mcp_audit_export");
        assert_eq!(view.risk_tier, "high");
        assert_eq!(
            view.data_categories,
            vec![
                "mcp_audit_metadata",
                "tool_metadata",
                "tool_input_text",
                "tool_output_text"
            ]
        );
        assert!(view.scope_summary.contains("工具输入参数文本"));
        assert!(view.scope_summary.contains("工具执行结果文本"));
        assert!(!view.writes_durable_state);
        assert!(view.privacy_sensitive);
        assert_eq!(view.external_transmission, "not_sent_externally");
        assert_eq!(view.backup_status, "not_required_read_only");
        assert!(view.final_action_enabled);
    }

    #[test]
    fn danger_action_preflight_marks_cleanup_and_key_rotation_as_mutating() {
        for action_type in ["mcp_audit_cleanup", "mcp_audit_key_rotation"] {
            let view = danger_action_preflight_for_action(action_type, false).unwrap();
            assert_eq!(view.action_type, action_type);
            assert!(view.writes_durable_state);
            assert!(view.privacy_sensitive);
            assert_eq!(view.external_transmission, "not_sent_externally");
            assert!(view.final_action_enabled);
            assert!(!view.safe_mode_blocked);
            assert!(
                view.backup_status == "none"
                    || view.backup_status == "historical_key_epochs_retained"
            );
        }
    }

    #[test]
    fn danger_action_preflight_covers_run_delete_and_vector_rebuild_without_raw_scope_leaks() {
        let view = danger_action_preflight_for_action_scoped(
            "agent_run_bulk_delete",
            false,
            DangerActionPreflightScope {
                target_ids: vec!["run-private-1".into(), "run-private-2".into()],
                affected_count: Some(2),
            },
        )
        .unwrap();

        assert_eq!(view.action_type, "agent_run_bulk_delete");
        assert!(view.writes_durable_state);
        assert!(view.confirmation_required);
        assert_eq!(view.confirmation_phrase.as_deref(), Some("DELETE RUNS"));
        assert_eq!(view.affected_item_count, 2);
        assert!(view.affected_item_digest.starts_with("bytes:"));
        assert!(view
            .source_refs
            .iter()
            .any(|source| source == "final_command:delete_agent_run"));
        let serialized = serde_json::to_string(&view).unwrap();
        assert!(!serialized.contains("run-private-1"));
        assert!(!serialized.contains("run-private-2"));

        let vector = danger_action_preflight_for_action_scoped(
            "vector_rebuild",
            false,
            DangerActionPreflightScope {
                target_ids: vec![],
                affected_count: Some(12),
            },
        )
        .unwrap();
        assert_eq!(vector.action_type, "vector_rebuild");
        assert_eq!(vector.confirmation_phrase.as_deref(), Some("REBUILD"));
        assert_eq!(vector.affected_item_count, 12);
        assert!(vector
            .source_refs
            .iter()
            .any(|source| source == "final_command:rebuild_memory_index"));
    }

    #[tokio::test]
    async fn danger_action_confirmation_requires_exact_phrase_and_scope() {
        let state = crate::test_utils::test_app_state();
        let target_ids = vec!["run-confirm-1".to_string()];
        let view = danger_action_preflight_for_action_scoped(
            "agent_run_delete",
            false,
            DangerActionPreflightScope {
                target_ids: target_ids.clone(),
                affected_count: Some(1),
            },
        )
        .unwrap();
        let evidence = DangerActionConfirmationEvidence {
            action_type: "agent_run_delete".into(),
            preflight_id: view.preflight_id.clone(),
            confirmation_phrase: view.confirmation_phrase.clone().unwrap(),
            confirmation_scope_digest: view.confirmation_scope_digest.clone(),
            safe_mode: false,
            target_ids: target_ids.clone(),
        };

        require_danger_action_confirmation(
            "agent_run_delete",
            &target_ids,
            Some(1),
            Some(&evidence),
            &state,
        )
        .await
        .unwrap();

        let missing = require_danger_action_confirmation(
            "agent_run_delete",
            &target_ids,
            Some(1),
            None,
            &state,
        )
        .await
        .unwrap_err();
        assert!(matches!(missing, AppError::PermissionDenied { .. }));

        let mut wrong_phrase = evidence.clone();
        wrong_phrase.confirmation_phrase = "WRONG".into();
        let err = require_danger_action_confirmation(
            "agent_run_delete",
            &target_ids,
            Some(1),
            Some(&wrong_phrase),
            &state,
        )
        .await
        .unwrap_err();
        assert!(err.message().contains("does not match"));

        let mut safe_mode_evidence = evidence.clone();
        safe_mode_evidence.safe_mode = true;
        let err = require_danger_action_confirmation(
            "agent_run_delete",
            &target_ids,
            Some(1),
            Some(&safe_mode_evidence),
            &state,
        )
        .await
        .unwrap_err();
        assert!(err.message().contains("Safe Mode"));

        let err = require_danger_action_confirmation(
            "agent_run_delete",
            &["run-other".into()],
            Some(1),
            Some(&evidence),
            &state,
        )
        .await
        .unwrap_err();
        assert!(err.message().contains("does not match"));
    }

    #[test]
    fn danger_action_preflight_safe_mode_blocks_destructive_actions() {
        for action_type in [
            "data_import_overwrite",
            "mcp_audit_cleanup",
            "mcp_audit_key_rotation",
            "agent_run_delete",
            "agent_run_bulk_delete",
            "vector_rebuild",
        ] {
            let view = danger_action_preflight_for_action(action_type, true).unwrap();
            assert!(view.writes_durable_state);
            assert!(view.safe_mode_blocked);
            assert!(!view.final_action_enabled);
            assert_eq!(
                view.blocking_reasons,
                vec!["safe_mode_blocks_durable_write"]
            );
        }

        for action_type in ["data_export", "mcp_audit_export"] {
            let view = danger_action_preflight_for_action(action_type, true).unwrap();
            assert!(!view.writes_durable_state);
            assert!(!view.safe_mode_blocked);
            assert!(view.final_action_enabled);
            assert!(view.blocking_reasons.is_empty());
        }
    }

    #[test]
    fn danger_action_preflight_rejects_unknown_action_type() {
        let err =
            danger_action_preflight_for_action("/tmp/sk-secret-unknown-action", false).unwrap_err();

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert_eq!(
            err.message(),
            "unsupported danger action preflight action type"
        );
        assert!(!err.message().contains("/tmp"));
        assert!(!err.message().contains("sk-secret"));
    }

    #[test]
    fn danger_action_preflight_never_serializes_payload_paths_or_key_material() {
        let views = [
            "data_export",
            "data_import_overwrite",
            "mcp_audit_export",
            "mcp_audit_cleanup",
            "mcp_audit_key_rotation",
            "agent_run_delete",
            "agent_run_bulk_delete",
            "vector_rebuild",
        ]
        .into_iter()
        .map(|action_type| danger_action_preflight_for_action(action_type, true).unwrap())
        .collect::<Vec<_>>();
        let serialized = serde_json::to_string(&views).unwrap();

        for forbidden in [
            "/tmp/",
            "/Users/",
            "C:\\",
            "sk-secret",
            "Bearer ",
            "api_key",
            "openai_key",
            "keyring",
            "payload",
            "arguments",
            "results",
            "raw_import",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "danger preflight leaked forbidden marker {forbidden}: {serialized}"
            );
        }
    }

    async fn seed_current_data(state: &Arc<AppState>) {
        {
            let manager = state.life_model_manager.lock().await;
            let mut model = manager.load().unwrap();
            model.identity.name = W84_IMPORT_CURRENT_NAME_SECRET.into();
            manager.save(&model).unwrap();
        }
        {
            let store = state.memory_store.lock().await;
            store
                .save_message(
                    "w84-current-session",
                    &ChatMessage {
                        role: "user".into(),
                        content: W84_IMPORT_CURRENT_MESSAGE_SECRET.into(),
                    },
                )
                .unwrap();
        }
        {
            let store = state.vector_store.lock().await;
            store
                .insert(
                    "w84-current-session",
                    W84_IMPORT_CURRENT_VECTOR_SECRET,
                    &[0.1, 0.2, 0.3, 0.4],
                    "w84-current",
                )
                .unwrap();
        }
    }

    fn import_payload() -> serde_json::Value {
        let mut model = LifeModel::default_model();
        model.identity.name = W84_IMPORT_PAYLOAD_NAME_SECRET.into();
        serde_json::json!({
            "version": "1.0",
            "life_model": model,
            "messages": [{
                "session_id": "w84-import-session",
                "role": "assistant",
                "content": W84_IMPORT_PAYLOAD_MESSAGE_SECRET,
                "created_at": "2026-06-03T00:00:00Z"
            }],
            "vectors": [{
                "session_id": "w84-import-session",
                "content": W84_IMPORT_PAYLOAD_VECTOR_SECRET,
                "embedding": [0.4, 0.3, 0.2, 0.1],
                "source": "w84-import",
                "created_at": "2026-06-03T00:00:00Z",
                "tier": 2,
                "access_count": 0,
                "last_accessed_at": "",
                "importance_score": 0.5,
                "archived": false,
                "archived_at": null,
                "summary": null
            }]
        })
    }

    async fn exported_message_contents(state: &Arc<AppState>) -> Vec<String> {
        state
            .memory_store
            .lock()
            .await
            .export_all_messages()
            .unwrap()
            .into_iter()
            .map(|message| message.content)
            .collect()
    }

    async fn exported_vector_contents(state: &Arc<AppState>) -> Vec<String> {
        state
            .vector_store
            .lock()
            .await
            .export_all_chunks()
            .unwrap()
            .into_iter()
            .map(|chunk| chunk.content)
            .collect()
    }

    async fn current_model_name(state: &Arc<AppState>) -> String {
        state
            .life_model_manager
            .lock()
            .await
            .load()
            .unwrap()
            .identity
            .name
    }

    #[tokio::test]
    async fn w93_import_all_data_without_governed_request_fails_closed() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;

        let err = import_all_data_with_state(import_payload(), &state)
            .await
            .expect_err("data import must fail closed by default");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("import_all_data"));
        assert!(err.message().contains("governed import request"));
        assert!(err.message().contains("explicitUserIntent=true"));
        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
        assert_eq!(
            exported_message_contents(&state).await,
            vec![W84_IMPORT_CURRENT_MESSAGE_SECRET.to_string()]
        );
        assert_eq!(
            exported_vector_contents(&state).await,
            vec![W84_IMPORT_CURRENT_VECTOR_SECRET.to_string()]
        );
    }

    #[tokio::test]
    async fn w93_import_all_data_governed_request_allows_metadata_safe_import() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;

        let result = import_all_data_with_state_for_governed_import(
            import_payload(),
            &state,
            GovernedDataImportRequest::manual_restore_all_targets(),
        )
        .await
        .unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(result["legacy"], false);
        assert_eq!(result["governed_operation"], true);
        assert_eq!(result["operation_kind"], "data_import");
        assert_eq!(result["operation_purpose"], "manual_restore");
        assert_eq!(result["metadata_safe"], true);
        assert_eq!(result["contains_raw_content"], false);
        assert_eq!(result["durable_lifemodel_write"], true);
        assert_eq!(result["imported_message_count"], 1);
        assert_eq!(result["imported_vector_count"], 1);
        assert!(result["import_payload_hash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:")));
        assert!(result["previous_model_hash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:")));
        assert!(result["imported_model_hash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:")));
        assert_eq!(result["pre_import_snapshot_created"], true);
        assert!(result["pre_import_snapshot_version"].is_string());
        assert_eq!(result["audit"]["metadata_safe"], true);
        assert_eq!(result["audit"]["contains_raw_content"], false);
        assert!(result.get("life_model").is_none());
        assert!(result.get("messages").is_none());
        assert!(result.get("vectors").is_none());
        assert!(result.get("payload").is_none());
        assert!(result.get("import_payload").is_none());

        let response_dump = result.to_string();
        for forbidden in [
            W84_IMPORT_CURRENT_NAME_SECRET,
            W84_IMPORT_PAYLOAD_NAME_SECRET,
            W84_IMPORT_CURRENT_MESSAGE_SECRET,
            W84_IMPORT_PAYLOAD_MESSAGE_SECRET,
            W84_IMPORT_CURRENT_VECTOR_SECRET,
            W84_IMPORT_PAYLOAD_VECTOR_SECRET,
        ] {
            assert!(
                !response_dump.contains(forbidden),
                "data import response leaked raw marker {forbidden}"
            );
        }

        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_PAYLOAD_NAME_SECRET
        );
        assert_eq!(
            exported_message_contents(&state).await,
            vec![W84_IMPORT_PAYLOAD_MESSAGE_SECRET.to_string()]
        );
        assert_eq!(
            exported_vector_contents(&state).await,
            vec![W84_IMPORT_PAYLOAD_VECTOR_SECRET.to_string()]
        );
    }

    #[tokio::test]
    async fn w93_import_all_data_invalid_governed_request_fails_closed() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;

        let err = import_all_data_with_state_gated(
            import_payload(),
            &state,
            Some(GovernedDataImportRequest {
                purpose: "normal_product".into(),
                explicit_user_intent: true,
                create_pre_change_snapshot: true,
                import_targets: vec!["life_model".into()],
            }),
        )
        .await
        .expect_err("invalid governed import purpose must fail closed");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("manual_restore"));
        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
    }

    #[tokio::test]
    async fn w93_import_all_data_payload_targets_must_match_governed_request() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;

        let err = import_all_data_with_state_for_governed_import(
            import_payload(),
            &state,
            GovernedDataImportRequest {
                purpose: "manual_restore".into(),
                explicit_user_intent: true,
                create_pre_change_snapshot: true,
                import_targets: vec!["life_model".into()],
            },
        )
        .await
        .expect_err("payload targets outside the governed request must fail closed");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("messages"));
        assert!(err.message().contains("import target"));
        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
        assert_eq!(
            exported_message_contents(&state).await,
            vec![W84_IMPORT_CURRENT_MESSAGE_SECRET.to_string()]
        );
        assert_eq!(
            exported_vector_contents(&state).await,
            vec![W84_IMPORT_CURRENT_VECTOR_SECRET.to_string()]
        );
    }

    #[tokio::test]
    async fn w93_import_all_data_unsupported_payload_target_fails_closed() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;
        let mut payload = import_payload();
        payload["unsupported_target"] =
            serde_json::json!({"secret": W84_IMPORT_PAYLOAD_NAME_SECRET});

        let err = import_all_data_with_state_for_governed_import(
            payload,
            &state,
            GovernedDataImportRequest::manual_restore_all_targets(),
        )
        .await
        .expect_err("unsupported import target must fail closed");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("unsupported import target"));
        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
    }

    #[tokio::test]
    async fn w84_export_all_data_remains_read_only_and_ungated() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;

        let exported = export_all_data_with_state(&state).await.unwrap();

        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
        assert_eq!(
            exported_message_contents(&state).await,
            vec![W84_IMPORT_CURRENT_MESSAGE_SECRET.to_string()]
        );
        assert_eq!(
            exported_vector_contents(&state).await,
            vec![W84_IMPORT_CURRENT_VECTOR_SECRET.to_string()]
        );
        assert!(exported
            .to_string()
            .contains(W84_IMPORT_CURRENT_NAME_SECRET));
        assert!(exported
            .to_string()
            .contains(W84_IMPORT_CURRENT_MESSAGE_SECRET));
        assert!(exported
            .to_string()
            .contains(W84_IMPORT_CURRENT_VECTOR_SECRET));
    }
}
