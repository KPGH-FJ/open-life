use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Intent classification result
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Intent {
    Greeting,
    Goodbye,
    Help,
    LifeModelQuery,
    SmallTalk,
    ToolRequest,
    Sensitive,
    Complex,
}

impl Intent {
    pub fn is_simple(&self) -> bool {
        matches!(
            self,
            Intent::Greeting | Intent::Goodbye | Intent::Help | Intent::SmallTalk
        )
    }

    pub fn is_complex(&self) -> bool {
        matches!(self, Intent::ToolRequest)
    }

    pub fn needs_deep_reasoning(&self) -> bool {
        matches!(self, Intent::LifeModelQuery | Intent::Sensitive)
    }

    pub fn direct_response(&self) -> Option<String> {
        match self {
            Intent::Greeting => {
                Some("你好！我是 OpenLife，很高兴陪伴你的成长。今天想聊聊什么？".into())
            }
            Intent::Goodbye => Some("再见！随时欢迎回来，我会一直在这里支持你。".into()),
            Intent::Help => Some(
                "你可以跟我聊人生目标、价值观、当前状态，也可以让我帮你调用工具完成任务。".into(),
            ),
            Intent::SmallTalk => Some("嗯，我在听呢。有什么我可以帮你的吗？".into()),
            _ => None,
        }
    }
}

/// Hybrid intent router (ONNX + rule-based fallback)
#[derive(Clone)]
pub struct IntentRouter {
    patterns: HashMap<Intent, Vec<Regex>>,
    privacy_patterns: Vec<Regex>,
    values_keywords: Vec<String>,
    reflex: Option<Arc<crate::reflex_engine::ReflexEngine>>,
    onnx_disabled: Arc<AtomicBool>,
    latency_threshold_us: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterStatus {
    pub onnx_available: bool,
    pub onnx_disabled: bool,
    pub active_backend: String,
    pub latency_threshold_us: u64,
}

impl IntentRouter {
    pub fn new() -> Self {
        Self::with_optional_onnx(None)
    }

    pub fn with_optional_onnx(model_dir: Option<&Path>) -> Self {
        let mut patterns: HashMap<Intent, Vec<Regex>> = HashMap::new();

        patterns.insert(
            Intent::Greeting,
            vec![Regex::new(r"^(你好|您好|哈喽|嗨|hello|hi|hey)[^\n]*$")
                .expect("invalid regex for greeting")],
        );
        patterns.insert(
            Intent::Goodbye,
            vec![Regex::new(r"^(再见|拜拜|bye|goodbye|see you)[^\n]*$")
                .expect("invalid regex for goodbye")],
        );
        patterns.insert(
            Intent::Help,
            vec![
                Regex::new(r"^(帮助|help|怎么用|你能做什么|你是什么)[^\n]*$")
                    .expect("invalid regex for help"),
            ],
        );
        patterns.insert(
            Intent::LifeModelQuery,
            vec![
                Regex::new(r"我的人生模型|我的价值观|我的目标|life model|values|goals")
                    .expect("invalid regex for lifemodel"),
            ],
        );
        patterns.insert(
            Intent::ToolRequest,
            vec![Regex::new(
                r"帮我(查|找|写|读|创建|删除|计算|搜索)|用工具|tool|fetch|file|read|write",
            )
            .expect("invalid regex for tool")],
        );

        let privacy_patterns = vec![
            Regex::new(r"\b1[3-9]\d{9}\b").expect("phone regex"), // phone
            Regex::new(r"\b\d{17}[\dXx]\b").expect("idcard regex"), // id card
            Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b")
                .expect("email regex"), // email
            Regex::new(r"\b\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b").expect("bankcard regex"), // bank card
        ];

        let values_keywords = vec![
            "成长".into(),
            "健康".into(),
            "家庭".into(),
            "自由".into(),
            "意义".into(),
            "创造".into(),
        ];

        let reflex = model_dir.and_then(|dir| {
            let tokenizer = dir.join("tokenizer.json");
            let labels: Vec<String> = vec![
                "greeting".into(),
                "goodbye".into(),
                "help".into(),
                "life_model_query".into(),
                "small_talk".into(),
                "tool_request".into(),
                "sensitive".into(),
                "complex".into(),
            ];
            crate::reflex_engine::ReflexEngine::new_with_optional_quantization(
                dir, &tokenizer, labels,
            )
            .ok()
            .map(Arc::new)
        });

        Self {
            patterns,
            privacy_patterns,
            values_keywords,
            reflex,
            onnx_disabled: Arc::new(AtomicBool::new(false)),
            latency_threshold_us: 50_000, // 50ms
        }
    }

    /// Temporarily disable ONNX fallback (e.g. after high latency).
    pub fn disable_onnx(&self) {
        self.onnx_disabled.store(true, Ordering::Relaxed);
    }

    /// Re-enable ONNX fallback.
    pub fn enable_onnx(&self) {
        self.onnx_disabled.store(false, Ordering::Relaxed);
    }

    /// Classify user message into an intent (target <50ms, synchronous).
    /// Tries ONNX first, falls back to regex rules.
    /// Monitors latency and auto-disables ONNX if threshold is exceeded.
    pub fn classify(&self, message: &str) -> Intent {
        if let Some(ref engine) = self.reflex {
            if !self.onnx_disabled.load(Ordering::Relaxed) {
                match engine.classify(message) {
                    Ok((intent, latency)) => {
                        if latency > self.latency_threshold_us {
                            self.onnx_disabled.store(true, Ordering::Relaxed);
                            eprintln!(
                                "[IntentRouter] ONNX latency {}us > threshold {}us, disabling ONNX",
                                latency, self.latency_threshold_us
                            );
                        } else {
                            return intent;
                        }
                    }
                    Err(e) => {
                        eprintln!("[IntentRouter] ONNX inference failed: {}, falling back to regex - router.rs:178", e);
                    }
                }
            }
        }
        let lower = message.to_lowercase();
        for (intent, regexes) in &self.patterns {
            for re in regexes {
                if re.is_match(&lower) {
                    return intent.clone();
                }
            }
        }
        Intent::Complex
    }

    /// Detect PII in the message
    pub fn detect_privacy(&self, message: &str) -> Vec<String> {
        let mut findings = Vec::new();
        for re in &self.privacy_patterns {
            for cap in re.find_iter(message) {
                findings.push(cap.as_str().to_string());
            }
        }
        findings
    }

    /// Check if message contains value-aligned keywords
    pub fn values_filter(&self, message: &str) -> bool {
        self.values_keywords.iter().any(|kw| message.contains(kw))
    }

    /// Full routing decision: runs classify and privacy detection in parallel.
    pub fn route(&self, message: &str) -> (Intent, Vec<String>) {
        let mut privacy_issues = Vec::new();
        let mut intent = Intent::Complex;
        std::thread::scope(|s| {
            let t_privacy = s.spawn(|| self.detect_privacy(message));
            let t_intent = s.spawn(|| self.classify(message));
            privacy_issues = t_privacy.join().unwrap_or_default();
            intent = t_intent.join().unwrap_or(Intent::Complex);
        });
        (intent, privacy_issues)
    }

    pub fn status(&self) -> RouterStatus {
        let onnx_available = self.reflex.is_some();
        let onnx_disabled = self.onnx_disabled.load(Ordering::Relaxed);
        RouterStatus {
            onnx_available,
            onnx_disabled,
            active_backend: if onnx_available && !onnx_disabled {
                "onnx".into()
            } else {
                "regex".into()
            },
            latency_threshold_us: self.latency_threshold_us,
        }
    }
}

impl Default for IntentRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_is_simple_and_complex() {
        assert!(Intent::Greeting.is_simple());
        assert!(Intent::SmallTalk.is_simple());
        assert!(!Intent::ToolRequest.is_simple());
        assert!(Intent::ToolRequest.is_complex());
        assert!(Intent::LifeModelQuery.needs_deep_reasoning());
    }

    #[test]
    fn intent_direct_response() {
        assert!(Intent::Greeting.direct_response().is_some());
        assert!(Intent::Help.direct_response().is_some());
        assert!(Intent::Complex.direct_response().is_none());
    }

    #[test]
    fn classify_greeting() {
        let router = IntentRouter::new();
        assert_eq!(router.classify("你好"), Intent::Greeting);
        assert_eq!(router.classify("hello"), Intent::Greeting);
    }

    #[test]
    fn classify_goodbye() {
        let router = IntentRouter::new();
        assert_eq!(router.classify("再见"), Intent::Goodbye);
        assert_eq!(router.classify("bye"), Intent::Goodbye);
    }

    #[test]
    fn classify_help() {
        let router = IntentRouter::new();
        assert_eq!(router.classify("帮助"), Intent::Help);
        assert_eq!(router.classify("你能做什么"), Intent::Help);
    }

    #[test]
    fn classify_life_model_query() {
        let router = IntentRouter::new();
        assert_eq!(router.classify("我的人生模型"), Intent::LifeModelQuery);
        assert_eq!(router.classify("我的价值观是什么"), Intent::LifeModelQuery);
    }

    #[test]
    fn classify_tool_request() {
        let router = IntentRouter::new();
        assert_eq!(router.classify("帮我查天气"), Intent::ToolRequest);
        assert_eq!(router.classify("用工具fetch数据"), Intent::ToolRequest);
    }

    #[test]
    fn classify_fallback_to_complex() {
        let router = IntentRouter::new();
        assert_eq!(router.classify("随便说一句很长很长的话"), Intent::Complex);
    }

    #[test]
    fn detect_privacy_finds_phone_and_email() {
        let router = IntentRouter::new();
        let issues = router.detect_privacy("电话 13800138000，邮箱 a@b.com");
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn values_filter_matches_keywords() {
        let router = IntentRouter::new();
        assert!(router.values_filter("我重视家庭"));
        assert!(!router.values_filter("今天天气不错"));
    }

    #[test]
    fn route_returns_intent_and_privacy() {
        let router = IntentRouter::new();
        let (intent, issues) = router.route("帮我查一下 13800138000 的归属地");
        assert_eq!(intent, Intent::ToolRequest);
        assert!(!issues.is_empty());
    }

    #[test]
    fn disable_and_enable_onnx() {
        let router = IntentRouter::new();
        router.disable_onnx();
        assert!(router
            .onnx_disabled
            .load(std::sync::atomic::Ordering::Relaxed));
        router.enable_onnx();
        assert!(!router
            .onnx_disabled
            .load(std::sync::atomic::Ordering::Relaxed));
    }
}
