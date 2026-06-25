use chrono::{Datelike, Offset};
use openlife_core::llm::ChatMessage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use crate::AppState;

pub(crate) const RUNTIME_FACT_SOURCE_TYPE: &str = "runtime_fact";
pub(crate) const RUNTIME_FACT_KEY_DATE: &str = "runtime.current_time.date";
pub(crate) const RUNTIME_FACT_KEY_TIME: &str = "runtime.current_time.time";
pub(crate) const RUNTIME_FACT_KEY_WEEKDAY: &str = "runtime.current_time.weekday";
pub(crate) const RUNTIME_FACT_KEY_TIMEZONE: &str = "runtime.current_time.timezone";
pub(crate) const RUNTIME_FACT_KEY_TRACE_GAP: &str = "runtime.current_time.trace_gap";
pub(crate) const RUNTIME_FACT_PROVIDER_GENERATION_PATH: &str = "main_chat_runtime_fact";

const SLICE_A_SCENARIOS: [&str; 6] = ["RF-01", "RF-02", "RF-03", "RF-04", "RF-05", "RF-06"];
const FIXED_CLOCK_RFC3339: &str = "2026-06-23T09:15:00+08:00";

#[derive(Debug, Clone)]
pub enum MainChatRuntimeClockSource {
    LocalSystem,
    Fixed(chrono::DateTime<chrono::FixedOffset>),
    Unavailable,
}

impl Default for MainChatRuntimeClockSource {
    fn default() -> Self {
        Self::LocalSystem
    }
}

impl MainChatRuntimeClockSource {
    fn now(&self) -> Option<chrono::DateTime<chrono::FixedOffset>> {
        match self {
            Self::LocalSystem => {
                let now = chrono::Local::now();
                let fixed_offset = now.offset().fix();
                Some(now.with_timezone(&fixed_offset))
            }
            Self::Fixed(now) => Some(*now),
            Self::Unavailable => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainChatRuntimeClockIntent {
    AskCurrentWeekday,
    AskCurrentDate,
    AskCurrentTime,
}

impl MainChatRuntimeClockIntent {
    fn as_str(self) -> &'static str {
        match self {
            Self::AskCurrentWeekday => "ask_current_weekday",
            Self::AskCurrentDate => "ask_current_date",
            Self::AskCurrentTime => "ask_current_time",
        }
    }

    fn fact_keys(self) -> Vec<&'static str> {
        match self {
            Self::AskCurrentWeekday => vec![
                RUNTIME_FACT_KEY_DATE,
                RUNTIME_FACT_KEY_WEEKDAY,
                RUNTIME_FACT_KEY_TIMEZONE,
            ],
            Self::AskCurrentDate => vec![
                RUNTIME_FACT_KEY_DATE,
                RUNTIME_FACT_KEY_WEEKDAY,
                RUNTIME_FACT_KEY_TIMEZONE,
            ],
            Self::AskCurrentTime => vec![
                RUNTIME_FACT_KEY_DATE,
                RUNTIME_FACT_KEY_TIME,
                RUNTIME_FACT_KEY_TIMEZONE,
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactAnswer {
    pub(crate) reply: String,
    pub(crate) intent: MainChatRuntimeClockIntent,
    pub(crate) fact_keys: Vec<&'static str>,
    pub(crate) facts: Vec<MainChatRuntimeFactBinding>,
    pub(crate) observed_at: Option<String>,
    pub(crate) source: Vec<&'static str>,
    pub(crate) authority: &'static str,
    pub(crate) freshness: &'static str,
    pub(crate) visibility: Vec<&'static str>,
    pub(crate) privacy: Vec<&'static str>,
    pub(crate) timezone: Option<String>,
    pub(crate) trace_gap: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactBinding {
    pub(crate) key: &'static str,
    pub(crate) value_shape: &'static str,
    pub(crate) value: Option<String>,
    pub(crate) source: Vec<&'static str>,
    pub(crate) authority: &'static str,
    pub(crate) freshness: &'static str,
    pub(crate) visibility: &'static str,
    pub(crate) privacy: &'static str,
    pub(crate) missing: bool,
}

impl MainChatRuntimeFactAnswer {
    pub(crate) fn generation_metadata(&self) -> Value {
        serde_json::json!({
            "sourceType": RUNTIME_FACT_SOURCE_TYPE,
            "runtimeFactKeys": self.fact_keys,
            "runtimeFacts": self.facts,
            "runtimeFactSource": self.source,
            "runtimeFactAuthority": self.authority,
            "runtimeFactFreshness": self.freshness,
            "runtimeFactVisibility": self.visibility,
            "runtimeFactPrivacy": self.privacy,
            "runtimeFactIntent": self.intent.as_str(),
            "runtimeFactObservedAt": self.observed_at,
            "runtimeFactTimezone": self.timezone,
            "runtimeFactTtl": "none",
            "runtimeFactTtlStatus": if self.trace_gap { "not_observed" } else { "fresh" },
            "runtimeFactMissingBehavior": "answer_unknown",
            "runtimeFactModelFallbackAllowed": false,
            "runtimeFactTraceGap": self.trace_gap,
            "modelGenerated": false,
            "schedulerGenerationCalled": false,
            "toolCalled": false,
            "directWritesExecuted": false,
            "legacyFallbackUsed": false,
            "providerGenerationPath": RUNTIME_FACT_PROVIDER_GENERATION_PATH,
        })
    }
}

pub(crate) fn resolve_runtime_clock_fact_answer(
    user_text: &str,
    clock_source: &MainChatRuntimeClockSource,
) -> Option<MainChatRuntimeFactAnswer> {
    let intent = classify_runtime_clock_query(user_text)?;
    let fact_keys = intent.fact_keys();
    let Some(now) = clock_source.now() else {
        let mut trace_gap_keys = fact_keys.clone();
        trace_gap_keys.push(RUNTIME_FACT_KEY_TRACE_GAP);
        return Some(MainChatRuntimeFactAnswer {
            reply: "当前时间未知：本机运行时钟不可用，无法回答当前日期或时间。".into(),
            intent,
            facts: missing_clock_fact_bindings(&fact_keys),
            fact_keys: trace_gap_keys,
            observed_at: None,
            source: vec!["local_clock"],
            authority: "runtime",
            freshness: "unknown",
            visibility: vec!["answer", "trace_only"],
            privacy: vec!["public", "internal"],
            timezone: None,
            trace_gap: true,
        });
    };

    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M").to_string();
    let weekday = chinese_weekday(now.weekday());
    let timezone = format!("UTC{}", now.format("%:z"));
    let facts = runtime_clock_fact_bindings(intent, &date, &time, weekday, &timezone);
    let reply = match intent {
        MainChatRuntimeClockIntent::AskCurrentTime => format!(
            "根据本机运行时钟，现在是 {} {}，{}（{}）。",
            date, time, weekday, timezone
        ),
        MainChatRuntimeClockIntent::AskCurrentDate
        | MainChatRuntimeClockIntent::AskCurrentWeekday => format!(
            "根据本机运行时钟，今天是 {}，{}（{}）。",
            date, weekday, timezone
        ),
    };

    Some(MainChatRuntimeFactAnswer {
        reply,
        intent,
        fact_keys,
        facts,
        observed_at: Some(now.to_rfc3339()),
        source: vec!["local_clock"],
        authority: "runtime",
        freshness: "instant",
        visibility: vec!["answer"],
        privacy: vec!["public", "internal"],
        timezone: Some(timezone),
        trace_gap: false,
    })
}

fn runtime_clock_fact_bindings(
    intent: MainChatRuntimeClockIntent,
    date: &str,
    time: &str,
    weekday: &str,
    timezone: &str,
) -> Vec<MainChatRuntimeFactBinding> {
    let mut facts = vec![
        clock_fact_binding(
            RUNTIME_FACT_KEY_DATE,
            "YYYY-MM-DD",
            Some(date),
            "answer",
            "public",
            "instant",
            false,
        ),
        clock_fact_binding(
            RUNTIME_FACT_KEY_WEEKDAY,
            "localized_weekday_label",
            Some(weekday),
            "answer",
            "public",
            "instant",
            false,
        ),
        clock_fact_binding(
            RUNTIME_FACT_KEY_TIMEZONE,
            "offset_label",
            Some(timezone),
            "answer",
            "internal",
            "instant",
            false,
        ),
    ];
    if intent == MainChatRuntimeClockIntent::AskCurrentTime {
        facts.insert(
            1,
            clock_fact_binding(
                RUNTIME_FACT_KEY_TIME,
                "HH:mm",
                Some(time),
                "answer",
                "public",
                "instant",
                false,
            ),
        );
    }
    facts
}

fn missing_clock_fact_bindings(keys: &[&'static str]) -> Vec<MainChatRuntimeFactBinding> {
    keys.iter()
        .copied()
        .map(|key| {
            let (value_shape, privacy) = match key {
                RUNTIME_FACT_KEY_DATE => ("YYYY-MM-DD", "public"),
                RUNTIME_FACT_KEY_TIME => ("HH:mm", "public"),
                RUNTIME_FACT_KEY_WEEKDAY => ("localized_weekday_label", "public"),
                RUNTIME_FACT_KEY_TIMEZONE => ("offset_label", "internal"),
                _ => ("unknown", "internal"),
            };
            clock_fact_binding(
                key,
                value_shape,
                None,
                "trace_only",
                privacy,
                "unknown",
                true,
            )
        })
        .collect()
}

fn clock_fact_binding(
    key: &'static str,
    value_shape: &'static str,
    value: Option<&str>,
    visibility: &'static str,
    privacy: &'static str,
    freshness: &'static str,
    missing: bool,
) -> MainChatRuntimeFactBinding {
    MainChatRuntimeFactBinding {
        key,
        value_shape,
        value: value.map(str::to_string),
        source: vec!["local_clock"],
        authority: "runtime",
        freshness,
        visibility,
        privacy,
        missing,
    }
}

pub(crate) fn classify_runtime_clock_query(user_text: &str) -> Option<MainChatRuntimeClockIntent> {
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

    if matches_exact_clock_phrase(
        compact,
        &[
            "今天星期几",
            "今天周几",
            "今天礼拜几",
            "星期几",
            "周几",
            "礼拜几",
        ],
    ) || matches_exact_clock_phrase(
        english_phrase,
        &[
            "what day is it",
            "what day is today",
            "what day of the week is it",
            "what weekday is it",
            "what is today's weekday",
            "today's weekday",
            "day of week today",
        ],
    ) {
        return Some(MainChatRuntimeClockIntent::AskCurrentWeekday);
    }

    if matches_exact_clock_phrase(
        compact,
        &[
            "今天几号",
            "今天日期",
            "今天是哪天",
            "今天哪一天",
            "当前日期",
            "现在日期",
        ],
    ) || matches_exact_clock_phrase(
        english_phrase,
        &[
            "today's date",
            "date today",
            "what is today's date",
            "what's today's date",
            "what is the date today",
            "what date is it",
        ],
    ) {
        return Some(MainChatRuntimeClockIntent::AskCurrentDate);
    }

    if matches_exact_clock_phrase(compact, &["现在几点", "几点了", "当前时间", "现在时间"])
        || matches_exact_clock_phrase(
            english_phrase,
            &[
                "current time",
                "time now",
                "what time is it",
                "what's the time",
                "what is the time",
                "what is the current time",
            ],
        )
    {
        return Some(MainChatRuntimeClockIntent::AskCurrentTime);
    }

    None
}

fn matches_exact_clock_phrase(value: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|phrase| value == *phrase)
}

fn trim_outer_punctuation(value: &str) -> &str {
    value
        .trim()
        .trim_matches(|ch: char| ch.is_ascii_punctuation() || is_common_cjk_punctuation(ch))
        .trim()
}

fn is_common_cjk_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '。' | '，' | '？' | '！' | '：' | '；' | '、' | '（' | '）' | '「' | '」' | '『' | '』'
    )
}

fn chinese_weekday(weekday: chrono::Weekday) -> &'static str {
    match weekday {
        chrono::Weekday::Mon => "星期一",
        chrono::Weekday::Tue => "星期二",
        chrono::Weekday::Wed => "星期三",
        chrono::Weekday::Thu => "星期四",
        chrono::Weekday::Fri => "星期五",
        chrono::Weekday::Sat => "星期六",
        chrono::Weekday::Sun => "星期日",
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsSliceReport {
    pub(crate) report_kind: &'static str,
    pub(crate) schema_version: u32,
    pub(crate) slice_id: &'static str,
    pub(crate) slice_name: &'static str,
    pub(crate) covered_scenario_ids: Vec<String>,
    pub(crate) out_of_scope_scenario_ids: Vec<String>,
    pub(crate) blocked_scenario_ids: Vec<String>,
    pub(crate) scenario_count: usize,
    pub(crate) passed_scenario_count: usize,
    pub(crate) blocked_scenario_count: usize,
    pub(crate) runtime_facts_slice_ready: bool,
    pub(crate) runtime_facts_ready: bool,
    pub(crate) ui_included: bool,
    pub(crate) source_registry_version: &'static str,
    pub(crate) ui_contract_version: &'static str,
    pub(crate) scenario_evidence: Vec<MainChatRuntimeFactsScenarioEvidence>,
    pub(crate) negative_assertion_summary: MainChatRuntimeFactsNegativeAssertionSummary,
    pub(crate) focused_test_commands: Vec<&'static str>,
    pub(crate) command_surface_proof: MainChatRuntimeFactsCommandSurfaceProof,
    pub(crate) no_silent_write_proof: bool,
    pub(crate) blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsScenarioEvidence {
    pub(crate) scenario_id: &'static str,
    pub(crate) entry_point: &'static str,
    pub(crate) user_text: &'static str,
    pub(crate) passed: bool,
    pub(crate) answer_preview: String,
    pub(crate) source_type: Option<String>,
    pub(crate) runtime_fact_keys: Vec<String>,
    pub(crate) runtime_fact_source: Vec<String>,
    pub(crate) runtime_fact_binding_count: usize,
    pub(crate) runtime_fact_authority: Option<String>,
    pub(crate) runtime_fact_freshness: Option<String>,
    pub(crate) runtime_fact_visibility: Vec<String>,
    pub(crate) runtime_fact_privacy: Vec<String>,
    pub(crate) model_generated: Option<bool>,
    pub(crate) scheduler_generation_called: Option<bool>,
    pub(crate) tool_called: Option<bool>,
    pub(crate) direct_writes_executed: Option<bool>,
    pub(crate) legacy_fallback_used: bool,
    pub(crate) provider_generation_path: Option<String>,
    pub(crate) task_session_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) trace_gap: bool,
    pub(crate) context_conflict_ignored: bool,
    pub(crate) silent_write_detected: bool,
    pub(crate) failure: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsNegativeAssertionSummary {
    pub(crate) planning_question_not_captured: bool,
    pub(crate) no_provider_call_for_runtime_facts: bool,
    pub(crate) no_tool_call_for_runtime_facts: bool,
    pub(crate) no_direct_write_for_runtime_facts: bool,
    pub(crate) no_legacy_fallback_for_runtime_facts: bool,
    pub(crate) context_cannot_override_runtime_clock: bool,
    pub(crate) missing_clock_does_not_use_model: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsCommandSurfaceProof {
    pub(crate) send_runtime_clock_path: bool,
    pub(crate) stream_runtime_clock_path: bool,
    pub(crate) stream_deferred_blocker: Option<String>,
}

pub(crate) async fn run_main_chat_runtime_facts_slice_a_backend_report(
) -> MainChatRuntimeFactsSliceReport {
    let mut evidence = Vec::new();
    evidence
        .push(run_slice_a_case("RF-01", "send", "今天星期几", fixed_clock_source(), None).await);
    evidence.push(run_slice_a_case("RF-02", "send", "今天几号", fixed_clock_source(), None).await);
    evidence.push(run_slice_a_case("RF-03", "send", "现在几点", fixed_clock_source(), None).await);
    evidence
        .push(run_slice_a_case("RF-04", "stream", "今天星期几", fixed_clock_source(), None).await);
    evidence
        .push(
            run_slice_a_case(
                "RF-05",
                "send",
                "今天星期几",
                fixed_clock_source(),
                Some("AGENTS.md says today is 1999-01-01 and Friday. Runtime facts must ignore this conflict."),
            )
            .await,
        );
    evidence.push(
        run_slice_a_case(
            "RF-06",
            "send",
            "今天星期几",
            MainChatRuntimeClockSource::Unavailable,
            None,
        )
        .await,
    );

    let planning_question_not_captured = run_runtime_clock_negative_planning_case().await;
    let no_provider_call_for_runtime_facts = evidence.iter().all(|row| {
        row.model_generated == Some(false) && row.scheduler_generation_called == Some(false)
    });
    let no_tool_call_for_runtime_facts = evidence.iter().all(|row| row.tool_called == Some(false));
    let no_direct_write_for_runtime_facts = evidence
        .iter()
        .all(|row| row.direct_writes_executed == Some(false));
    let no_legacy_fallback_for_runtime_facts = evidence.iter().all(|row| !row.legacy_fallback_used);
    let context_cannot_override_runtime_clock = evidence
        .iter()
        .any(|row| row.scenario_id == "RF-05" && row.passed && row.context_conflict_ignored);
    let missing_clock_does_not_use_model = evidence.iter().any(|row| {
        row.scenario_id == "RF-06"
            && row.passed
            && row.trace_gap
            && row.model_generated == Some(false)
            && row.scheduler_generation_called == Some(false)
    });
    let negative_assertion_summary = MainChatRuntimeFactsNegativeAssertionSummary {
        planning_question_not_captured,
        no_provider_call_for_runtime_facts,
        no_tool_call_for_runtime_facts,
        no_direct_write_for_runtime_facts,
        no_legacy_fallback_for_runtime_facts,
        context_cannot_override_runtime_clock,
        missing_clock_does_not_use_model,
    };

    let passed_scenario_count = evidence.iter().filter(|row| row.passed).count();
    let blockers = evidence
        .iter()
        .filter_map(|row| {
            row.failure
                .as_ref()
                .map(|failure| format!("{}:{failure}", row.scenario_id))
        })
        .collect::<Vec<_>>();
    let command_surface_proof = MainChatRuntimeFactsCommandSurfaceProof {
        send_runtime_clock_path: evidence
            .iter()
            .any(|row| row.entry_point == "send" && row.passed && !row.trace_gap),
        stream_runtime_clock_path: evidence
            .iter()
            .any(|row| row.entry_point == "stream" && row.passed && !row.trace_gap),
        stream_deferred_blocker: None,
    };
    let no_silent_write_proof = evidence.iter().all(|row| !row.silent_write_detected);
    let runtime_facts_slice_ready = passed_scenario_count == SLICE_A_SCENARIOS.len()
        && planning_question_not_captured
        && no_provider_call_for_runtime_facts
        && no_tool_call_for_runtime_facts
        && no_direct_write_for_runtime_facts
        && no_legacy_fallback_for_runtime_facts
        && context_cannot_override_runtime_clock
        && missing_clock_does_not_use_model
        && command_surface_proof.send_runtime_clock_path
        && command_surface_proof.stream_runtime_clock_path
        && no_silent_write_proof;

    MainChatRuntimeFactsSliceReport {
        report_kind: "main_chat_runtime_facts_slice",
        schema_version: 1,
        slice_id: "slice_a_backend",
        slice_name: "Runtime Clock Backend",
        covered_scenario_ids: SLICE_A_SCENARIOS
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        out_of_scope_scenario_ids: vec!["RF-22".into()],
        blocked_scenario_ids: Vec::new(),
        scenario_count: SLICE_A_SCENARIOS.len(),
        passed_scenario_count,
        blocked_scenario_count: 0,
        runtime_facts_slice_ready,
        runtime_facts_ready: false,
        ui_included: false,
        source_registry_version: "2026-06-25",
        ui_contract_version: "2026-06-25",
        scenario_evidence: evidence,
        negative_assertion_summary,
        focused_test_commands: vec![
            "cargo test -p openlife-tauri runtime_clock -- --nocapture",
            "cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture",
        ],
        command_surface_proof,
        no_silent_write_proof,
        blockers,
    }
}

async fn run_slice_a_case(
    scenario_id: &'static str,
    entry_point: &'static str,
    user_text: &'static str,
    clock_source: MainChatRuntimeClockSource,
    conflicting_agents_text: Option<&'static str>,
) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut source = state.runtime_clock_source.lock().await;
        *source = clock_source;
    }
    if let Some(conflicting_agents_text) = conflicting_agents_text {
        if let Err(error) = seed_conflicting_knowledge_root(&state, conflicting_agents_text).await {
            return MainChatRuntimeFactsScenarioEvidence::failed(
                scenario_id,
                entry_point,
                user_text,
                error,
            );
        }
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "provider-should-not-answer-runtime-clock".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("provider should not answer runtime clock");
    }

    let session_id = format!("runtime-facts-{entry_point}-{scenario_id}");
    let response = match entry_point {
        "send" => {
            let result = crate::main_chat_send::send_message_with_state(
                session_id,
                vec![ChatMessage {
                    role: "user".into(),
                    content: user_text.into(),
                }],
                None,
                &state,
            )
            .await;
            match result {
                Ok(result) => serde_json::to_value(result)
                    .map_err(|error| format!("serialize send response failed: {error}")),
                Err(error) => Err(error),
            }
        }
        "stream" => {
            let mut emitted_events = Vec::<(String, Value)>::new();
            let result = crate::main_chat_streaming::start_stream_message_with_state(
                session_id,
                vec![ChatMessage {
                    role: "user".into(),
                    content: user_text.into(),
                }],
                None,
                &state,
                |event, payload| emitted_events.push((event.to_string(), payload)),
            )
            .await;
            match result {
                Ok(()) => emitted_events
                    .iter()
                    .rev()
                    .find(|(event, _)| event == "stream-message-done")
                    .map(|(_, payload)| payload.clone())
                    .ok_or_else(|| "stream runtime fact case missing done payload".to_string()),
                Err(error) => Err(error),
            }
        }
        _ => Err(format!("unsupported entry point {entry_point}")),
    };

    match response {
        Ok(response) => evidence_from_runtime_fact_response(
            scenario_id,
            entry_point,
            user_text,
            response,
            conflicting_agents_text.is_some(),
        ),
        Err(error) => {
            MainChatRuntimeFactsScenarioEvidence::failed(scenario_id, entry_point, user_text, error)
        }
    }
}

impl MainChatRuntimeFactsScenarioEvidence {
    fn failed(
        scenario_id: &'static str,
        entry_point: &'static str,
        user_text: &'static str,
        failure: String,
    ) -> Self {
        Self {
            scenario_id,
            entry_point,
            user_text,
            passed: false,
            answer_preview: String::new(),
            source_type: None,
            runtime_fact_keys: Vec::new(),
            runtime_fact_source: Vec::new(),
            runtime_fact_binding_count: 0,
            runtime_fact_authority: None,
            runtime_fact_freshness: None,
            runtime_fact_visibility: Vec::new(),
            runtime_fact_privacy: Vec::new(),
            model_generated: None,
            scheduler_generation_called: None,
            tool_called: None,
            direct_writes_executed: None,
            legacy_fallback_used: false,
            provider_generation_path: None,
            task_session_id: None,
            run_id: None,
            trace_gap: false,
            context_conflict_ignored: false,
            silent_write_detected: false,
            failure: Some(failure),
        }
    }
}

fn evidence_from_runtime_fact_response(
    scenario_id: &'static str,
    entry_point: &'static str,
    user_text: &'static str,
    response: Value,
    has_context_conflict: bool,
) -> MainChatRuntimeFactsScenarioEvidence {
    let generation = response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"))
        .cloned()
        .unwrap_or(Value::Null);
    let reply = response
        .get("reply")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let runtime_fact_keys = string_array(&generation, "runtimeFactKeys");
    let runtime_fact_source = string_array(&generation, "runtimeFactSource");
    let runtime_fact_binding_count = generation
        .get("runtimeFacts")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let runtime_fact_visibility = string_array(&generation, "runtimeFactVisibility");
    let runtime_fact_privacy = string_array(&generation, "runtimeFactPrivacy");
    let model_generated = generation.get("modelGenerated").and_then(Value::as_bool);
    let scheduler_generation_called = generation
        .get("schedulerGenerationCalled")
        .and_then(Value::as_bool);
    let tool_called = generation.get("toolCalled").and_then(Value::as_bool);
    let direct_writes_executed = generation
        .get("directWritesExecuted")
        .and_then(Value::as_bool);
    let legacy_fallback_used = response
        .get("legacy_fallback_used")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let trace_gap = generation
        .get("runtimeFactTraceGap")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let expected_runtime_value_present = if trace_gap {
        reply.contains("当前时间未知")
            && runtime_fact_keys.contains(&RUNTIME_FACT_KEY_TRACE_GAP.into())
    } else {
        reply.contains("2026-06-23") && reply.contains("星期二") && reply.contains("UTC+08:00")
    };
    let context_conflict_ignored = !has_context_conflict
        || (reply.contains("2026-06-23")
            && reply.contains("星期二")
            && !reply.contains("1999-01-01")
            && !reply.contains("Friday"));
    let silent_write_detected = direct_writes_executed.unwrap_or(true)
        || response
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty());
    let passed = generation.get("sourceType").and_then(Value::as_str)
        == Some(RUNTIME_FACT_SOURCE_TYPE)
        && !runtime_fact_keys.is_empty()
        && runtime_fact_binding_count > 0
        && runtime_fact_source
            .iter()
            .any(|source| source == "local_clock")
        && generation
            .get("runtimeFactAuthority")
            .and_then(Value::as_str)
            == Some("runtime")
        && generation
            .get("runtimeFactFreshness")
            .and_then(Value::as_str)
            .is_some_and(|freshness| freshness == "instant" || freshness == "unknown")
        && runtime_fact_visibility
            .iter()
            .any(|value| value == "answer")
        && runtime_fact_privacy.iter().any(|value| value == "public")
        && model_generated == Some(false)
        && scheduler_generation_called == Some(false)
        && tool_called == Some(false)
        && direct_writes_executed == Some(false)
        && !legacy_fallback_used
        && generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            == Some(RUNTIME_FACT_PROVIDER_GENERATION_PATH)
        && expected_runtime_value_present
        && context_conflict_ignored
        && !silent_write_detected;

    MainChatRuntimeFactsScenarioEvidence {
        scenario_id,
        entry_point,
        user_text,
        passed,
        answer_preview: reply.chars().take(160).collect(),
        source_type: generation
            .get("sourceType")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_keys,
        runtime_fact_source,
        runtime_fact_binding_count,
        runtime_fact_authority: generation
            .get("runtimeFactAuthority")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_freshness: generation
            .get("runtimeFactFreshness")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_visibility,
        runtime_fact_privacy,
        model_generated,
        scheduler_generation_called,
        tool_called,
        direct_writes_executed,
        legacy_fallback_used,
        provider_generation_path: generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            .map(str::to_string),
        task_session_id: response
            .get("agent_ingress")
            .and_then(|ingress| ingress.get("agentTaskSessionId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        run_id: response
            .get("run_id")
            .or_else(|| response.get("runId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        trace_gap,
        context_conflict_ignored,
        silent_write_detected,
        failure: (!passed).then(|| "runtime fact command-surface evidence incomplete".into()),
    }
}

async fn run_runtime_clock_negative_planning_case() -> bool {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut source = state.runtime_clock_source.lock().await;
        *source = fixed_clock_source();
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "provider-planning".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("provider handled planning question");
    }
    let result = crate::main_chat_send::send_message_with_state(
        "runtime-facts-negative-planning".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "What time should I leave tomorrow?".into(),
        }],
        None,
        &state,
    )
    .await;
    let Ok(result) = result else {
        return false;
    };
    let Ok(response) = serde_json::to_value(result) else {
        return false;
    };
    let generation = response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"));
    response
        .get("reply")
        .and_then(Value::as_str)
        .is_some_and(|reply| reply.contains("provider handled planning question"))
        && generation
            .and_then(|value| value.get("sourceType"))
            .and_then(Value::as_str)
            != Some(RUNTIME_FACT_SOURCE_TYPE)
        && generation
            .and_then(|value| value.get("modelGenerated"))
            .and_then(Value::as_bool)
            == Some(true)
        && generation
            .and_then(|value| value.get("schedulerGenerationCalled"))
            .and_then(Value::as_bool)
            == Some(true)
}

async fn seed_conflicting_knowledge_root(
    state: &Arc<AppState>,
    conflicting_agents_text: &str,
) -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "openlife-runtime-facts-conflict-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("create runtime facts conflict root failed: {error}"))?;
    std::fs::write(root.join("AGENTS.md"), conflicting_agents_text)
        .map_err(|error| format!("write runtime facts conflict AGENTS.md failed: {error}"))?;
    let mut config = state.config.lock().await;
    config
        .system
        .knowledge_roots
        .push(root.to_string_lossy().to_string());
    Ok(())
}

fn fixed_clock_source() -> MainChatRuntimeClockSource {
    MainChatRuntimeClockSource::Fixed(
        chrono::DateTime::parse_from_rfc3339(FIXED_CLOCK_RFC3339).expect("fixed clock parses"),
    )
}

fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
