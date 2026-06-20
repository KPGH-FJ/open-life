# OpenLife Frontend Product Shell Rewrite Plan

> Status: planned product-surface rewrite entry
> Baseline: W150-W158 Skill Runtime Beta Maturity complete; default Chat remains `legacy_stream`
> Prototype reference: `/Users/fujing/Desktop/openlife-companion-stage-prototype`
> Scope: frontend product shell, IA, visual system, and existing-command wiring only
> CLI Goal-mode spec: `plans/frontend_product_shell_rewrite_goal_spec.md`

## 1. Purpose

OpenLife has enough backend governance, read-model contracts, Proposal-first
mechanics, Builder flow, Chat, Runs, Settings, and Review surfaces to begin a
serious frontend productization pass.

The current React frontend is still organized like an engineering control
plane:

- Workspace
- Agent
- Review
- Runs
- Settings
- Builder / Memory / MCP / A2A / Metrics as separate utility pages

The current prototype establishes a stronger product mental model:

- `陪伴`: the daily human-Agent relationship surface
- `今日`: the current day and one meaningful goal
- `Life Model`: the user's personal model, including build, overview, memory,
  evidence, and provenance
- `邮箱`: confirmations, proposals, permission requests, and reviewable updates

This plan turns that prototype direction into an executable React/Tauri rewrite
plan without changing backend authority or default Chat routing.

For implementation, use this document as the product/IA plan and use
`plans/frontend_product_shell_rewrite_goal_spec.md` as the coding-ready CLI
Goal-mode spec.

## 2. Product Thesis

OpenLife should not feel like a dashboard or a generic ChatGPT wrapper. It
should feel like a local-first personal Agent that:

- stays with the user through a lightweight companion surface;
- helps the user reduce life/work noise into a small next step;
- builds and maintains a governed Life Model over time;
- asks for confirmation like a person would ask for confirmation, not like a
  settings panel;
- makes memory, evidence, privacy, and proposal-first governance visible without
  overwhelming the ordinary user.

The frontend rewrite should therefore prioritize product structure and user
mental model before advanced runtime migration.

## 3. Hard Constraints

This rewrite must not:

- migrate default Chat away from `legacy_stream`;
- modify `send_message` or `start_stream_message` routing;
- treat W19-W158 readiness/status/proof reports as migration permission;
- call W144-W146 golden path helpers from ordinary Chat;
- call W147-W149 contract/final-gate helpers from ordinary Chat;
- call W150-W158 Skill Runtime readiness/status/run helpers from ordinary Chat;
- silently write durable LifeModel, Memory, file, calendar, email, external
  provider, or plugin state;
- convert prototype mock data into real product truth;
- remove Settings, Runs, MCP, A2A, Metrics, or diagnostic surfaces without a
  separate review;
- paste the prototype CSS wholesale into `frontend/src`;
- hide governance, proposal, privacy, or safe-mode warnings.

The rewrite may:

- reorganize the primary user-facing navigation;
- create new React components and pages;
- restyle existing frontend surfaces;
- call existing Tauri commands through `frontend/src/tauri.ts`;
- keep developer/governance pages as secondary routes;
- map existing Review proposals into a mailbox-style UI;
- map existing Builder and LifeModel data into a Life Model product surface;
- map existing Chat stream states into an Agent stage state.

## 4. Target Information Architecture

### Primary User Navigation

1. `陪伴`
   - Product role: daily relationship surface.
   - Owns: Agent status stage, ordinary Chat thread, composer, lightweight
     suggested replies/actions.
   - Backend: existing Chat commands only.

2. `今日`
   - Product role: one-day focus, not a project dashboard.
   - Owns: current daily goal, next action, tiny status summary.
   - Backend: existing daily goal/state commands.

3. `Life Model`
   - Product role: build, inspect, and understand the private model.
   - Owns: build methods, overview, evidence/memory, provenance, pending
     confirmations.
   - Backend: existing Builder/LifeModel/Memory/Proposal commands first.

4. `邮箱`
   - Product role: human-like confirmation center.
   - Owns: pending proposals, permissions, memory/LifeModel updates, replies,
     archive/postpone/reject/accept actions.
   - Backend: existing Proposal Review commands.

### Secondary / Developer Navigation

Keep these available, but not as the ordinary user's first mental model:

- Runs
- Settings
- MCP
- A2A
- Metrics
- Version Control
- Calibration

Recommended placement: secondary menu, Settings section, or developer drawer.

## 5. Component Plan

Create a product shell instead of continuing to grow page-specific layouts.

Recommended components:

- `ProductShell`
  - top-level layout and primary tabs;
  - safe-mode banner placement;
  - secondary navigation access.

- `MainTabs`
  - `陪伴 / 今日 / Life Model / 邮箱`;
  - stable route mapping.

- `CompanionPage`
  - composed of `AgentStage`, `ChatThread`, and `ChatComposer`;
  - keeps current ordinary Chat behavior.

- `AgentStage`
  - maps Agent UI states to visual states;
  - states: `idle`, `listening`, `sorting`, `memory`, `planning`, `review`,
    `privacy`, `error`;
  - first implementation can use CSS/SVG/bitmap assets from the prototype;
  - no raw tool/prompt payload display.

- `TodayPage`
  - one daily goal;
  - one next action;
  - minimal progress/status.

- `LifeModelPage`
  - tabs: `构建 / 概览 / 依据`;
  - wraps existing Builder and LifeModel data;
  - memory belongs under `依据`, not as a top-level product tab.

- `MailboxPage`
  - mail-like Proposal Review replacement;
  - `MailSidebar`, `MailList`, `MailReader`, `QuickReplyActions`;
  - supports accept, reject, edit, postpone, archive-like affordances mapped to
    existing proposal commands.

- `SecondaryToolsMenu`
  - links to Runs, Settings, MCP, A2A, Metrics, Versions, Calibration.

## 6. Data Wiring Principles

Do not invent new backend contracts during the first rewrite pass.

Use existing commands first:

- Chat:
  - `startStreamMessage`
  - `getChatHistory`
  - `listChatSessions`
  - `createChatSession`
  - `renameChatSession`
  - `deleteChatSession`
  - `saveFeedback`

- Mailbox / Proposal Review:
  - `listProposals`
  - `acceptProposal`
  - `rejectProposal`
  - `postponeProposal`
  - `editProposal`
  - `batchAcceptLowRiskProposals`

- Life Model:
  - `getLifeModel`
  - `getModel4DCompletion`
  - `builderStart`
  - `builderStep`
  - `builderListUnfinished`
  - `builderCreateProposals`

- Today:
  - `getDailyGoals`
  - `addDailyGoal`
  - `toggleDailyGoal`
  - `recordState`

- Status / safety:
  - `getSystemDiagnostics`
  - `getConfig`

If a desired prototype surface has no current backend contract, render it as a
clearly mock/placeholder UI only inside development scope, or defer it. Do not
wire fake data as if it were product truth.

## 7. Agent Stage State Mapping

The prototype's left visual should become a reusable state-driven surface.

Initial state mapping:

| UI State | Trigger Source | Visual Meaning |
| --- | --- | --- |
| `idle` | no active request | OpenLife is present and waiting |
| `listening` | composer focused or user message submitted | user is being heard |
| `sorting` | stream started, no final answer yet | information is being organized |
| `memory` | response/proposal references memory or LifeModel update | memory/evidence is being checked |
| `planning` | prompt or response indicates planning/goal/day structure | next step is being compressed |
| `review` | pending proposal generated or Review link suggested | user confirmation is needed |
| `privacy` | safe-mode, permission, local-only, external write, or sensitive warning | boundary/permission is active |
| `error` | send/runtime error | user needs repair path |

First pass implementation may infer state from existing UI events and available
metadata. It must not consume W137-W158 accepted guidance in ordinary Chat or
change default Chat routing.

## 8. Rewrite Phases

### Phase FPR-0: Product Contract Prep

Goal: make the rewrite safe to execute.

Tasks:

- Create a small product IA spec in code comments or tests for route mapping.
- Identify which old routes remain secondary.
- Define shared visual tokens in a scoped CSS file or Tailwind layer.
- Decide asset location for the cat/stage image.
- Add tests for route labels and basic rendering.

Acceptance:

- No backend command changes.
- Existing tests still pass.
- The app still builds.

### Phase FPR-1: Product Shell

Goal: replace the top-level user-facing navigation with the prototype IA.

Tasks:

- Add `ProductShell`.
- Add primary tabs: `陪伴`, `今日`, `Life Model`, `邮箱`.
- Move Runs/Settings/MCP/A2A/Metrics into a secondary access pattern.
- Preserve safe-mode and beta-readiness banners.

Acceptance:

- User sees the four primary product entries.
- Existing routes remain reachable.
- `frontend` tests and typecheck pass.

### Phase FPR-2: Companion Page

Goal: turn Chat into a companion surface without changing Chat runtime.

Tasks:

- Create `CompanionPage`.
- Extract or wrap existing Chat behavior.
- Add `AgentStage`.
- Map current UI events to stage states.
- Keep `startStreamMessage` as the ordinary send path.

Acceptance:

- Sending a message still uses existing ordinary Chat path.
- No default Chat migration.
- Agent stage changes state without blocking Chat.
- Errors still show actionable repair paths.

### Phase FPR-3: Mailbox Page

Goal: replace engineering-style Proposal Review with a mail-like confirmation
surface.

Tasks:

- Create `MailboxPage`.
- Render proposals as messages.
- Render selected proposal as a readable email.
- Map quick replies to accept/edit/reject/postpone flows.
- Preserve unsupported-type and safe-mode protections.

Acceptance:

- Pending proposals can still be accepted/rejected/postponed/edited.
- High-risk/unsupported proposals are not silently applied.
- Proposal filters remain available or are represented as mailbox folders.

### Phase FPR-4: Life Model Page

Goal: consolidate Builder, model overview, memory/evidence, and pending
confirmations under one product entry.

Tasks:

- Create `LifeModelPage` with `构建 / 概览 / 依据`.
- Reuse Builder commands for build sessions.
- Show model completion/readiness without overclaiming precision.
- Move memory/evidence into `依据`.
- Link pending confirmations to `邮箱`.

Acceptance:

- Existing Builder flow still works.
- Proposal-first Builder review remains intact.
- Memory/evidence is readable but not silently mutable.

### Phase FPR-5: Today Page

Goal: make Today a focused daily surface, not a dashboard.

Tasks:

- Render one current daily goal.
- Render one next action.
- Render minimal progress/status.
- Avoid large explanatory copy and fake metrics.

Acceptance:

- User can understand today's focus within one screen.
- Existing daily goal commands still work.

### Phase FPR-6: Visual QA And Hardening

Goal: make the rewrite production-grade.

Tasks:

- Desktop and mobile visual pass.
- Keyboard/focus pass.
- Reduced-motion pass.
- Empty/loading/error states.
- Safe-mode and no-backend states.
- Tests for primary route rendering and key proposal/chat actions.

Acceptance:

- `cd frontend && pnpm run typecheck`
- `cd frontend && pnpm test -- --run`
- `cd frontend && pnpm run build`
- Manual check in Tauri/dev browser for the four primary routes.

## 9. Recommended First Implementation Scope

Do not begin with all phases at once.

Start with:

1. `ProductShell`
2. route labels and primary tabs
3. non-destructive secondary menu
4. `CompanionPage` skeleton with existing Chat preserved
5. `AgentStage` as a self-contained component

This creates the product frame while minimizing backend and workflow risk.

## 10. Open Questions To Resolve Before Coding

- Should `OpenLife` be both product and Agent name, or should the product be
  `OpenLife` and the in-product Agent have a separate name?
- Should `今日` show one user-authored goal only, or may OpenLife propose one
  goal pending confirmation?
- Should mailbox folders be product categories (`收件箱/权限/记忆/行动`) or
  lifecycle states (`待确认/稍后/已确认`)?
- Should Life Model completion be shown as a percentage, or as a qualitative
  readiness state?
- Which existing routes should remain visible to ordinary users versus
  developer/settings-only users?

These questions do not block Phase FPR-0/FPR-1 if conservative defaults are
used.

## 11. Execution Prompt For Future Coding Agent

Use this prompt when starting the implementation:

```text
You are working in /Users/fujing/Desktop/偶来福.

Implement the OpenLife Frontend Product Shell Rewrite according to
plans/frontend_product_shell_rewrite_plan.md.

Hard constraints:
- Do not migrate default Chat. Ordinary Chat must keep using legacy_stream via
  existing startStreamMessage/send path.
- Do not change src-tauri ordinary send_message/start_stream_message routing.
- Do not call W144-W146 golden path helpers, W147-W149 contract/final-gate
  helpers, or W150-W158 Skill Runtime helpers from ordinary Chat.
- Do not directly write durable LifeModel/Memory/external provider state.
- Do not paste the static prototype CSS wholesale into frontend/src.
- Preserve safe-mode, proposal-first, unsupported proposal, and diagnostics
  behavior.

Implementation order:
1. Read AGENTS.md, plans/README.md, and this plan.
2. Inspect frontend/src/App.tsx, frontend/src/pages/ChatPage.tsx,
   frontend/src/pages/ProposalReviewPage.tsx, frontend/src/pages/BuilderPage.tsx,
   frontend/src/tauri.ts, and current tests.
3. Add ProductShell and MainTabs with primary entries:
   陪伴 / 今日 / Life Model / 邮箱.
4. Keep existing developer/governance routes reachable through a secondary menu.
5. Create a first CompanionPage/AgentStage pass without altering Chat runtime.
6. Add focused tests for route labels and key page rendering.
7. Run:
   cd frontend && pnpm run typecheck
   cd frontend && pnpm test -- --run
   cd frontend && pnpm run build

Report clearly what changed, what was intentionally left unchanged, and whether
default Chat remained isolated.
```

## 12. Verification Checklist

Before marking any implementation phase complete:

- [ ] Four primary tabs are visible.
- [ ] Existing Chat send path still works and remains ordinary/default.
- [ ] Mailbox/Review actions still use Proposal-first commands.
- [ ] Builder still creates reviewable proposals before LifeModel writes.
- [ ] Settings/Runs remain reachable.
- [ ] Safe-mode warning remains visible when applicable.
- [ ] No raw prompt/memory/LifeModel/tool payload leaks into product status
      summaries.
- [ ] No fake prototype data is presented as durable truth.
- [ ] Typecheck passes.
- [ ] Frontend tests pass.
- [ ] Production build passes.
