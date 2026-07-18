use openlife_core::llm::{
    chat_completions_url, effective_api_key_for_endpoint, ProviderInvocationStatus,
};
use openlife_core::network_client::resolve_network_policy_decision;
use std::sync::Arc;
use tauri::State;

use crate::state::AppState;

#[derive(serde::Serialize, Clone)]
pub struct ProviderStatus {
    pub name: String,
    pub enabled: bool,
    /// Compatibility boolean for existing consumers. It is true only when a
    /// fresh, identity-matching, completed adapter receipt exists.
    pub available: bool,
    pub configured: bool,
    pub availability_status: String,
    pub health_is_estimated: bool,
    pub last_error: Option<String>,
    pub latency_ms: Option<u64>,
    pub last_checked: Option<String>,
    pub last_receipt_status: Option<String>,
    pub last_receipt_request_id: Option<String>,
    pub network_policy_decision_id: Option<String>,
    pub network_policy_disposition: Option<String>,
    pub network_policy_reason_code: Option<String>,
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
    get_model_router_status_with_state(state.inner()).await
}

/// Project provider readiness from configuration plus the last durable
/// validation receipt. Reading Settings must never itself become an external
/// provider action, so this path performs no Ollama or cloud network request.
pub(crate) async fn get_model_router_status_with_state(
    state: &Arc<AppState>,
) -> Result<ModelRouterStatus, String> {
    get_model_router_status_with_state_and_validation_path(
        state,
        &crate::provider_validation::provider_validation_path(),
    )
    .await
}

async fn get_model_router_status_with_state_and_validation_path(
    state: &Arc<AppState>,
    validation_path: &std::path::Path,
) -> Result<ModelRouterStatus, String> {
    let runtime = state.provider_runtime_snapshot().await;
    let config = runtime.config;
    let scheduler = runtime.scheduler;
    let validation_load =
        crate::provider_validation::load_provider_validation_record_from_path(validation_path);
    let mut validation = crate::provider_validation::summarize_loaded_provider_validation(
        &config,
        &validation_load,
        chrono::Utc::now(),
    );
    if !runtime.coherent {
        validation.validated = false;
        validation.status = "runtime_generation_incoherent";
        validation.last_error = Some("provider_runtime_generation_incoherent".into());
    }
    let cloud_receipt = runtime
        .coherent
        .then(|| {
            crate::provider_validation::current_loaded_provider_validation_receipt(
                &config,
                &validation_load,
            )
        })
        .flatten();
    let cloud_receipt_status = cloud_receipt.map(|receipt| match receipt.status {
        ProviderInvocationStatus::Completed => "completed",
        ProviderInvocationStatus::Failed => "failed",
        ProviderInvocationStatus::RemoteUnknown => "remote_unknown",
    });
    let cloud_latency_ms = cloud_receipt.and_then(|receipt| {
        let duration = receipt
            .finished_at
            .signed_duration_since(receipt.started_at)
            .num_milliseconds();
        (duration >= 0).then_some(duration as u64)
    });
    let cloud_last_checked = validation
        .validated_at
        .clone()
        .or_else(|| validation.failed_at.clone());

    let provider_url = chat_completions_url(&scheduler.provider, &scheduler.openai_base);
    let network_capability = format!("provider.{}", scheduler.provider);
    let network_policy_decision = runtime
        .coherent
        .then(|| {
            resolve_network_policy_decision(
                &config.system.network_policy,
                &provider_url,
                &network_capability,
            )
            .ok()
        })
        .flatten();
    let cloud_configured = runtime.coherent
        && validation.configured
        && !effective_api_key_for_endpoint(
            &scheduler.provider,
            &scheduler.openai_base,
            &scheduler.openai_key,
        )
        .trim()
        .is_empty();
    let cloud_availability_status = match validation.status {
        "unvalidated" => "never_validated",
        other => other,
    };
    let cloud = ProviderStatus {
        name: scheduler.provider.clone(),
        enabled: cloud_configured,
        available: validation.validated,
        configured: cloud_configured,
        availability_status: cloud_availability_status.into(),
        health_is_estimated: false,
        last_error: validation.last_error.clone(),
        latency_ms: cloud_latency_ms,
        last_checked: cloud_last_checked.clone(),
        last_receipt_status: cloud_receipt_status.map(str::to_string),
        last_receipt_request_id: cloud_receipt.map(|receipt| receipt.request_id.clone()),
        network_policy_decision_id: network_policy_decision
            .as_ref()
            .map(|decision| decision.decision_id.clone()),
        network_policy_disposition: network_policy_decision
            .as_ref()
            .map(|decision| decision.disposition.as_str().to_string()),
        network_policy_reason_code: network_policy_decision
            .as_ref()
            .map(|decision| decision.reason_code.clone()),
    };
    let local_configured = !scheduler.local_model.trim().is_empty();
    let local = ProviderStatus {
        name: "ollama".into(),
        enabled: scheduler.prefer_local,
        available: false,
        configured: local_configured,
        availability_status: if local_configured {
            "unknown".into()
        } else {
            "unconfigured".into()
        },
        health_is_estimated: false,
        last_error: None,
        latency_ms: None,
        last_checked: None,
        last_receipt_status: None,
        last_receipt_request_id: None,
        network_policy_decision_id: None,
        network_policy_disposition: None,
        network_policy_reason_code: None,
    };

    Ok(ModelRouterStatus {
        enabled: true,
        providers: vec![local, cloud],
        last_check_at: cloud_last_checked,
        message: Some(if runtime.coherent {
            "Provider status is projected from one atomic runtime generation and the last durable adapter receipt; opening this page does not contact a provider."
                .into()
        } else {
            "Provider config and adapter generations are incoherent; readiness is fail-closed and opening this page performed no provider request."
                .into()
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn model_router_status_read_never_dispatches_to_the_configured_cloud_endpoint() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut config = state.config.lock().await.clone();
        config.local_model = "local-test".into();
        config.prefer_local_model = false;
        config.llm.provider = "openai".into();
        config.llm.openai_base = endpoint;
        config.llm.openai_key = "sk-test".into();
        config.llm.chat_model = "gpt-test".into();
        config.llm.embedding_enabled = false;
        config.system.network_policy = openlife_core::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        state.replace_provider_runtime_config(config).await;

        let dir = tempfile::tempdir().unwrap();
        let status = get_model_router_status_with_state_and_validation_path(
            &state,
            &dir.path().join("missing-provider-validation.json"),
        )
        .await
        .unwrap();
        let cloud = status
            .providers
            .iter()
            .find(|provider| provider.name == "openai")
            .unwrap();
        assert!(!cloud.available);
        assert!(matches!(
            cloud.availability_status.as_str(),
            "never_validated" | "stale" | "unknown"
        ));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err(),
            "a read-only status projection must perform zero cloud dispatches"
        );
    }

    #[tokio::test]
    async fn corrupt_provider_validation_is_unknown_not_never_validated() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut config = state.config.lock().await.clone();
        config.prefer_local_model = false;
        config.llm.provider = "openai".into();
        config.llm.openai_base = "https://api.openai.com/v1".into();
        config.llm.openai_key = "sk-test".into();
        config.llm.chat_model = "gpt-test".into();
        state.replace_provider_runtime_config(config).await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("provider-validation.json");
        std::fs::write(&path, b"not valid provider evidence").unwrap();

        let status = get_model_router_status_with_state_and_validation_path(&state, &path)
            .await
            .unwrap();
        let cloud = status
            .providers
            .iter()
            .find(|provider| provider.name == "openai")
            .unwrap();
        assert!(!cloud.available);
        assert_eq!(cloud.availability_status, "validation_record_corrupt");
        assert_eq!(
            cloud.last_error.as_deref(),
            Some("provider_validation_record_corrupt")
        );
    }

    #[test]
    fn retired_settings_cloud_probe_is_absent_from_the_product_router() {
        let source = include_str!("router.rs");
        assert!(!source.contains(concat!("probe_cloud_", "provider")));
        assert!(!source.contains(concat!("NetworkClient", "::new")));
        assert!(!source.contains(concat!("/", "models")));
    }
}
