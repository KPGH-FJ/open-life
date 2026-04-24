use crate::life_model::LifeModel;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackType {
    ThumbsUp,
    ThumbsDown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEntry {
    pub session_id: String,
    pub message_index: i64,
    pub feedback_type: FeedbackType,
    pub content_preview: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsSummary {
    pub total_messages: i64,
    pub total_feedback_up: i64,
    pub total_feedback_down: i64,
    pub session_count: i64,
}

pub struct FeedbackStore {
    conn: Mutex<Connection>,
}

impl FeedbackStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open feedback sqlite db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("failed to open in-memory feedback sqlite db")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                message_index INTEGER NOT NULL,
                feedback_type TEXT NOT NULL,
                content_preview TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS analytics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_name TEXT NOT NULL,
                session_id TEXT,
                detail TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS conversation_inferences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                dimension TEXT NOT NULL,
                target_name TEXT NOT NULL,
                suggested_delta REAL,
                confidence REAL,
                reason TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_feedback_session ON feedback(session_id, created_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_inference_created ON conversation_inferences(created_at)",
            [],
        )?;
        Ok(())
    }

    pub fn save_feedback(&self, entry: &FeedbackEntry) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let ft = match entry.feedback_type {
            FeedbackType::ThumbsUp => "up",
            FeedbackType::ThumbsDown => "down",
        };
        conn.execute(
            "INSERT INTO feedback (session_id, message_index, feedback_type, content_preview, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![entry.session_id, entry.message_index, ft, entry.content_preview, entry.created_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn log_event(
        &self,
        event_name: &str,
        session_id: Option<&str>,
        detail: Option<&str>,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO analytics (event_name, session_id, detail, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![event_name, session_id, detail, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn count_event_today(&self, event_name: &str) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM analytics WHERE event_name = ?1 AND DATE(created_at) = ?2",
                params![event_name, today],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count)
    }

    pub fn save_conversation_inference(
        &self,
        session_id: Option<&str>,
        dimension: &str,
        target_name: &str,
        suggested_delta: f32,
        confidence: f32,
        reason: &str,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO conversation_inferences (session_id, dimension, target_name, suggested_delta, confidence, reason, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![session_id, dimension, target_name, suggested_delta, confidence, reason, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Fetch evolution signals from feedback, analytics and conversation_inferences for the last N days.
    pub fn fetch_evolution_signals(&self, days: i64) -> Result<crate::evolution::EvolutionSignals> {
        use crate::evolution::EvolutionSignals;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();

        // feedback signals
        let mut stmt = conn.prepare(
            "SELECT content_preview, feedback_type FROM feedback WHERE created_at >= ?1 ORDER BY created_at DESC LIMIT 100",
        )?;
        let rows = stmt.query_map(params![&cutoff], |row| {
            let content: String = row.get(0)?;
            let ft: String = row.get(1)?;
            Ok((content, ft))
        })?;

        let mut up_keywords: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        let mut down_keywords: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        for row in rows {
            let (content, ft) = row?;
            let words = Self::tokenize(&content);
            if ft == "up" {
                for w in words {
                    *up_keywords.entry(w).or_insert(0) += 1;
                }
            } else {
                for w in words {
                    *down_keywords.entry(w).or_insert(0) += 1;
                }
            }
        }

        let mut feedback: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
        for (word, up) in &up_keywords {
            let down = down_keywords.get(word).copied().unwrap_or(0);
            let delta = if *up > down {
                0.01 * (*up - down).min(3) as f32
            } else if down > *up {
                -0.01 * (down - *up).min(3) as f32
            } else {
                0.0
            };
            if delta != 0.0 {
                feedback.insert(word.clone(), delta);
            }
        }

        // behavior signals
        let mut event_stmt = conn.prepare(
            "SELECT event_name, COUNT(*) as cnt FROM analytics WHERE created_at >= ?1 GROUP BY event_name",
        )?;
        let event_rows = event_stmt.query_map(params![&cutoff], |row| {
            let name: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            Ok((name, cnt))
        })?;
        let mut behavior: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
        for row in event_rows {
            let (name, cnt) = row?;
            if let Some(stripped) = name.strip_prefix("value_focus:") {
                behavior.insert(stripped.to_lowercase(), 0.01 * (cnt as f32).min(3.0));
            }
        }

        // inference signals
        let mut inf_stmt = conn.prepare(
            "SELECT dimension, target_name, suggested_delta, confidence FROM conversation_inferences WHERE created_at >= ?1",
        )?;
        let inf_rows = inf_stmt.query_map(params![&cutoff], |row| {
            let dim: String = row.get(0)?;
            let target: String = row.get(1)?;
            let delta: f64 = row.get(2)?;
            let conf: f64 = row.get(3)?;
            Ok((dim, target, delta as f32, conf as f32))
        })?;
        let mut inference: std::collections::HashMap<String, f32> =
            std::collections::HashMap::new();
        for row in inf_rows {
            let (dim, target, delta, conf) = row?;
            let key = format!("{}:{}", dim, target.to_lowercase());
            let weighted = delta * conf;
            *inference.entry(key).or_insert(0.0) += weighted;
        }

        Ok(EvolutionSignals {
            feedback,
            behavior,
            inference,
        })
    }

    pub fn summary(&self) -> Result<AnalyticsSummary> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let total_messages: i64 = conn
            .query_row("SELECT COUNT(*) FROM feedback", [], |row| row.get(0))
            .unwrap_or(0);
        let total_feedback_up: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM feedback WHERE feedback_type = 'up'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let total_feedback_down: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM feedback WHERE feedback_type = 'down'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let session_count: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT session_id) FROM feedback",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(AnalyticsSummary {
            total_messages,
            total_feedback_up,
            total_feedback_down,
            session_count,
        })
    }

    /// Tokenize mixed Chinese/English text into lowercase tokens.
    fn tokenize(text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current_word = String::new();
        for ch in text.chars() {
            if ch.is_alphabetic() || ch.is_numeric() {
                current_word.push(ch);
            } else if ch as u32 >= 0x4e00 && ch as u32 <= 0x9fff {
                // CJK unified ideographs: flush any pending Latin token, then emit the character itself as a token
                if !current_word.is_empty() {
                    tokens.push(current_word.to_lowercase());
                    current_word.clear();
                }
                tokens.push(ch.to_string());
            } else {
                // punctuation / whitespace: flush Latin token
                if !current_word.is_empty() {
                    tokens.push(current_word.to_lowercase());
                    current_word.clear();
                }
            }
        }
        if !current_word.is_empty() {
            tokens.push(current_word.to_lowercase());
        }
        tokens
    }

    /// Simple weight adjustment heuristic based on feedback
    pub fn apply_feedback_to_model(&self, model: &mut LifeModel) -> Result<String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT content_preview, feedback_type FROM feedback ORDER BY created_at DESC LIMIT 100",
        )?;
        let rows = stmt.query_map([], |row| {
            let content: String = row.get(0)?;
            let ft: String = row.get(1)?;
            Ok((content, ft))
        })?;

        let mut up_keywords: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        let mut down_keywords: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();

        for row in rows {
            let (content, ft) = row?;
            let words = Self::tokenize(&content);
            if ft == "up" {
                for w in words {
                    *up_keywords.entry(w).or_insert(0) += 1;
                }
            } else {
                for w in words {
                    *down_keywords.entry(w).or_insert(0) += 1;
                }
            }
        }

        let mut changed = Vec::new();
        for value in &mut model.identity.values {
            let name_lower = value.name.to_lowercase();
            let up_score = up_keywords.get(&name_lower).copied().unwrap_or(0);
            let down_score = down_keywords.get(&name_lower).copied().unwrap_or(0);
            if up_score > down_score && value.weight < 10 {
                value.weight = (value.weight + 1).min(10);
                changed.push(format!("提升 '{}' 权重到 {}", value.name, value.weight));
            } else if down_score > up_score && value.weight > 1 {
                value.weight = (value.weight - 1).max(1);
                changed.push(format!("降低 '{}' 权重到 {}", value.name, value.weight));
            }
        }

        if changed.is_empty() {
            Ok("暂无足够反馈来微调模型".to_string())
        } else {
            Ok(changed.join("\n"))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvolutionReport {
    pub liked_patterns: Vec<String>,
    pub disliked_patterns: Vec<String>,
    pub suggested_rules: Vec<String>,
    pub summary_text: String,
}

impl FeedbackStore {
    /// Generate a micro-evolution report from recent feedback and analytics.
    /// This scans the last 100 feedback entries and last 30 days of analytics events.
    pub fn generate_evolution_report(&self) -> Result<EvolutionReport> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339();

        // Gather feedback keywords
        let mut stmt = conn.prepare(
            "SELECT content_preview, feedback_type FROM feedback WHERE created_at >= ?1 ORDER BY created_at DESC LIMIT 100",
        )?;
        let rows = stmt.query_map(params![&cutoff], |row| {
            let content: String = row.get(0)?;
            let ft: String = row.get(1)?;
            Ok((content, ft))
        })?;

        let mut up_keywords: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        let mut down_keywords: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        for row in rows {
            let (content, ft) = row?;
            let words = Self::tokenize(&content);
            if ft == "up" {
                for w in words {
                    *up_keywords.entry(w).or_insert(0) += 1;
                }
            } else {
                for w in words {
                    *down_keywords.entry(w).or_insert(0) += 1;
                }
            }
        }

        // Extract top patterns
        let mut up_vec: Vec<(String, i32)> = up_keywords.clone().into_iter().collect();
        up_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let liked: Vec<String> = up_vec
            .into_iter()
            .take(5)
            .map(|(k, c)| format!("{} ({}次)", k, c))
            .collect();

        let mut down_vec: Vec<(String, i32)> = down_keywords.clone().into_iter().collect();
        down_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let disliked: Vec<String> = down_vec
            .into_iter()
            .take(5)
            .map(|(k, c)| format!("{} ({}次)", k, c))
            .collect();
        let total_up: i32 = up_keywords.values().sum();
        let total_down: i32 = down_keywords.values().sum();

        // Gather common analytics events as implicit signals
        let mut event_stmt = conn.prepare(
            "SELECT event_name, COUNT(*) as cnt FROM analytics WHERE created_at >= ?1 GROUP BY event_name ORDER BY cnt DESC LIMIT 20",
        )?;
        let event_rows = event_stmt.query_map(params![&cutoff], |row| {
            let name: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            Ok((name, cnt))
        })?;
        let mut events = Vec::new();
        for row in event_rows {
            let (name, cnt) = row?;
            events.push(format!("{} ({}次)", name, cnt));
        }

        // Build suggested rules heuristically
        let mut rules = Vec::new();
        if total_down > total_up && total_down >= 3 {
            rules.push("近期负面反馈较多，优先采用更温和、简短的回应方式。".to_string());
        }
        if total_up > total_down && total_up >= 5 {
            rules.push("近期正面反馈占主导，可以在回应中适当增加鼓励与肯定。".to_string());
        }
        if !disliked.is_empty() {
            rules.push(format!(
                "避免在回答中过多涉及这些不被喜欢的主题: {}",
                disliked.join(", ")
            ));
        }
        if !liked.is_empty() {
            rules.push(format!("用户偏好的表达方式或主题: {}", liked.join(", ")));
        }
        if events.iter().any(|e| e.contains("builder_quick_complete")) {
            rules.push("用户近期完成过快速构建，可以主动询问是否需要更新人生模型。".to_string());
        }
        if events.iter().any(|e| e.contains("memory_search")) {
            rules.push("用户频繁使用记忆搜索，回应时可引用更多历史记忆作为上下文。".to_string());
        }

        let summary = format!(
            "近30天反馈统计: 👍 {} 条, 👎 {} 条; 高频事件: {}",
            total_up,
            total_down,
            if events.is_empty() {
                "无".to_string()
            } else {
                events.join(", ")
            }
        );

        Ok(EvolutionReport {
            liked_patterns: liked,
            disliked_patterns: disliked,
            suggested_rules: rules,
            summary_text: summary,
        })
    }

    /// Run a 7-day sliding window micro-evolution on the life model.
    /// Adjusts value weights by ±0.01–0.03 based on recent feedback and analytics.
    pub fn run_micro_evolution(&self, model: &mut LifeModel) -> Result<String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT content_preview, feedback_type FROM feedback WHERE created_at >= ?1 ORDER BY created_at DESC LIMIT 100",
        )?;
        let rows = stmt.query_map(params![&cutoff], |row| {
            let content: String = row.get(0)?;
            let ft: String = row.get(1)?;
            Ok((content, ft))
        })?;

        let mut up_keywords: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        let mut down_keywords: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        for row in rows {
            let (content, ft) = row?;
            let words = Self::tokenize(&content);
            if ft == "up" {
                for w in words {
                    *up_keywords.entry(w).or_insert(0) += 1;
                }
            } else {
                for w in words {
                    *down_keywords.entry(w).or_insert(0) += 1;
                }
            }
        }

        // Analytics as implicit behavior signals (weight 0.3)
        let mut event_stmt = conn.prepare(
            "SELECT event_name, COUNT(*) as cnt FROM analytics WHERE created_at >= ?1 GROUP BY event_name",
        )?;
        let event_rows = event_stmt.query_map(params![&cutoff], |row| {
            let name: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            Ok((name, cnt))
        })?;
        let mut events: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for row in event_rows {
            let (name, cnt) = row?;
            events.insert(name, cnt);
        }

        let mut changed = Vec::new();
        for value in &mut model.identity.values {
            let name_lower = value.name.to_lowercase();
            let up_score = up_keywords.get(&name_lower).copied().unwrap_or(0) as f32;
            let down_score = down_keywords.get(&name_lower).copied().unwrap_or(0) as f32;

            // Base delta from explicit feedback (weight 0.5)
            let mut delta = 0.0f32;
            if up_score > down_score {
                delta += 0.01 * (up_score - down_score).min(3.0);
            } else if down_score > up_score {
                delta -= 0.01 * (down_score - up_score).min(3.0);
            }

            // Boost from analytics events (weight 0.3)
            let event_boost = events
                .get(&format!("value_focus:{}", value.name))
                .copied()
                .unwrap_or(0) as f32;
            if event_boost > 0.0 {
                delta += 0.01 * event_boost.min(3.0);
            }

            // Clamp weight to [1.0, 10.0]
            if delta.abs() >= 0.005 {
                let new_weight = (value.weight as f32 + delta).clamp(1.0, 10.0);
                let rounded = (new_weight * 100.0).round() / 100.0;
                if (rounded - value.weight as f32).abs() >= 0.005 {
                    value.weight = rounded as u8;
                    changed.push(format!("调整 '{}' 权重到 {:.2}", value.name, rounded));
                }
            }
        }

        if changed.is_empty() {
            Ok("近7天暂无足够信号来微调模型权重".to_string())
        } else {
            Ok(changed.join("\n"))
        }
    }
}

/// Report used for weekly/monthly calibration dialog.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalibrationReport {
    pub period_days: i64,
    pub feedback_up: i64,
    pub feedback_down: i64,
    pub top_liked_patterns: Vec<String>,
    pub top_disliked_patterns: Vec<String>,
    pub value_changes: Vec<String>,
    pub suggested_actions: Vec<String>,
    pub summary_text: String,
}

impl FeedbackStore {
    /// Generate a calibration report for the given period (default 7 days).
    pub fn generate_calibration_report(
        &self,
        model: &LifeModel,
        period_days: i64,
    ) -> Result<CalibrationReport> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(period_days as i64)).to_rfc3339();

        let feedback_up: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM feedback WHERE created_at >= ?1 AND feedback_type = 'up'",
                params![&cutoff],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let feedback_down: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM feedback WHERE created_at >= ?1 AND feedback_type = 'down'",
                params![&cutoff],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let mut stmt = conn.prepare(
            "SELECT content_preview, feedback_type FROM feedback WHERE created_at >= ?1 ORDER BY created_at DESC LIMIT 100",
        )?;
        let rows = stmt.query_map(params![&cutoff], |row| {
            let content: String = row.get(0)?;
            let ft: String = row.get(1)?;
            Ok((content, ft))
        })?;

        let mut up_keywords: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        let mut down_keywords: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        for row in rows {
            let (content, ft) = row?;
            let words = Self::tokenize(&content);
            if ft == "up" {
                for w in words {
                    *up_keywords.entry(w).or_insert(0) += 1;
                }
            } else {
                for w in words {
                    *down_keywords.entry(w).or_insert(0) += 1;
                }
            }
        }

        let mut up_vec: Vec<(String, i32)> = up_keywords.clone().into_iter().collect();
        up_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let top_liked: Vec<String> = up_vec
            .into_iter()
            .take(5)
            .map(|(k, c)| format!("{} ({}次)", k, c))
            .collect();

        let mut down_vec: Vec<(String, i32)> = down_keywords.clone().into_iter().collect();
        down_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let top_disliked: Vec<String> = down_vec
            .into_iter()
            .take(5)
            .map(|(k, c)| format!("{} ({}次)", k, c))
            .collect();

        // Propose value changes without mutating the model
        let mut value_changes = Vec::new();
        for value in &model.identity.values {
            let name_lower = value.name.to_lowercase();
            let up_score = up_keywords.get(&name_lower).copied().unwrap_or(0) as f32;
            let down_score = down_keywords.get(&name_lower).copied().unwrap_or(0) as f32;
            if up_score > down_score && up_score >= 2.0 {
                let new_weight =
                    ((value.weight as f32 + 0.02).clamp(1.0, 10.0) * 100.0).round() / 100.0;
                value_changes.push(format!(
                    "建议提升 '{}' 权重到 {:.2}",
                    value.name, new_weight
                ));
            } else if down_score > up_score && down_score >= 2.0 {
                let new_weight =
                    ((value.weight as f32 - 0.02).clamp(1.0, 10.0) * 100.0).round() / 100.0;
                value_changes.push(format!(
                    "建议降低 '{}' 权重到 {:.2}",
                    value.name, new_weight
                ));
            }
        }

        let mut suggested_actions = Vec::new();
        if feedback_down > feedback_up && feedback_down >= 2 {
            suggested_actions
                .push("近期负面反馈略多，建议在下一次对话中更关注用户的情绪状态。".to_string());
        }
        if !top_liked.is_empty() {
            suggested_actions.push(format!(
                "用户偏好的主题: {}。可以多围绕这些话题展开。",
                top_liked.join(", ")
            ));
        }
        if model.goals.short_term.is_empty() && model.goals.medium_term.is_empty() {
            suggested_actions.push("当前短期与中期目标为空，建议启动构建器更新目标。".to_string());
        }

        let summary = format!(
            "过去 {} 天的数据: 👍 {} 条正面反馈, 👎 {} 条负面反馈。",
            period_days, feedback_up, feedback_down
        );

        Ok(CalibrationReport {
            period_days,
            feedback_up,
            feedback_down,
            top_liked_patterns: top_liked,
            top_disliked_patterns: top_disliked,
            value_changes,
            suggested_actions,
            summary_text: summary,
        })
    }
}
