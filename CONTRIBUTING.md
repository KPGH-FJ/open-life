# Contributing to OpenLife

感谢你对 OpenLife 的兴趣！本文档将帮助你快速搭建开发环境并参与贡献。

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

```
.
├── frontend/          # React 18 + TypeScript + Tailwind + Vite
├── openlife-core/     # Rust 核心业务库
├── src-tauri/         # Tauri 桌面壳与命令层
├── plans/             # 架构文档与开发计划
└── docs/              # 项目文档（本文档所在目录）
```

核心模块：
- `LifeModel`: 用户私人上下文（身份/目标/能力/状态）
- `AgentRun`: 可追踪的任务执行记录
- `AgentProposal`: 待确认的生命模型变更
- `Memory`: 消息、会话、向量记忆的 SQLite 存储
- `Hermes`: 三层决策总线（Meaning→Strategy→Execution）

## 分支策略

采用简化 Git Flow：

```
main    ← 发布分支，稳定性最高
  ↑
dev     ← 集成分支，所有 feature 先合并到这里
  ↑
feature/* ← 单个 task 的开发分支
```

### 分支命名规范

| 类型 | 格式 | 示例 |
|------|------|------|
| 功能 | `feature/功能描述` | `feature/proposal-review-center` |
| 修复 | `fix/问题描述` | `fix/builder-session-cleanup` |
| 重构 | `refactor/范围` | `refactor/agent-run-types` |
| 文档 | `docs/范围` | `docs/architecture-update` |

## 提交规范

采用 [Conventional Commits](https://www.conventionalcommits.org/)：

```
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

## PR 流程

### feature → dev（快速通道）

1. **创建分支**: `git checkout -b feature/xxx dev`
2. **开发**: 编写代码 + 测试
3. **本地验证**:
   ```bash
   cargo test          # Rust 测试
   cargo clippy        # Lint 检查
   cargo fmt           # 格式化
   cd frontend && npm test   # 前端测试
   ```
4. **提交**: 遵循 Conventional Commits
5. **推送**: `git push origin feature/xxx`
6. **创建 PR**: 目标分支选择 `dev`
7. **等待 CI**: 自动运行 Ubuntu 快速检查
8. **合并**: Squash merge，保持 dev 分支整洁

### dev → main（严格模式）

1. **创建 PR**: 从 `dev` 到 `main`
2. **全平台 CI**: 自动运行 Ubuntu + macOS + Windows
3. **代码审查**: 至少 1 人 review
4. **合并**: 使用 Merge commit 保留历史

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
- [ ] `cargo clippy` 零警告
- [ ] `cargo fmt` 格式化完成
- [ ] `npm test`（前端）全部通过
- [ ] `npm run build`（前端）成功
- [ ] 新增代码包含测试
- [ ] 不暴露 secrets 或 API keys
- [ ] 文档已更新（如需要）

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

统一数据目录：`~/Library/Application Support/ai.openlife.app/`
旧版本数据在 `com.openlife.app`，如需迁移请手动复制。

## 资源

- [架构文档](./ARCHITECTURE.md)
- [详细架构](./ARCHITECTUREDETAILED.md)
- [开发计划](../plans/openlife_development_plan.md)
- [API 文档](./api/)
- [决策记录](./decisions/)

## 需要帮助？

- 提交 Issue: https://github.com/KPGH-FJ/open-life/issues
- 查看 AGENTS.md 获取项目完整上下文
