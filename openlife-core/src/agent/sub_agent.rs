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

/// Result from executing a sub-agent.
#[derive(Debug, Clone)]
pub struct SubAgentResult {
    /// The child AgentRun record.
    pub child_run: AgentRun,
    /// Observation that can be appended to the parent run.
    pub observation: AgentObservation,
    /// Whether the sub-agent completed successfully.
    pub success: bool,
    /// Structured output if the sub-agent has an output schema.
    pub structured_output: Option<serde_json::Value>,
    /// Any error from sub-agent execution.
    pub error: Option<String>,
}

impl SubAgentResult {
    pub fn new(
        child_run: AgentRun,
        observation: AgentObservation,
        success: bool,
    ) -> Self {
        Self {
            child_run,
            observation,
            success,
            structured_output: None,
            error: None,
        }
    }
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

    /// Execute a sub-agent in `call_as_tool` mode.
    ///
    /// Creates a child AgentRun linked to the parent, enforces the sub-agent's
    /// tool policy and context isolation, and returns a SubAgentResult.
    ///
    /// In this skeleton implementation, `result_text` is the sub-agent's output
    /// (in production, this would come from an AgentLoop execution with the
    /// sub-agent's spec). The function creates the proper parent-child linkage,
    /// applies tool policy, and records events.
    pub fn execute_call_as_tool(
        &self,
        spec: &SubAgentSpec,
        parent_run: &AgentRun,
        task_description: &str,
        result_text: &str,
    ) -> Result<SubAgentResult> {
        if spec.delegation_mode != DelegationMode::CallAsTool {
            return Err(anyhow::anyhow!(
                "SubAgentRuntime::call_as_tool requires DelegationMode::CallAsTool, got {:?}",
                spec.delegation_mode
            ));
        }

        // ── Create child AgentRun linked to parent ─────────────────────
        let mut child_run = Self::create_child_run(parent_run, &spec.spec, task_description);
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
            }),
        );

        // ── Enforce tool policy ────────────────────────────────────────
        let tool_policy_note = build_tool_policy_note(&spec.spec);

        // ── Apply context isolation ─────────────────────────────────────
        let context_note = if spec.isolated_context {
            "Sub-agent context is isolated from parent context.".to_string()
        } else {
            "Sub-agent inherits parent context.".to_string()
        };

        // ── Record the result ──────────────────────────────────────────
        let success = result_text.contains("error");
        child_run.status = if success {
            AgentRunStatus::Failed
        } else {
            AgentRunStatus::Completed
        };
        child_run.output_preview = Some(truncate(result_text, 200));
        child_run.finished_at = Some(Utc::now());
        self.agent_run_store.update_run(&child_run)?;

        let observation = AgentObservation {
            id: format!("sub-agent-obs-{}", uuid::Uuid::new_v4()),
            action_id: None,
            content: format!(
                "[Sub-Agent: {}] {} Task: {}\n\nPolicy: {}\n\nContext: {}\n\nResult: {}",
                spec.spec.name,
                spec.spec.purpose,
                task_description,
                tool_policy_note,
                context_note,
                result_text,
            ),
            source: format!("sub_agent:{}", spec.spec.role.to_string()),
            structured_result: None,
            timestamp: Utc::now(),
        };

        self.record_event(
            &parent_run.id,
            AgentRunEventType::ObservationCreated,
            AgentEventActor::SubAgent(spec.spec.role.to_string()),
            format!("Sub-agent '{}' completed: {}", spec.spec.name, if success { "failed" } else { "ok" }),
            serde_json::json!({
                "child_run_id": child_run.id,
                "success": !success,
                "parent_run_id": parent_run.id,
            }),
        );

        Ok(SubAgentResult::new(child_run, observation, !success))
    }

    fn create_child_run(
        parent_run: &AgentRun,
        spec: &AgentSpec,
        task_description: &str,
    ) -> AgentRun {
        AgentRun {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: format!("{}-child", parent_run.task_id),
            session_id: parent_run.session_id.clone(),
            status: AgentRunStatus::Running,
            kind: AgentTaskKind::ToolExecution,
            user_input: Some(format!(
                "[Sub-Agent {}] {}",
                spec.role.to_string(),
                task_description
            )),
            context_summary: None,
            model_route: None,
            output_preview: None,
            error: None,
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
            finished_at: None,
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
    use crate::agent::types::AgentSpec;

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

    #[test]
    fn test_call_as_tool_creates_child_run_linked_to_parent() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, Some(event_store.clone()));

        let parent_run = create_test_parent_run();
        let spec = create_test_sub_spec();

        let result = runtime
            .execute_call_as_tool(
                &spec,
                &parent_run,
                "Find all Rust files",
                "Found 12 Rust files in src/",
            )
            .unwrap();

        // Child run exists in store
        let child = runtime
            .agent_run_store()
            .get_run(&result.child_run.id)
            .unwrap()
            .unwrap();
        assert_eq!(child.status, AgentRunStatus::Completed);
        assert_eq!(
            child.user_input.as_deref(),
            Some("[Sub-Agent codebase_explorer] Find all Rust files")
        );
        assert_eq!(child.kind, AgentTaskKind::ToolExecution);

        // Result is returned successfully
        assert!(result.success);
        assert!(result
            .observation
            .content
            .contains("Found 12 Rust files"));
    }

    #[test]
    fn test_call_as_tool_requires_correct_delegation_mode() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, None);
        let parent_run = create_test_parent_run();

        let spec = SubAgentSpec::new(
            AgentSpec::default(),
            DelegationMode::Review, // wrong mode for call_as_tool
        );

        let result = runtime.execute_call_as_tool(
            &spec,
            &parent_run,
            "task",
            "output",
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("CallAsTool"));
    }

    #[test]
    fn test_call_as_tool_records_events() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, Some(event_store.clone()));

        let parent_run = create_test_parent_run();
        let spec = create_test_sub_spec();

        let result = runtime
            .execute_call_as_tool(&spec, &parent_run, "List files", "Done")
            .unwrap();

        // Child run events (RunCreated)
        let child_events = event_store
            .list_events_by_run(&result.child_run.id)
            .unwrap();
        assert_eq!(child_events.len(), 1);
        assert_eq!(child_events[0].event_type, AgentRunEventType::RunCreated);
        assert_eq!(
            child_events[0].actor,
            AgentEventActor::SubAgent("codebase_explorer".into())
        );

        // Parent run gets observation event
        let parent_events = event_store
            .list_events_by_run(&parent_run.id)
            .unwrap();
        assert_eq!(parent_events.len(), 1);
        assert_eq!(
            parent_events[0].event_type,
            AgentRunEventType::ObservationCreated
        );
        assert!(parent_events[0]
            .payload
            .get("child_run_id")
            .is_some());
        assert_eq!(
            parent_events[0]
                .payload
                .get("parent_run_id")
                .unwrap()
                .as_str()
                .unwrap(),
            parent_run.id
        );
    }

    #[test]
    fn test_sub_agent_result_includes_tool_policy() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, None);
        let parent_run = create_test_parent_run();
        let spec = create_test_sub_spec();

        let result = runtime
            .execute_call_as_tool(&spec, &parent_run, "Analyze", "Analysis result")
            .unwrap();

        assert!(result.observation.content.contains("read-only mode"));
        assert!(result.observation.content.contains("allowed tools"));
        assert!(result.observation.content.contains("file.read"));
        assert!(result.observation.content.contains("web.search"));
    }

    #[test]
    fn test_sub_agent_context_isolation() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, None);
        let parent_run = create_test_parent_run();

        // Default: isolated context
        let spec = create_test_sub_spec();
        assert!(spec.isolated_context);

        let result = runtime
            .execute_call_as_tool(&spec, &parent_run, "Task", "Result")
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
        );
        let spec = SubAgentSpec::new(base_spec, DelegationMode::CallAsTool)
            .with_inherited_context();

        assert!(!spec.isolated_context);

        let result = runtime
            .execute_call_as_tool(&spec, &parent_run, "Task", "Result")
            .unwrap();

        assert!(result.observation.content.contains("inherits parent"));
    }

    #[test]
    fn test_multiple_sub_agent_calls_isolated_children() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, Some(event_store.clone()));
        let parent_run = create_test_parent_run();

        let spec = create_test_sub_spec();

        let r1 = runtime
            .execute_call_as_tool(&spec, &parent_run, "Task A", "Result A")
            .unwrap();
        let r2 = runtime
            .execute_call_as_tool(&spec, &parent_run, "Task B", "Result B")
            .unwrap();

        // Each has its own child run
        assert_ne!(r1.child_run.id, r2.child_run.id);
        assert!(runtime
            .agent_run_store()
            .get_run(&r1.child_run.id)
            .unwrap()
            .is_some());
        assert!(runtime
            .agent_run_store()
            .get_run(&r2.child_run.id)
            .unwrap()
            .is_some());

        // Parent accumulates events from both
        let parent_events = event_store.list_events_by_run(&parent_run.id).unwrap();
        assert_eq!(parent_events.len(), 2);
    }
}

// ── ReviewAgent ────────────────────────────────────────────────────────

/// Structured review output from a ReviewAgent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewAgentOutput {
    /// Overall verdict: approved, needs_changes, rejected.
    pub verdict: ReviewVerdict,
    /// Score from 0.0 to 1.0.
    pub score: f32,
    /// List of issues found (if any).
    pub issues: Vec<ReviewIssue>,
    /// List of positive observations.
    pub strengths: Vec<String>,
    /// Free-form summary of the review.
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
    pub severity: String, // "info", "warning", "error", "critical"
    pub category: String, // "correctness", "completeness", "safety", "policy", etc.
    pub description: String,
    pub suggestion: Option<String>,
}

impl ReviewAgentOutput {
    /// Create a review output from a summary string.
    pub fn approved(summary: impl Into<String>) -> Self {
        Self {
            verdict: ReviewVerdict::Approved,
            score: 1.0,
            issues: Vec::new(),
            strengths: vec!["No issues found".into()],
            summary: summary.into(),
        }
    }

    /// Create a review output requesting changes.
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

    /// Check if the review passed.
    pub fn is_passed(&self) -> bool {
        matches!(self.verdict, ReviewVerdict::Approved)
    }

    /// Check if the review found critical issues.
    pub fn has_critical_issues(&self) -> bool {
        self.issues.iter().any(|i| i.severity == "critical")
    }
}

impl SubAgentRuntime {
    /// Execute a ReviewAgent sub-agent. The reviewer receives a subject
    /// (plan, output, patch, etc.) and returns a structured review without
    /// mutating any state.
    ///
    /// The reviewer cannot call write tools, cannot mutate LifeModel or
    /// Memory, and the review result is recorded in the parent run trace.
    pub fn execute_review(
        &self,
        parent_run: &AgentRun,
        subject_type: &str,
        _subject_content: &str,
        review_result: &ReviewAgentOutput,
    ) -> Result<SubAgentResult> {
        let mut spec = AgentSpec::new(
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
        spec.denied_tools = vec![
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
        ];
        spec.read_only = true;
        spec.can_generate_proposals = false;

        let sub_spec = SubAgentSpec::new(spec, DelegationMode::Review)
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
        let mut child_run =
            Self::create_child_run(parent_run, &spec.spec, &format!("Review {}", subject_type));
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

        child_run.status = AgentRunStatus::Completed;
        child_run.output_preview = Some(truncate(result_text, 200));
        child_run.finished_at = Some(Utc::now());
        self.agent_run_store.update_run(&child_run)?;

        let structured: Option<serde_json::Value> =
            serde_json::from_str(&structured_json).ok();

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

        let mut result = SubAgentResult::new(child_run, observation, true);
        result.structured_output = structured;
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
        assert!(result
            .observation
            .content
            .contains("Verdict: approved"));
        assert!(result.observation.content.contains("Score: 1.00"));
    }

    #[test]
    fn test_review_agent_needs_changes() {
        let run_store = AgentRunStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let runtime = SubAgentRuntime::new(run_store, Some(event_store.clone()));

        let parent = create_parent();
        let review = ReviewAgentOutput::needs_changes(
            "Plan has missing steps and unclear risk assessment",
            vec![
                ReviewIssue {
                    severity: "warning".into(),
                    category: "completeness".into(),
                    description: "Missing rollback plan".into(),
                    suggestion: Some("Add rollback_plan field".into()),
                },
                ReviewIssue {
                    severity: "error".into(),
                    category: "correctness".into(),
                    description: "Step 2 depends on undefined step 4".into(),
                    suggestion: Some("Fix dependency ordering".into()),
                },
            ],
            vec!["Goal is well-defined".into()],
        );

        assert_eq!(review.verdict, ReviewVerdict::NeedsChanges);
        assert!(!review.is_passed());
        assert!(!review.has_critical_issues());
        assert!(review.score < 1.0);

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

        // Review agent inherits write denials from with_read_only()
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

        // Parent trace should have the review observation
        let parent_events = event_store.list_events_by_run(&parent.id).unwrap();
        assert_eq!(parent_events.len(), 1);
        assert_eq!(
            parent_events[0].event_type,
            AgentRunEventType::ObservationCreated
        );
        assert_eq!(
            parent_events[0].actor,
            AgentEventActor::SubAgent("reviewer".into())
        );

        // Child run exists with correct kind
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
        assert!(json.contains("safe_paths"));

        let deserialized: ReviewAgentOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.verdict, ReviewVerdict::NeedsChanges);
        assert_eq!(deserialized.issues.len(), 1);
        assert_eq!(deserialized.issues[0].severity, "error");
    }

    #[test]
    fn test_review_verdict_display() {
        assert_eq!(ReviewVerdict::Approved.to_string(), "approved");
        assert_eq!(ReviewVerdict::NeedsChanges.to_string(), "needs_changes");
        assert_eq!(ReviewVerdict::Rejected.to_string(), "rejected");
    }
}
