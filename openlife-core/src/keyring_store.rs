//! OS Keyring integration for secure API key storage.
//!
//! Uses the system keyring (macOS Keychain, Windows Credential Manager,
//! Linux Secret Service) to store API keys encrypted at rest.
//!
//! Falls back gracefully if the keyring backend is unavailable.

const SERVICE: &str = "ai.openlife.desktop";

/// Initialize the native credential store for the current platform.
/// Must be called once at startup before using get/set functions.
pub fn init_native_store() {
    let _ = keyring::use_native_store(false);
}

pub fn get_api_key(provider: &str) -> Option<String> {
    let entry = keyring_core::Entry::new(SERVICE, provider).ok()?;
    entry.get_password().ok()
}

pub fn set_api_key(provider: &str, key: &str) -> bool {
    if let Ok(entry) = keyring_core::Entry::new(SERVICE, provider) {
        entry.set_password(key).is_ok()
    } else {
        false
    }
}

pub fn delete_api_key(provider: &str) -> bool {
    if let Ok(entry) = keyring_core::Entry::new(SERVICE, provider) {
        entry.delete_credential().is_ok()
    } else {
        false
    }
}

pub fn migrate_to_keyring(provider: &str, key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    set_api_key(provider, key)
}

pub fn migrate_all(keys: &[(&str, &str)]) -> usize {
    keys.iter()
        .filter(|(provider, key)| migrate_to_keyring(provider, key))
        .count()
}
