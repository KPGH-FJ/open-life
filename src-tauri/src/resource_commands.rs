//! Product IPC and process-local ownership for imported resources.
//!
//! The WebView never supplies filesystem paths. Rust opens the native picker,
//! reads only the paths returned by that picker, and hands bounded bytes to the
//! one ResourceGateway. Active imports are registered so one operation id has
//! exactly one parser/commit owner.

use openlife_core::resource::{
    ResourceDetachReceipt, ResourceImportReceipt, MAX_IMPORT_BYTES, MAX_RESOURCE_BYTES,
};
use openlife_core::resource_gateway::{
    ResourceGateway, ResourceImportCancellation, ResourceImportSource,
};
use serde::Serialize;
use std::collections::{hash_map::Entry, HashMap};
use std::fs::OpenOptions;
use std::io::{Read, Take};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Runtime;
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::AppState;

#[derive(Clone)]
pub(crate) struct ResourceRuntime {
    gateway: ResourceGateway,
    active_imports: Arc<Mutex<HashMap<String, ResourceImportCancellation>>>,
}

impl ResourceRuntime {
    pub(crate) fn new(gateway: ResourceGateway) -> Self {
        Self {
            gateway,
            active_imports: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn gateway(&self) -> &ResourceGateway {
        &self.gateway
    }

    fn begin_import(self: &Arc<Self>, operation_id: &str) -> Result<ActiveImport, String> {
        validate_uuid_v4("resource_import_operation_id", operation_id)?;
        let cancellation = ResourceImportCancellation::default();
        let mut active = self
            .active_imports
            .lock()
            .map_err(|_| "resource_import_registry_poisoned".to_string())?;
        match active.entry(operation_id.to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(cancellation.clone());
            }
            Entry::Occupied(_) => {
                return Err("resource_import_operation_already_active".into());
            }
        }
        Ok(ActiveImport {
            runtime: Arc::clone(self),
            operation_id: operation_id.to_string(),
            cancellation,
        })
    }
}

struct ActiveImport {
    runtime: Arc<ResourceRuntime>,
    operation_id: String,
    cancellation: ResourceImportCancellation,
}

impl Drop for ActiveImport {
    fn drop(&mut self) {
        if let Ok(mut active) = self.runtime.active_imports.lock() {
            active.remove(&self.operation_id);
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResourceImportSelectionResult {
    cancelled: bool,
    receipt: Option<ResourceImportReceipt>,
}

pub(crate) async fn pick_and_import_resources<R: Runtime>(
    import_operation_id: String,
    turn_operation_id: String,
    app_handle: tauri::AppHandle<R>,
    state: &Arc<AppState>,
) -> Result<ResourceImportSelectionResult, String> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ResourceStore"])
        .map_err(|error| error.to_string())?;
    validate_uuid_v4("resource_turn_operation_id", &turn_operation_id)?;
    let runtime = state
        .resource_runtime
        .as_ref()
        .cloned()
        .ok_or_else(|| "resource_runtime_unavailable".to_string())?;
    let active = runtime.begin_import(&import_operation_id)?;

    let (sender, receiver) = tokio::sync::oneshot::channel();
    app_handle
        .dialog()
        .file()
        .set_title("选择要交给 OpenLife 的文件")
        .add_filter(
            "OpenLife 支持的文件",
            &[
                "txt", "md", "markdown", "json", "rs", "ts", "tsx", "js", "jsx", "py", "go",
                "java", "c", "h", "cpp", "hpp", "swift", "kt", "kts", "toml", "yaml", "yml", "xml",
                "html", "css", "sql", "sh", "pdf", "docx", "csv", "xlsx",
            ],
        )
        .pick_files(move |paths| {
            let _ = sender.send(paths);
        });
    let selected = receiver
        .await
        .map_err(|_| "resource_native_picker_closed_without_result".to_string())?;
    let Some(selected) = selected.filter(|paths| !paths.is_empty()) else {
        return Ok(ResourceImportSelectionResult {
            cancelled: true,
            receipt: None,
        });
    };
    if active.cancellation.is_cancelled() {
        return Err("resource_import_cancelled_before_read".into());
    }
    let paths = selected
        .into_iter()
        .map(|file_path| {
            file_path
                .into_path()
                .map_err(|_| "resource_native_picker_path_invalid".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sources = tokio::task::spawn_blocking(move || read_selected_resources(paths))
        .await
        .map_err(|_| "resource_file_read_task_failed".to_string())??;
    if active.cancellation.is_cancelled() {
        return Err("resource_import_cancelled_after_read".into());
    }
    let receipt = runtime
        .gateway()
        .import_resources(
            import_operation_id,
            turn_operation_id,
            sources,
            active.cancellation.clone(),
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(ResourceImportSelectionResult {
        cancelled: false,
        receipt: Some(receipt),
    })
}

pub(crate) async fn detach_resource_from_turn(
    operation_id: String,
    turn_operation_id: String,
    resource_id: String,
    state: &Arc<AppState>,
) -> Result<ResourceDetachReceipt, String> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ResourceStore"])
        .map_err(|error| error.to_string())?;
    validate_uuid_v4("resource_detach_operation_id", &operation_id)?;
    validate_uuid_v4("resource_turn_operation_id", &turn_operation_id)?;
    validate_uuid_v4("resource_id", &resource_id)?;
    let gateway = state
        .resource_runtime
        .as_ref()
        .ok_or_else(|| "resource_runtime_unavailable".to_string())?
        .gateway()
        .clone();
    tokio::task::spawn_blocking(move || {
        gateway.detach_resource_from_message(&operation_id, &turn_operation_id, &resource_id)
    })
    .await
    .map_err(|_| "resource_detach_task_failed".to_string())?
    .map_err(|error| error.to_string())
}

fn read_selected_resources(paths: Vec<PathBuf>) -> Result<Vec<ResourceImportSource>, String> {
    if paths.is_empty() || paths.len() > openlife_core::resource::MAX_RESOURCES_PER_IMPORT {
        return Err("resource_import_file_count_exceeded".into());
    }
    let mut total_bytes = 0usize;
    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "resource_filename_invalid".to_string())?
            .to_string();
        let declared_mime = declared_mime_for_path(&path)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
        }
        let file = options
            .open(&path)
            .map_err(|_| "resource_selected_file_open_failed".to_string())?;
        let metadata = file
            .metadata()
            .map_err(|_| "resource_selected_file_metadata_failed".to_string())?;
        if !metadata.is_file() {
            return Err("resource_selected_path_not_regular_file".into());
        }
        if metadata.len() == 0 || metadata.len() > MAX_RESOURCE_BYTES as u64 {
            return Err("resource_file_bytes_exceeded".into());
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        let mut bounded: Take<_> = file.take((MAX_RESOURCE_BYTES + 1) as u64);
        bounded
            .read_to_end(&mut bytes)
            .map_err(|_| "resource_selected_file_read_failed".to_string())?;
        if bytes.is_empty() || bytes.len() > MAX_RESOURCE_BYTES {
            return Err("resource_file_bytes_exceeded".into());
        }
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| "resource_import_total_bytes_overflow".to_string())?;
        if total_bytes > MAX_IMPORT_BYTES {
            return Err("resource_import_total_bytes_exceeded".into());
        }
        sources.push(ResourceImportSource {
            filename,
            declared_mime: declared_mime.to_string(),
            bytes,
        });
    }
    Ok(sources)
}

fn declared_mime_for_path(path: &Path) -> Result<&'static str, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "resource_extension_missing".to_string())?;
    match extension.as_str() {
        "txt" => Ok("text/plain"),
        "md" | "markdown" => Ok("text/markdown"),
        "json" => Ok("application/json"),
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "c" | "h" | "cpp" | "hpp"
        | "swift" | "kt" | "kts" | "toml" | "yaml" | "yml" | "xml" | "html" | "css" | "sql"
        | "sh" => Ok("text/plain"),
        "pdf" => Ok("application/pdf"),
        "docx" => Ok("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "csv" => Ok("text/csv"),
        "xlsx" => Ok("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        _ => Err("resource_format_unsupported".into()),
    }
}

fn validate_uuid_v4(label: &str, value: &str) -> Result<(), String> {
    let parsed = Uuid::parse_str(value).map_err(|_| format!("{label}_invalid"))?;
    if parsed.get_version_num() != 4 || parsed.to_string() != value.to_ascii_lowercase() {
        return Err(format!("{label}_must_be_uuid_v4"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::resource::ResourceStore;
    use openlife_core::resource_gateway::ResourceParserProcess;

    fn runtime() -> Arc<ResourceRuntime> {
        Arc::new(ResourceRuntime::new(ResourceGateway::new(
            ResourceStore::new_in_memory().unwrap(),
            ResourceParserProcess::for_current_executable().unwrap(),
        )))
    }

    #[test]
    fn active_import_has_one_process_local_owner() {
        let runtime = runtime();
        let operation_id = Uuid::new_v4().to_string();
        let active = runtime.begin_import(&operation_id).unwrap();
        let duplicate = match runtime.begin_import(&operation_id) {
            Ok(_) => panic!("second import owner unexpectedly admitted"),
            Err(error) => error,
        };
        assert_eq!(duplicate, "resource_import_operation_already_active");
        drop(active);
        let next = runtime.begin_import(&operation_id).unwrap();
        drop(next);
    }

    #[test]
    fn selected_file_reader_accepts_supported_regular_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("roadshow.md");
        std::fs::write(&path, "# Roadshow\nEvidence").unwrap();
        let sources = read_selected_resources(vec![path]).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].filename, "roadshow.md");
        assert_eq!(sources[0].declared_mime, "text/markdown");
        assert_eq!(sources[0].bytes, b"# Roadshow\nEvidence");
    }

    #[test]
    fn resource_selector_has_no_parallel_semantic_index_or_model_route() {
        let source = include_str!("../../openlife-core/src/resource_selection.rs");
        for forbidden in [
            ["Vector", "Store"].concat(),
            ["vector", "_store"].concat(),
            ["embed", "ding"].concat(),
            ["Inference", "Scheduler"].concat(),
            ["prepare", "_chat_request"].concat(),
        ] {
            assert!(
                !source.contains(&forbidden),
                "deterministic Resource selector must not depend on {forbidden}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn selected_file_reader_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("private.md");
        let link = directory.path().join("selected.md");
        std::fs::write(&target, "private").unwrap();
        symlink(&target, &link).unwrap();
        assert_eq!(
            read_selected_resources(vec![link]).unwrap_err(),
            "resource_selected_file_open_failed"
        );
    }
}
