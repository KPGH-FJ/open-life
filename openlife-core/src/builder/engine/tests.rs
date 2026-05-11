#![cfg(test)]
use super::*;
use crate::builder::types::*;
use crate::life_model::{GoalItem, LifeModel, Skill, ValueItem};

#[test]
fn detect_gaps_finds_missing_dimensions() {
    let mut model = LifeModel::default_model();
    model.identity.values.clear();
    model.goals.short_term.clear();
    model.capabilities.skills.clear();
    let gaps = BuilderEngine::detect_gaps(&model);
    assert!(!gaps.is_empty());
    assert!(gaps
        .iter()
        .any(|g| g.contains("身份认同") || g.contains("目标设定") || g.contains("能力盘点")));
}

#[test]
fn generate_pairwise_pairs_builds_unique_pairs_with_cap() {
    let names = vec![
        "成长".to_string(),
        "自由".to_string(),
        "创造".to_string(),
        "连接".to_string(),
    ];
    let pairs = BuilderEngine::generate_pairwise_pairs(&names);
    assert_eq!(pairs.len(), 6);
    assert!(pairs.contains(&(String::from("成长"), String::from("自由"))));
    assert!(pairs.contains(&(String::from("成长"), String::from("创造"))));
    assert!(pairs.contains(&(String::from("成长"), String::from("连接"))));
    assert!(pairs.contains(&(String::from("自由"), String::from("创造"))));
    assert!(pairs.contains(&(String::from("自由"), String::from("连接"))));
    assert!(pairs.contains(&(String::from("创造"), String::from("连接"))));
}

#[test]
fn generate_pairwise_pairs_truncates_when_values_are_many() {
    let names = vec![
        "成长".to_string(),
        "自由".to_string(),
        "创造".to_string(),
        "连接".to_string(),
        "影响".to_string(),
    ];
    let pairs = BuilderEngine::generate_pairwise_pairs(&names);
    assert_eq!(pairs.len(), 6);
}

#[test]
fn scripted_prompt_uses_session_context() {
    let mut session = BuilderSession::new("s5", BuilderMode::Socratic);
    session.current_session = 2;
    session.step_index = 3;
    session.extracted_values = vec![
        ValueItem {
            name: "成长".into(),
            weight: 6,
            description: String::new(),
        },
        ValueItem {
            name: "创造".into(),
            weight: 5,
            description: String::new(),
        },
    ];
    let prompt =
        BuilderEngine::build_socratic_scripted_prompt(&session, &LifeModel::default_model());
    assert!(prompt.contains("成长"));
    assert!(prompt.contains("创造"));
    assert!(prompt.contains("责任") || prompt.contains("边界"));
}

#[test]
fn pairwise_explanation_ranks_and_connects_peak() {
    let mut session = BuilderSession::new("s6", BuilderMode::Socratic);
    session.pairwise_results = vec![
        ("自由".into(), "成长".into(), "成长".into()),
        ("自由".into(), "创造".into(), "自由".into()),
        ("成长".into(), "创造".into(), "成长".into()),
    ];
    session.peak_experience = Some(PeakExperience {
        raw_description: "在一次公开演讲中感受到成就感".into(),
        extracted_values: vec!["成长".into(), "自由".into(), "创造".into()],
        extracted_role_hints: vec!["演讲者".into()],
        extracted_capability_hints: vec!["表达".into(), "领导力".into()],
        extracted_preference_hints: vec!["早晨".into()],
        emotional_signal: "兴奋".into(),
    });
    let text = BuilderEngine::generate_pairwise_explanation(&session);
    assert!(text.contains("成长"), "应提到排名");
    assert!(text.contains("自由"), "应提到排名");
    assert!(text.contains("兴奋"), "应提到情绪");
    assert!(text.contains("表达"), "应提到能力暗示");
}

#[test]
fn patch_peak_experience_populates_model() {
    let mut session = BuilderSession::new("s7", BuilderMode::Socratic);
    session.peak_experience = Some(PeakExperience {
        raw_description: "带队完成项目很有成就感".into(),
        extracted_values: vec!["成就".into()],
        extracted_role_hints: vec!["团队领导".into(), "组织者".into()],
        extracted_capability_hints: vec!["项目管理".into()],
        extracted_preference_hints: vec!["直接沟通".into()],
        emotional_signal: "满足".into(),
    });
    let mut model = LifeModel::default_model();
    model.state.emotional_state.current_mood.clear(); // 清空默认值，确保 patch 生效
    BuilderEngine::patch_peak_experience(&session, &mut model);
    assert_eq!(model.identity.role_definition.primary_role, "团队领导");
    assert!(model
        .identity
        .role_definition
        .secondary_roles
        .contains(&"组织者".into()));
    assert!(model
        .capabilities
        .skills
        .iter()
        .any(|s| s.name == "项目管理"));
    assert_eq!(model.preferences.communication_style, "直接沟通");
    assert_eq!(model.state.emotional_state.current_mood, "满足");
}

#[test]
fn apply_signals_accepted_simple_field() {
    let mut model = LifeModel::default_model();
    let signals = vec![BuilderSignal {
        id: "sig1".into(),
        source_step: 0,
        source_question_id: "q1".into(),
        dimension: BuilderDimension::Identity,
        affected_path: "identity.name".into(),
        proposed_value: serde_json::Value::String("新名字".into()),
        confidence: 0.9,
        reason: "测试".into(),
        risk_level: RiskLevel::Low,
        user_status: SignalUserStatus::Accepted,
    }];
    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert_eq!(model.identity.name, "新名字");
    assert!(applied.iter().any(|s| s.contains("identity.name")));
    assert!(skipped.is_empty());
}

#[test]
fn apply_signals_edited_simple_field() {
    let mut model = LifeModel::default_model();
    // Edited status with a new proposed value should apply the edited value
    let signals = vec![BuilderSignal {
        id: "sig2".into(),
        source_step: 0,
        source_question_id: "q2".into(),
        dimension: BuilderDimension::State,
        affected_path: "state.current_focus".into(),
        proposed_value: serde_json::Value::String("编辑后的专注点".into()),
        confidence: 0.85,
        reason: "用户编辑".into(),
        risk_level: RiskLevel::Medium,
        user_status: SignalUserStatus::Edited,
    }];
    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert_eq!(model.state.current_focus, "编辑后的专注点");
    assert!(applied.iter().any(|s| s.contains("state.current_focus")));
    assert!(skipped.is_empty());
}

#[test]
fn apply_signals_rejected_skipped() {
    let mut model = LifeModel::default_model();
    model.identity.name = "原始名字".into();
    let signals = vec![BuilderSignal {
        id: "sig3".into(),
        source_step: 0,
        source_question_id: "q3".into(),
        dimension: BuilderDimension::Identity,
        affected_path: "identity.name".into(),
        proposed_value: serde_json::Value::String("被拒绝的名字".into()),
        confidence: 0.5,
        reason: "低置信度".into(),
        risk_level: RiskLevel::High,
        user_status: SignalUserStatus::Rejected,
    }];
    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert_eq!(model.identity.name, "原始名字"); // 应保持原值
    assert!(applied.is_empty()); // 没有任何字段被应用
    assert!(skipped.is_empty()); // 不记录 skipped，因为是被拒绝而非不支持
}

#[test]
fn apply_signals_goals_array_accepted() {
    let mut model = LifeModel::default_model();
    let signals = vec![BuilderSignal {
        id: "sig4".into(),
        source_step: 0,
        source_question_id: "q4".into(),
        dimension: BuilderDimension::Goals,
        affected_path: "goals.short_term".into(),
        proposed_value: serde_json::json!([
            {"name": "完成项目A", "priority": 8, "status": "pending", "progress": 0.0}
        ]),
        confidence: 0.9,
        reason: "短期目标".into(),
        risk_level: RiskLevel::Medium,
        user_status: SignalUserStatus::Accepted,
    }];
    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert_eq!(model.goals.short_term.len(), 1);
    assert_eq!(model.goals.short_term[0].name, "完成项目A");
    assert!(applied.iter().any(|s| s.contains("goals.short_term")));
    assert!(skipped.is_empty());
}

#[test]
fn apply_signals_goals_array_rejected() {
    let mut model = LifeModel::default_model();
    model.goals.short_term = vec![GoalItem {
        name: "现有目标".into(),
        description: "".into(),
        priority: 5,
        status: "pending".into(),
        progress: 0.0,
        deadline: None,
        milestones: vec![],
        related_memories: vec![],
        updated_at: None,
    }];
    let signals = vec![BuilderSignal {
        id: "sig5".into(),
        source_step: 0,
        source_question_id: "q5".into(),
        dimension: BuilderDimension::Goals,
        affected_path: "goals.short_term".into(),
        proposed_value: serde_json::json!([
            {"name": "被拒绝的目标", "priority": 5, "status": "pending", "progress": 0.0}
        ]),
        confidence: 0.5,
        reason: "不适合".into(),
        risk_level: RiskLevel::High,
        user_status: SignalUserStatus::Rejected,
    }];
    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert_eq!(model.goals.short_term.len(), 1);
    assert_eq!(model.goals.short_term[0].name, "现有目标"); // 应保持原值
    assert!(applied.is_empty());
    assert!(skipped.is_empty());
}

#[test]
fn apply_signals_goals_long_term_edited() {
    let mut model = LifeModel::default_model();
    // 模拟用户编辑后的 long_term 目标
    let signals = vec![BuilderSignal {
        id: "sig6".into(),
        source_step: 0,
        source_question_id: "q6".into(),
        dimension: BuilderDimension::Goals,
        affected_path: "goals.long_term".into(),
        proposed_value: serde_json::json!([
            {"name": "成为领域专家（编辑后）", "priority": 9, "status": "active", "progress": 0.1}
        ]),
        confidence: 0.9,
        reason: "用户编辑的长期目标".into(),
        risk_level: RiskLevel::Medium,
        user_status: SignalUserStatus::Edited,
    }];
    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert_eq!(model.goals.long_term.len(), 1);
    assert_eq!(model.goals.long_term[0].name, "成为领域专家（编辑后）");
    assert!(applied.iter().any(|s| s.contains("goals.long_term")));
    assert!(skipped.is_empty());
}

#[test]
fn apply_signals_capabilities_skills_accepted() {
    let mut model = LifeModel::default_model();
    let signals = vec![BuilderSignal {
        id: "sig7".into(),
        source_step: 0,
        source_question_id: "q7".into(),
        dimension: BuilderDimension::Capabilities,
        affected_path: "capabilities.skills".into(),
        proposed_value: serde_json::json!([
            {"name": "Rust编程", "proficiency": 8, "description": "熟练开发"}
        ]),
        confidence: 0.9,
        reason: "技能信号".into(),
        risk_level: RiskLevel::Low,
        user_status: SignalUserStatus::Accepted,
    }];
    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert_eq!(model.capabilities.skills.len(), 1);
    assert_eq!(model.capabilities.skills[0].name, "Rust编程");
    assert!(applied.iter().any(|s| s.contains("capabilities.skills")));
    assert!(skipped.is_empty());
}

#[test]
fn apply_signals_identity_values_array() {
    let mut model = LifeModel::default_model();
    let signals = vec![BuilderSignal {
        id: "sig8".into(),
        source_step: 0,
        source_question_id: "q8".into(),
        dimension: BuilderDimension::Identity,
        affected_path: "identity.values".into(),
        proposed_value: serde_json::json!([
            {"name": "诚信", "weight": 9, "description": "核心价值观"},
            {"name": "创新", "weight": 8, "description": "追求突破"}
        ]),
        confidence: 0.9,
        reason: "价值观".into(),
        risk_level: RiskLevel::Low,
        user_status: SignalUserStatus::Accepted,
    }];
    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert_eq!(model.identity.values.len(), 2);
    assert_eq!(model.identity.values[0].name, "诚信");
    assert_eq!(model.identity.values[1].name, "创新");
    assert!(applied.iter().any(|s| s.contains("identity.values")));
    assert!(skipped.is_empty());
}

#[test]
fn apply_signals_mixed_statuses() {
    let mut model = LifeModel::default_model();
    let signals = vec![
        BuilderSignal {
            id: "s1".into(),
            source_step: 0,
            source_question_id: "q1".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::Value::String("接受的名字".into()),
            confidence: 0.9,
            reason: "接受".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "s2".into(),
            source_step: 0,
            source_question_id: "q2".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.life_philosophy".into(),
            proposed_value: serde_json::Value::String("被拒绝的哲学".into()),
            confidence: 0.5,
            reason: "拒绝".into(),
            risk_level: RiskLevel::High,
            user_status: SignalUserStatus::Rejected,
        },
        BuilderSignal {
            id: "s3".into(),
            source_step: 0,
            source_question_id: "q3".into(),
            dimension: BuilderDimension::State,
            affected_path: "state.current_focus".into(),
            proposed_value: serde_json::Value::String("编辑后的专注点".into()),
            confidence: 0.8,
            reason: "编辑".into(),
            risk_level: RiskLevel::Medium,
            user_status: SignalUserStatus::Edited,
        },
    ];
    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);

    // Accepted: 应该被应用
    assert_eq!(model.identity.name, "接受的名字");
    // Rejected: 应该被跳过
    assert!(
        model.identity.life_philosophy.is_empty()
            || model.identity.life_philosophy != "被拒绝的哲学"
    );
    // Edited: 应该被应用
    assert_eq!(model.state.current_focus, "编辑后的专注点");

    // 应该只有 accepted 和 edited 的被应用到
    assert_eq!(applied.len(), 2);
    assert!(skipped.is_empty());
}

#[test]
fn apply_signals_unsupported_path_skipped() {
    let mut model = LifeModel::default_model();
    let signals = vec![
        BuilderSignal {
            id: "s_ok".into(),
            source_step: 0,
            source_question_id: "q_ok".into(),
            dimension: BuilderDimension::State,
            affected_path: "state.current_focus".into(),
            proposed_value: serde_json::Value::String("正常工作".into()),
            confidence: 0.9,
            reason: "支持的路径".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "s_skip".into(),
            source_step: 0,
            source_question_id: "q_skip".into(),
            dimension: BuilderDimension::Capabilities,
            affected_path: "capabilities.unknown_field".into(),
            proposed_value: serde_json::Value::String("不支持的值".into()),
            confidence: 0.5,
            reason: "不支持的路径".into(),
            risk_level: RiskLevel::Medium,
            user_status: SignalUserStatus::Accepted,
        },
    ];
    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert_eq!(applied.len(), 1);
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].path, "capabilities.unknown_field");
    assert!(skipped[0].reason.contains("unsupported"));
}

#[test]
fn apply_signals_goals_daily_and_state_alerts() {
    let mut model = LifeModel::default_model();
    let signals = vec![
        BuilderSignal {
            id: "s_daily".into(),
            source_step: 0,
            source_question_id: "q1".into(),
            dimension: BuilderDimension::Goals,
            affected_path: "goals.daily".into(),
            proposed_value: serde_json::json!([
                {"name": "晨跑", "done": false, "time_block": {"start": "06:30", "end": "07:00"}}
            ]),
            confidence: 0.9,
            reason: "每日目标".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "s_alert".into(),
            source_step: 0,
            source_question_id: "q2".into(),
            dimension: BuilderDimension::State,
            affected_path: "state.alerts".into(),
            proposed_value: serde_json::json!([
                {"dimension_name": "general", "severity": "warning", "message": "注意节奏", "triggered_at": ""}
            ]),
            confidence: 0.8,
            reason: "状态提醒".into(),
            risk_level: RiskLevel::Medium,
            user_status: SignalUserStatus::Accepted,
        },
    ];
    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert_eq!(model.goals.daily.len(), 1);
    assert_eq!(model.goals.daily[0].name, "晨跑");
    assert_eq!(model.state.alerts.len(), 1);
    assert_eq!(model.state.alerts[0].message, "注意节奏");
    assert_eq!(applied.len(), 2);
    assert!(skipped.is_empty());
}

#[test]
fn apply_signals_merges_arrays_without_overwriting_existing_model() {
    let mut model = LifeModel::default_model();
    model.identity.values = vec![ValueItem {
        name: "健康".into(),
        weight: 6,
        description: "原有价值观".into(),
    }];
    model.goals.short_term = vec![GoalItem {
        name: "现有目标".into(),
        description: "保留".into(),
        priority: 5,
        status: "active".into(),
        progress: 0.2,
        deadline: None,
        milestones: vec![],
        related_memories: vec![],
        updated_at: None,
    }];

    let signals = vec![
        BuilderSignal {
            id: "s_values".into(),
            source_step: 0,
            source_question_id: "q1".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.values".into(),
            proposed_value: serde_json::json!([
                {"name": "健康", "weight": 9, "description": "更新后的价值观"},
                {"name": "创造", "weight": 8, "description": "新增价值观"}
            ]),
            confidence: 0.9,
            reason: "价值观合并".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "s_goals".into(),
            source_step: 0,
            source_question_id: "q2".into(),
            dimension: BuilderDimension::Goals,
            affected_path: "goals.short_term".into(),
            proposed_value: serde_json::json!([
                {"name": "新目标", "priority": 7, "status": "pending", "progress": 0.0}
            ]),
            confidence: 0.9,
            reason: "目标合并".into(),
            risk_level: RiskLevel::Medium,
            user_status: SignalUserStatus::Accepted,
        },
    ];

    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert!(skipped.is_empty());
    assert_eq!(model.identity.values.len(), 2);
    assert_eq!(model.identity.values[0].name, "健康");
    assert_eq!(model.identity.values[0].weight, 9);
    assert!(model.identity.values.iter().any(|v| v.name == "创造"));
    assert_eq!(model.goals.short_term.len(), 2);
    assert!(model.goals.short_term.iter().any(|g| g.name == "现有目标"));
    assert!(model.goals.short_term.iter().any(|g| g.name == "新目标"));
    assert!(applied.iter().any(|s| s == "identity.values (merged)"));
    assert!(applied.iter().any(|s| s == "goals.short_term (merged)"));
}

#[test]
fn apply_signals_supported_path_with_invalid_type_returns_skipped() {
    let mut model = LifeModel::default_model();
    let signals = vec![BuilderSignal {
        id: "s_invalid".into(),
        source_step: 0,
        source_question_id: "q_invalid".into(),
        dimension: BuilderDimension::Goals,
        affected_path: "goals.short_term".into(),
        proposed_value: serde_json::Value::String("not an array".into()),
        confidence: 0.9,
        reason: "类型错误".into(),
        risk_level: RiskLevel::Medium,
        user_status: SignalUserStatus::Accepted,
    }];

    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert!(applied.is_empty());
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].path, "goals.short_term");
    assert!(!skipped[0].reason.is_empty());
    assert_eq!(
        skipped[0].expected.as_deref(),
        Some("array of GoalItem objects")
    );
}

#[test]
fn apply_signals_scalar_invalid_type_returns_skipped() {
    let mut model = LifeModel::default_model();
    let signals = vec![BuilderSignal {
        id: "s_invalid_scalar".into(),
        source_step: 0,
        source_question_id: "q_invalid_scalar".into(),
        dimension: BuilderDimension::State,
        affected_path: "state.health_status.energy_level".into(),
        proposed_value: serde_json::Value::String("high".into()),
        confidence: 0.9,
        reason: "类型错误".into(),
        risk_level: RiskLevel::Medium,
        user_status: SignalUserStatus::Accepted,
    }];

    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert!(applied.is_empty());
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].path, "state.health_status.energy_level");
    assert_eq!(skipped[0].expected.as_deref(), Some("integer 0-10"));
}

#[test]
fn apply_signals_alert_medium_maps_to_warning() {
    let mut model = LifeModel::default_model();
    let signals = vec![BuilderSignal {
        id: "s_alert_medium".into(),
        source_step: 0,
        source_question_id: "q_alert_medium".into(),
        dimension: BuilderDimension::State,
        affected_path: "state.alerts".into(),
        proposed_value: serde_json::json!([
            {"dimension_name": "general", "severity": "medium", "message": "当前压力偏高"}
        ]),
        confidence: 0.8,
        reason: "状态提醒".into(),
        risk_level: RiskLevel::Medium,
        user_status: SignalUserStatus::Accepted,
    }];

    let (_applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert!(skipped.is_empty());
    assert_eq!(model.state.alerts.len(), 1);
    assert!(matches!(
        model.state.alerts[0].level,
        crate::life_model::AlertLevel::Warning
    ));
}

#[test]
fn apply_signals_goal_merge_preserves_existing_details() {
    let mut model = LifeModel::default_model();
    model.goals.short_term = vec![GoalItem {
        name: "现有目标".into(),
        description: "不要丢失的描述".into(),
        priority: 5,
        status: "active".into(),
        progress: 0.4,
        deadline: Some("2026-05-01".into()),
        milestones: vec![crate::life_model::Milestone {
            name: "旧里程碑".into(),
            achieved: false,
            date: Some("2026-04-30".into()),
        }],
        related_memories: vec!["mem-1".into()],
        updated_at: None,
    }];
    let signals = vec![BuilderSignal {
        id: "s_goal_merge".into(),
        source_step: 0,
        source_question_id: "q_goal_merge".into(),
        dimension: BuilderDimension::Goals,
        affected_path: "goals.short_term".into(),
        proposed_value: serde_json::json!([
            {"name": "现有目标", "priority": 8, "milestones": [{"name": "新里程碑", "status": "pending"}]}
        ]),
        confidence: 0.8,
        reason: "目标更新".into(),
        risk_level: RiskLevel::Medium,
        user_status: SignalUserStatus::Accepted,
    }];

    let (_applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert!(skipped.is_empty());
    let goal = &model.goals.short_term[0];
    assert_eq!(goal.description, "不要丢失的描述");
    assert_eq!(goal.priority, 8);
    assert_eq!(goal.status, "active");
    assert_eq!(goal.deadline.as_deref(), Some("2026-05-01"));
    assert!(goal.milestones.iter().any(|m| m.name == "旧里程碑"));
    assert!(goal.milestones.iter().any(|m| m.name == "新里程碑"));
    assert!(goal.related_memories.contains(&"mem-1".to_string()));
}

#[test]
fn apply_signals_value_and_skill_merge_preserve_existing_descriptions() {
    let mut model = LifeModel::default_model();
    model.identity.values = vec![ValueItem {
        name: "健康".into(),
        weight: 6,
        description: "原有描述".into(),
    }];
    model.capabilities.skills = vec![Skill {
        name: "Rust".into(),
        proficiency: 5,
        description: "系统编程经验".into(),
    }];
    let signals = vec![
        BuilderSignal {
            id: "s_value_merge".into(),
            source_step: 0,
            source_question_id: "q_value_merge".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.values".into(),
            proposed_value: serde_json::json!([
                {"name": "健康", "weight": 9, "description": ""}
            ]),
            confidence: 0.9,
            reason: "价值观更新".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "s_skill_merge".into(),
            source_step: 0,
            source_question_id: "q_skill_merge".into(),
            dimension: BuilderDimension::Capabilities,
            affected_path: "capabilities.skills".into(),
            proposed_value: serde_json::json!([
                {"name": "Rust", "proficiency": 8, "description": ""}
            ]),
            confidence: 0.9,
            reason: "技能更新".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Accepted,
        },
    ];

    let (_applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert!(skipped.is_empty());
    assert_eq!(model.identity.values[0].weight, 9);
    assert_eq!(model.identity.values[0].description, "原有描述");
    assert_eq!(model.capabilities.skills[0].proficiency, 8);
    assert_eq!(model.capabilities.skills[0].description, "系统编程经验");
}

#[test]
fn apply_signals_goal_defaults_optional_fields() {
    let mut model = LifeModel::default_model();
    let signals = vec![BuilderSignal {
        id: "s_goal_defaults".into(),
        source_step: 0,
        source_question_id: "q_goal_defaults".into(),
        dimension: BuilderDimension::Goals,
        affected_path: "goals.short_term".into(),
        proposed_value: serde_json::json!([
            {"name": "只提供名称的目标"}
        ]),
        confidence: 0.8,
        reason: "目标默认字段".into(),
        risk_level: RiskLevel::Medium,
        user_status: SignalUserStatus::Accepted,
    }];

    let (_applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
    assert!(skipped.is_empty());
    let goal = &model.goals.short_term[0];
    assert_eq!(goal.name, "只提供名称的目标");
    assert_eq!(goal.priority, 5);
    assert_eq!(goal.status, "pending");
    assert_eq!(goal.progress, 0.0);
}

#[test]
fn apply_signals_realistic_quick_build_review_payload_updates_multiple_dimensions() {
    let mut model = LifeModel::default_model();
    let signals = vec![
        BuilderSignal {
            id: "sig_name".into(),
            source_step: 1,
            source_question_id: "name".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::Value::String("fujing".into()),
            confidence: 0.95,
            reason: "用户直接提供的称呼".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "sig_focus".into(),
            source_step: 2,
            source_question_id: "current_focus".into(),
            dimension: BuilderDimension::State,
            affected_path: "state.current_focus".into(),
            proposed_value: serde_json::Value::String("自我探索".into()),
            confidence: 0.9,
            reason: "用户选择的当前关注主题".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "sig_focus_areas".into(),
            source_step: 2,
            source_question_id: "current_focus".into(),
            dimension: BuilderDimension::State,
            affected_path: "state.focus_areas".into(),
            proposed_value: serde_json::json!(["自我探索"]),
            confidence: 0.85,
            reason: "当前关注作为焦点领域".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "sig_short_term".into(),
            source_step: 3,
            source_question_id: "short_term_goals".into(),
            dimension: BuilderDimension::Goals,
            affected_path: "goals.short_term".into(),
            proposed_value: serde_json::json!([
                {
                    "name": "把 OpenLife 跑通",
                    "priority": 5,
                    "status": "pending",
                    "milestones": [],
                    "description": "",
                    "progress": 0.0
                }
            ]),
            confidence: 0.8,
            reason: "用户描述的近期目标".into(),
            risk_level: RiskLevel::Medium,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "sig_long_term".into(),
            source_step: 4,
            source_question_id: "long_term_direction".into(),
            dimension: BuilderDimension::Goals,
            affected_path: "goals.long_term".into(),
            proposed_value: serde_json::json!([
                {
                    "name": "长期方向: 希望事业收获阶段性的成功",
                    "priority": 5,
                    "status": "pending",
                    "milestones": [],
                    "description": "希望事业收获阶段性的成功",
                    "progress": 0.0
                }
            ]),
            confidence: 0.6,
            reason: "用户描述的长期方向(需要确认)".into(),
            risk_level: RiskLevel::High,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "sig_alert".into(),
            source_step: 6,
            source_question_id: "current_blockers".into(),
            dimension: BuilderDimension::State,
            affected_path: "state.alerts".into(),
            proposed_value: serde_json::json!([
                {
                    "dimension_name": "general",
                    "message": "当前卡点: 方向不明确、拖延",
                    "severity": "medium"
                }
            ]),
            confidence: 0.65,
            reason: "用户主动报告的阻碍".into(),
            risk_level: RiskLevel::Medium,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "sig_comm_style".into(),
            source_step: 7,
            source_question_id: "companion_style".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "preferences.communication_style".into(),
            proposed_value: serde_json::Value::String("苏格拉底式追问型".into()),
            confidence: 0.9,
            reason: "用户选择的陪伴风格".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Accepted,
        },
        BuilderSignal {
            id: "sig_voice".into(),
            source_step: 7,
            source_question_id: "companion_style".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.voice_style.tone_descriptors".into(),
            proposed_value: serde_json::json!(["好奇", "探究"]),
            confidence: 0.85,
            reason: "根据陪伴风格映射的语调特征".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Accepted,
        },
    ];

    let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);

    assert!(skipped.is_empty());
    assert_eq!(model.identity.name, "fujing");
    assert_eq!(model.state.current_focus, "自我探索");
    assert!(model
        .state
        .focus_areas
        .iter()
        .any(|item| item == "自我探索"));
    assert!(model
        .goals
        .short_term
        .iter()
        .any(|goal| goal.name == "把 OpenLife 跑通"));
    assert!(model
        .goals
        .long_term
        .iter()
        .any(|goal| goal.description == "希望事业收获阶段性的成功"));
    assert_eq!(model.preferences.communication_style, "苏格拉底式追问型");
    assert!(model
        .identity
        .voice_style
        .tone_descriptors
        .iter()
        .any(|item| item == "好奇"));
    assert!(model
        .identity
        .voice_style
        .tone_descriptors
        .iter()
        .any(|item| item == "探究"));
    assert_eq!(model.state.alerts.len(), 1);
    assert!(applied.iter().any(|item| item.contains("identity.name")));
    assert!(applied
        .iter()
        .any(|item| item.contains("state.current_focus")));
    assert!(applied.iter().any(|item| item.contains("goals.short_term")));
    assert!(applied.iter().any(|item| item.contains("goals.long_term")));
    assert!(applied
        .iter()
        .any(|item| item.contains("preferences.communication_style")));
}

#[test]
fn builder_dimension_from_str_maps_goals_correctly() {
    use std::str::FromStr;
    assert_eq!(
        BuilderDimension::from_str("goals").unwrap(),
        BuilderDimension::Goals
    );
    assert_eq!(
        BuilderDimension::from_str("identity").unwrap(),
        BuilderDimension::Identity
    );
    assert_eq!(
        BuilderDimension::from_str("capabilities").unwrap(),
        BuilderDimension::Capabilities
    );
    assert_eq!(
        BuilderDimension::from_str("state").unwrap(),
        BuilderDimension::State
    );
}

#[test]
fn builder_dimension_from_str_rejects_invalid() {
    use std::str::FromStr;
    let err = BuilderDimension::from_str("foobar").unwrap_err();
    assert!(err.contains("foobar"));
    assert!(err.contains("identity") || err.contains("goals"));
}

#[test]
fn builder_session_target_dimension_settable() {
    let mut session = BuilderSession::new("s_dim", BuilderMode::Incremental);
    assert!(session.target_dimension.is_none());
    session.target_dimension = Some(BuilderDimension::Goals);
    assert_eq!(session.target_dimension, Some(BuilderDimension::Goals));
}
