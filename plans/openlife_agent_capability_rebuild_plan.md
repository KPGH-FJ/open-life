# OpenLife Agent Capability Rebuild Plan

Status: active

Current stage: Stage 7 — Product matrix, native/live proof, and final deletion

## Objective

Deliver one capable, general-purpose personal Agent whose behavior matches the
public product patterns of leading Agent tools: the model understands the goal,
plans when useful, chooses eligible tools adaptively, observes results, and
continues until it can return a verifiable result. Deterministic code enforces
scope, risk, schemas, budgets, receipts, and completion evidence; it does not
replace semantic understanding with keyword routing.

This is a clean replacement of retained half-products, not another wrapper
around them. Each stage closes a user-visible capability through runtime,
persistence, read model, frontend, recovery, deletion, and proportionate
evidence before moving on.

## Product contract

- Chat and Work share Conversation, Turn, Item, provider, context, streaming,
  cancellation, and recovery infrastructure. Work adds Task, Run, plan Items,
  tools, Artifacts, Review checkpoints, and verified FinalResult.
- A Project is optional organization and file scope. A Conversation is the
  user's ongoing thread. Neither is a second task lifecycle.
- The user selects the provider and model. OpenLife may retry that exact route
  within bounded policy, but never silently switches provider or model.
- One provider-agnostic Agent loop owns goal understanding, planning, tool
  choice, observation handling, and completion. Provider profiles describe
  only endpoint, credential, streaming, structured-output, reasoning, and
  tool-call transport capabilities; no provider or model receives a separate
  intent router, planner, completion rule, or product flow.
- Model output drives semantic goal interpretation, plan revision, eligible
  tool choice, and proposed completion through strict typed schemas.
- Runtime code validates capabilities, resource scope, data route, risk,
  arguments, budgets, idempotency, receipts, and mechanical completion facts.
- Ordinary public Web research and low-risk, recoverable file work inside the
  selected Project/task scope proceed without repeated prompts.
- Scope expansion, sensitive disclosure, destructive or irreversible effects,
  consequential external actions, and LifeModel updates use the appropriate
  just-in-time decision. Proposal is not the container for ordinary work.
- An explicit deliverable such as "output a Markdown file" produces a real
  Artifact. Without a bound Project, OpenLife creates a conversation-owned
  managed Artifact that can be previewed, downloaded, or saved later.
- Memory remains a simple user-visible work-continuity capability. LifeModel is
  an optional typed port for confirmed long-term understanding and never owns
  task execution, authorization, or completion.

## Essential capability scope

This plan must make the following capabilities real and dependable:

- create, resume, rename, and delete Conversations; stream responses; steer,
  cancel, continue, and retry work without losing canonical state;
- select and persist an exact provider/model route with understandable failure
  and retry behavior;
- inspect selected Project files and explicitly attached resources;
- read Markdown, text, HTML, JSON, CSV, PDF, DOCX, XLSX, and common images;
- search the public Web, fetch ordinary public pages, and present visible,
  clickable source citations;
- create and verify Markdown, HTML, CSV, JSON, and text Artifacts;
- use selected Skills and registered read-only MCP tools through the same tool
  contract and receipt path;
- retain simple, inspectable Agent Memory and optional LifeModel context through
  typed ports;
- generate DOCX, XLSX, and PPTX only after the core Artifact path is stable,
  using real render/parse verification rather than file-extension claims.

Not in this plan: Computer Use, arbitrary shell, account-connected mail or
calendar, payments, browser automation, scheduling, cloud task execution,
multi-agent orchestration, or advanced autonomous LifeModel learning. Their
future adapters must use the same lifecycle and permission contract.

## Current source facts and replacement boundary

The canonical Conversation and Task stores are the current runtime foundation.
Canonical Chat and Work now execute without `MainChatKernel`,
`main_chat_agent_v1`, or the general keyword governance router in release
builds. Those modules and their source-bound compatibility helpers have been
deleted from the repository; controlled tests now provide explicit typed plan
and Agent-step fixtures instead of reviving keyword behavior.

Other confirmed replacement targets include:

- report/plan-era APIs and schema branches inside the canonical Task store;
- the separate ScheduledTask store and proposal lane that have no complete
  release execution/read-model loop;
- the unused scoring `ModelRouter` and policy stores that do not represent the
  exact provider/model route selected by the user;
- legacy Artifact effect branches and generic JSON proposal envelopes;
- proposal-generating write tools unreachable from the canonical Work loop;
- old conversation/state ownership inside the broad Memory store;
- keyword-based Memory classification and misleading duplicate `gateway` and
  `scheduler` module names;
- legacy YAML LifeModel migration and compatibility projections;
- frontend legacy migration flows, duplicated snapshot/controller ownership,
  raw internal transport types, misleading journey names, and cross-feature
  dependency cycles.

Reusable code is retained by verified responsibility, not age or file name.
Provider validation, ToolGateway receipts, cancellation fencing,
ReviewWorkflow, Artifact materialization safety, effect-unknown handling,
canonical Memory/LifeModel ownership, and source validators survive only when
they accept the new canonical identities without preserving an old owner.

## Target source boundaries

The target is a conventional domain layout, not a new framework:

```text
openlife-core/src/
  runtime/          # Conversation/Turn/Run coordination and cancellation
  work/             # plan, step execution, completion, Task store
  provider/         # exact route, authorization, clients, receipts
  tools/            # gateway, manifests, permissions, read adapters
  review/           # workflow, store, typed review subjects
  conversation/     # store and bounded context
  memory/           # lifecycle truth, notes, retrieval projections
  lifemodel/        # model, candidates, write policy
  read_models/      # product projections only
  persistence/      # SQLite, outbox, atomic files

src-tauri/src/
  ipc/              # thin Tauri request/response adapters
  application/      # use-case orchestration
  platform/         # Keychain, filesystem, HTTP, native adapters
  bootstrap/        # composition and recovery only

frontend/src/
  app/              # routes and composition
  features/         # conversation, work, review, personal intelligence, settings
  shared/           # presentation primitives and envelopes
  ipc/              # domain clients and wire contracts
```

Directory moves happen after ownership is corrected. A large file is split
when it contains multiple reasons to change or lifecycle owners; line count by
itself is not an acceptance criterion.

## Stages

### Stage 0 — Behavioral baseline and deletion boundary

- Add failing product tests for the failures that exposed the weak harness:
  Chinese and English research requests, implicit tool needs, explicit file
  deliverables, source requirements, provider reasoning-only responses,
  task-title/result mapping, cancellation, and restart recovery.
- Record the production reachability and consumer of every proposed deletion.
- Define the typed Agent step, capability, receipt, review-subject, and
  completion contracts without building a parallel runtime.
- Reset only obsolete test execution data when needed. Preserve Keychain
  credentials, environment files, provider settings, Projects, user work
  directories, confirmed Memory, and canonical LifeModel data.

Stop condition: the new tests fail for the intended product reasons, every
delete target has a verified consumer disposition, and no implementation is
claimed complete from documentation or fixtures.

### Stage 1 — Remove non-products and compatibility owners

- Delete ScheduledTask creation, persistence, bootstrap migration, policy, and
  dead execution code; keep calendar data parsing only if a current read tool
  consumes it.
- Delete report/plan Task DTOs, writers, resolvers, read-model branches, and
  obsolete schema support after the fresh-development reset.
- Delete legacy Artifact effect state, unsupported proposal/source variants,
  unreachable write/proposal generators, old policy stores, and dead routing
  scaffolds.
- Delete legacy LifeModel YAML migration end to end and obsolete Memory
  conversation/state ownership.
- Remove matching frontend IPC, UI, fixtures, and product text in the same
  changes; add narrow release-absence tests for retired owners.

Stop condition: release code has one Conversation owner, one Work Task owner,
one Artifact effect owner, and no production fallback to a retired store.

### Stage 2 — One model-driven Agent loop

- Introduce one typed `AgentStep` loop inside the canonical Chat/Work runtime:
  model decision -> schema validation -> policy/capability check -> execution ->
  observation -> next model decision or proposed final result.
- Replace keyword intent routing and raw-text argument extraction with model
  structured output constrained by eligible capabilities and exact scope.
- Keep plans optional and dynamic Items; simple Chat and direct file creation
  must not pay a mandatory planning or Review tax.
- Move reusable context, source, provider, cancellation, and receipt contracts
  out of `MainChatKernel`/`main_chat_agent_v1`; then delete both production
  owners and their compatibility response types.

Stop condition: no release import references `MainChatKernel`,
`main_chat_agent_v1`, keyword intent routing, or a strategy-owned lifecycle;
the behavioral tests pass through the new loop.

### Stage 3 — Exact provider route and essential tools

- Replace scoring/automatic `ModelRouter` behavior with the user's exact active
  provider/model profile and explicit capability metadata.
- Split provider request types, authorization, OpenAI-compatible transport,
  Ollama transport, retry, streaming, and receipts into clear modules.
- Register real document, Project file, Web Search, Web Fetch, Skill, and
  read-only MCP adapters under one ToolGateway contract.
- Make public research autonomous within configured policy. Show search/tool
  activity and citations; confirm only scope expansion or sensitive/consequent
  actions, not each request.

Stop condition: mixed natural-language research tasks select and use required
tools without keyword gates; every claimed source has a receipt and every
provider/tool failure has a resumable product state.

### Stage 4 — Results, Artifacts, Review, and Office output

- Make Artifact identity independent of Proposal and replace generic proposal
  JSON with typed Review subjects for Artifact, Memory, LifeModel, and
  Permission only.
- Directly create recoverable files inside authorized Project/task scope.
  Unbound work creates managed preview/download Artifacts. Use Review only for
  actual boundary or consequence, and resume the same Run afterward.
- Verify content digest, target precondition, materialization receipt, and
  requested format before FinalResult can complete.
- After core formats pass, add DOCX/XLSX/PPTX adapters with native parsing and
  render-based verification proportional to the format.

Stop condition: one request for a Markdown document reliably returns a real
file, preview, path, sources, and verification; approval is absent for ordinary
in-scope creation and present at the required consequential boundaries.

### Stage 4 closure

- Artifact identity is owned by the canonical Task store rather than Proposal.
  A typed Artifact Review subject persists only exact identity, version,
  target, digest, and target-state precondition; generated content remains in
  the canonical Artifact draft.
- New files inside a bound Project are materialized directly. Unbound Work uses
  a conversation-owned managed Artifact root. Explicit review requests and
  overwrite of an existing target pause at Review; ordinary creation does not.
- Direct materialization has durable prepared/staged/confirmed/effect-unknown
  state and startup reconciliation. FinalResult requires a confirmed digest
  and Verification Item.
- Results show verified preview and path, and offer backend-verified Open and
  Save As actions. Save As uses an explicit native destination choice and
  verifies the copied digest.
- Markdown/text/HTML/JSON/CSV and DOCX/XLSX/PPTX use the same canonical
  Artifact spine; structured Office outputs are parsed and content-verified in
  controlled tests.
- Controlled closure evidence: core 680 passed/3 ignored; Tauri 354 passed/2
  ignored; frontend 232 passed; Rust Clippy, diff check, frontend formatting,
  typecheck, production build, and release-absence guard passed. Native file
  picker and external-live provider/Web behavior remain separate evidence.

### Stage 3 closure

- Each Chat or Work Turn is bound to the exact provider/model profile selected
  by the user. Bounded retry stays on that route and never silently changes
  provider or model.
- The release ToolGateway now exposes implemented document/file reads, public
  Web Search/Fetch, selected Skills, and registered read-only MCP adapters.
  Calendar, generic MCP wrappers, pseudo system tools, and Memory/LifeModel
  coupling were removed from this execution surface.
- Ordinary public Web access proceeds under the default allow policy; explicit
  ask and deny policies still fail closed. Source-bearing Work validates
  runtime-issued citations and performs at most one same-route repair attempt.
- Controlled evidence at closure: core 680 passed/3 ignored; Tauri 348
  passed/2 ignored; frontend 229 passed; Rust Clippy, diff check, frontend
  formatting, typecheck, production build, and release-absence guard passed.
  This is source/controlled evidence, not native or external-live credit.

### Stage 5 — Simple Memory and optional LifeModel ports

- Replace keyword/haystack Memory classification with a typed model-proposed
  candidate checked by deterministic sensitivity, scope, deduplication, and
  user-control rules.
- Keep Conversation context, long-term Memory truth, notes, and rebuildable
  retrieval projections under distinct owners and clear names.
- Keep ordinary Memory lightweight: use/ignore for this conversation, remember,
  forget, inspect, and delete. Do not build a version-management console.
- Keep LifeModel reads bounded and optional. Candidate generation may be
  assisted by the model, but only a reviewed typed diff can update canonical
  LifeModel state.

Stop condition: an empty or unavailable Memory/LifeModel never disables the
Agent; later LifeModel schema changes do not require rewriting the Agent loop.

Closure:

- Chat and Work ask the selected model for one strict `AgentStep`; remember,
  forget, and LifeModel suggestions are typed actions whose source span, kind,
  scope, and section are revalidated against the authenticated user Item.
  Test-only keyword shortcuts were deleted, so controlled tests exercise the
  same model-driven route as product builds.
- Low-risk explicit Memory is committed directly and reversibly; sensitive
  Memory creates Review without changing canonical truth; forgetting targets
  one exact/unique Memory; a LifeModel suggestion creates only a bounded
  candidate and cannot create a canonical version.
- Conversation lifecycle is owned by `ConversationStore`, long-term Memory
  truth by `MemoryLifecycleStore`, Knowledge Notes and lifecycle projections by
  `KnowledgeNoteProjectionStore`, and semantic retrieval by the rebuildable
  `VectorStore`. The historical on-disk/outbox `MemoryStore` identity remains
  only as a recovery protocol name.
- Recall scope comes from conversation Memory mode and canonical Project
  binding, not natural-language keyword narrowing. Empty or simultaneously
  unavailable Memory/LifeModel ports return bounded no-inference context and do
  not create Task state, grant permission, or block the selected model.
- Controlled closure evidence: 675 core tests passed with 3 ignored; 355 Tauri
  library tests passed with 2 ignored; strict Clippy passed. Ignored cases retain
  their explicit real-Keychain/external-live boundaries.

### Stage 6 — Frontend convergence

- Establish `app`, domain `features`, `shared`, and domain `ipc` boundaries;
  split the monolithic Tauri client without changing wire behavior first.
- Make the live Conversation controller the only frontend Conversation owner;
  remove duplicated Workbench lanes, task command state, Review queues, and
  provider-boundary fetch ownership.
- Replace `governedAction` and `durableTruth` product concepts with Conversation,
  Work, Review, Memory, and LifeModel features; remove cross-feature imports and
  wildcard barrels.
- Present user goals, progress, sources, results, retry/continue, and plain
  language failures by default. Keep ids, digests, receipts, and internal error
  codes inside an optional evidence inspector.

Stop condition: one current task cannot appear with contradictory statuses,
ordinary users do not see internal architecture terms, and no frontend import
or IPC type refers to a retired backend owner.

Closure:

- The production frontend now has explicit `app`, domain `features`, `shared`,
  and domain `ipc` boundaries. The former `ui/journeys` tree, wildcard feature
  barrels, legacy LifeModel migration UI, duplicate provider-boundary owner,
  and retired presentation helpers were removed.
- The live Conversation controller is the sole frontend Conversation owner.
  The Workbench aggregate carries one Task, Review, and provider-boundary lane;
  Work cancellation/undo and Review decisions each use one command controller.
- Work, Review, Settings, Memory, and LifeModel default views now present goals,
  progress, sources, results, and recovery in product language. Exact ids,
  receipts, digests, and raw internal status remain confined to optional
  technical details or typed internal contracts.
- The release-absence guard now validates the shipped app/features/ipc layout
  and scans every domain IPC client for retired commands instead of requiring
  the deleted Journey names.
- Controlled closure evidence: frontend typecheck and formatting passed; 227
  unit/component tests passed; the production build and release-absence guard
  passed; 11 browser-shell E2E tests passed. Rust formatting and strict Clippy
  passed; 675 core tests passed with 3 ignored and 355 Tauri library tests
  passed with 2 ignored. This is controlled evidence, not native or
  external-live credit.

### Stage 7 — Product matrix, native/live proof, and final deletion

- Run the complete controlled behavior matrix and ordinary Rust/frontend gates.
- Build one exact native app against isolated data and exercise real first-use,
  Conversation, Chat, Work, Project file, Web research, Artifact, Review,
  retry, cancellation, restart, Memory, and LifeModel paths.
- Run required external-live provider/Web cases with explicit credentials and
  network authorization; never substitute scripted or browser-shell evidence.
- Inspect the release import/handler/bootstrap graph and delete remaining
  adapters, aliases, feature leaks, stale documentation, test data, and old
  files. Re-run all gates after the last deletion.

Current progress:

- The synthetic prompt that named `web.search`, a stage-specific filename, and
  an explicit Review instruction is retained only as a narrow smoke test. It is
  not accepted as evidence that ordinary user language is understood.
- An exact signed QA build completed the natural request “查阅 OpenAI 官网，整理
  一份中文 Markdown 文档，比较 ChatGPT Work 和 Codex” without tool names,
  internal stages, a prescribed filename, or an approval instruction. The one
  Work Task executed Web Search, two observation-bound Web Fetches, draft,
  verification, direct in-scope materialization, and FinalResult. The resulting
  88-line `chatgpt-work-vs-codex.md` preserved the requested comparison and a
  separate uncertainty/source-limitations section. Canonical Artifact,
  ArtifactVersion, observed materialization, and filesystem SHA-256 all equal
  `175bdf6dbff7a5e32d86fdccb548984e0b0250dedfb62208301300539b825394`.
- Source attribution is protocol data rather than model-authored citation
  syntax. Web-only Markdown and text results use ordinary `content` plus exact
  HTTPS links observed in the current Run; the runtime rejects unobserved URLs
  and the independent semantic verifier checks source coverage. Selected-file
  and mixed file/Web work use typed `sourceBlocks` and backend-owned file
  markers because local documents do not have public URLs. Visible content
  cannot contain internal `webref_` or `cite_` values or model-authored
  citation markers. There are no model-calculated offsets or repeated-text
  anchors. The former second same-model
  grounding/goal-coverage call was deleted: it was not independent evidence,
  added latency and cost, and could not prove semantic entailment. Unsupported
  or incompletely bound source claims fail before display or materialization;
  goal, Artifact identity, source authority and receipts remain runtime-owned.
- Source binding is necessary but no longer sufficient for completion. Every
  source-backed FinalAnswer or Artifact path, including direct generation and
  terminal recovery, now enters one independent semantic-verification phase.
  A rejected candidate may be revised once against the exact reported gaps;
  repeating an unsupported result blocks the Task instead of completing it.
  A visible limitation counts only when the corresponding completion
  requirement explicitly permits transparent limitation, and the read model
  presents that disposition separately from fully supported completion.
- Web Fetch now extracts the server-rendered article/main body before applying
  its bounded text limit instead of truncating the page from head scripts and
  navigation. Office semantic verification uses text re-extracted from the
  generated DOCX/XLSX/PPTX bytes rather than base64 or a generic success label.
  The frontend separately labels semantic completion and post-materialization
  file-integrity verification; it no longer calls a matching digest “content
  verified.”
- This proves the native/external-live research-to-Markdown path for that exact
  build, not absence of hallucination. A preceding exact run correctly blocked
  when the first goal-coverage response was not valid Artifact JSON; the current
  retry contract then completed without bypassing the audit. Citations appeared
  as direct links and semantic grounding remains model-assisted. The latest
  source disclosed that only one official page was fetched and did not invent
  unsupported pricing, availability, or security claims. Independent manual
  retrieval of that page was unavailable because the site returned HTTP 403,
  so this run is not upgraded to a general factual-accuracy guarantee.
  A controlled conflicting-source row now proves that two materially different
  fetched pages remain distinct observations and citation authorities, and the
  grounding contract requires the disagreement and source-specific qualifiers
  to remain visible. Broader multi-source, unsupported-claim,
  adversarial-citation, and cross-provider behavior is covered at the
  controlled contract level; it is not implied by this one exact native/live
  run and remains future dogfooding coverage rather than a reason to weaken or
  special-case the runtime.
- Provider observations, Web excerpts, Resource text, tool output, and recalled
  personal context are now serialized as JSON `untrustedText` inside an
  explicit untrusted-data envelope. Only runtime-owned bounded instructions and
  output contracts remain trusted instruction blocks. A source can therefore
  contain instruction-like prose or forged delimiter text without minting a
  peer runtime instruction, permission, tool, citation, or completion claim.
  This is a protocol boundary plus a controlled adversarial test, not a claim
  that prompt injection has been generally solved.
- Resource citation authority is now scoped to the canonical Chat Turn or Work
  Run rather than to an individual provider request. Drafting, grounding, and
  repair calls within one Run therefore share the same source IDs, while a
  different Turn or Run cannot reuse them. The behavior matrix caught the old
  per-request drift once untrusted context was isolated; the fix is covered by
  exact document, Web, and mixed document/Web Artifact tests.
- Exact QA exposed an orchestration design defect rather than merely a small
  budget: the runtime pre-generated tool arguments in a batch and then paid a
  second same-model grounding pass. Raising `max_provider_attempts` from 8 to
  24 admitted that inefficient pipeline but did not fix it. The batch argument
  phase and second grounding pass are now deleted. Each tool decision is made
  just in time and receives prior successful or failed tool observations as
  bounded untrusted data before the next call. The first Work model decision
  can now return a complete answer, a source-independent Artifact, or an
  explicit personal-intelligence action directly; it returns a persisted plan
  only when tools, unobserved evidence, or dependent steps are needed. A
  controlled test proves that simple Work completes with no plan record or
  `work_plan_generation` Item. The emergency ceiling is 12 provider attempts
  and 48 total Items, but it is not an execution target: the current real
  document + Web + reviewed Artifact trial used six provider attempts for
  three tool Items. A typed budget failure remains distinct from provider
  failure. The special full-plan replan path has now been deleted. When a
  source-binding candidate is rejected after successful reads, the same Run
  retains its completed observations and asks for one bounded terminal
  `AgentStep`; it cannot replace the plan, repeat a completed tool, expand
  scope, or reset budget. The obsolete lifecycle flag that marked a provider
  call as plan generation was removed. The model now chooses among currently
  ready tool steps rather than executing a fixed plan order. A failed optional
  tool becomes a visible terminal Item; the next decision receives that
  observation and may choose another eligible route, while required-step
  failure still blocks completion. Canonical Task completion permits terminal
  optional failures but continues to reject waiting, running, or effect-unknown
  Items.
- The preceding exact QA build no longer hit the old provider-attempt ceiling
  or mislabeled it `provider_pre_dispatch_failed`, but it blocked because the
  retired token-in-prose citation protocol was too fragile. Web and selected
  files now reach one terminal source-binding validator; the provider client
  transports request-scoped source authority but cannot run a competing
  repair loop. This fixed the observed failure in which a Web repair discarded
  the previously valid file binding. Controlled mixed-source tests pass, and a
  fresh external-live provider/Web run completed the real selected-document +
  official-Web + cited Markdown + Review + materialization path in six model
  attempts. This is external-live harness evidence, not an exact native UI
  proof; the revised source still requires a new exact native build.
- A later exact QA run exposed another protocol defect: a static plan encoded
  several Web Fetch steps and asked the model to invent each URL before the
  preceding observation existed. The planner now declares at most one fetch
  capability while the same Run chooses distinct Search or Fetch actions from
  runtime-issued observations until the user outcome is supportable. Required
  Fetch cannot be skipped by prematurely returning a terminal result. The
  provider output purpose now matches the exact expected shape for initial
  decisions, personal-intelligence actions, tool arguments, tool calls,
  Artifact-or-tool decisions, answer-or-tool decisions, terminal results, and
  independent semantic verification. This removes the former conflict where a
  dynamic research turn asked for a tool call while advertising a terminal-only
  schema. A fresh external-live selected-document + Web + cited Markdown +
  Review + materialization run passed with six provider attempts and three tool
  Items after this correction; no budget ceiling was raised.
- The release Agent path is provider-agnostic. The same canonical Chat/Work
  runtime, plan and AgentStep schemas, ToolGateway, evidence rules, Review
  checkpoints, and completion verifier serve every selected model. The
  OpenAI-compatible adapter has protocol-only transport profiles: the default
  standard profile plus bounded structured-output compatibility for known
  endpoints. A controlled contract test rejects model identifiers and Agent
  semantics in those profiles. The generic adapter no longer carries an
  OpenRouter-specific function name.
- Provider waiting is governed by a cancellable ten-minute idle watchdog rather
  than the retired 120-second whole-request cutoff. The exact native run needed
  about 173 seconds for its first planning response and then continued normally,
  proving the old cutoff produced false failures. Read-only Tool uncertainty is
  a failed observation that the Agent may recover from; provider uncertainty
  fails the Task while preserving attempt certainty. Neither is mislabeled as
  an unknown external side effect; mutating-effect uncertainty remains fenced.
- The latest exact QA bundle is signed, sealed, bound to
  `ai.openlife.desktop.qa`, and launched against the isolated profile. A natural
  request using the selected `openrouter/stealth/ox-alpha` route completed one
  Web Search, one Web Fetch, five provider generations, a managed Markdown
  Artifact, materialization, verification, and FinalResult in the same Task.
  `stage7-native-final-v3.md` was materialized with SHA-256
  `4b05c66c0c01b670a240c8bfd7cb86b5e02771b8c209d4f7a3169cc57d6cbc23`.
  The in-scope new file required no redundant Review checkpoint. Canonical
  Task, Run, Item, ItemAttempt, receipts, ArtifactVersion, and filesystem state
  all reported completion for this exact build.
- Current controlled evidence after the semantic-verification and extraction
  fixes: the complete Agent behavior matrix passed; the focused canonical Work
  suite passed 74 tests with one explicit external-live ignore; strict Clippy,
  Rust formatting, and `git diff --check` passed. Full Rust tests passed with
  621 core tests (3 ignored), 357 Tauri tests (2 ignored), 2 resource-worker
  tests, and 2 doc-tests. All 230 frontend tests, formatting, typecheck,
  production build, release-absence guard, and 11 browser-shell E2E tests
  passed. The complete controlled behavior matrix also passed. Controlled
  evidence and the exact native/external-live run remain separately labeled.

Stop condition: every product matrix row has the required evidence level, the
release graph contains only the accepted owners, the worktree is clean, and
stable documentation describes the source that actually ships.

## Acceptance matrix

At minimum, the final matrix must cover:

| Scenario | Required result |
| --- | --- |
| Direct Chat | Streamed final answer, no Task, no unnecessary tool/approval |
| Research request | Real Web attempts, visible citations, limitations, verified result |
| Source-grounded result | Authentic citations are not enough: unsupported inference or product conflation is corrected or blocks delivery |
| Adversarial source content | Source/tool text remains untrusted data; forged instructions, delimiters, permissions, citations, or completion claims cannot change runtime authority |
| Named official source | Natural-language request becomes an exact domain constraint; only matching evidence can be cited |
| Project document synthesis | Exact selected-file reads, scoped output Artifact, sources |
| Explicit Markdown deliverable | Real `.md` Artifact in one request, preview/path/digest |
| Unbound deliverable | Managed Artifact, preview/download/save-later without fake path |
| Provider reasoning-only/timeout | Bounded retry, understandable failure, resumable state |
| Tool failure/partial evidence | No false completion; agent may adapt, retry, or ask clearly |
| Steering/cancel/retry/restart | Same canonical identities where required, no duplicate effect |
| Permission boundary | In-scope low-risk work proceeds; expansion/consequence asks once |
| Memory | Explicit remember/forget/use controls and inspectable provenance |
| LifeModel | Optional influence, reviewed typed update, no authority expansion |
| DOCX/XLSX/PPTX | Real parse/render verification before format credit |

## Evidence and completion rules

- Unit and integration tests prove controlled contracts only.
- Browser-shell tests prove frontend behavior against controlled boundaries, not
  native persistence, Keychain, provider, Web, or filesystem effects.
- Native evidence is bound to the exact built source and isolated profile.
- External-live evidence requires the real provider/tool, network path, request
  receipt, and user-visible result.
- A stage is not complete because code compiles, a schema exists, a proposal was
  accepted, an app process launched, or a lower evidence level passed.
- Every stage ends with its replaced production imports and files deleted in
  the same bounded commit series. Temporary adapters cannot cross a stage
  boundary.
- If required evidence is unavailable, status remains blocked or unknown; the
  plan does not reinterpret it as success.

## Checks

Use checks proportional to each stage and the common commands in `AGENTS.md`.
Behavior-changing stages require focused failing tests first, then full Rust and
frontend gates. Native and external-live runs occur only where the acceptance
matrix requires them and remain explicitly separated from controlled evidence.

## Completed baseline

- Stage 0 established failing product tests for ordinary-language Web research
  and real Markdown delivery, then mapped every owner that would be removed.
- Stage 1 deleted the retired ScheduledTask, report/plan Task, StateStore,
  legacy LifeModel migration and Artifact effect, scoring router/policy,
  duplicate Memory conversation owner, and unreachable proposal-tool surfaces.
- Stages 2–4 replaced keyword-driven intent and fixed-plan execution with a
  model-driven, schema-validated Work loop. The runtime owns capability,
  scope, risk, receipts, evidence, cancellation, and completion; the selected
  model chooses ready tools and adapts to observations without expanding scope.
- Stage 5 moved Chat and Work onto the canonical Conversation/Turn and
  Task/Run/Item/Artifact owners. Release builds no longer import
  `MainChatKernel`, `main_chat_agent_v1`, or the general keyword governance
  router. Their source files and compatibility response paths are deleted.
- Stage 6 consolidated the frontend into app, feature, shared, and domain IPC
  boundaries. Conversation, Work, Review, Personal Intelligence, and Settings
  have one product controller each; retired routes, migration UI, duplicate
  boundary reads, internal trace payloads, and old Journey trees are absent
  from the release graph.
- Stage 7 now has a passing controlled behavior matrix, complete Rust/frontend
  gates, an exact signed isolated QA build, and one successful native
  external-live Web-to-Markdown delivery through the selected provider. This
  evidence proves the exact path exercised; it does not prove universal factual
  correctness, every provider, or every possible task.

## Next pointer

Stage 7 implementation and proportional evidence are ready for review. The
remaining closure action is to review the full dirty diff as one bounded
rebuild, commit it, and return the branch to a clean baseline. Do not add more
features or restore compatibility owners before that review.
