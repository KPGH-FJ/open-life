use std::sync::Arc;

use openlife_core::agent::ReasoningTrace;
use openlife_core::life_model::LifeModel;

use crate::AppState;

pub(crate) fn try_auto_checkin_daily_goals(
    content: &str,
    life_model: &mut LifeModel,
) -> Option<String> {
    let lower = content.to_lowercase();
    let triggers = [
        "我今天完成了",
        "我完成了",
        "我已经完成了",
        "刚刚完成了",
        "我搞定了",
        "我做完了",
        "已经打卡了",
        "今天搞定了",
    ];
    let triggered = triggers.iter().any(|t| lower.contains(t));
    if !triggered {
        return None;
    }
    let mut checked = Vec::new();
    for goal in &mut life_model.goals.daily {
        if goal.done {
            continue;
        }
        if lower.contains(&goal.name.to_lowercase()) {
            goal.done = true;
            checked.push(goal.name.clone());
        }
    }
    if !checked.is_empty() {
        Some(format!("已自动打卡今日目标：{}", checked.join("、")))
    } else {
        None
    }
}

#[allow(dead_code)]
pub(crate) fn build_reasoning_trace_prompt(trace: &ReasoningTrace) -> String {
    let mut prompt = String::new();
    if let Some(ref m) = trace.meaning_result {
        if let Some(text) = m.get("text").and_then(|t| t.as_str()) {
            prompt.push_str(&format!("【意义层约束】{}\n", text));
        }
    }
    if let Some(ref s) = trace.strategy_result {
        if let Some(text) = s.get("text").and_then(|t| t.as_str()) {
            prompt.push_str(&format!("【策略层约束】{}\n", text));
        }
        if let Some(tools) = s.get("suggested_tools").and_then(|v| v.as_array()) {
            let tools_text = tools
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join("、");
            if !tools_text.is_empty() {
                prompt.push_str(&format!("【建议工具】{}\n", tools_text));
            }
        }
    }
    if let Some(ref safety) = trace.safety_check_result {
        if let Some(warnings) = safety.get("warnings").and_then(|v| v.as_array()) {
            let text = warnings
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join("；");
            if !text.is_empty() {
                prompt.push_str(&format!("【安全检查提醒】{}\n", text));
            }
        }
    }
    prompt
}

pub(crate) async fn capture_conversation_signals(
    session_id: &str,
    user_text: &str,
    life_model: &LifeModel,
    state: &Arc<AppState>,
) {
    let normalized = user_text.trim().to_lowercase();
    if normalized.is_empty() {
        return;
    }

    let positive = [
        "想继续",
        "想做",
        "准备",
        "投入",
        "推进",
        "喜欢",
        "热爱",
        "重要",
        "值得",
        "继续",
    ];
    let negative = [
        "不想",
        "放弃",
        "没意义",
        "拖延",
        "讨厌",
        "厌倦",
        "卡住",
        "做不到",
    ];
    let has_positive = positive.iter().any(|kw| normalized.contains(kw));
    let has_negative = negative.iter().any(|kw| normalized.contains(kw));
    let base_delta = if has_negative {
        -0.02
    } else if has_positive {
        0.02
    } else {
        0.01
    };
    let confidence = if has_positive || has_negative {
        0.68
    } else {
        0.45
    };

    let store = state.feedback_store.lock().await;

    for value in &life_model.identity.values {
        if normalized.contains(&value.name.to_lowercase()) {
            if let Err(e) = store.log_event(
                &format!("value_focus:{}", value.name),
                Some(session_id),
                Some("chat_match"),
            ) {
                log::warn!("[Memory] 记录价值观焦点事件失败: {}", e);
            }
            if let Err(e) = store.save_conversation_inference(
                Some(session_id),
                "identity.values",
                &value.name,
                base_delta,
                confidence,
                "用户在对话中主动提及或强化了该价值观",
            ) {
                log::warn!("[Memory] 保存价值观推断失败: {}", e);
            }
        }
    }

    for goal in life_model
        .goals
        .short_term
        .iter()
        .chain(life_model.goals.medium_term.iter())
        .chain(life_model.goals.long_term.iter())
        .chain(life_model.goals.life_goals.iter())
    {
        if normalized.contains(&goal.name.to_lowercase()) {
            if let Err(e) = store.save_conversation_inference(
                Some(session_id),
                "goals",
                &goal.name,
                base_delta,
                (confidence - 0.05).max(0.2),
                "用户在对话中直接提到该目标，表明关注度发生变化",
            ) {
                log::warn!("[Memory] 保存目标推断失败: {}", e);
            }
        }
    }

    for skill in &life_model.capabilities.skills {
        if normalized.contains(&skill.name.to_lowercase()) {
            let skill_delta = if normalized.contains("学习")
                || normalized.contains("练习")
                || normalized.contains("提升")
            {
                0.02
            } else {
                base_delta
            };
            if let Err(e) = store.save_conversation_inference(
                Some(session_id),
                "capabilities.skills",
                &skill.name,
                skill_delta,
                0.55,
                "用户在对话中主动提及技能投入或受阻情况",
            ) {
                log::warn!("[Memory] 保存技能推断失败: {}", e);
            }
        }
    }
}
