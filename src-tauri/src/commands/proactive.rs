use crate::AppState;
use openlife_core::proactive::{
    ProactiveConfig, ProactiveEngine, ProactiveLongTermGoal, ProactivePersonalContext,
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_proactive_suggestions(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::proactive::ProactiveSuggestion>, String> {
    get_proactive_suggestions_with_state(state.inner()).await
}

pub(crate) async fn get_proactive_suggestions_with_state(
    state: &Arc<AppState>,
) -> Result<Vec<openlife_core::proactive::ProactiveSuggestion>, String> {
    let cfg = state.config.lock().await;
    let config = ProactiveConfig {
        stale_goal_days: cfg.system.stale_goal_days,
        proposal_reminder_days: cfg.system.proposal_reminder_days,
        ..Default::default()
    };

    let canonical_life_model = {
        let manager = state.life_model_manager.lock().await;
        manager
            .load_v2_current(openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .map_err(|e| format!("Failed to load canonical LifeModel v2: {e}"))?
    };
    let state_store = state
        .state_store
        .as_ref()
        .ok_or_else(|| "Proactive StateStore unavailable".to_string())?;
    let daily_tasks = state_store
        .get_product_daily_tasks()
        .map_err(|e| format!("Failed to load proactive daily tasks: {e}"))?
        .into_iter()
        .map(|task| task.title)
        .collect::<Vec<_>>();
    let latest_state_observation_at = state_store
        .list_state_observations(false)
        .map_err(|e| format!("Failed to load proactive state observations: {e}"))?
        .into_iter()
        .map(|observation| observation.updated_at)
        .max();
    let (values, long_term_goals) = if let Some(version) = canonical_life_model {
        version
            .validate_integrity()
            .map_err(|e| format!("Canonical LifeModel v2 integrity failed: {e}"))?;
        let values = version
            .document
            .values
            .iter()
            .map(|value| value.statement.clone())
            .collect();
        let long_term_goals = version
            .document
            .long_term_goals
            .iter()
            .map(|goal| {
                let confirmed_at = chrono::DateTime::parse_from_rfc3339(&goal.confirmed_at)
                    .map_err(|_| "Canonical LifeModel goal has invalid confirmedAt".to_string())?
                    .with_timezone(&chrono::Utc);
                Ok(ProactiveLongTermGoal {
                    direction: goal.direction.clone(),
                    confirmed_at,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        (values, long_term_goals)
    } else {
        (Vec::new(), Vec::new())
    };
    let personal_context = ProactivePersonalContext::bounded(
        daily_tasks,
        values,
        long_term_goals,
        latest_state_observation_at,
    );

    // Count pending proposals
    let (pending_count, high_risk_count, oldest_days) = {
        let proposals = if let Some(ref store) = state.proposal_store {
            let store = store.lock().await;
            store
                .list_pending_proposals(100)
                .map_err(|e| format!("Failed to list proposals: {}", e))?
        } else {
            vec![]
        };

        let now = chrono::Utc::now();
        let high_risk = proposals
            .iter()
            .filter(|p| {
                matches!(
                    p.risk_level,
                    openlife_core::agent::RiskLevel::High
                        | openlife_core::agent::RiskLevel::Critical
                )
            })
            .count();
        let oldest = proposals
            .iter()
            .map(|p| (now - p.created_at).num_days())
            .max();

        (proposals.len(), high_risk, oldest)
    };

    let engine = ProactiveEngine::new(config);
    let evidence_store = state.evidence_store.lock().await;
    Ok(engine.generate_suggestions_with_evidence(
        &personal_context,
        pending_count,
        high_risk_count,
        oldest_days,
        &evidence_store,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
    use openlife_core::proactive::{ProactiveCategory, ProactiveEngine, ProactivePriority};

    #[tokio::test]
    async fn proactive_suggestions_with_state_use_negative_reminder_evidence() {
        let state = crate::test_utils::test_app_state();
        let pending = AgentProposal::new(
            ProposalType::ScheduledTask,
            "proactive.reminder.pending_proposal",
            serde_json::json!({
                "proactive_reminder_category": "pending_proposal",
                "prompt_digest": "pending-digest",
            }),
            "Pending proposal reminder",
            0.7,
            RiskLevel::High,
            ProposalSource::ProactiveAgent,
        );
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&pending).unwrap();
        }

        let mut rejected = AgentProposal::new(
            ProposalType::ScheduledTask,
            "proactive.reminder.pending_proposal",
            serde_json::json!({
                "proactive_reminder_category": "pending_proposal",
                "prompt_digest": "rejected-digest",
            }),
            "raw rejected reminder copy should not affect suggestions",
            0.7,
            RiskLevel::Low,
            ProposalSource::ProactiveAgent,
        );
        rejected.reject();
        {
            let evidence_store = state.evidence_store.lock().await;
            ProactiveEngine::default()
                .record_rejected_reminder_proposal(&evidence_store, &rejected)
                .unwrap();
        }

        let suggestions = get_proactive_suggestions_with_state(&state).await.unwrap();
        let pending = suggestions
            .iter()
            .find(|suggestion| suggestion.category == ProactiveCategory::PendingProposal)
            .expect("pending proposal reminder should still exist");
        assert_eq!(pending.priority, ProactivePriority::Low);
    }
}
