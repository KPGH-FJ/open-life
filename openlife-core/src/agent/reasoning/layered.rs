use crate::agent::context_assembler::AssembleOutput;
use crate::agent::reasoning::{
    ReasoningConfig, ReasoningError, ReasoningInput, ReasoningOutput, ReasoningPhaseKind,
    ReasoningStrategy, ReasoningTrace,
};
use crate::llm::{
    BoundedContextBlock, ChatMessage, ContextManifest, PreparedProviderOutcome,
    PreparedProviderRequest, ProviderInvocationReceipt, ProviderLocalOnlyReason,
    ProviderPayloadPurpose, ProviderPolicyAuthorization,
};
use crate::privacy::PrivacyEngine;
use crate::scheduler::InferenceScheduler;
use serde_json::json;
// use tokio::time::{timeout, Duration};

/// Layered reasoning strategy: three-phase sequential reasoning.
/// Layered reasoning strategy with meaning, strategy, and generation phases.
pub struct LayeredReasoner {
    scheduler: InferenceScheduler,
    config: ReasoningConfig,
    network_policy: crate::config::NetworkPolicy,
    provider_authorization: ProviderPolicyAuthorization,
    provider_subject_text: Option<String>,
    privacy_engine: PrivacyEngine,
    policy_provenance_refs: Vec<crate::llm::ProviderPolicyProvenanceRef>,
}

impl LayeredReasoner {
    pub fn new(scheduler: InferenceScheduler) -> Self {
        Self {
            scheduler,
            config: ReasoningConfig::default(),
            network_policy: crate::config::NetworkPolicy::default(),
            provider_authorization: ProviderPolicyAuthorization::local_only_fail_closed(
                ProviderLocalOnlyReason::MissingCanonicalPolicy,
            ),
            provider_subject_text: None,
            privacy_engine: PrivacyEngine::new(),
            policy_provenance_refs: Vec::new(),
        }
    }

    pub fn with_config(scheduler: InferenceScheduler, config: ReasoningConfig) -> Self {
        Self {
            scheduler,
            config,
            network_policy: crate::config::NetworkPolicy::default(),
            provider_authorization: ProviderPolicyAuthorization::local_only_fail_closed(
                ProviderLocalOnlyReason::MissingCanonicalPolicy,
            ),
            provider_subject_text: None,
            privacy_engine: PrivacyEngine::new(),
            policy_provenance_refs: Vec::new(),
        }
    }

    pub fn with_network_policy(mut self, network_policy: crate::config::NetworkPolicy) -> Self {
        self.network_policy = network_policy;
        self
    }

    pub fn with_provider_policy_context(
        mut self,
        provider_authorization: ProviderPolicyAuthorization,
        policy_provenance_refs: Vec<crate::llm::ProviderPolicyProvenanceRef>,
    ) -> Self {
        self.provider_authorization = provider_authorization;
        self.policy_provenance_refs = policy_provenance_refs;
        self
    }

    pub fn with_provider_subject_text(mut self, provider_subject_text: String) -> Self {
        self.provider_subject_text = Some(provider_subject_text);
        self
    }

    pub fn with_privacy_engine(mut self, privacy_engine: PrivacyEngine) -> Self {
        self.privacy_engine = privacy_engine;
        self
    }

    fn prepare_phase_payload(
        &self,
        phase: &str,
        messages: Vec<ChatMessage>,
        system_prompt: &str,
    ) -> (Vec<ChatMessage>, Vec<BoundedContextBlock>, ContextManifest) {
        let mut context_blocks = vec![BoundedContextBlock {
            source_ref: format!("layered_reasoning.{phase}"),
            category: "reasoning_instruction".into(),
            content: system_prompt.to_string(),
        }];

        // One batch keeps placeholders unique across messages and context blocks.
        let raw_payload = messages
            .iter()
            .map(|message| message.content.clone())
            .chain(context_blocks.iter().map(|block| block.content.clone()))
            .collect::<Vec<_>>();
        let (masked_payload, _) = self.privacy_engine.desensitize_batch(&raw_payload);
        let message_count = messages.len();
        let messages = messages
            .into_iter()
            .zip(masked_payload.iter().take(message_count))
            .map(|(mut message, masked)| {
                message.content = masked.clone();
                message
            })
            .collect::<Vec<_>>();
        for (block, masked) in context_blocks
            .iter_mut()
            .zip(masked_payload.into_iter().skip(message_count))
        {
            block.content = masked;
        }

        let mut selected_context_refs = context_blocks
            .iter()
            .map(|block| block.source_ref.clone())
            .collect::<Vec<_>>();
        selected_context_refs.sort();
        selected_context_refs.dedup();
        let mut included_context_categories = context_blocks
            .iter()
            .map(|block| block.category.clone())
            .collect::<Vec<_>>();
        included_context_categories.sort();
        included_context_categories.dedup();
        let manifest = ContextManifest {
            request_id: uuid::Uuid::new_v4().to_string(),
            privacy_decision_id: self.provider_authorization.decision_id().to_string(),
            selected_context_refs,
            included_context_categories,
            declared_payload_categories: vec![
                crate::llm::ProviderPayloadCategory::RuntimeCompiledMessages,
            ],
            policy_provenance_refs: self.policy_provenance_refs.clone(),
            raw_life_model_included: false,
            raw_unbounded_memory_included: false,
        };
        (messages, context_blocks, manifest)
    }

    async fn prepare_phase_request(
        &self,
        phase: &str,
        messages: Vec<ChatMessage>,
        system_prompt: &str,
    ) -> anyhow::Result<PreparedProviderRequest> {
        let (messages, context_blocks, manifest) =
            self.prepare_phase_payload(phase, messages, system_prompt);
        let provider_authorization = self
            .provider_authorization
            .clone()
            .authorize_derived_payload(
                ProviderPayloadPurpose::LayeredReasoningPhase,
                self.provider_subject_text.as_deref().unwrap_or_default(),
                &messages,
                &context_blocks,
            )?;
        self.scheduler
            .prepare_chat_request_with_authorization(
                messages,
                context_blocks,
                manifest,
                provider_authorization,
                self.network_policy.clone(),
                false,
            )
            .await
    }

    async fn generate_phase(
        &self,
        phase: &str,
        messages: Vec<ChatMessage>,
        system_prompt: &str,
    ) -> anyhow::Result<PreparedProviderOutcome> {
        let prepared = self
            .prepare_phase_request(phase, messages, system_prompt)
            .await?;
        let outcome = self.scheduler.execute_prepared(prepared).await;
        self.scheduler.verify_prepared_outcome_receipt(&outcome)?;
        Ok(outcome)
    }

    fn attach_provider_receipt(
        mut phase_result: serde_json::Value,
        receipt: Option<ProviderInvocationReceipt>,
    ) -> serde_json::Value {
        if let Some(receipt) = receipt {
            phase_result["provider_invocation_receipt"] = json!(receipt);
        }
        phase_result
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

        let forbidden = self.detect_forbidden_topics(user_text);
        let risk_level = if forbidden.is_empty() { "low" } else { "high" };

        let text = if forbidden.is_empty() {
            "本次回应应与已选择并经过隐私过滤的个人指导保持一致。".to_string()
        } else {
            format!(
                "本次请求触及敏感主题: {}。应以关怀、不评判、鼓励寻求专业帮助的方式回应。",
                forbidden.join(", ")
            )
        };

        let result = json!({
            "text": text,
            "forbidden_keywords": forbidden,
            "personal_guidance_source": "context_manifest",
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
            .generate_phase(
                "strategy",
                messages,
                "你是一个严谨的策略规划助手，只输出合法 JSON。",
            )
            .await
        {
            Ok(outcome) => {
                let receipt = outcome.receipt;
                let phase_result = match outcome.result {
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
                Self::attach_provider_receipt(phase_result, receipt)
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

        let required_keywords = vec!["下一步".to_string()];
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
            "personal_guidance_source": "context_manifest",
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
            .generate_phase("generation", messages.to_vec(), &system_prompt)
            .await
        {
            Ok(outcome) => {
                let receipt = outcome.receipt;
                match outcome.result {
                    Ok(text) => Self::attach_provider_receipt(
                        json!({
                            "text": text,
                            "used_strategy": strategy.clone(),
                        }),
                        receipt,
                    ),
                    Err(error) => {
                        if let Some(receipt) = receipt {
                            trace.set_layer_result(
                                ReasoningPhaseKind::Generation,
                                json!({ "provider_invocation_receipt": receipt }),
                            );
                        }
                        return Err(ReasoningError {
                            phase: "generation".to_string(),
                            message: format!("Generation phase LLM call failed: {error}"),
                            recoverable: false,
                        });
                    }
                }
            }
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

    #[tokio::test]
    async fn layered_local_only_route_is_bound_before_provider_execution() {
        let scheduler = InferenceScheduler::new(
            "local-model".into(),
            false,
            "openai".into(),
            "https://capture.invalid/v1".into(),
            "sk-capture".into(),
            "cloud-model".into(),
            String::new(),
            false,
        )
        .with_scripted_generation_response("fixture");
        let reasoner = LayeredReasoner::new(scheduler).with_provider_policy_context(
            crate::llm::ProviderPolicyAuthorization::local_only_fail_closed(
                crate::llm::ProviderLocalOnlyReason::TestFixture,
            ),
            Vec::new(),
        );

        let prepared = reasoner
            .prepare_phase_request(
                "strategy",
                vec![ChatMessage {
                    role: "user".into(),
                    content: "plan".into(),
                }],
                "instruction",
            )
            .await
            .unwrap();

        assert_eq!(
            prepared.data_route,
            crate::llm::ProviderDataRoute::LocalOnly
        );
        assert_eq!(prepared.provider_target, "ollama");
        assert_eq!(prepared.model_target, "local-model");
    }

    #[test]
    fn layered_reasoning_has_no_implicit_lifemodel_context() {
        let scheduler = InferenceScheduler::new(
            "local-model".into(),
            true,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "cloud-model".into(),
            String::new(),
            false,
        )
        .with_scripted_generation_response("fixture");
        let reasoner = LayeredReasoner::new(scheduler);

        let (messages, blocks, manifest) = reasoner.prepare_phase_payload(
            "strategy",
            vec![ChatMessage {
                role: "user".into(),
                content: "My email is user@example.com".into(),
            }],
            "Use only the explicit task context.",
        );
        let serialized = serde_json::to_string(&(messages, &blocks)).unwrap();

        assert!(!serialized.contains("user@example.com"));
        assert!(serialized.contains("<EMAIL_"));
        assert!(!manifest
            .included_context_categories
            .iter()
            .any(|category| category.contains("life")));
        assert!(!manifest
            .selected_context_refs
            .iter()
            .any(|source| source.contains("life_model")));
        assert!(!manifest.raw_life_model_included);
    }

    #[test]
    fn layered_phase_trace_retains_the_typed_provider_receipt() {
        let started_at = chrono::Utc::now();
        let receipt = ProviderInvocationReceipt {
            request_id: "layered-request".into(),
            provider: "ollama".into(),
            model: "local-model".into(),
            status: crate::llm::ProviderInvocationStatus::Completed,
            started_at,
            finished_at: started_at,
            error_digest: None,
            simulated: false,
            policy_evidence: None,
        };

        let traced = LayeredReasoner::attach_provider_receipt(
            json!({ "text": "bounded result" }),
            Some(receipt.clone()),
        );

        assert_eq!(
            serde_json::from_value::<ProviderInvocationReceipt>(
                traced["provider_invocation_receipt"].clone()
            )
            .unwrap(),
            receipt
        );
    }

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
