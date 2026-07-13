use crate::agent::heuristic_store::{HeuristicDraft, HeuristicStore};
use crate::agent::hs_selector::{HSSelector, HSSelectorInput};
use crate::agent::policy_store::{
    HeuristicPolicyEffect, ModelRoutePolicy, PolicyEvaluationRequest, PolicyStore, PolicyTopic,
    BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING, BUILTIN_HEURISTIC_REJECTED_REMINDER_DELAY,
    BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST, BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY,
};
use crate::agent::types::{AgentTaskKind, RiskLevel};
use crate::tool_manifest::{ToolManifest, ToolSource};
use anyhow::Result;
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegressionVerdict {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionResult {
    pub scenario_id: String,
    pub verdict: RegressionVerdict,
    pub asset_ids: Vec<String>,
    pub reason: String,
    pub details_digest: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegressionScenarioKind {
    SensitiveTopicLocalOnly,
    HeuristicCannotRelaxLocalOnly,
    ExternalWriteProposalFirst,
    LowEnergyPlanning,
    RejectedReminderDelay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegressionScenario {
    pub id: String,
    kind: RegressionScenarioKind,
}

#[derive(Debug, Clone)]
pub struct RegressionSuite {
    scenarios: Vec<RegressionScenario>,
}

impl RegressionSuite {
    pub fn mvp() -> Self {
        Self {
            scenarios: vec![
                scenario(
                    "regression.sensitive_topic_local_only",
                    RegressionScenarioKind::SensitiveTopicLocalOnly,
                ),
                scenario(
                    "regression.heuristic_cannot_relax_local_only",
                    RegressionScenarioKind::HeuristicCannotRelaxLocalOnly,
                ),
                scenario(
                    "regression.external_write_proposal_first",
                    RegressionScenarioKind::ExternalWriteProposalFirst,
                ),
                scenario(
                    "regression.low_energy_planning",
                    RegressionScenarioKind::LowEnergyPlanning,
                ),
                scenario(
                    "regression.rejected_reminder_delay",
                    RegressionScenarioKind::RejectedReminderDelay,
                ),
            ],
        }
    }

    pub fn run_all(
        &self,
        policy_store: &PolicyStore,
        heuristic_store: &HeuristicStore,
    ) -> Result<Vec<RegressionResult>> {
        self.scenarios
            .iter()
            .map(|scenario| self.run_scenario(scenario, policy_store, heuristic_store))
            .collect()
    }

    pub fn run_candidate_heuristic(&self, candidate: &HeuristicDraft) -> RegressionResult {
        let candidate_id = candidate
            .stable_id
            .clone()
            .unwrap_or_else(|| "candidate.heuristic".into());
        let violates_local_only = guidance_relaxes_policy(&candidate.guidance);
        result(
            "regression.local_only_candidate_guard",
            if violates_local_only {
                RegressionVerdict::Fail
            } else {
                RegressionVerdict::Pass
            },
            vec![
                candidate_id.clone(),
                BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
            ],
            if violates_local_only {
                "candidate heuristic attempts to relax LocalOnly policy"
            } else {
                "candidate heuristic does not relax LocalOnly policy"
            },
            serde_json::json!({
                "candidateId": candidate_id,
                "conditionDigest": digest_str(&candidate.conditions.join("|")),
                "guidanceDigest": digest_str(&candidate.guidance),
            }),
        )
    }

    fn run_scenario(
        &self,
        scenario: &RegressionScenario,
        policy_store: &PolicyStore,
        heuristic_store: &HeuristicStore,
    ) -> Result<RegressionResult> {
        match scenario.kind {
            RegressionScenarioKind::SensitiveTopicLocalOnly => {
                let packet = HSSelector.select(
                    policy_store,
                    heuristic_store,
                    &HSSelectorInput {
                        task_kind: AgentTaskKind::Planning,
                        intent_summary: "sanitized sensitive planning scenario".into(),
                        privacy_topic: PolicyTopic::Health,
                        risk_level: RiskLevel::Medium,
                        tool_requirements: vec![],
                        current_state_hints: serde_json::json!({ "energy": 2 }),
                        token_budget: 512,
                        agent_task_id: None,
                        agent_run_id: None,
                    },
                )?;
                let passed = packet.selected_policies.iter().any(|policy| {
                    policy.policy_id == BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY
                        && policy.route == Some(ModelRoutePolicy::LocalOnly)
                });
                Ok(result(
                    &scenario.id,
                    verdict(passed),
                    vec![BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into()],
                    if passed {
                        "sensitive topic selected LocalOnly policy"
                    } else {
                        "sensitive topic did not select LocalOnly policy"
                    },
                    serde_json::json!({ "selectedPolicyIds": packet.audit.selected_policy_ids }),
                ))
            }
            RegressionScenarioKind::HeuristicCannotRelaxLocalOnly => {
                let decision = policy_store.evaluate_context_policy(PolicyEvaluationRequest {
                    topic: PolicyTopic::Health,
                    requested_route: ModelRoutePolicy::CloudAllowed,
                    heuristic_effect: Some(HeuristicPolicyEffect {
                        heuristic_id: "regression.relaxing_heuristic".into(),
                        requested_route: Some(ModelRoutePolicy::CloudAllowed),
                    }),
                });
                let passed = decision.route() == ModelRoutePolicy::LocalOnly
                    && decision
                        .conflicts()
                        .iter()
                        .any(|conflict| conflict.policy_won);
                Ok(result(
                    &scenario.id,
                    verdict(passed),
                    vec![
                        BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
                        "regression.relaxing_heuristic".into(),
                    ],
                    if passed {
                        "policy won over relaxing heuristic"
                    } else {
                        "relaxing heuristic was not blocked"
                    },
                    serde_json::json!({ "conflictCount": decision.conflicts().len() }),
                ))
            }
            RegressionScenarioKind::ExternalWriteProposalFirst => {
                let mut manifest = ToolManifest::new(
                    "file.write",
                    "Direct file write",
                    serde_json::json!({}),
                    "high",
                    "1.0.0",
                    ToolSource::BuiltIn,
                )
                .with_capabilities(vec!["filesystem".into(), "write".into()]);
                manifest.action_type = "write".into();
                let decision = policy_store.evaluate_tool_action(&manifest, false);
                let passed = decision.proposal_first_required && !decision.allowed_direct;
                Ok(result(
                    &scenario.id,
                    verdict(passed),
                    vec![BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST.into()],
                    if passed {
                        "external write requires proposal-first"
                    } else {
                        "external write was allowed directly"
                    },
                    serde_json::json!({ "policyId": decision.policy_id }),
                ))
            }
            RegressionScenarioKind::LowEnergyPlanning => {
                let packet = HSSelector.select(
                    policy_store,
                    heuristic_store,
                    &HSSelectorInput {
                        task_kind: AgentTaskKind::Planning,
                        intent_summary: "sanitized low energy planning scenario".into(),
                        privacy_topic: PolicyTopic::General,
                        risk_level: RiskLevel::Low,
                        tool_requirements: vec![],
                        current_state_hints: serde_json::json!({ "energy": 2 }),
                        token_budget: 512,
                        agent_task_id: None,
                        agent_run_id: None,
                    },
                )?;
                let passed = packet.selected_heuristics.iter().any(|heuristic| {
                    heuristic.heuristic_id == BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING
                });
                Ok(result(
                    &scenario.id,
                    verdict(passed),
                    vec![BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING.into()],
                    if passed {
                        "low-energy planning heuristic selected"
                    } else {
                        "low-energy planning heuristic missing"
                    },
                    serde_json::json!({
                        "selectedHeuristicIds": packet.audit.selected_heuristic_ids
                    }),
                ))
            }
            RegressionScenarioKind::RejectedReminderDelay => {
                let packet = HSSelector.select(
                    policy_store,
                    heuristic_store,
                    &HSSelectorInput {
                        task_kind: AgentTaskKind::Proactive,
                        intent_summary: "sanitized rejected reminder scenario".into(),
                        privacy_topic: PolicyTopic::General,
                        risk_level: RiskLevel::Low,
                        tool_requirements: vec![],
                        current_state_hints: serde_json::json!({ "rejected_reminder": true }),
                        token_budget: 512,
                        agent_task_id: None,
                        agent_run_id: None,
                    },
                )?;
                let passed = packet.selected_heuristics.iter().any(|heuristic| {
                    heuristic.heuristic_id == BUILTIN_HEURISTIC_REJECTED_REMINDER_DELAY
                });
                Ok(result(
                    &scenario.id,
                    verdict(passed),
                    vec![BUILTIN_HEURISTIC_REJECTED_REMINDER_DELAY.into()],
                    if passed {
                        "rejected reminder heuristic selected"
                    } else {
                        "rejected reminder heuristic missing"
                    },
                    serde_json::json!({
                        "selectedHeuristicIds": packet.audit.selected_heuristic_ids
                    }),
                ))
            }
        }
    }
}

fn scenario(id: &str, kind: RegressionScenarioKind) -> RegressionScenario {
    RegressionScenario {
        id: id.into(),
        kind,
    }
}

fn verdict(passed: bool) -> RegressionVerdict {
    if passed {
        RegressionVerdict::Pass
    } else {
        RegressionVerdict::Fail
    }
}

fn result(
    scenario_id: &str,
    verdict: RegressionVerdict,
    asset_ids: Vec<String>,
    reason: &str,
    metadata: Value,
) -> RegressionResult {
    RegressionResult {
        scenario_id: scenario_id.into(),
        verdict,
        asset_ids,
        reason: reason.into(),
        details_digest: digest_str(&metadata.to_string()),
        metadata,
    }
}

fn guidance_relaxes_policy(guidance: &str) -> bool {
    let lower = guidance.to_lowercase();
    lower.contains("use cloud")
        || lower.contains("ignore privacy")
        || lower.contains("relax privacy")
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
