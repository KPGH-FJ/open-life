# OpenLife

OpenLife 是你的**终身成长合伙人**，一款基于 Tauri + React + Rust 的桌面端 AI 伴侣应用。它以你的人生模型（Life Model）为核心，通过价值观对齐的对话、三层决策架构（Hermes）、多协议生态集成（MCP / A2A）和持续进化系统，陪伴你实现长期目标。

## 核心特性

- **四维人生模型**：Identity（身份认同）、Goals（目标体系）、Capabilities（能力资源）、State（当前状态）。
- **三层对话引擎**：Reflex 意图路由 → Tactical 本地模型 / Strategic 云端大模型 → 语义记忆注入。
- **Hermes 三层决策总线**：Meaning / Strategy / Execution 的 JSON-RPC 2.0 风格进程内协议，带冲突仲裁器。
- **双协议生态桥接**：MCP 工具调用 + A2A Agent 互联（客户端发现 / 服务端暴露价值观评估）。
- **向量记忆 Tier 3**：基于 Embedding 的语义检索，自动注入对话上下文。
- **版本控制**：Git-like 人生模型快照，支持一键回滚与差异对比。
- **构建模式**：快速构建（60 分钟）、渐进构建、苏格拉底式峰值体验挖掘。
- **隐私感知**：PII 检测与脱敏上云，核心价值观本地拦截。

## 技术栈

| 层级 | 技术 |
|------|------|
| 前端 | React 18 + TypeScript + Tailwind CSS + Vite |
| 桌面壳 | Tauri 2.0 |
| 后端核心 | Rust Workspace (`openlife-core` + `openlife-tauri`) |
| 本地模型 | Ollama (`qwen2.5:7b` 等) |
| 云端模型 | OpenRouter API (`claude-3.5-sonnet` 等) |
| 数据存储 | SQLite（消息、反馈、向量记忆）+ YAML（人生模型文件） |

## 快速开始

### 前置要求

- [Rust](https://rustup.rs/)（>= 1.75）
- [Node.js](https://nodejs.org/) 18+
- [pnpm](https://pnpm.io/)
- （可选）[Ollama](https://ollama.com/) 本地运行 `qwen2.5:7b`

### 安装依赖

```bash
cd frontend && pnpm install
cd ..
```

### 配置云端 API Key（DeepSeek 优先）

桌面端现在优先支持 DeepSeek 试用路径。打开应用后进入 **Settings → LLM 配置**，选择 Provider 并点击“测试”，测试通过后再保存。

```bash
# DeepSeek（推荐试用）
export DEEPSEEK_API_KEY="sk-..."

# 或 OpenRouter
export OPENROUTER_API_KEY="sk-..."

# 或 OpenAI
export OPENAI_API_KEY="sk-..."
```

DeepSeek 默认配置为 `https://api.deepseek.com` + `deepseek-chat`，并默认关闭远端 embedding。embedding 会优先走本地 Ollama，失败时退回确定性 hash fallback，避免因为 embedding 服务不可用拖垮聊天主链路。

### 开发运行

```bash
pnpm tauri dev
```

### 构建安装包

```bash
# macOS
pnpm tauri build --target universal-apple-darwin

# Windows
pnpm tauri build --target x86_64-pc-windows-msvc

# Linux
pnpm tauri build --target x86_64-unknown-linux-gnu
```

构建产物位于 `src-tauri/target/release/bundle/`。

## 项目结构

```
.
├── frontend/               # React + Vite 前端
│   └── src/
│       ├── pages/          # 页面组件（Chat / Dashboard / Builder / A2A ...）
│       ├── App.tsx
│       └── tauri.ts        # Tauri Command 封装
├── openlife-core/          # Rust 核心业务逻辑库
│   └── src/
│       ├── life_model.rs   # 人生模型数据结构
│       ├── llm.rs          # LLM 调度（OpenRouter / Ollama）
│       ├── scheduler.rs    # 模型调度器
│       ├── memory.rs       # SQLite 消息与快照
│       ├── vectors.rs      # 向量记忆（Tier 3）
│       ├── hermes.rs       # Hermes 三层决策总线
│       ├── a2a.rs          # A2A 协议适配
│       ├── builder.rs      # 构建模式与苏格拉底对话
│       ├── router.rs       # 意图路由（Layer 1）
│       ├── privacy.rs      # 隐私检测与脱敏
│       ├── versioning.rs   # 版本控制
│       └── feedback.rs     # 反馈与微进化
├── src-tauri/              # Tauri 入口与命令注册
│   ├── src/lib.rs
│   └── tauri.conf.json
├── OpenLife_Final_PRD.md   # 产品需求文档
└── README.md
```

## 当前文档入口

- 长期愿景与完整需求: [OpenLife_Final_PRD.md](/Users/fujing/Desktop/偶来福/OpenLife_Final_PRD.md)
- 当前阶段主开发计划: [plans/openlife_development_plan.md](/Users/fujing/Desktop/偶来福/plans/openlife_development_plan.md)
- Codex 持续执行手册: [plans/openlife_codex_execution_playbook.md](/Users/fujing/Desktop/偶来福/plans/openlife_codex_execution_playbook.md)

当前建议的阅读顺序是：

1. 先看 `README`
2. 再看 `plans/openlife_development_plan.md`
3. 需要理解长期目标时再看 `OpenLife_Final_PRD.md`

## 试用前检查

进入桌面端后，建议先走这条最短路径：

1. 打开 **Settings**，确认“试用就绪检查”。
2. 如果使用 DeepSeek，选择 `DeepSeek` Provider，填入 API Key，点击“测试”，成功后保存。
3. 如果使用本地模型，先启动 Ollama，并确认本地模型名称能被解析，例如 `qwen2.5:7b` 或 `llama3:latest`。
4. 打开 **构建**，完成一次“快速构建”，在 Review 中确认要写入的人生模型字段。
5. 打开 **对话**，发送一条普通消息；刷新后确认 user 和 assistant 历史都还在。

常见错误修复：

- `start_stream_message missing required key args`：说明前后端构建版本可能不一致。请重新运行 `cd frontend && npm run build`，再重新启动 Tauri；当前版本已兼容 `args` 包裹参数和顶层参数两种形态。
- `401 / 403 / invalid API Key`：去 Settings 重新测试当前表单内容，确认 Provider、Base URL、模型名和 API Key 匹配。
- `Ollama connection refused / 11434`：启动 Ollama，或切换到 DeepSeek/OpenAI-compatible 云端模式。
- 数据库 schema 错误：Settings 诊断区会显示 active data dir、legacy data dir、database status 和 startup warnings；优先备份数据目录后再做清理。

## 主要页面说明

| 页面 | 说明 |
|------|------|
| **仪表盘** (`/dashboard`) | 目标进度、能力雷达图、语义记忆检索 |
| **人生模型** (`/`) | 表单式四维编辑 |
| **对话** (`/chat`) | 带 Hermes 思考过程展示、点赞/点踩反馈 |
| **版本控制** (`/versions`) | 快照列表、回滚、差异对比 |
| **记忆检索** (`/memory`) | 向量记忆的语义搜索与手动索引 |
| **A2A** (`/a2a`) | 发现外部 Agent、发送 Task、测试本地服务 |
| **构建** (`/builder`) | 快速构建 / 渐进构建 / 苏格拉底对话 |

## 许可

MIT License
