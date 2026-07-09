# Slice Plan And Acceptance Gates

Status: Goal-mode execution plan.

The repair phase should be delivered as small slices. Each slice must leave the
repo in a coherent state and must not rely on a later slice to explain away a
false product claim.

## Slice R0: Source Map, Inventory, And Guards

Objective:

- Establish the current code inventory before any architecture shift.
- Add or update guard tests that prevent old authority patterns from silently
  returning.

Implementation targets:

- `plans/openlife_single_system_phase1_inventory.json`
- `src-tauri/src/single_system_authority_tests.rs`
- this preparation package, if implementation discoveries change the map.

Required scans:

```sh
rg -n \
  "ViewModel|ReviewItem|materialization|LifeStateProjection|ProposalStore::create_proposal|create_proposal\\(" \
  src-tauri/src openlife-core/src frontend/src
rg -n \
  "getSystemDiagnostics\\(|listProposals\\(|listAgentRuns\\(|listMainChatAgentTasks\\(|getLifeModel\\(" \
  frontend/src/pages frontend/src/utils
```

Acceptance:

- Current raw reconstruction hotspots are classified.
- Guard tests distinguish product code from tests, docs, dev-only, and
  expected-absent Phase7 artifacts.
- No implementation claim is made from raw `rg` output without surface
  classification.

Suggested gates:

```sh
git diff --check
cargo test -p openlife-tauri single_system -- --nocapture
```

## Slice R1: Backend Shared ViewModel Contract

Objective:

- Move shared envelope/action/evidence/review/provider privacy contract
  ownership to backend types.

Implementation targets:

- New backend contract module, recommended:
  `openlife-core/src/agent/product_read_model.rs`
- Tauri bridge type mapping in `frontend/src/tauri.ts`
- Focused backend contract tests.

Required contract pieces:

- `ViewModelEnvelope<T>`
- `ViewModelStatus`
- `EvidenceRef`
- `ProductAction`
- `DebugAction`
- `ReviewAction`
- `ReviewItemMaterializationStatus`
- `ProviderPrivacyBoundarySummary`
- `BackendEntityRef`

Acceptance:

- Backend owns the canonical shared types.
- Frontend `frontend/src/viewmodels/shared/viewModelEnvelope.ts` is either
  mirrored from backend semantics or explicitly marked transitional.
- No backend type claims a surface-specific read model exists before that slice
  implements it.
- `ReviewAction.kind` and `ReviewAction.effect` invariant is tested.

Suggested gates:

```sh
cargo fmt --check
cargo test -p openlife-core product_read_model -- --nocapture
corepack pnpm --dir frontend typecheck
```

## Slice R2: ReviewItem And ReviewCenterViewModel

Objective:

- Create the backend authority for review grouping, action eligibility, and
  durable apply/materialization status.

Implementation targets:

- `openlife-core/src/agent/review_workflow.rs`
- `openlife-core/src/agent/proposal_store.rs`
- `openlife-core/src/agent/backend_contract_freeze.rs`
- New backend/Tauri read-model module for Review Center.
- `src-tauri/src/commands/proposal.rs`
- `frontend/src/pages/MailboxPage.tsx` only after backend read model exists.

Required data:

- `ReviewItem.id`
- `ReviewItem.type`
- `ReviewItem.source`
- `ReviewItem.status`
- `ReviewItem.materializationStatus`
- `ReviewItem.allowedActions`
- `ReviewItem.risk`
- `ReviewItem.expiresAt`
- `ReviewItem.evidenceRefs`
- `ReviewItem.targetRefs`
- `ReviewItem.taskResumeRelation`
- `ReviewCenterViewModel.items`
- summary counts by status, risk, and materialization state.

Acceptance:

- Frontend no longer decides acceptability from proposal type, safe mode, or
  safe paths when `ReviewItem.allowedActions` is present.
- `accepted` proposal status is not rendered as applied unless the refreshed
  read model says materialization is applied.
- `apply` and `resume` are request actions, not proof of completion.
- Backend rejects or disables mismatched action/effect combinations.
- Existing proposal acceptance still goes through safe-mode and domain gateway
  checks. Base-hash or stale-conflict checks are domain-specific freshness
  evidence, especially for canonical LifeModel materialization, and must not be
  assumed for every `ReviewItem` type.

R2 repair note:

- `ReviewItem.taskResumeRelation` now includes
  `resumeRequiresMaterialization`. Durable proposal types such as MemoryWrite,
  LifeModel updates, and external writes cannot request task resume from
  `accepted` alone while materialization is unknown.
- Tool-permission resume remains possible only because the backend relation
  explicitly declares materialization is not required. Resume remains a
  `TaskResumeRequest`, not completion or durable-apply proof.

Suggested gates:

```sh
cargo fmt --check
cargo test -p openlife-core review_workflow -- --nocapture
cargo test -p openlife-tauri single_system -- --nocapture
corepack pnpm --dir frontend test -- MailboxPage
corepack pnpm --dir frontend typecheck
```

## Slice R3: LifeModelViewModel Backend Owner

Objective:

- Replace frontend-only LifeModel limited truth assembly with a backend-owned
  read model.

Implementation targets:

- `openlife-core/src/life_model.rs`
- `openlife-core/src/life_model_write_gateway.rs`
- `src-tauri/src/life_model_write_gateway.rs`
- `src-tauri/src/commands/life_model.rs`
- New backend/Tauri read-model module for LifeModel.
- `frontend/src/viewmodels/lifemodel/**`
- `frontend/src/pages/LifeModelPage.tsx` only for consumption convergence.

Required data:

- `truthMode`
- `canonicalSummary`
- `currentViewSummary`
- dimension summaries with provenance and owner status.
- `candidateChanges`
- `materializedChanges`
- `manualOverrideState`
- `memoryLinkage`
- `relatedReviewItemRefs`
- `contractLimitations`

Acceptance:

- This slice adds a backend command for `LifeModelViewModel`.
- Frontend no longer has to combine raw LifeModel, current view, diagnostics,
  completion, memory counts, and proposals to decide truth mode.
- Accepted proposals are counted as approved-not-applied unless gateway/patch/
  snapshot evidence proves materialization.
- Stale base-hash conflicts remain fail-closed and tested.
- Manual override is governed and separated from accepted proposal
  materialization.

R3 implementation note:

- Implemented backend contract in
  `openlife-core/src/agent/life_model_view_model.rs`.
- Implemented Tauri command in `src-tauri/src/read_models/life_model.rs` and
  registered `get_life_model_view_model`.
- `frontend/src/pages/LifeModelPage.tsx` now consumes
  `getLifeModelViewModel`; it no longer calls raw LifeModel/current-view/
  diagnostics/completion/memory/proposal reconstruction APIs.
- `frontend/src/viewmodels/lifemodel/*` is now a TypeScript mirror/delegate,
  not a backend owner.
- Canonical summary and memory linkage remain deliberately limited/fail-closed
  until source-backed materialized provenance and R5 MemoryViewModel exist.

Suggested gates:

```sh
cargo fmt --check
cargo test -p openlife-core life_model_write_gateway -- --nocapture
cargo test -p openlife-tauri life_model -- --nocapture
cargo test -p openlife-tauri single_system -- --nocapture
corepack pnpm --dir frontend test -- lifeModelViewModel LifeModelPage
corepack pnpm --dir frontend typecheck
```

## Slice R4: TasksViewModel And Workspace Baseline

Objective:

- Create backend-owned task/run identity, lifecycle, blocker, review relation,
  and allowed control semantics before Workspace or Tasks V2 UI work.

Implementation targets:

- `openlife-core/src/agent/main_chat_runtime_contract.rs`
- `openlife-core/src/tasks.rs`
- `src-tauri/src/main_chat_task_controls.rs`
- `src-tauri/src/main_chat_event_stream.rs`
- New backend/Tauri read-model module for Tasks.
- Later, a limited backend Workspace read model.
- `frontend/src/pages/RunsPage.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/utils/runtimeDisclosure.ts`
- `frontend/src/utils/runDisplaySummary.ts`

Required data:

- Canonical task id and related run ids.
- Lifecycle status.
- Terminal/final delivery status.
- Pending blockers.
- Pending review item refs.
- Allowed controls.
- Resume/retry/cancel/refresh eligibility.
- Latest result preview and evidence refs.

Acceptance:

- Runs and Chat render task controls from backend allowed controls.
- Inline accept-and-resume flows either dispatch backend review actions or
  refresh the backend read model before resuming.
- Completed task wording is fail-closed when final delivery evidence is missing
  or pending review remains.

R4 repair note:

- `TasksViewModel` treats `final_delivery_present=true` with missing
  `final_delivery_status` as missing evidence. Only `completed` or `delivered`
  counts as delivered.
- `completed_with_pending_items` maps to pending-review semantics, not plain
  completed. `blocked`, `failed`, and `cancelled` map to their terminal
  fail-closed states.
- `TaskDetail.final_delivery` is no longer created from stored final summary
  alone. A final-result transcript entry must exist, and the task-detail layer
  now emits explicit canonical status evidence when it can derive it.

Suggested gates:

```sh
cargo fmt --check
cargo test -p openlife-tauri main_chat_task_controls -- --nocapture
cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture
cargo test -p openlife-tauri single_system -- --nocapture
corepack pnpm --dir frontend test -- RunsPage ChatPage
corepack pnpm --dir frontend typecheck
```

## Slice R5: MemoryViewModel And ProviderPrivacyBoundarySummary

Objective:

- Own Memory product state and provider/privacy boundary in backend read models.

Implementation targets:

- `openlife-core/src/memory_gateway.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`
- `src-tauri/src/memory_gateway.rs`
- `src-tauri/src/provider_validation.rs`
- `src-tauri/src/main_chat_runtime_status.rs`
- `openlife-core/src/agent/model_router.rs`
- New backend/Tauri read-model modules for Memory and provider/privacy.
- `frontend/src/pages/MemorySearch.tsx`
- `frontend/src/pages/SettingsPage.tsx`
- `frontend/src/utils/runtimeDisclosure.ts`
- Today/LifeModel/Workspace consumers.

Required Memory data:

- lane counts.
- lifecycle counts.
- candidate, confirmed, materialized, rolled-back, expired, archived states.
- provenance and evidence refs.
- ReviewItem refs.
- LifeModel linkage and conflict counts.

Required provider/privacy data:

- route type.
- external transmission status.
- provider and model labels.
- privacy label.
- risk.
- local-only requirement.
- blocked reason.
- evidence refs.

Acceptance:

- Today and Settings no longer invent provider/privacy boundary locally.
- Memory product readiness is not claimed from tier stats alone.
- Memory writes and archival actions remain governed by safe mode and gateway
  rules.
- Sensitive/external transmission unknown remains unknown.

R5 repair note:

- `ProviderPrivacyBoundarySummary` no longer infers `NotSent`, `Local`, or
  `Low` risk from `prefer_local_model` or `local_only_required` without
  runtime route evidence.
- When `latest_external_transmission` is missing, external transmission remains
  `unknown` or `possible` with an evidence-gap warning. Cloud configuration and
  provider validation do not prove either sent or not sent.

Suggested gates:

```sh
cargo fmt --check
cargo test -p openlife-core memory_gateway -- --nocapture
cargo test -p openlife-tauri life_state_projection -- --nocapture
cargo test -p openlife-tauri single_system -- --nocapture
corepack pnpm --dir frontend test -- MemorySearch SettingsPage todayViewModel
corepack pnpm --dir frontend typecheck
```

## Slice R6: Frontend Convergence And Anti-Hallucination Guards

Objective:

- Make product pages consume backend read models and prevent regression to
  page-local product truth.

Implementation targets:

- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/RunsPage.tsx`
- `frontend/src/pages/LifeModelPage.tsx`
- `frontend/src/pages/SettingsPage.tsx`
- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/utils/lifeStateProjection.ts`
- `frontend/src/utils/runtimeDisclosure.ts`
- `src-tauri/src/single_system_authority_tests.rs`
- Frontend static tests for forbidden raw reconstruction patterns.

Acceptance:

- Product pages consume backend read models for readiness, pending counts,
  allowed actions, materialization, task lifecycle, Memory lanes, LifeModel
  truth, and provider/privacy boundary.
- Frontend local helpers are display formatters only.
- Unknown/stale/error states remain visible and fail closed.
- Preview-only adapters remain clearly preview/transitional or are removed from
  product paths.

R6 implementation note:

- Added `frontend/src/pages/TodayPage.readModelConvergence.test.ts` to statically
  guard repaired frontend surfaces against concrete forbidden reconstruction
  patterns.
- Added
  `single_system_r6_frontend_convergence_guards_repaired_authority` to the Rust
  `single_system` suite so the frontend guard and product-page consumption
  contract are protected by the repository authority gate.
- No full Frontend V2, ProductShell replacement, or backend Today/Settings
  ViewModel was introduced. Today remains projection-backed/limited, and
  Settings remains a mixed settings/support page while Memory and provider/
  privacy product truth come from R5 backend read models.
- Chat task-continuity final delivery rendering now shows explicit status and
  treats missing status as missing evidence, not as a recorded/completed
  delivery. Mailbox surfaces backend resume blockers from
  `taskResumeRelation` instead of enabling resume locally.

Suggested gates:

```sh
git diff --check
cargo fmt --check
cargo test -p openlife-tauri single_system -- --nocapture
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend test -- \
  MailboxPage ChatPage RunsPage LifeModelPage SettingsPage TodayPage
```

## Final Phase Acceptance

The full repair phase is acceptable only when:

- Every new backend read model has focused unit/contract tests.
- Every frontend product consumer has tests that prove it renders backend
  unknown/stale/error/materialization states without overclaiming.
- Static guards catch at least one concrete forbidden pattern per repaired
  concern.
- `git diff --check`, focused Rust tests, focused frontend tests, frontend
  typecheck, and format checks pass or have documented bounded failures.
- Phase7 remains accurately described as blocked until product trial evidence
  changes.
- This repair does not provide live-provider, external transmission,
  durable-materialization, or product-trial readiness proof where the backend
  read models still report unknown or pending states.
