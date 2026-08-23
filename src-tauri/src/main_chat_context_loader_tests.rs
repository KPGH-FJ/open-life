use crate::main_chat_context_loader::sanitize_main_chat_selected_skill_id;
use crate::workspace_file_resolver::resolve_main_chat_workspace_file_target;

#[test]
fn main_chat_context_loader_tests_are_not_concentrated_in_lib_rs() {
    let lib_rs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");

    for forbidden in [
        "workspace_file_resolver_accepts_explicit_nested_workspace_relative_path",
        "workspace_file_resolver_blocks_explicit_path_traversal",
        "main_chat_selected_skill_id_is_sanitized_before_context_compilation",
        "ordinary_chat_commands_plumb_selected_skill_id_to_context_loader",
    ] {
        assert!(
            !source.contains(&format!("\n    fn {forbidden}(")),
            "Main Chat context-loader test {forbidden} should live outside src/lib.rs"
        );
    }
}

#[test]
fn workspace_file_resolver_accepts_explicit_nested_workspace_relative_path() {
    let (label, path) = resolve_main_chat_workspace_file_target("Read plans/README.md").unwrap();

    assert_eq!(label, "plans/README.md");
    assert!(path.ends_with("plans/README.md"));
}

#[test]
fn workspace_file_resolver_blocks_explicit_path_traversal() {
    let error = resolve_main_chat_workspace_file_target("Read ../Cargo.toml").unwrap_err();

    assert!(error.contains("outside workspace") || error.contains("path traversal"));
}

#[test]
fn main_chat_selected_skill_id_is_sanitized_before_context_compilation() {
    assert_eq!(
        sanitize_main_chat_selected_skill_id(Some(" summarize ")).as_deref(),
        Some("summarize")
    );
    assert_eq!(
        sanitize_main_chat_selected_skill_id(Some("planner.v1_beta-2")).as_deref(),
        Some("planner.v1_beta-2")
    );
    assert!(sanitize_main_chat_selected_skill_id(Some("../summarize")).is_none());
    assert!(sanitize_main_chat_selected_skill_id(Some("skills/summarize")).is_none());
    assert!(sanitize_main_chat_selected_skill_id(Some("bad skill")).is_none());
    assert!(sanitize_main_chat_selected_skill_id(None).is_none());
}

#[test]
fn ordinary_chat_commands_plumb_selected_skill_id_to_context_loader() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let send_body = extract_rust_function_body(&source, "async fn send_message(");
    let stream_body =
        extract_rust_function_body(&source, "async fn start_stream_message<R: tauri::Runtime>(");

    assert!(
        source.contains("selected_skill_id: Option<String>"),
        "ordinary chat command inputs must expose optional selected skill id"
    );
    assert!(
        send_body.contains("selected_skill_id.as_deref()"),
        "send_message must pass selected skill id into Main Chat context assembly"
    );
    assert!(
        stream_body.contains("selected_skill_id.as_deref()"),
        "start_stream_message must pass selected skill id into Main Chat context assembly"
    );
    assert!(
        source.contains("selected_skill_id: Option<String>,"),
        "stream args must carry optional selected skill id for args payloads"
    );
}

fn extract_rust_function_body(source: &str, signature: &str) -> String {
    let signature_start = source.find(signature).expect("function signature exists");
    let brace_start = source[signature_start..]
        .find('{')
        .map(|index| signature_start + index)
        .expect("function body starts");
    let mut depth = 0usize;

    for (offset, ch) in source[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = brace_start + offset + ch.len_utf8();
                    return source[brace_start..end].to_string();
                }
            }
            _ => {}
        }
    }

    panic!("function body closes");
}
