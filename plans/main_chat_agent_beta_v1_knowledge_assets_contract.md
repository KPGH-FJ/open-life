# Main Chat Agent Beta v1 Knowledge Assets Contract

> Date: 2026-06-18
> Workstream: 4 of 5
> Status: preparation artifact

## 1. Product Goal

Turn OpenLife's knowledge formats into understandable user-facing assets.

OpenLife should support the same broad pattern that strong agents use:

- scoped durable instruction files;
- bounded memory files;
- user profile snapshots;
- skills as workflow instructions;
- context source inspection;
- proposal/confirmation before durable truth changes.

But OpenLife must preserve its own governance model: knowledge files provide
context, not authority over privacy, model routing, tools, or memory truth.

## 2. Benchmark Insight

Codex uses layered `AGENTS.md` guidance and skills with progressive disclosure.
Claude Code documents `CLAUDE.md`-style project memory and auto memory files
that are inspectable. Hermes prompt assembly publicly separates `SOUL.md`,
project context files, `MEMORY.md`, `USER.md`, and skills. Claude's memory tool
shows the same general pattern: store knowledge in files, retrieve it on demand,
and keep application control over persistence.

OpenLife implication:

- `AGENTS.md`, `SOUL.md`, `USER.md`, `MEMORY.md`, and `SKILL.md` should be
  visible product objects with scope, source, digest, loaded state, and
  lifecycle.
- The user should be able to inspect what influenced a task.
- Memory edits must stay proposal/confirmation/rollback-based.

## 3. Knowledge Asset Types

| Asset | Product meaning | Edit policy | Runtime authority |
| --- | --- | --- | --- |
| `AGENTS.md` | Project or workspace operating guidance. | Inspect/propose by default; write only with explicit workspace-write confirmation and audit. | Context only; cannot override policy. |
| `SOUL.md` | Agent identity/personality layer. | Guardrailed proposal/edit flow only. | Context only; cannot override safety/policy. |
| `USER.md` | Short current user profile/preferences snapshot. | Through proposal/edit flow. | Context; generated from accepted memory/guidance. |
| `MEMORY.md` | Curated durable memory summary. | Through proposal/edit/rollback flow. | Context; backed by evidence/provenance. |
| `SKILL.md` | Workflow instruction package. | Inspect/propose by default; write only with explicit workspace-write confirmation, validation, and audit. | Context when selected; not permission. |
| Session search | Past conversation retrieval. | No direct file edit. | Evidence/source, not durable truth. |
| Evidence records | Source support for memory/proposals. | No direct edit in beta. | Provenance. |

## 4. Source Of Truth Rules

Knowledge assets must not create a second memory system.

- `AGENTS.md` and local `SKILL.md` files are inspectable project/workspace
  assets. The default product flow should propose a diff. Applying that diff is
  a durable workspace write and must require explicit user confirmation,
  workspace-scope validation, and audit evidence.
- `USER.md` and `MEMORY.md` are materialized views or curated projections of
  accepted memory/guidance. Direct file edits must not become durable user truth
  unless they go through proposal, confirmation, provenance, and rollback
  records.
- `SOUL.md` may be shown or edited only through guardrailed flows. It must not
  bypass privacy, model routing, tool, memory, or execution policy.
- The runtime store owns memory/proposal/evidence state. File surfaces expose
  inspectable context and materialized snapshots.
- If a file surface and runtime store disagree, the UI must show the conflict and
  prefer runtime provenance for governed memory truth.

## 5. Required Product Surface

The knowledge asset surface should support:

- list assets by scope and type;
- inspect loaded/not-loaded status for a task;
- show digest, size, truncation, source path, and last modified time;
- show why a skill or memory asset was selected;
- show policy boundaries: context only, not authority;
- create proposal for memory/profile changes;
- accept/reject/edit/rollback memory proposals;
- show active vs rolled-back memory state;
- show unselected `SKILL.md` absence from context.

## 6. Context Assembly Inventory

Every task should be able to expose a bounded inventory:

- eligible context files;
- loaded context files;
- skipped files with reason;
- selected skill id;
- context digests;
- truncation status;
- memory snapshot ids;
- proposal ids used or created;
- policy overrides rejected.

This inventory should be visible in a trace drawer, not dumped into the main
chat unless the user asks.

## 7. Governance Rules

- Assistant text cannot become `USER.md` or `MEMORY.md` truth without user
  confirmation.
- A selected `SKILL.md` cannot grant tool permission.
- File instructions cannot override ExecutionPolicy, privacy policy, model
  routing policy, or external write policy.
- Rolled-back memory must be excluded from active context and visible as
  historical.
- Knowledge files should be bounded and digested; oversized content must be
  truncated or loaded on demand with explicit trace.
- Direct edits to materialized `USER.md` or `MEMORY.md` must create a proposal or
  conflict record, not silently mutate accepted memory.

## 8. Acceptance

Knowledge assets are acceptable when:

- users can inspect which knowledge assets affected a Main Chat task;
- selected/unselected skill behavior is proven;
- memory proposal, accept, reject, edit, and rollback are visible and backed by
  records;
- `USER.md`/`MEMORY.md` reflect accepted state, not raw conversation;
- context inventory is included in eval output;
- direct file edits cannot bypass the governed memory source of truth;
- `AGENTS.md` / `SKILL.md` writes cannot bypass workspace write confirmation,
  validation, or audit evidence;
- unsafe or policy-overriding knowledge content is ignored or blocked with
  trace;
- no duplicate knowledge store is introduced.

## 9. Anti-patterns

- Treating vector memory as the product memory system.
- Treating raw transcript as durable user truth.
- Letting `SOUL.md` become an unrestricted jailbreak surface.
- Loading all skills into every prompt.
- Showing a knowledge asset in UI without proving whether it was loaded.
- Hiding rollback history.
- Treating generated `USER.md` or `MEMORY.md` text as authoritative without
  provenance.

## 10. Out Of Scope

- Full public skills marketplace.
- Automatic self-evolution of skills.
- Multi-device knowledge sync.
- Bulk import/export.
- Arbitrary direct edits to governed memory truth without proposal records.
- Silent direct writes to `AGENTS.md`, `SKILL.md`, `USER.md`, `MEMORY.md`, or
  `SOUL.md`.
