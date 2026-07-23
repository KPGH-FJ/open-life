# OpenLife 小范围试用说明

> Last fact sync: 2026-07-23
>
> Status: historical limited-trial entry retired during restart-baseline cleanup
>
> Current authority: `AGENTS.md`, `plans/README.md`, the Phase7 deletion
> manifest, and the single-system development preparation

## 当前结论

OpenLife 目前仍是 `red-until-trial-green`，没有一条可以据此开放小范围
用户试用的当前准入路径。本文件过去记录的 Stage1、Step6、retired final
gate 和外部 Provider 命令只属于历史证据；对应 npm 入口已经退出默认开发
流程，不应恢复或继续执行。

本轮只建立事实与干净开发基线：不调用外部 Provider，不批准真实 durable
write，不读取或删除 Keychain，不修改 release/dev/QA 产品数据。测试证据等级
与当前命令以 `docs/development/testing.md` 为准。

## 本轮允许的安全验证

```bash
git diff --check
cargo fmt --check
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend test
corepack pnpm --dir frontend build
corepack pnpm --dir frontend test:e2e
```

其中 Playwright 只验证 Vite + Chromium 的 Workbench browser shell：六个
规范路由、旧/未知路由不可用状态和无未捕获 JavaScript 错误。它不提供
Tauri、迁移、真实产品数据、durable write 或 external-live 信用。

cleanup PR 合入后，还必须从最终 `main` 启动真实 Tauri 并重新验证
`/settings`。该验证只观察启动、渲染与 fail-closed 状态，不测试 Provider，
不尝试 credential recovery，也不保存配置。

## 数据与凭据边界

OpenLife 的默认产品数据目录由 profile 区分：

- release: `~/Library/Application Support/ai.openlife.app`
- dev: `~/Library/Application Support/ai.openlife.app.dev`
- qa: `~/Library/Application Support/ai.openlife.app.qa`
- custom: 由 `OPENLIFE_DATA_DIR` 指定，当前路径视为 `UNKNOWN`

restart cleanup 保留上述目录、任何 legacy-looking QA 目录、`.env`、
`.env.live.local`、`frontend/test-results` 和 Phase4F 截图。不得为了“清干净”
而删除、迁移或检查其中内容。

## 永远需要显式授权的动作

普通聊天文本不是写入授权。下列动作必须进入当前治理合同规定的 proposal、
confirmation 或 blocker 流程：

- durable LifeModel truth 或长期 Memory 更新；
- workspace 文件、calendar、email、external provider 或 plugin 写入；
- high-risk tool、MCP 或 shell 动作；
- snapshot restore、数据导入或删除；
- credential recovery、Keychain 变更或真实配置保存。

创建 proposal 不等于 durable change 已完成；blocked、pending 或缺失证据也
不能显示为 completed。

## 重新开放试用的前置条件

restart baseline 冻结后，下一轮先做正式全仓 Review。只有 Review 重新定义
当前 trial contract、独立验证需要的 native/external-live 证据，并由人类明确
批准后，才应编写新的试用步骤。本文件不提前决定该路线，也不继承历史
Step6 报告的 readiness。
