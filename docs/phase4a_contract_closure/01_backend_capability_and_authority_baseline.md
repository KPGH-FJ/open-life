# Backend Capability And Authority Baseline

Status: `SOURCE_VERIFIED_AT_1267EE4`
Date: 2026-07-19

This is the Phase 4A rerun of the Phase 3F backend map. It is named a capability
and authority baseline because Phase 4A may add read-only projections while
preserving business authority. It is not a claim that backend code is frozen.

## 1. Scope Boundary

- Included: protected `main` at `1267ee4`, containing the merged roadshow and
  frontend-readiness convergence history.
- Excluded: the 13 commits previously identified as unique to paused Backend
  Remediation v4.
- Authority rule: current source and executable tests override older planning
  descriptions.

## 2. Capability Map

| Product concern | Current authority | Phase 4A result | Authority change |
| --- | --- | --- | --- |
| Review decision and materialization | `AgentProposal`, `ReviewWorkflow`, `ProposalStore`, `ReviewItem` | `ReviewItem.decisionContext` now projects readable before/after, reason, source, impact, target labels, expiry, evidence, and permission context | No decision or write authority change |
| Exact tool permission | `ActionBoundToolPermissionScope`, network-policy consent proposals, canonical ToolPermission store | Both `action_bound` and `network_policy` are projected; incomplete scope disables approve | No permission broadening or consumption change |
| Review actions | `ReviewItem.allowedActions` | Required ids/labels/targets, kind/effect, confirmation, disabled reason, and no-completion claim are executable invariants | No command dispatch change |
| Tasks | `TasksViewModel`, Main Chat task controls, AgentRun store | Existing lifecycle and control authority retained | None |
| Workspace | existing `WorkspaceViewModel` owner in `tasks_view_model.rs` and Tauri `read_models/tasks.rs` | Composes full active task, related ReviewItems, metadata-only task activity, provider/privacy boundary, sources, and limitations | Read-only projection expanded in place |
| Today | `LifeStateProjection`, `get_daily_goals`, `ProviderPrivacyBoundarySummary` | Strict adapter owner/version/input list frozen; missing projection or boundary remains error/unknown | No new Today truth owner |
| Settings edit/test/save | config commands, connection-test command, `ProviderPrivacyBoundarySummary` | Frontend orchestration state machine freezes test != save and save != refreshed boundary | No config or provider authority change |
| Provider/privacy truth | `ProviderPrivacyBoundarySummary` | Unknown route/transmission/risk remains unknown; no page-local green state | None |
| Resource/Web evidence | resource receipts, citation sets, task evidence views | Workspace exposes metadata-only event references; typed bodies remain behind their existing owners | No global Artifact Center added |

## 3. Review And Permission Source Chain

```text
AgentProposal
  before / after / reason / affected_path / source / expires_at
  -> build_review_decision_context
  -> ReviewItem.decisionContext

ToolPermission AgentProposal.after
  permission_scope_kind
  canonical_scope
  blocked_action
  -> action_bound parser OR network_policy parser
  -> PermissionDecisionContext
  -> approve disabled when context is incomplete
```

The permission projection does not substitute the global
`ProviderPrivacyBoundarySummary` for an arbitrary Web, MCP, A2A, plugin, or
tool target. It reports a permission-specific transmission boundary from the
exact canonical scope. Network scopes report `possible`, never `sent`, until
separate execution evidence proves transmission.

## 4. Workspace Source Chain

```text
Main Chat task summaries + TaskDetail + AgentRun + ReviewCenterViewModel
  -> TasksViewModel
  -> existing WorkspaceViewModel builder
     activeTask
     pendingReviewItems
     activity (TaskDetail.evidence_view.event_timeline, metadata only)
     recentTaskRefs
     ProviderPrivacyBoundarySummary
```

An old run-only row with unverified payload cannot become the active task. When
there is no running, waiting-permission, or blocked task, `activeTask` is absent
and activity is empty; recent history is not relabeled as current execution.

## 5. Preserved Safety Semantics

- Proposal approval is a decision, not durable application proof.
- Accepted with no materialization evidence remains `unknown`.
- Only refreshed backend materialization state `applied` is completion proof.
- Permission approval is disabled when the exact scope or transmission boundary
  is incomplete.
- Provider/privacy unknown remains unknown.
- Dispatch success transitions to refresh, not to success/completion.
- Product-safe activity contains bounded metadata, not raw prompt, tool input,
  output, credential, or authority material.

## 6. Known Backend Limits

- Review `Apply` remains disabled where no backend materialization request
  command exists.
- Resource/Web/artifact bodies are not embedded in Workspace; only typed source
  references are projected.
- Today remains a reviewed composition adapter rather than a backend
  `TodayViewModel`.
- Settings remains command orchestration rather than a composed backend
  `SettingsViewModel`.

These limits are explicit contracts. They do not authorize frontend inference.
