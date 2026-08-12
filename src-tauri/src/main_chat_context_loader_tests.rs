use crate::main_chat_context_loader::{
    load_markdown_memory_context_candidates_from_roots,
    load_workspace_knowledge_context_candidates, sanitize_main_chat_selected_skill_id,
};
use crate::main_chat_react_tool_selection::main_chat_workspace_file_target;

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
        "configured_markdown_memory_roots",
        "load_markdown_memory_files",
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
fn main_chat_markdown_memory_requires_an_explicit_scope_root() {
    let root = create_main_chat_knowledge_workspace();
    let generic = load_workspace_knowledge_context_candidates(
        root.path(),
        None,
        "use bounded memory context",
    );
    assert!(!generic
        .iter()
        .any(|candidate| candidate.source_id.contains("MEMORY.md")));

    let roots =
        crate::markdown_memory::configured_markdown_memory_roots(root.path().to_str(), None);
    let scoped =
        load_markdown_memory_context_candidates_from_roots(&roots, "use bounded memory context");
    assert!(scoped
        .iter()
        .any(|candidate| candidate.source_id == "markdown-memory:workspace:memories/MEMORY.md"));
}

#[test]
fn project_markdown_memory_precedes_broader_workspace_memory_for_a_project_task() {
    let workspace = tempfile::tempdir().expect("workspace memory root");
    let project = tempfile::tempdir().expect("project memory root");
    std::fs::write(
        workspace.path().join("MEMORY.md"),
        "# Workspace A Memory\n- 作用域标记：OL-WS-A-731。\n- 仅在 Workspace A 的任务中使用。",
    )
    .expect("write workspace memory");
    std::fs::write(
        project.path().join("MEMORY.md"),
        "# Project A Memory\n- 作用域标记：OL-PROJ-A-482。\n- 仅在 Project A 的发布复核中使用。",
    )
    .expect("write project memory");

    let roots = crate::markdown_memory::configured_markdown_memory_roots(
        workspace.path().to_str(),
        project.path().to_str(),
    );
    let candidates = load_markdown_memory_context_candidates_from_roots(
        &roots,
        "我正在进行 Project A 的发布复核，请引用当前工作记忆中的作用域标记。",
    );

    assert_eq!(
        candidates.first().map(|candidate| candidate.source_id.as_str()),
        Some("markdown-memory:project:MEMORY.md"),
        "the narrower, more relevant project memory must reach the model before broader workspace memory"
    );
    assert!(candidates[0].content.contains("OL-PROJ-A-482"));
    assert!(
        !candidates[0].content.contains("Scope precedence"),
        "scope and selection metadata must remain in the control layer rather than becoming factual evidence"
    );
    assert!(candidates[0]
        .inclusion_reason
        .contains("project Markdown working memory"));
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
fn main_chat_context_compiler_is_extracted_to_context_loader() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_context_loader.rs");
    let module_source =
        std::fs::read_to_string(&module_path).expect("read src/main_chat_context_loader.rs");
    let compile_body = extract_rust_function_body(
        &module_source,
        "pub(crate) async fn compile_main_chat_context(",
    );

    assert!(
        compile_body.contains("load_current_workspace_knowledge_context_candidates"),
        "Main Chat context assembly must use the controlled knowledge-format loader"
    );
    assert!(
        !compile_body.contains("std::fs::read_to_string(&path)"),
        "Main Chat context assembly must not regress to ad hoc AGENTS.md-only reads"
    );
    assert!(
        module_source.contains("selected_skill_id: Option<&str>"),
        "Main Chat context compiler must accept a selected skill id from ordinary chat surfaces"
    );
    assert!(
        !compile_body.contains("let selected_skill_id: Option<String> = None;"),
        "Main Chat context compiler must not discard selected skill ids"
    );
    assert!(
        module_source.contains("pub(crate) fn sanitize_main_chat_selected_skill_id("),
        "selected skill id sanitization should live with Main Chat context loading"
    );
    assert!(
        !source.contains("\nasync fn compile_main_chat_context("),
        "Main Chat context compiler should not remain in lib.rs"
    );
    assert!(
        !source.contains("\nfn sanitize_main_chat_selected_skill_id("),
        "selected skill id sanitizer should not remain in lib.rs"
    );
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
