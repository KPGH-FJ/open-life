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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveTopic {
    Credential,
    Health,
    Identity,
    Finance,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveContentAssessment {
    pub detected_privacy_types: Vec<PrivacyType>,
    pub sensitive_topics: Vec<SensitiveTopic>,
}

impl SensitiveContentAssessment {
    pub fn requires_memory_review(&self) -> bool {
        self.sensitive_topics.iter().any(|topic| {
            matches!(
                topic,
                SensitiveTopic::Credential
                    | SensitiveTopic::Health
                    | SensitiveTopic::Identity
                    | SensitiveTopic::Finance
                    | SensitiveTopic::Private
            )
        }) || self.detected_privacy_types.iter().any(|privacy_type| {
            matches!(
                privacy_type,
                PrivacyType::Phone
                    | PrivacyType::IdCard
                    | PrivacyType::Email
                    | PrivacyType::BankCard
                    | PrivacyType::Address
            )
        })
    }

    pub fn requires_local_only(&self) -> bool {
        // Maskable contact PII still requires memory governance, but it does
        // not by itself justify disabling a privacy-filtered cloud provider.
        // High-impact topics and credentials remain fail-closed local-only.
        !self.sensitive_topics.is_empty()
    }
}

pub fn assess_sensitive_content(message: &str) -> SensitiveContentAssessment {
    let mut detected_privacy_types = PrivacyEngine::new()
        .detect(message)
        .into_iter()
        .map(|(privacy_type, _)| privacy_type)
        .collect::<Vec<_>>();
    detected_privacy_types.sort_by_key(|privacy_type| match privacy_type {
        PrivacyType::Phone => 0,
        PrivacyType::IdCard => 1,
        PrivacyType::Email => 2,
        PrivacyType::BankCard => 3,
        PrivacyType::Address => 4,
        PrivacyType::Name => 5,
        PrivacyType::Generic => 6,
    });
    detected_privacy_types.dedup();

    let lower = message.to_ascii_lowercase();
    let mut sensitive_topics = Vec::new();
    if !secret_like_findings(message).is_empty() {
        sensitive_topics.push(SensitiveTopic::Credential);
    }
    if contains_personal_health_content(&lower) {
        sensitive_topics.push(SensitiveTopic::Health);
    }
    if contains_sensitive_term(
        &lower,
        &[
            "identity card",
            "id card",
            "passport",
            "social security",
            "ssn",
            "身份证",
            "护照",
            "证件号码",
        ],
    ) || detected_privacy_types.contains(&PrivacyType::IdCard)
    {
        sensitive_topics.push(SensitiveTopic::Identity);
    }
    if contains_sensitive_term(
        &lower,
        &[
            "bank account",
            "bank card",
            "credit card",
            "salary",
            "income",
            "debt",
            "银行账号",
            "银行卡",
            "信用卡",
            "工资",
            "收入",
            "债务",
        ],
    ) || detected_privacy_types.contains(&PrivacyType::BankCard)
    {
        sensitive_topics.push(SensitiveTopic::Finance);
    }
    if contains_private_data_context(&lower) {
        sensitive_topics.push(SensitiveTopic::Private);
    }
    sensitive_topics.sort_by_key(|topic| match topic {
        SensitiveTopic::Credential => 0,
        SensitiveTopic::Health => 1,
        SensitiveTopic::Identity => 2,
        SensitiveTopic::Finance => 3,
        SensitiveTopic::Private => 4,
    });
    sensitive_topics.dedup();

    SensitiveContentAssessment {
        detected_privacy_types,
        sensitive_topics,
    }
}

fn contains_sensitive_term(value: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| value.contains(term))
}

fn contains_personal_health_content(value: &str) -> bool {
    // A health topic is not itself private health data. Public educational
    // material routinely contains words such as `medical`, `身体`, or `胃` and
    // must remain usable with the provider the user selected. Tighten the
    // route only when health language is tied to a person, record, result, or
    // treatment. This keeps actual medical context fail-closed without making
    // an entire conversation local-only merely because it discusses health.
    let health_terms = [
        "health",
        "medical",
        "diagnosis",
        "prescription",
        "therapy",
        "symptom",
        "健康",
        "医疗",
        "诊断",
        "处方",
        "用药",
        "病历",
        "症状",
        "心慌",
        "头疼",
        "身体",
        "胃",
    ];
    if !contains_sensitive_term(value, &health_terms) {
        return false;
    }

    contains_sensitive_term(
        value,
        &[
            "my ",
            "my\n",
            "mine",
            "i have",
            "i was diagnosed",
            "patient record",
            "medical record",
            "test result",
            "lab result",
            "family member",
            "我的",
            "我有",
            "我被诊断",
            "本人",
            "患者",
            "家人",
            "家属",
            "他的",
            "她的",
            "孩子的",
            "父母的",
            "病历",
            "病例",
            "检查报告",
            "检验报告",
            "体检报告",
            "诊断结果",
            "用药记录",
            "医疗记录",
        ],
    )
}

fn contains_private_data_context(value: &str) -> bool {
    // `sensitive` and `敏感` are common domain words (for example insulin
    // sensitivity or 对皮质醇敏感度). They only describe private data when the
    // surrounding phrase says so; a bare substring must not force LocalOnly.
    contains_sensitive_term(
        value,
        &[
            "privacy",
            "private data",
            "private information",
            "private conversation",
            "private document",
            "private file",
            "private record",
            "personal data",
            "personal information",
            "sensitive data",
            "sensitive information",
            "confidential",
            "隐私",
            "私人信息",
            "个人信息",
            "个人数据",
            "敏感数据",
            "敏感信息",
            "机密",
        ],
    )
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
            // A bare Chinese surname followed by one or two Han characters is
            // not a reliable name detector: ordinary phrases such as
            // `官方页面` contain `方页面` and used to stop governed Web reads as
            // false PII. Keep custom Name patterns fully user-controlled, but
            // require an explicit name-bearing phrase for the built-in rule.
            if rule.ptype == PrivacyType::Name && rule.custom_pattern.is_none() {
                findings.extend(
                    contextual_chinese_names(message)
                        .into_iter()
                        .map(|name| (PrivacyType::Name, name)),
                );
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
        let mut counters = HashMap::new();
        let mut map = HashMap::new();
        let text = self.desensitize_with_state(message, &mut counters, &mut map);
        (text, map)
    }

    /// Desensitize one provider payload as a batch so placeholders stay unique
    /// across user, assistant, system, tool, and bounded-context entries.
    pub fn desensitize_batch(&self, messages: &[String]) -> (Vec<String>, HashMap<String, String>) {
        let mut counters = HashMap::new();
        let mut map = HashMap::new();
        let masked = messages
            .iter()
            .map(|message| self.desensitize_with_state(message, &mut counters, &mut map))
            .collect();
        (masked, map)
    }

    fn desensitize_with_state(
        &self,
        message: &str,
        counters: &mut HashMap<PrivacyType, usize>,
        map: &mut HashMap<String, String>,
    ) -> String {
        if !self.policy.enabled {
            return message.to_string();
        }

        let findings = self.detect(message);
        let mut text = message.to_string();

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

                    map.insert(placeholder.clone(), original.clone());
                    text = text.replacen(&original, &placeholder, 1);
                }
            }
        }

        text
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

fn contextual_chinese_names(message: &str) -> Vec<String> {
    let Ok(pattern) = Regex::new(
        r"(?:我叫|姓名(?:是|为|叫|[:：])?|名字(?:是|为|叫|[:：])?|联系人(?:是|为|叫|[:：])?)\s*(?P<name>[李王张刘陈杨赵黄周吴徐孙胡朱高林何郭马罗梁宋郑谢韩唐冯于董萧程曹袁邓许傅沈曾彭吕苏卢蒋蔡贾丁魏薛叶阎余潘杜戴夏钟汪田任姜范方石姚谭廖邹熊金陆郝孔白崔康毛邱秦江史顾侯邵孟龙万段雷钱汤尹黎易常武乔贺赖龚文][\u4e00-\u9fa5]{1,2})",
    ) else {
        return Vec::new();
    };
    pattern
        .captures_iter(message)
        .filter_map(|captures| {
            captures
                .name("name")
                .map(|value| value.as_str().to_string())
        })
        .collect()
}

fn secret_like_findings(message: &str) -> Vec<String> {
    let patterns = [
        r"(?is)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----",
        r"(?i)\bsk-[A-Za-z0-9_-]{8,}\b",
        r"(?i)\bsk-or-v1-[A-Za-z0-9_-]{8,}\b",
        r"(?i)\bgh[pousr]_[A-Za-z0-9_]{8,}\b",
        r"(?i)\bxox[baprs]-[A-Za-z0-9-]{8,}\b",
        r"(?i)\beyJ[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b",
        r#"(?i)\b[A-Za-z0-9_]*(password|passwd|api[_-]?key|access[_-]?token|refresh[_-]?token|secret)\s*[:=]\s*[`'"“”‘’]?[^`'"“”‘’\s,;，；。！？]{6,}"#,
        r#"(?i)\b(api[_ -]?key|access[_ -]?token|refresh[_ -]?token|token|password|passwd|secret|authorization|bearer)\b\s*(is|=|:)?\s*[`'"“”‘’]?[^`'"“”‘’\s,;，；。！？]{6,}"#,
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
    fn default_name_detection_requires_explicit_name_context() {
        let engine = PrivacyEngine::new();

        assert!(engine
            .detect("搜索 Example Domain 官方页面的标题")
            .iter()
            .all(|(privacy_type, _)| *privacy_type != PrivacyType::Name));
        assert!(engine
            .detect("我叫张三，请帮我整理资料")
            .iter()
            .any(|(privacy_type, value)| {
                *privacy_type == PrivacyType::Name && value == "张三"
            }));
        assert!(engine
            .detect("联系人：王小明")
            .iter()
            .any(|(privacy_type, value)| {
                *privacy_type == PrivacyType::Name && value == "王小明"
            }));
    }

    #[test]
    fn sensitive_content_assessment_requires_review_for_id_and_credentials() {
        let id = assess_sensitive_content("身份证号 110101199001011234");
        assert!(id.requires_memory_review());
        assert!(id.detected_privacy_types.contains(&PrivacyType::IdCard));
        assert!(id.sensitive_topics.contains(&SensitiveTopic::Identity));

        let credential = assess_sensitive_content("tool returned user_password=hunter2");
        assert!(credential.requires_memory_review());
        assert!(credential
            .sensitive_topics
            .contains(&SensitiveTopic::Credential));
    }

    #[test]
    fn maskable_contact_pii_requires_review_without_forcing_local_only() {
        let contact = assess_sensitive_content("Contact me at test@example.com or 13800138000");

        assert!(contact.requires_memory_review());
        assert!(!contact.requires_local_only());

        let health = assess_sensitive_content("My diagnosis is private; email test@example.com");
        assert!(health.requires_local_only());
    }

    #[test]
    fn public_health_education_does_not_force_a_local_provider() {
        let educational = assess_sensitive_content(
            "这份皮质醇科普介绍身体的应激反应、胃部影响、常见医疗知识与皮质醇敏感度。",
        );

        assert!(!educational.requires_local_only());
        assert!(!educational
            .sensitive_topics
            .contains(&SensitiveTopic::Health));

        let personal = assess_sensitive_content("我的身体最近心慌，请结合我的用药记录分析。");
        assert!(personal.requires_local_only());
        assert!(personal.sensitive_topics.contains(&SensitiveTopic::Health));
    }

    #[test]
    fn domain_sensitivity_is_not_private_data() {
        let domain_term = assess_sensitive_content("细胞对皮质醇敏感度下降。 ");
        assert!(!domain_term.requires_local_only());
        assert!(!domain_term
            .sensitive_topics
            .contains(&SensitiveTopic::Private));

        let private_data = assess_sensitive_content("这是敏感数据和个人信息，请勿外传。");
        assert!(private_data.requires_local_only());
        assert!(private_data
            .sensitive_topics
            .contains(&SensitiveTopic::Private));
    }

    #[test]
    fn sensitive_content_assessment_does_not_treat_ordinary_numbers_as_ids() {
        let ordinary = assess_sensitive_content("计划编号 202607110001，共三个专注时段");
        assert!(!ordinary.requires_memory_review());
        assert!(!ordinary
            .detected_privacy_types
            .contains(&PrivacyType::IdCard));
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
        let policy = PrivacyPolicy {
            enabled: false,
            ..Default::default()
        };
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
        let text = "我叫张三，目标是完成 OpenLife 能力版。api key: sk-test-secret-123456";
        let (masked, map) = engine.desensitize_secrets_only(text);

        assert!(masked.contains("张三"));
        assert!(masked.contains("OpenLife 能力版"));
        assert!(!masked.contains("sk-test-secret-123456"));
        assert!(masked.contains("<SECRET_0>"));
        assert_eq!(
            map.get("<SECRET_0>").map(String::as_str),
            Some("<redacted-secret>")
        );
        assert!(!map.values().any(|value| value.contains("sk-test-secret")));
    }

    #[test]
    fn secrets_only_redacts_exact_quoted_tool_password_without_rehydration() {
        let engine = PrivacyEngine::new();
        let text = "工具返回：REMEMBER user_password=hunter2。请告诉我工具返回了什么。";
        let (masked, map) = engine.desensitize_secrets_only(text);
        let reconstructed = engine.reconstruct(&masked, &map);

        assert!(!masked.contains("hunter2"));
        assert!(!reconstructed.contains("hunter2"));
        assert!(reconstructed.contains("<redacted-secret>"));
        assert!(map.values().all(|value| value == "<redacted-secret>"));
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
        let policy = PrivacyPolicy {
            enabled: false,
            ..Default::default()
        };
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
