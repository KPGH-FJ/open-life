use crate::AppState;
use openlife_core::proactive::{ProactiveConfig, ProactiveEngine};
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

    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager
            .load_active_legacy_runtime_model()
            .map_err(|e| format!("Failed to load LifeModel: {}", e))?
            .unwrap_or_else(openlife_core::life_model::LifeModel::default_model)
    };

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
        &life_model,
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
