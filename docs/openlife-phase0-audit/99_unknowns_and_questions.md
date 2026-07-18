# Unknowns and Questions

## Unknowns

1. Frontend typecheck status.
   - Evidence: `corepack pnpm --dir frontend typecheck` failed because
     `frontend/node_modules` is missing and `tsc` was not found.
   - Status: `UNKNOWN`.

2. Frontend unit test status.
   - Evidence: Vitest was not run because frontend dependencies are missing.
   - Status: `UNKNOWN`.

3. Desktop product trial status.
   - Evidence: This audit did not run the Tauri app or Computer Use trial.
   - Status: `UNKNOWN`; active Phase7 docs still say `red-until-trial-green`.

4. External live-provider generation.
   - Evidence: This audit did not run a live external provider request.
   - Status: `UNKNOWN` for current machine/runtime.

5. Full web AgentLoop and MCP AgentLoop readiness.
   - Evidence: Read-only tool/MCP code exists, but no live journey was run.
   - Status: `PARTIAL` capability, end-to-end readiness `UNKNOWN`.

6. Product usability of current dense diagnostics.
   - Evidence: Source code shows many panels and controls; no human usability
     test or live walkthrough was performed.
   - Status: `UNKNOWN`.

7. Current third-party product reference details.
   - Evidence: Cursor, Codex, Claude workspace, and Linear were not researched
     live in this audit.
   - Status: `UNKNOWN` for current feature specifics. General UX principles only
     were used.

## Questions For Human Review

1. Should frontend v2 merge Companion and Chat into one agent workbench?
2. Which product state must be added to backend projections before UI rewrite?
3. Which memory lanes may materialize locally without Review Center approval?
4. Should manual LifeModel editor save remain available, and under what copy?
5. Which advanced diagnostics are for users versus engineering support?
6. Should the next step install frontend dependencies and run full frontend
   typecheck, format, unit, and E2E gates?
7. What isolated data profile should be used for the next real desktop product
   trial?
