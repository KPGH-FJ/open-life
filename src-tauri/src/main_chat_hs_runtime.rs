use std::sync::Arc;

use openlife_core::agent::{
    AgentTask, AgentTaskKind, HSAssetAuthorityRegistry, HSAssetCategory, HSAssetOwner, PolicyTopic,
    RiskLevel, RuntimeHSPacket,
};
use openlife_core::life_model::LifeModel;
use openlife_core::privacy::{PrivacyEngine, PrivacyType};

use crate::AppState;

pub(crate) async fn build_chat_runtime_hs_packet(
    state: &Arc<AppState>,
    task: &AgentTask,
    life_model: &LifeModel,
    tools_prompt: &str,
    agent_run_id: Option<String>,
) -> Result<Option<RuntimeHSPacket>, String> {
    let authority_registry_path = {
        state
            .life_model_manager
            .lock()
            .await
            .hs_asset_authority_registry_path()
    };
    let authority_registry = HSAssetAuthorityRegistry::new(authority_registry_path)
        .map_err(|error| format!("HS asset authority registry unavailable: {error}"))?;
    let collaboration_authority = authority_registry
        .authority(HSAssetCategory::CollaborationGuidance)
        .map_err(|error| format!("HS collaboration guidance authority unavailable: {error}"))?;
    let topic = classify_hs_policy_topic(&task.user_text, tools_prompt);
    let tool_requirements = hs_tool_requirements(&task.user_text, tools_prompt);
    let risk_level = hs_risk_level(topic, &tool_requirements);
    let state_hints = serde_json::json!({
        "energy": life_model.state.health_status.energy_level,
    });
    let sanitized_intent_summary =
        sanitized_hs_intent_summary(task.kind, topic, &tool_requirements, &task.user_text);

    let packet = {
        let heuristic_store = state.heuristic_store.lock().await;
        openlife_core::agent::build_runtime_hs_packet(
            &state.policy_store,
            &heuristic_store,
            openlife_core::agent::RuntimeHSPacketBuildInput {
                task,
                sanitized_intent_summary,
                privacy_topic: topic,
                risk_level,
                tool_requirements,
                current_state_hints: state_hints,
                token_budget: 384,
                agent_run_id,
            },
        )
        .map_err(|e| format!("HS runtime packet build failed: {}", e))?
    };

    if collaboration_authority.owner == HSAssetOwner::AcceptedHsStore {
        return Ok(packet);
    }

    // LM-B shadow selection is evidence about deterministic materialization,
    // not evidence that a product turn completed with the candidate owner.
    // This pre-provider seam must never manufacture a product-scenario
    // receipt: no provider/tool/final fact exists yet and the packet below is
    // deliberately withheld while YAML remains canonical. A later, explicit
    // trial verifier may record a receipt only after linking a successful
    // terminal turn to the selected asset references and output digest.
    let _shadow_packet = packet;
    Ok(None)
}

pub(crate) fn classify_hs_policy_topic(user_text: &str, _tools_prompt: &str) -> PolicyTopic {
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

pub(crate) fn hs_tool_requirements(user_text: &str, _tools_prompt: &str) -> Vec<String> {
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

fn hs_risk_level(topic: PolicyTopic, tool_requirements: &[String]) -> RiskLevel {
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

fn sanitized_hs_intent_summary(
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
