use crate::AppState;
use openlife_core::hermes::{HermesContext, HermesRequest, HermesTrace};
use openlife_core::llm::ChatMessage;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn hermes_dispatch(
    session_id: String,
    method: String,
    messages: Vec<ChatMessage>,
    state: State<'_, Arc<AppState>>,
) -> Result<HermesTrace, String> {
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    let tools_prompt = {
        let reg = state.mcp_registry.lock().await;
        reg.tools_prompt()
    };
    let life_model_yaml = serde_yaml::to_string(&life_model).unwrap_or_default();
    let scheduler_clone = state.scheduler.lock().await.clone();
    let req = HermesRequest::new(
        &method,
        Some(serde_json::json!({"session_id": &session_id})),
    );
    let mut ctx = HermesContext {
        life_model_yaml,
        life_model: Some(life_model),
        recent_messages: messages,
        tools_prompt: Some(tools_prompt),
        memory_context: String::new(),
        extras: HashMap::new(),
        ..Default::default()
    };
    let bus = openlife_core::hermes::build_bus(
        ctx.life_model.clone().unwrap_or_default(),
        scheduler_clone,
    );
    bus.dispatch_with_arbitration(&req, &mut ctx)
        .await
        .map_err(|e| e)
}
