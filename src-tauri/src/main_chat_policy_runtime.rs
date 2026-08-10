use std::sync::Arc;

use openlife_core::agent::{
    AgentTask, AgentTaskKind, PolicyTopic, RiskLevel, RuntimeHSPacket,
    RuntimePolicyPacketBuildInput,
};
use openlife_core::privacy::{PrivacyEngine, PrivacyType};

use crate::AppState;

pub(crate) fn build_chat_runtime_policy_packet(
    state: &Arc<AppState>,
    task: &AgentTask,
    tools_prompt: &str,
    agent_run_id: Option<String>,
) -> Result<RuntimeHSPacket, String> {
    let topic = classify_main_chat_policy_topic(&task.user_text, tools_prompt);
    let tool_requirements = main_chat_policy_tool_requirements(&task.user_text, tools_prompt);
    let risk_level = main_chat_policy_risk_level(topic, &tool_requirements);
    let sanitized_intent_summary =
        sanitized_policy_intent_summary(task.kind, topic, &tool_requirements, &task.user_text);

    openlife_core::agent::build_runtime_policy_packet(
        &state.policy_store,
        RuntimePolicyPacketBuildInput {
            task,
            sanitized_intent_summary,
            privacy_topic: topic,
            risk_level,
            tool_requirements,
            agent_run_id,
        },
    )
    .map_err(|error| format!("Main Chat Policy runtime packet build failed: {error}"))
}

pub(crate) fn classify_main_chat_policy_topic(user_text: &str, _tools_prompt: &str) -> PolicyTopic {
    let text = user_text.to_lowercase();
    let privacy_engine = PrivacyEngine::new();
    let privacy_findings = privacy_engine.detect(&text);
    if privacy_findings
        .iter()
        .any(|(ptype, _)| matches!(ptype, PrivacyType::IdCard))
    {
        return PolicyTopic::Identity;
    }
    if privacy_findings
        .iter()
        .any(|(ptype, _)| matches!(ptype, PrivacyType::BankCard))
    {
        return PolicyTopic::Finance;
    }
    if privacy_findings.iter().any(|(ptype, _)| {
        matches!(
            ptype,
            PrivacyType::Email
                | PrivacyType::Phone
                | PrivacyType::Address
                | PrivacyType::Name
                | PrivacyType::Generic
        )
    }) {
        return PolicyTopic::PrivateFile;
    }

    if contains_any(
        &text,
        &[
            "health",
            "medical",
            "medicine",
            "medication",
            "prescription",
            "doctor",
            "therapy",
            "mental",
            "mental health",
            "illness",
            "diagnosis",
            "diagnose",
            "anxiety",
            "depression",
            "drug",
            "药",
            "用药",
            "处方",
            "病",
            "医院",
            "健康",
            "心理",
            "焦虑",
            "抑郁",
            "诊断",
            "治疗",
        ],
    ) {
        PolicyTopic::Health
    } else if contains_any(
        &text,
        &[
            "finance",
            "bank",
            "salary",
            "income",
            "insurance",
            "debt",
            "loan",
            "tax",
            "credit",
            "mortgage",
            "投资",
            "银行",
            "工资",
            "收入",
            "保险",
            "债务",
            "负债",
            "贷款",
            "税",
            "信用卡",
        ],
    ) {
        PolicyTopic::Finance
    } else if contains_any(
        &text,
        &[
            "identity",
            "identity card",
            "id card",
            "passport",
            "ssn",
            "values",
            "mission",
            "身份",
            "身份证",
            "护照",
            "证件",
            "价值观",
            "使命",
        ],
    ) {
        PolicyTopic::Identity
    } else if contains_any(
        &text,
        &[
            "relationship",
            "intimate relationship",
            "partner",
            "family",
            "breakup",
            "break up",
            "divorce",
            "family conflict",
            "关系",
            "亲密关系",
            "伴侣",
            "家人",
            "分手",
            "家庭矛盾",
            "家庭冲突",
            "婚姻",
            "离婚",
            "恋爱",
        ],
    ) {
        PolicyTopic::Relationship
    } else if contains_any(
        &text,
        &[
            "private file",
            "privacy",
            "private",
            "secret",
            "confidential",
            "contract",
            "resume",
            "cv",
            "私人文件",
            "隐私",
            "机密",
            "合同",
            "简历",
        ],
    ) {
        PolicyTopic::PrivateFile
    } else {
        PolicyTopic::General
    }
}

pub(crate) fn main_chat_policy_tool_requirements(
    user_text: &str,
    _tools_prompt: &str,
) -> Vec<String> {
    let text = user_text.to_lowercase();
    let mut requirements = Vec::new();
    if contains_any(
        &text,
        &[
            "write",
            "save",
            "send",
            "email",
            "calendar",
            "file.write",
            "propose_event",
            "保存",
            "写入",
            "发送",
            "邮件",
            "日历",
        ],
    ) {
        requirements.push("write".to_string());
    }
    if contains_any(
        &text,
        &[
            "send", "email", "calendar", "external", "发送", "邮件", "日历",
        ],
    ) {
        requirements.push("external_side_effect".to_string());
    }
    requirements.sort();
    requirements.dedup();
    requirements
}

fn main_chat_policy_risk_level(topic: PolicyTopic, tool_requirements: &[String]) -> RiskLevel {
    if topic != PolicyTopic::General
        || tool_requirements
            .iter()
            .any(|requirement| requirement == "write" || requirement == "external_side_effect")
    {
        RiskLevel::High
    } else {
        RiskLevel::Low
    }
}

fn sanitized_policy_intent_summary(
    task_kind: AgentTaskKind,
    topic: PolicyTopic,
    tool_requirements: &[String],
    user_text: &str,
) -> String {
    let char_count = user_text.chars().count();
    let length_bucket = match char_count {
        0..=80 => "short",
        81..=240 => "medium",
        _ => "long",
    };
    format!(
        "task_kind={}; topic={:?}; length_bucket={}; tool_requirements={}",
        task_kind,
        topic,
        length_bucket,
        tool_requirements.join(",")
    )
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}
