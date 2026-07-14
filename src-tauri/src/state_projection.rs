//! StateStore -> LifeModel YAML compatibility projection.
//!
//! StateStore remains canonical for assets carrying the `state-asset-v1`
//! marker. Existing unmarked YAML goals are preserved as read-only migration
//! source data; product mutations never write both owners.

use crate::life_model_materializer_guard::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::AppState;
use openlife_core::life_model::DailyGoal;
use openlife_core::state_store::{DailyTaskStatus, StateProjectionStatus};
use std::sync::Arc;

const STATE_ASSET_PROJECTION_DIGEST_PREFIX: &str = "state-asset-v1:";
const MAX_PROJECTION_BATCH: usize = 512;
const MAX_PROJECTION_CAS_ATTEMPTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StateProjectionReport {
    pub(crate) delivery_count: usize,
    pub(crate) status: StateProjectionStatus,
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
        "{}:{}:{}:{}:{}",
        asset.asset_id,
        asset.version,
        status,
        asset
            .due_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_default(),
        asset.title,
    ));
    DailyGoal {
        name: asset.title.clone(),
        done: asset.status == DailyTaskStatus::Completed,
        time_block: None,
        due_at: asset.due_at.map(|value| value.to_rfc3339()),
        operation_id: Some(asset.asset_id.clone()),
        operation_digest: Some(format!("{STATE_ASSET_PROJECTION_DIGEST_PREFIX}{digest}")),
    }
}

pub(crate) async fn reconcile_state_store_lifemodel_projection(
    state: &Arc<AppState>,
) -> Result<StateProjectionReport, String> {
    let store = state
        .state_store
        .as_ref()
        .ok_or_else(|| "state_store_unavailable_degraded".to_string())?;
    let deliveries = store
        .list_replayable_projection_deliveries(MAX_PROJECTION_BATCH)
        .map_err(|error| format!("load state projection outbox failed: {error}"))?;
    if deliveries.is_empty() {
        return Ok(StateProjectionReport {
            delivery_count: 0,
            status: StateProjectionStatus::Applied,
        });
    }
    // Building the compatibility model outside LifeModelWriteGateway is safe
    // only with compare-and-swap. Concurrent state commits or another
    // LifeModel writer can otherwise let an older projector overwrite a newer
    // view and then acknowledge the newer outbox delivery. Retry from fresh
    // canonical inputs instead of accepting that lost-update window.
    let mut projection_result = Err("state_projection_cas_retry_exhausted".to_string());
    for _ in 0..MAX_PROJECTION_CAS_ATTEMPTS {
        let assets = store
            .list_daily_tasks(false)
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
        model
            .goals
            .daily
            .retain(|goal| !is_state_store_projected_daily_goal(goal));
        model
            .goals
            .daily
            .extend(assets.iter().map(projected_daily_goal));

        projection_result =
            crate::life_model_write_gateway::persist_life_model_with_gateway_expected(
                state,
                model,
                false,
                LifeModelMaterializerCallerContext::new(
                    "state_store_daily_task_compatibility_projection",
                    LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
                    LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
                ),
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
            for delivery in &deliveries {
                store
                    .mark_projection_applied(&delivery.event_id)
                    .map_err(|error| format!("ack state projection failed: {error}"))?;
            }
            Ok(StateProjectionReport {
                delivery_count: deliveries.len(),
                status: StateProjectionStatus::Applied,
            })
        }
        Err(error) => {
            for delivery in &deliveries {
                if let Err(mark_error) = store.mark_projection_degraded(
                    &delivery.event_id,
                    "lifemodel_yaml_compatibility_projection_failed",
                ) {
                    return Err(format!(
                        "state projection failed ({error}); degraded receipt failed: {mark_error}"
                    ));
                }
            }
            Err(format!("state compatibility projection failed: {error}"))
        }
    }
}
