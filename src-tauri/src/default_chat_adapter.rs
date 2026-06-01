pub(crate) const LEGACY_STREAM_PATH: &str = "legacy_stream";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatAdapterRoute {
    pub(crate) current_mode: String,
    pub(crate) adapter_scaffold_present: bool,
    pub(crate) controlled_adapter_enabled: bool,
    pub(crate) automatic_migration_enabled: bool,
    pub(crate) default_send_path: String,
    pub(crate) start_stream_path: String,
    pub(crate) requires_separate_cutover_implementation: bool,
}

pub(crate) fn resolve_default_chat_adapter_route() -> DefaultChatAdapterRoute {
    DefaultChatAdapterRoute {
        current_mode: LEGACY_STREAM_PATH.into(),
        adapter_scaffold_present: true,
        controlled_adapter_enabled: false,
        automatic_migration_enabled: false,
        default_send_path: LEGACY_STREAM_PATH.into(),
        start_stream_path: LEGACY_STREAM_PATH.into(),
        requires_separate_cutover_implementation: true,
    }
}

pub(crate) fn ensure_default_chat_legacy_route(
    caller: &str,
    route: &DefaultChatAdapterRoute,
) -> Result<(), String> {
    let mut blockers = Vec::new();

    if route.current_mode != LEGACY_STREAM_PATH {
        blockers.push("current_mode_not_legacy_stream");
    }
    if route.controlled_adapter_enabled {
        blockers.push("controlled_adapter_enabled");
    }
    if route.automatic_migration_enabled {
        blockers.push("automatic_migration_enabled");
    }
    if route.default_send_path != LEGACY_STREAM_PATH {
        blockers.push("default_send_path_not_legacy_stream");
    }
    if route.start_stream_path != LEGACY_STREAM_PATH {
        blockers.push("start_stream_path_not_legacy_stream");
    }

    if blockers.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{caller} blocked by default Chat adapter route guard: {}",
            blockers.join(", ")
        ))
    }
}
