use crate::life_model::LifeModel;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Semantic version bump rule based on model changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionBump {
    Major, // structural changes
    Minor, // new dimensions
    Patch, // micro-adjustments / daily save
}

fn parse_version(v: &str) -> (u64, u64, u64) {
    let parts: Vec<&str> = v.split('.').collect();
    let a = parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0);
    let b = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let c = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    (a, b, c)
}

fn bump_version(v: &str, bump: VersionBump) -> String {
    let (major, minor, patch) = parse_version(v);
    match bump {
        VersionBump::Major => format!("{}.{}.{}", major + 1, 0, 0),
        VersionBump::Minor => format!("{}.{}.{}", major, minor + 1, 0),
        VersionBump::Patch => format!("{}.{}.{}", major, minor, patch + 1),
    }
}

/// Compare two models and decide which version component to bump.
pub fn detect_version_bump(before: &LifeModel, after: &LifeModel) -> VersionBump {
    // Structural: identity values / personality traits / role_definition / voice style changed count
    let identity_before = before.identity.values.len()
        + before.identity.personality_traits.len()
        + if before.identity.role_definition.primary_role.is_empty() {
            0
        } else {
            1
        };
    let identity_after = after.identity.values.len()
        + after.identity.personality_traits.len()
        + if after.identity.role_definition.primary_role.is_empty() {
            0
        } else {
            1
        };
    if identity_after != identity_before {
        return VersionBump::Major;
    }

    // New dimensions: goals or skills added
    let goals_before = before.goals.short_term.len()
        + before.goals.medium_term.len()
        + before.goals.long_term.len()
        + before.goals.life_goals.len();
    let goals_after = after.goals.short_term.len()
        + after.goals.medium_term.len()
        + after.goals.long_term.len()
        + after.goals.life_goals.len();
    let skills_before = before.capabilities.skills.len();
    let skills_after = after.capabilities.skills.len();
    if goals_after > goals_before || skills_after > skills_before {
        return VersionBump::Minor;
    }

    // Default to patch for any other change
    VersionBump::Patch
}

/// Update metadata timestamps and semantic version before persisting.
pub fn prepare_model_for_save(previous: Option<&LifeModel>, next: &mut LifeModel) -> VersionBump {
    let now = chrono::Local::now().to_rfc3339();
    if next.metadata.created_at.is_empty() {
        next.metadata.created_at = previous
            .map(|model| model.metadata.created_at.clone())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| now.clone());
    }
    next.metadata.updated_at = now;

    let bump = previous
        .map(|before| detect_version_bump(before, next))
        .unwrap_or(VersionBump::Patch);
    let base_version = previous
        .map(|model| model.metadata.version.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if next.metadata.version.is_empty() {
                "0.1.0"
            } else {
                next.metadata.version.as_str()
            }
        });
    next.metadata.version = bump_version(base_version, bump);
    bump
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifeModelVersion {
    pub version: String,
    pub timestamp: String,
    pub tag: String,
    pub note: String,
    pub yaml_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct VersionManifest {
    versions: Vec<VersionMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionMetadata {
    version: String,
    timestamp: String,
    tag: String,
    note: String,
}

pub struct VersionManager {
    versions_dir: PathBuf,
}

impl VersionManager {
    pub fn new(versions_dir: impl Into<PathBuf>) -> Self {
        let versions_dir = versions_dir.into();
        fs::create_dir_all(&versions_dir).ok();
        Self { versions_dir }
    }
}

impl Default for VersionManager {
    fn default() -> Self {
        Self::new(
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".openlife")
                .join("life-model")
                .join("versions"),
        )
    }
}

impl VersionManager {
    fn manifest_path(&self) -> PathBuf {
        self.versions_dir.join("index.json")
    }

    fn load_manifest(&self) -> Result<VersionManifest> {
        let path = self.manifest_path();
        if !path.exists() {
            return Ok(VersionManifest::default());
        }
        let text =
            fs::read_to_string(&path).with_context(|| format!("读取版本索引失败: {:?}", path))?;
        serde_json::from_str(&text).with_context(|| format!("解析版本索引失败: {:?}", path))
    }

    fn save_manifest(&self, manifest: &VersionManifest) -> Result<()> {
        let path = self.manifest_path();
        let text = serde_json::to_string_pretty(manifest).context("序列化版本索引失败")?;
        fs::write(&path, text).with_context(|| format!("写入版本索引失败: {:?}", path))
    }

    pub fn snapshot(&self, model: &LifeModel, tag: &str, note: &str) -> Result<LifeModelVersion> {
        let yaml_content = serde_yaml::to_string(model).context("序列化人生模型失败")?;
        let timestamp = chrono::Local::now().to_rfc3339();
        let safe_time = timestamp.replace(":", "-");
        let version = format!("{}_{}", model.metadata.version.replace(".", "_"), safe_time);
        let filename = format!("{}.yaml", version);
        let path = self.versions_dir.join(&filename);

        fs::write(&path, &yaml_content).with_context(|| format!("写入版本文件失败: {:?}", path))?;

        let mut manifest = self.load_manifest().unwrap_or_default();
        manifest.versions.retain(|entry| entry.version != version);
        manifest.versions.push(VersionMetadata {
            version: version.clone(),
            timestamp: timestamp.clone(),
            tag: tag.to_string(),
            note: note.to_string(),
        });
        manifest
            .versions
            .sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        let _ = self.save_manifest(&manifest);

        Ok(LifeModelVersion {
            version,
            timestamp,
            tag: tag.to_string(),
            note: note.to_string(),
            yaml_content,
        })
    }

    /// Create a snapshot specifically for patch application (before/after).
    pub fn snapshot_for_patch(
        &self,
        model: &LifeModel,
        patch_id: &str,
        phase: &str, // "before" or "after"
    ) -> Result<LifeModelVersion> {
        let tag = format!("patch:{}:{}", patch_id, phase);
        let note = format!("Snapshot {} patch {}", phase, patch_id);
        self.snapshot(model, &tag, &note)
    }

    /// List snapshots associated with a specific patch.
    pub fn get_patch_snapshots(&self, patch_id: &str) -> Result<Vec<LifeModelVersion>> {
        let all = self.list_versions()?;
        let prefix = format!("patch:{}:", patch_id);
        Ok(all
            .into_iter()
            .filter(|v| v.tag.starts_with(&prefix))
            .collect())
    }

    pub fn has_snapshot_tag_on_date(&self, tag: &str, date: &str) -> Result<bool> {
        let manifest = self.load_manifest().unwrap_or_default();
        Ok(manifest
            .versions
            .iter()
            .any(|entry| entry.tag == tag && entry.timestamp.starts_with(date)))
    }

    pub fn list_versions(&self) -> Result<Vec<LifeModelVersion>> {
        let mut versions = Vec::new();
        if !self.versions_dir.exists() {
            return Ok(versions);
        }
        let manifest = self.load_manifest().unwrap_or_default();
        let mut entries: Vec<_> = fs::read_dir(&self.versions_dir)
            .context("读取版本目录失败")?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("yaml") {
                continue;
            }
            let content = fs::read_to_string(&path).context("读取版本文件失败")?;
            let version = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let metadata = manifest
                .versions
                .iter()
                .find(|entry| entry.version == version);
            let timestamp = metadata
                .map(|entry| entry.timestamp.clone())
                .unwrap_or_else(|| version.split('_').last().unwrap_or("").replace("-", ":"));
            versions.push(LifeModelVersion {
                version,
                timestamp,
                tag: metadata.map(|entry| entry.tag.clone()).unwrap_or_default(),
                note: metadata.map(|entry| entry.note.clone()).unwrap_or_default(),
                yaml_content: content,
            });
        }
        Ok(versions)
    }

    pub fn restore(&self, version: &str) -> Result<LifeModel> {
        let path = self.versions_dir.join(format!("{}.yaml", version));
        let content =
            fs::read_to_string(&path).with_context(|| format!("未找到版本: {}", version))?;
        let model: LifeModel = serde_yaml::from_str(&content).context("解析版本 YAML 失败")?;
        Ok(model)
    }

    pub fn diff(&self, v1: &str, v2: &str) -> Result<String> {
        let path1 = self.versions_dir.join(format!("{}.yaml", v1));
        let path2 = self.versions_dir.join(format!("{}.yaml", v2));
        let content1 = fs::read_to_string(&path1).with_context(|| format!("未找到版本: {}", v1))?;
        let content2 = fs::read_to_string(&path2).with_context(|| format!("未找到版本: {}", v2))?;

        let diff = similar::TextDiff::from_lines(&content1, &content2);
        let mut output = String::new();
        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                similar::ChangeTag::Delete => "-",
                similar::ChangeTag::Insert => "+",
                similar::ChangeTag::Equal => " ",
            };
            output.push_str(&format!("{}{}", sign, change.value()));
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life_model::LifeModel;

    #[test]
    fn parse_and_bump_version() {
        assert_eq!(parse_version("1.2.3"), (1, 2, 3));
        assert_eq!(bump_version("1.2.3", VersionBump::Patch), "1.2.4");
        assert_eq!(bump_version("1.2.3", VersionBump::Minor), "1.3.0");
        assert_eq!(bump_version("1.2.3", VersionBump::Major), "2.0.0");
    }

    #[test]
    fn detect_version_bump_major_on_identity_change() {
        let before = LifeModel::default_model();
        let mut after = before.clone();
        after.identity.values.push(crate::life_model::ValueItem {
            name: "创新".into(),
            weight: 7,
            description: "".into(),
        });
        assert_eq!(detect_version_bump(&before, &after), VersionBump::Major);
    }

    #[test]
    fn detect_version_bump_minor_on_new_goal() {
        let before = LifeModel::default_model();
        let mut after = before.clone();
        after.goals.short_term.push(crate::life_model::GoalItem {
            name: "新目标".into(),
            description: "".into(),
            priority: 5,
            status: "active".into(),
            progress: 0.0,
            deadline: None,
            milestones: vec![],
            related_memories: vec![],
        });
        assert_eq!(detect_version_bump(&before, &after), VersionBump::Minor);
    }

    #[test]
    fn detect_version_bump_minor_on_new_skill() {
        let before = LifeModel::default_model();
        let mut after = before.clone();
        after.capabilities.skills.push(crate::life_model::Skill {
            name: "Rust".into(),
            proficiency: 5,
            description: "".into(),
        });
        assert_eq!(detect_version_bump(&before, &after), VersionBump::Minor);
    }

    #[test]
    fn detect_version_bump_patch_when_no_structural_change() {
        let before = LifeModel::default_model();
        let mut after = before.clone();
        after.state.emotional_state.current_mood = "开心".into();
        assert_eq!(detect_version_bump(&before, &after), VersionBump::Patch);
    }

    #[test]
    fn version_manager_snapshot_list_restore_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = VersionManager {
            versions_dir: dir.path().to_path_buf(),
        };
        let model = LifeModel::default_model();
        let snap = mgr.snapshot(&model, "test", "note").unwrap();
        assert!(!snap.version.is_empty());

        let list = mgr.list_versions().unwrap();
        assert!(!list.is_empty());
        assert_eq!(list[0].tag, "test");
        assert_eq!(list[0].note, "note");

        let restored = mgr.restore(&snap.version).unwrap();
        assert_eq!(restored.identity.name, model.identity.name);
    }

    #[test]
    fn version_manager_diff_between_two_versions() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = VersionManager {
            versions_dir: dir.path().to_path_buf(),
        };
        let mut m1 = LifeModel::default_model();
        m1.identity.name = "A".into();
        let mut m2 = m1.clone();
        m2.identity.name = "B".into();

        let v1 = mgr.snapshot(&m1, "v1", "").unwrap();
        let v2 = mgr.snapshot(&m2, "v2", "").unwrap();

        let diff = mgr.diff(&v1.version, &v2.version).unwrap();
        assert!(
            diff.contains("A") || diff.contains("B") || diff.contains("+") || diff.contains("-")
        );
    }

    #[test]
    fn prepare_model_for_save_bumps_version_and_updates_timestamp() {
        let before = LifeModel::default_model();
        let mut after = before.clone();
        after.goals.short_term.push(crate::life_model::GoalItem {
            name: "完成 Alpha".into(),
            description: "".into(),
            priority: 8,
            status: "active".into(),
            progress: 0.0,
            deadline: None,
            milestones: vec![],
            related_memories: vec![],
        });
        let bump = prepare_model_for_save(Some(&before), &mut after);
        assert_eq!(bump, VersionBump::Minor);
        assert_eq!(after.metadata.version, "0.2.0");
        assert!(!after.metadata.updated_at.is_empty());
    }

    #[test]
    fn version_manager_detects_snapshot_tag_on_date() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = VersionManager {
            versions_dir: dir.path().to_path_buf(),
        };
        let model = LifeModel::default_model();
        let snap = mgr.snapshot(&model, "auto:daily-save", "daily").unwrap();
        let date = snap
            .timestamp
            .split('T')
            .next()
            .unwrap_or_default()
            .to_string();
        assert!(mgr
            .has_snapshot_tag_on_date("auto:daily-save", &date)
            .unwrap());
        assert!(!mgr
            .has_snapshot_tag_on_date("auto:evolution", &date)
            .unwrap());
    }
}
