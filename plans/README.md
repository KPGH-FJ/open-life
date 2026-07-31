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

## Active Product Development Program

[`openlife_product_development_program.md`](openlife_product_development_program.md)
is the single active development program. It fixes the six-phase product path,
phase exit criteria, and the method used to investigate and implement each
phase.

The current phase pointer in that file is authoritative for development order.
Phase details may adapt to current source and product evidence, but Agents must
not add, replace, reorder, or rename Program phases without explicit user
approval.

## Accepted Decisions

- `adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `adr/0014-explicit-user-memory-write-lane.md`
- `adr/0015-transient-state-command-lane.md`

## CI Support

`openlife_rust_advisory_ownership.json` is retained because the security-audit
CI job consumes it directly.
