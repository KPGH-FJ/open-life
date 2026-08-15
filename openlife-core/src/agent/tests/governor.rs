use crate::agent::{
    ExternalWriteGovernanceInput, GovernanceDecisionClassification, GovernanceDecisionKind,
    LifeModelGovernor, MemoryWriteGovernanceInput, RiskLevel, ToolGovernanceInput,
};

#[test]
fn external_write_tool_requires_proposal() {
    let governor = LifeModelGovernor;
    let decision = governor.govern_tool_action(ToolGovernanceInput {
        tool_name: "calendar.create_event".into(),
        action_kind: "write".into(),
        risk_level: RiskLevel::Medium,
        declared_write: true,
    });

    assert_eq!(decision.kind, GovernanceDecisionKind::RequireProposal);
    assert_eq!(decision.risk_level, RiskLevel::Medium);
}

#[test]
fn read_only_tool_is_allowed() {
    let governor = LifeModelGovernor;
    let decision = governor.govern_tool_action(ToolGovernanceInput {
        tool_name: "memory.search".into(),
        action_kind: "search".into(),
        risk_level: RiskLevel::Low,
        declared_write: false,
    });

    assert_eq!(decision.kind, GovernanceDecisionKind::Allow);
}

#[test]
fn unified_governor_report_classifies_core_decision_types_without_raw_payloads() {
    let governor = LifeModelGovernor;

    let read_tool = governor
        .govern_tool_action(ToolGovernanceInput {
            tool_name: "memory.search".into(),
            action_kind: "search".into(),
            risk_level: RiskLevel::Low,
            declared_write: false,
        })
        .to_report();
    assert_eq!(
        read_tool.classification,
        GovernanceDecisionClassification::Allow
    );
    assert!(read_tool.allowed);

    let memory_write = governor
        .govern_memory_write(MemoryWriteGovernanceInput {
            risk_level: RiskLevel::Medium,
            source_run_id: Some("run-memory-write".into()),
            proposal_already_created: false,
        })
        .to_report();
    assert_eq!(
        memory_write.classification,
        GovernanceDecisionClassification::ProposalFirst
    );
    assert!(memory_write.requires_proposal);

    let external_write = governor
        .govern_external_write(ExternalWriteGovernanceInput {
            tool_name: "file.write".into(),
            risk_level: RiskLevel::High,
            source_run_id: Some("run-external-write".into()),
            proposal_already_created: false,
        })
        .to_report();
    assert_eq!(
        external_write.classification,
        GovernanceDecisionClassification::ProposalFirst
    );
    assert!(external_write.requires_proposal);
    assert_eq!(
        external_write.proposal_type.as_deref(),
        Some("external_write_action")
    );

    for report in [read_tool, memory_write, external_write] {
        assert_eq!(report.report_kind, "governor_decision_report");
        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert!(!report.raw_prompt_included);
        assert!(!report.raw_user_text_included);
        assert!(!report.raw_assistant_output_included);
        assert!(!report.raw_memory_included);
        assert!(!report.raw_life_model_included);
        assert!(!report.raw_tool_payload_included);
        assert!(report.decision_digest.starts_with("sha256:"));

        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("raw user_text"));
        assert!(!serialized.contains("raw assistant_output"));
        assert!(!serialized.contains("raw life event summary"));
        assert!(!serialized.contains("alice@example.com"));
    }
}
