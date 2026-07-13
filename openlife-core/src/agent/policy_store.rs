use crate::tool_manifest::ToolManifest;
use serde::{Deserialize, Serialize};

pub const BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY: &str = "policy.sensitive_topics.local_only";
pub const BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST: &str =
    "policy.external_writes.proposal_first";
pub const BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING: &str = "heuristic.low_energy_planning";
pub const BUILTIN_HEURISTIC_REJECTED_REMINDER_DELAY: &str = "heuristic.rejected_reminder_delay";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyTopic {
    Health,
    Relationship,
    Identity,
    Finance,
    PrivateFile,
    General,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRoutePolicy {
    CloudAllowed,
    LocalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeuristicPolicyEffect {
    pub heuristic_id: String,
    pub requested_route: Option<ModelRoutePolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyEvaluationRequest {
    pub topic: PolicyTopic,
    pub requested_route: ModelRoutePolicy,
    pub heuristic_effect: Option<HeuristicPolicyEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyConflictAudit {
    pub policy_id: String,
    pub heuristic_id: Option<String>,
    pub attempted_route: Option<ModelRoutePolicy>,
    pub enforced_route: ModelRoutePolicy,
    pub policy_won: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPolicyDecision {
    policy_id: String,
    route: ModelRoutePolicy,
    hard_boundary: bool,
    conflicts: Vec<PolicyConflictAudit>,
}

impl ContextPolicyDecision {
    pub fn policy_id(&self) -> &str {
        &self.policy_id
    }

    pub fn route(&self) -> ModelRoutePolicy {
        self.route
    }

    pub fn hard_boundary(&self) -> bool {
        self.hard_boundary
    }

    pub fn conflicts(&self) -> &[PolicyConflictAudit] {
        &self.conflicts
    }

    /// Re-check the invariant that makes this value an authority object rather
    /// than a caller-authored route label.
    pub(crate) fn validate_provider_authority(&self) -> anyhow::Result<()> {
        match self.policy_id.as_str() {
            BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY => {
                if self.route != ModelRoutePolicy::LocalOnly || !self.hard_boundary {
                    anyhow::bail!("sensitive-topic provider policy is not fail closed");
                }
                if self.conflicts.iter().any(|conflict| {
                    conflict.policy_id != BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY
                        || conflict.enforced_route != ModelRoutePolicy::LocalOnly
                        || !conflict.policy_won
                }) {
                    anyhow::bail!("sensitive-topic provider policy has invalid conflict evidence");
                }
            }
            "policy.general.default_route" => {
                if self.hard_boundary || !self.conflicts.is_empty() {
                    anyhow::bail!("general provider policy has non-canonical boundary evidence");
                }
            }
            _ => anyhow::bail!("unknown HS provider policy authority"),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPolicyDecision {
    pub policy_id: String,
    pub allowed_direct: bool,
    pub proposal_first_required: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRecord {
    pub id: String,
    pub hard_boundary: bool,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct PolicyStore {
    policies: Vec<PolicyRecord>,
}

impl PolicyStore {
    pub fn mvp_builtin() -> Self {
        Self {
            policies: vec![
                PolicyRecord {
                    id: BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
                    hard_boundary: true,
                    description:
                        "Sensitive health, relationship, identity, finance, and private-file topics default to LocalOnly."
                            .into(),
                },
                PolicyRecord {
                    id: BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST.into(),
                    hard_boundary: true,
                    description:
                        "External write actions require draft/proposal-first execution unless user-confirmed."
                            .into(),
                },
            ],
        }
    }

    pub fn is_hard_policy_id(&self, id: &str) -> bool {
        self.policies
            .iter()
            .any(|policy| policy.id == id && policy.hard_boundary)
    }

    pub fn evaluate_context_policy(
        &self,
        request: PolicyEvaluationRequest,
    ) -> ContextPolicyDecision {
        if Self::is_sensitive_topic(request.topic) {
            let mut conflicts = Vec::new();
            if let Some(effect) = request.heuristic_effect {
                if effect.requested_route == Some(ModelRoutePolicy::CloudAllowed) {
                    conflicts.push(PolicyConflictAudit {
                        policy_id: BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
                        heuristic_id: Some(effect.heuristic_id),
                        attempted_route: effect.requested_route,
                        enforced_route: ModelRoutePolicy::LocalOnly,
                        policy_won: true,
                    });
                }
            }
            return ContextPolicyDecision {
                policy_id: BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
                route: ModelRoutePolicy::LocalOnly,
                hard_boundary: true,
                conflicts,
            };
        }

        ContextPolicyDecision {
            policy_id: "policy.general.default_route".into(),
            route: request.requested_route,
            hard_boundary: false,
            conflicts: Vec::new(),
        }
    }

    pub fn evaluate_tool_action(
        &self,
        manifest: &ToolManifest,
        already_confirmed_by_user: bool,
    ) -> ToolPolicyDecision {
        if Self::is_external_write_action(manifest)
            && !Self::is_proposal_generation_tool(&manifest.name)
            && !already_confirmed_by_user
        {
            return ToolPolicyDecision {
                policy_id: BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST.into(),
                allowed_direct: false,
                proposal_first_required: true,
                reason: "external write action must create a draft/proposal first".into(),
            };
        }

        ToolPolicyDecision {
            policy_id: BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST.into(),
            allowed_direct: true,
            proposal_first_required: false,
            reason: if already_confirmed_by_user {
                "external write action already confirmed by user".into()
            } else {
                "not an unconfirmed direct external write action".into()
            },
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

    fn is_external_write_action(manifest: &ToolManifest) -> bool {
        manifest.action_type == "write"
            || manifest.action_type == "external_side_effect"
            || manifest
                .capabilities
                .iter()
                .any(|capability| matches!(capability.as_str(), "write" | "external_side_effect"))
    }

    fn is_proposal_generation_tool(name: &str) -> bool {
        name.ends_with("_proposal")
            || name.ends_with("_propose_write")
            || name.ends_with("_propose_archive")
            || name.ends_with("_propose_patch")
            || name.ends_with("_propose_update")
            || name.ends_with(".propose_write")
            || name.ends_with(".propose_archive")
            || name.ends_with(".propose_patch")
            || name.ends_with(".propose_update")
            || name.ends_with(".propose_event")
    }
}
