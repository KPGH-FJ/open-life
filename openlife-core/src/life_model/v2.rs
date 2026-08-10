//! Versioned canonical LifeModel document.
//!
//! This module deliberately does not migrate or write the legacy YAML owner.
//! It defines the v2 user-model boundary and an append-only SQLite owner that
//! can be consumed by shipped read models before any authority cutover.

use anyhow::{anyhow, bail, Context, Result};
use chrono::DateTime;
use ring::digest::{digest, SHA256};
use rusqlite::TransactionBehavior;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::LifeModel;

pub const LIFE_MODEL_V2_SCHEMA_VERSION: &str = "openlife.lifemodel.v2";
pub const DEFAULT_LIFE_MODEL_V2_MODEL_ID: &str = "primary";
const LIFE_MODEL_V2_STORE_SCHEMA_VERSION: i64 = 2;
const MAX_DOCUMENT_BYTES: usize = 256 * 1024;
const MAX_ITEMS_PER_SECTION: usize = 512;
const MAX_ITEM_ID_CHARS: usize = 160;
const MAX_STATEMENT_CHARS: usize = 4_096;
const MAX_SOURCE_REFS_PER_ITEM: usize = 32;
const MAX_SOURCE_REF_CHARS: usize = 256;
const MAX_LEGACY_MIGRATION_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_LEGACY_MIGRATION_ITEMS: usize = 4_096;
const MAX_TYPED_DIFF_OPERATIONS: usize = MAX_ITEMS_PER_SECTION * 10;
const MAX_VERSION_HISTORY_ENTRIES: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelStatementV2 {
    pub id: String,
    pub statement: String,
    pub source_refs: Vec<String>,
    pub confirmed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelLongTermGoalV2 {
    pub id: String,
    pub direction: String,
    pub meaning: String,
    pub source_refs: Vec<String>,
    pub confirmed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelRelationshipV2 {
    pub id: String,
    pub person_label: String,
    pub relationship: String,
    pub significance: String,
    pub source_refs: Vec<String>,
    pub confirmed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelCapabilityV2 {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_refs: Vec<String>,
    pub confirmed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelResourceV2 {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_refs: Vec<String>,
    pub confirmed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelDocumentV2 {
    pub schema_version: String,
    pub model_id: String,
    pub identity: Vec<LifeModelStatementV2>,
    pub values: Vec<LifeModelStatementV2>,
    pub long_term_goals: Vec<LifeModelLongTermGoalV2>,
    pub stable_preferences: Vec<LifeModelStatementV2>,
    pub personal_boundaries: Vec<LifeModelStatementV2>,
    pub important_relationships: Vec<LifeModelRelationshipV2>,
    pub capabilities: Vec<LifeModelCapabilityV2>,
    pub resources: Vec<LifeModelResourceV2>,
    pub decision_principles: Vec<LifeModelStatementV2>,
    pub collaboration_preferences: Vec<LifeModelStatementV2>,
}

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
    StateStore,
    Tasks,
    AgentMemory,
    AgentRuntime,
    MigrationMetadata,
    LegacyCompatibilityProjection,
    Unassigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelSectionV2 {
    Identity,
    Values,
    LongTermGoals,
    StablePreferences,
    PersonalBoundaries,
    ImportantRelationships,
    Capabilities,
    Resources,
    DecisionPrinciples,
    CollaborationPreferences,
}

pub const LIFE_MODEL_V2_TYPED_DIFF_SCHEMA: &str = "openlife.lifemodel.v2.typed-diff.v1";
pub const LIFE_MODEL_V2_TYPED_DIFF_PATH: &str = "$lifemodel_v2";
pub const LIFE_MODEL_V2_LEGACY_MIGRATION_SCHEMA: &str = "openlife.lifemodel.v2.legacy-migration.v1";
pub const LIFE_MODEL_V2_LEGACY_MIGRATION_PATH: &str = "$lifemodel_v2_migration";
pub const LIFE_MODEL_V2_CUTOVER_SCHEMA: &str = "openlife.lifemodel.v2.cutover.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LifeModelItemV2 {
    Statement(LifeModelStatementV2),
    LongTermGoal(LifeModelLongTermGoalV2),
    Relationship(LifeModelRelationshipV2),
    Capability(LifeModelCapabilityV2),
    Resource(LifeModelResourceV2),
}

impl LifeModelItemV2 {
    pub fn id(&self) -> &str {
        match self {
            Self::Statement(item) => &item.id,
            Self::LongTermGoal(item) => &item.id,
            Self::Relationship(item) => &item.id,
            Self::Capability(item) => &item.id,
            Self::Resource(item) => &item.id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LifeModelUserValueV2 {
    Statement {
        statement: String,
    },
    LongTermGoal {
        direction: String,
        meaning: String,
    },
    Relationship {
        person_label: String,
        relationship: String,
        significance: String,
    },
    Capability {
        name: String,
        description: String,
    },
    Resource {
        name: String,
        description: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum LifeModelTypedOperationV2 {
    Add {
        section: LifeModelSectionV2,
        item: LifeModelItemV2,
    },
    Replace {
        section: LifeModelSectionV2,
        item_id: String,
        before_item_digest: String,
        item: LifeModelItemV2,
    },
    Remove {
        section: LifeModelSectionV2,
        item_id: String,
        before_item_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelTypedDiffV2 {
    pub schema_version: String,
    pub model_id: String,
    pub base_version: Option<u64>,
    pub base_document_digest: Option<String>,
    pub operations: Vec<LifeModelTypedOperationV2>,
    pub result_document_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelPatchMaterializationResultV2 {
    pub version: LifeModelVersionV2,
    pub replayed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelVersionChangeSummaryV2 {
    pub added: usize,
    pub replaced: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelVersionHistoryEntryV2 {
    pub model_id: String,
    pub model_version: u64,
    pub parent_version: Option<u64>,
    pub document_digest: String,
    pub item_count: usize,
    pub summary: String,
    pub source_refs: Vec<String>,
    pub created_at: String,
    pub change_summary: LifeModelVersionChangeSummaryV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LegacyLifeModelMigrationCandidateValueV2 {
    Statement {
        statement: String,
    },
    LongTermGoal {
        direction: String,
        meaning: String,
    },
    Relationship {
        person_label: String,
        relationship: String,
        significance: String,
    },
    Capability {
        name: String,
        description: String,
    },
    Resource {
        name: String,
        description: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyLifeModelMigrationCandidateV2 {
    pub candidate_id: String,
    pub item_id: String,
    pub source_paths: Vec<String>,
    pub target_section: LifeModelSectionV2,
    pub proposed_value: LegacyLifeModelMigrationCandidateValueV2,
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
    pub edited_value: Option<LegacyLifeModelMigrationCandidateValueV2>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelV2CutoverReceipt {
    pub schema_version: String,
    pub model_id: String,
    pub legacy_source_digest: String,
    pub backup_digest: String,
    pub model_version: u64,
    pub document_digest: String,
    pub proposal_id: String,
    pub cutover_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyLifeModelBackupReceiptV2 {
    pub source_digest: String,
    pub backup_digest: String,
    pub backup_file_name: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelLegacyMigrationMaterializationResultV2 {
    pub version: LifeModelVersionV2,
    pub cutover: LifeModelV2CutoverReceipt,
    pub replayed: bool,
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

impl LegacyLifeModelMigrationPreviewV2 {
    /// Build a read-only migration preview from the exact legacy YAML source.
    ///
    /// Parsing the raw document, rather than a default-filled `LifeModel`, is
    /// essential: omitted legacy fields must not become migration candidates.
    pub fn from_legacy_yaml(source: &str) -> Result<Self> {
        if source.len() > MAX_LEGACY_MIGRATION_SOURCE_BYTES {
            bail!("legacy_lifemodel_migration_source_too_large");
        }
        let legacy_model: LifeModel =
            serde_yaml::from_str(source).context("parse_legacy_lifemodel_for_migration")?;
        let value: serde_yaml::Value =
            serde_yaml::from_str(source).context("parse_legacy_lifemodel_yaml_tree")?;
        if !matches!(value, serde_yaml::Value::Mapping(_)) {
            bail!("legacy_lifemodel_migration_root_must_be_mapping");
        }

        let mut items = Vec::new();
        collect_legacy_yaml_items(&value, "", &mut items)?;
        if legacy_model.is_effectively_empty() {
            // Older startup code persisted a fully expanded default document
            // even though the user had supplied no LifeModel information.
            // Keep only provenance metadata from that compatibility skeleton;
            // enum defaults and zero-valued state are not evidence of intent.
            items.retain(|item| {
                item.disposition == LegacyLifeModelMigrationDispositionV2::MigrationMetadata
            });
        }
        items.sort_by(|left, right| left.source_path.cmp(&right.source_path));

        let review_required_count = items
            .iter()
            .filter(|item| {
                item.disposition == LegacyLifeModelMigrationDispositionV2::ReviewRequired
            })
            .count();
        let external_owner_count = items
            .iter()
            .filter(|item| item.disposition == LegacyLifeModelMigrationDispositionV2::ExternalOwner)
            .count();
        let manual_classification_count = items
            .iter()
            .filter(|item| {
                item.disposition == LegacyLifeModelMigrationDispositionV2::ManualClassification
            })
            .count();
        let not_migrated_count = items
            .iter()
            .filter(|item| item.disposition == LegacyLifeModelMigrationDispositionV2::NotMigrated)
            .count();
        let migration_metadata_count = items
            .iter()
            .filter(|item| {
                item.disposition == LegacyLifeModelMigrationDispositionV2::MigrationMetadata
            })
            .count();
        let contains_sensitive_items = items.iter().any(|item| item.sensitive);

        let source_digest = format!(
            "sha256:{}",
            hex_digest(digest(&SHA256, source.as_bytes()).as_ref())
        );
        let candidates = build_legacy_migration_candidates(&value, &source_digest, &items)?;
        Ok(Self {
            schema_version: "openlife.lifemodel.legacy-migration-preview.v1".into(),
            source_digest,
            items,
            review_required_count,
            external_owner_count,
            manual_classification_count,
            not_migrated_count,
            migration_metadata_count,
            contains_sensitive_items,
            candidates,
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
        model_id: &str,
        selections: &[LegacyLifeModelMigrationSelectionV2],
        non_lifemodel_items_acknowledged: bool,
        confirmed_at: &str,
    ) -> Result<LegacyLifeModelMigrationPlanV2> {
        validate_identifier(model_id, "invalid_lifemodel_v2_migration_model_id")?;
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
                        .as_ref()
                        .unwrap_or(&candidate.proposed_value);
                    let item = migration_candidate_item(
                        candidate,
                        value,
                        &self.source_digest,
                        confirmed_at,
                    )?;
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
        operations
            .sort_by(|left, right| migration_operation_id(left).cmp(migration_operation_id(right)));

        let mut result = LifeModelDocumentV2::empty(model_id);
        let typed_diff = if operations.is_empty() {
            None
        } else {
            let mut value = serde_json::to_value(&result)
                .context("serialize_lifemodel_v2_migration_empty_document")?;
            for operation in &operations {
                apply_typed_operation(&mut value, operation)?;
            }
            result = serde_json::from_value(value)
                .context("deserialize_lifemodel_v2_migration_result")?;
            result.validate()?;
            Some(LifeModelTypedDiffV2 {
                schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
                model_id: model_id.into(),
                base_version: None,
                base_document_digest: None,
                operations,
                result_document_digest: result.digest()?,
            })
        };
        let plan = LegacyLifeModelMigrationPlanV2 {
            schema_version: LIFE_MODEL_V2_LEGACY_MIGRATION_SCHEMA.into(),
            model_id: model_id.into(),
            legacy_source_digest: self.source_digest.clone(),
            included_candidate_ids,
            excluded_candidate_ids,
            non_lifemodel_item_count,
            non_lifemodel_items_acknowledged,
            typed_diff,
            result_document_digest: result.digest()?,
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
                    || !migration_value_accepts_item(&candidate.proposed_value, item)
                    || migration_item_source_refs(item)
                        != migration_candidate_source_refs(candidate, &self.source_digest)
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
        if self.schema_version != LIFE_MODEL_V2_LEGACY_MIGRATION_SCHEMA {
            bail!("unsupported_lifemodel_v2_migration_schema");
        }
        validate_identifier(&self.model_id, "invalid_lifemodel_v2_migration_model_id")?;
        if !is_sha256_digest(&self.legacy_source_digest)
            || !is_sha256_digest(&self.result_document_digest)
        {
            bail!("invalid_lifemodel_v2_migration_digest");
        }
        if self.non_lifemodel_item_count > 0 && !self.non_lifemodel_items_acknowledged {
            bail!("lifemodel_v2_migration_non_lifemodel_items_not_acknowledged");
        }
        let mut decisions = BTreeSet::new();
        for candidate_id in self
            .included_candidate_ids
            .iter()
            .chain(&self.excluded_candidate_ids)
        {
            validate_identifier(candidate_id, "invalid_lifemodel_v2_migration_candidate_id")?;
            if !decisions.insert(candidate_id) {
                bail!("lifemodel_v2_migration_duplicate_candidate_decision");
            }
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

    fn result_document(&self) -> Result<LifeModelDocumentV2> {
        self.validate_contract()?;
        match self.typed_diff.as_ref() {
            Some(diff) => diff.apply_to_version_allow_empty(None),
            None => Ok(LifeModelDocumentV2::empty(&self.model_id)),
        }
    }
}

fn build_legacy_migration_candidates(
    root: &serde_yaml::Value,
    source_digest: &str,
    items: &[LegacyLifeModelMigrationItemV2],
) -> Result<Vec<LegacyLifeModelMigrationCandidateV2>> {
    let review_items = items
        .iter()
        .filter(|item| item.disposition == LegacyLifeModelMigrationDispositionV2::ReviewRequired)
        .collect::<Vec<_>>();
    let mut groups: BTreeMap<(LifeModelSectionV2, String), Vec<&LegacyLifeModelMigrationItemV2>> =
        BTreeMap::new();
    for item in review_items {
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
    for ((section, _group), mut fields) in groups {
        fields.sort_by(|left, right| left.source_path.cmp(&right.source_path));
        let source_paths = fields
            .iter()
            .map(|field| field.source_path.clone())
            .collect::<Vec<_>>();
        let sensitive = fields.iter().any(|field| field.sensitive);
        let values = fields
            .iter()
            .map(|field| {
                Ok((
                    field.source_path.as_str(),
                    yaml_scalar_at_path(root, &field.source_path)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let value = migration_candidate_value(section, &values)?;
        let identity = serde_json::to_vec(&serde_json::json!({
            "sourceDigest": source_digest,
            "section": section,
            "sourcePaths": source_paths,
        }))
        .context("serialize_lifemodel_v2_migration_candidate_identity")?;
        let candidate_digest = hex_digest(digest(&SHA256, &identity).as_ref());
        candidates.push(LegacyLifeModelMigrationCandidateV2 {
            candidate_id: format!("legacy-candidate:{}", &candidate_digest[..24]),
            item_id: format!("legacy:{}", &candidate_digest[..24]),
            source_paths,
            target_section: section,
            proposed_value: value,
            sensitive,
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

fn migration_candidate_value(
    section: LifeModelSectionV2,
    values: &BTreeMap<&str, String>,
) -> Result<LegacyLifeModelMigrationCandidateValueV2> {
    let suffix = |name: &str| {
        values
            .iter()
            .find(|(path, _)| path.rsplit('.').next() == Some(name))
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    let first = || values.values().next().cloned().unwrap_or_default();
    let value = match section {
        LifeModelSectionV2::LongTermGoals => {
            LegacyLifeModelMigrationCandidateValueV2::LongTermGoal {
                direction: suffix("name"),
                meaning: suffix("description"),
            }
        }
        LifeModelSectionV2::ImportantRelationships => {
            LegacyLifeModelMigrationCandidateValueV2::Relationship {
                person_label: suffix("name"),
                relationship: suffix("relationship_type"),
                significance: suffix("notes"),
            }
        }
        LifeModelSectionV2::Capabilities => LegacyLifeModelMigrationCandidateValueV2::Capability {
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
        LifeModelSectionV2::Resources => LegacyLifeModelMigrationCandidateValueV2::Resource {
            name: suffix("name"),
            description: suffix("description"),
        },
        _ => LegacyLifeModelMigrationCandidateValueV2::Statement { statement: first() },
    };
    Ok(value)
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

fn migration_candidate_item(
    candidate: &LegacyLifeModelMigrationCandidateV2,
    value: &LegacyLifeModelMigrationCandidateValueV2,
    source_digest: &str,
    confirmed_at: &str,
) -> Result<LifeModelItemV2> {
    let source_refs = migration_candidate_source_refs(candidate, source_digest);
    let item = match value {
        LegacyLifeModelMigrationCandidateValueV2::Statement { statement } => {
            LifeModelItemV2::Statement(LifeModelStatementV2 {
                id: candidate.item_id.clone(),
                statement: statement.clone(),
                source_refs,
                confirmed_at: confirmed_at.into(),
            })
        }
        LegacyLifeModelMigrationCandidateValueV2::LongTermGoal { direction, meaning } => {
            LifeModelItemV2::LongTermGoal(LifeModelLongTermGoalV2 {
                id: candidate.item_id.clone(),
                direction: direction.clone(),
                meaning: meaning.clone(),
                source_refs,
                confirmed_at: confirmed_at.into(),
            })
        }
        LegacyLifeModelMigrationCandidateValueV2::Relationship {
            person_label,
            relationship,
            significance,
        } => LifeModelItemV2::Relationship(LifeModelRelationshipV2 {
            id: candidate.item_id.clone(),
            person_label: person_label.clone(),
            relationship: relationship.clone(),
            significance: significance.clone(),
            source_refs,
            confirmed_at: confirmed_at.into(),
        }),
        LegacyLifeModelMigrationCandidateValueV2::Capability { name, description } => {
            LifeModelItemV2::Capability(LifeModelCapabilityV2 {
                id: candidate.item_id.clone(),
                name: name.clone(),
                description: description.clone(),
                source_refs,
                confirmed_at: confirmed_at.into(),
            })
        }
        LegacyLifeModelMigrationCandidateValueV2::Resource { name, description } => {
            LifeModelItemV2::Resource(LifeModelResourceV2 {
                id: candidate.item_id.clone(),
                name: name.clone(),
                description: description.clone(),
                source_refs,
                confirmed_at: confirmed_at.into(),
            })
        }
    };
    if !section_accepts_item(candidate.target_section, &item) {
        bail!("lifemodel_v2_migration_edited_value_kind_mismatch");
    }
    Ok(item)
}

fn migration_candidate_source_refs(
    candidate: &LegacyLifeModelMigrationCandidateV2,
    source_digest: &str,
) -> Vec<String> {
    candidate
        .source_paths
        .iter()
        .map(|path| format!("legacy-yaml:{source_digest}#{path}"))
        .collect()
}

fn migration_item_source_refs(item: &LifeModelItemV2) -> Vec<String> {
    match item {
        LifeModelItemV2::Statement(item) => item.source_refs.clone(),
        LifeModelItemV2::LongTermGoal(item) => item.source_refs.clone(),
        LifeModelItemV2::Relationship(item) => item.source_refs.clone(),
        LifeModelItemV2::Capability(item) => item.source_refs.clone(),
        LifeModelItemV2::Resource(item) => item.source_refs.clone(),
    }
}

fn migration_value_accepts_item(
    value: &LegacyLifeModelMigrationCandidateValueV2,
    item: &LifeModelItemV2,
) -> bool {
    matches!(
        (value, item),
        (
            LegacyLifeModelMigrationCandidateValueV2::Statement { .. },
            LifeModelItemV2::Statement(_)
        ) | (
            LegacyLifeModelMigrationCandidateValueV2::LongTermGoal { .. },
            LifeModelItemV2::LongTermGoal(_)
        ) | (
            LegacyLifeModelMigrationCandidateValueV2::Relationship { .. },
            LifeModelItemV2::Relationship(_)
        ) | (
            LegacyLifeModelMigrationCandidateValueV2::Capability { .. },
            LifeModelItemV2::Capability(_)
        ) | (
            LegacyLifeModelMigrationCandidateValueV2::Resource { .. },
            LifeModelItemV2::Resource(_)
        )
    )
}

fn migration_operation_id(operation: &LifeModelTypedOperationV2) -> &str {
    match operation {
        LifeModelTypedOperationV2::Add { item, .. } => item.id(),
        LifeModelTypedOperationV2::Replace { item_id, .. }
        | LifeModelTypedOperationV2::Remove { item_id, .. } => item_id,
    }
}

fn operation_section(operation: &LifeModelTypedOperationV2) -> LifeModelSectionV2 {
    match operation {
        LifeModelTypedOperationV2::Add { section, .. }
        | LifeModelTypedOperationV2::Replace { section, .. }
        | LifeModelTypedOperationV2::Remove { section, .. } => *section,
    }
}

#[derive(Clone, Copy)]
struct LegacyFieldClassification {
    disposition: LegacyLifeModelMigrationDispositionV2,
    target_owner: LegacyLifeModelMigrationOwnerV2,
    target_section: Option<LifeModelSectionV2>,
    reason_code: &'static str,
    sensitive: bool,
}

fn collect_legacy_yaml_items(
    value: &serde_yaml::Value,
    path: &str,
    items: &mut Vec<LegacyLifeModelMigrationItemV2>,
) -> Result<()> {
    match value {
        serde_yaml::Value::Null => Ok(()),
        serde_yaml::Value::Bool(_) | serde_yaml::Value::Number(_) => {
            push_legacy_leaf(value, path, items)
        }
        serde_yaml::Value::String(text) => {
            if text.is_empty() {
                Ok(())
            } else {
                push_legacy_leaf(value, path, items)
            }
        }
        serde_yaml::Value::Sequence(values) => {
            for (index, item) in values.iter().enumerate() {
                let child = format!("{path}[{index}]");
                collect_legacy_yaml_items(item, &child, items)?;
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
                        push_legacy_compatibility_projection(item, &child, items)?;
                    }
                    continue;
                }
                collect_legacy_yaml_items(item, &child, items)?;
            }
            Ok(())
        }
        serde_yaml::Value::Tagged(_) => {
            bail!("legacy_lifemodel_migration_yaml_tags_unsupported")
        }
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

fn push_legacy_compatibility_projection(
    value: &serde_yaml::Value,
    path: &str,
    items: &mut Vec<LegacyLifeModelMigrationItemV2>,
) -> Result<()> {
    if items.len() >= MAX_LEGACY_MIGRATION_ITEMS {
        bail!("legacy_lifemodel_migration_item_limit_exceeded");
    }
    let encoded = serde_yaml::to_string(value)
        .context("serialize_legacy_lifemodel_compatibility_projection")?;
    items.push(LegacyLifeModelMigrationItemV2 {
        source_path: path.into(),
        value_preview: "Derived legacy compatibility projection".into(),
        value_digest: format!(
            "sha256:{}",
            hex_digest(digest(&SHA256, encoded.as_bytes()).as_ref())
        ),
        value_truncated: true,
        disposition: LegacyLifeModelMigrationDispositionV2::ExternalOwner,
        target_owner: LegacyLifeModelMigrationOwnerV2::LegacyCompatibilityProjection,
        target_section: None,
        reason_code: "legacy_compatibility_projection_not_user_truth".into(),
        sensitive: true,
    });
    Ok(())
}

fn push_legacy_leaf(
    value: &serde_yaml::Value,
    path: &str,
    items: &mut Vec<LegacyLifeModelMigrationItemV2>,
) -> Result<()> {
    if items.len() >= MAX_LEGACY_MIGRATION_ITEMS {
        bail!("legacy_lifemodel_migration_item_limit_exceeded");
    }
    if path.is_empty() {
        bail!("legacy_lifemodel_migration_leaf_without_path");
    }
    let normalized_path = normalize_legacy_path(path)?;
    let classification = classify_legacy_path(&normalized_path)
        .ok_or_else(|| anyhow!("unclassified_legacy_lifemodel_field:{normalized_path}"))?;
    if let serde_yaml::Value::Number(number) = value {
        if number.as_f64().is_some_and(|value| !value.is_finite()) {
            bail!("legacy_lifemodel_migration_non_finite_number:{normalized_path}");
        }
    }
    let raw_value = match value {
        serde_yaml::Value::Bool(value) => value.to_string(),
        serde_yaml::Value::Number(value) => value.to_string(),
        serde_yaml::Value::String(value) => value.clone(),
        _ => bail!("legacy_lifemodel_migration_non_scalar_leaf"),
    };
    let (value_preview, value_truncated) = bounded_preview(&raw_value, 240);
    items.push(LegacyLifeModelMigrationItemV2 {
        source_path: path.into(),
        value_preview,
        value_digest: format!(
            "sha256:{}",
            hex_digest(digest(&SHA256, raw_value.as_bytes()).as_ref())
        ),
        value_truncated,
        disposition: classification.disposition,
        target_owner: classification.target_owner,
        target_section: classification.target_section,
        reason_code: classification.reason_code.into(),
        sensitive: classification.sensitive,
    });
    Ok(())
}

fn normalize_legacy_path(path: &str) -> Result<String> {
    let mut normalized = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '[' {
            normalized.push(character);
            continue;
        }
        let mut saw_digit = false;
        while let Some(next) = chars.peek().copied() {
            chars.next();
            if next == ']' {
                break;
            }
            if !next.is_ascii_digit() {
                bail!("invalid_legacy_lifemodel_sequence_path");
            }
            saw_digit = true;
        }
        if !saw_digit {
            bail!("invalid_legacy_lifemodel_sequence_path");
        }
        normalized.push_str("[]");
    }
    Ok(normalized)
}

fn bounded_preview(value: &str, max_chars: usize) -> (String, bool) {
    let mut chars = value.chars();
    let preview: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        (format!("{preview}…"), true)
    } else {
        (preview, false)
    }
}

fn classify_legacy_path(path: &str) -> Option<LegacyFieldClassification> {
    use LegacyLifeModelMigrationDispositionV2 as Disposition;
    use LegacyLifeModelMigrationOwnerV2 as Owner;
    use LifeModelSectionV2 as Section;

    let classification = match path {
        "metadata.version" | "metadata.created_at" | "metadata.updated_at" | "metadata.author" => {
            legacy_classification(
                Disposition::MigrationMetadata,
                Owner::MigrationMetadata,
                None,
                "legacy_document_metadata_only",
                false,
            )
        }
        "identity.name" | "identity.birth_date" => legacy_classification(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::Identity),
            "legacy_identity_requires_user_confirmation",
            path == "identity.birth_date",
        ),
        "identity.values[].name" | "identity.values[].description" => legacy_classification(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::Values),
            "legacy_value_requires_user_confirmation",
            false,
        ),
        "identity.values[].weight" => legacy_classification(
            Disposition::NotMigrated,
            Owner::Unassigned,
            None,
            "legacy_value_weight_is_not_canonical_truth",
            false,
        ),
        "identity.personality_traits[].trait_name" | "identity.personality_traits[].score" => {
            legacy_classification(
                Disposition::NotMigrated,
                Owner::Unassigned,
                None,
                "legacy_personality_score_requires_user_restatement",
                true,
            )
        }
        "identity.life_philosophy" => legacy_classification(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::DecisionPrinciples),
            "legacy_life_philosophy_requires_user_confirmation",
            false,
        ),
        "identity.mission_statement" => legacy_classification(
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
        | "identity.role_definition.personal[]" => legacy_classification(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::Identity),
            "legacy_role_requires_user_confirmation",
            false,
        ),
        "identity.role_definition.responsibilities[]" => legacy_classification(
            Disposition::ManualClassification,
            Owner::Unassigned,
            None,
            "legacy_responsibility_may_be_identity_or_work_context",
            false,
        ),
        "identity.role_definition.boundaries[]" => legacy_classification(
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
        | "identity.voice_style.emoji_usage" => legacy_classification(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::CollaborationPreferences),
            "legacy_voice_style_requires_user_confirmation",
            false,
        ),
        "capabilities.skills[].name" | "capabilities.skills[].description" => {
            legacy_classification(
                Disposition::ReviewRequired,
                Owner::LifeModelV2,
                Some(Section::Capabilities),
                "legacy_user_capability_requires_user_confirmation",
                false,
            )
        }
        "capabilities.skills[].proficiency"
        | "capabilities.knowledge_domains[].level"
        | "capabilities.knowledge_domains[].proficiency" => legacy_classification(
            Disposition::NotMigrated,
            Owner::Unassigned,
            None,
            "legacy_proficiency_score_is_not_canonical_truth",
            false,
        ),
        "capabilities.resources[].name" | "capabilities.resources[].description" => {
            legacy_classification(
                Disposition::ReviewRequired,
                Owner::LifeModelV2,
                Some(Section::Resources),
                "legacy_stable_resource_requires_user_confirmation",
                true,
            )
        }
        "capabilities.resources[].resource_type" | "capabilities.resources[].type" => {
            legacy_classification(
                Disposition::ManualClassification,
                Owner::Unassigned,
                None,
                "legacy_resource_type_has_no_lossless_v2_target",
                true,
            )
        }
        "capabilities.resources[].availability" => legacy_classification(
            Disposition::ExternalOwner,
            Owner::StateStore,
            None,
            "resource_availability_is_current_state",
            true,
        ),
        "capabilities.networks[]" => legacy_classification(
            Disposition::ManualClassification,
            Owner::Unassigned,
            None,
            "legacy_network_could_be_relationship_or_resource",
            true,
        ),
        "capabilities.tools[].name"
        | "capabilities.tools[].proficiency"
        | "capabilities.tools[].description" => legacy_classification(
            Disposition::ExternalOwner,
            Owner::AgentRuntime,
            None,
            "agent_tool_capability_is_not_user_lifemodel",
            false,
        ),
        "capabilities.knowledge_domains[].domain"
        | "capabilities.knowledge_domains[].name"
        | "capabilities.knowledge_domains[].description" => legacy_classification(
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
        | "relationships.collaborators[].notes" => legacy_classification(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::ImportantRelationships),
            "legacy_relationship_requires_sensitive_user_confirmation",
            true,
        ),
        "relationships.inner_circle[].importance"
        | "relationships.mentors[].importance"
        | "relationships.collaborators[].importance" => legacy_classification(
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
        | "preferences.learning_style" => legacy_classification(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::StablePreferences),
            "legacy_stable_preference_requires_user_confirmation",
            false,
        ),
        "preferences.communication_style"
        | "preferences.notification_preferences"
        | "preferences.notification_preferences[]" => legacy_classification(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::CollaborationPreferences),
            "legacy_collaboration_preference_requires_user_confirmation",
            false,
        ),
        "preferences.decision_making_style" => legacy_classification(
            Disposition::ReviewRequired,
            Owner::LifeModelV2,
            Some(Section::DecisionPrinciples),
            "legacy_decision_principle_requires_user_confirmation",
            false,
        ),
        "evolution_rules[]" => legacy_classification(
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
            let owner = if path.ends_with(".related_memories[]") {
                Owner::AgentMemory
            } else {
                Owner::Tasks
            };
            legacy_classification(
                Disposition::ExternalOwner,
                owner,
                None,
                "operational_goal_belongs_outside_lifemodel",
                false,
            )
        }
        _ if path.starts_with("goals.long_term[]") || path.starts_with("goals.life_goals[]") => {
            if path.ends_with(".name") || path.ends_with(".description") {
                legacy_classification(
                    Disposition::ReviewRequired,
                    Owner::LifeModelV2,
                    Some(Section::LongTermGoals),
                    "legacy_long_term_goal_requires_user_confirmation",
                    false,
                )
            } else if path.ends_with(".related_memories[]") {
                legacy_classification(
                    Disposition::ExternalOwner,
                    Owner::AgentMemory,
                    None,
                    "goal_memory_link_belongs_to_agent_memory",
                    false,
                )
            } else {
                legacy_classification(
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
            legacy_classification(
                Disposition::ExternalOwner,
                Owner::AgentMemory,
                None,
                "historical_experience_belongs_to_agent_memory",
                path.starts_with("state.recent_reflections[]"),
            )
        }
        _ if path.starts_with("state.") => legacy_classification(
            Disposition::ExternalOwner,
            Owner::StateStore,
            None,
            "current_state_belongs_to_state_store",
            path.starts_with("state.health_status") || path.starts_with("state.emotional_state"),
        ),
        _ => return None,
    };
    Some(classification)
}

fn legacy_classification(
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

impl LifeModelDocumentV2 {
    pub fn empty(model_id: impl Into<String>) -> Self {
        Self {
            schema_version: LIFE_MODEL_V2_SCHEMA_VERSION.into(),
            model_id: model_id.into(),
            identity: Vec::new(),
            values: Vec::new(),
            long_term_goals: Vec::new(),
            stable_preferences: Vec::new(),
            personal_boundaries: Vec::new(),
            important_relationships: Vec::new(),
            capabilities: Vec::new(),
            resources: Vec::new(),
            decision_principles: Vec::new(),
            collaboration_preferences: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.total_item_count() == 0
    }

    pub fn total_item_count(&self) -> usize {
        self.identity.len()
            + self.values.len()
            + self.long_term_goals.len()
            + self.stable_preferences.len()
            + self.personal_boundaries.len()
            + self.important_relationships.len()
            + self.capabilities.len()
            + self.resources.len()
            + self.decision_principles.len()
            + self.collaboration_preferences.len()
    }

    pub fn summary(&self) -> String {
        if self.is_empty() {
            return "No confirmed long-term user information has been materialized.".into();
        }
        format!(
            "{} confirmed long-term items: {} identity, {} values, {} goals, {} preferences, {} boundaries, {} relationships, {} capabilities, {} resources, {} decision principles, and {} collaboration preferences.",
            self.total_item_count(),
            self.identity.len(),
            self.values.len(),
            self.long_term_goals.len(),
            self.stable_preferences.len(),
            self.personal_boundaries.len(),
            self.important_relationships.len(),
            self.capabilities.len(),
            self.resources.len(),
            self.decision_principles.len(),
            self.collaboration_preferences.len(),
        )
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != LIFE_MODEL_V2_SCHEMA_VERSION {
            bail!("unsupported_lifemodel_v2_schema");
        }
        validate_identifier(&self.model_id, "invalid_lifemodel_v2_model_id")?;
        for count in [
            self.identity.len(),
            self.values.len(),
            self.long_term_goals.len(),
            self.stable_preferences.len(),
            self.personal_boundaries.len(),
            self.important_relationships.len(),
            self.capabilities.len(),
            self.resources.len(),
            self.decision_principles.len(),
            self.collaboration_preferences.len(),
        ] {
            if count > MAX_ITEMS_PER_SECTION {
                bail!("lifemodel_v2_section_item_limit_exceeded");
            }
        }

        let mut ids = BTreeSet::new();
        for item in self
            .identity
            .iter()
            .chain(&self.values)
            .chain(&self.stable_preferences)
            .chain(&self.personal_boundaries)
            .chain(&self.decision_principles)
            .chain(&self.collaboration_preferences)
        {
            validate_common_item(
                &item.id,
                &[&item.statement],
                &item.source_refs,
                &item.confirmed_at,
                &mut ids,
            )?;
        }
        for item in &self.long_term_goals {
            validate_common_item(
                &item.id,
                &[&item.direction, &item.meaning],
                &item.source_refs,
                &item.confirmed_at,
                &mut ids,
            )?;
        }
        for item in &self.important_relationships {
            validate_common_item(
                &item.id,
                &[&item.person_label, &item.relationship, &item.significance],
                &item.source_refs,
                &item.confirmed_at,
                &mut ids,
            )?;
        }
        for item in &self.capabilities {
            validate_common_item(
                &item.id,
                &[&item.name, &item.description],
                &item.source_refs,
                &item.confirmed_at,
                &mut ids,
            )?;
        }
        for item in &self.resources {
            validate_common_item(
                &item.id,
                &[&item.name, &item.description],
                &item.source_refs,
                &item.confirmed_at,
                &mut ids,
            )?;
        }

        if serde_json::to_vec(self)
            .context("serialize_lifemodel_v2_for_size")?
            .len()
            > MAX_DOCUMENT_BYTES
        {
            bail!("lifemodel_v2_document_too_large");
        }
        Ok(())
    }

    pub fn canonical_json(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string(&self.normalized()).context("serialize_lifemodel_v2_canonical_json")
    }

    pub fn digest(&self) -> Result<String> {
        let json = self.canonical_json()?;
        Ok(format!(
            "sha256:{}",
            hex_digest(digest(&SHA256, json.as_bytes()).as_ref())
        ))
    }

    pub fn deterministic_yaml(&self) -> Result<String> {
        self.validate()?;
        serde_yaml::to_string(&self.normalized()).context("serialize_lifemodel_v2_yaml_projection")
    }

    pub fn item(&self, section: LifeModelSectionV2, item_id: &str) -> Option<LifeModelItemV2> {
        match section {
            LifeModelSectionV2::Identity => statement_item(&self.identity, item_id),
            LifeModelSectionV2::Values => statement_item(&self.values, item_id),
            LifeModelSectionV2::LongTermGoals => self
                .long_term_goals
                .iter()
                .find(|item| item.id == item_id)
                .cloned()
                .map(LifeModelItemV2::LongTermGoal),
            LifeModelSectionV2::StablePreferences => {
                statement_item(&self.stable_preferences, item_id)
            }
            LifeModelSectionV2::PersonalBoundaries => {
                statement_item(&self.personal_boundaries, item_id)
            }
            LifeModelSectionV2::ImportantRelationships => self
                .important_relationships
                .iter()
                .find(|item| item.id == item_id)
                .cloned()
                .map(LifeModelItemV2::Relationship),
            LifeModelSectionV2::Capabilities => self
                .capabilities
                .iter()
                .find(|item| item.id == item_id)
                .cloned()
                .map(LifeModelItemV2::Capability),
            LifeModelSectionV2::Resources => self
                .resources
                .iter()
                .find(|item| item.id == item_id)
                .cloned()
                .map(LifeModelItemV2::Resource),
            LifeModelSectionV2::DecisionPrinciples => {
                statement_item(&self.decision_principles, item_id)
            }
            LifeModelSectionV2::CollaborationPreferences => {
                statement_item(&self.collaboration_preferences, item_id)
            }
        }
    }

    pub fn items(&self) -> Vec<(LifeModelSectionV2, LifeModelItemV2)> {
        let mut items = Vec::with_capacity(self.total_item_count());
        for (section, values) in [
            (LifeModelSectionV2::Identity, &self.identity),
            (LifeModelSectionV2::Values, &self.values),
            (
                LifeModelSectionV2::StablePreferences,
                &self.stable_preferences,
            ),
            (
                LifeModelSectionV2::PersonalBoundaries,
                &self.personal_boundaries,
            ),
            (
                LifeModelSectionV2::DecisionPrinciples,
                &self.decision_principles,
            ),
            (
                LifeModelSectionV2::CollaborationPreferences,
                &self.collaboration_preferences,
            ),
        ] {
            items.extend(
                values
                    .iter()
                    .cloned()
                    .map(|item| (section, LifeModelItemV2::Statement(item))),
            );
        }
        items.extend(self.long_term_goals.iter().cloned().map(|item| {
            (
                LifeModelSectionV2::LongTermGoals,
                LifeModelItemV2::LongTermGoal(item),
            )
        }));
        items.extend(self.important_relationships.iter().cloned().map(|item| {
            (
                LifeModelSectionV2::ImportantRelationships,
                LifeModelItemV2::Relationship(item),
            )
        }));
        items.extend(self.capabilities.iter().cloned().map(|item| {
            (
                LifeModelSectionV2::Capabilities,
                LifeModelItemV2::Capability(item),
            )
        }));
        items.extend(self.resources.iter().cloned().map(|item| {
            (
                LifeModelSectionV2::Resources,
                LifeModelItemV2::Resource(item),
            )
        }));
        items.sort_by(|left, right| {
            section_key(left.0)
                .cmp(section_key(right.0))
                .then_with(|| left.1.id().cmp(right.1.id()))
        });
        items
    }

    fn normalized(&self) -> Self {
        let mut document = self.clone();
        normalize_statements(&mut document.identity);
        normalize_statements(&mut document.values);
        normalize_statements(&mut document.stable_preferences);
        normalize_statements(&mut document.personal_boundaries);
        normalize_statements(&mut document.decision_principles);
        normalize_statements(&mut document.collaboration_preferences);
        document
            .long_term_goals
            .sort_by(|left, right| left.id.cmp(&right.id));
        for item in &mut document.long_term_goals {
            normalize_source_refs(&mut item.source_refs);
        }
        document
            .important_relationships
            .sort_by(|left, right| left.id.cmp(&right.id));
        for item in &mut document.important_relationships {
            normalize_source_refs(&mut item.source_refs);
        }
        document
            .capabilities
            .sort_by(|left, right| left.id.cmp(&right.id));
        for item in &mut document.capabilities {
            normalize_source_refs(&mut item.source_refs);
        }
        document
            .resources
            .sort_by(|left, right| left.id.cmp(&right.id));
        for item in &mut document.resources {
            normalize_source_refs(&mut item.source_refs);
        }
        document
    }
}

fn statement_item(values: &[LifeModelStatementV2], item_id: &str) -> Option<LifeModelItemV2> {
    values
        .iter()
        .find(|item| item.id == item_id)
        .cloned()
        .map(LifeModelItemV2::Statement)
}

impl LifeModelUserValueV2 {
    pub fn into_item(
        self,
        item_id: String,
        source_refs: Vec<String>,
        confirmed_at: String,
    ) -> LifeModelItemV2 {
        match self {
            Self::Statement { statement } => LifeModelItemV2::Statement(LifeModelStatementV2 {
                id: item_id,
                statement,
                source_refs,
                confirmed_at,
            }),
            Self::LongTermGoal { direction, meaning } => {
                LifeModelItemV2::LongTermGoal(LifeModelLongTermGoalV2 {
                    id: item_id,
                    direction,
                    meaning,
                    source_refs,
                    confirmed_at,
                })
            }
            Self::Relationship {
                person_label,
                relationship,
                significance,
            } => LifeModelItemV2::Relationship(LifeModelRelationshipV2 {
                id: item_id,
                person_label,
                relationship,
                significance,
                source_refs,
                confirmed_at,
            }),
            Self::Capability { name, description } => {
                LifeModelItemV2::Capability(LifeModelCapabilityV2 {
                    id: item_id,
                    name,
                    description,
                    source_refs,
                    confirmed_at,
                })
            }
            Self::Resource { name, description } => {
                LifeModelItemV2::Resource(LifeModelResourceV2 {
                    id: item_id,
                    name,
                    description,
                    source_refs,
                    confirmed_at,
                })
            }
        }
    }
}

impl LifeModelTypedDiffV2 {
    pub fn from_operations_for_review(
        model_id: &str,
        current: Option<&LifeModelVersionV2>,
        operations: Vec<LifeModelTypedOperationV2>,
        allow_empty_result: bool,
    ) -> Result<Self> {
        let mut document = match current {
            Some(version) => {
                if version.model_id != model_id {
                    bail!("lifemodel_v2_typed_diff_model_mismatch");
                }
                version.document.validate()?;
                version.document.clone()
            }
            None => LifeModelDocumentV2::empty(model_id),
        };
        let mut diff = Self {
            schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
            model_id: model_id.into(),
            base_version: current.map(|version| version.model_version),
            base_document_digest: current.map(|version| version.document_digest.clone()),
            operations,
            result_document_digest: LifeModelDocumentV2::empty(model_id).digest()?,
        };
        diff.validate_contract()?;
        let mut value = serde_json::to_value(&document)
            .context("serialize_lifemodel_v2_typed_diff_review_base")?;
        for operation in &diff.operations {
            apply_typed_operation(&mut value, operation)?;
        }
        document = serde_json::from_value(value)
            .context("deserialize_lifemodel_v2_typed_diff_review_result")?;
        document.validate()?;
        if current.is_some() && document.is_empty() && !allow_empty_result {
            bail!("lifemodel_v2_typed_diff_empty_result_requires_owner_cutover");
        }
        diff.result_document_digest = document.digest()?;
        diff.apply_to_version_for_review(current, allow_empty_result)?;
        Ok(diff)
    }

    pub fn apply_to_version(
        &self,
        current: Option<&LifeModelVersionV2>,
    ) -> Result<LifeModelDocumentV2> {
        self.apply_to_version_with_empty_authority(current, false)
    }

    fn apply_to_version_allow_empty(
        &self,
        current: Option<&LifeModelVersionV2>,
    ) -> Result<LifeModelDocumentV2> {
        self.apply_to_version_with_empty_authority(current, true)
    }

    pub fn apply_to_version_for_review(
        &self,
        current: Option<&LifeModelVersionV2>,
        allow_empty_result: bool,
    ) -> Result<LifeModelDocumentV2> {
        self.apply_to_version_with_empty_authority(current, allow_empty_result)
    }

    pub fn between_versions(
        current: &LifeModelVersionV2,
        target: &LifeModelVersionV2,
    ) -> Result<Self> {
        current.document.validate()?;
        target.document.validate()?;
        if current.model_id != target.model_id || current.model_id != current.document.model_id {
            bail!("lifemodel_v2_rollback_model_mismatch");
        }
        let current_items = current
            .document
            .items()
            .into_iter()
            .map(|(section, item)| (item.id().to_string(), (section, item)))
            .collect::<BTreeMap<_, _>>();
        let target_items = target
            .document
            .items()
            .into_iter()
            .map(|(section, item)| (item.id().to_string(), (section, item)))
            .collect::<BTreeMap<_, _>>();
        let mut operations = Vec::new();
        for (item_id, (section, current_item)) in &current_items {
            match target_items.get(item_id) {
                None => operations.push(LifeModelTypedOperationV2::Remove {
                    section: *section,
                    item_id: item_id.clone(),
                    before_item_digest: life_model_item_digest_v2(current_item)?,
                }),
                Some((target_section, target_item))
                    if target_section != section || target_item != current_item =>
                {
                    if target_section != section {
                        operations.push(LifeModelTypedOperationV2::Remove {
                            section: *section,
                            item_id: item_id.clone(),
                            before_item_digest: life_model_item_digest_v2(current_item)?,
                        });
                        operations.push(LifeModelTypedOperationV2::Add {
                            section: *target_section,
                            item: target_item.clone(),
                        });
                    } else {
                        operations.push(LifeModelTypedOperationV2::Replace {
                            section: *section,
                            item_id: item_id.clone(),
                            before_item_digest: life_model_item_digest_v2(current_item)?,
                            item: target_item.clone(),
                        });
                    }
                }
                Some(_) => {}
            }
        }
        for (item_id, (section, item)) in &target_items {
            if !current_items.contains_key(item_id) {
                operations.push(LifeModelTypedOperationV2::Add {
                    section: *section,
                    item: item.clone(),
                });
            }
        }
        if operations.is_empty() {
            bail!("lifemodel_v2_rollback_target_matches_current");
        }
        operations.sort_by(|left, right| {
            let left_key = (
                section_key(operation_section(left)),
                migration_operation_id(left),
            );
            let right_key = (
                section_key(operation_section(right)),
                migration_operation_id(right),
            );
            left_key.cmp(&right_key)
        });
        let diff = Self {
            schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
            model_id: current.model_id.clone(),
            base_version: Some(current.model_version),
            base_document_digest: Some(current.document_digest.clone()),
            operations,
            result_document_digest: target.document_digest.clone(),
        };
        diff.apply_to_version_for_review(Some(current), true)?;
        Ok(diff)
    }

    fn apply_to_version_with_empty_authority(
        &self,
        current: Option<&LifeModelVersionV2>,
        allow_empty_result: bool,
    ) -> Result<LifeModelDocumentV2> {
        self.validate_contract()?;
        let mut document = match current {
            Some(version) => {
                if self.base_version != Some(version.model_version)
                    || self.base_document_digest.as_deref()
                        != Some(version.document_digest.as_str())
                    || version.model_id != self.model_id
                {
                    bail!("lifemodel_v2_typed_diff_stale_base");
                }
                version.document.validate()?;
                version.document.clone()
            }
            None => {
                if self.base_version.is_some() || self.base_document_digest.is_some() {
                    bail!("lifemodel_v2_typed_diff_initial_base_mismatch");
                }
                LifeModelDocumentV2::empty(self.model_id.clone())
            }
        };

        let mut value =
            serde_json::to_value(&document).context("serialize_lifemodel_v2_typed_diff_base")?;
        for operation in &self.operations {
            apply_typed_operation(&mut value, operation)?;
        }
        document =
            serde_json::from_value(value).context("deserialize_lifemodel_v2_typed_diff_result")?;
        document.validate()?;
        if current.is_some() && document.is_empty() && !allow_empty_result {
            bail!("lifemodel_v2_typed_diff_empty_result_requires_owner_cutover");
        }
        if document.model_id != self.model_id || document.digest()? != self.result_document_digest {
            bail!("lifemodel_v2_typed_diff_result_digest_mismatch");
        }
        Ok(document)
    }

    pub fn validate_contract(&self) -> Result<()> {
        if self.schema_version != LIFE_MODEL_V2_TYPED_DIFF_SCHEMA {
            bail!("unsupported_lifemodel_v2_typed_diff_schema");
        }
        validate_identifier(&self.model_id, "invalid_lifemodel_v2_typed_diff_model_id")?;
        match (&self.base_version, &self.base_document_digest) {
            (None, None) => {}
            (Some(version), Some(digest)) if *version > 0 && is_sha256_digest(digest) => {}
            _ => bail!("invalid_lifemodel_v2_typed_diff_base"),
        }
        if self.operations.is_empty() || self.operations.len() > MAX_TYPED_DIFF_OPERATIONS {
            bail!("lifemodel_v2_typed_diff_operation_count_out_of_bounds");
        }
        if !is_sha256_digest(&self.result_document_digest) {
            bail!("invalid_lifemodel_v2_typed_diff_result_digest");
        }

        let mut targets = BTreeSet::new();
        for operation in &self.operations {
            let (section, item_id, before_digest, item) = match operation {
                LifeModelTypedOperationV2::Add { section, item } => {
                    (*section, item.id(), None, Some(item))
                }
                LifeModelTypedOperationV2::Replace {
                    section,
                    item_id,
                    before_item_digest,
                    item,
                } => (
                    *section,
                    item_id.as_str(),
                    Some(before_item_digest),
                    Some(item),
                ),
                LifeModelTypedOperationV2::Remove {
                    section,
                    item_id,
                    before_item_digest,
                } => (*section, item_id.as_str(), Some(before_item_digest), None),
            };
            validate_identifier(item_id, "invalid_lifemodel_v2_typed_diff_item_id")?;
            if before_digest.is_some_and(|digest| !is_sha256_digest(digest)) {
                bail!("invalid_lifemodel_v2_typed_diff_before_digest");
            }
            if let Some(item) = item {
                if item.id() != item_id || !section_accepts_item(section, item) {
                    bail!("lifemodel_v2_typed_diff_section_item_mismatch");
                }
            }
            if !targets.insert((section_key(section), item_id.to_string())) {
                bail!("duplicate_lifemodel_v2_typed_diff_target");
            }
        }
        Ok(())
    }
}

pub fn life_model_item_digest_v2(item: &LifeModelItemV2) -> Result<String> {
    let value = item_value(item)?;
    digest_json_value(&value)
}

fn apply_typed_operation(
    document: &mut serde_json::Value,
    operation: &LifeModelTypedOperationV2,
) -> Result<()> {
    let (section, target_id) = match operation {
        LifeModelTypedOperationV2::Add { section, item } => (*section, item.id()),
        LifeModelTypedOperationV2::Replace {
            section, item_id, ..
        }
        | LifeModelTypedOperationV2::Remove {
            section, item_id, ..
        } => (*section, item_id.as_str()),
    };
    let items = document
        .get_mut(section_key(section))
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| anyhow!("lifemodel_v2_typed_diff_section_unavailable"))?;
    let existing_index = items
        .iter()
        .position(|value| value.get("id").and_then(serde_json::Value::as_str) == Some(target_id));

    match operation {
        LifeModelTypedOperationV2::Add { item, .. } => {
            if existing_index.is_some() {
                bail!("lifemodel_v2_typed_diff_add_target_exists");
            }
            items.push(item_value(item)?);
        }
        LifeModelTypedOperationV2::Replace {
            before_item_digest,
            item,
            ..
        } => {
            let index =
                existing_index.ok_or_else(|| anyhow!("lifemodel_v2_typed_diff_target_missing"))?;
            if digest_json_value(&items[index])? != *before_item_digest {
                bail!("lifemodel_v2_typed_diff_before_item_mismatch");
            }
            items[index] = item_value(item)?;
        }
        LifeModelTypedOperationV2::Remove {
            before_item_digest, ..
        } => {
            let index =
                existing_index.ok_or_else(|| anyhow!("lifemodel_v2_typed_diff_target_missing"))?;
            if digest_json_value(&items[index])? != *before_item_digest {
                bail!("lifemodel_v2_typed_diff_before_item_mismatch");
            }
            items.remove(index);
        }
    }
    Ok(())
}

fn item_value(item: &LifeModelItemV2) -> Result<serde_json::Value> {
    match item {
        LifeModelItemV2::Statement(value) => serde_json::to_value(value),
        LifeModelItemV2::LongTermGoal(value) => serde_json::to_value(value),
        LifeModelItemV2::Relationship(value) => serde_json::to_value(value),
        LifeModelItemV2::Capability(value) => serde_json::to_value(value),
        LifeModelItemV2::Resource(value) => serde_json::to_value(value),
    }
    .context("serialize_lifemodel_v2_typed_diff_item")
}

fn section_accepts_item(section: LifeModelSectionV2, item: &LifeModelItemV2) -> bool {
    match section {
        LifeModelSectionV2::Identity
        | LifeModelSectionV2::Values
        | LifeModelSectionV2::StablePreferences
        | LifeModelSectionV2::PersonalBoundaries
        | LifeModelSectionV2::DecisionPrinciples
        | LifeModelSectionV2::CollaborationPreferences => {
            matches!(item, LifeModelItemV2::Statement(_))
        }
        LifeModelSectionV2::LongTermGoals => {
            matches!(item, LifeModelItemV2::LongTermGoal(_))
        }
        LifeModelSectionV2::ImportantRelationships => {
            matches!(item, LifeModelItemV2::Relationship(_))
        }
        LifeModelSectionV2::Capabilities => {
            matches!(item, LifeModelItemV2::Capability(_))
        }
        LifeModelSectionV2::Resources => matches!(item, LifeModelItemV2::Resource(_)),
    }
}

fn section_key(section: LifeModelSectionV2) -> &'static str {
    match section {
        LifeModelSectionV2::Identity => "identity",
        LifeModelSectionV2::Values => "values",
        LifeModelSectionV2::LongTermGoals => "longTermGoals",
        LifeModelSectionV2::StablePreferences => "stablePreferences",
        LifeModelSectionV2::PersonalBoundaries => "personalBoundaries",
        LifeModelSectionV2::ImportantRelationships => "importantRelationships",
        LifeModelSectionV2::Capabilities => "capabilities",
        LifeModelSectionV2::Resources => "resources",
        LifeModelSectionV2::DecisionPrinciples => "decisionPrinciples",
        LifeModelSectionV2::CollaborationPreferences => "collaborationPreferences",
    }
}

fn digest_json_value(value: &serde_json::Value) -> Result<String> {
    let bytes = serde_json::to_vec(value).context("serialize_lifemodel_v2_digest_value")?;
    Ok(format!(
        "sha256:{}",
        hex_digest(digest(&SHA256, &bytes).as_ref())
    ))
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn normalize_statements(items: &mut [LifeModelStatementV2]) {
    items.sort_by(|left, right| left.id.cmp(&right.id));
    for item in items {
        normalize_source_refs(&mut item.source_refs);
    }
}

fn normalize_source_refs(source_refs: &mut Vec<String>) {
    source_refs.sort();
    source_refs.dedup();
}

fn validate_common_item(
    id: &str,
    statements: &[&str],
    source_refs: &[String],
    confirmed_at: &str,
    ids: &mut BTreeSet<String>,
) -> Result<()> {
    validate_identifier(id, "invalid_lifemodel_v2_item_id")?;
    if !ids.insert(id.to_string()) {
        bail!("duplicate_lifemodel_v2_item_id");
    }
    for statement in statements {
        let trimmed = statement.trim();
        if trimmed.is_empty()
            || trimmed != *statement
            || statement.chars().count() > MAX_STATEMENT_CHARS
        {
            bail!("invalid_lifemodel_v2_item_content");
        }
    }
    if source_refs.is_empty() || source_refs.len() > MAX_SOURCE_REFS_PER_ITEM {
        bail!("lifemodel_v2_item_source_refs_required");
    }
    let mut unique_sources = BTreeSet::new();
    for source_ref in source_refs {
        let trimmed = source_ref.trim();
        if trimmed.is_empty()
            || trimmed != source_ref
            || source_ref.chars().count() > MAX_SOURCE_REF_CHARS
            || !unique_sources.insert(source_ref)
        {
            bail!("invalid_lifemodel_v2_item_source_ref");
        }
    }
    DateTime::parse_from_rfc3339(confirmed_at)
        .map_err(|_| anyhow!("invalid_lifemodel_v2_confirmed_at"))?;
    Ok(())
}

fn validate_identifier(value: &str, code: &str) -> Result<()> {
    if value.trim() != value
        || value.is_empty()
        || value.chars().count() > MAX_ITEM_ID_CHARS
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_:-.".contains(character))
    {
        bail!(code.to_string());
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelVersionV2 {
    pub model_id: String,
    pub schema_version: String,
    pub model_version: u64,
    pub parent_version: Option<u64>,
    pub parent_digest: Option<String>,
    pub document_digest: String,
    pub version_digest: String,
    pub document: LifeModelDocumentV2,
    pub materialization_id: String,
    pub source_refs: Vec<String>,
    pub created_at: String,
}

pub const LIFE_MODEL_V2_YAML_PROJECTION_SCHEMA: &str = "openlife.lifemodel.v2.yaml-projection.v1";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelHumanProjectionV2 {
    pub schema_version: String,
    pub model_id: String,
    pub model_version: u64,
    pub item_count: usize,
    pub document_digest: String,
    pub yaml_content_digest: String,
    pub projection_digest: String,
    pub yaml: String,
}

impl LifeModelVersionV2 {
    /// Verifies that an in-memory version is bound to the canonical document
    /// and to every persisted version identity field. Store loads perform the
    /// same checks, but runtime consumers use this method to preserve the
    /// boundary when a version is passed across modules or reconstructed in a
    /// test.
    pub fn validate_integrity(&self) -> Result<()> {
        self.document.validate()?;
        if self.model_version == 0
            || self.schema_version != LIFE_MODEL_V2_SCHEMA_VERSION
            || self.document.schema_version != self.schema_version
            || self.document.model_id != self.model_id
            || self.document.digest()? != self.document_digest
        {
            bail!("lifemodel_v2_version_binding_mismatch");
        }
        validate_identifier(
            &self.materialization_id,
            "invalid_lifemodel_v2_materialization_id",
        )?;
        validate_version_source_refs(&self.source_refs)?;
        DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|_| anyhow!("invalid_lifemodel_v2_created_at"))?;
        match self.model_version {
            1 if self.parent_version.is_some() || self.parent_digest.is_some() => {
                bail!("lifemodel_v2_initial_parent_mismatch");
            }
            1 => {}
            version => {
                if self.parent_version != Some(version - 1)
                    || self.parent_digest.as_deref().is_none_or(str::is_empty)
                {
                    bail!("lifemodel_v2_parent_binding_mismatch");
                }
            }
        }
        let expected = calculate_version_digest(
            &self.model_id,
            self.model_version,
            self.parent_version,
            self.parent_digest.as_deref(),
            &self.document_digest,
            &self.materialization_id,
            &self.source_refs,
            &self.created_at,
        )?;
        if expected != self.version_digest {
            bail!("lifemodel_v2_version_digest_mismatch");
        }
        Ok(())
    }

    pub fn human_yaml_projection(&self) -> Result<LifeModelHumanProjectionV2> {
        self.validate_integrity()
            .context("lifemodel_v2_yaml_projection_version_binding_mismatch")?;
        let yaml = self.document.deterministic_yaml()?;
        let yaml_content_digest = format!(
            "sha256:{}",
            hex_digest(digest(&SHA256, yaml.as_bytes()).as_ref())
        );
        let projection_digest = life_model_projection_digest(
            &self.model_id,
            self.model_version,
            self.document.total_item_count(),
            &self.document_digest,
            &yaml_content_digest,
        )?;
        Ok(LifeModelHumanProjectionV2 {
            schema_version: LIFE_MODEL_V2_YAML_PROJECTION_SCHEMA.into(),
            model_id: self.model_id.clone(),
            model_version: self.model_version,
            item_count: self.document.total_item_count(),
            document_digest: self.document_digest.clone(),
            yaml_content_digest,
            projection_digest,
            yaml,
        })
    }
}

impl LifeModelHumanProjectionV2 {
    pub fn validate_binding(
        &self,
        model_id: &str,
        model_version: u64,
        document_digest: &str,
    ) -> Result<()> {
        if self.schema_version != LIFE_MODEL_V2_YAML_PROJECTION_SCHEMA
            || self.model_id != model_id
            || self.model_version != model_version
            || self.document_digest != document_digest
            || self.yaml.trim().is_empty()
        {
            bail!("lifemodel_v2_yaml_projection_binding_mismatch");
        }
        let yaml_content_digest = format!(
            "sha256:{}",
            hex_digest(digest(&SHA256, self.yaml.as_bytes()).as_ref())
        );
        if yaml_content_digest != self.yaml_content_digest
            || life_model_projection_digest(
                model_id,
                model_version,
                self.item_count,
                document_digest,
                &yaml_content_digest,
            )? != self.projection_digest
        {
            bail!("lifemodel_v2_yaml_projection_digest_mismatch");
        }
        let projected: LifeModelDocumentV2 = serde_yaml::from_str(&self.yaml)
            .context("parse_lifemodel_v2_yaml_projection_for_validation")?;
        projected.validate()?;
        if projected.model_id != model_id
            || projected.total_item_count() != self.item_count
            || projected.digest()? != document_digest
        {
            bail!("lifemodel_v2_yaml_projection_document_mismatch");
        }
        Ok(())
    }
}

fn life_model_projection_digest(
    model_id: &str,
    model_version: u64,
    item_count: usize,
    document_digest: &str,
    yaml_content_digest: &str,
) -> Result<String> {
    let payload = serde_json::to_vec(&serde_json::json!({
        "schemaVersion": LIFE_MODEL_V2_YAML_PROJECTION_SCHEMA,
        "modelId": model_id,
        "modelVersion": model_version,
        "itemCount": item_count,
        "documentDigest": document_digest,
        "yamlContentDigest": yaml_content_digest,
    }))
    .context("serialize_lifemodel_v2_yaml_projection_digest")?;
    Ok(format!(
        "sha256:{}",
        hex_digest(digest(&SHA256, &payload).as_ref())
    ))
}

#[derive(Debug, Clone)]
pub(crate) struct LifeModelCommitV2 {
    pub document: LifeModelDocumentV2,
    pub expected_parent_version: Option<u64>,
    pub expected_parent_digest: Option<String>,
    pub materialization_id: String,
    pub source_refs: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LifeModelCommitResultV2 {
    pub version: LifeModelVersionV2,
    pub replayed: bool,
}

#[derive(Clone)]
pub(crate) struct LifeModelV2Store {
    connection: Arc<Mutex<Connection>>,
}

impl LifeModelV2Store {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create_lifemodel_v2_parent:{parent:?}"))?;
        }
        let connection = Connection::open(path).context("open_lifemodel_v2_store")?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .context("configure_lifemodel_v2_busy_timeout")?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .context("configure_lifemodel_v2_wal")?;
        Self::from_connection(connection)
    }

    #[cfg(test)]
    pub(crate) fn new_in_memory() -> Result<Self> {
        Self::from_connection(
            Connection::open_in_memory().context("open_in_memory_lifemodel_v2_store")?,
        )
    }

    fn from_connection(connection: Connection) -> Result<Self> {
        let current_version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("read_lifemodel_v2_store_schema")?;
        if current_version > LIFE_MODEL_V2_STORE_SCHEMA_VERSION {
            bail!("lifemodel_v2_store_schema_is_newer_than_runtime");
        }
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS life_model_v2_versions (
                    model_id TEXT NOT NULL,
                    model_version INTEGER NOT NULL,
                    parent_version INTEGER,
                    parent_digest TEXT,
                    schema_version TEXT NOT NULL,
                    document_json TEXT NOT NULL,
                    document_digest TEXT NOT NULL,
                    version_digest TEXT NOT NULL,
                    materialization_id TEXT NOT NULL,
                    source_refs_json TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    PRIMARY KEY(model_id, model_version),
                    UNIQUE(model_id, materialization_id)
                );
                CREATE TABLE IF NOT EXISTS life_model_v2_heads (
                    model_id TEXT PRIMARY KEY,
                    model_version INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS life_model_v2_cutovers (
                    model_id TEXT PRIMARY KEY,
                    schema_version TEXT NOT NULL,
                    legacy_source_digest TEXT NOT NULL,
                    backup_digest TEXT NOT NULL,
                    model_version INTEGER NOT NULL,
                    document_digest TEXT NOT NULL,
                    proposal_id TEXT NOT NULL UNIQUE,
                    cutover_at TEXT NOT NULL
                );",
            )
            .context("initialize_lifemodel_v2_store")?;
        connection
            .pragma_update(None, "user_version", LIFE_MODEL_V2_STORE_SCHEMA_VERSION)
            .context("set_lifemodel_v2_store_schema")?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub(crate) fn current(&self, model_id: &str) -> Result<Option<LifeModelVersionV2>> {
        validate_identifier(model_id, "invalid_lifemodel_v2_model_id")?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow!("lifemodel_v2_store_lock_poisoned"))?;
        load_current(&connection, model_id)
    }

    pub(crate) fn version(
        &self,
        model_id: &str,
        model_version: u64,
    ) -> Result<Option<LifeModelVersionV2>> {
        validate_identifier(model_id, "invalid_lifemodel_v2_model_id")?;
        if model_version == 0 {
            bail!("invalid_lifemodel_v2_model_version");
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow!("lifemodel_v2_store_lock_poisoned"))?;
        load_version(&connection, model_id, model_version)
    }

    pub(crate) fn history(
        &self,
        model_id: &str,
        limit: usize,
    ) -> Result<Vec<LifeModelVersionHistoryEntryV2>> {
        validate_identifier(model_id, "invalid_lifemodel_v2_model_id")?;
        if limit == 0 || limit > MAX_VERSION_HISTORY_ENTRIES {
            bail!("lifemodel_v2_history_limit_out_of_bounds");
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow!("lifemodel_v2_store_lock_poisoned"))?;
        let mut statement = connection
            .prepare(
                "SELECT model_version FROM life_model_v2_versions
                 WHERE model_id = ?1 ORDER BY model_version DESC LIMIT ?2",
            )
            .context("prepare_lifemodel_v2_history")?;
        let versions = statement
            .query_map(params![model_id, limit as u64], |row| row.get::<_, u64>(0))
            .context("query_lifemodel_v2_history")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("collect_lifemodel_v2_history")?;
        drop(statement);
        versions
            .into_iter()
            .map(|model_version| {
                let version = load_version(&connection, model_id, model_version)?
                    .ok_or_else(|| anyhow!("lifemodel_v2_history_version_missing"))?;
                let parent = version
                    .parent_version
                    .map(|parent_version| load_version(&connection, model_id, parent_version))
                    .transpose()?
                    .flatten();
                if version.parent_version.is_some() && parent.is_none() {
                    bail!("lifemodel_v2_history_parent_missing");
                }
                Ok(LifeModelVersionHistoryEntryV2 {
                    model_id: version.model_id.clone(),
                    model_version: version.model_version,
                    parent_version: version.parent_version,
                    document_digest: version.document_digest.clone(),
                    item_count: version.document.total_item_count(),
                    summary: version.document.summary(),
                    source_refs: version.source_refs.clone(),
                    created_at: version.created_at.clone(),
                    change_summary: version_change_summary(
                        parent.as_ref().map(|parent| &parent.document),
                        &version.document,
                    )?,
                })
            })
            .collect()
    }

    pub(crate) fn cutover(&self, model_id: &str) -> Result<Option<LifeModelV2CutoverReceipt>> {
        validate_identifier(model_id, "invalid_lifemodel_v2_model_id")?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow!("lifemodel_v2_store_lock_poisoned"))?;
        load_cutover(&connection, model_id)
    }

    pub(crate) fn commit(&self, request: LifeModelCommitV2) -> Result<LifeModelCommitResultV2> {
        request.document.validate()?;
        validate_identifier(
            &request.materialization_id,
            "invalid_lifemodel_v2_materialization_id",
        )?;
        validate_version_source_refs(&request.source_refs)?;
        DateTime::parse_from_rfc3339(&request.created_at)
            .map_err(|_| anyhow!("invalid_lifemodel_v2_created_at"))?;
        let document_json = request.document.canonical_json()?;
        let document_digest = request.document.digest()?;

        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow!("lifemodel_v2_store_lock_poisoned"))?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .context("begin_lifemodel_v2_commit")?;

        if let Some(existing) = load_by_materialization(
            &transaction,
            &request.document.model_id,
            &request.materialization_id,
        )? {
            if existing.document_digest != document_digest
                || existing.parent_version != request.expected_parent_version
                || existing.parent_digest != request.expected_parent_digest
                || existing.source_refs != normalized_source_refs(&request.source_refs)
                || existing.created_at != request.created_at
            {
                bail!("lifemodel_v2_materialization_identity_conflict");
            }
            return Ok(LifeModelCommitResultV2 {
                version: existing,
                replayed: true,
            });
        }

        let current = load_current(&transaction, &request.document.model_id)?;
        match current.as_ref() {
            None => {
                if request.expected_parent_version.is_some()
                    || request.expected_parent_digest.is_some()
                {
                    bail!("lifemodel_v2_initial_commit_has_parent");
                }
            }
            Some(current) => {
                if request.expected_parent_version != Some(current.model_version) {
                    bail!("lifemodel_v2_parent_version_conflict");
                }
                if request.expected_parent_digest.as_deref()
                    != Some(current.document_digest.as_str())
                {
                    bail!("lifemodel_v2_parent_digest_conflict");
                }
            }
        }

        let model_version = current
            .as_ref()
            .map(|version| version.model_version + 1)
            .unwrap_or(1);
        let source_refs = normalized_source_refs(&request.source_refs);
        let source_refs_json =
            serde_json::to_string(&source_refs).context("serialize_lifemodel_v2_source_refs")?;
        let version_digest = calculate_version_digest(
            &request.document.model_id,
            model_version,
            request.expected_parent_version,
            request.expected_parent_digest.as_deref(),
            &document_digest,
            &request.materialization_id,
            &source_refs,
            &request.created_at,
        )?;
        transaction
            .execute(
                "INSERT INTO life_model_v2_versions (
                    model_id, model_version, parent_version, parent_digest,
                    schema_version, document_json, document_digest, version_digest,
                    materialization_id, source_refs_json, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    request.document.model_id,
                    model_version,
                    request.expected_parent_version,
                    request.expected_parent_digest,
                    LIFE_MODEL_V2_SCHEMA_VERSION,
                    document_json,
                    document_digest,
                    version_digest,
                    request.materialization_id,
                    source_refs_json,
                    request.created_at,
                ],
            )
            .context("insert_lifemodel_v2_version")?;
        transaction
            .execute(
                "INSERT INTO life_model_v2_heads (model_id, model_version)
                 VALUES (?1, ?2)
                 ON CONFLICT(model_id) DO UPDATE SET model_version = excluded.model_version",
                params![request.document.model_id, model_version],
            )
            .context("advance_lifemodel_v2_head")?;
        transaction
            .commit()
            .context("commit_lifemodel_v2_version")?;

        let version = load_version(&connection, &request.document.model_id, model_version)?
            .ok_or_else(|| anyhow!("committed_lifemodel_v2_version_missing"))?;
        Ok(LifeModelCommitResultV2 {
            version,
            replayed: false,
        })
    }

    pub(crate) fn materialize_typed_diff(
        &self,
        diff: &LifeModelTypedDiffV2,
        materialization_id: &str,
        source_refs: Vec<String>,
        created_at: &str,
    ) -> Result<LifeModelPatchMaterializationResultV2> {
        diff.validate_contract()?;
        validate_identifier(
            materialization_id,
            "invalid_lifemodel_v2_materialization_id",
        )?;
        validate_version_source_refs(&source_refs)?;
        DateTime::parse_from_rfc3339(created_at)
            .map_err(|_| anyhow!("invalid_lifemodel_v2_created_at"))?;
        let existing = {
            let connection = self
                .connection
                .lock()
                .map_err(|_| anyhow!("lifemodel_v2_store_lock_poisoned"))?;
            load_by_materialization(&connection, &diff.model_id, materialization_id)?
        };
        if let Some(existing) = existing {
            if existing.document_digest != diff.result_document_digest
                || existing.parent_version != diff.base_version
                || existing.parent_digest != diff.base_document_digest
                || existing.source_refs != normalized_source_refs(&source_refs)
                || existing.created_at != created_at
            {
                bail!("lifemodel_v2_materialization_identity_conflict");
            }
            return Ok(LifeModelPatchMaterializationResultV2 {
                version: existing,
                replayed: true,
            });
        }
        let current = self.current(&diff.model_id)?;
        // Once a v2 head exists, an authoritative empty document remains a v2
        // head and cannot fall back to legacy data. Cutover is only needed for
        // the initial legacy-to-v2 owner switch.
        let allow_empty_result = current.is_some() || self.cutover(&diff.model_id)?.is_some();
        let document =
            diff.apply_to_version_with_empty_authority(current.as_ref(), allow_empty_result)?;
        let committed = self.commit(LifeModelCommitV2 {
            document,
            expected_parent_version: diff.base_version,
            expected_parent_digest: diff.base_document_digest.clone(),
            materialization_id: materialization_id.into(),
            source_refs,
            created_at: created_at.into(),
        })?;
        Ok(LifeModelPatchMaterializationResultV2 {
            version: committed.version,
            replayed: committed.replayed,
        })
    }

    pub(crate) fn materialize_legacy_migration(
        &self,
        plan: &LegacyLifeModelMigrationPlanV2,
        proposal_id: &str,
        backup_digest: &str,
        cutover_at: &str,
    ) -> Result<LifeModelLegacyMigrationMaterializationResultV2> {
        plan.validate_contract()?;
        validate_identifier(proposal_id, "invalid_lifemodel_v2_migration_proposal_id")?;
        if !is_sha256_digest(backup_digest) || backup_digest != plan.legacy_source_digest {
            bail!("lifemodel_v2_migration_backup_digest_mismatch");
        }
        DateTime::parse_from_rfc3339(cutover_at)
            .map_err(|_| anyhow!("invalid_lifemodel_v2_migration_cutover_at"))?;
        let document = plan.result_document()?;
        if document.digest()? != plan.result_document_digest {
            bail!("lifemodel_v2_migration_result_digest_mismatch");
        }

        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow!("lifemodel_v2_store_lock_poisoned"))?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .context("begin_lifemodel_v2_migration_cutover")?;
        if let Some(existing) = load_cutover(&transaction, &plan.model_id)? {
            if existing.legacy_source_digest != plan.legacy_source_digest
                || existing.backup_digest != backup_digest
                || existing.document_digest != plan.result_document_digest
                || existing.proposal_id != proposal_id
                || existing.cutover_at != cutover_at
            {
                bail!("lifemodel_v2_migration_cutover_identity_conflict");
            }
            let version = load_version(&transaction, &plan.model_id, existing.model_version)?
                .ok_or_else(|| anyhow!("lifemodel_v2_migration_cutover_version_missing"))?;
            return Ok(LifeModelLegacyMigrationMaterializationResultV2 {
                version,
                cutover: existing,
                replayed: true,
            });
        }
        if load_current(&transaction, &plan.model_id)?.is_some() {
            bail!("lifemodel_v2_migration_existing_canonical_head");
        }

        let materialization_id = format!("migration:{proposal_id}");
        let source_refs = normalized_source_refs(&[
            format!("proposal:{proposal_id}"),
            format!("legacy-yaml:{}", plan.legacy_source_digest),
        ]);
        validate_version_source_refs(&source_refs)?;
        let document_json = document.canonical_json()?;
        let document_digest = document.digest()?;
        let source_refs_json = serde_json::to_string(&source_refs)
            .context("serialize_lifemodel_v2_migration_source_refs")?;
        let version_digest = calculate_version_digest(
            &plan.model_id,
            1,
            None,
            None,
            &document_digest,
            &materialization_id,
            &source_refs,
            cutover_at,
        )?;
        transaction
            .execute(
                "INSERT INTO life_model_v2_versions (
                    model_id, model_version, parent_version, parent_digest,
                    schema_version, document_json, document_digest, version_digest,
                    materialization_id, source_refs_json, created_at
                ) VALUES (?1, 1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    plan.model_id,
                    LIFE_MODEL_V2_SCHEMA_VERSION,
                    document_json,
                    document_digest,
                    version_digest,
                    materialization_id,
                    source_refs_json,
                    cutover_at,
                ],
            )
            .context("insert_lifemodel_v2_migration_version")?;
        transaction
            .execute(
                "INSERT INTO life_model_v2_heads (model_id, model_version) VALUES (?1, 1)",
                params![plan.model_id],
            )
            .context("advance_lifemodel_v2_migration_head")?;
        transaction
            .execute(
                "INSERT INTO life_model_v2_cutovers (
                    model_id, schema_version, legacy_source_digest, backup_digest,
                    model_version, document_digest, proposal_id, cutover_at
                ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7)",
                params![
                    plan.model_id,
                    LIFE_MODEL_V2_CUTOVER_SCHEMA,
                    plan.legacy_source_digest,
                    backup_digest,
                    document_digest,
                    proposal_id,
                    cutover_at,
                ],
            )
            .context("insert_lifemodel_v2_cutover_receipt")?;
        transaction
            .commit()
            .context("commit_lifemodel_v2_migration_cutover")?;

        let version = load_version(&connection, &plan.model_id, 1)?
            .ok_or_else(|| anyhow!("committed_lifemodel_v2_migration_version_missing"))?;
        let cutover = load_cutover(&connection, &plan.model_id)?
            .ok_or_else(|| anyhow!("committed_lifemodel_v2_cutover_missing"))?;
        Ok(LifeModelLegacyMigrationMaterializationResultV2 {
            version,
            cutover,
            replayed: false,
        })
    }
}

fn version_change_summary(
    parent: Option<&LifeModelDocumentV2>,
    current: &LifeModelDocumentV2,
) -> Result<LifeModelVersionChangeSummaryV2> {
    let parent_items = parent
        .map(LifeModelDocumentV2::items)
        .unwrap_or_default()
        .into_iter()
        .map(|(section, item)| {
            Ok((
                item.id().to_string(),
                (section, life_model_item_digest_v2(&item)?),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let current_items = current
        .items()
        .into_iter()
        .map(|(section, item)| {
            Ok((
                item.id().to_string(),
                (section, life_model_item_digest_v2(&item)?),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let added = current_items
        .keys()
        .filter(|item_id| !parent_items.contains_key(*item_id))
        .count();
    let removed = parent_items
        .keys()
        .filter(|item_id| !current_items.contains_key(*item_id))
        .count();
    let replaced = current_items
        .iter()
        .filter(|(item_id, value)| {
            parent_items
                .get(*item_id)
                .is_some_and(|parent_value| parent_value != *value)
        })
        .count();
    Ok(LifeModelVersionChangeSummaryV2 {
        added,
        replaced,
        removed,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "this digest must bind every persisted LifeModel version identity field"
)]
pub(crate) fn calculate_version_digest(
    model_id: &str,
    model_version: u64,
    parent_version: Option<u64>,
    parent_digest: Option<&str>,
    document_digest: &str,
    materialization_id: &str,
    source_refs: &[String],
    created_at: &str,
) -> Result<String> {
    let payload = serde_json::to_vec(&serde_json::json!({
        "schemaVersion": LIFE_MODEL_V2_SCHEMA_VERSION,
        "modelId": model_id,
        "modelVersion": model_version,
        "parentVersion": parent_version,
        "parentDigest": parent_digest,
        "documentDigest": document_digest,
        "materializationId": materialization_id,
        "sourceRefs": source_refs,
        "createdAt": created_at,
    }))
    .context("serialize_lifemodel_v2_version_digest")?;
    Ok(format!(
        "sha256:{}",
        hex_digest(digest(&SHA256, &payload).as_ref())
    ))
}

fn validate_version_source_refs(source_refs: &[String]) -> Result<()> {
    if source_refs.is_empty() || source_refs.len() > MAX_SOURCE_REFS_PER_ITEM {
        bail!("lifemodel_v2_version_source_refs_required");
    }
    let normalized = normalized_source_refs(source_refs);
    if normalized.len() != source_refs.len()
        || source_refs.iter().any(|source_ref| {
            source_ref.trim() != source_ref
                || source_ref.is_empty()
                || source_ref.chars().count() > MAX_SOURCE_REF_CHARS
        })
    {
        bail!("invalid_lifemodel_v2_version_source_ref");
    }
    Ok(())
}

fn normalized_source_refs(source_refs: &[String]) -> Vec<String> {
    let mut normalized = source_refs.to_vec();
    normalize_source_refs(&mut normalized);
    normalized
}

fn load_cutover(
    connection: &Connection,
    model_id: &str,
) -> Result<Option<LifeModelV2CutoverReceipt>> {
    let row = connection
        .query_row(
            "SELECT schema_version, legacy_source_digest, backup_digest, model_version,
                    document_digest, proposal_id, cutover_at
             FROM life_model_v2_cutovers WHERE model_id = ?1",
            params![model_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .context("read_lifemodel_v2_cutover")?;
    let Some((
        schema_version,
        legacy_source_digest,
        backup_digest,
        model_version,
        document_digest,
        proposal_id,
        cutover_at,
    )) = row
    else {
        return Ok(None);
    };
    if schema_version != LIFE_MODEL_V2_CUTOVER_SCHEMA
        || !is_sha256_digest(&legacy_source_digest)
        || !is_sha256_digest(&backup_digest)
        || !is_sha256_digest(&document_digest)
        || model_version == 0
    {
        bail!("stored_lifemodel_v2_cutover_invalid");
    }
    validate_identifier(
        &proposal_id,
        "invalid_stored_lifemodel_v2_cutover_proposal_id",
    )?;
    DateTime::parse_from_rfc3339(&cutover_at)
        .map_err(|_| anyhow!("invalid_stored_lifemodel_v2_cutover_at"))?;
    let version = load_version(connection, model_id, model_version)?
        .ok_or_else(|| anyhow!("stored_lifemodel_v2_cutover_version_missing"))?;
    if version.document_digest != document_digest {
        bail!("stored_lifemodel_v2_cutover_document_mismatch");
    }
    Ok(Some(LifeModelV2CutoverReceipt {
        schema_version,
        model_id: model_id.into(),
        legacy_source_digest,
        backup_digest,
        model_version,
        document_digest,
        proposal_id,
        cutover_at,
    }))
}

fn load_current(connection: &Connection, model_id: &str) -> Result<Option<LifeModelVersionV2>> {
    let model_version = connection
        .query_row(
            "SELECT model_version FROM life_model_v2_heads WHERE model_id = ?1",
            params![model_id],
            |row| row.get::<_, u64>(0),
        )
        .optional()
        .context("read_lifemodel_v2_head")?;
    model_version
        .map(|version| load_version(connection, model_id, version))
        .transpose()
        .map(Option::flatten)
}

fn load_by_materialization(
    connection: &Connection,
    model_id: &str,
    materialization_id: &str,
) -> Result<Option<LifeModelVersionV2>> {
    let model_version = connection
        .query_row(
            "SELECT model_version FROM life_model_v2_versions
             WHERE model_id = ?1 AND materialization_id = ?2",
            params![model_id, materialization_id],
            |row| row.get::<_, u64>(0),
        )
        .optional()
        .context("read_lifemodel_v2_materialization")?;
    model_version
        .map(|version| load_version(connection, model_id, version))
        .transpose()
        .map(Option::flatten)
}

fn load_version(
    connection: &Connection,
    model_id: &str,
    model_version: u64,
) -> Result<Option<LifeModelVersionV2>> {
    let row = connection
        .query_row(
            "SELECT parent_version, parent_digest, schema_version, document_json,
                    document_digest, version_digest, materialization_id, source_refs_json, created_at
             FROM life_model_v2_versions WHERE model_id = ?1 AND model_version = ?2",
            params![model_id, model_version],
            |row| {
                Ok((
                    row.get::<_, Option<u64>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()
        .context("read_lifemodel_v2_version")?;
    let Some((
        parent_version,
        parent_digest,
        schema_version,
        document_json,
        document_digest,
        version_digest,
        materialization_id,
        source_refs_json,
        created_at,
    )) = row
    else {
        return Ok(None);
    };
    if schema_version != LIFE_MODEL_V2_SCHEMA_VERSION {
        bail!("stored_lifemodel_v2_schema_mismatch");
    }
    let document: LifeModelDocumentV2 =
        serde_json::from_str(&document_json).context("parse_stored_lifemodel_v2_document")?;
    document.validate()?;
    if document.model_id != model_id || document.digest()? != document_digest {
        bail!("stored_lifemodel_v2_document_digest_mismatch");
    }
    let source_refs: Vec<String> =
        serde_json::from_str(&source_refs_json).context("parse_lifemodel_v2_source_refs")?;
    validate_version_source_refs(&source_refs)?;
    DateTime::parse_from_rfc3339(&created_at)
        .map_err(|_| anyhow!("invalid_stored_lifemodel_v2_created_at"))?;
    validate_identifier(
        &materialization_id,
        "invalid_stored_lifemodel_v2_materialization_id",
    )?;
    match model_version {
        1 if parent_version.is_some() || parent_digest.is_some() => {
            bail!("stored_lifemodel_v2_initial_parent_mismatch");
        }
        1 => {}
        _ => {
            let expected_parent_version = model_version - 1;
            if parent_version != Some(expected_parent_version) {
                bail!("stored_lifemodel_v2_parent_version_mismatch");
            }
            let stored_parent_digest = connection
                .query_row(
                    "SELECT document_digest FROM life_model_v2_versions
                     WHERE model_id = ?1 AND model_version = ?2",
                    params![model_id, expected_parent_version],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .context("read_lifemodel_v2_parent_digest")?;
            if stored_parent_digest.as_deref() != parent_digest.as_deref() {
                bail!("stored_lifemodel_v2_parent_digest_mismatch");
            }
        }
    }
    let expected_version_digest = calculate_version_digest(
        model_id,
        model_version,
        parent_version,
        parent_digest.as_deref(),
        &document_digest,
        &materialization_id,
        &source_refs,
        &created_at,
    )?;
    if expected_version_digest != version_digest {
        bail!("stored_lifemodel_v2_version_digest_mismatch");
    }
    Ok(Some(LifeModelVersionV2 {
        model_id: model_id.into(),
        schema_version,
        model_version,
        parent_version,
        parent_digest,
        document_digest,
        version_digest,
        document,
        materialization_id,
        source_refs,
        created_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_preview_reads_only_fields_present_in_raw_yaml() {
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(
            "metadata:\n  version: '0.1'\nidentity:\n  name: Alice\n",
        )
        .expect("preview");

        let paths = preview
            .items
            .iter()
            .map(|item| item.source_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["identity.name", "metadata.version"]);
        assert_eq!(preview.review_required_count, 1);
        assert_eq!(preview.migration_metadata_count, 1);
        assert!(preview.has_user_content());
        assert!(paths
            .iter()
            .all(|path| !path.starts_with("identity.voice_style")));
    }

    #[test]
    fn legacy_preview_does_not_treat_persisted_default_skeleton_as_user_content() {
        let source = serde_yaml::to_string(&LifeModel::default_model()).unwrap();

        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(&source)
            .expect("default compatibility skeleton preview");

        assert!(!preview.has_user_content());
        assert!(preview.candidates.is_empty());
        assert!(preview.items.iter().all(|item| {
            item.disposition == LegacyLifeModelMigrationDispositionV2::MigrationMetadata
        }));
    }

    #[test]
    fn legacy_preview_classifies_long_term_user_state_memory_and_runtime_fields() {
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(
            r#"
identity:
  values:
    - name: independence
      weight: 8
      description: Decide deliberately.
  personality_traits:
    - trait_name: reflective
      score: 9
goals:
  long_term:
    - name: Build OpenLife
      description: Create a durable personal Agent OS.
      progress: 0.4
      related_memories: [memory-1]
  daily:
    - name: Review migration
      done: false
capabilities:
  skills:
    - name: Product design
      proficiency: 8
      description: Product and architecture work.
  tools:
    - name: shell
      proficiency: 5
      description: Agent tool capability.
state:
  current_focus: Stage 5
  recent_reflections:
    - date: 2026-08-08
      content: Keep product work focused.
      insights: [Avoid platform drift.]
relationships:
  collaborators:
    - name: Partner
      relationship_type: collaborator
      importance: 8
      notes: private
preferences:
  communication_style: conclusion_first
evolution_rules: [Always preserve user authority.]
"#,
        )
        .expect("preview");

        let item = |path: &str| {
            preview
                .items
                .iter()
                .find(|item| item.source_path == path)
                .unwrap_or_else(|| panic!("missing {path}"))
        };
        assert_eq!(
            item("identity.values[0].name").target_section,
            Some(LifeModelSectionV2::Values)
        );
        assert_eq!(
            item("goals.long_term[0].progress").target_owner,
            LegacyLifeModelMigrationOwnerV2::Tasks
        );
        assert_eq!(
            item("goals.long_term[0].related_memories[0]").target_owner,
            LegacyLifeModelMigrationOwnerV2::AgentMemory
        );
        assert_eq!(
            item("state.current_focus").target_owner,
            LegacyLifeModelMigrationOwnerV2::StateStore
        );
        assert_eq!(
            item("capabilities.tools[0].name").target_owner,
            LegacyLifeModelMigrationOwnerV2::AgentRuntime
        );
        assert_eq!(
            item("capabilities.skills[0].proficiency").disposition,
            LegacyLifeModelMigrationDispositionV2::NotMigrated
        );
        assert_eq!(
            item("relationships.collaborators[0].notes").target_section,
            Some(LifeModelSectionV2::ImportantRelationships)
        );
        assert!(item("relationships.collaborators[0].notes").sensitive);
        assert_eq!(
            item("evolution_rules[0]").target_owner,
            LegacyLifeModelMigrationOwnerV2::AgentMemory
        );
        assert!(preview.contains_sensitive_items);
    }

    #[test]
    fn legacy_preview_understands_supported_yaml_aliases_without_inventing_values() {
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(
            r#"
identity:
  role_definition:
    professional: founder
    personal: parent
  voice_style:
    formality_level: formal
capabilities:
  resources:
    - name: Studio
      type: workspace
preferences:
  peak_productivity_times: [morning, evening]
  notification_preferences: [quiet]
goals:
  daily:
    - name: Focus
      completed: true
"#,
        )
        .expect("preview");

        for path in [
            "identity.role_definition.professional",
            "identity.role_definition.personal",
            "identity.voice_style.formality_level",
            "capabilities.resources[0].type",
            "preferences.peak_productivity_times[0]",
            "preferences.notification_preferences[0]",
            "goals.daily[0].completed",
        ] {
            assert!(
                preview.items.iter().any(|item| item.source_path == path),
                "missing alias path {path}"
            );
        }
    }

    #[test]
    fn legacy_preview_fails_closed_for_an_unclassified_field() {
        let error = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(
            "identity:\n  name: Alice\n  invented_future_field: value\n",
        )
        .expect_err("unknown field must block migration preview");
        assert!(error
            .to_string()
            .contains("unclassified_legacy_lifemodel_field:identity.invented_future_field"));
    }

    #[test]
    fn legacy_preview_rejects_non_finite_numbers_and_oversized_sources() {
        let non_finite = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(
            "goals:\n  long_term:\n    - name: Goal\n      progress: .nan\n",
        )
        .expect_err("non-finite legacy value must fail closed");
        assert!(non_finite
            .to_string()
            .contains("legacy_lifemodel_migration_non_finite_number"));

        let oversized = format!(
            "identity:\n  name: {}\n",
            "a".repeat(MAX_LEGACY_MIGRATION_SOURCE_BYTES)
        );
        let error = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(&oversized)
            .expect_err("oversized source must fail closed");
        assert!(error
            .to_string()
            .contains("legacy_lifemodel_migration_source_too_large"));
    }

    #[test]
    fn legacy_preview_collapses_derived_compatibility_projection() {
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(
            "identity:\n  name: Alice\nhs_compatibility:\n  current_state:\n    current_focus: private\n",
        )
        .expect("preview");
        let compatibility = preview
            .items
            .iter()
            .find(|item| item.source_path == "hs_compatibility")
            .expect("compatibility projection receipt");
        assert_eq!(
            compatibility.target_owner,
            LegacyLifeModelMigrationOwnerV2::LegacyCompatibilityProjection
        );
        assert!(compatibility.value_truncated);
        assert!(!preview
            .items
            .iter()
            .any(|item| item.value_preview == "private"));
    }

    #[test]
    fn legacy_migration_requires_an_explicit_decision_for_every_candidate() {
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(
            "metadata:\n  version: '0.1'\nidentity:\n  name: Alice\npreferences:\n  communication_style: concise\n",
        )
        .unwrap();
        assert_eq!(preview.candidates.len(), 2);
        let one = LegacyLifeModelMigrationSelectionV2 {
            candidate_id: preview.candidates[0].candidate_id.clone(),
            decision: LegacyLifeModelMigrationDecisionV2::Include,
            edited_value: None,
        };
        let error = preview
            .build_migration_plan(
                DEFAULT_LIFE_MODEL_V2_MODEL_ID,
                &[one],
                true,
                "2026-08-08T10:00:00Z",
            )
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("lifemodel_v2_migration_candidate_decisions_incomplete"));
    }

    #[test]
    fn legacy_migration_atomically_materializes_version_and_cutover_and_replays() {
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(
            "metadata:\n  version: '0.1'\nidentity:\n  name: Alice\n",
        )
        .unwrap();
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
            .build_migration_plan(
                DEFAULT_LIFE_MODEL_V2_MODEL_ID,
                &selections,
                true,
                "2026-08-08T10:00:00Z",
            )
            .unwrap();
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let first = store
            .materialize_legacy_migration(
                &plan,
                "proposal:migration-1",
                &plan.legacy_source_digest,
                "2026-08-08T10:01:00Z",
            )
            .unwrap();
        assert_eq!(first.version.model_version, 1);
        assert_eq!(first.cutover.model_version, first.version.model_version);
        assert_eq!(first.cutover.document_digest, first.version.document_digest);
        assert!(!first.replayed);
        let replay = store
            .materialize_legacy_migration(
                &plan,
                "proposal:migration-1",
                &plan.legacy_source_digest,
                "2026-08-08T10:01:00Z",
            )
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(store.current("primary").unwrap(), Some(first.version));
        assert_eq!(store.cutover("primary").unwrap(), Some(first.cutover));
    }

    #[test]
    fn excluded_legacy_candidates_create_an_authoritative_empty_owner() {
        let preview =
            LegacyLifeModelMigrationPreviewV2::from_legacy_yaml("identity:\n  name: Alice\n")
                .unwrap();
        let selections = preview
            .candidates
            .iter()
            .map(|candidate| LegacyLifeModelMigrationSelectionV2 {
                candidate_id: candidate.candidate_id.clone(),
                decision: LegacyLifeModelMigrationDecisionV2::Exclude,
                edited_value: None,
            })
            .collect::<Vec<_>>();
        let plan = preview
            .build_migration_plan(
                DEFAULT_LIFE_MODEL_V2_MODEL_ID,
                &selections,
                false,
                "2026-08-08T10:00:00Z",
            )
            .unwrap();
        assert!(plan.typed_diff.is_none());
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let result = store
            .materialize_legacy_migration(
                &plan,
                "proposal:empty-migration",
                &plan.legacy_source_digest,
                "2026-08-08T10:01:00Z",
            )
            .unwrap();
        assert!(result.version.document.is_empty());
        assert!(store.cutover("primary").unwrap().is_some());
    }

    #[test]
    fn migration_cutover_insert_failure_rolls_back_version_and_head() {
        let preview =
            LegacyLifeModelMigrationPreviewV2::from_legacy_yaml("identity:\n  name: Alice\n")
                .unwrap();
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
            .build_migration_plan(
                DEFAULT_LIFE_MODEL_V2_MODEL_ID,
                &selections,
                false,
                "2026-08-08T10:00:00Z",
            )
            .unwrap();
        let store = LifeModelV2Store::new_in_memory().unwrap();
        store
            .connection
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TEMP TRIGGER fail_cutover_insert
                 BEFORE INSERT ON life_model_v2_cutovers
                 BEGIN SELECT RAISE(ABORT, 'forced cutover failure'); END;",
            )
            .unwrap();

        assert!(store
            .materialize_legacy_migration(
                &plan,
                "proposal:atomic-failure",
                &plan.legacy_source_digest,
                "2026-08-08T10:01:00Z",
            )
            .is_err());
        assert!(store.current("primary").unwrap().is_none());
        assert!(store.cutover("primary").unwrap().is_none());
    }

    #[test]
    fn corrupt_cutover_receipt_fails_closed_instead_of_selecting_an_owner() {
        let preview =
            LegacyLifeModelMigrationPreviewV2::from_legacy_yaml("identity:\n  name: Alice\n")
                .unwrap();
        let selections = preview
            .candidates
            .iter()
            .map(|candidate| LegacyLifeModelMigrationSelectionV2 {
                candidate_id: candidate.candidate_id.clone(),
                decision: LegacyLifeModelMigrationDecisionV2::Exclude,
                edited_value: None,
            })
            .collect::<Vec<_>>();
        let plan = preview
            .build_migration_plan(
                DEFAULT_LIFE_MODEL_V2_MODEL_ID,
                &selections,
                false,
                "2026-08-08T10:00:00Z",
            )
            .unwrap();
        let store = LifeModelV2Store::new_in_memory().unwrap();
        store
            .materialize_legacy_migration(
                &plan,
                "proposal:tamper",
                &plan.legacy_source_digest,
                "2026-08-08T10:01:00Z",
            )
            .unwrap();
        store
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE life_model_v2_cutovers SET document_digest = ?1 WHERE model_id = ?2",
                params![
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                    "primary"
                ],
            )
            .unwrap();
        assert!(store.cutover("primary").is_err());
    }

    fn statement(id: &str, value: &str) -> LifeModelStatementV2 {
        LifeModelStatementV2 {
            id: id.into(),
            statement: value.into(),
            source_refs: vec!["message:user:1".into()],
            confirmed_at: "2026-08-08T10:00:00Z".into(),
        }
    }

    fn commit_request(document: LifeModelDocumentV2) -> LifeModelCommitV2 {
        LifeModelCommitV2 {
            document,
            expected_parent_version: None,
            expected_parent_digest: None,
            materialization_id: "proposal:1".into(),
            source_refs: vec!["proposal:1".into()],
            created_at: "2026-08-08T10:01:00Z".into(),
        }
    }

    #[test]
    fn empty_document_is_unknown_not_fictional() {
        let document = LifeModelDocumentV2::empty("primary");
        document.validate().unwrap();
        assert!(document.is_empty());
        assert_eq!(document.total_item_count(), 0);
        assert!(!document.deterministic_yaml().unwrap().contains("期待"));
        assert!(!document.deterministic_yaml().unwrap().contains("energy"));
    }

    #[test]
    fn document_rejects_unknown_fields_and_operational_goal_state() {
        let raw = serde_json::json!({
            "schemaVersion": LIFE_MODEL_V2_SCHEMA_VERSION,
            "modelId": "primary",
            "identity": [],
            "values": [],
            "longTermGoals": [{
                "id": "goal:1",
                "direction": "Build OpenLife",
                "meaning": "Create a useful personal Agent OS",
                "progress": 0.5,
                "deadline": "2026-09-01",
                "sourceRefs": ["message:user:1"],
                "confirmedAt": "2026-08-08T10:00:00Z"
            }],
            "stablePreferences": [],
            "personalBoundaries": [],
            "importantRelationships": [],
            "capabilities": [],
            "resources": [],
            "decisionPrinciples": [],
            "collaborationPreferences": []
        });
        assert!(serde_json::from_value::<LifeModelDocumentV2>(raw).is_err());
    }

    #[test]
    fn document_rejects_duplicate_ids_and_missing_sources() {
        let mut document = LifeModelDocumentV2::empty("primary");
        document
            .identity
            .push(statement("shared:1", "I build products."));
        document
            .values
            .push(statement("shared:1", "I value user autonomy."));
        assert!(document.validate().is_err());

        document.values[0].id = "value:1".into();
        document.values[0].source_refs.clear();
        assert!(document.validate().is_err());
    }

    #[test]
    fn canonical_json_digest_and_yaml_are_order_independent() {
        let mut first = LifeModelDocumentV2::empty("primary");
        first.values.push(statement("value:b", "Clarity matters."));
        first.values.push(statement("value:a", "Autonomy matters."));
        let mut second = first.clone();
        second.values.reverse();

        assert_eq!(
            first.canonical_json().unwrap(),
            second.canonical_json().unwrap()
        );
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(
            first.deterministic_yaml().unwrap(),
            second.deterministic_yaml().unwrap()
        );
    }

    fn initial_statement_diff(item: LifeModelStatementV2) -> LifeModelTypedDiffV2 {
        let mut result = LifeModelDocumentV2::empty("primary");
        result.values.push(item.clone());
        LifeModelTypedDiffV2 {
            schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
            model_id: "primary".into(),
            base_version: None,
            base_document_digest: None,
            operations: vec![LifeModelTypedOperationV2::Add {
                section: LifeModelSectionV2::Values,
                item: LifeModelItemV2::Statement(item),
            }],
            result_document_digest: result.digest().unwrap(),
        }
    }

    #[test]
    fn typed_diff_materializes_exact_add_replace_remove_and_replays() {
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let initial = initial_statement_diff(statement("value:1", "Autonomy matters."));
        let first = store
            .materialize_typed_diff(
                &initial,
                "proposal:add",
                vec!["proposal:add".into()],
                "2026-08-08T10:01:00Z",
            )
            .unwrap();
        assert_eq!(first.version.model_version, 1);
        assert!(!first.replayed);
        let replay = store
            .materialize_typed_diff(
                &initial,
                "proposal:add",
                vec!["proposal:add".into()],
                "2026-08-08T10:01:00Z",
            )
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.version.version_digest, first.version.version_digest);

        let replacement = statement("value:1", "Autonomy and clarity matter.");
        let second_value = statement("value:2", "Care matters.");
        let mut replaced_document = first.version.document.clone();
        replaced_document.values[0] = replacement.clone();
        replaced_document.values.push(second_value.clone());
        let replace = LifeModelTypedDiffV2 {
            schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
            model_id: "primary".into(),
            base_version: Some(first.version.model_version),
            base_document_digest: Some(first.version.document_digest.clone()),
            operations: vec![
                LifeModelTypedOperationV2::Replace {
                    section: LifeModelSectionV2::Values,
                    item_id: "value:1".into(),
                    before_item_digest: life_model_item_digest_v2(&LifeModelItemV2::Statement(
                        first.version.document.values[0].clone(),
                    ))
                    .unwrap(),
                    item: LifeModelItemV2::Statement(replacement),
                },
                LifeModelTypedOperationV2::Add {
                    section: LifeModelSectionV2::Values,
                    item: LifeModelItemV2::Statement(second_value),
                },
            ],
            result_document_digest: replaced_document.digest().unwrap(),
        };
        let second = store
            .materialize_typed_diff(
                &replace,
                "proposal:replace",
                vec!["proposal:replace".into()],
                "2026-08-08T10:02:00Z",
            )
            .unwrap();
        assert_eq!(second.version.model_version, 2);
        assert_eq!(
            second.version.document.values[0].statement,
            "Autonomy and clarity matter."
        );

        let mut removed_document = second.version.document.clone();
        removed_document.values.retain(|item| item.id != "value:1");
        let remove = LifeModelTypedDiffV2 {
            schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
            model_id: "primary".into(),
            base_version: Some(second.version.model_version),
            base_document_digest: Some(second.version.document_digest.clone()),
            operations: vec![LifeModelTypedOperationV2::Remove {
                section: LifeModelSectionV2::Values,
                item_id: "value:1".into(),
                before_item_digest: life_model_item_digest_v2(&LifeModelItemV2::Statement(
                    second.version.document.values[0].clone(),
                ))
                .unwrap(),
            }],
            result_document_digest: removed_document.digest().unwrap(),
        };
        let third = store
            .materialize_typed_diff(
                &remove,
                "proposal:remove",
                vec!["proposal:remove".into()],
                "2026-08-08T10:03:00Z",
            )
            .unwrap();
        assert_eq!(third.version.model_version, 3);
        assert_eq!(third.version.document.values.len(), 1);
        assert_eq!(third.version.document.values[0].id, "value:2");
    }

    #[test]
    fn typed_diff_rejects_stale_tampered_and_wrong_section_without_writes() {
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let initial = initial_statement_diff(statement("value:1", "Autonomy matters."));
        let first = store
            .materialize_typed_diff(
                &initial,
                "proposal:add",
                vec!["proposal:add".into()],
                "2026-08-08T10:01:00Z",
            )
            .unwrap();

        let mut stale = initial.clone();
        stale.operations = vec![LifeModelTypedOperationV2::Add {
            section: LifeModelSectionV2::Values,
            item: LifeModelItemV2::Statement(statement("value:2", "Clarity matters.")),
        }];
        assert!(store
            .materialize_typed_diff(
                &stale,
                "proposal:stale",
                vec!["proposal:stale".into()],
                "2026-08-08T10:02:00Z",
            )
            .unwrap_err()
            .to_string()
            .contains("stale_base"));

        let wrong_section = LifeModelTypedDiffV2 {
            schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
            model_id: "primary".into(),
            base_version: Some(first.version.model_version),
            base_document_digest: Some(first.version.document_digest.clone()),
            operations: vec![LifeModelTypedOperationV2::Add {
                section: LifeModelSectionV2::Capabilities,
                item: LifeModelItemV2::Statement(statement("value:2", "Clarity matters.")),
            }],
            result_document_digest: first.version.document_digest.clone(),
        };
        assert!(wrong_section.validate_contract().is_err());

        let mut tampered = LifeModelTypedDiffV2 {
            schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
            model_id: "primary".into(),
            base_version: Some(first.version.model_version),
            base_document_digest: Some(first.version.document_digest.clone()),
            operations: vec![LifeModelTypedOperationV2::Replace {
                section: LifeModelSectionV2::Values,
                item_id: "value:1".into(),
                before_item_digest:
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
                item: LifeModelItemV2::Statement(statement("value:1", "Changed.")),
            }],
            result_document_digest: first.version.document_digest.clone(),
        };
        assert!(tampered.apply_to_version(Some(&first.version)).is_err());
        tampered.operations = vec![LifeModelTypedOperationV2::Add {
            section: LifeModelSectionV2::Values,
            item: LifeModelItemV2::Statement(statement("value:2", "Clarity matters.")),
        }];
        assert!(tampered.apply_to_version(Some(&first.version)).is_err());

        let empty = LifeModelDocumentV2::empty("primary");
        let remove_last = LifeModelTypedDiffV2 {
            schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
            model_id: "primary".into(),
            base_version: Some(first.version.model_version),
            base_document_digest: Some(first.version.document_digest.clone()),
            operations: vec![LifeModelTypedOperationV2::Remove {
                section: LifeModelSectionV2::Values,
                item_id: "value:1".into(),
                before_item_digest: life_model_item_digest_v2(&LifeModelItemV2::Statement(
                    first.version.document.values[0].clone(),
                ))
                .unwrap(),
            }],
            result_document_digest: empty.digest().unwrap(),
        };
        assert!(remove_last
            .apply_to_version(Some(&first.version))
            .unwrap_err()
            .to_string()
            .contains("empty_result_requires_owner_cutover"));

        let current = store.current("primary").unwrap().unwrap();
        assert_eq!(current.model_version, 1);
        assert_eq!(current.version_digest, first.version.version_digest);
    }

    #[test]
    fn reviewed_remove_last_appends_an_authoritative_empty_version() {
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let first = store
            .materialize_typed_diff(
                &initial_statement_diff(statement("value:1", "Autonomy matters.")),
                "proposal:add-for-clear",
                vec!["proposal:add-for-clear".into()],
                "2026-08-08T10:01:00Z",
            )
            .unwrap()
            .version;
        let remove = LifeModelTypedDiffV2::from_operations_for_review(
            "primary",
            Some(&first),
            vec![LifeModelTypedOperationV2::Remove {
                section: LifeModelSectionV2::Values,
                item_id: "value:1".into(),
                before_item_digest: life_model_item_digest_v2(&LifeModelItemV2::Statement(
                    first.document.values[0].clone(),
                ))
                .unwrap(),
            }],
            true,
        )
        .unwrap();

        let cleared = store
            .materialize_typed_diff(
                &remove,
                "proposal:remove-last",
                vec!["proposal:remove-last".into()],
                "2026-08-08T10:02:00Z",
            )
            .unwrap()
            .version;

        assert_eq!(cleared.model_version, 2);
        assert_eq!(cleared.parent_version, Some(1));
        assert!(cleared.document.is_empty());
        assert_eq!(store.current("primary").unwrap(), Some(cleared));
    }

    #[test]
    fn version_history_is_bounded_and_rollback_diff_appends_without_moving_history() {
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let first = store
            .materialize_typed_diff(
                &initial_statement_diff(statement("value:1", "Autonomy matters.")),
                "proposal:history-1",
                vec!["proposal:history-1".into()],
                "2026-08-08T10:01:00Z",
            )
            .unwrap()
            .version;
        let add_second = LifeModelTypedDiffV2::from_operations_for_review(
            "primary",
            Some(&first),
            vec![LifeModelTypedOperationV2::Add {
                section: LifeModelSectionV2::Values,
                item: LifeModelItemV2::Statement(statement("value:2", "Clarity matters.")),
            }],
            false,
        )
        .unwrap();
        let second = store
            .materialize_typed_diff(
                &add_second,
                "proposal:history-2",
                vec!["proposal:history-2".into()],
                "2026-08-08T10:02:00Z",
            )
            .unwrap()
            .version;

        let rollback = LifeModelTypedDiffV2::between_versions(&second, &first).unwrap();
        let third = store
            .materialize_typed_diff(
                &rollback,
                "proposal:rollback-to-1",
                vec![
                    "proposal:rollback-to-1".into(),
                    format!("lifemodel-version:primary:1:{}", first.document_digest),
                ],
                "2026-08-08T10:03:00Z",
            )
            .unwrap()
            .version;

        assert_eq!(third.model_version, 3);
        assert_eq!(third.parent_version, Some(2));
        assert_eq!(third.document, first.document);
        assert_eq!(store.version("primary", 1).unwrap(), Some(first));
        assert_eq!(store.version("primary", 2).unwrap(), Some(second));
        let history = store.history("primary", 3).unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].model_version, 3);
        assert_eq!(history[0].change_summary.removed, 1);
        assert_eq!(history[1].change_summary.added, 1);
        assert!(store
            .history("primary", MAX_VERSION_HISTORY_ENTRIES + 1)
            .is_err());
    }

    #[test]
    fn rollback_diff_rejects_same_content_and_corrupt_historical_version_fails_closed() {
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let first = store
            .materialize_typed_diff(
                &initial_statement_diff(statement("value:1", "Autonomy matters.")),
                "proposal:rollback-corrupt-1",
                vec!["proposal:rollback-corrupt-1".into()],
                "2026-08-08T10:01:00Z",
            )
            .unwrap()
            .version;
        assert!(LifeModelTypedDiffV2::between_versions(&first, &first)
            .unwrap_err()
            .to_string()
            .contains("target_matches_current"));

        store
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE life_model_v2_versions SET document_digest = ?1 WHERE model_id = ?2 AND model_version = 1",
                params![
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                    "primary"
                ],
            )
            .unwrap();
        assert!(store.version("primary", 1).is_err());
        assert!(store.history("primary", 1).is_err());
    }

    #[test]
    fn human_yaml_projection_is_deterministic_and_bound_to_exact_version() {
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let mut document = LifeModelDocumentV2::empty("primary");
        document
            .values
            .push(statement("value:autonomy", "Autonomy matters."));
        let version = store.commit(commit_request(document)).unwrap().version;

        let first = version.human_yaml_projection().unwrap();
        let second = version.human_yaml_projection().unwrap();
        assert_eq!(first, second);
        first
            .validate_binding(
                &version.model_id,
                version.model_version,
                &version.document_digest,
            )
            .unwrap();
        assert!(first.yaml.contains("Autonomy matters."));
        assert_eq!(first.model_version, 1);
    }

    #[test]
    fn human_yaml_projection_rejects_content_tamper_and_version_transplant() {
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let mut document = LifeModelDocumentV2::empty("primary");
        document
            .values
            .push(statement("value:clarity", "Clarity matters."));
        let version = store.commit(commit_request(document)).unwrap().version;
        let projection = version.human_yaml_projection().unwrap();

        let mut tampered = projection.clone();
        tampered.yaml.push_str("\n# changed\n");
        assert!(tampered
            .validate_binding(
                &version.model_id,
                version.model_version,
                &version.document_digest,
            )
            .is_err());
        assert!(projection
            .validate_binding(
                &version.model_id,
                version.model_version + 1,
                &version.document_digest,
            )
            .is_err());
    }

    #[test]
    fn store_commits_exact_parent_chain_and_survives_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("life_model_v2.db");
        let store = LifeModelV2Store::open(&path).unwrap();
        assert!(store.current("primary").unwrap().is_none());

        let mut first_document = LifeModelDocumentV2::empty("primary");
        first_document
            .values
            .push(statement("value:1", "Autonomy matters."));
        let first = store
            .commit(commit_request(first_document.clone()))
            .unwrap();
        assert_eq!(first.version.model_version, 1);
        assert!(!first.replayed);

        let replay = store.commit(commit_request(first_document)).unwrap();
        assert_eq!(replay.version.model_version, 1);
        assert!(replay.replayed);

        let mut second_document = first.version.document.clone();
        second_document
            .identity
            .push(statement("identity:1", "I am a product builder."));
        let second = store
            .commit(LifeModelCommitV2 {
                document: second_document,
                expected_parent_version: Some(1),
                expected_parent_digest: Some(first.version.document_digest.clone()),
                materialization_id: "proposal:2".into(),
                source_refs: vec!["proposal:2".into()],
                created_at: "2026-08-08T10:02:00Z".into(),
            })
            .unwrap();
        assert_eq!(second.version.model_version, 2);
        drop(store);

        let reopened = LifeModelV2Store::open(&path).unwrap();
        assert_eq!(
            reopened.current("primary").unwrap().unwrap(),
            second.version
        );
    }

    #[test]
    fn store_fails_closed_for_stale_parent_and_identity_drift() {
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let mut first_document = LifeModelDocumentV2::empty("primary");
        first_document
            .values
            .push(statement("value:1", "Autonomy matters."));
        let first = store.commit(commit_request(first_document)).unwrap();
        let mut next = first.version.document.clone();
        next.values.push(statement("value:2", "Evidence matters."));

        let stale = LifeModelCommitV2 {
            document: next.clone(),
            expected_parent_version: Some(99),
            expected_parent_digest: Some(first.version.document_digest.clone()),
            materialization_id: "proposal:2".into(),
            source_refs: vec!["proposal:2".into()],
            created_at: "2026-08-08T10:02:00Z".into(),
        };
        assert!(store.commit(stale).is_err());

        let drift = LifeModelCommitV2 {
            document: next,
            expected_parent_version: None,
            expected_parent_digest: None,
            materialization_id: "proposal:1".into(),
            source_refs: vec!["proposal:1".into()],
            created_at: "2026-08-08T10:03:00Z".into(),
        };
        assert!(store.commit(drift).is_err());
        assert_eq!(store.current("primary").unwrap().unwrap().model_version, 1);
    }

    #[test]
    fn store_detects_document_tampering() {
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let first = store
            .commit(commit_request(LifeModelDocumentV2::empty("primary")))
            .unwrap();
        store
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE life_model_v2_versions SET document_json = ?1
                 WHERE model_id = ?2 AND model_version = ?3",
                params![
                    LifeModelDocumentV2::empty("tampered")
                        .canonical_json()
                        .unwrap(),
                    "primary",
                    first.version.model_version
                ],
            )
            .unwrap();
        assert!(store.current("primary").is_err());
    }

    #[test]
    fn store_detects_source_relation_tampering() {
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let first = store
            .commit(commit_request(LifeModelDocumentV2::empty("primary")))
            .unwrap();
        store
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE life_model_v2_versions SET source_refs_json = ?1
                 WHERE model_id = ?2 AND model_version = ?3",
                params![
                    serde_json::to_string(&vec!["proposal:tampered"]).unwrap(),
                    "primary",
                    first.version.model_version
                ],
            )
            .unwrap();
        assert!(store.current("primary").is_err());
    }
}
