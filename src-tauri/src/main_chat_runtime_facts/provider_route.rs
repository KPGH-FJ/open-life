use openlife_core::agent::{AgentRun, AgentRunStatus, ModelRouteTrace};
use openlife_core::config::AppConfig;
use openlife_core::scheduler::InferenceScheduler;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeRouteEvidence {
    pub evidence_id: String,
    pub generated_at: String,
    pub conversation_id: Option<String>,
    pub run_id: Option<String>,
    pub task_session_id: Option<String>,
    pub answer_scope: String,
    pub planned_route: Option<RouteIdentity>,
    pub actual_route: Option<RouteIdentity>,
    pub last_completed_route: Option<RouteIdentity>,
    pub provider_readiness: ProviderReadiness,
    pub fallback: Option<FallbackEvidence>,
    pub external_transmission: String,
    pub source_refs: Vec<Value>,
    pub truth_confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderTransmissionSourceRef {
    pub source: String,
    pub ref_id: Option<String>,
    pub status: Option<String>,
    pub route_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderTransmissionHistoryItem {
    pub status: String,
    pub run_id: String,
    pub task_session_id: Option<String>,
    pub provider: String,
    pub model: String,
    pub route_type: String,
    pub reason: String,
    pub evidence_id: String,
    pub truth_confidence: String,
    pub data_category: String,
    pub source_refs: Vec<ProviderTransmissionSourceRef>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteIdentity {
    pub provider: String,
    pub model: String,
    pub route_type: String,
    pub privacy_level: String,
    pub reason: String,
    pub provider_health_is_estimated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderReadiness {
    pub configured: bool,
    pub credential_present: bool,
    pub validated: bool,
    pub validation_status: String,
    pub preferred: String,
    pub actually_used: Option<String>,
    pub stale: bool,
    pub failed: bool,
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FallbackEvidence {
    pub from_route: Option<RouteIdentity>,
    pub to_route: Option<RouteIdentity>,
    pub reason: String,
    pub blocker_codes: Vec<String>,
}

#[derive(Debug, Clone)]
struct ProviderRouteFactSnapshot {
    provider: Option<String>,
    model: Option<String>,
    route_type: Option<String>,
    run_id: Option<String>,
    reason: Option<String>,
    privacy_level: Option<String>,
    provider_health_is_estimated: Option<bool>,
    fallback_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct ProviderPreflightFactSnapshot {
    status: String,
    blockers: Vec<String>,
}

pub(crate) fn provider_route_fact_should_block_before_model(
    config: &AppConfig,
    scheduler: &InferenceScheduler,
) -> bool {
    let planned = planned_route_without_probe(scheduler);
    let preflight = provider_preflight_snapshot(config, scheduler, &planned);
    preflight.status == "blocked"
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
pub(crate) async fn resolve_provider_route_fact_answer(
    user_text: &str,
    state: &Arc<AppState>,
    config: &AppConfig,
    scheduler: &InferenceScheduler,
    session_id: &str,
    current_route: Option<ModelRouteTrace>,
    current_model_generated: bool,
    scheduler_generation_called: bool,
    provider_generation_path: &str,
) -> Option<MainChatRuntimeFactAnswer> {
    let intent = classify_provider_route_query(user_text)?;
    let planned_route = planned_route_without_probe(scheduler);
    let preflight = provider_preflight_snapshot(config, scheduler, &planned_route);
    let configured = ProviderRouteFactSnapshot {
        provider: Some(bounded_runtime_fact_label(&config.llm.provider)),
        model: Some(bounded_runtime_fact_label(&config.llm.chat_model)),
        route_type: None,
        run_id: None,
        reason: Some("configured_default_route".into()),
        privacy_level: Some("unknown".into()),
        provider_health_is_estimated: Some(true),
        fallback_reason: None,
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
    let runtime_route_evidence = build_runtime_route_evidence_from_snapshots(
        state,
        config,
        scheduler,
        Some(session_id),
        None,
        None,
        "current_turn",
        Some(&planned),
        Some(&current),
        last_completed_generation.as_ref(),
        current_model_generated,
    )
    .await;
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
        "runtimeRouteEvidence": runtime_route_evidence,
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

    if is_bounded_route_truth_mixed_prompt(&normalized, compact) {
        return Some(MainChatProviderRouteIntent::AskCurrentModelRoute);
    }

    None
}

pub(crate) fn provider_route_query_has_followup_task(user_text: &str) -> bool {
    let normalized = user_text.trim().to_lowercase();
    if normalized.is_empty() || classify_provider_route_query(&normalized).is_none() {
        return false;
    }
    let task_markers = [
        "然后",
        "顺便",
        "同时",
        "并且",
        "再回答",
        "再帮",
        "回答一下",
        "帮我",
        "写",
        "总结",
        "计划",
        "explain",
        "then",
        "also",
        "and answer",
        "please answer",
        "summarize",
        "write",
        "plan",
    ];
    task_markers
        .iter()
        .any(|marker| normalized.contains(marker))
}

pub(crate) async fn build_settings_runtime_route_evidence(
    state: &Arc<AppState>,
    config: &AppConfig,
    scheduler: &InferenceScheduler,
) -> RuntimeRouteEvidence {
    let planned = route_snapshot_from_trace(&planned_route_without_probe(scheduler), None);
    let last_completed = last_completed_generation_snapshot_any_session(state).await;
    build_runtime_route_evidence_from_snapshots(
        state,
        config,
        scheduler,
        None,
        None,
        None,
        "settings_readiness",
        Some(&planned),
        None,
        last_completed.as_ref(),
        false,
    )
    .await
}

pub(crate) fn provider_transmission_history_from_runs(
    runs: &[AgentRun],
) -> Vec<ProviderTransmissionHistoryItem> {
    runs.iter()
        .map(provider_transmission_history_item_from_run)
        .collect()
}

pub(crate) async fn provider_transmission_history_from_runs_with_state(
    state: &Arc<AppState>,
    runs: &[AgentRun],
) -> Result<Vec<ProviderTransmissionHistoryItem>, String> {
    let mut items = provider_transmission_history_from_runs(runs);
    let Some(event_store_arc) = state.main_chat_agent_event_store.as_ref() else {
        for item in &mut items {
            item.status = "unknown".into();
            item.truth_confidence = "unknown".into();
            item.reason = "durable_provider_lifecycle_store_unavailable".into();
        }
        return Ok(items);
    };
    let event_store = event_store_arc.lock().await;
    for (run, item) in runs.iter().zip(items.iter_mut()) {
        let event = event_store
            .latest_provider_event_for_run(&run.id)
            .map_err(|error| format!("provider lifecycle read failed for {}: {error}", run.id))?;
        let Some(event) = event else {
            if item.route_type == "cloud" {
                item.status = "unknown".into();
                item.truth_confidence = "unknown".into();
                item.reason = "durable_exact_request_lifecycle_missing".into();
            }
            continue;
        };
        apply_durable_provider_event_to_history_item(item, &event);
    }
    Ok(items)
}

fn apply_durable_provider_event_to_history_item(
    item: &mut ProviderTransmissionHistoryItem,
    event: &crate::main_chat_event_stream::MainChatAgentDurableEvent,
) {
    let provider = event
        .payload
        .get("provider")
        .and_then(Value::as_str)
        .map(metadata_safe_transmission_label)
        .unwrap_or_else(|| "unknown".into());
    let model = event
        .payload
        .get("model")
        .and_then(Value::as_str)
        .map(metadata_safe_transmission_label)
        .unwrap_or_else(|| "unknown".into());
    let route_type = if provider.eq_ignore_ascii_case("ollama") {
        "local"
    } else if provider == "unknown" {
        "unknown"
    } else {
        "cloud"
    };
    item.provider = provider;
    item.model = model;
    item.route_type = route_type.into();
    item.status = match (event.event_type.as_str(), route_type) {
        ("provider.completed", "cloud") => "sent",
        ("provider.completed" | "provider.failed" | "provider.remote_unknown", "local") => {
            "not_sent"
        }
        ("provider.started" | "provider.failed" | "provider.remote_unknown", "cloud") => "unknown",
        _ => "unknown",
    }
    .into();
    item.reason = format!(
        "durable_exact_request_{}",
        event.event_type.replace('.', "_")
    );
    item.evidence_id = metadata_safe_transmission_label(&event.event_id);
    item.truth_confidence = "verified".into();
    item.source_refs.push(ProviderTransmissionSourceRef {
        source: "turn_event_store".into(),
        ref_id: Some(metadata_safe_transmission_label(&event.event_id)),
        status: Some(metadata_safe_transmission_label(&event.event_type)),
        route_type: Some(route_type.into()),
    });
}

pub(crate) fn provider_transmission_history_item_from_run(
    run: &AgentRun,
) -> ProviderTransmissionHistoryItem {
    let generation_metadata = run_generation_metadata(run);
    let runtime_evidence = runtime_route_evidence_from_agent_run(run);
    let route = runtime_evidence
        .as_ref()
        .and_then(transmission_route_from_evidence)
        .or_else(|| {
            run.model_route
                .as_ref()
                .map(route_identity_from_model_route)
        });
    let status_route = runtime_evidence
        .as_ref()
        .and_then(transmission_invocation_route_from_evidence);
    let route_type = route
        .as_ref()
        .map(|route| normalized_evidence_route_type(Some(route.route_type.as_str())))
        .unwrap_or_else(|| "unknown".into());
    let status_route_type = status_route
        .as_ref()
        .map(|route| normalized_evidence_route_type(Some(route.route_type.as_str())))
        .unwrap_or_else(|| "unknown".into());
    let status = provider_transmission_status(
        run,
        runtime_evidence.as_ref(),
        generation_metadata,
        status_route.as_ref(),
        &status_route_type,
    );
    let reason = transmission_reason(
        runtime_evidence.as_ref(),
        generation_metadata,
        route.as_ref(),
    );
    let task_session_id = runtime_evidence
        .as_ref()
        .and_then(|evidence| evidence.task_session_id.as_deref())
        .or_else(|| metadata_string(generation_metadata, &["taskSessionId", "task_session_id"]))
        .or_else(|| {
            metadata_string(
                generation_metadata,
                &["agentTaskSessionId", "agent_task_session_id"],
            )
        })
        .map(metadata_safe_transmission_label);
    let evidence_id = runtime_evidence
        .as_ref()
        .map(|evidence| metadata_safe_transmission_label(&evidence.evidence_id))
        .or_else(|| {
            run.model_route.as_ref().map(|_| {
                format!(
                    "agent_run:{}:model_route",
                    metadata_safe_transmission_label(&run.id)
                )
            })
        })
        .unwrap_or_else(|| {
            format!(
                "agent_run:{}:route_not_instrumented",
                metadata_safe_transmission_label(&run.id)
            )
        });
    let truth_confidence =
        transmission_truth_confidence(&status, runtime_evidence.as_ref(), run.model_route.as_ref());
    let mut source_refs =
        transmission_source_refs(run, runtime_evidence.as_ref(), generation_metadata);
    if source_refs.is_empty() {
        source_refs.push(ProviderTransmissionSourceRef {
            source: "agent_run".into(),
            ref_id: Some(metadata_safe_transmission_label(&run.id)),
            status: Some("route_not_instrumented".into()),
            route_type: None,
        });
    }

    ProviderTransmissionHistoryItem {
        status,
        run_id: metadata_safe_transmission_label(&run.id),
        task_session_id,
        provider: route
            .as_ref()
            .map(|route| metadata_safe_transmission_label(&route.provider))
            .unwrap_or_else(|| "unknown".into()),
        model: route
            .as_ref()
            .map(|route| metadata_safe_transmission_label(&route.model))
            .unwrap_or_else(|| "unknown".into()),
        route_type,
        reason: metadata_safe_transmission_label(&reason),
        evidence_id,
        truth_confidence,
        data_category: "provider_transmission".into(),
        source_refs,
        started_at: run.started_at.to_rfc3339(),
        finished_at: run
            .finished_at
            .as_ref()
            .map(|finished_at| finished_at.to_rfc3339()),
    }
}

fn run_generation_metadata(run: &AgentRun) -> Option<&Value> {
    run.reasoning_trace
        .as_ref()
        .and_then(|trace| trace.generation_result.as_ref())
}

fn runtime_route_evidence_from_agent_run(run: &AgentRun) -> Option<RuntimeRouteEvidence> {
    run_generation_metadata(run)
        .and_then(|value| value.get("runtimeRouteEvidence").cloned())
        .and_then(|value| serde_json::from_value(value).ok())
}

fn transmission_route_from_evidence(evidence: &RuntimeRouteEvidence) -> Option<RouteIdentity> {
    evidence
        .actual_route
        .clone()
        .or_else(|| evidence.last_completed_route.clone())
        .or_else(|| evidence.planned_route.clone())
}

fn transmission_invocation_route_from_evidence(
    evidence: &RuntimeRouteEvidence,
) -> Option<RouteIdentity> {
    evidence
        .actual_route
        .clone()
        .or_else(|| evidence.last_completed_route.clone())
}

fn route_identity_from_model_route(route: &ModelRouteTrace) -> RouteIdentity {
    RouteIdentity {
        provider: metadata_safe_transmission_label(&route.provider),
        model: metadata_safe_transmission_label(&route.model),
        route_type: normalized_evidence_route_type(Some(&route.route_type)),
        privacy_level: metadata_safe_transmission_label(&route.privacy_level.to_string()),
        reason: metadata_safe_transmission_label(&route.reason),
        provider_health_is_estimated: route.provider_health_is_estimated.unwrap_or(true),
    }
}

fn provider_transmission_status(
    run: &AgentRun,
    evidence: Option<&RuntimeRouteEvidence>,
    metadata: Option<&Value>,
    route: Option<&RouteIdentity>,
    route_type: &str,
) -> String {
    let invocation_route = evidence
        .and_then(|evidence| {
            evidence
                .actual_route
                .as_ref()
                .or(evidence.last_completed_route.as_ref())
        })
        .or_else(|| if evidence.is_none() { route } else { None });
    if provider_preflight_blocked(evidence, metadata)
        && no_model_invocation_for_blocked_status(metadata, invocation_route)
    {
        return "blocked".into();
    }
    if positive_not_sent_route_type(route_type)
        || positive_evidence_transmission(
            evidence,
            "not_sent",
            &["local", "agent_runtime", "scripted"],
        )
    {
        return "not_sent".into();
    }
    if let Some(evidence) = evidence {
        return match evidence.external_transmission.as_str() {
            "not_instrumented" => "not_instrumented".into(),
            "unknown" => "unknown".into(),
            _ => "unknown".into(),
        };
    }
    if run.model_route.is_none() {
        "not_instrumented".into()
    } else {
        "unknown".into()
    }
}

fn positive_not_sent_route_type(route_type: &str) -> bool {
    matches!(route_type, "local" | "agent_runtime" | "scripted")
}

fn positive_evidence_transmission(
    evidence: Option<&RuntimeRouteEvidence>,
    expected_status: &str,
    allowed_route_types: &[&str],
) -> bool {
    let Some(evidence) = evidence else {
        return false;
    };
    if evidence.external_transmission != expected_status {
        return false;
    }
    let Some(route) = evidence
        .actual_route
        .as_ref()
        .or(evidence.last_completed_route.as_ref())
    else {
        return false;
    };
    let route_type = normalized_evidence_route_type(Some(route.route_type.as_str()));
    allowed_route_types.contains(&route_type.as_str())
}

fn provider_preflight_blocked(
    evidence: Option<&RuntimeRouteEvidence>,
    metadata: Option<&Value>,
) -> bool {
    let evidence_blocked = evidence.is_some_and(|evidence| {
        evidence.source_refs.iter().any(|source| {
            source.get("source").and_then(Value::as_str) == Some("provider_preflight")
                && source.get("status").and_then(Value::as_str) == Some("blocked")
        }) || evidence
            .fallback
            .as_ref()
            .is_some_and(|fallback| !fallback.blocker_codes.is_empty())
    });
    evidence_blocked
        || metadata_string(
            metadata,
            &[
                "providerPreflightStatus",
                "provider_preflight_status",
                "liveProviderPreflightStatus",
                "live_provider_preflight_status",
            ],
        ) == Some("blocked")
        || metadata_non_empty_array(
            metadata,
            &[
                "providerPreflightBlockers",
                "provider_preflight_blockers",
                "liveProviderPreflightBlockers",
                "live_provider_preflight_blockers",
            ],
        )
}

fn no_model_invocation_for_blocked_status(
    metadata: Option<&Value>,
    route: Option<&RouteIdentity>,
) -> bool {
    if metadata_bool(
        metadata,
        &[
            "liveProviderInvoked",
            "live_provider_invoked",
            "modelInvoked",
            "model_invoked",
            "modelGenerated",
            "model_generated",
            "schedulerGenerationCalled",
            "scheduler_generation_called",
        ],
    ) {
        return false;
    }
    !route.is_some_and(|route| {
        matches!(
            normalized_evidence_route_type(Some(route.route_type.as_str())).as_str(),
            "cloud" | "local" | "scripted"
        )
    })
}

fn transmission_reason(
    evidence: Option<&RuntimeRouteEvidence>,
    metadata: Option<&Value>,
    route: Option<&RouteIdentity>,
) -> String {
    evidence
        .and_then(|evidence| {
            evidence
                .fallback
                .as_ref()
                .map(|fallback| fallback.reason.clone())
        })
        .or_else(|| {
            metadata_string(
                metadata,
                &[
                    "blockerReason",
                    "blocker_reason",
                    "routeReason",
                    "route_reason",
                    "providerPreflightStatus",
                    "provider_preflight_status",
                ],
            )
            .map(str::to_string)
        })
        .or_else(|| route.map(|route| route.reason.clone()))
        .unwrap_or_else(|| "route_metadata_missing".into())
}

fn transmission_truth_confidence(
    status: &str,
    evidence: Option<&RuntimeRouteEvidence>,
    model_route: Option<&ModelRouteTrace>,
) -> String {
    if let Some(evidence) = evidence {
        return metadata_safe_transmission_label(&evidence.truth_confidence);
    }
    match status {
        "sent" | "not_sent" | "blocked" if model_route.is_some() => "verified".into(),
        "not_instrumented" => "unknown".into(),
        _ => "unknown".into(),
    }
}

fn transmission_source_refs(
    run: &AgentRun,
    evidence: Option<&RuntimeRouteEvidence>,
    metadata: Option<&Value>,
) -> Vec<ProviderTransmissionSourceRef> {
    let mut refs = vec![ProviderTransmissionSourceRef {
        source: "agent_run".into(),
        ref_id: Some(metadata_safe_transmission_label(&run.id)),
        status: Some(metadata_safe_transmission_label(&run.status.to_string())),
        route_type: run
            .model_route
            .as_ref()
            .map(|route| normalized_evidence_route_type(Some(&route.route_type))),
    }];
    if run.model_route.is_some() {
        refs.push(ProviderTransmissionSourceRef {
            source: "agent_run_model_route".into(),
            ref_id: Some(format!(
                "agent_run:{}:model_route",
                metadata_safe_transmission_label(&run.id)
            )),
            status: Some("present".into()),
            route_type: run
                .model_route
                .as_ref()
                .map(|route| normalized_evidence_route_type(Some(&route.route_type))),
        });
    }
    if let Some(evidence) = evidence {
        refs.push(ProviderTransmissionSourceRef {
            source: "runtime_route_evidence".into(),
            ref_id: Some(metadata_safe_transmission_label(&evidence.evidence_id)),
            status: Some(metadata_safe_transmission_label(
                &evidence.external_transmission,
            )),
            route_type: evidence
                .actual_route
                .as_ref()
                .or(evidence.last_completed_route.as_ref())
                .or(evidence.planned_route.as_ref())
                .map(|route| normalized_evidence_route_type(Some(route.route_type.as_str()))),
        });
        refs.extend(evidence.source_refs.iter().take(8).map(|source| {
            ProviderTransmissionSourceRef {
                source: source_ref_string(source, &["source"])
                    .unwrap_or_else(|| "runtime_route_evidence_source".into()),
                ref_id: source_ref_string(
                    source,
                    &["runId", "run_id", "evidenceId", "evidence_id"],
                ),
                status: source_ref_string(source, &["status"]).or_else(|| {
                    source
                        .get("blockers")
                        .and_then(Value::as_array)
                        .filter(|blockers| !blockers.is_empty())
                        .map(|_| "blocked".into())
                }),
                route_type: source_ref_string(source, &["routeType", "route_type"])
                    .map(|route_type| normalized_evidence_route_type(Some(route_type.as_str()))),
            }
        }));
    }
    if provider_preflight_blocked(evidence, metadata) {
        refs.push(ProviderTransmissionSourceRef {
            source: "provider_preflight".into(),
            ref_id: None,
            status: Some("blocked".into()),
            route_type: None,
        });
    }
    refs
}

fn source_ref_string(source: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| source.get(*key).and_then(Value::as_str))
        .map(metadata_safe_transmission_label)
}

fn metadata_string<'a>(metadata: Option<&'a Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        metadata
            .and_then(|value| value.get(*key))
            .and_then(Value::as_str)
    })
}

fn metadata_bool(metadata: Option<&Value>, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        metadata
            .and_then(|value| value.get(*key))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    })
}

fn metadata_non_empty_array(metadata: Option<&Value>, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        metadata
            .and_then(|value| value.get(*key))
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    })
}

fn metadata_safe_transmission_label(value: &str) -> String {
    let label = bounded_runtime_fact_label(value);
    if contains_secret_like_material(&label) {
        "redacted_sensitive".into()
    } else {
        label
    }
}

fn contains_secret_like_material(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    contains_key_prefix(&lower)
        || lower.contains("bearer ")
        || lower.contains("authorization:")
        || lower.contains("api_key=")
        || lower.contains("api key:")
        || lower.contains("apikey=")
        || lower.contains("token=")
        || lower.contains("secret=")
        || lower.contains("password=")
}

fn contains_key_prefix(value: &str) -> bool {
    value.match_indices("sk-").any(|(index, _)| {
        index == 0
            || value[..index]
                .chars()
                .next_back()
                .is_some_and(|ch| !ch.is_ascii_alphanumeric())
    })
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

fn is_bounded_route_truth_mixed_prompt(normalized: &str, compact: &str) -> bool {
    let mentions_route_truth = [
        "provider",
        "model",
        "route",
        "routetype",
        "fallback",
        "actually used",
        "current model",
        "deepseek",
        "ollama",
        "openai",
        "openrouter",
        "cloud",
        "local",
        "local-first",
        "模型",
        "路线",
        "路由",
        "云端",
        "本地",
        "回退",
        "外发",
        "调用云端",
        "当前实际使用",
        "当前实际",
        "实际使用",
        "你现在用",
        "当前用",
    ]
    .iter()
    .any(|marker| normalized.contains(marker) || compact.contains(marker));
    if !mentions_route_truth {
        return false;
    }

    let asks_for_truth = [
        "当前",
        "现在",
        "实际",
        "用了",
        "使用",
        "调用",
        "是否",
        "有没有",
        "为什么",
        "说明",
        "fallbackreason",
        "routetype",
        "actually",
        "current",
        "did you",
        "are you using",
        "why",
        "explain",
    ]
    .iter()
    .any(|marker| normalized.contains(marker) || compact.contains(marker));
    if asks_for_truth {
        return true;
    }

    let explicit_provider_request = [
        "用deepseek",
        "用openai",
        "用openrouter",
        "use deepseek",
        "use openai",
        "use openrouter",
        "use cloud",
        "走云端",
        "使用云端",
        "调用云端",
    ]
    .iter()
    .any(|marker| normalized.contains(marker) || compact.contains(marker));
    explicit_provider_request
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
async fn build_runtime_route_evidence_from_snapshots(
    _state: &Arc<AppState>,
    config: &AppConfig,
    scheduler: &InferenceScheduler,
    conversation_id: Option<&str>,
    run_id: Option<&str>,
    task_session_id: Option<&str>,
    answer_scope: &str,
    planned: Option<&ProviderRouteFactSnapshot>,
    actual: Option<&ProviderRouteFactSnapshot>,
    last_completed: Option<&ProviderRouteFactSnapshot>,
    current_turn_model_generated: bool,
) -> RuntimeRouteEvidence {
    let generated_at = chrono::Utc::now().to_rfc3339();
    let validation_load = crate::provider_validation::load_provider_validation_record_from_path(
        &crate::provider_validation::provider_validation_path(),
    );
    let validation = crate::provider_validation::summarize_loaded_provider_validation(
        config,
        &validation_load,
        chrono::Utc::now(),
    );
    let local_test_provider_ready = false;
    let preflight = planned
        .map(snapshot_to_model_route_trace)
        .map(|route| provider_preflight_snapshot(config, scheduler, &route))
        .unwrap_or_else(|| ProviderPreflightFactSnapshot {
            status: "blocked".into(),
            blockers: vec!["planned_route_missing".into()],
        });
    let planned_route = planned.and_then(route_identity_from_snapshot);
    let actual_route = if current_turn_model_generated {
        actual.and_then(route_identity_from_snapshot)
    } else if answer_scope == "settings_readiness" {
        None
    } else {
        Some(runtime_fact_route_identity())
    };
    let last_completed_route = last_completed.and_then(route_identity_from_snapshot);
    let current_fallback = fallback_evidence(
        planned_route.as_ref(),
        actual_route.as_ref(),
        actual.and_then(|snapshot| snapshot.fallback_reason.as_deref()),
        &preflight,
    );
    // Historical provider truth is a separate observation. It must never be
    // joined to today's planned route or preflight blockers to fabricate a
    // cross-request fallback. A historical fallback is exposed only when the
    // exact completed request matched the AgentRun provider/model metadata;
    // `route_snapshot_from_provider_event` clears fallback_reason otherwise.
    let historical_fallback = (answer_scope == "settings_readiness")
        .then(|| {
            let route = last_completed_route.as_ref()?;
            let reason = last_completed?.fallback_reason.as_deref()?;
            Some(FallbackEvidence {
                from_route: None,
                to_route: Some(route.clone()),
                reason: bounded_runtime_fact_label(reason),
                blocker_codes: Vec::new(),
            })
        })
        .flatten();
    let fallback = current_fallback.or(historical_fallback);
    let provider_readiness = provider_readiness(
        config,
        &validation,
        local_test_provider_ready,
        actual_route.as_ref().or(last_completed_route.as_ref()),
    );
    let external_transmission = external_transmission_status(
        actual_route.as_ref(),
        last_completed_route.as_ref(),
        answer_scope,
    );
    let mut source_refs = vec![
        json!({
            "source": "provider_validation",
            "status": provider_readiness.validation_status,
            "credentialPresent": provider_readiness.credential_present,
        }),
        json!({
            "source": "provider_preflight",
            "status": preflight.status,
            "blockers": preflight.blockers,
        }),
        json!({
            "source": "config",
            "provider": bounded_runtime_fact_label(&config.llm.provider),
            "model": bounded_runtime_fact_label(&config.llm.chat_model),
            "preferLocal": config.prefer_local_model,
        }),
    ];
    if let Some(route) = actual_route.as_ref() {
        source_refs.push(json!({
            "source": "current_turn_route",
            "provider": route.provider,
            "model": route.model,
            "routeType": route.route_type,
        }));
    }
    if let Some(route) = last_completed_route.as_ref() {
        source_refs.push(json!({
            "source": "agent_run",
            "runId": last_completed.and_then(|snapshot| snapshot.run_id.clone()),
            "provider": route.provider,
            "model": route.model,
            "routeType": route.route_type,
        }));
    }

    let truth_confidence = if actual_route.is_some() || last_completed_route.is_some() {
        "verified"
    } else if planned_route.is_some() || validation.configured {
        "inferred"
    } else {
        "unknown"
    }
    .to_string();

    let evidence_id = runtime_route_evidence_id(
        answer_scope,
        conversation_id,
        run_id.or_else(|| last_completed.and_then(|snapshot| snapshot.run_id.as_deref())),
        &generated_at,
    );

    RuntimeRouteEvidence {
        evidence_id,
        generated_at,
        conversation_id: conversation_id.map(bounded_runtime_fact_label),
        run_id: run_id
            .map(bounded_runtime_fact_label)
            .or_else(|| last_completed.and_then(|snapshot| snapshot.run_id.clone())),
        task_session_id: task_session_id.map(bounded_runtime_fact_label),
        answer_scope: answer_scope.to_string(),
        planned_route,
        actual_route,
        last_completed_route,
        provider_readiness,
        fallback,
        external_transmission,
        source_refs,
        truth_confidence,
    }
}

fn runtime_route_evidence_id(
    answer_scope: &str,
    conversation_id: Option<&str>,
    run_id: Option<&str>,
    generated_at: &str,
) -> String {
    format!(
        "runtime_route:{}:{}:{}:{}",
        bounded_runtime_fact_label(answer_scope),
        conversation_id
            .map(bounded_runtime_fact_label)
            .unwrap_or_else(|| "none".into()),
        run_id
            .map(bounded_runtime_fact_label)
            .unwrap_or_else(|| "none".into()),
        bounded_runtime_fact_label(generated_at),
    )
}

fn provider_readiness(
    config: &AppConfig,
    validation: &crate::provider_validation::ProviderValidationSummary,
    local_test_provider_ready: bool,
    actual_route: Option<&RouteIdentity>,
) -> ProviderReadiness {
    let identity = crate::provider_validation::current_provider_validation_identity(config);
    let validation_status = validation.status.to_string();
    ProviderReadiness {
        configured: validation.configured,
        credential_present: identity.key_present,
        validated: validation.status == "validated" && !local_test_provider_ready,
        validation_status: validation_status.clone(),
        preferred: if config.prefer_local_model {
            "local".into()
        } else {
            bounded_runtime_fact_label(&config.llm.provider)
        },
        actually_used: actual_route.map(|route| route.provider.clone()),
        stale: validation_status == "stale",
        failed: validation_status == "failed",
        last_checked_at: validation
            .validated_at
            .clone()
            .or_else(|| validation.failed_at.clone()),
    }
}

fn external_transmission_status(
    actual_route: Option<&RouteIdentity>,
    last_completed_route: Option<&RouteIdentity>,
    answer_scope: &str,
) -> String {
    let evidenced_route = actual_route.or(last_completed_route);
    match evidenced_route.map(|route| route.route_type.as_str()) {
        Some("cloud") => "sent".into(),
        Some("local" | "agent_runtime" | "scripted") => "not_sent".into(),
        Some(_) => "unknown".into(),
        None if answer_scope == "settings_readiness" => "not_instrumented".into(),
        None => "unknown".into(),
    }
}

fn fallback_evidence(
    planned: Option<&RouteIdentity>,
    actual: Option<&RouteIdentity>,
    fallback_reason: Option<&str>,
    preflight: &ProviderPreflightFactSnapshot,
) -> Option<FallbackEvidence> {
    let actual = actual?;
    let planned_cloud = planned.is_some_and(|route| route.route_type == "cloud");
    let actual_local = actual.route_type == "local";
    let planned_cloud_actual_local = planned_cloud && actual_local;
    if !planned_cloud_actual_local && fallback_reason.is_none() {
        return None;
    }
    let reason = fallback_reason
        .map(bounded_runtime_fact_label)
        .unwrap_or_else(|| "planned_cloud_actual_local".into());
    Some(FallbackEvidence {
        from_route: planned.cloned(),
        to_route: Some(actual.clone()),
        reason,
        blocker_codes: if preflight.status == "blocked" {
            preflight.blockers.clone()
        } else {
            Vec::new()
        },
    })
}

fn route_identity_from_snapshot(snapshot: &ProviderRouteFactSnapshot) -> Option<RouteIdentity> {
    let provider = snapshot.provider.as_deref()?;
    let model = snapshot.model.as_deref()?;
    Some(RouteIdentity {
        provider: bounded_runtime_fact_label(provider),
        model: bounded_runtime_fact_label(model),
        route_type: normalized_evidence_route_type(snapshot.route_type.as_deref()),
        privacy_level: snapshot
            .privacy_level
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        reason: snapshot
            .reason
            .as_deref()
            .map(bounded_runtime_fact_label)
            .unwrap_or_else(|| "unknown".into()),
        provider_health_is_estimated: snapshot.provider_health_is_estimated.unwrap_or(true),
    })
}

fn runtime_fact_route_identity() -> RouteIdentity {
    RouteIdentity {
        provider: "runtime_fact".into(),
        model: "runtime_fact".into(),
        route_type: "agent_runtime".into(),
        privacy_level: "internal".into(),
        reason: "runtime_fact_answer_no_model_invocation".into(),
        provider_health_is_estimated: false,
    }
}

fn normalized_evidence_route_type(route_type: Option<&str>) -> String {
    match route_type
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "local" => "local".into(),
        "cloud" => "cloud".into(),
        "direct" | "runtime_fact" | "agent_runtime" | "none" => "agent_runtime".into(),
        "scripted" | "local_eval" => "scripted".into(),
        _ => "unknown".into(),
    }
}

fn snapshot_to_model_route_trace(snapshot: &ProviderRouteFactSnapshot) -> ModelRouteTrace {
    ModelRouteTrace {
        provider: snapshot
            .provider
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        model: snapshot.model.clone().unwrap_or_else(|| "unknown".into()),
        route_type: snapshot
            .route_type
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        prefer_local: false,
        local_model: String::new(),
        reason: snapshot.reason.clone().unwrap_or_else(|| "unknown".into()),
        privacy_level: openlife_core::agent::RedactionLevel::None,
        latency_ms: None,
        retry_count: 0,
        fallback_reason: snapshot.fallback_reason.clone(),
        provider_health_is_estimated: snapshot.provider_health_is_estimated,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
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

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
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
    let runs = crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store
            .list_runs_for_session(session_id, 5)
            .map_err(|error| error.to_string()),
    )
    .ok()?;
    let run = runs
        .into_iter()
        .find(|run| run.status == AgentRunStatus::Completed)?;
    drop(store);
    if let Some(event_store_arc) = state.main_chat_agent_event_store.as_ref() {
        let event_store = event_store_arc.lock().await;
        if let Ok(Some(event)) = event_store.latest_completed_provider_event_for_run(&run.id) {
            return route_snapshot_from_provider_event(&event, &run);
        }
    }
    Some(deterministic_run_snapshot(&run))
}

async fn last_completed_generation_snapshot(
    state: &Arc<AppState>,
    session_id: &str,
) -> Option<ProviderRouteFactSnapshot> {
    let store_arc = state.agent_run_store.as_ref()?;
    let store = store_arc.lock().await;
    let runs = crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store
            .list_runs_for_session(session_id, 20)
            .map_err(|error| error.to_string()),
    )
    .ok()?;
    drop(store);
    let event_store_arc = state.main_chat_agent_event_store.as_ref()?;
    let event_store = event_store_arc.lock().await;
    runs.into_iter()
        .filter(|run| run.status == AgentRunStatus::Completed)
        .find_map(|run| {
            event_store
                .latest_completed_provider_event_for_run(&run.id)
                .ok()
                .flatten()
                .and_then(|event| route_snapshot_from_provider_event(&event, &run))
        })
}

async fn last_completed_generation_snapshot_any_session(
    state: &Arc<AppState>,
) -> Option<ProviderRouteFactSnapshot> {
    let store_arc = state.agent_run_store.as_ref()?;
    let store = store_arc.lock().await;
    let runs = crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store.list_runs(50, 0).map_err(|error| error.to_string()),
    )
    .ok()?;
    drop(store);
    let event_store_arc = state.main_chat_agent_event_store.as_ref()?;
    let event_store = event_store_arc.lock().await;
    runs.into_iter()
        .filter(|run| run.status == AgentRunStatus::Completed)
        .find_map(|run| {
            event_store
                .latest_completed_provider_event_for_run(&run.id)
                .ok()
                .flatten()
                .and_then(|event| route_snapshot_from_provider_event(&event, &run))
        })
}

fn route_snapshot_from_provider_event(
    event: &crate::main_chat_event_stream::MainChatAgentDurableEvent,
    run: &AgentRun,
) -> Option<ProviderRouteFactSnapshot> {
    if event.event_type != "provider.completed" {
        return None;
    }
    let provider = event.payload.get("provider")?.as_str()?;
    let model = event.payload.get("model")?.as_str()?;
    // Provider/model execution truth comes only from the exact-request durable
    // receipt. Route rationale remains AgentRun-owned metadata and is joined
    // only when it names the same provider/model, preventing cross-attempt
    // aggregation from inventing fallback or privacy facts.
    let matching_route = run
        .model_route
        .as_ref()
        .filter(|route| route.provider.eq_ignore_ascii_case(provider) && route.model == model);
    Some(ProviderRouteFactSnapshot {
        provider: Some(bounded_runtime_fact_label(provider)),
        model: Some(bounded_runtime_fact_label(model)),
        route_type: Some(if provider.eq_ignore_ascii_case("ollama") {
            "local".into()
        } else {
            "cloud".into()
        }),
        run_id: Some(bounded_runtime_fact_label(&run.id)),
        reason: Some(
            matching_route
                .map(|route| bounded_runtime_fact_label(&route.reason))
                .unwrap_or_else(|| "durable_exact_request_provider_completed".into()),
        ),
        privacy_level: Some(
            matching_route
                .map(|route| route.privacy_level.to_string())
                .unwrap_or_else(|| "unknown".into()),
        ),
        provider_health_is_estimated: matching_route
            .and_then(|route| route.provider_health_is_estimated)
            .or(Some(false)),
        fallback_reason: matching_route
            .and_then(|route| route.fallback_reason.as_deref())
            .map(bounded_runtime_fact_label),
    })
}

fn deterministic_run_snapshot(run: &AgentRun) -> ProviderRouteFactSnapshot {
    ProviderRouteFactSnapshot {
        provider: None,
        model: None,
        route_type: Some("agent_runtime".into()),
        run_id: Some(bounded_runtime_fact_label(&run.id)),
        reason: Some("no_durable_provider_completion_for_run".into()),
        privacy_level: Some("unknown".into()),
        provider_health_is_estimated: Some(false),
        fallback_reason: None,
    }
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
        reason: Some(bounded_runtime_fact_label(&route.reason)),
        privacy_level: Some(route.privacy_level.to_string()),
        provider_health_is_estimated: route.provider_health_is_estimated,
        fallback_reason: route
            .fallback_reason
            .as_deref()
            .map(bounded_runtime_fact_label),
    }
}

fn no_current_generation_snapshot() -> ProviderRouteFactSnapshot {
    ProviderRouteFactSnapshot {
        provider: None,
        model: None,
        route_type: Some("none".into()),
        run_id: None,
        reason: Some("no_model_generated".into()),
        privacy_level: Some("internal".into()),
        provider_health_is_estimated: Some(false),
        fallback_reason: None,
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
    if let Ok(decision) = scheduler
        .model_router
        .route_chat(None, scheduler.prefer_local)
    {
        return decision.to_trace();
    }

    ModelRouteTrace {
        provider: "none".into(),
        model: String::new(),
        route_type: "unknown".into(),
        prefer_local: scheduler.prefer_local,
        local_model: scheduler.local_model.clone(),
        reason: "provider_route_unobserved_without_health_snapshot".into(),
        privacy_level: openlife_core::agent::RedactionLevel::None,
        latency_ms: None,
        retry_count: 0,
        fallback_reason: None,
        provider_health_is_estimated: Some(true),
    }
}
