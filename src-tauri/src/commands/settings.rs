use crate::errors::AppError;
use openlife_core::config::AppConfig;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::{
    chat_completions_url, default_base_for_provider, effective_api_key, provider_label,
};
use openlife_core::mcp_audit::{AuditExport, AuditKeyConfig, KeyMode};
use openlife_core::privacy::PrivacyPolicy;
use openlife_core::scheduler::InferenceScheduler;
use std::sync::Arc;
use tauri::State;

use crate::persist_life_model;
use crate::storage::{
    app_data_dir, load_onboarding_status_from_path, mcp_audit_keyring_path, onboarding_status_path,
    privacy_policy_path, save_mcp_audit_keyring_to_path, save_onboarding_status_to_path,
    save_privacy_policy_to_path, OnboardingStatus,
};
use crate::AppState;

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
pub async fn export_all_data(state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, AppError> {
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
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
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

    let previous_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    let previous_messages = {
        let store = state.memory_store.lock().await;
        store.export_all_messages().map_err(AppError::from)?
    };
    let previous_vectors = {
        let store = state.vector_store.lock().await;
        store.export_all_chunks().map_err(AppError::from)?
    };

    if let Err(import_error) =
        apply_import_payload(state.inner().clone(), life_model, messages, vectors).await
    {
        let rollback_error = apply_import_payload(
            state.inner().clone(),
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
    Ok(())
}

async fn apply_import_payload(
    state: Arc<AppState>,
    life_model: LifeModel,
    messages: Vec<openlife_core::memory::ExportedMessage>,
    vectors: Vec<openlife_core::vectors::ExportedVectorChunk>,
) -> Result<(), AppError> {
    persist_life_model(&state, life_model, false).await?;
    {
        let store = state.memory_store.lock().await;
        store
            .replace_all_messages(&messages)
            .map_err(AppError::from)?;
    }
    {
        let store = state.vector_store.lock().await;
        store
            .replace_all_chunks(&vectors)
            .map_err(AppError::from)?;
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
pub async fn get_privacy_policy(state: State<'_, Arc<AppState>>) -> Result<PrivacyPolicy, AppError> {
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
    use super::{resolve_masked_api_key, KEY_MASK};

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
}
