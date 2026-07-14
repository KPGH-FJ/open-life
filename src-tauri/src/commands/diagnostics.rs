use crate::errors::AppError;
use crate::storage::app_data_dir;
use crate::{AppState, BuilderCompletion, OllamaModelInfo, SystemDiagnostics};
use openlife_core::ollama::inspect_ollama_status_for_generation;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_runtime_build_info(
) -> Result<crate::runtime_build_info::RuntimeBuildInfo, AppError> {
    Ok(crate::runtime_build_info::collect_runtime_build_info().await)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerConfigResponse {
    pub local_model: String,
    pub prefer_local: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRouterStatus {
    pub active_authority: String,
    pub authority_chain: Vec<String>,
    pub route_outputs: Vec<String>,
    pub app_state_old_routers_present: bool,
    pub diagnostics_surface: String,
}

fn current_policy_router_status() -> PolicyRouterStatus {
    PolicyRouterStatus {
        active_authority: "IntentFrame + PolicyRouter".into(),
        authority_chain: vec![
            "user_input".into(),
            "IntentFrame".into(),
            "PolicyRouter".into(),
            "AgentIngressDecision".into(),
            "OpenLifeTurnRuntime".into(),
            "MainChatKernel".into(),
        ],
        route_outputs: vec![
            "direct_answer".into(),
            "read_only_tool".into(),
            "proposal_only_write".into(),
            "plan_draft".into(),
            "ask_clarification".into(),
            "governed_blocker".into(),
            "confirmation_request".into(),
        ],
        app_state_old_routers_present: false,
        diagnostics_surface: "policy_router_status".into(),
    }
}

#[tauri::command]
pub async fn get_system_diagnostics(
    state: State<'_, Arc<AppState>>,
) -> Result<SystemDiagnostics, AppError> {
    get_system_diagnostics_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_system_diagnostics_with_state(
    state: &Arc<AppState>,
) -> Result<SystemDiagnostics, AppError> {
    let persistence_health = state.persistence_coordinator.snapshot();
    let policy_router = current_policy_router_status();
    let provider_runtime = state.provider_runtime_snapshot().await;
    let (mcp_server_count, mcp_tool_count) = {
        let registry = state.mcp_registry.lock().await;
        (
            registry.list_servers().len(),
            registry.list_all_tools().len(),
        )
    };
    let mcp_audit_projection = state.mcp_audit_read_gateway.diagnostic_counts(state).await;
    let (
        memory_chunk_count,
        vector_corrupt_embedding_count,
        vector_unknown_profile_count,
        vector_profile_dimension_mismatch_count,
    ) = {
        if state
            .persistence_coordinator
            .require_trusted_read("VectorStore")
            .is_ok()
        {
            let store = state.vector_store.lock().await;
            match store.integrity_report() {
                Ok(report) => (
                    report.total_chunks as usize,
                    report.corrupt_embedding_count as usize,
                    report.unknown_profile_count as usize,
                    report.profile_dimension_mismatch_count as usize,
                ),
                Err(_) => (0, 0, 0, 0),
            }
        } else {
            (0, 0, 0, 0)
        }
    };
    let (unfinished_builder_sessions, pending_builder_review_sessions) = {
        if state
            .persistence_coordinator
            .require_trusted_read("BuilderSessionStore")
            .is_ok()
        {
            let store = state.builder_session_store.lock().await;
            let sessions = store.list_unfinished_sessions().unwrap_or_default();
            let pending_review = sessions
                .iter()
                .filter(|session| session.finished && !session.pending_signals.is_empty())
                .count();
            (sessions.len(), pending_review)
        } else {
            (0, 0)
        }
    };
    let (
        local_model,
        prefer_local_model,
        cloud_api_configured,
        cloud_provider,
        cloud_api_validated,
        cloud_api_last_error,
        cloud_api_validation_status,
        cloud_api_validated_at,
        cloud_api_failed_at,
        cloud_api_validation_source,
        embedding_enabled,
    ) = {
        let cfg = &provider_runtime.config;
        let provider = cfg.effective_provider_label();
        let validation_load = crate::provider_validation::load_provider_validation_record_from_path(
            &crate::provider_validation::provider_validation_path(),
        );
        let mut validation = crate::provider_validation::summarize_loaded_provider_validation(
            cfg,
            &validation_load,
            chrono::Utc::now(),
        );
        if !provider_runtime.coherent {
            validation.validated = false;
            validation.status = "runtime_generation_incoherent";
            validation.last_error = Some("provider_runtime_generation_incoherent".into());
        }
        (
            cfg.local_model.clone(),
            cfg.prefer_local_model,
            validation.configured,
            provider,
            validation.validated,
            validation.last_error,
            validation.status.to_string(),
            validation.validated_at,
            validation.failed_at,
            validation.validation_source,
            cfg.llm.embedding_enabled,
        )
    };
    let ollama_status = inspect_ollama_status_for_generation(
        &local_model,
        provider_runtime.scheduler.provider_config_generation(),
    )
    .await;
    let ollama_service_online = ollama_status.server_online;
    let ollama_models = ollama_status
        .models
        .iter()
        .map(|(name, size)| OllamaModelInfo {
            name: name.clone(),
            size_mb: size / 1024 / 1024,
        })
        .collect::<Vec<_>>();
    let resolved_local_model = ollama_status.resolved_model;
    let ollama_online = resolved_local_model.is_some();
    let snapshot_count = {
        if state
            .persistence_coordinator
            .require_trusted_read("LifeModelFileStore")
            .is_ok()
        {
            let vm = state.version_manager.lock().await;
            vm.list_versions()
                .map(|versions| versions.len())
                .unwrap_or_default()
        } else {
            0
        }
    };
    let (life_model_ready, model_empty, builder_completion) = {
        let model = if state
            .persistence_coordinator
            .require_trusted_read("LifeModelFileStore")
            .is_ok()
        {
            state.life_model_manager.lock().await.load().ok()
        } else {
            None
        };
        match model {
            Some(model) => {
                let empty = model.is_effectively_empty();

                let completion = model.calculate_4d_completion();
                let lowest_dim = [
                    ("identity", completion.identity),
                    ("goals", completion.goals),
                    ("capabilities", completion.capabilities),
                    ("state", completion.state),
                ]
                .iter()
                .min_by_key(|(_, score)| *score)
                .map(|(name, _)| name.to_string());

                let builder_comp = BuilderCompletion {
                    identity: completion.identity as f32,
                    goals: completion.goals as f32,
                    capabilities: completion.capabilities as f32,
                    state: completion.state as f32,
                    overall: (completion.identity as f32
                        + completion.goals as f32
                        + completion.capabilities as f32
                        + completion.state as f32)
                        / 4.0,
                    lowest_dimension: lowest_dim,
                };
                (true, empty, builder_comp)
            }
            None => (
                false,
                true,
                BuilderCompletion {
                    identity: 0.0,
                    goals: 0.0,
                    capabilities: 0.0,
                    state: 0.0,
                    overall: 0.0,
                    lowest_dimension: None,
                },
            ),
        }
    };
    let chat_session_count = {
        if state
            .persistence_coordinator
            .require_trusted_read("MemoryStore")
            .is_ok()
        {
            let store = state.memory_store.lock().await;
            store
                .list_chat_sessions(1000)
                .map(|sessions| sessions.len())
                .unwrap_or_default()
        } else {
            0
        }
    };
    let (agent_run_count, agent_run_store_status) = {
        if state
            .persistence_coordinator
            .require_trusted_read("AgentRunStore")
            .is_err()
        {
            (0, "unavailable".to_string())
        } else if let Some(ref agent_run_store_arc) = state.agent_run_store {
            let store = agent_run_store_arc.lock().await;
            match store.run_count() {
                Ok(count) => (count as usize, "ok".to_string()),
                Err(_) => (0, "error".to_string()),
            }
        } else {
            (0, "unavailable".to_string())
        }
    };
    let mut readiness_issues = Vec::new();
    if !ollama_online && !cloud_api_configured {
        readiness_issues
            .push("聊天不可用：未检测到可用 Ollama 本地模型，也没有配置云端 API Key。".to_string());
    } else if !ollama_online && cloud_api_configured && !cloud_api_validated {
        readiness_issues.push(format!(
            "聊天不可用：未检测到可用 Ollama 本地模型，{} API 已配置但尚未通过真实连接验证。",
            cloud_provider
        ));
    }
    if !life_model_ready {
        readiness_issues
            .push("人生模型读取失败：请检查应用数据目录权限或重新保存人生模型。".to_string());
    }
    if model_empty && pending_builder_review_sessions > 0 {
        readiness_issues.push(format!(
            "检测到 {} 个待确认的人生模型 Review 会话。你其实已经完成了问题收集，下一步更适合先回到 Builder 审阅并应用这些建议，而不是重新开始构建。",
            pending_builder_review_sessions
        ));
    } else if model_empty && unfinished_builder_sessions > 0 {
        readiness_issues.push(format!(
            "检测到 {} 个未完成的人生模型构建会话，其中可能包含待确认的 Review 内容。建议先回到 Builder 继续或应用这些结果，再开始深度试用。",
            unfinished_builder_sessions
        ));
    }
    if prefer_local_model && !ollama_online && !cloud_api_validated {
        readiness_issues.push(format!(
            "当前设置为优先本地模型，但未找到可用模型：{}。",
            local_model
        ));
    }
    if cloud_api_configured && !cloud_api_validated {
        readiness_issues.push(format!(
            "{} API 尚未完成连接测试，若聊天失败请先到设置页测试连接。",
            cloud_provider
        ));
    }
    if cloud_provider == "DeepSeek" && local_model.is_empty() {
        let current_model = provider_runtime.config.llm.chat_model.clone();
        if current_model.to_lowercase().contains("reasoner") {
            readiness_issues.push(
                "当前云端聊天模型是 DeepSeek 推理模型 deepseek-reasoner；缓冲和流式请求都会使用这个已配置模型，不会在适配器中静默改写。首次可见输出可能更慢，如需更轻量的实时对话请在设置页显式选择 deepseek-chat。".to_string(),
            );
        }
        if embedding_enabled {
            readiness_issues.push(
                "当前聊天 Provider 是 DeepSeek，它不被声明为 embedding Provider；路由会在派发前显式选择本地确定性哈希，而不会拼接不存在的 /embeddings 端点。只有显式配置 Ollama 才会调用 Ollama。".to_string(),
            );
        }
    }
    if vector_corrupt_embedding_count > 0 {
        readiness_issues.push(format!(
            "检测到 {} 条向量记忆索引损坏，记忆检索可能不完整；请在设置页导出备份后重建记忆索引。",
            vector_corrupt_embedding_count
        ));
    }
    if vector_unknown_profile_count > 0 {
        readiness_issues.push(format!(
            "检测到 {} 条旧版向量缺少 embedding profile；其向量内容未必损坏，但必须重建后才能参与语义检索。",
            vector_unknown_profile_count
        ));
    }
    if vector_profile_dimension_mismatch_count > 0 {
        readiness_issues.push(format!(
            "检测到 {} 条向量的 profile 维度与实际数据不一致；已禁止静默过滤，请重建记忆索引。",
            vector_profile_dimension_mismatch_count
        ));
    }
    if chat_session_count > 0 && memory_chunk_count == 0 {
        readiness_issues.push(
            "检测到已有聊天会话，但语义记忆索引仍为空。聊天可以继续使用，但长期记忆、校准和相关建议会偏弱；建议去设置页恢复控制台重建向量索引。".to_string(),
        );
    }
    if !state.startup_warnings.is_empty() {
        readiness_issues.push(format!(
            "应用正在以降级数据模式运行：{}",
            state.startup_warnings.join("；")
        ));
    }
    if !persistence_health.canonical_writes_allowed {
        readiness_issues.push(format!(
            "持久化系统处于 {:?}：读取未观察到的事实必须视为 unknown，Provider、Tool 与所有 canonical write 已禁用。",
            persistence_health.mode
        ));
    }
    let cloud_chat_backend_available = cloud_api_validated;
    let chat_ready = persistence_health.provider_dispatch_allowed
        && life_model_ready
        && (ollama_online || cloud_chat_backend_available);

    let mut usage_readiness_issues = Vec::new();
    if !chat_ready {
        usage_readiness_issues
            .push("核心聊天链路未就绪，请先修复使用准备检查中的问题。".to_string());
    }
    if model_empty && pending_builder_review_sessions > 0 {
        usage_readiness_issues.push(format!(
            "人生模型构建中仍有 {} 个待确认项：请先到 Mailbox 审阅并应用结果，再验证个性化体验。",
            pending_builder_review_sessions
        ));
    } else if model_empty && unfinished_builder_sessions > 0 {
        usage_readiness_issues.push(
            "人生模型构建中仍有未完成会话：请先回到 Life Model 构建流程完成确认，再验证个性化体验。".to_string(),
        );
    }
    if model_empty {
        usage_readiness_issues.push(
            "人生模型尚未构建：请通过 Life Model 构建流程创建初始模型，以便获得个性化体验。"
                .to_string(),
        );
    }
    if chat_session_count == 0 && !model_empty {
        usage_readiness_issues
            .push("尚未开始任何对话：建议到 Companion 进行一次对话，验证核心链路。".to_string());
    }
    if cloud_api_configured && !cloud_api_validated {
        usage_readiness_issues.push(format!(
            "{} API 尚未通过真实连接验证：如需使用云端模型，请先在 Settings 测试连接。",
            cloud_provider
        ));
    }
    if !state.startup_warnings.is_empty() {
        usage_readiness_issues.push(
            "数据存储曾在启动时降级：请先确认数据目录和数据库状态，再继续深度试用。".to_string(),
        );
    }
    if vector_corrupt_embedding_count > 0 {
        usage_readiness_issues
            .push("向量记忆索引存在损坏记录：建议重建索引后再验证长期记忆体验。".to_string());
    }
    if vector_unknown_profile_count > 0 || vector_profile_dimension_mismatch_count > 0 {
        usage_readiness_issues.push(
            "向量索引存在未知或不兼容 embedding profile：关键词记忆仍可用，但语义检索需先重建。"
                .to_string(),
        );
    }
    if chat_session_count > 0 && memory_chunk_count == 0 {
        usage_readiness_issues.push(
            "已有聊天记录，但语义记忆索引仍为空：建议先重建记忆索引，再验证长期记忆与校准体验。"
                .to_string(),
        );
    }
    let (pending_proposal_count, high_risk_pending_proposal_count, proposal_store_status) = {
        if state
            .persistence_coordinator
            .require_trusted_read("ProposalStore")
            .is_err()
        {
            (0, 0, "unavailable".to_string())
        } else if let Some(ref store_arc) = state.proposal_store {
            let store = store_arc.lock().await;
            let pending = store.pending_count().unwrap_or(0) as usize;
            let high_risk = store
                .count_by_status_and_risk(
                    openlife_core::agent::ProposalStatus::Pending,
                    Some(openlife_core::agent::RiskLevel::High),
                )
                .unwrap_or(0) as usize;
            (pending, high_risk, "ok".to_string())
        } else {
            (0, 0, "unavailable".to_string())
        }
    };

    let usage_ready = chat_ready
        && !model_empty
        && chat_session_count > 0
        && state.startup_warnings.is_empty()
        && persistence_health.live_or_canonical_credit_eligible
        && vector_corrupt_embedding_count == 0
        && vector_unknown_profile_count == 0
        && vector_profile_dimension_mismatch_count == 0
        && memory_chunk_count != 0;
    let runtime_build_info = crate::runtime_build_info::collect_runtime_build_info().await;
    let scheduler_for_route_evidence = provider_runtime.scheduler.clone();
    let runtime_route_evidence =
        crate::main_chat_runtime_facts::build_settings_runtime_route_evidence(
            state,
            &provider_runtime.config,
            &scheduler_for_route_evidence,
        )
        .await;

    Ok(SystemDiagnostics {
        persistence_health: persistence_health.clone(),
        policy_router,
        mcp_server_count,
        mcp_tool_count,
        mcp_audit_read: mcp_audit_projection,
        memory_chunk_count,
        vector_corrupt_embedding_count,
        vector_unknown_profile_count,
        vector_profile_dimension_mismatch_count,
        unfinished_builder_sessions,
        pending_builder_review_sessions,
        ollama_service_online,
        ollama_online,
        local_model,
        resolved_local_model,
        prefer_local_model,
        cloud_api_configured,
        cloud_provider,
        cloud_api_validated,
        cloud_api_last_error,
        cloud_api_validation_status,
        cloud_api_validated_at,
        cloud_api_failed_at,
        cloud_api_validation_source,
        chat_ready,
        readiness_issues,
        data_dir: app_data_dir().display().to_string(),
        active_data_dir: app_data_dir().display().to_string(),
        database_status: if persistence_health.canonical_writes_allowed
            && state.startup_warnings.is_empty()
        {
            "ok".to_string()
        } else {
            "degraded".to_string()
        },
        startup_warnings: state.startup_warnings.clone(),
        snapshot_count,
        life_model_ready,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        model_empty,
        chat_session_count,
        usage_ready,
        usage_readiness_issues,
        builder_completion,
        ollama_models,
        agent_run_count,
        agent_run_store_status,
        pending_proposal_count,
        high_risk_pending_proposal_count,
        proposal_store_status,
        runtime_build_info,
        runtime_route_evidence,
    })
}

#[tauri::command]
pub async fn check_ollama_status(_state: State<'_, Arc<AppState>>) -> Result<bool, AppError> {
    Ok(openlife_core::ollama::is_ollama_server_online().await)
}

#[tauri::command]
pub async fn get_policy_router_status() -> Result<PolicyRouterStatus, AppError> {
    Ok(current_policy_router_status())
}

#[tauri::command]
pub async fn get_scheduler_config(
    state: State<'_, Arc<AppState>>,
) -> Result<SchedulerConfigResponse, AppError> {
    let cfg = state.provider_runtime_snapshot().await.config;
    Ok(SchedulerConfigResponse {
        local_model: cfg.local_model.clone(),
        prefer_local: cfg.prefer_local_model,
    })
}

#[tauri::command]
pub async fn set_scheduler_config(
    local_model: String,
    prefer_local: bool,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let _config_write_guard = crate::commands::settings::CONFIG_WRITE_COORDINATOR
        .lock()
        .await;
    let data_dir = app_data_dir();
    let config_path = data_dir.join("config.yaml");
    let mut cfg = state.provider_runtime_snapshot().await.config;
    cfg.local_model = local_model;
    cfg.prefer_local_model = prefer_local;
    cfg.save(&config_path).map_err(AppError::from)?;
    state.replace_provider_runtime_config(cfg).await;
    Ok(())
}
