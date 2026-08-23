use std::collections::HashMap;
use std::sync::Arc;

use openlife_core::memory::MemorySearchHit;
use openlife_core::vectors::{CanonicalVectorOwnerRef, MemoryChunk};

use crate::AppState;

const MEMORY_LIFECYCLE_SOURCE_PREFIX: &str = "memory_lifecycle:";

/// Keep vector/text recall subordinate to the canonical Memory lifecycle.
/// Index presence alone never proves that a Memory fact remains active.
pub(crate) async fn filter_canonical_retrievable_memory_results(
    results: Vec<(MemoryChunk, f32)>,
    state: &Arc<AppState>,
) -> Result<Vec<(MemoryChunk, f32)>, String> {
    let lifecycle_reader = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(|| "memory_retrieval_degraded:lifecycle_store_unavailable".to_string())?
        .lock()
        .await
        .retrieval_reader();
    lifecycle_reader.ensure_available().map_err(|error| {
        format!("memory_retrieval_degraded:lifecycle_store_query_failed:{error}")
    })?;

    let lifecycle_ids = results
        .iter()
        .filter_map(|(chunk, _)| lifecycle_memory_id_from_source(&chunk.source))
        .collect::<Vec<_>>();
    let mut active_lifecycle_ids = std::collections::HashSet::new();
    for memory_id in lifecycle_ids {
        if lifecycle_reader
            .is_memory_retrievable(memory_id)
            .map_err(|error| {
                format!("memory_retrieval_degraded:lifecycle_state_query_failed:{error}")
            })?
        {
            active_lifecycle_ids.insert(memory_id.to_string());
        }
    }

    let non_lifecycle_sources = results
        .iter()
        .filter(|(chunk, _)| lifecycle_memory_id_from_source(&chunk.source).is_none())
        .filter_map(|(chunk, _)| {
            canonical_memory_owner_from_source(&chunk.source)
                .map(|owner| (chunk.source.clone(), owner))
        })
        .collect::<Vec<_>>();
    let verified_sources = if non_lifecycle_sources.is_empty() {
        std::collections::HashSet::new()
    } else {
        let store = state.memory_store.lock().await;
        let mut verified = std::collections::HashSet::new();
        for (source, owner) in non_lifecycle_sources {
            let proven = store
                .is_verified_canonical_memory_owner(&owner)
                .map_err(|error| {
                    format!("memory_retrieval_degraded:owner_proof_query_failed:{error}")
                })?;
            let retrieval_active = store.is_memory_retrieval_active(&owner).map_err(|error| {
                format!("memory_retrieval_degraded:retrieval_state_query_failed:{error}")
            })?;
            if proven && retrieval_active {
                verified.insert(source);
            }
        }
        verified
    };

    Ok(results
        .into_iter()
        .filter(|(chunk, _)| {
            if let Some(memory_id) = lifecycle_memory_id_from_source(&chunk.source) {
                return active_lifecycle_ids.contains(memory_id);
            }
            if chunk.source.starts_with("knowledge_note:") {
                return verified_sources.contains(&chunk.source);
            }
            true
        })
        .collect())
}

fn canonical_memory_owner_from_source(source: &str) -> Option<CanonicalVectorOwnerRef> {
    for kind in ["knowledge_note", "memory_lifecycle"] {
        if let Some(id) = source.strip_prefix(&format!("{kind}:")) {
            return CanonicalVectorOwnerRef::new(kind, id).ok();
        }
    }
    None
}

fn lifecycle_memory_id_from_source(source: &str) -> Option<&str> {
    let memory_id = source.strip_prefix(MEMORY_LIFECYCLE_SOURCE_PREFIX)?;
    if memory_id.starts_with("memory:")
        && !memory_id.is_empty()
        && !memory_id
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        Some(memory_id)
    } else {
        None
    }
}

pub(crate) fn merge_memory_hits(
    vector_hits: Vec<(MemoryChunk, f32)>,
    text_hits: Vec<MemorySearchHit>,
    top_k: usize,
) -> Vec<(MemoryChunk, f32)> {
    let mut merged: HashMap<(u8, String, String), (MemoryChunk, f32)> = HashMap::new();
    for (chunk, score) in vector_hits {
        let key = memory_hit_merge_key(&chunk);
        merged
            .entry(key)
            .and_modify(|(_, existing)| *existing = existing.max(score))
            .or_insert((chunk, score));
    }
    for hit in text_hits {
        let key = memory_hit_merge_key(&hit.chunk);
        merged
            .entry(key)
            .and_modify(|(_, existing)| *existing = existing.max(hit.relevance_score))
            .or_insert((hit.chunk, hit.relevance_score));
    }
    let mut results = merged.into_values().collect::<Vec<_>>();
    results.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(top_k);
    results
}

fn memory_hit_merge_key(chunk: &MemoryChunk) -> (u8, String, String) {
    if lifecycle_memory_id_from_source(&chunk.source).is_some() {
        // A lifecycle source is the canonical owner identity. Its indexed
        // session field is projection metadata and may differ from the
        // current lexical candidate after a scope migration or rebuild.
        (1, chunk.source.clone(), String::new())
    } else {
        (0, chunk.session_id.clone(), chunk.content.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(session_id: &str, source: &str, content: &str) -> MemoryChunk {
        MemoryChunk {
            id: 1,
            session_id: session_id.into(),
            content: content.into(),
            source: source.into(),
            created_at: String::new(),
            tier: 0,
            access_count: 0,
            last_accessed_at: String::new(),
            importance_score: 0.0,
            archived: false,
            archived_at: None,
            summary: None,
        }
    }

    #[test]
    fn lifecycle_hits_merge_by_canonical_memory_owner_not_projection_session() {
        let source = "memory_lifecycle:memory:11111111-1111-4111-8111-111111111111";
        let vector = chunk("historical-session", source, "same canonical fact");
        let lexical = chunk("global", source, "same canonical fact");

        let merged = merge_memory_hits(
            vec![(vector, 0.9)],
            vec![MemorySearchHit {
                chunk: lexical,
                relevance_score: 0.6,
                source_tier: "canonical_lifecycle_lexical".into(),
            }],
            4,
        );

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].1, 0.9);
        assert_eq!(merged[0].0.source, source);
    }

    #[test]
    fn ordinary_hits_keep_session_and_content_identity() {
        let first = chunk("session-a", "legacy", "same text");
        let second = chunk("session-b", "legacy", "same text");

        let merged = merge_memory_hits(vec![(first, 0.9), (second, 0.8)], vec![], 4);

        assert_eq!(merged.len(), 2);
    }
}
