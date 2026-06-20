# OpenLife Agent Product Capability Matrix v1

> Date: 2026-06-16
> Status: product capability planning artifact for the next Main Chat Agent phase
> Scope: define OpenLife Agent product capability levels, current state, target state, UI/runtime dependencies, acceptance criteria, and benchmark gaps

## 1. Purpose

This document turns the current Main Chat Agent runtime progress into a product
capability contract.

The previous Main Chat Agent Execution work proved that OpenLife can route
ordinary Main Chat through governed sessions, strategies, ReAct/tool execution,
ActionQueue, ExecutionPolicy, proposals, and external live-provider acceptance.
That is a major runtime milestone, but it is not enough to define an excellent
Agent product.

The next phase must be judged by product capability, not only backend readiness.
For each capability, this matrix answers:

- what level of Agent ability it belongs to
- what OpenLife can do now
- what the target state should be
- what the user should see
- which backend objects are required
- which UI states are required
- how to verify it
- where OpenLife still differs from Codex, Hermes, and OpenClaw
- what the capability must prove before it can be called product-complete
- which product objects and UI states must map to real runtime evidence
- which behaviors must never be faked or hidden behind ordinary chat output

## 2. Capability Levels

| Level | Name | Product meaning | Current OpenLife state | Next target |
| --- | --- | --- | --- | --- |
| L0 | Direct Answer | The Agent answers without tools but still runs inside governed Main Chat. | Mostly done at runtime. DirectAnswer is a governed strategy with task/run trace and provider generation evidence. | Make direct answers visibly part of the same Agent control plane without adding noise for simple questions. |
| L1 | Governed Read | The Agent performs one safe read action, observes result, and answers. | Partially done. File read, memory/session search, web blocker/success, MCP read proof exist. Product UI is not complete. | User sees action, observation, source, blocker if any, and final answer in Chat. |
| L2 | Multi-step ReAct | The Agent plans/selects tools, executes, observes, and follows up. | Runtime mostly done for bounded read/tool cases, including provider-ranked MCP live proof. | Turn runtime trace into a product task timeline with retry/cancel/resume controls. |
| L3 | Plan-Execute-Review | The Agent decomposes a goal, executes steps, reviews outcome, and updates next actions. | Partial. PlanExecute draft and old Plan-Execute foundations exist, but Main Chat product loop is not complete. | A task can move from plan to execution to review in one visible session. |
| L4 | Memory/Knowledge Governance | The Agent proposes memory or LifeModel updates with evidence and confirmation. | Partial. Proposal-first constraints exist; bounded knowledge files load as context. Full user-managed memory product is incomplete. | Chat can propose, confirm, reject, revise, and roll back memory/knowledge updates. |
| L5 | Long-running Task Continuity | The Agent can pause, resume, recover, and continue long-running tasks across sessions. | Partial. AgentTaskSession, ActionQueue, retry/resume/cancel exist. Product-level long task UX is incomplete. | User can return later and see task status, pending blockers, last observation, and next action. |
| L6 | Tool/Skill Orchestration | The Agent discovers and invokes tools/skills under policy with clear boundaries. | Partial. MCP read candidates, selected `SKILL.md`, and Skill Runtime foundations exist. Skills Hub/product invocation is incomplete. | User can select/inspect skills, see why a tool was chosen, and approve risky operations. |
| L7 | Controlled Autonomy | The Agent can proactively continue bounded work under user-defined permissions. | Not in current scope. Governance foundations exist, but product autonomy is not implemented. | Future only. Do not build before L1-L5 product UX is reliable. |

### 2.1 First-class Agent Bar

OpenLife should be judged against first-class Agent products by behavior, not by
internal architecture names.

The bar for this phase is:

- Chat is the control surface, but the execution path is an Agent runtime.
- A user request that implies doing work must create a visible task frame unless
  it is intentionally classified as a direct answer.
- Plans, actions, observations, blockers, proposals, and final delivery must be
  visible and backed by runtime transcript evidence.
- Tool execution must be the default experience for tool-required tasks, not an
  experimental preview hidden behind a separate route.
- Memory and knowledge updates must be user-controllable, evidenced, reversible,
  and never silently promoted from assistant text.
- Failure must produce a recoverable product state, not only a backend error or
  generic apology.
- Product completion requires user-visible proof of what was executed, what was
  only proposed, what was blocked, and what still needs user input.

### 2.2 Completion Contract For Every Capability

Every capability in this matrix is incomplete until it defines all of the
following:

| Field | Required meaning |
| --- | --- |
| User job | The user goal this capability serves, written in user language. |
| Supported inputs | Concrete user prompts that must route to this capability. |
| Runtime path | DirectAnswer, ReAct, Plan-Execute, proposal, memory, or blocker path. |
| Product objects | Task, plan, action, observation, blocker, proposal, memory candidate, or final delivery objects created. |
| Visible states | Exact UI states and allowed user controls. |
| Durable changes | What can change permanently, what requires confirmation, and what must remain ephemeral. |
| Failure recovery | Retry, resume, edit plan, ask user, permission request, or terminal blocker behavior. |
| Eval assertions | Automated or manual assertions that prove the product behavior. |
| Benchmark gap | How far the behavior is from Codex/Hermes/OpenClaw-like product experience. |
| Non-fake rule | Which UI claims must be backed by runtime evidence and cannot be inferred from text. |

## 3. Product Capability Matrix

### 3.1 Ordinary Answer

| Item | Definition |
| --- | --- |
| Capability level | L0 Direct Answer |
| Current state | DirectAnswer runs as a governed strategy. It creates task/run trace and records scheduler/provider generation evidence. Legacy fallback is traceable. |
| Target state | Simple questions feel as lightweight as chat, but still retain task/run trace, context source, fallback visibility, and privacy/model-route policy. |
| User should see | Normal answer, optional compact trace indicator, provider/model/source disclosure when expanded, no fake action timeline. |
| Backend dependencies | AgentIngress, StrategyRouter, DirectAnswer strategy, AgentTaskSession, ExecutionTranscript, provider route trace. |
| UI required state | `answering`, `completed`, `fallback_used`, `provider_trace_available`, `context_sources_available`. |
| Acceptance | A simple question through send and stream completes with no tool calls, no silent writes, no legacy hidden bypass, and an expandable trace. |
| Benchmark gap | Codex makes direct answers feel integrated with workspace context; Hermes/OpenClaw keep direct answers subordinate to task execution. OpenLife has runtime governance but still needs a polished low-noise UI trace. |
| Product DoD | Direct answers use the same task/run identity model as other Agent paths, but the UI stays compact. Expanded trace must show route, model/provider, context sources, and no-tool reason. |
| Non-fake rule | Do not render an action timeline when no action was executed. Do not hide legacy fallback behind a normal answer. |

### 3.2 Read-only Tool Execution

| Item | Definition |
| --- | --- |
| Capability level | L1 Governed Read |
| Current state | File read, memory/session search, web read/blocker, fixture-backed web success, registered MCP read, and ToolPermission proposal proof exist. |
| Target state | User asks for a read task; Agent chooses the right read tool, executes safely, shows observation/source, and answers. |
| User should see | Tool selected, why selected, current action, observation preview, final answer, and blocker if policy prevents execution. |
| Backend dependencies | ActionExecutor, ExecutionPolicy, ActionQueue, workspace file resolver, MemoryStore/Session search, MCP registry, web policy. |
| UI required state | `tool_planned`, `tool_running`, `observation_ready`, `blocked_by_policy`, `completed`, `source_preview`. |
| Acceptance | At least 20 product tasks across file/memory/session/web/MCP read pass with visible action and observation; no silent writes. |
| Benchmark gap | Codex is strong at project file/workspace actions; Hermes/OpenClaw emphasize visible action execution. OpenLife has governed read foundations, but UI and tool breadth remain weaker. |
| Product DoD | A read task must show selected tool, policy allowance, running state, source/observation preview, final synthesis, and trace expansion. |
| Non-fake rule | Do not answer from model knowledge while implying a file/web/MCP read occurred. Observation UI must be backed by an ActionExecutor/AgentLoop transcript entry. |

### 3.3 Multi-step ReAct

| Item | Definition |
| --- | --- |
| Capability level | L2 Multi-step ReAct |
| Current state | Governed AgentLoop attempts first, uses metadata-safe candidate contract, exact allowlists, ExecutionPolicy, governed arguments, observation, and follow-up. DeepSeek live ReAct proof passed. |
| Target state | Multi-step read tasks can execute multiple observations without user micromanagement, while preserving clear policy and recovery behavior. |
| User should see | Goal, plan/action list, each action status, observations, final synthesis, retry/cancel controls, and explicit fallback if used. |
| Backend dependencies | AgentLoop, ActionExecutor, ActionQueue, ExecutionTranscript, ReAct runtime, tool selection/ranking, provider live route. |
| UI required state | `planning`, `action_queued`, `action_executing`, `observation_recorded`, `follow_up_generating`, `failed`, `retry_available`, `cancelled`. |
| Acceptance | A task requiring at least two read/observe/follow-up cycles completes in Chat with trace and no hidden fallback. |
| Benchmark gap | Hermes/OpenClaw appear stronger in default "do the task" execution feel. OpenLife now has stronger governance proof, but needs richer multi-step UX and more realistic task coverage. |
| Product DoD | At least two actions can be planned/executed/observed in one task, with each step independently retryable or explainably terminal. |
| Non-fake rule | A final answer after a failed AgentLoop cannot be shown as completed execution unless the fallback is explicitly labeled and supported by a valid fallback path. |

### 3.4 Plan-Execute-Review

| Item | Definition |
| --- | --- |
| Capability level | L3 Plan-Execute-Review |
| Current state | PlanExecute draft and older Plan-Execute product foundations exist. Main Chat can surface draft command paths, but full product loop is not complete. |
| Target state | User can ask for a goal plan, approve/modify it, execute steps, review outcome, and convert learnings into proposals/memory. |
| User should see | Plan outline, editable steps, active step, completed steps, skipped/blocked steps, review summary, next recommendation. |
| Backend dependencies | PlanExecute runtime, AgentTaskSession, ActionQueue, ExecutionTranscript, ProposalStore, Memory/Evidence bridge. |
| UI required state | `plan_draft`, `plan_confirmed`, `step_running`, `step_completed`, `step_blocked`, `review_ready`, `memory_proposal_available`. |
| Acceptance | 10 realistic planning tasks pass from plan creation to at least one executed step and review summary. |
| Benchmark gap | Hermes/OpenClaw are stronger when execution is the default task frame. Codex is strong when plan maps to workspace edits. OpenLife has planning foundations but not yet a convincing Main Chat product loop. |
| Product DoD | A plan can be drafted, edited/confirmed, executed step by step, reviewed, and converted into follow-up tasks or memory/proposal candidates. |
| Non-fake rule | A plan draft is not task completion. The UI must distinguish planned, executing, executed, skipped, blocked, and reviewed states. |

### 3.5 Memory Proposal And Confirmation

| Item | Definition |
| --- | --- |
| Capability level | L4 Memory/Knowledge Governance |
| Current state | Memory/LifeModel update intents are proposal-first. Long-term truth is not silently written. Bounded `USER.md`/`MEMORY.md`/`SOUL.md` context surfaces exist. |
| Target state | Agent proposes memory updates with evidence, confidence, conflict detection, user confirmation, rejection, and rollback path. |
| User should see | Proposed memory, source evidence, why it matters, confidence/conflict indicator, accept/reject/edit controls, rollback history. |
| Backend dependencies | ProposalStore, EvidenceStore, MemoryStore, LifeModel materializer, Accepted Guidance, context loader. |
| UI required state | `memory_candidate`, `evidence_visible`, `needs_confirmation`, `accepted`, `rejected`, `conflict_detected`, `rollback_available`. |
| Acceptance | "Remember this" never writes directly; it creates a reviewable proposal. Rejection does not become memory. Accepted proposal updates the governed surface with provenance. |
| Benchmark gap | Codex memory is simple and controllable; Hermes-style memory is more bounded/curated. OpenLife has stronger governance ambitions but lacks a clean user-facing memory management product. |
| Product DoD | Memory candidates show source evidence, confidence, conflict status, scope, edit controls, accept/reject, and rollback path. Accepted memory must be inspectable later. |
| Non-fake rule | Assistant-generated claims cannot become user facts. Rejected candidates must not appear in long-term memory or knowledge files. |

### 3.6 Long-term Task Recovery

| Item | Definition |
| --- | --- |
| Capability level | L5 Long-running Task Continuity |
| Current state | Task session lifecycle, resume, cancel, retry, permission-preserving replay, and terminal state guards exist. Product experience is incomplete. |
| Target state | User can leave and return to a task; Agent knows last state, blockers, next action, and whether continuation is safe. |
| User should see | Task list/status, last observation, pending permission, next recommended action, resume/retry/cancel buttons. |
| Backend dependencies | AgentTaskSessionStore, ActionQueueStore, ExecutionTranscript, task controls, ProposalStore, permission state. |
| UI required state | `active_task`, `paused`, `waiting_for_user`, `resumable`, `retryable`, `terminal`, `stale_context_warning`. |
| Acceptance | A task blocked by permission can be resumed after acceptance; failed safe read can retry; terminal task cannot be resumed illegally. |
| Benchmark gap | Codex is strong at session/workspace continuation; Hermes/OpenClaw emphasize task continuity. OpenLife has backend lifecycle control but needs a visible task home and recovery UX. |
| Product DoD | A task remains discoverable after navigation/restart, shows last observation and blocker, and resumes only when the stored context is still valid or explicitly refreshed. |
| Non-fake rule | Do not resume from a stale or terminal state without showing why continuation is safe. Do not replay a permission-sensitive action without preserving the original permission scope. |

### 3.7 Tool And Skill Calling

| Item | Definition |
| --- | --- |
| Capability level | L6 Tool/Skill Orchestration |
| Current state | MCP read-only candidate set, selected `SKILL.md` context, Skill Runtime foundations, and plugin declarative-only boundary exist. Product-level Skill Hub is incomplete. |
| Target state | User can choose or let Agent choose a skill/tool; Agent explains selection, loads bounded context, executes under policy, and shows result. |
| User should see | Available relevant skills/tools, selected skill/tool, reason, permission level, action trace, result, failure/blocker. |
| Backend dependencies | SkillManifest/Skill Runtime, selected `SKILL.md` loader, MCP registry, ActionExecutor, ExecutionPolicy, tool permission store. |
| UI required state | `skill_selected`, `tool_candidates`, `tool_selected`, `permission_required`, `tool_result`, `tool_failed`. |
| Acceptance | Selected `SKILL.md` loads only when selected; read-only MCP executes through allowlist; unsafe plugin/write-like tools are blocked or proposal-first. |
| Benchmark gap | Codex Skills are understandable and file-based; Hermes/OpenClaw make tools feel like default execution primitives. OpenLife has policy gates but lacks an ergonomic skill/tool product surface. |
| Product DoD | Relevant tools/skills are inspectable, selectable, explainable, permission-scoped, and auditable. The Agent can choose a tool, but the user can see why. |
| Non-fake rule | Do not inject unselected `SKILL.md` content. Do not expose unsafe/write-like tools as if they are normal read tools. |

### 3.8 Failure Recovery

| Item | Definition |
| --- | --- |
| Capability level | Cross-cutting L1-L5 |
| Current state | Runtime can fail-soft, return blockers, reject missing planned action, retry safe failed actions, and avoid silent fallback. UI recovery is incomplete. |
| Target state | Every failure has a user-understandable reason and a next action: retry, change permissions, edit plan, cancel, or ask user. |
| User should see | Clear blocker reason, failed action, retry button if safe, required permission if blocked, cancel option, diagnostic details behind expand. |
| Backend dependencies | ActionQueue status, blocker metadata, ExecutionPolicy reason codes, task controls, transcript entries. |
| UI required state | `failed`, `blocked`, `retry_available`, `permission_required`, `ask_user`, `cancel_available`, `diagnostics_expanded`. |
| Acceptance | No task fails silently. Every failed eval case exposes a blocker and at least one valid user action or terminal explanation. |
| Benchmark gap | Codex gives concrete failure logs; Hermes/OpenClaw often expose action failures in task timeline. OpenLife has backend blockers, but needs clearer product-facing recovery affordances. |
| Product DoD | Each failure has a reason code, user-facing explanation, affected action, safe next controls, and diagnostics behind expansion. |
| Non-fake rule | Do not convert an execution failure into a confident final answer without preserving the failure state. |

### 3.9 User Permission Control

| Item | Definition |
| --- | --- |
| Capability level | Cross-cutting L1-L7 |
| Current state | ExecutionPolicy, ToolPermission proposal, no silent writes, LocalOnly/privacy preflight, and fail-closed live provider policy exist. |
| Target state | Permissions are understandable, scoped, revocable, and shown exactly when they affect execution. |
| User should see | Requested permission, scope, duration, risk, affected tool, expected action, approve/deny/defer controls. |
| Backend dependencies | ExecutionPolicy, ToolPermissionStore, ProposalStore, PrivacyEngine, ModelRouter, ActionQueue. |
| UI required state | `permission_needed`, `risk_level`, `scope`, `allow_once`, `deny`, `defer`, `permission_applied`. |
| Acceptance | External write/high-risk action never runs silently. Read-only actions run automatically only when policy allows. Permission acceptance resumes the exact pending action. |
| Benchmark gap | OpenLife is stronger on governance than many products, but weaker on making that governance easy for users to understand and act on. |
| Product DoD | Permission requests include action, tool, target, risk, scope, duration, and exact consequence of approve/deny/defer. |
| Non-fake rule | Approval must apply to the pending action only. A broad approval UI cannot silently grant unrelated future actions. |

### 3.10 Final Task Delivery

| Item | Definition |
| --- | --- |
| Capability level | Cross-cutting L0-L5 |
| Current state | Runtime final answer/follow-up synthesis exists. Task terminal summaries exist in backend. Product delivery state is not mature. |
| Target state | Every task ends with a clear result, what was done, what was not done, sources/actions used, and any pending follow-up. |
| User should see | Final answer, completed actions, key observations, unresolved blockers, created proposals, memory changes pending/accepted, next step. |
| Backend dependencies | ExecutionTranscript, AgentRun finalization, ActionQueue terminal state, ProposalStore, Memory/Evidence trace. |
| UI required state | `final_answer`, `actions_completed`, `sources_used`, `proposals_created`, `pending_items`, `next_steps`. |
| Acceptance | For each product task, final delivery distinguishes executed work, proposed work, blocked work, and user-required next steps. |
| Benchmark gap | Codex is strong at concrete deliverables; Hermes/OpenClaw are strong at "task done" perception. OpenLife needs final delivery UX that proves completion rather than only replying. |
| Product DoD | Final delivery includes result, actions completed, sources/observations used, proposals created, blockers left unresolved, and recommended next action. |
| Non-fake rule | Do not claim "done" for proposed, blocked, skipped, or unexecuted work. |

### 3.11 Context And Knowledge Management

| Item | Definition |
| --- | --- |
| Capability level | L4-L6 |
| Current state | Controlled loader supports bounded `AGENTS.md`, `SOUL.md`, `USER.md`, `MEMORY.md`, and selected `SKILL.md`. It does not treat them as unrestricted truth. |
| Target state | Users can inspect, edit, propose, accept, reject, and roll back knowledge surfaces while preserving evidence/proposal governance. |
| User should see | Active context sources, selected skill, memory snapshot, pending knowledge proposals, provenance and rollback. |
| Backend dependencies | ContextCompiler, context loader, ProposalStore, EvidenceStore, MemoryStore, accepted guidance materializer. |
| UI required state | `context_sources`, `selected_skill`, `memory_snapshot`, `knowledge_proposal`, `provenance`, `rollback`. |
| Acceptance | `SKILL.md` full content loads only when selected; knowledge files affect behavior only as bounded context; durable truth still needs proposal/evidence path. |
| Benchmark gap | Codex has strong `AGENTS.md`/Skills/Memories mental model. OpenLife has pieces but has not yet made them first-class user-manageable assets. |
| Product DoD | Users can inspect active context sources, edit user-owned surfaces through governed flows, and understand precedence between files, memory, and runtime policy. |
| Non-fake rule | Knowledge files are context surfaces, not unrestricted canonical truth. They cannot override privacy, model routing, tool policy, or proposal requirements. |

### 3.12 User Trust And Reviewability

| Item | Definition |
| --- | --- |
| Capability level | Cross-cutting |
| Current state | Metadata-safe traces, eval reports, proposal records, and transcript entries exist. User-facing trust surface is incomplete. |
| Target state | User can audit what the Agent did, why, with which context/tools, and what changed. |
| User should see | Execution timeline, context sources, tool calls, observations, policy decisions, proposals, final changes, rollback affordances. |
| Backend dependencies | ExecutionTranscript, AgentRun trace, ProposalStore, EvidenceStore, policy metadata, live provider trace. |
| UI required state | `timeline`, `trace_expanded`, `context_expanded`, `policy_decision`, `change_summary`, `rollback_available`. |
| Acceptance | A non-technical user can answer: what did OpenLife do, what did it not do, what is waiting on me, and what changed permanently. |
| Benchmark gap | Codex is transparent for code/task traces; Hermes/OpenClaw emphasize action status. OpenLife has strong metadata but needs a product-grade audit surface. |
| Product DoD | The audit surface can reconstruct the task from user request to final delivery, including context, model route, tool calls, observations, policy decisions, proposals, and durable changes. |
| Non-fake rule | A trace panel must not display inferred actions or inferred sources that are not present in runtime evidence. |

### 3.13 Agent Control Plane Object Model

Main Chat must become an Agent control plane. The UI cannot be a separate
mocked timeline; it must render real runtime objects.

| Product object | Required meaning | Runtime evidence source | User-facing controls |
| --- | --- | --- | --- |
| TaskSession | The unit created for a user request that may answer, plan, act, propose, or block. | AgentTaskSession / AgentRun ids. | Open, continue, cancel, inspect trace. |
| Plan | Ordered or editable intended work. | PlanExecute draft or AgentLoop plan trace. | Confirm, edit, skip step, execute. |
| Action | A concrete tool or governed runtime action. | ActionQueue entry or AgentLoop action. | Retry if safe, cancel if pending, inspect policy. |
| Observation | Result of an executed action. | ExecutionTranscript observation. | Expand source, cite in final answer. |
| Blocker | A policy, missing input, missing tool, or execution failure. | ExecutionPolicy/blocker metadata. | Approve, deny, provide info, retry, cancel. |
| Proposal | A user-reviewable change request. | ProposalStore entry. | Accept, reject, edit, defer, rollback if applied. |
| MemoryCandidate | A proposed memory/knowledge update with evidence. | EvidenceStore / ProposalStore / Memory pipeline. | Accept, reject, edit, set scope. |
| FinalDelivery | Terminal task summary. | AgentRun finalization + transcript summary. | Review, continue, create follow-up. |

### 3.14 Agent Control Plane State Machine

The first product implementation should support this minimum state machine:

```text
idle
  -> classifying
  -> answering
  -> planning
  -> waiting_for_user
  -> queued
  -> executing
  -> observing
  -> synthesizing
  -> proposal_pending
  -> completed
  -> blocked
  -> failed
  -> cancelled
```

Required state rules:

- `answering` may complete without tool UI, but must retain trace expansion.
- `planning` must show plan draft or plan reason, not only hidden metadata.
- `waiting_for_user` must expose the exact missing input or permission.
- `executing` must map to a real queued/running action.
- `observing` must map to a real action result.
- `proposal_pending` must not be rendered as completed durable change.
- `completed` must include final delivery, not only a plain assistant message.
- `blocked` and `failed` must provide a valid next control or terminal reason.
- `cancelled` must stop pending work and preserve audit history.

### 3.15 Knowledge Format Lifecycle

OpenLife should learn from Codex-style file-backed instructions and memory, but
must preserve OpenLife's stronger governance model.

| Surface | Purpose | Location principle | Source of truth | Mutation path | Runtime use |
| --- | --- | --- | --- | --- | --- |
| `AGENTS.md` | Project/runtime instructions for agents working in a workspace. | Workspace root or configured project root. | User/project-authored file. | Direct user edit or explicit proposal flow if edited by Agent. | Bounded instruction context. |
| `USER.md` | Short, readable user preference/profile snapshot. | Global user knowledge directory and optional project override. | Materialized from accepted memory/guidance plus user edits. | Proposal-first for Agent changes; user can edit directly with provenance refresh. | Bounded preference context. |
| `MEMORY.md` | Curated long-term memory summary. | Global user knowledge directory and optional scoped memory folder. | Accepted memory materialization, not raw transcript. | Proposal/evidence/confirmation/rollback. | Bounded memory context. |
| `SOUL.md` | Stable user values, goals, identity-level guidance. | Global user knowledge directory only unless explicitly scoped. | High-confidence accepted guidance. | High-friction proposal with evidence and rollback. | Bounded high-priority context, never policy override. |
| `SKILL.md` | Human-readable skill workflow/instructions. | Skill package or user skill directory. | Skill author/user package. | Skill install/update flow, not automatic memory. | Loaded only when selected or explicitly routed. |
| Session search | Recent conversation/task context. | Database/transcript store. | Raw task/session history. | Append-only transcript; summaries are derived. | Retrieval context with source labels. |
| Evidence graph | Evidence for beliefs, preferences, and proposals. | Database. | Events, accepted/rejected proposals, user corrections. | Governed extraction and proposal outcomes. | Supports confidence/conflict/provenance. |

Hard rules:

- Raw transcript is not long-term memory.
- Assistant output is not user truth.
- File-backed knowledge is inspectable context, not a way around governance.
- Runtime policy always outranks knowledge files and skill instructions.
- Every Agent-authored durable knowledge change needs provenance and rollback.

## 4. Benchmark Gap Summary

| Benchmark | Where it is strong | OpenLife current gap | What to learn |
| --- | --- | --- | --- |
| Codex | Workspace-aware execution, project instructions, skills, terminal/file actions, concrete deliverables, session continuity. | OpenLife has less mature tool/workspace product UX and fewer concrete deliverable flows. | Make knowledge formats and workspace/task execution first-class, visible, and controllable. |
| Hermes | Execution-first task experience, user can see work happening, task/action orientation. | OpenLife runtime now executes, but Main Chat UI still does not fully feel like an Agent control plane. | Put task timeline, blockers, observations, and final delivery at the center. |
| OpenClaw | Tool/action orchestration and practical "do things" posture. | OpenLife is still conservative and eval-heavy; tool breadth and product affordances are limited. | Expand a small number of real tool workflows with strong UI and policy. |
| OpenLife advantage | Governance, proposal-first memory/LifeModel updates, metadata-safe traces, no silent write discipline. | Governance is not yet translated into easy product controls. | Keep governance, but make it visible and ergonomic instead of feeling like friction. |

### 4.1 Detailed Benchmark Gap Map

| Area | First-class Agent expectation | Current OpenLife gap | Required evolution |
| --- | --- | --- | --- |
| Default entry | Chat request automatically becomes the right task strategy. | Runtime is routed, but product still risks feeling like ordinary chat. | Main Chat must visibly classify and run DirectAnswer/ReAct/Plan/Proposal/Blocker paths. |
| Workspace/project context | Project instructions, files, sessions, and skills are first-class. | Context loader exists, but management UX is weak. | Add active context drawer, source visibility, selected skill control, and knowledge provenance. |
| Tool execution | Tool-required tasks execute by default with visible actions. | Read/tool foundations exist; breadth and UI are incomplete. | Finish L1/L2 product UX for file, memory/session, web, MCP, and selected skill actions. |
| Task timeline | User sees goal, plan, current action, observations, blockers, final result. | Backend transcript is stronger than UI. | Build task panel from runtime evidence, not a decorative timeline. |
| Failure recovery | Failures produce concrete next actions. | Blockers exist but are not yet product-grade. | Add retry/resume/permission/edit/cancel controls tied to blocker reason codes. |
| Memory control | Memory is readable, editable, confirmed, scoped, and reversible. | Proposal-first exists; user-facing memory product is incomplete. | Build memory proposal cards, review center links, accepted memory view, and rollback history. |
| Skill/tool ecosystem | Skills are understandable, selectable, inspectable, and safe. | `SKILL.md` can load, but hub/product invocation is missing. | Start with a narrow Skill Hub: selected skills, safe read tools, permission preview, action trace. |
| Session continuity | User can resume real tasks, not only continue chat. | Task state exists; product home is incomplete. | Add active/recent task list, stale context warning, and continuation summary. |
| Final deliverable | Completion is concrete and auditable. | Final answer exists but can look like normal reply. | Add final delivery block with executed/proposed/blocked/pending sections. |
| Trust and audit | User can understand what changed and why. | Metadata exists but is not surfaced cleanly. | Add trace expansion with context, model route, policy, action, observation, proposal, and durable changes. |

## 5. Next Phase Product Target

The next phase should not be "more backend gate" as the primary objective.

Recommended phase:

**Main Chat Agent Productization v1**

Goal:

> A user can give OpenLife a task in Main Chat, watch it plan/act/observe,
> handle permissions or proposals inline, recover from failure, and receive a
> clear final delivery.

Minimum next-phase target:

- L0 Direct Answer product trace is polished and low-noise.
- L1 Governed Read is product-complete for file, memory/session, web, and MCP read.
- L2 Multi-step ReAct is visible, controllable, and recoverable.
- L3 Plan-Execute-Review works for a narrow set of planning tasks.
- L4 Memory proposal/confirmation works from Chat to Review and back.
- L5 task resume/retry/cancel is visible and usable.

Explicit non-targets for the next phase:

- broad autonomous proactivity
- dangerous external writes
- large tool marketplace expansion
- automatic prompt/self-evolution
- replacing governance with prompt-only safety

### 5.1 Phase Boundaries

This phase should be ambitious on product experience but narrow on autonomy.

In scope:

- Main Chat Agent control plane.
- Product-complete L0-L2 for ordinary answer, read-only tool execution, and
  multi-step ReAct.
- Narrow L3 Plan-Execute-Review loop.
- Chat-to-review memory proposal flow.
- Long-task resume/retry/cancel visibility.
- Narrow Skill/Tool selection surface for bounded read and selected `SKILL.md`.
- Product eval scenarios and UI assertions.

Out of scope:

- Autonomous background work without explicit user-defined permission.
- Broad write tools or external side effects.
- Marketplace-scale plugin ecosystem.
- Self-modifying prompts, hidden self-evolution, or automatic policy edits.
- Memory ingestion from chat without evidence/proposal boundaries.

### 5.2 Hard Product Boundaries

These boundaries are non-negotiable:

- A proposal is not completion.
- A plan is not execution.
- A final answer is not proof of tool use.
- A UI timeline is invalid unless backed by runtime evidence.
- A memory candidate is not durable memory.
- A knowledge file is not higher priority than privacy/model/tool policy.
- A fallback answer must be labeled as fallback.
- A blocked task must remain visibly blocked until the blocker is resolved.
- A permission approval must be scoped to the exact pending action.
- A completed task must show what was done, not just what the model said.

## 6. Product-level Acceptance Gate

The next phase needs a new product-level gate in addition to existing runtime
tests.

Required scenario set:

- 10 ordinary answer scenarios
- 20 deterministic read-only tool execution scenarios across file,
  memory/session, fixture web, and MCP
- 10 multi-step ReAct scenarios
- 10 plan-execute-review scenarios
- 10 memory proposal/confirmation scenarios
- 8 long-task resume/retry/cancel scenarios
- 8 permission/blocker scenarios
- 8 tool/skill selection scenarios
- 8 final delivery/reviewability scenarios
- optional external live read/tool scenarios, opt-in only and excluded from the
  default deterministic pass rate

Minimum pass criteria:

- 90% of supported tasks show correct visible state transitions.
- 90% of tool tasks show action and observation before final answer.
- 100% of write-like tasks avoid silent durable writes.
- 95% of permission cases show correct approve/deny/defer behavior.
- 85% of resumable tasks resume from the correct state.
- 90% of final deliveries distinguish executed, proposed, blocked, and pending work.
- 0 cases where UI implies execution when only a proposal was created.
- 0 cases where bounded knowledge files become unrestricted canonical truth.

### 6.1 Scenario Contract Template

Each product eval scenario must use this structure:

| Field | Required content |
| --- | --- |
| Scenario id | Stable id, capability level, and supported/unsupported marker. |
| User prompt | Exact user input. |
| Expected strategy route | Canonical route such as `direct_answer`, `read_action`, `react_tool_execution`, `plan_execute`, `memory_proposal`, `permission_request`, `task_control`, or `blocked`. |
| Expected tools/actions | Tool/action names, allowed targets, and whether execution is automatic or permissioned. |
| Expected UI states | Ordered visible states and required controls. |
| Expected observations | Source/observation requirements and citation rules. |
| Expected durable changes | None, proposal only, accepted memory, or other governed change. |
| Expected final delivery | What must be included in the final result. |
| Negative assertions | What must not happen, such as silent write, fake execution, hidden fallback, or unselected skill injection. |
| Benchmark note | Which first-class Agent behavior this scenario is meant to approach. |

### 6.2 Required Scenario Inventory

The first scenario set should be concrete before implementation starts.

| Group | Minimum scenarios | Required coverage |
| --- | --- | --- |
| Ordinary answer | 10 | No-tool questions, context-aware questions, privacy/model route trace, fallback visibility. |
| File read | 5 | Explicit path, workspace-scoped search, missing file, outside-workspace blocker, source preview. |
| Memory/session read | 5 | Recall accepted preference, search recent session, conflicting memory, no-memory blocker, source disclosure. |
| Web read | 5 deterministic + optional live opt-in | Fixture-backed successful read, network-policy blocker, source preview, stale/failure recovery, no fake web claim; latest/live web belongs only to opt-in. |
| MCP read | 5 | Registered read success, missing manifest blocker, multi-candidate selection, permission proposal, unsafe manifest block. |
| Multi-step ReAct | 10 | At least two observations, tool selection explanation, retry, blocker, final synthesis with sources. |
| Plan-Execute-Review | 10 | Draft plan, edit plan, execute one step, blocked step, review summary, follow-up task. |
| Memory proposal | 10 | Remember-this, correction, conflict, reject, edit, accept, rollback, scoped memory. |
| Permission/blocker | 8 | Allow once, deny, defer, policy block, missing input, external write proposal, exact action replay. |
| Skill/tool selection | 8 | Selected `SKILL.md`, unselected skill not loaded, skill reason, safe tool, unsafe tool block. |
| Long task | 8 | Resume after permission, retry failed read, stale context warning, cancel queued action, terminal no-resume. |
| Final delivery | 8 | Executed/proposed/blocked/pending separation, source/action summary, continuation recommendation. |

### 6.3 Product Eval Assertions

The product eval should assert both runtime and UI behavior:

- Runtime created the expected task/run/session ids.
- Strategy route matches expected strategy.
- Every displayed action maps to a transcript/action-queue entry.
- Every displayed observation maps to an observation record.
- Every displayed proposal maps to a proposal record.
- Every durable memory/knowledge change has accepted proposal provenance.
- UI state order matches the scenario contract.
- User controls are present only when valid for the current state.
- Final delivery separates executed, proposed, blocked, pending, and next-step
  items.
- Negative assertions pass with zero tolerance for silent writes, fake
  execution, fake observations, hidden fallback, or unselected skill context.

## 7. Current Priority Order

### Phase A: Freeze Product Contract

1. Finalize this capability matrix with object model, state machine, scenario
   contract, and hard boundaries.
2. Write the first product eval scenario file using the scenario contract.
3. Define UI event/state payloads that map to runtime transcript evidence.

### Phase B: Build Agent Control Plane

1. Build execution-first Main Chat task panel from real task/session/action data.
2. Render DirectAnswer as low-noise governed output.
3. Render ReAct actions, observations, blockers, and final delivery from runtime
   evidence.
4. Add trace expansion for context, provider/model route, policy, tools, and
   durable changes.

### Phase C: Complete L1-L2 Product UX

1. Complete read-only tool execution UX for file, memory/session, web, and MCP.
2. Complete multi-step ReAct UX with retry/cancel/resume controls.
3. Add product-level UI assertions for all supported read/tool scenarios.
4. Keep fallback visible and fail closed where runtime evidence is missing.

### Phase D: Productize Proposal, Memory, And Permission

1. Make permission/proposal blockers actionable inside Chat.
2. Build memory proposal cards with evidence/confidence/conflict/edit controls.
3. Link Chat proposals to Review Center and accepted memory/knowledge surfaces.
4. Add rollback/provenance visibility for accepted changes.

### Phase E: Continuity And Skill Surface

1. Complete long-task resume/retry/cancel UX and recent task list.
2. Add stale context warning and continuation summary.
3. Add narrow skill/tool selection UI with bounded `SKILL.md` context.
4. Expand provider/tool coverage only after the core product flow is stable.

## 8. Completion Meaning

OpenLife should not claim "excellent Agent product" merely because backend
runtime and live-provider gates pass.

OpenLife can claim **Main Chat Agent Productization v1** only when:

- users can see and control execution
- tool actions produce visible observations
- blockers/proposals are actionable
- memory changes are confirmed and reversible
- tasks can resume after interruption
- final delivery proves what was actually done
- product-level eval passes with the criteria above

## 9. Required Additions Before Development Starts

Before coding the next phase, these artifacts should exist:

1. Product scenario set with at least the scenario inventory in section 6.2.
2. UI state payload contract for Agent control plane rendering.
3. Runtime-to-UI evidence mapping for task, plan, action, observation, blocker,
   proposal, memory candidate, and final delivery.
4. Knowledge format lifecycle spec for `AGENTS.md`, `USER.md`, `MEMORY.md`,
   `SOUL.md`, selected `SKILL.md`, session search, and evidence graph.
5. Permission UX spec with scoped approve/deny/defer behavior.
6. Memory proposal UX spec with confidence, conflict, edit, accept/reject, and
   rollback.
7. Product eval harness plan covering runtime assertions and UI assertions.
8. Explicit non-goals and anti-fake rules copied into the development prompt.

## 10. Readiness To Claim First-class Agent Experience

OpenLife is not ready to claim first-class Agent product experience until:

- Main Chat behaves as an Agent control plane by default.
- Supported tool-required tasks visibly execute through governed runtime paths.
- Plans, actions, observations, blockers, proposals, and final delivery are not
  just text; they are product objects.
- Users can inspect and control memory/knowledge changes.
- Users can recover from common failure states without restarting the task.
- Skill/tool selection is understandable and policy-scoped.
- Product eval passes with realistic user tasks, not only backend fixtures.
- The product can honestly show what was done and what was not done.
