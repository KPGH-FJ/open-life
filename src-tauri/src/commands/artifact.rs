use crate::{artifact_materializer::artifact_content_digest, AppState};
use openlife_core::atomic_file::write_atomic;
use openlife_core::task_runtime::CanonicalArtifactStatus;
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tauri::{Runtime, State};
use tauri_plugin_dialog::DialogExt;

pub(crate) async fn verified_artifact_path(
    state: &Arc<AppState>,
    artifact_id: &str,
    version: u64,
) -> Result<PathBuf, String> {
    if !artifact_id.starts_with("artifact:") || artifact_id.len() > 512 || version == 0 {
        return Err("artifact_reference_invalid".into());
    }
    state
        .persistence_coordinator
        .require_trusted_read("CanonicalTaskRuntimeStore")
        .map_err(|error| error.to_string())?;
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let (artifact, artifact_version, source_run_id) = {
        let store = store.lock().await;
        let artifact = store
            .load_artifact(artifact_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "artifact_not_found".to_string())?;
        let artifact_version = store
            .load_artifact_version(artifact_id, version)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "artifact_version_not_found".to_string())?;
        let task = store
            .load_task_snapshot(&artifact.task_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "artifact_task_not_found".to_string())?;
        let source_run_id = task
            .items
            .iter()
            .find(|item| item.id == artifact.source_item_id)
            .map(|item| item.run_id.clone())
            .ok_or_else(|| "artifact_source_run_missing".to_string())?;
        (artifact, artifact_version, source_run_id)
    };
    if artifact.current_version != version
        || artifact.status != CanonicalArtifactStatus::Materialized
        || artifact.content_digest != artifact_version.content_digest
        || artifact_version.observed_content_digest.as_deref()
            != Some(artifact.content_digest.as_str())
    {
        return Err("artifact_not_verified".into());
    }
    let reference = artifact
        .materialized_reference
        .as_deref()
        .filter(|reference| artifact_version.materialized_reference.as_deref() == Some(*reference))
        .ok_or_else(|| "artifact_materialized_reference_missing".to_string())?;
    let path = PathBuf::from(reference);
    let safe_paths = crate::canonical_work_runtime::artifact_materialized_safe_paths_for_task_run(
        state,
        &artifact.task_id,
        &source_run_id,
    )
    .await?;
    let metadata =
        std::fs::symlink_metadata(&path).map_err(|_| "artifact_file_unavailable".to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("artifact_file_type_invalid".into());
    }
    let canonical_path = path
        .canonicalize()
        .map_err(|_| "artifact_file_unavailable".to_string())?;
    let within_current_scope = safe_paths.iter().any(|safe| {
        PathBuf::from(safe)
            .canonicalize()
            .is_ok_and(|canonical_safe| canonical_path.starts_with(canonical_safe))
    });
    if !within_current_scope {
        return Err("artifact_path_outside_current_scope".into());
    }
    let bytes = std::fs::read(&path).map_err(|_| "artifact_file_unavailable".to_string())?;
    if artifact_content_digest(&bytes) != artifact.content_digest {
        return Err("artifact_file_changed".into());
    }
    Ok(path)
}

#[tauri::command]
pub async fn open_artifact_result(
    artifact_id: String,
    version: u64,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let path = verified_artifact_path(state.inner(), &artifact_id, version).await?;
    open::that(path).map_err(|_| "artifact_open_failed".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArtifactResult {
    pub cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<String>,
}

fn export_verified_artifact_to_path(source: &PathBuf, target: &PathBuf) -> Result<String, String> {
    let bytes = std::fs::read(source).map_err(|_| "artifact_file_unavailable".to_string())?;
    let digest = artifact_content_digest(&bytes);
    if source != target {
        write_atomic(target, &bytes).map_err(|_| "artifact_export_failed".to_string())?;
    }
    let observed = std::fs::read(target).map_err(|_| "artifact_export_unverified".to_string())?;
    if artifact_content_digest(&observed) != digest {
        return Err("artifact_export_digest_mismatch".into());
    }
    Ok(digest)
}

pub async fn export_artifact_result<R: Runtime>(
    app_handle: tauri::AppHandle<R>,
    state: &Arc<AppState>,
    artifact_id: &str,
    version: u64,
) -> Result<ExportArtifactResult, String> {
    let source = verified_artifact_path(state, artifact_id, version).await?;
    let suggested_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("openlife-result");
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app_handle
        .dialog()
        .file()
        .set_title("另存 OpenLife 结果")
        .set_file_name(suggested_name)
        .save_file(move |path| {
            let _ = sender.send(path);
        });
    let selected = receiver
        .await
        .map_err(|_| "artifact_export_picker_failed".to_string())?;
    let Some(selected) = selected else {
        return Ok(ExportArtifactResult {
            cancelled: true,
            saved_path: None,
            content_digest: None,
        });
    };
    let target = selected
        .into_path()
        .map_err(|_| "artifact_export_target_invalid".to_string())?;
    let digest = export_verified_artifact_to_path(&source, &target)?;
    Ok(ExportArtifactResult {
        cancelled: false,
        saved_path: Some(target.to_string_lossy().into_owned()),
        content_digest: Some(digest),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::task_runtime::{
        BeginDirectArtifactMaterializationInput, BeginGeneralTaskRunInput,
        BindArtifactVersionSourceInput, GeneralArtifactDraftInput,
    };

    #[tokio::test]
    async fn only_current_digest_verified_artifact_can_be_opened() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let store = state.canonical_task_runtime_store.as_ref().unwrap();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let bytes = b"verified artifact";
        let digest = artifact_content_digest(bytes);
        let instruction_digest = artifact_content_digest(b"instruction");
        let request_digest = artifact_content_digest(b"request");
        let managed_root =
            crate::artifact_materializer::managed_artifact_root(None, &conversation_id).unwrap();
        std::fs::create_dir_all(&managed_root).unwrap();
        let target = managed_root.join(format!("openlife-open-{}.md", uuid::Uuid::new_v4()));
        std::fs::write(&target, bytes).unwrap();
        let prepared = {
            let store = store.lock().await;
            store
                .begin_general_task_run(BeginGeneralTaskRunInput {
                    task_id: &task_id,
                    conversation_id: &conversation_id,
                    run_id: &run_id,
                    execution_session_id: &run_id,
                    instruction_digest: &instruction_digest,
                    plan_digest: None,
                    project_id: None,
                    project_revision: None,
                    scope_digest: None,
                    execution_mode: openlife_core::task_runtime::WorkExecutionMode::ScopedAgent,
                })
                .unwrap();
            store
                .prepare_general_artifact(GeneralArtifactDraftInput {
                    task_id: &task_id,
                    run_id: &run_id,
                    target_reference: &target.to_string_lossy(),
                    content_digest: &digest,
                    media_type: "text/markdown",
                })
                .unwrap()
        };
        {
            let store = store.lock().await;
            store
                .bind_general_artifact_version_source(BindArtifactVersionSourceInput {
                    artifact_id: &prepared.artifact_id,
                    version: prepared.version,
                    target_reference: &target.to_string_lossy(),
                    draft_reference: &target.to_string_lossy(),
                    expected_target_absent: true,
                    expected_target_digest: None,
                    pre_change_snapshot: None,
                })
                .unwrap();
            let effect_id = format!("direct:{}", uuid::Uuid::new_v4());
            let attempt_id = uuid::Uuid::new_v4().to_string();
            store
                .begin_direct_artifact_materialization(BeginDirectArtifactMaterializationInput {
                    artifact_id: &prepared.artifact_id,
                    version: prepared.version,
                    effect_id: &effect_id,
                    attempt_id: &attempt_id,
                    request_digest: &request_digest,
                    byte_size: bytes.len() as u64,
                    media_type: "text/markdown",
                })
                .unwrap();
            store.mark_direct_artifact_staged(&effect_id).unwrap();
            store
                .confirm_direct_artifact_materialized(
                    &effect_id,
                    &target.to_string_lossy(),
                    &digest,
                )
                .unwrap();
        }
        assert_eq!(
            verified_artifact_path(&state, &prepared.artifact_id, prepared.version)
                .await
                .unwrap(),
            target
        );
        std::fs::write(&target, b"changed").unwrap();
        assert_eq!(
            verified_artifact_path(&state, &prepared.artifact_id, prepared.version)
                .await
                .unwrap_err(),
            "artifact_file_changed"
        );
        std::fs::remove_dir_all(managed_root).unwrap();
    }

    #[test]
    fn explicit_export_writes_and_verifies_an_exact_copy() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("managed.md");
        let target = directory.path().join("saved.md");
        std::fs::write(&source, b"# Managed result").unwrap();

        let digest = export_verified_artifact_to_path(&source, &target).unwrap();

        assert_eq!(digest, artifact_content_digest(b"# Managed result"));
        assert_eq!(std::fs::read(target).unwrap(), b"# Managed result");
    }
}
