use crate::commands::diagnostics::get_system_diagnostics_with_state;
use crate::AppState;
use chrono::Utc;
use openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus;
use openlife_core::agent::{ProposalStatus, RiskLevel};
use openlife_core::tool_permissions::ToolPermissionPolicy;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeStateProjection {
    pub version: String,
    pub generated_at: String,
    pub pending: LifePendingProjection,
    pub readiness: LifeReadinessProjection,
    pub task_state: LifeTaskStateProjection,
    pub safe_mode: LifeSafeModeProjection,
    pub tool_permissions: LifeToolPermissionProjection,
    pub safe_paths: Vec<String>,
    pub surfaces: Vec<LifeSurfaceProjection>,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LifePendingProjection {
    pub pending_proposal_count: usize,
    pub edited_proposal_count: usize,
    pub total_review_required_count: usize,
    pub high_risk_review_required_count: usize,
    pub proposal_store_status: String,
    pub requires_user_action: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeReadinessProjection {
    pub chat_ready: bool,
    pub usage_ready: bool,
    pub life_model_ready: bool,
    pub model_empty: bool,
    pub pending_builder_review_sessions: usize,
    pub unfinished_builder_sessions: usize,
    pub database_status: String,
    pub readiness_issues: Vec<String>,
    pub usage_readiness_issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LifeTaskStateProjection {
    pub task_store_status: String,
    pub latest_task_id: Option<String>,
    pub latest_task_status: Option<String>,
    pub running_count: usize,
    pub waiting_permission_count: usize,
    pub blocked_count: usize,
    pub failed_count: usize,
    pub cancelled_count: usize,
    pub completed_count: usize,
    pub active_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeSafeModeProjection {
    pub active: bool,
    pub reason: String,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LifeToolPermissionProjection {
    pub total_count: usize,
    pub active_count: usize,
    pub consumed_count: usize,
    pub allow_count: usize,
    pub deny_count: usize,
    pub ask_every_time_count: usize,
    pub allow_once_count: usize,
    pub allow_until_revoked_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeSurfaceProjection {
    pub surface: String,
    pub pending_review_count: usize,
    pub edited_review_count: usize,
    pub total_review_required_count: usize,
    pub readiness_status: String,
    pub task_status: String,
    pub safe_mode_active: bool,
    pub waiting_permission_count: usize,
    pub active_tool_permission_count: usize,
}

#[tauri::command]
pub async fn get_life_state_projection(
    state: State<'_, Arc<AppState>>,
) -> Result<LifeStateProjection, String> {
    get_life_state_projection_with_state(state.inner()).await
}

pub(crate) async fn get_life_state_projection_with_state(
    state: &Arc<AppState>,
) -> Result<LifeStateProjection, String> {
    let diagnostics = get_system_diagnostics_with_state(state)
        .await
        .map_err(|err| err.to_string())?;
    let pending = build_pending_projection(state).await;
    let readiness = LifeReadinessProjection {
        chat_ready: diagnostics.chat_ready,
        usage_ready: diagnostics.usage_ready,
        life_model_ready: diagnostics.life_model_ready,
        model_empty: diagnostics.model_empty,
        pending_builder_review_sessions: diagnostics.pending_builder_review_sessions,
        unfinished_builder_sessions: diagnostics.unfinished_builder_sessions,
        database_status: diagnostics.database_status.clone(),
        readiness_issues: diagnostics.readiness_issues.clone(),
        usage_readiness_issues: diagnostics.usage_readiness_issues.clone(),
    };
    let safe_mode = build_safe_mode_projection(
        diagnostics.startup_warnings.clone(),
        diagnostics.vector_corrupt_embedding_count,
        Some(diagnostics.database_status.as_str()),
    );
    let task_state = build_task_state_projection(state).await;
    let tool_permissions = build_tool_permission_projection(state).await;
    let safe_paths = {
        let cfg = state.config.lock().await;
        cfg.system.safe_paths.clone()
    };
    let surfaces = build_surface_projection(
        &pending,
        &readiness,
        &task_state,
        &safe_mode,
        &tool_permissions,
    );

    Ok(LifeStateProjection {
        version: "life_state_projection_v1".into(),
        generated_at: Utc::now().to_rfc3339(),
        pending,
        readiness,
        task_state,
        safe_mode,
        tool_permissions,
        safe_paths,
        surfaces,
        source_refs: vec![
            "diagnostics".into(),
            "proposal_store:pending_and_edited".into(),
            "main_chat_agent_session_store".into(),
            "tool_permission_store".into(),
            "config:safe_paths".into(),
        ],
    })
}

async fn build_pending_projection(state: &Arc<AppState>) -> LifePendingProjection {
    let Some(store_arc) = state.proposal_store.as_ref() else {
        return LifePendingProjection {
            pending_proposal_count: 0,
            edited_proposal_count: 0,
            total_review_required_count: 0,
            high_risk_review_required_count: 0,
            proposal_store_status: "disabled".into(),
            requires_user_action: false,
        };
    };

    let store = store_arc.lock().await;
    let pending = store
        .list_proposals_filtered(Some(ProposalStatus::Pending), None, None, 200)
        .unwrap_or_default();
    let edited = store
        .list_proposals_filtered(Some(ProposalStatus::Edited), None, None, 200)
        .unwrap_or_default();
    let high_risk_pending = store
        .count_by_status_and_risk(ProposalStatus::Pending, Some(RiskLevel::High))
        .unwrap_or(0)
        .max(0) as usize;
    let high_risk_edited = store
        .count_by_status_and_risk(ProposalStatus::Edited, Some(RiskLevel::High))
        .unwrap_or(0)
        .max(0) as usize;
    pending_projection_from_counts(
        pending.len(),
        edited.len(),
        high_risk_pending.saturating_add(high_risk_edited),
        "ok",
    )
}

fn pending_projection_from_counts(
    pending_count: usize,
    edited_count: usize,
    high_risk_count: usize,
    proposal_store_status: &str,
) -> LifePendingProjection {
    let total_review_required_count = pending_count.saturating_add(edited_count);
    LifePendingProjection {
        pending_proposal_count: pending_count,
        edited_proposal_count: edited_count,
        total_review_required_count,
        high_risk_review_required_count: high_risk_count,
        proposal_store_status: proposal_store_status.into(),
        requires_user_action: total_review_required_count > 0,
    }
}

async fn build_task_state_projection(state: &Arc<AppState>) -> LifeTaskStateProjection {
    let Some(store_arc) = state.main_chat_agent_session_store.as_ref() else {
        return LifeTaskStateProjection {
            task_store_status: "disabled".into(),
            ..LifeTaskStateProjection::default()
        };
    };

    let store = store_arc.lock().await;
    let sessions = store.list_sessions(None, 200, 0).unwrap_or_default();
    let latest = sessions.first();
    let mut projection = LifeTaskStateProjection {
        task_store_status: "ok".into(),
        latest_task_id: latest.map(|session| session.id.clone()),
        latest_task_status: latest.map(|session| session.status.as_str().to_string()),
        ..LifeTaskStateProjection::default()
    };

    for session in sessions {
        match session.status {
            AgentTaskSessionStatus::Running => projection.running_count += 1,
            AgentTaskSessionStatus::WaitingPermission => projection.waiting_permission_count += 1,
            AgentTaskSessionStatus::Blocked => projection.blocked_count += 1,
            AgentTaskSessionStatus::Failed => projection.failed_count += 1,
            AgentTaskSessionStatus::Cancelled => projection.cancelled_count += 1,
            AgentTaskSessionStatus::Completed => projection.completed_count += 1,
        }
    }
    projection.active_count = projection
        .running_count
        .saturating_add(projection.waiting_permission_count);
    projection
}

fn build_safe_mode_projection(
    startup_warnings: Vec<String>,
    vector_corrupt_embedding_count: usize,
    database_status: Option<&str>,
) -> LifeSafeModeProjection {
    let active = !startup_warnings.is_empty()
        || vector_corrupt_embedding_count > 0
        || database_status == Some("degraded");
    let reason = if let Some(warning) = startup_warnings.first() {
        warning.clone()
    } else if vector_corrupt_embedding_count > 0 {
        format!(
            "检测到 {} 条损坏向量索引记录。",
            vector_corrupt_embedding_count
        )
    } else if database_status == Some("degraded") {
        "当前数据库处于降级模式，暂不建议继续高风险写入。".into()
    } else {
        "系统当前未处于 Safe Mode。".into()
    };

    let mut source_refs = Vec::new();
    if !startup_warnings.is_empty() {
        source_refs.push("diagnostics:startup_warnings".into());
    }
    if vector_corrupt_embedding_count > 0 {
        source_refs.push("diagnostics:vector_corrupt_embedding_count".into());
    }
    if database_status == Some("degraded") {
        source_refs.push("diagnostics:database_status".into());
    }

    LifeSafeModeProjection {
        active,
        reason,
        source_refs,
    }
}

async fn build_tool_permission_projection(state: &Arc<AppState>) -> LifeToolPermissionProjection {
    let store = state.tool_permission_store.lock().await;
    let records = store.list().unwrap_or_default();
    let now = Utc::now();
    let mut projection = LifeToolPermissionProjection {
        total_count: records.len(),
        ..LifeToolPermissionProjection::default()
    };

    for record in records {
        let active = record.consumed_at.is_none()
            && record
                .expires_at
                .map(|expires_at| expires_at > now)
                .unwrap_or(true);
        if active {
            projection.active_count += 1;
        } else {
            projection.consumed_count += 1;
        }

        match record.policy {
            ToolPermissionPolicy::Allow => projection.allow_count += 1,
            ToolPermissionPolicy::Deny => projection.deny_count += 1,
            ToolPermissionPolicy::AskEveryTime => projection.ask_every_time_count += 1,
            ToolPermissionPolicy::AllowOnce => projection.allow_once_count += 1,
            ToolPermissionPolicy::AllowUntilRevoked => projection.allow_until_revoked_count += 1,
        }
    }

    projection
}

fn build_surface_projection(
    pending: &LifePendingProjection,
    readiness: &LifeReadinessProjection,
    task_state: &LifeTaskStateProjection,
    safe_mode: &LifeSafeModeProjection,
    tool_permissions: &LifeToolPermissionProjection,
) -> Vec<LifeSurfaceProjection> {
    let readiness_status = if readiness.usage_ready {
        "ready"
    } else if readiness.chat_ready || readiness.life_model_ready {
        "partial"
    } else {
        "blocked"
    };
    let task_status = task_state
        .latest_task_status
        .clone()
        .unwrap_or_else(|| "idle".into());
    [
        "today",
        "mailbox",
        "chat",
        "companion",
        "life_model",
        "settings",
    ]
    .into_iter()
    .map(|surface| LifeSurfaceProjection {
        surface: surface.into(),
        pending_review_count: pending.pending_proposal_count,
        edited_review_count: pending.edited_proposal_count,
        total_review_required_count: pending.total_review_required_count,
        readiness_status: readiness_status.into(),
        task_status: task_status.clone(),
        safe_mode_active: safe_mode.active,
        waiting_permission_count: task_state.waiting_permission_count,
        active_tool_permission_count: tool_permissions.active_count,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_projection_counts_edited_as_user_action() {
        let projection = pending_projection_from_counts(2, 1, 1, "ok");

        assert_eq!(projection.pending_proposal_count, 2);
        assert_eq!(projection.edited_proposal_count, 1);
        assert_eq!(projection.total_review_required_count, 3);
        assert!(projection.requires_user_action);
    }

    #[test]
    fn safe_mode_projection_uses_same_product_sources() {
        let projection = build_safe_mode_projection(Vec::new(), 3, Some("ok"));

        assert!(projection.active);
        assert_eq!(
            projection.source_refs,
            vec!["diagnostics:vector_corrupt_embedding_count"]
        );
    }
}
