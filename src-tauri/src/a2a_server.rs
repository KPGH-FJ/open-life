use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use openlife_core::a2a::AgentCard;
use openlife_core::a2a::{A2AServerHandler, SendTaskRequest, SendTaskResponse};
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::AppState;

pub const A2A_PORT: u16 = 8765;

#[derive(Clone)]
pub struct A2AServerState {
    pub app_state: Arc<AppState>,
    pub bearer_token: String,
    pub instance_id: String,
}

/// Check whether a reachable local A2A server is the genuine OpenLife instance.
/// Sends a challenge to /health and verifies the proof response using the
/// local bearer token as the shared secret.
pub async fn has_reachable_local_server(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/agent.json", port);
    let client = reqwest::Client::new();
    let card: AgentCard = match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json().await {
            Ok(card) => card,
            Err(_) => return false,
        },
        _ => return false,
    };
    if card.name != "OpenLife" {
        return false;
    }
    // Get expected instance_id and local token
    let expected_iid = load_or_generate_instance_id();
    let local_token = load_or_generate_a2a_token();
    if local_token.is_empty() {
        return false;
    }
    // Verify via challenge-response: proof = SHA256(token + challenge)
    let challenge = uuid::Uuid::new_v4().to_string();
    let health_url = format!("http://127.0.0.1:{}/health?challenge={}", port, challenge);
    match client.get(&health_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if let (Some(iid), Some(proof)) = (
                    body.get("instance_id").and_then(|v| v.as_str()),
                    body.get("proof").and_then(|v| v.as_str()),
                ) {
                    // Check instance_id matches expected
                    if iid != expected_iid {
                        return false;
                    }
                    // Verify: proof = SHA256(local_token + challenge)
                    let mut hasher = Sha256::new();
                    hasher.update(local_token.as_bytes());
                    hasher.update(challenge.as_bytes());
                    let expected = format!("{:x}", hasher.finalize());
                    return proof == expected;
                }
            }
        }
        _ => {}
    }
    false
}

fn verify_bearer_token(headers: &HeaderMap, expected_token: &str) -> bool {
    if expected_token.is_empty() {
        return true; // no token configured — allow (backward-compatible for dev)
    }
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        token == expected_token
    } else {
        false
    }
}

pub async fn start(state: Arc<AppState>) {
    if has_reachable_local_server(A2A_PORT).await {
        println!(
            "[A2A] existing local server already available on port {} - a2a_server.rs:95",
            A2A_PORT
        );
        return;
    }

    let token = load_or_generate_a2a_token();
    let instance_id = load_or_generate_instance_id();

    let server_state = A2AServerState {
        app_state: state,
        bearer_token: token,
        instance_id,
    };

    let app = Router::new()
        .route("/agent.json", get(agent_card_handler))
        .route("/tasks/send", post(send_task))
        .route("/health", get(health_handler))
        .with_state(server_state);

    let bind_addr = format!("127.0.0.1:{}", A2A_PORT);
    match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => {
            let addr = listener.local_addr().unwrap();
            println!(
                "[A2A] HTTP server listening on http://{} - a2a_server.rs:96",
                addr
            );
            tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, app).await {
                    eprintln!("[A2A] Server error: {} - a2a_server.rs:99", e);
                }
            });
        }
        Err(e) => {
            eprintln!("[A2A] Failed to bind server: {} - a2a_server.rs:118", e);
        }
    }
}

async fn agent_card_handler(
    State(state): State<A2AServerState>,
) -> Result<Json<openlife_core::a2a::AgentCard>, axum::http::StatusCode> {
    let model = state
        .app_state
        .life_model_manager
        .lock()
        .await
        .load()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut card = A2AServerHandler::default_agent_card(A2A_PORT, &model);
    // Declare bearer authentication
    card.authentication = Some(openlife_core::a2a::AgentAuthentication {
        schemes: vec!["bearer".to_string()],
        credentials: None,
    });
    // Include instance_id in card for sidecar proof
    card.metadata = Some({
        let mut meta = std::collections::HashMap::new();
        meta.insert(
            "instance_id".to_string(),
            serde_json::Value::String(state.instance_id.clone()),
        );
        meta
    });
    Ok(Json(card))
}

async fn send_task(
    State(state): State<A2AServerState>,
    headers: HeaderMap,
    Json(req): Json<SendTaskRequest>,
) -> Result<Json<SendTaskResponse>, axum::http::StatusCode> {
    if !verify_bearer_token(&headers, &state.bearer_token) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let life_model = state
        .app_state
        .life_model_manager
        .lock()
        .await
        .load()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let privacy_engine = state.app_state.privacy_engine.lock().await.clone();
    let handler = A2AServerHandler {
        life_model,
        privacy_engine,
    };
    let resp = handler.handle_task(req);
    Ok(Json(resp))
}

async fn health_handler(
    State(state): State<A2AServerState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let challenge = params.get("challenge").cloned().unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(state.bearer_token.as_bytes());
    hasher.update(challenge.as_bytes());
    let proof = format!("{:x}", hasher.finalize());
    Ok(Json(serde_json::json!({
        "instance_id": state.instance_id,
        "proof": proof,
        "status": "ok"
    })))
}

fn load_or_generate_a2a_token() -> String {
    let path = crate::storage::app_data_dir().join("a2a_token");
    if path.exists() {
        if let Ok(token) = std::fs::read_to_string(&path) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return token;
            }
        }
    }
    let token = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::write(&path, &token);
    token
}

fn load_or_generate_instance_id() -> String {
    let path = crate::storage::app_data_dir().join("a2a_instance_id");
    if path.exists() {
        if let Ok(id) = std::fs::read_to_string(&path) {
            let id = id.trim().to_string();
            if !id.is_empty() {
                return id;
            }
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::write(&path, &id);
    id
}
