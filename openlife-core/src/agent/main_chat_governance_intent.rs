use serde::{Deserialize, Serialize};

use super::main_chat_memory_candidate::{
    extract_main_chat_memory_candidates, route_memory_candidates, MainChatMemoryRoutingResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainChatDurableWriteRequirement {
    MemoryProposal,
    LifeModelProposal,
}

impl MainChatDurableWriteRequirement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemoryProposal => "memory_proposal",
            Self::LifeModelProposal => "life_model_proposal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainChatExternalReadRequirement {
    CurrentExternalFactRead,
}

impl MainChatExternalReadRequirement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CurrentExternalFactRead => "current_external_fact_read",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainChatBlockerRequirement {
    DangerousLocalWrite,
    ExternalWriteConfirmation,
}

impl MainChatBlockerRequirement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DangerousLocalWrite => "dangerous_local_write",
            Self::ExternalWriteConfirmation => "external_write_confirmation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatGovernanceIntent {
    pub durable_write_requirement: Option<MainChatDurableWriteRequirement>,
    pub external_read_requirement: Option<MainChatExternalReadRequirement>,
    pub blocker_requirement: Option<MainChatBlockerRequirement>,
    pub reason_codes: Vec<String>,
    pub matched_terms: Vec<String>,
    pub confidence: f32,
}

impl MainChatGovernanceIntent {
    fn empty() -> Self {
        Self {
            durable_write_requirement: None,
            external_read_requirement: None,
            blocker_requirement: None,
            reason_codes: Vec::new(),
            matched_terms: Vec::new(),
            confidence: 0.0,
        }
    }

    pub fn requires_governance(&self) -> bool {
        self.durable_write_requirement.is_some()
            || self.external_read_requirement.is_some()
            || self.blocker_requirement.is_some()
    }
}

pub fn classify_main_chat_governance_intent(user_text: &str) -> MainChatGovernanceIntent {
    let normalized = normalize_for_matching(user_text);
    if normalized.is_empty() {
        return MainChatGovernanceIntent::empty();
    }

    let mut intent = MainChatGovernanceIntent::empty();
    collect_blocker_requirement(&normalized, &mut intent);
    let candidates = extract_main_chat_memory_candidates(user_text);
    let memory_routing = route_memory_candidates(&candidates);
    collect_durable_write_requirement_from_memory_routing(&memory_routing, &mut intent);
    collect_durable_write_requirement_from_knowledge_asset_operation(&normalized, &mut intent);
    collect_external_read_requirement(&normalized, &mut intent);

    intent.matched_terms.sort();
    intent.matched_terms.dedup();
    intent.reason_codes.sort();
    intent.reason_codes.dedup();
    intent.confidence = intent.confidence.clamp(0.0, 0.99);
    intent
}

fn collect_blocker_requirement(normalized: &str, intent: &mut MainChatGovernanceIntent) {
    if contains_any(
        normalized,
        &[
            "rm -rf",
            "shell.destructive",
            "drop database",
            "format disk",
            "delete project files",
            "删除项目文件",
            "清空数据库",
            "格式化硬盘",
        ],
    ) || (normalized.contains("shell") && contains_any(normalized, &["delete", "destroy"]))
    {
        set_blocker_requirement(
            intent,
            MainChatBlockerRequirement::DangerousLocalWrite,
            "dangerous_local_write_intent",
            "dangerous local write",
            0.98,
        );
        return;
    }

    if is_external_write_confirmation_intent(normalized) {
        set_blocker_requirement(
            intent,
            MainChatBlockerRequirement::ExternalWriteConfirmation,
            "external_write_confirmation_required",
            matched_external_write_term(normalized).unwrap_or("external write"),
            0.95,
        );
    }
}

fn collect_durable_write_requirement_from_memory_routing(
    memory_routing: &MainChatMemoryRoutingResult,
    intent: &mut MainChatGovernanceIntent,
) {
    if !memory_routing.lifemodel_proposal_candidate_ids.is_empty() {
        set_durable_write_requirement(
            intent,
            MainChatDurableWriteRequirement::LifeModelProposal,
            "memory_candidate_lifemodel_proposal_required",
            "life_model_candidate",
            0.92,
        );
        return;
    }

    if !memory_routing.memory_proposal_candidate_ids.is_empty() {
        set_durable_write_requirement(
            intent,
            MainChatDurableWriteRequirement::MemoryProposal,
            "memory_candidate_memory_proposal_required",
            "memory_candidate",
            0.92,
        );
    }
}

fn collect_durable_write_requirement_from_knowledge_asset_operation(
    normalized: &str,
    intent: &mut MainChatGovernanceIntent,
) {
    if intent.durable_write_requirement.is_some() || !is_knowledge_asset_edit_proposal(normalized) {
        return;
    }

    set_durable_write_requirement(
        intent,
        MainChatDurableWriteRequirement::LifeModelProposal,
        "knowledge_asset_edit_proposal_required",
        "knowledge_asset_edit",
        0.9,
    );
}

fn is_knowledge_asset_edit_proposal(normalized: &str) -> bool {
    contains_any(
        normalized,
        &[
            "propose an edit",
            "propose edit",
            "edit a knowledge asset",
            "edit knowledge asset",
            "edit agents.md",
            "edit soul.md",
            "edit user.md",
            "edit memory.md",
            "修改知识资产",
            "提议修改",
        ],
    ) && contains_any(
        normalized,
        &[
            "knowledge asset",
            "agents.md",
            "soul.md",
            "user.md",
            "memory.md",
            "知识资产",
        ],
    )
}

fn collect_external_read_requirement(normalized: &str, intent: &mut MainChatGovernanceIntent) {
    if is_pure_hypothetical_plan(normalized) {
        return;
    }

    let explicit_web_terms = [
        "web.read",
        "web search",
        "web.search",
        "search web",
        "lookup",
        "look up",
        "查一下",
        "查询",
        "查查",
        "帮我查",
        "帮我看一下",
        "帮我看看",
    ];
    let current_fact_terms = [
        "weather",
        "rain",
        "traffic",
        "open now",
        "business hours",
        "opening hours",
        "reservation",
        "tickets",
        "price",
        "exchange rate",
        "news",
        "flight",
        "天气",
        "下雨",
        "雨",
        "带伞",
        "路况",
        "营业",
        "开门",
        "关门",
        "开放时间",
        "开馆",
        "闭馆",
        "预约",
        "门票",
        "票价",
        "展览",
        "入馆",
        "价格",
        "汇率",
        "新闻",
        "航班",
    ];
    let temporal_terms = [
        "today",
        "tomorrow",
        "now",
        "current",
        "latest",
        "今天",
        "明天",
        "现在",
        "当前",
        "实时",
        "最新",
        "今晚",
        "今天晚上",
        "明早",
        "这个周末",
    ];
    let request_terms = [
        "should i",
        "do i need",
        "will it",
        "is it",
        "what is",
        "what's",
        "whats",
        "can you check",
        "please check",
        "please look up",
        "tell me",
        "tell us",
        "查",
        "看一下",
        "看看",
        "告诉",
        "请告诉",
        "说一下",
        "说说",
        "会不会",
        "要不要",
        "需不需要",
        "能不能",
        "是否",
        "有没有",
        "怎么预约",
        "如何预约",
        "怎么样",
        "几点",
        "多少",
    ];
    let public_venue_terms = [
        "博物馆",
        "博物院",
        "四川博物院",
        "museum",
        "gallery",
        "opening hours",
        "reservation",
        "tickets",
        "开放时间",
        "预约",
        "门票",
    ];

    let explicit_web_read = contains_any(normalized, &explicit_web_terms);
    let current_fact = contains_any(normalized, &current_fact_terms);
    let temporal = contains_any(normalized, &temporal_terms);
    let request = contains_any(normalized, &request_terms) || normalized.contains('?');
    let public_venue_read = contains_any(normalized, &public_venue_terms)
        && contains_any(
            normalized,
            &[
                "开放时间",
                "预约",
                "门票",
                "票价",
                "开馆",
                "闭馆",
                "opening hours",
                "reservation",
                "tickets",
            ],
        );

    if explicit_web_read && (current_fact || temporal || request)
        || current_fact && temporal && request
        || public_venue_read
    {
        set_external_read_requirement(
            intent,
            MainChatExternalReadRequirement::CurrentExternalFactRead,
            "current_external_fact_read_required",
            first_matched_term(normalized, &current_fact_terms)
                .or_else(|| first_matched_term(normalized, &explicit_web_terms))
                .unwrap_or("external fact"),
            0.9,
        );
    }
}

fn set_durable_write_requirement(
    intent: &mut MainChatGovernanceIntent,
    requirement: MainChatDurableWriteRequirement,
    reason_code: &str,
    matched_term: &str,
    confidence: f32,
) {
    intent.durable_write_requirement = Some(requirement);
    push_reason(intent, reason_code, matched_term, confidence);
}

fn set_external_read_requirement(
    intent: &mut MainChatGovernanceIntent,
    requirement: MainChatExternalReadRequirement,
    reason_code: &str,
    matched_term: &str,
    confidence: f32,
) {
    intent.external_read_requirement = Some(requirement);
    push_reason(intent, reason_code, matched_term, confidence);
}

fn set_blocker_requirement(
    intent: &mut MainChatGovernanceIntent,
    requirement: MainChatBlockerRequirement,
    reason_code: &str,
    matched_term: &str,
    confidence: f32,
) {
    intent.blocker_requirement = Some(requirement);
    push_reason(intent, reason_code, matched_term, confidence);
}

fn push_reason(
    intent: &mut MainChatGovernanceIntent,
    reason_code: &str,
    matched_term: &str,
    confidence: f32,
) {
    intent.reason_codes.push(reason_code.to_string());
    intent.matched_terms.push(matched_term.to_string());
    intent.confidence = intent.confidence.max(confidence);
}

fn normalize_for_matching(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn first_matched_term<'a>(value: &str, needles: &'a [&str]) -> Option<&'a str> {
    needles
        .iter()
        .copied()
        .find(|needle| value.contains(needle))
}

fn matched_external_write_term(value: &str) -> Option<&'static str> {
    const DIRECT_EXTERNAL_WRITE_TERMS: &[&str] = &[
        "skill that is not selected",
        "unselected skill",
        "not selected skill",
        "send email",
        "send an email",
        "email.send",
        "external write",
        "provider write",
        "send this to",
        "post to",
        "publish to",
        "forward to",
        "share with",
        "submit to",
        "calendar event",
        "create calendar",
        "add to calendar",
        "发给",
        "发送",
        "发邮件",
        "发送邮件",
        "发布",
        "发布到",
        "转发",
        "转发给",
        "提交",
        "提交到",
        "加到日历",
        "创建日历",
        "安排会议",
    ];
    first_matched_term(value, DIRECT_EXTERNAL_WRITE_TERMS).or_else(|| {
        [
            "send", "email", "publish", "post", "forward", "share", "submit",
        ]
        .into_iter()
        .find(|term| ascii_write_action_has_external_target(value, term))
    })
}

fn is_external_write_confirmation_intent(normalized: &str) -> bool {
    if is_external_write_planning_only(normalized) {
        return false;
    }

    if contains_any(
        normalized,
        &[
            "skill that is not selected",
            "unselected skill",
            "not selected skill",
        ],
    ) {
        return true;
    }

    if is_read_only_calendar_expression(normalized)
        && !is_calendar_write_intent(normalized)
        && !has_explicit_external_write_phrase(normalized)
    {
        return false;
    }

    if has_explicit_external_write_phrase(normalized) || is_calendar_write_intent(normalized) {
        return true;
    }

    [
        "send", "email", "publish", "post", "forward", "share", "submit",
    ]
    .into_iter()
    .any(|term| ascii_write_action_has_external_target(normalized, term))
}

fn has_explicit_external_write_phrase(normalized: &str) -> bool {
    contains_any(
        normalized,
        &[
            "send email",
            "send an email",
            "email.send",
            "external write",
            "provider write",
            "send this to",
            "post to",
            "publish to",
            "forward to",
            "share with",
            "submit to",
            "发给",
            "发送给",
            "发邮件",
            "发送邮件",
            "发布到",
            "转发给",
            "提交到",
        ],
    )
}

fn ascii_write_action_has_external_target(normalized: &str, action: &str) -> bool {
    if !contains_ascii_word(normalized, action) {
        return false;
    }
    if action == "email" {
        return normalized.starts_with("email ")
            || normalized.contains(" email to ")
            || normalized.contains(" email my ")
            || normalized.contains(" email the ")
            || normalized.contains(" email someone")
            || normalized.contains(" email coworker")
            || normalized.contains(" email client");
    }

    normalized.match_indices(action).any(|(start, _)| {
        let end = start + action.len();
        let before = normalized[..start].chars().next_back();
        let after = normalized[end..].chars().next();
        if before.is_some_and(is_ascii_word_char) || after.is_some_and(is_ascii_word_char) {
            return false;
        }
        let tail = &normalized[end..];
        tail.contains(" to ")
            || tail.contains(" with ")
            || tail.contains(" into ")
            || tail.contains(" onto ")
            || tail.contains(" external destination")
            || tail.contains(" coworker")
            || tail.contains(" slack")
            || tail.contains(" client")
            || tail.contains(" recipient")
            || tail.contains(" provider workspace")
    })
}

fn is_calendar_write_intent(normalized: &str) -> bool {
    contains_any(normalized, &["calendar", "日历"])
        && (["put", "add", "create", "schedule"]
            .into_iter()
            .any(|term| contains_ascii_word(normalized, term))
            || contains_any(normalized, &["加到", "创建", "安排会议"]))
}

fn is_external_write_planning_only(normalized: &str) -> bool {
    (contains_ascii_word(normalized, "plan")
        || contains_any(normalized, &["planning", "计划", "规划"]))
        && contains_any(
            normalized,
            &[
                "ask me before",
                "before any",
                "before executing",
                "do not execute",
                "do not send",
                "do not publish",
                "先问我",
                "先确认",
                "不要执行",
                "不要发送",
                "不要发布",
            ],
        )
}

fn is_read_only_calendar_expression(normalized: &str) -> bool {
    contains_any(
        normalized,
        &[
            "calendar.read",
            "read calendar",
            "read my calendar",
            "calendar read",
            "查询日程",
            "查看日程",
            "读取日历",
        ],
    )
}

fn contains_ascii_word(value: &str, needle: &str) -> bool {
    value.match_indices(needle).any(|(start, _)| {
        let end = start + needle.len();
        let before = value[..start].chars().next_back();
        let after = value[end..].chars().next();
        !before.is_some_and(is_ascii_word_char) && !after.is_some_and(is_ascii_word_char)
    })
}

fn is_ascii_word_char(value: char) -> bool {
    value.is_ascii_alphanumeric() || value == '_'
}

fn is_pure_hypothetical_plan(normalized: &str) -> bool {
    (normalized.contains("如果") || normalized.contains("假如") || normalized.contains("if "))
        && contains_any(normalized, &["就", "then", "改", "安排", "计划", "plan"])
        && !contains_any(
            normalized,
            &[
                "查",
                "看一下",
                "看看",
                "会不会",
                "要不要",
                "需不需要",
                "是否",
                "有没有",
                "should i",
                "do i need",
                "?",
            ],
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_chat_governance_intent_classifies_chinese_memory_request() {
        let intent = classify_main_chat_governance_intent(
            "这条对我挺重要：空腹喝咖啡会让我心慌，尤其是在赶路的时候。帮我记下来，下次提醒我先吃点东西。",
        );

        assert_eq!(
            intent.durable_write_requirement,
            Some(MainChatDurableWriteRequirement::LifeModelProposal)
        );
        assert!(intent.external_read_requirement.is_none());
        assert!(intent
            .reason_codes
            .contains(&"memory_candidate_lifemodel_proposal_required".to_string()));
        assert!(intent
            .matched_terms
            .iter()
            .any(|term| term == "life_model_candidate"));
        assert!(intent.confidence >= 0.9);
    }

    #[test]
    fn main_chat_governance_intent_classifies_future_preference_request() {
        let intent = classify_main_chat_governance_intent(
            "以后如果我说空腹喝了咖啡，你优先提醒我先吃点东西。",
        );

        assert_eq!(
            intent.durable_write_requirement,
            Some(MainChatDurableWriteRequirement::LifeModelProposal)
        );
        assert!(intent
            .reason_codes
            .contains(&"memory_candidate_lifemodel_proposal_required".to_string()));
        assert!(intent.external_read_requirement.is_none());
    }

    #[test]
    fn main_chat_governance_intent_classifies_chinese_current_weather_read() {
        let intent =
            classify_main_chat_governance_intent("帮我看一下今天上海会不会下雨，我要不要带伞");

        assert_eq!(
            intent.external_read_requirement,
            Some(MainChatExternalReadRequirement::CurrentExternalFactRead)
        );
        assert!(intent.durable_write_requirement.is_none());
        assert!(intent
            .reason_codes
            .contains(&"current_external_fact_read_required".to_string()));
    }

    #[test]
    fn main_chat_governance_intent_classifies_stage6c_native_weather_prompt() {
        let intent = classify_main_chat_governance_intent(
            "请告诉我今天旧金山的天气。必须使用可审计的 web/weather 读取证据；如果当前没有可用外部读取工具，请明确 fail closed，不要猜。",
        );

        assert_eq!(
            intent.external_read_requirement,
            Some(MainChatExternalReadRequirement::CurrentExternalFactRead)
        );
        assert!(intent.durable_write_requirement.is_none());
        assert!(intent
            .reason_codes
            .contains(&"current_external_fact_read_required".to_string()));
    }

    #[test]
    fn main_chat_governance_intent_classifies_english_live_weather_read() {
        let intent =
            classify_main_chat_governance_intent("What is the live weather in Shanghai right now?");

        assert_eq!(
            intent.external_read_requirement,
            Some(MainChatExternalReadRequirement::CurrentExternalFactRead)
        );
        assert!(intent.durable_write_requirement.is_none());
        assert!(intent
            .reason_codes
            .contains(&"current_external_fact_read_required".to_string()));
    }

    #[test]
    fn main_chat_governance_intent_keeps_weather_statement_and_hypothetical_plan_direct() {
        let statement = classify_main_chat_governance_intent("今天天气不错");
        assert!(!statement.requires_governance());

        let hypothetical = classify_main_chat_governance_intent("如果明天下雨就改室内安排");
        assert!(!hypothetical.requires_governance());
    }

    #[test]
    fn main_chat_governance_intent_routes_knowledge_asset_edit_without_intercepting_inspection() {
        let edit = classify_main_chat_governance_intent(
            "Propose an edit to SOUL.md knowledge asset wording.",
        );
        assert_eq!(
            edit.durable_write_requirement,
            Some(MainChatDurableWriteRequirement::LifeModelProposal)
        );
        assert!(edit
            .reason_codes
            .contains(&"knowledge_asset_edit_proposal_required".to_string()));

        let inspect = classify_main_chat_governance_intent("Inspect loaded knowledge assets.");
        assert!(inspect.durable_write_requirement.is_none());
    }

    #[test]
    fn main_chat_governance_intent_blocks_external_destination_publish() {
        let intent = classify_main_chat_governance_intent(
            "Publish the seeded `dogfood/policy_note.md` to a sensitive external destination named in the write-like action seed.",
        );

        assert_eq!(
            intent.blocker_requirement,
            Some(MainChatBlockerRequirement::ExternalWriteConfirmation)
        );
        assert!(intent.durable_write_requirement.is_none());
        assert!(intent
            .reason_codes
            .contains(&"external_write_confirmation_required".to_string()));
    }

    #[test]
    fn main_chat_governance_intent_blocks_sensitive_external_write_without_exact_phrase() {
        let intent = classify_main_chat_governance_intent(
            "Send this private medical update to my coworker.",
        );

        assert_eq!(
            intent.blocker_requirement,
            Some(MainChatBlockerRequirement::ExternalWriteConfirmation)
        );
        assert!(intent.durable_write_requirement.is_none());
    }

    #[test]
    fn main_chat_governance_intent_does_not_block_non_external_write_nouns() {
        for prompt in [
            "draft a blog post outline",
            "write a post-work reflection",
            "summarize this email",
            "plan a calendar for my week",
        ] {
            let intent = classify_main_chat_governance_intent(prompt);
            assert!(
                intent.blocker_requirement.is_none(),
                "{prompt} should not be treated as an external side-effect write"
            );
        }
    }

    #[test]
    fn main_chat_governance_intent_blocks_explicit_external_sensitive_writes() {
        for prompt in [
            "send my health note to my coworker",
            "publish my medical update to Slack",
            "add my therapy appointment to calendar",
        ] {
            let intent = classify_main_chat_governance_intent(prompt);
            assert_eq!(
                intent.blocker_requirement,
                Some(MainChatBlockerRequirement::ExternalWriteConfirmation),
                "{prompt} should require confirmation"
            );
        }
    }

    #[test]
    fn main_chat_governance_intent_does_not_block_calendar_read() {
        for prompt in ["calendar.read", "read calendar", "查询日程"] {
            let intent = classify_main_chat_governance_intent(prompt);
            assert!(
                intent.blocker_requirement.is_none(),
                "{prompt} should stay read-only"
            );
        }
    }

    #[test]
    fn main_chat_governance_intent_does_not_treat_arrange_today_work_as_lifemodel() {
        let intent = classify_main_chat_governance_intent("帮我安排今天下午工作");

        assert_ne!(
            intent.durable_write_requirement,
            Some(MainChatDurableWriteRequirement::LifeModelProposal)
        );
    }
}
