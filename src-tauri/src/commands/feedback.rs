use crate::errors::AppError;
use crate::AppState;
use openlife_core::feedback::{AnalyticsSummary, FeedbackEntry, FeedbackType};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn save_feedback(
    session_id: String,
    message_index: i64,
    feedback_type: String,
    content_preview: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let ft = match feedback_type.as_str() {
        "up" => FeedbackType::ThumbsUp,
        _ => FeedbackType::ThumbsDown,
    };
    let entry = FeedbackEntry {
        session_id,
        message_index,
        feedback_type: ft,
        content_preview,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let store = state.feedback_store.lock().await;
    store
        .save_feedback(&entry)
        .map_err(AppError::from)
        .map(|_| ())
}

#[tauri::command]
pub async fn get_feedback_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<AnalyticsSummary, AppError> {
    let store = state.feedback_store.lock().await;
    store.summary().map_err(AppError::from)
}

#[tauri::command]
pub async fn apply_feedback_evolution(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    apply_feedback_evolution_with_state_gated(state.inner()).await
}

#[cfg(test)]
async fn apply_feedback_evolution_with_state(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    apply_feedback_evolution_with_state_gated(state).await
}

async fn apply_feedback_evolution_with_state_gated(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    let store = state.feedback_store.lock().await;
    let report = store.generate_evolution_report().map_err(AppError::from)?;
    Err(AppError::permission(format!(
        "apply_feedback_evolution has been retired as a Feedback evolution legacy direct-write compatibility surface; create reviewable Proposal/Evidence candidates instead. Metadata-safe candidate counts: liked_patterns={}, disliked_patterns={}, suggested_rules={}.",
        report.liked_patterns.len(),
        report.disliked_patterns.len(),
        report.suggested_rules.len()
    )))
}

#[tauri::command]
pub async fn generate_evolution_report(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    generate_evolution_report_with_state(state.inner()).await
}

async fn generate_evolution_report_with_state(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    let store = state.feedback_store.lock().await;
    let report = store.generate_evolution_report().map_err(AppError::from)?;
    let liked_pattern_count = report.liked_patterns.len();
    let disliked_pattern_count = report.disliked_patterns.len();
    let suggested_rule_count = report.suggested_rules.len();

    Ok(serde_json::json!({
        "success": true,
        "read_only": true,
        "metadata_safe": true,
        "durable_lifemodel_write": false,
        "evolution_rules_write": false,
        "applied_rule_count": 0,
        "liked_pattern_count": liked_pattern_count,
        "disliked_pattern_count": disliked_pattern_count,
        "suggested_rule_count": suggested_rule_count,
        "proposal_candidate_count": suggested_rule_count,
        "candidate_status": "review_required_not_activated",
        "summary": format!(
            "Read-only feedback evolution report: {liked_pattern_count} liked pattern(s), {disliked_pattern_count} disliked pattern(s), {suggested_rule_count} review candidate(s)."
        ),
    }))
}

#[tauri::command]
pub async fn log_analytics_event(
    event_name: String,
    session_id: Option<String>,
    detail: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let store = state.feedback_store.lock().await;
    store
        .log_event(&event_name, session_id.as_deref(), detail.as_deref())
        .map_err(AppError::from)
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::feedback::{FeedbackEntry, FeedbackType};

    const RAW_FEEDBACK_MARKER: &str = "W83_RAW_FEEDBACK_TEXT_SECRET";
    const RAW_LIFEMODEL_VALUE: &str = "w83secretvalue";
    const RAW_LIFEMODEL_DESCRIPTION: &str = "W83_RAW_LIFEMODEL_DESCRIPTION_SECRET";
    const RAW_EXISTING_RULE: &str = "W83_RAW_EXISTING_EVOLUTION_RULE_SECRET";

    async fn seed_feedback_evolution_fixture(state: &Arc<AppState>) {
        {
            let manager = state.life_model_manager.lock().await;
            let mut model = manager.load().unwrap();
            model
                .identity
                .values
                .push(openlife_core::life_model::ValueItem {
                    name: RAW_LIFEMODEL_VALUE.into(),
                    weight: 5,
                    description: RAW_LIFEMODEL_DESCRIPTION.into(),
                });
            model.evolution_rules = vec![RAW_EXISTING_RULE.into()];
            manager.save(&model).unwrap();
        }

        let store = state.feedback_store.lock().await;
        for index in 0..5 {
            store
                .save_feedback(&FeedbackEntry {
                    session_id: "w83-feedback".into(),
                    message_index: index,
                    feedback_type: FeedbackType::ThumbsUp,
                    content_preview: format!("{RAW_FEEDBACK_MARKER} {RAW_LIFEMODEL_VALUE}"),
                    created_at: chrono::Utc::now().to_rfc3339(),
                })
                .unwrap();
        }
        store
            .save_conversation_inference(
                Some("w83-feedback"),
                "identity.values",
                RAW_LIFEMODEL_VALUE,
                0.03,
                1.0,
                "W83_RAW_CONVERSATION_INFERENCE_SECRET",
            )
            .unwrap();
    }

    async fn feedback_target_weight(state: &Arc<AppState>) -> u8 {
        let model = state.life_model_manager.lock().await.load().unwrap();
        model
            .identity
            .values
            .iter()
            .find(|value| value.name == RAW_LIFEMODEL_VALUE)
            .map(|value| value.weight)
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn w92_apply_feedback_evolution_fails_closed_as_retired_surface() {
        let state = crate::test_utils::test_app_state();
        seed_feedback_evolution_fixture(&state).await;

        let err = apply_feedback_evolution_with_state(&state)
            .await
            .expect_err("feedback evolution legacy direct apply must fail closed by default");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("apply_feedback_evolution"));
        assert!(err.message().contains("retired"));
        assert!(err.message().contains("Proposal/Evidence"));
        assert_eq!(feedback_target_weight(&state).await, 5);
        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(model.evolution_rules, vec![RAW_EXISTING_RULE.to_string()]);
    }

    #[tokio::test]
    async fn w92_apply_feedback_evolution_retirement_response_is_metadata_safe_and_writes_nothing()
    {
        let state = crate::test_utils::test_app_state();
        seed_feedback_evolution_fixture(&state).await;

        let err = apply_feedback_evolution_with_state(&state)
            .await
            .expect_err("W92 retires Feedback evolution direct apply");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("suggested_rules"));

        let response_dump = err.message().to_string();
        for forbidden in [
            RAW_FEEDBACK_MARKER,
            RAW_LIFEMODEL_VALUE,
            RAW_LIFEMODEL_DESCRIPTION,
            RAW_EXISTING_RULE,
            "W83_RAW_CONVERSATION_INFERENCE_SECRET",
            "提升",
            "用户偏好的表达方式",
        ] {
            assert!(
                !response_dump.contains(forbidden),
                "feedback evolution legacy response leaked raw marker {forbidden}"
            );
        }

        assert_eq!(feedback_target_weight(&state).await, 5);
        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(model.evolution_rules, vec![RAW_EXISTING_RULE.to_string()]);
    }

    #[tokio::test]
    async fn w83_generate_evolution_report_is_read_only_and_metadata_safe() {
        let state = crate::test_utils::test_app_state();
        seed_feedback_evolution_fixture(&state).await;
        let before_model = state.life_model_manager.lock().await.load().unwrap();

        let result = generate_evolution_report_with_state(&state).await.unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(result["read_only"], true);
        assert_eq!(result["metadata_safe"], true);
        assert_eq!(result["durable_lifemodel_write"], false);
        assert_eq!(result["evolution_rules_write"], false);
        assert_eq!(result["applied_rule_count"], 0);
        assert!(result["suggested_rule_count"].as_u64().unwrap_or_default() > 0);
        assert!(result.get("liked_patterns").is_none());
        assert!(result.get("disliked_patterns").is_none());
        assert!(result.get("suggested_rules").is_none());
        assert!(result.get("applied_rules").is_none());

        let after_model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(after_model.evolution_rules, before_model.evolution_rules);
        assert_eq!(after_model.identity.values[0].weight, 5);

        let response_dump = result.to_string();
        for forbidden in [
            RAW_FEEDBACK_MARKER,
            RAW_LIFEMODEL_VALUE,
            RAW_LIFEMODEL_DESCRIPTION,
            RAW_EXISTING_RULE,
            "W83_RAW_CONVERSATION_INFERENCE_SECRET",
            "用户偏好的表达方式",
        ] {
            assert!(
                !response_dump.contains(forbidden),
                "read-only feedback evolution report leaked raw marker {forbidden}"
            );
        }
    }
}
