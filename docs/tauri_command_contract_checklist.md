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
- If the backend temporarily accepts both camelCase and snake_case for compatibility, tests should still assert the canonical frontend wrapper contract.

## Proposal-Sensitive Commands

For commands that can mutate `LifeModel`, `Memory`, tool permissions, external state, or user data:

- Prefer creating an `AgentProposal` instead of direct mutation.
- Link generated proposals back to the originating `AgentRun`.
- Direct-write commands must be clearly marked legacy, migration, or debug-only.
- Safe Mode behavior must be explicit and tested.

## CI Guardrail

`make ci` must cover Rust tests, frontend tests, and frontend production build/typecheck so contract drift is caught before release.
