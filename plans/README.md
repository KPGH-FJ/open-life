# OpenLife Plans

`plans/` contains only current planning and accepted architecture decisions.
Completed and superseded plans remain available in Git history instead of the
working tree.

## Current Planning Rule

- Keep at most one active implementation plan for the current development
  objective.
- Use Markdown unless a small machine-readable runtime or CI interface is
  genuinely required.
- Delete or replace the active plan when its work is complete or superseded.
- Do not build ledgers, task-packet systems, approval chains, append-only
  registries, or validators for the planning process itself.

## Current Development Status

`openlife_foundation_control_loop_plan.md` is the one active implementation
plan. It supersedes the completed Agent capability rebuild plan, which remains
available in Git history. Product direction lives in `PRODUCT.md`; accepted
architecture lives in ADRs.

## Accepted Decisions

- `adr/0016-agent-memory-lifemodel-domain-boundaries.md`
- `adr/0019-capable-agent-harness.md`
- `adr/0018-product-reconstruction-contract.md`
- `adr/0017-canonical-task-runtime.md`
- `adr/0014-explicit-user-memory-write-lane.md`
- `adr/0015-transient-state-command-lane.md`

ADR 0013 is retained as superseded historical evidence and is not current
architecture authority.

## CI Support

`openlife_rust_advisory_ownership.json` is retained because the security-audit
CI job consumes it directly.
