# OpenLife 开发者接手指南

> 本文档面向新加入的开发者，帮助你快速理解项目结构、开发流程与关键约定。

---

## 1. 项目结构速览

```
.
├── Cargo.toml                  # Rust Workspace 定义
├── frontend/                   # React 18 + Vite 前端
│   ├── src/
│   │   ├── App.tsx             # 路由与导航
│   │   ├── tauri.ts            # ⭐ 所有后端调用的唯一入口
│   │   ├── test/mocks/tauri.ts # Tauri invoke mock（新增 command 必须同步）
│   │   ├── components/         # 通用组件
│   │   └── pages/              # 页面组件
│   └── package.json
├── openlife-core/              # Rust 核心业务库
│   └── src/
│       ├── lib.rs              # 模块暴露
│       ├── life_model.rs       # 四维人生模型
│       ├── hermes.rs           # 三层决策总线
│       ├── scheduler.rs        # 模型调度器
│       ├── memory.rs           # SQLite 持久化
│       ├── vectors.rs          # 向量记忆 Tier 3
│       ├── mcp.rs              # MCP 客户端
│       ├── feedback.rs         # 反馈与事件存储
│       └── ...
└── src-tauri/                  # Tauri 桌面壳
    ├── src/
    │   ├── lib.rs              # ⭐ Command 注册地（30+ 命令）
    │   ├── main.rs             # 桌面应用入口
    │   └── bin/a2a_server.rs   # 独立 A2A HTTP 服务器
    └── tauri.conf.json
```

### 关键文件对照表

| 功能 | Rust 文件 | TS 文件 | Mock 文件 |
|------|-----------|---------|-----------|
| 新增后端命令 | `src-tauri/src/lib.rs` | `frontend/src/tauri.ts` | `frontend/src/test/mocks/tauri.ts` |
| 人生模型逻辑 | `openlife-core/src/life_model.rs` | `frontend/src/types.ts` | — |
| 记忆检索 | `openlife-core/src/vectors.rs` | `frontend/src/pages/MemorySearch.tsx` | — |
| 错误展示 | — | `frontend/src/components/ErrorBanner.tsx` | `frontend/src/components/ErrorBanner.test.tsx` |

---

## 2. 开发环境搭建

```bash
# 1. 一键初始化（macOS/Linux）
make setup
# 或 Windows
.\setup.ps1

# 2. 启动开发模式（同时启动前端 Vite + Tauri dev）
make dev
# 或
./dev.sh

# 3. 运行测试
make test          # 全部测试
make test-front    # 仅前端
make test-rust     # 仅 Rust
```

### 前置依赖

- Rust >= 1.75
- Node.js 18+
- pnpm（推荐）或 npm
- （可选）Ollama（本地模型调试）

---

## 3. 前后端通信约定

### 3.1 新增一个 Tauri Command 的完整流程

假设你要新增 `get_foo` / `set_foo` 命令：

**Step 1: Rust 后端** — `src-tauri/src/lib.rs`

```rust
#[tauri::command]
async fn get_foo(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    // 实现逻辑
    Ok("foo".into())
}

#[tauri::command]
async fn set_foo(value: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    // 实现逻辑
    Ok(())
}
```

**Step 2: 注册命令** — 在 `lib.rs` 底部 `generate_handler!` 宏中添加：

```rust
generate_handler![
    // ... 已有命令
    get_foo,
    set_foo,
]
```

**Step 3: 前端 API 封装** — `frontend/src/tauri.ts`

```typescript
export async function getFoo(): Promise<string> {
  return safeInvoke<string>("get_foo");
}

export async function setFoo(value: string): Promise<void> {
  return safeInvoke("set_foo", { value });
}
```

**Step 4: 更新 Mock** — `frontend/src/test/mocks/tauri.ts`

```typescript
case 'get_foo':
  return Promise.resolve('mock-foo' as T)
case 'set_foo':
  return Promise.resolve(undefined as T)
```

**Step 5: 编写测试**

任何用到新命令的组件测试都必须先完成 Step 4，否则 `invoke` 会返回 `{}`，可能导致组件渲染异常或断言失败。

---

## 4. 核心开发规范

### 4.1 命名约定

| 范畴 | 约定 | 示例 |
|------|------|------|
| Rust 文件/函数 | `snake_case` | `life_model.rs`, `save_message()` |
| Rust 结构体/枚举 | `PascalCase` | `LifeModel`, `HermesBus` |
| TS 组件文件 | `PascalCase` | `ChatPage.tsx`, `ErrorBanner.tsx` |
| TS 函数/变量 | `camelCase` | `sendMessage()`, `exportAllData()` |
| TS 接口/类型 | `PascalCase` | `AppConfig`, `ChatMessage` |
| 数据库表名 | `snake_case` 复数 | `messages`, `chat_sessions` |

### 4.2 代码风格

- **Rust**: 4 空格缩进，双引号，约 100-120 字符行宽
- **TypeScript**: 2 空格缩进，双引号，`strict: true`
- **Tailwind**: 工具类为主，无自定义 BEM

### 4.3 错误处理

- 前端禁止直接使用 `alert()`，统一使用 `ErrorBanner` 组件
- 后端命令统一返回 `Result<T, String>`，错误信息使用中文
- 所有 `safeInvoke` 调用必须处理 `catch`（至少记录日志）

### 4.4 HashRouter 强制

Tauri 桌面应用基于 `file://` 协议，前端必须使用 `HashRouter`。如果使用 `BrowserRouter` 会导致白屏。

---

## 5. 测试规范

### 5.1 前端测试

- **框架**: Vitest + jsdom + @testing-library/react
- **Mock 完整性**: 新增 `invoke` 命令必须同步更新 `frontend/src/test/mocks/tauri.ts`
- **覆盖率**: 关键页面（Chat、Builder、Settings）至少覆盖主流程渲染和交互

### 5.2 Rust 测试

- 使用 `#[cfg(test)]` 模块
- 使用 `tempfile::TempDir` 创建隔离的测试数据库
- 向量存储、配置管理、记忆存储已有较好的测试覆盖，新增模块应参照其模式

### 5.3 测试运行

```bash
cd frontend && npx vitest run        # 前端测试
cargo test -p openlife-core          # Rust 核心测试
cargo test -p openlife-tauri         # Tauri 测试
```

---

## 6. 常见问题排查

### 6.1 `pnpm` 未找到

项目使用 pnpm 作为包管理器。如果没有安装：

```bash
npm install -g pnpm
```

如果只想用 npm，将 `Makefile` 和脚本中的 `pnpm` 替换为 `npm` 即可。

### 6.2 `.env` 修改后未生效

Tauri dev 不会热重载 `.env` 变更。修改 API Key 后需要重启开发服务器。

### 6.3 Ollama 检测不到

`ollama.rs` 中缓存 TTL 固定为 10 秒。如果刚启动 Ollama，等待缓存过期或重启应用。

### 6.4 前端测试因 mock 缺失失败

错误特征：`invoke` 返回 `{}`，导致组件状态异常。

解决：在 `frontend/src/test/mocks/tauri.ts` 的 `switch` 语句中添加对应的 `case`。

### 6.5 reqwest 版本冲突

`openlife-core` 使用 `reqwest 0.11`，`src-tauri` 使用 `reqwest 0.12`。目前编译通过，但建议逐步统一。

---

## 7. 提交规范（建议）

采用 [Conventional Commits](https://www.conventionalcommits.org/) 风格：

```
feat: 新增首次启动向导
fix: 修复导入时版本号校验错误
refactor: 统一错误提示为 ErrorBanner 组件
docs: 更新用户试用指南
test: 补充 OnboardingWizard 单元测试
```

---

## 8. 进一步了解

| 文档 | 内容 |
|------|------|
| [`AGENTS.md`](../AGENTS.md) | 完整的项目架构、数据流、业务规则、已知问题 |
| [`plans/openlife_development_plan.md`](../plans/openlife_development_plan.md) | 开发路线图与里程碑规划 |
| [`README.md`](../README.md) | 项目简介与快速开始 |
| [`docs/BETA_USER_GUIDE.md`](./BETA_USER_GUIDE.md) | 面向非技术用户的试用指南 |

---

欢迎加入 OpenLife 开发！如有疑问，先查阅 `AGENTS.md` 的「已知问题和注意事项」章节。
