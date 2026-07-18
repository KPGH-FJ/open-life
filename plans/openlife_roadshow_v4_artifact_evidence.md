# OpenLife Roadshow V4 Artifact Evidence

Status: the bounded Markdown/CSV reviewed-artifact slice is mechanically
verified. External cloud-provider evidence, a native desktop product trial,
cumulative journey evidence, and an independent read-only review remain
pending. This file does not claim that Phase7, the roadshow release, Backend
Remediation v4, or global BR4-D020/BR4-D050 is complete.

## Scope and commits

The V4 implementation is commit
`202bacc41c7347c90cf6f33b33bc2a2ba7d85863`; the exact frozen RC-06 scenario
test is commit `4444704293423c1f259fb2613ec9ce7bb6739413` on
`codex/roadshow-core-recovery`:

- `ProposalStore` remains the canonical proposal owner and now records only
  minimal artifact-effect metadata: proposal, claim and snapshot bindings,
  target/content digests, byte size, media type, state, observation and error;
  artifact bodies are not copied into the effect ledger;
- `ArtifactMaterializer` validates the configured safe root and path before
  staging, rejects symlink traversal, writes and `fsync`s a deterministic stage
  file, records staged truth, atomically renames it, `fsync`s the directory and
  confirms the observed digest;
- filesystem rename and SQLite state are not described as one atomic action.
  Restart reconciliation inspects the stage/final bytes and digest and reports
  `confirmed`, retryable `failed_before_effect`, or `unknown` without blind
  redispatch;
- an artifact dispatch claim is bound to the exact proposal version, effect
  snapshot and target. Proposal/effect CAS failure rolls back both sides, and
  only a proven pre-effect failure can acquire a replacement claim;
- `ExternalWriteAction` acceptance has one product effect route through
  `ArtifactMaterializer`; the former direct `safe_write_utf8` route is absent,
  and the generic state-apply route rejects external writes;
- a confirmed, typed backend `ArtifactMaterializationReceipt` is the only
  product completion fact. Proposal creation, staged bytes, missing receipt,
  projection lag or unknown observation cannot be presented as a saved file;
- generated Markdown/CSV uses the existing Main Chat TurnRuntime and a real
  ProviderAdapter request with purpose `main_chat_artifact_draft`. The Provider
  may return bounded document bodies, but the backend owns filenames, safe
  roots, media contracts, Proposal creation and all effects;
- generated output is strict JSON, rejects unknown/path-injection fields,
  validates the complete CSV row shape, and is capped at 100 KiB per artifact;
- one generated bundle expands to one Proposal per artifact. The parent task
  remains `WaitingPermission` until every linked review is terminal, then
  becomes `Completed` only after confirmed materialization receipts;
- startup recovery repairs the task projection before consuming the linked
  waiting AgentRun marker, removing the crash window that could otherwise leave
  a permanent permission wait;
- Mailbox renders path, bytes and digest only from the confirmed backend
  receipt. It does not infer file completion from Proposal status or prose.

## Frozen RC-06 and RC-07 evidence

The exact frozen prompts were not edited and no waiver was added.

### RC-06 permission wait and resume

Prompt: `把最终摘要保存到工作区的 roadshow-summary.md。`

The command-surface test observes one local HTTP ProviderAdapter request, one
pending Proposal and no file before review. Accepting the exact Proposal emits
a confirmed receipt whose content is the expected provider-generated Markdown,
materializes exactly `roadshow-summary.md` under the canonicalized safe root,
and moves the linked task to `Completed`.

The exact-prompt test is combined with the independent dispatch/restart tests
below for lifecycle evidence. It is not described as a native desktop restart
trial.

### RC-07 Markdown and CSV bundle

Prompt: `生成一份 Markdown 路演摘要和一份 CSV 风险清单，并在我确认后保存。`

The command-surface test observes one local HTTP ProviderAdapter request and
two pending Proposals. Neither final file exists before acceptance. Accepting
both Proposals produces exact Markdown and CSV bytes, two confirmed digest
receipts and one completed parent task. Reaccepting a confirmed Proposal returns
the original receipt and leaves only the two final files, with no stage copy or
second dispatch.

The durable AgentRun and product response are checked not to contain the
generated artifact bodies. The local HTTP fixture proves the real adapter and
request/receipt lifecycle; it is not external cloud-provider credit.

## Mechanical evidence

Verified on 2026-07-15 in `/Users/tw/Desktop/open-life-roadshow`:

| Gate | Result | Credit boundary |
| --- | --- | --- |
| `openlife-core` generated-artifact policy filter | 2/2 passed | exact current-user authority; quoted/untrusted instruction cannot authorize generation or write |
| `openlife-core` artifact ProposalStore filter | 3/3 passed | exact claim binding, atomic proposal/effect transition, CAS rollback, retry only before effect |
| Tauri generated-artifact kernel filter | 3/3 passed | provider owns content only; path injection and malformed late CSV row fail before Proposal creation |
| Tauri `artifact_materializer::tests` | 2/2 passed | safe path/symlink enforcement, staged-vs-final truth, exact digest confirmation |
| Tauri `artifact_restart_` filter | 3/3 passed | prepared-without-bytes retry, staged recovery, rename-before-receipt observation without rewrite |
| exact RC-06 command-surface test | passed | one Provider request, wait-before-effect, confirmed receipt, one summary file, completed task |
| exact RC-07 command-surface test | passed | one Provider request, two governed artifacts, no pre-accept file, exact receipts, idempotent reaccept |
| missing-safe-root counterfactual | passed | structured `artifact_safe_path_unavailable` blocker, no Proposal and no raw IPC false success |
| `cargo test -p openlife-tauri commands::proposal::tests -- --nocapture` | 66/66 passed | complete current Proposal command module, including acceptance, recovery and non-confirmed receipt rejection |
| current Tauri binary, `main_chat_runtime_module` | 30/30 passed | single TurnRuntime and retired-route/authority guards |
| `cargo check -p openlife-tauri --tests` | passed with two existing warning groups | all Rust/Tauri test targets compile; no warning-free claim |
| frontend MailboxPage test | 25/25 passed | confirmed receipt display and non-confirmed/unknown truth boundaries |
| frontend typecheck and format check | passed | TypeScript contract and formatting integrity |
| current single-system set | 25/32 passed | no new V4 regression; the same seven pre-existing authority failures remain red |
| `cargo fmt --check` and `git diff --check` | passed | Rust formatting and patch hygiene |

The focused direct-binary reruns use the current test executable produced by
the successful Cargo build to avoid repeated archive growth on a nearly full
disk. They are deterministic local evidence, not product-trial credit.

## Failure, counterfactual and recovery evidence

- assistant prose, quoted instructions and provider-suggested paths cannot
  authorize or redirect a file write;
- a missing safe root becomes an explicit blocker and creates no Proposal;
- an outside-root or symlink target fails before staging;
- malformed generated JSON, unknown fields, oversized body and inconsistent
  CSV rows fail before Proposal creation;
- Proposal creation and staged bytes are not file-completion receipts;
- a rename observed after restart is reconciled by target digest without a
  second write;
- matching staged bytes are completed through the original claim; a prepared
  claim with no bytes is retryable only because no effect is observed;
- an unrecognized or mismatched filesystem state remains `unknown` and is not
  automatically retried;
- failure to update the Proposal and artifact receipt together rolls back both
  database changes;
- a confirmed retry returns the durable original receipt and does not leave an
  extra stage file;
- task projection is reconciled before the waiting AgentRun marker is consumed,
  so a restart cannot permanently hide unfinished task repair;
- artifact bodies remain with the Proposal/filesystem canonical owners and are
  not copied into `artifact_effects`, AgentRun or the product response.

## Bounded red and remaining V4 evidence

The following results are intentionally not converted into green credit:

- no external cloud Provider has generated and saved an artifact in this V4
  evidence pass; local HTTP adapter evidence is not relabeled as external live;
- no native desktop user has completed RC-06 and RC-07 through the visible
  product UI, denied/expired a Proposal, restarted the app and inspected the
  projected result;
- no independent read-only reviewer has re-traced the implementation and
  rerun the gates;
- cumulative RC-01 through RC-08 and CC-01 through CC-03 integration has not
  yet passed as one frozen release candidate;
- the single-system suite remains red in seven pre-existing areas: D011 marker
  drift, empty retired proposal category handling, ReviewWorkflow marker drift,
  MemoryGateway marker drift, Chat pending-state reconstruction,
  ActionExecutor test-surface classification and ProviderPrivacy marker drift;
- the two existing Rust warning groups remain; no Clippy-clean claim is made;
- BR4-D020 and BR4-D050 remain globally open outside this bounded artifact
  subset.

V4 is therefore
`implementation_verified_external_live_product_trial_and_independent_review_pending`,
not fully complete. The next phase is cumulative integration, and the roadshow
release remains NO-GO.
