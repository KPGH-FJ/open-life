/// Callback trait for streaming agent loop execution.
/// Allows callers (e.g., Tauri shell) to receive real-time token chunks,
/// tool execution notifications, and status updates during AgentLoop execution.
#[async_trait::async_trait]
pub trait StreamingCallback: Send + Sync {
    /// A single token chunk from the model.
    async fn on_chunk(&self, chunk: &str, step: u32, phase: &str);
    /// A tool is about to be executed.
    async fn on_tool_start(&self, tool_name: &str, step: u32);
    /// A tool execution completed.
    async fn on_tool_result(&self, tool_name: &str, success: bool, step: u32);
    /// A proposal was generated.
    async fn on_proposal(&self, proposal_type: &str, proposal_id: &str);
    /// Status phase change.
    async fn on_status(&self, status: &str, message: &str, step: u32);
}
