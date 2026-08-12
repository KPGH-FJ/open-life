use crate::life_model::v2::{LifeModelDocumentV2, LifeModelSectionV2, LifeModelVersionV2};
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const MAX_SELECTED_FACTS: usize = 4;
const MAX_FACT_CHARS: usize = 320;
const MAX_REASON_CHARS: usize = 120;
const MAX_RENDERED_PROMPT_CHARS: usize = 600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelRuntimeFactV2 {
    pub item_id: String,
    pub section: LifeModelSectionV2,
    pub value: String,
    pub source_refs: Vec<String>,
    pub confirmed_at: String,
    pub confirmation_age_days: i64,
    pub selected_reason: String,
}

/// A bounded, task-relevant projection of the canonical, user-confirmed
/// LifeModel v2 document. It is prompt context only: it cannot grant a tool,
/// reveal a credential, or authorize a durable/external effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelRuntimeContextV2 {
    pub schema: String,
    pub model_id: String,
    pub model_version: u64,
    pub version_digest: String,
    pub document_digest: String,
    pub selected_sections: Vec<LifeModelSectionV2>,
    pub facts: Vec<LifeModelRuntimeFactV2>,
    pub omitted_relevant_fact_count: usize,
    pub permissions_granted: bool,
    pub raw_model_included: bool,
    pub content_digest: String,
}

impl LifeModelRuntimeContextV2 {
    pub fn build(
        version: &LifeModelVersionV2,
        task_text: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<Self>> {
        version.validate_integrity()?;
        if version.document.is_empty()
            || task_text.trim().is_empty()
            || task_explicitly_disables_lifemodel(task_text)
        {
            return Ok(None);
        }
        parse_not_future(&version.created_at, now)?;
        let normalized_task = normalize(task_text);
        let tokens = task_tokens(&normalized_task);
        let intents = TaskIntents::from_task(&normalized_task);
        let mut candidates = document_candidates(&version.document);

        for candidate in &mut candidates {
            let confirmed_at = parse_not_future(&candidate.confirmed_at, now)?;
            candidate.confirmation_age_days = (now - confirmed_at).num_days().max(0);
            candidate.score_for(&normalized_task, &tokens, intents);
        }
        candidates.retain(|candidate| candidate.relevant);
        candidates.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.section.cmp(&right.section))
                .then_with(|| left.item_id.cmp(&right.item_id))
        });
        candidates
            .dedup_by(|left, right| left.section == right.section && left.item_id == right.item_id);
        let omitted_relevant_fact_count = candidates.len().saturating_sub(MAX_SELECTED_FACTS);
        let facts = candidates
            .into_iter()
            .take(MAX_SELECTED_FACTS)
            .map(|candidate| LifeModelRuntimeFactV2 {
                item_id: candidate.item_id,
                section: candidate.section,
                value: bounded(&candidate.value, MAX_FACT_CHARS),
                source_refs: candidate.source_refs,
                confirmed_at: candidate.confirmed_at,
                confirmation_age_days: candidate.confirmation_age_days,
                selected_reason: bounded(&candidate.reason, MAX_REASON_CHARS),
            })
            .collect::<Vec<_>>();
        if facts.is_empty() {
            return Ok(None);
        }
        let mut selected_sections = facts.iter().map(|fact| fact.section).collect::<Vec<_>>();
        selected_sections.sort();
        selected_sections.dedup();
        let digest = crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
            "modelId": version.model_id,
            "modelVersion": version.model_version,
            "versionDigest": version.version_digest,
            "documentDigest": version.document_digest,
            "facts": facts,
        }));
        Ok(Some(Self {
            schema: "openlife.lifemodel.runtime-context.v2".into(),
            model_id: version.model_id.clone(),
            model_version: version.model_version,
            version_digest: version.version_digest.clone(),
            document_digest: version.document_digest.clone(),
            selected_sections,
            facts,
            omitted_relevant_fact_count,
            permissions_granted: false,
            raw_model_included: false,
            content_digest: format!("bytes:{} hash:{}", digest.0, digest.1),
        }))
    }

    pub fn render_prompt(&self) -> String {
        let has_communication_preference = self.facts.iter().any(|fact| {
            matches!(
                fact.section,
                LifeModelSectionV2::StablePreferences
                    | LifeModelSectionV2::CollaborationPreferences
            )
        });
        let mut prompt = format!(
            "Confirmed LifeModel v2 context (model {}, version {}; permissions=false).\nPriority: policy, current instruction, and verified task facts override this context. Never infer tools, credentials, actions, or durable writes.\nUse each fact only for its named role; never invent task facts from it.",
            self.model_id, self.model_version,
        );
        if has_communication_preference {
            prompt.push_str(
                "\nCommunication preferences constrain response tone, brevity, structure, and wording only.",
            );
        }

        let line_overhead = self
            .facts
            .iter()
            .map(|fact| section_label(fact.section).chars().count() + 5)
            .sum::<usize>();
        let remaining =
            MAX_RENDERED_PROMPT_CHARS.saturating_sub(prompt.chars().count() + line_overhead);
        let value_budget = remaining.checked_div(self.facts.len()).unwrap_or_default();
        for fact in &self.facts {
            prompt.push_str("\n- ");
            prompt.push_str(section_label(fact.section));
            prompt.push_str(": ");
            prompt.push_str(&bounded(&fact.value, value_budget));
        }
        bounded(&prompt, MAX_RENDERED_PROMPT_CHARS)
    }
}

fn parse_not_future(value: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>> {
    let parsed = DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc);
    if parsed > now {
        bail!("lifemodel_v2_runtime_timestamp_in_future");
    }
    Ok(parsed)
}

#[derive(Debug, Clone)]
struct Candidate {
    item_id: String,
    section: LifeModelSectionV2,
    value: String,
    source_refs: Vec<String>,
    confirmed_at: String,
    confirmation_age_days: i64,
    score: usize,
    reason: String,
    relevant: bool,
}

impl Candidate {
    fn score_for(&mut self, task: &str, tokens: &[String], intents: TaskIntents) {
        let value = normalize(&self.value);
        let direct_match = !value.is_empty() && (task.contains(&value) || value.contains(task));
        let token_matches = tokens
            .iter()
            .filter(|token| value.contains(token.as_str()))
            .count();
        let tool_preference_match = intents.tool_selection
            && matches!(
                self.section,
                LifeModelSectionV2::StablePreferences
                    | LifeModelSectionV2::CollaborationPreferences
            )
            && contains_any(&value, &["tool", "mcp", "local", "工具", "本地"]);
        let stable_communication_match = intents.communication
            && self.section == LifeModelSectionV2::StablePreferences
            && contains_any(
                &value,
                &[
                    "brief",
                    "concise",
                    "detailed",
                    "direct",
                    "formal",
                    "casual",
                    "tone",
                    "style",
                    "wording",
                    "communication",
                    "response",
                    "email",
                    "message",
                    "简洁",
                    "直接",
                    "详细",
                    "正式",
                    "语气",
                    "风格",
                    "措辞",
                    "表达",
                    "沟通",
                    "回复",
                    "邮件",
                ],
            );
        let intent_match =
            intents.matches(self.section) || tool_preference_match || stable_communication_match;
        let effective_token_matches = if intents.allows_keyword_match(self.section) {
            token_matches
        } else {
            0
        };
        self.relevant = direct_match || effective_token_matches > 0 || intent_match;
        self.score = effective_token_matches * 20
            + usize::from(direct_match) * 60
            + usize::from(intent_match) * 35;
        self.reason = if direct_match {
            "direct task match".into()
        } else if effective_token_matches > 0 {
            format!("task keyword matches: {effective_token_matches}")
        } else if intent_match {
            format!("task intent matches {}", section_label(self.section))
        } else {
            "not selected".into()
        };
    }
}

#[derive(Debug, Clone, Copy)]
struct TaskIntents {
    planning: bool,
    explicit_lifemodel: bool,
    communication: bool,
    identity: bool,
    boundary_or_decision: bool,
    capability_or_resource: bool,
    tool_selection: bool,
    relationship: bool,
}

impl TaskIntents {
    fn from_task(task: &str) -> Self {
        Self {
            planning: contains_any(
                task,
                &[
                    "plan", "schedule", "calendar", "roadmap", "计划", "安排", "日历", "目标",
                ],
            ),
            explicit_lifemodel: contains_any(
                task,
                &[
                    "life model",
                    "lifemodel",
                    "long-term goal",
                    "long term goal",
                    "长期目标",
                    "个人模型",
                ],
            ),
            communication: contains_any(
                task,
                &[
                    "write a",
                    "write an",
                    "draft",
                    "email",
                    "message",
                    "reply",
                    "tone",
                    "写一封",
                    "写一段",
                    "写邮件",
                    "撰写",
                    "邮件",
                    "回复",
                    "表达",
                ],
            ),
            identity: contains_any(
                task,
                &[
                    "introduce me",
                    "about me",
                    "biography",
                    "personal profile",
                    "self description",
                    "自我介绍",
                    "介绍我",
                    "关于我",
                    "个人简介",
                    "我的身份",
                ],
            ),
            boundary_or_decision: contains_any(
                task,
                &[
                    "decide", "choice", "tradeoff", "boundary", "risk", "决策", "选择", "取舍",
                    "边界", "风险",
                ],
            ),
            capability_or_resource: contains_any(
                task,
                &["ability", "resource", "available", "能力", "资源", "可用"],
            ) || (contains_any(task, &["tool", "工具"])
                && !explicitly_disables_tools(task)),
            tool_selection: contains_any(
                task,
                &[
                    "mcp", "read", "source", "search", "file", "读取", "来源", "搜索", "文件",
                ],
            ) || (contains_any(
                task,
                &[
                    "use tool",
                    "choose tool",
                    "call tool",
                    "使用工具",
                    "选择工具",
                    "调用工具",
                ],
            ) && !explicitly_disables_tools(task)),
            relationship: contains_any(
                task,
                &[
                    "family",
                    "partner",
                    "relationship",
                    "team",
                    "家庭",
                    "伴侣",
                    "关系",
                    "团队",
                ],
            ),
        }
    }

    fn matches(self, section: LifeModelSectionV2) -> bool {
        match section {
            LifeModelSectionV2::LongTermGoals => self.planning && self.explicit_lifemodel,
            LifeModelSectionV2::StablePreferences => false,
            LifeModelSectionV2::CollaborationPreferences => self.communication,
            LifeModelSectionV2::Values
            | LifeModelSectionV2::PersonalBoundaries
            | LifeModelSectionV2::DecisionPrinciples => self.boundary_or_decision,
            LifeModelSectionV2::Capabilities | LifeModelSectionV2::Resources => {
                self.capability_or_resource
            }
            LifeModelSectionV2::ImportantRelationships => self.relationship,
            LifeModelSectionV2::Identity => self.identity,
        }
    }

    fn allows_keyword_match(self, section: LifeModelSectionV2) -> bool {
        match section {
            LifeModelSectionV2::Identity => self.identity,
            LifeModelSectionV2::LongTermGoals => self.planning,
            LifeModelSectionV2::StablePreferences => false,
            LifeModelSectionV2::CollaborationPreferences => self.communication,
            LifeModelSectionV2::Values
            | LifeModelSectionV2::PersonalBoundaries
            | LifeModelSectionV2::DecisionPrinciples => self.boundary_or_decision,
            LifeModelSectionV2::Capabilities | LifeModelSectionV2::Resources => {
                self.capability_or_resource
            }
            LifeModelSectionV2::ImportantRelationships => self.relationship,
        }
    }
}

fn document_candidates(document: &LifeModelDocumentV2) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for (section, items) in [
        (LifeModelSectionV2::Identity, &document.identity),
        (LifeModelSectionV2::Values, &document.values),
        (
            LifeModelSectionV2::StablePreferences,
            &document.stable_preferences,
        ),
        (
            LifeModelSectionV2::PersonalBoundaries,
            &document.personal_boundaries,
        ),
        (
            LifeModelSectionV2::DecisionPrinciples,
            &document.decision_principles,
        ),
        (
            LifeModelSectionV2::CollaborationPreferences,
            &document.collaboration_preferences,
        ),
    ] {
        candidates.extend(items.iter().map(|item| {
            candidate(
                section,
                &item.id,
                &item.statement,
                &item.source_refs,
                &item.confirmed_at,
            )
        }));
    }
    candidates.extend(document.long_term_goals.iter().map(|item| {
        candidate(
            LifeModelSectionV2::LongTermGoals,
            &item.id,
            &format!("{}: {}", item.direction, item.meaning),
            &item.source_refs,
            &item.confirmed_at,
        )
    }));
    candidates.extend(document.important_relationships.iter().map(|item| {
        candidate(
            LifeModelSectionV2::ImportantRelationships,
            &item.id,
            &format!(
                "{}: {}; {}",
                item.person_label, item.relationship, item.significance
            ),
            &item.source_refs,
            &item.confirmed_at,
        )
    }));
    candidates.extend(document.capabilities.iter().map(|item| {
        candidate(
            LifeModelSectionV2::Capabilities,
            &item.id,
            &format!("{}: {}", item.name, item.description),
            &item.source_refs,
            &item.confirmed_at,
        )
    }));
    candidates.extend(document.resources.iter().map(|item| {
        candidate(
            LifeModelSectionV2::Resources,
            &item.id,
            &format!("{}: {}", item.name, item.description),
            &item.source_refs,
            &item.confirmed_at,
        )
    }));
    candidates
}

fn candidate(
    section: LifeModelSectionV2,
    item_id: &str,
    value: &str,
    source_refs: &[String],
    confirmed_at: &str,
) -> Candidate {
    Candidate {
        item_id: item_id.into(),
        section,
        value: value.into(),
        source_refs: source_refs.to_vec(),
        confirmed_at: confirmed_at.into(),
        confirmation_age_days: 0,
        score: 0,
        reason: String::new(),
        relevant: false,
    }
}

fn section_label(section: LifeModelSectionV2) -> &'static str {
    match section {
        LifeModelSectionV2::Identity => "identity",
        LifeModelSectionV2::Values => "values",
        LifeModelSectionV2::LongTermGoals => "long_term_goals",
        LifeModelSectionV2::StablePreferences => "stable_preferences",
        LifeModelSectionV2::PersonalBoundaries => "personal_boundaries",
        LifeModelSectionV2::ImportantRelationships => "important_relationships",
        LifeModelSectionV2::Capabilities => "capabilities",
        LifeModelSectionV2::Resources => "resources",
        LifeModelSectionV2::DecisionPrinciples => "decision_principles",
        LifeModelSectionV2::CollaborationPreferences => "collaboration_preferences",
    }
}

fn task_tokens(value: &str) -> Vec<String> {
    let mut tokens = value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| token.chars().count() >= 2)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut cjk_run = Vec::new();
    let flush_cjk = |run: &mut Vec<char>, tokens: &mut Vec<String>| {
        if run.len() >= 2 {
            tokens.extend(
                run.windows(2)
                    .map(|window| window.iter().collect::<String>()),
            );
        }
        run.clear();
    };
    for character in value.chars() {
        if matches!(character, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}')
        {
            cjk_run.push(character);
        } else {
            flush_cjk(&mut cjk_run, &mut tokens);
        }
    }
    flush_cjk(&mut cjk_run, &mut tokens);
    tokens.sort();
    tokens.dedup();
    tokens
}

fn normalize(value: &str) -> String {
    value.trim().to_lowercase()
}
fn bounded(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}
fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn explicitly_disables_tools(task: &str) -> bool {
    contains_any(
        task,
        &[
            "do not call tools",
            "do not use tools",
            "don't call tools",
            "don't use tools",
            "without tools",
            "不要调用工具",
            "不要使用工具",
            "不调用工具",
            "不使用工具",
            "无需工具",
        ],
    )
}

pub fn task_explicitly_disables_lifemodel(task_text: &str) -> bool {
    let task = normalize(task_text);
    contains_any(
        &task,
        &[
            "ignore my life model",
            "ignore lifemodel",
            "do not use my life model",
            "don't use my life model",
            "without my life model",
            "不要使用我的 life model",
            "不要参考我的 life model",
            "忽略我的 life model",
            "忽略 life model",
            "不使用 life model",
            "不参考 life model",
            "不要使用个人模型",
            "忽略个人模型",
            "不要参考长期目标",
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life_model::v2::{
        calculate_version_digest, LifeModelLongTermGoalV2, LifeModelStatementV2,
    };

    fn version() -> LifeModelVersionV2 {
        let mut document = LifeModelDocumentV2::empty("primary");
        document.long_term_goals.push(LifeModelLongTermGoalV2 {
            id: "goal-openlife".into(),
            direction: "完成 OpenLife 产品".into(),
            meaning: "让个人 Agent OS 真正可用".into(),
            source_refs: vec!["proposal:goal-1".into()],
            confirmed_at: "2026-08-01T00:00:00Z".into(),
        });
        document
            .collaboration_preferences
            .push(LifeModelStatementV2 {
                id: "communication-direct".into(),
                statement: "沟通保持简洁直接".into(),
                source_refs: vec!["proposal:preference-1".into()],
                confirmed_at: "2026-08-02T00:00:00Z".into(),
            });
        document.values.push(LifeModelStatementV2 {
            id: "unrelated-outdoors".into(),
            statement: "周末喜欢户外徒步".into(),
            source_refs: vec!["proposal:value-1".into()],
            confirmed_at: "2026-08-03T00:00:00Z".into(),
        });
        let document_digest = document.digest().unwrap();
        let source_refs = vec!["materialization:phase5-test".into()];
        let created_at = "2026-08-04T00:00:00Z".to_string();
        let materialization_id = "materialization-phase5-test".to_string();
        let version_digest = calculate_version_digest(
            "primary",
            1,
            None,
            None,
            &document_digest,
            &materialization_id,
            &source_refs,
            &created_at,
        )
        .unwrap();
        LifeModelVersionV2 {
            model_id: "primary".into(),
            schema_version: crate::life_model::v2::LIFE_MODEL_V2_SCHEMA_VERSION.into(),
            model_version: 1,
            parent_version: None,
            parent_digest: None,
            document_digest,
            version_digest,
            document,
            materialization_id,
            source_refs,
            created_at,
        }
    }

    fn now() -> DateTime<Utc> {
        "2026-08-09T00:00:00Z".parse().unwrap()
    }

    fn reseal(version: &mut LifeModelVersionV2) {
        version.document_digest = version.document.digest().unwrap();
        version.version_digest = calculate_version_digest(
            &version.model_id,
            version.model_version,
            version.parent_version,
            version.parent_digest.as_deref(),
            &version.document_digest,
            &version.materialization_id,
            &version.source_refs,
            &version.created_at,
        )
        .unwrap();
    }

    #[test]
    fn selects_only_relevant_confirmed_v2_items() {
        let packet = LifeModelRuntimeContextV2::build(
            &version(),
            "请为 OpenLife 项目制定计划，并写一封简洁的状态邮件",
            now(),
        )
        .unwrap()
        .expect("relevant packet");
        let prompt = packet.render_prompt();
        assert!(prompt.contains("OpenLife"));
        assert!(prompt.contains("简洁直接"));
        assert!(prompt.contains(
            "Communication preferences constrain response tone, brevity, structure, and wording only"
        ));
        assert!(prompt.contains("never invent task facts from it"));
        assert!(prompt.chars().count() <= MAX_RENDERED_PROMPT_CHARS);
        assert!(!prompt.contains("户外徒步"));
        assert!(!packet.permissions_granted);
        assert!(!packet.raw_model_included);
    }

    #[test]
    fn rejects_tampered_version_binding() {
        let mut version = version();
        version.document.long_term_goals[0].meaning = "tampered".into();
        let error = LifeModelRuntimeContextV2::build(&version, "OpenLife plan", now()).unwrap_err();
        assert!(error.to_string().contains("binding_mismatch"));
    }

    #[test]
    fn rejects_future_confirmation_timestamp() {
        let mut version = version();
        version.document.long_term_goals[0].confirmed_at = "2027-01-01T00:00:00Z".into();
        version.document_digest = version.document.digest().unwrap();
        version.version_digest = calculate_version_digest(
            &version.model_id,
            version.model_version,
            version.parent_version,
            version.parent_digest.as_deref(),
            &version.document_digest,
            &version.materialization_id,
            &version.source_refs,
            &version.created_at,
        )
        .unwrap();
        let error = LifeModelRuntimeContextV2::build(&version, "OpenLife plan", now()).unwrap_err();
        assert!(error.to_string().contains("timestamp_in_future"));
    }

    #[test]
    fn returns_none_when_no_v2_item_is_relevant() {
        assert!(
            LifeModelRuntimeContextV2::build(&version(), "解释 Rust borrow checker", now())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn negative_write_and_tool_constraints_do_not_activate_preferences() {
        let mut version = version();
        version
            .document
            .stable_preferences
            .push(LifeModelStatementV2 {
                id: "prefer-local-tool".into(),
                statement: "等价任务优先使用本地工具".into(),
                source_refs: vec!["proposal:prefer-local-tool".into()],
                confirmed_at: "2026-08-03T00:00:00Z".into(),
            });
        reseal(&mut version);

        assert!(LifeModelRuntimeContextV2::build(
            &version,
            "计算 2 + 2，只回答结果。不要调用工具，不要写入任何长期状态。",
            now(),
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn current_instruction_can_disable_lifemodel_context() {
        for instruction in [
            "Ignore my Life Model and plan this OpenLife task only from this message.",
            "忽略 Life Model，本轮只按当前要求规划 OpenLife。",
        ] {
            assert!(
                LifeModelRuntimeContextV2::build(&version(), instruction, now())
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn communication_task_excludes_unrelated_identity_and_schedule_preferences() {
        let mut version = version();
        version.document.identity.push(LifeModelStatementV2 {
            id: "identity-method".into(),
            statement: "我长期重视清晰、诚实、可验证并持续复盘的工作方式。".into(),
            source_refs: vec!["proposal:identity-method".into()],
            confirmed_at: "2026-08-03T00:00:00Z".into(),
        });
        version
            .document
            .stable_preferences
            .push(LifeModelStatementV2 {
                id: "focus-before-lunch".into(),
                statement: "focused work before lunch".into(),
                source_refs: vec!["proposal:focus-before-lunch".into()],
                confirmed_at: "2026-08-03T00:00:00Z".into(),
            });
        reseal(&mut version);

        let packet = LifeModelRuntimeContextV2::build(
            &version,
            "请写一封项目状态邮件：说明本周完成了阶段 5.4，并请对方在周五前确认。不要调用工具，不要写入任何长期状态。",
            now(),
        )
        .unwrap()
        .expect("communication preference packet");

        assert_eq!(packet.facts.len(), 1, "{:#?}", packet.facts);
        assert_eq!(packet.facts[0].item_id, "communication-direct");
    }

    #[test]
    fn chinese_write_a_paragraph_request_selects_collaboration_preference() {
        let packet = LifeModelRuntimeContextV2::build(
            &version(),
            "请为一次开发阶段复盘写一段四句话的内部说明，内容包含完成情况、主要问题、下一步。不要使用工具，不要执行任何外部或持久写入。",
            now(),
        )
        .unwrap()
        .expect("Chinese writing request should select communication context");

        assert_eq!(
            packet.selected_sections,
            vec![LifeModelSectionV2::CollaborationPreferences]
        );
        assert_eq!(packet.facts.len(), 1);
        assert_eq!(packet.facts[0].item_id, "communication-direct");
        assert!(!packet.permissions_granted);
    }

    #[test]
    fn rendered_prompt_keeps_all_selected_facts_inside_runtime_budget() {
        let mut version = version();
        for (id, marker) in [
            ("communication-alpha", "ALPHA"),
            ("communication-beta", "BETA"),
            ("communication-gamma", "GAMMA"),
        ] {
            version
                .document
                .collaboration_preferences
                .push(LifeModelStatementV2 {
                    id: id.into(),
                    statement: format!("{marker} concise {}", "x".repeat(MAX_FACT_CHARS)),
                    source_refs: vec![format!("proposal:{id}")],
                    confirmed_at: "2026-08-03T00:00:00Z".into(),
                });
        }
        reseal(&mut version);

        let packet = LifeModelRuntimeContextV2::build(
            &version,
            "Please draft a concise project email.",
            now(),
        )
        .unwrap()
        .expect("four communication preferences");
        let prompt = packet.render_prompt();

        assert_eq!(packet.facts.len(), MAX_SELECTED_FACTS);
        assert!(prompt.chars().count() <= MAX_RENDERED_PROMPT_CHARS);
        for marker in ["ALPHA", "BETA", "GAMMA", "沟通保持简洁直接"] {
            assert!(prompt.contains(marker), "missing {marker}: {prompt}");
        }
    }

    #[test]
    fn selects_confirmed_tool_preference_only_for_tool_tasks() {
        let mut version = version();
        version
            .document
            .stable_preferences
            .push(LifeModelStatementV2 {
                id: "prefer-mcp".into(),
                statement: "等价读取任务优先 MCP 工具".into(),
                source_refs: vec!["proposal:tool-preference".into()],
                confirmed_at: "2026-08-03T00:00:00Z".into(),
            });
        version.document_digest = version.document.digest().unwrap();
        version.version_digest = calculate_version_digest(
            &version.model_id,
            version.model_version,
            version.parent_version,
            version.parent_digest.as_deref(),
            &version.document_digest,
            &version.materialization_id,
            &version.source_refs,
            &version.created_at,
        )
        .unwrap();

        let packet = LifeModelRuntimeContextV2::build(
            &version,
            "Read two safe sources for this task.",
            now(),
        )
        .unwrap()
        .expect("tool preference packet");
        assert!(packet.facts.iter().any(|fact| fact.item_id == "prefer-mcp"));
    }
}
