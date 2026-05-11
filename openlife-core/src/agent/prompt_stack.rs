use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Purpose category for a prompt block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptPurpose {
    BaseSystem,
    LifeModel,
    MemoryEvidence,
    Task,
    Planning,
    Tool,
    Proposal,
    Privacy,
    OutputFormat,
    SubAgent,
    Custom(String),
}

impl PromptPurpose {
    pub fn as_str(&self) -> &str {
        match self {
            PromptPurpose::BaseSystem => "base_system",
            PromptPurpose::LifeModel => "life_model",
            PromptPurpose::MemoryEvidence => "memory_evidence",
            PromptPurpose::Task => "task",
            PromptPurpose::Planning => "planning",
            PromptPurpose::Tool => "tool",
            PromptPurpose::Proposal => "proposal",
            PromptPurpose::Privacy => "privacy",
            PromptPurpose::OutputFormat => "output_format",
            PromptPurpose::SubAgent => "sub_agent",
            PromptPurpose::Custom(s) => s.as_str(),
        }
    }
}

/// Privacy classification for a prompt block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptPrivacyLevel {
    Public,
    Internal,
    Sensitive,
    StrictlyLocal,
}

/// A single versioned, policy-aware block of prompt text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptBlock {
    pub id: String,
    pub version: String,
    pub purpose: PromptPurpose,
    pub content: String,
    pub privacy_level: PromptPrivacyLevel,
    pub cloud_allowed: bool,
    pub token_budget: usize,
    pub applies_to: Vec<String>,
}

impl PromptBlock {
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        purpose: PromptPurpose,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            purpose,
            content: content.into(),
            privacy_level: PromptPrivacyLevel::Internal,
            cloud_allowed: true,
            token_budget: 0,
            applies_to: Vec::new(),
        }
    }

    pub fn with_privacy(mut self, level: PromptPrivacyLevel) -> Self {
        self.privacy_level = level.clone();
        if matches!(level, PromptPrivacyLevel::StrictlyLocal) {
            self.cloud_allowed = false;
        }
        self
    }

    pub fn with_cloud_allowed(mut self, allowed: bool) -> Self {
        self.cloud_allowed = allowed;
        self
    }

    pub fn with_token_budget(mut self, budget: usize) -> Self {
        self.token_budget = budget;
        self
    }

    pub fn with_applies_to(mut self, agents: Vec<String>) -> Self {
        self.applies_to = agents;
        self
    }

    /// Factory: create a PlanningPrompt block for PlanMode.
    /// Includes planner role instructions, tool permissions, and output expectations.
    pub fn planning() -> Self {
        Self::new(
            "planning_prompt",
            "1.0.0",
            PromptPurpose::Planning,
            PLANNING_PROMPT_CONTENT,
        )
        .with_privacy(PromptPrivacyLevel::Internal)
        .with_cloud_allowed(true)
        .with_token_budget(800)
        .with_applies_to(vec!["Planner".into(), "PlanMode".into()])
    }

    pub fn base_identity() -> Self {
        Self::new(
            "base_identity",
            "1.0.0",
            PromptPurpose::BaseSystem,
            "你是 OpenLife，用户的终身成长合伙人。你的人设和行为必须严格基于下面这份「人生模型」。\n\
             请记住以下关于用户的信息，所有建议都必须经过人生模型的价值观过滤：",
        )
        .with_privacy(PromptPrivacyLevel::Internal)
    }

    pub fn behavioral_guidelines() -> Self {
        Self::new(
            "behavioral_guidelines",
            "1.0.0",
            PromptPurpose::BaseSystem,
            "在每次回应时：\n\
             1. 优先考虑用户的核心价值观\n\
             2. 结合用户当前的目标和状态给出建议\n\
             3. 语气要符合用户定义的人格特质\n\
             4. 如果用户的请求与人生模型冲突，请温和地提醒并引导对齐\n\
             5. 如果用户的状态显示精力低、压力高或情绪低落，请主动表达关心并调整建议的强度和节奏",
        )
        .with_privacy(PromptPrivacyLevel::Internal)
    }

    pub fn tool_call_format() -> Self {
        Self::new(
            "tool_call_format",
            "1.0.0",
            PromptPurpose::Tool,
            "【工具调用规范】\n\
             当你需要调用工具时，请严格按以下 JSON 格式输出（不要包含其他自然语言）：\n\
             ```json\n\
             {\n  \"tool_calls\": [\n    {\n      \"name\": \"工具名称\",\n      \"arguments\": { \"参数名\": \"参数值\" }\n    }\n  ]\n}\n\
             ```\n\
             如果不需要工具，直接以自然语言回答用户。",
        )
        .with_cloud_allowed(true)
    }

    pub fn life_model_yaml(life_model: &crate::life_model::LifeModel) -> Self {
        let yaml = serde_yaml::to_string(life_model).unwrap_or_default();
        Self::new(
            "life_model_yaml",
            "1.0.0",
            PromptPurpose::LifeModel,
            format!("```yaml\n{}\n```", yaml),
        )
        .with_privacy(PromptPrivacyLevel::Sensitive)
    }

    pub fn state_hint(life_model: &crate::life_model::LifeModel) -> Self {
        let state = &life_model.state;
        let mut parts: Vec<String> = Vec::new();
        if !state.current_focus.is_empty() {
            parts.push(format!("- 当前重心: {}", state.current_focus));
        }
        if !state.emotional_state.current_mood.is_empty() {
            parts.push(format!(
                "- 当前心情: {} (压力{}/10, 满足度{}/10)",
                state.emotional_state.current_mood,
                state.emotional_state.stress_level,
                state.emotional_state.fulfillment_score
            ));
        }
        if !state.health_status.physical.is_empty() || !state.health_status.mental.is_empty() {
            parts.push(format!(
                "- 身心健康: {}/{} (精力{}/10)",
                state.health_status.physical,
                state.health_status.mental,
                state.health_status.energy_level
            ));
        }
        if !state.focus_areas.is_empty() {
            parts.push(format!("- 关注领域: {}", state.focus_areas.join(", ")));
        }
        if !state.recent_events.is_empty() {
            parts.push(format!("- 近期事件: {}", state.recent_events.join(", ")));
        }
        if !state.habit_streaks.is_empty() {
            let streaks: Vec<String> = state
                .habit_streaks
                .iter()
                .map(|h| format!("{}({}天)", h.name, h.streak_days))
                .collect();
            parts.push(format!("- 习惯连续: {}", streaks.join(", ")));
        }
        let hint = if parts.is_empty() {
            "暂无状态记录".to_string()
        } else {
            parts.join("\n")
        };
        Self::new(
            "state_hint",
            "1.0.0",
            PromptPurpose::LifeModel,
            format!("【用户当前状态摘要】\n{}", hint),
        )
        .with_privacy(PromptPrivacyLevel::Sensitive)
    }

    pub fn evolution_hint(life_model: &crate::life_model::LifeModel) -> Self {
        let rules = if life_model.evolution_rules.is_empty() {
            "暂无进化规则".to_string()
        } else {
            life_model.evolution_rules.join("\n")
        };
        Self::new(
            "evolution_hint",
            "1.0.0",
            PromptPurpose::LifeModel,
            format!("【自动进化规则（基于近期反馈与行为数据）】\n{}", rules),
        )
        .with_privacy(PromptPrivacyLevel::Internal)
    }

    pub fn life_model_summary(life_model: &crate::life_model::LifeModel) -> Self {
        let state = &life_model.state;
        let state_hint = if !state.current_focus.is_empty() {
            format!(
                "- 当前重心: {}\n- 当前心情: {}",
                state.current_focus, state.emotional_state.current_mood
            )
        } else {
            "暂无状态摘要".to_string()
        };
        let goal_summary = format!(
            "短期目标 {} 个，中期 {} 个，长期 {} 个，人生目标 {} 个，每日 {} 个",
            life_model.goals.short_term.len(),
            life_model.goals.medium_term.len(),
            life_model.goals.long_term.len(),
            life_model.goals.life_goals.len(),
            life_model.goals.daily.len(),
        );
        let value_names: Vec<&str> = life_model
            .identity
            .values
            .iter()
            .map(|v| v.name.as_str())
            .collect();
        let values_hint = if value_names.is_empty() {
            "暂无核心价值观".to_string()
        } else {
            format!("核心价值观: {}", value_names.join("、"))
        };
        let content = format!(
            "【用户状态摘要】\n{}\n\n【目标摘要】\n{}\n\n【价值观方向】\n{}",
            state_hint, goal_summary, values_hint,
        );
        Self::new(
            "life_model_summary",
            "1.0.0",
            PromptPurpose::LifeModel,
            content,
        )
        .with_privacy(PromptPrivacyLevel::Internal)
    }

    pub fn summary_only_behavioral_guidelines() -> Self {
        Self::new(
            "summary_only_guidelines",
            "1.0.0",
            PromptPurpose::BaseSystem,
            "在每次回应时：\n\
             1. 基于用户的核心价值观方向给出建议\n\
             2. 结合用户当前的状态和大致目标方向\n\
             3. 语气要温和、支持但不透露具体个人信息\n\
             4. 如用户要求具体信息，请说明当前处于隐私保护模式，建议切换到本地模型",
        )
        .with_privacy(PromptPrivacyLevel::Internal)
    }

    pub fn available_tools(tools_text: impl Into<String>) -> Self {
        Self::new(
            "available_tools",
            "1.0.0",
            PromptPurpose::Tool,
            tools_text.into(),
        )
        .with_cloud_allowed(true)
    }

    /// Estimate token count using a simple heuristic (≈ chars / 4).
    pub fn estimated_tokens(&self) -> usize {
        self.content.chars().count() / 4
    }

    /// Whether this block is safe for cloud model calls.
    pub fn is_cloud_safe(&self) -> bool {
        self.cloud_allowed && !matches!(self.privacy_level, PromptPrivacyLevel::StrictlyLocal)
    }
}

/// Planning prompt content: planner role, allowed/disallowed tools, output contract.
const PLANNING_PROMPT_CONTENT: &str = "\
You are in PlanMode. Your role is to explore, analyze, and produce a structured AgentPlan.

You MAY:
- Use read-only tools (file.read, web.search, web.fetch, goal.read, life_model.read, \
state.read, memory.search, agent_run.lookup, tool.list_available, permission.check, \
calendar.read) to gather context.
- Inspect allowed context including LifeModel, goals, memory, and tool manifests.
- Generate proposals for LifeModel, memory, or tool permission changes.

You MUST NOT:
- Write files, mutate LifeModel or Memory directly.
- Call bash, shell, or execute external side effects.
- Bypass Proposal, Permission, or Audit protocols.

For complex, risky, or multi-step tasks, produce a structured AgentPlan as your output. \
The plan must follow the AgentPlan output schema exactly. Include:
- goal: what you aim to accomplish
- assumptions: what you assume to be true
- missing_context: what information you need but do not yet have
- steps: ordered action steps with tool intents and dependencies
- tool_intents: which tools each step intends to use, with risk and write classification
- permission_requirements: any permissions needed
- rollback_plan: how to undo if needed
- success_criteria: how to know the task is done
- risk_level: low, medium, high, or critical

If the task is simple, read-only, and low risk, you may answer directly without a plan.";

/// JSON Schema for AgentPlan output.
/// Used as the output_schema on a PromptStack in planning mode.
const AGENT_PLAN_OUTPUT_SCHEMA: &str = r#"{
  "type": "object",
  "description": "A structured plan for complex or risky agent tasks",
  "properties": {
    "plan": {
      "type": "object",
      "required": ["goal", "steps", "risk_level"],
      "properties": {
        "goal": {
          "type": "string",
          "description": "The overall goal of the plan"
        },
        "assumptions": {
          "type": "array",
          "description": "Facts or conditions assumed to be true",
          "items": {"type": "string"}
        },
        "missing_context": {
          "type": "array",
          "description": "Information needed but not yet available",
          "items": {"type": "string"}
        },
        "steps": {
          "type": "array",
          "description": "Ordered steps to execute the plan",
          "items": {
            "type": "object",
            "required": ["index", "description"],
            "properties": {
              "index": {"type": "integer", "description": "Zero-based step index"},
              "description": {"type": "string", "description": "What this step does"},
              "tool_intent": {
                "type": "string",
                "description": "Tool intended for this step, if any"
              },
              "expected_output": {
                "type": "string",
                "description": "What this step should produce"
              },
              "depends_on": {
                "type": "array",
                "description": "Indices of steps this depends on",
                "items": {"type": "integer"}
              }
            }
          }
        },
        "tool_intents": {
          "type": "array",
          "description": "All tools intended for use across the plan",
          "items": {
            "type": "object",
            "required": ["tool_name", "purpose", "risk_level", "is_write"],
            "properties": {
              "tool_name": {"type": "string", "description": "Name of the tool"},
              "purpose": {"type": "string", "description": "Why this tool is needed"},
              "risk_level": {
                "type": "string",
                "enum": ["low", "medium", "high", "critical"],
                "description": "Risk classification"
              },
              "is_write": {
                "type": "boolean",
                "description": "Whether this tool performs a write operation"
              },
              "parameters_summary": {
                "type": "string",
                "description": "Brief summary of parameters"
              }
            }
          }
        },
        "subagent_assignments": {
          "type": "array",
          "description": "Sub-agent delegations needed",
          "items": {
            "type": "object",
            "required": ["agent_role", "task", "delegation_mode"],
            "properties": {
              "agent_role": {"type": "string"},
              "task": {"type": "string"},
              "delegation_mode": {
                "type": "string",
                "enum": ["call_as_tool", "handoff", "review"]
              }
            }
          }
        },
        "permission_requirements": {
          "type": "array",
          "description": "Permissions the plan requires",
          "items": {
            "type": "object",
            "required": ["target", "reason", "risk_level"],
            "properties": {
              "target": {"type": "string", "description": "What requires permission"},
              "reason": {"type": "string", "description": "Why permission is needed"},
              "risk_level": {
                "type": "string",
                "enum": ["low", "medium", "high", "critical"]
              }
            }
          }
        },
        "rollback_plan": {
          "type": "string",
          "description": "How to undo the plan if needed"
        },
        "success_criteria": {
          "type": "array",
          "description": "Metrics or conditions that indicate success",
          "items": {"type": "string"}
        },
        "risk_level": {
          "type": "string",
          "enum": ["low", "medium", "high", "critical"],
          "description": "Overall risk level of the plan"
        }
      }
    }
  }
}"#;

/// Return the AgentPlan output JSON Schema as a serde_json::Value.
pub fn agent_plan_output_schema() -> serde_json::Value {
    serde_json::from_str(AGENT_PLAN_OUTPUT_SCHEMA).unwrap_or_else(|_| serde_json::json!({}))
}

/// A stack of prompt blocks assembled for a single agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptStack {
    pub blocks: Vec<PromptBlock>,
    pub output_schema: Option<serde_json::Value>,
    pub assembled_preview: String,
    pub redaction_summary: Option<String>,
}

impl PromptStack {
    /// Create an empty prompt stack.
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            output_schema: None,
            assembled_preview: String::new(),
            redaction_summary: None,
        }
    }

    /// Build a PromptStack from an AgentSpec's prompt block references
    /// using the provided registry.  Unknown block IDs return an error.
    pub fn try_from_agentspec(
        block_ids: &[String],
        registry: &PromptBlockRegistry,
    ) -> Result<Self, String> {
        let mut stack = Self::new();
        for id in block_ids {
            let block = registry
                .get(id)
                .ok_or_else(|| format!("unknown prompt block: {}", id))?;
            stack.push(block.clone());
        }
        Ok(stack)
    }

    #[deprecated(
        since = "0.1.0",
        note = "use try_from_agentspec with PromptBlockRegistry"
    )]
    #[allow(dead_code)]
    pub(crate) fn from_agentspec(_block_ids: &[String]) -> Self {
        Self::new()
    }
    /// Build a PromptStack for PlanMode with the planning prompt block
    /// and the AgentPlan output schema.
    pub fn plan_mode_stack() -> Self {
        Self::new()
            .with_block(PromptBlock::planning())
            .with_output_schema(agent_plan_output_schema())
    }

    /// Full chat system prompt: base identity + behavioral guidelines +
    /// LifeModel YAML + state hint + evolution rules + tool call format + available tools.
    pub fn chat_system_stack(
        life_model: &crate::life_model::LifeModel,
        tools_block: Option<PromptBlock>,
    ) -> Self {
        let mut stack = Self::new()
            .with_block(PromptBlock::base_identity())
            .with_block(PromptBlock::behavioral_guidelines())
            .with_block(PromptBlock::life_model_yaml(life_model))
            .with_block(PromptBlock::state_hint(life_model))
            .with_block(PromptBlock::evolution_hint(life_model));
        if let Some(tb) = tools_block {
            if !tb.content.trim().is_empty() {
                stack.push(tb);
                stack.push(PromptBlock::tool_call_format());
            }
        }
        stack
    }

    /// SummaryOnly variant: replaces full LifeModel with a summary-only block
    /// and uses privacy-conscious behavioral guidelines.
    pub fn chat_system_stack_summary_only(
        life_model: &crate::life_model::LifeModel,
        tools_block: Option<PromptBlock>,
    ) -> Self {
        let mut stack = Self::new()
            .with_block(PromptBlock::base_identity())
            .with_block(PromptBlock::summary_only_behavioral_guidelines())
            .with_block(PromptBlock::new(
                "privacy_notice",
                "1.0.0",
                PromptPurpose::Privacy,
                "[SummaryOnly] 云端隐私保护模式下，仅发送以下摘要信息：",
            ))
            .with_block(PromptBlock::life_model_summary(life_model));
        if let Some(tb) = tools_block {
            stack.push(PromptBlock::new(
                "tool_info_header",
                "1.0.0",
                PromptPurpose::Tool,
                "【工具信息】",
            ));
            stack.push(tb);
        }
        stack
    }

    /// Push a block onto the stack.
    pub fn push(&mut self, block: PromptBlock) {
        self.blocks.push(block);
    }

    /// Builder-style push.
    pub fn with_block(mut self, block: PromptBlock) -> Self {
        self.blocks.push(block);
        self
    }

    /// Set an output schema for the stack.
    pub fn with_output_schema(mut self, schema: serde_json::Value) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// Assemble all blocks into a single prompt string, separated by double newlines.
    pub fn assemble(&mut self) -> String {
        let parts: Vec<&str> = self.blocks.iter().map(|b| b.content.as_str()).collect();
        let assembled = parts.join("\n\n");
        self.assembled_preview = assembled.chars().take(500).collect();
        assembled
    }

    /// Read-only render — does not update assembled_preview.
    pub fn render(&self) -> String {
        let parts: Vec<&str> = self.blocks.iter().map(|b| b.content.as_str()).collect();
        parts.join("\n\n")
    }

    /// Return only cloud-allowed blocks, assembled.
    pub fn cloud_filtered(&self) -> PromptStack {
        let blocks: Vec<PromptBlock> = self
            .blocks
            .iter()
            .filter(|b| b.is_cloud_safe())
            .cloned()
            .collect();
        let mut stack = PromptStack {
            blocks,
            output_schema: self.output_schema.clone(),
            assembled_preview: String::new(),
            redaction_summary: None,
        };
        let removed_count = self.blocks.len() - stack.blocks.len();
        if removed_count > 0 {
            stack.redaction_summary = Some(format!(
                "{} block(s) removed for cloud safety",
                removed_count
            ));
        }
        stack
    }

    /// Total estimated token count across all blocks.
    pub fn estimated_tokens(&self) -> usize {
        self.blocks.iter().map(|b| b.estimated_tokens()).sum()
    }

    /// Produce a trace of block IDs and versions (no content).
    pub fn block_trace(&self) -> Vec<BlockTraceEntry> {
        self.blocks
            .iter()
            .map(|b| BlockTraceEntry {
                id: b.id.clone(),
                version: b.version.clone(),
                purpose: b.purpose.as_str().to_string(),
                cloud_allowed: b.cloud_allowed,
                estimated_tokens: b.estimated_tokens(),
            })
            .collect()
    }
}

impl Default for PromptStack {
    fn default() -> Self {
        Self::new()
    }
}

/// A trace entry for a single block in the stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTraceEntry {
    pub id: String,
    pub version: String,
    pub purpose: String,
    pub cloud_allowed: bool,
    pub estimated_tokens: usize,
}

impl PromptStack {
    /// Validate the stack: check for required blocks, empty content, etc.
    pub fn validate(&self) -> Result<Vec<String>> {
        let mut warnings = Vec::new();
        for block in &self.blocks {
            if block.content.trim().is_empty() {
                warnings.push(format!(
                    "Block '{}' (v{}) has empty content",
                    block.id, block.version
                ));
            }
            if block.estimated_tokens() > block.token_budget && block.token_budget > 0 {
                warnings.push(format!(
                    "Block '{}' exceeds token budget ({} > {})",
                    block.id,
                    block.estimated_tokens(),
                    block.token_budget
                ));
            }
        }
        Ok(warnings)
    }
}

// ── P6 Goal 2: PromptBlock Registry ──────────────────────────────────

/// Minimal runtime registry for prompt blocks, keyed by stable block id.
/// Populated with known built-in blocks at startup.
pub struct PromptBlockRegistry {
    blocks: std::collections::HashMap<String, PromptBlock>,
}

impl PromptBlockRegistry {
    pub fn new() -> Self {
        Self {
            blocks: std::collections::HashMap::new(),
        }
    }

    pub fn with_block(mut self, id: impl Into<String>, block: PromptBlock) -> Self {
        self.blocks.insert(id.into(), block);
        self
    }

    pub fn get(&self, id: &str) -> Option<&PromptBlock> {
        self.blocks.get(id)
    }

    /// Build a registry with all built-in blocks.
    pub fn built_in() -> Self {
        Self::new()
            .with_block("planning", PromptBlock::planning())
            .with_block("base_system", PromptBlock::base_identity())
            .with_block("base_identity", PromptBlock::base_identity())
            .with_block("behavioral_guidelines", PromptBlock::behavioral_guidelines())
            .with_block("tool_call_format", PromptBlock::tool_call_format())
            .with_block(
                "tool_discipline",
                PromptBlock::new(
                    "tool_discipline",
                    "1.0.0",
                    PromptPurpose::Tool,
                    "Tools are governed by ToolRuntime, Permission, and Proposal.",
                ),
            )
            .with_block(
                "privacy_rule",
                PromptBlock::new(
                    "privacy_rule",
                    "1.0.0",
                    PromptPurpose::Privacy,
                    "Do not expose sensitive user data to cloud models.",
                )
                .with_privacy(PromptPrivacyLevel::Internal),
            )
    }
}

impl Default for PromptBlockRegistry {
    fn default() -> Self {
        Self::built_in()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_stack() -> PromptStack {
        let base = PromptBlock::new(
            "base_system",
            "1.0.0",
            PromptPurpose::BaseSystem,
            "You are OpenLife, a LifeModel-governed personal agent framework.",
        )
        .with_privacy(PromptPrivacyLevel::Internal);

        let life_model = PromptBlock::new(
            "life_model",
            "1.0.0",
            PromptPurpose::LifeModel,
            "The user's LifeModel contains identity, goals, capabilities, and state.",
        )
        .with_privacy(PromptPrivacyLevel::Sensitive);

        let tools = PromptBlock::new(
            "tool_prompt",
            "1.0.0",
            PromptPurpose::Tool,
            "You may use tools: web.search, file.read, memory.search.",
        )
        .with_cloud_allowed(true);

        let private_block = PromptBlock::new(
            "local_only",
            "1.0.0",
            PromptPurpose::Custom("private_context".into()),
            "This contains strictly local data.",
        )
        .with_privacy(PromptPrivacyLevel::StrictlyLocal);

        PromptStack::new()
            .with_block(base)
            .with_block(life_model)
            .with_block(tools)
            .with_block(private_block)
    }

    #[test]
    fn test_stack_assembly() {
        let mut stack = make_test_stack();
        let assembled = stack.assemble();
        assert!(assembled.contains("OpenLife"));
        assert!(assembled.contains("LifeModel"));
        assert!(assembled.contains("web.search"));
        assert!(!stack.assembled_preview.is_empty());
    }

    #[test]
    fn test_cloud_filtering() {
        let stack = make_test_stack();
        let cloud = stack.cloud_filtered();
        // StrictlyLocal block should be removed
        assert_eq!(cloud.blocks.len(), 3);
        // Sensitive block should remain (cloud_allowed=true unless StrictlyLocal)
        let has_life_model = cloud.blocks.iter().any(|b| b.id == "life_model");
        // The life_model block is Sensitive but cloud_allowed is true (default)
        assert!(has_life_model);
        // StrictlyLocal block should be gone
        let has_local = cloud.blocks.iter().any(|b| b.id == "local_only");
        assert!(!has_local);
        // Redaction summary should be present
        assert!(cloud.redaction_summary.is_some());
    }

    #[test]
    fn test_block_trace() {
        let stack = make_test_stack();
        let trace = stack.block_trace();
        assert_eq!(trace.len(), 4);
        assert_eq!(trace[0].id, "base_system");
        assert_eq!(trace[0].version, "1.0.0");
        assert_eq!(trace[3].id, "local_only");
        assert!(!trace[3].cloud_allowed);
    }

    #[test]
    fn test_estimated_tokens() {
        let mut stack = PromptStack::new();
        stack.push(PromptBlock::new(
            "test",
            "1.0",
            PromptPurpose::Custom("test".into()),
            "a".repeat(400),
        ));
        let tokens = stack.estimated_tokens();
        assert_eq!(tokens, 100); // 400 chars / 4
    }

    #[test]
    fn test_validation_warns_empty_content() {
        let mut stack = PromptStack::new();
        stack.push(PromptBlock::new(
            "empty_block",
            "1.0",
            PromptPurpose::Custom("test".into()),
            "",
        ));
        let warnings = stack.validate().unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("empty content"));
    }

    #[test]
    fn test_validation_warns_budget_exceeded() {
        let mut stack = PromptStack::new();
        stack.push(
            PromptBlock::new(
                "big_block",
                "1.0",
                PromptPurpose::Custom("test".into()),
                "a".repeat(1000),
            )
            .with_token_budget(200), // 1000 chars = ~250 tokens > 200 budget
        );
        let warnings = stack.validate().unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("exceeds token budget"));
    }

    #[test]
    fn test_empty_stack_assembles_empty() {
        let mut stack = PromptStack::new();
        let assembled = stack.assemble();
        assert!(assembled.is_empty());
    }

    #[test]
    fn test_output_schema_persists_after_cloud_filter() {
        let schema =
            serde_json::json!({"type": "object", "properties": {"final": {"type": "string"}}});
        let stack = make_test_stack().with_output_schema(schema.clone());
        let cloud = stack.cloud_filtered();
        assert!(cloud.output_schema.is_some());
        assert_eq!(cloud.output_schema.unwrap(), schema);
    }

    #[test]
    fn test_block_metadata_preserved() {
        let block = PromptBlock::new(
            "role_prompt",
            "2.1.0",
            PromptPurpose::BaseSystem,
            "System role content",
        )
        .with_privacy(PromptPrivacyLevel::Sensitive)
        .with_token_budget(500)
        .with_applies_to(vec!["Generalist".into(), "Planner".into()]);

        assert_eq!(block.id, "role_prompt");
        assert_eq!(block.version, "2.1.0");
        assert_eq!(block.privacy_level, PromptPrivacyLevel::Sensitive);
        assert_eq!(block.token_budget, 500);
        assert_eq!(block.applies_to, vec!["Generalist", "Planner"]);
        assert!(block.cloud_allowed); // Sensitive but not StrictlyLocal
        assert!(block.is_cloud_safe());
    }

    // ── PlanMode/Planning prompt tests ───────────────────────────────────

    #[test]
    fn test_planning_prompt_block_created() {
        let block = PromptBlock::planning();
        assert_eq!(block.id, "planning_prompt");
        assert_eq!(block.version, "1.0.0");
        assert_eq!(block.purpose, PromptPurpose::Planning);
        assert_eq!(block.privacy_level, PromptPrivacyLevel::Internal);
        assert!(block.cloud_allowed);
        assert!(block.is_cloud_safe());
        assert_eq!(block.token_budget, 800);
        assert!(block.applies_to.contains(&"Planner".to_string()));
        assert!(block.applies_to.contains(&"PlanMode".to_string()));
        assert!(!block.content.is_empty());
        // Must mention PlanMode and Planner contract
        assert!(block.content.contains("PlanMode"));
        assert!(block.content.contains("read-only tools"));
        assert!(block.content.contains("MUST NOT"));
        assert!(block.content.contains("Write files") || block.content.contains("write files"));
        assert!(block.content.contains("AgentPlan"));
        assert!(block.content.contains("risk_level"));
    }

    #[test]
    fn test_plan_mode_stack_includes_block_and_schema() {
        let mut stack = PromptStack::plan_mode_stack();
        assert_eq!(stack.blocks.len(), 1);
        assert_eq!(stack.blocks[0].id, "planning_prompt");
        assert!(stack.output_schema.is_some());

        // Output schema should describe AgentPlan structure
        let schema = stack.output_schema.as_ref().unwrap();
        let plan_props = schema
            .get("properties")
            .and_then(|p| p.get("plan"))
            .and_then(|p| p.get("properties"));
        assert!(plan_props.is_some(), "schema must have plan.properties");

        let props = plan_props.unwrap();
        assert!(props.get("goal").is_some());
        assert!(props.get("steps").is_some());
        assert!(props.get("risk_level").is_some());
        assert!(props.get("assumptions").is_some());
        assert!(props.get("tool_intents").is_some());
        assert!(props.get("rollback_plan").is_some());
        assert!(props.get("success_criteria").is_some());

        // Required fields
        let required = schema
            .get("properties")
            .and_then(|p| p.get("plan"))
            .and_then(|p| p.get("required"));
        let required: Vec<&str> = required
            .and_then(|r| r.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        assert!(required.contains(&"goal"));
        assert!(required.contains(&"steps"));
        assert!(required.contains(&"risk_level"));

        // Assembly
        let assembled = stack.assemble();
        assert!(assembled.contains("PlanMode"));
        assert!(assembled.contains("AgentPlan"));
    }

    #[test]
    fn test_plan_mode_stack_cloud_filtering() {
        let stack = PromptStack::plan_mode_stack();
        let cloud = stack.cloud_filtered();

        // Planning prompt is cloud-safe (Internal privacy, cloud_allowed=true)
        assert_eq!(cloud.blocks.len(), 1);
        assert_eq!(cloud.blocks[0].id, "planning_prompt");

        // Output schema persists after cloud filtering
        assert!(cloud.output_schema.is_some());

        // No redaction needed
        assert!(cloud.redaction_summary.is_none());
    }

    #[test]
    fn test_plan_mode_stack_with_sensitive_block_cloud_filtering() {
        let mut stack = PromptStack::plan_mode_stack();
        let private_block = PromptBlock::new(
            "local_only",
            "1.0.0",
            PromptPurpose::Custom("sensitive_context".into()),
            "This contains strictly local LifeModel data.",
        )
        .with_privacy(PromptPrivacyLevel::StrictlyLocal);
        stack.push(private_block);

        assert_eq!(stack.blocks.len(), 2);

        let cloud = stack.cloud_filtered();
        // Only planning prompt survives — strictly-local block is removed
        assert_eq!(cloud.blocks.len(), 1);
        assert_eq!(cloud.blocks[0].id, "planning_prompt");
        assert!(cloud.output_schema.is_some());
        assert!(cloud.redaction_summary.is_some());
        assert!(cloud
            .redaction_summary
            .as_deref()
            .unwrap()
            .contains("1 block(s) removed"));
    }

    #[test]
    fn test_plan_mode_block_trace() {
        let mut stack = PromptStack::plan_mode_stack();
        stack.push(
            PromptBlock::new(
                "task_prompt",
                "1.0.0",
                PromptPurpose::Task,
                "Analyze project configuration.",
            )
            .with_cloud_allowed(true),
        );

        let trace = stack.block_trace();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].id, "planning_prompt");
        assert_eq!(trace[0].purpose, "planning");
        assert!(trace[0].cloud_allowed);
        assert!(trace[0].estimated_tokens > 0);
        assert_eq!(trace[1].id, "task_prompt");
        assert_eq!(trace[1].purpose, "task");
    }

    #[test]
    fn test_planning_prompt_content_conforms_to_adr0007() {
        let block = PromptBlock::planning();
        let content = &block.content;

        // ADR 0007 Planner permissions: MAY use read-only tools
        assert!(
            content.contains("read-only tools"),
            "planning prompt must mention read-only tools"
        );

        // ADR 0007: MUST NOT write/mutate
        assert!(
            content.to_lowercase().contains("write files")
                || content.to_lowercase().contains("mutate"),
            "planning prompt must forbid writes"
        );

        // ADR 0007: MUST NOT call bash/shell
        assert!(
            content.contains("bash") || content.contains("shell"),
            "planning prompt must forbid bash/shell"
        );

        // ADR 0007: MUST NOT bypass Proposal/Permission/Audit
        assert!(
            content.contains("Proposal")
                || content.contains("Permission")
                || content.contains("Audit"),
            "planning prompt must reference protocol checks"
        );

        // ADR 0007 AgentPlan required fields
        assert!(content.contains("goal"), "must mention goal");
        assert!(content.contains("steps"), "must mention steps");
        assert!(content.contains("risk_level"), "must mention risk_level");
    }

    #[test]
    fn test_agent_plan_output_schema_is_valid_json() {
        let schema = agent_plan_output_schema();
        assert!(schema.is_object());
        assert_eq!(schema.get("type").and_then(|t| t.as_str()), Some("object"));

        // Verify schema can be serialized
        let serialized = serde_json::to_string(&schema).unwrap();
        assert!(serialized.contains("goal"));
        assert!(serialized.contains("steps"));
        assert!(serialized.contains("risk_level"));
    }

    #[test]
    fn test_plan_mode_stack_assembly_not_empty() {
        let mut stack = PromptStack::plan_mode_stack();
        let assembled = stack.assemble();
        assert!(!assembled.is_empty());
        // preview is trimmed to 500 chars
        assert!(!stack.assembled_preview.is_empty());
        assert!(stack.assembled_preview.len() <= 500);
    }

    // ── Goal 2: PromptStack registry + try_from_agentspec ──────────────

    #[test]
    fn test_known_block_ids_assemble_non_empty_stack() {
        let registry = PromptBlockRegistry::built_in();
        let stack = PromptStack::try_from_agentspec(&["planning".to_string()], &registry).unwrap();
        assert!(!stack.blocks.is_empty());
    }

    #[test]
    fn test_unknown_block_id_returns_error() {
        let registry = PromptBlockRegistry::built_in();
        let result = PromptStack::try_from_agentspec(&["nonexistent".to_string()], &registry);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown prompt block"));
    }

    #[test]
    fn test_assembled_metadata_excludes_raw_content() {
        let registry = PromptBlockRegistry::built_in();
        let mut stack =
            PromptStack::try_from_agentspec(&["planning".to_string()], &registry).unwrap();
        let _ = stack.assemble(); // populate metadata
        let meta = format!(
            "{:?}",
            stack.blocks.iter().map(|b| &b.id).collect::<Vec<_>>()
        );
        assert!(!meta.contains("You are a planner")); // raw prompt text absent
    }

    // ── P6-5: AgentSpec prompt binding test ────────────────────────────

    #[test]
    #[allow(deprecated)]
    fn test_legacy_from_agentspec_returns_empty() {
        // Deprecated stub — kept for backward-compat, still returns empty.
        let stack = PromptStack::from_agentspec(&["anything".to_string()]);
        assert!(stack.blocks.is_empty());
    }

    #[test]
    fn test_unknown_block_fails_with_structured_error() {
        let registry = PromptBlockRegistry::built_in();
        let err = PromptStack::try_from_agentspec(&["nonexistent_block".to_string()], &registry)
            .unwrap_err();
        assert!(err.contains("unknown prompt block"));
        assert!(err.contains("nonexistent_block"));
    }

    // ── Post-Beta: PromptBlock factory tests ──────────────────────────

    fn make_test_life_model() -> crate::life_model::LifeModel {
        let mut lm = crate::life_model::LifeModel::default();
        lm.state.current_focus = "学习 Rust".to_string();
        lm.state.emotional_state.current_mood = "充实".to_string();
        lm.state.emotional_state.stress_level = 3;
        lm.state.emotional_state.fulfillment_score = 8;
        lm.state.health_status.physical = "良好".to_string();
        lm.state.health_status.mental = "专注".to_string();
        lm.state.health_status.energy_level = 7;
        lm.state.focus_areas = vec!["编程".to_string(), "健康".to_string()];
        lm.state.recent_events = vec!["完成了 P12 验收".to_string()];
        lm.identity.values = vec![crate::life_model::ValueItem {
            name: "持续学习".to_string(),
            weight: 9,
            description: "终身成长".to_string(),
        }];
        lm.goals.short_term = vec![crate::life_model::GoalItem::default()];
        lm.evolution_rules = vec!["每周复盘".to_string()];
        lm
    }

    #[test]
    fn test_state_hint_empty_lifemodel() {
        let lm = crate::life_model::LifeModel::default();
        let block = PromptBlock::state_hint(&lm);
        assert!(block.content.contains("用户当前状态摘要"));
        assert!(block.content.contains("暂无状态记录"));
        assert_eq!(block.purpose, PromptPurpose::LifeModel);
    }

    #[test]
    fn test_state_hint_populated() {
        let lm = make_test_life_model();
        let block = PromptBlock::state_hint(&lm);
        assert!(block.content.contains("当前重心: 学习 Rust"));
        assert!(block.content.contains("当前心情: 充实 (压力3/10, 满足度8/10)"));
        assert!(block.content.contains("身心健康: 良好/专注 (精力7/10)"));
        assert!(block.content.contains("关注领域: 编程, 健康"));
        assert!(block.content.contains("近期事件: 完成了 P12 验收"));
    }

    #[test]
    fn test_life_model_summary_content() {
        let lm = make_test_life_model();
        let block = PromptBlock::life_model_summary(&lm);
        assert!(block.content.contains("用户状态摘要"));
        assert!(block.content.contains("目标摘要"));
        assert!(block.content.contains("价值观方向"));
        assert!(block.content.contains("持续学习"));
        assert!(block.content.contains("短期目标 1 个"));
    }

    #[test]
    fn test_life_model_yaml_output() {
        let lm = make_test_life_model();
        let block = PromptBlock::life_model_yaml(&lm);
        assert!(block.content.contains("```yaml"));
        assert!(block.content.contains("学习 Rust"));
        assert_eq!(block.privacy_level, PromptPrivacyLevel::Sensitive);
    }

    #[test]
    fn test_available_tools_passthrough() {
        let block = PromptBlock::available_tools("- web.search: search the web");
        assert!(block.content.contains("web.search"));
        // Must NOT add duplicate prefix
        assert!(!block.content.starts_with("\n你可以使用以下工具:\n"));
        assert!(block.cloud_allowed);
    }

    #[test]
    fn test_available_tools_empty() {
        let block = PromptBlock::available_tools("");
        assert!(block.content.is_empty());
    }

    #[test]
    fn test_base_identity_contains_chinese() {
        let block = PromptBlock::base_identity();
        assert!(block.content.contains("OpenLife"));
        assert!(block.content.contains("终身成长合伙人"));
        assert!(block.content.contains("人生模型"));
    }

    #[test]
    fn test_behavioral_guidelines_content() {
        let block = PromptBlock::behavioral_guidelines();
        assert!(block.content.contains("核心价值观"));
        assert!(block.content.contains("人格特质"));
    }

    #[test]
    fn test_tool_call_format_block() {
        let block = PromptBlock::tool_call_format();
        assert!(block.content.contains("tool_calls"));
        assert!(block.content.contains("JSON"));
        assert!(block.cloud_allowed);
    }

    #[test]
    fn test_chat_system_stack_assembles_correct_order() {
        let lm = make_test_life_model();
        let tools = PromptBlock::available_tools("- web.search: search");
        let mut stack = PromptStack::chat_system_stack(&lm, Some(tools));
        let assembled = stack.assemble();
        // base_identity comes before behavioral_guidelines
        let base_pos = assembled.find("终身成长合伙人").unwrap();
        let guide_pos = assembled.find("核心价值观").unwrap();
        assert!(base_pos < guide_pos, "base_identity should appear before behavioral_guidelines");
        // tools_block comes before tool_call_format
        let tools_pos = assembled.find("web.search").unwrap();
        let format_pos = assembled.find("tool_calls").unwrap();
        assert!(tools_pos < format_pos, "tools_block should appear before tool_call_format");
    }

    #[test]
    fn test_chat_system_stack_no_tools() {
        let lm = make_test_life_model();
        let mut stack = PromptStack::chat_system_stack(&lm, None);
        let assembled = stack.assemble();
        assert!(!assembled.contains("tool_calls"));
    }

    #[test]
    fn test_chat_system_stack_summary_only_content() {
        let lm = make_test_life_model();
        let tools = PromptBlock::available_tools("- test: a test tool");
        let mut stack = PromptStack::chat_system_stack_summary_only(&lm, Some(tools));
        let assembled = stack.assemble();
        assert!(assembled.contains("SummaryOnly"));
        assert!(assembled.contains("云端隐私保护模式"));
        assert!(assembled.contains("用户状态摘要"));
        assert!(assembled.contains("目标摘要"));
        assert!(assembled.contains("test: a test tool"));
    }

    #[test]
    fn test_render_vs_assemble() {
        let lm = make_test_life_model();
        let mut stack = PromptStack::chat_system_stack(&lm, None);
        let rendered = stack.render();
        let assembled = stack.assemble();
        assert_eq!(rendered, assembled, "render and assemble should produce identical output");
    }

    #[test]
    fn test_built_in_registry_has_backward_compat_keys() {
        let registry = PromptBlockRegistry::built_in();
        assert!(registry.get("base_system").is_some(), "backward-compat key base_system");
        assert!(registry.get("base_identity").is_some(), "new key base_identity");
        assert!(registry.get("tool_discipline").is_some(), "backward-compat key tool_discipline");
        assert!(registry.get("privacy_rule").is_some(), "privacy_rule key");
    }

    #[test]
    fn test_evolution_hint_output() {
        let mut lm = make_test_life_model();
        lm.evolution_rules = vec!["规则1".to_string(), "规则2".to_string()];
        let block = PromptBlock::evolution_hint(&lm);
        assert!(block.content.contains("自动进化规则"));
        assert!(block.content.contains("规则1"));
        assert!(block.content.contains("规则2"));
    }

    #[test]
    fn test_summary_only_guidelines() {
        let block = PromptBlock::summary_only_behavioral_guidelines();
        assert!(block.content.contains("核心价值观方向"));
        assert!(block.content.contains("隐私保护模式"));
        assert!(block.content.contains("切换"));
    }
}
