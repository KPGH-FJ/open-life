# OpenLife 小范围试用说明

> 当前文档是小范围试用前的操作说明，不是 full Beta 宣告。试用许可仍以当前代码门禁、Step6 真实 Tauri browser 报告和 readiness gate 为准。

## 当前状态

- Rust / frontend / build 门禁已可作为本地基础验证。
- 外部 live provider gate 需要显式 `OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1` 和真实外部 provider key；通过标准是 DirectAnswer、web AgentLoop、registered MCP AgentLoop、MCP ToolPermission proposal 四个场景都获得 final gate credit。
- Step6 Tauri WebDriver 产品验收仍必须用真实 Tauri browser journey 报告证明。当前 macOS runner 会 fail-closed 为 `tauri_webdriver_macos_not_supported_by_tauri_driver`，不能用单元测试或 local fixture 代替。

## 试用前必跑检查

```bash
cargo fmt --check
cargo test --workspace --lib
corepack pnpm --dir frontend build
corepack pnpm --dir frontend test
```

外部 live provider 检查：

```bash
OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1 \
OPENLIFE_LIVE_EVAL_PROVIDER=<provider> \
OPENLIFE_LIVE_EVAL_BASE=<base-url> \
OPENLIFE_LIVE_EVAL_MODEL=<model> \
OPENLIFE_LIVE_EVAL_API_KEY=<api-key> \
cargo test -p openlife-tauri --lib main_chat_final_acceptance_gate_runner_accepts_external_live_provider_when_opted_in -- --ignored --nocapture
```

Step6 真实产品验收：

```bash
OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1 \
OPENLIFE_LIVE_EVAL_PROVIDER=<provider> \
OPENLIFE_LIVE_EVAL_BASE=<base-url> \
OPENLIFE_LIVE_EVAL_MODEL=<model> \
OPENLIFE_LIVE_EVAL_API_KEY=<api-key> \
corepack pnpm --dir frontend test:e2e:tauri:step6
```

## 数据位置

OpenLife 使用 Tauri app data 目录保存本地数据。数据目录由 profile 决定：

- release: `ai.openlife.app`
- dev: `ai.openlife.app.dev`
- qa: `ai.openlife.app.qa`

可用环境变量覆盖：

```bash
OPENLIFE_PROFILE=qa
OPENLIFE_DATA_DIR=/path/to/isolated/openlife-qa
```

试用前必须使用独立 QA 数据目录，不要复用开发者真实数据目录。

## 会发送给云模型的内容

只有在 provider route 允许 cloud、network policy 允许、API key 存在、且请求不触发 LocalOnly / high privacy / critical privacy blocker 时，才会调用外部模型。

Main Chat live/provider-backed ReAct 路径会发送 bounded、metadata-safe 的上下文和工具候选 contract。它不应发送完整 LifeModel YAML、原始 Memory dump、API key、未脱敏 credential 或未选中的 `SKILL.md` 内容。

敏感或 LocalOnly 场景应 fail-closed 到本地/blocked 状态，不允许 cloud fallback。

## 永远需要确认的动作

以下动作不能通过普通聊天静默执行：

- durable LifeModel truth 更新；
- long-term Memory 写入、归档、恢复；
- workspace 文件写入；
- calendar/email/external/provider/plugin 状态变更；
- high-risk 或 confirmation-required MCP/tool action；
- snapshot restore / data import；
- dangerous shell 或高风险外部写入。

这些动作必须进入 Review Center proposal、ToolPermission proposal、governed import/restore request，或显式 blocker/confirmation path。

## 建议 QA 流程

使用独立数据目录：

```bash
export OPENLIFE_PROFILE=qa
export OPENLIFE_DATA_DIR="$(mktemp -d /tmp/openlife-qa.XXXXXX)"
```

最小 QA checklist：

1. 完成 onboarding，确认 `hasCompletedOnboarding` 持久化。
2. 配置 provider，运行连接测试，确认 UI 不把未验证 key 显示成 available。
3. 发送首轮普通聊天，确认 Main Chat 产生 task session / run / final delivery evidence。
4. 触发记忆或 LifeModel 更新请求，确认只创建 proposal，不静默写 durable truth。
5. 在 Review Center 分别执行 accept / reject / postpone / edit 路径。
6. 触发 Safe Mode 条件，确认写入口被禁用或转为只读提示。
7. 执行 export，确认导出成功且不会自动导入。
8. 执行 governed import / restore 前确认会创建 pre-change snapshot，并返回 metadata-safe audit。
9. 跑 Step6 Tauri WebDriver 产品验收，确认 `frontend/test-results/main-chat-step6-product-acceptance-report.json` 新鲜且 `acceptanceReady=true`。

## 已知限制

- 当前 macOS Tauri WebDriver Step6 runner 会 fail-closed，不能形成真实 Step6 browser journey credit。
- app-level Tauri capability 仍包含 recursive app fs、http 和 shell open 权限；runtime 有治理约束，但试用前需要确认没有 UI/IPC 绕过治理策略。
- `docs/BETA_USER_GUIDE.md` 是历史草稿，不代表当前 release authority。

## 备份、恢复和删除

- 备份：使用 Settings 的 export 数据导出，或备份整个独立 `OPENLIFE_DATA_DIR`。
- 恢复：只使用 governed import / restore 流程；必须带显式用户意图、pre-change snapshot 和 metadata-safe audit。
- 删除：关闭应用后删除对应 QA 数据目录。

## 试用准入标准

只有同时满足以下条件，才建议进入用户小范围试用：

- Rust / frontend / build 全绿；
- external live final gate 通过；
- Step6 Tauri browser report 真实运行且 `acceptanceReady=true`；
- QA profile checklist 有人工签收记录；
- Tauri capability audit 中 P0 项已解决或明确写入试用限制。
