use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tauri::State;

use crate::artifact_materializer::{
    capture_artifact_target_precondition, prepare_artifact_move, ArtifactTargetPrecondition,
};
use crate::AppState;

pub(crate) const MARKDOWN_MEMORY_ENTRY_FILE: &str = "MEMORY.md";
pub(crate) const MARKDOWN_MEMORY_TOPIC_DIRECTORY: &str = "memories";
pub(crate) const MAX_MARKDOWN_MEMORY_FILES_PER_SCOPE: usize = 16;
pub(crate) const MAX_MARKDOWN_MEMORY_FILE_CHARS: usize = 32 * 1024;
pub(crate) const MAX_MARKDOWN_MEMORY_VIEW_CHARS: usize = 64 * 1024;
pub(crate) const MAX_MARKDOWN_MEMORY_CONTEXT_CHARS: usize = 4_800;
pub(crate) const MAX_MARKDOWN_MEMORY_CONTEXT_FILES: usize = 4;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MarkdownMemoryScope {
    Workspace,
    Project,
}

impl MarkdownMemoryScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Project => "project",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MarkdownMemoryRootView {
    pub scope: MarkdownMemoryScope,
    pub configured: bool,
    pub root_path: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MarkdownMemoryFileView {
    pub scope: MarkdownMemoryScope,
    pub relative_path: String,
    pub content: String,
    pub content_digest: String,
    pub char_count: usize,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MarkdownMemoryViewModel {
    pub roots: Vec<MarkdownMemoryRootView>,
    pub files: Vec<MarkdownMemoryFileView>,
    pub total_char_count: usize,
    pub truncated: bool,
    pub source_rule: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DraftMarkdownMemoryFileRequest {
    pub scope: MarkdownMemoryScope,
    pub relative_path: String,
    pub content: String,
    #[serde(default)]
    pub expected_current_digest: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DeactivateMarkdownMemoryFileRequest {
    pub scope: MarkdownMemoryScope,
    pub relative_path: String,
    pub expected_current_digest: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MarkdownMemoryProposalReceipt {
    pub proposal_id: String,
    pub scope: MarkdownMemoryScope,
    pub relative_path: String,
    pub operation: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub(crate) struct MarkdownMemoryRoot {
    pub(crate) scope: MarkdownMemoryScope,
    pub(crate) path: PathBuf,
}

fn root_from_config(
    scope: MarkdownMemoryScope,
    workspace_root: Option<&str>,
    project_root: Option<&str>,
) -> Option<MarkdownMemoryRoot> {
    let raw = match scope {
        MarkdownMemoryScope::Workspace => workspace_root,
        MarkdownMemoryScope::Project => project_root,
    }?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    let canonical = path.canonicalize().ok()?;
    canonical.is_dir().then_some(MarkdownMemoryRoot {
        scope,
        path: canonical,
    })
}

pub(crate) fn configured_markdown_memory_roots(
    workspace_root: Option<&str>,
    project_root: Option<&str>,
) -> Vec<MarkdownMemoryRoot> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    for scope in [MarkdownMemoryScope::Workspace, MarkdownMemoryScope::Project] {
        let Some(root) = root_from_config(scope, workspace_root, project_root) else {
            continue;
        };
        if seen.insert(root.path.clone()) {
            roots.push(root);
        }
    }
    roots
}

fn is_disabled_memory_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".disabled.md"))
}

fn is_safe_topic_filename(name: &str) -> bool {
    name.ends_with(".md")
        && !name.ends_with(".disabled.md")
        && name.len() <= 128
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

pub(crate) fn validate_markdown_memory_relative_path(raw: &str) -> Result<PathBuf, String> {
    let path = Path::new(raw.trim());
    if path.is_absolute() || path.as_os_str().is_empty() {
        return Err("markdown memory path must be a non-empty relative path".into());
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("markdown memory path traversal is blocked".into());
    }
    let normalized = path.to_string_lossy().replace('\\', "/");
    if normalized == MARKDOWN_MEMORY_ENTRY_FILE {
        return Ok(PathBuf::from(normalized));
    }
    let mut parts = normalized.split('/');
    let directory = parts.next();
    let filename = parts.next();
    if directory == Some(MARKDOWN_MEMORY_TOPIC_DIRECTORY)
        && filename.is_some_and(is_safe_topic_filename)
        && parts.next().is_none()
    {
        return Ok(PathBuf::from(normalized));
    }
    Err("markdown memory files are limited to MEMORY.md or memories/<topic>.md".into())
}

fn content_digest(content: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(content.as_bytes()))
}

fn read_memory_file(root: &MarkdownMemoryRoot, relative: &Path) -> Option<MarkdownMemoryFileView> {
    if is_disabled_memory_file(relative) {
        return None;
    }
    let candidate = root.path.join(relative);
    let canonical = candidate.canonicalize().ok()?;
    if !canonical.starts_with(&root.path) || !canonical.is_file() {
        return None;
    }
    let content = std::fs::read_to_string(canonical).ok()?;
    let char_count = content.chars().count();
    if char_count > MAX_MARKDOWN_MEMORY_FILE_CHARS {
        return None;
    }
    Some(MarkdownMemoryFileView {
        scope: root.scope,
        relative_path: relative.to_string_lossy().replace('\\', "/"),
        content_digest: content_digest(&content),
        content,
        char_count,
        active: true,
    })
}

pub(crate) fn load_markdown_memory_files(
    roots: &[MarkdownMemoryRoot],
) -> (Vec<MarkdownMemoryFileView>, bool) {
    let mut files = Vec::new();
    let mut total_chars = 0usize;
    let mut truncated = false;
    for root in roots {
        let mut relative_paths = vec![PathBuf::from(MARKDOWN_MEMORY_ENTRY_FILE)];
        let topic_dir = root.path.join(MARKDOWN_MEMORY_TOPIC_DIRECTORY);
        if let Ok(entries) = std::fs::read_dir(topic_dir) {
            let mut topics = entries
                .flatten()
                .filter_map(|entry| {
                    let file_type = entry.file_type().ok()?;
                    if !file_type.is_file() || file_type.is_symlink() {
                        return None;
                    }
                    let name = entry.file_name().into_string().ok()?;
                    is_safe_topic_filename(&name)
                        .then(|| PathBuf::from(MARKDOWN_MEMORY_TOPIC_DIRECTORY).join(name))
                })
                .collect::<Vec<_>>();
            topics.sort();
            if topics.len() + 1 > MAX_MARKDOWN_MEMORY_FILES_PER_SCOPE {
                truncated = true;
            }
            relative_paths.extend(topics);
        }
        for relative in relative_paths
            .into_iter()
            .take(MAX_MARKDOWN_MEMORY_FILES_PER_SCOPE)
        {
            let candidate_exists = root.path.join(&relative).symlink_metadata().is_ok();
            let Some(file) = read_memory_file(root, &relative) else {
                if candidate_exists {
                    truncated = true;
                }
                continue;
            };
            if total_chars.saturating_add(file.char_count) > MAX_MARKDOWN_MEMORY_VIEW_CHARS {
                truncated = true;
                break;
            }
            total_chars += file.char_count;
            files.push(file);
        }
    }
    (files, truncated)
}

fn configured_root_for_scope(
    scope: MarkdownMemoryScope,
    workspace_root: Option<&str>,
    project_root: Option<&str>,
) -> Result<MarkdownMemoryRoot, String> {
    root_from_config(scope, workspace_root, project_root).ok_or_else(|| {
        format!(
            "{} markdown memory root is not configured or unavailable",
            scope.as_str()
        )
    })
}

fn resolve_target(root: &MarkdownMemoryRoot, relative: &str) -> Result<(PathBuf, String), String> {
    let relative = validate_markdown_memory_relative_path(relative)?;
    let target = root.path.join(&relative);
    let parent = target
        .parent()
        .ok_or_else(|| "markdown memory target parent is unavailable".to_string())?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| format!("markdown memory target parent is unavailable: {error}"))?;
    if !canonical_parent.starts_with(&root.path) {
        return Err("markdown memory target escapes its configured root".into());
    }
    Ok((target, relative.to_string_lossy().replace('\\', "/")))
}

fn ensure_expected_digest(
    root: &MarkdownMemoryRoot,
    target: &Path,
    expected: Option<&str>,
    allow_absent: bool,
) -> Result<ArtifactTargetPrecondition, String> {
    let precondition = capture_artifact_target_precondition(
        &target.to_string_lossy(),
        &[root.path.to_string_lossy().into_owned()],
    )?;
    match (&precondition, expected) {
        (ArtifactTargetPrecondition::Absent, None) if allow_absent => Ok(precondition),
        (ArtifactTargetPrecondition::Absent, _) => {
            Err("markdown memory file is absent or the editor snapshot is stale".into())
        }
        (ArtifactTargetPrecondition::ContentDigest(actual), Some(expected))
            if actual == expected =>
        {
            Ok(precondition)
        }
        (ArtifactTargetPrecondition::ContentDigest(_), None) if allow_absent => {
            Err("markdown memory file already exists; reload before editing".into())
        }
        (ArtifactTargetPrecondition::ContentDigest(_), _) => {
            Err("markdown memory file changed since it was loaded".into())
        }
    }
}

async fn create_proposal(state: &Arc<AppState>, proposal: &AgentProposal) -> Result<(), String> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| error.to_string())?;
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "ProposalStore unavailable".to_string())?
        .lock()
        .await;
    store
        .create_proposal(proposal)
        .map_err(|error| format!("markdown memory proposal creation failed: {error}"))
}

pub(crate) async fn get_markdown_memory_view_model_with_state(
    state: &Arc<AppState>,
) -> Result<MarkdownMemoryViewModel, String> {
    state
        .persistence_coordinator
        .require_trusted_read("ConfigStore")
        .map_err(|error| error.to_string())?;
    let (workspace_root, project_root) = {
        let config = state.config.lock().await;
        (
            config.system.workspace_memory_root.clone(),
            config.system.project_memory_root.clone(),
        )
    };
    let roots =
        configured_markdown_memory_roots(workspace_root.as_deref(), project_root.as_deref());
    let (files, truncated) = load_markdown_memory_files(&roots);
    let total_char_count = files.iter().map(|file| file.char_count).sum();
    let root_views = [MarkdownMemoryScope::Workspace, MarkdownMemoryScope::Project]
        .into_iter()
        .map(|scope| {
            let raw = match scope {
                MarkdownMemoryScope::Workspace => workspace_root.as_deref(),
                MarkdownMemoryScope::Project => project_root.as_deref(),
            };
            let resolved =
                root_from_config(scope, workspace_root.as_deref(), project_root.as_deref());
            MarkdownMemoryRootView {
                scope,
                configured: raw.is_some(),
                root_path: resolved
                    .as_ref()
                    .map(|root| root.path.to_string_lossy().into_owned())
                    .or_else(|| raw.map(str::to_string)),
                status: if resolved.is_some() {
                    "ready".into()
                } else if raw.is_some() {
                    "unavailable".into()
                } else {
                    "unconfigured".into()
                },
            }
        })
        .collect();
    Ok(MarkdownMemoryViewModel {
        roots: root_views,
        files,
        total_char_count,
        truncated,
        source_rule: "Only MEMORY.md and memories/*.md from the explicitly selected Workspace/Project roots are readable; disabled files and other roots are excluded.".into(),
    })
}

#[tauri::command]
pub(crate) async fn get_markdown_memory_view_model(
    state: State<'_, Arc<AppState>>,
) -> Result<MarkdownMemoryViewModel, String> {
    get_markdown_memory_view_model_with_state(state.inner()).await
}

#[tauri::command]
pub(crate) async fn draft_markdown_memory_file_proposal(
    request: DraftMarkdownMemoryFileRequest,
    state: State<'_, Arc<AppState>>,
) -> Result<MarkdownMemoryProposalReceipt, String> {
    draft_markdown_memory_file_proposal_with_state(request, state.inner()).await
}

pub(crate) async fn draft_markdown_memory_file_proposal_with_state(
    request: DraftMarkdownMemoryFileRequest,
    state: &Arc<AppState>,
) -> Result<MarkdownMemoryProposalReceipt, String> {
    let (workspace_root, project_root) = {
        let config = state.config.lock().await;
        (
            config.system.workspace_memory_root.clone(),
            config.system.project_memory_root.clone(),
        )
    };
    let root = configured_root_for_scope(
        request.scope,
        workspace_root.as_deref(),
        project_root.as_deref(),
    )?;
    if request.content.chars().count() > MAX_MARKDOWN_MEMORY_FILE_CHARS {
        return Err("markdown memory content exceeds the per-file limit".into());
    }
    let (target, relative_path) = resolve_target(&root, &request.relative_path)?;
    let precondition = ensure_expected_digest(
        &root,
        &target,
        request.expected_current_digest.as_deref(),
        true,
    )?;
    let (expected_target_absent, expected_target_digest) = match precondition {
        ArtifactTargetPrecondition::Absent => (true, None),
        ArtifactTargetPrecondition::ContentDigest(digest) => (false, Some(digest)),
    };
    let affected_path = format!("filesystem.{}", target.to_string_lossy());
    let reason = format!(
        "User requested a reviewed {} Markdown working-memory change.",
        request.scope.as_str()
    );
    let mut proposal = AgentProposal::new(
        ProposalType::ExternalWriteAction,
        &affected_path,
        serde_json::json!({
            "path": target.to_string_lossy(),
            "content": request.content,
            "contentDigest": content_digest(&request.content),
            "expected_target_absent": expected_target_absent,
            "expected_target_digest": expected_target_digest,
            "encoding": "utf-8",
            "operation": "propose_write",
            "memoryScope": request.scope,
            "memoryRelativePath": relative_path,
            "source": "markdown_memory_editor",
            "directFileWrite": false,
            "fileWritten": false,
            "externalWritesExecuted": false,
            "directWritesExecuted": false,
        }),
        &reason,
        1.0,
        RiskLevel::High,
        ProposalSource::Manual,
    );
    proposal.id = format!("proposal:markdown-memory:{}", uuid::Uuid::new_v4());
    create_proposal(state, &proposal).await?;
    Ok(MarkdownMemoryProposalReceipt {
        proposal_id: proposal.id,
        scope: request.scope,
        relative_path,
        operation: "write".into(),
        status: "review_required".into(),
    })
}

#[tauri::command]
pub(crate) async fn deactivate_markdown_memory_file_proposal(
    request: DeactivateMarkdownMemoryFileRequest,
    state: State<'_, Arc<AppState>>,
) -> Result<MarkdownMemoryProposalReceipt, String> {
    deactivate_markdown_memory_file_proposal_with_state(request, state.inner()).await
}

pub(crate) async fn deactivate_markdown_memory_file_proposal_with_state(
    request: DeactivateMarkdownMemoryFileRequest,
    state: &Arc<AppState>,
) -> Result<MarkdownMemoryProposalReceipt, String> {
    let (workspace_root, project_root) = {
        let config = state.config.lock().await;
        (
            config.system.workspace_memory_root.clone(),
            config.system.project_memory_root.clone(),
        )
    };
    let root = configured_root_for_scope(
        request.scope,
        workspace_root.as_deref(),
        project_root.as_deref(),
    )?;
    let (source, relative_path) = resolve_target(&root, &request.relative_path)?;
    let current = ensure_expected_digest(
        &root,
        &source,
        Some(&request.expected_current_digest),
        false,
    )?;
    let source_digest = match current {
        ArtifactTargetPrecondition::ContentDigest(digest) => digest,
        ArtifactTargetPrecondition::Absent => unreachable!("absence rejected above"),
    };
    let filename = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| "markdown memory filename is invalid".to_string())?;
    let target = source.with_file_name(format!("{filename}.disabled.md"));
    let safe_paths = vec![root.path.to_string_lossy().into_owned()];
    let prepared = prepare_artifact_move(
        "proposal-preview",
        &source.to_string_lossy(),
        &target.to_string_lossy(),
        &source_digest,
        &safe_paths,
    )?;
    let affected_path = format!(
        "filesystem.{}->{}",
        prepared.source_path.to_string_lossy(),
        prepared.target_path.to_string_lossy()
    );
    let reason = format!(
        "User requested deactivation of one reviewed {} Markdown working-memory file.",
        request.scope.as_str()
    );
    let mut proposal = AgentProposal::new(
        ProposalType::ExternalWriteAction,
        &affected_path,
        serde_json::json!({
            "operation": "move",
            "source_path": prepared.source_path,
            "target_path": prepared.target_path,
            "source_digest": prepared.content_digest,
            "size_bytes": prepared.byte_size,
            "memoryScope": request.scope,
            "memoryRelativePath": relative_path,
            "source": "markdown_memory_editor",
            "directFileWrite": false,
            "fileWritten": false,
            "externalWritesExecuted": false,
            "directWritesExecuted": false,
        }),
        &reason,
        1.0,
        RiskLevel::High,
        ProposalSource::Manual,
    );
    proposal.id = format!("proposal:markdown-memory:{}", uuid::Uuid::new_v4());
    create_proposal(state, &proposal).await?;
    Ok(MarkdownMemoryProposalReceipt {
        proposal_id: proposal.id,
        scope: request.scope,
        relative_path,
        operation: "deactivate".into(),
        status: "review_required".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_contract_rejects_escape_nested_and_disabled_targets() {
        for rejected in [
            "../MEMORY.md",
            "/tmp/MEMORY.md",
            "notes.md",
            "memories/nested/topic.md",
            "memories/topic.disabled.md",
        ] {
            assert!(
                validate_markdown_memory_relative_path(rejected).is_err(),
                "unexpectedly allowed {rejected}"
            );
        }
        assert_eq!(
            validate_markdown_memory_relative_path("MEMORY.md").unwrap(),
            PathBuf::from("MEMORY.md")
        );
        assert_eq!(
            validate_markdown_memory_relative_path("memories/release-notes.md").unwrap(),
            PathBuf::from("memories/release-notes.md")
        );
    }

    #[test]
    fn roots_and_files_are_scope_isolated_and_disabled_files_stay_out() {
        let workspace = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(workspace.path().join("memories")).unwrap();
        std::fs::create_dir_all(project.path().join("memories")).unwrap();
        std::fs::write(workspace.path().join("MEMORY.md"), "workspace-only").unwrap();
        std::fs::write(project.path().join("MEMORY.md"), "project-only").unwrap();
        std::fs::write(
            project.path().join("memories/retired.disabled.md"),
            "must-not-load",
        )
        .unwrap();

        let roots =
            configured_markdown_memory_roots(workspace.path().to_str(), project.path().to_str());
        let (files, truncated) = load_markdown_memory_files(&roots);

        assert!(!truncated);
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|file| {
            file.scope == MarkdownMemoryScope::Workspace && file.content == "workspace-only"
        }));
        assert!(files.iter().any(|file| {
            file.scope == MarkdownMemoryScope::Project && file.content == "project-only"
        }));
        assert!(!files
            .iter()
            .any(|file| file.content.contains("must-not-load")));
    }

    #[test]
    fn same_physical_root_is_loaded_once_without_scope_duplication() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("MEMORY.md"), "single-owner").unwrap();
        let roots = configured_markdown_memory_roots(root.path().to_str(), root.path().to_str());
        let (files, _) = load_markdown_memory_files(&roots);

        assert_eq!(roots.len(), 1);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].scope, MarkdownMemoryScope::Workspace);
    }

    #[test]
    fn oversized_files_do_not_enter_the_read_model() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("MEMORY.md"),
            "x".repeat(MAX_MARKDOWN_MEMORY_FILE_CHARS + 1),
        )
        .unwrap();
        let roots = configured_markdown_memory_roots(root.path().to_str(), None);
        let (files, _) = load_markdown_memory_files(&roots);

        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn editor_creates_review_proposal_without_writing_the_file() {
        let root = tempfile::tempdir().unwrap();
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let canonical = root.path().canonicalize().unwrap();
        {
            let mut config = state.config.lock().await;
            config.system.project_memory_root = Some(canonical.to_string_lossy().into_owned());
        }
        let receipt = draft_markdown_memory_file_proposal_with_state(
            DraftMarkdownMemoryFileRequest {
                scope: MarkdownMemoryScope::Project,
                relative_path: "MEMORY.md".into(),
                content: "# Project\nKeep release notes current.".into(),
                expected_current_digest: None,
            },
            &state,
        )
        .await
        .unwrap();

        assert_eq!(receipt.status, "review_required");
        assert!(!root.path().join("MEMORY.md").exists());
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&receipt.proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(proposal.proposal_type, ProposalType::ExternalWriteAction);
        assert_eq!(proposal.after["directFileWrite"], false);
        assert_eq!(proposal.after["memoryScope"], "project");
        assert_eq!(proposal.after["expected_target_absent"], true);
    }

    #[tokio::test]
    async fn stale_editor_digest_is_rejected_before_review_creation() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("MEMORY.md"), "current").unwrap();
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let canonical = root.path().canonicalize().unwrap();
        {
            let mut config = state.config.lock().await;
            config.system.workspace_memory_root = Some(canonical.to_string_lossy().into_owned());
        }
        let error = draft_markdown_memory_file_proposal_with_state(
            DraftMarkdownMemoryFileRequest {
                scope: MarkdownMemoryScope::Workspace,
                relative_path: "MEMORY.md".into(),
                content: "replacement".into(),
                expected_current_digest: Some("sha256:stale".into()),
            },
            &state,
        )
        .await
        .unwrap_err();

        assert!(error.contains("changed since it was loaded"));
        assert_eq!(
            std::fs::read_to_string(root.path().join("MEMORY.md")).unwrap(),
            "current"
        );
    }

    #[tokio::test]
    async fn deactivation_is_a_reviewed_move_and_does_not_hide_memory_early() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("MEMORY.md");
        std::fs::write(&source, "active project rule").unwrap();
        let digest = content_digest("active project rule");
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let canonical = root.path().canonicalize().unwrap();
        {
            let mut config = state.config.lock().await;
            config.system.project_memory_root = Some(canonical.to_string_lossy().into_owned());
        }
        let receipt = deactivate_markdown_memory_file_proposal_with_state(
            DeactivateMarkdownMemoryFileRequest {
                scope: MarkdownMemoryScope::Project,
                relative_path: "MEMORY.md".into(),
                expected_current_digest: digest,
            },
            &state,
        )
        .await
        .unwrap();

        assert!(source.exists());
        assert!(!root.path().join("MEMORY.disabled.md").exists());
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&receipt.proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(proposal.after["operation"], "move");
        assert_eq!(proposal.after["directWritesExecuted"], false);
    }

    #[tokio::test]
    async fn accepted_write_and_deactivation_use_the_existing_artifact_materializer() {
        let root = tempfile::tempdir().unwrap();
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let canonical = root.path().canonicalize().unwrap();
        {
            let mut config = state.config.lock().await;
            config.system.project_memory_root = Some(canonical.to_string_lossy().into_owned());
        }
        let write = draft_markdown_memory_file_proposal_with_state(
            DraftMarkdownMemoryFileRequest {
                scope: MarkdownMemoryScope::Project,
                relative_path: "MEMORY.md".into(),
                content: "# Release\nPreserve exact source citations.".into(),
                expected_current_digest: None,
            },
            &state,
        )
        .await
        .unwrap();
        crate::commands::proposal::accept_proposal_with_state(write.proposal_id, &state)
            .await
            .unwrap();
        let source = root.path().join("MEMORY.md");
        assert_eq!(
            std::fs::read_to_string(&source).unwrap(),
            "# Release\nPreserve exact source citations."
        );
        let after_write = get_markdown_memory_view_model_with_state(&state)
            .await
            .unwrap();
        assert_eq!(after_write.files.len(), 1);

        let deactivate = deactivate_markdown_memory_file_proposal_with_state(
            DeactivateMarkdownMemoryFileRequest {
                scope: MarkdownMemoryScope::Project,
                relative_path: "MEMORY.md".into(),
                expected_current_digest: after_write.files[0].content_digest.clone(),
            },
            &state,
        )
        .await
        .unwrap();
        crate::commands::proposal::accept_proposal_with_state(deactivate.proposal_id, &state)
            .await
            .unwrap();

        assert!(!source.exists());
        assert!(root.path().join("MEMORY.disabled.md").exists());
        let after_deactivation = get_markdown_memory_view_model_with_state(&state)
            .await
            .unwrap();
        assert!(after_deactivation.files.is_empty());
    }

    #[tokio::test]
    async fn forged_markdown_memory_proposal_cannot_borrow_another_scope_target() {
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let canonical_project = project.path().canonicalize().unwrap();
        state.config.lock().await.system.project_memory_root =
            Some(canonical_project.to_string_lossy().into_owned());
        let target = outside.path().join("MEMORY.md");
        let mut proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            "filesystem.forged-markdown-memory",
            serde_json::json!({
                "path": target,
                "content": "must never be written",
                "expected_target_absent": true,
                "expected_target_digest": null,
                "operation": "propose_write",
                "memoryScope": "project",
                "memoryRelativePath": "MEMORY.md",
                "source": "markdown_memory_editor",
                "directFileWrite": false,
            }),
            "Forged scope binding test",
            1.0,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        proposal.id = format!("proposal:markdown-memory:{}", uuid::Uuid::new_v4());
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let result =
            crate::commands::proposal::accept_proposal_with_state(proposal.id, &state).await;

        assert!(result.is_err());
        assert!(!target.exists());
    }
}
