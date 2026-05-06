use crate::agent::event_store::AgentRunEventStore;
use crate::agent::plan_store::PlanStore;
use crate::agent::sub_agent::{ReviewAgentOutput, ReviewIssue};
use crate::agent::types::*;
use serde_json::json;
use std::sync::{Arc, Mutex};

pub enum PlanExecutionError {
    PlanNotFound(String),
    PlanNotConfirmed(String),
    StoreError(String),
    ReviewFailed(String),
}

impl std::fmt::Display for PlanExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanExecutionError::PlanNotFound(id) => write!(f, "plan not found: {}", id),
            PlanExecutionError::PlanNotConfirmed(id) => {
                write!(f, "plan {} requires confirmation before execution", id)
            }
            PlanExecutionError::StoreError(e) => write!(f, "store error: {}", e),
            PlanExecutionError::ReviewFailed(msg) => write!(f, "review failed: {}", msg),
        }
    }
}

impl std::fmt::Debug for PlanExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

/// Determines whether a completed plan execution passes review.
///
/// Implementations must be **read-only**: they may inspect the plan and
/// outcome, but must not mutate LifeModel, Memory, files, or external state.
pub trait PlanReviewGate {
    fn review(
        &self,
        plan: &AgentPlan,
        outcome: &PlanExecutionOutcome,
    ) -> Result<ReviewAgentOutput, PlanExecutionError>;
}

/// Default deterministic review gate: always approves successful executions.
///
/// Production may replace this with an LLM-based reviewer via
/// `SubAgentRuntime::execute_review()`, once that path is stable.
pub struct DefaultPlanReviewGate;

impl PlanReviewGate for DefaultPlanReviewGate {
    fn review(
        &self,
        _plan: &AgentPlan,
        outcome: &PlanExecutionOutcome,
    ) -> Result<ReviewAgentOutput, PlanExecutionError> {
        if outcome.success {
            Ok(ReviewAgentOutput::approved(
                "deterministic review gate: all steps passed",
            ))
        } else {
            Ok(ReviewAgentOutput::needs_changes(
                "deterministic review gate: execution failed",
                vec![ReviewIssue {
                    severity: "high".to_string(),
                    category: "execution_failure".to_string(),
                    description: format!("{} steps failed", outcome.steps_failed),
                    suggestion: Some("review failed steps and retry".to_string()),
                }],
                vec![],
            ))
        }
    }
}

/// Review gate backed by `SubAgentRuntime` for read-only boundary enforcement.
///
/// In the current implementation this gate produces the same deterministic
/// verdict as `DefaultPlanReviewGate`.  When a real LLM reviewer is
/// integrated the `review()` method should:
///
/// 1. Call the LLM to analyse the plan execution and produce a
///    `ReviewAgentOutput`.
/// 2. Route the output through `SubAgentRuntime::execute_review()` so
///    the read-only tool restriction and child-run linkage are enforced.
///
/// Until then, production code uses `DefaultPlanReviewGate`.
pub struct SubAgentReviewGate;

impl PlanReviewGate for SubAgentReviewGate {
    fn review(
        &self,
        _plan: &AgentPlan,
        outcome: &PlanExecutionOutcome,
    ) -> Result<ReviewAgentOutput, PlanExecutionError> {
        if outcome.success {
            Ok(ReviewAgentOutput::approved(
                "governed review gate: all steps passed",
            ))
        } else {
            Ok(ReviewAgentOutput::needs_changes(
                "governed review gate: execution failed",
                vec![ReviewIssue {
                    severity: "high".to_string(),
                    category: "execution_failure".to_string(),
                    description: format!("{} steps failed", outcome.steps_failed),
                    suggestion: Some("review failed steps and retry".to_string()),
                }],
                vec![],
            ))
        }
    }
}

pub struct PlanExecutor {
    plan_store: Arc<Mutex<PlanStore>>,
    event_store: Option<AgentRunEventStore>,
    agent_spec: Option<AgentSpec>,
}

impl PlanExecutor {
    pub fn new(plan_store: Arc<Mutex<PlanStore>>, event_store: Option<AgentRunEventStore>) -> Self {
        Self {
            plan_store,
            event_store,
            agent_spec: None,
        }
    }

    /// Attach an AgentSpec that constrains tool execution.
    pub fn with_agent_spec(mut self, spec: AgentSpec) -> Self {
        self.agent_spec = Some(spec);
        self
    }

    fn lock_store(&self) -> Result<std::sync::MutexGuard<'_, PlanStore>, PlanExecutionError> {
        self.plan_store
            .lock()
            .map_err(|e| PlanExecutionError::StoreError(format!("plan store lock poisoned: {}", e)))
    }

    /// Check whether the plan has been cancelled since execution started.
    fn is_plan_cancelled(&self, plan_id: &str) -> Result<bool, PlanExecutionError> {
        let guard = self.lock_store()?;
        match guard.get_plan(plan_id) {
            Ok(Some(plan)) => Ok(plan.status == PlanStatus::Cancelled),
            _ => Ok(false),
        }
    }

    /// Execute all plan steps without finalising plan status.
    ///
    /// Loads the plan, checks confirmation, executes steps, records step
    /// events and deviations.  On step failure persists `Failed` and records
    /// `plan.execution_failed`.  On success returns the plan and outcome
    /// **without** persisting `Completed` or recording `plan.execution_completed`.
    /// The caller is responsible for the final status transition.
    fn execute_steps_without_completion<F>(
        &self,
        plan_id: &str,
        run_id: &str,
        mut execute_step: F,
    ) -> Result<(AgentPlan, PlanExecutionOutcome), PlanExecutionError>
    where
        F: FnMut(&PlanStep, Option<&ToolIntent>) -> PlanStepExecutionResult,
    {
        let mut plan = self
            .lock_store()?
            .get_plan(plan_id)
            .map_err(|e| PlanExecutionError::StoreError(e.to_string()))?
            .ok_or_else(|| PlanExecutionError::PlanNotFound(plan_id.to_string()))?;

        // Reject unconfirmed high-risk plans.
        if plan.requires_confirmation && plan.status != PlanStatus::Confirmed {
            return Err(PlanExecutionError::PlanNotConfirmed(plan_id.to_string()));
        }

        // Reject already-terminal plans (cancelled/completed/rejected/failed).
        match plan.status {
            PlanStatus::Cancelled
            | PlanStatus::Completed
            | PlanStatus::Rejected
            | PlanStatus::Failed
            | PlanStatus::FailedReview => {
                return Ok((
                    plan,
                    PlanExecutionOutcome {
                        plan_id: plan_id.to_string(),
                        success: false,
                        steps_completed: 0,
                        steps_failed: 0,
                        deviations: vec![],
                        review_required: false,
                    },
                ));
            }
            _ => {}
        }

        // Record execution started.
        self.record_event(
            run_id,
            AgentRunEventType::PlanExecutionStarted,
            format!("started executing plan {}", plan_id),
            json!({"plan_id": plan_id, "total_steps": plan.steps.len()}),
        );

        plan.start_execution();
        let _ = self.lock_store().and_then(|s| {
            s.update_plan(&plan)
                .map_err(|e| PlanExecutionError::StoreError(e.to_string()))
        });

        let total_steps = plan.steps.len() as u32;
        let mut outcome = PlanExecutionOutcome {
            plan_id: plan_id.to_string(),
            success: true,
            steps_completed: 0,
            steps_failed: 0,
            deviations: Vec::new(),
            review_required: !matches!(plan.risk_level, RiskLevel::Low),
        };

        // Copy steps to avoid borrow issues.
        let steps: Vec<PlanStep> = plan.steps.clone();
        let tool_intents: Vec<ToolIntent> = plan.tool_intents.clone();

        for step in &steps {
            // Bail out if the plan was cancelled mid-execution.
            if self.is_plan_cancelled(plan_id)? {
                outcome.success = false;
                self.record_event(
                    run_id,
                    AgentRunEventType::PlanExecutionFailed,
                    format!("plan {} cancelled mid-execution", plan_id),
                    json!({"reason": "cancelled", "step_index": step.index}),
                );
                return Ok((plan, outcome));
            }
            self.record_event(
                run_id,
                AgentRunEventType::PlanStepStarted,
                format!("step {}: {}", step.index, step.description),
                json!({"step_index": step.index}),
            );

            let tool_intent = step
                .tool_intent
                .as_ref()
                .and_then(|tool_name| tool_intents.iter().find(|ti| ti.tool_name == *tool_name));

            let mut result = execute_step(step, tool_intent);

            // Enforce AgentSpec tool policy if attached.
            if let Some(ref spec) = self.agent_spec {
                if !spec.is_tool_allowed(&result.tool_name) {
                    result.success = false;
                    result.error = Some(format!(
                        "tool '{}' blocked by AgentSpec policy",
                        result.tool_name
                    ));
                    self.record_event(
                        run_id,
                        AgentRunEventType::ToolCallBlocked,
                        format!(
                            "tool '{}' blocked by AgentSpec {}",
                            result.tool_name, spec.id
                        ),
                        json!({"tool_name": result.tool_name, "agentspec_id": spec.id}),
                    );
                }
            }

            // Detect deviation.
            if let Some(intent) = tool_intent {
                if result.tool_name != intent.tool_name && result.tool_name != "skipped" {
                    let deviation_msg = format!(
                        "step {}: planned tool '{}' but executed '{}'",
                        step.index, intent.tool_name, result.tool_name
                    );
                    if let Some(ref dev_detail) = result.deviation {
                        outcome
                            .deviations
                            .push(format!("{}: {}", deviation_msg, dev_detail));
                    } else {
                        outcome.deviations.push(deviation_msg.clone());
                    }
                    self.record_event(
                        run_id,
                        AgentRunEventType::PlanDeviationRecorded,
                        deviation_msg,
                        json!({"step_index": step.index}),
                    );
                }
            }

            if result.success {
                outcome.steps_completed += 1;
                self.record_event(
                    run_id,
                    AgentRunEventType::PlanStepCompleted,
                    format!("step {} completed", step.index),
                    json!({"step_index": step.index, "tool": result.tool_name}),
                );
            } else {
                outcome.steps_failed += 1;
                outcome.success = false;
                self.record_event(
                    run_id,
                    AgentRunEventType::PlanStepFailed,
                    format!(
                        "step {} failed: {}",
                        step.index,
                        result.error.as_deref().unwrap_or("unknown error")
                    ),
                    json!({"step_index": step.index, "tool": result.tool_name}),
                );

                self.record_event(
                    run_id,
                    AgentRunEventType::PlanExecutionFailed,
                    format!(
                        "plan execution failed at step {} ({}/{})",
                        step.index,
                        step.index + 1,
                        total_steps
                    ),
                    json!({"failed_step": step.index}),
                );

                // Persist terminal failed status.
                plan.status = PlanStatus::Failed;
                plan.updated_at = chrono::Utc::now();
                let _ = self.lock_store().and_then(|s| {
                    s.update_plan(&plan)
                        .map_err(|e| PlanExecutionError::StoreError(e.to_string()))
                });

                return Ok((plan, outcome));
            }
        }

        Ok((plan, outcome))
    }

    /// Execute a confirmed plan and finalise it as `Completed`.
    ///
    /// This is the simple path used when **no review gate** is needed.
    /// After all steps succeed the plan is immediately completed.
    pub fn execute<F>(
        &self,
        plan_id: &str,
        run_id: &str,
        execute_step: F,
    ) -> Result<PlanExecutionOutcome, PlanExecutionError>
    where
        F: FnMut(&PlanStep, Option<&ToolIntent>) -> PlanStepExecutionResult,
    {
        let (mut plan, outcome) =
            self.execute_steps_without_completion(plan_id, run_id, execute_step)?;

        if outcome.success {
            // Guard: re-read persisted plan — if cancelled, don't complete.
            if self.is_plan_cancelled(plan_id)? {
                return Ok(outcome);
            }
            self.record_event(
                run_id,
                AgentRunEventType::PlanExecutionCompleted,
                format!(
                    "plan {} execution completed: {}/{} steps succeeded",
                    plan_id, outcome.steps_completed, plan.steps.len()
                ),
                json!({"steps_completed": outcome.steps_completed, "steps_failed": outcome.steps_failed}),
            );

            plan.complete();
            let _ = self.lock_store().and_then(|s| {
                s.update_plan(&plan)
                    .map_err(|e| PlanExecutionError::StoreError(e.to_string()))
            });
        }

        Ok(outcome)
    }

    /// Execute a confirmed plan **with a review gate**.
    ///
    /// For low-risk plans the gate is skipped and the plan is completed directly.
    /// For medium/high/critical-risk plan executions the review gate is invoked
    /// **before** the plan is finalised:
    ///
    /// | Gate result   | Plan status  | Final event                |
    /// |--------------|-------------|----------------------------|
    /// | Approved      | Completed   | `plan.execution_completed`  |
    /// | Critical      | FailedReview| `plan.execution_failed`     |
    ///
    /// The review observation event is always recorded **before** the final
    /// completion/failure event.
    pub fn execute_with_review<F, R>(
        &self,
        plan_id: &str,
        run_id: &str,
        execute_step: F,
        review_gate: &R,
    ) -> Result<PlanExecutionOutcome, PlanExecutionError>
    where
        F: FnMut(&PlanStep, Option<&ToolIntent>) -> PlanStepExecutionResult,
        R: PlanReviewGate,
    {
        let (mut plan, outcome) =
            self.execute_steps_without_completion(plan_id, run_id, execute_step)?;

        if !outcome.success {
            return Ok(outcome);
        }

        if !outcome.review_required {
            // Low-risk: complete directly.
            if self.is_plan_cancelled(plan_id)? {
                return Ok(outcome);
            }
            self.record_event(
                run_id,
                AgentRunEventType::PlanExecutionCompleted,
                format!(
                    "plan {} execution completed: {}/{} steps succeeded",
                    plan_id, outcome.steps_completed, plan.steps.len()
                ),
                json!({"steps_completed": outcome.steps_completed, "steps_failed": outcome.steps_failed}),
            );
            plan.complete();
            let _ = self.lock_store().and_then(|s| {
                s.update_plan(&plan)
                    .map_err(|e| PlanExecutionError::StoreError(e.to_string()))
            });
            return Ok(outcome);
        }

        // Review gate for medium/high/critical plans.
        let review = review_gate.review(&plan, &outcome)?;

        match self.review_execution(plan_id, run_id, &review) {
            Ok(()) => {
                // Guard: re-read persisted plan — cancelled mid-review.
                if self.is_plan_cancelled(plan_id)? {
                    return Ok(outcome);
                }
                // Approved: complete plan.
                self.record_event(
                    run_id,
                    AgentRunEventType::PlanExecutionCompleted,
                    format!("plan {} execution completed after approved review", plan_id),
                    json!({
                        "steps_completed": outcome.steps_completed,
                        "steps_failed": outcome.steps_failed,
                        "review_verdict": "approved"
                    }),
                );
                plan.complete();
                let _ = self.lock_store().and_then(|s| {
                    s.update_plan(&plan)
                        .map_err(|e| PlanExecutionError::StoreError(e.to_string()))
                });
                Ok(outcome)
            }
            Err(PlanExecutionError::ReviewFailed(msg)) => {
                // Critical review: record terminal failure event.
                // review_execution already set FailedReview status and
                // recorded the review observation event.
                self.record_event(
                    run_id,
                    AgentRunEventType::PlanExecutionFailed,
                    format!("plan {} failed review", plan_id),
                    json!({
                        "reason": "review_failed",
                        "status": "failed_review",
                        "message": msg,
                        "steps_completed": outcome.steps_completed,
                        "steps_failed": outcome.steps_failed
                    }),
                );
                Err(PlanExecutionError::ReviewFailed(msg))
            }
            Err(e) => Err(e),
        }
    }

    /// Review the execution result of a completed plan.
    ///
    /// Used as a post-execution gate for medium/high-risk plans.
    /// - `Approved` → plan stays Completed; records a review trace event.
    /// - `NeedsChanges` or `Rejected` (critical) → plan set to `FailedReview`; records event.
    ///
    /// This method does NOT mutate LifeModel, Memory, files, or external state.
    pub fn review_execution(
        &self,
        plan_id: &str,
        run_id: &str,
        review_result: &crate::agent::ReviewAgentOutput,
    ) -> Result<(), PlanExecutionError> {
        let mut plan = self
            .lock_store()?
            .get_plan(plan_id)
            .map_err(|e| PlanExecutionError::StoreError(e.to_string()))?
            .ok_or_else(|| PlanExecutionError::PlanNotFound(plan_id.to_string()))?;

        let verdict_str = match review_result.verdict {
            crate::agent::ReviewVerdict::Approved => "approved",
            crate::agent::ReviewVerdict::NeedsChanges => "needs_changes",
            crate::agent::ReviewVerdict::Rejected => "rejected",
        };

        self.record_event(
            run_id,
            AgentRunEventType::ObservationCreated,
            format!(
                "review agent verdict: {} (score {:.1})",
                verdict_str, review_result.score
            ),
            json!({
                "plan_id": plan_id,
                "verdict": verdict_str,
                "score": review_result.score,
                "issue_count": review_result.issues.len(),
                "has_critical": review_result.has_critical_issues(),
                "summary": review_result.summary,
            }),
        );

        if review_result.has_critical_issues() {
            plan.status = PlanStatus::FailedReview;
            plan.updated_at = chrono::Utc::now();
            let _ = self.lock_store().and_then(|s| {
                s.update_plan(&plan)
                    .map_err(|e| PlanExecutionError::StoreError(e.to_string()))
            });
            return Err(PlanExecutionError::ReviewFailed(format!(
                "plan {} failed review: {}",
                plan_id, review_result.summary
            )));
        }

        // Approved: plan stays completed (no action needed).
        Ok(())
    }

    fn record_event(
        &self,
        run_id: &str,
        event_type: AgentRunEventType,
        summary: String,
        payload: serde_json::Value,
    ) {
        if let Some(ref es) = self.event_store {
            let event = AgentRunEvent::new(
                run_id,
                event_type,
                AgentEventActor::Runtime,
                summary,
                payload,
            );
            let _ = es.append_event(&event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Arc<Mutex<PlanStore>>, AgentRunEventStore, String) {
        let ps = Arc::new(Mutex::new(PlanStore::new_in_memory().unwrap()));
        let es = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "test-run-1".to_string();
        (ps, es, run_id)
    }

    fn create_read_only_plan(confirmed: bool) -> AgentPlan {
        let mut plan = AgentPlan::new("read task", RiskLevel::Low);
        plan.steps = vec![PlanStep {
            index: 0,
            description: "step 0".to_string(),
            tool_intent: Some("life_model.read".to_string()),
            expected_output: Some("life model data".to_string()),
            depends_on: vec![],
        }];
        plan.tool_intents = vec![ToolIntent {
            tool_name: "life_model.read".to_string(),
            purpose: "read".to_string(),
            risk_level: RiskLevel::Low,
            is_write: false,
            parameters_summary: None,
        }];
        plan.publish();
        if confirmed {
            plan.confirm();
        }
        plan
    }

    fn create_high_risk_plan(confirmed: bool) -> AgentPlan {
        let mut plan = AgentPlan::new("write task", RiskLevel::High);
        plan.steps = vec![PlanStep {
            index: 0,
            description: "write step".to_string(),
            tool_intent: Some("file.write_proposal".to_string()),
            expected_output: None,
            depends_on: vec![],
        }];
        plan.tool_intents = vec![ToolIntent {
            tool_name: "file.write_proposal".to_string(),
            purpose: "write".to_string(),
            risk_level: RiskLevel::High,
            is_write: true,
            parameters_summary: None,
        }];
        plan.publish();
        if confirmed {
            plan.confirm();
        }
        plan
    }

    #[test]
    fn test_confirmed_plan_executes_read_only_step() {
        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps, Some(es.clone()));

        let outcome = executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: true,
                    output: Some("read ok".to_string()),
                    error: None,
                    duration_ms: 10,
                    deviation: None,
                }
            })
            .unwrap();

        assert!(outcome.success);
        assert_eq!(outcome.steps_completed, 1);
        assert_eq!(outcome.steps_failed, 0);

        let events = es.list_events_by_run(&run_id).unwrap();
        assert_eq!(
            events[0].event_type,
            AgentRunEventType::PlanExecutionStarted
        );
        assert_eq!(events[1].event_type, AgentRunEventType::PlanStepStarted);
        assert_eq!(events[2].event_type, AgentRunEventType::PlanStepCompleted);
        assert_eq!(
            events[3].event_type,
            AgentRunEventType::PlanExecutionCompleted
        );
    }

    #[test]
    fn test_unconfirmed_high_risk_plan_is_rejected() {
        let (ps, es, run_id) = setup();
        let plan = create_high_risk_plan(false); // Published but NOT confirmed
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps, Some(es));

        let result = executor.execute(&plan_id, &run_id, |_step, _intent| {
            PlanStepExecutionResult {
                step_index: 0,
                tool_name: "file.write_proposal".to_string(),
                success: true,
                output: None,
                error: None,
                duration_ms: 0,
                deviation: None,
            }
        });

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, PlanExecutionError::PlanNotConfirmed(_)),
            "expected PlanNotConfirmed, got: {:?}",
            err
        );
    }

    #[test]
    fn test_step_events_are_recorded_in_order() {
        let (ps, es, run_id) = setup();
        let mut plan = create_read_only_plan(true);
        plan.steps = vec![
            PlanStep {
                index: 0,
                description: "first".to_string(),
                tool_intent: Some("life_model.read".to_string()),
                expected_output: None,
                depends_on: vec![],
            },
            PlanStep {
                index: 1,
                description: "second".to_string(),
                tool_intent: Some("memory.search".to_string()),
                expected_output: None,
                depends_on: vec![],
            },
        ];
        plan.tool_intents = vec![
            ToolIntent {
                tool_name: "life_model.read".to_string(),
                purpose: "read".to_string(),
                risk_level: RiskLevel::Low,
                is_write: false,
                parameters_summary: None,
            },
            ToolIntent {
                tool_name: "memory.search".to_string(),
                purpose: "search".to_string(),
                risk_level: RiskLevel::Low,
                is_write: false,
                parameters_summary: None,
            },
        ];
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps, Some(es.clone()));

        let mut step_count = 0;
        let outcome = executor
            .execute(&plan_id, &run_id, |step, _intent| {
                step_count += 1;
                PlanStepExecutionResult {
                    step_index: step.index,
                    tool_name: if step.index == 0 {
                        "life_model.read"
                    } else {
                        "memory.search"
                    }
                    .to_string(),
                    success: true,
                    output: Some(format!("step {} ok", step.index)),
                    error: None,
                    duration_ms: 5,
                    deviation: None,
                }
            })
            .unwrap();

        assert!(outcome.success);
        assert_eq!(step_count, 2);

        let events = es.list_events_by_run(&run_id).unwrap();
        assert_eq!(events.len(), 6);
        assert_eq!(
            events[0].event_type,
            AgentRunEventType::PlanExecutionStarted
        );
        assert_eq!(events[1].event_type, AgentRunEventType::PlanStepStarted); // step 0
        assert_eq!(events[2].event_type, AgentRunEventType::PlanStepCompleted); // step 0
        assert_eq!(events[3].event_type, AgentRunEventType::PlanStepStarted); // step 1
        assert_eq!(events[4].event_type, AgentRunEventType::PlanStepCompleted); // step 1
        assert_eq!(
            events[5].event_type,
            AgentRunEventType::PlanExecutionCompleted
        );
    }

    #[test]
    fn test_deviation_event_recorded_when_action_differs_from_plan() {
        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps, Some(es.clone()));

        let outcome = executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "web.search".to_string(), // different from planned life_model.read
                    success: true,
                    output: Some("found results".to_string()),
                    error: None,
                    duration_ms: 20,
                    deviation: Some("executed different tool for broader context".to_string()),
                }
            })
            .unwrap();

        assert!(outcome.success);
        assert_eq!(outcome.deviations.len(), 1);
        assert!(outcome.deviations[0].contains("web.search"));

        let events = es.list_events_by_run(&run_id).unwrap();
        let deviation_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == AgentRunEventType::PlanDeviationRecorded)
            .collect();
        assert_eq!(deviation_events.len(), 1);
    }

    #[test]
    fn test_low_risk_read_only_plan_executes_without_confirmation() {
        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(false); // Published but not explicitly confirmed
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps, Some(es.clone()));

        let outcome = executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: true,
                    output: Some("done".to_string()),
                    error: None,
                    duration_ms: 1,
                    deviation: None,
                }
            })
            .unwrap();

        assert!(outcome.success);
        let events = es.list_events_by_run(&run_id).unwrap();
        assert_eq!(
            events[0].event_type,
            AgentRunEventType::PlanExecutionStarted
        );
        assert_eq!(
            events.last().unwrap().event_type,
            AgentRunEventType::PlanExecutionCompleted
        );
    }

    #[test]
    fn test_plan_not_found_error() {
        let (ps, es, run_id) = setup();
        let executor = PlanExecutor::new(ps, Some(es));
        let result = executor.execute("nonexistent", &run_id, |_, _| PlanStepExecutionResult {
            step_index: 0,
            tool_name: "none".to_string(),
            success: true,
            output: None,
            error: None,
            duration_ms: 0,
            deviation: None,
        });
        assert!(matches!(result, Err(PlanExecutionError::PlanNotFound(_))));
    }

    #[test]
    fn test_step_failure_stops_execution_and_records_failed_event() {
        let (ps, es, run_id) = setup();
        let mut plan = create_read_only_plan(true);
        plan.steps = vec![
            PlanStep {
                index: 0,
                description: "will succeed".to_string(),
                tool_intent: Some("life_model.read".to_string()),
                expected_output: None,
                depends_on: vec![],
            },
            PlanStep {
                index: 1,
                description: "will fail".to_string(),
                tool_intent: Some("memory.search".to_string()),
                expected_output: None,
                depends_on: vec![],
            },
            PlanStep {
                index: 2,
                description: "should not execute".to_string(),
                tool_intent: Some("goal.read".to_string()),
                expected_output: None,
                depends_on: vec![],
            },
        ];
        plan.tool_intents = vec![
            ToolIntent {
                tool_name: "life_model.read".to_string(),
                purpose: "read".to_string(),
                risk_level: RiskLevel::Low,
                is_write: false,
                parameters_summary: None,
            },
            ToolIntent {
                tool_name: "memory.search".to_string(),
                purpose: "search".to_string(),
                risk_level: RiskLevel::Low,
                is_write: false,
                parameters_summary: None,
            },
            ToolIntent {
                tool_name: "goal.read".to_string(),
                purpose: "read".to_string(),
                risk_level: RiskLevel::Low,
                is_write: false,
                parameters_summary: None,
            },
        ];
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps, Some(es.clone()));

        let mut call_count = 0;
        let outcome = executor
            .execute(&plan_id, &run_id, |step, _intent| {
                call_count += 1;
                if step.index == 1 {
                    PlanStepExecutionResult {
                        step_index: step.index,
                        tool_name: "memory.search".to_string(),
                        success: false,
                        output: None,
                        error: Some("search failed".to_string()),
                        duration_ms: 1,
                        deviation: None,
                    }
                } else {
                    PlanStepExecutionResult {
                        step_index: step.index,
                        tool_name: "life_model.read".to_string(),
                        success: true,
                        output: Some("ok".to_string()),
                        error: None,
                        duration_ms: 1,
                        deviation: None,
                    }
                }
            })
            .unwrap();

        assert!(!outcome.success);
        assert_eq!(call_count, 2); // step 2 should NOT execute
        assert_eq!(outcome.steps_completed, 1);
        assert_eq!(outcome.steps_failed, 1);

        let events = es.list_events_by_run(&run_id).unwrap();
        let failed_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == AgentRunEventType::PlanStepFailed)
            .collect();
        assert_eq!(failed_events.len(), 1);
        assert!(events
            .iter()
            .any(|e| e.event_type == AgentRunEventType::PlanExecutionFailed));
        // PlanExecutionCompleted should NOT be present.
        assert!(!events
            .iter()
            .any(|e| e.event_type == AgentRunEventType::PlanExecutionCompleted));
    }

    #[test]
    fn test_write_tool_returns_blocked_when_policy_disallows() {
        let (ps, es, run_id) = setup();
        let plan = create_high_risk_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps, Some(es.clone()));

        let outcome = executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "file.write_proposal".to_string(),
                    success: false,
                    output: None,
                    error: Some("blocked: write disabled by policy".to_string()),
                    duration_ms: 0,
                    deviation: None,
                }
            })
            .unwrap();

        assert!(!outcome.success);
        assert_eq!(outcome.steps_failed, 1);
        let events = es.list_events_by_run(&run_id).unwrap();
        assert!(events
            .iter()
            .any(|e| e.event_type == AgentRunEventType::PlanStepFailed));
    }

    // ── P4-S2: Terminal Failed State tests ────────────────────────────

    #[test]
    fn test_failed_step_persists_failed_status() {
        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps.clone(), Some(es));

        let _outcome = executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: false,
                    output: None,
                    error: Some("tool unavailable".to_string()),
                    duration_ms: 0,
                    deviation: None,
                }
            })
            .unwrap();

        let fetched = ps.lock().unwrap().get_plan(&plan_id).unwrap().unwrap();
        assert_eq!(fetched.status, PlanStatus::Failed);
    }

    #[test]
    fn test_failed_plan_not_left_executing() {
        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps.clone(), Some(es));

        executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: false,
                    output: None,
                    error: Some("fail".to_string()),
                    duration_ms: 0,
                    deviation: None,
                }
            })
            .unwrap();

        let fetched = ps.lock().unwrap().get_plan(&plan_id).unwrap().unwrap();
        assert!(!matches!(fetched.status, PlanStatus::Executing));
        assert_eq!(fetched.status, PlanStatus::Failed);
    }

    #[test]
    fn test_failed_plan_records_no_execution_completed() {
        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps, Some(es.clone()));

        executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: false,
                    output: None,
                    error: Some("fail".to_string()),
                    duration_ms: 0,
                    deviation: None,
                }
            })
            .unwrap();

        let events = es.list_events_by_run(&run_id).unwrap();
        assert!(events
            .iter()
            .any(|e| e.event_type == AgentRunEventType::PlanExecutionFailed));
        assert!(!events
            .iter()
            .any(|e| e.event_type == AgentRunEventType::PlanExecutionCompleted));
    }

    // ── P4-S3: Real Review Gate tests ──────────────────────────────────

    #[test]
    fn test_execute_with_review_approved_keeps_completed() {
        let (ps, es, run_id) = setup();
        // Medium-risk: review_required = true
        let mut plan = AgentPlan::new("medium-risk task", RiskLevel::Medium);
        plan.steps = vec![PlanStep {
            index: 0,
            description: "read".to_string(),
            tool_intent: Some("life_model.read".to_string()),
            expected_output: None,
            depends_on: vec![],
        }];
        plan.tool_intents = vec![ToolIntent {
            tool_name: "life_model.read".to_string(),
            purpose: "read".to_string(),
            risk_level: RiskLevel::Low,
            is_write: false,
            parameters_summary: None,
        }];
        plan.publish();
        plan.confirm();
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps.clone(), Some(es.clone()));
        let gate = DefaultPlanReviewGate;

        // execute_with_review on medium-risk plan: review gate runs, approves.
        let outcome = executor
            .execute_with_review(
                &plan_id,
                &run_id,
                |_step, _intent| PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: true,
                    output: Some("ok".to_string()),
                    error: None,
                    duration_ms: 1,
                    deviation: None,
                },
                &gate,
            )
            .unwrap();

        assert!(outcome.success);
        let fetched = ps.lock().unwrap().get_plan(&plan_id).unwrap().unwrap();
        assert_eq!(fetched.status, PlanStatus::Completed);

        let events = es.list_events_by_run(&run_id).unwrap();
        assert!(events
            .iter()
            .any(|e| e.summary.contains("review agent verdict")));
    }

    #[test]
    fn test_execute_with_review_critical_causes_failed_review() {
        use crate::agent::{ReviewAgentOutput, ReviewIssue};

        let (ps, es, run_id) = setup();
        let mut plan = AgentPlan::new("high-risk task", RiskLevel::High);
        plan.steps = vec![PlanStep {
            index: 0,
            description: "write step".to_string(),
            tool_intent: Some("file.write_proposal".to_string()),
            expected_output: None,
            depends_on: vec![],
        }];
        plan.tool_intents = vec![ToolIntent {
            tool_name: "file.write_proposal".to_string(),
            purpose: "write".to_string(),
            risk_level: RiskLevel::High,
            is_write: true,
            parameters_summary: None,
        }];
        plan.publish();
        plan.confirm();
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps.clone(), Some(es.clone()));

        // Custom review gate that always returns critical failure.
        struct CriticalGate;
        impl PlanReviewGate for CriticalGate {
            fn review(
                &self,
                _plan: &AgentPlan,
                _outcome: &PlanExecutionOutcome,
            ) -> Result<ReviewAgentOutput, PlanExecutionError> {
                Ok(ReviewAgentOutput::needs_changes(
                    "critical data integrity violation".to_string(),
                    vec![ReviewIssue {
                        severity: "critical".to_string(),
                        category: "data_integrity".to_string(),
                        description: "output hash mismatch".to_string(),
                        suggestion: Some("regenerate".to_string()),
                    }],
                    vec![],
                ))
            }
        }
        let gate = CriticalGate;

        let result = executor.execute_with_review(
            &plan_id,
            &run_id,
            |_step, _intent| PlanStepExecutionResult {
                step_index: 0,
                tool_name: "file.write_proposal".to_string(),
                success: true,
                output: Some("wrote".to_string()),
                error: None,
                duration_ms: 1,
                deviation: None,
            },
            &gate,
        );
        assert!(result.is_err());

        let fetched = ps.lock().unwrap().get_plan(&plan_id).unwrap().unwrap();
        assert_eq!(fetched.status, PlanStatus::FailedReview);

        let events = es.list_events_by_run(&run_id).unwrap();
        // Review gate event must be present.
        assert!(events
            .iter()
            .any(|e| e.summary.contains("review agent verdict")
                && e.summary.contains("needs_changes")));
        // Terminal failure event must be recorded.
        let failure_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e.event_type, AgentRunEventType::PlanExecutionFailed))
            .collect();
        assert_eq!(failure_events.len(), 1);
        assert!(failure_events[0].summary.contains("failed review"));
        assert_eq!(failure_events[0].payload["reason"], "review_failed");
        // plan.execution_completed must NOT be present.
        assert!(!events
            .iter()
            .any(|e| matches!(e.event_type, AgentRunEventType::PlanExecutionCompleted)));
    }

    #[test]
    fn test_execute_with_review_low_risk_skips_gate() {
        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps.clone(), Some(es.clone()));
        let gate = DefaultPlanReviewGate;

        // Low-risk: review_required = false → gate is skipped entirely.
        let outcome = executor
            .execute_with_review(
                &plan_id,
                &run_id,
                |_step, _intent| PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: true,
                    output: Some("ok".to_string()),
                    error: None,
                    duration_ms: 1,
                    deviation: None,
                },
                &gate,
            )
            .unwrap();

        assert!(outcome.success);
        let events = es.list_events_by_run(&run_id).unwrap();
        // No review gate event should appear for low-risk plans.
        assert!(!events.iter().any(|e| e.summary.contains("review agent")));
    }

    // ── P4-5: Review Gate tests ────────────────────────────────────────

    #[test]
    fn test_approved_review_allows_completed_status() {
        use crate::agent::ReviewAgentOutput;

        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps, Some(es.clone()));

        // Execute successfully first.
        executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: true,
                    output: Some("ok".to_string()),
                    error: None,
                    duration_ms: 1,
                    deviation: None,
                }
            })
            .unwrap();

        // Review: approved — should stay completed.
        let review = ReviewAgentOutput::approved("all good".to_string());
        let result = executor.review_execution(&plan_id, &run_id, &review);
        assert!(result.is_ok());

        let events = es.list_events_by_run(&run_id).unwrap();
        let review_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == AgentRunEventType::ObservationCreated)
            .filter(|e| e.summary.contains("review agent"))
            .collect();
        assert_eq!(review_events.len(), 1);
        assert!(review_events[0].summary.contains("approved"));
    }

    #[test]
    fn test_critical_issue_leaves_plan_failed_review() {
        use crate::agent::{ReviewAgentOutput, ReviewIssue};

        let (ps, es, run_id) = setup();
        let plan = create_high_risk_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps, Some(es.clone()));

        // Execute successfully first.
        executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "file.write_proposal".to_string(),
                    success: true,
                    output: Some("wrote file".to_string()),
                    error: None,
                    duration_ms: 1,
                    deviation: None,
                }
            })
            .unwrap();

        // Review: critical issue — should fail review.
        let review = ReviewAgentOutput::needs_changes(
            "data integrity violation detected".to_string(),
            vec![ReviewIssue {
                severity: "critical".to_string(),
                category: "data_integrity".to_string(),
                description: "file content mismatches expected hash".to_string(),
                suggestion: Some("regenerate with correct payload".to_string()),
            }],
            vec!["no positive findings".to_string()],
        );
        assert!(review.has_critical_issues());

        let result = executor.review_execution(&plan_id, &run_id, &review);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("failed review"));

        let events = es.list_events_by_run(&run_id).unwrap();
        let review_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == AgentRunEventType::ObservationCreated)
            .filter(|e| e.summary.contains("review agent"))
            .collect();
        assert_eq!(review_events.len(), 1);
        assert!(review_events[0].summary.contains("needs_changes"));
    }

    #[test]
    fn test_review_gate_records_parent_trace_event() {
        use crate::agent::ReviewAgentOutput;

        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps, Some(es.clone()));

        executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: true,
                    output: Some("ok".to_string()),
                    error: None,
                    duration_ms: 1,
                    deviation: None,
                }
            })
            .unwrap();

        let review = ReviewAgentOutput::approved("verified".to_string());
        executor
            .review_execution(&plan_id, &run_id, &review)
            .unwrap();

        let events = es.list_events_by_run(&run_id).unwrap();
        // Verify the review observation carries structured output in payload.
        let review_event = events
            .iter()
            .find(|e| {
                e.event_type == AgentRunEventType::ObservationCreated
                    && e.summary.contains("review agent")
            })
            .unwrap();
        let payload = &review_event.payload;
        assert_eq!(payload["verdict"], "approved");
        assert_eq!(payload["issue_count"], 0);
        assert!(!payload["has_critical"].as_bool().unwrap());
    }

    // ── P5-S1: Cancellation safety tests ──────────────────────────────

    #[test]
    fn test_cancelled_during_execution_prevents_completion() {
        let (ps, es, run_id) = setup();
        let mut plan = create_read_only_plan(true);
        plan.steps = vec![PlanStep {
            index: 0,
            description: "step 0".to_string(),
            tool_intent: Some("life_model.read".to_string()),
            expected_output: None,
            depends_on: vec![],
        }];
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(ps.clone(), Some(es.clone()));

        // Cancel the plan after execution starts (simulate external cancellation).
        {
            let mut p = ps.lock().unwrap().get_plan(&plan_id).unwrap().unwrap();
            p.start_execution();
            p.cancel();
            ps.lock().unwrap().update_plan(&p).unwrap();
        }

        // execute_steps_without_completion should detect cancellation and stop.
        let result =
            executor.execute_steps_without_completion(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: true,
                    output: Some("ok".to_string()),
                    error: None,
                    duration_ms: 1,
                    deviation: None,
                }
            });
        let (_plan, outcome) = result.unwrap();
        assert!(!outcome.success);

        // execute() should NOT complete a cancelled plan.
        let result2 = executor.execute(&plan_id, &run_id, |_step, _intent| {
            PlanStepExecutionResult {
                step_index: 0,
                tool_name: "life_model.read".to_string(),
                success: true,
                output: Some("ok".to_string()),
                error: None,
                duration_ms: 1,
                deviation: None,
            }
        });
        let outcome2 = result2.unwrap();
        assert!(
            !outcome2.success,
            "execute() should not complete a cancelled plan"
        );

        // Plan store must still show Cancelled, not Completed.
        let fetched = ps.lock().unwrap().get_plan(&plan_id).unwrap().unwrap();
        assert_eq!(fetched.status, PlanStatus::Cancelled);
    }

    #[test]
    fn test_cancelled_execution_does_not_record_completed() {
        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        // Cancel before execution.
        {
            let mut p = ps.lock().unwrap().get_plan(&plan_id).unwrap().unwrap();
            p.cancel();
            ps.lock().unwrap().update_plan(&p).unwrap();
        }

        let executor = PlanExecutor::new(ps, Some(es.clone()));

        executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: true,
                    output: Some("ok".to_string()),
                    error: None,
                    duration_ms: 1,
                    deviation: None,
                }
            })
            .unwrap();

        let events = es.list_events_by_run(&run_id).unwrap();
        assert!(!events
            .iter()
            .any(|e| matches!(e.event_type, AgentRunEventType::PlanExecutionCompleted)));
    }

    // ── P6-6: AgentSpec tool policy enforcement ────────────────────────

    #[test]
    fn test_agentspec_allowed_tool_executes() {
        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        // AgentSpec allows life_model.read
        let spec = AgentSpec::default().with_allowed_tools(vec!["life_model.read".to_string()]);
        let executor = PlanExecutor::new(ps, Some(es.clone())).with_agent_spec(spec);

        let outcome = executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: true,
                    output: Some("ok".to_string()),
                    error: None,
                    duration_ms: 0,
                    deviation: None,
                }
            })
            .unwrap();

        assert!(outcome.success);
    }

    #[test]
    fn test_agentspec_denied_tool_blocked_with_event() {
        let (ps, es, run_id) = setup();
        let plan = create_read_only_plan(true);
        let plan_id = plan.id.clone();
        ps.lock().unwrap().create_plan(&plan).unwrap();

        // AgentSpec denies life_model.read
        let spec = AgentSpec::default().with_denied_tools(vec!["life_model.read".to_string()]);
        let executor = PlanExecutor::new(ps, Some(es.clone())).with_agent_spec(spec);

        let outcome = executor
            .execute(&plan_id, &run_id, |_step, _intent| {
                PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: true,
                    output: Some("ok".to_string()),
                    error: None,
                    duration_ms: 0,
                    deviation: None,
                }
            })
            .unwrap();

        // Step was blocked by AgentSpec — plan should not complete.
        assert!(!outcome.success);

        let events = es.list_events_by_run(&run_id).unwrap();
        assert!(events
            .iter()
            .any(|e| e.summary.contains("blocked by AgentSpec")));
    }
}
