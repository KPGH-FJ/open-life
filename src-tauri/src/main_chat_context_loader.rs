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
) -> Result<openlife_core::agent::main_chat_agent_v1::CompiledContext, String> {
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
    let configured_knowledge_roots = {
        let config = state.config.lock().await;
        config.system.knowledge_roots.clone()
    };
    let current_skill_id = selected_skill_id.clone();
    let configured_skill_id = selected_skill_id.clone();
    let current_context = tokio::task::spawn_blocking(move || {
        load_current_workspace_knowledge_context_candidates(current_skill_id.as_deref())
    });
    let configured_context = tokio::task::spawn_blocking(move || {
        load_configured_knowledge_context_candidates(
            &configured_knowledge_roots,
            configured_skill_id.as_deref(),
        )
    });
    let (current_context, configured_context) = tokio::join!(current_context, configured_context);
    candidates.extend(current_context.unwrap_or_default());
    candidates.extend(configured_context.unwrap_or_default());
    candidates.extend(retrievable_lifecycle_context_candidates(state).await?);
    let sessions = {
        let store = state.memory_store.lock().await;
        store.list_sessions(5).map_err(|error| {
            format!("memory_retrieval_degraded:memory_store_query_failed:{error}")
        })?
    };
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

    Ok(ContextCompiler.compile(ContextCompilerInput {
        strategy: decision.selected_strategy,
        privacy_risk: decision.privacy_risk.clone(),
        active_session_id: Some(task_session_id.to_string()),
        token_budget: 160,
        selected_skill_id,
        candidates,
    }))
}

/// The only Main Chat adapter from canonical lifecycle Memory into prompt
/// context. Both ordinary send/stream compilation and the command-surface
/// kernel use this function, so neither can reinterpret a lagging vector or
/// MemoryStore projection as current lifecycle truth.
pub(crate) async fn retrievable_lifecycle_context_candidates(
    state: &Arc<AppState>,
) -> Result<Vec<ContextSourceCandidate>, String> {
    let lifecycle_store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(|| "memory_retrieval_degraded:lifecycle_store_unavailable".to_string())?;
    let store = lifecycle_store.lock().await;
    let records = store.list_retrievable_records(None, 8).map_err(|error| {
        format!("memory_retrieval_degraded:lifecycle_records_query_failed:{error}")
    })?;
    Ok(records
        .into_iter()
        .map(|record| {
            ContextSourceCandidate::new(
                ContextSourceKind::SelectedPersonalContext,
                &record.memory_id,
                format!(
                    "Accepted memory [{}:{}]: {}",
                    record.scope, record.category, record.content
                ),
                format!(
                    "accepted memory lifecycle; materialized view {} version {}",
                    record.materialized_view_id.as_deref().unwrap_or("unknown"),
                    record.materialized_view_version.unwrap_or_default()
                ),
                "private",
                16,
            )
        })
        .collect())
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

pub(crate) fn load_configured_knowledge_context_candidates(
    configured_roots: &[String],
    selected_skill_id: Option<&str>,
) -> Vec<ContextSourceCandidate> {
    let roots = configured_roots
        .iter()
        .enumerate()
        .filter_map(|(index, raw)| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            let path = PathBuf::from(trimmed);
            if !path.is_dir() {
                return None;
            }
            let label = if index == 0 {
                "app_configured".to_string()
            } else {
                format!("app_configured_{}", index + 1)
            };
            Some(KnowledgeContextRoot::new(
                label,
                path.canonicalize().ok().unwrap_or(path),
            ))
        })
        .collect::<Vec<_>>();

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

    fn accepted_lifecycle_memory_proposal(content: &str) -> openlife_core::agent::AgentProposal {
        let mut proposal = openlife_core::agent::AgentProposal::new(
            openlife_core::agent::ProposalType::MemoryWrite,
            "memory.candidates",
            serde_json::json!({
                "content": content,
                "scope": "global",
                "category": "fact",
                "candidateKind": "semantic_user_fact",
                "riskLevel": "low",
                "sensitivity": "internal"
            }),
            "User reviewed a lifecycle Memory candidate.",
            0.9,
            openlife_core::agent::RiskLevel::Low,
            openlife_core::agent::ProposalSource::Manual,
        );
        proposal.id = format!("proposal:context:retrieval:{}", uuid::Uuid::new_v4());
        proposal
    }

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

    #[tokio::test]
    async fn archived_lifecycle_memory_is_excluded_before_projection_catches_up() {
        const SENTINEL: &str = "ARCHIVED_CONTEXT_MUST_NOT_LEAK";
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let proposal = accepted_lifecycle_memory_proposal(SENTINEL);
        let accepted = {
            let store = state
                .memory_lifecycle_store
                .as_ref()
                .expect("lifecycle store")
                .lock()
                .await;
            store
                .accept_memory_proposal(
                    openlife_core::agent::MemoryLifecycleAcceptanceInput::from_memory_proposal(
                        &proposal,
                        SENTINEL.to_string(),
                    )
                    .expect("typed lifecycle acceptance"),
                )
                .expect("accept lifecycle memory")
        };

        let before = retrievable_lifecycle_context_candidates(&state)
            .await
            .expect("canonical lifecycle reader");
        assert!(before
            .iter()
            .any(|candidate| candidate.source_id == accepted.record.memory_id));

        let archive = {
            let store = state
                .memory_lifecycle_store
                .as_ref()
                .expect("lifecycle store")
                .lock()
                .await;
            let archive = store
                .set_memory_retrieval_disposition(
                    &accepted.record.memory_id,
                    openlife_core::memory::MemoryRetrievalDisposition::Archived,
                    "user_reviewed_archive",
                )
                .expect("archive canonical lifecycle owner");
            let event_id = archive
                .canonical_mutation
                .as_ref()
                .expect("archive outbox")
                .event_id
                .clone();
            assert_eq!(
                store
                    .projection_summary(&event_id)
                    .expect("pending archive projection")
                    .pending,
                1
            );
            archive
        };
        assert!(archive.changed);

        let after = retrievable_lifecycle_context_candidates(&state)
            .await
            .expect("canonical lifecycle reader");
        assert!(!after
            .iter()
            .any(|candidate| candidate.source_id == accepted.record.memory_id));
        assert!(!after
            .iter()
            .any(|candidate| candidate.content.contains(SENTINEL)));
    }

    #[tokio::test]
    async fn missing_lifecycle_store_is_degraded_not_healthy_empty_context() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("isolated state owner")
            .memory_lifecycle_store = None;

        let error = retrievable_lifecycle_context_candidates(&state)
            .await
            .expect_err("missing canonical lifecycle store cannot become empty context");
        assert_eq!(
            error,
            "memory_retrieval_degraded:lifecycle_store_unavailable"
        );
    }
}
