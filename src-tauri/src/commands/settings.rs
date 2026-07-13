use crate::errors::AppError;
use openlife_core::config::AppConfig;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::{
    chat_completions_url, default_base_for_provider, effective_api_key_for_endpoint,
    provider_label, ProviderInvocationReceipt, ProviderInvocationStatus,
};
use openlife_core::mcp_audit::{
    AuditExport, McpAuditCleanupScopeChanged, McpAuditRetentionDays, MCP_AUDIT_RETENTION_MAX_DAYS,
};
use openlife_core::network_client::resolve_network_policy_decision;
use openlife_core::privacy::PrivacyPolicy;
use openlife_core::scheduler::InferenceScheduler;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{Arc, LazyLock};
use tauri::State;

#[cfg(test)]
#[path = "../mcp_audit_export_gateway_tests.rs"]
mod mcp_audit_export_gateway_tests;

use crate::danger_action_confirmation::{
    issue_danger_action_challenge, require_native_danger_action_confirmation,
    NativeDangerActionRequest,
};
use crate::life_model_materializer_guard::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::provider_network_consent::{
    authorize_explicit_provider_probe, ExplicitProviderProbeAuthorization,
};
use crate::secret_store::{
    create_mcp_audit_key_material, stage_config_secrets, KeyringSecretStore, SecretStore,
};
use crate::storage::{
    app_data_dir, mcp_audit_keyring_path, privacy_policy_path, save_mcp_audit_keyring_to_path,
    save_privacy_policy_to_path,
};
use crate::AppState;
use crate::{life_model_write_gateway, memory_gateway};

static GOVERNED_DATA_IMPORT_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));
pub(crate) static CONFIG_WRITE_COORDINATOR: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

const MCP_AUDIT_CLEANUP_PREDICATE_VERSION: &str = "mcp-audit-created-before-request-cutoff-v1";

fn invalid_mcp_audit_retention_error() -> AppError {
    AppError::Config {
        message: "invalid_mcp_audit_retention_days".into(),
        hint: Some(format!(
            "retention_days_must_be_1_through_{MCP_AUDIT_RETENTION_MAX_DAYS}"
        )),
    }
}

fn validate_mcp_audit_retention_days(
    retention_days: i64,
) -> Result<McpAuditRetentionDays, AppError> {
    McpAuditRetentionDays::try_from(retention_days).map_err(|_| invalid_mcp_audit_retention_error())
}

fn mcp_audit_cleanup_preflight_scope_arguments(
    retention_days: i64,
    candidate_count: usize,
) -> serde_json::Value {
    serde_json::json!({
        "retention_days": retention_days,
        "predicate_version": MCP_AUDIT_CLEANUP_PREDICATE_VERSION,
        "candidate_count": candidate_count,
    })
}

fn map_mcp_audit_cleanup_error(error: anyhow::Error) -> AppError {
    if error
        .downcast_ref::<McpAuditCleanupScopeChanged>()
        .is_some()
    {
        AppError::permission("mcp_audit_cleanup_scope_changed_refresh_preflight")
    } else {
        AppError::db_with_hint(error.to_string(), "mcp_audit_store_error")
    }
}

fn require_mcp_audit_cleanup_effects_allowed(state: &Arc<AppState>) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))
}

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
pub struct DangerActionConfirmationReference {
    /// Opaque, random challenge identifier issued by the Rust authority. All
    /// other client fields are scope hints only and can never authorize an action.
    pub preflight_id: String,
    #[serde(default)]
    pub action_type: String,
    #[serde(default)]
    pub target_ids: Vec<String>,
}

pub(crate) struct DangerActionConfirmationRequest<'a> {
    pub action_type: &'a str,
    pub target_ids_for_new_challenge: &'a [String],
    pub requested_target: Option<&'a str>,
    pub affected_count: Option<usize>,
    pub reference: Option<&'a DangerActionConfirmationReference>,
    pub preflight_scope_arguments: Option<&'a serde_json::Value>,
    pub arguments: &'a serde_json::Value,
    pub arguments_summary: &'a str,
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
    preflight_scope_arguments: Option<serde_json::Value>,
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

fn danger_action_requires_native_confirmation(action_type: &str) -> bool {
    matches!(
        action_type,
        "data_export"
            | "data_import_overwrite"
            | "mcp_audit_export"
            | "mcp_audit_cleanup"
            | "mcp_audit_key_rotation"
            | "agent_run_delete"
            | "agent_run_bulk_delete"
            | "vector_rebuild"
    )
}

fn danger_action_scope_digest(
    action_type: &str,
    target_ids: &[String],
    affected_count: usize,
    preflight_scope_arguments: Option<&serde_json::Value>,
) -> Result<String, AppError> {
    let canonical = serde_json::json!({
        "action_type": action_type,
        "affected_count": affected_count,
        "target_id_count": target_ids.len(),
        "target_ids": target_ids,
        "preflight_scope_arguments": preflight_scope_arguments,
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
    let scope_digest = danger_action_scope_digest(
        action_type,
        &safe_target_ids,
        affected_count,
        scope.preflight_scope_arguments.as_ref(),
    )?;
    let confirmation_phrase = None;
    let requires_typed_confirmation = false;
    let confirmation_required = danger_action_requires_native_confirmation(action_type);
    // A usable preflight id is created only by `get_danger_action_preflight`
    // through the Rust-owned challenge authority. The deterministic view builder
    // intentionally cannot mint authorization state.
    let preflight_id = String::new();
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
            requires_typed_confirmation,
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
            requires_typed_confirmation,
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
            requires_typed_confirmation,
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
            requires_typed_confirmation,
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
            requires_typed_confirmation,
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
            requires_typed_confirmation,
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
            requires_typed_confirmation,
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
            requires_typed_confirmation,
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
    request: DangerActionConfirmationRequest<'_>,
    window: &tauri::WebviewWindow,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    let scope = DangerActionPreflightScope {
        target_ids: request.target_ids_for_new_challenge.to_vec(),
        affected_count: request.affected_count,
        preflight_scope_arguments: request.preflight_scope_arguments.cloned(),
    };
    let expected = danger_action_preflight_for_action_scoped(request.action_type, false, scope)?;
    if expected.writes_durable_state && danger_action_safe_mode_active(state).await {
        return Err(AppError::permission(
            "danger action blocked because Safe Mode is active",
        ));
    }
    if !expected.confirmation_required {
        return Ok(());
    }
    require_native_danger_action_confirmation(
        window,
        NativeDangerActionRequest {
            action_type: request.action_type,
            target_ids_for_new_challenge: request.target_ids_for_new_challenge,
            requested_target: request.requested_target,
            affected_count: expected.affected_item_count,
            preflight_scope_arguments: request.preflight_scope_arguments,
            arguments: request.arguments,
            arguments_summary: request.arguments_summary,
            scope_summary: &expected.scope_summary,
            challenge_id: request
                .reference
                .map(|reference| reference.preflight_id.as_str()),
        },
    )
    .await
}

#[tauri::command]
pub async fn get_danger_action_preflight(
    action_type: String,
    safe_mode: Option<bool>,
    target_ids: Option<Vec<String>>,
    affected_count: Option<usize>,
    retention_days: Option<i64>,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<DangerActionPreflightView, AppError> {
    let mut effective_safe_mode = safe_mode.unwrap_or(false);
    if danger_action_safe_mode_active(state.inner()).await {
        effective_safe_mode = true;
    }
    let target_ids = target_ids.unwrap_or_default();
    let mut preflight_scope_arguments = None;
    let effective_affected_count = if action_type == "mcp_audit_cleanup" {
        let retention_days = retention_days.ok_or_else(invalid_mcp_audit_retention_error)?;
        let retention = validate_mcp_audit_retention_days(retention_days)?;
        let candidate_count = {
            let store = state.mcp_audit_store.lock().await;
            store
                .count_cleanup_candidates(&retention)
                .map_err(map_mcp_audit_cleanup_error)?
        };
        preflight_scope_arguments = Some(mcp_audit_cleanup_preflight_scope_arguments(
            retention_days,
            candidate_count,
        ));
        Some(candidate_count)
    } else if action_type == "vector_rebuild" && affected_count.is_none() {
        let store = state.memory_store.lock().await;
        Some(store.export_all_messages().map_err(AppError::from)?.len())
    } else {
        affected_count
    };
    let mut view = danger_action_preflight_for_action_scoped(
        &action_type,
        effective_safe_mode,
        DangerActionPreflightScope {
            target_ids: target_ids.clone(),
            affected_count: effective_affected_count,
            preflight_scope_arguments: preflight_scope_arguments.clone(),
        },
    )?;
    if action_type == "mcp_audit_cleanup" {
        let retention_days = retention_days.ok_or_else(invalid_mcp_audit_retention_error)?;
        view.scope_summary = format!(
            "按服务端时钟删除创建时间早于当前请求时间减去 {retention_days} 天的本地 MCP 审计日志；影响数量来自后端候选快照。"
        );
        view.source_refs
            .push("mcp_audit_store:server_candidate_snapshot".into());
        view.source_refs.push(format!(
            "cleanup_predicate:{MCP_AUDIT_CLEANUP_PREDICATE_VERSION}"
        ));
    }
    if view.confirmation_required && view.final_action_enabled {
        view.preflight_id = issue_danger_action_challenge(
            window.label(),
            &action_type,
            &target_ids,
            view.affected_item_count,
            preflight_scope_arguments.as_ref(),
        )?;
        view.source_refs
            .push("native_confirmation:server_challenge_pending".into());
    }
    Ok(view)
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
            "version"
                | "app_version"
                | "exported_at"
                | "vector_export_semantics"
                | "life_model"
                | "messages"
                | "vectors"
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

fn provider_endpoint_identity(config: &AppConfig) -> Option<String> {
    let provider = config.llm.provider.trim().to_ascii_lowercase();
    if provider.is_empty() {
        return None;
    }
    let base = if config.llm.openai_base.trim().is_empty() {
        default_base_for_provider(&provider).to_string()
    } else {
        config.llm.openai_base.trim().to_string()
    };
    let endpoint = chat_completions_url(&provider, &base);
    let parsed = reqwest::Url::parse(&endpoint).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }
    let host = parsed
        .host_str()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let port = parsed.port_or_known_default()?;
    let path = parsed.path().trim_end_matches('/');
    Some(format!(
        "{provider}|{}://{host}:{port}{path}",
        parsed.scheme()
    ))
}

fn resolve_submitted_provider_api_key(submitted: &AppConfig, current: &AppConfig) -> String {
    let submitted_key = submitted.llm.openai_key.trim();
    if !submitted_key.is_empty() && submitted_key != KEY_MASK {
        return submitted.llm.openai_key.clone();
    }
    let identity_unchanged = provider_endpoint_identity(submitted).is_some_and(|identity| {
        provider_endpoint_identity(current).as_deref() == Some(identity.as_str())
    });
    if identity_unchanged {
        current.llm.openai_key.clone()
    } else {
        String::new()
    }
}

fn resolved_provider_credential_version(submitted: &AppConfig, current: &AppConfig) -> u64 {
    let identity_changed =
        provider_endpoint_identity(submitted) != provider_endpoint_identity(current);
    let submitted_key = submitted.llm.openai_key.trim();
    let explicit_key_changed = !submitted_key.is_empty()
        && submitted_key != KEY_MASK
        && submitted_key != current.llm.openai_key;
    if identity_changed || explicit_key_changed {
        current.llm.credential_version.saturating_add(1)
    } else {
        current.llm.credential_version
    }
}

async fn replace_runtime_provider_config(state: &Arc<AppState>, config: AppConfig) {
    state.replace_provider_runtime_config(config).await;
}

#[tauri::command]
pub async fn get_config(state: State<'_, Arc<AppState>>) -> Result<AppConfig, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("ConfigStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let mut cfg = state.config.lock().await.clone();
    // Sanitize API keys before sending to frontend
    if !cfg.llm.openai_key.is_empty() {
        cfg.llm.openai_key = KEY_MASK.to_string();
    }
    if !cfg.system.search_provider_key.is_empty() {
        cfg.system.search_provider_key = KEY_MASK.to_string();
    }
    Ok(cfg)
}

#[tauri::command]
pub async fn save_config(
    mut config: AppConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let _config_write_guard = CONFIG_WRITE_COORDINATOR.lock().await;
    config.normalize_provider_from_base();
    let data_dir = app_data_dir();
    let config_path = data_dir.join("config.yaml");

    // Preserve existing API key if the submitted config has a mask or empty key
    let current_config = {
        let cfg = state.config.lock().await;
        cfg.clone()
    };
    let provider_identity_unchanged = provider_endpoint_identity(&config).is_some_and(|identity| {
        provider_endpoint_identity(&current_config).as_deref() == Some(identity.as_str())
    });
    config.llm.credential_version = resolved_provider_credential_version(&config, &current_config);
    config.llm.openai_key = resolve_submitted_provider_api_key(&config, &current_config);
    config.system.search_provider_key = resolve_masked_api_key(
        &config.system.search_provider_key,
        &current_config.system.search_provider_key,
    );
    if !provider_identity_unchanged {
        // A secret reference is bound to the provider plus canonical endpoint. A masked
        // frontend value cannot carry an old credential to a different destination.
        config.llm.openai_key_ref = None;
    } else if config.llm.openai_key_ref.is_none() {
        config.llm.openai_key_ref = current_config.llm.openai_key_ref;
    }
    if config.system.search_provider_key_ref.is_none() {
        config.system.search_provider_key_ref = current_config.system.search_provider_key_ref;
    }

    let secret_store = KeyringSecretStore;
    let rollback = stage_config_secrets(&mut config, &secret_store).map_err(AppError::from)?;
    if let Err(save_error) = config.save(&config_path) {
        return match rollback.rollback(&secret_store) {
            Ok(()) => Err(AppError::from(save_error)),
            Err(rollback_error) => Err(AppError::internal(format!(
                "config save failed: {save_error}; credential rollback failed: {rollback_error}"
            ))),
        };
    }
    replace_runtime_provider_config(state.inner(), config).await;
    Ok(())
}

#[tauri::command]
pub async fn export_all_data(
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let export = export_all_data_with_state(state.inner()).await?;
    let export_digest = hash_json_value(&export)?;
    require_danger_action_confirmation(
        DangerActionConfirmationRequest {
            action_type: "data_export",
            target_ids_for_new_challenge: &[],
            requested_target: None,
            affected_count: None,
            reference: None,
            preflight_scope_arguments: None,
            arguments: &serde_json::json!({
                "export_digest": export_digest,
                "data_categories": ["life_model", "messages", "vectors"],
            }),
            arguments_summary:
                "导出当前 LifeModel、聊天和向量数据快照；原始内容不会复制进 confirmation grant。",
        },
        &window,
        state.inner(),
    )
    .await?;
    Ok(export)
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
        store.export_portable_chunks().map_err(AppError::from)?
    };
    Ok(serde_json::json!({
        "version": "1.0",
        "app_version": env!("CARGO_PKG_VERSION"),
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "vector_export_semantics": "portable_only_canonical_and_chat_projections_derived",
        "life_model": life_model,
        "messages": messages,
        "vectors": vectors,
    }))
}

#[tauri::command]
pub async fn import_all_data(
    payload: serde_json::Value,
    import_request: Option<GovernedDataImportRequest>,
    confirmation_evidence: Option<DangerActionConfirmationReference>,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let request = require_governed_data_import_request(import_request.as_ref())?.clone();
    validate_import_payload_shape(&payload)?;
    validate_import_targets_cover_payload(&payload, &request)?;
    let payload_digest = hash_json_value(&payload)?;
    let confirmation_arguments = serde_json::json!({
        "payload_digest": payload_digest,
        "governed_request": request,
    });
    require_danger_action_confirmation(
        DangerActionConfirmationRequest {
            action_type: "data_import_overwrite",
            target_ids_for_new_challenge: &[],
            requested_target: None,
            affected_count: None,
            reference: confirmation_evidence.as_ref(),
            preflight_scope_arguments: None,
            arguments: &confirmation_arguments,
            arguments_summary:
                "覆盖导入已校验的 OpenLife 备份；参数已绑定到 payload digest 和 governed request。",
        },
        &window,
        state.inner(),
    )
    .await?;
    import_all_data_governed_operation(payload, state.inner(), &request).await
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

#[cfg(test)]
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
    // Import snapshots, canonical replacement, and compensation form one
    // process-local destructive operation. Serializing the whole sequence
    // prevents two confirmed imports from interleaving their rollback truth.
    let _import_guard = GOVERNED_DATA_IMPORT_LOCK.lock().await;
    let import_payload_hash = hash_json_value(&payload)?;
    let life_model: LifeModel = serde_json::from_value(
        payload
            .get("life_model")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(|e| AppError::external(format!("解析 life_model 失败: {}", e)))?;
    // Missing means "not targeted", while an explicit empty array means
    // "replace this target with an empty set". Conflating the two silently
    // erased untargeted stores in the former import route.
    let messages: Option<Vec<openlife_core::memory::ExportedMessage>> = payload
        .get("messages")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| AppError::external(format!("解析 messages 失败: {}", e)))?;
    let vectors: Option<Vec<openlife_core::vectors::ExportedVectorChunk>> = payload
        .get("vectors")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| AppError::external(format!("解析 vectors 失败: {}", e)))?;
    let messages_targeted = messages.is_some();
    let vectors_targeted = vectors.is_some();
    let previous_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    let previous_model_hash =
        life_model_write_gateway::hash_life_model(&previous_model).map_err(AppError::from)?;
    let imported_model_hash =
        life_model_write_gateway::hash_life_model(&life_model).map_err(AppError::from)?;
    let pre_import_snapshot_version = {
        let vm = state.version_manager.lock().await;
        Some(
            vm.ensure_projection_snapshot(
                &previous_model,
                &format!("pre-change:import:{import_payload_hash}:{previous_model_hash}"),
                "auto:pre-import",
                "导入覆盖之前自动备份",
            )
            .map_err(AppError::from)?
            .version,
        )
    };
    let previous_messages = if messages_targeted {
        let store = state.memory_store.lock().await;
        Some(store.export_all_messages().map_err(AppError::from)?)
    } else {
        None
    };
    let previous_vectors = if vectors_targeted {
        let store = state.vector_store.lock().await;
        Some(store.export_portable_chunks().map_err(AppError::from)?)
    } else {
        None
    };
    let durable_lifemodel_write = serde_json::to_value(&previous_model).map_err(AppError::from)?
        != serde_json::to_value(&life_model).map_err(AppError::from)?;

    let memory_report = match apply_import_payload(
        state.clone(),
        life_model,
        messages,
        vectors,
        Some(previous_model_hash.clone()),
    )
    .await
    {
        Ok(report) => report,
        Err(import_error) => {
            let rollback_error = apply_import_payload(
                state.clone(),
                previous_model,
                previous_messages,
                previous_vectors,
                None,
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
    };
    let imported_message_count = memory_report.applied_message_count;
    let supplied_message_count = memory_report.supplied_message_count;
    let imported_vector_count = memory_report.vectors.applied;
    let supplied_vector_count = memory_report.vectors.supplied;
    let skipped_vector_count = memory_report.vectors.skipped();
    let import_audit = serde_json::json!({
        "source_kind": "data_import",
        "operation_purpose": request.purpose,
        "vector_import_semantics": "portable_only_canonical_and_chat_projections_skipped",
        "import_targets": request.import_targets,
        "import_payload_hash": import_payload_hash,
        "previous_model_hash": previous_model_hash,
        "imported_model_hash": imported_model_hash,
        "messages_targeted": memory_report.messages_targeted,
        "vectors_targeted": memory_report.vectors_targeted,
        "supplied_message_count": supplied_message_count,
        "imported_message_count": imported_message_count,
        "supplied_vector_count": supplied_vector_count,
        "imported_vector_count": imported_vector_count,
        "skipped_vector_count": skipped_vector_count,
        "skipped_canonical_vector_count": memory_report.vectors.skipped_canonical_projection,
        "skipped_legacy_chat_vector_count": memory_report.vectors.skipped_legacy_chat_projection,
        "pre_change_snapshot_version": pre_import_snapshot_version,
        "metadata_safe": true,
        "contains_raw_content": false,
    });
    Ok(serde_json::json!({
        "success": true,
        "legacy": false,
        "governed_operation": true,
        "operation_kind": "data_import",
        "operation_purpose": request.purpose,
        "warning": "data import ran as an explicit governed restore/import operation.",
        "vector_import_semantics": "portable_only_canonical_and_chat_projections_skipped",
        "metadata_safe": true,
        "contains_raw_content": false,
        "durable_lifemodel_write": durable_lifemodel_write,
        "messages_targeted": memory_report.messages_targeted,
        "vectors_targeted": memory_report.vectors_targeted,
        "supplied_message_count": supplied_message_count,
        "imported_message_count": imported_message_count,
        "supplied_vector_count": supplied_vector_count,
        "imported_vector_count": imported_vector_count,
        "skipped_vector_count": skipped_vector_count,
        "skipped_canonical_vector_count": memory_report.vectors.skipped_canonical_projection,
        "skipped_legacy_chat_vector_count": memory_report.vectors.skipped_legacy_chat_projection,
        "import_payload_hash": import_payload_hash,
        "previous_model_hash": previous_model_hash,
        "imported_model_hash": imported_model_hash,
        "pre_import_snapshot_created": pre_import_snapshot_version.is_some(),
        "pre_import_snapshot_version": pre_import_snapshot_version,
        "audit": import_audit,
    }))
}

async fn apply_import_payload(
    state: Arc<AppState>,
    life_model: LifeModel,
    messages: Option<Vec<openlife_core::memory::ExportedMessage>>,
    vectors: Option<Vec<openlife_core::vectors::ExportedVectorChunk>>,
    expected_lifemodel_hash: Option<String>,
) -> Result<memory_gateway::ImportedMemoryReplaceReport, AppError> {
    life_model_write_gateway::persist_life_model_with_gateway_expected(
        &state,
        life_model,
        false,
        LifeModelMaterializerCallerContext::new(
            "data_import_governed_operation",
            LifeModelMaterializerCallerKind::GovernedRestoreImportOperation,
            LifeModelMaterializerCallerPurpose::GovernedRestoreImportOperation,
        ),
        expected_lifemodel_hash.as_deref(),
    )
    .await?;
    memory_gateway::replace_imported_memory_with_state(
        &state,
        messages.as_deref(),
        vectors.as_deref(),
    )
    .await
}

#[derive(serde::Serialize)]
pub struct LlmConnectionTestResult {
    pub ok: bool,
    pub provider: String,
    pub message: String,
    pub validation_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_policy_decision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_network_policy_decision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_proposal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_id: Option<String>,
    /// Exact metadata-only terminal from the scheduler's provider adapter seam.
    /// Provider request/response bodies and credentials are never included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_invocation_receipt: Option<ProviderInvocationReceipt>,
}

#[tauri::command]
pub async fn test_llm_connection(
    config: AppConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<LlmConnectionTestResult, AppError> {
    test_llm_connection_with_state_and_validation_path(
        config,
        state.inner(),
        &crate::provider_validation::provider_validation_path(),
    )
    .await
}

pub(crate) async fn test_llm_connection_with_state_and_validation_path(
    mut config: AppConfig,
    state: &Arc<AppState>,
    validation_path: &std::path::Path,
) -> Result<LlmConnectionTestResult, AppError> {
    config.normalize_provider_from_base();
    let provider = config.llm.provider.clone();
    let label = provider_label(&provider);

    let current_runtime = state.provider_runtime_snapshot().await;
    let current_runtime_coherent = current_runtime.coherent;
    let current_config = current_runtime.config;
    config.llm.credential_version = resolved_provider_credential_version(&config, &current_config);
    config.llm.openai_key = resolve_submitted_provider_api_key(&config, &current_config);

    if !current_runtime_coherent {
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "provider_runtime_generation_incoherent",
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            validation_path,
            &record,
        )?;
        return Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: "Provider 配置与执行适配器不属于同一运行代；连接测试已在网络请求前失败关闭。"
                .into(),
            validation_status: "runtime_generation_incoherent".into(),
            network_policy_decision_id: None,
            effective_network_policy_decision_id: None,
            consent_status: Some("blocked".into()),
            review_proposal_id: None,
            permission_id: None,
            provider_invocation_receipt: None,
        });
    }

    let api_key =
        effective_api_key_for_endpoint(&provider, &config.llm.openai_base, &config.llm.openai_key);
    if api_key.trim().is_empty() {
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "missing_api_key",
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            validation_path,
            &record,
        )?;
        return Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: "未检测到 API Key，请填写后再测试。".to_string(),
            validation_status: "failed".into(),
            network_policy_decision_id: None,
            effective_network_policy_decision_id: None,
            consent_status: None,
            review_proposal_id: None,
            permission_id: None,
            provider_invocation_receipt: None,
        });
    }

    let backend_network_policy = current_config.system.network_policy.clone();
    // The submitted Settings payload cannot choose the network authority used
    // for either dispatch or durable validation identity.
    config.system.network_policy = backend_network_policy.clone();
    if !backend_network_policy.enabled {
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "network_policy_disabled",
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            validation_path,
            &record,
        )?;
        return Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: "连接测试被当前网络策略阻止。请先启用网络访问后再验证 provider。".to_string(),
            validation_status: "failed".into(),
            network_policy_decision_id: None,
            effective_network_policy_decision_id: None,
            consent_status: Some("blocked".into()),
            review_proposal_id: None,
            permission_id: None,
            provider_invocation_receipt: None,
        });
    }

    let base = if config.llm.openai_base.trim().is_empty() {
        default_base_for_provider(&provider).to_string()
    } else {
        config.llm.openai_base.trim_end_matches('/').to_string()
    };
    let model = config.llm.chat_model.trim().to_string();
    if model.is_empty() {
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "missing_model",
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            validation_path,
            &record,
        )?;
        return Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: "未配置要验证的模型；连接测试没有发送 provider 请求。".into(),
            validation_status: "failed".into(),
            network_policy_decision_id: None,
            effective_network_policy_decision_id: None,
            consent_status: None,
            review_proposal_id: None,
            permission_id: None,
            provider_invocation_receipt: None,
        });
    }
    config.llm.openai_base = base.clone();
    config.llm.chat_model = model.clone();
    config.llm.openai_key = api_key.clone();
    let probe_scheduler = InferenceScheduler::new(
        config.local_model.clone(),
        false,
        provider.clone(),
        base.clone(),
        api_key,
        model.clone(),
        config.llm.embedding_model.clone(),
        false,
    )
    .with_provider_credential_version(config.llm.credential_version);
    let probe_scheduler = {
        let permission_store = state.tool_permission_store.lock().await;
        permission_store.bind_explicit_provider_probe_scheduler(probe_scheduler)
    };
    let url = chat_completions_url(&provider, &base);
    let network_capability = format!("provider.{provider}");
    let network_policy_decision =
        resolve_network_policy_decision(&backend_network_policy, &url, &network_capability)
            .map_err(|_| AppError::external("provider network policy decision failed"))?;
    let original_network_policy_decision_id = network_policy_decision.decision_id.clone();
    let (probe_grant, effective_network_policy_decision_id, permission_id) =
        match authorize_explicit_provider_probe(
            state,
            &probe_scheduler,
            &backend_network_policy,
            &network_policy_decision,
            &url,
            &network_capability,
            &provider,
        )
        .await?
        {
            ExplicitProviderProbeAuthorization::Authorized {
                grant,
                effective_network_policy_decision_id,
                permission_id,
            } => (grant, effective_network_policy_decision_id, permission_id),
            ExplicitProviderProbeAuthorization::ConsentRequired { proposal_id } => {
                return Ok(LlmConnectionTestResult {
                    ok: false,
                    provider: label,
                    message: "需要在 Review Center 明确批准一次 provider 网络连接；批准前不会发送请求，批准后请重试连接测试。".into(),
                    validation_status: "consent_required".into(),
                    network_policy_decision_id: Some(original_network_policy_decision_id),
                    effective_network_policy_decision_id: None,
                    consent_status: Some("pending_review".into()),
                    review_proposal_id: Some(proposal_id),
                    permission_id: None,
                    provider_invocation_receipt: None,
                });
            }
            ExplicitProviderProbeAuthorization::Denied { reason_code } => {
                return Ok(LlmConnectionTestResult {
                    ok: false,
                    provider: label,
                    message: format!("连接测试被当前网络策略阻止（{reason_code}）。"),
                    validation_status: "blocked".into(),
                    network_policy_decision_id: Some(original_network_policy_decision_id),
                    effective_network_policy_decision_id: None,
                    consent_status: Some("blocked".into()),
                    review_proposal_id: None,
                    permission_id: None,
                    provider_invocation_receipt: None,
                });
            }
        };
    let prepared = match probe_scheduler.prepare_explicit_provider_probe(probe_grant) {
        Ok(prepared) => prepared,
        Err(_) => {
            let record = crate::provider_validation::failed_provider_validation_record(
                &config,
                "settings_manual_test",
                "provider_probe_pre_dispatch_rejected",
                chrono::Utc::now(),
            );
            crate::provider_validation::save_provider_validation_record_to_path(
                validation_path,
                &record,
            )?;
            return Ok(LlmConnectionTestResult {
                ok: false,
                provider: label,
                message: "连接测试在 provider 请求发出前被拒绝，未建立可用性证据。".into(),
                validation_status: "failed".into(),
                network_policy_decision_id: Some(original_network_policy_decision_id),
                effective_network_policy_decision_id: Some(
                    effective_network_policy_decision_id.clone(),
                ),
                consent_status: Some(if permission_id.is_some() {
                    "allow_once_consumed".into()
                } else {
                    "not_required".into()
                }),
                review_proposal_id: None,
                permission_id,
                provider_invocation_receipt: None,
            });
        }
    };
    // These are the exact prepared-generation facts later sealed into the
    // adapter terminal proof. They are captured before ownership moves into
    // execution; the submitted Settings payload is not proof authority.
    let prepared_provider_config_generation = prepared.provider_config_generation.clone();
    let prepared_network_policy = prepared.network_policy.clone();
    let prepared_network_policy_decision = prepared.network_policy_decision.clone();
    let outcome = probe_scheduler.execute_prepared(prepared).await;
    let result_has_content = outcome
        .result
        .as_ref()
        .is_ok_and(|content| !content.trim().is_empty());
    let observed_receipt = outcome.receipt;
    let terminal_proof = outcome.terminal_proof;
    let write_observed_at = chrono::Utc::now();
    let mut receipt = None;
    let mut terminal_status = None;
    let mut completed = false;
    let record = match (observed_receipt.as_ref(), terminal_proof) {
        (Some(observed), Some(proof)) if proof.receipt() == observed => {
            let candidate_status = proof.receipt().status;
            let candidate_receipt = proof.receipt().clone();
            let candidate_completed =
                candidate_status == ProviderInvocationStatus::Completed && result_has_content;
            let safe_error = match candidate_status {
                ProviderInvocationStatus::RemoteUnknown => "provider_remote_state_unknown",
                ProviderInvocationStatus::Failed => "provider_confirmed_failure",
                ProviderInvocationStatus::Completed if !candidate_completed => {
                    "provider_completion_inconsistent"
                }
                ProviderInvocationStatus::Completed => "validation_failed",
            };
            match crate::provider_validation::provider_validation_record_with_terminal_proof(
                &config,
                "settings_manual_test",
                proof,
                &prepared_provider_config_generation,
                &prepared_network_policy,
                &prepared_network_policy_decision,
                candidate_completed,
                (!candidate_completed).then_some(safe_error),
                write_observed_at,
            ) {
                Ok(record) => {
                    // A receipt reaches product/durable projection only after
                    // the opaque proof passes every exact runtime binding.
                    receipt = Some(candidate_receipt);
                    terminal_status = Some(candidate_status);
                    completed = candidate_completed;
                    record
                }
                Err(_) => crate::provider_validation::failed_provider_validation_record(
                    &config,
                    "settings_manual_test",
                    "provider_terminal_proof_invalid",
                    write_observed_at,
                ),
            }
        }
        (Some(_), None) => crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "provider_terminal_proof_missing",
            write_observed_at,
        ),
        (None, None) => crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "provider_not_attempted",
            write_observed_at,
        ),
        (None, Some(_)) | (Some(_), Some(_)) => {
            crate::provider_validation::failed_provider_validation_record(
                &config,
                "settings_manual_test",
                "provider_terminal_proof_mismatch",
                write_observed_at,
            )
        }
    };
    crate::provider_validation::save_provider_validation_record_to_path(validation_path, &record)?;

    if completed {
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
            network_policy_decision_id: Some(original_network_policy_decision_id),
            effective_network_policy_decision_id: Some(
                effective_network_policy_decision_id.clone(),
            ),
            consent_status: Some(if permission_id.is_some() {
                "allow_once_consumed".into()
            } else {
                "not_required".into()
            }),
            review_proposal_id: None,
            permission_id,
            provider_invocation_receipt: receipt,
        })
    } else {
        let remote_unknown = terminal_status == Some(ProviderInvocationStatus::RemoteUnknown);
        Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: if remote_unknown {
                "连接请求已开始，但没有观察到可信的远端终态；当前状态为 unknown，不能标记为可用。"
                    .into()
            } else if terminal_status == Some(ProviderInvocationStatus::Failed) {
                "Provider 已返回明确失败，连接不能标记为可用。请检查 provider、模型和 API Key。"
                    .into()
            } else {
                "没有获得完整且可信的 provider 响应，连接不能标记为可用。".into()
            },
            validation_status: if remote_unknown {
                "remote_unknown".into()
            } else {
                "failed".into()
            },
            network_policy_decision_id: Some(original_network_policy_decision_id),
            effective_network_policy_decision_id: Some(
                effective_network_policy_decision_id.clone(),
            ),
            consent_status: Some(if permission_id.is_some() {
                "allow_once_consumed".into()
            } else {
                "not_required".into()
            }),
            review_proposal_id: None,
            permission_id,
            provider_invocation_receipt: receipt,
        })
    }
}

#[tauri::command]
pub async fn export_mcp_audit_logs(
    days: i64,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<AuditExport, AppError> {
    let export = export_mcp_audit_logs_with_state(days, state.inner()).await?;
    let export_value = serde_json::to_value(&export)?;
    require_danger_action_confirmation(
        DangerActionConfirmationRequest {
            action_type: "mcp_audit_export",
            target_ids_for_new_challenge: &[],
            requested_target: None,
            affected_count: None,
            reference: None,
            preflight_scope_arguments: None,
            arguments: &serde_json::json!({
                "days": days,
                "export_digest": hash_json_value(&export_value)?,
            }),
            arguments_summary: &format!(
                "导出最近 {days} 天的 MCP 审计快照；原始日志不会复制进 confirmation grant。"
            ),
        },
        &window,
        state.inner(),
    )
    .await?;
    Ok(export)
}

async fn export_mcp_audit_logs_with_state(
    days: i64,
    state: &Arc<AppState>,
) -> Result<AuditExport, AppError> {
    state.mcp_audit_read_gateway.export_logs(state, days).await
}

#[tauri::command]
pub async fn cleanup_mcp_audit_logs(
    retention_days: i64,
    confirmation_evidence: Option<DangerActionConfirmationReference>,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<usize, AppError> {
    let app_state = state.inner();
    let window = &window;
    let confirmation_evidence = confirmation_evidence.as_ref();
    orchestrate_mcp_audit_cleanup(
        retention_days,
        |days| {
            McpAuditRetentionDays::try_from(days)
                .map_err(|_| invalid_mcp_audit_retention_error())
        },
        || require_mcp_audit_cleanup_effects_allowed(app_state),
        |retention| async move {
            let store = app_state.mcp_audit_store.lock().await;
            store
                .count_cleanup_candidates(&retention)
                .map_err(map_mcp_audit_cleanup_error)
        },
        |retention, candidate_count| async move {
            let retention_days = retention.get();
            let preflight_scope_arguments = mcp_audit_cleanup_preflight_scope_arguments(
                retention_days,
                candidate_count,
            );
            let confirmation_arguments = serde_json::json!({
                "retention_days": retention_days,
                "predicate_version": MCP_AUDIT_CLEANUP_PREDICATE_VERSION,
                "candidate_count": candidate_count,
                "cutoff_utc": retention.cutoff_rfc3339(),
            });
            require_danger_action_confirmation(
                DangerActionConfirmationRequest {
                    action_type: "mcp_audit_cleanup",
                    target_ids_for_new_challenge: &[],
                    requested_target: None,
                    affected_count: Some(candidate_count),
                    reference: confirmation_evidence,
                    preflight_scope_arguments: Some(&preflight_scope_arguments),
                    arguments: &confirmation_arguments,
                    arguments_summary: &format!(
                        "删除创建时间早于 {} 的 MCP 审计记录；保留期 {retention_days} 天，后端候选数量 {candidate_count}。",
                        retention.cutoff_rfc3339()
                    ),
                },
                window,
                app_state,
            )
            .await
        },
        |retention, candidate_count| async move {
            let store = app_state.mcp_audit_store.lock().await;
            // Confirmation can await while persistence degrades. Re-check only
            // after owning the store guard, then let the domain transaction
            // atomically compare the confirmed count and delete predicate.
            require_mcp_audit_cleanup_effects_allowed(app_state)?;
            store
                .cleanup(retention, candidate_count)
                .map_err(map_mcp_audit_cleanup_error)
        },
    )
    .await
}

/// Stable product orchestration seam for governed audit cleanup.
///
/// The command above is the production caller. Keeping validation, the global
/// effects gate, Rust-owned confirmation, and the mutation as explicit ports
/// lets the frozen D063 suite exercise every fail-closed transition without
/// pretending a concrete `WebviewWindow` command is a MockRuntime command.
async fn orchestrate_mcp_audit_cleanup<
    Retention,
    Validate,
    Effects,
    Prepare,
    PrepareFuture,
    Prepared,
    Confirm,
    ConfirmFuture,
    Mutate,
    MutateFuture,
>(
    retention_days: i64,
    validate: Validate,
    require_effects_allowed: Effects,
    prepare: Prepare,
    require_native_confirmation: Confirm,
    mutate: Mutate,
) -> Result<usize, AppError>
where
    Retention: Clone,
    Prepared: Clone,
    Validate: FnOnce(i64) -> Result<Retention, AppError>,
    Effects: Fn() -> Result<(), AppError>,
    Prepare: FnOnce(Retention) -> PrepareFuture,
    PrepareFuture: std::future::Future<Output = Result<Prepared, AppError>>,
    Confirm: FnOnce(Retention, Prepared) -> ConfirmFuture,
    ConfirmFuture: std::future::Future<Output = Result<(), AppError>>,
    Mutate: FnOnce(Retention, Prepared) -> MutateFuture,
    MutateFuture: std::future::Future<Output = Result<usize, AppError>>,
{
    let retention = validate(retention_days)?;
    require_effects_allowed()?;
    let prepared = prepare(retention.clone()).await?;
    require_native_confirmation(retention.clone(), prepared.clone()).await?;
    require_effects_allowed()?;
    mutate(retention, prepared).await
}

/// Test-only access to the exact production-called orchestration seam. This
/// forwarding surface is not compiled into release builds, so sibling modules
/// cannot inject alternate effects, confirmation, or mutation authorities.
#[cfg(test)]
pub(crate) async fn run_d063_cleanup_orchestration_harness<
    Retention,
    Validate,
    Effects,
    Prepare,
    PrepareFuture,
    Prepared,
    Confirm,
    ConfirmFuture,
    Mutate,
    MutateFuture,
>(
    retention_days: i64,
    validate: Validate,
    require_effects_allowed: Effects,
    prepare: Prepare,
    require_native_confirmation: Confirm,
    mutate: Mutate,
) -> Result<usize, AppError>
where
    Retention: Clone,
    Prepared: Clone,
    Validate: FnOnce(i64) -> Result<Retention, AppError>,
    Effects: Fn() -> Result<(), AppError>,
    Prepare: FnOnce(Retention) -> PrepareFuture,
    PrepareFuture: std::future::Future<Output = Result<Prepared, AppError>>,
    Confirm: FnOnce(Retention, Prepared) -> ConfirmFuture,
    ConfirmFuture: std::future::Future<Output = Result<(), AppError>>,
    Mutate: FnOnce(Retention, Prepared) -> MutateFuture,
    MutateFuture: std::future::Future<Output = Result<usize, AppError>>,
{
    orchestrate_mcp_audit_cleanup(
        retention_days,
        validate,
        require_effects_allowed,
        prepare,
        require_native_confirmation,
        mutate,
    )
    .await
}

#[tauri::command]
pub async fn rotate_mcp_audit_key(
    confirmation_evidence: Option<DangerActionConfirmationReference>,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    require_danger_action_confirmation(
        DangerActionConfirmationRequest {
            action_type: "mcp_audit_key_rotation",
            target_ids_for_new_challenge: &[],
            requested_target: None,
            affected_count: None,
            reference: confirmation_evidence.as_ref(),
            preflight_scope_arguments: None,
            arguments: &serde_json::json!({ "operation": "rotate_mcp_audit_key_epoch" }),
            arguments_summary: "轮换 MCP 审计加密 epoch，并保留历史 epoch 供旧记录解密。",
        },
        &window,
        state.inner(),
    )
    .await?;
    let mut store = state.mcp_audit_store.lock().await;
    let timestamp_epoch = chrono::Utc::now().timestamp().max(0) as u64;
    let epoch = timestamp_epoch.max(store.key_config().epoch.saturating_add(1));
    let secret_store = KeyringSecretStore;
    let material = create_mcp_audit_key_material(epoch, &secret_store).map_err(AppError::from)?;
    let secret_ref = material.config.key_ref.clone().unwrap_or_default();
    let snapshot = store.clone();
    if let Err(error) = store.rotate_key_material(material) {
        let _ = secret_store.delete(&secret_ref);
        return Err(AppError::from(error));
    }
    if let Err(error) =
        save_mcp_audit_keyring_to_path(&mcp_audit_keyring_path(), store.key_configs())
    {
        *store = snapshot;
        let _ = secret_store.delete(&secret_ref);
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_privacy_policy(
    state: State<'_, Arc<AppState>>,
) -> Result<PrivacyPolicy, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("PrivacyPolicyStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let engine = state.privacy_engine.lock().await;
    Ok(engine.policy().clone())
}

#[tauri::command]
pub async fn set_privacy_policy(
    policy: PrivacyPolicy,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    save_privacy_policy_to_path(&privacy_policy_path(), &policy)?;
    let mut engine = state.privacy_engine.lock().await;
    engine.set_policy(policy);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_network_consent::{
        authorize_provider_network_dispatch, NetworkConsentSubmissionScope,
        ProviderNetworkAuthorization,
    };
    use openlife_core::llm::{provider_endpoint_is_official, ChatMessage};

    const W84_IMPORT_CURRENT_NAME_SECRET: &str = "W84_IMPORT_CURRENT_LIFEMODEL_SECRET";
    const W84_IMPORT_PAYLOAD_NAME_SECRET: &str = "W84_IMPORT_PAYLOAD_LIFEMODEL_SECRET";
    const W84_IMPORT_CURRENT_MESSAGE_SECRET: &str = "W84_IMPORT_CURRENT_MESSAGE_SECRET";
    const W84_IMPORT_PAYLOAD_MESSAGE_SECRET: &str = "W84_IMPORT_PAYLOAD_MESSAGE_SECRET";
    const W84_IMPORT_CURRENT_VECTOR_SECRET: &str = "W84_IMPORT_CURRENT_VECTOR_SECRET";
    const W84_IMPORT_PAYLOAD_VECTOR_SECRET: &str = "W84_IMPORT_PAYLOAD_VECTOR_SECRET";

    #[test]
    fn d063_invalid_cleanup_retention_is_a_stable_config_error() {
        for invalid in [i64::MIN, -1, 0, MCP_AUDIT_RETENTION_MAX_DAYS + 1, i64::MAX] {
            let error = validate_mcp_audit_retention_days(invalid).unwrap_err();
            let AppError::Config { message, hint } = error else {
                panic!("invalid cleanup retention must not be reported as an internal error");
            };
            assert_eq!(message, "invalid_mcp_audit_retention_days");
            assert_eq!(
                hint.as_deref(),
                Some("retention_days_must_be_1_through_3650")
            );
        }
    }

    #[test]
    fn d063_cleanup_preflight_digest_binds_retention_predicate_and_server_count() {
        let scoped_view = |retention_days, candidate_count| {
            let arguments =
                mcp_audit_cleanup_preflight_scope_arguments(retention_days, candidate_count);
            danger_action_preflight_for_action_scoped(
                "mcp_audit_cleanup",
                false,
                DangerActionPreflightScope {
                    target_ids: vec![],
                    affected_count: Some(candidate_count),
                    preflight_scope_arguments: Some(arguments),
                },
            )
            .unwrap()
        };
        let expected = scoped_view(90, 2);
        let changed_retention = scoped_view(30, 2);
        let changed_count = scoped_view(90, 3);

        assert_eq!(expected.affected_item_count, 2);
        assert_ne!(
            expected.confirmation_scope_digest,
            changed_retention.confirmation_scope_digest
        );
        assert_ne!(
            expected.confirmation_scope_digest,
            changed_count.confirmation_scope_digest
        );
    }

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
    fn masked_provider_key_is_bound_to_the_same_provider_endpoint_identity() {
        let mut current = AppConfig::default();
        current.llm.provider = "openai".into();
        current.llm.openai_base = "https://api.openai.com/v1".into();
        current.llm.openai_key = "sk-current-openai".into();

        let mut same = current.clone();
        same.llm.openai_key = KEY_MASK.into();
        assert_eq!(
            resolve_submitted_provider_api_key(&same, &current),
            "sk-current-openai"
        );

        let mut changed_provider = same.clone();
        changed_provider.llm.provider = "deepseek".into();
        changed_provider.llm.openai_base = "https://api.deepseek.com".into();
        assert!(resolve_submitted_provider_api_key(&changed_provider, &current).is_empty());

        let mut changed_endpoint = same;
        changed_endpoint.llm.openai_base = "https://capture.example/v1".into();
        assert!(resolve_submitted_provider_api_key(&changed_endpoint, &current).is_empty());
    }

    #[test]
    fn only_canonical_provider_endpoint_can_implicitly_use_environment_credentials() {
        let mut config = AppConfig::default();
        config.llm.provider = "openai".into();
        config.llm.openai_base = "https://api.openai.com/v1/".into();
        assert!(provider_endpoint_is_official(
            &config.llm.provider,
            &config.llm.openai_base,
        ));

        config.llm.openai_base = "https://proxy.example/v1".into();
        assert!(!provider_endpoint_is_official(
            &config.llm.provider,
            &config.llm.openai_base,
        ));
    }

    #[test]
    fn provider_credential_version_changes_only_with_secret_identity() {
        let mut current = AppConfig::default();
        current.llm.provider = "openai".into();
        current.llm.openai_base = "https://api.openai.com/v1".into();
        current.llm.openai_key = "sk-current".into();
        current.llm.credential_version = 7;

        let mut masked_same = current.clone();
        masked_same.llm.openai_key = KEY_MASK.into();
        assert_eq!(
            resolved_provider_credential_version(&masked_same, &current),
            7
        );

        let mut replaced = masked_same.clone();
        replaced.llm.openai_key = "sk-replaced".into();
        assert_eq!(resolved_provider_credential_version(&replaced, &current), 8);

        let mut moved = masked_same;
        moved.llm.openai_base = "https://custom.example/v1".into();
        assert_eq!(resolved_provider_credential_version(&moved, &current), 8);
    }

    #[tokio::test]
    async fn explicit_provider_probe_uses_scheduler_receipt_and_keeps_loopback_capability() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let captured = Arc::new(std::sync::Mutex::new(String::new()));
        let captured_server = Arc::clone(&captured);
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 16 * 1024];
            let read = socket.read(&mut request).await.unwrap();
            *captured_server.lock().unwrap() =
                String::from_utf8_lossy(&request[..read]).to_string();
            let body = r#"{"choices":[{"message":{"content":"pong"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut runtime_config = state.config.lock().await.clone();
        runtime_config.system.network_policy = openlife_core::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        state.replace_provider_runtime_config(runtime_config).await;
        let mut config = AppConfig::default();
        config.llm.provider = "openai".into();
        config.llm.openai_base = base;
        config.llm.openai_key = "sk-test".into();
        config.llm.chat_model = "gpt-test".into();
        let dir = tempfile::tempdir().unwrap();
        let validation_path = dir.path().join("provider-validation.json");

        let result =
            test_llm_connection_with_state_and_validation_path(config, &state, &validation_path)
                .await
                .unwrap();
        server.await.unwrap();
        assert!(result.ok);
        assert_eq!(result.validation_status, "validated");
        let receipt = result.provider_invocation_receipt.unwrap();
        assert_eq!(receipt.status, ProviderInvocationStatus::Completed);
        assert_eq!(receipt.provider, "openai");
        assert_eq!(receipt.model, "gpt-test");
        assert!(!receipt.simulated);
        let request = captured.lock().unwrap().clone();
        assert!(request.contains(r#""content":"ping""#));
        let persisted =
            crate::provider_validation::load_provider_validation_record_from_path(&validation_path)
                .as_record()
                .expect("completed probe must persist a valid validation record")
                .clone();
        assert_eq!(
            persisted
                .invocation_receipt
                .as_ref()
                .map(|receipt| receipt.request_id.as_str()),
            Some(receipt.request_id.as_str())
        );
        let raw = std::fs::read_to_string(validation_path).unwrap();
        assert!(!raw.contains("ping"));
        assert!(!raw.contains("pong"));
        assert!(!raw.contains("sk-test"));
    }

    #[tokio::test]
    async fn explicit_provider_probe_remote_unknown_is_persisted_and_never_reports_success() {
        use tokio::io::AsyncReadExt;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request).await.unwrap();
            // Drop after the adapter start boundary without a terminal response.
        });
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut runtime_config = state.config.lock().await.clone();
        runtime_config.system.network_policy = openlife_core::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        state.replace_provider_runtime_config(runtime_config).await;
        let mut config = AppConfig::default();
        config.llm.provider = "openai".into();
        config.llm.openai_base = base;
        config.llm.openai_key = "sk-test".into();
        config.llm.chat_model = "gpt-test".into();
        config.system.network_policy = openlife_core::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        let validation_config = config.clone();
        let dir = tempfile::tempdir().unwrap();
        let validation_path = dir.path().join("provider-validation.json");

        let result =
            test_llm_connection_with_state_and_validation_path(config, &state, &validation_path)
                .await
                .unwrap();
        server.await.unwrap();
        assert!(!result.ok);
        assert_eq!(result.validation_status, "remote_unknown");
        assert!(result.message.contains("unknown"));
        assert_eq!(
            result
                .provider_invocation_receipt
                .as_ref()
                .map(|receipt| receipt.status),
            Some(ProviderInvocationStatus::RemoteUnknown)
        );
        let persisted =
            crate::provider_validation::load_provider_validation_record_from_path(&validation_path)
                .as_record()
                .expect("remote-unknown probe must persist a valid validation record")
                .clone();
        assert_eq!(
            crate::provider_validation::summarize_provider_validation(
                &validation_config,
                Some(&persisted),
                chrono::Utc::now(),
            )
            .status,
            "remote_unknown"
        );
    }

    #[tokio::test]
    async fn explicit_provider_probe_ask_stages_review_and_performs_zero_dispatch() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut runtime_config = state.config.lock().await.clone();
        runtime_config.system.network_policy = openlife_core::config::NetworkPolicy::default();
        state.replace_provider_runtime_config(runtime_config).await;
        let mut config = AppConfig::default();
        config.llm.provider = "openai".into();
        config.llm.openai_base = base;
        config.llm.openai_key = "sk-test".into();
        config.llm.chat_model = "gpt-test".into();
        let dir = tempfile::tempdir().unwrap();
        let validation_path = dir.path().join("provider-validation.json");

        let result =
            test_llm_connection_with_state_and_validation_path(config, &state, &validation_path)
                .await
                .unwrap();
        assert!(!result.ok);
        assert_eq!(result.validation_status, "consent_required");
        assert!(result.review_proposal_id.is_some());
        assert!(result.provider_invocation_receipt.is_none());
        assert!(!validation_path.exists());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err(),
            "an Ask decision must stage review before any provider dispatch"
        );
    }

    #[tokio::test]
    async fn provider_network_ask_reuses_review_workflow_and_allow_once_is_recoverable() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let policy = openlife_core::config::NetworkPolicy::default();
        let capability = "provider.openai";
        let url = "https://api.openai.com/v1/chat/completions";
        let ask = resolve_network_policy_decision(&policy, url, capability).unwrap();

        let proposal_id = match authorize_provider_network_dispatch(
            &state,
            &policy,
            &ask,
            url,
            capability,
            "openai",
            None,
            NetworkConsentSubmissionScope::ExplicitCommand,
        )
        .await
        .unwrap()
        {
            ProviderNetworkAuthorization::ConsentRequired { proposal_id } => proposal_id,
            _ => panic!("Ask must stage consent without dispatch authorization"),
        };
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            proposal.proposal_type,
            openlife_core::agent::ProposalType::ToolPermission
        );
        assert_eq!(
            proposal.status,
            openlife_core::agent::ProposalStatus::Pending
        );
        assert_eq!(
            proposal.source,
            openlife_core::agent::ProposalSource::NetworkConsent,
            "an explicit Settings probe must not claim Main Chat proposal authority"
        );

        crate::commands::proposal::accept_proposal_with_state(proposal_id, &state)
            .await
            .unwrap();
        let authorized = authorize_provider_network_dispatch(
            &state,
            &policy,
            &ask,
            url,
            capability,
            "openai",
            None,
            NetworkConsentSubmissionScope::ExplicitCommand,
        )
        .await
        .unwrap();
        match authorized {
            ProviderNetworkAuthorization::Authorized {
                network_policy,
                network_policy_decision,
                permission_id,
                ..
            } => {
                assert_eq!(
                    network_policy_decision.disposition,
                    openlife_core::network_client::NetworkPolicyDisposition::Allow
                );
                assert_eq!(
                    network_policy
                        .tool_overrides
                        .get(capability)
                        .map(String::as_str),
                    Some("allow")
                );
                assert!(permission_id.is_some());
            }
            _ => panic!("accepted AllowOnce must authorize exactly one retry"),
        }

        assert!(matches!(
            authorize_provider_network_dispatch(
                &state,
                &policy,
                &ask,
                url,
                capability,
                "openai",
                None,
                NetworkConsentSubmissionScope::ExplicitCommand,
            )
            .await
            .unwrap(),
            ProviderNetworkAuthorization::ConsentRequired { .. }
        ));
    }

    #[tokio::test]
    async fn replacing_provider_runtime_config_invalidates_cached_provider_truth() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        *state.provider_health_cache.lock().await = Some(crate::state::ProviderHealthCache {
            providers: Vec::new(),
            checked_at: chrono::Utc::now().to_rfc3339(),
            identity_digest: "stale-provider-identity".into(),
        });
        let mut replacement = AppConfig::default();
        replacement.llm.provider = "openai".into();
        replacement.llm.openai_base = "https://api.openai.com/v1/changed-path".into();
        replacement.llm.chat_model = "changed-model".into();
        replacement.llm.openai_key = String::new();

        replace_runtime_provider_config(&state, replacement).await;

        assert!(state.provider_health_cache.lock().await.is_none());
        let scheduler = state.scheduler.lock().await;
        assert_eq!(
            scheduler.openai_base,
            "https://api.openai.com/v1/changed-path"
        );
        assert_eq!(scheduler.chat_model, "changed-model");
        assert!(scheduler.openai_key.is_empty());
    }

    #[tokio::test]
    async fn concurrent_provider_replacement_never_exposes_a_mixed_status_generation() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let configured = |suffix: &str, credential_version: u64| {
            let mut config = AppConfig::default();
            config.local_model = format!("local-{suffix}");
            config.prefer_local_model = false;
            config.llm.provider = "openai".into();
            config.llm.openai_base = format!("https://api.example.test/{suffix}");
            config.llm.openai_key = format!("sk-{suffix}");
            config.llm.chat_model = format!("model-{suffix}");
            config.llm.credential_version = credential_version;
            config
        };
        let first = configured("generation-a", 41);
        let second = configured("generation-b", 42);
        replace_runtime_provider_config(&state, first.clone()).await;

        let writer = async {
            for index in 0..64 {
                let next = if index % 2 == 0 {
                    second.clone()
                } else {
                    first.clone()
                };
                replace_runtime_provider_config(&state, next).await;
                tokio::task::yield_now().await;
            }
        };
        let reader = async {
            for _ in 0..128 {
                let snapshot = state.provider_runtime_snapshot().await;
                assert!(
                    snapshot.coherent,
                    "a status snapshot must never combine config and adapter generations"
                );
                let observed = (
                    snapshot.config.llm.openai_base.as_str(),
                    snapshot.scheduler.openai_base.as_str(),
                    snapshot.config.llm.chat_model.as_str(),
                    snapshot.scheduler.chat_model.as_str(),
                    snapshot.config.llm.credential_version,
                    snapshot.scheduler.provider_credential_version(),
                );
                assert!(matches!(
                    observed,
                    (
                        "https://api.example.test/generation-a",
                        "https://api.example.test/generation-a",
                        "model-generation-a",
                        "model-generation-a",
                        41,
                        41
                    ) | (
                        "https://api.example.test/generation-b",
                        "https://api.example.test/generation-b",
                        "model-generation-b",
                        "model-generation-b",
                        42,
                        42
                    )
                ));
                assert!(!snapshot
                    .scheduler
                    .provider_config_generation()
                    .trim()
                    .is_empty());
                tokio::task::yield_now().await;
            }
        };

        tokio::join!(writer, reader);
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
        assert!(view.confirmation_required);
        assert!(!view.requires_typed_confirmation);
        assert!(view.confirmation_phrase.is_none());
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
        assert!(view.confirmation_required);
        assert!(!view.requires_typed_confirmation);
        assert!(view.confirmation_phrase.is_none());
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
                preflight_scope_arguments: None,
            },
        )
        .unwrap();

        assert_eq!(view.action_type, "agent_run_bulk_delete");
        assert!(view.writes_durable_state);
        assert!(view.confirmation_required);
        assert!(!view.requires_typed_confirmation);
        assert!(view.confirmation_phrase.is_none());
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
                preflight_scope_arguments: None,
            },
        )
        .unwrap();
        assert_eq!(vector.action_type, "vector_rebuild");
        assert!(vector.confirmation_required);
        assert!(!vector.requires_typed_confirmation);
        assert!(vector.confirmation_phrase.is_none());
        assert_eq!(vector.affected_item_count, 12);
        assert!(vector
            .source_refs
            .iter()
            .any(|source| source == "final_command:rebuild_memory_index"));
    }

    #[test]
    fn deterministic_preflight_view_cannot_mint_confirmation_authority() {
        let view = danger_action_preflight_for_action_scoped(
            "agent_run_delete",
            false,
            DangerActionPreflightScope {
                target_ids: vec!["run-confirm-1".to_string()],
                affected_count: Some(1),
                preflight_scope_arguments: None,
            },
        )
        .unwrap();
        assert!(view.confirmation_required);
        assert!(!view.requires_typed_confirmation);
        assert!(view.confirmation_phrase.is_none());
        assert!(view.preflight_id.is_empty());
        assert!(!view
            .source_refs
            .iter()
            .any(|source| source == "native_confirmation:server_challenge_pending"));
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
            let profile = openlife_core::embedding::EmbeddingProfile::new(
                openlife_core::embedding::EmbeddingRouteKind::DeterministicHash,
                "openlife-test",
                "settings-import-test-v1",
                "builtin:test",
                "settings-import-test-artifact-v1",
                4,
            )
            .unwrap();
            store
                .insert(
                    "w84-current-session",
                    W84_IMPORT_CURRENT_VECTOR_SECRET,
                    &[0.1, 0.2, 0.3, 0.4],
                    &profile,
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
    async fn governed_import_missing_memory_targets_preserves_existing_memory() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;
        let mut payload = import_payload();
        payload.as_object_mut().unwrap().remove("messages");
        payload.as_object_mut().unwrap().remove("vectors");

        let result = import_all_data_with_state_for_governed_import(
            payload,
            &state,
            GovernedDataImportRequest {
                purpose: "manual_restore".into(),
                explicit_user_intent: true,
                create_pre_change_snapshot: true,
                import_targets: vec!["life_model".into()],
            },
        )
        .await
        .unwrap();

        assert_eq!(result["messages_targeted"], false);
        assert_eq!(result["vectors_targeted"], false);
        assert_eq!(result["imported_message_count"], 0);
        assert_eq!(result["imported_vector_count"], 0);
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
    async fn governed_import_skips_derived_vectors_and_reports_only_applied_rows() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;
        let profile = openlife_core::embedding::EmbeddingProfile::new(
            openlife_core::embedding::EmbeddingRouteKind::DeterministicHash,
            "openlife-test",
            "settings-import-canonical-v1",
            "builtin:test",
            "settings-import-canonical-artifact-v1",
            4,
        )
        .unwrap();
        let owner = openlife_core::vectors::CanonicalVectorOwnerRef::new(
            "knowledge_note",
            "settings-import-owner",
        )
        .unwrap();
        state
            .vector_store
            .lock()
            .await
            .project_memory_embedding(
                "outbox:settings-import-owner",
                &owner,
                "canonical-settings-session",
                "CANONICAL_DESTINATION_VECTOR",
                &[0.1, 0.3, 0.2, 0.4],
                &profile,
            )
            .unwrap();

        let mut payload = import_payload();
        let portable = payload["vectors"][0].clone();
        let mut canonical = portable.clone();
        canonical["source"] = serde_json::Value::String(owner.source());
        canonical["content"] = serde_json::Value::String("SPOOFED_CANONICAL_VECTOR".into());
        let mut legacy_chat = portable;
        legacy_chat["source"] = serde_json::Value::String("user_message".into());
        legacy_chat["content"] = serde_json::Value::String("LEGACY_CHAT_VECTOR".into());
        payload["vectors"]
            .as_array_mut()
            .unwrap()
            .extend([canonical, legacy_chat]);

        let result = import_all_data_with_state_for_governed_import(
            payload,
            &state,
            GovernedDataImportRequest::manual_restore_all_targets(),
        )
        .await
        .unwrap();

        assert_eq!(result["supplied_vector_count"], 3);
        assert_eq!(result["imported_vector_count"], 1);
        assert_eq!(result["skipped_vector_count"], 2);
        assert_eq!(result["skipped_canonical_vector_count"], 1);
        assert_eq!(result["skipped_legacy_chat_vector_count"], 1);
        let vectors = state.vector_store.lock().await.export_all_chunks().unwrap();
        assert!(vectors
            .iter()
            .any(|chunk| chunk.content == "CANONICAL_DESTINATION_VECTOR"));
        assert!(vectors
            .iter()
            .any(|chunk| chunk.content == W84_IMPORT_PAYLOAD_VECTOR_SECRET));
        assert!(!vectors
            .iter()
            .any(|chunk| chunk.content == "SPOOFED_CANONICAL_VECTOR"));
        assert!(!vectors
            .iter()
            .any(|chunk| chunk.content == "LEGACY_CHAT_VECTOR"));

        let exported = export_all_data_with_state(&state).await.unwrap();
        assert_eq!(
            exported["vector_export_semantics"],
            "portable_only_canonical_and_chat_projections_derived"
        );
        assert_eq!(exported["vectors"].as_array().unwrap().len(), 1);
        assert_eq!(
            exported["vectors"][0]["content"],
            W84_IMPORT_PAYLOAD_VECTOR_SECRET
        );
    }

    #[tokio::test]
    async fn governed_import_vector_tombstone_failure_restores_all_preimport_truth() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;
        state
            .vector_store
            .lock()
            .await
            .project_conversation_tombstone("settings-import-tombstone", "blocked-import-session")
            .unwrap();
        let mut payload = import_payload();
        payload["messages"][0]["session_id"] =
            serde_json::Value::String("blocked-import-session".into());
        payload["vectors"][0]["session_id"] =
            serde_json::Value::String("blocked-import-session".into());

        let error = import_all_data_with_state_for_governed_import(
            payload,
            &state,
            GovernedDataImportRequest::manual_restore_all_targets(),
        )
        .await
        .expect_err("a projected conversation tombstone must reject archive resurrection");
        assert!(error.message().contains("已自动回滚"));
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
