use crate::builder::types::*;
use crate::life_model::{LifeModel, ValueItem};
use std::cmp::Reverse;

pub struct BuilderEngine;

impl Default for BuilderEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct DisplayedBuilderQuestion {
    sequence: u64,
    lane: String,
    step: usize,
    question_id: String,
    kind: String,
}

#[derive(Clone, Debug)]
struct PendingSignalSource {
    step: usize,
    question_id: String,
}

impl PendingSignalSource {
    fn new(step: usize, question_id: impl Into<String>) -> Self {
        Self {
            step,
            question_id: question_id.into(),
        }
    }
}

#[derive(Clone, Debug)]
struct PendingSignalAssessment {
    confidence: f32,
    reason: String,
    risk_level: RiskLevel,
}

impl PendingSignalAssessment {
    fn new(confidence: f32, reason: impl Into<String>, risk_level: RiskLevel) -> Self {
        Self {
            confidence,
            reason: reason.into(),
            risk_level,
        }
    }
}

impl BuilderEngine {
    /// Builder is intentionally deterministic and does not retain or invoke a
    /// provider. Assisted enrichment, if reintroduced, must be owned
    /// by the governed TurnRuntime and return review-only typed candidates.
    pub fn new() -> Self {
        Self
    }

    pub async fn next_prompt(
        &self,
        session: &mut BuilderSession,
        user_reply: &str,
        current_model: &LifeModel,
    ) -> (String, Option<LifeModel>) {
        let result = match session.mode {
            BuilderMode::Quick => {
                self.quick_build_step(session, user_reply, current_model)
                    .await
            }
            BuilderMode::Incremental => {
                self.incremental_prompt(session, user_reply, current_model)
                    .await
            }
            BuilderMode::Socratic => self.socratic_step(session, user_reply, current_model).await,
        };
        session.current_prompt = result.0.clone();
        // Update analysis after each step using latest model (or base if not yet produced)
        let draft_model = result.1.as_ref().unwrap_or(current_model);
        session.analysis = Some(BuilderAnalysis {
            completion: draft_model.calculate_4d_completion(),
            gaps: Self::detect_gaps(draft_model),
        });
        result
    }

    pub fn build_analysis(current_model: &LifeModel) -> BuilderAnalysis {
        BuilderAnalysis {
            completion: current_model.calculate_4d_completion(),
            gaps: Self::detect_gaps(current_model),
        }
    }

    fn append_answer_block(session: &mut BuilderSession, lane: &str, step: usize, answer: &str) {
        let answer = answer.trim();
        if answer.is_empty() {
            return;
        }
        let record = serde_json::json!({
            "lane": lane,
            "step": step,
            "answer": answer,
        });
        session
            .draft_yaml
            .push_str(&format!("\n# builder-answer-json {record}\n"));
    }

    fn answer_blocks(draft: &str, lane: &str) -> std::collections::BTreeMap<usize, String> {
        let mut answers = std::collections::BTreeMap::new();
        for line in draft.lines() {
            let Some(encoded) = line.strip_prefix("# builder-answer-json ") else {
                continue;
            };
            let Ok(record) = serde_json::from_str::<serde_json::Value>(encoded) else {
                continue;
            };
            if record.get("lane").and_then(|value| value.as_str()) != Some(lane) {
                continue;
            }
            let Some(step) = record
                .get("step")
                .and_then(|value| value.as_u64())
                .and_then(|value| usize::try_from(value).ok())
            else {
                continue;
            };
            let Some(answer) = record
                .get("answer")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            answers.insert(step, answer.to_string());
        }
        answers
    }

    fn append_displayed_question(
        session: &mut BuilderSession,
        lane: &str,
        step: usize,
        question_id: &str,
        kind: &str,
    ) {
        let sequence = session
            .draft_yaml
            .lines()
            .filter(|line| line.starts_with("# builder-question-json "))
            .count() as u64
            + 1;
        let record = DisplayedBuilderQuestion {
            sequence,
            lane: lane.to_string(),
            step,
            question_id: question_id.to_string(),
            kind: kind.to_string(),
        };
        if let Ok(encoded) = serde_json::to_string(&record) {
            session
                .draft_yaml
                .push_str(&format!("\n# builder-question-json {encoded}\n"));
        }
    }

    fn last_displayed_question(draft: &str) -> Option<DisplayedBuilderQuestion> {
        draft.lines().rev().find_map(|line| {
            line.strip_prefix("# builder-question-json ")
                .and_then(|encoded| serde_json::from_str(encoded).ok())
        })
    }

    fn record_answer_for_displayed_question(
        session: &mut BuilderSession,
        answer: &str,
    ) -> Option<DisplayedBuilderQuestion> {
        let answer = answer.trim();
        if answer.is_empty() {
            return None;
        }

        let displayed = Self::last_displayed_question(&session.draft_yaml).unwrap_or_else(|| {
            DisplayedBuilderQuestion {
                sequence: 0,
                lane: if session.waiting_phase_confirmation {
                    "socratic.confirmation".to_string()
                } else if session.waiting_pairwise {
                    "socratic.pairwise".to_string()
                } else {
                    format!("socratic.{}", session.current_session.max(1))
                },
                step: session.step_index,
                question_id: "legacy_displayed_question".to_string(),
                kind: if session.waiting_phase_confirmation {
                    "confirmation".to_string()
                } else if session.waiting_pairwise {
                    "pairwise".to_string()
                } else {
                    "content".to_string()
                },
            }
        });

        if displayed.kind == "content" {
            let record = serde_json::json!({
                "lane": displayed.lane.clone(),
                "step": displayed.step,
                "question_id": displayed.question_id.clone(),
                "question_sequence": displayed.sequence,
                "answer": answer,
            });
            session
                .draft_yaml
                .push_str(&format!("\n# builder-answer-json {record}\n"));
        }
        Some(displayed)
    }

    fn list_items(answer: &str, limit: usize) -> Vec<String> {
        answer
            .lines()
            .flat_map(|line| line.split(['；', ';']))
            .map(normalize_list_line)
            .filter(|item| !item.is_empty())
            .take(limit)
            .collect()
    }

    fn labeled_items(answer: &str, label: &str, limit: usize) -> Vec<String> {
        answer
            .split(['\n', '；', ';'])
            .filter_map(|segment| {
                let segment = segment.trim();
                segment
                    .strip_prefix(label)
                    .map(|value| value.trim_start_matches(['：', ':']).trim())
            })
            .flat_map(|value| value.split(['、', '，', ',']))
            .map(normalize_list_line)
            .filter(|item| !item.is_empty() && item != "无")
            .take(limit)
            .collect()
    }

    fn labeled_score(answer: &str, labels: &[&str]) -> Option<u8> {
        labels.iter().find_map(|label| {
            let (_, tail) = answer.split_once(label)?;
            let digits = tail
                .trim_start_matches(|ch: char| {
                    ch.is_whitespace() || matches!(ch, '：' | ':' | '=' | '是')
                })
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            digits
                .parse::<u8>()
                .ok()
                .filter(|score| (1..=10).contains(score))
        })
    }

    fn bounded_text(value: &str, max_chars: usize) -> String {
        value.trim().chars().take(max_chars).collect()
    }

    fn pending_signal(
        id: impl Into<String>,
        source: PendingSignalSource,
        dimension: BuilderDimension,
        affected_path: impl Into<String>,
        proposed_value: serde_json::Value,
        assessment: PendingSignalAssessment,
    ) -> BuilderSignal {
        BuilderSignal {
            id: id.into(),
            source_step: source.step,
            source_question_id: source.question_id,
            dimension,
            affected_path: affected_path.into(),
            proposed_value,
            confidence: assessment.confidence,
            reason: assessment.reason,
            risk_level: assessment.risk_level,
            user_status: SignalUserStatus::Pending,
        }
    }

    fn preview_model(base: &LifeModel, signals: &[BuilderSignal]) -> LifeModel {
        let mut preview = base.clone();
        let accepted_for_preview = signals
            .iter()
            .cloned()
            .map(|mut signal| {
                signal.user_status = SignalUserStatus::Accepted;
                signal
            })
            .collect::<Vec<_>>();
        let _ = Self::apply_signals_to_model(&mut preview, &accepted_for_preview);
        preview
    }

    async fn quick_build_step(
        &self,
        session: &mut BuilderSession,
        user_reply: &str,
        current_model: &LifeModel,
    ) -> (String, Option<LifeModel>) {
        if !user_reply.trim().is_empty() {
            Self::append_answer_block(session, "quick", session.step_index, user_reply);
        }

        let step = QUICK_BUILD_STEPS.get(session.step_index);
        if let Some(step_name) = step {
            let prompt = match *step_name {
                "name" => {
                    "【第 1 步/7：称呼】\n\n我应该怎么称呼你？\n可以是真名、昵称，或者你希望 OpenLife 使用的称呼。\n\n例如：小林、Alex、老傅，或者「先叫我用户也行」。".to_string()
                }
                "current_focus" => {
                    "【第 2 步/7：当前人生主题】\n\n你现在最关注的人生主题是什么？\n\n选项（可多选或自定义）：\n• 事业 / 学业\n• 健康 / 精力\n• 情绪 / 状态\n• 关系 / 家庭\n• 财富 / 资源\n• 创造 / 表达\n• 自我探索\n• 暂时说不清\n\n提示：不用选「最正确」的答案。选你最近最常想起、最消耗注意力的那个方向就好。".to_string()
                }
                "short_term_goals" => {
                    "【第 3 步/7：近期目标】\n\n接下来 1-3 个月，你最希望推进哪 1-3 件事？\n\n例如：\n• 找到更稳定的工作节奏\n• 做完一个产品原型\n• 恢复运动习惯\n• 减少焦虑和拖延\n\n我会按你的原意逐行整理为待确认目标草案，不自动补写里程碑、优先级或其他细节。".to_string()
                }
                "long_term_direction" => {
                    "【第 4 步/7：长期方向】⚠️ 需要确认\n\n如果把时间拉长一点，你希望未来 1-3 年的自己变成什么样？\n\n不需要宏大。你可以从生活状态、工作方式、关系、健康、创造力里任选一个角度说。\n\n注意：这里涉及长期方向，OpenLife 只会生成建议，稍后需要你确认才会写入人生模型。".to_string()
                }
                "capabilities" => {
                    "【第 5 步/7：已有能力】\n\n你觉得自己目前有哪些能力、经验或资源？\n哪怕它们还没有被充分发挥，也可以写下来。\n\n例如：\n• 我擅长分析复杂问题\n• 我做过产品/写作/编程/销售\n• 我有一些行业经验\n• 我有一台电脑、固定时间、朋友支持".to_string()
                }
                "current_blockers" => {
                    "【第 6 步/7：当前卡点】\n\n现在最阻碍你前进的是什么？\n\n选项辅助：\n• 时间不够\n• 精力不足\n• 拖延\n• 方向不清晰\n• 能力不够\n• 情绪压力\n• 外部环境限制\n• 缺少支持\n\n可以很具体，也可以很模糊。比如「我不知道为什么就是动不起来」也可以。".to_string()
                }
                "companion_style" => {
                    "【第 7 步/7：陪伴风格】\n\n你希望 OpenLife 用什么方式陪你？\n\n选项：\n• 温和支持型：多鼓励，少压迫\n• 直接高效型：少废话，直接给建议\n• 苏格拉底追问型：多问问题，帮我自己想清楚\n• 教练督促型：提醒我行动，不让我逃避\n• 朋友聊天型：自然一点，像朋友一样陪伴\n• 理性分析型：结构化、逻辑化、客观一点".to_string()
                }
                _ => String::new(),
            };
            session.step_index += 1;
            (prompt, None)
        } else {
            // Step 7 completed: materialize only deterministic, review-pending
            // candidates. The returned model is a preview and is never written.
            let signals = Self::extract_quick_build_signals(session, current_model);
            let model = Self::preview_model(current_model, &signals);
            session.pending_signals = signals;
            session.finished = true;
            (
                "快速构建问题已完成！接下来请审阅根据你回答生成的待确认建议。".to_string(),
                Some(model),
            )
        }
    }

    async fn incremental_prompt(
        &self,
        session: &mut BuilderSession,
        user_reply: &str,
        current_model: &LifeModel,
    ) -> (String, Option<LifeModel>) {
        if !user_reply.trim().is_empty() {
            let lane = match session.target_dimension {
                Some(BuilderDimension::Identity) => "incremental.identity",
                Some(BuilderDimension::Goals) => "incremental.goals",
                Some(BuilderDimension::Capabilities) => "incremental.capabilities",
                Some(BuilderDimension::State) => "incremental.state",
                None => "incremental.unselected",
            };
            Self::append_answer_block(session, lane, session.step_index, user_reply);
        }

        match session.target_dimension {
            Some(BuilderDimension::Identity) => {
                const IDENTITY_STEPS: &[&str] = &[
                    "values",
                    "life_philosophy",
                    "roles",
                    "boundaries",
                    "communication",
                ];
                let total = IDENTITY_STEPS.len();
                if session.step_index < total {
                    let prompt = match IDENTITY_STEPS[session.step_index] {
                        "values" => "【Identity 问题 1/5：核心价值观】\n\n最近一年里，有哪些事情会让你觉得\"这对我很重要，我不想妥协\"？\n\n可以是自由、成长、家人、健康、创造、稳定、影响力，也可以是你自己的说法。\n\n你的回答会帮助我识别你最底层的驱动力。".to_string(),
                        "life_philosophy" => "【Identity 问题 2/5：人生原则】\n\n请用一句你认可的话，描述目前最重要的人生原则或处事哲学。\n\n我会按原文生成 life_philosophy 待确认候选，不替你推断隐藏含义。".to_string(),
                        "roles" => "【Identity 问题 3/5：身份角色】\n\n你现在最重要的几个身份角色是什么？\n比如：创业者、学生、创作者、伴侣、家庭成员、探索者、管理者。\n\n不用排优先级，先把想到的列出来。".to_string(),
                        "boundaries" => "【Identity 问题 4/5：边界保护】\n\n有哪些事情你不希望 OpenLife 推着你去做？\n或者有哪些生活边界你想保护？\n\n比如：不希望周末被提醒工作、不想被push社交、需要保护自己的休息时间等。".to_string(),
                        "communication" => "【Identity 问题 5/5：沟通偏好】\n\n当你状态不好时，你希望 OpenLife 怎么和你说话？\n\n• 温和一点：多鼓励，少压迫\n• 直接一点：少废话，直接给建议\n• 多问问题：帮我自己想清楚\n• 帮我拆步骤：把大目标拆成可执行的小动作\n• 提醒我面对现实：不逃避，直面问题\n• 先共情再建议：先理解情绪，再给结构化建议\n\n选一个最贴近你的，或者描述你自己的偏好。".to_string(),
                        _ => String::new(),
                    };
                    session.step_index += 1;
                    (prompt, None)
                } else {
                    let signals =
                        Self::extract_incremental_signals(session, BuilderDimension::Identity);
                    let model = Self::preview_model(current_model, &signals);
                    session.pending_signals = signals;
                    session.finished = true;
                    (
                        "Identity 维度的问题已回答完毕！接下来请审阅根据你回答生成的待确认建议。"
                            .to_string(),
                        Some(model),
                    )
                }
            }
            Some(BuilderDimension::Goals) => {
                const GOALS_STEPS: &[&str] = &["braindump", "prioritize", "deep_dive", "blockers"];
                let total = GOALS_STEPS.len();
                if session.step_index < total {
                    let prompt = match GOALS_STEPS[session.step_index] {
                        "braindump" => "【Goals 问题 1/4：目标倾倒】\n\n现在你脑子里反复出现、觉得应该推进的事情有哪些？\n不用排序，先全部写出来。\n\n哪怕是\"应该做但还没开始\"的也可以写。".to_string(),
                        "prioritize" => "【Goals 问题 2/4：优先级聚焦】\n\n如果未来 90 天只能认真推进 1-2 件事，你会选什么？为什么？\n\n不用考虑\"应该\"，只考虑\"我现在真的想做、且有条件推进的\"。".to_string(),
                        "deep_dive" => "【Goals 问题 3/4：深层动机】\n\n这个目标真正重要的原因是什么？\n如果完成了，它会改变你的生活状态、身份感，还是现实处境？\n\n试着往深处想一层：这个目标满足了你什么底层需求？".to_string(),
                        "blockers" => "【Goals 问题 4/4：阻碍识别】\n\n你过去没有推进它，主要是因为什么？\n\n• 目标太大，不知从何下手\n• 缺少时间\n• 缺少能力或知识\n• 害怕失败或被评判\n• 没有反馈，动力不足\n• 状态不稳定\n• 其实没那么想要\n\n选一个最贴切的，或者描述你自己的情况。".to_string(),
                        _ => String::new(),
                    };
                    session.step_index += 1;
                    (prompt, None)
                } else {
                    let signals =
                        Self::extract_incremental_signals(session, BuilderDimension::Goals);
                    let model = Self::preview_model(current_model, &signals);
                    session.pending_signals = signals;
                    session.finished = true;
                    (
                        "Goals 维度的问题已回答完毕！接下来请审阅根据你回答生成的待确认建议。"
                            .to_string(),
                        Some(model),
                    )
                }
            }
            Some(BuilderDimension::Capabilities) => {
                const CAP_STEPS: &[&str] = &[
                    "natural_skills",
                    "knowledge_domains",
                    "resources",
                    "learning_style",
                ];
                let total = CAP_STEPS.len();
                if session.step_index < total {
                    let prompt = match CAP_STEPS[session.step_index] {
                        "natural_skills" => "【Capabilities 问题 1/4：自然能力】\n\n哪些事情是你做起来比较自然，或者别人曾经认可过你的？\n\n哪怕是\"小事\"也可以：比如擅长倾听、总能发现别人忽略的细节、能把复杂概念讲清楚。".to_string(),
                        "knowledge_domains" => "【Capabilities 问题 2/4：知识领域】\n\n你通过工作、项目或学习，明确积累了哪些知识领域？请逐行列出领域名称。\n\n未询问熟练等级，因此 level 会保持 0（未量化）。".to_string(),
                        "resources" => "【Capabilities 问题 3/4：可调用的资源】\n\n你现在有哪些可以调用的资源？\n\n比如：\n• 时间（每天/每周能投入多少？）\n• 设备（电脑、软件、工具）\n• 资金\n• 已完成的作品/项目\n• 平台/渠道\n• 人脉/社群\n• 环境（安静的空间、图书馆等）".to_string(),
                        "learning_style" => "【Capabilities 问题 4/4：学习方式】\n\n当你要补一个能力时，你更适合哪种方式？\n\n• 直接做项目：在实践中学习\n• 看系统课程：结构化学习\n• 读文档/书：自己研究\n• 找人交流：向有经验的人请教\n• 让 AI 陪跑：边问边做\n• 写总结复盘：通过输出倒逼输入\n\n选一个或几个最贴近你的。".to_string(),
                        _ => String::new(),
                    };
                    session.step_index += 1;
                    (prompt, None)
                } else {
                    let signals =
                        Self::extract_incremental_signals(session, BuilderDimension::Capabilities);
                    let model = Self::preview_model(current_model, &signals);
                    session.pending_signals = signals;
                    session.finished = true;
                    ("Capabilities 维度的问题已回答完毕！接下来请审阅根据你回答生成的待确认建议。".to_string(), Some(model))
                }
            }
            Some(BuilderDimension::State) => {
                const STATE_STEPS: &[&str] = &[
                    "current_state",
                    "energy_stress",
                    "current_focus",
                    "habits_tracking",
                ];
                let total = STATE_STEPS.len();
                if session.step_index < total {
                    let prompt = match STATE_STEPS[session.step_index] {
                        "current_state" => "【State 问题 1/4：当前状态】\n\n如果用 3 个词描述你最近的状态，会是什么？\n\n比如：兴奋、焦虑、疲惫、混乱、专注、期待、卡住、平静。\n\n不需要\"正确\"的答案，当下的真实感受就可以。".to_string(),
                        "energy_stress" => "【State 问题 2/4：精力与压力】\n\n最近一周你的精力和压力分别是多少？请按格式回答：\n精力：7/10；压力：6/10\n\n只有带标签且在 1-10 范围内的分数会形成候选。".to_string(),
                        "current_focus" => "【State 问题 3/4：当前关注】\n\n基于刚才的状态，接下来你最想优先关注和调整的一个领域是什么？\n\n请直接写领域名称；它会作为 current_focus 待确认候选。".to_string(),
                        "habits_tracking" => "【State 问题 4/4：习惯与追踪】\n\n你现在有哪些想维持、恢复或建立的小习惯？\n\n另外，如果 OpenLife 每天或每周帮你观察一个状态指标，你最想观察什么？\n\n• 专注度\n• 睡眠\n• 运动\n• 情绪稳定度\n• 创作产出\n• 学习投入\n• 社交能量\n• 压力水平".to_string(),
                        _ => String::new(),
                    };
                    session.step_index += 1;
                    (prompt, None)
                } else {
                    let signals =
                        Self::extract_incremental_signals(session, BuilderDimension::State);
                    let model = Self::preview_model(current_model, &signals);
                    session.pending_signals = signals;
                    session.finished = true;
                    (
                        "State 维度的问题已回答完毕！接下来请审阅根据你回答生成的待确认建议。"
                            .to_string(),
                        Some(model),
                    )
                }
            }
            None => ("请先选择一个要构建的维度。".to_string(), None),
        }
    }

    fn extract_incremental_signals(
        session: &BuilderSession,
        dimension: BuilderDimension,
    ) -> Vec<BuilderSignal> {
        let lane = match dimension {
            BuilderDimension::Identity => "incremental.identity",
            BuilderDimension::Goals => "incremental.goals",
            BuilderDimension::Capabilities => "incremental.capabilities",
            BuilderDimension::State => "incremental.state",
        };
        let answers = Self::answer_blocks(&session.draft_yaml, lane);
        let answer = |step: usize| answers.get(&step).map(String::as_str).unwrap_or("");
        let mut signals = Vec::new();

        match dimension {
            BuilderDimension::Identity => {
                let values = Self::list_items(answer(1), 6)
                    .into_iter()
                    .map(|name| {
                        serde_json::json!({
                            "name": Self::bounded_text(&name, 80),
                            "weight": 0,
                            "description": "用户在 Identity 构建中直接提供；weight=0 表示未量化"
                        })
                    })
                    .collect::<Vec<_>>();
                if !values.is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_identity_values",
                        PendingSignalSource::new(1, "values"),
                        dimension,
                        "identity.values",
                        serde_json::Value::Array(values),
                        PendingSignalAssessment::new(
                            0.95,
                            "用户直接列出的核心价值观；权重保持未量化",
                            RiskLevel::Medium,
                        ),
                    ));
                }
                if !answer(2).trim().is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_identity_life_philosophy",
                        PendingSignalSource::new(2, "life_philosophy"),
                        dimension,
                        "identity.life_philosophy",
                        serde_json::json!(answer(2).trim()),
                        PendingSignalAssessment::new(
                            0.95,
                            "用户对人生原则问题的原始回答",
                            RiskLevel::High,
                        ),
                    ));
                }
                let roles = Self::list_items(answer(3), 6);
                if !roles.is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_identity_roles",
                        PendingSignalSource::new(3, "roles"),
                        dimension,
                        "identity.role_definition.secondary_roles",
                        serde_json::json!(roles),
                        PendingSignalAssessment::new(
                            0.9,
                            "用户直接列出的身份角色",
                            RiskLevel::Medium,
                        ),
                    ));
                }
                let boundaries = Self::list_items(answer(4), 6);
                if !boundaries.is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_identity_boundaries",
                        PendingSignalSource::new(4, "boundaries"),
                        dimension,
                        "identity.role_definition.boundaries",
                        serde_json::json!(boundaries),
                        PendingSignalAssessment::new(0.95, "用户直接声明的边界", RiskLevel::High),
                    ));
                }
                if !answer(5).trim().is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_identity_communication",
                        PendingSignalSource::new(5, "communication"),
                        dimension,
                        "preferences.communication_style",
                        serde_json::json!(answer(5).trim()),
                        PendingSignalAssessment::new(
                            0.95,
                            "用户直接选择的沟通偏好",
                            RiskLevel::Low,
                        ),
                    ));
                }
            }
            BuilderDimension::Goals => {
                let selected = if answer(2).trim().is_empty() {
                    answer(1)
                } else {
                    answer(2)
                };
                let motivation = answer(3).trim();
                let blocker = answer(4).trim();
                let goals = Self::list_items(selected, 4)
                    .into_iter()
                    .map(|name| {
                        let mut context = Vec::new();
                        if !motivation.is_empty() {
                            context.push(format!("重要原因：{motivation}"));
                        }
                        if !blocker.is_empty() {
                            context.push(format!("当前阻碍：{blocker}"));
                        }
                        serde_json::json!({
                            "name": Self::bounded_text(&name, 100),
                            "priority": 0,
                            "status": "pending",
                            "progress": 0.0,
                            "milestones": [],
                            "description": context.join("\n")
                        })
                    })
                    .collect::<Vec<_>>();
                if !goals.is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_goals_short_term",
                        PendingSignalSource::new(2, "prioritize"),
                        dimension,
                        "goals.short_term",
                        serde_json::Value::Array(goals),
                        PendingSignalAssessment::new(
                            0.9,
                            "用户直接选择的 90 天目标；priority=0 表示未量化",
                            RiskLevel::Medium,
                        ),
                    ));
                }
            }
            BuilderDimension::Capabilities => {
                let skills = Self::list_items(answer(1), 6)
                    .into_iter()
                    .map(|name| {
                        serde_json::json!({
                            "name": Self::bounded_text(&name, 80),
                            "proficiency": 0,
                            "description": "用户自报的自然能力；proficiency=0 表示未量化"
                        })
                    })
                    .collect::<Vec<_>>();
                if !skills.is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_capabilities_skills",
                        PendingSignalSource::new(1, "natural_skills"),
                        dimension,
                        "capabilities.skills",
                        serde_json::Value::Array(skills),
                        PendingSignalAssessment::new(
                            0.85,
                            "用户直接报告的能力；熟练度保持未量化",
                            RiskLevel::Medium,
                        ),
                    ));
                }
                let domains = Self::list_items(answer(2), 8)
                    .into_iter()
                    .map(|domain| {
                        serde_json::json!({
                            "domain": domain,
                            "level": 0,
                            "description": "用户明确列出；level=0 表示未量化",
                        })
                    })
                    .collect::<Vec<_>>();
                if !domains.is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_capabilities_knowledge_domains",
                        PendingSignalSource::new(2, "knowledge_domains"),
                        dimension,
                        "capabilities.knowledge_domains",
                        serde_json::Value::Array(domains),
                        PendingSignalAssessment::new(
                            0.95,
                            "用户直接列出的知识领域；等级保持未量化",
                            RiskLevel::Medium,
                        ),
                    ));
                }
                let resources = Self::list_items(answer(3), 8)
                    .into_iter()
                    .map(|name| {
                        serde_json::json!({
                            "name": Self::bounded_text(&name, 100),
                            "type": "unknown",
                            "description": "用户直接报告的可调用资源",
                            "availability": "unknown"
                        })
                    })
                    .collect::<Vec<_>>();
                if !resources.is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_capabilities_resources",
                        PendingSignalSource::new(3, "resources"),
                        dimension,
                        "capabilities.resources",
                        serde_json::Value::Array(resources),
                        PendingSignalAssessment::new(0.85, "用户直接报告的资源", RiskLevel::Medium),
                    ));
                }
                if !answer(4).trim().is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_capabilities_learning_style",
                        PendingSignalSource::new(4, "learning_style"),
                        dimension,
                        "preferences.learning_style",
                        serde_json::json!(answer(4).trim()),
                        PendingSignalAssessment::new(0.9, "用户直接选择的学习方式", RiskLevel::Low),
                    ));
                }
            }
            BuilderDimension::State => {
                if !answer(1).trim().is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_state_mood",
                        PendingSignalSource::new(1, "current_state"),
                        dimension,
                        "state.emotional_state.current_mood",
                        serde_json::json!(answer(1).trim()),
                        PendingSignalAssessment::new(
                            0.9,
                            "用户对当前状态的直接描述",
                            RiskLevel::Medium,
                        ),
                    ));
                }
                if let Some(energy) = Self::labeled_score(answer(2), &["精力", "能量"]) {
                    signals.push(Self::pending_signal(
                        "incremental_state_energy",
                        PendingSignalSource::new(2, "energy_stress"),
                        dimension,
                        "state.health_status.energy_level",
                        serde_json::json!(energy),
                        PendingSignalAssessment::new(
                            0.95,
                            "用户直接给出的精力评分",
                            RiskLevel::Medium,
                        ),
                    ));
                }
                if let Some(stress) = Self::labeled_score(answer(2), &["压力"]) {
                    signals.push(Self::pending_signal(
                        "incremental_state_stress",
                        PendingSignalSource::new(2, "energy_stress"),
                        dimension,
                        "state.emotional_state.stress_level",
                        serde_json::json!(stress),
                        PendingSignalAssessment::new(
                            0.95,
                            "用户直接给出的压力评分",
                            RiskLevel::Medium,
                        ),
                    ));
                }
                if !answer(3).trim().is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_state_current_focus",
                        PendingSignalSource::new(3, "current_focus"),
                        dimension,
                        "state.current_focus",
                        serde_json::json!(answer(3).trim()),
                        PendingSignalAssessment::new(
                            0.95,
                            "用户对当前关注问题的原始回答",
                            RiskLevel::Low,
                        ),
                    ));
                }
                let focus_areas = Self::list_items(answer(4), 8);
                if !focus_areas.is_empty() {
                    signals.push(Self::pending_signal(
                        "incremental_state_focus_areas",
                        PendingSignalSource::new(4, "habits_tracking"),
                        dimension,
                        "state.focus_areas",
                        serde_json::json!(focus_areas),
                        PendingSignalAssessment::new(
                            0.8,
                            "用户直接列出的习惯或观察指标",
                            RiskLevel::Medium,
                        ),
                    ));
                }
            }
        }

        signals
    }

    async fn socratic_step(
        &self,
        session: &mut BuilderSession,
        user_reply: &str,
        current_model: &LifeModel,
    ) -> (String, Option<LifeModel>) {
        const MAX_TURNS: usize = 8;

        if session.step_index == 0 && user_reply.trim().is_empty() {
            return Self::emit_next_socratic_content_prompt(session, current_model);
        }

        let was_confirmation = session.waiting_phase_confirmation;
        let displayed = Self::record_answer_for_displayed_question(session, user_reply);
        let answered_content_step = displayed
            .as_ref()
            .filter(|question| question.kind == "content")
            .map(|question| question.step);

        if was_confirmation {
            if Self::is_socratic_confirmation(user_reply) {
                session.waiting_phase_confirmation = false;
                session.phase_summary = None;
            } else {
                Self::append_answer_block(
                    session,
                    "socratic.confirmation_correction",
                    session.step_index,
                    user_reply,
                );
                return Self::emit_socratic_hypothesis(session, current_model);
            }
        }

        if session.waiting_pairwise {
            return self
                .handle_pairwise_input(session, user_reply, current_model)
                .await;
        }

        if user_reply.trim().is_empty() {
            return (session.current_prompt.clone(), None);
        }

        if answered_content_step == Some(2) && session.peak_experience.is_none() {
            Self::extract_values_and_setup_pairwise(session);
            if session.waiting_pairwise {
                if let Some((a, b)) = session.pending_pairwise.first().cloned() {
                    return Self::emit_pairwise_prompt(session, &a, &b);
                }
            }
        }

        if !was_confirmation && matches!(answered_content_step, Some(3 | 6)) {
            return Self::emit_socratic_hypothesis(session, current_model);
        }

        if answered_content_step == Some(MAX_TURNS) {
            session.finished = true;
            let signals = Self::extract_socratic_signals(session);
            let model = Self::preview_model(current_model, &signals);
            session.pending_signals = signals;
            return (
                "苏格拉底式对话已完成！我已将你的明确回答整理成待确认建议，请审阅后再决定是否应用。".to_string(),
                Some(model),
            );
        }

        Self::emit_next_socratic_content_prompt(session, current_model)
    }

    fn is_socratic_confirmation(reply: &str) -> bool {
        matches!(
            reply.trim().to_lowercase().as_str(),
            "确认" | "继续" | "确认继续" | "确认并继续" | "确认，继续" | "可以继续" | "continue"
        )
    }

    fn emit_next_socratic_content_prompt(
        session: &mut BuilderSession,
        current_model: &LifeModel,
    ) -> (String, Option<LifeModel>) {
        let next_step = session.step_index.saturating_add(1);
        let Some((session_number, question_id, prompt)) =
            Self::socratic_prompt_for_step(next_step, session, current_model)
        else {
            return (session.current_prompt.clone(), None);
        };

        session.step_index = next_step;
        session.current_session = session_number;
        session.draft_yaml.push_str(&format!(
            "\nAssistant [S{session_number}-T{next_step}]: {prompt}"
        ));
        Self::append_displayed_question(
            session,
            &format!("socratic.{session_number}"),
            next_step,
            question_id,
            "content",
        );
        (prompt, None)
    }

    fn emit_socratic_hypothesis(
        session: &mut BuilderSession,
        current_model: &LifeModel,
    ) -> (String, Option<LifeModel>) {
        let hypothesis = Self::generate_socratic_hypothesis(session, current_model);
        session.waiting_phase_confirmation = true;
        session.phase_summary = Some(hypothesis.clone());
        session
            .draft_yaml
            .push_str(&format!("\nAssistant: {hypothesis}"));
        Self::append_displayed_question(
            session,
            "socratic.confirmation",
            session.step_index,
            if session.step_index == 3 {
                "phase_one_confirmation"
            } else {
                "phase_two_confirmation"
            },
            "confirmation",
        );
        (hypothesis, None)
    }

    fn emit_pairwise_prompt(
        session: &mut BuilderSession,
        a: &str,
        b: &str,
    ) -> (String, Option<LifeModel>) {
        let prompt = Self::pairwise_prompt(a, b);
        session
            .draft_yaml
            .push_str(&format!("\nAssistant: {prompt}"));
        Self::append_displayed_question(
            session,
            "socratic.pairwise",
            session.step_index,
            "value_pairwise",
            "pairwise",
        );
        (prompt, None)
    }

    fn socratic_prompt_for_step(
        step: usize,
        session: &BuilderSession,
        current_model: &LifeModel,
    ) -> Option<(u8, &'static str, String)> {
        let top_values = session
            .peak_experience
            .as_ref()
            .map(|peak| peak.extracted_values.join("、"))
            .filter(|values| !values.is_empty())
            .or_else(|| {
                let values = session
                    .extracted_values
                    .iter()
                    .map(|value| value.name.clone())
                    .collect::<Vec<_>>()
                    .join("、");
                (!values.is_empty()).then_some(values)
            })
            .unwrap_or_else(|| {
                current_model
                    .identity
                    .values
                    .iter()
                    .take(3)
                    .map(|value| value.name.clone())
                    .collect::<Vec<_>>()
                    .join("、")
            });
        let goals_hint = current_model
            .goals
            .short_term
            .iter()
            .chain(current_model.goals.medium_term.iter())
            .chain(current_model.goals.long_term.iter())
            .take(2)
            .map(|goal| goal.name.clone())
            .collect::<Vec<_>>()
            .join("、");

        match step {
            1 => Some((
                1,
                "peak_experience",
                "欢迎来到 OpenLife 的苏格拉底式构建模式。我们将通过 4 次简短对话，逐步勾勒你的人生模型。\n\n【会话 1/4：价值观与峰值体验】\n请回忆一次让你感到最有活力、最投入的「峰值体验」。当时你在做什么？那种体验里什么最吸引你？".to_string(),
            )),
            2 => Some((
                1,
                "peak_experience_follow_up",
                "在那次峰值体验里，最让你有力量感的瞬间是什么？它满足了你怎样的内在需要？".to_string(),
            )),
            3 => Some((
                2,
                "primary_role",
                if top_values.is_empty() {
                    "如果把你想成为的人浓缩成一个角色，你会用哪个角色名称？请只写一个你认可的角色名称；下一步我们再讨论使命。".to_string()
                } else {
                    format!("结合你重视的 {top_values}，如果把自己浓缩成一个角色，你会用哪个角色名称？请只写一个你认可的角色名称。")
                },
            )),
            4 => Some((
                2,
                "mission",
                "这个角色最想为谁创造什么影响？如果只能留下一个长期使命，你会怎么描述它？".to_string(),
            )),
            5 => Some((
                3,
                "long_term_goal",
                if goals_hint.is_empty() {
                    "未来 1 到 3 年，最值得你投入的一个核心目标是什么？为什么它现在重要？".to_string()
                } else {
                    format!("基于你已经提到的方向（{goals_hint}），如果只选一个最关键目标，它会是什么？为什么现在最重要？")
                },
            )),
            6 => Some((
                3,
                "goal_milestone",
                "为了让这个目标真正发生，未来 90 天最关键的里程碑是什么？你准备用什么标准判断自己在前进？".to_string(),
            )),
            7 => Some((
                4,
                "capabilities_and_resources",
                "要实现这个目标，你已经具备哪些能力、资源和支持网络？请按三行回答：\n能力：…\n资源：…\n支持网络：…\n没有的项目可写「无」。熟练度未询问，将保持未量化。".to_string(),
            )),
            8 => Some((
                4,
                "capability_gap",
                "关于仍缺少的能力、习惯或支持条件，你下一步最需要回答的一个问题是什么？请把它写成一个明确的问题；它会作为待确认的 open question 保留。".to_string(),
            )),
            _ => None,
        }
    }

    fn generate_socratic_hypothesis(
        session: &BuilderSession,
        _current_model: &LifeModel,
    ) -> String {
        let mut lines = vec![];
        lines.push("📋 我暂时这样理解你".to_string());
        lines.push("".to_string());
        let names = session
            .peak_experience
            .as_ref()
            .map(|peak| peak.extracted_values.clone())
            .filter(|values| !values.is_empty())
            .unwrap_or_else(|| {
                session
                    .extracted_values
                    .iter()
                    .map(|value| value.name.clone())
                    .collect()
            });
        if !names.is_empty() {
            lines.push(format!("你重视：{}", names.join("、")));
        }
        if let Some(peak) = &session.peak_experience {
            if !peak.extracted_role_hints.is_empty() {
                lines.push(format!(
                    "角色倾向：{}",
                    peak.extracted_role_hints.join("、")
                ));
            }
            if !peak.extracted_capability_hints.is_empty() {
                lines.push(format!(
                    "能力底色：{}",
                    peak.extracted_capability_hints.join("、")
                ));
            }
        }
        if session.pairwise_results.len() > 1 {
            let mut counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for (_, _, choice) in &session.pairwise_results {
                *counts.entry(choice.clone()).or_insert(0) += 1;
            }
            let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
            sorted.sort_by_key(|item| Reverse(item.1));
            if let Some((top, _)) = sorted.first() {
                lines.push(format!("价值排序中最优先的是：{}", top));
            }
        }
        for (lane, step, label) in [
            ("socratic.1", 1, "峰值体验"),
            ("socratic.1", 2, "内在需要"),
            ("socratic.2", 3, "角色"),
            ("socratic.2", 4, "使命"),
            ("socratic.3", 5, "长期目标"),
            ("socratic.3", 6, "里程碑与判断标准"),
        ] {
            if let Some(answer) = Self::answer_blocks(&session.draft_yaml, lane).get(&step) {
                lines.push(format!("{label}：{}", Self::bounded_text(answer, 180)));
            }
        }
        if let Some(correction) =
            Self::answer_blocks(&session.draft_yaml, "socratic.confirmation_correction")
                .get(&session.step_index)
        {
            lines.push(format!("你的修正：{}", Self::bounded_text(correction, 180)));
        }
        lines.push("".to_string());
        lines.push("请回复「确认」继续，或补充修正我的理解。".to_string());
        lines.join("\n")
    }

    fn generate_pairwise_explanation(session: &BuilderSession) -> String {
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (_, _, choice) in &session.pairwise_results {
            *counts.entry(choice.clone()).or_insert(0) += 1;
        }
        let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
        sorted.sort_by_key(|item| Reverse(item.1));
        let mut lines = vec!["根据你的回答，我整理出了你最重视的价值排序：".to_string()];
        for (i, (name, count)) in sorted.iter().enumerate() {
            lines.push(format!("{}. {}（在 {} 次比较中胜出）", i + 1, name, count));
        }
        if let Some(peak) = &session.peak_experience {
            if !peak.emotional_signal.is_empty() {
                lines.push(format!(
                    "\n这与你峰值体验中流露的「{}」情绪高度一致。",
                    peak.emotional_signal
                ));
            }
            if !peak.extracted_role_hints.is_empty() {
                lines.push(format!(
                    "你提到的角色暗示（{}）也指向了相似的方向。",
                    peak.extracted_role_hints.join("、")
                ));
            }
            if !peak.extracted_capability_hints.is_empty() {
                lines.push(format!(
                    "这些价值排序背后，似乎有你擅长「{}」的能力底色。",
                    peak.extracted_capability_hints.join("、")
                ));
            }
        }
        lines.push("\n接下来让我们基于这些价值观，继续探索你的角色与使命。".to_string());
        lines.join("\n")
    }

    fn extract_values_and_setup_pairwise(session: &mut BuilderSession) {
        let answer = Self::answer_blocks(&session.draft_yaml, "socratic.1")
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        let value_names = Self::explicit_value_candidates(&answer);
        let peak = PeakExperience {
            raw_description: answer,
            extracted_values: value_names.clone(),
            ..Default::default()
        };
        session.extracted_values = value_names
            .iter()
            .map(|name| ValueItem {
                name: name.clone(),
                weight: 0,
                description: "用户明确提供；weight=0 表示未量化".to_string(),
            })
            .collect();
        session.peak_experience = Some(peak);
        if value_names.len() >= 2 {
            session.pending_pairwise = Self::generate_pairwise_pairs(&value_names);
            session.waiting_pairwise = true;
            session.draft_yaml.push_str(&format!(
                "\n[System] 用户明确提供的价值观关键词: {:?}\n",
                value_names
            ));
        }
    }

    fn explicit_value_candidates(answer: &str) -> Vec<String> {
        let explicit = ["内在需要是", "需要是", "价值观是", "我重视", "重视的是"]
            .into_iter()
            .find_map(|marker| answer.rsplit_once(marker).map(|(_, value)| value));
        let Some(explicit) = explicit else {
            return Vec::new();
        };
        explicit
            .split(['、', '，', ',', '；', ';', '和'])
            .map(|value| {
                value.trim_matches(|ch: char| ch.is_whitespace() || "。.!！?？:：".contains(ch))
            })
            .filter(|value| !value.is_empty())
            .map(|value| Self::bounded_text(value, 32))
            .take(6)
            .collect()
    }

    fn generate_pairwise_pairs(names: &[String]) -> Vec<(String, String)> {
        let mut pairs = vec![];
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                pairs.push((names[i].clone(), names[j].clone()));
            }
        }
        if pairs.len() > 6 {
            pairs.truncate(6);
        }
        pairs
    }

    fn pairwise_prompt(a: &str, b: &str) -> String {
        format!(
            "基于你的峰值体验，我提炼出一些明确出现的价值关键词。让我们做个两两比较，看看哪些对你更重要。\n\nA：{a}\nB：{b}\n\n请回复 A 或 B（也可以直接描述你的选择）。"
        )
    }

    async fn handle_pairwise_input(
        &self,
        session: &mut BuilderSession,
        user_reply: &str,
        current_model: &LifeModel,
    ) -> (String, Option<LifeModel>) {
        if let Some((a, b)) = session.pending_pairwise.first().cloned() {
            let choice = user_reply.trim();
            let result = if choice.eq_ignore_ascii_case("a") {
                a.clone()
            } else if choice.eq_ignore_ascii_case("b") {
                b.clone()
            } else {
                choice.to_string()
            };
            session.pairwise_results.push((a, b, result));
            session.pending_pairwise.remove(0);
        }

        if let Some((a, b)) = session.pending_pairwise.first().cloned() {
            return Self::emit_pairwise_prompt(session, &a, &b);
        }

        // Pairwise done
        session.waiting_pairwise = false;
        let explanation = Self::generate_pairwise_explanation(session);
        let values_summary = session
            .pairwise_results
            .iter()
            .map(|(a, b, r)| format!("- {} vs {} -> 选择: {}", a, b, r))
            .collect::<Vec<_>>()
            .join("\n");
        session
            .draft_yaml
            .push_str(&format!("\n[System] 价值排序结果:\n{}\n", values_summary));
        session
            .draft_yaml
            .push_str(&format!("\nAssistant: {explanation}"));
        let (next_prompt, preview) =
            Self::emit_next_socratic_content_prompt(session, current_model);
        (format!("{explanation}\n\n{next_prompt}"), preview)
    }

    fn socratic_session_answers(session: &BuilderSession, session_number: u8) -> Vec<String> {
        let lane = format!("socratic.{session_number}");
        let tagged = Self::answer_blocks(&session.draft_yaml, &lane)
            .into_values()
            .collect::<Vec<_>>();
        if !tagged.is_empty() {
            return tagged;
        }

        // Resume compatibility for sessions persisted before typed answer
        // blocks existed. This path only preserves explicit user text; it does
        // not infer semantic fields from assistant or system transcript lines.
        session
            .draft_yaml
            .lines()
            .filter_map(|line| line.strip_prefix("User: "))
            .map(str::trim)
            .filter(|answer| {
                !answer.is_empty()
                    && *answer != "确认"
                    && !answer.eq_ignore_ascii_case("a")
                    && !answer.eq_ignore_ascii_case("b")
            })
            .enumerate()
            .filter(|(index, _)| ((*index / 2) + 1).min(4) == session_number as usize)
            .map(|(_, answer)| answer.to_string())
            .collect()
    }

    fn extract_socratic_signals(session: &BuilderSession) -> Vec<BuilderSignal> {
        let mut signals = Vec::new();

        if let Some(peak) = session.peak_experience.as_ref() {
            let values = peak
                .extracted_values
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "weight": 0,
                        "description": "用户明确提供；weight=0 表示未量化",
                    })
                })
                .collect::<Vec<_>>();
            if !values.is_empty() {
                signals.push(Self::pending_signal(
                    "socratic_explicit_values",
                    PendingSignalSource::new(2, "peak_experience_follow_up"),
                    BuilderDimension::Identity,
                    "identity.values",
                    serde_json::Value::Array(values),
                    PendingSignalAssessment::new(
                        0.9,
                        "用户明确提供的价值词；权重保持未量化",
                        RiskLevel::Medium,
                    ),
                ));
            }
        }

        let session_two = Self::answer_blocks(&session.draft_yaml, "socratic.2");
        if let Some(role) = session_two
            .get(&3)
            .map(String::as_str)
            .map(str::trim)
            .filter(|answer| !answer.is_empty())
        {
            signals.push(Self::pending_signal(
                "socratic_primary_role",
                PendingSignalSource::new(3, "primary_role"),
                BuilderDimension::Identity,
                "identity.role_definition.primary_role",
                serde_json::json!(Self::bounded_text(role, 100)),
                PendingSignalAssessment::new(
                    0.95,
                    "用户对单一角色名称问题的原始回答",
                    RiskLevel::High,
                ),
            ));
        }
        let mission = session_two.get(&4).cloned().or_else(|| {
            Self::socratic_session_answers(session, 2)
                .into_iter()
                .last()
        });
        if let Some(mission) = mission.filter(|answer| !answer.trim().is_empty()) {
            signals.push(Self::pending_signal(
                "socratic_mission",
                PendingSignalSource::new(4, "mission"),
                BuilderDimension::Identity,
                "identity.mission_statement",
                serde_json::json!(mission.trim()),
                PendingSignalAssessment::new(
                    0.95,
                    "用户对已展示使命问题的原始回答",
                    RiskLevel::High,
                ),
            ));
        }

        let session_three = Self::answer_blocks(&session.draft_yaml, "socratic.3");
        let goal = session_three.get(&5).cloned().or_else(|| {
            Self::socratic_session_answers(session, 3)
                .into_iter()
                .next()
        });
        let goal_context = session_three.get(&6).cloned().or_else(|| {
            Self::socratic_session_answers(session, 3)
                .into_iter()
                .nth(1)
        });
        if let Some(goal) = goal.filter(|answer| !answer.trim().is_empty()) {
            let description = goal_context
                .filter(|answer| !answer.trim().is_empty())
                .map(|context| format!("{}\n{}", goal.trim(), context.trim()))
                .unwrap_or_else(|| goal.trim().to_string());
            signals.push(Self::pending_signal(
                "socratic_long_term_goal",
                PendingSignalSource::new(5, "long_term_goal"),
                BuilderDimension::Goals,
                "goals.long_term",
                serde_json::json!([{
                    "name": Self::bounded_text(goal.trim(), 100),
                    "priority": 0,
                    "status": "pending",
                    "progress": 0.0,
                    "milestones": [],
                    "description": description,
                }]),
                PendingSignalAssessment::new(
                    0.8,
                    "用户对已展示目标问题的原始回答；priority=0 表示未量化",
                    RiskLevel::High,
                ),
            ));
        }

        let session_four = Self::answer_blocks(&session.draft_yaml, "socratic.4");
        if let Some(capability_answer) = session_four.get(&7) {
            let skills = Self::labeled_items(capability_answer, "能力", 8)
                .into_iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "proficiency": 0,
                        "description": "用户明确列出；proficiency=0 表示未量化",
                    })
                })
                .collect::<Vec<_>>();
            if !skills.is_empty() {
                signals.push(Self::pending_signal(
                    "socratic_capabilities",
                    PendingSignalSource::new(7, "capabilities_and_resources"),
                    BuilderDimension::Capabilities,
                    "capabilities.skills",
                    serde_json::Value::Array(skills),
                    PendingSignalAssessment::new(
                        0.95,
                        "用户按能力标签明确列出；熟练度保持未量化",
                        RiskLevel::Medium,
                    ),
                ));
            }

            let resources = Self::labeled_items(capability_answer, "资源", 8)
                .into_iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "type": "unknown",
                        "description": "用户按资源标签明确列出",
                        "availability": "unknown",
                    })
                })
                .collect::<Vec<_>>();
            if !resources.is_empty() {
                signals.push(Self::pending_signal(
                    "socratic_resources",
                    PendingSignalSource::new(7, "capabilities_and_resources"),
                    BuilderDimension::Capabilities,
                    "capabilities.resources",
                    serde_json::Value::Array(resources),
                    PendingSignalAssessment::new(
                        0.95,
                        "用户按资源标签明确列出；类型与可用性保持 unknown",
                        RiskLevel::Medium,
                    ),
                ));
            }

            let networks = Self::labeled_items(capability_answer, "支持网络", 8);
            if !networks.is_empty() {
                signals.push(Self::pending_signal(
                    "socratic_support_networks",
                    PendingSignalSource::new(7, "capabilities_and_resources"),
                    BuilderDimension::Capabilities,
                    "capabilities.networks",
                    serde_json::json!(networks),
                    PendingSignalAssessment::new(
                        0.95,
                        "用户按支持网络标签明确列出",
                        RiskLevel::Medium,
                    ),
                ));
            }
        }

        if let Some(open_question) = session_four
            .get(&8)
            .map(String::as_str)
            .map(str::trim)
            .filter(|answer| answer.ends_with('?') || answer.ends_with('？'))
        {
            signals.push(Self::pending_signal(
                "socratic_capability_open_question",
                PendingSignalSource::new(8, "capability_gap"),
                BuilderDimension::Capabilities,
                "state.open_questions",
                serde_json::json!([open_question]),
                PendingSignalAssessment::new(
                    0.95,
                    "用户按提示写下的能力或支持 open question",
                    RiskLevel::Medium,
                ),
            ));
        }

        signals
    }
    /// Extract signals from quick build answers with risk classification
    fn extract_quick_build_signals(
        session: &BuilderSession,
        _model: &LifeModel,
    ) -> Vec<BuilderSignal> {
        use std::collections::HashMap;

        let mut signals = vec![];
        let mut answers = Self::answer_blocks(&session.draft_yaml, "quick")
            .into_iter()
            .collect::<HashMap<_, _>>();

        // Resume compatibility for Quick sessions persisted before typed
        // answer records existed.
        if answers.is_empty() {
            let mut current_step: Option<usize> = None;
            for line in session.draft_yaml.lines() {
                if let Some(rest) = line.strip_prefix("# step ") {
                    if let Ok(step) = rest.parse::<usize>() {
                        current_step = Some(step);
                    }
                } else if let Some(step) = current_step {
                    answers.entry(step).or_default().push_str(line);
                    answers.entry(step).or_default().push('\n');
                }
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

        // Steps 3-5 preserve explicit names/descriptions. The LifeModel u8
        // convention reserves 0 for an unquantified value, so Builder must not
        // turn an unanswered scale into a midpoint such as 5.
        if let Some(ans) = answers.get(&3) {
            let goal_items = Self::list_items(ans, 4)
                .into_iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "priority": 0,
                        "status": "pending",
                        "milestones": [],
                        "description": "",
                        "progress": 0.0
                    })
                })
                .collect::<Vec<_>>();
            if !goal_items.is_empty() {
                signals.push(create_signal(
                    "sig_short_term",
                    3,
                    "goals",
                    "goals.short_term",
                    serde_json::Value::Array(goal_items),
                    0.8,
                    "用户描述的近期目标；priority=0 表示未量化",
                    RiskLevel::Medium,
                ));
            }
        }

        if let Some(ans) = answers.get(&4) {
            let direction = ans.trim();
            if !direction.is_empty() {
                signals.push(create_signal(
                    "sig_long_term",
                    4,
                    "goals",
                    "goals.long_term",
                    serde_json::json!([{
                        "name": format!("长期方向: {}", Self::bounded_text(direction, 30)),
                        "priority": 0,
                        "status": "pending",
                        "milestones": [],
                        "description": direction,
                        "progress": 0.0
                    }]),
                    0.6,
                    "用户描述的长期方向；priority=0 表示未量化",
                    RiskLevel::High,
                ));
            }
        }

        if let Some(ans) = answers.get(&5) {
            let skill_items = Self::list_items(ans, 5)
                .into_iter()
                .map(|description| {
                    serde_json::json!({
                        "name": Self::bounded_text(&description, 20),
                        "proficiency": 0,
                        "description": description
                    })
                })
                .collect::<Vec<_>>();
            if !skill_items.is_empty() {
                signals.push(create_signal(
                    "sig_skills",
                    5,
                    "capabilities",
                    "capabilities.skills",
                    serde_json::Value::Array(skill_items),
                    0.75,
                    "用户自报的能力；proficiency=0 表示未量化",
                    RiskLevel::Medium,
                ));
            }
        }

        // Step 6: preserve the explicit blocker as a reviewed open question.
        // State alerts are a derived product projection from canonical
        // StateStore history and are not a Builder-owned LifeModel write path.
        // Free-text keyword matching is not mood classification: negation and
        // context cannot be recovered safely with substring checks.
        if let Some(ans) = answers.get(&6) {
            let blockers = ans.trim().to_string();
            if !blockers.is_empty() {
                signals.push(create_signal(
                    "sig_blocker",
                    6,
                    "state",
                    "state.open_questions",
                    serde_json::Value::Array(vec![serde_json::Value::String(format!(
                        "当前卡点: {}",
                        blockers.chars().take(50).collect::<String>()
                    ))]),
                    0.65,
                    "用户主动报告的阻碍",
                    RiskLevel::Medium,
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
            match crate::life_model_write_gateway::life_model_field_authority(&signal.affected_path)
            {
                crate::life_model_write_gateway::LifeModelFieldAuthority::CanonicalLifeModel => {}
                crate::life_model_write_gateway::LifeModelFieldAuthority::StateStoreCanonical => {
                    Self::skip_field(
                        &mut skipped,
                        signal,
                        "field is StateStore canonical and cannot be materialized by Builder",
                        "StateGateway-owned transient state",
                    );
                    continue;
                }
                crate::life_model_write_gateway::LifeModelFieldAuthority::DerivedProjection => {
                    Self::skip_field(
                        &mut skipped,
                        signal,
                        "field is a derived projection and cannot be persisted by Builder",
                        "backend-derived product projection",
                    );
                    continue;
                }
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
                                    name: Self::parse_nonempty_string(v.get("name")?)?,
                                    weight: Self::parse_scale_u8(v.get("weight")?)?,
                                    description: v.get("description")?.as_str()?.to_string(),
                                })
                            })
                            .collect();
                        if items.is_empty() || items.len() != arr.len() {
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
                                    trait_name: Self::parse_nonempty_string(v.get("trait_name")?)?,
                                    score: Self::parse_scale_u8(v.get("score")?)?,
                                })
                            })
                            .collect();
                        if items.is_empty() || items.len() != arr.len() {
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
                        let items: Vec<String> =
                            arr.iter().filter_map(Self::parse_nonempty_string).collect();
                        if items.is_empty() || items.len() != arr.len() {
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
                        let items: Vec<String> =
                            arr.iter().filter_map(Self::parse_nonempty_string).collect();
                        if !items.is_empty() && items.len() == arr.len() {
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
                        let items: Vec<String> =
                            arr.iter().filter_map(Self::parse_nonempty_string).collect();
                        if !items.is_empty() && items.len() == arr.len() {
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
                        let items: Vec<String> =
                            arr.iter().filter_map(Self::parse_nonempty_string).collect();
                        if !items.is_empty() && items.len() == arr.len() {
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
                        if !items.is_empty() && items.len() == arr.len() {
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
                        if !items.is_empty() && items.len() == arr.len() {
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
                        if !items.is_empty() && items.len() == arr.len() {
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
                        if !items.is_empty() && items.len() == arr.len() {
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
                                    name: Self::parse_nonempty_string(v.get("name")?)?,
                                    proficiency: Self::parse_scale_u8(v.get("proficiency")?)?,
                                    description: v.get("description")?.as_str()?.to_string(),
                                })
                            })
                            .collect();
                        if !items.is_empty() && items.len() == arr.len() {
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
                                    name: Self::parse_nonempty_string(v.get("name")?)?,
                                    resource_type: v.get("type")?.as_str()?.to_string(),
                                    description: v.get("description")?.as_str()?.to_string(),
                                    availability: v.get("availability")?.as_str()?.to_string(),
                                })
                            })
                            .collect();
                        if !items.is_empty() && items.len() == arr.len() {
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
                        let items: Vec<String> =
                            arr.iter().filter_map(Self::parse_nonempty_string).collect();
                        if !items.is_empty() && items.len() == arr.len() {
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
                                    name: Self::parse_nonempty_string(v.get("name")?)?,
                                    proficiency: Self::parse_scale_u8(v.get("proficiency")?)?,
                                    description: v.get("description")?.as_str()?.to_string(),
                                })
                            })
                            .collect();
                        if !items.is_empty() && items.len() == arr.len() {
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
                                    domain: Self::parse_nonempty_string(v.get("domain")?)?,
                                    level: Self::parse_scale_u8(v.get("level")?)?,
                                    description: v
                                        .get("description")?
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect();
                        if !items.is_empty() && items.len() == arr.len() {
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
                    if let Some(val) = Self::parse_scale_u8(&signal.proposed_value) {
                        model.state.health_status.energy_level = val;
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
                    if let Some(val) = Self::parse_scale_u8(&signal.proposed_value) {
                        model.state.emotional_state.stress_level = val;
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
                    if let Some(val) = Self::parse_scale_u8(&signal.proposed_value) {
                        model.state.emotional_state.fulfillment_score = val;
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
                        let items: Vec<String> =
                            arr.iter().filter_map(Self::parse_nonempty_string).collect();
                        if !items.is_empty() && items.len() == arr.len() {
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
                        let items: Vec<String> =
                            arr.iter().filter_map(Self::parse_nonempty_string).collect();
                        if !items.is_empty() && items.len() == arr.len() {
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

    fn merge_strings(target: &mut Vec<String>, items: Vec<String>) {
        for item in items {
            if !item.trim().is_empty() && !target.iter().any(|existing| existing == &item) {
                target.push(item);
            }
        }
    }

    fn parse_scale_u8(value: &serde_json::Value) -> Option<u8> {
        let value = u8::try_from(value.as_u64()?).ok()?;
        (value <= 10).then_some(value)
    }

    fn parse_nonempty_string(value: &serde_json::Value) -> Option<String> {
        let value = value.as_str()?.trim();
        (!value.is_empty()).then(|| value.to_string())
    }

    fn parse_goal_item(v: &serde_json::Value) -> Option<crate::life_model::GoalItem> {
        let name = Self::parse_nonempty_string(v.get("name")?)?;
        let milestones = match v.get("milestones") {
            Some(value) => value
                .as_array()?
                .iter()
                .map(|milestone| {
                    let name = Self::parse_nonempty_string(milestone.get("name")?)?;
                    let achieved = match milestone.get("achieved") {
                        Some(value) => value.as_bool()?,
                        None => {
                            milestone.get("status").and_then(|value| value.as_str())
                                == Some("completed")
                        }
                    };
                    let date = match milestone
                        .get("date")
                        .or_else(|| milestone.get("target_date"))
                    {
                        Some(value) if value.is_null() => None,
                        Some(value) => Some(Self::parse_nonempty_string(value)?),
                        None => None,
                    };
                    Some(crate::life_model::Milestone {
                        name,
                        achieved,
                        date,
                    })
                })
                .collect::<Option<Vec<_>>>()?,
            None => Vec::new(),
        };
        let related_memories = match v.get("related_memories") {
            Some(value) => value
                .as_array()?
                .iter()
                .map(Self::parse_nonempty_string)
                .collect::<Option<Vec<_>>>()?,
            None => Vec::new(),
        };
        let priority = v
            .get("priority")
            .map(Self::parse_scale_u8)
            .unwrap_or(Some(0))?;
        let progress = v
            .get("progress")
            .map(|value| value.as_f64())
            .unwrap_or(Some(0.0))?;
        if !(0.0..=1.0).contains(&progress) {
            return None;
        }
        let deadline = match v.get("deadline") {
            Some(value) if value.is_null() => None,
            Some(value) => Some(Self::parse_nonempty_string(value)?),
            None => None,
        };

        Some(crate::life_model::GoalItem {
            name,
            description: match v.get("description") {
                Some(value) => value.as_str()?.to_string(),
                None => String::new(),
            },
            // LifeModel u8=0 is the explicit unquantified value. Builder must
            // never convert an omitted priority into a fabricated midpoint.
            priority,
            status: match v.get("status") {
                Some(value) => value.as_str()?.to_string(),
                None => String::new(),
            },
            progress: progress as f32,
            deadline,
            milestones,
            related_memories,
            updated_at: None,
        })
    }
    fn merge_value_items(
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

    fn merge_goal_items(
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

    fn merge_milestones(
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

    fn merge_skills(
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

    fn merge_resources(
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

    fn merge_tools(
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

    fn merge_knowledge_domains(
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

    fn skip_field(
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

    fn detect_gaps(model: &LifeModel) -> Vec<String> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life_model::{GoalItem, LifeModel, Skill};

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
        let (_, _, prompt) =
            BuilderEngine::socratic_prompt_for_step(3, &session, &LifeModel::default_model())
                .expect("step 3 Socratic prompt");
        assert!(prompt.contains("成长"));
        assert!(prompt.contains("创造"));
        assert!(prompt.contains("角色名称"));
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
    fn apply_signals_rejects_state_store_owned_and_derived_paths() {
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
        assert!(model.goals.daily.is_empty());
        assert!(model.state.alerts.is_empty());
        assert!(applied.is_empty());
        assert_eq!(skipped.len(), 2);
        assert!(skipped
            .iter()
            .any(|field| field.path == "goals.daily"
                && field.reason.contains("StateStore canonical")));
        assert!(skipped.iter().any(
            |field| field.path == "state.alerts" && field.reason.contains("derived projection")
        ));
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
    fn apply_signals_state_alert_is_rejected_as_derived_projection() {
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

        let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);
        assert!(applied.is_empty());
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].reason.contains("derived projection"));
        assert!(model.state.alerts.is_empty());
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
    fn apply_signals_missing_goal_priority_stays_unquantified() {
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
        assert_eq!(goal.priority, 0);
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
                id: "sig_blocker".into(),
                source_step: 6,
                source_question_id: "current_blockers".into(),
                dimension: BuilderDimension::State,
                affected_path: "state.open_questions".into(),
                proposed_value: serde_json::json!(["当前卡点: 方向不明确、拖延"]),
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
        assert!(model
            .state
            .open_questions
            .iter()
            .any(|item| item == "当前卡点: 方向不明确、拖延"));
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

    #[test]
    fn builder_engine_does_not_own_an_independent_provider_route() {
        let source = include_str!("engine.rs");
        for forbidden in [
            concat!("PreparedProvider", "Request"),
            concat!("prepare_", "chat_request"),
            concat!("generate_", "prepared"),
            concat!("generate_builder_", "phase"),
        ] {
            assert!(
                !source.contains(forbidden),
                "BuilderEngine must not own provider orchestration: found {forbidden}"
            );
        }
    }

    #[tokio::test]
    async fn quick_completion_returns_only_pending_candidates_and_a_preview() {
        let engine = BuilderEngine::new();
        let mut session = BuilderSession::new("quick-preview", BuilderMode::Quick);
        session.step_index = QUICK_BUILD_STEPS.len();
        session.draft_yaml = concat!(
            "\n# step 1\nAlex",
            "\n# step 2\n产品可靠性",
            "\n# step 3\n完成后端收口",
            "\n# step 4\n让 OpenLife 成为可靠的个人智能系统",
            "\n# step 5\n系统架构\n工程验证",
            "\n# step 6\n旧路线并存带来压力",
            "\n# step 7\n直接高效型"
        )
        .into();
        let canonical = LifeModel::default_model();

        let (_prompt, preview) = engine.next_prompt(&mut session, "", &canonical).await;

        assert!(session.finished);
        assert!(!session.pending_signals.is_empty());
        assert!(session
            .pending_signals
            .iter()
            .all(|signal| signal.user_status == SignalUserStatus::Pending));
        assert_eq!(
            canonical.identity.name,
            LifeModel::default_model().identity.name
        );
        assert_eq!(preview.unwrap().identity.name, "Alex");
    }

    #[test]
    fn every_incremental_dimension_materializes_typed_pending_candidates() {
        let fixtures = [
            (
                BuilderDimension::Identity,
                "incremental.identity",
                vec![
                    (1, "创造\n可靠"),
                    (3, "系统建设者"),
                    (4, "保护休息时间"),
                    (5, "直接沟通"),
                ],
            ),
            (
                BuilderDimension::Goals,
                "incremental.goals",
                vec![
                    (1, "后端收口"),
                    (2, "完成唯一运行时"),
                    (3, "恢复产品可靠性"),
                    (4, "旧路线并存"),
                ],
            ),
            (
                BuilderDimension::Capabilities,
                "incremental.capabilities",
                vec![
                    (1, "系统架构"),
                    (2, "完成多个复杂项目"),
                    (3, "稳定开发时间"),
                    (4, "项目实践"),
                ],
            ),
            (
                BuilderDimension::State,
                "incremental.state",
                vec![
                    (1, "专注但有压力"),
                    (2, "精力 7，压力 6"),
                    (3, "旧路线并存"),
                    (4, "睡眠\n专注度"),
                ],
            ),
        ];

        for (dimension, lane, answers) in fixtures {
            let mut session = BuilderSession::new("incremental-fixture", BuilderMode::Incremental);
            session.target_dimension = Some(dimension);
            for (step, answer) in answers {
                BuilderEngine::append_answer_block(&mut session, lane, step, answer);
            }
            let signals = BuilderEngine::extract_incremental_signals(&session, dimension);
            assert!(!signals.is_empty(), "missing candidates for {dimension:?}");
            assert!(signals.iter().all(|signal| {
                signal.dimension == dimension
                    && signal.user_status == SignalUserStatus::Pending
                    && !signal.affected_path.is_empty()
                    && !signal.proposed_value.is_null()
            }));
            let preview = BuilderEngine::preview_model(&LifeModel::default_model(), &signals);
            assert_ne!(
                serde_json::to_value(preview).unwrap(),
                serde_json::to_value(LifeModel::default_model()).unwrap(),
                "preview must expose the review candidate for {dimension:?}"
            );
        }
    }

    #[tokio::test]
    async fn incremental_goals_turns_normal_answers_into_pending_typed_candidates() {
        let engine = BuilderEngine::new();
        let mut session = BuilderSession::new("incremental-goals", BuilderMode::Incremental);
        session.target_dimension = Some(BuilderDimension::Goals);
        session.step_index = 4;
        BuilderEngine::append_answer_block(
            &mut session,
            "incremental.goals",
            1,
            "做完 OpenLife 后端重构",
        );
        BuilderEngine::append_answer_block(
            &mut session,
            "incremental.goals",
            2,
            "未来 90 天先完成后端收口",
        );
        BuilderEngine::append_answer_block(
            &mut session,
            "incremental.goals",
            3,
            "它能让产品恢复真实可用",
        );

        let (_prompt, preview) = engine
            .next_prompt(
                &mut session,
                "目前卡点是旧路线并存",
                &LifeModel::default_model(),
            )
            .await;

        assert!(session.finished);
        assert!(!session.pending_signals.is_empty());
        assert!(session.pending_signals.iter().all(|signal| {
            signal.dimension == BuilderDimension::Goals
                && signal.user_status == SignalUserStatus::Pending
                && signal.proposed_value.is_array()
        }));
        assert!(preview.is_some());
        assert!(!preview.unwrap().goals.short_term.is_empty());
    }

    #[tokio::test]
    async fn socratic_completion_produces_pending_signals_and_preview_without_silent_fallback() {
        let engine = BuilderEngine::new();
        let mut session = BuilderSession::new("socratic-preview", BuilderMode::Socratic);
        session.step_index = 8;
        session.current_session = 4;
        session.draft_yaml = concat!(
            "\nUser: 我最有活力的时候是在把复杂系统理顺，内在需要是创造和清晰",
            "\nUser: 我想成为系统建设者，为用户创造可靠产品",
            "\nUser: 未来三年让 OpenLife 成为真正可靠的个人智能系统",
            "\nUser: 我已经有架构和工程能力，但需要更稳定的验证机制"
        )
        .into();

        let (_prompt, preview) = engine
            .next_prompt(
                &mut session,
                "我需要更稳定的验证机制",
                &LifeModel::default_model(),
            )
            .await;

        assert!(session.finished);
        assert!(!session.pending_signals.is_empty());
        assert!(session
            .pending_signals
            .iter()
            .all(|signal| signal.user_status == SignalUserStatus::Pending));
        let preview = preview.expect("Socratic completion must return a review-only preview");
        assert!(
            !preview.identity.values.is_empty()
                || !preview.identity.mission_statement.is_empty()
                || !preview.goals.long_term.is_empty()
                || !preview.capabilities.skills.is_empty()
        );
    }

    #[tokio::test]
    async fn socratic_pairwise_state_is_exposed_as_the_actual_next_prompt() {
        let engine = BuilderEngine::new();
        let mut session = BuilderSession::new("socratic-pairwise", BuilderMode::Socratic);
        let model = LifeModel::default_model();

        engine.next_prompt(&mut session, "", &model).await;
        let (follow_up, _) = engine
            .next_prompt(&mut session, "我在解决复杂问题时最投入", &model)
            .await;
        assert!(follow_up.contains("那次峰值体验"));

        let (prompt, preview) = engine
            .next_prompt(&mut session, "这段体验里的内在需要是创造和清晰", &model)
            .await;

        assert!(preview.is_none());
        assert!(session.waiting_pairwise);
        assert!(prompt.contains("A：创造"));
        assert!(prompt.contains("B：清晰"));

        let (role_prompt, _) = engine.next_prompt(&mut session, "A", &model).await;
        assert!(!session.waiting_pairwise);
        assert_eq!(session.step_index, 3);
        assert!(role_prompt.contains("角色"));

        let (_hypothesis, _) = engine.next_prompt(&mut session, "系统建设者", &model).await;
        let role_answers = BuilderEngine::answer_blocks(&session.draft_yaml, "socratic.2");
        assert_eq!(role_answers.get(&3).map(String::as_str), Some("系统建设者"));
    }

    #[tokio::test]
    async fn socratic_confirmation_advances_instead_of_repeating_the_same_card() {
        let engine = BuilderEngine::new();
        let mut session = BuilderSession::new("socratic-confirmation", BuilderMode::Socratic);
        session.step_index = 3;
        session.current_session = 2;
        session.waiting_phase_confirmation = true;
        session.phase_summary = Some("📋 我暂时这样理解你".into());

        let (prompt, preview) = engine
            .next_prompt(&mut session, "确认", &LifeModel::default_model())
            .await;

        assert!(preview.is_none());
        assert!(!session.waiting_phase_confirmation);
        assert!(session.phase_summary.is_none());
        assert!(!prompt.contains("请回复「确认」继续"));
        assert_eq!(session.step_index, 4);
    }

    #[tokio::test]
    async fn socratic_answers_stay_bound_to_the_question_that_was_displayed() {
        let engine = BuilderEngine::new();
        let mut session = BuilderSession::new("socratic-lane-binding", BuilderMode::Socratic);
        let model = LifeModel::default_model();

        let (opening, _) = engine.next_prompt(&mut session, "", &model).await;
        assert!(opening.contains("峰值体验"));

        let (follow_up, _) = engine
            .next_prompt(&mut session, "我在解决复杂系统问题时最投入", &model)
            .await;
        assert!(follow_up.contains("那次峰值体验"));

        let (role_prompt, _) = engine
            .next_prompt(&mut session, "最有力量的是把混乱理清", &model)
            .await;
        assert!(role_prompt.contains("角色"));

        let session_one = BuilderEngine::answer_blocks(&session.draft_yaml, "socratic.1");
        let session_two = BuilderEngine::answer_blocks(&session.draft_yaml, "socratic.2");
        assert_eq!(
            session_one.get(&1).map(String::as_str),
            Some("我在解决复杂系统问题时最投入")
        );
        assert_eq!(
            session_one.get(&2).map(String::as_str),
            Some("最有力量的是把混乱理清")
        );
        assert!(
            session_two.is_empty(),
            "the displayed session-1 follow-up must not be recorded in session 2"
        );
    }

    #[tokio::test]
    async fn socratic_full_flow_transitions_only_after_consuming_displayed_questions() {
        let engine = BuilderEngine::new();
        let mut session = BuilderSession::new("socratic-full-lanes", BuilderMode::Socratic);
        let model = LifeModel::default_model();

        engine.next_prompt(&mut session, "", &model).await;
        engine
            .next_prompt(&mut session, "峰值体验回答", &model)
            .await;
        let (role_prompt, _) = engine
            .next_prompt(&mut session, "力量感来自清晰", &model)
            .await;
        assert!(role_prompt.contains("角色"));

        let (first_card, _) = engine
            .next_prompt(&mut session, "系统建设者与清晰边界", &model)
            .await;
        assert!(first_card.contains("请回复「确认」继续"));
        let (mission_prompt, _) = engine.next_prompt(&mut session, "确认", &model).await;
        assert!(mission_prompt.contains("长期使命"));
        assert!(!mission_prompt.contains("责任和边界"));

        let (goal_prompt, _) = engine
            .next_prompt(&mut session, "为用户创造可靠系统", &model)
            .await;
        assert!(goal_prompt.contains("未来 1 到 3 年"));
        engine
            .next_prompt(&mut session, "让 OpenLife 真正可靠", &model)
            .await;
        let (second_card, _) = engine
            .next_prompt(&mut session, "90 天完成后端收口", &model)
            .await;
        assert!(second_card.contains("请回复「确认」继续"));

        let (capability_prompt, _) = engine.next_prompt(&mut session, "确认", &model).await;
        assert!(capability_prompt.contains("能力、资源和支持网络"));
        let (gap_prompt, _) = engine
            .next_prompt(&mut session, "系统架构与验证能力", &model)
            .await;
        assert!(gap_prompt.contains("一个问题"));
        let (_done, preview) = engine
            .next_prompt(&mut session, "需要更稳定的验证机制", &model)
            .await;

        assert!(session.finished);
        assert!(preview.is_some());
        for (lane, expected_steps) in [
            ("socratic.1", vec![1, 2]),
            ("socratic.2", vec![3, 4]),
            ("socratic.3", vec![5, 6]),
            ("socratic.4", vec![7, 8]),
        ] {
            let answers = BuilderEngine::answer_blocks(&session.draft_yaml, lane);
            assert_eq!(answers.keys().copied().collect::<Vec<_>>(), expected_steps);
        }
    }

    #[test]
    fn unquantified_builder_answers_use_zero_instead_of_invented_midpoints() {
        fn collect_key_values<'a>(
            value: &'a serde_json::Value,
            key: &str,
            found: &mut Vec<&'a serde_json::Value>,
        ) {
            match value {
                serde_json::Value::Object(fields) => {
                    if let Some(value) = fields.get(key) {
                        found.push(value);
                    }
                    for value in fields.values() {
                        collect_key_values(value, key, found);
                    }
                }
                serde_json::Value::Array(values) => {
                    for value in values {
                        collect_key_values(value, key, found);
                    }
                }
                _ => {}
            }
        }

        let mut quick = BuilderSession::new("quick-unquantified", BuilderMode::Quick);
        BuilderEngine::append_answer_block(&mut quick, "quick", 3, "完成后端收口");
        BuilderEngine::append_answer_block(&mut quick, "quick", 4, "打造可靠的个人智能系统");
        BuilderEngine::append_answer_block(&mut quick, "quick", 5, "系统架构\n工程验证");
        let signals =
            BuilderEngine::extract_quick_build_signals(&quick, &LifeModel::default_model());

        for signal in &signals {
            for key in ["weight", "priority", "proficiency"] {
                let mut values = Vec::new();
                collect_key_values(&signal.proposed_value, key, &mut values);
                assert!(
                    values.iter().all(|value| value.as_u64() == Some(0)),
                    "unasked {key} must be 0=unknown in {}",
                    signal.id
                );
            }
        }
        assert!(signals
            .iter()
            .any(|signal| signal.affected_path == "goals.short_term"));
        assert!(signals
            .iter()
            .any(|signal| signal.affected_path == "capabilities.skills"));
    }

    #[test]
    fn unquantified_goal_answers_materialize_with_zero_priority() {
        let mut session = BuilderSession::new("goal-without-priority", BuilderMode::Incremental);
        session.target_dimension = Some(BuilderDimension::Goals);
        for (step, answer) in [
            (1, "完成后端收口"),
            (2, "未来 90 天完成唯一运行时"),
            (3, "为了恢复产品可靠性"),
            (4, "旧路线并存"),
        ] {
            BuilderEngine::append_answer_block(&mut session, "incremental.goals", step, answer);
        }

        let signals = BuilderEngine::extract_incremental_signals(&session, BuilderDimension::Goals);
        assert_eq!(signals.len(), 1);
        let priorities = signals[0].proposed_value.as_array().unwrap();
        assert!(priorities
            .iter()
            .all(|goal| { goal.get("priority").and_then(|value| value.as_u64()) == Some(0) }));
    }

    #[test]
    fn explicit_socratic_value_words_use_zero_for_unquantified_weight() {
        let mut session = BuilderSession::new("socratic-values", BuilderMode::Socratic);
        BuilderEngine::append_answer_block(
            &mut session,
            "socratic.1",
            1,
            "这段体验里的内在需要是创造和清晰",
        );

        BuilderEngine::extract_values_and_setup_pairwise(&mut session);

        assert_eq!(session.extracted_values.len(), 2);
        assert!(session
            .extracted_values
            .iter()
            .all(|value| value.weight == 0));
        assert_eq!(
            session
                .peak_experience
                .as_ref()
                .map(|peak| peak.extracted_values.clone()),
            Some(vec!["创造".to_string(), "清晰".to_string()])
        );
        assert_eq!(session.pending_pairwise.len(), 1);
    }

    #[test]
    fn negated_quick_blocker_does_not_infer_the_negated_mood() {
        let mut session = BuilderSession::new("quick-negation", BuilderMode::Quick);
        BuilderEngine::append_answer_block(
            &mut session,
            "quick",
            6,
            "我不焦虑，只是缺少明确的技术路线",
        );

        let signals =
            BuilderEngine::extract_quick_build_signals(&session, &LifeModel::default_model());

        assert!(signals.iter().any(|signal| {
            signal.id == "sig_blocker"
                && signal.affected_path == "state.open_questions"
                && signal.proposed_value
                    == serde_json::json!(["当前卡点: 我不焦虑，只是缺少明确的技术路线"])
        }));
        assert!(signals
            .iter()
            .all(|signal| { signal.affected_path != "state.emotional_state.current_mood" }));
    }

    #[tokio::test]
    async fn socratic_correction_is_saved_without_advancing_until_explicit_confirmation() {
        let engine = BuilderEngine::new();
        let mut session = BuilderSession::new("socratic-correction", BuilderMode::Socratic);
        let model = LifeModel::default_model();

        engine.next_prompt(&mut session, "", &model).await;
        engine.next_prompt(&mut session, "峰值体验", &model).await;
        engine
            .next_prompt(&mut session, "力量来自清晰", &model)
            .await;
        let (card, _) = engine
            .next_prompt(&mut session, "我的角色不是管理者，而是系统建设者", &model)
            .await;
        assert!(card.contains("确认"));
        assert_eq!(session.step_index, 3);

        let (corrected_card, _) = engine
            .next_prompt(&mut session, "修正：我的核心角色是系统建设者", &model)
            .await;
        assert!(session.waiting_phase_confirmation);
        assert_eq!(session.step_index, 3);
        assert!(corrected_card.contains("系统建设者"));
        let corrections =
            BuilderEngine::answer_blocks(&session.draft_yaml, "socratic.confirmation_correction");
        assert_eq!(
            corrections.get(&3).map(String::as_str),
            Some("修正：我的核心角色是系统建设者")
        );

        let (mission_prompt, _) = engine.next_prompt(&mut session, "确认", &model).await;
        assert!(!session.waiting_phase_confirmation);
        assert_eq!(session.step_index, 4);
        assert!(mission_prompt.contains("长期使命"));
    }

    #[test]
    fn socratic_checkpoint_contains_bounded_answers_from_the_current_phase() {
        let mut session = BuilderSession::new("socratic-checkpoint", BuilderMode::Socratic);
        for (lane, step, answer) in [
            ("socratic.1", 1, "我在把混乱系统理顺时最投入"),
            ("socratic.2", 3, "系统建设者"),
            ("socratic.2", 4, "为用户创造可靠的个人智能系统"),
            ("socratic.3", 5, "未来三年让 OpenLife 稳定可用"),
            ("socratic.3", 6, "90 天完成后端唯一运行时"),
        ] {
            BuilderEngine::append_answer_block(&mut session, lane, step, answer);
        }

        let checkpoint =
            BuilderEngine::generate_socratic_hypothesis(&session, &LifeModel::default_model());
        for expected in [
            "把混乱系统理顺",
            "系统建设者",
            "可靠的个人智能系统",
            "未来三年让 OpenLife 稳定可用",
            "90 天完成后端唯一运行时",
        ] {
            assert!(checkpoint.contains(expected), "checkpoint lost {expected}");
        }
    }

    #[test]
    fn socratic_materializes_explicit_role_capability_and_gap_answers() {
        let mut session = BuilderSession::new("socratic-explicit-fields", BuilderMode::Socratic);
        BuilderEngine::append_answer_block(&mut session, "socratic.2", 3, "系统建设者");
        BuilderEngine::append_answer_block(
            &mut session,
            "socratic.4",
            7,
            "能力：系统架构；资源：每周十小时；支持网络：工程伙伴",
        );
        BuilderEngine::append_answer_block(
            &mut session,
            "socratic.4",
            8,
            "我下一步如何建立稳定的验证机制？",
        );

        let signals = BuilderEngine::extract_socratic_signals(&session);
        for path in [
            "identity.role_definition.primary_role",
            "capabilities.skills",
            "capabilities.resources",
            "capabilities.networks",
            "state.open_questions",
        ] {
            assert!(
                signals.iter().any(|signal| signal.affected_path == path),
                "missing {path}"
            );
        }
    }

    #[test]
    fn incremental_prompts_map_every_explicitly_asked_field() {
        let fixtures = [
            (
                BuilderDimension::Identity,
                "incremental.identity",
                2,
                "持续创造可靠价值",
                "identity.life_philosophy",
            ),
            (
                BuilderDimension::Capabilities,
                "incremental.capabilities",
                2,
                "分布式系统\n人机交互",
                "capabilities.knowledge_domains",
            ),
            (
                BuilderDimension::State,
                "incremental.state",
                3,
                "睡眠与恢复",
                "state.current_focus",
            ),
        ];

        for (dimension, lane, step, answer, path) in fixtures {
            let mut session = BuilderSession::new("incremental-explicit", BuilderMode::Incremental);
            session.target_dimension = Some(dimension);
            BuilderEngine::append_answer_block(&mut session, lane, step, answer);
            let signals = BuilderEngine::extract_incremental_signals(&session, dimension);
            assert!(
                signals.iter().any(|signal| signal.affected_path == path),
                "explicit answer was dropped: {path}"
            );
        }
    }

    #[tokio::test]
    async fn quick_goal_prompt_does_not_promise_unimplemented_goal_refinement() {
        let engine = BuilderEngine::new();
        let mut session = BuilderSession::new("quick-prompt-truth", BuilderMode::Quick);
        session.step_index = 2;

        let (prompt, _) = engine
            .next_prompt(&mut session, "", &LifeModel::default_model())
            .await;

        assert!(!prompt.contains("帮你转成具体"));
        assert!(prompt.contains("不自动补写"));
    }

    #[test]
    fn state_scores_are_parsed_by_label_not_by_first_two_integers() {
        let mut session = BuilderSession::new("state-score-labels", BuilderMode::Incremental);
        session.target_dimension = Some(BuilderDimension::State);
        BuilderEngine::append_answer_block(
            &mut session,
            "incremental.state",
            2,
            "精力 7/10，压力 8/10",
        );

        let signals = BuilderEngine::extract_incremental_signals(&session, BuilderDimension::State);
        let value = |id: &str| {
            signals
                .iter()
                .find(|signal| signal.id == id)
                .and_then(|signal| signal.proposed_value.as_u64())
        };
        assert_eq!(value("incremental_state_energy"), Some(7));
        assert_eq!(value("incremental_state_stress"), Some(8));
    }

    #[test]
    fn malformed_typed_array_member_rejects_the_whole_signal_without_partial_mutation() {
        let mut model = LifeModel::default_model();
        model.capabilities.skills.clear();
        let signals = vec![BuilderSignal {
            id: "mixed-skills".into(),
            source_step: 1,
            source_question_id: "skills".into(),
            dimension: BuilderDimension::Capabilities,
            affected_path: "capabilities.skills".into(),
            proposed_value: serde_json::json!([
                {"name": "Rust", "proficiency": 8, "description": "系统编程"},
                {"name": "坏数据", "proficiency": 999, "description": "不得 wrap"}
            ]),
            confidence: 1.0,
            reason: "validation counterexample".into(),
            risk_level: RiskLevel::Medium,
            user_status: SignalUserStatus::Accepted,
        }];

        let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);

        assert!(applied.is_empty());
        assert_eq!(skipped.len(), 1);
        assert!(model.capabilities.skills.is_empty());
    }

    #[test]
    fn oversized_scalar_score_is_rejected_instead_of_wrapping_to_u8() {
        let mut model = LifeModel::default_model();
        let before = model.state.health_status.energy_level;
        let signals = vec![BuilderSignal {
            id: "oversized-energy".into(),
            source_step: 2,
            source_question_id: "energy".into(),
            dimension: BuilderDimension::State,
            affected_path: "state.health_status.energy_level".into(),
            proposed_value: serde_json::json!(300),
            confidence: 1.0,
            reason: "validation counterexample".into(),
            risk_level: RiskLevel::Medium,
            user_status: SignalUserStatus::Accepted,
        }];

        let (applied, skipped) = BuilderEngine::apply_signals_to_model(&mut model, &signals);

        assert!(applied.is_empty());
        assert_eq!(skipped.len(), 1);
        assert_eq!(model.state.health_status.energy_level, before);
    }
}
