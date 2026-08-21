# OpenLife AI Coding Entrypoint

This is the durable coding entrypoint for `/Users/tw/Desktop/open-life`.

## Read Order

1. `AGENTS.md`
2. `PRODUCT.md`
3. `plans/README.md`
4. Task-relevant source and stable architecture/decision documents

Historical plans are available in Git history. They are not current authority.
Do not recreate a machine-readable development program, problem ledger,
append-only evidence registry, task-packet system, or repository self-evolution
platform.

## Project Shape

- Product: local-first personal Agent OS
- Stack: Tauri 2, Rust, React 18, TypeScript, SQLite
- Product routes: `/workspace`, `/life-model`, `/settings`

Main Chat has separate send and stream entrypoints that converge on the same
runtime owner:

```text
frontend/src/tauri.ts
  -> src-tauri/src/lib.rs
  -> main_chat_send.rs | main_chat_streaming.rs
  -> canonical_chat_runtime.rs | canonical_work_runtime.rs
  -> main_chat_kernel.rs
  -> openlife-core/src/agent/main_chat_agent_v1.rs
```

## Product Rules

- Never silently write LifeModel, memory, files, calendar, email, provider,
  plugin, shell, or other durable/external state.
- Risky durable changes require confirmation or Review Center proposal flow.
- Creating or approving a proposal is not the same as proving materialization.
- Missing or stale evidence stays unknown, blocked, or failed.
- Use `LifeStateProjection` and backend ViewModels instead of rebuilding product
  truth from raw diagnostics or config fragments.
- Tool credit comes from explicit capability, risk, and action contracts.
- Do not treat local fixtures, mocks, browser-shell tests, or scripted providers
  as native or external-live evidence.

## Development Rules

- Use this checkout as the only writable OpenLife checkout.
- Do not create sibling OpenLife directories or additional worktrees.
- Preserve unrelated user changes in a dirty worktree.
- Start source changes from the real runtime/import/handler path.
- Keep plans small. One active Markdown plan is enough for substantial work.
- Do not add governance JSON unless it is a small, necessary runtime or CI
  interface with a real consumer.
- Prefer product tests over tests of planning documents, file names, digests,
  line counts, or approval records.
- Keep production, dev-only, test-only, and historical surfaces distinct.
- Do not restore retired product routes or compatibility fallbacks.

## Common Checks

Choose checks proportional to the change:

```sh
git diff --check
cargo fmt --check
cargo clippy --all --locked -- -D warnings
cargo test --all --locked
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend test
corepack pnpm --dir frontend build
corepack pnpm --dir frontend test:e2e
```

## Stable Documentation

- `README.md`
- `PRODUCT.md`
- `docs/ARCHITECTURE.md`
- `docs/architecture/`
- `docs/decisions/`
- `docs/development/testing.md`
- `docs/repository_document_governance.md`
- `plans/adr/`

When documentation conflicts with source, verify source and update the
documentation. Do not add another authority layer to explain the conflict.
