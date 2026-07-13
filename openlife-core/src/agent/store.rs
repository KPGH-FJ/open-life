use crate::agent::action_executor::{
    tool_executor::ObservedToolBodyAdmission, BoundContentReceiptIssuer,
};
use crate::agent::types::{
    AgentRun, AgentRunReceiptKey, AgentRunStatus, AgentTaskKind, ContentReceipt,
};
use crate::memory::CanonicalConversationMessageProof;
use crate::persistence_outbox::{
    self, CanonicalMutationReceipt, ProjectionDelivery, ProjectionSummary,
};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAX_AGENT_RUN_PROPOSAL_LINKS: usize = 256;
const AGENT_RUN_PAYLOAD_VERSION: i64 = 7;
const MAX_AGENT_RUN_ID_CHARS: usize = 192;
const MAX_AGENT_RUN_REF_CHARS: usize = 384;
const MAX_AGENT_RUN_RECEIPT_BYTES: usize = 16 * 1024 * 1024;
const MAX_AGENT_RUN_COLLECTION_ITEMS: usize = 4_096;
const MAX_AGENT_RUN_NESTED_REFS: usize = 512;
const MAX_AGENT_RUN_STORED_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
const BOUND_CONTENT_PENDING_TTL_SECONDS: i64 = 15 * 60;
const MAX_BOUND_CONTENT_PENDING_PER_RUN: i64 = 64;
const MAX_BOUND_CONTENT_PENDING_GLOBAL: i64 = 1_024;
const MAX_BOUND_CONTENT_ATTACHED_RETAINED: i64 = 4_096;
const BOUND_CONTENT_ATTACHED_RETENTION_SECONDS: i64 = 24 * 60 * 60;
const AGENT_RUN_V7_PHYSICAL_PURGE_MARKER: &str = "agent_run_v7_physical_purge_complete";
const AGENT_RUN_SELECT_COLUMNS: &str = "id, task_id, session_id, status, kind,
     context_summary_json, model_route_json, output_preview, error_json,
     generated_proposals_json, actions_json, observations_json,
     reasoning_strategy, hs_selection_audit_json, behavior_checks_json,
     status_updates_json, step_count, tool_call_count, deleted_at, delete_reason,
     started_at, finished_at, input_ref, input_digest, reasoning_trace_digest,
     payload_minimized_version, legacy_payload_unverified";
const AGENT_RUN_SELECT_COLUMNS_RUN: &str = "run.id, run.task_id, run.session_id, run.status, run.kind,
     run.context_summary_json, run.model_route_json, run.output_preview, run.error_json,
     run.generated_proposals_json, run.actions_json, run.observations_json,
     run.reasoning_strategy, run.hs_selection_audit_json, run.behavior_checks_json,
     run.status_updates_json, run.step_count, run.tool_call_count, run.deleted_at, run.delete_reason,
     run.started_at, run.finished_at, run.input_ref, run.input_digest, run.reasoning_trace_digest,
     run.payload_minimized_version, run.legacy_payload_unverified";

/// One canonical product-live predicate for every AgentRun read/reconciliation
/// path. `deleted_at` is the local row marker; an unsuperseded canonical
/// tombstone remains authoritative even if an anomalous legacy row lost that
/// marker. Recovery-only APIs deliberately do not use this predicate.
const LIVE_AGENT_RUN_SQL_PREDICATE: &str = "run.deleted_at IS NULL
    AND NOT EXISTS (
        SELECT 1 FROM canonical_tombstones tombstone
        WHERE tombstone.aggregate_kind = 'agent_run'
          AND tombstone.aggregate_id = run.id
          AND tombstone.superseded_at IS NULL
    )";

fn canonical_agent_run_database_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return std::fs::canonicalize(path).with_context(|| {
            format!(
                "canonicalize existing agent run database slot before open: {:?}",
                path
            )
        });
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let canonical_parent = std::fs::canonicalize(parent.unwrap_or_else(|| Path::new(".")))
        .with_context(|| {
            format!(
                "canonicalize agent run database parent before open: {:?}",
                path
            )
        })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("agent_run_database_file_name_missing"))?;
    Ok(canonical_parent.join(file_name))
}

fn open_agent_run_database_with_stable_slot<F, G>(
    path: &Path,
    after_expected_before_open: F,
    after_open_before_validation: G,
) -> Result<(
    Connection,
    PathBuf,
    crate::sqlite_migration::SqliteSlotOwnerLease,
)>
where
    F: FnOnce(),
    G: FnOnce(),
{
    let expected_slot = canonical_agent_run_database_path(path)?;
    let owner_lease =
        crate::sqlite_migration::SqliteSlotOwnerLease::acquire(&expected_slot, "agent_run_store")?;
    after_expected_before_open();
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open agent_runs db at {path:?}"))?;
    AgentRunStore::configure_writable_connection(&conn, true)?;
    after_open_before_validation();
    let observed_slot =
        crate::sqlite_migration::canonical_opened_main_database_path(&conn, "agent_run_store")?
            .ok_or_else(|| anyhow::anyhow!("agent_run_persistent_database_path_missing"))?;
    if observed_slot != expected_slot {
        anyhow::bail!(
            "agent_run_database_slot_changed_during_open:{}!={}",
            expected_slot.display(),
            observed_slot.display()
        );
    }
    owner_lease.bind_opened_database_identity()?;
    Ok((conn, observed_slot, owner_lease))
}

fn open_agent_run_database_read_only_with_stable_slot<F, G>(
    path: &Path,
    after_expected_before_open: F,
    after_open_before_validation: G,
) -> Result<(
    Connection,
    PathBuf,
    crate::sqlite_migration::SqliteDatabaseIdentityGuard,
)>
where
    F: FnOnce(),
    G: FnOnce(),
{
    let expected_slot = canonical_agent_run_database_path(path)?;
    after_expected_before_open();
    let conn =
        crate::sqlite_migration::open_existing_read_only(path, "agent_run_store", &["agent_runs"])?;
    after_open_before_validation();
    let observed_slot = crate::sqlite_migration::canonical_opened_main_database_path(
        &conn,
        "agent_run_store_read_only",
    )?
    .ok_or_else(|| anyhow::anyhow!("agent_run_read_only_database_path_missing"))?;
    if observed_slot != expected_slot {
        anyhow::bail!(
            "agent_run_database_slot_changed_during_read_only_open:{}!={}",
            expected_slot.display(),
            observed_slot.display()
        );
    }
    let identity_guard = crate::sqlite_migration::SqliteDatabaseIdentityGuard::capture(
        &observed_slot,
        "agent_run_store_read_only",
    )?;
    Ok((conn, observed_slot, identity_guard))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentRunTableRebuildFault {
    None,
    #[cfg(test)]
    AfterCopy,
    #[cfg(test)]
    AfterTableSwapBeforePurge,
}

// Execution-store boundary: status/kind/count/timestamps are typed state;
// run/task/action/observation/proposal/source identifiers are bounded refs;
// user, assistant, reasoning, error, and tool bodies are retained only as
// content-absent digest receipts. Canonical bodies stay in their domain owner.

fn metadata_digest(key: &AgentRunReceiptKey, purpose: &str, value: &str) -> String {
    key.sign(purpose, value)
}

fn is_exact_metadata_digest(value: &str) -> bool {
    value.strip_prefix("hmac-sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}

fn canonical_owner_content_digest(value: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, value.as_bytes());
    let hex = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn is_metadata_safe_text_receipt(kind: &str, value: &str) -> bool {
    let Some(receipt) = value.strip_prefix(&format!("{kind}:bytes=")) else {
        return false;
    };
    let Some((byte_count, digest)) = receipt.split_once(':') else {
        return false;
    };
    byte_count.parse::<usize>().ok().is_some_and(|parsed| {
        parsed <= MAX_AGENT_RUN_RECEIPT_BYTES && parsed.to_string() == byte_count
    }) && is_exact_metadata_digest(digest)
}

#[derive(Debug, Clone, Copy)]
enum ReceiptOrigin<'a> {
    /// Caller input or a pre-v4 row. Receipt-looking strings/objects are data,
    /// never proof that minimization already happened.
    NewInput(&'a AgentRunReceiptKey),
    /// A value decoded from the current persisted row and being validated for
    /// canonical shape.
    StoredCanonical(&'a AgentRunReceiptKey),
}

impl<'a> ReceiptOrigin<'a> {
    fn key(self) -> &'a AgentRunReceiptKey {
        match self {
            Self::NewInput(key) | Self::StoredCanonical(key) => key,
        }
    }

    fn is_stored(self) -> bool {
        matches!(self, Self::StoredCanonical(_))
    }
}

fn metadata_safe_text_receipt(kind: &str, value: &str, origin: ReceiptOrigin<'_>) -> String {
    if origin.is_stored() && is_metadata_safe_text_receipt(kind, value) {
        value.to_string()
    } else {
        format!(
            "{kind}:bytes={}:{}",
            value.len(),
            metadata_digest(origin.key(), kind, value)
        )
    }
}

fn metadata_safe_label_or_ref(kind: &str, value: &str, origin: ReceiptOrigin<'_>) -> String {
    let trimmed = value.trim();
    if (origin.is_stored() && is_metadata_safe_text_receipt(kind, trimmed))
        || is_authoritative_metadata_reference(kind, trimmed)
    {
        trimmed.to_string()
    } else {
        metadata_safe_text_receipt(kind, value, origin)
    }
}

fn is_authoritative_metadata_reference(kind: &str, value: &str) -> bool {
    if value.is_empty() || value.len() > 384 || value.trim() != value {
        return false;
    }
    if kind.ends_with("digest") || kind.ends_with("_hash") {
        return is_exact_metadata_digest(value);
    }
    if uuid::Uuid::parse_str(value).is_ok() {
        return true;
    }
    match kind {
        "action_id" => {
            value.starts_with("action-")
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':')
                })
        }
        "observation_id" => {
            value.starts_with("observation-")
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':')
                })
        }
        "memory_source_ref" => matches!(
            canonical_conversation_message_parts(value),
            Some((_session_id, _message_id))
        ),
        "life_model_section" => matches!(
            value,
            "identity"
                | "values"
                | "goals"
                | "preferences"
                | "constraints"
                | "relationships"
                | "routines"
        ),
        _ => false,
    }
}

fn metadata_safe_typed_code_or_receipt(
    kind: &str,
    value: &str,
    origin: ReceiptOrigin<'_>,
) -> String {
    let trimmed = value.trim();
    if is_registered_metadata_code(kind, trimmed) {
        trimmed.to_string()
    } else {
        metadata_safe_text_receipt(kind, value, origin)
    }
}

fn is_registered_metadata_code(kind: &str, value: &str) -> bool {
    match kind {
        "action_type" => matches!(
            value,
            "read"
                | "read_only"
                | "write"
                | "external_write"
                | "proposal"
                | "tool"
                | "mcp_tool"
                | "memory.search"
                | "memory_governance"
                | "calendar.read"
                | "calendar.write"
                | "file.read"
                | "web.read"
                | "web.search"
                | "shell"
                | "unknown"
        ),
        "tool_id" | "tool_name" => matches!(
            value,
            "memory.search"
                | "memory_search"
                | "memory.governance"
                | "calendar.read"
                | "calendar.write"
                | "file.read"
                | "file_read"
                | "web.read"
                | "web.search"
                | "web_search"
                | "mcp.read"
                | "mcp.read_only"
        ),
        "tool_source" | "observation_source" => matches!(
            value,
            "builtin"
                | "bundled"
                | "registered_manifest"
                | "planned_action"
                | "memory_search"
                | "tool_gateway"
                | "provider"
                | "system"
        ),
        "capability" => matches!(
            value,
            "read"
                | "write"
                | "network"
                | "filesystem"
                | "memory.read"
                | "memory.write"
                | "calendar"
                | "email"
                | "utility"
                | "external_side_effect"
        ),
        "permission_decision" => matches!(
            value,
            "allowed"
                | "blocked"
                | "denied"
                | "consent_required"
                | "read_only_memory_search"
                | "waiting_permission"
        ),
        "action_category" => matches!(value, "read" | "write" | "proposal" | "unknown"),
        "impact_kind" => matches!(value, "none" | "advisory" | "behavior" | "policy"),
        _ => false,
    }
}

fn metadata_safe_enum_or_receipt(
    kind: &str,
    value: &str,
    allowed: &[&str],
    origin: ReceiptOrigin<'_>,
) -> String {
    if allowed.contains(&value) {
        value.to_string()
    } else {
        metadata_safe_text_receipt(kind, value, origin)
    }
}

fn metadata_safe_bounded_summary(
    kind: &str,
    value: &str,
    max_chars: usize,
    origin: ReceiptOrigin<'_>,
) -> String {
    let _ = max_chars;
    if origin.is_stored() && is_metadata_safe_text_receipt(kind, value) {
        return value.to_string();
    }
    // A short string can still be a password, medical fact, or user-authored
    // identity. Execution stores do not own those bodies, so length is never
    // evidence that a free-text summary is safe to copy.
    metadata_safe_text_receipt(kind, value, origin)
}

fn is_metadata_safe_value_receipt(kind: &str, value: &serde_json::Value) -> bool {
    value.as_object().is_some_and(|receipt| {
        receipt.len() == 4
            && receipt.get("kind").and_then(serde_json::Value::as_str) == Some(kind)
            && receipt
                .get("byteCount")
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|bytes| bytes <= MAX_AGENT_RUN_RECEIPT_BYTES as u64)
            && receipt
                .get("digest")
                .and_then(serde_json::Value::as_str)
                .is_some_and(is_exact_metadata_digest)
            && receipt
                .get("contentStored")
                .and_then(serde_json::Value::as_bool)
                == Some(false)
    })
}

fn metadata_safe_value_receipt(
    kind: &str,
    value: &serde_json::Value,
    origin: ReceiptOrigin<'_>,
) -> serde_json::Value {
    if origin.is_stored() && is_metadata_safe_value_receipt(kind, value) {
        return value.clone();
    }
    let serialized = value.to_string();
    serde_json::json!({
        "kind": kind,
        "byteCount": serialized.len(),
        "digest": metadata_digest(origin.key(), kind, &serialized),
        "contentStored": false,
    })
}

fn minimize_react_trace(
    trace: &crate::agent::types::ReactActionTraceEnvelope,
    origin: ReceiptOrigin<'_>,
) -> crate::agent::types::ReactActionTraceEnvelope {
    let mut minimized = trace.clone();
    minimized.run_id = trace
        .run_id
        .as_deref()
        .map(|value| metadata_safe_label_or_ref("run_id", value, origin));
    minimized.action_id = metadata_safe_label_or_ref("action_id", &trace.action_id, origin);
    minimized.action_type =
        metadata_safe_typed_code_or_receipt("action_type", &trace.action_type, origin);
    if trace.metadata_safe {
        minimized.tool_id = metadata_safe_typed_code_or_receipt("tool_id", &trace.tool_id, origin);
        minimized.tool_name =
            metadata_safe_typed_code_or_receipt("tool_name", &trace.tool_name, origin);
        minimized.tool_source =
            metadata_safe_typed_code_or_receipt("tool_source", &trace.tool_source, origin);
    } else {
        minimized.tool_id = metadata_safe_text_receipt("tool_id", &trace.tool_id, origin);
        minimized.tool_name = metadata_safe_text_receipt("tool_name", &trace.tool_name, origin);
        minimized.tool_source =
            metadata_safe_text_receipt("tool_source", &trace.tool_source, origin);
    }
    minimized.action_category =
        metadata_safe_typed_code_or_receipt("action_category", &trace.action_category, origin);
    minimized.risk_level = metadata_safe_enum_or_receipt(
        "risk_level",
        &trace.risk_level,
        &["low", "medium", "high", "critical", "unknown"],
        origin,
    );
    minimized.permission_decision = trace
        .permission_decision
        .as_deref()
        .map(|value| metadata_safe_typed_code_or_receipt("permission_decision", value, origin));
    minimized.status = metadata_safe_enum_or_receipt(
        "status",
        &trace.status,
        &[
            "queued",
            "running",
            "succeeded",
            "failed",
            "blocked",
            "needs_confirmation",
            "waiting_permission",
            "cancelled",
            "local_aborted",
            "remote_unknown",
        ],
        origin,
    );
    minimized.proposal_id = trace
        .proposal_id
        .as_deref()
        .map(|value| metadata_safe_label_or_ref("proposal_id", value, origin));
    minimized.observation_id = trace
        .observation_id
        .as_deref()
        .map(|value| metadata_safe_label_or_ref("observation_id", value, origin));
    minimized.observation_status = trace
        .observation_status
        .as_deref()
        .map(|value| metadata_safe_label_or_ref("observation_status", value, origin));
    minimized.output_preview = None;
    // A receipt is meaningful only when it is bound to its owning AgentAction.
    // `canonicalize_execution_records` restores a verified copy after both
    // raw and persisted identities are available. Receipt-backed observations
    // drop their duplicate ReAct trace entirely.
    minimized.output_receipt = if origin.is_stored() {
        trace.output_receipt.clone()
    } else {
        None
    };
    minimized.metadata_safe = true;
    minimized
}

fn minimize_action(
    action: &crate::agent::types::AgentAction,
    value_origin: ReceiptOrigin<'_>,
) -> crate::agent::types::AgentAction {
    let mut minimized = action.clone();
    minimized.id = metadata_safe_label_or_ref("action_id", &action.id, value_origin);
    minimized.action_type =
        metadata_safe_typed_code_or_receipt("action_type", &action.action_type, value_origin);
    minimized.target = action
        .target
        .as_deref()
        .map(|value| metadata_safe_text_receipt("action_target", value, value_origin));
    minimized.input = metadata_safe_value_receipt("action_input", &action.input, value_origin);
    minimized.output = action.output.as_ref().map(|value| {
        if value_origin.is_stored() && is_bound_content_value_ref(value) {
            value.clone()
        } else {
            metadata_safe_value_receipt("action_output", value, value_origin)
        }
    });
    minimized.status = metadata_safe_enum_or_receipt(
        "action_status",
        &action.status,
        &[
            "queued",
            "pending",
            "running",
            "succeeded",
            "failed",
            "blocked",
            "needs_confirmation",
            "waiting_permission",
            "cancelled",
            "local_aborted",
            "remote_unknown",
        ],
        value_origin,
    );
    minimized.permission_decision = action.permission_decision.as_deref().map(|value| {
        metadata_safe_typed_code_or_receipt("permission_decision", value, value_origin)
    });
    minimized.error = action.error.as_deref().map(|value| {
        if value_origin.is_stored() && is_bound_content_text_ref(value) {
            value.to_string()
        } else {
            metadata_safe_text_receipt("action_error", value, value_origin)
        }
    });
    if let Some(scope) = minimized.tool_scope.as_mut() {
        scope.tool_id =
            metadata_safe_typed_code_or_receipt("tool_id", &scope.tool_id, value_origin);
        scope.tool_name =
            metadata_safe_typed_code_or_receipt("tool_name", &scope.tool_name, value_origin);
        scope.source =
            metadata_safe_typed_code_or_receipt("tool_source", &scope.source, value_origin);
        scope.risk_level = metadata_safe_enum_or_receipt(
            "risk_level",
            &scope.risk_level,
            &["low", "medium", "high", "critical", "unknown"],
            value_origin,
        );
        scope.capabilities = scope
            .capabilities
            .iter()
            .map(|value| metadata_safe_typed_code_or_receipt("capability", value, value_origin))
            .collect();
        scope.action_type =
            metadata_safe_typed_code_or_receipt("action_type", &scope.action_type, value_origin);
    }
    // AgentAction.output and AgentObservation.content already persist one
    // purpose-scoped body receipt each. Persisting a second ContentReceipt
    // would create an independently mutable claim about the same adapter body,
    // so the execution-store projection intentionally drops it.
    minimized.react_trace = action
        .react_trace
        .as_ref()
        .map(|trace| minimize_react_trace(trace, value_origin));
    minimized
}

fn minimize_observation(
    observation: &crate::agent::types::AgentObservation,
    value_origin: ReceiptOrigin<'_>,
) -> crate::agent::types::AgentObservation {
    let mut minimized = observation.clone();
    minimized.id = metadata_safe_label_or_ref("observation_id", &observation.id, value_origin);
    minimized.action_id = observation
        .action_id
        .as_deref()
        .map(|value| metadata_safe_label_or_ref("action_id", value, value_origin));
    minimized.content =
        if value_origin.is_stored() && is_bound_content_text_ref(&observation.content) {
            observation.content.clone()
        } else {
            metadata_safe_text_receipt("observation_content", &observation.content, value_origin)
        };
    minimized.source = metadata_safe_typed_code_or_receipt(
        "observation_source",
        &observation.source,
        value_origin,
    );
    minimized.structured_result = observation
        .structured_result
        .as_ref()
        .map(|value| metadata_safe_value_receipt("observation_result", value, value_origin));
    minimized.react_trace = observation
        .react_trace
        .as_ref()
        .map(|trace| minimize_react_trace(trace, value_origin));
    minimized
}

fn bound_content_text_ref(receipt_id: &str) -> String {
    let _ = receipt_id;
    "Tool body is not copied into this execution record; authenticated receipt metadata is available on the owning action."
        .to_string()
}

fn is_bound_content_text_ref(value: &str) -> bool {
    value
        == "Tool body is not copied into this execution record; authenticated receipt metadata is available on the owning action."
}

fn bound_content_value_ref(receipt_id: &str) -> serde_json::Value {
    serde_json::json!({
        "kind": "bound_content_receipt_ref",
        "receiptId": receipt_id,
    })
}

fn is_bound_content_value_ref(value: &serde_json::Value) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == 2
            && object.get("kind").and_then(serde_json::Value::as_str)
                == Some("bound_content_receipt_ref")
            && object
                .get("receiptId")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|receipt_id| uuid::Uuid::parse_str(receipt_id).is_ok())
    })
}

fn records_are_equal<T: serde::Serialize>(left: &T, right: &T) -> bool {
    serde_json::to_value(left).ok() == serde_json::to_value(right).ok()
}

fn minimize_action_for_update(
    action: &crate::agent::types::AgentAction,
    stored: Option<&crate::agent::types::AgentAction>,
    key: &AgentRunReceiptKey,
) -> Result<crate::agent::types::AgentAction> {
    let Some(stored) = stored else {
        return Ok(minimize_action(action, ReceiptOrigin::NewInput(key)));
    };
    if records_are_equal(action, stored) {
        return Ok(stored.clone());
    }

    let mut minimized = minimize_action(action, ReceiptOrigin::NewInput(key));
    // Updating typed execution state must not feed an already canonical body
    // receipt back through the NewInput minimizer. Preserve each unchanged
    // field only from the exact same stored action owner; changed fields still
    // cross the untrusted-input boundary independently.
    if action.action_type == stored.action_type {
        minimized.action_type = stored.action_type.clone();
    }
    if action.target == stored.target {
        minimized.target = stored.target.clone();
    }
    if action.input == stored.input {
        minimized.input = stored.input.clone();
    }
    if action.output == stored.output {
        minimized.output = stored.output.clone();
    }
    if action.status == stored.status {
        minimized.status = stored.status.clone();
    }
    if action.permission_decision == stored.permission_decision {
        minimized.permission_decision = stored.permission_decision.clone();
    }
    if action.error == stored.error {
        minimized.error = stored.error.clone();
    }
    if records_are_equal(&action.tool_scope, &stored.tool_scope) {
        minimized.tool_scope = stored.tool_scope.clone();
    }
    minimized.react_trace = match action.react_trace.as_ref() {
        Some(trace) => Some(minimize_composite_for_update(
            trace,
            stored.react_trace.as_ref(),
            |value| minimize_react_trace(value, ReceiptOrigin::NewInput(key)),
        )?),
        None => None,
    };
    Ok(minimized)
}

fn minimize_observation_for_update(
    observation: &crate::agent::types::AgentObservation,
    stored: Option<&crate::agent::types::AgentObservation>,
    key: &AgentRunReceiptKey,
) -> crate::agent::types::AgentObservation {
    let Some(stored) = stored else {
        return minimize_observation(observation, ReceiptOrigin::NewInput(key));
    };
    if records_are_equal(observation, stored) {
        return stored.clone();
    }
    let mut minimized = minimize_observation(observation, ReceiptOrigin::NewInput(key));
    if observation.id == stored.id {
        minimized.id = stored.id.clone();
    }
    if observation.action_id == stored.action_id {
        minimized.action_id = stored.action_id.clone();
    }
    if observation.content == stored.content {
        minimized.content = stored.content.clone();
    }
    if observation.source == stored.source {
        minimized.source = stored.source.clone();
    }
    if observation.structured_result == stored.structured_result {
        minimized.structured_result = stored.structured_result.clone();
    }
    if records_are_equal(&observation.react_trace, &stored.react_trace) {
        minimized.react_trace = stored.react_trace.clone();
    }
    minimized
}

fn observed_bound_content_body<'a>(
    action: &'a crate::agent::types::AgentAction,
    observation: &'a crate::agent::types::AgentObservation,
    field: crate::agent::types::BoundContentField,
) -> Result<&'a str> {
    let body = match field {
        crate::agent::types::BoundContentField::ActionOutputObservationContent => action
            .output
            .as_ref()
            .and_then(|value| value.get("text"))
            .and_then(serde_json::Value::as_str)
            .context("bound_content_receipt_action_output_missing")?,
        crate::agent::types::BoundContentField::ActionErrorObservationContent => action
            .error
            .as_deref()
            .context("bound_content_receipt_action_error_missing")?,
    };
    if observation.content != body {
        anyhow::bail!("bound_content_receipt_observed_body_mismatch");
    }
    Ok(body)
}

#[derive(Debug)]
struct PendingBoundContentAttachment {
    issuance_id: String,
    receipt_id: String,
    action_id: String,
    observation_id: String,
    receipt_json: String,
}

fn canonicalize_execution_records(
    tx: &Transaction<'_>,
    canonical_store_identity: &str,
    now: i64,
    run_id: &str,
    actions: &[crate::agent::types::AgentAction],
    observations: &[crate::agent::types::AgentObservation],
    stored_actions: Option<&[crate::agent::types::AgentAction]>,
    stored_observations: Option<&[crate::agent::types::AgentObservation]>,
    key: &AgentRunReceiptKey,
) -> Result<(
    Vec<crate::agent::types::AgentAction>,
    Vec<crate::agent::types::AgentObservation>,
    Vec<PendingBoundContentAttachment>,
)> {
    validate_raw_execution_identity_graph(run_id, actions, observations)?;
    let mut canonical_actions = actions
        .iter()
        .map(|action| {
            let stored = stored_actions
                .and_then(|stored| stored.iter().find(|candidate| candidate.id == action.id));
            minimize_action_for_update(action, stored, key)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut canonical_observations = observations
        .iter()
        .map(|observation| {
            let stored = stored_observations.and_then(|stored| {
                stored
                    .iter()
                    .find(|candidate| candidate.id == observation.id)
            });
            minimize_observation_for_update(observation, stored, key)
        })
        .collect::<Vec<_>>();
    let mut pending_attachments = Vec::new();

    for (action_index, action) in actions.iter().enumerate() {
        let Some(receipt) = action
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
        else {
            continue;
        };
        let exact_stored_action = stored_actions
            .and_then(|stored| stored.iter().find(|candidate| candidate.id == action.id))
            .is_some_and(|stored| records_are_equal(action, stored));
        if exact_stored_action {
            continue;
        }
        let observation_index = observations
            .iter()
            .position(|observation| observation.id == receipt.observation_id())
            .context("bound_content_receipt_observation_missing")?;
        let observation = &observations[observation_index];
        if observation.action_id.as_deref() != Some(action.id.as_str())
            || observation.react_trace.is_some()
        {
            anyhow::bail!("bound_content_receipt_action_observation_mismatch");
        }

        // A receipt already attached to this exact stored owner is not a new
        // body-bearing issuance. Rebuild its canonical graph from individually
        // minimized fields and reverify the durable HMAC before considering
        // any pending-ledger path. In v2 the lifecycle fields are part of the
        // binding, so changing one must fail explicitly as semantic drift; it
        // must never be misclassified as a missing raw body or resurrected as
        // a fresh pending attachment.
        let stored_owner = stored_actions
            .and_then(|stored| stored.iter().find(|candidate| candidate.id == action.id))
            .and_then(|stored_action| {
                let stored_receipt = stored_action
                    .react_trace
                    .as_ref()
                    .and_then(|trace| trace.output_receipt.as_ref())?;
                records_are_equal(receipt, stored_receipt)
                    .then_some((stored_action, stored_receipt))
            })
            .and_then(|(stored_action, stored_receipt)| {
                stored_observations
                    .and_then(|stored| {
                        stored
                            .iter()
                            .find(|candidate| candidate.id == observation.id)
                    })
                    .map(|stored_observation| (stored_action, stored_observation, stored_receipt))
            });
        if let Some((stored_action, stored_observation, stored_receipt)) = stored_owner {
            let bound_body_ref_unchanged = match receipt.field() {
                crate::agent::types::BoundContentField::ActionOutputObservationContent => {
                    action.output == stored_action.output
                }
                crate::agent::types::BoundContentField::ActionErrorObservationContent => {
                    action.error == stored_action.error
                }
            } && observation.content == stored_observation.content;
            if !bound_body_ref_unchanged {
                anyhow::bail!("bound_content_receipt_attached_body_ref_drift");
            }
            match receipt.field() {
                crate::agent::types::BoundContentField::ActionOutputObservationContent => {
                    canonical_actions[action_index].output = stored_action.output.clone();
                }
                crate::agent::types::BoundContentField::ActionErrorObservationContent => {
                    canonical_actions[action_index].error = stored_action.error.clone();
                }
            }
            canonical_actions[action_index]
                .react_trace
                .as_mut()
                .context("bound_content_receipt_canonical_action_trace_missing")?
                .output_receipt = Some(stored_receipt.clone());
            canonical_observations[observation_index].content = stored_observation.content.clone();
            canonical_observations[observation_index].react_trace = None;
            let canonical_binding =
                crate::agent::types::ContentReceiptBinding::from_canonical_action_graph(
                    canonical_store_identity,
                    run_id,
                    &canonical_actions[action_index],
                    &canonical_observations[observation_index],
                    receipt.field(),
                )?;
            if !receipt.verify_durable(key, &canonical_binding) {
                anyhow::bail!("bound_content_receipt_attached_semantic_drift");
            }
            continue;
        }
        let body = observed_bound_content_body(action, observation, receipt.field())?;
        let observed_binding = crate::agent::types::ContentReceiptBinding::from_action_graph(
            run_id,
            action,
            observation,
            receipt.field(),
        )?;
        if !receipt.verify_observed_body(key, &observed_binding, body)
            || receipt.action_id() != canonical_actions[action_index].id
            || receipt.observation_id() != canonical_observations[observation_index].id
        {
            anyhow::bail!("bound_content_receipt_canonical_identity_mismatch");
        }
        let reference = bound_content_text_ref(receipt.receipt_id());
        match receipt.field() {
            crate::agent::types::BoundContentField::ActionOutputObservationContent => {
                canonical_actions[action_index].output =
                    Some(bound_content_value_ref(receipt.receipt_id()));
            }
            crate::agent::types::BoundContentField::ActionErrorObservationContent => {
                canonical_actions[action_index].error = Some(reference.clone());
            }
        }
        canonical_observations[observation_index].content = reference;
        canonical_actions[action_index]
            .react_trace
            .as_mut()
            .context("bound_content_receipt_canonical_action_trace_missing")?
            .output_receipt = Some(receipt.clone());
        canonical_observations[observation_index].react_trace = None;
        let canonical_binding =
            crate::agent::types::ContentReceiptBinding::from_canonical_action_graph(
                canonical_store_identity,
                run_id,
                &canonical_actions[action_index],
                &canonical_observations[observation_index],
                receipt.field(),
            )?;
        if !receipt.verify_durable(key, &canonical_binding) {
            anyhow::bail!("bound_content_receipt_canonical_binding_invalid");
        }
        let is_attached_replay = stored_actions
            .and_then(|stored| stored.iter().find(|candidate| candidate.id == action.id))
            .is_some_and(|stored_action| {
                records_are_equal(&canonical_actions[action_index], stored_action)
                    && stored_observations
                        .and_then(|stored| {
                            stored
                                .iter()
                                .find(|candidate| candidate.id == observation.id)
                        })
                        .is_some_and(|stored_observation| {
                            records_are_equal(
                                &canonical_observations[observation_index],
                                stored_observation,
                            )
                        })
            });
        if is_attached_replay {
            continue;
        }

        let receipt_json = serde_json::to_string(receipt)
            .context("bound_content_receipt_ledger_serialization_failed")?;
        let pending_exists = tx
            .query_row(
                "SELECT 1
                 FROM bound_content_issuance_ledger
                 WHERE issuance_id = ?1
                   AND receipt_id = ?2
                   AND canonical_store_identity = ?3
                   AND run_id = ?4
                   AND action_id = ?5
                   AND observation_id = ?6
                   AND receipt_json = ?7
                   AND state = 'pending'
                   AND expires_at >= ?8",
                params![
                    receipt.issuance_id(),
                    receipt.receipt_id(),
                    canonical_store_identity,
                    run_id,
                    action.id,
                    observation.id,
                    receipt_json,
                    now,
                ],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !pending_exists {
            anyhow::bail!("bound_content_receipt_pending_issuance_missing_or_expired");
        }
        pending_attachments.push(PendingBoundContentAttachment {
            issuance_id: receipt.issuance_id().to_string(),
            receipt_id: receipt.receipt_id().to_string(),
            action_id: action.id.clone(),
            observation_id: observation.id.clone(),
            receipt_json,
        });
    }

    if observations.iter().any(|observation| {
        observation
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .is_some()
            && !actions.iter().any(|action| {
                action
                    .react_trace
                    .as_ref()
                    .and_then(|trace| trace.output_receipt.as_ref())
                    .is_some_and(|receipt| receipt.observation_id() == observation.id)
            })
    }) {
        anyhow::bail!("bound_content_receipt_action_reference_missing");
    }
    validate_persisted_execution_identity_graph(
        canonical_store_identity,
        run_id,
        &canonical_actions,
        &canonical_observations,
        key,
    )?;
    Ok((
        canonical_actions,
        canonical_observations,
        pending_attachments,
    ))
}

fn validate_raw_execution_identity_graph(
    run_id: &str,
    actions: &[crate::agent::types::AgentAction],
    observations: &[crate::agent::types::AgentObservation],
) -> Result<()> {
    let mut action_ids = std::collections::HashSet::new();
    for action in actions {
        if action.id.trim().is_empty() || !action_ids.insert(action.id.as_str()) {
            anyhow::bail!("agent_run_action_identity_not_unique:{run_id}");
        }
        if action.react_trace.as_ref().is_some_and(|trace| {
            trace.output_receipt.is_some()
                && (trace.run_id.as_deref() != Some(run_id)
                    || trace.action_id != action.id
                    || trace
                        .observation_id
                        .as_deref()
                        .is_none_or(|observation_id| {
                            !observations
                                .iter()
                                .any(|observation| observation.id == observation_id)
                        }))
        }) {
            anyhow::bail!("agent_run_action_trace_identity_mismatch:{run_id}");
        }
    }
    let mut observation_ids = std::collections::HashSet::new();
    for observation in observations {
        if observation.id.trim().is_empty() || !observation_ids.insert(observation.id.as_str()) {
            anyhow::bail!("agent_run_observation_identity_not_unique:{run_id}");
        }
        if observation
            .action_id
            .as_deref()
            .is_some_and(|action_id| !action_ids.contains(action_id))
        {
            anyhow::bail!("agent_run_observation_action_foreign_key_missing:{run_id}");
        }
        if observation.react_trace.as_ref().is_some_and(|trace| {
            trace.output_receipt.is_some()
                && (trace.run_id.as_deref() != Some(run_id)
                    || observation.action_id.as_deref() != Some(trace.action_id.as_str())
                    || trace.observation_id.as_deref() != Some(observation.id.as_str()))
        }) {
            anyhow::bail!("agent_run_observation_trace_identity_mismatch:{run_id}");
        }
    }
    Ok(())
}

fn validate_persisted_execution_identity_graph(
    canonical_store_identity: &str,
    run_id: &str,
    actions: &[crate::agent::types::AgentAction],
    observations: &[crate::agent::types::AgentObservation],
    key: &AgentRunReceiptKey,
) -> Result<()> {
    validate_raw_execution_identity_graph(run_id, actions, observations)?;
    let observations_by_id = observations
        .iter()
        .map(|observation| (observation.id.as_str(), observation))
        .collect::<std::collections::HashMap<_, _>>();
    let mut referenced_receipts = std::collections::HashSet::new();
    let mut referenced_observations = std::collections::HashSet::new();
    for action in actions {
        let Some(receipt) = action
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
        else {
            if action
                .output
                .as_ref()
                .is_some_and(is_bound_content_value_ref)
                || action
                    .error
                    .as_deref()
                    .is_some_and(is_bound_content_text_ref)
            {
                anyhow::bail!("bound_content_receipt_metadata_missing:{run_id}");
            }
            continue;
        };
        let observation = observations_by_id
            .get(receipt.observation_id())
            .context("bound_content_receipt_observation_missing")?;
        let canonical_binding =
            crate::agent::types::ContentReceiptBinding::from_canonical_action_graph(
                canonical_store_identity,
                run_id,
                action,
                observation,
                receipt.field(),
            )?;
        if !receipt.verify_durable(key, &canonical_binding)
            || receipt.run_id() != run_id
            || receipt.action_id() != action.id
            || observation.action_id.as_deref() != Some(action.id.as_str())
            || observation.react_trace.is_some()
            || observation.content != bound_content_text_ref(receipt.receipt_id())
            || !referenced_receipts.insert(receipt.receipt_id())
            || !referenced_observations.insert(observation.id.as_str())
        {
            anyhow::bail!("bound_content_receipt_owner_graph_invalid:{run_id}");
        }
        match receipt.field() {
            crate::agent::types::BoundContentField::ActionOutputObservationContent => {
                if action.output.as_ref() != Some(&bound_content_value_ref(receipt.receipt_id())) {
                    anyhow::bail!("bound_content_receipt_action_output_ref_invalid:{run_id}");
                }
            }
            crate::agent::types::BoundContentField::ActionErrorObservationContent => {
                if action.error.as_deref()
                    != Some(bound_content_text_ref(receipt.receipt_id()).as_str())
                {
                    anyhow::bail!("bound_content_receipt_action_error_ref_invalid:{run_id}");
                }
            }
        }
    }
    for observation in observations {
        if observation
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .is_some()
        {
            anyhow::bail!("bound_content_receipt_duplicate_observation_authority:{run_id}");
        }
        if is_bound_content_text_ref(&observation.content)
            && !referenced_observations.contains(observation.id.as_str())
        {
            anyhow::bail!("bound_content_receipt_observation_metadata_missing:{run_id}");
        }
    }
    Ok(())
}

fn minimize_error(
    error: &crate::agent::types::AgentRunError,
    origin: ReceiptOrigin<'_>,
) -> crate::agent::types::AgentRunError {
    crate::agent::types::AgentRunError {
        message: metadata_safe_text_receipt("run_error", &error.message, origin),
        phase: metadata_safe_enum_or_receipt(
            "error_phase",
            &error.phase,
            &[
                "preprocess",
                "model",
                "stream",
                "reasoning",
                "tool",
                "finalize",
                "execution",
                "parse",
                "meaning",
                "safety_check",
                "generation",
                "review_staging",
                "timeout",
                "cancelled",
                "interrupted",
                "provider_error",
                "tool_error",
                "policy_blocker",
                "unknown_error",
                "startup_projection_recovery",
                "unknown",
            ],
            origin,
        ),
        recoverable: error.recoverable,
    }
}

fn minimize_status_update(
    update: &crate::agent::types::AgentLoopStatusUpdate,
    origin: ReceiptOrigin<'_>,
) -> crate::agent::types::AgentLoopStatusUpdate {
    let mut minimized = update.clone();
    minimized.message = metadata_safe_text_receipt("status_message", &update.message, origin);
    minimized
}

fn metadata_safe_reason_code_or_receipt(
    kind: &str,
    value: &str,
    origin: ReceiptOrigin<'_>,
) -> String {
    if origin.is_stored() && is_metadata_safe_text_receipt(kind, value) {
        return value.to_string();
    }
    if is_registered_reason_code(value) {
        value.to_string()
    } else {
        metadata_safe_text_receipt(kind, value, origin)
    }
}

fn is_registered_reason_code(value: &str) -> bool {
    matches!(
        value,
        "explicit_user_request"
            | "policy_allowed"
            | "policy_blocked"
            | "provider_selected"
            | "local_preferred"
            | "local_unavailable"
            | "cloud_required"
            | "privacy_route"
            | "main_chat_kernel_direct_reflex"
            | "kernel_supported_plan_execute"
            | "task_domain_and_trigger_match"
            | "tool_requirement_write"
            | "unknown"
    )
}

fn metadata_safe_provider_or_receipt(value: &str, origin: ReceiptOrigin<'_>) -> String {
    if matches!(
        value,
        "ollama"
            | "openai"
            | "anthropic"
            | "deepseek"
            | "openrouter"
            | "google"
            | "qwen"
            | "zhipu"
            | "moonshot"
            | "minimax"
            | "custom"
            | "direct"
            | "none"
    ) {
        value.to_string()
    } else {
        metadata_safe_text_receipt("provider", value, origin)
    }
}

fn metadata_safe_route_type_or_receipt(value: &str, origin: ReceiptOrigin<'_>) -> String {
    metadata_safe_enum_or_receipt(
        "route_type",
        value,
        &["local", "cloud", "direct", "hybrid", "none", "unknown"],
        origin,
    )
}

fn minimize_model_route(
    route: &crate::agent::types::ModelRouteTrace,
    origin: ReceiptOrigin<'_>,
) -> crate::agent::types::ModelRouteTrace {
    let mut minimized = route.clone();
    minimized.provider = metadata_safe_provider_or_receipt(&route.provider, origin);
    // Model identifiers are provider-configured and may contain user secrets;
    // this store has no registry proof for them, so it keeps only a receipt.
    minimized.model = metadata_safe_text_receipt("model", &route.model, origin);
    minimized.route_type = metadata_safe_route_type_or_receipt(&route.route_type, origin);
    minimized.local_model = metadata_safe_text_receipt("local_model", &route.local_model, origin);
    minimized.reason = metadata_safe_reason_code_or_receipt("route_reason", &route.reason, origin);
    minimized.fallback_reason = route
        .fallback_reason
        .as_deref()
        .map(|value| metadata_safe_reason_code_or_receipt("fallback_reason", value, origin));
    minimized
}

fn minimize_context_summary(
    summary: &crate::agent::types::ContextSummary,
    origin: ReceiptOrigin<'_>,
) -> crate::agent::types::ContextSummary {
    let mut minimized = summary.clone();
    minimized.included_life_model_sections = summary
        .included_life_model_sections
        .iter()
        .map(|value| metadata_safe_label_or_ref("life_model_section", value, origin))
        .collect();
    minimized.memory_sources = summary
        .memory_sources
        .iter()
        .map(|value| metadata_safe_label_or_ref("memory_source_ref", value, origin))
        .collect();
    minimized
}

fn minimize_hs_selection_audit(
    audit: &crate::agent::hs_selector::HSSelectionAudit,
    origin: ReceiptOrigin<'_>,
) -> crate::agent::hs_selector::HSSelectionAudit {
    let mut minimized = audit.clone();
    minimized.agent_task_id = audit
        .agent_task_id
        .as_deref()
        .map(|value| metadata_safe_label_or_ref("agent_task_id", value, origin));
    minimized.agent_run_id = audit
        .agent_run_id
        .as_deref()
        .map(|value| metadata_safe_label_or_ref("agent_run_id", value, origin));
    minimized.input_digest =
        metadata_safe_label_or_ref("hs_input_digest", &audit.input_digest, origin);
    minimized.selected_policy_ids = audit
        .selected_policy_ids
        .iter()
        .map(|value| metadata_safe_label_or_ref("policy_id", value, origin))
        .collect();
    minimized.selected_heuristic_ids = audit
        .selected_heuristic_ids
        .iter()
        .map(|value| metadata_safe_label_or_ref("heuristic_id", value, origin))
        .collect();
    minimized.selected_guidance_ids = audit
        .selected_guidance_ids
        .iter()
        .map(|value| metadata_safe_label_or_ref("guidance_id", value, origin))
        .collect();
    for guidance in &mut minimized.selected_guidance_refs {
        guidance.guidance_id =
            metadata_safe_label_or_ref("guidance_id", &guidance.guidance_id, origin);
        guidance.guidance_digest =
            metadata_safe_label_or_ref("guidance_digest", &guidance.guidance_digest, origin);
        guidance.guidance_type =
            metadata_safe_label_or_ref("guidance_type", &guidance.guidance_type, origin);
        guidance.domain = metadata_safe_label_or_ref("guidance_domain", &guidance.domain, origin);
        guidance.trigger_digest =
            metadata_safe_label_or_ref("trigger_digest", &guidance.trigger_digest, origin);
        guidance.selected_reason = metadata_safe_reason_code_or_receipt(
            "selected_reason",
            &guidance.selected_reason,
            origin,
        );
        guidance.impact_kind =
            metadata_safe_typed_code_or_receipt("impact_kind", &guidance.impact_kind, origin);
        guidance.impact_summary =
            metadata_safe_bounded_summary("impact_summary", &guidance.impact_summary, 160, origin);
        guidance.source_proposal_id = guidance
            .source_proposal_id
            .as_deref()
            .map(|value| metadata_safe_label_or_ref("source_proposal_id", value, origin));
        guidance.source_lineage_digest = metadata_safe_label_or_ref(
            "source_lineage_digest",
            &guidance.source_lineage_digest,
            origin,
        );
        guidance.policy_boundary.constraint_digest = metadata_safe_label_or_ref(
            "constraint_digest",
            &guidance.policy_boundary.constraint_digest,
            origin,
        );
    }
    for exclusion in &mut minimized.excluded_assets {
        exclusion.asset_id = metadata_safe_label_or_ref("hs_asset_id", &exclusion.asset_id, origin);
    }
    minimized
}

fn minimize_behavior_check(
    check: &crate::agent::types::HSBehaviorCheckSummary,
    origin: ReceiptOrigin<'_>,
) -> crate::agent::types::HSBehaviorCheckSummary {
    let mut minimized = check.clone();
    minimized.id = metadata_safe_label_or_ref("behavior_check_id", &check.id, origin);
    minimized.label =
        metadata_safe_bounded_summary("behavior_check_label", &check.label, 120, origin);
    minimized.summary = check
        .summary
        .as_deref()
        .map(|value| metadata_safe_bounded_summary("behavior_check_summary", value, 200, origin));
    minimized
}

fn minimized_actions_json_with_origin(
    actions: &[crate::agent::types::AgentAction],
    value_origin: ReceiptOrigin<'_>,
) -> Result<String> {
    serde_json::to_string(
        &actions
            .iter()
            .map(|action| minimize_action(action, value_origin))
            .collect::<Vec<_>>(),
    )
    .context("failed to serialize minimized AgentRun actions")
}

fn minimized_observations_json_with_origin(
    observations: &[crate::agent::types::AgentObservation],
    value_origin: ReceiptOrigin<'_>,
) -> Result<String> {
    serde_json::to_string(
        &observations
            .iter()
            .map(|observation| minimize_observation(observation, value_origin))
            .collect::<Vec<_>>(),
    )
    .context("failed to serialize minimized AgentRun observations")
}

/// Preserve only leaves that came unchanged from the row that was just
/// decoded and canonically validated.  A sibling/status change must not turn
/// an existing receipt back into caller input, while every changed leaf still
/// passes through fresh minimization.
fn preserve_unchanged_json_leaves(
    incoming: &serde_json::Value,
    stored: &serde_json::Value,
    minimized: &mut serde_json::Value,
) {
    if incoming == stored {
        *minimized = stored.clone();
        return;
    }
    match (incoming, stored, minimized) {
        (
            serde_json::Value::Object(incoming),
            serde_json::Value::Object(stored),
            serde_json::Value::Object(minimized),
        ) => {
            for (key, incoming_value) in incoming {
                if let (Some(stored_value), Some(minimized_value)) =
                    (stored.get(key), minimized.get_mut(key))
                {
                    preserve_unchanged_json_leaves(incoming_value, stored_value, minimized_value);
                }
            }
        }
        (
            serde_json::Value::Array(incoming),
            serde_json::Value::Array(stored),
            serde_json::Value::Array(minimized),
        ) => {
            for (index, incoming_value) in incoming.iter().enumerate() {
                if let (Some(stored_value), Some(minimized_value)) =
                    (stored.get(index), minimized.get_mut(index))
                {
                    preserve_unchanged_json_leaves(incoming_value, stored_value, minimized_value);
                }
            }
        }
        _ => {}
    }
}

fn minimize_composite_for_update<T, F>(
    incoming: &T,
    stored: Option<&T>,
    minimize_new: F,
) -> Result<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
    F: FnOnce(&T) -> T,
{
    let minimized = minimize_new(incoming);
    let Some(stored) = stored else {
        return Ok(minimized);
    };
    let incoming_json = serde_json::to_value(incoming)?;
    let stored_json = serde_json::to_value(stored)?;
    let mut minimized_json = serde_json::to_value(minimized)?;
    preserve_unchanged_json_leaves(&incoming_json, &stored_json, &mut minimized_json);
    serde_json::from_value(minimized_json)
        .context("failed to decode field-provenance minimized AgentRun value")
}

fn minimized_status_updates_json(
    updates: &[crate::agent::types::AgentLoopStatusUpdate],
    key: &AgentRunReceiptKey,
) -> Result<String> {
    minimized_status_updates_json_with_origin(updates, ReceiptOrigin::NewInput(key))
}

fn minimized_status_updates_json_with_origin(
    updates: &[crate::agent::types::AgentLoopStatusUpdate],
    origin: ReceiptOrigin<'_>,
) -> Result<String> {
    serde_json::to_string(
        &updates
            .iter()
            .map(|update| minimize_status_update(update, origin))
            .collect::<Vec<_>>(),
    )
    .context("failed to serialize minimized AgentRun status updates")
}

fn minimized_behavior_checks_json(
    checks: &[crate::agent::types::HSBehaviorCheckSummary],
    key: &AgentRunReceiptKey,
) -> Result<String> {
    minimized_behavior_checks_json_with_origin(checks, ReceiptOrigin::NewInput(key))
}

fn minimized_behavior_checks_json_with_origin(
    checks: &[crate::agent::types::HSBehaviorCheckSummary],
    origin: ReceiptOrigin<'_>,
) -> Result<String> {
    serde_json::to_string(
        &checks
            .iter()
            .map(|check| minimize_behavior_check(check, origin))
            .collect::<Vec<_>>(),
    )
    .context("failed to serialize minimized AgentRun behavior checks")
}

fn minimized_behavior_checks_json_for_update(
    incoming: &[crate::agent::types::HSBehaviorCheckSummary],
    stored: &[crate::agent::types::HSBehaviorCheckSummary],
    key: &AgentRunReceiptKey,
) -> Result<String> {
    let minimized = incoming
        .iter()
        .map(|check| {
            minimize_composite_for_update(
                check,
                stored.iter().find(|candidate| candidate.id == check.id),
                |value| minimize_behavior_check(value, ReceiptOrigin::NewInput(key)),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    serde_json::to_string(&minimized)
        .context("failed to serialize field-provenance AgentRun behavior checks")
}

fn minimized_status_updates_json_for_update(
    incoming: &[crate::agent::types::AgentLoopStatusUpdate],
    stored: &[crate::agent::types::AgentLoopStatusUpdate],
    key: &AgentRunReceiptKey,
) -> Result<String> {
    let minimized = incoming
        .iter()
        .enumerate()
        .map(|(index, update)| {
            minimize_composite_for_update(update, stored.get(index), |value| {
                minimize_status_update(value, ReceiptOrigin::NewInput(key))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    serde_json::to_string(&minimized)
        .context("failed to serialize field-provenance AgentRun status updates")
}

fn parse_optional_legacy_json<T>(run_id: &str, column: &str, raw: Option<&str>) -> Result<Option<T>>
where
    T: serde::de::DeserializeOwned,
{
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(raw)
        .with_context(|| format!("invalid non-empty {column} for legacy AgentRun {run_id}"))
        .map(Some)
}

fn parse_legacy_json_array<T>(run_id: &str, column: &str, raw: Option<&str>) -> Result<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    parse_optional_legacy_json(run_id, column, raw).map(|value| value.unwrap_or_default())
}

fn parse_legacy_trace_array<T>(run_id: &str, column: &str, raw: Option<&str>) -> Result<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    let Some(raw) = raw.filter(|raw| !raw.trim().is_empty()) else {
        return Ok(Vec::new());
    };
    let mut value = serde_json::from_str::<serde_json::Value>(raw)
        .with_context(|| format!("invalid non-empty {column} for legacy AgentRun {run_id}"))?;
    fn remove_untrusted_legacy_output_receipt(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                // No pre-v5 row carried an in-process observed-body proof.
                // Drop both split legacy claims and any receipt-shaped value
                // instead of promoting a schema marker to provenance.
                object.remove("outputHash");
                object.remove("outputByteCount");
                object.remove("outputReceipt");
                for child in object.values_mut() {
                    remove_untrusted_legacy_output_receipt(child);
                }
            }
            serde_json::Value::Array(values) => {
                for child in values {
                    remove_untrusted_legacy_output_receipt(child);
                }
            }
            _ => {}
        }
    }
    remove_untrusted_legacy_output_receipt(&mut value);
    serde_json::from_value(value)
        .with_context(|| format!("invalid typed {column} for legacy AgentRun {run_id}"))
}

fn normalized_metadata_reference(
    run_id: &str,
    column: &str,
    value: Option<&str>,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > MAX_AGENT_RUN_REF_CHARS
        || value.trim() != value
        || value.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || !(character.is_ascii_alphanumeric()
                    || matches!(character, ':' | '/' | '.' | '_' | '-' | '@' | '+'))
        })
    {
        anyhow::bail!("invalid {column} reference for AgentRun {run_id}");
    }
    Ok(Some(value.to_string()))
}

fn normalized_identity_reference(owner: &str, column: &str, value: &str) -> Result<String> {
    if value.is_empty()
        || value.chars().count() > MAX_AGENT_RUN_ID_CHARS
        || value.trim() != value
        || value.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || !(character.is_ascii_alphanumeric()
                    || matches!(character, ':' | '/' | '.' | '_' | '-' | '@' | '+'))
        })
    {
        anyhow::bail!("invalid {column} identity for {owner}");
    }
    Ok(value.to_string())
}

fn validate_new_agent_run_identity(run: &AgentRun) -> Result<()> {
    #[cfg(any(test, feature = "test-utils"))]
    {
        let _ = run;
        return Ok(());
    }
    #[cfg(not(any(test, feature = "test-utils")))]
    {
        for (field, value) in [
            ("run_id", run.id.as_str()),
            ("task_id", run.task_id.as_str()),
        ] {
            uuid::Uuid::parse_str(value)
                .with_context(|| format!("agent_run_new_{field}_must_be_uuid"))?;
        }
        Ok(())
    }
}

fn is_canonical_conversation_message_ref(value: &str) -> bool {
    canonical_conversation_message_parts(value).is_some()
}

fn canonical_conversation_message_parts(value: &str) -> Option<(&str, i64)> {
    let Some(rest) = value.strip_prefix("conversation://") else {
        return None;
    };
    let Some((session_id, message_id)) = rest.split_once("/message/") else {
        return None;
    };
    let parsed_message_id = message_id.parse::<i64>().ok()?;
    (!session_id.is_empty()
        && parsed_message_id > 0
        && parsed_message_id.to_string() == message_id
        && normalized_identity_reference("conversation", "session_id", session_id).is_ok())
    .then_some((session_id, parsed_message_id))
}

fn canonical_conversation_message_id(value: &str) -> Option<i64> {
    canonical_conversation_message_parts(value).map(|(_, message_id)| message_id)
}

fn canonical_conversation_session_from_ref(value: &str) -> Option<&str> {
    canonical_conversation_message_parts(value).map(|(session_id, _)| session_id)
}

fn is_explicit_legacy_unresolvable_ref(value: &str) -> bool {
    value
        .strip_prefix("legacy-unresolvable://agent-run/")
        .is_some_and(is_exact_metadata_digest)
}

fn normalized_input_reference(run_id: &str, value: Option<&str>) -> Result<Option<String>> {
    let reference = normalized_metadata_reference(run_id, "input_ref", value)?;
    let Some(reference) = reference else {
        return Ok(None);
    };
    if !is_canonical_conversation_message_ref(&reference)
        && !is_explicit_legacy_unresolvable_ref(&reference)
    {
        anyhow::bail!("invalid or unresolvable input_ref for AgentRun {run_id}");
    }
    Ok(Some(reference))
}

fn input_reference_matches_session(input_ref: Option<&str>, session_id: Option<&str>) -> bool {
    match input_ref.and_then(canonical_conversation_session_from_ref) {
        Some(canonical_session) => Some(canonical_session) == session_id,
        None => true,
    }
}

fn normalized_metadata_digest(
    run_id: &str,
    column: &str,
    value: Option<&str>,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    if !is_exact_metadata_digest(value) {
        anyhow::bail!("invalid {column} digest for AgentRun {run_id}");
    }
    Ok(Some(value.to_string()))
}

fn is_agent_run_status(value: &str) -> bool {
    matches!(
        value,
        "running" | "waiting_permission" | "completed" | "failed" | "cancelled"
    )
}

fn is_agent_task_kind(value: &str) -> bool {
    matches!(
        value,
        "conversation"
            | "builder"
            | "calibration"
            | "evolution"
            | "tool_execution"
            | "proactive"
            | "planning"
            | "review"
            | "writing"
            | "memory_governance"
            | "skill"
            | "plugin"
    )
}

fn derived_legacy_input_ref(input_digest: &str) -> String {
    // A content digest is evidence, not a canonical address. Legacy rows did
    // not record the owning message id, so make that irrecoverability explicit
    // instead of manufacturing a resolvable-looking conversation URI.
    format!("legacy-unresolvable://agent-run/{input_digest}")
}

struct LegacyAgentRunPayload {
    run_id: String,
    task_id: String,
    status: String,
    kind: String,
    session_id: Option<String>,
    user_input: Option<String>,
    input_ref: Option<String>,
    _input_digest: Option<String>,
    context_summary_json: Option<String>,
    model_route_json: Option<String>,
    output_preview: Option<String>,
    error_json: Option<String>,
    generated_proposals_json: Option<String>,
    actions_json: Option<String>,
    observations_json: Option<String>,
    reasoning_strategy: Option<String>,
    reasoning_trace_json: Option<String>,
    _reasoning_trace_digest: Option<String>,
    hs_selection_audit_json: Option<String>,
    behavior_checks_json: Option<String>,
    status_updates_json: Option<String>,
    delete_reason: Option<String>,
    payload_minimized_version: i64,
}

fn scoped_run_content_digest(
    key: &AgentRunReceiptKey,
    run_id: &str,
    purpose: &str,
    value: &str,
) -> String {
    metadata_digest(
        key,
        purpose,
        &format!(
            "run_id\0{}:{}\0body\0{}:{}",
            run_id.len(),
            run_id,
            value.len(),
            value
        ),
    )
}

fn normalized_run_input(
    run: &AgentRun,
    key: &AgentRunReceiptKey,
    allow_unchanged_persisted_digest: bool,
) -> Result<(Option<String>, Option<String>)> {
    let input_ref = normalized_input_reference(&run.id, run.input_ref.as_deref())?;
    if let Some(canonical_session) = input_ref
        .as_deref()
        .and_then(canonical_conversation_session_from_ref)
    {
        if run.session_id.as_deref() != Some(canonical_session) {
            anyhow::bail!(
                "AgentRun {} canonical input_ref belongs to a different conversation",
                run.id
            );
        }
    }
    let input_digest =
        normalized_metadata_digest(&run.id, "input_digest", run.input_digest.as_deref())?;
    let Some(user_input) = run.user_input.as_deref() else {
        if input_digest.is_some() && !allow_unchanged_persisted_digest {
            anyhow::bail!(
                "AgentRun {} input_digest requires transient input or a canonical proof",
                run.id
            );
        }
        return Ok((input_ref, input_digest));
    };
    let computed_digest = scoped_run_content_digest(key, &run.id, "run_input", user_input);
    if input_digest
        .as_deref()
        .is_some_and(|digest| digest != computed_digest.as_str())
    {
        anyhow::bail!(
            "AgentRun {} input_digest does not match transient input",
            run.id
        );
    }
    Ok((input_ref, Some(computed_digest)))
}

fn normalized_reasoning_trace_digest(
    run: &AgentRun,
    key: &AgentRunReceiptKey,
    allow_unchanged_persisted_digest: bool,
) -> Result<Option<String>> {
    let existing = normalized_metadata_digest(
        &run.id,
        "reasoning_trace_digest",
        run.reasoning_trace_digest.as_deref(),
    )?;
    let Some(trace) = run.reasoning_trace.as_ref() else {
        if existing.is_some() && !allow_unchanged_persisted_digest {
            anyhow::bail!(
                "AgentRun {} reasoning_trace_digest requires transient trace",
                run.id
            );
        }
        return Ok(existing);
    };
    let serialized = serde_json::to_string(trace)
        .context("failed to serialize transient AgentRun reasoning trace for digest")?;
    let computed = scoped_run_content_digest(key, &run.id, "reasoning_trace", &serialized);
    if existing
        .as_deref()
        .is_some_and(|digest| digest != computed.as_str())
    {
        anyhow::bail!(
            "AgentRun {} reasoning_trace_digest does not match transient trace",
            run.id
        );
    }
    Ok(Some(computed))
}

fn ensure_agent_run_collection_bounds(run: &AgentRun) -> Result<()> {
    for (name, count) in [
        ("actions", run.actions.len()),
        ("observations", run.observations.len()),
        ("behavior_checks", run.behavior_checks.len()),
        ("status_updates", run.status_updates.len()),
        ("warnings", run.warnings.len()),
    ] {
        if count > MAX_AGENT_RUN_COLLECTION_ITEMS {
            anyhow::bail!(
                "agent_run_collection_limit_exceeded:{}:{}>{}",
                name,
                count,
                MAX_AGENT_RUN_COLLECTION_ITEMS
            );
        }
    }
    if let Some(summary) = run.context_summary.as_ref() {
        for (name, count) in [
            (
                "context_life_model_sections",
                summary.included_life_model_sections.len(),
            ),
            ("context_memory_sources", summary.memory_sources.len()),
        ] {
            if count > MAX_AGENT_RUN_NESTED_REFS {
                anyhow::bail!("agent_run_nested_reference_limit_exceeded:{name}");
            }
        }
    }
    if let Some(audit) = run.hs_selection_audit.as_ref() {
        for (name, count) in [
            ("selected_policy_ids", audit.selected_policy_ids.len()),
            ("selected_heuristic_ids", audit.selected_heuristic_ids.len()),
            ("selected_guidance_ids", audit.selected_guidance_ids.len()),
            ("selected_guidance_refs", audit.selected_guidance_refs.len()),
            ("excluded_assets", audit.excluded_assets.len()),
        ] {
            if count > MAX_AGENT_RUN_NESTED_REFS {
                anyhow::bail!("agent_run_nested_reference_limit_exceeded:{name}");
            }
        }
    }
    if run.actions.iter().any(|action| {
        action
            .tool_scope
            .as_ref()
            .is_some_and(|scope| scope.capabilities.len() > MAX_AGENT_RUN_NESTED_REFS)
    }) {
        anyhow::bail!("agent_run_nested_reference_limit_exceeded:tool_capabilities");
    }
    if run
        .user_input
        .as_deref()
        .is_some_and(|value| value.len() > MAX_AGENT_RUN_RECEIPT_BYTES)
    {
        anyhow::bail!("agent_run_transient_input_limit_exceeded");
    }
    if serde_json::to_vec(run)?.len() > MAX_AGENT_RUN_STORED_PAYLOAD_BYTES {
        anyhow::bail!("agent_run_payload_limit_exceeded");
    }
    Ok(())
}

fn ensure_minimized_payload_bounds<'a>(
    run_id: &str,
    payloads: impl IntoIterator<Item = Option<&'a str>>,
) -> Result<()> {
    let total = payloads
        .into_iter()
        .flatten()
        .try_fold(0usize, |total, payload| total.checked_add(payload.len()))
        .context("agent_run_payload_size_overflow")?;
    if total > MAX_AGENT_RUN_STORED_PAYLOAD_BYTES {
        anyhow::bail!(
            "agent_run_minimized_payload_limit_exceeded:{run_id}:{total}>{MAX_AGENT_RUN_STORED_PAYLOAD_BYTES}"
        );
    }
    Ok(())
}

fn agent_run_row_fault(column_index: usize, field: &str, reason: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column_index,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("agent_run_corrupt_row:{field}:{reason}"),
        )),
    )
}

fn decode_optional_minimized_json<T, F>(
    raw: Option<String>,
    column_index: usize,
    field: &str,
    minimize: F,
) -> rusqlite::Result<Option<T>>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
    F: FnOnce(&T) -> T,
{
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.trim().is_empty() {
        return Err(agent_run_row_fault(column_index, field, "empty_json"));
    }
    let decoded = serde_json::from_str::<T>(&raw)
        .map_err(|_| agent_run_row_fault(column_index, field, "invalid_json"))?;
    let canonical = minimize(&decoded);
    let decoded_value = serde_json::to_value(&decoded)
        .map_err(|_| agent_run_row_fault(column_index, field, "serialization_failed"))?;
    let canonical_value = serde_json::to_value(&canonical)
        .map_err(|_| agent_run_row_fault(column_index, field, "serialization_failed"))?;
    if decoded_value != canonical_value {
        return Err(agent_run_row_fault(
            column_index,
            field,
            "noncanonical_sensitive_payload",
        ));
    }
    Ok(Some(decoded))
}

fn decode_required_minimized_json<T, F>(
    raw: Option<String>,
    column_index: usize,
    field: &str,
    minimize: F,
) -> rusqlite::Result<T>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
    F: FnOnce(&T) -> T,
{
    decode_optional_minimized_json(raw, column_index, field, minimize)?
        .ok_or_else(|| agent_run_row_fault(column_index, field, "missing_json"))
}

fn decode_optional_timestamp(
    raw: Option<String>,
    column_index: usize,
    field: &str,
) -> rusqlite::Result<Option<chrono::DateTime<chrono::Utc>>> {
    raw.map(|value| {
        chrono::DateTime::parse_from_rfc3339(&value)
            .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
            .map_err(|_| agent_run_row_fault(column_index, field, "invalid_timestamp"))
    })
    .transpose()
}

#[derive(Clone)]
pub struct AgentRunStore {
    conn: Arc<crate::sqlite_migration::IdentityBoundSqliteConnection>,
    receipt_key: Arc<AgentRunReceiptKey>,
}

struct RawAgentRunToolExecutionRecord {
    run_id: String,
    receipt_id: String,
    manifest_id: String,
    request_digest: String,
    endpoint_digest: String,
    action_effect: String,
    idempotency_contract: String,
    dispatch_kind: String,
    state: String,
    revision: i64,
    dispatch_attempt_count: i64,
    transport_status: String,
    effect_status: String,
    execution_outcome: String,
    prepared_at: String,
    dispatch_attempted_at: Option<String>,
    response_observed_at: Option<String>,
    terminal_at: Option<String>,
}

fn exact_sha256_metadata_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn parse_agent_run_tool_execution_timestamp(
    value: &str,
    field: &str,
) -> Result<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .with_context(|| format!("agent_run_tool_execution_{field}_invalid"))
}

fn parse_optional_agent_run_tool_execution_timestamp(
    value: Option<String>,
    field: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    value
        .as_deref()
        .map(|value| parse_agent_run_tool_execution_timestamp(value, field))
        .transpose()
}

impl RawAgentRunToolExecutionRecord {
    fn into_typed(self) -> Result<crate::agent::AgentRunToolExecutionRecord> {
        use crate::tool_execution_receipt::{
            ToolActionEffect, ToolDispatchKind, ToolEffectStatus, ToolExecutionOutcome,
            ToolTransportStatus,
        };
        use crate::tool_manifest::ToolIdempotencyContract;

        let action_effect = match self.action_effect.as_str() {
            "read_only" => ToolActionEffect::ReadOnly,
            "local_mutation" => ToolActionEffect::LocalMutation,
            "external_mutation" => ToolActionEffect::ExternalMutation,
            "proposal_only" => ToolActionEffect::ProposalOnly,
            "unknown" => ToolActionEffect::Unknown,
            _ => anyhow::bail!("agent_run_tool_execution_action_effect_invalid"),
        };
        let idempotency_contract = match self.idempotency_contract.as_str() {
            "unspecified" => ToolIdempotencyContract::Unspecified,
            "non_idempotent" => ToolIdempotencyContract::NonIdempotent,
            "idempotent" => ToolIdempotencyContract::Idempotent,
            _ => anyhow::bail!("agent_run_tool_execution_idempotency_invalid"),
        };
        let dispatch_kind = match self.dispatch_kind.as_str() {
            "not_attempted" => ToolDispatchKind::NotAttempted,
            "local" => ToolDispatchKind::Local,
            "network" => ToolDispatchKind::Network,
            "mcp_stdio" => ToolDispatchKind::McpStdio,
            "a2a" => ToolDispatchKind::A2a,
            "simulated" => ToolDispatchKind::Simulated,
            "unknown" => ToolDispatchKind::Unknown,
            _ => anyhow::bail!("agent_run_tool_execution_dispatch_kind_invalid"),
        };
        let transport_status = match self.transport_status.as_str() {
            "not_attempted" => ToolTransportStatus::NotAttempted,
            "dispatched" => ToolTransportStatus::Dispatched,
            "response_observed" => ToolTransportStatus::ResponseObserved,
            "local_aborted" => ToolTransportStatus::LocalAborted,
            "remote_unknown" => ToolTransportStatus::RemoteUnknown,
            _ => anyhow::bail!("agent_run_tool_execution_transport_invalid"),
        };
        let effect_status = match self.effect_status.as_str() {
            "not_attempted" => ToolEffectStatus::NotAttempted,
            "confirmed" => ToolEffectStatus::Confirmed,
            "unknown" => ToolEffectStatus::Unknown,
            _ => anyhow::bail!("agent_run_tool_execution_effect_invalid"),
        };
        let execution_outcome = match self.execution_outcome.as_str() {
            "not_observed" => ToolExecutionOutcome::NotObserved,
            "succeeded" => ToolExecutionOutcome::Succeeded,
            "failed" => ToolExecutionOutcome::Failed,
            "unknown" => ToolExecutionOutcome::Unknown,
            _ => anyhow::bail!("agent_run_tool_execution_outcome_invalid"),
        };
        Ok(crate::agent::AgentRunToolExecutionRecord {
            run_id: self.run_id,
            receipt_id: self.receipt_id,
            manifest_id: self.manifest_id,
            request_digest: self.request_digest,
            endpoint_digest: self.endpoint_digest,
            action_effect,
            idempotency_contract,
            dispatch_kind,
            state: crate::agent::AgentRunToolExecutionState::from_str(&self.state)?,
            revision: u64::try_from(self.revision)
                .context("agent_run_tool_execution_revision_invalid")?,
            dispatch_attempt_count: u32::try_from(self.dispatch_attempt_count)
                .context("agent_run_tool_execution_attempt_count_invalid")?,
            transport_status,
            effect_status,
            execution_outcome,
            prepared_at: parse_agent_run_tool_execution_timestamp(
                &self.prepared_at,
                "prepared_at",
            )?,
            dispatch_attempted_at: parse_optional_agent_run_tool_execution_timestamp(
                self.dispatch_attempted_at,
                "dispatch_attempted_at",
            )?,
            response_observed_at: parse_optional_agent_run_tool_execution_timestamp(
                self.response_observed_at,
                "response_observed_at",
            )?,
            terminal_at: parse_optional_agent_run_tool_execution_timestamp(
                self.terminal_at,
                "terminal_at",
            )?,
        })
    }
}

/// Owner-module-only lookup seal for LifeEvent lineage. Its fields are
/// private to AgentRunStore and it has no serde implementation.
#[derive(Debug)]
pub(crate) struct CanonicalAgentRunLifeEventSourceSeal {
    run_id: String,
    canonical_store_identity: String,
    canonical_ref: String,
    content_digest: String,
    _lookup_nonce: uuid::Uuid,
}

impl CanonicalAgentRunLifeEventSourceSeal {
    pub(crate) fn run_id(&self) -> &str {
        &self.run_id
    }

    pub(crate) fn canonical_store_identity(&self) -> &str {
        &self.canonical_store_identity
    }

    pub(crate) fn canonical_ref(&self) -> &str {
        &self.canonical_ref
    }

    pub(crate) fn content_digest(&self) -> &str {
        &self.content_digest
    }
}

/// Opaque, non-cloneable execution-owner proof issued from the exact active
/// AgentRun row and its monotonic canonical mutation revision.
pub(crate) struct CanonicalAgentRunLifeEventExecutionProof {
    task_id: String,
    run_id: String,
    execution_id: String,
    session_id: String,
    input_message_store_identity: String,
    input_message_ref: String,
    canonical_store_identity: String,
    canonical_ref: String,
    canonical_content_digest: String,
    owner_revision: u64,
    runtime_binding_digest: String,
    runtime_nonce: uuid::Uuid,
}

impl CanonicalAgentRunLifeEventExecutionProof {
    fn runtime_material(&self) -> String {
        format!(
            "task_id\0{}:{}\0run_id\0{}:{}\0execution_id\0{}:{}\0session_id\0{}:{}\0input_message_store\0{}:{}\0input_message_ref\0{}:{}\0owner_store\0{}:{}\0owner_ref\0{}:{}\0owner_digest\0{}\0owner_revision\0{}\0nonce\0{}",
            self.task_id.len(),
            self.task_id,
            self.run_id.len(),
            self.run_id,
            self.execution_id.len(),
            self.execution_id,
            self.session_id.len(),
            self.session_id,
            self.input_message_store_identity.len(),
            self.input_message_store_identity,
            self.input_message_ref.len(),
            self.input_message_ref,
            self.canonical_store_identity.len(),
            self.canonical_store_identity,
            self.canonical_ref.len(),
            self.canonical_ref,
            self.canonical_content_digest,
            self.owner_revision,
            self.runtime_nonce,
        )
    }

    fn new(
        task_id: String,
        run_id: String,
        session_id: String,
        input_message_store_identity: String,
        input_message_ref: String,
        canonical_store_identity: String,
        canonical_ref: String,
        canonical_content_digest: String,
        owner_revision: u64,
    ) -> Self {
        let mut proof = Self {
            execution_id: run_id.clone(),
            task_id,
            run_id,
            session_id,
            input_message_store_identity,
            input_message_ref,
            canonical_store_identity,
            canonical_ref,
            canonical_content_digest,
            owner_revision,
            runtime_binding_digest: String::new(),
            runtime_nonce: uuid::Uuid::new_v4(),
        };
        proof.runtime_binding_digest = canonical_owner_content_digest(&proof.runtime_material());
        proof
    }

    pub(crate) fn runtime_seal_is_valid(&self) -> bool {
        self.runtime_binding_digest == canonical_owner_content_digest(&self.runtime_material())
    }

    pub(crate) fn task_id(&self) -> &str {
        &self.task_id
    }

    pub(crate) fn run_id(&self) -> &str {
        &self.run_id
    }

    pub(crate) fn execution_id(&self) -> &str {
        &self.execution_id
    }

    pub(crate) fn input_message_ref(&self) -> &str {
        &self.input_message_ref
    }

    pub(crate) fn input_message_store_identity(&self) -> &str {
        &self.input_message_store_identity
    }

    pub(crate) fn canonical_store_identity(&self) -> &str {
        &self.canonical_store_identity
    }

    pub(crate) fn canonical_ref(&self) -> &str {
        &self.canonical_ref
    }

    pub(crate) fn canonical_content_digest(&self) -> &str {
        &self.canonical_content_digest
    }

    pub(crate) fn owner_revision(&self) -> u64 {
        self.owner_revision
    }
}

impl std::fmt::Debug for CanonicalAgentRunLifeEventExecutionProof {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CanonicalAgentRunLifeEventExecutionProof")
            .field("task_id", &self.task_id)
            .field("run_id", &self.run_id)
            .field("execution_id", &self.execution_id)
            .field("owner_revision", &self.owner_revision)
            .field("authority", &"[REDACTED]")
            .finish()
    }
}

impl AgentRunStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        #[cfg(any(test, feature = "test-utils"))]
        {
            return Self::new_with_receipt_key(db_path, AgentRunReceiptKey::test_key());
        }
        #[cfg(not(any(test, feature = "test-utils")))]
        {
            let _ = db_path.into();
            anyhow::bail!("agent_run_receipt_key_required");
        }
    }

    pub fn new_with_receipt_key(
        db_path: impl Into<PathBuf>,
        receipt_key: AgentRunReceiptKey,
    ) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let (conn, canonical_db_path, owner_lease) =
            open_agent_run_database_with_stable_slot(&db_path, || {}, || {})?;
        let store_scoped_receipt_key =
            receipt_key.derive_for_canonical_database_slot(&canonical_db_path)?;
        let store = Self {
            conn: Arc::new(
                crate::sqlite_migration::IdentityBoundSqliteConnection::writable(conn, owner_lease),
            ),
            receipt_key: Arc::new(store_scoped_receipt_key),
        };
        store.init_tables(Some(&receipt_key))?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        #[cfg(any(test, feature = "test-utils"))]
        {
            return Self::new_in_memory_with_receipt_key(AgentRunReceiptKey::test_key());
        }
        #[cfg(not(any(test, feature = "test-utils")))]
        {
            anyhow::bail!("agent_run_receipt_key_required");
        }
    }

    pub fn new_in_memory_with_receipt_key(receipt_key: AgentRunReceiptKey) -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("failed to open in-memory agent_runs db")?;
        Self::configure_writable_connection(&conn, false)?;
        let store = Self {
            conn: Arc::new(crate::sqlite_migration::IdentityBoundSqliteConnection::in_memory(conn)),
            receipt_key: Arc::new(receipt_key),
        };
        store.init_tables(None)?;
        Ok(store)
    }

    pub fn open_read_only_existing(db_path: impl Into<PathBuf>) -> Result<Self> {
        #[cfg(any(test, feature = "test-utils"))]
        {
            return Self::open_read_only_existing_with_receipt_key(
                db_path,
                AgentRunReceiptKey::test_key(),
            );
        }
        #[cfg(not(any(test, feature = "test-utils")))]
        {
            let _ = db_path.into();
            anyhow::bail!("agent_run_receipt_key_required");
        }
    }

    pub fn open_read_only_existing_with_receipt_key(
        db_path: impl Into<PathBuf>,
        receipt_key: AgentRunReceiptKey,
    ) -> Result<Self> {
        let db_path = db_path.into();
        let (conn, canonical_db_path, identity_guard) =
            open_agent_run_database_read_only_with_stable_slot(&db_path, || {}, || {})?;
        let store_scoped_receipt_key =
            receipt_key.derive_for_canonical_database_slot(&canonical_db_path)?;
        Self::validate_receipt_key_binding(&conn, &store_scoped_receipt_key, None, false)?;
        if !Self::agent_run_v7_physical_purge_complete(&conn)? {
            anyhow::bail!("agent_run_v7_physical_purge_incomplete");
        }
        Ok(Self {
            conn: Arc::new(
                crate::sqlite_migration::IdentityBoundSqliteConnection::read_only(
                    conn,
                    identity_guard,
                ),
            ),
            receipt_key: Arc::new(store_scoped_receipt_key),
        })
    }

    fn load_or_create_canonical_store_identity(conn: &Connection) -> Result<String> {
        let existing = conn
            .query_row(
                "SELECT value FROM agent_run_store_metadata
                 WHERE key = 'canonical_store_identity'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing
                .strip_prefix("agent_run_store:")
                .and_then(|identity| uuid::Uuid::parse_str(identity).ok())
                .is_none()
            {
                anyhow::bail!("agent_run_canonical_store_identity_invalid");
            }
            return Ok(existing);
        }
        let identity = format!("agent_run_store:{}", uuid::Uuid::new_v4());
        conn.execute(
            "INSERT INTO agent_run_store_metadata(key, value)
             VALUES ('canonical_store_identity', ?1)",
            [&identity],
        )?;
        Ok(identity)
    }

    fn existing_canonical_store_identity(conn: &Connection) -> Result<String> {
        let identity = conn
            .query_row(
                "SELECT value FROM agent_run_store_metadata
                 WHERE key = 'canonical_store_identity'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .context("agent_run_canonical_store_identity_missing")?;
        if identity
            .strip_prefix("agent_run_store:")
            .and_then(|value| uuid::Uuid::parse_str(value).ok())
            .is_none()
        {
            anyhow::bail!("agent_run_canonical_store_identity_invalid");
        }
        Ok(identity)
    }

    pub(crate) fn canonical_store_identity(&self) -> Result<String> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        Self::existing_canonical_store_identity(&conn)
    }

    pub(crate) fn create_low_risk_life_event_from_authorities(
        &self,
        life_event_store: &crate::agent::LifeEventStore,
        message_proof: &crate::memory::CanonicalConversationMessageProof,
        policy_proof: crate::agent::main_chat_memory_candidate::DeterministicLifeEventPolicyProof,
        run_id: &str,
        operation_id: &str,
    ) -> Result<crate::agent::LifeEvent> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let owner_tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (
            canonical_store_identity,
            canonical_ref,
            canonical_content_digest,
            task_id,
            session_id,
            input_message_store_identity,
            input_message_ref,
        ) = Self::current_life_event_owner_snapshot(&owner_tx, self.receipt_key.as_ref(), run_id)?;
        if session_id != message_proof.session_id() {
            anyhow::bail!("life_event_create_execution_message_session_mismatch");
        }
        if input_message_ref != message_proof.canonical_ref() {
            anyhow::bail!("life_event_create_execution_message_ref_mismatch");
        }
        if input_message_store_identity != message_proof.canonical_store_identity() {
            anyhow::bail!("life_event_create_execution_message_store_mismatch");
        }
        let owner_revision = owner_tx
            .query_row(
                "SELECT revision FROM agent_run_canonical_revisions WHERE run_id = ?1",
                [run_id],
                |row| row.get::<_, u64>(0),
            )
            .optional()?
            .context("life_event_create_permit_owner_revision_missing_or_stale")?;
        let execution_proof = CanonicalAgentRunLifeEventExecutionProof::new(
            task_id,
            run_id.to_string(),
            session_id,
            input_message_store_identity,
            input_message_ref,
            canonical_store_identity,
            canonical_ref,
            canonical_content_digest,
            owner_revision,
        );
        let (permit, draft) =
            crate::agent::lifemodel_backend_completion::issue_life_event_create_permit(
                message_proof,
                policy_proof,
                &execution_proof,
                operation_id,
            )?;
        permit.matches_current_agent_run_owner(
            execution_proof.canonical_store_identity(),
            execution_proof.canonical_ref(),
            execution_proof.canonical_content_digest(),
            execution_proof.owner_revision(),
            execution_proof.task_id(),
            execution_proof.run_id(),
        )?;
        let event = life_event_store.create_event_with_permit(permit, draft)?;
        owner_tx.commit()?;
        Ok(event)
    }

    #[cfg(test)]
    pub(crate) fn issue_life_event_execution_proof_for_test(
        &self,
        run_id: &str,
        expected_session_id: &str,
    ) -> Result<CanonicalAgentRunLifeEventExecutionProof> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (
            store_identity,
            canonical_ref,
            content_digest,
            task_id,
            session_id,
            input_message_store_identity,
            input_message_ref,
        ) = Self::current_life_event_owner_snapshot(&tx, self.receipt_key.as_ref(), run_id)?;
        if session_id != expected_session_id {
            anyhow::bail!("life_event_create_execution_message_session_mismatch");
        }
        let revision = tx.query_row(
            "SELECT revision FROM agent_run_canonical_revisions WHERE run_id = ?1",
            [run_id],
            |row| row.get::<_, u64>(0),
        )?;
        tx.commit()?;
        Ok(CanonicalAgentRunLifeEventExecutionProof::new(
            task_id,
            run_id.to_string(),
            session_id,
            input_message_store_identity,
            input_message_ref,
            store_identity,
            canonical_ref,
            content_digest,
            revision,
        ))
    }

    #[cfg(test)]
    pub(crate) fn commit_prepared_life_event_for_test(
        &self,
        life_event_store: &crate::agent::LifeEventStore,
        permit: crate::agent::lifemodel_backend_completion::LifeEventCreatePermit,
        draft: crate::agent::LifeEventDraft,
    ) -> Result<crate::agent::LifeEvent> {
        if !permit.runtime_seal_is_valid() || !permit.matches_draft(&draft) {
            anyhow::bail!("life_event_create_permit_binding_invalid");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run_id = draft
            .source_run_id
            .as_deref()
            .context("life_event_create_permit_draft_run_missing")?;
        let (store_identity, canonical_ref, content_digest, task_id, _, _, _) =
            Self::current_life_event_owner_snapshot(&tx, self.receipt_key.as_ref(), run_id)?;
        let revision = tx.query_row(
            "SELECT revision FROM agent_run_canonical_revisions WHERE run_id = ?1",
            [run_id],
            |row| row.get::<_, u64>(0),
        )?;
        permit.matches_current_agent_run_owner(
            &store_identity,
            &canonical_ref,
            &content_digest,
            revision,
            &task_id,
            run_id,
        )?;
        let event = life_event_store.create_event_with_permit(permit, draft)?;
        tx.commit()?;
        Ok(event)
    }

    /// Return the monotonic canonical row revision used by cross-store owner
    /// receipts. Callers must bind this together with a content digest; a
    /// revision alone is not a substitute for canonical row validation.
    pub fn canonical_revision(&self, run_id: &str) -> Result<u64> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        conn.query_row(
            "SELECT revision FROM agent_run_canonical_revisions WHERE run_id = ?1",
            [run_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    #[cfg(test)]
    pub(crate) fn canonical_revision_for_test(&self, run_id: &str) -> Result<u64> {
        self.canonical_revision(run_id)
    }

    fn current_life_event_owner_snapshot(
        conn: &Connection,
        receipt_key: &AgentRunReceiptKey,
        run_id: &str,
    ) -> Result<(String, String, String, String, String, String, String)> {
        let canonical_store_identity = Self::existing_canonical_store_identity(conn)?;
        let input_message_store_identity = conn
            .query_row(
                "SELECT value FROM agent_run_store_metadata
                 WHERE key = 'canonical_memory_store_identity'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .context("life_event_create_execution_message_store_missing")?;
        let query = format!(
            "SELECT {AGENT_RUN_SELECT_COLUMNS}
             FROM agent_runs AS run
             WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let run = conn
            .query_row(&query, [run_id], |row| {
                Self::row_to_run(row, receipt_key, &canonical_store_identity)
            })
            .optional()?
            .context("canonical_agent_run_life_event_source_missing")?;
        let canonical_ref = format!("agent-run://{}", run.id);
        let canonical_content =
            serde_json::to_string(&run).context("serialize canonical AgentRun owner snapshot")?;
        let canonical_content_digest = canonical_owner_content_digest(&canonical_content);
        let input_message_ref = run
            .input_ref
            .clone()
            .context("life_event_create_execution_input_message_ref_missing")?;
        Ok((
            canonical_store_identity,
            canonical_ref,
            canonical_content_digest,
            run.task_id,
            run.session_id
                .context("life_event_create_execution_session_missing")?,
            input_message_store_identity,
            input_message_ref,
        ))
    }

    /// Fixed lock order: canonical AgentRun owner first, then LifeEventStore.
    /// No caller-provided closure or external await executes under the guard.
    pub(crate) fn create_life_event_from_active_run(
        &self,
        life_event_store: &crate::agent::LifeEventStore,
        run_id: &str,
        source_detail: Option<&str>,
        draft: crate::agent::LifeEventDraft,
        domain: crate::agent::LifeDomain,
        risk_level: crate::agent::RiskLevel,
        privacy_level: crate::agent::LifeEventPrivacyLevel,
    ) -> Result<crate::agent::LifeEvent> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let canonical_store_identity = conn
            .query_row(
                "SELECT value FROM agent_run_store_metadata
                 WHERE key = 'canonical_store_identity'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .context("agent_run_canonical_store_identity_missing")?;
        let active_source_query = format!(
            "SELECT run.id, run.task_id, run.status, run.kind, run.started_at,
                    run.payload_minimized_version
             FROM agent_runs AS run
             WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let current = conn
            .query_row(&active_source_query, [run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })
            .optional()?;
        let Some((id, task_id, status, kind, started_at, payload_version)) = current else {
            anyhow::bail!("canonical_agent_run_life_event_source_missing");
        };
        if id != run_id || payload_version != AGENT_RUN_PAYLOAD_VERSION {
            anyhow::bail!("canonical_agent_run_life_event_source_invalid");
        }
        let canonical_ref = format!("agent-run://{id}");
        let content_digest = crate::persistence_outbox::metadata_digest(&format!(
            "id\0{}:{}\0task_id\0{}:{}\0status\0{}\0kind\0{}\0started_at\0{}\0payload_version\0{}",
            id.len(),
            id,
            task_id.len(),
            task_id,
            status,
            kind,
            started_at,
            payload_version,
        ));
        let seal = CanonicalAgentRunLifeEventSourceSeal {
            run_id: id,
            canonical_store_identity,
            canonical_ref,
            content_digest,
            _lookup_nonce: uuid::Uuid::new_v4(),
        };
        life_event_store.create_event_from_canonical_sources(
            draft,
            vec![
                crate::agent::CanonicalLifeEventSourceProof::from_agent_run_lookup(
                    seal,
                    source_detail,
                ),
            ],
            domain,
            risk_level,
            privacy_level,
        )
    }

    fn validate_receipt_key_binding(
        conn: &Connection,
        receipt_key: &AgentRunReceiptKey,
        legacy_unscoped_receipt_key: Option<&AgentRunReceiptKey>,
        allow_initialize: bool,
    ) -> Result<()> {
        const VERIFIER_MATERIAL: &str = "openlife-agent-run-store-key-binding-v1";
        let expected = receipt_key.sign("store_key_verifier", VERIFIER_MATERIAL);
        let stored = conn
            .query_row(
                "SELECT value FROM agent_run_store_metadata
                 WHERE key = 'receipt_key_verifier'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match stored {
            Some(stored)
                if receipt_key.verify("store_key_verifier", VERIFIER_MATERIAL, &stored) =>
            {
                Ok(())
            }
            Some(stored)
                if allow_initialize
                    && legacy_unscoped_receipt_key.is_some_and(|legacy_key| {
                        legacy_key.verify("store_key_verifier", VERIFIER_MATERIAL, &stored)
                    }) =>
            {
                Self::quarantine_legacy_unscoped_receipt_authority(conn, receipt_key, &expected)
            }
            Some(_) => anyhow::bail!("agent_run_receipt_key_mismatch"),
            None if allow_initialize => {
                let current_receipt_rows: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM agent_runs
                     WHERE payload_minimized_version >= ?1",
                    [AGENT_RUN_PAYLOAD_VERSION],
                    |row| row.get(0),
                )?;
                if current_receipt_rows != 0 {
                    anyhow::bail!("agent_run_receipt_key_binding_missing_for_current_rows");
                }
                let legacy_rows: i64 =
                    conn.query_row("SELECT COUNT(*) FROM agent_runs", [], |row| row.get(0))?;
                if legacy_rows == 0 {
                    conn.execute(
                        "INSERT INTO agent_run_store_metadata(key, value)
                         VALUES ('receipt_key_verifier', ?1)",
                        [expected],
                    )?;
                    Ok(())
                } else {
                    Self::quarantine_legacy_unscoped_receipt_authority(conn, receipt_key, &expected)
                }
            }
            None => anyhow::bail!("agent_run_receipt_key_binding_missing"),
        }
    }

    /// A pre-slot-scoped database may contain receipts authenticated only by
    /// the installation-wide key. Those receipts cannot prove which copied
    /// database is the canonical owner. Preserve minimized history, but strip
    /// bound receipt authority, clear every pending/attached issuance, mark
    /// the rows unverified, rotate the store identity, and only then bind the
    /// current canonical filesystem slot.
    fn quarantine_legacy_unscoped_receipt_authority(
        conn: &Connection,
        receipt_key: &AgentRunReceiptKey,
        scoped_verifier: &str,
    ) -> Result<()> {
        if !receipt_key.verify(
            "store_key_verifier",
            "openlife-agent-run-store-key-binding-v1",
            scoped_verifier,
        ) {
            anyhow::bail!("agent_run_scoped_receipt_key_binding_failed");
        }
        let rows = {
            let mut statement = conn.prepare(
                "SELECT id, COALESCE(actions_json, '[]'), COALESCE(observations_json, '[]')
                 FROM agent_runs",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        let mut quarantined_rows = Vec::with_capacity(rows.len());
        for (run_id, actions_json, observations_json) in rows {
            let mut actions: Vec<crate::agent::types::AgentAction> =
                serde_json::from_str(&actions_json).with_context(|| {
                    format!("agent_run_legacy_receipt_actions_invalid:{run_id}")
                })?;
            for action in &mut actions {
                if action
                    .output
                    .as_ref()
                    .is_some_and(is_bound_content_value_ref)
                {
                    action.output = action.output.as_ref().map(|value| {
                        metadata_safe_value_receipt(
                            "action_output",
                            value,
                            ReceiptOrigin::NewInput(receipt_key),
                        )
                    });
                }
                if action
                    .error
                    .as_deref()
                    .is_some_and(is_bound_content_text_ref)
                {
                    action.error = action.error.as_deref().map(|value| {
                        metadata_safe_text_receipt(
                            "action_error",
                            value,
                            ReceiptOrigin::NewInput(receipt_key),
                        )
                    });
                }
                if let Some(trace) = action.react_trace.as_mut() {
                    trace.output_receipt = None;
                }
                action.runtime_execution_receipt = None;
            }
            let mut observations: Vec<crate::agent::types::AgentObservation> =
                serde_json::from_str(&observations_json).with_context(|| {
                    format!("agent_run_legacy_receipt_observations_invalid:{run_id}")
                })?;
            for observation in &mut observations {
                if is_bound_content_text_ref(&observation.content) {
                    observation.content = metadata_safe_text_receipt(
                        "observation_content",
                        &observation.content,
                        ReceiptOrigin::NewInput(receipt_key),
                    );
                }
                if let Some(trace) = observation.react_trace.as_mut() {
                    trace.output_receipt = None;
                }
            }
            quarantined_rows.push((
                run_id,
                serde_json::to_string(&actions)?,
                serde_json::to_string(&observations)?,
            ));
        }

        let tx = conn.unchecked_transaction()?;
        for (run_id, actions_json, observations_json) in quarantined_rows {
            tx.execute(
                "UPDATE agent_runs
                 SET actions_json = ?2,
                     observations_json = ?3,
                     legacy_payload_unverified = 1
                 WHERE id = ?1",
                params![run_id, actions_json, observations_json],
            )?;
        }
        tx.execute("DELETE FROM bound_content_issuance_ledger", [])?;
        let rotated_identity = format!("agent_run_store:{}", uuid::Uuid::new_v4());
        tx.execute(
            "INSERT INTO agent_run_store_metadata(key, value)
             VALUES ('canonical_store_identity', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [&rotated_identity],
        )?;
        tx.execute(
            "INSERT INTO agent_run_store_metadata(key, value)
             VALUES ('receipt_key_verifier', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [scoped_verifier],
        )?;
        tx.execute(
            "INSERT INTO agent_run_store_metadata(key, value)
             VALUES ('legacy_receipt_authority_quarantined', 'true')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn configure_writable_connection(conn: &Connection, file_backed: bool) -> Result<()> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA secure_delete = ON;
             PRAGMA foreign_keys = OFF;",
        )?;
        if file_backed {
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(None, "synchronous", "FULL")?;
        }
        Ok(())
    }

    /// Repair pre-fix rows whose local deletion marker was cleared while the
    /// canonical tombstone remained active. The tombstone timestamp is the
    /// only source used for the marker; this does not create, supersede, or
    /// reinterpret canonical deletion authority.
    fn reconcile_active_tombstone_run_markers(conn: &Connection) -> Result<usize> {
        conn.execute(
            "UPDATE agent_runs
             SET deleted_at = (
                 SELECT tombstone.created_at
                 FROM canonical_tombstones tombstone
                 WHERE tombstone.aggregate_kind = 'agent_run'
                   AND tombstone.aggregate_id = agent_runs.id
                   AND tombstone.superseded_at IS NULL
                 ORDER BY tombstone.created_at DESC, tombstone.tombstone_id DESC
                 LIMIT 1
             )
             WHERE deleted_at IS NULL
               AND EXISTS (
                   SELECT 1 FROM canonical_tombstones tombstone
                   WHERE tombstone.aggregate_kind = 'agent_run'
                     AND tombstone.aggregate_id = agent_runs.id
                     AND tombstone.superseded_at IS NULL
               )",
            [],
        )
        .map_err(Into::into)
    }

    fn prune_bound_content_issuance_ledger(conn: &Connection, now: i64) -> Result<()> {
        // Bootstrap-safe cleanup. Canonical tombstones are initialized only
        // after legacy receipt authority has been quarantined; marker
        // reconciliation then makes this local predicate converge with the
        // product-live predicate before a second pruning pass.
        conn.execute(
            "DELETE FROM bound_content_issuance_ledger
             WHERE state = 'pending'
               AND NOT EXISTS (
                   SELECT 1 FROM agent_runs
                   WHERE agent_runs.id = bound_content_issuance_ledger.run_id
                     AND agent_runs.deleted_at IS NULL
               )",
            [],
        )?;
        conn.execute(
            "DELETE FROM bound_content_issuance_ledger
             WHERE state = 'pending' AND expires_at < ?1",
            [now],
        )?;
        conn.execute(
            "DELETE FROM bound_content_issuance_ledger
             WHERE state = 'attached'
               AND attached_at IS NOT NULL
               AND attached_at < ?1",
            [now - BOUND_CONTENT_ATTACHED_RETENTION_SECONDS],
        )?;
        conn.execute(
            "DELETE FROM bound_content_issuance_ledger
             WHERE state = 'attached'
               AND issuance_id IN (
                   SELECT issuance_id
                   FROM bound_content_issuance_ledger
                   WHERE state = 'attached'
                   ORDER BY attached_at DESC, issuance_id DESC
                   LIMIT -1 OFFSET ?1
               )",
            [MAX_BOUND_CONTENT_ATTACHED_RETAINED],
        )?;
        Ok(())
    }

    fn attach_bound_content_issuances(
        tx: &Transaction<'_>,
        canonical_store_identity: &str,
        run_id: &str,
        attachments: &[PendingBoundContentAttachment],
        now: i64,
    ) -> Result<()> {
        for attachment in attachments {
            let changed = tx.execute(
                "UPDATE bound_content_issuance_ledger
                 SET state = 'attached', attached_at = ?1
                 WHERE issuance_id = ?2
                   AND receipt_id = ?3
                   AND canonical_store_identity = ?4
                   AND run_id = ?5
                   AND action_id = ?6
                   AND observation_id = ?7
                   AND receipt_json = ?8
                   AND state = 'pending'
                   AND expires_at >= ?1",
                params![
                    now,
                    attachment.issuance_id,
                    attachment.receipt_id,
                    canonical_store_identity,
                    run_id,
                    attachment.action_id,
                    attachment.observation_id,
                    attachment.receipt_json,
                ],
            )?;
            if changed != 1 {
                anyhow::bail!("bound_content_receipt_pending_attach_cas_failed");
            }
        }
        Ok(())
    }

    fn init_tables(&self, legacy_unscoped_receipt_key: Option<&AgentRunReceiptKey>) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agent_runs (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                session_id TEXT,
                status TEXT NOT NULL,
                kind TEXT NOT NULL,
                context_summary_json TEXT,
                model_route_json TEXT,
                output_preview TEXT,
                error_json TEXT,
                generated_proposals_json TEXT DEFAULT '[]',
                actions_json TEXT DEFAULT '[]',
                observations_json TEXT DEFAULT '[]',
                reasoning_strategy TEXT,
                hs_selection_audit_json TEXT,
                behavior_checks_json TEXT DEFAULT '[]',
                status_updates_json TEXT NOT NULL DEFAULT '[]',
                step_count INTEGER NOT NULL DEFAULT 0,
                tool_call_count INTEGER NOT NULL DEFAULT 0,
                deleted_at TEXT,
                delete_reason TEXT,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                input_ref TEXT,
                input_digest TEXT,
                reasoning_trace_digest TEXT,
                payload_minimized_version INTEGER NOT NULL DEFAULT 0,
                legacy_payload_unverified INTEGER NOT NULL DEFAULT 0
                    CHECK(legacy_payload_unverified IN (0, 1))
            )",
            [],
        )?;
        let legacy_raw_schema = Self::column_exists(&conn, "agent_runs", "user_input")?
            || Self::column_exists(&conn, "agent_runs", "reasoning_trace_json")?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agent_run_store_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            ) WITHOUT ROWID",
            [],
        )?;
        Self::load_or_create_canonical_store_identity(&conn)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS bound_content_issuance_ledger (
                issuance_id TEXT PRIMARY KEY,
                receipt_id TEXT NOT NULL UNIQUE,
                canonical_store_identity TEXT NOT NULL,
                run_id TEXT NOT NULL,
                action_id TEXT NOT NULL,
                observation_id TEXT NOT NULL,
                receipt_json TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN ('pending', 'attached')),
                issued_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                attached_at INTEGER,
                CHECK(
                    (state = 'pending' AND attached_at IS NULL)
                    OR (state = 'attached' AND attached_at IS NOT NULL)
                )
            ) WITHOUT ROWID;
            CREATE INDEX IF NOT EXISTS idx_bound_content_issuance_pending_run
                ON bound_content_issuance_ledger(state, run_id, expires_at);
            CREATE INDEX IF NOT EXISTS idx_bound_content_issuance_attached_at
                ON bound_content_issuance_ledger(state, attached_at);",
        )?;
        // Migration: add columns with idempotent helper
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "generated_proposals_json",
            "TEXT DEFAULT '[]'",
        )?;
        Self::add_column_if_missing(&conn, "agent_runs", "deleted_at", "TEXT")?;
        Self::add_column_if_missing(&conn, "agent_runs", "delete_reason", "TEXT")?;
        Self::add_column_if_missing(&conn, "agent_runs", "actions_json", "TEXT DEFAULT '[]'")?;
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "observations_json",
            "TEXT DEFAULT '[]'",
        )?;
        Self::add_column_if_missing(&conn, "agent_runs", "reasoning_strategy", "TEXT")?;
        if legacy_raw_schema {
            // Only an existing pre-v7 table may receive the historical raw
            // column needed to complete its one-way migration. A fresh v7
            // release database never creates either raw column.
            Self::add_column_if_missing(&conn, "agent_runs", "user_input", "TEXT")?;
            Self::add_column_if_missing(&conn, "agent_runs", "reasoning_trace_json", "TEXT")?;
        }
        Self::add_column_if_missing(&conn, "agent_runs", "hs_selection_audit_json", "TEXT")?;
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "behavior_checks_json",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        // Phase 0 migration: status_updates, step_count, tool_call_count
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "status_updates_json",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "step_count",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "tool_call_count",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        Self::add_column_if_missing(&conn, "agent_runs", "input_ref", "TEXT")?;
        Self::add_column_if_missing(&conn, "agent_runs", "input_digest", "TEXT")?;
        Self::add_column_if_missing(&conn, "agent_runs", "reasoning_trace_digest", "TEXT")?;
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "payload_minimized_version",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "legacy_payload_unverified",
            "INTEGER NOT NULL DEFAULT 0 CHECK(legacy_payload_unverified IN (0, 1))",
        )?;
        Self::validate_receipt_key_binding(
            &conn,
            &self.receipt_key,
            legacy_unscoped_receipt_key,
            true,
        )?;
        Self::prune_bound_content_issuance_ledger(&conn, chrono::Utc::now().timestamp())?;
        if legacy_raw_schema {
            Self::minimize_legacy_run_payloads(&conn, &self.receipt_key)?;
        } else {
            let unsupported_legacy_rows: i64 = conn.query_row(
                "SELECT COUNT(*) FROM agent_runs
                 WHERE payload_minimized_version < ?1",
                [AGENT_RUN_PAYLOAD_VERSION],
                |row| row.get(0),
            )?;
            if unsupported_legacy_rows != 0 {
                anyhow::bail!("agent_run_legacy_raw_source_columns_missing");
            }
        }
        // A missing/incomplete physical-purge marker is intentionally handled
        // even when the raw columns are already absent. That is the crash
        // recovery path for a process that committed the v7 table swap but
        // exited before checkpoint/VACUUM could reclaim the retired pages.
        Self::rebuild_agent_runs_without_raw_columns(&conn, AgentRunTableRebuildFault::None)?;
        // Preserve the established receipt-migration order. Tombstone
        // authority becomes readable only after legacy receipt payloads have
        // been quarantined/minimized; marker repair still runs before live
        // indexes, duplicate checks, or product reconciliation.
        persistence_outbox::init_schema(&conn)?;
        Self::reconcile_active_tombstone_run_markers(&conn)?;
        Self::prune_bound_content_issuance_ledger(&conn, chrono::Utc::now().timestamp())?;
        Self::install_agent_run_revision_authority(&conn)?;
        Self::validate_current_run_identity_domain(&conn)?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_session ON agent_runs(session_id, started_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_started ON agent_runs(started_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_started_id
             ON agent_runs(started_at DESC, id DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_deleted_at ON agent_runs(deleted_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_waiting_permission
             ON agent_runs(status, id)
             WHERE deleted_at IS NULL",
            [],
        )?;
        let duplicate_task_identity_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM (
                 SELECT task_id
                 FROM agent_runs
                 WHERE deleted_at IS NULL AND TRIM(task_id) != ''
                 GROUP BY task_id
                 HAVING COUNT(*) > 1
             )",
            [],
            |row| row.get(0),
        )?;
        if duplicate_task_identity_count != 0 {
            anyhow::bail!("agent_run_task_identity_migration_conflict");
        }
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_runs_canonical_task
             ON agent_runs(task_id)
             WHERE deleted_at IS NULL AND TRIM(task_id) != ''",
            [],
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_run_proposal_links (
                run_id TEXT NOT NULL,
                proposal_id TEXT NOT NULL,
                PRIMARY KEY (run_id, proposal_id),
                FOREIGN KEY (run_id) REFERENCES agent_runs(id) ON DELETE CASCADE
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_agent_run_proposal_links_proposal
             ON agent_run_proposal_links(proposal_id, run_id);",
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_run_tool_executions (
                receipt_id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                manifest_id TEXT NOT NULL,
                request_digest TEXT NOT NULL,
                endpoint_digest TEXT NOT NULL,
                action_effect TEXT NOT NULL,
                idempotency_contract TEXT NOT NULL,
                dispatch_kind TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN (
                    'prepared',
                    'dispatch_attempted',
                    'response_observed',
                    'terminal_succeeded',
                    'terminal_failed',
                    'terminal_not_attempted',
                    'terminal_remote_unknown'
                )),
                revision INTEGER NOT NULL CHECK(revision >= 0),
                dispatch_attempt_count INTEGER NOT NULL
                    CHECK(dispatch_attempt_count >= 0),
                transport_status TEXT NOT NULL,
                effect_status TEXT NOT NULL,
                execution_outcome TEXT NOT NULL,
                prepared_at TEXT NOT NULL,
                dispatch_attempted_at TEXT,
                response_observed_at TEXT,
                terminal_at TEXT,
                FOREIGN KEY (run_id) REFERENCES agent_runs(id) ON DELETE RESTRICT,
                CHECK(
                    (state = 'prepared'
                     AND revision = 0
                     AND dispatch_attempt_count = 0
                     AND dispatch_attempted_at IS NULL
                     AND response_observed_at IS NULL
                     AND terminal_at IS NULL)
                    OR state != 'prepared'
                ),
                CHECK(
                    (state LIKE 'terminal_%' AND terminal_at IS NOT NULL)
                    OR (state NOT LIKE 'terminal_%' AND terminal_at IS NULL)
                )
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_agent_run_tool_executions_run
             ON agent_run_tool_executions(run_id, prepared_at, receipt_id);
             CREATE INDEX IF NOT EXISTS idx_agent_run_tool_executions_recovery
             ON agent_run_tool_executions(state, prepared_at, receipt_id);",
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS openlife_schema_versions (
                component TEXT PRIMARY KEY,
                version INTEGER NOT NULL,
                applied_at TEXT NOT NULL
            )",
            [],
        )?;
        let tool_execution_schema_version = conn
            .query_row(
                "SELECT version FROM openlife_schema_versions
                 WHERE component = 'agent_run_tool_executions'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        if tool_execution_schema_version < 1 {
            let tx = conn.unchecked_transaction()?;
            crate::sqlite_migration::record_schema_version(&tx, "agent_run_tool_executions", 1)?;
            tx.commit()?;
        }
        let proposal_link_schema_version = conn
            .query_row(
                "SELECT version FROM openlife_schema_versions
                 WHERE component = 'agent_run_proposal_links'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        if proposal_link_schema_version < 1 {
            let legacy_links = {
                let mut statement =
                    conn.prepare("SELECT id, generated_proposals_json FROM agent_runs")?;
                let rows = statement
                    .query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                rows
            };
            let tx = conn.unchecked_transaction()?;
            tx.execute("DELETE FROM agent_run_proposal_links", [])?;
            for (run_id, proposals_json) in legacy_links {
                let proposal_ids = proposals_json
                    .as_deref()
                    .map(serde_json::from_str::<Vec<String>>)
                    .transpose()
                    .with_context(|| {
                        format!("invalid generated_proposals_json for AgentRun {run_id}")
                    })?
                    .unwrap_or_default();
                let proposal_ids = Self::normalize_proposal_ids(&run_id, &proposal_ids)?;
                tx.execute(
                    "UPDATE agent_runs SET generated_proposals_json = ?2 WHERE id = ?1",
                    params![
                        run_id,
                        serde_json::to_string(&proposal_ids)
                            .context("failed to serialize legacy AgentRun proposal references")?,
                    ],
                )?;
                Self::replace_proposal_links(&tx, &run_id, &proposal_ids)?;
            }
            crate::sqlite_migration::record_schema_version(&tx, "agent_run_proposal_links", 1)?;
            tx.commit()?;
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_run_session_tombstone_projections (
                tombstone_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                applied_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_agent_run_session_tombstone_session
             ON agent_run_session_tombstone_projections(session_id);",
        )?;
        Self::reconcile_agent_run_tool_execution_orphans(&conn, self.receipt_key.as_ref())?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let foreign_key_fault: Option<String> = conn
            .query_row("PRAGMA foreign_key_check", [], |row| row.get(0))
            .optional()?;
        if foreign_key_fault.is_some() {
            anyhow::bail!("agent_run_store_foreign_key_check_failed");
        }
        Ok(())
    }

    fn validate_live_a2a_tool_receipt_identity(
        run_id: &str,
        receipt: &crate::tool_execution_receipt::ToolExecutionReceipt,
    ) -> Result<()> {
        use crate::tool_execution_receipt::ToolActionEffect;
        use crate::tool_manifest::ToolIdempotencyContract;

        if !receipt.is_runtime_issued() {
            anyhow::bail!("agent_run_tool_execution_live_receipt_required");
        }
        if receipt.source_run_id.as_deref() != Some(run_id)
            || receipt
                .manifest_id
                .as_deref()
                .is_none_or(|manifest_id| manifest_id.trim().is_empty())
            || !exact_sha256_metadata_digest(&receipt.request_digest)
            || receipt.action_effect != ToolActionEffect::ExternalMutation
            || receipt.idempotency_contract != ToolIdempotencyContract::NonIdempotent
        {
            anyhow::bail!("agent_run_a2a_tool_execution_receipt_binding_invalid");
        }
        Ok(())
    }

    fn validate_agent_run_a2a_prepare<'a>(
        run_id: &str,
        endpoint_digest: &str,
        receipt: &'a crate::tool_execution_receipt::ToolExecutionReceipt,
    ) -> Result<&'a str> {
        use crate::tool_execution_receipt::{
            ToolDispatchKind, ToolEffectStatus, ToolExecutionOutcome, ToolTransportStatus,
        };

        Self::validate_live_a2a_tool_receipt_identity(run_id, receipt)?;
        if !exact_sha256_metadata_digest(endpoint_digest)
            || receipt.finished_at.is_some()
            || receipt.dispatch_kind != ToolDispatchKind::NotAttempted
            || receipt.dispatch_attempt_count != 0
            || receipt.dispatch_observed
            || receipt.transport_status != ToolTransportStatus::NotAttempted
            || receipt.effect_status != ToolEffectStatus::NotAttempted
            || receipt.execution_outcome != ToolExecutionOutcome::NotObserved
        {
            anyhow::bail!("agent_run_a2a_tool_execution_prepare_state_invalid");
        }
        receipt
            .manifest_id
            .as_deref()
            .context("agent_run_a2a_tool_execution_manifest_missing")
    }

    fn insert_prepared_agent_run_a2a_tool_execution(
        tx: &Transaction<'_>,
        run_id: &str,
        endpoint_digest: &str,
        manifest_id: &str,
        receipt: &crate::tool_execution_receipt::ToolExecutionReceipt,
    ) -> Result<()> {
        tx.execute(
            "INSERT INTO agent_run_tool_executions (
                receipt_id, run_id, manifest_id, request_digest,
                endpoint_digest, action_effect, idempotency_contract,
                dispatch_kind, state, revision, dispatch_attempt_count,
                transport_status, effect_status, execution_outcome,
                prepared_at, dispatch_attempted_at, response_observed_at,
                terminal_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                'not_attempted', 'prepared', 0, 0,
                'not_attempted', 'not_attempted', 'not_observed',
                ?8, NULL, NULL, NULL
             )",
            params![
                receipt.receipt_id,
                run_id,
                manifest_id,
                receipt.request_digest,
                endpoint_digest,
                receipt.action_effect.as_str(),
                receipt.idempotency_contract.as_str(),
                receipt.started_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Atomically establish the dedicated A2A AgentRun and its prepared child
    /// execution. This is intentionally stricter than `create_run`: callers
    /// cannot reuse an existing parent or silently attach a second request.
    pub(crate) fn create_run_and_prepare_agent_run_a2a_tool_execution(
        &self,
        run: &AgentRun,
        endpoint_digest: &str,
        receipt: &crate::tool_execution_receipt::ToolExecutionReceipt,
    ) -> Result<()> {
        if run.status != AgentRunStatus::Running
            || run.kind != AgentTaskKind::ToolExecution
            || !run.actions.is_empty()
            || !run.observations.is_empty()
            || run.finished_at.is_some()
            || run.deleted_at.is_some()
        {
            anyhow::bail!("agent_run_a2a_atomic_owner_template_invalid");
        }
        let manifest_id = Self::validate_agent_run_a2a_prepare(&run.id, endpoint_digest, receipt)?;
        self.create_run_internal_with_transaction(run, None, |tx| {
            Self::insert_prepared_agent_run_a2a_tool_execution(
                tx,
                &run.id,
                endpoint_digest,
                manifest_id,
                receipt,
            )
        })
    }

    pub(crate) fn prepare_agent_run_a2a_tool_execution(
        &self,
        run_id: &str,
        endpoint_digest: &str,
        receipt: &crate::tool_execution_receipt::ToolExecutionReceipt,
    ) -> Result<()> {
        let manifest_id = Self::validate_agent_run_a2a_prepare(run_id, endpoint_digest, receipt)?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let tx = conn.transaction()?;
        let active_owner_query = format!(
            "SELECT run.status FROM agent_runs AS run
             WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let active_owner: Option<String> = tx
            .query_row(&active_owner_query, [run_id], |row| row.get(0))
            .optional()?;
        if active_owner.as_deref() != Some("running") {
            anyhow::bail!("agent_run_a2a_tool_execution_owner_not_running");
        }
        Self::insert_prepared_agent_run_a2a_tool_execution(
            &tx,
            run_id,
            endpoint_digest,
            manifest_id,
            receipt,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn mark_agent_run_a2a_dispatch_attempted(
        &self,
        run_id: &str,
        receipt: &crate::tool_execution_receipt::ToolExecutionReceipt,
    ) -> Result<()> {
        use crate::tool_execution_receipt::{
            ToolDispatchKind, ToolEffectStatus, ToolExecutionOutcome, ToolTransportStatus,
        };

        Self::validate_live_a2a_tool_receipt_identity(run_id, receipt)?;
        if receipt.finished_at.is_some()
            || receipt.dispatch_kind != ToolDispatchKind::NotAttempted
            || receipt.dispatch_attempt_count != 0
            || receipt.dispatch_observed
            || receipt.transport_status != ToolTransportStatus::NotAttempted
            || receipt.effect_status != ToolEffectStatus::NotAttempted
            || receipt.execution_outcome != ToolExecutionOutcome::NotObserved
        {
            anyhow::bail!("agent_run_a2a_dispatch_attempt_precondition_invalid");
        }
        let attempted_at = chrono::Utc::now().to_rfc3339();
        let mut conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let update_query = format!(
            "UPDATE agent_run_tool_executions
             SET state = 'dispatch_attempted',
                 revision = 1,
                 dispatch_kind = 'a2a',
                 dispatch_attempt_count = 1,
                 transport_status = 'dispatched',
                 effect_status = 'unknown',
                 execution_outcome = 'not_observed',
                 dispatch_attempted_at = ?1
             WHERE receipt_id = ?2
               AND run_id = ?3
               AND manifest_id = ?4
               AND request_digest = ?5
               AND state = 'prepared'
               AND revision = 0
               AND EXISTS (
                   SELECT 1 FROM agent_runs AS run
                   WHERE run.id = agent_run_tool_executions.run_id
                     AND run.status = 'running'
                     AND {LIVE_AGENT_RUN_SQL_PREDICATE}
               )"
        );
        let changed = tx.execute(
            &update_query,
            params![
                attempted_at,
                receipt.receipt_id,
                run_id,
                receipt.manifest_id,
                receipt.request_digest,
            ],
        )?;
        if changed != 1 {
            match Self::live_agent_run_status_on_connection(&tx, run_id)?.as_deref() {
                None => anyhow::bail!("agent_run_a2a_parent_inactive"),
                Some("running") => {}
                Some(_) => anyhow::bail!("agent_run_a2a_parent_not_running"),
            }
            anyhow::bail!("agent_run_a2a_dispatch_attempt_cas_failed");
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn mark_agent_run_a2a_response_observed(
        &self,
        run_id: &str,
        receipt: &crate::tool_execution_receipt::ToolExecutionReceipt,
    ) -> Result<()> {
        use crate::tool_execution_receipt::{ToolDispatchKind, ToolTransportStatus};

        Self::validate_live_a2a_tool_receipt_identity(run_id, receipt)?;
        if receipt.finished_at.is_some()
            || receipt.dispatch_kind != ToolDispatchKind::A2a
            || receipt.dispatch_attempt_count != 1
            || !receipt.dispatch_observed
            || receipt.transport_status != ToolTransportStatus::ResponseObserved
            || receipt.response_observed_at.is_none()
        {
            anyhow::bail!("agent_run_a2a_response_observed_state_invalid");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let update_query = format!(
            "UPDATE agent_run_tool_executions
             SET state = 'response_observed',
                 revision = 2,
                 dispatch_kind = 'a2a',
                 dispatch_attempt_count = 1,
                 transport_status = 'response_observed',
                 effect_status = 'unknown',
                 execution_outcome = 'not_observed',
                 response_observed_at = ?1
             WHERE receipt_id = ?2
               AND run_id = ?3
               AND manifest_id = ?4
               AND request_digest = ?5
               AND state = 'dispatch_attempted'
               AND revision = 1
               AND EXISTS (
                   SELECT 1 FROM agent_runs AS run
                   WHERE run.id = agent_run_tool_executions.run_id
                     AND run.status = 'running'
                     AND {LIVE_AGENT_RUN_SQL_PREDICATE}
               )"
        );
        let changed = tx.execute(
            &update_query,
            params![
                receipt
                    .response_observed_at
                    .context("agent_run_a2a_response_observed_timestamp_missing")?
                    .to_rfc3339(),
                receipt.receipt_id,
                run_id,
                receipt.manifest_id,
                receipt.request_digest,
            ],
        )?;
        if changed != 1 {
            match Self::live_agent_run_status_on_connection(&tx, run_id)?.as_deref() {
                None => anyhow::bail!("agent_run_a2a_parent_inactive"),
                Some("running") => {}
                Some(_) => anyhow::bail!("agent_run_a2a_parent_not_running"),
            }
            anyhow::bail!("agent_run_a2a_response_observed_cas_failed");
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn commit_agent_run_a2a_tool_terminal(
        &self,
        run_id: &str,
        result: &crate::agent::ActionExecutionResult,
    ) -> Result<()> {
        use crate::tool_execution_receipt::{
            ToolEffectStatus, ToolExecutionOutcome, ToolTransportStatus,
        };

        let receipt = &result.execution_receipt;
        Self::validate_live_a2a_tool_receipt_identity(run_id, receipt)?;
        receipt
            .mechanically_valid_terminal()
            .map_err(|reason| anyhow::anyhow!("agent_run_a2a_terminal_receipt_invalid:{reason}"))?;
        if receipt.execution_outcome == ToolExecutionOutcome::Failed
            && receipt.effect_status == ToolEffectStatus::Confirmed
        {
            anyhow::bail!("agent_run_a2a_failed_terminal_cannot_confirm_effect");
        }
        let (expected_state, expected_revision, terminal_state, terminal_revision) =
            if receipt.proves_success() {
                ("response_observed", 2, "terminal_succeeded", 3)
            } else if receipt.proves_not_dispatched() {
                ("prepared", 0, "terminal_not_attempted", 1)
            } else if receipt.transport_status == ToolTransportStatus::RemoteUnknown
                && receipt.execution_outcome == ToolExecutionOutcome::Unknown
                && receipt.effect_status == ToolEffectStatus::Unknown
            {
                ("dispatch_attempted", 1, "terminal_remote_unknown", 2)
            } else if receipt.transport_status == ToolTransportStatus::ResponseObserved
                && receipt.execution_outcome == ToolExecutionOutcome::Failed
                && receipt.effect_status == ToolEffectStatus::Unknown
            {
                ("response_observed", 2, "terminal_failed", 3)
            } else {
                anyhow::bail!("agent_run_a2a_terminal_disposition_unproven");
            };
        let finished_at = receipt
            .finished_at
            .context("agent_run_a2a_terminal_timestamp_missing")?
            .to_rfc3339();
        let mut run = self
            .get_run(run_id)?
            .context("agent_run_a2a_terminal_parent_missing")?;
        if run.status != AgentRunStatus::Running {
            anyhow::bail!("agent_run_a2a_terminal_parent_not_running");
        }
        run.actions.push(result.action.clone());
        run.observations.push(result.observation.clone());
        run.step_count = run.step_count.saturating_add(1);
        run.tool_call_count = run.tool_call_count.saturating_add(1);
        match result.status {
            crate::agent::ActionExecutionStatus::Succeeded => {
                run.status = AgentRunStatus::Completed;
                run.error = None;
                run.finished_at = receipt.finished_at;
            }
            crate::agent::ActionExecutionStatus::NeedsConfirmation => {
                run.status = AgentRunStatus::WaitingPermission;
                run.error = None;
                run.finished_at = None;
            }
            crate::agent::ActionExecutionStatus::Blocked
            | crate::agent::ActionExecutionStatus::Failed => {
                run.status = if receipt.transport_status == ToolTransportStatus::RemoteUnknown
                    || receipt.execution_outcome == ToolExecutionOutcome::Unknown
                {
                    AgentRunStatus::RemoteUnknown
                } else {
                    AgentRunStatus::Failed
                };
                run.error = Some(crate::agent::AgentRunError {
                    message: result
                        .stop_reason
                        .clone()
                        .or_else(|| result.action.error.clone())
                        .unwrap_or_else(|| "a2a_tool_execution_failed".into()),
                    phase: "tool".into(),
                    recoverable: false,
                });
                run.finished_at = receipt.finished_at;
            }
        }
        self.update_run_internal_with_transaction(&run, |tx| {
            let changed = tx.execute(
                "UPDATE agent_run_tool_executions
                 SET state = ?1,
                     revision = ?2,
                     dispatch_kind = ?3,
                     dispatch_attempt_count = ?4,
                     transport_status = ?5,
                     effect_status = ?6,
                     execution_outcome = ?7,
                     terminal_at = ?8
                 WHERE receipt_id = ?9
                   AND run_id = ?10
                   AND manifest_id = ?11
                   AND request_digest = ?12
                   AND state = ?13
                   AND revision = ?14",
                params![
                    terminal_state,
                    terminal_revision,
                    receipt.dispatch_kind.as_str(),
                    receipt.dispatch_attempt_count,
                    receipt.transport_status.as_str(),
                    receipt.effect_status.as_str(),
                    receipt.execution_outcome.as_str(),
                    finished_at,
                    receipt.receipt_id,
                    run_id,
                    receipt.manifest_id,
                    receipt.request_digest,
                    expected_state,
                    expected_revision,
                ],
            )?;
            if changed != 1 {
                anyhow::bail!("agent_run_a2a_terminal_cas_failed");
            }
            Ok(())
        })
    }

    pub(crate) fn get_agent_run_tool_execution(
        &self,
        run_id: &str,
        receipt_id: &str,
    ) -> Result<Option<crate::agent::AgentRunToolExecutionRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let query = format!(
            "SELECT child.run_id, child.receipt_id, child.manifest_id, child.request_digest,
                    child.endpoint_digest, child.action_effect, child.idempotency_contract,
                    child.dispatch_kind, child.state, child.revision,
                    child.dispatch_attempt_count, child.transport_status,
                    child.effect_status, child.execution_outcome, child.prepared_at,
                    child.dispatch_attempted_at, child.response_observed_at, child.terminal_at
             FROM agent_run_tool_executions AS child
             INNER JOIN agent_runs AS run ON run.id = child.run_id
             WHERE child.run_id = ?1 AND child.receipt_id = ?2
               AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let raw = conn
            .query_row(&query, params![run_id, receipt_id], |row| {
                Ok(RawAgentRunToolExecutionRecord {
                    run_id: row.get(0)?,
                    receipt_id: row.get(1)?,
                    manifest_id: row.get(2)?,
                    request_digest: row.get(3)?,
                    endpoint_digest: row.get(4)?,
                    action_effect: row.get(5)?,
                    idempotency_contract: row.get(6)?,
                    dispatch_kind: row.get(7)?,
                    state: row.get(8)?,
                    revision: row.get(9)?,
                    dispatch_attempt_count: row.get(10)?,
                    transport_status: row.get(11)?,
                    effect_status: row.get(12)?,
                    execution_outcome: row.get(13)?,
                    prepared_at: row.get(14)?,
                    dispatch_attempted_at: row.get(15)?,
                    response_observed_at: row.get(16)?,
                    terminal_at: row.get(17)?,
                })
            })
            .optional()?;
        raw.map(RawAgentRunToolExecutionRecord::into_typed)
            .transpose()
    }

    pub fn list_agent_run_tool_executions(
        &self,
        run_id: &str,
    ) -> Result<Vec<crate::agent::AgentRunToolExecutionRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let query = format!(
            "SELECT child.run_id, child.receipt_id, child.manifest_id, child.request_digest,
                    child.endpoint_digest, child.action_effect, child.idempotency_contract,
                    child.dispatch_kind, child.state, child.revision,
                    child.dispatch_attempt_count, child.transport_status,
                    child.effect_status, child.execution_outcome, child.prepared_at,
                    child.dispatch_attempted_at, child.response_observed_at, child.terminal_at
             FROM agent_run_tool_executions AS child
             INNER JOIN agent_runs AS run ON run.id = child.run_id
             WHERE child.run_id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}
             ORDER BY child.prepared_at, child.receipt_id"
        );
        let mut statement = conn.prepare(&query)?;
        let rows = statement
            .query_map([run_id], |row| {
                Ok(RawAgentRunToolExecutionRecord {
                    run_id: row.get(0)?,
                    receipt_id: row.get(1)?,
                    manifest_id: row.get(2)?,
                    request_digest: row.get(3)?,
                    endpoint_digest: row.get(4)?,
                    action_effect: row.get(5)?,
                    idempotency_contract: row.get(6)?,
                    dispatch_kind: row.get(7)?,
                    state: row.get(8)?,
                    revision: row.get(9)?,
                    dispatch_attempt_count: row.get(10)?,
                    transport_status: row.get(11)?,
                    effect_status: row.get(12)?,
                    execution_outcome: row.get(13)?,
                    prepared_at: row.get(14)?,
                    dispatch_attempted_at: row.get(15)?,
                    response_observed_at: row.get(16)?,
                    terminal_at: row.get(17)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(RawAgentRunToolExecutionRecord::into_typed)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn raw_agent_run_tool_execution_state_for_test(
        &self,
        run_id: &str,
        receipt_id: &str,
    ) -> Result<Option<crate::agent::AgentRunToolExecutionState>> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        conn.query_row(
            "SELECT state FROM agent_run_tool_executions
             WHERE run_id = ?1 AND receipt_id = ?2",
            params![run_id, receipt_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|state| crate::agent::AgentRunToolExecutionState::from_str(&state))
        .transpose()
    }

    fn reconcile_agent_run_tool_execution_orphans(
        conn: &Connection,
        receipt_key: &AgentRunReceiptKey,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let tx = conn.unchecked_transaction()?;
        let canonical_store_identity = Self::existing_canonical_store_identity(&tx)?;
        let recovery_rows = {
            let query = format!(
                "SELECT child.run_id, child.receipt_id, child.state,
                        child.transport_status, child.effect_status,
                        child.execution_outcome
                 FROM agent_run_tool_executions AS child
                 INNER JOIN agent_runs AS run ON run.id = child.run_id
                 WHERE {LIVE_AGENT_RUN_SQL_PREDICATE}
                   AND (
                     child.state IN ('prepared', 'dispatch_attempted', 'response_observed')
                    OR (child.state = 'terminal_succeeded' AND run.status != 'completed')
                    OR (child.state IN (
                            'terminal_failed', 'terminal_not_attempted'
                        ) AND run.status != 'failed')
                    OR (child.state = 'terminal_remote_unknown'
                        AND run.status != 'remote_unknown')
                   )
                 ORDER BY child.prepared_at, child.receipt_id"
            );
            let mut statement = tx.prepare(&query)?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };

        for (run_id, receipt_id, child_state, transport, effect, outcome) in recovery_rows {
            let parent_query = format!(
                "SELECT {AGENT_RUN_SELECT_COLUMNS}
                 FROM agent_runs AS run
                 WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
            );
            let mut run = tx
                .query_row(&parent_query, [&run_id], |row| {
                    Self::row_to_run(row, receipt_key, &canonical_store_identity)
                })
                .optional()?
                .context("agent_run_a2a_recovery_parent_missing")?;
            if run.id != run_id {
                anyhow::bail!("agent_run_a2a_recovery_parent_identity_mismatch");
            }
            let (recovered_child_state, parent_status, action_status, reason_code) =
                match child_state.as_str() {
                    "terminal_succeeded" => (
                        "terminal_succeeded",
                        AgentRunStatus::Completed,
                        "succeeded",
                        "a2a_terminal_success_parent_recovered",
                    ),
                    "prepared" | "terminal_not_attempted" => (
                        "terminal_not_attempted",
                        AgentRunStatus::Failed,
                        "failed",
                        "a2a_not_dispatched_parent_recovered",
                    ),
                    "dispatch_attempted" | "response_observed" | "terminal_remote_unknown" => (
                        "terminal_remote_unknown",
                        AgentRunStatus::RemoteUnknown,
                        "remote_unknown",
                        "a2a_remote_unknown_parent_recovered",
                    ),
                    "terminal_failed" => (
                        "terminal_failed",
                        AgentRunStatus::Failed,
                        "failed",
                        "a2a_terminal_failure_parent_recovered",
                    ),
                    _ => anyhow::bail!("agent_run_a2a_recovery_child_state_invalid"),
                };
            let previous_actions = run.actions.clone();
            let previous_observations = run.observations.clone();
            let recovered_at = chrono::Utc::now();
            run.actions.push(crate::agent::AgentAction {
                id: receipt_id.clone(),
                action_type: "a2a.call_agent".into(),
                target: Some("a2a.call_agent".into()),
                input: serde_json::json!({}),
                output: Some(serde_json::json!({
                    "recoveredFromDurableToolExecution": true,
                    "receiptRef": receipt_id,
                    "childState": recovered_child_state,
                    "transportStatus": transport,
                    "effectStatus": effect,
                    "executionOutcome": outcome,
                })),
                status: action_status.into(),
                permission_decision: None,
                started_at: None,
                finished_at: Some(recovered_at),
                error: matches!(
                    parent_status,
                    AgentRunStatus::Failed | AgentRunStatus::RemoteUnknown
                )
                .then(|| reason_code.to_string()),
                timestamp: recovered_at,
                tool_scope: None,
                react_trace: None,
                runtime_execution_receipt: None,
            });
            run.observations.push(crate::agent::AgentObservation {
                id: uuid::Uuid::new_v4().to_string(),
                action_id: Some(receipt_id.clone()),
                content: reason_code.into(),
                source: "agent_run_tool_execution_recovery".into(),
                structured_result: Some(serde_json::json!({
                    "receiptRef": receipt_id,
                    "state": recovered_child_state,
                    "authoritativeSource": "agent_run_tool_executions",
                })),
                timestamp: recovered_at,
                react_trace: None,
            });
            run.status = parent_status;
            run.step_count = run.step_count.saturating_add(1);
            run.tool_call_count = run.tool_call_count.saturating_add(1);
            run.finished_at = Some(recovered_at);
            run.error = matches!(
                parent_status,
                AgentRunStatus::Failed | AgentRunStatus::RemoteUnknown
            )
            .then(|| crate::agent::AgentRunError {
                message: reason_code.into(),
                phase: "startup_projection_recovery".into(),
                recoverable: false,
            });
            let attach_time = recovered_at.timestamp();
            let (actions, observations, pending_attachments) = canonicalize_execution_records(
                &tx,
                &canonical_store_identity,
                attach_time,
                &run.id,
                &run.actions,
                &run.observations,
                Some(&previous_actions),
                Some(&previous_observations),
                receipt_key,
            )?;
            let actions_json = serde_json::to_string(&actions)?;
            let observations_json = serde_json::to_string(&observations)?;
            let error_json = run
                .error
                .as_ref()
                .map(|error| minimize_error(error, ReceiptOrigin::NewInput(receipt_key)))
                .map(|error| serde_json::to_string(&error))
                .transpose()?;
            let recovery_update_query = format!(
                "UPDATE agent_runs AS run
                 SET status = ?2,
                     actions_json = ?3,
                     observations_json = ?4,
                     error_json = ?5,
                     step_count = ?6,
                     tool_call_count = ?7,
                     finished_at = ?8
                 WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
            );
            let changed = tx.execute(
                &recovery_update_query,
                params![
                    run.id,
                    run.status.to_string(),
                    actions_json,
                    observations_json,
                    error_json,
                    run.step_count,
                    run.tool_call_count,
                    recovered_at.to_rfc3339(),
                ],
            )?;
            if changed != 1 {
                anyhow::bail!("agent_run_a2a_recovery_parent_update_failed");
            }
            Self::attach_bound_content_issuances(
                &tx,
                &canonical_store_identity,
                &run.id,
                &pending_attachments,
                attach_time,
            )?;
        }
        tx.execute(
            "UPDATE agent_run_tool_executions
             SET state = 'terminal_not_attempted',
                 revision = revision + 1,
                 transport_status = 'not_attempted',
                 effect_status = 'not_attempted',
                 execution_outcome = 'failed',
                 terminal_at = ?1
             WHERE state = 'prepared'",
            [&now],
        )?;
        tx.execute(
            "UPDATE agent_run_tool_executions
             SET state = 'terminal_remote_unknown',
                 revision = revision + 1,
                 transport_status = 'remote_unknown',
                 effect_status = 'unknown',
                 execution_outcome = 'unknown',
                 terminal_at = ?1
             WHERE state IN ('dispatch_attempted', 'response_observed')",
            [&now],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Reconcile an execution that failed after durable prepare but before
    /// the atomic terminal commit returned. The same routine runs on reopen;
    /// exposing it here prevents a still-live product session from presenting
    /// a child-less or stale Running parent until the next process start.
    #[doc(hidden)]
    pub fn reconcile_agent_run_tool_execution_owner_now(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        Self::reconcile_agent_run_tool_execution_orphans(&conn, self.receipt_key.as_ref())
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn install_tool_execution_fault_for_test(
        &self,
        point: crate::agent::AgentRunToolExecutionFaultPoint,
    ) -> Result<()> {
        use crate::agent::AgentRunToolExecutionFaultPoint;

        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS fail_agent_run_tool_execution_prepare_for_test;
             DROP TRIGGER IF EXISTS fail_agent_run_tool_execution_transition_for_test;
             DROP TRIGGER IF EXISTS fail_agent_run_bound_content_issue_for_test;
             DROP TRIGGER IF EXISTS fail_agent_run_update_for_tool_execution_test;",
        )?;
        let trigger = match point {
            AgentRunToolExecutionFaultPoint::Prepare => {
                "CREATE TEMP TRIGGER fail_agent_run_tool_execution_prepare_for_test
                 BEFORE INSERT ON agent_run_tool_executions
                 BEGIN
                    SELECT RAISE(ABORT, 'injected tool execution prepare failure');
                 END;"
            }
            AgentRunToolExecutionFaultPoint::DispatchAttempted => {
                "CREATE TEMP TRIGGER fail_agent_run_tool_execution_transition_for_test
                 BEFORE UPDATE ON agent_run_tool_executions
                 WHEN NEW.state = 'dispatch_attempted'
                 BEGIN
                    SELECT RAISE(ABORT, 'injected dispatch attempt failure');
                 END;"
            }
            AgentRunToolExecutionFaultPoint::ResponseObserved => {
                "CREATE TEMP TRIGGER fail_agent_run_tool_execution_transition_for_test
                 BEFORE UPDATE ON agent_run_tool_executions
                 WHEN NEW.state = 'response_observed'
                 BEGIN
                    SELECT RAISE(ABORT, 'injected response observed failure');
                 END;"
            }
            AgentRunToolExecutionFaultPoint::Terminal => {
                "CREATE TEMP TRIGGER fail_agent_run_tool_execution_transition_for_test
                 BEFORE UPDATE ON agent_run_tool_executions
                 WHEN NEW.state LIKE 'terminal_%'
                 BEGIN
                    SELECT RAISE(ABORT, 'injected tool execution terminal failure');
                 END;"
            }
            AgentRunToolExecutionFaultPoint::BoundContentReceiptIssuance => {
                "CREATE TEMP TRIGGER fail_agent_run_bound_content_issue_for_test
                 BEFORE INSERT ON bound_content_issuance_ledger
                 BEGIN
                    SELECT RAISE(ABORT, 'injected bound content receipt failure');
                 END;"
            }
            AgentRunToolExecutionFaultPoint::AgentRunUpdate => {
                "CREATE TEMP TRIGGER fail_agent_run_update_for_tool_execution_test
                 BEFORE UPDATE ON agent_runs
                 BEGIN
                    SELECT RAISE(ABORT, 'injected AgentRun update failure');
                 END;"
            }
        };
        conn.execute_batch(trigger)?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn install_terminal_succeeded_parent_drift_for_test(
        &self,
        run_id: &str,
        receipt_id: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let changed = conn.execute(
            "UPDATE agent_run_tool_executions
             SET state = 'terminal_succeeded',
                 revision = 3,
                 dispatch_kind = 'a2a',
                 dispatch_attempt_count = 1,
                 transport_status = 'response_observed',
                 effect_status = 'confirmed',
                 execution_outcome = 'succeeded',
                 dispatch_attempted_at = ?1,
                 response_observed_at = ?1,
                 terminal_at = ?1
             WHERE run_id = ?2 AND receipt_id = ?3 AND state = 'prepared'",
            params![now, run_id, receipt_id],
        )?;
        if changed != 1 {
            anyhow::bail!("inject_terminal_succeeded_parent_drift_failed");
        }
        Ok(())
    }

    fn replace_proposal_links(
        tx: &rusqlite::Transaction<'_>,
        run_id: &str,
        proposal_ids: &[String],
    ) -> Result<()> {
        let proposal_ids = Self::normalize_proposal_ids(run_id, proposal_ids)?;
        tx.execute(
            "DELETE FROM agent_run_proposal_links WHERE run_id = ?1",
            [run_id],
        )?;
        for proposal_id in proposal_ids {
            tx.execute(
                "INSERT INTO agent_run_proposal_links (run_id, proposal_id)
                 VALUES (?1, ?2)",
                params![run_id, proposal_id],
            )?;
        }
        Ok(())
    }

    fn normalize_proposal_ids(run_id: &str, proposal_ids: &[String]) -> Result<Vec<String>> {
        if proposal_ids.len() > MAX_AGENT_RUN_PROPOSAL_LINKS {
            anyhow::bail!(
                "AgentRun proposal reference limit exceeded for run {run_id}: {} > {}",
                proposal_ids.len(),
                MAX_AGENT_RUN_PROPOSAL_LINKS
            );
        }
        let mut normalized = Vec::with_capacity(proposal_ids.len());
        let mut unique = std::collections::HashSet::new();
        for proposal_id in proposal_ids {
            if proposal_id.is_empty()
                || proposal_id.len() > 192
                || proposal_id.trim() != proposal_id
                || proposal_id
                    .chars()
                    .any(|character| character.is_control() || character.is_whitespace())
            {
                anyhow::bail!("invalid AgentRun proposal reference for run {run_id}");
            }
            #[cfg(not(any(test, feature = "test-utils")))]
            uuid::Uuid::parse_str(proposal_id)
                .with_context(|| "agent_run_proposal_reference_must_be_uuid")?;
            if unique.insert(proposal_id.clone()) {
                normalized.push(proposal_id.clone());
            }
        }
        Ok(normalized)
    }

    fn minimize_legacy_run_payloads(
        conn: &Connection,
        receipt_key: &AgentRunReceiptKey,
    ) -> Result<()> {
        let tx = conn.unchecked_transaction()?;
        let legacy = {
            let mut stmt = tx.prepare(
                "SELECT id, task_id, status, kind, session_id, user_input, input_ref, input_digest,
                        context_summary_json, model_route_json, output_preview, error_json,
                        generated_proposals_json, actions_json, observations_json,
                        reasoning_strategy, reasoning_trace_json, reasoning_trace_digest,
                        hs_selection_audit_json, behavior_checks_json, status_updates_json,
                        delete_reason, payload_minimized_version
                 FROM agent_runs
                 WHERE payload_minimized_version < ?1",
            )?;
            let rows = stmt.query_map([AGENT_RUN_PAYLOAD_VERSION], |row| {
                Ok(LegacyAgentRunPayload {
                    run_id: row.get(0)?,
                    task_id: row.get(1)?,
                    status: row.get(2)?,
                    kind: row.get(3)?,
                    session_id: row.get(4)?,
                    user_input: row.get(5)?,
                    input_ref: row.get(6)?,
                    _input_digest: row.get(7)?,
                    context_summary_json: row.get(8)?,
                    model_route_json: row.get(9)?,
                    output_preview: row.get(10)?,
                    error_json: row.get(11)?,
                    generated_proposals_json: row.get(12)?,
                    actions_json: row.get(13)?,
                    observations_json: row.get(14)?,
                    reasoning_strategy: row.get(15)?,
                    reasoning_trace_json: row.get(16)?,
                    _reasoning_trace_digest: row.get(17)?,
                    hs_selection_audit_json: row.get(18)?,
                    behavior_checks_json: row.get(19)?,
                    status_updates_json: row.get(20)?,
                    delete_reason: row.get(21)?,
                    payload_minimized_version: row.get(22)?,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        for legacy in legacy {
            if !(0..AGENT_RUN_PAYLOAD_VERSION).contains(&legacy.payload_minimized_version) {
                anyhow::bail!(
                    "invalid legacy AgentRun payload version {} for {}",
                    legacy.payload_minimized_version,
                    legacy.run_id
                );
            }
            // No historical marker authenticates who produced a receipt-shaped
            // value. Re-minimize every pre-v7 value deterministically and mark
            // the row unverified instead of promoting version metadata to
            // provenance.
            let legacy_origin = ReceiptOrigin::NewInput(receipt_key);
            normalized_identity_reference(&legacy.run_id, "run_id", &legacy.run_id)?;
            normalized_identity_reference(&legacy.run_id, "task_id", &legacy.task_id)?;
            if let Some(session_id) = legacy.session_id.as_deref() {
                normalized_identity_reference(&legacy.run_id, "session_id", session_id)?;
            }
            if !is_agent_run_status(&legacy.status) {
                anyhow::bail!("invalid status enum for legacy AgentRun {}", legacy.run_id);
            }
            if !is_agent_task_kind(&legacy.kind) {
                anyhow::bail!("invalid kind enum for legacy AgentRun {}", legacy.run_id);
            }

            // Historical rows do not carry the non-serde MemoryStore proof.
            // Never preserve a resolvable-looking conversation URI solely
            // because its string shape is valid.
            let existing_input_ref = legacy
                .input_ref
                .as_deref()
                .filter(|reference| is_explicit_legacy_unresolvable_ref(reference))
                .map(str::to_string);
            let (input_ref, input_digest) = if let Some(user_input) = legacy.user_input.as_deref() {
                // Pre-v6 SHA-shaped fields were enumerable caller data, not
                // authority. Recompute from the body while it is still present
                // and never promote the old digest claim.
                let computed_digest =
                    scoped_run_content_digest(receipt_key, &legacy.run_id, "run_input", user_input);
                (
                    existing_input_ref.or_else(|| Some(derived_legacy_input_ref(&computed_digest))),
                    Some(computed_digest),
                )
            } else {
                let input_ref = existing_input_ref.or_else(|| {
                    legacy.input_ref.as_deref().map(|reference| {
                        let reference_digest =
                            metadata_digest(receipt_key, "legacy_input_reference", reference);
                        derived_legacy_input_ref(&reference_digest)
                    })
                });
                // A digest without the original body or an opaque canonical
                // proof has no semantic provenance. The row-level legacy flag
                // remains available to product readers, but the unsupported
                // digest claim itself is removed.
                (input_ref, None)
            };

            let context_summary_json =
                parse_optional_legacy_json::<crate::agent::types::ContextSummary>(
                    &legacy.run_id,
                    "context_summary_json",
                    legacy.context_summary_json.as_deref(),
                )?
                .map(|summary| {
                    serde_json::to_string(&minimize_context_summary(&summary, legacy_origin))
                })
                .transpose()
                .context("failed to serialize minimized legacy AgentRun context summary")?;
            let model_route_json =
                parse_optional_legacy_json::<crate::agent::types::ModelRouteTrace>(
                    &legacy.run_id,
                    "model_route_json",
                    legacy.model_route_json.as_deref(),
                )?
                .map(|route| serde_json::to_string(&minimize_model_route(&route, legacy_origin)))
                .transpose()
                .context("failed to serialize minimized legacy AgentRun model route")?;
            let output_preview = legacy
                .output_preview
                .as_deref()
                .map(|value| metadata_safe_text_receipt("run_output", value, legacy_origin));
            let error_json = parse_optional_legacy_json::<crate::agent::types::AgentRunError>(
                &legacy.run_id,
                "error_json",
                legacy.error_json.as_deref(),
            )?
            .map(|error| serde_json::to_string(&minimize_error(&error, legacy_origin)))
            .transpose()
            .context("failed to serialize minimized legacy AgentRun error")?;
            let proposal_ids = parse_legacy_json_array::<String>(
                &legacy.run_id,
                "generated_proposals_json",
                legacy.generated_proposals_json.as_deref(),
            )?;
            let proposal_ids = Self::normalize_proposal_ids(&legacy.run_id, &proposal_ids)?;
            let generated_proposals_json = serde_json::to_string(&proposal_ids)
                .context("failed to serialize legacy AgentRun proposal references")?;
            let actions = parse_legacy_trace_array::<crate::agent::types::AgentAction>(
                &legacy.run_id,
                "actions_json",
                legacy.actions_json.as_deref(),
            )?;
            if actions.len() > MAX_AGENT_RUN_COLLECTION_ITEMS {
                anyhow::bail!("legacy_agent_run_actions_limit_exceeded:{}", legacy.run_id);
            }
            let actions_json = minimized_actions_json_with_origin(&actions, legacy_origin)?;
            let observations = parse_legacy_trace_array::<crate::agent::types::AgentObservation>(
                &legacy.run_id,
                "observations_json",
                legacy.observations_json.as_deref(),
            )?;
            if observations.len() > MAX_AGENT_RUN_COLLECTION_ITEMS {
                anyhow::bail!(
                    "legacy_agent_run_observations_limit_exceeded:{}",
                    legacy.run_id
                );
            }
            let observations_json =
                minimized_observations_json_with_origin(&observations, legacy_origin)?;
            let reasoning_strategy = legacy.reasoning_strategy.as_deref().map(|value| {
                metadata_safe_enum_or_receipt(
                    "reasoning_strategy",
                    value,
                    &["layered", "direct", "react", "plan_execute", "unknown"],
                    legacy_origin,
                )
            });
            let reasoning_trace_digest =
                if let Some(reasoning_trace) = legacy.reasoning_trace_json.as_deref() {
                    if !reasoning_trace.trim().is_empty() {
                        serde_json::from_str::<serde_json::Value>(reasoning_trace).with_context(
                            || {
                                format!(
                                    "invalid non-empty reasoning_trace_json for legacy AgentRun {}",
                                    legacy.run_id
                                )
                            },
                        )?;
                        let computed_digest = scoped_run_content_digest(
                            receipt_key,
                            &legacy.run_id,
                            "reasoning_trace",
                            reasoning_trace,
                        );
                        Some(computed_digest)
                    } else {
                        None
                    }
                } else {
                    None
                };
            let hs_selection_audit_json = parse_optional_legacy_json::<
                crate::agent::hs_selector::HSSelectionAudit,
            >(
                &legacy.run_id,
                "hs_selection_audit_json",
                legacy.hs_selection_audit_json.as_deref(),
            )?
            .map(|audit| serde_json::to_string(&minimize_hs_selection_audit(&audit, legacy_origin)))
            .transpose()
            .context("failed to serialize minimized legacy AgentRun HS selection audit")?;
            let behavior_checks =
                parse_legacy_json_array::<crate::agent::types::HSBehaviorCheckSummary>(
                    &legacy.run_id,
                    "behavior_checks_json",
                    legacy.behavior_checks_json.as_deref(),
                )?;
            if behavior_checks.len() > MAX_AGENT_RUN_COLLECTION_ITEMS {
                anyhow::bail!(
                    "legacy_agent_run_behavior_checks_limit_exceeded:{}",
                    legacy.run_id
                );
            }
            let behavior_checks_json =
                minimized_behavior_checks_json_with_origin(&behavior_checks, legacy_origin)?;
            let status_updates =
                parse_legacy_json_array::<crate::agent::types::AgentLoopStatusUpdate>(
                    &legacy.run_id,
                    "status_updates_json",
                    legacy.status_updates_json.as_deref(),
                )?;
            if status_updates.len() > MAX_AGENT_RUN_COLLECTION_ITEMS {
                anyhow::bail!(
                    "legacy_agent_run_status_updates_limit_exceeded:{}",
                    legacy.run_id
                );
            }
            let status_updates_json =
                minimized_status_updates_json_with_origin(&status_updates, legacy_origin)?;
            let delete_reason = legacy
                .delete_reason
                .as_deref()
                .map(|value| metadata_safe_text_receipt("delete_reason", value, legacy_origin));
            ensure_minimized_payload_bounds(
                &legacy.run_id,
                [
                    context_summary_json.as_deref(),
                    model_route_json.as_deref(),
                    output_preview.as_deref(),
                    error_json.as_deref(),
                    Some(generated_proposals_json.as_str()),
                    Some(actions_json.as_str()),
                    Some(observations_json.as_str()),
                    reasoning_strategy.as_deref(),
                    hs_selection_audit_json.as_deref(),
                    Some(behavior_checks_json.as_str()),
                    Some(status_updates_json.as_str()),
                    delete_reason.as_deref(),
                ],
            )?;
            let changed = tx.execute(
                "UPDATE agent_runs
                 SET user_input = NULL,
                     reasoning_trace_json = NULL,
                     input_ref = ?2,
                     input_digest = ?3,
                     context_summary_json = ?4,
                     model_route_json = ?5,
                     output_preview = ?6,
                     error_json = ?7,
                     generated_proposals_json = ?8,
                     actions_json = ?9,
                     observations_json = ?10,
                     reasoning_strategy = ?11,
                     reasoning_trace_digest = ?12,
                     hs_selection_audit_json = ?13,
                     behavior_checks_json = ?14,
                     status_updates_json = ?15,
                     delete_reason = ?16,
                     payload_minimized_version = ?17,
                     legacy_payload_unverified = 1
                 WHERE id = ?1",
                params![
                    legacy.run_id,
                    input_ref,
                    input_digest,
                    context_summary_json,
                    model_route_json,
                    output_preview,
                    error_json,
                    generated_proposals_json,
                    actions_json,
                    observations_json,
                    reasoning_strategy,
                    reasoning_trace_digest,
                    hs_selection_audit_json,
                    behavior_checks_json,
                    status_updates_json,
                    delete_reason,
                    AGENT_RUN_PAYLOAD_VERSION,
                ],
            )?;
            if changed != 1 {
                anyhow::bail!("legacy AgentRun disappeared during payload migration");
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn validate_current_run_identity_domain(conn: &Connection) -> Result<()> {
        let mut statement = conn.prepare(
            "SELECT id, task_id, session_id, status, kind, input_ref,
                    payload_minimized_version, legacy_payload_unverified
             FROM agent_runs",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let run_id = row.get::<_, String>(0)?;
            let task_id = row.get::<_, String>(1)?;
            let session_id = row.get::<_, Option<String>>(2)?;
            let status = row.get::<_, String>(3)?;
            let kind = row.get::<_, String>(4)?;
            let input_ref = row.get::<_, Option<String>>(5)?;
            let version = row.get::<_, i64>(6)?;
            let legacy_payload_unverified = row.get::<_, i64>(7)?;
            if version != AGENT_RUN_PAYLOAD_VERSION {
                anyhow::bail!("agent_run_payload_version_unsupported:{version}");
            }
            if !matches!(legacy_payload_unverified, 0 | 1) {
                anyhow::bail!("agent_run_legacy_payload_flag_invalid:{run_id}");
            }
            normalized_identity_reference(&run_id, "run_id", &run_id)?;
            normalized_identity_reference(&run_id, "task_id", &task_id)?;
            if let Some(session_id) = session_id.as_deref() {
                normalized_identity_reference(&run_id, "session_id", session_id)?;
            }
            if !is_agent_run_status(&status) || !is_agent_task_kind(&kind) {
                anyhow::bail!("agent_run_typed_identity_invalid:{run_id}");
            }
            normalized_input_reference(&run_id, input_ref.as_deref())?;
            if !input_reference_matches_session(input_ref.as_deref(), session_id.as_deref()) {
                anyhow::bail!("agent_run_input_ref_session_mismatch:{run_id}");
            }
        }
        Ok(())
    }

    fn is_valid_sqlite_identifier(ident: &str) -> bool {
        if ident.is_empty() {
            return false;
        }
        let bytes = ident.as_bytes();
        // Must start with letter or underscore
        if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_' {
            return false;
        }
        // Rest must be alphanumeric or underscore
        bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
    }

    fn add_column_if_missing(
        conn: &Connection,
        table: &str,
        column: &str,
        definition: &str,
    ) -> Result<()> {
        if !Self::is_valid_sqlite_identifier(table) {
            return Err(anyhow::anyhow!(
                "Invalid table name for migration: {}",
                table
            ));
        }
        if !Self::is_valid_sqlite_identifier(column) {
            return Err(anyhow::anyhow!(
                "Invalid column name for migration: {}",
                column
            ));
        }
        // Definition is validated to be a simple type definition (e.g., "TEXT NOT NULL DEFAULT ''")
        // We allow spaces, commas, parentheses for type definitions
        if definition.is_empty() {
            return Err(anyhow::anyhow!("Column definition cannot be empty"));
        }

        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for col in columns {
            if col? == column {
                return Ok(());
            }
        }
        conn.execute(
            &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, definition),
            [],
        )?;
        Ok(())
    }

    fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
        if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            || !column
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            anyhow::bail!("invalid SQLite identifier");
        }
        let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
        for existing in columns {
            if existing? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn install_agent_run_revision_authority(conn: &Connection) -> Result<()> {
        // The earlier D010 draft table was never a shipped authority and
        // derived revision from content equality, which is ABA-unsafe. Remove
        // it before installing the canonical mutation clock.
        conn.execute_batch(
            "DROP TABLE IF EXISTS agent_run_life_event_owner_revisions;
             CREATE TABLE IF NOT EXISTS agent_run_canonical_revisions (
                run_id TEXT PRIMARY KEY,
                revision INTEGER NOT NULL CHECK(revision >= 1)
             ) WITHOUT ROWID;
             INSERT INTO agent_run_canonical_revisions(run_id, revision)
             SELECT id, 1 FROM agent_runs
              WHERE id NOT IN (SELECT run_id FROM agent_run_canonical_revisions);
             DROP TRIGGER IF EXISTS trg_agent_run_canonical_revision_insert;
             DROP TRIGGER IF EXISTS trg_agent_run_canonical_revision_update;
             DROP TRIGGER IF EXISTS trg_agent_run_canonical_revision_delete;
             CREATE TRIGGER trg_agent_run_canonical_revision_insert
             AFTER INSERT ON agent_runs
             BEGIN
                SELECT CASE WHEN COALESCE((
                    SELECT revision FROM agent_run_canonical_revisions WHERE run_id = NEW.id
                ), 0) >= 9223372036854775807
                THEN RAISE(ABORT, 'agent_run_canonical_revision_overflow') END;
                INSERT INTO agent_run_canonical_revisions(run_id, revision)
                VALUES (NEW.id, 1)
                ON CONFLICT(run_id) DO UPDATE SET revision = revision + 1;
             END;
             CREATE TRIGGER trg_agent_run_canonical_revision_update
             AFTER UPDATE ON agent_runs
             BEGIN
                SELECT CASE WHEN (
                    SELECT revision FROM agent_run_canonical_revisions WHERE run_id = NEW.id
                ) >= 9223372036854775807
                THEN RAISE(ABORT, 'agent_run_canonical_revision_overflow') END;
                UPDATE agent_run_canonical_revisions
                   SET revision = revision + 1
                 WHERE run_id = NEW.id;
             END;
             CREATE TRIGGER trg_agent_run_canonical_revision_delete
             AFTER DELETE ON agent_runs
             BEGIN
                SELECT CASE WHEN (
                    SELECT revision FROM agent_run_canonical_revisions WHERE run_id = OLD.id
                ) >= 9223372036854775807
                THEN RAISE(ABORT, 'agent_run_canonical_revision_overflow') END;
                UPDATE agent_run_canonical_revisions
                   SET revision = revision + 1
                 WHERE run_id = OLD.id;
             END;",
        )?;
        let missing: i64 = conn.query_row(
            "SELECT COUNT(*) FROM agent_runs run
             LEFT JOIN agent_run_canonical_revisions revision ON revision.run_id = run.id
             WHERE revision.run_id IS NULL OR revision.revision < 1",
            [],
            |row| row.get(0),
        )?;
        if missing != 0 {
            anyhow::bail!("agent_run_canonical_revision_authority_incomplete");
        }
        Ok(())
    }

    fn agent_run_v7_physical_purge_complete(conn: &Connection) -> Result<bool> {
        let marker = conn
            .query_row(
                "SELECT value FROM agent_run_store_metadata WHERE key = ?1",
                [AGENT_RUN_V7_PHYSICAL_PURGE_MARKER],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(marker.as_deref() == Some("complete")
            && !Self::column_exists(conn, "agent_runs", "user_input")?
            && !Self::column_exists(conn, "agent_runs", "reasoning_trace_json")?)
    }

    fn checkpoint_agent_run_wal(conn: &Connection) -> Result<()> {
        let (busy, log_frames, checkpointed_frames): (i64, i64, i64) =
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
        if busy != 0 || log_frames != 0 || checkpointed_frames != 0 {
            anyhow::bail!(
                "agent_run_v7_wal_checkpoint_incomplete:{busy}:{log_frames}:{checkpointed_frames}"
            );
        }
        Ok(())
    }

    /// Completes the non-transactional physical half of the v7 migration.
    ///
    /// SQLite cannot include checkpoint/VACUUM in the atomic table-swap
    /// transaction. The marker is therefore written only after the old pages
    /// have been reclaimed and the WAL has been verified empty. A crash before
    /// this final write leaves the marker missing/pending, so the next writable
    /// startup repeats this idempotent purge before serving the store.
    fn complete_agent_run_v7_physical_purge(conn: &Connection) -> Result<()> {
        if Self::column_exists(conn, "agent_runs", "user_input")?
            || Self::column_exists(conn, "agent_runs", "reasoning_trace_json")?
        {
            anyhow::bail!("agent_run_v7_raw_column_rebuild_incomplete");
        }

        let database_path: String = conn.query_row(
            "SELECT file FROM pragma_database_list WHERE name = 'main'",
            [],
            |row| row.get(0),
        )?;
        if database_path.is_empty() {
            conn.execute_batch("VACUUM;")?;
        } else {
            Self::checkpoint_agent_run_wal(conn)?;
            conn.execute_batch("VACUUM;")?;
            Self::checkpoint_agent_run_wal(conn)?;

            let wal_path = PathBuf::from(format!("{database_path}-wal"));
            if wal_path.exists() && std::fs::metadata(&wal_path)?.len() != 0 {
                anyhow::bail!("agent_run_v7_wal_not_truncated");
            }
        }

        let freelist_count: i64 = conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
        if freelist_count != 0 {
            anyhow::bail!("agent_run_v7_freelist_not_reclaimed");
        }

        conn.execute(
            "INSERT INTO agent_run_store_metadata(key, value)
             VALUES (?1, 'complete')
             ON CONFLICT(key) DO UPDATE SET value = 'complete'",
            [AGENT_RUN_V7_PHYSICAL_PURGE_MARKER],
        )?;
        Ok(())
    }

    /// One-way v7 rebuild. Merely NULLing historical raw columns leaves their
    /// values recoverable from SQLite pages/WAL and leaves a future write route
    /// in the release schema. This transaction copies only the canonical
    /// metadata columns, then atomically replaces the table. secure_delete,
    /// checkpoint and VACUUM remove the retired pages after commit.
    fn rebuild_agent_runs_without_raw_columns(
        conn: &Connection,
        fault: AgentRunTableRebuildFault,
    ) -> Result<()> {
        if !Self::column_exists(conn, "agent_runs", "user_input")?
            && !Self::column_exists(conn, "agent_runs", "reasoning_trace_json")?
        {
            if !Self::agent_run_v7_physical_purge_complete(conn)? {
                Self::complete_agent_run_v7_physical_purge(conn)?;
            }
            return Ok(());
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(
            "DROP TABLE IF EXISTS agent_runs_v7_rebuild;
             CREATE TABLE agent_runs_v7_rebuild (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                session_id TEXT,
                status TEXT NOT NULL,
                kind TEXT NOT NULL,
                context_summary_json TEXT,
                model_route_json TEXT,
                output_preview TEXT,
                error_json TEXT,
                generated_proposals_json TEXT DEFAULT '[]',
                actions_json TEXT DEFAULT '[]',
                observations_json TEXT DEFAULT '[]',
                reasoning_strategy TEXT,
                hs_selection_audit_json TEXT,
                behavior_checks_json TEXT DEFAULT '[]',
                status_updates_json TEXT NOT NULL DEFAULT '[]',
                step_count INTEGER NOT NULL DEFAULT 0,
                tool_call_count INTEGER NOT NULL DEFAULT 0,
                deleted_at TEXT,
                delete_reason TEXT,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                input_ref TEXT,
                input_digest TEXT,
                reasoning_trace_digest TEXT,
                payload_minimized_version INTEGER NOT NULL,
                legacy_payload_unverified INTEGER NOT NULL DEFAULT 0
                    CHECK(legacy_payload_unverified IN (0, 1))
             );
             INSERT INTO agent_runs_v7_rebuild (
                id, task_id, session_id, status, kind,
                context_summary_json, model_route_json, output_preview, error_json,
                generated_proposals_json, actions_json, observations_json,
                reasoning_strategy, hs_selection_audit_json, behavior_checks_json,
                status_updates_json, step_count, tool_call_count, deleted_at,
                delete_reason, started_at, finished_at, input_ref, input_digest,
                reasoning_trace_digest, payload_minimized_version,
                legacy_payload_unverified
             )
             SELECT id, task_id, session_id, status, kind,
                    context_summary_json, model_route_json, output_preview, error_json,
                    generated_proposals_json, actions_json, observations_json,
                    reasoning_strategy, hs_selection_audit_json, behavior_checks_json,
                    status_updates_json, step_count, tool_call_count, deleted_at,
                    delete_reason, started_at, finished_at, input_ref, input_digest,
                    reasoning_trace_digest, payload_minimized_version,
                    legacy_payload_unverified
             FROM agent_runs;",
        )?;
        #[cfg(test)]
        if fault == AgentRunTableRebuildFault::AfterCopy {
            anyhow::bail!("injected_agent_run_v7_rebuild_failure_after_copy");
        }
        #[cfg(not(test))]
        let _ = fault;
        tx.execute(
            "INSERT INTO agent_run_store_metadata(key, value)
             VALUES (?1, 'pending')
             ON CONFLICT(key) DO UPDATE SET value = 'pending'",
            [AGENT_RUN_V7_PHYSICAL_PURGE_MARKER],
        )?;
        tx.execute_batch(
            "DROP TABLE agent_runs;
             ALTER TABLE agent_runs_v7_rebuild RENAME TO agent_runs;",
        )?;
        tx.commit()?;

        #[cfg(test)]
        if fault == AgentRunTableRebuildFault::AfterTableSwapBeforePurge {
            anyhow::bail!("injected_agent_run_v7_rebuild_failure_before_physical_purge");
        }

        Self::complete_agent_run_v7_physical_purge(conn)
    }

    /// Creates a run that has no canonical conversation reference.  A caller
    /// cannot turn a receipt-shaped string into canonical ownership through
    /// this general entrypoint.
    pub fn create_run(&self, run: &AgentRun) -> Result<()> {
        if run.input_ref.is_some() {
            anyhow::bail!("agent_run_canonical_input_proof_required: {}", run.id);
        }
        self.create_run_internal(run, None)
    }

    /// Binds this execution store to the one canonical conversation owner.
    /// The binding is durable and immutable: a copied proof from another
    /// MemoryStore cannot be accepted after restart or configuration drift.
    pub fn bind_canonical_memory_store(
        &self,
        memory_store: &crate::memory::MemoryStore,
    ) -> Result<()> {
        let expected = memory_store.canonical_store_identity();
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let existing = tx
            .query_row(
                "SELECT value FROM agent_run_store_metadata
                 WHERE key = 'canonical_memory_store_identity'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match existing {
            Some(existing) if existing != expected => {
                anyhow::bail!("agent_run_canonical_memory_store_identity_conflict")
            }
            Some(_) => {}
            None => {
                tx.execute(
                    "INSERT INTO agent_run_store_metadata(key, value)
                     VALUES ('canonical_memory_store_identity', ?1)",
                    [expected],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Creates a run whose input is bound to the canonical conversation row
    /// committed by MemoryStore.  `proof` is non-serde and has private fields,
    /// so IPC/caller-shaped receipts cannot satisfy this boundary.
    pub(crate) fn create_run_with_input_proof(
        &self,
        run: &AgentRun,
        proof: &CanonicalConversationMessageProof,
    ) -> Result<()> {
        self.create_run_internal(run, Some(proof))
    }

    fn create_run_internal(
        &self,
        run: &AgentRun,
        input_proof: Option<&CanonicalConversationMessageProof>,
    ) -> Result<()> {
        self.create_run_internal_with_transaction(run, input_proof, |_| Ok(()))
    }

    fn create_run_internal_with_transaction<F>(
        &self,
        run: &AgentRun,
        input_proof: Option<&CanonicalConversationMessageProof>,
        before_commit: F,
    ) -> Result<()>
    where
        F: FnOnce(&Transaction<'_>) -> Result<()>,
    {
        ensure_agent_run_collection_bounds(run)?;
        validate_new_agent_run_identity(run)?;
        normalized_identity_reference(&run.id, "run_id", &run.id)?;
        normalized_identity_reference(&run.id, "task_id", &run.task_id)?;
        if let Some(session_id) = run.session_id.as_deref() {
            normalized_identity_reference(&run.id, "session_id", session_id)?;
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let receipt_key = self.receipt_key.as_ref();
        let (input_ref, input_digest) =
            normalized_run_input(run, receipt_key, false).map_err(|error| {
                if input_proof.is_some() {
                    anyhow::anyhow!(
                        "agent_run_canonical_input_proof_mismatch: {}: {error}",
                        run.id
                    )
                } else {
                    error
                }
            })?;
        let bound_memory_store_identity = conn
            .query_row(
                "SELECT value FROM agent_run_store_metadata
                 WHERE key = 'canonical_memory_store_identity'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match (input_ref.as_deref(), input_digest.as_deref(), input_proof) {
            (Some(input_ref), Some(_input_digest), Some(proof))
                if bound_memory_store_identity.as_deref()
                    == Some(proof.canonical_store_identity())
                    && proof.role() == "user"
                    && run.session_id.as_deref() == Some(proof.session_id())
                    && input_ref == proof.canonical_ref()
                    && run.user_input.as_deref().is_some_and(|body| {
                        canonical_owner_content_digest(body) == proof.content_digest()
                    })
                    && canonical_conversation_message_id(input_ref) == Some(proof.message_id()) => {
            }
            (None, _, None) => {}
            (Some(_), _, None) => {
                anyhow::bail!("agent_run_canonical_input_proof_required: {}", run.id)
            }
            (Some(_), _, Some(_)) if bound_memory_store_identity.is_none() => {
                anyhow::bail!("agent_run_canonical_memory_store_not_bound: {}", run.id)
            }
            _ => anyhow::bail!("agent_run_canonical_input_proof_mismatch: {}", run.id),
        }
        let reasoning_trace_digest = normalized_reasoning_trace_digest(run, receipt_key, false)?;
        let context_summary_json = run
            .context_summary
            .as_ref()
            .map(|summary| {
                serde_json::to_string(&minimize_context_summary(
                    summary,
                    ReceiptOrigin::NewInput(receipt_key),
                ))
            })
            .transpose()
            .context("failed to serialize minimized AgentRun context summary")?;
        let model_route_json = run
            .model_route
            .as_ref()
            .map(|route| minimize_model_route(route, ReceiptOrigin::NewInput(receipt_key)))
            .map(|route| serde_json::to_string(&route))
            .transpose()
            .context("failed to serialize minimized AgentRun model route")?;
        let output_receipt = run.output_preview.as_deref().map(|value| {
            metadata_safe_text_receipt("run_output", value, ReceiptOrigin::NewInput(receipt_key))
        });
        let error_json = run
            .error
            .as_ref()
            .map(|error| minimize_error(error, ReceiptOrigin::NewInput(receipt_key)))
            .map(|error| serde_json::to_string(&error))
            .transpose()
            .context("failed to serialize minimized AgentRun error")?;
        let tx = conn.transaction()?;
        let canonical_store_identity = Self::existing_canonical_store_identity(&tx)?;
        let bound_content_attach_time = chrono::Utc::now().timestamp();
        let (canonical_actions, canonical_observations, pending_bound_content_attachments) =
            canonicalize_execution_records(
                &tx,
                &canonical_store_identity,
                bound_content_attach_time,
                &run.id,
                &run.actions,
                &run.observations,
                None,
                None,
                receipt_key,
            )?;
        let actions_json = serde_json::to_string(&canonical_actions)
            .context("failed to serialize owner-bound AgentRun actions")?;
        let observations_json = serde_json::to_string(&canonical_observations)
            .context("failed to serialize owner-bound AgentRun observations")?;
        let reasoning_strategy = run.reasoning_strategy.as_deref().map(|value| {
            metadata_safe_enum_or_receipt(
                "reasoning_strategy",
                value,
                &["layered", "direct", "react", "plan_execute", "unknown"],
                ReceiptOrigin::NewInput(receipt_key),
            )
        });
        let hs_selection_audit_json = run
            .hs_selection_audit
            .as_ref()
            .map(|audit| {
                serde_json::to_string(&minimize_hs_selection_audit(
                    audit,
                    ReceiptOrigin::NewInput(receipt_key),
                ))
            })
            .transpose()
            .context("failed to serialize minimized AgentRun HS selection audit")?;
        let behavior_checks_json =
            minimized_behavior_checks_json(&run.behavior_checks, receipt_key)?;
        let status_updates_json = minimized_status_updates_json(&run.status_updates, receipt_key)?;
        let delete_reason = run.delete_reason.as_deref().map(|value| {
            metadata_safe_text_receipt("delete_reason", value, ReceiptOrigin::NewInput(receipt_key))
        });
        let proposal_ids = Self::normalize_proposal_ids(&run.id, &run.generated_proposals)?;
        let proposal_ids_json = serde_json::to_string(&proposal_ids)
            .context("failed to serialize AgentRun proposal references")?;
        ensure_minimized_payload_bounds(
            &run.id,
            [
                context_summary_json.as_deref(),
                model_route_json.as_deref(),
                output_receipt.as_deref(),
                error_json.as_deref(),
                Some(proposal_ids_json.as_str()),
                Some(actions_json.as_str()),
                Some(observations_json.as_str()),
                reasoning_strategy.as_deref(),
                hs_selection_audit_json.as_deref(),
                Some(behavior_checks_json.as_str()),
                Some(status_updates_json.as_str()),
                delete_reason.as_deref(),
            ],
        )?;
        if let Some(session_id) = run.session_id.as_deref() {
            let tombstoned = tx
                .query_row(
                    "SELECT 1 FROM agent_run_session_tombstone_projections
                     WHERE session_id = ?1 LIMIT 1",
                    [session_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if tombstoned {
                anyhow::bail!("agent_run_session_canonical_source_tombstoned");
            }
        }
        tx.execute(
            "INSERT INTO agent_runs (
                id, task_id, session_id, status, kind,
                context_summary_json, model_route_json, output_preview, error_json,
                generated_proposals_json, actions_json, observations_json,
                reasoning_strategy, hs_selection_audit_json, behavior_checks_json,
                status_updates_json, step_count, tool_call_count,
                deleted_at, delete_reason, started_at, finished_at,
                input_ref, input_digest, reasoning_trace_digest,
                payload_minimized_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
            params![
                run.id,
                run.task_id,
                run.session_id,
                run.status.to_string(),
                run.kind.to_string(),
                context_summary_json,
                model_route_json,
                output_receipt,
                error_json,
                proposal_ids_json,
                actions_json,
                observations_json,
                reasoning_strategy,
                hs_selection_audit_json,
                behavior_checks_json,
                status_updates_json,
                run.step_count,
                run.tool_call_count,
                run.deleted_at.map(|t| t.to_rfc3339()),
                delete_reason,
                run.started_at.to_rfc3339(),
                run.finished_at.map(|t| t.to_rfc3339()),
                input_ref,
                input_digest,
                reasoning_trace_digest,
                AGENT_RUN_PAYLOAD_VERSION,
            ],
        )?;
        Self::attach_bound_content_issuances(
            &tx,
            &canonical_store_identity,
            &run.id,
            &pending_bound_content_attachments,
            bound_content_attach_time,
        )?;
        Self::replace_proposal_links(&tx, &run.id, &proposal_ids)?;
        before_commit(&tx)?;
        tx.commit()?;
        Ok(())
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn install_create_failure_for_test(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute_batch(
            "CREATE TRIGGER fail_agent_run_create_for_test
             BEFORE INSERT ON agent_runs
             BEGIN
                 SELECT RAISE(ABORT, 'injected agent run create failure');
             END;",
        )?;
        Ok(())
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn install_update_failure_for_test(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute_batch(
            "CREATE TRIGGER fail_agent_run_update_for_test
             BEFORE UPDATE ON agent_runs
             BEGIN
                 SELECT RAISE(ABORT, 'injected agent run update failure');
             END;",
        )?;
        Ok(())
    }

    pub fn update_run(&self, run: &AgentRun) -> Result<()> {
        self.update_run_internal_with_transaction(run, |_| Ok(()))
    }

    fn update_run_internal_with_transaction<F>(
        &self,
        run: &AgentRun,
        before_commit: F,
    ) -> Result<()>
    where
        F: FnOnce(&Transaction<'_>) -> Result<()>,
    {
        ensure_agent_run_collection_bounds(run)?;
        normalized_identity_reference(&run.id, "run_id", &run.id)?;
        normalized_identity_reference(&run.id, "task_id", &run.task_id)?;
        if let Some(session_id) = run.session_id.as_deref() {
            normalized_identity_reference(&run.id, "session_id", session_id)?;
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let canonical_store_identity = Self::existing_canonical_store_identity(&tx)?;
        let stored_run_query =
            format!("SELECT {AGENT_RUN_SELECT_COLUMNS} FROM agent_runs WHERE id = ?1");
        let stored_run = tx
            .query_row(&stored_run_query, [&run.id], |row| {
                Self::row_to_run(row, self.receipt_key.as_ref(), &canonical_store_identity)
            })
            .optional()?;
        let Some(stored_run) = stored_run else {
            anyhow::bail!("agent_run_update_missing: {}", run.id);
        };
        if stored_run.deleted_at.is_some()
            || persistence_outbox::has_active_tombstone(&tx, "agent_run", &run.id)?
        {
            anyhow::bail!("agent_run_update_owner_inactive: {}", run.id);
        }
        if run.deleted_at != stored_run.deleted_at || run.delete_reason != stored_run.delete_reason
        {
            anyhow::bail!(
                "agent_run_delete_restore_fields_owned_by_canonical_transaction: {}",
                run.id
            );
        }
        let receipt_key = self.receipt_key.as_ref();
        if stored_run.task_id != run.task_id || stored_run.session_id != run.session_id {
            anyhow::bail!("agent_run_immutable_identity_update_conflict: {}", run.id);
        }
        let (input_ref, input_digest) = normalized_run_input(run, receipt_key, true)?;
        let reasoning_trace_digest = normalized_reasoning_trace_digest(run, receipt_key, true)?;
        let reasoning_digest_conflict = match stored_run.reasoning_trace_digest.as_deref() {
            Some(stored) => reasoning_trace_digest.as_deref() != Some(stored),
            None => reasoning_trace_digest.is_some() && run.reasoning_trace.is_none(),
        };
        if stored_run.input_ref != input_ref
            || stored_run.input_digest != input_digest
            || reasoning_digest_conflict
        {
            anyhow::bail!("agent_run_immutable_evidence_update_conflict: {}", run.id);
        }
        let context_summary_json = run
            .context_summary
            .as_ref()
            .map(|summary| {
                minimize_composite_for_update(
                    summary,
                    stored_run.context_summary.as_ref(),
                    |value| minimize_context_summary(value, ReceiptOrigin::NewInput(receipt_key)),
                )
                .and_then(|value| serde_json::to_string(&value).map_err(Into::into))
            })
            .transpose()
            .context("failed to serialize minimized AgentRun context summary")?;
        let model_route_json = run
            .model_route
            .as_ref()
            .map(|route| {
                minimize_composite_for_update(route, stored_run.model_route.as_ref(), |value| {
                    minimize_model_route(value, ReceiptOrigin::NewInput(receipt_key))
                })
                .and_then(|value| serde_json::to_string(&value).map_err(Into::into))
            })
            .transpose()
            .context("failed to serialize minimized AgentRun model route")?;
        let output_origin = if run.output_preview == stored_run.output_preview {
            ReceiptOrigin::StoredCanonical(receipt_key)
        } else {
            ReceiptOrigin::NewInput(receipt_key)
        };
        let output_receipt = run
            .output_preview
            .as_deref()
            .map(|value| metadata_safe_text_receipt("run_output", value, output_origin));
        let error_json = run
            .error
            .as_ref()
            .map(|error| {
                minimize_composite_for_update(error, stored_run.error.as_ref(), |value| {
                    minimize_error(value, ReceiptOrigin::NewInput(receipt_key))
                })
                .and_then(|value| serde_json::to_string(&value).map_err(Into::into))
            })
            .transpose()
            .context("failed to serialize minimized AgentRun error")?;
        let bound_content_attach_time = chrono::Utc::now().timestamp();
        let (canonical_actions, canonical_observations, pending_bound_content_attachments) =
            canonicalize_execution_records(
                &tx,
                &canonical_store_identity,
                bound_content_attach_time,
                &run.id,
                &run.actions,
                &run.observations,
                Some(&stored_run.actions),
                Some(&stored_run.observations),
                receipt_key,
            )?;
        let actions_json = serde_json::to_string(&canonical_actions)
            .context("failed to serialize owner-bound AgentRun actions for update")?;
        let observations_json = serde_json::to_string(&canonical_observations)
            .context("failed to serialize owner-bound AgentRun observations for update")?;
        let reasoning_strategy_origin = if run.reasoning_strategy == stored_run.reasoning_strategy {
            ReceiptOrigin::StoredCanonical(receipt_key)
        } else {
            ReceiptOrigin::NewInput(receipt_key)
        };
        let reasoning_strategy = run.reasoning_strategy.as_deref().map(|value| {
            metadata_safe_enum_or_receipt(
                "reasoning_strategy",
                value,
                &["layered", "direct", "react", "plan_execute", "unknown"],
                reasoning_strategy_origin,
            )
        });
        let hs_selection_audit_json = run
            .hs_selection_audit
            .as_ref()
            .map(|audit| {
                minimize_composite_for_update(
                    audit,
                    stored_run.hs_selection_audit.as_ref(),
                    |value| {
                        minimize_hs_selection_audit(value, ReceiptOrigin::NewInput(receipt_key))
                    },
                )
                .and_then(|value| serde_json::to_string(&value).map_err(Into::into))
            })
            .transpose()
            .context("failed to serialize minimized AgentRun HS selection audit")?;
        let behavior_checks_json = minimized_behavior_checks_json_for_update(
            &run.behavior_checks,
            &stored_run.behavior_checks,
            receipt_key,
        )?;
        let status_updates_json = minimized_status_updates_json_for_update(
            &run.status_updates,
            &stored_run.status_updates,
            receipt_key,
        )?;
        let proposal_ids = Self::normalize_proposal_ids(&run.id, &run.generated_proposals)?;
        let proposal_ids_json = serde_json::to_string(&proposal_ids)
            .context("failed to serialize AgentRun proposal references")?;
        ensure_minimized_payload_bounds(
            &run.id,
            [
                context_summary_json.as_deref(),
                model_route_json.as_deref(),
                output_receipt.as_deref(),
                error_json.as_deref(),
                Some(proposal_ids_json.as_str()),
                Some(actions_json.as_str()),
                Some(observations_json.as_str()),
                reasoning_strategy.as_deref(),
                hs_selection_audit_json.as_deref(),
                Some(behavior_checks_json.as_str()),
                Some(status_updates_json.as_str()),
                stored_run.delete_reason.as_deref(),
            ],
        )?;
        let update_query = format!(
            "UPDATE agent_runs AS run SET
                status = ?2,
                context_summary_json = ?3,
                model_route_json = ?4,
                output_preview = ?5,
                error_json = ?6,
                generated_proposals_json = ?7,
                actions_json = ?8,
                observations_json = ?9,
                reasoning_strategy = ?10,
                hs_selection_audit_json = ?11,
                behavior_checks_json = ?12,
                status_updates_json = ?13,
                step_count = ?14,
                tool_call_count = ?15,
                finished_at = ?16,
                input_ref = ?17,
                input_digest = ?18,
                reasoning_trace_digest = ?19,
                payload_minimized_version = ?20
            WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let changed = tx.execute(
            &update_query,
            params![
                run.id,
                run.status.to_string(),
                context_summary_json,
                model_route_json,
                output_receipt,
                error_json,
                proposal_ids_json,
                actions_json,
                observations_json,
                reasoning_strategy,
                hs_selection_audit_json,
                behavior_checks_json,
                status_updates_json,
                run.step_count,
                run.tool_call_count,
                run.finished_at.map(|t| t.to_rfc3339()),
                input_ref,
                input_digest,
                reasoning_trace_digest,
                AGENT_RUN_PAYLOAD_VERSION,
            ],
        )?;
        if changed != 1 {
            anyhow::bail!("agent_run_update_owner_inactive: {}", run.id);
        }
        Self::attach_bound_content_issuances(
            &tx,
            &canonical_store_identity,
            &run.id,
            &pending_bound_content_attachments,
            bound_content_attach_time,
        )?;
        Self::replace_proposal_links(&tx, &run.id, &proposal_ids)?;
        before_commit(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// Product-visible read. Canonically deleted runs are absent even while
    /// derived projection cleanup is pending.
    pub fn get_live_run(&self, run_id: &str) -> Result<Option<AgentRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let canonical_store_identity = Self::existing_canonical_store_identity(&conn)?;
        let query = format!(
            "SELECT {AGENT_RUN_SELECT_COLUMNS}
             FROM agent_runs AS run
             WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        conn.query_row(&query, [run_id], |row| {
            Self::row_to_run(row, self.receipt_key.as_ref(), &canonical_store_identity)
        })
        .optional()
        .map_err(Into::into)
    }

    /// Compatibility read with product-safe visibility semantics. Recovery
    /// code that must inspect a deleted canonical row must opt in explicitly
    /// through `get_run_including_deleted`.
    pub fn get_run(&self, run_id: &str) -> Result<Option<AgentRun>> {
        self.get_live_run(run_id)
    }

    fn live_agent_run_status_on_connection(
        conn: &Connection,
        run_id: &str,
    ) -> Result<Option<String>> {
        let query = format!(
            "SELECT run.status FROM agent_runs AS run
             WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        conn.query_row(&query, [run_id], |row| row.get(0))
            .optional()
            .map_err(Into::into)
    }

    fn active_bound_content_owner_exists_on_connection(
        conn: &Connection,
        run_id: &str,
    ) -> Result<bool> {
        Ok(Self::live_agent_run_status_on_connection(conn, run_id)?.as_deref() == Some("running"))
    }

    /// Read-only owner-authority check used by ToolGateway before an internal
    /// read can reach an adapter. Receipt issuance repeats the same check in
    /// its write transaction, so this is an early blocker rather than the
    /// final TOCTOU authority.
    pub(crate) fn has_active_bound_content_owner(&self, run_id: &str) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        Self::active_bound_content_owner_exists_on_connection(&conn, run_id)
    }

    pub fn get_run_including_deleted(&self, run_id: &str) -> Result<Option<AgentRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let canonical_store_identity = Self::existing_canonical_store_identity(&conn)?;
        let query = format!("SELECT {AGENT_RUN_SELECT_COLUMNS} FROM agent_runs WHERE id = ?1");
        let mut stmt = conn.prepare(&query)?;
        let row = stmt.query_row([run_id], |row| {
            Self::row_to_run(row, self.receipt_key.as_ref(), &canonical_store_identity)
        });
        match row {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_run_for_task_id(&self, task_id: &str) -> Result<Option<AgentRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let canonical_store_identity = Self::existing_canonical_store_identity(&conn)?;
        let query = format!(
            "SELECT {AGENT_RUN_SELECT_COLUMNS}
             FROM agent_runs AS run
             WHERE run.task_id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}
             ORDER BY run.started_at ASC
             LIMIT 2"
        );
        let mut statement = conn.prepare(&query)?;
        let runs = statement
            .query_map([task_id], |row| {
                Self::row_to_run(row, self.receipt_key.as_ref(), &canonical_store_identity)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if runs.len() > 1 {
            anyhow::bail!("agent_run_task_identity_conflict: {task_id}");
        }
        Ok(runs.into_iter().next())
    }

    pub fn list_runs_linked_to_proposal(&self, proposal_id: &str) -> Result<Vec<AgentRun>> {
        let proposal_id = proposal_id.trim();
        if proposal_id.is_empty() || proposal_id.len() > 192 {
            anyhow::bail!("invalid AgentRun proposal reference query");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let canonical_store_identity = Self::existing_canonical_store_identity(&conn)?;
        let query = format!(
            "SELECT {AGENT_RUN_SELECT_COLUMNS_RUN}
             FROM agent_run_proposal_links AS link
             INNER JOIN agent_runs AS run ON run.id = link.run_id
             WHERE link.proposal_id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}
             ORDER BY run.started_at ASC"
        );
        let mut statement = conn.prepare(&query)?;
        let runs = statement.query_map([proposal_id], |row| {
            Self::row_to_run(row, self.receipt_key.as_ref(), &canonical_store_identity)
        })?;
        runs.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Returns a bounded set of Proposal ids that still have at least one live
    /// WaitingPermission AgentRun projection. This is an indexed reconciliation queue,
    /// not a scan across all historical AgentRuns.
    pub fn list_waiting_permission_linked_proposal_ids(&self, limit: i64) -> Result<Vec<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let query = format!(
            "SELECT DISTINCT link.proposal_id
             FROM agent_runs AS run INDEXED BY idx_agent_runs_waiting_permission
             CROSS JOIN agent_run_proposal_links AS link
             WHERE run.status = 'waiting_permission'
               AND {LIVE_AGENT_RUN_SQL_PREDICATE}
               AND link.run_id = run.id
             LIMIT ?1"
        );
        let mut statement = conn.prepare(&query)?;
        let proposal_ids = statement.query_map([limit.clamp(1, 200)], |row| row.get(0))?;
        proposal_ids
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_runs(&self, limit: i64, offset: i64) -> Result<Vec<AgentRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let canonical_store_identity = Self::existing_canonical_store_identity(&conn)?;
        let query = format!(
            "SELECT {AGENT_RUN_SELECT_COLUMNS}
             FROM agent_runs AS run
             WHERE {LIVE_AGENT_RUN_SQL_PREDICATE}
             ORDER BY run.started_at DESC, run.id DESC
             LIMIT ?1 OFFSET ?2"
        );
        let mut stmt = conn.prepare(&query)?;
        let runs = stmt.query_map([limit, offset], |row| {
            Self::row_to_run(row, self.receipt_key.as_ref(), &canonical_store_identity)
        })?;
        runs.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn list_runs_for_session(&self, session_id: &str, limit: i64) -> Result<Vec<AgentRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let canonical_store_identity = Self::existing_canonical_store_identity(&conn)?;
        let query = format!(
            "SELECT {AGENT_RUN_SELECT_COLUMNS}
             FROM agent_runs AS run
             WHERE run.session_id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}
             ORDER BY run.started_at DESC, run.id DESC
             LIMIT ?2"
        );
        let mut stmt = conn.prepare(&query)?;
        let runs = stmt.query_map(rusqlite::params![session_id, limit], |row| {
            Self::row_to_run(row, self.receipt_key.as_ref(), &canonical_store_identity)
        })?;
        runs.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    fn row_to_run(
        row: &rusqlite::Row<'_>,
        receipt_key: &AgentRunReceiptKey,
        canonical_store_identity: &str,
    ) -> rusqlite::Result<AgentRun> {
        let run_id: String = row.get(0)?;
        let task_id: String = row.get(1)?;
        let session_id: Option<String> = row.get(2)?;
        let status_str: String = row.get(3)?;
        let kind_str: String = row.get(4)?;
        let context_summary_json: Option<String> = row.get(5)?;
        let model_route_json: Option<String> = row.get(6)?;
        let output_preview: Option<String> = row.get(7)?;
        let error_json: Option<String> = row.get(8)?;
        let generated_proposals_json: Option<String> = row.get(9)?;
        let actions_json: Option<String> = row.get(10)?;
        let observations_json: Option<String> = row.get(11)?;
        let reasoning_strategy: Option<String> = row.get(12)?;
        let hs_selection_audit_json: Option<String> = row.get(13)?;
        let behavior_checks_json: Option<String> = row.get(14)?;
        let status_updates_json: Option<String> = row.get(15)?;
        let step_count: u32 = row.get(16)?;
        let tool_call_count: u32 = row.get(17)?;
        let deleted_at_str: Option<String> = row.get(18)?;
        let delete_reason: Option<String> = row.get(19)?;
        let started_at_str: String = row.get(20)?;
        let finished_at_str: Option<String> = row.get(21)?;
        let input_ref: Option<String> = row.get(22)?;
        let input_digest: Option<String> = row.get(23)?;
        let reasoning_trace_digest: Option<String> = row.get(24)?;
        let payload_minimized_version: i64 = row.get(25)?;
        let legacy_payload_unverified = match row.get::<_, i64>(26)? {
            0 => false,
            1 => true,
            _ => {
                return Err(agent_run_row_fault(
                    26,
                    "legacy_payload_unverified",
                    "must_be_zero_or_one",
                ))
            }
        };

        if payload_minimized_version != AGENT_RUN_PAYLOAD_VERSION {
            return Err(agent_run_row_fault(
                25,
                "payload_minimized_version",
                "unsupported_version",
            ));
        }
        let stored_payload_bytes = [
            context_summary_json.as_deref(),
            model_route_json.as_deref(),
            output_preview.as_deref(),
            error_json.as_deref(),
            generated_proposals_json.as_deref(),
            actions_json.as_deref(),
            observations_json.as_deref(),
            reasoning_strategy.as_deref(),
            hs_selection_audit_json.as_deref(),
            behavior_checks_json.as_deref(),
            status_updates_json.as_deref(),
            delete_reason.as_deref(),
        ]
        .into_iter()
        .flatten()
        .try_fold(0usize, |total, payload| total.checked_add(payload.len()))
        .ok_or_else(|| agent_run_row_fault(25, "payload", "size_overflow"))?;
        if stored_payload_bytes > MAX_AGENT_RUN_STORED_PAYLOAD_BYTES {
            return Err(agent_run_row_fault(25, "payload", "size_limit_exceeded"));
        }
        normalized_identity_reference(&run_id, "run_id", &run_id)
            .map_err(|_| agent_run_row_fault(0, "run_id", "invalid_reference"))?;
        normalized_identity_reference(&run_id, "task_id", &task_id)
            .map_err(|_| agent_run_row_fault(1, "task_id", "invalid_reference"))?;
        if let Some(session_id) = session_id.as_deref() {
            normalized_identity_reference(&run_id, "session_id", session_id)
                .map_err(|_| agent_run_row_fault(2, "session_id", "invalid_reference"))?;
        }
        let status = match status_str.as_str() {
            "running" => AgentRunStatus::Running,
            "waiting_permission" => AgentRunStatus::WaitingPermission,
            "completed" => AgentRunStatus::Completed,
            "failed" => AgentRunStatus::Failed,
            "remote_unknown" => AgentRunStatus::RemoteUnknown,
            "cancelled" => AgentRunStatus::Cancelled,
            _ => return Err(agent_run_row_fault(3, "status", "invalid_enum")),
        };

        let kind = match kind_str.as_str() {
            "conversation" => AgentTaskKind::Conversation,
            "builder" => AgentTaskKind::Builder,
            "calibration" => AgentTaskKind::Calibration,
            "evolution" => AgentTaskKind::Evolution,
            "tool_execution" => AgentTaskKind::ToolExecution,
            "proactive" => AgentTaskKind::Proactive,
            "planning" => AgentTaskKind::Planning,
            "review" => AgentTaskKind::Review,
            "writing" => AgentTaskKind::Writing,
            "memory_governance" => AgentTaskKind::MemoryGovernance,
            "skill" => AgentTaskKind::Skill,
            "plugin" => AgentTaskKind::Plugin,
            _ => return Err(agent_run_row_fault(4, "kind", "invalid_enum")),
        };

        let context_summary = decode_optional_minimized_json(
            context_summary_json,
            5,
            "context_summary_json",
            |summary| {
                minimize_context_summary(summary, ReceiptOrigin::StoredCanonical(receipt_key))
            },
        )?;
        let model_route =
            decode_optional_minimized_json(model_route_json, 6, "model_route_json", |route| {
                minimize_model_route(route, ReceiptOrigin::StoredCanonical(receipt_key))
            })?;
        if output_preview
            .as_deref()
            .is_some_and(|value| !is_metadata_safe_text_receipt("run_output", value))
        {
            return Err(agent_run_row_fault(
                7,
                "output_preview",
                "noncanonical_receipt",
            ));
        }
        let error = decode_optional_minimized_json(error_json, 8, "error_json", |error| {
            minimize_error(error, ReceiptOrigin::StoredCanonical(receipt_key))
        })?;
        let generated_proposals = decode_required_minimized_json::<Vec<String>, _>(
            generated_proposals_json,
            9,
            "generated_proposals_json",
            |proposal_ids| proposal_ids.clone(),
        )?;
        let normalized_proposal_ids = Self::normalize_proposal_ids(&run_id, &generated_proposals)
            .map_err(|_| {
            agent_run_row_fault(9, "generated_proposals_json", "invalid_reference")
        })?;
        if normalized_proposal_ids != generated_proposals {
            return Err(agent_run_row_fault(
                9,
                "generated_proposals_json",
                "noncanonical_reference_set",
            ));
        }
        let actions = decode_required_minimized_json(
            actions_json,
            10,
            "actions_json",
            |actions: &Vec<crate::agent::types::AgentAction>| {
                actions
                    .iter()
                    .map(|action| {
                        minimize_action(action, ReceiptOrigin::StoredCanonical(receipt_key))
                    })
                    .collect()
            },
        )?;
        let observations = decode_required_minimized_json(
            observations_json,
            11,
            "observations_json",
            |observations: &Vec<crate::agent::types::AgentObservation>| {
                observations
                    .iter()
                    .map(|observation| {
                        minimize_observation(
                            observation,
                            ReceiptOrigin::StoredCanonical(receipt_key),
                        )
                    })
                    .collect()
            },
        )?;
        validate_persisted_execution_identity_graph(
            canonical_store_identity,
            &run_id,
            &actions,
            &observations,
            receipt_key,
        )
        .map_err(|_| agent_run_row_fault(10, "execution_records", "invalid_owner_graph"))?;
        if reasoning_strategy.as_deref().is_some_and(|strategy| {
            metadata_safe_enum_or_receipt(
                "reasoning_strategy",
                strategy,
                &["layered", "direct", "react", "plan_execute", "unknown"],
                ReceiptOrigin::StoredCanonical(receipt_key),
            ) != strategy
        }) {
            return Err(agent_run_row_fault(
                12,
                "reasoning_strategy",
                "noncanonical_type_or_receipt",
            ));
        }
        let hs_selection_audit = decode_optional_minimized_json(
            hs_selection_audit_json,
            13,
            "hs_selection_audit_json",
            |audit| minimize_hs_selection_audit(audit, ReceiptOrigin::StoredCanonical(receipt_key)),
        )?;
        let behavior_checks = decode_required_minimized_json(
            behavior_checks_json,
            14,
            "behavior_checks_json",
            |checks: &Vec<crate::agent::types::HSBehaviorCheckSummary>| {
                checks
                    .iter()
                    .map(|check| {
                        minimize_behavior_check(check, ReceiptOrigin::StoredCanonical(receipt_key))
                    })
                    .collect()
            },
        )?;
        let status_updates = decode_required_minimized_json(
            status_updates_json,
            15,
            "status_updates_json",
            |updates: &Vec<crate::agent::types::AgentLoopStatusUpdate>| {
                updates
                    .iter()
                    .map(|update| {
                        minimize_status_update(update, ReceiptOrigin::StoredCanonical(receipt_key))
                    })
                    .collect()
            },
        )?;
        let deleted_at = decode_optional_timestamp(deleted_at_str, 18, "deleted_at")?;
        if delete_reason
            .as_deref()
            .is_some_and(|value| !is_metadata_safe_text_receipt("delete_reason", value))
        {
            return Err(agent_run_row_fault(
                19,
                "delete_reason",
                "noncanonical_receipt",
            ));
        }

        let started_at = chrono::DateTime::parse_from_rfc3339(&started_at_str)
            .map_err(|_| agent_run_row_fault(20, "started_at", "invalid_timestamp"))?
            .with_timezone(&chrono::Utc);
        let finished_at = decode_optional_timestamp(finished_at_str, 21, "finished_at")?;
        let normalized_input_ref = normalized_input_reference(&run_id, input_ref.as_deref())
            .map_err(|_| agent_run_row_fault(22, "input_ref", "invalid_reference"))?;
        if normalized_input_ref != input_ref {
            return Err(agent_run_row_fault(
                22,
                "input_ref",
                "noncanonical_reference",
            ));
        }
        if !input_reference_matches_session(input_ref.as_deref(), session_id.as_deref()) {
            return Err(agent_run_row_fault(
                22,
                "input_ref",
                "conversation_session_mismatch",
            ));
        }
        let normalized_input_digest =
            normalized_metadata_digest(&run_id, "input_digest", input_digest.as_deref())
                .map_err(|_| agent_run_row_fault(23, "input_digest", "invalid_digest"))?;
        if normalized_input_digest != input_digest {
            return Err(agent_run_row_fault(
                23,
                "input_digest",
                "noncanonical_digest",
            ));
        }
        let normalized_reasoning_digest = normalized_metadata_digest(
            &run_id,
            "reasoning_trace_digest",
            reasoning_trace_digest.as_deref(),
        )
        .map_err(|_| agent_run_row_fault(24, "reasoning_trace_digest", "invalid_digest"))?;
        if normalized_reasoning_digest != reasoning_trace_digest {
            return Err(agent_run_row_fault(
                24,
                "reasoning_trace_digest",
                "noncanonical_digest",
            ));
        }

        let run = AgentRun {
            id: run_id,
            task_id,
            session_id,
            status,
            kind,
            user_input: None,
            input_ref,
            input_digest,
            context_summary,
            model_route,
            output_preview,
            error,
            generated_proposals,
            actions,
            observations,
            reasoning_strategy,
            reasoning_trace: None,
            reasoning_trace_digest,
            legacy_payload_unverified,
            hs_selection_audit,
            behavior_checks,
            warnings: Vec::new(),
            status_updates,
            step_count,
            tool_call_count,
            deleted_at,
            delete_reason,
            started_at,
            finished_at,
        };
        ensure_agent_run_collection_bounds(&run)
            .map_err(|_| agent_run_row_fault(25, "payload", "collection_limit_exceeded"))?;
        Ok(run)
    }

    pub fn run_count(&self) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let query = format!(
            "SELECT COUNT(*) FROM agent_runs AS run
             WHERE {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let count: i64 = conn.query_row(&query, [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn last_run_for_session(&self, session_id: &str) -> Result<Option<AgentRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let canonical_store_identity = Self::existing_canonical_store_identity(&conn)?;
        let query = format!(
            "SELECT {AGENT_RUN_SELECT_COLUMNS}
             FROM agent_runs AS run
             WHERE run.session_id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}
             ORDER BY run.started_at DESC, run.id DESC
             LIMIT 1"
        );
        let mut stmt = conn.prepare(&query)?;
        let row = stmt.query_row([session_id], |row| {
            Self::row_to_run(row, self.receipt_key.as_ref(), &canonical_store_identity)
        });
        match row {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn add_generated_proposal(&self, run_id: &str, proposal_id: &str) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let live_owner_query = format!(
            "SELECT run.generated_proposals_json, run.payload_minimized_version
             FROM agent_runs AS run
             WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let (current, version): (Option<String>, i64) =
            tx.query_row(&live_owner_query, [run_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        if version != AGENT_RUN_PAYLOAD_VERSION {
            anyhow::bail!("agent_run_proposal_payload_version_unsupported");
        }
        let current = current.context("agent_run_generated_proposals_json_missing")?;
        let mut proposals = serde_json::from_str::<Vec<String>>(&current)
            .with_context(|| format!("invalid generated_proposals_json for AgentRun {run_id}"))?;
        if Self::normalize_proposal_ids(run_id, &proposals)? != proposals {
            anyhow::bail!("noncanonical generated_proposals_json for AgentRun {run_id}");
        }
        proposals.push(proposal_id.to_string());
        let proposals = Self::normalize_proposal_ids(run_id, &proposals)?;
        let updated = serde_json::to_string(&proposals)
            .context("failed to serialize AgentRun proposal references")?;
        let update_query = format!(
            "UPDATE agent_runs AS run
             SET generated_proposals_json = ?2
             WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let changed = tx.execute(&update_query, rusqlite::params![run_id, updated])?;
        if changed != 1 {
            anyhow::bail!("agent_run_update_missing: {run_id}");
        }
        Self::replace_proposal_links(&tx, run_id, &proposals)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_run_with_tombstone(
        &self,
        run_id: &str,
        reason: Option<&str>,
    ) -> Result<CanonicalMutationReceipt> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let deleted_at = chrono::Utc::now().to_rfc3339();
        let safe_reason = reason.map(|value| {
            metadata_safe_text_receipt(
                "delete_reason",
                value,
                ReceiptOrigin::NewInput(self.receipt_key.as_ref()),
            )
        });
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE agent_runs SET deleted_at = ?2, delete_reason = ?3 WHERE id = ?1",
            rusqlite::params![run_id, deleted_at, safe_reason],
        )?;
        if changed == 0 {
            anyhow::bail!("agent_run_delete_missing: {run_id}");
        }
        tx.execute(
            "DELETE FROM bound_content_issuance_ledger
             WHERE run_id = ?1 AND state = 'pending'",
            [run_id],
        )?;
        let receipt = persistence_outbox::enqueue_tombstone(
            &tx,
            "agent_run",
            run_id,
            reason,
            &["turn_event_store", "action_queue_store", "life_event_store"],
        )?;
        tx.commit()?;
        Ok(receipt)
    }

    pub fn restore_run_with_receipt(&self, run_id: &str) -> Result<CanonicalMutationReceipt> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let parent_tombstoned = tx
            .query_row(
                "SELECT 1
                 FROM agent_runs run
                 JOIN agent_run_session_tombstone_projections tombstone
                   ON tombstone.session_id = run.session_id
                 WHERE run.id = ?1
                 LIMIT 1",
                [run_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if parent_tombstoned {
            anyhow::bail!("agent_run_restore_blocked_by_conversation_tombstone");
        }
        if !persistence_outbox::has_active_tombstone(&tx, "agent_run", run_id)? {
            anyhow::bail!("agent_run_restore_requires_active_canonical_tombstone");
        }
        let changed = tx.execute(
            "UPDATE agent_runs SET deleted_at = NULL, delete_reason = NULL WHERE id = ?1",
            [run_id],
        )?;
        if changed == 0 {
            anyhow::bail!("agent_run_restore_missing: {run_id}");
        }
        let receipt = persistence_outbox::supersede_active_tombstone(
            &tx,
            "agent_run",
            run_id,
            &persistence_outbox::metadata_digest(&format!("agent_run:{run_id}:restored")),
            &["turn_event_store", "action_queue_store", "life_event_store"],
        )?;
        tx.commit()?;
        Ok(receipt)
    }

    /// Idempotent derived cleanup for a canonical conversation tombstone. The
    /// conversation owner remains `memory.db`; this store only removes its
    /// AgentRun projection from live reads and retains metadata-safe lineage.
    pub fn project_conversation_tombstone(
        &self,
        tombstone_id: &str,
        session_id: &str,
    ) -> Result<usize> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let already_applied = tx
            .query_row(
                "SELECT 1 FROM agent_run_session_tombstone_projections
                 WHERE tombstone_id = ?1",
                [tombstone_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if already_applied {
            tx.commit()?;
            return Ok(0);
        }
        let deleted_at = chrono::Utc::now().to_rfc3339();
        let reason = metadata_safe_text_receipt(
            "delete_reason",
            &format!("conversation_tombstone:{tombstone_id}"),
            ReceiptOrigin::NewInput(self.receipt_key.as_ref()),
        );
        let affected = tx.execute(
            "UPDATE agent_runs
             SET deleted_at = COALESCE(deleted_at, ?2),
                 delete_reason = COALESCE(delete_reason, ?3)
             WHERE session_id = ?1 AND deleted_at IS NULL",
            params![session_id, deleted_at, reason],
        )?;
        tx.execute(
            "INSERT INTO agent_run_session_tombstone_projections (
                tombstone_id, session_id, applied_at
             ) VALUES (?1, ?2, ?3)",
            params![tombstone_id, session_id, chrono::Utc::now().to_rfc3339()],
        )?;
        tx.commit()?;
        Ok(affected)
    }

    /// Return metadata-only projection keys even after a conversation
    /// tombstone has hidden the runs. Reconciliation consumers need these keys
    /// to clean action/event projections without copying run content.
    pub fn projection_refs_for_session(&self, session_id: &str) -> Result<Vec<(String, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT id, task_id FROM agent_runs WHERE session_id = ?1 ORDER BY started_at ASC",
        )?;
        let refs = statement
            .query_map([session_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(anyhow::Error::from)?;
        Ok(refs)
    }

    pub fn list_replayable_projection_deliveries(
        &self,
        limit: usize,
    ) -> Result<Vec<ProjectionDelivery>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::list_replayable_deliveries(&conn, limit)
    }

    pub fn list_replayable_projection_deliveries_for_event(
        &self,
        event_id: &str,
    ) -> Result<Vec<ProjectionDelivery>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::list_replayable_deliveries_for_event(&conn, event_id)
    }

    pub fn superseded_tombstone_ids_for_restore_event(
        &self,
        restore_event_id: &str,
    ) -> Result<Vec<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::superseded_tombstone_ids_for_event(&conn, restore_event_id)
    }

    pub fn canonical_projection_head(
        &self,
        run_id: &str,
    ) -> Result<Option<CanonicalMutationReceipt>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::latest_mutation_for_aggregate(&conn, "agent_run", run_id)
    }

    pub fn canonical_tombstone_ids(&self, run_id: &str) -> Result<Vec<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::tombstone_ids_for_aggregate(&conn, "agent_run", run_id)
    }

    pub fn mark_projection_compensated_to_head(
        &self,
        stale_event_id: &str,
        head_event_id: &str,
        head_revision: u64,
        projection_target: &str,
    ) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_compensated_to_head(
            &mut conn,
            stale_event_id,
            head_event_id,
            head_revision,
            projection_target,
        )
    }

    pub fn mark_projection_applied_if_canonical_head(
        &self,
        event_id: &str,
        aggregate_revision: u64,
        projection_target: &str,
    ) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_applied_if_canonical_head(
            &mut conn,
            event_id,
            aggregate_revision,
            projection_target,
        )
    }

    pub fn mark_projection_applied(&self, event_id: &str, projection_target: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_applied(&conn, event_id, projection_target)
    }

    pub fn mark_projection_degraded(
        &self,
        event_id: &str,
        projection_target: &str,
        error: &str,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_degraded(&conn, event_id, projection_target, error)
    }

    pub fn projection_summary(&self, event_id: &str) -> Result<ProjectionSummary> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::projection_summary(&conn, event_id)
    }

    pub fn cleanup_old_deleted_runs(&self, days: i64) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let rows_affected = conn.execute(
            "DELETE FROM agent_runs WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
            [cutoff],
        )?;
        Ok(rows_affected)
    }

    pub fn add_action(
        &self,
        run_id: &str,
        action: &crate::agent::types::AgentAction,
    ) -> Result<()> {
        if action
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .is_some()
        {
            anyhow::bail!("bound_content_receipt_requires_atomic_action_observation_update");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let live_owner_query = format!(
            "SELECT run.actions_json, run.payload_minimized_version,
                    LENGTH(COALESCE(context_summary_json, '')) +
                    LENGTH(COALESCE(model_route_json, '')) +
                    LENGTH(COALESCE(output_preview, '')) +
                    LENGTH(COALESCE(error_json, '')) +
                    LENGTH(COALESCE(generated_proposals_json, '')) +
                    LENGTH(COALESCE(observations_json, '')) +
                    LENGTH(COALESCE(reasoning_strategy, '')) +
                    LENGTH(COALESCE(hs_selection_audit_json, '')) +
                    LENGTH(COALESCE(behavior_checks_json, '')) +
                    LENGTH(COALESCE(status_updates_json, '')) +
                    LENGTH(COALESCE(delete_reason, ''))
             FROM agent_runs AS run
             WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let (current, version, other_payload_bytes): (Option<String>, i64, i64) =
            tx.query_row(&live_owner_query, [run_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
        if version != AGENT_RUN_PAYLOAD_VERSION {
            anyhow::bail!("agent_run_actions_payload_version_unsupported");
        }
        let current = current.context("agent_run_actions_json_missing")?;
        let mut actions: Vec<crate::agent::types::AgentAction> = serde_json::from_str(&current)
            .with_context(|| format!("invalid actions_json for AgentRun {run_id}"))?;
        let canonical_current = minimized_actions_json_with_origin(
            &actions,
            ReceiptOrigin::StoredCanonical(self.receipt_key.as_ref()),
        )?;
        if serde_json::from_str::<serde_json::Value>(&current)?
            != serde_json::from_str::<serde_json::Value>(&canonical_current)?
        {
            anyhow::bail!("noncanonical actions_json for AgentRun {run_id}");
        }
        if actions.len() >= MAX_AGENT_RUN_COLLECTION_ITEMS {
            anyhow::bail!("agent_run_collection_limit_exceeded:actions");
        }
        if action
            .tool_scope
            .as_ref()
            .is_some_and(|scope| scope.capabilities.len() > MAX_AGENT_RUN_NESTED_REFS)
        {
            anyhow::bail!("agent_run_nested_reference_limit_exceeded:tool_capabilities");
        }
        actions.push(minimize_action(
            action,
            ReceiptOrigin::NewInput(self.receipt_key.as_ref()),
        ));
        let updated = serde_json::to_string(&actions)
            .context("failed to serialize updated AgentRun actions")?;
        if usize::try_from(other_payload_bytes)
            .ok()
            .and_then(|other| other.checked_add(updated.len()))
            .map_or(true, |total| total > MAX_AGENT_RUN_STORED_PAYLOAD_BYTES)
        {
            anyhow::bail!("agent_run_minimized_payload_limit_exceeded:{run_id}");
        }
        let update_query = format!(
            "UPDATE agent_runs AS run
             SET actions_json = ?2
             WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let changed = tx.execute(&update_query, rusqlite::params![run_id, updated])?;
        if changed != 1 {
            anyhow::bail!("agent_run_update_missing: {run_id}");
        }
        tx.commit()?;
        Ok(())
    }

    pub fn add_observation(
        &self,
        run_id: &str,
        observation: &crate::agent::types::AgentObservation,
    ) -> Result<()> {
        if observation
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .is_some()
        {
            anyhow::bail!("bound_content_receipt_requires_atomic_action_observation_update");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let live_owner_query = format!(
            "SELECT run.observations_json, run.payload_minimized_version,
                    LENGTH(COALESCE(context_summary_json, '')) +
                    LENGTH(COALESCE(model_route_json, '')) +
                    LENGTH(COALESCE(output_preview, '')) +
                    LENGTH(COALESCE(error_json, '')) +
                    LENGTH(COALESCE(generated_proposals_json, '')) +
                    LENGTH(COALESCE(actions_json, '')) +
                    LENGTH(COALESCE(reasoning_strategy, '')) +
                    LENGTH(COALESCE(hs_selection_audit_json, '')) +
                    LENGTH(COALESCE(behavior_checks_json, '')) +
                    LENGTH(COALESCE(status_updates_json, '')) +
                    LENGTH(COALESCE(delete_reason, ''))
             FROM agent_runs AS run
             WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let (current, version, other_payload_bytes): (Option<String>, i64, i64) =
            tx.query_row(&live_owner_query, [run_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
        if version != AGENT_RUN_PAYLOAD_VERSION {
            anyhow::bail!("agent_run_observations_payload_version_unsupported");
        }
        let current = current.context("agent_run_observations_json_missing")?;
        let mut observations: Vec<crate::agent::types::AgentObservation> =
            serde_json::from_str(&current)
                .with_context(|| format!("invalid observations_json for AgentRun {run_id}"))?;
        let canonical_current = minimized_observations_json_with_origin(
            &observations,
            ReceiptOrigin::StoredCanonical(self.receipt_key.as_ref()),
        )?;
        if serde_json::from_str::<serde_json::Value>(&current)?
            != serde_json::from_str::<serde_json::Value>(&canonical_current)?
        {
            anyhow::bail!("noncanonical observations_json for AgentRun {run_id}");
        }
        if observations.len() >= MAX_AGENT_RUN_COLLECTION_ITEMS {
            anyhow::bail!("agent_run_collection_limit_exceeded:observations");
        }
        observations.push(minimize_observation(
            observation,
            ReceiptOrigin::NewInput(self.receipt_key.as_ref()),
        ));
        let updated = serde_json::to_string(&observations)
            .context("failed to serialize updated AgentRun observations")?;
        if usize::try_from(other_payload_bytes)
            .ok()
            .and_then(|other| other.checked_add(updated.len()))
            .map_or(true, |total| total > MAX_AGENT_RUN_STORED_PAYLOAD_BYTES)
        {
            anyhow::bail!("agent_run_minimized_payload_limit_exceeded:{run_id}");
        }
        let update_query = format!(
            "UPDATE agent_runs AS run
             SET observations_json = ?2
             WHERE run.id = ?1 AND {LIVE_AGENT_RUN_SQL_PREDICATE}"
        );
        let changed = tx.execute(&update_query, rusqlite::params![run_id, updated])?;
        if changed != 1 {
            anyhow::bail!("agent_run_update_missing: {run_id}");
        }
        tx.commit()?;
        Ok(())
    }
}

impl BoundContentReceiptIssuer for AgentRunStore {
    fn issue_bound_content_receipt(
        &self,
        admission: ObservedToolBodyAdmission,
        action: &crate::agent::types::AgentAction,
        observation: &crate::agent::types::AgentObservation,
    ) -> Result<ContentReceipt> {
        if action
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .is_some()
            || observation
                .react_trace
                .as_ref()
                .and_then(|trace| trace.output_receipt.as_ref())
                .is_some()
        {
            anyhow::bail!("bound_content_receipt_already_attached");
        }
        let run_id = action
            .react_trace
            .as_ref()
            .and_then(|trace| trace.run_id.as_deref())
            .context("bound_content_receipt_run_identity_missing")?;
        let evidence = admission.into_issue_evidence();
        let field = crate::agent::types::BoundContentField::for_kind(evidence.kind());
        let observed_body = observed_bound_content_body(action, observation, field)?;
        if observed_body != evidence.body() {
            anyhow::bail!("bound_content_receipt_adapter_body_mismatch");
        }
        let observed_binding = crate::agent::types::ContentReceiptBinding::from_action_graph(
            run_id,
            action,
            observation,
            field,
        )?;
        let canonical_action =
            minimize_action(action, ReceiptOrigin::NewInput(self.receipt_key.as_ref()));
        let mut canonical_observation = minimize_observation(
            observation,
            ReceiptOrigin::NewInput(self.receipt_key.as_ref()),
        );
        canonical_observation.react_trace = None;

        let now = chrono::Utc::now().timestamp();
        let expires_at = now
            .checked_add(BOUND_CONTENT_PENDING_TTL_SECONDS)
            .context("bound_content_receipt_expiry_overflow")?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !Self::active_bound_content_owner_exists_on_connection(&tx, run_id)? {
            anyhow::bail!("bound_content_receipt_active_run_owner_missing");
        }
        Self::prune_bound_content_issuance_ledger(&tx, now)?;
        let canonical_store_identity = Self::existing_canonical_store_identity(&tx)?;
        let canonical_binding =
            crate::agent::types::ContentReceiptBinding::from_canonical_action_graph(
                &canonical_store_identity,
                run_id,
                &canonical_action,
                &canonical_observation,
                field,
            )?;
        let receipt = ContentReceipt::issue_durable(
            self.receipt_key.as_ref(),
            evidence,
            &observed_binding,
            &canonical_binding,
        )?;
        let pending_for_run: i64 = tx.query_row(
            "SELECT COUNT(*) FROM bound_content_issuance_ledger
             WHERE state = 'pending' AND run_id = ?1 AND expires_at >= ?2",
            params![run_id, now],
            |row| row.get(0),
        )?;
        if pending_for_run >= MAX_BOUND_CONTENT_PENDING_PER_RUN {
            anyhow::bail!("bound_content_receipt_pending_run_capacity_exceeded");
        }
        let pending_global: i64 = tx.query_row(
            "SELECT COUNT(*) FROM bound_content_issuance_ledger
             WHERE state = 'pending' AND expires_at >= ?1",
            [now],
            |row| row.get(0),
        )?;
        if pending_global >= MAX_BOUND_CONTENT_PENDING_GLOBAL {
            anyhow::bail!("bound_content_receipt_pending_global_capacity_exceeded");
        }
        let receipt_json = serde_json::to_string(&receipt)
            .context("bound_content_receipt_ledger_serialization_failed")?;
        tx.execute(
            "INSERT INTO bound_content_issuance_ledger (
                issuance_id, receipt_id, canonical_store_identity,
                run_id, action_id, observation_id, receipt_json,
                state, issued_at, expires_at, attached_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, ?9, NULL)",
            params![
                receipt.issuance_id(),
                receipt.receipt_id(),
                canonical_store_identity,
                run_id,
                action.id,
                observation.id,
                receipt_json,
                now,
                expires_at,
            ],
        )?;
        tx.commit()?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{AgentRunError, ContextSummary, ModelRouteTrace};
    use crate::persistence_outbox::ProjectionDeliveryState;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct CountingStoreDispatchObserver {
        count: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl crate::agent::ToolDispatchObserver for CountingStoreDispatchObserver {
        async fn before_dispatch(
            &self,
            _attempt: &crate::agent::ToolDispatchAttempt,
        ) -> anyhow::Result<()> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn create_test_run() -> AgentRun {
        AgentRun::new_chat_run("test-session", "Hello world")
    }

    fn bound_content_issuance_count(
        store: &AgentRunStore,
        run_id: &str,
        state: Option<&str>,
    ) -> i64 {
        let conn = store.conn.lock().unwrap();
        match state {
            Some(state) => conn
                .query_row(
                    "SELECT COUNT(*) FROM bound_content_issuance_ledger
                     WHERE run_id = ?1 AND state = ?2",
                    params![run_id, state],
                    |row| row.get(0),
                )
                .unwrap(),
            None => conn
                .query_row(
                    "SELECT COUNT(*) FROM bound_content_issuance_ledger WHERE run_id = ?1",
                    [run_id],
                    |row| row.get(0),
                )
                .unwrap(),
        }
    }

    async fn execute_observed_builtin_tool_output(
        store: &AgentRunStore,
        run_id: &str,
        step_index: u32,
        tool_name: &str,
        tool_id: &str,
        body: &str,
    ) -> anyhow::Result<crate::agent::action_executor::ActionExecutionResult> {
        let mut registry = crate::mcp::McpRegistry::new();
        let mut manifest = crate::tool_manifest::ToolManifest::new(
            tool_name,
            "Observed adapter fixture",
            serde_json::json!({"type": "object"}),
            "low",
            "1",
            crate::tool_manifest::ToolSource::BuiltIn,
        );
        manifest.id = tool_id.into();
        manifest.capabilities = vec!["read".into()];
        manifest.action_type = "read".into();
        manifest.idempotency_contract = crate::tool_manifest::ToolIdempotencyContract::Idempotent;
        let adapter_body = body.to_string();
        registry.register_builtin(
            manifest,
            Box::new(move |_arguments| Ok(adapter_body.clone())),
        );
        let permission_store =
            crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let audit_dir = tempfile::tempdir().unwrap();
        let audit_store = crate::mcp_audit::McpAuditStore::new(audit_dir.path().join("audit.db"));
        let privacy_engine = crate::privacy::PrivacyEngine::new();
        let safe_paths = Vec::<String>::new();
        let context = crate::agent::action_executor::ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &safe_paths,
        )
        .with_agent_run_store(store);
        crate::agent::ToolGateway::from_executor_config(
            crate::agent::action_executor::ActionExecutorConfig::default(),
        )
        .execute(
            crate::agent::action_executor::AgentActionRequest {
                action_type: "builtin_tool".into(),
                target: tool_name.into(),
                input: serde_json::json!({"arguments": {}}),
                source_run_id: Some(run_id.into()),
                step_index,
            },
            &context,
        )
        .await
    }

    async fn observed_builtin_tool_output_graph(
        store: &AgentRunStore,
        run_id: &str,
        step_index: u32,
        tool_name: &str,
        tool_id: &str,
        body: &str,
    ) -> (
        crate::agent::types::AgentAction,
        crate::agent::types::AgentObservation,
    ) {
        let result = execute_observed_builtin_tool_output(
            store, run_id, step_index, tool_name, tool_id, body,
        )
        .await
        .unwrap();
        assert_eq!(
            result.status,
            crate::agent::action_executor::ActionExecutionStatus::Succeeded
        );
        assert!(result
            .action
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .is_some());
        assert!(result.observation.react_trace.is_none());
        (result.action, result.observation)
    }

    async fn observed_builtin_tool_error_graph(
        store: &AgentRunStore,
        run_id: &str,
        step_index: u32,
        tool_name: &str,
        tool_id: &str,
        error_body: &str,
    ) -> (
        crate::agent::types::AgentAction,
        crate::agent::types::AgentObservation,
    ) {
        let mut registry = crate::mcp::McpRegistry::new();
        let mut manifest = crate::tool_manifest::ToolManifest::new(
            tool_name,
            "Observed adapter error fixture",
            serde_json::json!({"type": "object"}),
            "low",
            "1",
            crate::tool_manifest::ToolSource::BuiltIn,
        );
        manifest.id = tool_id.into();
        manifest.capabilities = vec!["read".into()];
        manifest.action_type = "read".into();
        manifest.idempotency_contract = crate::tool_manifest::ToolIdempotencyContract::Idempotent;
        let adapter_error = error_body.to_string();
        registry.register_builtin(
            manifest,
            Box::new(move |_arguments| Err(anyhow::anyhow!(adapter_error.clone()))),
        );
        let permission_store =
            crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let audit_dir = tempfile::tempdir().unwrap();
        let audit_store = crate::mcp_audit::McpAuditStore::new(audit_dir.path().join("audit.db"));
        let privacy_engine = crate::privacy::PrivacyEngine::new();
        let safe_paths = Vec::<String>::new();
        let context = crate::agent::action_executor::ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &safe_paths,
        )
        .with_agent_run_store(store);
        let result = crate::agent::ToolGateway::from_executor_config(
            crate::agent::action_executor::ActionExecutorConfig::default(),
        )
        .execute(
            crate::agent::action_executor::AgentActionRequest {
                action_type: "builtin_tool".into(),
                target: tool_name.into(),
                input: serde_json::json!({"arguments": {}}),
                source_run_id: Some(run_id.into()),
                step_index,
            },
            &context,
        )
        .await
        .unwrap();
        assert_eq!(
            result.status,
            crate::agent::action_executor::ActionExecutionStatus::Failed
        );
        assert_eq!(
            result
                .action
                .react_trace
                .as_ref()
                .and_then(|trace| trace.output_receipt.as_ref())
                .map(|receipt| receipt.kind()),
            Some(crate::agent::types::ContentReceiptKind::ToolError)
        );
        (result.action, result.observation)
    }

    fn install_legacy_raw_columns_for_test(conn: &Connection) {
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        AgentRunStore::add_column_if_missing(conn, "agent_runs", "user_input", "TEXT").unwrap();
        AgentRunStore::add_column_if_missing(conn, "agent_runs", "reasoning_trace_json", "TEXT")
            .unwrap();
    }

    /// Schema snapshots below are copied from the named git revisions. They
    /// are migration evidence for those concrete revisions, not invented
    /// semantic "v1/v2/v3" product versions.
    fn create_git_historical_agent_run_table(conn: &Connection, revision: &str) {
        let base = "
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            session_id TEXT,
            status TEXT NOT NULL,
            kind TEXT NOT NULL,
            user_input TEXT,
            context_summary_json TEXT,
            model_route_json TEXT,
            output_preview TEXT,
            error_json TEXT,
            generated_proposals_json TEXT DEFAULT '[]',
            actions_json TEXT DEFAULT '[]',
            observations_json TEXT DEFAULT '[]',
            deleted_at TEXT,
            delete_reason TEXT,
            started_at TEXT NOT NULL,
            finished_at TEXT";
        let additions = match revision {
            // e71458a (2026-04-27): AgentRun enhancement baseline.
            "e71458a" => "",
            // f872413 (2026-04-28): ReasoningTrace joined AgentRun.
            "f872413" => {
                ",
                reasoning_strategy TEXT,
                reasoning_trace_json TEXT"
            }
            // bace15b (2026-05-29): HS selection audit/check summaries.
            "bace15b" => {
                ",
                reasoning_strategy TEXT,
                reasoning_trace_json TEXT,
                hs_selection_audit_json TEXT,
                behavior_checks_json TEXT DEFAULT '[]'"
            }
            _ => panic!("unsupported git historical schema snapshot"),
        };
        conn.execute_batch(&format!("CREATE TABLE agent_runs ({base}{additions});"))
            .unwrap();
    }

    #[test]
    fn test_create_and_get_run() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run = create_test_run();
        store.create_run(&run).unwrap();

        let fetched = store.get_run(&run.id).unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, run.id);
        assert_eq!(fetched.session_id, Some("test-session".to_string()));
        assert!(fetched.user_input.is_none());
        assert!(fetched.input_ref.is_none());
        assert!(fetched
            .input_digest
            .as_deref()
            .is_some_and(|value| value.starts_with("hmac-sha256:")));
        assert_eq!(fetched.status, AgentRunStatus::Running);
        assert_eq!(fetched.kind, AgentTaskKind::Conversation);
    }

    #[test]
    fn persistent_store_requires_the_same_injected_receipt_key_after_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-runs-keyed.db");
        let first_key = AgentRunReceiptKey::from_bytes([0x11; 32]).unwrap();
        let wrong_key = AgentRunReceiptKey::from_bytes([0x22; 32]).unwrap();

        let store = AgentRunStore::new_with_receipt_key(&path, first_key.clone()).unwrap();
        let run = create_test_run();
        let run_id = run.id.clone();
        store.create_run(&run).unwrap();
        drop(store);

        let reopened = AgentRunStore::new_with_receipt_key(&path, first_key.clone()).unwrap();
        assert!(reopened.get_run(&run_id).unwrap().is_some());
        drop(reopened);

        let error = AgentRunStore::new_with_receipt_key(&path, wrong_key)
            .err()
            .expect("a different key must not authenticate persisted AgentRun receipts")
            .to_string();
        assert!(error.contains("receipt_key_mismatch"), "{error}");

        let raw = Connection::open(&path).unwrap();
        raw.execute(
            "DELETE FROM agent_run_store_metadata WHERE key = 'receipt_key_verifier'",
            [],
        )
        .unwrap();
        drop(raw);
        let error = AgentRunStore::new_with_receipt_key(&path, first_key)
            .err()
            .expect("current rows without their key binding must fail closed")
            .to_string();
        assert!(
            error.contains("receipt_key_binding_missing_for_current_rows"),
            "{error}"
        );
    }

    #[test]
    fn input_receipts_are_opaque_across_keys_and_bound_to_the_run_identity() {
        assert!(AgentRunReceiptKey::from_bytes([0; 32]).is_err());
        let first_key = AgentRunReceiptKey::from_bytes([0x31; 32]).unwrap();
        let second_key = AgentRunReceiptKey::from_bytes([0x32; 32]).unwrap();
        let first = AgentRunStore::new_in_memory_with_receipt_key(first_key.clone()).unwrap();
        let second = AgentRunStore::new_in_memory_with_receipt_key(second_key).unwrap();
        let third = AgentRunStore::new_in_memory_with_receipt_key(first_key.clone()).unwrap();

        let mut first_run = create_test_run();
        first_run.id = "same-body-run-a".into();
        first_run.task_id = "same-body-task-a".into();
        let second_run = first_run.clone();
        let mut third_run = first_run.clone();
        third_run.id = "same-body-run-b".into();
        third_run.task_id = "same-body-task-b".into();

        first.create_run(&first_run).unwrap();
        second.create_run(&second_run).unwrap();
        third.create_run(&third_run).unwrap();
        let first_digest = first.get_run(&first_run.id).unwrap().unwrap().input_digest;
        let second_digest = second
            .get_run(&second_run.id)
            .unwrap()
            .unwrap()
            .input_digest;
        let third_digest = third.get_run(&third_run.id).unwrap().unwrap().input_digest;

        assert_ne!(first_digest, second_digest);
        assert_ne!(first_digest, third_digest);
        assert_eq!(format!("{first_key:?}"), "AgentRunReceiptKey([REDACTED])");
    }

    #[test]
    fn list_runs_pagination_is_stable_when_started_at_timestamps_are_equal() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let same_started_at = chrono::Utc::now();
        for index in 0..260 {
            let mut run = create_test_run();
            run.id = format!("run-{index:04}");
            run.task_id = format!("task-{index:04}");
            run.started_at = same_started_at;
            store.create_run(&run).unwrap();
        }

        let first = store.list_runs(250, 0).unwrap();
        let second = store.list_runs(250, 250).unwrap();
        let ids = first
            .iter()
            .chain(second.iter())
            .map(|run| run.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), 260);
        assert_eq!(
            ids.iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            260
        );
        assert!(ids.windows(2).all(|pair| pair[0] > pair[1]));
    }

    #[test]
    fn proposal_link_projection_is_atomic_with_agent_run_create_and_update() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        run.generated_proposals = vec!["proposal-a".into(), "proposal-a".into()];
        store.create_run(&run).unwrap();

        assert_eq!(
            store.get_run(&run.id).unwrap().unwrap().generated_proposals,
            vec!["proposal-a"]
        );
        let linked_a = store.list_runs_linked_to_proposal("proposal-a").unwrap();
        assert_eq!(linked_a.len(), 1);
        assert_eq!(linked_a[0].id, run.id);

        run.generated_proposals = vec!["proposal-b".into()];
        store.update_run(&run).unwrap();
        assert!(store
            .list_runs_linked_to_proposal("proposal-a")
            .unwrap()
            .is_empty());
        let linked_b = store.list_runs_linked_to_proposal("proposal-b").unwrap();
        assert_eq!(linked_b.len(), 1);
        assert_eq!(linked_b[0].id, run.id);
    }

    #[test]
    fn add_generated_proposal_updates_json_and_link_projection_atomically() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run = create_test_run();
        let run_id = run.id.clone();
        store.create_run(&run).unwrap();

        store
            .add_generated_proposal(&run_id, "proposal-added-later")
            .unwrap();
        store
            .add_generated_proposal(&run_id, "proposal-added-later")
            .unwrap();
        assert!(store
            .add_generated_proposal(&run_id, " proposal-with-whitespace ")
            .is_err());

        let stored = store.get_run(&run_id).unwrap().unwrap();
        assert_eq!(stored.generated_proposals, vec!["proposal-added-later"]);
        let linked = store
            .list_runs_linked_to_proposal("proposal-added-later")
            .unwrap();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].id, run_id);
    }

    #[test]
    fn proposal_link_legacy_backfill_runs_once_not_on_every_open() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("agent-runs.db");
        let run_id = {
            let store = AgentRunStore::new(&db_path).unwrap();
            let mut run = create_test_run();
            run.generated_proposals = vec!["proposal-once".into()];
            let run_id = run.id.clone();
            store.create_run(&run).unwrap();
            run_id
        };
        let connection = rusqlite::Connection::open(&db_path).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_redundant_link_backfill
                 BEFORE DELETE ON agent_run_proposal_links
                 BEGIN
                   SELECT RAISE(ABORT, 'proposal links must not be rebuilt on every open');
                 END;",
            )
            .unwrap();
        drop(connection);

        let reopened = AgentRunStore::new(&db_path).unwrap();
        let linked = reopened
            .list_runs_linked_to_proposal("proposal-once")
            .unwrap();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].id, run_id);
    }

    #[test]
    fn waiting_permission_reconciliation_query_uses_status_and_link_indexes() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        run.status = AgentRunStatus::WaitingPermission;
        run.generated_proposals = vec!["proposal-indexed".into()];
        store.create_run(&run).unwrap();

        let connection = store.conn.lock().unwrap();
        let query = format!(
            "EXPLAIN QUERY PLAN
             SELECT DISTINCT link.proposal_id
             FROM agent_runs AS run INDEXED BY idx_agent_runs_waiting_permission
             CROSS JOIN agent_run_proposal_links AS link
             WHERE run.status = 'waiting_permission'
               AND {LIVE_AGENT_RUN_SQL_PREDICATE}
               AND link.run_id = run.id
             LIMIT 200"
        );
        let mut statement = connection.prepare(&query).unwrap();
        let details = statement
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");
        assert!(
            details.contains("idx_agent_runs_waiting_permission"),
            "{details}"
        );
        assert!(
            details.contains("SEARCH link USING PRIMARY KEY"),
            "{details}"
        );
    }

    #[test]
    fn test_update_run() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        store.create_run(&run).unwrap();

        let model_route = ModelRouteTrace {
            provider: "openrouter".to_string(),
            model: "deepseek-chat".to_string(),
            route_type: "cloud".to_string(),
            prefer_local: false,
            local_model: "llama3.2".to_string(),
            reason: "no_ollama".to_string(),
            privacy_level: crate::agent::types::RedactionLevel::None,
            latency_ms: None,
            retry_count: 0,
            fallback_reason: Some("no_ollama".to_string()),
            provider_health_is_estimated: Some(true),
        };
        let context_summary = ContextSummary {
            life_model_empty: true,
            included_life_model_sections: vec![],
            memory_hit_count: 0,
            memory_sources: vec![],
            used_tools_prompt: false,
            redaction_applied: false,
            redaction_level: crate::agent::types::RedactionLevel::None,
        };
        run.complete("Hello! How can I help?", model_route, context_summary);
        store.update_run(&run).unwrap();

        let fetched = store.get_run(&run.id).unwrap().unwrap();
        assert_eq!(fetched.status, AgentRunStatus::Completed);
        assert!(
            fetched
                .output_preview
                .as_deref()
                .is_some_and(|value| value.starts_with("run_output:bytes=")
                    && value.contains("hmac-sha256:")),
            "AgentRun must retain only an output receipt, not a second response body"
        );
        assert!(fetched.model_route.is_some());
        assert!(fetched.context_summary.is_some());
        assert!(fetched.finished_at.is_some());
    }

    #[test]
    fn update_missing_run_fails_instead_of_reporting_a_phantom_success() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run = create_test_run();

        let error = store.update_run(&run).unwrap_err().to_string();

        assert!(error.contains("agent_run_update_missing"));
        assert!(store.get_run(&run.id).unwrap().is_none());
    }

    #[test]
    fn canonical_task_id_allows_only_one_non_deleted_run_even_after_terminal_status() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut first = AgentRun::new_chat_run("canonical-task-session", "first");
        first.task_id = "canonical-task-id".into();
        store.create_run(&first).unwrap();
        first.status = AgentRunStatus::Completed;
        first.finished_at = Some(chrono::Utc::now());
        store.update_run(&first).unwrap();

        let mut competing = AgentRun::new_chat_run("canonical-task-session", "second");
        competing.task_id = first.task_id.clone();
        let error = store.create_run(&competing).unwrap_err().to_string();
        assert!(error.contains("UNIQUE constraint failed"));
        assert_eq!(
            store
                .get_run_for_task_id(&first.task_id)
                .unwrap()
                .unwrap()
                .id,
            first.id
        );

        store
            .delete_run_with_tombstone(&first.id, Some("explicitly retired canonical history"))
            .unwrap();
        store.create_run(&competing).unwrap();
        assert_eq!(
            store
                .get_run_for_task_id(&competing.task_id)
                .unwrap()
                .unwrap()
                .id,
            competing.id
        );
    }

    #[test]
    fn test_list_runs() {
        let store = AgentRunStore::new_in_memory().unwrap();
        for i in 0..5 {
            let run = AgentRun::new_chat_run("session-1", &format!("msg {}", i));
            store.create_run(&run).unwrap();
        }

        let runs = store.list_runs(10, 0).unwrap();
        assert_eq!(runs.len(), 5);

        let session_runs = store.list_runs_for_session("session-1", 10).unwrap();
        assert_eq!(session_runs.len(), 5);
    }

    #[test]
    fn test_run_count() {
        let store = AgentRunStore::new_in_memory().unwrap();
        assert_eq!(store.run_count().unwrap(), 0);

        let run = create_test_run();
        store.create_run(&run).unwrap();
        assert_eq!(store.run_count().unwrap(), 1);
    }

    #[test]
    fn test_last_run_for_session() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run1 = AgentRun::new_chat_run("session-1", "first");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let run2 = AgentRun::new_chat_run("session-1", "second");
        store.create_run(&run1).unwrap();
        store.create_run(&run2).unwrap();

        let last = store.last_run_for_session("session-1").unwrap();
        assert!(last.is_some());
        let last = last.unwrap();
        assert!(last.user_input.is_none());
        assert!(last
            .input_digest
            .as_deref()
            .is_some_and(|value| value.starts_with("hmac-sha256:")));
    }

    #[test]
    fn persisted_run_keeps_references_and_digests_not_raw_input_or_reasoning_trace() {
        const SECRET: &str = "AGENT_RUN_RAW_PRIVATE_SENTINEL";
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = AgentRun::new_chat_run("session-private", SECRET);
        run.output_preview = Some(SECRET.into());
        run.error = Some(crate::agent::types::AgentRunError {
            message: SECRET.into(),
            phase: "model".into(),
            recoverable: false,
        });
        run.model_route = Some(crate::agent::types::ModelRouteTrace {
            provider: "openai".into(),
            model: "test-model".into(),
            route_type: "cloud".into(),
            prefer_local: false,
            local_model: "local-model".into(),
            reason: SECRET.into(),
            privacy_level: crate::agent::types::RedactionLevel::Strict,
            latency_ms: None,
            retry_count: 0,
            fallback_reason: Some(SECRET.into()),
            provider_health_is_estimated: Some(false),
        });
        run.actions.push(crate::agent::types::AgentAction {
            id: "action-private".into(),
            action_type: "mcp_tool".into(),
            target: Some(SECRET.into()),
            input: serde_json::json!({ "private": SECRET }),
            output: Some(serde_json::json!({ "private": SECRET })),
            status: "failed".into(),
            permission_decision: Some("blocked".into()),
            started_at: Some(chrono::Utc::now()),
            finished_at: Some(chrono::Utc::now()),
            error: Some(SECRET.into()),
            timestamp: chrono::Utc::now(),
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: None,
        });
        run.observations
            .push(crate::agent::types::AgentObservation {
                id: "observation-private".into(),
                action_id: Some("action-private".into()),
                content: SECRET.into(),
                source: SECRET.into(),
                structured_result: Some(serde_json::json!({ "private": SECRET })),
                timestamp: chrono::Utc::now(),
                react_trace: None,
            });
        run.status_updates
            .push(crate::agent::types::AgentLoopStatusUpdate {
                phase: crate::agent::types::AgentLoopPhase::Failed,
                message: SECRET.into(),
                step_index: 1,
                tool_call_index: Some(1),
                timestamp: chrono::Utc::now(),
            });
        run.delete_reason = Some(SECRET.into());
        run.reasoning_trace = Some(crate::agent::reasoning::ReasoningTrace {
            input: Some(SECRET.into()),
            ..Default::default()
        });

        store.create_run(&run).unwrap();

        let conn = store.conn.lock().unwrap();
        let columns = conn
            .prepare("PRAGMA table_info(agent_runs)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(!columns.iter().any(|column| column == "user_input"));
        assert!(!columns
            .iter()
            .any(|column| column == "reasoning_trace_json"));
        let persisted: (Option<String>, Option<String>, String) = conn
            .query_row(
                "SELECT input_ref, reasoning_trace_digest,
                        COALESCE(model_route_json, '') || COALESCE(output_preview, '') ||
                        COALESCE(error_json, '') || COALESCE(actions_json, '') ||
                        COALESCE(observations_json, '') || COALESCE(status_updates_json, '') ||
                        COALESCE(delete_reason, '')
                 FROM agent_runs WHERE id = ?1",
                [&run.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert!(persisted.0.is_none());
        assert!(persisted
            .1
            .as_deref()
            .is_some_and(|value| value.starts_with("hmac-sha256:")));
        assert!(persisted.2.contains("contentStored"));
        assert!(persisted.2.contains("hmac-sha256:"));
        assert!(!format!("{persisted:?}").contains(SECRET));
    }

    #[test]
    fn v7_rebuild_physically_removes_raw_columns_wal_pages_and_freelist() {
        const RAW_SENTINEL: &str = "AGENT_RUN_V7_PHYSICAL_RAW_SENTINEL";
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-runs-v6-physical.db");
        {
            let conn = Connection::open(&path).unwrap();
            create_git_historical_agent_run_table(&conn, "f872413");
            conn.execute(
                "INSERT INTO agent_runs (
                    id, task_id, session_id, status, kind, user_input,
                    reasoning_trace_json, started_at
                 ) VALUES (?1, ?2, ?3, 'running', 'conversation', ?4, ?5, ?6)",
                params![
                    "physical-v6-run",
                    "physical-v6-task",
                    "physical-v6-session",
                    RAW_SENTINEL,
                    serde_json::json!({"private": RAW_SENTINEL}).to_string(),
                    chrono::Utc::now().to_rfc3339(),
                ],
            )
            .unwrap();
        }

        let key = AgentRunReceiptKey::from_bytes([0x31; 32]).unwrap();
        let migrated = AgentRunStore::new_with_receipt_key(&path, key).unwrap();
        assert!(migrated.get_run("physical-v6-run").unwrap().is_some());
        drop(migrated);

        let conn = Connection::open(&path).unwrap();
        let columns = conn
            .prepare("PRAGMA table_info(agent_runs)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(!columns.iter().any(|column| column == "user_input"));
        assert!(!columns
            .iter()
            .any(|column| column == "reasoning_trace_json"));
        let freelist: i64 = conn
            .query_row("PRAGMA freelist_count", [], |row| row.get(0))
            .unwrap();
        assert_eq!(freelist, 0);
        let physical_purge_marker: String = conn
            .query_row(
                "SELECT value FROM agent_run_store_metadata WHERE key = ?1",
                [AGENT_RUN_V7_PHYSICAL_PURGE_MARKER],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(physical_purge_marker, "complete");
        drop(conn);

        for candidate in [
            path.clone(),
            PathBuf::from(format!("{}-wal", path.display())),
            PathBuf::from(format!("{}-shm", path.display())),
        ] {
            if candidate.exists() {
                let bytes = std::fs::read(&candidate).unwrap();
                assert!(
                    !bytes
                        .windows(RAW_SENTINEL.len())
                        .any(|window| window == RAW_SENTINEL.as_bytes()),
                    "raw sentinel survived physical migration in {}",
                    candidate.display()
                );
            }
        }
    }

    #[test]
    fn fresh_v7_writable_store_marks_physical_purge_and_reopens_read_only() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-runs-v7-fresh.db");
        let key = AgentRunReceiptKey::from_bytes([0x36; 32]).unwrap();
        let run = create_test_run();
        let run_id = run.id.clone();

        let writable = AgentRunStore::new_with_receipt_key(&path, key.clone()).unwrap();
        writable.create_run(&run).unwrap();
        {
            let conn = writable.conn.lock().unwrap();
            assert!(AgentRunStore::agent_run_v7_physical_purge_complete(&conn).unwrap());
        }
        drop(writable);

        let read_only =
            AgentRunStore::open_read_only_existing_with_receipt_key(&path, key).unwrap();
        assert!(read_only.get_run(&run_id).unwrap().is_some());
        let conn = read_only.conn.lock().unwrap();
        assert!(AgentRunStore::agent_run_v7_physical_purge_complete(&conn).unwrap());
    }

    #[test]
    fn v7_rebuild_failure_rolls_back_before_old_table_is_dropped() {
        const RAW_SENTINEL: &str = "AGENT_RUN_V7_ROLLBACK_SENTINEL";
        let store = AgentRunStore::new_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();
        install_legacy_raw_columns_for_test(&conn);
        conn.execute(
            "INSERT INTO agent_runs (
                id, task_id, status, kind, started_at, user_input,
                payload_minimized_version
             ) VALUES ('rebuild-rollback-run', 'rebuild-rollback-task',
                       'running', 'conversation', ?1, ?2, ?3)",
            params![
                chrono::Utc::now().to_rfc3339(),
                RAW_SENTINEL,
                AGENT_RUN_PAYLOAD_VERSION,
            ],
        )
        .unwrap();

        let error = AgentRunStore::rebuild_agent_runs_without_raw_columns(
            &conn,
            AgentRunTableRebuildFault::AfterCopy,
        )
        .expect_err("fault after copy must roll back the whole table replacement")
        .to_string();
        assert!(error.contains("injected_agent_run_v7_rebuild_failure"));
        assert!(AgentRunStore::column_exists(&conn, "agent_runs", "user_input").unwrap());
        let raw: String = conn
            .query_row(
                "SELECT user_input FROM agent_runs WHERE id = 'rebuild-rollback-run'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raw, RAW_SENTINEL);

        AgentRunStore::rebuild_agent_runs_without_raw_columns(
            &conn,
            AgentRunTableRebuildFault::None,
        )
        .unwrap();
        assert!(!AgentRunStore::column_exists(&conn, "agent_runs", "user_input").unwrap());
    }

    #[test]
    fn v7_rebuild_recovers_after_table_swap_before_physical_purge() {
        const RAW_SENTINEL: &str = "AGENT_RUN_V7_CRASH_WINDOW_RAW_SENTINEL";
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-runs-v7-crash-recovery.db");
        let key = AgentRunReceiptKey::from_bytes([0x37; 32]).unwrap();

        let store = AgentRunStore::new_with_receipt_key(&path, key.clone()).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            install_legacy_raw_columns_for_test(&conn);
            conn.execute(
                "INSERT INTO agent_runs (
                    id, task_id, status, kind, started_at, user_input,
                    reasoning_trace_json, payload_minimized_version
                 ) VALUES ('rebuild-crash-run', 'rebuild-crash-task',
                           'running', 'conversation', ?1, ?2, ?3, ?4)",
                params![
                    chrono::Utc::now().to_rfc3339(),
                    RAW_SENTINEL,
                    serde_json::json!({ "private": RAW_SENTINEL }).to_string(),
                    AGENT_RUN_PAYLOAD_VERSION,
                ],
            )
            .unwrap();

            let error = AgentRunStore::rebuild_agent_runs_without_raw_columns(
                &conn,
                AgentRunTableRebuildFault::AfterTableSwapBeforePurge,
            )
            .expect_err("the injected crash window must stop before physical purge")
            .to_string();
            assert!(error.contains("failure_before_physical_purge"), "{error}");
            assert!(!AgentRunStore::column_exists(&conn, "agent_runs", "user_input").unwrap());
            assert!(
                !AgentRunStore::column_exists(&conn, "agent_runs", "reasoning_trace_json").unwrap()
            );
            let marker: String = conn
                .query_row(
                    "SELECT value FROM agent_run_store_metadata WHERE key = ?1",
                    [AGENT_RUN_V7_PHYSICAL_PURGE_MARKER],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(marker, "pending");
        }
        drop(store);

        let read_only_error =
            AgentRunStore::open_read_only_existing_with_receipt_key(&path, key.clone())
                .err()
                .expect("read-only startup must fail closed while physical purge is pending")
                .to_string();
        assert!(
            read_only_error.contains("physical_purge_incomplete"),
            "{read_only_error}"
        );

        let recovered = AgentRunStore::new_with_receipt_key(&path, key).unwrap();
        assert!(recovered.get_run("rebuild-crash-run").unwrap().is_some());
        {
            let conn = recovered.conn.lock().unwrap();
            let marker: String = conn
                .query_row(
                    "SELECT value FROM agent_run_store_metadata WHERE key = ?1",
                    [AGENT_RUN_V7_PHYSICAL_PURGE_MARKER],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(marker, "complete");
            let freelist: i64 = conn
                .query_row("PRAGMA freelist_count", [], |row| row.get(0))
                .unwrap();
            assert_eq!(freelist, 0);
        }
        drop(recovered);

        for candidate in [
            path.clone(),
            PathBuf::from(format!("{}-wal", path.display())),
            PathBuf::from(format!("{}-shm", path.display())),
        ] {
            if candidate.exists() {
                let bytes = std::fs::read(&candidate).unwrap();
                assert!(
                    !bytes
                        .windows(RAW_SENTINEL.len())
                        .any(|window| window == RAW_SENTINEL.as_bytes()),
                    "raw sentinel survived crash recovery in {}",
                    candidate.display()
                );
            }
        }
    }

    #[test]
    fn caller_shaped_text_and_value_receipts_are_minimized_as_new_input() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let forged_text = format!("run_output:bytes=1:hmac-sha256:{}", "a".repeat(64));
        let forged_error = format!("run_error:bytes=1:hmac-sha256:{}", "b".repeat(64));
        let forged_value = serde_json::json!({
            "kind": "action_input",
            "byteCount": 1,
            "digest": format!("hmac-sha256:{}", "c".repeat(64)),
            "contentStored": false,
        });
        let mut run = create_test_run();
        run.output_preview = Some(forged_text.clone());
        run.error = Some(crate::agent::types::AgentRunError {
            message: forged_error.clone(),
            phase: "model".into(),
            recoverable: false,
        });
        run.actions.push(crate::agent::types::AgentAction {
            id: "action-forged-receipt".into(),
            action_type: "memory.search".into(),
            target: None,
            input: forged_value.clone(),
            output: None,
            status: "failed".into(),
            permission_decision: Some("blocked".into()),
            started_at: None,
            finished_at: Some(chrono::Utc::now()),
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: None,
        });
        store.create_run(&run).unwrap();

        let mut stored = store.get_run(&run.id).unwrap().unwrap();
        assert_ne!(stored.output_preview.as_deref(), Some(forged_text.as_str()));
        assert_ne!(
            stored.error.as_ref().map(|error| error.message.as_str()),
            Some(forged_error.as_str())
        );
        assert_ne!(stored.actions[0].input, forged_value);
        assert!(is_metadata_safe_value_receipt(
            "action_input",
            &stored.actions[0].input
        ));

        let output_receipt = stored.output_preview.clone();
        let input_receipt = stored.actions[0].input.clone();
        stored.status = AgentRunStatus::Completed;
        stored.actions[0].status = "succeeded".into();
        stored.finished_at = Some(chrono::Utc::now());
        store.update_run(&stored).unwrap();
        let reread = store.get_run(&run.id).unwrap().unwrap();
        assert_eq!(reread.output_preview, output_receipt);
        assert_eq!(reread.actions[0].input, input_receipt);
        assert_eq!(reread.actions[0].status, "succeeded");
    }

    #[test]
    fn stored_text_receipt_requires_canonical_lowercase_digest_and_byte_count() {
        assert!(is_metadata_safe_text_receipt(
            "run_output",
            &format!("run_output:bytes=1:hmac-sha256:{}", "a".repeat(64))
        ));
        assert!(!is_metadata_safe_text_receipt(
            "run_output",
            &format!("run_output:bytes=01:hmac-sha256:{}", "a".repeat(64))
        ));
        assert!(!is_metadata_safe_text_receipt(
            "run_output",
            &format!("run_output:bytes=1:hmac-sha256:{}", "A".repeat(64))
        ));
        assert!(!is_metadata_safe_text_receipt(
            "run_output",
            &format!(
                "run_output:bytes={}:hmac-sha256:{}",
                MAX_AGENT_RUN_RECEIPT_BYTES + 1,
                "a".repeat(64)
            )
        ));
    }

    #[test]
    fn synthetic_pre_v6_fixture_does_not_authenticate_receipt_shapes_by_version_marker() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-run-v2-receipt-upgrade.db");
        let run_id = "legacy-v2-receipt-fixture";
        let output_receipt = format!("run_output:bytes=1:sha256:{}", "a".repeat(64));
        let input_receipt = serde_json::json!({
            "kind": "action_input",
            "byteCount": 1,
            "digest": format!("sha256:{}", "b".repeat(64)),
            "contentStored": false
        });
        {
            let store = AgentRunStore::new(&path).unwrap();
            // This fixture represents a pre-v7 database. A current v7 table
            // with only a downgraded version marker is deliberately rejected,
            // because it has no trustworthy legacy raw-source columns from
            // which the one-way minimizer can authenticate its migration.
            install_legacy_raw_columns_for_test(&store.conn.lock().unwrap());
            let action = crate::agent::types::AgentAction {
                id: "action-v2-receipt".into(),
                action_type: "memory.search".into(),
                target: None,
                input: input_receipt.clone(),
                output: None,
                status: "succeeded".into(),
                permission_decision: Some("read_only_memory_search".into()),
                started_at: None,
                finished_at: Some(chrono::Utc::now()),
                error: None,
                timestamp: chrono::Utc::now(),
                tool_scope: None,
                react_trace: None,
                runtime_execution_receipt: None,
            };
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO agent_runs
                        (id, task_id, status, kind, started_at, output_preview,
                         actions_json, payload_minimized_version)
                     VALUES (?1, ?2, 'running', 'conversation', ?3, ?4, ?5, 2)",
                    params![
                        run_id,
                        "legacy-v2-task-fixture",
                        chrono::Utc::now().to_rfc3339(),
                        &output_receipt,
                        serde_json::to_string(&vec![action]).unwrap(),
                    ],
                )
                .unwrap();
        }

        let reopened = AgentRunStore::new(&path).unwrap();
        let upgraded = reopened.get_run(&run_id).unwrap().unwrap();
        assert!(upgraded.legacy_payload_unverified);
        assert_ne!(
            upgraded.output_preview.as_deref(),
            Some(output_receipt.as_str())
        );
        assert_ne!(upgraded.actions[0].input, input_receipt);
        let (version, unverified): (i64, i64) = reopened
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT payload_minimized_version, legacy_payload_unverified
                 FROM agent_runs WHERE id = ?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(version, AGENT_RUN_PAYLOAD_VERSION);
        assert_eq!(unverified, 1);
        let stable_output = upgraded.output_preview.clone();
        let stable_input = upgraded.actions[0].input.clone();
        drop(reopened);
        let reopened_again = AgentRunStore::new(&path).unwrap();
        let stable = reopened_again.get_run(run_id).unwrap().unwrap();
        assert_eq!(stable.output_preview, stable_output);
        assert_eq!(stable.actions[0].input, stable_input);
    }

    #[test]
    fn synthetic_legacy_payload_fixture_is_minimized_atomically_and_marked_unverified() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let secret = "SYNTHETIC_LEGACY_PRIVATE_FIXTURE";
        install_legacy_raw_columns_for_test(&store.conn.lock().unwrap());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO agent_runs
                    (id, task_id, status, kind, started_at, user_input,
                     output_preview, payload_minimized_version)
                 VALUES ('legacy-v1-fixture', 'legacy-v1-task', 'running',
                         'conversation', ?1, ?2, ?2, 1)",
                params![chrono::Utc::now().to_rfc3339(), secret],
            )
            .unwrap();
        AgentRunStore::minimize_legacy_run_payloads(
            &store.conn.lock().unwrap(),
            store.receipt_key.as_ref(),
        )
        .unwrap();
        AgentRunStore::rebuild_agent_runs_without_raw_columns(
            &store.conn.lock().unwrap(),
            AgentRunTableRebuildFault::None,
        )
        .unwrap();
        let (payload, version, unverified): (String, i64, i64) = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COALESCE(output_preview, ''),
                        payload_minimized_version, legacy_payload_unverified
                 FROM agent_runs WHERE id = 'legacy-v1-fixture'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert!(!payload.contains(secret));
        assert_eq!(version, AGENT_RUN_PAYLOAD_VERSION);
        assert_eq!(unverified, 1);
    }

    #[test]
    fn git_historical_schema_snapshots_migrate_without_promoting_fake_evidence() {
        const PRIVATE_BODY: &str = "HISTORICAL_AGENT_RUN_PRIVATE_BODY";
        let directory = tempfile::tempdir().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        let v1_path = directory.path().join("agent-run-e71458a.db");
        {
            let conn = Connection::open(&v1_path).unwrap();
            create_git_historical_agent_run_table(&conn, "e71458a");
            conn.execute(
                "INSERT INTO agent_runs
                    (id, task_id, session_id, status, kind, user_input, output_preview,
                     started_at)
                 VALUES ('historical-e71458a', 'historical-task-e71458a',
                         'historical-session-e71458a', 'running', 'conversation', ?1, ?1, ?2)",
                params![PRIVATE_BODY, &now],
            )
            .unwrap();
        }
        let v1 = AgentRunStore::new(&v1_path)
            .unwrap()
            .get_run("historical-e71458a")
            .unwrap()
            .unwrap();
        assert!(v1.legacy_payload_unverified);
        assert!(v1.user_input.is_none());
        assert!(v1
            .input_ref
            .as_deref()
            .is_some_and(is_explicit_legacy_unresolvable_ref));
        assert!(v1
            .input_digest
            .as_deref()
            .is_some_and(is_exact_metadata_digest));
        assert!(!serde_json::to_string(&v1).unwrap().contains(PRIVATE_BODY));

        let legacy_trace = serde_json::json!([{
            "id": "historical-action-v2",
            "actionType": "memory.search",
            "input": {"query": PRIVATE_BODY},
            "output": null,
            "status": "succeeded",
            "timestamp": &now,
            "reactTrace": {
                "actionId": "historical-action-v2",
                "stepIndex": 1,
                "toolCallIndex": 1,
                "actionType": "read",
                "toolId": "memory.search",
                "toolName": "memory_search",
                "toolSource": "builtin",
                "actionCategory": "read",
                "riskLevel": "low",
                "status": "succeeded",
                "outputHash": format!("sha256:{}", "a".repeat(64)),
                "outputByteCount": 1,
                "metadataSafe": true
            }
        }]);
        let forged_run_output = format!("run_output:bytes=1:sha256:{}", "b".repeat(64));
        let v2_path = directory.path().join("agent-run-f872413.db");
        {
            let conn = Connection::open(&v2_path).unwrap();
            create_git_historical_agent_run_table(&conn, "f872413");
            conn.execute(
                "INSERT INTO agent_runs
                    (id, task_id, session_id, status, kind, output_preview, actions_json,
                     started_at)
                 VALUES ('historical-f872413', 'historical-task-f872413',
                         'historical-session-f872413', 'running', 'conversation', ?1, ?2, ?3)",
                params![&forged_run_output, legacy_trace.to_string(), &now],
            )
            .unwrap();
        }
        let v2 = AgentRunStore::new(&v2_path)
            .unwrap()
            .get_run("historical-f872413")
            .unwrap()
            .unwrap();
        assert!(v2.legacy_payload_unverified);
        assert_ne!(
            v2.output_preview.as_deref(),
            Some(forged_run_output.as_str())
        );
        assert!(v2.actions[0]
            .react_trace
            .as_ref()
            .unwrap()
            .output_receipt
            .is_none());
        assert!(!serde_json::to_string(&v2).unwrap().contains(PRIVATE_BODY));

        let forged_receipt_trace = serde_json::json!([{
            "id": "historical-action-v3",
            "actionType": "memory.search",
            "input": {},
            "output": null,
            "status": "succeeded",
            "timestamp": &now,
            "reactTrace": {
                "actionId": "historical-action-v3",
                "stepIndex": 1,
                "toolCallIndex": 1,
                "actionType": "read",
                "toolId": "memory.search",
                "toolName": "memory_search",
                "toolSource": "builtin",
                "actionCategory": "read",
                "riskLevel": "low",
                "status": "succeeded",
                "outputReceipt": {
                    "receiptId": uuid::Uuid::new_v4().to_string(),
                    "kind": "tool_output",
                    "provenance": "observed_tool_adapter_body",
                    "byteCount": 1,
                    "digest": format!("sha256:{}", "c".repeat(64))
                },
                "metadataSafe": true
            }
        }]);
        let v3_path = directory.path().join("agent-run-bace15b.db");
        {
            let conn = Connection::open(&v3_path).unwrap();
            create_git_historical_agent_run_table(&conn, "bace15b");
            conn.execute(
                "INSERT INTO agent_runs
                    (id, task_id, session_id, status, kind, actions_json, started_at)
                 VALUES ('historical-bace15b', 'historical-task-bace15b',
                         'historical-session-bace15b', 'running', 'conversation', ?1, ?2)",
                params![forged_receipt_trace.to_string(), &now],
            )
            .unwrap();
        }
        let v3 = AgentRunStore::new(&v3_path)
            .unwrap()
            .get_run("historical-bace15b")
            .unwrap()
            .unwrap();
        assert!(v3.legacy_payload_unverified);
        assert!(v3.input_ref.is_none());
        assert!(v3.input_digest.is_none());
        assert!(v3.reasoning_trace_digest.is_none());
        assert!(v3.actions[0]
            .react_trace
            .as_ref()
            .unwrap()
            .output_receipt
            .is_none());
    }

    #[test]
    fn typed_tool_status_reason_and_source_metadata_remain_product_usable() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        run.actions.push(crate::agent::types::AgentAction {
            id: "action-typed-metadata".into(),
            action_type: "memory.search".into(),
            target: None,
            input: serde_json::json!({"query": "private body"}),
            output: None,
            status: "succeeded".into(),
            permission_decision: Some("read_only_memory_search".into()),
            started_at: Some(chrono::Utc::now()),
            finished_at: Some(chrono::Utc::now()),
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: Some(crate::agent::types::ToolActionScope {
                tool_id: "memory.search".into(),
                tool_name: "memory_search".into(),
                source: "builtin".into(),
                risk_level: "low".into(),
                capabilities: vec!["memory.read".into()],
                action_type: "read_only".into(),
                requires_confirmation: false,
                allowed: true,
            }),
            react_trace: None,
            runtime_execution_receipt: None,
        });
        run.observations
            .push(crate::agent::types::AgentObservation {
                id: "observation-typed-metadata".into(),
                action_id: Some("action-typed-metadata".into()),
                content: "private observation body".into(),
                source: "memory_search".into(),
                structured_result: None,
                timestamp: chrono::Utc::now(),
                react_trace: None,
            });
        store.create_run(&run).unwrap();

        let stored = store.get_run(&run.id).unwrap().unwrap();
        let action = &stored.actions[0];
        assert_eq!(action.action_type, "memory.search");
        assert_eq!(action.status, "succeeded");
        assert_eq!(
            action.permission_decision.as_deref(),
            Some("read_only_memory_search")
        );
        let scope = action.tool_scope.as_ref().unwrap();
        assert_eq!(scope.tool_id, "memory.search");
        assert_eq!(scope.tool_name, "memory_search");
        assert_eq!(scope.source, "builtin");
        assert_eq!(stored.observations[0].source, "memory_search");
        assert!(stored.observations[0]
            .content
            .starts_with("observation_content:bytes="));
    }

    #[test]
    fn canonical_conversation_message_reference_requires_unforgeable_memory_proof() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        run.input_ref = Some("conversation://test-session/message/42".into());
        let error = store.create_run(&run).unwrap_err().to_string();
        assert!(error.contains("canonical_input_proof_required"), "{error}");
    }

    #[test]
    fn canonical_conversation_message_proof_binds_owner_reference_and_digest() {
        let memory = crate::memory::MemoryStore::new_in_memory().unwrap();
        memory
            .create_chat_session("test-session", "proof session")
            .unwrap();
        let message = crate::llm::ChatMessage {
            role: "user".into(),
            content: "Hello world".into(),
        };
        let commit = memory
            .save_message_idempotent_with_proof("test-session", &message, "proof-operation")
            .unwrap();
        let store = AgentRunStore::new_in_memory().unwrap();
        store.bind_canonical_memory_store(&memory).unwrap();
        let mut run = create_test_run();
        run.input_ref = Some(commit.receipt().canonical_ref.clone());
        memory
            .create_agent_run_from_active_conversation_message(&store, &run, commit.proof())
            .unwrap();
        assert_eq!(
            store
                .get_run(&run.id)
                .unwrap()
                .unwrap()
                .input_ref
                .as_deref(),
            Some(commit.receipt().canonical_ref.as_str())
        );

        let mut forged = create_test_run();
        forged.input_ref = Some(commit.receipt().canonical_ref.clone());
        forged.input_digest = Some(format!("hmac-sha256:{}", "a".repeat(64)));
        let error = store
            .create_run_with_input_proof(&forged, commit.proof())
            .unwrap_err()
            .to_string();
        assert!(error.contains("proof_mismatch"), "{error}");
    }

    #[test]
    fn caller_cannot_transplant_persisted_input_or_reasoning_hmac_to_a_new_run() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut source = create_test_run();
        source.reasoning_trace = Some(crate::agent::reasoning::ReasoningTrace {
            input: Some("source-only reasoning".into()),
            ..Default::default()
        });
        store.create_run(&source).unwrap();
        let source = store.get_run(&source.id).unwrap().unwrap();

        let mut transplanted_input = AgentRun::new_chat_run("second-session", "different body");
        transplanted_input.user_input = None;
        transplanted_input.input_digest = source.input_digest.clone();
        let error = store
            .create_run(&transplanted_input)
            .expect_err("a copied digest without observed body is not proof")
            .to_string();
        assert!(error.contains("requires transient input"), "{error}");

        let mut transplanted_trace = AgentRun::new_chat_run("third-session", "third body");
        transplanted_trace.reasoning_trace = None;
        transplanted_trace.reasoning_trace_digest = source.reasoning_trace_digest.clone();
        let error = store
            .create_run(&transplanted_trace)
            .expect_err("a copied reasoning digest without observed trace is not proof")
            .to_string();
        assert!(error.contains("requires transient trace"), "{error}");
    }

    #[test]
    fn canonical_input_proof_is_rejected_across_memory_store_identities() {
        let source_memory = crate::memory::MemoryStore::new_in_memory().unwrap();
        source_memory
            .create_chat_session("test-session", "source proof session")
            .unwrap();
        let message = crate::llm::ChatMessage {
            role: "user".into(),
            content: "Hello world".into(),
        };
        let commit = source_memory
            .save_message_idempotent_with_proof(
                "test-session",
                &message,
                "cross-store-proof-operation",
            )
            .unwrap();
        let bound_memory = crate::memory::MemoryStore::new_in_memory().unwrap();
        let store = AgentRunStore::new_in_memory().unwrap();
        store.bind_canonical_memory_store(&bound_memory).unwrap();
        let mut run = create_test_run();
        run.input_ref = Some(commit.receipt().canonical_ref.clone());

        let error = store
            .create_run_with_input_proof(&run, commit.proof())
            .expect_err("proof from another canonical owner must fail closed")
            .to_string();
        assert!(error.contains("proof_mismatch"), "{error}");
        assert!(store.get_run(&run.id).unwrap().is_none());
        assert!(store.bind_canonical_memory_store(&source_memory).is_err());
    }

    #[test]
    fn persisted_input_and_reasoning_evidence_cannot_drift_on_update() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        run.reasoning_trace = Some(crate::agent::reasoning::ReasoningTrace {
            input: Some("transient reasoning".into()),
            ..Default::default()
        });
        store.create_run(&run).unwrap();
        let mut stored = store.get_run(&run.id).unwrap().unwrap();
        stored.input_digest = Some(format!("hmac-sha256:{}", "a".repeat(64)));
        assert!(store
            .update_run(&stored)
            .unwrap_err()
            .to_string()
            .contains("immutable_evidence_update_conflict"));

        let mut stored = store.get_run(&run.id).unwrap().unwrap();
        stored.reasoning_trace_digest = Some(format!("hmac-sha256:{}", "b".repeat(64)));
        assert!(store
            .update_run(&stored)
            .unwrap_err()
            .to_string()
            .contains("immutable_evidence_update_conflict"));
    }

    #[test]
    fn reasoning_digest_can_be_bound_once_from_transient_trace_then_is_immutable() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run = create_test_run();
        store.create_run(&run).unwrap();
        let mut stored = store.get_run(&run.id).unwrap().unwrap();
        assert!(stored.reasoning_trace_digest.is_none());
        stored.reasoning_trace = Some(crate::agent::reasoning::ReasoningTrace {
            input: Some("first execution trace".into()),
            ..Default::default()
        });
        store.update_run(&stored).unwrap();
        let rebound = store.get_run(&run.id).unwrap().unwrap();
        assert!(rebound.reasoning_trace.is_none());
        assert!(rebound.reasoning_trace_digest.is_some());
    }

    #[tokio::test]
    async fn bound_content_receipt_is_shared_by_its_action_and_observation_without_body_copy() {
        let receipt_key = AgentRunReceiptKey::test_key();
        let store = AgentRunStore::new_in_memory_with_receipt_key(receipt_key.clone()).unwrap();
        let mut run = AgentRun::new_chat_run("receipt-session", "receipt input");
        run.task_id = uuid::Uuid::new_v4().to_string();
        store.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "receipt_fixture",
            "builtin.receipt_fixture",
            "payload",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        store.update_run(&run).unwrap();
        let round_tripped = store.get_run(&run.id).unwrap().unwrap();
        let durable = round_tripped.actions[0]
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .expect("durable action receipt");
        assert!(round_tripped.observations[0].react_trace.is_none());
        assert!(durable.digest().starts_with("hmac-sha256:"));
        assert!(durable.observed_body_receipt().starts_with("hmac-sha256:"));
        assert_eq!(durable.run_id(), round_tripped.id);
        assert_eq!(durable.action_id(), round_tripped.actions[0].id);
        assert_eq!(durable.observation_id(), round_tripped.observations[0].id);
        let durable_json = serde_json::to_string(&round_tripped).unwrap();
        assert!(!durable_json.contains("payload"));
        assert!(!durable_json.contains("bound-content-receipt://"));
        assert!(round_tripped.observations[0]
            .content
            .starts_with("Tool body is not copied"));

        let canonical_store_identity = store.canonical_store_identity().unwrap();
        let forged = serde_json::from_value(serde_json::json!({
            "issuanceId": uuid::Uuid::new_v4().to_string(),
            "receiptId": uuid::Uuid::new_v4().to_string(),
            "canonicalStoreIdentity": canonical_store_identity,
            "runId": round_tripped.id,
            "actionId": round_tripped.actions[0].id,
            "observationId": round_tripped.observations[0].id,
            "field": "action_output_observation_content",
            "kind": "tool_output",
            "provenance": "observed_tool_adapter_body",
            "byteCount": 7,
            "version": 2,
            "bindingReceipt": format!("hmac-sha256:{}", "a".repeat(64)),
            "bodyReceipt": format!("hmac-sha256:{}", "b".repeat(64)),
            "authorityTag": format!("hmac-sha256:{}", "c".repeat(64)),
        }));
        let forged: crate::agent::types::ContentReceipt = forged.unwrap();
        let forged_binding =
            crate::agent::types::ContentReceiptBinding::from_canonical_action_graph(
                &store.canonical_store_identity().unwrap(),
                &round_tripped.id,
                &round_tripped.actions[0],
                &round_tripped.observations[0],
                forged.field(),
            )
            .unwrap();
        assert!(!forged.verify_durable(&receipt_key, &forged_binding));
    }

    #[tokio::test]
    async fn attached_v2_receipt_rejects_bound_trace_drift_without_mutating_owner_row() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = AgentRun::new_chat_run("receipt-trace-drift-session", "");
        store.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "receipt_trace_drift_fixture",
            "builtin.receipt_trace_drift_fixture",
            "private adapter body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        store.update_run(&run).unwrap();

        // A run-level sibling update is outside the v2 content binding and
        // must preserve the exact attached receipt without needing raw body.
        let mut canonical = store.get_run(&run.id).unwrap().unwrap();
        canonical.status = AgentRunStatus::Completed;
        canonical.finished_at = Some(chrono::Utc::now());
        store.update_run(&canonical).unwrap();
        let before_drift = store.get_run(&run.id).unwrap().unwrap();
        let before_json = serde_json::to_value(&before_drift).unwrap();
        let before_receipt_id = before_drift.actions[0]
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .map(|receipt| receipt.receipt_id().to_string())
            .unwrap();
        let before_ledger = {
            let conn = store.conn.lock().unwrap();
            conn.query_row(
                "SELECT state FROM bound_content_issuance_ledger WHERE receipt_id = ?1",
                [&before_receipt_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
        };
        assert_eq!(before_ledger, "attached");

        // trace.status is part of the v2 HMAC semantic material. It cannot be
        // changed without a versioned receipt migration and fresh authority.
        let mut drifted = before_drift;
        drifted.actions[0].react_trace.as_mut().unwrap().status = "failed".into();
        let error = store.update_run(&drifted).unwrap_err().to_string();
        assert!(
            error.contains("bound_content_receipt_attached_semantic_drift"),
            "bound trace drift must fail at the attached receipt boundary: {error}"
        );

        let after = store.get_run(&run.id).unwrap().unwrap();
        assert_eq!(serde_json::to_value(after).unwrap(), before_json);
        let after_ledger = {
            let conn = store.conn.lock().unwrap();
            conn.query_row(
                "SELECT state FROM bound_content_issuance_ledger WHERE receipt_id = ?1",
                [&before_receipt_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
        };
        assert_eq!(after_ledger, "attached");
    }

    #[tokio::test]
    async fn bound_content_receipt_captures_real_adapter_error_and_reloads_from_store() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = AgentRun::new_chat_run("adapter-error-receipt", "");
        store.create_run(&run).unwrap();
        let error_body = "D010_ACTUAL_ADAPTER_ERROR_BODY";
        let (action, observation) = observed_builtin_tool_error_graph(
            &store,
            &run.id,
            1,
            "adapter_error_fixture",
            "builtin.adapter_error_fixture",
            error_body,
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        store.update_run(&run).unwrap();

        let stored = store.get_run(&run.id).unwrap().unwrap();
        let receipt = stored.actions[0]
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .expect("verified error receipt reload");
        assert_eq!(
            receipt.kind(),
            crate::agent::types::ContentReceiptKind::ToolError
        );
        assert_eq!(receipt.byte_count(), error_body.len());
        let encoded = serde_json::to_string(&stored).unwrap();
        assert!(!encoded.contains(error_body));
    }

    #[tokio::test]
    async fn observed_adapter_body_without_receipt_issuer_fails_closed() {
        let mut registry = crate::mcp::McpRegistry::new();
        let mut manifest = crate::tool_manifest::ToolManifest::new(
            "missing_issuer_fixture",
            "Missing issuer fixture",
            serde_json::json!({"type": "object"}),
            "low",
            "1",
            crate::tool_manifest::ToolSource::BuiltIn,
        );
        manifest.id = "builtin.missing_issuer_fixture".into();
        manifest.capabilities = vec!["read".into()];
        manifest.action_type = "read".into();
        manifest.idempotency_contract = crate::tool_manifest::ToolIdempotencyContract::Idempotent;
        registry.register_builtin(manifest, Box::new(|_| Ok("issuer-required-body".into())));
        let permission_store =
            crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let audit_dir = tempfile::tempdir().unwrap();
        let audit_store = crate::mcp_audit::McpAuditStore::new(audit_dir.path().join("audit.db"));
        let privacy_engine = crate::privacy::PrivacyEngine::new();
        let context = crate::agent::action_executor::ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        );
        let error = crate::agent::ToolGateway::from_executor_config(
            crate::agent::action_executor::ActionExecutorConfig::default(),
        )
        .execute(
            crate::agent::action_executor::AgentActionRequest {
                action_type: "builtin_tool".into(),
                target: "missing_issuer_fixture".into(),
                input: serde_json::json!({"arguments": {}}),
                source_run_id: Some(uuid::Uuid::new_v4().to_string()),
                step_index: 1,
            },
            &context,
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("bound_content_receipt_issuer_unavailable"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn action_execution_result_debug_redacts_bodies_and_receipt_authority() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run_id = uuid::Uuid::new_v4().to_string();
        let mut owner = AgentRun::new_tool_execution_run("debug_redaction_fixture");
        owner.id = run_id.clone();
        store.create_run(&owner).unwrap();
        let error_body = "D010_DEBUG_MUST_NOT_LEAK_ADAPTER_BODY";
        let (action, observation) = observed_builtin_tool_error_graph(
            &store,
            &run_id,
            1,
            "debug_redaction_fixture",
            "builtin.debug_redaction_fixture",
            error_body,
        )
        .await;
        let result = crate::agent::action_executor::ActionExecutionResult::without_observed_body(
            action,
            observation,
            crate::agent::action_executor::ActionExecutionStatus::Failed,
            Some("debug-redaction".into()),
            None,
            crate::tool_execution_receipt::ToolExecutionReceipt::failed_before_dispatch(
                Some(run_id),
                Some("builtin.debug_redaction_fixture".into()),
                "debug-redaction-request".into(),
                crate::tool_execution_receipt::ToolActionEffect::ReadOnly,
                crate::tool_manifest::ToolIdempotencyContract::Idempotent,
            ),
        );
        let debug = format!("{result:?}");
        assert!(!debug.contains(error_body));
        assert!(!debug.contains("hmac-sha256:"));
        assert!(!debug.contains("authority_tag"));
        assert!(debug.contains("observed_body_admission_present"));
    }

    #[tokio::test]
    async fn bound_content_issue_transaction_requires_a_current_active_owner() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run_id = uuid::Uuid::new_v4().to_string();
        let error = execute_observed_builtin_tool_output(
            &store,
            &run_id,
            1,
            "active_owner_fixture",
            "builtin.active_owner_fixture",
            "owner-bound body",
        )
        .await
        .expect_err("receipt issuance without an active owner must fail")
        .to_string();
        assert!(
            error.contains("bound_content_receipt_active_run_owner_missing"),
            "{error}"
        );
        assert_eq!(bound_content_issuance_count(&store, &run_id, None), 0);

        let mut owner = AgentRun::new_tool_execution_run("active_owner_fixture");
        owner.id = run_id.clone();
        store.create_run(&owner).unwrap();
        assert_eq!(
            bound_content_issuance_count(&store, &run_id, None),
            0,
            "creating the same id later cannot revive the rejected issuance"
        );
        let fresh = execute_observed_builtin_tool_output(
            &store,
            &run_id,
            2,
            "active_owner_fixture",
            "builtin.active_owner_fixture",
            "fresh owner-bound body",
        )
        .await
        .expect("a fresh execution against the active owner may issue");
        assert_eq!(
            fresh.status,
            crate::agent::action_executor::ActionExecutionStatus::Succeeded
        );
        assert_eq!(
            bound_content_issuance_count(&store, &run_id, Some("pending")),
            1
        );
    }

    #[tokio::test]
    async fn bound_content_issue_transaction_rejects_every_non_running_owner() {
        let store = AgentRunStore::new_in_memory().unwrap();
        for status in [
            AgentRunStatus::WaitingPermission,
            AgentRunStatus::Completed,
            AgentRunStatus::Failed,
            AgentRunStatus::RemoteUnknown,
            AgentRunStatus::Cancelled,
        ] {
            let mut owner = AgentRun::new_tool_execution_run("non_running_issuer_fixture");
            owner.id = format!("non-running-issuer-{status}");
            store.create_run(&owner).unwrap();
            owner.status = status;
            if status != AgentRunStatus::WaitingPermission {
                owner.finished_at = Some(chrono::Utc::now());
            }
            store.update_run(&owner).unwrap();

            let error = execute_observed_builtin_tool_output(
                &store,
                &owner.id,
                1,
                "non_running_issuer_fixture",
                "builtin.non_running_issuer_fixture",
                "non-running owner body",
            )
            .await
            .expect_err("receipt issuance requires a running canonical owner")
            .to_string();
            assert!(
                error.contains("bound_content_receipt_active_run_owner_missing"),
                "{status}: {error}"
            );
            assert_eq!(bound_content_issuance_count(&store, &owner.id, None), 0);
        }
    }

    #[tokio::test]
    async fn delete_clears_pending_issuance_without_erasing_attached_history() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let pending_owner = AgentRun::new_tool_execution_run("pending-delete");
        store.create_run(&pending_owner).unwrap();
        let (pending_action, pending_observation) = observed_builtin_tool_output_graph(
            &store,
            &pending_owner.id,
            1,
            "pending_delete_fixture",
            "builtin.pending_delete_fixture",
            "pending body",
        )
        .await;
        assert_eq!(
            bound_content_issuance_count(&store, &pending_owner.id, Some("pending")),
            1
        );

        store
            .delete_run_with_tombstone(&pending_owner.id, Some("delete pending owner"))
            .unwrap();
        assert_eq!(
            bound_content_issuance_count(&store, &pending_owner.id, Some("pending")),
            0
        );
        store.restore_run_with_receipt(&pending_owner.id).unwrap();
        let mut restored = store.get_run(&pending_owner.id).unwrap().unwrap();
        restored.actions.push(pending_action);
        restored.observations.push(pending_observation);
        let error = store.update_run(&restored).unwrap_err().to_string();
        assert!(
            error.contains("pending_issuance_missing_or_expired"),
            "restoring the owner must not revive its old pending receipt: {error}"
        );

        let mut attached_owner = AgentRun::new_tool_execution_run("attached-delete");
        store.create_run(&attached_owner).unwrap();
        let (attached_action, attached_observation) = observed_builtin_tool_output_graph(
            &store,
            &attached_owner.id,
            1,
            "attached_delete_fixture",
            "builtin.attached_delete_fixture",
            "attached body",
        )
        .await;
        attached_owner.actions.push(attached_action);
        attached_owner.observations.push(attached_observation);
        store.update_run(&attached_owner).unwrap();
        assert_eq!(
            bound_content_issuance_count(&store, &attached_owner.id, Some("attached")),
            1
        );
        store
            .delete_run_with_tombstone(&attached_owner.id, Some("retain attached history"))
            .unwrap();
        assert_eq!(
            bound_content_issuance_count(&store, &attached_owner.id, Some("pending")),
            0
        );
        assert_eq!(
            bound_content_issuance_count(&store, &attached_owner.id, Some("attached")),
            1,
            "canonical deletion retains already-attached minimized lineage"
        );
        store.restore_run_with_receipt(&attached_owner.id).unwrap();
        assert!(store
            .get_run(&attached_owner.id)
            .unwrap()
            .is_some_and(|run| run.actions.len() == 1 && run.observations.len() == 1));
    }

    #[test]
    fn bound_content_issue_delete_race_has_no_tombstoned_pending_terminal() {
        for iteration in 0..12 {
            let store = AgentRunStore::new_in_memory().unwrap();
            let owner_name = format!("issue-delete-{iteration}");
            let owner = AgentRun::new_tool_execution_run(&owner_name);
            store.create_run(&owner).unwrap();
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));

            let issue_store = store.clone();
            let issue_run_id = owner.id.clone();
            let issue_barrier = barrier.clone();
            let issue = std::thread::spawn(move || {
                issue_barrier.wait();
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(execute_observed_builtin_tool_output(
                        &issue_store,
                        &issue_run_id,
                        1,
                        "issue_delete_race_fixture",
                        "builtin.issue_delete_race_fixture",
                        "race body",
                    ))
            });

            let delete_store = store.clone();
            let delete_run_id = owner.id.clone();
            let delete_barrier = barrier.clone();
            let delete = std::thread::spawn(move || {
                delete_barrier.wait();
                delete_store
                    .delete_run_with_tombstone(&delete_run_id, Some("race canonical delete"))
            });

            barrier.wait();
            let issue_result = issue.join().unwrap();
            delete.join().unwrap().unwrap();
            if let Err(error) = issue_result {
                assert!(
                    error
                        .to_string()
                        .contains("bound_content_receipt_active_run_owner_missing"),
                    "unexpected issue terminal: {error}"
                );
            }
            assert!(store.get_run(&owner.id).unwrap().is_none());
            assert_eq!(
                bound_content_issuance_count(&store, &owner.id, Some("pending")),
                0,
                "iteration {iteration} left a tombstoned pending issuance"
            );
        }
    }

    #[tokio::test]
    async fn stale_update_cannot_resurrect_deleted_owner_or_authorize_internal_read() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let owner = AgentRun::new_tool_execution_run("stale-update-owner");
        store.create_run(&owner).unwrap();
        let mut stale_snapshot = store.get_run(&owner.id).unwrap().unwrap();
        stale_snapshot.tool_call_count = 1;

        store
            .delete_run_with_tombstone(&owner.id, Some("canonical delete before stale update"))
            .unwrap();
        let update_error = store
            .update_run(&stale_snapshot)
            .expect_err("stale update must not resurrect a deleted owner")
            .to_string();
        assert!(update_error.contains("agent_run_update_owner_inactive"));
        assert!(store.get_run(&owner.id).unwrap().is_none());
        assert!(store
            .get_run_including_deleted(&owner.id)
            .unwrap()
            .is_some_and(|run| run.deleted_at.is_some()));
        {
            let conn = store.conn.lock().unwrap();
            assert!(
                persistence_outbox::has_active_tombstone(&conn, "agent_run", &owner.id).unwrap()
            );
        }

        let memory_store = crate::memory::MemoryStore::new_in_memory().unwrap();
        memory_store
            .save_message(
                "stale-update-session",
                &crate::llm::ChatMessage {
                    role: "user".into(),
                    content: "stale update private session body".into(),
                },
            )
            .unwrap();
        let registry = crate::mcp::McpRegistry::new();
        let permission_store =
            crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
        let privacy_engine = crate::privacy::PrivacyEngine::new();
        let observer = CountingStoreDispatchObserver::default();
        let context = crate::agent::ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_memory_store(&memory_store)
        .with_agent_run_store(&store)
        .with_tool_dispatch_observer(&observer);
        let result = crate::agent::ToolGateway::from_executor_config(Default::default())
            .execute(
                crate::agent::AgentActionRequest {
                    action_type: "session_search".into(),
                    target: "session.search".into(),
                    input: serde_json::json!({
                        "query": "stale update",
                        "session_id": "stale-update-session",
                        "limit": 5,
                    }),
                    source_run_id: Some(owner.id.clone()),
                    step_index: 0,
                },
                &context,
            )
            .await
            .unwrap();
        assert_eq!(result.status, crate::agent::ActionExecutionStatus::Blocked);
        assert_eq!(
            result.execution_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
        );
        assert_eq!(
            result.execution_receipt.effect_status,
            crate::tool_execution_receipt::ToolEffectStatus::NotAttempted
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
        assert_eq!(bound_content_issuance_count(&store, &owner.id, None), 0);

        // Simulate an anomalous pre-fix database where a stale writer already
        // cleared the row marker but could not supersede the canonical
        // tombstone. The owner authority must still fail closed.
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE agent_runs
                 SET deleted_at = NULL, delete_reason = NULL
                 WHERE id = ?1",
                [&owner.id],
            )
            .unwrap();
            assert!(
                persistence_outbox::has_active_tombstone(&conn, "agent_run", &owner.id).unwrap()
            );
        }
        assert!(!store.has_active_bound_content_owner(&owner.id).unwrap());
        let anomalous_legacy_result =
            crate::agent::ToolGateway::from_executor_config(Default::default())
                .execute(
                    crate::agent::AgentActionRequest {
                        action_type: "session_search".into(),
                        target: "session.search".into(),
                        input: serde_json::json!({
                            "query": "stale update",
                            "session_id": "stale-update-session",
                            "limit": 5,
                        }),
                        source_run_id: Some(owner.id.clone()),
                        step_index: 1,
                    },
                    &context,
                )
                .await
                .unwrap();
        assert_eq!(
            anomalous_legacy_result.status,
            crate::agent::ActionExecutionStatus::Blocked
        );
        assert_eq!(
            anomalous_legacy_result.execution_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
        );
        assert_eq!(observer.count.load(Ordering::SeqCst), 0);
        assert_eq!(bound_content_issuance_count(&store, &owner.id, None), 0);
    }

    #[test]
    fn ordinary_update_cannot_mutate_delete_restore_owned_fields() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let owner = AgentRun::new_tool_execution_run("delete-field-owner");
        store.create_run(&owner).unwrap();
        let mut forged_delete = store.get_run(&owner.id).unwrap().unwrap();
        forged_delete.deleted_at = Some(chrono::Utc::now());
        forged_delete.delete_reason = Some("ordinary update attempted delete".into());
        let error = store.update_run(&forged_delete).unwrap_err().to_string();
        assert!(error.contains("agent_run_delete_restore_fields_owned_by_canonical_transaction"));
        assert!(store.get_run(&owner.id).unwrap().is_some());
        let conn = store.conn.lock().unwrap();
        assert!(!persistence_outbox::has_active_tombstone(&conn, "agent_run", &owner.id).unwrap());
    }

    #[test]
    fn update_delete_race_never_finishes_with_live_row_and_active_tombstone() {
        for iteration in 0..12 {
            let store = AgentRunStore::new_in_memory().unwrap();
            let owner_name = format!("update-delete-{iteration}");
            let owner = AgentRun::new_tool_execution_run(&owner_name);
            store.create_run(&owner).unwrap();
            let mut stale_snapshot = store.get_run(&owner.id).unwrap().unwrap();
            stale_snapshot.tool_call_count = iteration + 1;
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));

            let update_store = store.clone();
            let update_barrier = barrier.clone();
            let update = std::thread::spawn(move || {
                update_barrier.wait();
                update_store.update_run(&stale_snapshot)
            });

            let delete_store = store.clone();
            let delete_run_id = owner.id.clone();
            let delete_barrier = barrier.clone();
            let delete = std::thread::spawn(move || {
                delete_barrier.wait();
                delete_store.delete_run_with_tombstone(
                    &delete_run_id,
                    Some("update-delete race canonical delete"),
                )
            });

            barrier.wait();
            let update_result = update.join().unwrap();
            delete.join().unwrap().unwrap();
            if let Err(error) = update_result {
                assert!(
                    error
                        .to_string()
                        .contains("agent_run_update_owner_inactive"),
                    "unexpected update terminal: {error}"
                );
            }
            let conn = store.conn.lock().unwrap();
            let inconsistent: i64 = conn
                .query_row(
                    "SELECT COUNT(*)
                     FROM agent_runs run
                     JOIN canonical_tombstones tombstone
                       ON tombstone.aggregate_kind = 'agent_run'
                      AND tombstone.aggregate_id = run.id
                      AND tombstone.superseded_at IS NULL
                     WHERE run.id = ?1 AND run.deleted_at IS NULL",
                    [&owner.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                inconsistent, 0,
                "iteration {iteration} resurrected a live row behind an active tombstone"
            );
        }
    }

    #[test]
    fn active_tombstone_hides_every_product_read_until_explicit_restore() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut owner = AgentRun::new_tool_execution_run("product-live-fence");
        owner.task_id = "product-live-fence-task".into();
        owner.session_id = Some("product-live-fence-session".into());
        owner.status = AgentRunStatus::WaitingPermission;
        owner.generated_proposals = vec!["product-live-fence-proposal".into()];
        store.create_run(&owner).unwrap();
        store
            .delete_run_with_tombstone(&owner.id, Some("product live fence"))
            .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE agent_runs
                 SET deleted_at = NULL, delete_reason = NULL
                 WHERE id = ?1",
                [&owner.id],
            )
            .unwrap();
            assert!(
                persistence_outbox::has_active_tombstone(&conn, "agent_run", &owner.id).unwrap()
            );
        }

        assert!(store
            .get_run_including_deleted(&owner.id)
            .unwrap()
            .is_some());
        assert!(store.get_run(&owner.id).unwrap().is_none());
        assert!(store.get_run_for_task_id(&owner.task_id).unwrap().is_none());
        assert!(store.list_runs(20, 0).unwrap().is_empty());
        assert!(store
            .list_runs_for_session("product-live-fence-session", 20)
            .unwrap()
            .is_empty());
        assert!(store
            .last_run_for_session("product-live-fence-session")
            .unwrap()
            .is_none());
        assert!(store
            .list_runs_linked_to_proposal("product-live-fence-proposal")
            .unwrap()
            .is_empty());
        assert!(store
            .list_waiting_permission_linked_proposal_ids(20)
            .unwrap()
            .is_empty());
        assert_eq!(store.run_count().unwrap(), 0);

        store.restore_run_with_receipt(&owner.id).unwrap();
        assert!(store.get_run(&owner.id).unwrap().is_some());
        assert_eq!(
            store
                .get_run_for_task_id(&owner.task_id)
                .unwrap()
                .unwrap()
                .id,
            owner.id
        );
        assert_eq!(store.list_runs(20, 0).unwrap().len(), 1);
        assert_eq!(
            store
                .list_runs_for_session("product-live-fence-session", 20)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .list_runs_linked_to_proposal("product-live-fence-proposal")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .list_waiting_permission_linked_proposal_ids(20)
                .unwrap(),
            vec!["product-live-fence-proposal"]
        );
        assert_eq!(store.run_count().unwrap(), 1);
        let conn = store.conn.lock().unwrap();
        assert!(!persistence_outbox::has_active_tombstone(&conn, "agent_run", &owner.id).unwrap());
    }

    #[test]
    fn startup_reconciles_live_row_marker_behind_active_tombstone() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tombstone-marker-reconciliation.db");
        let run_id = {
            let store = AgentRunStore::new(&path).unwrap();
            let owner = AgentRun::new_tool_execution_run("startup-tombstone-reconciliation");
            store.create_run(&owner).unwrap();
            store
                .delete_run_with_tombstone(&owner.id, Some("startup marker repair"))
                .unwrap();
            {
                let conn = store.conn.lock().unwrap();
                conn.execute(
                    "UPDATE agent_runs
                     SET deleted_at = NULL, delete_reason = NULL
                     WHERE id = ?1",
                    [&owner.id],
                )
                .unwrap();
            }
            owner.id
        };

        let reopened = AgentRunStore::new(&path).unwrap();
        assert!(reopened.get_run(&run_id).unwrap().is_none());
        assert!(reopened
            .get_run_including_deleted(&run_id)
            .unwrap()
            .is_some_and(|run| run.deleted_at.is_some()));
        let conn = reopened.conn.lock().unwrap();
        assert!(persistence_outbox::has_active_tombstone(&conn, "agent_run", &run_id).unwrap());
    }

    #[tokio::test]
    async fn bound_content_receipt_pending_attach_is_one_shot_and_transaction_rollback_safe() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = AgentRun::new_chat_run("one-shot-attach", "");
        store.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "one_shot_fixture",
            "builtin.one_shot_fixture",
            "one-shot adapter body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        let replay = run.clone();

        store.install_update_failure_for_test().unwrap();
        assert!(store.update_run(&run).is_err());
        {
            let conn = store.conn.lock().unwrap();
            let state: String = conn
                .query_row(
                    "SELECT state FROM bound_content_issuance_ledger WHERE run_id = ?1",
                    [&run.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(state, "pending", "rollback must not burn the admission");
            conn.execute_batch("DROP TRIGGER fail_agent_run_update_for_test")
                .unwrap();
        }

        store.update_run(&run).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            let state: String = conn
                .query_row(
                    "SELECT state FROM bound_content_issuance_ledger WHERE run_id = ?1",
                    [&run.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(state, "attached");
        }
        let stored = store.get_run(&run.id).unwrap().unwrap();
        store
            .update_run(&stored)
            .expect("an exact canonical reload is not a second issuance");
        store
            .update_run(&replay)
            .expect("the owner may idempotently replay the same verified raw graph");
    }

    #[tokio::test]
    async fn bound_content_receipt_expired_pending_issuance_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("expired-pending.db");
        let key = AgentRunReceiptKey::from_bytes(
            [0x21; crate::agent::types::AGENT_RUN_RECEIPT_KEY_BYTES],
        )
        .unwrap();
        let store = AgentRunStore::new_with_receipt_key(&path, key.clone()).unwrap();
        let mut run = AgentRun::new_chat_run("expired-attach", "");
        store.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "expired_fixture",
            "builtin.expired_fixture",
            "expired adapter body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE bound_content_issuance_ledger SET expires_at = ?2 WHERE run_id = ?1",
                params![run.id, chrono::Utc::now().timestamp() - 1],
            )
            .unwrap();
        }
        let error = store.update_run(&run).unwrap_err().to_string();
        assert!(
            error.contains("pending_issuance_missing_or_expired"),
            "{error}"
        );
        assert!(store
            .get_run(&run.id)
            .unwrap()
            .is_some_and(|stored| stored.actions.is_empty() && stored.observations.is_empty()));
        drop(store);
        let reopened = AgentRunStore::new_with_receipt_key(&path, key).unwrap();
        let remaining: i64 = reopened
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM bound_content_issuance_ledger WHERE run_id = ?1",
                [&run.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            remaining, 0,
            "startup pruning must delete expired pending rows"
        );
    }

    #[tokio::test]
    async fn bound_content_receipt_pending_survives_reopen_and_rejects_wrong_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent-runs.db");
        let key = AgentRunReceiptKey::from_bytes(
            [0x31; crate::agent::types::AGENT_RUN_RECEIPT_KEY_BYTES],
        )
        .unwrap();
        let store = AgentRunStore::new_with_receipt_key(&path, key.clone()).unwrap();
        let mut run = AgentRun::new_chat_run("reopen-pending", "");
        store.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "reopen_fixture",
            "builtin.reopen_fixture",
            "reopen adapter body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        drop(store);

        let wrong_key = AgentRunReceiptKey::from_bytes(
            [0x32; crate::agent::types::AGENT_RUN_RECEIPT_KEY_BYTES],
        )
        .unwrap();
        assert!(AgentRunStore::new_with_receipt_key(&path, wrong_key).is_err());
        let reopened = AgentRunStore::new_with_receipt_key(&path, key).unwrap();
        reopened.update_run(&run).unwrap();
        assert!(reopened.get_run(&run.id).unwrap().is_some());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bound_content_receipt_same_canonical_path_and_symlink_share_one_slot() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canonical-agent-runs.db");
        let symlink_path = dir.path().join("canonical-agent-runs-link.db");
        let key = AgentRunReceiptKey::from_bytes(
            [0x39; crate::agent::types::AGENT_RUN_RECEIPT_KEY_BYTES],
        )
        .unwrap();
        let store = AgentRunStore::new_with_receipt_key(&path, key.clone()).unwrap();
        let mut run = AgentRun::new_chat_run("symlink-reopen", "");
        store.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "symlink_fixture",
            "builtin.symlink_fixture",
            "same canonical slot body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        store.update_run(&run).unwrap();
        drop(store);

        symlink(&path, &symlink_path).unwrap();
        let reopened = AgentRunStore::new_with_receipt_key(&symlink_path, key).unwrap();
        let stored = reopened.get_run(&run.id).unwrap().unwrap();
        assert!(stored.actions[0]
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .is_some());
        assert!(!stored.legacy_payload_unverified);
    }

    #[cfg(unix)]
    #[test]
    fn agent_run_store_rejects_symlink_slot_swap_during_open() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("slot-swap-source.db");
        let target_path = dir.path().join("slot-swap-target.db");
        let link_path = dir.path().join("slot-swap-link.db");
        for path in [&source_path, &target_path] {
            Connection::open(path)
                .unwrap()
                .execute_batch("CREATE TABLE agent_runs(id TEXT PRIMARY KEY);")
                .unwrap();
        }

        symlink(&source_path, &link_path).unwrap();
        let writable_error = open_agent_run_database_with_stable_slot(
            &link_path,
            || {
                std::fs::remove_file(&link_path).unwrap();
                symlink(&target_path, &link_path).unwrap();
            },
            || {
                std::fs::remove_file(&link_path).unwrap();
                symlink(&source_path, &link_path).unwrap();
            },
        )
        .err()
        .expect("writable open must reject a changed symlink slot")
        .to_string();
        assert!(
            writable_error.contains("agent_run_database_slot_changed_during_open"),
            "{writable_error}"
        );

        std::fs::remove_file(&link_path).unwrap();
        symlink(&source_path, &link_path).unwrap();
        let read_only_error = open_agent_run_database_read_only_with_stable_slot(
            &link_path,
            || {
                std::fs::remove_file(&link_path).unwrap();
                symlink(&target_path, &link_path).unwrap();
            },
            || {
                std::fs::remove_file(&link_path).unwrap();
                symlink(&source_path, &link_path).unwrap();
            },
        )
        .err()
        .expect("read-only open must reject a changed symlink slot")
        .to_string();
        assert!(
            read_only_error.contains("agent_run_database_slot_changed_during_read_only_open"),
            "{read_only_error}"
        );
    }

    #[test]
    fn same_path_database_replacement_cannot_create_two_agent_run_owners() {
        let dir = tempfile::tempdir().unwrap();
        let slot = dir.path().join("agent-runs.db");
        let displaced = dir.path().join("agent-runs-old-inode.db");
        let replacement = dir.path().join("agent-runs-copy.db");
        let key = AgentRunReceiptKey::from_bytes(
            [0x3a; crate::agent::types::AGENT_RUN_RECEIPT_KEY_BYTES],
        )
        .unwrap();
        let first = AgentRunStore::new_with_receipt_key(&slot, key.clone()).unwrap();
        let original_identity = {
            let conn = first.conn.lock().unwrap();
            AgentRunStore::existing_canonical_store_identity(&conn).unwrap()
        };
        {
            let conn = first.conn.lock().unwrap();
            AgentRunStore::checkpoint_agent_run_wal(&conn).unwrap();
        }
        std::fs::copy(&slot, &replacement).unwrap();
        std::fs::rename(&slot, &displaced).unwrap();
        std::fs::rename(&replacement, &slot).unwrap();

        let second_error = AgentRunStore::new_with_receipt_key(&slot, key.clone())
            .err()
            .expect("replacement pathname must not create a second live AgentRun owner")
            .to_string();
        assert!(
            second_error.contains("agent_run_store_sqlite_slot_owner_lease_unavailable"),
            "{second_error}"
        );
        assert!(first
            .conn
            .lock()
            .unwrap_err()
            .to_string()
            .contains("agent_run_store_database_identity_changed"));

        drop(first);
        for suffix in ["-wal", "-shm"] {
            let sidecar = PathBuf::from(format!("{}{}", slot.display(), suffix));
            let _ = std::fs::remove_file(sidecar);
        }
        let replacement_owner = AgentRunStore::new_with_receipt_key(&slot, key)
            .expect("final owner drop releases the AgentRun canonical slot lease");
        let reopened_identity = {
            let conn = replacement_owner.conn.lock().unwrap();
            AgentRunStore::existing_canonical_store_identity(&conn).unwrap()
        };
        assert_eq!(reopened_identity, original_identity);
    }

    #[tokio::test]
    async fn copied_agent_run_database_rejects_same_installation_key_at_another_slot() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("source-slot.db");
        let target_path = dir.path().join("target-slot.db");
        let key = AgentRunReceiptKey::from_bytes(
            [0x3a; crate::agent::types::AGENT_RUN_RECEIPT_KEY_BYTES],
        )
        .unwrap();
        let source = AgentRunStore::new_with_receipt_key(&source_path, key.clone()).unwrap();
        let mut run = AgentRun::new_chat_run("copied-slot", "");
        source.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &source,
            &run.id,
            1,
            "copied_slot_fixture",
            "builtin.copied_slot_fixture",
            "copied slot body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        source.update_run(&run).unwrap();
        drop(source);

        Connection::open(&source_path)
            .unwrap()
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        std::fs::copy(&source_path, &target_path).unwrap();
        let copied_error = AgentRunStore::new_with_receipt_key(&target_path, key.clone())
            .err()
            .expect("copied store must not retain receipt authority")
            .to_string();
        assert!(copied_error.contains("agent_run_receipt_key_mismatch"));
        assert!(AgentRunStore::new_with_receipt_key(&source_path, key)
            .unwrap()
            .get_run(&run.id)
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn legacy_unscoped_pending_receipt_is_quarantined_in_every_database_copy() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("legacy-source.db");
        let target_path = dir.path().join("legacy-copy.db");
        let key = AgentRunReceiptKey::from_bytes(
            [0x3b; crate::agent::types::AGENT_RUN_RECEIPT_KEY_BYTES],
        )
        .unwrap();
        let source = AgentRunStore::new_with_receipt_key(&source_path, key.clone()).unwrap();
        let mut run = AgentRun::new_chat_run("legacy-pending-copy", "");
        source.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &source,
            &run.id,
            1,
            "legacy_pending_fixture",
            "builtin.legacy_pending_fixture",
            "legacy pending body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        {
            let conn = source.conn.lock().unwrap();
            let legacy_verifier = key.sign(
                "store_key_verifier",
                "openlife-agent-run-store-key-binding-v1",
            );
            conn.execute(
                "UPDATE agent_run_store_metadata SET value = ?1
                 WHERE key = 'receipt_key_verifier'",
                [legacy_verifier],
            )
            .unwrap();
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .unwrap();
        }
        drop(source);
        std::fs::copy(&source_path, &target_path).unwrap();

        for path in [&source_path, &target_path] {
            let rebound = AgentRunStore::new_with_receipt_key(path, key.clone()).unwrap();
            let ledger_rows: i64 = rebound
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) FROM bound_content_issuance_ledger",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(ledger_rows, 0);
            let error = rebound.update_run(&run).unwrap_err().to_string();
            assert!(
                error.contains("bound_content_receipt"),
                "legacy pending graph must fail at receipt authority: {error}"
            );
            assert!(rebound
                .get_run(&run.id)
                .unwrap()
                .is_some_and(|stored| stored.actions.is_empty() && stored.observations.is_empty()));
        }
    }

    #[tokio::test]
    async fn legacy_unscoped_attached_receipt_is_explicitly_unverified_after_rebind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy-attached.db");
        let key = AgentRunReceiptKey::from_bytes(
            [0x3c; crate::agent::types::AGENT_RUN_RECEIPT_KEY_BYTES],
        )
        .unwrap();
        let store = AgentRunStore::new_with_receipt_key(&path, key.clone()).unwrap();
        let mut run = AgentRun::new_chat_run("legacy-attached", "");
        store.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "legacy_attached_fixture",
            "builtin.legacy_attached_fixture",
            "legacy attached body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        store.update_run(&run).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            let legacy_verifier = key.sign(
                "store_key_verifier",
                "openlife-agent-run-store-key-binding-v1",
            );
            conn.execute(
                "UPDATE agent_run_store_metadata SET value = ?1
                 WHERE key = 'receipt_key_verifier'",
                [legacy_verifier],
            )
            .unwrap();
        }
        drop(store);

        let rebound = AgentRunStore::new_with_receipt_key(&path, key).unwrap();
        let stored = rebound.get_run(&run.id).unwrap().unwrap();
        assert!(stored.legacy_payload_unverified);
        assert!(stored.actions[0]
            .react_trace
            .as_ref()
            .and_then(|trace| trace.output_receipt.as_ref())
            .is_none());
        let ledger_rows: i64 = rebound
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM bound_content_issuance_ledger",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(ledger_rows, 0);
    }

    #[tokio::test]
    async fn bound_content_receipt_rejects_cross_store_transplant_after_ledger_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("source.db");
        let target_path = dir.path().join("target.db");
        let key = AgentRunReceiptKey::from_bytes(
            [0x41; crate::agent::types::AGENT_RUN_RECEIPT_KEY_BYTES],
        )
        .unwrap();
        let source = AgentRunStore::new_with_receipt_key(&source_path, key.clone()).unwrap();
        let target = AgentRunStore::new_with_receipt_key(&target_path, key.clone()).unwrap();
        assert_ne!(
            source.canonical_store_identity().unwrap(),
            target.canonical_store_identity().unwrap()
        );
        let mut run = AgentRun::new_chat_run("cross-store-source", "");
        source.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &source,
            &run.id,
            1,
            "cross_store_fixture",
            "builtin.cross_store_fixture",
            "cross-store adapter body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        source.update_run(&run).unwrap();

        let mut target_run = AgentRun::new_chat_run("cross-store-target", "");
        target_run.id = run.id.clone();
        target.create_run(&target_run).unwrap();
        {
            let conn = source.conn.lock().unwrap();
            conn.execute("DELETE FROM bound_content_issuance_ledger", [])
                .unwrap();
        }
        drop(source);
        drop(target);

        let source = AgentRunStore::new_with_receipt_key(&source_path, key.clone()).unwrap();
        let target = AgentRunStore::new_with_receipt_key(&target_path, key).unwrap();
        let (actions_json, observations_json): (String, String) = {
            let conn = source.conn.lock().unwrap();
            conn.query_row(
                "SELECT actions_json, observations_json FROM agent_runs WHERE id = ?1",
                [&run.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
        };
        {
            let conn = target.conn.lock().unwrap();
            conn.execute(
                "UPDATE agent_runs SET actions_json = ?2, observations_json = ?3 WHERE id = ?1",
                params![run.id, actions_json, observations_json],
            )
            .unwrap();
        }
        let error = target.get_run(&run.id).unwrap_err().to_string();
        assert!(error.contains("invalid_owner_graph"), "{error}");
    }

    #[tokio::test]
    async fn bound_content_receipt_ledger_and_agent_run_db_never_store_observed_body() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("content-minimized.db");
        let store = AgentRunStore::new_with_receipt_key(
            &path,
            AgentRunReceiptKey::from_bytes(
                [0x51; crate::agent::types::AGENT_RUN_RECEIPT_KEY_BYTES],
            )
            .unwrap(),
        )
        .unwrap();
        let body = "D010_RAW_ADAPTER_BODY_SENTINEL_7f1c2a9e".repeat(4);
        let mut run = AgentRun::new_chat_run("body-absence", "");
        store.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "body_absence_fixture",
            "builtin.body_absence_fixture",
            &body,
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        {
            let conn = store.conn.lock().unwrap();
            let ledger_json: String = conn
                .query_row(
                    "SELECT receipt_json FROM bound_content_issuance_ledger WHERE run_id = ?1",
                    [&run.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(!ledger_json.contains(&body));
        }
        store.update_run(&run).unwrap();
        drop(store);
        for candidate in [
            path.clone(),
            std::path::PathBuf::from(format!("{}-wal", path.display())),
            std::path::PathBuf::from(format!("{}-shm", path.display())),
        ] {
            if candidate.exists() {
                let bytes = std::fs::read(&candidate).unwrap();
                assert!(
                    !bytes
                        .windows(body.len())
                        .any(|window| window == body.as_bytes()),
                    "raw adapter body leaked into {}",
                    candidate.display()
                );
            }
        }
    }

    #[tokio::test]
    async fn bound_content_receipt_rejects_body_mutation_cross_run_transplant_and_broken_graphs() {
        let receipt_key = AgentRunReceiptKey::test_key();
        let run_id = uuid::Uuid::new_v4().to_string();
        let store = AgentRunStore::new_in_memory_with_receipt_key(receipt_key).unwrap();
        let mut valid = AgentRun::new_chat_run("valid", "input");
        valid.id = run_id.clone();
        store.create_run(&valid).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run_id,
            1,
            "bound_fixture",
            "builtin.bound_fixture",
            "sealed-body",
        )
        .await;

        let mut mutated = valid.clone();
        mutated.actions.push(action.clone());
        mutated.actions[0].output = Some(serde_json::json!({"text": "MUTATED-body"}));
        mutated.observations.push(observation.clone());
        assert!(store
            .update_run(&mutated)
            .unwrap_err()
            .to_string()
            .contains("observed_body_mismatch"));

        let mut synchronized_body_forgery = valid.clone();
        synchronized_body_forgery.actions.push(action.clone());
        synchronized_body_forgery.actions[0].output =
            Some(serde_json::json!({"text": "forged-body"}));
        synchronized_body_forgery
            .observations
            .push(observation.clone());
        synchronized_body_forgery.observations[0].content = "forged-body".into();
        assert!(store
            .update_run(&synchronized_body_forgery)
            .unwrap_err()
            .to_string()
            .contains("canonical_identity_mismatch"));

        valid.actions.push(action.clone());
        valid.observations.push(observation.clone());
        store.update_run(&valid).unwrap();
        let stored = store.get_run(&valid.id).unwrap().unwrap();

        let mut transplant = AgentRun::new_chat_run("transplant", "input");
        transplant.id = uuid::Uuid::new_v4().to_string();
        transplant.actions = stored.actions.clone();
        transplant.observations = stored.observations.clone();
        transplant.actions[0]
            .react_trace
            .as_mut()
            .expect("stored action keeps its receipt trace")
            .run_id = Some(transplant.id.clone());
        assert!(store
            .create_run(&transplant)
            .unwrap_err()
            .to_string()
            .contains("bound_content_receipt"));

        let mut duplicate = AgentRun::new_chat_run("duplicate", "input");
        duplicate.id = valid.id.clone();
        duplicate.actions = vec![action.clone(), action];
        duplicate.observations = vec![observation.clone()];
        assert!(store
            .create_run(&duplicate)
            .unwrap_err()
            .to_string()
            .contains("action_identity_not_unique"));

        let mut orphan = AgentRun::new_chat_run("orphan", "input");
        let mut orphan_observation = observation;
        orphan_observation.action_id = Some("action-missing".into());
        orphan.observations.push(orphan_observation);
        assert!(store
            .create_run(&orphan)
            .unwrap_err()
            .to_string()
            .contains("observation_action_foreign_key_missing"));
    }

    #[tokio::test]
    async fn bound_content_receipt_rejects_direct_sql_semantic_transplant() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = AgentRun::new_chat_run("sql-semantic-transplant", "input");
        store.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "memory_search",
            "builtin.memory.search",
            "adapter body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        store.update_run(&run).unwrap();

        {
            let conn = store.conn.lock().unwrap();
            let raw: String = conn
                .query_row(
                    "SELECT actions_json FROM agent_runs WHERE id = ?1",
                    [&run.id],
                    |row| row.get(0),
                )
                .unwrap();
            let mut actions: Vec<crate::agent::types::AgentAction> =
                serde_json::from_str(&raw).unwrap();
            let trace = actions[0].react_trace.as_mut().unwrap();
            trace.tool_name = "calendar.read".into();
            actions[0].tool_scope.as_mut().unwrap().tool_name = "calendar.read".into();
            conn.execute(
                "UPDATE agent_runs SET actions_json = ?2 WHERE id = ?1",
                rusqlite::params![run.id, serde_json::to_string(&actions).unwrap()],
            )
            .unwrap();
        }

        let error = store.get_run(&run.id).unwrap_err().to_string();
        assert!(error.contains("invalid_owner_graph"), "{error}");
    }

    #[tokio::test]
    async fn bound_content_receipt_rejects_every_product_semantic_sql_mutation() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = AgentRun::new_chat_run("sql-semantic-matrix", "");
        store.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "semantic_matrix_fixture",
            "builtin.semantic_matrix_fixture",
            "semantic matrix adapter body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        store.update_run(&run).unwrap();
        let (original_actions, original_observations): (String, String) = {
            let conn = store.conn.lock().unwrap();
            conn.query_row(
                "SELECT actions_json, observations_json FROM agent_runs WHERE id = ?1",
                [&run.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
        };
        let hmac = |hex: char| format!("hmac-sha256:{}", hex.to_string().repeat(64));
        let metadata_value = |kind: &str, digest: String| {
            serde_json::json!({
                "kind": kind,
                "byteCount": 1,
                "digest": digest,
                "contentStored": false,
            })
        };
        let cases = vec![
            (
                "action_type",
                true,
                "/0/actionType",
                serde_json::json!("read"),
            ),
            (
                "action_target",
                true,
                "/0/target",
                serde_json::json!(format!("action_target:bytes=1:{}", hmac('a'))),
            ),
            (
                "action_input",
                true,
                "/0/input",
                metadata_value("action_input", hmac('b')),
            ),
            (
                "action_status",
                true,
                "/0/status",
                serde_json::json!("failed"),
            ),
            (
                "action_permission",
                true,
                "/0/permissionDecision",
                serde_json::json!("blocked"),
            ),
            (
                "action_started_at",
                true,
                "/0/startedAt",
                serde_json::json!("2030-01-01T00:00:00Z"),
            ),
            (
                "action_finished_at",
                true,
                "/0/finishedAt",
                serde_json::json!("2030-01-01T00:00:01Z"),
            ),
            (
                "action_timestamp",
                true,
                "/0/timestamp",
                serde_json::json!("2030-01-01T00:00:02Z"),
            ),
            (
                "scope_tool_id",
                true,
                "/0/toolScope/toolId",
                serde_json::json!("memory.search"),
            ),
            (
                "scope_tool_name",
                true,
                "/0/toolScope/toolName",
                serde_json::json!("memory_search"),
            ),
            (
                "scope_source",
                true,
                "/0/toolScope/source",
                serde_json::json!("bundled"),
            ),
            (
                "scope_risk",
                true,
                "/0/toolScope/riskLevel",
                serde_json::json!("medium"),
            ),
            (
                "scope_capabilities",
                true,
                "/0/toolScope/capabilities",
                serde_json::json!(["read", "utility"]),
            ),
            (
                "scope_action_type",
                true,
                "/0/toolScope/actionType",
                serde_json::json!("read_only"),
            ),
            (
                "scope_confirmation",
                true,
                "/0/toolScope/requiresConfirmation",
                serde_json::json!(true),
            ),
            (
                "scope_allowed",
                true,
                "/0/toolScope/allowed",
                serde_json::json!(false),
            ),
            (
                "trace_step_index",
                true,
                "/0/reactTrace/stepIndex",
                serde_json::json!(99),
            ),
            (
                "trace_tool_call_index",
                true,
                "/0/reactTrace/toolCallIndex",
                serde_json::json!(99),
            ),
            (
                "trace_action_category",
                true,
                "/0/reactTrace/actionCategory",
                serde_json::json!("write"),
            ),
            (
                "trace_permission",
                true,
                "/0/reactTrace/permissionDecision",
                serde_json::json!("blocked"),
            ),
            (
                "trace_status",
                true,
                "/0/reactTrace/status",
                serde_json::json!("failed"),
            ),
            (
                "trace_proposal",
                true,
                "/0/reactTrace/proposalId",
                serde_json::json!(format!("proposal_id:bytes=1:{}", hmac('c'))),
            ),
            (
                "trace_observation_status",
                true,
                "/0/reactTrace/observationStatus",
                serde_json::json!(format!("observation_status:bytes=1:{}", hmac('d'))),
            ),
            (
                "trace_output_count",
                true,
                "/0/reactTrace/outputItemCount",
                serde_json::json!(99),
            ),
            (
                "receipt_store_identity",
                true,
                "/0/reactTrace/outputReceipt/canonicalStoreIdentity",
                serde_json::json!(format!("agent_run_store:{}", uuid::Uuid::new_v4())),
            ),
            (
                "receipt_binding",
                true,
                "/0/reactTrace/outputReceipt/bindingReceipt",
                serde_json::json!(hmac('e')),
            ),
            (
                "receipt_body",
                true,
                "/0/reactTrace/outputReceipt/bodyReceipt",
                serde_json::json!(hmac('f')),
            ),
            (
                "receipt_authority",
                true,
                "/0/reactTrace/outputReceipt/authorityTag",
                serde_json::json!(hmac('0')),
            ),
            (
                "observation_source",
                false,
                "/0/source",
                serde_json::json!("bundled"),
            ),
            (
                "observation_structured_result",
                false,
                "/0/structuredResult",
                metadata_value("observation_result", hmac('1')),
            ),
            (
                "observation_timestamp",
                false,
                "/0/timestamp",
                serde_json::json!("2030-01-01T00:00:03Z"),
            ),
        ];

        for (label, mutate_actions, pointer, replacement) in cases {
            let mut actions: serde_json::Value = serde_json::from_str(&original_actions).unwrap();
            let mut observations: serde_json::Value =
                serde_json::from_str(&original_observations).unwrap();
            let target = if mutate_actions {
                &mut actions
            } else {
                &mut observations
            };
            *target
                .pointer_mut(pointer)
                .unwrap_or_else(|| panic!("missing mutation pointer {pointer} for {label}")) =
                replacement;
            {
                let conn = store.conn.lock().unwrap();
                conn.execute(
                    "UPDATE agent_runs SET actions_json = ?2, observations_json = ?3 WHERE id = ?1",
                    params![run.id, actions.to_string(), observations.to_string()],
                )
                .unwrap();
            }
            assert!(
                store.get_run(&run.id).is_err(),
                "semantic mutation unexpectedly survived: {label}"
            );
            {
                let conn = store.conn.lock().unwrap();
                conn.execute(
                    "UPDATE agent_runs SET actions_json = ?2, observations_json = ?3 WHERE id = ?1",
                    params![run.id, original_actions, original_observations],
                )
                .unwrap();
            }
            assert!(
                store.get_run(&run.id).unwrap().is_some(),
                "restore failed: {label}"
            );
        }
    }

    #[tokio::test]
    async fn bound_content_receipt_rejects_semantic_swap_between_two_actions() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = AgentRun::new_chat_run("semantic-swap", "input");
        store.create_run(&run).unwrap();
        let (first_action, first_observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "memory_search",
            "builtin.memory.search",
            "first adapter body",
        )
        .await;
        let (second_action, second_observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            2,
            "calendar_lookup",
            "builtin.calendar.lookup",
            "second adapter body",
        )
        .await;
        run.actions = vec![first_action, second_action];
        run.observations = vec![first_observation, second_observation];
        store.update_run(&run).unwrap();

        {
            let conn = store.conn.lock().unwrap();
            let raw: String = conn
                .query_row(
                    "SELECT actions_json FROM agent_runs WHERE id = ?1",
                    [&run.id],
                    |row| row.get(0),
                )
                .unwrap();
            let mut actions: Vec<crate::agent::types::AgentAction> =
                serde_json::from_str(&raw).unwrap();
            let first_target = actions[0].target.clone();
            actions[0].target = actions[1].target.clone();
            actions[1].target = first_target;
            let first_scope = actions[0].tool_scope.clone();
            actions[0].tool_scope = actions[1].tool_scope.clone();
            actions[1].tool_scope = first_scope;
            let first_trace = actions[0].react_trace.as_ref().unwrap().clone();
            let second_trace = actions[1].react_trace.as_ref().unwrap().clone();
            {
                let trace = actions[0].react_trace.as_mut().unwrap();
                trace.action_type = second_trace.action_type;
                trace.tool_name = second_trace.tool_name;
                trace.tool_id = second_trace.tool_id;
                trace.tool_source = second_trace.tool_source;
                trace.risk_level = second_trace.risk_level;
            }
            {
                let trace = actions[1].react_trace.as_mut().unwrap();
                trace.action_type = first_trace.action_type;
                trace.tool_name = first_trace.tool_name;
                trace.tool_id = first_trace.tool_id;
                trace.tool_source = first_trace.tool_source;
                trace.risk_level = first_trace.risk_level;
            }
            conn.execute(
                "UPDATE agent_runs SET actions_json = ?2 WHERE id = ?1",
                rusqlite::params![run.id, serde_json::to_string(&actions).unwrap()],
            )
            .unwrap();
        }

        let error = store.get_run(&run.id).unwrap_err().to_string();
        assert!(error.contains("invalid_owner_graph"), "{error}");
    }

    #[tokio::test]
    async fn legacy_v1_bound_content_receipt_is_explicitly_unverified_and_fails_reload() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = AgentRun::new_chat_run("legacy-content-receipt", "input");
        store.create_run(&run).unwrap();
        let (action, observation) = observed_builtin_tool_output_graph(
            &store,
            &run.id,
            1,
            "memory_search",
            "builtin.memory.search",
            "adapter body",
        )
        .await;
        run.actions.push(action);
        run.observations.push(observation);
        store.update_run(&run).unwrap();

        {
            let conn = store.conn.lock().unwrap();
            let raw: String = conn
                .query_row(
                    "SELECT actions_json FROM agent_runs WHERE id = ?1",
                    [&run.id],
                    |row| row.get(0),
                )
                .unwrap();
            let mut actions: Vec<crate::agent::types::AgentAction> =
                serde_json::from_str(&raw).unwrap();
            let durable = actions[0]
                .react_trace
                .as_ref()
                .and_then(|trace| trace.output_receipt.as_ref())
                .unwrap();
            let legacy: crate::agent::types::ContentReceipt =
                serde_json::from_value(serde_json::json!({
                    "receiptId": durable.receipt_id(),
                    "runId": durable.run_id(),
                    "actionId": durable.action_id(),
                    "observationId": durable.observation_id(),
                    "field": "action_output_observation_content",
                    "kind": "tool_output",
                    "provenance": "observed_tool_adapter_body",
                    "byteCount": durable.byte_count(),
                    "opaqueBodyReceipt": format!("hmac-sha256:{}", "b".repeat(64)),
                    "authorityTag": format!("hmac-sha256:{}", "c".repeat(64)),
                }))
                .unwrap();
            assert!(legacy.is_legacy_unverified());
            actions[0].react_trace.as_mut().unwrap().output_receipt = Some(legacy);
            conn.execute(
                "UPDATE agent_runs SET actions_json = ?2 WHERE id = ?1",
                rusqlite::params![run.id, serde_json::to_string(&actions).unwrap()],
            )
            .unwrap();
        }

        let error = store.get_run(&run.id).unwrap_err().to_string();
        assert!(error.contains("invalid_owner_graph"), "{error}");
    }

    #[test]
    fn agent_action_runtime_execution_receipt_is_an_in_process_only_sidecar() {
        let receipt = crate::tool_execution_receipt::ToolExecutionReceipt::test_observed_local_read(
            Some("runtime-sidecar-run".into()),
            Some("memory.search".into()),
            "runtime-sidecar-request".into(),
            true,
        );
        let action = crate::agent::types::AgentAction {
            id: "runtime-sidecar-action".into(),
            action_type: "memory_search".into(),
            target: Some("memory.search".into()),
            input: serde_json::json!({}),
            output: None,
            status: "succeeded".into(),
            permission_decision: Some("read_only_memory_search".into()),
            started_at: None,
            finished_at: Some(chrono::Utc::now()),
            error: None,
            timestamp: chrono::Utc::now(),
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: Some(receipt),
        };

        let wire = serde_json::to_value(&action).unwrap();
        assert!(wire.get("runtimeExecutionReceipt").is_none());
        assert!(wire.get("executionReceipt").is_none());
        let decoded: crate::agent::types::AgentAction = serde_json::from_value(wire).unwrap();
        assert!(decoded.runtime_execution_receipt.is_none());
        assert!(action.runtime_execution_receipt.is_some());
    }

    #[test]
    fn legacy_agent_run_payloads_are_minimized_once_without_losing_action_references() {
        const SECRET: &str = "LEGACY_AGENT_RUN_PRIVATE_SENTINEL";
        let store = AgentRunStore::new_in_memory().unwrap();
        let now = chrono::Utc::now();
        let action = crate::agent::types::AgentAction {
            id: "legacy-action-id".into(),
            action_type: "mcp_tool".into(),
            target: Some(SECRET.into()),
            input: serde_json::json!({ "private": SECRET }),
            output: Some(serde_json::json!({ "private": SECRET })),
            status: "succeeded".into(),
            permission_decision: None,
            started_at: Some(now),
            finished_at: Some(now),
            error: None,
            timestamp: now,
            tool_scope: None,
            react_trace: None,
            runtime_execution_receipt: None,
        };
        let observation = crate::agent::types::AgentObservation {
            id: "legacy-observation-id".into(),
            action_id: Some(action.id.clone()),
            content: SECRET.into(),
            source: SECRET.into(),
            structured_result: Some(serde_json::json!({ "private": SECRET })),
            timestamp: now,
            react_trace: None,
        };
        let status_update = crate::agent::types::AgentLoopStatusUpdate {
            phase: crate::agent::types::AgentLoopPhase::Failed,
            message: SECRET.into(),
            step_index: 1,
            tool_call_index: Some(1),
            timestamp: now,
        };
        let route = crate::agent::types::ModelRouteTrace {
            provider: "openai".into(),
            model: "legacy-model".into(),
            route_type: "cloud".into(),
            prefer_local: false,
            local_model: "local-model".into(),
            reason: SECRET.into(),
            privacy_level: crate::agent::types::RedactionLevel::Strict,
            latency_ms: None,
            retry_count: 0,
            fallback_reason: Some(SECRET.into()),
            provider_health_is_estimated: Some(false),
        };
        let error = crate::agent::types::AgentRunError {
            message: SECRET.into(),
            phase: "model".into(),
            recoverable: false,
        };

        let conn = store.conn.lock().unwrap();
        install_legacy_raw_columns_for_test(&conn);
        conn.execute(
            "INSERT INTO agent_runs (
                id, task_id, status, kind, started_at, user_input,
                reasoning_trace_json, model_route_json, output_preview, error_json,
                actions_json, observations_json, status_updates_json, delete_reason,
                payload_minimized_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0)",
            params![
                "legacy-run-id",
                "legacy-task-id",
                AgentRunStatus::Running.to_string(),
                AgentTaskKind::Conversation.to_string(),
                now.to_rfc3339(),
                SECRET,
                serde_json::json!({ "input": SECRET }).to_string(),
                serde_json::to_string(&route).unwrap(),
                SECRET,
                serde_json::to_string(&error).unwrap(),
                serde_json::to_string(&vec![action]).unwrap(),
                serde_json::to_string(&vec![observation]).unwrap(),
                serde_json::to_string(&vec![status_update]).unwrap(),
                SECRET,
            ],
        )
        .unwrap();

        AgentRunStore::minimize_legacy_run_payloads(&conn, store.receipt_key.as_ref()).unwrap();
        AgentRunStore::rebuild_agent_runs_without_raw_columns(
            &conn,
            AgentRunTableRebuildFault::None,
        )
        .unwrap();
        let (payload, actions_json, observations_json, version): (String, String, String, i64) =
            conn.query_row(
                "SELECT COALESCE(model_route_json, '') || COALESCE(output_preview, '') ||
                        COALESCE(error_json, '') || COALESCE(actions_json, '') ||
                        COALESCE(observations_json, '') || COALESCE(status_updates_json, '') ||
                        COALESCE(delete_reason, ''),
                        COALESCE(actions_json, '[]'), COALESCE(observations_json, '[]'),
                        payload_minimized_version
                 FROM agent_runs WHERE id = 'legacy-run-id'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(version, AGENT_RUN_PAYLOAD_VERSION);
        assert!(!payload.contains(SECRET));
        assert!(!payload.contains("legacy-action-id"));
        assert!(!payload.contains("legacy-observation-id"));
        let actions: Vec<crate::agent::types::AgentAction> =
            serde_json::from_str(&actions_json).unwrap();
        let observations: Vec<crate::agent::types::AgentObservation> =
            serde_json::from_str(&observations_json).unwrap();
        assert_eq!(
            observations[0].action_id.as_deref(),
            Some(actions[0].id.as_str())
        );
        assert!(payload.contains("contentStored"));
        assert!(payload.contains("hmac-sha256:"));
    }

    #[test]
    fn legacy_agent_run_payload_migration_is_atomic_and_fails_closed_on_nonempty_malformed_sensitive_json(
    ) {
        let store = AgentRunStore::new_in_memory().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let connection = store.conn.lock().unwrap();
        install_legacy_raw_columns_for_test(&connection);
        for (run_id, task_id, user_input, actions_json) in [
            (
                "legacy-valid-before-malformed",
                "legacy-valid-task",
                "LEGACY_VALID_PRIVATE_BODY",
                "[]",
            ),
            (
                "legacy-malformed-sensitive-json",
                "legacy-malformed-task",
                "LEGACY_MALFORMED_PRIVATE_BODY",
                "{\"not\":\"a complete action array\"",
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO agent_runs (
                        id, task_id, status, kind, started_at, user_input,
                        actions_json, payload_minimized_version
                     ) VALUES (?1, ?2, 'running', 'conversation', ?3, ?4, ?5, 0)",
                    params![run_id, task_id, now, user_input, actions_json],
                )
                .unwrap();
        }

        let error =
            AgentRunStore::minimize_legacy_run_payloads(&connection, store.receipt_key.as_ref())
                .expect_err(
                    "one malformed non-empty sensitive JSON column must abort the migration",
                )
                .to_string();
        assert!(error.contains("actions_json"), "{error}");

        let rows = connection
            .prepare(
                "SELECT id, user_input, payload_minimized_version
                 FROM agent_runs
                 WHERE id IN ('legacy-malformed-sensitive-json',
                              'legacy-valid-before-malformed')
                 ORDER BY id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|(_, _, version)| *version == 0));
        assert!(rows
            .iter()
            .any(|(_, input, _)| input.as_deref() == Some("LEGACY_VALID_PRIVATE_BODY")));
    }

    #[test]
    fn legacy_agent_run_migration_rejects_every_nonempty_malformed_sensitive_json_column() {
        const SENSITIVE_JSON_COLUMNS: [&str; 10] = [
            "context_summary_json",
            "model_route_json",
            "error_json",
            "generated_proposals_json",
            "actions_json",
            "observations_json",
            "reasoning_trace_json",
            "hs_selection_audit_json",
            "behavior_checks_json",
            "status_updates_json",
        ];
        for column in SENSITIVE_JSON_COLUMNS {
            let store = AgentRunStore::new_in_memory().unwrap();
            let connection = store.conn.lock().unwrap();
            install_legacy_raw_columns_for_test(&connection);
            connection
                .execute(
                    "INSERT INTO agent_runs (
                        id, task_id, status, kind, started_at, payload_minimized_version
                     ) VALUES ('legacy-malformed-column', 'legacy-malformed-column-task',
                               'running', 'conversation', ?1, 0)",
                    [chrono::Utc::now().to_rfc3339()],
                )
                .unwrap();
            connection
                .execute(
                    &format!(
                        "UPDATE agent_runs SET {column} = '{{broken-json' \
                         WHERE id = 'legacy-malformed-column'"
                    ),
                    [],
                )
                .unwrap();

            let error = AgentRunStore::minimize_legacy_run_payloads(
                &connection,
                store.receipt_key.as_ref(),
            )
            .expect_err("every non-empty malformed sensitive JSON column must fail closed")
            .to_string();
            assert!(error.contains(column), "column={column} error={error}");
            let version: i64 = connection
                .query_row(
                    "SELECT payload_minimized_version FROM agent_runs
                     WHERE id = 'legacy-malformed-column'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(version, 0, "column={column}");
        }
    }

    #[test]
    fn persisted_run_minimizes_context_hs_behavior_and_strategy_free_text() {
        const SECRET: &str = "AGENT RUN unaudited free text sentinel repeated beyond every bounded metadata summary limit so an execution store can never retain this canonical user authored body even if a caller mislabels it as context strategy behavior or HS audit metadata; private marker 74291";
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        run.context_summary = Some(crate::agent::types::ContextSummary {
            life_model_empty: false,
            included_life_model_sections: vec![SECRET.into()],
            memory_hit_count: 1,
            memory_sources: vec![SECRET.into()],
            used_tools_prompt: true,
            redaction_applied: true,
            redaction_level: crate::agent::types::RedactionLevel::Strict,
        });
        run.reasoning_strategy = Some(SECRET.into());
        run.hs_selection_audit = Some(crate::agent::hs_selector::HSSelectionAudit {
            agent_task_id: Some(run.task_id.clone()),
            agent_run_id: Some(run.id.clone()),
            input_digest: format!("sha256:{}", "a".repeat(64)),
            selected_policy_ids: vec![SECRET.into()],
            selected_heuristic_ids: vec![SECRET.into()],
            selected_guidance_ids: vec![SECRET.into()],
            selected_guidance_refs: Vec::new(),
            excluded_assets: Vec::new(),
            estimated_tokens: 1,
            token_budget: 2,
        });
        run.behavior_checks = vec![crate::agent::types::HSBehaviorCheckSummary {
            id: "behavior-check-ref".into(),
            label: SECRET.into(),
            passed: false,
            summary: Some(SECRET.into()),
        }];

        store.create_run(&run).unwrap();

        let persisted: String = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COALESCE(context_summary_json, '') ||
                        COALESCE(reasoning_strategy, '') ||
                        COALESCE(hs_selection_audit_json, '') ||
                        COALESCE(behavior_checks_json, '')
                 FROM agent_runs WHERE id = ?1",
                [&run.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!persisted.contains(SECRET));
        assert!(persisted.contains("hmac-sha256:"));
    }

    #[test]
    fn context_summary_memory_refs_default_deny_forged_uri_payloads() {
        const FORGED_URI_SENTINEL: &str =
            "memory://PRIVATE-FORGED-URI-SENTINEL/not-a-canonical-owner";
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        run.context_summary = Some(crate::agent::types::ContextSummary {
            life_model_empty: true,
            included_life_model_sections: Vec::new(),
            memory_hit_count: 2,
            memory_sources: vec![
                "conversation://session-safe/message/42".into(),
                FORGED_URI_SENTINEL.into(),
            ],
            used_tools_prompt: false,
            redaction_applied: true,
            redaction_level: crate::agent::types::RedactionLevel::Strict,
        });

        store.create_run(&run).unwrap();
        let persisted = store.get_run(&run.id).unwrap().unwrap();
        let memory_sources = &persisted.context_summary.unwrap().memory_sources;
        assert_eq!(memory_sources[0], "conversation://session-safe/message/42");
        assert!(memory_sources[1].starts_with("memory_source_ref:bytes="));

        let durable: String = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT context_summary_json FROM agent_runs WHERE id = ?1",
                [&run.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!durable.contains(FORGED_URI_SENTINEL));
        assert!(durable.contains("hmac-sha256:"));
    }

    #[test]
    fn short_behavior_free_text_is_not_misclassified_as_safe_metadata() {
        const SHORT_SECRET: &str = "pin_74291";
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        run.behavior_checks = vec![crate::agent::types::HSBehaviorCheckSummary {
            id: "behavior-check-short-secret".into(),
            label: SHORT_SECRET.into(),
            passed: false,
            summary: Some(SHORT_SECRET.into()),
        }];
        store.create_run(&run).unwrap();

        let payload: String = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT behavior_checks_json FROM agent_runs WHERE id = ?1",
                [&run.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!payload.contains(SHORT_SECRET));
        assert!(payload.contains("behavior_check_label:bytes="));
        assert!(payload.contains("behavior_check_summary:bytes="));
    }

    #[test]
    fn current_agent_run_read_fails_closed_on_malformed_sensitive_json_instead_of_fabricating_empty_actions(
    ) {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run = create_test_run();
        store.create_run(&run).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_runs SET actions_json = '{broken-json' WHERE id = ?1",
                [&run.id],
            )
            .unwrap();

        let error = store
            .get_run(&run.id)
            .expect_err("current minimized rows must not fabricate empty action truth")
            .to_string();
        assert!(error.contains("actions_json"), "{error}");
    }

    #[test]
    fn test_fail_run() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        store.create_run(&run).unwrap();

        let error = AgentRunError {
            message: "model timeout".to_string(),
            phase: "model".to_string(),
            recoverable: true,
        };
        run.fail(error);
        store.update_run(&run).unwrap();

        let fetched = store.get_run(&run.id).unwrap().unwrap();
        assert_eq!(fetched.status, AgentRunStatus::Failed);
        assert!(fetched.error.is_some());
    }

    #[test]
    fn test_restore_run() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run = create_test_run();
        store.create_run(&run).unwrap();

        // Soft delete
        store
            .delete_run_with_tombstone(&run.id, Some("test deletion"))
            .unwrap();
        assert!(store.get_live_run(&run.id).unwrap().is_none());
        let fetched = store.get_run_including_deleted(&run.id).unwrap().unwrap();
        assert!(fetched.deleted_at.is_some());
        assert!(fetched.delete_reason.as_deref().is_some_and(|value| value
            .starts_with("delete_reason:bytes=")
            && value.contains("hmac-sha256:")));

        // Restore
        store.restore_run_with_receipt(&run.id).unwrap();
        let restored = store.get_live_run(&run.id).unwrap().unwrap();
        assert!(restored.deleted_at.is_none());
        assert!(restored.delete_reason.is_none());
    }

    #[test]
    fn agent_run_delete_and_tombstone_commit_together_without_reason_copy() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run = create_test_run();
        store.create_run(&run).unwrap();
        let receipt = store
            .delete_run_with_tombstone(&run.id, Some("PRIVATE_AGENT_DELETE_REASON"))
            .unwrap();

        assert!(store
            .get_run_including_deleted(&run.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some());
        let deliveries = store.list_replayable_projection_deliveries(10).unwrap();
        assert_eq!(deliveries.len(), 3);
        assert!(deliveries
            .iter()
            .all(|delivery| delivery.event_id == receipt.event_id));
        assert!(!serde_json::to_string(&deliveries)
            .unwrap()
            .contains("PRIVATE_AGENT_DELETE_REASON"));
    }

    #[test]
    fn agent_run_restore_persists_superseded_delete_fence_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent-runs.db");
        let (deleted, restored) = {
            let store = AgentRunStore::new(&path).unwrap();
            let run = create_test_run();
            store.create_run(&run).unwrap();
            let deleted = store
                .delete_run_with_tombstone(&run.id, Some("user delete"))
                .unwrap();
            store
                .mark_projection_degraded(
                    &deleted.event_id,
                    "life_event_store",
                    "injected failure before restore",
                )
                .unwrap();
            let restored = store.restore_run_with_receipt(&run.id).unwrap();
            assert_eq!(deleted.aggregate_revision, 1);
            assert_eq!(restored.aggregate_revision, 2);
            assert_eq!(
                store
                    .superseded_tombstone_ids_for_restore_event(&restored.event_id)
                    .unwrap(),
                vec![deleted.tombstone_id.clone().unwrap()]
            );
            (deleted, restored)
        };

        let reopened = AgentRunStore::new(&path).unwrap();
        assert!(reopened
            .list_replayable_projection_deliveries_for_event(&deleted.event_id)
            .unwrap()
            .is_empty());
        let replayable = reopened.list_replayable_projection_deliveries(20).unwrap();
        assert_eq!(replayable.len(), 3);
        assert!(replayable
            .iter()
            .all(|delivery| delivery.event_id == restored.event_id));
        assert!(replayable
            .iter()
            .all(|delivery| delivery.aggregate_revision == 2));
        let deleted_summary = reopened.projection_summary(&deleted.event_id).unwrap();
        assert_eq!(deleted_summary.pending, 0);
        assert_eq!(deleted_summary.degraded, 0);
        assert_eq!(deleted_summary.superseded, 3);
        assert_eq!(deleted_summary.state(), ProjectionDeliveryState::Superseded);
        assert_eq!(
            reopened
                .superseded_tombstone_ids_for_restore_event(&restored.event_id)
                .unwrap(),
            vec![deleted.tombstone_id.unwrap()]
        );
    }

    #[test]
    fn agent_run_restart_limit_one_selects_current_head_not_superseded_restore() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent-runs-causal-head.db");
        let (restored_stale, deleted_current) = {
            let store = AgentRunStore::new(&path).unwrap();
            let run = create_test_run();
            store.create_run(&run).unwrap();

            let deleted_first = store
                .delete_run_with_tombstone(&run.id, Some("first delete"))
                .unwrap();
            for target in ["turn_event_store", "action_queue_store", "life_event_store"] {
                store
                    .mark_projection_applied(&deleted_first.event_id, target)
                    .unwrap();
            }
            let restored_stale = store.restore_run_with_receipt(&run.id).unwrap();
            let deleted_current = store
                .delete_run_with_tombstone(&run.id, Some("current delete"))
                .unwrap();

            assert_eq!(deleted_first.aggregate_revision, 1);
            assert_eq!(restored_stale.aggregate_revision, 2);
            assert_eq!(deleted_current.aggregate_revision, 3);
            (restored_stale, deleted_current)
        };

        let reopened = AgentRunStore::new(&path).unwrap();
        let replayable = reopened.list_replayable_projection_deliveries(1).unwrap();
        assert_eq!(replayable.len(), 1);
        assert_eq!(replayable[0].event_id, deleted_current.event_id);
        assert_eq!(replayable[0].aggregate_revision, 3);

        let stale_summary = reopened
            .projection_summary(&restored_stale.event_id)
            .unwrap();
        assert_eq!(stale_summary.pending, 0);
        assert_eq!(stale_summary.degraded, 0);
        assert_eq!(stale_summary.superseded, 3);
        assert_eq!(stale_summary.state(), ProjectionDeliveryState::Superseded);
    }

    #[test]
    fn conversation_tombstone_projection_is_idempotent_and_keeps_cleanup_refs() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        run.session_id = Some("deleted-conversation".into());
        let expected = (run.id.clone(), run.task_id.clone());
        store.create_run(&run).unwrap();

        assert_eq!(
            store
                .project_conversation_tombstone("conversation-tombstone", "deleted-conversation")
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .project_conversation_tombstone("conversation-tombstone", "deleted-conversation")
                .unwrap(),
            0
        );
        assert!(store
            .list_runs_for_session("deleted-conversation", 10)
            .unwrap()
            .is_empty());
        assert!(store.restore_run_with_receipt(&run.id).is_err());
        assert_eq!(
            store
                .projection_refs_for_session("deleted-conversation")
                .unwrap(),
            vec![expected]
        );
        let mut late = create_test_run();
        late.session_id = Some("deleted-conversation".into());
        assert!(store.create_run(&late).is_err());
    }
}
