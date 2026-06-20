# Main Chat Stage 5 Internal Tester Workflow

> Date: 2026-06-20
> Stage: Stage 5 - Internal Trial Release and Debug Operations
> Status: preparation workflow

## 1. Purpose

This workflow is for internal testers using OpenLife after Stage 5. It is not
the final S2-D manual dogfood protocol, but it prepares the artifacts needed to
run that protocol without losing traceability.

## 2. Before Testing

Tester should verify:

1. App build/commit is visible.
2. Workspace root is correct.
3. Provider preflight is either ready or has named blockers.
4. Network state is known.
5. MCP registry state is known.
6. Database/state stores are available.
7. Stage 2 readiness still shows not ready unless real manual/live evidence was
   supplied.

If preflight is blocked by missing key/network/provider setup, tester records
the blocker and does not mark product behavior as failed.

## 3. Running A Task

For each task:

1. Select or enter a scenario id, for example `S2-D07` or `DBG5-08`.
2. Send the user request through Main Chat.
3. Observe Agent task state, timeline, actions, blockers, proposals, memory
   usage, final delivery, and recovery controls.
4. Do not manually edit stores or database files during the run.
5. If the task blocks, use only visible product controls to resume/retry/cancel.
6. Export a debug bundle from the task panel.
7. Create an issue report with status:
   - `pass`
   - `fail`
   - `blocked_by_environment`
   - `blocked_by_policy`
   - `needs_product_decision`

The debug bundle and issue report should be saved as local app-data artifacts.
Testers should not manually place these generated artifacts into the git
workspace or edit source files to make a report pass.

## 4. Issue Report Fields

Required:

- scenario id;
- reviewer id or local tester alias;
- build commit;
- app version;
- task session id for task-attached reports;
- run id for task-attached reports;
- debug bundle id;
- local artifact id/storage alias/digest/byte size;
- pass/fail/blocker status;
- failure class when failed;
- redaction mode;
- notes digest or bounded notes preview;
- created timestamp.

Preflight-only or environment-blocked reports may omit task session id and run
id only when they include a named blocker and a metadata-safe reason explaining
why no task/run exists. Those reports cannot be marked as a task behavior
`pass`.

Optional:

- screenshot artifact digest;
- expected behavior;
- actual behavior;
- recovery attempted;
- suggested follow-up.

## 5. Stop Conditions

Tester should stop or mark the row invalid when:

- build commit is missing or stale;
- debug bundle export fails;
- task session id is missing for a task-attached report;
- run id is missing for a task-attached report marked `pass` or `fail`;
- preflight-only report omits task/run ids without a named blocker;
- raw API key or secret appears in UI/export;
- app uses local/mock provider while row claims external live behavior;
- task was completed through manual database/file edits;
- scenario was run on an unknown branch;
- exported artifact is not metadata-safe.
- artifact was manually edited after export.

## 6. How This Feeds Stage 2 Manual Dogfood

Stage 5 issue reports can become inputs to future Stage 2 S2-D manual dogfood,
but they are not automatically Stage 2 evidence.

To become Stage 2 evidence later, the report must be reviewed under the Stage 2
manual dogfood contract, matched to known build commit, and include the required
reviewer/runtime evidence for that S2-D row.

## 7. Tester-facing Copy Principles

- Explain environment blockers separately from Agent failures.
- Show exact recovery action.
- Avoid internal-only Rust/module names in primary UI.
- Keep raw private content out of exported artifacts.
- Make "not ready" actionable, not vague.
