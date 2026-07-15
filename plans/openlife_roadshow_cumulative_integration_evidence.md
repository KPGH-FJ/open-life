# OpenLife Roadshow Cumulative Integration Evidence

> Scope: cumulative roadshow capability integration only. This record is
> subordinate to `AGENTS.md`, the Phase7 authority stack, and
> `openlife_roadshow_core_capability_execution.md`.

## Current verdict

- Cumulative Integration is **in progress**.
- RC-01 has passed a local captured-HTTP streaming, Provider-failure, and
  same-operation terminal-recovery mechanical run, plus two consecutive
  external live Provider runs for the exact frozen prompt.
- RC-02 and RC-03 have passed frozen multi-file production-extractor,
  deterministic-selection, captured-HTTP Provider, and backend citation-
  validation mechanical runs, plus two consecutive external live Provider runs
  for both exact frozen prompts.
- RC-04 has passed a single-command mechanical integration run.
- RC-05 has passed a three-process create, complete, replay, undo, and final
  audit mechanical run against file-backed canonical stores.
- RC-06 has passed a three-process wait-for-review, accept, replay, and final
  audit run. RC-07 has passed its exact two-artifact journey and artifact crash
  reconciliation matrix. Both exact frozen prompts have also passed two
  consecutive external live Provider runs.
- RC-08 has passed both a local cancellation/new-operation-retry run and a
  three-process cancel, reopen, explicit retry, and audit mechanical run.
- CC-01 has passed a local Resource + Web + reviewed Markdown artifact
  mechanical run, a forged-Web-citation counterfactual, and two consecutive
  external live Resource + Web + Provider + ReviewWorkflow runs.
- CC-02 has passed a local Resource-to-atomic-transient-task mechanical run
  and untrusted-attachment counterfactual.
- CC-03 has passed a canonical explicit-Memory commit, rollback, and
  same-identity persistent-store reopen/recovery mechanical run, plus quoted-
  source and pre-existing-owner counterfactuals.
- RC-01, RC-02, RC-03, RC-04, RC-06, RC-07, and CC-01 have now each received two
  consecutive external live runs for their exact frozen prompts; RC-04 and
  CC-01 include real Web evidence. The remaining applicable journeys have not
  received their required external live credit, and no journey has received
  native desktop, repeated product-trial, or independent-review credit.
- RC-05, RC-06, RC-08, and CC-03 now have separate backend OS-process reopen
  proofs. Packaged Tauri bootstrap, window relaunch, native UI, and CC-03
  production-keychain evidence remain pending.
- The roadshow candidate remains **NO-GO**.

## RC-01 exact scenario

Frozen prompt:

> 把下面这段产品介绍改写成适合路演开场的三段话，然后给出一个五步执行计划：OpenLife 是一个由私人 LifeModel 引导的本地优先个人 Agent。

This request asks for writing and plan decomposition as answer content. It does
not authorize a tracked PlanExecute session, a Memory fact, a Proposal, or any
other durable product effect. The exact command test sends the frozen prompt
through the streaming Main Chat entrypoint and captures the real local HTTP
OpenAI-compatible request.

Observed positive facts:

- Policy selects `direct_answer` and grants only `provider_generation`;
- the Provider request crosses the HTTP adapter once with `stream=true` after
  PrivacyPolicy filtering;
- three distinct SSE content chunks reach the transport incrementally;
- the reconstructed answer contains three paragraphs and exactly five plan
  steps, and `stream-message-done` is last and emitted once;
- no Tool, Proposal, tracked PlanExecute session, or durable write is created;
- the same UUIDv4 operation recovered through the buffered command returns the
  canonical final without a second Provider dispatch.

Failure and counterfactual evidence:

- an observed local HTTP 503 produces a non-completed terminal with a Provider
  blocker; it does not return the hard-coded PlanExecute success text;
- an explicit request to track a supplied-text plan still routes to
  PlanExecute, so the repair does not remove tracked planning capability;
- a transformation verb does not suppress a separate real user-preference
  candidate, while the supplied source text itself grants no Memory authority;
- the frozen 40-case set remains encoded and was not edited; its separate
  10-case legacy router evaluator remains RED at 9/10 and is not counted as
  RC-01 completion evidence.

Root failures found and removed:

1. Memory candidate extraction interpreted preference-shaped source text inside
   a rewrite request as user Memory authority. Intent and Memory routing now
   share one bounded `transformation verb + supplied text` predicate.
2. Plan routing treated any `计划` token as authorization to create the fixed
   weekly PlanExecute draft, replacing the requested subject and answer shape.
   Supplied-text transformation output now remains side-effect-free unless the
   current user explicitly asks to track, save, resume, or execute that plan.
3. The first RC-01 harness used a JSON completion fixture for an SSE request,
   creating a truthful `remote_unknown` instead of test credit. The acceptance
   harness now captures a real three-chunk SSE response and separately injects
   an observed 503 failure.

This is local HTTP adapter and command-surface evidence. It is not external
cloud-provider, native desktop UI, full application-process restart, product-
trial, or independent-review credit. Same-operation recovery occurs in one
application process and is classified only as conversation/terminal reload
evidence.

RC-01 implementation commit:
`664f96ff1f2cb4e4863a4af10aedfdecc83a17b9`.

Mechanical evidence after the repair:

- exact RC-01 positive and Provider-failure tests — 2 passed;
- supplied-text DirectAnswer and explicit tracked-plan counterfactuals — 2
  passed;
- Memory candidate suite — 19 passed;
- first 40 case encoding guard — passed; the router-contract evaluator remains
  9/10 RED and is classified as existing legacy eval drift;
- full Main Chat command surface — 87 passed;
- Main Chat runtime module — 30 passed;
- single-system authority guards — 32 passed;
- `cargo check -p openlife-tauri --tests` — passed with existing dead-code
  warnings only;
- `cargo fmt --all -- --check`, `git diff --check`, and staged diff check —
  passed.

External live RC-01 evidence commit:
`157f466e9f56fe52a8408c423410abfdf823cbea`.

The initial external live attempts exposed unstable output-shape adherence.
The model produced useful content, but the first validator also confused
opening-list numbering with plan numbering and then accumulated format cases.
After three RED attempts, implementation stopped adding parser exceptions and
re-examined the architecture assumption. The root repair derives a bounded
response-shape contract only from the current Policy-bound user message,
appends it inside the existing PreparedProviderRequest authorization envelope,
and explicitly grants no Tool, write, Memory, or policy authority. Unsupported
counts fail closed to ordinary generation instead of being misparsed.

Two consecutive runs against the external Provider on the final code observed:

- exactly three opening prose paragraphs and top-level plan sequence
  `[1, 2, 3, 4, 5]`;
- 428 and 326 Provider-bound incremental chunks respectively;
- one `provider.started`, one `provider.completed`, and one
  `final_delivery.created` fact per operation;
- `stream-message-done` once and last;
- zero Tool, Proposal, tracked PlanExecute, review, or durable effect;
- same-operation buffered recovery with no new event or Provider dispatch.

The captured local request also proves that the bounded 3-paragraph/5-step
contract reached the Provider payload after PrivacyPolicy filtering. Kernel
regression passed 100/100, runtime module 30/30, command surface 93/93,
single-system guards 32/32, and `cargo check -p openlife-tauri --tests --locked`
passed. This closes only RC-01 external live Provider credit. Native product UI,
signed application, and independent-review credit remain pending.

## RC-02 and RC-03 exact attachment scenarios

Frozen prompts:

> 比较这两份文件的核心主张、分歧和风险，并给出逐条引用。

> 分析这两份表格的趋势、异常和可能的数据质量问题，并引用对应工作表和单元格范围。

The exact command tests parse the frozen PDF/DOCX and CSV/XLSX bytes with the
production bounded extractor, commit the resulting bytes/chunks/provenance to
the canonical ResourceStore under the same UUIDv4 turn identity, and send the
frozen prompt through ordinary Main Chat. One captured local HTTP Provider
request receives the deterministically selected, explicitly untrusted blocks.
The local Provider harness returns every backend-issued Resource citation; the
backend validates those ids before rendering the canonical source footer.

Observed RC-02 facts:

- Policy selects side-effect-free `direct_answer`; legacy fallback is false;
- selected context contains relevant PDF and DOCX evidence rather than forcing
  irrelevant sentinel paragraphs into the request;
- the rendered answer resolves citations to both `roadshow_compare.pdf` pages
  and `roadshow_compare.docx` paragraph ranges;
- the turn executes zero Tool, Proposal, or durable product write.

Observed RC-03 facts:

- selected context contains the anomaly row and formula-shaped data from both
  CSV and XLSX;
- the rendered answer resolves citations to CSV ranges and the
  `roadshow_metrics` XLSX sheet/ranges;
- `WEBSERVICE(...)` remains untrusted cell text and grants no Network, Tool,
  Proposal, or write authority;
- the turn executes zero Tool, Proposal, or durable product write.

Evidence boundaries and counterfactuals:

- the exact command tests use the production extractor in-process because a
  Rust libtest executable is not the shipped Tauri parser-worker entrypoint;
- a separate direct binary-protocol probe started
  `target/debug/openlife-tauri --openlife-resource-parser-worker-v1`, parsed the
  frozen PDF, exited zero, and returned two page-provenance chunks;
- the 21-test Resource suite separately proves wrong MIME/corrupt OOXML failure,
  bounded selection, replay/drift rejection, tombstone restart behavior, and
  kill/reap on parser cancellation or timeout;
- these local fixtures and the citation-echo Provider do not prove native file
  picker UX, external live-provider answer quality, packaged desktop restart,
  or product-trial usability.

No new parser, Resource store, selector, vector route, or provider-specific
upload path was introduced. This slice closes the missing cumulative vertical
journeys on the V1 architecture; it does not claim that all RC-02/RC-03 live
and lifecycle evidence is complete.

RC-02/RC-03 cumulative evidence commit:
`cf269e536425198139fcb865355cb3165e2992df`.

Mechanical evidence after the addition:

- exact RC-02 and RC-03 command tests — 2 passed;
- Resource parser/gateway/store/selection filter — 21 passed;
- production parser-worker binary protocol probe — passed;
- full Main Chat command surface — 90 passed;
- Main Chat runtime module — 30 passed;
- single-system authority guards — 32 passed;
- `cargo check -p openlife-tauri --tests` — passed with existing dead-code
  warnings only;
- `cargo fmt --all -- --check` and `git diff --check` — passed.

External live RC-02/RC-03 evidence commit:
`a33016c5b9bec0d3ac624d22b6005939107eed8f`.

The live gate reuses the production import parser, canonical ResourceStore,
operation binding, deterministic selection, ordinary streaming Main Chat
entrypoint, PrivacyPolicy-filtered Provider request, backend citation validator,
and backend-owned source footer. It does not introduce a test-only resource or
Provider route. Two consecutive runs on the final evidence code observed, for
each of RC-02 and RC-03:

- a completed external Provider-backed turn with exactly one
  `provider.started`, one `provider.completed`, and one
  `final_delivery.created` durable fact;
- both selected filenames in the backend-verified source footer;
- PDF page plus DOCX paragraph provenance for RC-02, and CSV/XLSX range plus
  worksheet provenance for RC-03;
- zero Tool, Proposal, review, or durable effect;
- citation validation before product-visible Provider token delivery, with
  `stream-message-done` last.

Reply sizes differed across the two runs (RC-02: 3509 then 2602 bytes; RC-03:
4121 then 2821 bytes), while all invariant checks remained green. This rules out
fixed-answer or same-output cache credit but does not by itself prove answer
helpfulness. The exact external gate passed twice, the two local exact attachment
tests passed, Main Chat command surface passed 93/93, and single-system guards
passed 32/32. Native file-picker UX, packaged healthy-Keychain behavior,
repeated product trial, and independent review remain pending.

## RC-04 exact scenario

Frozen prompt:

> 结合附件中的产品数据和今天公开网页中的相关信息，给出有来源的路演风险摘要。

The test binds the frozen Markdown fixture to the same UUIDv4 Main Chat
operation, executes one governed `web.search`, and captures the provider HTTP
request. The same provider request must contain both a canonical
`resource://...?...citation=cite_...` reference and a canonical
`websearch://...?...citation=webref_...` reference. The provider response must
use both issued citations; backend-owned projection then appends separate
Resource and Web source sections.

Observed product facts:

- selected strategy is `re_act_tool_execution`;
- exactly the policy-authorized read route is used;
- the Web action reaches `Completed` with a verified execution receipt;
- the answer contains one verified Resource citation and one bound-but-not-
  endorsed Web citation;
- the fixture's quoted Memory instruction creates zero proposals;
- the raw Web body marker is absent from product IPC and its receipt;
- legacy fallback remains false.

The original deterministic run uses a local Web fixture and captured local HTTP
Provider. A separate ignored live gate binds the same frozen Resource bytes to
the exact RC-04 prompt, executes a non-fixture network Web search, and invokes
the configured external Provider. After the citation contract repair below,
that full external path passed twice consecutively. Both runs contained a
completed Web action, ordered durable Provider start/completion facts, backend-
validated `cite_` and `webref_` references, zero Proposals, and one final
delivery.

This is external backend-command evidence, not native file-picker, packaged
desktop, window, or visual product-trial evidence.

## Root failures found and removed

1. The provider event schema rejected a canonical Resource context reference
   because the generic metadata validator prohibited `?` and `=`. The Resource
   selector now owns a strict canonical-reference validator. UUID version,
   ordinal representation, citation shape, uppercase citation text, appended
   filename leakage, and malformed identifiers have negative tests.
2. The exact Chinese prompt was initially classified as `direct_answer`.
   Policy intent classification now recognizes explicit synthesis from public
   Web evidence while a public-webpage design counterexample remains outside
   Web authority.
3. The Provider instruction described citation tokens but did not make the
   per-source requirement an output invariant. The external Provider therefore
   intermittently omitted the Resource citation and the backend correctly
   blocked with `resource_citation_validation_failed`. Resource and Web system
   instructions now require at least one exact supplied token whenever that
   evidence class is present. The validator was not weakened and no automatic
   retry was added.

Neither repair removes context evidence, broadens write authority, or lets a
model authorize the Web route.

## Mechanical evidence

- `cargo test -p openlife-core resource_selection::tests:: -- --nocapture` —
  3 passed.
- `cargo test -p openlife-core roadshow_external_read_policy_tests -- --nocapture`
  — 2 passed.
- exact RC-04 command-surface test — passed.
- exact RC-04 external Resource + Web + Provider gate — passed twice
  consecutively after the citation-contract repair.
- external DirectAnswer gate — passed; external streaming gate — passed with
  33 Provider-bound chunks and final-last delivery.
- historical Step6 meta-prompt Web gate — remains RED because the user message
  requests a tool JSON envelope rather than a cited final answer; it is not
  counted as RC-04 evidence and was not edited.
- `cargo test -p openlife-tauri main_chat_command_surface_tests:: -- --nocapture`
  — 74 passed.
- `cargo test -p openlife-tauri main_chat_kernel::tests:: -- --nocapture` —
  71 passed.
- provider selected-context source-reference schema test — passed.
- `cargo check -p openlife-tauri --tests` — passed with existing dead-code
  warnings only.
- `cargo fmt --all -- --check` and `git diff --check` — passed.

Implementation commit: `02fd7580a1078a57e6308921a5ed61f357e4e17d`.

External live citation-contract commit:
`3ea25fd295763a3dce5720253d73f97e3af10084`.

## RC-05 exact daily-task lifecycle

Frozen create prompt:

> 今天下午三点前提醒我完成路演设备检查，完成后我还要能撤销。

The cumulative harness uses three distinct backend OS processes and one shared
file-backed Conversation, AgentRun, TaskSession, ActionQueue, TurnEvent,
StateStore, and LifeModel root:

1. the seed process creates the task and completes it through ordinary Main
   Chat, then exits;
2. the verify process reopens every store, replays the same create and complete
   operation identities with zero event growth, performs one natural-language
   undo, replays that undo once, then exits;
3. the audit process reopens the stores again without executing a turn and
   verifies the one canonical tombstone and complete operation history.

Observed facts:

- create, complete, and undo remain deterministic
  `transient_state_command` operations with no Provider or Tool dispatch;
- one asset moves through versions 1, 2, and 3 and ends tombstoned, with zero
  active daily tasks after the final restart;
- exact replays do not add Conversation messages, durable events, or state
  mutations; the final audit observes six messages total, not duplicates;
- each operation has one minimal StateStore receipt, one `effect_committed`,
  and one `final_delivery.created` event;
- StateGateway mutations create no Tool/ActionQueue record and no Proposal;
- immutable `effect_committed` records preserve transaction-time
  `projectionStatus=pending`, while the current canonical receipt/read model
  reports `applied`; historical events are not rewritten as current truth;
- the backend task projection is empty after the tombstone and restart.

The widened gate exposed and corrected one stale test expectation that treated
the immutable event's projection status as the current read-model status. The
product implementation already stored the correct transaction-time event and
current receipt; the repair keeps those two truths separate instead of
rewriting event history.

This is backend OS-process and mock Tauri command-surface evidence. It is not a
packaged desktop relaunch, native UI task interaction, legacy YAML migration
cutover, or independent-review claim. Expiry behavior remains separately
covered by the StateStore fault/restart suite rather than this exact journey.

RC-05 cumulative evidence commit:
`6c40b30f8c0659efd7d351fb31ea6fc234617861`.

Mechanical evidence after the repair:

- three-process RC-05 exact lifecycle — passed;
- StateStore transaction/replay/concurrency/expiry filter — 19 passed;
- transient-state Tauri/runtime filter — 4 passed;
- full Main Chat command surface — 91 passed;
- Main Chat runtime module — 30 passed;
- single-system authority guards — 32 passed;
- `cargo check -p openlife-tauri --tests` — passed with existing dead-code
  warnings only;
- `cargo fmt --all -- --check` and `git diff --check` — passed.

## RC-06 and RC-07 reviewed artifacts

Frozen prompts:

> 把最终摘要保存到工作区的 roadshow-summary.md。

> 生成一份 Markdown 路演摘要和一份 CSV 风险清单，并在我确认后保存。

RC-06 now has one end-to-end three-process backend lifecycle over a shared
file-backed ProposalStore, Conversation, AgentRun, TaskSession, TurnEvent, and
safe workspace:

1. the seed process makes one captured local HTTP Provider request, persists
   one canonical-path-bound Proposal, proves the file is absent, and exits with
   the task in `WaitingPermission`;
2. the verify process reopens all stores, observes the same pending Proposal,
   accepts it once, verifies the confirmed/observed digest and exact bytes,
   and sees the task become `Completed`;
3. the audit process reopens again and reaccepts the Proposal. It receives the
   existing confirmed receipt, preserves the file modification time, leaves
   exactly one final file and no stage copy, and retains one conversation turn
   and one final delivery.

The Proposal target remains the Rust-owned canonical
`filesystem.<safe-root>/roadshow-summary.md` reference. Provider content cannot
select or alter that path. Proposal creation and `WaitingPermission` are never
reported as file completion.

RC-07 reuses the V4 implementation rather than adding a second route. Its
current exact command test proves one Provider request, two pending Proposals,
zero pre-accept files, exact Markdown/CSV bytes, two confirmed digest receipts,
one completed parent task, and idempotent reaccept with no stage copy. The
artifact restart matrix separately proves staged recovery, rename-before-
receipt observation, and retry only when no effect bytes exist.

Evidence boundary:

- RC-06 has separate backend OS-process wait/resume/replay evidence;
- RC-07 has exact journey plus component-level crash/restart evidence, not a
  single separate-process end-to-end bundle proof;
- both use a local HTTP Provider fixture and mock Tauri command surface, not an
  external cloud Provider, packaged desktop relaunch, or native Review Center
  product trial.

RC-06 cumulative evidence commit:
`bf06b0e9d5dc52f8c4fe48d2477dc26c8ba06470`.

Mechanical evidence after the addition:

- three-process RC-06 wait/resume/replay lifecycle — passed;
- exact RC-07 two-artifact lifecycle — passed;
- artifact restart reconciliation — 3 passed;
- complete Proposal command module — 66 passed;
- full Main Chat command surface — 92 passed;
- Main Chat runtime module — 30 passed;
- single-system authority guards — 32 passed;
- `cargo check -p openlife-tauri --tests` — passed with existing dead-code
  warnings only;
- `cargo fmt --all -- --check` and `git diff --check` — passed.

External live RC-06/RC-07 evidence commit:
`296c912f9969c2946c012f5c8e89958ba283ee4b`.

The ignored external gate uses the ordinary streaming Main Chat entrypoint and
configured external Provider, but otherwise keeps the same V4 owners: Rust
selects the safe paths, validates the bounded artifact envelope, stages bytes,
creates Proposal records, and materializes only through the existing Proposal
acceptance command. Two consecutive runs on the final evidence code observed:

- RC-06 produced one pending Markdown Proposal; RC-07 produced one Markdown and
  one CSV Proposal;
- each turn remained truthfully `blocked` with its canonical task in
  `WaitingPermission`, zero files before acceptance, zero Tool/effect events,
  and exactly one Provider start/completion/final lifecycle;
- unreviewed Provider bytes produced zero Provider-bound product token chunks
  and were absent from the product result, Proposal JSON, AgentRun, and durable
  TurnEvent JSON;
- each accepted artifact had matching intended and observed digests; RC-07
  remained `WaitingPermission` after its first acceptance and became
  `Completed` only after both were confirmed;
- reaccepting the first Proposal reused the original receipt and left no stage
  file or second final artifact.

The two RC-06 Markdown outputs were 1445 and 1119 bytes. The two RC-07 runs
produced Markdown/CSV sizes of 1346/628 and 1023/563 bytes. Different generated
bytes with invariant lifecycle results rule out fixed-output cache credit but do
not replace a human helpfulness review. The two local exact artifact tests,
Main Chat command surface 93/93, single-system guards 32/32, format checks, and
the live-test module ownership guard all passed. Native Review Center UX,
packaged healthy-Keychain behavior, signed release, repeated product trial, and
independent review remain pending.

## RC-08 exact scenario

Frozen prompt:

> 分析附件并检索网页；在执行中取消，然后重试一次。

The first UUIDv4 operation binds the frozen Markdown fixture, completes one
governed Web read, and then reaches a deliberately hanging local HTTP Provider
synthesis request. Cancellation is requested only after the HTTP request is
observed.

Observed first-attempt facts:

- selected strategy is `re_act_tool_execution`;
- one canonical `tool.completed` terminal precedes Provider cancellation;
- local cancellation completes in less than one second and closes the local
  HTTP connection;
- durable order contains `provider.started`, `cancel_requested`,
  `provider.remote_unknown`, and `local_aborted`;
- `remoteCancellationConfirmed` remains false;
- no `provider.completed` or `effect_committed` fact is created;
- releasing the late Provider response changes zero durable events;
- the AgentRun remains `Cancelled`.

The original harness then clears process-local Main Chat runtime facts and
performs the explicit retry as a new UUIDv4 operation. A second harness now
uses three distinct backend OS processes and one shared file-backed Main Chat
store set:

1. the seed process binds the frozen Resource, completes one governed Web read,
   observes Provider dispatch, cancels locally, records `remote_unknown`, and
   proves that releasing the late Provider response changes zero durable
   events;
2. the verify process reopens the durable stores, proves that the first
   AgentRun is still `Cancelled`, rebinds the same frozen Resource to a new
   UUIDv4 operation, and performs exactly one Web action and one Provider
   synthesis request with zero proposals;
3. the audit process reopens the stores again and proves that the first attempt
   still has one `provider.remote_unknown`, one `local_aborted`, one historical
   `tool.completed`, and no `provider.completed` or `effect_committed`, while
   the retry has one `provider.completed`, one final delivery, and a completed
   AgentRun.

The cancelled session's ActionQueue projection is intentionally hidden after
reopen; the immutable `tool.completed` TurnEvent remains the historical
authority. The retry's ActionQueue contains one `web.search`. This distinction
prevents a transient projection from being presented as canonical history.

This proves recovery after loss of all backend process-local runtime/cache
state and a user-visible new-operation retry. It does **not** prove packaged
Tauri bootstrap, native window relaunch, native UI projection, external live
Web, or external cloud Provider behavior; those evidence dimensions remain
pending.

The exact Chinese `检索网页` phrase initially failed to authorize Web read
capability. Policy intent classification now recognizes explicit search/query
Web phrases while the existing webpage-design counterexample still receives no
Web authority. The model cannot grant this capability.

RC-08 implementation/evidence commit:
`f84bed579b9e27bb0e3eb974cd66c38082a369b3`.

RC-08 separate-process evidence commit:
`1b3ec4975f6947bcd7dcb01a085c11cb44fe8867`.

Additional mechanical evidence:

- exact RC-04, RC-06, and RC-08 roadshow command tests, including the RC-08
  three-process lifecycle — passed;
- `roadshow_external_read_policy_tests` — 3 passed;
- generic released-late-provider cancellation regression — passed;
- full Main Chat command surface — 93 passed;
- Main Chat runtime module — 30 passed;
- single-system authority guards — 32 passed;
- `cargo check -p openlife-tauri --tests` — passed with existing dead-code
  warnings only.

## CC-01 exact scenario

Frozen prompt:

> 读取附件并查询公开网页，生成一份带引用的 Markdown 报告，等待我确认后保存。

The exact test binds the frozen `roadshow_combined_report.pdf` bytes and two
page-provenance chunks to the same UUIDv4 operation. It executes one governed
`web.search`, sends one bounded Resource + Web synthesis request through the
local HTTP Provider adapter, and requires the Provider's Markdown field to use
both request-scoped citation classes. The backend appends Resource and Web
source sections inside the typed artifact envelope before ReviewWorkflow
staging.

Observed positive facts:

- Policy remains `proposal_only_write` with exactly `file_write_proposal`,
  `provider_generation`, and `web_search` capabilities;
- one `web.search` ActionQueue item reaches `Completed` with a verified
  ToolGateway receipt;
- one Provider request contains the bounded PDF evidence and bounded Web
  observation with canonical `cite_...` and `webref_...` identities;
- exactly one external-write artifact proposal is pending;
- the target file is absent before acceptance;
- acceptance materializes one Markdown file whose observed digest equals the
  committed digest and whose two source footers are backend-owned;
- the raw Web body marker is absent from product IPC and the tool receipt;
- no Memory or LifeModel proposal is created.

The counterfactual returns a valid issued Resource citation and a forged Web
citation. The Web read remains visible as a completed fact, but synthesis ends
with `web_citation_validation_failed`, creates zero proposals, and writes zero
files.

Root failures found and removed:

1. The file-review Policy route dropped the already-classified Web capability.
   It now reuses the existing typed read-capability authority only when the
   current authenticated request explicitly requires external evidence.
2. Runtime read planning accepted only a pure `ReadOnly` action effect. The
   same read executor now admits a narrow compound lane only when Policy also
   authorizes `FileWriteProposal` and `ProviderGeneration`.
3. Resource citation rendering appended prose after Provider JSON. Artifact
   drafts now validate and render canonical Resource provenance inside the
   Markdown field; the typed artifact parser still rejects unknown fields,
   paths, empty content, and invalid CSV.
4. The write-result assembler hard-coded an empty tool-call list. It now
   reuses the canonical tool graph and existing tool-evidence projection before
   staging the proposal.
5. Durable Provider lifecycle validation omitted the already-defined
   `main_chat_artifact_draft` purpose. The closed list now includes that exact
   enum value; it was not widened to arbitrary strings.
6. The first external CC-01 runs exhausted the runtime worker stack after
   `tool_decision` and before `tool.started`. RC-04 proved the same real Web
   executor could dispatch on the shallower read-only route. The compound
   artifact route was polling the deep ToolGateway/network future inline while
   retaining its larger post-read continuation. Read-tool execution now crosses
   one Tokio `JoinSet` task boundary. Dropping the parent turn aborts the child,
   so the stack repair does not detach late tool execution from cancellation.

The original mechanical evidence above is canonical local storage, a fixture-
backed Web adapter, and a local HTTP Provider boundary. It does not itself earn
native desktop, external live Web, or external cloud-provider credit; the
separate gate below supplies the external evidence. The PDF parser itself was
proven in V1; CC-01 consumes the frozen PDF bytes and canonical page-provenance
representation rather than claiming a second parser trial.

CC-01 implementation commit:
`32923b1b18cd509a4acfc70739524ba2543cd90a`.

Mechanical evidence after the repair:

- exact CC-01 positive and forged-citation tests — 2 passed;
- `generated_artifact_policy_tests` — 3 passed;
- Resource and Web citation modules — 8 passed;
- full Main Chat kernel — 71 passed;
- full Main Chat command surface — 77 passed;
- single-system authority guards — 32 passed;
- Main Chat runtime module — 30 passed;
- `cargo check -p openlife-tauri --tests` — passed with existing dead-code
  warnings only;
- `cargo fmt --all -- --check`, `git diff --check`, and staged diff check —
  passed.

External live CC-01 and stack-repair commit:
`1bd986a1cac70325128087231dabd304516324e3`.

After the stack repair, two consecutive final-code runs observed:

- one production-parsed frozen PDF, one real non-fixture Web search, one
  external Provider synthesis request, and one pending external-write Proposal;
- backend validation and rendering of both request-scoped `cite_` and `webref_`
  source classes before ReviewWorkflow staging;
- truthful turn status `blocked` and canonical task status
  `WaitingPermission`, with zero files and zero durable effect before explicit
  acceptance;
- no Provider artifact token streaming and no full materialized report body in
  the product result, Proposal JSON, AgentRun, or durable TurnEvent JSON;
- one confirmed file with matching intended/observed digest after acceptance,
  followed by idempotent reaccept with the same receipt and no stage copy.

The two reports were 2187 and 1100 bytes. The generated bytes differed while
the lifecycle invariants remained identical; this rules out fixed-output cache
credit but not the pending human helpfulness review. Kernel tests passed 73/73,
runtime module 30/30, command surface 93/93, single-system guards 32/32, and
`cargo check -p openlife-tauri --tests --locked` passed. A dedicated
counterfactual drops a parent turn while a read tool is pending and proves the
isolated child future is aborted before any Provider call.

One additional RC-04 control run during diagnosis completed real Web and
external Provider dispatch but failed closed on `web_citation_validation_failed`
when the model omitted a required citation. It is retained as a stability
failure and is not counted as new RC-04 credit; the earlier two passing RC-04
runs remain separately recorded. Native picker/Review Center UX, signed package,
healthy production Keychain, repeated product trial, and independent review
remain pending.

## CC-02 exact scenario

Frozen prompt:

> 从附件提取今天的准备事项，创建短期任务；如果要写文件，先等待我确认，然后继续。

The exact test binds the frozen `roadshow_checklist.docx` bytes and four
canonical paragraph-provenance chunks to the same UUIDv4 Main Chat operation.
The Resource digest is checked against the frozen scenario digest before the
turn runs. Policy authorizes one bounded `TransientStateCommit`; attachment
text supplies task data but never supplies write authority.

Observed positive facts:

- selected strategy is `transient_state_command` with reason
  `explicit_resource_daily_task_batch`;
- Policy grants `transient_state_commit` but grants neither
  `file_write_proposal` nor `provider_generation`;
- one StateGateway admission covers one SQLite transaction containing three
  ordered task assets, three versions, three outbox rows, and one batch
  operation receipt;
- the canonical task order matches the attachment paragraph order;
- each minimal asset receipt carries only identifiers, digests, projection
  state, and Resource provenance; task bodies remain only in StateStore;
- three immutable `effect_committed` facts contain no task bodies and keep
  transaction-time projection/replay fields byte-stable for recovery;
- LifeModel compatibility projection reaches `applied` without becoming a
  second canonical owner;
- same-operation replay and concurrent same-operation execution produce one
  canonical batch and three tasks, not duplicate writes;
- the reply reports three created tasks and explicitly reports that no file
  approval was created.

The prompt's file clause is conditional. The task can be completed without a
file, so treating it as file-write authorization would be an overreach. This
run therefore creates zero file proposals, zero files, zero tool calls, and
zero Provider calls. The optional reviewed-file branch remains covered by the
separate RC-06/CC-01 artifact chain rather than being fabricated inside CC-02.

The counterfactual adds both English and Chinese prompt-injection paragraphs
to the untrusted attachment. Neither paragraph becomes a task, tool call,
proposal, or file effect. More importantly, even unrecognized attachment text
cannot widen the sealed TransientState-only capability grant.

Root failures found and removed:

1. The frozen prompt initially routed to `direct_answer`. Deterministic intent
   classification now recognizes only the explicit attachment + extraction +
   today + task conjunction and seals a non-serializable transient-state
   grant; the model cannot authorize the batch.
2. Sequential single-task writes could partially commit. StateStore schema v3
   adds one bounded batch operation and item receipts so all task assets,
   versions, and outbox rows commit or roll back together.
3. Equal task timestamps caused UUID ordering to scramble checklist order.
   Canonical task reads now join the batch ordinal instead of changing
   timestamps or adding a second task store.
4. Reconstructing an immutable effect event with current projection/replay
   state could conflict after recovery. The event now records the stable
   transaction-time fact; mutable projection and replay truth stays in the
   outbox-backed receipt and execution metadata.

This is canonical local storage and command-surface evidence. CC-02 uses the
frozen DOCX bytes plus canonical paragraph representation; it does not claim a
second native-picker/parser trial, external Provider credit, native desktop
credit, or application-process restart credit.

CC-02 implementation commit:
`3d2fff23d0df8ce185d2944918828cb83276ec12`.

Mechanical evidence after the repair:

- exact CC-02 positive and untrusted-attachment tests — 2 passed;
- CC-02 Policy test — 1 passed;
- StateStore tests, including v1/v2 migration, atomic rollback, concurrent
  same-operation replay, ordering, and minimal receipts — 19 passed;
- full Main Chat kernel — 71 passed;
- full Main Chat command surface — 79 passed;
- Main Chat runtime module — 30 passed;
- single-system authority guards — 32 passed;
- `cargo check -p openlife-tauri --tests` — passed with existing dead-code
  warnings only;
- `cargo fmt --all -- --check`, `git diff --check`, and staged diff check —
  passed.

## CC-03 exact scenario

Frozen prompt:

> 请记住：我的路演回答偏好是先给一句结论，再给三点证据。随后撤销这条记忆并重启检查。

Policy treats this as one explicit low/medium-risk reversible Memory fact plus
one narrowly bound same-instruction rollback. The source must be the current
canonical user message. The rollback grant is non-cloneable, non-serializable,
bound to the Policy contract, candidate, canonical owner, and exact commit
receipt, and consumed at MemoryGateway.

Observed positive facts:

- selected strategy remains `reversible_memory_commit` and Policy grants only
  the explicit Memory commit and rollback capabilities;
- the canonical lifecycle store records one `owner_created` admission followed
  by one rollback tombstone and an applied outbox projection;
- the final record is `rolled_back`, excluded from runtime retrieval, and the
  active Memory count returns to zero;
- execution order contains one completed `memory.explicit_write` action and
  one completed `memory.explicit_rollback` action;
- aggregate truth distinguishes the historical write/rollback from current
  active truth: `canonicalMemoryActive=false` and
  `acceptedDurableTruthWritten=false`;
- commit and rollback receipts contain references, digests, status, and
  projection facts but do not copy the Memory body;
- the turn creates zero Proposal, Tool, Provider, file, or canonical
  LifeModel-HS effects.

The recovery run reconstructs process-local runtime state around the same
persistent MemoryLifecycleStore and retries the same canonical session,
message, and UUIDv4 operation identity. It observes `terminal_historical`,
recovers the original rollback receipt with `replayed=true`, executes no new
direct Memory write or rollback, leaves zero active Memory, and retains exactly
one rollback event. This is persistent-store reopen and response-loss recovery;
it is not evidence that the desktop application process was actually stopped
and relaunched.

A separate backend OS-process harness now starts the same Rust test executable
twice against one file-backed AppState store set. The seed process executes the
exact CC-03 turn and exits. A distinct verify process reopens Conversation,
AgentRun, TaskSession, ActionQueue, TurnEvent, MemoryLifecycle, and LifeModel
storage, then invokes the same operation identity. Before and after recovery it
observes exactly two conversation messages, two Memory actions, one rolled-back
Memory owner, one rollback event, and one `final_delivery.created` event, with
no active Memory, Proposal, Provider, or Tool effect. This proves backend
recovery after loss of all process-local runtime/cache state. It does not prove
packaged Tauri bootstrap, production keychain integration, window relaunch, or
native UI projection.

Counterfactual evidence:

- the same text quoted from File, Web, MCP/tool, or Assistant content grants
  neither commit nor rollback authority and yields zero Memory candidate;
- a separate explicit instruction cannot use this lane to roll back an active
  owner created by an earlier instruction; `alias_linked` is preserved and the
  owner remains active;
- terminal-history recovery is reported as recovered prior fact, not as a new
  write or rollback;
- the cancellation commit permit is never held across an external `await`.

Root failures found and removed:

1. Policy had no typed representation for the compound rollback request. The
   new capability is deterministic and is issued only when the same current
   user message explicitly requests commit then rollback.
2. The frozen preference was typed as a generic semantic fact. Explicit
   preference claims now keep the reversible Memory destination while carrying
   `Preference` fact identity; this does not promote them to canonical HS.
3. Main Chat previously stopped after commit. The existing MemoryGateway and
   MemoryLifecycleStore now perform the exact rollback and project a minimal
   receipt; no second Memory store or runtime was added.
4. A naive implementation held the deliberately non-`Send` canonical commit
   permit across async projection work. The final implementation fences only
   the synchronous SQLite mutation and reconciles projection after the permit
   is settled.
5. Retrying after response loss needed to distinguish historical recovery from
   new execution. Product wording and aggregate booleans now preserve that
   distinction.

CC-03 implementation commit:
`b080aaa051270318f86254987818d49809d9c568`.

CC-03 separate-process evidence commit:
`b4c15abf8ceceeebb142a1f0ba4a9a9d575650cb`.

Mechanical evidence after the repair:

- exact CC-03 positive/reopen, pre-existing-owner, and separate OS-process
  recovery tests — 3 passed;
- CC-03 Policy positive and four-source quoted counterfactual matrix — 2
  passed;
- Memory candidate tests — 18 passed;
- Memory lifecycle transaction/concurrency/migration tests — 37 passed;
- MemoryGateway tests — 27 passed;
- Main Chat kernel-filtered regression — 99 passed;
- full Main Chat command surface — 88 passed;
- Main Chat runtime module — 30 passed;
- single-system authority guards — 32 passed;
- `cargo check -p openlife-tauri --tests` — passed with existing dead-code
  warnings only;
- `cargo fmt --all -- --check`, `git diff --check`, and staged diff check —
  passed.

The broad legacy/core `main_chat_agent_v1` filter remains 132 passed / 8
pre-existing RED, matching the already-recorded H2 baseline debt. Those stale
MCP/network/eval/minimization assertions are not counted as CC-03 evidence and
are not hidden by the scoped green gates.

## Default-release quarantine

Commit `a53688fe8da834bfd8bab528cdf2a1049caf45f9` closes the roadshow
release-path exposure without removing the basic Agent capabilities used by
the frozen journeys:

- the default bootstrap registry retains governed Web, file, task, and Memory
  manifests, but excludes `builtin_echo`, generic `mcp.call_tool`, and A2A;
- MCP registration, inspection, listing, recommendation, audit administration,
  and Plugin administration commands are compiled into the Tauri handler only
  with `dev-extensions`;
- A2A remains build-gated and the frontend MCP/A2A routes, menu entries, audit
  controls, and Plugin administration consume backend `devExtensionsEnabled`
  truth and fail closed while that truth is missing or false;
- Scheduler execution and periodic Vector tier maintenance no longer start in
  the default release. They start only in the isolated development-extension
  build;
- direct Provider-route preferences keep their existing compatibility command
  names because they are part of ordinary model selection, not Scheduler-backed
  automation;
- explicit Memory search, diagnostics, and user-confirmed repair remain core
  product capabilities. They are not the quarantined attachment route. The
  ResourceGateway selector remains deterministic and does not call VectorStore.

Mechanical evidence:

- release registry capability/absence tests — passed;
- release handler, bootstrap, background-worker, capability, and runtime-build
  guards — 7 relevant Phase0 tests passed;
- actual release bootstrap replay/recovery test with the extension echo utility
  absent — passed;
- `cargo check -p openlife-tauri --tests --features dev-extensions` — passed,
  proving the isolated development surface still compiles;
- current default Main Chat command surface — 93 passed;
- Main Chat runtime module — 30 passed;
- single-system authority guards — 32 passed;
- focused frontend route/settings/privacy tests — 66 passed;
- frontend typecheck and format check — passed.

The broader Phase0 filter also exposed two pre-existing documentation/inventory
failures: a stale immutable fingerprint for `BR4-D044`, and a frozen
traceability reference to the removed test name
`registry_requires_typed_contract_and_executes_matching_mcp_manifest`. Neither
failure is counted as quarantine credit or changed to obtain a green result.
They remain explicit historical-governance debt outside this roadshow slice.

## Reliability stress and fault-injection gates

The current cumulative implementation completed the frozen repeated-run gates
without weakening assertions or changing expected outcomes:

- deterministic journey loops: 50 rounds, 900/900 assertions passed;
- race and replay loops: 20 rounds, 120/120 assertions passed;
- mixed-capability loops: 20 rounds, 140/140 assertions passed;
- fault-injection matrix: 14/14 assertions passed;
- current default Main Chat command surface: 93/93 passed;
- Main Chat runtime module: 30/30 passed;
- single-system authority guards: 32/32 passed;
- focused frontend release-route/settings/privacy behavior: 66/66 passed,
  with frontend typecheck and format checks green;
- development-extension Tauri test compilation and the authenticated A2A
  parent/auth/bounds suite: passed.

These are repeatability and mechanical regression facts. They do not replace a
native product journey, an external live Provider run, or an independent
read-only review. The two previously recorded Phase0 historical-governance
failures remain red and were not renamed or waived to make this section green.

## Default-feature bundle and native shell trial

Commit `ea8a7b246e845f05fbe663cc96e5c5599715d2ae` removed two real blockers found
by building and launching the product artifact rather than inferring readiness
from unit tests.

First, the Tauri bundler enumerated the feature-gated A2A binary from the
product Cargo package and failed because the default build correctly had not
produced that binary. The A2A server now lives in the explicit
`tools/openlife-a2a-server` workspace package, remains authenticated and
`dev-extensions`-gated, and is no longer a product-package binary target. The
default-feature bundle now succeeds and its `Contents/MacOS` directory contains
only `openlife-tauri`; no MCP or A2A executable/resource is present.

Second, the first packaged launch blocked before creating a window. A native
process sample traced the main thread to synchronous macOS Keychain access
during bootstrap. Startup-only secret operations now run through a bounded,
noninteractive adapter with a 1.5 second hard timeout and an open circuit after
the first timeout. The Keychain interaction guard remains owned by the caller,
so it is restored even if the worker outlives the timeout. Settings-time secret
access remains interactive; no plaintext, file, or ephemeral-key fallback was
added.

Observed native facts on an isolated release-profile data directory:

- the packaged process reaches the AppKit event loop and displays a window;
- the same commit artifact quits and relaunches successfully;
- the build identity shown by the product is
  `OpenLife release · debug_bundle · ea8a7b2`;
- release navigation exposes Today, Companion, Mailbox, Life Model, Runs, and
  Settings; the Advanced area exposes Metrics, Calibration, and Versions;
- MCP, A2A, and Plugin administration surfaces are absent;
- when the ad-hoc debug bundle cannot access the expected Keychain secret, the
  product enters explicit Safe Mode, disables canonical writes/Provider/tool
  execution, and keeps unavailable truth unknown instead of silently creating
  replacement keys.

Signing/credential preflight found zero valid local code-signing identities.
The executable is linker ad-hoc signed with no TeamIdentifier, while the bundle
metadata reports `ai.openlife.app`. Canonical credential-item metadata exists
in the login Keychain, but no secret value was read during diagnosis; the
packaged Provider credential remains unconfigured. Relaxing Keychain ACLs,
copying plaintext, or minting replacement canonical keys was explicitly
rejected as false-green evidence.

Evidence boundary: this is a default-feature **debug, ad-hoc bundle**, not a
signed or notarized production release. Safe Mode proves bounded fail-closed
startup and shell relaunch, not healthy production Keychain integration. It
does not earn native RC-01 through RC-08 or CC-01 through CC-03 journey credit,
external live Provider credit, native picker credit, or full packaged restart
credit for their domain effects. The Tauri bundle-identifier warning for
`ai.openlife.app` is recorded as release configuration debt and is not changed
without an identity/keychain/data migration decision.

## Remaining cumulative work

- RC-01 native product UI trial; its exact external live Provider gate is now
  complete twice on the current implementation.
- RC-02/RC-03 native picker, healthy packaged restart, and repeated product
  trial; their exact external live Provider gates are now complete twice.
- RC-05 native task journey and repeated product trial on a healthy packaged
  application; generic shell bootstrap/relaunch is now evidenced separately.
- RC-06/RC-07 packaged/native Review Center and file trial, plus RC-07
  separate-process end-to-end bundle evidence; their exact external live
  Provider gates are now complete twice.
- CC-01 native picker, live-Web report review, and file trial; its exact external
  live backend gate is now complete twice.
- RC-08 native cancel/retry projection and external live Provider/Web journey;
  generic shell bootstrap/relaunch is now evidenced separately.
- CC-03 native Memory commit/rollback projection on a healthy packaged
  application; generic shell bootstrap/relaunch is now evidenced separately.
- signed/notarized release identity, healthy production Keychain access,
  packaged/native live product rounds, and independent rereview. Widened
  frontend regression and reliability loops are complete, but do not substitute
  for those remaining evidence dimensions.
