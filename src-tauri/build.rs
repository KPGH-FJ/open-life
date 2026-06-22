fn main() {
    if let Some(commit) = git_output(&["rev-parse", "--short=12", "HEAD"]) {
        println!("cargo:rustc-env=OPENLIFE_BUILD_COMMIT={}", commit);
    }
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
