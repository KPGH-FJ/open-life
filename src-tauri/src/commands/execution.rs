use crate::AppState;
use openlife_core::agent::{
    AgentAction, AgentObservation, AgentProposal, AgentRun, AgentRunStatus, AgentTaskKind,
    ProposalSource, ProposalType, RiskLevel,
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
    let result = {
        let registry = state.skill_registry.lock().await;
        registry
            .run_builtin(&skill_id, input.clone())
            .map_err(|e| e.to_string())?
    };

    let mut run = AgentRun::new_chat_run(
        "skill",
        input.get("text").and_then(Value::as_str).unwrap_or(""),
    );
    run.kind = AgentTaskKind::Skill;
    run.output_preview = Some(result.summary.clone());
    run.status = AgentRunStatus::Completed;
    run.finished_at = Some(chrono::Utc::now());
    run.actions.push(AgentAction {
        id: local_id("action"),
        action_type: "skill_run".into(),
        target: Some(skill_id.clone()),
        input: input.clone(),
        output: Some(result.structured_output.clone()),
        status: "succeeded".into(),
        permission_decision: Some("allow".into()),
        started_at: Some(run.started_at),
        finished_at: run.finished_at,
        error: None,
        timestamp: chrono::Utc::now(),
    });
    run.observations.push(AgentObservation {
        id: local_id("observation"),
        action_id: run.actions.first().map(|a| a.id.clone()),
        content: result.summary.clone(),
        source: format!("skill:{}", skill_id),
        structured_result: Some(result.structured_output.clone()),
        timestamp: chrono::Utc::now(),
    });

    let mut generated = Vec::new();
    if let Some(ref proposal_store_arc) = state.proposal_store {
        let store = proposal_store_arc.lock().await;
        for candidate in result.proposal_candidates {
            let proposal = AgentProposal::new(
                ProposalType::MemoryWrite,
                "memory.skill_output",
                serde_json::json!({
                    "content": candidate.get("content").cloned().unwrap_or(Value::String(result.summary.clone())),
                    "source": format!("skill:{}", skill_id),
                    "session_id": "skill"
                }),
                candidate
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or(&result.summary),
                0.7,
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

    if let Some(ref run_store_arc) = state.agent_run_store {
        let store = run_store_arc.lock().await;
        store.create_run(&run).map_err(|e| e.to_string())?;
    }

    Ok(SkillRunResponse {
        run_id: run.id,
        status: "completed".into(),
        summary: result.summary,
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
