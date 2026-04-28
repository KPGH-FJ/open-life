use crate::AppState;
use openlife_core::a2a::{
    a2a_response_to_reasoning_result, reasoning_input_to_a2a_task, A2AClient, A2AServerHandler,
    AgentCard, SendTaskRequest,
};
use openlife_core::agent::ReasoningInput;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn a2a_discover_agent(url: String) -> Result<AgentCard, String> {
    let client = A2AClient::new();
    client
        .discover_agent_card(&url)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn a2a_send_task(url: String, request_json: String) -> Result<String, String> {
    let req: SendTaskRequest = serde_json::from_str(&request_json).map_err(|e| e.to_string())?;
    let client = A2AClient::new();
    let resp = client
        .send_task(&url, &req)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn a2a_local_agent_card(state: State<'_, Arc<AppState>>) -> Result<AgentCard, String> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    Ok(A2AServerHandler::default_agent_card(8765, &model))
}

#[tauri::command]
pub async fn a2a_handle_task(
    request_json: String,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let req: SendTaskRequest = serde_json::from_str(&request_json).map_err(|e| e.to_string())?;
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    let privacy_engine = state.privacy_engine.lock().await.clone();
    let handler = A2AServerHandler {
        life_model,
        privacy_engine,
    };
    let resp = handler.handle_task(req);
    serde_json::to_string(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn a2a_bridge_local(
    session_id: Option<String>,
    method: String,
    text: String,
    skill: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let req = ReasoningInput {
        task_kind: openlife_core::agent::AgentTaskKind::Conversation,
        user_text: text.clone(),
        session_id: session_id.clone().unwrap_or_default(),
    };
    let a2a_req = reasoning_input_to_a2a_task(&req, skill.as_deref(), None);
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    let privacy_engine = state.privacy_engine.lock().await.clone();
    let handler = A2AServerHandler {
        life_model,
        privacy_engine,
    };
    let resp = handler.handle_task(a2a_req);
    let reasoning_result = a2a_response_to_reasoning_result(&resp).map_err(|e| e.to_string())?;
    let bridge_preview = reasoning_input_to_a2a_task(
        &ReasoningInput {
            task_kind: openlife_core::agent::AgentTaskKind::Conversation,
            user_text: text,
            session_id: session_id.unwrap_or_default(),
        },
        None,
        None,
    );
    Ok(serde_json::json!({
        "request": {
            "task_kind": "conversation",
            "user_text": req.user_text,
            "session_id": req.session_id,
        },
        "a2a_request": bridge_preview,
        "response": resp,
        "reasoning_result": reasoning_result,
    }))
}

#[tauri::command]
pub async fn a2a_restart_sidecar(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let sidecar = state.a2a_sidecar.lock().await;
    sidecar.stop().ok();
    sidecar.start().await
}

#[tauri::command]
pub async fn a2a_stop_sidecar(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let sidecar = state.a2a_sidecar.lock().await;
    sidecar.stop()
}
