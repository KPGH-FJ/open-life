//! StateStore -> LifeModel YAML compatibility projection.
//!
//! StateStore remains canonical for assets carrying the `state-asset-v1`
//! marker. Existing unmarked YAML goals are preserved as read-only migration
//! source data; product mutations never write both owners.

use crate::life_model_materializer_guard::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose, STATE_STORE_DAILY_TASK_COMPATIBILITY_MATERIALIZER_ID,
};
use crate::persistence_coordinator::{
    CanonicalCommitPermit, CanonicalWriteOwner, PersistenceGateError,
};
use crate::AppState;
use openlife_core::life_model::{DailyGoal, LifeModel, TimeBlock};
use openlife_core::persistence_outbox::ProjectionDeliveryState;
use openlife_core::state_store::{
    DailyTaskStatus, LegacyDailyTaskImportReceipt, LegacyDailyTaskShadowCandidate,
    LegacyDailyTaskShadowReceipt, LegacyStateHistoryShadowCandidate,
    LegacyStateHistoryShadowReceipt, StateProjectionStatus, StateStore,
};
use std::sync::Arc;

const STATE_ASSET_PROJECTION_DIGEST_PREFIX: &str = "state-asset-v1:";
const MAX_PROJECTION_BATCH: usize = 512;
const MAX_PROJECTION_CAS_ATTEMPTS: usize = 3;

#[derive(Debug)]
pub(crate) enum StateProjectionError {
    /// An exclusive terminalization fence superseded the marker admission.
    /// The canonical StateStore delivery remains pending and replayable; this
    /// is not a durable projection failure.
    Deferred(PersistenceGateError),
    Failed(String),
}

impl From<String> for StateProjectionError {
    fn from(error: String) -> Self {
        Self::Failed(error)
    }
}

impl From<&str> for StateProjectionError {
    fn from(error: &str) -> Self {
        Self::Failed(error.to_string())
    }
}

impl std::fmt::Display for StateProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deferred(error) => {
                write!(formatter, "state projection finalization deferred: {error}")
            }
            Self::Failed(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for StateProjectionError {}

/// StateStore delivery acknowledgement is itself a canonical StateStore
/// mutation. The LifeModel projector deliberately releases its LifeModel
/// permit before returning, so finalization must enter a fresh, short-lived
/// StateStore commit window instead of extending one permit across the
/// projection await or recursively acquiring the shared barrier.
async fn acquire_state_projection_finalize_permit<'state>(
    state: &'state Arc<AppState>,
) -> Result<CanonicalCommitPermit<'state>, StateProjectionError> {
    let admission = state
        .persistence_coordinator
        .require_canonical_write(CanonicalWriteOwner::StateStore)
        .map_err(|error| {
            StateProjectionError::Failed(format!("state projection finalization blocked: {error}"))
        })?;
    state
        .persistence_coordinator
        .acquire_canonical_commit_permit(&admission)
        .await
        .map_err(|error| match error {
            PersistenceGateError::AdmissionInvalidated { .. } => {
                StateProjectionError::Deferred(error)
            }
            other => StateProjectionError::Failed(format!(
                "state projection finalization blocked: {other}"
            )),
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateProjectionReport {
    pub(crate) delivery_count: usize,
    pub(crate) status: StateProjectionStatus,
    /// Present only for the foreground exact-event lane. This is read back
    /// from the durable outbox after acknowledgement; it is not inferred from
    /// a successful LifeModel write or from inclusion in the requested batch.
    pub(crate) required_event: Option<RequiredStateProjectionProof>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequiredStateProjectionProof {
    pub(crate) event_id: String,
    pub(crate) delivery_state: ProjectionDeliveryState,
}

pub(crate) fn is_state_store_projected_daily_goal(goal: &DailyGoal) -> bool {
    goal.operation_digest
        .as_deref()
        .is_some_and(|digest| digest.starts_with(STATE_ASSET_PROJECTION_DIGEST_PREFIX))
}

pub(crate) fn projected_daily_goal(asset: &openlife_core::state_store::StateAsset) -> DailyGoal {
    let status = match asset.status {
        DailyTaskStatus::Pending => "pending",
        DailyTaskStatus::Completed => "completed",
        DailyTaskStatus::Tombstoned => "tombstoned",
    };
    let digest = openlife_core::persistence_outbox::metadata_digest(&format!(
        "{}:{}:{}:{}:{}:{}:{}",
        asset.asset_id,
        asset.version,
        status,
        asset
            .due_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_default(),
        asset.time_block_start.as_deref().unwrap_or_default(),
        asset.time_block_end.as_deref().unwrap_or_default(),
        asset.title,
    ));
    DailyGoal {
        name: asset.title.clone(),
        done: asset.status == DailyTaskStatus::Completed,
        time_block: asset
            .time_block_start
            .as_ref()
            .zip(asset.time_block_end.as_ref())
            .map(|(start, end)| TimeBlock {
                start: start.clone(),
                end: end.clone(),
            }),
        due_at: asset.due_at.map(|value| value.to_rfc3339()),
        operation_id: Some(asset.asset_id.clone()),
        operation_digest: Some(format!("{STATE_ASSET_PROJECTION_DIGEST_PREFIX}{digest}")),
    }
}

pub(crate) fn legacy_yaml_daily_task_shadow_candidates(
    model: &LifeModel,
) -> Result<Vec<LegacyDailyTaskShadowCandidate>, String> {
    model
        .goals
        .daily
        .iter()
        .filter(|goal| !is_state_store_projected_daily_goal(goal))
        .enumerate()
        .map(|(ordinal, goal)| {
            let due_at = goal
                .due_at
                .as_deref()
                .map(|value| {
                    chrono::DateTime::parse_from_rfc3339(value)
                        .map(|value| value.with_timezone(&chrono::Utc))
                        .map_err(|_| format!("legacy_daily_task_due_at_invalid:ordinal={ordinal}"))
                })
                .transpose()?;
            Ok(LegacyDailyTaskShadowCandidate {
                source_ordinal: u32::try_from(ordinal)
                    .map_err(|_| "legacy_daily_task_ordinal_overflow".to_string())?,
                title: goal.name.clone(),
                completed: goal.done,
                time_block_start: goal.time_block.as_ref().map(|block| block.start.clone()),
                time_block_end: goal.time_block.as_ref().map(|block| block.end.clone()),
                due_at,
                legacy_operation_id: goal.operation_id.clone(),
                legacy_operation_digest: goal.operation_digest.clone(),
            })
        })
        .collect()
}

pub(crate) fn legacy_yaml_daily_task_source_digest(model: &LifeModel) -> Result<String, String> {
    let legacy_goals = model
        .goals
        .daily
        .iter()
        .filter(|goal| !is_state_store_projected_daily_goal(goal))
        .collect::<Vec<_>>();
    let encoded = serde_json::to_string(&legacy_goals)
        .map_err(|error| format!("legacy_daily_task_source_encode_failed:{error}"))?;
    Ok(openlife_core::persistence_outbox::metadata_digest(&encoded))
}

fn empty_legacy_yaml_daily_task_source_digest() -> String {
    openlife_core::persistence_outbox::metadata_digest("[]")
}

pub(crate) fn reconcile_legacy_yaml_daily_task_shadow(
    store: &StateStore,
    model: &LifeModel,
    observed_at: chrono::DateTime<chrono::Utc>,
) -> Result<LegacyDailyTaskShadowReceipt, String> {
    let source_asset_digest = legacy_yaml_daily_task_source_digest(model)?;
    let candidates = legacy_yaml_daily_task_shadow_candidates(model)?;
    store
        .reconcile_legacy_daily_task_shadow(source_asset_digest, candidates, observed_at)
        .map_err(|error| format!("legacy_daily_task_shadow_reconciliation_failed:{error}"))
}

pub(crate) fn reconcile_and_import_legacy_yaml_daily_tasks(
    store: &StateStore,
    model: &LifeModel,
    observed_at: chrono::DateTime<chrono::Utc>,
) -> Result<LegacyDailyTaskImportReceipt, String> {
    let source_asset_digest = legacy_yaml_daily_task_source_digest(model)?;
    if let Some(existing) = store
        .legacy_daily_task_import_receipt(false)
        .map_err(|error| format!("legacy_daily_task_import_receipt_load_failed:{error}"))?
    {
        if source_asset_digest == existing.source_asset_digest {
            return store
                .import_legacy_daily_task_shadow(observed_at)
                .map_err(|error| format!("legacy_daily_task_import_replay_failed:{error}"));
        }
        if source_asset_digest == empty_legacy_yaml_daily_task_source_digest() {
            return Ok(existing);
        }
        return Err("legacy_daily_task_source_changed_after_cutover".into());
    }

    reconcile_legacy_yaml_daily_task_shadow(store, model, observed_at)?;
    store
        .import_legacy_daily_task_shadow(observed_at)
        .map_err(|error| format!("legacy_daily_task_canonical_import_failed:{error}"))
}

pub(crate) fn validate_legacy_yaml_daily_task_cutover_source(
    store: &StateStore,
    model: &LifeModel,
) -> Result<LegacyDailyTaskImportReceipt, String> {
    let receipt = store
        .legacy_daily_task_import_receipt(false)
        .map_err(|error| format!("legacy_daily_task_import_receipt_load_failed:{error}"))?
        .ok_or_else(|| "daily_task_product_owner_not_ready".to_string())?;
    let source_asset_digest = legacy_yaml_daily_task_source_digest(model)?;
    if source_asset_digest != receipt.source_asset_digest
        && source_asset_digest != empty_legacy_yaml_daily_task_source_digest()
    {
        return Err("legacy_daily_task_source_changed_after_cutover".into());
    }
    if receipt.candidate_digest != receipt.canonical_digest {
        return Err("daily_task_product_owner_receipt_inconsistent".into());
    }
    Ok(receipt)
}

pub(crate) fn legacy_memory_state_history_shadow_candidates(
    snapshot: &openlife_core::memory::LegacyStateHistoryMigrationSnapshot,
) -> Result<Vec<LegacyStateHistoryShadowCandidate>, String> {
    snapshot
        .records
        .iter()
        .map(|record| {
            let recorded_at = chrono::DateTime::parse_from_rfc3339(&record.recorded_at)
                .map(|value| value.with_timezone(&chrono::Utc))
                .map_err(|_| {
                    format!(
                        "legacy_state_history_recorded_at_invalid:legacy_id={}",
                        record.id
                    )
                })?;
            Ok(LegacyStateHistoryShadowCandidate {
                legacy_id: record.id,
                dimension_name: record.dimension_name.clone(),
                value: record.value,
                unit: record.unit.clone(),
                recorded_at,
                note: record.note.clone(),
                legacy_operation_id: record.operation_id.clone(),
                legacy_operation_digest: record.operation_digest.clone(),
            })
        })
        .collect()
}

pub(crate) fn legacy_memory_state_history_source_digest(
    snapshot: &openlife_core::memory::LegacyStateHistoryMigrationSnapshot,
) -> Result<String, String> {
    if snapshot.source_store_identity.trim().is_empty() {
        return Err("legacy_state_history_source_store_identity_missing".into());
    }
    snapshot
        .validate_source_store_identity()
        .map_err(|error| error.to_string())?;
    let encoded = serde_json::to_string(snapshot)
        .map_err(|error| format!("legacy_state_history_source_encode_failed:{error}"))?;
    Ok(openlife_core::persistence_outbox::metadata_digest(&encoded))
}

pub(crate) fn reconcile_legacy_memory_state_history_shadow(
    store: &StateStore,
    snapshot: &openlife_core::memory::LegacyStateHistoryMigrationSnapshot,
    observed_at: chrono::DateTime<chrono::Utc>,
) -> Result<LegacyStateHistoryShadowReceipt, String> {
    let source_asset_digest = legacy_memory_state_history_source_digest(snapshot)?;
    let candidates = legacy_memory_state_history_shadow_candidates(snapshot)?;
    store
        .reconcile_legacy_state_history_shadow(source_asset_digest, candidates, observed_at)
        .map_err(|error| format!("legacy_state_history_shadow_reconciliation_failed:{error}"))
}

pub(crate) async fn reconcile_state_store_lifemodel_projection(
    state: &Arc<AppState>,
) -> Result<StateProjectionReport, StateProjectionError> {
    reconcile_state_store_lifemodel_projection_inner(state, None).await
}

async fn reconcile_state_store_lifemodel_projection_inner(
    state: &Arc<AppState>,
    required_event_id: Option<&str>,
) -> Result<StateProjectionReport, StateProjectionError> {
    let store = state
        .state_store
        .as_ref()
        .ok_or_else(|| "state_store_unavailable_degraded".to_string())?;
    let mut deliveries = store
        .list_replayable_projection_deliveries(MAX_PROJECTION_BATCH)
        .map_err(|error| format!("load state projection outbox failed: {error}"))?;
    if let Some(required_event_id) = required_event_id {
        let required_deliveries = store
            .list_replayable_projection_deliveries_for_event(required_event_id)
            .map_err(|error| format!("load required state projection event failed: {error}"))?;
        for required_delivery in required_deliveries {
            if !deliveries.iter().any(|delivery| {
                delivery.event_id == required_delivery.event_id
                    && delivery.projection_target == required_delivery.projection_target
            }) {
                deliveries.push(required_delivery);
            }
        }
        let required_state = store
            .projection_delivery_state_for_event(required_event_id)
            .map_err(|error| {
                format!("load required state projection delivery state failed: {error}")
            })?;
        if required_state != ProjectionDeliveryState::Applied
            && !deliveries
                .iter()
                .any(|delivery| delivery.event_id == required_event_id)
        {
            return Err(format!(
                "required state projection event is not replayable: event={required_event_id}, state={required_state:?}"
            )
            .into());
        }
    }
    if deliveries.is_empty() {
        let has_legacy_derived_alerts = !state
            .life_model_manager
            .lock()
            .await
            .load()
            .map_err(|error| format!("load LifeModel compatibility view failed: {error}"))?
            .state
            .alerts
            .is_empty();
        if !has_legacy_derived_alerts {
            return Ok(StateProjectionReport {
                delivery_count: 0,
                status: StateProjectionStatus::Applied,
                required_event: required_projection_proof(store, required_event_id)?,
            });
        }
    }
    // Building the compatibility model outside LifeModelWriteGateway is safe
    // only with compare-and-swap. Concurrent state commits or another
    // LifeModel writer can otherwise let an older projector overwrite a newer
    // view and then acknowledge the newer outbox delivery. Retry from fresh
    // canonical inputs instead of accepting that lost-update window.
    let mut projection_result = Err("state_projection_cas_retry_exhausted".to_string());
    for _ in 0..MAX_PROJECTION_CAS_ATTEMPTS {
        let assets = store
            .get_product_daily_tasks()
            .map_err(|error| format!("load canonical state assets failed: {error}"))?;
        let (mut model, expected_hash) = {
            let manager = state.life_model_manager.lock().await;
            let model = manager
                .load()
                .map_err(|error| format!("load LifeModel compatibility view failed: {error}"))?;
            let expected_hash = crate::life_model_write_gateway::hash_life_model(&model)
                .map_err(|error| format!("hash LifeModel compatibility view failed: {error}"))?;
            (model, expected_hash)
        };
        validate_legacy_yaml_daily_task_cutover_source(store, &model)?;
        model.goals.daily.clear();
        model
            .goals
            .daily
            .extend(assets.iter().map(projected_daily_goal));
        // Product alerts are derived on read from canonical StateStore
        // history plus LifeModel threshold configuration. Legacy Builder
        // alerts were persisted output and must be removed by this one exact,
        // auditable compatibility projector rather than kept as stale truth.
        model.state.alerts.clear();

        let caller_context = LifeModelMaterializerCallerContext::new(
            STATE_STORE_DAILY_TASK_COMPATIBILITY_MATERIALIZER_ID,
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
        );
        projection_result =
            crate::life_model_write_gateway::persist_life_model_with_gateway_expected(
                state,
                model,
                false,
                caller_context,
                Some(&expected_hash),
            )
            .await;
        match projection_result.as_ref() {
            Ok(_) => break,
            Err(error) if error == "LifeModel changed after required pre-change snapshot" => {
                continue;
            }
            Err(_) => break,
        }
    }
    match projection_result {
        Ok(_) => {
            let _commit_permit = acquire_state_projection_finalize_permit(state).await?;
            for delivery in &deliveries {
                store
                    .mark_projection_applied(&delivery.event_id)
                    .map_err(|error| format!("ack state projection failed: {error}"))?;
            }
            Ok(StateProjectionReport {
                delivery_count: deliveries.len(),
                status: StateProjectionStatus::Applied,
                required_event: required_projection_proof(store, required_event_id)?,
            })
        }
        Err(error) => {
            let _commit_permit = acquire_state_projection_finalize_permit(state)
                .await
                .map_err(|permit_error| match permit_error {
                    StateProjectionError::Deferred(error) => {
                        StateProjectionError::Deferred(error)
                    }
                    StateProjectionError::Failed(permit_error) => StateProjectionError::Failed(
                        format!(
                            "state projection failed ({error}); degraded receipt blocked: {permit_error}"
                        ),
                    ),
                })?;
            for delivery in &deliveries {
                if let Err(mark_error) = store.mark_projection_degraded(
                    &delivery.event_id,
                    "lifemodel_yaml_compatibility_projection_failed",
                ) {
                    return Err(format!(
                        "state projection failed ({error}); degraded receipt failed: {mark_error}"
                    )
                    .into());
                }
            }
            Err(format!("state compatibility projection failed: {error}").into())
        }
    }
}

fn required_projection_proof(
    store: &StateStore,
    required_event_id: Option<&str>,
) -> Result<Option<RequiredStateProjectionProof>, String> {
    let Some(required_event_id) = required_event_id else {
        return Ok(None);
    };
    let delivery_state = store
        .projection_delivery_state_for_event(required_event_id)
        .map_err(|error| {
            format!("verify required state projection delivery state failed: {error}")
        })?;
    if delivery_state != ProjectionDeliveryState::Applied {
        return Err(format!(
            "required state projection event was not applied: event={required_event_id}, state={delivery_state:?}"
        ));
    }
    Ok(Some(RequiredStateProjectionProof {
        event_id: required_event_id.to_string(),
        delivery_state,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::life_model::{LifeModel, TimeBlock};

    #[test]
    fn legacy_yaml_shadow_candidates_are_lossless_and_exclude_statestore_projection() {
        let mut model = LifeModel::default_model();
        model.goals.daily = vec![
            DailyGoal {
                name: "旧任务".into(),
                done: true,
                time_block: Some(TimeBlock {
                    start: "09:00".into(),
                    end: "10:00".into(),
                }),
                due_at: Some("2026-07-15T17:00:00Z".into()),
                operation_id: Some("legacy-operation".into()),
                operation_digest: Some("legacy-digest".into()),
            },
            DailyGoal {
                name: "StateStore 投影".into(),
                done: false,
                time_block: None,
                due_at: None,
                operation_id: Some(uuid::Uuid::new_v4().hyphenated().to_string()),
                operation_digest: Some(format!(
                    "{STATE_ASSET_PROJECTION_DIGEST_PREFIX}{}",
                    openlife_core::persistence_outbox::metadata_digest("canonical")
                )),
            },
        ];

        let candidates = legacy_yaml_daily_task_shadow_candidates(&model).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source_ordinal, 0);
        assert_eq!(candidates[0].title, "旧任务");
        assert!(candidates[0].completed);
        assert_eq!(candidates[0].time_block_start.as_deref(), Some("09:00"));
        assert_eq!(candidates[0].time_block_end.as_deref(), Some("10:00"));
        assert_eq!(
            candidates[0].due_at.unwrap().to_rfc3339(),
            "2026-07-15T17:00:00+00:00"
        );
        assert_eq!(
            candidates[0].legacy_operation_id.as_deref(),
            Some("legacy-operation")
        );
        assert_eq!(
            candidates[0].legacy_operation_digest.as_deref(),
            Some("legacy-digest")
        );
    }

    #[tokio::test]
    async fn exact_projector_clears_legacy_persisted_alerts_without_bumping_metadata() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut model = state.life_model_manager.lock().await.load().unwrap();
        reconcile_and_import_legacy_yaml_daily_tasks(
            state.state_store.as_ref().unwrap(),
            &model,
            chrono::Utc::now(),
        )
        .unwrap();
        reconcile_state_store_lifemodel_projection(&state)
            .await
            .unwrap();
        model = state.life_model_manager.lock().await.load().unwrap();
        model.metadata.version = "7.4.2".into();
        model.metadata.updated_at = "2026-07-16T00:00:00Z".into();
        model
            .state
            .alerts
            .push(openlife_core::life_model::StateAlert {
                dimension_name: "legacy-builder".into(),
                message: "persisted derived output".into(),
                ..Default::default()
            });
        state.life_model_manager.lock().await.save(&model).unwrap();

        let report = reconcile_state_store_lifemodel_projection(&state)
            .await
            .expect("legacy derived alerts must have one exact cleanup owner");

        assert_eq!(report.delivery_count, 0);
        assert_eq!(report.status, StateProjectionStatus::Applied);
        let after = state.life_model_manager.lock().await.load().unwrap();
        assert!(after.state.alerts.is_empty());
        assert_eq!(after.metadata.version, "7.4.2");
        assert_eq!(after.metadata.updated_at, "2026-07-16T00:00:00Z");
    }
    #[test]
    fn legacy_yaml_shadow_candidate_rejects_invalid_due_time_without_partial_guess() {
        let mut model = LifeModel::default_model();
        model.goals.daily.push(DailyGoal {
            name: "无法无损迁移".into(),
            done: false,
            time_block: None,
            due_at: Some("tomorrow-ish".into()),
            operation_id: None,
            operation_digest: None,
        });

        let error = legacy_yaml_daily_task_shadow_candidates(&model).unwrap_err();
        assert!(error.contains("legacy_daily_task_due_at_invalid"));
    }

    #[test]
    fn legacy_yaml_shadow_digest_is_scoped_to_the_daily_task_asset_category() {
        let mut model = LifeModel::default_model();
        model.goals.daily.push(DailyGoal {
            name: "同一个迁移任务".into(),
            done: false,
            time_block: None,
            due_at: None,
            operation_id: None,
            operation_digest: None,
        });
        let first = legacy_yaml_daily_task_source_digest(&model).unwrap();
        model.identity.name = "无关身份变化".into();
        let after_unrelated_change = legacy_yaml_daily_task_source_digest(&model).unwrap();
        assert_eq!(first, after_unrelated_change);

        model.goals.daily[0].done = true;
        let after_daily_task_change = legacy_yaml_daily_task_source_digest(&model).unwrap();
        assert_ne!(first, after_daily_task_change);
    }

    #[test]
    fn legacy_memory_state_history_shadow_candidates_preserve_every_source_field() {
        let operation_id = uuid::Uuid::new_v4().hyphenated().to_string();
        let operation_digest =
            openlife_core::persistence_outbox::metadata_digest("legacy-state-operation");
        let snapshot = openlife_core::memory::LegacyStateHistoryMigrationSnapshot {
            source_store_identity: "memory_store:v1:00000000-0000-4000-8000-000000000001".into(),
            records: vec![openlife_core::memory::LegacyStateHistorySourceRecord {
                id: 7,
                dimension_name: "专注度".into(),
                value: 8.5,
                unit: "分".into(),
                recorded_at: "2026-07-15T08:30:00Z".into(),
                note: Some("路演前".into()),
                operation_id: Some(operation_id.clone()),
                operation_digest: Some(operation_digest.clone()),
            }],
        };

        let candidates = legacy_memory_state_history_shadow_candidates(&snapshot).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].legacy_id, 7);
        assert_eq!(candidates[0].dimension_name, "专注度");
        assert_eq!(candidates[0].value, 8.5);
        assert_eq!(candidates[0].unit, "分");
        assert_eq!(
            candidates[0].recorded_at.to_rfc3339(),
            "2026-07-15T08:30:00+00:00"
        );
        assert_eq!(candidates[0].note.as_deref(), Some("路演前"));
        assert_eq!(
            candidates[0].legacy_operation_id.as_deref(),
            Some(operation_id.as_str())
        );
        assert_eq!(
            candidates[0].legacy_operation_digest.as_deref(),
            Some(operation_digest.as_str())
        );
        let first_digest = legacy_memory_state_history_source_digest(&snapshot).unwrap();
        assert_eq!(
            first_digest,
            openlife_core::persistence_outbox::metadata_digest(
                &serde_json::to_string(&snapshot).unwrap()
            )
        );

        let mut other_store_snapshot = snapshot;
        other_store_snapshot.source_store_identity =
            "memory_store:v1:00000000-0000-4000-8000-000000000002".into();
        assert_ne!(
            legacy_memory_state_history_source_digest(&other_store_snapshot).unwrap(),
            first_digest
        );
    }

    #[test]
    fn legacy_memory_state_history_shadow_rejects_invalid_timestamp_without_partial_guess() {
        let snapshot = openlife_core::memory::LegacyStateHistoryMigrationSnapshot {
            source_store_identity: "memory_store:v1:00000000-0000-4000-8000-000000000001".into(),
            records: vec![openlife_core::memory::LegacyStateHistorySourceRecord {
                id: 1,
                dimension_name: "energy".into(),
                value: 7.0,
                unit: "/10".into(),
                recorded_at: "yesterday-ish".into(),
                note: None,
                operation_id: None,
                operation_digest: None,
            }],
        };

        let error = legacy_memory_state_history_shadow_candidates(&snapshot).unwrap_err();
        assert!(error.contains("legacy_state_history_recorded_at_invalid:legacy_id=1"));
    }

    #[test]
    fn legacy_memory_state_history_shadow_rejects_missing_store_identity() {
        let snapshot = openlife_core::memory::LegacyStateHistoryMigrationSnapshot {
            source_store_identity: "   ".into(),
            records: Vec::new(),
        };

        let error = legacy_memory_state_history_source_digest(&snapshot).unwrap_err();
        assert_eq!(error, "legacy_state_history_source_store_identity_missing");
    }
}
