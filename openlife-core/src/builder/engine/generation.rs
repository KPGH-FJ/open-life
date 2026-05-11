use crate::builder::types::*;
use crate::life_model::{
    EmotionalState, GoalItem, HealthStatus, LifeModel, PersonalityTrait, Resource, Skill, ValueItem,
};
use crate::llm::ChatMessage;

impl<'a> super::BuilderEngine<'a> {
    pub(crate) fn extract_quick_build_signals(
        session: &BuilderSession,
        _model: &LifeModel,
    ) -> Vec<BuilderSignal> {
        use std::collections::HashMap;

        let mut signals = vec![];
        let draft = &session.draft_yaml;

        // Parse answers from draft (format: "# step N\nanswer")
        let mut answers: HashMap<usize, String> = HashMap::new();
        let mut current_step: Option<usize> = None;

        for line in draft.lines() {
            if let Some(rest) = line.strip_prefix("# step ") {
                if let Ok(step) = rest.parse::<usize>() {
                    current_step = Some(step);
                }
            } else if let Some(step) = current_step {
                answers.entry(step).or_default().push_str(line);
                answers.entry(step).or_default().push('\n');
            }
        }

        // Helper to create signal
        let create_signal = |id: &str,
                             step: usize,
                             dim: &str,
                             path: &str,
                             value: serde_json::Value,
                             conf: f32,
                             reason: &str,
                             risk: RiskLevel|
         -> BuilderSignal {
            BuilderSignal {
                id: id.to_string(),
                source_step: step,
                source_question_id: QUICK_BUILD_STEPS
                    .get(step.saturating_sub(1))
                    .unwrap_or(&"")
                    .to_string(),
                dimension: match dim {
                    "identity" => BuilderDimension::Identity,
                    "goals" => BuilderDimension::Goals,
                    "capabilities" => BuilderDimension::Capabilities,
                    "state" => BuilderDimension::State,
                    _ => BuilderDimension::Identity,
                },
                affected_path: path.to_string(),
                proposed_value: value,
                confidence: conf,
                reason: reason.to_string(),
                risk_level: risk,
                user_status: SignalUserStatus::Pending,
            }
        };

        // Step 1: Name (identity.name) - LOW RISK
        if let Some(ans) = answers.get(&1) {
            let name = ans.trim().to_string();
            if !name.is_empty() {
                signals.push(create_signal(
                    "sig_name",
                    1,
                    "identity",
                    "identity.name",
                    serde_json::Value::String(name.clone()),
                    0.95,
                    "用户直接提供的称呼",
                    RiskLevel::Low,
                ));
            }
        }

        // Step 2: Current Focus (state.current_focus) - LOW RISK
        if let Some(ans) = answers.get(&2) {
            let focus = ans.trim().to_string();
            if !focus.is_empty() {
                signals.push(create_signal(
                    "sig_focus",
                    2,
                    "state",
                    "state.current_focus",
                    serde_json::Value::String(focus.clone()),
                    0.90,
                    "用户选择的当前关注主题",
                    RiskLevel::Low,
                ));
                // Also add to focus_areas
                signals.push(create_signal(
                    "sig_focus_areas",
                    2,
                    "state",
                    "state.focus_areas",
                    serde_json::Value::Array(vec![serde_json::Value::String(focus)]),
                    0.85,
                    "当前关注作为焦点领域",
                    RiskLevel::Low,
                ));
            }
        }

        // Step 3: Short-term Goals (goals.short_term) - MEDIUM RISK
        if let Some(ans) = answers.get(&3) {
            let goals_text = ans.trim();
            if !goals_text.is_empty() {
                // Parse goals (split by newlines, support bullet formats)
                let goals: Vec<String> = goals_text
                    .lines()
                    .map(normalize_list_line)
                    .filter(|l| !l.is_empty())
                    .collect();

                let goal_items: Vec<serde_json::Value> = goals
                    .iter()
                    .map(|g| {
                        serde_json::json!({
                            "name": g,
                            "priority": 5,
                            "status": "pending",
                            "milestones": [],
                            "description": "",
                            "progress": 0.0
                        })
                    })
                    .collect();

                if !goal_items.is_empty() {
                    signals.push(create_signal(
                        "sig_short_term",
                        3,
                        "goals",
                        "goals.short_term",
                        serde_json::Value::Array(goal_items),
                        0.80,
                        "用户描述的近期目标",
                        RiskLevel::Medium,
                    ));
                }
            }
        }

        // Step 4: Long-term Direction (goals.long_term, identity.mission) - HIGH RISK
        if let Some(ans) = answers.get(&4) {
            let direction = ans.trim().to_string();
            if !direction.is_empty() {
                // Only suggest, don't auto-apply
                signals.push(create_signal(
                    "sig_long_term", 4, "goals", "goals.long_term",
                    serde_json::Value::Array(vec![serde_json::json!({
                        "name": format!("长期方向: {}", direction.chars().take(30).collect::<String>()),
                        "priority": 5,
                        "status": "pending",
                        "milestones": [],
                        "description": direction,
                        "progress": 0.0
                    })]),
                    0.60,
                    "用户描述的长期方向(需要确认)",
                    RiskLevel::High
                ));
            }
        }

        // Step 5: Capabilities (capabilities.skills, resources) - MEDIUM RISK
        if let Some(ans) = answers.get(&5) {
            let caps_text = ans.trim();
            if !caps_text.is_empty() {
                let skills: Vec<String> = caps_text
                    .lines()
                    .map(normalize_list_line)
                    .filter(|l| !l.is_empty())
                    .take(5) // Limit to top 5
                    .collect();

                let skill_items: Vec<serde_json::Value> = skills
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "name": s.chars().take(20).collect::<String>(),
                            "proficiency": 5,
                            "description": s
                        })
                    })
                    .collect();

                if !skill_items.is_empty() {
                    signals.push(create_signal(
                        "sig_skills",
                        5,
                        "capabilities",
                        "capabilities.skills",
                        serde_json::Value::Array(skill_items),
                        0.75,
                        "用户自报的能力",
                        RiskLevel::Medium,
                    ));
                }
            }
        }

        // Step 6: Current Blockers (state.emotional_state, alerts) - MEDIUM RISK
        if let Some(ans) = answers.get(&6) {
            let blockers = ans.trim().to_string();
            if !blockers.is_empty() {
                // Extract emotional state
                let emotional_keywords = [
                    "焦虑", "压力", "疲惫", "迷茫", "沮丧", "困惑", "紧张", "担忧",
                ];
                let found_emotion = emotional_keywords
                    .iter()
                    .find(|&&k| blockers.contains(k))
                    .map(|&k| k.to_string());

                if let Some(emotion) = found_emotion {
                    signals.push(create_signal(
                        "sig_emotion",
                        6,
                        "state",
                        "state.emotional_state.current_mood",
                        serde_json::Value::String(emotion),
                        0.70,
                        &format!("从用户描述中检测到的情绪关键词: {}", blockers),
                        RiskLevel::Medium,
                    ));
                }

                // Add as alert
                signals.push(create_signal(
                    "sig_alert", 6, "state", "state.alerts",
                    serde_json::Value::Array(vec![serde_json::json!({
                        "message": format!("当前卡点: {}", blockers.chars().take(50).collect::<String>()),
                        "severity": "medium",
                        "dimension_name": "general"
                    })]),
                    0.65,
                    "用户主动报告的阻碍",
                    RiskLevel::Medium
                ));
            }
        }

        // Step 7: Companion Style (identity.voice_style, preferences.communication_style) - LOW RISK
        if let Some(ans) = answers.get(&7) {
            let style = ans.trim().to_string();
            if !style.is_empty() {
                signals.push(create_signal(
                    "sig_comm_style",
                    7,
                    "identity",
                    "preferences.communication_style",
                    serde_json::Value::String(style.clone()),
                    0.90,
                    "用户选择的陪伴风格",
                    RiskLevel::Low,
                ));

                // Map to voice style descriptors
                let descriptors = if style.contains("温和") {
                    vec!["温暖", "支持"]
                } else if style.contains("直接") {
                    vec!["直接", "高效"]
                } else if style.contains("苏格拉底") {
                    vec!["好奇", "探究"]
                } else if style.contains("教练") {
                    vec!["激励", "结构化"]
                } else if style.contains("朋友") {
                    vec!["自然", "平等"]
                } else if style.contains("理性") {
                    vec!["分析", "客观"]
                } else {
                    vec!["适应"]
                };

                signals.push(create_signal(
                    "sig_voice",
                    7,
                    "identity",
                    "identity.voice_style.tone_descriptors",
                    serde_json::Value::Array(
                        descriptors
                            .into_iter()
                            .map(|s| serde_json::Value::String(s.to_string()))
                            .collect(),
                    ),
                    0.85,
                    &format!("根据陪伴风格「{}」映射的语调特征", style),
                    RiskLevel::Low,
                ));
            }
        }

        signals
    }

    /// Apply accepted signals to LifeModel.
    /// Returns (applied_field_descriptions, skipped_fields).
    pub fn apply_signals_to_model(
        model: &mut LifeModel,
        signals: &[BuilderSignal],
    ) -> (Vec<String>, Vec<SkippedField>) {
        let mut applied = vec![];
        let mut skipped = vec![];

        for signal in signals {
            if signal.user_status != SignalUserStatus::Accepted
                && signal.user_status != SignalUserStatus::Edited
            {
                continue;
            }

            // Simple path-based application
            let path_parts: Vec<&str> = signal.affected_path.split('.').collect();

            match path_parts.as_slice() {
                // Identity - simple fields
                ["identity", "name"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.identity.name = val.to_string();
                        applied.push(format!("identity.name = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                ["identity", "life_philosophy"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.identity.life_philosophy = val.to_string();
                        applied.push(format!("identity.life_philosophy = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                ["identity", "mission_statement"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.identity.mission_statement = val.to_string();
                        applied.push(format!("identity.mission_statement = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                // Identity - arrays of objects (merge strategy)
                ["identity", "values"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::ValueItem> = arr
                            .iter()
                            .filter_map(|v| {
                                Some(crate::life_model::ValueItem {
                                    name: v.get("name")?.as_str()?.to_string(),
                                    weight: v.get("weight")?.as_u64()? as u8,
                                    description: v
                                        .get("description")?
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect();
                        if items.is_empty() {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "value array parsed to empty",
                                "array of {name, weight, description}",
                            );
                        } else {
                            Self::merge_value_items(&mut model.identity.values, items);
                            applied.push("identity.values (merged)".to_string());
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of {name, weight, description}",
                        );
                    }
                }
                ["identity", "personality_traits"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::PersonalityTrait> = arr
                            .iter()
                            .filter_map(|v| {
                                Some(crate::life_model::PersonalityTrait {
                                    trait_name: v.get("trait_name")?.as_str()?.to_string(),
                                    score: v.get("score")?.as_u64()? as u8,
                                })
                            })
                            .collect();
                        if items.is_empty() {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "personality trait array parsed to empty",
                                "array of {trait_name, score}",
                            );
                        } else {
                            for item in items {
                                if let Some(existing) = model
                                    .identity
                                    .personality_traits
                                    .iter_mut()
                                    .find(|v| v.trait_name == item.trait_name)
                                {
                                    *existing = item;
                                } else {
                                    model.identity.personality_traits.push(item);
                                }
                            }
                            applied.push("identity.personality_traits (merged)".to_string());
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of {trait_name, score}",
                        );
                    }
                }
                // Identity - voice_style
                ["identity", "voice_style", "tone_descriptors"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if items.is_empty() {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "tone descriptor array parsed to empty",
                                "array of strings",
                            );
                        } else {
                            Self::merge_strings(
                                &mut model.identity.voice_style.tone_descriptors,
                                items,
                            );
                            applied
                                .push("identity.voice_style.tone_descriptors (merged)".to_string());
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of strings",
                        );
                    }
                }
                ["identity", "voice_style", "formality"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.identity.voice_style.formality = match val {
                            "casual" => crate::life_model::FormalityLevel::Casual,
                            "formal" => crate::life_model::FormalityLevel::Formal,
                            _ => crate::life_model::FormalityLevel::Neutral,
                        };
                        applied.push(format!("identity.voice_style.formality = {}", val));
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected string value",
                            "string: casual | neutral | formal",
                        );
                    }
                }
                ["identity", "voice_style", "vocabulary_preference"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.identity.voice_style.vocabulary_preference = val.to_string();
                        applied.push(format!(
                            "identity.voice_style.vocabulary_preference = {}",
                            val
                        ));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                // Identity - role_definition
                ["identity", "role_definition", "primary_role"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.identity.role_definition.primary_role = val.to_string();
                        applied.push(format!("identity.role_definition.primary_role = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                ["identity", "role_definition", "secondary_roles"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !items.is_empty() {
                            Self::merge_strings(
                                &mut model.identity.role_definition.secondary_roles,
                                items,
                            );
                            applied.push(
                                "identity.role_definition.secondary_roles (merged)".to_string(),
                            );
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "secondary role array parsed to empty",
                                "array of strings",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of strings",
                        );
                    }
                }
                ["identity", "role_definition", "responsibilities"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !items.is_empty() {
                            Self::merge_strings(
                                &mut model.identity.role_definition.responsibilities,
                                items,
                            );
                            applied.push(
                                "identity.role_definition.responsibilities (merged)".to_string(),
                            );
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "responsibility array parsed to empty",
                                "array of strings",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of strings",
                        );
                    }
                }
                ["identity", "role_definition", "boundaries"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !items.is_empty() {
                            Self::merge_strings(
                                &mut model.identity.role_definition.boundaries,
                                items,
                            );
                            applied
                                .push("identity.role_definition.boundaries (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "boundary array parsed to empty",
                                "array of strings",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of strings",
                        );
                    }
                }
                // Goals - all terms
                ["goals", "short_term"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::GoalItem> =
                            arr.iter().filter_map(Self::parse_goal_item).collect();
                        if !items.is_empty() {
                            Self::merge_goal_items(&mut model.goals.short_term, items);
                            applied.push("goals.short_term (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "goal array parsed to empty",
                                "array of GoalItem objects",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of GoalItem objects",
                        );
                    }
                }
                ["goals", "medium_term"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::GoalItem> =
                            arr.iter().filter_map(Self::parse_goal_item).collect();
                        if !items.is_empty() {
                            Self::merge_goal_items(&mut model.goals.medium_term, items);
                            applied.push("goals.medium_term (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "goal array parsed to empty",
                                "array of GoalItem objects",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of GoalItem objects",
                        );
                    }
                }
                ["goals", "long_term"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::GoalItem> =
                            arr.iter().filter_map(Self::parse_goal_item).collect();
                        if !items.is_empty() {
                            Self::merge_goal_items(&mut model.goals.long_term, items);
                            applied.push("goals.long_term (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "goal array parsed to empty",
                                "array of GoalItem objects",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of GoalItem objects",
                        );
                    }
                }
                ["goals", "life_goals"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::GoalItem> =
                            arr.iter().filter_map(Self::parse_goal_item).collect();
                        if !items.is_empty() {
                            Self::merge_goal_items(&mut model.goals.life_goals, items);
                            applied.push("goals.life_goals (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "goal array parsed to empty",
                                "array of GoalItem objects",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of GoalItem objects",
                        );
                    }
                }
                // Capabilities
                ["capabilities", "skills"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::Skill> = arr
                            .iter()
                            .filter_map(|v| {
                                Some(crate::life_model::Skill {
                                    name: v.get("name")?.as_str()?.to_string(),
                                    proficiency: v.get("proficiency")?.as_u64()? as u8,
                                    description: v
                                        .get("description")?
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect();
                        if !items.is_empty() {
                            Self::merge_skills(&mut model.capabilities.skills, items);
                            applied.push("capabilities.skills (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "skill array parsed to empty",
                                "array of {name, proficiency, description}",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of {name, proficiency, description}",
                        );
                    }
                }
                ["capabilities", "resources"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::Resource> = arr
                            .iter()
                            .filter_map(|v| {
                                Some(crate::life_model::Resource {
                                    name: v.get("name")?.as_str()?.to_string(),
                                    resource_type: v
                                        .get("type")?
                                        .as_str()
                                        .unwrap_or("other")
                                        .to_string(),
                                    description: v
                                        .get("description")?
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                    availability: v
                                        .get("availability")?
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect();
                        if !items.is_empty() {
                            Self::merge_resources(&mut model.capabilities.resources, items);
                            applied.push("capabilities.resources (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "resource array parsed to empty",
                                "array of {name, type, description, availability}",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of {name, type, description, availability}",
                        );
                    }
                }
                ["capabilities", "networks"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !items.is_empty() {
                            Self::merge_strings(&mut model.capabilities.networks, items);
                            applied.push("capabilities.networks (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "network array parsed to empty",
                                "array of strings",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of strings",
                        );
                    }
                }
                ["capabilities", "tools"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::ToolCapability> = arr
                            .iter()
                            .filter_map(|v| {
                                Some(crate::life_model::ToolCapability {
                                    name: v.get("name")?.as_str()?.to_string(),
                                    proficiency: v.get("proficiency")?.as_u64()? as u8,
                                    description: v
                                        .get("description")?
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect();
                        if !items.is_empty() {
                            Self::merge_tools(&mut model.capabilities.tools, items);
                            applied.push("capabilities.tools (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "tool array parsed to empty",
                                "array of {name, proficiency, description}",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of {name, proficiency, description}",
                        );
                    }
                }
                ["capabilities", "knowledge_domains"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::KnowledgeDomain> = arr
                            .iter()
                            .filter_map(|v| {
                                Some(crate::life_model::KnowledgeDomain {
                                    domain: v.get("domain")?.as_str()?.to_string(),
                                    level: v.get("level")?.as_u64()? as u8,
                                    description: v
                                        .get("description")?
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect();
                        if !items.is_empty() {
                            Self::merge_knowledge_domains(
                                &mut model.capabilities.knowledge_domains,
                                items,
                            );
                            applied.push("capabilities.knowledge_domains (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "knowledge domain array parsed to empty",
                                "array of {domain, level, description}",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of {domain, level, description}",
                        );
                    }
                }
                // State - simple fields
                ["state", "current_focus"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.state.current_focus = val.to_string();
                        applied.push(format!("state.current_focus = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                // State - health_status
                ["state", "health_status", "physical"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.state.health_status.physical = val.to_string();
                        applied.push(format!("state.health_status.physical = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                ["state", "health_status", "mental"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.state.health_status.mental = val.to_string();
                        applied.push(format!("state.health_status.mental = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                ["state", "health_status", "energy_level"] => {
                    if let Some(val) = signal.proposed_value.as_u64() {
                        model.state.health_status.energy_level = val as u8;
                        applied.push(format!("state.health_status.energy_level = {}", val));
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected integer value",
                            "integer 0-10",
                        );
                    }
                }
                // State - emotional_state
                ["state", "emotional_state", "current_mood"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.state.emotional_state.current_mood = val.to_string();
                        applied.push(format!("state.emotional_state.current_mood = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                ["state", "emotional_state", "stress_level"] => {
                    if let Some(val) = signal.proposed_value.as_u64() {
                        model.state.emotional_state.stress_level = val as u8;
                        applied.push(format!("state.emotional_state.stress_level = {}", val));
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected integer value",
                            "integer 0-10",
                        );
                    }
                }
                ["state", "emotional_state", "fulfillment_score"] => {
                    if let Some(val) = signal.proposed_value.as_u64() {
                        model.state.emotional_state.fulfillment_score = val as u8;
                        applied.push(format!("state.emotional_state.fulfillment_score = {}", val));
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected integer value",
                            "integer 0-10",
                        );
                    }
                }
                // State - arrays
                ["state", "focus_areas"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !items.is_empty() {
                            Self::merge_strings(&mut model.state.focus_areas, items);
                            applied.push("state.focus_areas (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "focus area array parsed to empty",
                                "array of strings",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of strings",
                        );
                    }
                }
                ["state", "open_questions"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !items.is_empty() {
                            Self::merge_strings(&mut model.state.open_questions, items);
                            applied.push("state.open_questions (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "open question array parsed to empty",
                                "array of strings",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of strings",
                        );
                    }
                }
                // Preferences
                ["preferences", "communication_style"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.preferences.communication_style = val.to_string();
                        applied.push(format!("preferences.communication_style = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                ["preferences", "learning_style"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.preferences.learning_style = val.to_string();
                        applied.push(format!("preferences.learning_style = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                ["preferences", "decision_making_style"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.preferences.decision_making_style = val.to_string();
                        applied.push(format!("preferences.decision_making_style = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                ["preferences", "peak_energy_time"] => {
                    if let Some(val) = signal.proposed_value.as_str() {
                        model.preferences.peak_energy_time = val.to_string();
                        applied.push(format!("preferences.peak_energy_time = {}", val));
                    } else {
                        Self::skip_field(&mut skipped, signal, "expected string value", "string");
                    }
                }
                ["goals", "daily"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::DailyGoal> = arr
                            .iter()
                            .filter_map(|v| {
                                Some(crate::life_model::DailyGoal {
                                    name: v.get("name")?.as_str()?.to_string(),
                                    done: v.get("done").and_then(|d| d.as_bool()).unwrap_or(false),
                                    time_block: v.get("time_block").and_then(|t| {
                                        Some(crate::life_model::TimeBlock {
                                            start: t.get("start")?.as_str()?.to_string(),
                                            end: t.get("end")?.as_str()?.to_string(),
                                        })
                                    }),
                                })
                            })
                            .collect();
                        if !items.is_empty() {
                            for item in items {
                                if let Some(existing) =
                                    model.goals.daily.iter_mut().find(|v| v.name == item.name)
                                {
                                    *existing = item;
                                } else {
                                    model.goals.daily.push(item);
                                }
                            }
                            applied.push("goals.daily (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "daily goal array parsed to empty",
                                "array of {name, done, time_block}",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of {name, done, time_block}",
                        );
                    }
                }
                ["state", "alerts"] => {
                    if let Some(arr) = signal.proposed_value.as_array() {
                        let items: Vec<crate::life_model::StateAlert> = arr
                            .iter()
                            .filter_map(|v| {
                                Some(crate::life_model::StateAlert {
                                    dimension_name: v
                                        .get("dimension_name")
                                        .or_else(|| v.get("dimension"))?
                                        .as_str()?
                                        .to_string(),
                                    level: match v.get("severity")?.as_str()? {
                                        "critical" => crate::life_model::AlertLevel::Critical,
                                        "warning" | "medium" => {
                                            crate::life_model::AlertLevel::Warning
                                        }
                                        _ => crate::life_model::AlertLevel::Info,
                                    },
                                    message: v.get("message")?.as_str()?.to_string(),
                                    triggered_at: v
                                        .get("triggered_at")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect();
                        if !items.is_empty() {
                            for item in items {
                                if let Some(existing) = model.state.alerts.iter_mut().find(|v| {
                                    v.dimension_name == item.dimension_name
                                        && v.message == item.message
                                }) {
                                    *existing = item;
                                } else {
                                    model.state.alerts.push(item);
                                }
                            }
                            applied.push("state.alerts (merged)".to_string());
                        } else {
                            Self::skip_field(
                                &mut skipped,
                                signal,
                                "state alert array parsed to empty",
                                "array of {dimension_name, severity, message}",
                            );
                        }
                    } else {
                        Self::skip_field(
                            &mut skipped,
                            signal,
                            "expected array value",
                            "array of {dimension_name, severity, message}",
                        );
                    }
                }
                unsupported => {
                    skipped.push(SkippedField {
                        path: signal.affected_path.clone(),
                        reason: format!("unsupported path '{}'", unsupported.join(".")),
                        expected: None,
                    });
                }
            }
        }

        (applied, skipped)
    }

    pub(crate) fn merge_strings(target: &mut Vec<String>, items: Vec<String>) {
        for item in items {
            if !item.trim().is_empty() && !target.iter().any(|existing| existing == &item) {
                target.push(item);
            }
        }
    }

    pub(crate) fn parse_goal_item(v: &serde_json::Value) -> Option<crate::life_model::GoalItem> {
        let name = v.get("name")?.as_str()?.trim().to_string();
        if name.is_empty() {
            return None;
        }
        let milestones = v
            .get("milestones")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let milestone_name = m.get("name")?.as_str()?.trim().to_string();
                        if milestone_name.is_empty() {
                            return None;
                        }
                        Some(crate::life_model::Milestone {
                            name: milestone_name,
                            achieved: m.get("achieved").and_then(|a| a.as_bool()).unwrap_or_else(
                                || m.get("status").and_then(|s| s.as_str()) == Some("completed"),
                            ),
                            date: m
                                .get("date")
                                .or_else(|| m.get("target_date"))
                                .and_then(|d| d.as_str())
                                .map(|s| s.to_string()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let related_memories = v
            .get("related_memories")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        Some(crate::life_model::GoalItem {
            name,
            description: v
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string(),
            priority: v.get("priority").and_then(|p| p.as_u64()).unwrap_or(5) as u8,
            status: v
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            progress: v.get("progress").and_then(|p| p.as_f64()).unwrap_or(0.0) as f32,
            deadline: v
                .get("deadline")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string()),
            milestones,
            related_memories,
            updated_at: None,
        })
    }

    pub(crate) fn merge_value_items(
        target: &mut Vec<crate::life_model::ValueItem>,
        items: Vec<crate::life_model::ValueItem>,
    ) {
        for item in items {
            if let Some(existing) = target.iter_mut().find(|v| v.name == item.name) {
                if item.weight > 0 {
                    existing.weight = item.weight;
                }
                if !item.description.trim().is_empty() {
                    existing.description = item.description;
                }
            } else {
                target.push(item);
            }
        }
    }

    pub(crate) fn merge_goal_items(
        target: &mut Vec<crate::life_model::GoalItem>,
        items: Vec<crate::life_model::GoalItem>,
    ) {
        for item in items {
            if let Some(existing) = target.iter_mut().find(|v| v.name == item.name) {
                if !item.description.trim().is_empty() {
                    existing.description = item.description;
                }
                if item.priority > 0 {
                    existing.priority = item.priority;
                }
                if !item.status.trim().is_empty() {
                    existing.status = item.status;
                }
                if item.progress > 0.0 {
                    existing.progress = item.progress;
                }
                if item.deadline.is_some() {
                    existing.deadline = item.deadline;
                }
                Self::merge_milestones(&mut existing.milestones, item.milestones);
                Self::merge_strings(&mut existing.related_memories, item.related_memories);
            } else {
                let mut new_item = item;
                if new_item.status.trim().is_empty() {
                    new_item.status = "pending".to_string();
                }
                target.push(new_item);
            }
        }
    }

    pub(crate) fn merge_milestones(
        target: &mut Vec<crate::life_model::Milestone>,
        items: Vec<crate::life_model::Milestone>,
    ) {
        for item in items {
            if let Some(existing) = target.iter_mut().find(|v| v.name == item.name) {
                existing.achieved = item.achieved || existing.achieved;
                if item.date.is_some() {
                    existing.date = item.date;
                }
            } else {
                target.push(item);
            }
        }
    }

    pub(crate) fn merge_skills(
        target: &mut Vec<crate::life_model::Skill>,
        items: Vec<crate::life_model::Skill>,
    ) {
        for item in items {
            if let Some(existing) = target.iter_mut().find(|v| v.name == item.name) {
                if item.proficiency > 0 {
                    existing.proficiency = item.proficiency;
                }
                if !item.description.trim().is_empty() {
                    existing.description = item.description;
                }
            } else {
                target.push(item);
            }
        }
    }

    pub(crate) fn merge_resources(
        target: &mut Vec<crate::life_model::Resource>,
        items: Vec<crate::life_model::Resource>,
    ) {
        for item in items {
            if let Some(existing) = target.iter_mut().find(|v| v.name == item.name) {
                if !item.resource_type.trim().is_empty() {
                    existing.resource_type = item.resource_type;
                }
                if !item.description.trim().is_empty() {
                    existing.description = item.description;
                }
                if !item.availability.trim().is_empty() {
                    existing.availability = item.availability;
                }
            } else {
                target.push(item);
            }
        }
    }

    pub(crate) fn merge_tools(
        target: &mut Vec<crate::life_model::ToolCapability>,
        items: Vec<crate::life_model::ToolCapability>,
    ) {
        for item in items {
            if let Some(existing) = target.iter_mut().find(|v| v.name == item.name) {
                if item.proficiency > 0 {
                    existing.proficiency = item.proficiency;
                }
                if !item.description.trim().is_empty() {
                    existing.description = item.description;
                }
            } else {
                target.push(item);
            }
        }
    }

    pub(crate) fn merge_knowledge_domains(
        target: &mut Vec<crate::life_model::KnowledgeDomain>,
        items: Vec<crate::life_model::KnowledgeDomain>,
    ) {
        for item in items {
            if let Some(existing) = target.iter_mut().find(|v| v.domain == item.domain) {
                if item.level > 0 {
                    existing.level = item.level;
                }
                if !item.description.trim().is_empty() {
                    existing.description = item.description;
                }
            } else {
                target.push(item);
            }
        }
    }

    pub(crate) fn skip_field(
        skipped: &mut Vec<SkippedField>,
        signal: &BuilderSignal,
        reason: &str,
        expected: &str,
    ) {
        skipped.push(SkippedField {
            path: signal.affected_path.clone(),
            reason: reason.to_string(),
            expected: Some(expected.to_string()),
        });
    }

    pub(crate) fn detect_gaps(model: &LifeModel) -> Vec<String> {
        let mut gaps = Vec::new();
        let id = &model.identity;
        if id.values.is_empty()
            && id.personality_traits.is_empty()
            && id.mission_statement.is_empty()
        {
            gaps.push("身份认同".to_string());
        }
        let g = &model.goals;
        if g.short_term.is_empty()
            && g.long_term.is_empty()
            && g.medium_term.is_empty()
            && g.life_goals.is_empty()
        {
            gaps.push("目标设定".to_string());
        }
        let c = &model.capabilities;
        if c.skills.is_empty() && c.tools.is_empty() && c.knowledge_domains.is_empty() {
            gaps.push("能力盘点".to_string());
        }
        let s = &model.state;
        if s.emotional_state.current_mood.is_empty() && s.health_status.physical.is_empty() {
            gaps.push("当前状态".to_string());
        }
        if model.relationships.inner_circle.is_empty()
            && model.relationships.mentors.is_empty()
            && model.relationships.collaborators.is_empty()
        {
            gaps.push("关键关系".to_string());
        }
        gaps
    }

    pub(crate) async fn draft_to_life_model(&self, draft: &str, base: &LifeModel) -> LifeModel {
        let mut model = base.clone();
        if draft.trim().is_empty() {
            return model;
        }

        let system_prompt = r#"你是一个结构化信息提取助手。请根据用户在 OpenLife 构建模式中的回答，提取人生模型信息，并严格只输出一段合法的 JSON（不要 Markdown 代码块，不要解释）。

JSON 结构如下：
{
  "identity": {
    "name": "",
    "life_philosophy": "",
    "mission_statement": "",
    "role_definition": {
      "primary_role": "",
      "secondary_roles": [],
      "responsibilities": [],
      "boundaries": []
    },
    "values": [{"name":"", "weight":1, "description":""}],
    "personality_traits": [{"trait_name":"", "score":5}]
  },
  "goals": {
    "short_term": [{"name":"", "priority":5, "status":"pending", "milestones":[], "description":""}],
    "medium_term": [{"name":"", "priority":5, "status":"pending", "milestones":[], "description":""}],
    "long_term": [{"name":"", "priority":5, "status":"pending", "milestones":[], "description":""}],
    "life_goals": [{"name":"", "priority":5, "status":"pending", "milestones":[], "description":""}]
  },
  "capabilities": {
    "skills": [{"name":"", "proficiency":5, "description":""}],
    "resources": [{"name":"", "type":"other", "description":""}],
    "networks": [""],
    "tools": [{"name":"", "proficiency":5, "description":""}],
    "knowledge_domains": [{"domain":"", "level":5, "description":""}]
  },
  "state": {
    "current_focus": "",
    "emotional_state": {"current_mood":"", "stress_level":5, "fulfillment_score":5},
    "health_status": {"physical":"", "mental":"", "energy_level":5}
  },
  "relationships": {
    "inner_circle": [{"name":"", "relationship_type":"", "importance":5, "notes":""}],
    "mentors": [{"name":"", "relationship_type":"mentor", "importance":5, "notes":""}],
    "collaborators": [{"name":"", "relationship_type":"collaborator", "importance":5, "notes":""}]
  },
  "preferences": {
    "peak_energy_time": "",
    "communication_style": "",
    "learning_style": "",
    "decision_making_style": ""
  }
}

规则：
1. 如果某字段无法从回答中推断，使用空字符串或空数组。
2. weight、priority、proficiency、score、stress_level、fulfillment_score、energy_level 是 1-10 的整数，请根据上下文合理推断。
3. 只输出 JSON，不要任何其他文字。"#.to_string();

        let messages = vec![ChatMessage {
            role: "user".into(),
            content: draft.to_string(),
        }];

        match self
            .scheduler
            .generate_raw(messages, Some(&system_prompt))
            .await
        {
            Ok(json_text) => {
                let cleaned = json_text
                    .trim()
                    .trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(cleaned) {
                    if let Some(values_arr) = parsed
                        .get("identity")
                        .and_then(|i| i.get("values"))
                        .and_then(|v| v.as_array())
                    {
                        let values: Vec<ValueItem> = values_arr
                            .iter()
                            .filter_map(|v| {
                                Some(ValueItem {
                                    name: v.get("name")?.as_str()?.to_string(),
                                    weight: v.get("weight")?.as_u64()? as u8,
                                    description: v.get("description")?.as_str()?.to_string(),
                                })
                            })
                            .collect();
                        if !values.is_empty() {
                            model.identity.values = values;
                        }
                    }
                    if let Some(traits_arr) = parsed
                        .get("identity")
                        .and_then(|i| i.get("personality_traits"))
                        .and_then(|v| v.as_array())
                    {
                        let traits: Vec<PersonalityTrait> = traits_arr
                            .iter()
                            .filter_map(|v| {
                                Some(PersonalityTrait {
                                    trait_name: v.get("trait_name")?.as_str()?.to_string(),
                                    score: v.get("score")?.as_u64()? as u8,
                                })
                            })
                            .collect();
                        if !traits.is_empty() {
                            model.identity.personality_traits = traits;
                        }
                    }
                    if let Some(lp) = parsed
                        .get("identity")
                        .and_then(|i| i.get("life_philosophy"))
                        .and_then(|v| v.as_str())
                    {
                        if !lp.is_empty() {
                            model.identity.life_philosophy = lp.to_string();
                        }
                    }
                    if let Some(ms) = parsed
                        .get("identity")
                        .and_then(|i| i.get("mission_statement"))
                        .and_then(|v| v.as_str())
                    {
                        if !ms.is_empty() {
                            model.identity.mission_statement = ms.to_string();
                        }
                    }
                    if let Some(role) = parsed
                        .get("identity")
                        .and_then(|i| i.get("role_definition"))
                    {
                        if let Some(primary) = role.get("primary_role").and_then(|v| v.as_str()) {
                            if !primary.is_empty() {
                                model.identity.role_definition.primary_role = primary.to_string();
                            }
                        }
                        if let Some(secondary) =
                            role.get("secondary_roles").and_then(|v| v.as_array())
                        {
                            let values: Vec<String> = secondary
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect();
                            if !values.is_empty() {
                                model.identity.role_definition.secondary_roles = values;
                            }
                        }
                        if let Some(responsibilities) =
                            role.get("responsibilities").and_then(|v| v.as_array())
                        {
                            let values: Vec<String> = responsibilities
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect();
                            if !values.is_empty() {
                                model.identity.role_definition.responsibilities = values;
                            }
                        }
                        if let Some(boundaries) = role.get("boundaries").and_then(|v| v.as_array())
                        {
                            let values: Vec<String> = boundaries
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect();
                            if !values.is_empty() {
                                model.identity.role_definition.boundaries = values;
                            }
                        }
                    }
                    for (goal_key, goal_field) in [
                        ("short_term", &mut model.goals.short_term),
                        ("medium_term", &mut model.goals.medium_term),
                        ("long_term", &mut model.goals.long_term),
                        ("life_goals", &mut model.goals.life_goals),
                    ] {
                        if let Some(goals_arr) = parsed
                            .get("goals")
                            .and_then(|g| g.get(goal_key))
                            .and_then(|v| v.as_array())
                        {
                            let goals: Vec<GoalItem> = goals_arr
                                .iter()
                                .filter_map(|v| {
                                    Some(GoalItem {
                                        name: v.get("name")?.as_str()?.to_string(),
                                        priority: v.get("priority")?.as_u64()? as u8,
                                        status: v.get("status")?.as_str()?.to_string(),
                                        milestones: vec![],
                                        description: v.get("description")?.as_str()?.to_string(),
                                        progress: 0.0,
                                        related_memories: vec![],
                                        deadline: None,
                                        updated_at: None,
                                    })
                                })
                                .collect();
                            if !goals.is_empty() {
                                *goal_field = goals;
                            }
                        }
                    }
                    if let Some(skills_arr) = parsed
                        .get("capabilities")
                        .and_then(|c| c.get("skills"))
                        .and_then(|v| v.as_array())
                    {
                        let skills: Vec<Skill> = skills_arr
                            .iter()
                            .filter_map(|v| {
                                Some(Skill {
                                    name: v.get("name")?.as_str()?.to_string(),
                                    proficiency: v.get("proficiency")?.as_u64()? as u8,
                                    description: v.get("description")?.as_str()?.to_string(),
                                })
                            })
                            .collect();
                        if !skills.is_empty() {
                            model.capabilities.skills = skills;
                        }
                    }
                    if let Some(resources_arr) = parsed
                        .get("capabilities")
                        .and_then(|c| c.get("resources"))
                        .and_then(|v| v.as_array())
                    {
                        let resources: Vec<Resource> = resources_arr
                            .iter()
                            .filter_map(|v| {
                                Some(Resource {
                                    name: v.get("name")?.as_str()?.to_string(),
                                    resource_type: v
                                        .get("type")?
                                        .as_str()
                                        .unwrap_or("other")
                                        .to_string(),
                                    description: v.get("description")?.as_str()?.to_string(),
                                    availability: "available".into(),
                                })
                            })
                            .collect();
                        if !resources.is_empty() {
                            model.capabilities.resources = resources;
                        }
                    }
                    if let Some(networks_arr) = parsed
                        .get("capabilities")
                        .and_then(|c| c.get("networks"))
                        .and_then(|v| v.as_array())
                    {
                        let networks: Vec<String> = networks_arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !networks.is_empty() {
                            model.capabilities.networks = networks;
                        }
                    }
                    if let Some(tools_arr) = parsed
                        .get("capabilities")
                        .and_then(|c| c.get("tools"))
                        .and_then(|v| v.as_array())
                    {
                        let tools: Vec<crate::life_model::ToolCapability> = tools_arr
                            .iter()
                            .filter_map(|v| {
                                Some(crate::life_model::ToolCapability {
                                    name: v.get("name")?.as_str()?.to_string(),
                                    proficiency: v
                                        .get("proficiency")
                                        .and_then(|n| n.as_u64())
                                        .unwrap_or(5)
                                        as u8,
                                    description: v
                                        .get("description")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect();
                        if !tools.is_empty() {
                            model.capabilities.tools = tools;
                        }
                    }
                    if let Some(domains_arr) = parsed
                        .get("capabilities")
                        .and_then(|c| c.get("knowledge_domains"))
                        .and_then(|v| v.as_array())
                    {
                        let domains: Vec<crate::life_model::KnowledgeDomain> = domains_arr
                            .iter()
                            .filter_map(|v| {
                                Some(crate::life_model::KnowledgeDomain {
                                    domain: v.get("domain")?.as_str()?.to_string(),
                                    level: v.get("level").and_then(|n| n.as_u64()).unwrap_or(5)
                                        as u8,
                                    description: v
                                        .get("description")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect();
                        if !domains.is_empty() {
                            model.capabilities.knowledge_domains = domains;
                        }
                    }
                    if let Some(current_focus) = parsed
                        .get("state")
                        .and_then(|s| s.get("current_focus"))
                        .and_then(|v| v.as_str())
                    {
                        if !current_focus.is_empty() {
                            model.state.current_focus = current_focus.to_string();
                        }
                    }
                    if let Some(emo) = parsed.get("state").and_then(|s| s.get("emotional_state")) {
                        let current_mood = emo
                            .get("current_mood")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let stress_level = emo
                            .get("stress_level")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(5) as u8;
                        let fulfillment_score = emo
                            .get("fulfillment_score")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(5) as u8;
                        if !current_mood.is_empty() {
                            model.state.emotional_state = EmotionalState {
                                current_mood,
                                stress_level,
                                fulfillment_score,
                            };
                        }
                    }
                    if let Some(hs) = parsed.get("state").and_then(|s| s.get("health_status")) {
                        let physical = hs
                            .get("physical")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let mental = hs
                            .get("mental")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let energy_level =
                            hs.get("energy_level").and_then(|v| v.as_u64()).unwrap_or(5) as u8;
                        if !physical.is_empty() || !mental.is_empty() {
                            model.state.health_status = HealthStatus {
                                physical,
                                mental,
                                energy_level,
                            };
                        }
                    }
                    if let Some(relationships) = parsed.get("relationships") {
                        for (key, target) in [
                            ("inner_circle", &mut model.relationships.inner_circle),
                            ("mentors", &mut model.relationships.mentors),
                            ("collaborators", &mut model.relationships.collaborators),
                        ] {
                            if let Some(items) = relationships.get(key).and_then(|v| v.as_array()) {
                                let values: Vec<crate::life_model::Relationship> = items
                                    .iter()
                                    .filter_map(|v| {
                                        Some(crate::life_model::Relationship {
                                            name: v.get("name")?.as_str()?.to_string(),
                                            relationship_type: v
                                                .get("relationship_type")
                                                .and_then(|s| s.as_str())
                                                .unwrap_or("")
                                                .to_string(),
                                            importance: v
                                                .get("importance")
                                                .and_then(|n| n.as_u64())
                                                .unwrap_or(5)
                                                as u8,
                                            notes: v
                                                .get("notes")
                                                .and_then(|s| s.as_str())
                                                .unwrap_or("")
                                                .to_string(),
                                        })
                                    })
                                    .collect();
                                if !values.is_empty() {
                                    *target = values;
                                }
                            }
                        }
                    }
                    if let Some(prefs) = parsed.get("preferences") {
                        if let Some(v) = prefs.get("peak_energy_time").and_then(|v| v.as_str()) {
                            if !v.is_empty() {
                                model.preferences.peak_energy_time = v.to_string();
                            }
                        }
                        if let Some(v) = prefs.get("communication_style").and_then(|v| v.as_str()) {
                            if !v.is_empty() {
                                model.preferences.communication_style = v.to_string();
                            }
                        }
                        if let Some(v) = prefs.get("learning_style").and_then(|v| v.as_str()) {
                            if !v.is_empty() {
                                model.preferences.learning_style = v.to_string();
                            }
                        }
                        if let Some(v) = prefs.get("decision_making_style").and_then(|v| v.as_str())
                        {
                            if !v.is_empty() {
                                model.preferences.decision_making_style = v.to_string();
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Builder LLM parse failed: {} - builder.rs:1870", e);
            }
        }
        model
    }
}
