use crate::agent::heuristic_store::{
    HeuristicConstraintSet, HeuristicDraft, HeuristicLifecycleStatus, HeuristicRecord,
    HeuristicStore, HeuristicValidationState,
};
use crate::agent::types::{AgentProposal, ProposalStatus, RiskLevel};
use crate::agent::EvidencePrivacyLevel;
use crate::life_model::{
    LifeModelCompatibilityAssetRef, LifeModelHSCompatibilityView,
    LifeModelMaterializedViewProvenance,
};
use anyhow::{anyhow, Result};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const DEFAULT_CHAT_LEGACY_PATH: &str = "legacy_stream";
const LOW_ENERGY_RULE_CANDIDATE_SCHEMA: &str = "w76.lowEnergyCollaborationRuleCandidate.v1";

#[derive(Clone)]
pub struct AcceptedGuidanceLifecycleInput {
    pub candidate_proposal: Option<AgentProposal>,
    pub target_status: HeuristicLifecycleStatus,
    pub default_chat_selected_adapter_path: String,
    pub ordinary_chat_entrypoint_attached: bool,
    pub runtime_executed: bool,
    pub model_called: bool,
    pub tool_called: bool,
}

impl Default for AcceptedGuidanceLifecycleInput {
    fn default() -> Self {
        Self {
            candidate_proposal: None,
            target_status: HeuristicLifecycleStatus::Trial,
            default_chat_selected_adapter_path: DEFAULT_CHAT_LEGACY_PATH.into(),
            ordinary_chat_entrypoint_attached: false,
            runtime_executed: false,
            model_called: false,
            tool_called: false,
        }
    }
}

impl AcceptedGuidanceLifecycleInput {
    pub fn for_candidate(candidate_proposal: AgentProposal) -> Self {
        Self {
            candidate_proposal: Some(candidate_proposal),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedGuidanceRollbackPath {
    pub heuristic_id: Option<String>,
    pub source_proposal_id: Option<String>,
    pub target_status: HeuristicLifecycleStatus,
    pub rollback_available: bool,
    pub deactivation_path: String,
}

impl Default for AcceptedGuidanceRollbackPath {
    fn default() -> Self {
        Self {
            heuristic_id: None,
            source_proposal_id: None,
            target_status: HeuristicLifecycleStatus::Archived,
            rollback_available: false,
            deactivation_path: "heuristic_store.update_lifecycle.archived".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedGuidanceLifecycleReport {
    pub report_kind: String,
    pub lifecycle_ready: bool,
    pub created_guidance: bool,
    pub reused_guidance: bool,
    pub heuristic_id: Option<String>,
    pub lifecycle_status: HeuristicLifecycleStatus,
    pub guidance_domain: Option<String>,
    pub trigger: Option<String>,
    pub guidance_digest: Option<String>,
    pub source_candidate_rule_id: Option<String>,
    pub priority: i32,
    pub privacy_constraints: Vec<String>,
    pub model_constraints: Vec<String>,
    pub tool_constraints: Vec<String>,
    pub route_policy_relaxed: bool,
    pub source_proposal_id: Option<String>,
    pub source_evidence_ids: Vec<String>,
    pub source_agent_run_ids: Vec<String>,
    pub rollback_path: AcceptedGuidanceRollbackPath,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub default_chat_unchanged: bool,
    pub ordinary_chat_entrypoint_attached: bool,
    pub blocking_reasons: Vec<String>,
    pub wrote_evidence_count: u32,
    pub wrote_proposal_count: u32,
    pub wrote_life_model_count: u32,
    pub wrote_memory_count: u32,
    pub wrote_heuristic_count: u32,
    pub wrote_chat_message_count: u32,
    pub wrote_agent_run_count: u32,
    pub wrote_mcp_audit_count: u32,
    pub wrote_external_count: u32,
    pub ran_runtime: bool,
    pub ran_model: bool,
    pub ran_tool: bool,
}

pub fn create_accepted_guidance_from_maturation_candidate(
    input: AcceptedGuidanceLifecycleInput,
    heuristic_store: &HeuristicStore,
) -> Result<AcceptedGuidanceLifecycleReport> {
    let mut report = evaluate_accepted_guidance_lifecycle(&input);
    if !report.lifecycle_ready {
        return Ok(report);
    }

    let candidate = input
        .candidate_proposal
        .as_ref()
        .expect("ready accepted guidance requires candidate proposal");
    let candidate_after = &candidate.after;
    let source_candidate_rule_id = report.source_candidate_rule_id.clone();
    let heuristic_id = accepted_guidance_id(
        candidate,
        source_candidate_rule_id.as_deref(),
        &report.source_evidence_ids,
        report.guidance_digest.as_deref(),
    );
    let constraints = HeuristicConstraintSet {
        privacy: report.privacy_constraints.clone(),
        model: report.model_constraints.clone(),
        tool: report.tool_constraints.clone(),
    };
    if let Some(existing) = heuristic_store.get_heuristic(&heuristic_id)? {
        report.heuristic_id = Some(existing.id.clone());
        if accepted_guidance_existing_record_matches(
            &existing,
            candidate,
            &report,
            &constraints,
            input.target_status,
        ) {
            report.reused_guidance = true;
            report.lifecycle_status = existing.status;
            report.rollback_path = AcceptedGuidanceRollbackPath {
                heuristic_id: Some(existing.id),
                source_proposal_id: Some(candidate.id.clone()),
                target_status: HeuristicLifecycleStatus::Archived,
                rollback_available: true,
                deactivation_path: "heuristic_store.update_lifecycle.archived".into(),
            };
            return Ok(report);
        }

        report.lifecycle_ready = false;
        report.created_guidance = false;
        report.reused_guidance = false;
        report.lifecycle_status = existing.status;
        push_unique(
            &mut report.blocking_reasons,
            "accepted_guidance_id_collision",
        );
        push_unique(
            &mut report.blocking_reasons,
            "accepted_guidance_lineage_mismatch",
        );
        return Ok(report);
    }

    let mut conditions = default_conditions(candidate_after);
    if let Some(source_candidate_rule_id) = source_candidate_rule_id.as_deref() {
        push_unique(
            &mut conditions,
            &format!("source.candidate_rule_id == {source_candidate_rule_id}"),
        );
    }
    let draft = HeuristicDraft::new(
        heuristic_domain(candidate_after).unwrap_or("planning"),
        heuristic_trigger(candidate_after).unwrap_or("accepted_maturation_guidance"),
        conditions,
        guidance_summary(candidate_after).unwrap_or_default(),
        report.priority,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_stable_id(heuristic_id.clone())
    .with_source_proposal(candidate.id.clone())
    .with_validation_state(HeuristicValidationState::Pending)
    .with_constraints(constraints);

    let mut draft = draft;
    for evidence_id in &report.source_evidence_ids {
        draft = draft.with_evidence_ref(evidence_id.clone());
    }

    let created = heuristic_store.create_heuristic(draft)?;
    let updated = heuristic_store.update_lifecycle(&created.id, input.target_status, None)?;

    report.created_guidance = true;
    report.reused_guidance = false;
    report.heuristic_id = Some(updated.id.clone());
    report.lifecycle_status = updated.status;
    report.rollback_path = AcceptedGuidanceRollbackPath {
        heuristic_id: Some(updated.id),
        source_proposal_id: Some(candidate.id.clone()),
        target_status: HeuristicLifecycleStatus::Archived,
        rollback_available: true,
        deactivation_path: "heuristic_store.update_lifecycle.archived".into(),
    };
    report.wrote_heuristic_count = 1;
    Ok(report)
}

pub fn deactivate_accepted_guidance(
    heuristic_store: &HeuristicStore,
    heuristic_id: &str,
) -> Result<AcceptedGuidanceRollbackPath> {
    let existing = heuristic_store
        .get_heuristic(heuristic_id)?
        .ok_or_else(|| anyhow!("heuristic record not found: {}", heuristic_id))?;
    if !existing.id.starts_with("accepted_guidance_")
        || !matches!(existing.source_proposal_id.as_deref(), Some(value) if !value.is_empty())
    {
        return Err(anyhow!(
            "accepted guidance deactivation requires a dedicated accepted guidance record"
        ));
    }
    let record =
        heuristic_store.update_lifecycle(heuristic_id, HeuristicLifecycleStatus::Archived, None)?;
    Ok(AcceptedGuidanceRollbackPath {
        heuristic_id: Some(record.id),
        source_proposal_id: record.source_proposal_id,
        target_status: HeuristicLifecycleStatus::Archived,
        rollback_available: false,
        deactivation_path: "heuristic_store.update_lifecycle.archived".into(),
    })
}

fn evaluate_accepted_guidance_lifecycle(
    input: &AcceptedGuidanceLifecycleInput,
) -> AcceptedGuidanceLifecycleReport {
    let mut blocking_reasons = Vec::new();
    let mut metadata_safe = true;
    let mut contains_raw_content = false;
    let mut guidance_domain = None;
    let mut trigger = None;
    let mut guidance_digest = None;
    let mut source_candidate_rule_id = None;
    let mut priority = 80;
    let mut privacy_constraints = default_privacy_constraints();
    let mut model_constraints = default_model_constraints();
    let mut tool_constraints = default_tool_constraints();
    let mut source_proposal_id = None;
    let mut source_evidence_ids = Vec::new();
    let mut source_agent_run_ids = Vec::new();
    let default_chat_unchanged =
        input.default_chat_selected_adapter_path == DEFAULT_CHAT_LEGACY_PATH;

    if !default_chat_unchanged {
        push_unique(
            &mut blocking_reasons,
            "default_chat_route_migration_assumed",
        );
    }
    if input.ordinary_chat_entrypoint_attached {
        push_unique(&mut blocking_reasons, "ordinary_chat_entrypoint_attached");
    }
    if input.runtime_executed {
        push_unique(&mut blocking_reasons, "runtime_execution_implied");
    }
    if input.model_called {
        push_unique(&mut blocking_reasons, "model_call_implied");
    }
    if input.tool_called {
        push_unique(&mut blocking_reasons, "tool_call_implied");
    }
    if input.target_status != HeuristicLifecycleStatus::Trial {
        push_unique(&mut blocking_reasons, "goal4_only_creates_trial_guidance");
    }

    match input.candidate_proposal.as_ref() {
        Some(candidate) => {
            source_proposal_id = Some(candidate.id.clone());
            if candidate.status != ProposalStatus::Accepted {
                push_unique(&mut blocking_reasons, "candidate_proposal_not_accepted");
            }
            if !matches!(candidate.risk_level, RiskLevel::Low) {
                push_unique(
                    &mut blocking_reasons,
                    "accepted_guidance_candidate_not_low_risk",
                );
            }
            if !is_supported_maturation_candidate(candidate) {
                push_unique(
                    &mut blocking_reasons,
                    "candidate_proposal_not_supported_maturation_guidance",
                );
            }
            if candidate_value_contains_raw_content(&candidate.after) {
                metadata_safe = false;
                contains_raw_content = true;
                push_unique(
                    &mut blocking_reasons,
                    "candidate_proposal_contains_raw_content",
                );
            }
            if !candidate_metadata_safe(&candidate.after) {
                metadata_safe = false;
                push_unique(
                    &mut blocking_reasons,
                    "candidate_proposal_metadata_not_safe",
                );
            }
            if candidate
                .after
                .get("activatesHeuristic")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || candidate
                    .after
                    .get("writesActiveRule")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                || candidate
                    .after
                    .get("heuristicActivationAllowed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                push_unique(
                    &mut blocking_reasons,
                    "unsafe_heuristic_activation_attempted",
                );
            }
            if guidance_summary(&candidate.after)
                .as_deref()
                .is_some_and(guidance_relaxes_policy)
                || candidate_attempts_route_override(&candidate.after)
            {
                push_unique(
                    &mut blocking_reasons,
                    "candidate_attempts_privacy_route_override",
                );
            }

            source_candidate_rule_id = candidate_rule_id(&candidate.after);
            guidance_domain = target_domain(&candidate.after).map(str::to_string);
            trigger = heuristic_trigger(&candidate.after).map(str::to_string);
            guidance_digest =
                guidance_summary(&candidate.after).map(|guidance| sha256_hex(guidance.as_bytes()));
            priority = candidate
                .after
                .get("priority")
                .and_then(Value::as_i64)
                .map(|value| value.clamp(0, 100) as i32)
                .unwrap_or(90);
            source_evidence_ids = candidate_source_evidence_ids(&candidate.after);
            source_agent_run_ids =
                candidate_source_lineage_array(&candidate.after, "linkedAgentRunIds");
            if source_evidence_ids.is_empty() {
                push_unique(&mut blocking_reasons, "source_evidence_lineage_missing");
            }
            if source_agent_run_ids.is_empty() {
                push_unique(&mut blocking_reasons, "source_agent_run_lineage_missing");
            }
            privacy_constraints = constraints_array(&candidate.after, "privacy")
                .unwrap_or_else(default_privacy_constraints);
            model_constraints = constraints_array(&candidate.after, "model")
                .unwrap_or_else(default_model_constraints);
            tool_constraints = constraints_array(&candidate.after, "tool")
                .unwrap_or_else(default_tool_constraints);
        }
        None => push_unique(&mut blocking_reasons, "candidate_proposal_missing"),
    }

    let route_policy_relaxed = model_constraints
        .iter()
        .any(|constraint| guidance_relaxes_policy(constraint));
    if route_policy_relaxed {
        push_unique(&mut blocking_reasons, "constraints_relax_route_policy");
    }

    let lifecycle_ready = blocking_reasons.is_empty() && metadata_safe && !contains_raw_content;
    AcceptedGuidanceLifecycleReport {
        report_kind: "w134.acceptedGuidanceLifecycle.v1".into(),
        lifecycle_ready,
        created_guidance: false,
        reused_guidance: false,
        heuristic_id: None,
        lifecycle_status: input.target_status,
        guidance_domain,
        trigger,
        guidance_digest,
        source_candidate_rule_id,
        priority,
        privacy_constraints,
        model_constraints,
        tool_constraints,
        route_policy_relaxed,
        source_proposal_id,
        source_evidence_ids,
        source_agent_run_ids,
        rollback_path: AcceptedGuidanceRollbackPath::default(),
        metadata_safe,
        contains_raw_content,
        default_chat_unchanged,
        ordinary_chat_entrypoint_attached: input.ordinary_chat_entrypoint_attached,
        blocking_reasons,
        wrote_evidence_count: 0,
        wrote_proposal_count: 0,
        wrote_life_model_count: 0,
        wrote_memory_count: 0,
        wrote_heuristic_count: 0,
        wrote_chat_message_count: 0,
        wrote_agent_run_count: 0,
        wrote_mcp_audit_count: 0,
        wrote_external_count: 0,
        ran_runtime: input.runtime_executed,
        ran_model: input.model_called,
        ran_tool: input.tool_called,
    }
}

fn accepted_guidance_id(
    candidate: &AgentProposal,
    source_candidate_rule_id: Option<&str>,
    source_evidence_ids: &[String],
    guidance_digest: Option<&str>,
) -> String {
    let source_evidence_ids = sorted_unique(source_evidence_ids.to_vec());
    let id_material = serde_json::json!({
        "schema": "w134.acceptedGuidanceId.v1",
        "proposalId": candidate.id,
        "candidateRuleId": source_candidate_rule_id.unwrap_or_default(),
        "sourceEvidenceIds": source_evidence_ids,
        "guidanceDigest": guidance_digest.unwrap_or_default(),
    });
    format!("accepted_guidance_{}", short_hash(&id_material.to_string()))
}

fn accepted_guidance_existing_record_matches(
    existing: &HeuristicRecord,
    candidate: &AgentProposal,
    report: &AcceptedGuidanceLifecycleReport,
    constraints: &HeuristicConstraintSet,
    target_status: HeuristicLifecycleStatus,
) -> bool {
    existing.id.starts_with("accepted_guidance_")
        && existing.status == target_status
        && existing.source_proposal_id.as_deref() == Some(candidate.id.as_str())
        && sorted_unique(existing.evidence_refs.clone())
            == sorted_unique(report.source_evidence_ids.clone())
        && &existing.constraints == constraints
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedGuidanceVersionRef {
    pub heuristic_id: String,
    pub lifecycle_status: HeuristicLifecycleStatus,
    pub source_proposal_id: Option<String>,
    pub source_evidence_ids: Vec<String>,
    pub guidance_digest: String,
    pub rollback_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelVersionAssetDiffRef {
    pub asset_kind: String,
    pub asset_id: String,
    pub affected_path: String,
    pub change_kind: String,
    pub before_digest: Option<String>,
    pub after_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelRollbackReadModelRef {
    pub target_version_id: String,
    pub requires_proposal: bool,
    pub source_proposal_ids: Vec<String>,
    pub source_evidence_ids: Vec<String>,
    pub source_patch_ids: Vec<String>,
    pub source_heuristic_ids: Vec<String>,
    pub materialized_view_source_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelVersionReadModel {
    pub report_kind: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub from_version_id: String,
    pub to_version_id: String,
    pub materialized_view_source_digest: String,
    pub materialized_view_provenance_digest: String,
    pub provenance: LifeModelMaterializedViewProvenance,
    pub accepted_guidance_refs: Vec<AcceptedGuidanceVersionRef>,
    pub changed_asset_refs: Vec<LifeModelVersionAssetDiffRef>,
    pub rollback_reference: Option<LifeModelRollbackReadModelRef>,
    pub diff_reference_digest: String,
    pub rollback_reference_digest: Option<String>,
    pub raw_content_included: bool,
}

pub fn build_lifemodel_version_read_model(
    from_version_id: impl Into<String>,
    to_version_id: impl Into<String>,
    from_view: LifeModelHSCompatibilityView,
    to_view: LifeModelHSCompatibilityView,
    accepted_guidance_records: Vec<HeuristicRecord>,
    rollback_target_version_id: Option<&str>,
) -> LifeModelVersionReadModel {
    let from_version_id = from_version_id.into();
    let to_version_id = to_version_id.into();
    let accepted_guidance_refs = accepted_guidance_records
        .iter()
        .map(accepted_guidance_version_ref)
        .collect::<Vec<_>>();
    let changed_asset_refs = diff_asset_refs(&from_view.asset_refs, &to_view.asset_refs);
    let rollback_reference =
        rollback_target_version_id.map(|target_version_id| LifeModelRollbackReadModelRef {
            target_version_id: target_version_id.to_string(),
            requires_proposal: true,
            source_proposal_ids: to_view.provenance.source_proposal_ids.clone(),
            source_evidence_ids: to_view.provenance.source_evidence_ids.clone(),
            source_patch_ids: to_view.provenance.source_patch_ids.clone(),
            source_heuristic_ids: to_view.provenance.source_heuristic_ids.clone(),
            materialized_view_source_digest: to_view.source_digest.clone(),
        });
    let diff_reference_digest = sha256_hex(
        serde_json::json!({
            "schema": "w136.lifeModelVersionDiffReadModel.digest.v1",
            "fromVersionId": from_version_id,
            "toVersionId": to_version_id,
            "fromSourceDigest": from_view.source_digest,
            "toSourceDigest": to_view.source_digest,
            "changedAssetRefs": changed_asset_refs,
            "acceptedGuidanceRefs": accepted_guidance_refs,
            "provenanceDigest": to_view.provenance.provenance_digest,
        })
        .to_string()
        .as_bytes(),
    );
    let rollback_reference_digest = rollback_reference.as_ref().map(|rollback| {
        sha256_hex(
            serde_json::to_string(rollback)
                .unwrap_or_default()
                .as_bytes(),
        )
    });

    LifeModelVersionReadModel {
        report_kind: "w136.lifeModelVersionReadModel.v1".into(),
        metadata_safe: true,
        contains_raw_content: false,
        from_version_id,
        to_version_id,
        materialized_view_source_digest: to_view.source_digest.clone(),
        materialized_view_provenance_digest: to_view.provenance.provenance_digest.clone(),
        provenance: to_view.provenance,
        accepted_guidance_refs,
        changed_asset_refs,
        rollback_reference,
        diff_reference_digest,
        rollback_reference_digest,
        raw_content_included: false,
    }
}

fn accepted_guidance_version_ref(record: &HeuristicRecord) -> AcceptedGuidanceVersionRef {
    AcceptedGuidanceVersionRef {
        heuristic_id: record.id.clone(),
        lifecycle_status: record.status,
        source_proposal_id: record.source_proposal_id.clone(),
        source_evidence_ids: sorted_unique(record.evidence_refs.clone()),
        guidance_digest: sha256_hex(
            serde_json::json!({
                "heuristicId": record.id,
                "domain": record.domain,
                "trigger": record.trigger,
                "status": record.status.to_string(),
                "version": record.version,
                "evidenceRefs": sorted_unique(record.evidence_refs.clone()),
            })
            .to_string()
            .as_bytes(),
        ),
        rollback_available: matches!(
            record.status,
            HeuristicLifecycleStatus::Trial
                | HeuristicLifecycleStatus::Active
                | HeuristicLifecycleStatus::Weakened
        ),
    }
}

fn diff_asset_refs(
    from_refs: &[LifeModelCompatibilityAssetRef],
    to_refs: &[LifeModelCompatibilityAssetRef],
) -> Vec<LifeModelVersionAssetDiffRef> {
    let from_map = asset_map(from_refs);
    let to_map = asset_map(to_refs);
    let keys = from_map
        .keys()
        .chain(to_map.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut diffs = Vec::new();
    for key in keys {
        match (from_map.get(&key), to_map.get(&key)) {
            (None, Some(after)) => diffs.push(asset_diff(None, Some(after), "added")),
            (Some(before), None) => diffs.push(asset_diff(Some(before), None, "removed")),
            (Some(before), Some(after)) if before.content_digest != after.content_digest => {
                diffs.push(asset_diff(Some(before), Some(after), "changed"));
            }
            _ => {}
        }
    }
    diffs
}

fn asset_map(
    refs: &[LifeModelCompatibilityAssetRef],
) -> BTreeMap<String, LifeModelCompatibilityAssetRef> {
    refs.iter()
        .map(|asset| {
            (
                format!("{}:{}", asset.asset_kind, asset.asset_id),
                asset.clone(),
            )
        })
        .collect()
}

fn asset_diff(
    before: Option<&LifeModelCompatibilityAssetRef>,
    after: Option<&LifeModelCompatibilityAssetRef>,
    change_kind: &str,
) -> LifeModelVersionAssetDiffRef {
    let reference = after
        .or(before)
        .expect("asset diff requires before or after");
    LifeModelVersionAssetDiffRef {
        asset_kind: reference.asset_kind.clone(),
        asset_id: reference.asset_id.clone(),
        affected_path: reference.affected_path.clone(),
        change_kind: change_kind.to_string(),
        before_digest: before.map(|asset| asset.content_digest.clone()),
        after_digest: after.map(|asset| asset.content_digest.clone()),
    }
}

fn is_supported_maturation_candidate(candidate: &AgentProposal) -> bool {
    candidate
        .source_detail
        .as_deref()
        .is_some_and(|detail| detail.starts_with("maturation:"))
        && candidate
            .after
            .get("schema")
            .and_then(Value::as_str)
            .is_some_and(|schema| schema == LOW_ENERGY_RULE_CANDIDATE_SCHEMA)
        && target_domain(&candidate.after).is_some()
        && guidance_summary(&candidate.after).is_some()
}

fn candidate_metadata_safe(value: &Value) -> bool {
    value
        .get("metadataSafe")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !value
            .get("containsRawContent")
            .and_then(Value::as_bool)
            .unwrap_or(true)
}

fn candidate_value_contains_raw_content(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, child)| {
            let lower = key.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "containsrawcontent"
                    | "rawpromptincluded"
                    | "assistantoutputincluded"
                    | "memoryrawtextincluded"
                    | "toolpayloadincluded"
                    | "secretincluded"
                    | "editedpayloadincluded"
            ) {
                return child.as_bool().unwrap_or(false);
            }
            lower.contains("raw")
                || lower.contains("secret")
                || (lower.contains("payload") && !lower.ends_with("included"))
                || candidate_value_contains_raw_content(child)
        }),
        Value::Array(items) => items.iter().any(candidate_value_contains_raw_content),
        Value::String(text) => string_looks_raw(text),
        _ => false,
    }
}

fn string_looks_raw(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("raw_prompt")
        || lower.contains("raw_assistant")
        || lower.contains("raw_memory")
        || lower.contains("raw_tool")
        || lower.contains("secret")
        || lower.contains("reviewer raw note")
}

fn guidance_relaxes_policy(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("use cloud")
        || lower.contains("ignore privacy")
        || lower.contains("relax privacy")
        || lower.contains("disable local")
}

fn candidate_attempts_route_override(value: &Value) -> bool {
    value
        .get("modelRoutePolicy")
        .or_else(|| value.get("routePolicy"))
        .and_then(Value::as_str)
        .is_some_and(|route| {
            let lower = route.to_ascii_lowercase();
            lower.contains("cloud") && lower != "local_only"
        })
}

fn target_domain(value: &Value) -> Option<&str> {
    value
        .get("targetDomain")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| {
            matches!(
                *value,
                "low_energy_planning"
                    | "planning_preference"
                    | "energy_pattern"
                    | "work_style"
                    | "communication_preference"
            )
        })
}

fn heuristic_domain(value: &Value) -> Option<&'static str> {
    match target_domain(value)? {
        "low_energy_planning" | "planning_preference" | "energy_pattern" => Some("planning"),
        "work_style" | "communication_preference" => Some("conversation"),
        _ => None,
    }
}

fn heuristic_trigger(value: &Value) -> Option<&'static str> {
    match target_domain(value)? {
        "low_energy_planning" => Some("current_energy_is_low"),
        "planning_preference" => Some("planning_preference_signal"),
        "energy_pattern" => Some("energy_pattern_signal"),
        "work_style" => Some("work_style_signal"),
        "communication_preference" => Some("communication_preference_signal"),
        _ => None,
    }
}

fn default_conditions(value: &Value) -> Vec<String> {
    match target_domain(value).unwrap_or_default() {
        "low_energy_planning" => vec!["state.energy <= 3".into()],
        domain => vec![format!("maturation.domain == {domain}")],
    }
}

fn guidance_summary(value: &Value) -> Option<String> {
    value
        .get("ruleSummary")
        .or_else(|| value.get("candidateSummary"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn candidate_rule_id(value: &Value) -> Option<String> {
    value
        .get("candidateRuleId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn candidate_source_evidence_ids(value: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    for key in [
        "acceptedOutcomeEvidenceIds",
        "editedOutcomeEvidenceIds",
        "sourceEvidenceIds",
    ] {
        for id in candidate_source_lineage_array(value, key) {
            push_unique(&mut ids, &id);
        }
    }
    ids
}

fn candidate_source_lineage_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get("sourceLineage")
        .and_then(|lineage| lineage.get(key))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn constraints_array(value: &Value, key: &str) -> Option<Vec<String>> {
    value
        .get("constraints")
        .and_then(|constraints| constraints.get(key))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
}

fn default_privacy_constraints() -> Vec<String> {
    vec!["do_not_relax_policy".into()]
}

fn default_model_constraints() -> Vec<String> {
    vec!["preserve_current_route_policy".into()]
}

fn default_tool_constraints() -> Vec<String> {
    vec!["write_tools_remain_proposal_first".into()]
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !value.trim().is_empty() && !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn short_hash(value: &str) -> String {
    sha256_hex(value.as_bytes()).chars().take(16).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let hash = digest(&SHA256, bytes);
    let bytes = hash.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}
