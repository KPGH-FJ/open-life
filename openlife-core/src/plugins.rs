use crate::skills::SkillManifest;
use crate::tool_manifest::ToolManifest;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    #[serde(default)]
    pub tools: Vec<ToolManifest>,
    #[serde(default)]
    pub skills: Vec<SkillManifest>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub settings_schema: Value,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_trust_level")]
    pub trust_level: String,
}

fn default_trust_level() -> String {
    "local".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRecord {
    pub manifest: PluginManifest,
    pub path: String,
    pub enabled: bool,
    pub error: Option<String>,
}

pub struct PluginRegistry {
    root: PathBuf,
    overrides_path: PathBuf,
    records: HashMap<String, PluginRecord>,
    enabled_overrides: BTreeMap<String, bool>,
}

impl PluginRegistry {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let overrides_path = root.join("enabled_overrides.json");
        let enabled_overrides = Self::load_enabled_overrides(&overrides_path).unwrap_or_default();
        Self {
            root,
            overrides_path,
            records: HashMap::new(),
            enabled_overrides,
        }
    }

    pub fn reload(&mut self) -> Result<Vec<PluginRecord>> {
        std::fs::create_dir_all(&self.root)?;
        self.records.clear();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            let manifest_path = if path.is_dir() {
                path.join("plugin.json")
            } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if path.file_name().and_then(|s| s.to_str()) == Some("enabled_overrides.json") {
                    continue;
                }
                path
            } else {
                continue;
            };
            let record = Self::load_manifest(&manifest_path, &self.enabled_overrides);
            self.records.insert(record.manifest.id.clone(), record);
        }
        Ok(self.list())
    }

    pub fn list(&self) -> Vec<PluginRecord> {
        let mut records: Vec<_> = self.records.values().cloned().collect();
        records.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
        records
    }

    pub fn enable(&mut self, plugin_id: &str, enabled: bool) -> Result<()> {
        if let Some(record) = self.records.get_mut(plugin_id) {
            record.enabled = enabled && record.error.is_none();
            record.manifest.enabled = record.enabled;
            self.enabled_overrides
                .insert(plugin_id.to_string(), enabled);
            self.save_enabled_overrides()?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("plugin not found: {}", plugin_id))
        }
    }

    fn load_enabled_overrides(path: &Path) -> Result<BTreeMap<String, bool>> {
        if !path.exists() {
            return Ok(BTreeMap::new());
        }
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text)
            .with_context(|| format!("invalid plugin enabled overrides file {}", path.display()))
    }

    fn save_enabled_overrides(&self) -> Result<()> {
        if let Some(parent) = self.overrides_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(&self.enabled_overrides)?;
        std::fs::write(&self.overrides_path, text)?;
        Ok(())
    }

    pub fn enabled_tools(&self) -> Vec<ToolManifest> {
        self.records
            .values()
            .filter(|record| record.enabled && record.error.is_none())
            .flat_map(|record| record.manifest.tools.clone())
            .map(ToolManifest::normalized)
            .collect()
    }

    pub fn enabled_skills(&self) -> Vec<SkillManifest> {
        self.records
            .values()
            .filter(|record| record.enabled && record.error.is_none())
            .flat_map(|record| {
                record
                    .manifest
                    .skills
                    .clone()
                    .into_iter()
                    .map(|skill| skill.as_plugin_declarative_only(&record.manifest.id))
            })
            .collect()
    }

    fn load_manifest(manifest_path: &Path, overrides: &BTreeMap<String, bool>) -> PluginRecord {
        let path_text = manifest_path.display().to_string();
        match std::fs::read_to_string(manifest_path)
            .with_context(|| format!("failed to read {}", path_text))
            .and_then(|text| {
                serde_json::from_str::<PluginManifest>(&text)
                    .with_context(|| format!("invalid plugin manifest {}", path_text))
            }) {
            Ok(mut manifest) => {
                let enabled = overrides
                    .get(&manifest.id)
                    .copied()
                    .unwrap_or(manifest.enabled);
                manifest.enabled = enabled;
                PluginRecord {
                    manifest,
                    path: path_text,
                    enabled,
                    error: None,
                }
            }
            Err(e) => {
                let id = manifest_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("invalid_plugin")
                    .to_string();
                PluginRecord {
                    manifest: PluginManifest {
                        id,
                        name: "Invalid Plugin".into(),
                        version: "0.0.0".into(),
                        description: "Manifest failed to load".into(),
                        author: "unknown".into(),
                        tools: vec![],
                        skills: vec![],
                        permissions: vec![],
                        settings_schema: Value::Null,
                        enabled: false,
                        trust_level: default_trust_level(),
                    },
                    path: path_text,
                    enabled: false,
                    error: Some(e.to_string()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_plugin(root: &Path, enabled: bool) {
        let plugin_dir = root.join("demo");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let manifest = serde_json::json!({
            "id": "demo",
            "name": "Demo",
            "version": "0.1.0",
            "description": "Demo plugin",
            "author": "test",
            "enabled": enabled,
            "tools": [],
            "skills": [],
            "permissions": [],
            "settingsSchema": {},
            "trustLevel": "local"
        });
        std::fs::write(
            plugin_dir.join("plugin.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn enable_override_persists_across_registry_restarts() {
        let dir = tempfile::tempdir().unwrap();
        write_plugin(dir.path(), false);

        let mut registry = PluginRegistry::new(dir.path());
        registry.reload().unwrap();
        assert!(!registry.list()[0].enabled);

        registry.enable("demo", true).unwrap();
        assert!(registry.list()[0].enabled);

        let mut restarted = PluginRegistry::new(dir.path());
        restarted.reload().unwrap();
        let records = restarted.list();
        assert_eq!(records.len(), 1);
        assert!(records[0].enabled);
        assert!(records[0].manifest.enabled);
    }

    #[test]
    fn enable_unknown_plugin_does_not_write_override() {
        let dir = tempfile::tempdir().unwrap();
        let mut registry = PluginRegistry::new(dir.path());

        assert!(registry.enable("missing", true).is_err());
        assert!(!dir.path().join("enabled_overrides.json").exists());
    }
}
