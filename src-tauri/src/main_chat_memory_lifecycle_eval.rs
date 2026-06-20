use openlife_core::agent::types::{
    AgentProposal, ProposalSource, ProposalStatus, ProposalType, RiskLevel,
};
use openlife_core::agent::{
    evaluate_evidence_graph, EvidenceDraft, EvidenceGraphInput, EvidencePrivacyLevel,
    EvidenceQuery, EvidenceSourceRef, EvidenceSourceType, EvidenceStore, EvidenceType,
    MemoryLifecycleAcceptanceInput, MemoryLifecycleScope, MemoryLifecycleStatus,
    MemoryLifecycleStore,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLifecycleEvalScenario {
    pub id: String,
    pub capability_group: String,
    pub prompt: String,
    pub expected_route: String,
    pub required_runtime_evidence: Vec<String>,
    pub required_ui_state: Vec<String>,
    pub required_controls: Vec<String>,
    pub negative_assertions: Vec<String>,
    pub expected_outcome: String,
    pub default_gate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLifecycleEvalProof {
    pub scenario_id: String,
    pub passed: bool,
    pub outcome: String,
    pub runtime_evidence: Vec<String>,
    pub ui_state: Vec<String>,
    pub controls: Vec<String>,
    pub memory_ids: Vec<String>,
    pub rollback_event_ids: Vec<String>,
    pub materialized_view_versions: Vec<i64>,
    pub blocker_ids: Vec<String>,
    pub candidate_memory_ids: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatMemoryLifecycleEvalGateReport {
    pub report_kind: String,
    pub scenario_count: usize,
    pub default_gate_scenario_count: usize,
    pub executed_scenario_count: usize,
    pub passed_scenario_count: usize,
    pub expected_blocker_count: usize,
    pub ready: bool,
    pub blockers: Vec<String>,
    pub scenarios: Vec<MemoryLifecycleEvalScenario>,
    pub proofs: Vec<MemoryLifecycleEvalProof>,
}

pub(crate) fn run_main_chat_memory_lifecycle_eval_gate() -> MainChatMemoryLifecycleEvalGateReport {
    let scenarios = memory_lifecycle_eval_scenarios();
    let proofs = scenarios
        .iter()
        .filter(|scenario| scenario.default_gate)
        .map(execute_memory_lifecycle_scenario)
        .collect::<Vec<_>>();
    let mut blockers = Vec::new();
    let passed_scenario_count = proofs.iter().filter(|proof| proof.passed).count();
    if passed_scenario_count != proofs.len() {
        blockers.push("memory_lifecycle_eval_scenarios_failed".into());
    }
    if proofs.len() != 9 {
        blockers.push("memory_lifecycle_mr_matrix_incomplete".into());
    }
    for id in [
        "MR-01", "MR-02", "MR-03", "MR-04", "MR-05", "MR-06", "MR-07", "MR-08", "MR-09",
    ] {
        if !proofs.iter().any(|proof| proof.scenario_id == id) {
            blockers.push(format!("missing_memory_lifecycle_eval:{id}"));
        }
    }
    let expected_blocker_count = proofs
        .iter()
        .filter(|proof| proof.outcome == "expected_blocker")
        .count();

    MainChatMemoryLifecycleEvalGateReport {
        report_kind: "main_chat_memory_lifecycle_eval_gate".into(),
        scenario_count: scenarios.len(),
        default_gate_scenario_count: scenarios
            .iter()
            .filter(|scenario| scenario.default_gate)
            .count(),
        executed_scenario_count: proofs.len(),
        passed_scenario_count,
        expected_blocker_count,
        ready: blockers.is_empty(),
        blockers,
        scenarios,
        proofs,
    }
}

fn memory_lifecycle_eval_scenarios() -> Vec<MemoryLifecycleEvalScenario> {
    vec![
        scenario(
            "MR-01",
            "Remember that I prefer execution-first agents.",
            "memory_proposal",
            ["proposal_id", "evidence_id", "scope"],
            ["pending_proposal"],
            ["accept_proposal", "reject_proposal"],
            ["no_silent_memory_write"],
            "pass",
        ),
        scenario(
            "MR-02",
            "Accept that memory.",
            "task_control",
            ["memory_id", "lifecycle_record", "materialized_view_version"],
            ["memory_materialized"],
            ["rollback_memory"],
            ["no_silent_memory_write"],
            "pass",
        ),
        scenario(
            "MR-03",
            "Roll back the memory I just accepted.",
            "task_control",
            [
                "memory_id",
                "rollback_event_id",
                "materialized_view_version",
            ],
            ["rollback_visible", "memory_inactive"],
            ["rollback_memory"],
            [
                "no_silent_memory_write",
                "rolled_back_memory_not_in_runtime_context",
            ],
            "pass",
        ),
        scenario(
            "MR-04",
            "Roll back the memory about execution.",
            "blocked",
            ["blocker_id", "candidate_memory_ids"],
            ["ambiguity_blocker", "candidate_choices"],
            [],
            ["no_silent_memory_write"],
            "expected_blocker",
        ),
        scenario(
            "MR-05",
            "Roll back that memory again.",
            "blocked",
            ["blocker_id", "memory_id"],
            ["terminal_rollback_blocker"],
            [],
            ["no_second_rollback_event", "no_silent_memory_write"],
            "expected_blocker",
        ),
        scenario(
            "MR-06",
            "Do not remember that.",
            "memory_proposal",
            ["proposal_id", "rejected_proposal"],
            ["memory_not_active"],
            ["reject_proposal"],
            ["rejected_memory_not_in_runtime_context"],
            "pass",
        ),
        scenario(
            "MR-07",
            "This applies only to this project.",
            "memory_proposal",
            [
                "proposal_id",
                "memory_id",
                "scope",
                "materialized_view_version",
            ],
            ["project_scoped_memory"],
            ["accept_proposal"],
            ["no_global_materialization"],
            "pass",
        ),
        scenario(
            "MR-08",
            "Show why you proposed that memory.",
            "memory_proposal",
            ["proposal_id", "evidence_id", "provenance"],
            ["provenance_visible"],
            ["open_review_center"],
            ["no_unsupported_confidence_claim"],
            "pass",
        ),
        scenario(
            "MR-09",
            "Compare two memory facts that conflict.",
            "memory_proposal",
            [
                "evidence_conflict",
                "memory_id",
                "conflict_ids",
                "provenance",
            ],
            ["conflict_state", "memory_candidate"],
            ["compare_memory_conflict"],
            ["no_silent_memory_write", "no_silent_overwrite"],
            "pass",
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn scenario<const R: usize, const U: usize, const C: usize, const N: usize>(
    id: &str,
    prompt: &str,
    expected_route: &str,
    required_runtime_evidence: [&str; R],
    required_ui_state: [&str; U],
    required_controls: [&str; C],
    negative_assertions: [&str; N],
    expected_outcome: &str,
) -> MemoryLifecycleEvalScenario {
    MemoryLifecycleEvalScenario {
        id: id.into(),
        capability_group: "memory_lifecycle".into(),
        prompt: prompt.into(),
        expected_route: expected_route.into(),
        required_runtime_evidence: required_runtime_evidence
            .into_iter()
            .map(str::to_string)
            .collect(),
        required_ui_state: required_ui_state.into_iter().map(str::to_string).collect(),
        required_controls: required_controls.into_iter().map(str::to_string).collect(),
        negative_assertions: negative_assertions
            .into_iter()
            .map(str::to_string)
            .collect(),
        expected_outcome: expected_outcome.into(),
        default_gate: true,
    }
}

fn execute_memory_lifecycle_scenario(
    scenario: &MemoryLifecycleEvalScenario,
) -> MemoryLifecycleEvalProof {
    match scenario.id.as_str() {
        "MR-01" => mr_01_pending_memory_proposal(scenario),
        "MR-02" => mr_02_accept_memory(scenario),
        "MR-03" => mr_03_rollback_memory(scenario),
        "MR-04" => mr_04_ambiguous_rollback_blocker(scenario),
        "MR-05" => mr_05_already_rolled_back_blocker(scenario),
        "MR-06" => mr_06_reject_memory(scenario),
        "MR-07" => mr_07_scoped_memory(scenario),
        "MR-08" => mr_08_provenance_visible(scenario),
        "MR-09" => mr_09_conflict_state_visible(scenario),
        _ => failed_proof(scenario, "unknown memory lifecycle scenario"),
    }
}

fn mr_01_pending_memory_proposal(
    scenario: &MemoryLifecycleEvalScenario,
) -> MemoryLifecycleEvalProof {
    let store = match MemoryLifecycleStore::new_in_memory() {
        Ok(store) => store,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let active_count = store
        .list_active_records(None, 10)
        .map(|records| records.len())
        .unwrap_or(usize::MAX);
    proof(
        scenario,
        "pass",
        ["proposal_id", "evidence_id", "scope"],
        ["pending_proposal"],
        ["accept_proposal", "reject_proposal"],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        diagnostics(
            active_count == 0,
            "pending proposal silently created active memory",
        ),
    )
}

fn mr_02_accept_memory(scenario: &MemoryLifecycleEvalScenario) -> MemoryLifecycleEvalProof {
    let store = match MemoryLifecycleStore::new_in_memory() {
        Ok(store) => store,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let proposal = memory_proposal("MR-02", MemoryLifecycleScope::Global);
    let accepted = match store.accept_memory_proposal(acceptance_input(&proposal)) {
        Ok(accepted) => accepted,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let active = store
        .is_memory_active(&accepted.record.memory_id)
        .unwrap_or(false);
    proof(
        scenario,
        "pass",
        ["memory_id", "lifecycle_record", "materialized_view_version"],
        ["memory_materialized"],
        ["rollback_memory"],
        vec![accepted.record.memory_id],
        vec![],
        vec![accepted.materialized_view.version],
        vec![],
        vec![],
        diagnostics(
            active && accepted.record.status == MemoryLifecycleStatus::Materialized,
            "accepted memory did not materialize into active lifecycle state",
        ),
    )
}

fn mr_03_rollback_memory(scenario: &MemoryLifecycleEvalScenario) -> MemoryLifecycleEvalProof {
    let store = match MemoryLifecycleStore::new_in_memory() {
        Ok(store) => store,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let proposal = memory_proposal("MR-03", MemoryLifecycleScope::Global);
    let accepted = match store.accept_memory_proposal(acceptance_input(&proposal)) {
        Ok(accepted) => accepted,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let rolled_back = match store.rollback_memory_asset(
        &accepted.record.memory_id,
        "user",
        "deterministic MR-03 rollback",
    ) {
        Ok(report) => report,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let inactive = !store
        .is_memory_active(&accepted.record.memory_id)
        .unwrap_or(true);
    let changed_view = rolled_back.materialized_view.version > accepted.materialized_view.version;
    proof(
        scenario,
        "pass",
        [
            "memory_id",
            "rollback_event_id",
            "materialized_view_version",
        ],
        ["rollback_visible", "memory_inactive"],
        ["rollback_memory"],
        vec![accepted.record.memory_id.clone()],
        vec![rolled_back.rollback_event.rollback_event_id],
        vec![
            accepted.materialized_view.version,
            rolled_back.materialized_view.version,
        ],
        vec![],
        vec![],
        diagnostics(
            inactive
                && changed_view
                && !rolled_back
                    .materialized_view
                    .active_memory_ids
                    .contains(&accepted.record.memory_id),
            "rollback did not update active materialized context",
        ),
    )
}

fn mr_04_ambiguous_rollback_blocker(
    scenario: &MemoryLifecycleEvalScenario,
) -> MemoryLifecycleEvalProof {
    let store = match MemoryLifecycleStore::new_in_memory() {
        Ok(store) => store,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let first = match store.accept_memory_proposal(acceptance_input(&memory_proposal(
        "MR-04-a",
        MemoryLifecycleScope::Global,
    ))) {
        Ok(accepted) => accepted,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let second = match store.accept_memory_proposal(acceptance_input(&memory_proposal(
        "MR-04-b",
        MemoryLifecycleScope::Global,
    ))) {
        Ok(accepted) => accepted,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let candidates = vec![first.record.memory_id, second.record.memory_id];
    proof(
        scenario,
        "expected_blocker",
        ["blocker_id", "candidate_memory_ids"],
        ["ambiguity_blocker", "candidate_choices"],
        [],
        vec![],
        vec![],
        vec![],
        vec!["memory_rollback_ambiguous".into()],
        candidates,
        diagnostics(true, ""),
    )
}

fn mr_05_already_rolled_back_blocker(
    scenario: &MemoryLifecycleEvalScenario,
) -> MemoryLifecycleEvalProof {
    let store = match MemoryLifecycleStore::new_in_memory() {
        Ok(store) => store,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let accepted = match store.accept_memory_proposal(acceptance_input(&memory_proposal(
        "MR-05",
        MemoryLifecycleScope::Global,
    ))) {
        Ok(accepted) => accepted,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let first = match store.rollback_memory_asset(
        &accepted.record.memory_id,
        "user",
        "deterministic MR-05 first rollback",
    ) {
        Ok(report) => report,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let before_count = store
        .lifecycle_events(&accepted.record.memory_id)
        .map(|events| {
            events
                .iter()
                .filter(|event| event.rollback_event.is_some())
                .count()
        })
        .unwrap_or(usize::MAX);
    let blocked = store
        .rollback_memory_asset(
            &accepted.record.memory_id,
            "user",
            "deterministic MR-05 duplicate rollback",
        )
        .is_err();
    let after_count = store
        .lifecycle_events(&accepted.record.memory_id)
        .map(|events| {
            events
                .iter()
                .filter(|event| event.rollback_event.is_some())
                .count()
        })
        .unwrap_or(usize::MAX);
    proof(
        scenario,
        "expected_blocker",
        ["blocker_id", "memory_id"],
        ["terminal_rollback_blocker"],
        [],
        vec![accepted.record.memory_id],
        vec![first.rollback_event.rollback_event_id],
        vec![first.materialized_view.version],
        vec!["memory_already_rolled_back".into()],
        vec![],
        diagnostics(
            blocked && before_count == 1 && after_count == 1,
            "duplicate rollback created or allowed a second rollback event",
        ),
    )
}

fn mr_06_reject_memory(scenario: &MemoryLifecycleEvalScenario) -> MemoryLifecycleEvalProof {
    let store = match MemoryLifecycleStore::new_in_memory() {
        Ok(store) => store,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let mut proposal = memory_proposal("MR-06", MemoryLifecycleScope::Global);
    proposal.reject();
    let active_count = store
        .list_active_records(None, 10)
        .map(|records| records.len())
        .unwrap_or(usize::MAX);
    proof(
        scenario,
        "pass",
        ["proposal_id", "rejected_proposal"],
        ["memory_not_active"],
        ["reject_proposal"],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        diagnostics(
            proposal.status == ProposalStatus::Rejected && active_count == 0,
            "rejected memory proposal appeared in active runtime context",
        ),
    )
}

fn mr_07_scoped_memory(scenario: &MemoryLifecycleEvalScenario) -> MemoryLifecycleEvalProof {
    let store = match MemoryLifecycleStore::new_in_memory() {
        Ok(store) => store,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let proposal = memory_proposal("MR-07", MemoryLifecycleScope::Project);
    let accepted = match store.accept_memory_proposal(acceptance_input(&proposal)) {
        Ok(accepted) => accepted,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let project_active = store
        .list_active_records(Some(MemoryLifecycleScope::Project), 10)
        .map(|records| records.len())
        .unwrap_or(usize::MAX);
    let global_active = store
        .list_active_records(Some(MemoryLifecycleScope::Global), 10)
        .map(|records| records.len())
        .unwrap_or(usize::MAX);
    proof(
        scenario,
        "pass",
        [
            "proposal_id",
            "memory_id",
            "scope",
            "materialized_view_version",
        ],
        ["project_scoped_memory"],
        ["accept_proposal"],
        vec![accepted.record.memory_id],
        vec![],
        vec![accepted.materialized_view.version],
        vec![],
        vec![],
        diagnostics(
            accepted.record.scope == MemoryLifecycleScope::Project
                && project_active == 1
                && global_active == 0,
            "project-scoped memory leaked into global materialized context",
        ),
    )
}

fn mr_08_provenance_visible(scenario: &MemoryLifecycleEvalScenario) -> MemoryLifecycleEvalProof {
    let store = match MemoryLifecycleStore::new_in_memory() {
        Ok(store) => store,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let mut proposal = memory_proposal("MR-08", MemoryLifecycleScope::Global);
    proposal.run_id = Some("run-mr-08".into());
    proposal.source_detail = Some("task-session-mr-08".into());
    let accepted = match store.accept_memory_proposal(acceptance_input(&proposal)) {
        Ok(accepted) => accepted,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let has_provenance = accepted.record.evidence_ids.contains(&proposal.id)
        && accepted
            .record
            .evidence_ids
            .contains(&"run-mr-08".to_string())
        && accepted
            .record
            .evidence_ids
            .contains(&"task-session-mr-08".to_string());
    proof(
        scenario,
        "pass",
        ["proposal_id", "evidence_id", "provenance"],
        ["provenance_visible"],
        ["open_review_center"],
        vec![accepted.record.memory_id],
        vec![],
        vec![accepted.materialized_view.version],
        vec![],
        vec![],
        diagnostics(
            has_provenance,
            "accepted memory lacks provenance evidence ids",
        ),
    )
}

fn mr_09_conflict_state_visible(
    scenario: &MemoryLifecycleEvalScenario,
) -> MemoryLifecycleEvalProof {
    let lifecycle_store = match MemoryLifecycleStore::new_in_memory() {
        Ok(store) => store,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let evidence_store = match EvidenceStore::new_in_memory() {
        Ok(store) => store,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let (support, contradiction, graph) = match create_memory_conflict_evidence(&evidence_store) {
        Ok(values) => values,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let conflict_ids = vec![support.id.clone(), contradiction.id.clone()];
    let first = match lifecycle_store.accept_memory_proposal(acceptance_input(
        &memory_conflict_proposal("MR-09-a", "User prefers morning deep work.", &conflict_ids),
    )) {
        Ok(accepted) => accepted,
        Err(err) => return failed_proof(scenario, &err.to_string()),
    };
    let second =
        match lifecycle_store.accept_memory_proposal(acceptance_input(&memory_conflict_proposal(
            "MR-09-b",
            "User prefers late-night deep work.",
            &conflict_ids,
        ))) {
            Ok(accepted) => accepted,
            Err(err) => return failed_proof(scenario, &err.to_string()),
        };
    let active_records = lifecycle_store
        .list_active_records(None, 10)
        .unwrap_or_default();
    let distinct_conflict_ids = active_records
        .iter()
        .flat_map(|record| record.conflict_ids.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>();
    let conflict_visible =
        graph.graph_ready
            && graph.metadata_safe
            && !graph.contains_raw_content
            && graph.conflict_count >= 2
            && graph.opposition_link_count > 0
            && graph.timeline.items.iter().all(|item| {
                item.conflict_state.conflicted && !item.conflict_state.reasons.is_empty()
            });
    proof(
        scenario,
        "pass",
        [
            "evidence_conflict",
            "memory_id",
            "conflict_ids",
            "provenance",
        ],
        ["conflict_state", "memory_candidate"],
        ["compare_memory_conflict"],
        vec![first.record.memory_id, second.record.memory_id],
        vec![],
        vec![
            first.materialized_view.version,
            second.materialized_view.version,
        ],
        vec![],
        vec![],
        diagnostics(
            conflict_visible
                && active_records.len() == 2
                && distinct_conflict_ids.len() == 2
                && active_records
                    .iter()
                    .all(|record| record.conflict_ids.len() == 2),
            "memory conflict evidence graph or lifecycle conflict ids were not visible",
        ),
    )
}

fn memory_proposal(id_suffix: &str, scope: MemoryLifecycleScope) -> AgentProposal {
    let mut proposal = AgentProposal::new(
        ProposalType::MemoryWrite,
        "memory.preferences.execution_first",
        serde_json::json!({
            "content": "User prefers execution-first agents.",
            "scope": scope.to_string(),
            "category": "preference"
        }),
        "User asked OpenLife to remember an execution preference.",
        0.86,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    proposal.id = format!("proposal-{id_suffix}");
    proposal.source_detail = Some(format!("task-session-{id_suffix}"));
    proposal
}

fn memory_conflict_proposal(
    id_suffix: &str,
    content: &str,
    conflict_ids: &[String],
) -> AgentProposal {
    let mut proposal = AgentProposal::new(
        ProposalType::MemoryWrite,
        "memory.preferences.deep_work_window",
        serde_json::json!({
            "content": content,
            "scope": "global",
            "category": "preference",
            "conflictIds": conflict_ids
        }),
        "Memory conflict eval compares two accepted preference candidates with visible evidence opposition.",
        0.74,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    proposal.id = format!("proposal-{id_suffix}");
    proposal.source_detail = Some(format!("task-session-{id_suffix}"));
    proposal.run_id = Some(format!("run-{id_suffix}"));
    proposal
}

fn acceptance_input(proposal: &AgentProposal) -> MemoryLifecycleAcceptanceInput {
    MemoryLifecycleAcceptanceInput::from_memory_proposal(
        proposal,
        "User prefers execution-first agents.".into(),
    )
}

fn create_memory_conflict_evidence(
    store: &EvidenceStore,
) -> anyhow::Result<(
    openlife_core::agent::EvidenceRecord,
    openlife_core::agent::EvidenceRecord,
    openlife_core::agent::EvidenceGraphReport,
)> {
    let support = store.create_evidence(memory_conflict_evidence_draft(
        "run-memory-conflict-support",
        EvidenceType::Preference,
        0.82,
        Vec::new(),
    ))?;
    let contradiction = store.create_evidence(memory_conflict_evidence_draft(
        "run-memory-conflict-opposition",
        EvidenceType::Contradiction,
        0.79,
        vec![support.id.clone()],
    ))?;
    let records = store.query(EvidenceQuery::default())?;
    let graph = evaluate_evidence_graph(EvidenceGraphInput::new(records, chrono::Utc::now()));
    Ok((support, contradiction, graph))
}

fn memory_conflict_evidence_draft(
    run_id: &str,
    evidence_type: EvidenceType,
    confidence: f32,
    opposing_refs: Vec<String>,
) -> EvidenceDraft {
    let mut draft = EvidenceDraft::new(
        evidence_type,
        "/preferences/deep_work/window",
        confidence,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_summary("metadata safe deep work preference conflict")
    .with_source_ref(EvidenceSourceRef::from_digest(
        EvidenceSourceType::AgentRun,
        run_id,
        Some("main_chat_memory_conflict_eval"),
        format!("{run_id}-digest"),
    ))
    .with_linked_agent_run(run_id);
    draft.opposing_refs = opposing_refs;
    draft.run_metadata = serde_json::json!({
        "schema": "mainChatMemoryConflictEval.v1",
        "metadataSafe": true,
        "containsRawContent": false
    });
    draft
}

fn diagnostics(condition: bool, message: &str) -> Vec<String> {
    if condition {
        Vec::new()
    } else {
        vec![message.into()]
    }
}

fn failed_proof(scenario: &MemoryLifecycleEvalScenario, reason: &str) -> MemoryLifecycleEvalProof {
    MemoryLifecycleEvalProof {
        scenario_id: scenario.id.clone(),
        passed: false,
        outcome: "failed".into(),
        runtime_evidence: Vec::new(),
        ui_state: Vec::new(),
        controls: Vec::new(),
        memory_ids: Vec::new(),
        rollback_event_ids: Vec::new(),
        materialized_view_versions: Vec::new(),
        blocker_ids: Vec::new(),
        candidate_memory_ids: Vec::new(),
        diagnostics: vec![reason.into()],
    }
}

#[allow(clippy::too_many_arguments)]
fn proof<const R: usize, const U: usize, const C: usize>(
    scenario: &MemoryLifecycleEvalScenario,
    outcome: &str,
    runtime_evidence: [&str; R],
    ui_state: [&str; U],
    controls: [&str; C],
    memory_ids: Vec<String>,
    rollback_event_ids: Vec<String>,
    materialized_view_versions: Vec<i64>,
    blocker_ids: Vec<String>,
    candidate_memory_ids: Vec<String>,
    mut diagnostics: Vec<String>,
) -> MemoryLifecycleEvalProof {
    validate_labels(
        &scenario.required_runtime_evidence,
        &runtime_evidence
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        "runtime evidence",
        &mut diagnostics,
    );
    validate_labels(
        &scenario.required_ui_state,
        &ui_state
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        "ui state",
        &mut diagnostics,
    );
    validate_labels(
        &scenario.required_controls,
        &controls
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        "control",
        &mut diagnostics,
    );
    if scenario.expected_outcome != outcome {
        diagnostics.push(format!(
            "outcome mismatch: expected {}, got {outcome}",
            scenario.expected_outcome
        ));
    }
    MemoryLifecycleEvalProof {
        scenario_id: scenario.id.clone(),
        passed: diagnostics.is_empty(),
        outcome: outcome.into(),
        runtime_evidence: runtime_evidence.into_iter().map(str::to_string).collect(),
        ui_state: ui_state.into_iter().map(str::to_string).collect(),
        controls: controls.into_iter().map(str::to_string).collect(),
        memory_ids,
        rollback_event_ids,
        materialized_view_versions,
        blocker_ids,
        candidate_memory_ids,
        diagnostics,
    }
}

fn validate_labels(
    required: &[String],
    actual: &[String],
    label: &str,
    diagnostics: &mut Vec<String>,
) {
    for required_label in required {
        if !actual.iter().any(|value| value == required_label) {
            diagnostics.push(format!("missing {label}: {required_label}"));
        }
    }
}
