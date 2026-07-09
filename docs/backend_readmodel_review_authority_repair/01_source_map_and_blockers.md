# Source Map And Blockers

Status: source-backed preparation map plus R0-R6 implementation inventory.

This file is a map for Goal-mode implementation. File paths are intentionally
file-level source areas, not fragile line anchors.

## R0 Verification Scope

Verified on 2026-07-09 during Slice R0 with the required source scans:

```sh
rg -n \
  "ViewModel|ReviewItem|materialization|LifeStateProjection|ProposalStore::create_proposal|create_proposal\(" \
  src-tauri/src openlife-core/src frontend/src
rg -n \
  "getSystemDiagnostics\(|listProposals\(|listAgentRuns\(|listMainChatAgentTasks\(|getLifeModel\(" \
  frontend/src/pages frontend/src/utils
```

Additional classification scan included the adjacent raw-read calls that the
current pages use for the same product-truth reconstruction problem:
`getLifeStateProjection`, `getDailyGoals`, `getSchedulerConfig`,
`getMemoryTierStats`, `getLifeModelCurrentView`, and
`getLifeModelCompletion`.

Raw `rg` output was not used as a completion/readiness claim. Hits were
classified as backend owner, storage-only, product page/hook, preview-only,
technical/support surface, display-only helper, bridge/type declaration, test
fixture, or Phase7 expected-absent historical artifact.

## R0 Current Authority Classification

| Concern | Current backend source | Current frontend/transitional source | R0 classification | R0 blocker |
| --- | --- | --- | --- | --- |
| Shared pending/readiness/task/safe-mode state | `src-tauri/src/life_state_projection.rs` plus `openlife-core/src/agent/product_read_model.rs` for shared contract types | `frontend/src/utils/lifeStateProjection.ts` | Backend partial projection plus shared contract owner; frontend formatting helper only | Surface-specific read models still missing. |
| Shared ViewModel envelope | `openlife-core/src/agent/product_read_model.rs` | `frontend/src/tauri.ts`, `frontend/src/viewmodels/shared/viewModelEnvelope.ts` | Backend shared contract owner plus frontend mirror/alias | Frontend aliases must not become product truth owners. |
| Today limited ViewModel | `src-tauri/src/life_state_projection.rs` for shared projection only; `src-tauri/src/read_models/provider_privacy.rs` supplies the R5 provider/privacy boundary summary | `frontend/src/pages/TodayV2PreviewPage.tsx`, `frontend/src/viewmodels/today/todayViewModelAdapter.ts` | Preview-only frontend adapter plus backend provider/privacy boundary input | No backend TodayViewModel yet; provider/privacy fallback must remain unknown when the R5 summary is unavailable. |
| Review proposal creation and review center read model | `openlife-core/src/agent/review_workflow.rs`, `openlife-core/src/agent/review_item.rs`, `src-tauri/src/read_models/review_center.rs` | `frontend/src/pages/MailboxPage.tsx` consumes `ReviewCenterViewModel` for action/materialization state and proposal payloads for display | Backend governance boundary plus R2 ReviewCenter owner | R2 covers review action eligibility and materialization labels; broader LifeModel/Memory/Task surface owners remain later slices. |
| Proposal storage | `openlife-core/src/agent/proposal_store.rs` | None | Storage-only | Must not decide product action eligibility or durable apply state. |
| Proposal review read model | `openlife-core/src/agent/backend_contract_freeze.rs` plus R2 `openlife-core/src/agent/review_item.rs` | `frontend/src/pages/MailboxPage.tsx` | Partial historical proposal read model plus current ReviewItem owner | `ProposalReviewItem` remains partial/historical support; R2 `ReviewItem` is the current review action/materialization authority. |
| LifeModel read model and durable materialization | `openlife-core/src/agent/life_model_view_model.rs`, `src-tauri/src/read_models/life_model.rs`, `openlife-core/src/life_model_write_gateway.rs`, `src-tauri/src/life_model_write_gateway.rs`, `openlife-core/src/agent/memory_view_model.rs` | `frontend/src/pages/LifeModelPage.tsx`, `frontend/src/viewmodels/lifemodel/lifeModelViewModelAdapter.ts` | R3 backend `LifeModelViewModel` owner plus LifeModel write/materialization gateway; R5 `MemoryViewModel` owns memory-side linkage counts | Canonical summary remains fail-closed without materialized provenance; full LifeModel/Memory relation remains partial where lifecycle records lack explicit relation evidence. |
| Task lifecycle and controls | `openlife-core/src/agent/tasks_view_model.rs`, `openlife-core/src/agent/main_chat_runtime_contract.rs`, `openlife-core/src/tasks.rs`, `src-tauri/src/main_chat_task_controls.rs`, `src-tauri/src/read_models/tasks.rs` | `frontend/src/pages/RunsPage.tsx`, `frontend/src/pages/AgentRunDetail.tsx`, `frontend/src/pages/ChatPage.tsx`, `frontend/src/utils/runDisplaySummary.ts`, `frontend/src/utils/runtimeDisclosure.ts` | R4 backend `TasksViewModel`/`WorkspaceViewModel` owner plus display helpers | Some product/detail surfaces still carry display-only AgentRun evidence; task controls remain request eligibility only. |
| Memory lanes/lifecycle | `openlife-core/src/memory_gateway.rs`, `openlife-core/src/agent/memory_lifecycle.rs`, `openlife-core/src/agent/memory_view_model.rs`, `src-tauri/src/memory_gateway.rs`, `src-tauri/src/read_models/memory.rs` | `frontend/src/pages/MemorySearch.tsx`, `frontend/src/pages/settings/tabs/ReviewMemoryTab.tsx` | R5 backend `MemoryViewModel` owner plus gateway/storage primitives | Vector tier counts are storage telemetry only; full LifeModel relation remains partial where lifecycle records lack explicit relation evidence. |
| Provider/privacy boundary | `openlife-core/src/agent/provider_privacy_boundary.rs`, `src-tauri/src/read_models/provider_privacy.rs`, `openlife-core/src/privacy.rs`, `openlife-core/src/agent/model_router.rs`, `src-tauri/src/provider_validation.rs`, `src-tauri/src/main_chat_runtime_status.rs` | `frontend/src/utils/runtimeDisclosure.ts`, `frontend/src/utils/capabilityStatus.ts`, `frontend/src/pages/SettingsPage.tsx`, `frontend/src/pages/TodayV2PreviewPage.tsx` | R5 backend `ProviderPrivacyBoundarySummary` owner plus display helpers | Config/validation proves configured or stale state only; external transmission stays possible/unknown unless runtime route evidence proves it. |
| Tauri bridge and TS types | Backend command functions in `src-tauri/src/lib.rs` and modules listed there | `frontend/src/tauri.ts` | Product bridge/type mirror only | Type declarations are not proof that a backend read-model command or owner exists. |

## R1 Shared Contract Update

R1 adds the backend-owned shared contract:

- `openlife-core/src/agent/product_read_model.rs`

This module owns:

- `ViewModelEnvelope<T>`
- `ViewModelStatus`
- `EvidenceRef`
- `ProductAction`
- `DebugAction`
- `ReviewAction`
- `ReviewItemMaterializationStatus`
- `ProviderPrivacyBoundarySummary`
- `BackendEntityRef`

`frontend/src/tauri.ts` mirrors the serialized contract for TypeScript
consumers. `frontend/src/viewmodels/shared/viewModelEnvelope.ts` is now a
type-only transitional alias to that bridge mirror. Neither frontend file is a
backend owner.

R1 did not create any surface-specific owner. R2 now creates the review-center
owner. The following remain absent until their later slices:

- `src-tauri/src/read_models/life_model.rs`
- `src-tauri/src/read_models/tasks.rs`
- `src-tauri/src/read_models/workspace.rs`
- `src-tauri/src/read_models/memory.rs`
- `src-tauri/src/read_models/provider_privacy.rs`

## R2 ReviewCenter Update

R2 adds the backend-owned review surface contract and command:

- `openlife-core/src/agent/review_item.rs`
- `src-tauri/src/read_models/review_center.rs`

`ReviewItem` owns:

- `id`
- `type`
- `source`
- `status`
- `materializationStatus`
- `allowedActions`
- `risk`
- `expiresAt`
- `evidenceRefs`
- `targetRefs`
- `taskResumeRelation`

`ReviewCenterViewModel` owns `items` and summary counts by status, risk, and
materialization status. `accepted` proposal status is not treated as durable
apply proof; without refreshed backend materialization evidence, R2 reports
accepted proposal materialization as `unknown`. Memory lifecycle records can
upgrade a memory item to `applied`, `applying`, `failed`, or `rolled_back`.

`frontend/src/pages/MailboxPage.tsx` now uses `ReviewItem.allowedActions` for
Accept, Reject, Later, and Edit button eligibility. Missing ReviewItem/action
state fails closed. Mailbox still reads proposal payloads for display and
`LifeStateProjection` for the shared pending-count banner; those remaining raw
reads are not backend ownership claims.

## Shared Read-Model Surface

Current sources:

- `openlife-core/src/agent/product_read_model.rs`
- `src-tauri/src/life_state_projection.rs`
- `frontend/src/utils/lifeStateProjection.ts`
- `frontend/src/viewmodels/shared/viewModelEnvelope.ts`
- `frontend/src/viewmodels/today/todayViewModelAdapter.ts`
- `frontend/src/viewmodels/lifemodel/lifeModelViewModelAdapter.ts`

Current state:

- `product_read_model.rs` owns the shared backend envelope/action/evidence/
  review-action/provider-privacy contract.
- `LifeStateProjection` owns shared pending/readiness/task/safe-mode/tool
  permission/safe-path state.
- Frontend shared `ViewModelEnvelope` is a transitional alias to the Tauri
  mirror of the backend contract; it is not an owner.
- Today and LifeModel limited slices use frontend pure adapters and explicit
  unknowns.

Blocker:

- Surface-specific backend read models are still missing. The shared R1
  contract does not claim `ReviewCenterViewModel`, `LifeModelViewModel`,
  `TasksViewModel`, `WorkspaceViewModel`, `MemoryViewModel`, or
  `SettingsViewModel` exists.

Repair:

- Use the backend shared contract for future read-model slices.
- Keep `frontend/src/viewmodels/**` as display or preview-only until the
  backend owner exists.

## Review And Durable Materialization

Current sources:

- `openlife-core/src/agent/review_workflow.rs`
- `openlife-core/src/agent/proposal_store.rs`
- `openlife-core/src/agent/backend_contract_freeze.rs`
- `src-tauri/src/commands/proposal.rs`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/pages/ChatPage.tsx`

Current state:

- Current source and `single_system` guards treat `ReviewWorkflow` as the
  product proposal-creation governance boundary. The older Phase7 prep note
  that proposal creation was "not a single workflow" is historical baseline
  evidence for what had to be repaired, not permission to add new direct
  product proposal writers.
- `ProposalStore` is storage.
- `ProposalReviewReadModel` exists, but current `ProposalReviewItem` is a
  metadata-safe proposal review item, not a full product `ReviewItem`.
- Mailbox still infers acceptability, safe-path blocking, and applied wording in
  page code.
- Chat contains inline accept-and-resume flows for task continuity and tool
  permission flows.

Blocker:

- There is no unified backend `ReviewItem` with type, source relation,
  decision status, materialization status, allowed actions, expiry, risk,
  evidence refs, target refs, and task-resume relation.

Repair:

- Introduce backend `ReviewItem` and `ReviewCenterViewModel`.
- Move action eligibility to backend: approve, reject, edit, later, apply,
  resume, revoke, view evidence.
- Preserve the invariant that `approve` is decision-only, `apply` is a
  materialization request, and `resume` is a task-resume request.
- After any action dispatch, frontend must refresh the read model and render the
  refreshed status.

## LifeModel Read Model

Current sources:

- `openlife-core/src/agent/life_model_view_model.rs`
- `openlife-core/src/life_model.rs`
- `openlife-core/src/life_model_write_gateway.rs`
- `src-tauri/src/read_models/life_model.rs`
- `src-tauri/src/life_model_write_gateway.rs`
- `src-tauri/src/commands/life_model.rs`
- `src-tauri/src/commands/proposal.rs`
- `frontend/src/pages/LifeModelPage.tsx`
- `frontend/src/viewmodels/lifemodel/lifeModelViewModelAdapter.ts`

Current state:

- R3 adds backend `LifeModelViewModel` and Tauri
  `get_life_model_view_model` command.
- `LifeModelPage` consumes backend `LifeModelViewModel` for truth mode,
  dimension summaries, candidate/materialized changes, pending counts, manual
  override state, and memory-linkage summary.
- Frontend LifeModel view-model files are command delegates/type aliases; they
  no longer build product truth from raw LifeModel, diagnostics, completion,
  memory, and proposal primitives.
- `LifeModelWriteGateway` owns canonical LifeModel writes and base/current hash
  conflict checks.
- The R3 backend read model uses existing current-view patch/snapshot/current
  value evidence as narrow materialization proof.

Blocker:

- Canonical summary remains unavailable without refreshed materialized
  provenance and must stay fail-closed.
- Memory linkage is still partial even after R5 because lifecycle records do
  not yet encode every LifeModel truth relation.
- Other pages and hooks can still read raw LifeModel primitives until their
  later convergence slices.

Repair:

- Extend or refine backend `LifeModelViewModel` only with source-backed
  materialization/provenance evidence.
- Accepted proposal status must not become materialization proof unless
  LifeModel gateway/patch/snapshot evidence proves it.
- Manual override must remain governed and separate from proposal
  materialization.

## Tasks And Workspace

Current sources:

- `openlife-core/src/agent/main_chat_runtime_contract.rs`
- `openlife-core/src/tasks.rs`
- `src-tauri/src/main_chat_task_controls.rs`
- `src-tauri/src/main_chat_event_stream.rs`
- `frontend/src/pages/RunsPage.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/utils/runtimeDisclosure.ts`
- `frontend/src/utils/runDisplaySummary.ts`

Current state:

- AgentRun and Main Chat task/session/event primitives exist.
- R4 `TasksViewModel` owns merged task identity, lifecycle, blockers, review
  refs, allowed controls, latest result, and request-control semantics.
- R4 `WorkspaceViewModel` owns a limited current-work summary and consumes the
  R5 provider/privacy boundary summary.

Blocker:

- Runs and Chat can still carry display-only AgentRun evidence while converging
  toward fewer raw detail reads.
- Resume/retry/cancel/refresh controls remain request eligibility only and
  must not be presented as completion proof.

Repair:

- Keep action/result wording sourced from `TasksViewModel`.
- Extend workspace scope in later slices only after source-backed evidence
  exists.
- Frontend controls should be rendered from backend allowed controls.

## Memory Read Model

Current sources:

- `openlife-core/src/memory_gateway.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`
- `openlife-core/src/agent/memory_view_model.rs`
- `src-tauri/src/memory_gateway.rs`
- `src-tauri/src/read_models/memory.rs`
- `src-tauri/src/commands/proposal.rs`
- `frontend/src/pages/MemorySearch.tsx`
- `frontend/src/pages/settings/tabs/ReviewMemoryTab.tsx`

Current state:

- Memory lanes and lifecycle/materialization primitives exist.
- R5 `MemoryViewModel` owns lane counts, lifecycle counts, materialized/
  rolled-back state, ReviewItem refs, and partial LifeModel linkage.
- MemorySearch consumes `MemoryViewModel` for product memory counts while
  technical search/index/archive actions remain command-backed.

Blocker:

- Full LifeModel relation remains partial where lifecycle records do not carry
  explicit relation evidence.

Repair:

- Keep vector tier stats classified as storage telemetry, not readiness or
  materialization proof.
- Extend lifecycle relation evidence in later slices before claiming complete
  LifeModel/Memory truth linkage.

## Provider And Privacy Boundary

Current sources:

- `openlife-core/src/privacy.rs`
- `openlife-core/src/agent/provider_privacy_boundary.rs`
- `src-tauri/src/read_models/provider_privacy.rs`
- `src-tauri/src/provider_validation.rs`
- `src-tauri/src/main_chat_runtime_status.rs`
- `openlife-core/src/agent/model_router.rs`
- `frontend/src/utils/runtimeDisclosure.ts`
- `frontend/src/pages/SettingsPage.tsx`
- `frontend/src/pages/settings/tabs/ProviderTab.tsx`

Current state:

- Runtime disclosure, model router status, provider validation, privacy policy,
  and Settings provider data exist.
- R5 `ProviderPrivacyBoundarySummary` is exposed by a backend command and
  consumed by Settings, Workspace, and Today V2 preview.

Blocker:

- Config and validation cannot prove sensitive prompt transmission. External
  transmission remains possible or unknown unless runtime evidence proves it.

Repair:

- Preserve provider/privacy summary as the single product boundary source and
  keep frontend runtime disclosure helpers display-only.
- Add runtime route evidence to the summary later before claiming sent/not-sent
  beyond local-only or observed evidence.

## R6 Frontend Convergence Guard Update

Current state:

- Product pages with repaired backend owners are now guarded from restoring
  concrete page-local inference patterns:
  - Mailbox uses `ReviewCenterViewModel` for review action eligibility and
    materialization labels.
  - Chat and Runs use `TasksViewModel` for lifecycle, terminal delivery, and
    request-only task controls.
  - LifeModel uses backend `LifeModelViewModel` for product LifeModel truth.
  - MemorySearch and Settings consume backend `MemoryViewModel` and
    `ProviderPrivacyBoundarySummary` where those concerns are visible.
- `frontend/src/pages/TodayPage.tsx` remains a limited, projection-backed page;
  it does not claim a backend TodayViewModel or provider/privacy owner.
  `TodayV2PreviewPage` remains preview-only and uses the R5 provider/privacy
  summary when building its preview envelope.
- `frontend/src/utils/runtimeDisclosure.ts` and
  `frontend/src/utils/lifeStateProjection.ts` are display/formatting helpers.
  They do not call Tauri commands and do not own task lifecycle, materialized
  review state, Memory truth, or provider/privacy boundary truth.
- `frontend/src/pages/TodayPage.readModelConvergence.test.ts` is the frontend
  static guard for forbidden raw reconstruction patterns, and
  `single_system_r6_frontend_convergence_guards_repaired_authority` keeps the
  same contract in the Rust `single_system` authority suite.

Residual boundaries:

- Chat and AgentRun detail still carry raw run/task evidence for transcript,
  continuity, and detail display. That evidence is not allowed to grant product
  task controls or completion credit.
- Settings still mixes product settings and support diagnostics. Provider/
  privacy and Memory truth are backend-owned, but a full `SettingsViewModel`
  remains outside this phase.
- Today still needs a future backend Today summary or projection extension
  before it can own Today-specific classification beyond the current limited
  projection/daily-goal view.

## Support And Debug Visibility

Current sources:

- `frontend/src/pages/SettingsPage.tsx`
- `frontend/src/pages/settings/tabs/AdvancedTab.tsx`
- `frontend/src/utils/internalDebug.ts`
- `frontend/src/productShellContract.ts`
- `frontend/src/components/ProductShell.tsx`

Current state:

- Settings still mixes product settings, support diagnostics, advanced tools,
  provider details, privacy/data, plugin controls, and developer-only toggles.

Blocker:

- No backend/product policy clearly separates user-facing settings, support
  diagnostics, and developer-only internals.

Repair:

- Add Settings/support visibility policy to `SettingsViewModel` before any full
  Settings V2 rewrite.

## Raw Reconstruction Hotspots

R0 classified the current frontend raw-read scan hits in
`plans/openlife_single_system_phase1_inventory.json` and guards them in
`src-tauri/src/single_system_authority_tests.rs`.

| Path | Surface class | Raw sources / local reconstruction risk | Backend owner needed before convergence |
| --- | --- | --- | --- |
| `frontend/src/pages/ChatPage.tsx` | Product page | Diagnostics, scheduler config, LifeModel, daily goals, projection, task summaries, snapshots, skill details. | `WorkspaceViewModel`, `TasksViewModel`, `ReviewCenterViewModel`, `LifeModelViewModel`, provider/privacy summary. |
| `frontend/src/pages/MailboxPage.tsx` | Product page | Proposal payload list plus projection remain for display/counting; R2 moves action eligibility and materialization labels to backend `ReviewCenterViewModel`. | Later slices must continue reducing display-only raw proposal reads where LifeModel/Memory/Task owners supersede them. |
| `frontend/src/pages/RunsPage.tsx` | Product page | Lists `AgentRun` rows as supporting evidence/delete targets while R4 `TasksViewModel` supplies lifecycle, terminal delivery, review refs, and controls. | Later convergence can reduce raw run evidence reads, but lifecycle/control authority already belongs to `TasksViewModel`. |
| `frontend/src/pages/AgentRunDetail.tsx` | Product detail page | Fetches task summaries to relate run/task state locally. | `TasksViewModel`. |
| `frontend/src/pages/LifeModelPage.tsx` | Product page | R3 consumes backend `LifeModelViewModel`; remaining direct projection/build-session reads are display context for safe mode and builder-session status. | `MemoryViewModel` in R5 for full memory linkage; later slices may fold projection/build context into broader workspace read models. |
| `frontend/src/pages/SettingsPage.tsx` | Product/settings page | Diagnostics plus projection remain for support/settings display; Memory and provider/privacy product summaries now come from R5 backend read models. | `SettingsViewModel` and support/debug visibility policy. |
| `frontend/src/pages/MemorySearch.tsx` | Technical memory surface | Consumes `MemoryViewModel` for product memory lifecycle/materialization; diagnostics and archive reads remain technical action support. | Later convergence may separate technical memory tools from product Memory read-model surfaces. |
| `frontend/src/pages/TodayPage.tsx` | Product page | Projection plus daily goals; Today-specific classification remains limited and guarded from raw proposal/diagnostic/provider reconstruction. | Today-specific backend summary or projection extension. |
| `frontend/src/pages/TodayV2PreviewPage.tsx` | Preview-only page | Projection, daily goals, and R5 provider/privacy summary fed into a frontend-only adapter. | Backend TodayViewModel or explicit continued preview-only scope. |
| `frontend/src/pages/chat/useChatContext.ts` | Product hook | Diagnostics, scheduler config, and LifeModel primitives for Chat context. | Workspace/Tasks/LifeModel read models. |
| `frontend/src/pages/LifeModelEditor.tsx` | Manual override surface | LifeModel plus diagnostics around editor flows. | Governed manual override policy plus LifeModel read model labels. |
| `frontend/src/pages/BuilderPage.tsx` | Domain product page | Diagnostics-derived setup/readiness context. | Settings/provider/privacy read models before broad readiness claims. |
| `frontend/src/pages/CalibrationPage.tsx` | Domain product page | Raw LifeModel primitive reads. | LifeModel read model before truth claims. |
| `frontend/src/pages/VersionControl.tsx` | Support/admin page | Diagnostics-derived support status. | Settings/support visibility policy. |

Derived helper hotspots:

| Path | Current role | R0 classification |
| --- | --- | --- |
| `frontend/src/utils/runtimeDisclosure.ts` | Formats route, provider, boundary, task, blocker, proposal, and next-action labels from run/task/evidence fragments. | Display helper only; not provider/privacy, review, Memory, or task lifecycle owner. |
| `frontend/src/utils/runDisplaySummary.ts` | Formats AgentRun plus task summary into list/search rows. | Display helper only; not TasksViewModel owner. |
| `frontend/src/utils/capabilityStatus.ts` | Builds capability/setup labels from diagnostics, pending count, and current run. | Display helper only; not SettingsViewModel or provider/privacy owner. |
| `frontend/src/utils/lifeStateProjection.ts` | Formats backend `LifeStateProjection` and centralizes pending-count lookup. | Frontend projection helper only; backend owner remains `src-tauri/src/life_state_projection.rs`. |

Surface exclusions:

- Tests and fixtures can seed proposals or mock materialization fields, but they
  do not establish product authority.
- `frontend/src/tauriDev.ts` remains dev/test-only and must not be imported by
  product pages/components.
- Historical Phase7 objects marked expected-absent in the deletion manifest are
  deletion evidence, not files or modules to restore.
- `frontend/src/tauri.ts` type declarations are bridge mirrors only and cannot
  prove a backend command or read-model owner exists.

Convergence rule:

- Display formatting may remain frontend-local.
- Readiness, pending counts, action eligibility, materialization, task
  lifecycle, provider/privacy boundary, and LifeModel/Memory truth must come
  from backend read models once the owner exists.
