use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use hmac::{Hmac, Mac};
use openlife_core::a2a::{A2AServerHandler, SendTaskRequest, SendTaskResponse};
use openlife_core::life_model::LifeModelManager;
use openlife_core::privacy::PrivacyEngine;
use sha2::Sha256;
use std::sync::Arc;

struct AppState {
    life_model_manager: LifeModelManager,
    privacy_engine: PrivacyEngine,
    bearer_token: String,
    instance_id: String,
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("A2A_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8765);

    let bearer_token = std::env::var("A2A_BEARER_TOKEN").unwrap_or_default();
    if bearer_token.trim().is_empty() {
        eprintln!(
            "FATAL: A2A_BEARER_TOKEN is not set or is empty. \
             Standalone A2A server requires a bearer token for authentication. \
             Set the environment variable and restart."
        );
        std::process::exit(1);
    }
    let instance_id =
        std::env::var("A2A_INSTANCE_ID").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());

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
        bearer_token,
        instance_id,
    });

    let card = A2AServerHandler::default_agent_card(port, &life_model);
    let card = {
        let mut c = card;
        c.authentication = Some(openlife_core::a2a::AgentAuthentication {
            schemes: vec!["bearer".to_string()],
            credentials: None,
        });
        c
    };
    let app = Router::new()
        .route(
            "/agent.json",
            get(move || async move { Json(card.clone()) }),
        )
        .route("/tasks/send", post(send_task))
        .route("/health", get(health_handler))
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

fn verify_bearer_token_standalone(headers: &HeaderMap, expected: &str) -> bool {
    if expected.is_empty() {
        return false;
    }
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        token == expected
    } else {
        false
    }
}

async fn send_task(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<SendTaskRequest>,
) -> Result<Json<SendTaskResponse>, axum::http::StatusCode> {
    if !verify_bearer_token_standalone(&headers, &state.bearer_token) {
        return Err(axum::http::StatusCode::UNAUTHORIZED);
    }

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

async fn health_handler(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let challenge = params.get("challenge").cloned().unwrap_or_default();
    let mut mac = Hmac::<Sha256>::new_from_slice(state.bearer_token.as_bytes()).expect("HMAC key");
    mac.update(challenge.as_bytes());
    let proof = hex::encode(mac.finalize().into_bytes());
    Ok(Json(serde_json::json!({
        "instance_id": state.instance_id,
        "proof": proof,
        "status": "ok"
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn test_empty_expected_token_returns_false() {
        let headers = HeaderMap::new();
        assert!(!verify_bearer_token_standalone(&headers, ""));
    }

    #[test]
    fn test_valid_token_passes() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer my-secret-token".parse().unwrap(),
        );
        assert!(verify_bearer_token_standalone(&headers, "my-secret-token"));
    }

    #[test]
    fn test_invalid_token_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer wrong-token".parse().unwrap(),
        );
        assert!(!verify_bearer_token_standalone(&headers, "my-secret-token"));
    }

    #[test]
    fn test_missing_auth_header_rejected() {
        let headers = HeaderMap::new();
        assert!(!verify_bearer_token_standalone(&headers, "my-secret-token"));
    }

    #[test]
    fn test_empty_bearer_token_in_header_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer ".parse().unwrap(),
        );
        // Empty token in header with non-empty expected token → rejected
        assert!(!verify_bearer_token_standalone(&headers, "my-secret-token"));
    }
}
