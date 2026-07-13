use anyhow::{Context, Result};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create atomic write parent {}", parent.display()))?;
    }
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("openlife-data");
    let temp_path = path.with_file_name(format!(".{file_name}.tmp-{}", uuid::Uuid::new_v4()));
    let write_result = (|| -> Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&temp_path)
            .with_context(|| format!("create atomic temp file {}", temp_path.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("write atomic temp file {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("fsync atomic temp file {}", temp_path.display()))?;
        std::fs::rename(&temp_path, path).with_context(|| {
            format!(
                "rename atomic temp file {} to {}",
                temp_path.display(),
                path.display()
            )
        })?;
        if let Some(parent) = path.parent() {
            std::fs::File::open(parent)
                .and_then(|directory| directory.sync_all())
                .with_context(|| format!("fsync atomic write parent {}", parent.display()))?;
        }
        Ok(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_content_without_leaving_temp_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.yaml");
        write_atomic(&path, b"version: 1\n").unwrap();
        write_atomic(&path, b"version: 2\n").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"version: 2\n");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
