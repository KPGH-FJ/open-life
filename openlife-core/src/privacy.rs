use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Global counter for privacy warn log sampling.
/// Fires 1 in every 100 unmatched rule warnings to prevent log flooding.
static PRIVACY_WARN_COUNT: AtomicUsize = AtomicUsize::new(0);
const PRIVACY_WARN_SAMPLE_RATE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrivacyType {
    Phone,
    IdCard,
    Email,
    BankCard,
    Address,
    Name,
    Generic,
}

impl PrivacyType {
    pub fn placeholder_prefix(&self) -> &'static str {
        match self {
            PrivacyType::Phone => "PHONE",
            PrivacyType::IdCard => "IDCARD",
            PrivacyType::Email => "EMAIL",
            PrivacyType::BankCard => "BANKCARD",
            PrivacyType::Address => "ADDRESS",
            PrivacyType::Name => "NAME",
            PrivacyType::Generic => "SENSITIVE",
        }
    }

    pub fn default_action(&self) -> PrivacyAction {
        match self {
            PrivacyType::IdCard | PrivacyType::BankCard => PrivacyAction::Block,
            PrivacyType::Phone | PrivacyType::Address | PrivacyType::Name => PrivacyAction::Mask,
            PrivacyType::Email | PrivacyType::Generic => PrivacyAction::Mask,
        }
    }
}

/// Action to take when sensitive data is detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyAction {
    /// Replace with placeholder (e.g. <PHONE_0>)
    Mask,
    /// Block the request entirely
    Block,
    /// Allow through without modification
    Allow,
}

/// Per-type configurable rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyRule {
    pub ptype: PrivacyType,
    pub enabled: bool,
    pub action: PrivacyAction,
    /// Optional custom regex override (empty = use default)
    pub custom_pattern: Option<String>,
}

impl Default for PrivacyRule {
    fn default() -> Self {
        Self {
            ptype: PrivacyType::Generic,
            enabled: true,
            action: PrivacyAction::Mask,
            custom_pattern: None,
        }
    }
}

/// Configurable privacy policy. Replaces hard-coded patterns with user-tunable rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    pub rules: Vec<PrivacyRule>,
    /// Global switch to disable all privacy processing
    pub enabled: bool,
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        use PrivacyType::*;
        Self {
            enabled: true,
            rules: vec![
                PrivacyRule {
                    ptype: Phone,
                    enabled: true,
                    action: PrivacyAction::Mask,
                    custom_pattern: None,
                },
                PrivacyRule {
                    ptype: IdCard,
                    enabled: true,
                    action: PrivacyAction::Block,
                    custom_pattern: None,
                },
                PrivacyRule {
                    ptype: Email,
                    enabled: true,
                    action: PrivacyAction::Mask,
                    custom_pattern: None,
                },
                PrivacyRule {
                    ptype: BankCard,
                    enabled: true,
                    action: PrivacyAction::Block,
                    custom_pattern: None,
                },
                PrivacyRule {
                    ptype: Address,
                    enabled: true,
                    action: PrivacyAction::Mask,
                    custom_pattern: None,
                },
                PrivacyRule {
                    ptype: Name,
                    enabled: true,
                    action: PrivacyAction::Mask,
                    custom_pattern: None,
                },
                PrivacyRule {
                    ptype: Generic,
                    enabled: false,
                    action: PrivacyAction::Mask,
                    custom_pattern: None,
                },
            ],
        }
    }
}

impl PrivacyPolicy {
    /// Load policy from YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        serde_yaml::from_str(yaml).map_err(|e| format!("parse privacy policy failed: {}", e))
    }

    /// Serialize to YAML.
    pub fn to_yaml(&self) -> Result<String, String> {
        serde_yaml::to_string(self).map_err(|e| format!("serialize privacy policy failed: {}", e))
    }

    /// Get the compiled regex for a rule (custom or default).
    fn compiled_pattern(&self, rule: &PrivacyRule) -> Option<Regex> {
        if let Some(ref custom) = rule.custom_pattern {
            if !custom.is_empty() {
                return Regex::new(custom).ok();
            }
        }
        default_regex(&rule.ptype)
    }

    /// Determine the action for a detected privacy type.
    pub fn action_for(&self, ptype: &PrivacyType) -> PrivacyAction {
        if !self.enabled {
            return PrivacyAction::Allow;
        }
        self.rules
            .iter()
            .find(|r| r.ptype == *ptype && r.enabled)
            .map(|r| r.action)
            .unwrap_or_else(|| {
                let count = PRIVACY_WARN_COUNT.fetch_add(1, Ordering::Relaxed);
                if count.is_multiple_of(PRIVACY_WARN_SAMPLE_RATE) {
                    log::warn!(
                        "[PrivacyEngine] No rule matched for privacy type {:?}, defaulting to Mask (fail-closed). \
                         This warning is sampled (1/{}). Total unmatched: {}",
                        ptype,
                        PRIVACY_WARN_SAMPLE_RATE,
                        count + 1
                    );
                }
                PrivacyAction::Mask
            })
    }
}

fn default_regex(ptype: &PrivacyType) -> Option<Regex> {
    match ptype {
        PrivacyType::Phone => Regex::new(r"1[3-9]\d{9}").ok(),
        PrivacyType::IdCard => Regex::new(r"\d{17}[\dXx]").ok(),
        PrivacyType::Email => Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").ok(),
        PrivacyType::BankCard => Regex::new(r"(?:\d{4}[\s-]?){3}\d{4}").ok(),
        PrivacyType::Address => Regex::new(r"([\u4e00-\u9fa5]{2,}(省|市|区|县|镇|乡|村|街|路|号|栋|楼|室|单元)){2,}").ok(),
        PrivacyType::Name => Regex::new(r"[李王张刘陈杨赵黄周吴徐孙胡朱高林何郭马罗梁宋郑谢韩唐冯于董萧程曹袁邓许傅沈曾彭吕苏卢蒋蔡贾丁魏薛叶阎余潘杜戴夏钟汪田任姜范方石姚谭廖邹熊金陆郝孔白崔康毛邱秦江史顾侯邵孟龙万段雷钱汤尹黎易常武乔贺赖龚文][\u4e00-\u9fa5]{1,2}").ok(),
        PrivacyType::Generic => None,
    }
}

/// Holds detected sensitive information and its mapping.
#[derive(Clone)]
pub struct PrivacyEngine {
    policy: PrivacyPolicy,
}

impl Default for PrivacyEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivacyEngine {
    pub fn new() -> Self {
        Self {
            policy: PrivacyPolicy::default(),
        }
    }

    pub fn with_policy(policy: PrivacyPolicy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> &PrivacyPolicy {
        &self.policy
    }

    pub fn set_policy(&mut self, policy: PrivacyPolicy) {
        self.policy = policy;
    }

    /// Detect all privacy-sensitive strings in a message, respecting policy rules.
    pub fn detect(&self, message: &str) -> Vec<(PrivacyType, String)> {
        if !self.policy.enabled {
            return Vec::new();
        }
        let mut findings = Vec::new();
        for rule in &self.policy.rules {
            if !rule.enabled {
                continue;
            }
            if let Some(re) = self.policy.compiled_pattern(rule) {
                for mat in re.find_iter(message) {
                    findings.push((rule.ptype.clone(), mat.as_str().to_string()));
                }
            }
        }
        findings
    }

    /// Desensitize a message, returning the masked text and a reconstruction map.
    /// This is the primary backward-compatible API.
    /// Block actions produce non-reconstructable BLOCKED placeholders.
    pub fn desensitize(&self, message: &str) -> (String, HashMap<String, String>) {
        if !self.policy.enabled {
            return (message.to_string(), HashMap::new());
        }

        let findings = self.detect(message);
        let mut text = message.to_string();
        let mut map = HashMap::new();
        let mut counters: HashMap<PrivacyType, usize> = HashMap::new();

        let mut sorted: Vec<_> = findings;
        sorted.sort_by_key(|item| Reverse(item.1.len()));

        for (ptype, original) in sorted {
            match self.policy.action_for(&ptype) {
                PrivacyAction::Allow => continue,
                PrivacyAction::Block => {
                    let count = counters.entry(ptype.clone()).or_insert(0);
                    let placeholder = format!("<BLOCKED_{}_{}>", ptype.placeholder_prefix(), count);
                    *count += 1;
                    text = text.replacen(&original, &placeholder, 1);
                }
                PrivacyAction::Mask => {
                    let count = counters.entry(ptype.clone()).or_insert(0);
                    let placeholder = format!("<{}_{}>", ptype.placeholder_prefix(), count);
                    *count += 1;

                    if !map.contains_key(&placeholder) {
                        map.insert(placeholder.clone(), original.clone());
                    }
                    text = text.replacen(&original, &placeholder, 1);
                }
            }
        }

        (text, map)
    }

    /// Redact only credential-like secrets while preserving ordinary personal context.
    /// The returned map intentionally does not contain raw secret values, so later
    /// reconstruction cannot rehydrate credentials into assistant output.
    pub fn desensitize_secrets_only(&self, message: &str) -> (String, HashMap<String, String>) {
        let mut text = message.to_string();
        let mut map = HashMap::new();
        let mut findings = secret_like_findings(message);
        findings.sort_by_key(|finding| Reverse(finding.len()));
        findings.dedup();

        for original in findings {
            let placeholder = format!("<SECRET_{}>", map.len());
            text = text.replacen(&original, &placeholder, 1);
            map.insert(placeholder, "<redacted-secret>".to_string());
        }

        (text, map)
    }

    /// Strict desensitize that returns Err if any Block-level finding is detected.
    /// Use this for contexts where blocked data should halt processing.
    pub fn desensitize_strict(
        &self,
        message: &str,
    ) -> Result<(String, HashMap<String, String>), Vec<PrivacyType>> {
        if !self.policy.enabled {
            return Ok((message.to_string(), HashMap::new()));
        }
        let findings = self.detect(message);
        let blocked: Vec<_> = findings
            .iter()
            .filter(|(t, _)| self.policy.action_for(t) == PrivacyAction::Block)
            .map(|(t, _)| t.clone())
            .collect();
        if !blocked.is_empty() {
            return Err(blocked);
        }
        Ok(self.desensitize(message))
    }

    /// Reconstruct original sensitive data from a response using the map.
    pub fn reconstruct(&self, message: &str, map: &HashMap<String, String>) -> String {
        let mut text = message.to_string();
        let mut keys: Vec<_> = map.keys().cloned().collect();
        keys.sort_by_key(|b| std::cmp::Reverse(b.len()));
        for key in keys {
            if let Some(original) = map.get(&key) {
                text = text.replace(&key, original);
            }
        }
        text
    }
}

fn secret_like_findings(message: &str) -> Vec<String> {
    let patterns = [
        r"(?is)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----",
        r"(?i)\bsk-[A-Za-z0-9_-]{8,}\b",
        r"(?i)\bsk-or-v1-[A-Za-z0-9_-]{8,}\b",
        r"(?i)\bgh[pousr]_[A-Za-z0-9_]{8,}\b",
        r"(?i)\bxox[baprs]-[A-Za-z0-9-]{8,}\b",
        r"(?i)\beyJ[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b",
        r#"(?i)\b(api[_ -]?key|access[_ -]?token|refresh[_ -]?token|token|password|passwd|secret|authorization|bearer)\b\s*(is|=|:)?\s*[`'"“”‘’]?[^`'"“”‘’\s,;]{6,}"#,
    ];

    let mut findings = Vec::new();
    for pattern in patterns {
        if let Ok(regex) = Regex::new(pattern) {
            for mat in regex.find_iter(message) {
                findings.push(mat.as_str().to_string());
            }
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_prefixes_are_correct() {
        assert_eq!(PrivacyType::Phone.placeholder_prefix(), "PHONE");
        assert_eq!(PrivacyType::IdCard.placeholder_prefix(), "IDCARD");
        assert_eq!(PrivacyType::Email.placeholder_prefix(), "EMAIL");
        assert_eq!(PrivacyType::BankCard.placeholder_prefix(), "BANKCARD");
        assert_eq!(PrivacyType::Address.placeholder_prefix(), "ADDRESS");
        assert_eq!(PrivacyType::Name.placeholder_prefix(), "NAME");
        assert_eq!(PrivacyType::Generic.placeholder_prefix(), "SENSITIVE");
    }

    #[test]
    fn detect_phone_and_email() {
        let engine = PrivacyEngine::new();
        let text = "联系我 13800138000 或发邮件到 test@example.com";
        let findings = engine.detect(text);
        assert_eq!(findings.len(), 2);
        assert!(findings
            .iter()
            .any(|(t, v)| *t == PrivacyType::Phone && v == "13800138000"));
        assert!(findings
            .iter()
            .any(|(t, v)| *t == PrivacyType::Email && v == "test@example.com"));
    }

    #[test]
    fn desensitize_and_reconstruct_roundtrip() {
        let engine = PrivacyEngine::new();
        let text = "我的电话是 13800138000";
        let (masked, map) = engine.desensitize(text);
        assert!(!masked.contains("13800138000"));
        assert!(masked.contains("<PHONE_0>"));
        let restored = engine.reconstruct(&masked, &map);
        assert_eq!(restored, text);
    }

    #[test]
    fn block_action_masks_idcard_in_standard_mode() {
        let mut policy = PrivacyPolicy::default();
        for rule in &mut policy.rules {
            if rule.ptype == PrivacyType::IdCard {
                rule.action = PrivacyAction::Block;
            }
        }
        let engine = PrivacyEngine::with_policy(policy);
        let text = "身份证号 110101199001011234";
        let (masked, _) = engine.desensitize(text);
        assert!(masked.contains("BLOCKED"));
        assert!(!masked.contains("110101199001011234"));
    }

    #[test]
    fn strict_mode_rejects_blocked_data() {
        let mut policy = PrivacyPolicy::default();
        for rule in &mut policy.rules {
            if rule.ptype == PrivacyType::IdCard {
                rule.action = PrivacyAction::Block;
            }
        }
        let engine = PrivacyEngine::with_policy(policy);
        let text = "身份证号 110101199001011234";
        let result = engine.desensitize_strict(text);
        assert!(result.is_err());
        let blocked = result.unwrap_err();
        assert!(blocked.contains(&PrivacyType::IdCard));
    }

    #[test]
    fn allow_action_leaves_data_intact() {
        let mut policy = PrivacyPolicy::default();
        for rule in &mut policy.rules {
            if rule.ptype == PrivacyType::Phone {
                rule.action = PrivacyAction::Allow;
            }
        }
        let engine = PrivacyEngine::with_policy(policy);
        let text = "我的电话是 13800138000";
        let (masked, map) = engine.desensitize(text);
        assert!(masked.contains("13800138000"));
        assert!(map.is_empty());
    }

    #[test]
    fn disabled_policy_passes_everything() {
        let mut policy = PrivacyPolicy::default();
        policy.enabled = false;
        let engine = PrivacyEngine::with_policy(policy);
        let text = "身份证号 110101199001011234，电话 13800138000";
        let (masked, map) = engine.desensitize(text);
        assert_eq!(masked, text);
        assert!(map.is_empty());
    }

    #[test]
    fn policy_serialization_roundtrip() {
        let policy = PrivacyPolicy::default();
        let yaml = policy.to_yaml().unwrap();
        let loaded = PrivacyPolicy::from_yaml(&yaml).unwrap();
        assert_eq!(loaded.enabled, policy.enabled);
        assert_eq!(loaded.rules.len(), policy.rules.len());
        assert_eq!(loaded.rules[0].ptype, policy.rules[0].ptype);
    }

    #[test]
    fn custom_pattern_override_works() {
        let mut policy = PrivacyPolicy::default();
        policy.rules.push(PrivacyRule {
            ptype: PrivacyType::Generic,
            enabled: true,
            action: PrivacyAction::Mask,
            custom_pattern: Some(r"\bSECRET_\d+\b".to_string()),
        });
        let engine = PrivacyEngine::with_policy(policy);
        let text = "Token: SECRET_12345";
        let findings = engine.detect(text);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].1, "SECRET_12345");
    }

    #[test]
    fn secrets_only_preserves_ordinary_context_but_redacts_credentials() {
        let engine = PrivacyEngine::new();
        let text = "我叫张三，目标是完成 OpenLife Beta。api key: sk-test-secret-123456";
        let (masked, map) = engine.desensitize_secrets_only(text);

        assert!(masked.contains("张三"));
        assert!(masked.contains("OpenLife Beta"));
        assert!(!masked.contains("sk-test-secret-123456"));
        assert!(masked.contains("<SECRET_0>"));
        assert_eq!(
            map.get("<SECRET_0>").map(String::as_str),
            Some("<redacted-secret>")
        );
        assert!(!map.values().any(|value| value.contains("sk-test-secret")));
    }

    #[test]
    fn secrets_only_redacts_private_key_blocks() {
        let engine = PrivacyEngine::new();
        let text = "key -----BEGIN PRIVATE KEY-----\nabc123\n-----END PRIVATE KEY----- done";
        let (masked, map) = engine.desensitize_secrets_only(text);

        assert!(!masked.contains("abc123"));
        assert!(masked.contains("<SECRET_0>"));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn secrets_only_redacts_credentials_even_when_general_privacy_is_disabled() {
        let mut policy = PrivacyPolicy::default();
        policy.enabled = false;
        let engine = PrivacyEngine::with_policy(policy);
        let text = "普通上下文保留，但 api key: sk-test-secret-123456 仍必须拦截";

        let (masked, map) = engine.desensitize_secrets_only(text);

        assert!(masked.contains("普通上下文"));
        assert!(!masked.contains("sk-test-secret-123456"));
        assert!(masked.contains("<SECRET_0>"));
        assert_eq!(
            map.get("<SECRET_0>").map(String::as_str),
            Some("<redacted-secret>")
        );
    }
}
