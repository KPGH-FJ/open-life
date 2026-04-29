use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

/// Unified manifest for all tools available in OpenLife.
/// Covers both built-in tools and external MCP tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolManifest {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub description: String,
    pub parameters: Value,        // JSON Schema object
    pub permission_level: String, // "low" | "medium" | "high"
    #[serde(default)]
    pub risk_level: String,
    pub version: String,
    pub source: ToolSource,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub requires_confirmation: bool,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Optional tags for recommendation engine matching.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolSource {
    /// Built-in tool implemented natively in openlife-core.
    BuiltIn,
    /// Tool provided by an MCP server.
    Mcp { server_name: String },
    /// Tool exposed by an A2A agent or bridge.
    A2A { agent_name: String },
    /// Tool declared by a local plugin manifest.
    Plugin { plugin_id: String },
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
        let name = name.into();
        let permission_level = permission_level.into();
        Self {
            id: name.clone(),
            name,
            description: description.into(),
            parameters,
            risk_level: permission_level.clone(),
            permission_level,
            version: version.into(),
            source,
            capabilities: Vec::new(),
            requires_confirmation: false,
            enabled: true,
            tags: Vec::new(),
        }
    }

    pub fn normalized(mut self) -> Self {
        if self.id.is_empty() {
            self.id = self.name.clone();
        }
        if self.risk_level.is_empty() {
            self.risk_level = self.permission_level.clone();
        }
        if self.permission_level.is_empty() {
            self.permission_level = self.risk_level.clone();
        }
        if self.capabilities.is_empty() {
            self.capabilities = Self::infer_capabilities(&self.name);
        }
        self.requires_confirmation = self.requires_confirmation
            || self.risk_level == "high"
            || self.capabilities.iter().any(|c| {
                matches!(
                    c.as_str(),
                    "write" | "filesystem" | "memory" | "lifemodel" | "external_side_effect"
                )
            });
        self
    }

    pub fn infer_capabilities(name: &str) -> Vec<String> {
        let lower = name.to_lowercase();
        let mut caps = BTreeSet::new();
        if lower.contains("write")
            || lower.contains("create")
            || lower.contains("delete")
            || lower.contains("remove")
            || lower.contains("update")
        {
            caps.insert("write".to_string());
        } else {
            caps.insert("read".to_string());
        }
        if lower.contains("search")
            || lower.contains("fetch")
            || lower.contains("request")
            || lower.contains("web")
            || lower.contains("http")
        {
            caps.insert("network".to_string());
        }
        if lower.contains("file") || lower.contains("path") || lower.contains("shell") {
            caps.insert("filesystem".to_string());
        }
        if lower.contains("memory") {
            caps.insert("memory".to_string());
        }
        if lower.contains("life") || lower.contains("goal") {
            caps.insert("lifemodel".to_string());
        }
        if lower.contains("send") || lower.contains("post") || lower.contains("github") {
            caps.insert("external_side_effect".to_string());
        }
        caps.into_iter().collect()
    }

    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self.normalized()
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
