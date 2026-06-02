use crate::agent::evidence_store::{
    EvidenceDraft, EvidencePrivacyLevel, EvidenceSourceRef, EvidenceSourceType, EvidenceStore,
    EvidenceType,
};
use crate::agent::governor::{GovernanceDecision, GovernanceDecisionKind, LifeModelGovernor};
use crate::agent::proposal_store::ProposalStore;
use crate::agent::runtime_contract::{LifeEventDraft, RuntimeOutput};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use anyhow::{anyhow, Result};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

const DEFAULT_CONFIDENCE: f32 = 0.7;
const MIN_CONFIDENCE: f32 = 0.55;
const MIN_SUMMARY_CHARS: usize = 4;
const DEFAULT_CHAT_LEGACY_PATH: &str = "legacy_stream";
const MATURATION_NEXT_ALLOWED_STEP: &str = "non_default_maturation_invocation";

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
            default_chat_selected_adapter_path: DEFAULT_CHAT_LEGACY_PATH.into(),
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
        input.default_chat_selected_adapter_path == DEFAULT_CHAT_LEGACY_PATH;
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
