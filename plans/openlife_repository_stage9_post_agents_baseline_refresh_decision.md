# OpenLife Repository Stage9 Post-AGENTS Baseline Refresh Decision

> Date: 2026-07-08
> Status: docs-only baseline refresh decision
> Authority: subordinate to `AGENTS.md`, `plans/README.md`, and the Phase7
> single-system deletion/product-trial contract.

## Inputs Read

Stage9 was executed only after reading the required inputs:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_repository_stage8_agents_compression_decision.md`
4. `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
5. `plans/openlife_repository_active_claim_audit.md`
6. `plans/openlife_repository_document_inventory.json`
7. `plans/openlife_repository_document_link_baseline.json`

## Objective

Refresh the repository document inventory and local link/path baseline after
Stage8-rework corrected the compressed `AGENTS.md` Main Chat source map.

Stage9 does not start architecture document rewriting, ADR movement, plan
archive work, product behavior repair, or Stage6E product RED repair.

## Recomputed Baseline

Stage9 recomputes the current Markdown/HTML document collection with:

```sh
rg --files -g '*.md' -g '*.html' | sort
```

Stage9 also recomputes local Markdown links and backticked repo-local path-like
mentions. The scanner treats external URLs, `/tmp` validation outputs, command
relative paths such as `corepack pnpm --dir frontend exec node scripts/...`,
existing glob references, and source line anchors as non-broken when the real
target exists. Historical decision tables and explicit negative/absence
contexts are classified separately from current actionable repair records.

The refreshed JSON artifacts are:

- `plans/openlife_repository_document_inventory.json`
- `plans/openlife_repository_document_link_baseline.json`

Both JSON artifacts carry `stage9_summary.recomputed=true`.

Stage9 refreshed counts:

| Field | Stage9 value |
| --- | ---: |
| Markdown/HTML documents | 212 |
| Documents newly added to the inventory since the retained Stage4C baseline | 7 |
| Missing local path/link baseline records | 359 |
| Active broken Markdown links | 0 |
| Active missing path records | 40 |
| Active expected-absent records | 40 |
| Active actionable repair records | 0 |
| Historical/private missing path or link records | 319 |

## Stage8-Rework Facts Recorded

- `AGENTS.md` currently has 190 lines.
- Stage8-rework corrected the Main Chat source map to two parallel command
  branches: `send_message` through `main_chat_send.rs` and
  `start_stream_message` through `main_chat_streaming.rs`.
- Both branches converge through `main_chat_turn_runtime.rs`,
  `main_chat_kernel.rs`, and the core Main Chat agent areas.
- The old compressed implication that `main_chat_send.rs` flows into
  `main_chat_streaming.rs` must not be reused.

## Blocker Classification

Stage9 distinguishes three categories:

| Category | Stage9 decision |
| --- | --- |
| Active broken Markdown links | Must be zero before proceeding. |
| Active actionable repair records | Must be zero before broad ownership cleanup. |
| Active expected-absent path mentions | May remain only when they preserve Phase7 deletion / retired-owner evidence and do not ask to recreate files. |

Stage9 keeps Stage4C, Stage5B, Stage7, and Stage8 as historical/current-status
records rather than rewriting their time-point evidence. Stage6E remains a
product development TODO boundary, not a repository cleanup blocker.

## Readiness Judgment

If the refreshed JSON has `active_doc_broken_links=0` and
`active_actionable_repair_records=0`, the next repository cleanup step may be a
bounded architecture/document ownership cleanup preparation pass.

The refreshed Stage9 JSON satisfies that condition. The next step may prepare
architecture/document ownership cleanup, but it may not skip the source-map and
file-scope decision step or start a broad rewrite directly.

That next step still must not directly rewrite the architecture docs at broad
scope. It should first name the exact owner surfaces, source maps, and files to
touch, and it must preserve the Phase7 non-completion, external live-provider
non-closure, and proposal-first/no-silent-write boundaries.

Stage9 makes no Phase7, Main Chat Agent Execution v1, external live-provider,
or final-readiness closure claim.

## Validation

Stage9 validation results:

| Check | Result |
| --- | --- |
| `git diff --check` | Passed. |
| Inventory JSON parse | Passed; pretty output written to `/tmp/openlife_repository_document_inventory_stage9.json`. |
| Link baseline JSON parse | Passed; pretty output written to `/tmp/openlife_repository_document_link_baseline_stage9.json`. |
| `wc -l AGENTS.md` | Passed with 190 lines. |
| Old sequential Main Chat source-map scan | No matches; this is expected because Stage8-rework replaced the sequential wording with parallel send/stream branches. |
| Prohibited completion/readiness scan | Non-zero by design: matches are validation command examples, prohibited-claim lists, historical plans, or explicit caveat language. No Stage9 closure claim was added. |
| Retired command/test-owner scan over shipped surfaces plus AGENTS/Stage8 decision | Matches only `AGENTS.md` and the Stage8 decision as forbidden/expected-absent wording; no shipped handler, command module, or frontend bridge match. |
