# Architecture Decision Records

Accepted decisions are stable context beneath `PRODUCT.md`, `AGENTS.md`, and
current source.

| ADR | Status | Canonical path | Authority impact |
| --- | --- | --- | --- |
| ADR 0001: LifeModel Patch | Historical reference; direct-write compatibility assumptions are superseded by ADR 0013 and later governed proposal-first work. | [`docs/decisions/0001-lifemodel-patch.md`](./0001-lifemodel-patch.md) | Preserves early patch/proposal rationale only; does not authorize current direct durable LifeModel-HS writes. |
| ADR 0002: Proposal Unified Layer | Accepted historical decision; implementation boundary is governed by ADR 0013 and current source. | [`docs/decisions/0002-proposal-unified.md`](./0002-proposal-unified.md) | Keeps Proposal/Review Center intent as context; current mutation semantics remain proposal-first. |
| ADR 0003: AgentRun Tracking | Accepted historical decision; trace envelope and query semantics are extended by later runtime/read-model work. | [`docs/decisions/0003-agent-run-tracking.md`](./0003-agent-run-tracking.md) | Preserves AgentRun trace rationale; does not override current Main Chat runtime trace, evidence, or blocker contracts. |
| ADR 0013: LifeModel-HS Source Of Truth And Governance | Accepted. | [`plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`](../../plans/adr/0013-lifemodel-hs-source-of-truth-governance.md) | Current LifeModel-HS source-of-truth, proposal-first mutation, privacy, and materialized-view constraint reference. |
