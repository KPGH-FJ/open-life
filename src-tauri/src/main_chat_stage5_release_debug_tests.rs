use crate::main_chat_stage5_release_debug::{
    classify_main_chat_stage5_failure, create_main_chat_internal_issue_report_with_store_root,
    delete_main_chat_debug_bundle_from_root,
    evaluate_main_chat_stage5_release_debug_preflight_with_state,
    export_main_chat_agent_debug_bundle_with_store_root, get_main_chat_debug_bundle_from_root,
    list_main_chat_debug_bundles_from_root,
    run_main_chat_stage5_release_debug_report_with_store_root, MainChatStage5IssueReportInput,
    MainChatStage5UiEvidence,
};
use openlife_core::llm::ChatMessage;
use tempfile::TempDir;

#[tokio::test]
async fn main_chat_stage5_preflight_is_read_only_and_reports_named_environment_blockers() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut cfg = state.config.lock().await;
        cfg.llm.provider = "openai".into();
        cfg.llm.openai_key.clear();
        cfg.system.network_policy.enabled = false;
    }

    let preflight = evaluate_main_chat_stage5_release_debug_preflight_with_state(&state)
        .await
        .expect("stage5 preflight should return metadata-safe blockers instead of failing");

    assert_eq!(
        preflight.report_kind,
        "main_chat_stage5_release_debug_preflight"
    );
    assert_eq!(preflight.schema_version, "stage5-preflight-v1");
    assert!(!preflight.external_provider_invoked_by_default);
    assert!(!preflight.model_invoked);
    assert!(!preflight.direct_writes_executed);
    assert!(!preflight.provider.key_present);
    assert!(preflight
        .blockers
        .contains(&"provider_api_key_missing".to_string()));
    assert_eq!(
        preflight.failure.class, "environment_preflight_failure",
        "missing setup must not be classified as model quality failure"
    );
}

#[tokio::test]
async fn main_chat_stage5_debug_bundle_and_issue_artifacts_are_redacted_reloadable_and_deletable() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = scheduler
            .clone()
            .with_scripted_generation_response("stage5 direct answer response");
    }
    let response = crate::main_chat_send::send_message_with_state(
        "stage5-debug-session".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "hello stage5 debug export".into(),
        }],
        None,
        &state,
    )
    .await
    .expect("direct answer fixture should create a task/run");
    let task_session_id = response
        .agent_ingress
        .and_then(|ingress| ingress.agent_task_session_id)
        .expect("task session id must exist");
    let run_id = response.run_id.expect("run id must exist");
    let store_root = TempDir::new().unwrap();

    let bundle = export_main_chat_agent_debug_bundle_with_store_root(
        &state,
        &store_root.path().join("app-data"),
        task_session_id.clone(),
        Some("DBG5-04".into()),
        Some("tester-alpha".into()),
        Some(MainChatStage5UiEvidence {
            frontend_route: "/chat".into(),
            surface: "AgentControlPlane".into(),
            visible_control_labels: vec![
                "Export debug bundle".into(),
                "Create issue report".into(),
            ],
            task_session_id: task_session_id.clone(),
            backend_snapshot_id: Some("snapshot-stage5".into()),
            timestamp: "2026-06-20T00:00:00Z".into(),
            dom_digest: Some("sha256:ui".into()),
            screenshot_digest: None,
        }),
    )
    .await
    .expect("bundle export should succeed");

    assert_eq!(bundle.schema_version, "stage5-debug-bundle-v1");
    assert_eq!(bundle.task.task_session_id, task_session_id);
    assert_eq!(bundle.task.run_id.as_deref(), Some(run_id.as_str()));
    assert_eq!(bundle.redaction.mode, "metadata_safe");
    assert!(!bundle.redaction.raw_content_included);
    assert!(bundle.artifact.byte_size > 0);
    assert!(bundle
        .artifact
        .storage_alias
        .starts_with("stage5/debug_bundles/"));
    assert!(!bundle
        .artifact
        .storage_alias
        .contains(store_root.path().to_string_lossy().as_ref()));

    let serialized = serde_json::to_string(&bundle).unwrap();
    assert!(!serialized.contains("hello stage5 debug export"));
    assert!(!serialized.contains("stage5 direct answer response"));

    let listed = list_main_chat_debug_bundles_from_root(&store_root.path().join("app-data"))
        .expect("list after refresh should load bundle metadata");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].artifact_id, bundle.artifact.artifact_id);
    let bundle_path = store_root
        .path()
        .join("app-data")
        .join(&bundle.artifact.storage_alias);
    assert_eq!(
        std::fs::metadata(&bundle_path).unwrap().len() as usize,
        bundle.artifact.byte_size,
        "artifact byte size must describe the stored JSON artifact"
    );
    let reloaded = get_main_chat_debug_bundle_from_root(
        &store_root.path().join("app-data"),
        &bundle.bundle_id,
    )
    .expect("get after refresh should return the stored bundle");
    assert_eq!(reloaded.bundle_id, bundle.bundle_id);

    let issue = create_main_chat_internal_issue_report_with_store_root(
        &state,
        &store_root.path().join("app-data"),
        MainChatStage5IssueReportInput {
            scenario_id: "DBG5-19".into(),
            reviewer_id: "tester-alpha".into(),
            status: "fail".into(),
            task_session_id: Some(bundle.task.task_session_id.clone()),
            run_id: bundle.task.run_id.clone(),
            bundle_id: Some(bundle.bundle_id.clone()),
            failure_class: Some(bundle.failure.class.clone()),
            notes: Some("Observed failure. Authorization: Bearer sk-stage5-secret".into()),
            preflight_only_missing_task_reason: None,
        },
    )
    .await
    .expect("task-attached issue report should save with redacted notes");

    assert_eq!(issue.schema_version, "stage5-issue-report-v1");
    assert_eq!(
        issue.task_session_id.as_deref(),
        Some(bundle.task.task_session_id.as_str())
    );
    assert_eq!(issue.run_id.as_deref(), bundle.task.run_id.as_deref());
    assert_eq!(issue.bundle_id.as_deref(), Some(bundle.bundle_id.as_str()));
    assert!(
        issue.notes_preview.is_none(),
        "unsafe notes preview must be dropped"
    );
    assert!(issue.artifact.byte_size > 0);

    let issue_json = serde_json::to_string(&issue).unwrap();
    assert!(!issue_json.contains("sk-stage5-secret"));
    assert!(!issue_json.contains("Authorization"));

    let mismatched_issue = create_main_chat_internal_issue_report_with_store_root(
        &state,
        &store_root.path().join("app-data"),
        MainChatStage5IssueReportInput {
            scenario_id: "DBG5-19".into(),
            reviewer_id: "tester-alpha".into(),
            status: "fail".into(),
            task_session_id: Some("other-task".into()),
            run_id: bundle.task.run_id.clone(),
            bundle_id: Some(bundle.bundle_id.clone()),
            failure_class: Some(bundle.failure.class.clone()),
            notes: None,
            preflight_only_missing_task_reason: None,
        },
    )
    .await
    .expect_err("issue reports must not attach a bundle to the wrong task");
    assert_eq!(mismatched_issue, "stage5_issue_task_session_id_mismatch");

    let unsafe_scenario = create_main_chat_internal_issue_report_with_store_root(
        &state,
        &store_root.path().join("app-data"),
        MainChatStage5IssueReportInput {
            scenario_id: " DBG5-19".into(),
            reviewer_id: "tester-alpha".into(),
            status: "blocked_by_environment".into(),
            task_session_id: None,
            run_id: None,
            bundle_id: None,
            failure_class: Some("environment_preflight_failure".into()),
            notes: None,
            preflight_only_missing_task_reason: Some("preflight blocked before task".into()),
        },
    )
    .await
    .expect_err("metadata-safe identity fields must reject wrapping whitespace");
    assert_eq!(unsafe_scenario, "stage5_issue_scenario_id_unsafe");

    assert!(get_main_chat_debug_bundle_from_root(
        &store_root.path().join("app-data"),
        "../stage5-bundle-mock"
    )
    .expect_err("artifact ids must not allow path traversal")
    .contains("stage5_artifact_id_unsafe"));
    assert!(delete_main_chat_debug_bundle_from_root(
        &store_root.path().join("app-data"),
        "../stage5-bundle-mock"
    )
    .expect_err("delete must use the same strict artifact id validation")
    .contains("stage5_artifact_id_unsafe"));

    let preflight_only_issue = create_main_chat_internal_issue_report_with_store_root(
        &state,
        &store_root.path().join("app-data"),
        MainChatStage5IssueReportInput {
            scenario_id: "DBG5-03".into(),
            reviewer_id: "tester-alpha".into(),
            status: "blocked_by_environment".into(),
            task_session_id: None,
            run_id: None,
            bundle_id: None,
            failure_class: Some("environment_preflight_failure".into()),
            notes: Some("Provider key missing in preflight.".into()),
            preflight_only_missing_task_reason: Some("preflight blocked before task".into()),
        },
    )
    .await
    .expect("preflight-only issue report should not require task/run/bundle ids");
    assert!(preflight_only_issue.task_session_id.is_none());
    assert!(preflight_only_issue.run_id.is_none());
    assert!(preflight_only_issue.bundle_id.is_none());
    assert!(preflight_only_issue
        .blockers
        .contains(&"stage5_issue_task_run_missing".to_string()));
    assert!(preflight_only_issue
        .blockers
        .contains(&"stage5_issue_bundle_missing_preflight_only".to_string()));

    let report = run_main_chat_stage5_release_debug_report_with_store_root(
        &state,
        &store_root.path().join("app-data"),
    )
    .await
    .expect("report should credit stored bundle and issue artifacts");
    assert!(report
        .rows
        .iter()
        .any(|row| row.id == "DBG5-04" && row.status == "passed"));
    assert!(report.rows.iter().any(|row| {
        row.id == "DBG5-05"
            && row.status == "blocked"
            && row
                .blockers
                .contains(&"stage5_read_action_debug_bundle_missing".to_string())
    }));
    assert!(report.rows.iter().any(|row| {
        row.id == "DBG5-07"
            && row.status == "blocked"
            && row
                .blockers
                .contains(&"stage5_mcp_read_debug_bundle_missing".to_string())
    }));
    assert!(report.rows.iter().any(|row| {
        row.id == "DBG5-10"
            && row.status == "blocked"
            && row
                .blockers
                .contains(&"stage5_memory_proposal_debug_bundle_missing".to_string())
    }));
    assert!(report
        .rows
        .iter()
        .any(|row| row.id == "DBG5-19" && row.status == "passed"));
    assert!(report
        .rows
        .iter()
        .any(|row| row.id == "DBG5-24" && row.status == "passed"));
    assert!(report.bundle_ids.contains(&bundle.bundle_id));
    assert!(report.issue_artifact_ids.contains(&issue.report_id));
    assert!(!report.readiness_claim);

    assert!(delete_main_chat_debug_bundle_from_root(
        &store_root.path().join("app-data"),
        &bundle.bundle_id
    )
    .expect("delete should remove bundle artifact"));
    assert!(
        list_main_chat_debug_bundles_from_root(&store_root.path().join("app-data"))
            .expect("list after delete should still be available")
            .is_empty()
    );
}

#[tokio::test]
async fn main_chat_stage5_report_covers_dbg5_rows_without_claiming_readiness() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let store_root = TempDir::new().unwrap();

    let report =
        run_main_chat_stage5_release_debug_report_with_store_root(&state, store_root.path())
            .await
            .expect("stage5 report should aggregate pass/block rows");

    assert_eq!(report.report_kind, "main_chat_stage5_release_debug");
    assert_eq!(report.scenario_count, 24);
    assert!(report.not_a_readiness_gate);
    assert!(!report.readiness_claim);
    assert!(report.stage2_readiness_preserved);
    assert!(report
        .rows
        .iter()
        .any(|row| row.id == "DBG5-21" && row.status == "passed"));
    assert!(report
        .rows
        .iter()
        .any(|row| row.id == "DBG5-22" && row.status == "passed"));
    assert!(report
        .rows
        .iter()
        .any(|row| row.id == "DBG5-12" && row.status == "passed"));
    assert!(report
        .rows
        .iter()
        .any(|row| row.id == "DBG5-13" && row.status == "passed"));
    assert!(report.rows.iter().any(|row| row.id == "DBG5-24"));
    assert!(report.managed_knowledge_eval.isolated_eval_app_state);
    assert!(report.managed_knowledge_eval.temp_workspace);
    assert!(!report.managed_knowledge_eval.real_workspace_write_executed);
    assert!(report.managed_knowledge_eval.user_write_completed);
    assert!(report.managed_knowledge_eval.memory_rollback_completed);
    assert_eq!(
        report.passed_scenario_count + report.blocked_scenario_count,
        report.scenario_count
    );
}

#[test]
fn main_chat_stage5_failure_taxonomy_maps_tool_and_redaction_failures() {
    let tool = classify_main_chat_stage5_failure(&["model_selected_disallowed_tool".into()]);
    assert_eq!(tool.class, "tool_selection_failure");
    assert_eq!(tool.recoverability, "needs_developer_fix");

    let redaction = classify_main_chat_stage5_failure(&["secret_detected_in_artifact".into()]);
    assert_eq!(redaction.class, "redaction_failure");
    assert_eq!(redaction.severity, "p0");

    let knowledge = classify_main_chat_stage5_failure(&["MEMORY.md rollback failed".into()]);
    assert_eq!(knowledge.class, "knowledge_asset_failure");
}
