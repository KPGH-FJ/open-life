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

1. Today typed card view and count taxonomy.
2. Navigation rename/grouping.
3. Composer AX/IME/responsive hardening.
4. Copy/language guard for internal governance text.
5. Builder/Review long-text scanability.

Exit only when core daily flows work without reading debug traces.
