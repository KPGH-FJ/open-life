use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use openlife_core::config::AppConfig;
use openlife_core::mcp_audit::{AuditKeyConfig, AuditKeyMaterial, KeyMode, McpAuditStore};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "com.openlife.desktop";
const PROVIDER_ACCOUNT: &str = "provider-api-key";
const SEARCH_ACCOUNT: &str = "search-provider-api-key";
const MAIN_CHAT_EVENT_INTEGRITY_ACCOUNT: &str = "main-chat-event-integrity-key-v1";
const ACTION_QUEUE_AUTHORITY_ACCOUNT: &str = "action-queue-authority-key-v1";
const TASK_STORE_AUTHORITY_ACCOUNT: &str = "task-store-authority-key-v1";
const AGENT_RUN_RECEIPT_ACCOUNT: &str = "agent-run-receipt-key-v1";
const MCP_AUDIT_ACCOUNT_PREFIX: &str = "mcp-audit-key-epoch-";
const PROVIDER_SECRET_ENVELOPE_VERSION: &str = "openlife_provider_secret_v1";

pub(crate) const PROVIDER_KEY_REF: &str = "keychain://com.openlife.desktop/provider-api-key";
pub(crate) const SEARCH_KEY_REF: &str = "keychain://com.openlife.desktop/search-provider-api-key";
pub(crate) const MAIN_CHAT_EVENT_INTEGRITY_KEY_REF: &str =
    "keychain://com.openlife.desktop/main-chat-event-integrity-key-v1";
pub(crate) const ACTION_QUEUE_AUTHORITY_KEY_REF: &str =
    "keychain://com.openlife.desktop/action-queue-authority-key-v1";
pub(crate) const TASK_STORE_AUTHORITY_KEY_REF: &str =
    "keychain://com.openlife.desktop/task-store-authority-key-v1";
pub(crate) const AGENT_RUN_RECEIPT_KEY_REF: &str =
    "keychain://com.openlife.desktop/agent-run-receipt-key-v1";
pub(crate) const MCP_AUDIT_KEY_REF_PREFIX: &str =
    "keychain://com.openlife.desktop/mcp-audit-key-epoch-";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProviderSecretBinding {
    provider: String,
    scheme: String,
    host: String,
    port: u16,
    base_path: String,
    credential_identity: String,
    credential_version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderSecretEnvelope {
    version: String,
    api_key: String,
    binding: ProviderSecretBinding,
}

fn provider_secret_binding(config: &AppConfig, api_key: &str) -> Result<ProviderSecretBinding> {
    let provider = config.llm.provider.trim().to_ascii_lowercase();
    if provider.is_empty() || provider == "ollama" || api_key.trim().is_empty() {
        anyhow::bail!("provider credential binding is incomplete");
    }
    let base = if config.llm.openai_base.trim().is_empty() {
        openlife_core::llm::default_base_for_provider(&provider).to_string()
    } else {
        config.llm.openai_base.trim().to_string()
    };
    let parsed = reqwest::Url::parse(&base).context("provider credential endpoint is invalid")?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        anyhow::bail!("provider credential endpoint is not canonical");
    }
    let host = parsed
        .host_str()
        .map(|host| host.trim_end_matches('.').to_ascii_lowercase())
        .filter(|host| !host.is_empty())
        .ok_or_else(|| anyhow::anyhow!("provider credential endpoint has no host"))?;
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| anyhow::anyhow!("provider credential endpoint has no canonical port"))?;
    let trimmed_path = parsed.path().trim_end_matches('/');
    let base_path = if trimmed_path.is_empty() {
        "/".to_string()
    } else {
        trimmed_path.to_string()
    };
    Ok(ProviderSecretBinding {
        provider,
        scheme: parsed.scheme().to_ascii_lowercase(),
        host,
        port,
        base_path,
        credential_identity: openlife_core::llm::provider_credential_identity(api_key),
        credential_version: config.llm.credential_version,
    })
}

fn encode_provider_secret(config: &AppConfig, api_key: &str) -> Result<String> {
    serde_json::to_string(&ProviderSecretEnvelope {
        version: PROVIDER_SECRET_ENVELOPE_VERSION.into(),
        api_key: api_key.to_string(),
        binding: provider_secret_binding(config, api_key)?,
    })
    .context("serialize provider credential envelope")
}

fn hydrate_bound_provider_secret(config: &AppConfig, encoded: &str) -> Result<String> {
    let envelope: ProviderSecretEnvelope =
        serde_json::from_str(encoded).context("provider credential reference is unbound")?;
    if envelope.version != PROVIDER_SECRET_ENVELOPE_VERSION || envelope.api_key.trim().is_empty() {
        anyhow::bail!("provider credential reference has no supported binding");
    }
    let expected = provider_secret_binding(config, &envelope.api_key)?;
    if envelope.binding != expected {
        anyhow::bail!("provider credential binding differs from current provider configuration");
    }
    Ok(envelope.api_key)
}

pub(crate) trait SecretStore {
    fn get(&self, secret_ref: &str) -> Result<Option<String>>;
    fn set(&self, secret_ref: &str, value: &str) -> Result<()>;
    fn delete(&self, secret_ref: &str) -> Result<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct KeyringSecretStore;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn keyring_entry(secret_ref: &str) -> Result<keyring::Entry> {
    let account = match secret_ref {
        PROVIDER_KEY_REF => PROVIDER_ACCOUNT.to_string(),
        SEARCH_KEY_REF => SEARCH_ACCOUNT.to_string(),
        MAIN_CHAT_EVENT_INTEGRITY_KEY_REF => MAIN_CHAT_EVENT_INTEGRITY_ACCOUNT.to_string(),
        ACTION_QUEUE_AUTHORITY_KEY_REF => ACTION_QUEUE_AUTHORITY_ACCOUNT.to_string(),
        TASK_STORE_AUTHORITY_KEY_REF => TASK_STORE_AUTHORITY_ACCOUNT.to_string(),
        AGENT_RUN_RECEIPT_KEY_REF => AGENT_RUN_RECEIPT_ACCOUNT.to_string(),
        value if value.starts_with(MCP_AUDIT_KEY_REF_PREFIX) => {
            let epoch = value.trim_start_matches(MCP_AUDIT_KEY_REF_PREFIX);
            if epoch.is_empty() || !epoch.chars().all(|character| character.is_ascii_digit()) {
                anyhow::bail!("invalid MCP audit secret reference");
            }
            format!("{MCP_AUDIT_ACCOUNT_PREFIX}{epoch}")
        }
        _ => anyhow::bail!("unsupported OpenLife secret reference"),
    };
    keyring::Entry::new(SERVICE, &account).context("initialize OS credential entry")
}

/// Hydrate one purpose-isolated 256-bit integrity key. Missing material is
/// created once in the OS secret owner; malformed or unreadable existing
/// material fails closed instead of silently rotating and orphaning durable
/// database authenticators.
pub(crate) fn hydrate_or_create_integrity_key(
    secret_ref: &'static str,
    store: &dyn SecretStore,
) -> Result<[u8; 32]> {
    if !matches!(
        secret_ref,
        MAIN_CHAT_EVENT_INTEGRITY_KEY_REF
            | ACTION_QUEUE_AUTHORITY_KEY_REF
            | AGENT_RUN_RECEIPT_KEY_REF
    ) {
        anyhow::bail!("unsupported OpenLife integrity key purpose");
    }
    if let Some(encoded) = store.get(secret_ref)? {
        let decoded = general_purpose::STANDARD
            .decode(encoded)
            .context("decode OpenLife integrity key material")?;
        let key: [u8; 32] = decoded
            .try_into()
            .map_err(|_| anyhow::anyhow!("OpenLife integrity key must contain exactly 32 bytes"))?;
        if key.iter().all(|byte| *byte == 0) {
            anyhow::bail!("OpenLife integrity key must not be all-zero");
        }
        return Ok(key);
    }

    create_random_integrity_key(secret_ref, store)
}

fn create_random_integrity_key(
    secret_ref: &'static str,
    store: &dyn SecretStore,
) -> Result<[u8; 32]> {
    let key = loop {
        let candidate = rand::random::<[u8; 32]>();
        if candidate.iter().any(|byte| *byte != 0) {
            break candidate;
        }
    };
    store.set(secret_ref, &general_purpose::STANDARD.encode(key))?;
    Ok(key)
}

/// A canonical database verifier cannot be rotated merely because its OS
/// secret entry is missing. When the database already exists, absence is a
/// recovery/blocker state; generating a replacement would permanently sever
/// the only authentication path for the existing canonical store.
pub(crate) fn hydrate_or_create_canonical_store_integrity_key(
    secret_ref: &'static str,
    canonical_store_path: &std::path::Path,
    store: &dyn SecretStore,
) -> Result<[u8; 32]> {
    if secret_ref != TASK_STORE_AUTHORITY_KEY_REF {
        anyhow::bail!("unsupported canonical store integrity key purpose");
    }
    if let Some(encoded) = store.get(secret_ref)? {
        let decoded = general_purpose::STANDARD
            .decode(encoded)
            .context("decode canonical store integrity key material")?;
        let key: [u8; 32] = decoded.try_into().map_err(|_| {
            anyhow::anyhow!("canonical store integrity key must contain exactly 32 bytes")
        })?;
        if key.iter().all(|byte| *byte == 0) {
            anyhow::bail!("canonical store integrity key must not be all-zero");
        }
        return Ok(key);
    }
    if canonical_store_path.exists() {
        match existing_task_store_authority_binding_state(canonical_store_path) {
            Ok(false) => {
                // Pre-v13 stores had no OS-key verifier. TaskStore will bind
                // this newly created key and quarantine pre-authority active
                // state transactionally during its schema migration.
            }
            Ok(true) => anyhow::bail!(
                "canonical TaskStore exists but its OS-owned authority key is unavailable"
            ),
            Err(error) => anyhow::bail!(
                "canonical TaskStore exists but its authority binding cannot be inspected: {error}"
            ),
        }
    }
    create_random_integrity_key(secret_ref, store)
}

fn existing_task_store_authority_binding_state(path: &std::path::Path) -> Result<bool> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .context("open existing TaskStore authority metadata read-only")?;
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) = 1 FROM sqlite_master
         WHERE type = 'table' AND name = 'task_store_metadata'",
        [],
        |row| row.get(0),
    )?;
    if !table_exists {
        return Ok(false);
    }
    let values = [
        "canonical_task_store_identity_v1",
        "canonical_task_store_slot_verifier_v1",
    ]
    .into_iter()
    .map(|key| {
        conn.query_row(
            "SELECT value FROM task_store_metadata WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
    })
    .collect::<rusqlite::Result<Vec<_>>>()?;
    match (values[0].as_ref(), values[1].as_ref()) {
        (None, None) => Ok(false),
        (Some(identity), Some(verifier)) if !identity.is_empty() && !verifier.is_empty() => {
            Ok(true)
        }
        _ => anyhow::bail!("TaskStore authority metadata is incomplete"),
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
impl SecretStore for KeyringSecretStore {
    fn get(&self, secret_ref: &str) -> Result<Option<String>> {
        match keyring_entry(secret_ref)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error).context("read secret from OS credential store"),
        }
    }

    fn set(&self, secret_ref: &str, value: &str) -> Result<()> {
        if value.trim().is_empty() {
            anyhow::bail!("refusing to store an empty secret");
        }
        keyring_entry(secret_ref)?
            .set_password(value)
            .context("write secret to OS credential store")
    }

    fn delete(&self, secret_ref: &str) -> Result<()> {
        match keyring_entry(secret_ref)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error).context("delete secret from OS credential store"),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
impl SecretStore for KeyringSecretStore {
    fn get(&self, _secret_ref: &str) -> Result<Option<String>> {
        anyhow::bail!("OS credential storage is unsupported on this platform")
    }

    fn set(&self, _secret_ref: &str, _value: &str) -> Result<()> {
        anyhow::bail!("OS credential storage is unsupported on this platform")
    }

    fn delete(&self, _secret_ref: &str) -> Result<()> {
        anyhow::bail!("OS credential storage is unsupported on this platform")
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct SecretHydrationOutcome {
    pub(crate) rewrite_config_without_plaintext: bool,
    /// Runtime capabilities disabled because their credential could not be
    /// moved into or read from the OS-owned secret store. The corresponding
    /// plaintext field is cleared before provider/tool construction.
    pub(crate) fail_closed_capabilities: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

/// Hydrate runtime-only secrets and migrate legacy plaintext without deleting the
/// old file unless every legacy secret has first been copied successfully.
pub(crate) fn hydrate_config_secrets(
    config: &mut AppConfig,
    store: &dyn SecretStore,
) -> SecretHydrationOutcome {
    let legacy_provider = !config.llm.openai_key.trim().is_empty();
    let legacy_search = !config.system.search_provider_key.trim().is_empty();
    let mut outcome = SecretHydrationOutcome::default();
    let mut migration_failed = false;
    let mut rejected_provider_reference = false;

    if legacy_provider {
        let encoded = encode_provider_secret(config, &config.llm.openai_key)
            .and_then(|encoded| store.set(PROVIDER_KEY_REF, &encoded));
        match encoded {
            Ok(()) => config.llm.openai_key_ref = Some(PROVIDER_KEY_REF.into()),
            Err(error) => {
                migration_failed = true;
                config.llm.openai_key.clear();
                config.llm.openai_key_ref = None;
                outcome
                    .fail_closed_capabilities
                    .push("provider_credential".into());
                outcome
                    .warnings
                    .push(format!("provider key migration failed: {error}"));
            }
        }
    } else if config.llm.openai_key_ref.as_deref() == Some(PROVIDER_KEY_REF) {
        match store.get(PROVIDER_KEY_REF) {
            Ok(Some(encoded)) => match hydrate_bound_provider_secret(config, &encoded) {
                Ok(secret) => config.llm.openai_key = secret,
                Err(_) => {
                    config.llm.openai_key.clear();
                    config.llm.openai_key_ref = None;
                    rejected_provider_reference = true;
                    outcome
                        .fail_closed_capabilities
                        .push("provider_credential".into());
                    outcome.warnings.push(
                        "provider key reference is unbound or belongs to another provider endpoint; reconfigure the credential"
                            .into(),
                    );
                }
            },
            Ok(None) => {
                outcome
                    .fail_closed_capabilities
                    .push("provider_credential".into());
                outcome
                    .warnings
                    .push("provider key reference has no credential".into());
            }
            Err(error) => {
                outcome
                    .fail_closed_capabilities
                    .push("provider_credential".into());
                outcome
                    .warnings
                    .push(format!("provider key hydration failed: {error}"));
            }
        }
    }

    if legacy_search {
        match store.set(SEARCH_KEY_REF, &config.system.search_provider_key) {
            Ok(()) => config.system.search_provider_key_ref = Some(SEARCH_KEY_REF.into()),
            Err(error) => {
                migration_failed = true;
                config.system.search_provider_key.clear();
                config.system.search_provider_key_ref = None;
                outcome
                    .fail_closed_capabilities
                    .push("search_provider_credential".into());
                outcome
                    .warnings
                    .push(format!("search key migration failed: {error}"));
            }
        }
    } else if config.system.search_provider_key_ref.as_deref() == Some(SEARCH_KEY_REF) {
        match store.get(SEARCH_KEY_REF) {
            Ok(Some(secret)) => config.system.search_provider_key = secret,
            Ok(None) => {
                outcome
                    .fail_closed_capabilities
                    .push("search_provider_credential".into());
                outcome
                    .warnings
                    .push("search key reference has no credential".into());
            }
            Err(error) => {
                outcome
                    .fail_closed_capabilities
                    .push("search_provider_credential".into());
                outcome
                    .warnings
                    .push(format!("search key hydration failed: {error}"));
            }
        }
    }

    outcome.rewrite_config_without_plaintext =
        ((legacy_provider || legacy_search) && !migration_failed) || rejected_provider_reference;
    outcome
}

#[derive(Debug)]
pub(crate) struct SecretWriteRollback {
    previous_values: Vec<(&'static str, Option<String>)>,
}

impl SecretWriteRollback {
    pub(crate) fn rollback(self, store: &dyn SecretStore) -> Result<()> {
        for (secret_ref, previous) in self.previous_values.into_iter().rev() {
            match previous {
                Some(value) => store.set(secret_ref, &value)?,
                None => store.delete(secret_ref)?,
            }
        }
        Ok(())
    }
}

/// Stage runtime secrets in the OS credential store. The caller must roll back
/// the returned snapshot if writing the reference-only config subsequently fails.
pub(crate) fn stage_config_secrets(
    config: &mut AppConfig,
    store: &dyn SecretStore,
) -> Result<SecretWriteRollback> {
    let mut previous_values = Vec::new();
    let provider_secret = (!config.llm.openai_key.trim().is_empty())
        .then(|| encode_provider_secret(config, &config.llm.openai_key))
        .transpose()?;
    let search_secret = (!config.system.search_provider_key.trim().is_empty())
        .then(|| config.system.search_provider_key.clone());
    for (secret_ref, value) in [
        (PROVIDER_KEY_REF, provider_secret),
        (SEARCH_KEY_REF, search_secret),
    ] {
        let Some(value) = value else {
            continue;
        };
        if value.trim().is_empty() {
            continue;
        }
        let previous = match store.get(secret_ref) {
            Ok(previous) => previous,
            Err(error) => {
                let rollback = SecretWriteRollback { previous_values };
                let rollback_error = rollback.rollback(store).err();
                return Err(match rollback_error {
                    Some(rollback_error) => anyhow::anyhow!(
                        "secret read failed: {error}; rollback also failed: {rollback_error}"
                    ),
                    None => error,
                });
            }
        };
        if let Err(error) = store.set(secret_ref, &value) {
            let rollback = SecretWriteRollback { previous_values };
            let rollback_error = rollback.rollback(store).err();
            return Err(match rollback_error {
                Some(rollback_error) => anyhow::anyhow!(
                    "secret write failed: {error}; rollback also failed: {rollback_error}"
                ),
                None => error,
            });
        }
        previous_values.push((secret_ref, previous));
    }
    if !config.llm.openai_key.trim().is_empty() {
        config.llm.openai_key_ref = Some(PROVIDER_KEY_REF.into());
    }
    if !config.system.search_provider_key.trim().is_empty() {
        config.system.search_provider_key_ref = Some(SEARCH_KEY_REF.into());
    }
    Ok(SecretWriteRollback { previous_values })
}

pub(crate) struct McpAuditKeyHydration {
    pub(crate) configs: Vec<AuditKeyConfig>,
    pub(crate) materials: Vec<AuditKeyMaterial>,
    pub(crate) config_changed: bool,
}

pub(crate) fn hydrate_or_create_mcp_audit_keys(
    mut configs: Vec<AuditKeyConfig>,
    store: &dyn SecretStore,
) -> Result<McpAuditKeyHydration> {
    configs.sort_by_key(|config| config.epoch);
    configs.dedup_by_key(|config| config.epoch);
    let mut materials = Vec::new();
    for config in &configs {
        if config.mode == KeyMode::Keychain {
            let secret_ref = config.key_ref.as_deref().ok_or_else(|| {
                anyhow::anyhow!("MCP audit keychain config has no secret reference")
            })?;
            let encoded = store
                .get(secret_ref)?
                .ok_or_else(|| anyhow::anyhow!("MCP audit keychain reference has no credential"))?;
            let decoded = general_purpose::STANDARD
                .decode(encoded)
                .context("decode MCP audit key material")?;
            let key: [u8; 32] = decoded
                .try_into()
                .map_err(|_| anyhow::anyhow!("MCP audit key must contain exactly 32 bytes"))?;
            materials.push(AuditKeyMaterial {
                config: config.clone(),
                key,
            });
        } else {
            materials.push(McpAuditStore::legacy_read_only_key_material(
                config.clone(),
            )?);
        }
    }

    let latest_epoch = configs.last().map_or(0, |config| config.epoch);
    let needs_keychain_epoch = match configs.last() {
        Some(config) => config.mode != KeyMode::Keychain,
        None => true,
    };
    if needs_keychain_epoch {
        let epoch = next_audit_epoch(latest_epoch);
        let material = create_mcp_audit_key_material(epoch, store)?;
        configs.push(material.config.clone());
        materials.push(material);
    }

    Ok(McpAuditKeyHydration {
        configs,
        materials,
        config_changed: needs_keychain_epoch,
    })
}

pub(crate) fn create_mcp_audit_key_material(
    epoch: u64,
    store: &dyn SecretStore,
) -> Result<AuditKeyMaterial> {
    let secret_ref = format!("{MCP_AUDIT_KEY_REF_PREFIX}{epoch}");
    let key = rand::random::<[u8; 32]>();
    store.set(&secret_ref, &general_purpose::STANDARD.encode(key))?;
    Ok(AuditKeyMaterial {
        config: AuditKeyConfig {
            mode: KeyMode::Keychain,
            salt_b64: None,
            env_var: None,
            key_ref: Some(secret_ref),
            epoch,
            created_at: chrono::Utc::now().to_rfc3339(),
        },
        key,
    })
}

fn next_audit_epoch(previous: u64) -> u64 {
    let timestamp = chrono::Utc::now().timestamp().max(0) as u64;
    timestamp.max(previous.saturating_add(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct KeychainCleanup<'a> {
        store: &'a dyn SecretStore,
        secret_ref: String,
    }

    impl Drop for KeychainCleanup<'_> {
        fn drop(&mut self) {
            let _ = self.store.delete(&self.secret_ref);
        }
    }

    #[derive(Default)]
    struct MemorySecretStore {
        values: Mutex<HashMap<String, String>>,
    }

    struct FailingSearchSecretStore {
        values: Mutex<HashMap<String, String>>,
    }

    #[derive(Default)]
    struct FailingProviderSecretStore;

    impl SecretStore for FailingProviderSecretStore {
        fn get(&self, _secret_ref: &str) -> Result<Option<String>> {
            Ok(None)
        }

        fn set(&self, secret_ref: &str, _value: &str) -> Result<()> {
            if secret_ref == PROVIDER_KEY_REF {
                anyhow::bail!("injected provider secret failure");
            }
            Ok(())
        }

        fn delete(&self, _secret_ref: &str) -> Result<()> {
            Ok(())
        }
    }

    impl SecretStore for FailingSearchSecretStore {
        fn get(&self, secret_ref: &str) -> Result<Option<String>> {
            Ok(self.values.lock().unwrap().get(secret_ref).cloned())
        }

        fn set(&self, secret_ref: &str, value: &str) -> Result<()> {
            if secret_ref == SEARCH_KEY_REF {
                anyhow::bail!("injected search secret failure");
            }
            self.values
                .lock()
                .unwrap()
                .insert(secret_ref.into(), value.into());
            Ok(())
        }

        fn delete(&self, secret_ref: &str) -> Result<()> {
            self.values.lock().unwrap().remove(secret_ref);
            Ok(())
        }
    }

    impl SecretStore for MemorySecretStore {
        fn get(&self, secret_ref: &str) -> Result<Option<String>> {
            Ok(self.values.lock().unwrap().get(secret_ref).cloned())
        }

        fn set(&self, secret_ref: &str, value: &str) -> Result<()> {
            self.values
                .lock()
                .unwrap()
                .insert(secret_ref.into(), value.into());
            Ok(())
        }

        fn delete(&self, secret_ref: &str) -> Result<()> {
            self.values.lock().unwrap().remove(secret_ref);
            Ok(())
        }
    }

    #[test]
    fn legacy_plaintext_is_copied_before_config_rewrite_is_allowed() {
        let store = MemorySecretStore::default();
        let mut config = AppConfig::default();
        config.llm.openai_key = "sk-provider-secret".into();
        config.system.search_provider_key = "sk-search-secret".into();

        let outcome = hydrate_config_secrets(&mut config, &store);

        assert!(outcome.rewrite_config_without_plaintext);
        assert!(outcome.warnings.is_empty());
        let encoded = store.get(PROVIDER_KEY_REF).unwrap().unwrap();
        let envelope: ProviderSecretEnvelope = serde_json::from_str(&encoded).unwrap();
        assert_eq!(envelope.api_key, "sk-provider-secret");
        assert_eq!(envelope.binding.provider, "openai");
        assert_eq!(envelope.binding.scheme, "https");
        assert_eq!(envelope.binding.host, "api.openai.com");
        assert_eq!(envelope.binding.port, 443);
        assert_eq!(envelope.binding.base_path, "/v1");
        assert_eq!(envelope.binding.credential_version, 0);
        assert_eq!(
            envelope.binding.credential_identity,
            openlife_core::llm::provider_credential_identity("sk-provider-secret")
        );
        assert_eq!(config.llm.openai_key_ref.as_deref(), Some(PROVIDER_KEY_REF));
        assert_eq!(
            config.system.search_provider_key_ref.as_deref(),
            Some(SEARCH_KEY_REF)
        );
    }

    #[test]
    fn failed_legacy_provider_migration_clears_runtime_plaintext_and_marks_degraded() {
        let mut config = AppConfig::default();
        config.llm.openai_key = "sk-plaintext-must-not-run".into();

        let outcome = hydrate_config_secrets(&mut config, &FailingProviderSecretStore);

        assert!(config.llm.openai_key.is_empty());
        assert!(config.llm.openai_key_ref.is_none());
        assert!(!outcome.rewrite_config_without_plaintext);
        assert_eq!(
            outcome.fail_closed_capabilities,
            vec!["provider_credential".to_string()]
        );
        assert!(outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("provider key migration failed")));
    }

    #[test]
    fn reference_only_config_is_hydrated_for_runtime_use() {
        let store = MemorySecretStore::default();
        let mut config = AppConfig::default();
        store
            .set(
                PROVIDER_KEY_REF,
                &encode_provider_secret(&config, "sk-provider-secret").unwrap(),
            )
            .unwrap();
        config.llm.openai_key_ref = Some(PROVIDER_KEY_REF.into());

        let outcome = hydrate_config_secrets(&mut config, &store);

        assert!(!outcome.rewrite_config_without_plaintext);
        assert_eq!(config.llm.openai_key, "sk-provider-secret");
    }

    #[test]
    fn unbound_legacy_keychain_reference_fails_closed_and_requires_reconfiguration() {
        let store = MemorySecretStore::default();
        store.set(PROVIDER_KEY_REF, "sk-legacy-unbound").unwrap();
        let mut config = AppConfig::default();
        config.llm.openai_key_ref = Some(PROVIDER_KEY_REF.into());

        let outcome = hydrate_config_secrets(&mut config, &store);

        assert!(config.llm.openai_key.is_empty());
        assert!(config.llm.openai_key_ref.is_none());
        assert!(outcome.rewrite_config_without_plaintext);
        assert!(outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("reconfigure")));
    }

    #[test]
    fn provider_endpoint_or_generation_drift_cannot_hydrate_a_bound_key() {
        let store = MemorySecretStore::default();
        let original = AppConfig::default();
        store
            .set(
                PROVIDER_KEY_REF,
                &encode_provider_secret(&original, "sk-official").unwrap(),
            )
            .unwrap();

        for mut drifted in [
            {
                let mut config = original.clone();
                config.llm.openai_base = "https://capture.example/v1".into();
                config
            },
            {
                let mut config = original.clone();
                config.llm.credential_version = 1;
                config
            },
        ] {
            drifted.llm.openai_key_ref = Some(PROVIDER_KEY_REF.into());
            let outcome = hydrate_config_secrets(&mut drifted, &store);
            assert!(drifted.llm.openai_key.is_empty());
            assert!(drifted.llm.openai_key_ref.is_none());
            assert!(outcome.rewrite_config_without_plaintext);
        }
    }

    #[test]
    fn staged_secret_write_rolls_back_when_the_second_secret_fails() {
        let store = FailingSearchSecretStore {
            values: Mutex::new(HashMap::from([(
                PROVIDER_KEY_REF.into(),
                "sk-old-provider".into(),
            )])),
        };
        let mut config = AppConfig::default();
        config.llm.openai_key = "sk-new-provider".into();
        config.system.search_provider_key = "sk-new-search".into();

        let result = stage_config_secrets(&mut config, &store);

        assert!(result.is_err());
        assert_eq!(
            store.get(PROVIDER_KEY_REF).unwrap().as_deref(),
            Some("sk-old-provider")
        );
        assert_eq!(store.get(SEARCH_KEY_REF).unwrap(), None);
    }

    #[test]
    fn mcp_audit_key_is_random_keychain_material_and_restart_hydrates_same_epoch() {
        let store = MemorySecretStore::default();
        let first = hydrate_or_create_mcp_audit_keys(Vec::new(), &store).unwrap();
        assert!(first.config_changed);
        assert_eq!(first.configs.len(), 1);
        assert_eq!(first.configs[0].mode, KeyMode::Keychain);
        assert!(first.configs[0].key_ref.is_some());
        assert_ne!(first.materials[0].key, [0u8; 32]);
        let serialized = serde_json::to_string(&first.configs).unwrap();
        assert!(!serialized.contains(&general_purpose::STANDARD.encode(first.materials[0].key)));

        let original_key = first.materials[0].key;
        let restarted = hydrate_or_create_mcp_audit_keys(first.configs, &store).unwrap();
        assert!(!restarted.config_changed);
        assert_eq!(restarted.materials[0].key, original_key);
    }

    #[test]
    fn legacy_audit_epoch_is_kept_for_reads_but_new_writes_use_keychain_epoch() {
        let store = MemorySecretStore::default();
        let legacy = AuditKeyConfig::default();
        let hydrated = hydrate_or_create_mcp_audit_keys(vec![legacy], &store).unwrap();
        assert_eq!(hydrated.materials.len(), 2);
        assert_eq!(hydrated.configs.last().unwrap().mode, KeyMode::Keychain);
        assert!(hydrated.configs[1].epoch > hydrated.configs[0].epoch);
    }

    #[test]
    fn integrity_keys_are_stable_and_purpose_isolated() {
        let store = MemorySecretStore::default();
        let directory = tempfile::tempdir().unwrap();
        let task_store_path = directory.path().join("tasks.db");
        let event_key =
            hydrate_or_create_integrity_key(MAIN_CHAT_EVENT_INTEGRITY_KEY_REF, &store).unwrap();
        let event_key_after_restart =
            hydrate_or_create_integrity_key(MAIN_CHAT_EVENT_INTEGRITY_KEY_REF, &store).unwrap();
        let action_key =
            hydrate_or_create_integrity_key(ACTION_QUEUE_AUTHORITY_KEY_REF, &store).unwrap();
        let task_store_key = hydrate_or_create_canonical_store_integrity_key(
            TASK_STORE_AUTHORITY_KEY_REF,
            &task_store_path,
            &store,
        )
        .unwrap();
        let task_store_key_after_restart = hydrate_or_create_canonical_store_integrity_key(
            TASK_STORE_AUTHORITY_KEY_REF,
            &task_store_path,
            &store,
        )
        .unwrap();
        let agent_run_key =
            hydrate_or_create_integrity_key(AGENT_RUN_RECEIPT_KEY_REF, &store).unwrap();
        let agent_run_key_after_restart =
            hydrate_or_create_integrity_key(AGENT_RUN_RECEIPT_KEY_REF, &store).unwrap();

        assert_eq!(event_key_after_restart, event_key);
        assert_eq!(task_store_key_after_restart, task_store_key);
        assert_eq!(agent_run_key_after_restart, agent_run_key);
        assert_ne!(action_key, event_key);
        assert_ne!(task_store_key, event_key);
        assert_ne!(task_store_key, action_key);
        assert_ne!(agent_run_key, event_key);
        assert_ne!(agent_run_key, action_key);
        assert_ne!(agent_run_key, task_store_key);
        assert!(event_key.iter().any(|byte| *byte != 0));
        assert!(action_key.iter().any(|byte| *byte != 0));
        assert!(task_store_key.iter().any(|byte| *byte != 0));
        assert!(agent_run_key.iter().any(|byte| *byte != 0));
    }

    #[test]
    fn malformed_integrity_key_fails_closed_without_rotation() {
        let store = MemorySecretStore::default();
        store
            .set(MAIN_CHAT_EVENT_INTEGRITY_KEY_REF, "not-base64")
            .unwrap();

        let error =
            hydrate_or_create_integrity_key(MAIN_CHAT_EVENT_INTEGRITY_KEY_REF, &store).unwrap_err();
        assert!(error
            .to_string()
            .contains("decode OpenLife integrity key material"));
        assert_eq!(
            store
                .get(MAIN_CHAT_EVENT_INTEGRITY_KEY_REF)
                .unwrap()
                .as_deref(),
            Some("not-base64")
        );
    }

    #[test]
    fn existing_task_store_never_rotates_a_missing_authority_key() {
        let directory = tempfile::tempdir().unwrap();
        let task_store_path = directory.path().join("tasks.db");
        std::fs::write(&task_store_path, b"existing canonical store sentinel").unwrap();
        let store = MemorySecretStore::default();

        let error = hydrate_or_create_canonical_store_integrity_key(
            TASK_STORE_AUTHORITY_KEY_REF,
            &task_store_path,
            &store,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("exists"), "{error}");
        assert!(store.get(TASK_STORE_AUTHORITY_KEY_REF).unwrap().is_none());
    }

    #[test]
    fn new_task_store_slot_creates_one_stable_os_owned_authority_key() {
        let directory = tempfile::tempdir().unwrap();
        let task_store_path = directory.path().join("tasks.db");
        let store = MemorySecretStore::default();

        let first = hydrate_or_create_canonical_store_integrity_key(
            TASK_STORE_AUTHORITY_KEY_REF,
            &task_store_path,
            &store,
        )
        .unwrap();
        let second = hydrate_or_create_canonical_store_integrity_key(
            TASK_STORE_AUTHORITY_KEY_REF,
            &task_store_path,
            &store,
        )
        .unwrap();

        assert_eq!(first, second);
        assert!(first.iter().any(|byte| *byte != 0));
    }

    #[test]
    #[ignore = "writes and deletes one random credential in the real OS keychain"]
    fn real_os_keychain_round_trip_uses_only_a_secret_reference() {
        let store = KeyringSecretStore;
        let epoch = chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or_default()
            .unsigned_abs();
        let secret_ref = format!("{MCP_AUDIT_KEY_REF_PREFIX}{epoch}");
        let _cleanup = KeychainCleanup {
            store: &store,
            secret_ref: secret_ref.clone(),
        };
        let secret = general_purpose::STANDARD.encode(rand::random::<[u8; 32]>());

        store.delete(&secret_ref).expect("clear prior collision");
        store.set(&secret_ref, &secret).expect("write OS keychain");
        assert_eq!(
            store.get(&secret_ref).expect("read OS keychain").as_deref(),
            Some(secret.as_str())
        );
        store.delete(&secret_ref).expect("delete OS keychain");
        assert_eq!(store.get(&secret_ref).expect("confirm deletion"), None);
    }
}
