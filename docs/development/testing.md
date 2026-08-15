# Testing OpenLife

Use the smallest check that matches the change, then broaden before merging.

## Fast Checks

```sh
git diff --check
cargo fmt --check
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
```

## Product Tests

```sh
cargo test -p openlife-core --locked
cargo test -p openlife-tauri --locked
corepack pnpm --dir frontend test
corepack pnpm --dir frontend build
corepack pnpm --dir frontend test:e2e
```

## Full Gate

```sh
cargo clippy --all --locked -- -D warnings
cargo test --all --locked
```

## Evidence Levels

- unit/contract tests prove only their code contract;
- browser-shell tests prove React routing and rendering, not native Tauri;
- native Tauri tests prove the exact local build and trial path;
- scripted or local HTTP providers are not external-live providers;
- external-live behavior requires an explicitly authorized live-provider run.

Tests must fail closed. A blocked prerequisite must not return success, and a
passing test must not be used as evidence for a broader layer.

Tests use synthetic resources under `test-fixtures/`. They must not read real
application data, Keychain contents, or private user files.

## H5 behavior matrix and evidence

Run the controlled H5 matrix with:

```sh
scripts/h5-behavior-matrix.zsh
```

Every row below enters through the release Chat/Work coordinator or its
canonical persistence/checkpoint contract. The matrix deliberately uses both
Chinese and English instructions; language never selects a different runtime.

| Matrix row | Language | Production owner exercised | Canonical and user-visible result | Minimum evidence |
| --- | --- | --- | --- | --- |
| `chat_bilingual_direct_answer` | Chinese + English | `CanonicalChatRuntime` | one Conversation Turn with user/assistant Items; no Task | controlled + exact-native |
| `chat_replay` | English | `CanonicalChatRuntime` | completed Turn replays without a second provider call | controlled |
| `chat_provider_failure` | language-independent | `CanonicalChatRuntime` | failed request creates no partial Turn | controlled + exact-native |
| `chat_scoped_startup_admission` | language-independent | Conversation and canonical Task store admission | an unavailable retired store cannot block a healthy canonical Chat/Work owner | controlled + exact-native |
| `work_direct_answer` | English | `CanonicalWorkRuntime` | one Task/Run/provider ItemAttempt/FinalResult | controlled + exact-native |
| `work_document_chinese` | Chinese | Work planner and `document.read` ToolGateway | exact bound document observation and cited result | controlled + exact-native |
| `work_web_chinese` | Chinese | Work planner and `web.search` ToolGateway | governed Web receipt, observation, and cited result | controlled + external-live |
| `work_mixed_report_chinese` | Chinese | Work planner, document/Web tools, Artifact checkpoint | one mixed-source Artifact waits for Review | controlled + exact-native + external-live |
| `work_selected_skill` | English | Work planner and selected Skill context port | bounded Skill observation on the same Run | controlled + exact-native |
| `work_read_only_mcp` | English | Work planner and registered MCP ToolGateway | read-only MCP attempt, receipt, and observation | controlled + exact-native |
| `work_plan_item_chinese` | Chinese | Work planner | Plan is an Item inside the Work Run, not another lifecycle | controlled + exact-native |
| `work_negative_output_constraints_chinese` | Chinese | Work policy and CompletionEvaluator | negated file/save terms remain output constraints; the requested answer completes without a false write ambiguity or incomplete-step blocker | controlled + exact-native |
| `work_steering` | language-independent | canonical Task runtime steering checkpoint | exact pending input survives restart and is consumed once | controlled + exact-native |
| `work_checkpoint_accept` | Chinese | Artifact Review checkpoint and materializer | approval resumes the same Task and verified delivery completes it | controlled + exact-native |
| `work_checkpoint_reject` | language-independent | canonical Artifact checkpoint | rejection blocks the same Task with no delivery | controlled + exact-native |
| `work_cancel` | language-independent | canonical cancellation owner | Turn/Run/Item/Attempt end cancelled; late completion is rejected | controlled + exact-native |
| `work_retry` | language-independent | canonical Work retry | a new Run belongs to the same Task | controlled + exact-native |
| `artifact_verification_undo` | language-independent | canonical Artifact effect and Undo checkpoint | only receipt-bound verified materialization can be undone | controlled + exact-native |
| `blocked_scope` | language-independent | Project scope admission | changed scope stays blocked and appears in Needs Attention | controlled + exact-native |
| `effect_unknown` | language-independent | canonical Artifact effect journal | uncertain effect never becomes completed delivery | controlled + exact-native |
| `work_provider_failure` | language-independent | Work provider ItemAttempt | Task fails with no FinalResult | controlled + exact-native |
| `restart_recovery` | language-independent | canonical Task recovery | open Run is interrupted and retry creates one new Run | controlled + exact-native |
| `workbench_user_visible_states` | Chinese UI | backend ViewModels and React Workbench | Chat, progress, inline decision, result, attention and controls remain one Conversation surface | browser-shell only until exact-native |

The H5 closure passed every controlled row in this table. The exact signed QA
bundle and isolated profile then exercised the visible Workbench through Chat,
canonical Work, selected-document plus real Web evidence, inline Artifact
Review, verified single materialization, cancellation, retry, and restart after
a changed executable CDHash. The authorized external-live case used the
selected DeepSeek `deepseek-v4-flash` route and real Web search without silent
provider substitution. No credential, provider payload, resource body, or
generated private content is retained as test documentation.

These results do not merge evidence levels: a controlled-only row remains
controlled, browser-shell remains UI-contract evidence, and exact-native/live
credit applies only to the golden paths actually exercised. Future changes to
the runtime, signing/profile boundary, Provider adapter, Web adapter, Review,
or materializer invalidate the proportional evidence and must rerun it.

## Historical report behavior matrix

Run the controlled S6 report matrix with:

```sh
scripts/s6-report-matrix.zsh
```

This historical script proves bounded report contracts that remain useful as
migration evidence. It does not prove the reconstructed general Agent, a native
product path, or an external-live path. It is not an R4 acceptance gate; R4
uses canonical Work generation, approval, materialization, restart recovery,
verification, and Undo product tests.

The required external-live report case is gated separately:

```sh
scripts/live-eval.zsh cargo test -p openlife-tauri --locked \
  reconstruction_external_live_document_web_report_waits_for_review_then_materializes_once \
  -- --ignored --nocapture
```

It uses the configured provider and real Web access through the canonical Work
owner, then proves Review-gated single Artifact materialization without growing
another execution owner. Never paste provider payloads,
credentials, resource bodies, or generated report content into plans or test
summaries. A failed or unavailable live adapter remains blocked.

Native review must use an exact current Tauri bundle and a purpose-specific
data profile. Release uses the product Keychain service. Dev and QA builds use
their own `0600` local profile secret file so rebuilds cannot inherit release
credentials or turn changing development CDHashes into repeated Keychain
prompts. A Provider or Search credential is never copied between profiles; it
must be entered explicitly in the profile that will use it.

## macOS exact-native identity

R0 and later native evidence must use an explicit local signing identity rather
than a linker-generated ad-hoc executable identity:

```sh
OPENLIFE_CODESIGN_IDENTITY="OpenLife Local Code Signing" \
  scripts/macos-exact-native.zsh
```

H5 QA evidence uses the separate QA identity and data profile:

```sh
OPENLIFE_NATIVE_PROFILE=qa \
OPENLIFE_NATIVE_BUILD_NONCE=h5-cross-build-a \
OPENLIFE_CODESIGN_IDENTITY="OpenLife Local Code Signing" \
  scripts/macos-exact-native.zsh
```

Repeat with a different nonce and verify that the executable CDHash changed,
the Bundle ID and Designated Requirement stayed `ai.openlife.desktop.qa`, and
the same QA profile opens without credential initialization or recovery. The
nonce exists only to make this cross-build acceptance reproducible; it is not a
runtime credential or product identifier.

The release verifier requires `ai.openlife.desktop`; QA requires
`ai.openlife.desktop.qa`. The legacy `ai.openlife.app` name remains only as the
explicit pre-reconstruction data-directory migration source and is not a valid
macOS bundle identity.

The script builds the exact source and verifies the configured bundle
identifier, the selected signing authority, and the strict deep resource seal.
It never reads secret values. Release continues to use the fixed product
Keychain service. Dev and QA use separate app-data directories and atomic local
profile secret files with Unix mode `0600`; this is an explicit development
tradeoff, not release-path credential evidence. Their Settings UI identifies
that storage boundary before a user enters a Provider credential.

The old local self-signed Keychain path is not credited for H5 because repeated
development builds did not reliably retain non-interactive access. H5
cross-build credit therefore uses the isolated QA profile-file path above and
proves reuse by reopening it from an exact build with a different CDHash. A
future distributed release still needs a stable Developer ID/Team identity and
must prove normal Keychain access across signed updates before shipment.
R0's measured exact-binary baseline on 2026-08-13 was 216.7 ms from launch to
all protected execution stores open, 70,592 KiB RSS at that boundary, and zero
observed network sockets. These are comparison measurements, not a release SLA.

## Reconstruction golden matrix

Run the controlled R8 reconstruction matrix with:

```sh
scripts/r8-golden-matrix.zsh
```

It covers the canonical Chat and Work runtimes, tool/document/Web behavior,
steering, review and Artifact Undo contracts, restart/retry, bounded
concurrency, background frontend behavior, independent personal-intelligence
ports, and Product Diagnostics. The diagnostics row enforces a controlled
750 ms ceiling for reading 200 canonical Conversations and Tasks. This is a
regression budget for the test environment, not a native startup or UI SLA.

The matrix also asserts bounded provider invocation counts inside the runtime
tests: exact Chat/Work replay performs no second request and Web citation retry
records each real attempt. It never estimates token price from text or upgrades
a local HTTP adapter to external-live evidence.

Passing this script is only controlled evidence. R8 still requires the exact
signed bundle and purpose-specific native profile; external-live provider/Web
checks remain separately gated through `scripts/live-eval.zsh`.
