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
    error: Option<String>,
}

impl TaskOutcome {
    fn completed(response: String) -> Self {
        Self {
            status: "completed".to_string(),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
            result_preview: Some(response.chars().take(500).collect()),
            error: None,
        }
    }

    fn failed(err: String) -> Self {
        Self {
            status: "failed".to_string(),
            completed_at: None,
            result_preview: None,
            error: Some(err),
        }
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

// ── Task execution (unchanged, no lock held) ─────────────────────

async fn execute_scheduled_task(
    state: &Arc<AppState>,
    _title: &str,
    prompt: &str,
) -> Result<String, String> {
    let cfg = state.config.lock().await;
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager
            .load()
            .map_err(|e| format!("LifeModel load failed: {}", e))?
    };
    let scheduler = state.scheduler.lock().await.clone();
    let privacy_engine = state.privacy_engine.lock().await.clone();
    let runtime_assembly = crate::execution_facade::build_runtime_assembly_config(
        &cfg,
        crate::execution_facade::TauriAgentExecutionMode::Scheduled,
        state.shutdown_notify.clone(),
    );
    let agent_loop = crate::execution_facade::build_governed_agent_loop(
        life_model.clone(),
        scheduler.clone(),
        &cfg,
        &runtime_assembly,
        &state.agent_run_event_store,
    );
    drop(cfg);

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

    let tools_prompt = String::new();

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
            return Err(format!(
                "AgentSpec resolution failed: {}. Scheduled execution requires a valid active main AgentSpec.",
                e
            ));
        }
    };
    let prompt_registry = crate::execution_facade::build_prompt_registry();

    let loop_result = {
        let action_ctx = crate::execution_facade::build_governed_action_context(
            state,
            &runtime_assembly,
            Some(life_model.clone()),
            Some(state.memory_store.clone()),
            agent_spec.clone(),
        );

        agent_loop
            .run(
                &task,
                &life_model,
                &tools_prompt,
                None,
                privacy_engine.clone(),
                agent_spec.privacy_policy,
                &agent_spec,
                &prompt_registry,
                &action_ctx,
            )
            .await
            .map_err(|e| format!("AgentLoop execution failed: {}", e))
    }?;

    if let Some(ref store) = state.agent_run_store {
        let store = store.lock().await;
        let run = loop_result.run;
        if let Err(e) = store.create_run(&run) {
            log::error!(
                "[scheduler_runner] Failed to persist scheduled AgentRun {}: {}",
                run.id,
                e
            );
        }
    }

    Ok(loop_result.final_response)
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
            TaskOutcome::completed("done A".into()),
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
            TaskOutcome::completed("A result".into()),
        )
        .await
        .unwrap();

        let result = read_tasks_json(&path);
        assert_eq!(result.len(), 2, "both tasks must remain");

        let a = result.iter().find(|t| t["id"] == "task-A").unwrap();
        assert_eq!(a["status"], "completed");
        assert!(a.get("completed_at").is_some());
        assert_eq!(a["result_preview"], "A result");

        let b = result.iter().find(|t| t["id"] == "task-B").unwrap();
        assert_eq!(b["status"], "pending");
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
            TaskOutcome::failed("exec error".into()),
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
            TaskOutcome::completed("safe done".into()),
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
            TaskOutcome::completed("ghost".into()),
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
            TaskOutcome::completed("done".into()),
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
}
