use crate::builder::types::*;
use crate::life_model::{
    EmotionalState, GoalItem, HealthStatus, LifeModel, PersonalityTrait, Resource, Skill, ValueItem,
};
use crate::llm::ChatMessage;
use crate::scheduler::InferenceScheduler;

pub struct BuilderEngine<'a> {
    scheduler: &'a InferenceScheduler,
}

impl<'a> BuilderEngine<'a> {
    pub fn new(scheduler: &'a InferenceScheduler) -> Self {
        Self { scheduler }
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

    async fn quick_build_step(
        &self,
        session: &mut BuilderSession,
        user_reply: &str,
        current_model: &LifeModel,
    ) -> (String, Option<LifeModel>) {
        if !user_reply.is_empty() {
            session
                .draft_yaml
                .push_str(&format!("\n# step {}\n{}", session.step_index, user_reply));
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
                    "【第 3 步/7：近期目标】\n\n接下来 1-3 个月，你最希望推进哪 1-3 件事？\n\n例如：\n• 找到更稳定的工作节奏\n• 做完一个产品 MVP\n• 恢复运动习惯\n• 减少焦虑和拖延\n\n如果写的是模糊愿望（比如「我想状态好一点」），我会帮你转成具体的目标草稿。".to_string()
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
            // Step 7 completed, now generate model and signals for review
            let model = self
                .draft_to_life_model(&session.draft_yaml, current_model)
                .await;
            let signals = Self::extract_quick_build_signals(session, &model);
            session.pending_signals = signals;
            session.finished = true;
            (
                "快速构建问题已完成！接下来请审阅 AI 生成的模型建议。".to_string(),
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
        if !user_reply.is_empty() {
            session
                .draft_yaml
                .push_str(&format!("\nUser: {}", user_reply));
        }

        match session.target_dimension {
            Some(BuilderDimension::Identity) => {
                const IDENTITY_STEPS: &[&str] = &[
                    "values",
                    "peak_experience",
                    "roles",
                    "boundaries",
                    "communication",
                ];
                let total = IDENTITY_STEPS.len();
                if session.step_index < total {
                    let prompt = match IDENTITY_STEPS[session.step_index] {
                        "values" => "【Identity 问题 1/5：核心价值观】\n\n最近一年里，有哪些事情会让你觉得\"这对我很重要，我不想妥协\"？\n\n可以是自由、成长、家人、健康、创造、稳定、影响力，也可以是你自己的说法。\n\n你的回答会帮助我识别你最底层的驱动力。".to_string(),
                        "peak_experience" => "【Identity 问题 2/5：峰值体验】\n\n有没有一个时刻，让你觉得\"那才像真正的我\"？\n当时你在做什么？为什么那个时刻重要？\n\n这个回答会帮我理解你内心深处最认同的自己是什么样的。".to_string(),
                        "roles" => "【Identity 问题 3/5：身份角色】\n\n你现在最重要的几个身份角色是什么？\n比如：创业者、学生、创作者、伴侣、家庭成员、探索者、管理者。\n\n不用排优先级，先把想到的列出来。".to_string(),
                        "boundaries" => "【Identity 问题 4/5：边界保护】\n\n有哪些事情你不希望 OpenLife 推着你去做？\n或者有哪些生活边界你想保护？\n\n比如：不希望周末被提醒工作、不想被push社交、需要保护自己的休息时间等。".to_string(),
                        "communication" => "【Identity 问题 5/5：沟通偏好】\n\n当你状态不好时，你希望 OpenLife 怎么和你说话？\n\n• 温和一点：多鼓励，少压迫\n• 直接一点：少废话，直接给建议\n• 多问问题：帮我自己想清楚\n• 帮我拆步骤：把大目标拆成可执行的小动作\n• 提醒我面对现实：不逃避，直面问题\n• 先共情再建议：先理解情绪，再给结构化建议\n\n选一个最贴近你的，或者描述你自己的偏好。".to_string(),
                        _ => String::new(),
                    };
                    session.step_index += 1;
                    (prompt, None)
                } else {
                    let model = self
                        .draft_to_life_model(&session.draft_yaml, current_model)
                        .await;
                    let signals = Self::extract_signals_for_dimension(
                        session,
                        &model,
                        BuilderDimension::Identity,
                    );
                    session.pending_signals = signals;
                    session.finished = true;
                    ("Identity 维度的问题已回答完毕！接下来请审阅 AI 根据你的回答生成的 Identity 建议。".to_string(), Some(model))
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
                    let model = self
                        .draft_to_life_model(&session.draft_yaml, current_model)
                        .await;
                    let signals = Self::extract_signals_for_dimension(
                        session,
                        &model,
                        BuilderDimension::Goals,
                    );
                    session.pending_signals = signals;
                    session.finished = true;
                    ("Goals 维度的问题已回答完毕！接下来请审阅 AI 根据你的回答生成的 Goals 建议。".to_string(), Some(model))
                }
            }
            Some(BuilderDimension::Capabilities) => {
                const CAP_STEPS: &[&str] = &[
                    "natural_skills",
                    "past_projects",
                    "resources",
                    "learning_style",
                ];
                let total = CAP_STEPS.len();
                if session.step_index < total {
                    let prompt = match CAP_STEPS[session.step_index] {
                        "natural_skills" => "【Capabilities 问题 1/4：自然能力】\n\n哪些事情是你做起来比较自然，或者别人曾经认可过你的？\n\n哪怕是\"小事\"也可以：比如擅长倾听、总能发现别人忽略的细节、能把复杂概念讲清楚。".to_string(),
                        "past_projects" => "【Capabilities 问题 2/4：过往项目】\n\n你过去做过哪些项目、工作、学习或长期投入？\n\n不需要全职工作，side project、自学、志愿活动都算。重点是你在其中积累了什么经验。".to_string(),
                        "resources" => "【Capabilities 问题 3/4：可调用的资源】\n\n你现在有哪些可以调用的资源？\n\n比如：\n• 时间（每天/每周能投入多少？）\n• 设备（电脑、软件、工具）\n• 资金\n• 已完成的作品/项目\n• 平台/渠道\n• 人脉/社群\n• 环境（安静的空间、图书馆等）".to_string(),
                        "learning_style" => "【Capabilities 问题 4/4：学习方式】\n\n当你要补一个能力时，你更适合哪种方式？\n\n• 直接做项目：在实践中学习\n• 看系统课程：结构化学习\n• 读文档/书：自己研究\n• 找人交流：向有经验的人请教\n• 让 AI 陪跑：边问边做\n• 写总结复盘：通过输出倒逼输入\n\n选一个或几个最贴近你的。".to_string(),
                        _ => String::new(),
                    };
                    session.step_index += 1;
                    (prompt, None)
                } else {
                    let model = self
                        .draft_to_life_model(&session.draft_yaml, current_model)
                        .await;
                    let signals = Self::extract_signals_for_dimension(
                        session,
                        &model,
                        BuilderDimension::Capabilities,
                    );
                    session.pending_signals = signals;
                    session.finished = true;
                    ("Capabilities 维度的问题已回答完毕！接下来请审阅 AI 根据你的回答生成的 Capabilities 建议。".to_string(), Some(model))
                }
            }
            Some(BuilderDimension::State) => {
                const STATE_STEPS: &[&str] = &[
                    "current_state",
                    "energy_stress",
                    "source",
                    "habits_tracking",
                ];
                let total = STATE_STEPS.len();
                if session.step_index < total {
                    let prompt = match STATE_STEPS[session.step_index] {
                        "current_state" => "【State 问题 1/4：当前状态】\n\n如果用 3 个词描述你最近的状态，会是什么？\n\n比如：兴奋、焦虑、疲惫、混乱、专注、期待、卡住、平静。\n\n不需要\"正确\"的答案，当下的真实感受就可以。".to_string(),
                        "energy_stress" => "【State 问题 2/4：精力与压力】\n\n最近一周你的精力水平大概是 1-10 分多少？\n压力水平又是多少？\n\n1 = 非常差，10 = 非常好。直觉打分就好。".to_string(),
                        "source" => "【State 问题 3/4：状态来源】\n\n这个状态主要来自哪里？\n\n• 工作/项目压力\n• 身体健康\n• 关系问题\n• 经济压力\n• 目标不清晰\n• 睡眠问题\n• 信息过载\n• 长期拖延带来的焦虑\n\n选一个最主要的，或者描述你自己的情况。".to_string(),
                        "habits_tracking" => "【State 问题 4/4：习惯与追踪】\n\n你现在有哪些想维持、恢复或建立的小习惯？\n\n另外，如果 OpenLife 每天或每周帮你观察一个状态指标，你最想观察什么？\n\n• 专注度\n• 睡眠\n• 运动\n• 情绪稳定度\n• 创作产出\n• 学习投入\n• 社交能量\n• 压力水平".to_string(),
                        _ => String::new(),
                    };
                    session.step_index += 1;
                    (prompt, None)
                } else {
                    let model = self
                        .draft_to_life_model(&session.draft_yaml, current_model)
                        .await;
                    let signals = Self::extract_signals_for_dimension(
                        session,
                        &model,
                        BuilderDimension::State,
                    );
                    session.pending_signals = signals;
                    session.finished = true;
                    ("State 维度的问题已回答完毕！接下来请审阅 AI 根据你的回答生成的 State 建议。".to_string(), Some(model))
                }
            }
            None => ("请先选择一个要构建的维度。".to_string(), None),
        }
    }

    fn extract_signals_for_dimension(
        session: &BuilderSession,
        model: &LifeModel,
        dimension: BuilderDimension,
    ) -> Vec<BuilderSignal> {
        let all_signals = Self::extract_quick_build_signals(session, model);
        all_signals
            .into_iter()
            .filter(|s| s.dimension == dimension)
            .collect()
    }

    async fn socratic_step(
        &self,
        session: &mut BuilderSession,
        user_reply: &str,
        current_model: &LifeModel,
    ) -> (String, Option<LifeModel>) {
        const MAX_TURNS: usize = 8;

        if !user_reply.is_empty() {
            session
                .draft_yaml
                .push_str(&format!("\nUser: {}", user_reply));
        }

        if session.step_index == 0 && user_reply.is_empty() {
            let opening = "欢迎来到 OpenLife 的苏格拉底式构建模式。我们将通过 4 次简短对话，逐步勾勒你的人生模型。\n\n【会话 1/4：价值观与峰值体验】\n请回忆一次让你感到最有活力、最投入的「峰值体验」。当时你在做什么？那种体验里什么最吸引你？".to_string();
            session
                .draft_yaml
                .push_str(&format!("\nAssistant: {}", opening));
            session.step_index = 1;
            session.current_session = 1;
            return (opening, None);
        }

        // Phase confirmation: user has confirmed the hypothesis card
        if session.waiting_phase_confirmation {
            session.waiting_phase_confirmation = false;
            session.phase_summary = None;
            // fall through to normal flow after confirmation
        }

        if session.waiting_pairwise {
            return self.handle_pairwise_input(session, current_model).await;
        }

        // Hypothesis cards at turn 3 and 6
        if session.step_index == 3
            && !session.waiting_phase_confirmation
            && session.phase_summary.is_none()
        {
            let hypothesis = Self::generate_socratic_hypothesis(session, current_model);
            session.waiting_phase_confirmation = true;
            session.phase_summary = Some(hypothesis.clone());
            return (hypothesis, None);
        }

        if session.step_index == 6
            && !session.waiting_phase_confirmation
            && session.phase_summary.is_none()
        {
            let hypothesis = Self::generate_socratic_hypothesis(session, current_model);
            session.waiting_phase_confirmation = true;
            session.phase_summary = Some(hypothesis.clone());
            return (hypothesis, None);
        }

        if session.step_index >= MAX_TURNS {
            session.finished = true;
            let mut model = self
                .draft_to_life_model(&session.draft_yaml, current_model)
                .await;
            Self::patch_socratic_values(session, &mut model);
            Self::patch_peak_experience(session, &mut model);
            return (
                "苏格拉底式对话已完成！我已根据你的回答生成了一份人生模型初稿。".to_string(),
                Some(model),
            );
        }

        let turn_label = format!(
            "S{}-T{}",
            session.current_session.max(1),
            session.step_index
        );
        let reply = Self::build_socratic_scripted_prompt(session, current_model);
        session
            .draft_yaml
            .push_str(&format!("\nAssistant [{}]: {}", turn_label, reply));
        session.step_index += 1;

        // Session transition logic
        if session.current_session == 1 && session.step_index >= 2 {
            self.extract_values_and_setup_pairwise(session).await;
        } else if session.current_session == 1 && !session.waiting_pairwise {
            session.current_session = 2;
        } else if session.current_session == 2 && session.step_index >= 4 {
            session.current_session = 3;
        } else if session.current_session == 3 && session.step_index >= 6 {
            session.current_session = 4;
        }

        (reply, None)
    }

    fn build_socratic_scripted_prompt(
        session: &BuilderSession,
        current_model: &LifeModel,
    ) -> String {
        let session_num = session.current_session.max(1);
        let local_turn = match session_num {
            1 => session.step_index.max(1),
            2 => session.step_index.saturating_sub(2).max(1),
            3 => session.step_index.saturating_sub(4).max(1),
            4 => session.step_index.saturating_sub(6).max(1),
            _ => 1,
        };
        let top_values = if !session.extracted_values.is_empty() {
            session
                .extracted_values
                .iter()
                .map(|value| value.name.clone())
                .collect::<Vec<_>>()
                .join("、")
        } else {
            current_model
                .identity
                .values
                .iter()
                .take(3)
                .map(|value| value.name.clone())
                .collect::<Vec<_>>()
                .join("、")
        };
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

        match (session_num, local_turn) {
            (1, 1) => "在那次峰值体验里，最让你有力量感的瞬间是什么？它满足了你怎样的内在需要？".to_string(),
            (2, 1) => {
                if top_values.is_empty() {
                    "如果把你想成为的人浓缩成一个角色，这个角色最重要的责任和边界是什么？".to_string()
                } else {
                    format!("结合你重视的 {}，如果把自己浓缩成一个角色，这个角色最重要的责任和边界是什么？", top_values)
                }
            }
            (2, _) => "这个角色最想为谁创造什么影响？如果只能留下一个长期使命，你会怎么描述它？".to_string(),
            (3, 1) => {
                if goals_hint.is_empty() {
                    "未来 1 到 3 年，最值得你投入的一个核心目标是什么？为什么它现在重要？".to_string()
                } else {
                    format!("基于你已经提到的方向（{}），如果只选一个最关键目标，它会是什么？为什么现在最重要？", goals_hint)
                }
            }
            (3, _) => "为了让这个目标真正发生，未来 90 天最关键的里程碑是什么？你准备用什么标准判断自己在前进？".to_string(),
            (4, 1) => "要实现这个目标，你最依赖的能力、资源和支持网络分别是什么？哪些已经具备，哪些还不稳定？".to_string(),
            (4, _) => "如果现在只选一个最关键缺口，你最需要补哪项能力、习惯或支持条件？接下来准备怎么补？".to_string(),
            _ => "接下来，哪一部分最值得我们继续深挖：价值观、目标、能力，还是当前状态？".to_string(),
        }
    }

    fn generate_socratic_hypothesis(
        session: &BuilderSession,
        _current_model: &LifeModel,
    ) -> String {
        let mut lines = vec![];
        lines.push("📋 我暂时这样理解你".to_string());
        lines.push("".to_string());
        if !session.extracted_values.is_empty() {
            let names: Vec<String> = session
                .extracted_values
                .iter()
                .map(|v| v.name.clone())
                .collect();
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
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            if let Some((top, _)) = sorted.first() {
                lines.push(format!("价值排序中最优先的是：{}", top));
            }
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
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
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

    fn patch_socratic_values(session: &BuilderSession, model: &mut LifeModel) {
        if session.extracted_values.is_empty() {
            return;
        }
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (_, _, choice) in &session.pairwise_results {
            *counts.entry(choice.clone()).or_insert(0) += 1;
        }
        let mut values: Vec<ValueItem> = session
            .extracted_values
            .iter()
            .map(|v| {
                let count = counts.get(&v.name).copied().unwrap_or(0);
                let weight = ((5 + count * 2) as u8).clamp(1, 10);
                ValueItem {
                    name: v.name.clone(),
                    weight,
                    description: v.description.clone(),
                }
            })
            .collect();
        values.sort_by(|a, b| b.weight.cmp(&a.weight));
        if !values.is_empty() {
            model.identity.values = values;
        }
    }

    /// 将峰值体验中提取的多维信号合并到最终 LifeModel 中。
    fn patch_peak_experience(session: &BuilderSession, model: &mut LifeModel) {
        let peak = match &session.peak_experience {
            Some(p) => p,
            None => return,
        };
        // role_hints -> primary_role / secondary_roles
        if let Some(first_role) = peak.extracted_role_hints.first() {
            if model.identity.role_definition.primary_role.is_empty() {
                model.identity.role_definition.primary_role = first_role.clone();
            }
            for hint in peak.extracted_role_hints.iter().skip(1) {
                if !model
                    .identity
                    .role_definition
                    .secondary_roles
                    .contains(hint)
                {
                    model
                        .identity
                        .role_definition
                        .secondary_roles
                        .push(hint.clone());
                }
            }
        }
        // capability_hints -> skills
        for hint in &peak.extracted_capability_hints {
            let exists = model.capabilities.skills.iter().any(|s| s.name == *hint);
            if !exists {
                model.capabilities.skills.push(Skill {
                    name: hint.clone(),
                    proficiency: 5,
                    description: String::new(),
                });
            }
        }
        // preference_hints -> preferences
        if let Some(pref) = peak.extracted_preference_hints.first() {
            if model.preferences.communication_style.is_empty() {
                model.preferences.communication_style = pref.clone();
            }
        }
        // emotional_signal -> current_mood
        if !peak.emotional_signal.is_empty() && model.state.emotional_state.current_mood.is_empty()
        {
            model.state.emotional_state.current_mood = peak.emotional_signal.clone();
        }
    }

    async fn extract_values_and_setup_pairwise(&self, session: &mut BuilderSession) {
        let prompt = format!(
            r#"请根据用户在峰值体验中的描述，提取多维度信号，输出严格 JSON 对象：
{{
  "values": ["关键词1", "关键词2"],
  "role_hints": ["角色暗示1"],
  "capability_hints": ["能力暗示1"],
  "preference_hints": ["偏好暗示1"],
  "emotional_signal": "情绪关键词"
}}
只输出 JSON，不要解释。

用户描述：
{}"#,
            session.draft_yaml
        );
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: prompt,
        }];
        let mut values = vec![];
        let mut peak = PeakExperience {
            raw_description: session.draft_yaml.clone(),
            ..Default::default()
        };
        if let Ok(reply) = self.scheduler.generate_raw(messages, None).await {
            let cleaned = reply
                .trim()
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim();
            #[derive(serde::Deserialize)]
            struct ExtractedSignals {
                #[serde(default)]
                values: Vec<String>,
                #[serde(default)]
                role_hints: Vec<String>,
                #[serde(default)]
                capability_hints: Vec<String>,
                #[serde(default)]
                preference_hints: Vec<String>,
                #[serde(default)]
                emotional_signal: String,
            }
            if let Ok(signals) = serde_json::from_str::<ExtractedSignals>(cleaned) {
                values = signals
                    .values
                    .into_iter()
                    .map(|name| ValueItem {
                        name,
                        weight: 5,
                        description: String::new(),
                    })
                    .collect();
                peak.extracted_values = values.iter().map(|v| v.name.clone()).collect();
                peak.extracted_role_hints = signals.role_hints;
                peak.extracted_capability_hints = signals.capability_hints;
                peak.extracted_preference_hints = signals.preference_hints;
                peak.emotional_signal = signals.emotional_signal;
            }
        }
        session.extracted_values = values.clone();
        session.peak_experience = Some(peak);
        if values.len() >= 2 {
            let names: Vec<String> = values.iter().map(|v| v.name.clone()).collect();
            session.pending_pairwise = Self::generate_pairwise_pairs(&names);
            session.waiting_pairwise = true;
            session
                .draft_yaml
                .push_str(&format!("\n[System] 提取到的价值观关键词: {:?}\n", names));
        }
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

    async fn handle_pairwise_input(
        &self,
        session: &mut BuilderSession,
        _current_model: &LifeModel,
    ) -> (String, Option<LifeModel>) {
        // Note: user_reply already appended to draft_yaml before entering here
        if let Some((a, b)) = session.pending_pairwise.first().cloned() {
            // Look at the last user line to determine choice
            let last_line = session.draft_yaml.lines().last().unwrap_or("");
            let choice = last_line.strip_prefix("User: ").unwrap_or(last_line).trim();
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
            let prompt = format!(
                "基于你的峰值体验，我提炼出一些价值关键词。让我们做个两两比较，看看哪些对你更重要。\n\nA：{}\nB：{}\n\n请回复 A 或 B（也可以直接描述你的选择）。",
                a, b
            );
            return (prompt, None);
        }

        // Pairwise done
        session.waiting_pairwise = false;
        session.current_session = 2;
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

        let prompt = format!(
            "【会话 2/4：角色与使命】\n{}\n\n基于你重视的价值观，如果让你用一个角色来形容自己（比如「桥梁建造者」、「探索者」、「守护者」），你会选什么？这个角色的核心使命是什么？",
            explanation
        );
        session
            .draft_yaml
            .push_str(&format!("\nAssistant: {}", prompt));
        session.step_index += 1;
        (prompt, None)
    }

    /// Extract signals from quick build answers with risk classification
    fn extract_quick_build_signals(
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

    fn merge_strings(target: &mut Vec<String>, items: Vec<String>) {
        for item in items {
            if !item.trim().is_empty() && !target.iter().any(|existing| existing == &item) {
                target.push(item);
            }
        }
    }

    fn parse_goal_item(v: &serde_json::Value) -> Option<crate::life_model::GoalItem> {
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

    async fn draft_to_life_model(&self, draft: &str, base: &LifeModel) -> LifeModel {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life_model::LifeModel;

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
}
