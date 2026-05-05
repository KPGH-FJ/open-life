use crate::agent::context_assembler::AssembleOutput;
use crate::agent::reasoning::{
    ReasoningConfig, ReasoningError, ReasoningInput, ReasoningOutput, ReasoningPhaseKind,
    ReasoningStrategy, ReasoningTrace,
};
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::scheduler::InferenceScheduler;
use serde_json::json;
// use tokio::time::{timeout, Duration};

/// Layered reasoning strategy: three-phase sequential reasoning.
/// Layered reasoning strategy with meaning, strategy, and generation phases.
pub struct LayeredReasoner {
    scheduler: InferenceScheduler,
    life_model: LifeModel,
    config: ReasoningConfig,
}

impl LayeredReasoner {
    pub fn new(scheduler: InferenceScheduler, life_model: LifeModel) -> Self {
        Self {
            scheduler,
            life_model,
            config: ReasoningConfig::default(),
        }
    }

    pub fn with_config(
        scheduler: InferenceScheduler,
        life_model: LifeModel,
        config: ReasoningConfig,
    ) -> Self {
        Self {
            scheduler,
            life_model,
            config,
        }
    }
}

#[async_trait::async_trait]
impl ReasoningStrategy for LayeredReasoner {
    fn name(&self) -> &'static str {
        "layered"
    }

    async fn reason(
        &self,
        input: &ReasoningInput,
        context: &AssembleOutput,
        _run_id: &str,
    ) -> Result<ReasoningOutput, ReasoningError> {
        let mut trace = ReasoningTrace {
            input: Some(input.user_text.clone()),
            ..Default::default()
        };

        // Phase 1: Meaning Analysis
        let meaning = self.run_meaning_phase(input, context, &mut trace).await?;

        // Phase 2: Strategy Planning
        let strategy = self
            .run_strategy_phase(input, context, &meaning, &mut trace)
            .await?;

        // Phase 3: Response Generation
        let generation = self
            .run_generation_phase(input, context, &meaning, &strategy, &mut trace)
            .await?;

        // Safety Check
        let safety_result = SafetyChecker::new(true).check(&trace);
        trace.safety_check_result = Some(serde_json::json!({
            "passed": safety_result.passed,
            "warnings": safety_result.warnings,
            "strict_mode": safety_result.strict_mode,
        }));

        if !safety_result.passed && safety_result.strict_mode {
            return Err(ReasoningError {
                phase: "safety_check".to_string(),
                message: format!("Safety check failed: {}", safety_result.warnings.join("; ")),
                recoverable: true,
            });
        }

        // Extract tool plan from strategy
        trace.tool_plan = strategy
            .get("suggested_tools")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Extract stable steps
        trace.stable_steps = strategy
            .get("plan_steps")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let system_prompt = self.build_system_prompt(&meaning, &strategy);

        trace.output = generation
            .get("text")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        let stable_steps = trace.stable_steps.clone();

        Ok(ReasoningOutput {
            system_prompt,
            trace,
            suggested_tools: strategy
                .get("suggested_tools")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            plan_steps: stable_steps,
        })
    }

    fn config(&self) -> ReasoningConfig {
        self.config.clone()
    }
}

impl LayeredReasoner {
    async fn run_meaning_phase(
        &self,
        input: &ReasoningInput,
        _context: &AssembleOutput,
        trace: &mut ReasoningTrace,
    ) -> Result<serde_json::Value, ReasoningError> {
        let start = std::time::Instant::now();
        let user_text = &input.user_text;

        let mut values: Vec<String> = self
            .life_model
            .identity
            .values
            .iter()
            .map(|v| v.name.clone())
            .collect();
        if values.is_empty() {
            values.push("成长".to_string());
            values.push("真诚".to_string());
        }

        let forbidden = self.detect_forbidden_topics(user_text);
        let risk_level = if forbidden.is_empty() { "low" } else { "high" };

        let aligned_values = values
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let text = if forbidden.is_empty() {
            format!("本次回应应体现以下核心价值观: {}", aligned_values)
        } else {
            format!(
                "本次请求触及敏感主题: {}。应以关怀、不评判、鼓励寻求专业帮助的方式回应，同时体现核心价值观: {}",
                forbidden.join(", "),
                aligned_values
            )
        };

        let result = json!({
            "text": text,
            "forbidden_keywords": forbidden,
            "aligned_values": values,
            "risk_level": risk_level,
        });

        let elapsed_ms = start.elapsed().as_millis() as u64;
        trace
            .layer_timings_ms
            .insert("Meaning".to_string(), elapsed_ms);
        trace.set_layer_result(ReasoningPhaseKind::Meaning, result.clone());

        Ok(result)
    }

    fn detect_forbidden_topics(&self, user_text: &str) -> Vec<String> {
        let mut forbidden = Vec::new();
        let lower = user_text.to_lowercase();
        let sensitive = vec![
            ("赌博", "赌博"),
            ("毒品", "毒品"),
            ("自杀", "自杀"),
            ("自残", "自残"),
            ("暴力", "暴力"),
            ("诈骗", "诈骗"),
            ("色情", "色情"),
        ];
        for (keyword, label) in sensitive {
            if lower.contains(keyword) {
                forbidden.push(label.to_string());
            }
        }
        forbidden
    }

    async fn run_strategy_phase(
        &self,
        input: &ReasoningInput,
        context: &AssembleOutput,
        _meaning: &serde_json::Value,
        trace: &mut ReasoningTrace,
    ) -> Result<serde_json::Value, ReasoningError> {
        let start = std::time::Instant::now();
        let user_text = &input.user_text;

        let mut prompt = format!(
            "你是 OpenLife 的策略规划层。请基于用户的人生模型目标，为以下请求制定回应策略。\n\n请求方法: {}\n用户输入: {}\n",
            input.task_kind, user_text
        );

        let active_goals: Vec<String> = self
            .life_model
            .goals
            .short_term
            .iter()
            .chain(self.life_model.goals.medium_term.iter())
            .chain(self.life_model.goals.long_term.iter())
            .chain(self.life_model.goals.life_goals.iter())
            .filter(|g| g.priority >= 3)
            .map(|g| format!("- [{}] {} (优先级{})", g.name, g.description, g.priority))
            .collect();
        if !active_goals.is_empty() {
            prompt.push_str("\n用户高优先级目标:\n");
            prompt.push_str(&active_goals.join("\n"));
            prompt.push('\n');
        }

        if !context.tools_prompt.is_empty() {
            prompt.push_str("\n可用工具: 是。若策略需要外部数据，请标记 needs_tools=true。");
        }

        if !context.memory_context.is_empty() {
            prompt.push_str("\n相关记忆: 存在。策略应保持一致性。");
        }

        prompt.push_str(
            "\n\n请用 JSON 输出策略（不要包含 markdown 代码块标记）:\n{\n  \"text\": \"策略描述（50字以内）\",\n  \"plan_steps\": [\"步骤1\", \"步骤2\"],\n  \"required_keywords\": [],\n  \"needs_tools\": false\n}\n"
        );

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }];

        let result = match self
            .scheduler
            .generate_raw(
                messages,
                Some("你是一个严谨的策略规划助手，只输出合法 JSON。"),
            )
            .await
        {
            Ok(raw) => {
                let cleaned = raw
                    .trim()
                    .trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();
                match serde_json::from_str::<serde_json::Value>(cleaned) {
                    Ok(mut v) => {
                        if v.get("text").is_none() {
                            v["text"] = json!("继续作为人生伴侣进行深度对话。");
                        }
                        if v.get("plan_steps").is_none() {
                            v["plan_steps"] = json!(["先理解问题，再给出下一步建议"]);
                        }
                        if v.get("needs_tools").is_none() {
                            v["needs_tools"] = json!(false);
                        }
                        if v.get("suggested_tools").is_none() {
                            v["suggested_tools"] = json!([]);
                        }
                        if v.get("conflict_flags").is_none() {
                            v["conflict_flags"] = json!([]);
                        }
                        v
                    }
                    Err(_) => self.fallback_strategy(user_text),
                }
            }
            Err(_) => self.fallback_strategy(user_text),
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;
        trace
            .layer_timings_ms
            .insert("Strategy".to_string(), elapsed_ms);
        trace.set_layer_result(ReasoningPhaseKind::Strategy, result.clone());

        Ok(result)
    }

    fn fallback_strategy(&self, user_text: &str) -> serde_json::Value {
        let mut plan_steps = vec![
            "先确认用户此刻最真实的目标与约束".to_string(),
            "结合人生模型给出一条可执行建议".to_string(),
        ];

        let mut required_keywords = vec!["下一步".to_string()];
        let mut aligned_goals = Vec::new();

        for goal in self
            .life_model
            .goals
            .short_term
            .iter()
            .chain(self.life_model.goals.medium_term.iter())
            .chain(self.life_model.goals.long_term.iter())
            .chain(self.life_model.goals.life_goals.iter())
            .filter(|g| g.priority >= 6)
            .take(3)
        {
            aligned_goals.push(goal.name.clone());
        }
        if !aligned_goals.is_empty() {
            plan_steps.insert(
                0,
                format!("优先围绕目标\"{}\"组织回应", aligned_goals.join("、")),
            );
            required_keywords.push(aligned_goals[0].clone());
        }

        let needs_tools = user_text.contains("搜索")
            || user_text.contains("查")
            || user_text.contains("文件")
            || user_text.to_lowercase().contains("search")
            || user_text.to_lowercase().contains("file");
        let suggested_tools = if needs_tools {
            if user_text.contains("文件") || user_text.to_lowercase().contains("file") {
                vec!["workspace.search".to_string()]
            } else {
                vec!["web.search".to_string()]
            }
        } else {
            vec![]
        };
        let conflict_flags = if user_text.contains("放弃") || user_text.contains("没意义") {
            vec!["motivation_drop".to_string()]
        } else {
            vec![]
        };

        if needs_tools {
            plan_steps.push("如果需要外部信息，明确说明将调用工具补充事实".to_string());
        }

        json!({
            "text": "围绕用户目标给出清晰、可执行、与人生模型一致的回应。",
            "plan_steps": plan_steps,
            "required_keywords": required_keywords,
            "aligned_goals": aligned_goals,
            "needs_tools": needs_tools,
            "suggested_tools": suggested_tools,
            "conflict_flags": conflict_flags,
            "response_style": if needs_tools { "investigative" } else { "coaching" },
        })
    }

    async fn run_generation_phase(
        &self,
        _input: &ReasoningInput,
        context: &AssembleOutput,
        meaning: &serde_json::Value,
        strategy: &serde_json::Value,
        trace: &mut ReasoningTrace,
    ) -> Result<serde_json::Value, ReasoningError> {
        let start = std::time::Instant::now();

        let mut system_parts = Vec::new();

        if let Some(text) = meaning.get("text").and_then(|t| t.as_str()) {
            system_parts.push(format!("【意义层约束】{}", text));
        }

        if let Some(text) = strategy.get("text").and_then(|t| t.as_str()) {
            system_parts.push(format!("【策略层约束】{}", text));
        }
        if let Some(goals) = strategy.get("aligned_goals").and_then(|v| v.as_array()) {
            let goals_text = goals
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("、");
            if !goals_text.is_empty() {
                system_parts.push(format!("【优先目标】{}", goals_text));
            }
        }
        if let Some(steps) = strategy.get("plan_steps").and_then(|s| s.as_array()) {
            if !steps.is_empty() {
                system_parts.push("【执行步骤】".to_string());
                for (i, step) in steps.iter().enumerate() {
                    if let Some(s) = step.as_str() {
                        system_parts.push(format!("{}. {}", i + 1, s));
                    }
                }
            }
        }

        let system_prompt = if system_parts.is_empty() {
            "你是 OpenLife，一位温暖而睿智的人生伴侣。".to_string()
        } else {
            system_parts.join("\n")
        };

        let mut messages = context.desensitized_messages.to_vec();
        if _input.task_kind != crate::agent::types::AgentTaskKind::Conversation {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: format!("当前调用方法: {:?}", _input.task_kind),
            });
        }

        let result = match self
            .scheduler
            .generate_raw(messages.to_vec(), Some(&system_prompt))
            .await
        {
            Ok(text) => json!({
                "text": text,
                "used_strategy": strategy.clone(),
            }),
            Err(e) => {
                return Err(ReasoningError {
                    phase: "generation".to_string(),
                    message: format!("Generation phase LLM call failed: {}", e),
                    recoverable: false,
                })
            }
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;
        trace
            .layer_timings_ms
            .insert("Generation".to_string(), elapsed_ms);
        trace.set_layer_result(ReasoningPhaseKind::Generation, result.clone());

        Ok(result)
    }

    fn build_system_prompt(
        &self,
        meaning: &serde_json::Value,
        strategy: &serde_json::Value,
    ) -> String {
        let mut parts = Vec::new();

        if let Some(text) = meaning.get("text").and_then(|t| t.as_str()) {
            parts.push(format!("【意义层约束】{}", text));
        }

        if let Some(text) = strategy.get("text").and_then(|t| t.as_str()) {
            parts.push(format!("【策略层约束】{}", text));
        }
        if let Some(goals) = strategy.get("aligned_goals").and_then(|v| v.as_array()) {
            let goals_text = goals
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("、");
            if !goals_text.is_empty() {
                parts.push(format!("【优先目标】{}", goals_text));
            }
        }
        if let Some(steps) = strategy.get("plan_steps").and_then(|s| s.as_array()) {
            if !steps.is_empty() {
                parts.push("【执行步骤】".to_string());
                for (i, step) in steps.iter().enumerate() {
                    if let Some(s) = step.as_str() {
                        parts.push(format!("{}. {}", i + 1, s));
                    }
                }
            }
        }

        if parts.is_empty() {
            "你是 OpenLife，一位温暖而睿智的人生伴侣。".to_string()
        } else {
            parts.join("\n")
        }
    }
}

/// Safety checker: validates generation output against meaning and strategy constraints.
pub struct SafetyChecker {
    strict_mode: bool,
}

#[derive(Debug, Clone)]
pub struct SafetyCheckResult {
    pub passed: bool,
    pub warnings: Vec<String>,
    pub strict_mode: bool,
}

impl SafetyChecker {
    pub fn new(strict_mode: bool) -> Self {
        Self { strict_mode }
    }

    pub fn check(&self, trace: &ReasoningTrace) -> SafetyCheckResult {
        let mut warnings = Vec::new();
        let execution = trace
            .generation_result
            .as_ref()
            .and_then(|v| v.get("text").and_then(|t| t.as_str()))
            .unwrap_or_default()
            .to_lowercase();

        // Check forbidden topics from meaning
        if let Some(ref meaning) = trace.meaning_result {
            if let Some(forbidden) = meaning.get("forbidden_keywords").and_then(|v| v.as_array()) {
                for kw in forbidden {
                    if let Some(k) = kw.as_str() {
                        if execution.contains(&k.to_lowercase()) && self.strict_mode {
                            warnings.push(format!("Execution contains forbidden keyword: {}", k));
                        }
                    }
                }
            }
        }

        // Check required keywords from strategy
        if let Some(ref strategy) = trace.strategy_result {
            if let Some(required) = strategy.get("required_keywords").and_then(|v| v.as_array()) {
                for kw in required {
                    if let Some(k) = kw.as_str() {
                        if !execution.contains(&k.to_lowercase()) {
                            warnings.push(format!("Execution missing required keyword: {}", k));
                        }
                    }
                }
            }

            // Check risk level support
            if let Some(ref meaning) = trace.meaning_result {
                if let Some(risk) = meaning.get("risk_level").and_then(|v| v.as_str()) {
                    if risk == "high" && !execution.contains("帮助") && !execution.contains("支持")
                    {
                        warnings.push(
                            "High-risk request response lacks support-oriented language"
                                .to_string(),
                        );
                    }
                }
            }
        }

        SafetyCheckResult {
            passed: warnings.is_empty(),
            warnings,
            strict_mode: self.strict_mode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safety_checker_pass() {
        let trace = ReasoningTrace {
            meaning_result: Some(json!({
                "forbidden_keywords": ["赌博"],
                "risk_level": "low"
            })),
            generation_result: Some(json!({
                "text": "这是一个正常的回复"
            })),
            ..Default::default()
        };

        let checker = SafetyChecker::new(true);
        let result = checker.check(&trace);
        assert!(result.passed);
    }

    #[test]
    fn test_safety_checker_forbidden() {
        let trace = ReasoningTrace {
            meaning_result: Some(json!({
                "forbidden_keywords": ["赌博"],
                "risk_level": "low"
            })),
            generation_result: Some(json!({
                "text": "我们来赌博吧"
            })),
            ..Default::default()
        };

        let checker = SafetyChecker::new(true);
        let result = checker.check(&trace);
        assert!(!result.passed);
        assert!(result.warnings.iter().any(|w| w.contains("赌博")));
    }
}
