use crate::AppState;
use openlife_core::proactive::{ProactiveConfig, ProactiveEngine};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_proactive_suggestions(
    state: State<'_, Arc<AppState>>,
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
            .load()
            .map_err(|e| format!("Failed to load LifeModel: {}", e))?
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
    Ok(engine.generate_suggestions(&life_model, pending_count, high_risk_count, oldest_days))
}
