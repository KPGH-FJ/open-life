# OpenLife Capable Product Plan

Status: complete

Baseline: `main@045fb06` with a clean working tree before this plan was added.

## Objective

Turn the reconstructed OpenLife baseline into a trustworthy general-purpose
personal Agent for knowledge work. A user must be able to start a Conversation,
delegate a meaningful Chat or Work outcome, let the Agent use authorized files
and Web capabilities, steer or approve boundaries without losing state, and
receive an inspectable and verified result.

This plan does not extend a historical slice. It replaces incomplete or
over-complex product contracts and deletes the paths they supersede.

## Product contract

- Workbench contains Projects, Conversations, Chat, Work, progress, inline
  decisions, results and a Needs Attention filter.
- Chat and Work share one Conversation, Turn and typed Item spine. Work adds a
  durable Task and Run with an explicit completion contract.
- The selected provider and model remain bound to the work. OpenLife does not
  silently switch either one.
- A task grant authorizes ordinary low-risk, recoverable operations inside its
  explicit scopes. Scope expansion and consequential actions require a
  just-in-time decision.
- Plan is an optional Item. It is not a separate runtime or product identity.
- Task, Run, Item, Attempt, receipt and digest are backend facts, not required
  top-level user concepts.
- Agent Memory is one simple product with personal and Project scopes. LifeModel
  remains a separate, optional and user-confirmed long-term model.
- Memory and LifeModel participate through narrow ports and never own task
  execution, permission, artifacts or completion.
- UI structure and terminology follow established ChatGPT Work/Codex, Claude
  Cowork and WorkBuddy patterns: simple Conversation-first interaction,
  progressive disclosure and a results/details pane when useful.

## Starting source baseline

Confirmed from production paths when this plan started:

- `/workspace`, `/life-model` and `/settings` are the shipped routes.
- `send_message` and `start_stream_message` select canonical Chat or Work and
  enter `canonical_chat_runtime.rs` or `canonical_work_runtime.rs`.
- `CanonicalTaskRuntimeStore` owns the current Work Task, Run, Item, Attempt,
  FinalResult and Artifact metadata spine.
- production paths exist for selected local document reads, Web reads,
  provider generation, governed artifact staging/materialization, cancellation,
  retry and backend ViewModel projection.
- Workbench loads one backend aggregate for task, activity, review and provider
  boundary state.

Gaps confirmed at plan start and resolved by the completed stages below:

- current policy and kernel paths still contain route-specific and
  compatibility-shaped orchestration rather than one small adaptive loop;
- the formal tool and deliverable surface remains narrower than a capable
  general knowledge-work Agent;
- large runtime and frontend controllers concentrate too many responsibilities;
- the product still exposes a Markdown Memory editor, Memory lifecycle lanes,
  proposal-backed ordinary Memory actions and a second lifecycle store, all of
  which conflict with the accepted simple Memory contract;
- exact native and external-live evidence must be re-earned for each completed
  user journey rather than inherited from historical stages.

## Evidence levels

The following levels remain distinct:

1. source and contract inspection;
2. focused unit/integration tests;
3. controlled product tests and browser-shell tests;
4. exact native application verification;
5. external-live provider, Web or connector verification when required.

A lower level never proves a higher one. A stage is complete only when its
user-visible path, canonical lifecycle, failure states, cleanup and required
evidence agree.

## Stage 0 — Current truth and product-contract cleanup

User result: the existing product no longer exposes or depends on superseded
concepts that distort later capability work.

In scope:

- keep this plan as the only active implementation plan and fix current
  authority drift when source and stable docs disagree;
- remove the Workbench Markdown Memory editor, its root-selection product flow,
  release IPC and product-only backend module;
- simplify Personal Intelligence Memory to content, scope, source and direct
  user control; remove lifecycle, lane, tier, materialization and linkage
  concepts from the product contract;
- converge canonical Memory ownership so FTS/vector state is a rebuildable
  projection rather than a second fact owner;
- remove ordinary Memory correction/archive/stop-recall Proposal flows and
  replace them with direct, reversible Memory commands;
- delete affected wrappers, types, fixtures, tests, configuration and stable
  documentation in the same changes;
- preserve the narrow Agent Memory context port so Agent capability work does
  not depend on Memory internals.

Out of scope:

- background Memory extraction and cross-product Memory import;
- LifeModel redesign;
- broad visual redesign;
- new connectors or Computer Use.

Acceptance:

- release has no Markdown Memory editor/root IPC or duplicate Memory fact owner;
- Workbench has no Memory management panel;
- Personal Intelligence uses ordinary user language and direct controls;
- Memory failure cannot make healthy Chat or Work unavailable;
- repository absence guards cover the retired product contracts;
- focused backend/frontend tests and the proportional common checks pass.

## Stage 1 — Reliable research and artifact work

User result: a user can delegate a document/Web research outcome and receive a
verified local result in one Work.

- make the model-driven loop own goal interpretation, optional planning, tool
  selection, observation and replanning inside one Run;
- support bounded local file/document reads, Web search/fetch, multi-source
  synthesis and Markdown/text/HTML/JSON/CSV artifacts;
- complete read -> reason -> create/modify -> verify in one Task;
- keep TaskContract, provider/model, scopes, budgets and definition of done
  explicit and immutable for each Run;
- delete route-specific or report-only execution assumptions as their consumers
  move to the general loop.

Acceptance includes success, no-evidence, conflicting-source, provider failure,
tool failure, safe-path failure, cancellation and verified artifact outcomes in
controlled tests and the exact native application. Web/provider external-live
evidence is required where the stage claims it.

## Stage 2 — Steering, decisions and recovery

User result: long work remains controllable without becoming a new task whenever
the user steers it or approves a boundary.

- steering updates the active TaskContract/PlanRevision through canonical Items;
- inline approval pauses and resumes the same Run and exact Item attempt;
- retry after failure or cancellation creates a new Run with preserved evidence;
- interruption, restart and effect-unknown recovery reach honest terminal or
  waiting states;
- concurrency uses one parent budget and bounded child work rather than
  unbounded parallel autonomy.

## Stage 3 — Workbench results and progressive disclosure

User result: the desktop product is understandable without exposing backend
implementation language.

- align the Workbench with established Agent layouts: Projects/Conversations on
  the left, Conversation and composer centrally, optional Results/Artifacts/
  Changes/Sources/Verification details on the right;
- keep Chat/Work, provider/model, attachments and Skills/Tools close to the
  composer without creating a setup form before every task;
- show plans and important activity inline, while keeping Attempt/receipt details
  behind an inspector;
- synchronize backend ViewModels and frontend cleanup in every change;
- remove migration-era diagnostics and unavailable buttons from ordinary flows.

## Stage 4 — Broader document and tool capability

User result: the same harness can produce and verify common knowledge-work
deliverables without a second runtime.

- add format-specific adapters only with render/parse/verification contracts;
- prioritize PDF, DOCX, XLSX and PPTX based on reusable implementation cost and
  product value;
- expose Tools, Skills and Connectors using established product terminology;
- keep MCP and manifest details in advanced/developer surfaces;
- add email/calendar or other connectors only through the same capability,
  identity, scope, approval and receipt contracts.

Computer Use, arbitrary shell and high-impact autonomous external actions remain
out of scope until separately accepted.

## Stage 5 — Minimal Agent Memory and optional LifeModel context

User result: OpenLife can continue useful preferences and Project context without
turning personalization into a second operating system.

- implement one global Memory setting and per-Conversation use-and-learn mode;
- support explicit remember/forget with undo and simple personal/Project scope;
- run at most one eligible background extraction after a Conversation becomes
  idle, using only its already-authorized provider/model route;
- deduplicate and replace clearer user facts without exposing versions;
- record only the Memory ids, scope, digest and selection reason used by a Run;
- keep LifeModel proposals explicit and separate; Memory never silently upgrades
  into LifeModel truth.

## Stage 6 — Native/live closure and clean release baseline

User result: the product claims match the exact distributable application.

- run the full proportional engineering gate bundle;
- verify first run, restart, credential continuity, Project isolation, Chat,
  Work, tools, approvals, recovery, results and Memory in an exact native build;
- run required external-live provider/Web/connector cases with isolated profiles;
- delete remaining replaced paths, stale docs, dev-only leakage and generated
  residue from release contracts;
- leave `main` clean, reproducible and ready for the next capability objective.

## Current pointer

Stage 0 through Stage 6 are complete. The implementation and evidence are ready
for review and commit; no later capability objective is active in this working
tree.

Stage 0 removed the Markdown Memory product path end to end, reduced the Memory
ViewModel and UI to ordinary content, scope, source and direct controls, made the
lifecycle store the single durable Agent Memory body owner, and kept semantic
index state rebuildable. Correction, archive and restore now commit directly and
reversibly without an ordinary Review proposal.

The full Rust and frontend gates passed. The exact signed QA bundle was verified
against its bundle identity and Designated Requirement, then exercised in the
native app. The current bundle contains no Markdown Memory editor; a missing
optional Agent Memory read model stays isolated from LifeModel and Workbench.
A provider-bound Chat failure remained a provider execution failure rather than
being mislabeled as Memory unavailability, and carries no Stage 1 success
credit.

Stage 1 now owns the next result: one reliable document/Web Work that completes
the read, reason, artifact, materialize and verify loop through the canonical
Task/Run/Item/Attempt spine.

The current Stage 1 implementation generalizes the governed Artifact contract
to Markdown, plain text, self-contained HTML, JSON and CSV; all five use the
same draft, Review, materialization and verification owner. A natural
"search, then open the result" request now requires both Web Search and Web
Fetch, with Fetch bound to a validated Search Observation from the same Run.
Controlled canonical Work tests are green for the existing behavior matrix,
the five-format Artifact contract and HTML/JSON end-to-end materialization.

Stage 1 closed the reliable research and Artifact path. Explicit URLs now become
required `web_fetch` plan steps instead of optional allowlist entries, and a
search-followed-by-open request binds Fetch to a validated Search Observation
from the same Run. A policy-approved HTTPS host can use the macOS fake-IP system
proxy without widening access to unapproved hosts. Structured DeepSeek requests
use JSON mode, bounded low-temperature generation and an output budget suitable
for reports; a missing final content response receives one same-provider,
same-model retry rather than silently switching routes or looping.

The full controlled behavior matrix and engineering gates passed. An isolated
external-live test completed the real DeepSeek Web search, selected DeepSeek
provider generation, local document evidence, Review, materialization and
verification path. The exact signed QA application then completed a separate
native Work against `https://example.com`: canonical Items recorded successful
`web.fetch` call and Observation, the first provider attempt truthfully failed
without final content, the one bounded retry produced the reviewed Markdown
Artifact, and the refreshed Task recorded materialization, verification and a
FinalResult. The written file digest and canonical Artifact projection matched.

Stage 2 closed steering, inline decision continuation, retry and recovery on the
same canonical Task/Run/Item spine. Native steering was consumed by the active
Run and advanced its PlanRevision without creating another Task. Inline Review
continued the exact Run through Artifact materialization and verification.
Retry after failure created a new Run while preserving prior evidence;
cancellation reached a normal cancelled product state; process restart exposed
an interrupted, retryable Run instead of guessing completion. Controlled tests
also cover effect-unknown and shared concurrency admission.

The full Rust and frontend gates passed after fixing a cancellation race, the
frontend cancelled-terminal projection and stream chunk whitespace handling.
The latest exact signed QA bundle passed its identity, Designated Requirement
and resource-seal checks. Native Chat preserved natural multi-word output, and
native Work covered steering, cancellation, restart, retry and reviewed file
delivery. Stage 3 now owns the Workbench result layout, progressive disclosure
and removal of backend terminology and migration-era controls from ordinary
use.

Stage 3 closed the Workbench product surface around established Agent patterns.
Projects and Conversations now form a compact left rail, Chat or Work and the
composer remain central, and a Results pane appears only when the selected
Conversation owns Work. A single Work no longer pays for search, filtering or
count controls; Project creation, files, Skills and Tools use progressive
disclosure. The selected provider/model remains visible beside both Chat and
Work. Inline decisions and result verification remain on the same surface,
while receipts, digests, sources and backend status details are available
through ordinary `Details` affordances instead of product-level engineering
terms.

The full frontend format, type, unit/component, production-build and browser
shell gates passed with 231 tests and 11 E2E scenarios. The exact signed QA
bundle was rebuilt from the final source, passed identity, Designated
Requirement and resource-seal checks, and was inspected in the native macOS
application. Native evidence confirmed the compact Project/Conversation rail,
the Chat/Work composer, persistent selected model, default-collapsed optional
context, conditional Results pane and simplified Details terminology. Stage 4
now owns broader document and tool capability through the same runtime.

Stage 4 now has one binary Artifact substrate rather than format-specific side
paths. PDF, DOCX, XLSX and PPTX are accepted as bounded document inputs. DOCX,
XLSX and PPTX output requests are represented as semantic model drafts; the
backend owns deterministic OOXML rendering, immediately re-parses each result,
stores the exact bytes as the canonical draft, and carries those bytes through
the existing Review, materialization, digest verification and recovery path.
Results and pre-approval drafts use the same bounded parsers to provide readable
previews. A gated LibreOffice check additionally loaded and converted all three
generated Office formats successfully. PDF output remains deliberately
unclaimed until a self-contained, Unicode-capable font embedding contract can
meet the same verification standard; PDF reading remains supported.

The final Stage 4 source passed the full Rust and frontend gates: 918 core
tests, 8 scheduler tests, 351 Tauri tests, 2 resource-worker tests, 7 doc tests,
231 frontend tests and 11 browser-shell scenarios. The exact signed QA bundle
then completed a real DeepSeek-backed Word Work. The native product rendered a
readable pre-approval DOCX preview, resumed the same Work through inline and
system confirmation, materialized the exact reviewed bytes, and projected the
Task, final result and Artifact as completed, materialized and content-verified.
The resulting `stage4-native.docx` passed ZIP integrity and a real headless
LibreOffice load and PDF conversion. Stage 5 now owns the deliberately small
Agent Memory product contract; it must reuse the existing runtime and must not
reintroduce the retired lifecycle or proposal-heavy UI.

Stage 5 closed the deliberately small Agent Memory product contract. Settings
owns one global switch, and each Conversation owns `Use and learn`, `Use only`,
or `Off`. Explicit low-risk remember and forget requests complete directly and
reversibly in Chat or Work without a provider call, fake Task, or ordinary
Review proposal. New Memory admissions expose only Personal and Project scope;
retired Conversation or Workspace scope wording is rejected rather than
silently widened.

After the latest completed Turn becomes idle, at most one eligible stable,
internal candidate may be checked through that Turn's exact authorized provider
and model. It can only create a deduplicated Review proposal; it cannot write
Memory or LifeModel. Work records only selected Memory id, product scope,
content digest and selection reason. A regression test also fixed duplicate
vector/text hits so one canonical Memory owner cannot create conflicting Run
receipts because of stale projection session metadata.

The Stage 5 controlled gates passed: 921 core tests with 4 explicit ignores, 8
scheduler tests, 362 Tauri tests with 2 explicit external/OS ignores, 2 resource
worker tests, 7 documentation tests, 234 frontend tests, the production absence
guard, and 11 browser-shell scenarios. Exact native Memory behavior and any
required external-live extraction evidence remain Stage 6 work; they are not
claimed by these controlled results.

Stage 6 closed the native/live product baseline. Remaining release-compiled
compatibility wrappers in the Memory gateway, duplicate governed-tool candidate
fields, an unused workspace-file wrapper, dead proposal helpers and a stale
JSON-RPC field were removed instead of hidden behind dead-code allowances. Two
concurrency/canonical-corruption checks that existed without test registration
now run as real tests. Stable documentation names the current Chat/Work owners
and current planning state rather than retired runtime or planning contracts.

The final Project Memory regression had two causes: the direct reversible port
incorrectly admitted only semantic facts even when typed policy proof had
authorized a low-risk preference, and forget compared a canonical Project
owner reference with the raw Project id. Both restrictions were corrected. A
new integration test proves that a Project preference can be remembered and
forgotten without a provider invocation, Agent Run or canonical Task. The exact
native application then repeated that same journey: the remember receipt was
committed, the forget receipt archived its retrieval state, the retrievable
record count became zero, and the Conversation still owned zero Tasks, task
sessions and Agent Runs.

The resulting source passed `git diff --check`, Rust formatting and clippy with
warnings denied, the full Rust workspace tests, 235 frontend tests, the
production absence guard and 11 browser-shell scenarios. A separate forced
dead-code compile reported no OpenLife-owned dead-code warnings. The exact QA
bundle was rebuilt as `ai.openlife.desktop.qa`; its resource seal and
Designated Requirement passed, and a full restart reopened the existing
Project without another Keychain recovery prompt.

The same final exact application used DeepSeek `deepseek-v4-flash` and a real
`web.fetch` of `https://example.com`, produced
`stage6-final-source-live.md`, paused at the inline and native governed-write
confirmations, resumed the same Work, materialized the file and projected
Verification plus FinalResult. The canonical Task completed with one ToolCall,
Observation, ArtifactDraft, ReviewCheckpoint, ArtifactMaterialized,
Verification and FinalResult. The reviewed content, observed filesystem
content and disk SHA-256 all matched
`d30433cca78b531b1112328a9560dd205a3941a5b614031a48fae2d240af5f53`.

The capable-product objective is therefore complete at all required evidence
levels. The current working tree remains intentionally uncommitted for user
review; Stage 6 completion is not a claim that Git publication has occurred.

## Common checks

Run checks proportional to each change, culminating in:

```sh
git diff --check
cargo fmt --check
cargo clippy --all --locked -- -D warnings
cargo test --all --locked
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend test
corepack pnpm --dir frontend build
corepack pnpm --dir frontend test:e2e
```

## Stop condition

Pause only for a real external credential/identity decision, destructive target
that cannot be resolved from the accepted clean-break policy, or a product
choice outside this contract. Test failures, difficult refactors and incomplete
evidence are work to resolve, not reasons to declare a stage complete.
