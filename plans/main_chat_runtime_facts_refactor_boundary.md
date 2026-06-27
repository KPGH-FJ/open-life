# Main Chat Runtime Facts Refactor Boundary

> Date: 2026-06-25
> Status: preparation artifact before Runtime Facts / Kernel boundary refactor
> Parent: `plans/main_chat_next_6_steps_master_spec.md`

## 1. Purpose

Runtime Facts now has the right direction but too much implementation mass in a
single module. This document defines the target boundary before any Step 4
refactor starts.

The goal is not file-size reduction by itself. The goal is to make fact source
authority, resolution, reply formatting, kernel consumption, and eval reporting
separable so later facts do not become another large catch-all subsystem.

## 2. Current Risk

Current `src-tauri/src/main_chat_runtime_facts.rs` contains:

- fact keys and constants;
- intent classifiers;
- source snapshots;
- answer resolvers;
- reply text formatting;
- report structs;
- slice runner setup;
- response evidence extraction;
- scenario-specific pass/fail assertions.

This makes the module easy to append to and hard to reason about. The risk is
that Runtime Facts becomes a new central mudball even though it was introduced
to reduce model/prompt ambiguity.

## 3. Target Modules

The exact filenames may change, but responsibilities must stay separate:

| Target responsibility | Allowed contents | Forbidden contents |
| --- | --- | --- |
| `runtime_facts_contract` | keys, source/authority/freshness enums, `MainChatRuntimeFactAnswer`, `MainChatRuntimeFactBinding` | scenario runners, UI-specific prose |
| `runtime_facts_registry` | static source registry mappings used by code | natural-language classifiers |
| `runtime_facts_resolver` | top-level routing from user intent to typed fact answer | test fixture setup, report aggregation |
| `runtime_facts_clock` | clock classifier, clock snapshot, clock reply | provider/tool/self-state logic |
| `runtime_facts_provider_route` | provider route snapshot and reply | tool availability or live-provider harness |
| `runtime_facts_tool_availability` | web/MCP/write availability snapshot and reply | active network probing in normal chat |
| `runtime_facts_agent_self_state` | task/session/run/action/proposal snapshots and replies | generic command-surface report building |
| `runtime_facts_eval` | slice report construction and scenario evidence extraction | production resolver logic not needed by eval |

## 4. Kernel Boundary

`MainChatKernel` may depend on:

- typed resolver entry points;
- `MainChatRuntimeFactAnswer`;
- fact-generation metadata merging.

`MainChatKernel` must not depend on:

- scenario IDs;
- slice report structs;
- eval-only fixture setup;
- fact-specific source registry internals;
- UI display row details.

## 5. Refactor Sequence

1. Move contract types and constants first.
2. Move clock implementation without behavior change.
3. Move provider-route implementation without behavior change.
4. Move tool-availability implementation without behavior change.
5. Move agent-self-state implementation without behavior change.
6. Move eval/report/scenario code last.
7. Add module-boundary tests after the first split and keep them updated.

Each move must preserve the same test behavior before the next move starts.

## 6. Invariants

The refactor must not change:

- `sourceType=runtime_fact`;
- existing runtime fact keys;
- existing metadata field names;
- no-model/no-tool/no-write/no-legacy assertions;
- RF-01 through RF-19 pass/fail outcomes;
- `runtime_facts_ready=false` semantics until the full-layer contract passes;
- source registry and UI contract version labels unless those contracts change.

## 7. Required Tests

Minimum after each refactor slice:

```bash
cargo fmt --check
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture
git diff --check
```

After the full refactor:

```bash
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
```

## 8. Stop Conditions

Stop the refactor if:

- a moved module needs to import eval scenario code;
- production code starts checking RF IDs;
- the kernel imports a slice report type;
- a fact-specific module needs raw UI component details;
- a behavior changes without a corresponding acceptance matrix update;
- a catch-all resolver starts using broad natural-language regexes for
  unrelated fact categories.
