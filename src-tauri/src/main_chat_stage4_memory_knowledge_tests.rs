use std::fs;

use openlife_core::agent::{
    AgentProposal, ProposalSource, ProposalStatus, ProposalType, RiskLevel,
};

use crate::commands::proposal::{accept_proposal_with_state, rollback_memory_asset_with_state};
use crate::main_chat_stage4_memory_knowledge::{
    build_stage4_knowledge_asset_inventory_for_root, confirm_managed_knowledge_write_with_state,
    create_managed_knowledge_write_draft_with_state, draft_edit_memory_proposal_with_state,
    evaluate_main_chat_stage4_memory_knowledge_for_root,
    rollback_managed_knowledge_write_with_state,
};

fn memory_write_proposal(content: &str) -> AgentProposal {
    let mut proposal = AgentProposal::new(
        ProposalType::MemoryWrite,
        "memory.stage4.preference",
        serde_json::json!({
            "content": content,
            "sessionId": "stage4-memory-session",
            "source": "stage4_test"
        }),
        "Stage 4 memory proposal fixture.",
        0.84,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    proposal.source_detail = Some("task-session-stage4".into());
    proposal
}

#[tokio::test]
async fn main_chat_stage4_draft_edit_keeps_memory_proposal_pending_without_durable_write() {
    let state = crate::test_utils::test_app_state();
    let proposal = memory_write_proposal("Prefer concise reviews.");
    let proposal_id = proposal.id.clone();
    state
        .proposal_store
        .as_ref()
        .expect("proposal store")
        .lock()
        .await
        .create_proposal(&proposal)
        .expect("create proposal");

    let report = draft_edit_memory_proposal_with_state(
        proposal_id.clone(),
        serde_json::json!({
            "content": "Prefer concise, rigorous reviews.",
            "sessionId": "stage4-memory-session",
            "source": "stage4_test"
        }),
        &state,
    )
    .await
    .expect("draft edit");

    assert!(report.draft_only);
    assert!(!report.durable_write_executed);
    assert!(report.original_provenance_preserved);
    assert_eq!(report.proposal_id, proposal_id);

    let stored = state
        .proposal_store
        .as_ref()
        .expect("proposal store")
        .lock()
        .await
        .get_proposal(&proposal_id)
        .expect("get proposal")
        .expect("stored proposal");
    assert_eq!(stored.status, ProposalStatus::Pending);
    assert!(stored.resolved_at.is_none());
    assert_eq!(
        stored.after["content"].as_str(),
        Some("Prefer concise, rigorous reviews.")
    );
    assert_eq!(stored.source_detail.as_deref(), Some("task-session-stage4"));

    let lifecycle_record = state
        .memory_lifecycle_store
        .as_ref()
        .expect("memory lifecycle")
        .lock()
        .await
        .get_record_by_proposal_id(&proposal_id)
        .expect("lifecycle query");
    assert!(
        lifecycle_record.is_none(),
        "draft-only edit must not materialize accepted memory"
    );
    let hits = state
        .memory_store
        .lock()
        .await
        .search_text_memories(
            Some("stage4-memory-session"),
            "concise rigorous reviews",
            10,
        )
        .expect("search memory");
    assert!(
        hits.is_empty(),
        "draft-only edit must not write legacy memory rows"
    );
}

#[tokio::test]
async fn main_chat_stage4_rollback_archives_lifecycle_linked_legacy_memory_and_vector_rows() {
    let state = crate::test_utils::test_app_state();
    let proposal = memory_write_proposal("Use crimson dashboards for rollback proof.");
    let proposal_id = proposal.id.clone();
    state
        .proposal_store
        .as_ref()
        .expect("proposal store")
        .lock()
        .await
        .create_proposal(&proposal)
        .expect("create proposal");

    let accepted = accept_proposal_with_state(proposal_id, &state)
        .await
        .expect("accept proposal");
    let memory_id = accepted["memoryLifecycle"]["memoryId"]
        .as_str()
        .expect("memory id")
        .to_string();
    let lifecycle_source = format!("memory_lifecycle:{memory_id}");
    state
        .vector_store
        .lock()
        .await
        .insert(
            "stage4-memory-session",
            "Use crimson dashboards for rollback proof.",
            &[1.0, 0.0, 0.0, 0.0],
            &lifecycle_source,
        )
        .expect("insert vector");

    let text_hits_before = state
        .memory_store
        .lock()
        .await
        .search_text_memories(Some("stage4-memory-session"), "crimson dashboards", 10)
        .expect("search before");
    assert!(text_hits_before
        .iter()
        .any(|hit| hit.chunk.source == lifecycle_source));
    let vector_hits_before = state
        .vector_store
        .lock()
        .await
        .search_by_session("stage4-memory-session", &[1.0, 0.0, 0.0, 0.0], 10, 100)
        .expect("vector before");
    assert!(vector_hits_before
        .iter()
        .any(|(chunk, _)| chunk.source == lifecycle_source));

    rollback_memory_asset_with_state(
        memory_id.clone(),
        "Stage 4 rollback exclusion proof.".into(),
        &state,
    )
    .await
    .expect("rollback");

    let text_hits_after = state
        .memory_store
        .lock()
        .await
        .search_text_memories(Some("stage4-memory-session"), "crimson dashboards", 10)
        .expect("search after");
    assert!(
        text_hits_after
            .iter()
            .all(|hit| hit.chunk.source != lifecycle_source),
        "rolled-back lifecycle memory must not leak through text memory search"
    );
    let vector_hits_after = state
        .vector_store
        .lock()
        .await
        .search_by_session("stage4-memory-session", &[1.0, 0.0, 0.0, 0.0], 10, 100)
        .expect("vector after");
    assert!(
        vector_hits_after
            .iter()
            .all(|(chunk, _)| chunk.source != lifecycle_source),
        "rolled-back lifecycle memory must not leak through vector search"
    );
}

#[test]
fn main_chat_stage4_knowledge_inventory_reports_loaded_skipped_truncated_and_skill_boundary() {
    let root = tempfile::tempdir().expect("root");
    fs::create_dir_all(root.path().join("skills/selected")).expect("selected skill dir");
    fs::create_dir_all(root.path().join("skills/unselected")).expect("unselected skill dir");
    fs::write(root.path().join("AGENTS.md"), "workspace guidance").expect("agents");
    fs::write(root.path().join("USER.md"), "u".repeat(1400)).expect("user");
    fs::write(root.path().join("MEMORY.md"), "active memory summary").expect("memory");
    fs::write(
        root.path().join("skills/selected/SKILL.md"),
        "selected skill guidance",
    )
    .expect("selected skill");
    fs::write(
        root.path().join("skills/unselected/SKILL.md"),
        "unselected skill guidance",
    )
    .expect("unselected skill");

    let inventory = build_stage4_knowledge_asset_inventory_for_root(root.path(), Some("selected"))
        .expect("inventory");

    assert!(inventory
        .loaded_assets
        .iter()
        .any(|asset| asset.relative_path == "USER.md" && asset.truncated));
    assert!(inventory
        .loaded_assets
        .iter()
        .any(|asset| asset.relative_path == "MEMORY.md" && !asset.digest.is_empty()));
    assert!(inventory.loaded_assets.iter().any(|asset| {
        asset.relative_path == "skills/selected/SKILL.md"
            && asset.selected_skill_id.as_deref() == Some("selected")
    }));
    assert!(inventory.skipped_assets.iter().any(|asset| {
        asset.relative_path == "skills/unselected/SKILL.md" && asset.reason == "unselected_skill"
    }));
    assert!(inventory
        .skipped_assets
        .iter()
        .any(|asset| asset.relative_path == "SOUL.md" && asset.reason == "missing"));
}

#[tokio::test]
async fn main_chat_stage4_managed_user_and_memory_writes_confirm_reload_and_roll_back() {
    let state = crate::test_utils::test_app_state();
    let root = tempfile::tempdir().expect("root");
    fs::write(root.path().join("USER.md"), "old user profile\n").expect("user");
    fs::write(root.path().join("MEMORY.md"), "old memory summary\n").expect("memory");

    for (target, new_content) in [
        ("USER.md", "new accepted user profile\n"),
        ("MEMORY.md", "new accepted memory summary\n"),
    ] {
        let before_content = fs::read_to_string(root.path().join(target)).expect("before");
        let draft = create_managed_knowledge_write_draft_with_state(
            target.into(),
            new_content.into(),
            None,
            vec!["memory:stage4-managed".into()],
            root.path(),
            &state,
        )
        .await
        .expect("draft");

        assert_eq!(draft.target_path, target);
        assert!(draft.validation.allowed);
        assert!(!draft.before_digest.is_empty());
        assert!(!draft.after_digest.is_empty());
        assert_ne!(draft.before_digest, draft.after_digest);
        assert!(draft.preview_diff.contains("+new accepted"));
        assert!(!draft.file_written_before_confirmation);
        assert_eq!(
            fs::read_to_string(root.path().join(target)).expect("unchanged"),
            before_content,
            "managed draft must not write before explicit confirmation"
        );

        let applied = confirm_managed_knowledge_write_with_state(
            draft.proposal_id.clone(),
            root.path(),
            &state,
        )
        .await
        .expect("confirm");

        assert_eq!(
            fs::read_to_string(root.path().join(target)).expect("after"),
            new_content
        );
        assert_eq!(applied.target_path, target);
        assert!(!applied.audit_id.is_empty());
        assert!(!applied.version_id.is_empty());
        assert!(!applied.rollback_snapshot_id.is_empty());
        assert_eq!(applied.after_digest, draft.after_digest);
        assert!(applied.context_reload.loaded);
        assert_eq!(applied.context_reload.digest, draft.after_digest);

        let rolled_back =
            rollback_managed_knowledge_write_with_state(applied.version_id.clone(), root.path())
                .expect("rollback");

        assert_eq!(rolled_back.target_path, target);
        assert_eq!(
            fs::read_to_string(root.path().join(target)).expect("restored"),
            before_content
        );
        assert!(rolled_back.context_reload.loaded);
        assert_eq!(rolled_back.restored_digest, draft.before_digest);
        assert_ne!(rolled_back.context_reload.digest, draft.after_digest);
    }
}

#[tokio::test]
async fn main_chat_stage4_managed_write_confirm_handles_truncated_context_inventory() {
    let state = crate::test_utils::test_app_state();
    let root = tempfile::tempdir().expect("root");
    fs::write(root.path().join("USER.md"), "old user profile\n").expect("user");
    let long_content = format!("{}\n", "long accepted user profile ".repeat(90));

    let draft = create_managed_knowledge_write_draft_with_state(
        "USER.md".into(),
        long_content.clone(),
        None,
        vec![],
        root.path(),
        &state,
    )
    .await
    .expect("draft");

    let inventory = build_stage4_knowledge_asset_inventory_for_root(root.path(), None)
        .expect("inventory before confirm");
    assert!(inventory
        .loaded_assets
        .iter()
        .any(|asset| asset.relative_path == "USER.md" && !asset.truncated));

    let applied =
        confirm_managed_knowledge_write_with_state(draft.proposal_id.clone(), root.path(), &state)
            .await
            .expect("confirm long managed write");

    assert_eq!(
        fs::read_to_string(root.path().join("USER.md")).expect("after"),
        long_content
    );
    assert_eq!(applied.after_digest, draft.after_digest);
    assert_eq!(applied.context_reload.digest, draft.after_digest);

    let inventory = build_stage4_knowledge_asset_inventory_for_root(root.path(), None)
        .expect("inventory after confirm");
    assert!(inventory
        .loaded_assets
        .iter()
        .any(|asset| asset.relative_path == "USER.md" && asset.truncated));
}

#[tokio::test]
async fn main_chat_stage4_report_covers_mk4_rows_without_readiness_claim() {
    let state = crate::test_utils::test_app_state();
    let root = tempfile::tempdir().expect("root");
    fs::write(root.path().join("USER.md"), "stage4 user context").expect("user");
    fs::write(root.path().join("MEMORY.md"), "stage4 memory context").expect("memory");

    let report = evaluate_main_chat_stage4_memory_knowledge_for_root(&state, root.path())
        .await
        .expect("report");

    assert_eq!(report.report_kind, "main_chat_stage4_memory_knowledge");
    assert_eq!(report.scenario_count, 18);
    assert!(report.not_a_readiness_gate);
    assert!(!report.readiness_claim);
    assert!(report.stage2_readiness_preserved);
    for index in 1..=18 {
        let id = format!("MK4-{index:02}");
        assert!(
            report.rows.iter().any(|row| row.id == id),
            "missing Stage 4 row {id}"
        );
    }
    assert!(!report
        .blockers
        .iter()
        .any(|blocker| blocker.contains("ready_for_limited_internal_trial")));
}
