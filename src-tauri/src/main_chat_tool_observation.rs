const REPLAY_SYNTHESIS_OBSERVATION_SCHEMA: &str = "openlife_replay_synthesis_observation_v1";
const MAX_REPLAY_SYNTHESIS_TEXT_CHARS: usize = 700;
const MAX_REPLAY_WEB_RESULTS: usize = 4;
const MAX_REPLAY_WEB_SNIPPET_CHARS: usize = 700;

fn bounded_text(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn normalized_replay_web_observation(
    tool_name: &str,
    observation_content: &str,
) -> Result<openlife_core::web_search::WebSearchObservation, String> {
    let observed = if tool_name == "web.fetch" {
        openlife_core::web_search::WebSearchObservation::from_fetch_tool_output(observation_content)
    } else {
        openlife_core::web_search::WebSearchObservation::parse_tool_output(observation_content)
    }
    .map_err(|_| "replay_synthesis_web_observation_invalid".to_string())?;
    let mut normalized = observed;
    normalized.results.truncate(MAX_REPLAY_WEB_RESULTS);
    for result in &mut normalized.results {
        result.snippet = bounded_text(&result.snippet, MAX_REPLAY_WEB_SNIPPET_CHARS);
    }
    normalized
        .validate()
        .map_err(|_| "replay_synthesis_web_observation_invalid".to_string())?;
    Ok(normalized)
}

pub(crate) fn attach_replay_synthesis_observation(
    metadata: &mut serde_json::Value,
    tool_name: &str,
    observation_content: &str,
) {
    let Some(object) = metadata.as_object_mut() else {
        return;
    };
    let observation = if tool_name == "document.read" {
        serde_json::from_str::<serde_json::Value>(observation_content)
            .map_err(|_| "replay_synthesis_document_observation_invalid".to_string())
            .and_then(|parsed| {
                let selection_digest = parsed
                    .get("selectionDigest")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| value.starts_with("sha256:") && value.len() == 71)
                    .ok_or_else(|| "replay_synthesis_document_digest_missing".to_string())?;
                let selected_chunk_count = parsed
                    .get("selectedChunkCount")
                    .and_then(serde_json::Value::as_u64)
                    .filter(|count| *count > 0)
                    .ok_or_else(|| "replay_synthesis_document_count_missing".to_string())?;
                Ok(serde_json::json!({
                    "schemaVersion": REPLAY_SYNTHESIS_OBSERVATION_SCHEMA,
                    "kind": "document",
                    "selectionDigest": selection_digest,
                    "selectedChunkCount": selected_chunk_count,
                }))
            })
    } else if matches!(tool_name, "web.search" | "web.fetch") {
        normalized_replay_web_observation(tool_name, observation_content).map(|observed| {
            serde_json::json!({
                "schemaVersion": REPLAY_SYNTHESIS_OBSERVATION_SCHEMA,
                "kind": "web",
                "observation": observed,
            })
        })
    } else {
        let content = bounded_text(observation_content, MAX_REPLAY_SYNTHESIS_TEXT_CHARS);
        if content.trim().is_empty() {
            Err("replay_synthesis_read_observation_empty".into())
        } else {
            Ok(serde_json::json!({
                "schemaVersion": REPLAY_SYNTHESIS_OBSERVATION_SCHEMA,
                "kind": "read",
                "content": content,
            }))
        }
    };
    match observation {
        Ok(observation) => {
            object.insert("replaySynthesisObservation".into(), observation);
            object.insert(
                "replaySynthesisObservationStatus".into(),
                serde_json::json!("ready"),
            );
        }
        Err(reason_code) => {
            object.remove("replaySynthesisObservation");
            object.insert(
                "replaySynthesisObservationStatus".into(),
                serde_json::json!("invalid"),
            );
            object.insert(
                "replaySynthesisObservationError".into(),
                serde_json::json!(reason_code),
            );
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "read execution evidence is one bounded adapter record"
)]
pub(crate) fn attach_read_observation_metadata(
    metadata: &mut serde_json::Value,
    tool_name: &str,
    execution_target: &str,
    arguments: &serde_json::Value,
    output_preview: &str,
    structured_result: Option<serde_json::Value>,
    fixture_backed: bool,
    succeeded: bool,
) {
    let source_kind = match tool_name {
        "document.read" => "document",
        "file.read" => "file",
        "web.search" | "web.fetch" | "web.read" => "web",
        "mcp.read_only" => "mcp",
        "memory.search" => "memory",
        "session.search" => "session",
        _ => "tool",
    };
    let source_label = match tool_name {
        "document.read" => "current_task_bound_resources",
        "file.read" => arguments
            .get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(execution_target),
        "mcp.read_only" => execution_target,
        _ => execution_target,
    };
    let evidence_kind = match tool_name {
        "document.read" => "imported_resource_read",
        "file.read" => "file_system_read",
        "web.search" if fixture_backed => "web_search_fixture",
        "web.search" => "web_search_network",
        "web.fetch" => "web_fetch_network",
        "web.read" => "governed_read",
        "mcp.read_only" => "registered_mcp_read",
        "memory.search" => "memory_read",
        "session.search" => "session_read",
        _ => "governed_read",
    };
    let network_read_attempted = matches!(tool_name, "web.search" | "web.fetch") && !fixture_backed;
    let real_read_only_execution = succeeded && !fixture_backed;
    let preview = if output_preview.trim().is_empty() {
        format!("{source_kind} read completed from {source_label}")
    } else {
        bounded_text(output_preview, 500)
    };
    let read_evidence = serde_json::json!({
        "kind": evidence_kind,
        "sourceKind": source_kind,
        "sourceLabel": source_label,
        "target": execution_target,
        "realReadOnlyExecution": real_read_only_execution,
        "fixtureBacked": fixture_backed,
        "networkReadAttempted": network_read_attempted,
        "directWritesExecuted": false,
    });
    if let Some(object) = metadata.as_object_mut() {
        object.insert("sourceKind".into(), serde_json::json!(source_kind));
        object.insert("sourceLabel".into(), serde_json::json!(source_label));
        object.insert("preview".into(), serde_json::json!(preview));
        let mut structured = structured_result.unwrap_or_else(|| serde_json::json!({}));
        if let Some(structured_object) = structured.as_object_mut() {
            structured_object.insert("readExecutionEvidence".into(), read_evidence);
            structured_object.insert("directWritesExecuted".into(), serde_json::json!(false));
        } else {
            structured = serde_json::json!({
                "readExecutionEvidence": read_evidence,
                "directWritesExecuted": false,
            });
        }
        object.insert("structuredResult".into(), structured);
    }
}

pub(crate) fn typed_permission_code(value: Option<&str>) -> Option<&'static str> {
    match value {
        Some("allow") => Some("allow"),
        Some("allow_once") => Some("allow_once"),
        Some("action_bound_allow_once") => Some("action_bound_allow_once"),
        Some("action_bound_allow_once_peek") => Some("action_bound_allow_once_peek"),
        Some("action_bound_allow_once_already_consumed") => {
            Some("action_bound_allow_once_already_consumed")
        }
        Some("action_bound_scope_mismatch") => Some("action_bound_scope_mismatch"),
        Some("deny") => Some("deny"),
        Some("ask") => Some("ask"),
        Some("ask_every_time") => Some("ask_every_time"),
        Some("expired") => Some("expired"),
        Some("blocked") => Some("blocked"),
        Some("proposal_required") => Some("proposal_required"),
        Some("tool_permission_required") => Some("tool_permission_required"),
        Some("network_policy_consent_required") => Some("network_policy_consent_required"),
        Some("mcp_read_tool_not_registered") => Some("mcp_read_tool_not_registered"),
        Some(value) if value.starts_with("network_") => Some("network_policy_blocked"),
        _ => None,
    }
}
