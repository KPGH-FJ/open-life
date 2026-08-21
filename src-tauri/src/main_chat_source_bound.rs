//! Bounded source contracts and output verification for Main Chat.
//!
//! This module is subordinate to the canonical Chat/Work runtimes and the Main
//! Chat kernel. It validates one turn's selected factual basis; it does not own
//! task lifecycle, provider authorization, tool execution, or durable state.

use openlife_core::agent::main_chat_agent_v1::{ContextSourceCandidate, ContextSourceKind};
use serde::Deserialize;

use crate::main_chat_context_loader::MainChatContextRequest;

const MAX_CONTEXT_CONTENT_CHARS: usize = 700;
const MAX_SYSTEM_PROMPT_CHARS: usize = 4_000;
const DIRECT_ANSWER_OUTPUT_CONTRACT_RETRY_PREFIX: &str =
    "[TRUSTED OPENLIFE ONE-SHOT OUTPUT CONTRACT RETRY]";

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn bounded_label(value: &str, max_chars: usize) -> String {
    value
        .replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatSourceBoundFact {
    pub(crate) handle: String,
    pub(crate) content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatSourceBoundContract {
    pub(crate) facts: Vec<MainChatSourceBoundFact>,
}

impl MainChatSourceBoundContract {
    pub(crate) fn from_selected_context(
        request: &MainChatContextRequest,
        selected_source_ids: &[String],
        candidates: &[ContextSourceCandidate],
    ) -> Option<Self> {
        if request.is_inline_fact_bound() {
            return Some(Self {
                facts: request
                    .inline_facts
                    .iter()
                    .map(|fact| MainChatSourceBoundFact {
                        handle: fact.handle.clone(),
                        content: fact.content.clone(),
                    })
                    .collect(),
            });
        }
        let mut facts = Vec::new();
        for source_id in selected_source_ids {
            let Some(candidate) = candidates
                .iter()
                .find(|candidate| candidate.source_id == *source_id)
            else {
                continue;
            };
            let content = if request.is_agent_memory_bound() {
                lifecycle_memory_model_evidence(&candidate.content)
                    .map(|(_, _, content)| content.to_string())
            } else if request.is_document_bound()
                && matches!(
                    candidate.source_kind,
                    ContextSourceKind::MaterializedFile
                        | ContextSourceKind::SelectedPersonalContext
                        | ContextSourceKind::Observation
                )
            {
                Some(candidate.content.clone())
            } else {
                None
            };
            if let Some(content) = content {
                facts.push(MainChatSourceBoundFact {
                    handle: if request.is_agent_memory_bound() {
                        format!("M{}", facts.len() + 1)
                    } else {
                        format!("S{}", facts.len() + 1)
                    },
                    content,
                });
            }
        }
        (!facts.is_empty()).then_some(Self { facts })
    }

    pub(crate) fn prompt_block(&self, user_text: &str) -> String {
        let facts = self
            .facts
            .iter()
            .map(|fact| {
                format!(
                    "{}: {}",
                    fact.handle,
                    serde_json::to_string(&fact.content)
                        .expect("bounded inline fact is JSON serializable")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        if user_text
            .chars()
            .any(|character| matches!(character as u32, 0x3400..=0x9fff))
        {
            format!(
                "你正在执行“限定资料回答”。用户选择的下列资料是本轮唯一允许使用的事实前提；用户输入只表示本轮接受这些前提，并不代表 OpenLife 已独立核实。每个完整句子的全部含义都必须由一条或多条资料支持，不能因为其中一部分匹配就补充说明。不得添加重要性、需求、质量、准确性、稳定性、目的、结果、完成状态、原因、保证、评价、程度、预测或其他资料中没有的事实。复合前提可以安全拆成多句，但每句只能保留该前提明确写出的一个部分。LifeModel 只能影响顺序、语气和详略，不能增加事实。内部标识符和运行时元数据不是证据。\n\n{facts}"
            )
        } else {
            format!(
                "You are answering a source-bound request. The current user selected the following sources as the only factual premises allowed for this turn; user-provided premises are not independently verified world truth. Every complete sentence must be fully entailed by one or more listed facts. Matching one clause does not license extra description. Do not add importance, need, quality, accuracy, stability, purpose, result, completion, causation, guarantee, evaluation, degree, prediction, or any other unstated fact. A compound premise may be split into separate sentences only when each sentence preserves one explicit part. LifeModel may affect order, tone, and brevity only; it cannot add facts. Internal identifiers and runtime metadata are not evidence.\n\n{facts}"
            )
        }
    }

    pub(crate) fn allowed_handles(&self) -> Vec<String> {
        self.facts.iter().map(|fact| fact.handle.clone()).collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MainChatEvidenceCheck {
    verdict: String,
    claims: Vec<MainChatEvidenceClaimCheck>,
    unsupported_draft_ids: Vec<String>,
    missing_fact_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MainChatEvidenceClaimCheck {
    draft_id: String,
    fact_ids: Vec<String>,
    supported: bool,
}

pub(crate) fn model_visible_factual_context(candidate: &ContextSourceCandidate) -> Option<String> {
    match candidate.source_kind {
        ContextSourceKind::SelectedPersonalContext
            if candidate.source_id.starts_with("memory:") =>
        {
            let (scope, freshness, content) = lifecycle_memory_model_evidence(&candidate.content)?;
            Some(format!(
                "Agent Memory; scope={}; freshness={}; content={}",
                bounded_label(scope, 32),
                bounded_label(freshness, 32),
                bounded_text(content, MAX_CONTEXT_CONTENT_CHARS),
            ))
        }
        ContextSourceKind::SelectedPersonalContext
        | ContextSourceKind::MaterializedFile
        | ContextSourceKind::Observation
        | ContextSourceKind::LifeModelContext
        | ContextSourceKind::HsSummary => Some(candidate.content.clone()),
        ContextSourceKind::StableCore
        | ContextSourceKind::RuntimePolicy
        | ContextSourceKind::PolicyDisposition
        | ContextSourceKind::SessionState
        | ContextSourceKind::ToolManifest
        | ContextSourceKind::WorkspaceInstruction
        | ContextSourceKind::SkillMetadata
        | ContextSourceKind::SkillInstruction
        | ContextSourceKind::LifeModelYaml
        | ContextSourceKind::RawMemorySnippet => None,
    }
}

pub(crate) fn lifecycle_memory_model_evidence(content: &str) -> Option<(&str, &str, &str)> {
    let scope = content
        .lines()
        .find_map(|line| line.strip_prefix("scope="))?
        .trim();
    let freshness = content
        .lines()
        .find_map(|line| line.strip_prefix("freshness="))?
        .trim();
    let (_, body) = content.split_once("\ncontent=")?;
    (!scope.is_empty() && !freshness.is_empty() && !body.trim().is_empty()).then_some((
        scope,
        freshness,
        body.trim(),
    ))
}

pub(crate) fn validate_agent_memory_evidence_binding(
    reply: &str,
    allowed_handles: &[String],
    internal_source_ids: &[String],
) -> Result<(), &'static str> {
    if internal_source_ids
        .iter()
        .any(|source_id| !source_id.is_empty() && reply.contains(source_id))
    {
        return Err("context_control_identifier_exposed");
    }

    let mut cited_handles = Vec::new();
    let mut remaining = reply;
    while let Some(start) = remaining.find("[M") {
        let candidate = &remaining[start + 1..];
        let Some(end) = candidate.find(']') else {
            break;
        };
        let handle = &candidate[..end];
        if handle.len() > 1
            && handle.starts_with('M')
            && handle[1..].bytes().all(|byte| byte.is_ascii_digit())
        {
            cited_handles.push(handle.to_string());
        }
        remaining = &candidate[end + 1..];
    }

    if cited_handles.is_empty() {
        return Err("context_evidence_citation_missing");
    }
    if cited_handles
        .iter()
        .any(|handle| !allowed_handles.contains(handle))
    {
        return Err("context_evidence_citation_not_allowed");
    }
    Ok(())
}

fn requested_count_before_suffix_where(
    text: &str,
    suffixes: &[&str],
    mut accepts_match: impl FnMut(&str, usize, usize) -> bool,
) -> Option<usize> {
    let compact = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    const COUNT_LABELS: [(usize, &str); 10] = [
        (1, "一"),
        (2, "二"),
        (3, "三"),
        (4, "四"),
        (5, "五"),
        (6, "六"),
        (7, "七"),
        (8, "八"),
        (9, "九"),
        (10, "十"),
    ];
    COUNT_LABELS.iter().find_map(|(count, chinese)| {
        suffixes
            .iter()
            .any(|suffix| {
                let mut count_labels = vec![count.to_string(), (*chinese).to_string()];
                if *count == 2 {
                    count_labels.push("两".to_string());
                }
                count_labels.iter().any(|label| {
                    let needle = format!("{label}{suffix}");
                    compact.match_indices(&needle).any(|(offset, _)| {
                        let count_boundary_is_valid = match compact[..offset].chars().next_back() {
                            None => true,
                            Some(preceding) => {
                                !preceding.is_ascii_digit()
                                    && !"一二三四五六七八九十".contains(preceding)
                            }
                        };
                        count_boundary_is_valid
                            && accepts_match(&compact, offset, offset + needle.len())
                    })
                })
            })
            .then_some(*count)
    })
}

fn requested_count_before_suffix(text: &str, suffixes: &[&str]) -> Option<usize> {
    requested_count_before_suffix_where(text, suffixes, |_, _, _| true)
}

fn sentence_count_is_scoped_to_each_step(compact: &str, start: usize, end: usize) -> bool {
    let chinese_before = compact[..start]
        .chars()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    let chinese_after = compact[end..].chars().take(10).collect::<String>();
    let chinese_context = format!("{chinese_before}{chinese_after}");
    let chinese_scoped = ["每个步骤", "每一步", "各步骤"]
        .iter()
        .any(|marker| chinese_context.contains(marker));

    let english_before = compact[..start]
        .chars()
        .rev()
        .take(16)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    let english_after = compact[end..].chars().take(16).collect::<String>();
    let english_context = format!("{english_before}{english_after}");
    let english_scoped = ["eachstep", "foreachstep", "perstep"]
        .iter()
        .any(|marker| english_context.contains(marker));

    chinese_scoped || english_scoped
}

pub(crate) fn direct_answer_structure_contract(current_user_text: &str) -> Option<String> {
    let paragraph_count = requested_count_before_suffix(
        current_user_text,
        &["段话", "个段落", "段落", "paragraphs", "paragraph"],
    )?;
    let step_count = requested_count_before_suffix(
        current_user_text,
        &["步执行计划", "步计划", "steps", "stepplan"],
    )?;
    let chinese_output = current_user_text
        .chars()
        .any(|character| matches!(character as u32, 0x3400..=0x9fff));
    let (opening_heading, plan_heading) = if chinese_output {
        ("路演开场", "执行计划")
    } else {
        ("Opening", "Execution Plan")
    };
    Some(format!(
        "The current authenticated user explicitly requested a structured answer. Follow this output contract exactly without changing the requested counts: write the heading '{opening_heading}', then exactly {paragraph_count} distinct prose paragraphs; do not turn them into alternative versions or a numbered list. Then write the heading '{plan_heading}', followed by exactly {step_count} top-level items numbered 1 through {step_count}. Do not add numbered sublists, a preface, or a closing offer. Preserve the user's language. This formatting instruction grants no tool, write, memory, or policy authority."
    ))
}

pub(crate) fn requested_direct_answer_sentence_count(current_user_text: &str) -> Option<usize> {
    requested_count_before_suffix_where(
        current_user_text,
        &["句话", "个句子", "句", "sentences", "sentence"],
        |compact, start, end| !sentence_count_is_scoped_to_each_step(compact, start, end),
    )
}

fn direct_answer_sentence_contract(current_user_text: &str) -> Option<String> {
    let sentence_count = requested_direct_answer_sentence_count(current_user_text)?;
    Some(format!(
        "The current authenticated user explicitly requested exactly {sentence_count} complete sentences. Produce exactly {sentence_count} complete sentences in the user's language. Do not add headings, bullets, numbered labels, a preface, or a closing offer unless the user explicitly requested them. Preserve the requested information order. This formatting instruction grants no tool, write, memory, or policy authority."
    ))
}

pub(crate) fn direct_answer_output_contract_retry_instruction(
    current_user_text: &str,
) -> Option<String> {
    let sentence_count = requested_direct_answer_sentence_count(current_user_text)?;
    Some(format!(
        "{DIRECT_ANSWER_OUTPUT_CONTRACT_RETRY_PREFIX}\nThe previous draft was rejected before display because it did not contain exactly {sentence_count} complete sentences. Produce one replacement with exactly {sentence_count} complete sentences in the user's language. If the supplied facts contain a compound statement, you may split that existing statement into separate sentences to satisfy the count, but you must not introduce a new fact. Preserve the current user's facts, order, and constraints; do not add headings, bullets, labels, facts, tools, writes, or a closing offer. Do not mention the rejected draft or this retry instruction."
    ))
}

fn is_sentence_terminal(character: char) -> bool {
    matches!(character, '。' | '！' | '？' | '.' | '!' | '?')
}

fn is_sentence_closer(character: char) -> bool {
    matches!(
        character,
        '"' | '\'' | '”' | '’' | '»' | '》' | '」' | '』' | ')' | '）' | ']' | '】'
    )
}

fn split_sentence_segments(text: &str, include_trailing_fragment: bool) -> Vec<String> {
    let characters = text.chars().collect::<Vec<_>>();
    let mut sentences = Vec::new();
    let mut index = 0usize;
    let mut sentence_start = 0usize;
    while index < characters.len() {
        let character = characters[index];
        if !is_sentence_terminal(character) {
            index += 1;
            continue;
        }
        if character == '.'
            && index > 0
            && index + 1 < characters.len()
            && characters[index - 1].is_ascii_digit()
            && characters[index + 1].is_ascii_digit()
        {
            index += 1;
            continue;
        }

        let mut boundary_end = index + 1;
        while boundary_end < characters.len()
            && (is_sentence_terminal(characters[boundary_end])
                || is_sentence_closer(characters[boundary_end]))
        {
            boundary_end += 1;
        }
        let is_ascii_boundary = !matches!(character, '。' | '！' | '？')
            && boundary_end < characters.len()
            && !characters[boundary_end].is_whitespace();
        if !is_ascii_boundary {
            let sentence = characters[sentence_start..boundary_end]
                .iter()
                .collect::<String>()
                .trim()
                .to_string();
            if !sentence.is_empty() {
                sentences.push(sentence);
            }
            sentence_start = boundary_end;
            while sentence_start < characters.len() && characters[sentence_start].is_whitespace() {
                sentence_start += 1;
            }
        }
        index = boundary_end;
    }
    if include_trailing_fragment && sentence_start < characters.len() {
        let trailing = characters[sentence_start..]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
        if !trailing.is_empty() {
            sentences.push(trailing);
        }
    }
    sentences
}

fn split_complete_sentences(text: &str) -> Vec<String> {
    split_sentence_segments(text, false)
}

pub(crate) fn split_evidence_check_segments(text: &str) -> Vec<String> {
    split_sentence_segments(text, true)
}

pub(crate) fn complete_sentence_count(text: &str) -> usize {
    split_complete_sentences(text).len()
}

pub(crate) fn parse_source_bound_evidence_check(content: &str) -> Option<MainChatEvidenceCheck> {
    let trimmed = content.trim();
    let json = if trimmed.starts_with("```json") && trimmed.ends_with("```") {
        trimmed.strip_prefix("```json")?.strip_suffix("```")?.trim()
    } else {
        trimmed
    };
    serde_json::from_str(json).ok()
}

pub(crate) fn validate_source_bound_evidence_check(
    contract: &MainChatSourceBoundContract,
    draft: &str,
    check: &MainChatEvidenceCheck,
) -> Result<(), &'static str> {
    let draft_sentences = split_evidence_check_segments(draft);
    let allowed_handles = contract.allowed_handles();
    if check.verdict == "conflict" {
        return Err("source_bound_evidence_conflict");
    }
    if check.verdict != "supported"
        || !check.unsupported_draft_ids.is_empty()
        || !check.missing_fact_ids.is_empty()
        || check.claims.len() != draft_sentences.len()
    {
        return Err("source_bound_claim_unsupported");
    }
    let mut covered_handles = Vec::new();
    let expected_draft_ids = (1..=draft_sentences.len())
        .map(|index| format!("D{index}"))
        .collect::<Vec<_>>();
    let mut seen_draft_ids = Vec::new();
    for claim in &check.claims {
        if !claim.supported
            || !expected_draft_ids.contains(&claim.draft_id)
            || seen_draft_ids.contains(&claim.draft_id)
            || claim.fact_ids.is_empty()
        {
            return Err("source_bound_claim_unsupported");
        }
        seen_draft_ids.push(claim.draft_id.clone());
        for fact_id in &claim.fact_ids {
            if !allowed_handles.contains(fact_id) {
                return Err("source_bound_claim_unsupported");
            }
            if !covered_handles.contains(fact_id) {
                covered_handles.push(fact_id.clone());
            }
        }
    }
    seen_draft_ids.sort_by_key(|draft_id| {
        draft_id
            .strip_prefix('D')
            .and_then(|suffix| suffix.parse::<usize>().ok())
            .unwrap_or(usize::MAX)
    });
    if seen_draft_ids != expected_draft_ids {
        return Err("source_bound_claim_unsupported");
    }
    if allowed_handles
        .iter()
        .any(|handle| !covered_handles.contains(handle))
    {
        return Err("source_bound_claim_unsupported");
    }
    Ok(())
}

pub(crate) fn source_bound_control_identifier_exposed(
    reply: &str,
    contract: &MainChatSourceBoundContract,
    session_id: &str,
    internal_source_ids: &[String],
) -> bool {
    std::iter::once(session_id)
        .chain(internal_source_ids.iter().map(String::as_str))
        .filter(|identifier| !identifier.trim().is_empty())
        .any(|identifier| {
            reply.contains(identifier)
                && !contract
                    .facts
                    .iter()
                    .any(|fact| fact.content.contains(identifier))
        })
        || contract.facts.iter().any(|fact| {
            matches!(fact.handle.chars().next(), Some('F' | 'S'))
                && reply.contains(&fact.handle)
                && !fact.content.contains(&fact.handle)
        })
}

pub(crate) fn deterministic_source_bound_rejection_reply(
    current_user_text: &str,
    code: &str,
) -> String {
    let chinese = current_user_text
        .chars()
        .any(|character| matches!(character as u32, 0x3400..=0x9fff));
    if chinese {
        if code == "source_bound_evidence_conflict" {
            "用户指定的资料之间存在冲突，当前无法安全得出单一结论；请先选择依据或补充资料。".into()
        } else if code.starts_with("context_evidence_citation") {
            "模型回答无法绑定到本轮获准的 Agent Memory 证据，因此未作为可信答案展示。".into()
        } else if code == "source_bound_check_unavailable" {
            "本轮资料边界检查不可用，因此未展示未经核对的模型答案。".into()
        } else {
            "模型回答仍包含无法绑定到用户指定资料的资料外主张，因此未作为可信答案展示。".into()
        }
    } else if code == "source_bound_evidence_conflict" {
        "The selected sources conflict, so OpenLife cannot safely produce one conclusion until you choose a basis or add evidence."
            .into()
    } else if code.starts_with("context_evidence_citation") {
        "The model answer could not be bound to the Agent Memory evidence allowed for this turn, so it was not presented as a trusted answer."
            .into()
    } else if code == "source_bound_check_unavailable" {
        "The source-bound check was unavailable, so the unchecked model answer was not shown."
            .into()
    } else {
        "The model answer still contained claims that could not be bound to the user-selected sources, so it was not shown as a trusted answer."
            .into()
    }
}

pub(crate) fn deterministic_source_bound_render(
    current_user_text: &str,
    context_request: &MainChatContextRequest,
    contract: &MainChatSourceBoundContract,
) -> Option<String> {
    let lower = current_user_text.to_lowercase();
    if let Some(field_label) = requested_verbatim_field_label(current_user_text) {
        let mut values = contract
            .facts
            .iter()
            .flat_map(|fact| fact.content.lines())
            .filter_map(|line| source_bound_field_value(line, &field_label))
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        if values.len() == 1 {
            return values.pop();
        }
    }
    if context_request.is_inline_fact_bound()
        && contains_any(
            &lower,
            &[
                "原样列出",
                "逐字列出",
                "逐条原样",
                "verbatim",
                "list exactly",
            ],
        )
    {
        let reply = contract
            .facts
            .iter()
            .map(|fact| fact.content.trim())
            .filter(|content| !content.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        return (!reply.is_empty()).then_some(reply);
    }

    let requested_sentences = requested_direct_answer_sentence_count(current_user_text)?;
    if requested_sentences <= contract.facts.len()
        || !contract.facts.iter().all(|fact| {
            fact.handle.starts_with('F')
                && fact.handle[1..].bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return None;
    }
    let mut extra_sentences = requested_sentences - contract.facts.len();
    let mut sentence_parts = Vec::new();
    for fact in &contract.facts {
        let content = fact
            .content
            .trim()
            .trim_end_matches(['。', '.', '！', '!', '？', '?'])
            .trim();
        if content.is_empty() {
            return None;
        }
        if extra_sentences > 0 {
            if let Some((left, right)) = split_explicit_compound_fact(content) {
                sentence_parts.push(left);
                sentence_parts.push(right);
                extra_sentences -= 1;
                continue;
            }
        }
        sentence_parts.push(content.to_string());
    }
    if extra_sentences != 0 || sentence_parts.len() != requested_sentences {
        return None;
    }
    let chinese = current_user_text
        .chars()
        .any(|character| matches!(character as u32, 0x3400..=0x9fff));
    let terminator = if chinese { '。' } else { '.' };
    Some(
        sentence_parts
            .into_iter()
            .map(|part| format!("{}{terminator}", part.trim()))
            .collect::<Vec<_>>()
            .join(if chinese { "" } else { " " }),
    )
}

fn requested_verbatim_field_label(current_user_text: &str) -> Option<String> {
    let lower = current_user_text.to_lowercase();
    if !contains_any(
        &lower,
        &["原样列出", "逐字列出", "verbatim", "list exactly"],
    ) {
        return None;
    }
    let (_, suffix) = current_user_text.split_once("其中的")?;
    let label = suffix
        .split(['；', '。', '！', '？', ';', '.', '!', '?', '，', ','])
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches(['`', '*', '_', '“', '”', '"']);
    (!label.is_empty() && label.chars().count() <= 64).then(|| label.to_string())
}

fn source_bound_field_value(line: &str, requested_label: &str) -> Option<String> {
    let line = line.trim().trim_start_matches(['-', '*', '+']).trim();
    let (label, value) = line.split_once('：').or_else(|| line.split_once(':'))?;
    let label = label.trim().trim_matches(['`', '*', '_', '#']).trim();
    if label != requested_label.trim() {
        return None;
    }
    let value = value
        .trim()
        .trim_end_matches(['。', '.', '；', ';'])
        .trim()
        .trim_matches(['`', '*', '_'])
        .trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn split_explicit_compound_fact(content: &str) -> Option<(String, String)> {
    for marker in ["并且", "以及", "并", " and "] {
        let Some(index) = content.find(marker) else {
            continue;
        };
        let left = content[..index].trim();
        let right = content[index..].trim();
        if left.chars().count() >= 2 && right.chars().count() >= marker.chars().count() + 2 {
            return Some((left.to_string(), right.to_string()));
        }
    }
    None
}

pub(crate) fn direct_answer_output_contract_is_satisfied(
    current_user_text: &str,
    reply: &str,
) -> bool {
    let Some(expected_sentence_count) = requested_direct_answer_sentence_count(current_user_text)
    else {
        return true;
    };
    let trimmed = reply.trim();
    if trimmed.is_empty() || complete_sentence_count(trimmed) != expected_sentence_count {
        return false;
    }
    trimmed
        .chars()
        .rev()
        .find(|character| !character.is_whitespace() && !is_sentence_closer(*character))
        .is_some_and(is_sentence_terminal)
}

pub(crate) fn direct_answer_requires_factual_basis(current_user_text: &str) -> bool {
    let lower = current_user_text.to_lowercase();
    let explicitly_requests_basis = contains_any(
        &lower,
        &["依据", "证据", "based on", "according to", "evidence"],
    );
    let requests_status_or_review = contains_any(
        &lower,
        &[
            "开发阶段",
            "阶段复盘",
            "完成情况",
            "主要问题",
            "项目状态",
            "this development stage",
            "current project status",
            "completion status",
        ],
    );
    explicitly_requests_basis && requests_status_or_review
}

pub(crate) fn deterministic_no_factual_evidence_reply(current_user_text: &str) -> String {
    let requested_sentence_count = requested_count_before_suffix(
        current_user_text,
        &["句话", "个句子", "句", "sentences", "sentence"],
    )
    .unwrap_or(2)
    .clamp(1, 10);
    let chinese_output = current_user_text
        .chars()
        .any(|character| matches!(character as u32, 0x3400..=0x9fff));

    if requested_sentence_count == 1 {
        return if chinese_output {
            "结论是：本轮没有选中能够证明开发阶段完成情况、主要问题或下一步的事实依据，因此这些内容目前均无法确认。".into()
        } else {
            "Conclusion: no factual evidence was selected to establish the stage's completion, main problems, or next step, so those points remain unknown.".into()
        };
    }

    let (body, conclusion): (Vec<&str>, &str) = if chinese_output {
        (
            vec![
                "本轮没有选中能够证明该开发阶段完成情况的事实依据。",
                "因此主要问题目前无法从获准上下文中确认。",
                "下一步应先提供或选择相关代码、运行记录或阶段验收证据。",
                "在证据进入本轮上下文前，不应把开发指令或控制信息当成项目事实。",
                "本轮没有调用工具来补充事实。",
                "本轮也没有执行外部或持久写入。",
                "Life Model 没有被用作这次复盘的依据。",
                "内部标识符不能替代可核对的产品证据。",
                "获得证据后才能重新形成有来源的阶段复盘。",
            ],
            "结论是：在获得这些依据之前，不能对该阶段作出真实复盘。",
        )
    } else {
        (
            vec![
                "No factual evidence was selected to establish completion of this development stage.",
                "The main problems therefore cannot be confirmed from the context allowed for this turn.",
                "The next step is to provide or select the relevant code, run records, or acceptance evidence.",
                "Development instructions and control metadata must not be treated as project facts before that evidence is available.",
                "No tool was called to obtain additional facts for this turn.",
                "No external or durable write was performed for this turn.",
                "Life Model was not used as evidence for this review.",
                "Internal identifiers cannot substitute for verifiable product evidence.",
                "A sourced stage review can be produced after the evidence is available.",
            ],
            "Conclusion: a truthful review of this stage cannot be made until that evidence is available.",
        )
    };

    body.into_iter()
        .take(requested_sentence_count - 1)
        .chain(std::iter::once(conclusion))
        .collect::<Vec<_>>()
        .join(if chinese_output { "" } else { " " })
}

pub(crate) fn append_direct_answer_structure_contract(
    system_prompt: String,
    current_user_text: &str,
) -> String {
    let mut instructions = Vec::new();
    if let Some(instruction) = direct_answer_structure_contract(current_user_text) {
        instructions.push(instruction);
    }
    if let Some(instruction) = direct_answer_sentence_contract(current_user_text) {
        instructions.push(instruction);
    }
    if instructions.is_empty() {
        return system_prompt;
    }
    let instruction = instructions.join("\n\n");
    let base_limit = MAX_SYSTEM_PROMPT_CHARS.saturating_sub(instruction.chars().count() + 2);
    format!(
        "{}\n\n{}",
        bounded_text(&system_prompt, base_limit),
        instruction
    )
}
