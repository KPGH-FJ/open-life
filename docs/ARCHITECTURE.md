# OpenLife Architecture

> 5 分钟快速理解 OpenLife 技术架构

## 一句话定义

OpenLife 是一个**本地优先的个人 Agent 框架**，围绕用户私人 LifeModel 构建，支持本地/云端模型协同、工具调用、记忆管理和用户确认下的持续进化。

## 技术栈

| 层级 | 技术 | 职责 |
|------|------|------|
| 前端 UI | React 18 + TypeScript + Tailwind + Vite | 用户界面、状态展示、交互 |
| 桌面壳 | Tauri 2.x | 跨平台桌面窗口、系统 API 调用 |
| 后端核心 | Rust Workspace | 业务逻辑、模型调度、数据持久化 |
| 数据存储 | SQLite + YAML | 结构化数据 + 配置文件 |
| 本地模型 | Ollama | 本地 LLM 推理（localhost:11434） |
| 云端模型 | DeepSeek / OpenAI / OpenRouter | 云端 LLM API |

## 核心模块

```
用户输入
    │
    ▼
┌─────────────┐
│  AgentTask  │  ← 任务入口（Chat/Builder/Calibration/Proactive）
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  AgentRun   │  ← 可追踪执行记录（模型路由、上下文、输出、Proposal）
└──────┬──────┘
       │
       ├── ContextAssembler  ← 组装 LifeModel + 记忆 + 会话上下文
       ├── ModelRouter       ← 选择本地/云端模型路径
       ├── ReAct Engine      ← Reason → Act → Observe 循环
       ├── ActionExecutor    ← 内部动作 / MCP / A2A / LifeModel patch
       └── ProposalEngine    ← 生成待确认变更
       │
       ▼
用户 Review (/review)
       │
       ▼
LifeModel / Memory / Snapshot / Audit  ← 应用并持久化
```

### 关键实体

| 实体 | 职责 | 存储 |
|------|------|------|
| **LifeModel** | 用户私人上下文（身份/目标/能力/状态/偏好） | YAML + SQLite 快照 |
| **AgentRun** | 单次可追踪执行（模型选择、上下文、输出） | SQLite `agent_runs.db` |
| **AgentProposal** | 待确认的生命模型变更（before/after/risk） | SQLite `proposals.db` |
| **MemoryChunk** | 向量记忆（embedding + 访问计数 + tier） | SQLite `vectors.db` |
| **ChatSession** | 对话历史 | SQLite `messages.db` |

### 模块关系

```
AgentRun
  ├── 依赖: LifeModel（上下文）
  ├── 依赖: MemoryStore（记忆检索）
  ├── 依赖: InferenceScheduler（模型调度）
  ├── 依赖: VectorStore（向量检索）
  └── 产生: AgentProposal（变更建议）

AgentProposal
  ├── 来源: Builder（构建理解）
  ├── 来源: Calibration（校准建议）
  ├── 来源: Chat（对话建议，未来）
  └── 去向: ProposalStore → Review Center

LifeModel
  ├── 输入: Builder（构建）
  ├── 输入: Proposal（确认后应用）
  ├── 输入: Calibration（校准）
  └── 输出: ContextAssembler（上下文组装）
```

## 数据流

### 1. Chat 流

```
用户消息 → IntentRouter（分类意图）
              │
              ▼
         LayerRouter（选择层级 L1/L2/L3）
              │
              ▼
         ContextAssembler（LifeModel + Memory + 历史）
              │
              ▼
         InferenceScheduler（本地/云端模型选择）
              │
              ▼
         LLM 生成回复
              │
              ▼
         AgentRun 记录（模型路由、上下文摘要）
              │
              ▼
         流式输出到前端
```

### 2. Builder 流

```
用户启动 Builder → 回答引导问题
              │
              ▼
         BuilderEngine 生成 Signal（理解建议）
              │
              ▼
         BuilderPatchReview（用户审阅）
              │
              ├── "发送到 Review Center"（推荐）
              │       │
              │       ▼
              │   AgentProposal 创建
              │       │
              │       ▼
              │   ProposalStore 持久化
              │       │
              │       ▼
              │   AgentRun 关联记录
              │
              └── "直接应用"（legacy/migration/debug only）
                      │
                      ▼
                  LifeModel 直接更新
```

### 3. Proposal 确认流

```
Review Center (/review)
    │
    ├── 查看 Proposal（分类/风险筛选）
    ├── 编辑 after 值
    ├── 接受 → LifeModel 更新 + Snapshot + Audit
    ├── 拒绝 → 记录拒绝原因
    └── 稍后 → 保持 pending
```

## 目录结构

```
.
├── frontend/                    # React 前端
│   ├── src/
│   │   ├── pages/               # 页面（Chat/Dashboard/Builder/Review Center）
│   │   ├── components/          # 通用组件
│   │   ├── tauri.ts             # Tauri Command 封装（所有后端调用入口）
│   │   └── App.tsx              # 路由 + 导航
│   └── package.json             # 脚本: dev/build/test
│
├── openlife-core/               # Rust 核心业务库
│   └── src/
│       ├── agent/               # AgentRun + AgentProposal + Store
│       ├── life_model.rs        # LifeModel 定义与操作
│       ├── builder.rs           # Builder 引擎
│       ├── hermes.rs            # 三层决策总线
│       ├── scheduler.rs         # 模型调度器
│       ├── llm.rs / ollama.rs   # 云端/本地模型调用
│       ├── memory.rs            # SQLite 消息/会话存储
│       ├── vectors.rs           # 向量记忆
│       ├── mcp.rs / a2a.rs      # 工具与外部 Agent
│       ├── privacy.rs           # PII 检测与脱敏
│       └── versioning.rs        # 快照与回滚
│
├── src-tauri/                   # Tauri 桌面壳与命令层
│   └── src/
│       ├── lib.rs               # 核心状态 + 聊天主链路
│       ├── commands/            # 按领域拆分的 Tauri commands
│       │   ├── chat.rs          # 6 个 Chat 命令
│       │   ├── builder.rs       # 11 个 Builder 命令
│       │   ├── proposal.rs      # 7 个 Proposal 命令
│       │   ├── calibration.rs   # 6 个 Calibration 命令
│       │   └── ...              # 其他领域命令
│       └── bin/
│           └── a2a_server.rs    # 独立 A2A HTTP 服务器
│
├── docs/                        # 项目文档
│   ├── ARCHITECTURE.md          # ← 本文档（精简版）
│   ├── ARCHITECTUREDETAILED.md  # 深度架构文档
│   ├── CONTRIBUTING.md          # 开发指南
│   ├── decisions/               # ADR 决策记录
│   └── api/                     # API 文档
│
├── plans/                       # 架构与开发计划
│   ├── openlife_agent_framework_architecture.md
│   └── openlife_development_plan.md
│
└── smoke.sh                     # 端到端验证脚本
```

## 新增模块规范

任何新功能必须能挂到以下概念之一：

| 概念 | 说明 | 示例 |
|------|------|------|
| `AgentTask` | 任务入口 | Chat 对话、Builder 构建、Calibration 校准 |
| `AgentRun` | 可追踪执行 | 记录模型选择、上下文、输出、错误 |
| `AgentProposal` | 待确认变更 | LifeModel 更新、Memory 更新、Tool 权限 |
| `LifeModel` | 私人上下文 | 身份、目标、能力、状态、偏好 |
| `Memory` | 长期记忆 | 消息历史、向量记忆、访问计数 |
| `ModelRouter` | 模型调度 | 本地优先、隐私感知、成本优化 |
| `Workspace` | 工作空间 | Dashboard、Review Center、Settings |

### 禁止

- ❌ 新增孤立页面（不挂到 AgentTask/Run/Proposal 体系）
- ❌ 绕过 Proposal/Confirmation 直接修改 LifeModel
- ❌ 在 Safe Mode 下执行高风险写入操作

## 快速参考

### 启动命令

```bash
make setup    # 初始化环境
make dev      # 启动开发服务器
make test     # 运行所有测试
make build    # 构建生产版本
```

### 测试命令

```bash
cargo test -p openlife-core          # Rust 核心测试
cargo test -p openlife-tauri         # Tauri 层测试
cd frontend && npm test              # 前端测试
./scripts/smoke.sh                   # 端到端验证
```

### 关键文件

| 文件 | 说明 |
|------|------|
| `frontend/src/tauri.ts` | 所有后端调用的唯一入口 |
| `openlife-core/src/agent/types.rs` | AgentRun/AgentProposal 类型定义 |
| `src-tauri/src/lib.rs` | 核心状态与聊天主链路 |
| `AGENTS.md` | 面向 AI Agent 的完整上下文指南 |
| `CONTRIBUTING.md` | 开发指南与 PR 流程 |

## 了解更多

- **深度架构**: [ARCHITECTUREDETAILED.md](./ARCHITECTUREDETAILED.md)
- **开发计划**: [plans/openlife_development_plan.md](../plans/openlife_development_plan.md)
- **决策记录**: [decisions/](./decisions/)
- **API 文档**: [api/](./api/)
