use crate::state::AppState;
use openlife_core::life_model::LifeModel;
use std::sync::Arc;

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
