//! Deterministic, bounded selection of imported-resource context.
//!
//! Imported document text is untrusted data. This module selects stored chunks
//! without model calls, labels every outbound block as untrusted, and issues
//! request-scoped citation identifiers that can be validated after generation.

use crate::llm::{BoundedContextBlock, ContextManifest, ProviderPayloadCategory};
use crate::resource::{ResourceContextChunk, ResourceProvenance, ResourceStore};
use anyhow::{Context, Result};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

pub const MAX_SELECTED_RESOURCE_BLOCKS: usize = 32;
pub const MAX_SELECTED_RESOURCE_CHARS: usize = 262_144;
pub const IMPORTED_RESOURCE_CONTEXT_CATEGORY: &str = "imported_resource_untrusted";
const MAX_RESOURCE_CONTEXT_REF_CHARS: usize = 128;
const RESOURCE_SOURCE_FOOTER_HEADING: &str = "来源（OpenLife 已核验）";
const UNVERIFIED_MODEL_SOURCE_HEADING: &str = "来源（模型文本，未验证）";

/// Validates the metadata-only reference persisted with provider lifecycle
/// evidence. It binds one selected chunk to an issued citation without
/// retaining the filename, document content, or query text.
pub fn is_canonical_resource_context_ref(reference: &str) -> bool {
    if reference.chars().count() > MAX_RESOURCE_CONTEXT_REF_CHARS {
        return false;
    }
    let Some(path_and_citation) = reference.strip_prefix("resource://") else {
        return false;
    };
    let Some((path, citation_id)) = path_and_citation.split_once("?citation=") else {
        return false;
    };
    if path_and_citation.matches("?citation=").count() != 1 {
        return false;
    }
    let Some((resource_id, ordinal)) = path.split_once("/chunk/") else {
        return false;
    };
    if path.matches("/chunk/").count() != 1 {
        return false;
    }
    let resource_id_is_canonical = Uuid::parse_str(resource_id)
        .is_ok_and(|parsed| parsed.get_version_num() == 4 && parsed.to_string() == resource_id);
    let ordinal_is_canonical = ordinal
        .parse::<u32>()
        .is_ok_and(|parsed| parsed.to_string() == ordinal);
    let citation_is_canonical = citation_id.len() == 29
        && citation_id.starts_with("cite_")
        && citation_id[5..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    resource_id_is_canonical && ordinal_is_canonical && citation_is_canonical
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCitation {
    pub citation_id: String,
    pub request_id: String,
    pub resource_id: String,
    pub filename: String,
    pub chunk_ordinal: u32,
    pub content_digest: String,
    pub provenance: ResourceProvenance,
}

/// The only authority accepted when resolving citations emitted by a model.
///
/// The set carries metadata, never document content. It is scoped to one
/// provider request so a citation from an older turn cannot be replayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCitationSet {
    request_id: String,
    entries: BTreeMap<String, ResourceCitation>,
}

impl ResourceCitationSet {
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn issued_ids(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    pub fn citations(&self) -> Vec<ResourceCitation> {
        self.entries.values().cloned().collect()
    }

    pub fn validate_model_citation_ids(
        &self,
        request_id: &str,
        citation_ids: &[String],
    ) -> Result<Vec<ResourceCitation>> {
        if request_id != self.request_id {
            anyhow::bail!("resource_citation_request_mismatch");
        }
        let mut resolved = Vec::with_capacity(citation_ids.len());
        for citation_id in citation_ids {
            let citation = self
                .entries
                .get(citation_id)
                .with_context(|| format!("resource_citation_unknown:{citation_id}"))?;
            resolved.push(citation.clone());
        }
        Ok(resolved)
    }

    pub fn validate_model_output(
        &self,
        request_id: &str,
        model_output: &str,
    ) -> Result<Vec<ResourceCitation>> {
        if request_id != self.request_id {
            anyhow::bail!("resource_citation_request_mismatch");
        }
        if self.entries.is_empty() {
            return Ok(Vec::new());
        }
        let citation_ids = extract_model_citation_ids(model_output)?;
        if citation_ids.is_empty() {
            anyhow::bail!("resource_citation_required");
        }
        self.validate_model_citation_ids(request_id, &citation_ids)
    }

    /// Validate every citation-shaped token and append a canonical source list.
    /// Provider prose cannot invent filenames or provenance because the footer
    /// is rendered exclusively from this request-scoped authority.
    pub fn validate_and_render_model_output(
        &self,
        request_id: &str,
        model_output: &str,
    ) -> Result<String> {
        let resolved = self.validate_model_output(request_id, model_output)?;
        if resolved.is_empty() {
            return Ok(model_output.to_string());
        }
        let mut rendered = model_output.trim_end().replace(
            RESOURCE_SOURCE_FOOTER_HEADING,
            UNVERIFIED_MODEL_SOURCE_HEADING,
        );
        rendered.push_str("\n\n");
        rendered.push_str(RESOURCE_SOURCE_FOOTER_HEADING);
        for citation in resolved {
            rendered.push_str(&format!(
                "\n- `{}` — {} — {}",
                citation.citation_id,
                escape_markdown(&citation.filename),
                provenance_label(&citation.provenance)
            ));
        }
        Ok(rendered)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedResourceContext {
    pub context_blocks: Vec<BoundedContextBlock>,
    pub context_manifest: ContextManifest,
    pub citation_set: ResourceCitationSet,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicResourceSelector;

impl DeterministicResourceSelector {
    pub fn select_for_message(
        &self,
        store: &ResourceStore,
        request_id: &str,
        privacy_decision_id: &str,
        message_id: &str,
        query: &str,
        declared_payload_categories: Vec<ProviderPayloadCategory>,
    ) -> Result<SelectedResourceContext> {
        self.select_for_message_with_budget(
            store,
            request_id,
            privacy_decision_id,
            message_id,
            query,
            declared_payload_categories,
            MAX_SELECTED_RESOURCE_BLOCKS,
            MAX_SELECTED_RESOURCE_CHARS,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub fn select_for_message_with_budget(
        &self,
        store: &ResourceStore,
        request_id: &str,
        privacy_decision_id: &str,
        message_id: &str,
        query: &str,
        mut declared_payload_categories: Vec<ProviderPayloadCategory>,
        max_blocks: usize,
        max_content_chars: usize,
    ) -> Result<SelectedResourceContext> {
        validate_uuid_v4("resource_selection_request_id", request_id)?;
        if privacy_decision_id.trim().is_empty() || privacy_decision_id.len() > 256 {
            anyhow::bail!("resource_selection_privacy_decision_id_invalid");
        }
        if query.trim().is_empty() || query.chars().count() > MAX_SELECTED_RESOURCE_CHARS {
            anyhow::bail!("resource_selection_query_invalid");
        }
        if max_blocks == 0
            || max_blocks > MAX_SELECTED_RESOURCE_BLOCKS
            || max_content_chars == 0
            || max_content_chars > MAX_SELECTED_RESOURCE_CHARS
        {
            anyhow::bail!("resource_selection_budget_invalid");
        }
        declared_payload_categories.sort();
        declared_payload_categories.dedup();
        if declared_payload_categories.is_empty() {
            anyhow::bail!("resource_selection_payload_category_missing");
        }

        let chunks = store.list_context_chunks_for_message(message_id)?;
        let query_terms = expanded_query_terms(query);
        let mut ranked = chunks
            .into_iter()
            .map(|chunk| (relevance_score(&chunk, &query_terms, query), chunk))
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| {
                    left.1
                        .resource
                        .resource_id
                        .cmp(&right.1.resource.resource_id)
                })
                .then_with(|| left.1.chunk.ordinal.cmp(&right.1.chunk.ordinal))
        });
        let has_positive_match = ranked.iter().any(|(score, _)| *score > 0);
        if has_positive_match {
            ranked.retain(|(score, _)| *score > 0);
        }

        // First admit the best matching chunk from each relevant resource.
        // This prevents one long file from crowding all other attachments out
        // of a comparison request. Remaining candidates retain score order.
        let mut covered_resources = BTreeSet::new();
        let mut primary = Vec::new();
        let mut remaining = Vec::new();
        for candidate in ranked {
            if covered_resources.insert(candidate.1.resource.resource_id.clone()) {
                primary.push(candidate);
            } else {
                remaining.push(candidate);
            }
        }
        primary.extend(remaining);

        let mut selected_chars = 0usize;
        let mut context_blocks = Vec::new();
        let mut citations = BTreeMap::new();
        for (_, candidate) in primary {
            if context_blocks.len() >= max_blocks {
                break;
            }
            let citation_id = citation_id(
                request_id,
                &candidate.resource.resource_id,
                candidate.chunk.ordinal,
                &candidate.chunk.content_digest,
            );
            let provenance = provenance_label(&candidate.chunk.provenance);
            let source_ref = format!(
                "resource://{}/chunk/{}?citation={}",
                candidate.resource.resource_id, candidate.chunk.ordinal, citation_id
            );
            let content = format!(
                "[CITATION {citation_id}]\n[UNTRUSTED IMPORTED RESOURCE: {} | {provenance}]\n{}",
                candidate.resource.filename, candidate.chunk.content
            );
            let block_chars = content.chars().count();
            if block_chars > max_content_chars.saturating_sub(selected_chars) {
                continue;
            }
            context_blocks.push(BoundedContextBlock {
                source_ref,
                category: IMPORTED_RESOURCE_CONTEXT_CATEGORY.to_string(),
                content,
            });
            citations.insert(
                citation_id.clone(),
                ResourceCitation {
                    citation_id,
                    request_id: request_id.to_string(),
                    resource_id: candidate.resource.resource_id,
                    filename: candidate.resource.filename,
                    chunk_ordinal: candidate.chunk.ordinal,
                    content_digest: candidate.chunk.content_digest,
                    provenance: candidate.chunk.provenance,
                },
            );
            selected_chars += block_chars;
        }

        let mut selected_context_refs = context_blocks
            .iter()
            .map(|block| block.source_ref.clone())
            .collect::<Vec<_>>();
        selected_context_refs.sort();
        let included_context_categories = if context_blocks.is_empty() {
            Vec::new()
        } else {
            vec![IMPORTED_RESOURCE_CONTEXT_CATEGORY.to_string()]
        };
        let context_manifest = ContextManifest {
            request_id: request_id.to_string(),
            privacy_decision_id: privacy_decision_id.to_string(),
            selected_context_refs,
            included_context_categories,
            declared_payload_categories,
            policy_provenance_refs: Vec::new(),
            raw_life_model_included: false,
            raw_unbounded_memory_included: false,
        };
        context_manifest.validate_context_truth(&context_blocks)?;
        Ok(SelectedResourceContext {
            context_blocks,
            context_manifest,
            citation_set: ResourceCitationSet {
                request_id: request_id.to_string(),
                entries: citations,
            },
        })
    }
}

fn validate_uuid_v4(label: &str, value: &str) -> Result<()> {
    let parsed = Uuid::parse_str(value).with_context(|| format!("{label}_invalid"))?;
    if parsed.get_version_num() != 4 || parsed.to_string() != value.to_ascii_lowercase() {
        anyhow::bail!("{label}_must_be_uuid_v4");
    }
    Ok(())
}

fn citation_id(request_id: &str, resource_id: &str, ordinal: u32, content_digest: &str) -> String {
    let material = format!("{request_id}\0{resource_id}\0{ordinal}\0{content_digest}");
    let digest = digest(&SHA256, material.as_bytes());
    let compact = digest
        .as_ref()
        .iter()
        .take(12)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("cite_{compact}")
}

fn extract_model_citation_ids(model_output: &str) -> Result<Vec<String>> {
    let mut citation_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for (start, _) in model_output.match_indices("cite_") {
        let candidate = model_output[start..].chars().take(29).collect::<String>();
        let valid = candidate.len() == 29
            && candidate.starts_with("cite_")
            && candidate[5..].bytes().all(|byte| byte.is_ascii_hexdigit());
        let trailing = model_output[start + candidate.len()..].chars().next();
        if !valid
            || trailing.is_some_and(|character| character.is_alphanumeric() || character == '_')
        {
            anyhow::bail!("resource_citation_malformed");
        }
        if seen.insert(candidate.clone()) {
            citation_ids.push(candidate);
        }
    }
    Ok(citation_ids)
}

fn escape_markdown(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if matches!(
                character,
                '\\' | '`'
                    | '*'
                    | '_'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '('
                    | ')'
                    | '#'
                    | '+'
                    | '-'
                    | '.'
                    | '!'
                    | '|'
            ) {
                vec!['\\', character]
            } else {
                vec![character]
            }
        })
        .collect()
}

fn relevance_score(
    candidate: &ResourceContextChunk,
    query_terms: &BTreeSet<String>,
    raw_query: &str,
) -> u64 {
    let content_terms = token_counts(&candidate.chunk.content);
    let filename_terms = token_counts(&candidate.resource.filename);
    let document_length = content_terms.values().copied().sum::<u64>().max(1);
    let mut score = 0u64;
    for term in query_terms {
        let frequency = content_terms.get(term).copied().unwrap_or_default();
        if frequency > 0 {
            score = score.saturating_add(
                frequency
                    .saturating_mul(10_000)
                    .saturating_div(document_length.saturating_add(50)),
            );
        }
        if filename_terms.contains_key(term) {
            score = score.saturating_add(4_000);
        }
    }
    let normalized_query = normalize(raw_query);
    let normalized_content = normalize(&candidate.chunk.content);
    if normalized_query.chars().count() >= 4 && normalized_content.contains(&normalized_query) {
        score = score.saturating_add(20_000);
    }
    score
}

fn expanded_query_terms(query: &str) -> BTreeSet<String> {
    let normalized = normalize(query);
    let mut terms = tokenize(&normalized).into_iter().collect::<BTreeSet<_>>();
    let aliases: &[(&str, &[&str])] = &[
        (
            "主张",
            &["claim", "claims", "recommendation", "recommendations"],
        ),
        ("核心", &["core", "claim", "claims"]),
        (
            "分歧",
            &["disagreement", "constraint", "conflict", "tradeoff"],
        ),
        ("风险", &["risk", "risks", "failure"]),
        (
            "趋势",
            &[
                "trend", "increase", "decrease", "上升", "下降", "本周", "上周",
            ],
        ),
        ("异常", &["anomaly", "sentinel", "异常"]),
        (
            "数据质量",
            &["data", "quality", "formula", "missing", "不可信"],
        ),
    ];
    for (trigger, additions) in aliases {
        if normalized.contains(trigger) {
            terms.extend(additions.iter().map(|term| (*term).to_string()));
        }
    }
    terms
}

fn token_counts(value: &str) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for token in tokenize(&normalize(value)) {
        *counts.entry(token).or_insert(0) += 1;
    }
    counts
}

fn normalize(value: &str) -> String {
    value.nfkc().flat_map(char::to_lowercase).collect()
}

fn tokenize(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut ascii = String::new();
    let mut non_ascii_run = Vec::new();
    let flush_ascii = |buffer: &mut String, output: &mut Vec<String>| {
        if !buffer.is_empty() {
            output.push(std::mem::take(buffer));
        }
    };
    let flush_non_ascii = |buffer: &mut Vec<char>, output: &mut Vec<String>| {
        if buffer.is_empty() {
            return;
        }
        for width in 1..=3 {
            if buffer.len() >= width {
                for window in buffer.windows(width) {
                    output.push(window.iter().collect());
                }
            }
        }
        buffer.clear();
    };

    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            flush_non_ascii(&mut non_ascii_run, &mut tokens);
            ascii.push(character);
        } else if !character.is_ascii() && character.is_alphanumeric() {
            flush_ascii(&mut ascii, &mut tokens);
            non_ascii_run.push(character);
        } else {
            flush_ascii(&mut ascii, &mut tokens);
            flush_non_ascii(&mut non_ascii_run, &mut tokens);
        }
    }
    flush_ascii(&mut ascii, &mut tokens);
    flush_non_ascii(&mut non_ascii_run, &mut tokens);
    tokens
}

fn provenance_label(provenance: &ResourceProvenance) -> String {
    match provenance {
        ResourceProvenance::Text {
            start_line,
            end_line,
        } => format!("lines {start_line}-{end_line}"),
        ResourceProvenance::Pdf { page } => format!("page {page}"),
        ResourceProvenance::Docx {
            paragraph_start,
            paragraph_end,
        } => format!("paragraphs {paragraph_start}-{paragraph_end}"),
        ResourceProvenance::Csv { range } => format!("range {range}"),
        ResourceProvenance::Xlsx { sheet, range } => {
            format!("sheet {sheet}, range {range}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{ResourceImportBatch, ResourceImportCandidate};
    use crate::resource_parser::{extract_resource, ResourceExtractionRequest};
    use std::path::Path;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../test-fixtures/resources")
                .join(name),
        )
        .unwrap()
    }

    fn import(store: &ResourceStore, message_id: &str, inputs: &[(&str, &str)]) {
        let resources = inputs
            .iter()
            .map(|(filename, declared_mime)| {
                let bytes = fixture(filename);
                let extraction = extract_resource(ResourceExtractionRequest {
                    filename: (*filename).to_string(),
                    declared_mime: (*declared_mime).to_string(),
                    bytes: bytes.clone(),
                })
                .unwrap();
                ResourceImportCandidate {
                    resource_id: Uuid::new_v4().to_string(),
                    filename: (*filename).to_string(),
                    declared_mime: (*declared_mime).to_string(),
                    detected_mime: extraction.detected_mime,
                    format: extraction.format,
                    bytes,
                    chunks: extraction.chunks,
                }
            })
            .collect();
        store
            .commit_import_batch(ResourceImportBatch {
                operation_id: Uuid::new_v4().to_string(),
                message_id: message_id.to_string(),
                resources,
            })
            .unwrap();
    }

    fn select(store: &ResourceStore, message_id: &str, query: &str) -> SelectedResourceContext {
        DeterministicResourceSelector
            .select_for_message(
                store,
                &Uuid::new_v4().to_string(),
                "privacy-decision-roadshow",
                message_id,
                query,
                vec![ProviderPayloadCategory::CurrentUserConversation],
            )
            .unwrap()
    }

    #[test]
    fn comparison_selection_is_bounded_untrusted_and_citation_scoped() {
        let store = ResourceStore::new_in_memory().unwrap();
        import(
            &store,
            "message-compare",
            &[
                ("comparison.pdf", "application/pdf"),
                (
                    "comparison.docx",
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                ),
            ],
        );
        let selected = select(
            &store,
            "message-compare",
            "请对比 PDF 和 DOCX 中的核心主张、分歧与风险",
        );

        assert!(!selected.context_blocks.is_empty());
        assert!(selected.context_blocks.len() <= MAX_SELECTED_RESOURCE_BLOCKS);
        assert!(selected.context_blocks.iter().all(|block| {
            block.category == IMPORTED_RESOURCE_CONTEXT_CATEGORY
                && block.content.contains("UNTRUSTED IMPORTED RESOURCE")
        }));
        let filenames = selected
            .citation_set
            .citations()
            .into_iter()
            .map(|citation| citation.filename)
            .collect::<BTreeSet<_>>();
        assert!(filenames.contains("comparison.pdf"));
        assert!(filenames.contains("comparison.docx"));
        selected
            .context_manifest
            .validate_context_truth(&selected.context_blocks)
            .unwrap();

        let issued = selected.citation_set.issued_ids();
        assert_eq!(
            selected
                .citation_set
                .validate_model_citation_ids(selected.citation_set.request_id(), &issued)
                .unwrap()
                .len(),
            issued.len()
        );
        assert!(selected
            .citation_set
            .validate_model_citation_ids(
                selected.citation_set.request_id(),
                &["cite_not_issued".to_string()],
            )
            .is_err());
        assert!(selected
            .citation_set
            .validate_model_citation_ids(&Uuid::new_v4().to_string(), &issued)
            .is_err());
        let rendered = selected
            .citation_set
            .validate_and_render_model_output(
                selected.citation_set.request_id(),
                &format!("结论基于附件证据 [{}]。", issued[0]),
            )
            .unwrap();
        assert!(rendered.contains("来源（OpenLife 已核验）"));
        assert!(rendered.contains("comparison"));
        let forged = selected
            .citation_set
            .validate_and_render_model_output(
                selected.citation_set.request_id(),
                &format!(
                    "结论 [{}]。\n\n{RESOURCE_SOURCE_FOOTER_HEADING}\n- `forged` — fake.md — model",
                    issued[0]
                ),
            )
            .unwrap();
        assert_eq!(forged.matches(RESOURCE_SOURCE_FOOTER_HEADING).count(), 1);
        assert!(forged.contains(UNVERIFIED_MODEL_SOURCE_HEADING));
        assert!(selected
            .citation_set
            .validate_and_render_model_output(
                selected.citation_set.request_id(),
                "结论没有任何引用。",
            )
            .is_err());
        assert!(selected
            .citation_set
            .validate_and_render_model_output(
                selected.citation_set.request_id(),
                "伪造引用 cite_000000000000000000000000。",
            )
            .is_err());
        assert!(selected
            .citation_set
            .validate_and_render_model_output(
                selected.citation_set.request_id(),
                "格式错误 cite_short。",
            )
            .is_err());

        let summary = select(&store, "message-compare", "请总结附件");
        let summary_filenames = summary
            .citation_set
            .citations()
            .into_iter()
            .map(|citation| citation.filename)
            .collect::<BTreeSet<_>>();
        assert!(summary_filenames.contains("comparison.pdf"));
        assert!(summary_filenames.contains("comparison.docx"));

        let unrelated_message = select(&store, "message-not-bound", "请总结附件");
        assert!(unrelated_message.context_blocks.is_empty());
        assert!(unrelated_message.citation_set.issued_ids().is_empty());
    }

    #[test]
    fn resource_context_reference_accepts_only_canonical_metadata() {
        let resource_id = Uuid::new_v4().to_string();
        let canonical =
            format!("resource://{resource_id}/chunk/0?citation=cite_0123456789abcdef01234567");
        assert!(is_canonical_resource_context_ref(&canonical));
        for invalid in [
            format!("resource://{resource_id}/chunk/00?citation=cite_0123456789abcdef01234567"),
            format!("resource://{resource_id}/chunk/0?citation=cite_0123456789ABCDEF01234567"),
            format!("resource://{resource_id}/chunk/0?citation=cite_short"),
            format!("resource://{resource_id}/chunk/0?citation=cite_0123456789abcdef01234567&filename=secret.md"),
            "resource://not-a-uuid/chunk/0?citation=cite_0123456789abcdef01234567".into(),
        ] {
            assert!(!is_canonical_resource_context_ref(&invalid), "{invalid}");
        }
    }

    #[test]
    fn table_selection_preserves_data_and_provenance_without_execution() {
        let store = ResourceStore::new_in_memory().unwrap();
        import(
            &store,
            "message-table",
            &[
                ("metrics.csv", "text/csv"),
                (
                    "metrics.xlsx",
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                ),
            ],
        );
        let selected = select(
            &store,
            "message-table",
            "总结本周和上周趋势、异常以及数据质量问题",
        );
        let filenames = selected
            .citation_set
            .citations()
            .into_iter()
            .map(|citation| citation.filename)
            .collect::<BTreeSet<_>>();
        assert!(filenames.contains("metrics.csv"));
        assert!(filenames.contains("metrics.xlsx"));
        assert!(selected
            .context_blocks
            .iter()
            .any(|block| block.content.contains("RESOURCE_ROW_SENTINEL")));
        assert!(selected
            .context_blocks
            .iter()
            .any(|block| block.content.contains("=WEBSERVICE")));
        assert!(selected
            .citation_set
            .citations()
            .iter()
            .any(|citation| { matches!(citation.provenance, ResourceProvenance::Csv { .. }) }));
        assert!(selected
            .citation_set
            .citations()
            .iter()
            .any(|citation| { matches!(citation.provenance, ResourceProvenance::Xlsx { .. }) }));
    }
}
