use crate::main_chat_react_tool_selection::{
    main_chat_governed_mcp_read_tool_candidates, main_chat_manifest_has_write_like_surface,
    main_chat_manifest_is_governed_read_candidate, main_chat_surface_contains_write_like_term,
};
use crate::AppState;
use chrono::{DateTime, Utc};
use openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus;
use openlife_core::skills::{SkillExecutionStatus, SkillManifest, SkillSourceKind};
use openlife_core::tool_manifest::ToolManifest;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAX_SKILL_PREVIEW_CHARS: usize = 900;
const MAX_SKILL_SUMMARY_CHARS: usize = 220;
const SKILL_SURFACE_SCOPE: &str = "session";
const PRODUCT_SKILL_MARKER: &str = "<!-- openlife-product-skill -->";

fn manifest_product_available(manifest: &SkillManifest) -> bool {
    manifest.source_kind == SkillSourceKind::BuiltIn
        && manifest.execution_status == SkillExecutionStatus::ExecutableBuiltIn
        && !manifest.execution_budget.allow_writes
        && manifest
            .capability_flags
            .iter()
            .any(|flag| flag == "main_chat_turn_runtime_native")
}

fn manifest_source_kind(manifest: &SkillManifest) -> &'static str {
    match manifest.source_kind {
        SkillSourceKind::BuiltIn => "bundled",
        SkillSourceKind::Plugin => "plugin",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatSkillSummary {
    pub skill_id: String,
    pub name: String,
    pub source: String,
    pub scope: String,
    pub description: String,
    pub risk_level: String,
    pub available: bool,
    pub selected: bool,
    pub instruction_digest: String,
    pub source_kind: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatSkillDetail {
    pub skill_id: String,
    pub manifest: Value,
    pub bounded_instructions_preview: String,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub policy_notes: Vec<String>,
    pub required_permissions: Vec<String>,
    pub evidence_digest: String,
    pub redaction_summary: String,
    pub last_modified_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatSelectedSkill {
    pub session_id: String,
    pub selected_skill_id: Option<String>,
    pub selected_skill_digest: Option<String>,
    pub selection_reason: String,
    pub bounded_instructions_preview: String,
    pub evidence_digest: String,
    pub policy_notes: Vec<String>,
    pub included_as_bounded_context_only: bool,
    pub unselected_skills_injected: bool,
    pub controls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatToolCandidate {
    pub candidate_id: String,
    pub tool_name: String,
    pub source: String,
    pub capability_labels: Vec<String>,
    pub risk_level: String,
    pub selection_reason: String,
    pub policy_decision: String,
    pub requires_permission: bool,
    pub candidate_digest: String,
    pub linked_action_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatBlockedTool {
    pub tool_name: String,
    pub reason_code: String,
    pub policy_decision: String,
    pub requires_permission: bool,
    pub blocker_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatToolFailureRecovery {
    pub failed_candidate_id: String,
    pub failure_reason: String,
    pub retry_available: bool,
    pub alternative_candidate_id: Option<String>,
    pub controls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatToolCandidateList {
    pub task_session_id: Option<String>,
    pub candidates: Vec<MainChatToolCandidate>,
    pub blocked_tools: Vec<MainChatBlockedTool>,
    pub failure_recovery: Option<MainChatToolFailureRecovery>,
    pub evidence_digest: String,
    pub controls: Vec<String>,
}

#[derive(Debug, Clone)]
struct LocalSkillRecord {
    skill_id: String,
    name: String,
    source: String,
    source_kind: String,
    preview: String,
    digest: String,
    description: String,
    risk_level: String,
    available: bool,
    last_modified_at: Option<String>,
    redaction_summary: String,
}

pub(crate) async fn list_main_chat_skills_with_state(
    state: &Arc<AppState>,
    session_id: Option<&str>,
) -> Result<Vec<MainChatSkillSummary>, String> {
    let selected_skill_id = selected_skill_id_for_session(state, session_id).await?;
    let mut summaries = Vec::new();
    let mut seen = BTreeSet::new();

    let local_records = tokio::task::spawn_blocking(discover_local_skill_records)
        .await
        .map_err(|error| format!("local skill discovery task failed: {error}"))?;
    for record in local_records {
        seen.insert(record.skill_id.clone());
        summaries.push(skill_summary(record, selected_skill_id.as_deref()));
    }

    let registry = state.skill_registry.lock().await;
    for manifest in registry.list() {
        if seen.contains(&manifest.id) {
            continue;
        }
        let digest = digest_label_for_value(&json!({
            "id": manifest.id,
            "name": manifest.name,
            "description": manifest.description,
            "allowedTools": manifest.allowed_tools,
            "executionStatus": manifest.execution_status,
        }));
        let available = manifest_product_available(&manifest);
        let source_kind = manifest_source_kind(&manifest).to_string();
        summaries.push(MainChatSkillSummary {
            skill_id: manifest.id.clone(),
            name: manifest.name,
            source: "bundled:skill_registry".into(),
            scope: SKILL_SURFACE_SCOPE.into(),
            description: manifest.description,
            risk_level: if manifest.execution_budget.allow_writes {
                "medium"
            } else {
                "low"
            }
            .into(),
            available,
            selected: selected_skill_id.as_deref() == Some(manifest.id.as_str()),
            instruction_digest: digest,
            source_kind,
            last_used_at: None,
        });
    }

    summaries.sort_by(|left, right| {
        right
            .available
            .cmp(&left.available)
            .then_with(|| left.skill_id.cmp(&right.skill_id))
    });
    Ok(summaries)
}

pub(crate) async fn get_main_chat_skill_detail_with_state(
    state: &Arc<AppState>,
    skill_id: &str,
) -> Result<MainChatSkillDetail, String> {
    let skill_id = sanitize_skill_id(skill_id).ok_or_else(|| "invalid_skill_id".to_string())?;
    let local_records = tokio::task::spawn_blocking(discover_local_skill_records)
        .await
        .map_err(|error| format!("local skill discovery task failed: {error}"))?;
    if let Some(record) = local_records
        .into_iter()
        .find(|record| record.skill_id == skill_id)
    {
        return Ok(skill_detail_for_record(&record, state).await);
    }

    let registry = state.skill_registry.lock().await;
    let manifest = registry
        .get(&skill_id)
        .ok_or_else(|| "skill_not_found".to_string())?;
    let prompt = registry
        .build_system_prompt(&skill_id)
        .unwrap_or_else(|_| manifest.description.clone());
    let preview = bounded_redacted_preview(&prompt).0;
    let evidence_digest = digest_label_for_value(&json!({
        "skillId": skill_id,
        "previewDigest": digest_label(preview.as_bytes()),
        "allowedTools": manifest.allowed_tools,
        "proposalPolicy": manifest.proposal_policy,
    }));
    Ok(MainChatSkillDetail {
        skill_id,
        manifest: json!({
            "name": manifest.name,
            "source": format!("{}:skill_registry", manifest_source_kind(&manifest)),
            "sourceKind": manifest_source_kind(&manifest),
            "available": manifest_product_available(&manifest),
            "executionStatus": manifest.execution_status,
        }),
        bounded_instructions_preview: preview,
        allowed_tools: manifest.allowed_tools,
        disallowed_tools: if manifest.execution_budget.allow_writes {
            vec!["write_budget".into()]
        } else {
            Vec::new()
        },
        policy_notes: skill_policy_notes(),
        required_permissions: Vec::new(),
        evidence_digest,
        redaction_summary: "bounded_preview_from_bundled_manifest".into(),
        last_modified_at: None,
    })
}

pub(crate) async fn select_main_chat_skill_with_state(
    state: &Arc<AppState>,
    session_id: &str,
    skill_id: &str,
) -> Result<MainChatSelectedSkill, String> {
    let session_id =
        sanitize_session_id(session_id).ok_or_else(|| "invalid_session_id".to_string())?;
    let detail = get_main_chat_skill_detail_with_state(state, skill_id).await?;
    let available = list_main_chat_skills_with_state(state, Some(&session_id))
        .await?
        .into_iter()
        .find(|summary| summary.skill_id == detail.skill_id)
        .map(|summary| summary.available)
        .unwrap_or(false);
    if !available {
        return Err("skill_not_available_for_main_chat_context".into());
    }
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| error.to_string())?;
    state
        .memory_store
        .lock()
        .await
        .set_chat_session_selected_skill(&session_id, Some(&detail.skill_id))
        .map_err(|error| error.to_string())?;
    Ok(selection_from_detail(
        &session_id,
        Some(&detail),
        "user_selected_local_skill",
        vec!["clear_skill".into()],
    ))
}

pub(crate) async fn clear_main_chat_skill_with_state(
    state: &Arc<AppState>,
    session_id: &str,
) -> Result<MainChatSelectedSkill, String> {
    let session_id =
        sanitize_session_id(session_id).ok_or_else(|| "invalid_session_id".to_string())?;
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| error.to_string())?;
    state
        .memory_store
        .lock()
        .await
        .set_chat_session_selected_skill(&session_id, None)
        .map_err(|error| error.to_string())?;
    Ok(MainChatSelectedSkill {
        session_id,
        selected_skill_id: None,
        selected_skill_digest: None,
        selection_reason: "user_cleared_local_skill".into(),
        bounded_instructions_preview: String::new(),
        evidence_digest: digest_label_for_value(&json!({
            "selection": "cleared",
            "unselectedSkillsInjected": false,
        })),
        policy_notes: vec!["Next task context has no selected skill.".into()],
        included_as_bounded_context_only: false,
        unselected_skills_injected: false,
        controls: vec!["select_skill".into()],
    })
}

pub(crate) async fn list_main_chat_tool_candidates_with_state(
    state: &Arc<AppState>,
    task_session_id: Option<&str>,
) -> Result<MainChatToolCandidateList, String> {
    let task_session_id = task_session_id.and_then(sanitize_optional_id);
    let registry = state.mcp_registry.lock().await;
    let safe_candidates = main_chat_governed_mcp_read_tool_candidates(&registry, "", 12)
        .into_iter()
        .map(|candidate| {
            let candidate_digest = digest_label_for_value(&json!({
                "candidateId": candidate.candidate_id,
                "toolName": candidate.target,
                "source": candidate.manifest_source,
                "capabilityLabels": candidate.capabilities,
                "policyDecision": "allow",
            }));
            MainChatToolCandidate {
                candidate_id: candidate.candidate_id,
                tool_name: candidate.target,
                source: candidate.manifest_source,
                capability_labels: candidate.capabilities,
                risk_level: "low".into(),
                selection_reason: candidate.match_reason,
                policy_decision: "allow".into(),
                requires_permission: false,
                candidate_digest,
                linked_action_id: None,
            }
        })
        .collect::<Vec<_>>();

    let mut blocked_tools = registry
        .list_manifests()
        .into_iter()
        .filter(|manifest| !main_chat_manifest_is_governed_read_candidate(manifest))
        .filter_map(blocked_tool_from_manifest)
        .collect::<Vec<_>>();
    blocked_tools.sort_by(|left, right| left.tool_name.cmp(&right.tool_name));
    blocked_tools.truncate(64);
    drop(registry);

    let failure_recovery = tool_failure_recovery(state, task_session_id.as_deref()).await;
    let mut controls = Vec::new();
    if failure_recovery.is_some() {
        controls.extend(["retry_tool".into(), "switch_tool".into()]);
    }
    let evidence_digest = digest_label_for_value(&json!({
        "taskSessionId": task_session_id,
        "candidateCount": safe_candidates.len(),
        "blockedToolCount": blocked_tools.len(),
        "failureRecovery": failure_recovery,
        "directWritesExecuted": false,
    }));
    Ok(MainChatToolCandidateList {
        task_session_id,
        candidates: safe_candidates,
        blocked_tools,
        failure_recovery,
        evidence_digest,
        controls,
    })
}

async fn tool_failure_recovery(
    state: &Arc<AppState>,
    task_session_id: Option<&str>,
) -> Option<MainChatToolFailureRecovery> {
    let task_session_id = task_session_id?;
    let store_arc = state.main_chat_action_queue_store.as_ref()?;
    let store = store_arc.lock().await;
    let failed = store
        .list_for_session(task_session_id)
        .ok()?
        .into_iter()
        .find(|action| {
            action.status == ExecutionQueueStatus::Failed
                && openlife_core::agent::main_chat_agent_v1::typed_tool_receipt_allows_automatic_retry(
                    action,
                )
        })?;
    Some(MainChatToolFailureRecovery {
        failed_candidate_id: failed
            .observation_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("candidateId"))
            .and_then(Value::as_str)
            .unwrap_or(&failed.action.action_type)
            .to_string(),
        failure_reason: failed.error.unwrap_or_else(|| "tool_failed_once".into()),
        retry_available: true,
        alternative_candidate_id: Some("builtin_echo".into()),
        controls: vec!["retry_tool".into(), "switch_tool".into()],
    })
}

fn discover_local_skill_records() -> Vec<LocalSkillRecord> {
    let mut roots = Vec::new();
    if let Ok(workspace) = crate::workspace_file_resolver::resolve_workspace_root() {
        roots.push(("workspace".to_string(), workspace));
    }
    if let Ok(current) = std::env::current_dir() {
        if !roots.iter().any(|(_, root)| root == &current) {
            roots.push(("workspace".to_string(), current));
        }
    }
    if let Ok(configured) = std::env::var("OPENLIFE_KNOWLEDGE_ROOT") {
        let trimmed = configured.trim();
        if !trimmed.is_empty() {
            roots.push(("project".to_string(), PathBuf::from(trimmed)));
        }
    }

    let mut records = Vec::new();
    let mut seen = BTreeSet::new();
    for (source_kind, root) in roots {
        let Ok(root) = root.canonicalize() else {
            continue;
        };
        let skills_dir = root.join("skills");
        let Ok(entries) = std::fs::read_dir(&skills_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(skill_id) = path
                .file_name()
                .and_then(|value| value.to_str())
                .and_then(sanitize_skill_id)
            else {
                continue;
            };
            if !seen.insert(skill_id.clone()) {
                continue;
            }
            let skill_path = path.join("SKILL.md");
            if let Some(record) =
                local_skill_record_from_path(&root, &skill_path, &skill_id, &source_kind)
            {
                records.push(record);
            }
        }
    }
    records
}

fn local_skill_record_from_path(
    root: &Path,
    path: &Path,
    skill_id: &str,
    source_kind: &str,
) -> Option<LocalSkillRecord> {
    let canonical = path.canonicalize().ok()?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return None;
    }
    let content = std::fs::read_to_string(&canonical).ok()?;
    if !content
        .lines()
        .any(|line| line.trim() == PRODUCT_SKILL_MARKER)
    {
        return None;
    }
    let (preview, redaction_summary) = bounded_redacted_preview(&content);
    let digest = digest_label(content.as_bytes());
    let metadata = std::fs::metadata(&canonical).ok();
    let last_modified_at = metadata
        .and_then(|metadata| metadata.modified().ok())
        .map(DateTime::<Utc>::from)
        .map(|timestamp| timestamp.to_rfc3339());
    let risk_level = if skill_content_write_like(&content) {
        "high"
    } else {
        "low"
    }
    .to_string();
    let description = skill_description_from_content(&content);
    Some(LocalSkillRecord {
        skill_id: skill_id.into(),
        name: skill_name_from_content(skill_id, &content),
        source: format!("{source_kind}:skills/{skill_id}/SKILL.md"),
        source_kind: source_kind.into(),
        preview,
        digest,
        description,
        risk_level: risk_level.clone(),
        available: risk_level != "high",
        last_modified_at,
        redaction_summary,
    })
}

fn skill_summary(
    record: LocalSkillRecord,
    selected_skill_id: Option<&str>,
) -> MainChatSkillSummary {
    MainChatSkillSummary {
        skill_id: record.skill_id.clone(),
        name: record.name,
        source: record.source,
        scope: SKILL_SURFACE_SCOPE.into(),
        description: bounded_text(&record.description, MAX_SKILL_SUMMARY_CHARS),
        risk_level: record.risk_level,
        available: record.available,
        selected: selected_skill_id == Some(record.skill_id.as_str()),
        instruction_digest: record.digest,
        source_kind: record.source_kind,
        last_used_at: None,
    }
}

async fn skill_detail_for_record(
    record: &LocalSkillRecord,
    state: &Arc<AppState>,
) -> MainChatSkillDetail {
    let tool_surface = list_main_chat_tool_candidates_with_state(state, None)
        .await
        .ok();
    let allowed_tools = tool_surface
        .as_ref()
        .map(|surface| {
            surface
                .candidates
                .iter()
                .take(8)
                .map(|candidate| candidate.tool_name.clone())
                .collect()
        })
        .unwrap_or_default();
    let disallowed_tools = tool_surface
        .as_ref()
        .map(|surface| {
            surface
                .blocked_tools
                .iter()
                .take(8)
                .map(|tool| tool.tool_name.clone())
                .collect()
        })
        .unwrap_or_default();
    let evidence_digest = digest_label_for_value(&json!({
        "skillId": record.skill_id,
        "instructionDigest": record.digest,
        "previewDigest": digest_label(record.preview.as_bytes()),
        "available": record.available,
        "allowedTools": allowed_tools,
        "disallowedTools": disallowed_tools,
    }));
    MainChatSkillDetail {
        skill_id: record.skill_id.clone(),
        manifest: json!({
            "name": record.name,
            "source": record.source,
            "sourceKind": record.source_kind,
            "available": record.available,
            "instructionDigest": record.digest,
        }),
        bounded_instructions_preview: record.preview.clone(),
        allowed_tools,
        disallowed_tools,
        policy_notes: skill_policy_notes(),
        required_permissions: Vec::new(),
        evidence_digest,
        redaction_summary: record.redaction_summary.clone(),
        last_modified_at: record.last_modified_at.clone(),
    }
}

fn selection_from_detail(
    session_id: &str,
    detail: Option<&MainChatSkillDetail>,
    selection_reason: &str,
    controls: Vec<String>,
) -> MainChatSelectedSkill {
    let selected_skill_id = detail.map(|detail| detail.skill_id.clone());
    let selected_skill_digest = detail
        .and_then(|detail| {
            detail
                .manifest
                .get("instructionDigest")
                .and_then(Value::as_str)
        })
        .map(str::to_string)
        .or_else(|| {
            detail.map(|detail| digest_label(detail.bounded_instructions_preview.as_bytes()))
        });
    MainChatSelectedSkill {
        session_id: session_id.into(),
        selected_skill_id,
        selected_skill_digest,
        selection_reason: selection_reason.into(),
        bounded_instructions_preview: detail
            .map(|detail| detail.bounded_instructions_preview.clone())
            .unwrap_or_default(),
        evidence_digest: detail
            .map(|detail| detail.evidence_digest.clone())
            .unwrap_or_else(|| digest_label_for_value(&json!({"selection": "none"}))),
        policy_notes: detail
            .map(|detail| detail.policy_notes.clone())
            .unwrap_or_else(skill_policy_notes),
        included_as_bounded_context_only: detail.is_some(),
        unselected_skills_injected: false,
        controls,
    }
}

async fn selected_skill_id_for_session(
    state: &Arc<AppState>,
    session_id: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(session_id) = session_id else {
        return Ok(None);
    };
    state
        .persistence_coordinator
        .require_trusted_read("MemoryStore")
        .map_err(|error| error.to_string())?;
    state
        .memory_store
        .lock()
        .await
        .chat_session_selected_skill(session_id)
        .map_err(|error| error.to_string())
}

fn blocked_tool_from_manifest(manifest: ToolManifest) -> Option<MainChatBlockedTool> {
    if manifest.name == "mcp.call_tool" || !manifest.enabled || manifest.declarative_only {
        return Some(MainChatBlockedTool {
            tool_name: safe_tool_name(&manifest),
            reason_code: if manifest.declarative_only {
                "declarative_only_tool_blocked"
            } else {
                "tool_unavailable"
            }
            .into(),
            policy_decision: "blocked".into(),
            requires_permission: false,
            blocker_id: Some(stable_blocker_id(&manifest.name, "tool_unavailable")),
        });
    }
    let high_risk = matches!(
        manifest.risk_level.to_ascii_lowercase().as_str(),
        "high" | "critical"
    ) || matches!(
        manifest.permission_level.to_ascii_lowercase().as_str(),
        "high" | "critical"
    );
    let write_like = matches!(
        manifest.action_type.to_ascii_lowercase().as_str(),
        "write" | "external_side_effect"
    ) || manifest.capabilities.iter().any(|capability| {
        matches!(
            capability.to_ascii_lowercase().as_str(),
            "write" | "external_side_effect"
        )
    }) || main_chat_manifest_has_write_like_surface(&manifest);
    if high_risk || write_like || manifest.requires_confirmation {
        let reason_code = if write_like {
            "write_like_tool_blocked"
        } else if high_risk {
            "high_risk_tool_blocked"
        } else {
            "permission_required"
        };
        return Some(MainChatBlockedTool {
            tool_name: safe_tool_name(&manifest),
            reason_code: reason_code.into(),
            policy_decision: if manifest.requires_confirmation || high_risk {
                "permission_required"
            } else {
                "blocked"
            }
            .into(),
            requires_permission: manifest.requires_confirmation || high_risk,
            blocker_id: Some(stable_blocker_id(&manifest.name, reason_code)),
        });
    }
    None
}

fn safe_tool_name(manifest: &ToolManifest) -> String {
    if manifest
        .name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        manifest.name.clone()
    } else {
        "contract_unsafe_tool".into()
    }
}

fn stable_blocker_id(tool_name: &str, reason: &str) -> String {
    format!("blocker_{}", short_hash(&format!("{tool_name}:{reason}")))
}

fn skill_policy_notes() -> Vec<String> {
    vec![
        "Selected SKILL.md is bounded context, not authority.".into(),
        "Privacy, model route, ExecutionPolicy, and ToolPermission policy stay higher priority."
            .into(),
        "Unselected skills are not injected into the Main Chat context.".into(),
    ]
}

fn bounded_redacted_preview(content: &str) -> (String, String) {
    let mut redacted = 0usize;
    let mut output = String::new();
    for line in content.lines() {
        let lower = line.to_ascii_lowercase();
        if ["api_key", "apikey", "secret", "token", "password"]
            .iter()
            .any(|term| lower.contains(term))
        {
            redacted += 1;
            continue;
        }
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(line);
        if output.chars().count() >= MAX_SKILL_PREVIEW_CHARS {
            break;
        }
    }
    let preview = bounded_text(&output, MAX_SKILL_PREVIEW_CHARS);
    let summary = if redacted == 0 {
        "bounded_preview_no_secrets".into()
    } else {
        format!("bounded_preview_redacted_secret_lines:{redacted}")
    };
    (preview, summary)
}

fn skill_name_from_content(skill_id: &str, content: &str) -> String {
    content
        .lines()
        .find_map(|line| line.trim().strip_prefix("# "))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            skill_id
                .split(['_', '-'])
                .filter(|part| !part.is_empty())
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        Some(first) => {
                            format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
                        }
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
}

fn skill_description_from_content(content: &str) -> String {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("<!--"))
        .unwrap_or("Local Main Chat skill instructions.")
        .to_string()
}

fn skill_content_write_like(content: &str) -> bool {
    content
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '.' && ch != '_')
        .any(main_chat_surface_contains_write_like_term)
}

fn sanitize_skill_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || Path::new(trimmed).is_absolute()
    {
        return None;
    }
    trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        .then(|| trimmed.to_string())
}

fn sanitize_session_id(value: &str) -> Option<String> {
    sanitize_optional_id(value)
}

fn sanitize_optional_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        .then(|| trimmed.to_string())
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn digest_label_for_value(value: &Value) -> String {
    let serialized = serde_json::to_vec(value).unwrap_or_default();
    digest_label(&serialized)
}

fn digest_label(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("bytes:{} hash:sha256:{:x}", bytes.len(), hasher.finalize())
}

fn short_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    digest.chars().take(16).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn registry_skill_without_turn_runtime_native_contract_is_not_selectable() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        state
            .memory_store
            .lock()
            .await
            .create_chat_session("skill-truth", "Skill truth")
            .unwrap();
        let skills = list_main_chat_skills_with_state(&state, Some("skill-truth"))
            .await
            .unwrap();
        let weekly_review = skills
            .iter()
            .find(|skill| skill.skill_id == "weekly_review")
            .expect("registry built-in remains inspectable");
        assert!(!weekly_review.available);

        let error = select_main_chat_skill_with_state(&state, "skill-truth", "weekly_review")
            .await
            .expect_err("a skill with no TurnRuntime-native context path must fail closed");
        assert_eq!(error, "skill_not_available_for_main_chat_context");
        assert_eq!(
            state
                .memory_store
                .lock()
                .await
                .chat_session_selected_skill("skill-truth")
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn product_skill_catalog_excludes_unmarked_repository_fixtures() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        state
            .memory_store
            .lock()
            .await
            .create_chat_session("skill-catalog", "Skill catalog")
            .unwrap();
        let skills = list_main_chat_skills_with_state(&state, Some("skill-catalog"))
            .await
            .unwrap();
        assert!(skills
            .iter()
            .any(|skill| skill.skill_id == "evidence_review" && skill.available));
        for fixture_only in [
            "planning_review",
            "unselected_context",
            "unselected_sensitive",
        ] {
            assert!(
                skills.iter().all(|skill| skill.skill_id != fixture_only),
                "unmarked repository fixture must not enter the product catalog: {fixture_only}"
            );
        }
    }

    #[tokio::test]
    async fn product_skill_selection_uses_the_conversation_store_as_owner() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        state
            .memory_store
            .lock()
            .await
            .create_chat_session("skill-persisted", "Skill persisted")
            .unwrap();

        let selected =
            select_main_chat_skill_with_state(&state, "skill-persisted", "evidence_review")
                .await
                .expect("select product skill");
        assert_eq!(
            selected.selected_skill_id.as_deref(),
            Some("evidence_review")
        );
        assert_eq!(
            state
                .memory_store
                .lock()
                .await
                .chat_session_selected_skill("skill-persisted")
                .unwrap()
                .as_deref(),
            Some("evidence_review")
        );

        clear_main_chat_skill_with_state(&state, "skill-persisted")
            .await
            .expect("clear product skill");
        assert_eq!(
            state
                .memory_store
                .lock()
                .await
                .chat_session_selected_skill("skill-persisted")
                .unwrap(),
            None
        );
    }
}
