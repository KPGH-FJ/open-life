use crate::AppState;
use openlife_core::tool_permissions::ToolPermissionPolicy;
use std::sync::Arc;
use tauri::State;

const TOOL_PERMISSION_STORE: &str = "ToolPermissionStore";

#[tauri::command]
pub async fn revoke_tool_permission(
    permission_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    revoke_tool_permission_with_state(&permission_id, state.inner()).await
}

pub(crate) async fn revoke_tool_permission_with_state(
    permission_id: &str,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let permission_id = permission_id.trim();
    uuid::Uuid::parse_str(permission_id).map_err(|_| "tool_permission_id_invalid".to_string())?;
    state
        .persistence_coordinator
        .require_effects_for_stores(&[TOOL_PERMISSION_STORE])
        .map_err(|error| error.to_string())?;

    let store = state.tool_permission_store.lock().await;
    let record = store
        .list()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|record| record.id == permission_id)
        .ok_or_else(|| "tool_permission_not_found".to_string())?;
    let now = chrono::Utc::now();
    let active = record.consumed_at.is_none()
        && record
            .expires_at
            .map(|expires_at| expires_at > now)
            .unwrap_or(true);
    if !active || record.policy == ToolPermissionPolicy::AllowOnce {
        return Err("tool_permission_not_revocable".into());
    }
    if !store
        .revoke(permission_id)
        .map_err(|error| error.to_string())?
    {
        return Err("tool_permission_not_found".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reusable_permission_is_projected_revoked_and_absent_after_refresh() {
        let state = crate::test_utils::test_app_state();
        let permission = {
            let store = state.tool_permission_store.lock().await;
            store
                .grant(
                    "web.search",
                    "builtin",
                    "medium",
                    "network",
                    ToolPermissionPolicy::AllowUntilRevoked,
                    None,
                )
                .unwrap()
        };

        let before =
            crate::read_models::tool_permissions::get_tool_permission_view_model_with_state(&state)
                .await
                .unwrap();
        assert!(before
            .data
            .unwrap()
            .items
            .iter()
            .any(|item| item.id == permission.id && item.revocable));

        revoke_tool_permission_with_state(&permission.id, &state)
            .await
            .unwrap();

        let after =
            crate::read_models::tool_permissions::get_tool_permission_view_model_with_state(&state)
                .await
                .unwrap();
        assert!(!after
            .data
            .unwrap()
            .items
            .iter()
            .any(|item| item.id == permission.id));
    }

    #[tokio::test]
    async fn settings_cannot_revoke_one_time_review_authority() {
        let state = crate::test_utils::test_app_state();
        let permission = {
            let store = state.tool_permission_store.lock().await;
            store
                .grant(
                    "web.fetch",
                    "builtin",
                    "high",
                    "network",
                    ToolPermissionPolicy::AllowOnce,
                    None,
                )
                .unwrap()
        };

        assert_eq!(
            revoke_tool_permission_with_state(&permission.id, &state)
                .await
                .unwrap_err(),
            "tool_permission_not_revocable"
        );
        let store = state.tool_permission_store.lock().await;
        assert!(store
            .list()
            .unwrap()
            .iter()
            .any(|item| item.id == permission.id));
    }
}
