fn main() {
    println!("cargo:rerun-if-env-changed=OPENLIFE_BUILD_PROFILE");
    println!("cargo:rerun-if-env-changed=OPENLIFE_NATIVE_BUILD_NONCE");
    println!("cargo:rerun-if-env-changed=OPENLIFE_BUILD_SOURCE_STATE");
    if let Ok(nonce) = std::env::var("OPENLIFE_NATIVE_BUILD_NONCE") {
        println!("cargo:rustc-env=OPENLIFE_BUILD_NONCE={nonce}");
    }
    if let Some(commit) = git_output(&["rev-parse", "--short=12", "HEAD"]) {
        println!("cargo:rustc-env=OPENLIFE_BUILD_COMMIT={}", commit);
    }
    let source_state = std::env::var("OPENLIFE_BUILD_SOURCE_STATE")
        .ok()
        .filter(|value| matches!(value.as_str(), "clean" | "dirty"))
        .unwrap_or_else(git_source_state);
    println!("cargo:rustc-env=OPENLIFE_BUILD_SOURCE_STATE={source_state}");
    let build_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| format!("unix:{}", duration.as_secs()))
        .unwrap_or_else(|_| "unknown".to_string());
    println!(
        "cargo:rustc-env=OPENLIFE_BUILD_TIMESTAMP={}",
        build_timestamp
    );
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
    tauri_build::build()
}

fn git_source_state() -> String {
    let Ok(output) = std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .output()
    else {
        return "unknown".to_string();
    };
    if !output.status.success() {
        return "unknown".to_string();
    }
    if output.stdout.is_empty() {
        "clean".to_string()
    } else {
        "dirty".to_string()
    }
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
