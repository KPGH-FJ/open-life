use crate::{errors::AppError, AppState};
use openlife_core::mcp_audit::{AuditExport, McpLogEntry};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

/// Single product seam for MCP audit reads.
///
/// This slice deliberately preserves the pre-D065 behavior behind one concrete
/// instance. The RED contract owns the semantic change: composite trust,
/// strict decrypt failure and typed product truth are implemented only by the
/// follow-up production slice.
#[derive(Default)]
pub(crate) struct McpAuditReadGateway {
    #[cfg(test)]
    diagnostics_calls: AtomicUsize,
    #[cfg(test)]
    list_calls: AtomicUsize,
    #[cfg(test)]
    export_calls: AtomicUsize,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct McpAuditReadGatewayCallCounts {
    pub diagnostics: usize,
    pub list: usize,
    pub export: usize,
}

impl McpAuditReadGateway {
    pub(crate) async fn diagnostic_counts(&self, state: &AppState) -> (usize, usize) {
        #[cfg(test)]
        self.diagnostics_calls.fetch_add(1, Ordering::Relaxed);

        if state
            .persistence_coordinator
            .require_trusted_read("McpAuditStore")
            .is_ok()
        {
            let audit = state.mcp_audit_store.lock().await;
            match audit.list_logs(50) {
                Ok(logs) => {
                    let pii_count = logs.iter().filter(|log| log.pii_found).count();
                    (logs.len(), pii_count)
                }
                Err(_) => (0, 0),
            }
        } else {
            (0, 0)
        }
    }

    pub(crate) async fn list_logs(
        &self,
        state: &AppState,
        limit: usize,
    ) -> Result<Vec<McpLogEntry>, AppError> {
        #[cfg(test)]
        self.list_calls.fetch_add(1, Ordering::Relaxed);

        let store = state.mcp_audit_store.lock().await;
        store.list_logs(limit).map_err(AppError::from)
    }

    pub(crate) async fn export_logs(
        &self,
        state: &AppState,
        days: i64,
    ) -> Result<AuditExport, AppError> {
        #[cfg(test)]
        self.export_calls.fetch_add(1, Ordering::Relaxed);

        let store = state.mcp_audit_store.lock().await;
        store.export_logs(days).map_err(AppError::from)
    }

    #[cfg(test)]
    pub(crate) fn call_counts(&self) -> McpAuditReadGatewayCallCounts {
        McpAuditReadGatewayCallCounts {
            diagnostics: self.diagnostics_calls.load(Ordering::Relaxed),
            list: self.list_calls.load(Ordering::Relaxed),
            export: self.export_calls.load(Ordering::Relaxed),
        }
    }
}
