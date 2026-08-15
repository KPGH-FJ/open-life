# Tauri Command Contract Checklist

Use this checklist whenever a Tauri command is added, renamed, removed, or changes payload shape.

## Required Sync Points

- Rust command function exists and is registered in `src-tauri/src/lib.rs`.
- TypeScript wrapper in `frontend/src/tauri.ts` uses the same command name and payload casing.
- Frontend mock in `frontend/src/test/mocks/tauri.ts` returns the same response shape.
- Page or component usage imports the wrapper instead of calling `invoke` directly.
- Tests cover at least one successful path and one failure/empty-state path for user-facing commands.
- Production build/typecheck passes with `cd frontend && npm run build`.

## Naming Rule

- Rust internals may remain snake_case.
- Tauri invoke payloads used by the frontend should expose camelCase fields in TypeScript.
- Do not add dual camelCase/snake_case payload fallbacks. Update the Rust
  command, TypeScript wrapper, mock, and contract tests together.

## Proposal-Sensitive Commands

For commands that can mutate `LifeModel`, `Memory`, tool permissions, external state, or user data:

- Use the typed domain gateway. A governed effect links to the canonical
  Conversation/Turn or Task/Run/Item/ItemAttempt owner that requested it.
- Review checkpoints must bind the exact proposal, capability, target, and
  expected effect; approval alone is not materialization proof.
- An explicit low-risk reversible Memory write may use its dedicated typed
  lane. Other durable or external effects require the applicable confirmation
  or Review contract.
- Safe Mode behavior must be explicit and tested.

## CI Guardrail

`make ci` must cover Rust tests, frontend tests, and frontend production build/typecheck so contract drift is caught before release.
