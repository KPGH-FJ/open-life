//! Application state container and helper types.
//! Holds all shared state for the Tauri application, including
//! store handles, registries, configuration, and lifecycle signals.

use openlife_core::config::AppConfig;
use openlife_core::conversation::ConversationStore;
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
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::Mutex;

const CREDENTIAL_BOOTSTRAP_SNAPSHOT_VERSION: &str = "credential_bootstrap_v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialBootstrapStatus {
    Available,
    InitializationRequired,
    MissingExistingData,
    Invalid,
    Unavailable,
    Unknown,
}

impl CredentialBootstrapStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::InitializationRequired => "initialization_required",
            Self::MissingExistingData => "missing_existing_data",
            Self::Invalid => "invalid",
            Self::Unavailable => "unavailable",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialPurposeBootstrapState {
    pub purpose: String,
    pub status: CredentialBootstrapStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialBootstrapSnapshot {
    pub version: String,
    pub digest: String,
    pub purposes: Vec<CredentialPurposeBootstrapState>,
}

impl CredentialBootstrapSnapshot {
    pub(crate) fn from_statuses(statuses: [CredentialBootstrapStatus; 3]) -> Self {
        let purpose_names = ["canonical_task_receipts", "task_store", "mcp_audit"];
        let mut purposes = purpose_names
            .into_iter()
            .zip(statuses)
            .map(|(purpose, status)| CredentialPurposeBootstrapState {
                purpose: purpose.into(),
                status,
            })
            .collect::<Vec<_>>();
        purposes.push(CredentialPurposeBootstrapState {
            purpose: "provider_api_key".into(),
            status: CredentialBootstrapStatus::MissingExistingData,
        });
        purposes.push(CredentialPurposeBootstrapState {
            purpose: "search_provider_api_key".into(),
            status: CredentialBootstrapStatus::MissingExistingData,
        });
        Self::from_purposes(purposes)
    }

    pub(crate) fn with_provider_status(mut self, status: CredentialBootstrapStatus) -> Self {
        if let Some(provider) = self
            .purposes
            .iter_mut()
            .find(|item| item.purpose == "provider_api_key")
        {
            provider.status = status;
        }
        Self::from_purposes(self.purposes)
    }

    pub(crate) fn with_search_provider_status(mut self, status: CredentialBootstrapStatus) -> Self {
        if let Some(provider) = self
            .purposes
            .iter_mut()
            .find(|item| item.purpose == "search_provider_api_key")
        {
            provider.status = status;
        }
        Self::from_purposes(self.purposes)
    }

    fn from_purposes(purposes: Vec<CredentialPurposeBootstrapState>) -> Self {
        let digest_material = purposes.iter().fold(
            CREDENTIAL_BOOTSTRAP_SNAPSHOT_VERSION.to_string(),
            |mut material, item| {
                material.push('|');
                material.push_str(&item.purpose);
                material.push('=');
                material.push_str(item.status.as_str());
                material
            },
        );
        let digest = format!("{:x}", Sha256::digest(digest_material.as_bytes()));
        Self {
            version: CREDENTIAL_BOOTSTRAP_SNAPSHOT_VERSION.into(),
            digest,
            purposes,
        }
    }
}

impl Default for CredentialBootstrapSnapshot {
    fn default() -> Self {
        Self::from_statuses([CredentialBootstrapStatus::Unknown; 3])
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
const DEFAULT_MAIN_CHAT_CONCURRENCY_LIMIT: usize = 3;

#[derive(Clone, Debug)]
pub struct MainChatRuntimeState {
    pub(crate) cancellation_registry: crate::main_chat_cancellation::MainChatCancellationRegistry,
    pub(crate) execution_slots: Arc<tokio::sync::Semaphore>,
}

impl Default for MainChatRuntimeState {
    fn default() -> Self {
        Self {
            cancellation_registry: Default::default(),
            execution_slots: Arc::new(tokio::sync::Semaphore::new(
                DEFAULT_MAIN_CHAT_CONCURRENCY_LIMIT,
            )),
        }
    }
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

/// Central application state shared across all Tauri commands.
#[derive(Clone)]
pub struct AppState {
    pub persistence_coordinator: Arc<crate::persistence_coordinator::PersistenceCoordinator>,
    /// Bootstrap-owned governed import journal. Construction performs schema
    /// migration, so product commands must reuse this instance and fail closed
    /// when bootstrap could not open it.
    pub(crate) governed_data_import_journal:
        Option<Arc<openlife_core::persistence_outbox::GovernedDataImportJournal>>,
    pub config: Arc<Mutex<AppConfig>>,
    pub life_model_manager: Arc<Mutex<LifeModelManager>>,
    /// Operation-level serialization for the file journal, canonical rename,
    /// and derived projection protocol. It never owns product data itself.
    pub life_model_write_coordinator: Arc<Mutex<()>>,
    pub memory_store: Arc<Mutex<MemoryStore>>,
    /// R1 canonical owner for ordinary Chat Conversation, Turn, and Item
    /// lifecycle. It is intentionally independent of Memory and Task stores.
    pub conversation_store: Option<Arc<Mutex<ConversationStore>>>,
    pub mcp_registry: Arc<Mutex<McpRegistry>>,
    pub scheduler: Arc<Mutex<InferenceScheduler>>,
    pub privacy_engine: Arc<Mutex<PrivacyEngine>>,
    pub version_manager: Arc<Mutex<VersionManager>>,
    pub feedback_store: Arc<Mutex<FeedbackStore>>,
    pub vector_store: Arc<Mutex<VectorStore>>,
    pub vector_persistence_mode: VectorPersistenceMode,
    pub last_snapshot_date: Arc<Mutex<Option<String>>>,
    pub mcp_audit_store: Arc<Mutex<McpAuditStore>>,
    /// Canonical Work owner for Task, Run, Item, ItemAttempt, FinalResult, and
    /// Artifact metadata. Its receipts use the independent canonical Task
    /// authority; retired run-store state is not part of this lifecycle.
    pub canonical_task_runtime_store:
        Option<Arc<Mutex<openlife_core::task_runtime::CanonicalTaskRuntimeStore>>>,
    pub evidence_store: Arc<Mutex<openlife_core::agent::EvidenceStore>>,
    pub policy_store: Arc<openlife_core::agent::PolicyStore>,
    pub proposal_store: Option<Arc<Mutex<openlife_core::agent::ProposalStore>>>,
    pub memory_lifecycle_store: Option<Arc<Mutex<openlife_core::agent::MemoryLifecycleStore>>>,
    /// Bounded Observation/Candidate bridge for LifeModel learning. It does
    /// not own proposals or canonical LifeModel state.
    pub life_model_learning_store: Option<Arc<Mutex<openlife_core::agent::LifeModelLearningStore>>>,
    pub main_chat_runtime_state: Arc<Mutex<MainChatRuntimeState>>,
    pub patch_store: Option<Arc<Mutex<openlife_core::life_model::patch_store::PatchStore>>>,
    pub tool_permission_store: Arc<Mutex<openlife_core::tool_permissions::ToolPermissionStore>>,
    pub skill_registry: Arc<Mutex<openlife_core::skills::SkillRegistry>>,
    pub hot_cache: SharedHotCache,
    pub startup_warnings: Vec<String>,
    pub credential_bootstrap_snapshot: CredentialBootstrapSnapshot,
    pub scheduled_task_store: Arc<openlife_core::tasks::TaskStore>,
    pub web_search_fixture_output: Arc<tokio::sync::Mutex<Option<String>>>,
    pub(crate) resource_runtime: Option<Arc<crate::resource_commands::ResourceRuntime>>,
    /// Canonical ADR 0015 owner. Absence is an explicit degraded state; release
    /// bootstrap never replaces it with a temporary or in-memory product store.
    pub(crate) state_store: Option<Arc<openlife_core::state_store::StateStore>>,
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
    /// scheduler as one in-process generation, so readers observe either the
    /// old or the new route rather than fields from both.
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
        *current_config = config;
        *scheduler = new_scheduler;
        generation
    }
}
