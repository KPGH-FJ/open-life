# OpenLife Roadshow Cumulative Integration Evidence

> Scope: cumulative roadshow capability integration only. This record is
> subordinate to `AGENTS.md`, the Phase7 authority stack, and
> `openlife_roadshow_core_capability_execution.md`.

## Current verdict

- Cumulative Integration is **in progress**.
- RC-04 has passed a single-command mechanical integration run.
- RC-04 has **not** received native desktop, external live-provider, repeated
  product-trial, or independent-review credit.
- RC-08 and CC-01 through CC-03 remain pending.
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

## Remaining cumulative work

- RC-08 cancellation, remote-unknown truth, one retry, restart, and no-late-
  commit chain.
- CC-01 Resource + Web + reviewed Markdown artifact.
- CC-02 Resource + transient tasks + conditional reviewed file write.
- CC-03 explicit reversible Memory + undo + restart.
- full RC-01 through RC-08 cumulative harness, negative scans, single-system
  guards, widened frontend/backend regression, reliability loops, live product
  rounds, and independent rereview.

