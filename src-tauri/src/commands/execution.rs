use crate::AppState;
use openlife_core::agent::{
    AgentAction, AgentObservation, AgentProposal, AgentRun, AgentTaskKind, ProposalSource,
    ProposalType, RiskLevel,
};
use openlife_core::tool_permissions::{
    ToolPermissionDecision, ToolPermissionPolicy, ToolPermissionRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

fn local_id(prefix: &str) -> String {
    format!(
        "{}-{}",
        prefix,
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRunResponse {
    pub run_id: String,
    pub status: String,
    pub summary: String,
    pub generated_proposals: Vec<String>,
}

#[tauri::command]
pub async fn list_tool_permissions(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ToolPermissionRecord>, String> {
    let store = state.tool_permission_store.lock().await;
    store.list().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn grant_tool_permission(
    tool_name: String,
    source: String,
    risk_level: String,
    action_type: String,
    policy: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ToolPermissionRecord, String> {
    let policy = policy
        .parse::<ToolPermissionPolicy>()
        .map_err(|e| e.to_string())?;
    let store = state.tool_permission_store.lock().await;
    store
        .grant(&tool_name, &source, &risk_level, &action_type, policy, None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn revoke_tool_permission(
    permission_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    let store = state.tool_permission_store.lock().await;
    store.revoke(&permission_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_tool_permission(
    tool_name: String,
    source: String,
    risk_level: String,
    action_type: String,
    capabilities: Vec<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<ToolPermissionDecision, String> {
    let store = state.tool_permission_store.lock().await;
    store
        .check(
            &tool_name,
            &source,
            &risk_level,
            &action_type,
            &capabilities,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_skills(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::skills::SkillManifest>, String> {
    let registry = state.skill_registry.lock().await;
    let mut skills = registry.list();
    let plugins = state.plugin_registry.lock().await;
    skills.extend(plugins.enabled_skills());
    Ok(skills)
}

#[tauri::command]
pub async fn run_skill(
    skill_id: String,
    input: Value,
    state: State<'_, Arc<AppState>>,
) -> Result<SkillRunResponse, String> {
    // 1. Get manifest and build prompts
    let (system_prompt, skill_prompt) = {
        let registry = state.skill_registry.lock().await;
        let _manifest = registry
            .get(&skill_id)
            .ok_or_else(|| format!("未知技能: {}", skill_id))?;
        let system = registry
            .build_system_prompt(&skill_id)
            .map_err(|e| e.to_string())?;
        let prompt = registry
            .build_skill_prompt(&skill_id, &input, &openlife_core::skills::SkillContext::default())
            .map_err(|e| e.to_string())?;
        (system, prompt)
    };

    // 2. Build AgentRuntime
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg);
    drop(cfg);

    // 3. Create skill task with system prompt
    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Skill,
        session_id: format!("skill-{}", skill_id),
        user_text: skill_prompt.clone(),
        messages: vec![
            openlife_core::llm::ChatMessage {
                role: "system".into(),
                content: system_prompt,
            },
            openlife_core::llm::ChatMessage {
                role: "user".into(),
                content: skill_prompt,
            },
        ],
        layer: openlife_core::layer_router::Layer::L2,
    };

    // 4. Execute via AgentRuntime
    let runtime_output = agent_runtime
        .execute_task(
            &task,
            &life_model,
            "",
            None,
            vec![],
            openlife_core::privacy::PrivacyEngine::default(),
        )
        .await
        .map_err(|e| e.to_string())?;

    // 5. Generate skill response from model
    let model_output = scheduler
        .generate(runtime_output.final_messages.clone(), &life_model, None)
        .await
        .map_err(|e| e.to_string())?;

    // 6. Parse JSON envelope
    let (envelope, parse_error) = match openlife_core::skills::parse_skill_json(&model_output) {
        Ok(env) => (env, None),
        Err(e) => {
            // Return a default envelope with parse error
            (
                openlife_core::skills::SkillJsonEnvelope {
                    summary: "JSON 解析失败".to_string(),
                    structured_output: serde_json::json!({
                        "raw_output": model_output,
                        "parse_error": e
                    }),
                    proposal_candidates: vec![],
                    warnings: vec![format!("解析错误: {}", e)],
                },
                Some(e),
            )
        }
    };

    // 7. Create AgentRun
    let mut run = AgentRun::new_chat_run(
        &task.session_id,
        input.get("text").and_then(Value::as_str).unwrap_or(""),
    );
    run.kind = AgentTaskKind::Skill;
    let model_route = scheduler.preview_chat_route(None).await;
    run.complete(
        &crate::preview_text(&envelope.summary, 200),
        model_route,
        runtime_output.context_summary,
    );
    run.reasoning_trace = Some(runtime_output.reasoning_trace);

    // Set status based on parse result
    if parse_error.is_some() {
        run.status = openlife_core::agent::AgentRunStatus::Completed;
        run.error = Some(openlife_core::agent::AgentRunError {
            message: parse_error.clone().unwrap(),
            phase: "skill_json_parse".into(),
            recoverable: true,
        });
    }

    run.actions.push(AgentAction {
        id: local_id("action"),
        action_type: "skill_run".into(),
        target: Some(skill_id.clone()),
        input: input.clone(),
        output: Some(serde_json::to_value(&envelope).unwrap_or_default()),
        status: if parse_error.is_some() {
            "completed_with_warnings".into()
        } else {
            "succeeded".into()
        },
        permission_decision: Some("allow".into()),
        tool_scope: None,
        started_at: Some(run.started_at),
        finished_at: run.finished_at,
        error: parse_error.clone(),
        timestamp: chrono::Utc::now(),
    });
    run.observations.push(AgentObservation {
        id: local_id("observation"),
        action_id: run.actions.first().map(|a| a.id.clone()),
        content: envelope.summary.clone(),
        source: format!("skill:{}", skill_id),
        structured_result: Some(serde_json::to_value(&envelope).unwrap_or_default()),
        timestamp: chrono::Utc::now(),
    });

    // 8. Generate proposals from envelope
    let mut generated = Vec::new();
    if parse_error.is_none() {
        if let Some(ref proposal_store_arc) = state.proposal_store {
            let store = proposal_store_arc.lock().await;
            for candidate in &envelope.proposal_candidates {
                let proposal_type = match candidate.proposal_type.as_str() {
                    "goal_update" => ProposalType::GoalUpdate,
                    "state_update" => ProposalType::StateUpdate,
                    "memory_write" => ProposalType::MemoryWrite,
                    "memory_archive" => ProposalType::MemoryArchive,
                    "preference_update" => ProposalType::PreferenceUpdate,
                    "capability_update" => ProposalType::CapabilityUpdate,
                    _ => ProposalType::MemoryWrite, // fallback
                };

                let proposal = AgentProposal::new(
                    proposal_type,
                    &candidate.affected_path,
                    candidate.after.clone(),
                    &candidate.reason,
                    candidate.confidence.clamp(0.0, 1.0),
                    RiskLevel::Medium,
                    ProposalSource::SkillRuntime,
                );
                let proposal_id = proposal.id.clone();
                store
                    .create_proposal(&proposal)
                    .map_err(|e| e.to_string())?;
                generated.push(proposal_id.clone());
                run.add_generated_proposal(&proposal_id);
            }
        }
    }

    if let Some(ref run_store_arc) = state.agent_run_store {
        let store = run_store_arc.lock().await;
        store.create_run(&run).map_err(|e| e.to_string())?;
    }

    Ok(SkillRunResponse {
        run_id: run.id,
        status: if parse_error.is_some() {
            "completed_with_warnings".into()
        } else {
            "completed".into()
        },
        summary: envelope.summary,
        generated_proposals: generated,
    })
}

#[tauri::command]
pub async fn get_skill_run_status(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<AgentRun>, String> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.get_run(&run_id).map_err(|e| e.to_string())
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn list_plugins(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::plugins::PluginRecord>, String> {
    let registry = state.plugin_registry.lock().await;
    Ok(registry.list())
}

#[tauri::command]
pub async fn reload_plugins(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::plugins::PluginRecord>, String> {
    let mut registry = state.plugin_registry.lock().await;
    registry.reload().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn enable_plugin(
    plugin_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut registry = state.plugin_registry.lock().await;
    registry.enable(&plugin_id, true).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disable_plugin(
    plugin_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut registry = state.plugin_registry.lock().await;
    registry
        .enable(&plugin_id, false)
        .map_err(|e| e.to_string())
}
