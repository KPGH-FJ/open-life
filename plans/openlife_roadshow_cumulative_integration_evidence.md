# OpenLife Roadshow Cumulative Integration Evidence

> Scope: cumulative roadshow capability integration only. This record is
> subordinate to `AGENTS.md`, the Phase7 authority stack, and
> `openlife_roadshow_core_capability_execution.md`.

## Current verdict

- Cumulative Integration is **in progress**.
- RC-04 has passed a single-command mechanical integration run.
- RC-08 has passed a local cancellation/new-operation-retry mechanical run.
- RC-04 and RC-08 have **not** received native desktop, external live-provider,
  repeated product-trial, or independent-review credit.
- RC-08 full process-restart reconciliation and CC-01 through CC-03 remain
  pending.
- The roadshow candidate remains **NO-GO**.

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

This is local fixture plus local HTTP adapter evidence. It is not external live
Web or cloud-provider evidence.

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

Neither repair removes context evidence, broadens write authority, or lets a
model authorize the Web route.

## Mechanical evidence

- `cargo test -p openlife-core resource_selection::tests:: -- --nocapture` —
  3 passed.
- `cargo test -p openlife-core roadshow_external_read_policy_tests -- --nocapture`
  — 2 passed.
- exact RC-04 command-surface test — passed.
- `cargo test -p openlife-tauri main_chat_command_surface_tests:: -- --nocapture`
  — 74 passed.
- `cargo test -p openlife-tauri main_chat_kernel::tests:: -- --nocapture` —
  71 passed.
- provider selected-context source-reference schema test — passed.
- `cargo check -p openlife-tauri --tests` — passed with existing dead-code
  warnings only.
- `cargo fmt --all -- --check` and `git diff --check` — passed.

Implementation commit: `02fd7580a1078a57e6308921a5ed61f357e4e17d`.

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

The harness then clears process-local Main Chat runtime facts and performs the
explicit retry as a new UUIDv4 operation. The retry rebinds the same fixture,
dispatches one Web action and one Provider synthesis request, validates both
Resource and Web citations, creates zero proposals, and finishes one AgentRun
as `Completed`. The raw Web body marker remains absent from product IPC and its
execution receipt.

This proves local runtime-state loss and a user-visible new-operation retry. It
does **not** prove full application-process restart/reopen, native desktop UI,
external live Web, or external cloud Provider behavior; those evidence
dimensions remain pending.

The exact Chinese `检索网页` phrase initially failed to authorize Web read
capability. Policy intent classification now recognizes explicit search/query
Web phrases while the existing webpage-design counterexample still receives no
Web authority. The model cannot grant this capability.

RC-08 implementation/evidence commit:
`f84bed579b9e27bb0e3eb974cd66c38082a369b3`.

Additional mechanical evidence:

- exact RC-04, RC-06, and RC-08 roadshow command tests — 3 passed;
- `roadshow_external_read_policy_tests` — 3 passed;
- generic released-late-provider cancellation regression — passed;
- full Main Chat command surface — 75 passed;
- `cargo check -p openlife-tauri --tests` — passed with existing dead-code
  warnings only.

## Remaining cumulative work

- RC-08 full application-process restart/reopen and native UI projection.
- CC-01 Resource + Web + reviewed Markdown artifact.
- CC-02 Resource + transient tasks + conditional reviewed file write.
- CC-03 explicit reversible Memory + undo + restart.
- full RC-01 through RC-08 cumulative harness, negative scans, single-system
  guards, widened frontend/backend regression, reliability loops, live product
  rounds, and independent rereview.
