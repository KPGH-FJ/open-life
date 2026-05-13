use std::sync::Arc;

use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use tauri::State;

use crate::AppState;

#[allow(clippy::unnecessary_map_or)]
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
    let leading_punct = |i: usize| {
        i == 0 || {
            lower[..i].chars().next_back().map_or(false, |c| {
                matches!(c, '.' | '!' | '?' | '\n' | '。' | '！' | '？')
            })
        }
    };
    let trigger_positions: Vec<usize> = triggers
        .iter()
        .flat_map(|t| {
            lower
                .match_indices(t)
                .filter(|(idx, _)| leading_punct(*idx))
                .map(|(idx, matched)| idx + matched.len())
        })
        .collect();
    if trigger_positions.is_empty() {
        return None;
    }
    let is_word_boundary_char = |c: char| -> bool {
        c.is_whitespace() || matches!(c, '.' | '!' | '?' | ',' | '。' | '！' | '？' | '，' | '\n')
    };
    const SEARCH_WINDOW_CHARS: usize = 60;
    let mut checked = Vec::new();
    for goal in &mut life_model.goals.daily {
        if goal.done {
            continue;
        }
        let goal_lower = goal.name.to_lowercase();
        let matched = trigger_positions.iter().any(|&start| {
            let window_end = (start + SEARCH_WINDOW_CHARS).min(lower.len());
            let window = &lower[start..window_end];
            window.match_indices(&goal_lower).any(|(idx, _)| {
                let before = idx == 0 || {
                    window[..idx]
                        .chars()
                        .next_back()
                        .map_or(false, is_word_boundary_char)
                };
                let after_idx = idx + goal_lower.len();
                let after = window[after_idx..]
                    .chars()
                    .next()
                    .map_or(true, is_word_boundary_char);
                before && after
            })
        });
        if matched {
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

pub(crate) async fn run_auto_checkin_and_stream_signals(
    user_msg: &Option<ChatMessage>,
    life_model: &mut LifeModel,
    session_id: &str,
    state: &State<'_, Arc<AppState>>,
    agent_run: Option<&mut openlife_core::agent::AgentRun>,
) -> Result<Option<String>, String> {
    let Some(ref m) = user_msg else {
        return Ok(None);
    };
    let msg = try_auto_checkin_daily_goals(&m.content, life_model);
    crate::capture_conversation_signals(session_id, &m.content, life_model, state.inner()).await;
    if msg.is_some() {
        if let Err(message) =
            crate::persist_life_model(&state.inner().clone(), life_model.clone(), false).await
        {
            if let Some(run) = agent_run {
                run.fail(openlife_core::agent::AgentRunError {
                    message: message.clone(),
                    phase: "preprocess".to_string(),
                    recoverable: true,
                });
                if let Some(ref store_arc) = state.agent_run_store {
                    let store = store_arc.lock().await;
                    if let Err(e) = store.update_run(run) {
                        log::warn!("[AgentRun] 更新运行记录失败: {}", e);
                    }
                }
            }
            return Err(message);
        }
    }
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::life_model::DailyGoal;

    fn make_test_life_model() -> LifeModel {
        let mut lm = LifeModel::default();
        lm.goals.daily = vec![DailyGoal {
            name: "运动30分钟".to_string(),
            done: false,
            time_block: None,
        }];
        lm
    }

    #[test]
    fn test_auto_checkin_triggers_on_match() {
        let mut lm = make_test_life_model();
        let result = try_auto_checkin_daily_goals("我今天完成了运动30分钟", &mut lm);
        assert!(result.is_some());
        assert!(lm.goals.daily[0].done);
    }

    #[test]
    fn test_auto_checkin_no_match() {
        let mut lm = make_test_life_model();
        let result = try_auto_checkin_daily_goals("今天天气真好", &mut lm);
        assert!(result.is_none());
        assert!(!lm.goals.daily[0].done);
    }

    #[test]
    fn test_auto_checkin_multiple_triggers() {
        let triggers = ["我完成了", "我搞定了", "已经打卡了"];
        for trigger in triggers {
            let mut lm = make_test_life_model();
            let result = try_auto_checkin_daily_goals(&format!("{trigger}运动30分钟"), &mut lm);
            assert!(result.is_some(), "trigger '{trigger}' should match");
            assert!(lm.goals.daily[0].done);
        }
    }

    #[test]
    fn test_auto_checkin_partial_match() {
        let mut lm = make_test_life_model();
        let result = try_auto_checkin_daily_goals("我今天完成了运动", &mut lm);
        assert!(!lm.goals.daily[0].done || result.is_some());
    }
}
