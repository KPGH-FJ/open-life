use openlife_core::life_model::LifeModel;
use openlife_core::router::RouterStatus;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Success,
    Error,
    Pending,
    Blocked,
    NeedsConfirmation,
}

#[derive(Clone, serde::Serialize)]
pub struct ToolCallResult {
    pub name: String,
    pub arguments: serde_json::Value,
    pub sanitized_arguments: Option<serde_json::Value>,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub permission_level: String,
    pub status: ToolCallStatus,
    pub requires_confirmation: bool,
    pub pii_found: bool,
    pub privacy_warnings: Vec<String>,
    pub action_id: Option<String>,
    pub run_id: Option<String>,
    pub permission_decision: Option<String>,
}

#[derive(serde::Serialize)]
pub struct SendMessageResult {
    pub reply: String,
    pub reasoning_trace: openlife_core::agent::ReasoningTrace,
    pub tool_calls: Vec<ToolCallResult>,
    pub run_id: Option<String>,
}

#[derive(serde::Serialize)]
pub struct BuilderCompletion {
    pub identity: f32,
    pub goals: f32,
    pub capabilities: f32,
    pub state: f32,
    pub overall: f32,
    pub lowest_dimension: Option<String>,
}

#[derive(serde::Serialize)]
pub struct SystemDiagnostics {
    pub router: RouterStatus,
    pub mcp_server_count: usize,
    pub mcp_tool_count: usize,
    pub mcp_recent_audit_count: usize,
    pub mcp_recent_pii_count: usize,
    pub memory_chunk_count: usize,
    pub vector_corrupt_embedding_count: usize,
    pub unfinished_builder_sessions: usize,
    pub pending_builder_review_sessions: usize,
    pub ollama_online: bool,
    pub local_model: String,
    pub resolved_local_model: Option<String>,
    pub prefer_local_model: bool,
    pub cloud_api_configured: bool,
    pub cloud_provider: String,
    pub cloud_api_validated: bool,
    pub cloud_api_last_error: Option<String>,
    pub chat_ready: bool,
    pub readiness_issues: Vec<String>,
    pub data_dir: String,
    pub active_data_dir: String,
    pub legacy_data_dir: Option<String>,
    pub database_status: String,
    pub startup_warnings: Vec<String>,
    pub snapshot_count: usize,
    pub life_model_ready: bool,
    pub app_version: String,
    pub model_empty: bool,
    pub chat_session_count: usize,
    pub onboarding_completed: bool,
    pub beta_ready: bool,
    pub beta_readiness_issues: Vec<String>,
    pub builder_completion: BuilderCompletion,
    pub agent_run_count: usize,
    pub agent_run_store_status: String,
    pub pending_proposal_count: usize,
    pub high_risk_pending_proposal_count: usize,
    pub proposal_store_status: String,
}

pub fn preview_text(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

pub fn included_life_model_sections(life_model: &LifeModel) -> Vec<String> {
    if life_model.is_effectively_empty() {
        Vec::new()
    } else {
        vec![
            "identity".to_string(),
            "goals".to_string(),
            "capabilities".to_string(),
            "state".to_string(),
        ]
    }
}

pub fn agent_actions_to_tool_call_results(
    actions: &[openlife_core::agent::AgentAction],
    run_id: &str,
) -> Vec<ToolCallResult> {
    actions
        .iter()
        .map(|action| {
            let output = action.output.as_ref().and_then(|value| {
                value
                    .get("text")
                    .and_then(|text| text.as_str())
                    .map(ToString::to_string)
                    .or_else(|| value.as_str().map(ToString::to_string))
            });
            ToolCallResult {
                name: action.target.clone().unwrap_or_default(),
                arguments: action.input.clone(),
                sanitized_arguments: None,
                success: matches!(
                    action.status.as_str(),
                    "succeeded" | "completed" | "success"
                ),
                output,
                error: action.error.clone(),
                permission_level: action
                    .tool_scope
                    .as_ref()
                    .map(|scope| scope.risk_level.clone())
                    .unwrap_or_else(|| "low".to_string()),
                status: match action.status.as_str() {
                    "success" | "succeeded" | "completed" => ToolCallStatus::Success,
                    "needs_confirmation" => ToolCallStatus::NeedsConfirmation,
                    "blocked" => ToolCallStatus::Blocked,
                    _ => ToolCallStatus::Error,
                },
                requires_confirmation: action.status == "needs_confirmation",
                pii_found: false,
                privacy_warnings: Vec::new(),
                action_id: Some(action.id.clone()),
                run_id: Some(run_id.to_string()),
                permission_decision: action.permission_decision.clone(),
            }
        })
        .collect()
}
