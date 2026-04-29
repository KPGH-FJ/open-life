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
    records: HashMap<String, PluginRecord>,
    enabled_overrides: BTreeMap<String, bool>,
}

impl PluginRegistry {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            records: HashMap::new(),
            enabled_overrides: BTreeMap::new(),
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
        self.enabled_overrides
            .insert(plugin_id.to_string(), enabled);
        if let Some(record) = self.records.get_mut(plugin_id) {
            record.enabled = enabled && record.error.is_none();
            record.manifest.enabled = record.enabled;
            Ok(())
        } else {
            Err(anyhow::anyhow!("plugin not found: {}", plugin_id))
        }
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
            .flat_map(|record| record.manifest.skills.clone())
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
