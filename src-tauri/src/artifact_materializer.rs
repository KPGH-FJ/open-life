use openlife_core::agent::metadata_safe_text_digest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct PreparedArtifactMaterialization {
    pub proposal_id: String,
    pub target_path: PathBuf,
    pub stage_path: PathBuf,
    pub target_reference_digest: String,
    pub content_digest: String,
    pub byte_size: u64,
    pub media_type: String,
    pub target_precondition: ArtifactTargetPrecondition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArtifactTargetPrecondition {
    Absent,
    ContentDigest(String),
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedArtifactMove {
    pub proposal_id: String,
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub target_reference_digest: String,
    pub content_digest: String,
    pub byte_size: u64,
    pub media_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArtifactFilesystemFailure {
    FailedBeforeEffect(String),
    Unknown(String),
}

impl ArtifactFilesystemFailure {
    pub fn code(&self) -> &str {
        match self {
            Self::FailedBeforeEffect(code) | Self::Unknown(code) => code,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArtifactFilesystemObservation {
    Confirmed { observed_content_digest: String },
    Staged,
    NoStagedOrFinalBytes,
    Unknown { reason_code: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactMaterializationReceipt {
    pub artifact_id: String,
    pub proposal_id: String,
    pub target_reference: String,
    pub target_reference_digest: String,
    pub content_digest: String,
    pub observed_content_digest: String,
    pub byte_size: u64,
    pub media_type: String,
    pub status: ArtifactMaterializationReceiptStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactMaterializationReceiptStatus {
    Confirmed,
}

fn path_contains_symlink(path: &Path) -> bool {
    path.ancestors().any(|component| {
        component
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
    })
}

fn canonical_safe_paths(safe_paths: &[String]) -> Vec<PathBuf> {
    safe_paths
        .iter()
        .filter_map(|safe| {
            let path = Path::new(safe);
            if path_contains_symlink(path) {
                return None;
            }
            path.canonicalize().ok()
        })
        .collect()
}

fn canonical_parent_in_safe_paths(
    target: &Path,
    safe_paths: &[PathBuf],
) -> Result<PathBuf, String> {
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            format!(
                "Path '{}' has no existing parent directory.",
                target.display()
            )
        })?;
    if path_contains_symlink(parent) {
        return Err(format!(
            "Path '{}' contains a symbolic link. Symbolic links are not allowed in safe paths.",
            parent.display()
        ));
    }
    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| format!("Failed to canonicalize parent directory: {error}"))?;
    if !safe_paths
        .iter()
        .any(|safe| canonical_parent.starts_with(safe))
    {
        return Err(format!(
            "Path '{}' is not in the configured safe paths.",
            target.display()
        ));
    }
    Ok(canonical_parent)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn file_digest(path: &Path) -> Result<Option<String>, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "Artifact path '{}' is a symbolic link.",
            path.display()
        )),
        Ok(metadata) if !metadata.is_file() => Err(format!(
            "Artifact path '{}' is not a regular file.",
            path.display()
        )),
        Ok(_) => std::fs::read(path)
            .map(|bytes| Some(sha256_bytes(&bytes)))
            .map_err(|error| format!("Failed to read artifact '{}': {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "Failed to inspect artifact '{}': {error}",
            path.display()
        )),
    }
}

fn media_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("md" | "markdown") => "text/markdown; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    }
}

pub(crate) fn prepare_artifact_materialization(
    proposal_id: &str,
    dispatch_claim_id: &str,
    path: &str,
    content: &str,
    safe_paths: &[String],
) -> Result<PreparedArtifactMaterialization, String> {
    let target_precondition = capture_artifact_target_precondition(path, safe_paths)?;
    prepare_artifact_materialization_with_precondition(
        proposal_id,
        dispatch_claim_id,
        path,
        content,
        safe_paths,
        target_precondition,
    )
}

pub(crate) fn capture_artifact_target_precondition(
    path: &str,
    safe_paths: &[String],
) -> Result<ArtifactTargetPrecondition, String> {
    let valid_safe_paths = canonical_safe_paths(safe_paths);
    if valid_safe_paths.is_empty() {
        return Err("No valid safe paths configured for artifact materialization.".into());
    }
    let requested_target = Path::new(path);
    if path_contains_symlink(requested_target) {
        return Err(format!(
            "Path '{}' contains a symbolic link. Symbolic links are not allowed in safe paths.",
            requested_target.display()
        ));
    }
    let canonical_parent = canonical_parent_in_safe_paths(requested_target, &valid_safe_paths)?;
    let filename = requested_target
        .file_name()
        .ok_or_else(|| format!("Path '{}' has no filename.", requested_target.display()))?;
    let target_path = canonical_parent.join(filename);
    if path_contains_symlink(&target_path) {
        return Err(format!(
            "Path '{}' contains a symbolic link. Symbolic links are not allowed in safe paths.",
            target_path.display()
        ));
    }
    Ok(match file_digest(&target_path)? {
        Some(digest) => ArtifactTargetPrecondition::ContentDigest(digest),
        None => ArtifactTargetPrecondition::Absent,
    })
}

pub(crate) fn prepare_artifact_materialization_with_precondition(
    proposal_id: &str,
    dispatch_claim_id: &str,
    path: &str,
    content: &str,
    safe_paths: &[String],
    target_precondition: ArtifactTargetPrecondition,
) -> Result<PreparedArtifactMaterialization, String> {
    let valid_safe_paths = canonical_safe_paths(safe_paths);
    if valid_safe_paths.is_empty() {
        return Err("No valid safe paths configured for artifact materialization.".into());
    }
    let requested_target = Path::new(path);
    if path_contains_symlink(requested_target) {
        return Err(format!(
            "Path '{}' contains a symbolic link. Symbolic links are not allowed in safe paths.",
            requested_target.display()
        ));
    }
    let canonical_parent = canonical_parent_in_safe_paths(requested_target, &valid_safe_paths)?;
    let filename = requested_target
        .file_name()
        .ok_or_else(|| format!("Path '{}' has no filename.", requested_target.display()))?;
    let target_path = canonical_parent.join(filename);
    if path_contains_symlink(&target_path) {
        return Err(format!(
            "Path '{}' contains a symbolic link. Symbolic links are not allowed in safe paths.",
            target_path.display()
        ));
    }
    let target_reference = target_path.to_string_lossy().into_owned();
    let target_reference_digest = metadata_safe_text_digest(&target_reference).1;
    let content_digest = sha256_bytes(content.as_bytes());
    let stage_identity = metadata_safe_text_digest(&format!(
        "{proposal_id}\0{dispatch_claim_id}\0{target_reference_digest}"
    ))
    .1;
    let stage_token = stage_identity
        .strip_prefix("sha256:")
        .unwrap_or(&stage_identity)
        .chars()
        .take(32)
        .collect::<String>();
    let stage_path = canonical_parent.join(format!(".openlife-artifact-{stage_token}.staged"));
    if path_contains_symlink(&stage_path) {
        return Err("Artifact staging path contains a symbolic link.".into());
    }
    Ok(PreparedArtifactMaterialization {
        proposal_id: proposal_id.to_string(),
        target_path,
        stage_path,
        target_reference_digest,
        content_digest,
        byte_size: content.len() as u64,
        media_type: media_type_for_path(requested_target).to_string(),
        target_precondition,
    })
}

pub(crate) fn prepare_artifact_move(
    proposal_id: &str,
    source: &str,
    target: &str,
    expected_content_digest: &str,
    safe_paths: &[String],
) -> Result<PreparedArtifactMove, String> {
    let valid_safe_paths = canonical_safe_paths(safe_paths);
    if valid_safe_paths.is_empty() {
        return Err("No valid safe paths configured for artifact move.".into());
    }
    let source_path = Path::new(source);
    let target_path = Path::new(target);
    if path_contains_symlink(source_path) || path_contains_symlink(target_path) {
        return Err("Artifact move paths cannot contain symbolic links.".into());
    }
    let canonical_source = source_path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve move source: {error}"))?;
    if !valid_safe_paths
        .iter()
        .any(|safe| canonical_source.starts_with(safe))
    {
        return Err("Artifact move source is outside configured safe paths.".into());
    }
    let metadata = std::fs::symlink_metadata(&canonical_source)
        .map_err(|error| format!("Failed to inspect move source: {error}"))?;
    if !metadata.is_file() {
        return Err("Artifact move source is not a regular file.".into());
    }
    let canonical_target_parent = canonical_parent_in_safe_paths(target_path, &valid_safe_paths)?;
    let target_name = target_path
        .file_name()
        .ok_or_else(|| "Artifact move target has no filename.".to_string())?;
    let resolved_target = canonical_target_parent.join(target_name);
    if resolved_target.exists() {
        return Err("Artifact move target already exists.".into());
    }
    let content_digest = file_digest(&canonical_source)?
        .ok_or_else(|| "Artifact move source disappeared during preparation.".to_string())?;
    let expected = if expected_content_digest.starts_with("sha256:") {
        expected_content_digest.to_string()
    } else {
        format!("sha256:{expected_content_digest}")
    };
    if !expected_content_digest.trim().is_empty() && content_digest != expected {
        return Err("Artifact move source digest does not match the reviewed proposal.".into());
    }
    let target_reference = format!(
        "{} -> {}",
        canonical_source.display(),
        resolved_target.display()
    );
    Ok(PreparedArtifactMove {
        proposal_id: proposal_id.to_string(),
        source_path: canonical_source,
        target_path: resolved_target.clone(),
        target_reference_digest: metadata_safe_text_digest(&target_reference).1,
        content_digest,
        byte_size: metadata.len(),
        media_type: media_type_for_path(&resolved_target).to_string(),
    })
}

pub(crate) fn trash_target_for_source(
    source: &str,
    safe_paths: &[String],
) -> Result<PathBuf, String> {
    let valid_safe_paths = canonical_safe_paths(safe_paths);
    let source = Path::new(source)
        .canonicalize()
        .map_err(|error| format!("Failed to resolve trash source: {error}"))?;
    if path_contains_symlink(&source)
        || !valid_safe_paths.iter().any(|safe| source.starts_with(safe))
    {
        return Err("Trash source is outside configured safe paths or contains a symlink.".into());
    }
    let parent = source
        .parent()
        .ok_or_else(|| "Trash source has no parent directory.".to_string())?;
    let filename = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Trash source filename is not valid UTF-8.".to_string())?;
    let digest = metadata_safe_text_digest(&source.to_string_lossy()).1;
    let token = digest
        .strip_prefix("sha256:")
        .unwrap_or(&digest)
        .chars()
        .take(16)
        .collect::<String>();
    Ok(parent.join(format!(".openlife-trash-{token}-{filename}")))
}

pub(crate) fn commit_artifact_move(
    prepared: &PreparedArtifactMove,
    safe_paths: &[String],
) -> Result<String, ArtifactFilesystemFailure> {
    let valid_safe_paths = canonical_safe_paths(safe_paths);
    let target_parent = canonical_parent_in_safe_paths(&prepared.target_path, &valid_safe_paths)
        .map_err(|_| ArtifactFilesystemFailure::Unknown("artifact_move_target_changed".into()))?;
    let source_parent = prepared
        .source_path
        .parent()
        .ok_or_else(|| ArtifactFilesystemFailure::Unknown("artifact_move_source_changed".into()))?;
    if path_contains_symlink(&prepared.source_path)
        || path_contains_symlink(&prepared.target_path)
        || !valid_safe_paths
            .iter()
            .any(|safe| prepared.source_path.starts_with(safe))
    {
        return Err(ArtifactFilesystemFailure::Unknown(
            "artifact_move_path_changed".into(),
        ));
    }
    match (
        file_digest(&prepared.source_path),
        file_digest(&prepared.target_path),
    ) {
        (Ok(None), Ok(Some(target_digest))) if target_digest == prepared.content_digest => {
            return Ok(target_digest)
        }
        (Ok(Some(source_digest)), Ok(None)) if source_digest == prepared.content_digest => {}
        (Ok(Some(_)), Ok(None)) => {
            return Err(ArtifactFilesystemFailure::FailedBeforeEffect(
                "artifact_move_source_digest_changed".into(),
            ))
        }
        (Ok(None), Ok(None)) => {
            return Err(ArtifactFilesystemFailure::Unknown(
                "artifact_move_source_and_target_missing".into(),
            ))
        }
        _ => {
            return Err(ArtifactFilesystemFailure::Unknown(
                "artifact_move_state_ambiguous".into(),
            ))
        }
    }
    if let Err(error) = std::fs::rename(&prepared.source_path, &prepared.target_path) {
        return match (
            file_digest(&prepared.source_path),
            file_digest(&prepared.target_path),
        ) {
            (Ok(Some(source_digest)), Ok(None)) if source_digest == prepared.content_digest => {
                Err(ArtifactFilesystemFailure::FailedBeforeEffect(format!(
                    "artifact_move_rename_failed:{}",
                    error.kind()
                )))
            }
            (Ok(None), Ok(Some(target_digest))) if target_digest == prepared.content_digest => {
                Ok(target_digest)
            }
            _ => Err(ArtifactFilesystemFailure::Unknown(
                "artifact_move_rename_outcome_unknown".into(),
            )),
        };
    }
    for parent in [source_parent, target_parent.as_path()] {
        if std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .is_err()
        {
            return Err(ArtifactFilesystemFailure::Unknown(
                "artifact_move_parent_sync_unknown".into(),
            ));
        }
    }
    match (
        file_digest(&prepared.source_path),
        file_digest(&prepared.target_path),
    ) {
        (Ok(None), Ok(Some(target_digest))) if target_digest == prepared.content_digest => {
            Ok(target_digest)
        }
        _ => Err(ArtifactFilesystemFailure::Unknown(
            "artifact_move_final_state_unconfirmed".into(),
        )),
    }
}

pub(crate) fn confirmed_move_receipt(
    prepared: &PreparedArtifactMove,
    observed_content_digest: String,
) -> ArtifactMaterializationReceipt {
    ArtifactMaterializationReceipt {
        artifact_id: format!("artifact:{}", prepared.proposal_id),
        proposal_id: prepared.proposal_id.clone(),
        target_reference: prepared.target_path.to_string_lossy().into_owned(),
        target_reference_digest: prepared.target_reference_digest.clone(),
        content_digest: prepared.content_digest.clone(),
        observed_content_digest,
        byte_size: prepared.byte_size,
        media_type: prepared.media_type.clone(),
        status: ArtifactMaterializationReceiptStatus::Confirmed,
    }
}

fn resolved_move_paths(
    source: &str,
    target: &str,
    safe_paths: &[String],
) -> Result<(PathBuf, PathBuf), String> {
    let valid_safe_paths = canonical_safe_paths(safe_paths);
    if valid_safe_paths.is_empty() {
        return Err("No valid safe paths configured for artifact move.".into());
    }
    let resolve = |raw: &str| -> Result<PathBuf, String> {
        let path = Path::new(raw);
        if path_contains_symlink(path) {
            return Err("Artifact move path contains a symbolic link.".into());
        }
        let parent = canonical_parent_in_safe_paths(path, &valid_safe_paths)?;
        let name = path
            .file_name()
            .ok_or_else(|| "Artifact move path has no filename.".to_string())?;
        Ok(parent.join(name))
    };
    Ok((resolve(source)?, resolve(target)?))
}

pub(crate) fn inspect_artifact_move(
    source: &str,
    target: &str,
    expected_content_digest: &str,
    safe_paths: &[String],
) -> Result<(String, ArtifactFilesystemObservation), String> {
    let (source, target) = resolved_move_paths(source, target, safe_paths)?;
    let target_reference = format!("{} -> {}", source.display(), target.display());
    let target_reference_digest = metadata_safe_text_digest(&target_reference).1;
    let observation = match (file_digest(&source), file_digest(&target)) {
        (Ok(None), Ok(Some(target_digest))) if target_digest == expected_content_digest => {
            ArtifactFilesystemObservation::Confirmed {
                observed_content_digest: target_digest,
            }
        }
        (Ok(Some(source_digest)), Ok(None)) if source_digest == expected_content_digest => {
            ArtifactFilesystemObservation::NoStagedOrFinalBytes
        }
        _ => ArtifactFilesystemObservation::Unknown {
            reason_code: "artifact_move_filesystem_state_ambiguous".into(),
        },
    };
    Ok((target_reference_digest, observation))
}

pub(crate) fn confirmed_move_receipt_from_paths(
    proposal_id: &str,
    target: &str,
    target_reference_digest: String,
    content_digest: String,
    byte_size: u64,
    media_type: String,
) -> ArtifactMaterializationReceipt {
    ArtifactMaterializationReceipt {
        artifact_id: format!("artifact:{proposal_id}"),
        proposal_id: proposal_id.to_string(),
        target_reference: target.to_string(),
        target_reference_digest,
        observed_content_digest: content_digest.clone(),
        content_digest,
        byte_size,
        media_type,
        status: ArtifactMaterializationReceiptStatus::Confirmed,
    }
}

pub(crate) fn stage_artifact_bytes(
    prepared: &PreparedArtifactMaterialization,
    content: &str,
) -> Result<(), ArtifactFilesystemFailure> {
    if sha256_bytes(content.as_bytes()) != prepared.content_digest
        || content.len() as u64 != prepared.byte_size
    {
        return Err(ArtifactFilesystemFailure::FailedBeforeEffect(
            "artifact_content_binding_mismatch".into(),
        ));
    }
    match file_digest(&prepared.stage_path) {
        Ok(Some(digest)) => {
            return if digest == prepared.content_digest {
                Ok(())
            } else {
                Err(ArtifactFilesystemFailure::Unknown(
                    "artifact_stage_digest_mismatch".into(),
                ))
            };
        }
        Ok(None) => {}
        Err(_) => {
            return Err(ArtifactFilesystemFailure::Unknown(
                "artifact_stage_inspection_failed".into(),
            ))
        }
    }
    let mut stage = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&prepared.stage_path)
    {
        Ok(stage) => stage,
        Err(error) => {
            return Err(ArtifactFilesystemFailure::FailedBeforeEffect(format!(
                "artifact_stage_create_failed:{}",
                error.kind()
            )))
        }
    };
    let stage_result = stage
        .write_all(content.as_bytes())
        .and_then(|_| stage.sync_all());
    drop(stage);
    if let Err(error) = stage_result {
        return match std::fs::remove_file(&prepared.stage_path) {
            Ok(()) => Err(ArtifactFilesystemFailure::FailedBeforeEffect(format!(
                "artifact_stage_write_failed:{}",
                error.kind()
            ))),
            Err(remove_error) if remove_error.kind() == std::io::ErrorKind::NotFound => {
                Err(ArtifactFilesystemFailure::FailedBeforeEffect(format!(
                    "artifact_stage_write_failed:{}",
                    error.kind()
                )))
            }
            Err(_) => Err(ArtifactFilesystemFailure::Unknown(
                "artifact_partial_stage_cleanup_unknown".into(),
            )),
        };
    }
    match file_digest(&prepared.stage_path) {
        Ok(Some(digest)) if digest == prepared.content_digest => Ok(()),
        Ok(_) => Err(ArtifactFilesystemFailure::Unknown(
            "artifact_stage_digest_unconfirmed".into(),
        )),
        Err(_) => Err(ArtifactFilesystemFailure::Unknown(
            "artifact_stage_inspection_failed".into(),
        )),
    }
}

pub(crate) fn commit_staged_artifact(
    prepared: &PreparedArtifactMaterialization,
    safe_paths: &[String],
) -> Result<String, ArtifactFilesystemFailure> {
    let valid_safe_paths = canonical_safe_paths(safe_paths);
    let parent = canonical_parent_in_safe_paths(&prepared.target_path, &valid_safe_paths)
        .map_err(|_| ArtifactFilesystemFailure::Unknown("artifact_parent_changed".into()))?;
    if parent != prepared.target_path.parent().unwrap_or(Path::new(""))
        || path_contains_symlink(&prepared.target_path)
        || path_contains_symlink(&prepared.stage_path)
    {
        return Err(ArtifactFilesystemFailure::Unknown(
            "artifact_path_changed_before_rename".into(),
        ));
    }
    match file_digest(&prepared.target_path) {
        Ok(Some(digest)) if digest == prepared.content_digest => {
            match file_digest(&prepared.stage_path) {
                Ok(Some(stage_digest)) if stage_digest == prepared.content_digest => {
                    std::fs::remove_file(&prepared.stage_path).map_err(|_| {
                        ArtifactFilesystemFailure::Unknown(
                            "artifact_confirmed_stage_cleanup_failed".into(),
                        )
                    })?;
                    std::fs::File::open(&parent)
                        .and_then(|directory| directory.sync_all())
                        .map_err(|_| {
                            ArtifactFilesystemFailure::Unknown(
                                "artifact_confirmed_stage_cleanup_sync_failed".into(),
                            )
                        })?;
                }
                Ok(None) => {}
                _ => {
                    return Err(ArtifactFilesystemFailure::Unknown(
                        "artifact_confirmed_stage_state_ambiguous".into(),
                    ))
                }
            }
            return Ok(digest);
        }
        Ok(current) => {
            let precondition_matches = match (&prepared.target_precondition, current.as_deref()) {
                (ArtifactTargetPrecondition::Absent, None) => true,
                (ArtifactTargetPrecondition::ContentDigest(expected), Some(actual)) => {
                    expected == actual
                }
                _ => false,
            };
            if !precondition_matches {
                return Err(ArtifactFilesystemFailure::FailedBeforeEffect(
                    "artifact_target_precondition_changed".into(),
                ));
            }
        }
        Err(_) => {
            return Err(ArtifactFilesystemFailure::Unknown(
                "artifact_target_inspection_failed".into(),
            ))
        }
    }
    match file_digest(&prepared.stage_path) {
        Ok(Some(digest)) if digest == prepared.content_digest => {}
        Ok(_) => {
            return Err(ArtifactFilesystemFailure::Unknown(
                "artifact_stage_missing_or_mismatched".into(),
            ))
        }
        Err(_) => {
            return Err(ArtifactFilesystemFailure::Unknown(
                "artifact_stage_inspection_failed".into(),
            ))
        }
    }
    let commit_result = match &prepared.target_precondition {
        ArtifactTargetPrecondition::Absent => {
            std::fs::hard_link(&prepared.stage_path, &prepared.target_path)
        }
        ArtifactTargetPrecondition::ContentDigest(_) => {
            std::fs::rename(&prepared.stage_path, &prepared.target_path)
        }
    };
    if let Err(error) = commit_result {
        if matches!(
            &prepared.target_precondition,
            ArtifactTargetPrecondition::Absent
        ) && error.kind() == std::io::ErrorKind::AlreadyExists
        {
            return Err(ArtifactFilesystemFailure::FailedBeforeEffect(
                "artifact_target_precondition_changed".into(),
            ));
        }
        return match file_digest(&prepared.target_path) {
            Ok(Some(digest)) if digest == prepared.content_digest => Ok(digest),
            _ => Err(ArtifactFilesystemFailure::Unknown(
                "artifact_rename_outcome_unknown".into(),
            )),
        };
    }
    if std::fs::File::open(&parent)
        .and_then(|directory| directory.sync_all())
        .is_err()
    {
        return Err(ArtifactFilesystemFailure::Unknown(
            "artifact_parent_sync_unknown".into(),
        ));
    }
    if matches!(
        &prepared.target_precondition,
        ArtifactTargetPrecondition::Absent
    ) {
        std::fs::remove_file(&prepared.stage_path).map_err(|_| {
            ArtifactFilesystemFailure::Unknown("artifact_link_stage_cleanup_unknown".into())
        })?;
        std::fs::File::open(&parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| {
                ArtifactFilesystemFailure::Unknown("artifact_link_cleanup_sync_unknown".into())
            })?;
    }
    match file_digest(&prepared.target_path) {
        Ok(Some(digest)) if digest == prepared.content_digest => Ok(digest),
        _ => Err(ArtifactFilesystemFailure::Unknown(
            "artifact_final_digest_unconfirmed".into(),
        )),
    }
}

pub(crate) fn inspect_artifact_filesystem(
    prepared: &PreparedArtifactMaterialization,
) -> ArtifactFilesystemObservation {
    let final_digest = file_digest(&prepared.target_path);
    let stage_digest = file_digest(&prepared.stage_path);
    match (final_digest, stage_digest) {
        (Ok(Some(final_digest)), Ok(None)) if final_digest == prepared.content_digest => {
            ArtifactFilesystemObservation::Confirmed {
                observed_content_digest: final_digest,
            }
        }
        (Ok(Some(final_digest)), Ok(Some(stage_digest)))
            if final_digest == prepared.content_digest
                && stage_digest == prepared.content_digest =>
        {
            ArtifactFilesystemObservation::Staged
        }
        (Ok(_), Ok(Some(stage_digest))) if stage_digest == prepared.content_digest => {
            ArtifactFilesystemObservation::Staged
        }
        (Ok(None), Ok(None)) => ArtifactFilesystemObservation::NoStagedOrFinalBytes,
        _ => ArtifactFilesystemObservation::Unknown {
            reason_code: "artifact_filesystem_state_ambiguous".into(),
        },
    }
}

pub(crate) fn confirmed_artifact_receipt(
    prepared: &PreparedArtifactMaterialization,
    observed_content_digest: String,
) -> ArtifactMaterializationReceipt {
    ArtifactMaterializationReceipt {
        artifact_id: format!("artifact:{}", prepared.proposal_id),
        proposal_id: prepared.proposal_id.clone(),
        target_reference: prepared.target_path.to_string_lossy().into_owned(),
        target_reference_digest: prepared.target_reference_digest.clone(),
        content_digest: prepared.content_digest.clone(),
        observed_content_digest,
        byte_size: prepared.byte_size,
        media_type: prepared.media_type.clone(),
        status: ArtifactMaterializationReceiptStatus::Confirmed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared_fixture(
        directory: &Path,
        proposal_id: &str,
        claim_id: &str,
    ) -> PreparedArtifactMaterialization {
        let directory = directory.canonicalize().unwrap();
        prepare_artifact_materialization(
            proposal_id,
            claim_id,
            &directory.join("roadshow-summary.md").to_string_lossy(),
            "# Roadshow\n\nVerified summary.",
            &[directory.to_string_lossy().into_owned()],
        )
        .unwrap()
    }

    #[test]
    fn staged_bytes_are_not_presented_as_final_and_commit_matches_digest() {
        let directory = tempfile::tempdir().unwrap();
        let prepared = prepared_fixture(directory.path(), "proposal-1", "claim-1");
        let content = "# Roadshow\n\nVerified summary.";
        stage_artifact_bytes(&prepared, content).unwrap();
        assert!(!prepared.target_path.exists());
        assert_eq!(
            inspect_artifact_filesystem(&prepared),
            ArtifactFilesystemObservation::Staged
        );
        let observed = commit_staged_artifact(
            &prepared,
            &[directory
                .path()
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned()],
        )
        .unwrap();
        assert_eq!(observed, prepared.content_digest);
        assert_eq!(
            std::fs::read_to_string(&prepared.target_path).unwrap(),
            content
        );
        assert_eq!(
            inspect_artifact_filesystem(&prepared),
            ArtifactFilesystemObservation::Confirmed {
                observed_content_digest: prepared.content_digest.clone()
            }
        );
    }

    #[test]
    fn target_created_after_review_is_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let prepared = prepared_fixture(directory.path(), "proposal-cas", "claim-cas");
        let content = "# Roadshow\n\nVerified summary.";
        stage_artifact_bytes(&prepared, content).unwrap();

        std::fs::write(&prepared.target_path, "concurrent user change").unwrap();

        assert!(commit_staged_artifact(
            &prepared,
            &[directory
                .path()
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned()],
        )
        .is_err());
        assert_eq!(
            std::fs::read_to_string(&prepared.target_path).unwrap(),
            "concurrent user change"
        );
    }

    #[test]
    fn unchanged_reviewed_target_can_be_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let safe_root = directory.path().canonicalize().unwrap();
        let target = safe_root.join("roadshow-summary.md");
        std::fs::write(&target, "reviewed old content").unwrap();
        let safe_paths = vec![safe_root.to_string_lossy().into_owned()];
        let content = "# Roadshow\n\nVerified summary.";
        let prepared = prepare_artifact_materialization(
            "proposal-overwrite",
            "claim-overwrite",
            &target.to_string_lossy(),
            content,
            &safe_paths,
        )
        .unwrap();

        stage_artifact_bytes(&prepared, content).unwrap();
        let observed = commit_staged_artifact(&prepared, &safe_paths).unwrap();

        assert_eq!(observed, prepared.content_digest);
        assert_eq!(std::fs::read_to_string(target).unwrap(), content);
    }

    #[test]
    fn target_already_equal_to_new_content_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let prepared = prepared_fixture(directory.path(), "proposal-idempotent", "claim-1");
        let content = "# Roadshow\n\nVerified summary.";
        stage_artifact_bytes(&prepared, content).unwrap();
        std::fs::write(&prepared.target_path, content).unwrap();
        let safe_paths = vec![directory
            .path()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned()];

        let observed = commit_staged_artifact(&prepared, &safe_paths).unwrap();

        assert_eq!(observed, prepared.content_digest);
        assert!(!prepared.stage_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn target_symlink_is_rejected_before_staging() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let safe_root = directory.path().canonicalize().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        let target = safe_root.join("roadshow-summary.md");
        symlink(outside.path(), &target).unwrap();
        let error = prepare_artifact_materialization(
            "proposal-1",
            "claim-1",
            &target.to_string_lossy(),
            "private",
            &[safe_root.to_string_lossy().into_owned()],
        )
        .unwrap_err();
        assert!(error.contains("symbolic link"), "{error}");
    }

    #[test]
    fn reviewed_move_is_digest_bound_and_can_be_reversed() {
        let directory = tempfile::tempdir().unwrap();
        let safe_root = directory.path().canonicalize().unwrap();
        let source = safe_root.join("source.md");
        let target = safe_root.join("target.md");
        std::fs::write(&source, "reviewed content").unwrap();
        let digest = sha256_bytes(b"reviewed content");
        let prepared = prepare_artifact_move(
            "proposal-move",
            &source.to_string_lossy(),
            &target.to_string_lossy(),
            &digest,
            &[safe_root.to_string_lossy().into_owned()],
        )
        .unwrap();

        assert_eq!(
            commit_artifact_move(&prepared, &[safe_root.to_string_lossy().into_owned()]).unwrap(),
            digest
        );
        assert!(!source.exists());
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "reviewed content"
        );

        let rollback = prepare_artifact_move(
            "proposal-rollback",
            &target.to_string_lossy(),
            &source.to_string_lossy(),
            &digest,
            &[safe_root.to_string_lossy().into_owned()],
        )
        .unwrap();
        commit_artifact_move(&rollback, &[safe_root.to_string_lossy().into_owned()]).unwrap();
        assert_eq!(std::fs::read_to_string(source).unwrap(), "reviewed content");
        assert!(!target.exists());
    }

    #[test]
    fn move_fails_before_effect_when_source_changed_after_review() {
        let directory = tempfile::tempdir().unwrap();
        let safe_root = directory.path().canonicalize().unwrap();
        let source = safe_root.join("source.md");
        let target = safe_root.join("target.md");
        std::fs::write(&source, "reviewed content").unwrap();
        let prepared = prepare_artifact_move(
            "proposal-move",
            &source.to_string_lossy(),
            &target.to_string_lossy(),
            &sha256_bytes(b"reviewed content"),
            &[safe_root.to_string_lossy().into_owned()],
        )
        .unwrap();
        std::fs::write(&source, "changed after review").unwrap();

        assert_eq!(
            commit_artifact_move(&prepared, &[safe_root.to_string_lossy().into_owned()]),
            Err(ArtifactFilesystemFailure::FailedBeforeEffect(
                "artifact_move_source_digest_changed".into()
            ))
        );
        assert!(source.exists());
        assert!(!target.exists());
    }
}
