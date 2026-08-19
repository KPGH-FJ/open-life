use serde::{Deserialize, Serialize};

/// Metadata-safe execution context returned with a Chat or Work turn.
///
/// The canonical runtimes currently expose only the generation projection
/// consumed by the Workbench. Internal model reasoning is deliberately not a
/// product contract.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProductAgentTrace {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_result: Option<serde_json::Value>,
}
