# Main Chat Stage 2 Internal Trial Acceptance Matrix

> Date: 2026-06-19
> Stage: Main Chat Agent Stage 2 - Internal Trial Readiness
> Status: preparation matrix

## 1. Purpose

This matrix defines what must be true before OpenLife can enter a limited
internal trial. It is product-facing: a passing backend test is not enough unless
the user-visible state and review evidence are also present.

## 2. Acceptance Levels

| Level | Meaning |
| --- | --- |
| P0 | Must pass before limited internal trial. |
| P1 | Should pass before trial; may remain as named non-P0 residual risk. |
| P2 | Improvement after trial starts. |

## 3. Matrix

| Area | Priority | Current state | Target state | User-visible requirement | Backend dependencies | Automated acceptance | Manual acceptance |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Main Chat task start | P0 | Stage 1 proves D01-D36 can start from real Tauri Chat UI. | Internal trial tasks start from normal Chat without hidden experimental route. | Task frame appears for work-like requests; simple answers remain compact. | AgentIngress, StrategyRouter, AgentTaskSession. | Tauri browser E2E includes Stage 2 P0 tasks. | Reviewer can run tasks without developer setup beyond documented env. |
| Direct answer | P0 | Governed DirectAnswer exists. | Simple answers are low-noise but traceable. | Optional trace shows route/provider/context/no-tool reason. | DirectAnswer strategy, provider route trace. | Send/stream test verifies no fake actions and no legacy fallback. | Reviewer can distinguish answer from task execution. |
| Read-only tool execution | P0 | File/session/memory/web/MCP read foundations exist. | Read tasks show selected tool, action, observation, source, and answer. | Action timeline and observation preview are visible. | ActionExecutor, ExecutionPolicy, transcript, workspace resolver, MCP registry. | At least 8 read scenarios pass with runtime/UI/final evidence. | Reviewer confirms source/observation match final answer. |
| Multi-step ReAct | P0 | Governed AgentLoop exists for bounded reads. | Multi-read tasks execute at least two observations and synthesize result. | User sees each action/observation and final synthesis. | AgentLoop, tool selection, ActionQueue, transcript. | At least 4 multi-step scenarios pass with no single-step fake fallback. | Reviewer can explain what was done and why. |
| Plan-Execute-Review | P0 | Plan foundations exist; Stage 1 has plan controls. | User can draft, edit/confirm, execute one or more steps, and review. | Plan states, active step, blocked/skipped/completed states visible. | PlanExecute runtime, task events, final delivery. | At least 4 plan scenarios pass. | Reviewer can change or skip a step and see correct result. |
| Memory proposal | P0 | Proposal-first memory lifecycle exists. | Memory update requests create reviewable proposals with evidence. | Candidate, evidence, conflict, accept/reject/edit/defer controls visible. | ProposalStore, EvidenceStore, Memory lifecycle, context loader. | At least 5 memory proposal scenarios pass with no direct durable write. | Reviewer accepts/rejects/edit one candidate and validates outcome. |
| Permission control | P0 | ToolPermission proposal and policy blockers exist. | Safe read permissions pause and resume with exact scope; write-like actions remain proposal-only or blocked in Stage 2. | Action, target, risk, scope, approve/deny/defer controls visible. | ExecutionPolicy, ToolPermissionStore, ActionQueue. | Safe-read permission acceptance resumes exact pending action; denial does not execute; write-like P0 actions never execute directly. | Reviewer can see consequence of approval before acting. |
| Failure recovery | P0 | Retry/resume/cancel and blockers exist. | Every failure has reason and next action or terminal explanation. | Retry, resume, cancel, ask-user, or terminal blocker visible. | ActionQueue, blockers, task controls, event stream. | At least 8 failure/recovery cases pass. | Reviewer can recover at least 4 failure tasks manually. |
| Final delivery | P0 | Final delivery sections exist. | Every task ends with executed/proposed/blocked/pending distinction. | Final card shows done work, sources, proposals, blockers, next step. | AgentRun finalization, transcript, ProposalStore. | Every P0 scenario has final delivery evidence. | Reviewer does not see "done" for proposed/blocked work. |
| Manual dogfood report | P0 | Stage 1 manual dogfood not attempted. | Internal reviewers complete bounded protocol. | Report has reviewer notes and trace ids. | Dogfood task set, trace export, readiness report. | Report schema validates required fields. | At least 2 reviewers complete assigned scope. |
| Live provider | P0 | Harness exists; external live separate. | Real provider passes P0 Direct/ReAct/MCP/proposal/recovery scenarios. | Live provider status visible and separate from deterministic readiness. | Live provider harness, scheduler, final gate. | P0 live eval passes with provider/model trace and no fake/local credit. | Reviewer understands live model limitations and blockers. |
| Event replay | P1 | Durable event stream exists. | Task state recovers after navigation/reload. | Replay status and gap recovery are visible if needed. | Event store, ChatPage replay, snapshot fallback. | At least 4 reload/replay scenarios pass. | Reviewer reloads during active/blocked task. |
| Knowledge assets | P1 | Bounded knowledge loader exists. | User can inspect active context sources and proposal-first edits. | Active sources, selected skill, proposal status visible. | Context loader, ProposalStore, Review Center. | At least 4 knowledge asset scenarios pass. | Reviewer can tell context from durable memory. |
| Skill/tool choice | P1 | Selected `SKILL.md` and MCP candidates exist. | Relevant tools/skills are selectable and auditable. | Selected skill/tool, reason, result/blocker shown. | Skill manifest/runtime, MCP registry, ExecutionPolicy. | At least 4 skill/tool scenarios pass. | Reviewer confirms unselected skill is not injected. |
| Trial onboarding | P1 | No complete internal trial operator guide. | Reviewers can set up and report without developer handholding. | Clear runbook and blocker template. | Docs, scripts, environment preflight. | Docs lint/manual checklist. | New reviewer follows runbook successfully. |

## 4. Required Stage 2 Readiness Report

The Stage 2 readiness report must follow
`plans/main_chat_stage2_readiness_gate_contract.md` and include:

- deterministic gate result;
- live provider gate result;
- manual dogfood result;
- scenario pass/fail counts;
- reviewer count;
- trace/artifact ids;
- silent write count;
- legacy fallback count;
- direct/fake evidence rejection count;
- readiness recommendation;
- residual blockers.

## 5. Fail-closed Rules

The readiness report must return `not_ready_for_limited_internal_trial` when:

- manual dogfood is missing;
- live provider P0 evidence is missing;
- any P0 task lacks final delivery;
- any P0 write-like action executes directly instead of staying proposal-only or
  blocked;
- any browser/live evidence is local, mock, scripted, or fixture-only while
  claiming external/real evidence;
- hidden legacy fallback count is non-zero;
- silent durable write count is non-zero.
