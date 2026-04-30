use openlife_core::llm::{default_base_for_provider, effective_api_key};
use openlife_core::ollama::resolve_ollama_model;
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    pub last_checked: Option<String>,
}

#[derive(serde::Serialize, Clone)]
pub struct ModelRouterStatus {
    pub enabled: bool,
    pub providers: Vec<ProviderStatus>,
    pub last_check_at: Option<String>,
    pub message: Option<String>,
}

#[tauri::command]
pub async fn get_model_router_status(
    state: State<'_, Arc<AppState>>,
) -> Result<ModelRouterStatus, String> {
    let cfg = state.config.lock().await.clone();

    let mut providers = Vec::new();
    let checked_at = chrono::Utc::now().to_rfc3339();

    // Ollama
    let scheduler = state.scheduler.lock().await.clone();
    let ollama_start = Instant::now();
    let ollama_available = resolve_ollama_model(&scheduler.local_model).await.is_some();
    providers.push(ProviderStatus {
        name: "ollama".to_string(),
        enabled: scheduler.prefer_local,
        available: ollama_available,
        health_is_estimated: false,
        last_error: if ollama_available {
            None
        } else {
            Some(format!(
                "local model '{}' is not available",
                scheduler.local_model
            ))
        },
        latency_ms: Some(ollama_start.elapsed().as_millis() as u64),
        last_checked: Some(checked_at.clone()),
    });

    // OpenAI / OpenRouter (unified provider)
    let provider_name = scheduler.provider.clone();
    let api_key = effective_api_key(&scheduler.provider, &scheduler.openai_key);
    let has_key = !api_key.trim().is_empty();
    let (cloud_available, cloud_latency, cloud_error) = if has_key {
        probe_cloud_provider(&scheduler.provider, &scheduler.openai_base, &api_key).await
    } else {
        (false, None, Some("API key is not configured".to_string()))
    };
    providers.push(ProviderStatus {
        name: provider_name,
        enabled: has_key,
        available: cloud_available,
        health_is_estimated: false,
        last_error: cloud_error,
        latency_ms: cloud_latency,
        last_checked: Some(checked_at.clone()),
    });

    Ok(ModelRouterStatus {
        enabled: cfg.experimental_model_router && scheduler.model_router.is_some(),
        providers,
        last_check_at: Some(checked_at),
        message: if cfg.experimental_model_router {
            None
        } else {
            Some("ModelRouter is disabled; provider health is shown for diagnostics only.".into())
        },
    })
}

async fn probe_cloud_provider(
    provider: &str,
    openai_base: &str,
    api_key: &str,
) -> (bool, Option<u64>, Option<String>) {
    let base = if openai_base.trim().is_empty() {
        default_base_for_provider(provider).to_string()
    } else {
        openai_base.trim().trim_end_matches('/').to_string()
    };
    let url = if base.ends_with("/models") {
        base
    } else {
        format!("{}/models", base)
    };
    let start = Instant::now();
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(e) => return (false, None, Some(e.to_string())),
    };
    match client
        .get(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => {
            (true, Some(start.elapsed().as_millis() as u64), None)
        }
        Ok(res) => {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            (
                false,
                Some(start.elapsed().as_millis() as u64),
                Some(format!(
                    "provider probe failed ({}): {}",
                    status,
                    text.chars().take(180).collect::<String>()
                )),
            )
        }
        Err(e) => (
            false,
            Some(start.elapsed().as_millis() as u64),
            Some(e.to_string()),
        ),
    }
}
