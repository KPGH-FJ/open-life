# Current OpenLife Product Development Plan

Status: complete

## Objective

Complete S4 on the canonical report lifecycle: let a user steer active work,
approve a governed boundary inline and continue the same Run, recover exact
checkpoints after interruption/restart, and run a bounded number of independent
tasks concurrently without allowing duplicate owners.

## Product path

```text
report Task starts before execution
  -> Run + Instruction + Plan
  -> governed reads / provider work
  -> optional Steering Item at a safe checkpoint
  -> optional Permission or Review checkpoint
  -> inline decision resumes the same Run
  -> Artifact / Verification / FinalResult
```

Steering is authenticated user input, not permission. It may refine outcome,
format, emphasis, or constraints inside the existing task grant, but it cannot
expand workspace, provider, network, tool, write, or destructive authority.

## In scope

1. Begin the canonical report Task/Run before tool or provider execution and
   append typed Items transactionally instead of creating the whole history at
   ArtifactDraft time.
2. Add a durable, digest-only Steering Item bound to the exact Task, Run, user
   message, and base plan revision.
3. Accept steering only while the target Run is active or waiting; consume it
   once at a defined safe checkpoint before the next provider generation.
4. Keep scope-expanding steering blocked behind the existing policy/permission
   boundary; current instruction never mints capability.
5. Add one Workspace inline approval-and-continue action for the exact pending
   proposal/checkpoint. Approval and continuation remain separately evidenced.
6. Reuse `OpenLifeTurnRuntime.run_replay`, terminal-owner replay epochs, and
   existing action-bound grants so continuation stays on the same Run.
7. Make restart recovery reload Steering/checkpoint truth from SQLite and fail
   closed on missing, stale, conflicting, or already-consumed input.
8. Bound simultaneous independent Task execution while retaining one owner per
   Task and preserving cancellation/receipt isolation.

## Out of scope

- editing a provider request already dispatched remotely;
- autonomous interpretation of ambiguous scope expansion;
- new connectors, computer use, shell, subagents, or background schedules;
- S5 Results/Changes/Preview visual redesign;
- provider auto-routing or cross-provider fallback;
- Memory or LifeModel learning changes;
- deleting remaining compatibility paths before S7.

## Ownership

- `CanonicalTaskRuntimeStore` owns Task/Run/Item order, steering identity,
  checkpoint state, and consumption revision; it never owns adapter execution.
- `OpenLifeTurnRuntime` remains the sole execution and continuation owner.
- PolicyRouter and existing permission/proposal authorities decide scope; a
  Steering Item cannot alter their grants.
- ToolGateway/provider receipts remain execution truth. Review acceptance and
  materialization remain distinct facts.
- Backend ViewModels own product projection; the frontend does not infer that
  approval, resume, or completion succeeded.

## Acceptance

| Scenario | Required result |
| --- | --- |
| Active report receives steering before provider | one Steering Item; same Task/Run; provider sees exact steered constraint |
| Duplicate steering submission | idempotent same Item; no duplicate generation |
| Conflicting reuse of steering id | rejected with no partial mutation |
| Steering after terminal completion | rejected; completed result is not rewritten |
| Steering requests new scope | stored only as blocked/pending input; no capability or effect |
| Waiting permission accepted inline | acceptance fact then same Run replay; exact action executes once |
| Review/materialization accepted inline | same Task advances only after observed materialization |
| Declined or stale checkpoint | no resume and no effect |
| Restart before steering consumption | exact pending Steering Item is recovered once |
| Restart after consumption | no duplicate provider/tool/effect |
| Same Task concurrent owners | exactly one wins; losing call cannot mutate state |
| Independent tasks within limit | execute concurrently with isolated cancel/receipts |
| Independent task above limit | typed busy/queued result; no task/run partial creation |
| Product read model | exposes steering/checkpoint/continuation states from backend truth |

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

S4 is complete only when steering and inline continuation are real canonical
product paths, restart and concurrency negatives fail closed, full gates pass,
stable docs agree, commits are reviewable, and the working tree is clean.
Browser-shell evidence does not count as native or external-live evidence.

## Closure

- The report Task/Run begins before governed reads or provider work.
- Digest-only Steering Items are restart-safe, revision-bound, idempotent, and
  consumed once before provider generation; scope expansion remains blocked.
- Review can approve and continue through one product action while preserving
  separate acceptance, materialization, and replay evidence.
- Independent turns are bounded before any canonical mutation; same-task
  execution retains one owner.
- Full Rust and frontend unit suites, production build/absence guard, and
  proportional static checks pass. Native and external-live evidence remain S6
  concerns.

## Next pointer

After S4 closes, begin S5: Results, Changes, Preview, and Verification product
surfaces over the same backend read models.
