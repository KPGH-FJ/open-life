use crate::agent::types::ContextSummary;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::privacy::PrivacyEngine;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// Assembles context for an AgentRun by combining LifeModel, Memory, Privacy, and Tools.
pub trait ContextAssembler: Send + Sync {
    fn name(&self) -> &'static str;
    fn assemble(&self, input: &AssembleInput) -> Result<AssembleOutput>;
}

/// A single memory hit from vector or text search.
#[derive(Debug, Clone)]
pub struct MemoryHit {
    pub id: i64,
    pub content: String,
    pub source: String,
    pub score: f32,
    pub tier: i64,
}

/// Input to context assembly.
/// Uses `Arc` for large structures to avoid deep clones during assembly chain.
pub struct AssembleInput {
    pub session_id: String,
    pub messages: Arc<Vec<ChatMessage>>,
    pub life_model: Arc<LifeModel>,
    pub tools_prompt: String,
    pub privacy_engine: PrivacyEngine,
    // Memory prefetch data (async-retrieved, passed in for sync assembly)
    pub memory_context: Option<String>,
    pub memory_hits: Vec<MemoryHit>,
    pub memory_retrieval_time_ms: u64,
}

/// Output from context assembly.
pub struct AssembleOutput {
    pub life_model: Arc<LifeModel>,
    pub tools_prompt: String,
    pub privacy_map: HashMap<String, String>,
    pub desensitized_messages: Arc<Vec<ChatMessage>>,
    pub memory_context: String,
    pub context_summary: ContextSummary,
    pub embed_error: Option<String>,
}

/// LifeModel assembler: loads and refreshes hot cache.
pub struct LifeModelAssembler;

impl ContextAssembler for LifeModelAssembler {
    fn name(&self) -> &'static str {
        "life_model"
    }

    fn assemble(&self, input: &AssembleInput) -> Result<AssembleOutput> {
        let included_sections = vec![
            "identity".to_string(),
            "goals".to_string(),
            "capabilities".to_string(),
            "state".to_string(),
        ];

        Ok(AssembleOutput {
            life_model: input.life_model.clone(),
            tools_prompt: input.tools_prompt.clone(),
            privacy_map: HashMap::new(),
            desensitized_messages: input.messages.clone(),
            memory_context: String::new(),
            context_summary: ContextSummary {
                life_model_empty: input.life_model.is_effectively_empty(),
                included_life_model_sections: included_sections,
                memory_hit_count: 0,
                memory_sources: vec![],
                used_tools_prompt: !input.tools_prompt.is_empty(),
                redaction_applied: false,
                redaction_level: crate::agent::types::RedactionLevel::None,
            },
            embed_error: None,
        })
    }
}

/// Memory assembler: injects prefetched memory context into the assembled output.
pub struct MemoryAssembler;

impl ContextAssembler for MemoryAssembler {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn assemble(&self, input: &AssembleInput) -> Result<AssembleOutput> {
        let hit_count = input.memory_hits.len();
        let memory_context = input.memory_context.clone().unwrap_or_default();

        // Build memory section for system prompt injection
        let memory_section = if hit_count > 0 {
            format!(
                "【相关记忆】\n{}\n\n[检索到 {} 条记忆，耗时 {}ms]",
                memory_context, hit_count, input.memory_retrieval_time_ms
            )
        } else {
            String::new()
        };

        // Extract memory sources for context summary
        let memory_sources: Vec<String> = input
            .memory_hits
            .iter()
            .map(|hit| hit.source.clone())
            .collect();

        Ok(AssembleOutput {
            life_model: input.life_model.clone(),
            tools_prompt: input.tools_prompt.clone(),
            privacy_map: HashMap::new(),
            desensitized_messages: input.messages.clone(),
            memory_context: memory_section,
            context_summary: ContextSummary {
                life_model_empty: input.life_model.is_effectively_empty(),
                included_life_model_sections: vec![],
                memory_hit_count: hit_count as i64,
                memory_sources,
                used_tools_prompt: !input.tools_prompt.is_empty(),
                redaction_applied: false,
                redaction_level: crate::agent::types::RedactionLevel::None,
            },
            embed_error: None,
        })
    }
}

/// Privacy assembler: desensitizes user messages.
pub struct PrivacyAssembler;

impl ContextAssembler for PrivacyAssembler {
    fn name(&self) -> &'static str {
        "privacy"
    }

    fn assemble(&self, input: &AssembleInput) -> Result<AssembleOutput> {
        let mut desensitized = Vec::new();
        let mut privacy_map = HashMap::new();

        for msg in input.messages.iter() {
            if msg.role == "user" {
                let (masked, map) = input.privacy_engine.desensitize(&msg.content);
                privacy_map.extend(map);
                desensitized.push(ChatMessage {
                    role: msg.role.clone(),
                    content: masked,
                });
            } else {
                desensitized.push(msg.clone());
            }
        }

        let redaction_level = if privacy_map.is_empty() {
            crate::agent::types::RedactionLevel::None
        } else {
            crate::agent::types::RedactionLevel::Light
        };

        Ok(AssembleOutput {
            life_model: input.life_model.clone(),
            tools_prompt: input.tools_prompt.clone(),
            privacy_map: privacy_map.clone(),
            desensitized_messages: Arc::new(desensitized),
            memory_context: String::new(),
            context_summary: ContextSummary {
                life_model_empty: input.life_model.is_effectively_empty(),
                included_life_model_sections: vec![],
                memory_hit_count: 0,
                memory_sources: vec![],
                used_tools_prompt: !input.tools_prompt.is_empty(),
                redaction_applied: !privacy_map.is_empty(),
                redaction_level,
            },
            embed_error: None,
        })
    }
}

/// Tools assembler: prepares tool prompts.
pub struct ToolsAssembler;

impl ContextAssembler for ToolsAssembler {
    fn name(&self) -> &'static str {
        "tools"
    }

    fn assemble(&self, input: &AssembleInput) -> Result<AssembleOutput> {
        Ok(AssembleOutput {
            life_model: input.life_model.clone(),
            tools_prompt: input.tools_prompt.clone(),
            privacy_map: HashMap::new(),
            desensitized_messages: input.messages.clone(),
            memory_context: String::new(),
            context_summary: ContextSummary {
                life_model_empty: input.life_model.is_effectively_empty(),
                included_life_model_sections: vec![],
                memory_hit_count: 0,
                memory_sources: vec![],
                used_tools_prompt: !input.tools_prompt.is_empty(),
                redaction_applied: false,
                redaction_level: crate::agent::types::RedactionLevel::None,
            },
            embed_error: None,
        })
    }
}

/// Composite assembler that chains multiple assemblers together.
pub struct CompositeAssembler {
    assemblers: Vec<Box<dyn ContextAssembler>>,
}

impl Default for CompositeAssembler {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositeAssembler {
    pub fn new() -> Self {
        Self {
            assemblers: Vec::new(),
        }
    }

    pub fn with(mut self, assembler: Box<dyn ContextAssembler>) -> Self {
        self.assemblers.push(assembler);
        self
    }
}

impl ContextAssembler for CompositeAssembler {
    fn name(&self) -> &'static str {
        "composite"
    }

    fn assemble(&self, input: &AssembleInput) -> Result<AssembleOutput> {
        let mut output = AssembleOutput {
            life_model: input.life_model.clone(),
            tools_prompt: input.tools_prompt.clone(),
            privacy_map: HashMap::new(),
            desensitized_messages: input.messages.clone(),
            memory_context: String::new(),
            context_summary: ContextSummary {
                life_model_empty: input.life_model.is_effectively_empty(),
                included_life_model_sections: vec![],
                memory_hit_count: 0,
                memory_sources: vec![],
                used_tools_prompt: !input.tools_prompt.is_empty(),
                redaction_applied: false,
                redaction_level: crate::agent::types::RedactionLevel::None,
            },
            embed_error: None,
        };

        for assembler in &self.assemblers {
            let partial = assembler.assemble(input)?;
            // Merge partial output into accumulated output
            output.privacy_map.extend(partial.privacy_map);
            if !partial.desensitized_messages.is_empty() {
                output.desensitized_messages = partial.desensitized_messages;
            }
            if !partial.memory_context.is_empty() {
                output.memory_context = partial.memory_context;
            }
            if partial.embed_error.is_some() {
                output.embed_error = partial.embed_error;
            }
            // Merge context summary
            output.context_summary.memory_hit_count += partial.context_summary.memory_hit_count;
            output
                .context_summary
                .memory_sources
                .extend(partial.context_summary.memory_sources);
            if partial.context_summary.redaction_applied {
                output.context_summary.redaction_applied = true;
                output.context_summary.redaction_level = partial.context_summary.redaction_level;
            }
            if !partial
                .context_summary
                .included_life_model_sections
                .is_empty()
            {
                output.context_summary.included_life_model_sections =
                    partial.context_summary.included_life_model_sections;
            }
        }

        Ok(output)
    }
}

// ── P6-3: Governed Context Assembly ──────────────────────────────────

/// Controls which context categories may be included in agent input.
#[derive(Debug, Clone)]
pub struct ContextPolicy {
    pub allow_lifemodel_summary: bool,
    pub allow_goals: bool,
    pub allow_state: bool,
    pub allow_memory: bool,
    pub allow_session_summary: bool,
    pub allow_tool_observations: bool,
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            allow_lifemodel_summary: true,
            allow_goals: true,
            allow_state: true,
            allow_memory: true,
            allow_session_summary: true,
            allow_tool_observations: true,
        }
    }
}

/// Result of governed context assembly — records what was included, excluded,
/// and any privacy/redaction notes without exposing raw sensitive text.
#[derive(Debug, Clone)]
pub struct GovernedAssembleOutput {
    pub included: Vec<String>,
    pub excluded: Vec<String>,
    pub privacy_notes: Vec<String>,
    /// Compact, event-safe summary suitable for AgentRunEvent.
    pub event_summary: String,
}

impl ContextPolicy {
    /// Report included/excluded categories (does not mutate).
    pub fn apply(&self, input: &AssembleInput) -> GovernedAssembleOutput {
        let mut included = Vec::new();
        let mut excluded = Vec::new();
        let mut privacy_notes = Vec::new();

        macro_rules! check {
            ($flag:expr, $name:expr, $has:expr) => {
                if $flag {
                    included.push($name.to_string());
                } else if $has {
                    excluded.push($name.to_string());
                    privacy_notes.push(format!("{} omitted per policy", $name));
                }
            };
        }

        check!(
            self.allow_lifemodel_summary,
            "lifemodel_summary",
            !input.life_model.identity.name.is_empty()
        );
        check!(
            self.allow_goals,
            "goals",
            !input.life_model.goals.short_term.is_empty()
        );
        check!(
            self.allow_state && !input.life_model.state.current_focus.is_empty(),
            "state",
            !input.life_model.state.current_focus.is_empty()
        );
        check!(self.allow_memory, "memory", !input.memory_hits.is_empty());
        check!(
            self.allow_session_summary,
            "session_summary",
            !input.messages.is_empty()
        );
        check!(
            self.allow_tool_observations,
            "tool_observations",
            !input.tools_prompt.is_empty()
        );

        let event_summary = format!(
            "governed assembly: included=[{}] excluded=[{}]",
            included.join(", "),
            excluded.join(", ")
        );

        GovernedAssembleOutput {
            included,
            excluded,
            privacy_notes,
            event_summary,
        }
    }

    /// Filter an `AssembleInput` in-place, clearing categories denied by
    /// this policy.  Returns a `GovernedAssembleOutput` for trace/audit.
    pub fn filter_input(&self, input: &mut AssembleInput) -> GovernedAssembleOutput {
        let report = self.apply(input);

        if !self.allow_memory {
            input.memory_context = None;
            input.memory_hits.clear();
        }
        if !self.allow_tool_observations {
            input.tools_prompt.clear();
        }
        if !self.allow_session_summary {
            input.messages = std::sync::Arc::new(vec![]);
        }
        if !self.allow_goals {
            std::sync::Arc::make_mut(&mut input.life_model).goals =
                crate::life_model::Goals::default();
        }
        if !self.allow_state {
            std::sync::Arc::make_mut(&mut input.life_model).state =
                crate::life_model::State::default();
        }
        if !self.allow_lifemodel_summary {
            // Replace with an empty LifeModel to avoid leaking all details.
            input.life_model = std::sync::Arc::new(crate::life_model::LifeModel::default_model());
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ChatMessage;

    fn create_test_input() -> AssembleInput {
        AssembleInput {
            session_id: "test-session".to_string(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello world".to_string(),
            }]),
            life_model: Arc::new(LifeModel::default()),
            tools_prompt: String::new(),
            privacy_engine: PrivacyEngine::default(),
            memory_context: None,
            memory_hits: vec![],
            memory_retrieval_time_ms: 0,
        }
    }

    #[test]
    fn test_life_model_assembler() {
        let assembler = LifeModelAssembler;
        let input = create_test_input();
        let output = assembler.assemble(&input).unwrap();

        assert_eq!(output.context_summary.included_life_model_sections.len(), 4);
        // Note: LifeModel::default() may not be effectively empty depending on implementation
    }

    #[test]
    fn test_privacy_assembler() {
        let assembler = PrivacyAssembler;
        let input = AssembleInput {
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: "我的电话是 13800138000".to_string(),
            }]),
            ..create_test_input()
        };

        let output = assembler.assemble(&input).unwrap();

        assert!(!output.privacy_map.is_empty());
        assert!(output.context_summary.redaction_applied);
    }

    #[test]
    fn test_memory_assembler_empty() {
        let assembler = MemoryAssembler;
        let input = create_test_input();
        let output = assembler.assemble(&input).unwrap();

        assert!(output.memory_context.is_empty());
        assert_eq!(output.context_summary.memory_hit_count, 0);
    }

    #[test]
    fn test_memory_assembler_with_hits() {
        let assembler = MemoryAssembler;
        let input = AssembleInput {
            memory_context: Some("最近讨论了三体问题".to_string()),
            memory_hits: vec![
                MemoryHit {
                    id: 1,
                    content: "三体问题讨论".to_string(),
                    source: "chat".to_string(),
                    score: 0.92,
                    tier: 1,
                },
                MemoryHit {
                    id: 2,
                    content: "时间管理技巧".to_string(),
                    source: "note".to_string(),
                    score: 0.85,
                    tier: 2,
                },
            ],
            memory_retrieval_time_ms: 45,
            ..create_test_input()
        };

        let output = assembler.assemble(&input).unwrap();

        assert!(output.memory_context.contains("最近讨论了三体问题"));
        assert!(output.memory_context.contains("检索到 2 条记忆"));
        assert!(output.memory_context.contains("耗时 45ms"));
        assert_eq!(output.context_summary.memory_hit_count, 2);
        assert_eq!(output.context_summary.memory_sources, vec!["chat", "note"]);
    }

    #[test]
    fn test_composite_assembler_with_memory() {
        let assembler = CompositeAssembler::new()
            .with(Box::new(LifeModelAssembler))
            .with(Box::new(MemoryAssembler));

        let mut input = create_test_input();
        input.memory_context = Some("关键记忆".to_string());
        input.memory_hits = vec![MemoryHit {
            id: 1,
            content: "测试".to_string(),
            source: "test".to_string(),
            score: 0.9,
            tier: 1,
        }];

        let output = assembler.assemble(&input).unwrap();

        assert_eq!(output.context_summary.included_life_model_sections.len(), 4);
        assert!(output.memory_context.contains("关键记忆"));
        assert_eq!(output.context_summary.memory_hit_count, 1);
    }

    // ── P6-3: ContextPolicy tests ─────────────────────────────────────

    #[test]
    fn test_policy_allows_lifemodel_summary() {
        let policy = ContextPolicy::default();
        let input = create_test_input();
        let output = policy.apply(&input);
        assert!(output.included.contains(&"lifemodel_summary".to_string()));
    }

    #[test]
    fn test_policy_denies_memory() {
        let policy = ContextPolicy {
            allow_memory: false,
            ..Default::default()
        };
        let mut input = create_test_input();
        input.memory_hits = vec![MemoryHit {
            id: 1,
            content: "secret".to_string(),
            source: "test".to_string(),
            score: 0.9,
            tier: 1,
        }];
        let output = policy.apply(&input);
        assert!(output.excluded.contains(&"memory".to_string()));
        assert!(output.privacy_notes.iter().any(|n| n.contains("memory")));
    }

    #[test]
    fn test_privacy_note_on_denied_context() {
        let policy = ContextPolicy {
            allow_memory: false,
            ..Default::default()
        };
        let mut input = create_test_input();
        input.memory_hits = vec![MemoryHit {
            id: 1,
            content: "some memory".to_string(),
            source: "test".to_string(),
            score: 0.5,
            tier: 1,
        }];
        let output = policy.apply(&input);
        assert!(output.excluded.contains(&"memory".to_string()));
        assert!(output.privacy_notes.iter().any(|n| n.contains("memory")));
    }

    #[test]
    fn test_event_summary_excludes_raw_sensitive_text() {
        let policy = ContextPolicy {
            allow_memory: false,
            ..Default::default()
        };
        let mut input = create_test_input();
        input.memory_hits = vec![MemoryHit {
            id: 1,
            content: "sensitive personal data here".to_string(),
            source: "test".to_string(),
            score: 0.9,
            tier: 1,
        }];
        let output = policy.apply(&input);
        assert!(!output.event_summary.contains("sensitive personal data"));
        assert!(output.event_summary.contains("excluded"));
    }
}
