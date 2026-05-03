use crate::agent::types::{AgentProposal, AgentRun, ProposalSource, ProposalType, RiskLevel};
use crate::life_model::LifeModel;
use anyhow::Result;
use serde_json::Value;

/// Trait for generating proposals from agent runs.
pub trait ProposalGenerator: Send + Sync {
    fn name(&self) -> &'static str;
    fn source(&self) -> ProposalSource;

    /// Generate proposals from an agent run output.
    ///
    /// # Arguments
    /// * `run` - The agent run that produced the output
    /// * `output` - The output text to analyze
    /// * `life_model` - Current life model for context
    fn generate(
        &self,
        run: &AgentRun,
        output: &str,
        life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>>;
}

/// Engine that manages multiple proposal generators.
pub struct ProposalEngine {
    generators: Vec<Box<dyn ProposalGenerator>>,
}

impl Default for ProposalEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ProposalEngine {
    pub fn new() -> Self {
        Self {
            generators: Vec::new(),
        }
    }

    /// Register a proposal generator.
    pub fn register(&mut self, generator: Box<dyn ProposalGenerator>) {
        self.generators.push(generator);
    }

    /// Generate proposals from all registered generators.
    pub fn generate_from_run(
        &self,
        run: &AgentRun,
        output: &str,
        life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        let mut all_proposals = Vec::new();

        for generator in &self.generators {
            match generator.generate(run, output, life_model) {
                Ok(proposals) => {
                    for mut proposal in proposals {
                        // Set run_id and source
                        proposal.run_id = Some(run.id.clone());
                        proposal.source = generator.source();
                        all_proposals.push(proposal);
                    }
                }
                Err(e) => {
                    log::warn!("Proposal generator '{}' failed: {}", generator.name(), e);
                }
            }
        }

        Ok(all_proposals)
    }
}

/// Builder proposal generator.
pub struct BuilderProposalGenerator;

impl ProposalGenerator for BuilderProposalGenerator {
    fn name(&self) -> &'static str {
        "builder"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::BuilderReview
    }

    fn generate(
        &self,
        _run: &AgentRun,
        _output: &str,
        _life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        // Builder proposals are created directly in builder.rs
        // This generator is a placeholder for future automatic proposal generation
        Ok(Vec::new())
    }
}

/// Calibration proposal generator.
pub struct CalibrationProposalGenerator;

impl ProposalGenerator for CalibrationProposalGenerator {
    fn name(&self) -> &'static str {
        "calibration"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::CalibrationRun
    }

    fn generate(
        &self,
        _run: &AgentRun,
        _output: &str,
        _life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        // Calibration proposals are created directly in calibration.rs
        Ok(Vec::new())
    }
}

/// Feedback proposal generator.
pub struct FeedbackProposalGenerator;

impl ProposalGenerator for FeedbackProposalGenerator {
    fn name(&self) -> &'static str {
        "feedback"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::FeedbackEvolution
    }

    fn generate(
        &self,
        _run: &AgentRun,
        _output: &str,
        _life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        // TODO: Implement feedback-based proposal generation
        Ok(Vec::new())
    }
}

/// Memory governance proposal generator.
pub struct MemoryProposalGenerator;

impl ProposalGenerator for MemoryProposalGenerator {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::MemoryGovernance
    }

    fn generate(
        &self,
        run: &AgentRun,
        output: &str,
        _life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        let mut proposals = Vec::new();
        if let Ok(value) = serde_json::from_str::<Value>(output) {
            collect_memory_proposals_from_value(&value, "assistant_output", &mut proposals);
        }
        for observation in &run.observations {
            if let Some(value) = &observation.structured_result {
                collect_memory_proposals_from_value(value, &observation.source, &mut proposals);
            }
        }
        for action in &run.actions {
            if let Some(value) = &action.output {
                collect_memory_proposals_from_value(value, "action_output", &mut proposals);
            }
        }
        Ok(proposals)
    }
}

/// Tool permission proposal generator.
pub struct ToolProposalGenerator;

impl ProposalGenerator for ToolProposalGenerator {
    fn name(&self) -> &'static str {
        "tool"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::Manual
    }

    fn generate(
        &self,
        run: &AgentRun,
        _output: &str,
        _life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        let mut proposals = Vec::new();
        for action in &run.actions {
            let needs_permission = action.status == "needs_confirmation"
                || action.permission_decision.as_deref() == Some("ask_every_time");
            if !needs_permission || !action.action_type.contains("tool") {
                continue;
            }
            let tool_name = action
                .tool_scope
                .as_ref()
                .map(|scope| scope.tool_name.as_str())
                .or(action.target.as_deref())
                .unwrap_or("unknown_tool");
            let source = action
                .tool_scope
                .as_ref()
                .map(|scope| scope.source.as_str())
                .unwrap_or("builtin");
            let risk_level = action
                .tool_scope
                .as_ref()
                .map(|scope| scope.risk_level.as_str())
                .or_else(|| action.input.get("risk_level").and_then(Value::as_str))
                .unwrap_or("high");
            let action_type = action
                .tool_scope
                .as_ref()
                .map(|scope| scope.action_type.as_str())
                .unwrap_or(&action.action_type);
            let after = serde_json::json!({
                "tool_name": tool_name,
                "permission": "allow_until_revoked",
                "source": source,
                "risk_level": risk_level,
                "action_type": action_type,
            });
            let mut proposal = AgentProposal::new(
                ProposalType::ToolPermission,
                &format!("tool_permissions.{}", tool_name),
                after,
                "工具调用被权限策略阻断，需要用户确认后才能放行。",
                0.8,
                RiskLevel::High,
                ProposalSource::Manual,
            );
            proposal.source_detail = Some(action.id.clone());
            proposals.push(proposal);
        }
        Ok(proposals)
    }
}

/// Chat proposal generator - extracts LifeModel change suggestions from conversation.
pub struct ChatProposalGenerator;

impl ProposalGenerator for ChatProposalGenerator {
    fn name(&self) -> &'static str {
        "chat"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::ChatConversation
    }

    fn generate(
        &self,
        _run: &AgentRun,
        output: &str,
        life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        let mut proposals = Vec::new();

        // Pattern 1: Goal updates
        if let Some(goal) = extract_goal_suggestion(output) {
            let confidence = calculate_goal_confidence(output, &goal);
            if confidence > 0.6 {
                let after = serde_json::json!({
                    "goals": {
                        "short_term": [goal]
                    }
                });
                proposals.push(AgentProposal::new(
                    ProposalType::LifeModelUpdate,
                    "goals.short_term",
                    after,
                    &format!("对话中检测到目标建议: {}", goal),
                    confidence,
                    RiskLevel::Low,
                    ProposalSource::ChatConversation,
                ));
            }
        }

        // Pattern 2: State updates (emotion, focus, health)
        if let Some((path, value)) = extract_state_suggestion(output) {
            let confidence = calculate_state_confidence(output, &path, &value);
            if confidence > 0.6 {
                let after = serde_json::json!({
                    "state": {
                        &path: value
                    }
                });
                proposals.push(AgentProposal::new(
                    ProposalType::LifeModelUpdate,
                    &format!("state.{}", path),
                    after,
                    &format!("对话中检测到状态更新建议: {} = {}", path, value),
                    confidence,
                    RiskLevel::Low,
                    ProposalSource::ChatConversation,
                ));
            }
        }

        // Pattern 3: Capability updates
        if let Some(capability) = extract_capability_suggestion(output) {
            let confidence = calculate_capability_confidence(output, &capability);
            if confidence > 0.6 {
                let mut capabilities = life_model.capabilities.clone();
                let skill_exists = capabilities.skills.iter().any(|s| s.name == capability);
                if !skill_exists {
                    capabilities.skills.push(crate::life_model::Skill {
                        name: capability.clone(),
                        proficiency: 1,
                        description: format!("从对话中提取的能力建议: {}", capability),
                    });
                }
                let after = serde_json::json!({ "capabilities": capabilities });
                proposals.push(AgentProposal::new(
                    ProposalType::LifeModelUpdate,
                    "capabilities.skills",
                    after,
                    &format!("对话中检测到能力培养建议: {}", capability),
                    confidence,
                    RiskLevel::Low,
                    ProposalSource::ChatConversation,
                ));
            }
        }

        Ok(proposals)
    }
}

/// Extract goal suggestion from assistant output.
fn extract_goal_suggestion(text: &str) -> Option<String> {
    // Pattern: "建议你将...作为目标" / "你的目标应该是..." / "我推荐你设定..."
    let patterns = [
        r"建议你将(.+?)(?:作为|设为)目标",
        r"你的目标应该是(.+?)(?:。|$)",
        r"我推荐你设定(.+?)(?:目标|计划)",
        r"(?:可以|应该)把(.+?)加入(?:你的)?(?:短期|近期)?目标",
    ];

    for pattern in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(text) {
                if let Some(matched) = caps.get(1) {
                    let goal = matched.as_str().trim();
                    if !goal.is_empty() && goal.len() > 3 {
                        return Some(goal.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Extract state suggestion from assistant output.
fn extract_state_suggestion(text: &str) -> Option<(String, String)> {
    // Pattern: "你现在的状态是..." / "你的情绪看起来..." / "当前焦点在..."
    let patterns: [(&str, &str); 4] = [
        (
            r"(?:你现在的|当前)(?:情绪|心情|感受)(?:是|看起来)(.+?)(?:。|$)",
            "emotional_state",
        ),
        (
            r"(?:你的|当前)(?:焦点|关注点)(?:是|在)(.+?)(?:。|$)",
            "current_focus",
        ),
        (
            r"(?:你(?:看起来|似乎)|当前)(?:健康|身体)(?:状况|状态)(?:是|为)(.+?)(?:。|$)",
            "health_status",
        ),
        (
            r"(?:你|当前)(?:精力|能量|状态)(?:水平|指数)(?:是|为)(.+?)(?:。|$)",
            "energy_level",
        ),
    ];

    for (pattern, field) in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(text) {
                if let Some(matched) = caps.get(1) {
                    let value = matched.as_str().trim();
                    if !value.is_empty() && value.len() > 2 {
                        return Some((field.to_string(), value.to_string()));
                    }
                }
            }
        }
    }
    None
}

/// Extract capability suggestion from assistant output.
fn extract_capability_suggestion(text: &str) -> Option<String> {
    // Pattern: "你需要培养..." / "建议你学习..." / "提升...能力"
    let patterns = [
        r"(?:需要|应该|建议)(?:你)?(?:培养|发展|提升|学习)(?:.+?)?(.+?)(?:能力|技能|素质)",
        r"(?:可以|值得)(?:你)?(?:学习|掌握|培养)(?:一下)?(.+?)(?:。|$)",
    ];

    for pattern in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(text) {
                if let Some(matched) = caps.get(1) {
                    let cap = matched.as_str().trim();
                    if !cap.is_empty() && cap.len() > 2 {
                        return Some(cap.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Calculate confidence score for goal suggestion.
fn calculate_goal_confidence(text: &str, goal: &str) -> f32 {
    let mut score: f32 = 0.5;

    // Direct suggestion indicators increase confidence
    if text.contains("建议") || text.contains("推荐") {
        score += 0.15;
    }
    if text.contains("目标") || text.contains("计划") {
        score += 0.1;
    }

    // Specific and actionable goals are higher confidence
    if goal.len() > 10 {
        score += 0.1;
    }
    if goal.contains("每天") || goal.contains("每周") || goal.contains("每月") {
        score += 0.1; // Time-bound goals are more specific
    }

    score.min(0.95)
}

/// Calculate confidence score for state suggestion.
fn calculate_state_confidence(text: &str, _path: &str, value: &str) -> f32 {
    let mut score: f32 = 0.5;

    if text.contains("看起来") || text.contains("似乎") {
        score += 0.1;
    }
    if text.contains("现在") || text.contains("当前") {
        score += 0.1;
    }

    // Specific values are higher confidence
    if value.len() > 3 && value.len() < 50 {
        score += 0.15;
    }

    score.min(0.95)
}

/// Calculate confidence score for capability suggestion.
fn calculate_capability_confidence(text: &str, _capability: &str) -> f32 {
    let mut score: f32 = 0.5;

    if text.contains("培养") || text.contains("学习") || text.contains("提升") {
        score += 0.15;
    }
    if text.contains("能力") || text.contains("技能") {
        score += 0.1;
    }

    score.min(0.95)
}

fn collect_memory_proposals_from_value(
    value: &Value,
    source_detail: &str,
    proposals: &mut Vec<AgentProposal>,
) {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        if let Ok(nested) = serde_json::from_str::<Value>(text) {
            collect_memory_proposals_from_value(&nested, source_detail, proposals);
        }
    }

    // Note: external_write_action proposals are created exclusively by ActionExecutor
    // during file.write_proposal execution. ProposalEngine no longer scans for them
    // to avoid duplicate proposals.

    if let Some(memory_write) = value
        .get("memory_write")
        .or_else(|| value.get("memory_candidate"))
    {
        if let Some(content) = memory_write.get("content").and_then(Value::as_str) {
            let mut after = memory_write.clone();
            if after.get("content").is_none() {
                after = serde_json::json!({ "content": content });
            }
            let mut proposal = AgentProposal::new(
                ProposalType::MemoryWrite,
                "memory.candidates",
                after,
                "检测到显式长期记忆候选，需要用户确认后写入 MemoryStore。",
                0.7,
                RiskLevel::Medium,
                ProposalSource::MemoryGovernance,
            );
            proposal.source_detail = Some(source_detail.to_string());
            proposals.push(proposal);
        }
    }

    if let Some(candidates) = value.get("memory_candidates").and_then(Value::as_array) {
        for candidate in candidates {
            if candidate.get("content").and_then(Value::as_str).is_some() {
                let mut proposal = AgentProposal::new(
                    ProposalType::MemoryWrite,
                    "memory.candidates",
                    candidate.clone(),
                    "检测到显式长期记忆候选，需要用户确认后写入 MemoryStore。",
                    0.7,
                    RiskLevel::Medium,
                    ProposalSource::MemoryGovernance,
                );
                proposal.source_detail = Some(source_detail.to_string());
                proposals.push(proposal);
            }
        }
    }

    if let Some(memory_archive) = value.get("memory_archive") {
        if memory_archive
            .get("chunk_ids")
            .and_then(Value::as_array)
            .is_some_and(|chunk_ids| !chunk_ids.is_empty())
        {
            let mut proposal = AgentProposal::new(
                ProposalType::MemoryArchive,
                "memory.archive",
                memory_archive.clone(),
                "检测到显式记忆归档建议，需要用户确认后归档指定 chunk。",
                0.7,
                RiskLevel::Medium,
                ProposalSource::MemoryGovernance,
            );
            proposal.source_detail = Some(source_detail.to_string());
            proposals.push(proposal);
        }
    }
}

/// Chat proposal generator adapter that wraps proposal_generators::ChatProposalGenerator.
pub struct ChatProposalGeneratorAdapter {
    inner: crate::agent::proposal_generators::ChatProposalGenerator,
}

impl Default for ChatProposalGeneratorAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatProposalGeneratorAdapter {
    pub fn new() -> Self {
        Self {
            inner: crate::agent::proposal_generators::ChatProposalGenerator::default(),
        }
    }
}

impl ProposalGenerator for ChatProposalGeneratorAdapter {
    fn name(&self) -> &'static str {
        "chat"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::ProactiveAgent
    }

    fn generate(
        &self,
        run: &AgentRun,
        _output: &str,
        life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        let session_id = run.session_id.as_deref().unwrap_or("unknown");
        let user_input = run.user_input.as_deref().unwrap_or("");
        self.inner
            .generate_proposals(session_id, user_input, life_model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proposal_engine_registration() {
        let mut engine = ProposalEngine::new();
        engine.register(Box::new(BuilderProposalGenerator));
        engine.register(Box::new(CalibrationProposalGenerator));
        engine.register(Box::new(ChatProposalGeneratorAdapter::new()));

        assert_eq!(engine.generators.len(), 3);
    }

    #[test]
    fn test_builder_generator_source() {
        let gen = BuilderProposalGenerator;
        assert_eq!(gen.name(), "builder");
        assert_eq!(gen.source(), ProposalSource::BuilderReview);
    }

    #[test]
    fn test_chat_adapter_name() {
        let gen = ChatProposalGeneratorAdapter::new();
        assert_eq!(gen.name(), "chat");
        assert_eq!(gen.source(), ProposalSource::ProactiveAgent);
    }

    #[test]
    fn test_memory_generator_creates_write_and_archive_proposals_from_explicit_json() {
        let gen = MemoryProposalGenerator;
        let run = AgentRun::new_chat_run("session-1", "记住这件事");
        let output = serde_json::json!({
            "memory_write": { "content": "用户偏好短句反馈", "source": "chat" },
            "memory_archive": { "chunk_ids": [1, 2], "reason": "过期" }
        })
        .to_string();

        let proposals = gen.generate(&run, &output, &LifeModel::default()).unwrap();

        assert_eq!(proposals.len(), 2);
        assert!(proposals
            .iter()
            .any(|proposal| proposal.proposal_type == ProposalType::MemoryWrite));
        assert!(proposals
            .iter()
            .any(|proposal| proposal.proposal_type == ProposalType::MemoryArchive));
    }

    #[test]
    fn test_memory_generator_creates_external_write_proposal_from_action_output() {
        let gen = MemoryProposalGenerator;
        let mut run = AgentRun::new_chat_run("session-1", "写文件");
        run.actions.push(crate::agent::types::AgentAction {
            id: "action-1".into(),
            action_type: "mcp_tool_call".into(),
            target: Some("file.write_proposal".into()),
            input: serde_json::json!({ "arguments": { "path": "/tmp/openlife.txt" } }),
            output: Some(serde_json::json!({
                "text": serde_json::json!({
                    "proposal_type": "external_write_action",
                    "external_write_action": {
                        "path": "/tmp/openlife.txt",
                        "content": "hello",
                        "content_preview": "hello",
                        "content_length": 5
                    }
                }).to_string()
            })),
            status: "succeeded".into(),
            permission_decision: Some("allow".into()),
            tool_scope: None,
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
        });

        let proposals = gen.generate(&run, "", &LifeModel::default()).unwrap();

        // ProposalEngine no longer creates ExternalWriteAction proposals from action output.
        // ExternalWriteAction proposals are created exclusively by ActionExecutor.
        assert_eq!(proposals.len(), 0);
    }

    #[test]
    fn test_tool_generator_creates_permission_proposal_for_blocked_action() {
        let gen = ToolProposalGenerator;
        let mut run = AgentRun::new_chat_run("session-1", "调用工具");
        run.actions.push(crate::agent::types::AgentAction {
            id: "action-1".into(),
            action_type: "mcp_tool_call".into(),
            target: Some("write_file".into()),
            input: serde_json::json!({ "arguments": {}, "risk_level": "high" }),
            output: None,
            status: "needs_confirmation".into(),
            permission_decision: Some("ask_every_time".into()),
            tool_scope: None,
            started_at: None,
            finished_at: None,
            error: None,
            timestamp: chrono::Utc::now(),
        });

        let proposals = gen.generate(&run, "", &LifeModel::default()).unwrap();

        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].proposal_type, ProposalType::ToolPermission);
        assert_eq!(proposals[0].affected_path, "tool_permissions.write_file");
    }
}
