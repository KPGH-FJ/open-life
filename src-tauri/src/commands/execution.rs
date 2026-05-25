use crate::errors::AppError;
use crate::AppState;
use openlife_core::agent::trace_payloads;
use openlife_core::agent::{
    AgentAction, AgentObservation, AgentProposal, AgentRun, AgentRunError, AgentTaskKind,
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

/// Production helper: create a governed AgentRun for skill execution.
///
/// Extracted so that both `run_skill` and plain-unit tests can assert
/// the AgentSpec binding without requiring a live LLM scheduler.
pub(crate) fn build_skill_agent_run(
    skill_id: &str,
    input: &Value,
    agent_spec_id: &str,
) -> AgentRun {
    let mut run = AgentRun::new_chat_run(
        &format!("skill-{}", skill_id),
        input.get("text").and_then(Value::as_str).unwrap_or(""),
    );
    run.kind = AgentTaskKind::Skill;
    run.agent_spec_id = Some(agent_spec_id.to_string());
    run
}

#[tauri::command]
pub async fn run_skill(
    skill_id: String,
    input: Value,
    state: State<'_, Arc<AppState>>,
) -> Result<SkillRunResponse, AppError> {
    run_skill_with_state(skill_id, input, state.inner()).await
}

pub(crate) fn validate_skill_agent_spec_tool_policy(
    manifest: &openlife_core::skills::SkillManifest,
    agent_spec: &openlife_core::agent::AgentSpec,
) -> Result<(), AppError> {
    if let Some(disallowed) = manifest
        .allowed_tools
        .iter()
        .find(|tool| !agent_spec.is_tool_allowed(tool))
    {
        return Err(AppError::permission(format!(
            "Skill '{}' requires tool '{}' but AgentSpec '{}' does not allow it",
            manifest.id, disallowed, agent_spec.id
        )));
    }
    Ok(())
}

async fn persist_failed_skill_run(
    state: &Arc<AppState>,
    run: &AgentRun,
    phase: &str,
    message: &str,
    recoverable: bool,
) -> Result<(), AppError> {
    let mut failed_run = run.clone();
    failed_run.fail(AgentRunError {
        message: crate::preview_text(message, 500),
        phase: phase.to_string(),
        recoverable,
    });

    if let Some(ref run_store_arc) = state.agent_run_store {
        let store = run_store_arc.lock().await;
        store.create_run(&failed_run).map_err(AppError::from)?;
    }

    if let Some(ref es) = state.agent_run_event_store {
        es.append_event(&openlife_core::agent::AgentRunEvent::new(
            &failed_run.id,
            openlife_core::agent::AgentRunEventType::RunFailed,
            openlife_core::agent::AgentEventActor::Runtime,
            format!("Skill runtime failed in {}", phase),
            trace_payloads::build_run_failed_payload(crate::preview_text(message, 500)),
        ))
        .map_err(AppError::from)?;
    }

    Ok(())
}

pub(crate) async fn run_skill_with_state(
    skill_id: String,
    input: Value,
    state: &Arc<AppState>,
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
    let (manifest, system_prompt, skill_prompt) = {
        let registry = state.skill_registry.lock().await;
        let manifest = registry
            .get(&skill_id)
            .ok_or_else(|| format!("未知技能: {}", skill_id))?;
        let system = registry
            .build_system_prompt(&skill_id)
            .map_err(AppError::from)?;
        let prompt = registry
            .build_skill_prompt(&skill_id, &input, &skill_context)
            .map_err(AppError::from)?;
        (manifest, system, prompt)
    };

    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg);
    drop(cfg);

    // Resolve stored default AgentSpec for governed execution — fail closed, no fallback.
    let prompt_registry = openlife_core::agent::prompt_stack::PromptBlockRegistry::built_in();
    let agent_spec =
        crate::commands::agent_spec::resolve_required_agent_spec(&state.agent_spec_store, None)
            .await?;
    validate_skill_agent_spec_tool_policy(&manifest, &agent_spec)?;

    // Create AgentRun before governed execution so events can reference the run_id.
    let mut run = build_skill_agent_run(&skill_id, &input, &agent_spec.id);

    // Record AgentSpecSelected event
    if let Some(ref es) = state.agent_run_event_store {
        let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
            &run.id,
            openlife_core::agent::AgentRunEventType::AgentSpecSelected,
            openlife_core::agent::AgentEventActor::Runtime,
            format!(
                "AgentSpec {} selected for skill {}",
                agent_spec.id, skill_id
            ),
            trace_payloads::build_agent_spec_selected_payload(
                &agent_spec.id,
                agent_spec.role.to_string(),
                agent_spec.privacy_policy.to_string(),
            ),
        ));
    }

    // 3. Create skill task with system prompt (skill prompt as user message)
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
        agent_spec_id: Some(agent_spec.id.clone()),
        ..Default::default()
    };

    // 4. Execute via AgentRuntime with stored AgentSpec governance
    let runtime_output = match agent_runtime
        .execute_task_with_spec(
            &task,
            &life_model,
            "",
            None,
            vec![],
            openlife_core::privacy::PrivacyEngine::default(),
            &agent_spec,
            &prompt_registry,
        )
        .await
    {
        Ok(output) => output,
        Err(err) => {
            let message = format!("Skill runtime failed: {}", err);
            let phase =
                if message.contains("prompt stack") || message.contains("unknown prompt block") {
                    "skill_prompt_stack"
                } else {
                    "skill_runtime"
                };
            persist_failed_skill_run(state, &run, phase, &message, err.is_governance_failure())
                .await?;
            return Err(AppError::internal(message));
        }
    };

    // 5. Generate skill response from model with privacy enforcement
    let model_output = match scheduler
        .generate_governed(
            runtime_output.final_messages.clone(),
            &life_model,
            None,
            agent_spec.privacy_policy,
        )
        .await
    {
        Ok(output) => output,
        Err(err) => {
            let message = format!("Skill model generation failed: {}", err);
            persist_failed_skill_run(state, &run, "skill_model_generation", &message, true).await?;
            return Err(AppError::external(message));
        }
    };

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

    // 7. Finalize AgentRun (created earlier for event recording)
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
                store.create_proposal(&proposal).map_err(AppError::from)?;
                generated.push(proposal_id.clone());
                run.add_generated_proposal(&proposal_id);
            }
        }
    }

    if let Some(ref run_store_arc) = state.agent_run_store {
        let store = run_store_arc.lock().await;
        store.create_run(&run).map_err(AppError::from)?;
    }

    // Record governance events after run is persisted (avoids orphan events).
    if let Some(ref es) = state.agent_run_event_store {
        let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
            &run.id,
            openlife_core::agent::AgentRunEventType::PromptStackAssembled,
            openlife_core::agent::AgentEventActor::Runtime,
            format!(
                "PromptStack assembled with {} blocks from AgentSpec {} for skill {}",
                runtime_output.prompt_block_trace.len(),
                agent_spec.id,
                skill_id
            ),
            trace_payloads::build_prompt_stack_assembled_payload(
                &agent_spec.id,
                serde_json::to_value(&runtime_output.prompt_block_trace).unwrap_or_default(),
            ),
        ));
        let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
            &run.id,
            openlife_core::agent::AgentRunEventType::ContextGovernanceApplied,
            openlife_core::agent::AgentEventActor::Runtime,
            format!(
                "Context governance applied by AgentSpec {} for skill {}",
                agent_spec.id, skill_id
            ),
            trace_payloads::build_context_governance_applied_payload(
                &agent_spec.id,
                runtime_output
                    .governed_context_summary
                    .as_ref()
                    .map(|g| g.included.clone())
                    .unwrap_or_default(),
                runtime_output
                    .governed_context_summary
                    .as_ref()
                    .map(|g| g.excluded.clone())
                    .unwrap_or_default(),
                agent_spec.privacy_policy.to_string(),
                trace_payloads::ContextGovernanceEmitter::StreamingExecution,
            ),
        ));
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
        registry.enable(&plugin_id, true).map_err(AppError::from)?;
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
        registry.enable(&plugin_id, false).map_err(AppError::from)?;
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

#[cfg(test)]
mod tests {
    use super::{
        build_skill_agent_run, run_skill_with_state, validate_skill_agent_spec_tool_policy,
    };
    use crate::test_utils::test_app_state;
    use openlife_core::agent::{
        AgentRunEventType, AgentRunStatus, AgentSpec, AgentTaskKind, PrivacyPolicy,
    };
    use openlife_core::skills::SkillManifest;
    use serde_json::json;

    #[tokio::test]
    async fn skill_runtime_run_persists_agent_spec_id() {
        let state = test_app_state();

        let run = build_skill_agent_run("test-skill", &json!({"text": "do thing"}), "main.default");

        assert_eq!(run.kind, AgentTaskKind::Skill);
        assert_eq!(
            run.agent_spec_id.as_deref(),
            Some("main.default"),
            "build_skill_agent_run must bind agent_spec_id"
        );

        let run_id = run.id.clone();
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }

        let fetched = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.get_run(&run_id).unwrap().unwrap()
        };

        assert_eq!(fetched.kind, AgentTaskKind::Skill);
        assert_eq!(
            fetched.agent_spec_id.as_deref(),
            Some("main.default"),
            "persisted SkillRuntime run must retain agent_spec_id"
        );
    }

    #[tokio::test]
    async fn skill_runtime_missing_agentspec_fails_closed_without_run_or_chat_fallback() {
        let state = test_app_state();
        {
            let store = state.agent_spec_store.lock().await;
            store.set_active("main.default", false).unwrap();
        }

        let err = run_skill_with_state(
            "weekly_review".into(),
            json!({"text": "missing AgentSpec must not call model"}),
            &state,
        )
        .await
        .expect_err("missing AgentSpec must fail closed");

        assert!(
            err.message().contains("AgentSpec")
                || err.message().contains("no active main AgentSpec"),
            "error should make AgentSpec resolution visible, got: {}",
            err.message()
        );

        let runs = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_runs(10, 0)
            .unwrap();
        assert!(
            runs.is_empty(),
            "missing AgentSpec must not create AgentRun"
        );

        let events = state.agent_run_event_store.as_ref().unwrap();
        assert_eq!(
            events
                .count_events_by_type(AgentRunEventType::FallbackStarted)
                .unwrap(),
            0
        );
        assert_eq!(
            events
                .count_events_by_type(AgentRunEventType::FallbackCompleted)
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn skill_runtime_prompt_stack_failure_persists_failed_run_with_safe_payload() {
        let state = test_app_state();
        {
            let store = state.agent_spec_store.lock().await;
            let mut spec = store.get_default_spec().unwrap().unwrap();
            spec.prompt_block_ids = vec!["missing.skill.prompt.block".into()];
            store.update_spec(&spec).unwrap();
        }

        let raw_input = "raw prompt sentinel and raw LifeModel sentinel must not enter events";
        let err = run_skill_with_state("weekly_review".into(), json!({"text": raw_input}), &state)
            .await
            .expect_err("unknown PromptStack block must fail");
        assert!(
            err.message().contains("prompt stack")
                || err.message().contains("unknown prompt block"),
            "PromptStack failure must be readable, got: {}",
            err.message()
        );

        let runs = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_runs(10, 0)
            .unwrap();
        assert_eq!(
            runs.len(),
            1,
            "PromptStack failure should persist one failed run"
        );
        let run = &runs[0];
        assert_eq!(run.kind, AgentTaskKind::Skill);
        assert_eq!(run.status, AgentRunStatus::Failed);
        assert_eq!(run.agent_spec_id.as_deref(), Some("main.default"));
        assert!(
            run.error
                .as_ref()
                .is_some_and(|e| e.phase == "skill_prompt_stack"),
            "failed run should identify the PromptStack phase"
        );

        let events = state
            .agent_run_event_store
            .as_ref()
            .unwrap()
            .list_events_by_run(&run.id)
            .unwrap();
        assert!(
            events
                .iter()
                .any(|event| event.event_type == AgentRunEventType::RunFailed),
            "PromptStack failure should be observable as run.failed"
        );
        assert!(
            events.iter().all(|event| {
                let payload = event.payload.to_string();
                !payload.contains(raw_input)
                    && !payload.contains("raw LifeModel")
                    && !payload.contains("raw prompt")
            }),
            "Skill runtime events must keep payloads metadata-only"
        );
        assert_eq!(
            state
                .agent_run_event_store
                .as_ref()
                .unwrap()
                .count_events_by_type(AgentRunEventType::FallbackStarted)
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn skill_runtime_model_generation_failure_persists_failed_run_not_success() {
        openlife_core::ollama::set_ollama_cache_ttl_seconds(0);
        openlife_core::ollama::set_ollama_base_url("http://127.0.0.1:9");
        let state = test_app_state();
        let raw_input = "model failure raw prompt sentinel";

        let result =
            run_skill_with_state("weekly_review".into(), json!({"text": raw_input}), &state).await;
        openlife_core::ollama::set_ollama_base_url("");
        openlife_core::ollama::set_ollama_cache_ttl_seconds(10);
        let err = result.expect_err("unavailable model backend must fail the skill run");

        assert!(
            err.message().contains("Skill model generation failed"),
            "model failure should be explicit, got: {}",
            err.message()
        );

        let runs = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_runs(10, 0)
            .unwrap();
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(run.status, AgentRunStatus::Failed);
        assert_ne!(run.status, AgentRunStatus::Completed);
        assert_eq!(run.agent_spec_id.as_deref(), Some("main.default"));
        assert!(
            run.error
                .as_ref()
                .is_some_and(|e| e.phase == "skill_model_generation"),
            "model failure should identify skill_model_generation phase"
        );

        let events = state
            .agent_run_event_store
            .as_ref()
            .unwrap()
            .list_events_by_run(&run.id)
            .unwrap();
        assert!(
            events
                .iter()
                .any(|event| event.event_type == AgentRunEventType::RunFailed),
            "model generation failure should be observable as run.failed"
        );
        assert!(
            events
                .iter()
                .all(|event| !event.payload.to_string().contains(raw_input)),
            "model failure event payload must not include raw prompt text"
        );
    }

    #[test]
    fn skill_runtime_agent_spec_restricted_toolset_blocks_disallowed_skill_tools() {
        let manifest = SkillManifest {
            id: "tool_skill".into(),
            name: "Tool Skill".into(),
            description: "Requires a governed tool".into(),
            required_context: vec![],
            allowed_tools: vec!["web.search".into()],
            execution_budget: Default::default(),
            input_schema: json!({}),
            output_schema: json!({}),
            proposal_policy: "review_required".into(),
        };
        let spec = AgentSpec::default_main_spec()
            .with_allowed_tools(vec!["goal.read".into()])
            .with_privacy_policy(PrivacyPolicy::LocalOnly);

        let err = validate_skill_agent_spec_tool_policy(&manifest, &spec)
            .expect_err("restricted AgentSpec must block undeclared skill tools");
        assert!(
            err.message().contains("web.search") && err.message().contains("AgentSpec"),
            "tool governance error should be readable, got: {}",
            err.message()
        );
    }

    #[test]
    fn skill_runtime_success_response_shape_stays_frontend_compatible() {
        let response = super::SkillRunResponse {
            run_id: "run-skill-1".into(),
            status: "completed".into(),
            summary: "Skill completed".into(),
            generated_proposals: vec!["proposal-skill-1".into()],
        };

        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["runId"], "run-skill-1");
        assert_eq!(value["status"], "completed");
        assert_eq!(value["summary"], "Skill completed");
        assert_eq!(value["generatedProposals"][0], "proposal-skill-1");
        assert!(value.get("run_id").is_none());
        assert!(value.get("generated_proposals").is_none());
    }
}
