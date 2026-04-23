use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use openlife_core::a2a::{A2AServerHandler, SendTaskRequest, SendTaskResponse};
use std::sync::Arc;

use crate::AppState;

pub const A2A_PORT: u16 = 8765;

pub async fn start(state: Arc<AppState>) {
    let app = Router::new()
        .route("/agent.json", get(agent_card_handler))
        .route("/tasks/send", post(send_task))
        .with_state(state);

    let bind_addr = format!("127.0.0.1:{}", A2A_PORT);
    match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => {
            let addr = listener.local_addr().unwrap();
            println!(
                "[A2A] HTTP server listening on http://{} - a2a_server.rs:23",
                addr
            );
            tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, app).await {
                    eprintln!("[A2A] Server error: {} - a2a_server.rs:26", e);
                }
            });
        }
        Err(e) => {
            eprintln!("[A2A] Failed to bind server: {} - a2a_server.rs:31", e);
        }
    }
}

async fn agent_card_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<openlife_core::a2a::AgentCard>, axum::http::StatusCode> {
    let model = state
        .life_model_manager
        .lock()
        .await
        .load()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let card = A2AServerHandler::default_agent_card(A2A_PORT, &model);
    Ok(Json(card))
}

async fn send_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SendTaskRequest>,
) -> Result<Json<SendTaskResponse>, axum::http::StatusCode> {
    let life_model = state
        .life_model_manager
        .lock()
        .await
        .load()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let privacy_engine = state.privacy_engine.lock().await.clone();
    let handler = A2AServerHandler {
        life_model,
        privacy_engine,
    };
    let resp = handler.handle_task(req);
    Ok(Json(resp))
}
