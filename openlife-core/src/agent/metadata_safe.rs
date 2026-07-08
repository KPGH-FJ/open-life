use ring::digest::{digest, SHA256};
use serde_json::Value;

pub fn metadata_safe_value_digest(value: &Value) -> (usize, String) {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    metadata_safe_text_digest(&serialized)
}

pub fn metadata_safe_text_digest(text: &str) -> (usize, String) {
    let bytes = text.as_bytes();
    let hash = digest(&SHA256, bytes);
    let hex = hash
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    (bytes.len(), format!("sha256:{hex}"))
}

pub fn metadata_safe_value_preview(value: &Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    metadata_safe_text_preview(&serialized)
}

pub fn metadata_safe_text_preview(text: &str) -> String {
    format!("{} bytes redacted", text.len())
}
