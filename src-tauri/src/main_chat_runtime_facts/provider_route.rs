use openlife_core::agent::{AgentRunStatus, ModelRouteTrace};
use openlife_core::config::AppConfig;
use openlife_core::scheduler::InferenceScheduler;
use std::sync::Arc;

use super::contract::{
    bounded_runtime_fact_label, label_or_unknown, matches_exact_runtime_fact_phrase,
    merge_json_object, trim_outer_punctuation, MainChatProviderRouteIntent,
    MainChatRuntimeFactAnswer, MainChatRuntimeFactBinding,
    RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_MODEL,
    RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER, RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL,
    RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED, RUNTIME_FACT_KEY_PROVIDER_CURRENT_PROVIDER,
    RUNTIME_FACT_KEY_PROVIDER_CURRENT_ROUTE_TYPE, RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_MODEL,
    RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_PROVIDER,
    RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_RUN_ID, RUNTIME_FACT_KEY_PROVIDER_PLANNED_MODEL,
    RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER, RUNTIME_FACT_KEY_PROVIDER_PLANNED_ROUTE_TYPE,
    RUNTIME_FACT_KEY_PROVIDER_PREFLIGHT_STATUS,
};
use super::registry::provider_route_fact_keys;
use crate::AppState;

#[derive(Debug, Clone)]
struct ProviderRouteFactSnapshot {
    provider: Option<String>,
    model: Option<String>,
    route_type: Option<String>,
    run_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ProviderPreflightFactSnapshot {
    status: String,
    blockers: Vec<String>,
}

pub(crate) async fn provider_route_fact_should_block_before_model(
    state: &Arc<AppState>,
    scheduler: &InferenceScheduler,
) -> bool {
    let config = state.config.lock().await.clone();
    let planned = planned_route_without_probe(scheduler);
    let preflight = provider_preflight_snapshot(&config, scheduler, &planned);
    preflight.status == "blocked"
}

pub(crate) async fn resolve_provider_route_fact_answer(
    user_text: &str,
    state: &Arc<AppState>,
    scheduler: &InferenceScheduler,
    session_id: &str,
    current_route: Option<ModelRouteTrace>,
    current_model_generated: bool,
    scheduler_generation_called: bool,
    provider_generation_path: &str,
) -> Option<MainChatRuntimeFactAnswer> {
    let intent = classify_provider_route_query(user_text)?;
    let config = state.config.lock().await.clone();
    let planned_route = planned_route_without_probe(scheduler);
    let preflight = provider_preflight_snapshot(&config, scheduler, &planned_route);
    let configured = ProviderRouteFactSnapshot {
        provider: Some(bounded_runtime_fact_label(&config.llm.provider)),
        model: Some(bounded_runtime_fact_label(&config.llm.chat_model)),
        route_type: None,
        run_id: None,
    };
    let planned = route_snapshot_from_trace(&planned_route, None);
    let current = if current_model_generated {
        current_route
            .as_ref()
            .map(|route| route_snapshot_from_trace(route, None))
            .unwrap_or_else(no_current_generation_snapshot)
    } else {
        no_current_generation_snapshot()
    };
    let last_completed_generation = last_completed_generation_snapshot(state, session_id).await;
    let last_turn = last_turn_snapshot(state, session_id).await;
    let facts = provider_route_fact_bindings(
        &configured,
        &current,
        &last_completed_generation,
        &planned,
        current_model_generated,
        &preflight,
    );
    let fact_keys = provider_route_fact_keys();
    let route_labels = provider_route_labels(
        &configured,
        &current,
        &last_completed_generation,
        &planned,
        current_model_generated,
        &preflight,
    );
    let reply = provider_route_reply(
        intent,
        &configured,
        &current,
        &last_completed_generation,
        &planned,
        &last_turn,
        current_model_generated,
        &preflight,
    );
    let mut extra_metadata = serde_json::json!({
        "providerGenerationPath": provider_generation_path,
        "modelGenerated": current_model_generated,
        "schedulerGenerationCalled": scheduler_generation_called,
        "toolCalled": false,
        "directWritesExecuted": false,
        "legacyFallbackUsed": false,
        "configuredProvider": configured.provider.clone(),
        "configuredModel": configured.model.clone(),
        "configuredDefaultRouteLabel": route_labels.configured.clone(),
        "currentTurnGenerationProvider": current.provider.clone(),
        "currentTurnGenerationModel": current.model.clone(),
        "currentTurnGenerationRouteType": current.route_type.clone().unwrap_or_else(|| "none".into()),
        "currentTurnGenerationModelGenerated": current_model_generated,
        "currentTurnGenerationRouteLabel": route_labels.current.clone(),
        "lastCompletedGenerationProvider": last_completed_generation.as_ref().and_then(|route| route.provider.clone()),
        "lastCompletedGenerationModel": last_completed_generation.as_ref().and_then(|route| route.model.clone()),
        "lastCompletedGenerationRunId": last_completed_generation.as_ref().and_then(|route| route.run_id.clone()),
        "lastCompletedGenerationRouteLabel": route_labels.last_completed.clone(),
        "plannedRouteIfModelNeededProvider": planned.provider.clone(),
        "plannedRouteIfModelNeededModel": planned.model.clone(),
        "plannedRouteIfModelNeededRouteType": planned.route_type.clone(),
        "plannedRouteIfModelNeededLabel": route_labels.planned.clone(),
        "providerPreflightStatus": preflight.status.clone(),
        "providerPreflightBlockers": preflight.blockers.clone(),
        "providerPreflightIsInvocationProof": false,
        "routeLabels": [
            route_labels.current.clone(),
            route_labels.last_completed.clone(),
            route_labels.configured.clone(),
            route_labels.planned.clone(),
        ],
        "uiPrimarySourceChip": "运行时路线",
        "uiStatus": if preflight.status == "blocked" { "restricted" } else { "completed" },
    });
    if let Some(last_turn) = last_turn {
        merge_json_object(
            &mut extra_metadata,
            serde_json::json!({
                "lastTurnProvider": last_turn.provider,
                "lastTurnModel": last_turn.model,
                "lastTurnRouteType": last_turn.route_type,
                "lastTurnModelGenerated": route_snapshot_is_model_generation(&last_turn),
            }),
        );
    }

    Some(MainChatRuntimeFactAnswer {
        reply,
        intent: intent.as_str().into(),
        fact_keys,
        facts,
        observed_at: Some(chrono::Utc::now().to_rfc3339()),
        source: vec![
            "provider_route",
            "agent_run",
            "config",
            "model_router",
            "provider_preflight",
        ],
        authority: if preflight.status == "blocked" {
            "policy"
        } else {
            "run_trace"
        },
        freshness: "run_trace",
        visibility: vec!["answer", "ui_badge", "trace_only"],
        privacy: vec!["internal"],
        timezone: None,
        trace_gap: false,
        extra_metadata,
    })
}

pub(crate) fn classify_provider_route_query(
    user_text: &str,
) -> Option<MainChatProviderRouteIntent> {
    let normalized = user_text.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    let compact = normalized
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let compact = trim_outer_punctuation(&compact);
    let english_phrase = trim_outer_punctuation(&normalized);

    if matches_exact_runtime_fact_phrase(
        compact,
        &[
            "刚才回答今天星期几用了什么模型",
            "刚才回答今天星期几时用了什么模型",
            "上一轮用了什么模型",
            "上次回答用了什么模型",
            "刚刚用了什么模型",
        ],
    ) || matches_exact_runtime_fact_phrase(
        english_phrase,
        &[
            "what model did you use last turn",
            "what model did you use for the last answer",
            "which model answered the previous turn",
        ],
    ) {
        return Some(MainChatProviderRouteIntent::AskPreviousTurnModelRoute);
    }

    if matches_exact_runtime_fact_phrase(
        compact,
        &[
            "你现在用什么模型",
            "你当前用什么模型",
            "当前用什么模型",
            "现在用什么模型",
            "你现在用哪个模型",
            "你当前用哪个模型",
            "你现在走什么模型路线",
            "当前模型路线是什么",
        ],
    ) || matches_exact_runtime_fact_phrase(
        english_phrase,
        &[
            "what model are you using",
            "what model are you using now",
            "which model are you using",
            "which provider are you using",
            "what provider are you using",
            "what is your current model route",
        ],
    ) {
        return Some(MainChatProviderRouteIntent::AskCurrentModelRoute);
    }

    None
}

fn provider_route_fact_bindings(
    configured: &ProviderRouteFactSnapshot,
    current: &ProviderRouteFactSnapshot,
    last_completed: &Option<ProviderRouteFactSnapshot>,
    planned: &ProviderRouteFactSnapshot,
    current_model_generated: bool,
    preflight: &ProviderPreflightFactSnapshot,
) -> Vec<MainChatRuntimeFactBinding> {
    let mut facts = Vec::new();
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_PROVIDER,
        "bounded_label_or_none",
        current.provider.as_deref(),
        vec!["provider_route", "agent_run"],
        "run_trace",
        "run_trace",
        "ui_badge",
        current.provider.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL,
        "bounded_label_or_none",
        current.model.as_deref(),
        vec!["provider_route", "agent_run"],
        "run_trace",
        "run_trace",
        "ui_badge",
        current.model.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_ROUTE_TYPE,
        "local_cloud_direct_or_none",
        current.route_type.as_deref().or(Some("none")),
        vec!["provider_route"],
        "run_trace",
        "run_trace",
        "ui_badge",
        false,
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED,
        "boolean",
        Some(if current_model_generated {
            "true"
        } else {
            "false"
        }),
        vec!["generation_metadata"],
        "run_trace",
        "run_trace",
        "trace_only",
        false,
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_PROVIDER,
        "bounded_label",
        last_completed
            .as_ref()
            .and_then(|route| route.provider.as_deref()),
        vec!["agent_run"],
        "run_trace",
        "store_snapshot",
        "trace_only",
        last_completed.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_MODEL,
        "bounded_label",
        last_completed
            .as_ref()
            .and_then(|route| route.model.as_deref()),
        vec!["agent_run"],
        "run_trace",
        "store_snapshot",
        "trace_only",
        last_completed.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_RUN_ID,
        "bounded_id",
        last_completed
            .as_ref()
            .and_then(|route| route.run_id.as_deref()),
        vec!["agent_run"],
        "run_trace",
        "store_snapshot",
        "trace_only",
        last_completed.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER,
        "bounded_label",
        configured.provider.as_deref(),
        vec!["config"],
        "config",
        "turn_snapshot",
        "trace_only",
        configured.provider.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_MODEL,
        "bounded_label",
        configured.model.as_deref(),
        vec!["config"],
        "config",
        "turn_snapshot",
        "trace_only",
        configured.model.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER,
        "bounded_label",
        planned.provider.as_deref(),
        vec!["provider_route", "model_router"],
        "config",
        "turn_snapshot",
        "trace_only",
        planned.provider.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_PLANNED_MODEL,
        "bounded_label",
        planned.model.as_deref(),
        vec!["provider_route", "model_router"],
        "config",
        "turn_snapshot",
        "trace_only",
        planned.model.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_PLANNED_ROUTE_TYPE,
        "local_cloud_or_unknown",
        planned.route_type.as_deref().or(Some("unknown")),
        vec!["provider_route", "model_router"],
        "config",
        "turn_snapshot",
        "trace_only",
        false,
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_PREFLIGHT_STATUS,
        "ready_or_blocker_labels",
        Some(preflight.status.as_str()),
        vec!["provider_preflight"],
        "policy",
        "turn_snapshot",
        "trace_only",
        false,
    ));
    facts
}

fn provider_fact_binding(
    key: &'static str,
    value_shape: &'static str,
    value: Option<&str>,
    source: Vec<&'static str>,
    authority: &'static str,
    freshness: &'static str,
    visibility: &'static str,
    missing: bool,
) -> MainChatRuntimeFactBinding {
    MainChatRuntimeFactBinding {
        key,
        value_shape,
        value: value.map(str::to_string),
        source,
        authority,
        freshness,
        visibility,
        privacy: "internal",
        missing,
    }
}

#[derive(Debug, Clone)]
struct ProviderRouteLabels {
    current: String,
    last_completed: String,
    configured: String,
    planned: String,
}

fn provider_route_labels(
    configured: &ProviderRouteFactSnapshot,
    current: &ProviderRouteFactSnapshot,
    last_completed: &Option<ProviderRouteFactSnapshot>,
    planned: &ProviderRouteFactSnapshot,
    current_model_generated: bool,
    preflight: &ProviderPreflightFactSnapshot,
) -> ProviderRouteLabels {
    let current_label = if current_model_generated {
        format!(
            "current_turn_generation: actual {} / {} ({})",
            label_or_unknown(current.provider.as_deref()),
            label_or_unknown(current.model.as_deref()),
            label_or_unknown(current.route_type.as_deref())
        )
    } else {
        "current_turn_generation: no model generated in this turn".into()
    };
    let last_label = last_completed
        .as_ref()
        .map(|route| {
            format!(
                "last_completed_generation: {} / {} ({}) run {}",
                label_or_unknown(route.provider.as_deref()),
                label_or_unknown(route.model.as_deref()),
                label_or_unknown(route.route_type.as_deref()),
                label_or_unknown(route.run_id.as_deref())
            )
        })
        .unwrap_or_else(|| "last_completed_generation: unknown".into());
    let configured_label = format!(
        "configured_default_route: {} / {}",
        label_or_unknown(configured.provider.as_deref()),
        label_or_unknown(configured.model.as_deref())
    );
    let planned_label = format!(
        "planned_route_if_model_needed: {} / {} ({}) preflight={}",
        label_or_unknown(planned.provider.as_deref()),
        label_or_unknown(planned.model.as_deref()),
        label_or_unknown(planned.route_type.as_deref()),
        preflight.status
    );

    ProviderRouteLabels {
        current: current_label,
        last_completed: last_label,
        configured: configured_label,
        planned: planned_label,
    }
}

fn provider_route_reply(
    intent: MainChatProviderRouteIntent,
    configured: &ProviderRouteFactSnapshot,
    current: &ProviderRouteFactSnapshot,
    last_completed: &Option<ProviderRouteFactSnapshot>,
    planned: &ProviderRouteFactSnapshot,
    last_turn: &Option<ProviderRouteFactSnapshot>,
    current_model_generated: bool,
    preflight: &ProviderPreflightFactSnapshot,
) -> String {
    let configured_label = format!(
        "{} / {}",
        label_or_unknown(configured.provider.as_deref()),
        label_or_unknown(configured.model.as_deref())
    );
    let planned_label = format!(
        "{} / {} ({})",
        label_or_unknown(planned.provider.as_deref()),
        label_or_unknown(planned.model.as_deref()),
        label_or_unknown(planned.route_type.as_deref())
    );
    let last_completed_label = last_completed
        .as_ref()
        .map(|route| {
            format!(
                "{} / {} ({})，run {}",
                label_or_unknown(route.provider.as_deref()),
                label_or_unknown(route.model.as_deref()),
                label_or_unknown(route.route_type.as_deref()),
                label_or_unknown(route.run_id.as_deref())
            )
        })
        .unwrap_or_else(|| "未知：本会话还没有已完成的模型生成记录".into());
    let preflight_label = if preflight.blockers.is_empty() {
        "provider.preflight.status=ready（这只是路由前置状态，不是实际调用证明）".into()
    } else {
        format!(
            "provider.preflight.status=blocked（{}）；这不是 readiness，也不会当作实际模型调用证明",
            preflight.blockers.join(", ")
        )
    };
    let last_turn_label = last_turn
        .as_ref()
        .map(|route| {
            if route_snapshot_is_model_generation(route) {
                format!(
                    "上一轮记录为模型生成：{} / {} ({})。",
                    label_or_unknown(route.provider.as_deref()),
                    label_or_unknown(route.model.as_deref()),
                    label_or_unknown(route.route_type.as_deref())
                )
            } else {
                "上一轮是确定性 runtime fact/direct 路径，没有调用模型。".into()
            }
        })
        .unwrap_or_else(|| "上一轮没有可用运行记录。".into());

    match intent {
        MainChatProviderRouteIntent::AskCurrentModelRoute if current_model_generated => format!(
            "current_turn_generation：本轮实际调用的是 {} / {}（{}）。configured_default_route：{}。planned_route_if_model_needed：{}。last_completed_generation：{}。{}",
            label_or_unknown(current.provider.as_deref()),
            label_or_unknown(current.model.as_deref()),
            label_or_unknown(current.route_type.as_deref()),
            configured_label,
            planned_label,
            last_completed_label,
            preflight_label
        ),
        MainChatProviderRouteIntent::AskCurrentModelRoute => format!(
            "current_turn_generation：本轮没有调用模型，因此没有 current-turn provider/model。configured_default_route：{}。planned_route_if_model_needed：{}。last_completed_generation：{}。{}",
            configured_label, planned_label, last_completed_label, preflight_label
        ),
        MainChatProviderRouteIntent::AskPreviousTurnModelRoute => format!(
            "{} current_turn_generation：本轮为了回答这个 runtime fact 问题没有调用模型，因此没有 current-turn provider/model。configured_default_route：{}。planned_route_if_model_needed：{}。last_completed_generation：{}。{}",
            last_turn_label, configured_label, planned_label, last_completed_label, preflight_label
        ),
    }
}

async fn last_turn_snapshot(
    state: &Arc<AppState>,
    session_id: &str,
) -> Option<ProviderRouteFactSnapshot> {
    let store_arc = state.agent_run_store.as_ref()?;
    let store = store_arc.lock().await;
    let runs = store.list_runs_for_session(session_id, 5).ok()?;
    runs.into_iter()
        .find(|run| run.status == AgentRunStatus::Completed)
        .and_then(|run| {
            run.model_route
                .as_ref()
                .map(|route| route_snapshot_from_trace(route, Some(run.id.as_str())))
        })
}

async fn last_completed_generation_snapshot(
    state: &Arc<AppState>,
    session_id: &str,
) -> Option<ProviderRouteFactSnapshot> {
    let store_arc = state.agent_run_store.as_ref()?;
    let store = store_arc.lock().await;
    let runs = store.list_runs_for_session(session_id, 20).ok()?;
    runs.into_iter()
        .filter(|run| run.status == AgentRunStatus::Completed)
        .find_map(|run| {
            let route = run.model_route.as_ref()?;
            let snapshot = route_snapshot_from_trace(route, Some(run.id.as_str()));
            route_snapshot_is_model_generation(&snapshot).then_some(snapshot)
        })
}

fn route_snapshot_from_trace(
    route: &ModelRouteTrace,
    run_id: Option<&str>,
) -> ProviderRouteFactSnapshot {
    ProviderRouteFactSnapshot {
        provider: Some(bounded_runtime_fact_label(&route.provider)),
        model: Some(bounded_runtime_fact_label(&route.model)),
        route_type: Some(bounded_runtime_fact_label(&route.route_type)),
        run_id: run_id.map(bounded_runtime_fact_label),
    }
}

fn no_current_generation_snapshot() -> ProviderRouteFactSnapshot {
    ProviderRouteFactSnapshot {
        provider: None,
        model: None,
        route_type: Some("none".into()),
        run_id: None,
    }
}

fn route_snapshot_is_model_generation(route: &ProviderRouteFactSnapshot) -> bool {
    let provider = route.provider.as_deref().unwrap_or_default();
    let model = route.model.as_deref().unwrap_or_default();
    let route_type = route.route_type.as_deref().unwrap_or_default();
    !provider.is_empty()
        && provider != "direct"
        && !matches!(model, "runtime_fact" | "L1_reflex")
        && !matches!(route_type, "direct" | "none")
}

fn provider_preflight_snapshot(
    config: &AppConfig,
    scheduler: &InferenceScheduler,
    planned: &ModelRouteTrace,
) -> ProviderPreflightFactSnapshot {
    let mut blockers = Vec::new();
    let planned_route_type = planned.route_type.trim().to_ascii_lowercase();
    let planned_provider = planned.provider.trim().to_ascii_lowercase();
    let configured_provider = config.llm.provider.trim().to_ascii_lowercase();
    if configured_provider.is_empty() || configured_provider == "none" {
        blockers.push("configured_provider_missing".into());
    }
    let configured_cloud_provider = is_cloud_route_provider(&configured_provider);
    let planned_cloud_route = planned_route_type == "cloud";
    let cloud_route_needs_preflight = configured_cloud_provider || planned_cloud_route;
    if cloud_route_needs_preflight && !config.system.network_policy.enabled {
        blockers.push("network_disabled".into());
    }
    if ((configured_cloud_provider && config.effective_cloud_api_key().trim().is_empty())
        || (planned_cloud_route && scheduler.effective_api_key().trim().is_empty()))
        && scheduler.scripted_generation_response.is_none()
    {
        blockers.push("provider_api_key_missing".into());
    }
    if planned_provider.is_empty() || planned_provider == "none" || planned_route_type == "fallback"
    {
        blockers.push("provider_route_unavailable".into());
    }
    blockers.sort();
    blockers.dedup();
    ProviderPreflightFactSnapshot {
        status: if blockers.is_empty() {
            "ready".into()
        } else {
            "blocked".into()
        },
        blockers,
    }
}

fn is_cloud_route_provider(provider: &str) -> bool {
    let provider = provider.trim().to_ascii_lowercase();
    !matches!(
        provider.as_str(),
        "" | "none" | "ollama" | "local" | "direct" | "runtime_fact"
    )
}

fn planned_route_without_probe(scheduler: &InferenceScheduler) -> ModelRouteTrace {
    if let Some(router) = scheduler.model_router.as_ref() {
        if let Ok(decision) = router.route_chat(None, scheduler.prefer_local) {
            return decision.to_trace();
        }
    }

    let has_remote_key = !scheduler.effective_api_key().trim().is_empty();
    let use_configured_local = scheduler.prefer_local && !has_remote_key;
    let (provider, model, route_type, reason) = if use_configured_local {
        (
            "ollama".to_string(),
            scheduler.local_model.clone(),
            "local".to_string(),
            "configured_local_preference_without_probe".to_string(),
        )
    } else if scheduler.provider.trim().is_empty() || scheduler.provider == "none" {
        (
            "none".to_string(),
            scheduler.chat_model.clone(),
            "unknown".to_string(),
            "configured_provider_missing_without_probe".to_string(),
        )
    } else {
        (
            scheduler.provider.clone(),
            scheduler.chat_model.clone(),
            "cloud".to_string(),
            "configured_cloud_route_without_probe".to_string(),
        )
    };

    ModelRouteTrace {
        provider,
        model,
        route_type,
        prefer_local: scheduler.prefer_local,
        local_model: scheduler.local_model.clone(),
        reason,
        privacy_level: openlife_core::agent::RedactionLevel::None,
        latency_ms: None,
        retry_count: 0,
        fallback_reason: None,
        provider_health_is_estimated: Some(true),
    }
}
