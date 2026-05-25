//! Production-trace payload builders.
//!
//! These functions construct the canonical `serde_json::Value` payload for
//! each `AgentRunEventType`.  Every production emit site MUST use these
//! builders (or helper closures that delegate to them) so that contract
//! tests can lock the real payload shape without duplicating hand-written
//! JSON assertions.
//!
//! All field names are **snake_case** — the frontend explainability layer
//! consumes these exact names.  camelCase is a legacy fallback only.
//!
//! ## Contract alignment
//!
//! The frontend test fixtures (`frontend/src/test/fixtures/agentRunEvents.ts`)
//! mirror the output of these builders.  A change to any builder's output
//! MUST be reflected in:
//! 1. The corresponding fixture(s)
//! 2. The contract matrix (`plans/openlife_trace_contract_matrix.md`)
//! 3. The Rust contract tests in `event_store.rs`
//! 4. The frontend `typedContract.ts` parser

use serde_json::{json, Value};

// ── Governance events (agent_spec.selected / prompt_stack.assembled /
//    context_governance.applied) ──────────────────────────────────────

/// Build the payload for `agent_spec.selected`.
///
/// Contract: `{agent_spec_id, role, privacy_policy}` — all snake_case.
pub fn build_agent_spec_selected_payload(
    agent_spec_id: impl Into<String>,
    role: impl Into<String>,
    privacy_policy: impl Into<String>,
) -> Value {
    json!({
        "agent_spec_id": agent_spec_id.into(),
        "role": role.into(),
        "privacy_policy": privacy_policy.into(),
    })
}

/// Build the payload for `prompt_stack.assembled`.
///
/// Contract: `{agent_spec_id, prompt_blocks}` where `prompt_blocks` is an
/// array of objects that each have at least `id`.  **No** `prompt_stack_id`
/// field exists — the frontend extracts block info from `prompt_blocks`
/// directly (Scheme B).
pub fn build_prompt_stack_assembled_payload(
    agent_spec_id: impl Into<String>,
    prompt_blocks: Value,
) -> Value {
    json!({
        "agent_spec_id": agent_spec_id.into(),
        "prompt_blocks": prompt_blocks,
    })
}

/// Which path is emitting `context_governance.applied`.
///
/// The **streaming/execution** path writes `privacy_policy`.
/// The **orchestrator/AgentLoop** path writes `agent_spec_privacy_policy`
/// (plus optionally `effective_privacy_policy`).
pub enum ContextGovernanceEmitter {
    /// Streaming / skill-execution path — uses `privacy_policy`.
    StreamingExecution,
    /// AgentLoop orchestrator path — uses `agent_spec_privacy_policy`.
    Orchestrator,
}

/// Build the payload for `context_governance.applied`.
///
/// Contract: `{agent_spec_id, context_included, context_excluded}` plus
/// either `privacy_policy` (StreamingExecution) or
/// `agent_spec_privacy_policy` (Orchestrator).
pub fn build_context_governance_applied_payload(
    agent_spec_id: impl Into<String>,
    context_included: Vec<String>,
    context_excluded: Vec<String>,
    privacy_policy: impl Into<String>,
    emitter: ContextGovernanceEmitter,
) -> Value {
    let pp = privacy_policy.into();
    match emitter {
        ContextGovernanceEmitter::StreamingExecution => json!({
            "agent_spec_id": agent_spec_id.into(),
            "context_included": context_included,
            "context_excluded": context_excluded,
            "privacy_policy": pp,
        }),
        ContextGovernanceEmitter::Orchestrator => json!({
            "agent_spec_id": agent_spec_id.into(),
            "context_included": context_included,
            "context_excluded": context_excluded,
            "agent_spec_privacy_policy": pp.clone(),
            "effective_privacy_policy": pp,
        }),
    }
}

// ── Typed governance events (tool.call_blocked / replay.*) ──────────

/// Build a standard `tool.call_blocked` payload.
///
/// Contract: `{status, tool_name, source, agent_spec_id}` plus at least
/// one of `block_reason` / `proposal_reason` / `failure_kind`.
///
/// `agent_spec_id` is `None` when no AgentSpec is in scope (e.g. budget
/// exceeded by the AgentLoop runtime, missing spec at shell.run gate).
/// When `None` the field is serialized as `null` — frontend
/// `typedContract.ts` treats `null | string` as a valid contract for
/// this field.
///
/// Extra fields (e.g. `proposal_id`, `target_tool_name`,
/// `wrapper_tool_name`, `reason`, `max_tool_calls`, `needs_confirmation`,
/// `permission_decision`) are merged via `extra`.  The merge uses
/// `or_insert` so core fields are never overwritten—this is safe and
/// intentional.
#[allow(clippy::too_many_arguments)]
pub fn build_tool_call_blocked_payload(
    status: impl Into<String>,
    tool_name: impl Into<String>,
    source: impl Into<String>,
    agent_spec_id: Option<impl Into<String>>,
    block_reason: Option<impl Into<String>>,
    proposal_reason: Option<impl Into<String>>,
    failure_kind: Option<impl Into<String>>,
    extra: Option<Value>,
) -> Value {
    let mut payload = json!({
        "status": status.into(),
        "tool_name": tool_name.into(),
        "source": source.into(),
        "agent_spec_id": agent_spec_id.map(|s| Value::String(s.into())),
        "block_reason": block_reason.map(|s| Value::String(s.into())),
        "proposal_reason": proposal_reason.map(|s| Value::String(s.into())),
        "failure_kind": failure_kind.map(|s| Value::String(s.into())),
    });
    if let Some(extra_map) = extra.and_then(|v| v.as_object().cloned()) {
        if let Some(obj) = payload.as_object_mut() {
            for (k, v) in extra_map {
                obj.entry(k).or_insert(v);
            }
        }
    }
    payload
}

/// Build a standard `replay.failed` payload.
///
/// Contract: `{status:"failed", run_id, action_id, replay_of_action_id}`
/// plus at least one of `block_reason` / `failure_kind`.
pub fn build_replay_failed_payload(
    run_id: impl Into<String>,
    action_id: impl Into<String>,
    replay_of_action_id: impl Into<String>,
    human_message: impl Into<String>,
    block_reason: Option<impl Into<String>>,
    failure_kind: Option<impl Into<String>>,
    extra: Option<Value>,
) -> Value {
    let run_id = run_id.into();
    let block_reason = block_reason.map(|s| s.into());
    let failure_kind = failure_kind.map(|s| s.into());
    let reason = block_reason
        .clone()
        .or_else(|| failure_kind.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let mut payload = json!({
        "status": "failed",
        "run_id": run_id,
        "original_run_id": run_id,
        "action_id": action_id.into(),
        "replay_of_action_id": replay_of_action_id.into(),
        "proposal_id": null,
        "agent_spec_id": null,
        "reason": reason,
        "human_message": human_message.into(),
        "block_reason": block_reason.map(Value::String),
        "failure_kind": failure_kind.map(Value::String),
    });
    if let Some(extra_map) = extra.and_then(|v| v.as_object().cloned()) {
        if let Some(obj) = payload.as_object_mut() {
            for (k, v) in extra_map {
                obj.entry(k).or_insert(v);
            }
        }
    }
    payload
}

/// Build a `replay.started` payload.
pub fn build_replay_started_payload(
    run_id: impl Into<String>,
    action_id: impl Into<String>,
    replay_of_action_id: impl Into<String>,
    agent_spec_id: impl Into<String>,
    tool_name: impl Into<String>,
    source: impl Into<String>,
) -> Value {
    let run_id = run_id.into();
    json!({
        "status": "started",
        "run_id": run_id,
        "original_run_id": run_id,
        "action_id": action_id.into(),
        "replay_of_action_id": replay_of_action_id.into(),
        "proposal_id": null,
        "agent_spec_id": agent_spec_id.into(),
        "tool_name": tool_name.into(),
        "source": source.into(),
    })
}

/// Build a `replay.completed` payload.
#[allow(clippy::too_many_arguments)]
pub fn build_replay_completed_payload(
    outcome_status: impl Into<String>,
    run_id: impl Into<String>,
    action_id: impl Into<String>,
    replay_of_action_id: impl Into<String>,
    agent_spec_id: impl Into<String>,
    tool_name: impl Into<String>,
    source: impl Into<String>,
    block_reason: Option<impl Into<String>>,
    proposal_reason: Option<impl Into<String>>,
    failure_kind: Option<impl Into<String>>,
) -> Value {
    let run_id = run_id.into();
    let block_reason = block_reason.map(|s| s.into());
    let proposal_reason = proposal_reason.map(|s| s.into());
    let failure_kind = failure_kind.map(|s| s.into());
    let reason = block_reason
        .clone()
        .or_else(|| proposal_reason.clone())
        .or_else(|| failure_kind.clone())
        .unwrap_or_else(|| "completed".to_string());
    json!({
        "status": outcome_status.into(),
        "run_id": run_id,
        "original_run_id": run_id,
        "action_id": action_id.into(),
        "replay_of_action_id": replay_of_action_id.into(),
        "proposal_id": null,
        "agent_spec_id": agent_spec_id.into(),
        "tool_name": tool_name.into(),
        "source": source.into(),
        "reason": reason,
        "block_reason": block_reason.map(Value::String),
        "proposal_reason": proposal_reason.map(Value::String),
        "failure_kind": failure_kind.map(Value::String),
    })
}

// ── Generic-failure events ──────────────────────────────────────────

/// Build a generic `model.failed` payload.
pub fn build_model_failed_payload(
    agent_spec_id: impl Into<String>,
    error: impl Into<String>,
) -> Value {
    json!({
        "agent_spec_id": agent_spec_id.into(),
        "error": error.into(),
    })
}

/// Build a generic `run.failed` payload.
pub fn build_run_failed_payload(error: impl Into<String>) -> Value {
    json!({"error": error.into()})
}

/// Build a generic `tool.call_failed` payload.
pub fn build_tool_call_failed_payload(tool: impl Into<String>, error: impl Into<String>) -> Value {
    json!({"tool": tool.into(), "error": error.into()})
}

/// Build a generic `model.call_failed` payload.
pub fn build_model_call_failed_payload(
    provider: impl Into<String>,
    model: impl Into<String>,
    error: impl Into<String>,
) -> Value {
    json!({
        "provider": provider.into(),
        "model": model.into(),
        "error": error.into(),
    })
}

// ── Allowed typed-reason enum values ─────────────────────────────────

/// Return the set of all valid `block_reason` string values — mirrors
/// `ExecutionBlockReason::Display` output.
pub fn allowed_block_reasons() -> &'static [&'static str] {
    &[
        "agent_spec_denied",
        "agent_spec_missing",
        "tool_permission_denied",
        "network_policy_denied",
        "sandbox_denied",
        "missing_mcp_client",
        "disabled_manifest",
        "declarative_only",
        "invalid_arguments",
        "replay_spec_missing",
        "path_not_safe",
        "domain_blocked",
        "pii_detected",
        "unknown",
    ]
}

/// Return the set of all valid `proposal_reason` string values — mirrors
/// `ExecutionProposalReason::Display` output.
pub fn allowed_proposal_reasons() -> &'static [&'static str] {
    &[
        "network_policy_ask",
        "tool_permission_ask",
        "high_risk_action",
    ]
}

/// Return the set of all valid `failure_kind` string values — mirrors
/// `ExecutionFailureKind::Display` output.
pub fn allowed_failure_kinds() -> &'static [&'static str] {
    &[
        "tool_runtime_error",
        "mcp_client_error",
        "missing_mcp_server",
        "internal_error",
        "serialization_error",
    ]
}

/// Check whether `value` is a recognized typed-reason string for the
/// given `field` name (`"block_reason"`, `"proposal_reason"`, or
/// `"failure_kind"`).
///
/// Returns `true` when the string is a recognized enum variant.  Returns
/// `false` for null, empty, `"null"`, or unknown strings.
pub fn is_valid_typed_reason(field: &str, value: &str) -> bool {
    if value.is_empty() || value == "null" {
        return false;
    }
    let allowed: &[&str] = match field {
        "block_reason" => allowed_block_reasons(),
        "proposal_reason" => allowed_proposal_reasons(),
        "failure_kind" => allowed_failure_kinds(),
        _ => return false,
    };
    allowed.contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_block_reasons_known() {
        for reason in allowed_block_reasons() {
            assert!(
                is_valid_typed_reason("block_reason", reason),
                "{} should be valid block_reason",
                reason
            );
        }
    }

    #[test]
    fn test_allowed_proposal_reasons_known() {
        for reason in allowed_proposal_reasons() {
            assert!(
                is_valid_typed_reason("proposal_reason", reason),
                "{} should be valid proposal_reason",
                reason
            );
        }
    }

    #[test]
    fn test_allowed_failure_kinds_known() {
        for kind in allowed_failure_kinds() {
            assert!(
                is_valid_typed_reason("failure_kind", kind),
                "{} should be valid failure_kind",
                kind
            );
        }
    }

    #[test]
    fn test_rejects_invalid_enum_variant() {
        assert!(!is_valid_typed_reason(
            "block_reason",
            "not_a_real_enum_variant"
        ));
        assert!(!is_valid_typed_reason(
            "proposal_reason",
            "not_a_real_enum_variant"
        ));
        assert!(!is_valid_typed_reason(
            "failure_kind",
            "not_a_real_enum_variant"
        ));
    }

    #[test]
    fn test_rejects_null_and_empty() {
        assert!(!is_valid_typed_reason("block_reason", "null"));
        assert!(!is_valid_typed_reason("block_reason", ""));
        assert!(!is_valid_typed_reason("proposal_reason", "null"));
        assert!(!is_valid_typed_reason("proposal_reason", ""));
        assert!(!is_valid_typed_reason("failure_kind", "null"));
        assert!(!is_valid_typed_reason("failure_kind", ""));
    }

    #[test]
    fn test_builders_produce_snake_case_fields() {
        // agent_spec.selected
        let p = build_agent_spec_selected_payload("main.default", "main", "local_only");
        assert_eq!(p["agent_spec_id"].as_str(), Some("main.default"));
        assert_eq!(p["role"].as_str(), Some("main"));
        assert_eq!(p["privacy_policy"].as_str(), Some("local_only"));
        assert!(p.get("agentSpecId").is_none());

        // prompt_stack.assembled
        let blocks = json!([{"id": "bs", "version": "1.0.0"}]);
        let p = build_prompt_stack_assembled_payload("main.default", blocks);
        assert_eq!(p["agent_spec_id"].as_str(), Some("main.default"));
        assert!(p["prompt_blocks"].is_array());
        assert!(p.get("prompt_stack_id").is_none());
        assert!(p.get("promptStackId").is_none());

        // context_governance.applied — streaming
        let p = build_context_governance_applied_payload(
            "main.default",
            vec!["summary".into()],
            vec!["health".into()],
            "local_only",
            ContextGovernanceEmitter::StreamingExecution,
        );
        assert_eq!(p["agent_spec_id"].as_str(), Some("main.default"));
        assert!(p["context_included"].is_array());
        assert!(p["context_excluded"].is_array());
        assert_eq!(p["privacy_policy"].as_str(), Some("local_only"));
        assert!(p.get("agent_spec_privacy_policy").is_none());

        // context_governance.applied — orchestrator
        let p = build_context_governance_applied_payload(
            "main.default",
            vec!["summary".into()],
            vec![],
            "local_only",
            ContextGovernanceEmitter::Orchestrator,
        );
        assert_eq!(p["agent_spec_id"].as_str(), Some("main.default"));
        assert_eq!(p["agent_spec_privacy_policy"].as_str(), Some("local_only"));
        assert_eq!(p["effective_privacy_policy"].as_str(), Some("local_only"));

        // tool.call_blocked — agent_spec_id = Some
        let p = build_tool_call_blocked_payload(
            "blocked",
            "web.search",
            "builtin",
            Some("main.default"),
            Some("agent_spec_denied"),
            None::<&str>,
            None::<&str>,
            None,
        );
        assert_eq!(p["status"].as_str(), Some("blocked"));
        assert_eq!(p["tool_name"].as_str(), Some("web.search"));
        assert_eq!(p["source"].as_str(), Some("builtin"));
        assert_eq!(p["agent_spec_id"].as_str(), Some("main.default"));
        assert_eq!(p["block_reason"].as_str(), Some("agent_spec_denied"));

        // tool.call_blocked — agent_spec_id = None (serialized as null)
        let p = build_tool_call_blocked_payload(
            "blocked",
            "web.search",
            "runtime",
            None::<&str>,
            Some("invalid_arguments"),
            None::<&str>,
            None::<&str>,
            None,
        );
        assert_eq!(p["agent_spec_id"], serde_json::Value::Null);
        assert_eq!(p["block_reason"].as_str(), Some("invalid_arguments"));

        // replay.failed
        let p = build_replay_failed_payload(
            "run-1",
            "a1",
            "orig-1",
            "Run not found",
            Some("replay_spec_missing"),
            None::<&str>,
            None,
        );
        assert_eq!(p["status"].as_str(), Some("failed"));
        assert_eq!(p["block_reason"].as_str(), Some("replay_spec_missing"));
    }
}
