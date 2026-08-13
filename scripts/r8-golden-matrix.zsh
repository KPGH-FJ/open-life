#!/usr/bin/env zsh
set -euo pipefail

repo_root="${0:A:h:h}"
cd "$repo_root"

# This matrix proves controlled source and product-contract behavior only.
# Exact-native and external-live evidence are separate R8 gates and must never
# be inferred from this script.
run_row() {
  local row="$1"
  shift
  print "R8_MATRIX_ROW_START=$row"
  "$@"
  print "R8_MATRIX_ROW_PASS=$row"
}

run_row chat_canonical \
  cargo test -p openlife-tauri --locked canonical_chat_runtime::tests
run_row work_canonical \
  cargo test -p openlife-tauri --locked canonical_work_runtime::tests
run_row task_steering_restart \
  cargo test -p openlife-core --locked pending_steering_survives_restart_and_terminal_task_refuses_new_input
run_row task_artifact_review_undo \
  cargo test -p openlife-core --locked governed_artifact_undo_is_independent_and_receipt_bound
run_row task_recovery_retry \
  cargo test -p openlife-core --locked recovery_interrupts_open_general_run_and_retry_adds_a_new_run
run_row conversation_restart_scope \
  cargo test -p openlife-core --locked restart_marks_only_incomplete_turns_interrupted
run_row personal_intelligence_ports \
  cargo test -p openlife-tauri --locked personal_intelligence_ports::tests
run_row product_diagnostics_budget \
  cargo test -p openlife-tauri --locked product_diagnostics_reads_two_hundred_canonical_tasks_inside_controlled_budget
run_row frontend_workbench \
  corepack pnpm --dir frontend test -- --run \
    src/ui/journeys/governedAction/useWorkspaceConversation.test.tsx \
    src/ui/journeys/governedAction/GovernedActionJourney.test.tsx \
    src/ui/journeys/productWorkbench/ProductWorkbenchJourney.test.tsx \
    src/ui/journeys/settingsPrivacy/SettingsPrivacyView.test.tsx \
    src/ui/journeys/durableTruth/DurableTruthJourney.test.tsx

print "R8_MATRIX_EVIDENCE_LEVEL=controlled"
print "R8_MATRIX_RESULT=pass"
