# OpenLife Repository Stage7 Scope Reset Baseline Decision

> Date: 2026-07-08
> Status: docs-only repository cleanup decision
> Authority: repository cleanup decision record subordinate to `AGENTS.md`,
> `plans/README.md`, and the Phase7 single-system contract.

## Stage7 Objective

Stage7 resets this repository cleanup thread back to documentation and
knowledge-architecture scope after the Stage6E native product trial.

The Stage7 objectives are:

- keep this pass out of product ToolPermission, web read, network policy, and
  native trial repair work;
- sync Stage6E facts into repository cleanup records without converting product
  RED findings into documentation blockers;
- refresh the inventory/link baseline metadata at the summary level;
- verify active documentation does not overstate Phase7, Main Chat Agent
  Execution v1, or external live-provider completion;
- decide whether the next repository cleanup slice may enter `AGENTS.md`
  compression.

## Stage6E Product RED Boundary

Stage6E is a product trial result, not a repository cleanup failure.

The Stage6E native rerun remains RED because ToolPermission accept/resume did
not earn completion credit, the LifeModel Mailbox proposal/materialization path
was not completed natively, and no external live-provider credit exists. Those
items are product development follow-ups. They do not require Stage7 to edit
Rust, Tauri, React, frontend bridge, ToolPermission, web read, network policy,
or native trial code.

Stage7 therefore records the Stage6E RED state only as a product TODO boundary:
repository cleanup may continue as docs-only work, but it must not claim Phase7
completion, Main Chat Agent Execution v1 completion, or external live-provider
evidence completion.

## Product Code Repair Exclusion

This pass does not modify product behavior code and does not attempt to repair:

- ToolPermission accept/resume completion;
- `web.search` or `web.fetch` policy behavior;
- network-policy or provider execution behavior;
- native trial flow, seed paths, WebDriver paths, or fixture evidence;
- Tauri command handlers, frontend product pages, or the product bridge.

Stage7 also does not restore
`run_main_chat_agent_execution_v1_final_acceptance_gate`, does not restore
`src-tauri/src/main_chat_final_acceptance_tests.rs`, does not move ADR 0013,
and does not mass move `plans/` into an archive namespace.

## Baseline Refresh Result

Stage7 refreshes the baseline by appending a `stage7_summary` to:

- `plans/openlife_repository_document_inventory.json`;
- `plans/openlife_repository_document_link_baseline.json`.

This is not a full inventory/link recomputation.

| Field | Stage7 value |
| --- | ---: |
| `refreshed_at` | `2026-07-08T13:38:59+0800` |
| `recomputed` | `false` |
| current `rg --files -g '*.md' -g '*.html'` count | 209 |
| inventory document records retained from existing baseline | 205 |
| active broken Markdown link count retained from existing baseline | 0 |
| active actionable repair count retained from existing baseline | 0 |
| active missing records retained as expected-absent evidence | 37 |

Stage4C and Stage5B summaries remain historical/current-status records in the
JSON artifacts. Stage7 does not rewrite those time-point rows and does not
pretend the document inventory details were regenerated.

## Active Claim Scan Result

The Stage7 completion-claim scan has matches only in prohibited-claim lists,
validation command examples, historical plans, or explicit caveat language.
No reviewed hit was interpreted as a current active claim that Phase7, Main
Chat Agent Execution v1, or external live-provider evidence is complete.

The retired final acceptance command scan remains clear for shipped/product
surfaces. Matches in the preparation document are historical/guard text, not
shipped handler or frontend bridge restoration.

The `tauriDev` product import scan returns no matches in the checked product
pages/components and `frontend/src/tauri.ts` surface.

## Stage8 Decision

`ready_for_stage8_agents_compression=true`

Reason: repository cleanup active actionable repair count remains 0, active
broken Markdown link count remains 0, Stage4C expected-absent records are
classified as deletion/test/archive evidence, Stage5B already records the
runtime-module guard supersession without restoring retired owners, and Stage6E
RED findings are product TODOs rather than repository cleanup blockers.

Stage8 is allowed only as `AGENTS.md` compression. It must preserve the Phase7
authority stack, the Main Chat / live-provider non-completion caveats, the no
silent write / proposal-first constraints, and the retired-command absence
contract. It must not start product fixes unless a later user instruction
explicitly changes scope.

## Prohibited Claims

Stage7 and Stage8 must not claim:

- Phase7 complete or completed;
- Main Chat Agent Execution v1 complete or completed;
- external live-provider evidence complete;
- `finalCompletionReady=true`;
- the retired final acceptance command is restored;
- the deleted old final-acceptance test-owner is restored;
- Stage6E RED product findings are repository cleanup blockers;
- AGENTS compression is product trial completion or authority promotion.
