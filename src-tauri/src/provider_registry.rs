//! User-selected provider/model registry for Conversation and Task runtimes.
//!
//! The runtime deliberately has no automatic cross-provider routing. Settings own the
//! selection; this registry snapshots the exact executable profile and gives
//! the Turn an immutable binding before any provider request starts.

use crate::state::AppState;
use openlife_core::conversation::{ProviderBinding, ReasoningEffort};
use openlife_core::llm::{
    ProviderReasoningCapability, ReasoningCapabilitySource, ReasoningWireProtocol,
};
use openlife_core::task_runtime::CanonicalTaskStatus;
use ring::digest::{digest, SHA256};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

const PROVIDER_CAPABILITY_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone)]
struct CachedReasoningCapability {
    observed_at: Instant,
    capability: Option<ProviderReasoningCapability>,
}

fn provider_capability_cache(
) -> &'static tokio::sync::Mutex<HashMap<String, CachedReasoningCapability>> {
    static CACHE: OnceLock<tokio::sync::Mutex<HashMap<String, CachedReasoningCapability>>> =
        OnceLock::new();
    CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

async fn discover_openrouter_reasoning_capability(
    config: &openlife_core::config::AppConfig,
    config_generation: &str,
    model: &str,
) -> Option<ProviderReasoningCapability> {
    let key = format!(
        "{}\0{}\0{}",
        config_generation,
        config.llm.openai_base.trim(),
        model
    );
    if let Some(cached) = provider_capability_cache().lock().await.get(&key).cloned() {
        if cached.observed_at.elapsed() < PROVIDER_CAPABILITY_CACHE_TTL {
            return cached.capability;
        }
    }
    let capability = openlife_core::llm::discover_openrouter_reasoning_capability(
        &config.llm.openai_base,
        &config.effective_cloud_api_key(),
        model,
        &config.system.network_policy,
    )
    .await
    .ok()
    .flatten();
    provider_capability_cache().lock().await.insert(
        key,
        CachedReasoningCapability {
            observed_at: Instant::now(),
            capability: capability.clone(),
        },
    );
    capability
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileViewModel {
    pub profile_id: String,
    pub provider_id: String,
    pub model_id: String,
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
    pub chat_compatibility: String,
    pub work_compatibility: String,
    pub work_compatibility_reason: Option<String>,
}

#[derive(Clone)]
pub(crate) struct SelectedProviderProfile {
    pub binding: ProviderBinding,
    pub scheduler: openlife_core::scheduler::InferenceScheduler,
    pub reasoning_capability: Option<ProviderReasoningCapability>,
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
    let cloud_validation_load =
        crate::provider_validation::load_provider_validation_record_from_path(
            &crate::provider_validation::provider_validation_path(),
        );
    let cloud_validation = crate::provider_validation::summarize_loaded_provider_validation(
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
    let mut profiles = Vec::new();
    let default_profile_id = if default_provider.is_empty() || default_model.is_empty() {
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
        if reasoning_capability.is_none()
            && default_provider == "openrouter"
            && default_class == "cloud"
            && cloud_validation.validated
        {
            reasoning_capability = discover_openrouter_reasoning_capability(
                &runtime.config,
                runtime.scheduler.provider_config_generation(),
                &default_model,
            )
            .await;
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
        profiles.push(ProviderProfileViewModel {
            profile_id: id.clone(),
            provider_id: default_provider,
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
            chat_compatibility: chat_compatibility.into(),
            work_compatibility: "unverified".into(),
            work_compatibility_reason: None,
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
            chat_compatibility: "reachable_unverified".into(),
            work_compatibility: "unverified".into(),
            work_compatibility_reason: None,
        });
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
            if snapshot.task.status == CanonicalTaskStatus::Completed
                && snapshot.final_result.is_some()
                && turn.status == openlife_core::conversation::TurnStatus::Completed
            {
                observed.insert(turn.provider.profile_id.clone(), ("validated".into(), None));
            } else if let Some(error) = turn
                .error_code
                .as_deref()
                .filter(|error| is_model_work_contract_failure(error))
            {
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
            profile.work_compatibility = status;
            profile.work_compatibility_reason = reason;
        }
    }
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
    let scheduler = if profile.endpoint_class == "local" {
        runtime
            .scheduler
            .with_selected_local_model(profile.model_id.clone())
    } else {
        runtime.scheduler
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
    })
}

pub(crate) async fn selected_provider_profile(
    state: &Arc<AppState>,
) -> Result<SelectedProviderProfile, String> {
    resolve_provider_profile(None, None, state).await
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
    }
}
