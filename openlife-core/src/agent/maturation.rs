use crate::agent::evidence_graph::{
    evaluate_evidence_graph, EvidenceClusterSummary, EvidenceGraphInput, EvidenceGraphReport,
    EvidencePolarity, EvidenceTimelineItem,
};
use crate::agent::evidence_store::{
    EvidenceDraft, EvidencePrivacyLevel, EvidenceRecord, EvidenceSourceRef, EvidenceSourceType,
    EvidenceStore, EvidenceType,
};
use crate::agent::governor::{GovernanceDecision, GovernanceDecisionKind, LifeModelGovernor};
use crate::agent::hs_selector::RuntimeHSPacket;
use crate::agent::maturation_domain::{
    classify_supported_maturation_domain, high_risk_maturation_text, SupportedMaturationDomain,
};
use crate::agent::policy_store::{
    ModelRoutePolicy, PolicyTopic, BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
    BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY,
};
use crate::agent::proposal_store::ProposalStore;
use crate::agent::runtime_contract::{LifeEventDraft, RuntimeOutput};
use crate::agent::types::{
    AgentProposal, AgentTaskKind, ProposalSource, ProposalStatus, ProposalType, RiskLevel,
};
use anyhow::{anyhow, Result};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};

const DEFAULT_CONFIDENCE: f32 = 0.7;
const MIN_CONFIDENCE: f32 = 0.55;
const MIN_SUMMARY_CHARS: usize = 4;
const DEFAULT_CHAT_KERNEL_PATH: &str = "main_chat_kernel";
const MATURATION_NEXT_ALLOWED_STEP: &str = "non_default_maturation_invocation";
const LOW_ENERGY_RULE_CANDIDATE_SOURCE_DETAIL: &str =
    "maturation:low_energy_collaboration_rule_candidate";
const LOW_ENERGY_RULE_CANDIDATE_PATH: &str = "/collaboration/rule_candidates/low_energy_planning";
const LOW_ENERGY_RULE_CANDIDATE_SUMMARY: &str =
    "Prefer low-pressure planning suggestions with small next steps when the user signals low energy.";
const DEFAULT_LOW_ENERGY_RULE_CANDIDATE_MIN_SUPPORT: usize = 1;
const MATURATION_ENGINE_V1_REPORT_KIND: &str = "maturation_engine_v1";
const MIN_ENGINE_EFFECTIVE_CONFIDENCE: f32 = 0.45;
const MIN_ENGINE_STABILITY_SCORE: f32 = 0.45;

#[derive(Clone, PartialEq)]
pub struct LifeModelMaturationReadinessInput {
    pub candidate: Option<LifeEventDraft>,
    pub default_chat_selected_adapter_path: String,
    pub ordinary_chat_auto_maturation_enabled: bool,
    pub require_direct_life_model_write: bool,
    pub require_direct_memory_write: bool,
    pub require_heuristic_activation: bool,
}

impl Default for LifeModelMaturationReadinessInput {
    fn default() -> Self {
        Self {
            candidate: None,
            default_chat_selected_adapter_path: DEFAULT_CHAT_KERNEL_PATH.into(),
            ordinary_chat_auto_maturation_enabled: false,
            require_direct_life_model_write: false,
            require_direct_memory_write: false,
            require_heuristic_activation: false,
        }
    }
}

impl LifeModelMaturationReadinessInput {
    pub fn for_candidate(candidate: LifeEventDraft) -> Self {
        Self {
            candidate: Some(candidate),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelMaturationReadinessSideEffectBudget {
    pub runtime_calls: u32,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub store_writes: u32,
    pub chat_message_writes: u32,
    pub agent_run_writes: u32,
    pub evidence_writes: u32,
    pub proposal_writes: u32,
    pub life_model_writes: u32,
    pub memory_writes: u32,
    pub heuristic_writes: u32,
    pub mcp_audit_writes: u32,
    pub external_writes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelMaturationReadinessReport {
    pub readiness_ready: bool,
    pub ready: bool,
    pub default_chat_unchanged: bool,
    pub ordinary_chat_entrypoint_unchanged: bool,
    pub runtime_output_candidate_shape_present: bool,
    pub maturation_service_present: bool,
    pub evidence_store_present: bool,
    pub proposal_store_present: bool,
    pub governor_present: bool,
    pub proposal_first_required: bool,
    pub direct_life_model_write_allowed: bool,
    pub direct_memory_write_allowed: bool,
    pub direct_heuristic_write_allowed: bool,
    pub heuristic_activation_allowed: bool,
    pub low_energy_planning_domain_only: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub source_lineage_required: bool,
    pub source_lineage_present: bool,
    pub negative_evidence_required_for_rejection: bool,
    pub accepted_rule_runtime_packet_future_only: bool,
    pub business_write_disabled: bool,
    pub side_effect_budget_zero: bool,
    pub side_effect_budget: LifeModelMaturationReadinessSideEffectBudget,
    pub candidate_digest: Option<String>,
    pub candidate_confidence: Option<f32>,
    pub blocking_reasons: Vec<String>,
    pub next_allowed_step: String,
}

pub fn evaluate_lifemodel_maturation_readiness(
    input: LifeModelMaturationReadinessInput,
) -> LifeModelMaturationReadinessReport {
    let runtime_output_candidate_shape_present =
        type_available::<RuntimeOutput>() && type_available::<LifeEventDraft>();
    let maturation_service_present =
        type_available::<MaturationService>() && type_available::<LifeModelMaturationService>();
    let evidence_store_present = type_available::<EvidenceStore>();
    let proposal_store_present = type_available::<ProposalStore>();
    let governor_present = type_available::<LifeModelGovernor>();

    let default_chat_unchanged =
        input.default_chat_selected_adapter_path == DEFAULT_CHAT_KERNEL_PATH;
    let ordinary_chat_entrypoint_unchanged = !input.ordinary_chat_auto_maturation_enabled;
    let mut blocking_reasons = Vec::new();

    if !runtime_output_candidate_shape_present {
        push_unique_reason(
            &mut blocking_reasons,
            "runtime_output_candidate_shape_missing",
        );
    }
    if !maturation_service_present {
        push_unique_reason(&mut blocking_reasons, "maturation_service_missing");
    }
    if !evidence_store_present {
        push_unique_reason(&mut blocking_reasons, "evidence_store_missing");
    }
    if !proposal_store_present {
        push_unique_reason(&mut blocking_reasons, "proposal_store_missing");
    }
    if !governor_present {
        push_unique_reason(&mut blocking_reasons, "governor_missing");
    }
    if !default_chat_unchanged {
        push_unique_reason(
            &mut blocking_reasons,
            "default_chat_route_migration_assumed",
        );
    }
    if !ordinary_chat_entrypoint_unchanged {
        push_unique_reason(
            &mut blocking_reasons,
            "ordinary_chat_auto_maturation_assumed",
        );
    }
    if input.require_direct_life_model_write {
        push_unique_reason(&mut blocking_reasons, "direct_lifemodel_write_required");
    }
    if input.require_direct_memory_write {
        push_unique_reason(&mut blocking_reasons, "direct_memory_write_required");
    }
    if input.require_heuristic_activation {
        push_unique_reason(&mut blocking_reasons, "heuristic_activation_required");
    }

    let candidate_digest = input
        .candidate
        .as_ref()
        .map(|draft| draft_digest(draft, draft.source_run_id.as_deref()));
    let candidate_confidence = input
        .candidate
        .as_ref()
        .map(|draft| confidence_from_metadata(&draft.metadata).unwrap_or(DEFAULT_CONFIDENCE));
    let source_lineage_present = input
        .candidate
        .as_ref()
        .and_then(|draft| draft.source_run_id.as_deref())
        .map(|source| !source.trim().is_empty())
        .unwrap_or(false);
    let contains_raw_content = input
        .candidate
        .as_ref()
        .map(candidate_contains_raw_content)
        .unwrap_or(false);

    match input.candidate.as_ref() {
        Some(candidate) => {
            if !is_low_energy_planning_candidate(candidate) {
                push_unique_reason(
                    &mut blocking_reasons,
                    "candidate_type_outside_low_energy_planning_domain",
                );
            }
            if candidate_confidence.unwrap_or(DEFAULT_CONFIDENCE) < MIN_CONFIDENCE {
                push_unique_reason(&mut blocking_reasons, "candidate_confidence_too_low");
            }
            if proposal_only_from_metadata(&candidate.metadata) == Some(false) {
                push_unique_reason(&mut blocking_reasons, "proposal_only_false");
            }
            if contains_raw_content {
                push_unique_reason(
                    &mut blocking_reasons,
                    "candidate_metadata_contains_raw_content",
                );
            }
            if !source_lineage_present {
                push_unique_reason(&mut blocking_reasons, "source_lineage_missing");
            }
        }
        None => push_unique_reason(&mut blocking_reasons, "candidate_missing"),
    }

    let ready = blocking_reasons.is_empty();
    LifeModelMaturationReadinessReport {
        readiness_ready: ready,
        ready,
        default_chat_unchanged,
        ordinary_chat_entrypoint_unchanged,
        runtime_output_candidate_shape_present,
        maturation_service_present,
        evidence_store_present,
        proposal_store_present,
        governor_present,
        proposal_first_required: true,
        direct_life_model_write_allowed: false,
        direct_memory_write_allowed: false,
        direct_heuristic_write_allowed: false,
        heuristic_activation_allowed: false,
        low_energy_planning_domain_only: true,
        metadata_safe: true,
        contains_raw_content,
        source_lineage_required: true,
        source_lineage_present,
        negative_evidence_required_for_rejection: true,
        accepted_rule_runtime_packet_future_only: true,
        business_write_disabled: true,
        side_effect_budget_zero: true,
        side_effect_budget: LifeModelMaturationReadinessSideEffectBudget::default(),
        candidate_digest,
        candidate_confidence,
        blocking_reasons,
        next_allowed_step: if ready {
            MATURATION_NEXT_ALLOWED_STEP.into()
        } else {
            "blocked".into()
        },
    }
}

pub fn ensure_lifemodel_maturation_readiness(
    input: LifeModelMaturationReadinessInput,
) -> Result<LifeModelMaturationReadinessReport> {
    let report = evaluate_lifemodel_maturation_readiness(input);
    if report.ready {
        Ok(report)
    } else {
        Err(anyhow!(
            "lifemodel maturation readiness blocked: {}",
            report.blocking_reasons.join(",")
        ))
    }
}

#[derive(Clone)]
pub struct LifeModelMaturationNonDefaultInvocationInput {
    pub runtime_output: RuntimeOutput,
    pub default_chat_selected_adapter_path: String,
    pub ordinary_chat_auto_maturation_enabled: bool,
    pub require_direct_life_model_write: bool,
    pub require_direct_memory_write: bool,
    pub require_heuristic_activation: bool,
}

impl LifeModelMaturationNonDefaultInvocationInput {
    pub fn for_runtime_output(runtime_output: RuntimeOutput) -> Self {
        Self {
            runtime_output,
            default_chat_selected_adapter_path: DEFAULT_CHAT_KERNEL_PATH.into(),
            ordinary_chat_auto_maturation_enabled: false,
            require_direct_life_model_write: false,
            require_direct_memory_write: false,
            require_heuristic_activation: false,
        }
    }

    pub fn run(
        self,
        evidence_store: &EvidenceStore,
        proposal_store: &ProposalStore,
    ) -> Result<LifeModelMaturationNonDefaultInvocationReport> {
        run_lifemodel_maturation_non_default_invocation(self, evidence_store, proposal_store)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelMaturationNonDefaultInvocationReport {
    pub invocation_ready: bool,
    pub readiness_report: LifeModelMaturationReadinessReport,
    pub non_default_invocation: bool,
    pub default_chat_unchanged: bool,
    pub ordinary_chat_entrypoint_unchanged: bool,
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
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub source_run_id: Option<String>,
    pub evidence_ids: Vec<String>,
    pub proposal_ids: Vec<String>,
    pub blocking_reasons: Vec<String>,
}

pub fn run_lifemodel_maturation_non_default_invocation(
    input: LifeModelMaturationNonDefaultInvocationInput,
    evidence_store: &EvidenceStore,
    proposal_store: &ProposalStore,
) -> Result<LifeModelMaturationNonDefaultInvocationReport> {
    let candidate_count = input.runtime_output.life_event_candidates.len();
    let candidate = input.runtime_output.life_event_candidates.first().cloned();
    let source_run_id = candidate
        .as_ref()
        .and_then(|candidate| candidate.source_run_id.clone());
    let readiness_report =
        evaluate_lifemodel_maturation_readiness(LifeModelMaturationReadinessInput {
            candidate,
            default_chat_selected_adapter_path: input.default_chat_selected_adapter_path,
            ordinary_chat_auto_maturation_enabled: input.ordinary_chat_auto_maturation_enabled,
            require_direct_life_model_write: input.require_direct_life_model_write,
            require_direct_memory_write: input.require_direct_memory_write,
            require_heuristic_activation: input.require_heuristic_activation,
        });

    let mut blocking_reasons = readiness_report.blocking_reasons.clone();
    if candidate_count != 1 {
        push_unique_reason(&mut blocking_reasons, "candidate_count_not_one");
    }
    if readiness_report.next_allowed_step != MATURATION_NEXT_ALLOWED_STEP {
        push_unique_reason(
            &mut blocking_reasons,
            "readiness_next_step_not_non_default_invocation",
        );
    }

    let mut report = LifeModelMaturationNonDefaultInvocationReport {
        invocation_ready: false,
        default_chat_unchanged: readiness_report.default_chat_unchanged,
        ordinary_chat_entrypoint_unchanged: readiness_report.ordinary_chat_entrypoint_unchanged,
        non_default_invocation: true,
        wrote_evidence_count: 0,
        wrote_proposal_count: 0,
        wrote_life_model_count: 0,
        wrote_memory_count: 0,
        wrote_heuristic_count: 0,
        wrote_chat_message_count: 0,
        wrote_agent_run_count: 0,
        wrote_mcp_audit_count: 0,
        wrote_external_count: 0,
        ran_runtime: false,
        ran_model: false,
        ran_tool: false,
        metadata_safe: readiness_report.metadata_safe,
        contains_raw_content: readiness_report.contains_raw_content,
        source_run_id,
        evidence_ids: Vec::new(),
        proposal_ids: Vec::new(),
        blocking_reasons,
        readiness_report,
    };

    if !report.readiness_report.ready || !report.blocking_reasons.is_empty() {
        return Ok(report);
    }

    let maturation_report = MaturationService::default().mature_runtime_output(
        &input.runtime_output,
        evidence_store,
        proposal_store,
    )?;
    report.evidence_ids = maturation_report.evidence_ids;
    report.proposal_ids = maturation_report.proposal_ids;
    report.wrote_evidence_count = report.evidence_ids.len() as u32;
    report.wrote_proposal_count = report.proposal_ids.len() as u32;
    report.invocation_ready = true;
    Ok(report)
}

pub fn ensure_lifemodel_maturation_non_default_invocation(
    input: LifeModelMaturationNonDefaultInvocationInput,
    evidence_store: &EvidenceStore,
    proposal_store: &ProposalStore,
) -> Result<LifeModelMaturationNonDefaultInvocationReport> {
    let report =
        run_lifemodel_maturation_non_default_invocation(input, evidence_store, proposal_store)?;
    if report.invocation_ready {
        Ok(report)
    } else {
        Err(anyhow!(
            "lifemodel maturation non-default invocation blocked: {}",
            report.blocking_reasons.join(",")
        ))
    }
}

#[derive(Clone)]
pub struct LowEnergyCollaborationRuleCandidateInput {
    pub outcome_evidence: Vec<EvidenceRecord>,
    pub target_domain: String,
    pub default_chat_selected_adapter_path: String,
    pub ordinary_chat_auto_rule_candidate_enabled: bool,
    pub require_direct_life_model_write: bool,
    pub require_direct_memory_write: bool,
    pub require_heuristic_activation: bool,
    pub min_supporting_outcome_count: usize,
}

impl Default for LowEnergyCollaborationRuleCandidateInput {
    fn default() -> Self {
        Self {
            outcome_evidence: Vec::new(),
            target_domain: "low_energy_planning".into(),
            default_chat_selected_adapter_path: DEFAULT_CHAT_KERNEL_PATH.into(),
            ordinary_chat_auto_rule_candidate_enabled: false,
            require_direct_life_model_write: false,
            require_direct_memory_write: false,
            require_heuristic_activation: false,
            min_supporting_outcome_count: DEFAULT_LOW_ENERGY_RULE_CANDIDATE_MIN_SUPPORT,
        }
    }
}

impl LowEnergyCollaborationRuleCandidateInput {
    pub fn for_outcome_evidence(outcome_evidence: Vec<EvidenceRecord>) -> Self {
        Self {
            outcome_evidence,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LowEnergyCollaborationRuleCandidateReport {
    pub ready: bool,
    pub reviewable_candidate_ready: bool,
    pub default_chat_unchanged: bool,
    pub ordinary_chat_entrypoint_unchanged: bool,
    pub low_energy_planning_domain_only: bool,
    pub target_domain: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub candidate_only: bool,
    pub activates_heuristic: bool,
    pub writes_active_rule: bool,
    pub accepted_outcome_evidence_ids: Vec<String>,
    pub rejected_outcome_evidence_ids: Vec<String>,
    pub edited_outcome_evidence_ids: Vec<String>,
    pub opposing_outcome_evidence_ids: Vec<String>,
    pub source_evidence_ids: Vec<String>,
    pub linked_proposal_ids: Vec<String>,
    pub linked_agent_run_ids: Vec<String>,
    pub support_outcome_count: usize,
    pub opposing_outcome_count: usize,
    pub weakened_by_opposing_outcome: bool,
    pub candidate_rule_id: String,
    pub candidate_rule_digest: String,
    pub candidate_rule_summary: String,
    pub candidate_confidence: f32,
    pub candidate_proposal_id: Option<String>,
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
    pub blocking_reasons: Vec<String>,
}

pub fn evaluate_low_energy_collaboration_rule_candidate(
    input: LowEnergyCollaborationRuleCandidateInput,
) -> LowEnergyCollaborationRuleCandidateReport {
    let mut accepted_outcome_evidence_ids = Vec::new();
    let mut rejected_outcome_evidence_ids = Vec::new();
    let mut edited_outcome_evidence_ids = Vec::new();
    let mut opposing_outcome_evidence_ids = Vec::new();
    let mut source_evidence_ids = Vec::new();
    let mut linked_proposal_ids = Vec::new();
    let mut linked_agent_run_ids = Vec::new();
    let mut blocking_reasons = Vec::new();
    let mut metadata_safe = true;
    let mut contains_raw_content = false;
    let mut recognized_outcome_count = 0usize;

    let default_chat_unchanged =
        input.default_chat_selected_adapter_path == DEFAULT_CHAT_KERNEL_PATH;
    let ordinary_chat_entrypoint_unchanged = !input.ordinary_chat_auto_rule_candidate_enabled;

    if !default_chat_unchanged {
        push_unique_reason(
            &mut blocking_reasons,
            "default_chat_route_migration_assumed",
        );
    }
    if !ordinary_chat_entrypoint_unchanged {
        push_unique_reason(
            &mut blocking_reasons,
            "ordinary_chat_auto_rule_candidate_assumed",
        );
    }
    if input.require_direct_life_model_write {
        push_unique_reason(&mut blocking_reasons, "direct_lifemodel_write_required");
    }
    if input.require_direct_memory_write {
        push_unique_reason(&mut blocking_reasons, "direct_memory_write_required");
    }
    if input.require_heuristic_activation {
        push_unique_reason(&mut blocking_reasons, "heuristic_activation_required");
    }

    if !is_low_energy_rule_candidate_domain(&input.target_domain) {
        push_unique_reason(&mut blocking_reasons, "non_low_energy_planning_domain");
    }

    for record in &input.outcome_evidence {
        if record.evidence_type != EvidenceType::ProposalOutcome {
            push_unique_reason(
                &mut blocking_reasons,
                "non_proposal_outcome_evidence_present",
            );
            continue;
        }
        let record_contains_raw = outcome_record_contains_raw_content(record);
        if record_contains_raw {
            contains_raw_content = true;
            metadata_safe = false;
            push_unique_reason(
                &mut blocking_reasons,
                "outcome_evidence_contains_raw_content",
            );
        }

        let record_metadata_safe = record
            .run_metadata
            .get("metadataSafe")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && !record
                .run_metadata
                .get("containsRawContent")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        if !record_metadata_safe {
            metadata_safe = false;
            push_unique_reason(&mut blocking_reasons, "outcome_evidence_metadata_not_safe");
        }

        if !is_maturation_proposal_outcome_record(record) {
            push_unique_reason(&mut blocking_reasons, "maturation_outcome_lineage_missing");
            continue;
        }
        if !outcome_record_in_low_energy_collaboration_scope(record) {
            push_unique_reason(
                &mut blocking_reasons,
                "outcome_evidence_outside_low_energy_collaboration_scope",
            );
        }

        recognized_outcome_count += 1;
        let outcome = record
            .run_metadata
            .get("outcome")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match outcome {
            "accepted" => push_unique_string(&mut accepted_outcome_evidence_ids, &record.id),
            "rejected" => push_unique_string(&mut rejected_outcome_evidence_ids, &record.id),
            "edited" => push_unique_string(&mut edited_outcome_evidence_ids, &record.id),
            _ => push_unique_reason(&mut blocking_reasons, "unknown_proposal_outcome"),
        }

        let negative = record
            .run_metadata
            .get("negative")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let opposing = record
            .run_metadata
            .get("opposing")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if outcome == "rejected" || negative || opposing || !record.opposing_refs.is_empty() {
            push_unique_string(&mut opposing_outcome_evidence_ids, &record.id);
        }

        for source_evidence_id in metadata_string_array(&record.run_metadata, "sourceEvidenceIds") {
            push_unique_string(&mut source_evidence_ids, &source_evidence_id);
        }
        if let Some(proposal_id) = record
            .run_metadata
            .get("proposalId")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            push_unique_string(&mut linked_proposal_ids, proposal_id);
        }
        for proposal_id in &record.linked_proposal_ids {
            push_unique_string(&mut linked_proposal_ids, proposal_id);
        }
        if let Some(run_id) = record
            .run_metadata
            .get("sourceRunId")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            push_unique_string(&mut linked_agent_run_ids, run_id);
        }
        for run_id in metadata_string_array(&record.run_metadata, "linkedAgentRunIds") {
            push_unique_string(&mut linked_agent_run_ids, &run_id);
        }
        for run_id in &record.linked_agent_run_ids {
            push_unique_string(&mut linked_agent_run_ids, run_id);
        }
    }

    if recognized_outcome_count == 0 {
        push_unique_reason(&mut blocking_reasons, "proposal_outcome_evidence_missing");
    }

    let support_outcome_count =
        accepted_outcome_evidence_ids.len() + edited_outcome_evidence_ids.len();
    let opposing_outcome_count = opposing_outcome_evidence_ids.len();
    if support_outcome_count < input.min_supporting_outcome_count {
        push_unique_reason(
            &mut blocking_reasons,
            "supporting_outcome_evidence_insufficient",
        );
    }
    if opposing_outcome_count > 0 && opposing_outcome_count >= support_outcome_count {
        push_unique_reason(
            &mut blocking_reasons,
            "opposing_outcome_evidence_blocks_candidate",
        );
    }
    if support_outcome_count > 0 && source_evidence_ids.is_empty() {
        push_unique_reason(&mut blocking_reasons, "source_evidence_lineage_missing");
    }
    if support_outcome_count > 0 && linked_proposal_ids.is_empty() {
        push_unique_reason(&mut blocking_reasons, "linked_proposal_lineage_missing");
    }
    if support_outcome_count > 0 && linked_agent_run_ids.is_empty() {
        push_unique_reason(&mut blocking_reasons, "linked_agent_run_lineage_missing");
    }

    let weakened_by_opposing_outcome = opposing_outcome_count > 0;
    let candidate_confidence =
        low_energy_candidate_confidence(support_outcome_count, opposing_outcome_count);
    let candidate_rule_id = BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING.to_string();
    let candidate_rule_digest = low_energy_candidate_rule_digest(
        &input.target_domain,
        &candidate_rule_id,
        &accepted_outcome_evidence_ids,
        &edited_outcome_evidence_ids,
        &rejected_outcome_evidence_ids,
    );
    let ready = blocking_reasons.is_empty();

    LowEnergyCollaborationRuleCandidateReport {
        ready,
        reviewable_candidate_ready: ready,
        default_chat_unchanged,
        ordinary_chat_entrypoint_unchanged,
        low_energy_planning_domain_only: true,
        target_domain: input.target_domain,
        metadata_safe,
        contains_raw_content,
        candidate_only: true,
        activates_heuristic: false,
        writes_active_rule: false,
        accepted_outcome_evidence_ids,
        rejected_outcome_evidence_ids,
        edited_outcome_evidence_ids,
        opposing_outcome_evidence_ids,
        source_evidence_ids,
        linked_proposal_ids,
        linked_agent_run_ids,
        support_outcome_count,
        opposing_outcome_count,
        weakened_by_opposing_outcome,
        candidate_rule_id,
        candidate_rule_digest,
        candidate_rule_summary: LOW_ENERGY_RULE_CANDIDATE_SUMMARY.into(),
        candidate_confidence,
        candidate_proposal_id: None,
        wrote_evidence_count: 0,
        wrote_proposal_count: 0,
        wrote_life_model_count: 0,
        wrote_memory_count: 0,
        wrote_heuristic_count: 0,
        wrote_chat_message_count: 0,
        wrote_agent_run_count: 0,
        wrote_mcp_audit_count: 0,
        wrote_external_count: 0,
        ran_runtime: false,
        ran_model: false,
        ran_tool: false,
        blocking_reasons,
    }
}

pub fn propose_low_energy_collaboration_rule_candidate(
    input: LowEnergyCollaborationRuleCandidateInput,
    proposal_store: &ProposalStore,
) -> Result<LowEnergyCollaborationRuleCandidateReport> {
    let mut report = evaluate_low_energy_collaboration_rule_candidate(input);
    if !report.ready {
        return Ok(report);
    }

    let mut proposal = AgentProposal::new(
        ProposalType::Unsupported,
        LOW_ENERGY_RULE_CANDIDATE_PATH,
        low_energy_candidate_proposal_payload(&report),
        "Metadata-safe maturation proposal outcomes support a reviewable low-energy collaboration rule candidate; human review is required before any heuristic activation.",
        report.candidate_confidence,
        RiskLevel::Low,
        ProposalSource::FeedbackEvolution,
    );
    proposal.run_id = report.linked_agent_run_ids.first().cloned();
    proposal.source_detail = Some(LOW_ENERGY_RULE_CANDIDATE_SOURCE_DETAIL.into());
    proposal_store.create_proposal(&proposal)?;

    report.candidate_proposal_id = Some(proposal.id);
    report.wrote_proposal_count = 1;
    Ok(report)
}

#[derive(Clone)]
pub struct AcceptedLowEnergyRuleSelectionInput {
    pub candidate_proposal: Option<AgentProposal>,
    pub target_task_kind: AgentTaskKind,
    pub target_domain: String,
    pub planning_intent_present: bool,
    pub privacy_topic: PolicyTopic,
    pub current_route_policy: ModelRoutePolicy,
    pub existing_hs_packet: Option<RuntimeHSPacket>,
}

impl Default for AcceptedLowEnergyRuleSelectionInput {
    fn default() -> Self {
        Self {
            candidate_proposal: None,
            target_task_kind: AgentTaskKind::Planning,
            target_domain: "low_energy_planning".into(),
            planning_intent_present: true,
            privacy_topic: PolicyTopic::General,
            current_route_policy: ModelRoutePolicy::CloudAllowed,
            existing_hs_packet: None,
        }
    }
}

impl AcceptedLowEnergyRuleSelectionInput {
    pub fn for_candidate(candidate_proposal: AgentProposal) -> Self {
        Self {
            candidate_proposal: Some(candidate_proposal),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedLowEnergyRuleSelectionHSPacketAuditProof {
    pub metadata_safe: bool,
    pub planning_task_only: bool,
    pub low_energy_domain_only: bool,
    pub privacy_policy_relaxed: bool,
    pub enforced_route_policy: ModelRoutePolicy,
    pub selected_policy_ids: Vec<String>,
    pub selected_guidance_summary: Option<String>,
    pub selected_candidate_proposal_id: Option<String>,
    pub selected_candidate_rule_digest: Option<String>,
    pub source_outcome_evidence_ids: Vec<String>,
    pub source_proposal_ids: Vec<String>,
    pub source_agent_run_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedLowEnergyRuleSelectionReport {
    pub selected: bool,
    pub planning_task_only: bool,
    pub low_energy_domain_only: bool,
    pub privacy_policy_relaxed: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub target_task_kind: AgentTaskKind,
    pub target_domain: String,
    pub privacy_topic: PolicyTopic,
    pub current_route_policy: ModelRoutePolicy,
    pub enforced_route_policy: ModelRoutePolicy,
    pub selected_guidance_summary: Option<String>,
    pub selected_candidate_proposal_id: Option<String>,
    pub selected_candidate_rule_digest: Option<String>,
    pub source_outcome_evidence_ids: Vec<String>,
    pub source_proposal_ids: Vec<String>,
    pub source_agent_run_ids: Vec<String>,
    pub blocking_reasons: Vec<String>,
    pub hs_packet_audit_proof: AcceptedLowEnergyRuleSelectionHSPacketAuditProof,
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

pub fn evaluate_accepted_low_energy_rule_selection(
    input: AcceptedLowEnergyRuleSelectionInput,
) -> AcceptedLowEnergyRuleSelectionReport {
    let mut blocking_reasons = Vec::new();
    let mut metadata_safe = true;
    let mut contains_raw_content = false;
    let mut selected_guidance_summary = None;
    let mut selected_candidate_proposal_id = None;
    let mut selected_candidate_rule_digest = None;
    let mut source_outcome_evidence_ids = Vec::new();
    let mut source_proposal_ids = Vec::new();
    let mut source_agent_run_ids = Vec::new();

    if !matches!(input.target_task_kind, AgentTaskKind::Planning) || !input.planning_intent_present
    {
        push_unique_reason(&mut blocking_reasons, "non_planning_task");
    }
    if !is_low_energy_rule_candidate_domain(&input.target_domain) {
        push_unique_reason(&mut blocking_reasons, "non_low_energy_planning_domain");
    }

    match input.candidate_proposal.as_ref() {
        Some(candidate) => {
            if candidate.status != ProposalStatus::Accepted {
                push_unique_reason(&mut blocking_reasons, "candidate_proposal_not_accepted");
            }
            if !is_w76_low_energy_rule_candidate_proposal(candidate) {
                push_unique_reason(
                    &mut blocking_reasons,
                    "candidate_proposal_not_w76_low_energy_rule_candidate",
                );
            }

            let candidate_contains_raw = outcome_metadata_contains_raw_content(&candidate.after);
            if candidate_contains_raw {
                contains_raw_content = true;
                metadata_safe = false;
                push_unique_reason(
                    &mut blocking_reasons,
                    "candidate_proposal_contains_raw_content",
                );
            }
            let candidate_metadata_safe = candidate
                .after
                .get("metadataSafe")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && !candidate
                    .after
                    .get("containsRawContent")
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
                && !candidate_contains_raw;
            if !candidate_metadata_safe {
                metadata_safe = false;
                push_unique_reason(
                    &mut blocking_reasons,
                    "candidate_proposal_metadata_not_safe",
                );
            }

            if !candidate
                .after
                .get("candidateOnly")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                push_unique_reason(&mut blocking_reasons, "candidate_only_false");
            }
            if candidate
                .after
                .get("activatesHeuristic")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                push_unique_reason(&mut blocking_reasons, "heuristic_activation_attempted");
            }
            if candidate
                .after
                .get("writesActiveRule")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                push_unique_reason(&mut blocking_reasons, "active_rule_write_attempted");
            }
            if candidate
                .after
                .get("heuristicActivationAllowed")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                push_unique_reason(&mut blocking_reasons, "heuristic_activation_allowed");
            }

            if let Some(candidate_domain) = candidate
                .after
                .get("targetDomain")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
            {
                if !is_low_energy_rule_candidate_domain(candidate_domain) {
                    push_unique_reason(
                        &mut blocking_reasons,
                        "candidate_rule_non_low_energy_domain",
                    );
                }
            } else {
                push_unique_reason(&mut blocking_reasons, "candidate_rule_domain_missing");
            }

            let guidance_summary = candidate
                .after
                .get("ruleSummary")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            if guidance_summary.is_none() {
                push_unique_reason(&mut blocking_reasons, "candidate_rule_summary_missing");
            }
            if guidance_summary
                .as_deref()
                .is_some_and(string_looks_secret_like)
            {
                metadata_safe = false;
                contains_raw_content = true;
                push_unique_reason(&mut blocking_reasons, "selected_guidance_not_metadata_safe");
            }
            if guidance_summary
                .as_deref()
                .is_some_and(guidance_summary_relaxes_privacy_policy)
                || candidate_attempts_privacy_route_override(&candidate.after)
            {
                push_unique_reason(
                    &mut blocking_reasons,
                    "candidate_attempts_privacy_route_override",
                );
            }

            if let Some(digest) = candidate
                .after
                .get("candidateRuleDigest")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                selected_candidate_rule_digest = Some(digest.to_string());
            } else {
                push_unique_reason(&mut blocking_reasons, "candidate_rule_digest_missing");
            }

            selected_guidance_summary = guidance_summary;
            selected_candidate_proposal_id = Some(candidate.id.clone());
            source_outcome_evidence_ids = candidate_source_outcome_evidence_ids(&candidate.after);
            source_proposal_ids =
                candidate_source_lineage_string_array(&candidate.after, "linkedProposalIds");
            source_agent_run_ids =
                candidate_source_lineage_string_array(&candidate.after, "linkedAgentRunIds");
            if source_outcome_evidence_ids.is_empty() {
                push_unique_reason(
                    &mut blocking_reasons,
                    "source_outcome_evidence_lineage_missing",
                );
            }
            if source_proposal_ids.is_empty() {
                push_unique_reason(&mut blocking_reasons, "source_proposal_lineage_missing");
            }
            if source_agent_run_ids.is_empty() {
                push_unique_reason(&mut blocking_reasons, "source_agent_run_lineage_missing");
            }
        }
        None => push_unique_reason(&mut blocking_reasons, "candidate_proposal_missing"),
    }

    let selected_policy_ids = selected_policy_ids_for_selection(
        input.existing_hs_packet.as_ref(),
        input.privacy_topic,
        input.current_route_policy,
    );
    let enforced_route_policy = enforced_selection_route_policy(
        input.existing_hs_packet.as_ref(),
        input.privacy_topic,
        input.current_route_policy,
    );
    let selected = blocking_reasons.is_empty();
    if !selected {
        selected_guidance_summary = None;
        selected_candidate_proposal_id = None;
        selected_candidate_rule_digest = None;
    }

    let hs_packet_audit_proof = AcceptedLowEnergyRuleSelectionHSPacketAuditProof {
        metadata_safe,
        planning_task_only: true,
        low_energy_domain_only: true,
        privacy_policy_relaxed: false,
        enforced_route_policy,
        selected_policy_ids,
        selected_guidance_summary: selected_guidance_summary.clone(),
        selected_candidate_proposal_id: selected_candidate_proposal_id.clone(),
        selected_candidate_rule_digest: selected_candidate_rule_digest.clone(),
        source_outcome_evidence_ids: source_outcome_evidence_ids.clone(),
        source_proposal_ids: source_proposal_ids.clone(),
        source_agent_run_ids: source_agent_run_ids.clone(),
    };

    AcceptedLowEnergyRuleSelectionReport {
        selected,
        planning_task_only: true,
        low_energy_domain_only: true,
        privacy_policy_relaxed: false,
        metadata_safe,
        contains_raw_content,
        target_task_kind: input.target_task_kind,
        target_domain: input.target_domain,
        privacy_topic: input.privacy_topic,
        current_route_policy: input.current_route_policy,
        enforced_route_policy,
        selected_guidance_summary,
        selected_candidate_proposal_id,
        selected_candidate_rule_digest,
        source_outcome_evidence_ids,
        source_proposal_ids,
        source_agent_run_ids,
        blocking_reasons,
        hs_packet_audit_proof,
        wrote_evidence_count: 0,
        wrote_proposal_count: 0,
        wrote_life_model_count: 0,
        wrote_memory_count: 0,
        wrote_heuristic_count: 0,
        wrote_chat_message_count: 0,
        wrote_agent_run_count: 0,
        wrote_mcp_audit_count: 0,
        wrote_external_count: 0,
        ran_runtime: false,
        ran_model: false,
        ran_tool: false,
    }
}

pub fn ensure_accepted_low_energy_rule_selection(
    input: AcceptedLowEnergyRuleSelectionInput,
) -> Result<AcceptedLowEnergyRuleSelectionReport> {
    let report = evaluate_accepted_low_energy_rule_selection(input);
    if report.selected {
        Ok(report)
    } else {
        Err(anyhow!(
            "accepted low-energy rule selection blocked: {}",
            report.blocking_reasons.join(",")
        ))
    }
}

#[derive(Clone)]
pub struct LowEnergyRuleTraceVisibilityInput {
    pub selection_report: AcceptedLowEnergyRuleSelectionReport,
    pub trace_payload: Option<Value>,
    pub default_chat_selected_adapter_path: String,
    pub ordinary_chat_entrypoint_attached: bool,
    pub runtime_executed: bool,
    pub model_called: bool,
    pub tool_called: bool,
    pub life_model_written: bool,
    pub memory_written: bool,
    pub heuristic_activated: bool,
    pub agent_run_written: bool,
}

impl LowEnergyRuleTraceVisibilityInput {
    pub fn for_selection_report(selection_report: AcceptedLowEnergyRuleSelectionReport) -> Self {
        Self {
            selection_report,
            trace_payload: None,
            default_chat_selected_adapter_path: DEFAULT_CHAT_KERNEL_PATH.into(),
            ordinary_chat_entrypoint_attached: false,
            runtime_executed: false,
            model_called: false,
            tool_called: false,
            life_model_written: false,
            memory_written: false,
            heuristic_activated: false,
            agent_run_written: false,
        }
    }

    pub fn with_trace_payload(mut self, trace_payload: Value) -> Self {
        self.trace_payload = Some(trace_payload);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LowEnergyRuleTraceLineageItem {
    pub id: String,
    pub id_hash: String,
    pub record_type: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LowEnergyRuleTraceLineageSummary {
    pub items: Vec<LowEnergyRuleTraceLineageItem>,
    pub count: usize,
    pub ids_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LowEnergyRuleTraceMetadata {
    pub schema: String,
    pub trace_kind: String,
    pub trace_metadata_fields: Vec<String>,
    pub trace_metadata_field_count: usize,
    pub selection_report_hash: String,
    pub trace_payload_hash: Option<String>,
    pub selected_guidance_summary: Option<String>,
    pub selected_guidance_hash: Option<String>,
    pub runtime_hs_packet_guidance_hash: Option<String>,
    pub selected_candidate_proposal_id: Option<String>,
    pub selected_candidate_proposal_hash: Option<String>,
    pub selected_candidate_rule_digest: Option<String>,
    pub target_task_kind: AgentTaskKind,
    pub target_domain: String,
    pub privacy_topic: PolicyTopic,
    pub enforced_route_policy: ModelRoutePolicy,
    pub selected_policy_ids: Vec<String>,
    pub selected_policy_count: usize,
    pub evidence_lineage: LowEnergyRuleTraceLineageSummary,
    pub proposal_lineage: LowEnergyRuleTraceLineageSummary,
    pub agent_run_lineage: LowEnergyRuleTraceLineageSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LowEnergyRuleTraceVisibilityReport {
    pub trace_visibility_ready: bool,
    pub selected_rule_visible: bool,
    pub runtime_hs_packet_guidance_visible: bool,
    pub evidence_lineage_visible: bool,
    pub proposal_lineage_visible: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub default_chat_unchanged: bool,
    pub ordinary_chat_entrypoint_attached: bool,
    pub runtime_executed: bool,
    pub model_called: bool,
    pub tool_called: bool,
    pub life_model_written: bool,
    pub memory_written: bool,
    pub heuristic_activated: bool,
    pub agent_run_written: bool,
    pub privacy_policy_preserved: bool,
    pub local_only_policy_preserved: bool,
    pub target_task_kind: AgentTaskKind,
    pub target_domain: String,
    pub privacy_topic: PolicyTopic,
    pub enforced_route_policy: ModelRoutePolicy,
    pub trace_metadata: LowEnergyRuleTraceMetadata,
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

pub fn evaluate_low_energy_rule_trace_visibility(
    input: LowEnergyRuleTraceVisibilityInput,
) -> LowEnergyRuleTraceVisibilityReport {
    let selection_report = input.selection_report;
    let mut blocking_reasons = Vec::new();
    let mut metadata_safe = selection_report.metadata_safe
        && selection_report.hs_packet_audit_proof.metadata_safe
        && !selection_report.contains_raw_content;
    let mut contains_raw_content = selection_report.contains_raw_content;
    let default_chat_unchanged =
        input.default_chat_selected_adapter_path == DEFAULT_CHAT_KERNEL_PATH;

    if !selection_report.selected {
        push_unique_reason(&mut blocking_reasons, "w77_selection_not_selected");
    }
    if !selection_report.blocking_reasons.is_empty() {
        push_unique_reason(&mut blocking_reasons, "w77_selection_blocked");
        for reason in &selection_report.blocking_reasons {
            push_unique_reason(&mut blocking_reasons, reason);
        }
    }
    if !selection_report.metadata_safe || !selection_report.hs_packet_audit_proof.metadata_safe {
        metadata_safe = false;
        push_unique_reason(&mut blocking_reasons, "w77_selection_metadata_not_safe");
    }
    if selection_report.contains_raw_content {
        contains_raw_content = true;
        metadata_safe = false;
        push_unique_reason(&mut blocking_reasons, "w77_selection_contains_raw_content");
    }
    if !selection_report.planning_task_only
        || !selection_report.hs_packet_audit_proof.planning_task_only
        || !matches!(selection_report.target_task_kind, AgentTaskKind::Planning)
    {
        push_unique_reason(&mut blocking_reasons, "non_planning_task");
    }
    if !selection_report.low_energy_domain_only
        || !selection_report
            .hs_packet_audit_proof
            .low_energy_domain_only
        || !is_low_energy_rule_candidate_domain(&selection_report.target_domain)
    {
        push_unique_reason(&mut blocking_reasons, "non_low_energy_planning_domain");
    }
    if selection_report.privacy_policy_relaxed
        || selection_report
            .hs_packet_audit_proof
            .privacy_policy_relaxed
    {
        push_unique_reason(&mut blocking_reasons, "privacy_policy_relaxed");
    }
    if !default_chat_unchanged {
        push_unique_reason(
            &mut blocking_reasons,
            "default_chat_route_migration_assumed",
        );
    }
    if input.ordinary_chat_entrypoint_attached {
        push_unique_reason(&mut blocking_reasons, "ordinary_chat_entrypoint_attached");
    }
    if input.runtime_executed {
        push_unique_reason(&mut blocking_reasons, "runtime_execution_implied");
    }
    if input.model_called {
        push_unique_reason(&mut blocking_reasons, "model_call_implied");
    }
    if input.tool_called {
        push_unique_reason(&mut blocking_reasons, "tool_call_implied");
    }
    if input.life_model_written {
        push_unique_reason(&mut blocking_reasons, "lifemodel_write_implied");
    }
    if input.memory_written {
        push_unique_reason(&mut blocking_reasons, "memory_write_implied");
    }
    if input.heuristic_activated {
        push_unique_reason(&mut blocking_reasons, "heuristic_activation_implied");
    }
    if input.agent_run_written {
        push_unique_reason(&mut blocking_reasons, "agent_run_write_implied");
    }

    let mut trace_payload_hash = None;
    let mut trace_payload_relaxes_policy = false;
    if let Some(trace_payload) = input.trace_payload.as_ref() {
        trace_payload_hash = Some(sha256_hex(trace_payload.to_string().as_bytes()));
        if trace_payload_metadata_not_safe(trace_payload) {
            metadata_safe = false;
            push_unique_reason(&mut blocking_reasons, "trace_payload_metadata_not_safe");
        }
        if trace_payload_contains_raw_content(trace_payload) {
            metadata_safe = false;
            contains_raw_content = true;
            push_unique_reason(&mut blocking_reasons, "trace_payload_contains_raw_content");
        }
        if trace_payload_relaxes_privacy_or_route_policy(trace_payload) {
            trace_payload_relaxes_policy = true;
            push_unique_reason(
                &mut blocking_reasons,
                "trace_payload_relaxes_privacy_or_route_policy",
            );
        }
        if trace_payload_implies_default_chat_route_cutover(trace_payload) {
            push_unique_reason(
                &mut blocking_reasons,
                "trace_payload_implies_default_chat_route_cutover",
            );
        }
        if trace_payload_implies_runtime_execution(trace_payload) {
            push_unique_reason(
                &mut blocking_reasons,
                "trace_payload_implies_runtime_execution",
            );
        }
        if trace_payload_implies_model_call(trace_payload) {
            push_unique_reason(&mut blocking_reasons, "trace_payload_implies_model_call");
        }
        if trace_payload_implies_tool_call(trace_payload) {
            push_unique_reason(&mut blocking_reasons, "trace_payload_implies_tool_call");
        }
        if trace_payload_implies_heuristic_activation(trace_payload) {
            push_unique_reason(
                &mut blocking_reasons,
                "trace_payload_implies_heuristic_activation",
            );
        }
    }

    let selected_rule_visible = selection_report.selected
        && selection_report.selected_guidance_summary.is_some()
        && selection_report.selected_candidate_proposal_id.is_some()
        && selection_report.selected_candidate_rule_digest.is_some();
    let runtime_hs_packet_guidance_visible = selected_rule_visible
        && selection_report
            .hs_packet_audit_proof
            .selected_guidance_summary
            == selection_report.selected_guidance_summary
        && selection_report
            .hs_packet_audit_proof
            .selected_candidate_proposal_id
            == selection_report.selected_candidate_proposal_id
        && selection_report
            .hs_packet_audit_proof
            .selected_candidate_rule_digest
            == selection_report.selected_candidate_rule_digest;
    let evidence_lineage_visible = !selection_report.source_outcome_evidence_ids.is_empty()
        && selection_report.source_outcome_evidence_ids
            == selection_report
                .hs_packet_audit_proof
                .source_outcome_evidence_ids;
    let proposal_lineage_visible = !selection_report.source_proposal_ids.is_empty()
        && selection_report.source_proposal_ids
            == selection_report.hs_packet_audit_proof.source_proposal_ids;

    if !selected_rule_visible {
        push_unique_reason(&mut blocking_reasons, "selected_rule_not_visible");
    }
    if !runtime_hs_packet_guidance_visible {
        push_unique_reason(
            &mut blocking_reasons,
            "runtime_hs_packet_guidance_not_visible",
        );
    }
    if !evidence_lineage_visible {
        push_unique_reason(&mut blocking_reasons, "evidence_lineage_not_visible");
    }
    if !proposal_lineage_visible {
        push_unique_reason(&mut blocking_reasons, "proposal_lineage_not_visible");
    }

    let privacy_policy_preserved = !selection_report.privacy_policy_relaxed
        && !selection_report
            .hs_packet_audit_proof
            .privacy_policy_relaxed
        && !trace_payload_relaxes_policy;
    let local_only_required = selection_report.current_route_policy == ModelRoutePolicy::LocalOnly
        || selection_report.enforced_route_policy == ModelRoutePolicy::LocalOnly
        || selection_report.hs_packet_audit_proof.enforced_route_policy
            == ModelRoutePolicy::LocalOnly
        || selection_report
            .hs_packet_audit_proof
            .selected_policy_ids
            .iter()
            .any(|policy_id| policy_id == BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY)
        || topic_requires_local_only(selection_report.privacy_topic);
    let local_only_policy_preserved = !trace_payload_relaxes_policy
        && (!local_only_required
            || selection_report.enforced_route_policy == ModelRoutePolicy::LocalOnly);
    if !privacy_policy_preserved {
        push_unique_reason(&mut blocking_reasons, "privacy_policy_not_preserved");
    }
    if !local_only_policy_preserved {
        push_unique_reason(&mut blocking_reasons, "local_only_policy_not_preserved");
    }

    let trace_metadata = low_energy_rule_trace_metadata(&selection_report, trace_payload_hash);
    let trace_visibility_ready = blocking_reasons.is_empty()
        && selected_rule_visible
        && runtime_hs_packet_guidance_visible
        && evidence_lineage_visible
        && proposal_lineage_visible
        && metadata_safe
        && !contains_raw_content
        && default_chat_unchanged
        && !input.ordinary_chat_entrypoint_attached
        && !input.runtime_executed
        && !input.model_called
        && !input.tool_called
        && !input.life_model_written
        && !input.memory_written
        && !input.heuristic_activated
        && !input.agent_run_written
        && privacy_policy_preserved
        && local_only_policy_preserved;

    LowEnergyRuleTraceVisibilityReport {
        trace_visibility_ready,
        selected_rule_visible,
        runtime_hs_packet_guidance_visible,
        evidence_lineage_visible,
        proposal_lineage_visible,
        metadata_safe,
        contains_raw_content,
        default_chat_unchanged,
        ordinary_chat_entrypoint_attached: input.ordinary_chat_entrypoint_attached,
        runtime_executed: input.runtime_executed,
        model_called: input.model_called,
        tool_called: input.tool_called,
        life_model_written: input.life_model_written,
        memory_written: input.memory_written,
        heuristic_activated: input.heuristic_activated,
        agent_run_written: input.agent_run_written,
        privacy_policy_preserved,
        local_only_policy_preserved,
        target_task_kind: selection_report.target_task_kind,
        target_domain: selection_report.target_domain.clone(),
        privacy_topic: selection_report.privacy_topic,
        enforced_route_policy: selection_report.enforced_route_policy,
        trace_metadata,
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

pub fn ensure_low_energy_rule_trace_visibility(
    input: LowEnergyRuleTraceVisibilityInput,
) -> Result<LowEnergyRuleTraceVisibilityReport> {
    let report = evaluate_low_energy_rule_trace_visibility(input);
    if report.trace_visibility_ready {
        Ok(report)
    } else {
        Err(anyhow!(
            "low-energy rule trace visibility blocked: {}",
            report.blocking_reasons.join(",")
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaturationCandidateDomain {
    PlanningPreference,
    EnergyPattern,
    WorkStyle,
    CommunicationPreference,
}

impl MaturationCandidateDomain {
    pub fn as_str(self) -> &'static str {
        match self {
            MaturationCandidateDomain::PlanningPreference => "planning_preference",
            MaturationCandidateDomain::EnergyPattern => "energy_pattern",
            MaturationCandidateDomain::WorkStyle => "work_style",
            MaturationCandidateDomain::CommunicationPreference => "communication_preference",
        }
    }
}

impl std::fmt::Display for MaturationCandidateDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone)]
pub struct MaturationEngineV1Input {
    pub evidence_graph: EvidenceGraphReport,
    pub default_chat_selected_adapter_path: String,
    pub ordinary_chat_auto_maturation_enabled: bool,
    pub require_direct_life_model_write: bool,
    pub require_direct_memory_write: bool,
    pub require_heuristic_activation: bool,
    pub min_effective_confidence: f32,
    pub min_stability_score: f32,
}

impl MaturationEngineV1Input {
    pub fn from_graph_input(input: EvidenceGraphInput) -> Self {
        Self::from_graph_report(evaluate_evidence_graph(input))
    }

    pub fn from_graph_report(evidence_graph: EvidenceGraphReport) -> Self {
        Self {
            evidence_graph,
            default_chat_selected_adapter_path: DEFAULT_CHAT_KERNEL_PATH.into(),
            ordinary_chat_auto_maturation_enabled: false,
            require_direct_life_model_write: false,
            require_direct_memory_write: false,
            require_heuristic_activation: false,
            min_effective_confidence: MIN_ENGINE_EFFECTIVE_CONFIDENCE,
            min_stability_score: MIN_ENGINE_STABILITY_SCORE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturationEngineCandidate {
    pub candidate_id: String,
    pub candidate_digest: String,
    pub domain: MaturationCandidateDomain,
    pub affected_path: String,
    pub proposal_type: ProposalType,
    pub risk_level: RiskLevel,
    pub source_cluster_id: String,
    pub source_cluster_hash: String,
    pub support_evidence_ids: Vec<String>,
    pub opposing_evidence_ids: Vec<String>,
    pub linked_proposal_ids: Vec<String>,
    pub linked_agent_run_ids: Vec<String>,
    pub source_weight_total: f32,
    pub confidence: f32,
    pub stability_score: f32,
    pub proposal_required: bool,
    pub candidate_only: bool,
    pub candidate_summary: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturationCandidateSuppressionReport {
    pub source_cluster_id: String,
    pub source_cluster_hash: String,
    pub affected_path: String,
    pub domain: Option<MaturationCandidateDomain>,
    pub suppressed: bool,
    pub correction_recommended: bool,
    pub reasons: Vec<String>,
    pub support_evidence_ids: Vec<String>,
    pub opposing_evidence_ids: Vec<String>,
    pub rejected_evidence_ids: Vec<String>,
    pub rejected_proposal_ids: Vec<String>,
    pub linked_proposal_ids: Vec<String>,
    pub linked_agent_run_ids: Vec<String>,
    pub cooldown_active: bool,
    pub conflict_active: bool,
    pub decayed: bool,
    pub rejected_similar_history_count: usize,
    pub effective_confidence: f32,
    pub stability_score: f32,
    pub candidate_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturationEngineV1Report {
    pub report_kind: String,
    pub engine_ready: bool,
    pub candidate_generation_ready: bool,
    pub graph_ready: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub default_chat_unchanged: bool,
    pub ordinary_chat_entrypoint_unchanged: bool,
    pub candidate_count: usize,
    pub suppressed_candidate_count: usize,
    pub blocked_cluster_count: usize,
    pub supported_domain_count: usize,
    pub high_risk_cluster_count: usize,
    pub unsupported_cluster_count: usize,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub candidates: Vec<MaturationEngineCandidate>,
    pub suppressed_candidates: Vec<MaturationCandidateSuppressionReport>,
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

pub fn evaluate_maturation_engine_v1(input: MaturationEngineV1Input) -> MaturationEngineV1Report {
    let graph = &input.evidence_graph;
    let mut blocking_reasons = Vec::new();
    let default_chat_unchanged =
        input.default_chat_selected_adapter_path == DEFAULT_CHAT_KERNEL_PATH;
    let ordinary_chat_entrypoint_unchanged = !input.ordinary_chat_auto_maturation_enabled;

    if !graph.graph_ready {
        push_unique_reason(&mut blocking_reasons, "evidence_graph_not_ready");
    }
    if !graph.metadata_safe || graph.contains_raw_content {
        push_unique_reason(&mut blocking_reasons, "evidence_graph_metadata_not_safe");
    }
    if !default_chat_unchanged {
        push_unique_reason(
            &mut blocking_reasons,
            "default_chat_route_migration_assumed",
        );
    }
    if !ordinary_chat_entrypoint_unchanged {
        push_unique_reason(
            &mut blocking_reasons,
            "ordinary_chat_auto_maturation_assumed",
        );
    }
    if input.require_direct_life_model_write {
        push_unique_reason(&mut blocking_reasons, "direct_lifemodel_write_required");
    }
    if input.require_direct_memory_write {
        push_unique_reason(&mut blocking_reasons, "direct_memory_write_required");
    }
    if input.require_heuristic_activation {
        push_unique_reason(&mut blocking_reasons, "heuristic_activation_required");
    }

    let mut candidates = Vec::new();
    let mut suppressed_candidates = Vec::new();
    let mut supported_domain_count = 0usize;
    let mut high_risk_cluster_count = 0usize;
    let mut unsupported_cluster_count = 0usize;
    let timeline_items = graph.timeline.items.clone();

    let mut clusters = graph.clusters.clone();
    clusters.sort_by(|a, b| {
        a.affected_path
            .cmp(&b.affected_path)
            .then_with(|| a.cluster_id.cmp(&b.cluster_id))
    });

    for cluster in &clusters {
        let cluster_items = timeline_items_for_cluster(&timeline_items, &cluster.cluster_id);
        let high_risk = cluster_is_high_risk(cluster, &cluster_items);
        let domain = if high_risk {
            None
        } else {
            supported_candidate_domain(&cluster.affected_path)
        };
        if high_risk {
            high_risk_cluster_count += 1;
        } else if domain.is_some() {
            supported_domain_count += 1;
        } else {
            unsupported_cluster_count += 1;
        }

        let suppression =
            evaluate_cluster_suppression(cluster, &cluster_items, domain, high_risk, &input);
        if suppression.suppressed {
            suppressed_candidates.push(suppression);
            continue;
        }

        let Some(domain) = domain else {
            suppressed_candidates.push(suppression);
            continue;
        };
        candidates.push(candidate_from_cluster(
            cluster,
            &cluster_items,
            domain,
            &input,
        ));
    }

    candidates.sort_by(|a, b| {
        a.domain
            .cmp(&b.domain)
            .then_with(|| a.affected_path.cmp(&b.affected_path))
            .then_with(|| a.candidate_id.cmp(&b.candidate_id))
    });
    suppressed_candidates.sort_by(|a, b| {
        a.affected_path
            .cmp(&b.affected_path)
            .then_with(|| a.source_cluster_id.cmp(&b.source_cluster_id))
    });

    if candidates.is_empty() {
        if high_risk_cluster_count > 0 {
            push_unique_reason(&mut blocking_reasons, "high_risk_domain_cluster_present");
        }
        push_unique_reason(
            &mut blocking_reasons,
            "low_risk_supported_candidate_missing",
        );
    }

    let metadata_safe = graph.metadata_safe && !graph.contains_raw_content;
    let candidate_generation_ready =
        blocking_reasons.is_empty() && metadata_safe && !candidates.is_empty();
    MaturationEngineV1Report {
        report_kind: MATURATION_ENGINE_V1_REPORT_KIND.into(),
        engine_ready: candidate_generation_ready,
        candidate_generation_ready,
        graph_ready: graph.graph_ready,
        metadata_safe,
        contains_raw_content: graph.contains_raw_content,
        default_chat_unchanged,
        ordinary_chat_entrypoint_unchanged,
        candidate_count: candidates.len(),
        suppressed_candidate_count: suppressed_candidates.len(),
        blocked_cluster_count: suppressed_candidates
            .iter()
            .filter(|candidate| candidate.suppressed)
            .count(),
        supported_domain_count,
        high_risk_cluster_count,
        unsupported_cluster_count,
        generated_at: graph.generated_at,
        candidates,
        suppressed_candidates,
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
        ran_runtime: false,
        ran_model: false,
        ran_tool: false,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturationInput {
    pub run_id: Option<String>,
    pub user_text: String,
    pub assistant_output: String,
    pub life_event_candidates: Vec<LifeEventDraft>,
    pub accepted_proposal_ids: Vec<String>,
    pub rejected_proposal_ids: Vec<String>,
}

impl MaturationInput {
    pub fn from_runtime_output(
        user_text: impl Into<String>,
        output: &RuntimeOutput,
        accepted_proposal_ids: Vec<String>,
        rejected_proposal_ids: Vec<String>,
    ) -> Self {
        Self {
            run_id: output.run_id.clone(),
            user_text: user_text.into(),
            assistant_output: output.user_output.clone(),
            life_event_candidates: output.life_event_candidates.clone(),
            accepted_proposal_ids,
            rejected_proposal_ids,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturationOutput {
    pub proposal_candidates: Vec<MaturationProposalCandidate>,
    pub dropped_reasons: Vec<MaturationDropReason>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturationDropReason {
    pub reason_code: String,
    pub candidate_digest: String,
    pub source_run_id: Option<String>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturationProposalCandidate {
    pub proposal_type: ProposalType,
    pub affected_path: String,
    pub payload: Value,
    pub reason: String,
    pub confidence: f32,
    pub risk_level: RiskLevel,
    pub source_run_id: Option<String>,
    pub source_event_type: String,
    pub proposal_only: bool,
}

impl MaturationProposalCandidate {
    pub fn to_agent_proposal(&self) -> AgentProposal {
        let source = match self.proposal_type {
            ProposalType::MemoryWrite | ProposalType::MemoryArchive => {
                ProposalSource::MemoryGovernance
            }
            _ => ProposalSource::FeedbackEvolution,
        };
        let mut proposal = AgentProposal::new(
            self.proposal_type,
            &self.affected_path,
            self.payload.clone(),
            &self.reason,
            self.confidence,
            self.risk_level,
            source,
        );
        proposal.run_id = self.source_run_id.clone();
        proposal.source_detail = Some(format!("maturation:{}", self.source_event_type));
        proposal
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturationReport {
    pub source_run_id: Option<String>,
    pub candidate_count: usize,
    pub evidence_ids: Vec<String>,
    pub proposal_ids: Vec<String>,
    pub dropped_reasons: Vec<MaturationDropReason>,
    pub governance_summary: MaturationGovernanceSummary,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturationGovernanceSummary {
    pub blocked_count: usize,
    pub confirm_required_count: usize,
    pub proposal_only_count: usize,
    pub decisions: Vec<MaturationGovernanceAudit>,
}

impl MaturationGovernanceSummary {
    fn push(&mut self, audit: MaturationGovernanceAudit) {
        if audit.decision_kind == GovernanceDecisionKind::Block {
            self.blocked_count += 1;
        }
        if audit.decision_kind == GovernanceDecisionKind::RequireConfirmation {
            self.confirm_required_count += 1;
        }
        if audit.proposal_only {
            self.proposal_only_count += 1;
        }
        self.decisions.push(audit);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturationGovernanceAudit {
    pub candidate_digest: String,
    pub source_run_id: Option<String>,
    pub proposal_type: ProposalType,
    pub affected_path: String,
    pub risk_level: RiskLevel,
    pub decision_kind: GovernanceDecisionKind,
    pub reason_code: String,
    pub proposal_only: bool,
}

impl MaturationGovernanceAudit {
    fn from_decision(
        candidate: &MaturationProposalCandidate,
        candidate_digest: String,
        decision: &GovernanceDecision,
    ) -> Self {
        Self {
            candidate_digest,
            source_run_id: candidate.source_run_id.clone(),
            proposal_type: candidate.proposal_type,
            affected_path: candidate.affected_path.clone(),
            risk_level: decision.risk_level,
            decision_kind: decision.kind,
            reason_code: governance_reason_code(decision).to_string(),
            proposal_only: candidate.proposal_only,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MaturationService {
    candidate_service: LifeModelMaturationService,
    governor: LifeModelGovernor,
}

impl MaturationService {
    pub fn with_governor(governor: LifeModelGovernor) -> Self {
        Self {
            candidate_service: LifeModelMaturationService::default(),
            governor,
        }
    }

    pub fn mature_runtime_output(
        &self,
        output: &RuntimeOutput,
        evidence_store: &EvidenceStore,
        proposal_store: &ProposalStore,
    ) -> Result<MaturationReport> {
        let maturation_output =
            self.candidate_service
                .mature(MaturationInput::from_runtime_output(
                    "",
                    output,
                    Vec::new(),
                    Vec::new(),
                ));
        let mut report = MaturationReport {
            source_run_id: output.run_id.clone(),
            candidate_count: output.life_event_candidates.len(),
            dropped_reasons: maturation_output.dropped_reasons,
            ..MaturationReport::default()
        };

        for candidate in maturation_output.proposal_candidates {
            let candidate_digest = proposal_candidate_digest(&candidate);
            let decision = self.governor.govern_maturation_candidate(&candidate);
            let audit = MaturationGovernanceAudit::from_decision(
                &candidate,
                candidate_digest.clone(),
                &decision,
            );
            report.governance_summary.push(audit);

            let linked_proposal_id = if decision.kind == GovernanceDecisionKind::Block {
                None
            } else {
                let proposal = candidate.to_agent_proposal();
                proposal_store.create_proposal(&proposal)?;
                report.proposal_ids.push(proposal.id.clone());
                Some(proposal.id)
            };

            let evidence = evidence_store.create_evidence(evidence_draft_from_candidate(
                &candidate,
                &candidate_digest,
                &decision,
                linked_proposal_id.as_deref(),
            ))?;
            report.evidence_ids.push(evidence.id);
        }

        Ok(report)
    }
}

#[derive(Debug, Clone)]
pub struct LifeModelMaturationService {
    min_confidence: f32,
    min_summary_chars: usize,
}

impl Default for LifeModelMaturationService {
    fn default() -> Self {
        Self {
            min_confidence: MIN_CONFIDENCE,
            min_summary_chars: MIN_SUMMARY_CHARS,
        }
    }
}

impl LifeModelMaturationService {
    pub fn mature(&self, input: MaturationInput) -> MaturationOutput {
        let mut output = MaturationOutput::default();
        let mut seen = HashSet::new();

        for draft in input.life_event_candidates {
            let source_run_id = draft.source_run_id.clone().or_else(|| input.run_id.clone());
            let candidate_digest = draft_digest(&draft, source_run_id.as_deref());
            let summary = normalize_summary(&draft.summary);
            if summary.is_empty() {
                output.dropped_reasons.push(drop_reason(
                    "empty_candidate",
                    candidate_digest,
                    source_run_id,
                    confidence_from_metadata(&draft.metadata),
                ));
                output.warnings.push(format!(
                    "dropped empty LifeEventDraft '{}'",
                    draft.event_type
                ));
                continue;
            }
            if summary.chars().count() < self.min_summary_chars {
                output.dropped_reasons.push(drop_reason(
                    "too_short_candidate",
                    candidate_digest,
                    source_run_id,
                    confidence_from_metadata(&draft.metadata),
                ));
                output.warnings.push(format!(
                    "dropped too-short LifeEventDraft '{}'",
                    draft.event_type
                ));
                continue;
            }

            let confidence =
                confidence_from_metadata(&draft.metadata).unwrap_or(DEFAULT_CONFIDENCE);
            if confidence < self.min_confidence {
                output.dropped_reasons.push(drop_reason(
                    "low_confidence",
                    candidate_digest,
                    source_run_id,
                    Some(confidence),
                ));
                output.warnings.push(format!(
                    "dropped low-confidence LifeEventDraft '{}' ({:.2})",
                    draft.event_type, confidence
                ));
                continue;
            }

            let dedupe_key = dedupe_key(source_run_id.as_deref(), &draft.event_type, &summary);
            if !seen.insert(dedupe_key) {
                output.dropped_reasons.push(drop_reason(
                    "duplicate_candidate",
                    candidate_digest,
                    source_run_id,
                    Some(confidence),
                ));
                output.warnings.push(format!(
                    "dropped duplicate LifeEventDraft '{}'",
                    draft.event_type
                ));
                continue;
            }

            match candidate_from_draft(&draft.event_type, &summary, confidence, source_run_id) {
                Some(mut candidate) => {
                    if let Some(proposal_only) = proposal_only_from_metadata(&draft.metadata) {
                        candidate.proposal_only = proposal_only;
                    }
                    output.proposal_candidates.push(candidate);
                }
                None => {
                    output.dropped_reasons.push(drop_reason(
                        "unsupported_candidate_type",
                        candidate_digest,
                        draft.source_run_id.clone().or_else(|| input.run_id.clone()),
                        Some(confidence),
                    ));
                    output.warnings.push(format!(
                        "unsupported LifeEventDraft type '{}'",
                        draft.event_type
                    ));
                }
            }
        }

        output
    }
}

fn drop_reason(
    reason_code: &str,
    candidate_digest: String,
    source_run_id: Option<String>,
    confidence: Option<f32>,
) -> MaturationDropReason {
    MaturationDropReason {
        reason_code: reason_code.to_string(),
        candidate_digest,
        source_run_id,
        confidence,
    }
}

fn evidence_draft_from_candidate(
    candidate: &MaturationProposalCandidate,
    candidate_digest: &str,
    decision: &GovernanceDecision,
    linked_proposal_id: Option<&str>,
) -> EvidenceDraft {
    let source_id = candidate
        .source_run_id
        .as_deref()
        .unwrap_or("runtime-output");
    let source_ref = EvidenceSourceRef::from_digest(
        EvidenceSourceType::AgentRun,
        source_id,
        Some("maturation_candidate"),
        candidate_digest,
    );
    let mut draft = EvidenceDraft::new(
        evidence_type_for_proposal(candidate.proposal_type),
        candidate.affected_path.clone(),
        candidate.confidence,
        decision.risk_level,
        privacy_from_risk(decision.risk_level),
    )
    .with_summary(format!(
        "{} maturation candidate for {}",
        candidate.proposal_type, candidate.affected_path
    ))
    .with_source_ref(source_ref);

    if let Some(proposal_id) = linked_proposal_id {
        draft = draft.with_linked_proposal(proposal_id);
    }
    if let Some(source_run_id) = candidate.source_run_id.as_deref() {
        draft = draft.with_linked_agent_run(source_run_id);
    }

    draft.run_metadata = json!({
        "candidateDigest": candidate_digest,
        "sourceRunId": candidate.source_run_id,
        "confidence": candidate.confidence,
        "risk": decision.risk_level.to_string(),
        "path": candidate.affected_path,
        "proposalType": candidate.proposal_type.to_string(),
        "reasonCode": governance_reason_code(decision),
        "governanceDecision": decision.kind,
        "proposalOnly": candidate.proposal_only,
    });
    draft
}

fn evidence_type_for_proposal(proposal_type: ProposalType) -> EvidenceType {
    match proposal_type {
        ProposalType::GoalUpdate => EvidenceType::Goal,
        ProposalType::StateUpdate => EvidenceType::State,
        ProposalType::PreferenceUpdate => EvidenceType::Preference,
        ProposalType::CapabilityUpdate => EvidenceType::Capability,
        ProposalType::MemoryWrite | ProposalType::MemoryArchive => EvidenceType::Memory,
        ProposalType::ToolPermission
        | ProposalType::PluginPermission
        | ProposalType::ModelPolicyChange
        | ProposalType::DataExport => EvidenceType::Policy,
        ProposalType::ScheduledTask | ProposalType::ExternalWriteAction => {
            EvidenceType::RuntimeBehavior
        }
        ProposalType::ScheduleCheckin => EvidenceType::State,
        ProposalType::Unsupported | ProposalType::LifeModelUpdate => EvidenceType::Other,
    }
}

fn privacy_from_risk(risk_level: RiskLevel) -> EvidencePrivacyLevel {
    match risk_level {
        RiskLevel::Low => EvidencePrivacyLevel::Internal,
        RiskLevel::Medium => EvidencePrivacyLevel::Sensitive,
        RiskLevel::High | RiskLevel::Critical => EvidencePrivacyLevel::StrictlyLocal,
    }
}

fn governance_reason_code(decision: &GovernanceDecision) -> &str {
    decision
        .metadata_safe_summary
        .get("policyReasonCode")
        .and_then(Value::as_str)
        .unwrap_or("unknown_governance_reason")
}

fn draft_digest(draft: &LifeEventDraft, source_run_id: Option<&str>) -> String {
    sha256_hex(
        json!({
            "eventType": draft.event_type,
            "summary": draft.summary,
            "sourceRunId": source_run_id,
            "confidence": confidence_from_metadata(&draft.metadata),
        })
        .to_string()
        .as_bytes(),
    )
}

fn proposal_candidate_digest(candidate: &MaturationProposalCandidate) -> String {
    sha256_hex(
        json!({
            "proposalType": candidate.proposal_type.to_string(),
            "affectedPath": candidate.affected_path,
            "payload": candidate.payload,
            "reason": candidate.reason,
            "confidence": candidate.confidence,
            "riskLevel": candidate.risk_level.to_string(),
            "sourceRunId": candidate.source_run_id,
            "sourceEventType": candidate.source_event_type,
            "proposalOnly": candidate.proposal_only,
        })
        .to_string()
        .as_bytes(),
    )
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

fn candidate_from_draft(
    event_type: &str,
    summary: &str,
    confidence: f32,
    source_run_id: Option<String>,
) -> Option<MaturationProposalCandidate> {
    let combined = searchable(event_type, summary);

    if contains_any(&combined, &["memory", "remember", "记忆", "记住"]) {
        let risk_level = risk_for_memory(&combined);
        return Some(candidate(
            ProposalType::MemoryWrite,
            "memory.candidates",
            json!({
                "content": summary,
                "source": "maturation_life_event",
                "event_type": event_type,
                "confidence": confidence,
            }),
            event_type,
            confidence,
            risk_level,
            source_run_id,
        ));
    }

    if contains_any(&combined, &["relationship", "relationships", "关系"]) {
        return Some(candidate(
            ProposalType::LifeModelUpdate,
            "/relationships",
            lifemodel_payload(summary, event_type),
            event_type,
            confidence,
            RiskLevel::High,
            source_run_id,
        ));
    }

    if contains_any(
        &combined,
        &[
            "identity",
            "value",
            "values",
            "mission",
            "philosophy",
            "身份",
            "价值观",
            "使命",
            "人生哲学",
        ],
    ) {
        return Some(candidate(
            ProposalType::LifeModelUpdate,
            identity_path(&combined),
            lifemodel_payload(summary, event_type),
            event_type,
            confidence,
            RiskLevel::High,
            source_run_id,
        ));
    }

    if contains_any(&combined, &["goal", "goals", "目标"]) {
        let risk_level = if is_long_horizon_goal(&combined) {
            RiskLevel::High
        } else {
            RiskLevel::Medium
        };
        return Some(candidate(
            ProposalType::GoalUpdate,
            goal_path(&combined),
            json!({
                "summary": summary,
                "event_type": event_type,
            }),
            event_type,
            confidence,
            risk_level,
            source_run_id,
        ));
    }

    if contains_any(
        &combined,
        &[
            "state",
            "current_focus",
            "focus",
            "health",
            "financial",
            "finance",
            "状态",
            "当前重心",
            "健康",
            "财务",
        ],
    ) {
        let risk_level = if is_sensitive_state(&combined) {
            RiskLevel::High
        } else {
            RiskLevel::Medium
        };
        return Some(candidate(
            ProposalType::StateUpdate,
            state_path(&combined),
            json!({
                "summary": summary,
                "event_type": event_type,
            }),
            event_type,
            confidence,
            risk_level,
            source_run_id,
        ));
    }

    if contains_any(
        &combined,
        &[
            "preference",
            "preferences",
            "communication",
            "learning",
            "workflow",
            "habit",
            "work_hours",
            "偏好",
            "沟通",
            "学习",
            "工作流",
            "习惯",
        ],
    ) {
        let risk_level = if contains_any(
            &combined,
            &[
                "workflow",
                "habit",
                "work_hours",
                "decision",
                "工作流",
                "习惯",
                "决策",
            ],
        ) {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };
        return Some(candidate(
            ProposalType::PreferenceUpdate,
            preference_path(&combined),
            json!({
                "summary": summary,
                "event_type": event_type,
            }),
            event_type,
            confidence,
            risk_level,
            source_run_id,
        ));
    }

    None
}

fn candidate(
    proposal_type: ProposalType,
    affected_path: &str,
    payload: Value,
    event_type: &str,
    confidence: f32,
    risk_level: RiskLevel,
    source_run_id: Option<String>,
) -> MaturationProposalCandidate {
    MaturationProposalCandidate {
        proposal_type,
        affected_path: affected_path.to_string(),
        payload,
        reason: reason_for(event_type, risk_level),
        confidence,
        risk_level,
        source_run_id,
        source_event_type: event_type.to_string(),
        proposal_only: true,
    }
}

fn lifemodel_payload(summary: &str, event_type: &str) -> Value {
    json!({
        "summary": summary,
        "event_type": event_type,
    })
}

fn reason_for(event_type: &str, risk_level: RiskLevel) -> String {
    let risk_note = match risk_level {
        RiskLevel::Low => "low-risk",
        RiskLevel::Medium => "medium-risk",
        RiskLevel::High | RiskLevel::Critical => "high-risk",
    };
    format!(
        "{} maturation candidate from LifeEventDraft '{}'; user confirmation is required before any LifeModel or MemoryStore write.",
        risk_note, event_type
    )
}

fn confidence_from_metadata(metadata: &Value) -> Option<f32> {
    metadata
        .get("confidence")
        .and_then(Value::as_f64)
        .map(|value| value.clamp(0.0, 1.0) as f32)
}

fn proposal_only_from_metadata(metadata: &Value) -> Option<bool> {
    metadata
        .get("proposal_only")
        .or_else(|| metadata.get("proposalOnly"))
        .and_then(Value::as_bool)
}

fn normalize_summary(summary: &str) -> String {
    summary.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn dedupe_key(run_id: Option<&str>, event_type: &str, summary: &str) -> String {
    format!(
        "{}|{}|{}",
        run_id.unwrap_or(""),
        event_type.trim().to_ascii_lowercase(),
        summary.to_ascii_lowercase()
    )
}

fn searchable(event_type: &str, summary: &str) -> String {
    format!(
        "{} {}",
        event_type.trim().to_ascii_lowercase(),
        summary.to_ascii_lowercase()
    )
}

fn timeline_items_for_cluster(
    timeline_items: &[EvidenceTimelineItem],
    cluster_id: &str,
) -> Vec<EvidenceTimelineItem> {
    let mut items = timeline_items
        .iter()
        .filter(|item| item.cluster_id == cluster_id)
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| {
        a.evidence_id
            .cmp(&b.evidence_id)
            .then_with(|| a.affected_path.cmp(&b.affected_path))
    });
    items
}

fn supported_candidate_domain(path: &str) -> Option<MaturationCandidateDomain> {
    classify_supported_maturation_domain(path, None).map(maturation_domain_for_engine)
}

fn high_risk_maturation_path(path: &str) -> bool {
    high_risk_maturation_text(path)
}

fn maturation_domain_for_engine(domain: SupportedMaturationDomain) -> MaturationCandidateDomain {
    match domain {
        SupportedMaturationDomain::PlanningPreference => {
            MaturationCandidateDomain::PlanningPreference
        }
        SupportedMaturationDomain::EnergyPattern => MaturationCandidateDomain::EnergyPattern,
        SupportedMaturationDomain::WorkStyle => MaturationCandidateDomain::WorkStyle,
        SupportedMaturationDomain::CommunicationPreference => {
            MaturationCandidateDomain::CommunicationPreference
        }
    }
}

fn cluster_is_high_risk(
    cluster: &EvidenceClusterSummary,
    timeline_items: &[EvidenceTimelineItem],
) -> bool {
    high_risk_maturation_path(&cluster.affected_path)
        || timeline_items.iter().any(|item| {
            matches!(item.risk_level.as_str(), "high" | "critical")
                || item.privacy_level == "strictly_local"
        })
}

fn evaluate_cluster_suppression(
    cluster: &EvidenceClusterSummary,
    timeline_items: &[EvidenceTimelineItem],
    domain: Option<MaturationCandidateDomain>,
    high_risk: bool,
    input: &MaturationEngineV1Input,
) -> MaturationCandidateSuppressionReport {
    let mut reasons = Vec::new();
    let support_evidence_ids = sorted_unique(cluster.supporting_evidence_ids.clone());
    let mut opposing_evidence_ids = sorted_unique(cluster.opposing_evidence_ids.clone());
    let rejected_evidence_ids = rejected_evidence_ids_for_cluster(cluster, timeline_items);
    let rejected_proposal_ids = rejected_proposal_ids_for_cluster(cluster, timeline_items);
    let linked_proposal_ids = linked_proposal_ids_for_timeline(timeline_items);
    let linked_agent_run_ids = linked_agent_run_ids_for_timeline(timeline_items);
    let stability_score = cluster_stability_score(cluster);

    if high_risk {
        push_unique_reason(&mut reasons, "high_risk_domain");
    }
    if domain.is_none() && !high_risk {
        push_unique_reason(&mut reasons, "unsupported_maturation_domain");
    }
    if support_evidence_ids.is_empty() {
        push_unique_reason(&mut reasons, "supporting_evidence_missing");
    }
    if cluster.cooldown_state.active {
        push_unique_reason(&mut reasons, "rejected_similar_cooldown_active");
    }
    if cluster.conflict_state.conflicted {
        push_unique_reason(&mut reasons, "cluster_conflict_active");
    }
    if !cluster.conflict_state.opposing_evidence_ids.is_empty() {
        for id in &cluster.conflict_state.opposing_evidence_ids {
            push_unique_string(&mut opposing_evidence_ids, id);
        }
    }
    if !opposing_evidence_ids.is_empty() || cluster.opposition_link_count > 0 {
        push_unique_reason(&mut reasons, "opposing_evidence_present");
    }
    if !rejected_evidence_ids.is_empty() {
        push_unique_reason(&mut reasons, "rejected_similar_history_present");
        if rejected_evidence_ids.len() >= support_evidence_ids.len().max(1) {
            push_unique_reason(&mut reasons, "rejected_similar_history_blocks_candidate");
        }
    }
    if cluster.effective_confidence < input.min_effective_confidence
        || timeline_items
            .iter()
            .filter(|item| item.polarity == EvidencePolarity::Supporting)
            .all(|item| item.decay_state.decayed)
    {
        push_unique_reason(&mut reasons, "supporting_evidence_decayed");
    }
    if stability_score < input.min_stability_score {
        push_unique_reason(&mut reasons, "stability_score_too_low");
    }

    MaturationCandidateSuppressionReport {
        source_cluster_id: cluster.cluster_id.clone(),
        source_cluster_hash: cluster.cluster_hash.clone(),
        affected_path: cluster.affected_path.clone(),
        domain,
        suppressed: !reasons.is_empty(),
        correction_recommended: cluster.conflict_state.conflicted
            || !opposing_evidence_ids.is_empty()
            || !rejected_evidence_ids.is_empty(),
        reasons,
        support_evidence_ids,
        opposing_evidence_ids: sorted_unique(opposing_evidence_ids),
        rejected_evidence_ids,
        rejected_proposal_ids,
        linked_proposal_ids,
        linked_agent_run_ids,
        cooldown_active: cluster.cooldown_state.active,
        conflict_active: cluster.conflict_state.conflicted,
        decayed: cluster.effective_confidence < input.min_effective_confidence,
        rejected_similar_history_count: rejected_evidence_ids_for_cluster(cluster, timeline_items)
            .len(),
        effective_confidence: round4_maturation(cluster.effective_confidence),
        stability_score,
        candidate_digest: engine_candidate_digest(EngineCandidateDigestInput {
            domain,
            affected_path: &cluster.affected_path,
            cluster_hash: &cluster.cluster_hash,
            support_evidence_ids: &cluster.supporting_evidence_ids,
            opposing_evidence_ids: &cluster.opposing_evidence_ids,
            linked_proposal_ids: &linked_proposal_ids_for_timeline(timeline_items),
            linked_agent_run_ids: &linked_agent_run_ids_for_timeline(timeline_items),
            effective_confidence: cluster.effective_confidence,
            stability_score,
        }),
    }
}

fn candidate_from_cluster(
    cluster: &EvidenceClusterSummary,
    timeline_items: &[EvidenceTimelineItem],
    domain: MaturationCandidateDomain,
    _input: &MaturationEngineV1Input,
) -> MaturationEngineCandidate {
    let linked_proposal_ids = linked_proposal_ids_for_timeline(timeline_items);
    let linked_agent_run_ids = linked_agent_run_ids_for_timeline(timeline_items);
    let support_evidence_ids = sorted_unique(cluster.supporting_evidence_ids.clone());
    let opposing_evidence_ids = sorted_unique(cluster.opposing_evidence_ids.clone());
    let stability_score = cluster_stability_score(cluster);
    let candidate_digest = engine_candidate_digest(EngineCandidateDigestInput {
        domain: Some(domain),
        affected_path: &cluster.affected_path,
        cluster_hash: &cluster.cluster_hash,
        support_evidence_ids: &support_evidence_ids,
        opposing_evidence_ids: &opposing_evidence_ids,
        linked_proposal_ids: &linked_proposal_ids,
        linked_agent_run_ids: &linked_agent_run_ids,
        effective_confidence: cluster.effective_confidence,
        stability_score,
    });
    MaturationEngineCandidate {
        candidate_id: format!("mc_{}", short_hash_maturation(&candidate_digest)),
        candidate_digest,
        domain,
        affected_path: cluster.affected_path.clone(),
        proposal_type: proposal_type_for_engine_domain(domain),
        risk_level: RiskLevel::Low,
        source_cluster_id: cluster.cluster_id.clone(),
        source_cluster_hash: cluster.cluster_hash.clone(),
        support_evidence_ids,
        opposing_evidence_ids,
        linked_proposal_ids,
        linked_agent_run_ids,
        source_weight_total: round4_maturation(cluster.source_weight_total),
        confidence: round4_maturation(cluster.effective_confidence.clamp(0.0, 1.0)),
        stability_score,
        proposal_required: true,
        candidate_only: true,
        candidate_summary: engine_candidate_summary(domain).to_string(),
        metadata_safe: true,
        contains_raw_content: false,
    }
}

fn proposal_type_for_engine_domain(domain: MaturationCandidateDomain) -> ProposalType {
    match domain {
        MaturationCandidateDomain::EnergyPattern => ProposalType::StateUpdate,
        MaturationCandidateDomain::PlanningPreference
        | MaturationCandidateDomain::WorkStyle
        | MaturationCandidateDomain::CommunicationPreference => ProposalType::PreferenceUpdate,
    }
}

fn engine_candidate_summary(domain: MaturationCandidateDomain) -> &'static str {
    match domain {
        MaturationCandidateDomain::PlanningPreference => {
            "Reviewable planning preference candidate from evidence cluster."
        }
        MaturationCandidateDomain::EnergyPattern => {
            "Reviewable energy pattern candidate from evidence cluster."
        }
        MaturationCandidateDomain::WorkStyle => {
            "Reviewable work style candidate from evidence cluster."
        }
        MaturationCandidateDomain::CommunicationPreference => {
            "Reviewable communication preference candidate from evidence cluster."
        }
    }
}

fn cluster_stability_score(cluster: &EvidenceClusterSummary) -> f32 {
    let opposition_penalty = (cluster.opposing_evidence_ids.len() as f32 * 0.18)
        + if cluster.conflict_state.conflicted {
            0.25
        } else {
            0.0
        }
        + if cluster.cooldown_state.active {
            0.25
        } else {
            0.0
        };
    round4_maturation((cluster.effective_confidence - opposition_penalty).clamp(0.0, 1.0))
}

fn rejected_evidence_ids_for_cluster(
    cluster: &EvidenceClusterSummary,
    timeline_items: &[EvidenceTimelineItem],
) -> Vec<String> {
    let mut ids = cluster.cooldown_state.rejected_evidence_ids.clone();
    for item in timeline_items {
        if item.evidence_type == "proposal_outcome" && item.polarity == EvidencePolarity::Opposing {
            push_unique_string(&mut ids, &item.evidence_id);
        }
    }
    sorted_unique(ids)
}

fn rejected_proposal_ids_for_cluster(
    cluster: &EvidenceClusterSummary,
    timeline_items: &[EvidenceTimelineItem],
) -> Vec<String> {
    let mut ids = cluster.cooldown_state.rejected_proposal_ids.clone();
    for item in timeline_items {
        if item.evidence_type == "proposal_outcome" && item.polarity == EvidencePolarity::Opposing {
            for proposal_id in &item.linked_proposal_ids {
                push_unique_string(&mut ids, proposal_id);
            }
        }
    }
    sorted_unique(ids)
}

fn linked_proposal_ids_for_timeline(timeline_items: &[EvidenceTimelineItem]) -> Vec<String> {
    let mut ids = Vec::new();
    for item in timeline_items {
        for proposal_id in &item.linked_proposal_ids {
            push_unique_string(&mut ids, proposal_id);
        }
    }
    sorted_unique(ids)
}

fn linked_agent_run_ids_for_timeline(timeline_items: &[EvidenceTimelineItem]) -> Vec<String> {
    let mut ids = Vec::new();
    for item in timeline_items {
        for run_id in &item.linked_agent_run_ids {
            push_unique_string(&mut ids, run_id);
        }
    }
    sorted_unique(ids)
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

struct EngineCandidateDigestInput<'a> {
    domain: Option<MaturationCandidateDomain>,
    affected_path: &'a str,
    cluster_hash: &'a str,
    support_evidence_ids: &'a [String],
    opposing_evidence_ids: &'a [String],
    linked_proposal_ids: &'a [String],
    linked_agent_run_ids: &'a [String],
    effective_confidence: f32,
    stability_score: f32,
}

fn engine_candidate_digest(input: EngineCandidateDigestInput<'_>) -> String {
    sha256_hex(
        json!({
            "schema": "w131.maturationEngineCandidate.digest.v1",
            "domain": input.domain.map(|domain| domain.as_str()),
            "affectedPath": input.affected_path,
            "sourceClusterHash": input.cluster_hash,
            "supportEvidenceIds": sorted_unique(input.support_evidence_ids.to_vec()),
            "opposingEvidenceIds": sorted_unique(input.opposing_evidence_ids.to_vec()),
            "linkedProposalIds": sorted_unique(input.linked_proposal_ids.to_vec()),
            "linkedAgentRunIds": sorted_unique(input.linked_agent_run_ids.to_vec()),
            "effectiveConfidence": round4_maturation(input.effective_confidence),
            "stabilityScore": input.stability_score,
        })
        .to_string()
        .as_bytes(),
    )
}

fn round4_maturation(value: f32) -> f32 {
    (value * 10_000.0).round() / 10_000.0
}

fn short_hash_maturation(value: &str) -> String {
    sha256_hex(value.as_bytes()).chars().take(16).collect()
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn type_available<T>() -> bool {
    !std::any::type_name::<T>().is_empty()
}

fn push_unique_reason(reasons: &mut Vec<String>, reason: &str) {
    if !reasons.iter().any(|existing| existing == reason) {
        reasons.push(reason.to_string());
    }
}

fn push_unique_string(values: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if !value.is_empty() && !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn metadata_string_array(metadata: &Value, key: &str) -> Vec<String> {
    metadata
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn is_low_energy_rule_candidate_domain(domain: &str) -> bool {
    matches!(
        domain.trim().to_ascii_lowercase().as_str(),
        "low_energy_planning"
            | "low-pressure-planning"
            | "low_pressure_planning"
            | "low_energy_collaboration"
            | "low_pressure_collaboration"
    )
}

fn is_maturation_proposal_outcome_record(record: &EvidenceRecord) -> bool {
    let schema_ok = record
        .run_metadata
        .get("schema")
        .and_then(Value::as_str)
        .is_some_and(|schema| schema == "w75.maturationProposalOutcomeEvidence.v1")
        || record
            .run_metadata
            .get("w75")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let source_detail_maturation = record
        .run_metadata
        .get("sourceDetailMaturation")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    schema_ok && source_detail_maturation
}

fn outcome_record_in_low_energy_collaboration_scope(record: &EvidenceRecord) -> bool {
    let path = record.affected_path.trim().to_ascii_lowercase();
    path.starts_with("/preferences")
        || path.starts_with("preferences")
        || path.starts_with("/collaboration")
        || path.starts_with("collaboration")
        || contains_any(
            &path,
            &[
                "low_energy",
                "low-energy",
                "low_pressure",
                "low-pressure",
                "planning",
            ],
        )
}

fn outcome_record_contains_raw_content(record: &EvidenceRecord) -> bool {
    outcome_metadata_contains_raw_content(&record.run_metadata)
}

fn outcome_metadata_contains_raw_content(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            let normalized = normalized_metadata_key(key);
            if raw_presence_flag_key(&normalized) {
                return value.as_bool().unwrap_or(true);
            }
            raw_outcome_payload_key(&normalized) || outcome_metadata_contains_raw_content(value)
        }),
        Value::Array(values) => values.iter().any(outcome_metadata_contains_raw_content),
        Value::String(value) => string_looks_secret_like(value),
        _ => false,
    }
}

fn normalized_metadata_key(key: &str) -> String {
    key.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn raw_presence_flag_key(normalized: &str) -> bool {
    matches!(
        normalized,
        "rawpromptincluded"
            | "assistantoutputincluded"
            | "memoryrawtextincluded"
            | "toolpayloadincluded"
            | "secretincluded"
            | "editedpayloadincluded"
            | "reviewernoteincluded"
    )
}

fn raw_outcome_payload_key(normalized: &str) -> bool {
    matches!(
        normalized,
        "rawprompt"
            | "prompt"
            | "rawassistantoutput"
            | "assistantoutput"
            | "rawmemorytext"
            | "memoryrawtext"
            | "rawmemorycontext"
            | "memorycontext"
            | "toolpayload"
            | "secret"
            | "apikey"
            | "token"
            | "password"
            | "raweditedpayload"
            | "editedpayload"
            | "payloadraw"
            | "rawpayload"
    )
}

fn low_energy_candidate_confidence(support_count: usize, opposing_count: usize) -> f32 {
    let confidence = 0.75 + (support_count as f32 * 0.05) - (opposing_count as f32 * 0.2);
    confidence.clamp(0.2, 0.95)
}

fn low_energy_candidate_rule_digest(
    target_domain: &str,
    candidate_rule_id: &str,
    accepted_ids: &[String],
    edited_ids: &[String],
    rejected_ids: &[String],
) -> String {
    sha256_hex(
        json!({
            "schema": "w76.lowEnergyCollaborationRuleCandidate.digest.v1",
            "targetDomain": target_domain,
            "candidateRuleId": candidate_rule_id,
            "acceptedOutcomeEvidenceIds": accepted_ids,
            "editedOutcomeEvidenceIds": edited_ids,
            "rejectedOutcomeEvidenceIds": rejected_ids,
        })
        .to_string()
        .as_bytes(),
    )
}

fn low_energy_candidate_proposal_payload(
    report: &LowEnergyCollaborationRuleCandidateReport,
) -> Value {
    json!({
        "schema": "w76.lowEnergyCollaborationRuleCandidate.v1",
        "w76": true,
        "kind": "collaboration_rule_candidate",
        "candidateOnly": true,
        "reviewRequired": true,
        "activatesHeuristic": false,
        "writesActiveRule": false,
        "heuristicActivationAllowed": false,
        "candidateRuleId": report.candidate_rule_id,
        "candidateRuleDigest": report.candidate_rule_digest,
        "targetDomain": report.target_domain,
        "ruleSummary": report.candidate_rule_summary,
        "confidence": report.candidate_confidence,
        "weakenedByOpposingOutcome": report.weakened_by_opposing_outcome,
        "metadataSafe": true,
        "containsRawContent": false,
        "rawPromptIncluded": false,
        "assistantOutputIncluded": false,
        "memoryRawTextIncluded": false,
        "toolPayloadIncluded": false,
        "secretIncluded": false,
        "editedPayloadIncluded": false,
        "sourceLineage": {
            "acceptedOutcomeEvidenceIds": report.accepted_outcome_evidence_ids,
            "rejectedOutcomeEvidenceIds": report.rejected_outcome_evidence_ids,
            "editedOutcomeEvidenceIds": report.edited_outcome_evidence_ids,
            "opposingOutcomeEvidenceIds": report.opposing_outcome_evidence_ids,
            "sourceEvidenceIds": report.source_evidence_ids,
            "linkedProposalIds": report.linked_proposal_ids,
            "linkedAgentRunIds": report.linked_agent_run_ids,
        },
    })
}

fn is_w76_low_energy_rule_candidate_proposal(proposal: &AgentProposal) -> bool {
    proposal.proposal_type == ProposalType::Unsupported
        && proposal.source == ProposalSource::FeedbackEvolution
        && proposal.source_detail.as_deref() == Some(LOW_ENERGY_RULE_CANDIDATE_SOURCE_DETAIL)
        && proposal.affected_path == LOW_ENERGY_RULE_CANDIDATE_PATH
        && proposal
            .after
            .get("schema")
            .and_then(Value::as_str)
            .is_some_and(|schema| schema == "w76.lowEnergyCollaborationRuleCandidate.v1")
        && proposal
            .after
            .get("w76")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && proposal
            .after
            .get("kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "collaboration_rule_candidate")
        && proposal
            .after
            .get("candidateRuleId")
            .and_then(Value::as_str)
            .is_some_and(|rule_id| rule_id == BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING)
}

fn candidate_source_outcome_evidence_ids(candidate_after: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    for key in [
        "acceptedOutcomeEvidenceIds",
        "editedOutcomeEvidenceIds",
        "rejectedOutcomeEvidenceIds",
        "opposingOutcomeEvidenceIds",
    ] {
        for id in candidate_source_lineage_string_array(candidate_after, key) {
            push_unique_string(&mut ids, &id);
        }
    }
    ids
}

fn candidate_source_lineage_string_array(candidate_after: &Value, key: &str) -> Vec<String> {
    candidate_after
        .get("sourceLineage")
        .map(|lineage| metadata_string_array(lineage, key))
        .unwrap_or_default()
}

fn selected_policy_ids_for_selection(
    packet: Option<&RuntimeHSPacket>,
    privacy_topic: PolicyTopic,
    current_route_policy: ModelRoutePolicy,
) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(packet) = packet {
        for policy in &packet.selected_policies {
            push_unique_string(&mut ids, &policy.policy_id);
        }
        for policy_id in &packet.audit.selected_policy_ids {
            push_unique_string(&mut ids, policy_id);
        }
    }
    if selection_requires_local_only(packet, privacy_topic, current_route_policy)
        && topic_requires_local_only(privacy_topic)
    {
        push_unique_string(&mut ids, BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY);
    }
    ids
}

fn enforced_selection_route_policy(
    packet: Option<&RuntimeHSPacket>,
    privacy_topic: PolicyTopic,
    current_route_policy: ModelRoutePolicy,
) -> ModelRoutePolicy {
    if selection_requires_local_only(packet, privacy_topic, current_route_policy) {
        ModelRoutePolicy::LocalOnly
    } else {
        current_route_policy
    }
}

fn selection_requires_local_only(
    packet: Option<&RuntimeHSPacket>,
    privacy_topic: PolicyTopic,
    current_route_policy: ModelRoutePolicy,
) -> bool {
    topic_requires_local_only(privacy_topic)
        || current_route_policy == ModelRoutePolicy::LocalOnly
        || packet_requires_local_only_route(packet)
}

fn topic_requires_local_only(topic: PolicyTopic) -> bool {
    matches!(
        topic,
        PolicyTopic::Health
            | PolicyTopic::Relationship
            | PolicyTopic::Identity
            | PolicyTopic::Finance
            | PolicyTopic::PrivateFile
    )
}

fn packet_requires_local_only_route(packet: Option<&RuntimeHSPacket>) -> bool {
    packet.is_some_and(|packet| {
        packet.selected_policies.iter().any(|policy| {
            policy.policy_id == BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY
                || policy.route == Some(ModelRoutePolicy::LocalOnly)
        }) || packet
            .audit
            .selected_policy_ids
            .iter()
            .any(|policy_id| policy_id == BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY)
    })
}

fn candidate_attempts_privacy_route_override(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            let normalized = normalized_metadata_key(key);
            if privacy_route_override_key(&normalized) {
                return route_override_value_relaxes_privacy(value);
            }
            candidate_attempts_privacy_route_override(value)
        }),
        Value::Array(values) => values.iter().any(candidate_attempts_privacy_route_override),
        _ => false,
    }
}

fn privacy_route_override_key(normalized: &str) -> bool {
    contains_any(
        normalized,
        &[
            "routepolicy",
            "modelroute",
            "requestedroute",
            "privacypolicy",
            "privacyroute",
            "privacypolicyrelaxed",
        ],
    )
}

fn route_override_value_relaxes_privacy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::String(value) => {
            let lower = value.to_ascii_lowercase();
            contains_any(
                &lower,
                &[
                    "cloud",
                    "cloud_allowed",
                    "cloudallowed",
                    "remote",
                    "relax",
                    "override",
                    "bypass",
                ],
            )
        }
        Value::Object(_) | Value::Array(_) => candidate_attempts_privacy_route_override(value),
        _ => false,
    }
}

fn guidance_summary_relaxes_privacy_policy(summary: &str) -> bool {
    let lower = summary.to_ascii_lowercase();
    contains_any(
        &lower,
        &[
            "cloud allowed",
            "use cloud",
            "remote model",
            "ignore privacy",
            "override privacy",
            "relax privacy",
            "bypass local",
            "disable local-only",
        ],
    )
}

fn low_energy_rule_trace_metadata(
    selection_report: &AcceptedLowEnergyRuleSelectionReport,
    trace_payload_hash: Option<String>,
) -> LowEnergyRuleTraceMetadata {
    let selected_guidance_hash = selection_report
        .selected_guidance_summary
        .as_deref()
        .map(|summary| sha256_hex(summary.as_bytes()));
    let selected_candidate_proposal_hash = selection_report
        .selected_candidate_proposal_id
        .as_deref()
        .map(|id| sha256_hex(id.as_bytes()));
    let runtime_hs_packet_guidance_hash =
        selection_report
            .selected_guidance_summary
            .as_ref()
            .map(|summary| {
                sha256_hex(
                    json!({
                        "schema": "w78.runtimeHSPacketGuidanceTrace.v1",
                        "selectedGuidanceSummary": summary,
                        "selectedCandidateProposalId": selection_report.selected_candidate_proposal_id.as_deref(),
                        "selectedCandidateRuleDigest": selection_report.selected_candidate_rule_digest.as_deref(),
                        "sourceOutcomeEvidenceIds": &selection_report.source_outcome_evidence_ids,
                        "sourceProposalIds": &selection_report.source_proposal_ids,
                        "sourceAgentRunIds": &selection_report.source_agent_run_ids,
                        "selectedPolicyIds": &selection_report.hs_packet_audit_proof.selected_policy_ids,
                        "enforcedRoutePolicy": selection_report.enforced_route_policy,
                    })
                    .to_string()
                    .as_bytes(),
                )
            });
    let selection_report_json =
        serde_json::to_string(selection_report).unwrap_or_else(|_| "unserializable".into());
    let trace_metadata_fields = vec![
        "selectedGuidanceSummary",
        "selectedGuidanceHash",
        "runtimeHSPacketGuidanceHash",
        "selectedCandidateProposalId",
        "selectedCandidateProposalHash",
        "selectedCandidateRuleDigest",
        "selectionReportHash",
        "tracePayloadHash",
        "targetTaskKind",
        "targetDomain",
        "privacyTopic",
        "enforcedRoutePolicy",
        "selectedPolicyIds",
        "selectedPolicyCount",
        "evidenceLineage.items",
        "evidenceLineage.count",
        "evidenceLineage.idsHash",
        "proposalLineage.items",
        "proposalLineage.count",
        "proposalLineage.idsHash",
        "agentRunLineage.items",
        "agentRunLineage.count",
        "agentRunLineage.idsHash",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();

    LowEnergyRuleTraceMetadata {
        schema: "w78.lowEnergyRuleTraceVisibilityMetadata.v1".into(),
        trace_kind: "future_runtime_trace_visibility_contract".into(),
        trace_metadata_field_count: trace_metadata_fields.len(),
        trace_metadata_fields,
        selection_report_hash: sha256_hex(selection_report_json.as_bytes()),
        trace_payload_hash,
        selected_guidance_summary: selection_report.selected_guidance_summary.clone(),
        selected_guidance_hash,
        runtime_hs_packet_guidance_hash,
        selected_candidate_proposal_id: selection_report.selected_candidate_proposal_id.clone(),
        selected_candidate_proposal_hash,
        selected_candidate_rule_digest: selection_report.selected_candidate_rule_digest.clone(),
        target_task_kind: selection_report.target_task_kind,
        target_domain: selection_report.target_domain.clone(),
        privacy_topic: selection_report.privacy_topic,
        enforced_route_policy: selection_report.enforced_route_policy,
        selected_policy_count: selection_report
            .hs_packet_audit_proof
            .selected_policy_ids
            .len(),
        selected_policy_ids: selection_report
            .hs_packet_audit_proof
            .selected_policy_ids
            .clone(),
        evidence_lineage: low_energy_rule_trace_lineage_summary(
            &selection_report.source_outcome_evidence_ids,
            "proposal_outcome_evidence",
            "selected",
        ),
        proposal_lineage: low_energy_rule_trace_lineage_summary(
            &selection_report.source_proposal_ids,
            "proposal",
            "source_linked",
        ),
        agent_run_lineage: low_energy_rule_trace_lineage_summary(
            &selection_report.source_agent_run_ids,
            "agent_run",
            "source_linked",
        ),
    }
}

fn low_energy_rule_trace_lineage_summary(
    ids: &[String],
    record_type: &str,
    status: &str,
) -> LowEnergyRuleTraceLineageSummary {
    let items = ids
        .iter()
        .map(|id| LowEnergyRuleTraceLineageItem {
            id: id.clone(),
            id_hash: sha256_hex(id.as_bytes()),
            record_type: record_type.into(),
            status: status.into(),
        })
        .collect::<Vec<_>>();
    LowEnergyRuleTraceLineageSummary {
        count: items.len(),
        ids_hash: sha256_hex(
            json!({
                "recordType": record_type,
                "status": status,
                "ids": ids,
            })
            .to_string()
            .as_bytes(),
        ),
        items,
    }
}

fn trace_payload_metadata_not_safe(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            let normalized = normalized_metadata_key(key);
            matches!(normalized.as_str(), "metadatasafe" | "tracemetadatasafe")
                && value.as_bool() == Some(false)
                || matches!(
                    normalized.as_str(),
                    "containsrawcontent" | "tracecontainsrawcontent"
                ) && value.as_bool() == Some(true)
                || trace_payload_metadata_not_safe(value)
        }),
        Value::Array(values) => values.iter().any(trace_payload_metadata_not_safe),
        _ => false,
    }
}

fn trace_payload_contains_raw_content(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            let normalized = normalized_metadata_key(key);
            if trace_raw_content_presence_key(&normalized) {
                return value.as_bool().unwrap_or(true);
            }
            if trace_raw_content_payload_key(&normalized) {
                return !matches!(value, Value::Bool(false) | Value::Null);
            }
            trace_payload_contains_raw_content(value)
        }),
        Value::Array(values) => values.iter().any(trace_payload_contains_raw_content),
        Value::String(value) => string_looks_secret_like(value),
        _ => false,
    }
}

fn trace_raw_content_presence_key(normalized: &str) -> bool {
    raw_presence_flag_key(normalized)
        || matches!(
            normalized,
            "containsrawcontent" | "tracecontainsrawcontent" | "rawlifemodeltextincluded"
        )
}

fn trace_raw_content_payload_key(normalized: &str) -> bool {
    raw_outcome_payload_key(normalized)
        || contains_any(
            normalized,
            &[
                "rawtoolpayload",
                "rawmemorytext",
                "memoryrawtext",
                "rawlifemodeltext",
                "lifemodelrawtext",
                "rawlifemodel",
                "lifemodeltext",
            ],
        )
}

fn trace_payload_relaxes_privacy_or_route_policy(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            let normalized = normalized_metadata_key(key);
            if matches!(
                normalized.as_str(),
                "privacypolicypreserved"
                    | "localonlypolicypreserved"
                    | "localonlypreserved"
                    | "routepolicypreserved"
            ) {
                return trace_value_is_false(value);
            }
            if matches!(
                normalized.as_str(),
                "enforcedroutepolicy" | "modelroutepolicy" | "currentroutepolicy"
            ) || privacy_route_override_key(&normalized)
            {
                return route_override_value_relaxes_privacy(value);
            }
            trace_payload_relaxes_privacy_or_route_policy(value)
        }),
        Value::Array(values) => values
            .iter()
            .any(trace_payload_relaxes_privacy_or_route_policy),
        _ => false,
    }
}

fn trace_payload_implies_default_chat_route_cutover(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            let normalized = normalized_metadata_key(key);
            match normalized.as_str() {
                "defaultchatunchanged" => trace_value_is_false(value),
                "ordinarychatentrypointattached"
                | "routecutoverpermission"
                | "migrationpermission"
                | "defaultchatroutecutover" => trace_value_is_truthy(value),
                "defaultchatroute" | "selectedadapterpath" | "defaultchatselectedadapterpath" => {
                    trace_route_value_changes_default_chat(value)
                }
                _ => trace_payload_implies_default_chat_route_cutover(value),
            }
        }),
        Value::Array(values) => values
            .iter()
            .any(trace_payload_implies_default_chat_route_cutover),
        _ => false,
    }
}

fn trace_payload_implies_runtime_execution(value: &Value) -> bool {
    trace_payload_implies_bool_flag(
        value,
        &[
            "runtimeexecuted",
            "ranruntime",
            "runtimeexecution",
            "runtimeran",
        ],
    )
}

fn trace_payload_implies_model_call(value: &Value) -> bool {
    trace_payload_implies_bool_flag(value, &["modelcalled", "ranmodel", "modelcall"])
}

fn trace_payload_implies_tool_call(value: &Value) -> bool {
    trace_payload_implies_bool_flag(value, &["toolcalled", "rantool", "toolcall"])
}

fn trace_payload_implies_heuristic_activation(value: &Value) -> bool {
    trace_payload_implies_bool_flag(
        value,
        &[
            "heuristicactivated",
            "activatesheuristic",
            "heuristicactivation",
        ],
    )
}

fn trace_payload_implies_bool_flag(value: &Value, keys: &[&str]) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            let normalized = normalized_metadata_key(key);
            if keys.iter().any(|candidate| normalized == *candidate) {
                return trace_value_is_truthy(value);
            }
            trace_payload_implies_bool_flag(value, keys)
        }),
        Value::Array(values) => values
            .iter()
            .any(|value| trace_payload_implies_bool_flag(value, keys)),
        _ => false,
    }
}

fn trace_route_value_changes_default_chat(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::String(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            !normalized.is_empty()
                && !matches!(
                    normalized.as_str(),
                    DEFAULT_CHAT_KERNEL_PATH
                        | "legacy"
                        | "false"
                        | "none"
                        | "unchanged"
                        | "default_chat_unchanged"
                )
        }
        Value::Object(_) | Value::Array(_) => {
            trace_payload_implies_default_chat_route_cutover(value)
        }
        _ => false,
    }
}

fn trace_value_is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_i64().is_some_and(|value| value > 0),
        Value::String(value) => {
            let lower = value.trim().to_ascii_lowercase();
            matches!(
                lower.as_str(),
                "true"
                    | "yes"
                    | "enabled"
                    | "executed"
                    | "called"
                    | "activated"
                    | "written"
                    | "attached"
                    | "cutover"
                    | "migrated"
                    | "controlled_adapter"
                    | "controlled adapter"
            )
        }
        Value::Object(_) | Value::Array(_) => trace_payload_contains_truthy_leaf(value),
        _ => false,
    }
}

fn trace_value_is_false(value: &Value) -> bool {
    match value {
        Value::Bool(value) => !*value,
        Value::String(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "false" | "no" | "disabled" | "not_preserved" | "relaxed"
        ),
        _ => false,
    }
}

fn trace_payload_contains_truthy_leaf(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.values().any(trace_value_is_truthy),
        Value::Array(values) => values.iter().any(trace_value_is_truthy),
        _ => trace_value_is_truthy(value),
    }
}

fn candidate_contains_raw_content(candidate: &LifeEventDraft) -> bool {
    string_looks_secret_like(&candidate.summary) || value_contains_raw_content(&candidate.metadata)
}

fn value_contains_raw_content(value: &Value) -> bool {
    match value {
        Value::Object(map) => map
            .iter()
            .any(|(key, value)| raw_content_metadata_key(key) || value_contains_raw_content(value)),
        Value::Array(values) => values.iter().any(value_contains_raw_content),
        Value::String(value) => string_looks_secret_like(value),
        _ => false,
    }
}

fn raw_content_metadata_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    contains_any(
        &normalized,
        &[
            "rawprompt",
            "prompt",
            "rawassistantoutput",
            "assistantoutput",
            "rawmemorycontext",
            "memorycontext",
            "toolpayload",
            "secret",
            "apikey",
            "token",
            "password",
        ],
    )
}

fn string_looks_secret_like(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    contains_any(
        &lower,
        &[
            "raw prompt",
            "assistant output",
            "memory context",
            "tool payload",
            "private key",
            "secret",
            "password",
            "api key",
            "sk-",
        ],
    ) || looks_like_email(value)
}

fn looks_like_email(value: &str) -> bool {
    value.split_whitespace().any(|part| {
        let trimmed = part.trim_matches(|ch: char| {
            matches!(
                ch,
                ',' | ';' | ':' | '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']'
            )
        });
        let Some((local, domain)) = trimmed.split_once('@') else {
            return false;
        };
        !local.is_empty() && domain.contains('.') && !domain.ends_with('.')
    })
}

fn is_low_energy_planning_candidate(candidate: &LifeEventDraft) -> bool {
    let metadata_domain = candidate
        .metadata
        .get("domain")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let combined = searchable(
        &format!("{} {}", candidate.event_type, metadata_domain),
        &candidate.summary,
    );
    let planning = contains_any(&combined, &["planning", "plan", "计划", "规划"]);
    let low_pressure = contains_any(
        &combined,
        &[
            "low_energy",
            "low-energy",
            "low energy",
            "low_pressure",
            "low-pressure",
            "low pressure",
            "低能量",
            "低压力",
        ],
    );
    let supported_surface = contains_any(
        &combined,
        &[
            "preference",
            "preferences",
            "collaboration",
            "planning",
            "偏好",
            "协作",
            "规划",
        ],
    );
    planning && low_pressure && supported_surface
}

fn risk_for_memory(text: &str) -> RiskLevel {
    if contains_any(
        text,
        &[
            "identity",
            "value",
            "mission",
            "relationship",
            "health",
            "medical",
            "financial",
            "finance",
            "身份",
            "价值观",
            "使命",
            "关系",
            "健康",
            "医疗",
            "财务",
        ],
    ) {
        RiskLevel::High
    } else if contains_any(text, &["habit", "workflow", "work_hours", "习惯", "工作流"]) {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
}

fn identity_path(text: &str) -> &'static str {
    if contains_any(text, &["mission", "使命"]) {
        "/identity/mission_statement"
    } else if contains_any(text, &["value", "values", "价值观"]) {
        "/identity/values"
    } else if contains_any(text, &["philosophy", "人生哲学"]) {
        "/identity/life_philosophy"
    } else {
        "/identity"
    }
}

fn goal_path(text: &str) -> &'static str {
    if contains_any(text, &["life_goal", "life_goals", "人生目标"]) {
        "/goals/life_goals"
    } else if contains_any(text, &["long_term", "long-term", "长期"]) {
        "/goals/long_term"
    } else if contains_any(text, &["medium_term", "medium-term", "中期"]) {
        "/goals/medium_term"
    } else if contains_any(text, &["daily", "每日", "日常"]) {
        "/goals/daily"
    } else {
        "/goals/short_term"
    }
}

fn is_long_horizon_goal(text: &str) -> bool {
    contains_any(
        text,
        &[
            "life_goal",
            "life_goals",
            "long_term",
            "long-term",
            "mission",
            "人生目标",
            "长期",
            "使命",
        ],
    )
}

fn state_path(text: &str) -> &'static str {
    if contains_any(text, &["current_focus", "focus", "当前重心"]) {
        "/state/current_focus"
    } else if contains_any(text, &["health", "medical", "健康", "医疗"]) {
        "/state/health_status"
    } else {
        "/state"
    }
}

fn is_sensitive_state(text: &str) -> bool {
    contains_any(
        text,
        &[
            "health",
            "medical",
            "financial",
            "finance",
            "健康",
            "医疗",
            "财务",
        ],
    )
}

fn preference_path(text: &str) -> &'static str {
    if contains_any(text, &["communication", "沟通"]) {
        "/preferences/communication_style"
    } else if contains_any(text, &["learning", "学习"]) {
        "/preferences/learning_style"
    } else if contains_any(text, &["decision", "决策"]) {
        "/preferences/decision_making_style"
    } else if contains_any(text, &["work_hours", "工作时间"]) {
        "/preferences/work_hours"
    } else {
        "/preferences"
    }
}
