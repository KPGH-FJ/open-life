use crate::AppState;
use std::sync::Arc;

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn test_app_state() -> Arc<AppState> {
    crate::main_chat_eval_state::build_isolated_main_chat_eval_state()
}
