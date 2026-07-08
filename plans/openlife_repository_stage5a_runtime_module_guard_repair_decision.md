# Stage5A Runtime Module Guard Repair Decision

Date: 2026-07-07
Status: decision record only; not a Phase7 completion claim

## Decision

`src-tauri/src/main_chat_runtime_module_tests.rs` should verify the current
Phase7 file ownership instead of requiring the retired final acceptance
test/command owner to exist.

Current ownership:

- `src-tauri/src/main_chat_final_gate.rs` owns reusable final-gate aggregation,
  required-evidence labels, and blocked/completed live-provider harness report
  builders.
- `src-tauri/src/main_chat_live_provider_harness.rs` owns live-provider harness
  execution and uses the reusable blocked-report builder when preflight fails.
- `src-tauri/src/main_chat_live_provider_tests.rs` owns current live-provider
  harness contract tests and uses the reusable completed-report builder for
  credited-report fixtures.
- `src-tauri/src/commands/agent_runtime/mod.rs` must not restore the retired
  final acceptance command surface.

## Non-Goals

- Do not restore `src-tauri/src/main_chat_final_acceptance_tests.rs`.
- Do not restore or re-expose
  `run_main_chat_agent_execution_v1_final_acceptance_gate`.
- Do not claim Phase7 complete.
- Do not claim Main Chat Agent Execution v1 complete.
- Do not count local HTTP provider proof as external live-provider evidence.

## Guard Repair

The Stage5A guard now checks:

- reusable final-gate aggregation still lives in `main_chat_final_gate.rs`;
- completed live-provider report fixtures use the reusable final-gate helper
  from the current live-provider test owner;
- the retired final acceptance test file remains absent;
- `commands/agent_runtime` does not regain the retired final acceptance runner.

This preserves Stage4C expected-absent closure while keeping the reusable
production helper boundary explicit.
