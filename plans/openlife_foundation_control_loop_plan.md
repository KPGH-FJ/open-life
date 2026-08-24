# OpenLife Foundation Control Loop Plan

Status: active

Current stage: Slice 7 — final deletion and documentation convergence

Gate 0 implementation status: complete in source and isolated native QA. The
release profile remains untouched and its legacy data must still be reviewed by
the user before any legacy code or dependency is retired.

## Objective

Bring OpenLife's ordinary local Work loop to the dependable baseline established
by leading desktop Agent products. A user must be able to choose a workspace,
model, permission mode, memory behavior, and task resources before sending;
follow and steer a durable Run; resolve a boundary decision; continue the same
Run; inspect the result; and recover the same facts after refresh or restart.

This plan repairs the vertical control loop. It does not add a second runtime,
planning platform, compatibility layer, or document-governance system.

## First-release boundary

Included:

- local Projects and folder scope;
- provider/model discovery, selection, readiness, and immutable Run binding;
- reasoning, permission, Memory, file, Skill, and tool controls before send;
- durable Task/Run progress, steering, stop, retry, resume, and interruption;
- Review decisions that continue or terminate the exact blocked action;
- global activity, needs-attention state, notifications, and history;
- Artifact preview, verification, export, undo, and provenance;
- safe migration of legacy LifeModel data before legacy code is retired;
- deletion of replaced code, dependencies, generated caches, and stale docs.

Deferred until this loop is dependable:

- Computer Use and browser automation;
- account connectors and connector marketplaces;
- scheduled or cloud-hosted tasks;
- arbitrary shell access;
- multi-Agent orchestration;
- advanced autonomous LifeModel learning.

Deferred capabilities must later use the same Task, scope, permission, Review,
receipt, and recovery contracts.

## Product control-loop invariant

```text
composer selection
  -> immutable Turn/Run admission
  -> runtime execution and live events
  -> durable receipts and read models
  -> just-in-time Review when required
  -> continuation of the same Run/action
  -> verified result and recoverable history
```

- The frontend selects and presents product intent; it does not reconstruct
  provider, permission, task, or completion truth.
- Runtime code binds exact Project, provider/model, resource, Memory, Skill,
  tool, network, and permission scopes before execution.
- Stores preserve the exact bindings, attempts, steering, decisions, files,
  results, and limitations needed for later inspection.
- Backend ViewModels restore those facts after switching Conversations,
  refreshing, or restarting.
- Review approval is not completion. The blocked action must either continue
  under the approved scope or reach an explicit terminal state.

## Evidence rule

Every behavior-changing slice must pass all applicable layers:

1. source contract and focused automated tests;
2. full Rust and frontend checks proportional to the change;
3. a native release-app task using real local persistence and filesystem scope;
4. refresh, Conversation switch, application restart, and recovery checks;
5. denied, offline, stale-scope, cancellation, and interruption paths;
6. user-visible provenance for the exact Run, model, resources, Review, and
   result.

Fixtures, mocks, scripted providers, and browser-shell tests remain controlled
evidence. They do not substitute for native or external-live proof.

## Gate 0 — protect data and establish the migration boundary

### Outcomes

- Existing release and QA data can be backed up and restored consistently.
- Legacy LifeModel YAML and version history have an explicit, reviewable path
  into the canonical v2 store.
- No legacy reader or dependency is removed before migrated data can be read,
  compared, exported, and rolled back.
- Build caches can be cleaned without deleting credentials, user notes,
  release evidence, or product data.

### Work

1. Add a read-only legacy LifeModel inventory and migration preview owned by
   the LifeModel application layer. It may expose structure, counts, digests,
   timestamps, conflicts, and redacted summaries; it must not silently write.
2. Define a typed legacy-to-v2 mapping for identity, preferences, values,
   relationships, goals, source metadata, version order, and unsupported
   fields. Unknown or conflicting data remains explicit.
3. Add an idempotent reviewed migration command that writes through the
   canonical LifeModel gateway, records source digests, and produces a receipt.
4. Add export and rollback evidence sufficient to recover the pre-migration
   state without treating a backup copy as canonical product truth.
5. Run migration only while ordinary product effects are quiescent. Native QA
   must include database checkpoint, integrity check, migration, restart,
   canonical readback, and rollback rehearsal.
6. Add narrow cleanup commands or documented commands for reproducible build
   caches. Never use a repository-wide ignored-file clean as the product
   cleanup mechanism.

### Stop conditions

- A profile containing legacy LifeModel data is not displayed as canonical
  empty without an explicit migration state.
- Repeating the migration is idempotent or returns a precise conflict.
- Canonical v2 readback, version relationships, source digests, export, and
  rollback are proven in a native isolated profile.
- Credentials, environment files, user notes, QA receipts, release bundles,
  and application data are outside cache-clean targets.

## Slice 1 — workspace and provider truth

### Outcomes

- Opening or creating a local Project binds one or more explicit folders.
- One primary folder is the default working root; secondary folders remain
  explicit readable resources.
- Every filesystem capability resolves paths from the admitted Run scope,
  never from the process current directory.
- The composer presents available provider/model choices and authoritative
  readiness before a Run is created.
- Project read roots, Artifact destinations, and additional authorized paths
  are separate typed scopes.

### Backend work

1. Make Project scope a complete application contract: create, update with
   revision, archive, restore, list, and safe deletion eligibility.
2. Normalize and canonicalize selected folder paths in the platform adapter;
   store only the product-safe scope facts required by Conversation and Run
   admission.
3. Persist an immutable Project scope snapshot and digest on every Work Run.
4. Change workspace file resolution and ToolGateway filesystem adapters to
   require that snapshot. Remove process-cwd fallback from production Work.
5. Turn provider profiles into a real registry instead of a single selected
   profile projection. Include provider, model, endpoint class, transport
   capabilities, reasoning support, local/cloud route, and readiness.
6. Expose local-model discovery through the same registry and readiness read
   model. Discovery failure must not erase manually configured profiles.
7. Scope provider/privacy summaries to the selected Conversation and Run.
   Global health remains a separate projection.
8. Split Project read roots, Artifact destination, and extra safe paths in
   configuration and ToolGateway contracts.

### Frontend work

1. Replace name-only Project creation with Open Folder, create-project, and
   edit-project flows using native folder selection.
2. Allow a newly created Project to be selected before the first message.
3. Show the primary folder, additional folders, scope status, and edit/archive
   actions without exposing unnecessary private path detail in diagnostics.
4. Add model and reasoning controls beside the composer. Settings defines
   profiles and defaults; the composer selects the Conversation/Run binding.
5. Present local, cloud, unverified, offline, stale, and degraded states from
   the authoritative readiness projection with one precise recovery action.
6. Keep mode, Project, model, reasoning, and permission controls visually
   available before send; hide advanced details without hiding current scope.

### Stop conditions

- A native Work task reads an allowed file from the selected Project while the
  app was launched from an unrelated directory.
- A traversal, symlink escape, secondary-folder omission, or unapproved path is
  blocked with a precise scope reason.
- Changing a Project while a Run is active does not mutate that Run's scope.
- Model discovery, manual configuration, offline state, and exact Run binding
  survive refresh and restart.
- Historical Tasks show the provider/model and Project scope they actually
  used, not the current global settings.

### Implementation checkpoint — 2026-08-23

Completed in the current checkout:

- native Open Folder creates or revision-binds a canonical Project root, and
  empty-workspace Project selection survives restart;
- Work snapshots Project id/revision/scope digest, resolves workspace files
  only from that root, and no longer adds process cwd to ToolGateway scope;
- an unavailable Project root now creates and terminalizes the exact blocked
  Task/Run before any provider or Artifact effect, while a retry whose original
  provider profile disappeared records a stale-scope attention item;
- the composer lists configured and discovered Ollama profiles, distinguishes
  authoritative cloud validation states, sends the selected profile with Chat
  and Work, and creates no partial Turn for an unknown/unavailable profile;
- each profile now exposes adapter protocol, structured-output enforcement,
  reasoning control, canonical observed Chat validation, and canonical observed
  Work compatibility as separate facts. A model with an observed Agent contract
  failure remains usable but is no longer presented as proven Work-ready;
- Conversation Turns persist and restore the exact provider/model binding;
- Tasks project the exact provider/model-bound Turn plus immutable Project
  name/revision/scope digest instead of current Settings values.
- Workbench provider/privacy evidence now resolves the exact active Run Turn
  when one exists, otherwise the latest provider-bound Turn inside the exact
  selected Conversation, instead of leaking the globally latest Conversation;
  the Settings summary remains explicitly global.
- Project lifecycle is now revision-bound end to end: active Projects can be
  renamed or folder-rebound, archive is reversible and requires no active
  linked Conversation, restore advances revision, and archived Projects remain
  visible with backend-owned blockers and allowed controls;
- permanent Project deletion now requires zero Conversation and canonical
  Task/Run references, fails closed when Task history is unavailable, rechecks
  both stores after native confirmation, and deletes only Project metadata—not
  the selected folder or its contents.
- native QA exposed and closed a first-turn gap: an existing or restored active
  Project can now be selected for a not-yet-created Conversation through a
  dedicated ConversationStore command, rather than enabling a frontend control
  with no durable backend owner.

Isolated native evidence:

- a QA-compiled bundle launched from `/tmp` selected `ollama · llama3:latest`,
  completed a real local Chat Turn, persisted the exact binding, kept its
  profile receipt secret in a mode-0600 local store, passed SQLite checkpoint
  and integrity gates, and restored the Conversation, reply, and model after
  restart;
- the same profile admitted a real Work Run under the selected Project with the
  exact model and Project scope. The model returned an invalid Artifact content
  type on its second structured step, so the runtime safely blocked with
  `agent_step_artifact_content_type_invalid`. After restart, the composer showed
  `Chat 已验证` and `Work 协议失败` from the persisted Turn/Task evidence. This is
  not evidence that the model is Work-compatible and must not be patched by
  relaxing the schema.
- a fresh QA bundle and isolated profile created a Project from a native folder
  picker, renamed it, archived it, restored it, selected the restored Project
  for the first not-yet-created Conversation, and re-archived it. The backend
  projected zero Conversation and Task/Run references and enabled deletion;
  invoking it opened the native system confirmation with exact metadata-only
  scope. The confirmation was cancelled, the Project remained archived, the
  user-owned marker file kept its original digest, and every isolated-profile
  SQLite database passed `integrity_check`. Automated backend coverage proves
  successful eligible metadata deletion while retaining the folder; this
  native run does not claim a user-confirmed permanent deletion or exact-signed
  release acceptance.

Regression checkpoint:

- Rust formatting and warning-denied Clippy pass;
- the full Rust suite passes with 1,048 executed tests and zero failures (five
  explicitly gated native/live tests remain ignored);
- frontend formatting, type checking, 259 tests, production build, and release
  authority guard pass;
- the 11-test browser-shell suite passes; it remains controlled browser
  evidence rather than native Tauri evidence;
- the regenerated development-profile build cache was removed with Cargo's
  profile-scoped clean (8.5 GiB in the latest run); release/QA bundles and
  application data were retained.

Typed filesystem-scope checkpoint:

- the overloaded `system.safe_paths` configuration has been replaced by
  `artifact_output_directory` and `additional_read_roots`; the old key migrates
  one-way to the narrower Artifact destination and is not retained as read
  authority;
- native Artifact directory selection changes only reviewed non-Project export
  scope, while canonical Work delivery remains bound to the exact Project or
  app-managed root and ToolGateway captures only additional read roots;
- editable Settings JSON cannot expand either filesystem scope. Controlled
  regressions cover traversal, an existing absolute path outside the Project,
  symlink escape, and an unrelated existing file outside the authorized read
  root;
- this is source and controlled-test evidence. A compatible external/native
  Work read and exact release-app acceptance are still separate evidence gates.
- Project scope now persists up to eight secondary readable roots with stable
  ids and labels. Native add/remove operations are revision-bound, survive
  database reopen, and update the scope digest; removal deletes metadata only.
- canonical Work admits the full Project read-root set, exposes only bounded
  root ids/names to the model, resolves one root-relative path, and grants
  ToolGateway access only to that selected root. The primary root remains the
  default, while secondary roots never become Artifact write destinations.
- the composer shows secondary root labels without exposing full paths and
  supports native add/remove with canonical ViewModel readback.
- user-selectable reasoning is now a typed provider/model capability, not a
  relabeling of the adapter's structured-output policy or a GPT-only name
  allowlist. Verified built-ins cover the documented OpenAI GPT-5.2 Codex,
  GPT-5.3 Codex, GPT-5.4, GPT-5.5, and GPT-5.6 effort sets, Google Gemini
  thinking levels, DeepSeek V4 thinking/effort, and Ollama GPT-OSS `think`
  levels. Unknown compatible models remain provider-default rather than being
  optimistically classified.
- official OpenRouter profiles discover the exact model's supported efforts,
  default, and mandatory state from its bounded `/models` metadata and cache
  that non-secret capability by provider generation. Dynamic router entries
  with no reasoning metadata expose no selector; discovery/network failure
  does not erase the manually configured model.
- the selected/default effort is admitted before Turn creation, persisted on
  the Conversation Turn and every canonical Work provider Attempt, preserved
  by retry, restored by Conversation and Task ViewModels, and translated only
  at the adapter boundary: OpenAI/Gemini flat `reasoning_effort`, OpenRouter's
  unified `reasoning` object, DeepSeek `thinking` plus high/max effort, and
  Ollama GPT-OSS `think`. Choosing model default now omits the override instead
  of materializing a guessed value. Unsupported values fail before a partial
  Turn is created.
- `minimal` is a first-class persisted effort. Explicit v7-to-v8 Conversation
  and v19-to-v20 Task migrations use named-column copies and foreign-key gates;
  the migration suite caught and prevented physical-column-order corruption in
  older databases.

Still required before Slice 1 is complete:

- one native Work file read must complete with a model proven compatible with
  the AgentStep contract; traversal/symlink/unapproved-root negatives currently
  have controlled-test evidence but still need native acceptance;

## Slice 2 — first-turn admission controls

- Make Project, model, reasoning, permission mode, Memory use/learn mode,
  files, Skill, and tools selectable before the first message.
- Create Conversation and Run from one typed admission request; remove
  unreachable preselection states and post-create patch-up behavior.
- Persist per-turn attachments and capability selections as bounded metadata.
- Gate Chat and Work with the same authoritative provider and persistence
  readiness facts.

Completed checkpoint:

- Memory use/learn mode is selectable before a Conversation exists. First send
  atomically persists the selected Memory mode and optional active Project in
  ConversationStore before any Turn/Run admission; an archived or unknown
  Project rejects the whole create and leaves no partial Conversation.
- Existing Conversations retain the dedicated Memory update command and accept
  the change only after canonical ViewModel readback. Automatic learning
  continues to read the persisted mode rather than frontend state.
- ResourceStore now exposes a bounded metadata-only attachment projection for
  the exact durable Turn binding. Conversation ViewModel messages preserve the
  Turn id, filename, detected type, format, digest, size, and chunk count after
  refresh without loading attachment bytes or extracted content.
- The transcript renders those restored file facts on the originating user
  message. An unreadable or unavailable ResourceStore is explicitly projected
  as unknown instead of being displayed as an authoritative empty attachment
  list; detach immediately disappears from the canonical projection.
- This attachment path has source, store, ViewModel, and component-test
  evidence. Application-restart and formal native acceptance remain required
  by the slice evidence rule.
- A Skill can now be selected while the Conversation is still a draft. First
  send validates its current availability and writes Project, Memory mode, and
  Skill id in the same ConversationStore admission; an unavailable Skill
  leaves no partial Conversation. Existing Conversations keep the canonical
  selection command, and every path preserves the rule that Skill context
  cannot elevate tool or write authority.
- Work now exposes an explicit pre-send execution ceiling using the converged
  leading-Agent pattern: standard scoped execution for ordinary recoverable
  in-scope work, or observe-only research. This is an immutable Run fact, not a
  ToolPermissionStore grant. Observe-only removes Artifact and
  personal-intelligence writes from provider tool schemas, plan eligibility,
  and direct typed-step execution; hostile write output terminalizes the Run as
  blocked without creating an Artifact.
- Task schema v21 persists the ceiling, v20 profiles migrate to the prior
  scoped-agent behavior, retries inherit the exact prior Run mode, and Tasks
  ViewModel plus Results restore the historical value. Focused store,
  runtime-negative, controller, and component tests cover those boundaries.
- The composer now consumes the complete backend tool-admission projection
  instead of reducing it to tool names. It distinguishes Run-admitted read
  tools, permission-at-use boundaries, policy selection reasons, blocked
  write/high-risk/unavailable tools, and controlled failure recovery. This is
  visibility over backend policy truth, not a frontend permission calculator.
- The existing backend Skill detail owner is now a registered release command
  and the selected Skill's allowed tools, disallowed surfaces, and required
  permissions are restored in the composer. The UI does not infer these facts
  from the Skill name or bounded instruction text.
- Settings now consumes a dedicated backend `ToolPermissionViewModel` with the
  exact tool, source, risk, action, policy, expiry, and usage state. The
  canonical revoke command admits only an active reusable record, while
  consumed, expired, and `allow_once` Review authority remain non-revocable.
  Successful revocation is accepted only after the backend model is refreshed;
  the frontend never mutates or reconstructs permission truth optimistically.

## Slice 3 — live execution and user control

- Stream typed plan, step, tool, observation, permission, Artifact,
  verification, and terminal events.
- Consume steering at canonical Work checkpoints and persist pending, applied,
  rejected, or blocked status.
- Replace keyword-based scope expansion checks with typed scope diffs.
- Define consistent Stop generation, Pause, Cancel Task, retry, and resume
  effects and confirmations.
- Preserve completion refresh when the user changes Conversation.

Completed checkpoint:

- Steering admission no longer inspects user text for literal scope-expansion
  markers. The command authenticates one exact Conversation `UserSteering`
  item and persists it pending against the current Task, Run, and plan
  revision; semantic handling belongs to canonical Work.
- Task schema v22 records `pending`, `applied`, `rejected`, and `blocked` with
  resolution code, resolved time, and applied plan revision. The v21 migration
  preserves historical pending/consumed/blocked records and maps consumed
  history to applied rather than deleting it.
- Canonical Work asks the selected provider for a new typed plan at safe
  checkpoints after planning, between governed tool steps, before terminal
  generation, and within the observation-bound Web loop. A direct first action
  is promoted to a persisted plan when Steering arrives during the initial
  provider decision.
- Replanning is confined to the Run's existing typed capability kinds and MCP
  targets plus authority-free analyze/verify/deliver steps. A new capability or
  target is blocked by the plan validator; malformed or semantically unusable
  revisions are rejected. Completed steps must remain byte-for-byte identical.
- Applying Steering is one transaction across Run revision, current plan,
  immutable plan history, Steering status, and Steering Item. Typed resolution
  events are streamed and Tasks ViewModel restores the exact lifecycle and
  applied revision after refresh.
- Run control now follows the terminal-Run pattern published by leading Agent
  systems: stopping an active Run is immediate and terminal for that Run;
  continuing creates a new Run on the same durable Task. The product contract
  is `stop_run` for Running, `retry` for Failed/Blocked, and `resume` for
  Cancelled/Interrupted. Every action is bound to the exact latest Run id.
- Retry and Continue have separate Tauri commands and status admission even
  though they share the restart coordinator. A failed Task cannot enter through
  Continue, an interrupted Task cannot enter through Retry, and a historical
  Run cannot be supplied as the restart target.
- React accepts Stop only after the same Task projects the targeted Run as
  cancelled. It accepts Retry or Continue only after the same Task projects a
  different new Run; a command return is not lifecycle proof.
- Switching Conversations detaches the visible transcript without discarding
  completion refresh. The finished Work callback carries its originating
  Conversation id and refreshes Results only when that Conversation is again
  selected, preventing one Conversation's completion from replacing another's
  projection.
- `refresh_context` was removed from the TaskControl contract because it had no
  action owner. Pause and outcome-level Cancel Task remain deliberately
  unadvertised until a durable safe-checkpoint owner and a distinct
  task-abandonment disposition exist. Stop current Run is not mislabeled as
  either capability.

## Slice 4 — Review continuation and permission management

- Link every permission Review item to the exact Task, Run, step, action, and
  freshness boundary.
- Continue or explicitly terminate the same blocked action after decision.
- Implement executable open-run, open-review, trace, evidence, apply, and revoke
  contracts where the backend advertises them. Generic resume and
  refresh-context are not Review actions.
- Complete the permission center's Review-to-Task continuation links. Exact
  scope, target, source, duration, usage state, and revocation are already
  projected from the canonical permission store in Settings.

### Implementation checkpoint — 2026-08-24

Completed in the current checkout:

- canonical TaskRuntime schema v23 owns a durable tool Review checkpoint bound
  to the exact Task, Run, ToolCall Item, Review Item, model-authored step,
  executor Action, Proposal, and metadata-safe permission-scope digest;
- a ToolGateway `NeedsConfirmation` result now creates a pending
  `ToolPermission` Review rather than terminalizing the whole Work Run as an
  unresumable generic blocker;
- ordinary action-bound permission and NetworkPolicy `ask` use distinct
  one-shot grant contracts. Both preserve the first blocked Attempt, and an
  accepted decision creates a second Attempt on the same ToolCall Item and
  same Run; the exact grant is consumed at ToolGateway dispatch;
- rejection terminalizes the same Run as blocked, while Stop can cancel a Run
  in either `running` or `waiting_review`. A stopped checkpoint cannot later be
  accepted to dispatch an action;
- a live in-process waiting owner is required before acceptance returns the Run
  to `running`. If Review is accepted after restart, the accepted permission
  fact is retained but the Run becomes `interrupted` and exposes new-Run
  Continue instead of fabricating same-process continuation;
- TasksViewModel now includes non-Artifact permission Review references and
  allows exact Stop on a waiting Review Run. Workbench already scopes and
  renders those Review items inline from the backend references.

Controlled evidence now covers action-bound permission continuation,
NetworkPolicy consent continuation, schema migration, rejected/cancelled
terminal states, and the missing-live-owner restart fallback. This remains
source/controlled evidence; the formal release app and profile were not
replaced or used as native acceptance evidence.

## Slice 5 — activity, history, deletion, and restart recovery

- Add one global activity surface for running, waiting, needs-input, ready,
  completed, failed, cancelled, and interrupted work.
- Add native notifications that deep-link to the exact Task or Review item.
- Add Conversation search, archive, restore, and safe deletion.
- Coordinate deletion across Conversation, Task, Resource, Review, Artifact,
  Memory-learning, and evidence owners; never orphan a running Task.
- Restore or clearly retry interrupted Runs after restart without losing their
  original immutable bindings.

### Implementation checkpoint — 2026-08-24

Completed in the current checkout at the controlled source/runtime level:

- the Workbench now has one global Activity surface backed by the canonical
  TasksViewModel rather than the selected Conversation. It restores running,
  waiting, attention, completed, failed, cancelled, and interrupted work after
  refresh and can open the exact Task across Conversation boundaries, including
  an archived Conversation whose original transcript remains available but
  whose next-Turn controls are locked until restore;
- Conversation search now covers active and archived records. Archive and
  restore are distinct backend lifecycle commands; archive fails closed while
  a Chat Turn or canonical Work Task is active;
- the old direct-delete path no longer cascades arbitrary Conversation history.
  Permanent deletion is limited to an archived, empty Conversation with zero
  Turn, message, and canonical Task references, is rechecked across both stores
  after native confirmation, and otherwise preserves history in place;
- Tauri's official notification contract was checked before implementation.
  Desktop notification actions are not being treated as proven deep links:
  the official action-button contract is mobile-only and the upstream desktop
  notification click-handler request remains open. Tauri's deep-link plugin can
  deliver statically configured schemes on desktop, including bundled macOS
  apps, but the notification plugin does not bind a desktop notification click
  to such a URL. Exact desktop Task/Review routing therefore still requires a
  verified app-activation owner rather than a decorative notification patch.

Regression evidence for the implemented Slice 5 boundary:

- warning-denied Rust Clippy, 1,055 executed Rust tests, frontend formatting,
  type checking, 265 tests, production build, and the 11-test browser-shell
  suite pass;
- focused integration coverage proves that global Activity opens the exact
  archived Conversation, restores its transcript, and keeps mode, Project
  assignment/creation, model, Memory, file, Skill, and message controls locked;
- this is controlled source/browser evidence. No native notification,
  app-activation deep link, or formal release-profile acceptance is claimed.

## Slice 6 — result and Artifact closure

- Preserve the existing preview, digest verification, open, export, and undo
  foundation.
- Add local failure feedback for every Artifact action.
- Show source resources, Run/model provenance, limitations, verification, and
  version relationships with the result.
- Support focused revision without silently replacing unrelated output.

### Implementation checkpoint — 2026-08-24

Completed in the current checkout:

- each Artifact card now projects the current/previous version relationship,
  exact source Run/provider/model binding, and local resources bound to that
  source Turn from the backend Tasks ViewModel. React does not substitute the
  latest Settings model or current Conversation attachments;
- opening and exporting still re-read the exact current ArtifactVersion,
  require matching recorded and observed digests, and verify exported bytes.
  Open, Export, and governed Undo request failures now remain local to the
  Artifact card with explicit feedback instead of becoming unhandled promises
  or success announcements;
- a controlled mixed local-resource/Web Artifact test proves that the source
  model and selected local resource survive materialization into the Result
  projection. A separate boundary test prevents the non-existent `v0` from
  being projected as the predecessor of an initial `v1`;
- completion with a transparent evidence limitation remains visibly distinct
  from full verification. Schema v24 now persists the exact requirement id,
  description, and evidence references for each limitation; deferred Artifact
  results copy the same validated list into FinalResult transactionally. The
  read model and Results UI render those entries as limitations rather than
  source support, while migrated legacy rows remain explicitly unavailable;
- schema v25 now retains existing target bytes in governed, digest-bound
  pre-change storage before a replacement can enter Review. Replacement Undo
  is a second high-risk Review whose canonical record binds the snapshot,
  restore digest, current-target digest precondition, ArtifactVersion, Run, and
  Project scope. A controlled Tauri integration test proves replacement,
  request, approval, physical restore, receipt projection, and terminal Undo;
  changed or unavailable bytes fail closed;
- schema v26 adds typed post-completion Artifact revision admission. A focused
  revision binds one exact verified current ArtifactVersion and instruction
  digest, creates a new Run under the original provider/model, Skill, resource,
  execution-mode, and Project scope, and retains every earlier FinalResult;
- the verified base is sent as bounded untrusted data to generation and
  independent semantic verification. The model cannot change target or media
  type, return multiple Artifacts, or receive completion credit for a no-op;
  replacement still waits for Review and creates the next ArtifactVersion;
- backend ReadModel availability, the Results form, exact Run-id IPC receipt,
  action-local failure states, v25-to-v26 migration, and controlled runtime
  coverage now close the source/browser contract. The integration proves the
  old file remains until approval, v1 and its FinalResult remain queryable,
  and approval materializes v2;
- first-decision direct Artifacts now pass the same independent semantic
  verifier as planned Artifact generation, closing a shorter-path verification
  bypass. Focused revision remains distinct from free-form follow-up, failed
  Run Retry/Continue, in-Run Steering, and filesystem Undo.

Regression checkpoint:

- warning-denied Rust Clippy and 1,062 executed Rust tests pass; five explicitly
  gated native/live tests remain ignored;
- frontend formatting, type checking, all 270 tests, production build, release
  authority guard, and the 11-test browser-shell suite pass;
- this does not replace isolated native acceptance or authorize mutation of the
  formal release profile.

## Slice 7 — final deletion and documentation convergence

- Remove legacy LifeModel readers, VersionManager state, and dedicated
  dependencies only after Gate 0 migration proof.
- Remove duplicate gateways, unused direct dependencies, dead scripts, stale
  ignore rules, and reproducible caches through bounded changes.
- Split oversized source owners only when the preceding slices establish the
  correct responsibility boundary.
- Update stable architecture documents in the same slice that changes their
  source truth. Delete this plan when the objective is complete; Git history is
  its archive.

### Implementation checkpoint — 2026-08-24

Completed source cleanup in the current checkout:

- removed the unconsumed core `LifeModelWriteGateway` decision shell. It had no
  runtime caller; the Tauri application materializer remains the single owner
  of canonical persistence admission, exact migration/typed-diff validation,
  commit serialization, and audit evidence;
- removed unused direct `serde_yaml` and `rusqlite` dependencies from the Tauri
  crate while preserving the core legacy migration dependencies protected by
  Gate 0;
- removed retired `product-audit` and tract-specific Makefile branches, stale
  phase-build ignore rules, and aligned the ordinary Rust and browser-shell
  Make targets with the repository's locked CI checks;
- made Cargo cleanup profile-scoped so it cannot delete release bundles, and
  removed release-bundle deletion entirely from the bounded native UI cleanup
  script. Stable testing documentation now states the exact protected data and
  evidence boundaries;
- consumer tracing kept Tailwind/PostCSS, current platform setup/dev/build
  scripts, security-audit ownership data, and the legacy LifeModel migration
  reader/version state. These have real build, CI, platform, or formal-profile
  protection consumers and are not cleanup candidates.

Regression checkpoint:

- warning-denied full-workspace Clippy and 1,058 executed Rust tests pass; five
  explicitly gated native/live tests remain ignored;
- frontend formatting, type checking, all 271 tests, production build, release
  authority guard, and the 11-test browser-shell suite pass;
- this source cleanup does not claim exact-native acceptance and did not touch
  the formal release application or profile.

Exact-native checkpoint:

- a newly built `OpenLife QA` bundle is bound to
  `ai.openlife.desktop.qa`, signed by `OpenLife Local Code Signing`, satisfies
  its Designated Requirement, and passes strict deep resource-seal validation;
- the QA build now blocks only a running instance of the exact profile being
  rebuilt. The installed release app remained running, its source-bundle
  executable digest stayed unchanged, and neither its process nor profile was
  touched;
- from an unrelated `/tmp` process cwd, a fresh isolated profile used the
  native folder picker to create and select a Project, discovered the running
  Ollama models in the composer, and bound `llama3:latest` to two real Work
  Runs. Both Runs failed closed with
  `agent_step_artifact_format_not_allowed`; no Result was claimed and the sole
  user-owned source file retained its exact digest;
- restart recovered the selected Project/model, both exact blocked Tasks and
  Runs, global Activity attention, retry-as-new-Run action, and Work
  compatibility failure. Every SQLite database in both isolated profiles
  passed checkpoint and `integrity_check`;
- the same local model completed a real Chat Turn with the requested exact
  reply. Native QA then exposed a frontend readback gap: the provider registry
  correctly persisted Chat validation, but the controller copied only messages
  from the post-Turn `ConversationViewModel`, so capability state appeared
  stale until restart;
- the controller now consumes provider and Work capability facts from the
  complete backend ViewModel after a Turn, failure refresh, cancellation, and
  Conversation switch. A fresh rebuilt and re-signed QA profile returned
  `FRESH-NATIVE-CHAT-OK` and immediately displayed `Chat 已验证` without a
  refresh or restart.

Open native gates:

- the installed local Llama model is proven Chat-capable and Work-contract
  incompatible for this Artifact schema. No compatible Work provider
  credential is configured in an isolated profile, so successful Project file
  read, Review continuation, Artifact revision, and governed Undo remain
  unclaimed rather than being simulated or patched around;
- desktop notification activation routing and formal-profile legacy LifeModel
  migration remain under their existing explicit boundaries.

## Current next action

Keep the formal release profile and its legacy data untouched. Preserve the
explicit Slice 5 notification limitation until a native desktop activation
owner is proven; do not emulate it in frontend state. Keep legacy LifeModel
readers until the formal-profile migration protection gate is explicitly
cleared. Configure a Work-compatible provider or model inside an isolated
profile without copying release credentials, then finish the outstanding
successful Project file-read, Review continuation, Result revision, and
governed Undo native gates. Delete this plan only after those gates are proven;
the current model-contract failure is evidence, not acceptance.
