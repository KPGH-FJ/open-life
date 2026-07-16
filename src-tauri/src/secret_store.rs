use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use openlife_core::config::AppConfig;
use openlife_core::mcp_audit::{
    AuditKeyConfig, AuditKeyMaterial, KeyMode, McpAuditPendingSecretEffectPermit, McpAuditStore,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const SERVICE: &str = "com.openlife.desktop";
const PROVIDER_ACCOUNT: &str = "provider-api-key";
const SEARCH_ACCOUNT: &str = "search-provider-api-key";
const MAIN_CHAT_EVENT_INTEGRITY_ACCOUNT: &str = "main-chat-event-integrity-key-v1";
const ACTION_QUEUE_AUTHORITY_ACCOUNT: &str = "action-queue-authority-key-v1";
const TASK_STORE_AUTHORITY_ACCOUNT: &str = "task-store-authority-key-v1";
const AGENT_RUN_RECEIPT_ACCOUNT: &str = "agent-run-receipt-key-v1";
const MCP_AUDIT_ACCOUNT_PREFIX: &str = "mcp-audit-key-epoch-";
const MCP_AUDIT_STORE_ACCOUNT_PREFIX: &str = "mcp-audit-key-store-";
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
pub(crate) const MCP_AUDIT_STORE_KEY_REF_PREFIX: &str =
    "keychain://com.openlife.desktop/mcp-audit-key-store-";

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

pub(crate) trait SecretStore: Send + Sync {
    fn get(&self, secret_ref: &str) -> Result<Option<String>>;
    fn set(&self, secret_ref: &str, value: &str) -> Result<()>;
    fn delete(&self, secret_ref: &str) -> Result<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct KeyringSecretStore;

fn keyring_account_for_secret_ref(secret_ref: &str) -> Result<String> {
    Ok(match secret_ref {
        PROVIDER_KEY_REF => PROVIDER_ACCOUNT.to_string(),
        SEARCH_KEY_REF => SEARCH_ACCOUNT.to_string(),
        MAIN_CHAT_EVENT_INTEGRITY_KEY_REF => MAIN_CHAT_EVENT_INTEGRITY_ACCOUNT.to_string(),
        ACTION_QUEUE_AUTHORITY_KEY_REF => ACTION_QUEUE_AUTHORITY_ACCOUNT.to_string(),
        TASK_STORE_AUTHORITY_KEY_REF => TASK_STORE_AUTHORITY_ACCOUNT.to_string(),
        AGENT_RUN_RECEIPT_KEY_REF => AGENT_RUN_RECEIPT_ACCOUNT.to_string(),
        value if value.starts_with(MCP_AUDIT_STORE_KEY_REF_PREFIX) => {
            let suffix = value
                .strip_prefix(MCP_AUDIT_STORE_KEY_REF_PREFIX)
                .ok_or_else(|| anyhow::anyhow!("invalid store-bound MCP audit secret reference"))?;
            let (store_identity, epoch) = suffix
                .rsplit_once("-epoch-")
                .ok_or_else(|| anyhow::anyhow!("invalid store-bound MCP audit secret reference"))?;
            let parsed_identity = parse_random_mcp_audit_store_identity(store_identity)?;
            if store_identity != parsed_identity.simple().to_string()
                || epoch.is_empty()
                || !epoch.chars().all(|character| character.is_ascii_digit())
            {
                anyhow::bail!("invalid store-bound MCP audit secret reference");
            }
            format!("{MCP_AUDIT_STORE_ACCOUNT_PREFIX}{store_identity}-epoch-{epoch}")
        }
        value if value.starts_with(MCP_AUDIT_KEY_REF_PREFIX) => {
            let epoch = value
                .strip_prefix(MCP_AUDIT_KEY_REF_PREFIX)
                .ok_or_else(|| anyhow::anyhow!("invalid MCP audit secret reference"))?;
            if epoch.is_empty() || !epoch.chars().all(|character| character.is_ascii_digit()) {
                anyhow::bail!("invalid MCP audit secret reference");
            }
            format!("{MCP_AUDIT_ACCOUNT_PREFIX}{epoch}")
        }
        _ => anyhow::bail!("unsupported OpenLife secret reference"),
    })
}

fn parse_random_mcp_audit_store_identity(value: &str) -> Result<uuid::Uuid> {
    let identity =
        uuid::Uuid::parse_str(value).context("validate MCP audit canonical store identity")?;
    if identity.is_nil() || identity.get_version_num() != 4 {
        anyhow::bail!("MCP audit canonical store identity must be a random UUIDv4");
    }
    Ok(identity)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn keyring_entry(secret_ref: &str) -> Result<keyring::Entry> {
    let account = keyring_account_for_secret_ref(secret_ref)?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum McpAuditSecretCreateCommitState {
    NotCommitted,
    VisibleOrExistenceUnknown,
}

#[derive(Debug)]
pub(crate) struct McpAuditSecretCreateError {
    commit_state: McpAuditSecretCreateCommitState,
    secret_ref: String,
    detail: String,
}

impl McpAuditSecretCreateError {
    #[cfg(test)]
    pub(crate) fn commit_state(&self) -> McpAuditSecretCreateCommitState {
        self.commit_state
    }
}

impl std::fmt::Display for McpAuditSecretCreateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "mcp_audit_secret_create_{:?}:ref={}:{}",
            self.commit_state, self.secret_ref, self.detail
        )
    }
}

impl std::error::Error for McpAuditSecretCreateError {}

#[cfg(test)]
pub(crate) struct McpAuditCreatedSecretReceipt {
    secret_ref: String,
}

#[cfg(test)]
impl std::fmt::Debug for McpAuditCreatedSecretReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpAuditCreatedSecretReceipt")
            .field("secret_ref", &self.secret_ref)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
pub(crate) struct McpAuditCreatedKeyMaterial {
    pub(crate) material: AuditKeyMaterial,
}

pub(crate) struct McpAuditSecretCreationPlan {
    config: AuditKeyConfig,
    key: Zeroizing<[u8; 32]>,
    encoded_key: Zeroizing<String>,
    expected_digest: String,
}

/// Typed create-only ownership for one exact MCP-audit credential reference.
///
/// The reservation is acquired before a Prepared reference can become
/// durable and is retained through the credential get/set/post-read sequence.
/// This prevents two independently reserved SQLite stores from publishing
/// competing canonical references for the same OS credential account.
pub(crate) struct McpAuditSecretReferenceReservation {
    secret_ref: String,
    _owner: openlife_core::sqlite_migration::SqliteSlotOwnerReservation,
}

impl std::fmt::Debug for McpAuditSecretReferenceReservation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpAuditSecretReferenceReservation")
            .field("secret_ref", &self.secret_ref)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for McpAuditSecretCreationPlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpAuditSecretCreationPlan")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl McpAuditSecretCreationPlan {
    pub(crate) fn material(&self) -> AuditKeyMaterial {
        AuditKeyMaterial {
            config: self.config.clone(),
            key: *self.key,
        }
    }

    pub(crate) fn expected_digest(&self) -> &str {
        &self.expected_digest
    }

    pub(crate) fn reserve_create_only(
        &self,
        store: &dyn SecretStore,
    ) -> std::result::Result<McpAuditSecretReferenceReservation, McpAuditSecretCreateError> {
        let secret_ref = self
            .config
            .key_ref
            .as_deref()
            .expect("MCP audit secret plans always carry a key reference");
        reserve_new_mcp_audit_secret(secret_ref, store)
    }

    pub(crate) fn execute(
        &self,
        store: &dyn SecretStore,
        permit: McpAuditPendingSecretEffectPermit<'_>,
        reservation: McpAuditSecretReferenceReservation,
    ) -> std::result::Result<(), McpAuditSecretCreateError> {
        let secret_ref = self
            .config
            .key_ref
            .as_deref()
            .expect("MCP audit secret plans always carry a key reference");
        permit
            .validate_at_effect_edge(self.config.epoch, secret_ref, &self.expected_digest)
            .map_err(|error| McpAuditSecretCreateError {
                commit_state: McpAuditSecretCreateCommitState::NotCommitted,
                secret_ref: secret_ref.to_string(),
                detail: format!("mcp_audit_secret_effect_authority_rejected:{error}"),
            })?;
        write_new_mcp_audit_secret_with_reservation(
            secret_ref,
            &self.encoded_key,
            store,
            reservation,
        )
    }
}

fn mcp_audit_secret_value_digest(encoded_key: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"openlife-mcp-audit-secret-value-v1\0");
    digest.update(encoded_key.as_bytes());
    format!("sha256:{:x}", digest.finalize())
}

#[cfg(test)]
pub(crate) struct McpAuditKeyHydration {
    pub(crate) configs: Vec<AuditKeyConfig>,
    pub(crate) materials: Vec<AuditKeyMaterial>,
    pub(crate) config_changed: bool,
}

#[cfg(test)]
pub(crate) fn hydrate_or_create_mcp_audit_keys(
    mut configs: Vec<AuditKeyConfig>,
    store: &dyn SecretStore,
) -> Result<McpAuditKeyHydration> {
    for pair in configs.windows(2) {
        if pair[0].epoch >= pair[1].epoch {
            anyhow::bail!("MCP audit key epochs must be strictly increasing");
        }
    }
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
        let created = create_legacy_test_mcp_audit_key_material(epoch, store)?;
        configs.push(created.material.config.clone());
        materials.push(created.material);
    }

    Ok(McpAuditKeyHydration {
        configs,
        materials,
        config_changed: needs_keychain_epoch,
    })
}

#[cfg(test)]
pub(crate) fn hydrate_or_create_store_bound_mcp_audit_keys(
    mut configs: Vec<AuditKeyConfig>,
    store_identity: &str,
    store: &dyn SecretStore,
) -> Result<McpAuditKeyHydration> {
    let store_identity = parse_random_mcp_audit_store_identity(store_identity)?;
    let mut materials = hydrate_existing_mcp_audit_key_materials(&configs, store)?;
    let store_identity = store_identity.simple().to_string();
    let active_is_store_bound = configs.last().is_some_and(|config| {
        let expected_reference = format!(
            "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{store_identity}-epoch-{}",
            config.epoch
        );
        config.mode == KeyMode::Keychain
            && config.key_ref.as_deref() == Some(expected_reference.as_str())
    });
    if !active_is_store_bound {
        let latest_epoch = configs.last().map_or(0, |config| config.epoch);
        let epoch = next_audit_epoch(latest_epoch);
        let created = create_store_bound_mcp_audit_key_material(epoch, &store_identity, store)?;
        configs.push(created.material.config.clone());
        materials.push(created.material);
    }
    Ok(McpAuditKeyHydration {
        configs,
        materials,
        config_changed: !active_is_store_bound,
    })
}

pub(crate) fn hydrate_existing_mcp_audit_key_materials(
    configs: &[AuditKeyConfig],
    store: &dyn SecretStore,
) -> Result<Vec<AuditKeyMaterial>> {
    for pair in configs.windows(2) {
        if pair[0].epoch >= pair[1].epoch {
            anyhow::bail!("MCP audit key epochs must be strictly increasing");
        }
    }
    let mut materials = Vec::with_capacity(configs.len());
    for config in configs {
        if config.mode == KeyMode::Keychain {
            let secret_ref = config.key_ref.as_deref().ok_or_else(|| {
                anyhow::anyhow!("MCP audit keychain config has no secret reference")
            })?;
            let encoded = Zeroizing::new(store.get(secret_ref)?.ok_or_else(|| {
                anyhow::anyhow!("MCP audit keychain reference has no credential")
            })?);
            let decoded = Zeroizing::new(
                general_purpose::STANDARD
                    .decode(encoded.as_bytes())
                    .context("decode MCP audit key material")?,
            );
            let key: [u8; 32] = decoded
                .as_slice()
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
    Ok(materials)
}

#[cfg(test)]
fn create_legacy_test_mcp_audit_key_material(
    epoch: u64,
    store: &dyn SecretStore,
) -> Result<McpAuditCreatedKeyMaterial> {
    let secret_ref = format!("{MCP_AUDIT_KEY_REF_PREFIX}{epoch}");
    let key = rand::random::<[u8; 32]>();
    write_new_mcp_audit_secret(&secret_ref, &general_purpose::STANDARD.encode(key), store)?;
    Ok(McpAuditCreatedKeyMaterial {
        material: AuditKeyMaterial {
            config: AuditKeyConfig {
                mode: KeyMode::Keychain,
                salt_b64: None,
                env_var: None,
                key_ref: Some(secret_ref),
                epoch,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
            key,
        },
    })
}

#[cfg(test)]
pub(crate) fn create_store_bound_mcp_audit_key_material(
    epoch: u64,
    store_identity: &str,
    store: &dyn SecretStore,
) -> Result<McpAuditCreatedKeyMaterial> {
    let plan = plan_store_bound_mcp_audit_key_material(epoch, store_identity)?;
    let material = plan.material();
    let secret_ref = plan
        .config
        .key_ref
        .as_deref()
        .expect("store-bound test plan has key ref");
    write_new_mcp_audit_secret(secret_ref, &plan.encoded_key, store)?;
    Ok(McpAuditCreatedKeyMaterial { material })
}

pub(crate) fn plan_store_bound_mcp_audit_key_material(
    epoch: u64,
    store_identity: &str,
) -> Result<McpAuditSecretCreationPlan> {
    let store_identity = parse_random_mcp_audit_store_identity(store_identity)?;
    let secret_ref = format!(
        "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{}-epoch-{epoch}",
        store_identity.simple()
    );
    let key = Zeroizing::new(rand::random::<[u8; 32]>());
    let encoded_key = Zeroizing::new(general_purpose::STANDARD.encode(*key));
    Ok(McpAuditSecretCreationPlan {
        config: AuditKeyConfig {
            mode: KeyMode::Keychain,
            salt_b64: None,
            env_var: None,
            key_ref: Some(secret_ref),
            epoch,
            created_at: chrono::Utc::now().to_rfc3339(),
        },
        key,
        expected_digest: mcp_audit_secret_value_digest(&encoded_key),
        encoded_key,
    })
}

pub(crate) fn next_store_bound_mcp_audit_epoch(previous: u64) -> u64 {
    next_audit_epoch(previous)
}

pub(crate) enum McpAuditPendingSecretRecovery {
    Exact(AuditKeyMaterial),
    Missing,
}

impl std::fmt::Debug for McpAuditPendingSecretRecovery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exact(material) => formatter
                .debug_tuple("Exact")
                .field(&material.config)
                .finish(),
            Self::Missing => formatter.write_str("Missing"),
        }
    }
}

pub(crate) fn recover_pending_mcp_audit_secret(
    config: &AuditKeyConfig,
    expected_digest: &str,
    store: &dyn SecretStore,
) -> Result<McpAuditPendingSecretRecovery> {
    if config.mode != KeyMode::Keychain {
        anyhow::bail!("mcp_audit_pending_secret_not_keychain");
    }
    let secret_ref = config
        .key_ref
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("mcp_audit_pending_secret_reference_missing"))?;
    let encoded = match store.get(secret_ref) {
        Ok(Some(encoded)) => Zeroizing::new(encoded),
        Ok(None) => return Ok(McpAuditPendingSecretRecovery::Missing),
        Err(error) => {
            let _ = error;
            anyhow::bail!("mcp_audit_pending_secret_visibility_unknown:ref={secret_ref}");
        }
    };
    if mcp_audit_secret_value_digest(&encoded) != expected_digest {
        anyhow::bail!("mcp_audit_pending_secret_digest_mismatch:ref={secret_ref}");
    }
    let decoded = Zeroizing::new(
        general_purpose::STANDARD
            .decode(encoded.as_bytes())
            .context("decode pending MCP audit key material")?,
    );
    let key: [u8; 32] = decoded
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("pending MCP audit key must contain exactly 32 bytes"))?;
    Ok(McpAuditPendingSecretRecovery::Exact(AuditKeyMaterial {
        config: config.clone(),
        key,
    }))
}

/// Test-only composition of reservation plus credential effect. Product paths
/// reserve before publishing Prepared and call the reserved effect directly.
#[cfg(test)]
fn write_new_mcp_audit_secret(
    secret_ref: &str,
    encoded_key: &str,
    store: &dyn SecretStore,
) -> std::result::Result<McpAuditCreatedSecretReceipt, McpAuditSecretCreateError> {
    let reservation = reserve_new_mcp_audit_secret(secret_ref, store)?;
    write_new_mcp_audit_secret_with_reservation(secret_ref, encoded_key, store, reservation)?;
    Ok(McpAuditCreatedSecretReceipt {
        secret_ref: secret_ref.to_string(),
    })
}

fn reserve_new_mcp_audit_secret(
    secret_ref: &str,
    store: &dyn SecretStore,
) -> std::result::Result<McpAuditSecretReferenceReservation, McpAuditSecretCreateError> {
    let create_error = |commit_state, detail: String| McpAuditSecretCreateError {
        commit_state,
        secret_ref: secret_ref.to_string(),
        detail,
    };
    let owner = reserve_global_mcp_audit_secret_reference(secret_ref).map_err(|error| {
        create_error(
            McpAuditSecretCreateCommitState::NotCommitted,
            format!("reservation_failed:{error}"),
        )
    })?;
    match store.get(secret_ref) {
        Ok(Some(_)) => {
            return Err(create_error(
                McpAuditSecretCreateCommitState::NotCommitted,
                "reference_already_exists".into(),
            ));
        }
        Ok(None) => {}
        Err(error) => {
            let _ = error;
            return Err(create_error(
                McpAuditSecretCreateCommitState::VisibleOrExistenceUnknown,
                "precreate_existence_unknown".into(),
            ));
        }
    }
    Ok(McpAuditSecretReferenceReservation {
        secret_ref: secret_ref.to_string(),
        _owner: owner,
    })
}

fn write_new_mcp_audit_secret_with_reservation(
    secret_ref: &str,
    encoded_key: &str,
    store: &dyn SecretStore,
    reservation: McpAuditSecretReferenceReservation,
) -> std::result::Result<(), McpAuditSecretCreateError> {
    let create_error = |commit_state, detail: String| McpAuditSecretCreateError {
        commit_state,
        secret_ref: secret_ref.to_string(),
        detail,
    };
    if reservation.secret_ref != secret_ref {
        return Err(create_error(
            McpAuditSecretCreateCommitState::NotCommitted,
            "reservation_reference_mismatch".into(),
        ));
    }
    if let Err(error) = store.set(secret_ref, encoded_key) {
        let _ = error;
        return Err(create_error(
            McpAuditSecretCreateCommitState::VisibleOrExistenceUnknown,
            "set_outcome_unknown".into(),
        ));
    }
    match store.get(secret_ref) {
        Ok(Some(observed)) => {
            let observed = Zeroizing::new(observed);
            if observed.as_str() == encoded_key {
                Ok(())
            } else {
                Err(create_error(
                    McpAuditSecretCreateCommitState::VisibleOrExistenceUnknown,
                    "postcreate_value_mismatch".into(),
                ))
            }
        }
        Ok(None) => Err(create_error(
            McpAuditSecretCreateCommitState::VisibleOrExistenceUnknown,
            "postcreate_visibility_missing".into(),
        )),
        Err(error) => {
            let _ = error;
            Err(create_error(
                McpAuditSecretCreateCommitState::VisibleOrExistenceUnknown,
                "postcreate_visibility_unknown".into(),
            ))
        }
    }
}

fn mcp_audit_secret_reference_lock_root() -> std::path::PathBuf {
    #[cfg(test)]
    {
        if let Ok(path) = std::env::var("OPENLIFE_TEST_MCP_AUDIT_SECRET_LOCK_ROOT") {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return std::path::PathBuf::from(trimmed);
            }
        }
        return std::env::temp_dir().join(format!(
            "openlife-mcp-audit-secret-ref-locks-v1-test-{}",
            std::process::id()
        ));
    }
    #[cfg(not(test))]
    {
        dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("ai.openlife.secret-ref-locks-v1")
    }
}

fn reserve_global_mcp_audit_secret_reference(
    secret_ref: &str,
) -> Result<openlife_core::sqlite_migration::SqliteSlotOwnerReservation> {
    // Validate before hashing so arbitrary caller-controlled strings cannot
    // consume the global lock namespace.
    let _ = keyring_account_for_secret_ref(secret_ref)?;
    let root = mcp_audit_secret_reference_lock_root();
    std::fs::create_dir_all(&root).context("create MCP audit secret reservation root")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .context("restrict MCP audit secret reservation root")?;
    }
    let digest = format!("{:x}", Sha256::digest(secret_ref.as_bytes()));
    let synthetic_slot = root.join(format!("{digest}.secret-slot"));
    let canonical_slot = openlife_core::sqlite_migration::canonical_sqlite_slot(
        &synthetic_slot,
        "mcp_audit_secret_ref",
    )?;
    let reservation = openlife_core::sqlite_migration::SqliteSlotOwnerLease::reserve_no_create(
        &canonical_slot,
        "mcp_audit_secret_ref",
    )?;
    if reservation.existing_database_len()?.is_some() {
        anyhow::bail!("mcp_audit_secret_reservation_slot_contaminated");
    }
    Ok(reservation)
}

#[cfg(test)]
std::thread_local! {
    static FIXED_MCP_AUDIT_EPOCH: std::cell::RefCell<Option<u64>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) struct FixedMcpAuditEpochGuard {
    previous: Option<u64>,
}

#[cfg(test)]
impl Drop for FixedMcpAuditEpochGuard {
    fn drop(&mut self) {
        FIXED_MCP_AUDIT_EPOCH.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

/// Fix only the audit epoch source while retaining the complete product key
/// creation path. Thread-local scope prevents parallel-test contamination.
#[cfg(test)]
pub(crate) fn inject_fixed_mcp_audit_epoch_for_test(epoch: u64) -> FixedMcpAuditEpochGuard {
    let previous = FIXED_MCP_AUDIT_EPOCH.with(|slot| slot.replace(Some(epoch)));
    FixedMcpAuditEpochGuard { previous }
}

#[cfg(test)]
pub(crate) fn fixed_mcp_audit_epoch_for_test() -> Option<u64> {
    FIXED_MCP_AUDIT_EPOCH.with(|slot| *slot.borrow())
}

fn next_audit_epoch(previous: u64) -> u64 {
    #[cfg(test)]
    if let Some(epoch) = fixed_mcp_audit_epoch_for_test() {
        return epoch.max(previous.saturating_add(1));
    }
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

    #[derive(Clone, Copy)]
    enum AmbiguousPostCreateRead {
        Error,
        Missing,
        Mismatch,
    }

    struct AmbiguousCreateSecretStore {
        mode: AmbiguousPostCreateRead,
        value: Mutex<Option<String>>,
        gets: Mutex<usize>,
        deletes: Mutex<usize>,
    }

    impl AmbiguousCreateSecretStore {
        fn new(mode: AmbiguousPostCreateRead) -> Self {
            Self {
                mode,
                value: Mutex::new(None),
                gets: Mutex::new(0),
                deletes: Mutex::new(0),
            }
        }
    }

    impl SecretStore for AmbiguousCreateSecretStore {
        fn get(&self, _secret_ref: &str) -> Result<Option<String>> {
            let mut gets = self.gets.lock().unwrap();
            *gets += 1;
            if *gets == 1 {
                return Ok(None);
            }
            match self.mode {
                AmbiguousPostCreateRead::Error => {
                    anyhow::bail!("SUPER_SECRET_SHOULD_NOT_LEAK")
                }
                AmbiguousPostCreateRead::Missing => Ok(None),
                AmbiguousPostCreateRead::Mismatch => Ok(Some("different-value".into())),
            }
        }

        fn set(&self, _secret_ref: &str, value: &str) -> Result<()> {
            *self.value.lock().unwrap() = Some(value.to_string());
            Ok(())
        }

        fn delete(&self, _secret_ref: &str) -> Result<()> {
            *self.deletes.lock().unwrap() += 1;
            *self.value.lock().unwrap() = None;
            Ok(())
        }
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
    fn mcp_audit_secret_plan_and_receipt_debug_never_expose_key_or_digest() {
        let identity = uuid::Uuid::new_v4();
        let plan = plan_store_bound_mcp_audit_key_material(77, &identity.to_string()).unwrap();
        let raw = plan.encoded_key.to_string();
        let digest = plan.expected_digest().to_string();
        let plan_debug = format!("{plan:?}");
        assert!(!plan_debug.contains(&raw));
        assert!(!plan_debug.contains(&digest));

        let store = MemorySecretStore::default();
        let secret_ref = plan.config.key_ref.as_deref().unwrap();
        let receipt = write_new_mcp_audit_secret(secret_ref, &plan.encoded_key, &store).unwrap();
        let receipt_debug = format!("{receipt:?}");
        assert!(!receipt_debug.contains(&raw));
        assert!(!receipt_debug.contains(&digest));
    }

    #[test]
    fn pending_secret_recovery_requires_the_exact_domain_separated_digest() {
        let identity = uuid::Uuid::new_v4();
        let plan = plan_store_bound_mcp_audit_key_material(78, &identity.to_string()).unwrap();
        let material = plan.material();
        let digest = plan.expected_digest().to_string();
        let store = MemorySecretStore::default();
        let secret_ref = plan.config.key_ref.as_deref().unwrap();
        write_new_mcp_audit_secret(secret_ref, &plan.encoded_key, &store).unwrap();

        match recover_pending_mcp_audit_secret(&material.config, &digest, &store).unwrap() {
            McpAuditPendingSecretRecovery::Exact(recovered) => {
                assert_eq!(recovered.key, material.key)
            }
            McpAuditPendingSecretRecovery::Missing => panic!("exact secret must recover"),
        }
        assert!(recover_pending_mcp_audit_secret(
            &material.config,
            &format!("sha256:{}", "00".repeat(32)),
            &store,
        )
        .unwrap_err()
        .to_string()
        .contains("mcp_audit_pending_secret_digest_mismatch"));
        store
            .delete(material.config.key_ref.as_deref().unwrap())
            .unwrap();
        assert!(matches!(
            recover_pending_mcp_audit_secret(&material.config, &digest, &store).unwrap(),
            McpAuditPendingSecretRecovery::Missing
        ));
    }

    #[test]
    fn ambiguous_postcreate_reads_never_trigger_blind_secret_deletion() {
        let secret_ref = format!(
            "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{}-epoch-79",
            uuid::Uuid::new_v4().simple()
        );
        let encoded = general_purpose::STANDARD.encode([0x79; 32]);
        for mode in [
            AmbiguousPostCreateRead::Error,
            AmbiguousPostCreateRead::Missing,
            AmbiguousPostCreateRead::Mismatch,
        ] {
            let store = AmbiguousCreateSecretStore::new(mode);
            let error = write_new_mcp_audit_secret(&secret_ref, &encoded, &store).unwrap_err();
            assert_eq!(
                error.commit_state(),
                McpAuditSecretCreateCommitState::VisibleOrExistenceUnknown
            );
            assert_eq!(
                store.value.lock().unwrap().as_deref(),
                Some(encoded.as_str())
            );
            assert_eq!(*store.deletes.lock().unwrap(), 0);
            let rendered = error.to_string();
            assert!(!rendered.contains(&encoded));
            assert!(!rendered.contains("SUPER_SECRET_SHOULD_NOT_LEAK"));
        }
    }

    struct ProcessFileSecretStore {
        secret_path: std::path::PathBuf,
        entered_path: Option<std::path::PathBuf>,
        release_path: Option<std::path::PathBuf>,
    }

    impl SecretStore for ProcessFileSecretStore {
        fn get(&self, _secret_ref: &str) -> Result<Option<String>> {
            match std::fs::read_to_string(&self.secret_path) {
                Ok(value) => Ok(Some(value)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error.into()),
            }
        }

        fn set(&self, _secret_ref: &str, value: &str) -> Result<()> {
            std::fs::write(&self.secret_path, value)?;
            if let (Some(entered), Some(release)) =
                (self.entered_path.as_ref(), self.release_path.as_ref())
            {
                std::fs::write(entered, b"entered")?;
                for _ in 0..1_000 {
                    if release.exists() {
                        return Ok(());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                anyhow::bail!("timed out waiting for cross-process secret release barrier");
            }
            Ok(())
        }

        fn delete(&self, _secret_ref: &str) -> Result<()> {
            match std::fs::remove_file(&self.secret_path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.into()),
            }
        }
    }

    #[test]
    #[ignore = "subprocess worker for cross-process MCP audit secret reservation proof"]
    fn mcp_audit_secret_child_worker() {
        if std::env::var("OPENLIFE_MCP_SECRET_CHILD_WORKER").as_deref() != Ok("1") {
            return;
        }
        let secret_path = std::path::PathBuf::from(
            std::env::var("OPENLIFE_MCP_SECRET_CHILD_SECRET_PATH").unwrap(),
        );
        let result_path = std::path::PathBuf::from(
            std::env::var("OPENLIFE_MCP_SECRET_CHILD_RESULT_PATH").unwrap(),
        );
        let entered_path = std::env::var("OPENLIFE_MCP_SECRET_CHILD_ENTERED_PATH")
            .ok()
            .map(std::path::PathBuf::from);
        let release_path = std::env::var("OPENLIFE_MCP_SECRET_CHILD_RELEASE_PATH")
            .ok()
            .map(std::path::PathBuf::from);
        let store = ProcessFileSecretStore {
            secret_path,
            entered_path,
            release_path,
        };
        let secret_ref = std::env::var("OPENLIFE_MCP_SECRET_CHILD_REF").unwrap();
        let value = std::env::var("OPENLIFE_MCP_SECRET_CHILD_VALUE").unwrap();
        let outcome = write_new_mcp_audit_secret(&secret_ref, &value, &store)
            .map(|_| "ok".to_string())
            .unwrap_or_else(|error| format!("err:{error}"));
        std::fs::write(result_path, outcome).unwrap();
    }

    fn wait_for_path(path: &std::path::Path) {
        for _ in 0..1_000 {
            if path.exists() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("timed out waiting for {}", path.display());
    }

    fn wait_for_child(mut child: std::process::Child) -> std::process::ExitStatus {
        for _ in 0..1_000 {
            if let Some(status) = child.try_wait().unwrap() {
                return status;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let _ = child.kill();
        panic!("timed out waiting for MCP audit secret child process");
    }

    #[test]
    fn store_bound_secret_create_only_is_serialized_across_processes() {
        let directory = tempfile::tempdir().unwrap();
        let lock_root = directory.path().join("shared-lock-root");
        let secret_path = directory.path().join("credential-value");
        let entered_path = directory.path().join("k1-entered");
        let release_path = directory.path().join("release-k1");
        let first_result = directory.path().join("k1-result");
        let second_result = directory.path().join("k2-result");
        let identity = uuid::Uuid::new_v4();
        let secret_ref = format!(
            "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{}-epoch-501",
            identity.simple()
        );
        let test_binary = std::env::current_exe().unwrap();
        let worker_name = "secret_store::tests::mcp_audit_secret_child_worker";
        let child_base = |result_path: &std::path::Path, value: &str| {
            let mut command = std::process::Command::new(&test_binary);
            command
                .arg("--exact")
                .arg(worker_name)
                .arg("--ignored")
                .arg("--nocapture")
                .env("OPENLIFE_MCP_SECRET_CHILD_WORKER", "1")
                .env("OPENLIFE_TEST_MCP_AUDIT_SECRET_LOCK_ROOT", &lock_root)
                .env("OPENLIFE_MCP_SECRET_CHILD_SECRET_PATH", &secret_path)
                .env("OPENLIFE_MCP_SECRET_CHILD_RESULT_PATH", result_path)
                .env("OPENLIFE_MCP_SECRET_CHILD_REF", &secret_ref)
                .env("OPENLIFE_MCP_SECRET_CHILD_VALUE", value)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            command
        };

        let mut first_command = child_base(&first_result, "K1-stable-bytes");
        first_command
            .env("OPENLIFE_MCP_SECRET_CHILD_ENTERED_PATH", &entered_path)
            .env("OPENLIFE_MCP_SECRET_CHILD_RELEASE_PATH", &release_path);
        let first = first_command.spawn().unwrap();
        wait_for_path(&entered_path);

        let second = child_base(&second_result, "K2-must-not-overwrite")
            .spawn()
            .unwrap();
        assert!(wait_for_child(second).success());
        std::fs::write(&release_path, b"release").unwrap();
        assert!(wait_for_child(first).success());

        assert_eq!(std::fs::read_to_string(&first_result).unwrap(), "ok");
        let second_outcome = std::fs::read_to_string(&second_result).unwrap();
        assert!(
            second_outcome.contains("sqlite_slot_owner_lease_unavailable"),
            "{second_outcome}"
        );
        assert_eq!(
            std::fs::read_to_string(&secret_path).unwrap(),
            "K1-stable-bytes"
        );
        let lock_entries = std::fs::read_dir(&lock_root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(lock_entries.len(), 1);
        assert!(lock_entries[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with(".secret-slot.openlife-owner.lock"));
        assert!(!lock_root.join("credential-value").exists());
        assert!(!lock_entries[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains(&identity.simple().to_string()));
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
        let _epoch = inject_fixed_mcp_audit_epoch_for_test(6_590);
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
    fn store_bound_audit_reference_maps_to_a_distinct_valid_keychain_account() {
        let store_identity = uuid::Uuid::new_v4();
        let secret_ref = format!(
            "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{}-epoch-44",
            store_identity.simple()
        );
        assert_eq!(
            keyring_account_for_secret_ref(&secret_ref).unwrap(),
            format!(
                "{MCP_AUDIT_STORE_ACCOUNT_PREFIX}{}-epoch-44",
                store_identity.simple()
            )
        );
        assert!(keyring_account_for_secret_ref(&format!("{secret_ref}-trailing")).is_err());
        assert!(keyring_account_for_secret_ref(
            "keychain://com.openlife.desktop/mcp-audit-key-store-not-a-uuid-epoch-44"
        )
        .is_err());
        assert!(keyring_account_for_secret_ref(&format!(
            "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{MCP_AUDIT_STORE_KEY_REF_PREFIX}{}-epoch-44",
            store_identity.simple()
        ))
        .is_err());
        assert!(keyring_account_for_secret_ref(
            "keychain://com.openlife.desktop/mcp-audit-key-store-00000000000010008000000000000000-epoch-44"
        )
        .is_err());
    }

    #[test]
    fn concurrent_store_bound_secret_creation_is_globally_create_only() {
        let store = std::sync::Arc::new(MemorySecretStore::default());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let identity = uuid::Uuid::new_v4();
        let secret_ref = format!(
            "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{}-epoch-77",
            identity.simple()
        );
        let mut handles = Vec::new();
        for value in ["first-secret", "second-secret"] {
            let store = store.clone();
            let barrier = barrier.clone();
            let secret_ref = secret_ref.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                write_new_mcp_audit_secret(&secret_ref, value, store.as_ref())
                    .map(|_| value.to_string())
            }));
        }
        let outcomes = handles
            .into_iter()
            .map(|handle| handle.join().expect("secret creation thread"))
            .collect::<Vec<_>>();
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(outcomes.iter().filter(|result| result.is_err()).count(), 1);
        let winning_value = outcomes
            .iter()
            .find_map(|result| result.as_ref().ok())
            .expect("one winner");
        assert_eq!(
            store.get(&secret_ref).unwrap().as_deref(),
            Some(winning_value.as_str())
        );
    }

    #[test]
    fn legacy_audit_epoch_is_kept_for_reads_but_new_writes_use_keychain_epoch() {
        let _epoch = inject_fixed_mcp_audit_epoch_for_test(6_591);
        let store = MemorySecretStore::default();
        let legacy = AuditKeyConfig::default();
        let hydrated = hydrate_or_create_mcp_audit_keys(vec![legacy], &store).unwrap();
        assert_eq!(hydrated.materials.len(), 2);
        assert_eq!(hydrated.configs.last().unwrap().mode, KeyMode::Keychain);
        assert!(hydrated.configs[1].epoch > hydrated.configs[0].epoch);
    }

    #[test]
    fn store_bound_hydration_rejects_nonmonotonic_epochs_without_normalizing() {
        let store = MemorySecretStore::default();
        let identity = uuid::Uuid::new_v4();
        let configs = [12u64, 11u64]
            .into_iter()
            .map(|epoch| AuditKeyConfig {
                mode: KeyMode::Keychain,
                salt_b64: None,
                env_var: None,
                key_ref: Some(format!(
                    "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{}-epoch-{epoch}",
                    identity.simple()
                )),
                epoch,
                created_at: "2026-07-13T00:00:00Z".into(),
            })
            .collect::<Vec<_>>();

        let error = match hydrate_or_create_store_bound_mcp_audit_keys(
            configs,
            &identity.to_string(),
            &store,
        ) {
            Ok(_) => panic!("nonmonotonic epochs must be rejected"),
            Err(error) => error,
        };

        assert!(error
            .to_string()
            .contains("epochs must be strictly increasing"));
        assert!(store.values.lock().unwrap().is_empty());
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
