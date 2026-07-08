# Testing

## Status

Stage3-A source-backed testing map. This page explains current local validation
commands and their interpretation. It does not turn local/scripted evidence into
external live provider readiness. Stage5A repaired the inherited
runtime-module guard, but that repair is not Phase7 completion, Main Chat Agent
Execution v1 completion, or external live-provider completion.

## Authority

Authority remains with `AGENTS.md`, `plans/README.md`,
`plans/openlife_single_system_deletion_manifest.md`, and
`plans/openlife_single_system_development_preparation.md`. This page is a
developer explainer beneath the active authority docs.

## Last verified

2026-07-07 during Stage3-A source-map reading. The Stage3-A validation record is
kept in `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
after the command set is run.

## Source map

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

## Stage5B Current Guard Status

`cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` now
passes after Stage5A updated the guard to the current Phase7 owner shape. The
current owner shape keeps final-gate aggregation and live-provider report
builders in `src-tauri/src/main_chat_final_gate.rs`, live-provider harness
contract tests in `src-tauri/src/main_chat_live_provider_tests.rs`, and keeps
the retired final acceptance command/test owner absent.

Older Stage2/Stage3/Stage4 validation rows below that record this command as
failed are preserved as original time-point evidence. They are superseded for
current status by the Stage5A run; they must not be read as current truth.

## Stage5B Validation Commands

Run this set after Stage5B status-sync documentation edits:

```sh
git diff --check
cargo fmt --check
python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage5b.json
python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage5b.json
cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture
cargo test -p openlife-tauri single_system -- --nocapture
cargo test -p openlife-tauri main_chat_live_provider -- --nocapture
test ! -f src-tauri/src/main_chat_final_acceptance_tests.rs
rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts || true
```

Expected interpretation: the three targeted Rust suites pass, the old
final-acceptance test owner is absent, and the retired final acceptance command
has no shipped-surface matches. This does not recompute the document link
baseline and does not create external live-provider evidence.

## Workspace Shape

The root `Cargo.toml` is a Cargo workspace with two members:
`src-tauri` and `openlife-core`.

`openlife-core/Cargo.toml` defines the Rust core crate and its storage,
privacy, vector, and async dependencies. `src-tauri/Cargo.toml` defines the
Tauri crate, depends on `openlife-core`, and disables doctests for the library.

The frontend package is under `frontend/`. `frontend/package.json` uses
`pnpm@9.1.0`, React 18, Tauri API packages, Vite, Vitest, Playwright, and
TypeScript. `frontend/pnpm-lock.yaml` is the lockfile for those frontend
dependencies.

## Stage3-A Validation Commands

Run this set after Stage3-A documentation edits:

```sh
git diff --check
cargo fmt --check
python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3a_pretty.json
python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3a_pretty.json
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
rg -n "Phase7 complete|Main Chat Agent Execution v1 complete|live-provider evidence complete|runtime-module guard is fixed by docs cleanup|run_main_chat_agent_execution_v1_final_acceptance_gate" AGENTS.md README.md CONTRIBUTING.md docs/ARCHITECTURE.md docs/DEV_HANDOVER.md OpenLife_Final_PRD.md plans/README.md
cargo test -p openlife-tauri single_system -- --nocapture
cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture
```

## Stage3-B Validation Commands

Run this set after Stage3-B inventory, link-baseline, and document-governance
edits:

```sh
git diff --check
cargo fmt --check
python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3b_pretty.json
python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3b_pretty.json
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
python3 - <<'PY'
import json
from pathlib import Path

baseline = json.loads(
    Path("plans/openlife_repository_document_link_baseline.json").read_text()
)
summary = baseline["summary"]
print(f"missing_local_path_records={summary['missing_local_path_records']}")
print(f"active_doc_broken_links={summary['active_doc_broken_links']}")
print(
    "stage3a_new_document_broken_link_records="
    f"{summary['stage3a_new_document_broken_link_records']}"
)
print(f"uncategorized_broken_records={summary['uncategorized_broken_records']}")
raise SystemExit(
    0
    if summary["stage3a_new_document_broken_link_records"] == 0
    and summary["uncategorized_broken_records"] == 0
    else 1
)
PY
rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts
rg -n "Phase7 complete|Main Chat Agent Execution v1 complete|live-provider evidence complete|runtime-module guard is fixed by docs cleanup|run_main_chat_agent_execution_v1_final_acceptance_gate" AGENTS.md README.md CONTRIBUTING.md docs/ARCHITECTURE.md docs/DEV_HANDOVER.md OpenLife_Final_PRD.md plans/README.md docs/repository_document_governance.md
cargo test -p openlife-tauri single_system -- --nocapture
cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture
```

## Interpretation

`git diff --check` checks patch whitespace only. It is not runtime evidence.

`cargo fmt --check` verifies Rust formatting. Stage3-A should not edit Rust
source, but the check confirms the repo still formats cleanly.

The two `python3 -m json.tool` commands verify that the Stage 1 document
inventory and link-baseline JSON files remain parseable.

The source-map existence check verifies that every Stage2C source-map path
exists. It should print `source_map_path_count=62` and `missing_count=0`.

The retired-command scan over `src-tauri/src/lib.rs`, `src-tauri/src/commands`,
and `frontend/src/tauri.ts` should have no matches and exit 1. That no-match
exit is the desired result for this scan.

The active-doc claim scan may find blocker or historical contexts in active
docs. It must not find a new completion or restoration claim.

`cargo test -p openlife-tauri single_system -- --nocapture` is the current
single-system authority guard set. Stage2C recorded it as green with 17 tests.

`cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` was a
known inherited blocker during Stage2/Stage3/Stage4. Those rows recorded 24
passed and 2 failed around final-gate aggregation and live-provider
completed-report builder ownership. Stage5A supersedes that current status:
the guard now passes by checking the current owner shape without restoring the
retired final acceptance command or old test owner.

## Local, Core, And Live Evidence

Local command-surface tests and local HTTP provider-client proof can demonstrate
ordinary send/stream shape and local provider-client behavior. They do not
count as external live provider completion.

Live provider credit requires the dedicated live harness and final gate evidence
for direct generation, web AgentLoop, registered MCP AgentLoop, and
proposal-permission scenarios. Missing trace, fallback, silent writes,
synthetic/local providers, or malformed evidence remain blockers.
