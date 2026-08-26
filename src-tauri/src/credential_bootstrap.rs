//! First-run initialization for OpenLife-owned integrity credentials.
//!
//! This module distinguishes an empty profile from credential recovery. It
//! may create missing internal keys only while every protected owner for that
//! key is absent. Existing, invalid, or unavailable authority is never
//! rotated here and remains a Settings recovery concern.

use crate::secret_store::{
    hydrate_bound_provider_secret, hydrate_or_create_integrity_key,
    inspect_and_hydrate_integrity_key, IntegrityKeyHydration, SecretReader, SecretStore,
    CANONICAL_TASK_RECEIPT_KEY_REF, SEARCH_KEY_REF,
};
use crate::state::CredentialBootstrapStatus;
use anyhow::{Context, Result};
use openlife_core::config::AppConfig;
use openlife_core::conversation::ProviderConnectionRecord;
use sha2::{Digest, Sha256};
use std::fs::File;
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::Path;

struct FreshInitializationLock(File);

impl FreshInitializationLock {
    fn acquire(data_dir: &Path) -> Result<Self> {
        let file = File::open(data_dir).context("open fresh credential initialization owner")?;
        #[cfg(unix)]
        {
            // SAFETY: `file` owns this descriptor for the guard lifetime. The
            // data-directory lock serializes reinspection and all exact writes
            // without introducing another durable lock authority.
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result != 0 {
                return Err(std::io::Error::last_os_error())
                    .context("fresh credential initialization is active in another process");
            }
            Ok(Self(file))
        }
        #[cfg(not(unix))]
        {
            let _ = file;
            anyhow::bail!("fresh credential initialization locking is unavailable")
        }
    }
}

impl Drop for FreshInitializationLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // SAFETY: this guard still owns the live descriptor and releases
            // it immediately before the descriptor itself is dropped.
            unsafe {
                libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
            }
        }
    }
}

fn protected_paths_are_absent(data_dir: &Path, relative_paths: &[&str]) -> Result<bool> {
    for relative_path in relative_paths {
        match std::fs::symlink_metadata(data_dir.join(relative_path)) {
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("inspect protected credential owner"),
        }
    }
    Ok(true)
}

fn fixed_key_needs_initialization<R: SecretReader + ?Sized>(
    data_dir: &Path,
    reader: &R,
    secret_ref: &'static str,
    protected_paths: &[&str],
) -> Result<bool> {
    match inspect_and_hydrate_integrity_key(secret_ref, reader) {
        IntegrityKeyHydration::Available(_) => Ok(false),
        IntegrityKeyHydration::Missing
            if protected_paths_are_absent(data_dir, protected_paths)? =>
        {
            Ok(true)
        }
        IntegrityKeyHydration::Missing => {
            anyhow::bail!("existing protected data has no matching internal credential")
        }
        IntegrityKeyHydration::Invalid => anyhow::bail!("an internal credential is invalid"),
        IntegrityKeyHydration::Unavailable => {
            anyhow::bail!("an internal credential is unavailable")
        }
    }
}

fn provider_connection_credential_status<R: SecretReader + ?Sized>(
    connection: &ProviderConnectionRecord,
    reader: &R,
) -> CredentialBootstrapStatus {
    let Some(reference) = connection.credential_reference.as_deref() else {
        return CredentialBootstrapStatus::MissingExistingData;
    };
    match reader.read_secret(reference) {
        Ok(Some(encoded)) => {
            if hydrate_bound_provider_secret(
                &connection.provider_id,
                &connection.endpoint,
                connection.credential_version,
                &encoded,
            )
            .is_ok()
            {
                CredentialBootstrapStatus::Available
            } else {
                CredentialBootstrapStatus::Invalid
            }
        }
        Ok(None) => CredentialBootstrapStatus::MissingExistingData,
        Err(_) => CredentialBootstrapStatus::Unavailable,
    }
}

/// Summarize the credential readiness of the persistent cloud Connection set.
///
/// The aggregate is deliberately fail-closed: an unavailable OS credential
/// dominates invalid or missing data, because the user-facing recovery path
/// must remain available until every retained Connection can be inspected.
/// Local Connections are omitted because they do not own provider secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderConnectionsCredentialInspection {
    pub status: CredentialBootstrapStatus,
    pub scope_digest: String,
}

pub(crate) fn inspect_provider_connections_credentials<R: SecretReader + ?Sized>(
    connections: &[ProviderConnectionRecord],
    reader: &R,
) -> ProviderConnectionsCredentialInspection {
    let mut cloud_connections = connections
        .iter()
        .filter(|connection| connection.endpoint_class == "cloud")
        .collect::<Vec<_>>();
    cloud_connections.sort_by(|left, right| left.id.cmp(&right.id));
    let scope_material = cloud_connections.iter().fold(
        "provider_connection_credential_scope_v1".to_string(),
        |mut material, connection| {
            for value in [
                connection.id.as_str(),
                connection.provider_id.as_str(),
                connection.endpoint.as_str(),
                connection.credential_reference.as_deref().unwrap_or(""),
            ] {
                material.push('\0');
                material.push_str(value);
            }
            material.push('\0');
            material.push_str(&connection.credential_version.to_string());
            material
        },
    );
    let scope_digest = format!("{:x}", Sha256::digest(scope_material.as_bytes()));
    let statuses = cloud_connections
        .iter()
        .map(|connection| provider_connection_credential_status(connection, reader))
        .collect::<Vec<_>>();
    if statuses.is_empty() {
        return ProviderConnectionsCredentialInspection {
            status: CredentialBootstrapStatus::MissingExistingData,
            scope_digest,
        };
    }
    for status in [
        CredentialBootstrapStatus::Unavailable,
        CredentialBootstrapStatus::Invalid,
        CredentialBootstrapStatus::MissingExistingData,
    ] {
        if statuses.contains(&status) {
            return ProviderConnectionsCredentialInspection {
                status,
                scope_digest,
            };
        }
    }
    ProviderConnectionsCredentialInspection {
        status: CredentialBootstrapStatus::Available,
        scope_digest,
    }
}

/// Inspect only a credential independently owned by the configured search
/// adapter. Hosted search that reuses the selected model route is covered by
/// the Provider Connection aggregate and must not duplicate that status.
pub(crate) fn inspect_search_credential_status<R: SecretReader + ?Sized>(
    config: &AppConfig,
    selected_connection: Option<&ProviderConnectionRecord>,
    reader: &R,
) -> CredentialBootstrapStatus {
    let configured = config.system.search_provider.trim().to_ascii_lowercase();
    if configured == "auto" {
        return CredentialBootstrapStatus::Available;
    }
    if matches!(configured.as_str(), "duckduckgo" | "searxng") {
        return CredentialBootstrapStatus::Available;
    }
    if selected_connection.is_some_and(|connection| {
        selected_provider_connection_supports_hosted_search(connection, &configured)
    }) {
        return CredentialBootstrapStatus::Available;
    }
    if config.system.search_provider_key_ref.as_deref() != Some(SEARCH_KEY_REF) {
        return CredentialBootstrapStatus::MissingExistingData;
    }
    match reader.read_secret(SEARCH_KEY_REF) {
        Ok(Some(secret)) if !secret.trim().is_empty() => CredentialBootstrapStatus::Available,
        Ok(Some(_)) => CredentialBootstrapStatus::Invalid,
        Ok(None) => CredentialBootstrapStatus::MissingExistingData,
        Err(_) => CredentialBootstrapStatus::Unavailable,
    }
}

fn selected_provider_connection_supports_hosted_search(
    connection: &ProviderConnectionRecord,
    configured_search: &str,
) -> bool {
    let provider = connection.provider_id.trim().to_ascii_lowercase();
    if configured_search != provider || !matches!(provider.as_str(), "deepseek" | "openrouter") {
        return false;
    }
    reqwest::Url::parse(connection.endpoint.trim()).is_ok_and(|url| {
        let path = url.path().trim_end_matches('/');
        let official_origin = url.scheme() == "https"
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none();
        official_origin
            && match provider.as_str() {
                "deepseek" => {
                    url.host_str() == Some("api.deepseek.com") && matches!(path, "" | "/v1")
                }
                "openrouter" => url.host_str() == Some("openrouter.ai") && path == "/api/v1",
                _ => false,
            }
    })
}

/// Initialize the one credential required by the canonical Work owner when
/// its protected store is provably absent. Retired execution and optional
/// capability credentials are explicit Settings concerns and cannot block a
/// new Chat/Work profile.
pub(crate) fn initialize_fresh_profile_credentials<R: SecretReader + ?Sized>(
    data_dir: &Path,
    reader: &R,
    store: &dyn SecretStore,
) -> Result<bool> {
    std::fs::create_dir_all(data_dir).context("create application data directory")?;
    let _lock = FreshInitializationLock::acquire(data_dir)?;

    let needs_canonical_task = fixed_key_needs_initialization(
        data_dir,
        reader,
        CANONICAL_TASK_RECEIPT_KEY_REF,
        &["task_runtime.db"],
    )?;
    if !needs_canonical_task {
        return Ok(false);
    }
    hydrate_or_create_integrity_key(CANONICAL_TASK_RECEIPT_KEY_REF, store)
        .context("initialize canonical Task receipt credential")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::conversation::ProviderConnectionRecord;
    use std::collections::HashMap;

    struct MemorySecretReader {
        values: HashMap<String, anyhow::Result<Option<String>>>,
    }

    impl SecretReader for MemorySecretReader {
        fn read_secret(&self, secret_ref: &str) -> Result<Option<String>> {
            match self.values.get(secret_ref) {
                Some(Ok(value)) => Ok(value.clone()),
                Some(Err(error)) => anyhow::bail!(error.to_string()),
                None => Ok(None),
            }
        }
    }

    fn cloud_connection(id: &str, secret_ref: Option<&str>) -> ProviderConnectionRecord {
        let now = chrono::Utc::now();
        ProviderConnectionRecord {
            id: id.into(),
            provider_id: "deepseek".into(),
            display_name: "DeepSeek".into(),
            endpoint: "https://api.deepseek.com".into(),
            endpoint_class: "cloud".into(),
            credential_reference: secret_ref.map(str::to_string),
            credential_version: 0,
            protocol: "openai_compatible_chat_completions".into(),
            privacy_boundary: "provider_hosted".into(),
            validation_state: "unverified".into(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn provider_bootstrap_status_is_aggregated_from_persistent_cloud_connections() {
        let available = cloud_connection("available", Some("secret://available"));
        let missing = cloud_connection("missing", Some("secret://missing"));
        let encoded = crate::secret_store::encode_provider_secret(
            &available.provider_id,
            &available.endpoint,
            available.credential_version,
            "sk-available",
        )
        .unwrap();
        let reader = MemorySecretReader {
            values: HashMap::from([
                ("secret://available".into(), Ok(Some(encoded))),
                ("secret://missing".into(), Ok(None)),
            ]),
        };

        assert_eq!(
            inspect_provider_connections_credentials(&[available], &reader).status,
            crate::state::CredentialBootstrapStatus::Available
        );
        assert_eq!(
            inspect_provider_connections_credentials(&[missing], &reader).status,
            crate::state::CredentialBootstrapStatus::MissingExistingData
        );
    }

    #[test]
    fn unavailable_connection_dominates_provider_bootstrap_aggregate() {
        let unavailable = cloud_connection("unavailable", Some("secret://unavailable"));
        let missing = cloud_connection("missing", None);
        let reader = MemorySecretReader {
            values: HashMap::from([(
                "secret://unavailable".into(),
                Err(anyhow::anyhow!("keychain denied")),
            )]),
        };

        assert_eq!(
            inspect_provider_connections_credentials(&[missing, unavailable], &reader).status,
            crate::state::CredentialBootstrapStatus::Unavailable
        );
    }

    #[test]
    fn provider_bootstrap_scope_digest_changes_when_connection_ownership_changes() {
        let first = cloud_connection("first", None);
        let second = cloud_connection("second", None);
        let reader = MemorySecretReader {
            values: HashMap::new(),
        };

        let first_scope =
            inspect_provider_connections_credentials(&[first.clone()], &reader).scope_digest;
        let expanded_scope =
            inspect_provider_connections_credentials(&[first, second], &reader).scope_digest;

        assert_ne!(first_scope, expanded_scope);
    }
}
