# OpenLife Repository Stage 2C Phase C Readiness Decision

> Decision date: 2026-07-07
> Status: Stage 2C readiness gate for Phase C / Stage3 doc build.
> Authority: subordinate to `AGENTS.md`, `plans/README.md`,
> `plans/openlife_single_system_deletion_manifest.md`, and
> `plans/openlife_single_system_development_preparation.md`.

This decision creates the formal entry condition for Phase C / Stage3 document
build work. It does not create architecture docs, development docs, product
docs, source code, Tauri commands, runtime authority, or Phase7 completion
evidence.

## Decision

Phase C may continue under the current red `main_chat_runtime_module` guard only
as a docs-only source-backed explanatory slice.

This is a formal scope-out, not a fix:

- the red `main_chat_runtime_module` guard blocks runtime promotion, shipped
  command promotion, final-gate authority promotion, and any Phase7 or Main Chat
  Agent Execution v1 completion claim;
- the red guard does not block source-backed explanatory documentation when the
  new documents explicitly record it as an inherited blocker;
- every new Phase C document must include `Status`, `Authority`,
  `Last verified`, `Source map`, and `Inherited blocker` front matter;
- the inherited blocker text must state that
  `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture`
  remains red until a reviewed Phase7 decision reconciles the guard;
- no Phase C document may convert a local/docs-only observation into runtime,
  provider, product, or active-authority readiness.

## Stage3 Readiness Verdict

`ready_for_stage3_doc_build=true`

Reason:

- Stage 1 inventory and link-baseline artifacts exist and parse as JSON.
- Stage 2A defines active-claim and broken-link repair scope.
- Stage 2B records active-doc repair work without restoring retired commands or
  claiming runtime completion.
- Phase C is now limited to source-backed explanatory docs with an explicit
  inherited runtime-module blocker.
- Current target directories are absent, so Phase C can create only populated
  directories in the same patch as real files; no empty directory is allowed.

This verdict does not mean `ready_for_authority_promotion=true`.

## Directory Precheck

Verified before this decision:

| Target directory | Current state | Stage3 rule |
| --- | --- | --- |
| `docs/architecture/` | Missing | May be created only in the same patch as at least one approved architecture doc. |
| `docs/development/` | Missing | May be created only in the same patch as `docs/development/testing.md`. |
| Product-doc namespace | Missing | Remains blocked unless the user separately approves public product docs. |

Do not create empty directories.

## Phase C File Manifest

Initial Stage3 doc build may edit or create only these files:

| File | Required status | Purpose |
| --- | --- | --- |
| `docs/architecture/agent-runtime.md` | Create | Source-backed current Main Chat runtime explainer. |
| `docs/architecture/life-model.md` | Create | Source-backed LifeModel / LifeModel-HS data and proposal explainer. |
| `docs/architecture/governance.md` | Create | Source-backed privacy, policy, permission, proposal, model-route, and tool-governance explainer. |
| `docs/architecture/memory.md` | Create | Source-backed Memory, context, candidate, proposal, and materialization boundary explainer. |
| `docs/development/testing.md` | Create | Current verification command map with local/core/live-provider distinctions. |
| `docs/ARCHITECTURE.md` | Optional edit | Convert to an index or mark historical after replacement docs exist; do not add current runtime claims beyond the new source-backed docs. |
| `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | Optional append only | Record Stage3 validation outcome; no architecture正文. |

Product docs are not part of initial Stage3 readiness. No concrete product-doc
filenames are authorized unless the user explicitly approves a public-safety
reviewed product doc slice.

## Source-Map Starting Points

Every Phase C document must start from current source and active authority, not
from historical Goal/Stage/Beta plans alone.

### `docs/architecture/agent-runtime.md`

Start with:

- `plans/README.md`
- `plans/openlife_single_system_deletion_manifest.md`
- `plans/openlife_single_system_development_preparation.md`
- `src-tauri/src/main_chat_send.rs`
- `src-tauri/src/main_chat_streaming.rs`
- `src-tauri/src/main_chat_turn_runtime.rs`
- `src-tauri/src/main_chat_turn_pipeline.rs`
- `src-tauri/src/main_chat_kernel.rs`
- `src-tauri/src/main_chat_context_loader.rs`
- `src-tauri/src/main_chat_hs_runtime.rs`
- `src-tauri/src/main_chat_react_tool_selection.rs`
- `src-tauri/src/main_chat_react_runtime.rs`
- `src-tauri/src/main_chat_react_execution.rs`
- `src-tauri/src/main_chat_runtime_support.rs`
- `src-tauri/src/main_chat_command_surface_eval.rs`
- `src-tauri/src/main_chat_final_gate.rs`
- `src-tauri/src/main_chat_live_provider_harness.rs`
- `src-tauri/src/main_chat_runtime_module_tests.rs`
- `src-tauri/src/single_system_authority_tests.rs`
- `src-tauri/src/main_chat_command_surface_tests.rs`
- `src-tauri/src/main_chat_live_provider_tests.rs`
- `openlife-core/src/agent/main_chat_agent_v1.rs`
- `openlife-core/src/agent/model_router.rs`

### `docs/architecture/life-model.md`

Start with:

- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `openlife-core/src/life_model.rs`
- `openlife-core/src/life_model/patch.rs`
- `openlife-core/src/life_model/patch_store.rs`
- `openlife-core/src/life_model_write_gateway.rs`
- `openlife-core/src/agent/proposal_engine.rs`
- `openlife-core/src/agent/proposal_store.rs`
- `openlife-core/src/agent/proposal_outcome.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`
- `src-tauri/src/life_model_write_gateway.rs`
- `src-tauri/src/life_model_materializer_guard.rs`
- `src-tauri/src/commands/life_model.rs`
- `src-tauri/src/commands/proposal.rs`

### `docs/architecture/governance.md`

Start with:

- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `docs/repository_document_governance.md`
- `openlife-core/src/privacy.rs`
- `openlife-core/src/tool_permissions.rs`
- `openlife-core/src/tool_manifest.rs`
- `openlife-core/src/mcp.rs`
- `openlife-core/src/mcp_audit.rs`
- `openlife-core/src/agent/tool_gateway.rs`
- `openlife-core/src/agent/model_router.rs`
- `src-tauri/src/main_chat_proposal_support.rs`
- `src-tauri/src/main_chat_task_controls.rs`
- `src-tauri/src/main_chat_task_control_tests.rs`
- `src-tauri/src/commands/mcp.rs`
- `src-tauri/src/commands/memory.rs`
- `src-tauri/src/commands/proposal.rs`

### `docs/architecture/memory.md`

Start with:

- `openlife-core/src/memory.rs`
- `openlife-core/src/memory_gateway.rs`
- `openlife-core/src/memory_cache.rs`
- `openlife-core/src/agent/memory_service.rs`
- `openlife-core/src/agent/main_chat_memory_candidate.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`
- `src-tauri/src/memory_gateway.rs`
- `src-tauri/src/main_chat_memory_proposals.rs`
- `src-tauri/src/main_chat_context_loader.rs`
- `src-tauri/src/commands/memory.rs`

### `docs/development/testing.md`

Start with:

- `Cargo.toml`
- `openlife-core/Cargo.toml`
- `src-tauri/Cargo.toml`
- `frontend/package.json`
- `frontend/pnpm-lock.yaml`
- `plans/openlife_single_system_development_preparation.md`
- `plans/openlife_repository_document_inventory.json`
- `plans/openlife_repository_document_link_baseline.json`
- `src-tauri/src/single_system_authority_tests.rs`
- `src-tauri/src/main_chat_runtime_module_tests.rs`
- `src-tauri/src/main_chat_command_surface_tests.rs`
- `src-tauri/src/main_chat_live_provider_tests.rs`

## Claims Prohibited In Phase C

Phase C docs must not state or imply:

- Phase7 complete.
- Main Chat Agent Execution v1 complete.
- live-provider evidence complete.
- `main_chat_runtime_module is green`.
- `run_main_chat_agent_execution_v1_final_acceptance_gate` is shipped, current,
  restored, or required as the docs cleanup fix.
- The deleted old final-acceptance test-owner file is the current test owner.
- Local HTTP OpenAI-compatible proof is external live-provider evidence.
- Stage 1 JSON parse or Stage 2 docs repair is runtime evidence.
- New docs are active authority promotion.
- Old `IntentRouter`, `LayerRouter`, `hermes.rs`,
  `multi_strategy_runtime.rs`, or `runtime_migration_gate.rs` are current
  implementation authority.
- `docs/architecture/`, `docs/development/`, or the product-doc namespace
  existed before Stage3.

## Required Validation Commands

Run these for Stage2C and again after any Phase C doc build:

```sh
git diff --check
cargo fmt --check
python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage2c_pretty.json
python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage2c_pretty.json
python3 - <<'PY'
from pathlib import Path

doc_path = Path("plans/openlife_repository_stage2c_phase_c_readiness_decision.md")
doc = doc_path.read_text()
section = doc.split("## Source-Map Starting Points", 1)[1].split(
    "## Claims Prohibited In Phase C", 1
)[0]
paths = []
for line in section.splitlines():
    line = line.strip()
    if line.startswith("- `") and line.endswith("`"):
        paths.append(line[3:-1])
unique_paths = list(dict.fromkeys(paths))
missing = [path for path in unique_paths if not Path(path).exists()]
print(f"source_map_path_count={len(unique_paths)}")
print(f"missing_count={len(missing)}")
for path in missing:
    print(path)
raise SystemExit(1 if missing else 0)
PY
rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts
cargo test -p openlife-tauri single_system -- --nocapture
cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture
rg -n "Phase7 complete|Main Chat Agent Execution v1 complete|live-provider evidence complete|main_chat_runtime_module is green|run_main_chat_agent_execution_v1_final_acceptance_gate" AGENTS.md README.md CONTRIBUTING.md docs/ARCHITECTURE.md docs/DEV_HANDOVER.md OpenLife_Final_PRD.md plans/README.md
```

Expected interpretation:

- the retired-command shipped/product bridge scan should have no matches and
  exit 1;
- `main_chat_runtime_module` is expected to fail until the inherited Phase7
  runtime-module guard is reconciled;
- the active-doc scan may report blocker or historical contexts, but must not
  find a completion or restoration claim.

## Stage2C Validation Record

Validation rerun during the Stage2C-rework pass after fixing the source-map
hallucination:

| Check | Result | Interpretation |
| --- | --- | --- |
| `git diff --check` | Passed | Stage2C doc edits have no whitespace errors. |
| `cargo fmt --check` | Passed | Rust formatting remains unchanged and valid. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage2c_pretty.json` | Passed | Stage 1 inventory JSON remains parseable. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage2c_pretty.json` | Passed | Stage 1 link baseline JSON remains parseable. |
| Source-map existence check | Passed with `source_map_path_count=62`, `missing_count=0` | Stage2C-rework removed the nonexistent root package manifest source-map entry, used existing package evidence such as `frontend/package.json`, and verified every current source-map path exists. |
| `rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts` | No matches; `rg` exited 1 | The retired command is absent from shipped handler and product bridge surfaces. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests | Single-system authority guards remain green. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed, 24 passed and 2 failed | Inherited blocker remains red. Failures were `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |
| Active-doc scan for `Phase7 complete\|Main Chat Agent Execution v1 complete\|live-provider evidence complete\|main_chat_runtime_module is green\|run_main_chat_agent_execution_v1_final_acceptance_gate` | Non-zero by design: 2 `AGENTS.md` matches | Both matches are retired-command / incomplete-evidence / inherited-blocker contexts. No completion or restoration claim was found. |

The `main_chat_runtime_module` failure details remain:

- `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` panicked at
  `src-tauri/src/main_chat_runtime_module_tests.rs:604` because the guard still
  expects a final acceptance runner to call the reusable final-gate aggregation
  module.
- `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`
  panicked at `src-tauri/src/main_chat_runtime_module_tests.rs:689` while trying
  to read the missing final acceptance tests module.

This validates the Stage2C decision: Phase C may proceed only as docs-only
source-backed explanation with the inherited blocker recorded, not as authority
promotion.
