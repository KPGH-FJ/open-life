//! User-selected provider/model registry for Conversation and Task runtimes.
//!
//! The runtime deliberately has no automatic cross-provider routing. Settings own the
//! selection; this registry snapshots the exact executable profile and gives
//! the Turn an immutable binding before any provider request starts.

use crate::secret_store::{hydrate_bound_provider_secret, ProfileSecretStore, SecretStore};
use crate::state::AppState;
use openlife_core::conversation::{
    ProviderBinding, ProviderConnectionRecord, ProviderModelProfileRecord, ReasoningEffort,
};
use openlife_core::llm::{
    DiscoveredOpenRouterModel, DiscoveredProviderModelCapabilities, ProviderReasoningCapability,
    ReasoningCapabilitySource, ReasoningWireProtocol,
};
#[cfg(test)]
use openlife_core::task_runtime::CanonicalTaskStatus;
use ring::digest::{digest, SHA256};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

const PROVIDER_CAPABILITY_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone)]
struct CachedProviderCapabilities {
    observed_at: Instant,
    capabilities: Option<DiscoveredProviderModelCapabilities>,
}

fn provider_capability_cache(
) -> &'static tokio::sync::Mutex<HashMap<String, CachedProviderCapabilities>> {
    static CACHE: OnceLock<tokio::sync::Mutex<HashMap<String, CachedProviderCapabilities>>> =
        OnceLock::new();
    CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

#[derive(Clone)]
struct CachedOpenRouterCatalog {
    observed_at: Instant,
    models: Option<Vec<DiscoveredOpenRouterModel>>,
}

fn openrouter_catalog_cache(
) -> &'static tokio::sync::Mutex<HashMap<String, CachedOpenRouterCatalog>> {
    static CACHE: OnceLock<tokio::sync::Mutex<HashMap<String, CachedOpenRouterCatalog>>> =
        OnceLock::new();
    CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

async fn discover_openrouter_catalog(
    config: &openlife_core::config::AppConfig,
    config_generation: &str,
) -> Option<Vec<DiscoveredOpenRouterModel>> {
    let key = format!("{}\0{}", config_generation, config.llm.openai_base.trim());
    if let Some(cached) = openrouter_catalog_cache().lock().await.get(&key).cloned() {
        if cached.observed_at.elapsed() < PROVIDER_CAPABILITY_CACHE_TTL {
            return cached.models;
        }
    }
    let models = openlife_core::llm::discover_openrouter_model_catalog(
        &config.llm.openai_base,
        &config.effective_cloud_api_key(),
        &config.system.network_policy,
    )
    .await
    .ok();
    openrouter_catalog_cache().lock().await.insert(
        key,
        CachedOpenRouterCatalog {
            observed_at: Instant::now(),
            models: models.clone(),
        },
    );
    models
}

async fn discover_openrouter_capabilities(
    config: &openlife_core::config::AppConfig,
    config_generation: &str,
    model: &str,
) -> Option<DiscoveredProviderModelCapabilities> {
    let key = format!(
        "{}\0{}\0{}",
        config_generation,
        config.llm.openai_base.trim(),
        model
    );
    if let Some(cached) = provider_capability_cache().lock().await.get(&key).cloned() {
        if cached.observed_at.elapsed() < PROVIDER_CAPABILITY_CACHE_TTL {
            return cached.capabilities;
        }
    }
    let capabilities = discover_openrouter_catalog(config, config_generation)
        .await
        .and_then(|models| {
            models
                .into_iter()
                .find(|entry| entry.model_id == model)
                .map(|entry| entry.capabilities)
        });
    provider_capability_cache().lock().await.insert(
        key,
        CachedProviderCapabilities {
            observed_at: Instant::now(),
            capabilities: capabilities.clone(),
        },
    );
    capabilities
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileViewModel {
    pub profile_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
    pub endpoint_class: String,
    pub selected: bool,
    pub availability: String,
    pub unavailable_reason: Option<String>,
    pub size_bytes: Option<u64>,
    pub protocol: String,
    pub structured_output_contract: String,
    pub reasoning_control: String,
    pub supported_reasoning_efforts: Vec<ReasoningEffort>,
    pub default_reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_mandatory: bool,
    pub reasoning_capability_source: String,
    pub input_modalities: Vec<String>,
    pub input_capability_source: String,
    pub chat_compatibility: String,
    pub work_compatibility: String,
    pub work_compatibility_reason: Option<String>,
    pub work_compatibility_eval_version: Option<String>,
    pub work_compatibility_evaluated_at: Option<String>,
    pub tool_compatibility: String,
    pub tool_compatibility_reason: Option<String>,
}

#[derive(Clone)]
pub(crate) struct SelectedProviderProfile {
    pub binding: ProviderBinding,
    pub scheduler: openlife_core::scheduler::InferenceScheduler,
    pub reasoning_capability: Option<ProviderReasoningCapability>,
    pub input_modalities: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderProfileRegistry {
    pub profiles: Vec<ProviderProfileViewModel>,
    pub default_profile_id: Option<String>,
    pub default_error_code: Option<String>,
}

pub(crate) async fn provider_profile_registry(
    state: &Arc<AppState>,
) -> Result<ProviderProfileRegistry, String> {
    let runtime = state.provider_runtime_snapshot().await;
    if !runtime.coherent {
        return Err("provider_runtime_generation_incoherent".into());
    }
    let route = runtime.scheduler.resolve_selected_provider_route().await;
    // The product registry is owned by persisted Connections and Model Profiles.
    // In particular, it must not consult the retired process-global validation
    // file, even in a test build: doing so makes tests depend on ambient machine
    // state and can accidentally restore the old single-provider authority.
    let cloud_validation_load = crate::provider_validation::ProviderValidationLoad::Missing;
    let cloud_validation = crate::provider_validation::summarize_loaded_provider_validation(
        &runtime.config,
        &cloud_validation_load,
        chrono::Utc::now(),
    );
    let cloud_work_compatibility = crate::provider_validation::summarize_loaded_work_compatibility(
        &runtime.config,
        &cloud_validation_load,
        chrono::Utc::now(),
    );
    let (default_provider, default_model, default_class, route_ready, route_error) =
        if route.provider != "none" && !route.model.trim().is_empty() {
            let class = if route.route_type == "local" {
                "local"
            } else {
                "cloud"
            };
            (route.provider, route.model, class.to_string(), true, None)
        } else if runtime.config.prefer_local_model {
            (
                "ollama".into(),
                runtime.config.local_model.trim().to_string(),
                "local".into(),
                false,
                Some("provider_selected_local_route_unavailable".into()),
            )
        } else {
            (
                runtime.config.llm.provider.trim().to_ascii_lowercase(),
                runtime.config.llm.chat_model.trim().to_string(),
                "cloud".into(),
                false,
                Some("configured_provider_unavailable".into()),
            )
        };
    let controlled_test_profile_ready = cfg!(test)
        && runtime.config.llm.chat_model == "gpt-local-provider-harness"
        && runtime
            .config
            .llm
            .openai_base
            .starts_with("http://127.0.0.1:");
    let (default_availability, default_error) = if default_class == "local" {
        (
            if route_ready { "ready" } else { "offline" }.to_string(),
            route_error,
        )
    } else if cloud_validation.validated || controlled_test_profile_ready {
        ("ready".into(), None)
    } else {
        (
            cloud_availability(cloud_validation.status).into(),
            cloud_validation
                .last_error
                .clone()
                .or_else(|| Some(format!("provider_validation_{}", cloud_validation.status))),
        )
    };
    let expose_config_profile = exposes_legacy_config_profile(&default_class, cfg!(test));
    let mut profiles = Vec::new();
    let mut default_profile_id =
        if default_provider.is_empty() || default_model.is_empty() || !expose_config_profile {
            None
        } else {
            let id = stable_profile_id(
                &default_provider,
                &default_model,
                &default_class,
                profile_endpoint_material(&default_class, &runtime.config.llm.openai_base),
            );
            let (protocol, structured_output_contract) = adapter_contract(&default_class);
            let mut reasoning_capability =
                reasoning_capability(&default_provider, &default_model, &default_class);
            let mut input_modalities = vec!["text".to_string()];
            let mut input_capability_source = "adapter_default";
            if default_provider == "openrouter"
                && default_class == "cloud"
                && cloud_validation.validated
            {
                if let Some(discovered) = discover_openrouter_capabilities(
                    &runtime.config,
                    runtime.scheduler.provider_config_generation(),
                    &default_model,
                )
                .await
                {
                    if reasoning_capability.is_none() {
                        reasoning_capability = discovered.reasoning;
                    }
                    input_modalities = discovered.input_modalities;
                    input_capability_source = "provider_discovery";
                }
            }
            let chat_compatibility = if default_class == "local" {
                if route_ready {
                    "reachable_unverified"
                } else {
                    "unavailable"
                }
            } else if cloud_validation.validated || controlled_test_profile_ready {
                "validated"
            } else {
                "unverified"
            };
            let default_is_cloud = default_class == "cloud";
            profiles.push(ProviderProfileViewModel {
                profile_id: id.clone(),
                provider_id: default_provider,
                display_name: default_model.clone(),
                model_id: default_model,
                endpoint_class: default_class,
                selected: true,
                availability: default_availability,
                unavailable_reason: default_error.clone(),
                size_bytes: None,
                protocol: protocol.into(),
                structured_output_contract: structured_output_contract.into(),
                reasoning_control: if reasoning_capability.is_some() {
                    "effort_selector"
                } else {
                    "provider_default_only"
                }
                .into(),
                supported_reasoning_efforts: reasoning_capability
                    .as_ref()
                    .map(|capability| capability.supported_efforts.clone())
                    .unwrap_or_default(),
                default_reasoning_effort: reasoning_capability
                    .as_ref()
                    .and_then(|capability| capability.default_effort),
                reasoning_mandatory: reasoning_capability
                    .as_ref()
                    .is_some_and(|capability| capability.mandatory),
                reasoning_capability_source: reasoning_capability
                    .as_ref()
                    .map(|capability| reasoning_capability_source(capability.source))
                    .unwrap_or("unavailable")
                    .into(),
                input_modalities,
                input_capability_source: input_capability_source.into(),
                chat_compatibility: chat_compatibility.into(),
                work_compatibility: if default_is_cloud {
                    cloud_work_compatibility.status
                } else {
                    "unverified"
                }
                .into(),
                work_compatibility_reason: default_is_cloud
                    .then(|| cloud_work_compatibility.reason.clone())
                    .flatten(),
                work_compatibility_eval_version: default_is_cloud
                    .then(|| cloud_work_compatibility.eval_version.clone())
                    .flatten(),
                work_compatibility_evaluated_at: default_is_cloud
                    .then(|| cloud_work_compatibility.evaluated_at.clone())
                    .flatten(),
                tool_compatibility: "unverified".into(),
                tool_compatibility_reason: Some("tool_compatibility_eval_not_run".into()),
            });
            Some(id)
        };

    #[cfg(not(test))]
    let discovered_local_models = openlife_core::ollama::list_ollama_models().await;
    #[cfg(test)]
    let discovered_local_models: Vec<(String, u64)> = Vec::new();
    for (model, size_bytes) in discovered_local_models {
        let profile_id = stable_profile_id("ollama", &model, "local", "local://ollama");
        if let Some(existing) = profiles
            .iter_mut()
            .find(|profile| profile.profile_id == profile_id)
        {
            existing.availability = "ready".into();
            existing.unavailable_reason = None;
            existing.size_bytes = Some(size_bytes);
            continue;
        }
        let reasoning_capability = reasoning_capability("ollama", &model, "local");
        profiles.push(ProviderProfileViewModel {
            profile_id,
            provider_id: "ollama".into(),
            display_name: model.clone(),
            model_id: model,
            endpoint_class: "local".into(),
            selected: false,
            availability: "ready".into(),
            unavailable_reason: None,
            size_bytes: Some(size_bytes),
            protocol: "ollama_chat".into(),
            structured_output_contract: "json_schema_requested_locally_validated".into(),
            reasoning_control: if reasoning_capability.is_some() {
                "effort_selector"
            } else {
                "provider_default_only"
            }
            .into(),
            supported_reasoning_efforts: reasoning_capability
                .as_ref()
                .map(|capability| capability.supported_efforts.clone())
                .unwrap_or_default(),
            default_reasoning_effort: reasoning_capability
                .as_ref()
                .and_then(|capability| capability.default_effort),
            reasoning_mandatory: reasoning_capability
                .as_ref()
                .is_some_and(|capability| capability.mandatory),
            reasoning_capability_source: reasoning_capability
                .as_ref()
                .map(|capability| reasoning_capability_source(capability.source))
                .unwrap_or("unavailable")
                .into(),
            input_modalities: vec!["text".into()],
            input_capability_source: "adapter_default".into(),
            chat_compatibility: "reachable_unverified".into(),
            work_compatibility: "unverified".into(),
            work_compatibility_reason: None,
            work_compatibility_eval_version: None,
            work_compatibility_evaluated_at: None,
            tool_compatibility: "unverified".into(),
            tool_compatibility_reason: Some("tool_compatibility_eval_not_run".into()),
        });
    }
    if let Some(store) = state.conversation_store.as_ref() {
        let (connections, stored_profiles, selected_profile_id) = {
            let store = store.lock().await;
            (
                store.list_provider_connections().unwrap_or_default(),
                store.list_provider_model_profiles().unwrap_or_default(),
                store.selected_provider_profile_id(None).ok().flatten(),
            )
        };
        let connections = connections
            .into_iter()
            .map(|connection| (connection.id.clone(), connection))
            .collect::<HashMap<_, _>>();
        for stored in stored_profiles {
            let Some(connection) = connections.get(&stored.connection_id) else {
                continue;
            };
            if let Some(existing) = profiles
                .iter_mut()
                .find(|profile| profile.profile_id == stored.profile_id)
            {
                existing.selected =
                    selected_profile_id.as_deref() == Some(stored.profile_id.as_str());
                continue;
            }
            let credential_ready = if connection.endpoint_class == "local" {
                true
            } else {
                connection
                    .credential_reference
                    .as_deref()
                    .and_then(|reference| ProfileSecretStore.get(reference).ok().flatten())
                    .is_some_and(|encoded| {
                        hydrate_bound_provider_secret(
                            &connection.provider_id,
                            &connection.endpoint,
                            connection.credential_version,
                            &encoded,
                        )
                        .is_ok()
                    })
            };
            let ready = stored.validation_state == "ready" && credential_ready;
            let reasoning_capability = reasoning_capability(
                &connection.provider_id,
                &stored.model_id,
                &connection.endpoint_class,
            );
            let (protocol, structured_output_contract) =
                adapter_contract(&connection.endpoint_class);
            profiles.push(ProviderProfileViewModel {
                profile_id: stored.profile_id.clone(),
                provider_id: connection.provider_id.clone(),
                model_id: stored.model_id,
                display_name: stored.display_name,
                endpoint_class: connection.endpoint_class.clone(),
                selected: selected_profile_id.as_deref() == Some(stored.profile_id.as_str()),
                availability: if ready { "ready" } else { "unverified" }.into(),
                unavailable_reason: (!ready).then(|| {
                    if credential_ready {
                        format!("provider_validation_{}", stored.validation_state)
                    } else {
                        "provider_connection_credential_unavailable".into()
                    }
                }),
                size_bytes: None,
                protocol: protocol.into(),
                structured_output_contract: structured_output_contract.into(),
                reasoning_control: if reasoning_capability.is_some() {
                    "effort_selector"
                } else {
                    "provider_default_only"
                }
                .into(),
                supported_reasoning_efforts: reasoning_capability
                    .as_ref()
                    .map(|capability| capability.supported_efforts.clone())
                    .unwrap_or_default(),
                default_reasoning_effort: reasoning_capability
                    .as_ref()
                    .and_then(|capability| capability.default_effort),
                reasoning_mandatory: reasoning_capability
                    .as_ref()
                    .is_some_and(|capability| capability.mandatory),
                reasoning_capability_source: reasoning_capability
                    .as_ref()
                    .map(|capability| reasoning_capability_source(capability.source))
                    .unwrap_or("unavailable")
                    .into(),
                input_modalities: vec!["text".into()],
                input_capability_source: stored.capability_source,
                chat_compatibility: if ready { "validated" } else { "unverified" }.into(),
                work_compatibility: "unverified".into(),
                work_compatibility_reason: None,
                work_compatibility_eval_version: None,
                work_compatibility_evaluated_at: None,
                tool_compatibility: "unverified".into(),
                tool_compatibility_reason: Some("tool_compatibility_eval_not_run".into()),
            });
        }
        if selected_profile_id.as_deref().is_some_and(|selected| {
            profiles
                .iter()
                .any(|profile| profile.profile_id == selected)
        }) {
            default_profile_id = selected_profile_id;
        }
    }
    apply_observed_compatibility(state, &mut profiles).await;
    let default_error_code = default_profile_id.as_deref().and_then(|id| {
        profiles
            .iter()
            .find(|profile| profile.profile_id == id)
            .and_then(|profile| profile.unavailable_reason.clone())
    });
    Ok(ProviderProfileRegistry {
        profiles,
        default_profile_id,
        default_error_code,
    })
}

fn reasoning_capability(
    provider: &str,
    model: &str,
    endpoint_class: &str,
) -> Option<ProviderReasoningCapability> {
    let capability = openlife_core::llm::built_in_reasoning_capability(provider, model)?;
    let expected_class = if capability.provider_id == "ollama" {
        "local"
    } else {
        "cloud"
    };
    (endpoint_class == expected_class).then_some(capability)
}

fn reasoning_capability_source(source: ReasoningCapabilitySource) -> &'static str {
    match source {
        ReasoningCapabilitySource::OfficialBuiltin => "official_builtin",
        ReasoningCapabilitySource::ProviderDiscovery => "provider_discovery",
        ReasoningCapabilitySource::ExplicitConfiguration => "explicit_configuration",
    }
}

fn reasoning_capability_from_profile(
    profile: &ProviderProfileViewModel,
) -> Result<Option<ProviderReasoningCapability>, String> {
    if profile.supported_reasoning_efforts.is_empty() {
        return Ok(None);
    }
    let source = match profile.reasoning_capability_source.as_str() {
        "official_builtin" => ReasoningCapabilitySource::OfficialBuiltin,
        "provider_discovery" => ReasoningCapabilitySource::ProviderDiscovery,
        "explicit_configuration" => ReasoningCapabilitySource::ExplicitConfiguration,
        _ => return Err("provider_reasoning_capability_source_invalid".into()),
    };
    let wire_protocol = match profile.provider_id.as_str() {
        "openai" => ReasoningWireProtocol::OpenAiReasoningEffort,
        "gemini" => ReasoningWireProtocol::GeminiReasoningEffort,
        "deepseek" => ReasoningWireProtocol::DeepSeekThinking,
        "ollama" => ReasoningWireProtocol::OllamaThink,
        "openrouter" => ReasoningWireProtocol::OpenRouterUnified,
        _ => return Err("provider_reasoning_wire_protocol_unavailable".into()),
    };
    let capability = ProviderReasoningCapability {
        provider_id: profile.provider_id.clone(),
        model_id: profile.model_id.clone(),
        wire_protocol,
        supported_efforts: profile.supported_reasoning_efforts.clone(),
        default_effort: profile.default_reasoning_effort,
        mandatory: profile.reasoning_mandatory,
        source,
    };
    capability
        .validate_for_target(&profile.provider_id, &profile.model_id)
        .map_err(|_| "provider_reasoning_capability_invalid".to_string())?;
    Ok(Some(capability))
}

fn adapter_contract(endpoint_class: &str) -> (&'static str, &'static str) {
    if endpoint_class == "local" {
        return ("ollama_chat", "json_schema_requested_locally_validated");
    }
    (
        "openai_compatible_chat_completions",
        "json_object_requested_locally_validated",
    )
}

async fn apply_observed_compatibility(
    state: &Arc<AppState>,
    profiles: &mut [ProviderProfileViewModel],
) {
    let Some(task_store) = state.canonical_task_runtime_store.as_ref() else {
        return;
    };
    let Ok(snapshots) = task_store.lock().await.list_task_snapshots(100) else {
        return;
    };
    let Some(conversation_store) = state.conversation_store.as_ref() else {
        return;
    };
    let work_turn_ids = snapshots
        .iter()
        .filter(|snapshot| snapshot.task.task_kind == "work")
        .flat_map(|snapshot| snapshot.runs.iter().rev())
        .map(|run| run.execution_session_id.clone())
        .collect::<HashSet<_>>();
    let recent_turns = {
        let store = conversation_store.lock().await;
        match store.list_recent_turns(500) {
            Ok(turns) => turns,
            Err(_) => return,
        }
    };
    let turns_by_id = recent_turns
        .iter()
        .map(|turn| (turn.id.clone(), turn))
        .collect::<HashMap<_, _>>();
    let observed_chat = recent_turns
        .iter()
        .filter(|turn| !work_turn_ids.contains(&turn.id))
        .filter(|turn| turn.status == openlife_core::conversation::TurnStatus::Completed)
        .map(|turn| turn.provider.profile_id.clone())
        .collect::<HashSet<_>>();
    let mut observed = HashMap::<String, (String, Option<String>)>::new();
    for snapshot in snapshots {
        if snapshot.task.task_kind != "work" {
            continue;
        }
        for run in snapshot.runs.iter().rev() {
            let Some(turn) = turns_by_id.get(&run.execution_session_id) else {
                continue;
            };
            if observed.contains_key(&turn.provider.profile_id) {
                continue;
            }
            // Ordinary user Work is not a compatibility evaluation. A newer
            // successful run does prove that an older observed failure is no
            // longer the latest runtime observation, but it must only return
            // the profile to `unverified`; a dedicated versioned eval remains
            // the sole authority for `validated`.
            if turn.status == openlife_core::conversation::TurnStatus::Completed {
                observed.insert(
                    turn.provider.profile_id.clone(),
                    ("unverified".into(), None),
                );
                continue;
            }
            // Contract failures remain useful negative observations because
            // their exact error is already bounded.
            if let Some(error) = ordinary_work_contract_failure(turn.error_code.as_deref()) {
                observed.insert(
                    turn.provider.profile_id.clone(),
                    ("observed_contract_failure".into(), Some(error.to_string())),
                );
            }
        }
    }
    for profile in profiles {
        if observed_chat.contains(&profile.profile_id) {
            profile.chat_compatibility = "validated".into();
        }
        if let Some((status, reason)) = observed.remove(&profile.profile_id) {
            if status == "unverified" && profile.work_compatibility != "unverified" {
                continue;
            }
            profile.work_compatibility = status;
            profile.work_compatibility_reason = reason;
        }
    }
}

fn ordinary_work_contract_failure(error: Option<&str>) -> Option<&str> {
    error.filter(|error| is_model_work_contract_failure(error))
}

fn is_model_work_contract_failure(error: &str) -> bool {
    [
        "agent_step_",
        "initial_work_decision_",
        "observation_bound_agent_step_",
        "work_plan_",
        "work_semantic_verification_",
    ]
    .iter()
    .any(|prefix| error.starts_with(prefix))
        || error == "provider_reasoning_without_final_content"
}

fn cloud_availability(validation_status: &str) -> &'static str {
    match validation_status {
        "validated" => "ready",
        "unconfigured" => "unconfigured",
        "unvalidated" => "unverified",
        "stale" => "stale",
        "failed" => "offline",
        "remote_unknown"
        | "unknown"
        | "validation_record_corrupt"
        | "validation_record_io_error" => "degraded",
        _ => "degraded",
    }
}

fn profile_endpoint_material<'a>(class: &str, configured_endpoint: &'a str) -> &'a str {
    if class == "local" {
        "local://ollama"
    } else {
        configured_endpoint
    }
}

fn exposes_legacy_config_profile(endpoint_class: &str, controlled_test: bool) -> bool {
    endpoint_class == "local" || controlled_test
}

pub(crate) async fn resolve_provider_profile(
    requested_profile_id: Option<&str>,
    requested_reasoning_effort: Option<ReasoningEffort>,
    state: &Arc<AppState>,
) -> Result<SelectedProviderProfile, String> {
    let registry = provider_profile_registry(state).await?;
    let selected_id = requested_profile_id
        .map(str::to_string)
        .or_else(|| registry.default_profile_id.clone())
        .ok_or_else(|| "configured_provider_unavailable".to_string())?;
    let profile = registry
        .profiles
        .iter()
        .find(|profile| profile.profile_id == selected_id)
        .ok_or_else(|| "provider_profile_not_found".to_string())?;
    if profile.availability != "ready" {
        return Err(profile
            .unavailable_reason
            .clone()
            .unwrap_or_else(|| "provider_profile_unavailable".into()));
    }
    let reasoning_effort = match requested_reasoning_effort {
        Some(effort) if profile.supported_reasoning_efforts.contains(&effort) => Some(effort),
        Some(_) => return Err("provider_reasoning_effort_unsupported".into()),
        None => None,
    };
    let runtime = state.provider_runtime_snapshot().await;
    if !runtime.coherent {
        return Err("provider_runtime_generation_incoherent".into());
    }
    let persisted_connection = if let Some(store) = state.conversation_store.as_ref() {
        let store = store.lock().await;
        let stored_profile = store
            .list_provider_model_profiles()
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|stored| stored.profile_id == profile.profile_id);
        match stored_profile {
            Some(stored) => store
                .get_provider_connection(&stored.connection_id)
                .map_err(|error| error.to_string())?,
            None => None,
        }
    } else {
        None
    };
    let scheduler = if let Some(connection) = persisted_connection {
        if connection.endpoint_class == "local" {
            runtime
                .scheduler
                .with_selected_local_model(profile.model_id.clone())
        } else {
            let reference = connection
                .credential_reference
                .as_deref()
                .ok_or_else(|| "provider_connection_credential_missing".to_string())?;
            let encoded = ProfileSecretStore
                .get(reference)
                .map_err(|_| "provider_connection_credential_unavailable".to_string())?
                .ok_or_else(|| "provider_connection_credential_missing".to_string())?;
            let mut config = runtime.config.clone();
            config.prefer_local_model = false;
            config.llm.provider = connection.provider_id;
            config.llm.openai_base = connection.endpoint;
            config.llm.chat_model = profile.model_id.clone();
            config.llm.openai_key_ref = connection.credential_reference;
            config.llm.credential_version = connection.credential_version;
            config.llm.openai_key = hydrate_bound_provider_secret(
                &config.llm.provider,
                &config.llm.openai_base,
                config.llm.credential_version,
                &encoded,
            )
            .map_err(|_| "provider_connection_credential_invalid".to_string())?;
            openlife_core::scheduler::InferenceScheduler::new(
                config.local_model,
                false,
                config.llm.provider,
                config.llm.openai_base,
                config.llm.openai_key,
                config.llm.chat_model,
                config.llm.embedding_model,
                config.llm.embedding_enabled,
            )
            .with_provider_credential_version(config.llm.credential_version)
        }
    } else if profile.endpoint_class == "local" {
        runtime
            .scheduler
            .with_selected_local_model(profile.model_id.clone())
    } else if cfg!(test) {
        runtime
            .scheduler
            .with_selected_cloud_model(profile.model_id.clone())
    } else {
        return Err("provider_profile_connection_missing".into());
    };
    let selected_reasoning_capability = reasoning_capability_from_profile(profile)?;
    Ok(SelectedProviderProfile {
        binding: ProviderBinding {
            profile_id: profile.profile_id.clone(),
            provider_id: profile.provider_id.clone(),
            model_id: profile.model_id.clone(),
            endpoint_class: profile.endpoint_class.clone(),
            config_generation: scheduler.provider_config_generation().to_string(),
            reasoning_effort,
        },
        scheduler,
        reasoning_capability: selected_reasoning_capability,
        input_modalities: profile.input_modalities.clone(),
    })
}

#[cfg(test)]
pub(crate) async fn selected_provider_profile(
    state: &Arc<AppState>,
) -> Result<SelectedProviderProfile, String> {
    resolve_provider_profile(None, None, state).await
}

pub(crate) fn stable_profile_id(
    provider: &str,
    model: &str,
    class: &str,
    endpoint: &str,
) -> String {
    let material = format!("{provider}\0{model}\0{class}\0{endpoint}");
    let hex = digest(&SHA256, material.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("provider-profile:{}", &hex[..24])
}

pub(crate) fn stable_connection_id(
    provider: &str,
    class: &str,
    endpoint: &str,
    credential_reference: Option<&str>,
) -> String {
    let material = format!(
        "{provider}\0{class}\0{endpoint}\0{}",
        credential_reference.unwrap_or("")
    );
    let hex = digest(&SHA256, material.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("provider-connection:{}", &hex[..24])
}

pub(crate) async fn persist_provider_profile_selection(
    conversation_id: Option<&str>,
    profile_id: &str,
    state: &Arc<AppState>,
) -> Result<(), String> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| error.to_string())?;
    let registry = provider_profile_registry(state).await?;
    let profile = registry
        .profiles
        .iter()
        .find(|profile| profile.profile_id == profile_id)
        .ok_or_else(|| "provider_profile_not_found".to_string())?;
    if profile.availability != "ready" {
        return Err(profile
            .unavailable_reason
            .clone()
            .unwrap_or_else(|| "provider_profile_unavailable".into()));
    }
    let store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| "conversation_store_unavailable".to_string())?;
    {
        let store = store.lock().await;
        let already_persisted = store
            .list_provider_model_profiles()
            .map_err(|error| error.to_string())?
            .iter()
            .any(|stored| stored.profile_id == profile_id);
        if already_persisted {
            return store
                .set_selected_provider_profile(conversation_id, profile_id)
                .map_err(|error| error.to_string());
        }
    }
    let runtime = state.provider_runtime_snapshot().await;
    if !runtime.coherent {
        return Err("provider_runtime_generation_incoherent".into());
    }
    let endpoint =
        profile_endpoint_material(&profile.endpoint_class, &runtime.config.llm.openai_base);
    let credential_reference = (profile.endpoint_class == "cloud")
        .then(|| runtime.config.llm.openai_key_ref.clone())
        .flatten();
    let connection_id = stable_connection_id(
        &profile.provider_id,
        &profile.endpoint_class,
        endpoint,
        credential_reference.as_deref(),
    );
    let now = chrono::Utc::now();
    let connection = ProviderConnectionRecord {
        id: connection_id.clone(),
        provider_id: profile.provider_id.clone(),
        display_name: profile.provider_id.clone(),
        endpoint: endpoint.to_string(),
        endpoint_class: profile.endpoint_class.clone(),
        credential_reference,
        credential_version: if profile.endpoint_class == "cloud" {
            runtime.config.llm.credential_version
        } else {
            0
        },
        protocol: profile.protocol.clone(),
        privacy_boundary: if profile.endpoint_class == "local" {
            "local_only"
        } else {
            "provider_hosted"
        }
        .into(),
        validation_state: profile.availability.clone(),
        created_at: now,
        updated_at: now,
    };
    let capability_snapshot_json = serde_json::json!({
        "inputModalities": profile.input_modalities,
        "inputCapabilitySource": profile.input_capability_source,
        "structuredOutputContract": profile.structured_output_contract,
        "reasoningControl": profile.reasoning_control,
        "supportedReasoningEfforts": profile.supported_reasoning_efforts,
        "reasoningMandatory": profile.reasoning_mandatory,
        "reasoningCapabilitySource": profile.reasoning_capability_source,
        "toolCompatibility": profile.tool_compatibility,
    })
    .to_string();
    let model_profile = ProviderModelProfileRecord {
        profile_id: profile.profile_id.clone(),
        connection_id,
        model_id: profile.model_id.clone(),
        display_name: profile.display_name.clone(),
        capability_snapshot_json,
        capability_source: profile.input_capability_source.clone(),
        validation_state: profile.availability.clone(),
        created_at: now,
        updated_at: now,
    };
    let store = store.lock().await;
    store
        .upsert_provider_model_profile(&connection, &model_profile)
        .map_err(|error| error.to_string())?;
    store
        .set_selected_provider_profile(conversation_id, profile_id)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::conversation::BeginChatTurn;
    use openlife_core::task_runtime::BeginGeneralTaskRunInput;

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

    #[test]
    fn cloud_validation_state_is_never_collapsed_into_ready() {
        assert_eq!(cloud_availability("validated"), "ready");
        assert_eq!(cloud_availability("unvalidated"), "unverified");
        assert_eq!(cloud_availability("stale"), "stale");
        assert_eq!(cloud_availability("failed"), "offline");
        assert_eq!(cloud_availability("remote_unknown"), "degraded");
        assert_eq!(cloud_availability("validation_record_corrupt"), "degraded");
    }

    #[test]
    fn product_registry_never_exposes_cloud_config_as_a_profile_owner() {
        assert!(!exposes_legacy_config_profile("cloud", false));
        assert!(exposes_legacy_config_profile("local", false));
        assert!(exposes_legacy_config_profile("cloud", true));
    }

    #[test]
    fn registry_exposes_reasoning_only_for_exact_supported_profile() {
        let supported = reasoning_capability("openai", "gpt-5.6-terra", "cloud").unwrap();
        assert_eq!(supported.default_effort, Some(ReasoningEffort::Medium));
        assert!(supported.supported_efforts.contains(&ReasoningEffort::Max));

        assert!(reasoning_capability("openrouter", "gpt-5.6-terra", "cloud").is_none());
        assert!(reasoning_capability("openai", "custom-gpt-5.6", "cloud").is_none());
        assert!(reasoning_capability("ollama", "gpt-oss:20b", "local").is_some());
    }

    #[test]
    fn work_compatibility_failure_is_limited_to_model_contract_errors() {
        assert!(is_model_work_contract_failure(
            "agent_step_artifact_content_type_invalid"
        ));
        assert!(is_model_work_contract_failure("work_plan_schema_invalid"));
        assert!(!is_model_work_contract_failure("permission_required"));
        assert!(!is_model_work_contract_failure(
            "project_workspace_root_unavailable"
        ));
        assert!(!is_model_work_contract_failure(
            "provider_remote_state_unknown"
        ));
    }

    #[test]
    fn ordinary_work_success_never_claims_model_compatibility_validation() {
        assert_eq!(ordinary_work_contract_failure(None), None);
        assert_eq!(
            ordinary_work_contract_failure(Some("permission_required")),
            None
        );
        assert_eq!(
            ordinary_work_contract_failure(Some("work_plan_schema_invalid")),
            Some("work_plan_schema_invalid")
        );
    }

    #[tokio::test]
    async fn ready_profile_selection_persists_connection_model_and_conversation_choice() {
        let state = crate::test_utils::test_app_state();
        crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_local_http_provider(
            &state,
            "unused",
        )
        .await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Persistent model")
            .unwrap();
        let registry = provider_profile_registry(&state).await.unwrap();
        let profile_id = registry.default_profile_id.unwrap();

        persist_provider_profile_selection(Some(&conversation_id), &profile_id, &state)
            .await
            .unwrap();

        let store = state.conversation_store.as_ref().unwrap().lock().await;
        assert_eq!(store.list_provider_connections().unwrap().len(), 1);
        let profiles = store.list_provider_model_profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].profile_id, profile_id);
        assert_eq!(
            store
                .selected_provider_profile_id(Some(&conversation_id))
                .unwrap()
                .as_deref(),
            Some(profile_id.as_str())
        );
        drop(store);
        let view = crate::commands::chat::get_conversation_view_model_with_state(
            Some(&conversation_id),
            &state,
        )
        .await
        .unwrap();
        assert_eq!(
            view.selected_provider_profile_id.as_deref(),
            Some(profile_id.as_str())
        );
    }

    #[tokio::test]
    async fn registry_projects_exact_observed_work_contract_failure() {
        let state = crate::test_utils::test_app_state();
        crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_local_http_provider(
            &state,
            "unused",
        )
        .await;
        let provider = selected_provider_profile(&state).await.unwrap().binding;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let turn_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Work compatibility")
            .unwrap();
        let begun = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .begin_chat_turn_with_proof(BeginChatTurn {
                turn_id: &turn_id,
                conversation_id: &conversation_id,
                user_message: "Run one structured Work step.",
                provider: &provider,
            })
            .unwrap();
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: &turn_id,
                instruction_digest: begun.user_message_proof.content_digest(),
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
                execution_mode: openlife_core::task_runtime::WorkExecutionMode::ScopedAgent,
            })
            .unwrap();
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .terminalize_general_run(&task_id, &run_id, CanonicalTaskStatus::Blocked)
            .unwrap();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .fail_chat_turn(&turn_id, "agent_step_artifact_content_type_invalid")
            .unwrap();
        let chat_conversation_id = uuid::Uuid::new_v4().to_string();
        let chat_turn_id = uuid::Uuid::new_v4().to_string();
        let conversation_store = state.conversation_store.as_ref().unwrap().lock().await;
        conversation_store
            .create_conversation(&chat_conversation_id, "Chat compatibility")
            .unwrap();
        conversation_store
            .begin_chat_turn_with_proof(BeginChatTurn {
                turn_id: &chat_turn_id,
                conversation_id: &chat_conversation_id,
                user_message: "Reply once.",
                provider: &provider,
            })
            .unwrap();
        conversation_store
            .complete_chat_turn(&chat_turn_id, "Done.")
            .unwrap();
        drop(conversation_store);

        let registry = provider_profile_registry(&state).await.unwrap();
        let observed = registry
            .profiles
            .iter()
            .find(|profile| profile.profile_id == provider.profile_id)
            .unwrap();
        assert_eq!(observed.work_compatibility, "observed_contract_failure");
        assert_eq!(
            observed.work_compatibility_reason.as_deref(),
            Some("agent_step_artifact_content_type_invalid")
        );
        assert_eq!(observed.chat_compatibility, "validated");

        let recovery_turn_id = uuid::Uuid::new_v4().to_string();
        let recovery_run_id = uuid::Uuid::new_v4().to_string();
        let recovery_begun = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .begin_chat_turn_with_proof(BeginChatTurn {
                turn_id: &recovery_turn_id,
                conversation_id: &conversation_id,
                user_message: "Run one structured Work step.",
                provider: &provider,
            })
            .unwrap();
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &recovery_run_id,
                execution_session_id: &recovery_turn_id,
                instruction_digest: recovery_begun.user_message_proof.content_digest(),
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
                execution_mode: openlife_core::task_runtime::WorkExecutionMode::ScopedAgent,
            })
            .unwrap();
        let completed_turn = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .complete_chat_turn(&recovery_turn_id, "Recovered result.")
            .unwrap();
        let assistant_item = completed_turn
            .items
            .iter()
            .find(|item| {
                item.kind == openlife_core::conversation::ConversationItemKind::AssistantMessage
            })
            .unwrap();
        let final_item_id =
            openlife_core::task_runtime::final_result_item_id(&task_id, &recovery_run_id);
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .complete_general_task(openlife_core::task_runtime::CompleteGeneralTaskInput {
                task_id: &task_id,
                run_id: &recovery_run_id,
                final_item_id: &final_item_id,
                conversation_item_id: &assistant_item.id,
                result_digest: &assistant_item.content_digest,
                summary_code: "work_completed",
                completion_limitations: &[],
            })
            .unwrap();

        let recovered_registry = provider_profile_registry(&state).await.unwrap();
        let recovered = recovered_registry
            .profiles
            .iter()
            .find(|profile| profile.profile_id == provider.profile_id)
            .unwrap();
        assert_eq!(recovered.work_compatibility, "unverified");
        assert_eq!(recovered.work_compatibility_reason, None);
    }
}
