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
    })
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
        Ok(_) => {}
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
    if std::fs::rename(&prepared.stage_path, &prepared.target_path).is_err() {
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
}
