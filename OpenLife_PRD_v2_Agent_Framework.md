# OpenLife PRD v2: Personal Agent Framework

> Version: 2026-04-24
> Status: Current product definition
> Supersedes: `OpenLife_Final_PRD.md` for future planning

## 1. Product Definition

OpenLife is a local-first personal Agent framework.

It gives each user a private LifeModel, then uses that LifeModel to guide local and cloud models while they complete tasks, hold conversations, plan, review, call tools, and help the user evolve over time.

OpenLife is not just an app. It is a framework for personal AI execution.

The core product formula is:

```text
OpenLife = LifeModel + Agent Runtime + Local/Cloud Model Router + Memory + Feedback Loop
```

## 2. Product Thesis

Most AI products treat the user as a temporary prompt.

OpenLife treats the user as an evolving model.

The system should understand:

- Who the user is.
- What the user wants.
- What the user can currently do.
- What state the user is in.
- What constraints and preferences matter.
- What has happened before.
- What should be reviewed, updated, or protected.

With this personal context, OpenLife should make AI outputs more useful, more aligned, less generic, and easier to turn into action.

## 3. What OpenLife Is

OpenLife is:

- A personal Agent framework.
- A local-first private context system.
- A LifeModel-driven AI execution environment.
- A hybrid local/cloud model orchestration layer.
- A memory and feedback system.
- A user-controlled system for continuous self-model evolution.
- A foundation for future proactive personal agents.

## 4. What OpenLife Is Not

OpenLife is not:

- A generic ChatGPT wrapper.
- A simple goal tracker.
- A static life dashboard.
- A habit app.
- A pure local LLM client.
- A pure cloud AI client.
- An autonomous system that silently rewrites the user.

## 5. Target Users

### 5.1 Primary User

An individual who wants AI to understand their personal goals, constraints, working style, values, and current state.

They may use OpenLife for:

- Planning.
- Reflection.
- Writing.
- Decision support.
- Daily execution.
- Personal knowledge management.
- LifeModel construction.
- Goal tracking and review.

### 5.2 Secondary User

A builder or researcher who wants a local-first Agent framework with:

- Personal data control.
- Model routing.
- Tool execution.
- LifeModel feedback loops.
- MCP/A2A interoperability.

## 6. Core User Problems

### Problem 1: Generic AI Does Not Know the User

Most AI replies are helpful but generic. Users repeatedly explain their goals, preferences, state, and context.

OpenLife should solve this by maintaining a private LifeModel.

### Problem 2: AI Outputs Are Disconnected From Long-Term Goals

AI can answer a single question, but it rarely understands what matters over weeks, months, or years.

OpenLife should connect daily tasks and conversations to long-term goals.

### Problem 3: Private Context Is Hard To Use Safely

Useful personalization requires sensitive context. Sending everything to cloud models is risky.

OpenLife should route context through local-first privacy policies and selective cloud delegation.

### Problem 4: AI Does Not Learn Safely From Usage

If AI learns silently, users lose control. If AI never learns, personalization stagnates.

OpenLife should propose updates and let users confirm, edit, or reject them.

### Problem 5: Agent Actions Need Traceability

When an agent uses tools or changes memory, users need to know what happened and why.

OpenLife should make every meaningful run traceable.

## 7. Core Concepts

## 7.1 LifeModel

The LifeModel is the user's private structured self-model.

Core dimensions:

| Dimension | Meaning |
|---|---|
| Identity | Values, roles, personality, philosophy, mission, communication style |
| Goals | Daily, short-term, medium-term, long-term, life goals |
| Capabilities | Skills, tools, resources, knowledge domains, constraints |
| State | Current focus, health, emotion, habits, alerts, custom dimensions |

The LifeModel should influence:

- Model prompts.
- Planning.
- Prioritization.
- Reflection.
- Tool decisions.
- Proactive suggestions.
- Memory relevance.

The LifeModel must not be silently overwritten.

## 7.2 AgentTask

An AgentTask is what the user or system wants OpenLife to do.

Examples:

- "Help me plan today."
- "Write this document in my style."
- "Review my week."
- "Build my LifeModel."
- "Summarize these memories."
- "Use a tool to fetch information."

Every significant user request should eventually map to an AgentTask.

## 7.3 AgentRun

An AgentRun is one execution of an AgentTask.

It records:

- User intent.
- Context used.
- Model/provider selected.
- Memory retrieved.
- Actions attempted.
- Tool calls.
- Output.
- Errors.
- Proposed LifeModel or memory updates.

The user should be able to inspect enough of the AgentRun to trust the result.

## 7.4 AgentAction

An AgentAction is something the agent does beyond pure text generation.

Examples:

- Read memory.
- Write memory.
- Patch LifeModel.
- Call MCP tool.
- Send A2A task.
- Create snapshot.
- Archive memory.

Actions must be governed by risk policy.

## 7.5 AgentProposal

An AgentProposal is a suggested change that requires user review or policy-based approval.

Examples:

- Update current focus.
- Add a short-term goal.
- Change communication preference.
- Add a new skill.
- Archive memory.
- Allow a tool call.

Every proposal should include:

- Source.
- Reason.
- Confidence.
- Risk level.
- Before and after.
- Accept / edit / reject / postpone decision.

## 8. Core Product Flows

### 8.1 First-Time LifeModel Construction

Goal:

Help the user construct an initial LifeModel.

Modes:

- Quick Build.
- Incremental Build.
- Socratic Build.

Expected behavior:

- The user answers guided questions.
- OpenLife extracts candidate LifeModel signals.
- Signals become proposals.
- The user reviews and confirms.
- Confirmed fields write to LifeModel.
- A snapshot is created.

Success criteria:

- The user sees what was understood.
- The user can edit or reject important fields.
- The LifeModel file actually changes.
- Chat and Workspace reflect the updated model.

### 8.2 Context-Aware Agent Conversation

Goal:

Let the user talk with an AI that understands their LifeModel and memory.

Expected behavior:

- User sends message.
- OpenLife creates an AgentTask and AgentRun.
- ContextAssembler selects relevant LifeModel and memory.
- ModelRouter selects local or cloud model.
- Agent returns response.
- AgentRun records trace.
- System may generate proposals from the conversation.

Success criteria:

- User can inspect why the answer was personalized.
- User can inspect which model was used.
- Chat history persists.
- User can accept or reject proposed updates.

### 8.3 Task Execution With Tools

Goal:

Let the user use OpenLife as a ReAct-style agent framework.

Expected behavior:

- Agent plans steps.
- Agent proposes or executes allowed actions.
- MCP/A2A tools are governed by permission policy.
- Tool calls are audited.
- Results feed back into the task.

Success criteria:

- Read-only low-risk actions may run smoothly.
- Write/destructive/sensitive actions require confirmation.
- User can inspect what happened.

### 8.4 Daily Planning and Review

Goal:

Help the user align daily actions with goals and state.

Expected behavior:

- OpenLife checks current goals and state.
- It suggests next actions.
- It identifies stale goals, conflicts, or risks.
- It records review outcomes as proposals or memories.

Success criteria:

- The user gets a useful next step.
- Suggestions reference LifeModel context.
- Updates require confirmation when significant.

### 8.5 Continuous LifeModel Evolution

Goal:

Keep the LifeModel current without removing user control.

Expected behavior:

- Conversations, feedback, reviews, and actions produce signals.
- Signals are converted into proposals.
- The user confirms, edits, rejects, or postpones.
- Accepted changes create snapshots and audit records.

Success criteria:

- The LifeModel improves over time.
- The user knows what changed and why.
- Risky changes are never silent.

## 9. Functional Requirements

### 9.1 LifeModel Requirements

- The system must support a structured LifeModel with identity, goals, capabilities, state, preferences, and relationships.
- The system must support snapshots and rollback.
- The system must expose LifeModel completion and readiness state.
- The system must support patch-based updates.
- The system must classify patch risk.
- The system must prevent silent high-risk overwrites.

### 9.2 Agent Runtime Requirements

- The system must represent significant user requests as AgentTasks.
- The system must represent executions as AgentRuns.
- AgentRuns must store model route, context summary, actions, output, errors, and proposals.
- AgentRuns must be queryable after refresh.
- Existing Chat should be the first flow migrated to AgentRun.

### 9.3 Model Routing Requirements

- The system must support local models through Ollama.
- The system must support DeepSeek, OpenAI, OpenRouter, and custom OpenAI-compatible providers.
- The system must distinguish model roles: chat, planner, tool use, summarizer, extractor, embedding.
- The system must record why a model route was selected.
- The system must enforce cloud privacy policy before sending context.

### 9.4 Memory Requirements

- The system must persist chat history.
- The system must support semantic memory search.
- The system must distinguish memory source and sensitivity.
- The system must support memory archive and recovery.
- Memory writes inferred from usage should become proposals when sensitive or high-impact.

### 9.5 Tool and A2A Requirements

- The system must support MCP tool discovery and invocation.
- The system must support A2A interop as a future external agent layer.
- Tool calls must be represented as AgentActions.
- External write/destructive actions must default to confirmation.
- Tool calls must be auditable.

### 9.6 Proposal Requirements

- Builder, Calibration, Evolution, Memory, and proactive suggestions must converge on one proposal model.
- Proposals must include source, reason, confidence, risk level, before/after, and status.
- Users must be able to accept, edit, reject, and postpone.
- Accepted LifeModel proposals must create snapshots.

### 9.7 Proactive Agent Requirements

- The system should support daily and weekly check-ins.
- Proactive suggestions must be explainable.
- Proactive behavior should start as cards or reminders, not intrusive autonomous chat.
- Proactive outputs must create AgentRuns or proposals.

## 10. Non-Functional Requirements

### 10.1 Privacy

- Local-first by default.
- User data should not be sent to cloud providers without routing policy.
- Sensitive context should be redacted or summarized.
- The user should be able to understand what context was used.

### 10.2 Reliability

- The app should not crash on database or config errors.
- Safe Mode should make data problems visible.
- Import/export should avoid partial destructive writes.
- Errors should guide users to recovery paths.

### 10.3 Explainability

- Outputs should expose useful trace summaries.
- Model route should be visible.
- LifeModel changes should include reasons.
- Tool actions should be auditable.

### 10.4 Extensibility

- New providers should be added through provider registry.
- New tools should be integrated through AgentAction.
- New proactive flows should use AgentTask and AgentRun.

## 11. Frontend Product Requirements

The frontend should migrate toward this information architecture:

| Section | Purpose |
|---|---|
| Workspace | Operating surface: readiness, active tasks, next action, pending proposals |
| Agent | Conversation and task execution surface |
| LifeModel | Build, inspect, edit, and version personal model |
| Memory | Search, manage, archive, restore |
| Runs | Execution history and traces |
| Settings | Providers, privacy, recovery, diagnostics |

The current pages can be migrated gradually.

## 12. MVP Scope

The next MVP should not attempt full autonomy.

The next MVP should prove this loop:

```text
User message -> AgentRun -> LifeModel/memory context -> model route -> answer -> proposal -> user confirmation
```

MVP requirements:

- Chat creates AgentRun.
- AgentRun stores context summary and model route.
- Chat displays trace summary.
- Builder and Calibration begin emitting unified proposals.
- Settings remains the control plane.

## 13. Out of Scope for Near-Term Beta

- Multi-device sync.
- Account system.
- Digital legacy.
- Fully autonomous agents.
- Public plugin marketplace.
- Federated social agent network.
- Complex project management suite.

These may be future directions, but they should not block the Agent Framework baseline.

## 14. Success Metrics

### Product Metrics

- User can complete first LifeModel build.
- User can send a personalized Agent message.
- User can inspect why the answer was personalized.
- User can accept or reject LifeModel update proposals.
- User can recover from common configuration/data failures.

### Engineering Metrics

- Chat path creates persistent AgentRuns.
- High-risk LifeModel changes require confirmation.
- Model route is recorded per run.
- Provider diagnostics are visible.
- Main smoke path is stable.

## 15. Beta Readiness Criteria

OpenLife can be considered Beta-ready when:

- First-time user path works without developer explanation.
- Settings -> Builder -> Agent -> Workspace path is coherent.
- Chat, Builder, Calibration, and Memory share proposal semantics.
- AgentRuns are persisted and inspectable.
- ModelRouter supports DeepSeek, OpenAI, OpenRouter, Ollama, and custom compatible providers.
- Safe Mode prevents silent data loss.
- Documentation matches actual behavior.

## 16. Immediate Development Recommendation

Start with:

```text
AgentRun Baseline for Chat
```

Why:

- It introduces the missing architectural spine.
- It does not require a full UI rewrite.
- It makes debugging model calls, context, and persistence easier.
- It prepares the ground for proposals, tool actions, and proactive behavior.

Concrete next deliverables:

- `openlife-core/src/agent/` module.
- `AgentTask`, `AgentRun`, `ModelRouteTrace`, `ContextSummary`.
- `AgentRunStore`.
- Tauri commands for run query.
- Chat integration.
- Minimal Run Trace UI.

## 17. Relationship to Old PRD

`OpenLife_Final_PRD.md` remains useful as a historical record of earlier product thinking.

However, future product and engineering decisions should prioritize this PRD v2 and the architecture document:

- [`plans/openlife_agent_framework_architecture.md`](plans/openlife_agent_framework_architecture.md)
- [`plans/openlife_development_plan.md`](plans/openlife_development_plan.md)

