use openlife_core::config::NetworkPolicy;
use openlife_core::llm::{
    ChatMessage, ContextManifest, ProviderInvocationStatus, ProviderPayloadCategory,
    ProviderPayloadPurpose, ProviderPolicyAuthorization,
};
use openlife_core::scheduler::{
    InferenceScheduler, ProviderInvocationProgress, ScheduledInferenceScheduler,
    ScheduledProviderLocalAbortCause,
};
use openlife_core::tasks::{
    ScheduledClaimSettlement, ScheduledTask, ScheduledTaskClaim, TaskStore, TaskStoreAuthorityKey,
};
use std::sync::{Arc, Mutex, MutexGuard};

static OLLAMA_ENV_LOCK: Mutex<()> = Mutex::new(());

struct OllamaEnvGuard {
    _lock: MutexGuard<'static, ()>,
    previous_openlife_base_url: Option<std::ffi::OsString>,
    previous_ollama_host: Option<std::ffi::OsString>,
}

impl OllamaEnvGuard {
    fn install(base_url: &str) -> Self {
        let lock = OLLAMA_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous_openlife_base_url = std::env::var_os("OPENLIFE_OLLAMA_BASE_URL");
        let previous_ollama_host = std::env::var_os("OLLAMA_HOST");
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", base_url);
        std::env::remove_var("OLLAMA_HOST");
        Self {
            _lock: lock,
            previous_openlife_base_url,
            previous_ollama_host,
        }
    }
}

impl Drop for OllamaEnvGuard {
    fn drop(&mut self) {
        match self.previous_openlife_base_url.take() {
            Some(value) => std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", value),
            None => std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL"),
        }
        match self.previous_ollama_host.take() {
            Some(value) => std::env::set_var("OLLAMA_HOST", value),
            None => std::env::remove_var("OLLAMA_HOST"),
        }
    }
}

fn due_task(id: &str) -> ScheduledTask {
    let mut task = ScheduledTask::new(
        "Scheduled review",
        "Prepare a short review",
        Some((chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339()),
        "medium",
    );
    task.id = id.into();
    task.source_proposal_id = Some(format!("proposal-{id}"));
    task.seal_deterministic_local_provider_grant();
    task
}

fn claim_and_begin(store: &TaskStore, id: &str) -> ScheduledTaskClaim {
    store.create_task_idempotent(&due_task(id)).unwrap();
    let claim = store
        .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
        .unwrap()
        .unwrap();
    assert!(store.begin_claim_execution(&claim).unwrap());
    claim
}

fn real_local_scheduler() -> InferenceScheduler {
    InferenceScheduler::new(
        "local-model".into(),
        true,
        "openai".into(),
        "http://127.0.0.1:9/v1".into(),
        "cloud-key-must-not-be-used".into(),
        "unused-cloud-model".into(),
        "unused-embedding".into(),
        false,
    )
}

fn allow_network_policy() -> NetworkPolicy {
    NetworkPolicy {
        default_decision: "allow".into(),
        ..NetworkPolicy::default()
    }
}

async fn prepare_scheduled_request(
    scheduler: &ScheduledInferenceScheduler,
    claim: &ScheduledTaskClaim,
    request_id: &str,
) -> openlife_core::llm::PreparedProviderRequest {
    let messages = vec![ChatMessage {
        role: "user".into(),
        content: claim.task().description.clone(),
    }];
    let authorization = ProviderPolicyAuthorization::from_scheduled_claim(claim)
        .and_then(|authorization| {
            authorization.authorize_derived_payload(
                ProviderPayloadPurpose::ScheduledTaskStep,
                &claim.task().description,
                &messages,
                &[],
            )
        })
        .expect("scheduled claim must authorize its exact compiled payload");
    scheduler
        .prepare_scheduled_chat_request(
            messages,
            Vec::new(),
            ContextManifest {
                request_id: request_id.into(),
                privacy_decision_id: claim.provider_grant().policy_decision_digest.clone(),
                selected_context_refs: Vec::new(),
                included_context_categories: Vec::new(),
                declared_payload_categories: vec![ProviderPayloadCategory::RuntimeCompiledMessages],
                policy_provenance_refs: Vec::new(),
                raw_life_model_included: false,
                raw_unbounded_memory_included: false,
            },
            authorization,
            allow_network_policy(),
            false,
        )
        .await
        .expect("real local scheduled request must prepare")
}

async fn serve_ollama_completed(listener: tokio::net::TcpListener) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for (expected_path, response_body) in [
        (
            "/api/tags",
            r#"{"models":[{"name":"local-model","size":1}]}"#,
        ),
        (
            "/api/chat",
            r#"{"message":{"role":"assistant","content":"local response"},"done":true}"#,
        ),
    ] {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0_u8; 16 * 1024];
        let read = socket.read(&mut request).await.unwrap();
        let request_text = String::from_utf8_lossy(&request[..read]);
        assert!(
            request_text
                .lines()
                .next()
                .is_some_and(|line| line.contains(expected_path)),
            "expected {expected_path}, got {request_text}"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    }
}

async fn serve_ollama_hanging_chat(
    listener: tokio::net::TcpListener,
    chat_seen: Arc<tokio::sync::Notify>,
    release_chat: Arc<tokio::sync::Notify>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (mut tags_socket, _) = listener.accept().await.unwrap();
    let mut request = vec![0_u8; 16 * 1024];
    let read = tags_socket.read(&mut request).await.unwrap();
    assert!(String::from_utf8_lossy(&request[..read]).contains("/api/tags"));
    let tags_body = r#"{"models":[{"name":"local-model","size":1}]}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        tags_body.len(),
        tags_body
    );
    tags_socket.write_all(response.as_bytes()).await.unwrap();

    let (mut chat_socket, _) = listener.accept().await.unwrap();
    let read = chat_socket.read(&mut request).await.unwrap();
    assert!(String::from_utf8_lossy(&request[..read]).contains("/api/chat"));
    chat_seen.notify_one();
    release_chat.notified().await;
}

async fn serve_ollama_and_detect_unexpected_chat(listener: tokio::net::TcpListener) -> bool {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (mut tags_socket, _) = listener.accept().await.unwrap();
    let mut request = vec![0_u8; 16 * 1024];
    let read = tags_socket.read(&mut request).await.unwrap();
    assert!(String::from_utf8_lossy(&request[..read]).contains("/api/tags"));
    let tags_body = r#"{"models":[{"name":"local-model","size":1}]}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        tags_body.len(),
        tags_body
    );
    tags_socket.write_all(response.as_bytes()).await.unwrap();
    drop(tags_socket);

    match tokio::time::timeout(std::time::Duration::from_millis(750), listener.accept()).await {
        Ok(Ok((mut chat_socket, _))) => {
            let read = chat_socket.read(&mut request).await.unwrap_or(0);
            String::from_utf8_lossy(&request[..read]).contains("/api/chat")
        }
        Ok(Err(error)) => {
            panic!("loopback listener failed while proving zero chat requests: {error}")
        }
        Err(_) => false,
    }
}

#[test]
fn scheduler_claim_policy_and_attempt_are_one_durable_fact() {
    let store = TaskStore::new_in_memory().unwrap();
    let claim = claim_and_begin(&store, "integration-policy");
    let attempt = store
        .latest_attempt_for_task(&claim.task().id)
        .unwrap()
        .unwrap();

    assert_eq!(attempt.attempt_id, claim.attempt_id());
    assert_eq!(attempt.status, "executing");
    assert_eq!(attempt.data_route, "local_only");
    assert_eq!(attempt.provider_grant_id, claim.provider_grant().grant_id);
    assert!(!claim.provider_grant().allows_cloud());
}

#[tokio::test(flavor = "current_thread")]
async fn pre_dispatch_failure_reclaims_but_dispatched_timeout_is_quarantined() {
    let store = Arc::new(TaskStore::new_in_memory().unwrap());
    let first = claim_and_begin(&store, "integration-settlement");
    assert_eq!(
        store
            .settle_claim_after_error(
                &first,
                "local_model_unavailable",
                Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            )
            .unwrap(),
        ScheduledClaimSettlement::ReclaimedBeforeDispatch
    );
    assert!(store
        .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
        .unwrap()
        .is_none());

    let second = store
        .claim_next_due(
            chrono::Utc::now() + chrono::Duration::minutes(10),
            chrono::Duration::seconds(30),
        )
        .unwrap()
        .unwrap();
    store.begin_claim_execution(&second).unwrap();
    let second = Arc::new(second);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let _env_guard = OllamaEnvGuard::install(&base_url);
    let chat_seen = Arc::new(tokio::sync::Notify::new());
    let release_chat = Arc::new(tokio::sync::Notify::new());
    let server = tokio::spawn(serve_ollama_hanging_chat(
        listener,
        Arc::clone(&chat_seen),
        Arc::clone(&release_chat),
    ));
    let (scheduler, handle) = real_local_scheduler()
        .bind_scheduled_provider_truth_scope(Arc::clone(&store), Arc::clone(&second))
        .unwrap();
    let prepared = prepare_scheduled_request(&scheduler, &second, "request-unknown").await;
    let mut execution = Box::pin(scheduler.execute_scheduled_provider_request(prepared));
    tokio::select! {
        observed = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            chat_seen.notified(),
        ) => observed.expect("real scheduled request must cross the HTTP edge"),
        outcome = &mut execution => panic!("provider unexpectedly terminated: {:?}", outcome.result),
    }
    let started = store
        .provider_receipts_for_attempt(second.attempt_id())
        .unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].status, "started");
    assert!(started[0].prepared_request_digest.is_some());
    let unknown = handle
        .take_remote_unknown_after_local_abort(ScheduledProviderLocalAbortCause::ExecutionTimeout)
        .unwrap();
    assert_eq!(unknown.len(), 1);
    for admission in unknown {
        store.record_provider_truth(&second, admission).unwrap();
    }
    drop(execution);
    release_chat.notify_one();
    server.await.unwrap();
    assert_eq!(
        store.settle_claim_after_timeout(&second).unwrap(),
        ScheduledClaimSettlement::UnknownRequiresReconciliation
    );
    assert!(store
        .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
        .unwrap()
        .is_none());
    // A timeout and local abort are not reconciliation evidence. This
    // integration test deliberately has no provider query reconciler or
    // native confirmation owner, so the unknown task must remain quarantined.
    assert!(store
        .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
        .unwrap()
        .is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn completion_requires_terminal_provider_receipt_linked_to_attempt() {
    let missing_receipt_store = TaskStore::new_in_memory().unwrap();
    let missing_receipt_claim =
        claim_and_begin(&missing_receipt_store, "integration-receipt-missing");
    let store = Arc::new(TaskStore::new_in_memory().unwrap());
    let claim = Arc::new(claim_and_begin(&store, "integration-receipt"));
    let result_digest = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let missing_result_ref = "conversation://scheduled/integration-receipt-missing/message/1";
    let result_ref = "conversation://scheduled/integration-receipt/message/1";
    missing_receipt_store
        .stage_claim_result_delivery(&missing_receipt_claim, missing_result_ref, result_digest)
        .unwrap();
    assert!(missing_receipt_store
        .complete_claim(
            &missing_receipt_claim,
            "agent-run-integration-missing",
            missing_result_ref,
            result_digest,
        )
        .is_err());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let _env_guard = OllamaEnvGuard::install(&base_url);
    let server = tokio::spawn(serve_ollama_completed(listener));
    let (scheduler, handle) = real_local_scheduler()
        .bind_scheduled_provider_truth_scope(Arc::clone(&store), Arc::clone(&claim))
        .unwrap();
    let prepared = prepare_scheduled_request(&scheduler, &claim, "request-completed").await;
    drop(handle);
    let outcome = scheduler.execute_scheduled_provider_request(prepared).await;
    assert_eq!(outcome.result.as_deref(), Ok("local response"));
    server.await.unwrap();
    assert_eq!(
        outcome.receipt.as_ref().map(|receipt| receipt.status),
        Some(ProviderInvocationStatus::Completed)
    );
    store
        .stage_claim_result_delivery(&claim, result_ref, result_digest)
        .unwrap();
    assert!(store
        .complete_claim(&claim, "agent-run-integration", result_ref, result_digest)
        .unwrap());
    let receipt = store
        .provider_receipts_for_attempt(claim.attempt_id())
        .unwrap()
        .remove(0);
    assert_eq!(receipt.task_id, claim.task().id);
    assert_eq!(receipt.claim_token, claim.claim_token());
    assert_eq!(receipt.provider_grant_id, claim.provider_grant().grant_id);
    assert_eq!(receipt.status, "completed");
    assert!(receipt.prepared_request_digest.is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn local_only_scheduler_policy_rejects_cloud_provider_truth() {
    let store = Arc::new(TaskStore::new_in_memory().unwrap());
    let claim = Arc::new(claim_and_begin(&store, "integration-cloud-reject"));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let _env_guard = OllamaEnvGuard::install(&base_url);
    let server = tokio::spawn(serve_ollama_completed(listener));
    let (scheduler, handle) = real_local_scheduler()
        .bind_scheduled_provider_truth_scope(Arc::clone(&store), Arc::clone(&claim))
        .unwrap();
    let prepared = prepare_scheduled_request(&scheduler, &claim, "request-local-proof").await;
    let caller_shaped_cloud = ProviderInvocationProgress::Started {
        request_id: "request-cloud".into(),
        provider: "openai".into(),
        model: "gpt-test".into(),
        started_at: chrono::Utc::now(),
        policy_evidence: prepared.policy_receipt_evidence(),
    };
    assert!(handle.take_for_progress(&caller_shaped_cloud).is_err());
    assert!(store
        .provider_receipts_for_attempt(claim.attempt_id())
        .unwrap()
        .is_empty());

    let outcome = scheduler.execute_scheduled_provider_request(prepared).await;
    assert_eq!(outcome.result.as_deref(), Ok("local response"));
    server.await.unwrap();
    let receipts = store
        .provider_receipts_for_attempt(claim.attempt_id())
        .unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].status, "completed");
    let ollama_digest = openlife_core::agent::metadata_safe::metadata_safe_text_digest("ollama").1;
    let openai_digest = openlife_core::agent::metadata_safe::metadata_safe_text_digest("openai").1;
    assert_eq!(
        claim.provider_grant().provider_digest.as_deref(),
        Some(ollama_digest.as_str())
    );
    assert_ne!(
        claim.provider_grant().provider_digest.as_deref(),
        Some(openai_digest.as_str())
    );
}

#[tokio::test(flavor = "current_thread")]
async fn provider_start_persistence_failure_prevents_the_real_chat_http_request() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("provider-start-fault.db");
    let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x4d; 32]).unwrap();
    {
        let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        store
            .create_task_idempotent(&due_task("integration-provider-start-fault"))
            .unwrap();
    }
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_provider_start_for_fault_test
             BEFORE INSERT ON scheduler_provider_receipts
             BEGIN
                 SELECT RAISE(ABORT, 'fault-injected provider start persistence failure');
             END;",
        )
        .unwrap();
    }
    let store = Arc::new(TaskStore::new_with_authority_key(&path, &authority_key).unwrap());
    let claim = store
        .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
        .unwrap()
        .unwrap();
    assert!(store.begin_claim_execution(&claim).unwrap());
    let claim = Arc::new(claim);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let _env_guard = OllamaEnvGuard::install(&base_url);
    let server = tokio::spawn(serve_ollama_and_detect_unexpected_chat(listener));
    let (scheduler, _handle) = real_local_scheduler()
        .bind_scheduled_provider_truth_scope(Arc::clone(&store), Arc::clone(&claim))
        .unwrap();
    let prepared =
        prepare_scheduled_request(&scheduler, &claim, "request-start-persistence-fault").await;
    let outcome = scheduler.execute_scheduled_provider_request(prepared).await;

    assert!(outcome.result.is_err());
    assert!(
        !server.await.unwrap(),
        "the real /api/chat edge was crossed after durable provider-start persistence failed"
    );
    assert!(store
        .provider_receipts_for_attempt(claim.attempt_id())
        .unwrap()
        .is_empty());
    assert_eq!(
        store
            .settle_claim_after_error(
                &claim,
                "provider_start_truth_persistence_failed",
                Some("sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",),
            )
            .unwrap(),
        ScheduledClaimSettlement::ReclaimedBeforeDispatch
    );
}

#[tokio::test(flavor = "current_thread")]
async fn generic_scheduler_cannot_dispatch_a_scheduled_request_to_real_http() {
    let store = Arc::new(TaskStore::new_in_memory().unwrap());
    let claim = Arc::new(claim_and_begin(&store, "integration-generic-bypass"));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let _env_guard = OllamaEnvGuard::install(&base_url);
    let server = tokio::spawn(serve_ollama_and_detect_unexpected_chat(listener));
    let generic_scheduler = real_local_scheduler();
    let (scheduled_scheduler, _handle) = generic_scheduler
        .clone()
        .bind_scheduled_provider_truth_scope(Arc::clone(&store), Arc::clone(&claim))
        .unwrap();
    let prepared =
        prepare_scheduled_request(&scheduled_scheduler, &claim, "request-generic-bypass").await;

    let observer_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observer_called_for_execution = Arc::clone(&observer_called);
    let outcome = generic_scheduler
        .execute_prepared_with_observer(prepared, move |_| {
            observer_called_for_execution.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .await;
    assert!(!observer_called.load(std::sync::atomic::Ordering::SeqCst));
    assert!(outcome.receipt.is_none());
    assert!(outcome.terminal_proof.is_none());
    assert!(outcome
        .result
        .unwrap_err()
        .contains("scheduled_policy_requires_scheduled_executor"));
    assert!(
        !server.await.unwrap(),
        "generic execution crossed the real /api/chat edge for scheduled authority"
    );
    assert!(store
        .provider_receipts_for_attempt(claim.attempt_id())
        .unwrap()
        .is_empty());
}

#[test]
fn pre_v13_task_store_quarantines_all_product_truth_without_copying_sensitive_payloads() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("task-store-v4.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE tasks (
            id TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
            due_date TEXT, priority TEXT NOT NULL DEFAULT 'medium',
            status TEXT NOT NULL DEFAULT 'pending', created_at TEXT NOT NULL,
            completed_at TEXT, source_run_id TEXT, source_proposal_id TEXT,
            action_type TEXT NOT NULL DEFAULT 'scheduled_task',
            attempt_count INTEGER NOT NULL DEFAULT 0, claim_token TEXT,
            lease_expires_at TEXT, last_error TEXT, result_digest TEXT, eligible_at TEXT
         );
         -- This fixture intentionally preserves the historical column name so
         -- the migration proves it can ingest an existing pre-v10 database.
         CREATE TABLE scheduler_attempts (
            attempt_id TEXT PRIMARY KEY, task_id TEXT NOT NULL, claim_token TEXT NOT NULL UNIQUE,
            attempt_number INTEGER NOT NULL, status TEXT NOT NULL,
            policy_decision_id TEXT NOT NULL, policy_version TEXT NOT NULL,
            data_route TEXT NOT NULL CHECK(data_route = 'local_only'),
            policy_reason_code TEXT NOT NULL, claimed_at TEXT NOT NULL,
            execution_started_at TEXT, finished_at TEXT, agent_run_ref_digest TEXT,
            error_digest TEXT, reconciliation_evidence_digest TEXT, reconciled_at TEXT,
            UNIQUE(task_id, attempt_number),
            FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
         );
         CREATE TABLE scheduler_provider_receipts (
            request_id TEXT PRIMARY KEY, attempt_id TEXT NOT NULL, task_id TEXT NOT NULL,
            claim_token TEXT NOT NULL, policy_decision_id TEXT NOT NULL,
            provider_digest TEXT NOT NULL, model_digest TEXT NOT NULL,
            status TEXT NOT NULL, started_at TEXT NOT NULL, finished_at TEXT,
            error_digest TEXT, simulated INTEGER,
            FOREIGN KEY(attempt_id) REFERENCES scheduler_attempts(attempt_id) ON DELETE CASCADE,
            FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
         );
         CREATE TABLE scheduler_tool_dispatches (
            dispatch_id TEXT PRIMARY KEY, attempt_id TEXT NOT NULL, task_id TEXT NOT NULL,
            claim_token TEXT NOT NULL, dispatch_index INTEGER NOT NULL,
            manifest_digest TEXT NOT NULL, tool_digest TEXT NOT NULL,
            source_run_ref_digest TEXT, status TEXT NOT NULL,
            observed_at TEXT NOT NULL, finished_at TEXT,
            UNIQUE(attempt_id, dispatch_index),
            FOREIGN KEY(attempt_id) REFERENCES scheduler_attempts(attempt_id) ON DELETE CASCADE,
            FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
         );
         CREATE TABLE openlife_schema_versions (
            component TEXT PRIMARY KEY, version INTEGER NOT NULL, applied_at TEXT NOT NULL
         );",
    )
    .unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tasks (
            id, title, description, due_date, priority, status, created_at,
            source_run_id, source_proposal_id, action_type, attempt_count,
            claim_token, lease_expires_at, eligible_at
         ) VALUES (?1, 'Legacy', 'Legacy scheduled subject', ?2, 'medium', 'running', ?2,
                   'run-v4', 'proposal-v4', 'scheduled_task', 1, 'claim-v4', ?3, ?2)",
        rusqlite::params![
            "task-v4",
            now,
            (chrono::Utc::now() + chrono::Duration::minutes(1)).to_rfc3339()
        ],
    )
    .unwrap();
    conn.execute_batch(&format!(
        "INSERT INTO tasks (
            id, title, description, due_date, priority, status, created_at,
            completed_at, source_run_id, source_proposal_id, action_type,
            attempt_count, result_digest, eligible_at
         ) VALUES
            ('task-v4-completed', 'Legacy completed', 'reported completed', '{now}',
             'medium', 'completed', '{now}', '{now}', 'run-v4-completed',
             'proposal-v4-completed', 'scheduled_task', 1,
             'sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc', NULL),
            ('task-v4-failed', 'Legacy failed', 'reported failed', '{now}',
             'medium', 'failed', '{now}', NULL, 'run-v4-failed',
             'proposal-v4-failed', 'scheduled_task', 1, NULL, NULL);
         INSERT INTO scheduler_attempts (
            attempt_id, task_id, claim_token, attempt_number, status,
            policy_decision_id, policy_version, data_route, policy_reason_code,
            claimed_at, execution_started_at, finished_at, error_digest
         ) VALUES
            ('attempt-v4-completed', 'task-v4-completed', 'claim-v4-completed', 1,
             'completed', 'legacy-decision-completed', 'scheduler-local-only-v1',
             'local_only', 'legacy_completed', '{now}', '{now}', '{now}', NULL),
            ('attempt-v4-failed', 'task-v4-failed', 'claim-v4-failed', 1,
             'failed', 'legacy-decision-failed', 'scheduler-local-only-v1',
             'local_only', 'legacy_failed', '{now}', '{now}', '{now}',
             'sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd');
         INSERT INTO scheduler_provider_receipts (
            request_id, attempt_id, task_id, claim_token, policy_decision_id,
            provider_digest, model_digest, status, started_at, finished_at, error_digest
         ) VALUES
            ('request-v4-completed', 'attempt-v4-completed', 'task-v4-completed',
             'claim-v4-completed', 'legacy-decision-completed',
             'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
             'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
             'completed', '{now}', '{now}', NULL),
            ('request-v4-failed', 'attempt-v4-failed', 'task-v4-failed',
             'claim-v4-failed', 'legacy-decision-failed',
             'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
             'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
             'failed', '{now}', '{now}',
             'sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee');"
    ))
    .unwrap();
    conn.execute(
        "INSERT INTO scheduler_attempts (
            attempt_id, task_id, claim_token, attempt_number, status,
            policy_decision_id, policy_version, data_route, policy_reason_code,
            claimed_at, execution_started_at
         ) VALUES ('attempt-v4', 'task-v4', 'claim-v4', 1, 'executing',
                   'legacy-decision', 'scheduler-local-only-v1', 'local_only',
                   'legacy_reason', ?1, ?1)",
        [&now],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO scheduler_provider_receipts (
            request_id, attempt_id, task_id, claim_token, policy_decision_id,
            provider_digest, model_digest, status, started_at
         ) VALUES ('request-v4', 'attempt-v4', 'task-v4', 'claim-v4',
                   'legacy-decision', ?1, ?2, 'started', ?3)",
        rusqlite::params![
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            now,
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO scheduler_tool_dispatches (
            dispatch_id, attempt_id, task_id, claim_token, dispatch_index,
            manifest_digest, tool_digest, status, observed_at
         ) VALUES ('dispatch-v4', 'attempt-v4', 'task-v4', 'claim-v4', 0,
                   ?1, ?2, 'started', ?3)",
        rusqlite::params![
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            now,
        ],
    )
    .unwrap();
    const LEGACY_STATUS_SECRET_SENTINEL: &str = "OPENLIFE_LEGACY_STATUS_SECRET_SENTINEL";
    let hostile_status = format!(
        "{LEGACY_STATUS_SECRET_SENTINEL}:{}",
        "oversized-status".repeat(1024)
    );
    conn.execute(
        "INSERT INTO tasks (
            id, title, description, due_date, priority, status, created_at,
            source_run_id, source_proposal_id, action_type, attempt_count,
            claim_token, lease_expires_at, eligible_at
         ) VALUES ('task-v4-hostile', 'Hostile legacy status', ?1, ?2, 'medium', ?3, ?2,
                   'run-v4-hostile', 'proposal-v4-hostile', 'scheduled_task', 1,
                   'claim-v4-hostile', ?2, ?2)",
        rusqlite::params![LEGACY_STATUS_SECRET_SENTINEL, now, hostile_status],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO scheduler_attempts (
            attempt_id, task_id, claim_token, attempt_number, status,
            policy_decision_id, policy_version, data_route, policy_reason_code,
            claimed_at, execution_started_at
         ) VALUES ('attempt-v4-hostile', 'task-v4-hostile', 'claim-v4-hostile', 1, ?1,
                   'legacy-decision-hostile', 'scheduler-local-only-v1', 'local_only',
                   'legacy_hostile', ?2, ?2)",
        rusqlite::params![hostile_status, now],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO scheduler_provider_receipts (
            request_id, attempt_id, task_id, claim_token, policy_decision_id,
            provider_digest, model_digest, status, started_at
         ) VALUES ('request-v4-hostile', 'attempt-v4-hostile', 'task-v4-hostile',
                   'claim-v4-hostile', 'legacy-decision-hostile', ?1, ?2, ?3, ?4)",
        rusqlite::params![
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            hostile_status,
            now,
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO scheduler_tool_dispatches (
            dispatch_id, attempt_id, task_id, claim_token, dispatch_index,
            manifest_digest, tool_digest, status, observed_at
         ) VALUES ('dispatch-v4-hostile', 'attempt-v4-hostile', 'task-v4-hostile',
                   'claim-v4-hostile', 0, ?1, ?2, ?3, ?4)",
        rusqlite::params![
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            hostile_status,
            now,
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO openlife_schema_versions VALUES ('task_store', 4, ?1)",
        [&now],
    )
    .unwrap();
    drop(conn);

    let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x5a; 32]).unwrap();
    let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
    assert!(store.list_tasks(None).unwrap().is_empty());
    assert!(store.latest_attempt_for_task("task-v4").unwrap().is_none());
    assert!(store
        .provider_receipts_for_attempt("attempt-v4")
        .unwrap()
        .is_empty());
    drop(store);

    let conn = rusqlite::Connection::open(&path).unwrap();
    for table in [
        "tasks",
        "scheduler_attempts",
        "scheduler_provider_receipts",
        "scheduler_tool_dispatches",
        "scheduler_provider_grant_consumptions",
    ] {
        let row_count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(row_count, 0, "pre-v13 canonical rows survived in {table}");
    }
    let version: i64 = conn
        .query_row(
            "SELECT version FROM openlife_schema_versions WHERE component = 'task_store'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, 15);
    let quarantine = conn
        .prepare(
            "SELECT source_schema_version, task_status, attempt_status,
                    provider_receipt_count, terminal_truth_digest,
                    task_status_source_digest, attempt_status_source_digest,
                    provider_status_digest, tool_dispatch_count, tool_status_digest
             FROM legacy_task_store_truth_quarantine ORDER BY task_status",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(quarantine.len(), 4);
    assert_eq!(
        quarantine
            .iter()
            .map(|row| (row.0, row.1.as_str(), row.2.as_str(), row.3))
            .collect::<Vec<_>>(),
        vec![
            (4, "completed", "completed", 1),
            (4, "failed", "failed", 1),
            (4, "running", "executing", 1),
            (4, "unknown_legacy_status", "unknown_legacy_status", 1,),
        ]
    );
    assert!(quarantine.iter().all(|row| row.4.starts_with("sha256:")));
    assert!(quarantine.iter().all(|row| {
        row.5.starts_with("sha256:")
            && row.6.starts_with("sha256:")
            && row.7.starts_with("sha256:")
            && row.9.starts_with("sha256:")
    }));
    assert_eq!(
        quarantine
            .iter()
            .find(|row| row.1 == "running")
            .map(|row| row.8),
        Some(1),
        "the source tool terminal category was not included in quarantine metadata"
    );
    let attempts_schema: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'scheduler_attempts'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(attempts_schema.contains("'policy_allowed'"));
    let foreign_key_violations: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(foreign_key_violations, 0);
    drop(conn);

    for candidate in [
        path.clone(),
        std::path::PathBuf::from(format!("{}-wal", path.display())),
        std::path::PathBuf::from(format!("{}-shm", path.display())),
    ] {
        if candidate.exists() {
            let bytes = std::fs::read(&candidate).unwrap();
            assert!(
                !bytes
                    .windows(LEGACY_STATUS_SECRET_SENTINEL.len())
                    .any(|window| window == LEGACY_STATUS_SECRET_SENTINEL.as_bytes()),
                "raw legacy status/task payload survived physical retirement in {}",
                candidate.display()
            );
        }
    }

    let reopened = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
    assert!(reopened.list_tasks(None).unwrap().is_empty());
    assert!(reopened
        .provider_receipts_for_attempt("attempt-v4")
        .unwrap()
        .is_empty());
    drop(reopened);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let quarantine_count_after_reopen: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM legacy_task_store_truth_quarantine",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(quarantine_count_after_reopen, 4);
    for table in [
        "tasks",
        "scheduler_attempts",
        "scheduler_provider_receipts",
        "scheduler_tool_dispatches",
        "scheduler_provider_grant_consumptions",
    ] {
        let row_count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(row_count, 0, "reopen reinterpreted quarantine as {table}");
    }
}

#[test]
fn schema_v14_without_purge_marker_recovers_physical_residue_before_open() {
    const PURGE_MARKER: &str = "pre_v13_physical_purge_complete_v1";
    const CRASH_RESIDUE_SENTINEL: &str = "OPENLIFE_COMMITTED_BEFORE_PURGE_SENTINEL";
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("task-store-purge-recovery.sqlite");
    let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x2f; 32]).unwrap();
    drop(TaskStore::new_with_authority_key(&path, &authority_key).unwrap());

    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);
             PRAGMA journal_mode = DELETE;
             PRAGMA secure_delete = OFF;
             DROP TRIGGER IF EXISTS task_store_authority_metadata_immutable_update;
             DROP TRIGGER IF EXISTS task_store_authority_metadata_immutable_delete;",
        )
        .unwrap();
        conn.execute(
            "DELETE FROM task_store_metadata WHERE key = ?1",
            [PURGE_MARKER],
        )
        .unwrap();
        conn.execute_batch(
            "CREATE TABLE crash_residue (payload TEXT NOT NULL);
             BEGIN IMMEDIATE;",
        )
        .unwrap();
        let oversized = format!("{CRASH_RESIDUE_SENTINEL}:{}", "residue".repeat(4096));
        conn.execute(
            "INSERT INTO crash_residue (payload) VALUES (?1)",
            [&oversized],
        )
        .unwrap();
        conn.execute_batch("COMMIT; DROP TABLE crash_residue;")
            .unwrap();
    }
    let before_recovery = std::fs::read(&path).unwrap();
    assert!(
        before_recovery
            .windows(CRASH_RESIDUE_SENTINEL.len())
            .any(|window| window == CRASH_RESIDUE_SENTINEL.as_bytes()),
        "fixture did not reproduce committed physical residue"
    );

    let read_only = TaskStore::open_read_only_existing_with_authority_key(&path, &authority_key)
        .err()
        .expect("read-only open must fail closed while purge marker is absent");
    assert!(read_only
        .to_string()
        .contains("task_store_physical_purge_incomplete"));

    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    }
    let reader = rusqlite::Connection::open(&path).unwrap();
    reader.execute_batch("BEGIN").unwrap();
    let _: i64 = reader
        .query_row(
            "SELECT version FROM openlife_schema_versions WHERE component = 'task_store'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let busy_recovery = TaskStore::new_with_authority_key(&path, &authority_key)
        .err()
        .expect("a reader-blocked WAL truncate must fail closed");
    assert!(busy_recovery
        .to_string()
        .contains("task_store_wal_checkpoint_incomplete"));
    let inspection = rusqlite::Connection::open(&path).unwrap();
    let marker_count: i64 = inspection
        .query_row(
            "SELECT COUNT(*) FROM task_store_metadata WHERE key = ?1",
            [PURGE_MARKER],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        marker_count, 0,
        "a busy checkpoint incorrectly minted the purge-complete marker"
    );
    drop(inspection);
    reader.execute_batch("COMMIT").unwrap();
    drop(reader);

    drop(TaskStore::new_with_authority_key(&path, &authority_key).unwrap());
    let conn = rusqlite::Connection::open(&path).unwrap();
    let marker: String = conn
        .query_row(
            "SELECT value FROM task_store_metadata WHERE key = ?1",
            [PURGE_MARKER],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(marker, "complete");
    drop(conn);
    for candidate in [
        path.clone(),
        std::path::PathBuf::from(format!("{}-wal", path.display())),
        std::path::PathBuf::from(format!("{}-shm", path.display())),
    ] {
        if candidate.exists() {
            let bytes = std::fs::read(&candidate).unwrap();
            assert!(
                !bytes
                    .windows(CRASH_RESIDUE_SENTINEL.len())
                    .any(|window| window == CRASH_RESIDUE_SENTINEL.as_bytes()),
                "marker recovery exposed TaskStore before purging {}",
                candidate.display()
            );
        }
    }

    let reopened = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
    assert!(reopened.list_tasks(None).unwrap().is_empty());
}
