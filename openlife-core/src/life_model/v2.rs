//! Versioned canonical LifeModel document.
//!
//! This module defines the user-owned LifeModel boundary and its append-only
//! SQLite authority. Historical YAML compatibility is intentionally not part
//! of the shipped model.

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

pub const LIFE_MODEL_V2_SCHEMA_VERSION: &str = "openlife.lifemodel.v2";
pub const DEFAULT_LIFE_MODEL_V2_MODEL_ID: &str = "primary";
const LIFE_MODEL_V2_STORE_SCHEMA_VERSION: i64 = 2;
const MAX_DOCUMENT_BYTES: usize = 256 * 1024;
const MAX_ITEMS_PER_SECTION: usize = 512;
const MAX_ITEM_ID_CHARS: usize = 160;
const MAX_STATEMENT_CHARS: usize = 4_096;
const MAX_SOURCE_REFS_PER_ITEM: usize = 32;
const MAX_SOURCE_REF_CHARS: usize = 256;
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
            bail!("lifemodel_v2_typed_diff_empty_result_requires_existing_owner");
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
                operation_item_id(left),
            );
            let right_key = (
                section_key(operation_section(right)),
                operation_item_id(right),
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
            bail!("lifemodel_v2_typed_diff_empty_result_requires_existing_owner");
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

fn operation_section(operation: &LifeModelTypedOperationV2) -> LifeModelSectionV2 {
    match operation {
        LifeModelTypedOperationV2::Add { section, .. }
        | LifeModelTypedOperationV2::Replace { section, .. }
        | LifeModelTypedOperationV2::Remove { section, .. } => *section,
    }
}

fn operation_item_id(operation: &LifeModelTypedOperationV2) -> &str {
    match operation {
        LifeModelTypedOperationV2::Add { item, .. } => item.id(),
        LifeModelTypedOperationV2::Replace { item_id, .. }
        | LifeModelTypedOperationV2::Remove { item_id, .. } => item_id,
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
                ",
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
        // A first write must establish at least one confirmed item. Once a
        // canonical head exists, a reviewed change may clear it completely.
        let allow_empty_result = current.is_some();
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
            .contains("empty_result_requires_existing_owner"));

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
