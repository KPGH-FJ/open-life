# Contributing to OpenLife

感谢你对 OpenLife 的兴趣！本文档将帮助你快速搭建开发环境并参与贡献。

OpenLife 是本地优先的个人 Agent 框架。贡献不仅要让功能能跑起来，也要保护用户的 LifeModel、memory、privacy、permission 和 auditability。所有重要变更都应该可 review、可测试、可回滚、可治理。

## 环境要求

| 工具 | 最低版本 | 说明 |
|------|---------|------|
| Rust | 1.75+ | 后端核心语言 |
| Node.js | 18+ | 前端运行时 |
| npm | 9+ | 包管理器 |
| pnpm | 9.1.0 | 推荐使用（已在 package.json 中声明） |
| Tauri CLI | 2.x | 桌面应用框架 |

### macOS 快速安装

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js (通过 Homebrew)
brew install node

# pnpm
npm install -g pnpm

# Tauri 系统依赖 (macOS)
brew install openssl@3
```

## 快速启动

```bash
# 1. 克隆仓库
git clone https://github.com/KPGH-FJ/open-life.git
cd open-life

# 2. 初始化环境
make setup
# 或手动:
# cd frontend && pnpm install

# 3. 启动开发服务器
make dev
# 或手动:
# ./scripts/dev.sh
```

开发服务器启动后：

- **前端开发服务器**: http://localhost:5173
- **Tauri 桌面窗口**: 自动弹出
- **A2A 服务**: http://localhost:8765

## 项目结构速览

```text
.
├── frontend/          # React 18 + TypeScript + Tailwind + Vite
├── openlife-core/     # Rust 核心业务库
├── src-tauri/         # Tauri 桌面壳与命令层
├── plans/             # 架构文档与开发计划
└── docs/              # 项目文档
```

核心模块：

- `LifeModel`: 用户私人上下文（身份/目标/能力/状态）
- `AgentRun`: 可追踪的任务执行记录
- `AgentProposal`: 待确认的生命模型变更
- `Memory`: 消息、会话、向量记忆的 SQLite 存储
- `AgentRuntime`: ReAct / tool / proposal / audit 的执行核心

## 分支策略

采用简化 Git Flow：

```text
main        ← 稳定默认分支，质量最高
  ↑
dev         ← 集成分支，feature / Codex PR 先合并到这里
  ↑
feature/*   ← 人类开发分支
codex/*     ← Agent / Codex 开发分支
```

### 分支命名规范

| 类型 | 格式 | 示例 |
|------|------|------|
| Codex | `codex/范围` | `codex/lmhs-1-evidence-store` |
| 功能 | `feature/功能描述` | `feature/proposal-review-center` |
| 修复 | `fix/问题描述` | `fix/builder-session-cleanup` |
| 重构 | `refactor/范围` | `refactor/agent-run-types` |
| 文档 | `docs/范围` | `docs/architecture-update` |

正常流程：

```text
issue -> plan -> branch -> implementation -> tests -> PR -> review -> dev -> main
```

LifeModel-HS 工作应通过 focused PR 先合入 `dev`。

## 提交规范

采用 [Conventional Commits](https://www.conventionalcommits.org/)：

```text
<type>(<scope>): <subject>

<body>

<footer>
```

### 类型说明

| 类型 | 说明 | 示例 |
|------|------|------|
| `feat` | 新功能 | `feat(proposal): 添加批量接受低风险 Proposal` |
| `fix` | 修复 | `fix(builder): 修复 session 清理失败时丢失数据` |
| `refactor` | 重构 | `refactor(store): 统一 AgentRun 和 Proposal 存储接口` |
| `docs` | 文档 | `docs(adr): 添加 Proposal 统一层决策记录` |
| `test` | 测试 | `test(proposal): 添加 Safe Mode 边界测试` |
| `chore` | 杂项 | `chore(ci): 添加 GitHub Actions 工作流` |

### 提交示例

```bash
feat(agent): AgentRun 与 Proposal 双向关联

- AgentProposal 新增 source_run_id 和 source_kind 字段
- AgentRun 新增 generated_proposals 数组
- Builder/Calibration 创建 Proposal 时自动关联 AgentRun

Closes #123
```

## Issue 流程

使用 GitHub Issue Forms：

- `LifeModel-HS Epic`: LifeModel-HS MVP 父 issue。
- `LifeModel-HS Task`: 严格对应 `plans/lifemodel_hs_mvp_task_specs.md` 中一个 LMHS task。
- `Engineering Task`: 非 LMHS 的边界清晰工程任务。
- `Bug Report`: 可复现 bug 或 regression。

不要用 blank issue 承载结构化开发任务。

LifeModel-HS issue 必须包含：

- parent Epic，
- selected LMHS task，
- required reading，
- Codex Working Mode，
- expected behavior，
- allowed edit areas，
- non-goals，
- out-of-scope failure examples，
- verification commands。

## Codex Working Mode

非平凡 issue 的第一轮 Codex pass 默认应为 plan-only，除非 issue 明确写明 implementation approved。

Before editing:

1. Read the required documents and relevant code.
2. Summarize current behavior.
3. Propose the smallest safe implementation plan.
4. Identify risky assumptions.
5. Wait for review when the issue requires plan-only first pass.

During implementation:

- Stay within allowed edit areas.
- Keep changes additive unless the issue explicitly allows migration.
- Avoid unrelated refactors.
- Add focused tests.
- Do not introduce production dependencies without clear justification.

Before final response or PR:

- Summarize changed files.
- Summarize tests run.
- List remaining risks and follow-ups.
- Call out out-of-scope work intentionally avoided.

## LifeModel-HS Governance

LifeModel-HS work must follow:

- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `plans/lifemodel_hs_mvp_task_specs.md`
- `plans/lifemodel_hs_architecture_plan.md`

Hard rules:

- Current YAML LifeModel remains a compatibility materialized view during MVP.
- Do not switch source of truth in one step.
- Privacy is hard Policy, not soft Heuristic.
- Heuristics cannot relax Policy.
- Raw Life Data cannot directly mutate canonical HS assets.
- Risky HS mutation is Proposal-first.
- High-risk identity, values, mission, long-term goals, sensitive relationships, and privacy boundaries require explicit user confirmation.
- Runtime selection audit must be metadata-safe.
- Deterministic regression comes before broad runtime integration.
- Preserve PromptStack, ModelRouter, ToolRuntime, ExecutionFacade, Proposal, and AgentRunEvent governance.

## PR 流程

### feature / codex → dev（快速通道）

1. **创建分支**: `git checkout -b codex/xxx dev` 或 `git checkout -b feature/xxx dev`
2. **开发前计划**: 对非平凡任务，先在 issue 中完成 implementation plan review
3. **开发**: 编写代码 + 测试
4. **本地验证**:

   ```bash
   cargo test
   cargo clippy --all -- -D warnings
   cargo fmt --all -- --check
   pnpm --dir frontend test
   pnpm --dir frontend build
   ```

5. **提交**: 遵循 Conventional Commits
6. **推送**: `git push origin codex/xxx`
7. **创建 PR**: 目标分支选择 `dev`
8. **填写 PR 模板**: 说明 issue、范围、治理检查、测试与剩余风险
9. **等待 CI**: 自动运行 Ubuntu 快速检查
10. **合并**: Squash merge，保持 `dev` 分支整洁

### dev → main（严格模式）

1. **创建 PR**: 从 `dev` 到 `main`
2. **全平台 CI**: 自动运行 Ubuntu + macOS + Windows 相关检查
3. **代码审查**: 至少 1 人 review
4. **合并**: 使用 Merge commit 保留集成历史

Review should reject PRs that:

- broaden runtime authority without an accepted ADR,
- bypass Proposal-first governance,
- store raw sensitive data unnecessarily,
- relax privacy policy through heuristics,
- switch LifeModel source of truth in one step,
- implement adjacent LMHS tasks without explicit scope,
- perform broad unrelated refactors.

GitHub labels、milestones、branch protection 和 LifeModel-HS issue flow 见
`docs/github_repository_governance.md`。

## 代码规范

### Rust

- **格式化**: `cargo fmt`（配置见 `.rustfmt.toml`）
- **Lint**: `cargo clippy -- -D warnings`
- **测试**: `cargo test -p openlife-core` + `cargo test -p openlife-tauri`
- **命名**: snake_case 文件/函数，PascalCase 结构体

### TypeScript / React

- **格式化**: Prettier（配置见 `.prettierrc.json`）
- **Lint**: TypeScript 严格模式（`tsconfig.json`）
- **测试**: Vitest（`npm test`）
- **命名**: PascalCase 组件/类型，camelCase 函数

## 开发检查清单

提交 PR 前请确认：

- [ ] `cargo test` 全部通过
- [ ] `cargo clippy --all -- -D warnings` 零警告
- [ ] `cargo fmt --all -- --check` 通过
- [ ] `pnpm --dir frontend test` 全部通过
- [ ] `pnpm --dir frontend build` 成功
- [ ] 新增代码包含测试
- [ ] 不暴露 secrets 或 API keys
- [ ] 不暴露 raw LifeModel、raw memory、raw private files 或 raw sensitive chat
- [ ] 文档已更新（如需要）

整包门控：

```bash
make ci
```

## Security And Privacy

Do not put these in issues, PR descriptions, logs, screenshots, test fixtures, or audit records:

- API keys,
- raw LifeModel content,
- raw memory,
- raw private files,
- raw sensitive chat,
- complete prompts containing private context,
- full model outputs containing private context.

Prefer source references, digests, redacted summaries, and metadata-only audit records.

## 常见问题

### Q: 前端测试失败？

检查是否更新了 Tauri mock：

```bash
# 新增 command 时必须同步更新
frontend/src/test/mocks/tauri.ts
```

### Q: Rust 编译失败？

常见原因：

- reqwest 版本不一致（openlife-core 0.11 vs src-tauri 0.12）
- 缺少系统依赖（macOS: `brew install openssl@3`）

### Q: 数据目录不一致？

统一数据目录：`~/Library/Application Support/ai.openlife.desktop/`
如果你曾使用旧版本数据目录 `com.openlife.app` 或 `ai.openlife.app`，请将其中的数据手动复制到 `ai.openlife.desktop`。

## 资源

- [架构文档](./docs/ARCHITECTURE.md)
- [详细架构](./docs/ARCHITECTUREDETAILED.md)
- [GitHub Repository Governance](./docs/github_repository_governance.md)
- [开发计划](./plans/openlife_development_plan.md)
- [API 文档](./docs/api/)
- [决策记录](./docs/decisions/)

## 需要帮助？

- 提交 Issue: https://github.com/KPGH-FJ/open-life/issues
- 查看 AGENTS.md 获取项目完整上下文
