use openlife_core::config::AppConfig;
use openlife_core::scheduler::InferenceScheduler;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeRouteEvidence {
    pub evidence_id: String,
    pub generated_at: String,
    pub conversation_id: Option<String>,
    pub run_id: Option<String>,
    pub task_id: Option<String>,
    pub answer_scope: String,
    pub planned_route: Option<RouteIdentity>,
    pub actual_route: Option<RouteIdentity>,
    pub last_completed_route: Option<RouteIdentity>,
    pub provider_readiness: ProviderReadiness,
    pub fallback: Option<FallbackEvidence>,
    pub external_transmission: String,
    pub source_refs: Vec<Value>,
    pub truth_confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteIdentity {
    pub provider: String,
    pub model: String,
    pub route_type: String,
    pub privacy_level: String,
    pub reason: String,
    pub provider_health_is_estimated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderReadiness {
    pub configured: bool,
    pub credential_present: bool,
    pub validated: bool,
    pub validation_status: String,
    pub preferred: String,
    pub actually_used: Option<String>,
    pub stale: bool,
    pub failed: bool,
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FallbackEvidence {
    pub from_route: Option<RouteIdentity>,
    pub to_route: Option<RouteIdentity>,
    pub reason: String,
    pub blocker_codes: Vec<String>,
}

pub(crate) async fn build_settings_runtime_route_evidence(
    _state: &std::sync::Arc<crate::AppState>,
    config: &AppConfig,
    scheduler: &InferenceScheduler,
) -> RuntimeRouteEvidence {
    let generated_at = chrono::Utc::now().to_rfc3339();
    let validation_load = crate::provider_validation::load_provider_validation_record_from_path(
        &crate::provider_validation::provider_validation_path(),
    );
    let validation = crate::provider_validation::summarize_loaded_provider_validation(
        config,
        &validation_load,
        chrono::Utc::now(),
    );
    let provider = scheduler.provider.trim().to_ascii_lowercase();
    let model = scheduler.chat_model.trim().to_string();
    let configured = !provider.is_empty() && provider != "none" && !model.is_empty();
    let local = matches!(provider.as_str(), "ollama" | "local");
    let credential_present = local || !scheduler.effective_api_key().trim().is_empty();
    let network_ready = local || config.system.network_policy.enabled;
    let planned_route = configured.then(|| RouteIdentity {
        provider: provider.clone(),
        model: model.clone(),
        route_type: if local { "local" } else { "cloud" }.into(),
        privacy_level: if local {
            "local_only"
        } else {
            "provider_bound"
        }
        .into(),
        reason: "configured_model_selection".into(),
        provider_health_is_estimated: true,
    });
    let failed = configured && (!credential_present || !network_ready);
    let preferred = if config.prefer_local_model {
        "local"
    } else {
        "configured"
    };
    let provider_readiness = ProviderReadiness {
        configured,
        credential_present,
        validated: validation.validated,
        validation_status: validation.status.into(),
        preferred: preferred.into(),
        actually_used: None,
        stale: matches!(validation.status, "stale" | "expired"),
        failed,
        last_checked_at: validation.validated_at.or(validation.failed_at),
    };
    let source_refs = vec![
        json!({
            "source": "provider_configuration",
            "provider": provider,
            "model": model,
            "networkEnabled": config.system.network_policy.enabled,
        }),
        json!({
            "source": "provider_validation",
            "status": provider_readiness.validation_status,
            "credentialPresent": provider_readiness.credential_present,
        }),
    ];

    RuntimeRouteEvidence {
        evidence_id: format!("provider-settings:{}", uuid::Uuid::new_v4()),
        generated_at,
        conversation_id: None,
        run_id: None,
        task_id: None,
        answer_scope: "settings_readiness".into(),
        planned_route,
        actual_route: None,
        last_completed_route: None,
        provider_readiness,
        fallback: None,
        external_transmission: "unknown".into(),
        source_refs,
        truth_confidence: if configured { "inferred" } else { "unknown" }.into(),
    }
}
