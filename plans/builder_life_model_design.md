# OpenLife Builder 人生模型构建设计方案

> Scoped reference. Use for Builder UX/domain context only. Current LifeModel-HS
> governance, proposal-first rules, and development order are defined by
> `plans/README.md`, `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`,
> and `plans/openlife_lifemodel_governed_agent_runtime.md`.

> 目标：设计 OpenLife 初次使用时的人生模型构建体验，让用户通过「快速构建 / 渐进构建 / 苏格拉底式对话」三种方式建立 Identity / Goals / Capabilities / State 四维人生模型。

---

## 1. 设计定位

OpenLife 的 Builder 不是问卷，也不是心理测试，而是 OpenLife 第一次认真认识用户的入口。

人生模型构建需要完成三件事：

- 让用户知道每一步在做什么。
- 让 AI 生成的内容可解释、可编辑、可拒绝。
- 让构建出来的人生模型立刻支撑 Chat / Dashboard / Calibration。

核心产品表达建议：

```text
让 OpenLife 先认识你
```

辅助说明：

```text
OpenLife 会基于你的身份、目标、能力和当前状态，构建一个初始人生模型。
这个模型不是固定标签，而是一个可以持续修改、校准和进化的“当前版本的你”。
```

---

## 2. 三种构建方式

| 构建方式 | 用户心理 | 时间 | 输出 | 推荐场景 |
| --- | --- | ---: | --- | --- |
| 快速构建 | 我想先试用，不想填太多 | 3-5 分钟 | 可用但较粗的初始人生模型 | 首次打开、低耐心用户 |
| 渐进构建 | 我愿意认真补全，但不要一次太累 | 10-30 分钟，可中断 | 四维逐步完善的人生模型 | 模型补全、长期使用 |
| 苏格拉底式对话 | 我想更深地理解自己 | 20-60 分钟，可多轮 | 高质量人生叙事、价值观、冲突与目标 | 迷茫、转型、深度探索 |

推荐逻辑：

- 首次打开 OpenLife：默认推荐「快速构建」。
- 已有初始模型但完整度低：推荐「渐进构建」。
- 用户主动表达迷茫、冲突、不知道自己想要什么：推荐「苏格拉底式对话」。

---

## 3. Builder 入口页

页面标题：

```text
让 OpenLife 先认识你
```

页面说明：

```text
你可以选择一种方式开始构建人生模型。
不用担心一次说清楚所有事情，OpenLife 会在之后的对话、校准和反馈中逐渐理解你。
```

### 3.1 快速构建卡片

```text
快速构建
3-5 分钟

回答几个关键问题，生成一个可以立即使用的初始人生模型。

适合你，如果：
- 你想先快速体验 OpenLife
- 你不想一开始就回答太多深度问题
- 你愿意之后再慢慢完善

输出：
- 初始身份画像
- 近期目标
- 能力与资源摘要
- 当前状态摘要
```

按钮：

```text
开始快速构建
```

### 3.2 渐进构建卡片

```text
渐进构建
10-30 分钟，可随时暂停

按 Identity / Goals / Capabilities / State 四个维度逐步完善。
你可以今天只完成一部分，下次继续。

适合你，如果：
- 你希望 OpenLife 更准确地理解你
- 你愿意认真整理自己的目标和状态
- 你想看到人生模型完整度逐步提高

输出：
- 四维人生模型
- 每个维度的完成度
- 待确认建议
- 下一步完善建议
```

按钮：

```text
开始渐进构建
```

### 3.3 苏格拉底式对话卡片

```text
苏格拉底式对话
20-60 分钟，可多次进行

OpenLife 会通过追问帮你澄清价值、目标、冲突和当前人生阶段。
它不会直接替你下结论，每个重要理解都会等你确认。

适合你，如果：
- 你最近比较迷茫
- 你正在做人生选择
- 你有目标冲突
- 你想更深地理解自己

输出：
- 阶段性人生叙事
- 深层价值观
- 目标冲突分析
- 可执行的小实验目标
```

按钮：

```text
开始深度对话
```

---

## 4. 快速构建方案

### 4.1 产品目标

快速构建的目标是让用户在 3-5 分钟内生成一个能被系统使用的人生模型。

它不追求深度准确，只追求：

- LifeModel 不为空。
- Chat 可以调用模型上下文。
- Dashboard 有内容。
- 用户知道之后还能继续完善。

### 4.2 用户流程

```text
选择快速构建
  ↓
回答 7 个核心问题
  ↓
AI 生成人生模型草稿
  ↓
用户确认 / 编辑 / 跳过部分字段
  ↓
保存 LifeModel
  ↓
创建 initial:quick-builder 快照
  ↓
跳转 Chat / Dashboard / 渐进构建
```

### 4.3 快速构建问题脚本

#### Step 1：称呼

问题：

```text
我应该怎么称呼你？
可以是真名、昵称，或者你希望 OpenLife 使用的称呼。
```

输入提示：

```text
例如：小林、Alex、老傅，或者“先叫我用户也行”。
```

写入字段：

```text
identity.name
```

#### Step 2：当前人生主题

问题：

```text
你现在最关注的人生主题是什么？
```

选项：

```text
事业 / 学业
健康 / 精力
情绪 / 状态
关系 / 家庭
财富 / 资源
创造 / 表达
自我探索
暂时说不清
```

引导提示：

```text
不用选“最正确”的答案。选你最近最常想起、最消耗注意力的那个方向就好。
```

写入字段：

```text
state.current_focus
state.focus_areas
```

#### Step 3：近期目标

问题：

```text
接下来 1-3 个月，你最希望推进哪 1-3 件事？
```

输入提示：

```text
例如：
- 找到更稳定的工作节奏
- 做完一个产品 MVP
- 恢复运动习惯
- 减少焦虑和拖延
```

写入字段：

```text
goals.short_term
goals.daily 可选
```

AI 提取规则：

```text
如果用户写的是模糊愿望，需要转成目标草稿。

例如：
“我想状态好一点”

转成：
目标：恢复稳定精力和生活节奏
```

#### Step 4：长期方向

问题：

```text
如果把时间拉长一点，你希望未来 1-3 年的自己变成什么样？
```

引导提示：

```text
不需要宏大。你可以从生活状态、工作方式、关系、健康、创造力里任选一个角度说。
```

写入字段：

```text
goals.long_term
identity.mission_statement
identity.life_philosophy
```

高风险说明：

```text
这里涉及长期方向，OpenLife 只会生成建议，不会直接写入，稍后需要你确认。
```

#### Step 5：已有能力

问题：

```text
你觉得自己目前有哪些能力、经验或资源？
哪怕它们还没有被充分发挥，也可以写下来。
```

输入提示：

```text
例如：
- 我擅长分析复杂问题
- 我做过产品/写作/编程/销售
- 我有一些行业经验
- 我有一台电脑、固定时间、朋友支持
```

写入字段：

```text
capabilities.skills
capabilities.resources
capabilities.knowledge_domains
```

#### Step 6：当前卡点

问题：

```text
现在最阻碍你前进的是什么？
```

选项辅助：

```text
时间不够
精力不足
拖延
方向不清晰
能力不够
情绪压力
外部环境限制
缺少支持
```

引导提示：

```text
可以很具体，也可以很模糊。比如“我不知道为什么就是动不起来”也可以。
```

写入字段：

```text
state.emotional_state
state.alerts
capabilities gap
goals risk note
```

#### Step 7：陪伴风格

问题：

```text
你希望 OpenLife 用什么方式陪你？
```

选项：

```text
温和支持型：多鼓励，少压迫
直接高效型：少废话，直接给建议
苏格拉底追问型：多问问题，帮我自己想清楚
教练督促型：提醒我行动，不让我逃避
朋友聊天型：自然一点，像朋友一样陪伴
理性分析型：结构化、逻辑化、客观一点
```

写入字段：

```text
identity.voice_style
preferences.communication_style
```

### 4.4 快速构建完成页

标题：

```text
这是 OpenLife 对你的初步理解
```

展示结构：

```text
身份主线
你目前像是处在一个关注「事业推进与状态恢复」的阶段。
你希望自己逐渐成为一个更稳定、更能持续产出的人。

核心价值草稿
- 成长
- 自主
- 稳定

近期目标
- 完成当前项目 MVP
- 恢复规律作息
- 降低拖延带来的压力

已有能力
- 产品思考
- 编程基础
- 长期自学能力

当前状态
你现在的主要卡点可能是精力波动和目标过多。
OpenLife 会先帮助你把目标拆小、建立节奏。

不确定的地方
- 你的长期使命还比较模糊
- 你的能力水平需要后续进一步校准
- 当前压力来源还需要更多对话确认
```

确认按钮：

```text
保存为初始人生模型
```

辅助按钮：

```text
我想修改
先不保存
进入渐进构建继续完善
```

### 4.5 快速构建 AI 输出格式

```ts
interface QuickBuildResult {
  life_model_patch: Partial<LifeModel>;
  summary: {
    identity_summary: string;
    goals_summary: string;
    capabilities_summary: string;
    state_summary: string;
  };
  assumptions: string[];
  uncertain_fields: string[];
  confidence_by_dimension: {
    identity: number;
    goals: number;
    capabilities: number;
    state: number;
  };
  suggested_next_steps: string[];
}
```

### 4.6 快速构建风险规则

默认可勾选：

```text
state.current_focus
goals.short_term
capabilities.skills
preferences.communication_style
```

必须手动确认：

```text
identity.values
identity.mission_statement
goals.long_term
goals.life_goals
```

---

## 5. 渐进构建方案

### 5.1 产品目标

渐进构建的目标是让用户分模块、分阶段完善人生模型，而不是一次性填完所有内容。

它应该像一个可以随时继续的自我建模工作台。

### 5.2 用户流程

```text
选择渐进构建
  ↓
看到四维完成度
  ↓
选择一个维度开始
  ↓
完成该维度问题组
  ↓
AI 生成该维度 patch
  ↓
用户确认字段
  ↓
保存局部更新
  ↓
返回构建进度页
```

### 5.3 渐进构建首页

```text
你的人生模型构建进度

Identity 我是谁               45%
Goals 我要去哪里              30%
Capabilities 我有什么          40%
State 我现在怎么样             65%

推荐下一步：
建议先完善 Goals，因为 OpenLife 目前还不清楚你接下来最想推进什么。
```

按钮：

```text
继续推荐部分
完善 Identity
完善 Goals
完善 Capabilities
完善 State
```

### 5.4 Identity 维度构建

目标：识别用户的身份、价值观、角色、边界、沟通偏好。

问题组：

```text
最近一年里，有哪些事情会让你觉得“这对我很重要，我不想妥协”？
```

提示：

```text
可以是自由、成长、家人、健康、创造、稳定、影响力，也可以是你自己的说法。
```

```text
有没有一个时刻，你觉得“那才像真正的我”？
当时你在做什么？为什么那个时刻重要？
```

```text
你现在最重要的几个身份角色是什么？
比如：创业者、学生、创作者、伴侣、家庭成员、探索者、管理者。
```

```text
有哪些事情你不希望 OpenLife 推着你去做？
或者有哪些生活边界你想保护？
```

```text
当你状态不好时，你希望 OpenLife 怎么和你说话？
```

选项：

```text
温和一点
直接一点
多问问题
帮我拆步骤
提醒我面对现实
先共情再建议
```

Identity 输出示例：

```text
Identity 更新建议

我对你的理解：
你目前最核心的身份可能是「正在寻找稳定产出节奏的创造者」。
你重视成长、自主和真实表达，同时也希望保护自己的精力边界。

建议写入：
[ ] 核心价值观：自主
原因：你多次提到不希望被固定路径限制。
风险：高，需要确认。

[ ] 核心价值观：成长
原因：你描述高光体验时强调了学习、突破和持续进步。
风险：高，需要确认。

[x] 沟通偏好：先共情，再给结构化建议
原因：你提到状态不好时不希望被直接催促。
风险：低。
```

### 5.5 Goals 维度构建

目标：把模糊愿望整理成目标层级，并识别目标冲突。

问题组：

```text
现在你脑子里反复出现、觉得应该推进的事情有哪些？
不用排序，先全部写出来。
```

```text
如果未来 90 天只能认真推进 1-2 件事，你会选什么？
为什么？
```

```text
这个目标真正重要的原因是什么？
如果完成了，它会改变你的生活状态、身份感，还是现实处境？
```

```text
你过去没有推进它，主要是因为什么？
```

选项辅助：

```text
目标太大
缺少时间
缺少能力
害怕失败
没有反馈
不知道第一步
状态不稳定
其实没那么想要
```

目标冲突提示示例：

```text
我注意到你同时提到了「想快速推进事业」和「想恢复健康节奏」。
这两个目标可能会争夺时间和精力。

你觉得这个冲突存在吗？
```

选项：

```text
存在
不存在
不确定
我想重新描述
```

Goals 输出示例：

```text
Goals 更新建议

短期目标：
[x] 完成 OpenLife 当前版本试用
原因：这是你近期反复提到、并且有明确行动场景的目标。
风险：中。

[x] 恢复稳定作息和精力
原因：你把精力波动描述为推进项目的主要阻碍。
风险：中。

长期目标：
[ ] 建立一个长期可持续的个人产品系统
原因：你提到希望未来能持续创造和积累，而不是只做短期项目。
风险：高，需要确认。

检测到的目标冲突：
事业推进 vs 健康恢复
建议：先用 2 周小实验验证最低可持续节奏。
```

### 5.6 Capabilities 维度构建

目标：盘点能力、资源、知识领域，并和目标建立关联。

问题组：

```text
哪些事情是你做起来比较自然，或者别人曾经认可过你的？
```

```text
你过去做过哪些项目、工作、学习或长期投入？
```

```text
你现在有哪些可以调用的资源？
比如时间、设备、资金、作品、平台、人脉、环境。
```

系统基于目标生成：

```text
为了推进「完成 OpenLife 试用版本」，我看到这些能力可能很关键：

1. 产品闭环设计
2. 桌面应用 Debug
3. 用户试用反馈收集
4. 长期规划收束能力

你觉得哪些是你已经具备的？哪些是缺口？
```

学习偏好问题：

```text
当你要补一个能力时，你更适合哪种方式？
```

选项：

```text
直接做项目
看系统课程
读文档
找人交流
让 AI 陪跑
写总结复盘
```

Capabilities 输出示例：

```text
Capabilities 更新建议

已有技能：
[x] 产品规划
原因：你能持续讨论阶段、PRD、试用路径和功能闭环。
风险：中。

[x] AI Agent 协作
原因：你已经使用 Codex 推进项目开发。
风险：中。

资源：
[x] 本地开发环境
[x] OpenLife 当前代码库
[x] Codex 协作流程

能力缺口：
- Beta 试用前的系统化 QA
- 用户反馈收集机制
- 桌面端打包发布流程
```

### 5.7 State 维度构建

目标：识别用户当前状态、压力、精力、习惯和可追踪指标。

问题组：

```text
如果用 3 个词描述你最近的状态，会是什么？
```

提示：

```text
比如：兴奋、焦虑、疲惫、混乱、专注、期待、卡住、平静。
```

```text
最近一周你的精力水平大概是 1-10 分多少？
压力水平又是多少？
```

```text
这个状态主要来自哪里？
```

选项：

```text
工作/项目
身体健康
关系
经济压力
目标不清晰
睡眠
信息过载
长期拖延
```

```text
你现在有哪些想维持、恢复或建立的小习惯？
```

```text
如果 OpenLife 每天或每周帮你观察一个状态指标，你最想观察什么？
```

选项：

```text
专注度
睡眠
运动
情绪稳定度
创作产出
学习投入
社交能量
压力水平
```

State 输出示例：

```text
State 更新建议

当前关注：
[x] OpenLife 试用前稳定化
原因：你最近持续围绕这个项目推进开发和调试。
风险：低。

当前状态：
[x] 压力水平：中等偏高
原因：你多次提到“功能基本无法实现”“为试用做准备”等压力场景。
风险：中，需要后续校准。

建议追踪：
[x] 每日项目推进情况
[x] 精力水平
[x] 阻塞问题数量

提醒：
如果连续 3 天压力高于 8，OpenLife 可以提醒你收束任务范围。
```

---

## 6. 苏格拉底式对话方案

### 6.1 产品目标

苏格拉底式对话不是为了快速填字段，而是为了帮助用户澄清：

- 我真正重视什么？
- 我为什么卡住？
- 我想要的目标是不是我自己的？
- 我现在的人生阶段到底在发生什么？

### 6.2 用户流程

```text
选择苏格拉底式对话
  ↓
选择一个探索主题
  ↓
OpenLife 一次只问一个问题
  ↓
每 3-5 轮生成阶段性理解
  ↓
用户确认 / 修改 / 拒绝
  ↓
形成 BuilderSignal
  ↓
生成 LifeModel patch
  ↓
用户最终确认保存
```

### 6.3 入口主题选择

页面文案：

```text
我们可以从一个真实问题开始。
你不需要先说清楚答案，只需要选择一个你愿意探索的方向。
```

主题选项：

```text
我最近有点迷茫
我有一个目标，但总是推进不了
我正在做一个重要选择
我想重新理解自己
我想整理当前人生阶段
我不知道从哪里开始
```

### 6.4 对话阶段

#### 阶段 1：入口澄清

目标：找到用户最有情绪能量的问题。

```text
如果我们今天只探索一件事，你最希望聊清楚什么？
```

如果用户说“不知道”：

```text
那我们可以先不急着找答案。
最近有没有一件事反复占用你的注意力，哪怕它很模糊？
```

如果用户仍然不知道：

```text
没关系。那我给你三个入口，你选一个最接近的：

1. 我想知道自己真正想要什么
2. 我想知道为什么自己行动不起来
3. 我想知道接下来应该优先做什么
```

写入候选：

```text
state.current_focus
state.open_questions
```

#### 阶段 2：事实层

目标：先还原现实，不急着解释。

```text
这件事最近具体是怎么表现出来的？
```

```text
如果只描述事实，不评价好坏，现在的局面是什么？
```

```text
这个状态大概持续多久了？
```

引导提示：

```text
你可以像记录流水账一样说，不需要整理得很清楚。
```

写入候选：

```text
state.recent_events
state.current_focus
state.emotional_state
```

#### 阶段 3：感受层

目标：识别情绪和压力。

```text
当你想到这件事时，最明显的感受是什么？
```

```text
这种感受更接近焦虑、兴奋、疲惫、愧疚、抗拒，还是别的？
```

```text
如果给这个压力打 1-10 分，它大概是多少？
```

写入候选：

```text
state.emotional_state.current_mood
state.emotional_state.stress_level
state.health_status.energy_level
```

#### 阶段 4：意义层

目标：找到用户在乎什么。

```text
这件事为什么对你重要？
```

```text
如果它一直没有改变，你最担心失去什么？
```

```text
如果它真的变好了，你觉得自己会重新获得什么？
```

引导提示：

```text
这里没有标准答案。很多时候，我们在意的不是事情本身，而是它代表的东西。
```

写入候选：

```text
identity.values
identity.life_philosophy
goals.long_term
```

#### 阶段 5：冲突层

目标：识别内部矛盾。

```text
你有没有感觉自己一部分想要 A，另一部分又想要 B？
```

```text
如果你真的去做这件事，你会付出什么代价？
```

```text
如果你不做，又会付出什么代价？
```

```text
你现在更像是缺少方法，还是有一部分自己并不想真的走这条路？
```

输出候选：

```ts
{
  conflict_type: "value_conflict" | "goal_conflict" | "identity_conflict" | "capacity_conflict";
  side_a: string;
  side_b: string;
  confidence: number;
}
```

#### 阶段 6：能力与现实层

目标：把洞察落回行动。

```text
现在你已经具备哪些条件？
```

```text
真正缺的是能力、时间、精力、环境、支持，还是清晰度？
```

```text
如果只做一个很小的实验，什么行动可以帮你验证这个方向？
```

写入候选：

```text
capabilities.skills
capabilities.resources
capability gaps
goals.short_term
goals.daily
```

#### 阶段 7：阶段性确认

每 3-5 轮触发一次总结。

总结模板：

```text
我先整理一下目前的理解，但这不是结论，你可以改。

我目前这样理解你：

1. 你现在最关注的是「xxx」。
2. 这件事背后，你可能很重视「xxx」。
3. 你的卡点不只是「xxx」，也可能和「xxx」有关。
4. 你真正想要的目标，可能不是「xxx」，而是「xxx」。
5. 我还不确定的是「xxx」。

你觉得哪些准确？哪些不准确？
```

用户选项：

```text
基本准确，保存这些理解
部分准确，我来修改
不准确，重新理解
先不保存，继续聊
```

### 6.5 苏格拉底式安全边界

必须避免：

```text
你其实是一个……
你的真正问题是……
你的人生使命是……
你应该……
```

应该使用：

```text
我暂时这样理解……
这可能说明……
我不确定这是否准确……
你愿意确认或修正吗？
```

高风险字段永远不能自动写入：

```text
identity.values
identity.mission_statement
identity.life_philosophy
goals.life_goals
goals.long_term
role_definition.primary_role
```

---

## 7. 统一确认页

三种构建方式都进入统一的模型更新确认页。

页面标题：

```text
OpenLife 准备更新你的人生模型
```

分四个维度展示：

```text
Identity 我是谁
[ ] 核心价值观：成长
原因：你多次提到希望持续突破和学习。
来源：快速构建 Q4 / 苏格拉底对话第 3 轮
置信度：78%
风险：高，需要确认

Goals 我要去哪里
[x] 短期目标：完成 OpenLife 试用版本
原因：你近期持续围绕这个项目推进。
来源：快速构建 Q3
置信度：85%
风险：中

Capabilities 我有什么
[x] 技能：AI Agent 协作
原因：你正在使用 Codex 推进项目开发。
来源：快速构建 Q5
置信度：80%
风险：中

State 我现在怎么样
[x] 当前关注：试用前稳定化
原因：你近期多次围绕 Debug 和试用准备展开。
来源：对话信号
置信度：90%
风险：低
```

底部按钮：

```text
保存选中内容
全部保存
返回修改
暂不保存
```

保存后提示：

```text
已更新你的人生模型，并创建快照：
initial:quick-builder
progressive:goals
socratic:identity-values
```

---

## 8. 数据结构建议

### 8.1 BuilderSession

```ts
interface BuilderSession {
  id: string;
  mode: "quick" | "progressive" | "socratic";
  status: "active" | "paused" | "completed" | "abandoned";
  current_stage: string;
  current_dimension?: "identity" | "goals" | "capabilities" | "state";
  answers: BuilderAnswer[];
  extracted_signals: BuilderSignal[];
  pending_patch: Partial<LifeModel>;
  confirmed_patch: Partial<LifeModel>;
  confidence_by_dimension: Record<string, number>;
  created_at: string;
  updated_at: string;
  completed_at?: string;
}
```

### 8.2 BuilderAnswer

```ts
interface BuilderAnswer {
  question_id: string;
  question_text: string;
  answer_text: string;
  dimension: string;
  created_at: string;
}
```

### 8.3 BuilderSignal

```ts
interface BuilderSignal {
  id: string;
  source_answer_id: string;
  dimension: "identity" | "goals" | "capabilities" | "state";
  affected_path: string;
  proposed_value: unknown;
  confidence: number;
  reason: string;
  risk_level: "low" | "medium" | "high";
  user_status: "pending" | "accepted" | "edited" | "rejected";
}
```

### 8.4 BuilderSummary

```ts
interface BuilderSummary {
  identity_summary: string;
  goals_summary: string;
  capabilities_summary: string;
  state_summary: string;
  assumptions: string[];
  unresolved_questions: string[];
  recommended_next_steps: string[];
}
```

---

## 9. 开发计划

## Phase 1：快速构建 MVP

### 目标

让新用户能在 3-5 分钟内完成初始人生模型构建，并进入 Chat / Dashboard。

### 后端范围

涉及文件：

```text
openlife-core/src/builder.rs
src-tauri/src/commands/builder.rs 或 src-tauri/src/lib.rs
openlife-core/src/life_model.rs
```

开发内容：

- 定义 `BuilderMode::Quick`。
- 定义快速构建问题列表。
- 实现 `quick_build_from_answers()`。
- 生成 `LifeModelPatch`。
- 生成 `BuilderSummary`。
- 支持确认后应用 patch。
- 保存 LifeModel。
- 创建快照 `initial:quick-builder`。

建议结构：

```rust
pub enum BuilderMode {
    Quick,
    Progressive,
    Socratic,
}

pub struct BuilderQuestion {
    pub id: String,
    pub dimension: BuilderDimension,
    pub text: String,
    pub helper_text: Option<String>,
    pub choices: Vec<String>,
    pub required: bool,
}

pub struct BuilderSignal {
    pub dimension: BuilderDimension,
    pub affected_path: String,
    pub proposed_value: serde_json::Value,
    pub confidence: f32,
    pub reason: String,
    pub risk_level: RiskLevel,
    pub source_question_id: String,
}

pub struct BuilderPatch {
    pub signals: Vec<BuilderSignal>,
    pub summary: BuilderSummary,
}
```

### 前端范围

涉及文件：

```text
frontend/src/pages/BuilderPage.tsx
frontend/src/tauri.ts
frontend/src/types.ts
```

开发内容：

- Builder 首页显示三种模式。
- 快速构建问题流。
- 回答进度条。
- AI 生成草稿 loading 状态。
- 模型确认页。
- 保存成功页。
- 跳转 Chat / Dashboard / 渐进构建。

### 验收标准

```text
空白用户进入 Builder
选择快速构建
回答 7 个问题
看到模型草稿
确认保存
Dashboard 能显示模型摘要
Chat 能基于模型对话
VersionControl 能看到 initial 快照
```

### 测试

前端：

```text
BuilderPage.test.tsx
- 渲染三种构建方式
- 快速构建可以进入问题流
- 回答完成后显示确认页
- 保存后调用 apply_builder_patch
```

Rust：

```text
builder.rs
- quick answers 可以生成四维 patch
- 高风险字段默认 pending
- 保存后 LifeModel 非空
```

---

## Phase 2：统一确认组件

### 目标

所有 AI 推断都必须可确认、可编辑、可拒绝。

### 前端组件

新增：

```text
frontend/src/components/BuilderPatchReview.tsx
```

功能：

- 按四维分组显示 signals。
- 显示 reason / source / confidence / risk。
- 低风险默认勾选。
- 中风险默认勾选但可取消。
- 高风险默认不勾选。
- 支持编辑 proposed value。
- 支持批量保存。

### 后端支持

新增或统一：

```rust
apply_builder_signals(session_id, accepted_signals)
reject_builder_signals(session_id, rejected_signals)
edit_builder_signal(signal_id, new_value)
```

### 验收标准

```text
Quick / Progressive / Socratic 都进入同一个确认页
高风险字段不会自动保存
保存后生成快照
VersionControl 可以看到来源
```

---

## Phase 3：渐进构建 MVP

### 目标

让用户按四维逐步完善模型，可以暂停和继续。

### 后端内容

- 定义 `BuilderMode::Progressive`。
- 定义四个维度的问题组。
- 支持 `current_dimension`。
- 支持 session pause / resume。
- 每个维度生成局部 patch。
- 计算完成度变化。
- 保存维度级快照。

新增接口建议：

```rust
builder_start(mode, session_id)
builder_get_progress(session_id)
builder_start_dimension(session_id, dimension)
builder_answer_question(session_id, question_id, answer)
builder_generate_dimension_patch(session_id, dimension)
builder_apply_signals(session_id, accepted_signal_ids)
builder_pause(session_id)
builder_resume(session_id)
```

### 前端内容

- 渐进构建首页显示四维完成度。
- 每个维度有独立入口。
- 问题组逐步展示。
- 维度完成后显示更新建议。
- 支持暂停和继续。
- 首页推荐下一步维度。

### 验收标准

```text
用户可以只完善 Goals
退出页面
再次进入后继续
Goals 完成度提升
其他维度不受影响
保存后创建 progressive:goals 快照
```

---

## Phase 4：苏格拉底式对话 MVP

### 目标

支持深度对话式建模，并通过阶段性确认写入 LifeModel。

### 后端内容

- 定义 `BuilderMode::Socratic`。
- 定义 Socratic stages。
- 实现下一问题生成器。
- 实现每 3-5 轮信号提取。
- 实现阶段性总结。
- 生成 pending signals。
- 用户确认后写入 LifeModel。
- 高风险字段必须手动确认。

Socratic stages：

```text
entry
facts
emotion
meaning
conflict
capability
confirmation
```

建议结构：

```rust
pub enum SocraticStage {
    Entry,
    Facts,
    Emotion,
    Meaning,
    Conflict,
    Capability,
    Confirmation,
}

pub struct SocraticTurn {
    pub user_reply: String,
    pub assistant_question: String,
    pub stage: SocraticStage,
    pub extracted_signals: Vec<BuilderSignal>,
}

pub struct SocraticSummary {
    pub understanding: Vec<String>,
    pub uncertain_points: Vec<String>,
    pub pending_signals: Vec<BuilderSignal>,
    pub next_question: String,
}
```

### 前端内容

- 对话式 Builder 页面。
- 当前阶段提示。
- 一次只问一个问题。
- 阶段性总结卡片。
- 用户可选择保存、修改、拒绝、继续聊。
- 最终确认页复用统一确认组件。

### 验收标准

```text
用户选择“我最近有点迷茫”
系统进入 entry stage
连续对话 3-5 轮
系统生成阶段性理解
用户确认后才写入模型
高风险字段默认不勾选
```

---

## Phase 5：Builder 与产品主链路联动

### 目标

Builder 不只是 onboarding，而是长期可回来的模型完善入口。

开发内容：

- Dashboard 显示模型缺口。
- Chat 中识别模型缺失并推荐 Builder。
- Settings Beta readiness 检查 Builder 完成度。
- Calibration 可以引用 Builder signals。
- VersionControl 展示 Builder 快照来源。

Dashboard 示例：

```text
Goals 完成度较低，建议继续渐进构建。
```

Chat 示例：

```text
我还不太了解你的长期目标，要不要进入 Builder 完善？
```

验收标准：

```text
用户完成快速构建后
Dashboard 推荐继续完善 Goals
用户完成 Goals 渐进构建后
Dashboard 推荐进入 Chat 验证
```

---

## 10. 推荐开发顺序

### 第一轮：快速构建完整闭环

```text
目标：新用户能完成初始模型。
范围：Quick Builder + 确认页 + 保存 + 快照。
不做：苏格拉底深度对话。
```

### 第二轮：统一确认组件

```text
目标：所有 AI 推断都必须可确认。
范围：BuilderPatchReview + risk level + apply signals。
不做：复杂 UI 动画。
```

### 第三轮：渐进构建四维工作台

```text
目标：用户能分维度补全模型。
范围：四维完成度 + dimension question flow + pause/resume。
不做：深度 AI 追问。
```

### 第四轮：苏格拉底式对话 MVP

```text
目标：支持深度探索和阶段性总结。
范围：stage engine + 3-5 轮总结 + pending signals。
不做：复杂心理咨询式功能。
```

### 第五轮：主链路联动

```text
目标：Builder 和 Dashboard / Chat / Calibration / VersionControl 打通。
范围：缺口推荐、快照来源、继续构建入口。
```

---

## 11. 下一步 Codex 开发任务模板

```md
目标：
实现 OpenLife Builder 的快速构建完整闭环。

范围：
1. Builder 首页展示三种构建方式。
2. 实现 Quick Builder 七步问题流。
3. 根据回答生成 LifeModel patch。
4. 增加 BuilderPatchReview 确认页。
5. 用户确认后保存 LifeModel。
6. 创建 initial:quick-builder 快照。
7. 完成后跳转 Chat / Dashboard / 渐进构建。

涉及文件：
- openlife-core/src/builder.rs
- src-tauri/src/commands/builder.rs 或 src-tauri/src/lib.rs
- frontend/src/pages/BuilderPage.tsx
- frontend/src/tauri.ts
- frontend/src/components/BuilderPatchReview.tsx
- frontend/src/pages/BuilderPage.test.tsx

非目标：
- 本轮不做完整苏格拉底式对话。
- 本轮不做复杂 LLM 自动追问。
- 本轮不做多用户账号体系。

验证：
- cargo test -q
- cd frontend && npm test -- --run BuilderPage
- cd frontend && npm run build
```
