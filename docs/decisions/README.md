# Architecture Decision Records

> Status: Stage4A ADR index.
> Authority: decision-log index only; subordinate to `AGENTS.md`,
> `plans/README.md`, and current Phase7 single-system authority.

This index records the public ADR surface without moving ADR 0013. Stage4A uses
a no-move canonical pointer for ADR 0013: the accepted LifeModel-HS governance
record remains at `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`.
No duplicate ADR 0013 file is created under `docs/decisions/`.

| ADR | Status | Canonical path | Authority impact |
| --- | --- | --- | --- |
| ADR 0001: LifeModel Patch | Historical reference; direct-write compatibility assumptions are superseded by ADR 0013 and later governed proposal-first work. | [`docs/decisions/0001-lifemodel-patch.md`](./0001-lifemodel-patch.md) | Preserves early patch/proposal rationale only; does not authorize current direct durable LifeModel-HS writes. |
| ADR 0002: Proposal Unified Layer | Accepted historical decision; implementation boundary is governed by later Phase7, ADR 0013, and Main Chat governance docs. | [`docs/decisions/0002-proposal-unified.md`](./0002-proposal-unified.md) | Keeps Proposal/Review Center intent as context; current mutation semantics remain proposal-first and governed by active authority. |
| ADR 0003: AgentRun Tracking | Accepted historical decision; trace envelope and query semantics are extended by later runtime/read-model work. | [`docs/decisions/0003-agent-run-tracking.md`](./0003-agent-run-tracking.md) | Preserves AgentRun trace rationale; does not override current Main Chat runtime trace, evidence, or blocker contracts. |
| ADR 0013: LifeModel-HS Source Of Truth And Governance | Accepted; Stage4A no-move canonical pointer. | [`plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`](../../plans/adr/0013-lifemodel-hs-source-of-truth-governance.md) | Current LifeModel-HS source-of-truth, proposal-first mutation, governance, privacy, and materialized-view constraint reference; remains anchored at the existing `plans/adr/` path for this stage. |

## Stage4A Boundary

Stage4A creates this index only. It does not move ADR 0013, create a duplicate
ADR 0013 file under `docs/decisions/`, create `docs/product/`, create
`plans/archive/` or `plans/active/`, edit source code, promote authority, or
claim Phase7, Main Chat Agent Execution v1, live-provider evidence, or
runtime-module completion.
