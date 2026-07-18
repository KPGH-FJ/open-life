use std::path::{Component, Path, PathBuf};

pub(crate) fn resolve_main_chat_workspace_file_target(
    user_text: &str,
) -> Result<(String, String), String> {
    let workspace = resolve_workspace_root()?;
    let relative = select_workspace_file_relative_path(user_text);
    let safe_relative = validate_relative_workspace_path(&relative)?;
    let candidate = workspace.join(&safe_relative);
    if !candidate.starts_with(&workspace) {
        return Err("file read outside workspace is blocked".into());
    }

    let label = safe_relative.to_string_lossy().replace('\\', "/");
    // This boundary performs lexical policy validation only. Whether the
    // candidate exists, resolves through a symlink, is readable, or exceeds a
    // size bound is operating-system execution truth and belongs to the
    // ToolGateway filesystem adapter.
    Ok((label, candidate.to_string_lossy().to_string()))
}

pub(crate) fn resolve_workspace_root() -> Result<PathBuf, String> {
    let cwd =
        std::env::current_dir().map_err(|err| format!("workspace path unavailable: {err}"))?;
    for candidate in cwd.ancestors() {
        if candidate.join("AGENTS.md").is_file() && candidate.join("plans").is_dir() {
            return candidate
                .canonicalize()
                .map_err(|err| format!("workspace canonicalization failed: {err}"));
        }
    }
    cwd.canonicalize()
        .map_err(|err| format!("workspace canonicalization failed: {err}"))
}

fn select_workspace_file_relative_path(user_text: &str) -> String {
    if let Some(explicit) = extract_explicit_relative_path(user_text) {
        return explicit;
    }

    let lower = user_text.to_ascii_lowercase();
    if lower.contains("cargo.toml") {
        "Cargo.toml".into()
    } else if lower.contains("main_chat_agent_migration_v1_goal_spec") {
        "plans/main_chat_agent_migration_v1_goal_spec.md".into()
    } else if lower.contains("readme") {
        "README.md".into()
    } else {
        "AGENTS.md".into()
    }
}

fn extract_explicit_relative_path(user_text: &str) -> Option<String> {
    user_text
        .split_whitespace()
        .map(trim_path_token)
        .find(|token| looks_like_workspace_path(token))
        .map(str::to_string)
}

fn trim_path_token(token: &str) -> &str {
    let trimmed = token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '`' | ',' | ':' | ';' | ')' | '(' | '[' | ']' | '{' | '}'
        )
    });
    trimmed.strip_suffix('.').unwrap_or(trimmed)
}

fn looks_like_workspace_path(token: &str) -> bool {
    if token.is_empty() || token.starts_with("http://") || token.starts_with("https://") {
        return false;
    }
    token.contains('/')
        || token.starts_with("./")
        || token.starts_with("../")
        || token.ends_with(".md")
        || token.ends_with(".toml")
        || token.ends_with(".txt")
        || token.ends_with(".json")
        || token.ends_with(".yaml")
        || token.ends_with(".yml")
        || token.ends_with(".rs")
}

fn validate_relative_workspace_path(relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path.is_absolute() {
        return Err("absolute file read paths are blocked; use a workspace-relative path".into());
    }

    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            Component::ParentDir => return Err("file read path traversal is blocked".into()),
            Component::RootDir | Component::Prefix(_) => {
                return Err("file read outside workspace is blocked".into())
            }
        }
    }

    if safe.as_os_str().is_empty() {
        return Err("workspace file path is empty".into());
    }

    Ok(safe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_explicit_nested_path_before_keyword_fallback() {
        assert_eq!(
            select_workspace_file_relative_path("Read plans/README.md"),
            "plans/README.md"
        );
    }

    #[test]
    fn rejects_parent_dir_traversal() {
        let error = validate_relative_workspace_path("../Cargo.toml").unwrap_err();

        assert!(error.contains("path traversal"));
    }

    #[test]
    fn missing_workspace_candidate_is_resolved_without_pre_gateway_io() {
        let (label, path) = resolve_main_chat_workspace_file_target(
            "Read frontend/definitely-missing-tool-gateway-owner.md",
        )
        .expect("lexically safe missing file must reach ToolGateway");

        assert_eq!(label, "frontend/definitely-missing-tool-gateway-owner.md");
        assert!(path.ends_with("frontend/definitely-missing-tool-gateway-owner.md"));
        assert!(!std::path::Path::new(&path).exists());
    }

    #[test]
    fn resolves_project_root_from_crate_test_cwd() {
        let root = resolve_workspace_root().unwrap();

        assert!(root.join("AGENTS.md").is_file());
        assert!(root.join("plans").is_dir());
    }
}
