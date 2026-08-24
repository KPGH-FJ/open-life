use crate::AppState;
use chrono::{DateTime, Utc};
use openlife_core::{
    agent::{
        EvidenceRef, EvidenceSensitivity, EvidenceSource, ViewModelEnvelope, ViewModelStatus,
        ViewModelWarning, ViewModelWarningSeverity,
    },
    tool_permissions::{ToolPermissionPolicy, ToolPermissionRecord},
};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

const TOOL_PERMISSION_STORE: &str = "ToolPermissionStore";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolPermissionViewModel {
    pub items: Vec<ToolPermissionViewModelItem>,
    pub total_count: usize,
    pub active_count: usize,
    pub revocable_count: usize,
    pub contract_limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolPermissionViewModelItem {
    pub id: String,
    pub tool_name: String,
    pub source: String,
    pub risk_level: String,
    pub action_type: String,
    pub policy: ToolPermissionPolicy,
    pub lifecycle_state: ToolPermissionLifecycleState,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub consumed_at: Option<String>,
    pub revocable: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolPermissionLifecycleState {
    Active,
    Consumed,
    Expired,
}

#[tauri::command]
pub async fn get_tool_permission_view_model(
    state: State<'_, Arc<AppState>>,
) -> Result<ViewModelEnvelope<ToolPermissionViewModel>, String> {
    get_tool_permission_view_model_with_state(state.inner()).await
}

pub(crate) async fn get_tool_permission_view_model_with_state(
    state: &Arc<AppState>,
) -> Result<ViewModelEnvelope<ToolPermissionViewModel>, String> {
    if let Err(error) = state
        .persistence_coordinator
        .require_trusted_read(TOOL_PERMISSION_STORE)
    {
        return Ok(unavailable_envelope(error.to_string()));
    }

    let records = {
        let store = state.tool_permission_store.lock().await;
        match store.list() {
            Ok(records) => records,
            Err(error) => return Ok(unavailable_envelope(error.to_string())),
        }
    };
    Ok(envelope_for_records(records, Utc::now()))
}

pub(crate) fn envelope_for_records(
    records: Vec<ToolPermissionRecord>,
    now: DateTime<Utc>,
) -> ViewModelEnvelope<ToolPermissionViewModel> {
    let items = records
        .into_iter()
        .map(|record| item_for_record(record, now))
        .collect::<Vec<_>>();
    let active_count = items
        .iter()
        .filter(|item| item.lifecycle_state == ToolPermissionLifecycleState::Active)
        .count();
    let revocable_count = items.iter().filter(|item| item.revocable).count();
    let model = ToolPermissionViewModel {
        total_count: items.len(),
        active_count,
        revocable_count,
        items,
        contract_limitations: vec![
            "This surface projects canonical permission metadata only; it never grants a capability or reconstructs policy in the frontend.".into(),
            "One-time reviewed permissions are execution-bound and cannot be revoked as reusable grants from Settings.".into(),
        ],
    };
    let status = if model.items.is_empty() {
        ViewModelStatus::Empty
    } else {
        ViewModelStatus::Ready
    };
    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(model));
    envelope.last_updated_at = Some(now.to_rfc3339());
    envelope.evidence_refs = vec![source_ref()];
    envelope
}

fn item_for_record(
    record: ToolPermissionRecord,
    now: DateTime<Utc>,
) -> ToolPermissionViewModelItem {
    let lifecycle_state = if record.consumed_at.is_some() {
        ToolPermissionLifecycleState::Consumed
    } else if record
        .expires_at
        .is_some_and(|expires_at| expires_at <= now)
    {
        ToolPermissionLifecycleState::Expired
    } else {
        ToolPermissionLifecycleState::Active
    };
    let revocable = lifecycle_state == ToolPermissionLifecycleState::Active
        && record.policy != ToolPermissionPolicy::AllowOnce;
    ToolPermissionViewModelItem {
        id: record.id,
        tool_name: record.tool_name,
        source: record.source,
        risk_level: record.risk_level,
        action_type: record.action_type,
        policy: record.policy,
        lifecycle_state,
        created_at: record.created_at.to_rfc3339(),
        expires_at: record.expires_at.map(|value| value.to_rfc3339()),
        consumed_at: record.consumed_at.map(|value| value.to_rfc3339()),
        revocable,
    }
}

fn source_ref() -> EvidenceRef {
    EvidenceRef {
        id: "tool_permission_store".into(),
        label: "Canonical tool permission store".into(),
        source: EvidenceSource::BackendReadModel,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
    }
}

fn unavailable_envelope(reason: String) -> ViewModelEnvelope<ToolPermissionViewModel> {
    let mut envelope = ViewModelEnvelope::backend_read_model(ViewModelStatus::Error, None);
    envelope.last_updated_at = Some(Utc::now().to_rfc3339());
    envelope.evidence_refs = vec![source_ref()];
    envelope.warnings = vec![ViewModelWarning {
        code: "tool_permission_store_unavailable".into(),
        message: format!(
            "ToolPermissionViewModel could not read canonical permission state: {reason}"
        ),
        severity: ViewModelWarningSeverity::Warning,
        evidence_refs: vec![source_ref()],
    }];
    envelope
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        id: &str,
        policy: ToolPermissionPolicy,
        expires_at: Option<DateTime<Utc>>,
        consumed_at: Option<DateTime<Utc>>,
    ) -> ToolPermissionRecord {
        ToolPermissionRecord {
            id: id.into(),
            tool_name: "web.search".into(),
            source: "builtin".into(),
            risk_level: "medium".into(),
            action_type: "network".into(),
            policy,
            created_at: Utc::now(),
            expires_at,
            consumed_at,
        }
    }

    #[test]
    fn projection_keeps_reusable_grants_revocable_but_not_one_time_or_inactive_records() {
        let now = Utc::now();
        let envelope = envelope_for_records(
            vec![
                record(
                    "persistent",
                    ToolPermissionPolicy::AllowUntilRevoked,
                    None,
                    None,
                ),
                record("once", ToolPermissionPolicy::AllowOnce, None, None),
                record(
                    "expired",
                    ToolPermissionPolicy::Deny,
                    Some(now - chrono::Duration::seconds(1)),
                    None,
                ),
                record("consumed", ToolPermissionPolicy::AllowOnce, None, Some(now)),
            ],
            now,
        );

        let model = envelope.data.expect("permission model");
        assert_eq!(model.total_count, 4);
        assert_eq!(model.active_count, 2);
        assert_eq!(model.revocable_count, 1);
        assert!(model.items[0].revocable);
        assert!(!model.items[1].revocable);
        assert_eq!(
            model.items[2].lifecycle_state,
            ToolPermissionLifecycleState::Expired
        );
        assert_eq!(
            model.items[3].lifecycle_state,
            ToolPermissionLifecycleState::Consumed
        );
    }
}
