use serde::{Deserialize, Serialize};

/// Provider attempt certainty shared by the canonical Chat and Work runtimes.
/// It describes an adapter invocation only; it never owns Task or Turn state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderInvocationState {
    #[default]
    NotAttempted,
    Started,
    Completed,
    Failed,
    LocallyAborted,
    RemoteUnknown,
    Invalid,
}

impl ProviderInvocationState {
    pub(crate) fn observed_adapter_start(self) -> bool {
        matches!(
            self,
            Self::Started
                | Self::Completed
                | Self::Failed
                | Self::LocallyAborted
                | Self::RemoteUnknown
        )
    }
}
