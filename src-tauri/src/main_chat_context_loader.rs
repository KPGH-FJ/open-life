use openlife_core::agent::{memory_scope_owner_ref, MemoryLifecycleRecord, MemoryLifecycleScope};
use openlife_core::agent::{ContextSourceCandidate, ContextSourceKind};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use crate::AppState;

#[cfg(test)]
pub(crate) async fn retrievable_lifecycle_context_candidates(
    state: &Arc<AppState>,
    conversation_owner_id: &str,
    query: &str,
    life_model_rerank_terms: &[String],
) -> Result<Vec<ContextSourceCandidate>, String> {
    retrievable_lifecycle_context_candidates_with_scope_filter(
        state,
        conversation_owner_id,
        query,
        life_model_rerank_terms,
        None,
    )
    .await
}

pub(crate) async fn retrievable_lifecycle_context_candidates_with_scope_filter(
    state: &Arc<AppState>,
    conversation_owner_id: &str,
    query: &str,
    life_model_rerank_terms: &[String],
    exclusive_scopes: Option<&[MemoryLifecycleScope]>,
) -> Result<Vec<ContextSourceCandidate>, String> {
    let Some(lifecycle_store) = state.memory_lifecycle_store.as_ref() else {
        return Ok(vec![ContextSourceCandidate::new(
            ContextSourceKind::RuntimePolicy,
            "memory.lifecycle.unavailable",
            "memory_retrieval_degraded:lifecycle_store_unavailable; accepted lifecycle memory is unavailable for this turn and must not be inferred",
            "explicit optional-memory degradation boundary",
            "internal",
            12,
        )]);
    };
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let records = lifecycle_store
        .lock()
        .await
        .list_retrievable_records(None, 200)
        .map_err(|error| {
            format!("memory_retrieval_degraded:lifecycle_records_query_failed:{error}")
        })?;
    let scope = runtime_memory_scope(state, conversation_owner_id, &records).await?;
    let records = records
        .into_iter()
        .filter(|record| {
            scope.allows(record)
                && exclusive_scopes
                    .as_ref()
                    .is_none_or(|allowed| allowed.contains(&record.scope))
        })
        .map(|record| (record.memory_id.clone(), record))
        .collect::<HashMap<_, _>>();
    let allowed_memory_ids = records.keys().cloned().collect::<HashSet<_>>();

    let search = match crate::memory_gateway::search_lifecycle_memory_with_state(
        query.into(),
        512,
        &allowed_memory_ids,
        state,
    )
    .await
    {
        Ok(search) => search,
        Err(error) => {
            return Ok(vec![memory_retrieval_degraded_candidate(
                "memory_search_unavailable",
                &error.to_string(),
            )]);
        }
    };
    let retrieval_mode = if search.vector_status == "ready" {
        "fts_vector_candidates"
    } else {
        "fts_fallback"
    };
    let approximate_vector_requires_lexical_support = matches!(
        search.route_quality,
        crate::memory_gateway::EmbeddingRouteQuality::DeterministicHashApproximation
    );
    let mut ranked = search
        .hits
        .into_iter()
        .filter_map(|(chunk, relevance)| {
            let memory_id = chunk.source.strip_prefix("memory_lifecycle:")?;
            let record = records.get(memory_id)?.clone();
            if !record.conflict_ids.is_empty() {
                return None;
            }
            if approximate_vector_requires_lexical_support
                && !memory_has_lexical_support(query, &record.content)
            {
                return None;
            }
            let freshness = memory_freshness(&record);
            let life_model_bonus =
                life_model_memory_rerank_bonus(query, &record.content, life_model_rerank_terms);
            let score = retrieval_rank(relevance, &record, freshness) + life_model_bonus;
            Some((record, score, freshness, life_model_bonus))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.0.accepted_at.cmp(&left.0.accepted_at))
            .then_with(|| left.0.memory_id.cmp(&right.0.memory_id))
    });

    let mut selected = Vec::new();
    let mut selected_chars = 0usize;
    for (record, score, freshness, life_model_bonus) in ranked.into_iter().take(4) {
        let remaining = 4_800usize.saturating_sub(selected_chars);
        if remaining < 80 {
            break;
        }
        let content = bounded_memory_content(&record.content, remaining.min(1_200));
        selected_chars = selected_chars.saturating_add(content.chars().count());
        let scope_owner = record.scope_owner_ref.as_deref().unwrap_or("global");
        let selected_reason = format!(
            "mode={retrieval_mode}; relevance_rank={score:.3}; lifemodel_rerank_bonus={life_model_bonus:.3}; freshness={}; source_quality={}; conflict=none",
            freshness.as_str(),
            memory_source_quality(&record)
        );
        let context = format!(
            "Agent Memory (not identity, permission, or completion evidence)\nsource_ref={}\nscope={}\nscope_owner={}\nfreshness={}\nselected_reason={}\ncontent={}",
            record.memory_id,
            record.scope,
            scope_owner,
            freshness.as_str(),
            selected_reason,
            content
        );
        selected.push(ContextSourceCandidate::new(
            ContextSourceKind::SelectedPersonalContext,
            &record.memory_id,
            context,
            selected_reason,
            "private",
            (content.chars().count() / 4).clamp(12, 48) as u32,
        ));
    }

    if search.vector_status != "ready" {
        selected.insert(
            0,
            memory_retrieval_degraded_candidate(&search.vector_status, retrieval_mode),
        );
    }
    Ok(selected)
}

#[cfg(test)]
pub(crate) fn is_lifecycle_memory_context_candidate(candidate: &ContextSourceCandidate) -> bool {
    candidate.source_kind == ContextSourceKind::SelectedPersonalContext
        && candidate.source_id.starts_with("memory:")
}

fn life_model_memory_rerank_bonus(
    query: &str,
    memory_content: &str,
    life_model_terms: &[String],
) -> f32 {
    if life_model_terms.is_empty() {
        return 0.0;
    }
    let query = query.to_lowercase();
    let memory = memory_content.to_lowercase();
    let mut matched = 0usize;
    for term in life_model_terms {
        let term = term.to_lowercase();
        let overlaps_current_task = retrieval_tokens(&term)
            .into_iter()
            .any(|token| query.contains(&token) && memory.contains(&token));
        if overlaps_current_task {
            matched += 1;
        }
    }
    (matched.min(2) as f32) * 0.04
}

fn retrieval_tokens(value: &str) -> Vec<String> {
    let mut tokens = value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| token.chars().count() >= 3)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let cjk = value
        .chars()
        .filter(|character| {
            matches!(character, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}')
        })
        .collect::<Vec<_>>();
    tokens.extend(
        cjk.windows(2)
            .map(|window| window.iter().collect::<String>()),
    );
    tokens.sort();
    tokens.dedup();
    tokens
}

fn memory_has_lexical_support(query: &str, memory_content: &str) -> bool {
    let memory_tokens = retrieval_tokens(&memory_content.to_lowercase())
        .into_iter()
        .collect::<HashSet<_>>();
    retrieval_tokens(&query.to_lowercase())
        .into_iter()
        .any(|token| memory_tokens.contains(&token))
}

#[derive(Debug)]
struct RuntimeMemoryScope {
    conversation: String,
    legacy_conversations: HashSet<String>,
    workspace: Option<String>,
    project: Option<String>,
}

impl RuntimeMemoryScope {
    fn allows(&self, record: &MemoryLifecycleRecord) -> bool {
        match record.scope {
            MemoryLifecycleScope::Global => record.scope_owner_ref.is_none(),
            MemoryLifecycleScope::Conversation => {
                record.scope_owner_ref.as_deref() == Some(self.conversation.as_str())
                    || record
                        .scope_owner_ref
                        .as_ref()
                        .is_some_and(|owner| self.legacy_conversations.contains(owner))
            }
            MemoryLifecycleScope::Workspace => self
                .workspace
                .as_deref()
                .is_some_and(|owner| record.scope_owner_ref.as_deref() == Some(owner)),
            MemoryLifecycleScope::Project => self
                .project
                .as_deref()
                .is_some_and(|owner| record.scope_owner_ref.as_deref() == Some(owner)),
        }
    }
}

async fn runtime_memory_scope(
    state: &Arc<AppState>,
    conversation_owner_id: &str,
    _records: &[MemoryLifecycleRecord],
) -> Result<RuntimeMemoryScope, String> {
    let project = if let Some(store) = state.conversation_store.as_ref() {
        let store = store.lock().await;
        let conversation = store
            .get_conversation(conversation_owner_id)
            .map_err(|error| format!("memory_scope_conversation_query_failed:{error}"))?;
        match conversation.and_then(|conversation| conversation.project_id) {
            Some(project_id) => {
                let project = store
                    .get_project(&project_id)
                    .map_err(|error| format!("memory_scope_project_query_failed:{error}"))?
                    .ok_or_else(|| "memory_scope_project_missing".to_string())?;
                Some(
                    memory_scope_owner_ref(MemoryLifecycleScope::Project, &project.id)
                        .map_err(|error| error.to_string())?,
                )
            }
            None => None,
        }
    } else {
        None
    };
    Ok(RuntimeMemoryScope {
        conversation: memory_scope_owner_ref(
            MemoryLifecycleScope::Conversation,
            conversation_owner_id,
        )
        .map_err(|error| error.to_string())?,
        // Unscoped historical rows may remain visible in Personal
        // Intelligence, but production recall never guesses a Conversation
        // owner without an exact canonical scope binding.
        legacy_conversations: HashSet::new(),
        workspace: None,
        project,
    })
}

#[derive(Debug, Clone, Copy)]
enum MemoryFreshness {
    Recent,
    Current,
    Aging,
    Stale,
    Unknown,
}

impl MemoryFreshness {
    fn as_str(self) -> &'static str {
        match self {
            Self::Recent => "recent_30d",
            Self::Current => "current_180d",
            Self::Aging => "aging_365d",
            Self::Stale => "stale_over_365d",
            Self::Unknown => "unknown",
        }
    }

    fn rank_adjustment(self) -> f32 {
        match self {
            Self::Recent => 0.15,
            Self::Current => 0.08,
            Self::Aging => 0.0,
            Self::Stale => -0.20,
            Self::Unknown => -0.10,
        }
    }
}

fn memory_freshness(record: &MemoryLifecycleRecord) -> MemoryFreshness {
    let Some(accepted_at) = record.accepted_at else {
        return MemoryFreshness::Unknown;
    };
    let age = chrono::Utc::now().signed_duration_since(accepted_at);
    if age.num_days() <= 30 {
        MemoryFreshness::Recent
    } else if age.num_days() <= 180 {
        MemoryFreshness::Current
    } else if age.num_days() <= 365 {
        MemoryFreshness::Aging
    } else {
        MemoryFreshness::Stale
    }
}

fn memory_source_quality(record: &MemoryLifecycleRecord) -> &'static str {
    if record.proposal_id.starts_with("explicit_memory:") {
        "explicit_user_instruction"
    } else {
        "reviewed_proposal"
    }
}

fn retrieval_rank(
    relevance: f32,
    record: &MemoryLifecycleRecord,
    freshness: MemoryFreshness,
) -> f32 {
    let source_bonus = if record.proposal_id.starts_with("explicit_memory:") {
        0.10
    } else {
        0.05
    };
    relevance.clamp(0.0, 1.0)
        + freshness.rank_adjustment()
        + record.confidence.clamp(0.0, 1.0) * 0.10
        + source_bonus
}

fn bounded_memory_content(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        return content.to_string();
    }
    let mut bounded = content
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    if max_chars > 0 {
        bounded.push('…');
    }
    bounded
}

fn memory_retrieval_degraded_candidate(reason_code: &str, detail: &str) -> ContextSourceCandidate {
    ContextSourceCandidate::new(
        ContextSourceKind::RuntimePolicy,
        "memory.retrieval.degraded",
        format!(
            "memory_retrieval_degraded:{reason_code}; optional Agent Memory recall is incomplete for this turn; continue without inferring missing memories"
        ),
        format!("visible text-search fallback boundary: {detail}"),
        "internal",
        12,
    )
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

/// Add the packaged instruction only when no selected skill file was loaded.
/// This keeps configured workspace skills authoritative while making built-in
/// product skills independent from the process working directory.
pub(crate) fn ensure_bundled_selected_skill_context_candidate(
    candidates: &mut Vec<ContextSourceCandidate>,
    selected_skill_id: Option<&str>,
) {
    let Some(skill_id) = sanitize_main_chat_selected_skill_id(selected_skill_id) else {
        return;
    };
    if candidates.iter().any(|candidate| {
        candidate.source_kind == ContextSourceKind::SkillInstruction
            && candidate.selected_skill_id.as_deref() == Some(skill_id.as_str())
    }) {
        return;
    }
    let Some(instruction) =
        openlife_core::skills::SkillRegistry::built_in_runtime_instruction(&skill_id)
    else {
        return;
    };
    candidates.push(
        ContextSourceCandidate::new(
            ContextSourceKind::SkillInstruction,
            format!("bundled:skills/{skill_id}/SKILL.md"),
            instruction,
            "packaged selected skill instruction; repository working directory not required",
            "internal",
            18,
        )
        .for_skill(skill_id),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepted_lifecycle_memory_proposal(content: &str) -> openlife_core::agent::AgentProposal {
        lifecycle_memory_proposal(content, "global", None, Vec::new())
    }

    fn lifecycle_memory_proposal(
        content: &str,
        scope: &str,
        scope_owner_ref: Option<&str>,
        conflict_ids: Vec<&str>,
    ) -> openlife_core::agent::AgentProposal {
        let mut after = serde_json::json!({
            "content": content,
            "scope": scope,
            "category": "fact",
            "candidateKind": "semantic_user_fact",
            "riskLevel": "low",
            "sensitivity": "internal",
            "conflictIds": conflict_ids,
        });
        if let Some(scope_owner_ref) = scope_owner_ref {
            after["scopeOwnerRef"] = serde_json::json!(scope_owner_ref);
        }
        let mut proposal = openlife_core::agent::AgentProposal::new(
            openlife_core::agent::ProposalType::MemoryWrite,
            "memory.candidates",
            after,
            "User reviewed a lifecycle Memory candidate.",
            0.9,
            openlife_core::agent::RiskLevel::Low,
            openlife_core::agent::ProposalSource::Manual,
        );
        proposal.id = format!("proposal:context:retrieval:{}", uuid::Uuid::new_v4());
        proposal
    }

    async fn accept_and_project_memory(
        state: &Arc<AppState>,
        proposal: &openlife_core::agent::AgentProposal,
    ) -> openlife_core::agent::MemoryLifecycleRecord {
        let content = proposal.after["content"].as_str().unwrap().to_string();
        let accepted = state
            .memory_lifecycle_store
            .as_ref()
            .expect("lifecycle store")
            .lock()
            .await
            .accept_memory_proposal(
                openlife_core::agent::MemoryLifecycleAcceptanceInput::from_memory_proposal(
                    proposal, content,
                )
                .expect("typed lifecycle proposal"),
            )
            .expect("accepted lifecycle memory")
            .record;
        crate::memory_gateway::reconcile_canonical_outboxes_with_state(state, 32)
            .await
            .expect("project lifecycle Memory");
        accepted
    }

    #[test]
    fn packaged_evidence_review_does_not_require_a_workspace_file() {
        let mut candidates = Vec::new();
        ensure_bundled_selected_skill_context_candidate(&mut candidates, Some("evidence_review"));

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(candidate.source_kind, ContextSourceKind::SkillInstruction);
        assert_eq!(
            candidate.source_id,
            "bundled:skills/evidence_review/SKILL.md"
        );
        assert_eq!(
            candidate.selected_skill_id.as_deref(),
            Some("evidence_review")
        );
        assert!(candidate.content.contains("Review only evidence supplied"));
    }

    #[test]
    fn workspace_skill_instruction_wins_over_packaged_fallback() {
        let mut candidates = vec![ContextSourceCandidate::new(
            ContextSourceKind::SkillInstruction,
            "skills/evidence_review/SKILL.md",
            "workspace-owned selected instruction",
            "selected skill instruction",
            "internal",
            18,
        )
        .for_skill("evidence_review")];
        ensure_bundled_selected_skill_context_candidate(&mut candidates, Some("evidence_review"));

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].content,
            "workspace-owned selected instruction"
        );
    }

    #[tokio::test]
    async fn deterministic_hash_top_k_does_not_admit_unrelated_lifecycle_memory() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        for content in [
            "5.6C 全局发布标记是 OL-G5-GLOBAL-314",
            "5.6C 工作区标记是 OL-G5-WORKSPACE-482",
            "5.6C 项目标记是 OL-G5-PROJECT-693",
        ] {
            accept_and_project_memory(&state, &accepted_lifecycle_memory_proposal(content)).await;
        }

        let candidates = retrievable_lifecycle_context_candidates(
            &state,
            "conversation-a",
            "请为一次开发阶段复盘写一段四句话的内部说明，内容包含完成情况、主要问题、下一步。",
            &[],
        )
        .await
        .expect("ordinary generation context retrieval");

        assert!(
            !candidates.iter().any(is_lifecycle_memory_context_candidate),
            "unrelated approximate Top-K results must not enter the model context"
        );
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
        crate::memory_gateway::reconcile_canonical_outboxes_with_state(&state, 16)
            .await
            .expect("project lifecycle memory before retrieval");

        let before =
            retrievable_lifecycle_context_candidates(&state, "conversation-a", SENTINEL, &[])
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

        let after =
            retrievable_lifecycle_context_candidates(&state, "conversation-a", SENTINEL, &[])
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
    async fn project_scope_recall_requires_a_canonical_project_binding() {
        const ALLOWED: &str = "PROJECT_SCOPE_ALLOWED_RELEASE_CHECKLIST";
        const BLOCKED: &str = "PROJECT_SCOPE_BLOCKED_RELEASE_CHECKLIST";
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let project_a_id = uuid::Uuid::new_v4().to_string();
        let project_b_id = uuid::Uuid::new_v4().to_string();
        {
            let store = state
                .conversation_store
                .as_ref()
                .expect("conversation store")
                .lock()
                .await;
            store
                .create_conversation(&conversation_id, "Project Memory isolation")
                .expect("create conversation");
            store
                .create_project(&project_a_id, "Project A", None)
                .expect("create project A");
            store
                .create_project(&project_b_id, "Project B", None)
                .expect("create project B");
            store
                .assign_conversation_project(&conversation_id, Some(&project_a_id))
                .expect("bind project A");
        }
        let owner_a = memory_scope_owner_ref(MemoryLifecycleScope::Project, &project_a_id).unwrap();
        let owner_b = memory_scope_owner_ref(MemoryLifecycleScope::Project, &project_b_id).unwrap();
        let allowed = accept_and_project_memory(
            &state,
            &lifecycle_memory_proposal(ALLOWED, "project", Some(&owner_a), Vec::new()),
        )
        .await;
        let blocked = accept_and_project_memory(
            &state,
            &lifecycle_memory_proposal(BLOCKED, "project", Some(&owner_b), Vec::new()),
        )
        .await;

        let candidates = retrievable_lifecycle_context_candidates(
            &state,
            &conversation_id,
            "PROJECT SCOPE RELEASE CHECKLIST",
            &[],
        )
        .await
        .expect("scoped lifecycle retrieval");
        assert!(candidates
            .iter()
            .any(|candidate| candidate.source_id == allowed.memory_id));
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.source_id == blocked.memory_id));
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.content.contains(BLOCKED)));

        state
            .conversation_store
            .as_ref()
            .expect("conversation store")
            .lock()
            .await
            .assign_conversation_project(&conversation_id, Some(&project_b_id))
            .expect("bind project B");
        let project_b_candidates = retrievable_lifecycle_context_candidates(
            &state,
            &conversation_id,
            "PROJECT SCOPE RELEASE CHECKLIST",
            &[],
        )
        .await
        .expect("project B lifecycle retrieval");
        assert!(!project_b_candidates
            .iter()
            .any(|candidate| candidate.source_id == allowed.memory_id));
        assert!(project_b_candidates
            .iter()
            .any(|candidate| candidate.source_id == blocked.memory_id));

        state
            .conversation_store
            .as_ref()
            .expect("conversation store")
            .lock()
            .await
            .assign_conversation_project(&conversation_id, None)
            .expect("remove project binding");
        let unbound_candidates = retrievable_lifecycle_context_candidates(
            &state,
            &conversation_id,
            "PROJECT SCOPE RELEASE CHECKLIST",
            &[],
        )
        .await
        .expect("unbound lifecycle retrieval");
        assert!(!unbound_candidates
            .iter()
            .any(|candidate| candidate.source_id == allowed.memory_id));
        assert!(!unbound_candidates
            .iter()
            .any(|candidate| candidate.source_id == blocked.memory_id));
        let projected = state
            .memory_store
            .lock()
            .await
            .export_active_memory_records()
            .expect("non-lifecycle Memory rows");
        assert!(!projected
            .iter()
            .any(|memory| memory.content == ALLOWED || memory.content == BLOCKED));
    }

    #[tokio::test]
    async fn conflicted_and_unbound_non_global_memory_are_not_recalled() {
        const CONFLICTED: &str = "CONFLICTED_MEMORY_MUST_NOT_RECALL";
        const UNBOUND: &str = "UNBOUND_WORKSPACE_MEMORY_MUST_NOT_RECALL";
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let workspace = tempfile::tempdir().expect("workspace");
        let workspace_path = workspace.path().canonicalize().unwrap();
        let owner = memory_scope_owner_ref(
            MemoryLifecycleScope::Workspace,
            workspace_path.to_str().unwrap(),
        )
        .unwrap();
        let conflicted = accept_and_project_memory(
            &state,
            &lifecycle_memory_proposal(
                CONFLICTED,
                "workspace",
                Some(&owner),
                vec!["conflict:current-user-correction"],
            ),
        )
        .await;
        let unbound = accept_and_project_memory(
            &state,
            &lifecycle_memory_proposal(UNBOUND, "workspace", None, Vec::new()),
        )
        .await;

        for (query, memory_id) in [
            (CONFLICTED, conflicted.memory_id),
            (UNBOUND, unbound.memory_id),
        ] {
            let candidates = retrievable_lifecycle_context_candidates(
                &state,
                "conversation-a",
                query,
                &[query.to_string()],
            )
            .await
            .expect("fail-closed lifecycle retrieval");
            assert!(!candidates
                .iter()
                .any(|candidate| candidate.source_id == memory_id));
        }
    }

    #[tokio::test]
    async fn chinese_memory_is_recalled_with_source_scope_freshness_and_reason() {
        const MEMORY: &str = "发布前先检查中文本地化和键盘导航";
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let accepted =
            accept_and_project_memory(&state, &accepted_lifecycle_memory_proposal(MEMORY)).await;

        let candidates = retrievable_lifecycle_context_candidates(
            &state,
            "conversation-a",
            "发布检查包括哪些本地化工作",
            &[],
        )
        .await
        .expect("Chinese lifecycle retrieval");
        let recalled = candidates
            .iter()
            .find(|candidate| candidate.source_id == accepted.memory_id)
            .expect("Chinese Memory should be recalled");
        assert!(recalled.content.contains("source_ref=memory:"));
        assert!(recalled.content.contains("scope=global"));
        assert!(recalled.content.contains("freshness="));
        assert!(recalled.content.contains("selected_reason="));
    }

    #[tokio::test]
    async fn lifecycle_context_recall_is_bounded_and_meets_the_local_latency_guard() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        for index in 0..6 {
            let content = format!(
                "bounded recall checklist item {index}: {}",
                "verify product evidence ".repeat(80)
            );
            accept_and_project_memory(&state, &accepted_lifecycle_memory_proposal(&content)).await;
        }

        let started = std::time::Instant::now();
        let candidates = retrievable_lifecycle_context_candidates(
            &state,
            "conversation-a",
            "bounded recall checklist product evidence",
            &[],
        )
        .await
        .expect("bounded lifecycle retrieval");
        let elapsed = started.elapsed();
        let memories = candidates
            .iter()
            .filter(|candidate| candidate.source_id.starts_with("memory:"))
            .collect::<Vec<_>>();
        let injected_chars = memories
            .iter()
            .filter_map(|candidate| {
                candidate
                    .content
                    .split_once("\ncontent=")
                    .map(|(_, body)| body)
            })
            .map(|body| body.chars().count())
            .sum::<usize>();

        assert!(memories.len() <= 4);
        assert!(injected_chars <= 4_800);
        assert!(elapsed < std::time::Duration::from_secs(2), "{elapsed:?}");
    }

    #[test]
    fn stale_memory_is_ranked_below_an_equivalent_recent_memory() {
        let recent = openlife_core::agent::MemoryLifecycleRecord {
            memory_id: "memory:recent".into(),
            proposal_id: "explicit_memory:recent".into(),
            source_task_id: None,
            source_run_id: None,
            content: "same relevant content".into(),
            scope: MemoryLifecycleScope::Global,
            scope_owner_ref: None,
            category: openlife_core::agent::MemoryLifecycleCategory::Fact,
            risk_level: openlife_core::agent::MemoryLifecycleRiskLevel::Low,
            sensitivity: openlife_core::agent::MemoryLifecycleSensitivity::Internal,
            audit_digest: "sha256:test".into(),
            status: openlife_core::agent::MemoryLifecycleStatus::Materialized,
            materialization_status: openlife_core::agent::MemoryMaterializationStatus::Materialized,
            materialization_error_code: None,
            created_by: "test".into(),
            accepted_by: Some("user".into()),
            accepted_at: Some(chrono::Utc::now()),
            materialized_view_id: Some("view".into()),
            materialized_view_version: Some(1),
            evidence_ids: vec!["evidence".into()],
            confidence: 1.0,
            conflict_ids: Vec::new(),
            supersedes_memory_id: None,
            replacement_memory_id: None,
            rolled_back_by_event_id: None,
            runtime_context_excluded_at: None,
        };
        let mut stale = recent.clone();
        stale.memory_id = "memory:stale".into();
        stale.accepted_at = Some(chrono::Utc::now() - chrono::Duration::days(400));

        assert!(
            retrieval_rank(0.7, &recent, memory_freshness(&recent))
                > retrieval_rank(0.7, &stale, memory_freshness(&stale))
        );
        assert_eq!(memory_freshness(&stale).as_str(), "stale_over_365d");
    }

    #[test]
    fn lifemodel_memory_bonus_is_bounded_and_requires_current_task_overlap() {
        let hints = vec!["OpenLife personal Agent OS product".to_string()];
        assert_eq!(
            life_model_memory_rerank_bonus(
                "Plan the OpenLife release",
                "Past OpenLife release checklist",
                &hints,
            ),
            0.04
        );
        assert_eq!(
            life_model_memory_rerank_bonus(
                "Plan a family vacation",
                "Past OpenLife release checklist",
                &hints,
            ),
            0.0
        );
        let many = vec![
            "OpenLife release".to_string(),
            "OpenLife product".to_string(),
            "OpenLife Agent OS".to_string(),
        ];
        assert_eq!(
            life_model_memory_rerank_bonus(
                "Plan the OpenLife release",
                "OpenLife release product Agent OS",
                &many,
            ),
            0.08
        );
    }

    #[tokio::test]
    async fn eligible_memory_candidate_records_lifemodel_rerank_without_new_admission() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let accepted = accept_and_project_memory(
            &state,
            &accepted_lifecycle_memory_proposal("OpenLife release checklist from the last sprint"),
        )
        .await;
        let candidates = retrievable_lifecycle_context_candidates(
            &state,
            "conversation-a",
            "Plan the OpenLife release",
            &["OpenLife personal Agent OS product".into()],
        )
        .await
        .expect("eligible lifecycle memory retrieval");
        let candidate = candidates
            .iter()
            .find(|candidate| candidate.source_id == accepted.memory_id)
            .expect("already-eligible memory candidate");

        assert!(candidate
            .inclusion_reason
            .contains("lifemodel_rerank_bonus=0.040"));
        assert!(candidate.content.contains("scope=global"));
        assert!(candidate.content.contains("conflict=none"));
    }

    #[test]
    fn embedding_failure_marker_does_not_claim_complete_retrieval() {
        let candidate = memory_retrieval_degraded_candidate("embedding_failed", "fts_fallback");
        assert_eq!(candidate.source_kind, ContextSourceKind::RuntimePolicy);
        assert!(candidate.content.contains("memory_retrieval_degraded"));
        assert!(candidate.inclusion_reason.contains("text-search fallback"));
        assert!(candidate.content.contains("recall is incomplete"));
    }

    #[tokio::test]
    async fn missing_lifecycle_store_is_explicitly_degraded_without_blocking_base_context() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("isolated state owner")
            .memory_lifecycle_store = None;

        let candidates = retrievable_lifecycle_context_candidates(
            &state,
            "conversation-a",
            "find remembered context",
            &[],
        )
        .await
        .expect("optional lifecycle memory must not disable the base Agent");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source_id, "memory.lifecycle.unavailable");
        assert!(candidates[0]
            .content
            .contains("memory_retrieval_degraded:lifecycle_store_unavailable"));
    }
}
