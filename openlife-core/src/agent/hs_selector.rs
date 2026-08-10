use crate::agent::heuristic_store::{
    HeuristicActivationAuthority, HeuristicConstraintSet, HeuristicLifecycleStatus, HeuristicQuery,
    HeuristicRecord, HeuristicStore,
};
use crate::agent::policy_store::{
    ModelRoutePolicy, PolicyEvaluationRequest, PolicyStore, PolicyTopic,
    BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST,
};
use crate::agent::types::{AgentTaskKind, RiskLevel};
use crate::agent::{AgentTask, EvidencePrivacyLevel, HSBehaviorCheckSummary};
use anyhow::Result;
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HSAssetKind {
    Policy,
    Heuristic,
    Evidence,
    State,
    LifeModelCompat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HSExclusionReason {
    InactiveLifecycle,
    TaskMismatch,
    TriggerMismatch,
    PolicyConflict,
    OverBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HSAssetExclusion {
    pub asset_id: String,
    pub asset_kind: HSAssetKind,
    pub reason: HSExclusionReason,
}

#[derive(Debug, Clone)]
pub struct HSSelectorInput {
    pub task_kind: AgentTaskKind,
    pub intent_summary: String,
    pub privacy_topic: PolicyTopic,
    pub risk_level: RiskLevel,
    pub tool_requirements: Vec<String>,
    pub current_state_hints: Value,
    pub token_budget: usize,
    pub agent_task_id: Option<String>,
    pub agent_run_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeHSPacketBuildInput<'a> {
    pub task: &'a AgentTask,
    pub sanitized_intent_summary: String,
    pub privacy_topic: PolicyTopic,
    pub risk_level: RiskLevel,
    pub tool_requirements: Vec<String>,
    pub current_state_hints: Value,
    pub token_budget: usize,
    pub agent_run_id: Option<String>,
}

/// Narrow input for callers that need PolicyStore authority but must not load
/// the legacy HS/Heuristic personalization path.
#[derive(Debug, Clone)]
pub struct RuntimePolicyContextBuildInput<'a> {
    pub task: &'a AgentTask,
    pub sanitized_intent_summary: String,
    pub privacy_topic: PolicyTopic,
    pub risk_level: RiskLevel,
    pub tool_requirements: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedPolicyRef {
    pub policy_id: String,
    pub reason: String,
    pub route: Option<ModelRoutePolicy>,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedHeuristic {
    pub heuristic_id: String,
    pub domain: String,
    pub guidance: String,
    pub priority: i32,
    pub source_ids: Vec<String>,
    pub digest: String,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidanceAffectedSurface {
    ReactPrompt,
    ReactConfig,
    ActionBoundary,
    PlanExecuteDraft,
    PlanExecuteTrace,
    RuntimeTrace,
}

impl std::fmt::Display for GuidanceAffectedSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuidanceAffectedSurface::ReactPrompt => write!(f, "react_prompt"),
            GuidanceAffectedSurface::ReactConfig => write!(f, "react_config"),
            GuidanceAffectedSurface::ActionBoundary => write!(f, "action_boundary"),
            GuidanceAffectedSurface::PlanExecuteDraft => write!(f, "plan_execute_draft"),
            GuidanceAffectedSurface::PlanExecuteTrace => write!(f, "plan_execute_trace"),
            GuidanceAffectedSurface::RuntimeTrace => write!(f, "runtime_trace"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuidancePolicyBoundarySummary {
    pub hard_policy_boundary: bool,
    pub route_policy_relaxed: bool,
    pub tool_policy_relaxed: bool,
    pub proposal_first_preserved: bool,
    pub privacy_constraint_count: usize,
    pub model_constraint_count: usize,
    pub tool_constraint_count: usize,
    pub constraint_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedGuidanceRef {
    pub guidance_id: String,
    pub guidance_digest: String,
    pub guidance_type: String,
    pub lifecycle_status: HeuristicLifecycleStatus,
    pub domain: String,
    pub trigger_digest: String,
    pub selected_reason: String,
    pub impact_kind: String,
    pub impact_summary: String,
    pub risk_level: RiskLevel,
    pub privacy_level: EvidencePrivacyLevel,
    pub source_proposal_id: Option<String>,
    pub source_evidence_count: usize,
    pub source_lineage_digest: String,
    pub policy_boundary: GuidancePolicyBoundarySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HSSelectionAudit {
    pub agent_task_id: Option<String>,
    pub agent_run_id: Option<String>,
    pub input_digest: String,
    pub selected_policy_ids: Vec<String>,
    pub selected_heuristic_ids: Vec<String>,
    #[serde(default)]
    pub selected_guidance_ids: Vec<String>,
    #[serde(default)]
    pub selected_guidance_refs: Vec<SelectedGuidanceRef>,
    pub excluded_assets: Vec<HSAssetExclusion>,
    pub estimated_tokens: usize,
    pub token_budget: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHSPacket {
    pub selected_policies: Vec<SelectedPolicyRef>,
    pub selected_heuristics: Vec<SelectedHeuristic>,
    #[serde(default)]
    pub guidance_refs: Vec<SelectedGuidanceRef>,
    pub estimated_tokens: usize,
    pub audit: HSSelectionAudit,
    /// Ephemeral capability issued by PolicyStore. It is intentionally omitted
    /// from serialization; a deserialized packet cannot recreate cloud
    /// authority from its public metadata.
    #[serde(skip)]
    pub provider_authorization: crate::llm::ProviderPolicyAuthorization,
}

impl RuntimeHSPacket {
    pub fn provider_authorization(&self) -> &crate::llm::ProviderPolicyAuthorization {
        &self.provider_authorization
    }

    pub fn provider_policy_provenance_refs(&self) -> Vec<crate::llm::ProviderPolicyProvenanceRef> {
        let route_digest = crate::agent::metadata_safe::metadata_safe_text_digest(&format!(
            "{}:{}:{:?}",
            self.provider_authorization.decision_id(),
            self.provider_authorization.policy_version(),
            self.provider_authorization.data_route(),
        ))
        .1;
        let route_kind = match self.provider_authorization.authority() {
            crate::llm::ProviderPolicyAuthority::MainChatPolicyRouter => {
                crate::llm::ProviderPolicyProvenanceKind::MainChatRouteDecision
            }
            crate::llm::ProviderPolicyAuthority::PolicyStore => {
                crate::llm::ProviderPolicyProvenanceKind::PolicyStoreRouteDecision
            }
            crate::llm::ProviderPolicyAuthority::HsPolicyStore => {
                crate::llm::ProviderPolicyProvenanceKind::HsRouteDecision
            }
            crate::llm::ProviderPolicyAuthority::ScheduledPolicy => {
                crate::llm::ProviderPolicyProvenanceKind::ScheduledRouteDecision
            }
            crate::llm::ProviderPolicyAuthority::ExplicitProviderProbePolicy => {
                crate::llm::ProviderPolicyProvenanceKind::ExplicitProviderProbeDecision
            }
            crate::llm::ProviderPolicyAuthority::LocalOnlyFailClosed => {
                crate::llm::ProviderPolicyProvenanceKind::FailClosedRouteDecision
            }
        };
        let mut refs = vec![crate::llm::ProviderPolicyProvenanceRef::new(
            route_kind,
            self.provider_authorization.decision_id(),
            route_digest,
        )];
        let policy_kind = if self.provider_authorization.authority()
            == crate::llm::ProviderPolicyAuthority::PolicyStore
        {
            crate::llm::ProviderPolicyProvenanceKind::PolicyStorePolicy
        } else {
            crate::llm::ProviderPolicyProvenanceKind::HsPolicy
        };
        refs.extend(self.selected_policies.iter().map(|policy| {
            crate::llm::ProviderPolicyProvenanceRef::new(
                policy_kind,
                &policy.policy_id,
                &policy.digest,
            )
        }));
        refs.extend(self.guidance_refs.iter().map(|guidance| {
            crate::llm::ProviderPolicyProvenanceRef::new(
                crate::llm::ProviderPolicyProvenanceKind::HsGuidance,
                &guidance.guidance_id,
                &guidance.guidance_digest,
            )
        }));
        refs.sort();
        refs.dedup();
        refs
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuidanceImpactRef {
    pub guidance_id: String,
    pub guidance_digest: String,
    pub guidance_type: String,
    pub lifecycle_status: HeuristicLifecycleStatus,
    pub domain: String,
    pub impact_kind: String,
    pub selected_reason: String,
    pub source_proposal_id: Option<String>,
    pub source_evidence_count: usize,
    pub source_lineage_digest: String,
    pub affected_run_count: usize,
    pub affected_surfaces: Vec<GuidanceAffectedSurface>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuidanceImpactReadModel {
    pub report_kind: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub run_id: Option<String>,
    pub strategy_kind: String,
    pub selected_guidance_count: usize,
    pub selected_policy_count: usize,
    pub guidance_refs: Vec<GuidanceImpactRef>,
    pub selected_policy_ids: Vec<String>,
    pub affected_surfaces: Vec<GuidanceAffectedSurface>,
    pub behavior_check_count: usize,
    pub read_model_digest: String,
    pub raw_prompt_included: bool,
    pub raw_user_text_included: bool,
    pub raw_assistant_output_included: bool,
    pub raw_memory_included: bool,
    pub raw_life_model_included: bool,
    pub raw_tool_payload_included: bool,
    pub raw_guidance_included: bool,
}

#[derive(Debug, Clone, Default)]
pub struct HSSelector;

impl HSSelector {
    pub fn select(
        &self,
        policy_store: &PolicyStore,
        heuristic_store: &HeuristicStore,
        input: &HSSelectorInput,
    ) -> Result<RuntimeHSPacket> {
        let mut selected_policies = Vec::new();
        let mut selected_heuristics = Vec::new();
        let mut guidance_refs = Vec::new();
        let mut excluded_assets = Vec::new();
        let mut estimated_tokens = 0usize;

        let context_policy = policy_store.evaluate_context_policy(PolicyEvaluationRequest {
            topic: input.privacy_topic,
            requested_route: ModelRoutePolicy::CloudAllowed,
            heuristic_effect: None,
        });
        if context_policy.hard_boundary() {
            selected_policies.push(SelectedPolicyRef {
                policy_id: context_policy.policy_id().to_string(),
                reason: "sensitive_topic_route".into(),
                route: Some(context_policy.route()),
                digest: digest_str(context_policy.policy_id()),
            });
        }

        if input
            .tool_requirements
            .iter()
            .any(|req| matches!(req.as_str(), "write" | "external_side_effect"))
        {
            selected_policies.push(SelectedPolicyRef {
                policy_id: BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST.into(),
                reason: "tool_requirement_write".into(),
                route: None,
                digest: digest_str(BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST),
            });
        }

        let domain = task_domain(input.task_kind);
        let heuristics = heuristic_store.query(HeuristicQuery {
            domain: domain.map(str::to_string),
            ..HeuristicQuery::default()
        })?;

        for heuristic in heuristics {
            if !matches!(
                heuristic.status,
                HeuristicLifecycleStatus::Active | HeuristicLifecycleStatus::Trial
            ) {
                excluded_assets.push(exclude(&heuristic.id, HSExclusionReason::InactiveLifecycle));
                continue;
            }
            if !trigger_matches(&heuristic.trigger, &input.current_state_hints) {
                excluded_assets.push(exclude(&heuristic.id, HSExclusionReason::TriggerMismatch));
                continue;
            }
            if guidance_relaxes_policy(&heuristic.guidance)
                || constraints_relax_policy(&heuristic.constraints)
                || (is_sensitive_topic(input.privacy_topic)
                    && guidance_relaxes_policy(&heuristic.guidance))
            {
                excluded_assets.push(exclude(&heuristic.id, HSExclusionReason::PolicyConflict));
                continue;
            }

            let token_estimate = estimate_tokens(&heuristic.guidance);
            if estimated_tokens + token_estimate > input.token_budget {
                excluded_assets.push(exclude(&heuristic.id, HSExclusionReason::OverBudget));
                continue;
            }

            estimated_tokens += token_estimate;
            let guidance_ref = is_accepted_runtime_guidance_asset(&heuristic)
                .then(|| selected_guidance_ref(&heuristic, input));
            selected_heuristics.push(SelectedHeuristic {
                heuristic_id: heuristic.id.clone(),
                domain: heuristic.domain.clone(),
                guidance: heuristic.guidance.clone(),
                priority: heuristic.priority,
                source_ids: heuristic.evidence_refs.clone(),
                digest: digest_str(&heuristic.guidance),
                estimated_tokens: token_estimate,
            });
            if let Some(guidance_ref) = guidance_ref {
                guidance_refs.push(guidance_ref);
            }
        }

        let audit = HSSelectionAudit {
            agent_task_id: input.agent_task_id.clone(),
            agent_run_id: input.agent_run_id.clone(),
            input_digest: digest_str(&format!(
                "{}:{}:{}",
                input.task_kind, input.risk_level, input.intent_summary
            )),
            selected_policy_ids: selected_policies
                .iter()
                .map(|policy| policy.policy_id.clone())
                .collect(),
            selected_heuristic_ids: selected_heuristics
                .iter()
                .map(|heuristic| heuristic.heuristic_id.clone())
                .collect(),
            selected_guidance_ids: guidance_refs
                .iter()
                .map(|guidance| guidance.guidance_id.clone())
                .collect(),
            selected_guidance_refs: guidance_refs.clone(),
            excluded_assets,
            estimated_tokens,
            token_budget: input.token_budget,
        };
        let provider_authorization =
            crate::llm::ProviderPolicyAuthorization::from_hs_context_decision(
                &context_policy,
                audit.input_digest.clone(),
            )?;

        Ok(RuntimeHSPacket {
            selected_policies,
            selected_heuristics,
            guidance_refs,
            estimated_tokens,
            audit,
            provider_authorization,
        })
    }
}

pub fn build_runtime_hs_packet(
    policy_store: &PolicyStore,
    heuristic_store: &HeuristicStore,
    input: RuntimeHSPacketBuildInput<'_>,
) -> Result<Option<RuntimeHSPacket>> {
    let mut packet = HSSelector.select(
        policy_store,
        heuristic_store,
        &HSSelectorInput {
            task_kind: input.task.kind,
            intent_summary: input.sanitized_intent_summary,
            privacy_topic: input.privacy_topic,
            risk_level: input.risk_level,
            tool_requirements: input.tool_requirements,
            current_state_hints: input.current_state_hints,
            token_budget: input.token_budget,
            agent_task_id: None,
            agent_run_id: input.agent_run_id,
        },
    )?;
    packet.provider_authorization = packet
        .provider_authorization
        .bind_hs_current_user_subject(&input.task.user_text)?;

    // The packet now carries the canonical provider-policy capability even
    // when no optional heuristic or hard-boundary asset was selected. Dropping
    // an otherwise-empty packet would discard the HS PolicyStore decision and
    // force downstream runtimes either to self-authorize or silently lose
    // cloud capability.
    Ok(Some(packet))
}

/// Evaluate the narrow PolicyStore facts consumed by generic runtime paths.
/// This result cannot contain heuristic guidance or personal model data.
pub fn build_runtime_policy_context(
    policy_store: &PolicyStore,
    input: RuntimePolicyContextBuildInput<'_>,
) -> Result<crate::agent::RuntimePolicyContext> {
    let context_policy = policy_store.evaluate_context_policy(PolicyEvaluationRequest {
        topic: input.privacy_topic,
        requested_route: ModelRoutePolicy::CloudAllowed,
        heuristic_effect: None,
    });
    let mut selected_policies = Vec::new();
    if context_policy.hard_boundary() {
        selected_policies.push(SelectedPolicyRef {
            policy_id: context_policy.policy_id().to_string(),
            reason: "sensitive_topic_route".into(),
            route: Some(context_policy.route()),
            digest: digest_str(context_policy.policy_id()),
        });
    }
    if input
        .tool_requirements
        .iter()
        .any(|requirement| matches!(requirement.as_str(), "write" | "external_side_effect"))
    {
        selected_policies.push(SelectedPolicyRef {
            policy_id: BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST.into(),
            reason: "tool_requirement_write".into(),
            route: None,
            digest: digest_str(BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST),
        });
    }
    let input_digest = digest_str(&format!(
        "{}:{}:{}",
        input.task.kind, input.risk_level, input.sanitized_intent_summary
    ));
    let provider_authorization =
        crate::llm::ProviderPolicyAuthorization::from_policy_store_context_decision(
            &context_policy,
            input_digest.clone(),
        )?
        .bind_policy_store_current_user_subject(&input.task.user_text)?;
    let mut provenance = vec![crate::llm::ProviderPolicyProvenanceRef::new(
        crate::llm::ProviderPolicyProvenanceKind::PolicyStoreRouteDecision,
        provider_authorization.decision_id(),
        &input_digest,
    )];
    provenance.extend(selected_policies.iter().map(|policy| {
        crate::llm::ProviderPolicyProvenanceRef::new(
            crate::llm::ProviderPolicyProvenanceKind::PolicyStorePolicy,
            &policy.policy_id,
            &policy.digest,
        )
    }));
    let external_write_requires_proposal = selected_policies
        .iter()
        .any(|policy| policy.policy_id == BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST);

    Ok(crate::agent::RuntimePolicyContext::new(
        provider_authorization,
        provenance,
        external_write_requires_proposal,
    ))
}

pub fn behavior_checks_for_packet(packet: &RuntimeHSPacket) -> Vec<HSBehaviorCheckSummary> {
    let mut checks = Vec::new();

    if packet
        .audit
        .selected_policy_ids
        .iter()
        .any(|id| id == crate::agent::BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY)
    {
        checks.push(HSBehaviorCheckSummary {
            id: "regression.sensitive_topic_local_only".into(),
            label: "Sensitive topics stay local".into(),
            passed: true,
            summary: Some("Local-only routing policy was selected.".into()),
        });
    }

    if packet
        .audit
        .selected_policy_ids
        .iter()
        .any(|id| id == crate::agent::BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST)
    {
        checks.push(HSBehaviorCheckSummary {
            id: "regression.external_write_proposal_first".into(),
            label: "External writes stay reviewable".into(),
            passed: true,
            summary: Some("Direct external writes become proposals first.".into()),
        });
    }

    if packet
        .audit
        .selected_heuristic_ids
        .iter()
        .any(|id| id == crate::agent::BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING)
        || packet
            .guidance_refs
            .iter()
            .any(|guidance| guidance.impact_kind == "gentle_planning")
    {
        checks.push(HSBehaviorCheckSummary {
            id: "regression.low_energy_planning".into(),
            label: "Low-energy planning stays gentle".into(),
            passed: true,
            summary: Some("Planning guidance was bounded to selected collaboration style.".into()),
        });
    }

    if !packet.guidance_refs.is_empty() {
        checks.push(HSBehaviorCheckSummary {
            id: "runtime_guidance.selected_guidance_metadata".into(),
            label: "Runtime guidance metadata is traceable".into(),
            passed: true,
            summary: Some(format!(
                "Selected guidance refs are metadata-safe; count={}.",
                packet.guidance_refs.len()
            )),
        });
    }

    checks
}

pub fn build_guidance_impact_read_model(
    run_id: Option<&str>,
    strategy_kind: impl Into<String>,
    packet: &RuntimeHSPacket,
    affected_surfaces: Vec<GuidanceAffectedSurface>,
) -> GuidanceImpactReadModel {
    let strategy_kind = strategy_kind.into();
    let guidance_refs = packet
        .guidance_refs
        .iter()
        .map(|guidance| GuidanceImpactRef {
            guidance_id: guidance.guidance_id.clone(),
            guidance_digest: guidance.guidance_digest.clone(),
            guidance_type: guidance.guidance_type.clone(),
            lifecycle_status: guidance.lifecycle_status,
            domain: guidance.domain.clone(),
            impact_kind: guidance.impact_kind.clone(),
            selected_reason: guidance.selected_reason.clone(),
            source_proposal_id: guidance.source_proposal_id.clone(),
            source_evidence_count: guidance.source_evidence_count,
            source_lineage_digest: guidance.source_lineage_digest.clone(),
            affected_run_count: run_id.is_some() as usize,
            affected_surfaces: affected_surfaces.clone(),
        })
        .collect::<Vec<_>>();
    let selected_policy_ids = packet
        .selected_policies
        .iter()
        .map(|policy| policy.policy_id.clone())
        .collect::<Vec<_>>();
    let behavior_check_count = behavior_checks_for_packet(packet).len();
    let read_model_digest = digest_str(
        &serde_json::json!({
            "schema": "w140.guidanceImpactReadModel.digest.v1",
            "runId": run_id,
            "strategyKind": strategy_kind,
            "guidanceRefs": guidance_refs,
            "policyIds": selected_policy_ids,
            "affectedSurfaces": affected_surfaces,
            "behaviorCheckCount": behavior_check_count,
        })
        .to_string(),
    );

    GuidanceImpactReadModel {
        report_kind: "w140.guidanceImpactReadModel.v1".into(),
        metadata_safe: true,
        contains_raw_content: false,
        run_id: run_id.map(str::to_string),
        strategy_kind,
        selected_guidance_count: guidance_refs.len(),
        selected_policy_count: selected_policy_ids.len(),
        guidance_refs,
        selected_policy_ids,
        affected_surfaces,
        behavior_check_count,
        read_model_digest: format!("sha256:{read_model_digest}"),
        raw_prompt_included: false,
        raw_user_text_included: false,
        raw_assistant_output_included: false,
        raw_memory_included: false,
        raw_life_model_included: false,
        raw_tool_payload_included: false,
        raw_guidance_included: false,
    }
}

fn task_domain(task_kind: AgentTaskKind) -> Option<&'static str> {
    match task_kind {
        AgentTaskKind::Planning => Some("planning"),
        AgentTaskKind::Proactive => Some("proactive"),
        AgentTaskKind::Conversation => Some("conversation"),
        AgentTaskKind::ToolExecution => Some("runtime_behavior"),
        _ => None,
    }
}

fn trigger_matches(trigger: &str, state: &Value) -> bool {
    match trigger {
        "current_energy_is_low" => state
            .get("energy")
            .and_then(Value::as_i64)
            .is_some_and(|energy| energy <= 3),
        "similar_reminder_was_rejected" => state
            .get("rejected_reminder")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        _ => true,
    }
}

fn selected_guidance_ref(
    heuristic: &HeuristicRecord,
    input: &HSSelectorInput,
) -> SelectedGuidanceRef {
    let policy_boundary = guidance_policy_boundary(heuristic);
    let impact_kind = guidance_impact_kind(heuristic).to_string();
    let source_lineage_digest = digest_str(
        &serde_json::json!({
            "sourceProposalId": heuristic.source_proposal_id,
            "sourceEvidenceRefs": sorted_unique(heuristic.evidence_refs.clone()),
            "opposingEvidenceRefCount": heuristic.opposing_evidence_refs.len(),
            "version": heuristic.version,
        })
        .to_string(),
    );
    SelectedGuidanceRef {
        guidance_id: heuristic.id.clone(),
        guidance_digest: format!("sha256:{}", digest_str(&heuristic.guidance)),
        guidance_type: "accepted_guidance".into(),
        lifecycle_status: heuristic.status,
        domain: heuristic.domain.clone(),
        trigger_digest: format!("sha256:{}", digest_str(&heuristic.trigger)),
        selected_reason: selected_reason(heuristic, input).into(),
        impact_kind: impact_kind.clone(),
        impact_summary: guidance_impact_summary(&impact_kind).into(),
        risk_level: heuristic.risk_level,
        privacy_level: heuristic.privacy_level,
        source_proposal_id: heuristic.source_proposal_id.clone(),
        source_evidence_count: sorted_unique(heuristic.evidence_refs.clone()).len(),
        source_lineage_digest: format!("sha256:{source_lineage_digest}"),
        policy_boundary,
    }
}

fn is_accepted_runtime_guidance_asset(heuristic: &HeuristicRecord) -> bool {
    let explicit_accepted_lifecycle_asset = heuristic.id.starts_with("accepted_guidance_")
        && heuristic
            .source_proposal_id
            .as_deref()
            .is_some_and(|id| !id.trim().is_empty());
    let accepted_authority = matches!(
        heuristic.activation_authority,
        Some(HeuristicActivationAuthority::AcceptedProposal(_))
    );

    explicit_accepted_lifecycle_asset || accepted_authority
}

fn selected_reason(heuristic: &HeuristicRecord, input: &HSSelectorInput) -> &'static str {
    if task_domain(input.task_kind) == Some(heuristic.domain.as_str())
        && trigger_matches(&heuristic.trigger, &input.current_state_hints)
    {
        "task_domain_and_trigger_match"
    } else {
        "selector_match"
    }
}

fn guidance_policy_boundary(heuristic: &HeuristicRecord) -> GuidancePolicyBoundarySummary {
    let route_policy_relaxed = guidance_relaxes_policy(&heuristic.guidance)
        || constraints_relax_route(&heuristic.constraints);
    let tool_policy_relaxed = guidance_relaxes_tool_policy(&heuristic.guidance)
        || constraints_relax_tool(&heuristic.constraints);
    let proposal_first_preserved = !tool_policy_relaxed
        && (heuristic
            .constraints
            .tool
            .iter()
            .any(|constraint| constraint.contains("proposal_first"))
            || !heuristic
                .guidance
                .to_ascii_lowercase()
                .contains("direct write"));
    let constraint_digest = digest_str(
        &serde_json::json!({
            "privacyCount": heuristic.constraints.privacy.len(),
            "modelCount": heuristic.constraints.model.len(),
            "toolCount": heuristic.constraints.tool.len(),
            "routePolicyRelaxed": route_policy_relaxed,
            "toolPolicyRelaxed": tool_policy_relaxed,
            "proposalFirstPreserved": proposal_first_preserved,
        })
        .to_string(),
    );

    GuidancePolicyBoundarySummary {
        hard_policy_boundary: true,
        route_policy_relaxed,
        tool_policy_relaxed,
        proposal_first_preserved,
        privacy_constraint_count: heuristic.constraints.privacy.len(),
        model_constraint_count: heuristic.constraints.model.len(),
        tool_constraint_count: heuristic.constraints.tool.len(),
        constraint_digest: format!("sha256:{constraint_digest}"),
    }
}

fn guidance_impact_kind(heuristic: &HeuristicRecord) -> &'static str {
    let haystack = format!(
        "{} {} {} {}",
        heuristic.domain,
        heuristic.trigger,
        heuristic.guidance,
        heuristic.conditions.join(" ")
    )
    .to_ascii_lowercase();
    if heuristic.domain == "planning"
        && (heuristic.trigger == "current_energy_is_low"
            || haystack.contains("low pressure")
            || haystack.contains("small")
            || haystack.contains("tiny")
            || haystack.contains("energy"))
    {
        "gentle_planning"
    } else if heuristic
        .constraints
        .tool
        .iter()
        .any(|constraint| constraint.contains("proposal_first"))
    {
        "proposal_first_boundary"
    } else {
        "collaboration_guidance"
    }
}

fn guidance_impact_summary(impact_kind: &str) -> &'static str {
    match impact_kind {
        "gentle_planning" => "Prefer smaller lower-pressure planning actions.",
        "proposal_first_boundary" => "Keep write-like actions proposal-first.",
        _ => "Apply accepted collaboration guidance.",
    }
}

fn is_sensitive_topic(topic: PolicyTopic) -> bool {
    matches!(
        topic,
        PolicyTopic::Health
            | PolicyTopic::Relationship
            | PolicyTopic::Identity
            | PolicyTopic::Finance
            | PolicyTopic::PrivateFile
    )
}

fn guidance_relaxes_policy(guidance: &str) -> bool {
    let lower = guidance.to_lowercase();
    lower.contains("use cloud")
        || lower.contains("ignore privacy")
        || lower.contains("relax privacy")
        || lower.contains("bypass proposal")
        || lower.contains("skip proposal")
        || lower.contains("bypass review")
        || lower.contains("direct write")
        || lower.contains("ignore tool policy")
}

fn guidance_relaxes_tool_policy(guidance: &str) -> bool {
    let lower = guidance.to_lowercase();
    lower.contains("bypass proposal")
        || lower.contains("skip proposal")
        || lower.contains("bypass review")
        || lower.contains("direct write")
        || lower.contains("ignore tool policy")
}

fn constraints_relax_policy(constraints: &HeuristicConstraintSet) -> bool {
    constraints_relax_route(constraints) || constraints_relax_tool(constraints)
}

fn constraints_relax_route(constraints: &HeuristicConstraintSet) -> bool {
    constraints
        .privacy
        .iter()
        .chain(constraints.model.iter())
        .any(|constraint| guidance_relaxes_policy(constraint))
}

fn constraints_relax_tool(constraints: &HeuristicConstraintSet) -> bool {
    constraints
        .tool
        .iter()
        .any(|constraint| guidance_relaxes_tool_policy(constraint))
}

fn estimate_tokens(text: &str) -> usize {
    (text.chars().count() / 4).max(1) + 8
}

fn exclude(asset_id: &str, reason: HSExclusionReason) -> HSAssetExclusion {
    HSAssetExclusion {
        asset_id: asset_id.to_string(),
        asset_kind: HSAssetKind::Heuristic,
        reason,
    }
}

fn digest_str(value: &str) -> String {
    let hash = digest(&SHA256, value.as_bytes());
    let bytes = hash.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}
