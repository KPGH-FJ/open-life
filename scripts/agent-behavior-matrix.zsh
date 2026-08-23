#!/usr/bin/env zsh
set -euo pipefail

repo_root="${0:A:h:h}"
cd "$repo_root"

# Current controlled Agent behavior matrix. Each row calls a production owner
# through a focused product test. Passing this script proves controlled
# behavior only; it is never native or external-live evidence.
for retired_owner in \
  openlife-core/src/agent/main_chat_agent_v1.rs \
  openlife-core/src/agent/main_chat_governance_intent.rs \
  openlife-core/src/agent/life_model_explicit_read.rs \
  openlife-core/src/calendar.rs \
  openlife-core/src/state_store.rs \
  openlife-core/src/tasks.rs \
  src-tauri/src/main_chat_kernel.rs \
  src-tauri/src/main_chat_source_bound.rs \
  src-tauri/src/main_chat_tool_observation.rs
do
  if [[ -e "$retired_owner" ]]; then
    print -u2 "retired Agent owner returned to the source graph: $retired_owner"
    exit 65
  fi
done

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
run_tauri_row work_web_observation_refinement \
  web_agent_can_refine_an_insufficient_search_inside_the_same_run
run_tauri_row work_independent_semantic_verification \
  independent_semantic_verification_returns_a_gap_to_the_same_agent_run
run_tauri_row work_direct_artifact_semantic_verification \
  direct_artifact_cannot_bypass_independent_semantic_verification
run_tauri_row work_semantic_limitation_contract \
  semantic_verification_contract_rejects_self_contradictory_outcomes
run_core_row work_web_overlapping_result_deduplication \
  iterative_search_deduplicates_overlapping_urls_without_losing_authority
run_core_row provider_untrusted_context_boundary \
  untrusted_context_is_json_data_and_cannot_mint_a_runtime_instruction_block
run_tauri_row work_natural_research_artifact_semantic_grounding_chinese \
  natural_research_and_markdown_request_executes_web_and_stages_a_real_artifact
run_tauri_row work_fetched_source_authority \
  fetched_web_evidence_excludes_search_only_results_from_final_citation_authority
run_tauri_row work_optional_capability_recovery \
  unavailable_optional_web_does_not_tax_required_local_work
run_core_row work_terminal_optional_attempt_completion \
  general_completion_blocks_unsettled_items_but_preserves_terminal_failed_attempts
run_tauri_row work_conflicting_fetched_source_authority \
  conflicting_fetched_sources_remain_distinct_grounding_authorities
run_tauri_row work_artifact_source_validation \
  web_backed_markdown_artifact_rejects_unobserved_sources_and_gets_backend_footer
run_tauri_row work_mixed_report_chinese \
  document_and_web_evidence_flow_into_one_reviewed_work_artifact
run_tauri_row work_selected_skill \
  selected_executable_skill_is_a_bounded_canonical_observation
run_tauri_row work_read_only_mcp \
  registered_stdio_mcp_read_uses_canonical_attempt_and_receipt
run_tauri_row work_plan_item_chinese \
  planning_request_is_a_plan_item_inside_the_work_run
run_tauri_row work_web_evidence_diversity \
  work_planner_contract_values_evidence_independence_over_a_fixed_page_count
run_tauri_row work_negative_output_constraints_chinese \
  negated_file_terms_in_plan_request_complete_as_an_answer
run_core_row work_steering \
  consumed_steering_advances_run_current_plan_and_revision_history_together
run_tauri_row work_checkpoint_accept \
  generated_artifact_uses_one_work_task_through_review_and_materialization
run_tauri_row office_artifact_content_verification \
  office_artifacts_share_one_canonical_review_and_materialization_spine
run_tauri_row work_project_without_folder_artifact \
  project_without_workspace_root_uses_managed_storage_without_review
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
run_tauri_row effect_unknown \
  effect_unknown_requires_attention_and_never_offers_automatic_retry
run_tauri_row work_provider_failure \
  provider_failure_terminalizes_work_without_a_final_result
run_core_row work_provider_budget_boundary \
  lifecycle_budget_rejection_is_not_reported_as_a_provider_failure
run_core_row restart_recovery \
  recovery_interrupts_open_general_run_and_retry_adds_a_new_run
run_core_row conversation_restart_scope \
  restart_marks_only_incomplete_turns_interrupted
run_core_row conversation_schema_migration \
  v1_store_migrates_projects_and_repeated_steering_without_losing_history
run_core_row task_runtime_schema_migration \
  general_tasks_are_distinct_inside_one_conversation_and_runs_are_canonical
run_tauri_row fresh_profile_restart_credentials \
  credential_access_recovery_restores_existing_keys_without_writes_or_secret_output
run_core_row selected_route_search_credential \
  automatic_search_reuses_only_a_supported_selected_official_provider_route
run_tauri_row selected_openrouter_runtime_search_credential \
  automatic_openrouter_search_reuses_the_exact_selected_route
run_tauri_row personal_intelligence_ports \
  personal_intelligence_ports::tests
run_tauri_row product_diagnostics_budget \
  product_diagnostics_reads_two_hundred_canonical_tasks_inside_controlled_budget
run_row workbench_user_visible_states \
  corepack pnpm --dir frontend test -- --run \
    src/app/ProductWorkbench.test.tsx \
    src/app/WorkspaceRoute.test.tsx \
    src/features/conversation/useConversationController.test.tsx

print "AGENT_MATRIX_EVIDENCE_LEVEL=controlled"
print "AGENT_MATRIX_RESULT=pass"
