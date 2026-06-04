use crate::agent::governor::{
    GovernanceDecision, GovernanceDecisionKind, GovernanceSubject, LifeModelGovernor,
    ToolGovernanceInput,
};
use crate::agent::hs_selector::{
    build_guidance_impact_read_model, GuidanceAffectedSurface, GuidanceImpactReadModel,
};
use crate::agent::runtime_contract::{RuntimeInput, RuntimeOutput};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use crate::agent::ProposalStore;
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

pub const WEEKLY_PLANNING_MAX_STEP_COUNT: usize = 5;
const PRODUCT_STEP_TITLE_MAX_LEN: usize = 96;
const PRODUCT_PROPOSAL_PAYLOAD_MAX_BYTES: usize = 2048;

#[derive(Debug, Clone)]
pub struct PlanExecuteInput {
    pub runtime_input: RuntimeInput,
    pub objective: String,
    pub max_steps: usize,
}

impl PlanExecuteInput {
    pub fn from_runtime_input(
        runtime_input: RuntimeInput,
        objective: impl Into<String>,
        max_steps: usize,
    ) -> Self {
        Self {
            runtime_input,
            objective: objective.into(),
            max_steps,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDraft {
    pub objective: String,
    pub steps: Vec<PlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub id: String,
    pub title: String,
    pub intent: String,
    pub tool_name: Option<String>,
    pub action_kind: String,
    pub risk_level: RiskLevel,
    pub declared_write: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanExecuteProductScenario {
    WeeklyPlanning,
}

impl PlanExecuteProductScenario {
    pub fn as_id(self) -> &'static str {
        match self {
            PlanExecuteProductScenario::WeeklyPlanning => "weekly_planning",
        }
    }

    pub fn try_from_id(id: &str) -> std::result::Result<Self, PlanExecuteProductContractReport> {
        match id {
            "weekly_planning" => Ok(PlanExecuteProductScenario::WeeklyPlanning),
            _ => Err(PlanExecuteProductContractReport::blocked(
                "unsupported_scenario",
                "unknown",
                0,
            )),
        }
    }
}

impl std::fmt::Display for PlanExecuteProductScenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_id())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteProductContract {
    pub scenario: PlanExecuteProductScenario,
    pub max_step_count: usize,
    pub allowed_action_kinds: Vec<String>,
    pub allowed_risk_levels: Vec<RiskLevel>,
    pub proposal_first_write_boundary: bool,
    pub metadata_safe_summary: Value,
}

impl PlanExecuteProductContract {
    pub fn weekly_planning() -> Self {
        Self {
            scenario: PlanExecuteProductScenario::WeeklyPlanning,
            max_step_count: WEEKLY_PLANNING_MAX_STEP_COUNT,
            allowed_action_kinds: vec![
                "reason".into(),
                "search".into(),
                "plan".into(),
                "schedule".into(),
                "create".into(),
                "update".into(),
            ],
            allowed_risk_levels: vec![RiskLevel::Low, RiskLevel::Medium],
            proposal_first_write_boundary: true,
            metadata_safe_summary: json!({
                "contractKind": "plan_execute_product_vertical",
                "scenarioId": "weekly_planning",
                "maxStepCount": WEEKLY_PLANNING_MAX_STEP_COUNT,
                "allowedActionKinds": ["reason", "search", "plan", "schedule", "create", "update"],
                "allowedRiskLevels": ["low", "medium"],
                "proposalFirstWriteBoundary": true,
                "directWritesAllowed": false,
                "externalSideEffectsAllowed": false,
                "rawContentStoredInReports": false,
            }),
        }
    }

    pub fn evaluate_draft(
        &self,
        draft: &PlanDraft,
    ) -> std::result::Result<PlanExecuteProductContractReport, PlanExecuteProductContractReport>
    {
        if draft.steps.len() > self.max_step_count {
            return Err(PlanExecuteProductContractReport::blocked(
                "step_count_exceeds_contract",
                self.scenario.as_id(),
                draft.steps.len(),
            ));
        }

        for step in &draft.steps {
            if !self.allowed_action_kinds.contains(&step.action_kind) {
                return Err(PlanExecuteProductContractReport::blocked(
                    "unsupported_action_kind",
                    self.scenario.as_id(),
                    draft.steps.len(),
                ));
            }
            if step.declared_write
                && matches!(step.risk_level, RiskLevel::High | RiskLevel::Critical)
            {
                return Err(PlanExecuteProductContractReport::blocked(
                    "direct_write_risk_exceeds_contract",
                    self.scenario.as_id(),
                    draft.steps.len(),
                ));
            }
            if !self.allowed_risk_levels.contains(&step.risk_level) {
                return Err(PlanExecuteProductContractReport::blocked(
                    "unsupported_risk_level",
                    self.scenario.as_id(),
                    draft.steps.len(),
                ));
            }
        }

        Ok(PlanExecuteProductContractReport {
            ready: true,
            scenario_id: self.scenario.as_id().into(),
            step_count: draft.steps.len(),
            reason_code: "contract_ready".into(),
            metadata_safe_summary: json!({
                "contractKind": "plan_execute_product_vertical",
                "scenarioId": self.scenario.as_id(),
                "stepCount": draft.steps.len(),
                "maxStepCount": self.max_step_count,
                "proposalFirstWriteBoundary": self.proposal_first_write_boundary,
                "directWritesAllowed": false,
                "externalSideEffectsAllowed": false,
                "rawContentStoredInReports": false,
            }),
        })
    }

    pub fn tools_authority_report(
        &self,
        input: &RuntimeInput,
    ) -> PlanExecuteProductAuthorityReport {
        PlanExecuteProductAuthorityReport {
            scenario_id: self.scenario.as_id().into(),
            tools_prompt_present: !input.tools_prompt.trim().is_empty(),
            metadata_safe_summary: json!({
                "reportKind": "plan_execute_product_tools_authority",
                "scenarioId": self.scenario.as_id(),
                "toolsPromptPresent": !input.tools_prompt.trim().is_empty(),
                "toolsPromptAuthority": "descriptive_only",
                "directWritesAllowed": false,
                "externalSideEffectsAllowed": false,
                "proposalFirstWriteBoundary": true,
                "rawToolsPromptStored": false,
                "rawRuntimeInputStored": false,
            }),
        }
    }

    pub fn metadata_safe_report(&self, input: &RuntimeInput) -> PlanExecuteProductAuthorityReport {
        PlanExecuteProductAuthorityReport {
            scenario_id: self.scenario.as_id().into(),
            tools_prompt_present: !input.tools_prompt.trim().is_empty(),
            metadata_safe_summary: json!({
                "reportKind": "plan_execute_product_contract",
                "scenarioId": self.scenario.as_id(),
                "taskKind": input.task.kind.to_string(),
                "maxStepCount": self.max_step_count,
                "hasHsPacket": input.hs_packet.is_some(),
                "selectedGuidanceCount": selected_guidance_count(input),
                "guidanceImpactKinds": selected_guidance_impact_kinds(input),
                "toolsPromptPresent": !input.tools_prompt.trim().is_empty(),
                "memoryContextPresent": input.memory_context.is_some(),
                "proposalFirstWriteBoundary": true,
                "directWritesAllowed": false,
                "externalSideEffectsAllowed": false,
                "rawPromptStored": false,
                "rawAssistantOutputStored": false,
                "rawLifeModelStored": false,
                "rawMemoryStored": false,
                "rawToolPayloadStored": false,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteProductContractReport {
    pub ready: bool,
    pub scenario_id: String,
    pub step_count: usize,
    pub reason_code: String,
    pub metadata_safe_summary: Value,
}

impl PlanExecuteProductContractReport {
    fn blocked(reason_code: &str, scenario_id: &str, step_count: usize) -> Self {
        Self {
            ready: false,
            scenario_id: scenario_id.into(),
            step_count,
            reason_code: reason_code.into(),
            metadata_safe_summary: json!({
                "contractKind": "plan_execute_product_vertical",
                "scenarioId": scenario_id,
                "stepCount": step_count,
                "ready": false,
                "reasonCode": reason_code,
                "metadataSafe": true,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteProductAuthorityReport {
    pub scenario_id: String,
    pub tools_prompt_present: bool,
    pub metadata_safe_summary: Value,
}

impl PlanStep {
    pub fn to_tool_governance_input(&self) -> ToolGovernanceInput {
        ToolGovernanceInput {
            tool_name: self
                .tool_name
                .clone()
                .unwrap_or_else(|| "runtime.reasoning".into()),
            action_kind: self.action_kind.clone(),
            risk_level: self.risk_level,
            declared_write: self.declared_write,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStepTrace {
    pub step_id: String,
    pub decision: GovernanceDecision,
    pub status: PlanStepStatus,
    pub output_summary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatus {
    Planned,
    Skipped,
    Blocked,
    RequiresProposal,
    RequiresConfirmation,
    Executed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanGovernanceDecisionSummary {
    pub step_id: String,
    pub subject: String,
    pub decision_kind: GovernanceDecisionKind,
    pub risk_level: RiskLevel,
    pub policy_reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanObservationSummary {
    pub step_id: String,
    pub source: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteReport {
    pub plan_id: String,
    pub source_run_id: Option<String>,
    pub step_count: usize,
    pub executed_read_only_step_count: usize,
    pub blocked_or_proposal_required_step_count: usize,
    pub governance_decisions: Vec<PlanGovernanceDecisionSummary>,
    pub observation_summaries: Vec<PlanObservationSummary>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub guidance_impact: Option<Box<GuidanceImpactReadModel>>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecutionOutput {
    pub report: PlanExecuteReport,
    pub plan: PlanDraft,
    pub traces: Vec<PlanStepTrace>,
    pub runtime_outputs: Vec<RuntimeOutput>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PlanExecuteService;

impl PlanExecuteService {
    pub fn draft_plan(&self, input: &PlanExecuteInput) -> PlanDraft {
        let mut steps = Vec::new();
        let user_text = input.runtime_input.task.user_text.to_ascii_lowercase();

        if contains_search_intent(&user_text) {
            push_step(
                &mut steps,
                input.max_steps,
                PlanStepSpec {
                    title: "Read relevant context",
                    intent: "read_only_search",
                    tool_name: Some("memory.search"),
                    action_kind: "search",
                    risk_level: RiskLevel::Low,
                    declared_write: false,
                },
            );
        }

        if let Some(action_kind) = write_action_kind(&user_text) {
            push_step(
                &mut steps,
                input.max_steps,
                PlanStepSpec {
                    title: "Prepare write proposal",
                    intent: "write_like_external_action",
                    tool_name: Some("external.write_proposal"),
                    action_kind,
                    risk_level: RiskLevel::Medium,
                    declared_write: true,
                },
            );
        }

        if steps.is_empty() && input.max_steps > 0 {
            push_step(
                &mut steps,
                input.max_steps,
                PlanStepSpec {
                    title: "Reason about objective",
                    intent: "read_only_reasoning",
                    tool_name: None,
                    action_kind: "reason",
                    risk_level: RiskLevel::Low,
                    declared_write: false,
                },
            );
        }

        PlanDraft {
            objective: input.objective.clone(),
            steps,
        }
    }

    pub fn execute_plan(
        &self,
        input: PlanExecuteInput,
        governor: &LifeModelGovernor,
    ) -> PlanExecutionOutput {
        let source_run_id = source_run_id(&input.runtime_input);
        let plan = self.draft_plan(&input);
        let plan_id = format!("plan-{}", Uuid::new_v4());
        let mut traces = Vec::with_capacity(plan.steps.len());
        let mut governance_decisions = Vec::with_capacity(plan.steps.len());
        let mut observation_summaries = Vec::new();
        let mut warnings = Vec::new();

        if input.max_steps == 0 {
            warnings.push("plan execution skipped because max_steps=0".into());
        }

        for step in &plan.steps {
            let decision = governor.govern_tool_action(step.to_tool_governance_input());
            let status = step_status(step, decision.kind);
            let output_summary = if status == PlanStepStatus::Executed {
                let observation = execute_internal_read_only_step(step);
                let summary = observation.summary.clone();
                observation_summaries.push(observation);
                Some(summary)
            } else {
                Some(metadata_safe_step_summary(step, &decision))
            };

            governance_decisions.push(metadata_safe_governance_summary(step, &decision));
            warnings.extend(decision.warnings.iter().cloned());
            traces.push(PlanStepTrace {
                step_id: step.id.clone(),
                output_summary,
                decision,
                status,
            });
        }

        let guidance_impact = if input.runtime_input.guidance_consumption_mode.is_enabled() {
            input.runtime_input.hs_packet.as_ref().map(|packet| {
                Box::new(build_guidance_impact_read_model(
                    source_run_id.as_deref(),
                    "plan_execute",
                    packet,
                    vec![GuidanceAffectedSurface::PlanExecuteTrace],
                ))
            })
        } else {
            None
        };
        let report = PlanExecuteReport::new(
            plan_id,
            source_run_id,
            &traces,
            governance_decisions,
            observation_summaries,
            warnings.clone(),
            guidance_impact,
        );

        PlanExecutionOutput {
            report,
            plan,
            traces,
            runtime_outputs: Vec::new(),
            warnings,
        }
    }

    pub fn draft_product_plan(
        &self,
        input: &PlanExecuteInput,
        scenario: PlanExecuteProductScenario,
    ) -> PlanDraft {
        match scenario {
            PlanExecuteProductScenario::WeeklyPlanning => draft_weekly_planning_plan(input),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanExecuteSessionStatus {
    Draft,
    Finalized,
    InProgress,
    Completed,
    Cancelled,
}

impl std::fmt::Display for PlanExecuteSessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanExecuteSessionStatus::Draft => write!(f, "draft"),
            PlanExecuteSessionStatus::Finalized => write!(f, "finalized"),
            PlanExecuteSessionStatus::InProgress => write!(f, "in_progress"),
            PlanExecuteSessionStatus::Completed => write!(f, "completed"),
            PlanExecuteSessionStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteStepRecord {
    pub step_id: String,
    pub order: usize,
    pub title: String,
    pub intent: String,
    pub tool_name: Option<String>,
    pub action_kind: String,
    pub risk_level: RiskLevel,
    pub declared_write: bool,
    pub status: PlanStepStatus,
    pub linked_proposal_id: Option<String>,
    pub observation_summary: Option<String>,
    pub policy_reason_code: Option<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteStepEdit {
    pub step_id: String,
    pub title: Option<String>,
    pub intent: Option<String>,
    pub action_kind: Option<String>,
    pub tool_name: Option<Option<String>>,
    pub declared_write: Option<bool>,
    pub risk_level: Option<RiskLevel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteStepExecutionResult {
    pub session_id: String,
    pub step_id: String,
    pub step_status: PlanStepStatus,
    pub linked_proposal_id: Option<String>,
    pub observation_summary: Option<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteSession {
    pub session_id: String,
    pub source_agent_run_id: Option<String>,
    pub source_chat_session_id: Option<String>,
    pub scenario: PlanExecuteProductScenario,
    pub status: PlanExecuteSessionStatus,
    pub created_at: String,
    pub updated_at: String,
    pub finalized_at: Option<String>,
    pub metadata_safe_objective: String,
    pub step_count: usize,
    pub completed_step_count: usize,
    pub proposal_required_step_count: usize,
    pub linked_proposal_ids: Vec<String>,
    pub warnings: Vec<String>,
    pub steps: Vec<PlanExecuteStepRecord>,
    pub metadata_safe_summary: Value,
}

impl PlanExecuteSession {
    pub fn new_draft(
        source_chat_session_id: Option<String>,
        source_agent_run_id: Option<String>,
        contract: PlanExecuteProductContract,
        draft: PlanDraft,
    ) -> Result<Self> {
        contract.evaluate_draft(&draft).map_err(|report| {
            anyhow::anyhow!("Plan-Execute contract blocked: {}", report.reason_code)
        })?;
        let now = Utc::now().to_rfc3339();
        let session_id = format!("plan-session-{}", Uuid::new_v4());
        let steps: Vec<PlanExecuteStepRecord> = draft
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| PlanExecuteStepRecord::from_plan_step(step, index + 1))
            .collect();
        let mut session = Self {
            session_id,
            source_agent_run_id,
            source_chat_session_id,
            scenario: contract.scenario,
            status: PlanExecuteSessionStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            finalized_at: None,
            metadata_safe_objective: draft.objective,
            step_count: steps.len(),
            completed_step_count: 0,
            proposal_required_step_count: steps.iter().filter(|step| step.declared_write).count(),
            linked_proposal_ids: Vec::new(),
            warnings: Vec::new(),
            steps,
            metadata_safe_summary: Value::Null,
        };
        session.refresh_counts_and_summary();
        Ok(session)
    }

    pub fn apply_draft_edits(&mut self, edits: Vec<PlanExecuteStepEdit>) -> Result<()> {
        if self.status != PlanExecuteSessionStatus::Draft {
            return Err(anyhow::anyhow!("Plan-Execute session is not editable"));
        }
        let contract = PlanExecuteProductContract::weekly_planning();
        for edit in edits {
            let step = self
                .steps
                .iter_mut()
                .find(|step| step.step_id == edit.step_id)
                .ok_or_else(|| anyhow::anyhow!("Plan-Execute step not found"))?;
            if let Some(title) = edit.title {
                validate_step_title(&title)?;
                step.title = title;
            }
            if let Some(intent) = edit.intent {
                validate_step_intent(&intent)?;
                step.intent = intent;
            }
            if let Some(action_kind) = edit.action_kind {
                validate_action_kind(&contract, &action_kind)?;
                step.action_kind = action_kind;
            }
            if let Some(tool_name) = edit.tool_name {
                step.tool_name = tool_name.filter(|value| !value.trim().is_empty());
            }
            if let Some(declared_write) = edit.declared_write {
                step.declared_write = declared_write;
            }
            if let Some(risk_level) = edit.risk_level {
                validate_risk_level(&contract, risk_level)?;
                step.risk_level = risk_level;
            }
            validate_step_record(&contract, step)?;
            step.metadata_safe_summary = step_record_summary(step);
        }
        self.touch();
        self.refresh_counts_and_summary();
        Ok(())
    }

    pub fn finalize(&mut self) -> Result<()> {
        if self.status != PlanExecuteSessionStatus::Draft {
            return Err(anyhow::anyhow!(
                "Plan-Execute session cannot be finalized from current status"
            ));
        }
        let contract = PlanExecuteProductContract::weekly_planning();
        let draft = self.to_plan_draft();
        contract.evaluate_draft(&draft).map_err(|report| {
            anyhow::anyhow!("Plan-Execute contract blocked: {}", report.reason_code)
        })?;
        let now = Utc::now().to_rfc3339();
        self.status = PlanExecuteSessionStatus::Finalized;
        self.finalized_at = Some(now.clone());
        self.updated_at = now;
        self.refresh_counts_and_summary();
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<()> {
        if matches!(
            self.status,
            PlanExecuteSessionStatus::Completed | PlanExecuteSessionStatus::Cancelled
        ) {
            return Err(anyhow::anyhow!(
                "Plan-Execute session cannot be cancelled from current status"
            ));
        }
        self.status = PlanExecuteSessionStatus::Cancelled;
        self.touch();
        self.refresh_counts_and_summary();
        Ok(())
    }

    pub fn execute_step(
        &mut self,
        step_id: &str,
        governor: &LifeModelGovernor,
        proposal_store: &ProposalStore,
    ) -> Result<PlanExecuteStepExecutionResult> {
        if !matches!(
            self.status,
            PlanExecuteSessionStatus::Finalized | PlanExecuteSessionStatus::InProgress
        ) {
            return Err(anyhow::anyhow!(
                "Plan-Execute session must be finalized before execution"
            ));
        }

        let session_id = self.session_id.clone();
        let source_run_id = self.source_agent_run_id.clone();
        let mut linked_proposal_id_to_add = None;
        {
            let step = self
                .steps
                .iter_mut()
                .find(|step| step.step_id == step_id)
                .ok_or_else(|| anyhow::anyhow!("Plan-Execute step not found"))?;

            if matches!(
                step.status,
                PlanStepStatus::Executed | PlanStepStatus::Blocked
            ) || step.linked_proposal_id.is_some()
            {
                return Ok(step_execution_result(&session_id, step));
            }

            let plan_step = step.to_plan_step();
            let decision = governor.govern_tool_action(plan_step.to_tool_governance_input());
            let status = step_status(&plan_step, decision.kind);
            step.policy_reason_code = Some(policy_reason_code(&decision).into());

            if plan_step.declared_write || status == PlanStepStatus::RequiresProposal {
                let proposal_id = create_step_proposal(
                    &session_id,
                    source_run_id.as_deref(),
                    step,
                    proposal_store,
                )?;
                step.status = PlanStepStatus::RequiresProposal;
                step.linked_proposal_id = Some(proposal_id.clone());
                linked_proposal_id_to_add = Some(proposal_id);
            } else if status == PlanStepStatus::Executed {
                let observation = execute_internal_read_only_step(&plan_step);
                step.status = PlanStepStatus::Executed;
                step.observation_summary = Some(observation.summary);
            } else {
                step.status = status;
                step.observation_summary = Some(metadata_safe_step_summary(&plan_step, &decision));
            }

            step.metadata_safe_summary = step_record_summary(step);
        }

        if let Some(proposal_id) = linked_proposal_id_to_add {
            push_unique(&mut self.linked_proposal_ids, proposal_id);
        }
        self.status = PlanExecuteSessionStatus::InProgress;
        self.touch();
        self.refresh_counts_and_summary();
        if self.steps.iter().all(is_terminal_product_step) {
            self.status = PlanExecuteSessionStatus::Completed;
            self.touch();
            self.refresh_counts_and_summary();
        }

        let step = self
            .steps
            .iter()
            .find(|step| step.step_id == step_id)
            .expect("step exists after execution");
        Ok(step_execution_result(&self.session_id, step))
    }

    pub fn to_plan_draft(&self) -> PlanDraft {
        PlanDraft {
            objective: self.metadata_safe_objective.clone(),
            steps: self
                .steps
                .iter()
                .map(PlanExecuteStepRecord::to_plan_step)
                .collect(),
        }
    }

    fn touch(&mut self) {
        self.updated_at = Utc::now().to_rfc3339();
    }

    fn refresh_counts_and_summary(&mut self) {
        self.step_count = self.steps.len();
        self.completed_step_count = self
            .steps
            .iter()
            .filter(|step| step.status == PlanStepStatus::Executed)
            .count();
        self.proposal_required_step_count = self
            .steps
            .iter()
            .filter(|step| step.declared_write || step.status == PlanStepStatus::RequiresProposal)
            .count();
        self.linked_proposal_ids = self
            .steps
            .iter()
            .filter_map(|step| step.linked_proposal_id.clone())
            .fold(Vec::new(), |mut ids, id| {
                push_unique(&mut ids, id);
                ids
            });
        self.metadata_safe_summary = json!({
            "planExecuteProductVertical": true,
            "scenarioId": self.scenario.as_id(),
            "planSessionId": self.session_id,
            "sourceAgentRunId": self.source_agent_run_id,
            "sourceChatSessionId": self.source_chat_session_id,
            "status": self.status.to_string(),
            "stepCount": self.step_count,
            "completedStepCount": self.completed_step_count,
            "proposalRequiredStepCount": self.proposal_required_step_count,
            "linkedProposalIds": self.linked_proposal_ids,
            "warningCount": self.warnings.len(),
            "rawPromptStored": false,
            "rawWeeklyPlanProseStoredInTrace": false,
            "directLifeModelWrites": false,
            "externalWritesExecuted": false,
        });
    }
}

impl PlanExecuteStepRecord {
    fn from_plan_step(step: &PlanStep, order: usize) -> Self {
        let mut record = Self {
            step_id: step.id.clone(),
            order,
            title: step.title.clone(),
            intent: step.intent.clone(),
            tool_name: step.tool_name.clone(),
            action_kind: step.action_kind.clone(),
            risk_level: step.risk_level,
            declared_write: step.declared_write,
            status: PlanStepStatus::Planned,
            linked_proposal_id: None,
            observation_summary: None,
            policy_reason_code: None,
            metadata_safe_summary: Value::Null,
        };
        record.metadata_safe_summary = step_record_summary(&record);
        record
    }

    fn to_plan_step(&self) -> PlanStep {
        PlanStep {
            id: self.step_id.clone(),
            title: self.title.clone(),
            intent: self.intent.clone(),
            tool_name: self.tool_name.clone(),
            action_kind: self.action_kind.clone(),
            risk_level: self.risk_level,
            declared_write: self.declared_write,
        }
    }
}

pub struct PlanExecuteSessionStore {
    conn: Mutex<Connection>,
}

impl PlanExecuteSessionStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open Plan-Execute sessions db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("failed to open in-memory Plan-Execute sessions db")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS plan_execute_sessions (
                id TEXT PRIMARY KEY,
                scenario TEXT NOT NULL,
                status TEXT NOT NULL,
                source_agent_run_id TEXT,
                source_chat_session_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                session_json TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_plan_execute_sessions_updated ON plan_execute_sessions(updated_at DESC)",
            [],
        )?;
        Ok(())
    }

    pub fn create_session(&self, session: &PlanExecuteSession) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO plan_execute_sessions (
                id, scenario, status, source_agent_run_id, source_chat_session_id,
                created_at, updated_at, session_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session.session_id,
                session.scenario.to_string(),
                session.status.to_string(),
                session.source_agent_run_id,
                session.source_chat_session_id,
                session.created_at,
                session.updated_at,
                serde_json::to_string(session)?,
            ],
        )?;
        Ok(())
    }

    pub fn update_session(&self, session: &PlanExecuteSession) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "UPDATE plan_execute_sessions SET
                status = ?2,
                source_agent_run_id = ?3,
                source_chat_session_id = ?4,
                updated_at = ?5,
                session_json = ?6
             WHERE id = ?1",
            params![
                session.session_id,
                session.status.to_string(),
                session.source_agent_run_id,
                session.source_chat_session_id,
                session.updated_at,
                serde_json::to_string(session)?,
            ],
        )?;
        Ok(())
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<PlanExecuteSession>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt =
            conn.prepare("SELECT session_json FROM plan_execute_sessions WHERE id = ?1")?;
        let row = stmt.query_row([session_id], |row| row.get::<_, String>(0));
        match row {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_sessions(&self, limit: i64) -> Result<Vec<PlanExecuteSession>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT session_json FROM plan_execute_sessions
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;
        let sessions = stmt.query_map([limit], |row| row.get::<_, String>(0))?;
        sessions
            .map(|row| {
                let json = row?;
                serde_json::from_str(&json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }
}

fn draft_weekly_planning_plan(input: &PlanExecuteInput) -> PlanDraft {
    let max_steps = input.max_steps.min(WEEKLY_PLANNING_MAX_STEP_COUNT);
    let mut steps = Vec::new();
    if has_gentle_planning_guidance(input) {
        push_step(
            &mut steps,
            max_steps,
            PlanStepSpec {
                title: "Choose one small weekly focus",
                intent: "read_only_planning",
                tool_name: None,
                action_kind: "plan",
                risk_level: RiskLevel::Low,
                declared_write: false,
            },
        );
        push_step(
            &mut steps,
            max_steps,
            PlanStepSpec {
                title: "Prepare lightweight weekly check-in proposal",
                intent: "write_like_schedule_task",
                tool_name: Some("review_center.propose_scheduled_task"),
                action_kind: "schedule",
                risk_level: RiskLevel::Medium,
                declared_write: true,
            },
        );

        return PlanDraft {
            objective: metadata_safe_weekly_objective(input),
            steps,
        };
    }

    push_step(
        &mut steps,
        max_steps,
        PlanStepSpec {
            title: "Review current priorities",
            intent: "read_only_reasoning",
            tool_name: None,
            action_kind: "reason",
            risk_level: RiskLevel::Low,
            declared_write: false,
        },
    );
    push_step(
        &mut steps,
        max_steps,
        PlanStepSpec {
            title: "Shape this week's focus",
            intent: "read_only_planning",
            tool_name: None,
            action_kind: "plan",
            risk_level: RiskLevel::Low,
            declared_write: false,
        },
    );
    push_step(
        &mut steps,
        max_steps,
        PlanStepSpec {
            title: "Prepare weekly check-in proposal",
            intent: "write_like_schedule_task",
            tool_name: Some("review_center.propose_scheduled_task"),
            action_kind: "schedule",
            risk_level: RiskLevel::Medium,
            declared_write: true,
        },
    );

    PlanDraft {
        objective: metadata_safe_weekly_objective(input),
        steps,
    }
}

fn metadata_safe_weekly_objective(input: &PlanExecuteInput) -> String {
    format!(
        "scenario=weekly_planning task_kind={} max_steps={} selected_guidance_count={}",
        input.runtime_input.task.kind,
        input.max_steps.min(WEEKLY_PLANNING_MAX_STEP_COUNT),
        selected_guidance_count(&input.runtime_input)
    )
}

fn validate_step_title(title: &str) -> Result<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(anyhow::anyhow!("Plan-Execute step title is required"));
    }
    if title.chars().count() > PRODUCT_STEP_TITLE_MAX_LEN {
        return Err(anyhow::anyhow!(
            "Plan-Execute step title exceeds product limit"
        ));
    }
    Ok(())
}

fn validate_step_intent(intent: &str) -> Result<()> {
    if matches!(
        intent,
        "read_only_reasoning"
            | "read_only_planning"
            | "read_only_search"
            | "write_like_schedule_task"
            | "write_like_external_action"
    ) {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Plan-Execute step intent is unsupported"))
    }
}

fn validate_action_kind(contract: &PlanExecuteProductContract, action_kind: &str) -> Result<()> {
    if contract
        .allowed_action_kinds
        .iter()
        .any(|kind| kind == action_kind)
    {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Plan-Execute action kind is unsupported"))
    }
}

fn validate_risk_level(contract: &PlanExecuteProductContract, risk_level: RiskLevel) -> Result<()> {
    if contract.allowed_risk_levels.contains(&risk_level) {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Plan-Execute risk level is unsupported"))
    }
}

fn validate_step_record(
    contract: &PlanExecuteProductContract,
    step: &PlanExecuteStepRecord,
) -> Result<()> {
    validate_step_title(&step.title)?;
    validate_step_intent(&step.intent)?;
    validate_action_kind(contract, &step.action_kind)?;
    validate_risk_level(contract, step.risk_level)?;
    if step.declared_write && matches!(step.risk_level, RiskLevel::High | RiskLevel::Critical) {
        return Err(anyhow::anyhow!(
            "Plan-Execute direct write risk exceeds product contract"
        ));
    }
    Ok(())
}

fn step_record_summary(step: &PlanExecuteStepRecord) -> Value {
    json!({
        "stepId": step.step_id,
        "order": step.order,
        "actionKind": step.action_kind,
        "riskLevel": step.risk_level.to_string(),
        "declaredWrite": step.declared_write,
        "status": format!("{:?}", step.status).to_ascii_lowercase(),
        "linkedProposalId": step.linked_proposal_id,
        "policyReasonCode": step.policy_reason_code,
        "metadataSafe": true,
        "rawPromptStored": false,
        "rawToolPayloadStored": false,
        "externalWritesExecuted": false,
    })
}

fn create_step_proposal(
    session_id: &str,
    source_run_id: Option<&str>,
    step: &PlanExecuteStepRecord,
    proposal_store: &ProposalStore,
) -> Result<String> {
    let payload = minimized_step_proposal_payload(session_id, step);
    let payload_len = serde_json::to_vec(&payload)?.len();
    if payload_len > PRODUCT_PROPOSAL_PAYLOAD_MAX_BYTES {
        return Err(anyhow::anyhow!(
            "Plan-Execute proposal payload exceeds product limit"
        ));
    }
    let proposal_type = step_proposal_type(step);
    let mut proposal = AgentProposal::new(
        proposal_type,
        &format!(
            "plan_execute.sessions.{}.steps.{}",
            session_id, step.step_id
        ),
        payload,
        "Plan-Execute weekly planning step requires Review Center approval.",
        0.8,
        step.risk_level,
        ProposalSource::PlanningSession,
    );
    proposal.run_id = source_run_id.map(str::to_string);
    proposal.source_detail = Some(format!("plan_execute_session:{}", session_id));
    let proposal_id = proposal.id.clone();
    proposal_store.create_proposal(&proposal)?;
    Ok(proposal_id)
}

fn minimized_step_proposal_payload(session_id: &str, step: &PlanExecuteStepRecord) -> Value {
    json!({
        "kind": "plan_execute_step_proposal",
        "scenarioId": "weekly_planning",
        "sessionId": session_id,
        "stepId": step.step_id,
        "stepOrder": step.order,
        "title": step.title,
        "actionKind": step.action_kind,
        "declaredWrite": step.declared_write,
        "metadataSafe": true,
        "rawProviderPayloadStored": false,
        "rawBodyStored": false,
        "externalWriteExecuted": false,
    })
}

fn step_proposal_type(step: &PlanExecuteStepRecord) -> ProposalType {
    if matches!(step.action_kind.as_str(), "schedule" | "create")
        || step.intent.contains("schedule")
    {
        ProposalType::ScheduledTask
    } else if step.intent.contains("memory") {
        ProposalType::MemoryWrite
    } else if step.intent.contains("lifemodel") || step.intent.contains("goal") {
        ProposalType::LifeModelUpdate
    } else {
        ProposalType::ExternalWriteAction
    }
}

fn is_terminal_product_step(step: &PlanExecuteStepRecord) -> bool {
    matches!(
        step.status,
        PlanStepStatus::Executed | PlanStepStatus::RequiresProposal | PlanStepStatus::Blocked
    ) || step.linked_proposal_id.is_some()
}

fn step_execution_result(
    session_id: &str,
    step: &PlanExecuteStepRecord,
) -> PlanExecuteStepExecutionResult {
    PlanExecuteStepExecutionResult {
        session_id: session_id.into(),
        step_id: step.step_id.clone(),
        step_status: step.status,
        linked_proposal_id: step.linked_proposal_id.clone(),
        observation_summary: step.observation_summary.clone(),
        metadata_safe_summary: json!({
            "planExecuteProductVertical": true,
            "scenarioId": "weekly_planning",
            "planSessionId": session_id,
            "stepId": step.step_id,
            "stepStatus": format!("{:?}", step.status).to_ascii_lowercase(),
            "linkedProposalId": step.linked_proposal_id,
            "observationSummaryPresent": step.observation_summary.is_some(),
            "metadataSafe": true,
            "directLifeModelWrites": false,
            "memoryWrites": false,
            "externalWritesExecuted": false,
        }),
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

struct PlanStepSpec {
    title: &'static str,
    intent: &'static str,
    tool_name: Option<&'static str>,
    action_kind: &'static str,
    risk_level: RiskLevel,
    declared_write: bool,
}

fn push_step(steps: &mut Vec<PlanStep>, max_steps: usize, spec: PlanStepSpec) {
    if steps.len() >= max_steps {
        return;
    }

    steps.push(PlanStep {
        id: format!("step-{}", steps.len() + 1),
        title: spec.title.into(),
        intent: spec.intent.into(),
        tool_name: spec.tool_name.map(String::from),
        action_kind: spec.action_kind.into(),
        risk_level: spec.risk_level,
        declared_write: spec.declared_write,
    });
}

fn contains_search_intent(lowercase_text: &str) -> bool {
    ["search", "查找", "检索"]
        .iter()
        .any(|needle| lowercase_text.contains(needle))
}

fn write_action_kind(lowercase_text: &str) -> Option<&'static str> {
    [
        ("write", "write"),
        ("create", "create"),
        ("update", "update"),
        ("send", "send"),
        ("schedule", "schedule"),
        ("写入", "write"),
        ("创建", "create"),
        ("更新", "update"),
        ("发送", "send"),
        ("安排", "schedule"),
    ]
    .iter()
    .find_map(|(needle, action_kind)| lowercase_text.contains(needle).then_some(*action_kind))
}

fn step_status(step: &PlanStep, kind: GovernanceDecisionKind) -> PlanStepStatus {
    match kind {
        GovernanceDecisionKind::Allow => {
            if !step.declared_write && step.risk_level == RiskLevel::Low {
                PlanStepStatus::Executed
            } else {
                PlanStepStatus::Planned
            }
        }
        GovernanceDecisionKind::RequireProposal => PlanStepStatus::RequiresProposal,
        GovernanceDecisionKind::RequireConfirmation => PlanStepStatus::RequiresConfirmation,
        GovernanceDecisionKind::RequireLocalOnly => PlanStepStatus::RequiresConfirmation,
        GovernanceDecisionKind::Block => PlanStepStatus::Blocked,
    }
}

impl PlanExecuteReport {
    fn new(
        plan_id: String,
        source_run_id: Option<String>,
        traces: &[PlanStepTrace],
        governance_decisions: Vec<PlanGovernanceDecisionSummary>,
        observation_summaries: Vec<PlanObservationSummary>,
        warnings: Vec<String>,
        guidance_impact: Option<Box<GuidanceImpactReadModel>>,
    ) -> Self {
        let step_count = traces.len();
        let executed_read_only_step_count = traces
            .iter()
            .filter(|trace| trace.status == PlanStepStatus::Executed)
            .count();
        let blocked_or_proposal_required_step_count = traces
            .iter()
            .filter(|trace| {
                matches!(
                    trace.status,
                    PlanStepStatus::Blocked | PlanStepStatus::RequiresProposal
                )
            })
            .count();
        let metadata_safe_summary = json!({
            "reportKind": "plan_execute_v1",
            "planId": plan_id,
            "sourceRunId": source_run_id,
            "stepCount": step_count,
            "executedReadOnlyStepCount": executed_read_only_step_count,
            "blockedOrProposalRequiredStepCount": blocked_or_proposal_required_step_count,
            "governanceDecisionCount": governance_decisions.len(),
            "observationSummaryCount": observation_summaries.len(),
            "selectedGuidanceCount": guidance_impact
                .as_ref()
                .map(|impact| impact.selected_guidance_count)
                .unwrap_or(0),
            "warningCount": warnings.len(),
        });

        Self {
            plan_id,
            source_run_id,
            step_count,
            executed_read_only_step_count,
            blocked_or_proposal_required_step_count,
            governance_decisions,
            observation_summaries,
            warnings,
            guidance_impact,
            metadata_safe_summary,
        }
    }
}

fn selected_guidance_count(input: &RuntimeInput) -> usize {
    if !input.guidance_consumption_mode.is_enabled() {
        return 0;
    }
    input
        .hs_packet
        .as_ref()
        .map(|packet| packet.guidance_refs.len())
        .unwrap_or(0)
}

fn selected_guidance_impact_kinds(input: &RuntimeInput) -> Vec<String> {
    if !input.guidance_consumption_mode.is_enabled() {
        return Vec::new();
    }
    input
        .hs_packet
        .as_ref()
        .map(|packet| {
            packet
                .guidance_refs
                .iter()
                .map(|guidance| guidance.impact_kind.clone())
                .fold(Vec::new(), |mut kinds, kind| {
                    push_unique(&mut kinds, kind);
                    kinds
                })
        })
        .unwrap_or_default()
}

fn has_gentle_planning_guidance(input: &PlanExecuteInput) -> bool {
    if !input.runtime_input.guidance_consumption_mode.is_enabled() {
        return false;
    }
    input
        .runtime_input
        .hs_packet
        .as_ref()
        .is_some_and(|packet| {
            packet
                .guidance_refs
                .iter()
                .any(|guidance| guidance.impact_kind == "gentle_planning")
        })
}

fn source_run_id(input: &RuntimeInput) -> Option<String> {
    input
        .hs_packet
        .as_ref()
        .and_then(|packet| packet.audit.agent_run_id.clone())
}

fn execute_internal_read_only_step(step: &PlanStep) -> PlanObservationSummary {
    let summary = match step.intent.as_str() {
        "read_only_search" => {
            "read-only context lookup completed; raw query, memory content, and PII omitted"
        }
        "read_only_reasoning" => {
            "read-only internal reasoning completed; raw prompt and memory content omitted"
        }
        _ => "read-only internal step completed; raw inputs and content omitted",
    };

    PlanObservationSummary {
        step_id: step.id.clone(),
        source: "internal_read_only".into(),
        summary: summary.into(),
    }
}

fn metadata_safe_governance_summary(
    step: &PlanStep,
    decision: &GovernanceDecision,
) -> PlanGovernanceDecisionSummary {
    PlanGovernanceDecisionSummary {
        step_id: step.id.clone(),
        subject: governance_subject_kind(decision.subject).into(),
        decision_kind: decision.kind,
        risk_level: decision.risk_level,
        policy_reason_code: policy_reason_code(decision).into(),
    }
}

fn metadata_safe_step_summary(step: &PlanStep, decision: &GovernanceDecision) -> String {
    let reason_code = policy_reason_code(decision);

    format!(
        "step_id={} action_kind={} risk_level={} decision={:?} policy_reason_code={} tool_name={}",
        step.id,
        step.action_kind,
        step.risk_level,
        decision.kind,
        reason_code,
        step.tool_name.as_deref().unwrap_or("runtime.reasoning")
    )
}

fn policy_reason_code(decision: &GovernanceDecision) -> &str {
    decision
        .metadata_safe_summary
        .get("policyReasonCode")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown")
}

fn governance_subject_kind(subject: GovernanceSubject) -> &'static str {
    match subject {
        GovernanceSubject::RuntimeInput => "runtime_input",
        GovernanceSubject::ToolAction => "tool_action",
        GovernanceSubject::MaturationCandidate => "maturation_candidate",
        GovernanceSubject::ModelRoute => "model_route",
        GovernanceSubject::MemoryWrite => "memory_write",
        GovernanceSubject::ExternalWrite => "external_write",
    }
}
