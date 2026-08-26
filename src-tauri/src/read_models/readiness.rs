use crate::errors::AppError;
use crate::AppState;
use openlife_core::ollama::inspect_ollama_status_for_generation;
use std::sync::Arc;

/// Minimal readiness facts consumed by product projections. This is not a
/// general diagnostics surface and intentionally omits raw paths, internal
/// router descriptions, audit contents, and compatibility-store counters.
#[derive(Debug, Clone)]
pub(crate) struct ProductReadinessSnapshot {
    pub persistence_health: crate::persistence_coordinator::PersistenceHealthSnapshot,
    pub chat_ready: bool,
    pub usage_ready: bool,
    pub life_model_ready: bool,
    pub model_empty: bool,
    pub database_status: String,
    pub vector_corrupt_embedding_count: usize,
    pub readiness_issues: Vec<String>,
    pub usage_readiness_issues: Vec<String>,
}

pub(crate) async fn load_product_readiness(
    state: &Arc<AppState>,
) -> Result<ProductReadinessSnapshot, AppError> {
    let persistence_health = state.persistence_coordinator.snapshot();
    let provider_runtime = state.provider_runtime_snapshot().await;
    let config = &provider_runtime.config;
    let provider_registry = crate::provider_registry::provider_profile_registry(state).await;
    let (selected_cloud_provider, cloud_configured, cloud_ready, cloud_error) =
        match provider_registry {
            Ok(registry) => match registry.default_profile_id.as_deref().and_then(|selected| {
                registry
                    .profiles
                    .iter()
                    .find(|profile| profile.profile_id == selected)
            }) {
                Some(profile) if profile.endpoint_class == "cloud" => (
                    profile.display_name.clone(),
                    true,
                    profile.availability == "ready",
                    profile.unavailable_reason.clone(),
                ),
                _ => ("未选择云端 Provider".into(), false, false, None),
            },
            Err(error) => ("Provider 状态未知".into(), false, false, Some(error)),
        };

    let ollama_status = inspect_ollama_status_for_generation(
        &config.local_model,
        provider_runtime.scheduler.provider_config_generation(),
    )
    .await;
    let ollama_online = ollama_status.resolved_model.is_some();

    let (life_model_ready, model_empty) = if state
        .persistence_coordinator
        .require_trusted_read("LifeModelFileStore")
        .is_ok()
    {
        match state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID)
        {
            Ok(model) => (
                true,
                model
                    .as_ref()
                    .is_none_or(|version| version.document.is_empty()),
            ),
            Err(_) => (false, true),
        }
    } else {
        (false, true)
    };

    let chat_session_count = if state
        .persistence_coordinator
        .require_trusted_read("ConversationStore")
        .is_ok()
    {
        match state.conversation_store.as_ref() {
            Some(store) => store
                .lock()
                .await
                .list_conversations(false, 1000)
                .map(|conversations| conversations.len())
                .unwrap_or_default(),
            None => 0,
        }
    } else {
        0
    };

    let (
        memory_chunk_count,
        vector_corrupt_embedding_count,
        vector_unknown_profile_count,
        vector_profile_dimension_mismatch_count,
    ) = if state
        .persistence_coordinator
        .require_trusted_read("VectorStore")
        .is_ok()
    {
        match state.vector_store.lock().await.integrity_report() {
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
    };

    let mut readiness_issues = Vec::new();
    if !ollama_online && !cloud_configured {
        readiness_issues
            .push("聊天不可用：未检测到可用 Ollama 本地模型，也没有配置云端 API Key。".into());
    } else if !ollama_online && cloud_configured && !cloud_ready {
        readiness_issues.push(format!(
            "聊天不可用：未检测到可用 Ollama 本地模型，{selected_cloud_provider} Profile 尚未就绪（{}）。",
            cloud_error.as_deref().unwrap_or("provider_profile_unavailable")
        ));
    }
    if !life_model_ready {
        readiness_issues.push("LifeModel 读取失败；个性化保持不可用。".into());
    }
    if config.prefer_local_model && !ollama_online && !cloud_ready {
        readiness_issues.push(format!(
            "当前设置为优先本地模型，但未找到可用模型：{}。",
            config.local_model
        ));
    }
    if cloud_configured && !cloud_ready {
        readiness_issues.push(format!(
            "{selected_cloud_provider} Profile 尚未完成连接测试；云端模型保持不可用。"
        ));
    }
    if vector_corrupt_embedding_count > 0 {
        readiness_issues.push(format!(
            "检测到 {vector_corrupt_embedding_count} 条向量记忆索引损坏；语义记忆可能不完整。"
        ));
    }
    if vector_unknown_profile_count > 0 || vector_profile_dimension_mismatch_count > 0 {
        readiness_issues
            .push("向量索引包含未知或不兼容的 embedding profile；语义检索需要重建。".into());
    }
    if chat_session_count > 0 && memory_chunk_count == 0 {
        readiness_issues
            .push("已有对话，但语义记忆索引为空；聊天可继续，长期记忆能力会受限。".into());
    }
    if !state.startup_warnings.is_empty() {
        readiness_issues.push("一个或多个持久化 owner 在启动时降级。".into());
    }
    if !persistence_health.canonical_writes_allowed {
        readiness_issues
            .push("canonical 持久化不可写；Provider、Tool 与 durable effects 保持关闭。".into());
    }

    let chat_ready = persistence_health.provider_dispatch_allowed
        && life_model_ready
        && (ollama_online || cloud_ready);
    let mut usage_readiness_issues = Vec::new();
    if !chat_ready {
        usage_readiness_issues.push("核心聊天链路尚未就绪。".into());
    }
    if model_empty {
        usage_readiness_issues.push("LifeModel 尚未建立。".into());
    }
    if chat_session_count == 0 && !model_empty {
        usage_readiness_issues.push("尚未开始任何对话。".into());
    }
    if memory_chunk_count == 0 && chat_session_count > 0 {
        usage_readiness_issues.push("长期记忆索引尚未建立。".into());
    }

    let database_status = if persistence_health.canonical_writes_allowed {
        "ok"
    } else {
        "degraded"
    }
    .to_string();
    let usage_ready = chat_ready
        && !model_empty
        && chat_session_count > 0
        && persistence_health.live_or_canonical_credit_eligible
        && vector_corrupt_embedding_count == 0
        && vector_unknown_profile_count == 0
        && vector_profile_dimension_mismatch_count == 0
        && memory_chunk_count > 0;

    Ok(ProductReadinessSnapshot {
        persistence_health,
        chat_ready,
        usage_ready,
        life_model_ready,
        model_empty,
        database_status,
        vector_corrupt_embedding_count,
        readiness_issues,
        usage_readiness_issues,
    })
}
