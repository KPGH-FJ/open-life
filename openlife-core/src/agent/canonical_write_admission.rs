use std::error::Error;
use std::fmt;

/// Metadata-only request to enter a canonical mutation boundary.
///
/// The request deliberately carries only a bounded domain and an opaque object
/// reference. User-authored bodies, tool arguments, and proposal payloads must
/// never be copied into execution-epoch facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalWriteAdmissionRequest {
    pub domain: String,
    pub object_ref: String,
}

impl CanonicalWriteAdmissionRequest {
    pub fn new(domain: impl Into<String>, object_ref: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            object_ref: object_ref.into(),
        }
    }
}

/// A fail-closed rejection from the execution owner that linearizes canonical
/// writes against cancellation or another terminal transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalWriteAdmissionRejection {
    reason_code: String,
}

impl CanonicalWriteAdmissionRejection {
    pub fn new(reason_code: impl Into<String>) -> Self {
        let reason_code = reason_code.into();
        let reason_code = if !reason_code.is_empty()
            && reason_code.len() <= 96
            && reason_code.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.')
            }) {
            reason_code
        } else {
            "invalid_admission_rejection_reason".to_string()
        };
        Self { reason_code }
    }

    pub fn reason_code(&self) -> &str {
        &self.reason_code
    }
}

impl fmt::Display for CanonicalWriteAdmissionRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "canonical_write_admission_rejected:{}",
            self.reason_code
        )
    }
}

impl Error for CanonicalWriteAdmissionRejection {}

/// Scope-owned RAII permit for one canonical mutation attempt.
///
/// Dropping a permit without an explicit terminal method must remain an
/// observable `unknown` outcome in the execution owner. Implementations own
/// that drop behavior; core callers must explicitly report committed, failed,
/// or a successful no-op (for example an idempotent proposal reuse).
pub trait CanonicalWritePermit {
    fn finish_committed(self: Box<Self>);
    fn finish_failed(self: Box<Self>);
    fn finish_noop(self: Box<Self>);
}

/// Execution-owner admission authority used by canonical gateways.
///
/// This trait lives in `openlife-core` so product runtimes can supply their
/// cancellation epoch without introducing a core -> Tauri dependency.
pub trait CanonicalWriteAdmission: Send + Sync {
    fn acquire(
        &self,
        request: CanonicalWriteAdmissionRequest,
    ) -> Result<Box<dyn CanonicalWritePermit>, CanonicalWriteAdmissionRejection>;
}

/// Explicit deterministic-eval fixture, visible only inside the `agent`
/// module tree. It cannot be supplied by Tauri or another product consumer.
pub(super) struct DeterministicFixtureCanonicalWriteAdmission;

struct FixtureCanonicalWritePermit;

impl CanonicalWritePermit for FixtureCanonicalWritePermit {
    fn finish_committed(self: Box<Self>) {}

    fn finish_failed(self: Box<Self>) {}

    fn finish_noop(self: Box<Self>) {}
}

impl CanonicalWriteAdmission for DeterministicFixtureCanonicalWriteAdmission {
    fn acquire(
        &self,
        _request: CanonicalWriteAdmissionRequest,
    ) -> Result<Box<dyn CanonicalWritePermit>, CanonicalWriteAdmissionRejection> {
        Ok(Box::new(FixtureCanonicalWritePermit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejection_reason_cannot_copy_user_authored_content() {
        let rejection = CanonicalWriteAdmissionRejection::new(
            "please persist this user-authored sentence and secret 1234",
        );

        assert_eq!(
            rejection.reason_code(),
            "invalid_admission_rejection_reason"
        );
        assert!(!rejection.to_string().contains("secret 1234"));
    }
}
