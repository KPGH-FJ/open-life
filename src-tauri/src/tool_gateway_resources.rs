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
    pub(crate) safe_paths: Vec<String>,
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
    safe_paths: Vec<String>,
    network_policy: openlife_core::config::NetworkPolicy,
    search_provider: openlife_core::agent::action_executor::helpers::SearchProviderConfig,
}

async fn capture_tool_runtime_config(state: &Arc<AppState>) -> CapturedToolRuntimeConfig {
    let config = state.config.lock().await;
    let mut search_provider =
        openlife_core::agent::action_executor::helpers::SearchProviderConfig::from_system_config(
            &config.system,
        );
    search_provider.provider = config
        .effective_search_provider()
        .unwrap_or("unavailable")
        .to_string();
    if search_provider.api_key.trim().is_empty()
        && config.search_reuses_selected_provider_credential()
    {
        search_provider.api_key = config.llm.openai_key.clone();
        search_provider.model = config.llm.chat_model.clone();
    }
    CapturedToolRuntimeConfig {
        safe_paths: config.system.safe_paths.clone(),
        network_policy: config.system.network_policy.clone(),
        search_provider,
    }
}

async fn capture_shared_after_config(
    state: &Arc<AppState>,
    safe_paths: Vec<String>,
) -> SharedToolGatewayResources {
    let permission_store = { state.tool_permission_store.lock().await.clone() };
    let registry = { state.mcp_registry.lock().await.clone() };
    let audit_store = { state.mcp_audit_store.lock().await.clone() };
    let privacy_engine = { state.privacy_engine.lock().await.clone() };
    SharedToolGatewayResources {
        safe_paths,
        permission_store,
        registry,
        audit_store,
        privacy_engine,
        persistence_coordinator: Arc::clone(&state.persistence_coordinator),
    }
}

async fn capture_governed(
    state: &Arc<AppState>,
) -> (GovernedToolGatewayResources, CapturedToolRuntimeConfig) {
    let config = capture_tool_runtime_config(state).await;
    let shared = capture_shared_after_config(state, config.safe_paths.clone()).await;
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
) -> Result<MainChatReadToolGatewayResources, String> {
    let (governed, _) = capture_governed(state).await;
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

        let (first, _) = capture_governed(&state).await;
        assert_eq!(first.search_provider.provider, "brave");
        assert_eq!(first.search_provider.api_key, "brave-test-key");

        let mut searxng = state.config.lock().await.clone();
        searxng.system.search_provider = "searxng".into();
        searxng.system.search_provider_key.clear();
        searxng.system.searxng_url = "https://search.example.test".into();
        state.replace_provider_runtime_config(searxng).await;

        let (second, _) = capture_governed(&state).await;
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
    async fn official_deepseek_search_reuses_the_selected_model_credential_without_duplication() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut config = state.config.lock().await.clone();
        config.prefer_local_model = false;
        config.llm.provider = "deepseek".into();
        config.llm.openai_base = "https://api.deepseek.com".into();
        config.llm.openai_key = "shared-deepseek-test-key".into();
        config.system.search_provider = "deepseek".into();
        config.system.search_provider_key.clear();
        state.replace_provider_runtime_config(config).await;

        let (captured, _) = capture_governed(&state).await;
        assert_eq!(captured.search_provider.provider, "deepseek");
        assert_eq!(captured.search_provider.api_key, "shared-deepseek-test-key");
    }

    #[tokio::test]
    async fn automatic_openrouter_search_reuses_the_exact_selected_route() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut config = state.config.lock().await.clone();
        config.prefer_local_model = false;
        config.llm.provider = "openrouter".into();
        config.llm.openai_base = "https://openrouter.ai/api/v1".into();
        config.llm.chat_model = "openrouter/test-model".into();
        config.llm.openai_key = "shared-openrouter-test-key".into();
        config.system.search_provider = "auto".into();
        config.system.search_provider_key.clear();
        state.replace_provider_runtime_config(config).await;

        let (captured, _) = capture_governed(&state).await;
        assert_eq!(captured.search_provider.provider, "openrouter");
        assert_eq!(captured.search_provider.model, "openrouter/test-model");
        assert_eq!(
            captured.search_provider.api_key,
            "shared-openrouter-test-key"
        );
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

        let (captured, _) = capture_governed(&state).await;
        assert_eq!(captured.search_provider.provider, "unavailable");
        assert!(captured.search_provider.api_key.is_empty());
        assert!(captured.search_provider.model.is_empty());
    }
}
