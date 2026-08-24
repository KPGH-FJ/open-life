//! Read-only inventory and preview for the retired YAML LifeModel owner.
//!
//! This module never writes, renames, or deletes legacy data. It reads the
//! exact YAML tree so omitted fields cannot be manufactured by serde defaults.

use super::v2::{
    LifeModelDocumentV2, LifeModelItemV2, LifeModelSectionV2, LifeModelTypedDiffV2,
    LifeModelTypedOperationV2, LifeModelUserValueV2, DEFAULT_LIFE_MODEL_V2_MODEL_ID,
};
use super::LifeModel;
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const MAX_LEGACY_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_LEGACY_ITEMS: usize = 4_096;
pub const LIFE_MODEL_V2_LEGACY_MIGRATION_SCHEMA: &str = "openlife.lifemodel.v2.legacy-migration.v1";
pub const LIFE_MODEL_V2_LEGACY_MIGRATION_PATH: &str = "$lifemodel_v2_migration";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyLifeModelMigrationDispositionV2 {
    ReviewRequired,
    ExternalOwner,
    ManualClassification,
    NotMigrated,
    MigrationMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyLifeModelMigrationOwnerV2 {
    LifeModelV2,
    State,
    Tasks,
    AgentMemory,
    AgentRuntime,
    MigrationMetadata,
    LegacyCompatibilityProjection,
    Unassigned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyLifeModelMigrationItemV2 {
    pub source_path: String,
    pub value_preview: String,
    pub value_digest: String,
    pub value_truncated: bool,
    pub disposition: LegacyLifeModelMigrationDispositionV2,
    pub target_owner: LegacyLifeModelMigrationOwnerV2,
    pub target_section: Option<LifeModelSectionV2>,
    pub reason_code: String,
    pub sensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyLifeModelMigrationPreviewV2 {
    pub schema_version: String,
    pub source_digest: String,
    pub items: Vec<LegacyLifeModelMigrationItemV2>,
    pub review_required_count: usize,
    pub external_owner_count: usize,
    pub manual_classification_count: usize,
    pub not_migrated_count: usize,
    pub migration_metadata_count: usize,
    pub contains_sensitive_items: bool,
    pub candidates: Vec<LegacyLifeModelMigrationCandidateV2>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyLifeModelMigrationCandidateV2 {
    pub candidate_id: String,
    pub item_id: String,
    pub source_paths: Vec<String>,
    pub target_section: LifeModelSectionV2,
    pub proposed_value: LifeModelUserValueV2,
    pub sensitive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyLifeModelMigrationDecisionV2 {
    Include,
    Exclude,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyLifeModelMigrationSelectionV2 {
    pub candidate_id: String,
    pub decision: LegacyLifeModelMigrationDecisionV2,
    pub edited_value: Option<LifeModelUserValueV2>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyLifeModelMigrationPlanV2 {
    pub schema_version: String,
    pub model_id: String,
    pub legacy_source_digest: String,
    pub included_candidate_ids: Vec<String>,
    pub excluded_candidate_ids: Vec<String>,
    pub non_lifemodel_item_count: usize,
    pub non_lifemodel_items_acknowledged: bool,
    pub typed_diff: Option<LifeModelTypedDiffV2>,
    pub result_document_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyLifeModelBackupReceiptV2 {
    pub source_digest: String,
    pub backup_digest: String,
    pub backup_file_name: String,
    pub replayed: bool,
}

impl LegacyLifeModelMigrationPreviewV2 {
    pub fn from_legacy_yaml(source: &str) -> Result<Self> {
        if source.len() > MAX_LEGACY_SOURCE_BYTES {
            bail!("legacy_lifemodel_migration_source_too_large");
        }
        let _: LifeModel =
            serde_yaml::from_str(source).context("parse_legacy_lifemodel_for_migration")?;
        let value: serde_yaml::Value =
            serde_yaml::from_str(source).context("parse_legacy_lifemodel_yaml_tree")?;
        if !matches!(value, serde_yaml::Value::Mapping(_)) {
            bail!("legacy_lifemodel_migration_root_must_be_mapping");
        }

        let mut items = Vec::new();
        collect_items(&value, "", &mut items)?;
        items.sort_by(|left, right| left.source_path.cmp(&right.source_path));
        let count = |disposition| {
            items
                .iter()
                .filter(|item| item.disposition == disposition)
                .count()
        };
        let source_digest = sha256_digest(source.as_bytes());
        let candidates = build_candidates(&value, &source_digest, &items)?;
        Ok(Self {
            schema_version: "openlife.lifemodel.legacy-migration-preview.v1".into(),
            source_digest,
            review_required_count: count(LegacyLifeModelMigrationDispositionV2::ReviewRequired),
            external_owner_count: count(LegacyLifeModelMigrationDispositionV2::ExternalOwner),
            manual_classification_count: count(
                LegacyLifeModelMigrationDispositionV2::ManualClassification,
            ),
            not_migrated_count: count(LegacyLifeModelMigrationDispositionV2::NotMigrated),
            migration_metadata_count: count(
                LegacyLifeModelMigrationDispositionV2::MigrationMetadata,
            ),
            contains_sensitive_items: items.iter().any(|item| item.sensitive),
            candidates,
            items,
        })
    }

    pub fn has_user_content(&self) -> bool {
        self.review_required_count
            + self.external_owner_count
            + self.manual_classification_count
            + self.not_migrated_count
            > 0
    }

    pub fn build_migration_plan(
        &self,
        selections: &[LegacyLifeModelMigrationSelectionV2],
        non_lifemodel_items_acknowledged: bool,
        confirmed_at: &str,
    ) -> Result<LegacyLifeModelMigrationPlanV2> {
        DateTime::parse_from_rfc3339(confirmed_at)
            .map_err(|_| anyhow!("invalid_lifemodel_v2_migration_confirmed_at"))?;
        let non_lifemodel_item_count = self.items.len() - self.review_required_count;
        if non_lifemodel_item_count > 0 && !non_lifemodel_items_acknowledged {
            bail!("lifemodel_v2_migration_non_lifemodel_items_not_acknowledged");
        }
        if selections.len() != self.candidates.len() {
            bail!("lifemodel_v2_migration_candidate_decisions_incomplete");
        }
        let candidates = self
            .candidates
            .iter()
            .map(|candidate| (candidate.candidate_id.as_str(), candidate))
            .collect::<BTreeMap<_, _>>();
        let mut seen = BTreeSet::new();
        let mut included_candidate_ids = Vec::new();
        let mut excluded_candidate_ids = Vec::new();
        let mut operations = Vec::new();
        for selection in selections {
            if !seen.insert(selection.candidate_id.as_str()) {
                bail!("lifemodel_v2_migration_duplicate_candidate_decision");
            }
            let candidate = candidates
                .get(selection.candidate_id.as_str())
                .ok_or_else(|| anyhow!("lifemodel_v2_migration_unknown_candidate"))?;
            match selection.decision {
                LegacyLifeModelMigrationDecisionV2::Include => {
                    let value = selection
                        .edited_value
                        .clone()
                        .unwrap_or_else(|| candidate.proposed_value.clone());
                    let item = value.into_item(
                        candidate.item_id.clone(),
                        candidate_source_refs(candidate, &self.source_digest),
                        confirmed_at.into(),
                    );
                    included_candidate_ids.push(candidate.candidate_id.clone());
                    operations.push(LifeModelTypedOperationV2::Add {
                        section: candidate.target_section,
                        item,
                    });
                }
                LegacyLifeModelMigrationDecisionV2::Exclude => {
                    if selection.edited_value.is_some() {
                        bail!("lifemodel_v2_migration_excluded_candidate_cannot_be_edited");
                    }
                    excluded_candidate_ids.push(candidate.candidate_id.clone());
                }
            }
        }
        included_candidate_ids.sort();
        excluded_candidate_ids.sort();
        operations.sort_by(|left, right| operation_item_id(left).cmp(operation_item_id(right)));
        let typed_diff = if operations.is_empty() {
            None
        } else {
            Some(LifeModelTypedDiffV2::from_operations_for_review(
                DEFAULT_LIFE_MODEL_V2_MODEL_ID,
                None,
                operations,
                false,
            )?)
        };
        let result_document_digest = match typed_diff.as_ref() {
            Some(diff) => diff.result_document_digest.clone(),
            None => LifeModelDocumentV2::empty(DEFAULT_LIFE_MODEL_V2_MODEL_ID).digest()?,
        };
        let plan = LegacyLifeModelMigrationPlanV2 {
            schema_version: LIFE_MODEL_V2_LEGACY_MIGRATION_SCHEMA.into(),
            model_id: DEFAULT_LIFE_MODEL_V2_MODEL_ID.into(),
            legacy_source_digest: self.source_digest.clone(),
            included_candidate_ids,
            excluded_candidate_ids,
            non_lifemodel_item_count,
            non_lifemodel_items_acknowledged,
            typed_diff,
            result_document_digest,
        };
        self.validate_migration_plan(&plan)?;
        Ok(plan)
    }

    pub fn validate_migration_plan(&self, plan: &LegacyLifeModelMigrationPlanV2) -> Result<()> {
        plan.validate_contract()?;
        if plan.legacy_source_digest != self.source_digest
            || plan.non_lifemodel_item_count != self.items.len() - self.review_required_count
        {
            bail!("lifemodel_v2_migration_preview_binding_mismatch");
        }
        let expected = self
            .candidates
            .iter()
            .map(|candidate| candidate.candidate_id.as_str())
            .collect::<BTreeSet<_>>();
        let decided = plan
            .included_candidate_ids
            .iter()
            .chain(&plan.excluded_candidate_ids)
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if expected != decided {
            bail!("lifemodel_v2_migration_candidate_set_mismatch");
        }
        if let Some(diff) = plan.typed_diff.as_ref() {
            let included = plan
                .included_candidate_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if diff.operations.len() != included.len() {
                bail!("lifemodel_v2_migration_operation_count_mismatch");
            }
            for operation in &diff.operations {
                let LifeModelTypedOperationV2::Add { section, item } = operation else {
                    bail!("lifemodel_v2_migration_only_add_operations_allowed");
                };
                let candidate = self
                    .candidates
                    .iter()
                    .find(|candidate| candidate.item_id == item.id())
                    .ok_or_else(|| anyhow!("lifemodel_v2_migration_item_not_from_preview"))?;
                if !included.contains(candidate.candidate_id.as_str())
                    || *section != candidate.target_section
                    || item_source_refs(item)
                        != candidate_source_refs(candidate, &self.source_digest)
                {
                    bail!("lifemodel_v2_migration_item_binding_mismatch");
                }
            }
        }
        Ok(())
    }
}

impl LegacyLifeModelMigrationPlanV2 {
    pub fn validate_contract(&self) -> Result<()> {
        if self.schema_version != LIFE_MODEL_V2_LEGACY_MIGRATION_SCHEMA
            || self.model_id != DEFAULT_LIFE_MODEL_V2_MODEL_ID
            || !is_sha256_digest(&self.legacy_source_digest)
            || !is_sha256_digest(&self.result_document_digest)
        {
            bail!("invalid_lifemodel_v2_migration_contract");
        }
        if self.non_lifemodel_item_count > 0 && !self.non_lifemodel_items_acknowledged {
            bail!("lifemodel_v2_migration_non_lifemodel_items_not_acknowledged");
        }
        let decisions = self
            .included_candidate_ids
            .iter()
            .chain(&self.excluded_candidate_ids)
            .collect::<BTreeSet<_>>();
        if decisions.len() != self.included_candidate_ids.len() + self.excluded_candidate_ids.len()
        {
            bail!("lifemodel_v2_migration_duplicate_candidate_decision");
        }
        match self.typed_diff.as_ref() {
            Some(diff) => {
                diff.validate_contract()?;
                if diff.model_id != self.model_id
                    || diff.base_version.is_some()
                    || diff.base_document_digest.is_some()
                    || diff.result_document_digest != self.result_document_digest
                    || diff.operations.len() != self.included_candidate_ids.len()
                {
                    bail!("lifemodel_v2_migration_typed_diff_mismatch");
                }
            }
            None => {
                if !self.included_candidate_ids.is_empty()
                    || LifeModelDocumentV2::empty(&self.model_id).digest()?
                        != self.result_document_digest
                {
                    bail!("lifemodel_v2_migration_empty_result_mismatch");
                }
            }
        }
        Ok(())
    }

    pub(crate) fn result_document(&self) -> Result<LifeModelDocumentV2> {
        self.validate_contract()?;
        match self.typed_diff.as_ref() {
            Some(diff) => diff.apply_to_version_for_review(None, false),
            None => Ok(LifeModelDocumentV2::empty(&self.model_id)),
        }
    }
}

fn build_candidates(
    root: &serde_yaml::Value,
    source_digest: &str,
    items: &[LegacyLifeModelMigrationItemV2],
) -> Result<Vec<LegacyLifeModelMigrationCandidateV2>> {
    let mut groups: BTreeMap<(LifeModelSectionV2, String), Vec<&LegacyLifeModelMigrationItemV2>> =
        BTreeMap::new();
    for item in items
        .iter()
        .filter(|item| item.disposition == LegacyLifeModelMigrationDispositionV2::ReviewRequired)
    {
        let section = item
            .target_section
            .ok_or_else(|| anyhow!("lifemodel_v2_migration_review_item_without_section"))?;
        let group = if matches!(
            section,
            LifeModelSectionV2::LongTermGoals
                | LifeModelSectionV2::ImportantRelationships
                | LifeModelSectionV2::Capabilities
                | LifeModelSectionV2::Resources
        ) {
            item.source_path
                .rsplit_once('.')
                .map(|(prefix, _)| prefix.to_string())
                .unwrap_or_else(|| item.source_path.clone())
        } else {
            item.source_path.clone()
        };
        groups.entry((section, group)).or_default().push(item);
    }

    let mut candidates = Vec::new();
    for ((section, _), mut fields) in groups {
        fields.sort_by(|left, right| left.source_path.cmp(&right.source_path));
        let source_paths = fields
            .iter()
            .map(|field| field.source_path.clone())
            .collect::<Vec<_>>();
        let values = fields
            .iter()
            .map(|field| {
                Ok((
                    field.source_path.as_str(),
                    yaml_scalar_at_path(root, &field.source_path)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let identity = serde_json::to_vec(&serde_json::json!({
            "sourceDigest": source_digest,
            "section": section,
            "sourcePaths": source_paths,
        }))
        .context("serialize_lifemodel_v2_migration_candidate_identity")?;
        let candidate_digest = sha256_hex(&identity);
        candidates.push(LegacyLifeModelMigrationCandidateV2 {
            candidate_id: format!("legacy-candidate:{}", &candidate_digest[..24]),
            item_id: format!("legacy:{}", &candidate_digest[..24]),
            source_paths,
            target_section: section,
            proposed_value: candidate_value(section, &values),
            sensitive: fields.iter().any(|field| field.sensitive),
        });
    }
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let covered = candidates
        .iter()
        .flat_map(|candidate| candidate.source_paths.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let expected = items
        .iter()
        .filter(|item| item.disposition == LegacyLifeModelMigrationDispositionV2::ReviewRequired)
        .map(|item| item.source_path.as_str())
        .collect::<BTreeSet<_>>();
    if covered != expected {
        bail!("lifemodel_v2_migration_candidate_source_coverage_mismatch");
    }
    Ok(candidates)
}

fn candidate_value(
    section: LifeModelSectionV2,
    values: &BTreeMap<&str, String>,
) -> LifeModelUserValueV2 {
    let suffix = |name: &str| {
        values
            .iter()
            .find(|(path, _)| path.rsplit('.').next() == Some(name))
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    let first = || values.values().next().cloned().unwrap_or_default();
    match section {
        LifeModelSectionV2::LongTermGoals => LifeModelUserValueV2::LongTermGoal {
            direction: suffix("name"),
            meaning: suffix("description"),
        },
        LifeModelSectionV2::ImportantRelationships => LifeModelUserValueV2::Relationship {
            person_label: suffix("name"),
            relationship: suffix("relationship_type"),
            significance: suffix("notes"),
        },
        LifeModelSectionV2::Capabilities => LifeModelUserValueV2::Capability {
            name: {
                let name = suffix("name");
                if name.is_empty() {
                    suffix("domain")
                } else {
                    name
                }
            },
            description: suffix("description"),
        },
        LifeModelSectionV2::Resources => LifeModelUserValueV2::Resource {
            name: suffix("name"),
            description: suffix("description"),
        },
        _ => LifeModelUserValueV2::Statement { statement: first() },
    }
}

fn yaml_scalar_at_path(root: &serde_yaml::Value, path: &str) -> Result<String> {
    let mut current = root;
    for segment in path.split('.') {
        let key_end = segment.find('[').unwrap_or(segment.len());
        let key = &segment[..key_end];
        if !key.is_empty() {
            current = current
                .as_mapping()
                .and_then(|mapping| mapping.get(serde_yaml::Value::String(key.into())))
                .ok_or_else(|| anyhow!("lifemodel_v2_migration_source_path_missing:{path}"))?;
        }
        let mut remainder = &segment[key_end..];
        while let Some(index_start) = remainder.strip_prefix('[') {
            let (index, tail) = index_start
                .split_once(']')
                .ok_or_else(|| anyhow!("lifemodel_v2_migration_source_path_invalid:{path}"))?;
            let index = index
                .parse::<usize>()
                .map_err(|_| anyhow!("lifemodel_v2_migration_source_path_invalid:{path}"))?;
            current = current
                .as_sequence()
                .and_then(|sequence| sequence.get(index))
                .ok_or_else(|| anyhow!("lifemodel_v2_migration_source_path_missing:{path}"))?;
            remainder = tail;
        }
        if !remainder.is_empty() {
            bail!("lifemodel_v2_migration_source_path_invalid:{path}");
        }
    }
    match current {
        serde_yaml::Value::Bool(value) => Ok(value.to_string()),
        serde_yaml::Value::Number(value) => Ok(value.to_string()),
        serde_yaml::Value::String(value) => Ok(value.clone()),
        _ => bail!("lifemodel_v2_migration_source_value_not_scalar:{path}"),
    }
}

fn candidate_source_refs(
    candidate: &LegacyLifeModelMigrationCandidateV2,
    source_digest: &str,
) -> Vec<String> {
    candidate
        .source_paths
        .iter()
        .map(|path| format!("legacy-yaml:{source_digest}#{path}"))
        .collect()
}

fn item_source_refs(item: &LifeModelItemV2) -> Vec<String> {
    match item {
        LifeModelItemV2::Statement(item) => item.source_refs.clone(),
        LifeModelItemV2::LongTermGoal(item) => item.source_refs.clone(),
        LifeModelItemV2::Relationship(item) => item.source_refs.clone(),
        LifeModelItemV2::Capability(item) => item.source_refs.clone(),
        LifeModelItemV2::Resource(item) => item.source_refs.clone(),
    }
}

fn operation_item_id(operation: &LifeModelTypedOperationV2) -> &str {
    match operation {
        LifeModelTypedOperationV2::Add { item, .. } => item.id(),
        LifeModelTypedOperationV2::Replace { item_id, .. }
        | LifeModelTypedOperationV2::Remove { item_id, .. } => item_id,
    }
}

fn is_sha256_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyLifeModelInventoryV2 {
    pub schema_version: String,
    pub current_source_present: bool,
    pub current_source_bytes: u64,
    pub current_source_modified_at: Option<String>,
    pub current_source_digest: Option<String>,
    pub history_manifest_present: bool,
    pub history_manifest_entry_count: usize,
    pub history_manifest_digest: Option<String>,
    pub history_yaml_file_count: usize,
    pub history_yaml_total_bytes: u64,
    pub preview: Option<LegacyLifeModelMigrationPreviewV2>,
}

/// Inspect the current YAML and sibling version journal without modifying
/// either source. Returns `None` only when no legacy source exists at all.
pub fn inspect_legacy_lifemodel(current_dir: &Path) -> Result<Option<LegacyLifeModelInventoryV2>> {
    let source_path = current_dir.join("life_model.yaml");
    let versions_dir = current_dir
        .parent()
        .ok_or_else(|| anyhow!("legacy_lifemodel_current_directory_has_no_parent"))?
        .join("versions");
    reject_symlink_if_present(&source_path)?;
    reject_symlink_if_present(&versions_dir)?;

    let (current_source_present, current_source_bytes, current_source_modified_at, source, preview) =
        if source_path.exists() {
            let metadata = fs::metadata(&source_path)
                .with_context(|| format!("inspect_legacy_lifemodel_source:{source_path:?}"))?;
            if !metadata.is_file() {
                bail!("legacy_lifemodel_source_is_not_regular_file");
            }
            if metadata.len() > MAX_LEGACY_SOURCE_BYTES as u64 {
                bail!("legacy_lifemodel_migration_source_too_large");
            }
            let bytes = fs::read(&source_path)
                .with_context(|| format!("read_legacy_lifemodel_source:{source_path:?}"))?;
            let source = String::from_utf8(bytes)
                .map_err(|_| anyhow!("legacy_lifemodel_source_not_utf8"))?;
            let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(&source)?;
            (
                true,
                metadata.len(),
                metadata.modified().ok().map(system_time_rfc3339),
                Some(source),
                Some(preview),
            )
        } else {
            (false, 0, None, None, None)
        };

    let manifest_path = versions_dir.join("index.json");
    reject_symlink_if_present(&manifest_path)?;
    let (history_manifest_present, history_manifest_entry_count, history_manifest_digest) =
        if manifest_path.exists() {
            let metadata = fs::metadata(&manifest_path)
                .with_context(|| format!("inspect_legacy_lifemodel_manifest:{manifest_path:?}"))?;
            if !metadata.is_file() {
                bail!("legacy_lifemodel_history_manifest_is_not_regular_file");
            }
            let bytes = fs::read(&manifest_path)
                .with_context(|| format!("read_legacy_lifemodel_manifest:{manifest_path:?}"))?;
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .context("parse_legacy_lifemodel_history_manifest")?;
            let count = value
                .get("versions")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| anyhow!("legacy_lifemodel_history_manifest_shape_invalid"))?
                .len();
            (true, count, Some(sha256_digest(&bytes)))
        } else {
            (false, 0, None)
        };

    let (history_yaml_file_count, history_yaml_total_bytes) = inspect_history_files(&versions_dir)?;
    if !current_source_present && !history_manifest_present && history_yaml_file_count == 0 {
        return Ok(None);
    }

    Ok(Some(LegacyLifeModelInventoryV2 {
        schema_version: "openlife.lifemodel.legacy-inventory.v1".into(),
        current_source_present,
        current_source_bytes,
        current_source_modified_at,
        current_source_digest: source
            .as_deref()
            .map(|value| sha256_digest(value.as_bytes())),
        history_manifest_present,
        history_manifest_entry_count,
        history_manifest_digest,
        history_yaml_file_count,
        history_yaml_total_bytes,
        preview,
    }))
}

fn inspect_history_files(versions_dir: &Path) -> Result<(usize, u64)> {
    if !versions_dir.exists() {
        return Ok((0, 0));
    }
    let metadata = fs::metadata(versions_dir)
        .with_context(|| format!("inspect_legacy_lifemodel_versions:{versions_dir:?}"))?;
    if !metadata.is_dir() {
        bail!("legacy_lifemodel_history_is_not_directory");
    }
    let mut count = 0usize;
    let mut bytes = 0u64;
    for entry in fs::read_dir(versions_dir)
        .with_context(|| format!("read_legacy_lifemodel_versions:{versions_dir:?}"))?
    {
        let entry = entry.context("read_legacy_lifemodel_version_entry")?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("yaml") {
            continue;
        }
        reject_symlink_if_present(&path)?;
        let metadata = entry
            .metadata()
            .with_context(|| format!("inspect_legacy_lifemodel_version:{path:?}"))?;
        if !metadata.is_file() {
            bail!("legacy_lifemodel_history_entry_is_not_regular_file");
        }
        count += 1;
        bytes = bytes
            .checked_add(metadata.len())
            .ok_or_else(|| anyhow!("legacy_lifemodel_history_size_overflow"))?;
    }
    Ok((count, bytes))
}

fn reject_symlink_if_present(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("legacy_lifemodel_symlink_not_allowed")
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("inspect_legacy_lifemodel_path:{path:?}")),
    }
}

fn system_time_rfc3339(value: std::time::SystemTime) -> String {
    DateTime::<Utc>::from(value).to_rfc3339()
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Clone, Copy)]
struct LegacyFieldClassification {
    disposition: LegacyLifeModelMigrationDispositionV2,
    target_owner: LegacyLifeModelMigrationOwnerV2,
    target_section: Option<LifeModelSectionV2>,
    reason_code: &'static str,
    sensitive: bool,
}

fn collect_items(
    value: &serde_yaml::Value,
    path: &str,
    items: &mut Vec<LegacyLifeModelMigrationItemV2>,
) -> Result<()> {
    match value {
        serde_yaml::Value::Null => Ok(()),
        serde_yaml::Value::Bool(_) | serde_yaml::Value::Number(_) => push_leaf(value, path, items),
        serde_yaml::Value::String(text) if text.is_empty() => Ok(()),
        serde_yaml::Value::String(_) => push_leaf(value, path, items),
        serde_yaml::Value::Sequence(values) => {
            for (index, item) in values.iter().enumerate() {
                collect_items(item, &format!("{path}[{index}]"), items)?;
            }
            Ok(())
        }
        serde_yaml::Value::Mapping(mapping) => {
            for (key, item) in mapping {
                let key = key
                    .as_str()
                    .ok_or_else(|| anyhow!("legacy_lifemodel_migration_non_string_key"))?;
                let child = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                if child == "hs_compatibility" {
                    if !yaml_value_is_empty(item) {
                        push_compatibility_projection(item, &child, items)?;
                    }
                } else {
                    collect_items(item, &child, items)?;
                }
            }
            Ok(())
        }
        serde_yaml::Value::Tagged(_) => bail!("legacy_lifemodel_migration_yaml_tags_unsupported"),
    }
}

fn yaml_value_is_empty(value: &serde_yaml::Value) -> bool {
    match value {
        serde_yaml::Value::Null => true,
        serde_yaml::Value::String(value) => value.is_empty(),
        serde_yaml::Value::Sequence(value) => value.is_empty(),
        serde_yaml::Value::Mapping(value) => value.is_empty(),
        _ => false,
    }
}

fn push_compatibility_projection(
    value: &serde_yaml::Value,
    path: &str,
    items: &mut Vec<LegacyLifeModelMigrationItemV2>,
) -> Result<()> {
    enforce_item_limit(items)?;
    let encoded = serde_yaml::to_string(value)
        .context("serialize_legacy_lifemodel_compatibility_projection")?;
    items.push(LegacyLifeModelMigrationItemV2 {
        source_path: path.into(),
        value_preview: "Derived legacy compatibility projection".into(),
        value_digest: sha256_digest(encoded.as_bytes()),
        value_truncated: true,
        disposition: LegacyLifeModelMigrationDispositionV2::ExternalOwner,
        target_owner: LegacyLifeModelMigrationOwnerV2::LegacyCompatibilityProjection,
        target_section: None,
        reason_code: "legacy_compatibility_projection_not_user_truth".into(),
        sensitive: true,
    });
    Ok(())
}

fn push_leaf(
    value: &serde_yaml::Value,
    path: &str,
    items: &mut Vec<LegacyLifeModelMigrationItemV2>,
) -> Result<()> {
    enforce_item_limit(items)?;
    if path.is_empty() {
        bail!("legacy_lifemodel_migration_leaf_without_path");
    }
    let normalized = normalize_path(path)?;
    let classification = classify_path(&normalized)
        .ok_or_else(|| anyhow!("unclassified_legacy_lifemodel_field:{normalized}"))?;
    let raw = match value {
        serde_yaml::Value::Bool(value) => value.to_string(),
        serde_yaml::Value::Number(value) => {
            if value.as_f64().is_some_and(|number| !number.is_finite()) {
                bail!("legacy_lifemodel_migration_non_finite_number:{normalized}");
            }
            value.to_string()
        }
        serde_yaml::Value::String(value) => value.clone(),
        _ => bail!("legacy_lifemodel_migration_non_scalar_leaf"),
    };
    let (value_preview, value_truncated) = bounded_preview(&raw, 240);
    items.push(LegacyLifeModelMigrationItemV2 {
        source_path: path.into(),
        value_preview,
        value_digest: sha256_digest(raw.as_bytes()),
        value_truncated,
        disposition: classification.disposition,
        target_owner: classification.target_owner,
        target_section: classification.target_section,
        reason_code: classification.reason_code.into(),
        sensitive: classification.sensitive,
    });
    Ok(())
}

fn enforce_item_limit(items: &[LegacyLifeModelMigrationItemV2]) -> Result<()> {
    if items.len() >= MAX_LEGACY_ITEMS {
        bail!("legacy_lifemodel_migration_item_limit_exceeded");
    }
    Ok(())
}

fn normalize_path(path: &str) -> Result<String> {
    let mut normalized = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '[' {
            normalized.push(character);
            continue;
        }
        let mut saw_digit = false;
        let mut closed = false;
        for next in chars.by_ref() {
            if next == ']' {
                closed = true;
                break;
            }
            if !next.is_ascii_digit() {
                bail!("invalid_legacy_lifemodel_sequence_path");
            }
            saw_digit = true;
        }
        if !saw_digit || !closed {
            bail!("invalid_legacy_lifemodel_sequence_path");
        }
        normalized.push_str("[]");
    }
    Ok(normalized)
}

fn bounded_preview(value: &str, max_chars: usize) -> (String, bool) {
    let mut chars = value.chars();
    let preview = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        (format!("{preview}…"), true)
    } else {
        (preview, false)
    }
}

fn classify_path(path: &str) -> Option<LegacyFieldClassification> {
    use LegacyLifeModelMigrationDispositionV2 as Disposition;
    use LegacyLifeModelMigrationOwnerV2 as Owner;
    use LifeModelSectionV2 as Section;

    let classification = match path {
        "metadata.version" | "metadata.created_at" | "metadata.updated_at" | "metadata.author" => {
            class(
                Disposition::MigrationMetadata,
                Owner::MigrationMetadata,
                None,
                "legacy_document_metadata_only",
                false,
            )
        }
        "identity.name" | "identity.birth_date" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::Identity),
            "legacy_identity_requires_user_confirmation",
            path == "identity.birth_date",
        ),
        "identity.values[].name" | "identity.values[].description" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::Values),
            "legacy_value_requires_user_confirmation",
            false,
        ),
        "identity.values[].weight" => class(
            Disposition::NotMigrated,
            Owner::Unassigned,
            None,
            "legacy_value_weight_is_not_canonical_truth",
            false,
        ),
        "identity.personality_traits[].trait_name" | "identity.personality_traits[].score" => {
            class(
                Disposition::NotMigrated,
                Owner::Unassigned,
                None,
                "legacy_personality_score_requires_user_restatement",
                true,
            )
        }
        "identity.life_philosophy" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::DecisionPrinciples),
            "legacy_life_philosophy_requires_user_confirmation",
            false,
        ),
        "identity.mission_statement" => class(
            Disposition::ManualClassification,
            Owner::Unassigned,
            None,
            "legacy_mission_could_be_identity_or_long_term_goal",
            false,
        ),
        "identity.role_definition.primary_role"
        | "identity.role_definition.professional"
        | "identity.role_definition.secondary_roles"
        | "identity.role_definition.secondary_roles[]"
        | "identity.role_definition.personal"
        | "identity.role_definition.personal[]" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::Identity),
            "legacy_role_requires_user_confirmation",
            false,
        ),
        "identity.role_definition.responsibilities[]" => class(
            Disposition::ManualClassification,
            Owner::Unassigned,
            None,
            "legacy_responsibility_may_be_identity_or_work_context",
            false,
        ),
        "identity.role_definition.boundaries[]" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::PersonalBoundaries),
            "legacy_boundary_requires_user_confirmation",
            true,
        ),
        "identity.voice_style.tone_descriptors[]"
        | "identity.voice_style.formality"
        | "identity.voice_style.formality_level"
        | "identity.voice_style.vocabulary_preference"
        | "identity.voice_style.emoji_usage" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::CollaborationPreferences),
            "legacy_voice_style_requires_user_confirmation",
            false,
        ),
        "capabilities.skills[].name" | "capabilities.skills[].description" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::Capabilities),
            "legacy_user_capability_requires_user_confirmation",
            false,
        ),
        "capabilities.skills[].proficiency"
        | "capabilities.knowledge_domains[].level"
        | "capabilities.knowledge_domains[].proficiency" => class(
            Disposition::NotMigrated,
            Owner::Unassigned,
            None,
            "legacy_proficiency_score_is_not_canonical_truth",
            false,
        ),
        "capabilities.resources[].name" | "capabilities.resources[].description" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::Resources),
            "legacy_stable_resource_requires_user_confirmation",
            true,
        ),
        "capabilities.resources[].resource_type" | "capabilities.resources[].type" => class(
            Disposition::ManualClassification,
            Owner::Unassigned,
            None,
            "legacy_resource_type_has_no_lossless_v2_target",
            true,
        ),
        "capabilities.resources[].availability" => class(
            Disposition::ExternalOwner,
            Owner::State,
            None,
            "resource_availability_is_current_state",
            true,
        ),
        "capabilities.networks[]" => class(
            Disposition::ManualClassification,
            Owner::Unassigned,
            None,
            "legacy_network_could_be_relationship_or_resource",
            true,
        ),
        "capabilities.tools[].name"
        | "capabilities.tools[].proficiency"
        | "capabilities.tools[].description" => class(
            Disposition::ExternalOwner,
            Owner::AgentRuntime,
            None,
            "agent_tool_capability_is_not_user_lifemodel",
            false,
        ),
        "capabilities.knowledge_domains[].domain"
        | "capabilities.knowledge_domains[].name"
        | "capabilities.knowledge_domains[].description" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::Capabilities),
            "legacy_knowledge_domain_requires_user_confirmation",
            false,
        ),
        "relationships.inner_circle[].name"
        | "relationships.inner_circle[].relationship_type"
        | "relationships.inner_circle[].notes"
        | "relationships.mentors[].name"
        | "relationships.mentors[].relationship_type"
        | "relationships.mentors[].notes"
        | "relationships.collaborators[].name"
        | "relationships.collaborators[].relationship_type"
        | "relationships.collaborators[].notes" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::ImportantRelationships),
            "legacy_relationship_requires_sensitive_user_confirmation",
            true,
        ),
        "relationships.inner_circle[].importance"
        | "relationships.mentors[].importance"
        | "relationships.collaborators[].importance" => class(
            Disposition::NotMigrated,
            Owner::Unassigned,
            None,
            "legacy_relationship_score_is_not_canonical_truth",
            true,
        ),
        "preferences.work_hours.preferred_start"
        | "preferences.work_hours.preferred_end"
        | "preferences.work_hours.timezone"
        | "preferences.peak_energy_time"
        | "preferences.peak_productivity_times"
        | "preferences.peak_productivity_times[]"
        | "preferences.learning_style" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::StablePreferences),
            "legacy_stable_preference_requires_user_confirmation",
            false,
        ),
        "preferences.communication_style"
        | "preferences.notification_preferences"
        | "preferences.notification_preferences[]" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::CollaborationPreferences),
            "legacy_collaboration_preference_requires_user_confirmation",
            false,
        ),
        "preferences.decision_making_style" => class(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::DecisionPrinciples),
            "legacy_decision_principle_requires_user_confirmation",
            false,
        ),
        "evolution_rules[]" => class(
            Disposition::ExternalOwner,
            Owner::AgentMemory,
            None,
            "procedural_rule_belongs_to_agent_memory",
            false,
        ),
        _ if path.starts_with("goals.short_term[]")
            || path.starts_with("goals.medium_term[]")
            || path.starts_with("goals.daily[]") =>
        {
            class(
                Disposition::ExternalOwner,
                if path.ends_with(".related_memories[]") {
                    Owner::AgentMemory
                } else {
                    Owner::Tasks
                },
                None,
                "operational_goal_belongs_outside_lifemodel",
                false,
            )
        }
        _ if path.starts_with("goals.long_term[]") || path.starts_with("goals.life_goals[]") => {
            if path.ends_with(".name") || path.ends_with(".description") {
                class(
                    Disposition::ReviewRequired,
                    Owner::LifeModelV2,
                    Some(Section::LongTermGoals),
                    "legacy_long_term_goal_requires_user_confirmation",
                    false,
                )
            } else if path.ends_with(".related_memories[]") {
                class(
                    Disposition::ExternalOwner,
                    Owner::AgentMemory,
                    None,
                    "goal_memory_link_belongs_to_agent_memory",
                    false,
                )
            } else {
                class(
                    Disposition::ExternalOwner,
                    Owner::Tasks,
                    None,
                    "long_term_goal_operational_state_is_not_lifemodel",
                    false,
                )
            }
        }
        _ if path.starts_with("state.recent_reflections[]")
            || path.starts_with("state.open_questions[]")
            || path.starts_with("state.recent_events[]") =>
        {
            class(
                Disposition::ExternalOwner,
                Owner::AgentMemory,
                None,
                "historical_experience_belongs_to_agent_memory",
                path.starts_with("state.recent_reflections[]"),
            )
        }
        _ if path.starts_with("state.") => class(
            Disposition::ExternalOwner,
            Owner::State,
            None,
            "current_state_belongs_to_state",
            path.starts_with("state.health_status") || path.starts_with("state.emotional_state"),
        ),
        _ => return None,
    };
    Some(classification)
}

fn class(
    disposition: LegacyLifeModelMigrationDispositionV2,
    target_owner: LegacyLifeModelMigrationOwnerV2,
    target_section: Option<LifeModelSectionV2>,
    reason_code: &'static str,
    sensitive: bool,
) -> LegacyFieldClassification {
    LegacyFieldClassification {
        disposition,
        target_owner,
        target_section,
        reason_code,
        sensitive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_classifies_exact_present_leaves_without_default_expansion() {
        let source = "metadata:\n  version: 0.1.0\nidentity:\n  name: Alice\n  values:\n    - name: Autonomy\n      weight: 8\ngoals:\n  short_term:\n    - name: Ship now\nstate:\n  current_focus: Launch\n";
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(source).unwrap();
        assert_eq!(preview.items.len(), 6);
        assert_eq!(preview.review_required_count, 2);
        assert_eq!(preview.external_owner_count, 2);
        assert_eq!(preview.not_migrated_count, 1);
        assert_eq!(preview.migration_metadata_count, 1);
        assert!(preview
            .items
            .iter()
            .all(|item| item.source_path != "state.health_status.energy_level"));
    }

    #[test]
    fn unknown_or_oversized_sources_fail_closed() {
        let unknown =
            LegacyLifeModelMigrationPreviewV2::from_legacy_yaml("mystery: value\n").unwrap_err();
        assert!(unknown
            .to_string()
            .contains("unclassified_legacy_lifemodel_field"));
        let oversized = "a".repeat(MAX_LEGACY_SOURCE_BYTES + 1);
        assert!(
            LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(&oversized)
                .unwrap_err()
                .to_string()
                .contains("source_too_large")
        );
    }

    #[test]
    fn inventory_reads_current_and_history_metadata_without_writing() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("life-model/current");
        let versions = root.path().join("life-model/versions");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(&versions).unwrap();
        fs::write(
            current.join("life_model.yaml"),
            "identity:\n  name: Alice\n",
        )
        .unwrap();
        fs::write(versions.join("one.yaml"), "identity:\n  name: Alice\n").unwrap();
        fs::write(
            versions.join("index.json"),
            r#"{"versions":[{"version":"one"}]}"#,
        )
        .unwrap();

        let inventory = inspect_legacy_lifemodel(&current).unwrap().unwrap();
        assert!(inventory.current_source_present);
        assert_eq!(inventory.history_manifest_entry_count, 1);
        assert_eq!(inventory.history_yaml_file_count, 1);
        assert!(inventory.current_source_digest.is_some());
        assert_eq!(
            inventory.current_source_digest,
            inventory
                .preview
                .as_ref()
                .map(|preview| preview.source_digest.clone())
        );
        assert!(!current.join("life_model_v2.db").exists());
    }

    #[test]
    fn inventory_returns_none_for_an_absent_legacy_owner() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("life-model/current");
        assert!(inspect_legacy_lifemodel(&current).unwrap().is_none());
        assert!(!current.exists());
    }

    #[test]
    fn reviewed_plan_backs_up_materializes_and_replays_one_atomic_cutover() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("life-model/current");
        fs::create_dir_all(&current).unwrap();
        let source = "identity:\n  name: Alice\ngoals:\n  short_term:\n    - name: Ship now\n";
        fs::write(current.join("life_model.yaml"), source).unwrap();
        let manager = crate::life_model::LifeModelManager::new(&current);
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(source).unwrap();
        let selections = preview
            .candidates
            .iter()
            .map(|candidate| LegacyLifeModelMigrationSelectionV2 {
                candidate_id: candidate.candidate_id.clone(),
                decision: LegacyLifeModelMigrationDecisionV2::Include,
                edited_value: None,
            })
            .collect::<Vec<_>>();
        let plan = preview
            .build_migration_plan(&selections, true, "2026-08-23T12:00:00Z")
            .unwrap();
        let backup = manager
            .backup_legacy_source_for_migration(&preview.source_digest)
            .unwrap();
        assert!(!backup.replayed);
        let first = manager
            .materialize_reviewed_legacy_v2_migration(
                &plan,
                "migration-proposal-1",
                &backup.backup_digest,
                "2026-08-23T12:00:00Z",
            )
            .unwrap();
        assert!(!first.replayed);
        assert_eq!(first.version.model_version, 1);
        assert_eq!(first.version.document.identity.len(), 1);
        assert_eq!(first.cutover.legacy_source_digest, preview.source_digest);
        assert_eq!(
            manager
                .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
                .unwrap()
                .unwrap(),
            first.version
        );

        let backup_replay = manager
            .backup_legacy_source_for_migration(&preview.source_digest)
            .unwrap();
        assert!(backup_replay.replayed);
        let replay = manager
            .materialize_reviewed_legacy_v2_migration(
                &plan,
                "migration-proposal-1",
                &backup_replay.backup_digest,
                "2026-08-23T12:00:00Z",
            )
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.version, first.version);
        assert_eq!(replay.cutover, first.cutover);
        assert!(current.join("life_model.yaml").exists());
    }

    #[test]
    fn migration_requires_every_candidate_decision_and_source_acknowledgement() {
        let source = "identity:\n  name: Alice\ngoals:\n  short_term:\n    - name: Ship now\n";
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(source).unwrap();
        assert!(preview
            .build_migration_plan(&[], true, "2026-08-23T12:00:00Z")
            .unwrap_err()
            .to_string()
            .contains("candidate_decisions_incomplete"));
        let selections = preview
            .candidates
            .iter()
            .map(|candidate| LegacyLifeModelMigrationSelectionV2 {
                candidate_id: candidate.candidate_id.clone(),
                decision: LegacyLifeModelMigrationDecisionV2::Exclude,
                edited_value: None,
            })
            .collect::<Vec<_>>();
        assert!(preview
            .build_migration_plan(&selections, false, "2026-08-23T12:00:00Z")
            .unwrap_err()
            .to_string()
            .contains("non_lifemodel_items_not_acknowledged"));
        let plan = preview
            .build_migration_plan(&selections, true, "2026-08-23T12:00:00Z")
            .unwrap();
        assert!(plan.typed_diff.is_none());
        assert_eq!(
            plan.result_document_digest,
            LifeModelDocumentV2::empty(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
                .digest()
                .unwrap()
        );
    }
}
