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

    let verified_sources = {
        let store = state.memory_store.lock().await;
        let mut verified = std::collections::HashSet::new();
        for (chunk, _) in &results {
            let Some(owner) = canonical_memory_owner_from_source(&chunk.source) else {
                continue;
            };
            let proven = store
                .is_verified_canonical_memory_owner(&owner)
                .map_err(|error| {
                    format!("memory_retrieval_degraded:owner_proof_query_failed:{error}")
                })?;
            let retrieval_active = if owner.kind() == "memory_lifecycle" {
                true
            } else {
                store.is_memory_retrieval_active(&owner).map_err(|error| {
                    format!("memory_retrieval_degraded:retrieval_state_query_failed:{error}")
                })?
            };
            if proven && retrieval_active {
                verified.insert(chunk.source.clone());
            }
        }
        verified
    };

    Ok(results
        .into_iter()
        .filter(|(chunk, _)| {
            if let Some(memory_id) = lifecycle_memory_id_from_source(&chunk.source) {
                return active_lifecycle_ids.contains(memory_id)
                    && verified_sources.contains(&chunk.source);
            }
            if chunk.source.starts_with("knowledge_note:")
                || chunk.source.starts_with("memory_record:")
            {
                return verified_sources.contains(&chunk.source);
            }
            true
        })
        .collect())
}

fn canonical_memory_owner_from_source(source: &str) -> Option<CanonicalVectorOwnerRef> {
    for kind in ["knowledge_note", "memory_record", "memory_lifecycle"] {
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
    let mut merged: HashMap<(String, String), (MemoryChunk, f32)> = HashMap::new();
    for (chunk, score) in vector_hits {
        let key = (chunk.session_id.clone(), chunk.content.clone());
        merged
            .entry(key)
            .and_modify(|(_, existing)| *existing = existing.max(score))
            .or_insert((chunk, score));
    }
    for hit in text_hits {
        let key = (hit.chunk.session_id.clone(), hit.chunk.content.clone());
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
