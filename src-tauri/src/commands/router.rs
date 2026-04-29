use openlife_core::ollama::resolve_ollama_model;
use std::sync::Arc;
use tauri::State;

use crate::AppState;

#[derive(serde::Serialize, Clone)]
pub struct ProviderStatus {
    pub name: String,
    pub enabled: bool,
    pub available: bool,
    pub health_is_estimated: bool,
    pub last_error: Option<String>,
    pub latency_ms: Option<u64>,
}

#[derive(serde::Serialize, Clone)]
pub struct ModelRouterStatus {
    pub enabled: bool,
    pub providers: Vec<ProviderStatus>,
    pub last_check_at: Option<String>,
}

#[tauri::command]
pub async fn get_model_router_status(
    state: State<'_, Arc<AppState>>,
) -> Result<ModelRouterStatus, String> {
    let _cfg = state.config.lock().await;

    let mut providers = Vec::new();

    // Ollama
    let scheduler = state.scheduler.lock().await;
    let ollama_available = resolve_ollama_model(&scheduler.local_model).await.is_some();
    providers.push(ProviderStatus {
        name: "ollama".to_string(),
        enabled: scheduler.prefer_local,
        available: ollama_available,
        health_is_estimated: true,
        last_error: None,
        latency_ms: None,
    });

    // OpenAI / OpenRouter (unified provider)
    let has_key = !scheduler.openai_key.is_empty();
    let provider_name = scheduler.provider.clone();
    providers.push(ProviderStatus {
        name: provider_name,
        enabled: has_key,
        available: has_key,
        health_is_estimated: true,
        last_error: None,
        latency_ms: None,
    });

    Ok(ModelRouterStatus {
        enabled: false,
        providers,
        last_check_at: Some(chrono::Utc::now().to_rfc3339()),
    })
}
