use crate::agent::heuristic_store::{HeuristicLifecycleStatus, HeuristicQuery, HeuristicStore};
use crate::agent::policy_store::{
    ModelRoutePolicy, PolicyEvaluationRequest, PolicyStore, PolicyTopic,
    BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST,
};
use crate::agent::types::{AgentTaskKind, RiskLevel};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HSSelectionAudit {
    pub agent_task_id: Option<String>,
    pub agent_run_id: Option<String>,
    pub input_digest: String,
    pub selected_policy_ids: Vec<String>,
    pub selected_heuristic_ids: Vec<String>,
    pub excluded_assets: Vec<HSAssetExclusion>,
    pub estimated_tokens: usize,
    pub token_budget: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHSPacket {
    pub selected_policies: Vec<SelectedPolicyRef>,
    pub selected_heuristics: Vec<SelectedHeuristic>,
    pub estimated_tokens: usize,
    pub audit: HSSelectionAudit,
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
        let mut excluded_assets = Vec::new();
        let mut estimated_tokens = 0usize;

        let context_policy = policy_store.evaluate_context_policy(PolicyEvaluationRequest {
            topic: input.privacy_topic,
            requested_route: ModelRoutePolicy::CloudAllowed,
            heuristic_effect: None,
        });
        if context_policy.hard_boundary {
            selected_policies.push(SelectedPolicyRef {
                policy_id: context_policy.policy_id.clone(),
                reason: "sensitive_topic_route".into(),
                route: Some(context_policy.route),
                digest: digest_str(&context_policy.policy_id),
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
            if is_sensitive_topic(input.privacy_topic)
                && guidance_relaxes_policy(&heuristic.guidance)
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
            selected_heuristics.push(SelectedHeuristic {
                heuristic_id: heuristic.id,
                domain: heuristic.domain,
                guidance: heuristic.guidance.clone(),
                priority: heuristic.priority,
                source_ids: heuristic.evidence_refs,
                digest: digest_str(&heuristic.guidance),
                estimated_tokens: token_estimate,
            });
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
            excluded_assets,
            estimated_tokens,
            token_budget: input.token_budget,
        };

        Ok(RuntimeHSPacket {
            selected_policies,
            selected_heuristics,
            estimated_tokens,
            audit,
        })
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
