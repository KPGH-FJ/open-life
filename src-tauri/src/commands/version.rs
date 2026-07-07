use crate::errors::AppError;
use crate::life_model_materializer_guard::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::life_model_write_gateway;
use crate::AppState;
use openlife_core::versioning::LifeModelVersion;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GovernedSnapshotRestoreRequest {
    pub purpose: String,
    pub explicit_user_intent: bool,
    pub create_pre_change_snapshot: bool,
}

impl GovernedSnapshotRestoreRequest {
    #[cfg(test)]
    fn manual_restore() -> Self {
        Self {
            purpose: "manual_restore".into(),
            explicit_user_intent: true,
            create_pre_change_snapshot: true,
        }
    }

    fn is_valid(&self) -> bool {
        self.explicit_user_intent
            && self.create_pre_change_snapshot
            && matches!(self.purpose.as_str(), "manual_restore" | "migration")
    }
}

fn require_governed_snapshot_restore_request(
    governed_request: Option<&GovernedSnapshotRestoreRequest>,
) -> Result<&GovernedSnapshotRestoreRequest, AppError> {
    if let Some(request) = governed_request.filter(|request| request.is_valid()) {
        Ok(request)
    } else {
        Err(AppError::permission(
            "restore_snapshot requires an explicit governed restore request with purpose manual_restore or migration, explicitUserIntent=true, and createPreChangeSnapshot=true.",
        ))
    }
}

fn hash_json_value(value: &serde_json::Value) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn hash_life_model(model: &openlife_core::life_model::LifeModel) -> Result<String, AppError> {
    hash_json_value(&serde_json::to_value(model)?)
}

fn validate_snapshot_restore_response_is_metadata_safe(value: &serde_json::Value) -> bool {
    value.get("life_model").is_none()
        && value.get("model").is_none()
        && value.get("yaml_content").is_none()
        && value.get("snapshot").is_none()
        && value.get("raw_snapshot").is_none()
}

fn ensure_snapshot_restore_response_metadata_safe(
    value: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    if validate_snapshot_restore_response_is_metadata_safe(&value) {
        Ok(value)
    } else {
        Err(AppError::internal(
            "governed snapshot restore response contained raw payload fields",
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
    governed_request: Option<GovernedSnapshotRestoreRequest>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    restore_snapshot_with_state_gated(version, state.inner(), governed_request).await
}

#[cfg(test)]
async fn restore_snapshot_with_state(
    version: String,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    restore_snapshot_with_state_gated(version, state, None).await
}

#[cfg(test)]
async fn restore_snapshot_with_state_for_governed_manual_restore(
    version: String,
    state: &Arc<AppState>,
    governed_request: GovernedSnapshotRestoreRequest,
) -> Result<serde_json::Value, AppError> {
    restore_snapshot_with_state_gated(version, state, Some(governed_request)).await
}

async fn restore_snapshot_with_state_gated(
    version: String,
    state: &Arc<AppState>,
    governed_request: Option<GovernedSnapshotRestoreRequest>,
) -> Result<serde_json::Value, AppError> {
    let request = require_governed_snapshot_restore_request(governed_request.as_ref())?;
    restore_snapshot_governed_operation(version, state, request).await
}

async fn restore_snapshot_governed_operation(
    version: String,
    state: &Arc<AppState>,
    request: &GovernedSnapshotRestoreRequest,
) -> Result<serde_json::Value, AppError> {
    let current_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    let current_model_hash = hash_life_model(&current_model)?;
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
    let restored_model_hash = hash_life_model(&restored_model)?;
    life_model_write_gateway::restore_life_model_with_gateway(
        state,
        &restored_model,
        LifeModelMaterializerCallerContext::new(
            "snapshot_restore_governed_operation",
            LifeModelMaterializerCallerKind::GovernedRestoreImportOperation,
            LifeModelMaterializerCallerPurpose::GovernedRestoreImportOperation,
        ),
    )
    .await?;

    ensure_snapshot_restore_response_metadata_safe(serde_json::json!({
        "success": true,
        "legacy": false,
        "governed_operation": true,
        "operation_kind": "snapshot_restore",
        "operation_purpose": request.purpose,
        "warning": "snapshot restore ran as an explicit governed restore operation.",
        "metadata_safe": true,
        "contains_raw_content": false,
        "durable_lifemodel_write": durable_lifemodel_write,
        "restored_snapshot_version": version,
        "restored_model_version": restored_model_version,
        "current_model_hash": current_model_hash,
        "restored_model_hash": restored_model_hash,
        "pre_restore_snapshot_created": pre_restore_snapshot_version.is_some(),
        "pre_restore_snapshot_version": pre_restore_snapshot_version,
        "audit": {
            "source_kind": "snapshot_restore",
            "operation_purpose": request.purpose,
            "current_model_hash": current_model_hash,
            "restored_model_hash": restored_model_hash,
            "pre_change_snapshot_version": pre_restore_snapshot_version,
            "metadata_safe": true,
            "contains_raw_content": false,
        },
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
    async fn w93_restore_snapshot_without_governed_request_fails_closed() {
        let state = crate::test_utils::test_app_state();
        save_model_name(&state, W84_CURRENT_NAME_SECRET).await;
        let snapshot_version = snapshot_named_model(&state, W84_SNAPSHOT_NAME_SECRET).await;
        let before_snapshot_count = snapshot_count(&state).await;

        let err = restore_snapshot_with_state(snapshot_version, &state)
            .await
            .expect_err("snapshot restore must fail closed by default");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("restore_snapshot"));
        assert!(err.message().contains("governed restore request"));
        assert!(err.message().contains("explicitUserIntent=true"));
        assert_eq!(current_model_name(&state).await, W84_CURRENT_NAME_SECRET);
        assert_eq!(snapshot_count(&state).await, before_snapshot_count);
    }

    #[tokio::test]
    async fn w93_restore_snapshot_governed_request_allows_metadata_safe_restore() {
        let state = crate::test_utils::test_app_state();
        save_model_name(&state, W84_CURRENT_NAME_SECRET).await;
        let snapshot_version = snapshot_named_model(&state, W84_SNAPSHOT_NAME_SECRET).await;

        let result = restore_snapshot_with_state_for_governed_manual_restore(
            snapshot_version.clone(),
            &state,
            GovernedSnapshotRestoreRequest::manual_restore(),
        )
        .await
        .unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(result["legacy"], false);
        assert_eq!(result["governed_operation"], true);
        assert_eq!(result["operation_kind"], "snapshot_restore");
        assert_eq!(result["operation_purpose"], "manual_restore");
        assert_eq!(result["metadata_safe"], true);
        assert_eq!(result["contains_raw_content"], false);
        assert_eq!(result["durable_lifemodel_write"], true);
        assert_eq!(result["restored_snapshot_version"], snapshot_version);
        assert_eq!(result["pre_restore_snapshot_created"], true);
        assert!(result["pre_restore_snapshot_version"].is_string());
        assert!(result["current_model_hash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:")));
        assert!(result["restored_model_hash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:")));
        assert_eq!(result["audit"]["metadata_safe"], true);
        assert_eq!(result["audit"]["contains_raw_content"], false);
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
    async fn w93_restore_snapshot_invalid_governed_request_fails_closed() {
        let state = crate::test_utils::test_app_state();
        save_model_name(&state, W84_CURRENT_NAME_SECRET).await;
        let snapshot_version = snapshot_named_model(&state, W84_SNAPSHOT_NAME_SECRET).await;

        let err = restore_snapshot_with_state_gated(
            snapshot_version,
            &state,
            Some(GovernedSnapshotRestoreRequest {
                purpose: "normal_product".into(),
                explicit_user_intent: true,
                create_pre_change_snapshot: true,
            }),
        )
        .await
        .expect_err("invalid governed snapshot restore purpose must fail closed");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("manual_restore"));
        assert_eq!(current_model_name(&state).await, W84_CURRENT_NAME_SECRET);
    }
}
