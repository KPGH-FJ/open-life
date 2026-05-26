use crate::agent::{prompt_stack::PromptStack, PrivacyPolicy};
use crate::builder::types::*;
use crate::life_model::{LifeModel, Skill, ValueItem};

impl<'a> super::BuilderEngine<'a> {
    pub(crate) async fn quick_build_step(
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

    pub(crate) async fn incremental_prompt(
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

    pub(crate) fn extract_signals_for_dimension(
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

    pub(crate) async fn socratic_step(
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

    pub(crate) fn build_socratic_scripted_prompt(
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

    pub(crate) fn generate_socratic_hypothesis(
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

    pub(crate) fn generate_pairwise_explanation(session: &BuilderSession) -> String {
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

    pub(crate) fn patch_socratic_values(session: &BuilderSession, model: &mut LifeModel) {
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
    pub(crate) fn patch_peak_experience(session: &BuilderSession, model: &mut LifeModel) {
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

    pub(crate) async fn extract_values_and_setup_pairwise(&self, session: &mut BuilderSession) {
        let mut values = vec![];
        let mut peak = PeakExperience {
            raw_description: session.draft_yaml.clone(),
            ..Default::default()
        };
        let stack = PromptStack::builder_signal_extraction_stack(
            &session.draft_yaml,
            PrivacyPolicy::LocalOnly,
        );
        if let Ok(mut stack) = stack {
            let prompt = stack.assemble();
            if let Ok(reply) = self
                .scheduler
                .generate_raw_governed(vec![], Some(&prompt), PrivacyPolicy::LocalOnly)
                .await
            {
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

    pub(crate) fn generate_pairwise_pairs(names: &[String]) -> Vec<(String, String)> {
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

    pub(crate) async fn handle_pairwise_input(
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
}
