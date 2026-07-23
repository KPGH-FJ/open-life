use crate::agent::product_read_model::{
    EvidenceRef, EvidenceSensitivity, ExternalTransmissionStatus,
};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType};
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
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
}

pub fn build_review_decision_context(
    proposal: &AgentProposal,
    evidence_refs: &[EvidenceRef],
) -> ReviewDecisionContext {
    let permission = (proposal.proposal_type == ProposalType::ToolPermission)
        .then(|| build_permission_decision_context(proposal, evidence_refs));
    let after = if permission.is_some() {
        ReviewReadableValue {
            kind: ReviewReadableValueKind::Redacted,
            summary: "Exact permission scope is projected in the permission decision context."
                .into(),
            detail: None,
            sensitivity: EvidenceSensitivity::Redacted,
            truncated: false,
        }
    } else {
        readable_value(&proposal.after)
    };

    ReviewDecisionContext {
        review_item_id: proposal.id.clone(),
        title: proposal_title(proposal.proposal_type).into(),
        summary: proposal_summary(proposal),
        before: proposal.before.as_ref().map(readable_value),
        after,
        reason_summary: bounded_text(&proposal.reason, "No reason was supplied."),
        source_summary: proposal_source_summary(proposal.source).into(),
        impact_summary: impact_summary(proposal.proposal_type).into(),
        affected_object_labels: vec![affected_object_label(proposal)],
        expires_at: proposal.expires_at,
        permission,
        evidence_refs: evidence_refs.to_vec(),
    }
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

fn proposal_title(proposal_type: ProposalType) -> &'static str {
    match proposal_type {
        ProposalType::GoalUpdate => "Update a goal",
        ProposalType::StateUpdate => "Update personal state",
        ProposalType::PreferenceUpdate => "Update a preference",
        ProposalType::CapabilityUpdate => "Update a capability",
        ProposalType::MemoryWrite => "Add a memory",
        ProposalType::MemoryArchive => "Archive a memory",
        ProposalType::ToolPermission => "Allow one action",
        ProposalType::PluginPermission => "Review plugin access",
        ProposalType::ScheduledTask => "Schedule a task",
        ProposalType::ExternalWriteAction => "Review an external write",
        ProposalType::ModelPolicyChange => "Change model policy",
        ProposalType::DataExport => "Export data",
        ProposalType::ScheduleCheckin => "Schedule a check-in",
        ProposalType::LifeModelUpdate => "Update LifeModel",
        ProposalType::Unsupported => "Unsupported change",
    }
}

fn proposal_summary(proposal: &AgentProposal) -> String {
    match proposal.proposal_type {
        ProposalType::ToolPermission => {
            "Review the exact permission scope before allowing one matching action.".into()
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

fn impact_summary(proposal_type: ProposalType) -> &'static str {
    match proposal_type {
        ProposalType::ToolPermission => {
            "Approval creates only a scoped permission decision. Execution and its result require refreshed backend evidence."
        }
        ProposalType::PluginPermission => {
            "No plugin access is granted until a supported backend approval path confirms it."
        }
        ProposalType::ExternalWriteAction | ProposalType::DataExport => {
            "Approval is not proof that the external effect completed; materialization evidence remains separate."
        }
        _ => {
            "Approval is a decision only. The change is complete only after refreshed materialization evidence reports applied."
        }
    }
}

fn affected_object_label(proposal: &AgentProposal) -> String {
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
