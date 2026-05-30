use crate::agent::types::{AgentRun, AgentRunStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct RuntimeMigrationGateInput<'a> {
    pub default_chat_uses_multi_strategy: bool,
    pub preview_run: Option<&'a AgentRun>,
    pub fallback_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMigrationGateReport {
    pub default_chat_unchanged: bool,
    pub preview_path_healthy: bool,
    pub metadata_safe_trace_ready: bool,
    pub fallback_available: bool,
    pub no_external_writes: bool,
    pub proposal_first_preserved: bool,
    pub blocking_reasons: Vec<String>,
}

pub fn evaluate_runtime_migration_gate(
    input: RuntimeMigrationGateInput<'_>,
) -> RuntimeMigrationGateReport {
    let mut report = RuntimeMigrationGateReport {
        default_chat_unchanged: !input.default_chat_uses_multi_strategy,
        preview_path_healthy: false,
        metadata_safe_trace_ready: false,
        fallback_available: input.fallback_available,
        no_external_writes: false,
        proposal_first_preserved: false,
        blocking_reasons: Vec::new(),
    };

    if !report.default_chat_unchanged {
        push_reason(&mut report, "default_chat_replaced");
    }
    if !report.fallback_available {
        push_reason(&mut report, "fallback_unavailable");
    }

    let Some(preview_run) = input.preview_run else {
        push_reason(&mut report, "preview_audit_missing");
        return report;
    };
    let Some(audit) = preview_run
        .reasoning_trace
        .as_ref()
        .and_then(|trace| trace.strategy_result.as_ref())
    else {
        push_reason(&mut report, "preview_audit_missing");
        return report;
    };

    let is_preview_runtime = audit
        .get("previewRuntime")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "multi_strategy");
    let is_preview_strategy =
        preview_run.reasoning_strategy.as_deref() == Some("multi_strategy_preview");
    let completed = preview_run.status == AgentRunStatus::Completed;
    let outer_trace_is_primary = preview_run
        .reasoning_trace
        .as_ref()
        .and_then(|trace| trace.output.as_deref())
        .is_some_and(|output| output == "multi_strategy_preview");
    let inner_run_is_child_metadata = audit
        .get("innerRunId")
        .and_then(Value::as_str)
        .map(|inner_run_id| !inner_run_id.is_empty() && inner_run_id != preview_run.id)
        .unwrap_or(true);

    report.preview_path_healthy = is_preview_runtime
        && is_preview_strategy
        && completed
        && outer_trace_is_primary
        && inner_run_is_child_metadata;
    if !report.preview_path_healthy {
        push_reason(&mut report, "preview_path_unhealthy");
    }
    if !outer_trace_is_primary {
        push_reason(&mut report, "preview_outer_trace_not_primary");
    }
    if !inner_run_is_child_metadata {
        push_reason(&mut report, "preview_inner_run_not_child_metadata");
    }

    let metadata_safe_flag = audit
        .get("metadataSafe")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let preview_run_drops_user_input = preview_run.user_input.is_none();
    let audit_is_metadata_safe = !contains_metadata_unsafe_content(audit);
    report.metadata_safe_trace_ready =
        metadata_safe_flag && preview_run_drops_user_input && audit_is_metadata_safe;
    if !report.metadata_safe_trace_ready {
        push_reason(&mut report, "preview_trace_not_metadata_safe");
    }

    let write_control = audit.get("writeControl");
    let declared_write_count = write_count(write_control, "declaredWriteStepCount");
    let proposal_required_count = write_count(write_control, "proposalRequiredStepCount");
    let blocked_count = write_count(write_control, "blockedStepCount");
    let executed_declared_writes =
        declared_write_count.saturating_sub(proposal_required_count + blocked_count);

    report.no_external_writes = preview_run.actions.is_empty()
        && preview_run.observations.is_empty()
        && preview_run.tool_call_count == 0
        && executed_declared_writes == 0;
    if !report.no_external_writes {
        push_reason(&mut report, "external_write_risk_detected");
    }

    report.proposal_first_preserved = declared_write_count == 0
        || proposal_required_count + blocked_count >= declared_write_count;
    if !report.proposal_first_preserved {
        push_reason(&mut report, "proposal_first_not_preserved");
    }

    report
}

fn push_reason(report: &mut RuntimeMigrationGateReport, reason: &str) {
    if !report
        .blocking_reasons
        .iter()
        .any(|existing| existing == reason)
    {
        report.blocking_reasons.push(reason.to_string());
    }
}

fn write_count(write_control: Option<&Value>, field: &str) -> u64 {
    write_control
        .and_then(|value| value.get(field))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn contains_metadata_unsafe_content(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            is_metadata_unsafe_key(key) || contains_metadata_unsafe_content(value)
        }),
        Value::Array(items) => items.iter().any(contains_metadata_unsafe_content),
        Value::String(text) => looks_like_email(text),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn is_metadata_unsafe_key(key: &str) -> bool {
    matches!(
        normalize_key(key).as_str(),
        "rawprompt"
            | "rawmemorycontext"
            | "rawmemory"
            | "rawusertext"
            | "userinput"
            | "usertext"
            | "memorycontext"
            | "messages"
            | "prompt"
    )
}

fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn looks_like_email(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let token = token.trim_matches(|ch: char| {
            matches!(ch, ',' | ';' | ':' | '"' | '\'' | '(' | ')' | '[' | ']')
        });
        let Some((local, domain)) = token.split_once('@') else {
            return false;
        };
        !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    })
}
