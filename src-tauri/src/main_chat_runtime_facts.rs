//! Provider-route diagnostics for the canonical Chat/Work runtime.
//!
//! Execution truth lives on canonical Conversation and Task items. This module
//! intentionally exposes only a settings projection; it does not reconstruct
//! an execution lifecycle from retired run or event stores.

mod provider_route;

pub(crate) use provider_route::{build_settings_runtime_route_evidence, RuntimeRouteEvidence};
