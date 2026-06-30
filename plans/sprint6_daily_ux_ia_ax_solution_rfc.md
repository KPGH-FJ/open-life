# Sprint 6 Solution RFC: Daily UX, IA, Accessibility

Date: 2026-06-29

Status: prepared for targeted UX implementation after core truth chains are stable.

## Scope

Raw issues: `OL-004`, `OL-009`, `OL-011`, `V4-004`, `V4-009`, `V4-010`, `V4-012`, `V4-016`, `V5-009`, `V5-012`, `V5-016`, `V5-017`, `V5-019`, `V5-023`, `V5-024`.

Primary source entrypoints:

- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/utils/dailyGoalDisplayGuard.ts`
- `frontend/src/components/ProductShell.tsx`
- `frontend/src/pages/chat/ChatInputArea.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/pages/settings/tabs/AdvancedTab.tsx`
- `frontend/src/pages/settings/tabs/ReviewMemoryTab.tsx`

## Product Goal

OpenLife should feel like a daily personal AI OS, not an internal control panel. Ordinary users should know what to do next, while advanced/debug surfaces remain available but separated.

## Non-Goals

- Do not do a broad visual redesign before core truth chains are fixed.
- Do not remove developer/debug pages; move or label them appropriately.
- Do not change underlying LifeModel semantics in this sprint.

## Today Taxonomy

Create `TodayCardView`.

| Card type | Meaning | May become task? |
|---|---|---|
| `goal` | User-confirmed objective. | yes |
| `task` | Specific actionable item. | yes |
| `suggestion` | OpenLife recommendation based on accepted/pending evidence. | user confirms |
| `state_signal` | Energy, pressure, mood, context signal. | no |
| `pending_proposal` | Needs Review action. | no |
| `blocker` | Something prevents useful action. | no |

Rules:

- State metrics such as `qapressure = 8 points` are never goals.
- Pending proposal count must match Review count taxonomy.
- Cards show source/confidence when not obvious.

## Count Taxonomy

| Count | Meaning | Owner |
|---|---|---|
| pending proposals | Review items awaiting user decision | Review |
| model gaps | LifeModel missing/low-confidence sections | LifeModel |
| active tasks | non-terminal task sessions | Runs |
| blockers | blocked tasks needing recovery | Runs |
| unread messages | product notifications only | future notification center |

## Navigation Contract

Primary:

- Today
- Companion
- Review
- Life Model
- Runs
- Settings

Advanced:

- MCP / Tools
- A2A
- Metrics
- Calibration
- Stage/debug/eval pages

Naming:

- Use `Review` or `确认中心`; avoid `Mailbox` for user-facing navigation.
- Keep technical terms in expandable technical detail, not primary copy.

## Composer / AX Contract

`ChatInputArea` must guarantee:

- textarea role/name: `消息输入`.
- send button accessible name: `发送消息` or `正在发送消息`.
- cancel button accessible name when available.
- Enter/send behavior does not corrupt Chinese IME composition.
- 560/720 width layout keeps input and send/cancel visible.
- Long text scrolls inside textarea without covering controls.

## Language / Copy Contract

- User-facing Chinese session should not suddenly show English governance/debug copy.
- Internal terms such as governed draft, routeType, fallback, transcript appear only in technical detail.
- Real-life plans must use natural Chinese and explicit assumptions.

## Tests

Every command gate must record a non-zero matched/passed test count. Existing broad page tests are useful but not enough unless they assert the exact Today taxonomy, count, AX, IME, and responsive contracts below.

Frontend:

- Today does not render suspicious state metric as goal/task.
- Review count equals pending proposal count across Today/LifeModel/Settings/Review.
- ProductShell primary/advanced navigation grouping.
- ChatInputArea AX labels and IME-safe composition.
- 560/720 responsive snapshots for Chat, Review, LifeModel, Settings.

Candidate command-level frontend gates after adding/updating focused tests:

- `cd frontend && corepack pnpm test -- TodayPage.test.tsx`
- `cd frontend && corepack pnpm test -- ChatPage.test.tsx`
- add `frontend/src/components/ProductShell.test.tsx` or equivalent exact routing-shell test before claiming navigation grouping coverage.
- add `frontend/src/pages/chat/ChatInputArea.test.tsx` or equivalent exact composer test before claiming AX/IME coverage.

Replay:

- v4/v5 Today `qapressure`.
- v5 Chinese task output.
- v5 560/720 layout.
- v5 AX tree for composer send/cancel.

## Development Slices

Development must stay sequential. Do not combine Today semantics, shell IA, and
composer accessibility in one diff; each slice needs its own focused tests and
replay notes.

### Slice 6A: Today Typed Cards And Counts

Goal: stop state metrics and pending-review counts from being rendered as goals
or arbitrary next actions.

Tasks:

- Add or harden a `TodayCardView` / `dailyGoalDisplayGuard` boundary that
  classifies `goal`, `task`, `suggestion`, `state_signal`, `pending_proposal`,
  and `blocker`.
- Ensure `qapressure`, energy/mood/pressure signals, and confidence metrics are
  rendered as signals, never as goals/tasks.
- Use one Review pending-count source for Today, Life Model, Settings, and
  Review entry copy where this slice touches counts.
- Show source/confidence only when backed by existing typed data; do not invent
  provenance from UI copy or assistant prose.

Non-goals:

- No broad Today redesign.
- No LifeModel schema rewrite.
- No new task-creation workflow.

Required tests:

- `cd frontend && corepack pnpm test -- TodayPage.test.tsx`
- Add/extend a focused guard test if `TodayPage.test.tsx` cannot directly cover
  suspicious metric classification.

Replay:

- v4/v5 `qapressure = 8 points` appears as `state_signal`, not goal/task.
- Pending Review count matches the Review source used by the touched UI.

### Slice 6B: Navigation IA And Review Naming

Goal: make governance surfaces discoverable without exposing developer/control
plane pages as primary user navigation.

Tasks:

- Rename user-facing `Mailbox` navigation copy to `Review` or `确认中心` while
  preserving route compatibility if existing routes are named `/mailbox`.
- Keep Today, Companion, Review, Life Model, Runs, and Settings as primary
  user-facing navigation.
- Move MCP/Tools, A2A, Metrics, Calibration, stage/debug/eval pages into
  Advanced/More with copy that explains they are technical surfaces.
- Use count taxonomy labels consistently: pending proposals, model gaps, active
  tasks, blockers. Do not reuse "unread" for proposal counts.

Non-goals:

- No route deletion or migration that breaks existing bookmarks.
- No large visual redesign of `ProductShell`.
- No developer page removal.

Required tests:

- Add `frontend/src/components/ProductShell.test.tsx` or an equivalent focused
  routing-shell test before claiming navigation coverage.
- Run the new ProductShell test plus any existing page tests changed by route
  copy.

Replay:

- v4 navigation screenshot scenario: Review, Runs, Settings, Life Model are
  findable; debug pages remain available but secondary.

### Slice 6C: Composer Accessibility, IME, And Narrow Layout

Goal: make the main prompt composer operable by keyboard, accessibility tools,
Chinese IME, and 560/720 width layouts.

Tasks:

- `ChatInputArea` exposes textarea accessible name `消息输入`.
- Send button exposes `发送消息` when idle and a distinct busy name while sending.
- Cancel/stop button has a stable accessible name when visible.
- Composition events must prevent Enter from submitting while Chinese IME is
  composing.
- Long text scrolls inside the textarea; controls remain visible at 560 and 720
  widths.

Non-goals:

- No Main Chat runtime changes.
- No provider/model route changes.
- No rewrite of the entire Chat page.

Required tests:

- Add `frontend/src/pages/chat/ChatInputArea.test.tsx` or equivalent exact
  composer test before claiming AX/IME coverage.
- `cd frontend && corepack pnpm test -- ChatPage.test.tsx ChatInputArea.test.tsx`
- `cd frontend && corepack pnpm typecheck`

Replay:

- v5 AX scenario: input/send/cancel names are present.
- v5 Chinese prompt scenario: composed Chinese text is not corrupted or
  submitted mid-composition.
- v5 560/720 layout scenario: composer controls remain visible.

### Slice 6D: Copy And Long-Text Scanability

Goal: reduce internal governance/debug copy in ordinary flows and make long
Builder/Review text readable.

Tasks:

- Move `governed draft`, route ids, transcript ids, raw fallback metadata, and
  similar technical terms into expandable technical details where possible.
- Keep natural Chinese in ordinary planning/review copy.
- Improve long candidate/proposal text scanability with constrained height,
  expand/collapse, and source/detail separation.

Non-goals:

- No backend proposal schema migration.
- No localization framework rewrite.

Required tests:

- Focused tests for the exact changed copy/long-text component.
- Existing Review/Builder page tests touched by the diff.

Replay:

- v5 Chinese planning/review screenshots no longer show unexplained English
  governance copy in the primary surface.
- Builder long text remains readable without losing action controls.

## Anti-Hallucination Checks

- Today source/confidence must come from typed data or existing read models, not
  assistant output text.
- Pending counts must be traced to one query/helper; UI-local recomputation is
  allowed only if the exact source collection is named in code/tests.
- Navigation readiness cannot imply provider/tool readiness.
- AX coverage cannot be claimed from screenshots alone; use Testing Library role
  queries and, when doing manual replay, Computer Use accessibility evidence.
- IME safety must be tested with composition events, not only ASCII Enter.
- Responsive readiness needs explicit 560/720 viewport checks or component tests
  that exercise the narrow layout contract.

## Industry Practice References

- ChatGPT Projects/Memory: user-owned memory/project context should be visible,
  controllable, and named in user language; use this for Review/LifeModel naming.
- Claude Artifacts: substantial generated or reviewed content should be
  separated from chat/debug traces and remain readable/editable; use this for
  Builder/Review long text.
- Notion AI: workspace AI should preserve source feel and avoid unsupported
  claims; use this for Today provenance and count taxonomy.
- Codex/Cursor task surfaces: task status and technical logs are available, but
  ordinary users see status, blocker, and next control first; use this for
  primary vs Advanced IA.

Exit only when core daily flows work without reading debug traces.
