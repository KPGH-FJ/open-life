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

There is intentionally no active product-development plan during repository
cleanup. The next plan will be written after the clean baseline is reviewed.

## Accepted Decisions

- `adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `adr/0014-explicit-user-memory-write-lane.md`
- `adr/0015-transient-state-command-lane.md`

## CI Support

`openlife_rust_advisory_ownership.json` is retained because the security-audit
CI job consumes it directly.
