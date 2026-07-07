const CORE_VALUE_SIGNAL_KEYWORDS: &[&str] = &["成长", "健康", "家庭", "自由", "意义", "创造"];

/// Detects whether text mentions a broad value signal.
///
/// This is intentionally a stateless feature extractor, not a router. Product
/// route authority lives in IntentFrame + PolicyRouter.
pub fn contains_core_value_signal(message: &str) -> bool {
    CORE_VALUE_SIGNAL_KEYWORDS
        .iter()
        .any(|keyword| message.contains(keyword))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_core_value_signal_without_routing() {
        assert!(contains_core_value_signal("我重视家庭和健康"));
        assert!(!contains_core_value_signal("今天天气不错"));
    }
}
