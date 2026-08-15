use crate::persistence_coordinator::{
    PersistenceHealthSnapshot, PersistenceRuntimeMode, PersistenceStoreHealth,
};
use crate::runtime_build_info::RuntimeBuildInfo;
use crate::state::{AppState, CredentialBootstrapSnapshot};
use openlife_core::task_runtime::CanonicalTaskStatus;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::Arc;
use tauri::State;

const PRODUCT_STORE_NAMES: [&str; 6] = [
    "ConversationStore",
    "CanonicalTaskRuntimeStore",
    "ProposalStore",
    "MemoryLifecycleStore",
    "LifeModelFileStore",
    "LifeModelLearningStore",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductStoreDiagnostic {
    pub store: String,
    pub status: String,
    pub reason_code: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductContentCounts {
    pub project_count: Option<usize>,
    pub conversation_count: Option<usize>,
    pub task_count: Option<usize>,
    pub active_task_count: Option<usize>,
    pub waiting_task_count: Option<usize>,
    pub completed_task_count: Option<usize>,
    pub failed_task_count: Option<usize>,
    pub unresolved_attention_count: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDiagnosticsViewModel {
    pub generated_at: String,
    pub status: String,
    pub app_version: String,
    pub runtime_build: RuntimeBuildInfo,
    pub persistence_mode: String,
    pub canonical_writes_allowed: bool,
    pub provider_dispatch_allowed: bool,
    pub tool_dispatch_allowed: bool,
    pub stores: Vec<ProductStoreDiagnostic>,
    pub counts: ProductContentCounts,
    pub credential_bootstrap: CredentialBootstrapSnapshot,
    pub blocker_codes: Vec<String>,
}

fn runtime_mode_label(mode: PersistenceRuntimeMode) -> &'static str {
    match mode {
        PersistenceRuntimeMode::Initializing => "initializing",
        PersistenceRuntimeMode::ReadWrite => "read_write",
        PersistenceRuntimeMode::ReadOnlyDegraded => "read_only_degraded",
        PersistenceRuntimeMode::UnavailableDegraded => "unavailable_degraded",
        PersistenceRuntimeMode::EphemeralDevelopment => "ephemeral_development",
        PersistenceRuntimeMode::IsolatedEvaluation => "isolated_evaluation",
    }
}

fn product_store_diagnostics(
    health: &PersistenceHealthSnapshot,
    blockers: &mut BTreeSet<String>,
) -> Vec<ProductStoreDiagnostic> {
    PRODUCT_STORE_NAMES
        .into_iter()
        .map(|name| {
            let observed: Option<&PersistenceStoreHealth> =
                health.stores.iter().find(|store| store.store == name);
            let (status, reason_code) = match observed {
                Some(store) => (
                    serde_json::to_value(store.mode)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .unwrap_or_else(|| "unknown".into()),
                    store.reason_code.clone(),
                ),
                None => ("unknown".into(), Some("store_health_not_reported".into())),
            };
            if status != "read_write_canonical" {
                blockers.insert(format!(
                    "store:{}:{}",
                    name,
                    reason_code.as_deref().unwrap_or(&status)
                ));
            }
            ProductStoreDiagnostic {
                store: name.into(),
                status,
                reason_code,
            }
        })
        .collect()
}

pub(crate) async fn get_product_diagnostics_view_model_with_state(
    state: &Arc<AppState>,
) -> ProductDiagnosticsViewModel {
    let health = state.persistence_coordinator.snapshot();
    let canonical_work_ready = state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore", "CanonicalTaskRuntimeStore"])
        .is_ok();
    let mut blocker_codes = health
        .global_reason_codes
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let stores = product_store_diagnostics(&health, &mut blocker_codes);
    let mut counts = ProductContentCounts::default();

    if let Some(store) = state.conversation_store.as_ref() {
        let store = store.lock().await;
        match store.list_projects(1_000) {
            Ok(projects) => counts.project_count = Some(projects.len()),
            Err(_) => {
                blocker_codes.insert("conversation_store:project_count_failed".into());
            }
        }
        match store.list_conversations(false, 1_000) {
            Ok(conversations) => counts.conversation_count = Some(conversations.len()),
            Err(_) => {
                blocker_codes.insert("conversation_store:conversation_count_failed".into());
            }
        }
    } else {
        blocker_codes.insert("conversation_store:unavailable".into());
    }

    if let Some(store) = state.canonical_task_runtime_store.as_ref() {
        match store.lock().await.list_task_snapshots(200) {
            Ok(tasks) => {
                counts.task_count = Some(tasks.len());
                counts.active_task_count = Some(
                    tasks
                        .iter()
                        .filter(|snapshot| {
                            matches!(
                                snapshot.task.status,
                                CanonicalTaskStatus::Running | CanonicalTaskStatus::Interrupted
                            )
                        })
                        .count(),
                );
                counts.waiting_task_count = Some(
                    tasks
                        .iter()
                        .filter(|snapshot| {
                            matches!(
                                snapshot.task.status,
                                CanonicalTaskStatus::WaitingReview
                                    | CanonicalTaskStatus::Blocked
                                    | CanonicalTaskStatus::EffectUnknown
                            )
                        })
                        .count(),
                );
                counts.completed_task_count = Some(
                    tasks
                        .iter()
                        .filter(|snapshot| snapshot.task.status == CanonicalTaskStatus::Completed)
                        .count(),
                );
                counts.failed_task_count = Some(
                    tasks
                        .iter()
                        .filter(|snapshot| {
                            matches!(
                                snapshot.task.status,
                                CanonicalTaskStatus::Failed | CanonicalTaskStatus::Cancelled
                            )
                        })
                        .count(),
                );
                counts.unresolved_attention_count = Some(
                    tasks
                        .iter()
                        .flat_map(|snapshot| snapshot.attention.iter())
                        .filter(|attention| attention.resolved_at.is_none())
                        .count(),
                );
            }
            Err(_) => {
                blocker_codes.insert("canonical_task_runtime_store:count_failed".into());
            }
        }
    } else {
        blocker_codes.insert("canonical_task_runtime_store:unavailable".into());
    }

    if !canonical_work_ready {
        blocker_codes.insert(format!(
            "persistence_mode:{}",
            runtime_mode_label(health.mode)
        ));
    }
    let status = if blocker_codes.is_empty() {
        "ready"
    } else if counts.conversation_count.is_some() && counts.task_count.is_some() {
        "degraded"
    } else {
        "blocked"
    };

    ProductDiagnosticsViewModel {
        generated_at: chrono::Utc::now().to_rfc3339(),
        status: status.into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        runtime_build: crate::runtime_build_info::collect_runtime_build_info().await,
        persistence_mode: if canonical_work_ready {
            "read_write".into()
        } else {
            runtime_mode_label(health.mode).into()
        },
        canonical_writes_allowed: canonical_work_ready,
        provider_dispatch_allowed: canonical_work_ready,
        tool_dispatch_allowed: canonical_work_ready,
        stores,
        counts,
        credential_bootstrap: state.credential_bootstrap_snapshot.clone(),
        blocker_codes: blocker_codes.into_iter().collect(),
    }
}

#[tauri::command]
pub async fn get_product_diagnostics_view_model(
    state: State<'_, Arc<AppState>>,
) -> Result<ProductDiagnosticsViewModel, String> {
    Ok(get_product_diagnostics_view_model_with_state(state.inner()).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn product_diagnostics_counts_only_canonical_conversations_and_tasks() {
        let state = crate::test_utils::test_app_state();
        let project_id = "00000000-0000-4000-8000-000000000101";
        let conversation_id = "00000000-0000-4000-8000-000000000102";
        let task_id = "00000000-0000-4000-8000-000000000103";
        let run_id = "00000000-0000-4000-8000-000000000104";
        let session_id = "00000000-0000-4000-8000-000000000105";
        {
            let conversations = state.conversation_store.as_ref().unwrap().lock().await;
            conversations
                .create_project(project_id, "Diagnostics", None)
                .unwrap();
            conversations
                .create_conversation(conversation_id, "Diagnostics")
                .unwrap();
        }
        {
            let tasks = state
                .canonical_task_runtime_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            tasks
                .begin_general_task_run(openlife_core::task_runtime::BeginGeneralTaskRunInput {
                    task_id,
                    conversation_id,
                    run_id,
                    execution_session_id: session_id,
                    instruction_digest: &format!("sha256:{}", "1".repeat(64)),
                    plan_digest: None,
                    project_id: None,
                    project_revision: None,
                    scope_digest: None,
                })
                .unwrap();
        }

        let diagnostics = get_product_diagnostics_view_model_with_state(&state).await;

        assert_eq!(diagnostics.counts.project_count, Some(1));
        assert_eq!(diagnostics.counts.conversation_count, Some(1));
        assert_eq!(diagnostics.counts.task_count, Some(1));
        assert_eq!(diagnostics.counts.active_task_count, Some(1));
        assert_eq!(
            diagnostics.runtime_build.bundle_identifier,
            crate::runtime_build_info::bundle_identifier_for_profile(
                &crate::storage::openlife_profile()
            )
        );
    }

    #[tokio::test]
    async fn product_diagnostics_reads_two_hundred_canonical_tasks_inside_controlled_budget() {
        let state = crate::test_utils::test_app_state();
        let identities = (0..200)
            .map(|_| {
                (
                    uuid::Uuid::new_v4().to_string(),
                    uuid::Uuid::new_v4().to_string(),
                    uuid::Uuid::new_v4().to_string(),
                    uuid::Uuid::new_v4().to_string(),
                )
            })
            .collect::<Vec<_>>();
        {
            let conversations = state.conversation_store.as_ref().unwrap().lock().await;
            for (ordinal, (conversation_id, _, _, _)) in identities.iter().enumerate() {
                conversations
                    .create_conversation(conversation_id, &format!("Diagnostics {ordinal}"))
                    .unwrap();
            }
        }
        {
            let tasks = state
                .canonical_task_runtime_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            for (conversation_id, task_id, run_id, session_id) in &identities {
                tasks
                    .begin_general_task_run(openlife_core::task_runtime::BeginGeneralTaskRunInput {
                        task_id,
                        conversation_id,
                        run_id,
                        execution_session_id: session_id,
                        instruction_digest: &format!("sha256:{}", "2".repeat(64)),
                        plan_digest: None,
                        project_id: None,
                        project_revision: None,
                        scope_digest: None,
                    })
                    .unwrap();
            }
        }

        let started = std::time::Instant::now();
        let diagnostics = get_product_diagnostics_view_model_with_state(&state).await;
        let elapsed = started.elapsed();

        assert_eq!(diagnostics.counts.conversation_count, Some(200));
        assert_eq!(diagnostics.counts.task_count, Some(200));
        assert!(
            elapsed < std::time::Duration::from_millis(750),
            "controlled ProductDiagnostics read exceeded 750ms: {elapsed:?}"
        );
    }
}
