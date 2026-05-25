//! Scheduled task runner: periodically checks scheduled_tasks.json
//! for pending tasks and triggers AgentLoop executions.
//! Runs as a background tokio task spawned during app bootstrap.
//!
//! Concurrency safety: uses short-lock + merge-by-id write-back.
//! Never holds the mutex across `execute_scheduled_task`.
//! Never rewrites the file from a stale in-memory snapshot.

use crate::storage::app_data_dir;
use crate::AppState;
use openlife_core::agent::{AgentTask, AgentTaskKind};
use openlife_core::layer_router::Layer;
use openlife_core::llm::ChatMessage;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

const CHECK_INTERVAL_SECONDS: u64 = 60;
const RUNNING_TASK_STALE_AFTER_SECONDS: i64 = 30 * 60;

pub fn start_scheduler_runner(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let tasks_path = app_data_dir().join("scheduled_tasks.json");
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(CHECK_INTERVAL_SECONDS)).await;

            let now = chrono::Utc::now();
            let claimed =
                match claim_due_scheduled_tasks(&state.scheduled_task_mutex, &tasks_path, &now)
                    .await
                {
                    Ok(t) => t,
                    Err(e) => {
                        log::warn!("[scheduler_runner] Failed to claim due tasks: {}", e);
                        continue;
                    }
                };

            for claimed_task in claimed {
                let task_id = claimed_task
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let title = claimed_task
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("Scheduled Task")
                    .to_string();
                let prompt = claimed_task
                    .get("prompt")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();

                log::info!("[scheduler_runner] Executing scheduled task: {}", title);

                if task_id.is_empty() {
                    log::warn!(
                        "[scheduler_runner] Task '{}' has no id; skipping outcome merge.",
                        title
                    );
                }

                match execute_scheduled_task(&state, &title, &prompt).await {
                    Ok(response) => {
                        log::info!("[scheduler_runner] Task '{}' completed", title);
                        if !task_id.is_empty() {
                            if let Err(e) = merge_scheduled_task_outcome_by_id(
                                &state.scheduled_task_mutex,
                                &tasks_path,
                                &task_id,
                                TaskOutcome::completed(response),
                            )
                            .await
                            {
                                log::warn!(
                                    "[scheduler_runner] Failed to persist completed outcome for task '{}': {}",
                                    task_id,
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("[scheduler_runner] Task '{}' failed: {}", title, e);
                        if !task_id.is_empty() {
                            if let Err(pe) = merge_scheduled_task_outcome_by_id(
                                &state.scheduled_task_mutex,
                                &tasks_path,
                                &task_id,
                                TaskOutcome::failed(e),
                            )
                            .await
                            {
                                log::warn!(
                                    "[scheduler_runner] Failed to persist failed outcome for task '{}': {}",
                                    task_id,
                                    pe
                                );
                            }
                        }
                    }
                }
            }
        }
    });
}

// ── Outcome type for merging results back into the file ──────────

struct TaskOutcome {
    status: String,
    completed_at: Option<String>,
    result_preview: Option<String>,
    agent_run_id: Option<String>,
    error: Option<String>,
}

impl TaskOutcome {
    fn completed(result: ScheduledTaskExecutionResult) -> Self {
        Self {
            status: "completed".to_string(),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
            result_preview: Some(result.result_preview),
            agent_run_id: Some(result.run_id),
            error: None,
        }
    }

    fn failed(err: impl Into<ScheduledTaskExecutionError>) -> Self {
        let err = err.into();
        Self {
            status: "failed".to_string(),
            completed_at: None,
            result_preview: None,
            agent_run_id: err.agent_run_id,
            error: Some(err.message),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScheduledTaskExecutionError {
    message: String,
    agent_run_id: Option<String>,
}

impl ScheduledTaskExecutionError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            agent_run_id: None,
        }
    }

    fn from_facade_error(error: crate::execution_facade::TauriExecutionFacadeError) -> Self {
        let prefix = match error.kind {
            crate::execution_facade::TauriExecutionFacadeErrorKind::Governance => {
                "Scheduled governance failure"
            }
            crate::execution_facade::TauriExecutionFacadeErrorKind::Runtime => {
                "Scheduled runtime failure"
            }
        };
        Self {
            message: format!("{}: {}", prefix, error),
            agent_run_id: error.run_id,
        }
    }
}

impl From<String> for ScheduledTaskExecutionError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for ScheduledTaskExecutionError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

impl std::fmt::Display for ScheduledTaskExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

// ── File I/O helpers (path-parameterized for testability) ───────

fn load_tasks_from_path(path: &Path) -> Result<Vec<Value>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse {:?}: {}", path, e))
}

fn save_tasks_to_path(path: &Path, tasks: &[Value]) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    let content = serde_json::to_string_pretty(tasks)
        .map_err(|e| format!("Failed to serialize tasks: {}", e))?;
    std::fs::write(&temp, &content).map_err(|e| format!("Failed to write temp file: {}", e))?;
    std::fs::rename(&temp, path).map_err(|e| format!("Failed to atomically save tasks: {}", e))?;
    Ok(())
}

// ── Short-lock claim helper ──────────────────────────────────────

fn parse_rfc3339_utc(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

fn task_due_at_or_before(task: &Value, now: chrono::DateTime<chrono::Utc>) -> bool {
    task.get("scheduled_at")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339_utc)
        .is_some_and(|scheduled_at| scheduled_at <= now)
}

fn is_running_task_stale(task: &Value, now: chrono::DateTime<chrono::Utc>) -> bool {
    task.get("running_started_at")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339_utc)
        .is_some_and(|started_at| {
            now.signed_duration_since(started_at)
                >= chrono::Duration::seconds(RUNNING_TASK_STALE_AFTER_SECONDS)
        })
}

fn ensure_task_id(task: &mut Value) {
    if task
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        if let Some(obj) = task.as_object_mut() {
            let generated_id = uuid::Uuid::new_v4().to_string();
            obj.insert("id".to_string(), Value::String(generated_id));
        }
    }
}

/// Acquire the lock, read the latest task list, find pending & due tasks,
/// mark them `running` in the file, release the lock, and return the
/// claimed task copies for execution.
///
/// Tasks without an `id` field are assigned a generated id so they can
/// later be merged by id.
async fn claim_due_scheduled_tasks(
    mutex: &Mutex<()>,
    path: &Path,
    now: &chrono::DateTime<chrono::Utc>,
) -> Result<Vec<Value>, String> {
    let _guard = mutex.lock().await;
    let mut tasks = load_tasks_from_path(path)?;
    let mut claimed = Vec::new();
    let mut dirty = false;
    let now_str = now.to_rfc3339();

    for task in &mut tasks {
        let current_status = task.get("status").and_then(Value::as_str).unwrap_or("");
        match current_status {
            "pending" => {
                if !task_due_at_or_before(task, *now) {
                    continue;
                }

                ensure_task_id(task);

                if let Some(obj) = task.as_object_mut() {
                    obj.insert("status".to_string(), Value::String("running".to_string()));
                    obj.insert(
                        "running_started_at".to_string(),
                        Value::String(now_str.clone()),
                    );
                }
                claimed.push(task.clone());
                dirty = true;
            }
            "running" => {
                if is_running_task_stale(task, *now) {
                    ensure_task_id(task);

                    if let Some(obj) = task.as_object_mut() {
                        obj.insert("status".to_string(), Value::String("running".to_string()));
                        obj.insert(
                            "running_started_at".to_string(),
                            Value::String(now_str.clone()),
                        );
                        obj.insert(
                            "stale_recovered_at".to_string(),
                            Value::String(now_str.clone()),
                        );
                    }

                    claimed.push(task.clone());
                    dirty = true;
                } else {
                    let task_id = task
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>");
                    if task
                        .get("running_started_at")
                        .and_then(Value::as_str)
                        .is_none()
                    {
                        log::warn!(
                            "[scheduler_runner] Running task '{}' has no running_started_at; skipping reclaim.",
                            task_id
                        );
                    }
                }
            }
            _ => {}
        }
    }

    if dirty {
        save_tasks_to_path(path, &tasks)?;
    }

    Ok(claimed)
}

// ── Short-lock merge-by-id helper ────────────────────────────────

/// Acquire the lock, read the latest task list, find the task by `id`,
/// update only that task's fields, write back.
///
/// New tasks that arrived since the claim step are preserved.
/// If the target id is not found, this is a no-op (logged as info).
async fn merge_scheduled_task_outcome_by_id(
    mutex: &Mutex<()>,
    path: &Path,
    task_id: &str,
    outcome: TaskOutcome,
) -> Result<(), String> {
    let _guard = mutex.lock().await;
    let mut tasks = load_tasks_from_path(path)?;
    let mut found = false;

    for task in &mut tasks {
        let id = task.get("id").and_then(Value::as_str).unwrap_or("");
        if id == task_id {
            if let Some(obj) = task.as_object_mut() {
                obj.insert("status".to_string(), Value::String(outcome.status));
                if let Some(at) = outcome.completed_at {
                    obj.insert("completed_at".to_string(), Value::String(at));
                }
                if let Some(preview) = outcome.result_preview {
                    obj.insert("result_preview".to_string(), Value::String(preview));
                }
                if let Some(run_id) = outcome.agent_run_id {
                    obj.insert("agent_run_id".to_string(), Value::String(run_id));
                }
                if let Some(err) = outcome.error {
                    obj.insert("error".to_string(), Value::String(err));
                }
            }
            found = true;
            break;
        }
    }

    if !found {
        log::info!(
            "[scheduler_runner] Task id '{}' not found in latest file; may have been removed.",
            task_id
        );
    }

    save_tasks_to_path(path, &tasks)
}

// ── Task execution (unchanged file-state ownership, no lock held) ─

#[derive(Debug)]
struct ScheduledTaskExecutionResult {
    result_preview: String,
    run_id: String,
}

async fn execute_scheduled_task(
    state: &Arc<AppState>,
    _title: &str,
    prompt: &str,
) -> Result<ScheduledTaskExecutionResult, ScheduledTaskExecutionError> {
    let cfg = state.config.lock().await.clone();
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| {
            ScheduledTaskExecutionError::new(format!("LifeModel load failed: {}", e))
        })?
    };
    let scheduler = state.scheduler.lock().await.clone();
    let privacy_engine = state.privacy_engine.lock().await.clone();
    let task = AgentTask {
        kind: AgentTaskKind::Proactive,
        session_id: format!("scheduled-{}", uuid::Uuid::new_v4()),
        user_text: prompt.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        }],
        layer: Layer::L2,
        ..Default::default()
    };

    let agent_spec = match crate::commands::agent_spec::resolve_required_agent_spec(
        &state.agent_spec_store,
        None,
    )
    .await
    {
        Ok(spec) => spec,
        Err(e) => {
            log::error!(
                "[scheduler_runner] AgentSpec resolution failed: {}. Scheduled task execution aborted (fail-closed).",
                e
            );
            return Err(ScheduledTaskExecutionError::new(format!(
                "AgentSpec resolution failed: {}. Scheduled execution requires a valid active main AgentSpec.",
                e
            )));
        }
    };
    let prompt_registry = crate::execution_facade::build_prompt_registry();
    let network_policy = cfg.system.network_policy.clone();

    let outcome = crate::execution_facade::run_tauri_scheduled_execution(
        crate::execution_facade::TauriScheduledExecutionInput {
            task,
            app_state: state.clone(),
            config: cfg,
            life_model,
            scheduler,
            privacy_engine,
            agent_spec: Some(agent_spec),
            network_policy: Some(network_policy),
            prompt_registry: Some(prompt_registry),
        },
    )
    .await
    .map_err(ScheduledTaskExecutionError::from_facade_error)?;

    Ok(ScheduledTaskExecutionResult {
        result_preview: outcome.result_preview,
        run_id: outcome.run_id,
    })
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use tokio::sync::Mutex;

    fn temp_tasks_path(dir: &std::path::Path) -> PathBuf {
        dir.join("test_scheduled_tasks.json")
    }

    fn write_tasks_json(path: &Path, tasks: &[Value]) {
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir).unwrap();
        let content = serde_json::to_string_pretty(tasks).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn read_tasks_json(path: &Path) -> Vec<Value> {
        if !path.exists() {
            return vec![];
        }
        let content = std::fs::read_to_string(path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    fn completed_result(preview: &str) -> ScheduledTaskExecutionResult {
        ScheduledTaskExecutionResult {
            result_preview: preview.to_string(),
            run_id: format!("run-{}", preview.replace(' ', "-")),
        }
    }

    fn now_dt() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }

    fn seconds_ago_str(seconds: i64) -> String {
        (chrono::Utc::now() - chrono::Duration::seconds(seconds)).to_rfc3339()
    }

    fn past_str() -> String {
        (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339()
    }

    fn future_str() -> String {
        (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339()
    }

    #[tokio::test]
    async fn scheduler_claim_due_marks_running_and_preserves_new_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        // Write an initial file with one due pending task
        let task_a = json!({
            "id": "task-A",
            "title": "Task A",
            "prompt": "do A",
            "scheduled_at": past_str(),
            "status": "pending"
        });
        write_tasks_json(&path, &[task_a.clone()]);

        // Claim => task-A should be marked running in file
        let now = now_dt();
        let claimed = claim_due_scheduled_tasks(&mutex, &path, &now)
            .await
            .unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0]["id"], "task-A");

        // File should now have task-A as "running"
        let tasks_after_claim = read_tasks_json(&path);
        assert_eq!(tasks_after_claim.len(), 1);
        assert_eq!(tasks_after_claim[0]["status"], "running");
        assert_eq!(tasks_after_claim[0]["id"], "task-A");

        // ---------------
        // Simulate a concurrent write: new task B arrives after claim
        // but before the runner merges outcomes.
        let task_b = json!({
            "id": "task-B",
            "title": "Task B",
            "prompt": "do B",
            "scheduled_at": past_str(),
            "status": "pending"
        });
        let _guard = mutex.lock().await;
        let mut latest = load_tasks_from_path(&path).unwrap();
        latest.push(task_b);
        save_tasks_to_path(&path, &latest).unwrap();
        drop(_guard);

        // File now has task-A (running) + task-B (pending)
        let tasks_after_concurrent = read_tasks_json(&path);
        assert_eq!(tasks_after_concurrent.len(), 2);

        // Now merge task-A's outcome => only task-A should change, task-B stays
        merge_scheduled_task_outcome_by_id(
            &mutex,
            &path,
            "task-A",
            TaskOutcome::completed(completed_result("done A")),
        )
        .await
        .unwrap();

        let tasks_final = read_tasks_json(&path);
        assert_eq!(
            tasks_final.len(),
            2,
            "task-B must still exist, no overwrite"
        );

        let a = tasks_final.iter().find(|t| t["id"] == "task-A").unwrap();
        assert_eq!(a["status"], "completed");
        assert!(a.get("completed_at").is_some());

        let b = tasks_final.iter().find(|t| t["id"] == "task-B").unwrap();
        assert_eq!(b["status"], "pending");
        assert!(
            tasks_final.iter().any(|t| t["id"] == "task-B"),
            "task B must be preserved, not overwritten by old snapshot"
        );
    }

    #[tokio::test]
    async fn scheduler_lease_is_short_and_complete_merges_interleaved_pending_task() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        write_tasks_json(
            &path,
            &[json!({
                "id": "long-model-task",
                "title": "Long Model Task",
                "prompt": "run the model",
                "scheduled_at": past_str(),
                "status": "pending"
            })],
        );

        let now = now_dt();
        let claimed = claim_due_scheduled_tasks(&mutex, &path, &now)
            .await
            .unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0]["id"], "long-model-task");

        {
            let _guard = mutex.lock().await;
            let mut latest = load_tasks_from_path(&path).unwrap();
            latest.push(json!({
                "id": "interleaved-task",
                "title": "Interleaved Task",
                "prompt": "arrived while model was running",
                "scheduled_at": past_str(),
                "status": "pending"
            }));
            save_tasks_to_path(&path, &latest).unwrap();
        }

        merge_scheduled_task_outcome_by_id(
            &mutex,
            &path,
            "long-model-task",
            TaskOutcome::completed(completed_result("model completed")),
        )
        .await
        .unwrap();

        let final_tasks = read_tasks_json(&path);
        let completed = final_tasks
            .iter()
            .find(|t| t["id"] == "long-model-task")
            .unwrap();
        let interleaved = final_tasks
            .iter()
            .find(|t| t["id"] == "interleaved-task")
            .unwrap();

        assert_eq!(completed["status"], "completed");
        assert_eq!(
            interleaved["status"], "pending",
            "a task appended during execution must survive outcome merge"
        );
    }

    #[tokio::test]
    async fn scheduler_complete_task_merges_latest_file_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        // File initially has task-A (running) and task-B (pending)
        let tasks = vec![
            json!({
                "id": "task-A",
                "title": "Task A",
                "prompt": "do A",
                "scheduled_at": past_str(),
                "status": "running"
            }),
            json!({
                "id": "task-B",
                "title": "Task B",
                "prompt": "do B",
                "scheduled_at": past_str(),
                "status": "pending"
            }),
        ];
        write_tasks_json(&path, &tasks);

        // Complete task A
        merge_scheduled_task_outcome_by_id(
            &mutex,
            &path,
            "task-A",
            TaskOutcome::completed(completed_result("A result")),
        )
        .await
        .unwrap();

        let result = read_tasks_json(&path);
        assert_eq!(result.len(), 2, "both tasks must remain");

        let a = result.iter().find(|t| t["id"] == "task-A").unwrap();
        assert_eq!(a["status"], "completed");
        assert!(a.get("completed_at").is_some());
        assert_eq!(a["result_preview"], "A result");
        assert_eq!(a["agent_run_id"], "run-A-result");

        let b = result.iter().find(|t| t["id"] == "task-B").unwrap();
        assert_eq!(b["status"], "pending");
    }

    #[tokio::test]
    async fn scheduled_successful_outcome_records_facade_preview_and_run_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        write_tasks_json(
            &path,
            &[json!({
                "id": "successful-scheduled-task",
                "title": "Successful Scheduled Task",
                "prompt": "summarize",
                "scheduled_at": past_str(),
                "status": "running"
            })],
        );

        merge_scheduled_task_outcome_by_id(
            &mutex,
            &path,
            "successful-scheduled-task",
            TaskOutcome::completed(ScheduledTaskExecutionResult {
                result_preview: "facade preview".into(),
                run_id: "scheduled-run-123".into(),
            }),
        )
        .await
        .unwrap();

        let tasks = read_tasks_json(&path);
        let task = tasks
            .iter()
            .find(|t| t["id"] == "successful-scheduled-task")
            .unwrap();
        assert_eq!(task["status"], "completed");
        assert_eq!(task["result_preview"], "facade preview");
        assert_eq!(task["agent_run_id"], "scheduled-run-123");
        assert!(task.get("completed_at").is_some());
        assert!(task.get("error").is_none());
    }

    #[tokio::test]
    async fn scheduler_failed_task_merges_latest_file_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        let tasks = vec![
            json!({
                "id": "task-X",
                "title": "Task X",
                "prompt": "do X",
                "scheduled_at": past_str(),
                "status": "running"
            }),
            json!({
                "id": "task-Y",
                "title": "Task Y",
                "prompt": "do Y",
                "scheduled_at": past_str(),
                "status": "pending"
            }),
        ];
        write_tasks_json(&path, &tasks);

        merge_scheduled_task_outcome_by_id(
            &mutex,
            &path,
            "task-X",
            TaskOutcome::failed("exec error"),
        )
        .await
        .unwrap();

        let result = read_tasks_json(&path);
        assert_eq!(result.len(), 2);

        let x = result.iter().find(|t| t["id"] == "task-X").unwrap();
        assert_eq!(x["status"], "failed");
        assert_eq!(x["error"], "exec error");

        let y = result.iter().find(|t| t["id"] == "task-Y").unwrap();
        assert_eq!(y["status"], "pending");
    }

    #[tokio::test]
    async fn scheduled_failure_observability_records_readable_error_without_completion() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        write_tasks_json(
            &path,
            &[json!({
                "id": "observable-failure",
                "title": "Observable Failure",
                "prompt": "fail clearly",
                "scheduled_at": past_str(),
                "status": "running"
            })],
        );

        merge_scheduled_task_outcome_by_id(
            &mutex,
            &path,
            "observable-failure",
            TaskOutcome::failed("AgentSpec resolution failed: no active main AgentSpec found"),
        )
        .await
        .unwrap();

        let result = read_tasks_json(&path);
        let failed = result
            .iter()
            .find(|t| t["id"] == "observable-failure")
            .unwrap();
        assert_eq!(failed["status"], "failed");
        assert!(failed["error"]
            .as_str()
            .unwrap()
            .contains("AgentSpec resolution failed"));
        assert!(
            failed.get("completed_at").is_none(),
            "failed scheduled tasks must not be marked completed"
        );
        assert!(
            failed.get("result_preview").is_none(),
            "failed scheduled tasks must not receive success output"
        );
    }

    #[tokio::test]
    async fn scheduler_missing_id_task_does_not_overwrite_unrelated() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        // Create a file with a task that has no id
        let tasks = vec![
            json!({
                "title": "NoIdTask",
                "prompt": "do something",
                "scheduled_at": past_str(),
                "status": "pending"
            }),
            json!({
                "id": "safe-task",
                "title": "Safe Task",
                "prompt": "safe",
                "scheduled_at": past_str(),
                "status": "pending"
            }),
        ];
        write_tasks_json(&path, &tasks);

        let now = now_dt();
        let claimed = claim_due_scheduled_tasks(&mutex, &path, &now)
            .await
            .unwrap();

        // Both should be claimed (the no-id one gets a generated id)
        assert_eq!(claimed.len(), 2);

        // Verify the file was updated: both are now "running"
        let after_claim = read_tasks_json(&path);
        assert_eq!(after_claim.len(), 2);
        for t in &after_claim {
            assert_eq!(t["status"], "running");
            // The no-id task should now have a generated id
            assert!(!t["id"].as_str().unwrap_or("").is_empty());
        }

        // Now merge an outcome for "safe-task" only.
        // The no-id task should remain untouched.
        merge_scheduled_task_outcome_by_id(
            &mutex,
            &path,
            "safe-task",
            TaskOutcome::completed(completed_result("safe done")),
        )
        .await
        .unwrap();

        let final_tasks = read_tasks_json(&path);
        assert_eq!(final_tasks.len(), 2);

        let safe = final_tasks.iter().find(|t| t["id"] == "safe-task").unwrap();
        assert_eq!(safe["status"], "completed");

        // The no-id task (now with a generated id) should still be "running"
        let noid = final_tasks.iter().find(|t| t["id"] != "safe-task").unwrap();
        assert_eq!(noid["status"], "running");
    }

    #[tokio::test]
    async fn scheduler_claim_skips_future_and_non_pending() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        let tasks = vec![
            json!({
                "id": "future-task",
                "title": "Future Task",
                "prompt": "future",
                "scheduled_at": future_str(),
                "status": "pending"
            }),
            json!({
                "id": "already-completed",
                "title": "Completed Task",
                "prompt": "done",
                "scheduled_at": past_str(),
                "status": "completed"
            }),
            json!({
                "id": "already-failed",
                "title": "Failed Task",
                "prompt": "fail",
                "scheduled_at": past_str(),
                "status": "failed"
            }),
            json!({
                "id": "due-task",
                "title": "Due Task",
                "prompt": "due",
                "scheduled_at": past_str(),
                "status": "pending"
            }),
        ];
        write_tasks_json(&path, &tasks);

        let now = now_dt();
        let claimed = claim_due_scheduled_tasks(&mutex, &path, &now)
            .await
            .unwrap();

        assert_eq!(
            claimed.len(),
            1,
            "only the due pending task should be claimed"
        );
        assert_eq!(claimed[0]["id"], "due-task");

        let after = read_tasks_json(&path);
        assert_eq!(after.len(), 4);

        let due = after.iter().find(|t| t["id"] == "due-task").unwrap();
        assert_eq!(due["status"], "running");

        // Others unchanged
        let future = after.iter().find(|t| t["id"] == "future-task").unwrap();
        assert_eq!(future["status"], "pending");

        let completed = after
            .iter()
            .find(|t| t["id"] == "already-completed")
            .unwrap();
        assert_eq!(completed["status"], "completed");

        let failed = after.iter().find(|t| t["id"] == "already-failed").unwrap();
        assert_eq!(failed["status"], "failed");
    }

    #[tokio::test]
    async fn scheduler_completed_and_failed_tasks_are_not_reclaimed_even_if_old() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());
        let old_started_at = seconds_ago_str(RUNNING_TASK_STALE_AFTER_SECONDS + 600);

        let tasks = vec![
            json!({
                "id": "old-completed",
                "title": "Old Completed Task",
                "prompt": "done",
                "scheduled_at": past_str(),
                "status": "completed",
                "running_started_at": old_started_at,
                "completed_at": past_str()
            }),
            json!({
                "id": "old-failed",
                "title": "Old Failed Task",
                "prompt": "failed",
                "scheduled_at": past_str(),
                "status": "failed",
                "running_started_at": seconds_ago_str(RUNNING_TASK_STALE_AFTER_SECONDS + 600),
                "error": "previous failure"
            }),
        ];
        write_tasks_json(&path, &tasks);

        let before = std::fs::read_to_string(&path).unwrap();
        let now = now_dt();
        let claimed = claim_due_scheduled_tasks(&mutex, &path, &now)
            .await
            .unwrap();

        assert!(
            claimed.is_empty(),
            "terminal scheduled tasks must never be reclaimed"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    #[tokio::test]
    async fn scheduler_active_running_task_is_not_reclaimed() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        let tasks = vec![json!({
            "id": "active-running",
            "title": "Active Running Task",
            "prompt": "active",
            "scheduled_at": past_str(),
            "status": "running",
            "running_started_at": seconds_ago_str(5)
        })];
        write_tasks_json(&path, &tasks);

        let now = now_dt();
        let claimed = claim_due_scheduled_tasks(&mutex, &path, &now)
            .await
            .unwrap();

        assert_eq!(
            claimed.len(),
            0,
            "active running task must not be reclaimed"
        );

        let after = read_tasks_json(&path);
        assert_eq!(after.len(), 1);
        assert_eq!(after[0]["status"], "running");
        assert_eq!(after[0]["id"], "active-running");
    }

    #[tokio::test]
    async fn scheduler_stale_running_task_is_reclaimed_after_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        let old_started_at = seconds_ago_str(RUNNING_TASK_STALE_AFTER_SECONDS + 60);
        let tasks = vec![json!({
            "id": "stale-running",
            "title": "Stale Running Task",
            "prompt": "stale",
            "scheduled_at": past_str(),
            "status": "running",
            "running_started_at": old_started_at
        })];
        write_tasks_json(&path, &tasks);

        let now = now_dt();
        let claimed = claim_due_scheduled_tasks(&mutex, &path, &now)
            .await
            .unwrap();

        assert_eq!(claimed.len(), 1, "stale running task should be reclaimed");
        assert_eq!(claimed[0]["id"], "stale-running");

        let after = read_tasks_json(&path);
        assert_eq!(after.len(), 1);
        assert_eq!(after[0]["status"], "running");
        assert_eq!(after[0]["running_started_at"], now.to_rfc3339());
        assert_eq!(after[0]["stale_recovered_at"], now.to_rfc3339());
    }

    #[tokio::test]
    async fn scheduler_running_without_started_at_is_not_reclaimed() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        let tasks = vec![json!({
            "id": "legacy-running",
            "title": "Legacy Running Task",
            "prompt": "legacy",
            "scheduled_at": past_str(),
            "status": "running"
        })];
        write_tasks_json(&path, &tasks);

        let before = std::fs::read_to_string(&path).unwrap();
        let now = now_dt();
        let claimed = claim_due_scheduled_tasks(&mutex, &path, &now)
            .await
            .unwrap();

        assert_eq!(
            claimed.len(),
            0,
            "running task without running_started_at must fail safe"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    #[tokio::test]
    async fn scheduler_pending_claim_writes_running_started_at() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        let tasks = vec![json!({
            "id": "pending-due",
            "title": "Pending Due Task",
            "prompt": "pending",
            "scheduled_at": past_str(),
            "status": "pending"
        })];
        write_tasks_json(&path, &tasks);

        let now = now_dt();
        let claimed = claim_due_scheduled_tasks(&mutex, &path, &now)
            .await
            .unwrap();

        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0]["id"], "pending-due");

        let after = read_tasks_json(&path);
        assert_eq!(after.len(), 1);
        assert_eq!(after[0]["status"], "running");
        assert_eq!(after[0]["running_started_at"], now.to_rfc3339());
        assert!(chrono::DateTime::parse_from_rfc3339(
            after[0]["running_started_at"].as_str().unwrap()
        )
        .is_ok());
    }

    #[tokio::test]
    async fn scheduler_merge_nonexistent_id_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        let tasks = vec![json!({
            "id": "task-real",
            "title": "Real Task",
            "prompt": "real",
            "scheduled_at": past_str(),
            "status": "running"
        })];
        write_tasks_json(&path, &tasks);

        // Merge for a task that doesn't exist
        let result = merge_scheduled_task_outcome_by_id(
            &mutex,
            &path,
            "nonexistent-id",
            TaskOutcome::completed(completed_result("ghost")),
        )
        .await;
        assert!(result.is_ok(), "noop should not error");

        let after = read_tasks_json(&path);
        assert_eq!(after.len(), 1);
        assert_eq!(after[0]["id"], "task-real");
        assert_eq!(after[0]["status"], "running");
    }

    #[tokio::test]
    async fn scheduler_apply_and_claim_interleaving_safety() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Arc::new(Mutex::new(()));

        // Initial: task-A pending due
        let task_a = json!({
            "id": "task-A",
            "title": "Task A",
            "prompt": "do A",
            "scheduled_at": past_str(),
            "status": "pending"
        });
        write_tasks_json(&path, &[task_a]);

        let now = now_dt();

        // Claim task-A (marks running, writes back)
        let m1 = mutex.clone();
        let p1 = path.clone();
        let n1 = now;
        let h1 =
            tokio::spawn(async move { claim_due_scheduled_tasks(&m1, &p1, &n1).await.unwrap() });

        // Concurrently, simulate apply_scheduled_task adding task-B
        let m2 = mutex.clone();
        let p2 = path.clone();
        let h2 = tokio::spawn(async move {
            let _guard = m2.lock().await;
            let mut tasks = load_tasks_from_path(&p2).unwrap();
            tasks.push(json!({
                "id": "task-B",
                "title": "Task B",
                "prompt": "do B",
                "scheduled_at": past_str(),
                "status": "pending"
            }));
            save_tasks_to_path(&p2, &tasks).unwrap();
        });

        let claimed = h1.await.unwrap();
        h2.await.unwrap();

        // Merge task-A outcome
        merge_scheduled_task_outcome_by_id(
            &mutex,
            &path,
            "task-A",
            TaskOutcome::completed(completed_result("done")),
        )
        .await
        .unwrap();

        let final_tasks = read_tasks_json(&path);
        assert_eq!(
            final_tasks.len(),
            2,
            "both tasks must exist: task-A completed + task-B"
        );

        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0]["id"], "task-A");

        let a = final_tasks.iter().find(|t| t["id"] == "task-A").unwrap();
        assert_eq!(a["status"], "completed");

        let b = final_tasks.iter().find(|t| t["id"] == "task-B").unwrap();
        assert_eq!(b["status"], "pending");
    }

    #[tokio::test]
    async fn scheduler_empty_file_claim_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());
        let now = now_dt();

        let claimed = claim_due_scheduled_tasks(&mutex, &path, &now)
            .await
            .unwrap();
        assert!(claimed.is_empty());
    }

    #[tokio::test]
    async fn scheduled_missing_agentspec_fails_closed_without_chat_fallback() {
        let state = crate::test_utils::test_app_state();
        {
            let store = state.agent_spec_store.lock().await;
            store.set_active("main.default", false).unwrap();
        }

        let err = execute_scheduled_task(&state, "Missing AgentSpec", "scheduled prompt")
            .await
            .unwrap_err();

        assert!(
            err.message.contains("AgentSpec resolution failed"),
            "scheduled governance failure must be explicit: {}",
            err
        );
        assert!(
            !err.message.to_lowercase().contains("fallback"),
            "Scheduled must not surface Chat fallback warnings: {}",
            err
        );
        assert!(
            err.agent_run_id.is_none(),
            "governance failure before run creation must not carry agent_run_id"
        );

        let run_count = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .run_count()
            .unwrap();
        assert_eq!(
            run_count, 0,
            "missing AgentSpec must fail before creating scheduled or fallback AgentRuns"
        );

        let event_store = state.agent_run_event_store.as_ref().unwrap();
        assert_eq!(
            event_store
                .count_events_by_type(openlife_core::agent::AgentRunEventType::FallbackStarted)
                .unwrap(),
            0,
            "Scheduled missing AgentSpec must not record FallbackStarted"
        );
        assert_eq!(
            event_store
                .count_events_by_type(openlife_core::agent::AgentRunEventType::FallbackCompleted)
                .unwrap(),
            0,
            "Scheduled missing AgentSpec must not record FallbackCompleted"
        );
    }

    #[tokio::test]
    async fn scheduled_missing_agentspec_records_scheduler_task_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let state = crate::test_utils::test_app_state();

        {
            let store = state.agent_spec_store.lock().await;
            store.set_active("main.default", false).unwrap();
        }

        write_tasks_json(
            &path,
            &[json!({
                "id": "missing-spec-task",
                "title": "Missing Spec Task",
                "prompt": "scheduled prompt",
                "scheduled_at": past_str(),
                "status": "pending"
            })],
        );

        let now = now_dt();
        let claimed = claim_due_scheduled_tasks(&state.scheduled_task_mutex, &path, &now)
            .await
            .unwrap();
        assert_eq!(claimed.len(), 1);

        let err = execute_scheduled_task(&state, "Missing Spec Task", "scheduled prompt")
            .await
            .unwrap_err();
        merge_scheduled_task_outcome_by_id(
            &state.scheduled_task_mutex,
            &path,
            "missing-spec-task",
            TaskOutcome::failed(err),
        )
        .await
        .unwrap();

        let tasks = read_tasks_json(&path);
        let task = tasks
            .iter()
            .find(|t| t["id"] == "missing-spec-task")
            .unwrap();
        assert_eq!(task["status"], "failed");
        assert!(task["error"]
            .as_str()
            .unwrap()
            .contains("AgentSpec resolution failed"));
        assert!(
            task.get("completed_at").is_none(),
            "governance failure must keep scheduler task failure semantics"
        );
        assert!(
            task.get("agent_run_id").is_none(),
            "missing AgentSpec fails before run creation and must not write agent_run_id"
        );
    }

    #[tokio::test]
    async fn scheduled_runtime_failure_records_scheduler_task_failure_with_run_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_tasks_path(dir.path());
        let mutex = Mutex::new(());

        write_tasks_json(
            &path,
            &[json!({
                "id": "runtime-failed-task",
                "title": "Runtime Failed Task",
                "prompt": "runtime failure",
                "scheduled_at": past_str(),
                "status": "running"
            })],
        );

        merge_scheduled_task_outcome_by_id(
            &mutex,
            &path,
            "runtime-failed-task",
            TaskOutcome::failed(ScheduledTaskExecutionError {
                message: "Scheduled runtime failure: prompt stack error: unknown prompt block"
                    .into(),
                agent_run_id: Some("failed-run-123".into()),
            }),
        )
        .await
        .unwrap();

        let tasks = read_tasks_json(&path);
        let task = tasks
            .iter()
            .find(|t| t["id"] == "runtime-failed-task")
            .unwrap();
        assert_eq!(task["status"], "failed");
        assert!(task["error"]
            .as_str()
            .unwrap()
            .contains("Scheduled runtime failure"));
        assert_eq!(task["agent_run_id"], "failed-run-123");
        assert!(
            task.get("completed_at").is_none(),
            "failed runtime task must not be marked completed"
        );
        assert!(
            task.get("result_preview").is_none(),
            "failed runtime task must not receive success output"
        );
    }

    #[test]
    fn scheduled_execution_uses_scheduled_facade_wrapper_without_chat_fallback() {
        let source = include_str!("scheduler_runner.rs");
        let start = source
            .find("async fn execute_scheduled_task")
            .expect("scheduled execution helper should exist");
        let end = source[start..]
            .find("// ── Tests")
            .map(|offset| start + offset)
            .expect("tests section should follow scheduled helper");
        let scheduled_path = &source[start..end];
        let direct_run_call = [".", "run("].concat();

        assert!(
            scheduled_path.contains("run_tauri_scheduled_execution"),
            "Scheduled must call its dedicated Tauri ExecutionFacade wrapper"
        );
        assert!(
            scheduled_path.contains("build_prompt_registry"),
            "Scheduled must require PromptBlockRegistry assembly"
        );
        assert!(
            !scheduled_path.contains(&direct_run_call),
            "execute_scheduled_task must not call AgentLoop::run directly"
        );
        assert!(
            !scheduled_path.contains("run_tauri_agent_task"),
            "Scheduled must not be migrated to the Chat/StreamChat task entrypoint"
        );
        assert!(
            !scheduled_path.contains("handle_agent_loop_fallback")
                && !scheduled_path.contains("FallbackStarted")
                && !scheduled_path.contains("FallbackCompleted"),
            "Scheduled must not inherit Chat fallback"
        );
    }
}
