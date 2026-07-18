use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use openlife_core::a2a::AgentCard;
use openlife_core::a2a::{A2AServerHandler, SendTaskRequest, SendTaskResponse};
use openlife_core::life_model::LifeModelManager;
use openlife_core::privacy::{PrivacyEngine, PrivacyPolicy};
use std::{path::Path, sync::Arc, time::Duration};

pub const A2A_PORT: u16 = 8765;
pub const A2A_DEV_PORT: u16 = 8766;
const A2A_MAX_REQUEST_BYTES: usize = 512 * 1024;
const A2A_PROTOCOL_VERSION: &str = "0.1";
const A2A_ENABLE_ENV: &str = "OPENLIFE_ENABLE_DEV_A2A";
const A2A_PAIRING_TOKEN_ENV: &str = "OPENLIFE_A2A_PAIRED_TOKEN";
pub const A2A_PARENT_PIPE_GUARD_ENV: &str = "OPENLIFE_A2A_PARENT_PIPE_GUARD";

fn validate_authenticated_dev_a2a_enablement(
    debug_build: bool,
    profile: &str,
    enabled: bool,
    pairing_token: Option<&str>,
    custom_data_dir: bool,
    custom_data_dir_reviewed: bool,
) -> Result<String, String> {
    if !debug_build {
        return Err("A2A development server is forbidden outside debug builds".into());
    }
    if profile != "dev" {
        return Err("A2A development server requires OPENLIFE_PROFILE=dev".into());
    }
    if !enabled {
        return Err(format!(
            "A2A development server requires explicit {A2A_ENABLE_ENV}=1"
        ));
    }
    if custom_data_dir && !custom_data_dir_reviewed {
        return Err(
            "dev A2A refuses OPENLIFE_DATA_DIR without an explicit isolated-data override".into(),
        );
    }
    let token = pairing_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("authenticated A2A requires {A2A_PAIRING_TOKEN_ENV}"))?;
    if !(32..=4096).contains(&token.len()) || token.chars().any(char::is_control) {
        return Err("A2A pairing token must contain 32..=4096 non-control characters".into());
    }
    Ok(token.to_string())
}

pub fn require_authenticated_dev_a2a_opt_in() -> Result<String, String> {
    validate_authenticated_dev_a2a_enablement(
        cfg!(debug_assertions),
        &crate::storage::openlife_profile(),
        std::env::var(A2A_ENABLE_ENV).as_deref() == Ok("1"),
        std::env::var(A2A_PAIRING_TOKEN_ENV).ok().as_deref(),
        std::env::var_os("OPENLIFE_DATA_DIR").is_some(),
        std::env::var("OPENLIFE_ALLOW_DEV_EXTENSIONS_WITH_CUSTOM_DATA_DIR").as_deref() == Ok("1"),
    )
}

pub fn paired_token_for_local_client() -> Result<String, String> {
    require_authenticated_dev_a2a_opt_in()
}

#[derive(Clone)]
pub struct A2AServerRuntimeState {
    life_model_manager: Arc<LifeModelManager>,
    privacy_engine: PrivacyEngine,
    port: u16,
}

#[derive(Clone)]
struct A2AAuthState {
    pairing_token: Arc<str>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct A2APublicHealth {
    pub status: String,
    pub protocol_version: String,
}

impl A2APublicHealth {
    pub fn current() -> Self {
        Self {
            status: "ok".into(),
            protocol_version: A2A_PROTOCOL_VERSION.into(),
        }
    }
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
    AuthenticationMismatch,
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
            LocalSidecarStatus::AuthenticationMismatch => "authentication_mismatch".to_string(),
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
                "existing A2A sidecar profile mismatch: expected {expected}, got {actual}"
            )),
            LocalSidecarStatus::MismatchedBuild { expected, actual } => Some(format!(
                "existing A2A sidecar build mismatch: expected {expected}, got {actual}"
            )),
            LocalSidecarStatus::MismatchedPort { expected, actual } => Some(format!(
                "existing A2A sidecar port mismatch: expected {expected}, got {actual}"
            )),
            LocalSidecarStatus::AuthenticationMismatch => Some(
                "existing A2A sidecar rejected the configured pairing credential".to_string(),
            ),
            LocalSidecarStatus::LegacyOpenLifeNoHealth => Some(
                "existing A2A server has no authenticated OpenLife health endpoint; treating it as stale"
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

fn load_persisted_a2a_runtime_state_from_data_dir(
    port: u16,
    data_dir: &Path,
) -> Result<A2AServerRuntimeState, String> {
    let policy_text =
        std::fs::read_to_string(data_dir.join("privacy_policy.yaml")).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "a2a_persisted_privacy_policy_missing".to_string()
            } else {
                format!("a2a_persisted_privacy_policy_read_failed:{error}")
            }
        })?;
    let privacy_policy = PrivacyPolicy::from_yaml(&policy_text)
        .map_err(|error| format!("a2a_persisted_privacy_policy_parse_failed:{error}"))?;
    let life_model_manager = Arc::new(LifeModelManager::new(
        data_dir.join("life-model").join("current"),
    ));
    life_model_manager
        .load()
        .map_err(|error| format!("a2a_life_model_load_failed:{error}"))?;
    Ok(A2AServerRuntimeState {
        life_model_manager,
        privacy_engine: PrivacyEngine::with_policy(privacy_policy),
        port,
    })
}

pub fn load_persisted_a2a_runtime_state(port: u16) -> Result<A2AServerRuntimeState, String> {
    load_persisted_a2a_runtime_state_from_data_dir(port, &crate::storage::app_data_dir())
}

pub async fn has_reachable_local_server(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/.well-known/agent.json");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(250))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<AgentCard>().await {
            Ok(card) => card.name == "OpenLife" && card.skills.is_empty(),
            Err(_) => false,
        },
        _ => false,
    }
}

enum PrivateHealthProbe {
    NotRunning,
    Unauthorized,
    Current(A2AHealth),
    Unusable,
}

async fn fetch_local_private_health(port: u16, pairing_token: &str) -> PrivateHealthProbe {
    let url = format!("http://127.0.0.1:{port}/private/health");
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(250))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
    {
        Ok(client) => client,
        Err(_) => return PrivateHealthProbe::Unusable,
    };
    let response = match client.get(url).bearer_auth(pairing_token).send().await {
        Ok(response) => response,
        Err(_) => return PrivateHealthProbe::NotRunning,
    };
    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return PrivateHealthProbe::Unauthorized;
    }
    if !response.status().is_success() {
        return PrivateHealthProbe::Unusable;
    }
    match response.json::<A2AHealth>().await {
        Ok(health) => PrivateHealthProbe::Current(health),
        Err(_) => PrivateHealthProbe::Unusable,
    }
}

pub async fn classify_local_sidecar(port: u16, pairing_token: &str) -> LocalSidecarStatus {
    match fetch_local_private_health(port, pairing_token).await {
        PrivateHealthProbe::Current(health) => {
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
            LocalSidecarStatus::Current
        }
        PrivateHealthProbe::Unauthorized => LocalSidecarStatus::AuthenticationMismatch,
        PrivateHealthProbe::Unusable if has_reachable_local_server(port).await => {
            LocalSidecarStatus::LegacyOpenLifeNoHealth
        }
        PrivateHealthProbe::Unusable => LocalSidecarStatus::Unhealthy,
        PrivateHealthProbe::NotRunning if has_reachable_local_server(port).await => {
            LocalSidecarStatus::LegacyOpenLifeNoHealth
        }
        PrivateHealthProbe::NotRunning => LocalSidecarStatus::NotRunning,
    }
}

fn bearer_token_matches(headers: &HeaderMap, expected: &str) -> bool {
    let Some(observed) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    else {
        return false;
    };
    let observed = observed.as_bytes();
    let expected = expected.as_bytes();
    if observed.len() != expected.len() {
        return false;
    }
    observed
        .iter()
        .zip(expected)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

async fn require_paired_bearer(
    State(state): State<A2AAuthState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !bearer_token_matches(request.headers(), &state.pairing_token) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}

pub fn build_a2a_router(runtime_state: A2AServerRuntimeState, pairing_token: String) -> Router {
    let auth_state = A2AAuthState {
        pairing_token: Arc::<str>::from(pairing_token),
    };
    let private = Router::new()
        .route("/agent.json", get(private_agent_card_handler))
        .route("/private/health", get(private_health_handler))
        .route("/tasks/send", post(send_task))
        .route_layer(middleware::from_fn_with_state(
            auth_state,
            require_paired_bearer,
        ));
    Router::new()
        .route("/.well-known/agent.json", get(public_agent_card_handler))
        .route("/health", get(public_health_handler))
        .merge(private)
        .layer(DefaultBodyLimit::max(A2A_MAX_REQUEST_BYTES))
        .with_state(runtime_state)
}

async fn public_agent_card_handler(
    State(state): State<A2AServerRuntimeState>,
) -> Json<openlife_core::a2a::AgentCard> {
    Json(A2AServerHandler::public_agent_card(state.port))
}

async fn private_agent_card_handler(
    State(state): State<A2AServerRuntimeState>,
) -> Result<Json<openlife_core::a2a::AgentCard>, StatusCode> {
    let model = state
        .life_model_manager
        .load()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(A2AServerHandler::default_agent_card(
        state.port, &model,
    )))
}

async fn public_health_handler() -> Json<A2APublicHealth> {
    Json(A2APublicHealth::current())
}

async fn private_health_handler(State(state): State<A2AServerRuntimeState>) -> Json<A2AHealth> {
    Json(A2AHealth::current(state.port))
}

async fn send_task(
    State(state): State<A2AServerRuntimeState>,
    Json(req): Json<SendTaskRequest>,
) -> Result<Json<SendTaskResponse>, StatusCode> {
    openlife_core::a2a::validate_external_task_request(&req)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let life_model = state
        .life_model_manager
        .load()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let handler = A2AServerHandler {
        life_model,
        privacy_engine: state.privacy_engine.clone(),
    };
    Ok(Json(handler.handle_task(req).await))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn protected_probe() -> StatusCode {
        StatusCode::OK
    }

    #[test]
    fn authenticated_dev_enablement_is_explicit_and_requires_strong_pairing() {
        assert!(validate_authenticated_dev_a2a_enablement(
            true,
            "dev",
            false,
            Some("01234567890123456789012345678901"),
            false,
            false,
        )
        .unwrap_err()
        .contains(A2A_ENABLE_ENV));
        assert!(
            validate_authenticated_dev_a2a_enablement(true, "dev", true, None, false, false,)
                .unwrap_err()
                .contains(A2A_PAIRING_TOKEN_ENV)
        );
        assert!(validate_authenticated_dev_a2a_enablement(
            true,
            "dev",
            true,
            Some("01234567890123456789012345678901"),
            false,
            false,
        )
        .is_ok());
    }

    #[tokio::test]
    async fn paired_bearer_middleware_has_real_loopback_success_and_unauthenticated_fail_closed() {
        let pairing_token = "paired-loopback-012345678901234567890123";
        let app = Router::new()
            .route("/protected", get(protected_probe))
            .route_layer(middleware::from_fn_with_state(
                A2AAuthState {
                    pairing_token: Arc::<str>::from(pairing_token),
                },
                require_paired_bearer,
            ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/protected", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = reqwest::Client::new();

        let unauthenticated = client.get(&url).send().await.unwrap();
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
        let authenticated = client
            .get(&url)
            .bearer_auth(pairing_token)
            .send()
            .await
            .unwrap();
        assert_eq!(authenticated.status(), StatusCode::OK);

        server.abort();
    }

    #[test]
    fn public_health_and_card_are_minimal() {
        let health = serde_json::to_value(A2APublicHealth::current()).unwrap();
        assert_eq!(health["status"], "ok");
        assert!(health.get("pid").is_none());
        assert!(health.get("profile").is_none());
        assert!(health.get("gitSha").is_none());
        assert!(health.get("binaryPath").is_none());

        let card = A2AServerHandler::public_agent_card(8766);
        assert!(card.skills.is_empty());
        assert!(card.authentication.is_some());
        let serialized = serde_json::to_string(&card).unwrap();
        assert!(!serialized.contains("values"));
        assert!(!serialized.contains("goals"));
        assert!(!serialized.contains("skills:"));
    }

    #[test]
    fn a2a_runtime_state_requires_and_uses_the_persisted_privacy_policy() {
        let data_dir = tempfile::tempdir().unwrap();
        let missing = load_persisted_a2a_runtime_state_from_data_dir(8766, data_dir.path())
            .err()
            .expect("missing persisted privacy policy must fail closed");
        assert!(missing.contains("a2a_persisted_privacy_policy_missing"));

        std::fs::write(
            data_dir.path().join("privacy_policy.yaml"),
            "enabled: [broken",
        )
        .unwrap();
        let malformed = load_persisted_a2a_runtime_state_from_data_dir(8766, data_dir.path())
            .err()
            .expect("malformed persisted privacy policy must fail closed");
        assert!(malformed.contains("a2a_persisted_privacy_policy_parse_failed"));

        let persisted = PrivacyPolicy {
            enabled: false,
            ..Default::default()
        };
        std::fs::write(
            data_dir.path().join("privacy_policy.yaml"),
            persisted.to_yaml().unwrap(),
        )
        .unwrap();
        let runtime = load_persisted_a2a_runtime_state_from_data_dir(8766, data_dir.path())
            .expect("valid persisted privacy policy runtime state");
        assert!(!runtime.privacy_engine.policy().enabled);
        assert_eq!(runtime.port, 8766);
    }
}
