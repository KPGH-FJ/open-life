use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Unified manifest for all tools available in OpenLife.
/// Covers both built-in tools and external MCP tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolManifest {
    pub name: String,
    pub description: String,
    pub parameters: Value,        // JSON Schema object
    pub permission_level: String, // "low" | "medium" | "high"
    pub version: String,
    pub source: ToolSource,
    /// Optional tags for recommendation engine matching.
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolSource {
    /// Built-in tool implemented natively in openlife-core.
    BuiltIn,
    /// Tool provided by an MCP server.
    Mcp { server_name: String },
}

impl ToolManifest {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        permission_level: impl Into<String>,
        version: impl Into<String>,
        source: ToolSource,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            permission_level: permission_level.into(),
            version: version.into(),
            source,
            tags: Vec::new(),
        }
    }

    /// Infer permission level from tool name when not explicitly provided.
    pub fn infer_permission_level(name: &str) -> String {
        let lower = name.to_lowercase();
        if lower.contains("write")
            || lower.contains("delete")
            || lower.contains("remove")
            || lower.contains("create")
            || lower.contains("exec")
            || lower.contains("run")
            || lower.contains("shell")
            || lower.contains("bash")
            || lower.contains("chmod")
        {
            "high".to_string()
        } else if lower.contains("search")
            || lower.contains("fetch")
            || lower.contains("request")
            || lower.contains("query")
            || lower.contains("github")
            || lower.contains("slack")
            || lower.contains("email")
        {
            "medium".to_string()
        } else {
            "low".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_new_has_default_tags() {
        let m = ToolManifest::new(
            "test_tool",
            "A test tool",
            serde_json::json!({}),
            "low",
            "1.0.0",
            ToolSource::BuiltIn,
        );
        assert_eq!(m.name, "test_tool");
        assert_eq!(m.permission_level, "low");
        assert!(m.tags.is_empty());
    }

    #[test]
    fn infer_permission_level_high_for_write() {
        assert_eq!(ToolManifest::infer_permission_level("write_file"), "high");
        assert_eq!(ToolManifest::infer_permission_level("delete_row"), "high");
        assert_eq!(ToolManifest::infer_permission_level("run_shell"), "high");
        assert_eq!(ToolManifest::infer_permission_level("exec_cmd"), "high");
    }

    #[test]
    fn infer_permission_level_medium_for_search() {
        assert_eq!(ToolManifest::infer_permission_level("web_search"), "medium");
        assert_eq!(ToolManifest::infer_permission_level("fetch_url"), "medium");
        assert_eq!(
            ToolManifest::infer_permission_level("github_issue"),
            "medium"
        );
    }

    #[test]
    fn infer_permission_level_low_default() {
        assert_eq!(ToolManifest::infer_permission_level("hello_world"), "low");
        assert_eq!(ToolManifest::infer_permission_level("calculator"), "low");
    }
}
