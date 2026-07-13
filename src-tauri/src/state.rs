//! Application state container and helper types.
//! Holds all shared state for the Tauri application, including
//! store handles, registries, configuration, and lifecycle signals.

use crate::a2a_sidecar;
use openlife_core::builder::BuilderSessionStore;
use openlife_core::config::AppConfig;
use openlife_core::feedback::FeedbackStore;
use openlife_core::life_model::LifeModelManager;
use openlife_core::mcp::McpRegistry;
use openlife_core::mcp_audit::McpAuditStore;
use openlife_core::memory::MemoryStore;
use openlife_core::memory_cache::SharedHotCache;
use openlife_core::privacy::PrivacyEngine;
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
    /// Metadata-only digest of provider/base/model/local preference,
    /// credential version, and the concrete network decision.
    pub identity_digest: String,
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

/// One atomically captured provider runtime generation.
///
/// `AppConfig` owns the canonical policy/configuration while
/// `InferenceScheduler` owns the executable adapter binding.  They are still
/// stored separately for their existing domain responsibilities, but product
/// status/read-model code must consume this snapshot instead of locking them
/// in two independent generations.
#[derive(Clone)]
pub(crate) struct ProviderRuntimeSnapshot {
    pub config: AppConfig,
    pub scheduler: InferenceScheduler,
    pub coherent: bool,
}

fn provider_runtime_is_coherent(config: &AppConfig, scheduler: &InferenceScheduler) -> bool {
    let config_provider = config.llm.provider.trim().to_ascii_lowercase();
    let scheduler_provider = scheduler.provider.trim().to_ascii_lowercase();
    let config_endpoint =
        openlife_core::llm::chat_completions_url(&config_provider, &config.llm.openai_base);
    let scheduler_endpoint =
        openlife_core::llm::chat_completions_url(&scheduler_provider, &scheduler.openai_base);

    scheduler.provider_runtime_identity_is_valid()
        && config_provider == scheduler_provider
        && config_endpoint == scheduler_endpoint
        && config.llm.chat_model.trim() == scheduler.chat_model.trim()
        && config.local_model.trim() == scheduler.local_model.trim()
        && config.prefer_local_model == scheduler.prefer_local
        && config.llm.credential_version == scheduler.provider_credential_version()
        && config.effective_cloud_api_key() == scheduler.effective_api_key()
}

/// In-memory Main Chat route evidence for the current app process.
#[derive(Clone, Debug, Default)]
pub struct MainChatRuntimeState {
    pub legacy_fallback_used_count: u64,
    pub last_legacy_fallback_reason_code: Option<String>,
    pub last_legacy_fallback_at: Option<String>,
    pub last_kernel_event_count: Option<usize>,
    pub latest_turn_route_evidence: Option<MainChatTurnRouteEvidenceSnapshot>,
    pub latest_final_gate_readiness: Option<MainChatFinalGateReadinessSnapshot>,
    pub(crate) cancellation_registry: crate::main_chat_cancellation::MainChatCancellationRegistry,
}

impl MainChatRuntimeState {
    pub fn shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::default()))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VectorPersistenceMode {
    #[default]
    Enabled,
    EvalDisabled,
}

impl VectorPersistenceMode {
    pub fn skip_reason(self) -> Option<&'static str> {
        match self {
            Self::Enabled => None,
            Self::EvalDisabled => Some("eval_disabled"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MainChatFinalGateReadinessSnapshot {
    pub status: String,
    pub blockers: Vec<String>,
    pub last_report_run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MainChatTurnRouteEvidenceSnapshot {
    pub stream_mode: String,
    pub execution_path: String,
    pub strategy_label: String,
    pub reason_code: String,
    pub kernel_supported: bool,
    pub kernel_support_disposition: String,
    pub fallback_allowed: bool,
    pub requires_tool_loop: bool,
    pub observed_agent_loop: bool,
    pub observed_agent_loop_without_fallback: bool,
    pub legacy_fallback_used: bool,
    pub kernel_event_count: Option<usize>,
    pub recorded_at: String,
}

/// Central application state shared across all Tauri commands.
#[derive(Clone)]
pub struct AppState {
    pub persistence_coordinator: Arc<crate::persistence_coordinator::PersistenceCoordinator>,
    pub config: Arc<Mutex<AppConfig>>,
    pub life_model_manager: Arc<Mutex<LifeModelManager>>,
    /// Operation-level serialization for the file journal, canonical rename,
    /// and derived projection protocol. It never owns product data itself.
    pub life_model_write_coordinator: Arc<Mutex<()>>,
    pub memory_store: Arc<Mutex<MemoryStore>>,
    pub mcp_registry: Arc<Mutex<McpRegistry>>,
    pub scheduler: Arc<Mutex<InferenceScheduler>>,
    pub privacy_engine: Arc<Mutex<PrivacyEngine>>,
    pub version_manager: Arc<Mutex<VersionManager>>,
    pub feedback_store: Arc<Mutex<FeedbackStore>>,
    pub vector_store: Arc<Mutex<VectorStore>>,
    pub vector_persistence_mode: VectorPersistenceMode,
    pub builder_session_store: Arc<Mutex<BuilderSessionStore>>,
    pub a2a_sidecar: Arc<Mutex<a2a_sidecar::A2ASidecar>>,
    pub last_snapshot_date: Arc<Mutex<Option<String>>>,
    pub mcp_audit_store: Arc<Mutex<McpAuditStore>>,
    pub(crate) mcp_audit_read_gateway: Arc<crate::mcp_audit_read_gateway::McpAuditReadGateway>,
    pub agent_run_store: Option<Arc<Mutex<openlife_core::agent::AgentRunStore>>>,
    pub evidence_store: Arc<Mutex<openlife_core::agent::EvidenceStore>>,
    pub life_event_store: Option<Arc<Mutex<openlife_core::agent::LifeEventStore>>>,
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
    pub main_chat_runtime_state: Arc<Mutex<MainChatRuntimeState>>,
    pub patch_store: Option<Arc<Mutex<openlife_core::life_model::patch_store::PatchStore>>>,
    pub rollout_metrics_store: Option<Arc<Mutex<openlife_core::agent::RolloutMetricsStore>>>,
    pub tool_permission_store: Arc<Mutex<openlife_core::tool_permissions::ToolPermissionStore>>,
    pub skill_registry: Arc<Mutex<openlife_core::skills::SkillRegistry>>,
    pub plugin_registry: Arc<Mutex<openlife_core::plugins::PluginRegistry>>,
    pub hot_cache: SharedHotCache,
    pub startup_warnings: Vec<String>,
    pub provider_health_cache: Arc<tokio::sync::Mutex<Option<ProviderHealthCache>>>,
    pub scheduled_task_store: Arc<openlife_core::tasks::TaskStore>,
    pub(crate) runtime_clock_source:
        Arc<tokio::sync::Mutex<crate::main_chat_runtime_facts::MainChatRuntimeClockSource>>,
    pub web_search_fixture_output: Arc<tokio::sync::Mutex<Option<String>>>,
    pub shutdown_notify: Arc<tokio::sync::Notify>,
}

impl AppState {
    /// Capture config and executable provider state while both generation locks
    /// are held in the canonical order.  A legacy/direct partial mutation is
    /// exposed as `coherent = false`; readers must fail closed instead of
    /// combining fields from two authorities.
    pub(crate) async fn provider_runtime_snapshot(&self) -> ProviderRuntimeSnapshot {
        let config = self.config.lock().await;
        let scheduler = self.scheduler.lock().await;
        ProviderRuntimeSnapshot {
            config: config.clone(),
            scheduler: scheduler.clone(),
            coherent: provider_runtime_is_coherent(&config, &scheduler),
        }
    }

    /// Replace the canonical provider configuration and its executable
    /// scheduler as one in-process generation.  The cache is invalidated under
    /// the same lock order, so a status reader observes either the old or the
    /// new generation, never a mixed pair.
    pub(crate) async fn replace_provider_runtime_config(&self, config: AppConfig) -> String {
        let new_scheduler = InferenceScheduler::new(
            config.local_model.clone(),
            config.prefer_local_model,
            config.llm.provider.clone(),
            config.llm.openai_base.clone(),
            config.llm.openai_key.clone(),
            config.llm.chat_model.clone(),
            config.llm.embedding_model.clone(),
            config.llm.embedding_enabled,
        )
        .with_provider_credential_version(config.llm.credential_version);
        let generation = new_scheduler.provider_config_generation().to_string();
        let mut current_config = self.config.lock().await;
        let mut scheduler = self.scheduler.lock().await;
        let mut provider_health_cache = self.provider_health_cache.lock().await;
        *current_config = config;
        *scheduler = new_scheduler;
        *provider_health_cache = None;
        generation
    }
}
