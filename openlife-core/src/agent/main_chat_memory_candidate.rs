use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCandidateKind {
    EpisodicLifeEvent,
    SemanticUserFact,
    ProceduralRule,
    Preference,
    IdentityOrRole,
}

impl MemoryCandidateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EpisodicLifeEvent => "episodic_life_event",
            Self::SemanticUserFact => "semantic_user_fact",
            Self::ProceduralRule => "procedural_rule",
            Self::Preference => "preference",
            Self::IdentityOrRole => "identity_or_role",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryDestination {
    SessionOnly,
    LifeEvent,
    MemoryProposal,
    LifeModelProposal,
    NoOp,
}

impl MemoryDestination {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SessionOnly => "session_only",
            Self::LifeEvent => "life_event",
            Self::MemoryProposal => "memory_proposal",
            Self::LifeModelProposal => "life_model_proposal",
            Self::NoOp => "no_op",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatMemoryCandidate {
    pub candidate_id: String,
    pub source_span_id: String,
    pub kind: MemoryCandidateKind,
    pub destination: MemoryDestination,
    pub evidence_text: String,
    pub source_preview: String,
    pub normalized_claim: String,
    pub sensitivity: String,
    pub stability: String,
    pub explicitness: String,
    pub future_actionability: String,
    pub confidence: f32,
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatMemoryRoutingResult {
    pub candidates: Vec<MainChatMemoryCandidate>,
    pub life_event_candidate_ids: Vec<String>,
    pub memory_proposal_candidate_ids: Vec<String>,
    pub lifemodel_proposal_candidate_ids: Vec<String>,
    pub session_only_candidate_ids: Vec<String>,
    pub no_op_candidate_ids: Vec<String>,
    pub blockers: Vec<String>,
}

pub fn extract_main_chat_memory_candidates(user_text: &str) -> Vec<MainChatMemoryCandidate> {
    let normalized = compact_text(user_text);
    if normalized.is_empty() {
        return Vec::new();
    }
    if is_current_external_fact_request(&normalized) {
        return Vec::new();
    }

    let spans = split_spans(user_text);
    let has_future_rule_any = spans
        .iter()
        .map(|span| compact_text(span).to_ascii_lowercase())
        .any(|span| is_future_rule(&span));
    let mut candidates = Vec::new();
    let mut previous_memory_spans: Vec<String> = Vec::new();

    for (index, span) in spans.iter().enumerate() {
        let compact = compact_text(span);
        if compact.is_empty() {
            continue;
        }
        let lower = compact.to_ascii_lowercase();
        let span_id = source_span_id(index, &compact);
        let explicit_memory = has_explicit_memory_marker(&lower);
        let future_rule = is_future_rule(&lower);
        let identity_or_preference = is_identity_or_long_term_preference(&lower);
        let no_op_weather_statement = is_weather_statement_only(&lower);
        let hypothetical_only = is_hypothetical_plan_only(&lower);

        if explicit_memory {
            if let Some(life_event_claim) = explicit_memory_life_event_claim(&compact) {
                let life_event_lower = life_event_claim.to_ascii_lowercase();
                if is_life_event_expression(&life_event_lower)
                    && !is_weather_statement_only(&life_event_lower)
                    && !is_hypothetical_plan_only(&life_event_lower)
                {
                    push_candidate(
                        &mut candidates,
                        &span_id,
                        MemoryCandidateKind::EpisodicLifeEvent,
                        MemoryDestination::LifeEvent,
                        &life_event_claim,
                        &normalized_claim(&life_event_claim),
                        sensitivity_for_text(&life_event_claim),
                        "episodic",
                        "explicit",
                        "local_log",
                        0.89,
                        vec![
                            "life_event_local_capture".into(),
                            "explicit_memory_same_span".into(),
                        ],
                    );
                }
            }
        }

        if future_rule {
            let claim = normalized_future_rule_claim(&compact);
            push_candidate(
                &mut candidates,
                &span_id,
                MemoryCandidateKind::ProceduralRule,
                MemoryDestination::LifeModelProposal,
                &compact,
                &claim,
                "internal",
                "stable",
                if explicit_memory {
                    "explicit"
                } else {
                    "implicit"
                },
                "future_rule",
                0.91,
                vec!["future_behavior_rule".into()],
            );
        }

        if identity_or_preference && !future_rule {
            let kind = if contains_any(&lower, &["identity", "i am", "我是", "身份"]) {
                MemoryCandidateKind::IdentityOrRole
            } else {
                MemoryCandidateKind::Preference
            };
            push_candidate(
                &mut candidates,
                &span_id,
                kind,
                MemoryDestination::LifeModelProposal,
                &compact,
                &normalized_claim(&compact),
                "internal",
                "stable",
                "explicit",
                "future_actionable",
                0.9,
                vec!["stable_identity_or_preference".into()],
            );
        }

        if has_future_rule_any
            && !explicit_memory
            && !future_rule
            && !identity_or_preference
            && !is_life_event_expression(&lower)
            && is_stable_memory_fact_expression(&lower)
        {
            push_candidate(
                &mut candidates,
                &span_id,
                MemoryCandidateKind::SemanticUserFact,
                MemoryDestination::MemoryProposal,
                &compact,
                &normalized_claim(&compact),
                sensitivity_for_text(&compact),
                "stable",
                "implicit",
                "retrieval_fact",
                0.86,
                vec!["stable_fact_supports_future_rule".into()],
            );
        }

        if explicit_memory {
            let claim = memory_claim_for_span(&compact).or_else(|| {
                (!previous_memory_spans.is_empty()).then(|| previous_memory_spans.join(" "))
            });
            if let Some(claim) = claim.filter(|value| meaningful_claim(value)) {
                push_candidate(
                    &mut candidates,
                    &span_id,
                    MemoryCandidateKind::SemanticUserFact,
                    MemoryDestination::MemoryProposal,
                    &compact,
                    &claim,
                    sensitivity_for_text(&claim),
                    "stable",
                    "explicit",
                    "retrieval_fact",
                    0.92,
                    vec!["explicit_memory_request".into()],
                );
            }
        }

        let life_event_allowed = !explicit_memory;
        if life_event_allowed
            && is_life_event_expression(&lower)
            && !no_op_weather_statement
            && !hypothetical_only
        {
            push_candidate(
                &mut candidates,
                &span_id,
                MemoryCandidateKind::EpisodicLifeEvent,
                MemoryDestination::LifeEvent,
                &compact,
                &normalized_claim(&compact),
                sensitivity_for_text(&compact),
                "episodic",
                "implicit",
                "local_log",
                0.88,
                vec!["life_event_local_capture".into()],
            );
        }

        if candidates_for_span(&candidates, &span_id).is_empty()
            && (no_op_weather_statement || hypothetical_only)
        {
            push_candidate(
                &mut candidates,
                &span_id,
                MemoryCandidateKind::EpisodicLifeEvent,
                MemoryDestination::NoOp,
                &compact,
                &normalized_claim(&compact),
                "internal",
                "unstable",
                "implicit",
                "none",
                0.82,
                vec![if no_op_weather_statement {
                    "weather_statement_no_memory".into()
                } else {
                    "hypothetical_plan_no_memory".into()
                }],
            );
        }

        if meaningful_claim(&compact)
            && !explicit_memory
            && !future_rule
            && !identity_or_preference
            && !no_op_weather_statement
            && !hypothetical_only
        {
            previous_memory_spans.push(compact);
            if previous_memory_spans.len() > 3 {
                previous_memory_spans.remove(0);
            }
        }
    }

    dedupe_candidates(candidates)
}

pub fn route_memory_candidates(
    candidates: &[MainChatMemoryCandidate],
) -> MainChatMemoryRoutingResult {
    let mut result = MainChatMemoryRoutingResult {
        candidates: candidates.to_vec(),
        ..MainChatMemoryRoutingResult::default()
    };

    for candidate in candidates {
        if candidate.confidence < 0.7
            && matches!(
                candidate.destination,
                MemoryDestination::LifeEvent
                    | MemoryDestination::MemoryProposal
                    | MemoryDestination::LifeModelProposal
            )
        {
            push_unique(&mut result.blockers, "low_confidence_candidate_not_routed");
            continue;
        }
        match candidate.destination {
            MemoryDestination::LifeEvent => push_unique(
                &mut result.life_event_candidate_ids,
                &candidate.candidate_id,
            ),
            MemoryDestination::MemoryProposal => push_unique(
                &mut result.memory_proposal_candidate_ids,
                &candidate.candidate_id,
            ),
            MemoryDestination::LifeModelProposal => push_unique(
                &mut result.lifemodel_proposal_candidate_ids,
                &candidate.candidate_id,
            ),
            MemoryDestination::SessionOnly => push_unique(
                &mut result.session_only_candidate_ids,
                &candidate.candidate_id,
            ),
            MemoryDestination::NoOp => {
                push_unique(&mut result.no_op_candidate_ids, &candidate.candidate_id)
            }
        }
    }

    result
}

pub fn plan_main_chat_memory_routing(user_text: &str) -> MainChatMemoryRoutingResult {
    let candidates = extract_main_chat_memory_candidates(user_text);
    route_memory_candidates(&candidates)
}

fn push_candidate(
    candidates: &mut Vec<MainChatMemoryCandidate>,
    source_span_id: &str,
    kind: MemoryCandidateKind,
    destination: MemoryDestination,
    evidence_text: &str,
    normalized_claim: &str,
    sensitivity: &str,
    stability: &str,
    explicitness: &str,
    future_actionability: &str,
    confidence: f32,
    reason_codes: Vec<String>,
) {
    let claim = normalized_claim.trim();
    if claim.is_empty() {
        return;
    }
    let candidate_id = candidate_id(kind, destination, claim, source_span_id);
    candidates.push(MainChatMemoryCandidate {
        candidate_id,
        source_span_id: source_span_id.to_string(),
        kind,
        destination,
        evidence_text: bounded_preview(evidence_text, 240),
        source_preview: bounded_preview(evidence_text, 120),
        normalized_claim: bounded_preview(claim, 220),
        sensitivity: sensitivity.to_string(),
        stability: stability.to_string(),
        explicitness: explicitness.to_string(),
        future_actionability: future_actionability.to_string(),
        confidence,
        reason_codes,
    });
}

fn split_spans(user_text: &str) -> Vec<String> {
    user_text
        .split(|ch| matches!(ch, '。' | '.' | '!' | '！' | ';' | '；' | '\n'))
        .map(compact_text)
        .filter(|span| !span.is_empty())
        .collect()
}

fn candidates_for_span<'a>(
    candidates: &'a [MainChatMemoryCandidate],
    source_span_id: &str,
) -> Vec<&'a MainChatMemoryCandidate> {
    candidates
        .iter()
        .filter(|candidate| candidate.source_span_id == source_span_id)
        .collect()
}

fn memory_claim_for_span(span: &str) -> Option<String> {
    let lower = span.to_ascii_lowercase();
    let triggers = [
        "please remember this",
        "remember this",
        "please remember",
        "remember that",
        "remember",
        "save this",
        "帮我记下来",
        "帮我记一下",
        "请记住",
        "记下来",
        "记一下",
        "记住",
        "加入记忆",
    ];
    for trigger in triggers {
        if let Some(pos) = lower.find(trigger) {
            let before = compact_claim(&span[..pos]);
            let after = compact_claim(&span[pos + trigger.len()..]);
            if is_deictic_memory_trigger(trigger) {
                if meaningful_claim(&before) && !is_future_rule(&before.to_ascii_lowercase()) {
                    return Some(before);
                }
                if meaningful_memory_candidate(&after)
                    && !is_future_rule(&after.to_ascii_lowercase())
                {
                    return Some(after);
                }
                return None;
            }
            if meaningful_claim(&before) && !is_future_rule(&before.to_ascii_lowercase()) {
                return Some(before);
            }
            if meaningful_claim(&after) && !is_future_rule(&after.to_ascii_lowercase()) {
                return Some(after);
            }
        }
    }
    None
}

fn is_deictic_memory_trigger(trigger: &str) -> bool {
    matches!(
        trigger,
        "please remember this" | "remember this" | "save this"
    )
}

fn explicit_memory_life_event_claim(span: &str) -> Option<String> {
    let lower = span.to_ascii_lowercase();
    for trigger in [
        "please remember this",
        "remember this",
        "please remember",
        "remember that",
        "remember",
        "save this",
        "帮我记下来",
        "帮我记一下",
        "请记住",
        "记下来",
        "记一下",
        "记住",
        "加入记忆",
    ] {
        if let Some(pos) = lower.find(trigger) {
            let before = compact_claim(&span[..pos]);
            if meaningful_claim(&before) && !is_future_rule(&before.to_ascii_lowercase()) {
                return Some(before);
            }
            return None;
        }
    }
    None
}

fn normalized_future_rule_claim(span: &str) -> String {
    let mut claim = compact_claim(span);
    for prefix in ["帮我记下来", "记下来", "请记住", "remember", "之后", "以后"] {
        if claim.to_ascii_lowercase().starts_with(prefix) {
            claim = compact_claim(&claim[prefix.len()..]);
        }
    }
    claim
}

fn normalized_claim(value: &str) -> String {
    compact_claim(value)
}

fn compact_claim(value: &str) -> String {
    compact_text(value)
        .trim_matches(|ch: char| matches!(ch, ':' | '：' | ',' | '，' | '-' | '—'))
        .trim()
        .to_string()
}

fn compact_text(value: &str) -> String {
    value
        .replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn bounded_preview(value: &str, max_chars: usize) -> String {
    let compact = compact_text(value);
    let mut result = String::new();
    for ch in compact.chars().take(max_chars) {
        if ch.is_control() {
            result.push(' ');
        } else {
            result.push(ch);
        }
    }
    if compact.chars().count() > max_chars {
        result.push('…');
    }
    result
}

fn meaningful_claim(value: &str) -> bool {
    value.chars().count() >= 4
        && !contains_any(
            &value.to_ascii_lowercase(),
            &["帮我记下来", "记下来", "please remember", "remember this"],
        )
}

fn meaningful_memory_candidate(value: &str) -> bool {
    meaningful_claim(value) && !looks_like_instruction_fragment(value)
}

fn looks_like_instruction_fragment(value: &str) -> bool {
    contains_any(
        &value.to_ascii_lowercase(),
        &[
            "locally if appropriate",
            "if appropriate",
            "please",
            "do not",
            "don't",
            "不要",
            "不允许",
        ],
    )
}

fn has_explicit_memory_marker(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "please remember",
            "remember this",
            "remember that",
            "remember",
            "save this",
            "帮我记下来",
            "帮我记一下",
            "请记住",
            "记下来",
            "记一下",
            "记住",
            "加入记忆",
        ],
    )
}

fn is_future_rule(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "以后",
            "下次",
            "往后",
            "长期",
            "以后都",
            "之后",
            "next time",
            "from now on",
            "going forward",
        ],
    ) && contains_any(
        lower,
        &[
            "优先",
            "按这个",
            "按照这个",
            "提醒",
            "先确认",
            "先看",
            "安排",
            "处理",
            "prefer",
            "remind",
            "confirm",
            "before",
        ],
    )
}

fn is_identity_or_long_term_preference(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "update my identity",
            "i am becoming",
            "i am a",
            "design lead",
            "life model",
            "lifemodel",
            "我是",
            "身份",
            "长期偏好",
            "价值观",
        ],
    ) || (contains_any(lower, &["i prefer", "我偏好", "我更喜欢"])
        && contains_any(lower, &["以后", "长期", "always", "以后都"]))
}

fn is_life_event_expression(lower: &str) -> bool {
    let has_life_fact = contains_any(
        lower,
        &[
            "今天",
            "昨晚",
            "下午",
            "上午",
            "中午",
            "午饭",
            "晚饭",
            "早饭",
            "睡",
            "情绪",
            "心情",
            "运动",
            "跑步",
            "犯困",
            "心慌",
            "头疼",
            "胃",
            "吃了",
            "喝了",
            "空腹",
            "身体",
            "today",
            "this morning",
            "yesterday",
            "last night",
            "lunch",
            "dinner",
            "breakfast",
            "coffee",
            "bread",
            "slept",
            "sleep",
            "mood",
            "exercise",
            "tired",
            "scattered",
        ],
    );
    let has_episode_marker = contains_any(
        lower,
        &[
            "今天",
            "刚刚",
            "昨晚",
            "下午",
            "上午",
            "中午",
            "午饭",
            "晚饭",
            "早饭",
            "这次",
            "today",
            "this morning",
            "yesterday",
            "last night",
            "lunch",
            "dinner",
            "breakfast",
        ],
    );
    has_life_fact && has_episode_marker && !is_current_external_fact_request(lower)
}

fn is_stable_memory_fact_expression(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "会心慌",
            "会头痛",
            "会犯困",
            "会缓解",
            "会让我",
            "容易",
            "缓解",
            "makes me",
            "helps",
            "tends to",
            "usually",
        ],
    ) && !is_current_external_fact_request(lower)
}

fn is_weather_statement_only(lower: &str) -> bool {
    contains_any(lower, &["今天天气不错", "天气不错", "weather is nice"])
        && !contains_any(lower, &["查", "看一下", "会不会", "要不要", "?"])
}

fn is_hypothetical_plan_only(lower: &str) -> bool {
    (lower.contains("如果") || lower.contains("假如") || lower.contains("if "))
        && contains_any(lower, &["就", "then", "改", "安排", "计划", "plan"])
        && !contains_any(lower, &["查", "看一下", "会不会", "要不要", "?"])
}

fn is_current_external_fact_request(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "weather",
            "rain",
            "traffic",
            "price",
            "exchange rate",
            "news",
            "flight",
            "天气",
            "下雨",
            "带伞",
            "路况",
            "价格",
            "汇率",
            "新闻",
            "航班",
        ],
    ) && contains_any(
        lower,
        &[
            "查",
            "看一下",
            "看看",
            "会不会",
            "要不要",
            "需不需要",
            "should i",
            "do i need",
            "will it",
            "current",
            "latest",
            "?",
        ],
    )
}

fn sensitivity_for_text(value: &str) -> &'static str {
    let lower = value.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "medical",
            "health",
            "private",
            "sensitive",
            "身体",
            "心慌",
            "头疼",
            "胃",
            "医疗",
            "健康",
            "隐私",
            "敏感",
        ],
    ) {
        "sensitive"
    } else {
        "internal"
    }
}

fn source_span_id(index: usize, value: &str) -> String {
    format!("span_{}_{}", index + 1, short_digest(value))
}

fn candidate_id(
    kind: MemoryCandidateKind,
    destination: MemoryDestination,
    normalized_claim: &str,
    source_span_id: &str,
) -> String {
    short_prefixed_digest(
        "mc",
        &format!(
            "{}|{}|{}|{}",
            kind.as_str(),
            destination.as_str(),
            normalized_claim,
            source_span_id
        ),
    )
}

fn short_digest(value: &str) -> String {
    short_prefixed_digest("", value)
}

fn short_prefixed_digest(prefix: &str, value: &str) -> String {
    let hash = digest(&SHA256, value.as_bytes());
    let hex = hash
        .as_ref()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if prefix.is_empty() {
        hex
    } else {
        format!("{prefix}_{hex}")
    }
}

fn dedupe_candidates(candidates: Vec<MainChatMemoryCandidate>) -> Vec<MainChatMemoryCandidate> {
    let mut deduped = Vec::new();
    for candidate in candidates {
        if !deduped.iter().any(|existing: &MainChatMemoryCandidate| {
            existing.candidate_id == candidate.candidate_id
        }) {
            deduped.push(candidate);
        }
    }
    deduped
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn routed(text: &str) -> MainChatMemoryRoutingResult {
        plan_main_chat_memory_routing(text)
    }

    #[test]
    fn main_chat_memory_candidate_routes_chinese_food_and_body_state_to_life_event() {
        let result = routed("今天午饭吃了牛肉面，下午犯困");

        assert_eq!(result.life_event_candidate_ids.len(), 1);
        assert!(result.memory_proposal_candidate_ids.is_empty());
        assert!(result.lifemodel_proposal_candidate_ids.is_empty());
        assert_eq!(
            result.candidates[0].destination,
            MemoryDestination::LifeEvent
        );
    }

    #[test]
    fn main_chat_memory_candidate_routes_explicit_user_fact_to_memory_proposal() {
        let result = routed("帮我记下来：空腹喝咖啡会心慌");

        assert_eq!(result.memory_proposal_candidate_ids.len(), 1);
        assert!(result.life_event_candidate_ids.is_empty());
        assert!(result.lifemodel_proposal_candidate_ids.is_empty());
        assert!(result
            .candidates
            .iter()
            .any(|candidate| candidate.kind == MemoryCandidateKind::SemanticUserFact));
    }

    #[test]
    fn main_chat_memory_candidate_splits_same_sentence_life_event_and_memory_request() {
        let result = routed("今天午饭吃了牛肉面，下午犯困，帮我记下来");

        assert_eq!(result.life_event_candidate_ids.len(), 1);
        assert_eq!(result.memory_proposal_candidate_ids.len(), 1);
        assert!(result.lifemodel_proposal_candidate_ids.is_empty());
        let life_event = result
            .candidates
            .iter()
            .find(|candidate| candidate.destination == MemoryDestination::LifeEvent)
            .expect("life event candidate");
        assert!(life_event.normalized_claim.contains("牛肉面"));
        assert!(life_event.normalized_claim.contains("犯困"));
    }

    #[test]
    fn main_chat_memory_candidate_splits_sleep_headache_memory_request() {
        let result = routed("今天睡了 5 小时，上午头痛，帮我记一下");

        assert_eq!(result.life_event_candidate_ids.len(), 1);
        assert_eq!(result.memory_proposal_candidate_ids.len(), 1);
        assert!(result.lifemodel_proposal_candidate_ids.is_empty());
    }

    #[test]
    fn main_chat_memory_candidate_resolves_remember_this_to_prior_facts() {
        let result = routed(
            "This morning I had coffee and bread for breakfast. I am rushing between errands and feel a bit scattered. Please remember this locally if appropriate.",
        );

        assert_eq!(result.memory_proposal_candidate_ids.len(), 1);
        let candidate = result
            .candidates
            .iter()
            .find(|candidate| candidate.destination == MemoryDestination::MemoryProposal)
            .expect("memory proposal candidate");
        assert!(candidate.normalized_claim.contains("coffee and bread"));
        assert!(candidate.normalized_claim.contains("scattered"));
        assert!(!candidate
            .normalized_claim
            .contains("locally if appropriate"));
    }

    #[test]
    fn main_chat_memory_candidate_resolves_remember_this_colon_to_following_fact() {
        let result = routed("Remember this: I prefer morning deep work.");

        assert_eq!(result.memory_proposal_candidate_ids.len(), 1);
        assert!(result.life_event_candidate_ids.is_empty());
        assert!(result.lifemodel_proposal_candidate_ids.is_empty());
        let candidate = result
            .candidates
            .iter()
            .find(|candidate| candidate.destination == MemoryDestination::MemoryProposal)
            .expect("memory proposal candidate");
        assert_eq!(candidate.normalized_claim, "I prefer morning deep work");
    }

    #[test]
    fn main_chat_memory_candidate_routes_future_rule_to_lifemodel_proposal() {
        let result = routed("以后早上安排工作前先确认我有没有吃东西");

        assert_eq!(result.lifemodel_proposal_candidate_ids.len(), 1);
        assert!(result.memory_proposal_candidate_ids.is_empty());
    }

    #[test]
    fn main_chat_memory_candidate_keeps_today_arrangement_out_of_lifemodel() {
        let result = routed("帮我安排今天下午工作");

        assert!(result.lifemodel_proposal_candidate_ids.is_empty());
        assert!(result.memory_proposal_candidate_ids.is_empty());
    }

    #[test]
    fn main_chat_memory_candidate_splits_mixed_memory_governance_artifacts() {
        let result =
            routed("空腹喝咖啡会心慌，香蕉酸奶会缓解。以后早上安排工作前先确认我有没有吃东西");

        assert!(result.life_event_candidate_ids.is_empty());
        assert_eq!(result.memory_proposal_candidate_ids.len(), 1);
        assert_eq!(result.lifemodel_proposal_candidate_ids.len(), 1);
        assert!(result.candidates.len() >= 2);
    }

    #[test]
    fn main_chat_memory_candidate_keeps_weather_statement_noop() {
        let result = routed("今天天气不错");

        assert!(result.life_event_candidate_ids.is_empty());
        assert!(result.memory_proposal_candidate_ids.is_empty());
        assert!(result.lifemodel_proposal_candidate_ids.is_empty());
        assert_eq!(result.no_op_candidate_ids.len(), 1);
    }

    #[test]
    fn main_chat_memory_candidate_external_weather_read_is_not_user_memory() {
        let result = routed("帮我看一下今天上海会不会下雨，我要不要带伞");

        assert!(result.candidates.is_empty());
        assert!(result.life_event_candidate_ids.is_empty());
        assert!(result.memory_proposal_candidate_ids.is_empty());
        assert!(result.lifemodel_proposal_candidate_ids.is_empty());
    }

    #[test]
    fn main_chat_memory_candidate_does_not_intercept_knowledge_asset_operations() {
        for text in [
            "Inspect loaded knowledge assets.",
            "Propose an edit to AGENTS.md knowledge asset: add a bounded capability evidence note.",
        ] {
            let result = routed(text);

            assert!(
                result.candidates.is_empty(),
                "knowledge asset operation should stay on the existing proposal/context path: {text}"
            );
            assert!(result.life_event_candidate_ids.is_empty());
            assert!(result.memory_proposal_candidate_ids.is_empty());
            assert!(result.lifemodel_proposal_candidate_ids.is_empty());
        }
    }
}
