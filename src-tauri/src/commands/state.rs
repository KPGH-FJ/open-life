use crate::{persist_life_model, AppState};
use openlife_core::life_model::{
    AlertLevel, CustomStateDimension, DailyGoal, StateAlert, TimeBlock,
};
use openlife_core::memory::StateHistoryEntry;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn record_state(
    dimension_name: String,
    value: f64,
    unit: String,
    note: Option<String>,
    min_threshold: Option<f32>,
    max_threshold: Option<f32>,
    alert_days: Option<u32>,
    state: State<'_, Arc<AppState>>,
) -> Result<i64, String> {
    let store = state.memory_store.lock().await;
    let id = store
        .record_state_entry(&dimension_name, value, &unit, note.as_deref())
        .map_err(|e| e.to_string())?;
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(|e| e.to_string())?;
    if let Some(dim) = model
        .state
        .custom_dimensions
        .iter_mut()
        .find(|d| d.name == dimension_name)
    {
        dim.current_value = value as f32;
        if let Some(min) = min_threshold {
            dim.min_threshold = Some(min);
        }
        if let Some(max) = max_threshold {
            dim.max_threshold = Some(max);
        }
        if let Some(days) = alert_days {
            dim.alert_days = days;
        }
    } else {
        model.state.custom_dimensions.push(CustomStateDimension {
            name: dimension_name,
            unit,
            current_value: value as f32,
            min_threshold,
            max_threshold,
            alert_days: alert_days.unwrap_or(3),
        });
    }
    drop(manager);
    let _ = persist_life_model(&state.inner().clone(), model, true).await?;
    Ok(id)
}

#[tauri::command]
pub async fn get_state_history(
    dimension_name: String,
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<StateHistoryEntry>, String> {
    let store = state.memory_store.lock().await;
    store
        .get_state_history(&dimension_name, limit)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_state_alerts(state: State<'_, Arc<AppState>>) -> Result<Vec<StateAlert>, String> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(|e| e.to_string())?;
    let store = state.memory_store.lock().await;
    let mut alerts = Vec::new();
    for dim in &model.state.custom_dimensions {
        let entries = store
            .get_state_history(&dim.name, (dim.alert_days.max(1) as usize) * 2)
            .map_err(|e| e.to_string())?;
        if entries.len() < dim.alert_days.max(1) as usize {
            continue;
        }
        let recent: Vec<_> = entries
            .iter()
            .rev()
            .take(dim.alert_days.max(1) as usize)
            .collect();
        let mut out_of_range_count = 0u32;
        for e in &recent {
            let out = match (dim.min_threshold, dim.max_threshold) {
                (Some(min), Some(max)) => e.value < min as f64 || e.value > max as f64,
                (Some(min), None) => e.value < min as f64,
                (None, Some(max)) => e.value > max as f64,
                (None, None) => false,
            };
            if out {
                out_of_range_count += 1;
            }
        }
        if out_of_range_count >= dim.alert_days {
            let msg = match (dim.min_threshold, dim.max_threshold) {
                (Some(min), Some(max)) => format!(
                    "{} 连续 {} 天超出阈值范围 [{}, {}]，当前 {:.1} {}",
                    dim.name, dim.alert_days, min, max, dim.current_value, dim.unit
                ),
                (Some(min), None) => format!(
                    "{} 连续 {} 天低于阈值 {}，当前 {:.1} {}",
                    dim.name, dim.alert_days, min, dim.current_value, dim.unit
                ),
                (None, Some(max)) => format!(
                    "{} 连续 {} 天高于阈值 {}，当前 {:.1} {}",
                    dim.name, dim.alert_days, max, dim.current_value, dim.unit
                ),
                _ => format!(
                    "{} 连续 {} 天异常，当前 {:.1} {}",
                    dim.name, dim.alert_days, dim.current_value, dim.unit
                ),
            };
            alerts.push(StateAlert {
                dimension_name: dim.name.clone(),
                level: AlertLevel::Warning,
                message: msg,
                triggered_at: chrono::Utc::now().to_rfc3339(),
            });
        }
    }
    Ok(alerts)
}

#[tauri::command]
pub async fn get_daily_goals(state: State<'_, Arc<AppState>>) -> Result<Vec<DailyGoal>, String> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(|e| e.to_string())?;
    Ok(model.goals.daily)
}

#[tauri::command]
pub async fn add_daily_goal(
    name: String,
    time_block: Option<TimeBlock>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(|e| e.to_string())?;
    model.goals.daily.push(DailyGoal {
        name,
        done: false,
        time_block,
    });
    drop(manager);
    persist_life_model(&state.inner().clone(), model, true)
        .await
        .map(|_| ())
}

#[tauri::command]
pub async fn update_daily_goal(
    index: usize,
    name: String,
    time_block: Option<TimeBlock>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(|e| e.to_string())?;
    if let Some(goal) = model.goals.daily.get_mut(index) {
        goal.name = name;
        goal.time_block = time_block;
        drop(manager);
        persist_life_model(&state.inner().clone(), model, true)
            .await
            .map(|_| ())
    } else {
        Err("invalid index".to_string())
    }
}

#[tauri::command]
pub async fn delete_daily_goal(
    index: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(|e| e.to_string())?;
    if index < model.goals.daily.len() {
        model.goals.daily.remove(index);
        drop(manager);
        persist_life_model(&state.inner().clone(), model, true)
            .await
            .map(|_| ())
    } else {
        Err("invalid index".to_string())
    }
}

#[tauri::command]
pub async fn toggle_daily_goal(
    index: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(|e| e.to_string())?;
    if index >= model.goals.daily.len() {
        return Err("invalid index".to_string());
    }
    model.goals.daily[index].done = !model.goals.daily[index].done;
    let completed = model.goals.daily[index].done;
    drop(manager);
    let _ = persist_life_model(&state.inner().clone(), model, true).await?;
    Ok(completed)
}
