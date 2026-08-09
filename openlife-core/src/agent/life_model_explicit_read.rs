use crate::life_model::v2::{LifeModelItemV2, LifeModelSectionV2, LifeModelVersionV2};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

const MAX_SELECTED_FACTS: usize = 6;
const MAX_FACT_CHARS: usize = 320;
const MAX_SOURCE_REFS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelExplicitReadFact {
    pub section: String,
    pub item_id: String,
    pub summary: String,
    pub source_refs: Vec<String>,
    pub confirmed_at: String,
    pub selection_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelExplicitReadAnswer {
    pub schema: String,
    pub model_id: String,
    pub model_version: u64,
    pub document_digest: String,
    pub version_created_at: String,
    pub total_item_count: usize,
    pub facts: Vec<LifeModelExplicitReadFact>,
    pub permissions_granted: bool,
}

pub fn is_explicit_lifemodel_read_intent(user_text: &str) -> bool {
    let normalized = normalize(user_text);
    let names_lifemodel = contains_any(
        &normalized,
        &[
            "life model",
            "lifemodel",
            "个人模型",
            "长期画像",
            "人生模型",
        ],
    );
    let asks_to_read = contains_any(
        &normalized,
        &[
            "what",
            "which",
            "show",
            "tell",
            "read",
            "know",
            "recorded",
            "inside",
            "查看",
            "看看",
            "读取",
            "告诉",
            "知道",
            "了解",
            "记录了",
            "有什么",
            "有哪些",
            "是什么",
            "展示",
            "什么",
        ],
    );
    let asks_to_write = contains_any(
        &normalized,
        &[
            "update my",
            "update the life model",
            "update life model",
            "update lifemodel",
            "change my",
            "change the life model",
            "change life model",
            "change lifemodel",
            "add to",
            "remove from",
            "delete from",
            "save to",
            "write to",
            "remember in",
            "更新我的",
            "更新我",
            "请更新",
            "修改我的",
            "修改我",
            "请修改",
            "写入",
            "加入",
            "添加到",
            "请删除",
            "删除我的",
            "清空",
            "保存到",
            "记住到",
            "更新 life model",
            "更新lifemodel",
            "更新个人模型",
            "修改 life model",
            "修改lifemodel",
            "修改个人模型",
            "写入 life model",
            "写入lifemodel",
            "写入个人模型",
            "加入 life model",
            "加入lifemodel",
            "加入个人模型",
            "添加到 life model",
            "添加到lifemodel",
            "添加到个人模型",
            "从 life model 删除",
            "从lifemodel删除",
            "从个人模型删除",
            "清空 life model",
            "清空lifemodel",
            "清空个人模型",
            "保存到 life model",
            "保存到lifemodel",
            "保存到个人模型",
        ],
    );
    names_lifemodel && asks_to_read && !asks_to_write
}

impl LifeModelExplicitReadAnswer {
    pub fn build(version: &LifeModelVersionV2, user_text: &str) -> Result<Self> {
        if !is_explicit_lifemodel_read_intent(user_text) {
            bail!("lifemodel_v2_explicit_read_intent_required");
        }
        version.human_yaml_projection()?;

        let normalized = normalize(user_text);
        let general_read = contains_any(
            &normalized,
            &[
                "what is in",
                "what's in",
                "show my",
                "entire",
                "all",
                "里面",
                "全部",
                "整体",
                "有哪些",
                "有什么",
            ],
        );
        let query_tokens = query_tokens(&normalized);
        let mut candidates = version
            .document
            .items()
            .into_iter()
            .filter_map(|(section, item)| {
                let (item_id, summary, source_refs, confirmed_at) = item_fields(&item);
                let section_match = section_keywords(section)
                    .iter()
                    .any(|keyword| normalized.contains(keyword));
                let content_match = query_tokens
                    .iter()
                    .any(|token| normalize(&summary).contains(token));
                let score = if section_match {
                    100
                } else if content_match {
                    60
                } else if general_read {
                    10
                } else {
                    0
                };
                (score > 0).then(|| {
                    (
                        score,
                        LifeModelExplicitReadFact {
                            section: section_name(section).into(),
                            item_id,
                            summary: bounded(&summary, MAX_FACT_CHARS),
                            source_refs: source_refs.into_iter().take(MAX_SOURCE_REFS).collect(),
                            confirmed_at,
                            selection_reason: if section_match {
                                format!(
                                    "explicit LifeModel read matched the {} section",
                                    section_name(section)
                                )
                            } else if content_match {
                                "explicit LifeModel read matched the confirmed item content".into()
                            } else {
                                "explicit LifeModel overview requested".into()
                            },
                        },
                    )
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.section.cmp(&right.1.section))
                .then_with(|| left.1.item_id.cmp(&right.1.item_id))
        });

        Ok(Self {
            schema: "openlife.lifemodel-explicit-read.v1".into(),
            model_id: version.model_id.clone(),
            model_version: version.model_version,
            document_digest: version.document_digest.clone(),
            version_created_at: version.created_at.clone(),
            total_item_count: version.document.total_item_count(),
            facts: candidates
                .into_iter()
                .take(MAX_SELECTED_FACTS)
                .map(|(_, fact)| fact)
                .collect(),
            permissions_granted: false,
        })
    }

    pub fn render_for_user(&self, user_text: &str) -> String {
        if contains_cjk(user_text) {
            self.render_chinese()
        } else {
            self.render_english()
        }
    }

    fn render_chinese(&self) -> String {
        let header = format!(
            "我读取的是已确认的 Life Model v2 第 {} 版（版本时间：{}），不是会话记忆或待审核候选。",
            self.model_version, self.version_created_at
        );
        let body = if self.facts.is_empty() {
            format!(
                "当前版本共有 {} 条已确认信息，但没有与这次问题精确匹配的条目。",
                self.total_item_count
            )
        } else {
            self.facts
                .iter()
                .map(|fact| {
                    format!(
                        "- {}：{}\n  来源：{}\n  确认时间：{}\n  使用原因：{}",
                        chinese_section_name(&fact.section),
                        fact.summary,
                        render_sources(&fact.source_refs),
                        fact.confirmed_at,
                        chinese_reason(&fact.selection_reason),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!(
            "{header}\n\n{body}\n\n本次只读取已确认的个人模型；没有写入任何内容，也没有因此获得工具、凭据或外部操作权限。"
        )
    }

    fn render_english(&self) -> String {
        let header = format!(
            "I read confirmed Life Model v2 version {} (created {}). This is not conversation memory or a pending candidate.",
            self.model_version, self.version_created_at
        );
        let body = if self.facts.is_empty() {
            format!(
                "The current version contains {} confirmed items, but none precisely match this question.",
                self.total_item_count
            )
        } else {
            self.facts
                .iter()
                .map(|fact| {
                    format!(
                        "- {}: {}\n  Sources: {}\n  Confirmed: {}\n  Why selected: {}",
                        fact.section,
                        fact.summary,
                        render_sources(&fact.source_refs),
                        fact.confirmed_at,
                        fact.selection_reason,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!(
            "{header}\n\n{body}\n\nThis was a read-only use of confirmed personal context. It did not write anything or grant tool, credential, or external-action permission."
        )
    }
}

fn item_fields(item: &LifeModelItemV2) -> (String, String, Vec<String>, String) {
    match item {
        LifeModelItemV2::Statement(item) => (
            item.id.clone(),
            item.statement.clone(),
            item.source_refs.clone(),
            item.confirmed_at.clone(),
        ),
        LifeModelItemV2::LongTermGoal(item) => (
            item.id.clone(),
            format!("{} — {}", item.direction, item.meaning),
            item.source_refs.clone(),
            item.confirmed_at.clone(),
        ),
        LifeModelItemV2::Relationship(item) => (
            item.id.clone(),
            format!(
                "{} — {}; {}",
                item.person_label, item.relationship, item.significance
            ),
            item.source_refs.clone(),
            item.confirmed_at.clone(),
        ),
        LifeModelItemV2::Capability(item) => (
            item.id.clone(),
            format!("{} — {}", item.name, item.description),
            item.source_refs.clone(),
            item.confirmed_at.clone(),
        ),
        LifeModelItemV2::Resource(item) => (
            item.id.clone(),
            format!("{} — {}", item.name, item.description),
            item.source_refs.clone(),
            item.confirmed_at.clone(),
        ),
    }
}

fn section_keywords(section: LifeModelSectionV2) -> &'static [&'static str] {
    match section {
        LifeModelSectionV2::Identity => &["identity", "role", "身份", "角色"],
        LifeModelSectionV2::Values => &["values", "value", "价值观", "价值"],
        LifeModelSectionV2::LongTermGoals => &["long-term goal", "goal", "长期目标", "方向"],
        LifeModelSectionV2::StablePreferences => {
            &["stable preference", "preference", "稳定偏好", "偏好"]
        }
        LifeModelSectionV2::PersonalBoundaries => {
            &["personal boundary", "boundary", "个人边界", "底线"]
        }
        LifeModelSectionV2::ImportantRelationships => {
            &["relationship", "important person", "重要关系", "关系"]
        }
        LifeModelSectionV2::Capabilities => &["capability", "skill", "能力", "技能"],
        LifeModelSectionV2::Resources => &["resource", "资源"],
        LifeModelSectionV2::DecisionPrinciples => &["decision principle", "principle", "决策原则"],
        LifeModelSectionV2::CollaborationPreferences => &[
            "collaboration",
            "communication",
            "work style",
            "协作",
            "沟通",
            "工作方式",
        ],
    }
}

fn section_name(section: LifeModelSectionV2) -> &'static str {
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

fn chinese_section_name(section: &str) -> &str {
    match section {
        "identity" => "身份",
        "values" => "价值观",
        "long_term_goals" => "长期目标",
        "stable_preferences" => "稳定偏好",
        "personal_boundaries" => "个人边界",
        "important_relationships" => "重要关系",
        "capabilities" => "能力",
        "resources" => "资源",
        "decision_principles" => "决策原则",
        "collaboration_preferences" => "协作偏好",
        _ => "长期信息",
    }
}

fn chinese_reason(reason: &str) -> &str {
    match reason {
        "explicit LifeModel read matched the confirmed item content" => {
            "你明确询问了 Life Model，且问题与这条已确认内容匹配"
        }
        "explicit LifeModel overview requested" => "你明确要求查看 Life Model 概览",
        _ => "你明确询问了 Life Model，且问题与该条所属分类匹配",
    }
}

fn render_sources(source_refs: &[String]) -> String {
    if source_refs.is_empty() {
        "unknown".into()
    } else {
        source_refs.join(", ")
    }
}

fn query_tokens(value: &str) -> Vec<String> {
    let ignored = [
        "life",
        "model",
        "lifemodel",
        "what",
        "which",
        "show",
        "tell",
        "read",
        "know",
        "about",
        "my",
        "me",
        "the",
        "个人模型",
        "长期画像",
        "查看",
        "告诉",
        "记录了",
        "是什么",
        "有哪些",
        "有什么",
    ];
    value
        .split(|character: char| {
            !character.is_alphanumeric() && !('\u{4e00}'..='\u{9fff}').contains(&character)
        })
        .map(str::trim)
        .filter(|token| token.chars().count() >= 2 && !ignored.contains(token))
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize(value: &str) -> String {
    value
        .replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn bounded(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn contains_any(value: &str, values: &[&str]) -> bool {
    values.iter().any(|candidate| value.contains(candidate))
}

fn contains_cjk(value: &str) -> bool {
    value
        .chars()
        .any(|character| ('\u{4e00}'..='\u{9fff}').contains(&character))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life_model::v2::{
        LifeModelCommitV2, LifeModelDocumentV2, LifeModelStatementV2, LifeModelV2Store,
    };

    fn version() -> LifeModelVersionV2 {
        let mut document = LifeModelDocumentV2::empty("primary");
        document
            .collaboration_preferences
            .push(LifeModelStatementV2 {
                id: "learning:communication".into(),
                statement: "Prefer concise updates with the conclusion first.".into(),
                source_refs: vec![
                    "proposal:review-1".into(),
                    "lifemodel-learning-candidate:candidate-1".into(),
                    "lifemodel-learning-observation:observation-1".into(),
                ],
                confirmed_at: "2026-08-09T08:00:00Z".into(),
            });
        let store = LifeModelV2Store::new_in_memory().unwrap();
        let first = store
            .commit(LifeModelCommitV2 {
                document: LifeModelDocumentV2::empty("primary"),
                expected_parent_version: None,
                expected_parent_digest: None,
                materialization_id: "proposal:review-base".into(),
                source_refs: vec!["proposal:review-base".into()],
                created_at: "2026-08-09T07:59:59Z".into(),
            })
            .unwrap()
            .version;
        store
            .commit(LifeModelCommitV2 {
                document,
                expected_parent_version: Some(first.model_version),
                expected_parent_digest: Some(first.document_digest),
                materialization_id: "proposal:review-1".into(),
                source_refs: vec!["proposal:review-1".into()],
                created_at: "2026-08-09T08:00:01Z".into(),
            })
            .unwrap()
            .version
    }

    #[test]
    fn explicit_read_selects_confirmed_v2_fact_with_version_source_and_reason() {
        let answer =
            LifeModelExplicitReadAnswer::build(&version(), "我的 Life Model 记录了什么沟通偏好？")
                .unwrap();
        assert_eq!(answer.model_version, 2);
        assert_eq!(answer.facts.len(), 1);
        assert!(answer.facts[0]
            .source_refs
            .contains(&"proposal:review-1".to_string()));
        let rendered = answer.render_for_user("我的 Life Model 记录了什么沟通偏好？");
        assert!(rendered.contains("第 2 版"));
        assert!(rendered.contains("Prefer concise updates"));
        assert!(rendered.contains("proposal:review-1"));
        assert!(rendered.contains("使用原因"));
        assert!(rendered.contains("没有写入任何内容"));
    }

    #[test]
    fn write_request_is_not_an_explicit_read() {
        assert!(!is_explicit_lifemodel_read_intent(
            "Update my LifeModel: communication style is concise."
        ));
        assert!(!is_explicit_lifemodel_read_intent(
            "Show my Life Model and update my communication style."
        ));
        assert!(!is_explicit_lifemodel_read_intent(
            "看看我的个人模型，然后更新我的沟通偏好。"
        ));
        assert!(is_explicit_lifemodel_read_intent(
            "What is recorded in my Life Model?"
        ));
        assert!(is_explicit_lifemodel_read_intent(
            "What changed in my Life Model?"
        ));
        assert!(is_explicit_lifemodel_read_intent(
            "我的 Life Model 更新了什么？"
        ));
    }

    #[test]
    fn tampered_version_cannot_be_rendered_as_confirmed() {
        let mut version = version();
        version.document.collaboration_preferences[0]
            .statement
            .push_str(" tampered");
        assert!(LifeModelExplicitReadAnswer::build(
            &version,
            "What communication preference is in my Life Model?"
        )
        .is_err());
    }
}
