use crate::agent::context_assembler::MemoryHit;
use crate::memory::{MemorySearchHit, MemoryStore};
use crate::vectors::{MemoryChunk, VectorStore};
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
}

/// Service for retrieving relevant memories from both text and vector stores.
pub struct MemoryService;

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
    /// If embedding fails, gracefully falls back to text-only search.
    pub async fn retrieve_context(
        &self,
        session_id: &str,
        query: &str,
        memory_store: &MemoryStore,
        vector_store: &VectorStore,
        embedding_config: &EmbeddingConfig,
    ) -> Result<MemoryContext> {
        let start = std::time::Instant::now();

        // 1. Text search (always available)
        let text_hits = memory_store
            .search_text_memories(Some(session_id), query, 3)
            .unwrap_or_default();

        // 2. Vector search (if embedding is enabled)
        let (vector_hits, used_embedding) = if embedding_config.enabled {
            match Self::search_vector_memories(
                session_id,
                query,
                vector_store,
                embedding_config,
            )
            .await
            {
                Ok(hits) => (hits, true),
                Err(e) => {
                    eprintln!("[MemoryService] Vector search failed, falling back to text: {}", e);
                    (vec![], false)
                }
            }
        } else {
            (vec![], false)
        };

        // 3. Merge results
        let merged = Self::merge_hits(vector_hits, text_hits, 3);

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
            used_embedding,
        })
    }

    async fn search_vector_memories(
        session_id: &str,
        query: &str,
        vector_store: &VectorStore,
        config: &EmbeddingConfig,
    ) -> Result<Vec<(MemoryChunk, f32)>> {
        let embedding = crate::vectors::embed_text_with_config(
            query,
            &config.provider,
            &config.openai_base,
            &config.openai_key,
            &config.embedding_model,
            true, // embedding_enabled
        )
        .await?;

        let hits = vector_store
            .search_by_session(session_id, &embedding, 3, 1000)
            .unwrap_or_default();

        Ok(hits)
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
            "以下是你过去记忆中的相关内容，请在回应中自然地参考它们：\n{}",
            snippets.join("\n")
        )
    }
}

/// Configuration for embedding-based memory retrieval.
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub enabled: bool,
    pub provider: String,
    pub openai_base: String,
    pub openai_key: String,
    pub embedding_model: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: String::new(),
            openai_base: String::new(),
            openai_key: String::new(),
            embedding_model: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_context_empty() {
        let context = MemoryService::format_context(&[]);
        assert!(context.is_empty());
    }

    #[test]
    fn test_format_context_with_hits() {
        let hits = vec![
            MemoryHit {
                id: 1,
                content: "测试内容".to_string(),
                source: "chat".to_string(),
                score: 0.92,
                tier: 1,
            },
        ];
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
}
