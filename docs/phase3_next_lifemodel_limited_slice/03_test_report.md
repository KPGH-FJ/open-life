# LifeModel Limited Slice Test Report

Status: local validation report for the frontend-only limited adapter.

Naming boundary: this is the LifeModel limited slice after Phase 3A-2, not an
official Phase 3B.

## Gates Run

| Command | Result | Notes |
| --- | --- | --- |
| `git diff --check` | Pass | No whitespace errors in the final diff. |
| `corepack pnpm --dir frontend test -- lifeModelViewModel` | Pass | 12 focused contract tests passed. |
| `corepack pnpm --dir frontend typecheck` | Pass | TypeScript checked with `tsc --noEmit`. |
| `corepack pnpm --dir frontend format:check` | Pass | Prettier check passed after formatting the new files. |
| `corepack pnpm --dir frontend test -- LifeModelPage` | Pass | Existing LifeModelPage suite passed: 12 tests. React Router future-flag warnings only. |

## Contract Coverage

Covered by `frontend/src/viewmodels/lifemodel/lifeModelViewModel.test.ts`:

1. Empty LifeModel returns an empty limited envelope without fake canonical
   summary.
2. Current compatibility view is labeled `current_compatibility`, not
   canonical.
3. Dimension summaries retain limited confidence and provenance labels.
4. Pending LifeModel proposals become candidate changes and pending counts only.
5. Explicit `life_model_update` proposals are counted even when the affected
   path is not dimension-prefixed.
6. Accepted proposal/current-view evidence does not become applied
   materialization.
7. Stale and Safe Mode states disable risky actions.
8. Error envelope does not fall back to raw LifeModel data.
9. Memory linkage remains partial/unknown with only count and tier stats.
10. `actions.debugOnly` does not leak into primary actions.
11. Static scan blocks forbidden bridge/write symbols in the new ViewModel
    package.

## Not Run

- Browser E2E.
- Desktop/Tauri product trial.
- Backend Rust tests.
- External live-provider, Web AgentLoop, or MCP AgentLoop validation.

These are outside this frontend-only limited slice. Phase7 remains
`red-until-trial-green`.
