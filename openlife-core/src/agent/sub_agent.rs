//! SubAgentRuntime — executes sub-agents in `call_as_tool` mode.
//!
//! The SubAgentRuntime enforces:
//! - Parent-child AgentRun linkage
//! - Isolated context policy
//! - Tool policy (only allowed tools from AgentSpec)
//! - No direct writes, no shell, no external side effects for default sub-agents
//! - Result is returned as a structured observation to the parent run.

use crate::agent::event_store::AgentRunEventStore;
use crate::agent::store::AgentRunStore;
use crate::agent::types::{
    AgentEventActor, AgentObservation, AgentRun, AgentRunEvent, AgentRunEventType, AgentRunStatus,
    AgentSpec, AgentTaskKind, DelegationMode, SubAgentSpec,
};
use anyhow::Result;
use chrono::Utc;

/// Explicit outcome from a sub-agent execution.
/// Replaces the previous `result_text.contains("error")` heuristic.
#[derive(Debug, Clone)]
pub struct SubAgentExecutionOutcome {
    pub success: bool,
    pub output: String,
    pub structured_output: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl SubAgentExecutionOutcome {
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            structured_output: None,
            error: None,
        }
    }

    pub fn with_structured(mut self, structured: serde_json::Value) -> Self {
        self.structured_output = Some(structured);
        self
    }

    pub fn err(output: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: output.into(),
            structured_output: None,
            error: Some(error.into()),
        }
    }
}

/// Result from executing a sub-agent.
#[derive(Debug, Clone)]
pub struct SubAgentResult {
    pub child_run: AgentRun,
    pub observation: AgentObservation,
    pub success: bool,
    pub structured_output: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Runtime for executing sub-agents under governance constraints.
pub struct SubAgentRuntime {
    agent_run_store: AgentRunStore,
    event_store: Option<AgentRunEventStore>,
}

impl SubAgentRuntime {
    pub fn new(
        agent_run_store: AgentRunStore,
        event_store: Option<AgentRunEventStore>,
    ) -> Self {
        Self {
            agent_run_store,
            event_store,
        }
    }

    // ── Spec validation ────────────────────────────────────────────────

    /// Validate that a sub-agent spec meets the minimum safety requirements
    /// for call_as_tool execution.
    pub fn validate_spec(spec: &SubAgentSpec) -> Result<()> {
        if spec.delegation_mode != DelegationMode::CallAsTool {
            return Err(anyhow::anyhow!(
                "expected DelegationMode::CallAsTool, got {:?}",
                spec.delegation_mode
            ));
        }
        if !spec.spec.read_only {
            return Err(anyhow::anyhow!(
                "sub-agent spec '{}' must be read-only for call_as_tool",
                spec.spec.name
            ));
        }
        // Verify no write tools in allowed_tools
        for tool in &spec.spec.allowed_tools {
            if spec.spec.is_tool_denied(tool) {
                return Err(anyhow::anyhow!(
                    "tool '{}' is both allowed and denied in spec '{}'",
                    tool,
                    spec.spec.name
                ));
            }
        }
        if spec.spec.can_generate_proposals {
            // Allow proposal generation only if explicitly enabled and
            // the spec is marked read_only (proposals are not writes).
            // This is a warning-level condition, not an error.
        }
        Ok(())
    }

    /// Derive the sub-agent's toolset_allowlist from its spec.
    /// Used to configure the child AgentLoop.
    pub fn derive_allowlist(spec: &AgentSpec) -> Vec<String> {
        if spec.allowed_tools.is_empty() {
            // No allowlist means use defaults: all non-denied read tools
            Vec::new()
        } else {
            spec.allowed_tools.clone()
        }
    }

    // ── Execution ──────────────────────────────────────────────────────

    /// Execute a sub-agent in `call_as_tool` mode.
    ///
    /// Validates the spec, creates a child AgentRun linked to the parent,
    /// builds the child run with tool policy and context isolation applied,
    /// writes the complete run to the store, and returns a SubAgentResult.
    pub fn execute_call_as_tool(
        &self,
        spec: &SubAgentSpec,
        parent_run: &AgentRun,
        task_description: &str,
        outcome: &SubAgentExecutionOutcome,
    ) -> Result<SubAgentResult> {
        Self::validate_spec(spec)?;

        // ── Build complete child run ──────────────────────────────────
        let child_run = Self::create_child_run(
            parent_run,
            &spec.spec,
            task_description,
            outcome,
        );

        // ── Create run + record event in one logical step ─────────────
        self.agent_run_store.create_run(&child_run)?;

        self.record_event(
            &child_run.id,
            AgentRunEventType::RunCreated,
            AgentEventActor::SubAgent(spec.spec.role.to_string()),
            format!("Sub-agent '{}' run created", spec.spec.name),
            serde_json::json!({
                "parent_run_id": parent_run.id,
                "role": spec.spec.role.to_string(),
                "delegation_mode": "call_as_tool",
                "read_only": spec.spec.read_only,
            }),
        );

        // ── Tool policy ───────────────────────────────────────────────
        let tool_policy_note = build_tool_policy_note(&spec.spec);
        let allowlist = Self::derive_allowlist(&spec.spec);

        // ── Context isolation ─────────────────────────────────────────
        let context_note = if spec.isolated_context {
            "Sub-agent context is isolated from parent context.".to_string()
        } else {
            "Sub-agent inherits parent context.".to_string()
        };

        let allowlist_display = if allowlist.is_empty() {
            "(default)".to_string()
        } else {
            allowlist.join(", ")
        };

        // ── Build observation (parent trace) ──────────────────────────
        let observation = AgentObservation {
            id: format!("sub-agent-obs-{}", uuid::Uuid::new_v4()),
            action_id: None,
            content: format!(
                "[Sub-Agent: {}] {} Task: {}\n\nPolicy: {}\nAllowed tools: {}\nContext: {}\n\nResult: {}",
                spec.spec.name,
                spec.spec.purpose,
                task_description,
                tool_policy_note,
                allowlist_display,
                context_note,
                outcome.output,
            ),
            source: format!("sub_agent:{}", spec.spec.role),
            structured_result: outcome.structured_output.clone(),
            timestamp: Utc::now(),
        };

        self.record_event(
            &parent_run.id,
            AgentRunEventType::ObservationCreated,
            AgentEventActor::SubAgent(spec.spec.role.to_string()),
            format!(
                "Sub-agent '{}' completed: {}",
                spec.spec.name,
                if outcome.success { "ok" } else { "failed" }
            ),
            serde_json::json!({
                "child_run_id": child_run.id,
                "success": outcome.success,
                "parent_run_id": parent_run.id,
            }),
        );

        let mut result = SubAgentResult {
            child_run,
            observation,
            success: outcome.success,
            structured_output: outcome.structured_output.clone(),
            error: outcome.error.clone(),
        };
        // Also update error on result if present
        if let Some(ref e) = outcome.error {
            result.error = Some(e.clone());
        }
        Ok(result)
    }

    /// Create a complete child run (write-once pattern, no separate update).
    fn create_child_run(
        parent_run: &AgentRun,
        spec: &AgentSpec,
        task_description: &str,
        outcome: &SubAgentExecutionOutcome,
    ) -> AgentRun {
        AgentRun {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: format!("{}-child", parent_run.task_id),
            session_id: parent_run.session_id.clone(),
            status: if outcome.success {
                AgentRunStatus::Completed
            } else {
                AgentRunStatus::Failed
            },
            kind: AgentTaskKind::ToolExecution,
            user_input: Some(format!(
                "[Sub-Agent {}] {}",
                spec.role,
                task_description
            )),
            context_summary: None,
            model_route: None,
            output_preview: Some(truncate(&outcome.output, 200)),
            error: outcome.error.as_ref().map(|e| crate::agent::types::AgentRunError {
                message: e.clone(),
                phase: "sub_agent".into(),
                recoverable: false,
            }),
            generated_proposals: Vec::new(),
            actions: Vec::new(),
            observations: Vec::new(),
            reasoning_strategy: Some("sub_agent_direct".into()),
            reasoning_trace: None,
            warnings: Vec::new(),
            status_updates: Vec::new(),
            step_count: 0,
            tool_call_count: 0,
            deleted_at: None,
            delete_reason: None,
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
        }
    }

    fn record_event(
        &self,
        run_id: &str,
        event_type: AgentRunEventType,
        actor: AgentEventActor,
        summary: impl Into<String>,
        payload: serde_json::Value,
    ) {
        if let Some(ref store) = self.event_store {
            let event = AgentRunEvent::new(run_id, event_type, actor, summary, payload);
            if let Err(e) = store.append_event(&event) {
                eprintln!("[SubAgentRuntime] Failed to record event: {}", e);
            }
        }
    }

    pub fn agent_run_store(&self) -> &AgentRunStore {
        &self.agent_run_store
    }
}

/// Build a human-readable tool policy note from an AgentSpec.
fn build_tool_policy_note(spec: &AgentSpec) -> String {
    let mut parts = Vec::new();
    if spec.read_only {
        parts.push("read-only mode (no writes)".to_string());
    }
    if !spec.allowed_tools.is_empty() {
        parts.push(format!(
            "allowed tools: [{}]",
            spec.allowed_tools.join(", ")
        ));
    }
    if !spec.denied_tools.is_empty() {
        parts.push(format!("denied tools: [{}]", spec.denied_tools.join(", ")));
    }
    if spec.allowed_tools.is_empty() && spec.denied_tools.is_empty() && !spec.read_only {
        parts.push("no explicit tool restrictions".to_string());
    }
    if parts.is_empty() {
        "tool policy not configured".to_string()
    } else {
        parts.join("; ")
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_len {
        s.to_string()
    } else {
        chars.into_iter().take(max_len).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::event_store::AgentRunEventStore;
    use crate::agent::store::AgentRunStore;

    fn create_test_parent_run() -> AgentRun {
        AgentRun::new_chat_run("session-001", "Please analyze this project")
    }

    fn create_test_sub_spec() -> SubAgentSpec {
        let spec = AgentSpec::new(
            crate::agent::types::AgentRoleKind::CodebaseExplorer,
            "CodebaseExplorer",
            "Explore codebase and return findings",
        )
        .with_allowed_tools(vec!["file.read".into(), "web.search".into()])
        .with_read_only();

        SubAgentSpec::new(spec, DelegationMode::CallAsTool)
            .with_deadline(30)
    }

    // ── Spec validation tests ──────────────────────────────────────────

    #[test]
    fn test_validate_spec_requires_read_only() {
        let spec = AgentSpec::new(
            crate::agent::types::AgentRoleKind::CodebaseExplorer,
            "Explorer",
            "Explore",
        );
        // NOT read_only — should fail validation
        let sub = SubAgentSpec::new(spec, DelegationMode::CallAsTool);
        let result = SubAgentRuntime::validate_spec(&sub);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("read-only"));
    }

    #[test]
    fn test_validate_spec_requires_correct_mode() {
        let spec = AgentSpec::new(
            crate::agent::types::AgentRoleKind::Reviewer,
            "Reviewer",
            "Review",
        )
        .with_read_only();
        let sub = SubAgentSpec::new(spec, DelegationMode::Review);
        let result = SubAgentRuntime::validate_spec(&sub);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CallAsTool"));
    }

    #[test]
    fn test_validate_spec_passes_read_only_call_as_tool() {
        let spec = create_test_sub_spec();
        let result = SubAgentRuntime::validate_spec(&spec);
        assert!(result.is_ok(), "valid spec should pass: {:?}", result);
    }

    // ── Execution tests ────────────────────────────────────────────────

    #[test]
    fn test_call_as_tool_creates_child_run_linked_to_parent() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, Some(event_store.clone()));

        let parent_run = create_test_parent_run();
        let spec = create_test_sub_spec();
        let outcome = SubAgentExecutionOutcome::ok("Found 12 Rust files in src/");

        let result = runtime
            .execute_call_as_tool(&spec, &parent_run, "Find all Rust files", &outcome)
            .unwrap();

        let child = runtime
            .agent_run_store()
            .get_run(&result.child_run.id)
            .unwrap()
            .unwrap();
        assert_eq!(child.status, AgentRunStatus::Completed);
        assert!(result.success);
        assert!(result
            .observation
            .content
            .contains("Found 12 Rust files"));
    }

    #[test]
    fn test_call_as_tool_error_outcome() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, None);

        let parent_run = create_test_parent_run();
        let spec = create_test_sub_spec();
        let outcome = SubAgentExecutionOutcome::err(
            "Failed to read: permission denied",
            "permission_denied",
        );

        let result = runtime
            .execute_call_as_tool(&spec, &parent_run, "Read file", &outcome)
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.child_run.status, AgentRunStatus::Failed);
        assert!(result.error.is_some());
        assert_eq!(result.error.as_deref(), Some("permission_denied"));
        assert!(result.child_run.error.is_some());
    }

    #[test]
    fn test_call_as_tool_records_events() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, Some(event_store.clone()));

        let parent_run = create_test_parent_run();
        let spec = create_test_sub_spec();
        let outcome = SubAgentExecutionOutcome::ok("Done");

        let result = runtime
            .execute_call_as_tool(&spec, &parent_run, "List files", &outcome)
            .unwrap();

        let child_events = event_store
            .list_events_by_run(&result.child_run.id)
            .unwrap();
        assert_eq!(child_events.len(), 1);
        assert_eq!(child_events[0].event_type, AgentRunEventType::RunCreated);

        let parent_events = event_store.list_events_by_run(&parent_run.id).unwrap();
        assert_eq!(parent_events.len(), 1);
        assert_eq!(
            parent_events[0].event_type,
            AgentRunEventType::ObservationCreated
        );
    }

    #[test]
    fn test_sub_agent_result_includes_tool_policy() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, None);
        let parent_run = create_test_parent_run();
        let spec = create_test_sub_spec();
        let outcome = SubAgentExecutionOutcome::ok("Analysis result");

        let result = runtime
            .execute_call_as_tool(&spec, &parent_run, "Analyze", &outcome)
            .unwrap();

        assert!(result.observation.content.contains("read-only mode"));
        assert!(result.observation.content.contains("allowed tools"));
        assert!(result.observation.content.contains("file.read"));
    }

    #[test]
    fn test_sub_agent_context_isolation() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, None);
        let parent_run = create_test_parent_run();

        let spec = create_test_sub_spec();
        assert!(spec.isolated_context);

        let outcome = SubAgentExecutionOutcome::ok("Result");
        let result = runtime
            .execute_call_as_tool(&spec, &parent_run, "Task", &outcome)
            .unwrap();

        assert!(result.observation.content.contains("isolated from parent"));
    }

    #[test]
    fn test_sub_agent_with_inherited_context() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, None);
        let parent_run = create_test_parent_run();

        let base_spec = AgentSpec::new(
            crate::agent::types::AgentRoleKind::MemoryCurator,
            "Curator",
            "Curate memories",
        )
        .with_read_only();
        let spec = SubAgentSpec::new(base_spec, DelegationMode::CallAsTool)
            .with_inherited_context();

        assert!(!spec.isolated_context);

        let outcome = SubAgentExecutionOutcome::ok("Result");
        let result = runtime
            .execute_call_as_tool(&spec, &parent_run, "Task", &outcome)
            .unwrap();

        assert!(result.observation.content.contains("inherits parent"));
    }
}

// ── ReviewAgent ────────────────────────────────────────────────────────

/// Structured review output from a ReviewAgent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewAgentOutput {
    pub verdict: ReviewVerdict,
    pub score: f32,
    pub issues: Vec<ReviewIssue>,
    pub strengths: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Approved,
    NeedsChanges,
    Rejected,
}

impl std::fmt::Display for ReviewVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReviewVerdict::Approved => write!(f, "approved"),
            ReviewVerdict::NeedsChanges => write!(f, "needs_changes"),
            ReviewVerdict::Rejected => write!(f, "rejected"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewIssue {
    pub severity: String,
    pub category: String,
    pub description: String,
    pub suggestion: Option<String>,
}

impl ReviewAgentOutput {
    pub fn approved(summary: impl Into<String>) -> Self {
        Self {
            verdict: ReviewVerdict::Approved,
            score: 1.0,
            issues: Vec::new(),
            strengths: vec!["No issues found".into()],
            summary: summary.into(),
        }
    }

    pub fn needs_changes(
        summary: impl Into<String>,
        issues: Vec<ReviewIssue>,
        strengths: Vec<String>,
    ) -> Self {
        let score = if issues.is_empty() {
            1.0
        } else {
            (1.0 - 0.15 * issues.len() as f32).max(0.1)
        };
        Self {
            verdict: ReviewVerdict::NeedsChanges,
            score,
            issues,
            strengths,
            summary: summary.into(),
        }
    }

    pub fn is_passed(&self) -> bool {
        matches!(self.verdict, ReviewVerdict::Approved)
    }

    pub fn has_critical_issues(&self) -> bool {
        self.issues.iter().any(|i| i.severity == "critical")
    }
}

impl SubAgentRuntime {
    pub fn execute_review(
        &self,
        parent_run: &AgentRun,
        subject_type: &str,
        _subject_content: &str,
        review_result: &ReviewAgentOutput,
    ) -> Result<SubAgentResult> {
        let spec = AgentSpec::new(
            crate::agent::types::AgentRoleKind::Reviewer,
            "ReviewAgent",
            "Review and audit plans, outputs, and patches",
        )
        .with_read_only()
        .with_allowed_tools(vec![
            "file.read".into(),
            "web.search".into(),
            "life_model.read".into(),
            "goal.read".into(),
        ])
        .with_output_schema("review_agent_v1");

        // Review agent explicitly forbids all write operations
        let full_spec = AgentSpec {
            denied_tools: vec![
                "file.write_proposal".into(),
                "life_model.propose_patch".into(),
                "memory.propose_write".into(),
                "memory.propose_archive".into(),
                "permission.replay_action".into(),
                "a2a.call_agent".into(),
                "mcp.call_tool".into(),
                "calendar.propose_event".into(),
                "email.propose_draft".into(),
                "task.create_proposal".into(),
            ],
            read_only: true,
            can_generate_proposals: false,
            ..spec
        };

        let sub_spec = SubAgentSpec::new(full_spec, DelegationMode::Review)
            .with_parent_run(&parent_run.id)
            .with_deadline(60);

        let review_json = serde_json::to_string(review_result).unwrap_or_default();
        let result_text = format!(
            "[Review of {}]\nVerdict: {}\nScore: {:.2}\nSummary: {}\n{} issues, {} strengths",
            subject_type,
            review_result.verdict,
            review_result.score,
            review_result.summary,
            review_result.issues.len(),
            review_result.strengths.len(),
        );

        self.execute_review_internal(
            &sub_spec,
            parent_run,
            &result_text,
            review_json,
            subject_type,
        )
    }

    fn execute_review_internal(
        &self,
        spec: &SubAgentSpec,
        parent_run: &AgentRun,
        result_text: &str,
        structured_json: String,
        subject_type: &str,
    ) -> Result<SubAgentResult> {
        let outcome = SubAgentExecutionOutcome::ok(result_text);
        let structured: Option<serde_json::Value> =
            serde_json::from_str(&structured_json).ok();
        let mut outcome = outcome;
        if let Some(ref s) = structured {
            outcome = outcome.with_structured(s.clone());
        }

        // Build complete child run (write-once)
        let mut child_run = Self::create_child_run(
            parent_run,
            &spec.spec,
            &format!("Review {}", subject_type),
            &outcome,
        );
        child_run.kind = AgentTaskKind::Review;
        self.agent_run_store.create_run(&child_run)?;

        self.record_event(
            &child_run.id,
            AgentRunEventType::RunCreated,
            AgentEventActor::SubAgent("reviewer".into()),
            format!("ReviewAgent reviewing {}", subject_type),
            serde_json::json!({
                "parent_run_id": parent_run.id,
                "role": "reviewer",
                "delegation_mode": "review",
            }),
        );

        let observation = AgentObservation {
            id: format!("review-obs-{}", uuid::Uuid::new_v4()),
            action_id: None,
            content: format!(
                "[ReviewAgent] Review of {}\n\n{}",
                subject_type, result_text
            ),
            source: "sub_agent:reviewer".into(),
            structured_result: structured.clone(),
            timestamp: Utc::now(),
        };

        self.record_event(
            &parent_run.id,
            AgentRunEventType::ObservationCreated,
            AgentEventActor::SubAgent("reviewer".into()),
            format!("ReviewAgent completed review of {}", subject_type),
            serde_json::json!({
                "child_run_id": child_run.id,
                "parent_run_id": parent_run.id,
                "verdict": "completed",
            }),
        );

        let result = SubAgentResult {
            child_run,
            observation,
            success: true,
            structured_output: structured,
            error: None,
        };
        Ok(result)
    }
}

#[cfg(test)]
mod review_agent_tests {
    use super::*;
    use crate::agent::event_store::AgentRunEventStore;
    use crate::agent::store::AgentRunStore;

    fn create_parent() -> AgentRun {
        AgentRun::new_chat_run("review-session", "Review this plan")
    }

    #[test]
    fn test_review_agent_approved_verdict() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, Some(event_store.clone()));

        let parent = create_parent();
        let review = ReviewAgentOutput::approved("The plan looks correct and complete.");

        let result = runtime
            .execute_review(&parent, "plan", "Goal: analyze project", &review)
            .unwrap();

        assert!(result.success);
        assert!(result.structured_output.is_some());
        assert!(result.observation.content.contains("Verdict: approved"));
    }

    #[test]
    fn test_review_agent_needs_changes() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, Some(event_store.clone()));

        let parent = create_parent();
        let review = ReviewAgentOutput::needs_changes(
            "Plan has missing steps",
            vec![
                ReviewIssue {
                    severity: "warning".into(),
                    category: "completeness".into(),
                    description: "Missing rollback plan".into(),
                    suggestion: Some("Add rollback_plan".into()),
                },
                ReviewIssue {
                    severity: "error".into(),
                    category: "correctness".into(),
                    description: "Invalid dependency".into(),
                    suggestion: Some("Fix ordering".into()),
                },
            ],
            vec!["Clear goal".into()],
        );

        let result = runtime
            .execute_review(&parent, "plan", "Goal: X", &review)
            .unwrap();

        assert!(result.success);
        let content = &result.observation.content;
        assert!(content.contains("Verdict: needs_changes"));
        assert!(content.contains("2 issues"));
        assert!(content.contains("1 strengths"));
    }

    #[test]
    fn test_review_agent_cannot_call_write_tools() {
        let spec = AgentSpec::new(
            crate::agent::types::AgentRoleKind::Reviewer,
            "Reviewer",
            "Review",
        )
        .with_read_only();

        assert!(spec.is_tool_denied("file.write_proposal"));
        assert!(spec.is_tool_denied("life_model.propose_patch"));
        assert!(spec.is_tool_denied("memory.propose_write"));
        assert!(spec.is_tool_denied("memory.propose_archive"));
        assert!(spec.read_only);
        assert!(!spec.can_generate_proposals);
    }

    #[test]
    fn test_review_agent_records_parent_trace() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, Some(event_store.clone()));

        let parent = create_parent();
        let review = ReviewAgentOutput::approved("Pass");

        let result = runtime
            .execute_review(&parent, "output", "Some output text", &review)
            .unwrap();

        let parent_events = event_store.list_events_by_run(&parent.id).unwrap();
        assert_eq!(parent_events.len(), 1);
        assert_eq!(
            parent_events[0].event_type,
            AgentRunEventType::ObservationCreated
        );

        let child = runtime
            .agent_run_store()
            .get_run(&result.child_run.id)
            .unwrap()
            .unwrap();
        assert_eq!(child.kind, AgentTaskKind::Review);
    }

    #[test]
    fn test_review_agent_structured_output_serialization() {
        let review = ReviewAgentOutput::needs_changes(
            "Issues found",
            vec![ReviewIssue {
                severity: "error".into(),
                category: "safety".into(),
                description: "Plan proposes deleting system files".into(),
                suggestion: Some("Restrict to safe_paths".into()),
            }],
            vec!["Clear goal statement".into()],
        );

        let json = serde_json::to_string(&review).unwrap();
        assert!(json.contains("needs_changes"));
        assert!(json.contains("safety"));

        let deserialized: ReviewAgentOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.verdict, ReviewVerdict::NeedsChanges);
        assert_eq!(deserialized.issues.len(), 1);
    }

    #[test]
    fn test_review_verdict_display() {
        assert_eq!(ReviewVerdict::Approved.to_string(), "approved");
        assert_eq!(ReviewVerdict::NeedsChanges.to_string(), "needs_changes");
        assert_eq!(ReviewVerdict::Rejected.to_string(), "rejected");
    }
}
