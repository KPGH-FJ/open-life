use crate::agent::types::{CompactedObservation, CompactionSummary, PrivacyPolicy};
use crate::llm::ChatMessage;

#[derive(Debug, Clone)]
pub struct CompactionConfig {
    pub enabled: bool,
    pub token_threshold: usize,
    pub message_count_threshold: usize,
    pub min_messages_before_compaction: usize,
    pub target_summary_tokens: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            token_threshold: 4000,
            message_count_threshold: 20,
            min_messages_before_compaction: 8,
            target_summary_tokens: 500,
        }
    }
}

impl CompactionConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompactionDecision {
    pub should_compact: bool,
    pub reason: Option<String>,
    pub original_token_estimate: usize,
    pub message_count: usize,
}

pub fn estimate_message_tokens(messages: &[ChatMessage]) -> usize {
    let mut total = 0usize;
    for msg in messages {
        let text = &msg.content;
        let chars = text.chars().count();
        let bytes = text.len();
        total += std::cmp::max(chars, bytes / 3);
    }
    total += messages.len() * 3;
    total
}

pub fn should_compact(messages: &[ChatMessage], config: &CompactionConfig) -> CompactionDecision {
    let message_count = messages.len();
    let original_token_estimate = estimate_message_tokens(messages);

    if !config.enabled {
        return CompactionDecision {
            should_compact: false,
            reason: Some("compaction disabled".into()),
            original_token_estimate,
            message_count,
        };
    }

    if message_count == 0 {
        return CompactionDecision {
            should_compact: false,
            reason: Some("no messages".into()),
            original_token_estimate,
            message_count,
        };
    }

    if message_count < config.min_messages_before_compaction {
        return CompactionDecision {
            should_compact: false,
            reason: Some(format!(
                "below min_messages_before_compaction ({})",
                config.min_messages_before_compaction
            )),
            original_token_estimate,
            message_count,
        };
    }

    if message_count >= config.message_count_threshold {
        return CompactionDecision {
            should_compact: true,
            reason: Some(format!(
                "message count {} >= threshold {}",
                message_count, config.message_count_threshold
            )),
            original_token_estimate,
            message_count,
        };
    }

    if original_token_estimate >= config.token_threshold {
        return CompactionDecision {
            should_compact: true,
            reason: Some(format!(
                "token estimate {} >= threshold {}",
                original_token_estimate, config.token_threshold
            )),
            original_token_estimate,
            message_count,
        };
    }

    CompactionDecision {
        should_compact: false,
        reason: Some("below all thresholds".into()),
        original_token_estimate,
        message_count,
    }
}

// ── P8-2: CompactionSummary Builder ───────────────────────────────────

pub struct CompactionInput {
    pub run_id: String,
    pub messages: Vec<ChatMessage>,
    pub active_proposal_ids: Vec<String>,
    pub unresolved_observations: Vec<CompactedObservation>,
    pub preserved_decisions: Vec<String>,
    pub pending_task_summaries: Vec<String>,
    pub privacy_policy: PrivacyPolicy,
    pub original_token_estimate: usize,
    pub target_summary_tokens: usize,
}

pub struct CompactionSummaryBuilder;

impl CompactionSummaryBuilder {
    pub fn build_rule_based(input: CompactionInput) -> CompactionSummary {
        let conversation_summary = build_safe_conversation_summary(&input);
        let compacted_token_estimate = estimate_message_tokens(&[ChatMessage {
            role: "system".into(),
            content: conversation_summary.clone(),
        }]);

        let mut redacted_fields: Vec<String> = Vec::new();
        let mut sensitive_content_redacted = false;

        let messages_text: Vec<&str> = input.messages.iter().map(|m| m.content.as_str()).collect();
        if has_pii_in_batch(&messages_text) {
            sensitive_content_redacted = true;
            redacted_fields.push("pii_detected".into());
        }
        if contains_lifemodel_raw(&input.messages) {
            sensitive_content_redacted = true;
            if !redacted_fields.contains(&"life_model".into()) {
                redacted_fields.push("life_model".into());
            }
        }
        if contains_memory_raw(&input.messages) {
            sensitive_content_redacted = true;
            if !redacted_fields.contains(&"memory".into()) {
                redacted_fields.push("memory".into());
            }
        }
        if contains_sensitive_user_text(&input.messages) {
            sensitive_content_redacted = true;
            if !redacted_fields.contains(&"sensitive_user_text".into()) {
                redacted_fields.push("sensitive_user_text".into());
            }
        }

        let redaction_policy = format!(
            "{} under {}",
            "PII and LifeModel redacted", input.privacy_policy
        );

        let mut summary = CompactionSummary::new(
            &input.run_id,
            conversation_summary,
            input.original_token_estimate,
            compacted_token_estimate,
        );

        for pid in &input.active_proposal_ids {
            summary = summary.with_active_proposal(pid);
        }
        for obs in &input.unresolved_observations {
            summary = summary.with_observation(obs.clone());
        }

        if sensitive_content_redacted {
            summary = summary.with_redaction(redaction_policy, redacted_fields);
        }

        summary.preserved_decisions = input.preserved_decisions;
        summary.pending_task_summaries = input.pending_task_summaries;
        summary.source_message_count = input.messages.len();
        summary.privacy_policy = Some(input.privacy_policy);

        summary
    }
}

fn build_safe_conversation_summary(input: &CompactionInput) -> String {
    let mut parts: Vec<String> = Vec::new();

    parts.push("Prior context was compacted under privacy policy.".into());

    let user_msgs: Vec<&ChatMessage> = input.messages.iter().filter(|m| m.role == "user").collect();
    let assistant_msgs: Vec<&ChatMessage> = input
        .messages
        .iter()
        .filter(|m| m.role == "assistant")
        .collect();

    if !user_msgs.is_empty() {
        parts.push(format!(
            "{} user requests were processed. Topics included decision-making assistance, task planning, and context exploration.",
            redact_count(user_msgs.len())
        ));
    }
    if !assistant_msgs.is_empty() {
        parts.push(format!(
            "{} assistant responses were provided with analysis and recommendations.",
            redact_count(assistant_msgs.len())
        ));
    }

    if !input.active_proposal_ids.is_empty() {
        parts.push(format!(
            "{} active proposals remain pending user confirmation.",
            input.active_proposal_ids.len()
        ));
    }

    if !input.unresolved_observations.is_empty() {
        let tool_names: Vec<String> = input
            .unresolved_observations
            .iter()
            .map(|o| o.tool_name.clone())
            .collect();
        parts.push(format!(
            "{} unresolved tool observations from: {}.",
            input.unresolved_observations.len(),
            tool_names.join(", ")
        ));
    }

    if !input.preserved_decisions.is_empty() {
        parts.push(format!(
            "{} important decisions were made: {}",
            input.preserved_decisions.len(),
            input.preserved_decisions.join("; ")
        ));
    }

    if !input.pending_task_summaries.is_empty() {
        parts.push(format!(
            "{} pending tasks: {}",
            input.pending_task_summaries.len(),
            input.pending_task_summaries.join("; ")
        ));
    }

    parts.join(" ")
}

fn redact_count(n: usize) -> String {
    if n <= 5 {
        format!("{}", n)
    } else if n <= 20 {
        "several".into()
    } else if n <= 50 {
        "many".into()
    } else {
        "a large number of".into()
    }
}

fn has_pii(text: &str) -> bool {
    let email_pattern =
        regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap();
    let us_phone_pattern = regex::Regex::new(r"\b\d{3}[-.\s]?\d{3}[-.\s]?\d{4}\b").unwrap();
    let cn_mobile_pattern =
        regex::Regex::new(r"(?:\+?86[-.\s]?)?1[3-9]\d[-.\s]?\d{4}[-.\s]?\d{4}").unwrap();
    let cn_landline_pattern =
        regex::Regex::new(r"(?:\+?86[-.\s]?)?0\d{2,3}[-.\s]?\d{7,8}").unwrap();

    email_pattern.is_match(text)
        || us_phone_pattern.is_match(text)
        || cn_mobile_pattern.is_match(text)
        || cn_landline_pattern.is_match(text)
}

fn has_pii_in_batch(texts: &[&str]) -> bool {
    texts.iter().any(|t| has_pii(t))
}

fn contains_lifemodel_raw(messages: &[ChatMessage]) -> bool {
    let lifemodel_keys = [
        "life_model",
        "identity",
        "mission_statement",
        "life_philosophy",
        "personality_traits",
        "values",
        "role_definition",
        "voice_style",
    ];
    for msg in messages {
        let lower = msg.content.to_lowercase();
        for key in &lifemodel_keys {
            if lower.contains(key) {
                return true;
            }
        }
    }
    false
}

fn contains_memory_raw(messages: &[ChatMessage]) -> bool {
    let memory_keys = ["memory", "memory_chunk", "vector_chunks", "memory_snippet"];
    for msg in messages {
        let lower = msg.content.to_lowercase();
        for key in &memory_keys {
            if lower.contains(key) {
                return true;
            }
        }
    }
    false
}

fn contains_sensitive_user_text(messages: &[ChatMessage]) -> bool {
    let sensitive_patterns = ["password", "secret", "token", "api_key", "credential"];
    for msg in messages {
        if msg.role == "user" {
            let lower = msg.content.to_lowercase();
            for pat in &sensitive_patterns {
                if lower.contains(pat) {
                    return true;
                }
            }
        }
    }
    false
}

// ── Safe Observation Summarizer ──────────────────────────────────────

/// Allowlisted sources that may have non-sensitive structured output and
/// can include a char-safe content preview in the compacted summary.
fn observation_source_is_safe_read(source: &str) -> bool {
    let lower = source.to_lowercase();
    matches!(
        lower.as_str(),
        "goal.read"
            | "state.read"
            | "tool.list_available"
            | "permission.check"
            | "proposal.list"
            | "agent_run.lookup"
    )
}

/// Sources whose raw output is inherently risky and must never be copied
/// into a compaction summary (file/memory/web/email/mcp/a2a content).
fn observation_source_is_raw_output(source: &str) -> bool {
    let lower = source.to_lowercase();
    lower.starts_with("file.")
        || lower.starts_with("memory.")
        || lower.starts_with("web.")
        || lower.starts_with("email.")
        || lower.starts_with("mcp.")
        || lower.starts_with("a2a.")
        || lower.starts_with("life_model.")
        || matches!(
            lower.as_str(),
            "file.read"
                | "file.write_proposal"
                | "memory.search"
                | "memory.propose_write"
                | "memory.propose_archive"
                | "web.fetch"
                | "web.search"
                | "email.read"
                | "email.propose_draft"
                | "mcp.call_tool"
                | "a2a.call_agent"
                | "life_model.read"
        )
}

/// Case‑insensitive keyword check for secrets, credentials, tokens and
/// other high‑risk terms that must block any content preview.
fn content_has_sensitive_keywords(content: &str) -> bool {
    let lower = content.to_lowercase();
    lower.contains("password")
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("api_key")
        || lower.contains("credential")
        || lower.contains("life_model")
        || lower.contains("identity")
        || lower.contains("mission_statement")
}

/// Char‑safe preview: takes at most `max_chars` **characters** (not bytes).
/// Appends "…" if the content was truncated.  Never panics on multi‑byte
/// UTF‑8 (Chinese, emoji, etc.).
fn safe_char_preview(content: &str, max_chars: usize) -> String {
    let truncated: String = content.chars().take(max_chars).collect();
    if content.chars().count() > max_chars {
        format!("{}…", truncated)
    } else {
        truncated
    }
}

fn sanitize_observation_summary(source: &str, content: &str) -> String {
    if content.is_empty() {
        return format!("Observation from {} was compacted.", source);
    }

    // 1) Never expose raw file/memory/web/email/mcp/a2a output.
    if observation_source_is_raw_output(source) {
        return format!(
            "Observation from {} was compacted; raw tool output omitted.",
            source
        );
    }

    // 2) Unknown sources: default to conservative — no preview.
    if !observation_source_is_safe_read(source) {
        return format!(
            "Observation from {} was compacted; raw tool output omitted.",
            source
        );
    }

    // 3) Safe allowlisted source — still check PII and sensitive keywords.
    if has_pii(content) || content_has_sensitive_keywords(content) {
        return format!(
            "Observation from {} was compacted; sensitive content omitted.",
            source
        );
    }

    // 4) Safe source, clean content — include a char‑safe preview.
    let preview = safe_char_preview(content, 100);
    format!("Observation from {}: {}", source, preview)
}

pub fn build_safe_compacted_observation(
    source: &str,
    content: &str,
) -> crate::agent::types::CompactedObservation {
    let safe_summary = sanitize_observation_summary(source, content);
    crate::agent::types::CompactedObservation {
        tool_name: source.to_string(),
        summary: safe_summary,
        pending_action: "Needs follow-up".into(),
        risk_level: "medium".into(),
    }
}

// ── P8-4: CompactionResult ────────────────────────────────────────────

pub struct CompactionResult {
    pub summary: CompactionSummary,
    pub compacted_messages: Vec<ChatMessage>,
    pub decision: CompactionDecision,
}

pub fn compact_messages_for_agent_loop(
    messages: &[ChatMessage],
    run_id: &str,
    config: &CompactionConfig,
    privacy_policy: PrivacyPolicy,
    active_proposal_ids: Vec<String>,
    unresolved_observations: Vec<CompactedObservation>,
) -> Option<CompactionResult> {
    let decision = should_compact(messages, config);
    if !decision.should_compact {
        return None;
    }

    let input = CompactionInput {
        run_id: run_id.to_string(),
        messages: messages.to_vec(),
        active_proposal_ids,
        unresolved_observations,
        preserved_decisions: Vec::new(),
        pending_task_summaries: Vec::new(),
        privacy_policy,
        original_token_estimate: decision.original_token_estimate,
        target_summary_tokens: config.target_summary_tokens,
    };

    let summary = CompactionSummaryBuilder::build_rule_based(input);
    let compacted_messages = build_compacted_context(messages, &summary);

    Some(CompactionResult {
        summary,
        compacted_messages,
        decision,
    })
}

fn build_compacted_context(
    original: &[ChatMessage],
    summary: &CompactionSummary,
) -> Vec<ChatMessage> {
    let keep_recent = 6usize.min(original.len().saturating_sub(2));

    let last_user_idx = original
        .iter()
        .rposition(|m| m.role == "user")
        .unwrap_or(original.len().saturating_sub(1));

    let mut result: Vec<ChatMessage> = Vec::new();

    result.push(ChatMessage {
        role: "system".into(),
        content: format!(
            "[COMPACTED CONTEXT] Prior conversation was compacted. {}",
            summary.conversation_summary
        ),
    });

    let start_idx = if last_user_idx > keep_recent {
        last_user_idx.saturating_sub(keep_recent)
    } else {
        0
    };

    for msg in original.iter().skip(start_idx).cloned() {
        result.push(msg);
    }

    result
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.into(),
            content: content.into(),
        }
    }

    fn make_messages(count: usize) -> Vec<ChatMessage> {
        let mut msgs = Vec::new();
        for i in 0..count {
            msgs.push(make_msg(
                if i % 2 == 0 { "user" } else { "assistant" },
                &format!("Message number {} with some padding text to increase token count substantially.", i),
            ));
        }
        msgs
    }

    fn make_long_messages(count: usize) -> Vec<ChatMessage> {
        let mut msgs = Vec::new();
        for i in 0..count {
            msgs.push(make_msg(
                if i % 2 == 0 { "user" } else { "assistant" },
                &format!(
                    "Message number {} {}. Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
                    i,
                    "x".repeat(80),
                ),
            ));
        }
        msgs
    }

    // ── P8-1 tests ───────────────────────────────────────────

    #[test]
    fn test_disabled_config_never_compacts() {
        let config = CompactionConfig::disabled();
        let msgs = make_long_messages(30);
        let decision = should_compact(&msgs, &config);
        assert!(!decision.should_compact);
        assert_eq!(decision.reason.unwrap(), "compaction disabled");
    }

    #[test]
    fn test_empty_messages_do_not_compact() {
        let config = CompactionConfig::default();
        let msgs: Vec<ChatMessage> = vec![];
        let decision = should_compact(&msgs, &config);
        assert!(!decision.should_compact);
        assert_eq!(decision.reason.unwrap(), "no messages");
        assert_eq!(decision.message_count, 0);
    }

    #[test]
    fn test_below_thresholds_does_not_compact() {
        let config = CompactionConfig {
            min_messages_before_compaction: 8,
            message_count_threshold: 50,
            token_threshold: 10000,
            ..Default::default()
        };
        let msgs = make_messages(10);
        let decision = should_compact(&msgs, &config);
        assert!(!decision.should_compact);
    }

    #[test]
    fn test_token_threshold_triggers_compaction() {
        let config = CompactionConfig {
            min_messages_before_compaction: 5,
            token_threshold: 300,
            message_count_threshold: 100,
            ..Default::default()
        };
        let msgs = make_long_messages(15);
        let tokens = estimate_message_tokens(&msgs);
        assert!(tokens > 300, "expected tokens {} > 300", tokens);
        let decision = should_compact(&msgs, &config);
        assert!(decision.should_compact);
    }

    #[test]
    fn test_message_count_threshold_triggers_compaction() {
        let config = CompactionConfig {
            min_messages_before_compaction: 5,
            token_threshold: 100000,
            message_count_threshold: 15,
            ..Default::default()
        };
        let msgs = make_messages(20);
        let decision = should_compact(&msgs, &config);
        assert!(decision.should_compact);
    }

    #[test]
    fn test_min_messages_before_compaction_prevents_premature() {
        let config = CompactionConfig {
            min_messages_before_compaction: 20,
            message_count_threshold: 10,
            token_threshold: 100,
            ..Default::default()
        };
        let msgs = make_messages(12);
        let decision = should_compact(&msgs, &config);
        assert!(!decision.should_compact);
        assert!(decision
            .reason
            .unwrap()
            .contains("min_messages_before_compaction"));
    }

    #[test]
    fn test_estimate_message_tokens_is_deterministic() {
        let msgs = make_messages(5);
        let t1 = estimate_message_tokens(&msgs);
        let t2 = estimate_message_tokens(&msgs);
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_estimate_message_tokens_scales_with_content() {
        let short = vec![make_msg("user", "hi")];
        let long = vec![make_msg("user", &"x".repeat(1000))];
        let ts = estimate_message_tokens(&short);
        let tl = estimate_message_tokens(&long);
        assert!(tl > ts, "long {} > short {}", tl, ts);
    }

    // ── P8-2 tests ───────────────────────────────────────────

    #[test]
    fn test_build_summary_preserves_active_proposals() {
        let input = CompactionInput {
            run_id: "run-1".into(),
            messages: make_messages(10),
            active_proposal_ids: vec!["p1".into(), "p2".into()],
            unresolved_observations: vec![],
            preserved_decisions: vec![],
            pending_task_summaries: vec![],
            privacy_policy: PrivacyPolicy::LocalOnly,
            original_token_estimate: 1000,
            target_summary_tokens: 200,
        };
        let summary = CompactionSummaryBuilder::build_rule_based(input);
        assert!(summary.has_active_proposals());
        assert_eq!(summary.active_proposal_ids.len(), 2);
        assert!(summary.active_proposal_ids.contains(&"p1".into()));
        assert!(summary.active_proposal_ids.contains(&"p2".into()));
    }

    #[test]
    fn test_build_summary_preserves_unresolved_observations() {
        let input = CompactionInput {
            run_id: "run-2".into(),
            messages: make_messages(10),
            active_proposal_ids: vec![],
            unresolved_observations: vec![CompactedObservation {
                tool_name: "web.search".into(),
                summary: "Search done".into(),
                pending_action: "Review".into(),
                risk_level: "low".into(),
            }],
            preserved_decisions: vec![],
            pending_task_summaries: vec![],
            privacy_policy: PrivacyPolicy::SummaryOnly,
            original_token_estimate: 1000,
            target_summary_tokens: 200,
        };
        let summary = CompactionSummaryBuilder::build_rule_based(input);
        assert!(summary.has_unresolved_observations());
        assert_eq!(summary.unresolved_observation_count, 1);
    }

    #[test]
    fn test_build_summary_preserves_decisions_and_tasks() {
        let input = CompactionInput {
            run_id: "run-3".into(),
            messages: make_messages(10),
            active_proposal_ids: vec![],
            unresolved_observations: vec![],
            preserved_decisions: vec!["Use Rust for backend".into()],
            pending_task_summaries: vec!["Review API design".into()],
            privacy_policy: PrivacyPolicy::LocalOnly,
            original_token_estimate: 1000,
            target_summary_tokens: 200,
        };
        let summary = CompactionSummaryBuilder::build_rule_based(input);
        assert_eq!(summary.preserved_decisions.len(), 1);
        assert!(summary.preserved_decisions[0].contains("Rust"));
        assert_eq!(summary.pending_task_summaries.len(), 1);
        assert!(summary.pending_task_summaries[0].contains("API"));
    }

    #[test]
    fn test_build_summary_pii_redacted() {
        let msgs = vec![
            make_msg("user", "my email is test@example.com"),
            make_msg("assistant", "ok got it"),
        ];
        let input = CompactionInput {
            run_id: "run-4".into(),
            messages: msgs,
            active_proposal_ids: vec![],
            unresolved_observations: vec![],
            preserved_decisions: vec![],
            pending_task_summaries: vec![],
            privacy_policy: PrivacyPolicy::SummaryOnly,
            original_token_estimate: 500,
            target_summary_tokens: 200,
        };
        let summary = CompactionSummaryBuilder::build_rule_based(input);
        assert!(summary.sensitive_content_redacted);
        assert!(
            summary.redacted_fields.contains(&"pii_detected".into())
                || summary.redacted_fields.contains(&"life_model".into())
                || summary
                    .redacted_fields
                    .contains(&"sensitive_user_text".into())
        );
        assert!(!summary.conversation_summary.contains("test@example.com"));
    }

    #[test]
    fn test_build_summary_excludes_raw_lifemodel_from_cloud_safe() {
        let msgs = vec![
            make_msg("user", "my life_model identity values are X"),
            make_msg("assistant", "understood"),
        ];
        let input = CompactionInput {
            run_id: "run-5".into(),
            messages: msgs,
            active_proposal_ids: vec![],
            unresolved_observations: vec![],
            preserved_decisions: vec![],
            pending_task_summaries: vec![],
            privacy_policy: PrivacyPolicy::SummaryOnly,
            original_token_estimate: 500,
            target_summary_tokens: 200,
        };
        let summary = CompactionSummaryBuilder::build_rule_based(input);
        assert!(summary.sensitive_content_redacted);
        assert!(!summary.conversation_summary.contains("life_model"));
    }

    #[test]
    fn test_build_summary_summary_only_excludes_sensitive() {
        let msgs = vec![
            make_msg("user", "my password is secret123"),
            make_msg("assistant", "ok"),
        ];
        let input = CompactionInput {
            run_id: "run-6".into(),
            messages: msgs,
            active_proposal_ids: vec![],
            unresolved_observations: vec![],
            preserved_decisions: vec![],
            pending_task_summaries: vec![],
            privacy_policy: PrivacyPolicy::SummaryOnly,
            original_token_estimate: 500,
            target_summary_tokens: 200,
        };
        let summary = CompactionSummaryBuilder::build_rule_based(input);
        assert!(!summary.conversation_summary.contains("secret123"));
        assert!(summary.sensitive_content_redacted);
    }

    #[test]
    fn test_build_summary_serde_round_trip() {
        let input = CompactionInput {
            run_id: "run-7".into(),
            messages: make_messages(10),
            active_proposal_ids: vec!["p1".into()],
            unresolved_observations: vec![CompactedObservation {
                tool_name: "file.read".into(),
                summary: "Read file".into(),
                pending_action: "Review".into(),
                risk_level: "low".into(),
            }],
            preserved_decisions: vec!["D1".into()],
            pending_task_summaries: vec!["T1".into()],
            privacy_policy: PrivacyPolicy::LocalOnly,
            original_token_estimate: 1000,
            target_summary_tokens: 200,
        };
        let summary = CompactionSummaryBuilder::build_rule_based(input);
        let json = serde_json::to_string(&summary).unwrap();
        let deser: CompactionSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.id, summary.id);
        assert_eq!(deser.active_proposal_ids, summary.active_proposal_ids);
        assert_eq!(
            deser.unresolved_observation_count,
            summary.unresolved_observation_count
        );
        assert_eq!(deser.preserved_decisions, summary.preserved_decisions);
        assert_eq!(deser.pending_task_summaries, summary.pending_task_summaries);
        assert_eq!(deser.source_message_count, summary.source_message_count);
        assert_eq!(deser.privacy_policy, summary.privacy_policy);
    }

    #[test]
    fn test_build_summary_source_message_count() {
        let input = CompactionInput {
            run_id: "run-8".into(),
            messages: make_messages(25),
            active_proposal_ids: vec![],
            unresolved_observations: vec![],
            preserved_decisions: vec![],
            pending_task_summaries: vec![],
            privacy_policy: PrivacyPolicy::LocalOnly,
            original_token_estimate: 2000,
            target_summary_tokens: 300,
        };
        let summary = CompactionSummaryBuilder::build_rule_based(input);
        assert_eq!(summary.source_message_count, 25);
    }

    #[test]
    fn test_build_summary_privacy_policy_stored() {
        let input = CompactionInput {
            run_id: "run-9".into(),
            messages: make_messages(10),
            active_proposal_ids: vec![],
            unresolved_observations: vec![],
            preserved_decisions: vec![],
            pending_task_summaries: vec![],
            privacy_policy: PrivacyPolicy::SummaryOnly,
            original_token_estimate: 1000,
            target_summary_tokens: 200,
        };
        let summary = CompactionSummaryBuilder::build_rule_based(input);
        assert_eq!(summary.privacy_policy, Some(PrivacyPolicy::SummaryOnly));
    }

    // ── P8-4 tests ───────────────────────────────────────────

    #[test]
    fn test_compact_messages_reduces_count() {
        let config = CompactionConfig {
            min_messages_before_compaction: 5,
            message_count_threshold: 15,
            token_threshold: 100000,
            ..Default::default()
        };
        let msgs = make_messages(20);
        let original_count = msgs.len();
        let result = compact_messages_for_agent_loop(
            &msgs,
            "run-test",
            &config,
            PrivacyPolicy::LocalOnly,
            vec![],
            vec![],
        );
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.compacted_messages.len() < original_count);
        assert_eq!(r.decision.should_compact, true);
    }

    #[test]
    fn test_compact_messages_preserves_latest_user() {
        let config = CompactionConfig {
            min_messages_before_compaction: 5,
            message_count_threshold: 12,
            token_threshold: 100000,
            ..Default::default()
        };
        let mut msgs = Vec::new();
        for i in 0..15 {
            msgs.push(make_msg(
                if i % 2 == 0 { "user" } else { "assistant" },
                &format!("msg {}", i),
            ));
        }
        let result = compact_messages_for_agent_loop(
            &msgs,
            "run-test",
            &config,
            PrivacyPolicy::LocalOnly,
            vec![],
            vec![],
        );
        assert!(result.is_some());
        let r = result.unwrap();
        let has_user = r.compacted_messages.iter().any(|m| m.role == "user");
        assert!(has_user, "compacted messages must contain user messages");
        assert!(r.compacted_messages.len() < 15);
    }

    #[test]
    fn test_no_compaction_below_threshold() {
        let config = CompactionConfig {
            min_messages_before_compaction: 5,
            message_count_threshold: 50,
            token_threshold: 100000,
            ..Default::default()
        };
        let msgs = make_messages(10);
        let result = compact_messages_for_agent_loop(
            &msgs,
            "run-test",
            &config,
            PrivacyPolicy::LocalOnly,
            vec![],
            vec![],
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_compact_messages_includes_system_summary() {
        let config = CompactionConfig {
            min_messages_before_compaction: 5,
            message_count_threshold: 15,
            token_threshold: 100000,
            ..Default::default()
        };
        let msgs = make_messages(20);
        let result = compact_messages_for_agent_loop(
            &msgs,
            "run-test",
            &config,
            PrivacyPolicy::LocalOnly,
            vec![],
            vec![],
        );
        assert!(result.is_some());
        let r = result.unwrap();
        let has_system = r
            .compacted_messages
            .iter()
            .any(|m| m.role == "system" && m.content.contains("COMPACTED CONTEXT"));
        assert!(has_system);
    }

    #[test]
    fn test_compact_messages_no_panic_on_empty_config() {
        let config = CompactionConfig::default();
        let msgs: Vec<ChatMessage> = vec![];
        let result = compact_messages_for_agent_loop(
            &msgs,
            "run-test",
            &config,
            PrivacyPolicy::LocalOnly,
            vec![],
            vec![],
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_summary_content_excludes_raw_sensitive() {
        let config = CompactionConfig {
            min_messages_before_compaction: 5,
            message_count_threshold: 10,
            token_threshold: 100,
            ..Default::default()
        };
        let msgs = vec![
            make_msg("user", "my email is user@secret.com and password is hidden"),
            make_msg("user", "second message"),
            make_msg("assistant", "first reply"),
            make_msg("assistant", "second reply"),
            make_msg("user", "third message"),
            make_msg("user", "fourth message"),
            make_msg("assistant", "third reply"),
            make_msg("assistant", "fourth reply"),
            make_msg("user", "fifth message"),
            make_msg("assistant", "fifth reply"),
        ];
        let result = compact_messages_for_agent_loop(
            &msgs,
            "run-test",
            &config,
            PrivacyPolicy::SummaryOnly,
            vec![],
            vec![],
        );
        assert!(result.is_some());
        let r = result.unwrap();
        let concat: String = r
            .compacted_messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!concat.contains("user@secret.com"));
        assert!(!concat.contains("hidden"));
    }

    #[test]
    fn test_compaction_disabled_config_no_op() {
        let config = CompactionConfig::disabled();
        let msgs = make_long_messages(30);
        let result = compact_messages_for_agent_loop(
            &msgs,
            "run-test",
            &config,
            PrivacyPolicy::LocalOnly,
            vec![],
            vec![],
        );
        assert!(result.is_none());
    }

    // ── Task 1: Safe observation compaction tests ──────────────────────

    #[test]
    fn test_file_read_raw_output_omitted() {
        let obs = build_safe_compacted_observation("file.read", "result: password=secret123");
        assert!(!obs.summary.contains("secret123"));
        assert!(!obs.summary.contains("password"));
        assert!(obs.summary.contains("raw tool output omitted"));
        assert_eq!(obs.tool_name, "file.read");
    }

    #[test]
    fn test_file_read_chinese_text_not_in_summary() {
        let obs = build_safe_compacted_observation(
            "file.read",
            "季度策略草稿：我们准备在下季度推进产品迭代和用户体验优化，重点关注移动端适配。",
        );
        assert!(!obs.summary.contains("季度策略"));
        assert!(!obs.summary.contains("移动端"));
        assert!(obs.summary.contains("raw tool output omitted"));
    }

    #[test]
    fn test_file_read_chinese_no_panic() {
        let _obs = build_safe_compacted_observation(
            "file.read",
            "季度策略草稿：我们准备在下季度推进产品迭代和用户体验优化……",
        );
        // Should not panic on multi-byte UTF-8.
    }

    #[test]
    fn test_memory_search_raw_output_omitted() {
        let obs = build_safe_compacted_observation(
            "memory.search",
            "用户上周提到想学习 Rust 编程，对系统编程感兴趣。",
        );
        assert!(!obs.summary.contains("Rust"));
        assert!(!obs.summary.contains("系统编程"));
        assert!(obs.summary.contains("raw tool output omitted"));
    }

    #[test]
    fn test_web_fetch_raw_output_omitted() {
        let obs = build_safe_compacted_observation(
            "web.fetch",
            "<html><body>网页正文内容，包含大量信息……</body></html>",
        );
        assert!(!obs.summary.contains("网页正文"));
        assert!(!obs.summary.contains("html"));
        assert!(obs.summary.contains("raw tool output omitted"));
    }

    #[test]
    fn test_email_read_raw_output_omitted() {
        let obs = build_safe_compacted_observation(
            "email.read",
            "From: alice@example.com\nSubject: 会议通知\n\n明天下午3点开会。",
        );
        assert!(!obs.summary.contains("alice@example.com"));
        assert!(!obs.summary.contains("会议通知"));
        assert!(obs.summary.contains("raw tool output omitted"));
    }

    #[test]
    fn test_mcp_call_tool_raw_output_omitted() {
        let obs = build_safe_compacted_observation(
            "mcp.call_tool",
            r#"{"status":"ok","data":{"items":["a","b","c"]}}"#,
        );
        assert!(!obs.summary.contains(r#"{"status"#));
        assert!(obs.summary.contains("raw tool output omitted"));
    }

    #[test]
    fn test_unknown_source_default_omitted() {
        let obs = build_safe_compacted_observation("unknown.tool", "some random output text");
        assert!(!obs.summary.contains("random output"));
        assert!(obs.summary.contains("raw tool output omitted"));
    }

    #[test]
    fn test_safe_allowlist_source_preserves_preview() {
        let obs = build_safe_compacted_observation("goal.read", "3 active goals listed");
        assert!(obs.summary.contains("3 active goals listed"));
        assert!(!obs.summary.contains("raw tool output omitted"));
        assert!(!obs.summary.contains("sensitive content omitted"));
        assert_eq!(obs.tool_name, "goal.read");
    }

    #[test]
    fn test_safe_source_with_password_case_insensitive_omitted() {
        let obs = build_safe_compacted_observation("goal.read", "Password=abc123 for service");
        assert!(!obs.summary.contains("abc123"));
        assert!(!obs.summary.contains("Password"));
        assert!(obs.summary.contains("sensitive content omitted"));
    }

    #[test]
    fn test_safe_source_with_uppercase_secret_omitted() {
        let obs = build_safe_compacted_observation("goal.read", "SECRET=xyz789");
        assert!(!obs.summary.contains("xyz789"));
        assert!(!obs.summary.contains("SECRET"));
        assert!(obs.summary.contains("sensitive content omitted"));
    }

    #[test]
    fn test_safe_source_with_token_omitted() {
        let obs = build_safe_compacted_observation("goal.read", "TOKEN=abc.def.ghi");
        assert!(!obs.summary.contains("abc.def.ghi"));
        assert!(!obs.summary.contains("TOKEN"));
        assert!(obs.summary.contains("sensitive content omitted"));
    }

    #[test]
    fn test_safe_source_with_api_key_omitted() {
        let obs = build_safe_compacted_observation("goal.read", "Api_Key=sk-abc123");
        assert!(!obs.summary.contains("sk-abc123"));
        assert!(!obs.summary.contains("Api_Key"));
        assert!(obs.summary.contains("sensitive content omitted"));
    }

    #[test]
    fn test_safe_source_with_credential_omitted() {
        let obs =
            build_safe_compacted_observation("goal.read", "Credential=mysecret used for login");
        assert!(!obs.summary.contains("mysecret"));
        assert!(!obs.summary.contains("Credential"));
        assert!(obs.summary.contains("sensitive content omitted"));
    }

    #[test]
    fn test_chinese_emoji_preview_no_panic() {
        let long_text = "中文测试内容🎉😀🚀包含超过一百个字符的预览文本，用于验证char-safe截断功能是否正常工作。"
            .repeat(5);
        let obs = build_safe_compacted_observation("goal.read", &long_text);
        assert!(!obs.summary.contains("raw tool output omitted"));
        assert!(!obs.summary.contains("sensitive content omitted"));
        assert!(obs.summary.starts_with("Observation from goal.read:"));
        // Verify it's valid UTF-8
        assert!(String::from_utf8(obs.summary.as_bytes().to_vec()).is_ok());
    }

    #[test]
    fn test_observation_retains_tool_name_and_risk_semantics() {
        // file.read is raw output — summary must omit content but retain metadata
        let obs = build_safe_compacted_observation("file.read", "any content");
        assert_eq!(obs.tool_name, "file.read");
        assert_eq!(obs.risk_level, "medium");
        assert!(!obs.summary.is_empty());
        assert!(obs.pending_action.contains("Needs follow-up"));
    }

    #[test]
    fn test_a2a_call_agent_raw_output_omitted() {
        let obs = build_safe_compacted_observation(
            "a2a.call_agent",
            "agent responded with structured data",
        );
        assert!(!obs.summary.contains("structured data"));
        assert!(obs.summary.contains("raw tool output omitted"));
    }

    #[test]
    fn test_email_propose_draft_raw_output_omitted() {
        let obs =
            build_safe_compacted_observation("email.propose_draft", "draft email content for user");
        assert!(!obs.summary.contains("draft email content"));
        assert!(obs.summary.contains("raw tool output omitted"));
    }

    // ── LifeModel observation must not leak raw content ─────────────────

    #[test]
    fn test_life_model_read_values_not_in_summary() {
        let obs = build_safe_compacted_observation(
            "life_model.read",
            "values: family, creativity, honesty",
        );
        assert!(!obs.summary.contains("family"));
        assert!(!obs.summary.contains("creativity"));
        assert!(!obs.summary.contains("honesty"));
        assert!(obs.summary.contains("raw tool output omitted"));
    }

    #[test]
    fn test_life_model_read_goals_not_in_summary() {
        let obs =
            build_safe_compacted_observation("life_model.read", "goals: change job, learn Rust");
        assert!(!obs.summary.contains("change job"));
        assert!(!obs.summary.contains("learn Rust"));
        assert!(obs.summary.contains("raw tool output omitted"));
    }

    #[test]
    fn test_life_model_prefix_raw_output_omitted() {
        let obs = build_safe_compacted_observation(
            "life_model.custom_read",
            "preferences: quiet morning work, relationship notes: spouse birthday",
        );
        assert!(!obs.summary.contains("quiet morning"));
        assert!(!obs.summary.contains("spouse birthday"));
        assert!(obs.summary.contains("raw tool output omitted"));
    }

    #[test]
    fn test_goal_read_still_preserves_preview() {
        let obs = build_safe_compacted_observation(
            "goal.read",
            "short-term: exercise daily, medium-term: certification",
        );
        assert!(!obs.summary.contains("raw tool output omitted"));
        assert!(!obs.summary.contains("sensitive content omitted"));
        assert!(obs.summary.contains("exercise daily"));
        assert_eq!(obs.tool_name, "goal.read");
    }

    // ── Task 3: CompactionEventPayload canonical tests ─────────────────

    #[test]
    fn test_compaction_event_payload_serde_round_trip() {
        use crate::agent::types::CompactionEventPayload;
        let payload = CompactionEventPayload {
            compaction_id: "c1".into(),
            run_id: "run-1".into(),
            reason: "token threshold".into(),
            original_token_estimate: 5000,
            compacted_token_estimate: 800,
            source_message_count: 30,
            active_proposal_count: 2,
            unresolved_observation_count: 1_u32,
            redacted_fields: vec!["pii_detected".into()],
            privacy_policy: "summary_only".into(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("compactionId"));
        assert!(json.contains("unresolvedObservationCount"));
        assert!(!json.contains("secret123"));
        assert!(!json.contains("raw_prompt"));

        let deser: CompactionEventPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.compaction_id, "c1");
        assert_eq!(deser.unresolved_observation_count, 1_u32);
    }

    #[test]
    fn test_compaction_event_payload_camelcase() {
        use crate::agent::types::CompactionEventPayload;
        let payload = CompactionEventPayload {
            compaction_id: "c1".into(),
            run_id: "run-1".into(),
            reason: "test".into(),
            original_token_estimate: 100,
            compacted_token_estimate: 50,
            source_message_count: 10,
            active_proposal_count: 1,
            unresolved_observation_count: 0,
            redacted_fields: vec![],
            privacy_policy: "local_only".into(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert!(json.get("compactionId").is_some());
        assert!(json.get("runId").is_some());
        assert!(json.get("sourceMessageCount").is_some());
        assert!(json.get("unresolvedObservationCount").is_some());
        assert!(json.get("privacyPolicy").is_some());
    }

    #[test]
    fn test_compaction_event_payload_no_raw_sensitive() {
        use crate::agent::types::CompactionEventPayload;
        let payload = CompactionEventPayload {
            compaction_id: "c1".into(),
            run_id: "run-1".into(),
            reason: "test".into(),
            original_token_estimate: 100,
            compacted_token_estimate: 50,
            source_message_count: 10,
            active_proposal_count: 1,
            unresolved_observation_count: 0,
            redacted_fields: vec!["pii".into()],
            privacy_policy: "summary_only".into(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(!json.contains("password"));
        assert!(!json.contains("secret"));
        assert!(!json.contains("life_model"));
        assert!(!json.contains("raw_memory"));
    }

    // ── Task 4: PII detection tests ────────────────────────────────────

    #[test]
    fn test_has_pii_detects_email() {
        assert!(has_pii("contact user@example.com for details"));
        assert!(has_pii("admin@test.org"));
    }

    #[test]
    fn test_has_pii_detects_us_phone() {
        assert!(has_pii("call 555-123-4567 for info"));
        assert!(has_pii("number 800.555.1234"));
    }

    #[test]
    fn test_has_pii_detects_cn_mobile() {
        assert!(has_pii("phone 13800138000"));
        assert!(has_pii("+86 13912345678"));
        assert!(has_pii("+86-138-0013-8000"));
    }

    #[test]
    fn test_has_pii_no_false_positive_on_numbers() {
        assert!(!has_pii("year 2026"));
        assert!(!has_pii("4000 tokens estimated"));
        assert!(!has_pii("message 12 of 25"));
        assert!(!has_pii("step 3 completed"));
    }

    #[test]
    fn test_has_pii_no_false_positive_on_short_numbers() {
        assert!(!has_pii("123")); // too short
        assert!(!has_pii("count: 42"));
    }
}
