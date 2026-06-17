//! Application state container and helper types.
//! Holds all shared state for the Tauri application, including
//! store handles, registries, configuration, and lifecycle signals.

use crate::a2a_sidecar;
use openlife_core::builder::{BuilderSession, BuilderSessionStore};
use openlife_core::config::AppConfig;
use openlife_core::feedback::FeedbackStore;
use openlife_core::layer_router::LayerRouter;
use openlife_core::life_model::LifeModelManager;
use openlife_core::mcp::McpRegistry;
use openlife_core::mcp_audit::McpAuditStore;
use openlife_core::memory::MemoryStore;
use openlife_core::memory_cache::SharedHotCache;
use openlife_core::privacy::PrivacyEngine;
use openlife_core::router::IntentRouter;
use openlife_core::scheduler::InferenceScheduler;
use openlife_core::vectors::VectorStore;
use openlife_core::versioning::VersionManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Cached provider health status from ModelRouter, refreshed periodically.
pub struct ProviderHealthCache {
    pub providers: Vec<crate::commands::router::ProviderStatus>,
    pub checked_at: String,
}

impl ProviderHealthCache {
    pub fn is_fresh(&self) -> bool {
        if let Ok(checked) = chrono::DateTime::parse_from_rfc3339(&self.checked_at) {
            let elapsed =
                chrono::Utc::now().signed_duration_since(checked.with_timezone(&chrono::Utc));
            elapsed.num_seconds() < 30
        } else {
            false
        }
    }
}

/// Central application state shared across all Tauri commands.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
    pub life_model_manager: Arc<Mutex<LifeModelManager>>,
    pub memory_store: Arc<Mutex<MemoryStore>>,
    pub mcp_registry: Arc<Mutex<McpRegistry>>,
    pub intent_router: Arc<Mutex<IntentRouter>>,
    pub layer_router: Arc<Mutex<LayerRouter>>,
    pub scheduler: Arc<Mutex<InferenceScheduler>>,
    pub privacy_engine: Arc<Mutex<PrivacyEngine>>,
    pub version_manager: Arc<Mutex<VersionManager>>,
    pub feedback_store: Arc<Mutex<FeedbackStore>>,
    pub vector_store: Arc<Mutex<VectorStore>>,
    pub builder_sessions: Arc<Mutex<HashMap<String, BuilderSession>>>,
    pub builder_session_store: Arc<Mutex<BuilderSessionStore>>,
    pub a2a_sidecar: Arc<Mutex<a2a_sidecar::A2ASidecar>>,
    pub last_snapshot_date: Arc<Mutex<Option<String>>>,
    pub mcp_audit_store: Arc<Mutex<McpAuditStore>>,
    pub agent_run_store: Option<Arc<Mutex<openlife_core::agent::AgentRunStore>>>,
    pub evidence_store: Arc<Mutex<openlife_core::agent::EvidenceStore>>,
    pub heuristic_store: Arc<Mutex<openlife_core::agent::HeuristicStore>>,
    pub policy_store: Arc<openlife_core::agent::PolicyStore>,
    pub proposal_store: Option<Arc<Mutex<openlife_core::agent::ProposalStore>>>,
    pub memory_lifecycle_store: Option<Arc<Mutex<openlife_core::agent::MemoryLifecycleStore>>>,
    pub plan_execute_session_store:
        Option<Arc<Mutex<openlife_core::agent::PlanExecuteSessionStore>>>,
    pub main_chat_agent_session_store:
        Option<Arc<Mutex<openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStore>>>,
    pub main_chat_action_queue_store:
        Option<Arc<Mutex<openlife_core::agent::main_chat_agent_v1::ActionQueueStore>>>,
    pub main_chat_agent_event_store:
        Option<Arc<Mutex<crate::main_chat_event_stream::MainChatAgentEventStore>>>,
    pub main_chat_selected_skill_ids: Arc<Mutex<HashMap<String, String>>>,
    pub patch_store: Option<Arc<Mutex<openlife_core::life_model::patch_store::PatchStore>>>,
    pub rollout_metrics_store: Option<Arc<Mutex<openlife_core::agent::RolloutMetricsStore>>>,
    pub tool_permission_store: Arc<Mutex<openlife_core::tool_permissions::ToolPermissionStore>>,
    pub skill_registry: Arc<Mutex<openlife_core::skills::SkillRegistry>>,
    pub plugin_registry: Arc<Mutex<openlife_core::plugins::PluginRegistry>>,
    pub hot_cache: SharedHotCache,
    pub proposal_engine: Arc<tokio::sync::Mutex<openlife_core::agent::ProposalEngine>>,
    pub startup_warnings: Vec<String>,
    pub provider_health_cache: Arc<tokio::sync::Mutex<Option<ProviderHealthCache>>>,
    pub scheduled_task_mutex: Arc<tokio::sync::Mutex<()>>,
    pub web_search_fixture_output: Arc<tokio::sync::Mutex<Option<String>>>,
    pub shutdown_notify: Arc<tokio::sync::Notify>,
}

impl AppState {
    /// Acquire MCP-related locks in a fixed order to prevent deadlocks.
    pub async fn get_mcp_state(
        &self,
    ) -> (
        tokio::sync::MutexGuard<'_, McpRegistry>,
        tokio::sync::MutexGuard<'_, McpAuditStore>,
    ) {
        let reg = self.mcp_registry.lock().await;
        let audit = self.mcp_audit_store.lock().await;
        (reg, audit)
    }
}
