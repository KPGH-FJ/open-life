use crate::errors::AppError;
use crate::{merge_memory_hits, AppState};
use openlife_core::memory_cache::HotMemoryCache;
use openlife_core::vectors::{
    embed_text, embed_text_with_config, ArchivedChunkSummary, ExportedVectorChunk, MemoryChunk,
    TierStats,
};
use std::sync::Arc;
use tauri::State;

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
    let (openai_base, openai_key, embedding_model) = {
        let cfg = state.config.lock().await;
        (
            cfg.llm.openai_base.clone(),
            cfg.llm.openai_key.clone(),
            cfg.llm.embedding_model.clone(),
        )
    };
    let embedding = embed_text(&content, &openai_base, &openai_key, &embedding_model)
        .await
        .map_err(AppError::from)?;
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
    let text_hits = {
        let store = state.memory_store.lock().await;
        store
            .search_text_memories(None, &query, top_k)
            .map_err(AppError::from)?
    };
    let (openai_base, openai_key, embedding_model) = {
        let cfg = state.config.lock().await;
        (
            cfg.llm.openai_base.clone(),
            cfg.llm.openai_key.clone(),
            cfg.llm.embedding_model.clone(),
        )
    };
    let vector_hits = match embed_text(&query, &openai_base, &openai_key, &embedding_model).await {
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
    let messages = {
        let store = state.memory_store.lock().await;
        store.export_all_messages().map_err(AppError::from)?
    };
    let previous_vectors = {
        let store = state.vector_store.lock().await;
        store.export_all_chunks().map_err(AppError::from)?
    };
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

    let mut rebuilt = Vec::<ExportedVectorChunk>::new();
    let mut skipped = 0_usize;
    for msg in messages {
        let content = msg.content.trim().to_string();
        if content.is_empty() {
            skipped += 1;
            continue;
        }
        let embedding = embed_text_with_config(
            &content,
            &provider,
            &openai_base,
            &openai_key,
            &embedding_model,
            embedding_enabled,
        )
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
