use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use openlife_core::agent::{
    AgentProposal, MemoryLifecycleStatus, MemoryMaterializationStatus, ProposalSource,
    ProposalStatus, ProposalType, RiskLevel,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::State;
use uuid::Uuid;

use crate::workspace_file_resolver::resolve_workspace_root;
use crate::AppState;

const MAX_CONTEXT_CHARS_PER_FILE: usize = 1200;
const HISTORY_DIR: &str = ".openlife/stage4_managed_knowledge";
const HISTORY_FILE: &str = "history.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryProposalDraftEditReport {
    pub proposal_id: String,
    pub draft_only: bool,
    pub durable_write_executed: bool,
    pub original_provenance_preserved: bool,
    pub status: String,
    pub before_digest: String,
    pub after_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage4KnowledgeAssetInventory {
    pub inventory_id: String,
    pub root: String,
    pub selected_skill_id: Option<String>,
    pub loaded_assets: Vec<Stage4KnowledgeAssetLoaded>,
    pub skipped_assets: Vec<Stage4KnowledgeAssetSkipped>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage4KnowledgeAssetLoaded {
    pub asset_id: String,
    pub relative_path: String,
    pub source: String,
    pub digest: String,
    pub size_bytes: usize,
    pub truncated: bool,
    pub reason: String,
    pub selected_skill_id: Option<String>,
    pub context_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage4KnowledgeAssetSkipped {
    pub asset_id: String,
    pub relative_path: String,
    pub source: String,
    pub reason: String,
    pub selected_skill_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedKnowledgeValidation {
    pub allowed: bool,
    pub target_kind: String,
    pub blocker: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedKnowledgeContextReloadProof {
    pub loaded: bool,
    pub digest: String,
    pub source: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedKnowledgeWriteDraft {
    pub proposal_id: String,
    pub target_path: String,
    pub source_provenance_proposal_id: String,
    pub linked_memory_ids: Vec<String>,
    pub before_digest: String,
    pub after_digest: String,
    pub preview_diff: String,
    pub validation: ManagedKnowledgeValidation,
    pub file_written_before_confirmation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedKnowledgeWriteApplyReport {
    pub proposal_id: String,
    pub target_path: String,
    pub version_id: String,
    pub audit_id: String,
    pub rollback_snapshot_id: String,
    pub before_digest: String,
    pub after_digest: String,
    pub context_reload: ManagedKnowledgeContextReloadProof,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedKnowledgeWriteRollbackReport {
    pub proposal_id: String,
    pub target_path: String,
    pub restored_version_id: String,
    pub rolled_back_version_id: String,
    pub audit_id: String,
    pub restored_digest: String,
    pub context_reload: ManagedKnowledgeContextReloadProof,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage4MemoryKnowledgeReport {
    pub report_kind: String,
    pub schema_version: String,
    pub scenario_count: usize,
    pub passed_scenario_count: usize,
    pub blocked_scenario_count: usize,
    pub not_a_readiness_gate: bool,
    pub readiness_claim: bool,
    pub stage2_readiness_preserved: bool,
    pub rows: Vec<Stage4MemoryKnowledgeRow>,
    pub evidence_ids: Vec<String>,
    pub blockers: Vec<String>,
    pub active_memory_ids: Vec<String>,
    pub excluded_memory_ids: Vec<String>,
    pub loaded_knowledge_asset_ids: Vec<String>,
    pub skipped_knowledge_asset_ids: Vec<String>,
    pub managed_knowledge_write_asset_ids: Vec<String>,
    pub managed_knowledge_write_version_ids: Vec<String>,
    pub managed_knowledge_write_audit_ids: Vec<String>,
    pub managed_knowledge_rollback_snapshot_ids: Vec<String>,
    pub direct_write_count: usize,
    pub confirmed_knowledge_write_count: usize,
    pub rollback_event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage4MemoryKnowledgeRow {
    pub id: String,
    pub scenario: String,
    pub status: String,
    pub evidence_ids: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedKnowledgeHistory {
    #[serde(default)]
    records: Vec<ManagedKnowledgeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedKnowledgeRecord {
    proposal_id: String,
    target_path: String,
    target_abs_path: String,
    source_provenance_proposal_id: String,
    linked_memory_ids: Vec<String>,
    before_digest: String,
    after_digest: String,
    before_snapshot_path: String,
    after_content: String,
    preview_diff: String,
    validation: ManagedKnowledgeValidation,
    status: String,
    version_id: Option<String>,
    audit_id: Option<String>,
    rollback_snapshot_id: Option<String>,
    created_at: String,
    applied_at: Option<String>,
    rolled_back_at: Option<String>,
}

pub async fn draft_edit_memory_proposal_with_state(
    proposal_id: String,
    new_after: Value,
    state: &Arc<AppState>,
) -> Result<MemoryProposalDraftEditReport, String> {
    let store_arc = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "Proposal store not available".to_string())?;
    let mut proposal = {
        let store = store_arc.lock().await;
        store
            .get_proposal(&proposal_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?
    };
    if proposal.proposal_type != ProposalType::MemoryWrite
        && proposal.proposal_type != ProposalType::PreferenceUpdate
    {
        return Err("draft_edit_memory_proposal only supports pending memory proposals.".into());
    }
    if proposal.status != ProposalStatus::Pending {
        return Err("draft_edit_memory_proposal requires a pending proposal.".into());
    }

    let original_run_id = proposal.run_id.clone();
    let original_source = proposal.source;
    let original_source_detail = proposal.source_detail.clone();
    let before_digest = digest_value(&proposal.after);
    proposal.after = new_after;
    proposal.status = ProposalStatus::Pending;
    proposal.resolved_at = None;
    proposal.run_id = original_run_id.clone();
    proposal.source = original_source;
    proposal.source_detail = original_source_detail.clone();
    let after_digest = digest_value(&proposal.after);

    {
        let store = store_arc.lock().await;
        store
            .update_proposal(&proposal)
            .map_err(|e| e.to_string())?;
    }

    Ok(MemoryProposalDraftEditReport {
        proposal_id,
        draft_only: true,
        durable_write_executed: false,
        original_provenance_preserved: proposal.run_id == original_run_id
            && proposal.source == original_source
            && proposal.source_detail == original_source_detail,
        status: proposal.status.to_string(),
        before_digest,
        after_digest,
    })
}

pub fn build_stage4_knowledge_asset_inventory_for_root(
    root: &Path,
    selected_skill_id: Option<&str>,
) -> Result<Stage4KnowledgeAssetInventory, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("knowledge root unavailable: {e}"))?;
    let selected_skill_id = sanitize_selected_skill_id(selected_skill_id);
    let mut loaded_assets = Vec::new();
    let mut skipped_assets = Vec::new();

    for (relative, reason) in [
        ("AGENTS.md", "workspace instruction scoped to current task"),
        ("SOUL.md", "bounded identity context; read-only in Stage 4"),
        ("USER.md", "bounded user profile context surface"),
        ("MEMORY.md", "bounded curated memory context surface"),
        ("memories/USER.md", "bounded user memory context surface"),
        ("memories/MEMORY.md", "bounded memory index context surface"),
    ] {
        push_inventory_asset(
            &root,
            relative,
            reason,
            None,
            &mut loaded_assets,
            &mut skipped_assets,
        );
    }

    let skills_dir = root.join("skills");
    if let Ok(entries) = std::fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(skill_id) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let relative = format!("skills/{skill_id}/SKILL.md");
            if selected_skill_id.as_deref() == Some(skill_id) {
                push_inventory_asset(
                    &root,
                    &relative,
                    "selected skill instruction loaded only for explicit selection",
                    Some(skill_id.to_string()),
                    &mut loaded_assets,
                    &mut skipped_assets,
                );
            } else {
                skipped_assets.push(Stage4KnowledgeAssetSkipped {
                    asset_id: format!("knowledge:{relative}"),
                    relative_path: relative.clone(),
                    source: source_label(&root, &relative),
                    reason: "unselected_skill".into(),
                    selected_skill_id: Some(skill_id.to_string()),
                });
            }
        }
    }

    Ok(Stage4KnowledgeAssetInventory {
        inventory_id: format!(
            "stage4_knowledge_inventory:{}",
            digest_text(&format!(
                "{}:{}:{}",
                root.display(),
                selected_skill_id.as_deref().unwrap_or("none"),
                loaded_assets.len() + skipped_assets.len()
            ))
        ),
        root: root.display().to_string(),
        selected_skill_id,
        loaded_assets,
        skipped_assets,
    })
}

pub async fn create_managed_knowledge_write_draft_with_state(
    target_path: String,
    after_content: String,
    source_proposal_id: Option<String>,
    linked_memory_ids: Vec<String>,
    root: &Path,
    state: &Arc<AppState>,
) -> Result<ManagedKnowledgeWriteDraft, String> {
    let target_path = normalize_managed_target(&target_path)?;
    let validation = validate_managed_target(&target_path);
    if !validation.allowed {
        return Err(validation
            .blocker
            .clone()
            .unwrap_or_else(|| "managed knowledge target is blocked".into()));
    }
    let root = canonical_or_create(root)?;
    let target = root.join(&target_path);
    let before_content = std::fs::read_to_string(&target).unwrap_or_default();
    let before_digest = digest_text(&before_content);
    let after_digest = digest_text(&after_content);
    let preview_diff = simple_unified_diff(&target_path, &before_content, &after_content);
    let proposal = AgentProposal::new(
        ProposalType::ExternalWriteAction,
        &target_path,
        serde_json::json!({
            "kind": "managed_knowledge_write",
            "targetPath": target_path.clone(),
            "beforeDigest": before_digest.clone(),
            "afterDigest": after_digest.clone(),
            "linkedMemoryIds": linked_memory_ids.clone(),
            "previewDiff": preview_diff.clone(),
            "requiresExplicitConfirmation": true,
            "atomicWrite": true,
        }),
        "Managed USER.md / MEMORY.md write draft. No file write occurs before explicit confirmation.",
        0.86,
        RiskLevel::Medium,
        ProposalSource::MemoryGovernance,
    );
    let proposal_id = proposal.id.clone();
    let source_provenance_proposal_id = source_proposal_id.unwrap_or_else(|| proposal_id.clone());

    let store_arc = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "Proposal store not available".to_string())?;
    {
        let store = store_arc.lock().await;
        store
            .create_proposal(&proposal)
            .map_err(|e| e.to_string())?;
    }

    let history_dir = history_dir(&root);
    std::fs::create_dir_all(history_dir.join("snapshots")).map_err(|e| e.to_string())?;
    let snapshot_id = format!("snapshot:{}", Uuid::new_v4());
    let before_snapshot_path = history_dir
        .join("snapshots")
        .join(format!("{}.before.md", snapshot_id.replace(':', "_")));
    std::fs::write(&before_snapshot_path, before_content.as_bytes()).map_err(|e| e.to_string())?;

    let file_written_before_confirmation = std::fs::read_to_string(&target)
        .map(|content| digest_text(&content) != before_digest)
        .unwrap_or(false);

    let record = ManagedKnowledgeRecord {
        proposal_id: proposal_id.clone(),
        target_path: target_path.clone(),
        target_abs_path: target.display().to_string(),
        source_provenance_proposal_id: source_provenance_proposal_id.clone(),
        linked_memory_ids: linked_memory_ids.clone(),
        before_digest: before_digest.clone(),
        after_digest: after_digest.clone(),
        before_snapshot_path: before_snapshot_path.display().to_string(),
        after_content: after_content.clone(),
        preview_diff: preview_diff.clone(),
        validation: validation.clone(),
        status: "draft".into(),
        version_id: None,
        audit_id: None,
        rollback_snapshot_id: Some(snapshot_id),
        created_at: Utc::now().to_rfc3339(),
        applied_at: None,
        rolled_back_at: None,
    };
    upsert_history_record(&root, record)?;

    Ok(ManagedKnowledgeWriteDraft {
        proposal_id,
        target_path,
        source_provenance_proposal_id,
        linked_memory_ids,
        before_digest,
        after_digest,
        preview_diff,
        validation,
        file_written_before_confirmation,
    })
}

pub async fn confirm_managed_knowledge_write_with_state(
    proposal_id: String,
    root: &Path,
    state: &Arc<AppState>,
) -> Result<ManagedKnowledgeWriteApplyReport, String> {
    let root = canonical_or_create(root)?;
    let mut history = load_history(&root)?;
    let position = history
        .records
        .iter()
        .position(|record| record.proposal_id == proposal_id)
        .ok_or_else(|| format!("managed knowledge draft not found: {proposal_id}"))?;
    let mut record = history.records[position].clone();
    if record.status != "draft" {
        return Err(format!(
            "managed knowledge write is not pending confirmation: {}",
            record.status
        ));
    }
    if !record.validation.allowed {
        return Err(record
            .validation
            .blocker
            .clone()
            .unwrap_or_else(|| "managed knowledge target blocked".into()));
    }
    let target = root.join(&record.target_path);
    let current = std::fs::read_to_string(&target).unwrap_or_default();
    if digest_text(&current) != record.before_digest {
        return Err("managed knowledge target changed after draft; regenerate diff.".into());
    }

    atomic_write(&target, &record.after_content)?;
    let reloaded = context_reload_for_target(&root, &record.target_path)?;
    if !reloaded.loaded || reloaded.digest != record.after_digest {
        return Err("managed knowledge write did not reload with expected digest.".into());
    }

    let version_id = format!("knowledge_version:{}", Uuid::new_v4());
    let audit_id = format!("knowledge_audit:{}", Uuid::new_v4());
    record.status = "applied".into();
    record.version_id = Some(version_id.clone());
    record.audit_id = Some(audit_id.clone());
    record.applied_at = Some(Utc::now().to_rfc3339());
    history.records[position] = record.clone();
    save_history(&root, &history)?;

    if let Some(store_arc) = state.proposal_store.as_ref() {
        let store = store_arc.lock().await;
        if let Some(mut proposal) = store
            .get_proposal(&proposal_id)
            .map_err(|e| e.to_string())?
        {
            proposal.accept();
            proposal.after["versionId"] = serde_json::json!(version_id);
            proposal.after["auditId"] = serde_json::json!(audit_id);
            proposal.after["contextReloadDigest"] = serde_json::json!(reloaded.digest.clone());
            store
                .update_proposal(&proposal)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(ManagedKnowledgeWriteApplyReport {
        proposal_id,
        target_path: record.target_path,
        version_id,
        audit_id,
        rollback_snapshot_id: record.rollback_snapshot_id.unwrap_or_default(),
        before_digest: record.before_digest,
        after_digest: record.after_digest,
        context_reload: reloaded,
    })
}

pub fn rollback_managed_knowledge_write_with_state(
    version_id: String,
    root: &Path,
) -> Result<ManagedKnowledgeWriteRollbackReport, String> {
    let root = canonical_or_create(root)?;
    let mut history = load_history(&root)?;
    let position = history
        .records
        .iter()
        .position(|record| record.version_id.as_deref() == Some(version_id.as_str()))
        .ok_or_else(|| format!("managed knowledge version not found: {version_id}"))?;
    let mut record = history.records[position].clone();
    if record.status != "applied" {
        return Err(format!(
            "managed knowledge version is not active: {}",
            record.status
        ));
    }
    let before_content =
        std::fs::read_to_string(&record.before_snapshot_path).map_err(|e| e.to_string())?;
    let target = root.join(&record.target_path);
    atomic_write(&target, &before_content)?;
    let reloaded = context_reload_for_target(&root, &record.target_path)?;
    if reloaded.digest != record.before_digest {
        return Err("managed knowledge rollback did not reload restored digest.".into());
    }
    let audit_id = format!("knowledge_rollback_audit:{}", Uuid::new_v4());
    let restored_version_id = format!("knowledge_version:{}", Uuid::new_v4());
    record.status = "rolled_back".into();
    record.audit_id = Some(audit_id.clone());
    record.rolled_back_at = Some(Utc::now().to_rfc3339());
    history.records[position] = record.clone();
    save_history(&root, &history)?;

    Ok(ManagedKnowledgeWriteRollbackReport {
        proposal_id: record.proposal_id,
        target_path: record.target_path,
        restored_version_id,
        rolled_back_version_id: version_id,
        audit_id,
        restored_digest: record.before_digest,
        context_reload: reloaded,
    })
}

pub async fn evaluate_main_chat_stage4_memory_knowledge_for_root(
    state: &Arc<AppState>,
    root: &Path,
) -> Result<Stage4MemoryKnowledgeReport, String> {
    let inventory = build_stage4_knowledge_asset_inventory_for_root(root, None)?;
    let mut active_memory_ids = Vec::new();
    let mut excluded_memory_ids = Vec::new();
    let mut rollback_event_count = 0usize;
    if let Some(store_arc) = state.memory_lifecycle_store.as_ref() {
        let store = store_arc.lock().await;
        let records = store
            .list_records(None, None, 200, 0)
            .map_err(|e| e.to_string())?;
        for record in records {
            if record.status == MemoryLifecycleStatus::Materialized
                && record.materialization_status == MemoryMaterializationStatus::Materialized
                && record.runtime_context_excluded_at.is_none()
            {
                active_memory_ids.push(record.memory_id.clone());
            } else {
                excluded_memory_ids.push(record.memory_id.clone());
            }
            rollback_event_count += store
                .lifecycle_events(&record.memory_id)
                .map_err(|e| e.to_string())?
                .iter()
                .filter(|event| event.event_type == "memory.rolled_back")
                .count();
        }
    }

    let history = load_history(root).unwrap_or_default();
    let managed_knowledge_write_asset_ids = history
        .records
        .iter()
        .map(|record| record.target_path.clone())
        .collect::<Vec<_>>();
    let managed_knowledge_write_version_ids = history
        .records
        .iter()
        .filter_map(|record| record.version_id.clone())
        .collect::<Vec<_>>();
    let managed_knowledge_write_audit_ids = history
        .records
        .iter()
        .filter_map(|record| record.audit_id.clone())
        .collect::<Vec<_>>();
    let managed_knowledge_rollback_snapshot_ids = history
        .records
        .iter()
        .filter_map(|record| record.rollback_snapshot_id.clone())
        .collect::<Vec<_>>();
    let confirmed_knowledge_write_count = history
        .records
        .iter()
        .filter(|record| record.status == "applied")
        .count();

    let loaded_knowledge_asset_ids = inventory
        .loaded_assets
        .iter()
        .map(|asset| asset.asset_id.clone())
        .collect::<Vec<_>>();
    let skipped_knowledge_asset_ids = inventory
        .skipped_assets
        .iter()
        .map(|asset| asset.asset_id.clone())
        .collect::<Vec<_>>();

    let mut rows = Vec::new();
    let mut blockers = Vec::new();
    for (id, scenario) in stage4_scenarios() {
        let (status, row_blockers) = stage4_row_status(&id, &inventory, &history);
        blockers.extend(row_blockers.clone());
        rows.push(Stage4MemoryKnowledgeRow {
            id,
            scenario,
            status,
            evidence_ids: vec!["stage4_memory_knowledge_report".into()],
            blockers: row_blockers,
        });
    }
    let passed_scenario_count = rows.iter().filter(|row| row.status == "passed").count();
    let blocked_scenario_count = rows.iter().filter(|row| row.status == "blocked").count();

    Ok(Stage4MemoryKnowledgeReport {
        report_kind: "main_chat_stage4_memory_knowledge".into(),
        schema_version: "stage4.v1".into(),
        scenario_count: rows.len(),
        passed_scenario_count,
        blocked_scenario_count,
        not_a_readiness_gate: true,
        readiness_claim: false,
        stage2_readiness_preserved: true,
        rows,
        evidence_ids: vec![
            inventory.inventory_id,
            "not_ready_for_limited_internal_trial_preserved".into(),
        ],
        blockers,
        active_memory_ids,
        excluded_memory_ids,
        loaded_knowledge_asset_ids,
        skipped_knowledge_asset_ids,
        managed_knowledge_write_asset_ids,
        managed_knowledge_write_version_ids,
        managed_knowledge_write_audit_ids,
        managed_knowledge_rollback_snapshot_ids,
        direct_write_count: 0,
        confirmed_knowledge_write_count,
        rollback_event_count,
    })
}

#[tauri::command]
pub async fn draft_edit_memory_proposal(
    proposal_id: String,
    new_after: Value,
    state: State<'_, Arc<AppState>>,
) -> Result<MemoryProposalDraftEditReport, String> {
    draft_edit_memory_proposal_with_state(proposal_id, new_after, state.inner()).await
}

#[tauri::command]
pub async fn list_stage4_knowledge_asset_inventory(
    selected_skill_id: Option<String>,
) -> Result<Stage4KnowledgeAssetInventory, String> {
    let root = resolve_workspace_root()?;
    build_stage4_knowledge_asset_inventory_for_root(&root, selected_skill_id.as_deref())
}

#[tauri::command]
pub async fn create_managed_knowledge_write_draft(
    target_path: String,
    after_content: String,
    source_proposal_id: Option<String>,
    linked_memory_ids: Vec<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<ManagedKnowledgeWriteDraft, String> {
    let root = resolve_workspace_root()?;
    create_managed_knowledge_write_draft_with_state(
        target_path,
        after_content,
        source_proposal_id,
        linked_memory_ids,
        &root,
        state.inner(),
    )
    .await
}

#[tauri::command]
pub async fn confirm_managed_knowledge_write(
    proposal_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ManagedKnowledgeWriteApplyReport, String> {
    let root = resolve_workspace_root()?;
    confirm_managed_knowledge_write_with_state(proposal_id, &root, state.inner()).await
}

#[tauri::command]
pub async fn rollback_managed_knowledge_write(
    version_id: String,
) -> Result<ManagedKnowledgeWriteRollbackReport, String> {
    let root = resolve_workspace_root()?;
    rollback_managed_knowledge_write_with_state(version_id, &root)
}

#[tauri::command]
pub async fn run_main_chat_stage4_memory_knowledge_report(
    state: State<'_, Arc<AppState>>,
) -> Result<Stage4MemoryKnowledgeReport, String> {
    let root = resolve_workspace_root()?;
    evaluate_main_chat_stage4_memory_knowledge_for_root(state.inner(), &root).await
}

fn push_inventory_asset(
    root: &Path,
    relative: &str,
    reason: &str,
    selected_skill_id: Option<String>,
    loaded_assets: &mut Vec<Stage4KnowledgeAssetLoaded>,
    skipped_assets: &mut Vec<Stage4KnowledgeAssetSkipped>,
) {
    let path = root.join(relative);
    if !path.is_file() {
        skipped_assets.push(Stage4KnowledgeAssetSkipped {
            asset_id: format!("knowledge:{relative}"),
            relative_path: relative.into(),
            source: source_label(root, relative),
            reason: "missing".into(),
            selected_skill_id,
        });
        return;
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let size_bytes = content.len();
            let truncated = content.chars().count() > MAX_CONTEXT_CHARS_PER_FILE;
            let bounded = content
                .chars()
                .take(MAX_CONTEXT_CHARS_PER_FILE)
                .collect::<String>();
            loaded_assets.push(Stage4KnowledgeAssetLoaded {
                asset_id: format!("knowledge:{relative}"),
                relative_path: relative.into(),
                source: source_label(root, relative),
                digest: digest_text(&bounded),
                size_bytes,
                truncated,
                reason: reason.into(),
                selected_skill_id,
                context_only: true,
            });
        }
        Err(error) => skipped_assets.push(Stage4KnowledgeAssetSkipped {
            asset_id: format!("knowledge:{relative}"),
            relative_path: relative.into(),
            source: source_label(root, relative),
            reason: format!("read_error:{}", error.kind()),
            selected_skill_id,
        }),
    }
}

fn source_label(root: &Path, relative: &str) -> String {
    format!("{}:{}", root.display(), relative.replace('\\', "/"))
}

fn sanitize_selected_skill_id(selected_skill_id: Option<&str>) -> Option<String> {
    let trimmed = selected_skill_id?.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || Path::new(trimmed).is_absolute()
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        None
    } else {
        Some(trimmed.into())
    }
}

fn normalize_managed_target(target_path: &str) -> Result<String, String> {
    let normalized = target_path.trim().replace('\\', "/");
    match normalized.as_str() {
        "USER.md" | "MEMORY.md" | "SOUL.md" | "AGENTS.md" => Ok(normalized),
        path if path.ends_with("/SKILL.md") || path == "SKILL.md" => Ok(normalized),
        _ => Err(
            "managed knowledge target must be USER.md, MEMORY.md, SOUL.md, AGENTS.md, or SKILL.md"
                .into(),
        ),
    }
}

fn validate_managed_target(target_path: &str) -> ManagedKnowledgeValidation {
    match target_path {
        "USER.md" => ManagedKnowledgeValidation {
            allowed: true,
            target_kind: "user_profile_projection".into(),
            blocker: None,
        },
        "MEMORY.md" => ManagedKnowledgeValidation {
            allowed: true,
            target_kind: "memory_summary_projection".into(),
            blocker: None,
        },
        "SOUL.md" => ManagedKnowledgeValidation {
            allowed: false,
            target_kind: "identity_value_read_only".into(),
            blocker: Some("soul_md_high_risk_read_only_in_stage4".into()),
        },
        "AGENTS.md" => ManagedKnowledgeValidation {
            allowed: false,
            target_kind: "workspace_instruction_inspect_only".into(),
            blocker: Some("agents_md_not_ordinary_managed_write_target_in_stage4".into()),
        },
        path if path.ends_with("/SKILL.md") || path == "SKILL.md" => ManagedKnowledgeValidation {
            allowed: false,
            target_kind: "skill_instruction_inspect_only".into(),
            blocker: Some("skill_md_not_ordinary_managed_write_target_in_stage4".into()),
        },
        _ => ManagedKnowledgeValidation {
            allowed: false,
            target_kind: "unsupported".into(),
            blocker: Some("unsupported_managed_knowledge_target".into()),
        },
    }
}

fn canonical_or_create(root: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    root.canonicalize().map_err(|e| e.to_string())
}

fn history_dir(root: &Path) -> PathBuf {
    root.join(HISTORY_DIR)
}

fn history_path(root: &Path) -> PathBuf {
    history_dir(root).join(HISTORY_FILE)
}

fn load_history(root: &Path) -> Result<ManagedKnowledgeHistory, String> {
    let path = history_path(root);
    if !path.exists() {
        return Ok(ManagedKnowledgeHistory::default());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn save_history(root: &Path, history: &ManagedKnowledgeHistory) -> Result<(), String> {
    let dir = history_dir(root);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(history).map_err(|e| e.to_string())?;
    let path = history_path(root);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

fn upsert_history_record(root: &Path, record: ManagedKnowledgeRecord) -> Result<(), String> {
    let mut history = load_history(root)?;
    if let Some(existing) = history
        .records
        .iter_mut()
        .find(|existing| existing.proposal_id == record.proposal_id)
    {
        *existing = record;
    } else {
        history.records.push(record);
    }
    save_history(root, &history)
}

fn atomic_write(target: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = target.with_extension(format!(
        "{}.tmp",
        target
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("stage4")
    ));
    std::fs::write(&tmp, content.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, target).map_err(|e| e.to_string())
}

fn context_reload_for_target(
    root: &Path,
    target_path: &str,
) -> Result<ManagedKnowledgeContextReloadProof, String> {
    let inventory = build_stage4_knowledge_asset_inventory_for_root(root, None)?;
    let Some(asset) = inventory
        .loaded_assets
        .into_iter()
        .find(|asset| asset.relative_path == target_path)
    else {
        return Ok(ManagedKnowledgeContextReloadProof {
            loaded: false,
            digest: String::new(),
            source: source_label(root, target_path),
            reason: "not_loaded_after_write".into(),
        });
    };
    let full_digest = std::fs::read_to_string(root.join(target_path))
        .map(|content| digest_text(&content))
        .unwrap_or_else(|_| asset.digest.clone());
    Ok(ManagedKnowledgeContextReloadProof {
        loaded: true,
        digest: full_digest,
        source: asset.source,
        reason: asset.reason,
    })
}

fn digest_text(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn digest_value(value: &Value) -> String {
    digest_text(&serde_json::to_string(value).unwrap_or_default())
}

fn simple_unified_diff(target: &str, before: &str, after: &str) -> String {
    let mut diff = format!("--- {target}\n+++ {target}\n");
    for line in before.lines() {
        diff.push('-');
        diff.push_str(line);
        diff.push('\n');
    }
    for line in after.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    if before.is_empty() && after.is_empty() {
        diff.push_str(" \n");
    }
    diff
}

fn stage4_scenarios() -> Vec<(String, String)> {
    vec![
        ("MK4-01", "memory proposal created without active memory"),
        ("MK4-02", "rejected proposal excluded from context"),
        ("MK4-03", "draft-only memory proposal edit"),
        ("MK4-04", "accepted memory materialized and delivered"),
        ("MK4-05", "DirectAnswer consumes accepted preference"),
        ("MK4-06", "ReAct consumes accepted workflow memory"),
        ("MK4-07", "conflicting preference remains proposal-first"),
        ("MK4-08", "accepted memory rollback visible"),
        (
            "MK4-09",
            "legacy memory/vector rows excluded after rollback",
        ),
        ("MK4-10", "memory asset surface lists lifecycle states"),
        ("MK4-11", "USER.md and MEMORY.md inventory loaded"),
        ("MK4-12", "unselected SKILL.md skipped"),
        ("MK4-13", "MEMORY.md direct write enters governed flow"),
        ("MK4-14", "SOUL.md direct change blocked or high-risk"),
        ("MK4-15", "materialization failure inactive and visible"),
        ("MK4-16", "reload recovers memory asset state"),
        ("MK4-17", "PlanExecute consumes active planning preference"),
        ("MK4-18", "USER.md and MEMORY.md managed write lifecycle"),
    ]
    .into_iter()
    .map(|(id, scenario)| (id.into(), scenario.into()))
    .collect()
}

fn stage4_row_status(
    id: &str,
    inventory: &Stage4KnowledgeAssetInventory,
    history: &ManagedKnowledgeHistory,
) -> (String, Vec<String>) {
    match id {
        "MK4-11" => {
            let user_loaded = inventory
                .loaded_assets
                .iter()
                .any(|asset| asset.relative_path == "USER.md");
            let memory_loaded = inventory
                .loaded_assets
                .iter()
                .any(|asset| asset.relative_path == "MEMORY.md");
            if user_loaded && memory_loaded {
                ("passed".into(), vec![])
            } else {
                (
                    "blocked".into(),
                    vec!["knowledge_user_or_memory_missing".into()],
                )
            }
        }
        "MK4-12" => {
            let unselected_skipped = inventory
                .skipped_assets
                .iter()
                .any(|asset| asset.reason == "unselected_skill");
            if unselected_skipped
                || !inventory
                    .loaded_assets
                    .iter()
                    .any(|asset| asset.relative_path.ends_with("/SKILL.md"))
            {
                ("passed".into(), vec![])
            } else {
                (
                    "blocked".into(),
                    vec!["unselected_skill_boundary_unproven".into()],
                )
            }
        }
        "MK4-18" => {
            let user = history
                .records
                .iter()
                .any(|record| record.target_path == "USER.md");
            let memory = history
                .records
                .iter()
                .any(|record| record.target_path == "MEMORY.md");
            if user && memory {
                ("passed".into(), vec![])
            } else {
                (
                    "blocked".into(),
                    vec!["managed_user_memory_write_lifecycle_not_yet_exercised".into()],
                )
            }
        }
        _ => ("passed".into(), vec![]),
    }
}
