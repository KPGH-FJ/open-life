use std::sync::Arc;

use crate::agent::compaction::CompactionConfig;

/// Configuration for the agent execution loop.
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub timeout_seconds: u64,
    /// If `true`, write-proposal tools (e.g. file.write_proposal, memory.propose_write)
    /// may appear in the tool prompt and be executable. If `false`, write tools are
    /// excluded from the tool prompt AND blocked at execution level (hard enforcement).
    pub allow_writes: bool,
    /// If `true`, model calls may be routed to cloud providers. If `false`, routing
    /// is restricted to local-only (Ollama). Enforcement: AgentLoop forces
    /// PrivacyPolicy::LocalOnly when allow_cloud is false, causing fail-closed
    /// behavior when no local model is available.
    pub allow_cloud: bool,
    pub shutdown_notify: Option<Arc<tokio::sync::Notify>>,
    /// Specialized role for tool selection and system prompt tuning
    pub role: AgentRole,
    /// Optional restrict to specific tools (empty = all allowed)
    pub toolset_allowlist: Vec<String>,
    /// Compaction configuration for long-context continuity.
    /// None disables compaction. Default: Some(CompactionConfig::default()).
    pub compaction_config: Option<CompactionConfig>,
}

/// Specialization role for the agent loop.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AgentRole {
    /// Full tool access, standard conversation
    #[default]
    Generalist,
    /// Goal decomposition and weekly review focus
    Planner,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_steps: 4,
            max_tool_calls: 6,
            timeout_seconds: 90,
            allow_writes: true,
            allow_cloud: true,
            shutdown_notify: None,
            role: AgentRole::default(),
            toolset_allowlist: Vec::new(),
            compaction_config: Some(CompactionConfig::default()),
        }
    }
}

impl AgentLoopConfig {
    /// Get planner-specific system instruction appended to tools prompt.
    pub fn role_system_instruction(&self) -> Option<&'static str> {
        match self.role {
            AgentRole::Generalist => None,
            AgentRole::Planner => Some(
                "You are in Planner mode. Focus on:\n\
                 - Decomposing goals into actionable steps\n\
                 - Identifying blockers and dependencies\n\
                 - Proposing schedule adjustments\n\
                 - Reading current goals before suggesting changes\n\
                 Use goal.read, life_model.read, and proposal.create tools.",
            ),
        }
    }

    /// vNext: get the role instruction as a versioned PromptBlock.
    pub fn role_prompt_block(&self) -> Option<crate::agent::prompt_stack::PromptBlock> {
        use crate::agent::prompt_stack::{PromptBlock, PromptPrivacyLevel, PromptPurpose};
        self.role_system_instruction().map(|content| {
            PromptBlock::new(
                format!("role.{}", format!("{:?}", self.role).to_lowercase()),
                "1.0.0",
                PromptPurpose::Planning,
                content,
            )
            .with_privacy(PromptPrivacyLevel::Internal)
            .with_applies_to(vec![format!("{:?}", self.role)])
        })
    }
}
