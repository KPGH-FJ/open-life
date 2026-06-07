# Frontend Product Shell Rewrite Goal Spec

> Status: proposed CLI Goal-mode spec; not yet executed
> Baseline: W150-W158 complete; default Chat remains `legacy_stream`
> Product plan: `plans/frontend_product_shell_rewrite_plan.md`
> Prototype reference: `/Users/fujing/Desktop/openlife-companion-stage-prototype`
> Goal range: W159-W166

## 1. Objective

Rewrite the OpenLife frontend product shell and primary product surfaces around
the current prototype direction:

```text
陪伴 -> 今日 -> Life Model -> 邮箱
```

The goal is productization, not backend/runtime migration.

The final output of this Goal should be a React/Tauri frontend that makes
OpenLife feel like a local-first personal Agent product while preserving all
existing governance, safe-mode, proposal-first, and default Chat isolation
boundaries.

## 2. Non-Negotiable Constraints

Do not:

- migrate default Chat away from `legacy_stream`;
- modify Rust ordinary `send_message` / `start_stream_message` routing;
- call governed preview, migration gate, cutover, golden path, contract gate, or
  Skill Runtime helpers from ordinary Chat;
- hide Safe Mode, beta-readiness, provider diagnostics, unsupported proposal,
  or proposal-first warnings;
- directly write durable LifeModel/Memory/file/calendar/email/provider/plugin
  state from new UI affordances;
- delete existing pages until the replacement surface is tested and the old
  route remains reachable as a secondary route;
- paste prototype CSS wholesale into `frontend/src`;
- present prototype mock metrics as real user state.

If any implementation requires violating one of these constraints, stop and
split out a separate reviewed Goal.

## 3. Existing Frontend Facts To Preserve

Current code already contains important product and governance behavior:

- `frontend/src/App.tsx`
  - safe-mode banner;
  - beta-readiness banner;
  - onboarding wizard;
  - lazy route loading;
  - existing developer/governance routes.

- `frontend/src/pages/ChatPage.tsx`
  - ordinary Chat path;
  - session list/history;
  - quick command support;
  - companion cockpit / Life Model pulse;
  - governed preview is explicit and separate from ordinary Send;
  - runtime error repair messages.

- `frontend/src/pages/chat/useChatStreaming.ts`
  - `startStreamMessage` remains the ordinary stream entry;
  - stream event listeners update messages, run id, reasoning trace, tool calls,
    interrupted state, and analytics.

- `frontend/src/pages/ProposalReviewPage.tsx`
  - proposal accept/reject/postpone/edit;
  - unsupported proposal protection;
  - safe paths / safe-mode checks;
  - evidence summaries without raw sensitive payloads.

- `frontend/src/pages/BuilderPage.tsx`
  - Builder sessions;
  - proposal creation before LifeModel writes;
  - safe-mode blocked messages.

- Existing tests already protect key behavior:
  - `frontend/src/App.test.tsx`
  - `frontend/src/pages/ChatPage.test.tsx`
  - `frontend/src/pages/ProposalReviewPage.test.tsx`
  - `frontend/src/pages/BuilderPage.test.tsx`

These should be adapted only when UI labels/route structure intentionally
change, not removed to make the rewrite easier.

## 4. Target Route Map

Primary product routes:

| Product Tab | Route | Source/Replacement |
| --- | --- | --- |
| `陪伴` | `/companion` | new product wrapper around Chat |
| `今日` | `/today` | new focused daily goal surface |
| `Life Model` | `/life-model` | new wrapper around Builder/LifeModel/Memory evidence |
| `邮箱` | `/mailbox` | new mail-like Proposal Review surface |

Backward-compatible routes must remain reachable:

| Existing Route | Requirement |
| --- | --- |
| `/` | may redirect/render `/companion` only after shell tests pass |
| `/chat`, `/agent` | alias or secondary route to companion/Chat behavior |
| `/review` | alias or secondary route to mailbox/Proposal Review behavior |
| `/builder`, `/life`, `/map`, `/memory` | remain reachable during Life Model migration |
| `/runs`, `/settings`, `/mcp`, `/a2a`, `/metrics`, `/versions`, `/calibration` | remain reachable through secondary tools/developer menu |

## 5. Goal Slices

### W159: Product Shell Preflight Contract

Purpose: make the rewrite executable without ambiguity.

Tasks:

- Add route/product IA tests before changing major UI.
- Decide exact route map and aliases.
- Add a shared list of forbidden ordinary Chat commands for tests if not already
  factored.
- Locate production-safe asset path for `AgentStage` imagery.
- Document old route retention.

Acceptance:

- No visual rewrite yet.
- No backend/Rust changes.
- Existing tests pass.
- New tests fail only if route labels or forbidden-call assertions are broken.

### W160: ProductShell And MainTabs

Purpose: introduce the new product frame safely.

Tasks:

- Add `ProductShell`.
- Add `MainTabs` with `陪伴 / 今日 / Life Model / 邮箱`.
- Preserve safe-mode and beta-readiness banners from `App.tsx`.
- Add `SecondaryToolsMenu` for Runs/Settings/MCP/A2A/Metrics/Versions/
  Calibration.
- Keep existing pages reachable.

Acceptance:

- Four primary tabs render.
- Settings and Runs remain reachable.
- App onboarding still works.
- Safe-mode and beta-readiness tests still pass or are updated to equivalent
  product-shell selectors.

### W161: AgentStage Component

Purpose: move the prototype's state stage into React as a self-contained visual
component.

Tasks:

- Add `AgentStage`.
- Support states:
  - `idle`
  - `listening`
  - `sorting`
  - `memory`
  - `planning`
  - `review`
  - `privacy`
  - `error`
- Use scoped CSS/Tailwind classes; do not paste the prototype stylesheet.
- Support `prefers-reduced-motion`.
- Provide accessible status text outside purely decorative assets.

Acceptance:

- Component can render every state in tests.
- No raw prompt/memory/LifeModel/tool payload is rendered.
- Reduced-motion styling is present.

### W162: Companion Surface With Existing Chat Runtime

Purpose: make Chat feel like the daily companion surface without changing the
ordinary Chat path.

Tasks:

- Add `CompanionPage`.
- Reuse or extract the existing Chat session/thread/composer behavior.
- Wire `AgentStage` state from existing UI events:
  - input focused/submitted -> `listening`
  - sending/streaming -> `sorting`
  - stream error -> `error`
  - pending proposal hint -> `review`
  - safe-mode/privacy warning -> `privacy`
- Keep `startStreamMessage` as ordinary Send.
- Keep governed preview explicit and visually secondary/developer-facing.

Acceptance:

- `ChatPage.test.tsx` ordinary send isolation remains true.
- Ordinary Send does not call:
  - `run_multi_strategy_agent_preview`
  - `check_runtime_migration_gate`
  - controlled/default adapter migration commands
  - Skill Runtime commands
- Slash commands remain usable when ordinary Chat is not ready.
- Runtime error repair copy remains visible.

### W163: Mailbox Proposal Surface

Purpose: turn Proposal Review into a human-like mailbox without weakening
governance.

Tasks:

- Add `MailboxPage`.
- Render proposals as mail rows.
- Render selected proposal as mail reader content:
  - sender;
  - subject;
  - concise reason;
  - impact;
  - evidence/boundary details;
  - quick replies.
- Map quick replies to existing proposal commands.
- Preserve edit/reject/postpone/accept behavior.
- Preserve unsupported proposal blocking and safe-mode checks.

Acceptance:

- Existing proposal tests pass or are updated to mailbox labels.
- Low-risk proposal can still be accepted.
- Unsupported proposal type cannot be accepted.
- Evidence summaries remain metadata-safe.

### W164: Life Model Product Surface

Purpose: consolidate Builder, Life Model overview, memory/evidence, and pending
confirmations.

Tasks:

- Add `LifeModelPage`.
- Tabs: `构建 / 概览 / 依据`.
- Reuse existing Builder commands for build flow.
- Link pending Builder/Model proposals to Mailbox.
- Keep exact LifeModel writes proposal-first.
- Prefer qualitative readiness labels over fake precision if completion data is
  unavailable.

Acceptance:

- Builder flow still creates reviewable proposals.
- Existing Builder tests pass or are updated to equivalent user-facing labels.
- Memory/evidence views do not directly mutate durable truth.

### W165: Today Surface

Purpose: make `今日` a simple daily focus surface.

Tasks:

- Add `TodayPage`.
- Show one current daily goal or empty-state prompt.
- Show one next action.
- Use existing daily goal/state commands only.
- Avoid project-dashboard density and fake stats.

Acceptance:

- User can see today's focus in one screen.
- Existing daily goal actions still work if exposed.
- No fake prototype progress values are shown as real state.

### W166: Visual QA, Test Hardening, Docs Sync

Purpose: complete the product shell rewrite with verification.

Tasks:

- Desktop and mobile visual QA.
- Keyboard/focus QA.
- Empty/loading/error state QA.
- Update docs with actual route map and preserved old routes.
- Run full frontend verification.

Acceptance:

- `cd frontend && pnpm run typecheck`
- `cd frontend && pnpm test -- --run`
- `cd frontend && pnpm run build`
- Manual Browser/Tauri check for:
  - `/companion`
  - `/today`
  - `/life-model`
  - `/mailbox`
  - `/settings`
  - `/runs`

## 6. Forbidden Command Audit

The Goal must preserve ordinary Chat isolation. At minimum, ordinary Send tests
must assert no calls to:

- `run_multi_strategy_agent_preview`
- `check_runtime_migration_gate`
- `check_controlled_chat_pilot_eligibility`
- `record_controlled_pilot_promotion_evidence`
- `check_controlled_pilot_promotion_readiness`
- `draft_controlled_chat_migration_plan`
- `record_controlled_chat_migration_review_decision`
- `check_controlled_chat_migration_implementation_gate`
- `run_controlled_chat_migration_shadow_run`
- `check_controlled_chat_cutover_readiness`
- `run_controlled_chat_cutover_candidate`
- `get_default_chat_runtime_boundary_status`
- `draft_default_chat_adapter_activation_plan`
- `check_default_chat_adapter_activation_implementation_gate`
- `get_default_chat_adapter_routing_status`
- `check_default_chat_adapter_contract_harness`
- `run_default_chat_adapter_dry_run`
- `check_default_chat_adapter_implementation_readiness`
- `run_default_chat_adapter_controlled_preview`
- `draft_default_chat_adapter_cutover_implementation_plan`
- `get_react_beta_execution_status`
- `get_runtime_strategy_registry_status`
- `get_skill_runtime_status`
- `run_skill`

If a new command appears in the project that belongs to default Chat migration,
backend golden paths, pre-UI contracts, or Skill Runtime status/run surfaces,
add it to this audit before proceeding.

## 7. Required Verification Commands

Run after each Goal slice unless the slice is docs-only:

```bash
cd frontend && pnpm run typecheck
cd frontend && pnpm test -- --run
cd frontend && pnpm run build
```

Run after W162 and at final W166:

```bash
rg -n "run_multi_strategy_agent_preview|check_runtime_migration_gate|run_skill|get_skill_runtime_status|get_react_beta_execution_status|get_runtime_strategy_registry_status" frontend/src/pages frontend/src/components frontend/src/App.tsx
rg -n "send_message|start_stream_message" src-tauri/src openlife-core/src
```

The first command is not a blanket failure if explicit preview/settings pages
still contain those commands. It is a review trigger: ordinary Companion/Chat
Send paths must not call them.

## 8. CLI Goal Prompt

Use this prompt to start the Goal:

```text
You are working in /Users/fujing/Desktop/偶来福.

Execute plans/frontend_product_shell_rewrite_goal_spec.md in CLI Goal mode.
Start with W159 only unless the user explicitly authorizes continuing to the
next slice after verification.

Before editing:
- Read AGENTS.md.
- Read plans/README.md.
- Read plans/frontend_product_shell_rewrite_plan.md.
- Read plans/frontend_product_shell_rewrite_goal_spec.md.
- Inspect frontend/src/App.tsx, frontend/src/pages/ChatPage.tsx,
  frontend/src/pages/chat/useChatStreaming.ts,
  frontend/src/pages/ProposalReviewPage.tsx,
  frontend/src/pages/BuilderPage.tsx, frontend/src/tauri.ts, and current tests.

Hard constraints:
- Do not migrate default Chat. Ordinary Send remains the existing
  startStreamMessage/legacy_stream path.
- Do not modify src-tauri ordinary send_message/start_stream_message routing.
- Do not call governed preview, migration gate, backend golden path,
  pre-UI contract/final-gate, or Skill Runtime commands from ordinary Chat.
- Do not directly write durable LifeModel/Memory/external provider state.
- Preserve Safe Mode, beta-readiness, unsupported proposal, proposal-first, and
  diagnostic behavior.
- Keep old routes reachable while adding the product shell.

Implementation discipline:
- Implement one W-slice at a time.
- Prefer small React components and tests over large rewrites.
- Do not paste prototype CSS wholesale.
- Add or update tests before broad UI replacements.
- After each slice, run frontend typecheck, tests, and build.

Report:
- completed W-slice;
- files changed;
- tests run;
- default Chat isolation status;
- remaining blockers.
```

## 9. Stop Conditions

Stop the Goal and report blockers if:

- ordinary Send calls any forbidden migration/preview/Skill Runtime command;
- existing proposal accept/reject/postpone/edit behavior is broken;
- Builder can write LifeModel without reviewable proposals;
- safe-mode or beta-readiness banners disappear without equivalent replacement;
- Settings or Runs becomes unreachable;
- frontend typecheck, tests, or build fail and cannot be fixed within the slice;
- a required backend command does not exist and the UI would need fake product
  truth.

