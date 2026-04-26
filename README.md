# OpenLife

OpenLife 是一个**本地优先的个人 Agent 框架**。它不是单纯的聊天应用，也不是普通的目标管理工具，而是围绕用户私人数据构建的个人 AI 操作系统雏形。

OpenLife 的核心范式是：

```text
LifeModel + Local/Cloud Model Router + Agent Runtime + Memory/Feedback Loop
```

用户先构建自己的 LifeModel，包括身份、目标、能力、状态、偏好和关系等私人上下文。之后，OpenLife 会让本地模型或云端模型在这个人生模型的约束下完成对话、规划、写作、复盘、工具调用、状态更新和长期反馈。系统不只是回答问题，还应该逐步理解用户，并在用户确认下持续更新 LifeModel。

## 当前定位

当前项目处于 **Agent Framework Alpha** 阶段：

- 已经具备 LifeModel、Builder、Chat、Memory、MCP/A2A、Calibration、VersionControl、Diagnostics 等核心材料。
- 还没有完全完成统一的 Agent Runtime。
- 接下来的开发重点不是继续堆页面，而是把 `AgentTask -> AgentRun -> Actions/Observations -> Proposals -> Confirmation -> Persistence` 这条架构主线打通。

新的架构基准文档见：

- [OpenLife Agent Framework Architecture](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)

## 核心能力

| 能力 | 当前状态 | 目标形态 |
|---|---|---|
| LifeModel | 已有四维模型和编辑器 | 成为所有 Agent 任务的私人上下文层 |
| Builder | 已支持快速、渐进、苏格拉底式构建 | 通过 Proposal 机制安全写入 LifeModel |
| Chat | 已支持流式对话和历史持久化 | 升级为 Agent 执行界面，展示上下文、模型路由和运行轨迹 |
| Model Router | 已支持本地 Ollama 与云端 OpenAI-compatible | 升级为按任务、隐私、能力和成本路由的 ModelRouter |
| Memory | 已有 SQLite 与向量记忆 | 升级为可治理、可归档、可追踪来源的长期记忆层 |
| MCP/A2A | 已有工具和外部 Agent 接入基础 | 成为 AgentAction 执行层，并默认受权限和审计保护 |
| Calibration/Evolution | 已有建议和校准雏形 | 统一进入 Proposal/Confirmation 机制 |
| Diagnostics/Safe Mode | 已有试用稳定化能力 | 成为系统控制台和恢复中枢 |

## 技术栈

| 层级 | 技术 |
|---|---|
| 前端 | React 18 + TypeScript + Tailwind CSS + Vite |
| 桌面壳 | Tauri 2.x |
| 后端核心 | Rust Workspace (`openlife-core` + `openlife-tauri`) |
| 本地模型 | Ollama |
| 云端模型 | DeepSeek / OpenAI / OpenRouter / Custom OpenAI-compatible |
| 数据存储 | SQLite + YAML |

## 项目结构

```text
.
├── frontend/                     # React 前端
│   └── src/
│       ├── pages/                # Chat / Dashboard / Builder / Settings 等当前页面
│       ├── components/           # 通用组件
│       ├── tauri.ts              # Tauri command 封装层
│       └── App.tsx               # 路由与全局布局
├── openlife-core/                # Rust 核心业务库
│   └── src/
│       ├── life_model.rs         # LifeModel
│       ├── builder.rs            # LifeModel 构建与 Review
│       ├── hermes.rs             # 早期 Meaning/Strategy/Execution 决策总线
│       ├── scheduler.rs          # 当前模型调度器
│       ├── llm.rs / ollama.rs    # 云端与本地模型调用
│       ├── memory.rs             # 消息、会话、状态等 SQLite 存储
│       ├── vectors.rs            # 向量记忆
│       ├── mcp.rs / a2a.rs       # 工具与外部 Agent 接入
│       ├── privacy.rs            # 隐私检测与脱敏
│       ├── feedback.rs           # 反馈信号
│       ├── evolution.rs          # 微进化
│       └── versioning.rs         # 快照与回滚
├── src-tauri/                    # Tauri 命令层和桌面壳
│   └── src/
│       ├── lib.rs                # 核心状态与聊天主链路
│       └── commands/             # 按领域拆分的 Tauri commands
├── plans/                        # 架构与开发计划
└── OpenLife_Final_PRD.md         # 旧版 PRD，当前作为历史参考
```

## 推荐阅读顺序

1. [OpenLife Agent Framework Architecture](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)
2. [OpenLife PRD v2: Personal Agent Framework](/Users/fujing/Desktop/偶来福/OpenLife_PRD_v2_Agent_Framework.md)
3. [OpenLife Development Plan](/Users/fujing/Desktop/偶来福/plans/openlife_development_plan.md)
4. [Codex Execution Playbook](/Users/fujing/Desktop/偶来福/plans/openlife_codex_execution_playbook.md)
5. [OpenLife Final PRD](/Users/fujing/Desktop/偶来福/OpenLife_Final_PRD.md)，仅作为历史需求参考

## 快速开始

### 前置要求

- Rust >= 1.75
- Node.js 18+
- pnpm 或 npm
- 可选：Ollama，本地模型服务

### 安装依赖

```bash
cd frontend && pnpm install
cd ..
```

如果本机没有 `pnpm`，项目脚本会尝试 fallback 到 npm。

### 配置模型

当前推荐先使用 DeepSeek 跑通云端试用链路，也可以使用 Ollama 本地模型。

```bash
# DeepSeek，推荐试用路径
export DEEPSEEK_API_KEY="sk-..."

# OpenRouter
export OPENROUTER_API_KEY="sk-..."

# OpenAI
export OPENAI_API_KEY="sk-..."
```

桌面端中进入 `Settings`，选择 Provider，填写 Key，点击测试连接，成功后保存。

### 开发运行

```bash
./dev.sh
```

### 测试

```bash
cargo test -q
cd frontend && npm test -- --run
cd frontend && npm run build
```

## 当前推荐试用路径

当前 UI 还没有完全迁移到 Agent Workspace，因此建议按下面路径体验：

1. `Settings` 完成模型配置、诊断检查和 Safe Mode 检查。
2. `Builder` 完成一次快速构建，或恢复待确认 Review。
3. 在 Review 中确认要写入 LifeModel 的字段。
4. `Chat` 发起一次个性化对话。
5. `Dashboard` 查看下一步行动和模型依据。
6. `Calibration / VersionControl / Memory` 查看建议来源、记忆和回滚路径。

这条路径是当前 Alpha 的主链路。后续会被迁移为：

```text
Workspace -> Agent Task -> Agent Run Trace -> Proposal Review -> LifeModel/Memory Update
```

## 当前重要开发方向

短期不再优先扩页面，而是围绕以下主线推进：

1. 引入 `AgentTask` 和 `AgentRun`，让每次 AI 执行可追踪。
2. 将 Chat 升级为第一个 Agent 执行表面。
3. 统一 Builder、Calibration、Evolution 的 Proposal/Confirmation 机制。
4. 将 Scheduler 升级为真正的 ModelRouter。
5. 将 Dashboard 重构为 Workspace。
6. 建立 Proactive Agent 的安全 MVP。

## 常见问题

- API Key 测试失败：确认 Provider、Base URL、模型名和 API Key 匹配。
- Ollama 连接失败：确认 Ollama 已启动，且模型名称存在。
- Safe Mode：说明当前数据环境存在风险，先去 Settings 的恢复控制台导出备份并修复。
- Chat 无响应或一直思考：先查看 Settings 诊断，再检查模型 Provider 测试结果。
- Builder Review 应用后模型没有变化：检查 skipped 字段和版本快照，确认是否被安全策略阻止。

## License

MIT License
