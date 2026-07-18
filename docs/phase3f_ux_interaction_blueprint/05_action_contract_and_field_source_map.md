# Action Contract And Field Source Map

Status: `PARTIAL_BLOCKED`

## 1. Action Families

### 1.1 ProductAction

Authoritative shape: `openlife-core/src/agent/product_read_model.rs`.

| Field | UI use | Rule |
|---|---|---|
| `id` | stable event and analytics key | required; never derived from label |
| `label` | user command | Chinese product language may format backend label, but meaning cannot change |
| `kind` | open/start/continue/retry/cancel/refresh/inspect/configure | determines component and expected navigation/dispatch class |
| `enabled` | native disabled state | false always wins over page logic |
| `disabledReason` | adjacent explanation and accessibility description | required when disabled |
| `targetRef` | navigation/command target | must match the refreshed target before outcome rendering |

### 1.2 ReviewAction

| Field | UI use | Rule |
|---|---|---|
| `id` | exact decision action id | never replace with generic approve handler |
| `label` | product decision label | may map to plain Chinese wording |
| `kind` | approve/reject/edit/later/revoke/apply/resume/view_evidence | keeps decision, materialization, task, and evidence lanes separate |
| `effect` | decision_only/materialization_request/task_resume_request/evidence_only | frontend rejects mismatched kind/effect |
| `enabled` | disabled state | backend false wins |
| `disabledReason` | why action cannot run | visible near control |
| `requiresConfirmation` | dialog requirement | confirmation content uses decision context, never raw JSON |
| `targetReviewItemId` | dispatch target | refreshed item id must match |
| `expectedMaterializationStatusAfterDispatch` | expectation only | never treated as returned proof |

### 1.3 TaskControl

Authoritative shape: `openlife-core/src/agent/tasks_view_model.rs`.

| Field | UI use | Rule |
|---|---|---|
| `id` | exact task control | required |
| `kind` + `effect` | resume/retry/cancel/refresh/navigation/evidence | mismatch is a contract error |
| `enabled` + `disabledReason` | control availability | frontend does not broaden |
| `requiresConfirmation` | confirmation dialog | cancellation and risky resume may require it |
| `targetTaskId` | task dispatch target | exact match required |
| `targetActionId` | retry/action replay target | cannot be inferred from selected row |
| `completionProofAfterDispatch` | always false in current contract | dispatch never proves completion |

### 1.4 DebugAction

Raw trace, JSON, provider health, route evidence, transcript, and export actions
are separated in `debugOnly`. They render only in Inspector/Advanced and never
share the primary action bar.

## 2. Proposed Readable Decision Projection

The following is a target contract, not current code:

```ts
interface ReviewDecisionContext {
  reviewItemId: string;
  title: string;
  summary: string;
  before?: ReadableValue;
  after?: ReadableValue;
  reasonSummary: string;
  sourceSummary: string;
  impactSummary: string;
  affectedObjectLabels: string[];
  expiresAt?: string;
  permission?: PermissionDecisionContext;
  evidenceRefs: EvidenceRef[];
}

interface PermissionDecisionContext {
  scopeKind: "action_bound";
  policy: "allow_once";
  toolLabel: string;
  toolName: string;
  capabilityLabels: string[];
  requestedTargetLabel: string;
  resolvedTargetLabel: string;
  purposeSummary: string;
  inputDigest: string;
  inputLengthBytes: number;
  blockedRunId: string;
  blockedStepIndex: number;
  routeBoundary: ProviderPrivacyBoundarySummary;
}
```

This must be projected in the backend from authoritative proposal, manifest,
action queue, task, provider/privacy, and evidence owners. The frontend must not
join raw `AgentProposal.after` and task fragments to recreate it.

## 3. Screen Field Sources

### 3.1 Today

| UI field | Source | Classification |
|---|---|---|
| daily task name/done/time block/due | `get_daily_goals` from canonical StateStore compatibility projection | `PRODUCT_BRIDGE` |
| pending review count | `LifeStateProjection` or Review Center summary | `PRODUCT_READ_MODEL` |
| safe/readiness state | `LifeStateProjection` | `PRODUCT_READ_MODEL` |
| provider/privacy boundary | `ProviderPrivacyBoundarySummary` | `PRODUCT_READ_MODEL` |
| one current focus sentence | future Today projection; prototype story content | `TARGET_CONTRACT` / `LAYOUT_FIXTURE` |
| “will not auto-send/write” boundary | action/write policy plus current enabled actions | `TARGET_CONTRACT` summary |

Today may format canonical daily tasks. It may not create, complete, reorder, or
infer goals locally.

### 3.2 Workspace

| UI field | Source | Classification |
|---|---|---|
| active task ref, recent refs, review refs, compact timeline | `WorkspaceViewModel` | `PRODUCT_READ_MODEL`, limited |
| title, lifecycle, blockers, controls, next control, final evidence | referenced `TasksViewModel` item/current task state | `PRODUCT_READ_MODEL` |
| action/observation details | task transcript/events projected for product use | `TARGET_CONTRACT` where current fragments are too raw |
| imported files | exact `ResourceImportReceipt` for current turn | `PRODUCT_BRIDGE` |
| selected resource citations | backend-issued ResourceCitation result | `VERIFIED_BACKEND` / bridge result |
| Web action and citations | current task action/observation and Web citation set | `VERIFIED_BACKEND`, product projection partial |
| provider route | `ProviderPrivacyBoundarySummary` and current task/provider receipt | `PRODUCT_READ_MODEL` / evidence |
| composer draft | page-local ephemeral input | `LOCAL_EPHEMERAL`, never product truth |

### 3.3 Tasks

| UI field | Source | Classification |
|---|---|---|
| counts and lifecycle filters | `TasksViewModel.summary` | `PRODUCT_READ_MODEL` |
| list rows | `TasksViewModel.items` | `PRODUCT_READ_MODEL` |
| task controls | `allowedControls` | `PRODUCT_READ_MODEL` |
| latest result | `latestResultPreview` + terminal evidence | `PRODUCT_READ_MODEL` |
| deep timeline/raw transcript | task detail/evidence | advanced, not inferred |
| fixture titles/times in prototype | Phase 3F scenario data | `LAYOUT_FIXTURE` |

### 3.4 Review Center

| UI field | Source | Classification |
|---|---|---|
| queue grouping | `ReviewCenterViewModel.batches/items` | `PRODUCT_READ_MODEL` |
| decision/materialization/risk/expiry | `ReviewItem` | `PRODUCT_READ_MODEL` |
| actions and task resume relation | `ReviewItem.allowedActions/taskResumeRelation` | `PRODUCT_READ_MODEL` |
| before/after/reason/impact | proposed `ReviewDecisionContext` | `TARGET_CONTRACT`, blocking |
| exact permission scope | proposed `PermissionDecisionContext` | `TARGET_CONTRACT`, blocking |
| evidence id/label/source/sensitivity | `EvidenceRef` | `PRODUCT_READ_MODEL` |
| evidence summary/body | future typed evidence detail command | `TARGET_CONTRACT`; current EvidenceRef alone is metadata |

### 3.5 LifeModel And Memory

| UI field | Source | Classification |
|---|---|---|
| truth mode/current/canonical summary | `LifeModelViewModel` | `PRODUCT_READ_MODEL` |
| dimensions/confidence/stale/provenance | `LifeModelViewModel.dimensionSummaries` | `PRODUCT_READ_MODEL` |
| candidates/applied/rollback | candidate and materialized changes | `PRODUCT_READ_MODEL` |
| memory lifecycle/linkage | `MemoryViewModel` and LifeModel memory linkage | `PRODUCT_READ_MODEL`, limited |
| applied completion | accepted proposal + applied patch/effect + snapshots/current match | `VERIFIED_BACKEND` rule |
| readable sample preferences | Phase 3F user story | `LAYOUT_FIXTURE` |

### 3.6 Settings

| UI field | Source | Classification |
|---|---|---|
| provider/model/base URL/local preference | sanitized `get_config` | `PRODUCT_BRIDGE` |
| current route/transmission/risk | `ProviderPrivacyBoundarySummary` | `PRODUCT_READ_MODEL` |
| connection validation result | `LlmConnectionTestResult` | `PRODUCT_BRIDGE` |
| save state before refreshed summary | frontend command lifecycle | `LOCAL_EPHEMERAL` |
| permissions | `list_tool_permissions` plus future readable grouping | `PRODUCT_BRIDGE` / `TARGET_CONTRACT` |
| export/import/recovery | danger preflight and governed commands | `PRODUCT_BRIDGE` |
| model/provider options in static prototype | plausible layout examples | `LAYOUT_FIXTURE` |

## 4. Phase 3F Prototype Action Matrix

Every enabled prototype control has a deterministic visible result.

| Prototype action | Contract family | Static result |
|---|---|---|
| navigate between product pages | ProductAction Open | screen changes, heading focus, live announcement |
| open/close Inspector | ProductAction Inspect | panel/sheet state, focus trap on mobile, focus restoration |
| select task/filter | local presentation | selected row/detail or empty filter state |
| attach fixture | ProductAction Open demonstration | import lifecycle appears; explicitly marked static |
| cancel/detach fixture import | product bridge demonstration | receipt-style result and announcement |
| approve exact permission | ReviewAction Approve then TaskControl Resume demonstration | confirmation -> reviewing -> refreshed -> resuming -> running fixture |
| approve unknown permission | ReviewAction Approve | disabled with reason |
| reject/later/edit Review item | ReviewAction | explicit result/pending-edited state, no hidden completion |
| approve Review item | ReviewAction Approve | approved-not-applied state only |
| apply approved item | ReviewAction Apply | disabled because current command gap is preserved |
| settings search/category | local presentation | filtered categories/content and result announcement |
| connection test fixture | ProductAction Configure demonstration | confirmation -> testing -> exact test result; no save |
| save settings fixture | ProductAction Configure demonstration | saving -> saved-awaiting-boundary-refresh -> unknown fixture |
| unavailable entry | ProductAction Open | explicit unavailable dialog/state |

## 5. Blocking Contract Work Before React

1. Project `ReviewDecisionContext` from backend authority.
2. Project readable action-bound permission context and current transmission
   boundary into Review/Workspace.
3. Decide the V2 Workspace composition owner rather than enlarging the limited
   WorkspaceViewModel by page-local joins.
4. Define a bounded Today read model or explicitly retain a reviewed adapter
   over StateStore daily tasks and LifeStateProjection.
5. Define product-safe resource/Web/artifact timeline entries.
6. Define a composed Settings ViewModel or a strict frontend orchestration
   contract across config, validation, privacy summary, and permissions.

Until these are reviewed, the Phase 3F prototype is a target interaction
specification, not an implementation map that permits arbitrary frontend joins.
