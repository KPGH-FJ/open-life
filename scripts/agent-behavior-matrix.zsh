#!/usr/bin/env zsh
set -euo pipefail

repo_root="${0:A:h:h}"
cd "$repo_root"

# Current controlled Agent behavior matrix. Each row calls a production owner
# through a focused product test. Passing this script proves controlled
# behavior only; it is never native or external-live evidence.
run_row() {
  local row="$1"
  shift
  print "AGENT_MATRIX_ROW_START=$row"
  "$@"
  print "AGENT_MATRIX_ROW_PASS=$row"
}

run_tauri_row() {
  local row="$1"
  local filter="$2"
  local test_list
  test_list="$(cargo test -p openlife-tauri --locked "$filter" -- --list)"
  if [[ "$test_list" != *"$filter"* ]]; then
    print -u2 "missing OpenLife Tauri test for matrix row: $row ($filter)"
    exit 65
  fi
  run_row "$row" cargo test -p openlife-tauri --locked "$filter"
}

run_core_row() {
  local row="$1"
  local filter="$2"
  local test_list
  test_list="$(cargo test -p openlife-core --locked "$filter" -- --list)"
  if [[ "$test_list" != *"$filter"* ]]; then
    print -u2 "missing openlife-core test for matrix row: $row ($filter)"
    exit 65
  fi
  run_row "$row" cargo test -p openlife-core --locked "$filter"
}

run_tauri_row chat_bilingual_direct_answer \
  chinese_and_english_chat_share_the_same_conversation_turn_contract
run_tauri_row chat_replay \
  completed_turn_replays_without_a_second_provider_request
run_tauri_row chat_provider_failure \
  unavailable_provider_does_not_create_a_partial_turn
run_tauri_row work_direct_answer \
  work_owns_task_run_attempt_and_final_result
run_tauri_row work_document_chinese \
  task_bound_document_read_uses_exact_turn_and_canonical_tool_lifecycle
run_tauri_row work_web_chinese \
  governed_web_read_is_tool_attempt_observation_and_cited_final_result
run_tauri_row work_mixed_report_chinese \
  document_and_web_evidence_flow_into_one_reviewed_work_artifact
run_tauri_row work_selected_skill \
  selected_executable_skill_is_a_bounded_canonical_observation
run_tauri_row work_read_only_mcp \
  registered_stdio_mcp_read_uses_canonical_attempt_and_receipt
run_tauri_row work_plan_item_chinese \
  planning_request_is_a_plan_item_inside_the_work_run
run_tauri_row work_negative_output_constraints_chinese \
  negated_file_terms_in_plan_request_complete_as_an_answer
run_core_row work_steering \
  pending_steering_survives_restart_and_terminal_task_refuses_new_input
run_tauri_row work_checkpoint_accept \
  generated_artifact_uses_one_work_task_through_review_and_materialization
run_core_row work_checkpoint_reject \
  rejected_review_blocks_the_same_task_and_checkpoint_without_delivery
run_tauri_row work_cancel \
  active_work_cancel_terminalizes_turn_run_item_and_attempt
run_tauri_row work_retry \
  failed_work_retries_as_a_new_run_of_the_same_task
run_core_row artifact_verification_undo \
  governed_artifact_undo_is_independent_and_receipt_bound
run_tauri_row blocked_scope \
  retry_refuses_a_changed_project_scope_and_records_attention
run_core_row effect_unknown \
  effect_unknown_never_completes_task
run_tauri_row work_provider_failure \
  provider_failure_terminalizes_work_without_a_final_result
run_core_row restart_recovery \
  recovery_interrupts_open_general_run_and_retry_adds_a_new_run
run_core_row conversation_restart_scope \
  restart_marks_only_incomplete_turns_interrupted
run_core_row conversation_schema_migration \
  v1_store_migrates_projects_and_repeated_steering_without_losing_history
run_core_row task_runtime_schema_migration \
  v1_runtime_migrates_without_rewriting_legacy_execution_facts
run_tauri_row fresh_profile_restart_credentials \
  credential_access_recovery_restores_existing_keys_without_writes_or_secret_output
run_tauri_row personal_intelligence_ports \
  personal_intelligence_ports::tests
run_tauri_row product_diagnostics_budget \
  product_diagnostics_reads_two_hundred_canonical_tasks_inside_controlled_budget
run_row workbench_user_visible_states \
  corepack pnpm --dir frontend test -- --run \
    src/ui/journeys/productWorkbench/ProductWorkbenchJourney.test.tsx \
    src/ui/journeys/governedAction/GovernedActionJourney.test.tsx \
    src/ui/journeys/governedAction/useWorkspaceConversation.test.tsx

print "AGENT_MATRIX_EVIDENCE_LEVEL=controlled"
print "AGENT_MATRIX_RESULT=pass"
