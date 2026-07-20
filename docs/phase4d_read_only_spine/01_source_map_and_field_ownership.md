# Phase 4D Read-Only Spine Source Map

Status: `IMPLEMENTED`
Date: 2026-07-20

## Runtime Path

```text
Phase 4D dev-only Tauri window
  -> ReadOnlySpineJourney
  -> tauriReadOnlySpineDataSource
      -> get_life_state_projection
      -> get_daily_goals
      -> get_provider_privacy_boundary_summary
      -> get_tasks_view_model
  -> strict Today adapter / backend Tasks envelope
  -> TodayReadOnlyView or TasksReadOnlyView
  -> OpenLifeWorkbenchShell + structured Inspector
```

Browser layout fixtures implement the same typed data-source interface, but
remain in `frontend/src/dev/phase4d/**` behind an external QA toolbar. They are
not fallback product data and never count as Tauri evidence.

## Today Ownership

| UI fact | Exact source | Frontend rule |
| --- | --- | --- |
| current daily goal title/state/time input | `get_daily_goals` compatibility DTO, formatted by `openlife.today-adapter.v1` | no create, reorder, complete, or local classification |
| readiness and Safe Mode | `LifeStateProjection` | missing projection produces error; no daily-goal fallback |
| active/waiting/blocked task pressure | `LifeStateProjection.taskState` | counts remain projection-owned; no task reconstruction |
| pending review count | `LifeStateProjection.pending.totalReviewRequiredCount` | count never implies approval or application |
| provider route/transmission/risk | `ProviderPrivacyBoundarySummary` | unknown/possible/stale never renders green local certainty |
| Today envelope status | strict adapter plus source-load result | source read failure degrades to stale/error, never ready by omission |
| Inspector evidence | projection refs, daily-goal refs, and boundary evidence refs | metadata only; no invented evidence body |
| last refresh time | envelope `lastUpdatedAt` | Inspector technical disclosure only |

The Today renderer consumes `TodayViewModelEnvelope`; it does not import raw
proposal, diagnostic, task-store, provider-config, or AgentRun APIs.

## Tasks Ownership

| UI fact | Exact source | Frontend rule |
| --- | --- | --- |
| list and totals | `TasksViewModel.items/summary` | no `listAgentRuns` join |
| title/strategy/lifecycle | `TaskViewModelItem` | unknown remains unknown |
| blockers and review refs | `TaskViewModelItem.pendingBlockers/pendingReviewItemRefs` | displayed as attention, not completion |
| latest result | `latestResultPreview` plus terminal evidence fields | `completed` is green only with final-delivery evidence |
| search/filter input | local ephemeral UI state | changes visibility only, never backend truth |
| selected task | local ephemeral UI state | only changes Inspector context |
| task controls | `allowedControls` | intentionally not dispatched in this read-only slice |
| evidence | item/model/envelope `EvidenceRef` values | id/label/source/sensitivity preserved |
| contract limits | backend `contractLimitations` | Inspector technical disclosure, not hidden success |

The old `RunsPage` still joins `listAgentRuns` and dispatches task controls. It
remains the production owner until later journey slices and the Phase 4E atomic
switch. The Phase 4D Tasks renderer does not copy that join.

Envelope status is authoritative over a payload. An `error` or `loading`
envelope never exposes payload totals, rows, search, or filters, even if the
backend includes an empty or previously loaded model. A failed read is unknown,
not a confirmed zero-item list.

## Boundary Status Mapping

| Backend state | Shell result |
| --- | --- |
| envelope loading | neutral `正在读取传输边界` |
| envelope error or no data | amber/error `传输边界未知` |
| envelope stale | amber `传输边界已陈旧` |
| route `local`, transmission `not_sent`, fresh evidence present | verified green `本地路由，未外传` |
| transmission `possible` | amber `可能发生外部传输` |
| transmission `unknown` or route `unknown` | amber `是否外传未知` |
| transmission `sent` | neutral/amber `已发生外部传输` |

The mapping never converts `preferLocal`, `localOnlyRequired`, a provider name,
or missing evidence into a local/private success claim.

## Actions In This Slice

| Visible control | Contract/source | Result |
| --- | --- | --- |
| refresh Today | ProductAction `today.refresh` | invoke Today source load; announce success/failure |
| open Workspace | Today adapter ProductAction | show explicit unported Workspace state; no redirect |
| open Review Center | Today adapter ProductAction | show explicit unported Review state; no decision change |
| navigate Tasks | Shell product navigation | load backend `TasksViewModel`; move focus to heading |
| search/filter Tasks | local ephemeral control | update visible rows and announce count |
| select task/evidence | local inspect interaction | update/open Inspector only |
| open Settings/category | Shell utility navigation | show explicit unavailable state and restore focus on Back |

There are no ReviewAction, task-control dispatch, DebugAction dispatch, or
durable/external effects in this slice.

## Remaining Old Callers

- shell authority: `frontend/src/App.tsx` -> `ProductShell`;
- Today route: `frontend/src/App.tsx` -> `TodayPage`;
- Tasks route: `frontend/src/App.tsx` -> `RunsPage`;
- task/workspace joins and controls: `ChatPage` and `RunsPage`;
- settings boundary UI: `SettingsPage` and its tabs.

No row is `delete_ready` after this slice because the new journey has no
production caller.
