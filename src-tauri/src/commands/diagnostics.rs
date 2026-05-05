use crate::errors::AppError;
use crate::storage::{app_data_dir, load_onboarding_status_from_path, onboarding_status_path};
use crate::{AppState, BuilderCompletion, SystemDiagnostics};
use openlife_core::ollama::resolve_ollama_model;
use openlife_core::router::RouterStatus;
use std::sync::Arc;
use tauri::State;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerConfigResponse {
    pub local_model: String,
    pub prefer_local: bool,
}

#[tauri::command]
pub async fn get_system_diagnostics(
    state: State<'_, Arc<AppState>>,
) -> Result<SystemDiagnostics, AppError> {
    let router = {
        let status = state.intent_router.lock().await.status();
        status
    };
    let (mcp_server_count, mcp_tool_count) = {
        let registry = state.mcp_registry.lock().await;
        (
            registry.list_servers().len(),
            registry.list_all_tools().len(),
        )
    };
    let (mcp_recent_audit_count, mcp_recent_pii_count) = {
        let audit = state.mcp_audit_store.lock().await;
        let logs = audit.list_logs(50).map_err(AppError::from)?;
        let pii_count = logs.iter().filter(|log| log.pii_found).count();
        (logs.len(), pii_count)
    };
    let (memory_chunk_count, vector_corrupt_embedding_count) = {
        let store = state.vector_store.lock().await;
        let report = store.integrity_report().map_err(AppError::from)?;
        (
            report.total_chunks as usize,
            report.corrupt_embedding_count as usize,
        )
    };
    let (unfinished_builder_sessions, pending_builder_review_sessions) = {
        let store = state.builder_session_store.lock().await;
        let sessions = store
            .list_unfinished_sessions()
            .map_err(AppError::from)?;
        let pending_review = sessions
            .iter()
            .filter(|session| session.finished && !session.pending_signals.is_empty())
            .count();
        (sessions.len(), pending_review)
    };
    let (
        local_model,
        prefer_local_model,
        cloud_api_configured,
        cloud_provider,
        cloud_api_validated,
        cloud_api_last_error,
        embedding_enabled,
    ) = {
        let cfg = state.config.lock().await;
        let effective_key = cfg.effective_cloud_api_key();
        let provider = cfg.effective_provider_label();
        let base_ready = !cfg.llm.openai_base.trim().is_empty()
            || matches!(
                cfg.llm.provider.as_str(),
                "deepseek"
                    | "openrouter"
                    | "openai"
                    | "siliconflow"
                    | "moonshot"
                    | "dashscope"
                    | "zhipu"
            );
        let model_ready = !cfg.llm.chat_model.trim().is_empty();
        let configured = !effective_key.trim().is_empty() && base_ready && model_ready;
        (
            cfg.local_model.clone(),
            cfg.prefer_local_model,
            configured,
            provider,
            configured,
            None,
            cfg.llm.embedding_enabled,
        )
    };
    let resolved_local_model = resolve_ollama_model(&local_model).await;
    let ollama_online = resolved_local_model.is_some();
    let snapshot_count = {
        let vm = state.version_manager.lock().await;
        vm.list_versions()
            .map(|versions| versions.len())
            .unwrap_or_default()
    };
    let (life_model_ready, model_empty, builder_completion) = {
        let manager = state.life_model_manager.lock().await;
        match manager.load() {
            Ok(model) => {
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
            Err(_) => (
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
        let store = state.memory_store.lock().await;
        match store.list_chat_sessions(1000) {
            Ok(sessions) => sessions.len(),
            Err(_) => 0,
        }
    };
    let (agent_run_count, agent_run_store_status) = {
        if let Some(ref agent_run_store_arc) = state.agent_run_store {
            let store = agent_run_store_arc.lock().await;
            match store.run_count() {
                Ok(count) => (count as usize, "ok".to_string()),
                Err(_) => (0, "error".to_string()),
            }
        } else {
            (0, "disabled".to_string())
        }
    };
    let onboarding_completed =
        load_onboarding_status_from_path(&onboarding_status_path()).completed;
    let mut readiness_issues = Vec::new();
    if !ollama_online && !cloud_api_configured {
        readiness_issues
            .push("聊天不可用：未检测到可用 Ollama 本地模型，也没有配置云端 API Key。".to_string());
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
    if prefer_local_model && !ollama_online && !cloud_api_configured {
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
        let current_model = {
            let cfg = state.config.lock().await;
            cfg.llm.chat_model.clone()
        };
        if current_model.to_lowercase().contains("reasoner") {
            readiness_issues.push(
                "当前云端聊天模型是 DeepSeek 推理模型 deepseek-reasoner。为保证桌面端实时对话体验，主聊天流会自动切到 deepseek-chat；如果你想要更稳定的试用体验，也建议直接在设置页把模型改成 deepseek-chat。".to_string(),
            );
        }
        if embedding_enabled {
            readiness_issues.push(
                "当前是 DeepSeek 云端配置，远端 embedding 会自动关闭并回退到本地/Ollama 或哈希向量；如果历史聊天很多但记忆索引仍为空，请去设置页恢复控制台重建向量索引。".to_string(),
            );
        }
    }
    if vector_corrupt_embedding_count > 0 {
        readiness_issues.push(format!(
            "检测到 {} 条向量记忆索引损坏，记忆检索可能不完整；请在设置页导出备份后重建记忆索引。",
            vector_corrupt_embedding_count
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
    let chat_ready = life_model_ready && (ollama_online || cloud_api_configured);

    let mut beta_readiness_issues = Vec::new();
    if !chat_ready {
        beta_readiness_issues
            .push("核心聊天链路未就绪，请先修复试用就绪检查中的问题。".to_string());
    }
    if model_empty && pending_builder_review_sessions > 0 {
        beta_readiness_issues.push(format!(
            "Builder 中仍有 {} 个待确认 Review：请先回到 Builder 审阅并应用结果，再验证个性化体验。",
            pending_builder_review_sessions
        ));
    } else if model_empty && unfinished_builder_sessions > 0 {
        beta_readiness_issues.push(
            "Builder 中仍有未完成或待确认的构建会话：请先回到 Builder 完成 Review 并应用结果，再验证个性化体验。".to_string(),
        );
    }
    if model_empty {
        beta_readiness_issues.push(
            "人生模型尚未构建：请通过「构建」模式创建初始模型，以便获得个性化体验。".to_string(),
        );
    }
    if chat_session_count == 0 && !model_empty {
        beta_readiness_issues
            .push("尚未开始任何对话：建议到 Chat 页面进行一次对话，验证核心链路。".to_string());
    }
    if !cloud_api_configured {
        beta_readiness_issues.push("未配置云端 API：试用期间建议至少配置 OpenRouter 或 OpenAI API Key，以获得更稳定的体验。".to_string());
    }
    if !onboarding_completed {
        beta_readiness_issues.push(
            "首次启动引导尚未完成：请完成或跳过 Onboarding，以确保新用户路径可验证。".to_string(),
        );
    }
    if !state.startup_warnings.is_empty() {
        beta_readiness_issues.push(
            "数据存储曾在启动时降级：请先确认数据目录和数据库状态，再继续深度试用。".to_string(),
        );
    }
    if vector_corrupt_embedding_count > 0 {
        beta_readiness_issues
            .push("向量记忆索引存在损坏记录：建议重建索引后再验证长期记忆体验。".to_string());
    }
    if chat_session_count > 0 && memory_chunk_count == 0 {
        beta_readiness_issues.push(
            "已有聊天记录，但语义记忆索引仍为空：建议先重建记忆索引，再验证长期记忆与校准体验。"
                .to_string(),
        );
    }
    let (pending_proposal_count, high_risk_pending_proposal_count, proposal_store_status) = {
        if let Some(ref store_arc) = state.proposal_store {
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
            (0, 0, "disabled".to_string())
        }
    };

    let beta_ready = chat_ready
        && !model_empty
        && chat_session_count > 0
        && cloud_api_configured
        && onboarding_completed
        && state.startup_warnings.is_empty()
        && vector_corrupt_embedding_count == 0
        && !(chat_session_count > 0 && memory_chunk_count == 0);

    Ok(SystemDiagnostics {
        router,
        mcp_server_count,
        mcp_tool_count,
        mcp_recent_audit_count,
        mcp_recent_pii_count,
        memory_chunk_count,
        vector_corrupt_embedding_count,
        unfinished_builder_sessions,
        pending_builder_review_sessions,
        ollama_online,
        local_model,
        resolved_local_model,
        prefer_local_model,
        cloud_api_configured,
        cloud_provider,
        cloud_api_validated,
        cloud_api_last_error,
        chat_ready,
        readiness_issues,
        data_dir: app_data_dir().display().to_string(),
        active_data_dir: app_data_dir().display().to_string(),
        legacy_data_dir: None, // 已统一为 ai.openlife.app
        database_status: if state.startup_warnings.is_empty() {
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
        onboarding_completed,
        beta_ready,
        beta_readiness_issues,
        builder_completion,
        agent_run_count,
        agent_run_store_status,
        pending_proposal_count,
        high_risk_pending_proposal_count,
        proposal_store_status,
    })
}

#[tauri::command]
pub async fn check_ollama_status(state: State<'_, Arc<AppState>>) -> Result<bool, AppError> {
    let local_model = { state.scheduler.lock().await.local_model.clone() };
    Ok(openlife_core::ollama::is_ollama_available(&local_model).await)
}

#[tauri::command]
pub async fn get_router_status(state: State<'_, Arc<AppState>>) -> Result<RouterStatus, AppError> {
    let router = state.intent_router.lock().await;
    Ok(router.status())
}

#[tauri::command]
pub async fn get_scheduler_config(
    state: State<'_, Arc<AppState>>,
) -> Result<SchedulerConfigResponse, AppError> {
    let cfg = state.config.lock().await;
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
    let mut scheduler = state.scheduler.lock().await;
    scheduler.local_model = local_model.clone();
    scheduler.prefer_local = prefer_local;

    let data_dir = app_data_dir();
    let config_path = data_dir.join("config.yaml");

    let mut cfg = state.config.lock().await;
    cfg.local_model = local_model;
    cfg.prefer_local_model = prefer_local;
    cfg.save(&config_path).map_err(AppError::from)?;
    Ok(())
}
