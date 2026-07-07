use crate::commands::settings::{
    require_danger_action_confirmation, DangerActionConfirmationEvidence,
};
use crate::errors::AppError;
use crate::memory_gateway;
use crate::AppState;
use openlife_core::memory_cache::HotMemoryCache;
use openlife_core::vectors::{ArchivedChunkSummary, MemoryChunk, TierStats};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn run_memory_tier_maintenance(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let (promoted, demoted) =
        memory_gateway::run_memory_tier_maintenance_with_state(state.inner()).await?;
    Ok(serde_json::json!({ "promoted": promoted, "demoted": demoted }))
}

#[tauri::command]
pub async fn count_memory_chunks(state: State<'_, Arc<AppState>>) -> Result<i64, AppError> {
    memory_gateway::count_memory_chunks_with_state(state.inner()).await
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
    memory_gateway::index_memory_chunk_with_state(session_id, content, source, state).await
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
    memory_gateway::search_memory_with_state(query, top_k, state).await
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
    memory_gateway::archive_low_access_memories_with_state(state.inner()).await
}

#[tauri::command]
pub async fn restore_archived_chunks(
    chunk_ids: Vec<i64>,
    state: State<'_, Arc<AppState>>,
) -> Result<usize, AppError> {
    memory_gateway::restore_archived_chunks_with_state(&chunk_ids, state.inner()).await
}

#[tauri::command]
pub async fn list_archived_chunks(
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ArchivedChunkSummary>, AppError> {
    memory_gateway::list_archived_chunks_with_state(limit, state.inner()).await
}

#[tauri::command]
pub async fn get_memory_tier_stats(state: State<'_, Arc<AppState>>) -> Result<TierStats, AppError> {
    memory_gateway::get_memory_tier_stats_with_state(state.inner()).await
}

#[tauri::command]
pub async fn rebuild_memory_index(
    confirmation_evidence: Option<DangerActionConfirmationEvidence>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let affected_count = {
        let store = state.memory_store.lock().await;
        store.export_all_messages().map_err(AppError::from)?.len()
    };
    require_danger_action_confirmation(
        "vector_rebuild",
        &[],
        Some(affected_count),
        confirmation_evidence.as_ref(),
        state.inner(),
    )
    .await?;
    rebuild_memory_index_with_state(state.inner()).await
}

pub(crate) async fn rebuild_memory_index_with_state(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    memory_gateway::rebuild_memory_index_with_state(state).await
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
