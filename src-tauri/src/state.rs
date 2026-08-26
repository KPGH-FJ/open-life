//! Application state container and helper types.
//! Holds all shared state for the Tauri application, including
//! store handles, registries, configuration, and lifecycle signals.

use openlife_core::config::AppConfig;
use openlife_core::conversation::ConversationStore;
use openlife_core::feedback::FeedbackStore;
use openlife_core::life_model::LifeModelManager;
use openlife_core::mcp::McpRegistry;
use openlife_core::mcp_audit::McpAuditStore;
use openlife_core::memory::KnowledgeNoteProjectionStore;
use openlife_core::privacy::PrivacyEngine;
use openlife_core::scheduler::InferenceScheduler;
use openlife_core::vectors::VectorStore;
use openlife_core::versioning::VersionManager;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

const CREDENTIAL_BOOTSTRAP_SNAPSHOT_VERSION: &str = "credential_bootstrap_v2";

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_digest: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialBootstrapSnapshot {
    pub version: String,
    pub digest: String,
    pub purposes: Vec<CredentialPurposeBootstrapState>,
}

impl CredentialBootstrapSnapshot {
    pub(crate) fn from_statuses(statuses: [CredentialBootstrapStatus; 2]) -> Self {
        let purpose_names = ["canonical_task_receipts", "mcp_audit"];
        let mut purposes = purpose_names
            .into_iter()
            .zip(statuses)
            .map(|(purpose, status)| CredentialPurposeBootstrapState {
                purpose: purpose.into(),
                status,
                scope_digest: None,
            })
            .collect::<Vec<_>>();
        purposes.push(CredentialPurposeBootstrapState {
            purpose: "provider_connections".into(),
            status: CredentialBootstrapStatus::MissingExistingData,
            scope_digest: None,
        });
        purposes.push(CredentialPurposeBootstrapState {
            purpose: "search_provider_api_key".into(),
            status: CredentialBootstrapStatus::MissingExistingData,
            scope_digest: None,
        });
        Self::from_purposes(purposes)
    }

    pub(crate) fn with_provider_connections_status(
        mut self,
        status: CredentialBootstrapStatus,
        scope_digest: Option<String>,
    ) -> Self {
        if let Some(provider) = self
            .purposes
            .iter_mut()
            .find(|item| item.purpose == "provider_connections")
        {
            provider.status = status;
            provider.scope_digest = scope_digest;
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
                if let Some(scope_digest) = item.scope_digest.as_deref() {
                    material.push('@');
                    material.push_str(scope_digest);
                }
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
        Self::from_statuses([CredentialBootstrapStatus::Unknown; 2])
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkReviewDecision {
    Accepted,
    Rejected,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WorkReviewDecisionRegistry {
    pending:
        Arc<std::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<WorkReviewDecision>>>>,
}

impl WorkReviewDecisionRegistry {
    pub(crate) fn register(
        &self,
        proposal_id: &str,
    ) -> Result<tokio::sync::oneshot::Receiver<WorkReviewDecision>, String> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "work_review_decision_registry_poisoned".to_string())?;
        if pending.contains_key(proposal_id) {
            return Err("work_review_decision_owner_conflict".into());
        }
        pending.insert(proposal_id.to_string(), sender);
        Ok(receiver)
    }

    pub(crate) fn resolve(
        &self,
        proposal_id: &str,
        decision: WorkReviewDecision,
    ) -> Result<bool, String> {
        let sender = self
            .pending
            .lock()
            .map_err(|_| "work_review_decision_registry_poisoned".to_string())?
            .remove(proposal_id);
        Ok(sender.is_some_and(|sender| sender.send(decision).is_ok()))
    }

    pub(crate) fn has_waiter(&self, proposal_id: &str) -> Result<bool, String> {
        Ok(self
            .pending
            .lock()
            .map_err(|_| "work_review_decision_registry_poisoned".to_string())?
            .contains_key(proposal_id))
    }

    pub(crate) fn discard(&self, proposal_id: &str) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(proposal_id);
        }
    }
}

#[derive(Clone, Debug)]
pub struct MainChatRuntimeState {
    pub(crate) cancellation_registry: crate::main_chat_cancellation::MainChatCancellationRegistry,
    pub(crate) work_review_decision_registry: WorkReviewDecisionRegistry,
    pub(crate) execution_slots: Arc<tokio::sync::Semaphore>,
}

impl Default for MainChatRuntimeState {
    fn default() -> Self {
        Self {
            cancellation_registry: Default::default(),
            work_review_decision_registry: Default::default(),
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

/// Central application state shared across all Tauri commands.
#[derive(Clone)]
pub struct AppState {
    pub persistence_coordinator: Arc<crate::persistence_coordinator::PersistenceCoordinator>,
    pub config: Arc<Mutex<AppConfig>>,
    pub life_model_manager: Arc<Mutex<LifeModelManager>>,
    /// Operation-level serialization for the file journal, canonical rename,
    /// and derived projection protocol. It never owns product data itself.
    pub life_model_write_coordinator: Arc<Mutex<()>>,
    /// Canonical KnowledgeNote owner and derived projection store for
    /// lifecycle-owned Agent Memory. The field name remains aligned with the
    /// stable on-disk/outbox `MemoryStore` protocol identity.
    pub memory_store: Arc<Mutex<KnowledgeNoteProjectionStore>>,
    /// Canonical owner for ordinary Chat Conversation, Turn, and Item
    /// lifecycle. It is intentionally independent of Memory and Task stores.
    pub conversation_store: Option<Arc<Mutex<ConversationStore>>>,
    pub mcp_registry: Arc<Mutex<McpRegistry>>,
    pub scheduler: Arc<Mutex<InferenceScheduler>>,
    pub privacy_engine: Arc<Mutex<PrivacyEngine>>,
    pub version_manager: Arc<Mutex<VersionManager>>,
    pub feedback_store: Arc<Mutex<FeedbackStore>>,
    pub vector_store: Arc<Mutex<VectorStore>>,
    pub last_snapshot_date: Arc<Mutex<Option<String>>>,
    pub mcp_audit_store: Arc<Mutex<McpAuditStore>>,
    /// Canonical Work owner for Task, Run, Item, ItemAttempt, FinalResult, and
    /// Artifact metadata. Its receipts use the independent canonical Task
    /// authority; retired run-store state is not part of this lifecycle.
    pub canonical_task_runtime_store:
        Option<Arc<Mutex<openlife_core::task_runtime::CanonicalTaskRuntimeStore>>>,
    pub proposal_store: Option<Arc<Mutex<openlife_core::agent::ProposalStore>>>,
    pub memory_lifecycle_store: Option<Arc<Mutex<openlife_core::agent::MemoryLifecycleStore>>>,
    /// Bounded Observation/Candidate bridge for LifeModel learning. It does
    /// not own proposals or canonical LifeModel state.
    pub life_model_learning_store: Option<Arc<Mutex<openlife_core::agent::LifeModelLearningStore>>>,
    pub main_chat_runtime_state: Arc<Mutex<MainChatRuntimeState>>,
    pub tool_permission_store: Arc<Mutex<openlife_core::tool_permissions::ToolPermissionStore>>,
    pub skill_registry: Arc<Mutex<openlife_core::skills::SkillRegistry>>,
    pub startup_warnings: Vec<String>,
    pub credential_bootstrap_snapshot: CredentialBootstrapSnapshot,
    #[cfg(test)]
    pub web_search_fixture_output: Arc<tokio::sync::Mutex<Option<String>>>,
    /// Controlled provider output for the model-authored Work plan. Product
    /// builds never expose this seam; behavior tests use it instead of
    /// substituting deterministic keyword planning.
    #[cfg(test)]
    pub work_initial_decision_fixture_output: Arc<tokio::sync::Mutex<Option<String>>>,
    /// Controlled output for the independent authenticated Work-goal contract.
    /// Product builds always ask the selected provider before planning; tests
    /// may supply the capability floor without relying on keyword routing.
    #[cfg(test)]
    pub work_goal_contract_fixture_output: Arc<tokio::sync::Mutex<Option<String>>>,
    /// Controlled typed replanning output for an authenticated Steering item.
    /// Product builds always use the selected provider at the safe checkpoint.
    #[cfg(test)]
    pub work_steering_replan_fixture_output: Arc<tokio::sync::Mutex<Option<String>>>,
    /// Ordered model outputs for typed AgentStep decisions. This seam exists
    /// only in controlled tests; production always asks the selected provider.
    #[cfg(test)]
    pub work_agent_step_fixture_outputs: Arc<tokio::sync::Mutex<Vec<String>>>,
    /// Ordered semantic verification decisions for source-backed Work
    /// candidates. Product builds always call the selected provider; tests may
    /// supply an independent verifier outcome without changing AgentStep
    /// fixtures.
    #[cfg(test)]
    pub work_semantic_verification_fixture_outputs: Arc<tokio::sync::Mutex<Vec<String>>>,
    pub(crate) resource_runtime: Option<Arc<crate::resource_commands::ResourceRuntime>>,
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
