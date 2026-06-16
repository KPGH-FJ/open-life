use openlife_core::agent::main_chat_agent_v1::{
    ContextCompiler, ContextCompilerInput, ContextSourceCandidate, ContextSourceKind,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::AppState;

const MAX_CONTEXT_CHARS_PER_FILE: usize = 1200;
const CONFIGURED_KNOWLEDGE_ROOT_ENV: &str = "OPENLIFE_KNOWLEDGE_ROOT";

#[allow(dead_code)]
pub(crate) async fn compile_main_chat_context(
    state: &Arc<AppState>,
    decision: &openlife_core::agent::main_chat_agent_v1::AgentIngressDecision,
    task_session_id: &str,
    user_text: &str,
    selected_skill_id: Option<&str>,
) -> openlife_core::agent::main_chat_agent_v1::CompiledContext {
    let mut candidates = vec![
        ContextSourceCandidate::new(
            ContextSourceKind::StableCore,
            "openlife.main_chat_agent_v1",
            "OpenLife Main Chat uses AgentIngress, strategy routing, policy, action queue, proposal blockers, and traceable fallback.",
            "stable core behavior",
            "public",
            24,
        ),
        ContextSourceCandidate::new(
            ContextSourceKind::RuntimePolicy,
            "policy.main_chat_agent_v1",
            "No silent durable LifeModel, Memory, file, calendar, email, external, provider, plugin, or dangerous writes.",
            "runtime policy overlay",
            "internal",
            20,
        ),
        ContextSourceCandidate::new(
            ContextSourceKind::StrategyContract,
            decision.selected_strategy.as_str(),
            format!("Selected strategy: {}", decision.selected_strategy.as_str()),
            "strategy contract",
            "internal",
            8,
        ),
        ContextSourceCandidate::new(
            ContextSourceKind::SessionState,
            task_session_id,
            format!(
                "Active Main Chat task session for request {}",
                decision.request_id
            ),
            "active task session",
            "internal",
            10,
        ),
        ContextSourceCandidate::new(
            ContextSourceKind::Observation,
            "turn.user_request_shape",
            format!(
                "User request present: {}; chars: {}",
                !user_text.trim().is_empty(),
                user_text.chars().count()
            ),
            "ephemeral turn context",
            "internal",
            8,
        ),
    ];

    let selected_skill_id = sanitize_main_chat_selected_skill_id(selected_skill_id);
    candidates.extend(load_current_workspace_knowledge_context_candidates(
        selected_skill_id.as_deref(),
    ));
    if let Ok(sessions) = {
        let store = state.memory_store.lock().await;
        store.list_sessions(5)
    } {
        candidates.push(ContextSourceCandidate::new(
            ContextSourceKind::SelectedPersonalContext,
            "chat_sessions.recent",
            format!(
                "Recent session count available for search: {}",
                sessions.len()
            ),
            "bounded session search metadata",
            "internal",
            8,
        ));
    }

    ContextCompiler.compile(ContextCompilerInput {
        strategy: decision.selected_strategy,
        privacy_risk: decision.privacy_risk.clone(),
        active_session_id: Some(task_session_id.to_string()),
        token_budget: 160,
        selected_skill_id,
        candidates,
    })
}

pub(crate) fn sanitize_main_chat_selected_skill_id(
    selected_skill_id: Option<&str>,
) -> Option<String> {
    let trimmed = selected_skill_id?.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || Path::new(trimmed).is_absolute()
    {
        return None;
    }
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

// Bounded knowledge-format surfaces: AGENTS.md, SOUL.md, USER.md, MEMORY.md,
// memories/USER.md, memories/MEMORY.md, skills/<selected>/SKILL.md.
pub(crate) fn load_current_workspace_knowledge_context_candidates(
    selected_skill_id: Option<&str>,
) -> Vec<ContextSourceCandidate> {
    let mut roots = Vec::new();
    if let Ok(workspace) = crate::workspace_file_resolver::resolve_workspace_root() {
        roots.push(KnowledgeContextRoot::new("workspace", workspace));
    }
    if let Some(configured) = configured_knowledge_root() {
        roots.push(KnowledgeContextRoot::new("configured", configured));
    }

    load_knowledge_context_candidates_from_roots(&roots, selected_skill_id)
}

#[cfg(test)]
pub(crate) fn load_workspace_knowledge_context_candidates(
    root: &Path,
    selected_skill_id: Option<&str>,
) -> Vec<ContextSourceCandidate> {
    let roots = vec![KnowledgeContextRoot::new("workspace", root)];
    load_knowledge_context_candidates_from_roots(&roots, selected_skill_id)
}

fn configured_knowledge_root() -> Option<PathBuf> {
    let configured = std::env::var(CONFIGURED_KNOWLEDGE_ROOT_ENV).ok()?;
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if path.is_dir() {
        path.canonicalize().ok().or(Some(path))
    } else {
        None
    }
}

#[derive(Debug, Clone)]
struct KnowledgeContextRoot {
    label: String,
    path: PathBuf,
}

impl KnowledgeContextRoot {
    fn new(label: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            label: label.into(),
            path: path.into(),
        }
    }
}

fn load_knowledge_context_candidates_from_roots(
    roots: &[KnowledgeContextRoot],
    selected_skill_id: Option<&str>,
) -> Vec<ContextSourceCandidate> {
    let selected_skill_id = selected_skill_id.and_then(validate_selected_skill_id);
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for root in roots {
        push_known_file(
            &mut candidates,
            &mut seen,
            root,
            ContextSourceKind::WorkspaceInstruction,
            "AGENTS.md",
            "workspace instruction scoped to current task",
            "internal",
            28,
        );

        if let Some(skill_id) = selected_skill_id.as_deref() {
            let skill_relative = format!("skills/{skill_id}/SKILL.md");
            push_selected_skill_file(&mut candidates, &mut seen, root, &skill_relative, skill_id);
        }

        push_known_file(
            &mut candidates,
            &mut seen,
            root,
            ContextSourceKind::MaterializedFile,
            "SOUL.md",
            "bounded materialized identity context; not canonical truth",
            "private",
            12,
        );
        push_known_file(
            &mut candidates,
            &mut seen,
            root,
            ContextSourceKind::SelectedPersonalContext,
            "USER.md",
            "bounded user context surface; not canonical truth",
            "private",
            8,
        );
        push_known_file(
            &mut candidates,
            &mut seen,
            root,
            ContextSourceKind::SelectedPersonalContext,
            "MEMORY.md",
            "bounded memory context surface; not trusted raw memory",
            "private",
            8,
        );
        push_known_file(
            &mut candidates,
            &mut seen,
            root,
            ContextSourceKind::SelectedPersonalContext,
            "memories/USER.md",
            "bounded user context surface; not canonical truth",
            "private",
            8,
        );
        push_known_file(
            &mut candidates,
            &mut seen,
            root,
            ContextSourceKind::SelectedPersonalContext,
            "memories/MEMORY.md",
            "bounded memory context surface; not trusted raw memory",
            "private",
            8,
        );
    }

    candidates
}

fn push_selected_skill_file(
    candidates: &mut Vec<ContextSourceCandidate>,
    seen: &mut HashSet<String>,
    root: &KnowledgeContextRoot,
    relative: &str,
    selected_skill_id: &str,
) {
    if let Some(candidate) = read_context_file(
        root,
        ContextSourceKind::SkillInstruction,
        relative,
        "full selected skill instruction; loaded only for selected skill",
        "internal",
        18,
    ) {
        push_unique(candidates, seen, candidate.for_skill(selected_skill_id));
    }
}

#[allow(clippy::too_many_arguments)]
fn push_known_file(
    candidates: &mut Vec<ContextSourceCandidate>,
    seen: &mut HashSet<String>,
    root: &KnowledgeContextRoot,
    kind: ContextSourceKind,
    relative: &str,
    reason: &str,
    privacy_class: &str,
    token_estimate: u32,
) {
    if let Some(candidate) =
        read_context_file(root, kind, relative, reason, privacy_class, token_estimate)
    {
        push_unique(candidates, seen, candidate);
    }
}

fn push_unique(
    candidates: &mut Vec<ContextSourceCandidate>,
    seen: &mut HashSet<String>,
    candidate: ContextSourceCandidate,
) {
    if seen.insert(candidate.source_id.clone()) {
        candidates.push(candidate);
    }
}

fn read_context_file(
    root: &KnowledgeContextRoot,
    kind: ContextSourceKind,
    relative: &str,
    reason: &str,
    privacy_class: &str,
    token_estimate: u32,
) -> Option<ContextSourceCandidate> {
    let relative_path = validate_context_relative_path(relative)?;
    let canonical_root = root.path.canonicalize().ok()?;
    let path = canonical_root.join(relative_path);
    let canonical = path.canonicalize().ok()?;
    if !canonical.starts_with(&canonical_root) || !canonical.is_file() {
        return None;
    }
    let content = std::fs::read_to_string(canonical).ok()?;
    let bounded = content
        .chars()
        .take(MAX_CONTEXT_CHARS_PER_FILE)
        .collect::<String>();
    Some(ContextSourceCandidate::new(
        kind,
        source_id(root, relative),
        bounded,
        reason,
        privacy_class,
        token_estimate,
    ))
}

fn validate_context_relative_path(relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute() {
        return None;
    }
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => safe.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => return None,
        }
    }
    if safe.as_os_str().is_empty() {
        None
    } else {
        Some(safe)
    }
}

fn source_id(root: &KnowledgeContextRoot, relative: &str) -> String {
    if root.label == "workspace" {
        relative.replace('\\', "/")
    } else {
        format!("{}:{}", root.label, relative.replace('\\', "/"))
    }
}

fn validate_selected_skill_id(selected_skill_id: &str) -> Option<String> {
    sanitize_main_chat_selected_skill_id(Some(selected_skill_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_bounded_workspace_knowledge_surfaces_and_selected_skill_only() {
        let dir = tempfile::tempdir().expect("temp knowledge root");
        std::fs::create_dir_all(dir.path().join("memories")).expect("memories dir");
        std::fs::create_dir_all(dir.path().join("skills/summarize")).expect("selected skill dir");
        std::fs::create_dir_all(dir.path().join("skills/other")).expect("other skill dir");
        std::fs::write(dir.path().join("AGENTS.md"), "workspace instructions").expect("agents");
        std::fs::write(dir.path().join("SOUL.md"), "soul context").expect("soul");
        std::fs::write(dir.path().join("USER.md"), "user context").expect("user");
        std::fs::write(dir.path().join("MEMORY.md"), "memory context").expect("memory");
        std::fs::write(dir.path().join("memories/USER.md"), "memories user")
            .expect("memories user");
        std::fs::write(dir.path().join("memories/MEMORY.md"), "memories memory")
            .expect("memories memory");
        std::fs::write(
            dir.path().join("skills/summarize/SKILL.md"),
            "selected skill instruction",
        )
        .expect("selected skill");
        std::fs::write(
            dir.path().join("skills/other/SKILL.md"),
            "unselected skill instruction",
        )
        .expect("other skill");

        let roots = vec![KnowledgeContextRoot::new("workspace", dir.path())];
        let candidates = load_knowledge_context_candidates_from_roots(&roots, Some("summarize"));
        let source_ids = candidates
            .iter()
            .map(|candidate| candidate.source_id.as_str())
            .collect::<Vec<_>>();

        for expected in [
            "AGENTS.md",
            "SOUL.md",
            "USER.md",
            "MEMORY.md",
            "memories/USER.md",
            "memories/MEMORY.md",
            "skills/summarize/SKILL.md",
        ] {
            assert!(
                source_ids.contains(&expected),
                "missing bounded knowledge source {expected}"
            );
        }
        assert!(!source_ids.contains(&"skills/other/SKILL.md"));
        assert!(candidates.iter().any(|candidate| {
            candidate.source_kind == ContextSourceKind::SkillInstruction
                && candidate.selected_skill_id.as_deref() == Some("summarize")
        }));
    }

    #[test]
    fn rejects_selected_skill_path_traversal() {
        let dir = tempfile::tempdir().expect("temp knowledge root");
        std::fs::create_dir_all(dir.path().join("skills/summarize")).expect("skill dir");
        std::fs::write(
            dir.path().join("skills/summarize/SKILL.md"),
            "selected skill instruction",
        )
        .expect("selected skill");

        let roots = vec![KnowledgeContextRoot::new("workspace", dir.path())];
        let candidates = load_knowledge_context_candidates_from_roots(&roots, Some("../summarize"));

        assert!(!candidates
            .iter()
            .any(|candidate| candidate.source_kind == ContextSourceKind::SkillInstruction));
    }

    #[test]
    fn bounds_loaded_file_content() {
        let dir = tempfile::tempdir().expect("temp knowledge root");
        std::fs::write(
            dir.path().join("AGENTS.md"),
            "a".repeat(MAX_CONTEXT_CHARS_PER_FILE + 50),
        )
        .expect("agents");

        let roots = vec![KnowledgeContextRoot::new("workspace", dir.path())];
        let candidates = load_knowledge_context_candidates_from_roots(&roots, None);
        let agents = candidates
            .iter()
            .find(|candidate| candidate.source_id == "AGENTS.md")
            .expect("agents candidate");

        assert_eq!(agents.content.chars().count(), MAX_CONTEXT_CHARS_PER_FILE);
    }
}
