# Main Chat Kernel Rescue Spec-Coding Contract

> Date: 2026-06-22
> Status: shared implementation contract for all eight goal-mode passes
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## 1. Purpose

Every goal-mode pass must produce code that matches a written spec, not a vague
intention. This contract defines the minimum spec-coding bar for the rescue.

## 2. Required Sections Per Goal

Each goal spec must include:

- objective;
- system position;
- OpenLife lessons applied;
- industry practices applied;
- allowed implementation scope;
- out-of-scope list;
- required runtime contracts;
- acceptance checklist;
- verification commands;
- stop conditions;
- completion report requirements.

## 3. Required Runtime Contracts

Where applicable, implementation must define or preserve:

- input type contract;
- result type contract;
- event contract;
- persistence contract;
- permission/proposal contract;
- UI evidence contract;
- migration/legacy compatibility contract;
- test contract.

## 4. Cross-Goal Invariants

These invariants must remain true after every goal:

- no ordinary Main Chat path silently writes durable LifeModel or Memory truth;
- no file, calendar, email, provider, plugin, or shell side effect occurs
  without proposal, permission, or hard blocker;
- unsupported tool requests do not become fake successful answers;
- send and stream must converge toward one kernel path, not diverge;
- HS context cannot override privacy/tool/write policy;
- final/live gates cannot be required for basic local kernel behavior;
- UI claims must be backed by runtime evidence.

## 5. Source And Consistency Checks

Before starting and before completing each goal, check that the implementation
does not conflict with:

- `plans/main_chat_agent_kernel_rescue_industry_practices.md`;
- `plans/main_chat_agent_kernel_rescue_acceptance_matrix.md`;
- `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`;
- `AGENTS.md`.

If an external source is used to justify a design rule, prefer official
documentation or primary-source engineering posts, and record any version/date
assumption in the goal completion report.

## 6. Test Strategy

Each goal should prefer focused tests before broad gates:

1. Unit or helper test for the new contract.
2. Kernel-level test for the turn behavior.
3. Command-surface test for send/stream parity.
4. UI test only when a user-facing state changes.
5. Final/readiness gate update only after the kernel behavior is stable.

Every runtime-changing goal must include explicit compile checks for both core
and Tauri packages unless the goal completion report explains why one command is
not applicable:

```bash
cargo check -p openlife-core
cargo check -p openlife-tauri
```

## 7. Completion Rule

A goal is complete only when:

- all acceptance checklist items are satisfied;
- the matching K-row entries in
  `plans/main_chat_agent_kernel_rescue_acceptance_matrix.md` are satisfied;
- listed verification commands have passed or a command limitation is recorded;
- no stop condition remains active;
- the completion report follows
  `plans/main_chat_agent_kernel_rescue_goal_completion_template.md` and names
  changed files, verification evidence, safety evidence, fallback/direct-write
  evidence, source consistency, and remaining risk.
