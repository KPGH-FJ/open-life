use crate::agent::context_assembler::MemoryHit;
use crate::embedding::{
    execute_embedding, prepare_embedding_request_recorded, EmbeddingInvocationReceipt,
    EmbeddingProfile, EmbeddingRouteConfig, PreparedEmbeddingRequestOutcome,
    UNKNOWN_EMBEDDING_PROFILE_ID,
};
use crate::memory::MemorySearchHit;
use crate::vectors::{MemoryChunk, VectorRebuildEvidence, VectorSearchOutcome, VectorStore};
use anyhow::Result;

/// Context retrieved from memory stores (text + vector).
pub struct MemoryContext {
    /// Formatted memory context string for prompt injection
    pub context: String,
    /// Individual memory hits with metadata
    pub hits: Vec<MemoryHit>,
    /// Retrieval time in milliseconds
    pub retrieval_time_ms: u64,
    /// Whether embedding was used (false = text-only fallback)
    pub used_embedding: bool,
    pub embedding_profile: Option<EmbeddingProfile>,
    pub embedding_receipt: Option<EmbeddingInvocationReceipt>,
    pub embedding_rebuild_required: bool,
    pub embedding_rebuild: Option<VectorRebuildEvidence>,
    pub embedding_error: Option<String>,
}

/// Service for retrieving relevant memories from both text and vector stores.
pub struct MemoryService;

impl Default for MemoryService {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryService {
    pub fn new() -> Self {
        Self
    }

    /// Retrieve memory context for a given session and query.
    ///
    /// Strategy:
    /// 1. Search text memories (keywords)
    /// 2. If embedding is enabled, search vector memories
    /// 3. Merge and deduplicate results
    /// 4. Format into context string
    ///
    /// If embedding fails, text search remains available while the typed receipt
    /// and rebuild evidence preserve the degraded vector state for the caller.
    pub async fn retrieve_context(
        &self,
        session_id: &str,
        query: &str,
        text_hits: Vec<MemorySearchHit>,
        vector_store: &VectorStore,
        embedding_config: &EmbeddingConfig,
        top_k: usize,
    ) -> Result<MemoryContext> {
        let start = std::time::Instant::now();

        let vector_search = if embedding_config.enabled {
            Self::search_vector_memories(session_id, query, vector_store, embedding_config, top_k)
                .await
        } else {
            VectorMemorySearch::disabled()
        };

        // 3. Merge results
        let merged = Self::merge_hits(vector_search.hits, text_hits, top_k);

        // 4. Convert to MemoryHits
        let hits: Vec<MemoryHit> = merged
            .iter()
            .map(|(chunk, score)| MemoryHit {
                id: chunk.id,
                content: chunk.content.clone(),
                source: chunk.source.clone(),
                score: *score,
                tier: chunk.tier,
            })
            .collect();

        // 5. Format context string
        let context = Self::format_context(&hits);

        Ok(MemoryContext {
            context,
            hits,
            retrieval_time_ms: start.elapsed().as_millis() as u64,
            used_embedding: vector_search.used_embedding,
            embedding_profile: vector_search.profile,
            embedding_receipt: vector_search.receipt,
            embedding_rebuild_required: vector_search.rebuild_required,
            embedding_rebuild: vector_search.rebuild,
            embedding_error: vector_search.error,
        })
    }

    async fn search_vector_memories(
        session_id: &str,
        query: &str,
        vector_store: &VectorStore,
        config: &EmbeddingConfig,
        top_k: usize,
    ) -> VectorMemorySearch {
        let privacy_engine = crate::privacy::PrivacyEngine::new();
        let privacy_plan =
            crate::vectors::plan_embedding_privacy(query, &privacy_engine, config.hs_local_only);
        let prepared = match prepare_embedding_request_recorded(
            query,
            EmbeddingRouteConfig::from_product_config(
                config.provider.clone(),
                config.openai_base.clone(),
                config.embedding_model.clone(),
                config.enabled,
                &config.openai_key,
                config.credential_version,
                config.network_policy.clone(),
            ),
            privacy_plan,
        ) {
            PreparedEmbeddingRequestOutcome::Prepared(prepared) => prepared,
            PreparedEmbeddingRequestOutcome::Rejected(outcome) => {
                return VectorMemorySearch {
                    profile: Some(outcome.profile),
                    receipt: Some(outcome.receipt),
                    error: outcome.result.err(),
                    ..VectorMemorySearch::disabled()
                }
            }
        };
        let outcome = execute_embedding(prepared).await;
        let profile = outcome.profile.clone();
        let receipt = outcome.receipt.clone();
        let embedding = match outcome.result {
            Ok(embedding) => embedding,
            Err(error) => {
                return VectorMemorySearch {
                    profile: Some(profile),
                    receipt: Some(receipt),
                    error: Some(error),
                    ..VectorMemorySearch::disabled()
                }
            }
        };
        if profile.id == UNKNOWN_EMBEDDING_PROFILE_ID {
            return VectorMemorySearch {
                profile: Some(profile),
                receipt: Some(receipt),
                rebuild_required: true,
                error: Some("embedding_profile_identity_unknown".into()),
                ..VectorMemorySearch::disabled()
            };
        }
        match vector_store.search_by_session(session_id, &embedding, &profile, top_k, 1000) {
            Ok(VectorSearchOutcome::Matches { matches, rebuild }) => {
                let rebuild_required = rebuild.is_some();
                VectorMemorySearch {
                    hits: matches,
                    used_embedding: true,
                    profile: Some(profile),
                    receipt: Some(receipt),
                    rebuild_required,
                    rebuild,
                    error: rebuild_required.then(|| "vector_rebuild_required".into()),
                }
            }
            Ok(VectorSearchOutcome::RebuildRequired(rebuild)) => VectorMemorySearch {
                profile: Some(profile),
                receipt: Some(receipt),
                rebuild_required: true,
                rebuild: Some(rebuild),
                error: Some("vector_rebuild_required".into()),
                ..VectorMemorySearch::disabled()
            },
            Err(error) => VectorMemorySearch {
                profile: Some(profile),
                receipt: Some(receipt),
                error: Some(error.to_string()),
                ..VectorMemorySearch::disabled()
            },
        }
    }

    fn merge_hits(
        vector_hits: Vec<(MemoryChunk, f32)>,
        text_hits: Vec<MemorySearchHit>,
        top_k: usize,
    ) -> Vec<(MemoryChunk, f32)> {
        use std::collections::HashMap;

        let mut merged: HashMap<(String, String), (MemoryChunk, f32)> = HashMap::new();

        for (chunk, score) in vector_hits {
            let key = (chunk.session_id.clone(), chunk.content.clone());
            merged
                .entry(key)
                .and_modify(|(_, s)| *s = s.max(score))
                .or_insert((chunk, score));
        }

        for hit in text_hits {
            let key = (hit.chunk.session_id.clone(), hit.chunk.content.clone());
            merged
                .entry(key)
                .and_modify(|(_, s)| *s = s.max(hit.relevance_score))
                .or_insert((hit.chunk, hit.relevance_score));
        }

        let mut results: Vec<_> = merged.into_values().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    fn format_context(hits: &[MemoryHit]) -> String {
        if hits.is_empty() {
            return String::new();
        }

        let snippets: Vec<String> = hits
            .iter()
            .map(|hit| {
                format!(
                    "- [{}] {} (相关度: {:.2})",
                    hit.source,
                    hit.content.replace('\n', " "),
                    hit.score
                )
            })
            .collect();

        format!(
            "以下是你过去记忆中与当前问题相关的内容。仅在它们和当前用户指令直接相关时参考；当前用户指令优先，不要把旧任务状态当作当前事实：\n{}",
            snippets.join("\n")
        )
    }
}

/// Configuration for embedding-based memory retrieval.
#[derive(Debug, Clone, Default)]
pub struct EmbeddingConfig {
    pub enabled: bool,
    pub provider: String,
    pub openai_base: String,
    pub openai_key: String,
    pub embedding_model: String,
    pub hs_local_only: bool,
    pub credential_version: u64,
    pub network_policy: crate::config::NetworkPolicy,
}

struct VectorMemorySearch {
    hits: Vec<(MemoryChunk, f32)>,
    used_embedding: bool,
    profile: Option<EmbeddingProfile>,
    receipt: Option<EmbeddingInvocationReceipt>,
    rebuild_required: bool,
    rebuild: Option<VectorRebuildEvidence>,
    error: Option<String>,
}

impl VectorMemorySearch {
    fn disabled() -> Self {
        Self {
            hits: Vec::new(),
            used_embedding: false,
            profile: None,
            receipt: None,
            rebuild_required: false,
            rebuild: None,
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::EmbeddingInvocationStatus;

    #[test]
    fn test_format_context_empty() {
        let context = MemoryService::format_context(&[]);
        assert!(context.is_empty());
    }

    #[test]
    fn test_format_context_with_hits() {
        let hits = vec![MemoryHit {
            id: 1,
            content: "测试内容".to_string(),
            source: "chat".to_string(),
            score: 0.92,
            tier: 1,
        }];
        let context = MemoryService::format_context(&hits);
        assert!(context.contains("测试内容"));
        assert!(context.contains("chat"));
        assert!(context.contains("0.92"));
    }

    #[test]
    fn test_merge_hits_deduplicates() {
        let chunk1 = MemoryChunk {
            id: 1,
            session_id: "s1".to_string(),
            content: "相同内容".to_string(),
            source: "chat".to_string(),
            created_at: String::new(),
            tier: 1,
            access_count: 0,
            last_accessed_at: String::new(),
            importance_score: 0.0,
            archived: false,
            archived_at: None,
            summary: None,
        };

        let vector_hits = vec![(chunk1.clone(), 0.8)];
        let text_hits = vec![MemorySearchHit {
            chunk: chunk1,
            relevance_score: 0.9,
            source_tier: "text".to_string(),
        }];

        let merged = MemoryService::merge_hits(vector_hits, text_hits, 3);
        assert_eq!(merged.len(), 1);
        assert!((merged[0].1 - 0.9).abs() < 0.01); // Should take max score
    }

    #[tokio::test]
    async fn prepare_failure_keeps_text_hits_and_not_attempted_receipt() {
        let store = VectorStore::new_in_memory().unwrap();
        let text_hits = vec![MemorySearchHit {
            chunk: MemoryChunk {
                id: 42,
                session_id: "session-prepare-failure".into(),
                content: "text retrieval remains available".into(),
                source: "manual".into(),
                created_at: chrono::Utc::now().to_rfc3339(),
                tier: 1,
                access_count: 0,
                last_accessed_at: chrono::Utc::now().to_rfc3339(),
                importance_score: 0.5,
                archived: false,
                archived_at: None,
                summary: None,
            },
            relevance_score: 0.9,
            source_tier: "text".into(),
        }];
        let context = MemoryService::new()
            .retrieve_context(
                "session-prepare-failure",
                "",
                text_hits,
                &store,
                &EmbeddingConfig {
                    enabled: true,
                    provider: "openai".into(),
                    openai_base: "https://api.openai.com/v1".into(),
                    openai_key: "test-key".into(),
                    embedding_model: "text-embedding-3-small".into(),
                    hs_local_only: false,
                    credential_version: 1,
                    network_policy: crate::config::NetworkPolicy::default(),
                },
                5,
            )
            .await
            .unwrap();

        assert_eq!(context.hits.len(), 1);
        assert_eq!(context.hits[0].content, "text retrieval remains available");
        let receipt = context
            .embedding_receipt
            .expect("prepare rejection must be observable");
        assert_eq!(receipt.status, EmbeddingInvocationStatus::NotAttempted);
        assert!(receipt.provider_dispatches.is_empty());
        assert!(!context.used_embedding);
    }
}
