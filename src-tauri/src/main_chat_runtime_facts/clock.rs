use chrono::{Datelike, Offset};
use serde_json::Value;

use super::contract::{
    matches_exact_runtime_fact_phrase, trim_outer_punctuation, MainChatRuntimeClockIntent,
    MainChatRuntimeFactAnswer, MainChatRuntimeFactBinding, RUNTIME_FACT_KEY_DATE,
    RUNTIME_FACT_KEY_TIME, RUNTIME_FACT_KEY_TIMEZONE, RUNTIME_FACT_KEY_TRACE_GAP,
    RUNTIME_FACT_KEY_WEEKDAY,
};

#[derive(Debug, Clone, Default)]
pub enum MainChatRuntimeClockSource {
    #[default]
    LocalSystem,
    Fixed(chrono::DateTime<chrono::FixedOffset>),
    Unavailable,
}

impl MainChatRuntimeClockSource {
    pub(crate) fn now(&self) -> Option<chrono::DateTime<chrono::FixedOffset>> {
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
            intent: intent.as_str().into(),
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
            extra_metadata: Value::Null,
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
        intent: intent.as_str().into(),
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
        extra_metadata: Value::Null,
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

    if matches_exact_runtime_fact_phrase(
        compact,
        &[
            "今天星期几",
            "今天周几",
            "今天礼拜几",
            "星期几",
            "周几",
            "礼拜几",
        ],
    ) || matches_exact_runtime_fact_phrase(
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

    if matches_exact_runtime_fact_phrase(
        compact,
        &[
            "今天几号",
            "今天日期",
            "今天是哪天",
            "今天哪一天",
            "当前日期",
            "现在日期",
        ],
    ) || matches_exact_runtime_fact_phrase(
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

    if matches_exact_runtime_fact_phrase(compact, &["现在几点", "几点了", "当前时间", "现在时间"])
        || matches_exact_runtime_fact_phrase(
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
