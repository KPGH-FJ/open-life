use std::sync::Arc;

use openlife_core::agent::{ProposalStatus, ProposalType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::AppState;

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

#[tauri::command]
pub async fn draft_edit_memory_proposal(
    proposal_id: String,
    new_after: Value,
    state: State<'_, Arc<AppState>>,
) -> Result<MemoryProposalDraftEditReport, String> {
    draft_edit_memory_proposal_with_state(proposal_id, new_after, state.inner()).await
}

fn digest_value(value: &Value) -> String {
    let json = serde_json::to_string(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
