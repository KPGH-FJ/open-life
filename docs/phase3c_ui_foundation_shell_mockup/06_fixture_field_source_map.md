# Phase 3C Fixture Field Source Map

Status: static fixture provenance map.
Date: 2026-07-10.

## Boundary

Every status, metric, list row, decision context, permission field, and action
in the mockup has a `sourceRef`. A source reference declares the intended
contract owner; it does not prove that a fixed fixture value came from a live
backend.

Classifications:

- `contract-shaped/static-value`: current contract path, fixed sample value.
- `PROPOSED`: target projection not currently present on `ReviewItem`.
- `layout-only`: realistic content used only to test information density.
- `qa-only`: fixture control outside the product shell.

Canonical implemented source areas:

- `openlife-core/src/agent/product_read_model.rs`
- `openlife-core/src/agent/provider_privacy_boundary.rs`
- `openlife-core/src/agent/review_item.rs`
- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/action_executor/tool_executor.rs`

## Global Surfaces

| Surface | Source or classification |
| --- | --- |
| QA state selector | `qa-only`; outside `WorkbenchShell` |
| Top status | State `primaryStatus.sourceRef`; one primary conclusion |
| Sidebar/mobile privacy | `ProviderPrivacyBoundarySummary`; unknown/possible never maps to green |
| Inspector first summary | State `inspectorSummary`; static product explanation, omitted in the focused Workspace Inspector to avoid repeating the main event |
| Reference records | `EvidenceRef { id, label, source, sensitivity }`; raw id collapsed |
| Technical field map | Collected `sourceRef` strings; collapsed and `qa-only` |
| Support information | Debug-only actions in a collapsed Inspector section, not navigation |

## Today: Plan Available

| UI element | Fixture value | Source/classification |
| --- | --- | --- |
| Main status | 今日计划可用 | `ViewModelEnvelope.status + TodayViewModel.dailyStateSummary.readiness` |
| Current focus | 上午完成客户方案初稿 | `TodayViewModel.primaryDailyGoal` |
| Decision blocker | 深度工作偏好待决定 | `TodayViewModel.blockers[waiting_review] + pendingReviewCount` |
| Metric | 2 今日重点 | `layout-only: daily focus count fixture` |
| Metric | 11:30 下次提醒 | `layout-only: daily schedule fixture` |
| Metric | 0 已授权外部动作 | `layout-only: external action count fixture` |
| Row | 完成方案结构 | `TodayViewModel.primaryDailyGoal` |
| Row | 午后复诊 | `layout-only: daily schedule fixture` |
| Row | 不会自动发送方案 | `TodayViewModel.safeMode` |
| Row | 不会自动改变长期偏好 | `ReviewItem.status + ReviewItem.materializationStatus` |
| Action `today:open-pending-review` | 查看待决定建议 | `ViewModelEnvelope.actions.primary`; static navigation to `review-pending-decision` |
| Action `today:inspect-plan-basis` | 查看今日依据 | `ViewModelEnvelope.actions.primary`; focuses EvidenceRef |
| Action `today:raw-json` | fixture JSON | `ViewModelEnvelope.actions.debugOnly` |

The Today action is explicitly guarded from navigating directly to the
approved fixture.

## Today: Stale Or Unknown

| UI element | Fixture value | Source/classification |
| --- | --- | --- |
| Main status | 计划已陈旧 | `ViewModelEnvelope.status` |
| Current focus | 先恢复可信快照 | `TodayViewModel.dailyStateSummary + ViewModelEnvelope.status` |
| Safe Mode blocker | 风险动作保持关闭 | `TodayViewModel.safeMode + ViewModelEnvelope.warnings` |
| Metric | 2 可查看历史项 | `layout-only: stale snapshot fixture` |
| Metric | 0 可执行动作 | `ViewModelEnvelope.actions.primary[].enabled` |
| Metric | 昨日快照 | `ViewModelEnvelope.lastUpdatedAt` |
| Row | 昨日客户方案任务 | `ViewModelEnvelope.status` |
| Row | 昨日复诊提醒 | `ViewModelEnvelope.lastUpdatedAt` |
| Action `today:refresh-stale` | 刷新今日计划 | disabled `ViewModelEnvelope.actions.primary` |
| Action `today:inspect-stale-evidence` | 查看缺失依据 | `ViewModelEnvelope.actions.primary` |
| Action `today-stale:raw-json` | fixture JSON | `ViewModelEnvelope.actions.debugOnly` |

## Workspace: Waiting For Permission

| UI element | Fixture value | Source/classification |
| --- | --- | --- |
| Main status | 等待访问决定 | `TasksViewModel.items[].lifecycleStatus` |
| Current task | 整理杭州周末行程 | `WorkspaceViewModel.activeTaskRef + TasksViewModel.items[].title` |
| Blocker | 一次性访问语义缺失 | `PROPOSED ReviewDecisionContext.permissionScope` |
| Presentation | Timeline-first; Inspector on demand | `layout-only`; does not claim a current product component contract |
| Timeline event | 已建立行程结构 | `WorkspaceViewModel.timeline[]` |
| Timeline event | 读取“旅行/杭州周末” | `WorkspaceViewModel.timeline[] + ReviewItem.status` |
| Timeline event | 生成可复查的行程草稿 | `WorkspaceViewModel.timeline[] + TasksViewModel.items[].lifecycleStatus` |
| Scope summary | 4 份 PDF | `PROPOSED PermissionDecisionContext.dataScopeSummary` |
| Scope summary | 仅此文件夹 | `PROPOSED PermissionDecisionContext.targetLabel` |
| Scope summary | 外传未知 | `ProviderPrivacyBoundarySummary.externalTransmission` |
| Permission tool | 本地文件读取 | `PROPOSED` projection from `AgentProposal.after.canonical_scope` |
| Capability | `filesystem.read` | `PROPOSED` projection from canonical scope |
| Target | `~/Documents/旅行/杭州周末/` | `PROPOSED` projection from blocked action target |
| Data scope | 4 PDFs, no parent directory | `PROPOSED` bounded summary |
| Transmission | unknown | `ProviderPrivacyBoundarySummary.externalTransmission` |
| Duration/revocation | missing | `PROPOSED`; not present on current `ReviewItem` |
| Action `workspace:inspect-task` | 查看任务依据 | `ViewModelEnvelope.actions.primary` |
| Action `workspace:allow-once` | 仅允许本次 | disabled `PROPOSED` action over current approve contract |
| Action `workspace:reject-permission` | 拒绝 | `ViewModelEnvelope.actions.review` |
| Action `workspace:view-permission-scope` | 查看访问范围 | `ViewModelEnvelope.actions.review + PROPOSED permission projection` |
| Action `workspace:raw-json` | fixture JSON | `ViewModelEnvelope.actions.debugOnly` |

## Review: Pending Decision

The rich decision context is intentionally marked `PROPOSED`; current
`ReviewItem` does not project these fields.

| UI element | Fixture value | Source/classification |
| --- | --- | --- |
| Main status | 等待你的决定 | `ReviewItem.status` |
| Change summary | 上午优先深度工作 | `PROPOSED ReviewDecisionContext` |
| Current -> suggested | no preference -> 09:00-11:00 | `AgentProposal.before + after`; `PROPOSED` projection |
| Reason | three morning plans | `AgentProposal.reason + source_detail`; `PROPOSED` projection |
| Risk | low | `ReviewItem.risk` plus bounded explanation |
| Impact | future schedule suggestions | `AgentProposal.affected_path + after`; `PROPOSED` projection |
| Expiry | 7 days | `ReviewItem.expiresAt` |
| Metric | 3 observations | `PROPOSED ReviewDecisionContext.sourceCount` |
| Metric | low risk | `ReviewItem.risk` |
| Metric | 7 days remaining | `ReviewItem.expiresAt` |
| Row | three morning plans | `AgentProposal.source_detail` |
| Row | no existing calendar write | `AgentProposal.affected_path + after` |
| Action `review:reject-deep-work` | 拒绝 | `ReviewItem.allowedActions` |
| Action `review:later-deep-work` | 稍后处理 | `ReviewItem.allowedActions` |
| Action `review:edit-deep-work` | 修改 | `ReviewItem.allowedActions` |
| Action `review:approve-deep-work` | 批准变更 | `ReviewItem.allowedActions`; confirmation then static approved state |
| Action `review-pending:raw-json` | fixture JSON | `ViewModelEnvelope.actions.debugOnly` |

## Review: Approved, Not Applied

| UI element | Fixture value | Source/classification |
| --- | --- | --- |
| Main status | 已批准，尚未应用 | `ReviewItem.status + materializationStatus` |
| Current result | decision recorded | `ReviewCenterViewModel.items[0]` |
| Blocker | no application result | `ReviewItem.materializationStatus` |
| Metric | 1 recorded decision | `summary.byStatus.approved` |
| Metric | 0 applied changes | `summary.byMaterializationStatus.applied` |
| Metric | 0 failures | `summary.byMaterializationStatus.failed` |
| Row | approved decision | `ReviewItem.status` |
| Row | current preference unchanged | `ReviewItem.materializationStatus` |
| Action `review:inspect-application-status` | 查看应用依据 | `ViewModelEnvelope.actions.primary` |
| Action `review:request-apply` | 应用变更 | disabled `ReviewItem.allowedActions`; backend command gap |
| Action `review-approved:raw-json` | fixture JSON | `ViewModelEnvelope.actions.debugOnly` |

## LifeModel: Limited Compatibility

| UI element | Fixture value | Source/classification |
| --- | --- | --- |
| Main status | 当前视图受限 | `LifeModelViewModel.truthMode + contractLimitations` |
| Current understanding | meeting preparation buffer | `currentViewSummary + truthMode` |
| Blocker | pending suggestion excluded | `LifeModelViewModel.contractLimitations` |
| Metrics | 3 sources / 1 pending / 0 newly applied | `provenanceRefs`, `pendingUpdateCounts`, `materializedChanges` |
| Row | meeting buffer | `LifeModelViewModel.currentViewSummary` |
| Row | source record | `LifeModelViewModel.provenanceRefs` |
| Action `lifemodel:inspect-current-view` | 查看来源 | `ViewModelEnvelope.actions.primary` |
| Action `lifemodel:open-pending-review` | 查看待决定建议 | `ViewModelEnvelope.actions.primary` |
| Action `lifemodel:raw-json` | fixture JSON | `ViewModelEnvelope.actions.debugOnly` |

## Settings: Provider And Privacy Unknown

| UI element | Fixture value | Source/classification |
| --- | --- | --- |
| Main status | 传输边界待确认 | `SettingsViewModel.providerPrivacyBoundary` |
| Current setting | choose model for travel data | `providerPrivacyBoundary + setupReadiness` |
| Blocker | automatic cloud processing off | `ProviderPrivacyBoundarySummary.blockedReason` |
| Metric | 1 configured local model | `layout-only: configured local model count fixture` |
| Metric | cloud provider unselected | `ProviderPrivacyBoundarySummary.providerLabel` |
| Metric | automatic route off | `SettingsViewModel.setupReadiness` |
| Row | local model configured | `ProviderPrivacyBoundarySummary.providerLabel` |
| Row | cloud provider unselected | `externalTransmission` |
| Action `settings:inspect-privacy` | 查看传输说明 | `ViewModelEnvelope.actions.primary` |
| Action `settings:configure-provider` | 选择云端供应商 | disabled `ViewModelEnvelope.actions.primary` |
| Action `settings:raw-json` | fixture JSON | `ViewModelEnvelope.actions.debugOnly` |

## Automated Guard

Run:

```sh
node docs/phase3c_ui_foundation_shell_mockup/static_mockup/validate-fixtures.mjs
```

The validator checks all seven states, sourceRef coverage, privacy fail-closed
rules, action contracts, confirmation behavior, navigation hierarchy, the
pending-review transition, required decision fields, permission-scope fields,
the one-time-permission contract blocker, and the focused Workspace timeline
shape. It rejects restored Workspace metrics or duplicate progress sections.
