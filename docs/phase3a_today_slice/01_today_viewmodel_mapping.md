# TodayViewModel Mapping

Status: Phase 3A-1 limited-slice mapping.

The adapter maps existing `LifeStateProjection` and daily-goal input into
`ViewModelEnvelope<TodayViewModel>`. It does not claim that a backend
`TodayViewModel` owner exists.

| Field | Source | Owner status | Adapter behavior | Limitations |
| --- | --- | --- | --- | --- |
| `dailyStateSummary` | `LifeStateProjection.safeMode`, `LifeStateProjection.taskState`, `LifeStateProjection.pending`, `LifeStateProjection.readiness`, daily-goal presence | `PARTIAL` | Builds a conservative loaded/empty/limited/blocked/safe-mode summary from projection-owned fields. | Rich daily summary wording and backend daily next-action ownership remain `PHASE_2_REQUIRED`. |
| `dailyStateSummary.providerPrivacyBoundary` | None beyond projection evidence refs | `PHASE_2_REQUIRED` | Emits unknown provider/privacy values with a warning. | Provider route, external transmission, model label, and privacy risk are not backend-owned by this slice. |
| `safeMode` | `LifeStateProjection.safeMode` | `EXISTING` | Copies active state, reason when active, source refs, and Safe Mode write/external-action blocking. | No diagnostic fallback is used. |
| `pendingReviewCount` | `LifeStateProjection.pending.totalReviewRequiredCount` | `EXISTING` | Copies the projection pending-review total. | Does not count proposals locally and does not use surface-row overrides as the authority. |
| `currentTaskPressure` | `LifeStateProjection.taskState` | `EXISTING/PARTIAL` | Copies active, waiting-permission, and blocked counts. Uses `highestRisk: "unknown"` when pressure exists because risk is not in the projection. | Stale task count is not backend-owned in the current projection and remains `0` in this limited adapter. |
| `blockers` | `LifeStateProjection.safeMode`, `LifeStateProjection.taskState` | `PARTIAL` | Emits Safe Mode, waiting-permission, and blocked-task blocker summaries only when projection fields report them. | Rich blocker classification/provenance remains `PHASE_2_REQUIRED`. No daily-goal local classifier is used. |
| `suggestions` | None | `PHASE_2_REQUIRED` | Always empty and accompanied by `today.suggestions_limited`. | Suggestions require a backend Today read model. |
| `primaryDailyGoal` | Existing daily-goal input | `PARTIAL` | Selects the first incomplete daily goal, or the first goal if all are done. Marks priority and backend classification as `unknown`. | Backend goal classification and priority are not provided by the current daily-goal input. |
| `nextRecommendedAction` | None | `PHASE_2_REQUIRED` | Always `null` with `today.next_action_limited` warning. | The adapter does not invent a next recommended action from local goal/card heuristics. |
| `workspaceLink` | Current product route convention | `PARTIAL` | Emits an ordinary product action targeting `route:companion`, labeled as the current workspace route. Disabled when the envelope is stale. | This is not a V2 Workspace route and does not create Workspace UI. |
| `reviewCenterLink` | Current product route convention | `PARTIAL` | Emits an ordinary product action targeting `route:mailbox`, labeled as the current review route. | This is not a V2 Review Center route and does not invent review actions. |
| `sourceRefs` | `LifeStateProjection.sourceRefs`, `LifeStateProjection.safeMode.sourceRefs`, daily-goal input refs | `PARTIAL` | Preserves projection source refs and daily-goal evidence refs in the envelope and nested fields. | Current daily-goal input has no stable backend entity id, so the adapter uses indexed evidence refs. |
| `actions.primary` | Adapter action lane | `PHASE_2_REQUIRED` | Contains refresh, current workspace route, and current review route actions. | No review approval/materialization action is included. |
| `actions.review` | None | `PHASE_2_REQUIRED` | Always an empty array. | Review item actions require backend Review Center ownership. |
| `actions.debugOnly` | Projection evidence refs | `PHASE_2_REQUIRED` | Contains inspect-source-refs action when source refs are available. | Debug action is not included in primary actions. |

## Explicit Unknowns

- Daily-goal classification: `unknown`
- Daily-goal priority: `unknown`
- Today suggestions: empty, limited
- Today next recommended action: `null`, limited
- Provider/privacy boundary: unknown, `PHASE_2_REQUIRED`
- Task pressure risk: `unknown` when pressure exists

These unknowns are intentional. They preserve the Phase 2 contract boundary
instead of promoting frontend-only interpretation to product truth.
