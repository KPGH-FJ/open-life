//! User-selected provider/model registry for Conversation and Task runtimes.
//!
//! The runtime deliberately has no automatic cross-provider routing. Settings own the
//! selection; this registry snapshots the exact executable profile and gives
//! the Turn an immutable binding before any provider request starts.

use crate::state::AppState;
use openlife_core::conversation::ProviderBinding;
use ring::digest::{digest, SHA256};
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileViewModel {
    pub profile_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub endpoint_class: String,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SelectedProviderProfile {
    pub binding: ProviderBinding,
    pub profiles: Vec<ProviderProfileViewModel>,
    pub route: openlife_core::agent::types::ModelRouteTrace,
}

pub(crate) async fn selected_provider_profile(
    state: &Arc<AppState>,
) -> Result<SelectedProviderProfile, String> {
    let runtime = state.provider_runtime_snapshot().await;
    if !runtime.coherent {
        return Err("provider_runtime_generation_incoherent".into());
    }
    let route = runtime.scheduler.preview_chat_route(None).await;
    if route.provider == "none" || route.model.trim().is_empty() {
        return Err("configured_provider_unavailable".into());
    }
    let endpoint_class = match route.route_type.as_str() {
        "local" => "local",
        "cloud" | "direct" => "cloud",
        other => other,
    }
    .to_string();
    let profile_id = stable_profile_id(
        &route.provider,
        &route.model,
        &endpoint_class,
        &runtime.config.llm.openai_base,
    );
    let binding = ProviderBinding {
        profile_id: profile_id.clone(),
        provider_id: route.provider.clone(),
        model_id: route.model.clone(),
        endpoint_class: endpoint_class.clone(),
        config_generation: runtime.scheduler.provider_config_generation().to_string(),
    };
    Ok(SelectedProviderProfile {
        profiles: vec![ProviderProfileViewModel {
            profile_id,
            provider_id: route.provider.clone(),
            model_id: route.model.clone(),
            endpoint_class,
            selected: true,
        }],
        binding,
        route,
    })
}

fn stable_profile_id(provider: &str, model: &str, class: &str, endpoint: &str) -> String {
    let material = format!("{provider}\0{model}\0{class}\0{endpoint}");
    let hex = digest(&SHA256, material.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("provider-profile:{}", &hex[..24])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_identity_binds_provider_model_class_and_endpoint() {
        let first = stable_profile_id("openai", "gpt", "cloud", "https://a.example/v1");
        assert_eq!(
            first,
            stable_profile_id("openai", "gpt", "cloud", "https://a.example/v1")
        );
        assert_ne!(
            first,
            stable_profile_id("openai", "gpt", "cloud", "https://b.example/v1")
        );
    }
}
