# Sprint 6 Final Acceptance: Daily UX, IA, Accessibility

Date: 2026-06-30

Status: prepared for final Sprint 6 validation after slices 6A-6D.

## Goal

Decide whether Sprint 6 can be closed as a daily UX / IA / accessibility
improvement, not just a set of isolated component changes.

The acceptance pass must verify that ordinary users can understand Today,
navigate primary surfaces, use the chat composer, and read Review/LifeModel
content without relying on debug traces.

## Scope

Validate the implemented Sprint 6 slices:

- 6A Today typed cards and count taxonomy.
- 6B ProductShell navigation IA and Review naming.
- 6C Chat composer accessibility, IME, and narrow layout.
- 6D Review/LifeModel copy and long-text scanability.

No new product behavior should be added during this pass except small test or
copy fixes needed to make the acceptance contract true.

## Non-Goals

- No backend runtime/provider route changes.
- No LifeModel schema migration.
- No broad visual redesign.
- No deletion of Runs, Settings Advanced, trace, or technical evidence surfaces.
- No claim that Sprint 6 fixes route truth, LifeModel write closure, provider
  governance, or agent task recovery beyond the UX surfaces touched here.

## Required Evidence

Record the final evidence in the next review response or a follow-up result
document if code changes are needed.

Minimum command gates:

- `cd frontend && corepack pnpm test -- TodayPage.test.tsx`
- `cd frontend && corepack pnpm test -- ProductShell.test.tsx App.test.tsx`
- `cd frontend && corepack pnpm test -- ChatInputArea.test.tsx ChatPage.test.tsx`
- `cd frontend && corepack pnpm test -- MailboxPage.test.tsx LifeModelPage.test.tsx proposalDisplay.test.ts`
- `cd frontend && corepack pnpm typecheck`
- Changed-file `prettier --check` for any files touched in the acceptance pass.
- `git diff --check`

Optional broader confidence gate when runtime budget allows:

- `cd frontend && corepack pnpm test -- TodayPage.test.tsx ProductShell.test.tsx App.test.tsx ChatInputArea.test.tsx ChatPage.test.tsx MailboxPage.test.tsx LifeModelPage.test.tsx proposalDisplay.test.ts SettingsPage.test.tsx ProviderTab.test.tsx ToolsPermissionsTab.test.tsx`

## Replay Matrix

| Case | Evidence Required | Pass Criteria |
|---|---|---|
| Today qapressure state | Today focused test or inspected typed card | suspicious state metric is not rendered as goal/task |
| Review pending count | Today/Review/LifeModel/Settings tests or source trace | count label means pending proposals, not unread mail |
| Primary navigation | ProductShell test and route aliases | Today, Companion, Review, Life Model, Runs, Settings are primary; debug surfaces are Advanced |
| Composer AX | Testing Library role/name assertions | `消息输入`, `发送消息`, busy send name, cancel/stop name are queryable |
| Composer IME | composition event test | Enter during composition does not submit; after composition it can submit |
| Composer 560/720 | component layout test, browser replay if available | input and send/cancel remain visible, long text scrolls |
| Review long text | Mailbox focused test | source details are collapsed by default, expandable, and actions remain available |
| LifeModel evidence copy | LifeModel focused test | ordinary evidence summary avoids raw internal source text; technical details retain trace evidence |

## Browser / Tauri Constraint

The Vite browser dev server may not enter the same Chat/LifeModel state as the
Tauri desktop app because native commands are unavailable. If browser replay is
blocked by missing Tauri IPC, mark that replay as blocked with the observed
reason and rely on focused component tests for AX/IME/layout proof. Do not
pretend the browser shell proves a desktop-only flow.

## Anti-Hallucination Checks

- Do not claim a route/provider/runtime fix from Sprint 6 UI copy.
- Do not claim AX coverage from screenshots; use role/name tests.
- Do not claim IME safety from ASCII Enter tests.
- Do not claim long-text scanability unless expand/collapse and action
  availability are both tested.
- Do not claim raw evidence was removed; it should remain in technical details.
- Do not treat a test command as evidence unless it reports non-zero matched
  tests.

## Closure Decision

Sprint 6 can be marked complete only when:

- All required command gates pass.
- The replay matrix is pass or explicitly blocked with a concrete environment
  reason.
- No raw internal governance/debug copy appears in ordinary primary surfaces
  touched by Sprint 6.
- Technical evidence remains reachable from explicit details surfaces.
- The final review states remaining residual risk, especially any blocked
  desktop-only replay.
