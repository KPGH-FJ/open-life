use openlife_core::agent::main_chat_agent_v1::{
    ContextCompiler, ContextCompilerInput, ContextSourceCandidate, ContextSourceKind,
};
use openlife_core::agent::{memory_scope_owner_ref, MemoryLifecycleRecord, MemoryLifecycleScope};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::AppState;

const MAX_CONTEXT_CHARS_PER_FILE: usize = 1200;
const CONFIGURED_KNOWLEDGE_ROOT_ENV: &str = "OPENLIFE_KNOWLEDGE_ROOT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatContextTaskMode {
    OpenEnded,
    EvidenceBoundSources,
    EvidenceBoundMarkdown,
    EvidenceBoundDocuments,
    EvidenceBoundAgentMemory,
    ExactAgentMemoryRead,
}

impl MainChatContextTaskMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::OpenEnded => "open_ended",
            Self::EvidenceBoundSources => "evidence_bound_sources",
            Self::EvidenceBoundMarkdown => "evidence_bound_markdown",
            Self::EvidenceBoundDocuments => "evidence_bound_documents",
            Self::EvidenceBoundAgentMemory => "evidence_bound_agent_memory",
            Self::ExactAgentMemoryRead => "exact_agent_memory_read",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatInlineFact {
    pub(crate) handle: String,
    pub(crate) content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatContextRequest {
    pub(crate) task_mode: MainChatContextTaskMode,
    pub(crate) memory_scopes: Vec<MemoryLifecycleScope>,
    pub(crate) inline_facts: Vec<MainChatInlineFact>,
}

impl MainChatContextRequest {
    pub(crate) fn from_user_text(user_text: &str) -> Self {
        let lower = user_text.to_lowercase();
        let positive_source_text = without_negated_source_clauses(&lower);
        let explicitly_exclusive = [
            "只允许使用",
            "只使用",
            "仅使用",
            "只根据",
            "仅根据",
            "只基于",
            "仅基于",
            "only use",
            "use only",
            "only rely on",
            "exclusively use",
            "based only on",
            "only based on",
        ]
        .iter()
        .any(|marker| positive_source_text.contains(marker));
        if !explicitly_exclusive {
            return Self::open_ended();
        }

        let explicitly_agent_memory = ["agent memory", "agent-memory", "智能体记忆", "代理记忆"]
            .iter()
            .any(|marker| positive_source_text.contains(marker));
        if !explicitly_agent_memory {
            let inline_facts = extract_explicit_inline_facts(user_text);
            let explicitly_names_a_source = [
                "以下信息",
                "给定信息",
                "以下事实",
                "给定事实",
                "以下资料",
                "这些资料",
                "选中的资料",
                "文档",
                "markdown",
                "these facts",
                "following facts",
                "provided information",
                "selected sources",
                "selected documents",
            ]
            .iter()
            .any(|marker| positive_source_text.contains(marker));
            return if !inline_facts.is_empty() {
                Self {
                    task_mode: MainChatContextTaskMode::EvidenceBoundSources,
                    memory_scopes: Vec::new(),
                    inline_facts,
                }
            } else if positive_source_text.contains("markdown") {
                Self {
                    task_mode: MainChatContextTaskMode::EvidenceBoundMarkdown,
                    memory_scopes: Vec::new(),
                    inline_facts: Vec::new(),
                }
            } else if explicitly_names_a_source {
                Self {
                    task_mode: MainChatContextTaskMode::EvidenceBoundDocuments,
                    memory_scopes: Vec::new(),
                    inline_facts: Vec::new(),
                }
            } else {
                Self::open_ended()
            };
        }

        let mut memory_scopes = Vec::new();
        for (scope, markers) in [
            (
                MemoryLifecycleScope::Global,
                &[
                    "全局 agent memory",
                    "全局作用域",
                    "全局范围",
                    "global agent memory",
                    "global scope",
                    "global-scoped",
                ][..],
            ),
            (
                MemoryLifecycleScope::Conversation,
                &[
                    "当前会话 agent memory",
                    "当前会话作用域",
                    "当前会话范围",
                    "current conversation agent memory",
                    "conversation scope",
                    "conversation-scoped",
                ][..],
            ),
            (
                MemoryLifecycleScope::Workspace,
                &[
                    "当前工作区 agent memory",
                    "当前工作区作用域",
                    "当前工作区范围",
                    "current workspace agent memory",
                    "workspace scope",
                    "workspace-scoped",
                ][..],
            ),
            (
                MemoryLifecycleScope::Project,
                &[
                    "当前项目 agent memory",
                    "当前项目作用域",
                    "当前项目范围",
                    "current project agent memory",
                    "project scope",
                    "project-scoped",
                ][..],
            ),
        ] {
            if markers
                .iter()
                .any(|marker| positive_source_text.contains(marker))
            {
                memory_scopes.push(scope);
            }
        }

        Self {
            task_mode: if memory_scopes.is_empty() {
                MainChatContextTaskMode::EvidenceBoundAgentMemory
            } else {
                MainChatContextTaskMode::ExactAgentMemoryRead
            },
            memory_scopes,
            inline_facts: Vec::new(),
        }
    }

    fn open_ended() -> Self {
        Self {
            task_mode: MainChatContextTaskMode::OpenEnded,
            memory_scopes: Vec::new(),
            inline_facts: Vec::new(),
        }
    }

    pub(crate) fn is_agent_memory_bound(&self) -> bool {
        matches!(
            self.task_mode,
            MainChatContextTaskMode::EvidenceBoundAgentMemory
                | MainChatContextTaskMode::ExactAgentMemoryRead
        )
    }

    pub(crate) fn is_source_bound(&self) -> bool {
        self.task_mode != MainChatContextTaskMode::OpenEnded
    }

    pub(crate) fn is_inline_fact_bound(&self) -> bool {
        self.task_mode == MainChatContextTaskMode::EvidenceBoundSources
            && !self.inline_facts.is_empty()
    }

    pub(crate) fn is_markdown_bound(&self) -> bool {
        self.task_mode == MainChatContextTaskMode::EvidenceBoundMarkdown
    }

    pub(crate) fn is_document_bound(&self) -> bool {
        self.task_mode == MainChatContextTaskMode::EvidenceBoundDocuments
    }
}

fn without_negated_source_clauses(value: &str) -> String {
    value
        .split(['。', '！', '？', '；', '.', '!', '?', ';'])
        .filter(|clause| {
            ![
                "不要使用",
                "不要读取",
                "不要参考",
                "不使用",
                "不读取",
                "不参考",
                "排除",
                "do not use",
                "don't use",
                "do not read",
                "don't read",
                "do not rely on",
                "don't rely on",
                "without using",
                "without reading",
                "exclude",
            ]
            .iter()
            .any(|marker| clause.contains(marker))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

const MAX_EXPLICIT_INLINE_FACTS: usize = 16;
const MAX_EXPLICIT_INLINE_FACT_CHARS: usize = 1_000;
const MAX_EXPLICIT_INLINE_FACT_TOTAL_CHARS: usize = 6_000;

fn extract_explicit_inline_facts(user_text: &str) -> Vec<MainChatInlineFact> {
    let lower = user_text.to_lowercase();
    let source_start = [
        "以下信息",
        "给定信息",
        "以下事实",
        "给定事实",
        "以下资料",
        "these facts",
        "following facts",
        "provided information",
    ]
    .iter()
    .filter_map(|marker| lower.find(marker).map(|index| index + marker.len()))
    .min();
    let Some(source_start) = source_start else {
        return Vec::new();
    };
    let Some(after_marker) = user_text.get(source_start..) else {
        return Vec::new();
    };
    let Some(separator) = after_marker.find(['：', ':']) else {
        return Vec::new();
    };
    let separator_len = after_marker[separator..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or_default();
    let mut fact_text = after_marker[separator + separator_len..].trim();
    let fact_text_lower = fact_text.to_lowercase();
    if let Some(control_start) = [
        "不要使用工具",
        "不要调用工具",
        "不要执行任何",
        "不要执行外部",
        "do not use tools",
        "do not call tools",
        "do not execute",
        "do not perform",
    ]
    .iter()
    .filter_map(|marker| fact_text_lower.find(marker))
    .min()
    {
        fact_text = fact_text[..control_start].trim();
    }
    fact_text = fact_text.trim_end_matches(['。', '.', '；', ';', ' ', '\n', '\r']);

    let mut total_chars = 0usize;
    let mut facts = Vec::new();
    for part in fact_text.split(['；', ';', '\n']) {
        let content = part
            .trim()
            .trim_start_matches(['-', '*', '•', ' '])
            .trim()
            .trim_end_matches(['。', '.'])
            .trim();
        let chars = content.chars().count();
        if chars == 0 || chars > MAX_EXPLICIT_INLINE_FACT_CHARS {
            continue;
        }
        total_chars = total_chars.saturating_add(chars);
        if total_chars > MAX_EXPLICIT_INLINE_FACT_TOTAL_CHARS
            || facts.len() >= MAX_EXPLICIT_INLINE_FACTS
        {
            return Vec::new();
        }
        facts.push(MainChatInlineFact {
            handle: format!("F{}", facts.len() + 1),
            content: content.to_string(),
        });
    }
    facts
}

#[allow(dead_code)]
pub(crate) async fn compile_main_chat_context(
    state: &Arc<AppState>,
    decision: &openlife_core::agent::main_chat_agent_v1::AgentIngressDecision,
    task_session_id: &str,
    conversation_owner_id: &str,
    user_text: &str,
    selected_skill_id: Option<&str>,
) -> Result<openlife_core::agent::main_chat_agent_v1::CompiledContext, String> {
    let context_request = MainChatContextRequest::from_user_text(user_text);
    if context_request.is_agent_memory_bound() {
        let candidates =
            retrievable_lifecycle_context_candidates(state, conversation_owner_id, user_text, &[])
                .await?
                .into_iter()
                .filter(|candidate| {
                    lifecycle_memory_candidate_matches_request(candidate, &context_request)
                })
                .collect();
        return Ok(ContextCompiler.compile(ContextCompilerInput {
            strategy: decision.selected_strategy,
            privacy_risk: decision.privacy_risk.clone(),
            active_session_id: Some(task_session_id.to_string()),
            token_budget: 160,
            selected_skill_id: None,
            candidates,
        }));
    }

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
    let current_task_text = user_text.to_string();
    let configured_task_text = user_text.to_string();
    let current_context = tokio::task::spawn_blocking(move || {
        load_current_workspace_knowledge_context_candidates(
            current_skill_id.as_deref(),
            &current_task_text,
        )
    });
    let configured_context = tokio::task::spawn_blocking(move || {
        load_configured_knowledge_context_candidates(
            &configured_knowledge_roots,
            configured_skill_id.as_deref(),
            &configured_task_text,
        )
    });
    let (current_context, configured_context) = tokio::join!(current_context, configured_context);
    candidates.extend(
        retrievable_lifecycle_context_candidates(state, conversation_owner_id, user_text, &[])
            .await?,
    );
    candidates.extend(current_context.unwrap_or_default());
    candidates.extend(configured_context.unwrap_or_default());
    candidates.extend(load_configured_markdown_memory_context_candidates(state, user_text).await?);
    ensure_bundled_selected_skill_context_candidate(&mut candidates, selected_skill_id.as_deref());

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
    conversation_owner_id: &str,
    query: &str,
    life_model_rerank_terms: &[String],
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
    let exclusive_scopes = explicit_exclusive_memory_scopes(query);
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

fn explicit_exclusive_memory_scopes(query: &str) -> Option<Vec<MemoryLifecycleScope>> {
    let request = MainChatContextRequest::from_user_text(query);
    (request.task_mode == MainChatContextTaskMode::ExactAgentMemoryRead)
        .then_some(request.memory_scopes)
}

pub(crate) fn is_lifecycle_memory_context_candidate(candidate: &ContextSourceCandidate) -> bool {
    candidate.source_kind == ContextSourceKind::SelectedPersonalContext
        && candidate.source_id.starts_with("memory:")
}

pub(crate) fn lifecycle_memory_candidate_matches_request(
    candidate: &ContextSourceCandidate,
    request: &MainChatContextRequest,
) -> bool {
    if !is_lifecycle_memory_context_candidate(candidate) {
        return false;
    }
    let Some(scope) = candidate
        .content
        .lines()
        .find_map(|line| line.strip_prefix("scope="))
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
    else {
        return false;
    };
    request.memory_scopes.is_empty()
        || request
            .memory_scopes
            .iter()
            .any(|allowed| allowed.as_str() == scope)
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
            MemoryLifecycleScope::Workspace => {
                record.scope_owner_ref.as_deref() == self.workspace.as_deref()
            }
            MemoryLifecycleScope::Project => {
                record.scope_owner_ref.as_deref() == self.project.as_deref()
            }
        }
    }
}

async fn runtime_memory_scope(
    state: &Arc<AppState>,
    conversation_owner_id: &str,
    _records: &[MemoryLifecycleRecord],
) -> Result<RuntimeMemoryScope, String> {
    let (workspace_root, project_root) = {
        let config = state.config.lock().await;
        (
            config.system.workspace_memory_root.clone(),
            config.system.project_memory_root.clone(),
        )
    };
    let scoped_ref = |scope, identity: Option<String>| {
        identity
            .map(|identity| {
                memory_scope_owner_ref(scope, &identity).map_err(|error| error.to_string())
            })
            .transpose()
    };
    Ok(RuntimeMemoryScope {
        conversation: memory_scope_owner_ref(
            MemoryLifecycleScope::Conversation,
            conversation_owner_id,
        )
        .map_err(|error| error.to_string())?,
        // Retained pre-reconstruction rows stay visible in Personal
        // Intelligence, but production recall never guesses a canonical
        // Conversation owner through the retired TaskSession store.
        legacy_conversations: HashSet::new(),
        workspace: scoped_ref(MemoryLifecycleScope::Workspace, workspace_root)?,
        project: scoped_ref(MemoryLifecycleScope::Project, project_root)?,
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

// Bounded instruction and user-context surfaces: AGENTS.md, SOUL.md, USER.md,
// memories/USER.md, and skills/<selected>/SKILL.md. Markdown working memory has
// its own explicitly selected Workspace/Project roots below; it must not be
// rediscovered through the process working directory or every knowledge root.
pub(crate) fn load_current_workspace_knowledge_context_candidates(
    selected_skill_id: Option<&str>,
    task_text: &str,
) -> Vec<ContextSourceCandidate> {
    let mut roots = Vec::new();
    if let Ok(workspace) = crate::workspace_file_resolver::resolve_workspace_root() {
        roots.push(KnowledgeContextRoot::new("workspace", workspace));
    }
    if let Some(configured) = configured_knowledge_root() {
        roots.push(KnowledgeContextRoot::new("configured", configured));
    }

    load_knowledge_context_candidates_from_roots(&roots, selected_skill_id, task_text)
}

pub(crate) fn load_configured_knowledge_context_candidates(
    configured_roots: &[String],
    selected_skill_id: Option<&str>,
    task_text: &str,
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

    load_knowledge_context_candidates_from_roots(&roots, selected_skill_id, task_text)
}

#[cfg(test)]
pub(crate) fn load_workspace_knowledge_context_candidates(
    root: &Path,
    selected_skill_id: Option<&str>,
    task_text: &str,
) -> Vec<ContextSourceCandidate> {
    let roots = vec![KnowledgeContextRoot::new("workspace", root)];
    load_knowledge_context_candidates_from_roots(&roots, selected_skill_id, task_text)
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
    _task_text: &str,
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
            "memories/USER.md",
            "bounded user context surface; not canonical truth",
            "private",
            8,
        );
    }

    candidates
}

pub(crate) async fn load_configured_markdown_memory_context_candidates(
    state: &Arc<AppState>,
    task_text: &str,
) -> Result<Vec<ContextSourceCandidate>, String> {
    let (workspace_root, project_root) = {
        let config = state.config.lock().await;
        (
            config.system.workspace_memory_root.clone(),
            config.system.project_memory_root.clone(),
        )
    };
    let roots = crate::markdown_memory::configured_markdown_memory_roots(
        workspace_root.as_deref(),
        project_root.as_deref(),
    );
    let task_text = task_text.to_string();
    tokio::task::spawn_blocking(move || {
        load_markdown_memory_context_candidates_from_roots(&roots, &task_text)
    })
    .await
    .map_err(|error| format!("Markdown memory context worker failed: {error}"))
}

pub(crate) fn load_markdown_memory_context_candidates_from_roots(
    roots: &[crate::markdown_memory::MarkdownMemoryRoot],
    task_text: &str,
) -> Vec<ContextSourceCandidate> {
    let (files, _) = crate::markdown_memory::load_markdown_memory_files(roots);
    let mut ranked = files
        .into_iter()
        .filter_map(|file| {
            select_task_relevant_markdown(&file.content, task_text)
                .map(|selection| (file, selection))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(
        |(left_file, left_selection), (right_file, right_selection)| {
            right_selection
                .relevance_score
                .cmp(&left_selection.relevance_score)
                .then_with(|| {
                    markdown_memory_scope_specificity(right_file.scope)
                        .cmp(&markdown_memory_scope_specificity(left_file.scope))
                })
                .then_with(|| left_file.relative_path.cmp(&right_file.relative_path))
        },
    );

    let mut selected = Vec::new();
    let mut used_chars = 0usize;
    for (file, selection) in ranked {
        let source = format!(
            "markdown-memory:{}:{}",
            file.scope.as_str(),
            file.relative_path
        );
        let remaining =
            crate::markdown_memory::MAX_MARKDOWN_MEMORY_CONTEXT_CHARS.saturating_sub(used_chars);
        if remaining == 0 {
            break;
        }
        let per_file_budget = MAX_CONTEXT_CHARS_PER_FILE.min(remaining);
        let content = selection
            .content
            .chars()
            .take(per_file_budget)
            .collect::<String>();
        used_chars += content.chars().count();
        selected.push(ContextSourceCandidate::new(
            ContextSourceKind::SelectedPersonalContext,
            source,
            content,
            format!(
                "task-relevant {} Markdown working memory; bounded, source-visible, and non-authoritative",
                file.scope.as_str()
            ),
            "private",
            if file.scope == crate::markdown_memory::MarkdownMemoryScope::Project {
                10
            } else {
                8
            },
        ));
        if selected.len() >= crate::markdown_memory::MAX_MARKDOWN_MEMORY_CONTEXT_FILES {
            break;
        }
    }
    selected
}

#[derive(Debug)]
struct MarkdownSectionSelection {
    content: String,
    relevance_score: usize,
}

fn markdown_memory_scope_specificity(scope: crate::markdown_memory::MarkdownMemoryScope) -> u8 {
    match scope {
        crate::markdown_memory::MarkdownMemoryScope::Workspace => 0,
        crate::markdown_memory::MarkdownMemoryScope::Project => 1,
    }
}

fn select_task_relevant_markdown(
    content: &str,
    task_text: &str,
) -> Option<MarkdownSectionSelection> {
    let task = task_text.trim().to_ascii_lowercase();
    if task.is_empty() {
        return None;
    }
    let task_terms = task_relevance_terms(&task);
    let mut sections = Vec::<String>::new();
    let mut current = String::new();
    for line in content.lines() {
        if line.trim_start().starts_with('#') && !current.trim().is_empty() {
            sections.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.trim().is_empty() {
        sections.push(current);
    }

    let mut selected = sections
        .into_iter()
        .filter_map(|section| {
            let lower = section.to_ascii_lowercase();
            let heading = section
                .lines()
                .next()
                .unwrap_or("")
                .trim_start_matches('#')
                .trim();
            let heading_lower = heading.to_ascii_lowercase();
            let direct = !heading_lower.is_empty()
                && (task.contains(&heading_lower) || heading_lower.contains(&task));
            let matches = task_terms
                .iter()
                .filter(|term| lower.contains(term.as_str()))
                .count();
            (direct || matches > 0).then_some((usize::from(direct) * 100 + matches, section))
        })
        .collect::<Vec<_>>();
    selected.sort_by_key(|item| std::cmp::Reverse(item.0));
    let relevance_score = selected.first().map(|(score, _)| *score)?;
    let output = selected
        .into_iter()
        .take(3)
        .map(|(_, section)| section)
        .collect::<Vec<_>>()
        .join("\n\n");
    (!output.trim().is_empty()).then_some(MarkdownSectionSelection {
        content: output,
        relevance_score,
    })
}

fn task_relevance_terms(value: &str) -> Vec<String> {
    let mut terms = value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|term| term.chars().count() >= 3)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut cjk_run = Vec::new();
    let flush_cjk = |run: &mut Vec<char>, terms: &mut Vec<String>| {
        if run.len() >= 2 {
            terms.extend(
                run.windows(2)
                    .map(|window| window.iter().collect::<String>()),
            );
        }
        run.clear();
    };
    for character in value.chars() {
        if is_cjk(character) {
            cjk_run.push(character);
        } else {
            flush_cjk(&mut cjk_run, &mut terms);
        }
    }
    flush_cjk(&mut cjk_run, &mut terms);
    terms.sort();
    terms.dedup();
    terms
}

fn is_cjk(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{faff}'
    )
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

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
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
    fn generic_knowledge_roots_do_not_gain_markdown_memory_authority() {
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
        let candidates = load_knowledge_context_candidates_from_roots(
            &roots,
            Some("summarize"),
            "use this memory context",
        );
        let source_ids = candidates
            .iter()
            .map(|candidate| candidate.source_id.as_str())
            .collect::<Vec<_>>();

        for expected in [
            "AGENTS.md",
            "SOUL.md",
            "USER.md",
            "memories/USER.md",
            "skills/summarize/SKILL.md",
        ] {
            assert!(
                source_ids.contains(&expected),
                "missing bounded knowledge source {expected}"
            );
        }
        assert!(!source_ids.contains(&"MEMORY.md"));
        assert!(!source_ids.contains(&"memories/MEMORY.md"));
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
        let candidates =
            load_knowledge_context_candidates_from_roots(&roots, Some("../summarize"), "task");

        assert!(!candidates
            .iter()
            .any(|candidate| candidate.source_kind == ContextSourceKind::SkillInstruction));
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

    #[test]
    fn bounds_loaded_file_content() {
        let dir = tempfile::tempdir().expect("temp knowledge root");
        std::fs::write(
            dir.path().join("AGENTS.md"),
            "a".repeat(MAX_CONTEXT_CHARS_PER_FILE + 50),
        )
        .expect("agents");

        let roots = vec![KnowledgeContextRoot::new("workspace", dir.path())];
        let candidates = load_knowledge_context_candidates_from_roots(&roots, None, "task");
        let agents = candidates
            .iter()
            .find(|candidate| candidate.source_id == "AGENTS.md")
            .expect("agents candidate");

        assert_eq!(agents.content.chars().count(), MAX_CONTEXT_CHARS_PER_FILE);
    }

    #[test]
    fn markdown_memory_loads_only_task_relevant_evidence_without_control_metadata() {
        let dir = tempfile::tempdir().expect("temp knowledge root");
        std::fs::write(
            dir.path().join("MEMORY.md"),
            "# Roadshow\nUse the verified investor deck.\n\n# Gardening\nBuy tomato seeds.",
        )
        .expect("memory");

        let roots =
            crate::markdown_memory::configured_markdown_memory_roots(dir.path().to_str(), None);
        let candidates = load_markdown_memory_context_candidates_from_roots(
            &roots,
            "Help prepare the roadshow deck",
        );
        let memory = candidates
            .iter()
            .find(|candidate| candidate.source_id == "markdown-memory:workspace:MEMORY.md")
            .expect("task-relevant memory");

        assert!(memory.content.contains("Roadshow"));
        assert!(
            !memory.content.contains("Selection reason"),
            "selection metadata is control data and must not enter the evidence body"
        );
        assert!(memory.inclusion_reason.contains("non-authoritative"));
        assert!(!memory.content.contains("Gardening"));
    }

    #[test]
    fn unrelated_markdown_memory_is_not_injected() {
        let dir = tempfile::tempdir().expect("temp knowledge root");
        std::fs::write(
            dir.path().join("MEMORY.md"),
            "# Gardening\nBuy tomato seeds.",
        )
        .expect("memory");

        let roots =
            crate::markdown_memory::configured_markdown_memory_roots(dir.path().to_str(), None);
        let candidates = load_markdown_memory_context_candidates_from_roots(
            &roots,
            "Prepare the quarterly finance report",
        );

        assert!(!candidates
            .iter()
            .any(|candidate| candidate.source_id.contains("MEMORY.md")));
    }

    #[test]
    fn chinese_task_selects_related_markdown_section_without_loading_unrelated_sections() {
        let dir = tempfile::tempdir().expect("temp knowledge root");
        std::fs::write(
            dir.path().join("MEMORY.md"),
            "# 季度财务\n财务报告需要先核对现金流。\n\n# 园艺\n购买番茄种子。",
        )
        .expect("memory");

        let roots =
            crate::markdown_memory::configured_markdown_memory_roots(dir.path().to_str(), None);
        let candidates =
            load_markdown_memory_context_candidates_from_roots(&roots, "请帮我准备季度财务报告");
        let memory = candidates
            .iter()
            .find(|candidate| candidate.source_id == "markdown-memory:workspace:MEMORY.md")
            .expect("Chinese task-relevant memory");
        assert!(memory.content.contains("现金流"));
        assert!(!memory.content.contains("番茄"));
    }

    #[test]
    fn switching_project_root_cannot_recall_the_previous_project() {
        let project_a = tempfile::tempdir().expect("project a");
        let project_b = tempfile::tempdir().expect("project b");
        std::fs::write(
            project_a.path().join("MEMORY.md"),
            "# Launch\nALPHA_PROJECT_SECRET launch checklist.",
        )
        .unwrap();
        std::fs::write(
            project_b.path().join("MEMORY.md"),
            "# Launch\nBETA_PROJECT_ONLY launch checklist.",
        )
        .unwrap();
        let roots = crate::markdown_memory::configured_markdown_memory_roots(
            None,
            project_b.path().to_str(),
        );
        let candidates =
            load_markdown_memory_context_candidates_from_roots(&roots, "prepare launch checklist");
        let serialized = serde_json::to_string(&candidates).unwrap();

        assert!(serialized.contains("BETA_PROJECT_ONLY"));
        assert!(!serialized.contains("ALPHA_PROJECT_SECRET"));
    }

    #[test]
    fn markdown_memory_runtime_context_has_a_total_and_file_count_budget() {
        let root = tempfile::tempdir().expect("bounded project");
        std::fs::create_dir_all(root.path().join("memories")).unwrap();
        for index in 0..8 {
            std::fs::write(
                root.path().join(format!("memories/topic-{index}.md")),
                format!(
                    "# Shared task {index}\n{}",
                    "bounded shared context ".repeat(100)
                ),
            )
            .unwrap();
        }
        let roots =
            crate::markdown_memory::configured_markdown_memory_roots(None, root.path().to_str());
        let candidates =
            load_markdown_memory_context_candidates_from_roots(&roots, "shared task context");
        let total_chars = candidates
            .iter()
            .map(|candidate| candidate.content.chars().count())
            .sum::<usize>();

        assert!(candidates.len() <= crate::markdown_memory::MAX_MARKDOWN_MEMORY_CONTEXT_FILES);
        assert!(total_chars <= crate::markdown_memory::MAX_MARKDOWN_MEMORY_CONTEXT_CHARS);
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
    async fn project_scope_recall_is_bound_to_the_selected_project_owner() {
        const ALLOWED: &str = "PROJECT_SCOPE_ALLOWED_RELEASE_CHECKLIST";
        const BLOCKED: &str = "PROJECT_SCOPE_BLOCKED_RELEASE_CHECKLIST";
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let project_a = tempfile::tempdir().expect("project a");
        let project_b = tempfile::tempdir().expect("project b");
        let project_a_path = project_a.path().canonicalize().unwrap();
        let project_b_path = project_b.path().canonicalize().unwrap();
        let owner_a = memory_scope_owner_ref(
            MemoryLifecycleScope::Project,
            project_a_path.to_str().unwrap(),
        )
        .unwrap();
        let owner_b = memory_scope_owner_ref(
            MemoryLifecycleScope::Project,
            project_b_path.to_str().unwrap(),
        )
        .unwrap();
        state.config.lock().await.system.project_memory_root =
            Some(project_a_path.to_string_lossy().into_owned());

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
            "conversation-a",
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
        let projected = state
            .memory_store
            .lock()
            .await
            .export_active_memory_records()
            .expect("projected Memory rows");
        let allowed_accesses = projected
            .iter()
            .find(|memory| memory.content == ALLOWED)
            .expect("allowed projection")
            .access_count;
        let blocked_accesses = projected
            .iter()
            .find(|memory| memory.content == BLOCKED)
            .expect("blocked projection")
            .access_count;
        assert!(allowed_accesses > 0);
        assert_eq!(blocked_accesses, 0);
        let vectors = state
            .vector_store
            .lock()
            .await
            .export_all_chunks()
            .expect("projected vectors");
        assert_eq!(
            vectors
                .iter()
                .find(|chunk| chunk.content == BLOCKED)
                .expect("blocked vector projection")
                .access_count,
            0
        );
    }

    #[tokio::test]
    async fn explicit_conversation_only_recall_excludes_broader_applicable_scopes() {
        const GLOBAL: &str = "5.6C GLOBAL SCOPE MUST NOT ENTER CONVERSATION ONLY RECALL";
        const PROJECT: &str = "5.6C PROJECT SCOPE MUST NOT ENTER CONVERSATION ONLY RECALL";
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let project = tempfile::tempdir().expect("selected project");
        let project_path = project.path().canonicalize().unwrap();
        let project_owner = memory_scope_owner_ref(
            MemoryLifecycleScope::Project,
            project_path.to_str().unwrap(),
        )
        .unwrap();
        state.config.lock().await.system.project_memory_root =
            Some(project_path.to_string_lossy().into_owned());

        let global = accept_and_project_memory(
            &state,
            &lifecycle_memory_proposal(GLOBAL, "global", None, Vec::new()),
        )
        .await;
        let project = accept_and_project_memory(
            &state,
            &lifecycle_memory_proposal(PROJECT, "project", Some(&project_owner), Vec::new()),
        )
        .await;

        let candidates = retrievable_lifecycle_context_candidates(
            &state,
            "new-conversation-without-memory",
            "只允许使用当前会话 Agent Memory；5.6C 标记是什么？",
            &[],
        )
        .await
        .expect("exclusive conversation lifecycle retrieval");

        assert!(!candidates
            .iter()
            .any(|candidate| candidate.source_id == global.memory_id));
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.source_id == project.memory_id));
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.source_id.starts_with("memory:")));
    }

    #[tokio::test]
    async fn legacy_task_owned_conversation_memory_is_not_recalled_by_canonical_chat() {
        const SENTINEL: &str = "LEGACY_CONVERSATION_OWNER_SAME_CHAT_ONLY";
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let source_task = {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("task session store")
                .lock()
                .await;
            store
                .create_session(
                    openlife_core::agent::main_chat_agent_v1::AgentTaskSessionDraft {
                        chat_session_id: "chat-a".into(),
                        user_goal: "create historical conversation memory".into(),
                        selected_strategy: openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::MemoryProposal,
                        current_plan_summary: None,
                        context_snapshot_refs: Vec::new(),
                    },
                )
                .expect("source task session")
        };
        let legacy_owner =
            memory_scope_owner_ref(MemoryLifecycleScope::Conversation, &source_task.id).unwrap();
        let proposal =
            lifecycle_memory_proposal(SENTINEL, "conversation", Some(&legacy_owner), Vec::new());
        let accepted = {
            let mut input =
                openlife_core::agent::MemoryLifecycleAcceptanceInput::from_memory_proposal(
                    &proposal,
                    SENTINEL.into(),
                )
                .expect("legacy typed lifecycle proposal");
            input.source_task_session_id = Some(source_task.id.clone());
            let accepted = state
                .memory_lifecycle_store
                .as_ref()
                .expect("lifecycle store")
                .lock()
                .await
                .accept_memory_proposal(input)
                .expect("accept historical task-owned memory")
                .record;
            crate::memory_gateway::reconcile_canonical_outboxes_with_state(&state, 32)
                .await
                .expect("project historical task-owned memory");
            accepted
        };

        let same_chat = retrievable_lifecycle_context_candidates(
            &state,
            "chat-a",
            "LEGACY CONVERSATION OWNER SAME CHAT ONLY",
            &[],
        )
        .await
        .expect("same-chat legacy retrieval");
        assert!(!same_chat
            .iter()
            .any(|candidate| candidate.source_id == accepted.memory_id));

        let foreign_chat = retrievable_lifecycle_context_candidates(
            &state,
            "chat-b",
            "LEGACY CONVERSATION OWNER SAME CHAT ONLY",
            &[],
        )
        .await
        .expect("foreign-chat legacy retrieval");
        assert!(!foreign_chat
            .iter()
            .any(|candidate| candidate.source_id == accepted.memory_id));

        for (content, source_task_id, owner_identity) in [
            (
                "FORGED_LEGACY_CONVERSATION_OWNER_MUST_NOT_RECALL",
                source_task.id.as_str(),
                "different-task-owner",
            ),
            (
                "MISSING_LEGACY_SOURCE_TASK_MUST_NOT_RECALL",
                "missing-source-task",
                "missing-source-task",
            ),
        ] {
            let owner =
                memory_scope_owner_ref(MemoryLifecycleScope::Conversation, owner_identity).unwrap();
            let proposal =
                lifecycle_memory_proposal(content, "conversation", Some(&owner), Vec::new());
            let rejected_id = {
                let mut input =
                    openlife_core::agent::MemoryLifecycleAcceptanceInput::from_memory_proposal(
                        &proposal,
                        content.into(),
                    )
                    .expect("typed invalid historical proposal");
                input.source_task_session_id = Some(source_task_id.into());
                let record = state
                    .memory_lifecycle_store
                    .as_ref()
                    .expect("lifecycle store")
                    .lock()
                    .await
                    .accept_memory_proposal(input)
                    .expect("store historical row before runtime verification")
                    .record;
                crate::memory_gateway::reconcile_canonical_outboxes_with_state(&state, 32)
                    .await
                    .expect("project historical row before runtime verification");
                record.memory_id
            };
            let candidates =
                retrievable_lifecycle_context_candidates(&state, "chat-a", content, &[])
                    .await
                    .expect("invalid legacy row retrieval");
            assert!(!candidates
                .iter()
                .any(|candidate| candidate.source_id == rejected_id));
        }
    }

    #[test]
    fn explicit_memory_scope_filter_requires_exclusive_and_precise_scope_language() {
        assert_eq!(
            explicit_exclusive_memory_scopes(
                "只允许使用当前会话作用域的 Agent Memory；没有依据就回答未知。"
            ),
            Some(vec![MemoryLifecycleScope::Conversation])
        );
        assert_eq!(
            explicit_exclusive_memory_scopes("只使用当前会话 Agent Memory；没有依据就回答未知。"),
            Some(vec![MemoryLifecycleScope::Conversation])
        );
        assert_eq!(
            explicit_exclusive_memory_scopes("仅使用当前工作区 Agent Memory。"),
            Some(vec![MemoryLifecycleScope::Workspace])
        );
        assert_eq!(
            explicit_exclusive_memory_scopes("只使用全局 Agent Memory。"),
            Some(vec![MemoryLifecycleScope::Global])
        );
        assert_eq!(
            explicit_exclusive_memory_scopes("Only use the current project Agent Memory."),
            Some(vec![MemoryLifecycleScope::Project])
        );
        assert_eq!(
            explicit_exclusive_memory_scopes(
                "Only use the workspace scope and project-scoped Agent Memory."
            ),
            Some(vec![
                MemoryLifecycleScope::Workspace,
                MemoryLifecycleScope::Project
            ])
        );
        assert_eq!(
            explicit_exclusive_memory_scopes("当前会话可能有相关记忆。"),
            None
        );
        assert_eq!(explicit_exclusive_memory_scopes("只使用中文回答。"), None);
    }

    #[test]
    fn context_request_requires_an_explicit_agent_memory_source_boundary() {
        let exact = MainChatContextRequest::from_user_text(
            "只允许使用当前会话作用域的 Agent Memory；没有依据就回答未知。",
        );
        assert_eq!(
            exact.task_mode,
            MainChatContextTaskMode::ExactAgentMemoryRead
        );
        assert_eq!(
            exact.memory_scopes,
            vec![MemoryLifecycleScope::Conversation]
        );

        let source_bound =
            MainChatContextRequest::from_user_text("Only use Agent Memory to answer this.");
        assert_eq!(
            source_bound.task_mode,
            MainChatContextTaskMode::EvidenceBoundAgentMemory
        );
        assert!(source_bound.memory_scopes.is_empty());

        assert_eq!(
            MainChatContextRequest::from_user_text("只使用中文回答。").task_mode,
            MainChatContextTaskMode::OpenEnded
        );
        assert_eq!(
            MainChatContextRequest::from_user_text("请参考当前会话继续回答。").task_mode,
            MainChatContextTaskMode::OpenEnded
        );
    }

    #[test]
    fn context_request_uses_the_positively_selected_source_not_negated_exclusions() {
        let markdown = MainChatContextRequest::from_user_text(
            "只允许使用当前已绑定 Project Markdown Memory 回答；不要使用当前对话历史、Agent Memory、LifeModel、文件资料或一般知识。",
        );
        assert_eq!(
            markdown.task_mode,
            MainChatContextTaskMode::EvidenceBoundMarkdown
        );

        let document = MainChatContextRequest::from_user_text(
            "仅使用选中的文档回答；不要使用 Agent Memory、Markdown、LifeModel 或一般知识。",
        );
        assert_eq!(
            document.task_mode,
            MainChatContextTaskMode::EvidenceBoundDocuments
        );

        let agent_memory = MainChatContextRequest::from_user_text(
            "只允许使用当前会话作用域的 Agent Memory 回答；不要使用当前对话历史、Markdown、LifeModel 或一般知识。",
        );
        assert_eq!(
            agent_memory.task_mode,
            MainChatContextTaskMode::ExactAgentMemoryRead
        );
        assert_eq!(
            agent_memory.memory_scopes,
            vec![MemoryLifecycleScope::Conversation]
        );

        let english_markdown = MainChatContextRequest::from_user_text(
            "Only use the selected Markdown memory. Do not use conversation history, Agent Memory, LifeModel, documents, or general knowledge.",
        );
        assert_eq!(
            english_markdown.task_mode,
            MainChatContextTaskMode::EvidenceBoundMarkdown
        );
    }

    #[test]
    fn context_request_extracts_turn_local_facts_without_misclassifying_language_constraints() {
        let request = MainChatContextRequest::from_user_text(
            "请只根据以下三条给定信息写一段四句话的内部说明，不补充未提供的项目事实：已完成核心流程联调；主要问题是回归验证不足；下一步是补足回归并重新验收。不要使用工具，不要执行任何外部或持久写入。",
        );

        assert_eq!(
            request.task_mode,
            MainChatContextTaskMode::EvidenceBoundSources
        );
        assert_eq!(
            request.inline_facts,
            vec![
                MainChatInlineFact {
                    handle: "F1".into(),
                    content: "已完成核心流程联调".into(),
                },
                MainChatInlineFact {
                    handle: "F2".into(),
                    content: "主要问题是回归验证不足".into(),
                },
                MainChatInlineFact {
                    handle: "F3".into(),
                    content: "下一步是补足回归并重新验收".into(),
                },
            ]
        );
        assert!(request.is_source_bound());
        assert!(request.is_inline_fact_bound());
        assert!(!request.is_agent_memory_bound());

        let language_only = MainChatContextRequest::from_user_text("只使用中文回答。");
        assert_eq!(language_only.task_mode, MainChatContextTaskMode::OpenEnded);
        assert!(!language_only.is_source_bound());
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
        state.config.lock().await.system.workspace_memory_root =
            Some(workspace_path.to_string_lossy().into_owned());

        accept_and_project_memory(
            &state,
            &lifecycle_memory_proposal(
                CONFLICTED,
                "workspace",
                Some(&owner),
                vec!["conflict:current-user-correction"],
            ),
        )
        .await;
        accept_and_project_memory(
            &state,
            &lifecycle_memory_proposal(UNBOUND, "workspace", None, Vec::new()),
        )
        .await;

        for query in [CONFLICTED, UNBOUND] {
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
                .any(|candidate| candidate.content.contains(query)));
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
            source_task_session_id: None,
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
