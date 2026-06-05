use crate::errors::AppError;
use crate::{merge_memory_hits, AppState};
use openlife_core::memory_cache::HotMemoryCache;
use openlife_core::vectors::{
    embed_text_with_privacy, ArchivedChunkSummary, ExportedVectorChunk, MemoryChunk, TierStats,
};
use std::sync::Arc;
use tauri::State;

#[derive(Clone)]
struct EmbeddingPrivacyContext {
    provider: String,
    openai_base: String,
    openai_key: String,
    embedding_model: String,
    embedding_enabled: bool,
    privacy_engine: openlife_core::privacy::PrivacyEngine,
}

async fn embedding_privacy_context(state: &Arc<AppState>) -> EmbeddingPrivacyContext {
    let (provider, openai_base, openai_key, embedding_model, embedding_enabled) = {
        let cfg = state.config.lock().await;
        (
            cfg.llm.provider.clone(),
            cfg.llm.openai_base.clone(),
            cfg.llm.openai_key.clone(),
            cfg.llm.embedding_model.clone(),
            cfg.llm.embedding_enabled,
        )
    };
    let privacy_engine = {
        let engine = state.privacy_engine.lock().await;
        engine.clone()
    };

    EmbeddingPrivacyContext {
        provider,
        openai_base,
        openai_key,
        embedding_model,
        embedding_enabled,
        privacy_engine,
    }
}

async fn embed_memory_text_with_privacy(
    text: &str,
    state: &Arc<AppState>,
) -> Result<Vec<f32>, AppError> {
    let ctx = embedding_privacy_context(state).await;
    let hs_local_only =
        crate::classify_hs_policy_topic(text, "") != openlife_core::agent::PolicyTopic::General;
    embed_text_with_privacy(
        text,
        &ctx.provider,
        &ctx.openai_base,
        &ctx.openai_key,
        &ctx.embedding_model,
        ctx.embedding_enabled,
        &ctx.privacy_engine,
        hs_local_only,
    )
    .await
    .map_err(AppError::from)
}

#[tauri::command]
pub async fn run_memory_tier_maintenance(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let store = state.vector_store.lock().await;
    let (promoted, demoted) = store.run_tier_maintenance().map_err(AppError::from)?;
    Ok(serde_json::json!({ "promoted": promoted, "demoted": demoted }))
}

#[tauri::command]
pub async fn count_memory_chunks(state: State<'_, Arc<AppState>>) -> Result<i64, AppError> {
    let store = state.vector_store.lock().await;
    store.count_all_chunks().map_err(AppError::from)
}

#[tauri::command]
pub async fn index_memory_chunk(
    session_id: String,
    content: String,
    source: String,
    state: State<'_, Arc<AppState>>,
) -> Result<i64, AppError> {
    index_memory_chunk_with_state(session_id, content, source, state.inner()).await
}

pub(crate) async fn index_memory_chunk_with_state(
    session_id: String,
    content: String,
    source: String,
    state: &Arc<AppState>,
) -> Result<i64, AppError> {
    let embedding = embed_memory_text_with_privacy(&content, state).await?;
    let embedding_id = {
        let store = state.vector_store.lock().await;
        store
            .insert(&session_id, &content, &embedding, &source)
            .map_err(AppError::from)?
    };
    {
        let store = state.memory_store.lock().await;
        let tags = vec!["manual".to_string(), format!("source:{}", source)];
        let _ = store.save_memory_record(
            &session_id,
            &content,
            "indexed_note",
            &source,
            &tags,
            "private",
            Some(embedding_id),
        );
    }
    Ok(embedding_id)
}

#[tauri::command]
pub async fn search_memory(
    query: String,
    top_k: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<(MemoryChunk, f32)>, AppError> {
    search_memory_with_state(query, top_k, state.inner()).await
}

pub(crate) async fn search_memory_with_state(
    query: String,
    top_k: usize,
    state: &Arc<AppState>,
) -> Result<Vec<(MemoryChunk, f32)>, AppError> {
    let desensitized_query = {
        let privacy_engine = state.privacy_engine.lock().await;
        privacy_engine.desensitize(&query).0
    };
    let text_hits = {
        let store = state.memory_store.lock().await;
        store
            .search_text_memories(None, &desensitized_query, top_k)
            .map_err(AppError::from)?
    };
    let vector_hits = match embed_memory_text_with_privacy(&query, state).await {
        Ok(embedding) => {
            let store = state.vector_store.lock().await;
            store.search(&embedding, top_k).map_err(AppError::from)?
        }
        Err(_) => vec![],
    };
    Ok(merge_memory_hits(vector_hits, text_hits, top_k))
}

#[tauri::command]
pub async fn get_hot_cache(state: State<'_, Arc<AppState>>) -> Result<HotMemoryCache, AppError> {
    let cache = state.hot_cache.read().await;
    Ok(cache.clone())
}

#[tauri::command]
pub async fn archive_low_access_memories(
    state: State<'_, Arc<AppState>>,
) -> Result<usize, AppError> {
    let store = state.vector_store.lock().await;
    store.archive_low_access_memories().map_err(AppError::from)
}

#[tauri::command]
pub async fn restore_archived_chunks(
    chunk_ids: Vec<i64>,
    state: State<'_, Arc<AppState>>,
) -> Result<usize, AppError> {
    let store = state.vector_store.lock().await;
    store.restore_archived(&chunk_ids).map_err(AppError::from)
}

#[tauri::command]
pub async fn list_archived_chunks(
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ArchivedChunkSummary>, AppError> {
    let store = state.vector_store.lock().await;
    store.list_archived(limit).map_err(AppError::from)
}

#[tauri::command]
pub async fn get_memory_tier_stats(state: State<'_, Arc<AppState>>) -> Result<TierStats, AppError> {
    let store = state.vector_store.lock().await;
    store.tier_stats().map_err(AppError::from)
}

#[tauri::command]
pub async fn rebuild_memory_index(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    rebuild_memory_index_with_state(state.inner()).await
}

pub(crate) async fn rebuild_memory_index_with_state(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    let messages = {
        let store = state.memory_store.lock().await;
        store.export_all_messages().map_err(AppError::from)?
    };
    let previous_vectors = {
        let store = state.vector_store.lock().await;
        store.export_all_chunks().map_err(AppError::from)?
    };

    let mut rebuilt = Vec::<ExportedVectorChunk>::new();
    let mut skipped = 0_usize;
    for msg in messages {
        let content = msg.content.trim().to_string();
        if content.is_empty() {
            skipped += 1;
            continue;
        }
        let embedding = embed_memory_text_with_privacy(&content, state)
            .await
            .map_err(|e| AppError::internal(format!("重建向量索引时生成 embedding 失败: {}", e)))?;
        if embedding.is_empty() {
            skipped += 1;
            continue;
        }
        rebuilt.push(ExportedVectorChunk {
            session_id: msg.session_id,
            content,
            embedding,
            source: format!("rebuild:{}", msg.role),
            created_at: msg.created_at,
            tier: 2,
            access_count: 0,
            last_accessed_at: String::new(),
            importance_score: 0.5,
            archived: false,
            archived_at: None,
            summary: None,
        });
    }

    {
        let store = state.vector_store.lock().await;
        if let Err(rebuild_error) = store.replace_all_chunks(&rebuilt) {
            let rollback_error = store.replace_all_chunks(&previous_vectors).err();
            if let Some(rollback_error) = rollback_error {
                return Err(AppError::internal(format!(
                    "重建向量索引失败，且回滚失败。重建错误: {}; 回滚错误: {}",
                    rebuild_error, rollback_error
                )));
            }
            return Err(AppError::internal(format!(
                "重建向量索引失败，已回滚: {}",
                rebuild_error
            )));
        }
    }

    Ok(serde_json::json!({
        "processed": rebuilt.len() + skipped,
        "indexed": rebuilt.len(),
        "skipped": skipped,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::{llm::ChatMessage, vectors::clear_embedding_cache};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc as StdArc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn fake_cloud_embedding_endpoint() -> (String, StdArc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cloud_call_count = StdArc::new(AtomicUsize::new(0));
        let cloud_call_count_clone = cloud_call_count.clone();

        tokio::spawn(async move {
            loop {
                let accepted =
                    tokio::time::timeout(std::time::Duration::from_millis(750), listener.accept())
                        .await;
                let Ok(Ok((mut socket, _))) = accepted else {
                    break;
                };
                cloud_call_count_clone.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 2048];
                let _ = socket.read(&mut buf).await;
                let body = r#"{"data":[{"embedding":[0.1,0.2,0.3]}]}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });

        (format!("http://{}", addr), cloud_call_count)
    }

    async fn configure_cloud_embeddings(state: &Arc<AppState>, openai_base: String) {
        let mut cfg = state.config.lock().await;
        cfg.llm.provider = "openai".to_string();
        cfg.llm.openai_base = openai_base;
        cfg.llm.openai_key = "sk-test".to_string();
        cfg.llm.embedding_model = "text-embedding-3-small".to_string();
        cfg.llm.embedding_enabled = true;
    }

    #[tokio::test]
    async fn index_memory_chunk_sensitive_content_does_not_call_cloud_embedding() {
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        let (openai_base, cloud_call_count) = fake_cloud_embedding_endpoint().await;
        configure_cloud_embeddings(&state, openai_base).await;

        let id = index_memory_chunk_with_state(
            "memory-index-sensitive".to_string(),
            "身份证 11010519491231002X，邮箱 index-sensitive@example.com，最近用药焦虑".to_string(),
            "manual".to_string(),
            &state,
        )
        .await
        .unwrap();

        assert!(id > 0);
        assert_eq!(cloud_call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn search_memory_sensitive_query_does_not_call_cloud_embedding() {
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        let (openai_base, cloud_call_count) = fake_cloud_embedding_endpoint().await;
        configure_cloud_embeddings(&state, openai_base).await;

        let hits = search_memory_with_state(
            "银行卡 6222 0202 0202 0202，邮箱 search-sensitive@example.com，健康诊断".to_string(),
            5,
            &state,
        )
        .await
        .unwrap();

        assert!(hits.is_empty());
        assert_eq!(cloud_call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn rebuild_memory_index_sensitive_history_does_not_call_cloud_embedding() {
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        let (openai_base, cloud_call_count) = fake_cloud_embedding_endpoint().await;
        configure_cloud_embeddings(&state, openai_base).await;

        {
            let store = state.memory_store.lock().await;
            store
                .save_message(
                    "rebuild-sensitive",
                    &ChatMessage {
                        role: "user".to_string(),
                        content:
                            "身份证 11010519491231002X，邮箱 rebuild-sensitive@example.com，健康诊断"
                                .to_string(),
                    },
                )
                .unwrap();
            store
                .save_message(
                    "rebuild-sensitive",
                    &ChatMessage {
                        role: "user".to_string(),
                        content: "银行卡 6222 0202 0202 0202，负债和贷款压力".to_string(),
                    },
                )
                .unwrap();
        }

        let report = rebuild_memory_index_with_state(&state).await.unwrap();

        assert_eq!(report["skipped"], 0);
        assert_eq!(cloud_call_count.load(Ordering::SeqCst), 0);
    }
}
