# OpenLife Beta Trial Guide

> 面向真实测试用户。无需阅读源码或架构文档即可完成首次试用。

---

## OpenLife 是什么

OpenLife 是一个本地优先的个人 Agent 框架雏形。它围绕你的私人 LifeModel（人生模型）运行 AI 模型，帮你完成对话、规划、写作、复盘和工具调用，并在你确认后持续更新对你的理解。

它不是单纯的聊天应用，也不是普通的待办清单。它会在你明确确认的前提下，记住你的目标、偏好、状态和能力。

---

## 测试前准备

### 系统支持

- **macOS**：当前主要测试平台
- **Windows / Linux**：已知可以编译运行，但当前 Beta RC 未经这两个平台充分测试

### 你需要什么

**推荐最低配置：至少一个可用的云端模型 Provider**

- 一个 DeepSeek API Key、OpenAI API Key 或 OpenRouter API Key
- 能够正常访问对应 API 的网络环境

**可选：使用本地 Ollama 模型**

- 在本地安装并运行 [Ollama](https://ollama.com)
- 至少 pull 一个模型（如 `deepseek-r1:7b`、`qwen2.5:7b` 等）

**不需要安装：**

- 不需要安装 Node.js、Rust、pnpm 等开发工具（这些仅开发者需要）
- 不需要阅读任何源码

### 数据存放位置

所有数据存储在本地：

- macOS: `~/Library/Application Support/ai.openlife.desktop/`
- 你的 LifeModel、聊天记录、记忆、提案等全部保存在本地 SQLite 数据库中
- 没有数据自动上传到云端

---

## 启动应用

> **当前是 RC 阶段，还没有独立的桌面应用包。** 你需要用开发者命令启动，或者使用实验性构建产物。

### 方式一：开发者启动（推荐）

如果你已安装开发环境（Node.js + Rust + pnpm），在项目根目录执行：

```bash
make dev
```

### 方式二：Release 构建产物（如已构建）

如果已有 release build 产物，在 `src-tauri/target/release/bundle/` 中找到对应平台的应用包打开即可。

> 当前 release build 尚未签名/公证，macOS 可能需要手动允许运行：系统偏好设置 → 安全性与隐私 → 仍要打开。

---

## 第一步：进入 Settings 查看状态

打开应用后：

1. 点击右上角齿轮图标进入 **Settings**
2. 默认进入 **Overview** 标签页
3. 查看 **Beta Readiness** 卡片

### 理解 Readiness 状态

| 状态 | 颜色 | 含义 | 下一步 |
|------|------|------|--------|
| Ready | 绿色 | 该项已就绪 | 继续下一项 |
| Partial | 橙色 | 部分就绪，有警告 | 点击查看详情 |
| Blocked | 红色 | 必须修复 | 点击跳转到修复位置 |
| Safe Mode | 黄色横幅 | 系统检测到问题，高风险操作已禁用 | 导出诊断 → 尝试修复 |

常见首次启动时的 blocked 项：
- **模型后端未就绪** → 下一节配置 Provider
- **LifeModel 未建立** → 参考"构建 LifeModel"章节

---

## 第二步：配置模型 Provider

### 配置云端 Provider（推荐）

1. Settings → **Provider** 标签页
2. 选择 Provider：**DeepSeek**、**OpenAI（兼容）** 或 **OpenRouter**
3. 填入你的 **API Key**
4. 确认 **Base URL** 正确（通常默认即可）
5. 确认 **Model Name** 正确（如 `deepseek-chat`、`gpt-4o-mini`）
6. 点击 **Test Connection**
7. 看到成功提示后点击 **Save**

常见失败原因：
- API Key 错误或已过期
- Base URL 填写错误
- 模型名填写错误（注意区分 `deepseek-chat`、`deepseek-reasoner` 等）
- 网络无法访问 API

### 配置本地 Ollama（可选）

1. 确保本地 `ollama serve` 正在运行（默认 http://localhost:11434）
2. Settings → **Provider** → 选择 **Ollama**
3. 填入已 pull 的模型名
4. 点击 **Test Connection**
5. 保存配置

常见失败原因：
- Ollama 服务未启动：终端执行 `ollama serve`
- 模型未 pull：终端执行 `ollama pull <模型名>`
- 端口被占用

> **注意**：Ollama 本地 7B 级别模型在工具调用和复杂推理上不如云端模型。如果 Chat 无响应或响应质量差，建议切换到云端 Provider。

### 验证配置

回到 Settings → **Overview**，确认"模型后端"项从 blocked 变为 ready（绿色勾）。

---

## 第三步：构建最小 LifeModel

LifeModel 是 OpenLife 的核心——它让 AI 了解你是谁、你的目标、能力和当前状态。

1. 导航到 **Builder** 页面
2. 选择 **Quick Build** 模式（推荐首次使用）
3. 按引导回答以下维度的问题：
   - **身份**：你是谁、你的价值观
   - **目标**：短期和中长期目标
   - **能力**：你的技能和资源
   - **状态**：当前关注点和状态

### 重要：Builder 默认不直接修改 LifeModel

Build 完成后，Builder 会生成 **Proposal（提案）** 而不是直接写入。这是设计行为——高风险的 LifeModel 字段需要你明确确认才会写入。

完成 Build 后：
1. 页面会提示 Proposal 已生成
2. 前往 **Review Center** 查看待确认的提案
3. 逐条审阅并 accept（接受）、edit（编辑）或 reject（拒绝）

> 如果 Builder 完成但找不到 Proposal：直接打开 Review Center 导航页。

---

## 第四步：完成第一次对话

1. 导航到 **Chat** 页面
2. 输入一个真实问题或陈述，例如：
   - "我想学西班牙语，帮我制定一个学习计划"
   - "最近工作上压力很大，帮我分析一下"
3. 等待助手回复

### 预期行为

- 助手应该基于你的 LifeModel 给出个性化回复
- 如果对话内容触发了 LifeModel 更新建议，**顶部会出现 Proposal 横幅**，点击可跳转到 Review Center
- 如果回复一直无响应：
  - 回到 Settings → Overview 检查是否有 readiness 问题
  - 确认 Provider 测试连接成功
  - 尝试再次发送消息

### 理解流式输出

- 助手回复会逐字显示
- 回复下方会显示：使用的模型/Provider、工具调用次数、生成的 Proposal 数量
- 每次对话都会在 **Runs** 页面创建一条记录

---

## 第五步：审阅 Proposal

Proposal 是 OpenLife 的确认机制——AI 的建议必须经过你的确认。

1. 导航到 **Review Center**
2. 查看待处理提案列表，关键信息：
   - **影响路径**：这个提案会修改 LifeModel/Memory/Tool 的哪个部分
   - **风险等级**：low / medium / high
   - **来源**：来自哪个 Builder 会话或 Chat AgentRun
3. 每个提案的操作：
   - **Accept**：接受并应用
   - **Reject**：拒绝
   - **Edit**：修改后再接受
   - **Postpone**：稍后处理

### 高风险变更特别注意

- 身份、价值观、长期目标等高风险字段的变更需要特别注意
- 应用成功后会自动创建版本快照，方便回滚
- 如果 apply 失败（status 仍为 pending），说明系统遇到了配置或权限问题，请导出诊断后检查

---

## 第六步：查看 Runs / Trace

每次 AI 交互都会创建一条 AgentRun 记录。

1. 导航到 **Runs** 页面
2. 点击最近一次运行查看详情
3. Run 详情包含：
   - **时间线**：运行过程中各阶段的摘要
   - **工具调用**：模型在本次运行中使用了哪些工具、输出摘要
   - **Proposal 关联**：本次运行生成了哪些提案
   - **元数据**：使用的模型、Provider、fallback 标记

> 这是一个可追溯的审计界面，不必理解内部架构也能看懂大致发生了什么。

---

## 第七步：导出诊断并反馈

### 导出诊断报告

1. Settings → **Data** 标签页
2. 点击 **Export Diagnostics**
3. 保存 `.json` 文件

**诊断报告默认不包含以下敏感数据：**
- API Key
- 完整的 LifeModel 原始内容
- 完整的聊天消息
- 完整的记忆原始内容
- 工具调用输出原文
- 本地文件路径（已替换为 `[local-path]` / `[local-file-url]`）

### 导出完整备份

Settings → Data → **Export All Data**：
- 用于个人数据备份和迁移
- 包含你的完整 LifeModel、消息、记忆等
- **此文件包含私人数据，注意妥善保管，不要分享给他人**

### 如何反馈

反馈时请提供以下信息（不要在反馈中包含你的 API Key、LifeModel 原文、聊天内容原文等私人数据）：

```
**操作步骤：** （你做了什么）
**预期结果：** （你期望看到什么）
**实际结果：** （实际发生了什么）
**Run ID / Proposal ID：** （如果看得到）
**诊断报告：** （附上导出的诊断报告）
```

发送反馈至项目指定的反馈渠道。

---

## 常见问题

### Chat 无响应

1. 检查 Settings → Overview，"模型后端"是否为 blocked
2. 前往 Settings → Provider → Test Connection
3. 如果测试失败，检查 API Key / Base URL / 模型名
4. 如果使用 Ollama，检查 `ollama serve` 是否在运行
5. 导出诊断报告，查看是否有报错信息

### Provider 测试失败

- **Invalid API Key**：确认 Key 正确、未过期、有余额
- **Base URL 错误**：确认格式正确，包含 `https://` 前缀
- **Model not found**：确认模型名与大写/小写/连字符完全一致
- **Connection refused**：检查网络和防火墙
- **Ollama not reachable**：确认 `ollama serve` 在 `localhost:11434` 运行

### Builder 完成后没有变化

- Builder 生成的是 Proposal，不是直接修改
- 前往 Review Center 查看并确认提案
- 确认后 LifeModel 才会更新

### Proposal apply 失败

- 提案保持 pending 状态，不会丢失
- 这是安全设计，防止错误的配置导致数据问题
- 导出诊断报告查看具体错误
- 检查是否处于 Safe Mode

### Safe Mode

- 出现黄色 Safe Mode 横幅说明系统检测到潜在问题
- Safe Mode 下高风险操作被自动禁用
- 操作顺序：导出诊断 → 备份数据 → 尝试修复 → 刷新状态
- 不要忽略 Safe Mode 继续执行写操作

### 诊断导出在哪里

Settings → **Data** 标签页 → **Export Diagnostics**

### 如何备份数据

- Settings → Data → **Export All Data** 导出 JSON 备份
- 数据默认存储在 `~/Library/Application Support/ai.openlife.desktop/`（macOS）
- 你可以手动复制整个目录作为备份

---

## 已知限制

这是 **小范围 Beta RC**，不是公开发布版本：

- **没有正式签名/公证的桌面应用包**：当前 macOS 需要手动允许运行
- **Windows/Linux 未经充分测试**
- **Shell/Terminal 类能力默认关闭**：没有命令行终端，AI 不能执行系统命令
- **部分工具为 proposal-first 或 declarative-only**：
  - 某些 email/calendar 集成可能需要外部配置
  - 声明-only 工具仅存在于工具清单中，不会进入实际执行
- **插件/外部工具能力受治理限制**
- **多语言支持有限**：当前以中文为主

---

## 相关文档

如需更详细的测试脚本和验收标准，参考：

- [P11 Trial Path Matrix](plans/openlife_vnext_p11_trial_path_matrix.md) — 结构化冒烟测试脚本
- [P12 Beta RC Acceptance Report](plans/openlife_vnext_p12_beta_rc_acceptance_report.md) — RC 验收报告模板

---

*本文档面向 Beta RC 测试用户。不要粘贴 API Key、私人 LifeModel、聊天原文到公开反馈中。*
