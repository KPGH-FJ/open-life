# OpenLife ReAct Beta Roadmap

> Version: 2026-05-01
> Status: Beta target baseline
> Scope: Define what "Beta" means for OpenLife as a ReAct-driven personal Agent OS.

> 2026-05-30 alignment note: This roadmap remains the ReAct execution
> seriousness baseline, but implementation order is now governed by
> `plans/openlife_lifemodel_governed_agent_runtime.md`. ReAct is the current
> default strategy, not the final runtime boundary. Tool status in this roadmap
> must stay synchronized with `AGENTS.md` Tool Taxonomy.

## 1. Alignment

OpenLife's Agent architecture is ReAct-oriented. Its execution expectations should therefore be closer to an execution agent framework such as OpenClaw than to a chat app with memory.

The difference is that OpenLife is not a general-purpose remote execution shell. OpenLife is a local-first, LifeModel-aware personal Agent OS. It must use tools to act, but tool execution must be governed by privacy, permission, proposal review, audit, and rollback.

The Beta target is:

```text
User intent
  -> AgentTask
  -> ContextAssembler
  -> ModelRouter
  -> AgentLoop
  -> Tool/Skill planning
  -> ActionExecutor
  -> Observation
  -> Follow-up reasoning
  -> Proposal/Permission/Audit
  -> User Review
  -> Apply/Replay/Rollback
  -> AgentRun Trace
```

Beta is reached only when this loop is trustworthy across the main product paths. A single fixed bug or one additional page does not make the project Beta.

## 2. Stage Definitions

### Alpha+

Alpha+ means the framework skeleton exists:

- AgentRun records exist.
- Proposal/Review Center exists.
- Action and Observation JSON storage exists.
- ToolPermissionStore exists.
- Skill and Plugin manifests exist.
- ModelRouter and ContextAssembler exist in gray state.
- Runs, Review, Settings, and Workspace can expose early trace/control surfaces.

Alpha+ is not enough if the agent cannot reliably act through tools, observe results, request permission, replay approved actions, and explain the run.

### Beta

Beta means OpenLife has a minimum credible ReAct execution kernel:

- The AgentLoop really calls the model, parses intended actions, executes or blocks them, records observations, and produces follow-up output.
- Tools are first-class, not optional attachments.
- Internal OpenLife capabilities are exposed as governed tools.
- High-risk writes and external side effects go through permission or proposal review.
- Review Center approval can unblock and replay the original action.
- Runs can explain what happened, which context/model/tools were used, and what was changed or proposed.

## 3. Required Tool Taxonomy

OpenLife Beta requires a tool layer comparable in seriousness to OpenClaw-style execution systems, but scoped to a personal OS.

### 3.1 Core OS Tools

These tools let the agent operate OpenLife itself:

| Tool | Purpose | Beta behavior |
|---|---|---|
| `life_model.read` | Read selected LifeModel fields | Low-risk read, auditable |
| `life_model.propose_patch` | Propose LifeModel changes | Creates Proposal, never silent write |
| `goal.read` | Read goals and daily actions | Low-risk read |
| `goal.propose_update` | Propose goal/milestone/action changes | Creates Proposal |
| `memory.search` | Retrieve relevant memory | Low-risk read with privacy trace |
| `memory.propose_write` | Propose durable memory | Creates MemoryWrite Proposal |
| `memory.propose_archive` | Propose archiving memory chunks | Creates MemoryArchive Proposal |
| `proposal.create` | Create typed proposals | Validates type/payload |
| `proposal.list` | Inspect pending/resolved proposals | Read-only |
| `agent_run.lookup` | Inspect prior runs | Read-only, redacted where needed |
| `snapshot.create` | Create safety snapshot before high-risk apply | Internal controlled action |

### 3.2 External Execution Tools

These tools let the agent act outside the LifeModel. They are required for Beta because OpenLife is an execution-oriented ReAct system, not only a personal context manager.

| Tool | Purpose | Beta behavior |
|---|---|---|
| `mcp.call_tool` | Call a registered MCP tool | Manifest-based execution, permission checked, audited |
| `a2a.call_agent` | Call a registered A2A agent/capability | Manifest-based execution, permission checked, audited |
| `file.read` | Read user-approved local files | Filesystem capability declared; low/medium risk depending path/scope |
| `file.write_proposal` | Propose writing or modifying local files | Creates ExternalWriteAction Proposal or requires scoped permission |
| `web.search` | Search the web or configured search provider | Network capability declared; privacy routed; source citations retained |
| `web.fetch` | Fetch a specific URL/document | Network capability declared; privacy routed; content summarized/audited |
| `calendar.read` | Read configured calendar context | Read-only connector; explicit account/source scope |
| `calendar.propose_event` | Propose event creation/update | Governance calibration item: must create `ScheduledTask` Proposal only, or be disabled/declarative-only; no silent write and no `ExternalWriteAction` fallback |
| `email.read` | Read configured email context | Read-only connector; explicit account/source scope; privacy filtered |
| `email.propose_draft` | Draft email without sending | Governance calibration item: must create `DataExport`/email-draft Proposal only, or be disabled/declarative-only; must not be misclassified as `ExternalWriteAction`; send is out of Beta unless explicitly governed |
| `task.create_proposal` | Propose a task/reminder/action item | Creates ScheduledTask or Goal/Task Proposal |

Beta does not require every provider integration to be production-grade. It does require the tool contracts, permission model, audit trail, and at least one useful implementation path for each execution class:

- MCP/A2A are the primary generic execution adapters.
- File tools may start as local safe-path tools.
- Web tools may start with search/fetch adapters behind explicit network capability.
- Calendar/email may start as connector stubs that can read configured data or create proposals, but must not pretend to send/write if no executor exists.

Current calibration note:

- `calendar.propose_event` and `email.propose_draft` must not be treated as
  completed P1 until code behavior, proposal payloads, integration tests, and
  Tool Taxonomy agree.
- `ExternalWriteAction` proposal creation must enforce pre-insert size limits
  and payload minimization. This is a hard acceptance gate, not a follow-up
  suggestion.

If an execution tool is not implemented, it must be disabled or clearly marked as `declarative_only`; it must not appear as an executable enabled tool.

### 3.3 Governance Tools

These are not user-facing tricks; they are required for a safe ReAct system:

| Tool | Purpose |
|---|---|
| `permission.check` | Check current policy before execution |
| `permission.request` | Create ToolPermission Proposal |
| `permission.replay_action` | Replay an already-approved blocked action |
| `privacy.inspect` | Classify/redact sensitive data before model/tool use |
| `audit.write` | Record action, observation, and decision |
| `risk.classify` | Decide read/write/network/filesystem/lifemodel risk |

### 3.4 Skill Tools

Built-in skills should be callable by the runtime, not only launched from UI cards:

| Skill | Purpose | Beta behavior |
|---|---|---|
| `skill.weekly_review` | Review recent runs, goals, state, memory | Produces structured output and proposals |
| `skill.goal_breakdown` | Convert goal/text into milestones and actions | Produces Goal proposals |
| `skill.memory_consolidation` | Convert recent interactions into long-term memory candidates | Produces Memory proposals |

### 3.5 Plugin Tools

Beta does not execute remote plugin code. Local plugin manifests may declare tools and skills, but any non-executable declaration must be clearly marked as disabled or declarative-only. A plugin tool must not appear executable unless the registry has a real local executor.

## 4. Beta Gates

### Gate 1: ReAct Execution Core

Goal: make AgentLoop a true execution loop.

Required outcomes:

- ContextAssembler creates context.
- ModelRouter selects model and records route.
- Model generation produces assistant text or structured action requests.
- Parser extracts tool/action requests.
- ActionExecutor executes or blocks.
- Observations are appended to AgentRun.
- Follow-up model call produces final answer.
- Budget limits stop loops safely.
- Recoverable tool failures still produce user-facing final output.

Acceptance scenario:

```text
User asks for a task requiring memory lookup and a tool.
Agent reads context, calls one tool, observes the result, and responds with a final answer.
Runs shows route, action, observation, status, and warnings.
```

### Gate 2: Tool Registry and ActionExecutor

Goal: make tools the agent's reliable hands.

Required outcomes:

- Built-in, MCP, A2A, and plugin-declared tools normalize into one `ToolManifest`.
- Execution tools include the Beta set: `mcp.call_tool`, `a2a.call_agent`, `file.read`, `file.write_proposal`, `web.search`, `web.fetch`, `calendar.read`, `calendar.propose_event`, `email.read`, `email.propose_draft`, and `task.create_proposal`.
- `calendar.propose_event` and `email.propose_draft` remain governance
  calibration items until W1 Tool Proposal Hygiene verifies their proposal
  semantics and taxonomy status.
- Unknown or disabled tools are blocked, not "needs confirmation".
- Allowed tools execute through `execute_manifest`, not bypass paths.
- Plugin tools are declarative-only unless a real executor exists.
- All tool calls create AgentAction and AgentObservation records.
- Unimplemented execution tools are registered as disabled/declarative-only, not as enabled tools that fail at runtime.

Acceptance scenario:

```text
Agent calls a low-risk built-in memory search tool and a registered MCP read tool.
Both are executed through manifest registry and appear in the action timeline.
```

Extended execution acceptance:

```text
Agent can perform at least one real external read through MCP/A2A/web/file,
and can turn file/calendar/email/task writes into proposals instead of silently mutating external systems.
```

### Gate 3: Permission and Replay

Goal: close the high-risk tool loop.

Required outcomes:

- High-risk/write/network/external side-effect actions create ToolPermission Proposal when no policy allows them.
- Review Center accept persists canonical policy scope.
- Chat and Runs keep the blocked action pending until authorized.
- Replay reuses the original action, checks the accepted policy, then executes.
- Deny/disabled/unknown states cannot be fixed by Review Center and are shown as blocked/failed.

Acceptance scenario:

```text
Agent attempts a high-risk tool call.
It is blocked and a ToolPermission Proposal is created.
User accepts it in Review Center.
User returns to Chat or Runs and replays the same action successfully.
```

### Gate 4: LifeModel and Memory Governance

Goal: make internal writes safe and useful.

Required outcomes:

- `memory.propose_write` creates MemoryWrite Proposal.
- `memory.propose_archive` creates MemoryArchive Proposal.
- `life_model.propose_patch` and `goal.propose_update` create typed LifeModel/Goal proposals.
- Accepting proposals writes stores and links back to AgentRun.
- Applying LifeModel changes creates snapshots first.
- Invalid or unsupported proposal types remain pending and fail explicitly.

Acceptance scenario:

```text
Agent suggests a durable memory and a goal update.
Both appear in Review Center.
Accepting them updates Memory/LifeModel and links the result to the original run.
```

### Gate 5: Skill Runtime

Goal: make built-in skills real ReAct capabilities.

Required outcomes:

- `weekly_review`, `goal_breakdown`, and `memory_consolidation` use ContextAssembler and model generation.
- Skills output a JSON envelope with summary, structured output, proposal candidates, and warnings.
- Validation is fail-soft: warnings are recorded, unsafe candidates are skipped.
- Skill runs are AgentRuns with route, context, output, proposals, and parse warnings.

Acceptance scenario:

```text
User runs weekly review.
The skill reads recent AgentRuns, goals, state, and memory.
It generates a structured review and Goal/Memory proposals without directly mutating data.
```

### Gate 6: ModelRouter and Privacy

Goal: make route decisions honest and safe.

Required outcomes:

- Provider health is real or explicitly estimated.
- Missing cloud keys mean unavailable.
- Probe failures record `last_error`.
- High/Critical privacy tasks default local-only.
- No local model for high privacy returns a clear routing error, not cloud fallback.
- AgentRun records provider, model, privacy level, retry count, fallback reason, and health estimation flag.

### Gate 7: Runs as ReAct Trace Viewer

Goal: make execution understandable.

Required outcomes:

- Runs detail shows action timeline, observations, permission decisions, route trace, proposal links, parse warnings, and replay state.
- Failed phases are clear.
- Safe Mode blocks apply/replay paths that mutate data.
- Restore/delete behavior is real or hidden; no UI-only fake recovery.

### Gate 8: Product Shell and Documentation

Goal: present Beta honestly.

Required outcomes:

- Workspace exposes core task/skill entry points.
- Navigation does not imply incomplete tools are production-ready.
- README, AGENTS, PRD, and development plan all agree on Alpha+/Beta meaning.
- `make ci` is the release gate and does not rewrite the worktree.

## 5. Beta Golden Path

OpenLife is Beta-ready when this can be completed end-to-end:

```text
User: Help me review this week and prepare next week's plan.

OpenLife:
1. Creates a Skill AgentRun.
2. Assembles LifeModel, goals, memory, and recent AgentRuns.
3. Routes to an allowed model under privacy policy.
4. Runs weekly_review through the AgentLoop.
5. Uses internal read tools and records observations.
6. Produces structured summary and Goal/Memory proposals.
7. User accepts some proposals and rejects others.
8. Accepted changes create snapshots and update stores.
9. Runs shows context, route, actions, observations, proposals, and final output.
10. Any high-risk tool call requires permission first and can be replayed after approval.
```

## 6. Development Order

Recommended order:

1. Gate 1: ReAct Execution Core.
2. Gate 2: Tool Registry and ActionExecutor.
3. Gate 3: Permission and Replay.
4. Gate 4: LifeModel and Memory Governance.
5. Gate 5: Skill Runtime.
6. Gate 6: ModelRouter and Privacy.
7. Gate 7: Runs as ReAct Trace Viewer.
8. Gate 8: Product Shell and Documentation.

The first three gates should be treated as one engineering spine. UI redesign should wait until this spine is stable.

## 7. Non-Goals for Beta

- No plugin marketplace.
- No remote plugin code execution.
- No fully autonomous background agent swarm.
- No full calendar/email production integration unless implemented as governed tools. Beta still requires the proposal-facing tool contracts so the ReAct runtime can plan against them safely.
- No large visual redesign before the execution spine is stable.
- No silent high-risk LifeModel, memory, filesystem, or external writes.
