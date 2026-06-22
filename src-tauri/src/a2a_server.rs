use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use openlife_core::a2a::AgentCard;
use openlife_core::a2a::{A2AServerHandler, SendTaskRequest, SendTaskResponse};
use std::{sync::Arc, time::Duration};

use crate::AppState;

pub const A2A_PORT: u16 = 8765;
pub const A2A_DEV_PORT: u16 = 8766;

#[derive(Clone)]
struct A2AHttpState {
    app_state: Arc<AppState>,
    port: u16,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct A2AHealth {
    pub pid: u32,
    pub profile: String,
    pub version: String,
    pub git_sha: String,
    pub build_time: String,
    pub binary_path: String,
    pub port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalSidecarStatus {
    NotRunning,
    Current,
    LegacyOpenLifeNoHealth,
    MismatchedProfile { expected: String, actual: String },
    MismatchedBuild { expected: String, actual: String },
    MismatchedPort { expected: u16, actual: u16 },
    Unhealthy,
}

impl LocalSidecarStatus {
    pub fn status_label(&self) -> String {
        match self {
            LocalSidecarStatus::NotRunning => "not_running".to_string(),
            LocalSidecarStatus::Current => "current".to_string(),
            LocalSidecarStatus::LegacyOpenLifeNoHealth => "legacy_openlife_no_health".to_string(),
            LocalSidecarStatus::MismatchedProfile { .. } => "mismatched_profile".to_string(),
            LocalSidecarStatus::MismatchedBuild { .. } => "mismatched_build".to_string(),
            LocalSidecarStatus::MismatchedPort { .. } => "mismatched_port".to_string(),
            LocalSidecarStatus::Unhealthy => "unhealthy".to_string(),
        }
    }

    pub fn mismatch_detail(&self) -> Option<String> {
        match self {
            LocalSidecarStatus::MismatchedProfile { expected, actual } => Some(format!(
                "existing A2A sidecar profile mismatch: expected {}, got {}",
                expected, actual
            )),
            LocalSidecarStatus::MismatchedBuild { expected, actual } => Some(format!(
                "existing A2A sidecar build mismatch: expected {}, got {}",
                expected, actual
            )),
            LocalSidecarStatus::MismatchedPort { expected, actual } => Some(format!(
                "existing A2A sidecar port mismatch: expected {}, got {}",
                expected, actual
            )),
            LocalSidecarStatus::LegacyOpenLifeNoHealth => Some(
                "existing A2A server has no OpenLife health endpoint; treating it as stale"
                    .to_string(),
            ),
            LocalSidecarStatus::Unhealthy => {
                Some("existing A2A sidecar health response is not usable".to_string())
            }
            _ => None,
        }
    }
}

impl A2AHealth {
    pub fn current(port: u16) -> Self {
        Self {
            pid: std::process::id(),
            profile: crate::storage::openlife_profile(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            git_sha: crate::runtime_build_info::build_git_sha(),
            build_time: crate::runtime_build_info::build_time(),
            binary_path: crate::runtime_build_info::current_exe_label(),
            port,
        }
    }
}

pub fn configured_a2a_port() -> u16 {
    std::env::var("A2A_PORT")
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .unwrap_or_else(|| {
            if crate::storage::openlife_profile() == "dev" {
                A2A_DEV_PORT
            } else {
                A2A_PORT
            }
        })
}

pub async fn has_reachable_local_server(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/agent.json", port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(250))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<AgentCard>().await {
            Ok(card) => card.name == "OpenLife",
            Err(_) => false,
        },
        _ => false,
    }
}

pub async fn fetch_local_health(port: u16) -> Option<A2AHealth> {
    let url = format!("http://127.0.0.1:{}/health", port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(250))
        .build()
        .ok()?;
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json::<A2AHealth>().await.ok()
}

pub async fn classify_local_sidecar(port: u16) -> LocalSidecarStatus {
    if let Some(health) = fetch_local_health(port).await {
        let expected_profile = crate::storage::openlife_profile();
        if health.profile != expected_profile {
            return LocalSidecarStatus::MismatchedProfile {
                expected: expected_profile,
                actual: health.profile,
            };
        }
        if health.port != port {
            return LocalSidecarStatus::MismatchedPort {
                expected: port,
                actual: health.port,
            };
        }
        let expected_build = crate::runtime_build_info::build_git_sha();
        if expected_build != "unknown"
            && health.git_sha != "unknown"
            && health.git_sha != expected_build
        {
            return LocalSidecarStatus::MismatchedBuild {
                expected: expected_build,
                actual: health.git_sha,
            };
        }
        return LocalSidecarStatus::Current;
    }

    if has_reachable_local_server(port).await {
        LocalSidecarStatus::LegacyOpenLifeNoHealth
    } else {
        LocalSidecarStatus::NotRunning
    }
}

pub async fn start(state: Arc<AppState>) {
    let port = configured_a2a_port();
    match classify_local_sidecar(port).await {
        LocalSidecarStatus::Current => {
            println!(
                "[A2A] existing local sidecar is current on port {} - a2a_server.rs:125",
                port
            );
            return;
        }
        LocalSidecarStatus::NotRunning => {}
        status => {
            eprintln!(
                "[A2A] refusing to reuse sidecar on port {}: {} - a2a_server.rs:134",
                port,
                status
                    .mismatch_detail()
                    .unwrap_or_else(|| status.status_label())
            );
            return;
        }
    }

    let http_state = A2AHttpState {
        app_state: state,
        port,
    };
    let app = Router::new()
        .route("/agent.json", get(agent_card_handler))
        .route("/health", get(health_handler))
        .route("/tasks/send", post(send_task))
        .with_state(http_state);

    let bind_addr = format!("127.0.0.1:{}", port);
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
            eprintln!("[A2A] Failed to bind server: {} - a2a_server.rs:45", e);
        }
    }
}

async fn agent_card_handler(
    State(state): State<A2AHttpState>,
) -> Result<Json<openlife_core::a2a::AgentCard>, axum::http::StatusCode> {
    let model = state
        .app_state
        .life_model_manager
        .lock()
        .await
        .load()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let card = A2AServerHandler::default_agent_card(state.port, &model);
    Ok(Json(card))
}

async fn health_handler(State(state): State<A2AHttpState>) -> Json<A2AHealth> {
    Json(A2AHealth::current(state.port))
}

async fn send_task(
    State(state): State<A2AHttpState>,
    Json(req): Json<SendTaskRequest>,
) -> Result<Json<SendTaskResponse>, axum::http::StatusCode> {
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
