use crate::a2a_server;
use crate::storage;

const BUNDLE_IDENTIFIER: &str = "ai.openlife.app";
const PRODUCT_NAME: &str = "OpenLife";

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBuildInfo {
    pub profile: String,
    pub git_sha: String,
    pub build_time: String,
    pub current_exe: String,
    pub binary_kind: String,
    pub frontend_mode: String,
    pub dev_url: String,
    pub frontend_dist: String,
    pub data_dir: String,
    pub a2a_port: u16,
    pub a2a_status: String,
    pub bundle_identifier: String,
    pub product_name: String,
}

pub fn build_git_sha() -> String {
    std::env::var("OPENLIFE_BUILD_COMMIT")
        .ok()
        .or_else(|| std::env::var("GITHUB_SHA").ok())
        .or_else(|| option_env!("OPENLIFE_BUILD_COMMIT").map(str::to_string))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn build_time() -> String {
    std::env::var("OPENLIFE_BUILD_TIMESTAMP")
        .ok()
        .or_else(|| option_env!("OPENLIFE_BUILD_TIMESTAMP").map(str::to_string))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn current_exe_label() -> String {
    std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

pub fn current_binary_kind() -> String {
    let exe = current_exe_label();
    let normalized = exe.replace('\\', "/");
    if normalized.contains("/target/debug/bundle/")
        || (cfg!(debug_assertions) && normalized.contains(".app/Contents/MacOS/"))
    {
        "debug_bundle".to_string()
    } else if normalized.contains("/target/debug/") {
        "debug_binary".to_string()
    } else if normalized.contains(".app/Contents/MacOS/")
        || normalized.contains("/target/release/")
        || normalized.contains("/target/universal-apple-darwin/release/")
    {
        "release_bundle".to_string()
    } else {
        "unknown".to_string()
    }
}

pub fn frontend_mode() -> String {
    if std::env::var("OPENLIFE_FRONTEND_MODE").ok().as_deref() == Some("dev_server")
        || std::env::var("OPENLIFE_DEV_URL")
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
    {
        "dev_server".to_string()
    } else if current_binary_kind().ends_with("_bundle") {
        "bundled_dist".to_string()
    } else {
        "unknown".to_string()
    }
}

pub fn dev_url() -> String {
    std::env::var("OPENLIFE_DEV_URL").unwrap_or_default()
}

pub fn frontend_dist() -> String {
    std::env::var("OPENLIFE_FRONTEND_DIST")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map(|path| path.join("frontend").join("dist").display().to_string())
                .unwrap_or_else(|_| "unknown".to_string())
        })
}

pub async fn collect_runtime_build_info() -> RuntimeBuildInfo {
    let a2a_port = a2a_server::configured_a2a_port();
    let a2a_status = a2a_server::classify_local_sidecar(a2a_port)
        .await
        .status_label();

    RuntimeBuildInfo {
        profile: storage::openlife_profile(),
        git_sha: build_git_sha(),
        build_time: build_time(),
        current_exe: current_exe_label(),
        binary_kind: current_binary_kind(),
        frontend_mode: frontend_mode(),
        dev_url: dev_url(),
        frontend_dist: frontend_dist(),
        data_dir: storage::app_data_dir().display().to_string(),
        a2a_port,
        a2a_status,
        bundle_identifier: BUNDLE_IDENTIFIER.to_string(),
        product_name: PRODUCT_NAME.to_string(),
    }
}
