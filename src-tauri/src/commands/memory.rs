use openlife_core::memory_cache::HotMemoryCache;
use openlife_core::vectors::{embed_text, ArchivedChunkSummary, MemoryChunk, TierStats};
use std::sync::Arc;
use tauri::State;

use crate::{merge_memory_hits, AppState};

#[tauri::command]
pub async fn run_memory_tier_maintenance(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let store = state.vector_store.lock().await;
    let (promoted, demoted) = store.run_tier_maintenance().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "promoted": promoted, "demoted": demoted }))
}

#[tauri::command]
pub async fn count_memory_chunks(state: State<'_, Arc<AppState>>) -> Result<i64, String> {
    let store = state.vector_store.lock().await;
    store.count_all_chunks().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn index_memory_chunk(
    session_id: String,
    content: String,
    source: String,
    state: State<'_, Arc<AppState>>,
) -> Result<i64, String> {
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
        .map_err(|e| e.to_string())?;
    let embedding_id = {
        let store = state.vector_store.lock().await;
        store
            .insert(&session_id, &content, &embedding, &source)
            .map_err(|e| e.to_string())?
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
) -> Result<Vec<(MemoryChunk, f32)>, String> {
    let text_hits = {
        let store = state.memory_store.lock().await;
        store
            .search_text_memories(None, &query, top_k)
            .map_err(|e| e.to_string())?
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
            store.search(&embedding, top_k).map_err(|e| e.to_string())?
        }
        Err(_) => vec![],
    };
    Ok(merge_memory_hits(vector_hits, text_hits, top_k))
}

#[tauri::command]
pub async fn get_hot_cache(state: State<'_, Arc<AppState>>) -> Result<HotMemoryCache, String> {
    let cache = state.hot_cache.lock().unwrap();
    Ok(cache.clone())
}

#[tauri::command]
pub async fn archive_low_access_memories(state: State<'_, Arc<AppState>>) -> Result<usize, String> {
    let store = state.vector_store.lock().await;
    store
        .archive_low_access_memories()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_archived_chunks(
    chunk_ids: Vec<i64>,
    state: State<'_, Arc<AppState>>,
) -> Result<usize, String> {
    let store = state.vector_store.lock().await;
    store
        .restore_archived(&chunk_ids)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_archived_chunks(
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ArchivedChunkSummary>, String> {
    let store = state.vector_store.lock().await;
    store.list_archived(limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_memory_tier_stats(state: State<'_, Arc<AppState>>) -> Result<TierStats, String> {
    let store = state.vector_store.lock().await;
    store.tier_stats().map_err(|e| e.to_string())
}
