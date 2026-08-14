use crate::state::AppState;
use openlife_core::agent::{
    build_review_center_view_model, AgentProposal, MemoryLifecycleStatus,
    MemoryMaterializationStatus, ProposalStatus, ProposalType, ReviewCenterBuildInput,
    ReviewCenterViewModel, ReviewItemArtifactEvidence, ReviewItemMaterializationStatus,
    ViewModelEnvelope, ViewModelStatus, ViewModelWarning, ViewModelWarningSeverity,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_review_center_view_model(
    state: State<'_, Arc<AppState>>,
) -> Result<ViewModelEnvelope<ReviewCenterViewModel>, String> {
    get_review_center_view_model_with_state(state.inner()).await
}

pub(crate) async fn get_review_center_view_model_with_state(
    state: &Arc<AppState>,
) -> Result<ViewModelEnvelope<ReviewCenterViewModel>, String> {
    let Some(proposal_store) = state.proposal_store.as_ref() else {
        let mut envelope = ViewModelEnvelope::backend_read_model(ViewModelStatus::Error, None);
        envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
        envelope.warnings.push(warning(
            "proposal_store_unavailable",
            "Proposal store is unavailable; ReviewCenterViewModel cannot determine action eligibility.",
        ));
        return Ok(envelope);
    };

    let proposal_store = proposal_store.lock().await;
    let proposals = proposal_store
        .list_all_proposals(100, 0)
        .map_err(|err| format!("failed to load review proposals: {err}"))?;
    let terminal_owner_task_session_ids = proposals
        .iter()
        .filter_map(|proposal| {
            proposal_store
                .terminal_owner_origin_binding(&proposal.id)
                .transpose()
                .map(|result| {
                    result
                        .map(|binding| (proposal.id.clone(), binding.task_session_id().to_string()))
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(|err| format!("failed to load canonical review origins: {err}"))?;
    let artifact_evidence = proposals
        .iter()
        .map(|proposal| {
            proposal_store.artifact_effect(&proposal.id).map(|record| {
                record.map(|record| {
                    (
                        proposal.id.clone(),
                        ReviewItemArtifactEvidence {
                            state: record.state.as_str().into(),
                            target_reference_digest: record.target_reference_digest,
                            content_digest: record.content_digest,
                            observed_content_digest: record.observed_content_digest,
                            byte_size: record.byte_size,
                            media_type: record.media_type,
                            error_code: record.error_code,
                        },
                    )
                })
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to load artifact materialization evidence: {err}"))?
        .into_iter()
        .flatten()
        .collect::<BTreeMap<_, _>>();
    let (dispatch_materialization_overrides, mut dispatch_warnings) =
        dispatch_materialization_overrides(&proposal_store, &proposals);
    drop(proposal_store);
    let config = state.config.lock().await;
    let safe_paths = config.system.safe_paths.clone();
    drop(config);

    let (safe_path_overrides, mut safe_path_warnings) =
        proposal_safe_path_overrides(state, &proposals).await;

    let (mut materialization_overrides, mut warnings) =
        memory_materialization_overrides(state, &proposals).await;
    materialization_overrides.extend(dispatch_materialization_overrides);
    warnings.append(&mut dispatch_warnings);
    warnings.append(&mut safe_path_warnings);
    // Review availability is owned by the exact Proposal/Artifact capability
    // being reviewed. Unrelated startup warnings (including retired execution
    // stores) must not turn the whole Review Center into Safe Mode.
    let safe_mode_reason = None;
    let model = build_review_center_view_model(ReviewCenterBuildInput {
        proposals,
        safe_mode_active: false,
        safe_mode_reason,
        safe_paths,
        safe_path_overrides,
        materialization_overrides,
        terminal_owner_task_session_ids,
        artifact_evidence,
    });

    let status = if model.items.is_empty() {
        ViewModelStatus::Empty
    } else {
        ViewModelStatus::Ready
    };
    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(model));
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.warnings.append(&mut warnings);
    Ok(envelope)
}

async fn proposal_safe_path_overrides(
    state: &Arc<AppState>,
    proposals: &[AgentProposal],
) -> (BTreeMap<String, Vec<String>>, Vec<ViewModelWarning>) {
    let mut overrides = BTreeMap::new();
    let mut warnings = Vec::new();
    for proposal in proposals.iter().filter(|proposal| {
        proposal
            .after
            .get("source")
            .and_then(serde_json::Value::as_str)
            == Some("markdown_memory_editor")
    }) {
        match crate::commands::proposal::artifact_safe_paths_for_proposal(state, proposal).await {
            Ok(paths) => {
                overrides.insert(proposal.id.clone(), paths);
            }
            Err(error) => warnings.push(warning(
                "markdown_memory_review_scope_unavailable",
                format!(
                    "Markdown Memory review scope could not be confirmed for proposal {}: {error}",
                    proposal.id
                ),
            )),
        }
    }
    (overrides, warnings)
}

fn dispatch_materialization_overrides(
    proposal_store: &openlife_core::agent::ProposalStore,
    proposals: &[AgentProposal],
) -> (
    BTreeMap<String, ReviewItemMaterializationStatus>,
    Vec<ViewModelWarning>,
) {
    let mut overrides = BTreeMap::new();
    let mut warnings = Vec::new();
    for proposal in proposals
        .iter()
        .filter(|proposal| is_dispatch_backed_review_item(proposal))
    {
        match proposal_store.dispatch_state(&proposal.id) {
            Ok(Some(dispatch_state)) => {
                if let Some(status) =
                    action_materialization_status(proposal.status, dispatch_state.as_str())
                {
                    overrides.insert(proposal.id.clone(), status);
                }
            }
            Ok(None) => {
                overrides.insert(
                    proposal.id.clone(),
                    ReviewItemMaterializationStatus::Unknown,
                );
                warnings.push(warning(
                    "governed_action_dispatch_receipt_missing",
                    format!(
                        "Governed action {} has no dispatch receipt; its effect stays unknown.",
                        proposal.id
                    ),
                ));
            }
            Err(error) => {
                overrides.insert(
                    proposal.id.clone(),
                    ReviewItemMaterializationStatus::Unknown,
                );
                warnings.push(warning(
                    "governed_action_dispatch_receipt_unavailable",
                    format!(
                        "Governed action {} dispatch receipt could not be read: {error}",
                        proposal.id
                    ),
                ));
            }
        }
    }
    (overrides, warnings)
}

fn is_dispatch_backed_review_item(proposal: &AgentProposal) -> bool {
    match proposal.proposal_type {
        ProposalType::ScheduledTask | ProposalType::MemoryArchive => true,
        ProposalType::LifeModelUpdate => matches!(
            proposal.affected_path.as_str(),
            openlife_core::life_model::v2::LIFE_MODEL_V2_TYPED_DIFF_PATH
                | openlife_core::life_model::v2::LIFE_MODEL_V2_LEGACY_MIGRATION_PATH
        ),
        ProposalType::DataExport => matches!(
            proposal
                .after
                .get("tool")
                .and_then(serde_json::Value::as_str),
            Some("email.propose_draft" | "browser.open" | "local.run_utility")
        ),
        _ => false,
    }
}

fn action_materialization_status(
    proposal_status: ProposalStatus,
    dispatch_state: &str,
) -> Option<ReviewItemMaterializationStatus> {
    match dispatch_state {
        "unclaimed" => None,
        "claimed" | "confirmed_projection_pending" => {
            Some(ReviewItemMaterializationStatus::Applying)
        }
        "failed_before_effect" => Some(ReviewItemMaterializationStatus::Failed),
        "unknown" => Some(ReviewItemMaterializationStatus::Unknown),
        "confirmed" if proposal_status == ProposalStatus::Accepted => {
            Some(ReviewItemMaterializationStatus::Applied)
        }
        "confirmed" => Some(ReviewItemMaterializationStatus::Unknown),
        _ => Some(ReviewItemMaterializationStatus::Unknown),
    }
}

async fn memory_materialization_overrides(
    state: &Arc<AppState>,
    proposals: &[openlife_core::agent::AgentProposal],
) -> (
    BTreeMap<String, ReviewItemMaterializationStatus>,
    Vec<ViewModelWarning>,
) {
    let Some(lifecycle_store) = state.memory_lifecycle_store.as_ref() else {
        return (
            BTreeMap::new(),
            vec![warning(
                "memory_lifecycle_store_unavailable",
                "Memory lifecycle proof is unavailable; accepted memory review items stay fail-closed.",
            )],
        );
    };

    let lifecycle_store = lifecycle_store.lock().await;
    let mut overrides = BTreeMap::new();
    let mut warnings = Vec::new();
    for proposal in proposals {
        match lifecycle_store.get_record_by_proposal_id(&proposal.id) {
            Ok(Some(record)) => {
                overrides.insert(
                    proposal.id.clone(),
                    materialization_status_from_memory_lifecycle(
                        record.status,
                        record.materialization_status,
                    ),
                );
            }
            Ok(None) => {}
            Err(err) => warnings.push(warning(
                "memory_lifecycle_lookup_failed",
                format!(
                    "Memory lifecycle proof lookup failed for proposal {}: {err}",
                    proposal.id
                ),
            )),
        }
    }
    (overrides, warnings)
}

fn materialization_status_from_memory_lifecycle(
    status: MemoryLifecycleStatus,
    materialization_status: MemoryMaterializationStatus,
) -> ReviewItemMaterializationStatus {
    if status == MemoryLifecycleStatus::RolledBack {
        return ReviewItemMaterializationStatus::RolledBack;
    }
    if status == MemoryLifecycleStatus::MaterializationFailed {
        return ReviewItemMaterializationStatus::Failed;
    }
    match materialization_status {
        MemoryMaterializationStatus::NotRequired => ReviewItemMaterializationStatus::NotApplicable,
        MemoryMaterializationStatus::Pending => ReviewItemMaterializationStatus::Applying,
        MemoryMaterializationStatus::Materialized => ReviewItemMaterializationStatus::Applied,
        MemoryMaterializationStatus::Failed => ReviewItemMaterializationStatus::Failed,
    }
}

fn warning(code: impl Into<String>, message: impl Into<String>) -> ViewModelWarning {
    ViewModelWarning {
        code: code.into(),
        message: message.into(),
        severity: ViewModelWarningSeverity::Warning,
        evidence_refs: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        action_materialization_status, dispatch_materialization_overrides,
        is_dispatch_backed_review_item,
    };
    use openlife_core::agent::{
        AgentProposal, ProposalSource, ProposalStatus, ProposalStore, ProposalType,
        ReviewItemMaterializationStatus, RiskLevel,
    };
    use serde_json::json;

    fn local_utility_proposal() -> AgentProposal {
        AgentProposal::new(
            ProposalType::DataExport,
            "local.utility",
            json!({
                "tool": "local.run_utility",
                "command": "uptime",
                "timeout_ms": 3_000,
                "content": "Run the reviewed read-only utility `uptime`."
            }),
            "Run one reviewed read-only utility.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        )
    }

    fn memory_stop_recall_proposal() -> AgentProposal {
        AgentProposal::new(
            ProposalType::MemoryArchive,
            "memory.lifecycle.memory:test",
            json!({
                "owner": {
                    "ownerKind": "memory_lifecycle",
                    "ownerId": "memory:test"
                },
                "recallDisposition": "paused"
            }),
            "Stop recalling one reviewed Memory.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        )
    }

    #[test]
    fn confirmed_dispatch_is_applied_only_after_accepted_projection() {
        assert_eq!(
            action_materialization_status(ProposalStatus::Accepted, "confirmed"),
            Some(ReviewItemMaterializationStatus::Applied)
        );
        assert_eq!(
            action_materialization_status(ProposalStatus::Pending, "confirmed"),
            Some(ReviewItemMaterializationStatus::Unknown)
        );
        assert_eq!(
            action_materialization_status(ProposalStatus::Pending, "unknown"),
            Some(ReviewItemMaterializationStatus::Unknown)
        );
    }

    #[test]
    fn review_projection_reads_confirmed_local_utility_dispatch_receipt() {
        let store = ProposalStore::new_in_memory().expect("proposal store");
        let mut proposal = local_utility_proposal();
        store.create_proposal(&proposal).expect("create proposal");
        let claim = store
            .claim_dispatch(&proposal.id)
            .expect("claim dispatch")
            .expect("claim id");
        assert!(store
            .mark_effect_confirmed_projection_pending(&proposal.id, &claim)
            .expect("persist confirmed effect"));
        proposal.accept();
        assert!(store
            .project_confirmed_effect(&proposal, &claim)
            .expect("project accepted proposal"));

        let (overrides, warnings) =
            dispatch_materialization_overrides(&store, std::slice::from_ref(&proposal));

        assert!(warnings.is_empty());
        assert_eq!(
            overrides.get(&proposal.id),
            Some(&ReviewItemMaterializationStatus::Applied)
        );
    }

    #[test]
    fn review_projection_reads_confirmed_lifemodel_v2_dispatch_receipt() {
        let store = ProposalStore::new_in_memory().expect("proposal store");
        let mut proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            openlife_core::life_model::v2::LIFE_MODEL_V2_TYPED_DIFF_PATH,
            json!({"schemaVersion": "openlife.lifemodel.typed-diff.v2"}),
            "Apply one reviewed LifeModel v2 change.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        store.create_proposal(&proposal).expect("create proposal");
        let claim = store
            .claim_dispatch(&proposal.id)
            .expect("claim dispatch")
            .expect("claim id");
        assert!(store
            .mark_effect_confirmed_projection_pending(&proposal.id, &claim)
            .expect("persist confirmed effect"));
        proposal.accept();
        assert!(store
            .project_confirmed_effect(&proposal, &claim)
            .expect("project accepted proposal"));

        let (overrides, warnings) =
            dispatch_materialization_overrides(&store, std::slice::from_ref(&proposal));

        assert!(warnings.is_empty());
        assert_eq!(
            overrides.get(&proposal.id),
            Some(&ReviewItemMaterializationStatus::Applied)
        );
    }

    #[test]
    fn review_projection_reads_confirmed_memory_retrieval_dispatch_receipt() {
        let store = ProposalStore::new_in_memory().expect("proposal store");
        let mut proposal = memory_stop_recall_proposal();
        store.create_proposal(&proposal).expect("create proposal");
        let claim = store
            .claim_dispatch(&proposal.id)
            .expect("claim dispatch")
            .expect("claim id");
        assert!(store
            .mark_effect_confirmed_projection_pending(&proposal.id, &claim)
            .expect("persist confirmed effect"));
        proposal.accept();
        assert!(store
            .project_confirmed_effect(&proposal, &claim)
            .expect("project accepted proposal"));

        let (overrides, warnings) =
            dispatch_materialization_overrides(&store, std::slice::from_ref(&proposal));

        assert!(warnings.is_empty());
        assert_eq!(
            overrides.get(&proposal.id),
            Some(&ReviewItemMaterializationStatus::Applied)
        );
    }

    #[test]
    fn generic_data_export_does_not_gain_dispatch_backed_action_credit() {
        let proposal = AgentProposal::new(
            ProposalType::DataExport,
            "exports.generic",
            json!({"content": "data", "filename": "export.txt"}),
            "Export reviewed data.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );

        assert!(!is_dispatch_backed_review_item(&proposal));
    }
}
