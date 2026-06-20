# Main Chat Stage 5 Objective And Scope

> Date: 2026-06-20
> Stage: Stage 5 - Internal Trial Release and Debug Operations
> Status: preparation draft

## 1. Objective

Implement the release/debug operations layer required before serious internal
testing of Main Chat Agent.

Stage 5 should let internal testers answer:

1. Which build did I test?
2. Was my provider/environment ready?
3. What Agent task/run did my message create?
4. What strategy, tools, memory, knowledge files, and policy decisions were
   involved?
5. What completed, what was only proposed, what was blocked, and what still
   needs user input?
6. Why did the task fail, and what is the correct recovery action?
7. Can I export an issue report without leaking secrets or raw private data?
8. Can this report later support a real S2-D manual dogfood artifact?

## 2. Non-goals

- Do not implement public beta or app distribution.
- Do not create a cloud telemetry service.
- Do not create a second Agent runtime, task runtime, proposal system, memory
  system, or control plane.
- Do not implement new autonomous capabilities.
- Do not lower provider, privacy, tool, memory, final acceptance, or Stage 2
  readiness gates.
- Do not run or fill S2-D01 through S2-D24 manual dogfood rows as part of Stage
  5 development.
- Do not export raw API keys, auth headers, full prompts, full transcripts,
  full private memory, or full knowledge files by default.

## 3. Workstreams

| Workstream | Stage 5 output |
| --- | --- |
| Release provenance | Build info: commit, branch, app version, build timestamp, dirty-state flag if available. |
| Environment preflight | Provider/key/network/scheduler/workspace/MCP/database readiness with metadata-safe blockers. |
| Agent debug bundle | Metadata-safe app-data artifact keyed by `task_session_id` and optional scenario id. |
| Failure taxonomy | Stable failure classes and recovery recommendations. |
| Issue report/export | User/tester-facing command and UI to create a local app-data issue artifact. |
| Tester workflow | Scenario id, reviewer identity, pass/fail/blocker, notes, bundle refs. |
| Stage 5 gate | DBG5 coverage report that proves mechanics without claiming readiness. |

## 4. Source Objects To Reuse

- `AgentTaskSessionStore`
- `AgentRun`
- `ExecutionTranscript`
- `ActionQueue`
- `ProposalStore`
- `MemoryLifecycleStore`
- Stage 3 execution UX report/state
- Stage 4 memory/knowledge inventory and managed write history
- Stage 2 readiness/manual/live-provider contracts
- final acceptance runner and live-provider preflight
- existing frontend Main Chat, AgentControlPlane, Review Center, diagnostics,
  and Tauri wrapper patterns

## 5. Required Product Surfaces

At minimum:

- a debug/preflight panel or equivalent inspectable surface;
- an export/debug bundle command;
- an issue-report command or local artifact writer;
- UI controls to export the current task/session;
- visible failure category and recovery recommendation;
- Stage 5 DBG report command.

## 6. Exit Criteria

Stage 5 is complete when:

- DBG5-01 through DBG5-24 pass or are blocked with named blockers;
- at least DirectAnswer, read tool, ReAct blocker, memory proposal, memory
  context, managed knowledge write, and final delivery paths can export
  metadata-safe bundles;
- export rejects secrets and unsafe raw content by default;
- task-attached issue report includes build, scenario, reviewer, task, run, and
  bundle ids;
- preflight-only or environment-blocked issue report includes named blockers and
  explicit missing task/run reasons, and cannot be marked as task behavior pass;
- debug bundles and issue reports are schema-versioned, atomically written,
  reloadable after refresh, and not stored in the git workspace by default;
- Stage 2 readiness remains fail-closed without real manual/live evidence;
- all Stage 1/2/3/4/final/local gates remain passing or fail-closed for the
  same documented external/manual blockers.

## 7. Non-readiness Statement

Stage 5 can make OpenLife ready to run internal dogfood more responsibly. It
does not itself prove that dogfood passed. The final decision to enter or pass
limited internal trial still depends on real S2-D manual evidence and
current-commit external live-provider evidence.
