use crate::agent::runtime_contract::{LifeEventDraft, RuntimeOutput};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

const DEFAULT_CONFIDENCE: f32 = 0.7;
const MIN_CONFIDENCE: f32 = 0.55;
const MIN_SUMMARY_CHARS: usize = 4;

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
    pub warnings: Vec<String>,
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
            let summary = normalize_summary(&draft.summary);
            if summary.is_empty() {
                output.warnings.push(format!(
                    "dropped empty LifeEventDraft '{}'",
                    draft.event_type
                ));
                continue;
            }
            if summary.chars().count() < self.min_summary_chars {
                output.warnings.push(format!(
                    "dropped too-short LifeEventDraft '{}'",
                    draft.event_type
                ));
                continue;
            }

            let confidence =
                confidence_from_metadata(&draft.metadata).unwrap_or(DEFAULT_CONFIDENCE);
            if confidence < self.min_confidence {
                output.warnings.push(format!(
                    "dropped low-confidence LifeEventDraft '{}' ({:.2})",
                    draft.event_type, confidence
                ));
                continue;
            }

            let source_run_id = draft.source_run_id.clone().or_else(|| input.run_id.clone());
            let dedupe_key = dedupe_key(source_run_id.as_deref(), &draft.event_type, &summary);
            if !seen.insert(dedupe_key) {
                continue;
            }

            match candidate_from_draft(&draft.event_type, &summary, confidence, source_run_id) {
                Some(candidate) => output.proposal_candidates.push(candidate),
                None => output.warnings.push(format!(
                    "unsupported LifeEventDraft type '{}'",
                    draft.event_type
                )),
            }
        }

        output
    }
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
