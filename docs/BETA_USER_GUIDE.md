# OpenLife Historical Beta 用户试用指南

> Historical/draft user guide. This file is not a current product entrypoint or
> onboarding contract.
>
> Current product entrypoints are `Today`, `Companion`, `Mailbox`, `Life Model`,
> `Runs`, and `Settings`. Legacy URLs only redirect to those current surfaces.
> OpenLife has not been declared full Beta by the current authority docs. Use
> this file only for historical product-language reference. Current release
> status, blockers, tool capabilities, and development order are governed by
> `AGENTS.md`, `plans/README.md`,
> `plans/openlife_lifemodel_governed_agent_runtime.md`, and
> `plans/openlife_react_beta_roadmap.md`.

---

## 1. 什么是 OpenLife？

OpenLife 是你的**终身成长合伙人**。它通过 AI 持续对话，帮助你梳理人生目标、记录成长轨迹、并获得贴合你个人背景的建议。

核心概念只有四个：

| 维度 | 含义 | 示例 |
|------|------|------|
| **身份 (Identity)** | 你是谁、重视什么 | 价值观：健康、家庭、成长 |
| **目标 (Goals)** | 你想达成什么 | 短期：每天阅读30分钟；长期：三年内转行 |
| **能力 (Capabilities)** | 你拥有什么技能与资源 | 编程、人脉、时间 |
| **状态 (State)** | 你现在的身心状况 | 精力水平、情绪、近期事件 |

OpenLife 的所有建议都基于这四个维度的信息。信息越完整，AI 的建议越贴合你。

---

## 2. 历史首次启动设想（已退役）

早期草案曾设计全屏「欢迎使用 OpenLife」启动向导。该向导已经退役；当前产品不再用
blocking onboarding 截断默认路径。首次进入应直接落在当前 ProductShell，并通过 Today
或 Settings 的非阻塞使用准备状态提示后续配置。

### 当前建议：配置至少一个模型后端

OpenLife 需要至少一个 LLM 后端才能对话。推荐方式：

**云端模型（推荐）：**
- 前往「设置」页面
- 在「LLM 配置」中填写你的 OpenRouter API Key 或 OpenAI API Key
- 点击「测试连接」确认可用

**本地模型（可选，数据完全本地）：**
- 安装 [Ollama](https://ollama.com/)
- 运行 `ollama run qwen2.5:7b`
- OpenLife 会自动检测本地模型状态

> ⚠️ 如果没有配置任何后端，当前 `Companion` 会显示配置提示，且无法发送消息。

### 历史第 3 步：构建你的人生模型（当前入口是 Life Model → Build）

- 前往当前 `Life Model` 页面中的 build 二级流程
- 选择「快速构建」模式（约 3-5 分钟）
- 回答几个引导性问题，AI 会自动提取你的价值观、目标和能力
- 完成后回到 `Life Model` 检查，并在 `Mailbox` 中确认候选更新

> 💡 人生模型是 OpenLife 理解你的核心。即使只完成一次快速构建，`Companion` 的对话质量也会显著提升。

---

## 3. 日常使用流程

### 3.1 历史对话草案（当前入口是 Companion）

- 在 `Companion` 输入任何想法、困惑或目标
- AI 会根据你的人生模型和对话历史给出建议
- 支持快捷指令：
  - `/goal 名称` — 添加每日目标
  - `/done 名称` — 完成每日目标
  - `/state 维度名 数值 [备注]` — 记录状态

### 3.2 历史仪表盘草案（当前由 Today / Life Model / Runs / Settings 分担）

- `Today` 查看当前状态、建议和待处理确认项
- `Life Model` 查看四维摘要、可信度和待确认更新
- `Runs` 查看 AgentRun 和 trace evidence
- `Settings` 查看诊断、Provider 和高级配置

### 3.3 人生模型（手动编辑）

- 随时编辑你的价值观、目标、技能和状态
- 所有修改会自动保存
- 支持折叠/展开各个区块

### 3.4 周期校准（每周/每月）

- 系统会在每周一和每月 1 日提示进行校准
- 校准会分析你的反馈、行为和对话，生成优化建议
- 你可以选择接受或拒绝每一项建议
- 每个建议都标注了来源（反馈 / 行为 / 对话推断）和置信度

### 3.5 版本控制（人生模型快照）

- 在「版本控制」页面可以手动创建快照
- 快照保存了某一时刻的完整人生模型
- 支持对比两个版本的差异和一键回滚

---

## 4. 常见问题

### Q: 对话时提示「未配置 LLM 后端」怎么办？

A: 前往「设置 → LLM 配置」，填写 OpenRouter 或 OpenAI 的 API Key，或确保 Ollama 已在本地运行。

### Q: 为什么 AI 的建议很通用？

A: 请先完成「构建」向导创建初始人生模型。AI 只有在了解你的价值观和目标后，才能给出个性化建议。

### Q: 数据保存在哪里？安全吗？

A: 所有数据保存在你的电脑本地：
- macOS: `~/Library/Application Support/ai.openlife.app/`
- Windows: `%APPDATA%/ai.openlife.app/`
- Linux: `~/.config/ai.openlife.app/`

如果你使用云端模型，仅对话内容会发送到对应的 API 服务商（如 OpenRouter）。所有敏感信息（身份证号、银行卡号等）在发送前会被自动脱敏或拦截。

### Q: 如何备份数据？

A: 前往「设置 → 数字遗产 / 数据迁移」，点击「导出全部数据」。导出文件为 JSON 格式，包含人生模型、聊天记录和向量记忆。

导入时，如果文件版本与当前应用版本的主版本号不一致，系统会拒绝导入以防止数据不兼容。

### Q: 旧 Beta 试用设想有什么已知限制？

A:
- 本地模型（Ollama）在工具调用方面能力有限，涉及工具调用时会自动切换到云端模型
- 向量记忆检索在数据量极大时可能有性能瓶颈
- 首次启动 Ollama 模型时可能需要数秒加载时间

---

## 5. 反馈渠道

试用过程中遇到任何问题，请通过以下方式反馈：

1. 在「设置 → 系统维护」中查看系统诊断信息
2. 将诊断信息和问题描述一并提交

感谢你的试用！你的反馈将帮助 OpenLife 变得更好。
