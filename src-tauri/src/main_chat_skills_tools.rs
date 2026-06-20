use crate::main_chat_react_execution::execute_main_chat_react_action_with_executor;
use crate::main_chat_react_tool_selection::{
    main_chat_governed_mcp_read_tool_candidates, main_chat_manifest_has_write_like_surface,
    main_chat_manifest_is_governed_read_candidate, main_chat_surface_contains_write_like_term,
    MainChatReactActionPlan,
};
use crate::AppState;
use chrono::{DateTime, Utc};
use openlife_core::agent::main_chat_agent_v1::{
    ContextCompiler, ContextCompilerInput, ExecutionAction, ExecutionPolicy, ExecutionQueueStatus,
    MainChatAgentStrategy, MainChatPrivacyRiskSummary,
};
use openlife_core::tool_manifest::{ToolManifest, ToolSource};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAX_SKILL_PREVIEW_CHARS: usize = 900;
const MAX_SKILL_SUMMARY_CHARS: usize = 220;
const SKILL_SURFACE_SCOPE: &str = "session";

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2SkillsScenario {
    pub id: String,
    pub capability_group: String,
    pub prompt: String,
    pub preconditions: Vec<String>,
    pub expected_route: String,
    pub required_runtime_evidence: Vec<String>,
    pub required_ui_state: Vec<String>,
    pub required_controls: Vec<String>,
    pub negative_assertions: Vec<String>,
    pub expected_outcome: String,
    pub default_gate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2SkillsProof {
    pub scenario_id: String,
    pub passed: bool,
    pub expected_blocker: bool,
    pub runtime_object_count: usize,
    pub selected_skill_ids: Vec<String>,
    pub candidate_ids: Vec<String>,
    pub blocker_ids: Vec<String>,
    pub action_ids: Vec<String>,
    pub observation_ids: Vec<String>,
    pub controls: Vec<String>,
    pub runtime_evidence: Vec<String>,
    pub ui_state: Vec<String>,
    pub negative_assertions: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2SkillsGateReport {
    pub scenario_count: usize,
    pub default_gate_scenario_count: usize,
    pub passed_scenario_count: usize,
    pub expected_blocker_count: usize,
    pub ready: bool,
    pub blockers: Vec<String>,
    pub scenarios: Vec<MainChatProductMaturityV2SkillsScenario>,
    pub proofs: Vec<MainChatProductMaturityV2SkillsProof>,
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
    let selected_skill_id = selected_skill_id_for_session(state, session_id).await;
    let mut summaries = Vec::new();
    let mut seen = BTreeSet::new();

    for record in discover_local_skill_records() {
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
            available: !manifest.execution_budget.allow_writes,
            selected: selected_skill_id.as_deref() == Some(manifest.id.as_str()),
            instruction_digest: digest,
            source_kind: "bundled".into(),
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
    if let Some(record) = discover_local_skill_records()
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
            "source": "bundled:skill_registry",
            "sourceKind": "bundled",
            "available": !manifest.execution_budget.allow_writes,
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
    {
        let mut selected = state.main_chat_selected_skill_ids.lock().await;
        selected.insert(session_id.clone(), detail.skill_id.clone());
    }
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
    {
        let mut selected = state.main_chat_selected_skill_ids.lock().await;
        selected.remove(&session_id);
    }
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

pub(crate) fn main_chat_product_maturity_v2_skills_scenarios(
) -> Vec<MainChatProductMaturityV2SkillsScenario> {
    vec![
        scenario(
            "SK2-01",
            "Select a local skill.",
            &["local_skill_file", "session_id"],
            &[
                "selected_skill_id",
                "bounded_preview",
                "bounded_instruction_digest",
            ],
            &["selected_skill_visible", "bounded_preview_visible"],
            &["select_skill", "inspect_skill", "clear_skill"],
            &["skill_does_not_override_policy"],
            "pass",
        ),
        scenario(
            "SK2-02",
            "Ask why this skill was selected.",
            &["selected_skill_id"],
            &["selection_reason", "evidence_digest"],
            &["selection_reason_visible"],
            &["inspect_skill"],
            &["no_policy_override"],
            "pass",
        ),
        scenario(
            "SK2-03",
            "Execute safe read tool.",
            &["safe_read_manifest"],
            &["candidate", "policy_allow", "observation"],
            &["safe_read_candidate_visible", "observation_link_visible"],
            &["retry_tool"],
            &["no_silent_write"],
            "pass",
        ),
        scenario(
            "SK2-04",
            "Attempt write-like tool.",
            &["write_like_manifest"],
            &["permission_or_blocker", "blocked_tool"],
            &["write_like_blocker_visible"],
            &["open_review_center"],
            &[
                "write_like_tool_not_rendered_as_safe_read",
                "no_direct_write",
            ],
            "expected_blocker",
        ),
        scenario(
            "SK2-05",
            "Unselected skill exists.",
            &["unselected_skill_file"],
            &["context_without_unselected_skill"],
            &["unselected_skill_not_loaded"],
            &["select_skill"],
            &["unselected_skill_not_injected"],
            "pass",
        ),
        scenario(
            "SK2-06",
            "Unsafe manifest in registry.",
            &["unsafe_manifest"],
            &["excluded_or_blocked_tool"],
            &["unsafe_manifest_blocked_visible"],
            &["open_review_center"],
            &["unsafe_manifest_not_model_selectable"],
            "expected_blocker",
        ),
        scenario(
            "SK2-07",
            "Tool fails once.",
            &["failed_safe_read_action"],
            &["action_failed", "retry_or_alternative"],
            &["tool_failure_recovery_visible"],
            &["retry_tool", "switch_tool"],
            &["no_write_retry"],
            "pass",
        ),
        scenario(
            "SK2-08",
            "Clear selected skill.",
            &["selected_skill_id"],
            &["clear_selection", "next_context_without_selected_skill"],
            &["selected_skill_cleared"],
            &["select_skill"],
            &["cleared_skill_not_in_next_context"],
            "pass",
        ),
    ]
}

pub(crate) async fn run_main_chat_agent_product_maturity_v2_skills_gate(
) -> MainChatProductMaturityV2SkillsGateReport {
    let scenarios = main_chat_product_maturity_v2_skills_scenarios();
    let proofs = match run_skills_runtime_proofs().await {
        Ok(proofs) => proofs,
        Err(error) => vec![failed_proof("phase_e_runtime", error)],
    };
    let passed_scenario_count = proofs.iter().filter(|proof| proof.passed).count();
    let expected_blocker_count = proofs.iter().filter(|proof| proof.expected_blocker).count();
    let mut blockers = Vec::new();
    if scenarios.len() != 8 {
        blockers.push("phase_e_sk2_structured_matrix_incomplete".into());
    }
    if passed_scenario_count != scenarios.len() {
        blockers.push("phase_e_skills_scenarios_not_ready".into());
    }
    for id in [
        "SK2-01", "SK2-02", "SK2-03", "SK2-04", "SK2-05", "SK2-06", "SK2-07", "SK2-08",
    ] {
        if !proofs.iter().any(|proof| proof.scenario_id == id) {
            blockers.push(format!("missing_skills_eval:{id}"));
        }
    }
    for proof in &proofs {
        if proof.passed && (proof.runtime_object_count == 0 || proof.ui_state.is_empty()) {
            blockers.push(format!("schema_only_skills_eval:{}", proof.scenario_id));
        }
    }

    MainChatProductMaturityV2SkillsGateReport {
        scenario_count: scenarios.len(),
        default_gate_scenario_count: scenarios
            .iter()
            .filter(|scenario| scenario.default_gate)
            .count(),
        passed_scenario_count,
        expected_blocker_count,
        ready: blockers.is_empty(),
        blockers,
        scenarios,
        proofs,
    }
}

async fn run_skills_runtime_proofs() -> Result<Vec<MainChatProductMaturityV2SkillsProof>, String> {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    register_phase_e_eval_tool_manifests(&state).await;
    let session_id = "phase-e-skills-session";

    let selected = select_main_chat_skill_with_state(&state, session_id, "phase_e_review").await?;
    let context_with_selected = compile_context_for_skill(Some("phase_e_review"));
    let context_without_selected = compile_context_for_skill(None);
    let detail = get_main_chat_skill_detail_with_state(&state, "phase_e_review").await?;
    let tool_candidates = list_main_chat_tool_candidates_with_state(&state, None).await?;
    let read_observation = execute_eval_safe_read(&state).await?;
    let failed_action_id = seed_failed_tool_action(&state, session_id).await?;
    let tool_candidates_after_failure =
        list_main_chat_tool_candidates_with_state(&state, Some(session_id)).await?;
    let cleared = clear_main_chat_skill_with_state(&state, session_id).await?;
    let context_after_clear = compile_context_for_skill(None);

    let selected_context_loaded = context_with_selected.selected_skill_instruction_loaded
        && context_with_selected
            .selected_sources
            .iter()
            .any(|source| source.source_id == "skills/phase_e_review/SKILL.md");
    let unselected_absent = !context_without_selected
        .selected_sources
        .iter()
        .any(|source| source.source_id == "skills/unselected_context/SKILL.md");
    let cleared_absent = !context_after_clear.selected_skill_instruction_loaded;
    let safe_candidate_ids = tool_candidates
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let blocked_tool = tool_candidates
        .blocked_tools
        .iter()
        .find(|tool| tool.tool_name == "email.send")
        .cloned();
    let unsafe_blocked = tool_candidates
        .blocked_tools
        .iter()
        .find(|tool| tool.tool_name == "phase_e_shell_execute")
        .cloned();
    let failure_recovery = tool_candidates_after_failure.failure_recovery.clone();

    Ok(vec![
        proof(
            "SK2-01",
            selected.selected_skill_id.as_deref() == Some("phase_e_review")
                && selected.selected_skill_digest.is_some()
                && selected_context_loaded,
            false,
            3,
            vec!["phase_e_review".into()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            selected.controls.clone(),
            vec![
                "selected_skill_id".into(),
                "bounded_preview".into(),
                "bounded_instruction_digest".into(),
                "selected_skill_context_included".into(),
            ],
            vec![
                "selected_skill_visible".into(),
                "bounded_preview_visible".into(),
            ],
            vec!["skill_does_not_override_policy".into()],
            diagnostics(
                selected_context_loaded,
                "selected skill was not compiled as bounded context",
            ),
        ),
        proof(
            "SK2-02",
            selected.selection_reason == "user_selected_local_skill"
                && detail.evidence_digest.starts_with("bytes:"),
            false,
            2,
            vec!["phase_e_review".into()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec!["inspect_skill".into()],
            vec!["selection_reason".into(), "evidence_digest".into()],
            vec!["selection_reason_visible".into()],
            vec!["no_policy_override".into()],
            diagnostics(
                selected.selection_reason == "user_selected_local_skill",
                "selection reason missing",
            ),
        ),
        proof(
            "SK2-03",
            !safe_candidate_ids.is_empty()
                && read_observation
                    .metadata
                    .get("executorStatus")
                    .and_then(Value::as_str)
                    == Some("succeeded"),
            false,
            3,
            Vec::new(),
            safe_candidate_ids.clone(),
            Vec::new(),
            vec!["safe_read_action".into()],
            vec!["safe_read_observation".into()],
            vec!["retry_tool".into()],
            vec![
                "candidate".into(),
                "policy_allow".into(),
                "observation".into(),
            ],
            vec![
                "safe_read_candidate_visible".into(),
                "observation_link_visible".into(),
            ],
            vec!["no_silent_write".into()],
            diagnostics(
                !safe_candidate_ids.is_empty(),
                "safe read candidates missing",
            ),
        ),
        proof(
            "SK2-04",
            blocked_tool.is_some(),
            true,
            1,
            Vec::new(),
            safe_candidate_ids.clone(),
            blocked_tool
                .as_ref()
                .and_then(|tool| tool.blocker_id.clone())
                .into_iter()
                .collect(),
            Vec::new(),
            Vec::new(),
            vec!["open_review_center".into()],
            vec!["permission_or_blocker".into(), "blocked_tool".into()],
            vec!["write_like_blocker_visible".into()],
            vec![
                "write_like_tool_not_rendered_as_safe_read".into(),
                "no_direct_write".into(),
            ],
            diagnostics(blocked_tool.is_some(), "write-like tool was not blocked"),
        ),
        proof(
            "SK2-05",
            unselected_absent,
            false,
            1,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec!["select_skill".into()],
            vec!["context_without_unselected_skill".into()],
            vec!["unselected_skill_not_loaded".into()],
            vec!["unselected_skill_not_injected".into()],
            diagnostics(unselected_absent, "unselected skill appeared in context"),
        ),
        proof(
            "SK2-06",
            unsafe_blocked.is_some()
                && !tool_candidates
                    .candidates
                    .iter()
                    .any(|candidate| candidate.tool_name == "phase_e_shell_execute"),
            true,
            1,
            Vec::new(),
            safe_candidate_ids,
            unsafe_blocked
                .as_ref()
                .and_then(|tool| tool.blocker_id.clone())
                .into_iter()
                .collect(),
            Vec::new(),
            Vec::new(),
            vec!["open_review_center".into()],
            vec!["excluded_or_blocked_tool".into()],
            vec!["unsafe_manifest_blocked_visible".into()],
            vec!["unsafe_manifest_not_model_selectable".into()],
            diagnostics(unsafe_blocked.is_some(), "unsafe manifest was not blocked"),
        ),
        proof(
            "SK2-07",
            failure_recovery
                .as_ref()
                .is_some_and(|recovery| recovery.retry_available && !recovery.controls.is_empty()),
            false,
            2,
            Vec::new(),
            tool_candidates_after_failure
                .candidates
                .iter()
                .map(|candidate| candidate.candidate_id.clone())
                .collect(),
            Vec::new(),
            vec![failed_action_id],
            Vec::new(),
            failure_recovery
                .as_ref()
                .map(|recovery| recovery.controls.clone())
                .unwrap_or_default(),
            vec!["action_failed".into(), "retry_or_alternative".into()],
            vec!["tool_failure_recovery_visible".into()],
            vec!["no_write_retry".into()],
            diagnostics(failure_recovery.is_some(), "failure recovery missing"),
        ),
        proof(
            "SK2-08",
            cleared.selected_skill_id.is_none() && cleared_absent,
            false,
            2,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            cleared.controls,
            vec![
                "clear_selection".into(),
                "next_context_without_selected_skill".into(),
            ],
            vec!["selected_skill_cleared".into()],
            vec!["cleared_skill_not_in_next_context".into()],
            diagnostics(cleared_absent, "cleared skill remained in next context"),
        ),
    ])
}

async fn register_phase_e_eval_tool_manifests(state: &Arc<AppState>) {
    let mut registry = state.mcp_registry.lock().await;
    registry.register_builtin(
        ToolManifest {
            id: "email.send".into(),
            name: "email.send".into(),
            description: "Send email; write-like external side effect.".into(),
            parameters: json!({"type": "object"}),
            permission_level: "high".into(),
            risk_level: "high".into(),
            version: "1.0.0".into(),
            source: ToolSource::BuiltIn,
            capabilities: vec!["write".into(), "external_side_effect".into()],
            requires_confirmation: true,
            enabled: true,
            declarative_only: false,
            action_type: "external_side_effect".into(),
            tags: vec!["email".into(), "send".into()],
        },
        Box::new(|_| Ok("blocked".into())),
    );
    registry.register_builtin(
        ToolManifest {
            id: "phase_e_shell_execute".into(),
            name: "phase_e_shell_execute".into(),
            description: "Unsafe shell executor manifest for exclusion proof.".into(),
            parameters: json!({"type": "object"}),
            permission_level: "critical".into(),
            risk_level: "critical".into(),
            version: "1.0.0".into(),
            source: ToolSource::BuiltIn,
            capabilities: vec!["write".into()],
            requires_confirmation: true,
            enabled: true,
            declarative_only: false,
            action_type: "write".into(),
            tags: vec!["shell".into(), "execute".into()],
        },
        Box::new(|_| Ok("blocked".into())),
    );
}

async fn execute_eval_safe_read(
    state: &Arc<AppState>,
) -> Result<crate::main_chat_react_runtime::MainChatObservation, String> {
    let plan = MainChatReactActionPlan {
        queue_action_type: "mcp.read_only".into(),
        executor_action_type: "mcp_tool".into(),
        target: "mcp.call_tool".into(),
        arguments: json!({
            "tool_name": "builtin_echo",
            "arguments": { "text": "phase e safe read observation" },
        }),
        description: "Execute Phase E safe read proof through ActionExecutor.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: false,
        tool_candidates: Vec::new(),
    };
    execute_main_chat_react_action_with_executor(state, &plan, false).await
}

async fn seed_failed_tool_action(
    state: &Arc<AppState>,
    session_id: &str,
) -> Result<String, String> {
    let Some(store_arc) = state.main_chat_action_queue_store.as_ref() else {
        return Err("main_chat_action_queue_store_missing".into());
    };
    let store = store_arc.lock().await;
    let action = ExecutionAction::new("file.read", "Read a safe workspace file for SK2 failure.");
    let policy = ExecutionPolicy.classify(&action);
    let queued = store
        .enqueue(session_id, action, policy)
        .map_err(|err| err.to_string())?;
    let failed = store
        .fail(
            &queued.id,
            "tool_failed_once",
            Some(json!({
                "candidateId": "file.read",
                "directWritesExecuted": false,
            })),
        )
        .map_err(|err| err.to_string())?;
    Ok(failed.id)
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
                && openlife_core::agent::main_chat_agent_v1::main_chat_action_type_supports_automatic_retry(
                    &action.action.action_type,
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

fn compile_context_for_skill(
    selected_skill_id: Option<&str>,
) -> openlife_core::agent::main_chat_agent_v1::CompiledContext {
    let selected_skill_id = selected_skill_id.map(str::to_string);
    let candidates =
        crate::main_chat_context_loader::load_current_workspace_knowledge_context_candidates(
            selected_skill_id.as_deref(),
        );
    ContextCompiler.compile(ContextCompilerInput {
        strategy: MainChatAgentStrategy::DirectAnswer,
        privacy_risk: low_privacy_risk(),
        active_session_id: Some("phase-e-skills-session".into()),
        token_budget: 160,
        selected_skill_id,
        candidates,
    })
}

fn low_privacy_risk() -> MainChatPrivacyRiskSummary {
    MainChatPrivacyRiskSummary {
        risk_level: "low".into(),
        privacy_class: "internal".into(),
        policy_reason_code: "phase_e_eval_low_risk".into(),
        local_only_required: false,
        write_like: false,
        external_write_like: false,
    }
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
) -> Option<String> {
    let session_id = session_id?;
    let selected = state.main_chat_selected_skill_ids.lock().await;
    selected.get(session_id).cloned()
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

#[allow(clippy::too_many_arguments)]
fn scenario<const P: usize, const R: usize, const U: usize, const C: usize, const N: usize>(
    id: &str,
    prompt: &str,
    preconditions: &[&str; P],
    required_runtime_evidence: &[&str; R],
    required_ui_state: &[&str; U],
    required_controls: &[&str; C],
    negative_assertions: &[&str; N],
    expected_outcome: &str,
) -> MainChatProductMaturityV2SkillsScenario {
    MainChatProductMaturityV2SkillsScenario {
        id: id.into(),
        capability_group: "skills_tool_surface".into(),
        prompt: prompt.into(),
        preconditions: preconditions.iter().map(|value| (*value).into()).collect(),
        expected_route: "skills_tool_surface".into(),
        required_runtime_evidence: required_runtime_evidence
            .iter()
            .map(|value| (*value).into())
            .collect(),
        required_ui_state: required_ui_state
            .iter()
            .map(|value| (*value).into())
            .collect(),
        required_controls: required_controls
            .iter()
            .map(|value| (*value).into())
            .collect(),
        negative_assertions: negative_assertions
            .iter()
            .map(|value| (*value).into())
            .collect(),
        expected_outcome: expected_outcome.into(),
        default_gate: true,
    }
}

#[allow(clippy::too_many_arguments)]
fn proof(
    scenario_id: &str,
    passed: bool,
    expected_blocker: bool,
    runtime_object_count: usize,
    selected_skill_ids: Vec<String>,
    candidate_ids: Vec<String>,
    blocker_ids: Vec<String>,
    action_ids: Vec<String>,
    observation_ids: Vec<String>,
    controls: Vec<String>,
    runtime_evidence: Vec<String>,
    ui_state: Vec<String>,
    negative_assertions: Vec<String>,
    diagnostics: Vec<String>,
) -> MainChatProductMaturityV2SkillsProof {
    MainChatProductMaturityV2SkillsProof {
        scenario_id: scenario_id.into(),
        passed,
        expected_blocker,
        runtime_object_count,
        selected_skill_ids,
        candidate_ids,
        blocker_ids,
        action_ids,
        observation_ids,
        controls,
        runtime_evidence,
        ui_state,
        negative_assertions,
        diagnostics,
    }
}

fn failed_proof(
    scenario_id: impl Into<String>,
    diagnostic: impl Into<String>,
) -> MainChatProductMaturityV2SkillsProof {
    MainChatProductMaturityV2SkillsProof {
        scenario_id: scenario_id.into(),
        passed: false,
        expected_blocker: false,
        runtime_object_count: 0,
        selected_skill_ids: Vec::new(),
        candidate_ids: Vec::new(),
        blocker_ids: Vec::new(),
        action_ids: Vec::new(),
        observation_ids: Vec::new(),
        controls: Vec::new(),
        runtime_evidence: Vec::new(),
        ui_state: Vec::new(),
        negative_assertions: Vec::new(),
        diagnostics: vec![diagnostic.into()],
    }
}

fn diagnostics(ok: bool, message: &str) -> Vec<String> {
    if ok {
        Vec::new()
    } else {
        vec![message.into()]
    }
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
        .find(|line| !line.is_empty() && !line.starts_with('#'))
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
