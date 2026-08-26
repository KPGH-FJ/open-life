use crate::storage;

pub(crate) fn bundle_identifier_for_profile(profile: &str) -> &'static str {
    match storage::normalize_openlife_profile(Some(profile)) {
        "dev" => "ai.openlife.desktop.dev",
        "qa" => "ai.openlife.desktop.qa",
        _ => "ai.openlife.desktop",
    }
}

fn product_name_for_profile(profile: &str) -> &'static str {
    match storage::normalize_openlife_profile(Some(profile)) {
        "dev" => "OpenLife Dev",
        "qa" => "OpenLife QA",
        _ => "OpenLife",
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBuildInfo {
    pub profile: String,
    pub git_sha: String,
    pub source_state: String,
    pub build_time: String,
    pub current_exe: String,
    pub binary_kind: String,
    pub frontend_mode: String,
    pub dev_url: String,
    pub frontend_dist: String,
    pub data_dir: String,
    pub dev_extensions_enabled: bool,
    pub arbitrary_mcp_registration_enabled: bool,
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

pub fn build_source_state() -> String {
    option_env!("OPENLIFE_BUILD_SOURCE_STATE")
        .filter(|value| matches!(*value, "clean" | "dirty"))
        .unwrap_or("unknown")
        .to_string()
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
    let profile = storage::openlife_profile();
    let bundle_identifier = bundle_identifier_for_profile(&profile).to_string();
    let product_name = product_name_for_profile(&profile).to_string();
    let dev_extensions_enabled = cfg!(all(feature = "dev-extensions", debug_assertions));
    let arbitrary_mcp_registration_enabled = dev_extensions_enabled && profile == "dev";

    RuntimeBuildInfo {
        profile,
        git_sha: build_git_sha(),
        source_state: build_source_state(),
        build_time: build_time(),
        current_exe: current_exe_label(),
        binary_kind: current_binary_kind(),
        frontend_mode: frontend_mode(),
        dev_url: dev_url(),
        frontend_dist: frontend_dist(),
        data_dir: storage::app_data_dir().display().to_string(),
        dev_extensions_enabled,
        arbitrary_mcp_registration_enabled,
        bundle_identifier,
        product_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_identifier_tracks_the_exact_compiled_profile() {
        assert_eq!(
            bundle_identifier_for_profile("release"),
            "ai.openlife.desktop"
        );
        assert_eq!(
            bundle_identifier_for_profile("dev"),
            "ai.openlife.desktop.dev"
        );
        assert_eq!(
            bundle_identifier_for_profile("qa"),
            "ai.openlife.desktop.qa"
        );
        assert_eq!(product_name_for_profile("release"), "OpenLife");
        assert_eq!(product_name_for_profile("dev"), "OpenLife Dev");
        assert_eq!(product_name_for_profile("qa"), "OpenLife QA");
        assert!(matches!(
            build_source_state().as_str(),
            "clean" | "dirty" | "unknown"
        ));
    }
}
