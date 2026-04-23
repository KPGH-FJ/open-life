use crate::AppState;
use openlife_core::llm::ChatMessage;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_chat_history(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ChatMessage>, String> {
    let store = state.memory_store.lock().await;
    store
        .load_recent_messages(&session_id, 200)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_chat_message(
    session_id: String,
    message: ChatMessage,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let store = state.memory_store.lock().await;
    store
        .save_message(&session_id, &message)
        .map_err(|e| e.to_string())?;
    store
        .touch_chat_session(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_chat_session(
    session_id: String,
    title: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let store = state.memory_store.lock().await;
    store
        .create_chat_session(&session_id, &title)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_chat_session(
    session_id: String,
    title: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let store = state.memory_store.lock().await;
    store
        .rename_chat_session(&session_id, &title)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_chat_session(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let store = state.memory_store.lock().await;
    store
        .delete_chat_session(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_chat_sessions(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::memory::ChatSession>, String> {
    let store = state.memory_store.lock().await;
    store.list_chat_sessions(200).map_err(|e| e.to_string())
}
