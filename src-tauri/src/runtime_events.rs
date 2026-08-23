//! Canonical runtime event transport shared by Chat and Work.
//!
//! These events describe provider lifecycle and user-visible terminal facts.
//! They are not an Agent strategy, task owner, or permission source.

use openlife_core::llm::{
    ProviderInvocationReceipt, ProviderInvocationStatus, ProviderLifecycleAdmissionFailure,
    ProviderPolicyReceiptEvidence,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeEvent {
    ProviderStarted {
        request_id: String,
        provider: String,
        model: String,
        started_at: chrono::DateTime<chrono::Utc>,
        policy_evidence: ProviderPolicyReceiptEvidence,
    },
    ProviderCompleted {
        request_id: String,
        provider: String,
        model: String,
        finished_at: chrono::DateTime<chrono::Utc>,
    },
    ProviderFailed {
        request_id: String,
        provider: String,
        model: String,
        finished_at: chrono::DateTime<chrono::Utc>,
        error_digest: String,
    },
    ProviderRemoteUnknown {
        request_id: String,
        provider: String,
        model: String,
        finished_at: chrono::DateTime<chrono::Utc>,
        reason_digest: String,
    },
    ProviderPolicyEvidence {
        request_id: String,
        policy_evidence: ProviderPolicyReceiptEvidence,
    },
    ProviderToken {
        session_id: String,
        request_id: String,
        chunk: String,
    },
    FinalAnswer {
        content_preview: String,
        content_chars: usize,
    },
    Blocker {
        code: String,
    },
}

pub trait RuntimeEventSink: Send {
    fn emit(&mut self, event: RuntimeEvent);

    /// Fallible only at the real provider adapter-start edge. Runtime wrappers
    /// use this synchronous seam to linearize start against cancellation before
    /// the HTTP adapter enters `.send()`; ordinary late events remain
    /// best-effort projections through `emit`.
    fn emit_provider_started(
        &mut self,
        request_id: String,
        provider: String,
        model: String,
        started_at: chrono::DateTime<chrono::Utc>,
        policy_evidence: ProviderPolicyReceiptEvidence,
    ) -> Result<(), ProviderLifecycleAdmissionFailure> {
        self.emit(RuntimeEvent::ProviderStarted {
            request_id: request_id.clone(),
            provider,
            model,
            started_at,
            policy_evidence: policy_evidence.clone(),
        });
        self.emit(RuntimeEvent::ProviderPolicyEvidence {
            request_id,
            policy_evidence,
        });
        Ok(())
    }

    fn events(&self) -> &[RuntimeEvent] {
        &[]
    }
}

#[derive(Debug, Default, Clone)]
pub struct BufferedRuntimeEventSink {
    events: Vec<RuntimeEvent>,
}

impl BufferedRuntimeEventSink {
    pub fn events(&self) -> &[RuntimeEvent] {
        &self.events
    }
}

impl RuntimeEventSink for BufferedRuntimeEventSink {
    fn emit(&mut self, event: RuntimeEvent) {
        self.events.push(event);
    }

    fn events(&self) -> &[RuntimeEvent] {
        &self.events
    }
}

pub(crate) fn emit_provider_receipt<S>(
    receipt: &ProviderInvocationReceipt,
    event_sink: &mut S,
) -> Result<(), String>
where
    S: RuntimeEventSink + ?Sized,
{
    if receipt.simulated {
        return Ok(());
    }
    let policy_evidence = receipt
        .policy_evidence
        .as_ref()
        .ok_or_else(|| "provider_receipt_policy_evidence_missing".to_string())?;
    let start_seen = event_sink.events().iter().any(|event| {
        matches!(
            event,
            RuntimeEvent::ProviderStarted {
                request_id,
                provider,
                model,
                started_at,
                policy_evidence: observed_policy_evidence,
            } if request_id == &receipt.request_id
                && provider == &receipt.provider
                && model == &receipt.model
                && started_at == &receipt.started_at
                && observed_policy_evidence == policy_evidence
        )
    });
    if !start_seen {
        return Err("provider_receipt_observed_start_missing".into());
    }
    let evidence_seen = event_sink.events().iter().any(|event| {
        matches!(
            event,
            RuntimeEvent::ProviderPolicyEvidence {
                request_id,
                policy_evidence: existing,
            } if request_id == &receipt.request_id && existing == policy_evidence
        )
    });
    if !evidence_seen {
        event_sink.emit(RuntimeEvent::ProviderPolicyEvidence {
            request_id: receipt.request_id.clone(),
            policy_evidence: policy_evidence.clone(),
        });
    }
    let terminal_seen = event_sink.events().iter().any(|event| {
        matches!(
            event,
            RuntimeEvent::ProviderCompleted { request_id, .. }
                | RuntimeEvent::ProviderFailed { request_id, .. }
                | RuntimeEvent::ProviderRemoteUnknown { request_id, .. }
                if request_id == &receipt.request_id
        )
    });
    if terminal_seen {
        return Ok(());
    }
    match receipt.status {
        ProviderInvocationStatus::Completed => event_sink.emit(RuntimeEvent::ProviderCompleted {
            request_id: receipt.request_id.clone(),
            provider: receipt.provider.clone(),
            model: receipt.model.clone(),
            finished_at: receipt.finished_at,
        }),
        ProviderInvocationStatus::Failed => event_sink.emit(RuntimeEvent::ProviderFailed {
            request_id: receipt.request_id.clone(),
            provider: receipt.provider.clone(),
            model: receipt.model.clone(),
            finished_at: receipt.finished_at,
            error_digest: receipt
                .error_digest
                .clone()
                .unwrap_or_else(|| "provider_error_digest_missing".into()),
        }),
        ProviderInvocationStatus::RemoteUnknown => {
            event_sink.emit(RuntimeEvent::ProviderRemoteUnknown {
                request_id: receipt.request_id.clone(),
                provider: receipt.provider.clone(),
                model: receipt.model.clone(),
                finished_at: receipt.finished_at,
                reason_digest: receipt
                    .error_digest
                    .clone()
                    .unwrap_or_else(|| "provider_remote_unknown_reason_digest_missing".into()),
            })
        }
    }
    Ok(())
}
