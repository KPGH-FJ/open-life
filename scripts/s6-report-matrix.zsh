#!/usr/bin/env zsh
set -euo pipefail

repo_root="${0:A:h:h}"
cd "$repo_root"

# This matrix proves controlled source contracts only. Native and external-live
# evidence are separate gates and must never be inferred from this script.
cargo test -p openlife-core --locked report_
cargo test -p openlife-core --locked scope_expanding_steering_is_recorded_blocked_and_never_consumed
cargo test -p openlife-core --locked pending_steering_survives_restart_and_terminal_task_refuses_new_input
cargo test -p openlife-core --locked effect_unknown_never_completes_task
cargo test -p openlife-tauri --locked s3_
cargo test -p openlife-tauri --locked roadshow_cc01_exact_prompt_reads_resource_and_web_then_reviews_one_cited_report
cargo test -p openlife-tauri --locked cc01_missing_bound_document_fails_before_web_or_provider_dispatch
cargo test -p openlife-tauri --locked web_backed_generated_artifact_retries_citation_once_before_staging
cargo test -p openlife-tauri --locked generated_artifact_blocks_when_required_field_is_missing_twice
cargo test -p openlife-tauri --locked inline_report_approval_rejects_wrong_task_owner_before_materialization
cargo test -p openlife-tauri --locked concurrent_exact_duplicate_has_one_execution_owner_message_task_and_run
cargo test -p openlife-tauri --locked concurrency_limit_rejects_before_message_task_or_run_mutation
cargo test -p openlife-tauri --locked roadshow_rc08_exact_prompt_cancels_locally_without_late_commit_then_retries_once
cargo test -p openlife-tauri --locked accepted_report_proposal_materializes_the_preexisting_canonical_artifact
cargo test -p openlife-tauri --locked artifact_restart_recovers_staged_bytes_without_blind_redispatch
cargo test -p openlife-tauri --locked tasks_view_model_projects_canonical_report_items_and_artifact_delivery
