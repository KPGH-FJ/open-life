use crate::agent::governor::{
    GovernanceDecision, GovernanceDecisionKind, GovernanceSubject, LifeModelGovernor,
    ToolGovernanceInput,
};
use crate::agent::review_workflow::{
    DurableWriteRequest, DurableWriteSource, DurableWriteSubject, ReviewWorkflow,
};
use crate::agent::runtime_contract::{RuntimeInput, RuntimeOutput};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use crate::agent::ProposalStore;
use crate::life_model::v2::LifeModelSectionV2;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub const WEEKLY_PLANNING_MAX_STEP_COUNT: usize = 5;
const PRODUCT_STEP_TITLE_MAX_LEN: usize = 96;
const PRODUCT_PROPOSAL_PAYLOAD_MAX_BYTES: usize = 2048;

#[derive(Debug, Clone)]
pub struct PlanExecuteInput {
    pub runtime_input: RuntimeInput,
    pub objective: String,
    pub max_steps: usize,
    pub life_model_hints: Vec<PlanExecuteLifeModelHint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteLifeModelHint {
    pub item_id: String,
    pub section: LifeModelSectionV2,
    pub value: String,
    pub selected_reason: String,
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
            life_model_hints: Vec::new(),
        }
    }

    pub fn with_life_model_hints(mut self, hints: Vec<PlanExecuteLifeModelHint>) -> Self {
        self.life_model_hints = hints;
        self
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
                "typedPolicyPresent": true,
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
    Cancelled,
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

        let report = PlanExecuteReport::new(
            plan_id,
            source_run_id,
            &traces,
            governance_decisions,
            observation_summaries,
            warnings.clone(),
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
    #[serde(default)]
    pub plan_id: String,
    pub step_id: String,
    #[serde(default)]
    pub index: usize,
    pub order: usize,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_step_kind")]
    pub kind: String,
    pub intent: String,
    pub tool_name: Option<String>,
    pub action_kind: String,
    pub risk_level: RiskLevel,
    pub declared_write: bool,
    pub status: PlanStepStatus,
    #[serde(default = "default_plan_revision")]
    pub revision: u64,
    #[serde(default = "default_plan_revision")]
    pub base_plan_revision: u64,
    pub linked_proposal_id: Option<String>,
    #[serde(default)]
    pub linked_action_ids: Vec<String>,
    #[serde(default)]
    pub linked_observation_ids: Vec<String>,
    #[serde(default)]
    pub linked_proposal_ids: Vec<String>,
    #[serde(default)]
    pub blocker_ids: Vec<String>,
    #[serde(default)]
    pub linked_final_delivery_ids: Vec<String>,
    #[serde(default)]
    pub skip_reason: Option<String>,
    pub observation_summary: Option<String>,
    pub policy_reason_code: Option<String>,
    #[serde(default)]
    pub policy_decision_id: Option<String>,
    #[serde(default)]
    pub status_reason: Option<String>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
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
    pub plan_id: String,
    pub step_id: String,
    pub step_status: PlanStepStatus,
    pub revision: u64,
    pub base_plan_revision: u64,
    pub step_kind: String,
    pub linked_proposal_id: Option<String>,
    pub linked_action_ids: Vec<String>,
    pub linked_observation_ids: Vec<String>,
    pub linked_proposal_ids: Vec<String>,
    pub blocker_ids: Vec<String>,
    pub linked_final_delivery_ids: Vec<String>,
    pub skip_reason: Option<String>,
    pub observation_summary: Option<String>,
    pub policy_decision_id: Option<String>,
    pub status_reason: Option<String>,
    pub evidence_ids: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteCancelResult {
    pub session_id: String,
    pub plan_id: String,
    pub revision: u64,
    pub base_plan_revision: u64,
    pub cancelled_step_ids: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteReviewItem {
    pub step_id: String,
    pub title: String,
    pub status: String,
    pub evidence_ids: Vec<String>,
    pub linked_action_ids: Vec<String>,
    pub linked_observation_ids: Vec<String>,
    pub linked_proposal_ids: Vec<String>,
    pub blocker_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteReviewSummary {
    pub review_id: String,
    pub plan_id: String,
    pub plan_session_id: String,
    pub plan_status: String,
    pub base_plan_revision: u64,
    pub reviewed_at: String,
    pub completed_steps: Vec<PlanExecuteReviewItem>,
    pub skipped_steps: Vec<PlanExecuteReviewItem>,
    pub blocked_steps: Vec<PlanExecuteReviewItem>,
    pub proposals_created: Vec<PlanExecuteReviewItem>,
    pub observations_used: Vec<PlanExecuteReviewItem>,
    pub unresolved: Vec<PlanExecuteReviewItem>,
    pub recommended_next_action: Vec<String>,
    pub completion_claimed: bool,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteSession {
    pub session_id: String,
    #[serde(default)]
    pub plan_id: String,
    pub source_agent_run_id: Option<String>,
    pub source_chat_session_id: Option<String>,
    pub scenario: PlanExecuteProductScenario,
    pub status: PlanExecuteSessionStatus,
    #[serde(default = "default_plan_revision")]
    pub revision: u64,
    #[serde(default = "default_revision_id")]
    pub revision_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub finalized_at: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<String>,
    #[serde(default)]
    pub review_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_summary: Option<PlanExecuteReviewSummary>,
    #[serde(default)]
    pub source_evidence_ids: Vec<String>,
    #[serde(default)]
    pub superseded_by_plan_id: Option<String>,
    pub metadata_safe_objective: String,
    pub step_count: usize,
    pub completed_step_count: usize,
    pub proposal_required_step_count: usize,
    pub linked_proposal_ids: Vec<String>,
    pub warnings: Vec<String>,
    pub steps: Vec<PlanExecuteStepRecord>,
    pub metadata_safe_summary: Value,
}

fn default_plan_revision() -> u64 {
    1
}

fn default_revision_id() -> String {
    revision_id_for(1)
}

fn default_step_kind() -> String {
    "read".into()
}

fn revision_id_for(revision: u64) -> String {
    format!("rev-{revision}")
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
        let plan_id = format!("plan:{session_id}");
        let revision = 1;
        let steps: Vec<PlanExecuteStepRecord> = draft
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                PlanExecuteStepRecord::from_plan_step(step, index + 1, &plan_id, revision)
            })
            .collect();
        let mut session = Self {
            session_id,
            plan_id,
            source_agent_run_id,
            source_chat_session_id,
            scenario: contract.scenario,
            status: PlanExecuteSessionStatus::Draft,
            revision,
            revision_id: revision_id_for(revision),
            created_at: now.clone(),
            updated_at: now,
            finalized_at: None,
            confirmed_at: None,
            review_id: None,
            review_summary: None,
            source_evidence_ids: Vec::new(),
            superseded_by_plan_id: None,
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
        self.ensure_phase_c_defaults();
        self.apply_draft_edits_at_revision(self.revision, edits)
    }

    pub fn apply_draft_edits_at_revision(
        &mut self,
        base_revision: u64,
        edits: Vec<PlanExecuteStepEdit>,
    ) -> Result<()> {
        self.ensure_phase_c_defaults();
        self.require_current_revision(base_revision)?;
        if self.status != PlanExecuteSessionStatus::Draft {
            return Err(anyhow::anyhow!("Plan-Execute session is not editable"));
        }
        let contract = PlanExecuteProductContract::weekly_planning();
        let next_revision = self.revision + 1;
        let session_id = self.session_id.clone();
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
            step.kind = step_kind_for_record(step);
            step.description = step_description_for_record(step);
            step.revision = next_revision;
            step.base_plan_revision = next_revision;
            step.status_reason = Some("edited_by_user".into());
            push_unique(
                &mut step.evidence_ids,
                format!(
                    "plan-step-edit:{}:{}:{}",
                    session_id, step.step_id, next_revision
                ),
            );
            step.metadata_safe_summary = step_record_summary(step);
        }
        if !self.steps.is_empty() {
            self.revision = next_revision;
            self.revision_id = revision_id_for(next_revision);
        }
        self.touch();
        self.refresh_counts_and_summary();
        Ok(())
    }

    pub fn finalize(&mut self) -> Result<()> {
        self.ensure_phase_c_defaults();
        self.finalize_at_revision(self.revision)
    }

    pub fn finalize_at_revision(&mut self, base_revision: u64) -> Result<()> {
        self.ensure_phase_c_defaults();
        self.require_current_revision(base_revision)?;
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
        self.confirmed_at = Some(now.clone());
        self.updated_at = now;
        self.refresh_counts_and_summary();
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<PlanExecuteCancelResult> {
        self.ensure_phase_c_defaults();
        self.cancel_at_revision(self.revision)
    }

    pub fn cancel_at_revision(&mut self, base_revision: u64) -> Result<PlanExecuteCancelResult> {
        self.ensure_phase_c_defaults();
        self.require_current_revision(base_revision)?;
        if matches!(
            self.status,
            PlanExecuteSessionStatus::Completed | PlanExecuteSessionStatus::Cancelled
        ) {
            return Err(anyhow::anyhow!(
                "Plan-Execute session cannot be cancelled from current status"
            ));
        }
        let session_id = self.session_id.clone();
        let plan_id = self.plan_id.clone();
        let next_revision = self.revision + 1;
        let mut cancelled_step_ids = Vec::new();
        let mut evidence_ids = Vec::new();
        for step in &mut self.steps {
            if is_terminal_product_step(step) {
                continue;
            }
            step.status = PlanStepStatus::Cancelled;
            step.status_reason = Some("cancelled_by_user".into());
            step.revision = next_revision;
            step.base_plan_revision = base_revision;
            step.kind = step_kind_for_record(step);
            step.description = step_description_for_record(step);
            let evidence_id = format!(
                "plan-step-cancel:{}:{}:{}",
                session_id, step.step_id, next_revision
            );
            push_unique(&mut step.evidence_ids, evidence_id.clone());
            cancelled_step_ids.push(step.step_id.clone());
            evidence_ids.push(evidence_id);
            step.metadata_safe_summary = step_record_summary(step);
        }
        self.status = PlanExecuteSessionStatus::Cancelled;
        self.revision = next_revision;
        self.revision_id = revision_id_for(next_revision);
        self.review_id = None;
        self.review_summary = None;
        self.touch();
        self.refresh_counts_and_summary();
        let metadata_safe_summary = json!({
            "planExecuteProductVertical": true,
            "scenarioId": self.scenario.as_id(),
            "planSessionId": self.session_id,
            "planId": self.plan_id,
            "revision": self.revision,
            "basePlanRevision": base_revision,
            "cancelledStepIds": cancelled_step_ids.clone(),
            "evidenceIds": evidence_ids.clone(),
            "metadataSafe": true,
            "directLifeModelWrites": false,
            "memoryWrites": false,
            "externalWritesExecuted": false,
        });
        Ok(PlanExecuteCancelResult {
            session_id,
            plan_id,
            revision: self.revision,
            base_plan_revision: base_revision,
            cancelled_step_ids,
            evidence_ids,
            metadata_safe_summary,
        })
    }

    pub fn execute_step(
        &mut self,
        step_id: &str,
        governor: &LifeModelGovernor,
        proposal_store: &ProposalStore,
    ) -> Result<PlanExecuteStepExecutionResult> {
        self.ensure_phase_c_defaults();
        self.execute_step_at_revision(step_id, self.revision, governor, proposal_store)
    }

    pub fn execute_step_at_revision(
        &mut self,
        step_id: &str,
        base_revision: u64,
        governor: &LifeModelGovernor,
        proposal_store: &ProposalStore,
    ) -> Result<PlanExecuteStepExecutionResult> {
        self.ensure_phase_c_defaults();
        self.require_current_revision(base_revision)?;
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
        let next_revision = self.revision + 1;
        let mut linked_proposal_id_to_add = None;
        {
            let step = self
                .steps
                .iter_mut()
                .find(|step| step.step_id == step_id)
                .ok_or_else(|| anyhow::anyhow!("Plan-Execute step not found"))?;

            if matches!(
                step.status,
                PlanStepStatus::Skipped | PlanStepStatus::Cancelled
            ) {
                return Err(anyhow::anyhow!(
                    "Plan-Execute skipped or cancelled step cannot be executed"
                ));
            }

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
            let policy_reason = policy_reason_code(&decision).to_string();
            step.policy_reason_code = Some(policy_reason.clone());
            step.policy_decision_id = Some(format!(
                "plan-policy:{}:{}:{}",
                session_id, step.step_id, policy_reason
            ));

            if plan_step.declared_write || status == PlanStepStatus::RequiresProposal {
                let proposal_id = create_step_proposal(
                    &session_id,
                    source_run_id.as_deref(),
                    step,
                    proposal_store,
                )?;
                step.status = PlanStepStatus::RequiresProposal;
                step.linked_proposal_id = Some(proposal_id.clone());
                push_unique(&mut step.linked_proposal_ids, proposal_id.clone());
                push_unique(&mut step.evidence_ids, proposal_id.clone());
                step.status_reason = Some("proposal_first_write_boundary".into());
                linked_proposal_id_to_add = Some(proposal_id);
            } else if status == PlanStepStatus::Executed {
                let observation = execute_internal_read_only_step(&plan_step);
                let action_id = plan_step_action_id(&session_id, &step.step_id, next_revision);
                let observation_id =
                    plan_step_observation_id(&session_id, &step.step_id, next_revision);
                step.status = PlanStepStatus::Executed;
                step.observation_summary = Some(observation.summary);
                push_unique(&mut step.linked_action_ids, action_id.clone());
                push_unique(&mut step.linked_observation_ids, observation_id.clone());
                push_unique(&mut step.evidence_ids, action_id);
                push_unique(&mut step.evidence_ids, observation_id);
                step.status_reason = Some("read_only_execution_completed".into());
            } else {
                step.status = status;
                step.observation_summary = Some(metadata_safe_step_summary(&plan_step, &decision));
                let blocker_id = plan_step_blocker_id(&session_id, &step.step_id, next_revision);
                push_unique(&mut step.blocker_ids, blocker_id.clone());
                push_unique(&mut step.evidence_ids, blocker_id);
                step.status_reason = Some(policy_reason);
            }

            step.revision = next_revision;
            step.base_plan_revision = base_revision;
            step.kind = step_kind_for_record(step);
            step.description = step_description_for_record(step);
            step.metadata_safe_summary = step_record_summary(step);
        }

        if let Some(proposal_id) = linked_proposal_id_to_add {
            push_unique(&mut self.linked_proposal_ids, proposal_id);
        }
        self.status = PlanExecuteSessionStatus::InProgress;
        self.revision = next_revision;
        self.revision_id = revision_id_for(next_revision);
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

    pub fn skip_step_at_revision(
        &mut self,
        step_id: &str,
        base_revision: u64,
        reason: &str,
    ) -> Result<PlanExecuteStepExecutionResult> {
        self.ensure_phase_c_defaults();
        self.require_current_revision(base_revision)?;
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(anyhow::anyhow!("Plan-Execute skip reason is required"));
        }
        if matches!(
            self.status,
            PlanExecuteSessionStatus::Completed | PlanExecuteSessionStatus::Cancelled
        ) {
            return Err(anyhow::anyhow!(
                "Plan-Execute session cannot skip steps from current status"
            ));
        }
        let session_id = self.session_id.clone();
        let next_revision = self.revision + 1;
        {
            let step = self
                .steps
                .iter_mut()
                .find(|step| step.step_id == step_id)
                .ok_or_else(|| anyhow::anyhow!("Plan-Execute step not found"))?;
            if matches!(
                step.status,
                PlanStepStatus::Executed
                    | PlanStepStatus::RequiresProposal
                    | PlanStepStatus::Blocked
                    | PlanStepStatus::Cancelled
            ) || step.linked_proposal_id.is_some()
            {
                return Err(anyhow::anyhow!(
                    "Plan-Execute terminal step cannot be skipped"
                ));
            }
            step.status = PlanStepStatus::Skipped;
            step.skip_reason = Some(reason.into());
            step.status_reason = Some("skipped_by_user".into());
            step.revision = next_revision;
            step.base_plan_revision = base_revision;
            step.kind = step_kind_for_record(step);
            step.description = step_description_for_record(step);
            push_unique(
                &mut step.evidence_ids,
                format!(
                    "plan-step-skip:{}:{}:{}",
                    session_id, step.step_id, next_revision
                ),
            );
            step.metadata_safe_summary = step_record_summary(step);
        }
        self.status = PlanExecuteSessionStatus::InProgress;
        self.revision = next_revision;
        self.revision_id = revision_id_for(next_revision);
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
            .expect("step exists after skip");
        Ok(step_execution_result(&self.session_id, step))
    }

    pub fn review_at_revision(&mut self, base_revision: u64) -> Result<PlanExecuteReviewSummary> {
        self.ensure_phase_c_defaults();
        self.require_current_revision(base_revision)?;
        if matches!(
            self.status,
            PlanExecuteSessionStatus::Draft | PlanExecuteSessionStatus::Finalized
        ) {
            return Err(anyhow::anyhow!(
                "Plan-Execute review requires runtime step evidence"
            ));
        }
        let summary = build_review_summary(self, base_revision)?;
        self.review_id = Some(summary.review_id.clone());
        self.review_summary = Some(summary.clone());
        self.touch();
        self.refresh_counts_and_summary();
        Ok(summary)
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

    fn require_current_revision(&self, base_revision: u64) -> Result<()> {
        if base_revision != self.revision {
            return Err(anyhow::anyhow!(
                "Plan-Execute stale revision: expected {}, got {}",
                self.revision,
                base_revision
            ));
        }
        Ok(())
    }

    fn ensure_phase_c_defaults(&mut self) {
        if self.plan_id.trim().is_empty() {
            self.plan_id = format!("plan:{}", self.session_id);
        }
        if self.revision == 0 {
            self.revision = 1;
        }
        if self.revision_id.trim().is_empty() {
            self.revision_id = revision_id_for(self.revision);
        }
        let session_id = self.session_id.clone();
        let plan_id = self.plan_id.clone();
        for (index, step) in self.steps.iter_mut().enumerate() {
            if step.plan_id.trim().is_empty() {
                step.plan_id = plan_id.clone();
            }
            if step.index == 0 {
                step.index = index + 1;
            }
            if step.description.trim().is_empty() {
                step.description = step_description_for_record(step);
            }
            if step.kind.trim().is_empty() {
                step.kind = step_kind_for_record(step);
            }
            if step.revision == 0 {
                step.revision = self.revision;
            }
            if step.base_plan_revision == 0 {
                step.base_plan_revision = self.revision;
            }
            if let Some(proposal_id) = step.linked_proposal_id.clone() {
                push_unique(&mut step.linked_proposal_ids, proposal_id);
            }
            if let Some(policy_reason) = step.policy_reason_code.clone() {
                if step.policy_decision_id.is_none() {
                    step.policy_decision_id = Some(format!(
                        "plan-policy:{}:{}:{}",
                        session_id, step.step_id, policy_reason
                    ));
                }
            }
            step.metadata_safe_summary = step_record_summary(step);
        }
    }

    fn refresh_counts_and_summary(&mut self) {
        self.ensure_phase_c_defaults();
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
            "planId": self.plan_id,
            "revision": self.revision,
            "revisionId": self.revision_id,
            "sourceAgentRunId": self.source_agent_run_id,
            "sourceChatSessionId": self.source_chat_session_id,
            "status": self.status.to_string(),
            "confirmedAt": self.confirmed_at,
            "reviewId": self.review_id,
            "sourceEvidenceIds": self.source_evidence_ids,
            "supersededByPlanId": self.superseded_by_plan_id,
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
    fn from_plan_step(step: &PlanStep, order: usize, plan_id: &str, revision: u64) -> Self {
        let mut record = Self {
            plan_id: plan_id.into(),
            step_id: step.id.clone(),
            index: order,
            order,
            title: step.title.clone(),
            description: step.intent.clone(),
            kind: step_kind_for_plan_step(step).into(),
            intent: step.intent.clone(),
            tool_name: step.tool_name.clone(),
            action_kind: step.action_kind.clone(),
            risk_level: step.risk_level,
            declared_write: step.declared_write,
            status: PlanStepStatus::Planned,
            revision,
            base_plan_revision: revision,
            linked_proposal_id: None,
            linked_action_ids: Vec::new(),
            linked_observation_ids: Vec::new(),
            linked_proposal_ids: Vec::new(),
            blocker_ids: Vec::new(),
            linked_final_delivery_ids: Vec::new(),
            skip_reason: None,
            observation_summary: None,
            policy_reason_code: None,
            policy_decision_id: None,
            status_reason: Some("draft_created".into()),
            evidence_ids: vec![format!("plan-step:{plan_id}:{}", step.id)],
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

fn draft_weekly_planning_plan(input: &PlanExecuteInput) -> PlanDraft {
    let max_steps = input.max_steps.min(WEEKLY_PLANNING_MAX_STEP_COUNT);
    let mut steps = Vec::new();
    if let Some(goal) = input
        .life_model_hints
        .iter()
        .find(|hint| hint.section == LifeModelSectionV2::LongTermGoals)
    {
        let title = format!(
            "Align this week with {}",
            bounded_product_step_text(&goal.value, 64)
        );
        push_named_step(&mut steps, max_steps, title, "lifemodel_goal_alignment");
    }
    if input.life_model_hints.iter().any(|hint| {
        matches!(
            hint.section,
            LifeModelSectionV2::PersonalBoundaries | LifeModelSectionV2::DecisionPrinciples
        )
    }) {
        push_step(
            &mut steps,
            max_steps,
            PlanStepSpec {
                title: "Check confirmed personal boundaries before scheduling",
                intent: "lifemodel_boundary_check",
                tool_name: None,
                action_kind: "reason",
                risk_level: RiskLevel::Low,
                declared_write: false,
            },
        );
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
        "scenario=weekly_planning task_kind={} max_steps={} lifemodel_hint_count={}",
        input.runtime_input.task.kind,
        input.max_steps.min(WEEKLY_PLANNING_MAX_STEP_COUNT),
        input.life_model_hints.len(),
    )
}

fn bounded_product_step_text(value: &str, max_chars: usize) -> String {
    value.trim().chars().take(max_chars).collect()
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
    "planId": step.plan_id,
    "stepId": step.step_id,
    "index": step.index,
    "order": step.order,
    "kind": step.kind,
    "actionKind": step.action_kind,
    "riskLevel": step.risk_level.to_string(),
    "declaredWrite": step.declared_write,
    "status": format!("{:?}", step.status).to_ascii_lowercase(),
    "revision": step.revision,
    "basePlanRevision": step.base_plan_revision,
    "linkedActionIds": step.linked_action_ids,
    "linkedObservationIds": step.linked_observation_ids,
    "linkedProposalId": step.linked_proposal_id,
    "linkedProposalIds": step.linked_proposal_ids,
    "blockerIds": step.blocker_ids,
    "linkedFinalDeliveryIds": step.linked_final_delivery_ids,
    "skipReasonPresent": step.skip_reason.is_some(),
    "policyReasonCode": step.policy_reason_code,
    "policyDecisionId": step.policy_decision_id,
    "statusReason": step.status_reason,
    "evidenceIds": step.evidence_ids,
    "metadataSafe": true,
        "rawPromptStored": false,
        "rawToolPayloadStored": false,
        "directLifeModelWrites": false,
        "memoryWrites": false,
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
    let outcome = ReviewWorkflow::new(proposal_store).submit(
        DurableWriteRequest::from_agent_proposal(
            DurableWriteSource::PlanExecute,
            DurableWriteSubject::PlanStep,
            proposal,
            "Plan step proposal is pending Review Center approval.",
        )
        .with_evidence_refs(vec![format!("plan_execute_session:{session_id}")]),
    )?;
    Ok(outcome.proposal_id().to_string())
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
        PlanStepStatus::Executed
            | PlanStepStatus::RequiresProposal
            | PlanStepStatus::Blocked
            | PlanStepStatus::Skipped
            | PlanStepStatus::Cancelled
    ) || step.linked_proposal_id.is_some()
}

fn build_review_summary(
    session: &PlanExecuteSession,
    base_revision: u64,
) -> Result<PlanExecuteReviewSummary> {
    let mut completed_steps = Vec::new();
    let mut skipped_steps = Vec::new();
    let mut blocked_steps = Vec::new();
    let mut proposals_created = Vec::new();
    let mut observations_used = Vec::new();
    let mut unresolved = Vec::new();

    for step in &session.steps {
        let item = review_item_for_step(step);
        match step.status {
            PlanStepStatus::Executed => {
                if !step.linked_action_ids.is_empty() || !step.linked_observation_ids.is_empty() {
                    completed_steps.push(item.clone());
                    if !step.linked_observation_ids.is_empty() {
                        observations_used.push(item);
                    }
                } else {
                    unresolved.push(item);
                }
            }
            PlanStepStatus::Skipped => {
                if step
                    .evidence_ids
                    .iter()
                    .any(|id| id.contains("plan-step-skip"))
                {
                    skipped_steps.push(item);
                } else {
                    unresolved.push(item);
                }
            }
            PlanStepStatus::RequiresProposal => {
                if !step.linked_proposal_ids.is_empty() {
                    proposals_created.push(item);
                } else {
                    unresolved.push(item);
                }
            }
            PlanStepStatus::Blocked => {
                if !step.blocker_ids.is_empty() {
                    blocked_steps.push(item);
                } else {
                    unresolved.push(item);
                }
            }
            PlanStepStatus::Cancelled => {
                unresolved.push(item);
            }
            PlanStepStatus::Planned | PlanStepStatus::RequiresConfirmation => {
                unresolved.push(item);
            }
        }
    }

    let has_runtime_evidence = completed_steps
        .iter()
        .chain(skipped_steps.iter())
        .chain(blocked_steps.iter())
        .chain(proposals_created.iter())
        .chain(observations_used.iter())
        .chain(unresolved.iter())
        .any(|item| {
            !item.evidence_ids.is_empty()
                || !item.linked_action_ids.is_empty()
                || !item.linked_observation_ids.is_empty()
                || !item.linked_proposal_ids.is_empty()
                || !item.blocker_ids.is_empty()
        });
    if !has_runtime_evidence {
        return Err(anyhow::anyhow!(
            "Plan-Execute review requires linked runtime evidence"
        ));
    }

    let completion_claimed =
        session.status == PlanExecuteSessionStatus::Completed && unresolved.is_empty();
    let recommended_next_action = recommended_next_action_for_review(
        session,
        &unresolved,
        &proposals_created,
        &blocked_steps,
    );
    let reviewed_at = Utc::now().to_rfc3339();
    let review_id = format!("plan-review:{}:rev-{}", session.session_id, base_revision);
    let metadata_safe_summary = json!({
        "planExecuteProductVertical": true,
        "scenarioId": session.scenario.as_id(),
        "reviewId": review_id,
        "planId": session.plan_id,
        "planSessionId": session.session_id,
        "planStatus": session.status.to_string(),
        "basePlanRevision": base_revision,
        "completedStepCount": completed_steps.len(),
        "skippedStepCount": skipped_steps.len(),
        "blockedStepCount": blocked_steps.len(),
        "proposalCreatedCount": proposals_created.len(),
        "observationUsedCount": observations_used.len(),
        "unresolvedCount": unresolved.len(),
        "completionClaimed": completion_claimed,
        "metadataSafe": true,
        "directLifeModelWrites": false,
        "memoryWrites": false,
        "externalWritesExecuted": false,
    });

    Ok(PlanExecuteReviewSummary {
        review_id,
        plan_id: session.plan_id.clone(),
        plan_session_id: session.session_id.clone(),
        plan_status: session.status.to_string(),
        base_plan_revision: base_revision,
        reviewed_at,
        completed_steps,
        skipped_steps,
        blocked_steps,
        proposals_created,
        observations_used,
        unresolved,
        recommended_next_action,
        completion_claimed,
        metadata_safe_summary,
    })
}

fn review_item_for_step(step: &PlanExecuteStepRecord) -> PlanExecuteReviewItem {
    PlanExecuteReviewItem {
        step_id: step.step_id.clone(),
        title: step.title.clone(),
        status: format!("{:?}", step.status).to_ascii_lowercase(),
        evidence_ids: step.evidence_ids.clone(),
        linked_action_ids: step.linked_action_ids.clone(),
        linked_observation_ids: step.linked_observation_ids.clone(),
        linked_proposal_ids: step.linked_proposal_ids.clone(),
        blocker_ids: step.blocker_ids.clone(),
    }
}

fn recommended_next_action_for_review(
    session: &PlanExecuteSession,
    unresolved: &[PlanExecuteReviewItem],
    proposals_created: &[PlanExecuteReviewItem],
    blocked_steps: &[PlanExecuteReviewItem],
) -> Vec<String> {
    if session.status == PlanExecuteSessionStatus::Cancelled {
        return vec!["Review cancelled steps before starting a new plan.".into()];
    }
    if !blocked_steps.is_empty() {
        return vec!["Resolve blockers before continuing this plan.".into()];
    }
    if !proposals_created.is_empty() {
        return vec!["Review created proposals before applying any changes.".into()];
    }
    if !unresolved.is_empty() {
        return vec!["Decide whether to skip, execute, or cancel unresolved steps.".into()];
    }
    vec!["No remaining plan action is required.".into()]
}

fn step_execution_result(
    session_id: &str,
    step: &PlanExecuteStepRecord,
) -> PlanExecuteStepExecutionResult {
    PlanExecuteStepExecutionResult {
        session_id: session_id.into(),
        plan_id: step.plan_id.clone(),
        step_id: step.step_id.clone(),
        step_status: step.status,
        revision: step.revision,
        base_plan_revision: step.base_plan_revision,
        step_kind: step.kind.clone(),
        linked_proposal_id: step.linked_proposal_id.clone(),
        linked_action_ids: step.linked_action_ids.clone(),
        linked_observation_ids: step.linked_observation_ids.clone(),
        linked_proposal_ids: step.linked_proposal_ids.clone(),
        blocker_ids: step.blocker_ids.clone(),
        linked_final_delivery_ids: step.linked_final_delivery_ids.clone(),
        skip_reason: step.skip_reason.clone(),
        observation_summary: step.observation_summary.clone(),
        policy_decision_id: step.policy_decision_id.clone(),
        status_reason: step.status_reason.clone(),
        evidence_ids: step.evidence_ids.clone(),
        metadata_safe_summary: json!({
            "planExecuteProductVertical": true,
            "scenarioId": "weekly_planning",
            "planSessionId": session_id,
            "planId": step.plan_id,
            "stepId": step.step_id,
            "stepStatus": format!("{:?}", step.status).to_ascii_lowercase(),
            "revision": step.revision,
            "basePlanRevision": step.base_plan_revision,
            "stepKind": step.kind,
            "linkedActionIds": step.linked_action_ids,
            "linkedObservationIds": step.linked_observation_ids,
            "linkedProposalId": step.linked_proposal_id,
            "linkedProposalIds": step.linked_proposal_ids,
            "blockerIds": step.blocker_ids,
            "linkedFinalDeliveryIds": step.linked_final_delivery_ids,
            "skipReasonPresent": step.skip_reason.is_some(),
            "observationSummaryPresent": step.observation_summary.is_some(),
            "policyDecisionId": step.policy_decision_id,
            "statusReason": step.status_reason,
            "evidenceIds": step.evidence_ids,
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

fn step_kind_for_plan_step(step: &PlanStep) -> &'static str {
    if step.declared_write {
        "proposal"
    } else if matches!(step.action_kind.as_str(), "reason" | "search" | "plan") {
        "read"
    } else {
        "manual"
    }
}

fn step_kind_for_record(step: &PlanExecuteStepRecord) -> String {
    match step.status {
        PlanStepStatus::Blocked => "blocked".into(),
        PlanStepStatus::RequiresConfirmation => "ask_user".into(),
        _ => {
            if step.declared_write || step.linked_proposal_id.is_some() {
                "proposal".into()
            } else if matches!(step.action_kind.as_str(), "reason" | "search" | "plan") {
                "read".into()
            } else {
                "manual".into()
            }
        }
    }
}

fn step_description_for_record(step: &PlanExecuteStepRecord) -> String {
    if !step.intent.trim().is_empty() {
        step.intent.clone()
    } else {
        format!("{} step", step.kind)
    }
}

fn plan_step_action_id(session_id: &str, step_id: &str, revision: u64) -> String {
    format!("plan-action:{session_id}:{step_id}:rev-{revision}")
}

fn plan_step_observation_id(session_id: &str, step_id: &str, revision: u64) -> String {
    format!("plan-observation:{session_id}:{step_id}:rev-{revision}")
}

fn plan_step_blocker_id(session_id: &str, step_id: &str, revision: u64) -> String {
    format!("plan-blocker:{session_id}:{step_id}:rev-{revision}")
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

fn push_named_step(steps: &mut Vec<PlanStep>, max_steps: usize, title: String, intent: &str) {
    if steps.len() >= max_steps {
        return;
    }
    steps.push(PlanStep {
        id: format!("step-{}", steps.len() + 1),
        title,
        intent: intent.into(),
        tool_name: None,
        action_kind: "plan".into(),
        risk_level: RiskLevel::Low,
        declared_write: false,
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
            metadata_safe_summary,
        }
    }
}

fn source_run_id(input: &RuntimeInput) -> Option<String> {
    input.source_run_id.clone()
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
        GovernanceSubject::ModelRoute => "model_route",
        GovernanceSubject::MemoryWrite => "memory_write",
        GovernanceSubject::ExternalWrite => "external_write",
    }
}
