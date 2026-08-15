use crate::main_chat_context_loader::{
    load_workspace_knowledge_context_candidates, sanitize_main_chat_selected_skill_id,
};
use crate::main_chat_tool_selection::main_chat_workspace_file_target;

#[test]
fn main_chat_context_loader_tests_are_not_concentrated_in_lib_rs() {
    let lib_rs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");

    for forbidden in [
        "main_chat_context_loader_declares_controlled_knowledge_format_surfaces",
        "workspace_file_resolver_accepts_explicit_nested_workspace_relative_path",
        "workspace_file_resolver_blocks_explicit_path_traversal",
        "main_chat_knowledge_context_loader_loads_bounded_workspace_formats",
        "main_chat_knowledge_context_loader_does_not_load_unselected_skill_instruction",
        "main_chat_context_compiler_is_extracted_to_context_loader",
        "main_chat_selected_skill_id_is_sanitized_before_context_compilation",
        "ordinary_chat_commands_plumb_selected_skill_id_to_context_loader",
    ] {
        assert!(
            !source.contains(&format!("\n    fn {forbidden}(")),
            "Main Chat context-loader test {forbidden} should live outside src/lib.rs"
        );
    }
    assert!(
        !source.contains("\n    fn create_main_chat_knowledge_workspace("),
        "Main Chat context-loader test helper should live outside src/lib.rs"
    );
}

#[test]
fn main_chat_context_loader_declares_controlled_knowledge_format_surfaces() {
    let module_path = format!(
        "{}/src/main_chat_context_loader.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(module_path)
        .expect("Main Chat knowledge context loader module should exist");

    for required_surface in [
        "AGENTS.md",
        "SOUL.md",
        "USER.md",
        "memories/USER.md",
        "skills/<selected>/SKILL.md",
    ] {
        assert!(
            source.contains(required_surface),
            "knowledge context loader must declare bounded surface {required_surface}"
        );
    }
    assert!(source.contains("ContextSourceKind::SkillInstruction"));
    assert!(source.contains("selected_skill_id"));
    assert!(source.contains("validate_selected_skill_id"));
}

#[test]
fn workspace_file_resolver_accepts_explicit_nested_workspace_relative_path() {
    let (label, path) = main_chat_workspace_file_target("Read plans/README.md").unwrap();

    assert_eq!(label, "plans/README.md");
    assert!(path.ends_with("plans/README.md"));
}

#[test]
fn workspace_file_resolver_blocks_explicit_path_traversal() {
    let error = main_chat_workspace_file_target("Read ../Cargo.toml").unwrap_err();

    assert!(error.contains("outside workspace") || error.contains("path traversal"));
}

fn create_main_chat_knowledge_workspace() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("create knowledge workspace");
    std::fs::create_dir_all(root.path().join("plans")).expect("create plans dir");
    std::fs::create_dir_all(root.path().join("memories")).expect("create memories dir");
    std::fs::create_dir_all(root.path().join("skills/summarize"))
        .expect("create selected skill dir");
    std::fs::create_dir_all(root.path().join("skills/other")).expect("create other skill dir");
    std::fs::write(
        root.path().join("AGENTS.md"),
        format!("workspace instruction\n{}", "A".repeat(1600)),
    )
    .expect("write AGENTS.md");
    std::fs::write(root.path().join("SOUL.md"), "bounded soul context").expect("write SOUL.md");
    std::fs::write(root.path().join("memories/USER.md"), "bounded user context")
        .expect("write USER.md");
    std::fs::write(
        root.path().join("memories/MEMORY.md"),
        "bounded memory context",
    )
    .expect("write MEMORY.md");
    std::fs::write(
        root.path().join("skills/summarize/SKILL.md"),
        "selected summarize skill instructions",
    )
    .expect("write selected SKILL.md");
    std::fs::write(
        root.path().join("skills/other/SKILL.md"),
        "unselected skill instructions must not load",
    )
    .expect("write other SKILL.md");
    root
}

#[test]
fn main_chat_knowledge_context_loader_loads_bounded_workspace_formats() {
    let root = create_main_chat_knowledge_workspace();
    let candidates = load_workspace_knowledge_context_candidates(
        root.path(),
        Some("summarize"),
        "use bounded memory context",
    );
    let source_ids = candidates
        .iter()
        .map(|candidate| candidate.source_id.as_str())
        .collect::<Vec<_>>();

    assert!(source_ids.contains(&"AGENTS.md"));
    assert!(source_ids.contains(&"SOUL.md"));
    assert!(source_ids.contains(&"memories/USER.md"));
    assert!(!source_ids.contains(&"memories/MEMORY.md"));
    assert!(source_ids.contains(&"skills/summarize/SKILL.md"));
    assert!(!source_ids.contains(&"skills/other/SKILL.md"));
    assert!(candidates
        .iter()
        .all(|candidate| candidate.content.chars().count() <= 1200));
    let selected_skill = candidates
        .iter()
        .find(|candidate| candidate.source_id == "skills/summarize/SKILL.md")
        .expect("selected skill candidate");
    assert_eq!(
        selected_skill.source_kind,
        openlife_core::agent::main_chat_agent_v1::ContextSourceKind::SkillInstruction
    );
    assert_eq!(
        selected_skill.selected_skill_id.as_deref(),
        Some("summarize")
    );
}

#[test]
fn main_chat_knowledge_context_loader_does_not_load_unselected_skill_instruction() {
    let root = create_main_chat_knowledge_workspace();
    let candidates =
        load_workspace_knowledge_context_candidates(root.path(), None, "ordinary task");

    assert!(!candidates.iter().any(|candidate| {
        candidate.source_kind
            == openlife_core::agent::main_chat_agent_v1::ContextSourceKind::SkillInstruction
    }));
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
