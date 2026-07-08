# OpenLife

OpenLife is a local-first personal Agent OS built with a Tauri desktop shell,
React frontend, Rust core, and SQLite. The product path is a single governed
system: ordinary Main Chat enters AgentIngress, task/session evidence, policy,
tool/memory/proposal gateways, and the shared product read model.

## Current Authority

The active development authority is:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`

Older planning documents are historical evidence only. They do not authorize a
second product runtime, old command surfaces, old frontend status fields, or old
readiness routes.

## Documentation Map

- Architecture index: `docs/ARCHITECTURE.md`
- Source-backed explainers: `docs/architecture/`
- Testing and validation commands: `docs/development/testing.md`
- Document governance: `docs/repository_document_governance.md`

## Phase7 Status

Phase7 is a real deletion pass, not a compatibility pass. The product contract
is:

- no old product runtime module in the product crate graph;
- no old product command registered in the shipped Tauri handler;
- no frontend product page or product bridge depending on old fallback/status
  fields;
- no active README or active plan index steering work toward old routes;
- Computer Use trial evidence must be green, or red with clear fail-closed
  blockers and no completion claim.

Current blockers must be tracked in
`plans/openlife_single_system_deletion_manifest.md`. Do not describe Phase7 as
complete while that manifest marks a Phase7 object as not done or the trial
report is red.

## Product Entry Points

- Desktop app shell: `src-tauri/src/lib.rs`
- Main Chat send path: `src-tauri/src/main_chat_send.rs`
- Main Chat stream path: `src-tauri/src/main_chat_streaming.rs`
- Product read model: `src-tauri/src/life_state_projection.rs`
- Frontend product bridge: `frontend/src/tauri.ts`
- Main product pages: Today, Companion, Mailbox, Life Model, Runs, Settings

Dev/test-only artifacts must stay outside product pages, product bridge exports,
and product module graphs.

## Verification

Minimum Phase7 gates:

```sh
git diff --check
cargo fmt --check
cargo test -p openlife-tauri single_system -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend test -- App.test.tsx ChatPage.test.tsx tauri.test.ts
```

The Computer Use product trial report for this pass must live under
`frontend/test-results/phase7-computer-use-trial/`.
