use crate::danger_action_confirmation::{
    require_native_danger_action_confirmation, NativeDangerActionRequest,
};
use crate::errors::AppError;
use crate::memory_gateway;
use crate::AppState;
use std::sync::Arc;
use tauri::State;

async fn lifecycle_record(
    state: &Arc<AppState>,
    memory_id: &str,
) -> Result<openlife_core::agent::MemoryLifecycleRecord, String> {
    if memory_id.trim() != memory_id || !memory_id.starts_with("memory:") || memory_id.len() > 256 {
        return Err("Memory action requires an exact lifecycle memory id".into());
    }
    state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(|| "MemoryLifecycleStore unavailable".to_string())?
        .lock()
        .await
        .get_record(memory_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Memory action target not found".into())
}

#[tauri::command]
pub async fn correct_memory(
    memory_id: String,
    content: String,
    state: State<'_, Arc<AppState>>,
) -> Result<memory_gateway::MemoryCorrectionResult, String> {
    memory_gateway::correct_memory_asset_with_state(memory_id, content, state.inner()).await
}

#[tauri::command]
pub async fn archive_memory(
    memory_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<memory_gateway::MemoryRetrievalMutationResult, AppError> {
    memory_gateway::archive_memory_asset_with_state(memory_id, state.inner()).await
}

#[tauri::command]
pub async fn restore_memory(
    memory_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<memory_gateway::MemoryRetrievalMutationResult, AppError> {
    memory_gateway::restore_memory_asset_with_state(memory_id, state.inner()).await
}

#[tauri::command]
pub async fn privacy_erase_memory_asset(
    memory_id: String,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::agent::MemoryPrivacyEraseReport, String> {
    let record = lifecycle_record(state.inner(), &memory_id).await?;
    if record.content.is_empty() {
        return Err("Memory content is already privacy-erased".into());
    }
    let arguments = serde_json::json!({
        "memory_id": memory_id,
        "operation": "irreversible_privacy_erase",
        "content_digest": openlife_core::persistence_outbox::metadata_digest(&record.content),
    });
    require_native_danger_action_confirmation(
        &window,
        NativeDangerActionRequest {
            action_type: "memory_privacy_erase",
            target_ids_for_new_challenge: std::slice::from_ref(&memory_id),
            requested_target: Some(memory_id.as_str()),
            affected_count: 1,
            arguments: &arguments,
            arguments_summary:
                "永久擦除一条 Memory 的 canonical 正文及其 MemoryStore/Vector 派生内容。",
            scope_summary: "该操作不可恢复；仅保留不含正文的最小 tombstone 与 outbox 审计元数据。",
            challenge_id: None,
        },
    )
    .await
    .map_err(|error| error.to_string())?;
    memory_gateway::privacy_erase_memory_asset_with_state(memory_id, state.inner()).await
}

#[cfg(test)]
pub(crate) async fn create_knowledge_note_with_state(
    operation_id: String,
    session_id: String,
    content: String,
    source: String,
    state: &Arc<AppState>,
) -> Result<crate::memory_gateway::KnowledgeNoteWriteResult, AppError> {
    memory_gateway::create_knowledge_note_with_state(
        operation_id,
        session_id,
        content,
        source,
        state,
    )
    .await
}

#[cfg(test)]
pub(crate) async fn search_memory_with_state(
    query: String,
    top_k: usize,
    state: &Arc<AppState>,
) -> Result<crate::memory_gateway::MemorySearchResult, AppError> {
    memory_gateway::search_memory_with_state(query, top_k, state).await
}

#[cfg(test)]
pub(crate) async fn rebuild_memory_index_with_state(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    let report = memory_gateway::rebuild_memory_index_with_state(state).await?;
    if report.get("status").and_then(serde_json::Value::as_str) == Some("completed") {
        Ok(report)
    } else {
        Err(AppError::external(
            serde_json::json!({
                "operation": "rebuild_memory_index",
                "terminal": false,
                "rebuild": report,
            })
            .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::{
        agent::{
            AgentProposal, MemoryLifecycleAcceptanceInput, MemoryLifecycleStatus, ProposalSource,
            ProposalType, RiskLevel,
        },
        embedding::clear_embedding_cache,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc as StdArc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    static OLLAMA_ENV_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    async fn accepted_test_memory(state: &Arc<AppState>) -> String {
        let mut proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.project",
            serde_json::json!({
                "content": "User prefers written project updates.",
                "scope": "project",
                "category": "preference",
                "candidateKind": "preference",
                "riskLevel": "low",
                "sensitivity": "internal"
            }),
            "reviewed test memory",
            1.0,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        proposal.id = "proposal:accepted-memory-action".into();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        let input = MemoryLifecycleAcceptanceInput::from_memory_proposal(
            &proposal,
            "User prefers written project updates.".into(),
        )
        .unwrap();
        let memory_id = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .accept_memory_proposal(input)
            .unwrap()
            .record
            .memory_id;
        proposal.accept();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .update_proposal(&proposal)
            .unwrap();
        memory_id
    }

    #[tokio::test]
    async fn correction_archive_and_restore_are_direct_reversible_memory_controls() {
        let state = crate::test_utils::test_app_state();
        let memory_id = accepted_test_memory(&state).await;
        let proposal_count_before = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_all_proposals(100, 0)
            .unwrap()
            .len();

        let correction = memory_gateway::correct_memory_asset_with_state(
            memory_id.clone(),
            "User prefers spoken project updates.".into(),
            &state,
        )
        .await
        .unwrap();
        assert!(correction.canonical_committed);
        assert_eq!(correction.replaced_memory_id, memory_id);
        assert!(correction.undo_available);

        let archive =
            memory_gateway::archive_memory_asset_with_state(correction.memory_id.clone(), &state)
                .await
                .unwrap();
        assert!(archive.canonical_committed);
        assert_eq!(archive.disposition, "archived");
        let restore =
            memory_gateway::restore_memory_asset_with_state(correction.memory_id.clone(), &state)
                .await
                .unwrap();
        assert!(restore.canonical_committed);
        assert_eq!(restore.disposition, "active");

        let proposal_count_after = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_all_proposals(100, 0)
            .unwrap()
            .len();
        assert_eq!(proposal_count_after, proposal_count_before);

        let old_record = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_record(&memory_id)
            .unwrap()
            .unwrap();
        assert_eq!(old_record.status, MemoryLifecycleStatus::Superseded);
        assert_eq!(
            old_record.replacement_memory_id.as_deref(),
            Some(correction.memory_id.as_str())
        );
        let replacement = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_record(&correction.memory_id)
            .unwrap()
            .unwrap();
        assert_eq!(replacement.content, "User prefers spoken project updates.");
        assert!(replacement.status.is_runtime_active());
    }

    #[tokio::test]
    async fn privacy_erase_redacts_the_review_payload_and_canonical_memory_body() {
        let state = crate::test_utils::test_app_state();
        let memory_id = accepted_test_memory(&state).await;

        let report =
            memory_gateway::privacy_erase_memory_asset_with_state(memory_id.clone(), &state)
                .await
                .unwrap();

        assert!(report.canonical_committed);
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal("proposal:accepted-memory-action")
            .unwrap()
            .unwrap();
        assert_eq!(proposal.after["privacyErased"], true);
        assert_eq!(proposal.after["memoryId"], memory_id);
        assert!(!serde_json::to_string(&proposal)
            .unwrap()
            .contains("User prefers written project updates."));
        let record = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_record(&memory_id)
            .unwrap()
            .unwrap();
        assert!(record.content.is_empty());
        assert!(record.runtime_context_excluded_at.is_some());
    }

    async fn fake_cloud_embedding_endpoint() -> (String, StdArc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cloud_call_count = StdArc::new(AtomicUsize::new(0));
        let cloud_call_count_clone = cloud_call_count.clone();

        tokio::spawn(async move {
            loop {
                let accepted =
                    tokio::time::timeout(std::time::Duration::from_millis(750), listener.accept())
                        .await;
                let Ok(Ok((mut socket, _))) = accepted else {
                    break;
                };
                cloud_call_count_clone.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 2048];
                let _ = socket.read(&mut buf).await;
                let body = r#"{"data":[{"embedding":[0.1,0.2,0.3]}]}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });

        (format!("http://{}", addr), cloud_call_count)
    }

    async fn fake_hanging_embedding_endpoint() -> (String, StdArc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accepted_count = StdArc::new(AtomicUsize::new(0));
        let accepted_count_clone = accepted_count.clone();
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                accepted_count_clone.fetch_add(1, Ordering::SeqCst);
                let mut buffer = [0_u8; 2048];
                let _ = socket.read(&mut buffer).await;
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        });
        (format!("http://{}", addr), accepted_count)
    }

    async fn configure_cloud_embeddings(state: &Arc<AppState>, openai_base: String) {
        let mut cfg = state.config.lock().await;
        cfg.llm.provider = "openai".to_string();
        cfg.llm.openai_base = openai_base;
        cfg.llm.openai_key = "sk-test".to_string();
        cfg.llm.embedding_model = "text-embedding-3-small".to_string();
        cfg.llm.embedding_enabled = true;
    }

    #[tokio::test]
    async fn create_knowledge_note_sensitive_content_does_not_call_cloud_embedding() {
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        let (openai_base, cloud_call_count) = fake_cloud_embedding_endpoint().await;
        configure_cloud_embeddings(&state, openai_base).await;

        let id = create_knowledge_note_with_state(
            uuid::Uuid::new_v4().to_string(),
            "memory-index-sensitive".to_string(),
            "身份证 11010519491231002X，邮箱 index-sensitive@example.com，最近用药焦虑".to_string(),
            "manual".to_string(),
            &state,
        )
        .await
        .unwrap();

        assert!(id.canonical_committed);
        assert_eq!(
            id.projection_state,
            openlife_core::persistence_outbox::ProjectionDeliveryState::Pending
        );
        assert!(id.embedding_id.is_none());
        assert!(id.embedding_receipt.is_none());
        assert_eq!(cloud_call_count.load(Ordering::SeqCst), 0);

        let background = crate::memory_gateway::reconcile_canonical_outboxes_with_state(&state, 20)
            .await
            .unwrap();
        assert_eq!(background.applied, 1);
        assert_eq!(
            state.vector_store.lock().await.count_all_chunks().unwrap(),
            1
        );
        assert_eq!(cloud_call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn create_knowledge_note_command_replays_one_operation_and_rejects_payload_drift() {
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        state.config.lock().await.llm.embedding_enabled = false;
        let operation_id = uuid::Uuid::new_v4().to_string();

        let first = create_knowledge_note_with_state(
            operation_id.clone(),
            "memory-index-operation".to_string(),
            "COMMAND_OPERATION_SENTINEL".to_string(),
            "manual".to_string(),
            &state,
        )
        .await
        .unwrap();
        let replay = create_knowledge_note_with_state(
            operation_id.clone(),
            "memory-index-operation".to_string(),
            "COMMAND_OPERATION_SENTINEL".to_string(),
            "manual".to_string(),
            &state,
        )
        .await
        .unwrap();

        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(first.operation_id, operation_id);
        assert_eq!(first.knowledge_note_id, replay.knowledge_note_id);
        assert_eq!(first.outbox_event_id, replay.outbox_event_id);
        assert_eq!(
            state
                .memory_store
                .lock()
                .await
                .search_text_memories(None, "COMMAND_OPERATION_SENTINEL", 10)
                .unwrap()
                .len(),
            1
        );

        let drift = create_knowledge_note_with_state(
            operation_id,
            "memory-index-operation".to_string(),
            "DIFFERENT_COMMAND_PAYLOAD".to_string(),
            "manual".to_string(),
            &state,
        )
        .await;
        assert!(
            drift.is_err(),
            "operation id payload drift must fail closed"
        );
        assert!(state
            .memory_store
            .lock()
            .await
            .search_text_memories(None, "DIFFERENT_COMMAND_PAYLOAD", 10)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn create_knowledge_note_commits_canonical_before_failed_vector_projection_and_replays() {
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        state.config.lock().await.llm.embedding_enabled = false;
        state
            .vector_store
            .lock()
            .await
            .install_memory_projection_failure_for_test()
            .unwrap();

        let result = create_knowledge_note_with_state(
            uuid::Uuid::new_v4().to_string(),
            "memory-index-degraded".to_string(),
            "canonical note survives embedding failure".to_string(),
            "manual".to_string(),
            &state,
        )
        .await
        .expect("embedding failure must not erase canonical success");

        assert!(result.canonical_committed);
        assert!(result.embedding_id.is_none());
        assert_eq!(
            result.projection_state,
            openlife_core::persistence_outbox::ProjectionDeliveryState::Pending
        );
        assert!(state
            .memory_store
            .lock()
            .await
            .get_active_memory_record(result.knowledge_note_id)
            .unwrap()
            .is_some());
        assert_eq!(
            state.vector_store.lock().await.count_all_chunks().unwrap(),
            0
        );
        assert_eq!(
            state
                .memory_store
                .lock()
                .await
                .projection_summary(&result.outbox_event_id)
                .unwrap()
                .pending,
            1
        );

        let failed_background =
            crate::memory_gateway::reconcile_canonical_outboxes_with_state(&state, 20)
                .await
                .unwrap();
        assert_eq!(failed_background.degraded, 1);
        assert_eq!(
            state
                .memory_store
                .lock()
                .await
                .projection_summary(&result.outbox_event_id)
                .unwrap()
                .degraded,
            1
        );

        clear_embedding_cache();
        state
            .vector_store
            .lock()
            .await
            .remove_memory_projection_failure_for_test()
            .unwrap();
        let replay = crate::memory_gateway::reconcile_canonical_outboxes_with_state(&state, 20)
            .await
            .unwrap();

        assert_eq!(replay.applied, 1);
        assert_eq!(
            state
                .memory_store
                .lock()
                .await
                .projection_summary(&result.outbox_event_id)
                .unwrap()
                .state(),
            openlife_core::persistence_outbox::ProjectionDeliveryState::Applied
        );
        assert_eq!(
            state.vector_store.lock().await.count_all_chunks().unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn missing_canonical_knowledge_note_can_never_be_marked_projection_applied() {
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        state.config.lock().await.llm.embedding_enabled = false;
        let write = create_knowledge_note_with_state(
            uuid::Uuid::new_v4().to_string(),
            "missing-canonical-knowledge-note".to_string(),
            "MISSING_CANONICAL_ROW_MUST_DEGRADE".to_string(),
            "manual".to_string(),
            &state,
        )
        .await
        .unwrap();
        state
            .memory_store
            .lock()
            .await
            .remove_canonical_memory_row_for_corruption_test(write.knowledge_note_id)
            .unwrap();

        let report = crate::memory_gateway::reconcile_canonical_outboxes_with_state(&state, 20)
            .await
            .unwrap();
        assert_eq!(report.applied, 0);
        assert_eq!(report.degraded, 1);
        assert_eq!(
            state.vector_store.lock().await.count_all_chunks().unwrap(),
            0
        );
        let summary = state
            .memory_store
            .lock()
            .await
            .projection_summary(&write.outbox_event_id)
            .unwrap();
        assert_eq!(
            summary.state(),
            openlife_core::persistence_outbox::ProjectionDeliveryState::Degraded
        );
        assert_eq!(summary.applied, 0);
        assert_eq!(summary.degraded, 1);
    }

    #[tokio::test]
    async fn search_memory_sensitive_query_does_not_call_cloud_embedding() {
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        let (openai_base, cloud_call_count) = fake_cloud_embedding_endpoint().await;
        configure_cloud_embeddings(&state, openai_base).await;

        let hits = search_memory_with_state(
            "银行卡 6222 0202 0202 0202，邮箱 search-sensitive@example.com，健康诊断".to_string(),
            5,
            &state,
        )
        .await
        .unwrap();

        assert!(hits.hits.is_empty());
        assert_eq!(hits.vector_status, "ready");
        assert_eq!(cloud_call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn rebuild_memory_index_uses_only_long_term_memory() {
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        let (openai_base, cloud_call_count) = fake_cloud_embedding_endpoint().await;
        configure_cloud_embeddings(&state, openai_base).await;

        {
            let store = state.memory_store.lock().await;
            store
                .save_knowledge_note_idempotent_with_outbox(
                    &uuid::Uuid::new_v4().to_string(),
                    "rebuild-sensitive",
                    "LONG_TERM_MEMORY_SENTINEL 银行卡 6222 0202 0202 0202，负债和贷款压力",
                    "knowledge_note",
                    "manual",
                    &[
                        "canonical_owner:knowledge_note".into(),
                        "source:manual".into(),
                    ],
                    "private",
                )
                .unwrap();
        }

        let report = rebuild_memory_index_with_state(&state).await.unwrap();
        let vectors = state.vector_store.lock().await.export_all_chunks().unwrap();

        assert_eq!(report["skipped"], 0);
        assert_eq!(report["indexed"], 1);
        assert_eq!(report["processed"], 1);
        assert_eq!(report["status"], "completed");
        assert_eq!(report["providerInvocations"], 0);
        assert!(vectors
            .iter()
            .any(|chunk| chunk.content.contains("LONG_TERM_MEMORY_SENTINEL")));
        assert_eq!(cloud_call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn ordinary_and_sensitive_notes_share_one_local_rebuild_and_query_profile() {
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        let (openai_base, cloud_call_count) = fake_cloud_embedding_endpoint().await;
        configure_cloud_embeddings(&state, openai_base).await;

        for (session_id, content) in [
            (
                "mixed-profile-ordinary",
                "ORDINARY_CANONICAL_NOTE about weekend reading",
            ),
            (
                "mixed-profile-sensitive",
                "SENSITIVE_CANONICAL_NOTE 身份证 11010519491231002X 最近用药焦虑",
            ),
        ] {
            create_knowledge_note_with_state(
                uuid::Uuid::new_v4().to_string(),
                session_id.to_string(),
                content.to_string(),
                "manual".to_string(),
                &state,
            )
            .await
            .unwrap();
        }
        let projected = crate::memory_gateway::reconcile_canonical_outboxes_with_state(&state, 20)
            .await
            .unwrap();
        assert_eq!(projected.applied, 2);
        assert_eq!(cloud_call_count.load(Ordering::SeqCst), 0);

        let rebuilt = rebuild_memory_index_with_state(&state).await.unwrap();
        assert_eq!(rebuilt["status"], "completed");
        assert_eq!(rebuilt["indexed"], 2);
        assert_eq!(rebuilt["skipped"], 0);
        let chunks = state.vector_store.lock().await.export_all_chunks().unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.embedding_profile_id.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len(),
            1
        );

        for sentinel in ["ORDINARY_CANONICAL_NOTE", "SENSITIVE_CANONICAL_NOTE"] {
            let result = search_memory_with_state(sentinel.to_string(), 10, &state)
                .await
                .unwrap();
            assert!(result
                .hits
                .iter()
                .any(|(chunk, _)| chunk.content.contains(sentinel)));
        }
        assert_eq!(cloud_call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn cancelling_a_hung_rebuild_keeps_the_active_projection_unchanged() {
        let _env_guard = OLLAMA_ENV_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        let (ollama_base, accepted_count) = fake_hanging_embedding_endpoint().await;
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", &ollama_base);
        std::env::remove_var("OLLAMA_HOST");
        {
            let mut cfg = state.config.lock().await;
            cfg.llm.provider = "ollama".into();
            cfg.llm.embedding_enabled = true;
            cfg.llm.embedding_model = "nomic-embed-text:latest".into();
        }
        {
            let profile = openlife_core::embedding::EmbeddingProfile::new(
                openlife_core::embedding::EmbeddingRouteKind::DeterministicHash,
                "openlife-test",
                "existing-vector-v1",
                "builtin:test",
                "existing-vector-artifact-v1",
                4,
            )
            .unwrap();
            state
                .vector_store
                .lock()
                .await
                .insert(
                    "old-session",
                    "ACTIVE_VECTOR_BEFORE_CANCEL",
                    &[1.0, 0.0, 0.0, 0.0],
                    &profile,
                    "unowned:old",
                )
                .unwrap();
            state
                .memory_store
                .lock()
                .await
                .save_knowledge_note_idempotent_with_outbox(
                    &uuid::Uuid::new_v4().to_string(),
                    "rebuild-cancel",
                    "Plan a calm weekend with reading and a walk",
                    "knowledge_note",
                    "manual",
                    &[
                        "canonical_owner:knowledge_note".into(),
                        "source:manual".into(),
                    ],
                    "private",
                )
                .unwrap();
        }

        let runner_state = state.clone();
        let runner = tokio::spawn(async move {
            memory_gateway::rebuild_memory_index_with_state(&runner_state).await
        });
        let job = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if let Some(job) =
                    memory_gateway::get_memory_index_rebuild_progress_with_state(&state)
                        .await
                        .unwrap()
                {
                    break job;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("rebuild must publish durable progress before provider completion");
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while accepted_count.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the cancellable embedding must reach the provider edge");
        let requested = memory_gateway::cancel_memory_index_rebuild_with_state(
            Some(job.job_id.clone()),
            &state,
        )
        .await
        .unwrap();
        assert_eq!(
            requested.status,
            openlife_core::vectors::VectorRebuildJobStatus::CancelRequested
        );
        let report = tokio::time::timeout(std::time::Duration::from_secs(2), runner)
            .await
            .expect("local rebuild cancellation must interrupt a hung embedding")
            .unwrap()
            .unwrap();

        assert_eq!(report["status"], "cancelled");
        assert_eq!(report["remoteUnknownProviderAttempts"], 1);
        let active = state.vector_store.lock().await.export_all_chunks().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].content, "ACTIVE_VECTOR_BEFORE_CANCEL");
        assert_eq!(accepted_count.load(Ordering::SeqCst), 1);
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
    }

    #[tokio::test]
    async fn embedding_failure_preserves_text_hits_with_degraded_receipt() {
        let _env_guard = OLLAMA_ENV_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", format!("http://{address}"));
        std::env::remove_var("OLLAMA_HOST");
        state
            .memory_store
            .lock()
            .await
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "degraded-search",
                "DEGRADED_TEXT_HIT_SENTINEL",
                "knowledge_note",
                "test",
                &[
                    "canonical_owner:knowledge_note".into(),
                    "source:test".into(),
                ],
                "private",
            )
            .unwrap();
        {
            let mut cfg = state.config.lock().await;
            cfg.llm.provider = "ollama".into();
            cfg.llm.embedding_enabled = true;
            cfg.llm.embedding_model = "nomic-embed-text:latest".into();
        }
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request).await.unwrap();
            let body = r#"{"error":"ollama unavailable"}"#;
            let response = format!(
                "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let result = search_memory_with_state("DEGRADED_TEXT_HIT_SENTINEL".into(), 5, &state).await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        server.await.unwrap();
        let result = result.unwrap();

        assert!(result
            .hits
            .iter()
            .any(|(chunk, _)| chunk.content == "DEGRADED_TEXT_HIT_SENTINEL"));
        assert_eq!(result.vector_status, "embedding_failed");
        assert_eq!(
            result.embedding_receipt.status,
            openlife_core::embedding::EmbeddingInvocationStatus::Failed
        );
        assert!(result.embedding_receipt.error_digest.is_some());
        let stored = state
            .memory_store
            .lock()
            .await
            .export_active_memory_records()
            .unwrap();
        let searched = stored
            .iter()
            .find(|record| record.content == "DEGRADED_TEXT_HIT_SENTINEL")
            .expect("text search result remains canonical");
        assert_eq!(searched.access_count, 1);
        assert!(searched.last_accessed_at.is_some());
    }

    #[tokio::test]
    async fn ollama_search_verifies_artifact_identity_and_preserves_text_hits() {
        let _env_guard = OLLAMA_ENV_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let digest = format!("sha256:{}", "a".repeat(64));
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", format!("http://{address}"));
        std::env::remove_var("OLLAMA_HOST");
        state
            .memory_store
            .lock()
            .await
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "mutable-profile-search",
                "MUTABLE_PROFILE_TEXT_HIT",
                "knowledge_note",
                "test",
                &[
                    "canonical_owner:knowledge_note".into(),
                    "source:test".into(),
                ],
                "private",
            )
            .unwrap();
        {
            let mut cfg = state.config.lock().await;
            cfg.llm.provider = "ollama".into();
            cfg.llm.embedding_enabled = true;
            cfg.llm.embedding_model = "nomic-embed-text:latest".into();
        }
        let expected_digest = digest.clone();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            let manifest = serde_json::json!({
                "models": [{
                    "name": "nomic-embed-text:latest",
                    "model": "nomic-embed-text:latest",
                    "digest": expected_digest,
                    "size": 1234,
                }]
            })
            .to_string();
            let embedding = serde_json::json!({
                "model": "nomic-embed-text:latest",
                "embeddings": [[0.1, 0.2, 0.3, 0.4]],
            })
            .to_string();
            let mut requests = Vec::new();
            loop {
                let accepted = tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accepted = listener.accept() => accepted,
                };
                let (mut socket, _) = accepted.unwrap();
                let mut request = [0_u8; 4096];
                let read = socket.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..read]).into_owned();
                let body = if request.starts_with("POST /api/embed ") {
                    &embedding
                } else {
                    &manifest
                };
                requests.push(request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });

        let result = search_memory_with_state("MUTABLE_PROFILE_TEXT_HIT".into(), 5, &state).await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        let _ = shutdown_tx.send(());
        let requests = server.await.unwrap();
        let result = result.unwrap();

        assert!(result
            .hits
            .iter()
            .any(|(chunk, _)| chunk.content == "MUTABLE_PROFILE_TEXT_HIT"));
        let embed_index = requests
            .iter()
            .position(|request| request.starts_with("POST /api/embed "))
            .expect("the selected embedding model is invoked");
        assert!(requests[..embed_index]
            .iter()
            .any(|request| request.starts_with("GET /api/tags ")));
        assert!(requests[embed_index + 1..]
            .iter()
            .any(|request| request.starts_with("GET /api/tags ")));
        assert_eq!(result.vector_status, "ready");
        assert_ne!(result.embedding_profile.id, "unknown");
        assert_eq!(result.embedding_profile.model_artifact_identity, digest);
        assert_eq!(
            result.embedding_receipt.status,
            openlife_core::embedding::EmbeddingInvocationStatus::Completed
        );
        assert!(result.degraded_evidence.is_none());
    }
}
