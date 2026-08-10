use crate::llm::ChatMessage;
use ring::digest::{Context as DigestContext, SHA256};

pub const CONVERSATION_CONTEXT_SCHEMA_VERSION: u32 = 1;
pub const MAIN_CHAT_CONVERSATION_CONTEXT_CHAR_BUDGET: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalConversationMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversationContextConfig {
    pub max_chars: usize,
    pub recent_chars: usize,
    pub summary_chars: usize,
    pub excerpt_chars: usize,
}

impl Default for ConversationContextConfig {
    fn default() -> Self {
        Self {
            max_chars: MAIN_CHAT_CONVERSATION_CONTEXT_CHAR_BUDGET,
            recent_chars: 44 * 1024,
            summary_chars: 16 * 1024,
            excerpt_chars: 1_200,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationContextSummary {
    pub schema_version: u32,
    pub source_start_message_id: i64,
    pub source_end_message_id: i64,
    pub source_message_count: usize,
    pub source_digest: String,
    pub summary_digest: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ConversationContextProjection {
    pub provider_messages: Vec<ChatMessage>,
    pub summary: Option<ConversationContextSummary>,
    pub omitted_message_count: usize,
    pub total_chars: usize,
}

pub fn compact_conversation_context(
    messages: &[CanonicalConversationMessage],
    config: ConversationContextConfig,
) -> ConversationContextProjection {
    let raw_chars = messages
        .iter()
        .map(|message| message.content.chars().count())
        .sum::<usize>();
    if raw_chars <= config.max_chars {
        return projection_without_summary(messages, raw_chars);
    }

    let mut recent_start = messages.len();
    let mut recent_chars = 0usize;
    for (index, message) in messages.iter().enumerate().rev() {
        let message_chars = message.content.chars().count();
        let must_keep_current_turn = index + 1 == messages.len();
        if !must_keep_current_turn
            && recent_chars.saturating_add(message_chars) > config.recent_chars
        {
            break;
        }
        recent_chars = recent_chars.saturating_add(message_chars);
        recent_start = index;
    }

    let source = &messages[..recent_start];
    let recent = &messages[recent_start..];
    let available_summary_chars = config
        .max_chars
        .saturating_sub(recent_chars)
        .min(config.summary_chars);
    let built_summary =
        build_extractive_summary(source, config.excerpt_chars, available_summary_chars);

    let mut provider_messages = Vec::with_capacity(
        recent.len()
            + built_summary
                .as_ref()
                .map(|summary| summary.provider_messages.len())
                .unwrap_or_default(),
    );
    if let Some(summary) = built_summary.as_ref() {
        provider_messages.extend(summary.provider_messages.clone());
    }
    provider_messages.extend(recent.iter().map(|message| ChatMessage {
        role: message.role.clone(),
        content: message.content.clone(),
    }));
    let total_chars = provider_messages
        .iter()
        .map(|message| message.content.chars().count())
        .sum();
    ConversationContextProjection {
        provider_messages,
        summary: built_summary.map(|summary| summary.projection),
        omitted_message_count: source.len(),
        total_chars,
    }
}

fn projection_without_summary(
    messages: &[CanonicalConversationMessage],
    total_chars: usize,
) -> ConversationContextProjection {
    ConversationContextProjection {
        provider_messages: messages
            .iter()
            .map(|message| ChatMessage {
                role: message.role.clone(),
                content: message.content.clone(),
            })
            .collect(),
        summary: None,
        omitted_message_count: 0,
        total_chars,
    }
}

#[derive(Debug)]
struct SummaryCandidate<'a> {
    message: &'a CanonicalConversationMessage,
    priority: u16,
    labels: Vec<&'static str>,
}

struct BuiltConversationContextSummary {
    projection: ConversationContextSummary,
    provider_messages: Vec<ChatMessage>,
}

fn build_extractive_summary(
    source: &[CanonicalConversationMessage],
    excerpt_chars: usize,
    max_chars: usize,
) -> Option<BuiltConversationContextSummary> {
    let first = source.first()?;
    let last = source.last()?;
    let digest = canonical_source_digest(source);
    let header = format!(
        "[OpenLife conversation context v1 | canonical messages {}-{} | {}]\n以下均为原始会话的精确摘录，不代表任务已完成：\n",
        first.id, last.id, digest
    );
    if header.chars().count() >= max_chars {
        return None;
    }

    let first_user_id = source
        .iter()
        .find(|message| message.role == "user")
        .map(|message| message.id);
    let last_user_id = source
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| message.id);
    let recent_source_floor = source.len().saturating_sub(3);
    let mut candidates = source
        .iter()
        .enumerate()
        .filter_map(|(index, message)| {
            let mut labels = Vec::new();
            let mut priority = 0u16;
            if Some(message.id) == first_user_id {
                labels.push("目标");
                priority = priority.max(100);
            }
            if Some(message.id) == last_user_id {
                labels.push("最近目标");
                priority = priority.max(95);
            }
            if contains_any(
                &message.content,
                &[
                    "必须",
                    "不要",
                    "不能",
                    "仅允许",
                    "只允许",
                    "约束",
                    "must",
                    "never",
                    "do not",
                    "only",
                ],
            ) {
                labels.push("约束");
                priority = priority.max(120);
            }
            if contains_any(
                &message.content,
                &[
                    "未完成",
                    "待完成",
                    "待处理",
                    "阻塞",
                    "blocked",
                    "pending",
                    "todo",
                    "unfinished",
                ],
            ) {
                labels.push("未决");
                priority = priority.max(115);
            }
            if contains_any(
                &message.content,
                &["review", "审核", "批准", "确认", "permission", "许可"],
            ) {
                labels.push("Review/权限");
                priority = priority.max(110);
            }
            if contains_any(
                &message.content,
                &[
                    "http://",
                    "https://",
                    "file://",
                    "/tmp/",
                    "证据",
                    "evidence",
                    "tool result",
                    "工具结果",
                ],
            ) {
                labels.push("证据");
                priority = priority.max(105);
            }
            if index >= recent_source_floor {
                labels.push("近期上下文");
                priority = priority.max(60);
            }
            if priority == 0 {
                None
            } else {
                labels.sort_unstable();
                labels.dedup();
                Some(SummaryCandidate {
                    message,
                    priority,
                    labels,
                })
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| right.message.id.cmp(&left.message.id))
    });

    let mut selected = Vec::new();
    let mut used_chars = header.chars().count();
    let mut used_provider_chars = 0usize;
    for candidate in candidates {
        let excerpt = truncate_exact_excerpt(&candidate.message.content, excerpt_chars);
        let line = format!(
            "- [message:{} {} {}] {}\n",
            candidate.message.id,
            candidate.message.role,
            candidate.labels.join("/"),
            excerpt
        );
        let line_chars = line.chars().count();
        let provider_line_chars = format!(
            "[Earlier canonical excerpt message:{}; {}; not completion evidence] {}",
            candidate.message.id,
            candidate.labels.join("/"),
            excerpt
        )
        .chars()
        .count()
            + if selected.is_empty() {
                "; source:".chars().count() + digest.chars().count()
            } else {
                0
            };
        if used_chars.saturating_add(line_chars) > max_chars
            || used_provider_chars.saturating_add(provider_line_chars) > max_chars
        {
            continue;
        }
        used_chars += line_chars;
        used_provider_chars += provider_line_chars;
        selected.push((candidate, excerpt, line));
    }
    if selected.is_empty() {
        return None;
    }
    selected.sort_by_key(|(candidate, _, _)| candidate.message.id);
    let content = header
        + &selected
            .iter()
            .map(|(_, _, line)| line.as_str())
            .collect::<String>();
    let summary_digest = summary_content_digest(&content);
    let provider_messages = selected
        .into_iter()
        .enumerate()
        .map(|(index, (candidate, excerpt, _))| {
            let source_ref = if index == 0 {
                format!("; source:{}", digest)
            } else {
                String::new()
            };
            ChatMessage {
                role: candidate.message.role.clone(),
                content: format!(
                    "[Earlier canonical excerpt message:{}{}; {}; not completion evidence] {}",
                    candidate.message.id,
                    source_ref,
                    candidate.labels.join("/"),
                    excerpt
                ),
            }
        })
        .collect();
    Some(BuiltConversationContextSummary {
        projection: ConversationContextSummary {
            schema_version: CONVERSATION_CONTEXT_SCHEMA_VERSION,
            source_start_message_id: first.id,
            source_end_message_id: last.id,
            source_message_count: source.len(),
            source_digest: digest,
            summary_digest,
            content,
        },
        provider_messages,
    })
}

fn contains_any(content: &str, needles: &[&str]) -> bool {
    let normalized = content.to_lowercase();
    needles.iter().any(|needle| normalized.contains(needle))
}

fn truncate_exact_excerpt(content: &str, max_chars: usize) -> String {
    let normalized = content.replace(['\r', '\n'], " ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let head_chars = max_chars.saturating_mul(2) / 3;
    let tail_chars = max_chars.saturating_sub(head_chars).saturating_sub(1);
    let head = normalized.chars().take(head_chars).collect::<String>();
    let tail = normalized
        .chars()
        .rev()
        .take(tail_chars)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{head}…{tail}")
}

pub fn canonical_source_digest(messages: &[CanonicalConversationMessage]) -> String {
    let mut digest = DigestContext::new(&SHA256);
    digest.update(b"openlife:conversation-context-source:v1");
    digest.update(&(messages.len() as u64).to_le_bytes());
    for message in messages {
        update_digest_field(&mut digest, &message.id.to_le_bytes());
        update_digest_field(&mut digest, message.role.as_bytes());
        update_digest_field(&mut digest, message.content.as_bytes());
    }
    let hash = digest.finish();
    format!(
        "sha256:{}",
        hash.as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

pub fn summary_matches_source(
    summary: &ConversationContextSummary,
    source: &[CanonicalConversationMessage],
) -> bool {
    summary.schema_version == CONVERSATION_CONTEXT_SCHEMA_VERSION
        && source.first().map(|message| message.id) == Some(summary.source_start_message_id)
        && source.last().map(|message| message.id) == Some(summary.source_end_message_id)
        && source.len() == summary.source_message_count
        && canonical_source_digest(source) == summary.source_digest
        && summary_content_digest(&summary.content) == summary.summary_digest
}

pub fn summary_content_digest(content: &str) -> String {
    let mut digest = DigestContext::new(&SHA256);
    digest.update(b"openlife:conversation-context-summary:v1");
    update_digest_field(&mut digest, content.as_bytes());
    let hash = digest.finish();
    format!(
        "sha256:{}",
        hash.as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn update_digest_field(digest: &mut DigestContext, value: &[u8]) {
    digest.update(&(value.len() as u64).to_le_bytes());
    digest.update(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: i64, role: &str, content: impl Into<String>) -> CanonicalConversationMessage {
        CanonicalConversationMessage {
            id,
            role: role.into(),
            content: content.into(),
        }
    }

    fn compact_test_config() -> ConversationContextConfig {
        ConversationContextConfig {
            max_chars: 1_400,
            recent_chars: 650,
            summary_chars: 600,
            excerpt_chars: 180,
        }
    }

    #[test]
    fn long_context_is_bounded_and_keeps_the_exact_current_user_turn() {
        let mut messages = (1..=18)
            .map(|id| {
                message(
                    id,
                    if id % 2 == 0 { "assistant" } else { "user" },
                    "x".repeat(180),
                )
            })
            .collect::<Vec<_>>();
        messages.push(message(
            19,
            "user",
            "请继续完成发布前检查，不要执行任何外部写入。",
        ));

        let projection = compact_conversation_context(&messages, compact_test_config());

        assert!(projection.total_chars <= compact_test_config().max_chars);
        assert_eq!(projection.provider_messages.last().unwrap().role, "user");
        assert_eq!(
            projection.provider_messages.last().unwrap().content,
            "请继续完成发布前检查，不要执行任何外部写入。"
        );
        assert!(projection.omitted_message_count > 0);
        assert!(projection.summary.is_some());
    }

    #[test]
    fn summary_preserves_constraints_unfinished_work_and_evidence_as_exact_excerpts() {
        let messages = vec![
            message(1, "user", "目标：完成发布前检查。"),
            message(2, "assistant", "已完成编译。"),
            message(3, "user", "必须保持离线，不要调用 Provider。"),
            message(4, "assistant", "未完成：原生重启验证仍 blocked。"),
            message(
                5,
                "user",
                "证据见 /tmp/openlife-report.txt 和 https://example.com/evidence",
            ),
            message(6, "assistant", "普通说明。".repeat(160)),
            message(7, "user", "现在继续。"),
        ];

        let projection = compact_conversation_context(
            &messages,
            ConversationContextConfig {
                max_chars: 700,
                recent_chars: 80,
                summary_chars: 520,
                excerpt_chars: 180,
            },
        );
        let summary = projection.summary.expect("long context summary");

        assert!(summary.content.contains("目标：完成发布前检查。"));
        assert!(summary
            .content
            .contains("必须保持离线，不要调用 Provider。"));
        assert!(summary.content.contains("未完成：原生重启验证仍 blocked。"));
        assert!(summary.content.contains("/tmp/openlife-report.txt"));
        assert!(summary.content.contains("https://example.com/evidence"));
        assert!(projection
            .provider_messages
            .iter()
            .all(|message| message.role != "system"));
        assert!(projection.provider_messages.iter().any(|message| {
            message.role == "user" && message.content.contains("必须保持离线")
        }));
        assert!(projection.provider_messages.iter().any(|message| {
            message.role == "assistant" && message.content.contains("未完成")
        }));
    }

    #[test]
    fn source_digest_is_stable_but_rejects_mutated_or_incomplete_transcripts() {
        let source = vec![message(1, "user", "one"), message(2, "assistant", "two")];
        let summary = ConversationContextSummary {
            schema_version: CONVERSATION_CONTEXT_SCHEMA_VERSION,
            source_start_message_id: 1,
            source_end_message_id: 2,
            source_message_count: 2,
            source_digest: canonical_source_digest(&source),
            summary_digest: summary_content_digest("projection"),
            content: "projection".into(),
        };

        assert!(summary_matches_source(&summary, &source));
        assert!(!summary_matches_source(
            &summary,
            &[
                message(1, "user", "changed"),
                message(2, "assistant", "two")
            ]
        ));
        assert!(!summary_matches_source(&summary, &source[..1]));
        let mut tampered = summary.clone();
        tampered.content.push_str(" changed");
        assert!(!summary_matches_source(&tampered, &source));
    }

    #[test]
    fn rebuilding_from_the_same_canonical_transcript_is_deterministic() {
        let messages = (1..=20)
            .map(|id| {
                message(
                    id,
                    if id % 2 == 0 { "assistant" } else { "user" },
                    format!("message-{id}-{}", "z".repeat(90)),
                )
            })
            .collect::<Vec<_>>();

        let first = compact_conversation_context(&messages, compact_test_config());
        let rebuilt = compact_conversation_context(&messages, compact_test_config());

        assert_eq!(first.summary, rebuilt.summary);
        assert_eq!(first.omitted_message_count, rebuilt.omitted_message_count);
        assert_eq!(first.total_chars, rebuilt.total_chars);
        assert_eq!(
            first.provider_messages.len(),
            rebuilt.provider_messages.len()
        );
        for (left, right) in first
            .provider_messages
            .iter()
            .zip(&rebuilt.provider_messages)
        {
            assert_eq!(left.role, right.role);
            assert_eq!(left.content, right.content);
        }
        assert!(first.summary.is_some());
    }
}
