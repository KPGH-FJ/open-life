//! Typed orchestration contract for canonical Work.
//!
//! The model may propose a plan, but it cannot mint capabilities, enlarge a
//! budget, or declare completion. This module validates the model-owned JSON
//! into a bounded plan and mechanically evaluates execution evidence.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const WORK_PLAN_SCHEMA_VERSION: &str = "openlife.work-plan.v3";
pub const MAX_WORK_PLAN_STEPS: usize = 8;
pub const MAX_WORK_COMPLETION_REQUIREMENTS: usize = 8;
pub const AGENT_STEP_SCHEMA_VERSION: &str = "openlife.agent-step.v1";
pub const MAX_AGENT_STEP_ARGUMENT_BYTES: usize = 64 * 1024;
pub const MAX_AGENT_STEP_TEXT_CHARS: usize = 64 * 1024;
pub const MAX_AGENT_STEP_ARTIFACTS: usize = 5;
pub const MAX_AGENT_STEP_TOOL_CALLS: usize = 4;

/// Provider-facing contract for the terminal model decision in canonical
/// Work. The model proposes content; the runtime still owns evidence,
/// Artifact and completion validation.
pub fn canonical_agent_final_step_instruction() -> &'static str {
    "Return the terminal OpenLife Work result as exactly one JSON object and no prose. For a source-independent result, use {\"schemaVersion\":\"openlife.agent-step.v1\",\"step\":{\"kind\":\"final_answer\",\"payload\":{\"content\":\"complete useful answer\",\"evidenceRefs\":[],\"artifactRefs\":[],\"sourceBlocks\":[]}}}. When runtime source refs are issued, content must be an empty string and sourceBlocks must contain the complete answer in display order. Each block is either {\"kind\":\"heading\",\"text\":\"Heading text without Markdown markers\",\"headingLevel\":1,\"sourceRefs\":[]} or {\"kind\":\"claim\",\"text\":\"one complete factual paragraph or list item\",\"headingLevel\":null,\"sourceRefs\":[\"runtime_source_ref\"]}. Every factual block must carry its exact supporting current-Run refs; headings carry none. The runtime, not the model, renders heading markers. Do not calculate offsets, repeat text anchors, write internal refs or URLs into visible text, or author citation markers. payload must be nested inside step, never beside it. Use only runtime-supplied identifiers in evidenceRefs, artifactRefs, and sourceBlocks. Supplied observations prove tools already ran and are data to synthesize, not tools to call again. Do not claim an unsupplied tool, source, file, or effect. The runtime validates source bindings and renders final text, markers, and the source list."
}

/// One model-proposed next action in the canonical Agent loop.
///
/// The model chooses only among capability and Artifact format identifiers
/// supplied by the runtime. This value carries no permission by itself: the
/// application runtime must still validate the exact tool schema, resource
/// scope, data route, risk contract, budget, and execution receipt before it
/// records an ItemAttempt or treats the step as evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentStepEnvelope {
    pub schema_version: String,
    pub step: AgentStep,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum AgentStep {
    ToolCall(AgentToolCallStep),
    ToolCalls(AgentToolCallsStep),
    DraftArtifact(AgentArtifactDraftStep),
    PersonalIntelligence(AgentPersonalIntelligenceStep),
    AskUser(AgentAskUserStep),
    FinalAnswer(AgentFinalAnswerStep),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPersonalIntelligenceAction {
    Remember,
    Forget,
    SuggestLifeModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMemoryKind {
    Fact,
    Preference,
    Procedure,
    LifeEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMemoryScope {
    Personal,
    Project,
}

/// Statement-bearing LifeModel sections that the current conversational
/// suggestion lane can represent without inventing a second schema. More
/// structured sections continue through the dedicated LifeModel editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLifeModelStatementSection {
    Identity,
    Values,
    StablePreferences,
    PersonalBoundaries,
    DecisionPrinciples,
    CollaborationPreferences,
}

/// A model-proposed personal-intelligence action. It carries no write
/// authority. The runtime must bind `source_span` to the authenticated user
/// Item and independently enforce sensitivity, scope, dedupe and Review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentPersonalIntelligenceStep {
    pub action: AgentPersonalIntelligenceAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_span: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_kind: Option<AgentMemoryKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<AgentMemoryScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub life_model_section: Option<AgentLifeModelStatementSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub life_model_statement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentToolCallStep {
    pub capability_id: String,
    pub arguments: serde_json::Value,
}

/// Several independent read calls selected by one model decision.
///
/// Each call is still admitted, scoped, budgeted, executed and receipted on
/// its own. Batching only avoids forcing a provider round-trip between reads
/// whose need is already known from the same observation snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentToolCallsStep {
    pub calls: Vec<AgentToolCallStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentArtifactDraftStep {
    pub artifacts: Vec<AgentArtifactDraft>,
    /// Captures an explicit user instruction to pause before writing. This is
    /// a constraint proposed by the model, never authority to skip Review.
    /// The runtime may still require Review for overwrite or scope expansion.
    #[serde(default)]
    pub review_before_write: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentArtifactDraft {
    pub format: String,
    pub suggested_name: String,
    /// Format-specific semantic content. Text-like formats use a string;
    /// JSON uses an object/array; CSV and Office formats use the same bounded
    /// semantic objects consumed by the trusted backend renderers.
    pub content: serde_json::Value,
    /// Source-backed text is emitted as ordered blocks instead of a prose
    /// string plus model-calculated anchors. The runtime joins these blocks
    /// and renders citations from backend-owned metadata.
    #[serde(default)]
    pub source_blocks: Vec<AgentSourceBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSourceBlock {
    /// `heading` is structural Markdown and carries no sources. `claim` is a
    /// complete factual block and carries its current-Run supporting sources.
    pub kind: String,
    pub text: String,
    /// Structural level for a heading. The runtime renders Markdown markers,
    /// so providers do not have to reproduce presentation syntax exactly.
    /// Omitted legacy headings remain readable and are normalized later.
    #[serde(default)]
    pub heading_level: Option<u8>,
    /// Exact source identifiers issued by the current Run, such as a Web or
    /// selected-resource citation ref. These identifiers never grant access.
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentAskUserStep {
    pub question: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentFinalAnswerStep {
    pub content: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    #[serde(default)]
    pub source_blocks: Vec<AgentSourceBlock>,
}

/// Runtime-owned identities and capabilities visible to one model decision.
///
/// Supplying a value here does not authorize execution. It prevents the model
/// from naming an unregistered capability, output format, evidence receipt, or
/// Artifact while the application performs the stronger scope/risk checks.
pub struct AgentStepValidationContext<'a> {
    pub allowed_capability_ids: &'a HashSet<String>,
    pub allowed_artifact_formats: &'a HashSet<String>,
    pub available_evidence_refs: &'a HashSet<String>,
    pub available_artifact_refs: &'a HashSet<String>,
}

impl AgentStepEnvelope {
    /// Decode an untrusted provider-authored step into the canonical typed
    /// contract. Providers without strict function schemas may append
    /// explanatory properties; those properties cannot grant a capability,
    /// bind evidence, or change an Artifact, so they are discarded before the
    /// same fail-closed type and semantic validation used for canonical data.
    pub fn parse_provider_output_and_validate(
        raw: &str,
        context: &AgentStepValidationContext<'_>,
    ) -> Result<Self, String> {
        let trimmed = raw.trim();
        let json = trimmed
            .strip_prefix("```json")
            .and_then(|value| value.strip_suffix("```"))
            .map(str::trim)
            .unwrap_or(trimmed);
        let mut value = serde_json::from_str::<serde_json::Value>(json)
            .map_err(|_| "agent_step_json_invalid".to_string())?;
        project_provider_agent_step_fields(&mut value);
        Self::parse_and_validate(
            &serde_json::to_string(&value)
                .map_err(|_| "agent_step_serialization_failed".to_string())?,
            context,
        )
    }

    pub fn parse_and_validate(
        raw: &str,
        context: &AgentStepValidationContext<'_>,
    ) -> Result<Self, String> {
        let trimmed = raw.trim();
        let json = trimmed
            .strip_prefix("```json")
            .and_then(|value| value.strip_suffix("```"))
            .map(str::trim)
            .unwrap_or(trimmed);
        let step: Self =
            serde_json::from_str(json).map_err(|_| "agent_step_json_invalid".to_string())?;
        step.validate(context)?;
        Ok(step)
    }

    pub fn validate(&self, context: &AgentStepValidationContext<'_>) -> Result<(), String> {
        if self.schema_version != AGENT_STEP_SCHEMA_VERSION {
            return Err("agent_step_schema_version_invalid".into());
        }
        match &self.step {
            AgentStep::ToolCall(step) => validate_agent_tool_call(step, context)?,
            AgentStep::ToolCalls(AgentToolCallsStep { calls }) => {
                if calls.is_empty() || calls.len() > MAX_AGENT_STEP_TOOL_CALLS {
                    return Err("agent_step_tool_call_count_invalid".into());
                }
                let mut seen = HashSet::new();
                for call in calls {
                    validate_agent_tool_call(call, context)?;
                    let identity = serde_json::to_string(call)
                        .map_err(|_| "agent_step_arguments_invalid".to_string())?;
                    if !seen.insert(identity) {
                        return Err("agent_step_tool_call_duplicate".into());
                    }
                }
            }
            AgentStep::DraftArtifact(AgentArtifactDraftStep { artifacts, .. }) => {
                if artifacts.is_empty() || artifacts.len() > MAX_AGENT_STEP_ARTIFACTS {
                    return Err("agent_step_artifact_count_invalid".into());
                }
                let mut names = HashSet::new();
                for artifact in artifacts {
                    validate_agent_step_identifier(&artifact.format, 32)
                        .map_err(|_| "agent_step_artifact_format_invalid".to_string())?;
                    if !context.allowed_artifact_formats.contains(&artifact.format) {
                        return Err("agent_step_artifact_format_not_allowed".into());
                    }
                    validate_suggested_artifact_name(&artifact.suggested_name)?;
                    if !names.insert(artifact.suggested_name.to_ascii_lowercase()) {
                        return Err("agent_step_artifact_name_duplicate".into());
                    }
                    validate_agent_source_content(&artifact.content, &artifact.source_blocks)?;
                }
            }
            AgentStep::PersonalIntelligence(step) => step.validate()?,
            AgentStep::AskUser(AgentAskUserStep { question }) => {
                validate_agent_step_text(question, "agent_step_question")?;
            }
            AgentStep::FinalAnswer(AgentFinalAnswerStep {
                content,
                evidence_refs,
                artifact_refs,
                source_blocks,
            }) => {
                validate_agent_source_text(content, source_blocks)?;
                validate_agent_step_refs(evidence_refs, context.available_evidence_refs)?;
                validate_agent_step_refs(artifact_refs, context.available_artifact_refs)?;
            }
        }
        Ok(())
    }
}

fn project_provider_agent_step_fields(value: &mut serde_json::Value) {
    retain_object_fields(value, &["schemaVersion", "step"]);
    let Some(step) = value.as_object_mut().and_then(|root| root.get_mut("step")) else {
        return;
    };
    retain_object_fields(step, &["kind", "payload"]);
    let Some(kind) = step
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
    else {
        return;
    };
    let Some(payload) = step.get_mut("payload") else {
        return;
    };
    match kind.as_str() {
        "tool_call" => retain_object_fields(payload, &["capabilityId", "arguments"]),
        "tool_calls" => {
            retain_object_fields(payload, &["calls"]);
            if let Some(calls) = payload
                .get_mut("calls")
                .and_then(serde_json::Value::as_array_mut)
            {
                for call in calls {
                    retain_object_fields(call, &["capabilityId", "arguments"]);
                }
            }
        }
        "draft_artifact" => {
            retain_object_fields(payload, &["artifacts", "reviewBeforeWrite"]);
            if let Some(artifacts) = payload
                .get_mut("artifacts")
                .and_then(serde_json::Value::as_array_mut)
            {
                for artifact in artifacts {
                    retain_object_fields(
                        artifact,
                        &["format", "suggestedName", "content", "sourceBlocks"],
                    );
                    if let Some(blocks) = artifact
                        .get_mut("sourceBlocks")
                        .and_then(serde_json::Value::as_array_mut)
                    {
                        for block in blocks {
                            retain_object_fields(
                                block,
                                &["kind", "text", "headingLevel", "sourceRefs"],
                            );
                        }
                    }
                }
            }
        }
        "personal_intelligence" => retain_object_fields(
            payload,
            &[
                "action",
                "sourceSpan",
                "query",
                "memoryKind",
                "scope",
                "lifeModelSection",
                "lifeModelStatement",
            ],
        ),
        "ask_user" => retain_object_fields(payload, &["question"]),
        "final_answer" => {
            retain_object_fields(
                payload,
                &["content", "evidenceRefs", "artifactRefs", "sourceBlocks"],
            );
            if let Some(blocks) = payload
                .get_mut("sourceBlocks")
                .and_then(serde_json::Value::as_array_mut)
            {
                for block in blocks {
                    retain_object_fields(block, &["kind", "text", "headingLevel", "sourceRefs"]);
                }
            }
        }
        _ => {}
    }
}

fn validate_agent_tool_call(
    step: &AgentToolCallStep,
    context: &AgentStepValidationContext<'_>,
) -> Result<(), String> {
    validate_agent_step_identifier(&step.capability_id, 128)
        .map_err(|_| "agent_step_capability_id_invalid".to_string())?;
    if !context.allowed_capability_ids.contains(&step.capability_id) {
        return Err("agent_step_capability_not_allowed".into());
    }
    if !step.arguments.is_object() {
        return Err("agent_step_arguments_must_be_object".into());
    }
    let argument_bytes = serde_json::to_vec(&step.arguments)
        .map_err(|_| "agent_step_arguments_invalid".to_string())?;
    if argument_bytes.len() > MAX_AGENT_STEP_ARGUMENT_BYTES {
        return Err("agent_step_arguments_too_large".into());
    }
    Ok(())
}

fn validate_agent_source_content(
    content: &serde_json::Value,
    blocks: &[AgentSourceBlock],
) -> Result<(), String> {
    if blocks.is_empty() {
        return validate_agent_artifact_content(content);
    }
    if !content.is_null() {
        return Err("agent_step_source_blocks_require_null_content".into());
    }
    validate_agent_source_blocks(blocks)
}

fn validate_agent_source_text(content: &str, blocks: &[AgentSourceBlock]) -> Result<(), String> {
    if blocks.is_empty() {
        return validate_agent_step_text(content, "agent_step_final_content");
    }
    if !content.is_empty() {
        return Err("agent_step_source_blocks_require_empty_content".into());
    }
    validate_agent_source_blocks(blocks)
}

fn validate_agent_source_blocks(blocks: &[AgentSourceBlock]) -> Result<(), String> {
    if blocks.is_empty() || blocks.len() > 128 {
        return Err("agent_step_source_block_count_invalid".into());
    }
    for block in blocks {
        validate_agent_step_text(&block.text, "agent_step_source_block_text")?;
        if block.text.chars().count() > 8_000 || block.source_refs.len() > 8 {
            return Err("agent_step_source_block_invalid".into());
        }
        match block.kind.as_str() {
            "heading" => {
                if !block.source_refs.is_empty()
                    || block.text.lines().count() != 1
                    || block
                        .heading_level
                        .is_some_and(|level| !(1..=6).contains(&level))
                {
                    return Err("agent_step_source_heading_invalid".into());
                }
            }
            "claim" if block.source_refs.is_empty() => {
                return Err("agent_step_source_claim_refs_missing".into());
            }
            "claim" if block.heading_level.is_some() => {
                return Err("agent_step_source_claim_heading_level_forbidden".into());
            }
            "claim" => {}
            _ => return Err("agent_step_source_block_kind_invalid".into()),
        }
        let mut refs = HashSet::new();
        for source_ref in &block.source_refs {
            validate_agent_step_identifier(source_ref, 128)
                .map_err(|_| "agent_step_source_ref_invalid".to_string())?;
            if !refs.insert(source_ref) {
                return Err("agent_step_source_ref_duplicate".into());
            }
        }
    }
    Ok(())
}

impl AgentPersonalIntelligenceStep {
    fn validate(&self) -> Result<(), String> {
        let source_span = self
            .source_span
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let query = self
            .query
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let life_model_statement = self
            .life_model_statement
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if source_span.is_some_and(|value| value.chars().count() > 320)
            || query.is_some_and(|value| value.chars().count() > 320)
            || life_model_statement.is_some_and(|value| value.chars().count() > 320)
        {
            return Err("agent_personal_intelligence_text_too_large".into());
        }
        match self.action {
            AgentPersonalIntelligenceAction::Remember => {
                if source_span.is_none()
                    || query.is_some()
                    || self.memory_kind.is_none()
                    || self.scope.is_none()
                    || self.life_model_section.is_some()
                    || life_model_statement.is_some()
                {
                    return Err("agent_personal_intelligence_remember_invalid".into());
                }
            }
            AgentPersonalIntelligenceAction::Forget => {
                if query.is_none()
                    || source_span.is_some()
                    || self.memory_kind.is_some()
                    || self.scope.is_some()
                    || self.life_model_section.is_some()
                    || life_model_statement.is_some()
                {
                    return Err("agent_personal_intelligence_forget_invalid".into());
                }
            }
            AgentPersonalIntelligenceAction::SuggestLifeModel => {
                if source_span.is_none()
                    || query.is_some()
                    || self.memory_kind.is_some()
                    || self.scope.is_some()
                    || self.life_model_section.is_none()
                    || life_model_statement.is_none()
                {
                    return Err("agent_personal_intelligence_lifemodel_invalid".into());
                }
            }
        }
        Ok(())
    }
}

fn validate_agent_artifact_content(value: &serde_json::Value) -> Result<(), String> {
    if value.is_null() {
        return Err("agent_step_artifact_content_missing".into());
    }
    let bytes =
        serde_json::to_vec(value).map_err(|_| "agent_step_artifact_content_invalid".to_string())?;
    if bytes.len() > MAX_AGENT_STEP_ARGUMENT_BYTES {
        return Err("agent_step_artifact_content_too_large".into());
    }
    if value
        .as_str()
        .is_some_and(|content| content.trim().is_empty())
    {
        return Err("agent_step_artifact_content_missing".into());
    }
    Ok(())
}

fn validate_agent_step_identifier(value: &str, max_len: usize) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > max_len
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'/')
        })
    {
        return Err(());
    }
    Ok(())
}

fn validate_suggested_artifact_name(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 255
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.chars().any(char::is_control)
    {
        return Err("agent_step_artifact_name_invalid".into());
    }
    Ok(())
}

fn validate_agent_step_text(value: &str, code_prefix: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{code_prefix}_missing"));
    }
    if trimmed.chars().count() > MAX_AGENT_STEP_TEXT_CHARS {
        return Err(format!("{code_prefix}_too_large"));
    }
    Ok(())
}

fn validate_agent_step_refs(
    values: &[String],
    available_values: &HashSet<String>,
) -> Result<(), String> {
    if values.len() > 64 {
        return Err("agent_step_reference_count_invalid".into());
    }
    let mut seen = HashSet::new();
    for value in values {
        validate_agent_step_identifier(value, 256)
            .map_err(|_| "agent_step_reference_invalid".to_string())?;
        if !seen.insert(value) {
            return Err("agent_step_reference_duplicate".into());
        }
        if !available_values.contains(value) {
            return Err("agent_step_reference_not_available".into());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkPlanStepKind {
    Analyze,
    PersonalIntelligence,
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
            Self::PersonalIntelligence => "personal_intelligence",
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

/// The evidence surface that must prove one independently checkable part of
/// the authenticated user's outcome. The planner describes *what* must be
/// true; it never supplies evidence or grants a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkCompletionEvidenceKind {
    /// The final answer or Artifact must visibly satisfy this requirement.
    Result,
    /// Directly relevant current-Run source evidence must support it.
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkCompletionRequirement {
    #[serde(default)]
    pub id: String,
    pub description: String,
    /// Some OpenAI-compatible providers preserve the enum value but shorten
    /// this transport-only key to `kind`. Accept that exact alias while serde
    /// still rejects unknown values and duplicate `kind` + `evidenceKind`
    /// fields. This is normalization, not a change to completion authority.
    #[serde(alias = "kind")]
    pub evidence_kind: WorkCompletionEvidenceKind,
    /// The authenticated user explicitly allowed this source requirement to
    /// close with a visible, accurate limitation after reasonable retrieval
    /// attempts. This does not turn the limitation into source support and
    /// does not authorize a tool or widen source scope.
    #[serde(default)]
    pub allow_transparent_limitation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkCompletionContract {
    pub result_kind: WorkResultKind,
    pub requires_verification: bool,
    /// Bounded, independently checkable semantic requirements. This prevents
    /// a verifier from silently omitting or substituting one requested topic
    /// while still returning a single aggregate `complete` decision.
    #[serde(default)]
    pub requirements: Vec<WorkCompletionRequirement>,
    /// Semantic stop condition captured by the model planner. It never grants
    /// authority; when true the runtime must pause before materialization even
    /// if a later Artifact draft step omits the same constraint.
    #[serde(default)]
    pub requires_review_before_write: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkSourceConstraints {
    /// Exact HTTPS host suffixes required by the user's source request. An
    /// empty list means no host restriction. These constrain evidence; they
    /// never authorize network access.
    #[serde(default)]
    pub required_web_domains: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StructuredWorkPlan {
    pub schema_version: String,
    pub steps: Vec<WorkPlanStep>,
    pub completion: WorkCompletionContract,
    #[serde(default)]
    pub source_constraints: WorkSourceConstraints,
}

impl StructuredWorkPlan {
    /// Decode an untrusted provider-authored plan into OpenLife's typed plan
    /// contract. OpenAI-compatible providers that do not support strict tool
    /// schemas may add explanatory properties next to otherwise valid
    /// function arguments. Those properties cannot grant authority and are
    /// intentionally discarded before the same strict semantic validation
    /// used for canonical plans.
    ///
    /// This is provider-agnostic transport normalization: required fields,
    /// value types, enum values, dependency references, capability targets,
    /// and completion rules remain fail-closed.
    pub fn parse_provider_output_and_validate(
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
        let mut value = serde_json::from_str::<serde_json::Value>(json)
            .map_err(|error| work_plan_json_error_code(error, json))?;
        project_provider_work_plan_fields(&mut value);
        Self::parse_and_validate(
            &serde_json::to_string(&value)
                .map_err(|_| "work_plan_serialization_failed".to_string())?,
            allowed_kinds,
            allowed_mcp_target_ids,
        )
    }

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
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
            if let Some(error) = provider_work_plan_shape_error(&value) {
                return Err(error);
            }
        }
        let mut plan: Self =
            serde_json::from_str(json).map_err(|error| work_plan_json_error_code(error, json))?;
        plan.normalize_model_step_ids_if_needed()?;
        plan.normalize_model_requirement_ids_if_needed()?;
        plan.validate(allowed_kinds, allowed_mcp_target_ids)?;
        Ok(plan)
    }

    /// Step ids are model-authored graph labels, not capability or authority
    /// identities. Some otherwise valid providers emit labels such as
    /// `step-1` or `Step1` despite the requested schema. Canonicalize that
    /// presentation-only variance before validation while preserving the
    /// exact dependency graph. Duplicate or unbounded labels still fail
    /// closed because their dependency meaning would be ambiguous.
    fn normalize_model_step_ids_if_needed(&mut self) -> Result<(), String> {
        if self
            .steps
            .iter()
            .all(|step| validate_step_id(&step.id).is_ok())
        {
            return Ok(());
        }

        let mut replacements = HashMap::new();
        for (index, step) in self.steps.iter().enumerate() {
            if step.id.is_empty() || step.id.len() > 256 {
                return Err("work_plan_step_id_invalid".into());
            }
            if replacements
                .insert(step.id.clone(), format!("step{}", index + 1))
                .is_some()
            {
                return Err("work_plan_step_id_duplicate".into());
            }
        }

        for step in &mut self.steps {
            step.id = replacements
                .get(&step.id)
                .cloned()
                .ok_or_else(|| "work_plan_step_id_invalid".to_string())?;
            for dependency in &mut step.depends_on {
                if let Some(replacement) = replacements.get(dependency) {
                    *dependency = replacement.clone();
                }
            }
        }
        Ok(())
    }

    /// Completion requirement ids are runtime tracking labels, not semantic
    /// content, evidence, or authority. Models therefore do not need to invent
    /// them. Preserve a complete valid set for stable replay, but when any id
    /// is omitted or malformed assign deterministic bounded ids to the whole
    /// list. Duplicate explicit labels remain rejected rather than silently
    /// collapsing two independently checkable requirements.
    fn normalize_model_requirement_ids_if_needed(&mut self) -> Result<(), String> {
        let mut explicit_ids = HashSet::new();
        for requirement in &self.completion.requirements {
            if !requirement.id.is_empty() && !explicit_ids.insert(requirement.id.as_str()) {
                return Err("work_plan_requirement_id_duplicate".into());
            }
        }
        if self
            .completion
            .requirements
            .iter()
            .all(|requirement| validate_step_id(&requirement.id).is_ok())
        {
            return Ok(());
        }
        for (index, requirement) in self.completion.requirements.iter_mut().enumerate() {
            requirement.id = format!("requirement{}", index + 1);
        }
        Ok(())
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
        if self.completion.requirements.len() > MAX_WORK_COMPLETION_REQUIREMENTS {
            return Err("work_plan_requirement_count_invalid".into());
        }
        if self.completion.requires_verification && self.completion.requirements.is_empty() {
            return Err("work_plan_verification_requirements_missing".into());
        }
        if !self.completion.requires_verification && !self.completion.requirements.is_empty() {
            return Err("work_plan_requirements_without_verification".into());
        }
        let mut requirement_ids = HashSet::new();
        for requirement in &self.completion.requirements {
            validate_step_id(&requirement.id)
                .map_err(|_| "work_plan_requirement_id_invalid".to_string())?;
            if !requirement_ids.insert(requirement.id.as_str()) {
                return Err("work_plan_requirement_id_duplicate".into());
            }
            if requirement.description.trim().is_empty()
                || requirement.description.chars().count() > 320
                || requirement.description.chars().any(char::is_control)
            {
                return Err("work_plan_requirement_description_invalid".into());
            }
            if requirement.evidence_kind == WorkCompletionEvidenceKind::Source
                && !self.steps.iter().any(|step| {
                    step.required
                        && matches!(
                            step.kind,
                            WorkPlanStepKind::ReadImportedDocument
                                | WorkPlanStepKind::ReadWorkspaceFile
                                | WorkPlanStepKind::WebSearch
                                | WorkPlanStepKind::WebFetch
                                | WorkPlanStepKind::ReadMcp
                        )
                })
            {
                return Err("work_plan_source_requirement_without_source_step".into());
            }
        }
        if self.completion.result_kind == WorkResultKind::Artifact
            && !self
                .steps
                .iter()
                .any(|step| step.required && step.kind == WorkPlanStepKind::DraftArtifact)
        {
            return Err("work_plan_artifact_step_missing".into());
        }
        if self.completion.requires_review_before_write
            && self.completion.result_kind != WorkResultKind::Artifact
        {
            return Err("work_plan_review_without_artifact".into());
        }
        if self.source_constraints.required_web_domains.len() > 8 {
            return Err("work_plan_web_domain_count_invalid".into());
        }
        let mut domains = HashSet::new();
        for domain in &self.source_constraints.required_web_domains {
            if !validate_required_web_domain(domain) || !domains.insert(domain.as_str()) {
                return Err("work_plan_web_domain_invalid".into());
            }
        }
        if !self.source_constraints.required_web_domains.is_empty()
            && !self.steps.iter().any(|step| {
                matches!(
                    step.kind,
                    WorkPlanStepKind::WebSearch | WorkPlanStepKind::WebFetch
                )
            })
        {
            return Err("work_plan_web_domain_without_web_step".into());
        }
        if self
            .steps
            .iter()
            .any(|step| step.kind == WorkPlanStepKind::PersonalIntelligence)
            && self.steps.iter().any(|step| {
                !matches!(
                    step.kind,
                    WorkPlanStepKind::Analyze
                        | WorkPlanStepKind::PersonalIntelligence
                        | WorkPlanStepKind::DeliverResult
                )
            })
        {
            return Err("work_plan_personal_intelligence_must_be_standalone".into());
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

        let artifact_mechanically_required =
            required_kinds.contains(&WorkPlanStepKind::DraftArtifact);
        if artifact_mechanically_required && self.completion.result_kind != WorkResultKind::Artifact
        {
            return Err("work_plan_required_result_kind_mismatch".into());
        }
        Ok(())
    }

    pub fn canonical_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|_| "work_plan_serialization_failed".into())
    }
}

fn retain_object_fields(value: &mut serde_json::Value, allowed: &[&str]) {
    if let Some(object) = value.as_object_mut() {
        object.retain(|key, _| allowed.contains(&key.as_str()));
    }
}

fn project_provider_work_plan_fields(value: &mut serde_json::Value) {
    retain_object_fields(
        value,
        &["schemaVersion", "steps", "completion", "sourceConstraints"],
    );
    let Some(root) = value.as_object_mut() else {
        return;
    };
    if let Some(steps) = root
        .get_mut("steps")
        .and_then(serde_json::Value::as_array_mut)
    {
        for step in steps {
            // targetContractDigest is deliberately excluded: only the runtime
            // may bind a manifest digest after validating targetId.
            retain_object_fields(step, &["id", "kind", "required", "dependsOn", "targetId"]);
        }
    }
    if let Some(completion) = root.get_mut("completion") {
        retain_object_fields(
            completion,
            &[
                "resultKind",
                "requiresVerification",
                "requirements",
                "requiresReviewBeforeWrite",
            ],
        );
        if let Some(requirements) = completion
            .get_mut("requirements")
            .and_then(serde_json::Value::as_array_mut)
        {
            for requirement in requirements {
                retain_object_fields(
                    requirement,
                    &[
                        "id",
                        "description",
                        "evidenceKind",
                        "kind",
                        "allowTransparentLimitation",
                    ],
                );
            }
        }
    }
    if let Some(source_constraints) = root.get_mut("sourceConstraints") {
        retain_object_fields(source_constraints, &["requiredWebDomains"]);
    }
}

/// Return a metadata-only description of the first provider transport shape
/// mismatch. The path and JSON type are safe to persist; provider-authored
/// values and the original response remain private. This keeps failures
/// actionable across OpenAI-compatible providers without adding a
/// provider-specific parser or weakening the typed plan contract.
fn provider_work_plan_shape_error(value: &serde_json::Value) -> Option<String> {
    fn type_name(value: &serde_json::Value) -> &'static str {
        match value {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }

    fn expect_type(
        value: Option<&serde_json::Value>,
        path: &str,
        expected: &str,
        predicate: impl FnOnce(&serde_json::Value) -> bool,
    ) -> Option<String> {
        let value = value?;
        (!predicate(value)).then(|| {
            format!(
                "work_plan_json_shape_invalid:{path}:{}_expected_{expected}",
                type_name(value)
            )
        })
    }

    let root = match value.as_object() {
        Some(root) => root,
        None => {
            return Some(format!(
                "work_plan_json_shape_invalid:root:{}_expected_object",
                type_name(value)
            ))
        }
    };
    if let Some(error) = expect_type(root.get("schemaVersion"), "schemaVersion", "string", |v| {
        v.is_string()
    }) {
        return Some(error);
    }
    if let Some(error) = expect_type(root.get("steps"), "steps", "array", |v| v.is_array()) {
        return Some(error);
    }
    if let Some(steps) = root.get("steps").and_then(serde_json::Value::as_array) {
        for (index, step) in steps.iter().enumerate() {
            let path = format!("steps[{index}]");
            let step = match step.as_object() {
                Some(step) => step,
                None => {
                    return Some(format!(
                        "work_plan_json_shape_invalid:{path}:{}_expected_object",
                        type_name(step)
                    ))
                }
            };
            for field in ["id", "kind"] {
                if let Some(error) =
                    expect_type(step.get(field), &format!("{path}.{field}"), "string", |v| {
                        v.is_string()
                    })
                {
                    return Some(error);
                }
            }
            if step
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|kind| {
                    !matches!(
                        kind,
                        "analyze"
                            | "personal_intelligence"
                            | "read_imported_document"
                            | "read_workspace_file"
                            | "web_search"
                            | "web_fetch"
                            | "use_selected_skill"
                            | "read_mcp"
                            | "draft_artifact"
                            | "verify"
                            | "deliver_result"
                    )
                })
            {
                return Some(format!(
                    "work_plan_json_shape_invalid:{path}.kind:enum_invalid"
                ));
            }
            if let Some(error) = expect_type(
                step.get("required"),
                &format!("{path}.required"),
                "boolean",
                |v| v.is_boolean(),
            ) {
                return Some(error);
            }
            if let Some(error) = expect_type(
                step.get("dependsOn"),
                &format!("{path}.dependsOn"),
                "array",
                |v| v.is_array(),
            ) {
                return Some(error);
            }
            if let Some(dependencies) = step.get("dependsOn").and_then(serde_json::Value::as_array)
            {
                for (dependency_index, dependency) in dependencies.iter().enumerate() {
                    if !dependency.is_string() {
                        return Some(format!(
                            "work_plan_json_shape_invalid:{path}.dependsOn[{dependency_index}]:{}_expected_string",
                            type_name(dependency)
                        ));
                    }
                }
            }
            if let Some(target_id) = step.get("targetId") {
                // `targetId` is an Option in the canonical contract. Several
                // OpenAI-compatible function-call implementations serialize
                // an absent optional property as JSON null. Serde already
                // normalizes that to None, so the metadata-only diagnostic
                // must not reject a shape the typed decoder accepts.
                if !target_id.is_null() && !target_id.is_string() {
                    return Some(format!(
                        "work_plan_json_shape_invalid:{path}.targetId:{}_expected_string_or_null",
                        type_name(target_id)
                    ));
                }
            }
        }
    }

    if let Some(error) = expect_type(root.get("completion"), "completion", "object", |v| {
        v.is_object()
    }) {
        return Some(error);
    }
    if let Some(completion) = root
        .get("completion")
        .and_then(serde_json::Value::as_object)
    {
        if let Some(error) = expect_type(
            completion.get("resultKind"),
            "completion.resultKind",
            "string",
            |v| v.is_string(),
        ) {
            return Some(error);
        }
        if completion
            .get("resultKind")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kind| !matches!(kind, "answer" | "artifact"))
        {
            return Some("work_plan_json_shape_invalid:completion.resultKind:enum_invalid".into());
        }
        for field in ["requiresVerification", "requiresReviewBeforeWrite"] {
            if let Some(error) = expect_type(
                completion.get(field),
                &format!("completion.{field}"),
                "boolean",
                |v| v.is_boolean(),
            ) {
                return Some(error);
            }
        }
        if let Some(error) = expect_type(
            completion.get("requirements"),
            "completion.requirements",
            "array",
            |v| v.is_array(),
        ) {
            return Some(error);
        }
        if let Some(requirements) = completion
            .get("requirements")
            .and_then(serde_json::Value::as_array)
        {
            for (index, requirement) in requirements.iter().enumerate() {
                let path = format!("completion.requirements[{index}]");
                let requirement = match requirement.as_object() {
                    Some(requirement) => requirement,
                    None => {
                        return Some(format!(
                            "work_plan_json_shape_invalid:{path}:{}_expected_object",
                            type_name(requirement)
                        ))
                    }
                };
                for field in ["id", "description", "evidenceKind", "kind"] {
                    if let Some(value) = requirement.get(field) {
                        if !value.is_string() {
                            return Some(format!(
                                "work_plan_json_shape_invalid:{path}.{field}:{}_expected_string",
                                type_name(value)
                            ));
                        }
                    }
                }
                if requirement.contains_key("evidenceKind") && requirement.contains_key("kind") {
                    return Some(format!(
                        "work_plan_json_shape_invalid:{path}.evidenceKind:duplicate_alias"
                    ));
                }
                if requirement
                    .get("evidenceKind")
                    .or_else(|| requirement.get("kind"))
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|kind| !matches!(kind, "result" | "source"))
                {
                    return Some(format!(
                        "work_plan_json_shape_invalid:{path}.evidenceKind:enum_invalid"
                    ));
                }
                if let Some(value) = requirement.get("allowTransparentLimitation") {
                    if !value.is_boolean() {
                        return Some(format!(
                            "work_plan_json_shape_invalid:{path}.allowTransparentLimitation:{}_expected_boolean",
                            type_name(value)
                        ));
                    }
                }
            }
        }
    }

    if let Some(error) = expect_type(
        root.get("sourceConstraints"),
        "sourceConstraints",
        "object",
        |v| v.is_object(),
    ) {
        return Some(error);
    }
    if let Some(source_constraints) = root
        .get("sourceConstraints")
        .and_then(serde_json::Value::as_object)
    {
        if let Some(error) = expect_type(
            source_constraints.get("requiredWebDomains"),
            "sourceConstraints.requiredWebDomains",
            "array",
            |v| v.is_array(),
        ) {
            return Some(error);
        }
        if let Some(domains) = source_constraints
            .get("requiredWebDomains")
            .and_then(serde_json::Value::as_array)
        {
            for (index, domain) in domains.iter().enumerate() {
                if !domain.is_string() {
                    return Some(format!(
                        "work_plan_json_shape_invalid:sourceConstraints.requiredWebDomains[{index}]:{}_expected_string",
                        type_name(domain)
                    ));
                }
            }
        }
    }
    None
}

/// Keep provider-shape failures diagnosable without persisting or surfacing
/// the provider's raw response. Field names are part of OpenLife's trusted
/// schema, while values may contain user or model-authored content and must
/// not become lifecycle reason codes.
fn work_plan_json_error_code(error: serde_json::Error, json: &str) -> String {
    let message = error.to_string();
    for (prefix, code) in [
        ("missing field `", "work_plan_json_missing_field"),
        ("unknown field `", "work_plan_json_unknown_field"),
        ("duplicate field `", "work_plan_json_duplicate_field"),
    ] {
        if let Some(field) = message
            .split_once(prefix)
            .and_then(|(_, tail)| tail.split_once('`').map(|(field, _)| field))
            .filter(|field| {
                !field.is_empty()
                    && field.len() <= 64
                    && field
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '_')
            })
        {
            let location = if code == "work_plan_json_unknown_field" {
                work_plan_unknown_field_location(json, field)
            } else {
                None
            };
            return format!("{code}:{}", location.unwrap_or(field));
        }
    }
    match error.classify() {
        serde_json::error::Category::Syntax | serde_json::error::Category::Eof => {
            "work_plan_json_syntax_invalid".into()
        }
        serde_json::error::Category::Data => "work_plan_json_shape_invalid".into(),
        serde_json::error::Category::Io => "work_plan_json_invalid".into(),
    }
}

fn work_plan_unknown_field_location<'a>(json: &str, field: &'a str) -> Option<&'a str> {
    let value = serde_json::from_str::<serde_json::Value>(json).ok()?;
    let root = value.as_object()?;
    if root.contains_key(field)
        && !["schemaVersion", "steps", "completion", "sourceConstraints"].contains(&field)
    {
        return Some(match field {
            "kind" => "root_kind",
            _ => field,
        });
    }
    if let Some(completion) = root
        .get("completion")
        .and_then(serde_json::Value::as_object)
    {
        if completion.contains_key(field)
            && ![
                "resultKind",
                "requiresVerification",
                "requirements",
                "requiresReviewBeforeWrite",
            ]
            .contains(&field)
        {
            return Some(match field {
                "kind" => "completion_kind",
                _ => field,
            });
        }
        if root
            .get("completion")
            .and_then(|completion| completion.get("requirements"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|requirements| {
                requirements.iter().any(|requirement| {
                    requirement
                        .as_object()
                        .is_some_and(|requirement| requirement.contains_key(field))
                })
            })
            && !["id", "description", "evidenceKind"].contains(&field)
        {
            return Some(match field {
                "kind" => "requirement_kind",
                _ => field,
            });
        }
    }
    if root
        .get("sourceConstraints")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|constraints| constraints.contains_key(field))
        && field != "requiredWebDomains"
    {
        return Some(match field {
            "kind" => "source_constraints_kind",
            _ => field,
        });
    }
    None
}

fn validate_required_web_domain(domain: &str) -> bool {
    if domain.is_empty()
        || domain.len() > 253
        || domain != domain.to_ascii_lowercase()
        || domain.starts_with('.')
        || domain.ends_with('.')
        || domain.contains("..")
    {
        return false;
    }
    let labels = domain.split('.').collect::<Vec<_>>();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
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
            // This is an emergency loop ceiling, not the execution design and
            // not a target for ordinary work. Tool decisions now happen
            // just-in-time after observations; source binding is part of the
            // terminal AgentStep; the runtime no longer budgets a batch of
            // pre-generated arguments plus a second same-model grounding pass.
            // `max_provider_attempts` is the absolute emergency ceiling, not
            // one undifferentiated bucket. Twelve ordinary Agent decisions
            // are available for planning, observed tool turns, and terminal
            // candidates; two final slots are reserved for the independent
            // semantic verifier. Ordinary execution cannot consume that
            // completion reserve.
            max_provider_attempts: 14,
            // This is one hard emergency ceiling for the whole observed tool
            // loop. Do not split it into hidden "ordinary" and "recovery"
            // pools: the provider selects the next action from current
            // observations, while the runtime rejects duplicates and unsafe
            // calls. A hidden pool can strand a valid multi-source task before
            // semantic verification has had a chance to inspect a candidate.
            max_tool_attempts: 12,
            // Provider attempts, declared plan Items, tool call/observation
            // pairs, Artifact verification and FinalResult all share this
            // canonical Item ceiling. Keep enough room for the bounded loop's
            // legal terminal path without allowing unbounded execution.
            max_total_items: 48,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkRunBudgetUsage {
    pub plan_attempts: u32,
    pub provider_attempts: u32,
    /// Provider attempts whose sole purpose is independent semantic
    /// verification. They remain part of the total provider count, but must
    /// not be consumed by ordinary Agent decisions before a candidate can be
    /// checked.
    pub verification_attempts: u32,
    pub tool_attempts: u32,
    pub total_items: u32,
}

impl WorkRunBudgetPolicy {
    const SEMANTIC_VERIFICATION_RESERVE: u32 = 2;

    fn max_agent_provider_attempts(self) -> u32 {
        self.max_provider_attempts
            .saturating_sub(Self::SEMANTIC_VERIFICATION_RESERVE)
    }

    pub fn remaining_tool_attempts(self, usage: WorkRunBudgetUsage) -> u32 {
        self.max_tool_attempts.saturating_sub(usage.tool_attempts)
    }

    pub fn admit_plan(self, usage: WorkRunBudgetUsage) -> Result<(), String> {
        if usage.plan_attempts >= self.max_plan_attempts {
            return Err("work_plan_attempt_budget_exhausted".into());
        }
        self.admit_provider(usage)
    }

    pub fn admit_provider(self, usage: WorkRunBudgetUsage) -> Result<(), String> {
        let agent_attempts = usage
            .provider_attempts
            .saturating_sub(usage.verification_attempts);
        if usage.provider_attempts >= self.max_provider_attempts
            || agent_attempts >= self.max_agent_provider_attempts()
        {
            return Err("work_provider_budget_exhausted".into());
        }
        self.admit_item(usage)
    }

    pub fn admit_semantic_verification(self, usage: WorkRunBudgetUsage) -> Result<(), String> {
        if usage.provider_attempts >= self.max_provider_attempts
            || usage.verification_attempts >= Self::SEMANTIC_VERIFICATION_RESERVE
        {
            return Err("work_semantic_verification_budget_exhausted".into());
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

    fn agent_context<'a>(
        capabilities: &'a HashSet<String>,
        formats: &'a HashSet<String>,
        evidence: &'a HashSet<String>,
        artifacts: &'a HashSet<String>,
    ) -> AgentStepValidationContext<'a> {
        AgentStepValidationContext {
            allowed_capability_ids: capabilities,
            allowed_artifact_formats: formats,
            available_evidence_refs: evidence,
            available_artifact_refs: artifacts,
        }
    }

    #[test]
    fn model_step_selects_only_runtime_eligible_capabilities() {
        let allowed_capabilities =
            HashSet::from(["web.search".to_string(), "document.read".to_string()]);
        let allowed_formats = HashSet::from(["markdown".to_string()]);
        let empty = HashSet::new();
        let context = agent_context(&allowed_capabilities, &allowed_formats, &empty, &empty);
        let step = AgentStepEnvelope::parse_and_validate(
            r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"tool_call","payload":{"capabilityId":"web.search","arguments":{"query":"continuous learning"}}}}"#,
            &context,
        )
        .unwrap();
        assert_eq!(
            step.step,
            AgentStep::ToolCall(AgentToolCallStep {
                capability_id: "web.search".into(),
                arguments: serde_json::json!({"query": "continuous learning"}),
            })
        );

        assert_eq!(
            AgentStepEnvelope::parse_and_validate(
                r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"tool_call","payload":{"capabilityId":"browser.open","arguments":{"url":"https://example.com"}}}}"#,
                &context,
            )
            .unwrap_err(),
            "agent_step_capability_not_allowed"
        );

        assert!(
            AgentStepEnvelope::parse_and_validate(
                r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"tool_calls","payload":{"calls":[{"capabilityId":"web.search","arguments":{"query":"ChatGPT Work long tasks"}},{"capabilityId":"web.search","arguments":{"query":"Codex permission modes"}}]}}}"#,
                &context,
            )
            .is_ok(),
            "one model decision must be able to request several independently validated reads"
        );
        assert_eq!(
            AgentStepEnvelope::parse_and_validate(
                r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"tool_calls","payload":{"calls":[{"capabilityId":"web.search","arguments":{"query":"same"}},{"capabilityId":"web.search","arguments":{"query":"same"}}]}}}"#,
                &context,
            )
            .unwrap_err(),
            "agent_step_tool_call_duplicate"
        );
        assert_eq!(
            AgentStepEnvelope::parse_and_validate(
                r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"tool_calls","payload":{"calls":[{"capabilityId":"web.search","arguments":{"query":"one"}},{"capabilityId":"web.search","arguments":{"query":"two"}},{"capabilityId":"web.search","arguments":{"query":"three"}},{"capabilityId":"web.search","arguments":{"query":"four"}},{"capabilityId":"web.search","arguments":{"query":"five"}}]}}}"#,
                &context,
            )
            .unwrap_err(),
            "agent_step_tool_call_count_invalid"
        );
    }

    #[test]
    fn artifact_step_carries_content_but_cannot_mint_a_target_path() {
        let allowed_capabilities = HashSet::new();
        let allowed_formats = HashSet::from(["markdown".to_string()]);
        let empty = HashSet::new();
        let context = agent_context(&allowed_capabilities, &allowed_formats, &empty, &empty);
        let step = AgentStepEnvelope::parse_and_validate(
            r##"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"draft_artifact","payload":{"artifacts":[{"format":"markdown","suggestedName":"continuous-learning.md","content":"# Continuous learning"}]}}}"##,
            &context,
        )
        .unwrap();
        assert!(matches!(step.step, AgentStep::DraftArtifact(_)));

        assert_eq!(
            AgentStepEnvelope::parse_and_validate(
                r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"draft_artifact","payload":{"artifacts":[{"format":"markdown","suggestedName":"../outside.md","content":"unsafe"}]}}}"#,
                &context,
            )
            .unwrap_err(),
            "agent_step_artifact_name_invalid"
        );
    }

    #[test]
    fn provider_agent_step_projection_ignores_explanatory_fields_without_widening_authority() {
        let empty = HashSet::new();
        let formats = HashSet::from(["markdown".to_string()]);
        let context = agent_context(&empty, &formats, &empty, &empty);
        let step = AgentStepEnvelope::parse_provider_output_and_validate(
            r##"{"schemaVersion":"openlife.agent-step.v1","providerNote":"ignored","step":{"kind":"draft_artifact","stepRationale":"ignored","payload":{"artifacts":[{"format":"markdown","suggestedName":"result.md","content":"# Result","sourceBlocks":[],"mimeType":"text/markdown"}],"reviewBeforeWrite":false,"artifactCountNote":"one"}}}"##,
            &context,
        )
        .expect("known AgentStep fields survive provider projection");
        assert!(matches!(step.step, AgentStep::DraftArtifact(_)));
    }

    #[test]
    fn provider_agent_step_projection_still_rejects_invalid_typed_values() {
        let empty = HashSet::new();
        let formats = HashSet::from(["markdown".to_string()]);
        let context = agent_context(&empty, &formats, &empty, &empty);
        let error = AgentStepEnvelope::parse_provider_output_and_validate(
            r##"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"draft_artifact","payload":{"artifacts":[{"format":"markdown","suggestedName":"result.md","content":"# Result","sourceBlocks":"not-an-array","note":"ignored"}],"reviewBeforeWrite":false}}}"##,
            &context,
        )
        .unwrap_err();
        assert_eq!(error, "agent_step_json_invalid");
    }

    #[test]
    fn source_backed_steps_use_ordered_blocks_instead_of_text_anchors() {
        let empty = HashSet::new();
        let allowed_formats = HashSet::from(["markdown".to_string()]);
        let context = agent_context(&empty, &allowed_formats, &empty, &empty);
        let artifact = AgentStepEnvelope::parse_and_validate(
            r##"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"draft_artifact","payload":{"artifacts":[{"format":"markdown","suggestedName":"report.md","content":null,"sourceBlocks":[{"kind":"heading","text":"# Report","sourceRefs":[]},{"kind":"claim","text":"The same supported claim.","sourceRefs":["webref_aaaaaaaaaaaaaaaaaaaaaaaa"]},{"kind":"claim","text":"The same supported claim.","sourceRefs":["webref_aaaaaaaaaaaaaaaaaaaaaaaa"]}]}],"reviewBeforeWrite":false}}}"##,
            &context,
        )
        .unwrap();
        assert!(matches!(artifact.step, AgentStep::DraftArtifact(_)));

        let runtime_rendered_heading = AgentStepEnvelope::parse_and_validate(
            r##"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"draft_artifact","payload":{"artifacts":[{"format":"markdown","suggestedName":"report.md","content":null,"sourceBlocks":[{"kind":"heading","text":"Report","headingLevel":1,"sourceRefs":[]},{"kind":"claim","text":"A supported claim.","headingLevel":null,"sourceRefs":["webref_aaaaaaaaaaaaaaaaaaaaaaaa"]}]}],"reviewBeforeWrite":false}}}"##,
            &context,
        )
        .unwrap();
        assert!(matches!(
            runtime_rendered_heading.step,
            AgentStep::DraftArtifact(_)
        ));

        assert_eq!(
            AgentStepEnvelope::parse_and_validate(
                r##"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"draft_artifact","payload":{"artifacts":[{"format":"markdown","suggestedName":"report.md","content":"duplicate prose","sourceBlocks":[{"kind":"claim","text":"A claim.","sourceRefs":["webref_aaaaaaaaaaaaaaaaaaaaaaaa"]}]}],"reviewBeforeWrite":false}}}"##,
                &context,
            )
            .unwrap_err(),
            "agent_step_source_blocks_require_null_content"
        );
        assert_eq!(
            AgentStepEnvelope::parse_and_validate(
                r##"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"draft_artifact","payload":{"artifacts":[{"format":"markdown","suggestedName":"report.md","content":null,"sourceBlocks":[{"kind":"claim","text":"An unbound claim.","sourceRefs":[]}]}],"reviewBeforeWrite":false}}}"##,
                &context,
            )
            .unwrap_err(),
            "agent_step_source_claim_refs_missing"
        );
    }

    #[test]
    fn personal_intelligence_step_is_typed_and_cannot_smuggle_extra_authority() {
        let empty = HashSet::new();
        let context = agent_context(&empty, &empty, &empty, &empty);
        let step = AgentStepEnvelope::parse_and_validate(
            r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"personal_intelligence","payload":{"action":"remember","sourceSpan":"先给结论，再解释依据","memoryKind":"preference","scope":"personal"}}}"#,
            &context,
        )
        .unwrap();
        assert!(matches!(
            step.step,
            AgentStep::PersonalIntelligence(AgentPersonalIntelligenceStep {
                action: AgentPersonalIntelligenceAction::Remember,
                ..
            })
        ));
        assert_eq!(
            AgentStepEnvelope::parse_and_validate(
                r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"personal_intelligence","payload":{"action":"remember","sourceSpan":"fact","scope":"personal"}}}"#,
                &context,
            )
            .unwrap_err(),
            "agent_personal_intelligence_remember_invalid"
        );
        assert_eq!(
            AgentStepEnvelope::parse_and_validate(
                r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"personal_intelligence","payload":{"action":"forget","query":"咖啡偏好","scope":"personal"}}}"#,
                &context,
            )
            .unwrap_err(),
            "agent_personal_intelligence_forget_invalid"
        );
        let suggestion = AgentStepEnvelope::parse_and_validate(
            r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"personal_intelligence","payload":{"action":"suggest_life_model","sourceSpan":"沟通保持简洁直接","lifeModelSection":"collaboration_preferences","lifeModelStatement":"沟通保持简洁直接"}}}"#,
            &context,
        )
        .unwrap();
        assert!(matches!(
            suggestion.step,
            AgentStep::PersonalIntelligence(AgentPersonalIntelligenceStep {
                action: AgentPersonalIntelligenceAction::SuggestLifeModel,
                ..
            })
        ));
    }

    #[test]
    fn final_step_references_existing_runtime_identities_without_claiming_completion() {
        let empty = HashSet::new();
        let evidence = HashSet::from(["item:web-search-1".to_string(), "item:1".to_string()]);
        let artifacts = HashSet::from(["artifact:report-1".to_string()]);
        let context = agent_context(&empty, &empty, &evidence, &artifacts);
        let step = AgentStepEnvelope::parse_and_validate(
            r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"final_answer","payload":{"content":"Research complete.","evidenceRefs":["item:web-search-1"],"artifactRefs":["artifact:report-1"]}}}"#,
            &context,
        )
        .unwrap();
        assert!(matches!(step.step, AgentStep::FinalAnswer(_)));

        assert_eq!(
            AgentStepEnvelope::parse_and_validate(
                r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"final_answer","payload":{"content":"Research complete.","evidenceRefs":["item:1","item:1"],"artifactRefs":[]}}}"#,
                &context,
            )
            .unwrap_err(),
            "agent_step_reference_duplicate"
        );

        assert_eq!(
            AgentStepEnvelope::parse_and_validate(
                r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"final_answer","payload":{"content":"Research complete.","evidenceRefs":["item:invented"],"artifactRefs":[]}}}"#,
                &context,
            )
            .unwrap_err(),
            "agent_step_reference_not_available"
        );
    }

    #[test]
    fn model_step_rejects_unknown_contract_fields() {
        let empty = HashSet::new();
        let context = agent_context(&empty, &empty, &empty, &empty);
        assert_eq!(
            AgentStepEnvelope::parse_and_validate(
                r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"ask_user","payload":{"question":"Which source?","permission":"all"}}}"#,
                &context,
            )
            .unwrap_err(),
            "agent_step_json_invalid"
        );
    }

    #[test]
    fn validates_a_bounded_dependency_ordered_plan() {
        let plan = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"verify","kind":"verify","required":true,"dependsOn":["research"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"answer","requiresVerification":true,"requirements":[{"id":"topic","description":"Direct source evidence supports the requested topic.","evidenceKind":"source"}]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(plan.steps.len(), 3);
    }

    #[test]
    fn runtime_assigns_non_authoritative_requirement_ids_when_the_model_omits_them() {
        let plan = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"verify","kind":"verify","required":true,"dependsOn":["research"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"answer","requiresVerification":true,"requirements":[{"description":"Official source evidence supports the requested topic.","evidenceKind":"source"},{"description":"The result clearly explains the requested topic.","evidenceKind":"result"}]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();

        assert_eq!(
            plan.completion
                .requirements
                .iter()
                .map(|requirement| requirement.id.as_str())
                .collect::<Vec<_>>(),
            vec!["requirement1", "requirement2"]
        );
    }

    #[test]
    fn plan_shape_failures_expose_only_the_rejected_schema_field() {
        assert_eq!(
            StructuredWorkPlan::parse_and_validate(
                r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":[]}],"completion":{"resultKind":"answer","requiresVerification":false,"requirements":[],"unexpected":"value"}}"#,
                &allowed(),
                &HashSet::new(),
            )
            .unwrap_err(),
            "work_plan_json_unknown_field:unexpected"
        );
        assert_eq!(
            StructuredWorkPlan::parse_and_validate(
                r#"{"schemaVersion":"openlife.work-plan.v3","kind":"plan","steps":[{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":[]}],"completion":{"resultKind":"answer","requiresVerification":false,"requirements":[]}}"#,
                &allowed(),
                &HashSet::new(),
            )
            .unwrap_err(),
            "work_plan_json_unknown_field:root_kind"
        );
        assert_eq!(
            StructuredWorkPlan::parse_and_validate(
                r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":[]}],"completion":{"requiresVerification":false,"requirements":[]}}"#,
                &allowed(),
                &HashSet::new(),
            )
            .unwrap_err(),
            "work_plan_json_missing_field:resultKind"
        );

        let aliased_requirement = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"verify","kind":"verify","required":true,"dependsOn":["research"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"answer","requiresVerification":true,"requirements":[{"id":"topic","description":"Official source evidence supports the requested topic.","kind":"source"}]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(
            aliased_requirement.completion.requirements[0].evidence_kind,
            WorkCompletionEvidenceKind::Source
        );
    }

    #[test]
    fn provider_plan_projection_ignores_explanatory_fields_without_widening_authority() {
        let plan = StructuredWorkPlan::parse_provider_output_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","providerNote":"not canonical","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[],"dependsOnNote":"starts first","targetContractDigest":"model-controlled"},{"id":"verify","kind":"verify","required":true,"dependsOn":["research"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"answer","requiresVerification":true,"requirements":[{"description":"Official source evidence supports the answer.","evidenceKind":"source","providerRationale":"use official docs"}],"requiresReviewBeforeWrite":false,"providerSummary":"ignored"},"sourceConstraints":{"requiredWebDomains":[],"providerDomainGuess":["example.com"]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();

        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].kind, WorkPlanStepKind::WebSearch);
        assert_eq!(plan.steps[0].target_contract_digest, None);
        assert!(plan.source_constraints.required_web_domains.is_empty());
        assert_eq!(plan.completion.requirements[0].id, "requirement1");
    }

    #[test]
    fn provider_plan_projection_still_rejects_invalid_typed_values() {
        let error = StructuredWorkPlan::parse_provider_output_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":"yes","dependsOn":[],"note":"ignored"},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["research"]}],"completion":{"resultKind":"answer","requiresVerification":false,"requirements":[],"requiresReviewBeforeWrite":false},"sourceConstraints":{"requiredWebDomains":[]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap_err();

        assert_eq!(
            error,
            "work_plan_json_shape_invalid:steps[0].required:string_expected_boolean"
        );

        let invalid_enum = StructuredWorkPlan::parse_provider_output_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"browse_the_web","required":true,"dependsOn":[]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["research"]}],"completion":{"resultKind":"artifact","requiresVerification":false,"requirements":[]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert_eq!(
            invalid_enum,
            "work_plan_json_shape_invalid:steps[0].kind:enum_invalid"
        );
    }

    #[test]
    fn provider_plan_accepts_null_for_an_absent_optional_target() {
        let plan = StructuredWorkPlan::parse_provider_output_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[],"targetId":null},{"id":"verify","kind":"verify","required":true,"dependsOn":["research"],"targetId":null},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"],"targetId":null}],"completion":{"resultKind":"answer","requiresVerification":true,"requirements":[{"description":"Official source evidence supports the answer.","evidenceKind":"source"}],"requiresReviewBeforeWrite":false},"sourceConstraints":{"requiredWebDomains":[]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();

        assert!(plan.steps.iter().all(|step| step.target_id.is_none()));
    }

    #[test]
    fn verified_plan_requires_bounded_independent_semantic_requirements() {
        let missing = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"verify","kind":"verify","required":true,"dependsOn":["research"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"answer","requiresVerification":true,"requirements":[]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert_eq!(missing, "work_plan_verification_requirements_missing");

        let duplicate = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"verify","kind":"verify","required":true,"dependsOn":["research"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"answer","requiresVerification":true,"requirements":[{"id":"topic","description":"First requested subject.","evidenceKind":"source"},{"id":"topic","description":"Second requested subject.","evidenceKind":"source"}]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert_eq!(duplicate, "work_plan_requirement_id_duplicate");
    }

    #[test]
    fn canonicalizes_non_authoritative_model_step_labels() {
        let plan = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"Web-Research","kind":"web_search","required":true,"dependsOn":[]},{"id":"Verify Result","kind":"verify","required":true,"dependsOn":["Web-Research"]},{"id":"Deliver.Result","kind":"deliver_result","required":true,"dependsOn":["Verify Result"]}],"completion":{"resultKind":"answer","requiresVerification":true,"requirements":[{"id":"topic","description":"Direct source evidence supports the requested topic.","evidenceKind":"source"}]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();

        assert_eq!(
            plan.steps
                .iter()
                .map(|step| step.id.as_str())
                .collect::<Vec<_>>(),
            vec!["step1", "step2", "step3"]
        );
        assert_eq!(plan.steps[1].depends_on, vec!["step1"]);
        assert_eq!(plan.steps[2].depends_on, vec!["step2"]);
    }

    #[test]
    fn rejects_ambiguous_duplicate_model_step_labels() {
        let error = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"Step-1","kind":"web_search","required":true,"dependsOn":[]},{"id":"Step-1","kind":"verify","required":true,"dependsOn":["Step-1"]},{"id":"Deliver","kind":"deliver_result","required":true,"dependsOn":["Step-1"]}],"completion":{"resultKind":"answer","requiresVerification":true,"requirements":[{"id":"topic","description":"Direct source evidence supports the requested topic.","evidenceKind":"source"}]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap_err();

        assert_eq!(error, "work_plan_step_id_duplicate");
    }

    #[test]
    fn rejects_ungranted_capability_and_forward_dependency() {
        let ungranted = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"read","kind":"read_imported_document","required":true,"dependsOn":[]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["read"]}],"completion":{"resultKind":"answer","requiresVerification":false,"requirements":[]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert_eq!(ungranted, "work_plan_capability_not_allowed");

        let forward = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":["deliver"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":[]}],"completion":{"resultKind":"answer","requiresVerification":false,"requirements":[]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert_eq!(forward, "work_plan_dependency_order_invalid");
    }

    #[test]
    fn required_capability_floor_cannot_be_omitted_by_a_valid_allowed_plan() {
        let answer_only = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"analyze","kind":"analyze","required":true,"dependsOn":[]},{"id":"verify","kind":"verify","required":true,"dependsOn":["analyze"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"answer","requiresVerification":true,"requirements":[{"id":"outcome","description":"The result satisfies the authenticated user outcome.","evidenceKind":"result"}]}}"#,
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
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"verify","kind":"verify","required":true,"dependsOn":["research"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"answer","requiresVerification":true,"requirements":[{"id":"topic","description":"Direct source evidence supports the requested topic.","evidenceKind":"source"}]}}"#,
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
        assert_eq!(policy.max_provider_attempts, 14);
        assert_eq!(policy.max_tool_attempts, 12);
        assert_eq!(policy.max_total_items, 48);
        assert_eq!(
            policy.remaining_tool_attempts(WorkRunBudgetUsage {
                tool_attempts: 8,
                ..WorkRunBudgetUsage::default()
            }),
            4,
            "all unused tool attempts remain visible to the observed Agent loop"
        );
        assert_eq!(
            policy
                .admit_tool(WorkRunBudgetUsage {
                    tool_attempts: policy.max_tool_attempts,
                    ..WorkRunBudgetUsage::default()
                })
                .unwrap_err(),
            "work_tool_budget_exhausted"
        );
        assert_eq!(
            policy
                .admit_provider(WorkRunBudgetUsage {
                    provider_attempts: policy.max_provider_attempts,
                    ..WorkRunBudgetUsage::default()
                })
                .unwrap_err(),
            "work_provider_budget_exhausted"
        );
        assert!(policy
            .admit_provider(WorkRunBudgetUsage {
                // The native Stage 7 regression reached a legitimate ninth
                // provider decision only after bounded schema and citation
                // correction paths had been exercised.
                provider_attempts: 8,
                ..WorkRunBudgetUsage::default()
            })
            .is_ok());
        assert!(policy
            .admit_semantic_verification(WorkRunBudgetUsage {
                provider_attempts: policy.max_agent_provider_attempts(),
                verification_attempts: 0,
                ..WorkRunBudgetUsage::default()
            })
            .is_ok(), "ordinary Agent decisions must not consume the independent completion-verification reserve");
        assert_eq!(
            policy
                .admit_semantic_verification(WorkRunBudgetUsage {
                    provider_attempts: policy.max_provider_attempts - 1,
                    verification_attempts: WorkRunBudgetPolicy::SEMANTIC_VERIFICATION_RESERVE,
                    ..WorkRunBudgetUsage::default()
                })
                .unwrap_err(),
            "work_semantic_verification_budget_exhausted"
        );

        let plan = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["research"]}],"completion":{"resultKind":"answer","requiresVerification":false,"requirements":[]}}"#,
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

        let review_without_artifact = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":[]}],"completion":{"resultKind":"answer","requiresVerification":false,"requirements":[],"requiresReviewBeforeWrite":true}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert_eq!(review_without_artifact, "work_plan_review_without_artifact");

        let reviewed_artifact = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"draft","kind":"draft_artifact","required":true,"dependsOn":[]},{"id":"verify","kind":"verify","required":true,"dependsOn":["draft"]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["verify"]}],"completion":{"resultKind":"artifact","requiresVerification":true,"requirements":[{"id":"artifact","description":"The requested Artifact is complete.","evidenceKind":"result"}],"requiresReviewBeforeWrite":true}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();
        assert!(reviewed_artifact.completion.requires_review_before_write);

        let official_source = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["research"]}],"completion":{"resultKind":"answer","requiresVerification":false,"requirements":[]},"sourceConstraints":{"requiredWebDomains":["openai.com"]}}"#,
            &allowed(),
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(
            official_source.source_constraints.required_web_domains,
            ["openai.com"]
        );
        assert_eq!(
            StructuredWorkPlan::parse_and_validate(
                r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"research","kind":"web_search","required":true,"dependsOn":[]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["research"]}],"completion":{"resultKind":"answer","requiresVerification":false,"requirements":[]},"sourceConstraints":{"requiredWebDomains":["https://openai.com/"]}}"#,
                &allowed(),
                &HashSet::new(),
            )
            .unwrap_err(),
            "work_plan_web_domain_invalid"
        );
    }

    #[test]
    fn mcp_target_must_be_an_exact_allowed_manifest_identity() {
        let allowed_kinds =
            HashSet::from([WorkPlanStepKind::ReadMcp, WorkPlanStepKind::DeliverResult]);
        let allowed_targets = HashSet::from(["weather.current".to_string()]);
        let plan = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"read","kind":"read_mcp","required":true,"dependsOn":[],"targetId":"weather.current"},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["read"]}],"completion":{"resultKind":"answer","requiresVerification":false,"requirements":[]}}"#,
            &allowed_kinds,
            &allowed_targets,
        )
        .unwrap();
        assert_eq!(plan.steps[0].target_id.as_deref(), Some("weather.current"));

        let error = StructuredWorkPlan::parse_and_validate(
            r#"{"schemaVersion":"openlife.work-plan.v3","steps":[{"id":"read","kind":"read_mcp","required":true,"dependsOn":[],"targetId":"unregistered.tool"},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["read"]}],"completion":{"resultKind":"answer","requiresVerification":false,"requirements":[]}}"#,
            &allowed_kinds,
            &allowed_targets,
        )
        .unwrap_err();
        assert_eq!(error, "work_plan_mcp_target_not_allowed");
    }
}
