use crate::errors::AppError;
use crate::main_chat_hs_runtime::build_chat_runtime_hs_packet;
use crate::AppState;
use openlife_core::agent::{
    behavior_checks_for_packet, AgentAction, AgentObservation, AgentProposal, AgentRun,
    AgentTaskKind, ContextSummary, ModelRoutePolicy, ProposalSource, RedactionLevel,
};
use openlife_core::skills::{
    build_skill_context, evaluate_skill_runtime_readiness, govern_skill_proposal_candidates,
    normalize_skill_output, SkillContextAssemblyInput, SkillContextSource, SkillExecutionStatus,
    SkillRuntimeReadinessReport, SkillSourceKind,
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

fn metadata_digest(value: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn input_text(input: &Value) -> String {
    input
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn metadata_warning_messages(warnings: &[String]) -> Vec<String> {
    warnings
        .iter()
        .enumerate()
        .map(|(index, warning)| {
            format!(
                "skill_warning index={} byte_count={} digest={}",
                index,
                warning.len(),
                metadata_digest(warning)
            )
        })
        .collect()
}

fn skill_context_summary(
    report: &openlife_core::skills::SkillContextReport,
    hs_packet: Option<&openlife_core::agent::RuntimeHSPacket>,
) -> ContextSummary {
    let memory_item = report.items.iter().find(|item| item.context_id == "memory");
    let local_only = hs_packet
        .map(|packet| {
            packet
                .selected_policies
                .iter()
                .any(|policy| policy.route == Some(ModelRoutePolicy::LocalOnly))
        })
        .unwrap_or(false);
    ContextSummary {
        life_model_empty: false,
        included_life_model_sections: report
            .items
            .iter()
            .filter(|item| item.available && item.context_id.starts_with("life_model."))
            .map(|item| item.context_id.clone())
            .collect(),
        memory_hit_count: memory_item.map(|item| item.item_count as i64).unwrap_or(0),
        memory_sources: memory_item.map(|item| item.ids.clone()).unwrap_or_default(),
        used_tools_prompt: false,
        redaction_applied: true,
        redaction_level: if local_only {
            RedactionLevel::LocalOnly
        } else {
            RedactionLevel::Summary
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRunResponse {
    pub run_id: String,
    pub status: String,
    pub summary: String,
    pub generated_proposals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRuntimeStatusReport {
    pub report_kind: String,
    pub readiness: SkillRuntimeReadinessReport,
    pub default_chat_unchanged: bool,
    pub migration_permission: bool,
    pub read_only: bool,
    pub runtime_execution_performed: bool,
    pub model_call_performed: bool,
    pub tool_call_performed: bool,
    pub business_writes_performed: bool,
    pub metadata_safe: bool,
    pub blockers: Vec<String>,
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
    Ok(registry.list())
}

#[tauri::command]
pub async fn get_skill_runtime_status(
    state: State<'_, Arc<AppState>>,
) -> Result<SkillRuntimeStatusReport, AppError> {
    get_skill_runtime_status_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_skill_runtime_status_with_state(
    state: &Arc<AppState>,
) -> Result<SkillRuntimeStatusReport, AppError> {
    let registry = state.skill_registry.lock().await;
    let readiness = evaluate_skill_runtime_readiness(&registry);
    Ok(SkillRuntimeStatusReport {
        report_kind: "w156.skillRuntimeStatus.v1".into(),
        default_chat_unchanged: readiness.default_chat_unchanged,
        migration_permission: readiness.migration_permission,
        read_only: true,
        runtime_execution_performed: false,
        model_call_performed: false,
        tool_call_performed: false,
        business_writes_performed: false,
        metadata_safe: readiness.metadata_safe,
        blockers: readiness.blockers.clone(),
        readiness,
    })
}

#[tauri::command]
pub async fn run_skill(
    skill_id: String,
    input: Value,
    state: State<'_, Arc<AppState>>,
) -> Result<SkillRunResponse, AppError> {
    let user_input = input_text(&input);
    let input_digest = metadata_digest(&user_input);

    let manifest = {
        let registry = state.skill_registry.lock().await;
        registry
            .get(&skill_id)
            .ok_or_else(|| AppError::not_found(format!("未知技能: {}", skill_id)))?
    };
    if manifest.source_kind == SkillSourceKind::Plugin
        && manifest.execution_status == SkillExecutionStatus::DisabledDeclarativeOnly
    {
        return Err(AppError::permission(format!(
            "plugin skill requires a configured executor/provider before it can run: {}",
            skill_id
        )));
    }
    if manifest.execution_budget.allow_writes {
        return Err(AppError::permission(format!(
            "skill write budget is not allowed: {}",
            skill_id
        )));
    }

    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };

    let mut context_sources = vec![
        SkillContextSource {
            context_id: "life_model.summary".into(),
            item_count: 1,
            ids: vec!["lifemodel.compat.summary".into()],
            timestamps: vec![life_model.metadata.updated_at.clone()],
            summary: format!(
                "goals_short={} goals_medium={} goals_long={} daily={} focus_set={}",
                life_model.goals.short_term.len(),
                life_model.goals.medium_term.len(),
                life_model.goals.long_term.len(),
                life_model.goals.daily.len(),
                !life_model.state.current_focus.trim().is_empty()
            ),
            raw_text: None,
            safe_excerpt_permitted: false,
        },
        SkillContextSource {
            context_id: "life_model.goals".into(),
            item_count: life_model.goals.short_term.len()
                + life_model.goals.medium_term.len()
                + life_model.goals.long_term.len()
                + life_model.goals.daily.len(),
            ids: vec!["lifemodel.compat.goals".into()],
            timestamps: vec![life_model.metadata.updated_at.clone()],
            summary: format!(
                "short={} medium={} long={} daily={}",
                life_model.goals.short_term.len(),
                life_model.goals.medium_term.len(),
                life_model.goals.long_term.len(),
                life_model.goals.daily.len()
            ),
            raw_text: serde_json::to_string(&life_model.goals).ok(),
            safe_excerpt_permitted: false,
        },
        SkillContextSource {
            context_id: "life_model.state".into(),
            item_count: life_model.state.focus_areas.len()
                + life_model.state.recent_events.len()
                + life_model.state.recent_reflections.len(),
            ids: vec!["lifemodel.compat.state".into()],
            timestamps: vec![life_model.metadata.updated_at.clone()],
            summary: format!(
                "energy={} stress={} focus_areas={} recent_events={}",
                life_model.state.health_status.energy_level,
                life_model.state.emotional_state.stress_level,
                life_model.state.focus_areas.len(),
                life_model.state.recent_events.len()
            ),
            raw_text: serde_json::to_string(&life_model.state).ok(),
            safe_excerpt_permitted: false,
        },
    ];

    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        match store.list_runs(10, 0) {
            Ok(runs) => {
                context_sources.push(SkillContextSource {
                    context_id: "agent_runs".into(),
                    item_count: runs.len(),
                    ids: runs.iter().take(8).map(|run| run.id.clone()).collect(),
                    timestamps: runs
                        .iter()
                        .take(8)
                        .map(|run| run.started_at.to_rfc3339())
                        .collect(),
                    summary: format!(
                        "recent_runs={} skill_runs={} generated_proposals={}",
                        runs.len(),
                        runs.iter()
                            .filter(|run| run.kind == AgentTaskKind::Skill)
                            .count(),
                        runs.iter()
                            .map(|run| run.generated_proposals.len())
                            .sum::<usize>()
                    ),
                    raw_text: None,
                    safe_excerpt_permitted: false,
                });
            }
            Err(e) => {
                context_sources.push(SkillContextSource {
                    context_id: "agent_runs".into(),
                    item_count: 0,
                    ids: vec![],
                    timestamps: vec![],
                    summary: format!("agent_runs_unavailable:{}", e),
                    raw_text: None,
                    safe_excerpt_permitted: false,
                });
            }
        }
    }

    {
        let memory = state.memory_store.lock().await;
        let memory_hits = if user_input.is_empty() {
            Ok(Vec::new())
        } else {
            memory.search_text_memories(None, &user_input, 5)
        };
        match memory_hits {
            Ok(hits) => {
                context_sources.push(SkillContextSource {
                    context_id: "memory".into(),
                    item_count: hits.len(),
                    ids: hits
                        .iter()
                        .take(8)
                        .map(|hit| format!("memory:{}", hit.chunk.id))
                        .collect(),
                    timestamps: vec![],
                    summary: format!("memory_hits={} query_digest={}", hits.len(), input_digest),
                    raw_text: Some(
                        hits.iter()
                            .map(|hit| hit.chunk.content.as_str())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                    safe_excerpt_permitted: false,
                });
            }
            Err(e) => {
                context_sources.push(SkillContextSource {
                    context_id: "memory".into(),
                    item_count: 0,
                    ids: vec![],
                    timestamps: vec![],
                    summary: format!("memory_unavailable:{}", e),
                    raw_text: None,
                    safe_excerpt_permitted: false,
                });
            }
        }

        if let Some(session_id) = input.get("sessionId").and_then(Value::as_str) {
            match memory.load_recent_messages(session_id, 8) {
                Ok(messages) => {
                    context_sources.push(SkillContextSource {
                        context_id: "chat_history".into(),
                        item_count: messages.len(),
                        ids: vec![format!("chat_session:{}", session_id)],
                        timestamps: vec![],
                        summary: format!(
                            "recent_chat_messages={} user_messages={}",
                            messages.len(),
                            messages
                                .iter()
                                .filter(|message| message.role == "user")
                                .count()
                        ),
                        raw_text: Some(
                            messages
                                .iter()
                                .map(|message| format!("{}:{}", message.role, message.content))
                                .collect::<Vec<_>>()
                                .join("\n"),
                        ),
                        safe_excerpt_permitted: false,
                    });
                }
                Err(e) => {
                    context_sources.push(SkillContextSource {
                        context_id: "chat_history".into(),
                        item_count: 0,
                        ids: vec![format!("chat_session:{}", session_id)],
                        timestamps: vec![],
                        summary: format!("chat_history_unavailable:{}", e),
                        raw_text: None,
                        safe_excerpt_permitted: false,
                    });
                }
            }
        }
    }

    let skill_context = build_skill_context(
        &manifest,
        SkillContextAssemblyInput {
            sources: context_sources,
            max_summary_chars: 160,
            max_excerpt_chars: 0,
        },
    );

    let (system_prompt, skill_prompt) = {
        let registry = state.skill_registry.lock().await;
        let system = registry
            .build_system_prompt(&skill_id)
            .map_err(AppError::from)?;
        let prompt = registry
            .build_skill_prompt(&skill_id, &input, &skill_context)
            .map_err(AppError::from)?;
        (system, prompt)
    };

    // 3. Create skill task with system prompt
    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Skill,
        session_id: format!("skill-{}", skill_id),
        user_text: user_input.clone(),
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
        layer: openlife_core::layer::Layer::L2,
    };

    let hs_packet =
        build_chat_runtime_hs_packet(&state.inner().clone(), &task, &life_model, "", None)
            .await
            .map_err(AppError::internal)?;

    // 5. Generate skill response from model
    let scheduler = state.scheduler.lock().await.clone();
    let model_life_model = openlife_core::life_model::LifeModel::default();
    let model_output = if let Some(ref packet) = hs_packet {
        scheduler
            .generate_with_hs_packet(task.messages.clone(), &model_life_model, None, packet)
            .await
            .map_err(AppError::from)?
    } else {
        scheduler
            .generate(task.messages.clone(), &model_life_model, None)
            .await
            .map_err(AppError::from)?
    };

    // 6. Parse and normalize JSON envelope
    let normalized = normalize_skill_output(&model_output);
    let mut envelope = normalized.envelope.clone();
    let parse_error = normalized.parse_error.clone();
    let governance = if parse_error.is_none() {
        govern_skill_proposal_candidates(&skill_id, &envelope.proposal_candidates)
    } else {
        govern_skill_proposal_candidates(&skill_id, &[])
    };
    envelope.warnings.extend(governance.warnings.clone());

    // 7. Create AgentRun
    let safe_user_input = format!(
        "skill_input skill_id={} byte_count={} digest={}",
        skill_id,
        user_input.len(),
        input_digest
    );
    let mut run = AgentRun::new_chat_run(&task.session_id, &safe_user_input);
    run.kind = AgentTaskKind::Skill;
    let mut model_route = scheduler.preview_chat_route(None).await;
    if hs_packet.as_ref().is_some_and(|packet| {
        packet
            .selected_policies
            .iter()
            .any(|policy| policy.route == Some(ModelRoutePolicy::LocalOnly))
    }) {
        model_route.provider = "ollama".into();
        model_route.route_type = "local".into();
        model_route.prefer_local = true;
        model_route.privacy_level = RedactionLevel::LocalOnly;
        model_route.fallback_reason = None;
        model_route.reason = format!("{}; skill_hs_local_only_enforced", model_route.reason);
    }
    let summary_digest = metadata_digest(&envelope.summary);
    let structured_output_digest =
        metadata_digest(&serde_json::to_string(&envelope.structured_output).unwrap_or_default());
    run.complete(
        &format!(
            "skill_output skill_id={} summary_digest={} structured_output_digest={} warning_count={}",
            skill_id,
            summary_digest,
            structured_output_digest,
            envelope.warnings.len()
        ),
        model_route,
        skill_context_summary(&skill_context.report, hs_packet.as_ref()),
    );
    run.hs_selection_audit = hs_packet.as_ref().map(|packet| packet.audit.clone());
    run.behavior_checks = hs_packet
        .as_ref()
        .map(behavior_checks_for_packet)
        .unwrap_or_default();
    run.warnings = metadata_warning_messages(&envelope.warnings);

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

    // 8. Generate proposals from envelope
    let mut generated = Vec::new();
    if parse_error.is_none() {
        if let Some(ref proposal_store_arc) = state.proposal_store {
            let store = proposal_store_arc.lock().await;
            for accepted in &governance.accepted {
                let mut proposal = AgentProposal::new(
                    accepted.proposal_type,
                    &accepted.candidate.affected_path,
                    accepted.candidate.after.clone(),
                    &accepted.candidate.reason,
                    accepted.candidate.confidence,
                    accepted.risk_level,
                    ProposalSource::SkillRuntime,
                );
                proposal.run_id = Some(run.id.clone());
                proposal.source_detail = Some(format!(
                    "skill_id={};candidate_digest={}",
                    skill_id,
                    metadata_digest(
                        &serde_json::to_string(&accepted.candidate.after).unwrap_or_default(),
                    )
                ));
                crate::life_model_write_gateway::stamp_lifemodel_proposal_base_hash_with_state(
                    state.inner(),
                    &mut proposal,
                )
                .await
                .map_err(AppError::from)?;
                let outcome = openlife_core::agent::ReviewWorkflow::new(&store)
                    .submit(
                        openlife_core::agent::DurableWriteRequest::from_agent_proposal(
                            openlife_core::agent::DurableWriteSource::SkillRuntime,
                            openlife_core::agent::DurableWriteSubject::from_proposal_type(
                                proposal.proposal_type,
                            ),
                            proposal.clone(),
                            "Skill runtime proposal is pending Review Center approval.",
                        ),
                    )
                    .map_err(AppError::from)?;
                generated.push(outcome.proposal_id().to_string());
                run.add_generated_proposal(outcome.proposal_id());
            }
        }
    }

    let skill_trace = serde_json::json!({
        "traceKind": "skill_runtime",
        "skillId": skill_id.clone(),
        "skillSourceKind": format!("{:?}", manifest.source_kind),
        "executionStatus": format!("{:?}", manifest.execution_status),
        "parseStatus": normalized.parse_status,
        "validationStatus": normalized.validation_status,
        "redactionStatus": normalized.redaction_status.clone(),
        "metadata": normalized.metadata.clone(),
        "contextReport": skill_context.report.clone(),
        "proposalCandidateCount": envelope.proposal_candidates.len(),
        "acceptedProposalCandidateCount": governance.accepted.len(),
        "skippedProposalCandidateCount": governance.skipped.len(),
        "generatedProposalIds": generated.clone(),
        "warningCount": envelope.warnings.len(),
        "metadataSafe": true,
        "containsRawContent": false,
        "guidanceConsumptionMode": "disabled",
    });

    run.actions.push(AgentAction {
        id: local_id("action"),
        action_type: "skill_run".into(),
        target: Some(skill_id.clone()),
        input: serde_json::json!({
            "skillId": skill_id.clone(),
            "inputDigest": input_digest.clone(),
            "inputByteCount": user_input.len(),
            "contextReport": skill_context.report.clone(),
            "metadataSafe": true,
            "containsRawContent": false,
        }),
        output: Some(serde_json::json!({
            "summaryDigest": summary_digest.clone(),
            "structuredOutputDigest": structured_output_digest.clone(),
            "proposalCandidateCount": envelope.proposal_candidates.len(),
            "warningCount": envelope.warnings.len(),
            "skillTrace": skill_trace.clone(),
            "metadataSafe": true,
            "containsRawContent": false,
        })),
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
        react_trace: None,
    });
    let observation_content = format!(
        "skill_observation skill_id={} summary_digest={} structured_output_digest={} warning_count={} generated_proposal_count={}",
        skill_id,
        summary_digest,
        structured_output_digest,
        envelope.warnings.len(),
        generated.len()
    );

    run.observations.push(AgentObservation {
        id: local_id("observation"),
        action_id: run.actions.first().map(|a| a.id.clone()),
        content: observation_content,
        source: format!("skill:{}", skill_id),
        structured_result: Some(serde_json::json!({
            "summaryDigest": summary_digest,
            "structuredOutputDigest": structured_output_digest,
            "warningCount": envelope.warnings.len(),
            "generatedProposalIds": run.generated_proposals.clone(),
            "skillTrace": skill_trace.clone(),
            "metadataSafe": true,
            "containsRawContent": false,
        })),
        timestamp: chrono::Utc::now(),
        react_trace: None,
    });

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

    // Plugin tools require a configured executor/provider; do not register them to McpRegistry.
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
                    let mut skill_clone = skill
                        .clone()
                        .as_plugin_declarative_only(&record.manifest.id);
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
                // Plugin tools require a configured executor/provider; do not register to McpRegistry.
                // Register skills only
                let mut skill_reg = state.skill_registry.lock().await;
                for skill in &record.manifest.skills {
                    let mut skill_clone = skill.clone().as_plugin_declarative_only(&plugin_id);
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
    use super::*;
    use openlife_core::skills::{SkillExecutionStatus, SkillSourceKind};

    async fn side_effect_counts(state: &Arc<AppState>) -> (usize, usize, usize) {
        let run_count = if let Some(store) = state.agent_run_store.as_ref() {
            store.lock().await.list_runs(100, 0).unwrap().len()
        } else {
            0
        };
        let proposal_count = if let Some(store) = state.proposal_store.as_ref() {
            store.lock().await.list_all_proposals(100, 0).unwrap().len()
        } else {
            0
        };
        let memory_count = state
            .memory_store
            .lock()
            .await
            .export_all_messages()
            .unwrap()
            .len();
        (run_count, proposal_count, memory_count)
    }

    #[tokio::test]
    async fn skill_runtime_status_is_read_only_and_ready_for_builtins() {
        let state = crate::test_utils::test_app_state();
        let before = side_effect_counts(&state).await;

        let report = get_skill_runtime_status_with_state(&state).await.unwrap();

        let after = side_effect_counts(&state).await;
        assert_eq!(before, after);
        assert!(report.read_only);
        assert!(report.readiness.ready);
        assert!(report.metadata_safe);
        assert!(!report.migration_permission);
        assert!(!report.runtime_execution_performed);
        assert!(!report.model_call_performed);
        assert!(!report.tool_call_performed);
        assert!(!report.business_writes_performed);
    }

    #[tokio::test]
    async fn skill_runtime_status_reports_unsafe_plugin_blocker() {
        let state = crate::test_utils::test_app_state();
        {
            let mut registry = state.skill_registry.lock().await;
            let mut plugin_skill = openlife_core::skills::SkillRegistry::built_in()
                .get("goal_breakdown")
                .unwrap();
            plugin_skill.id = "plugin:demo:goal_breakdown".into();
            plugin_skill.source_kind = SkillSourceKind::Plugin;
            plugin_skill.execution_status = SkillExecutionStatus::ExecutableBuiltIn;
            plugin_skill.execution_budget.allow_writes = true;
            plugin_skill.allowed_tools = vec!["plugin.demo.write_file".into()];
            registry.register(plugin_skill);
        }

        let report = get_skill_runtime_status_with_state(&state).await.unwrap();

        assert!(!report.readiness.ready);
        assert!(report
            .blockers
            .iter()
            .any(|blocker| blocker.contains("plugin_skill_executable_without_governance")));
        assert!(
            report
                .readiness
                .plugin_boundary_summary
                .plugin_tools_declarative_only
        );
    }

    #[test]
    fn skill_trace_warning_messages_are_digest_only() {
        let warnings = vec![
            "raw skill warning with jane@example.com and SECRET-123".to_string(),
            "another raw warning".to_string(),
        ];

        let safe = metadata_warning_messages(&warnings);
        let serialized = serde_json::to_string(&safe).unwrap();

        assert_eq!(safe.len(), 2);
        assert!(serialized.contains("digest=sha256:"));
        assert!(serialized.contains("byte_count="));
        assert!(!serialized.contains("jane@example.com"));
        assert!(!serialized.contains("SECRET-123"));
        assert!(!serialized.contains("another raw warning"));
    }
}
