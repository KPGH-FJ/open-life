# Current OpenLife Product Development Plan

Status: active

## Objective

Complete S7 by removing the remaining release-reachable parallel lifecycle and
compatibility surfaces left after the canonical report-path migration, while
preserving the proven Workspace -> Task -> Review -> Artifact result loop.
Finish with one clean release baseline whose production commands, stores,
read models, and documentation no longer advertise retired product owners.

## In scope

1. Remove unused standalone PlanExecute IPC, frontend contracts, mocks, and
   release command registration. Planning remains an Item inside a Task.
2. Retire residual PlanExecute session ownership from ordinary Main Chat after
   migrating any still-required report/task facts to the canonical runtime.
3. Remove obsolete compatibility wrappers and projections that have no current
   product caller, including algorithm-named durable strategy state where it no
   longer owns behavior.
4. Reduce overlapping TaskSession, ActionQueue, AgentRun, and Event lifecycle
   ownership only through complete production paths; do not introduce dual
   writes or a second runtime.
5. Add product/static guards that keep retired release commands and fallback
   paths absent.
6. Run full Rust/frontend gates, build the exact release bundle, and perform
   proportional isolated native verification of the report path.

## Out of scope

- new tools, connectors, computer use, arbitrary shell, subagents, or provider
  auto-routing;
- expanding Memory or LifeModel behavior;
- deleting user profiles, Keychain credentials, or historical database files;
- rewriting healthy gateways, receipts, cancellation, outbox, or Review
  materializers merely to rename them;
- a big-bang migration of unrelated scheduled-task or personal-intelligence
  domains.

## Deletion order

1. Standalone PlanExecute release surface with no current frontend caller.
2. Ordinary Main Chat PlanExecute session/store ownership and its compatibility
   projections.
3. Remaining report-path duplicate lifecycle state that canonical Task Items
   already own.
4. Dead adapters, DTO fields, mocks, tests, and documentation exposed only by
   the retired paths.

Each slice must leave send and stream converged on `OpenLifeTurnRuntime`, keep
the canonical report behavior matrix green, and end in a reviewable commit.

## Acceptance

- Release frontend and Tauri handlers expose no standalone PlanExecute product
  API.
- A report plan is represented by canonical Task Items, not an independent
  PlanExecute session.
- Approval resumes the same Task and verified Artifact result without a
  compatibility fallback.
- No removed command remains in frontend mocks, release guards, or generated
  handler registration.
- Missing historical stores fail closed or remain read-only migration input;
  they are never silently recreated as active product owners.
- Full checks pass, the final working tree is clean, and an exact release bundle
  is produced.

## Checks

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

## Stop condition

S7 closes only when the retired release surfaces and duplicate report-path
owners are absent, current-source tests and product guards pass, the report
path remains verified, the exact release bundle builds, and the repository is
clean. If a legacy store still has a real production consumer, stop deletion
at that boundary and migrate the consumer before removing the store.
