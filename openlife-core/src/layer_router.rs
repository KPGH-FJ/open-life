use crate::router::Intent;

/// Processing layer according to ARCH-004
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Reflex layer: <50ms local direct response
    L1,
    /// Tactical layer: standard LLM with tools
    L2,
    /// Strategic layer: deep reasoning via Hermes
    L3,
}

/// Lightweight task complexity score
#[derive(Debug, Clone, Default)]
pub struct TaskComplexity {
    pub token_estimate: usize,
    pub sentence_count: usize,
    pub tool_keywords_count: usize,
    pub context_length: usize,
    pub score: u8, // 0-100
}

impl TaskComplexity {
    pub fn from_message(message: &str, context_len: usize) -> Self {
        let token_estimate = message.chars().count();
        let sentence_count = message
            .split(['。', '?', '？', '!', '\n'])
            .filter(|s| !s.trim().is_empty())
            .count();
        let tool_keywords = vec![
            "搜索",
            "查",
            "工具",
            "调用",
            "计算",
            "文件",
            "日历",
            "邮件",
            "schedule",
            "file",
            "search",
            "calculate",
        ];
        let tool_keywords_count = tool_keywords
            .iter()
            .filter(|kw| message.to_lowercase().contains(&kw.to_lowercase()))
            .count();

        let mut score: u8 = 0;
        if token_estimate > 80 {
            score += 15;
        }
        if token_estimate > 200 {
            score += 20;
        }
        if sentence_count > 2 {
            score += 15;
        }
        if sentence_count > 5 {
            score += 20;
        }
        score += (tool_keywords_count * 10).min(20) as u8;
        if context_len > 10 {
            score += 10;
        }

        Self {
            token_estimate,
            sentence_count,
            tool_keywords_count,
            context_length: context_len,
            score: score.min(100),
        }
    }
}

/// Layer router decides which layer should handle a message.
#[derive(Clone)]
pub struct LayerRouter;

impl LayerRouter {
    pub fn new() -> Self {
        Self
    }

    /// Resolve the target layer based on intent, message heuristics and task complexity.
    pub fn resolve(&self, intent: &Intent, message: &str) -> Layer {
        self.resolve_with_context(intent, message, 0)
    }

    pub fn resolve_with_context(
        &self,
        intent: &Intent,
        message: &str,
        context_len: usize,
    ) -> Layer {
        // Deep reasoning triggers: life model queries or value-aligned long messages
        if intent.needs_deep_reasoning() {
            return Layer::L3;
        }
        // Simple intents go to L1
        if intent.is_simple() {
            return Layer::L1;
        }

        let complexity = TaskComplexity::from_message(message, context_len);

        // High complexity -> L3
        if complexity.score >= 60 {
            return Layer::L3;
        }
        // Medium complexity or explicit tool need -> L2
        if complexity.score >= 30 || complexity.tool_keywords_count > 0 {
            return Layer::L2;
        }
        // Tool requests go to L2 immediately
        if matches!(intent, Intent::ToolRequest) {
            return Layer::L2;
        }

        // Default heuristic: long messages (>80 chars or multi-sentence) -> L3
        let len = message.chars().count();
        let sentence_count = message
            .split(['。', '?', '？', '!'])
            .count();
        if len > 80 || sentence_count > 2 {
            Layer::L3
        } else {
            Layer::L2
        }
    }

    /// Fallback chain: L3 -> L2 -> L1 -> None
    pub fn fallback(&self, layer: Layer) -> Option<Layer> {
        match layer {
            Layer::L3 => Some(Layer::L2),
            Layer::L2 => Some(Layer::L1),
            Layer::L1 => None,
        }
    }
}

impl Default for LayerRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::Intent;

    #[test]
    fn task_complexity_from_short_message() {
        let tc = TaskComplexity::from_message("你好", 0);
        assert_eq!(tc.token_estimate, 2);
        assert!(tc.score < 30);
    }

    #[test]
    fn task_complexity_from_long_message_with_tools() {
        let msg = "帮我搜索一下文件，然后计算一下结果。这是第一句。这是第二句。这是第三句。这是第四句。这是第五句。";
        let tc = TaskComplexity::from_message(msg, 12);
        assert!(tc.tool_keywords_count >= 2);
        assert!(tc.score >= 60, "expected score >= 60, got {}", tc.score);
    }

    #[test]
    fn layer_router_simple_intent_goes_l1() {
        let router = LayerRouter::new();
        assert_eq!(router.resolve(&Intent::Greeting, "hello"), Layer::L1);
        assert_eq!(router.resolve(&Intent::SmallTalk, "天气不错"), Layer::L1);
    }

    #[test]
    fn layer_router_complex_intent_goes_l2() {
        let router = LayerRouter::new();
        assert_eq!(
            router.resolve(&Intent::ToolRequest, "帮我查一下文件"),
            Layer::L2
        );
    }

    #[test]
    fn layer_router_deep_reasoning_goes_l3() {
        let router = LayerRouter::new();
        assert_eq!(
            router.resolve(&Intent::LifeModelQuery, "我的人生模型是怎样的"),
            Layer::L3
        );
    }

    #[test]
    fn layer_router_fallback_chain() {
        let router = LayerRouter::new();
        assert_eq!(router.fallback(Layer::L3), Some(Layer::L2));
        assert_eq!(router.fallback(Layer::L2), Some(Layer::L1));
        assert_eq!(router.fallback(Layer::L1), None);
    }

    #[test]
    fn layer_router_heuristic_long_message_to_l3() {
        let router = LayerRouter::new();
        let msg = "这是一个很长的问题。我想要规划未来五年的人生目标，并且分析当前的能力缺口。我还需要制定详细的行动计划，并评估潜在的风险和机会。请从多个维度给出建议。";
        let layer = router.resolve_with_context(&Intent::Complex, msg, 12);
        assert_eq!(layer, Layer::L3);
    }
}
