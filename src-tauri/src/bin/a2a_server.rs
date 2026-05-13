use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use openlife_core::a2a::{A2AServerHandler, SendTaskRequest, SendTaskResponse};
use openlife_core::life_model::LifeModelManager;
use openlife_core::privacy::PrivacyEngine;
use std::sync::Arc;

struct AppState {
    life_model_manager: LifeModelManager,
    privacy_engine: PrivacyEngine,
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("A2A_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8765);

    let data_dir = dirs::data_dir()
        .map(|d| d.join("ai.openlife.desktop"))
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("ai.openlife.desktop");

    let life_model_manager = LifeModelManager::new(data_dir.join("life-model").join("current"));
    let privacy_engine = PrivacyEngine::new();

    let life_model = life_model_manager
        .load()
        .unwrap_or_else(|_| openlife_core::life_model::LifeModel::default_model());

    let state = Arc::new(AppState {
        life_model_manager,
        privacy_engine,
    });

    let card = A2AServerHandler::default_agent_card(port, &life_model);
    let app = Router::new()
        .route(
            "/agent.json",
            get(move || async move { Json(card.clone()) }),
        )
        .route("/tasks/send", post(send_task))
        .with_state(state);

    let bind_addr = format!("127.0.0.1:{}", port);
    match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => {
            let addr = listener.local_addr().unwrap();
            println!(
                "[A2A] HTTP server listening on http://{} - a2a_server.rs:49",
                addr
            );
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("[A2A] Server error: {}", e);
                return;
            }
        }
        Err(e) => {
            eprintln!("[A2A] Failed to bind server: {}", e);
            // 尝试其他端口或优雅退出
            for port in 8766..=8775 {
                let bind_addr = format!("127.0.0.1:{}", port);
                match tokio::net::TcpListener::bind(&bind_addr).await {
                    Ok(listener) => {
                        let addr = listener.local_addr().unwrap();
                        println!("[A2A] HTTP server listening on http://{}", addr);
                        if let Err(e) = axum::serve(listener, app).await {
                            eprintln!("[A2A] Server error: {}", e);
                        }
                        return;
                    }
                    Err(_) => continue,
                }
            }
            eprintln!("[A2A] Could not bind to any port in range 8765-8775");
        }
    }
}

async fn send_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SendTaskRequest>,
) -> Result<Json<SendTaskResponse>, axum::http::StatusCode> {
    let life_model = state
        .life_model_manager
        .load()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let handler = A2AServerHandler {
        life_model,
        privacy_engine: state.privacy_engine.clone(),
    };
    let resp = handler.handle_task(req);
    Ok(Json(resp))
}
