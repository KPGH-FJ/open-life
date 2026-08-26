//! Owned ToolGateway resource snapshots for Tauri product execution paths.
//!
//! `AppState` mutexes protect replaceable handles; they must never become part
//! of provider, tool, or network I/O lifetimes. This module is the sole lock
//! acquisition authority for product `ActionExecutionContext` construction.
//! Each public function returns a purpose-specific type whose required stores
//! are non-optional, and every outer guard is dropped before the next lock.
//!
//! Product source map and lock-order contract:
//! 1. `config` (copy bounded typed execution fields; credentials remain only
//!    in non-serializable runtime objects),
//! 2. `tool_permission_store`,
//! 3. `mcp_registry`,
//! 4. `mcp_audit_store`,
//! 5. `privacy_engine`,
//! 6. required proposal or canonical Work runtime only where the
//!    purpose-specific return type declares them.
//!
//! The consumers are the dev ToolGateway command, Main Chat read execution,
//! canonical Work item execution and scheduled execution. MCP's
//! `McpSession` mutex is intentionally outside this table: it
//! is a single-owner transport/session lock on an owned registry snapshot, not
//! an `AppState` registry guard, and cancellation poisons/kills that transport.

use std::sync::Arc;

use crate::AppState;

#[derive(Clone)]
pub(crate) struct SharedToolGatewayResources {
    pub(crate) additional_read_roots: Vec<String>,
    pub(crate) permission_store: openlife_core::tool_permissions::ToolPermissionStore,
    pub(crate) registry: openlife_core::mcp::McpRegistry,
    pub(crate) audit_store: openlife_core::mcp_audit::McpAuditStore,
    pub(crate) privacy_engine: openlife_core::privacy::PrivacyEngine,
    pub(crate) persistence_coordinator: Arc<crate::persistence_coordinator::PersistenceCoordinator>,
}

#[derive(Clone)]
pub(crate) struct GovernedToolGatewayResources {
    pub(crate) shared: SharedToolGatewayResources,
    pub(crate) network_policy: openlife_core::config::NetworkPolicy,
    pub(crate) search_provider:
        openlife_core::agent::action_executor::helpers::SearchProviderConfig,
}

pub(crate) struct MainChatReadToolGatewayResources {
    pub(crate) governed: GovernedToolGatewayResources,
}

struct CapturedToolRuntimeConfig {
    additional_read_roots: Vec<String>,
    network_policy: openlife_core::config::NetworkPolicy,
    search_provider: openlife_core::agent::action_executor::helpers::SearchProviderConfig,
}

fn selected_route_hosted_search_provider(
    configured_search_provider: &str,
    selected_provider: &str,
    selected_endpoint: &str,
) -> Option<&'static str> {
    let selected_provider = selected_provider.trim().to_ascii_lowercase();
    let configured = configured_search_provider.trim().to_ascii_lowercase();
    if configured != "auto" && configured != selected_provider {
        return None;
    }
    let url = reqwest::Url::parse(selected_endpoint.trim()).ok()?;
    let normalized_path = url.path().trim_end_matches('/');
    let official_origin = url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none();
    match selected_provider.as_str() {
        "deepseek"
            if official_origin
                && url.host_str() == Some("api.deepseek.com")
                && matches!(normalized_path, "" | "/v1") =>
        {
            Some("deepseek")
        }
        "openrouter"
            if official_origin
                && url.host_str() == Some("openrouter.ai")
                && normalized_path == "/api/v1" =>
        {
            Some("openrouter")
        }
        _ => None,
    }
}

fn bind_search_to_selected_provider_route(
    search_provider: &mut openlife_core::agent::action_executor::helpers::SearchProviderConfig,
    configured_search_provider: &str,
    selected_provider: &str,
    selected_endpoint: &str,
    selected_api_key: String,
    selected_model: &str,
) -> bool {
    let Some(provider) = selected_route_hosted_search_provider(
        configured_search_provider,
        selected_provider,
        selected_endpoint,
    ) else {
        return false;
    };
    search_provider.provider = provider.into();
    search_provider.api_key = selected_api_key;
    search_provider.model = selected_model.to_string();
    true
}

async fn capture_tool_runtime_config(
    state: &Arc<AppState>,
    provider_profile_id: Option<&str>,
) -> CapturedToolRuntimeConfig {
    let config = state.config.lock().await;
    let mut search_provider =
        openlife_core::agent::action_executor::helpers::SearchProviderConfig::from_system_config(
            &config.system,
        );
    let configured_search_provider = config.system.search_provider.clone();
    let additional_read_roots = config.system.additional_read_roots.clone();
    let network_policy = config.system.network_policy.clone();
    drop(config);
    if search_provider.api_key.trim().is_empty()
        && matches!(
            configured_search_provider
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "auto" | "deepseek" | "openrouter"
        )
    {
        if let Ok(selected) =
            crate::provider_registry::resolve_provider_profile(provider_profile_id, None, state)
                .await
        {
            if !bind_search_to_selected_provider_route(
                &mut search_provider,
                &configured_search_provider,
                &selected.scheduler.provider,
                &selected.scheduler.openai_base,
                selected.scheduler.effective_api_key(),
                &selected.scheduler.chat_model,
            ) {
                search_provider.provider = "unavailable".into();
                search_provider.model.clear();
            }
        } else {
            search_provider.provider = "unavailable".into();
            search_provider.model.clear();
        }
    } else if configured_search_provider.eq_ignore_ascii_case("auto") {
        search_provider.provider = "unavailable".into();
    } else {
        search_provider.provider = configured_search_provider;
    }
    CapturedToolRuntimeConfig {
        additional_read_roots,
        network_policy,
        search_provider,
    }
}

async fn capture_shared_after_config(
    state: &Arc<AppState>,
    additional_read_roots: Vec<String>,
) -> SharedToolGatewayResources {
    let permission_store = { state.tool_permission_store.lock().await.clone() };
    let registry = { state.mcp_registry.lock().await.clone() };
    let audit_store = { state.mcp_audit_store.lock().await.clone() };
    let privacy_engine = { state.privacy_engine.lock().await.clone() };
    SharedToolGatewayResources {
        additional_read_roots,
        permission_store,
        registry,
        audit_store,
        privacy_engine,
        persistence_coordinator: Arc::clone(&state.persistence_coordinator),
    }
}

async fn capture_governed(
    state: &Arc<AppState>,
    provider_profile_id: Option<&str>,
) -> (GovernedToolGatewayResources, CapturedToolRuntimeConfig) {
    let config = capture_tool_runtime_config(state, provider_profile_id).await;
    let shared = capture_shared_after_config(state, config.additional_read_roots.clone()).await;
    (
        GovernedToolGatewayResources {
            shared,
            network_policy: config.network_policy.clone(),
            search_provider: config.search_provider.clone(),
        },
        config,
    )
}

pub(crate) async fn snapshot_tool_gateway_resources_for_main_chat_read(
    state: &Arc<AppState>,
    provider_profile_id: Option<&str>,
) -> Result<MainChatReadToolGatewayResources, String> {
    let (governed, _) = capture_governed(state, provider_profile_id).await;
    Ok(MainChatReadToolGatewayResources { governed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn governed_snapshot_tracks_each_config_generation_without_global_search_state() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut brave = state.config.lock().await.clone();
        brave.system.search_provider = "brave".into();
        brave.system.search_provider_key = "brave-test-key".into();
        state.replace_provider_runtime_config(brave).await;

        let (first, _) = capture_governed(&state, None).await;
        assert_eq!(first.search_provider.provider, "brave");
        assert_eq!(first.search_provider.api_key, "brave-test-key");

        let mut searxng = state.config.lock().await.clone();
        searxng.system.search_provider = "searxng".into();
        searxng.system.search_provider_key.clear();
        searxng.system.searxng_url = "https://search.example.test".into();
        state.replace_provider_runtime_config(searxng).await;

        let (second, _) = capture_governed(&state, None).await;
        assert_eq!(second.search_provider.provider, "searxng");
        assert_eq!(
            second.search_provider.searxng_url,
            "https://search.example.test"
        );
        assert_eq!(
            first.search_provider.provider, "brave",
            "an in-flight ToolGateway snapshot must remain bound to its original config generation"
        );
    }

    #[tokio::test]
    async fn artifact_output_directory_never_becomes_generic_read_authority() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let artifact_root = tempfile::tempdir().unwrap();
        let read_root = tempfile::tempdir().unwrap();
        let mut config = state.config.lock().await.clone();
        config.system.artifact_output_directory = Some(
            artifact_root
                .path()
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        );
        config.system.additional_read_roots = vec![read_root
            .path()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned()];
        state.replace_provider_runtime_config(config).await;

        let (captured, _) = capture_governed(&state, None).await;
        let canonical_artifact_root = artifact_root
            .path()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        assert_eq!(captured.shared.additional_read_roots.len(), 1);
        assert_eq!(
            captured.shared.additional_read_roots[0],
            read_root.path().canonicalize().unwrap().to_string_lossy()
        );
        assert!(!captured
            .shared
            .additional_read_roots
            .contains(&canonical_artifact_root));
    }

    #[tokio::test]
    async fn official_deepseek_search_reuses_the_selected_model_credential_without_duplication() {
        let mut search =
            openlife_core::agent::action_executor::helpers::SearchProviderConfig::default();
        assert!(bind_search_to_selected_provider_route(
            &mut search,
            "deepseek",
            "deepseek",
            "https://api.deepseek.com",
            "shared-deepseek-test-key".into(),
            "deepseek-chat",
        ));
        assert_eq!(search.provider, "deepseek");
        assert_eq!(search.api_key, "shared-deepseek-test-key");
        assert_eq!(search.model, "deepseek-chat");
    }

    #[tokio::test]
    async fn automatic_openrouter_search_reuses_the_exact_selected_route() {
        let mut search =
            openlife_core::agent::action_executor::helpers::SearchProviderConfig::default();
        assert!(bind_search_to_selected_provider_route(
            &mut search,
            "auto",
            "openrouter",
            "https://openrouter.ai/api/v1",
            "shared-openrouter-test-key".into(),
            "openrouter/test-model",
        ));
        assert_eq!(search.provider, "openrouter");
        assert_eq!(search.model, "openrouter/test-model");
        assert_eq!(search.api_key, "shared-openrouter-test-key");
    }

    #[tokio::test]
    async fn automatic_search_does_not_reuse_a_custom_gateway_credential() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut config = state.config.lock().await.clone();
        config.prefer_local_model = false;
        config.llm.provider = "openrouter".into();
        config.llm.openai_base = "https://proxy.example.test/v1".into();
        config.llm.chat_model = "openrouter/test-model".into();
        config.llm.openai_key = "must-not-cross-boundary".into();
        config.system.search_provider = "auto".into();
        state.replace_provider_runtime_config(config).await;

        let (captured, _) = capture_governed(&state, None).await;
        assert_eq!(captured.search_provider.provider, "unavailable");
        assert!(captured.search_provider.api_key.is_empty());
        assert!(captured.search_provider.model.is_empty());
    }
}
