use anyhow::{Context, Result};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Metadata {
    pub version: String,
    pub created_at: String,
    pub updated_at: String,
    pub author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Identity {
    pub name: String,
    pub birth_date: Option<String>,
    pub values: Vec<ValueItem>,
    pub personality_traits: Vec<PersonalityTrait>,
    pub life_philosophy: String,
    pub mission_statement: String,
    pub role_definition: RoleDefinition,
    pub voice_style: VoiceStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ValueItem {
    pub name: String,
    pub weight: u8,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PersonalityTrait {
    pub trait_name: String,
    pub score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RoleDefinition {
    #[serde(alias = "professional")]
    pub primary_role: String,
    #[serde(
        alias = "personal",
        deserialize_with = "deserialize_string_or_vec",
        default
    )]
    pub secondary_roles: Vec<String>,
    #[serde(default)]
    pub responsibilities: Vec<String>,
    #[serde(default)]
    pub boundaries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VoiceStyle {
    pub tone_descriptors: Vec<String>,
    #[serde(alias = "formality_level")]
    pub formality: FormalityLevel,
    #[serde(default)]
    pub vocabulary_preference: String,
    pub emoji_usage: EmojiUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FormalityLevel {
    #[default]
    Casual,
    Neutral,
    Formal,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmojiUsage {
    #[default]
    #[serde(alias = "none")]
    Never,
    #[serde(alias = "minimal")]
    Sparingly,
    #[serde(alias = "moderate", alias = "frequent")]
    Often,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Goals {
    pub short_term: Vec<GoalItem>,
    pub medium_term: Vec<GoalItem>,
    pub long_term: Vec<GoalItem>,
    pub life_goals: Vec<GoalItem>,
    pub daily: Vec<DailyGoal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct GoalItem {
    pub name: String,
    pub description: String,
    pub priority: u8,
    pub status: String,
    pub progress: f32,
    pub deadline: Option<String>,
    pub milestones: Vec<Milestone>,
    pub related_memories: Vec<String>,
    /// Last time this goal was updated (RFC 3339 timestamp)
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Milestone {
    pub name: String,
    pub achieved: bool,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DailyGoal {
    pub name: String,
    #[serde(alias = "completed")]
    pub done: bool,
    pub time_block: Option<TimeBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TimeBlock {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Capabilities {
    pub skills: Vec<Skill>,
    pub resources: Vec<Resource>,
    pub networks: Vec<String>,
    pub tools: Vec<ToolCapability>,
    pub knowledge_domains: Vec<KnowledgeDomain>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Skill {
    pub name: String,
    pub proficiency: u8,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Resource {
    pub name: String,
    #[serde(alias = "type")]
    pub resource_type: String,
    pub description: String,
    pub availability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ToolCapability {
    pub name: String,
    pub proficiency: u8,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KnowledgeDomain {
    #[serde(alias = "name")]
    pub domain: String,
    #[serde(alias = "proficiency")]
    pub level: u8,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct State {
    pub current_focus: String,
    pub health_status: HealthStatus,
    pub emotional_state: EmotionalState,
    pub recent_reflections: Vec<Reflection>,
    pub open_questions: Vec<String>,
    pub focus_areas: Vec<String>,
    pub recent_events: Vec<String>,
    pub habit_streaks: Vec<HabitStreak>,
    pub custom_dimensions: Vec<CustomStateDimension>,
    pub alerts: Vec<StateAlert>,
    /// Last time the state was explicitly checked in (RFC 3339 timestamp)
    #[serde(default)]
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CustomStateDimension {
    pub name: String,
    pub current_value: f32,
    pub unit: String,
    pub min_threshold: Option<f32>,
    pub max_threshold: Option<f32>,
    /// 连续 N 天超出阈值时触发预警
    pub alert_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StateAlert {
    pub dimension_name: String,
    pub level: AlertLevel,
    pub message: String,
    pub triggered_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AlertLevel {
    #[default]
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HealthStatus {
    pub physical: String,
    pub mental: String,
    pub energy_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EmotionalState {
    pub current_mood: String,
    pub stress_level: u8,
    pub fulfillment_score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Reflection {
    pub date: String,
    pub content: String,
    pub insights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HabitStreak {
    pub name: String,
    pub streak_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Relationships {
    #[serde(default)]
    pub inner_circle: Vec<Relationship>,
    #[serde(default)]
    pub mentors: Vec<Relationship>,
    #[serde(default)]
    pub collaborators: Vec<Relationship>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Relationship {
    pub name: String,
    pub relationship_type: String,
    pub importance: u8,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Preferences {
    pub work_hours: WorkHours,
    #[serde(
        alias = "peak_productivity_times",
        deserialize_with = "deserialize_first_string_or_vec",
        default
    )]
    pub peak_energy_time: String,
    #[serde(
        alias = "notification_preferences",
        deserialize_with = "deserialize_first_string_or_vec",
        default
    )]
    pub communication_style: String,
    #[serde(default)]
    pub learning_style: String,
    #[serde(default)]
    pub decision_making_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WorkHours {
    pub preferred_start: String,
    pub preferred_end: String,
    pub timezone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Model4DCompletion {
    pub identity: u8,
    pub goals: u8,
    pub capabilities: u8,
    pub state: u8,
    pub overall: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LifeModel {
    pub metadata: Metadata,
    pub identity: Identity,
    pub goals: Goals,
    pub capabilities: Capabilities,
    pub state: State,
    pub relationships: Relationships,
    pub preferences: Preferences,
    pub evolution_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LifeModelCompatibilitySummary {
    pub summary: String,
    pub source_asset_ids: Vec<String>,
    pub content_digest: String,
}

impl Default for LifeModelCompatibilitySummary {
    fn default() -> Self {
        Self::new("", Vec::new())
    }
}

impl LifeModelCompatibilitySummary {
    pub fn new(summary: impl Into<String>, source_asset_ids: Vec<String>) -> Self {
        let summary = summary.into();
        let content_digest = sha256_hex(
            serde_json::json!({
                "summary": summary,
                "source_asset_ids": source_asset_ids,
            })
            .to_string()
            .as_bytes(),
        );
        Self {
            summary,
            source_asset_ids,
            content_digest,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LifeModelCompatibilityStateSummary {
    pub current_focus: String,
    pub current_mood: String,
    pub energy_level: u8,
    pub last_updated: Option<String>,
    pub content_digest: String,
}

impl LifeModelCompatibilityStateSummary {
    fn from_life_model(model: &LifeModel) -> Self {
        let current_focus = model.state.current_focus.clone();
        let current_mood = model.state.emotional_state.current_mood.clone();
        let energy_level = model.state.health_status.energy_level;
        let last_updated = model.state.last_updated.clone();
        let content_digest = sha256_hex(
            serde_json::json!({
                "current_focus": current_focus,
                "current_mood": current_mood,
                "energy_level": energy_level,
                "last_updated": last_updated,
            })
            .to_string()
            .as_bytes(),
        );
        Self {
            current_focus,
            current_mood,
            energy_level,
            last_updated,
            content_digest,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LifeModelCompatibilityAssetRef {
    pub asset_kind: String,
    pub asset_id: String,
    pub affected_path: String,
    pub source_ids: Vec<String>,
    pub content_digest: String,
}

impl LifeModelCompatibilityAssetRef {
    fn from_evidence(record: &crate::agent::evidence_store::EvidenceRecord) -> Self {
        let mut source_ids = record
            .source_refs
            .iter()
            .map(|source| source.source_id.clone())
            .collect::<Vec<_>>();
        source_ids.extend(record.linked_proposal_ids.iter().cloned());
        source_ids.extend(record.linked_agent_run_ids.iter().cloned());
        source_ids.sort();
        source_ids.dedup();

        let content_digest = sha256_hex(
            serde_json::json!({
                "asset_kind": "evidence",
                "asset_id": record.id,
                "evidence_type": record.evidence_type.to_string(),
                "affected_path": record.affected_path,
                "status": record.status.to_string(),
                "confidence": record.confidence,
                "source_digests": record
                    .source_refs
                    .iter()
                    .map(|source| source.digest.clone())
                    .collect::<Vec<_>>(),
            })
            .to_string()
            .as_bytes(),
        );

        Self {
            asset_kind: "evidence".into(),
            asset_id: record.id.clone(),
            affected_path: record.affected_path.clone(),
            source_ids,
            content_digest,
        }
    }

    fn from_heuristic(record: &crate::agent::heuristic_store::HeuristicRecord) -> Self {
        let mut source_ids = record.evidence_refs.clone();
        if let Some(source_proposal_id) = record.source_proposal_id.clone() {
            source_ids.push(source_proposal_id);
        }
        source_ids.sort();
        source_ids.dedup();

        let affected_path = format!("heuristics.{}.{}", record.domain, record.trigger);
        let content_digest = sha256_hex(
            serde_json::json!({
                "asset_kind": "heuristic",
                "asset_id": record.id,
                "domain": record.domain,
                "trigger": record.trigger,
                "priority": record.priority,
                "status": record.status.to_string(),
                "validation_state": record.validation_state,
                "version": record.version,
            })
            .to_string()
            .as_bytes(),
        );

        Self {
            asset_kind: "heuristic".into(),
            asset_id: record.id.clone(),
            affected_path,
            source_ids,
            content_digest,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LifeModelHSCompatibilityView {
    pub schema_version: String,
    pub materialized_at: String,
    pub current_state_summary: LifeModelCompatibilityStateSummary,
    pub collaboration_summaries: Vec<LifeModelCompatibilitySummary>,
    pub asset_refs: Vec<LifeModelCompatibilityAssetRef>,
    pub source_digest: String,
}

#[derive(Serialize)]
struct LifeModelCompatibilityEnvelope<'a> {
    #[serde(flatten)]
    life_model: &'a LifeModel,
    hs_compatibility: LifeModelHSCompatibilityView,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AlignmentIssue {
    pub goal_name: String,
    pub severity: String,
    pub related_values: Vec<String>,
    pub reason: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CapabilityGap {
    pub goal_name: String,
    pub skill_name: String,
    pub current_level: u8,
    pub target_level: u8,
    pub severity: String,
    pub suggestion: String,
}

fn deserialize_string_or_vec<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        One(String),
        Many(Vec<String>),
    }

    let value = Option::<StringOrVec>::deserialize(deserializer)?;
    Ok(match value {
        Some(StringOrVec::One(v)) if !v.is_empty() => vec![v],
        Some(StringOrVec::Many(v)) => v,
        _ => Vec::new(),
    })
}

fn deserialize_first_string_or_vec<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        One(String),
        Many(Vec<String>),
    }

    let value = Option::<StringOrVec>::deserialize(deserializer)?;
    Ok(match value {
        Some(StringOrVec::One(v)) => v,
        Some(StringOrVec::Many(v)) => v.into_iter().next().unwrap_or_default(),
        None => String::new(),
    })
}

impl LifeModel {
    pub fn default_model() -> Self {
        let now = chrono::Local::now().to_rfc3339();
        Self {
            metadata: Metadata {
                version: "0.1.0".to_string(),
                created_at: now.clone(),
                updated_at: now,
                author: "".to_string(),
            },
            identity: Identity {
                name: "".to_string(),
                birth_date: None,
                values: vec![],
                personality_traits: vec![],
                life_philosophy: "".to_string(),
                mission_statement: "".to_string(),
                role_definition: RoleDefinition::default(),
                voice_style: VoiceStyle::default(),
            },
            goals: Goals {
                short_term: vec![],
                medium_term: vec![],
                long_term: vec![],
                life_goals: vec![],
                daily: vec![],
            },
            capabilities: Capabilities {
                skills: vec![],
                resources: vec![],
                networks: vec![],
                tools: vec![],
                knowledge_domains: vec![],
            },
            state: State {
                current_focus: "构建人生模型".to_string(),
                health_status: HealthStatus {
                    physical: "良好".to_string(),
                    mental: "积极".to_string(),
                    energy_level: 7,
                },
                emotional_state: EmotionalState {
                    current_mood: "期待".to_string(),
                    stress_level: 3,
                    fulfillment_score: 6,
                },
                recent_reflections: vec![],
                open_questions: vec![],
                focus_areas: vec![],
                recent_events: vec![],
                habit_streaks: vec![],
                custom_dimensions: vec![],
                alerts: vec![],
                last_updated: None,
            },
            relationships: Relationships::default(),
            preferences: Preferences::default(),
            evolution_rules: vec![],
        }
    }

    pub fn materialize_yaml_compatibility_view(
        &self,
        collaboration_summaries: Vec<LifeModelCompatibilitySummary>,
        evidence_records: &[crate::agent::evidence_store::EvidenceRecord],
        heuristic_records: &[crate::agent::heuristic_store::HeuristicRecord],
    ) -> Result<String> {
        let current_state_summary = LifeModelCompatibilityStateSummary::from_life_model(self);
        let mut asset_refs = evidence_records
            .iter()
            .map(LifeModelCompatibilityAssetRef::from_evidence)
            .chain(
                heuristic_records
                    .iter()
                    .map(LifeModelCompatibilityAssetRef::from_heuristic),
            )
            .collect::<Vec<_>>();
        asset_refs.sort_by(|left, right| {
            left.asset_kind
                .cmp(&right.asset_kind)
                .then_with(|| left.asset_id.cmp(&right.asset_id))
        });

        let source_digest = sha256_hex(
            serde_json::json!({
                "life_model_version": self.metadata.version,
                "life_model_updated_at": self.metadata.updated_at,
                "state_digest": current_state_summary.content_digest,
                "summary_digests": collaboration_summaries
                    .iter()
                    .map(|summary| summary.content_digest.clone())
                    .collect::<Vec<_>>(),
                "asset_digests": asset_refs
                    .iter()
                    .map(|asset| asset.content_digest.clone())
                    .collect::<Vec<_>>(),
            })
            .to_string()
            .as_bytes(),
        );

        let envelope = LifeModelCompatibilityEnvelope {
            life_model: self,
            hs_compatibility: LifeModelHSCompatibilityView {
                schema_version: "lifemodel_hs_compat_v1".into(),
                materialized_at: chrono::Utc::now().to_rfc3339(),
                current_state_summary,
                collaboration_summaries,
                asset_refs,
                source_digest,
            },
        };
        serde_yaml::to_string(&envelope).with_context(|| "序列化 LifeModel HS 兼容视图失败")
    }

    pub fn is_effectively_empty(&self) -> bool {
        let default = Self::default_model();

        self.identity.name.trim().is_empty()
            && self.identity.birth_date.is_none()
            && self.identity.values.is_empty()
            && self.identity.personality_traits.is_empty()
            && self.identity.life_philosophy.trim().is_empty()
            && self.identity.mission_statement.trim().is_empty()
            && self.identity.role_definition.primary_role.trim().is_empty()
            && self.identity.role_definition.secondary_roles.is_empty()
            && self.identity.role_definition.responsibilities.is_empty()
            && self.identity.role_definition.boundaries.is_empty()
            && self.identity.voice_style.tone_descriptors.is_empty()
            && self
                .identity
                .voice_style
                .vocabulary_preference
                .trim()
                .is_empty()
            && self.goals.short_term.is_empty()
            && self.goals.medium_term.is_empty()
            && self.goals.long_term.is_empty()
            && self.goals.life_goals.is_empty()
            && self.goals.daily.is_empty()
            && self.capabilities.skills.is_empty()
            && self.capabilities.resources.is_empty()
            && self.capabilities.networks.is_empty()
            && self.capabilities.tools.is_empty()
            && self.capabilities.knowledge_domains.is_empty()
            && self.state.current_focus == default.state.current_focus
            && self.state.health_status.physical == default.state.health_status.physical
            && self.state.health_status.mental == default.state.health_status.mental
            && self.state.health_status.energy_level == default.state.health_status.energy_level
            && self.state.emotional_state.current_mood == default.state.emotional_state.current_mood
            && self.state.emotional_state.stress_level == default.state.emotional_state.stress_level
            && self.state.emotional_state.fulfillment_score
                == default.state.emotional_state.fulfillment_score
            && self.state.recent_reflections.is_empty()
            && self.state.open_questions.is_empty()
            && self.state.focus_areas.is_empty()
            && self.state.recent_events.is_empty()
            && self.state.habit_streaks.is_empty()
            && self.state.custom_dimensions.is_empty()
            && self.state.alerts.is_empty()
            && self.relationships.inner_circle.is_empty()
            && self.relationships.mentors.is_empty()
            && self.relationships.collaborators.is_empty()
            && self
                .preferences
                .work_hours
                .preferred_start
                .trim()
                .is_empty()
            && self.preferences.work_hours.preferred_end.trim().is_empty()
            && self.preferences.work_hours.timezone.trim().is_empty()
            && self.preferences.peak_energy_time.trim().is_empty()
            && self.preferences.communication_style.trim().is_empty()
            && self.preferences.learning_style.trim().is_empty()
            && self.preferences.decision_making_style.trim().is_empty()
            && self.evolution_rules.is_empty()
    }

    pub fn calculate_4d_completion(&self) -> Model4DCompletion {
        let identity = {
            let mut score = 0u8;
            if !self.identity.name.is_empty() {
                score += 10;
            }
            if self.identity.birth_date.is_some() {
                score += 5;
            }
            if !self.identity.values.is_empty() {
                score += 20;
            }
            if !self.identity.personality_traits.is_empty() {
                score += 15;
            }
            if !self.identity.life_philosophy.is_empty() {
                score += 10;
            }
            if !self.identity.mission_statement.is_empty() {
                score += 20;
            }
            if !self.identity.role_definition.primary_role.is_empty() {
                score += 10;
            }
            if !self.identity.voice_style.tone_descriptors.is_empty() {
                score += 10;
            }
            score.min(100)
        };
        let goals = {
            let total = self.goals.short_term.len()
                + self.goals.medium_term.len()
                + self.goals.long_term.len()
                + self.goals.life_goals.len()
                + self.goals.daily.len();
            let mut score = (total as u8) * 8;
            if score > 100 {
                score = 100;
            }
            score
        };
        let capabilities = {
            let mut score = 0u8;
            if !self.capabilities.skills.is_empty() {
                score += 30;
            }
            if !self.capabilities.resources.is_empty() {
                score += 20;
            }
            if !self.capabilities.networks.is_empty() {
                score += 20;
            }
            if !self.capabilities.tools.is_empty() {
                score += 15;
            }
            if !self.capabilities.knowledge_domains.is_empty() {
                score += 15;
            }
            score
        };
        let state = {
            let mut score = 0u8;
            if !self.state.current_focus.is_empty() {
                score += 15;
            }
            if !self.state.health_status.physical.is_empty() {
                score += 15;
            }
            if !self.state.emotional_state.current_mood.is_empty() {
                score += 20;
            }
            if !self.state.recent_reflections.is_empty() {
                score += 10;
            }
            if !self.state.focus_areas.is_empty() {
                score += 15;
            }
            if !self.state.recent_events.is_empty() {
                score += 10;
            }
            if !self.state.habit_streaks.is_empty() {
                score += 15;
            }
            score
        };
        let overall = (identity / 4) + (goals / 4) + (capabilities / 4) + (state / 4);
        Model4DCompletion {
            identity,
            goals,
            capabilities,
            state,
            overall,
        }
    }

    /// 检查高优先级目标是否与核心价值观存在潜在冲突
    pub fn identity_goal_alignment_report(&self) -> Vec<AlignmentIssue> {
        let mut conflicts = Vec::new();
        let top_values: Vec<String> = self
            .identity
            .values
            .iter()
            .filter(|v| v.weight >= 7)
            .map(|v| v.name.to_lowercase())
            .collect();
        if top_values.is_empty() {
            return conflicts;
        }

        let all_goals: Vec<&GoalItem> = self
            .goals
            .short_term
            .iter()
            .chain(self.goals.medium_term.iter())
            .chain(self.goals.long_term.iter())
            .chain(self.goals.life_goals.iter())
            .filter(|g| g.priority >= 7)
            .collect();

        for goal in all_goals {
            let goal_text = format!("{} {}", goal.name, goal.description).to_lowercase();
            let mut aligned = false;
            for value in &top_values {
                if goal_text.contains(value) {
                    aligned = true;
                    break;
                }
            }
            if !aligned {
                let core_values: Vec<String> = self
                    .identity
                    .values
                    .iter()
                    .filter(|v| v.weight >= 7)
                    .map(|v| v.name.clone())
                    .collect();
                conflicts.push(AlignmentIssue {
                    goal_name: goal.name.clone(),
                    severity: if goal.priority >= 9 {
                        "high".into()
                    } else {
                        "medium".into()
                    },
                    related_values: core_values.clone(),
                    reason: format!("目标描述中未体现核心价值观（{}）", core_values.join(", ")),
                    suggestion: "补充目标背后的动机，或调整目标描述，使其与高权重价值观更一致。"
                        .into(),
                });
            }
        }
        conflicts
    }

    pub fn identity_goal_alignment_check(&self) -> Vec<String> {
        self.identity_goal_alignment_report()
            .into_iter()
            .map(|issue| {
                format!(
                    "目标 '{}' {}，建议：{}",
                    issue.goal_name, issue.reason, issue.suggestion
                )
            })
            .collect()
    }

    /// 对比目标所需能力与当前能力，返回缺口列表
    pub fn goal_capability_gap_report(&self) -> Vec<CapabilityGap> {
        let mut gaps = Vec::new();
        let skills = &self.capabilities.skills;
        let skill_names: Vec<String> = skills.iter().map(|s| s.name.to_lowercase()).collect();
        let all_goals: Vec<&GoalItem> = self
            .goals
            .short_term
            .iter()
            .chain(self.goals.medium_term.iter())
            .chain(self.goals.long_term.iter())
            .chain(self.goals.life_goals.iter())
            .collect();
        for goal in &all_goals {
            let text = format!("{} {}", goal.name, goal.description).to_lowercase();
            for skill in &skill_names {
                if text.contains(skill) {
                    if let Some(s) = skills.iter().find(|s| s.name.to_lowercase() == *skill) {
                        if s.proficiency < 6 {
                            gaps.push(CapabilityGap {
                                goal_name: goal.name.clone(),
                                skill_name: s.name.clone(),
                                current_level: s.proficiency,
                                target_level: 7,
                                severity: if s.proficiency <= 3 { "high".into() } else { "medium".into() },
                                suggestion: format!("围绕 '{}' 制定 2-4 周的刻意练习计划，并为 '{}' 设置一个可验证里程碑。", s.name, goal.name),
                            });
                        }
                    }
                }
            }
        }
        if gaps.is_empty() && !all_goals.is_empty() && skills.len() < 3 {
            gaps.push(CapabilityGap {
                goal_name: "整体目标体系".into(),
                skill_name: "能力画像".into(),
                current_level: skills.len() as u8,
                target_level: 3,
                severity: "low".into(),
                suggestion:
                    "能力记录较少，建议先补充更多技能、工具或知识域，方便系统做更准确的差距分析。"
                        .into(),
            });
        }
        gaps
    }

    pub fn goal_capability_gap_analysis(&self) -> Vec<String> {
        self.goal_capability_gap_report()
            .into_iter()
            .map(|gap| {
                format!(
                    "目标 '{}' 涉及能力 '{}'，当前水平 {}/10，目标水平 {}/10。建议：{}",
                    gap.goal_name,
                    gap.skill_name,
                    gap.current_level,
                    gap.target_level,
                    gap.suggestion
                )
            })
            .collect()
    }

    /// Apply a single patch to this LifeModel.
    pub fn apply_patch(
        &mut self,
        patch: &crate::life_model::patch::LifeModelPatch,
    ) -> Result<crate::life_model::patch::PatchApplyResult, crate::life_model::patch::PatchError>
    {
        use crate::life_model::patch::{PatchApplyResult, PatchError, PatchOp};

        // 1. Serialize current model to Value
        let mut value =
            serde_json::to_value(&self).map_err(|e| PatchError::Serialization(e.to_string()))?;

        // 2. Validate path exists
        let target = value
            .pointer(&patch.path_pointer)
            .ok_or_else(|| PatchError::InvalidPath(patch.path_pointer.clone()))?;

        // 3. Validate before value matches (optimistic locking)
        if let Some(ref expected_before) = patch.before {
            if target != expected_before {
                return Err(PatchError::BeforeMismatch {
                    expected: expected_before.clone(),
                    actual: target.clone(),
                });
            }
        }

        // 4. Apply operation
        let result = match patch.operation {
            PatchOp::Replace => {
                // Replace value at path
                if let Some(parent_path) = get_parent_pointer(&patch.path_pointer) {
                    let key = get_last_segment(&patch.path_pointer)
                        .ok_or_else(|| PatchError::InvalidPath(patch.path_pointer.clone()))?;

                    if let Some(parent) = value.pointer_mut(&parent_path) {
                        if let Some(obj) = parent.as_object_mut() {
                            obj.insert(key.to_string(), patch.after.clone());
                        } else if let Some(arr) = parent.as_array_mut() {
                            if let Ok(index) = key.parse::<usize>() {
                                if index < arr.len() {
                                    arr[index] = patch.after.clone();
                                } else {
                                    return Err(PatchError::IndexOutOfBounds {
                                        index,
                                        len: arr.len(),
                                    });
                                }
                            } else {
                                return Err(PatchError::InvalidOperation {
                                    op: patch.operation,
                                    reason: format!("Invalid array index: {}", key),
                                });
                            }
                        } else {
                            return Err(PatchError::InvalidOperation {
                                op: patch.operation,
                                reason: "Parent is neither object nor array".to_string(),
                            });
                        }
                    }
                } else {
                    // Root replacement
                    value = patch.after.clone();
                }
                Ok(())
            }
            PatchOp::Merge => {
                // Shallow merge for objects
                if let Some(target_mut) = value.pointer_mut(&patch.path_pointer) {
                    if let Some(obj) = target_mut.as_object_mut() {
                        if let Some(after_obj) = patch.after.as_object() {
                            for (k, v) in after_obj {
                                obj.insert(k.clone(), v.clone());
                            }
                            Ok(())
                        } else {
                            Err(PatchError::InvalidOperation {
                                op: patch.operation,
                                reason: "Merge requires object value".to_string(),
                            })
                        }
                    } else {
                        Err(PatchError::InvalidOperation {
                            op: patch.operation,
                            reason: "Merge requires object target".to_string(),
                        })
                    }
                } else {
                    Err(PatchError::InvalidPath(patch.path_pointer.clone()))
                }
            }
            PatchOp::Append => {
                // Append to array
                if let Some(parent_path) = get_parent_pointer(&patch.path_pointer) {
                    let parent = value
                        .pointer_mut(&parent_path)
                        .ok_or(PatchError::InvalidPath(parent_path))?;

                    if let Some(arr) = parent.as_array_mut() {
                        arr.push(patch.after.clone());
                        Ok(())
                    } else {
                        Err(PatchError::InvalidOperation {
                            op: patch.operation,
                            reason: "Append requires array parent".to_string(),
                        })
                    }
                } else {
                    Err(PatchError::InvalidOperation {
                        op: patch.operation,
                        reason: "Append requires array path".to_string(),
                    })
                }
            }
            PatchOp::Insert => {
                // Insert at specific array index
                if let Some(parent_path) = get_parent_pointer(&patch.path_pointer) {
                    let key = get_last_segment(&patch.path_pointer)
                        .ok_or_else(|| PatchError::InvalidPath(patch.path_pointer.clone()))?;

                    let parent = value
                        .pointer_mut(&parent_path)
                        .ok_or(PatchError::InvalidPath(parent_path))?;

                    if let Some(arr) = parent.as_array_mut() {
                        if let Ok(index) = key.parse::<usize>() {
                            if index <= arr.len() {
                                arr.insert(index, patch.after.clone());
                                Ok(())
                            } else {
                                Err(PatchError::IndexOutOfBounds {
                                    index,
                                    len: arr.len(),
                                })
                            }
                        } else {
                            Err(PatchError::InvalidOperation {
                                op: patch.operation,
                                reason: format!("Invalid array index: {}", key),
                            })
                        }
                    } else {
                        Err(PatchError::InvalidOperation {
                            op: patch.operation,
                            reason: "Insert requires array parent".to_string(),
                        })
                    }
                } else {
                    Err(PatchError::InvalidOperation {
                        op: patch.operation,
                        reason: "Insert requires array path".to_string(),
                    })
                }
            }
            PatchOp::Delete => {
                // Delete element
                if let Some(parent_path) = get_parent_pointer(&patch.path_pointer) {
                    let key = get_last_segment(&patch.path_pointer)
                        .ok_or_else(|| PatchError::InvalidPath(patch.path_pointer.clone()))?;

                    let parent = value
                        .pointer_mut(&parent_path)
                        .ok_or(PatchError::InvalidPath(parent_path))?;

                    if let Some(obj) = parent.as_object_mut() {
                        obj.remove(&key);
                        Ok(())
                    } else if let Some(arr) = parent.as_array_mut() {
                        if let Ok(index) = key.parse::<usize>() {
                            if index < arr.len() {
                                arr.remove(index);
                                Ok(())
                            } else {
                                Err(PatchError::IndexOutOfBounds {
                                    index,
                                    len: arr.len(),
                                })
                            }
                        } else {
                            Err(PatchError::InvalidOperation {
                                op: patch.operation,
                                reason: format!("Invalid array index: {}", key),
                            })
                        }
                    } else {
                        Err(PatchError::InvalidOperation {
                            op: patch.operation,
                            reason: "Delete requires object or array parent".to_string(),
                        })
                    }
                } else {
                    Err(PatchError::InvalidOperation {
                        op: patch.operation,
                        reason: "Cannot delete root".to_string(),
                    })
                }
            }
        };

        if let Err(e) = result {
            return Ok(PatchApplyResult {
                patch_id: patch.id.clone(),
                success: false,
                path: patch.path_pointer.clone(),
                operation: patch.operation.to_string(),
                error: Some(e.to_string()),
            });
        }

        // 5. Deserialize back to LifeModel
        let new_model: LifeModel =
            serde_json::from_value(value).map_err(|e| PatchError::Serialization(e.to_string()))?;

        // 6. Validate
        let report = new_model.validate();
        if !report.errors.is_empty() {
            return Ok(PatchApplyResult {
                patch_id: patch.id.clone(),
                success: false,
                path: patch.path_pointer.clone(),
                operation: patch.operation.to_string(),
                error: Some(format!("Validation failed: {}", report.errors.join(", "))),
            });
        }

        // 7. Apply changes
        *self = new_model;

        Ok(PatchApplyResult {
            patch_id: patch.id.clone(),
            success: true,
            path: patch.path_pointer.clone(),
            operation: patch.operation.to_string(),
            error: None,
        })
    }

    /// Apply multiple patches with dependency analysis and transaction support.
    pub fn apply_patches(
        &mut self,
        patches: &[crate::life_model::patch::LifeModelPatch],
        policy: &crate::life_model::patch::PatchBatchPolicy,
    ) -> Result<crate::life_model::patch::BatchApplyResult, crate::life_model::patch::PatchError>
    {
        use crate::agent::types::RiskLevel;
        use crate::life_model::patch::*;

        // Create checkpoint for rollback
        let checkpoint = self.clone();
        let mut applied = Vec::new();
        let mut skipped = Vec::new();

        // Detect conflicts
        let conflicts = if policy.detect_dependencies {
            detect_conflicts(patches)
        } else {
            vec![]
        };

        // Auto-resolve low-risk conflicts
        let (accepted_ids, rejected_ids, pending_review) = if policy.auto_resolve_low_risk {
            auto_resolve_conflicts(patches, &conflicts)
        } else {
            (
                patches.iter().map(|p| p.id.clone()).collect(),
                vec![],
                conflicts.clone(),
            )
        };

        // Filter patches
        let patches_to_apply: Vec<_> = patches
            .iter()
            .filter(|p| accepted_ids.contains(&p.id) && !rejected_ids.contains(&p.id))
            .collect();

        // Apply patches in order
        for patch in &patches_to_apply {
            match self.apply_patch(patch) {
                Ok(result) => {
                    if result.success {
                        applied.push(result);
                    } else {
                        match policy.failure_mode {
                            FailureMode::Atomic => {
                                // Rollback on any failure
                                *self = checkpoint;
                                return Ok(BatchApplyResult {
                                    applied,
                                    skipped,
                                    pending_review,
                                    rolled_back: true,
                                    error: result.error,
                                });
                            }
                            FailureMode::Partial => {
                                // Skip this patch, continue
                                skipped.push(patch.id.clone());
                            }
                            FailureMode::Adaptive => {
                                if patch.risk_level == RiskLevel::Critical {
                                    // Rollback on critical failure
                                    *self = checkpoint;
                                    return Ok(BatchApplyResult {
                                        applied,
                                        skipped,
                                        pending_review,
                                        rolled_back: true,
                                        error: result.error,
                                    });
                                } else {
                                    // Skip non-critical failure
                                    skipped.push(patch.id.clone());
                                }
                            }
                        }
                    }
                }
                Err(e) => match policy.failure_mode {
                    FailureMode::Atomic => {
                        *self = checkpoint;
                        return Ok(BatchApplyResult {
                            applied,
                            skipped,
                            pending_review,
                            rolled_back: true,
                            error: Some(e.to_string()),
                        });
                    }
                    FailureMode::Partial => {
                        skipped.push(patch.id.clone());
                    }
                    FailureMode::Adaptive => {
                        if patch.risk_level == RiskLevel::Critical {
                            *self = checkpoint;
                            return Ok(BatchApplyResult {
                                applied,
                                skipped,
                                pending_review,
                                rolled_back: true,
                                error: Some(e.to_string()),
                            });
                        } else {
                            skipped.push(patch.id.clone());
                        }
                    }
                },
            }
        }

        // Collect rejected IDs
        for id in rejected_ids {
            if !skipped.contains(&id) && !applied.iter().any(|r| r.patch_id == id) {
                skipped.push(id);
            }
        }

        Ok(BatchApplyResult {
            applied,
            skipped,
            pending_review,
            rolled_back: false,
            error: None,
        })
    }

    /// Validate the model integrity.
    pub fn validate(&self) -> ValidationReport {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check required fields
        if self.identity.name.trim().is_empty() {
            warnings.push("Identity name is empty".to_string());
        }

        // Check value ranges
        for value in &self.identity.values {
            if value.weight > 100 {
                errors.push(format!(
                    "Value '{}' weight {} exceeds maximum 100",
                    value.name, value.weight
                ));
            }
        }

        // Check goal priorities
        for goal in &self.goals.short_term {
            if goal.priority > 10 {
                errors.push(format!(
                    "Goal '{}' priority {} exceeds maximum 10",
                    goal.name, goal.priority
                ));
            }
        }

        // Check skill proficiency
        for skill in &self.capabilities.skills {
            if skill.proficiency > 10 {
                errors.push(format!(
                    "Skill '{}' proficiency {} exceeds maximum 10",
                    skill.name, skill.proficiency
                ));
            }
        }

        // Check energy level
        if self.state.health_status.energy_level > 10 {
            errors.push(format!(
                "Energy level {} exceeds maximum 10",
                self.state.health_status.energy_level
            ));
        }

        ValidationReport { errors, warnings }
    }
}

/// Report from model validation.
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Helper to get parent JSON Pointer.
fn get_parent_pointer(pointer: &str) -> Option<String> {
    let parts: Vec<&str> = pointer.split('/').collect();
    if parts.len() <= 2 {
        // Root or single segment, no parent
        None
    } else {
        let parent = parts[..parts.len() - 1].join("/");
        Some(parent)
    }
}

/// Helper to get last segment of JSON Pointer.
fn get_last_segment(pointer: &str) -> Option<String> {
    pointer
        .split('/')
        .next_back()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

pub struct LifeModelManager {
    data_dir: PathBuf,
}

impl LifeModelManager {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        fs::create_dir_all(&data_dir).ok();
        Self { data_dir }
    }
}

impl Default for LifeModelManager {
    fn default() -> Self {
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".openlife")
            .join("life-model")
            .join("current");
        fs::create_dir_all(&data_dir).ok();
        Self { data_dir }
    }
}

impl LifeModelManager {
    pub fn load(&self) -> Result<LifeModel> {
        let path = self.data_dir.join("life_model.yaml");
        if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("读取人生模型失败: {:?}", path))?;
            let model: LifeModel =
                serde_yaml::from_str(&content).with_context(|| "解析人生模型 YAML 失败")?;
            Ok(model)
        } else {
            let model = LifeModel::default_model();
            self.save(&model)?;
            Ok(model)
        }
    }

    pub fn save(&self, model: &LifeModel) -> Result<()> {
        let path = self.data_dir.join("life_model.yaml");
        let content = serde_yaml::to_string(model).with_context(|| "序列化人生模型失败")?;
        fs::write(&path, content).with_context(|| format!("写入人生模型失败: {:?}", path))?;
        Ok(())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let hash = digest(&SHA256, bytes);
    let bytes = hash.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

pub mod patch;
pub mod patch_store;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::evidence_store::{
        EvidenceDraft, EvidencePrivacyLevel, EvidenceSourceRef, EvidenceSourceType, EvidenceStore,
        EvidenceType,
    };
    use crate::agent::heuristic_store::{HeuristicDraft, HeuristicStore};
    use crate::agent::types::RiskLevel;

    #[test]
    fn default_model_has_metadata() {
        let m = LifeModel::default_model();
        assert_eq!(m.metadata.version, "0.1.0");
        assert!(!m.metadata.created_at.is_empty());
    }

    #[test]
    fn calculate_4d_completion_empty_is_not_zero_for_state() {
        let m = LifeModel::default_model();
        let c = m.calculate_4d_completion();
        // default model sets some state fields
        assert!(c.state > 0);
        assert_eq!(
            c.overall,
            (c.identity / 4) + (c.goals / 4) + (c.capabilities / 4) + (c.state / 4)
        );
    }

    #[test]
    fn default_model_is_effectively_empty() {
        let model = LifeModel::default_model();
        assert!(model.is_effectively_empty());
    }

    #[test]
    fn builder_populated_model_is_not_effectively_empty() {
        let mut model = LifeModel::default_model();
        model.identity.name = "fujing".into();
        model.goals.short_term.push(GoalItem {
            name: "把 OpenLife 跑通".into(),
            description: "".into(),
            priority: 5,
            status: "pending".into(),
            progress: 0.0,
            deadline: None,
            milestones: vec![],
            related_memories: vec![],
            updated_at: None,
        });
        assert!(!model.is_effectively_empty());
    }

    #[test]
    fn completion_identity_full() {
        let mut m = LifeModel::default_model();
        m.identity.name = "Alice".into();
        m.identity.birth_date = Some("1990-01-01".into());
        m.identity.values.push(ValueItem {
            name: "成长".into(),
            weight: 8,
            description: "".into(),
        });
        m.identity.personality_traits.push(PersonalityTrait {
            trait_name: "乐观".into(),
            score: 7,
        });
        m.identity.life_philosophy = "活在当下".into();
        m.identity.mission_statement = "创造价值".into();
        m.identity.role_definition.primary_role = "工程师".into();
        m.identity.voice_style.tone_descriptors.push("友好".into());
        let c = m.calculate_4d_completion();
        assert_eq!(c.identity, 100);
    }

    #[test]
    fn identity_goal_alignment_no_conflict_when_aligned() {
        let mut m = LifeModel::default_model();
        m.identity.values.push(ValueItem {
            name: "成长".into(),
            weight: 8,
            description: "".into(),
        });
        m.goals.short_term.push(GoalItem {
            name: "每天成长".into(),
            description: "追求成长".into(),
            priority: 8,
            deadline: None,
            milestones: vec![],
            status: "active".into(),
            progress: 0.0,
            related_memories: vec![],
            updated_at: None,
        });
        let issues = m.identity_goal_alignment_check();
        assert!(issues.is_empty());
    }

    #[test]
    fn identity_goal_alignment_detects_conflict() {
        let mut m = LifeModel::default_model();
        m.identity.values.push(ValueItem {
            name: "成长".into(),
            weight: 8,
            description: "".into(),
        });
        m.goals.short_term.push(GoalItem {
            name: "赚钱".into(),
            description: "积累财富".into(),
            priority: 8,
            deadline: None,
            milestones: vec![],
            status: "active".into(),
            progress: 0.0,
            related_memories: vec![],
            updated_at: None,
        });
        let issues = m.identity_goal_alignment_check();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("赚钱"));

        let report = m.identity_goal_alignment_report();
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].goal_name, "赚钱");
    }

    #[test]
    fn goal_capability_gap_detects_low_proficiency() {
        let mut m = LifeModel::default_model();
        m.capabilities.skills.push(Skill {
            name: "编程".into(),
            proficiency: 4,
            description: "".into(),
        });
        m.goals.short_term.push(GoalItem {
            name: "提升编程能力".into(),
            description: "练习编程".into(),
            priority: 5,
            deadline: None,
            milestones: vec![],
            status: "active".into(),
            progress: 0.0,
            related_memories: vec![],
            updated_at: None,
        });
        let gaps = m.goal_capability_gap_analysis();
        assert_eq!(gaps.len(), 1);
        assert!(gaps[0].contains("编程"));

        let report = m.goal_capability_gap_report();
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].skill_name, "编程");
    }

    #[test]
    fn manager_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = LifeModelManager::new(dir.path());
        let mut model = LifeModel::default_model();
        model.identity.name = "Test".into();
        mgr.save(&model).unwrap();
        let loaded = mgr.load().unwrap();
        assert_eq!(loaded.identity.name, "Test");
    }

    #[test]
    fn materialized_compatibility_yaml_keeps_life_model_fields_and_allowed_sections() {
        let mut model = LifeModel::default_model();
        model.identity.name = "Compat User".into();
        model.state.current_focus = "Shipping LMHS".into();
        model.state.emotional_state.current_mood = "focused".into();
        model.state.health_status.energy_level = 4;

        let evidence_store = EvidenceStore::new_in_memory().unwrap();
        let evidence = evidence_store
            .create_evidence(
                EvidenceDraft::new(
                    EvidenceType::RuntimeBehavior,
                    "proactive.reminder.pending_proposal",
                    0.65,
                    RiskLevel::Low,
                    EvidencePrivacyLevel::Internal,
                )
                .with_summary("raw-private-evidence-summary-should-stay-out")
                .with_source_ref(EvidenceSourceRef::from_payload(
                    EvidenceSourceType::Proposal,
                    "proposal-123",
                    Some("raw source detail should stay out"),
                    "raw source payload should stay out",
                )),
            )
            .unwrap();
        let heuristic_store = HeuristicStore::new_in_memory().unwrap();
        let heuristic = heuristic_store
            .create_heuristic(
                HeuristicDraft::new(
                    "planning",
                    "low_energy",
                    vec!["state.energy <= 3".into()],
                    "full private heuristic guidance should stay out",
                    90,
                    RiskLevel::Low,
                    EvidencePrivacyLevel::Internal,
                )
                .with_opposing_evidence_ref("opposing-raw-evidence-id"),
            )
            .unwrap();

        let yaml = model
            .materialize_yaml_compatibility_view(
                vec![LifeModelCompatibilitySummary::new(
                    "Prefer concise planning when energy is low.",
                    vec![heuristic.id.clone()],
                )],
                &[evidence],
                &[heuristic],
            )
            .unwrap();

        assert!(yaml.contains("identity:"));
        assert!(yaml.contains("hs_compatibility:"));
        assert!(yaml.contains("current_state_summary:"));
        assert!(yaml.contains("collaboration_summaries:"));
        assert!(yaml.contains("asset_refs:"));
        assert!(yaml.contains("proposal-123"));
        assert!(yaml.contains("content_digest:"));
        assert!(yaml.contains("source_digest:"));

        let loaded: LifeModel = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(loaded.identity.name, "Compat User");
        assert_eq!(loaded.state.current_focus, "Shipping LMHS");
    }

    #[test]
    fn materialized_compatibility_yaml_omits_raw_hs_internals() {
        let model = LifeModel::default_model();
        let evidence_store = EvidenceStore::new_in_memory().unwrap();
        let evidence = evidence_store
            .create_evidence(
                EvidenceDraft::new(
                    EvidenceType::Preference,
                    "identity.values",
                    0.8,
                    RiskLevel::Medium,
                    EvidencePrivacyLevel::Sensitive,
                )
                .with_summary("raw-sensitive-evidence-body")
                .with_source_ref(EvidenceSourceRef::from_payload(
                    EvidenceSourceType::ChatMessage,
                    "chat-1",
                    Some("raw-chat-detail"),
                    "raw-chat-source-text",
                )),
            )
            .unwrap();
        let heuristic_store = HeuristicStore::new_in_memory().unwrap();
        let heuristic = heuristic_store
            .create_heuristic(
                HeuristicDraft::new(
                    "identity",
                    "private_trigger",
                    vec!["raw condition should stay out".into()],
                    "raw heuristic guidance should stay out",
                    50,
                    RiskLevel::Medium,
                    EvidencePrivacyLevel::Sensitive,
                )
                .with_opposing_evidence_ref("opposing-evidence-raw"),
            )
            .unwrap();

        let yaml = model
            .materialize_yaml_compatibility_view(
                vec![LifeModelCompatibilitySummary::new(
                    "Concise collaboration summary is allowed.",
                    vec![heuristic.id.clone(), evidence.id.clone()],
                )],
                &[evidence],
                &[heuristic],
            )
            .unwrap();

        assert!(yaml.contains("Concise collaboration summary is allowed."));
        assert!(!yaml.contains("raw-sensitive-evidence-body"));
        assert!(!yaml.contains("raw-chat-source-text"));
        assert!(!yaml.contains("raw-chat-detail"));
        assert!(!yaml.contains("raw heuristic guidance should stay out"));
        assert!(!yaml.contains("raw condition should stay out"));
        assert!(!yaml.contains("opposing-evidence-raw"));
    }
}
