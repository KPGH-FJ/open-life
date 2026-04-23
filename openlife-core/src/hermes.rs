use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::scheduler::InferenceScheduler;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{timeout, Duration};

/// Hermes 协议：JSON-RPC 2.0 风格的进程内消息总线
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesRequest {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

impl HermesRequest {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: Some(rand::random::<u64>()),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    #[serde(flatten)]
    pub body: HermesResponseBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HermesResponseBody {
    Result { result: serde_json::Value },
    Error { error: HermesError },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// 三层节点定义
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HermesLayer {
    Meaning,
    Strategy,
    Execution,
}

impl HermesLayer {
    pub fn timeout_ms(&self) -> u64 {
        match self {
            HermesLayer::Meaning => 500,
            HermesLayer::Strategy => 2000,
            HermesLayer::Execution => 5000,
        }
    }
}

/// 每层节点的抽象接口
#[async_trait::async_trait]
pub trait HermesNode: Send + Sync {
    fn layer(&self) -> HermesLayer;
    async fn handle(
        &self,
        req: &HermesRequest,
        ctx: &HermesContext,
    ) -> Result<serde_json::Value, String>;
}

/// 上下文：包含人生模型摘要、对话历史、可用工具提示等
#[derive(Debug, Clone, Default)]
pub struct HermesContext {
    pub life_model_yaml: String,
    pub life_model: Option<LifeModel>,
    pub recent_messages: Vec<ChatMessage>,
    pub tools_prompt: Option<String>,
    pub memory_context: String,
    pub extras: HashMap<String, String>,
    pub trace: HermesTrace,
}

impl HermesContext {
    pub fn extract_user_text(&self) -> String {
        self.recent_messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default()
    }
}

/// 进程内总线：负责请求路由与层间传递
pub struct HermesBus {
    nodes: Vec<Box<dyn HermesNode>>,
    max_retries: u32,
}

impl HermesBus {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            max_retries: 1,
        }
    }

    pub fn with_retries(max_retries: u32) -> Self {
        Self {
            nodes: Vec::new(),
            max_retries,
        }
    }

    pub fn register<N: HermesNode + 'static>(&mut self, node: N) {
        self.nodes.push(Box::new(node));
    }

    /// 顺序执行 Meaning -> Strategy -> Execution，并收集每层的输出
    pub async fn dispatch(
        &self,
        req: &HermesRequest,
        ctx: &mut HermesContext,
    ) -> Result<HermesTrace, String> {
        let mut trace = HermesTrace::default();
        trace.input = req
            .params
            .as_ref()
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        for layer in [
            HermesLayer::Meaning,
            HermesLayer::Strategy,
            HermesLayer::Execution,
        ] {
            if let Some(node) = self.nodes.iter().find(|n| n.layer() == layer) {
                let start = std::time::Instant::now();
                let mut last_error = None;
                let mut result: Option<serde_json::Value> = None;

                for attempt in 0..=self.max_retries {
                    match timeout(
                        Duration::from_millis(layer.timeout_ms()),
                        node.handle(req, ctx),
                    )
                    .await
                    {
                        Ok(Ok(res)) => {
                            result = Some(res);
                            break;
                        }
                        Ok(Err(e)) => {
                            last_error = Some(e.clone());
                            if attempt < self.max_retries {
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                        }
                        Err(_) => {
                            last_error =
                                Some(format!("{:?} 层超时 ({} ms)", layer, layer.timeout_ms()));
                            if attempt < self.max_retries {
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                        }
                    }
                }

                let elapsed_ms = start.elapsed().as_millis() as u64;
                trace
                    .layer_timings_ms
                    .insert(format!("{:?}", layer), elapsed_ms);

                if let Some(res) = result {
                    trace.set_layer_result(layer, res);
                    ctx.trace = trace.clone();
                } else if let Some(e) = last_error {
                    trace.set_layer_error(layer, e.clone());
                    return Err(e);
                }
            }
        }

        trace.output = trace
            .execution_result
            .as_ref()
            .and_then(|v| {
                v.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            })
            .or_else(|| {
                trace
                    .execution_result
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });
        Ok(trace)
    }
}

impl Default for HermesBus {
    fn default() -> Self {
        Self::new()
    }
}

/// 记录每层输出，供仲裁器与前端展示
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HermesTrace {
    pub input: Option<String>,
    pub meaning_result: Option<serde_json::Value>,
    pub strategy_result: Option<serde_json::Value>,
    pub execution_result: Option<serde_json::Value>,
    pub output: Option<String>,
    pub errors: Vec<String>,
    #[serde(default)]
    pub tool_plan: Vec<String>,
    pub arbitration_result: Option<serde_json::Value>,
    #[serde(default)]
    pub layer_timings_ms: HashMap<String, u64>,
    /// 稳定步骤计划：从 Strategy 层提取，供 Execution 层遵循并供前端展示
    #[serde(default)]
    pub stable_steps: Vec<String>,
}

impl HermesTrace {
    pub fn set_layer_result(&mut self, layer: HermesLayer, value: serde_json::Value) {
        match layer {
            HermesLayer::Meaning => self.meaning_result = Some(value),
            HermesLayer::Strategy => self.strategy_result = Some(value),
            HermesLayer::Execution => self.execution_result = Some(value),
        }
    }

    pub fn set_layer_error(&mut self, layer: HermesLayer, error: String) {
        self.errors.push(format!("{:?}: {}", layer, error));
    }

    pub fn to_markdown(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref v) = self.meaning_result {
            if let Some(text) = v.as_str() {
                parts.push(format!("**Meaning**: {}", text));
            } else if let Some(text) = v.get("text").and_then(|t| t.as_str()) {
                parts.push(format!("**Meaning**: {}", text));
            }
        }
        if let Some(ref v) = self.strategy_result {
            if let Some(text) = v.as_str() {
                parts.push(format!("**Strategy**: {}", text));
            } else if let Some(text) = v.get("text").and_then(|t| t.as_str()) {
                parts.push(format!("**Strategy**: {}", text));
            }
        }
        if !self.tool_plan.is_empty() {
            parts.push(format!("**Tools**: {}", self.tool_plan.join(", ")));
        }
        if !self.errors.is_empty() {
            parts.push(format!("**Errors**: {}", self.errors.join("; ")));
        }
        parts.join("\n")
    }
}

/// 冲突仲裁器：检查 Execution 是否偏离 Strategy / Meaning
pub struct Arbitrator {
    pub strict_mode: bool,
}

impl Arbitrator {
    pub fn new(strict_mode: bool) -> Self {
        Self { strict_mode }
    }

    /// 简单仲裁：检查 execution 文本是否包含 strategy 要求的关键词约束
    pub fn arbitrate(&self, trace: &HermesTrace) -> Result<String, String> {
        let execution = trace
            .execution_result
            .as_ref()
            .and_then(|v| v.get("text").and_then(|t| t.as_str()))
            .ok_or("Execution 层无输出")?;

        // 若 Meaning 层给出了禁止主题，检查 execution 是否包含
        if let Some(ref meaning) = trace.meaning_result {
            if let Some(forbidden) = meaning.get("forbidden_keywords").and_then(|v| v.as_array()) {
                for kw in forbidden {
                    if let Some(k) = kw.as_str() {
                        if execution.to_lowercase().contains(&k.to_lowercase()) {
                            if self.strict_mode {
                                return Err(format!("Execution 包含禁止关键词: {}", k));
                            }
                        }
                    }
                }
            }
        }

        // 若 Strategy 层给出了必须包含的关键词
        if let Some(ref strategy) = trace.strategy_result {
            if let Some(required) = strategy.get("required_keywords").and_then(|v| v.as_array()) {
                for kw in required {
                    if let Some(k) = kw.as_str() {
                        if !execution.to_lowercase().contains(&k.to_lowercase()) {
                            if self.strict_mode {
                                return Err(format!("Execution 未包含必须关键词: {}", k));
                            }
                        }
                    }
                }
            }
        }

        Ok(execution.to_string())
    }

    pub fn inspect(&self, trace: &HermesTrace) -> serde_json::Value {
        let mut warnings = Vec::new();
        let execution = trace
            .execution_result
            .as_ref()
            .and_then(|v| v.get("text").and_then(|t| t.as_str()))
            .unwrap_or_default()
            .to_lowercase();

        if let Some(ref meaning) = trace.meaning_result {
            if let Some(risk) = meaning.get("risk_level").and_then(|v| v.as_str()) {
                if risk == "high" && !execution.contains("帮助") && !execution.contains("支持")
                {
                    warnings.push("高风险请求的回应缺少明确的支持/求助导向".to_string());
                }
            }
        }

        if let Some(ref strategy) = trace.strategy_result {
            if let Some(required) = strategy.get("required_keywords").and_then(|v| v.as_array()) {
                for kw in required {
                    if let Some(k) = kw.as_str() {
                        if !execution.contains(&k.to_lowercase()) {
                            warnings.push(format!("执行结果未覆盖策略要求关键词：{}", k));
                        }
                    }
                }
            }
        }

        serde_json::json!({
            "passed": warnings.is_empty(),
            "warnings": warnings,
            "strict_mode": self.strict_mode,
        })
    }
}

// ========================================
// 内置三层节点实现（工程化版）
// ========================================

pub struct MeaningNode {
    life_model: LifeModel,
}

impl MeaningNode {
    pub fn new(life_model: LifeModel) -> Self {
        Self { life_model }
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
}

#[async_trait::async_trait]
impl HermesNode for MeaningNode {
    fn layer(&self) -> HermesLayer {
        HermesLayer::Meaning
    }

    async fn handle(
        &self,
        req: &HermesRequest,
        ctx: &HermesContext,
    ) -> Result<serde_json::Value, String> {
        let user_text = ctx.extract_user_text();
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

        let forbidden = self.detect_forbidden_topics(&user_text);
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

        Ok(serde_json::json!({
            "text": text,
            "forbidden_keywords": forbidden,
            "aligned_values": values,
            "risk_level": risk_level,
            "method": req.method,
        }))
    }
}

pub struct StrategyNode {
    scheduler: InferenceScheduler,
}

impl StrategyNode {
    pub fn new(scheduler: InferenceScheduler) -> Self {
        Self { scheduler }
    }

    fn build_strategy_prompt(&self, req: &HermesRequest, ctx: &HermesContext) -> String {
        let method = req.method.as_str();
        let user_text = ctx.extract_user_text();

        let mut prompt = format!(
            "你是 OpenLife 的策略规划层。请基于用户的人生模型目标，为以下请求制定回应策略。\n\n请求方法: {}\n用户输入: {}\n",
            method, user_text
        );

        if let Some(ref lm) = ctx.life_model {
            let active_goals: Vec<String> = lm
                .goals
                .short_term
                .iter()
                .chain(lm.goals.medium_term.iter())
                .chain(lm.goals.long_term.iter())
                .chain(lm.goals.life_goals.iter())
                .filter(|g| g.priority >= 3)
                .map(|g| format!("- [{}] {} (优先级{})", g.name, g.description, g.priority))
                .collect();
            if !active_goals.is_empty() {
                prompt.push_str("\n用户高优先级目标:\n");
                prompt.push_str(&active_goals.join("\n"));
                prompt.push('\n');
            }
        }

        if ctx.tools_prompt.is_some() {
            prompt.push_str("\n可用工具: 是。若策略需要外部数据，请标记 needs_tools=true。");
        }

        if !ctx.memory_context.is_empty() {
            prompt.push_str("\n相关记忆: 存在。策略应保持一致性。");
        }

        prompt.push_str(
            "\n\n请用 JSON 输出策略（不要包含 markdown 代码块标记）:\n{\n  \"text\": \"策略描述（50字以内）\",\n  \"plan_steps\": [\"步骤1\", \"步骤2\"],\n  \"required_keywords\": [],\n  \"needs_tools\": false\n}\n"
        );

        prompt
    }

    fn fallback_strategy(&self, ctx: &HermesContext) -> serde_json::Value {
        let user_text = ctx.extract_user_text();
        let mut plan_steps = vec![
            "先确认用户此刻最真实的目标与约束".to_string(),
            "结合人生模型给出一条可执行建议".to_string(),
        ];

        let mut required_keywords = vec!["下一步".to_string()];
        let mut aligned_goals = Vec::new();

        if let Some(ref lm) = ctx.life_model {
            for goal in lm
                .goals
                .short_term
                .iter()
                .chain(lm.goals.medium_term.iter())
                .chain(lm.goals.long_term.iter())
                .chain(lm.goals.life_goals.iter())
                .filter(|g| g.priority >= 6)
                .take(3)
            {
                aligned_goals.push(goal.name.clone());
            }
            if !aligned_goals.is_empty() {
                plan_steps.insert(
                    0,
                    format!("优先围绕目标“{}”组织回应", aligned_goals.join("、")),
                );
                required_keywords.push(aligned_goals[0].clone());
            }
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

        serde_json::json!({
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
}

#[async_trait::async_trait]
impl HermesNode for StrategyNode {
    fn layer(&self) -> HermesLayer {
        HermesLayer::Strategy
    }

    async fn handle(
        &self,
        req: &HermesRequest,
        ctx: &HermesContext,
    ) -> Result<serde_json::Value, String> {
        let system_prompt = self.build_strategy_prompt(req, ctx);
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: system_prompt,
        }];

        match self
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
                        if !v.get("text").is_some() {
                            v["text"] =
                                serde_json::Value::String("继续作为人生伴侣进行深度对话。".into());
                        }
                        if !v.get("plan_steps").is_some() {
                            v["plan_steps"] = serde_json::json!(["先理解问题，再给出下一步建议"]);
                        }
                        if !v.get("needs_tools").is_some() {
                            v["needs_tools"] = serde_json::Value::Bool(false);
                        }
                        if !v.get("suggested_tools").is_some() {
                            v["suggested_tools"] = serde_json::json!([]);
                        }
                        if !v.get("conflict_flags").is_some() {
                            v["conflict_flags"] = serde_json::json!([]);
                        }
                        Ok(v)
                    }
                    Err(_) => Ok(self.fallback_strategy(ctx)),
                }
            }
            Err(_) => Ok(self.fallback_strategy(ctx)),
        }
    }
}

pub struct ExecutionNode {
    scheduler: InferenceScheduler,
}

impl ExecutionNode {
    pub fn new(scheduler: InferenceScheduler) -> Self {
        Self { scheduler }
    }

    fn build_execution_prompt(
        &self,
        req: &HermesRequest,
        ctx: &HermesContext,
    ) -> (String, Vec<ChatMessage>) {
        let mut system_parts = Vec::new();

        if let Some(ref meaning) = ctx.trace.meaning_result {
            if let Some(text) = meaning.get("text").and_then(|t| t.as_str()) {
                system_parts.push(format!("【意义层约束】{}", text));
            }
        }

        if let Some(ref strategy) = ctx.trace.strategy_result {
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
        }

        let system_prompt = if system_parts.is_empty() {
            "你是 OpenLife，一位温暖而睿智的人生伴侣。".to_string()
        } else {
            system_parts.join("\n")
        };

        let mut messages = ctx.recent_messages.clone();
        // 如果 method 不是 chat，在末尾附加方法提示
        if req.method != "chat" {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: format!("当前调用方法: {}", req.method),
            });
        }

        (system_prompt, messages)
    }
}

#[async_trait::async_trait]
impl HermesNode for ExecutionNode {
    fn layer(&self) -> HermesLayer {
        HermesLayer::Execution
    }

    async fn handle(
        &self,
        req: &HermesRequest,
        ctx: &HermesContext,
    ) -> Result<serde_json::Value, String> {
        let (system_prompt, messages) = self.build_execution_prompt(req, ctx);

        match self
            .scheduler
            .generate_raw(messages, Some(&system_prompt))
            .await
        {
            Ok(text) => Ok(serde_json::json!({
                "text": text,
                "used_strategy": ctx.trace.strategy_result.clone(),
            })),
            Err(e) => Err(format!("ExecutionNode LLM 调用失败: {}", e)),
        }
    }
}

/// 便捷函数：构造一个带结构化依赖注入的 Bus
pub fn build_bus(life_model: LifeModel, scheduler: InferenceScheduler) -> HermesBus {
    let mut bus = HermesBus::new();
    bus.register(MeaningNode::new(life_model));
    bus.register(StrategyNode::new(scheduler.clone()));
    bus.register(ExecutionNode::new(scheduler));
    bus
}

impl HermesBus {
    pub async fn dispatch_with_arbitration(
        &self,
        req: &HermesRequest,
        ctx: &mut HermesContext,
    ) -> Result<HermesTrace, String> {
        let mut trace = self.dispatch(req, ctx).await?;
        if let Some(ref strategy) = trace.strategy_result {
            trace.tool_plan = strategy
                .get("suggested_tools")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            // 提取稳定步骤计划
            let steps: Vec<String> = strategy
                .get("plan_steps")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            trace.stable_steps = Self::normalize_stable_steps(&steps, &trace);
        }
        let arbitrator = Arbitrator::new(false);
        trace.arbitration_result = Some(arbitrator.inspect(&trace));
        Ok(trace)
    }

    /// 对策略步骤进行规范化：去重、去空、限制长度、避免与执行输出冲突
    fn normalize_stable_steps(raw: &[String], trace: &HermesTrace) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for s in raw {
            let t = s.trim();
            if t.is_empty() || seen.contains(t) {
                continue;
            }
            // 若执行输出已经明确包含该步骤描述，则不再重复注入
            if let Some(exec) = trace
                .execution_result
                .as_ref()
                .and_then(|v| v.get("text").and_then(|t| t.as_str()))
            {
                if exec.contains(t) {
                    continue;
                }
            }
            seen.insert(t.to_string());
            out.push(t.to_string());
            if out.len() >= 6 {
                break;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_stable_steps_dedup_and_trim() {
        let trace = HermesTrace::default();
        let raw = vec![
            "  第一步  ".into(),
            "第一步".into(),
            "".into(),
            "第二步".into(),
            "第三步".into(),
            "第四步".into(),
            "第五步".into(),
            "第六步".into(),
            "第七步".into(),
        ];
        let out = HermesBus::normalize_stable_steps(&raw, &trace);
        assert_eq!(out.len(), 6);
        assert_eq!(out[0], "第一步");
        assert_eq!(out[1], "第二步");
    }

    #[test]
    fn normalize_stable_steps_skips_when_in_execution() {
        let mut trace = HermesTrace::default();
        trace.execution_result = Some(serde_json::json!({
            "text": "回复中已经包含了第一步",
        }));
        let raw = vec!["第一步".into(), "第二步".into()];
        let out = HermesBus::normalize_stable_steps(&raw, &trace);
        assert_eq!(out, vec!["第二步"]);
    }

    #[test]
    fn hermes_trace_stable_steps_default_empty() {
        let trace = HermesTrace::default();
        assert!(trace.stable_steps.is_empty());
    }
}

/// 兼容旧接口的默认 Bus（使用最小占位实现，供不需要依赖注入的场景）
pub fn build_default_bus() -> HermesBus {
    let mut bus = HermesBus::new();
    bus.register(MeaningNode::new(LifeModel::default_model()));
    bus.register(StrategyNode::new(InferenceScheduler::default()));
    bus.register(ExecutionNode::new(InferenceScheduler::default()));
    bus
}
