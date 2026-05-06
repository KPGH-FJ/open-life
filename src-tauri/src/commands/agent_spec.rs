use crate::errors::AppError;
use crate::AppState;
use openlife_core::agent::{AgentSpec, AgentSpecStoreError};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_agent_spec(
    spec_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<AgentSpec>, AppError> {
    let store = state.agent_spec_store.lock().map_err(|e| AppError::internal(format!("{}", e)))?;
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
pub async fn list_agent_specs(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentSpec>, AppError> {
    let store = state.agent_spec_store.lock().map_err(|e| AppError::internal(format!("{}", e)))?;
    store.list_specs().map_err(|e| AppError::internal(e.to_string()))
}

#[tauri::command]
pub async fn get_default_agent_spec(
    state: State<'_, Arc<AppState>>,
) -> Result<Option<AgentSpec>, AppError> {
    let store = state.agent_spec_store.lock().map_err(|e| AppError::internal(format!("{}", e)))?;
    store.get_default_spec().map_err(|e| AppError::internal(e.to_string()))
}

#[tauri::command]
pub async fn update_agent_spec(
    spec: AgentSpec,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let store = state.agent_spec_store.lock().map_err(|e| AppError::internal(format!("{}", e)))?;
    store.update_spec(&spec).map_err(|e| AppError::internal(e.to_string()))
}

#[tauri::command]
pub async fn set_default_agent_spec(
    spec_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let store = state.agent_spec_store.lock().map_err(|e| AppError::internal(format!("{}", e)))?;
    store.set_default_main_spec(&spec_id).map_err(|e| match &e {
        AgentSpecStoreError::NotFound(_) => AppError::not_found(e.to_string()),
        AgentSpecStoreError::InvalidRole { .. } => AppError::permission(e.to_string()),
        _ => AppError::internal(e.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use openlife_core::agent::{AgentRoleKind, AgentSpec, AgentSpecStore};

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
