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

use crate::legacy_write_convergence::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::persist_life_model;
use crate::storage::{
    app_data_dir, load_onboarding_status_from_path, mcp_audit_keyring_path, onboarding_status_path,
    privacy_policy_path, save_mcp_audit_keyring_to_path, save_onboarding_status_to_path,
    save_privacy_policy_to_path, OnboardingStatus,
};
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportRequest {
    pub purpose: String,
    pub explicit_user_intent: bool,
    pub create_pre_change_snapshot: bool,
    pub import_targets: Vec<String>,
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

    // ModelRouter is now graduated from experimental (Beta)
    let router = openlife_core::agent::ModelRouter::new();
    new_scheduler = new_scheduler.with_model_router(router);
    eprintln!("[Scheduler] ModelRouter enabled (Beta)");

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
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
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
    {
        let store = state.memory_store.lock().await;
        store
            .replace_all_messages(&messages)
            .map_err(AppError::from)?;
    }
    {
        let store = state.vector_store.lock().await;
        store.replace_all_chunks(&vectors).map_err(AppError::from)?;
    }
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
}

#[tauri::command]
pub async fn test_llm_connection(
    mut config: AppConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<LlmConnectionTestResult, AppError> {
    let provider = config.llm.provider.clone();
    let label = provider_label(&provider);

    let current_key = {
        let cfg = state.config.lock().await;
        cfg.llm.openai_key.clone()
    };
    let resolved_key = resolve_masked_api_key(&config.llm.openai_key, &current_key);
    if !resolved_key.trim().is_empty() {
        config.llm.openai_key = resolved_key;
    }

    let api_key = effective_api_key(&provider, &config.llm.openai_key);
    if api_key.trim().is_empty() {
        return Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: "未检测到 API Key，请填写后再测试。".to_string(),
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
        .map_err(|e| AppError::external(format!("API request failed: {}", e)))?;
    let status = res.status();
    let text = res.text().await.unwrap_or_default();
    if status.is_success() {
        let model_note = if model.to_lowercase().contains("reasoner") {
            " 当前选择的是推理模型，首次可见输出可能更慢；试用聊天建议优先使用 deepseek-chat 这类通用聊天模型。"
        } else {
            ""
        };
        Ok(LlmConnectionTestResult {
            ok: true,
            provider: label,
            message: format!("连接成功，云端模型可用。{}", model_note),
        })
    } else {
        Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: format!(
                "连接失败 ({}): {}",
                status,
                text.chars().take(240).collect::<String>()
            ),
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
    state: State<'_, Arc<AppState>>,
) -> Result<usize, AppError> {
    let store = state.mcp_audit_store.lock().await;
    store.cleanup(retention_days).map_err(AppError::from)
}

#[tauri::command]
pub async fn rotate_mcp_audit_key(state: State<'_, Arc<AppState>>) -> Result<(), AppError> {
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

#[tauri::command]
pub async fn has_completed_onboarding() -> Result<bool, AppError> {
    Ok(load_onboarding_status_from_path(&onboarding_status_path()).completed)
}

#[tauri::command]
pub async fn mark_onboarding_completed() -> Result<(), AppError> {
    let status = OnboardingStatus {
        completed: true,
        completed_at: Some(chrono::Utc::now().to_rfc3339()),
    };
    save_onboarding_status_to_path(&onboarding_status_path(), &status)
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
    }

    #[test]
    fn resolve_masked_api_key_uses_submitted_new_key() {
        assert_eq!(resolve_masked_api_key("sk-new", "sk-current"), "sk-new");
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
