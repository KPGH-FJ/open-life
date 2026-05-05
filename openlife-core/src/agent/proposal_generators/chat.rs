use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use crate::life_model::LifeModel;
use anyhow::Result;

/// Generates proposals from chat messages using keyword-based signal extraction.
pub struct ChatProposalGenerator {
    /// Minimum message length to trigger extraction
    pub min_message_length: usize,
    /// Confidence threshold for proposals
    pub confidence_threshold: f32,
    /// Cooldown duration in seconds between extractions for the same session
    pub cooldown_seconds: i64,
    /// Last extraction timestamp per session
    last_extraction:
        std::sync::Mutex<std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>>,
}

impl Default for ChatProposalGenerator {
    fn default() -> Self {
        Self {
            min_message_length: 10,
            confidence_threshold: 0.6,
            cooldown_seconds: 300, // 5 minutes
            last_extraction: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl ChatProposalGenerator {
    pub fn new(min_length: usize, confidence: f32, cooldown: i64) -> Self {
        Self {
            min_message_length: min_length,
            confidence_threshold: confidence,
            cooldown_seconds: cooldown,
            last_extraction: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Calculate dynamic confidence based on signal count, strength, and context.
    fn calculate_confidence(signal_count: usize, text_len: usize, has_emphasis: bool) -> f32 {
        let base = 0.5f32;
        let count_bonus = (signal_count as f32 * 0.05).min(0.2);
        let strength = if text_len > 0 {
            (20.0 / text_len as f32).min(1.0)
        } else {
            0.0
        };
        let strength_bonus = strength * 0.2;
        let context_bonus = if has_emphasis { 0.1 } else { 0.0 };
        (base + count_bonus + strength_bonus + context_bonus).min(0.95)
    }

    /// Check if text contains emphasis markers (exclamation, strong words).
    fn has_emphasis_markers(text: &str) -> bool {
        let emphasis_words = ["!", "！", "非常", "特别", "强烈", "一定", "必须", "绝对"];
        emphasis_words.iter().any(|word| text.contains(word))
    }

    /// Generate proposals from a chat message.
    /// Attempts LLM-based extraction first (Ollama), falls back to keyword-based.
    pub fn generate_proposals(
        &self,
        session_id: &str,
        message: &str,
        life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        // Check cooldown
        if let Some(last_time) = self.get_last_extraction(session_id) {
            let elapsed = chrono::Utc::now().signed_duration_since(last_time);
            if elapsed.num_seconds() < self.cooldown_seconds {
                return Ok(Vec::new());
            }
        }

        // Check minimum length
        if message.len() < self.min_message_length {
            return Ok(Vec::new());
        }

        // Try LLM-based extraction first (Ollama), then fall back to keyword
        if let Some(llm_proposals) = self.try_llm_extract(session_id, message, life_model) {
            if !llm_proposals.is_empty() {
                self.update_last_extraction(session_id);
                return Ok(llm_proposals);
            }
        }

        // Fallback: keyword-based extraction
        let mut proposals = Vec::new();

        let has_emphasis = Self::has_emphasis_markers(message);
        let text_len = message.len();

        // Extract goals
        if let Some(goals) = Self::extract_goals(message) {
            let signal_count = goals.len();
            for goal in goals {
                let confidence = Self::calculate_confidence(signal_count, text_len, has_emphasis);
                if confidence >= self.confidence_threshold {
                    let mut proposal = AgentProposal::new(
                        ProposalType::GoalUpdate,
                        "/goals/short_term",
                        serde_json::json!({
                            "name": goal.name,
                            "description": goal.description,
                            "priority": goal.priority,
                        }),
                        &format!("Chat signal: user mentioned '{}'", goal.name),
                        confidence,
                        RiskLevel::Low,
                        ProposalSource::FeedbackEvolution,
                    );
                    proposal.source_detail = Some(format!("session:{}", session_id));
                    proposals.push(proposal);
                }
            }
        }

        // Extract state changes
        if let Some(state_update) = Self::extract_state_changes(message) {
            // Count matched state signals
            let signal_count = state_update.as_object().map(|obj| obj.len()).unwrap_or(0);
            let confidence = Self::calculate_confidence(signal_count, text_len, has_emphasis);
            if confidence >= self.confidence_threshold {
                let mut proposal = AgentProposal::new(
                    ProposalType::StateUpdate,
                    "/state",
                    state_update.clone(),
                    "Chat signal: detected state change in conversation",
                    confidence,
                    RiskLevel::Low,
                    ProposalSource::FeedbackEvolution,
                );
                proposal.before = Some(serde_json::to_value(&life_model.state)?);
                proposal.source_detail = Some(format!("session:{}", session_id));
                proposals.push(proposal);
            }
        }

        // Extract capabilities
        if let Some(capability) = Self::extract_capabilities(message) {
            let signal_count = 1; // One capability per extraction
            let confidence = Self::calculate_confidence(signal_count, text_len, has_emphasis);
            if confidence >= self.confidence_threshold {
                let mut proposal = AgentProposal::new(
                    ProposalType::CapabilityUpdate,
                    "/capabilities/skills",
                    capability.clone(),
                    "Chat signal: user mentioned new capability",
                    confidence,
                    RiskLevel::Low,
                    ProposalSource::FeedbackEvolution,
                );
                proposal.source_detail = Some(format!("session:{}", session_id));
                proposals.push(proposal);
            }
        }

        // Extract memory signals (explicit and implicit)
        if let Some(memory_content) = Self::extract_explicit_memory(message) {
            // Explicit memory: high confidence, bypass threshold
            let mut proposal = AgentProposal::new(
                ProposalType::MemoryWrite,
                "/memory/explicit",
                serde_json::json!({
                    "content": memory_content,
                    "source": "chat_explicit",
                    "session_id": session_id,
                }),
                &format!(
                    "用户明确要求记住: {}",
                    &memory_content[..memory_content.len().min(50)]
                ),
                0.95,
                RiskLevel::Medium,
                ProposalSource::FeedbackEvolution,
            );
            proposal.source_detail = Some(format!("session:{}", session_id));
            proposals.push(proposal);
        } else if let Some(memory_content) = Self::extract_implicit_memory(message) {
            // Implicit memory: high threshold, evidence required
            let confidence =
                Self::calculate_confidence(1, message.len(), Self::has_emphasis_markers(message));
            if confidence >= 0.8 {
                let mut proposal = AgentProposal::new(
                    ProposalType::MemoryWrite,
                    "/memory/implicit",
                    serde_json::json!({
                        "content": memory_content,
                        "source": "chat_implicit",
                        "session_id": session_id,
                        "evidence": format!("检测到记忆信号: {}", &memory_content[..memory_content.len().min(50)]),
                    }),
                    &format!(
                        "对话中检测到可记忆内容: {}",
                        &memory_content[..memory_content.len().min(50)]
                    ),
                    confidence,
                    RiskLevel::Low,
                    ProposalSource::FeedbackEvolution,
                );
                proposal.source_detail = Some(format!("session:{}", session_id));
                proposals.push(proposal);
            }
        }

        // Update last extraction time
        if !proposals.is_empty() {
            self.update_last_extraction(session_id);
        }

        Ok(proposals)
    }

    fn get_last_extraction(&self, session_id: &str) -> Option<chrono::DateTime<chrono::Utc>> {
        let map = self.last_extraction.lock().unwrap_or_else(|e| e.into_inner());
        map.get(session_id).copied()
    }

    /// Attempt LLM-based signal extraction via Ollama.
    /// Returns None if Ollama is unavailable or extraction fails (silent fallback).
    fn try_llm_extract(
        &self,
        _session_id: &str,
        message: &str,
        life_model: &LifeModel,
    ) -> Option<Vec<AgentProposal>> {
        let prompt = format!(
            "You are OpenLife's signal extractor. Analyze the user message and extract structured intent.\n\
             Return ONLY valid JSON (no markdown, no explanation):\n\
             {{\n\
               \"explicit_memories\": [\"user: remembered fact\", ...],\n\
               \"goal_signals\": [{{\"text\": \"...\", \"type\": \"short_term|medium_term|long_term\", \"priority\": \"low|medium|high\"}}, ...],\n\
               \"capability_signals\": [{{\"name\": \"skill\", \"proficiency\": 1-10}}, ...],\n\
               \"state_signals\": [{{\"field\": \"emotional_state|health_status|current_focus\", \"value\": \"...\"}}, ...]\n\
             }}\n\
             User message: {message}\n\
             Current LifeModel summary: values={values:?}, goals_count={goal_count}, capabilities_count={cap_count}",
            message = message,
            values = &life_model.identity.values,
            goal_count = life_model.goals.short_term.len()
                + life_model.goals.medium_term.len()
                + life_model.goals.long_term.len(),
            cap_count = life_model.capabilities.skills.len(),
        );

        let life_model_clone = life_model.clone();
        let message_owned = message.to_string();

        let reply = std::thread::spawn(move || -> Result<String, String> {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;
            rt.block_on(async {
                tokio::time::timeout(
                    std::time::Duration::from_secs(8),
                    crate::ollama::chat_with_ollama(
                        "llama3.2:latest",
                        vec![crate::llm::ChatMessage {
                            role: "user".to_string(),
                            content: prompt,
                        }],
                        &life_model_clone,
                    ),
                )
                .await
                .map_err(|_| anyhow::anyhow!("Ollama chat extraction timed out"))
                .and_then(|r| r)
            })
            .map_err(|e| e.to_string())
        })
        .join()
        .ok()
        .unwrap_or_else(|| Err("Ollama extraction thread panicked".to_string()));

        let reply = match reply {
            Ok(r) => r,
            Err(_) => return None,
        };

        // Parse JSON from reply
        let json: serde_json::Value = match serde_json::from_str(&reply) {
            Ok(v) => v,
            Err(_) => {
                // Try extracting JSON from markdown code fences
                let extracted = crate::json_utils::extract_first_json_object(&reply);
                match extracted {
                    Some(v) => match serde_json::from_str::<serde_json::Value>(v) {
                        Ok(val) => val,
                        Err(_) => return None,
                    },
                    None => return None,
                }
            }
        };

        let mut proposals = Vec::new();
        let msg_len = message_owned.len();
        let has_emphasis = Self::has_emphasis_markers(&message_owned);

        // Extract explicit memories
        if let Some(memories) = json.get("explicit_memories").and_then(|v| v.as_array()) {
            for m in memories {
                if let Some(text) = m.as_str() {
                    if !text.is_empty() && text.len() >= 10 {
                        let confidence = Self::calculate_confidence(1, msg_len, has_emphasis);
                        if confidence >= self.confidence_threshold {
                            let proposal = AgentProposal::new(
                                ProposalType::MemoryWrite,
                                &format!("memory.chat.{}", uuid::Uuid::new_v4()),
                                serde_json::json!({"content": text, "source": "chat_llm"}),
                                &format!("Chat message indicates memory: {}", text),
                                confidence,
                                RiskLevel::Low,
                                ProposalSource::Manual,
                            );
                            proposals.push(proposal);
                        }
                    }
                }
            }
        }

        // Extract goals
        if let Some(goals) = json.get("goal_signals").and_then(|v| v.as_array()) {
            for g in goals {
                if let Some(text) = g.get("text").and_then(|v| v.as_str()) {
                    if text.len() >= 5 {
                        let confidence = Self::calculate_confidence(2, msg_len, has_emphasis);
                        if confidence >= self.confidence_threshold {
                            proposals.push(AgentProposal::new(
                                ProposalType::GoalUpdate,
                                "goals",
                                serde_json::json!({
                                    "text": text,
                                    "type": g.get("type").and_then(|v| v.as_str()).unwrap_or("short_term"),
                                    "priority": g.get("priority").and_then(|v| v.as_str()).unwrap_or("medium"),
                                }),
                                &format!("Chat message suggests goal: {}", text),
                                confidence,
                                RiskLevel::Medium,
                                ProposalSource::Manual,
                            ));
                        }
                    }
                }
            }
        }

        // Extract state changes
        if let Some(states) = json.get("state_signals").and_then(|v| v.as_array()) {
            for s in states {
                if let (Some(field), Some(value)) = (
                    s.get("field").and_then(|v| v.as_str()),
                    s.get("value").and_then(|v| v.as_str()),
                ) {
                    let confidence = Self::calculate_confidence(1, msg_len, has_emphasis);
                    if confidence >= self.confidence_threshold {
                        proposals.push(AgentProposal::new(
                            ProposalType::StateUpdate,
                            &format!("state.{}", field),
                            serde_json::json!({field: value}),
                            &format!("Chat message suggests state change: {} = {}", field, value),
                            confidence,
                            RiskLevel::Low,
                            ProposalSource::Manual,
                        ));
                    }
                }
            }
        }

        Some(proposals)
    }

    fn update_last_extraction(&self, session_id: &str) {
        let mut map = self.last_extraction.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(session_id.to_string(), chrono::Utc::now());
    }

    /// Extract goals from text using keyword matching.
    fn extract_goals(text: &str) -> Option<Vec<ExtractedGoal>> {
        let text_lower = text.to_lowercase();

        // Chinese keywords
        let cn_keywords = ["我想", "我要", "计划", "目标", "打算", "希望", "准备"];
        // English keywords
        let en_keywords = [
            "want to",
            "plan to",
            "goal",
            "objective",
            "aim to",
            "would like to",
        ];

        let has_goal_signal = cn_keywords.iter().any(|kw| text_lower.contains(kw))
            || en_keywords.iter().any(|kw| text_lower.contains(kw));

        if !has_goal_signal {
            return None;
        }

        let mut goals = Vec::new();

        // Extract specific goals using simple patterns
        // Pattern: "我想/我要/计划 + [action]"
        for keyword in &cn_keywords {
            if let Some(pos) = text_lower.find(keyword) {
                let start = pos + keyword.len();
                let remaining = &text[start..];
                // Extract up to punctuation or 20 chars
                let end_pos = remaining
                    .find(['。', '，', '！', '\n'])
                    .unwrap_or(remaining.len().min(30));
                let goal_text = remaining[..end_pos].trim();
                if !goal_text.is_empty() && goal_text.len() > 2 {
                    goals.push(ExtractedGoal {
                        name: goal_text.to_string(),
                        description: format!("Extracted from chat: {}", goal_text),
                        priority: 5,
                    });
                }
            }
        }

        // English patterns
        for keyword in &en_keywords {
            if let Some(pos) = text_lower.find(keyword) {
                let start = pos + keyword.len();
                let remaining = &text[start..];
                let end_pos = remaining
                    .find(['.', ',', '!', '\n'])
                    .unwrap_or(remaining.len().min(40));
                let goal_text = remaining[..end_pos].trim();
                if !goal_text.is_empty() && goal_text.len() > 3 {
                    goals.push(ExtractedGoal {
                        name: goal_text.to_string(),
                        description: format!("Extracted from chat: {}", goal_text),
                        priority: 5,
                    });
                }
            }
        }

        if goals.is_empty() {
            None
        } else {
            Some(goals)
        }
    }

    /// Extract state changes from text.
    fn extract_state_changes(text: &str) -> Option<serde_json::Value> {
        let text_lower = text.to_lowercase();

        // Energy level detection
        let energy_level = if text_lower.contains("累")
            || text_lower.contains(" tired")
            || text_lower.contains(" exhausted")
            || text_lower.contains(" fatigue")
        {
            Some(2u8)
        } else if text_lower.contains("精力充沛")
            || text_lower.contains(" energetic")
            || text_lower.contains(" excited")
            || text_lower.contains("活力")
        {
            Some(8u8)
        } else if text_lower.contains("还行")
            || text_lower.contains("okay")
            || text_lower.contains("一般")
        {
            Some(5u8)
        } else {
            None
        };

        // Mood detection
        let mood = if text_lower.contains("开心")
            || text_lower.contains("高兴")
            || text_lower.contains("happy")
            || text_lower.contains(" great")
            || text_lower.contains("不错")
        {
            Some("positive")
        } else if text_lower.contains("难过")
            || text_lower.contains("伤心")
            || text_lower.contains("sad")
            || text_lower.contains(" upset")
            || text_lower.contains("沮丧")
        {
            Some("negative")
        } else if text_lower.contains("平静")
            || text_lower.contains("calm")
            || text_lower.contains(" relaxed")
        {
            Some("neutral")
        } else {
            None
        };

        // Stress level detection
        let stress_level = if text_lower.contains("压力")
            || text_lower.contains("焦虑")
            || text_lower.contains("stress")
            || text_lower.contains(" anxious")
            || text_lower.contains("紧张")
        {
            Some(7u8)
        } else if text_lower.contains("放松")
            || text_lower.contains("relaxed")
            || text_lower.contains(" calm")
        {
            Some(2u8)
        } else {
            None
        };

        if energy_level.is_some() || mood.is_some() || stress_level.is_some() {
            let mut state = serde_json::Map::new();
            if let Some(energy) = energy_level {
                state.insert("energy_level".to_string(), serde_json::json!(energy));
            }
            if let Some(m) = mood {
                state.insert("current_mood".to_string(), serde_json::json!(m));
            }
            if let Some(stress) = stress_level {
                state.insert("stress_level".to_string(), serde_json::json!(stress));
            }
            Some(serde_json::Value::Object(state))
        } else {
            None
        }
    }

    /// Extract explicit memory request from text.
    /// Returns the content to remember if user explicitly asks.
    fn extract_explicit_memory(text: &str) -> Option<String> {
        let text_lower = text.to_lowercase();

        // Explicit memory triggers
        let triggers = [
            "记住这个",
            "记住",
            "以后提醒我",
            "这是我的偏好",
            "保存这个",
            "记下来",
            "remember this",
            "save this",
            "this is my preference",
        ];

        for trigger in &triggers {
            if let Some(pos) = text_lower.find(trigger) {
                let start = pos + trigger.len();
                let remaining = &text[start..];
                // Extract up to punctuation or 100 chars
                let end_pos = remaining
                    .find(['。', '，', '！', '\n', '.', '!'])
                    .unwrap_or(remaining.len().min(100));
                let memory_text = remaining[..end_pos].trim();
                if !memory_text.is_empty() && memory_text.len() > 3 {
                    return Some(memory_text.to_string());
                }
            }
        }

        None
    }

    /// Extract implicit memory signals from text.
    /// Detects important personal information, preferences, habits.
    fn extract_implicit_memory(text: &str) -> Option<String> {
        let text_lower = text.to_lowercase();

        // Implicit memory signals
        let memory_signals = [
            "我喜欢",
            "我不喜欢",
            "我习惯",
            "我通常",
            "我总是",
            "我从不",
            "我的生日",
            "我的爱好",
            "i like",
            "i don't like",
            "i usually",
            "i always",
            "my birthday",
            "my hobby",
        ];

        for signal in &memory_signals {
            if let Some(pos) = text_lower.find(signal) {
                let start = pos;
                let remaining = &text[start..];
                let end_pos = remaining
                    .find(['。', '，', '！', '\n', '.', '!'])
                    .unwrap_or(remaining.len().min(100));
                let memory_text = remaining[..end_pos].trim();
                if !memory_text.is_empty() && memory_text.len() > 5 {
                    return Some(memory_text.to_string());
                }
            }
        }

        None
    }

    /// Extract capabilities from text.
    fn extract_capabilities(text: &str) -> Option<serde_json::Value> {
        let text_lower = text.to_lowercase();

        // Capability detection keywords
        let cn_capability_keywords = ["会", "擅长", "学会了", "掌握", "精通", "熟悉"];
        let en_capability_keywords = [
            "know",
            "can",
            "skilled in",
            "proficient in",
            "good at",
            "expert in",
        ];

        let has_capability_signal = cn_capability_keywords
            .iter()
            .any(|kw| text_lower.contains(kw))
            || en_capability_keywords
                .iter()
                .any(|kw| text_lower.contains(kw));

        if !has_capability_signal {
            return None;
        }

        // Try to extract skill name (simplified)
        // Look for patterns like "我会[skill]" or "I can [skill]"
        let skill_name = if text_lower.contains("python") || text_lower.contains("python") {
            Some("Python")
        } else if text_lower.contains("rust") {
            Some("Rust")
        } else if text_lower.contains("javascript") || text_lower.contains("js") {
            Some("JavaScript")
        } else if text_lower.contains("设计") || text_lower.contains("design") {
            Some("Design")
        } else if text_lower.contains("写作") || text_lower.contains("writing") {
            Some("Writing")
        } else {
            None
        };

        skill_name.map(|skill| {
            serde_json::json!({
                "name": skill,
                "proficiency": 7,
                "description": format!("Extracted from chat: user mentioned {} skill", skill),
            })
        })
    }
}

#[derive(Debug, Clone)]
struct ExtractedGoal {
    name: String,
    description: String,
    priority: u8,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life_model::LifeModel;

    fn create_test_generator() -> ChatProposalGenerator {
        ChatProposalGenerator::new(5, 0.5, 0) // No cooldown for testing
    }

    #[test]
    fn test_extract_goals_chinese() {
        let text = "我想学习 Rust 编程语言";
        let goals = ChatProposalGenerator::extract_goals(text).unwrap();
        assert!(!goals.is_empty());
        assert!(goals[0].name.contains("Rust") || goals[0].name.contains("学习"));
    }

    #[test]
    fn test_extract_goals_english() {
        let text = "I want to learn Python and become a data scientist";
        let goals = ChatProposalGenerator::extract_goals(text).unwrap();
        assert!(!goals.is_empty());
    }

    #[test]
    fn test_no_goal_signal() {
        let text = "今天天气不错";
        let goals = ChatProposalGenerator::extract_goals(text);
        assert!(goals.is_none());
    }

    #[test]
    fn test_extract_state_changes() {
        let text = "我今天很累，压力很大";
        let state = ChatProposalGenerator::extract_state_changes(text).unwrap();
        assert!(state.get("energy_level").is_some());
        assert!(state.get("stress_level").is_some());
    }

    #[test]
    fn test_extract_capabilities() {
        let text = "我会 Python 编程";
        let cap = ChatProposalGenerator::extract_capabilities(text).unwrap();
        assert_eq!(cap.get("name").unwrap(), "Python");
    }

    #[test]
    fn test_generate_proposals() {
        let generator = create_test_generator();
        let model = LifeModel::default();
        let proposals = generator
            .generate_proposals("session-1", "我想学习 Rust 编程，今天感觉很累", &model)
            .unwrap();

        // Should have at least goal and state proposals
        assert!(!proposals.is_empty());
        assert!(proposals
            .iter()
            .any(|p| p.proposal_type == ProposalType::GoalUpdate));
    }

    #[test]
    fn test_cooldown() {
        let generator = ChatProposalGenerator::new(5, 0.5, 3600); // 1 hour cooldown
        let model = LifeModel::default();

        // First extraction should work
        let proposals1 = generator
            .generate_proposals("session-1", "我想学习 Rust", &model)
            .unwrap();
        assert!(!proposals1.is_empty());

        // Second extraction within cooldown should return empty
        let proposals2 = generator
            .generate_proposals("session-1", "我想学习 Python", &model)
            .unwrap();
        assert!(proposals2.is_empty());
    }

    #[test]
    fn test_min_length_filter() {
        let generator = ChatProposalGenerator::new(20, 0.5, 0);
        let model = LifeModel::default();
        let proposals = generator
            .generate_proposals("session-1", "短", &model)
            .unwrap();
        assert!(proposals.is_empty());
    }

    #[test]
    fn test_dynamic_confidence_with_emphasis() {
        let generator = ChatProposalGenerator::new(5, 0.5, 0);
        let model = LifeModel::default();

        // With emphasis markers
        let proposals_emphasis = generator
            .generate_proposals("session-1", "我想学习 Rust！非常有兴趣！", &model)
            .unwrap();

        // Without emphasis markers
        let proposals_normal = generator
            .generate_proposals("session-2", "我想学习 Rust", &model)
            .unwrap();

        // Emphasis should produce higher confidence
        if let (Some(emph), Some(norm)) = (
            proposals_emphasis
                .iter()
                .find(|p| p.proposal_type == ProposalType::GoalUpdate),
            proposals_normal
                .iter()
                .find(|p| p.proposal_type == ProposalType::GoalUpdate),
        ) {
            assert!(
                emph.confidence > norm.confidence,
                "Emphasis confidence ({}) should be > normal confidence ({})",
                emph.confidence,
                norm.confidence
            );
        }
    }

    #[test]
    fn test_dynamic_confidence_signal_count() {
        // More signals = higher confidence
        let conf_1 = ChatProposalGenerator::calculate_confidence(1, 50, false);
        let conf_3 = ChatProposalGenerator::calculate_confidence(3, 50, false);
        let conf_5 = ChatProposalGenerator::calculate_confidence(5, 50, false);

        assert!(
            conf_3 > conf_1,
            "3 signals ({}) > 1 signal ({})",
            conf_3,
            conf_1
        );
        assert!(
            conf_5 > conf_3,
            "5 signals ({}) > 3 signals ({})",
            conf_5,
            conf_3
        );
        assert_eq!(
            conf_5,
            ChatProposalGenerator::calculate_confidence(10, 50, false),
            "Signal count capped at 4 (0.2 bonus max)"
        );
    }

    #[test]
    fn test_dynamic_confidence_bounds() {
        // Minimum confidence
        let min_conf = ChatProposalGenerator::calculate_confidence(0, 1000, false);
        assert!(min_conf >= 0.5, "Minimum confidence should be >= 0.5");

        // Maximum confidence
        let max_conf = ChatProposalGenerator::calculate_confidence(10, 10, true);
        assert!(max_conf <= 0.95, "Maximum confidence should be <= 0.95");
        assert!(max_conf > 0.8, "High signal + emphasis should be > 0.8");
    }

    #[test]
    fn test_extract_explicit_memory() {
        let text = "记住这个：我每天早上 6 点起床跑步";
        let memory = ChatProposalGenerator::extract_explicit_memory(text);
        assert!(memory.is_some());
        assert!(memory.unwrap().contains("早上 6 点起床跑步"));
    }

    #[test]
    fn test_extract_explicit_memory_english() {
        let text = "Remember this: I prefer dark mode for all my apps";
        let memory = ChatProposalGenerator::extract_explicit_memory(text);
        assert!(memory.is_some());
        assert!(memory.unwrap().contains("dark mode"));
    }

    #[test]
    fn test_no_explicit_memory() {
        let text = "今天天气不错";
        let memory = ChatProposalGenerator::extract_explicit_memory(text);
        assert!(memory.is_none());
    }

    #[test]
    fn test_extract_implicit_memory() {
        let text = "我喜欢在周末去爬山，这是我最大的爱好";
        let memory = ChatProposalGenerator::extract_implicit_memory(text);
        assert!(memory.is_some());
        assert!(memory.unwrap().contains("喜欢"));
    }

    #[test]
    fn test_generate_memory_proposal_explicit() {
        let generator = create_test_generator();
        let model = LifeModel::default();
        let proposals = generator
            .generate_proposals("session-1", "记住这个：我喜欢喝美式咖啡", &model)
            .unwrap();

        // Should have memory proposal
        assert!(proposals
            .iter()
            .any(|p| p.proposal_type == ProposalType::MemoryWrite));
        let memory_proposal = proposals
            .iter()
            .find(|p| p.proposal_type == ProposalType::MemoryWrite)
            .unwrap();
        assert!(memory_proposal.confidence >= 0.9); // Explicit memory should have high confidence
    }
}
