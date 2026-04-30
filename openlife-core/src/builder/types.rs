use crate::life_model::ValueItem;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Normalize a list line by trimming and removing common bullet/number prefixes.
/// Returns the cleaned line, or empty string if the line is just a bullet.
pub fn normalize_list_line(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Remove common prefixes: -, •, 1., 1), etc.
    let without_prefix = trimmed.trim_start_matches(|c: char| {
        c.is_numeric() || c == '.' || c == ')' || c == '•' || c == '-' || c == '*'
    });
    without_prefix.trim().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuilderMode {
    Quick,
    Incremental,
    Socratic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuilderDimension {
    Identity,
    Goals,
    Capabilities,
    State,
}

impl std::str::FromStr for BuilderDimension {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "identity" => Ok(BuilderDimension::Identity),
            "goals" => Ok(BuilderDimension::Goals),
            "capabilities" => Ok(BuilderDimension::Capabilities),
            "state" => Ok(BuilderDimension::State),
            _ => Err(format!(
                "非法维度 '{}', 可选值: identity, goals, capabilities, state",
                s
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderProgress {
    pub progress: f32,
    pub current_step_label: String,
    pub step_index: usize,
    pub total_steps: usize,
    pub current_session: u8,
    pub waiting_pairwise: bool,
    #[serde(default)]
    pub waiting_phase_confirmation: bool,
    #[serde(default)]
    pub phase_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderAnalysis {
    pub completion: crate::life_model::Model4DCompletion,
    pub gaps: Vec<String>,
}

/// Multi-dimensional signals extracted from a user's peak experience narrative.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PeakExperience {
    pub raw_description: String,
    pub extracted_values: Vec<String>,
    pub extracted_role_hints: Vec<String>,
    pub extracted_capability_hints: Vec<String>,
    pub extracted_preference_hints: Vec<String>,
    pub emotional_signal: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RiskLevel {
    Low,
    #[default]
    Medium,
    High,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
        }
    }
}

/// Signal extracted from builder answers, representing a proposed change to LifeModel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderSignal {
    pub id: String,
    pub source_step: usize,
    pub source_question_id: String,
    pub dimension: BuilderDimension,
    pub affected_path: String,
    pub proposed_value: serde_json::Value,
    pub confidence: f32,
    pub reason: String,
    pub risk_level: RiskLevel,
    pub user_status: SignalUserStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SignalUserStatus {
    #[default]
    Pending,
    Accepted,
    Edited,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderSignalDecision {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedField {
    pub path: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderPatchReview {
    pub signals: Vec<BuilderSignal>,
    pub summary: BuilderSummary,
    pub assumptions: Vec<String>,
    pub uncertain_fields: Vec<String>,
    pub confidence_by_dimension: HashMap<String, f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuilderSummary {
    pub identity_summary: String,
    pub goals_summary: String,
    pub capabilities_summary: String,
    pub state_summary: String,
    pub assumptions: Vec<String>,
    pub unresolved_questions: Vec<String>,
    pub recommended_next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickBuildAnswers {
    pub name: String,
    pub current_focus: String,
    pub short_term_goals: Vec<String>,
    pub long_term_direction: String,
    pub capabilities: Vec<String>,
    pub current_blockers: String,
    pub companion_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderSession {
    pub session_id: String,
    pub mode: BuilderMode,
    pub step_index: usize,
    pub finished: bool,
    pub draft_yaml: String,
    pub analysis: Option<BuilderAnalysis>,
    #[serde(default)]
    pub current_session: u8,
    #[serde(default)]
    pub extracted_values: Vec<ValueItem>,
    #[serde(default)]
    pub pending_pairwise: Vec<(String, String)>,
    #[serde(default)]
    pub pairwise_results: Vec<(String, String, String)>,
    #[serde(default)]
    pub waiting_pairwise: bool,
    #[serde(default)]
    pub current_prompt: String,
    #[serde(default)]
    pub peak_experience: Option<PeakExperience>,
    /// Signals extracted from answers, waiting for user confirmation (Quick Build)
    #[serde(default)]
    pub pending_signals: Vec<BuilderSignal>,
    /// Confirmed signals that have been applied
    #[serde(default)]
    pub confirmed_signals: Vec<BuilderSignal>,
    /// For Incremental mode: which dimension is being built
    #[serde(default)]
    pub target_dimension: Option<BuilderDimension>,
    /// For Socratic mode: waiting for user to confirm phase summary before continuing
    #[serde(default)]
    pub waiting_phase_confirmation: bool,
    /// For Socratic mode: current phase summary text shown to user for confirmation
    #[serde(default)]
    pub phase_summary: Option<String>,
    /// For Socratic mode: the theme chosen by the user
    #[serde(default)]
    pub socratic_theme: Option<String>,
}

impl BuilderSession {
    pub fn new(session_id: impl Into<String>, mode: BuilderMode) -> Self {
        Self {
            session_id: session_id.into(),
            mode,
            step_index: 0,
            finished: false,
            draft_yaml: String::new(),
            analysis: None,
            current_session: 0,
            extracted_values: vec![],
            pending_pairwise: vec![],
            pairwise_results: vec![],
            waiting_pairwise: false,
            current_prompt: String::new(),
            peak_experience: None,
            pending_signals: vec![],
            confirmed_signals: vec![],
            target_dimension: None,
            waiting_phase_confirmation: false,
            phase_summary: None,
            socratic_theme: None,
        }
    }

    pub fn progress(&self) -> BuilderProgress {
        match self.mode {
            BuilderMode::Quick => {
                let total = QUICK_BUILD_STEPS.len();
                let idx = self.step_index.min(total);
                let label = if self.finished {
                    "审阅模型建议".to_string()
                } else {
                    QUICK_BUILD_STEPS
                        .get(idx.saturating_sub(1))
                        .map(|s| quick_step_label(s))
                        .unwrap_or_else(|| "准备生成".to_string())
                };
                BuilderProgress {
                    progress: if self.finished {
                        1.0
                    } else {
                        idx as f32 / total.max(1) as f32
                    },
                    current_step_label: label,
                    step_index: idx,
                    total_steps: total,
                    current_session: 0,
                    waiting_pairwise: false,
                    waiting_phase_confirmation: false,
                    phase_summary: None,
                }
            }
            BuilderMode::Socratic => {
                const MAX_TURNS: usize = 8;
                let idx = self.step_index.min(MAX_TURNS);
                let session_num = self.current_session.clamp(1, 4);
                let session_labels = [
                    "",
                    "价值观与峰值体验",
                    "角色与使命",
                    "目标系统",
                    "能力与缺口",
                ];
                let label = if self.waiting_pairwise {
                    format!("会话 {}/4：价值排序（两两比较）", session_num)
                } else {
                    format!(
                        "会话 {}/4：{}",
                        session_num,
                        session_labels
                            .get(session_num as usize)
                            .unwrap_or(&"深入探讨")
                    )
                };
                BuilderProgress {
                    progress: idx as f32 / MAX_TURNS as f32,
                    current_step_label: label,
                    step_index: idx,
                    total_steps: MAX_TURNS,
                    current_session: session_num,
                    waiting_pairwise: self.waiting_pairwise,
                    waiting_phase_confirmation: self.waiting_phase_confirmation,
                    phase_summary: self.phase_summary.clone(),
                }
            }
            BuilderMode::Incremental => {
                let (total_steps, label) = match self.target_dimension {
                    Some(BuilderDimension::Identity) => (5, "Identity 身份构建"),
                    Some(BuilderDimension::Goals) => (4, "Goals 目标构建"),
                    Some(BuilderDimension::Capabilities) => (4, "Capabilities 能力构建"),
                    Some(BuilderDimension::State) => (4, "State 状态构建"),
                    None => (1, "选择构建维度"),
                };
                let idx = self.step_index.min(total_steps);
                let current_label = if self.finished {
                    "审阅模型建议".to_string()
                } else {
                    format!("{} (问题 {}/{})", label, idx.saturating_add(1), total_steps)
                };
                BuilderProgress {
                    progress: if self.finished {
                        1.0
                    } else {
                        idx as f32 / total_steps as f32
                    },
                    current_step_label: current_label,
                    step_index: idx,
                    total_steps,
                    current_session: 0,
                    waiting_pairwise: false,
                    waiting_phase_confirmation: false,
                    phase_summary: None,
                }
            }
        }
    }
}

fn quick_step_label(step: &str) -> String {
    match step {
        "name" => "称呼".to_string(),
        "current_focus" => "当前人生主题".to_string(),
        "short_term_goals" => "近期目标".to_string(),
        "long_term_direction" => "长期方向".to_string(),
        "capabilities" => "已有能力".to_string(),
        "current_blockers" => "当前卡点".to_string(),
        "companion_style" => "陪伴风格".to_string(),
        _ => "构建中".to_string(),
    }
}

/// Quick build: 7 steps aligned with design doc
/// Step 1: name (identity.name)
/// Step 2: current_focus (state.current_focus)  
/// Step 3: short_term_goals (goals.short_term)
/// Step 4: long_term_direction (goals.long_term, identity.mission) - HIGH RISK
/// Step 5: capabilities (capabilities.skills, resources)
/// Step 6: current_blockers (state.emotional_state, alerts)
/// Step 7: companion_style (identity.voice_style, preferences.communication_style)
pub(crate) const QUICK_BUILD_STEPS: &[&str] = &[
    "name",
    "current_focus",
    "short_term_goals",
    "long_term_direction",
    "capabilities",
    "current_blockers",
    "companion_style",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_session_new_and_progress_quick() {
        let session = BuilderSession::new("s1", BuilderMode::Quick);
        assert_eq!(session.session_id, "s1");
        assert_eq!(session.mode, BuilderMode::Quick);
        assert_eq!(session.step_index, 0);

        let p = session.progress();
        assert_eq!(p.total_steps, QUICK_BUILD_STEPS.len());
        assert_eq!(p.progress, 0.0);
    }

    #[test]
    fn builder_session_progress_socratic() {
        let mut session = BuilderSession::new("s2", BuilderMode::Socratic);
        session.step_index = 4;
        session.current_session = 2;
        session.waiting_pairwise = true;
        let p = session.progress();
        assert_eq!(p.total_steps, 8);
        assert!(p.current_step_label.contains("价值排序"));
        assert_eq!(p.current_session, 2);
        assert!(p.waiting_pairwise);
    }

    #[test]
    fn builder_session_progress_incremental() {
        let session = BuilderSession::new("s3", BuilderMode::Incremental);
        let p = session.progress();
        assert_eq!(p.current_step_label, "选择构建维度 (问题 1/1)");
        assert_eq!(p.total_steps, 1);
    }
}
