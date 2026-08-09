use crate::agent::product_read_model::{
    EvidenceRef, EvidenceSensitivity, ExternalTransmissionStatus,
};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType};
use crate::life_model::v2::{
    LegacyLifeModelMigrationPlanV2, LifeModelItemV2, LifeModelTypedDiffV2,
    LifeModelTypedOperationV2, LifeModelUserValueV2, LIFE_MODEL_V2_LEGACY_MIGRATION_PATH,
    LIFE_MODEL_V2_TYPED_DIFF_PATH,
};
use crate::tool_permissions::ActionBoundToolPermissionScope;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const MAX_READABLE_TEXT_CHARS: usize = 320;
const MAX_READABLE_DETAIL_BYTES: usize = 1_200;
const MAX_READABLE_DEPTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewReadableValueKind {
    Text,
    Number,
    Boolean,
    List,
    Object,
    Redacted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewReadableValue {
    pub kind: ReviewReadableValueKind,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub sensitivity: EvidenceSensitivity,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecisionContextStatus {
    Ready,
    Incomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionScopeKind {
    ActionBound,
    NetworkPolicy,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionPolicyKind {
    AllowOnce,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionRequestDigestKind {
    Input,
    Endpoint,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionTransmissionBoundary {
    pub external_transmission: ExternalTransmissionStatus,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_label: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDecisionContext {
    pub status: PermissionDecisionContextStatus,
    pub scope_kind: PermissionScopeKind,
    pub policy: PermissionPolicyKind,
    pub tool_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub capability_labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_target_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_target_label: Option<String>,
    pub purpose_summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_digest: Option<String>,
    pub request_digest_kind: PermissionRequestDigestKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_length_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_step_index: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_policy_decision_id: Option<String>,
    pub transmission_boundary: PermissionTransmissionBoundary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    pub revocation_summary: String,
    #[serde(default)]
    pub missing_fields: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
}

impl PermissionDecisionContext {
    pub fn is_ready(&self) -> bool {
        self.status == PermissionDecisionContextStatus::Ready
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedActionReviewContract {
    pub capability_id: String,
    pub operation: String,
    pub confirmation_summary: String,
    pub terminal_evidence_summary: String,
    pub effect_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelLearningReviewContext {
    pub candidate_id: String,
    pub candidate_snapshot_digest: String,
    pub section: String,
    pub proposed_statement: String,
    pub explicitness: String,
    pub stability: String,
    pub sensitivity: String,
    pub conflict_status: String,
    pub support_count: usize,
    pub independent_support_count: usize,
    pub confirmed_at: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub source_kinds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDecisionContext {
    pub review_item_id: String,
    pub title: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<ReviewReadableValue>,
    pub after: ReviewReadableValue,
    pub reason_summary: String,
    pub source_summary: String,
    pub impact_summary: String,
    #[serde(default)]
    pub affected_object_labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<PermissionDecisionContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_contract: Option<GovernedActionReviewContract>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub life_model_learning: Option<LifeModelLearningReviewContext>,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
}

pub fn build_review_decision_context(
    proposal: &AgentProposal,
    evidence_refs: &[EvidenceRef],
) -> ReviewDecisionContext {
    let life_model_v2_diff = reviewed_lifemodel_v2_diff(proposal);
    let legacy_migration = reviewed_legacy_lifemodel_migration(proposal);
    let permission = (proposal.proposal_type == ProposalType::ToolPermission)
        .then(|| build_permission_decision_context(proposal, evidence_refs));
    let life_model_learning = reviewed_lifemodel_learning_context(proposal);
    let after = if permission.is_some() {
        ReviewReadableValue {
            kind: ReviewReadableValueKind::Redacted,
            summary: "Exact permission scope is projected in the permission decision context."
                .into(),
            detail: None,
            sensitivity: EvidenceSensitivity::Redacted,
            truncated: false,
        }
    } else if let Some(diff) = life_model_v2_diff.as_ref() {
        readable_lifemodel_v2_diff(diff)
    } else if let Some(plan) = legacy_migration.as_ref() {
        plan.typed_diff.as_ref().map_or_else(
            || ReviewReadableValue {
                kind: ReviewReadableValueKind::List,
                summary: "Create an authoritative empty LifeModel v2".into(),
                detail: Some(format!(
                    "0 included; {} explicitly excluded",
                    plan.excluded_candidate_ids.len()
                )),
                sensitivity: EvidenceSensitivity::LocalPrivate,
                truncated: false,
            },
            readable_lifemodel_v2_diff,
        )
    } else {
        readable_value(&proposal.after)
    };
    let before = life_model_v2_diff
        .as_ref()
        .map(|diff| ReviewReadableValue {
            kind: ReviewReadableValueKind::Object,
            summary: diff.base_version.map_or_else(
                || "Empty LifeModel v2".into(),
                |version| format!("LifeModel v2 version {version}"),
            ),
            detail: diff.base_document_digest.clone(),
            sensitivity: EvidenceSensitivity::LocalPrivate,
            truncated: false,
        })
        .or_else(|| {
            legacy_migration.as_ref().map(|plan| ReviewReadableValue {
                kind: ReviewReadableValueKind::Object,
                summary: "Reviewed legacy YAML snapshot".into(),
                detail: Some(format!(
            "{}; {} candidate(s) included; {} excluded; {} other legacy field(s) remain outside v2",
            plan.legacy_source_digest,
            plan.included_candidate_ids.len(),
            plan.excluded_candidate_ids.len(),
            plan.non_lifemodel_item_count,
        )),
                sensitivity: EvidenceSensitivity::LocalPrivate,
                truncated: false,
            })
        });

    ReviewDecisionContext {
        review_item_id: proposal.id.clone(),
        title: proposal_title(proposal),
        summary: proposal_summary(proposal),
        before: before.or_else(|| proposal.before.as_ref().map(readable_value)),
        after,
        reason_summary: bounded_text(&proposal.reason, "No reason was supplied."),
        source_summary: if life_model_learning.is_some() {
            "User-confirmed LifeModel learning evidence".into()
        } else {
            proposal_source_summary(proposal.source).into()
        },
        impact_summary: impact_summary(proposal).into(),
        affected_object_labels: vec![affected_object_label(proposal)],
        expires_at: proposal.expires_at,
        permission,
        action_contract: build_governed_action_review_contract(proposal),
        life_model_learning,
        evidence_refs: evidence_refs.to_vec(),
    }
}

pub fn is_lifemodel_learning_review(proposal: &AgentProposal) -> bool {
    reviewed_lifemodel_learning_context(proposal).is_some()
}

fn reviewed_lifemodel_learning_context(
    proposal: &AgentProposal,
) -> Option<LifeModelLearningReviewContext> {
    let diff = reviewed_lifemodel_v2_diff(proposal)?;
    let before = proposal.before.as_ref()?;
    if before.get("schema")?.as_str()? != "openlife.lifemodel.learning.review.v1"
        || before.get("conflictStatus")?.as_str()? != "none"
        || diff.operations.len() != 1
    {
        return None;
    }
    let (section, statement) = match &diff.operations[0] {
        LifeModelTypedOperationV2::Add {
            section,
            item: LifeModelItemV2::Statement(item),
        } => (
            format!("{section:?}").to_ascii_lowercase(),
            item.statement.clone(),
        ),
        _ => return None,
    };
    let proposed_statement =
        match serde_json::from_value::<LifeModelUserValueV2>(before.get("proposedValue")?.clone())
            .ok()?
        {
            LifeModelUserValueV2::Statement { statement } => statement,
            _ => return None,
        };
    if statement != proposed_statement {
        return None;
    }
    let strings = |key: &str| {
        before
            .get(key)
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .take(8)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    Some(LifeModelLearningReviewContext {
        candidate_id: before.get("candidateId")?.as_str()?.to_string(),
        candidate_snapshot_digest: before.get("candidateSnapshotDigest")?.as_str()?.to_string(),
        section,
        proposed_statement,
        explicitness: before.get("explicitness")?.as_str()?.to_string(),
        stability: before.get("stability")?.as_str()?.to_string(),
        sensitivity: before.get("sensitivity")?.as_str()?.to_string(),
        conflict_status: "none".into(),
        support_count: before.get("supportCount")?.as_u64()? as usize,
        independent_support_count: before.get("independentSupportCount")?.as_u64()? as usize,
        confirmed_at: before.get("confirmedAt")?.as_str()?.to_string(),
        source_refs: strings("sourceRefs"),
        source_kinds: strings("sourceKinds"),
    })
}

fn reviewed_legacy_lifemodel_migration(
    proposal: &AgentProposal,
) -> Option<LegacyLifeModelMigrationPlanV2> {
    (proposal.proposal_type == ProposalType::LifeModelUpdate
        && proposal.affected_path == LIFE_MODEL_V2_LEGACY_MIGRATION_PATH)
        .then(|| serde_json::from_value(proposal.after.clone()).ok())
        .flatten()
        .filter(|plan: &LegacyLifeModelMigrationPlanV2| plan.validate_contract().is_ok())
}

fn reviewed_lifemodel_v2_diff(proposal: &AgentProposal) -> Option<LifeModelTypedDiffV2> {
    (proposal.proposal_type == ProposalType::LifeModelUpdate
        && proposal.affected_path == LIFE_MODEL_V2_TYPED_DIFF_PATH)
        .then(|| serde_json::from_value(proposal.after.clone()).ok())
        .flatten()
        .filter(|diff: &LifeModelTypedDiffV2| diff.validate_contract().is_ok())
}

fn readable_lifemodel_v2_diff(diff: &LifeModelTypedDiffV2) -> ReviewReadableValue {
    let mut add = 0;
    let mut replace = 0;
    let mut remove = 0;
    let mut lines = Vec::new();
    for operation in &diff.operations {
        let (verb, section, item_id, content) = match operation {
            LifeModelTypedOperationV2::Add { section, item } => {
                add += 1;
                ("add", section, item_id(item), Some(item_summary(item)))
            }
            LifeModelTypedOperationV2::Replace {
                section,
                item_id,
                item,
                ..
            } => {
                replace += 1;
                (
                    "replace",
                    section,
                    item_id.as_str(),
                    Some(item_summary(item)),
                )
            }
            LifeModelTypedOperationV2::Remove {
                section, item_id, ..
            } => {
                remove += 1;
                ("remove", section, item_id.as_str(), None)
            }
        };
        let section = format!("{section:?}").to_ascii_lowercase();
        lines.push(match content {
            Some(content) => format!("{verb} {section}/{item_id}: {content}"),
            None => format!("{verb} {section}/{item_id}"),
        });
    }
    let detail = lines.join("\n");
    ReviewReadableValue {
        kind: ReviewReadableValueKind::List,
        summary: format!(
            "{} LifeModel change(s): {add} add, {replace} replace, {remove} remove",
            diff.operations.len()
        ),
        detail: (detail.len() <= MAX_READABLE_DETAIL_BYTES).then_some(detail),
        sensitivity: EvidenceSensitivity::LocalPrivate,
        truncated: lines.join("\n").len() > MAX_READABLE_DETAIL_BYTES,
    }
}

fn item_id(item: &LifeModelItemV2) -> &str {
    match item {
        LifeModelItemV2::Statement(item) => &item.id,
        LifeModelItemV2::LongTermGoal(item) => &item.id,
        LifeModelItemV2::Relationship(item) => &item.id,
        LifeModelItemV2::Capability(item) => &item.id,
        LifeModelItemV2::Resource(item) => &item.id,
    }
}

fn item_summary(item: &LifeModelItemV2) -> String {
    match item {
        LifeModelItemV2::Statement(item) => bounded_text(&item.statement, "Empty statement"),
        LifeModelItemV2::LongTermGoal(item) => bounded_text(
            &format!("{} — {}", item.direction, item.meaning),
            "Empty long-term goal",
        ),
        LifeModelItemV2::Relationship(item) => bounded_text(
            &format!(
                "{} — {} — {}",
                item.person_label, item.relationship, item.significance
            ),
            "Empty relationship",
        ),
        LifeModelItemV2::Capability(item) => bounded_text(
            &format!("{} — {}", item.name, item.description),
            "Empty capability",
        ),
        LifeModelItemV2::Resource(item) => bounded_text(
            &format!("{} — {}", item.name, item.description),
            "Empty resource",
        ),
    }
}

fn build_governed_action_review_contract(
    proposal: &AgentProposal,
) -> Option<GovernedActionReviewContract> {
    let contract = match proposal.proposal_type {
        ProposalType::ExternalWriteAction => {
            let operation = proposal_operation(proposal).unwrap_or("create");
            GovernedActionReviewContract {
                capability_id: "filesystem.write".into(),
                operation: operation.into(),
                confirmation_summary: match operation {
                    "move" | "trash" | "restore" => {
                        "Confirm one exact source, destination, and reviewed source digest."
                    }
                    _ => "Confirm one exact path and reviewed content digest.",
                }
                .into(),
                terminal_evidence_summary:
                    "Completion requires a refreshed matching filesystem materialization receipt."
                        .into(),
                effect_boundary: "local_filesystem".into(),
            }
        }
        ProposalType::ScheduledTask if proposal_tool(proposal) == Some("calendar.propose_event") => {
            GovernedActionReviewContract {
                capability_id: "calendar.local_projection".into(),
                operation: "create_local_calendar_projection".into(),
                confirmation_summary:
                    "Confirm the exact local task title, scheduled time, and optional ICS projection."
                        .into(),
                terminal_evidence_summary:
                    "Completion proves the local scheduled task. A configured ICS file is only a local projection and never proves a system or remote calendar event."
                        .into(),
                effect_boundary: "local_task_and_optional_ics_projection".into(),
            }
        }
        ProposalType::ScheduledTask => GovernedActionReviewContract {
            capability_id: "tasks.schedule".into(),
            operation: "create_scheduled_task".into(),
            confirmation_summary: "Confirm the exact task title and scheduled time.".into(),
            terminal_evidence_summary:
                "Completion requires the refreshed scheduled-task materialization state.".into(),
            effect_boundary: "local_task_store".into(),
        },
        ProposalType::DataExport => match proposal_tool(proposal) {
            Some("email.propose_draft") => GovernedActionReviewContract {
                capability_id: "email.draft".into(),
                operation: "open_email_draft".into(),
                confirmation_summary: "Confirm the exact recipient, subject, and draft body."
                    .into(),
                terminal_evidence_summary:
                    "Success proves only that the operating system accepted the draft handoff; it never proves send or delivery."
                        .into(),
                effect_boundary: "os_mail_handoff_unverified".into(),
            },
            Some("browser.open") => GovernedActionReviewContract {
                capability_id: "browser.open".into(),
                operation: "open_browser_url".into(),
                confirmation_summary: "Confirm one exact HTTP(S) address.".into(),
                terminal_evidence_summary:
                    "Success proves only that the operating system accepted the browser handoff; page load remains unverified."
                        .into(),
                effect_boundary: "os_browser_handoff_unverified".into(),
            },
            Some("local.run_utility") => GovernedActionReviewContract {
                capability_id: "local.utility.read_only".into(),
                operation: "run_local_utility".into(),
                confirmation_summary: "Confirm one exact allowlisted command.".into(),
                terminal_evidence_summary:
                    "Completion requires a bounded exit result; timeout or interrupted execution is not success."
                        .into(),
                effect_boundary: "bounded_local_process".into(),
            },
            _ => GovernedActionReviewContract {
                capability_id: "data.export".into(),
                operation: "export_data".into(),
                confirmation_summary: "Confirm the exact export target and content scope.".into(),
                terminal_evidence_summary:
                    "Completion requires refreshed export materialization evidence.".into(),
                effect_boundary: "local_export".into(),
            },
        },
        _ => return None,
    };
    Some(contract)
}

fn build_permission_decision_context(
    proposal: &AgentProposal,
    evidence_refs: &[EvidenceRef],
) -> PermissionDecisionContext {
    let after = &proposal.after;
    let canonical_scope = object_alias(after, &["canonical_scope", "canonicalScope"]);
    let blocked_action = object_alias(after, &["blocked_action", "blockedAction"]);
    let scope_kind =
        match string_alias(after, &["permission_scope_kind", "permissionScopeKind"]).as_deref() {
            Some("action_bound") => PermissionScopeKind::ActionBound,
            Some("network_policy") => PermissionScopeKind::NetworkPolicy,
            _ => PermissionScopeKind::Unknown,
        };
    let policy = match string_alias(after, &["policy", "permission"]).as_deref() {
        Some("allow_once") => PermissionPolicyKind::AllowOnce,
        _ => PermissionPolicyKind::Unknown,
    };
    let mut missing_fields = Vec::new();
    if scope_kind == PermissionScopeKind::Unknown {
        missing_fields.push("scopeKind".into());
    }
    if policy == PermissionPolicyKind::Unknown {
        missing_fields.push("policy".into());
    }

    let mut context = PermissionDecisionContext {
        status: PermissionDecisionContextStatus::Incomplete,
        scope_kind,
        policy,
        tool_label: "Unknown tool".into(),
        tool_name: None,
        capability_labels: string_array_alias(canonical_scope, &["capabilities"])
            .or_else(|| string_array_alias(after.as_object(), &["capabilities"]))
            .unwrap_or_default(),
        requested_target_label: string_alias_in(blocked_action, &["target"]),
        resolved_target_label: string_alias_in(
            blocked_action,
            &["resolved_target", "resolvedTarget"],
        ),
        purpose_summary: bounded_text(&proposal.reason, "Permission is requested for one action."),
        scope_digest: scope_digest(after),
        request_digest_kind: PermissionRequestDigestKind::Unknown,
        request_digest: None,
        request_length_bytes: None,
        blocked_run_id: string_alias_in(
            canonical_scope,
            &["blocked_run_id", "blockedRunId"],
        )
        .or_else(|| string_alias_in(blocked_action, &["source_run_id", "sourceRunId"])),
        blocked_step_index: u64_alias_in(
            canonical_scope,
            &["blocked_step_index", "blockedStepIndex"],
        )
        .or_else(|| u64_alias_in(blocked_action, &["step_index", "stepIndex"])),
        network_policy_decision_id: string_alias_in(
            canonical_scope,
            &["network_policy_decision_id", "networkPolicyDecisionId"],
        )
        .or_else(|| {
            string_alias_in(
                blocked_action,
                &["network_policy_decision_id", "networkPolicyDecisionId"],
            )
        }),
        transmission_boundary: PermissionTransmissionBoundary {
            external_transmission: ExternalTransmissionStatus::Unknown,
            summary: "The requested action's transmission boundary is unknown.".into(),
            target_label: None,
            evidence_refs: evidence_refs.to_vec(),
        },
        expires_at: proposal.expires_at,
        revocation_summary: "No valid one-time permission is created until approval. A granted permission is consumed only by one exact matching action or expires."
            .into(),
        missing_fields,
        evidence_refs: evidence_refs.to_vec(),
    };

    match scope_kind {
        PermissionScopeKind::ActionBound => populate_action_bound_context(after, &mut context),
        PermissionScopeKind::NetworkPolicy => {
            populate_network_policy_context(after, canonical_scope, blocked_action, &mut context)
        }
        PermissionScopeKind::Unknown => {}
    }
    if context.transmission_boundary.external_transmission == ExternalTransmissionStatus::Unknown {
        context.missing_fields.push("transmissionBoundary".into());
    }
    require_some(
        &context.expires_at,
        "expiresAt",
        &mut context.missing_fields,
    );
    require_some(
        &context.scope_digest,
        "scopeDigest",
        &mut context.missing_fields,
    );

    context.missing_fields.sort();
    context.missing_fields.dedup();
    if context.missing_fields.is_empty() {
        context.status = PermissionDecisionContextStatus::Ready;
    }
    context
}

fn populate_action_bound_context(after: &Value, context: &mut PermissionDecisionContext) {
    match ActionBoundToolPermissionScope::from_proposal_after(after) {
        Ok(scope) => {
            context.tool_label = readable_identifier(&scope.tool_name);
            context.tool_name = Some(scope.tool_name.clone());
            context.requested_target_label = Some(bounded_text(&scope.requested_target, "unknown"));
            context.resolved_target_label = Some(bounded_text(&scope.resolved_target, "unknown"));
            context.request_digest_kind = PermissionRequestDigestKind::Input;
            context.request_digest = Some(scope.input_hash.clone());
            context.request_length_bytes = Some(scope.input_length_bytes);
            if context.capability_labels.is_empty() {
                context
                    .capability_labels
                    .push(readable_identifier(&scope.manifest_action_type));
            }
            context.transmission_boundary = action_bound_transmission_boundary(
                &scope.source,
                &context.capability_labels,
                context.resolved_target_label.clone(),
                &context.evidence_refs,
            );
        }
        Err(_) => context.missing_fields.push("canonicalActionScope".into()),
    }
    require_some(
        &context.blocked_run_id,
        "blockedRunId",
        &mut context.missing_fields,
    );
    require_some(
        &context.blocked_step_index,
        "blockedStepIndex",
        &mut context.missing_fields,
    );
    require_some(
        &context.request_digest,
        "requestDigest",
        &mut context.missing_fields,
    );
    require_some(
        &context.request_length_bytes,
        "requestLengthBytes",
        &mut context.missing_fields,
    );
}

fn populate_network_policy_context(
    after: &Value,
    canonical_scope: Option<&Map<String, Value>>,
    blocked_action: Option<&Map<String, Value>>,
    context: &mut PermissionDecisionContext,
) {
    context.tool_name = string_alias(after, &["tool_name", "toolName", "name"])
        .or_else(|| string_alias_in(canonical_scope, &["tool_name", "toolName", "name"]));
    context.tool_label = context
        .tool_name
        .as_deref()
        .map(readable_identifier)
        .unwrap_or_else(|| "Unknown network action".into());

    let host = string_alias_in(canonical_scope, &["host"]);
    let endpoint_digest = string_alias_in(canonical_scope, &["endpoint_digest", "endpointDigest"])
        .or_else(|| string_alias_in(blocked_action, &["endpoint_digest", "endpointDigest"]));
    let input_digest = string_alias_in(canonical_scope, &["input_digest", "inputDigest"]);
    let endpoint_length = u64_alias_in(
        canonical_scope,
        &["endpoint_length_bytes", "endpointLengthBytes"],
    )
    .or_else(|| {
        u64_alias_in(
            blocked_action,
            &["endpoint_length_bytes", "endpointLengthBytes"],
        )
    });
    let input_length = u64_alias_in(canonical_scope, &["input_length_bytes", "inputLengthBytes"]);

    if let Some(digest) = endpoint_digest {
        context.request_digest_kind = PermissionRequestDigestKind::Endpoint;
        context.request_digest = Some(digest);
        context.request_length_bytes = endpoint_length;
    } else if let Some(digest) = input_digest {
        context.request_digest_kind = PermissionRequestDigestKind::Input;
        context.request_digest = Some(digest);
        context.request_length_bytes = input_length;
    }

    if context.capability_labels.is_empty() {
        if let Some(capability) = string_alias_in(
            canonical_scope,
            &["network_capability", "networkCapability"],
        ) {
            context.capability_labels.push(capability);
        }
    }
    let target = host
        .or_else(|| context.requested_target_label.clone())
        .or_else(|| context.resolved_target_label.clone());
    context.transmission_boundary = PermissionTransmissionBoundary {
        external_transmission: ExternalTransmissionStatus::Possible,
        summary: "Approval permits one exact external network request; it does not prove that transmission occurred."
            .into(),
        target_label: target.clone(),
        evidence_refs: context.evidence_refs.clone(),
    };
    if context.requested_target_label.is_none() {
        context.requested_target_label.clone_from(&target);
    }
    if context.resolved_target_label.is_none() {
        context.resolved_target_label.clone_from(&target);
    }

    require_some(&context.tool_name, "toolName", &mut context.missing_fields);
    if context.capability_labels.is_empty() {
        context.missing_fields.push("capabilityLabels".into());
    }
    require_some(
        &context.requested_target_label,
        "requestedTarget",
        &mut context.missing_fields,
    );
    require_some(
        &context.network_policy_decision_id,
        "networkPolicyDecisionId",
        &mut context.missing_fields,
    );
    require_some(
        &context.request_digest,
        "requestDigest",
        &mut context.missing_fields,
    );
    require_some(
        &context.request_length_bytes,
        "requestLengthBytes",
        &mut context.missing_fields,
    );

    // Main Chat network consent is action-bound to a run and step. Explicit
    // provider connection tests use an endpoint-bound command scope instead.
    if context.blocked_run_id.is_some() || context.blocked_step_index.is_some() {
        require_some(
            &context.blocked_run_id,
            "blockedRunId",
            &mut context.missing_fields,
        );
        require_some(
            &context.blocked_step_index,
            "blockedStepIndex",
            &mut context.missing_fields,
        );
    }
}

fn action_bound_transmission_boundary(
    source: &str,
    capabilities: &[String],
    target_label: Option<String>,
    evidence_refs: &[EvidenceRef],
) -> PermissionTransmissionBoundary {
    let external = source.starts_with("mcp:")
        || source.starts_with("a2a:")
        || source.starts_with("plugin:")
        || capabilities.iter().any(|capability| {
            matches!(
                capability.as_str(),
                "network" | "external_side_effect" | "external_transmission"
            )
        });
    if external {
        PermissionTransmissionBoundary {
            external_transmission: ExternalTransmissionStatus::Possible,
            summary: "The exact tool scope can cross the local process boundary; approval does not prove transmission occurred."
                .into(),
            target_label,
            evidence_refs: evidence_refs.to_vec(),
        }
    } else if source == "builtin" {
        PermissionTransmissionBoundary {
            external_transmission: ExternalTransmissionStatus::NotSent,
            summary: "The canonical built-in tool scope has no external-network capability. This describes the requested tool action only."
                .into(),
            target_label,
            evidence_refs: evidence_refs.to_vec(),
        }
    } else {
        PermissionTransmissionBoundary {
            external_transmission: ExternalTransmissionStatus::Unknown,
            summary: "The requested action's transmission boundary is unknown.".into(),
            target_label,
            evidence_refs: evidence_refs.to_vec(),
        }
    }
}

fn proposal_tool(proposal: &AgentProposal) -> Option<&str> {
    proposal.after.get("tool").and_then(Value::as_str)
}

fn proposal_operation(proposal: &AgentProposal) -> Option<&str> {
    proposal.after.get("operation").and_then(Value::as_str)
}

fn proposal_title(proposal: &AgentProposal) -> String {
    if reviewed_lifemodel_learning_context(proposal).is_some() {
        return "Review a learned long-term fact".into();
    }
    if reviewed_legacy_lifemodel_migration(proposal).is_some() {
        return "Review legacy LifeModel migration".into();
    }
    if reviewed_lifemodel_v2_diff(proposal).is_some() {
        return "Review LifeModel changes".into();
    }
    let title = match proposal.proposal_type {
        ProposalType::GoalUpdate => "Update a goal",
        ProposalType::StateUpdate => "Update personal state",
        ProposalType::PreferenceUpdate => "Update a preference",
        ProposalType::CapabilityUpdate => "Update a capability",
        ProposalType::MemoryWrite => "Add a memory",
        ProposalType::MemoryArchive
            if proposal
                .after
                .get("recallDisposition")
                .and_then(Value::as_str)
                == Some("paused") =>
        {
            "Stop recalling a memory"
        }
        ProposalType::MemoryArchive => "Archive a memory",
        ProposalType::ToolPermission => "Allow one action",
        ProposalType::PluginPermission => "Review plugin access",
        ProposalType::ScheduledTask
            if proposal_tool(proposal) == Some("calendar.propose_event") =>
        {
            "Create a local task and calendar projection"
        }
        ProposalType::ScheduledTask => "Schedule a task",
        ProposalType::ExternalWriteAction => match proposal_operation(proposal) {
            Some("move") => "Move a file",
            Some("trash") => "Move a file to OpenLife recovery",
            Some("restore") => "Restore a file",
            Some("overwrite") => "Overwrite a file",
            _ => "Create a file",
        },
        ProposalType::ModelPolicyChange => "Change model policy",
        ProposalType::DataExport => match proposal_tool(proposal) {
            Some("email.propose_draft") => "Open an email draft",
            Some("browser.open") => "Open a reviewed web address",
            Some("local.run_utility") => "Run a reviewed local utility",
            _ => "Export data",
        },
        ProposalType::ScheduleCheckin => "Schedule a check-in",
        ProposalType::LifeModelUpdate => "Update LifeModel",
        ProposalType::Unsupported => "Unsupported change",
    };
    title.into()
}

fn proposal_summary(proposal: &AgentProposal) -> String {
    if reviewed_lifemodel_learning_context(proposal).is_some() {
        return "Review one user-confirmed candidate and its exact sources before it becomes part of LifeModel v2."
            .into();
    }
    if reviewed_legacy_lifemodel_migration(proposal).is_some() {
        return "Review every selected legacy field before one backed-up, atomic switch to the canonical LifeModel v2 owner."
            .into();
    }
    if reviewed_lifemodel_v2_diff(proposal).is_some() {
        return "Review the exact version-bound LifeModel changes before a new canonical version is materialized."
            .into();
    }
    match proposal.proposal_type {
        ProposalType::MemoryArchive
            if proposal.after.get("recallDisposition").and_then(Value::as_str)
                == Some("paused") =>
        {
            "Review whether this memory should stop participating in recall while remaining available to restore."
                .into()
        }
        ProposalType::MemoryArchive => {
            "Review whether this memory should move to the archive and leave active recall."
                .into()
        }
        ProposalType::ToolPermission => {
            "Review the exact permission scope before allowing one matching action.".into()
        }
        ProposalType::ExternalWriteAction => match proposal_operation(proposal) {
            Some("move" | "trash" | "restore") => {
                "Review the exact source, destination, and source digest before one filesystem change."
                    .into()
            }
            _ => "Review the exact path and content digest before one file write.".into(),
        },
        ProposalType::ScheduledTask if proposal_tool(proposal) == Some("calendar.propose_event") => {
            "Review the exact title and time before creating a local task and optional ICS projection."
                .into()
        }
        ProposalType::DataExport if proposal_tool(proposal) == Some("email.propose_draft") => {
            "Review the recipient, subject, and body before opening a draft; this action never sends email."
                .into()
        }
        ProposalType::DataExport if proposal_tool(proposal) == Some("browser.open") => {
            "Review the exact HTTP(S) address before handing it to the system browser.".into()
        }
        ProposalType::DataExport if proposal_tool(proposal) == Some("local.run_utility") => {
            "Review one exact allowlisted read-only command before local execution.".into()
        }
        _ => format!(
            "Proposed change to {}.",
            readable_identifier(&proposal.affected_path)
        ),
    }
}

fn proposal_source_summary(source: ProposalSource) -> &'static str {
    match source {
        ProposalSource::BuilderReview => "LifeModel Builder review",
        ProposalSource::CalibrationRun => "Calibration run",
        ProposalSource::FeedbackEvolution => "Feedback learning",
        ProposalSource::MemoryGovernance => "Memory governance",
        ProposalSource::SkillRuntime => "Selected skill runtime",
        ProposalSource::Plugin => "Plugin request",
        ProposalSource::NetworkConsent => "Explicit network consent",
        ProposalSource::Manual => "User or runtime initiated review",
        ProposalSource::ChatConversation => "Current conversation",
        ProposalSource::ProactiveAgent => "Proactive suggestion",
        ProposalSource::PlanningSession => "Planning session",
    }
}

fn impact_summary(proposal: &AgentProposal) -> &'static str {
    match proposal.proposal_type {
        ProposalType::ToolPermission => {
            "Approval creates only a scoped permission decision. Execution and its result require refreshed backend evidence."
        }
        ProposalType::PluginPermission => {
            "No plugin access is granted until a supported backend approval path confirms it."
        }
        ProposalType::ExternalWriteAction => {
            "Approval is not proof that the external effect completed; materialization evidence remains separate."
        }
        ProposalType::ScheduledTask if proposal_tool(proposal) == Some("calendar.propose_event") => {
            "Approval creates a local scheduled task and may create an ICS projection. It does not prove a system or remote calendar event exists."
        }
        ProposalType::DataExport if proposal_tool(proposal) == Some("email.propose_draft") => {
            "Approval asks the operating system to open a draft. OpenLife does not send the message, and a successful handoff does not prove delivery."
        }
        ProposalType::DataExport if proposal_tool(proposal) == Some("browser.open") => {
            "Approval asks the operating system to open this address. It does not prove the page loaded or that any remote action succeeded."
        }
        ProposalType::DataExport if proposal_tool(proposal) == Some("local.run_utility") => {
            "Approval runs one exact allowlisted read-only utility with an empty environment and a bounded timeout."
        }
        ProposalType::DataExport => {
            "Approval is not proof that the export completed; refreshed backend evidence remains separate."
        }
        _ => {
            "Approval is a decision only. The change is complete only after refreshed materialization evidence reports applied."
        }
    }
}

fn affected_object_label(proposal: &AgentProposal) -> String {
    if reviewed_legacy_lifemodel_migration(proposal).is_some() {
        return "Legacy YAML to LifeModel v2 owner cutover".into();
    }
    if reviewed_lifemodel_v2_diff(proposal).is_some() {
        return "LifeModel v2 canonical version".into();
    }
    let prefix = match proposal.proposal_type {
        ProposalType::MemoryWrite | ProposalType::MemoryArchive => "Memory",
        ProposalType::ToolPermission | ProposalType::PluginPermission => "Permission",
        ProposalType::ExternalWriteAction => "External target",
        ProposalType::DataExport => "Export target",
        ProposalType::ScheduledTask | ProposalType::ScheduleCheckin => "Schedule",
        ProposalType::ModelPolicyChange => "Model policy",
        _ => "LifeModel",
    };
    format!("{prefix}: {}", readable_identifier(&proposal.affected_path))
}

fn readable_value(value: &Value) -> ReviewReadableValue {
    match value {
        Value::String(text) => ReviewReadableValue {
            kind: ReviewReadableValueKind::Text,
            summary: bounded_text(text, "Empty text"),
            detail: None,
            sensitivity: EvidenceSensitivity::LocalPrivate,
            truncated: text.chars().count() > MAX_READABLE_TEXT_CHARS,
        },
        Value::Number(number) => ReviewReadableValue {
            kind: ReviewReadableValueKind::Number,
            summary: number.to_string(),
            detail: None,
            sensitivity: EvidenceSensitivity::LocalPrivate,
            truncated: false,
        },
        Value::Bool(value) => ReviewReadableValue {
            kind: ReviewReadableValueKind::Boolean,
            summary: value.to_string(),
            detail: None,
            sensitivity: EvidenceSensitivity::LocalPrivate,
            truncated: false,
        },
        Value::Array(items) => collection_readable_value(
            ReviewReadableValueKind::List,
            format!("{} item(s)", items.len()),
            value,
        ),
        Value::Object(fields) => collection_readable_value(
            ReviewReadableValueKind::Object,
            format!("{} field(s)", fields.len()),
            value,
        ),
        Value::Null => ReviewReadableValue {
            kind: ReviewReadableValueKind::Unknown,
            summary: "No value".into(),
            detail: None,
            sensitivity: EvidenceSensitivity::LocalPrivate,
            truncated: false,
        },
    }
}

fn collection_readable_value(
    kind: ReviewReadableValueKind,
    summary: String,
    value: &Value,
) -> ReviewReadableValue {
    let sanitized = sanitize_value(value, 0, None);
    let rendered = serde_json::to_string_pretty(&sanitized).ok();
    let truncated = rendered
        .as_ref()
        .is_some_and(|detail| detail.len() > MAX_READABLE_DETAIL_BYTES);
    ReviewReadableValue {
        kind,
        summary,
        detail: rendered.filter(|detail| detail.len() <= MAX_READABLE_DETAIL_BYTES),
        sensitivity: EvidenceSensitivity::LocalPrivate,
        truncated,
    }
}

fn sanitize_value(value: &Value, depth: usize, key_hint: Option<&str>) -> Value {
    if key_hint.is_some_and(is_sensitive_key) {
        return Value::String("[REDACTED]".into());
    }
    if depth >= MAX_READABLE_DEPTH {
        return Value::String("[NESTED VALUE OMITTED]".into());
    }
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        sanitize_value(value, depth + 1, Some(key.as_str())),
                    )
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .take(20)
                .map(|value| sanitize_value(value, depth + 1, None))
                .collect(),
        ),
        Value::String(text) => Value::String(bounded_text(text, "")),
        _ => value.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace(['-', '_'], "");
    [
        "apikey",
        "authorization",
        "credential",
        "password",
        "secret",
        "token",
    ]
    .iter()
    .any(|candidate| normalized.contains(candidate))
}

fn bounded_text(value: &str, empty_fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return empty_fallback.into();
    }
    let mut chars = trimmed.chars();
    let bounded = chars
        .by_ref()
        .take(MAX_READABLE_TEXT_CHARS)
        .collect::<String>();
    if chars.next().is_some() {
        format!("{bounded}...")
    } else {
        bounded
    }
}

fn readable_identifier(value: &str) -> String {
    value
        .trim()
        .replace(['_', '.'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn scope_digest(after: &Value) -> Option<String> {
    let scope_kind = after
        .get("permission_scope_kind")
        .or_else(|| after.get("permissionScopeKind"));
    let canonical_scope = after
        .get("canonical_scope")
        .or_else(|| after.get("canonicalScope"));
    let blocked_action = after
        .get("blocked_action")
        .or_else(|| after.get("blockedAction"));
    if scope_kind.is_none() || canonical_scope.is_none() || blocked_action.is_none() {
        return None;
    }
    Some(
        crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
            "scopeKind": scope_kind,
            "canonicalScope": canonical_scope,
            "blockedAction": blocked_action,
            "policy": after.get("policy").or_else(|| after.get("permission")),
        }))
        .1,
    )
}

fn object_alias<'a>(value: &'a Value, aliases: &[&str]) -> Option<&'a Map<String, Value>> {
    aliases
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_object))
}

fn string_alias(value: &Value, aliases: &[&str]) -> Option<String> {
    aliases.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn string_alias_in(value: Option<&Map<String, Value>>, aliases: &[&str]) -> Option<String> {
    value.and_then(|value| {
        aliases.iter().find_map(|key| {
            value
                .get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
    })
}

fn u64_alias_in(value: Option<&Map<String, Value>>, aliases: &[&str]) -> Option<u64> {
    value.and_then(|value| {
        aliases
            .iter()
            .find_map(|key| value.get(*key).and_then(Value::as_u64))
    })
}

fn string_array_alias(value: Option<&Map<String, Value>>, aliases: &[&str]) -> Option<Vec<String>> {
    value.and_then(|value| {
        aliases.iter().find_map(|key| {
            value.get(*key).and_then(Value::as_array).map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(readable_identifier)
                    .collect::<Vec<_>>()
            })
        })
    })
}

fn require_some<T>(value: &Option<T>, field: &str, missing_fields: &mut Vec<String>) {
    if value.is_none() {
        missing_fields.push(field.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{ProposalSource, RiskLevel};
    use serde_json::json;

    fn proposal(proposal_type: ProposalType, after: Value) -> AgentProposal {
        let mut proposal = AgentProposal::new(
            proposal_type,
            "preferences.focus_mode",
            after,
            "Keep focus sessions bounded.",
            0.9,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        proposal.id = "review-item-1".into();
        proposal.run_id = Some("run-1".into());
        proposal
    }

    #[test]
    fn readable_review_value_redacts_secret_fields() {
        let proposal = proposal(
            ProposalType::PreferenceUpdate,
            json!({"mode": "focused", "apiKey": "must-not-leak"}),
        );
        let context = build_review_decision_context(&proposal, &[]);
        let detail = context.after.detail.expect("object detail");

        assert!(detail.contains("[REDACTED]"));
        assert!(!detail.contains("must-not-leak"));
    }

    #[test]
    fn lifemodel_v2_typed_diff_has_version_bound_human_review_language() {
        use crate::life_model::v2::{
            LifeModelDocumentV2, LifeModelItemV2, LifeModelSectionV2, LifeModelStatementV2,
            LifeModelTypedDiffV2, LifeModelTypedOperationV2, LIFE_MODEL_V2_TYPED_DIFF_SCHEMA,
        };
        let item = LifeModelStatementV2 {
            id: "value:autonomy".into(),
            statement: "Autonomy matters.".into(),
            source_refs: vec!["message:user:1".into()],
            confirmed_at: "2026-08-08T10:00:00Z".into(),
        };
        let mut result = LifeModelDocumentV2::empty("primary");
        result.values.push(item.clone());
        let diff = LifeModelTypedDiffV2 {
            schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
            model_id: "primary".into(),
            base_version: None,
            base_document_digest: None,
            operations: vec![LifeModelTypedOperationV2::Add {
                section: LifeModelSectionV2::Values,
                item: LifeModelItemV2::Statement(item),
            }],
            result_document_digest: result.digest().unwrap(),
        };
        let mut proposal = proposal(
            ProposalType::LifeModelUpdate,
            serde_json::to_value(diff).unwrap(),
        );
        proposal.affected_path = LIFE_MODEL_V2_TYPED_DIFF_PATH.into();

        let context = build_review_decision_context(&proposal, &[]);

        assert_eq!(context.title, "Review LifeModel changes");
        assert_eq!(context.before.unwrap().summary, "Empty LifeModel v2");
        assert!(context.after.summary.contains("1 add"));
        assert!(context.after.detail.unwrap().contains("Autonomy matters."));
        assert_eq!(
            context.affected_object_labels,
            vec!["LifeModel v2 canonical version"]
        );
    }

    #[test]
    fn legacy_migration_review_explains_atomic_owner_switch_without_raw_yaml() {
        use crate::life_model::v2::{
            LegacyLifeModelMigrationDecisionV2, LegacyLifeModelMigrationPreviewV2,
            LegacyLifeModelMigrationSelectionV2, DEFAULT_LIFE_MODEL_V2_MODEL_ID,
        };
        let raw = "metadata:\n  version: '0.1'\nidentity:\n  name: Alice\n";
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(raw).unwrap();
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
        let mut migration = proposal(
            ProposalType::LifeModelUpdate,
            serde_json::to_value(plan).unwrap(),
        );
        migration.affected_path = LIFE_MODEL_V2_LEGACY_MIGRATION_PATH.into();

        let context = build_review_decision_context(&migration, &[]);
        assert_eq!(context.title, "Review legacy LifeModel migration");
        assert!(context.summary.contains("atomic switch"));
        let before = context.before.unwrap();
        assert!(before.summary.contains("legacy YAML snapshot"));
        assert!(before.detail.unwrap().contains(&preview.source_digest));
        assert!(context.after.summary.contains("LifeModel change"));
        assert!(!context.reason_summary.contains(raw));
    }

    #[test]
    fn memory_stop_recall_and_archive_have_distinct_review_language() {
        let paused = build_review_decision_context(
            &proposal(
                ProposalType::MemoryArchive,
                json!({"recallDisposition": "paused"}),
            ),
            &[],
        );
        let archived = build_review_decision_context(
            &proposal(
                ProposalType::MemoryArchive,
                json!({"recallDisposition": "archived"}),
            ),
            &[],
        );

        assert_eq!(paused.title, "Stop recalling a memory");
        assert!(paused.summary.contains("remaining available to restore"));
        assert_eq!(archived.title, "Archive a memory");
        assert!(archived.summary.contains("move to the archive"));
    }

    #[test]
    fn governed_actions_project_exact_capability_confirmation_and_evidence_boundaries() {
        for (proposal_type, after, capability, operation, evidence_phrase) in [
            (
                ProposalType::ExternalWriteAction,
                json!({
                    "operation": "move",
                    "source_path": "/safe/a.md",
                    "target_path": "/safe/b.md",
                    "source_digest": format!("sha256:{}", "0".repeat(64)),
                }),
                "filesystem.write",
                "move",
                "filesystem materialization receipt",
            ),
            (
                ProposalType::ScheduledTask,
                json!({
                    "tool": "calendar.propose_event",
                    "title": "Review",
                    "scheduled_at": "2026-08-12T09:00:00+08:00",
                }),
                "calendar.local_projection",
                "create_local_calendar_projection",
                "local scheduled task",
            ),
            (
                ProposalType::DataExport,
                json!({
                    "tool": "email.propose_draft",
                    "to": "alice@example.com",
                    "subject": "Review",
                    "body": "Ready",
                    "content": "Ready",
                }),
                "email.draft",
                "open_email_draft",
                "never proves send",
            ),
            (
                ProposalType::DataExport,
                json!({
                    "tool": "browser.open",
                    "url": "https://example.com",
                    "content": "Open URL",
                }),
                "browser.open",
                "open_browser_url",
                "page load remains unverified",
            ),
            (
                ProposalType::DataExport,
                json!({
                    "tool": "local.run_utility",
                    "command": "uptime",
                    "content": "Run uptime",
                }),
                "local.utility.read_only",
                "run_local_utility",
                "bounded exit result",
            ),
        ] {
            let context = build_review_decision_context(&proposal(proposal_type, after), &[]);
            let contract = context.action_contract.expect("action contract");
            assert_eq!(contract.capability_id, capability);
            assert_eq!(contract.operation, operation);
            assert!(!contract.confirmation_summary.trim().is_empty());
            assert!(
                contract.terminal_evidence_summary.contains(evidence_phrase),
                "{}",
                contract.terminal_evidence_summary
            );
        }
    }

    #[test]
    fn action_bound_permission_projects_exact_ready_scope() {
        let proposal = proposal(
            ProposalType::ToolPermission,
            json!({
                "permission_scope_kind": "action_bound",
                "permission": "allow_once",
                "policy": "allow_once",
                "tool_name": "file.read",
                "source": "builtin",
                "risk_level": "medium",
                "canonical_scope": {
                    "tool_name": "file.read",
                    "source": "builtin",
                    "risk_level": "medium",
                    "action_type": "read",
                    "capabilities": ["read"],
                    "blocked_run_id": "run-1",
                    "blocked_step_index": 2,
                    "input_hash": format!("sha256:{}", "0".repeat(64)),
                    "input_length_bytes": 42
                },
                "blocked_action": {
                    "action_type": "inspect_file",
                    "target": "notes/today.md",
                    "resolved_target": "file.read",
                    "source_run_id": "run-1",
                    "step_index": 2,
                    "input_hash": format!("sha256:{}", "0".repeat(64)),
                    "input_length_bytes": 42
                }
            }),
        );
        let context = build_review_decision_context(&proposal, &[])
            .permission
            .expect("permission context");

        assert_eq!(context.status, PermissionDecisionContextStatus::Ready);
        assert_eq!(context.scope_kind, PermissionScopeKind::ActionBound);
        assert_eq!(context.blocked_step_index, Some(2));
        assert_eq!(
            context.transmission_boundary.external_transmission,
            ExternalTransmissionStatus::NotSent
        );
        assert!(context.missing_fields.is_empty());
    }

    #[test]
    fn incomplete_permission_scope_fails_closed() {
        let proposal = proposal(
            ProposalType::ToolPermission,
            json!({
                "permission_scope_kind": "action_bound",
                "permission": "allow_once",
                "tool_name": "file.read"
            }),
        );
        let context = build_review_decision_context(&proposal, &[])
            .permission
            .expect("permission context");

        assert_eq!(context.status, PermissionDecisionContextStatus::Incomplete);
        assert!(context
            .missing_fields
            .iter()
            .any(|field| field == "canonicalActionScope"));
    }

    #[test]
    fn permission_without_expiry_cannot_be_ready() {
        let mut proposal = proposal(
            ProposalType::ToolPermission,
            json!({
                "permission_scope_kind": "action_bound",
                "permission": "allow_once",
                "policy": "allow_once",
                "tool_name": "file.read",
                "source": "builtin",
                "risk_level": "medium",
                "canonical_scope": {
                    "tool_name": "file.read",
                    "source": "builtin",
                    "risk_level": "medium",
                    "action_type": "read",
                    "capabilities": ["read"],
                    "blocked_run_id": "run-1",
                    "blocked_step_index": 2,
                    "input_hash": format!("sha256:{}", "0".repeat(64)),
                    "input_length_bytes": 42
                },
                "blocked_action": {
                    "action_type": "inspect_file",
                    "target": "notes/today.md",
                    "resolved_target": "file.read",
                    "source_run_id": "run-1",
                    "step_index": 2,
                    "input_hash": format!("sha256:{}", "0".repeat(64)),
                    "input_length_bytes": 42
                }
            }),
        );
        proposal.expires_at = None;
        let context = build_review_decision_context(&proposal, &[])
            .permission
            .expect("permission context");

        assert_eq!(context.status, PermissionDecisionContextStatus::Incomplete);
        assert!(context
            .missing_fields
            .iter()
            .any(|field| field == "expiresAt"));
    }

    #[test]
    fn network_permission_projects_possible_transmission_without_claiming_sent() {
        let proposal = proposal(
            ProposalType::ToolPermission,
            json!({
                "permission_scope_kind": "network_policy",
                "permission": "allow_once",
                "policy": "allow_once",
                "tool_name": "web.search@example.test",
                "canonical_scope": {
                    "tool_name": "web.search@example.test",
                    "capabilities": ["network", "external_side_effect"],
                    "network_policy_decision_id": "network-policy-1",
                    "network_capability": "web.search",
                    "host": "example.test",
                    "blocked_run_id": "run-1",
                    "blocked_step_index": 3,
                    "input_digest": format!("sha256:{}", "1".repeat(64)),
                    "input_length_bytes": 64
                },
                "blocked_action": {
                    "target": "web.search",
                    "source_run_id": "run-1",
                    "step_index": 3,
                    "network_policy_decision_id": "network-policy-1"
                }
            }),
        );
        let context = build_review_decision_context(&proposal, &[])
            .permission
            .expect("permission context");

        assert_eq!(context.status, PermissionDecisionContextStatus::Ready);
        assert_eq!(context.scope_kind, PermissionScopeKind::NetworkPolicy);
        assert_eq!(
            context.transmission_boundary.external_transmission,
            ExternalTransmissionStatus::Possible
        );
        assert_eq!(
            context.transmission_boundary.target_label.as_deref(),
            Some("example.test")
        );
    }
}
