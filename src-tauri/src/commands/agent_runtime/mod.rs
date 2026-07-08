use crate::AppState;
use std::sync::Arc;
use tauri::State;

mod plan_execute_product;

pub use plan_execute_product::*;

#[tauri::command]
pub async fn list_main_chat_skills(
    session_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::main_chat_skills_tools::MainChatSkillSummary>, String> {
    crate::main_chat_skills_tools::list_main_chat_skills_with_state(
        &state.inner().clone(),
        session_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn get_main_chat_skill_detail(
    skill_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_skills_tools::MainChatSkillDetail, String> {
    crate::main_chat_skills_tools::get_main_chat_skill_detail_with_state(
        &state.inner().clone(),
        &skill_id,
    )
    .await
}

#[tauri::command]
pub async fn select_main_chat_skill(
    session_id: String,
    skill_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_skills_tools::MainChatSelectedSkill, String> {
    crate::main_chat_skills_tools::select_main_chat_skill_with_state(
        &state.inner().clone(),
        &session_id,
        &skill_id,
    )
    .await
}

#[tauri::command]
pub async fn clear_main_chat_skill(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_skills_tools::MainChatSelectedSkill, String> {
    crate::main_chat_skills_tools::clear_main_chat_skill_with_state(
        &state.inner().clone(),
        &session_id,
    )
    .await
}

#[tauri::command]
pub async fn list_main_chat_tool_candidates(
    task_session_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_skills_tools::MainChatToolCandidateList, String> {
    crate::main_chat_skills_tools::list_main_chat_tool_candidates_with_state(
        &state.inner().clone(),
        task_session_id.as_deref(),
    )
    .await
}
