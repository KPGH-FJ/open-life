//! Typed orchestration contract for canonical Work.
//!
//! The model may propose a plan, but it cannot mint capabilities, enlarge a
//! budget, or declare completion. This module validates the model-owned JSON
//! into a bounded plan and mechanically evaluates execution evidence.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const WORK_PLAN_SCHEMA_VERSION: &str = "openlife.work-plan.v2";
pub const MAX_WORK_PLAN_STEPS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkPlanStepKind {
    Analyze,
    ReadImportedDocument,
    ReadWorkspaceFile,
    WebSearch,
    WebFetch,
    UseSelectedSkill,
    ReadMcp,
    DraftArtifact,
    Verify,
    DeliverResult,
}

impl WorkPlanStepKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Analyze => "analyze",
            Self::ReadImportedDocument => "read_imported_document",
            Self::ReadWorkspaceFile => "read_workspace_file",
            Self::WebSearch => "web_search",
            Self::WebFetch => "web_fetch",
            Self::UseSelectedSkill => "use_selected_skill",
            Self::ReadMcp => "read_mcp",
            Self::DraftArtifact => "draft_artifact",
            Self::Verify => "verify",
            Self::DeliverResult => "deliver_result",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkPlanStep {
    pub id: String,
    pub kind: WorkPlanStepKind,
    pub required: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Exact registered manifest identity for extension capabilities. Fixed
    /// built-in capabilities are represented by their step kind and must not
    /// carry a target. The model selects only from the bounded manifest ids
    /// supplied by the runtime; it never supplies executable arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    /// Runtime-bound digest of the exact registered execution contract. This
    /// is added after model output validation and prevents a later manifest
    /// replacement from inheriting authority solely by reusing an id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_contract_digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkResultKind {
    Answer,
    Artifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkCompletionContract {
    pub result_kind: WorkResultKind,
    pub requires_verification: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StructuredWorkPlan {
    pub schema_version: String,
    pub steps: Vec<WorkPlanStep>,
    pub completion: WorkCompletionContract,
}

impl StructuredWorkPlan {
    pub fn parse_and_validate(
        raw: &str,
        allowed_kinds: &HashSet<WorkPlanStepKind>,
        allowed_mcp_target_ids: &HashSet<String>,
    ) -> Result<Self, String> {
        let trimmed = raw.trim();
        let json = trimmed
            .strip_prefix("```json")
            .and_then(|value| value.strip_suffix("```"))
            .map(str::trim)
            .unwrap_or(trimmed);
        let plan: Self =
            serde_json::from_str(json).map_err(|_| "work_plan_json_invalid".to_string())?;
        plan.validate(allowed_kinds, allowed_mcp_target_ids)?;
        Ok(plan)
    }

    pub fn validate(
        &self,
        allowed_kinds: &HashSet<WorkPlanStepKind>,
        allowed_mcp_target_ids: &HashSet<String>,
    ) -> Result<(), String> {
        if self.schema_version != WORK_PLAN_SCHEMA_VERSION {
            return Err("work_plan_schema_version_invalid".into());
        }
        if self.steps.is_empty() || self.steps.len() > MAX_WORK_PLAN_STEPS {
            return Err("work_plan_step_count_invalid".into());
        }
        let mut seen = HashSet::new();
        let mut deliver_count = 0usize;
        let mut verify_count = 0usize;
        for step in &self.steps {
            validate_step_id(&step.id)?;
            if !seen.insert(step.id.clone()) {
                return Err("work_plan_step_id_duplicate".into());
            }
            if !allowed_kinds.contains(&step.kind) {
                return Err("work_plan_capability_not_allowed".into());
            }
            match step.kind {
                WorkPlanStepKind::ReadMcp => {
                    let target_id = step
                        .target_id
                        .as_deref()
                        .ok_or_else(|| "work_plan_mcp_target_missing".to_string())?;
                    validate_target_id(target_id)?;
                    if !allowed_mcp_target_ids.contains(target_id) {
                        return Err("work_plan_mcp_target_not_allowed".into());
                    }
                    if let Some(digest) = step.target_contract_digest.as_deref() {
                        validate_contract_digest(digest)?;
                    }
                }
                _ if step.target_id.is_some() || step.target_contract_digest.is_some() => {
                    return Err("work_plan_fixed_capability_target_forbidden".into())
                }
                _ => {}
            }
            if step.depends_on.len() > MAX_WORK_PLAN_STEPS {
                return Err("work_plan_dependency_count_invalid".into());
            }
            let mut dependencies = HashSet::new();
            for dependency in &step.depends_on {
                if !dependencies.insert(dependency) {
                    return Err("work_plan_dependency_duplicate".into());
                }
                if !seen.contains(dependency) || dependency == &step.id {
                    return Err("work_plan_dependency_order_invalid".into());
                }
            }
            if step.kind == WorkPlanStepKind::DeliverResult {
                deliver_count += 1;
                if !step.required {
                    return Err("work_plan_delivery_must_be_required".into());
                }
            }
            if step.kind == WorkPlanStepKind::Verify && step.required {
                verify_count += 1;
            }
        }
        if deliver_count != 1
            || self.steps.last().map(|step| step.kind) != Some(WorkPlanStepKind::DeliverResult)
        {
            return Err("work_plan_delivery_terminal_invalid".into());
        }
        if self.completion.requires_verification && verify_count == 0 {
            return Err("work_plan_verification_step_missing".into());
        }
        if self.completion.result_kind == WorkResultKind::Artifact
            && !self
                .steps
                .iter()
                .any(|step| step.required && step.kind == WorkPlanStepKind::DraftArtifact)
        {
            return Err("work_plan_artifact_step_missing".into());
        }
        Ok(())
    }

    /// A model may choose among allowed capabilities, but it may not omit a
    /// capability that the authenticated instruction mechanically requires.
    /// This second boundary is deliberately separate from `validate`: allowed
    /// is an authorization ceiling, while required is the task contract floor.
    pub fn validate_required_kinds(
        &self,
        required_kinds: &HashSet<WorkPlanStepKind>,
    ) -> Result<(), String> {
        let mut required_kinds = required_kinds.iter().copied().collect::<Vec<_>>();
        required_kinds.sort_by_key(|kind| kind.as_str());
        for required_kind in &required_kinds {
            if !self
                .steps
                .iter()
                .any(|step| step.required && step.kind == *required_kind)
            {
                return Err(format!(
                    "work_plan_required_step_missing_{}",
                    required_kind.as_str()
                ));
            }
        }

        let evidence_or_effect_required = required_kinds.iter().any(|kind| {
            matches!(
                kind,
                WorkPlanStepKind::ReadImportedDocument
                    | WorkPlanStepKind::ReadWorkspaceFile
                    | WorkPlanStepKind::WebSearch
                    | WorkPlanStepKind::WebFetch
                    | WorkPlanStepKind::ReadMcp
                    | WorkPlanStepKind::DraftArtifact
            )
        });
        if evidence_or_effect_required && !self.completion.requires_verification {
            return Err("work_plan_required_verification_missing".into());
        }

        let artifact_required = required_kinds.contains(&WorkPlanStepKind::DraftArtifact);
        if artifact_required != (self.completion.result_kind == WorkResultKind::Artifact) {
            return Err("work_plan_required_result_kind_mismatch".into());
        }
        Ok(())
    }

    pub fn canonical_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|_| "work_plan_serialization_failed".into())
    }
}

fn validate_step_id(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 32 {
        return Err("work_plan_step_id_invalid".into());
    }
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err("work_plan_step_id_invalid".into());
    };
    if !first.is_ascii_lowercase()
        || !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err("work_plan_step_id_invalid".into());
    }
    Ok(())
}

fn validate_target_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'/')
        })
    {
        return Err("work_plan_target_id_invalid".into());
    }
    Ok(())
}

fn validate_contract_digest(value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err("work_plan_target_contract_digest_invalid".into());
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("work_plan_target_contract_digest_invalid".into());
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkRunBudgetPolicy {
    pub max_plan_attempts: u32,
    pub max_provider_attempts: u32,
    pub max_tool_attempts: u32,
    pub max_total_items: u32,
}

impl Default for WorkRunBudgetPolicy {
    fn default() -> Self {
        Self {
            max_plan_attempts: 2,
            max_provider_attempts: 6,
            max_tool_attempts: 8,
            max_total_items: 32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkRunBudgetUsage {
    pub plan_attempts: u32,
    pub provider_attempts: u32,
    pub tool_attempts: u32,
    pub total_items: u32,
}

impl WorkRunBudgetPolicy {
    pub fn admit_plan(self, usage: WorkRunBudgetUsage) -> Result<(), String> {
        if usage.plan_attempts >= self.max_plan_attempts {
            return Err("work_plan_attempt_budget_exhausted".into());
        }
        self.admit_provider(usage)
    }

    pub fn admit_provider(self, usage: WorkRunBudgetUsage) -> Result<(), String> {
        if usage.provider_attempts >= self.max_provider_attempts {
            return Err("work_provider_budget_exhausted".into());
        }
        self.admit_item(usage)
    }

    pub fn admit_tool(self, usage: WorkRunBudgetUsage) -> Result<(), String> {
        if usage.tool_attempts >= self.max_tool_attempts {
            return Err("work_tool_budget_exhausted".into());
        }
        self.admit_item(usage)
    }

    pub fn admit_item(self, usage: WorkRunBudgetUsage) -> Result<(), String> {
        if usage.total_items >= self.max_total_items {
            return Err("work_item_budget_exhausted".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkCompletionEvidence {
    pub required_steps_complete: bool,
    pub pending_or_unknown_items: bool,
    pub final_result_present: bool,
    pub artifact_required: bool,
    pub artifact_ready_or_waiting_review: bool,
    pub verification_required: bool,
    pub verification_complete: bool,
}

pub struct WorkCompletionEvaluator;

impl WorkCompletionEvaluator {
    pub fn evaluate(evidence: WorkCompletionEvidence) -> Result<(), String> {
        if !evidence.required_steps_complete || evidence.pending_or_unknown_items {
            return Err("work_completion_required_item_incomplete".into());
        }
        if evidence.artifact_required && !evidence.artifact_ready_or_waiting_review {
            return Err("work_completion_artifact_missing".into());
        }
        if evidence.verification_required && !evidence.verification_complete {
            return Err("work_completion_verification_missing".into());
        }
        if !evidence.final_result_present {
            return Err("work_completion_final_result_missing".into());
        }
        Ok(())
    }
}

/// Orders already validated plan steps for one canonical Run. Dependencies
/// are forward-only by contract, so a single stable pass is the scheduler; no
/// second queue or task owner is introduced.
pub struct WorkItemScheduler;

impl WorkItemScheduler {
    pub fn schedule(plan: &StructuredWorkPlan) -> Vec<&WorkPlanStep> {
        plan.steps.iter().collect()
    }
}

/// Mechanical executor-side completion projection. Capability adapters decide
/// whether one exact step produced valid evidence; this component decides
/// whether every required scheduled step has such evidence.
pub struct WorkItemExecutor;

impl WorkItemExecutor {
    pub fn required_steps_complete(
        plan: &StructuredWorkPlan,
        completed_step_ids: &HashSet<String>,
    ) -> bool {
        plan.steps
            .iter()
            .filter(|step| step.required)
            .all(|step| completed_step_ids.contains(&step.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed() -> HashSet<WorkPlanStepKind> {
        [
            WorkPlanStepKind::Analyze,
            WorkPlanStepKind::WebSearch,
            WorkPlanStepKind::DraftArtifact,
            WorkPlanStepKind::Verify,
            WorkPlanStepKind::DeliverResult,
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn validates_a_bounded_dependency_ordered_plan() {
        let plan = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v2","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"verify","kind":"verify","required":true,"dependsOn":["research"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"answer","requiresVerification":true}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(plan.steps.len(), 3);
    }

    #[test]
    fn rejects_ungranted_capability_and_forward_dependency() {
        let ungranted = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v2","steps":[{"id":"read","kind":"read_imported_document","required":true,"dependsOn":[]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["read"]}],"completion":{"resultKind":"answer","requiresVerification":false}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert_eq!(ungranted, "work_plan_capability_not_allowed");

        let forward = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v2","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":["deliver"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":[]}],"completion":{"resultKind":"answer","requiresVerification":false}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert_eq!(forward, "work_plan_dependency_order_invalid");
    }

    #[test]
    fn required_capability_floor_cannot_be_omitted_by_a_valid_allowed_plan() {
        let answer_only = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v2","steps":[{"id":"analyze","kind":"analyze","required":true,"dependsOn":[]},{"id":"verify","kind":"verify","required":true,"dependsOn":["analyze"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"answer","requiresVerification":true}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(
            answer_only
                .validate_required_kinds(&HashSet::from([
                    WorkPlanStepKind::WebSearch,
                    WorkPlanStepKind::Verify,
                    WorkPlanStepKind::DeliverResult,
                ]))
                .unwrap_err(),
            "work_plan_required_step_missing_web_search"
        );

        let governed_web = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v2","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"verify","kind":"verify","required":true,"dependsOn":["research"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"answer","requiresVerification":true}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();
        governed_web
            .validate_required_kinds(&HashSet::from([
                WorkPlanStepKind::WebSearch,
                WorkPlanStepKind::Verify,
                WorkPlanStepKind::DeliverResult,
            ]))
            .unwrap();
    }

    #[test]
    fn completion_and_budget_fail_closed() {
        assert_eq!(
            WorkCompletionEvaluator::evaluate(WorkCompletionEvidence {
                required_steps_complete: false,
                final_result_present: true,
                ..WorkCompletionEvidence::default()
            })
            .unwrap_err(),
            "work_completion_required_item_incomplete"
        );
        let policy = WorkRunBudgetPolicy::default();
        assert_eq!(
            policy
                .admit_tool(WorkRunBudgetUsage {
                    tool_attempts: policy.max_tool_attempts,
                    ..WorkRunBudgetUsage::default()
                })
                .unwrap_err(),
            "work_tool_budget_exhausted"
        );

        let plan = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v2","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["research"]}],"completion":{"resultKind":"answer","requiresVerification":false}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(WorkItemScheduler::schedule(&plan).len(), 2);
        assert!(!WorkItemExecutor::required_steps_complete(
            &plan,
            &HashSet::from(["research".to_string()])
        ));
        assert!(WorkItemExecutor::required_steps_complete(
            &plan,
            &HashSet::from(["research".to_string(), "deliver".to_string()])
        ));
    }

    #[test]
    fn mcp_target_must_be_an_exact_allowed_manifest_identity() {
        let allowed_kinds =
            HashSet::from([WorkPlanStepKind::ReadMcp, WorkPlanStepKind::DeliverResult]);
        let allowed_targets = HashSet::from(["weather.current".to_string()]);
        let plan = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v2","steps":[{"id":"read","kind":"read_mcp","required":true,"dependsOn":[],"targetId":"weather.current"},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["read"]}],"completion":{"resultKind":"answer","requiresVerification":false}}"#,
            &allowed_kinds,
            &allowed_targets,
        )
        .unwrap();
        assert_eq!(plan.steps[0].target_id.as_deref(), Some("weather.current"));

        let error = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v2","steps":[{"id":"read","kind":"read_mcp","required":true,"dependsOn":[],"targetId":"unregistered.tool"},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["read"]}],"completion":{"resultKind":"answer","requiresVerification":false}}"#,
            &allowed_kinds,
            &allowed_targets,
        )
        .unwrap_err();
        assert_eq!(error, "work_plan_mcp_target_not_allowed");
    }
}
