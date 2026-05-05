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

    /// Estimate token count using a simple heuristic (≈ chars / 4).
    pub fn estimated_tokens(&self) -> usize {
        self.content.chars().count() / 4
    }

    /// Whether this block is safe for cloud model calls.
    pub fn is_cloud_safe(&self) -> bool {
        self.cloud_allowed && !matches!(self.privacy_level, PromptPrivacyLevel::StrictlyLocal)
    }
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
}
