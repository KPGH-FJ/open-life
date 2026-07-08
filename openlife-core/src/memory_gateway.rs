use crate::agent::AgentProposal;
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLane {
    TurnContext,
    EpisodicLifeEvent,
    SemanticFactPreference,
    ProceduralRule,
    EvidenceRecord,
    CanonicalLifeModelTruth,
}

impl MemoryLane {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TurnContext => "turn_context",
            Self::EpisodicLifeEvent => "episodic_life_event",
            Self::SemanticFactPreference => "semantic_fact_preference",
            Self::ProceduralRule => "procedural_rule",
            Self::EvidenceRecord => "evidence_record",
            Self::CanonicalLifeModelTruth => "canonical_lifemodel_truth",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryGatewayWriteStatus {
    ContextOnly,
    LocalMemoryWritten,
    ProposalRequired,
    CanonicalLifeModelWritten,
    Blocked,
}

impl MemoryGatewayWriteStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ContextOnly => "context_only",
            Self::LocalMemoryWritten => "local_memory_written",
            Self::ProposalRequired => "proposal_required",
            Self::CanonicalLifeModelWritten => "canonical_lifemodel_written",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryGatewaySubject {
    ChatTurn,
    FoodEvent,
    HealthEvent,
    Preference,
    Routine,
    FuturePlanRule,
    Evidence,
    CanonicalLifeModel,
    ManualIndexedNote,
    ImportedArchive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryGatewayRequest {
    pub source: Option<String>,
    pub proposal_type: Option<String>,
    pub affected_path: Option<String>,
    pub payload_kind: Option<String>,
    pub content_preview_digest: Option<String>,
    pub user_intent_kind: Option<String>,
    pub evidence_refs: Vec<String>,
    #[serde(skip)]
    payload_text: Option<String>,
}

impl MemoryGatewayRequest {
    pub fn from_subject(subject: MemoryGatewaySubject) -> Self {
        let (source, proposal_type, affected_path, payload_kind, user_intent_kind, payload_text) =
            match subject {
                MemoryGatewaySubject::ChatTurn => (
                    Some("chat_turn".into()),
                    None,
                    Some("turn.context".into()),
                    Some("chat_message".into()),
                    Some("turn_context_capture".into()),
                    None,
                ),
                MemoryGatewaySubject::FoodEvent => (
                    Some("memory_gateway_subject".into()),
                    Some("memory_write".into()),
                    Some("memory.food_event".into()),
                    Some("food_event".into()),
                    Some("local_memory_materialization".into()),
                    Some("food diet meal".into()),
                ),
                MemoryGatewaySubject::HealthEvent => (
                    Some("memory_gateway_subject".into()),
                    Some("memory_write".into()),
                    Some("memory.health_event".into()),
                    Some("health_event".into()),
                    Some("local_memory_materialization".into()),
                    Some("health sleep energy body".into()),
                ),
                MemoryGatewaySubject::Preference => (
                    Some("memory_gateway_subject".into()),
                    Some("memory_write".into()),
                    Some("memory.preference".into()),
                    Some("preference".into()),
                    Some("local_memory_materialization".into()),
                    Some("preference like dislike communication style".into()),
                ),
                MemoryGatewaySubject::Routine => (
                    Some("memory_gateway_subject".into()),
                    Some("memory_write".into()),
                    Some("memory.routine".into()),
                    Some("routine".into()),
                    Some("local_memory_materialization".into()),
                    Some("routine habit schedule pattern".into()),
                ),
                MemoryGatewaySubject::FuturePlanRule => (
                    Some("memory_gateway_subject".into()),
                    Some("memory_write".into()),
                    Some("memory.procedural_rule".into()),
                    Some("procedural_rule".into()),
                    Some("proposal_review_required".into()),
                    Some("以后 下次 做计划时 按这个规则 future planning rule".into()),
                ),
                MemoryGatewaySubject::Evidence => (
                    Some("memory_gateway_subject".into()),
                    Some("memory_write".into()),
                    Some("memory.evidence".into()),
                    Some("evidence_record".into()),
                    Some("local_memory_materialization".into()),
                    None,
                ),
                MemoryGatewaySubject::CanonicalLifeModel => (
                    Some("memory_gateway_subject".into()),
                    Some("life_model_update".into()),
                    Some("lifemodel.canonical".into()),
                    Some("canonical_lifemodel_truth".into()),
                    Some("proposal_review_required".into()),
                    None,
                ),
                MemoryGatewaySubject::ManualIndexedNote => (
                    Some("manual".into()),
                    Some("memory_write".into()),
                    Some("memory.manual_indexed_note".into()),
                    Some("manual_indexed_note".into()),
                    Some("local_memory_materialization".into()),
                    None,
                ),
                MemoryGatewaySubject::ImportedArchive => (
                    Some("import".into()),
                    Some("memory_write".into()),
                    Some("memory.imported_archive".into()),
                    Some("imported_archive".into()),
                    Some("governed_import_restore".into()),
                    None,
                ),
            };
        let content_preview_digest = payload_text.as_deref().map(digest_label);
        Self {
            source,
            proposal_type,
            affected_path,
            payload_kind,
            content_preview_digest,
            user_intent_kind,
            evidence_refs: Vec::new(),
            payload_text,
        }
    }

    pub fn from_proposal(
        proposal: &AgentProposal,
        after: &serde_json::Value,
        user_intent_kind: impl Into<String>,
        evidence_refs: Vec<String>,
    ) -> Self {
        let payload_text = payload_text_for_classification(after);
        Self {
            source: Some(proposal.source.to_string()),
            proposal_type: Some(proposal.proposal_type.to_string()),
            affected_path: Some(proposal.affected_path.clone()),
            payload_kind: Some(payload_kind(after).to_string()),
            content_preview_digest: payload_text.as_deref().map(digest_label),
            user_intent_kind: Some(user_intent_kind.into()),
            evidence_refs,
            payload_text,
        }
    }

    pub fn with_payload_text(mut self, payload_text: impl Into<String>) -> Self {
        let payload_text = payload_text.into();
        self.content_preview_digest = Some(digest_label(&payload_text));
        self.payload_text = Some(payload_text);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryGatewayDecision {
    pub lane: MemoryLane,
    pub status: MemoryGatewayWriteStatus,
    pub local_memory_allowed: bool,
    pub evidence_required: bool,
    pub proposal_required: bool,
    pub approval_required: bool,
    pub canonical_lifemodel_allowed: bool,
    pub reason_code: String,
}

impl MemoryGatewayDecision {
    fn new(
        lane: MemoryLane,
        status: MemoryGatewayWriteStatus,
        reason_code: impl Into<String>,
    ) -> Self {
        Self {
            lane,
            status,
            local_memory_allowed: false,
            evidence_required: false,
            proposal_required: false,
            approval_required: false,
            canonical_lifemodel_allowed: false,
            reason_code: reason_code.into(),
        }
    }

    fn local(mut self) -> Self {
        self.local_memory_allowed = true;
        self
    }

    fn evidence(mut self) -> Self {
        self.evidence_required = true;
        self
    }

    fn proposal(mut self) -> Self {
        self.proposal_required = true;
        self.approval_required = true;
        self
    }

    fn canonical(mut self) -> Self {
        self.canonical_lifemodel_allowed = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryGatewayReadModel {
    pub remembered_what: Vec<String>,
    pub context_only: Vec<String>,
    pub proposal_required: Vec<String>,
    pub canonical_lifemodel_written: Vec<String>,
}

impl MemoryGatewayReadModel {
    pub fn from_decisions(decisions: &[MemoryGatewayDecision]) -> Self {
        let mut model = Self {
            remembered_what: Vec::new(),
            context_only: Vec::new(),
            proposal_required: Vec::new(),
            canonical_lifemodel_written: Vec::new(),
        };

        for decision in decisions {
            let label = format!("{}:{}", decision.lane.as_str(), decision.reason_code);
            match decision.status {
                MemoryGatewayWriteStatus::ContextOnly => model.context_only.push(label),
                MemoryGatewayWriteStatus::LocalMemoryWritten => model.remembered_what.push(label),
                MemoryGatewayWriteStatus::ProposalRequired => model.proposal_required.push(label),
                MemoryGatewayWriteStatus::CanonicalLifeModelWritten => {
                    model.canonical_lifemodel_written.push(label)
                }
                MemoryGatewayWriteStatus::Blocked => {}
            }
        }

        model
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryGateway;

impl MemoryGateway {
    pub fn decide(subject: MemoryGatewaySubject) -> MemoryGatewayDecision {
        Self::decide_request(&MemoryGatewayRequest::from_subject(subject))
    }

    pub fn decide_request(request: &MemoryGatewayRequest) -> MemoryGatewayDecision {
        let haystack = request_haystack(request);
        let accepted_materialization = request
            .user_intent_kind
            .as_deref()
            .is_some_and(|intent| intent == "accepted_proposal_materialization");

        if is_chat_turn(request) {
            return MemoryGatewayDecision::new(
                MemoryLane::TurnContext,
                MemoryGatewayWriteStatus::ContextOnly,
                "turn_context_not_long_term_truth",
            )
            .local();
        }

        if is_canonical_lifemodel_request(request, &haystack) {
            return MemoryGatewayDecision::new(
                MemoryLane::CanonicalLifeModelTruth,
                MemoryGatewayWriteStatus::ProposalRequired,
                "canonical_lifemodel_truth_requires_lifemodel_write_gateway",
            )
            .evidence()
            .proposal();
        }

        if contains_any(
            &haystack,
            &["evidence", "evidence_record", "证据", "source evidence"],
        ) {
            return MemoryGatewayDecision::new(
                MemoryLane::EvidenceRecord,
                MemoryGatewayWriteStatus::LocalMemoryWritten,
                "metadata_safe_evidence_record_allowed",
            )
            .local();
        }

        if contains_any(
            &haystack,
            &[
                "以后",
                "下次",
                "做计划时",
                "做规划时",
                "按这个规则",
                "规则",
                "future plan",
                "future planning",
                "next time",
                "from now on",
                "whenever",
                "when planning",
                "planning rule",
                "procedural_rule",
            ],
        ) {
            if accepted_materialization {
                return MemoryGatewayDecision::new(
                    MemoryLane::ProceduralRule,
                    MemoryGatewayWriteStatus::LocalMemoryWritten,
                    "accepted_procedural_rule_materialized_after_review",
                )
                .local()
                .evidence();
            }
            return MemoryGatewayDecision::new(
                MemoryLane::ProceduralRule,
                MemoryGatewayWriteStatus::ProposalRequired,
                "procedural_rule_requires_review_proposal",
            )
            .evidence()
            .proposal();
        }

        if contains_any(
            &haystack,
            &[
                "food",
                "meal",
                "diet",
                "breakfast",
                "lunch",
                "dinner",
                "calorie",
                "饮食",
                "吃",
                "早餐",
                "午餐",
                "晚餐",
                "health",
                "state",
                "body",
                "sleep",
                "energy",
                "健康",
                "身体",
                "睡眠",
                "精力",
                "低能量",
            ],
        ) {
            return MemoryGatewayDecision::new(
                MemoryLane::EpisodicLifeEvent,
                MemoryGatewayWriteStatus::LocalMemoryWritten,
                "local_episodic_memory_allowed",
            )
            .local()
            .evidence();
        }

        if contains_any(
            &haystack,
            &[
                "preference",
                "prefer",
                "like",
                "dislike",
                "communication style",
                "routine",
                "habit",
                "schedule pattern",
                "偏好",
                "喜欢",
                "不喜欢",
                "沟通风格",
                "习惯",
                "routine",
                "日常",
            ],
        ) {
            return MemoryGatewayDecision::new(
                MemoryLane::SemanticFactPreference,
                MemoryGatewayWriteStatus::LocalMemoryWritten,
                "semantic_memory_allowed_until_canonical_threshold",
            )
            .local()
            .evidence();
        }

        MemoryGatewayDecision::new(
            MemoryLane::SemanticFactPreference,
            MemoryGatewayWriteStatus::LocalMemoryWritten,
            "local_memory_default_semantic_fact",
        )
        .local()
        .evidence()
    }

    pub fn canonical_write_materialized() -> MemoryGatewayDecision {
        MemoryGatewayDecision::new(
            MemoryLane::CanonicalLifeModelTruth,
            MemoryGatewayWriteStatus::CanonicalLifeModelWritten,
            "accepted_proposal_materialized_canonical_lifemodel",
        )
        .evidence()
        .canonical()
    }
}

fn payload_kind(value: &serde_json::Value) -> &'static str {
    if value.is_string() {
        "string"
    } else if value.get("content").is_some() {
        "content"
    } else if value.get("requestedChange").is_some() || value.get("requested_change").is_some() {
        "requested_change"
    } else if value.is_object() {
        "structured_object"
    } else if value.is_array() {
        "array"
    } else {
        "scalar"
    }
}

fn payload_text_for_classification(value: &serde_json::Value) -> Option<String> {
    let mut values = Vec::new();
    collect_payload_text(value, &mut values);
    let joined = values.join(" ");
    if joined.trim().is_empty() {
        None
    } else {
        Some(joined.chars().take(1024).collect())
    }
}

fn collect_payload_text(value: &serde_json::Value, values: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => values.push(text.clone()),
        serde_json::Value::Array(items) => {
            for item in items.iter().take(16) {
                collect_payload_text(item, values);
            }
        }
        serde_json::Value::Object(map) => {
            for key in [
                "content",
                "requestedChange",
                "requested_change",
                "summary",
                "source",
                "category",
                "kind",
                "payloadSummary",
                "payload_summary",
                "memoryKind",
                "memory_kind",
                "userIntentKind",
                "user_intent_kind",
            ] {
                if let Some(value) = map.get(key) {
                    collect_payload_text(value, values);
                }
            }
        }
        _ => {}
    }
}

fn request_haystack(request: &MemoryGatewayRequest) -> String {
    [
        request.source.as_deref(),
        request.proposal_type.as_deref(),
        request.affected_path.as_deref(),
        request.payload_kind.as_deref(),
        request.user_intent_kind.as_deref(),
        request.payload_text.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase()
}

fn is_chat_turn(request: &MemoryGatewayRequest) -> bool {
    request
        .user_intent_kind
        .as_deref()
        .is_some_and(|intent| intent == "turn_context_capture")
        || request
            .affected_path
            .as_deref()
            .is_some_and(|path| path.starts_with("turn."))
}

fn is_canonical_lifemodel_request(request: &MemoryGatewayRequest, haystack: &str) -> bool {
    let proposal_type = request.proposal_type.as_deref().unwrap_or_default();
    matches!(
        proposal_type,
        "life_model_update"
            | "goal_update"
            | "state_update"
            | "preference_update"
            | "capability_update"
    ) || haystack.contains("lifemodel")
        || haystack.contains("life_model")
        || haystack.contains("canonical_lifemodel")
        || request
            .affected_path
            .as_deref()
            .is_some_and(canonical_lifemodel_path)
}

fn canonical_lifemodel_path(path: &str) -> bool {
    let path = path.trim_start_matches('/').to_ascii_lowercase();
    path.starts_with("identity.")
        || path.starts_with("goals.")
        || path.starts_with("state.")
        || path.starts_with("preferences.")
        || path.starts_with("capabilities.")
        || path.starts_with("evolution_rules")
        || path.starts_with("lifemodel.")
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn digest_label(input: &str) -> String {
    let bytes = input.as_bytes();
    let digest = digest(&SHA256, bytes);
    format!(
        "bytes:{} hash:sha256:{}",
        bytes.len(),
        digest
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

#[cfg(test)]
mod memory_gateway_tests {
    use super::*;

    #[test]
    fn memory_gateway_diet_event_is_local_episodic_memory() {
        let request = MemoryGatewayRequest::from_subject(MemoryGatewaySubject::FoodEvent)
            .with_payload_text("早餐吃了燕麦和咖啡，记录饮食事件");
        let decision = MemoryGateway::decide_request(&request);

        assert_eq!(decision.lane, MemoryLane::EpisodicLifeEvent);
        assert_eq!(
            decision.status,
            MemoryGatewayWriteStatus::LocalMemoryWritten
        );
        assert!(decision.local_memory_allowed);
        assert!(decision.evidence_required);
        assert!(!decision.proposal_required);
        assert!(!decision.canonical_lifemodel_allowed);
    }

    #[test]
    fn memory_gateway_preference_enters_semantic_memory_not_canonical_truth() {
        let request = MemoryGatewayRequest::from_subject(MemoryGatewaySubject::Preference)
            .with_payload_text("User prefers concise communication style.");
        let decision = MemoryGateway::decide_request(&request);

        assert_eq!(decision.lane, MemoryLane::SemanticFactPreference);
        assert_eq!(
            decision.status,
            MemoryGatewayWriteStatus::LocalMemoryWritten
        );
        assert!(decision.local_memory_allowed);
        assert!(!decision.proposal_required);
        assert!(!decision.canonical_lifemodel_allowed);
    }

    #[test]
    fn memory_gateway_future_plan_rules_require_proposal() {
        let request = MemoryGatewayRequest::from_subject(MemoryGatewaySubject::FuturePlanRule)
            .with_payload_text("以后做计划时按这个规则安排低能量任务");
        let decision = MemoryGateway::decide_request(&request);

        assert_eq!(decision.lane, MemoryLane::ProceduralRule);
        assert_eq!(decision.status, MemoryGatewayWriteStatus::ProposalRequired);
        assert!(decision.proposal_required);
        assert!(decision.approval_required);
        assert!(!decision.local_memory_allowed);
        assert!(!decision.canonical_lifemodel_allowed);
    }

    #[test]
    fn memory_gateway_read_model_answers_memory_status_categories() {
        let decisions = vec![
            MemoryGateway::decide(MemoryGatewaySubject::ChatTurn),
            MemoryGateway::decide(MemoryGatewaySubject::Preference),
            MemoryGateway::decide(MemoryGatewaySubject::FuturePlanRule),
            MemoryGateway::canonical_write_materialized(),
        ];

        let read_model = MemoryGatewayReadModel::from_decisions(&decisions);

        assert_eq!(read_model.context_only.len(), 1);
        assert_eq!(read_model.remembered_what.len(), 1);
        assert_eq!(read_model.proposal_required.len(), 1);
        assert_eq!(read_model.canonical_lifemodel_written.len(), 1);
    }

    #[test]
    fn memory_gateway_classifies_real_memory_write_payload_lanes() {
        let food = AgentProposal::new(
            crate::agent::ProposalType::MemoryWrite,
            "memory.records",
            serde_json::json!({"content": "午餐吃了沙拉，下午精力更稳定", "source": "review_center"}),
            "remember diet event",
            0.8,
            crate::agent::RiskLevel::Low,
            crate::agent::ProposalSource::Manual,
        );
        let food_request = MemoryGatewayRequest::from_proposal(
            &food,
            &food.after,
            "accepted_proposal_materialization",
            vec!["evidence:diet-event".into()],
        );
        let food_decision = MemoryGateway::decide_request(&food_request);
        assert_eq!(food_decision.lane, MemoryLane::EpisodicLifeEvent);

        let preference = AgentProposal::new(
            crate::agent::ProposalType::MemoryWrite,
            "memory.records",
            serde_json::json!({"content": "User likes short status updates.", "source": "review_center"}),
            "remember preference",
            0.8,
            crate::agent::RiskLevel::Low,
            crate::agent::ProposalSource::Manual,
        );
        let preference_request = MemoryGatewayRequest::from_proposal(
            &preference,
            &preference.after,
            "accepted_proposal_materialization",
            Vec::new(),
        );
        let preference_decision = MemoryGateway::decide_request(&preference_request);
        assert_eq!(preference_decision.lane, MemoryLane::SemanticFactPreference);
    }

    #[test]
    fn memory_gateway_future_planning_rule_uses_procedural_lane_until_accepted() {
        let proposal = AgentProposal::new(
            crate::agent::ProposalType::MemoryWrite,
            "memory.rules.planning",
            serde_json::json!({"content": "以后做计划时，先安排最难的任务。", "source": "review_center"}),
            "remember planning rule",
            0.8,
            crate::agent::RiskLevel::Medium,
            crate::agent::ProposalSource::Manual,
        );
        let review_request = MemoryGatewayRequest::from_proposal(
            &proposal,
            &proposal.after,
            "proposal_review_required",
            Vec::new(),
        );
        let review_decision = MemoryGateway::decide_request(&review_request);
        assert_eq!(review_decision.lane, MemoryLane::ProceduralRule);
        assert_eq!(
            review_decision.status,
            MemoryGatewayWriteStatus::ProposalRequired
        );

        let accepted_request = MemoryGatewayRequest::from_proposal(
            &proposal,
            &proposal.after,
            "accepted_proposal_materialization",
            Vec::new(),
        );
        let accepted_decision = MemoryGateway::decide_request(&accepted_request);
        assert_eq!(accepted_decision.lane, MemoryLane::ProceduralRule);
        assert_eq!(
            accepted_decision.status,
            MemoryGatewayWriteStatus::LocalMemoryWritten
        );
    }
}
