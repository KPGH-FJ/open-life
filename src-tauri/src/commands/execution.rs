use crate::errors::AppError;
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
) -> Result<Vec<ToolPermissionRecord>, AppError> {
    let store = state.tool_permission_store.lock().await;
    store.list().map_err(AppError::from)
}

#[tauri::command]
pub async fn grant_tool_permission(
    tool_name: String,
    source: String,
    risk_level: String,
    action_type: String,
    policy: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ToolPermissionRecord, AppError> {
    let policy = policy
        .parse::<ToolPermissionPolicy>()
        .map_err(AppError::from)?;
    let store = state.tool_permission_store.lock().await;
    store
        .grant(&tool_name, &source, &risk_level, &action_type, policy, None)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn revoke_tool_permission(
    permission_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, AppError> {
    let store = state.tool_permission_store.lock().await;
    store.revoke(&permission_id).map_err(AppError::from)
}

#[tauri::command]
pub async fn check_tool_permission(
    tool_name: String,
    source: String,
    risk_level: String,
    action_type: String,
    capabilities: Vec<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<ToolPermissionDecision, AppError> {
    let store = state.tool_permission_store.lock().await;
    store
        .check(
            &tool_name,
            &source,
            &risk_level,
            &action_type,
            &capabilities,
        )
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn list_skills(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::skills::SkillManifest>, AppError> {
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
) -> Result<SkillRunResponse, AppError> {
    // 1. Build enhanced SkillContext and load LifeModel
    let (life_model, skill_context) = {
        let manager = state.life_model_manager.lock().await;
        let lm = manager.load().map_err(AppError::from)?;

        // Build enhanced SkillContext
        let mut ctx = openlife_core::skills::SkillContext {
            life_model_json: Some(serde_json::to_string(&lm).unwrap_or_default()),
            ..Default::default()
        };

        // Load recent runs
        if let Some(ref store_arc) = state.agent_run_store {
            let store = store_arc.lock().await;
            if let Ok(runs) = store.list_runs(10, 0) {
                ctx.recent_runs_json = Some(serde_json::to_string(&runs).unwrap_or_default());
            }
        }

        (lm, ctx)
    };

    // 2. Get manifest and build prompts
    let (system_prompt, skill_prompt) = {
        let registry = state.skill_registry.lock().await;
        let _manifest = registry
            .get(&skill_id)
            .ok_or_else(|| format!("未知技能: {}", skill_id))?;
        let system = registry
            .build_system_prompt(&skill_id)
            .map_err(AppError::from)?;
        let prompt = registry
            .build_skill_prompt(&skill_id, &input, &skill_context)
            .map_err(AppError::from)?;
        (system, prompt)
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
        .map_err(AppError::from)?;

    // 5. Generate skill response from model
    let model_output = scheduler
        .generate(runtime_output.final_messages.clone(), &life_model, None)
        .await
        .map_err(AppError::from)?;

    // 6. Parse JSON envelope
    let (mut envelope, parse_error) = match openlife_core::skills::parse_skill_json(&model_output) {
        Ok(env) => (env, None),
        Err(e) => {
            // Return a default envelope with parse error
            (
                openlife_core::skills::SkillJsonEnvelope {
                    summary: "JSON 解析失败".to_string(),
                    structured_output: serde_json::json!({
                        "raw_output": model_output.clone(),
                        "parse_error": e.clone()
                    }),
                    proposal_candidates: vec![],
                    warnings: vec![format!("解析错误: {}", e)],
                },
                Some(e),
            )
        }
    };

    // 6.5. Validate envelope with fail-soft strategy
    if parse_error.is_none() {
        let (validated, validation_warnings) =
            openlife_core::skills::validate_skill_envelope(envelope, &model_output);
        envelope = validated;
        // Log validation warnings
        for warning in &validation_warnings {
            eprintln!("[Skill Validation] {}", warning);
        }
    }

    let mut validated_candidates = Vec::new();
    if parse_error.is_none() {
        for candidate in &envelope.proposal_candidates {
            let proposal_type = match candidate.proposal_type.as_str() {
                "goal_update" => Some(ProposalType::GoalUpdate),
                "state_update" => Some(ProposalType::StateUpdate),
                "memory_write" => Some(ProposalType::MemoryWrite),
                "memory_archive" => Some(ProposalType::MemoryArchive),
                "preference_update" => Some(ProposalType::PreferenceUpdate),
                "capability_update" => Some(ProposalType::CapabilityUpdate),
                other => {
                    envelope.warnings.push(format!(
                        "跳过不支持的 proposal_type: {} (affected_path={})",
                        other, candidate.affected_path
                    ));
                    None
                }
            };
            if let Some(proposal_type) = proposal_type {
                validated_candidates.push((proposal_type, candidate.clone()));
            }
        }
    }

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

    // Set status based on parse/validation result
    let has_warnings = parse_error.is_some() || !envelope.warnings.is_empty();
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
        status: if has_warnings {
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
    // Build observation content including warnings
    let observation_content = if envelope.warnings.is_empty() {
        envelope.summary.clone()
    } else {
        format!(
            "{}\n\n[Warnings]\n{}",
            envelope.summary,
            envelope.warnings.join("\n")
        )
    };

    run.observations.push(AgentObservation {
        id: local_id("observation"),
        action_id: run.actions.first().map(|a| a.id.clone()),
        content: observation_content,
        source: format!("skill:{}", skill_id),
        structured_result: Some(serde_json::to_value(&envelope).unwrap_or_default()),
        timestamp: chrono::Utc::now(),
    });

    // 8. Generate proposals from envelope
    let mut generated = Vec::new();
    if parse_error.is_none() {
        if let Some(ref proposal_store_arc) = state.proposal_store {
            let store = proposal_store_arc.lock().await;
            for (proposal_type, candidate) in &validated_candidates {
                let proposal = AgentProposal::new(
                    *proposal_type,
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
                    .map_err(AppError::from)?;
                generated.push(proposal_id.clone());
                run.add_generated_proposal(&proposal_id);
            }
        }
    }

    if let Some(ref run_store_arc) = state.agent_run_store {
        let store = run_store_arc.lock().await;
        store.create_run(&run).map_err(AppError::from)?;
    }

    Ok(SkillRunResponse {
        run_id: run.id,
        status: if has_warnings {
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
) -> Result<Option<AgentRun>, AppError> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.get_run(&run_id).map_err(AppError::from)
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn list_plugins(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::plugins::PluginRecord>, AppError> {
    let registry = state.plugin_registry.lock().await;
    Ok(registry.list())
}

#[tauri::command]
pub async fn reload_plugins(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::plugins::PluginRecord>, AppError> {
    let mut registry = state.plugin_registry.lock().await;
    let records = registry.reload().map_err(AppError::from)?;

    // Plugin tools are declarative-only in Beta; do not register them to McpRegistry.
    // They remain visible in PluginRegistry for manifest inspection only.
    {
        let mut mcp = state.mcp_registry.lock().await;
        mcp.remove_builtins_by_source(|source| {
            matches!(
                source,
                openlife_core::tool_manifest::ToolSource::Plugin { .. }
            )
        });
    }

    // Sync plugin skills to SkillRegistry
    {
        let mut skill_reg = state.skill_registry.lock().await;
        skill_reg.remove_by_source_prefix("plugin:");
        for record in &records {
            if record.enabled && record.error.is_none() {
                for skill in &record.manifest.skills {
                    let mut skill_clone = skill.clone();
                    skill_clone.id = format!("plugin:{}:{}", record.manifest.id, skill.id);
                    skill_reg.register(skill_clone);
                }
            }
        }
    }

    Ok(records)
}

#[tauri::command]
pub async fn enable_plugin(
    plugin_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    {
        let mut registry = state.plugin_registry.lock().await;
        registry
            .enable(&plugin_id, true)
            .map_err(AppError::from)?;
    }

    // Sync to registries
    {
        let registry = state.plugin_registry.lock().await;
        if let Some(record) = registry
            .list()
            .into_iter()
            .find(|r| r.manifest.id == plugin_id)
        {
            if record.enabled && record.error.is_none() {
                // Plugin tools are declarative-only in Beta; do not register to McpRegistry.
                // Register skills only
                let mut skill_reg = state.skill_registry.lock().await;
                for skill in &record.manifest.skills {
                    let mut skill_clone = skill.clone();
                    skill_clone.id = format!("plugin:{}:{}", plugin_id, skill.id);
                    skill_reg.register(skill_clone);
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn disable_plugin(
    plugin_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    {
        let mut registry = state.plugin_registry.lock().await;
        registry
            .enable(&plugin_id, false)
            .map_err(AppError::from)?;
    }

    // Remove from registries
    {
        let mut mcp = state.mcp_registry.lock().await;
        mcp.remove_builtins_by_source(|source| {
            matches!(source, openlife_core::tool_manifest::ToolSource::Plugin { plugin_id: ref pid } if pid == &plugin_id)
        });

        let mut skill_reg = state.skill_registry.lock().await;
        skill_reg.remove_by_source_prefix(&format!("plugin:{}:", plugin_id));
    }

    Ok(())
}
