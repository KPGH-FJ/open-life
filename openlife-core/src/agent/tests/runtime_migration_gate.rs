use crate::agent::ReasoningTrace;
use crate::agent::{
    evaluate_runtime_migration_gate, AgentAction, AgentRun, AgentRunStatus,
    RuntimeMigrationGateInput,
};
use chrono::Utc;
use serde_json::json;

fn healthy_preview_run() -> AgentRun {
    let mut run = AgentRun::new_chat_run("session-gate", "raw prompt should be cleared");
    run.status = AgentRunStatus::Completed;
    run.user_input = None;
    run.reasoning_strategy = Some("multi_strategy_preview".into());
    run.output_preview = Some("Multi-strategy preview: react / allow".into());
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(json!({
            "previewRuntime": "multi_strategy",
            "strategyKind": "react",
            "payloadKind": "react",
            "governanceDecisionKind": "allow",
            "metadataSafe": true,
            "innerRunId": "inner-react-run",
            "writeControl": {
                "declaredWriteStepCount": 0,
                "proposalRequiredStepCount": 0,
                "blockedStepCount": 0
            }
        })),
        output: Some("multi_strategy_preview".into()),
        ..ReasoningTrace::default()
    });
    run.finished_at = Some(Utc::now());
    run
}

fn gate_input(preview_run: Option<&AgentRun>) -> RuntimeMigrationGateInput<'_> {
    RuntimeMigrationGateInput {
        default_chat_uses_multi_strategy: false,
        preview_run,
        fallback_available: true,
    }
}

#[test]
fn runtime_migration_gate_passes_for_healthy_preview_audit() {
    let run = healthy_preview_run();

    let report = evaluate_runtime_migration_gate(gate_input(Some(&run)));

    assert!(report.default_chat_unchanged);
    assert!(report.preview_path_healthy);
    assert!(report.metadata_safe_trace_ready);
    assert!(report.fallback_available);
    assert!(report.no_external_writes);
    assert!(report.proposal_first_preserved);
    assert!(report.blocking_reasons.is_empty());
}

#[test]
fn runtime_migration_gate_blocks_missing_preview_fallback_and_metadata_safe_trace() {
    let mut run = healthy_preview_run();
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(json!({
            "previewRuntime": "multi_strategy",
            "metadataSafe": false,
            "rawPrompt": "Alice raw prompt alice@example.com",
            "writeControl": {
                "declaredWriteStepCount": 1,
                "proposalRequiredStepCount": 0,
                "blockedStepCount": 0
            }
        })),
        ..ReasoningTrace::default()
    });
    run.user_input = Some("Alice raw prompt alice@example.com".into());

    let report = evaluate_runtime_migration_gate(RuntimeMigrationGateInput {
        default_chat_uses_multi_strategy: false,
        preview_run: Some(&run),
        fallback_available: false,
    });

    assert!(!report.metadata_safe_trace_ready);
    assert!(!report.fallback_available);
    assert!(!report.no_external_writes);
    assert!(!report.proposal_first_preserved);
    assert!(report
        .blocking_reasons
        .contains(&"preview_trace_not_metadata_safe".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"fallback_unavailable".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"external_write_risk_detected".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"proposal_first_not_preserved".to_string()));
}

#[test]
fn runtime_migration_gate_blocks_when_preview_audit_is_missing() {
    let mut run = healthy_preview_run();
    run.reasoning_trace = None;

    let report = evaluate_runtime_migration_gate(gate_input(Some(&run)));

    assert!(!report.preview_path_healthy);
    assert!(!report.metadata_safe_trace_ready);
    assert!(report
        .blocking_reasons
        .contains(&"preview_audit_missing".to_string()));
}

#[test]
fn runtime_migration_gate_marks_default_chat_replacement_as_blocking() {
    let run = healthy_preview_run();

    let report = evaluate_runtime_migration_gate(RuntimeMigrationGateInput {
        default_chat_uses_multi_strategy: true,
        preview_run: Some(&run),
        fallback_available: true,
    });

    assert!(!report.default_chat_unchanged);
    assert!(report
        .blocking_reasons
        .contains(&"default_chat_replaced".to_string()));
}

#[test]
fn runtime_migration_gate_does_not_execute_tools_or_external_writes() {
    let mut run = healthy_preview_run();
    run.actions.push(AgentAction {
        id: "action-1".into(),
        action_type: "tool_call".into(),
        target: Some("calendar.create_event".into()),
        input: json!({"title": "private event"}),
        output: None,
        status: "completed".into(),
        permission_decision: None,
        started_at: None,
        finished_at: None,
        error: None,
        timestamp: Utc::now(),
        tool_scope: None,
    });
    run.tool_call_count = 1;

    let report = evaluate_runtime_migration_gate(gate_input(Some(&run)));

    assert!(!report.no_external_writes);
    assert!(report
        .blocking_reasons
        .contains(&"external_write_risk_detected".to_string()));
}

#[test]
fn runtime_migration_gate_output_is_metadata_safe() {
    let mut run = healthy_preview_run();
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(json!({
            "previewRuntime": "multi_strategy",
            "metadataSafe": false,
            "rawMemoryContext": "Alice private memory alice@example.com"
        })),
        ..ReasoningTrace::default()
    });

    let report = evaluate_runtime_migration_gate(gate_input(Some(&run)));
    let serialized = serde_json::to_string(&report).unwrap();

    assert!(!serialized.contains("Alice"));
    assert!(!serialized.contains("alice@example.com"));
    assert!(!serialized.contains("rawMemoryContext"));
    assert!(!serialized.contains("raw prompt"));
}
