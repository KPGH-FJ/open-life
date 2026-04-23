use crate::feedback::FeedbackStore;
use crate::life_model::{GoalItem, LifeModel};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single signal source contributing to an evolution change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignalSource {
    pub source: String, // "feedback" | "behavior" | "inference"
    pub score: f32,
    pub weight: f32,
}

/// A single proposed change from micro-evolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvolutionChange {
    pub dimension: String, // "identity.values" | "goals" | "capabilities.skills"
    pub target_name: String,
    pub old_value: f32,
    pub new_value: f32,
    pub reason: String,
    /// Confidence score (0.0 ~ 1.0). Higher means more sources agree on direction.
    pub confidence: f32,
    /// Per-source breakdown for transparency.
    pub sources: Vec<SignalSource>,
}

/// Result of running micro-evolution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MicroEvolutionResult {
    pub changes: Vec<EvolutionChange>,
    pub applied: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignalContributor {
    pub name: String,
    pub score: f32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EvolutionSignalSummary {
    pub feedback_terms: usize,
    pub behavior_events: usize,
    pub inference_items: usize,
    pub top_feedback: Vec<SignalContributor>,
    pub top_behavior: Vec<SignalContributor>,
    pub top_inference: Vec<SignalContributor>,
}

/// Micro-evolution engine with 7-day sliding window and fusion of three signals.
pub struct MicroEvolutionEngine<'a> {
    store: &'a FeedbackStore,
}

impl<'a> MicroEvolutionEngine<'a> {
    pub fn new(store: &'a FeedbackStore) -> Self {
        Self { store }
    }

    /// Run fusion evolution on the given model.
    /// Returns proposed changes. If consistency check fails, returns empty changes with applied=false.
    pub fn run(&self, model: &LifeModel) -> Result<MicroEvolutionResult> {
        let (result, _) = self.run_with_signals(model)?;
        Ok(result)
    }

    pub fn run_with_signals(
        &self,
        model: &LifeModel,
    ) -> Result<(MicroEvolutionResult, EvolutionSignals)> {
        let signals = self.store.fetch_evolution_signals(7)?;

        let mut changes = Vec::new();
        let mut working_model = model.clone();

        // === Identity values ===
        for value in &mut working_model.identity.values {
            let delta = Self::fuse_delta(&signals, &value.name, "identity.values");
            if delta.abs() >= 0.005 {
                let new_weight = (value.weight as f32 + delta).clamp(1.0, 10.0);
                let rounded = (new_weight * 100.0).round() / 100.0;
                let max_delta = 0.03f32;
                let clamped = if rounded > value.weight as f32 + max_delta {
                    value.weight as f32 + max_delta
                } else if rounded < value.weight as f32 - max_delta {
                    value.weight as f32 - max_delta
                } else {
                    rounded
                };
                let final_val = (clamped * 100.0).round() / 100.0;
                if (final_val - value.weight as f32).abs() >= 0.005 {
                    let (confidence, sources) = Self::compute_confidence_and_sources(
                        &signals,
                        &value.name,
                        "identity.values",
                    );
                    changes.push(EvolutionChange {
                        dimension: "identity.values".into(),
                        target_name: value.name.clone(),
                        old_value: value.weight as f32,
                        new_value: final_val,
                        reason: format!(
                            "融合信号调整: feedback={:.2}, behavior={:.2}, inference={:.2}",
                            signals.feedback_score(&value.name),
                            signals.behavior_score(&value.name),
                            signals.inference_score_for_dimension(&value.name, "identity.values")
                        ),
                        confidence,
                        sources,
                    });
                    value.weight = final_val as u8;
                }
            }
        }

        // === Goal priorities ===
        let mut goal_lists: [&mut Vec<GoalItem>; 4] = [
            &mut working_model.goals.short_term,
            &mut working_model.goals.medium_term,
            &mut working_model.goals.long_term,
            &mut working_model.goals.life_goals,
        ];
        for goals in goal_lists.iter_mut() {
            for goal in goals.iter_mut() {
                let delta = Self::fuse_delta(&signals, &goal.name, "goals");
                if delta.abs() >= 0.005 {
                    let old = goal.priority as f32;
                    let new_val = (old + delta).clamp(1.0, 10.0);
                    let rounded = (new_val * 100.0).round() / 100.0;
                    let clamped = rounded.min(old + 0.03).max(old - 0.03);
                    let final_val = (clamped * 100.0).round() / 100.0;
                    if (final_val - old).abs() >= 0.005 {
                        let (confidence, sources) =
                            Self::compute_confidence_and_sources(&signals, &goal.name, "goals");
                        changes.push(EvolutionChange {
                            dimension: "goals".into(),
                            target_name: goal.name.clone(),
                            old_value: old,
                            new_value: final_val,
                            reason: format!("目标优先级微调: 综合信号 delta={:.2}", delta),
                            confidence,
                            sources,
                        });
                        goal.priority = final_val as u8;
                    }
                }
            }
        }

        // === Capability skills ===
        for skill in &mut working_model.capabilities.skills {
            let delta = Self::fuse_delta(&signals, &skill.name, "capabilities.skills");
            if delta.abs() >= 0.005 {
                let old = skill.proficiency as f32;
                let new_val = (old + delta).clamp(1.0, 10.0);
                let rounded = (new_val * 100.0).round() / 100.0;
                let clamped = rounded.min(old + 0.03).max(old - 0.03);
                let final_val = (clamped * 100.0).round() / 100.0;
                if (final_val - old).abs() >= 0.005 {
                    let (confidence, sources) = Self::compute_confidence_and_sources(
                        &signals,
                        &skill.name,
                        "capabilities.skills",
                    );
                    changes.push(EvolutionChange {
                        dimension: "capabilities.skills".into(),
                        target_name: skill.name.clone(),
                        old_value: old,
                        new_value: final_val,
                        reason: format!("能力熟练度微调: 综合信号 delta={:.2}", delta),
                        confidence,
                        sources,
                    });
                    skill.proficiency = final_val as u8;
                }
            }
        }

        // === Consistency check ===
        let before_issues = model.identity_goal_alignment_check().len();
        let after_issues = working_model.identity_goal_alignment_check().len();
        if after_issues > before_issues {
            return Ok((
                MicroEvolutionResult {
                    changes: Vec::new(),
                    applied: false,
                    message: format!(
                        "一致性校验失败：调整前冲突 {} 个，调整后冲突 {} 个，已回退全部变更。",
                        before_issues, after_issues
                    ),
                },
                signals,
            ));
        }

        if changes.is_empty() {
            Ok((
                MicroEvolutionResult {
                    changes,
                    applied: false,
                    message: "近7天暂无足够信号来微调模型权重".into(),
                },
                signals,
            ))
        } else {
            let n = changes.len();
            Ok((
                MicroEvolutionResult {
                    changes,
                    applied: true,
                    message: format!("已应用 {} 项微调调整", n),
                },
                signals,
            ))
        }
    }

    /// Apply a list of approved changes to the model.
    pub fn apply_changes(model: &mut LifeModel, changes: &[EvolutionChange]) -> Result<String> {
        for change in changes {
            match change.dimension.as_str() {
                "identity.values" => {
                    if let Some(v) = model
                        .identity
                        .values
                        .iter_mut()
                        .find(|v| v.name == change.target_name)
                    {
                        v.weight = change.new_value as u8;
                    }
                }
                "goals" => {
                    for goals in [
                        &mut model.goals.short_term,
                        &mut model.goals.medium_term,
                        &mut model.goals.long_term,
                        &mut model.goals.life_goals,
                    ] {
                        if let Some(g) = goals.iter_mut().find(|g| g.name == change.target_name) {
                            g.priority = change.new_value as u8;
                        }
                    }
                }
                "capabilities.skills" => {
                    if let Some(s) = model
                        .capabilities
                        .skills
                        .iter_mut()
                        .find(|s| s.name == change.target_name)
                    {
                        s.proficiency = change.new_value as u8;
                    }
                }
                _ => {}
            }
        }
        Ok(format!("已手动应用 {} 项变更", changes.len()))
    }

    fn fuse_delta(signals: &EvolutionSignals, target_name: &str, dimension: &str) -> f32 {
        let fb = signals.feedback_score(target_name);
        let bh = signals.behavior_score(target_name);
        let inf = signals.inference_score_for_dimension(target_name, dimension);
        0.5 * fb + 0.3 * bh + 0.2 * inf
    }

    fn compute_confidence_and_sources(
        signals: &EvolutionSignals,
        target_name: &str,
        dimension: &str,
    ) -> (f32, Vec<SignalSource>) {
        let fb = signals.feedback_score(target_name);
        let bh = signals.behavior_score(target_name);
        let inf = signals.inference_score_for_dimension(target_name, dimension);

        let sources = vec![
            SignalSource {
                source: "feedback".into(),
                score: fb,
                weight: 0.5,
            },
            SignalSource {
                source: "behavior".into(),
                score: bh,
                weight: 0.3,
            },
            SignalSource {
                source: "inference".into(),
                score: inf,
                weight: 0.2,
            },
        ];

        let non_zero: Vec<f32> = [fb, bh, inf]
            .into_iter()
            .filter(|s| s.abs() > 0.001)
            .collect();
        let agreement = if non_zero.is_empty() {
            0.0
        } else {
            let pos_count = non_zero.iter().filter(|s| **s > 0.0).count();
            let neg_count = non_zero.iter().filter(|s| **s < 0.0).count();
            let max_agree = pos_count.max(neg_count) as f32;
            max_agree / non_zero.len() as f32
        };

        let avg_strength = if non_zero.is_empty() {
            0.0
        } else {
            non_zero.iter().map(|s| s.abs()).sum::<f32>() / non_zero.len() as f32
        };

        let confidence = (agreement * 0.6 + avg_strength.min(1.0) * 0.4).clamp(0.0, 1.0);
        (confidence, sources)
    }
}

/// Signal container used by FusionEngine.
#[derive(Debug, Clone, Default)]
pub struct EvolutionSignals {
    pub feedback: HashMap<String, f32>,
    pub behavior: HashMap<String, f32>,
    pub inference: HashMap<String, f32>, // key = "dimension:name"
}

impl EvolutionSignals {
    pub fn feedback_score(&self, name: &str) -> f32 {
        *self.feedback.get(&name.to_lowercase()).unwrap_or(&0.0)
    }
    pub fn behavior_score(&self, name: &str) -> f32 {
        *self.behavior.get(&name.to_lowercase()).unwrap_or(&0.0)
    }
    pub fn inference_score_for_dimension(&self, name: &str, dimension: &str) -> f32 {
        let key = format!("{}:{}", dimension, name.to_lowercase());
        *self.inference.get(&key).unwrap_or(&0.0)
    }

    pub fn summary(&self) -> EvolutionSignalSummary {
        EvolutionSignalSummary {
            feedback_terms: self.feedback.len(),
            behavior_events: self.behavior.len(),
            inference_items: self.inference.len(),
            top_feedback: top_contributors(&self.feedback, "feedback"),
            top_behavior: top_contributors(&self.behavior, "behavior"),
            top_inference: top_contributors(&self.inference, "inference"),
        }
    }
}

fn top_contributors(map: &HashMap<String, f32>, source: &str) -> Vec<SignalContributor> {
    let mut items: Vec<_> = map
        .iter()
        .map(|(name, score)| SignalContributor {
            name: name.clone(),
            score: *score,
            source: source.to_string(),
        })
        .collect();
    items.sort_by(|a, b| {
        b.score
            .abs()
            .partial_cmp(&a.score.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    items.truncate(5);
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life_model::{GoalItem, LifeModel, Skill, ValueItem};

    #[test]
    fn evolution_signals_scoring() {
        let mut signals = EvolutionSignals::default();
        signals.feedback.insert("health".into(), 0.2);
        signals.behavior.insert("health".into(), -0.1);
        signals
            .inference
            .insert("identity.values:health".into(), 0.5);

        assert_eq!(signals.feedback_score("Health"), 0.2);
        assert_eq!(signals.behavior_score("health"), -0.1);
        assert_eq!(
            signals.inference_score_for_dimension("health", "identity.values"),
            0.5
        );
        assert_eq!(
            signals.inference_score_for_dimension("health", "goals"),
            0.0
        );
    }

    #[test]
    fn fuse_delta_computes_weighted_sum() {
        let mut signals = EvolutionSignals::default();
        signals.feedback.insert("test".into(), 0.2);
        signals.behavior.insert("test".into(), 0.3);
        signals.inference.insert("goals:test".into(), 0.5);
        let delta = MicroEvolutionEngine::fuse_delta(&signals, "test", "goals");
        let expected = 0.5 * 0.2 + 0.3 * 0.3 + 0.2 * 0.5;
        assert!((delta - expected).abs() < 0.001);
    }

    #[test]
    fn apply_changes_updates_identity_values() {
        let mut model = LifeModel::default_model();
        model.identity.values.push(ValueItem {
            name: "成长".into(),
            weight: 5,
            description: "".into(),
        });
        let changes = vec![EvolutionChange {
            dimension: "identity.values".into(),
            target_name: "成长".into(),
            old_value: 5.0,
            new_value: 7.0,
            reason: "".into(),
            confidence: 0.0,
            sources: vec![],
        }];
        MicroEvolutionEngine::apply_changes(&mut model, &changes).unwrap();
        assert_eq!(model.identity.values[0].weight, 7);
    }

    #[test]
    fn apply_changes_updates_goals_and_skills() {
        let mut model = LifeModel::default_model();
        model.goals.short_term.push(GoalItem {
            name: "目标A".into(),
            description: "".into(),
            priority: 3,
            status: "active".into(),
            progress: 0.0,
            deadline: None,
            milestones: vec![],
            related_memories: vec![],
        });
        model.capabilities.skills.push(Skill {
            name: "技能A".into(),
            proficiency: 4,
            description: "".into(),
        });
        let changes = vec![
            EvolutionChange {
                dimension: "goals".into(),
                target_name: "目标A".into(),
                old_value: 3.0,
                new_value: 6.0,
                reason: "".into(),
                confidence: 0.0,
                sources: vec![],
            },
            EvolutionChange {
                dimension: "capabilities.skills".into(),
                target_name: "技能A".into(),
                old_value: 4.0,
                new_value: 8.0,
                reason: "".into(),
                confidence: 0.0,
                sources: vec![],
            },
        ];
        MicroEvolutionEngine::apply_changes(&mut model, &changes).unwrap();
        assert_eq!(model.goals.short_term[0].priority, 6);
        assert_eq!(model.capabilities.skills[0].proficiency, 8);
    }

    #[test]
    fn signal_summary_reports_top_contributors() {
        let mut signals = EvolutionSignals::default();
        signals.feedback.insert("成长".into(), 0.03);
        signals.feedback.insert("自由".into(), -0.02);
        signals.behavior.insert("value_focus:成长".into(), 0.02);
        signals
            .inference
            .insert("identity.values:成长".into(), 0.04);
        let summary = signals.summary();
        assert_eq!(summary.feedback_terms, 2);
        assert_eq!(summary.behavior_events, 1);
        assert_eq!(summary.inference_items, 1);
        assert_eq!(summary.top_feedback[0].name, "成长");
        assert_eq!(summary.top_inference[0].name, "identity.values:成长");
    }
}
