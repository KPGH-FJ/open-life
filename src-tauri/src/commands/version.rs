use crate::errors::AppError;
use crate::AppState;
use openlife_core::versioning::LifeModelVersion;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotRestoreLegacyDirectApplyOverride {
    pub allow_legacy_direct_apply: bool,
    pub purpose: String,
}

impl SnapshotRestoreLegacyDirectApplyOverride {
    #[cfg(test)]
    fn allow_for_manual_restore() -> Self {
        Self {
            allow_legacy_direct_apply: true,
            purpose: "manual_restore".into(),
        }
    }

    fn is_valid_restore_override(&self) -> bool {
        self.allow_legacy_direct_apply
            && matches!(
                self.purpose.as_str(),
                "dev_migration" | "migration" | "manual_restore" | "test_migration"
            )
    }
}

fn require_snapshot_restore_legacy_direct_apply_override(
    restore_override: Option<&SnapshotRestoreLegacyDirectApplyOverride>,
) -> Result<(), AppError> {
    if restore_override
        .is_some_and(SnapshotRestoreLegacyDirectApplyOverride::is_valid_restore_override)
    {
        Ok(())
    } else {
        Err(AppError::permission(
            "restore_snapshot is a W84 snapshot restore legacy direct write path and requires an explicit dev/migration/manual restore override with purpose dev_migration, migration, manual_restore, or test_migration.",
        ))
    }
}

#[tauri::command]
pub async fn create_snapshot(
    tag: String,
    note: String,
    state: State<'_, Arc<AppState>>,
) -> Result<LifeModelVersion, AppError> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    let vm = state.version_manager.lock().await;
    vm.snapshot(&model, &tag, &note).map_err(AppError::from)
}

#[tauri::command]
pub async fn list_snapshots(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<LifeModelVersion>, AppError> {
    let vm = state.version_manager.lock().await;
    vm.list_versions().map_err(AppError::from)
}

#[tauri::command]
pub async fn restore_snapshot(
    version: String,
    restore_override: Option<SnapshotRestoreLegacyDirectApplyOverride>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    restore_snapshot_with_state_gated(version, state.inner(), restore_override).await
}

#[cfg(test)]
async fn restore_snapshot_with_state(
    version: String,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    restore_snapshot_with_state_gated(version, state, None).await
}

#[cfg(test)]
async fn restore_snapshot_with_state_for_manual_restore(
    version: String,
    state: &Arc<AppState>,
    restore_override: SnapshotRestoreLegacyDirectApplyOverride,
) -> Result<serde_json::Value, AppError> {
    restore_snapshot_with_state_gated(version, state, Some(restore_override)).await
}

async fn restore_snapshot_with_state_gated(
    version: String,
    state: &Arc<AppState>,
    restore_override: Option<SnapshotRestoreLegacyDirectApplyOverride>,
) -> Result<serde_json::Value, AppError> {
    require_snapshot_restore_legacy_direct_apply_override(restore_override.as_ref())?;
    restore_snapshot_direct_apply_after_gate(version, state).await
}

async fn restore_snapshot_direct_apply_after_gate(
    version: String,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    let current_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    let pre_restore_snapshot_version = {
        let vm = state.version_manager.lock().await;
        vm.snapshot(
            &current_model,
            "auto:pre-restore",
            &format!("回滚到 {} 之前自动备份", version),
        )
        .ok()
        .map(|snapshot| snapshot.version)
    };
    let restored_model = {
        let vm = state.version_manager.lock().await;
        vm.restore(&version).map_err(AppError::from)?
    };
    let durable_lifemodel_write = serde_json::to_value(&current_model).map_err(AppError::from)?
        != serde_json::to_value(&restored_model).map_err(AppError::from)?;
    let restored_model_version = restored_model.metadata.version.clone();
    {
        let manager = state.life_model_manager.lock().await;
        manager.save(&restored_model).map_err(AppError::from)?;
    }

    Ok(serde_json::json!({
        "success": true,
        "legacy": true,
        "warning": "snapshot restore legacy direct write path bypasses Review Center; use only for explicit migration/manual restore.",
        "metadata_safe": true,
        "durable_lifemodel_write": durable_lifemodel_write,
        "restored_snapshot_version": version,
        "restored_model_version": restored_model_version,
        "pre_restore_snapshot_created": pre_restore_snapshot_version.is_some(),
        "pre_restore_snapshot_version": pre_restore_snapshot_version,
    }))
}

#[tauri::command]
pub async fn diff_snapshots(
    v1: String,
    v2: String,
    state: State<'_, Arc<AppState>>,
) -> Result<String, AppError> {
    let vm = state.version_manager.lock().await;
    vm.diff(&v1, &v2).map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W84_CURRENT_NAME_SECRET: &str = "W84_RESTORE_CURRENT_LIFEMODEL_SECRET";
    const W84_SNAPSHOT_NAME_SECRET: &str = "W84_RESTORE_SNAPSHOT_LIFEMODEL_SECRET";

    async fn save_model_name(state: &Arc<AppState>, name: &str) {
        let manager = state.life_model_manager.lock().await;
        let mut model = manager.load().unwrap();
        model.identity.name = name.into();
        manager.save(&model).unwrap();
    }

    async fn current_model_name(state: &Arc<AppState>) -> String {
        state
            .life_model_manager
            .lock()
            .await
            .load()
            .unwrap()
            .identity
            .name
    }

    async fn snapshot_named_model(state: &Arc<AppState>, name: &str) -> String {
        let mut model = state.life_model_manager.lock().await.load().unwrap();
        model.identity.name = name.into();
        let vm = state.version_manager.lock().await;
        vm.snapshot(&model, "w84-restore-source", "W84 restore source snapshot")
            .unwrap()
            .version
    }

    async fn snapshot_count(state: &Arc<AppState>) -> usize {
        state
            .version_manager
            .lock()
            .await
            .list_versions()
            .unwrap()
            .len()
    }

    #[tokio::test]
    async fn w84_restore_snapshot_default_fails_closed_without_manual_restore_override() {
        let state = crate::test_utils::test_app_state();
        save_model_name(&state, W84_CURRENT_NAME_SECRET).await;
        let snapshot_version = snapshot_named_model(&state, W84_SNAPSHOT_NAME_SECRET).await;
        let before_snapshot_count = snapshot_count(&state).await;

        let err = restore_snapshot_with_state(snapshot_version, &state)
            .await
            .expect_err("snapshot restore must fail closed by default");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("restore_snapshot"));
        assert!(err.message().contains("W84"));
        assert!(err.message().contains("dev/migration/manual"));
        assert_eq!(current_model_name(&state).await, W84_CURRENT_NAME_SECRET);
        assert_eq!(snapshot_count(&state).await, before_snapshot_count);
    }

    #[tokio::test]
    async fn w84_restore_snapshot_manual_restore_override_allows_metadata_safe_restore() {
        let state = crate::test_utils::test_app_state();
        save_model_name(&state, W84_CURRENT_NAME_SECRET).await;
        let snapshot_version = snapshot_named_model(&state, W84_SNAPSHOT_NAME_SECRET).await;

        let result = restore_snapshot_with_state_for_manual_restore(
            snapshot_version.clone(),
            &state,
            SnapshotRestoreLegacyDirectApplyOverride::allow_for_manual_restore(),
        )
        .await
        .unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(result["legacy"], true);
        assert_eq!(result["metadata_safe"], true);
        assert_eq!(result["durable_lifemodel_write"], true);
        assert_eq!(result["restored_snapshot_version"], snapshot_version);
        assert_eq!(result["pre_restore_snapshot_created"], true);
        assert!(result.get("life_model").is_none());
        assert!(result.get("model").is_none());
        assert!(result.get("yaml_content").is_none());
        assert!(result.get("snapshot").is_none());
        assert!(result.get("raw_snapshot").is_none());

        let response_dump = result.to_string();
        for forbidden in [W84_CURRENT_NAME_SECRET, W84_SNAPSHOT_NAME_SECRET] {
            assert!(
                !response_dump.contains(forbidden),
                "snapshot restore response leaked raw marker {forbidden}"
            );
        }

        assert_eq!(current_model_name(&state).await, W84_SNAPSHOT_NAME_SECRET);
        assert!(
            snapshot_count(&state).await >= 2,
            "manual restore should create a metadata-reported pre-restore backup snapshot"
        );
    }

    #[tokio::test]
    async fn w84_restore_snapshot_invalid_override_fails_closed() {
        let state = crate::test_utils::test_app_state();
        save_model_name(&state, W84_CURRENT_NAME_SECRET).await;
        let snapshot_version = snapshot_named_model(&state, W84_SNAPSHOT_NAME_SECRET).await;

        let err = restore_snapshot_with_state_gated(
            snapshot_version,
            &state,
            Some(SnapshotRestoreLegacyDirectApplyOverride {
                allow_legacy_direct_apply: true,
                purpose: "normal_product".into(),
            }),
        )
        .await
        .expect_err("invalid snapshot restore override purpose must fail closed");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("manual_restore"));
        assert_eq!(current_model_name(&state).await, W84_CURRENT_NAME_SECRET);
    }
}
