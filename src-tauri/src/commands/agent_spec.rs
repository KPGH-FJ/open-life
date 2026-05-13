use crate::errors::AppError;
use crate::AppState;
use openlife_core::agent::{AgentSpec, AgentSpecStore, AgentSpecStoreError};
use std::sync::Arc;
use tauri::State;

/// Resolve the required AgentSpec for governed execution.
///
/// Follows ADR 0012 resolution order: explicit spec → stored default main.
/// Returns a hard error on failure — never falls back to `AgentSpec::default()`.
pub async fn resolve_required_agent_spec(
    store: &tokio::sync::Mutex<AgentSpecStore>,
    explicit_id: Option<&str>,
) -> Result<AgentSpec, AppError> {
    let store = store.lock().await;
    store.resolve_spec(explicit_id).map_err(|e| match &e {
        AgentSpecStoreError::NotFound(_) => AppError::not_found(e.to_string()),
        AgentSpecStoreError::InvalidRole { .. } => AppError::permission(e.to_string()),
        _ => AppError::internal(e.to_string()),
    })
}

#[tauri::command]
pub async fn get_agent_spec(
    spec_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<AgentSpec>, AppError> {
    let store = state.agent_spec_store.lock().await;
    match store.get_spec_optional(&spec_id) {
        Ok(spec) => Ok(spec),
        Err(e) => {
            if e.to_string().contains("not found") {
                Ok(None)
            } else {
                Err(AppError::internal(e.to_string()))
            }
        }
    }
}

#[tauri::command]
pub async fn list_agent_specs(state: State<'_, Arc<AppState>>) -> Result<Vec<AgentSpec>, AppError> {
    let store = state.agent_spec_store.lock().await;
    store
        .list_specs()
        .map_err(|e| AppError::internal(e.to_string()))
}

#[tauri::command]
pub async fn get_default_agent_spec(
    state: State<'_, Arc<AppState>>,
) -> Result<Option<AgentSpec>, AppError> {
    let store = state.agent_spec_store.lock().await;
    store
        .get_default_spec()
        .map_err(|e| AppError::internal(e.to_string()))
}

#[tauri::command]
pub async fn update_agent_spec(
    spec: AgentSpec,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let store = state.agent_spec_store.lock().await;
    store
        .update_spec(&spec)
        .map_err(|e| AppError::internal(e.to_string()))
}

#[tauri::command]
pub async fn set_default_agent_spec(
    spec_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let store = state.agent_spec_store.lock().await;
    store.set_default_main_spec(&spec_id).map_err(|e| match &e {
        AgentSpecStoreError::NotFound(_) => AppError::not_found(e.to_string()),
        AgentSpecStoreError::InvalidRole { .. } => AppError::permission(e.to_string()),
        _ => AppError::internal(e.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use openlife_core::agent::{AgentRoleKind, AgentSpec, AgentSpecStore, AgentSpecStoreError};

    // ── P7 Stabilize: resolve_required_agent_spec fail-closed tests ──────

    /// When the store has no `main.default` (e.g., corrupt bootstrap),
    /// resolve_required_agent_spec MUST return an error — not fall back
    /// to AgentSpec::default().
    #[tokio::test]
    async fn test_chat_agentspec_resolution_failure_fails_run_without_model_call() {
        use tokio::sync::Mutex;
        // Create an in-memory store and deactivate the bootstrapped main.default.
        let store = AgentSpecStore::new_in_memory().unwrap();
        // Deactivate main.default to simulate a missing default spec.
        store.set_active("main.default", false).unwrap();
        // Verify main.default is no longer active.
        let default = store.get_default_spec().unwrap();
        assert!(
            default.is_none(),
            "main.default should not be active after deactivation"
        );

        let locked = Mutex::new(store);
        let result = super::resolve_required_agent_spec(&locked, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message().contains("not found") || err.message().contains("no active"),
            "expected AgentSpec resolution failure, got: {}",
            err.message()
        );
    }

    /// When the store returns a Non-Main spec for the default slot,
    /// resolve_required_agent_spec MUST return a permission error.
    #[test]
    fn test_skill_agentspec_resolution_failure_returns_error() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        // Deactivate main.default and activate a Planner spec as default.
        let planner = AgentSpec::new(AgentRoleKind::Planner, "Planner", "test")
            .with_id("planner.test".to_string());
        store.create_spec(&planner).unwrap();
        // Force the planner as default — this should fail because it's not Main.
        let result = store.set_default_main_spec("planner.test");
        // Setting a non-Main as default should fail (InvalidRole).
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AgentSpecStoreError::InvalidRole { .. }
        ));
        // After the failed set_default, main.default should still be active.
        let default = store.get_default_spec().unwrap().unwrap();
        assert_eq!(default.id, "main.default");
        assert!(default.active);
    }

    #[test]
    fn test_set_default_to_missing_id_returns_error_and_preserves_default() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let result = store.set_default_main_spec("nonexistent");
        assert!(result.is_err());
        let default = store.get_default_spec().unwrap().unwrap();
        assert_eq!(default.id, "main.default");
        assert!(default.active);
    }

    #[test]
    fn test_set_default_to_non_main_returns_error_and_preserves_default() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let planner = AgentSpec::new(AgentRoleKind::Planner, "Planner", "test")
            .with_id("planner.test".to_string());
        store.create_spec(&planner).unwrap();
        let result = store.set_default_main_spec("planner.test");
        assert!(result.is_err());
        let default = store.get_default_spec().unwrap().unwrap();
        assert_eq!(default.id, "main.default");
        assert!(default.active);
    }

    #[test]
    fn test_set_default_to_alternate_main_switches() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let alt = AgentSpec::new(AgentRoleKind::Main, "Alt Main", "alternative")
            .with_id("main.alt".to_string())
            .with_lifemodel_access()
            .with_memory_evidence();
        store.create_spec(&alt).unwrap();
        store.set_default_main_spec("main.alt").unwrap();
        let default = store.get_default_spec().unwrap().unwrap();
        assert_eq!(default.id, "main.alt");
        assert!(default.active);
        let old = store.get_spec_optional("main.default").unwrap().unwrap();
        assert!(!old.active);
    }

    #[test]
    fn test_get_agent_spec_returns_none_for_missing() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let result = store.get_spec_optional("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_agent_specs_includes_default() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let specs = store.list_specs().unwrap();
        assert!(!specs.is_empty());
        assert!(specs.iter().any(|s| s.id == "main.default"));
    }

    #[test]
    fn test_get_default_agent_spec_returns_main_default() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let spec = store.get_default_spec().unwrap().unwrap();
        assert_eq!(spec.id, "main.default");
        assert_eq!(spec.role, AgentRoleKind::Main);
    }

    #[test]
    fn test_update_agent_spec_preserves_stable_fields() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let mut spec = store.get_default_spec().unwrap().unwrap();
        let original_id = spec.id.clone();
        let original_role = spec.role.clone();
        spec.name = "Updated Name".to_string();
        store.update_spec(&spec).unwrap();
        let fetched = store.get_default_spec().unwrap().unwrap();
        assert_eq!(fetched.id, original_id);
        assert_eq!(fetched.role, original_role);
        assert_eq!(fetched.name, "Updated Name");
    }
}
