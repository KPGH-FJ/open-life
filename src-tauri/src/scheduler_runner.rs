//! Scheduled task runner: periodically checks scheduled_tasks.json
//! for pending tasks and triggers AgentLoop executions.
//! Runs as a background tokio task spawned during app bootstrap.

use crate::storage::app_data_dir;
use crate::AppState;
use openlife_core::agent::agent_loop::{AgentLoopConfig, AgentRole};
use openlife_core::agent::{AgentLoop, AgentTask, AgentTaskKind};
use openlife_core::layer_router::Layer;
use openlife_core::llm::ChatMessage;
use serde_json::Value;
use std::sync::Arc;

/// Check interval (seconds) for the scheduled task runner.
const CHECK_INTERVAL_SECONDS: u64 = 60;

/// Start the scheduled task runner as a background loop.
pub fn start_scheduler_runner(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(CHECK_INTERVAL_SECONDS)).await;

            let mut tasks = match read_scheduled_tasks() {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("[scheduler_runner] Failed to read scheduled tasks: {}", e);
                    continue;
                }
            };
            let now = chrono::Utc::now().to_rfc3339();
            let mut tasks_dirty = false;

            for task in &mut tasks {
                if task.get("status").and_then(Value::as_str) != Some("pending") {
                    continue;
                }

                let scheduled_at = task
                    .get("scheduled_at")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if scheduled_at.is_empty() || scheduled_at > now.as_str() {
                    continue;
                }

                let title = task
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("Scheduled Task")
                    .to_string();
                let prompt = task
                    .get("prompt")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();

                log::info!("[scheduler_runner] Executing scheduled task: {}", title,);

                // Mark as running
                if let Some(obj) = task.as_object_mut() {
                    obj.insert("status".to_string(), Value::String("running".to_string()));
                }
                tasks_dirty = true;

                // Execute agent
                match execute_scheduled_task(&state, &title, &prompt).await {
                    Ok(response) => {
                        log::info!("[scheduler_runner] Task '{}' completed", title);
                        if let Some(obj) = task.as_object_mut() {
                            obj.insert(
                                "status".to_string(),
                                Value::String("completed".to_string()),
                            );
                            obj.insert(
                                "completed_at".to_string(),
                                Value::String(chrono::Utc::now().to_rfc3339()),
                            );
                            obj.insert(
                                "result_preview".to_string(),
                                Value::String(response.chars().take(500).collect()),
                            );
                        }
                    }
                    Err(e) => {
                        log::warn!("[scheduler_runner] Task '{}' failed: {}", title, e);
                        if let Some(obj) = task.as_object_mut() {
                            obj.insert("status".to_string(), Value::String("failed".to_string()));
                            obj.insert("error".to_string(), Value::String(e));
                        }
                    }
                }
            }

            // Persist if tasks were modified
            if tasks_dirty {
                if let Err(e) = write_scheduled_tasks(&tasks) {
                    log::warn!("[scheduler_runner] Failed to persist tasks: {}", e);
                }
            }
        }
    });
}

fn read_scheduled_tasks() -> Result<Vec<Value>, String> {
    let path = app_data_dir().join("scheduled_tasks.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read scheduled_tasks.json: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse scheduled_tasks.json: {}", e))
}

fn write_scheduled_tasks(tasks: &[Value]) -> Result<(), String> {
    let path = app_data_dir().join("scheduled_tasks.json");
    let temp = path.with_extension("tmp");
    let content = serde_json::to_string_pretty(tasks)
        .map_err(|e| format!("Failed to serialize tasks: {}", e))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create scheduled task directory: {}", e))?;
    }
    std::fs::write(&temp, &content).map_err(|e| format!("Failed to write tasks temp: {}", e))?;
    std::fs::rename(&temp, &path).map_err(|e| format!("Failed to atomically save tasks: {}", e))?;
    Ok(())
}

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
    let safe_paths = cfg.system.safe_paths.clone();
    let calendar_ics_paths = cfg.system.calendar_ics_paths.clone();
    let network_policy = cfg.system.network_policy.clone();

    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg);
    let action_executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let loop_config = AgentLoopConfig {
        max_steps: 2,
        max_tool_calls: 4,
        timeout_seconds: 60,
        allow_writes: false,
        allow_cloud: true,
        shutdown_notify: Some(state.shutdown_notify.clone()),
        role: AgentRole::Planner,
        toolset_allowlist: vec![
            "goal.read".into(),
            "life_model.read".into(),
            "state.read".into(),
            "memory.search".into(),
            "proposal.create".into(),
        ],
        tool_action_allowlist: Vec::new(),
    };
    let agent_loop = AgentLoop::new(
        agent_runtime,
        action_executor,
        scheduler.clone(),
        loop_config,
    );

    let task = AgentTask {
        kind: AgentTaskKind::Proactive,
        session_id: format!("scheduled-{}", uuid::Uuid::new_v4()),
        user_text: prompt.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        }],
        layer: Layer::L2,
    };

    let tools_prompt = String::new();

    let loop_result = {
        let (reg, audit) = state.get_mcp_state().await;
        let permission_store = state.tool_permission_store.lock().await;
        let memory_store = state.memory_store.lock().await;
        let proposal_store_guard = if let Some(ref store) = state.proposal_store {
            Some(store.lock().await)
        } else {
            None
        };
        let agent_run_store_guard = if let Some(ref store) = state.agent_run_store {
            Some(store.lock().await)
        } else {
            None
        };
        let action_ctx = openlife_core::agent::ActionExecutionContext {
            registry: &reg,
            permission_store: &permission_store,
            audit_store: &audit,
            privacy_engine: &privacy_engine,
            safe_paths: &safe_paths,
            calendar_ics_paths: &calendar_ics_paths,
            life_model: Some(&life_model),
            memory_store: Some(&memory_store),
            proposal_store: proposal_store_guard.as_deref(),
            agent_run_store: agent_run_store_guard.as_deref(),
            network_policy: Some(&network_policy),
            hs_runtime_packet: None,
            web_search_fixture_output: None,
        };

        agent_loop
            .run(
                &task,
                &life_model,
                &tools_prompt,
                None,
                privacy_engine.clone(),
                &action_ctx,
            )
            .await
            .map_err(|e| format!("AgentLoop execution failed: {}", e))
    }?;

    // Persist AgentRun
    if let Some(ref store) = state.agent_run_store {
        if let Ok(store) = store.try_lock() {
            let run = loop_result.run;
            let _ = store.create_run(&run);
        }
    }

    Ok(loop_result.final_response)
}
