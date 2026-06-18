# Main Chat Stage 1 Readiness Gate Contract

> Date: 2026-06-18
> Scope: single readiness report for Stage 1 dogfood
> Status: preparation artifact

## 1. Target Command And Report

Target command:

```text
run_main_chat_agent_stage1_dogfood_gate
```

Target report:

```text
MainChatAgentStage1DogfoodReport
```

The report must run in an isolated eval state unless explicitly configured for
manual or live dogfood.

## 2. Aggregated Inputs

The report should aggregate:

- Stage 1 seed manifest;
- deterministic scenario matrix;
- ordinary `send_message` / `start_stream_message` results;
- seeded task-control E2E results;
- frontend integration results;
- required Playwright/browser E2E smoke report;
- Beta v1 readiness report;
- Product Maturity v2 readiness report;
- opt-in live provider report when enabled;
- manual dogfood report status when present.

## 3. Readiness Dimensions

Required dimensions:

- routing;
- UI state;
- event replay;
- memory/proposal/rollback;
- plan interaction;
- tools and skills;
- permissions;
- recovery;
- final delivery;
- knowledge assets;
- seed isolation;
- live provider opt-in;
- no silent writes;
- no hidden legacy fallback;
- no fake execution.

## 4. Default Readiness

Default readiness requires:

- all P0/P1 required deterministic scenarios executed;
- all P0 required deterministic scenarios passed or expected-blocked correctly;
- every P1 deterministic scenario produced a structured evidence row;
- P1 blocker rows are allowed only when explicitly marked non-blocking and
  listed as accepted residual risk;
- at least 20 scenarios enter through Chat input;
- at least 8 seeded task-control scenarios pass;
- 100% of scenarios have runtime evidence;
- 100% of scenarios have UI evidence;
- 100% of scenarios have final delivery evidence;
- required browser E2E smoke scenarios pass;
- browser E2E environment is self-contained and does not depend on a manually
  pre-started dev server;
- `legacyFallbackCount=0`;
- `silentDurableWriteCount=0`;
- `fakeExecutionDetectedCount=0`;
- external live not required.

## 5. Internal Trial Recommendation

The report should produce:

- `not_ready`;
- `ready_for_engineering_dogfood`;
- `ready_for_internal_trial`.

Rules:

- `not_ready`: any default readiness blocker.
- `ready_for_engineering_dogfood`: automated deterministic dogfood and required
  browser smoke pass; P1 residual risks, if any, are documented and accepted;
  full manual dogfood may still be incomplete.
- `ready_for_internal_trial`: automated deterministic dogfood, required browser
  smoke, and required manual P0/P1 dogfood pass with no blocker or major issues.

## 6. Required Blockers

The gate must fail closed for:

- missing seed manifest;
- missing task session for work-like prompt;
- browser E2E environment not self-contained or unavailable;
- required browser E2E smoke not run or failed;
- UI state without runtime evidence;
- final delivery claiming unexecuted work as completed;
- assistant text used as state evidence;
- missing proposal for memory/knowledge update;
- silent durable write;
- hidden legacy fallback;
- stale resume replaying action;
- permission approval applying to wrong action;
- local/mock provider counted as external live;
- expected blocker not visible to user.

## 7. Test Plan

Minimum deterministic gate:

```bash
git diff --check
cargo test -p openlife-tauri main_chat_agent_stage1_dogfood -- --nocapture
cargo test -p openlife-tauri main_chat_agent_beta_v1_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/tauri.test.ts
corepack pnpm --dir frontend test:e2e -- main-chat-stage1-dogfood
```

The browser E2E command is part of the minimum gate. If the app-level E2E
environment is unavailable, the Stage 1 report must be `not_ready` with a
browser-specific blocker.

The implementation must also update `frontend/playwright.config.ts` with a
checked-in `webServer` configuration or provide an equivalent checked-in
self-contained E2E runner. Stage 1 readiness must not depend on a human manually
starting `localhost:5173`.

Additional browser E2E suites may be run with:

```bash
corepack pnpm --dir frontend test:e2e
```

Opt-in live:

```bash
OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1 \
OPENLIFE_LIVE_EVAL_PROVIDER=deepseek \
OPENLIFE_LIVE_EVAL_BASE=https://api.deepseek.com \
OPENLIFE_LIVE_EVAL_MODEL=deepseek-v4-flash \
cargo test -p openlife-tauri main_chat_live_provider -- --ignored --nocapture
```

The opt-in live command also requires `OPENLIFE_LIVE_EVAL_API_KEY` to already be
exported in the shell or provided by a non-repo secret mechanism. Do not write
the key into repository files.
