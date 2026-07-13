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
    fn apply(&self, input: &AssembleInput, output: &mut AssembleOutput) -> Result<()>;

    fn assemble(&self, input: &AssembleInput) -> Result<AssembleOutput> {
        let mut output = AssembleOutput::from_input(input);
        self.apply(input, &mut output)?;
        Ok(output)
    }
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

impl AssembleOutput {
    fn from_input(input: &AssembleInput) -> Self {
        Self {
            life_model: input.life_model.clone(),
            tools_prompt: input.tools_prompt.clone(),
            privacy_map: HashMap::new(),
            desensitized_messages: input.messages.clone(),
            memory_context: String::new(),
            context_summary: ContextSummary {
                life_model_empty: input.life_model.is_effectively_empty(),
                included_life_model_sections: Vec::new(),
                memory_hit_count: 0,
                memory_sources: Vec::new(),
                used_tools_prompt: !input.tools_prompt.is_empty(),
                redaction_applied: false,
                redaction_level: crate::agent::types::RedactionLevel::None,
            },
            embed_error: None,
        }
    }
}

/// LifeModel assembler: loads and refreshes hot cache.
pub struct LifeModelAssembler;

impl ContextAssembler for LifeModelAssembler {
    fn name(&self) -> &'static str {
        "life_model"
    }

    fn apply(&self, input: &AssembleInput, output: &mut AssembleOutput) -> Result<()> {
        output.life_model = input.life_model.clone();
        output.context_summary.life_model_empty = input.life_model.is_effectively_empty();
        output.context_summary.included_life_model_sections = vec![
            "identity".to_string(),
            "goals".to_string(),
            "capabilities".to_string(),
            "state".to_string(),
        ];
        Ok(())
    }
}

/// Memory assembler: injects prefetched memory context into the assembled output.
pub struct MemoryAssembler;

impl ContextAssembler for MemoryAssembler {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn apply(&self, input: &AssembleInput, output: &mut AssembleOutput) -> Result<()> {
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

        output.memory_context = memory_section;
        output.context_summary.memory_hit_count = hit_count as i64;
        output.context_summary.memory_sources = memory_sources;
        Ok(())
    }
}

/// Privacy assembler: desensitizes every message selected for provider context.
pub struct PrivacyAssembler;

impl ContextAssembler for PrivacyAssembler {
    fn name(&self) -> &'static str {
        "privacy"
    }

    fn apply(&self, input: &AssembleInput, output: &mut AssembleOutput) -> Result<()> {
        let contents = input
            .messages
            .iter()
            .map(|message| message.content.clone())
            .collect::<Vec<_>>();
        let (masked_contents, privacy_map) = input.privacy_engine.desensitize_batch(&contents);
        let desensitized = input
            .messages
            .iter()
            .zip(masked_contents)
            .map(|(message, content)| ChatMessage {
                role: message.role.clone(),
                content,
            })
            .collect::<Vec<_>>();

        let redaction_level = if privacy_map.is_empty() {
            crate::agent::types::RedactionLevel::None
        } else {
            crate::agent::types::RedactionLevel::Light
        };

        output.privacy_map = privacy_map.clone();
        output.desensitized_messages = Arc::new(desensitized);
        output.context_summary.redaction_applied = !privacy_map.is_empty();
        output.context_summary.redaction_level = redaction_level;
        Ok(())
    }
}

/// Tools assembler: prepares tool prompts.
pub struct ToolsAssembler;

impl ContextAssembler for ToolsAssembler {
    fn name(&self) -> &'static str {
        "tools"
    }

    fn apply(&self, input: &AssembleInput, output: &mut AssembleOutput) -> Result<()> {
        output.tools_prompt = input.tools_prompt.clone();
        output.context_summary.used_tools_prompt = !input.tools_prompt.is_empty();
        Ok(())
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

    fn apply(&self, input: &AssembleInput, output: &mut AssembleOutput) -> Result<()> {
        for assembler in &self.assemblers {
            assembler.apply(input, output)?;
        }
        Ok(())
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

    #[test]
    fn production_composite_preserves_privacy_transform_after_memory_and_tools() {
        let assembler = CompositeAssembler::new()
            .with(Box::new(LifeModelAssembler))
            .with(Box::new(PrivacyAssembler))
            .with(Box::new(MemoryAssembler))
            .with(Box::new(ToolsAssembler));
        let input = AssembleInput {
            messages: Arc::new(vec![
                ChatMessage {
                    role: "system".into(),
                    content: "System contact system@example.com.".into(),
                },
                ChatMessage {
                    role: "assistant".into(),
                    content: "Prior reply restored assistant@example.com.".into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: "Contact qa@example.com or 13800138000.".into(),
                },
            ]),
            tools_prompt: "typed tool manifest".into(),
            memory_context: Some("bounded memory context".into()),
            memory_hits: vec![MemoryHit {
                id: 1,
                content: "bounded memory context".into(),
                source: "test".into(),
                score: 1.0,
                tier: 1,
            }],
            ..create_test_input()
        };

        let output = assembler.assemble(&input).unwrap();
        let outbound_text = output
            .desensitized_messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for sensitive in [
            "system@example.com",
            "assistant@example.com",
            "qa@example.com",
            "13800138000",
        ] {
            assert!(!outbound_text.contains(sensitive));
        }
        assert!(outbound_text.contains("<EMAIL_0>"));
        assert!(outbound_text.contains("<EMAIL_1>"));
        assert!(outbound_text.contains("<EMAIL_2>"));
        assert!(outbound_text.contains("<PHONE_0>"));
        assert!(!output.privacy_map.is_empty());
        assert!(output.context_summary.redaction_applied);
        assert!(output.memory_context.contains("bounded memory context"));
        assert_eq!(output.tools_prompt, "typed tool manifest");
        assert_eq!(output.context_summary.included_life_model_sections.len(), 4);
    }

    #[test]
    fn composite_privacy_transform_is_order_invariant() {
        let input = AssembleInput {
            messages: Arc::new(vec![ChatMessage {
                role: "user".into(),
                content: "Contact qa@example.com or 13800138000.".into(),
            }]),
            tools_prompt: "typed tool manifest".into(),
            memory_context: Some("bounded memory context".into()),
            memory_hits: vec![MemoryHit {
                id: 1,
                content: "bounded memory context".into(),
                source: "test".into(),
                score: 1.0,
                tier: 1,
            }],
            ..create_test_input()
        };
        let privacy_first = CompositeAssembler::new()
            .with(Box::new(PrivacyAssembler))
            .with(Box::new(MemoryAssembler))
            .with(Box::new(ToolsAssembler))
            .assemble(&input)
            .unwrap();
        let privacy_last = CompositeAssembler::new()
            .with(Box::new(MemoryAssembler))
            .with(Box::new(ToolsAssembler))
            .with(Box::new(PrivacyAssembler))
            .assemble(&input)
            .unwrap();

        let message_projection = |messages: &Arc<Vec<ChatMessage>>| {
            messages
                .iter()
                .map(|message| (message.role.clone(), message.content.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            message_projection(&privacy_first.desensitized_messages),
            message_projection(&privacy_last.desensitized_messages)
        );
        assert_eq!(privacy_first.privacy_map, privacy_last.privacy_map);
        assert_eq!(
            privacy_first.context_summary.redaction_applied,
            privacy_last.context_summary.redaction_applied
        );
    }
}
