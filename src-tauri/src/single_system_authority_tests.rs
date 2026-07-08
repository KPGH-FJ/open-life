use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has repo parent")
        .to_path_buf()
}

fn read_repo_file(path: &str) -> String {
    fs::read_to_string(repo_root().join(path)).unwrap_or_else(|err| panic!("read {path}: {err}"))
}

fn inventory() -> serde_json::Value {
    serde_json::from_str(&read_repo_file(
        "plans/openlife_single_system_phase1_inventory.json",
    ))
    .expect("parse Phase 1 inventory")
}

fn inventory_entries<'a>(
    inventory: &'a serde_json::Value,
    key: &str,
) -> Vec<&'a serde_json::Value> {
    inventory
        .get(key)
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("inventory category {key} must be an array"))
        .iter()
        .collect()
}

fn entry_str<'a>(entry: &'a serde_json::Value, field: &str) -> &'a str {
    entry
        .get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("inventory entry missing string field {field}: {entry:?}"))
}

fn entry_u64(entry: &serde_json::Value, field: &str) -> u64 {
    entry
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| panic!("inventory entry missing integer field {field}: {entry:?}"))
}

fn entry_optional_str<'a>(entry: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    entry.get(field).and_then(serde_json::Value::as_str)
}

fn entry_bool(entry: &serde_json::Value, field: &str) -> bool {
    entry
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn validate_inventory_path_contract(category: &str, entry: &serde_json::Value, disposition: &str) {
    let id = entry_str(entry, "id");
    let path = entry_optional_str(entry, "path");
    let former_path = entry_optional_str(entry, "former_path");
    let expected_absent = entry_bool(entry, "expected_absent");
    let path_kind = entry_optional_str(entry, "path_kind").unwrap_or("file");

    if disposition == "deleted" || expected_absent {
        assert_eq!(
            disposition, "deleted",
            "inventory entry {id} in {category} uses expected_absent without deleted disposition"
        );
        if path_kind == "symbol" {
            let host_path = entry_str(entry, "host_path");
            let former_symbol = entry_str(entry, "former_symbol");
            assert!(
                repo_root().join(host_path).exists(),
                "deleted symbol inventory host_path must exist: {host_path}"
            );
            let source = read_repo_file(host_path);
            assert!(
                !strip_cfg_test_module(&source).contains(former_symbol),
                "deleted symbol {former_symbol} must be absent from {host_path}"
            );
            return;
        }
        assert!(
            path.is_none(),
            "deleted inventory entry {id} in {category} must use former_path, not active path"
        );
        assert!(
            entry_bool(entry, "retired"),
            "deleted inventory entry {id} in {category} must set retired=true"
        );
        assert!(
            expected_absent,
            "deleted inventory entry {id} in {category} must set expected_absent=true"
        );
        let former_path = former_path.unwrap_or_else(|| {
            panic!("deleted inventory entry {id} in {category} must include former_path")
        });
        assert!(
            !repo_root().join(former_path).exists(),
            "deleted inventory former_path must be absent: {former_path}"
        );
    } else {
        assert!(
            former_path.is_none(),
            "active inventory entry {id} in {category} must not use former_path"
        );
        let path =
            path.unwrap_or_else(|| panic!("active inventory entry {id} in {category} needs path"));
        assert!(
            repo_root().join(path).exists(),
            "active inventory path must exist: {path}"
        );
    }
}

#[test]
fn single_system_phase3_old_product_routers_are_absent_from_product_surfaces() {
    let forbidden_terms = [
        "IntentRouter",
        "LayerRouter",
        "StrategySelector",
        "intent_router",
        "layer_router",
        "get_router_status",
    ];

    for file in source_files(&["openlife-core/src", "src-tauri/src", "frontend/src"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with("_tests.rs")
            || rel.ends_with(".test.ts")
            || rel.ends_with(".test.tsx")
            || rel == "src-tauri/src/single_system_authority_tests.rs"
            || rel.starts_with("openlife-core/src/agent/tests/")
        {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        let stripped = strip_cfg_test_module(&source);
        for term in forbidden_terms {
            assert!(
                !stripped.contains(term),
                "Phase 3 product source must not contain old router surface {term}: {rel}"
            );
        }
    }

    let state = read_repo_file("src-tauri/src/state.rs");
    assert!(!state.contains("intent_router"));
    assert!(!state.contains("layer_router"));

    let bootstrap = read_repo_file("src-tauri/src/bootstrap.rs");
    assert!(!bootstrap.contains("IntentRouter"));
    assert!(!bootstrap.contains("LayerRouter"));

    let diagnostics = read_repo_file("src-tauri/src/commands/diagnostics.rs");
    assert!(!diagnostics.contains("get_router_status"));
    assert!(!diagnostics.contains("state.intent_router"));

    let lib = read_repo_file("src-tauri/src/lib.rs");
    assert!(!lib.contains("get_router_status"));
}

fn to_repo_path(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .expect("path inside repo")
        .to_string_lossy()
        .replace('\\', "/")
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap_or_else(|err| panic!("read dir {:?}: {err}", root)) {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_files(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn source_files(roots: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in roots {
        collect_files(&repo_root().join(root), &mut files);
    }
    files
        .into_iter()
        .filter(|path| {
            matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("rs" | "ts" | "tsx")
            )
        })
        .collect()
}

fn strip_cfg_test_module(source: &str) -> &str {
    source
        .split("\n#[cfg(test)]\nmod tests")
        .next()
        .unwrap_or(source)
}

fn count_occurrences(source: &str, needle: &str) -> usize {
    source.match_indices(needle).count()
}

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    source
        .split(start)
        .nth(1)
        .unwrap_or_else(|| panic!("missing function start marker: {start}"))
        .split(end)
        .next()
        .unwrap_or_else(|| panic!("missing function end marker: {end}"))
}

fn expected_count_map(category: &str) -> BTreeMap<String, usize> {
    let inventory = inventory();
    inventory_entries(&inventory, category)
        .into_iter()
        .map(|entry| {
            (
                entry_str(entry, "path").to_string(),
                entry_u64(entry, "expected_count") as usize,
            )
        })
        .collect()
}

fn legacy_development_command_tokens() -> Vec<&'static str> {
    vec![
        "stage",
        "beta",
        "migration",
        "cutover",
        "dogfood",
        "eval",
        "productization",
        "maturity",
        "readiness",
        "acceptance",
        "final_acceptance",
        "step6",
        "pilot",
        "debug",
        "capability",
        "strategy",
        "preview",
        "internal_issue_report",
        "issue_report",
    ]
}

#[test]
fn single_system_readme_declares_active_authority_before_historical_plans() {
    let readme = read_repo_file("plans/README.md");
    let dev_doc = "plans/openlife_single_system_development_preparation.md";
    let manifest_doc = "plans/openlife_single_system_deletion_manifest.md";
    let inventory_doc = "plans/openlife_single_system_phase1_inventory.json";

    for required in [
        dev_doc,
        manifest_doc,
        inventory_doc,
        "no new system without deletion",
        "no product-visible legacy/beta/stage/migration/cutover route",
        "no direct durable write outside gateway",
        "no frontend independent product readiness source",
        "historical reference",
    ] {
        assert!(
            readme.contains(required),
            "plans/README.md must contain active single-system authority text: {required}"
        );
    }

    let dev_pos = readme
        .find(dev_doc)
        .expect("single-system prep doc in README");
    let manifest_pos = readme
        .find(manifest_doc)
        .expect("single-system deletion manifest in README");
    for historical in [
        "plans/main_chat_agent_kernel_rescue_goal_8_cleanup_final_gate.md",
        "plans/main_chat_stage2_preparation_index.md",
        "plans/main_chat_agent_stage2_internal_trial_goal_spec.md",
        "plans/main_chat_agent_migration_v1_goal_spec.md",
    ] {
        let old_pos = readme
            .find(historical)
            .unwrap_or_else(|| panic!("historical plan {historical} still listed"));
        assert!(
            dev_pos < old_pos && manifest_pos < old_pos,
            "{historical} must appear after the active single-system docs"
        );
    }
}

#[test]
fn single_system_phase1_inventory_has_required_categories_and_contract_fields() {
    let inventory = inventory();
    let required_categories = [
        "product_authorities",
        "old_runtime_surfaces",
        "old_router_surfaces",
        "direct_proposal_write_surfaces",
        "phase4_proposal_creation_source_map",
        "direct_memory_lifemodel_write_surfaces",
        "frontend_multi_source_state_surfaces",
        "product_command_allowlist",
        "legacy_development_command_surfaces",
        "product_old_route_markers",
    ];
    let allowed_dispositions = BTreeSet::from([
        "keep",
        "absorb_then_delete",
        "delete",
        "deleted",
        "storage_only",
        "test_fixture_only",
        "archive_reference",
        "quarantined_historical_runtime",
        "internal_debug_historical_surface",
        "legacy_development_command",
    ]);

    for category in required_categories {
        let entries = inventory_entries(&inventory, category);
        assert!(
            !entries.is_empty(),
            "inventory category {category} must not be empty"
        );
        for entry in entries {
            let id = entry_str(entry, "id");
            let disposition = entry_str(entry, "disposition");
            assert!(!id.trim().is_empty(), "inventory id must be non-empty");
            validate_inventory_path_contract(category, entry, disposition);
            assert!(
                entry
                    .get("phase")
                    .and_then(serde_json::Value::as_u64)
                    .is_some(),
                "inventory entry {id} must include numeric phase"
            );
            assert!(
                allowed_dispositions.contains(disposition),
                "inventory entry {id} uses unsupported disposition {disposition}"
            );
            assert!(
                !entry_str(entry, "reason").trim().is_empty(),
                "inventory entry {id} must include a reason"
            );
        }
    }

    let manifest = read_repo_file("plans/openlife_single_system_deletion_manifest.md");
    let combined = format!("{}\n{}", inventory, manifest).to_lowercase();
    for forbidden in [
        "maybe later",
        "optional",
        "temporary compatibility",
        "if needed",
        "if still needed",
    ] {
        assert!(
            !combined.contains(forbidden),
            "single-system deletion contracts must not use vague disposition wording: {forbidden}"
        );
    }
}

#[test]
fn single_system_phase2_openlife_turn_runtime_retirement_guard_matches_inventory() {
    let inventory = inventory();
    let retired_runtime_ids = BTreeSet::from([
        "main_chat_strategy",
        "main_chat_tool_loop",
        "main_chat_legacy_agent_loop",
    ]);
    let old_runtime_entries = inventory_entries(&inventory, "old_runtime_surfaces");
    for retired_id in &retired_runtime_ids {
        let entry = old_runtime_entries
            .iter()
            .copied()
            .find(|entry| entry_str(entry, "id") == *retired_id)
            .unwrap_or_else(|| panic!("retired runtime inventory entry missing: {retired_id}"));
        assert_eq!(entry_str(entry, "disposition"), "deleted");
        validate_inventory_path_contract("old_runtime_surfaces", entry, "deleted");
    }

    let lib = read_repo_file("src-tauri/src/lib.rs");
    for registration in [
        ["pub(crate) mod main_chat_", "strategy;"].concat(),
        ["pub(crate) mod main_chat_", "tool_loop;"].concat(),
        ["pub(crate) mod main_chat_", "legacy_agent_loop;"].concat(),
    ] {
        assert!(
            !lib.contains(&registration),
            "lib.rs must not register retired runtime module {registration}"
        );
    }

    let send = read_repo_file("src-tauri/src/main_chat_send.rs");
    let stream = read_repo_file("src-tauri/src/main_chat_streaming.rs");
    let pipeline = read_repo_file("src-tauri/src/main_chat_turn_pipeline.rs");
    let runtime = read_repo_file("src-tauri/src/main_chat_turn_runtime.rs");
    for (label, source) in [
        ("send", send.as_str()),
        ("stream", stream.as_str()),
        ("pipeline", pipeline.as_str()),
    ] {
        assert!(
            source.contains("OpenLifeTurnRuntime::new("),
            "{label} must delegate ordinary Main Chat turns to OpenLifeTurnRuntime"
        );
    }
    for (label, source) in [
        ("send", send.as_str()),
        ("stream", stream.as_str()),
        ("pipeline", pipeline.as_str()),
        ("runtime", runtime.as_str()),
    ] {
        for forbidden in [
            ["try_run_main_chat_agent_", "strategy("].concat(),
            ["run_main_chat_tool_loop_", "adapter("].concat(),
            ["send_message_with_", "agent_loop("].concat(),
            ["start_stream_message_with_", "agent_loop("].concat(),
            ["handle_agent_loop_", "fallback("].concat(),
            ["run_single_step_react_", "fallback("].concat(),
        ] {
            assert!(
                !source.contains(&forbidden),
                "{label} must not call retired runtime helper {forbidden}"
            );
        }
    }
    for (label, source) in [
        ("send", send.as_str()),
        ("stream", stream.as_str()),
        ("pipeline", pipeline.as_str()),
        ("runtime", runtime.as_str()),
    ] {
        for forbidden in [
            "preview_main_chat_turn_route",
            "attach_route_preview_trace",
            "main_chat_route_preview",
        ] {
            assert!(
                !source.contains(forbidden),
                "{label} must not use route preview in the Phase 2 ordinary runtime path"
            );
        }
    }
    assert!(
        !pipeline.contains("persist_life_model("),
        "thin turn pipeline wrapper must not directly write LifeModel state"
    );

    let legacy_json_true = ["legacyFallbackUsed", "\": true"].concat();
    let single_step_json_true = ["singleStepFallbackUsed", "\": true"].concat();
    let legacy_assignment_true = ["legacy_fallback_used = ", "true"].concat();
    for file in source_files(&["src-tauri/src"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with("_tests.rs") || rel == "src-tauri/src/single_system_authority_tests.rs" {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        let stripped = strip_cfg_test_module(&source);
        assert!(
            !stripped.contains(&legacy_json_true)
                && !stripped.contains(&single_step_json_true)
                && !stripped.contains(&legacy_assignment_true),
            "product source must not mark retired fallback as successful: {rel}"
        );
    }
}

#[test]
fn single_system_handler_legacy_development_commands_match_inventory_or_allowlist() {
    let lib = read_repo_file("src-tauri/src/lib.rs");
    let handler = lib
        .split("tauri::generate_handler![")
        .nth(1)
        .and_then(|rest| rest.split("])").next())
        .expect("Tauri generate_handler body");
    let tokens = legacy_development_command_tokens();
    for required in [
        "stage",
        "beta",
        "migration",
        "cutover",
        "dogfood",
        "eval",
        "productization",
        "maturity",
        "readiness",
        "acceptance",
        "final_acceptance",
        "step6",
        "pilot",
        "debug",
        "capability",
        "internal_issue_report",
        "issue_report",
    ] {
        assert!(
            tokens.contains(&required),
            "legacy/development command token set must include {required}"
        );
    }

    let legacy_like_in_handler: BTreeSet<String> = handler
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .filter(|token| !token.is_empty())
        .filter(|token| tokens.iter().any(|old| token.contains(old)))
        .map(str::to_string)
        .collect();

    let inventory = inventory();
    let legacy_inventory: BTreeSet<String> =
        inventory_entries(&inventory, "legacy_development_command_surfaces")
            .into_iter()
            .map(|entry| entry_str(entry, "command").to_string())
            .collect();
    let product_allowlist: BTreeSet<String> =
        inventory_entries(&inventory, "product_command_allowlist")
            .into_iter()
            .map(|entry| entry_str(entry, "command").to_string())
            .collect();
    let old_commands_still_registered: BTreeSet<String> = legacy_like_in_handler
        .intersection(&legacy_inventory)
        .cloned()
        .collect();
    assert_eq!(
        old_commands_still_registered,
        BTreeSet::new(),
        "Phase 7 forbids legacy/development/eval-like commands in the shipped Tauri handler"
    );
    assert_eq!(
        legacy_like_in_handler, product_allowlist,
        "only product allowlist commands may match broad legacy/development tokens in the shipped handler"
    );

    let manifest = read_repo_file("plans/openlife_single_system_deletion_manifest.md");
    for command in &legacy_inventory {
        assert!(
            manifest.contains(&format!("`{command}`")),
            "deletion manifest must classify retired legacy/development command {command}"
        );
    }
    for command in &product_allowlist {
        assert!(
            manifest.contains(&format!("`{command}`")) && manifest.contains("product allowlist"),
            "deletion manifest must explain product allowlist command {command}"
        );
    }
}

#[test]
fn single_system_direct_proposal_write_callsites_match_inventory() {
    let expected = expected_count_map("direct_proposal_write_surfaces");
    let mut actual = BTreeMap::new();
    for file in source_files(&["src-tauri/src", "openlife-core/src"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with("_tests.rs") || rel.contains("/tests/") {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        let count = count_occurrences(strip_cfg_test_module(&source), ".create_proposal(");
        if count > 0 {
            actual.insert(rel, count);
        }
    }
    assert_eq!(
        actual, expected,
        "direct proposal write callsites must be registered in Phase 1 inventory"
    );
}

#[test]
fn single_system_phase4_proposal_creation_is_review_workflow_governed() {
    let inventory = inventory();
    let allowed_phase4_classes = BTreeSet::from([
        "product_path",
        "review_workflow_target",
        "storage_only",
        "test_fixture",
        "historical_eval",
        "to_delete_later",
        "deleted",
    ]);
    for entry in inventory_entries(&inventory, "phase4_proposal_creation_source_map") {
        let class = entry_str(entry, "phase4_classification");
        if class == "deleted" {
            continue;
        }
        let path = entry_str(entry, "path");
        let route = entry_str(entry, "proposal_creation_route");
        assert!(
            allowed_phase4_classes.contains(class),
            "Phase4 proposal creation source-map entry {path} uses unknown class {class}"
        );
        if class == "product_path" {
            assert_eq!(
                route, "ReviewWorkflow",
                "product proposal creation path {path} must route through ReviewWorkflow"
            );
        }
        if class == "review_workflow_target" {
            assert_eq!(
                route, "ProposalStore",
                "ReviewWorkflow target {path} must be the only product layer calling ProposalStore"
            );
        }
    }

    let allowed_direct_classes = BTreeSet::from([
        "review_workflow_target",
        "storage_only",
        "test_fixture",
        "historical_eval",
    ]);
    let mut classified_paths = BTreeMap::new();
    for entry in inventory_entries(&inventory, "direct_proposal_write_surfaces") {
        let path = entry_str(entry, "path").to_string();
        let class = entry_str(entry, "phase4_classification").to_string();
        assert!(
            allowed_direct_classes.contains(class.as_str()),
            "direct proposal create callsite {path} has disallowed Phase4 classification {class}"
        );
        assert_ne!(
            class, "product_path",
            "product proposal creation must route through ReviewWorkflow, not direct ProposalStore"
        );
        classified_paths.insert(path, class);
    }

    let expected = expected_count_map("direct_proposal_write_surfaces");
    for (path, _) in expected {
        assert!(
            classified_paths.contains_key(&path),
            "direct proposal create callsite {path} must include a Phase4 source-map classification"
        );
    }
}

#[test]
fn single_system_phase4_review_workflow_outcome_is_authoritative() {
    let inventory = inventory();
    let mut product_paths = BTreeSet::new();
    for entry in inventory_entries(&inventory, "phase4_proposal_creation_source_map") {
        if entry_str(entry, "phase4_classification") == "product_path"
            && entry_str(entry, "proposal_creation_route") == "ReviewWorkflow"
        {
            product_paths.insert(entry_str(entry, "path").to_string());
        }
    }

    for required_path in [
        "openlife-core/src/agent/action_executor/core_os_tools.rs",
        "openlife-core/src/agent/action_executor/declarative_stubs.rs",
        "openlife-core/src/agent/action_executor/execution_tools.rs",
        "openlife-core/src/agent/action_executor/tool_executor.rs",
        "openlife-core/src/agent/maturation.rs",
        "openlife-core/src/agent/plan_execute.rs",
        "src-tauri/src/main_chat_proposal_support.rs",
        "src-tauri/src/main_chat_generation_support.rs",
        "src-tauri/src/main_chat_kernel.rs",
        "src-tauri/src/commands/agent.rs",
        "src-tauri/src/commands/builder.rs",
        "src-tauri/src/commands/calibration.rs",
        "src-tauri/src/commands/execution.rs",
    ] {
        assert!(
            product_paths.contains(required_path),
            "Phase4 source map must classify {required_path} as a ReviewWorkflow product path"
        );
        let raw_source = read_repo_file(required_path);
        let source = strip_cfg_test_module(&raw_source);
        assert!(
            source.contains("ReviewWorkflow::new"),
            "Phase4 product path {required_path} must call ReviewWorkflow"
        );
        assert!(
            source.contains("outcome.proposal_id()") || source.contains("outcome.proposal"),
            "Phase4 product path {required_path} must consume ReviewWorkflowOutcome as the authoritative proposal fact"
        );
    }

    let retired_stage4_memory_knowledge = "src-tauri/src/main_chat_stage4_memory_knowledge.rs";
    assert!(
        !repo_root().join(retired_stage4_memory_knowledge).exists(),
        "Phase7 contract requires the historical Stage4 memory/knowledge shell to be deleted from the product crate"
    );
    let memory_proposal_raw = read_repo_file("src-tauri/src/main_chat_memory_proposals.rs");
    let memory_proposal_source = strip_cfg_test_module(&memory_proposal_raw);
    assert!(
        memory_proposal_source.contains("update_proposal(&proposal)")
            && memory_proposal_source.contains("durable_write_executed: false"),
        "Phase7 keeps only the focused memory proposal draft-edit helper; it must not recreate the old Stage4 proposal route"
    );

    for (path, forbidden) in [
        (
            "src-tauri/src/main_chat_generation_support.rs",
            "let proposal_id = proposal.id.clone();",
        ),
        (
            "src-tauri/src/main_chat_generation_support.rs",
            "add_generated_proposal(&agent_run.id, &proposal.id)",
        ),
        (
            "src-tauri/src/main_chat_proposal_support.rs",
            "let proposal_id = proposal.id.clone();",
        ),
        (
            "src-tauri/src/commands/builder.rs",
            "store.add_generated_proposal(&run_id, &proposal.id)",
        ),
        (
            "src-tauri/src/commands/calibration.rs",
            "created_ids.push(proposal.id",
        ),
        (
            "src-tauri/src/commands/agent.rs",
            "run.add_generated_proposal(&proposal.id)",
        ),
        (
            "src-tauri/src/commands/execution.rs",
            "run.add_generated_proposal(&proposal.id)",
        ),
        (
            "openlife-core/src/agent/maturation.rs",
            "candidate_proposal_id = Some(proposal.id",
        ),
        (
            "openlife-core/src/agent/maturation.rs",
            "proposal_ids.push(proposal.id",
        ),
        (
            "openlife-core/src/agent/plan_execute.rs",
            "let proposal_id = proposal.id.clone();",
        ),
        ("openlife-core/src/agent/plan_execute.rs", "Ok(proposal_id)"),
    ] {
        let raw_source = read_repo_file(path);
        let source = strip_cfg_test_module(&raw_source);
        assert!(
            !source.contains(forbidden),
            "Phase4 product path {path} must not keep using pre-submit proposal ids: {forbidden}"
        );
    }
}

#[test]
fn single_system_phase5_product_memory_lifemodel_writes_use_gateways() {
    let product_write_paths = [
        "src-tauri/src/commands/chat.rs",
        "src-tauri/src/commands/memory.rs",
        "src-tauri/src/commands/proposal.rs",
        "src-tauri/src/commands/settings.rs",
        "src-tauri/src/commands/state.rs",
        "src-tauri/src/main_chat_generation_support.rs",
        "src-tauri/src/main_chat_kernel.rs",
    ];
    let forbidden_bottom_write_markers = [
        ".save_message(",
        ".save_memory_record(",
        ".record_state_entry(",
        ".insert(&session_id",
        ".insert_batch(",
        ".replace_all_messages(",
        ".replace_all_chunks(",
        ".archive_lifecycle_memory_records(",
        ".rollback_memory_asset(",
        ".rebuild_materialized_view(",
        "manager.save(",
    ];

    for path in product_write_paths {
        let raw_source = read_repo_file(path);
        let source = strip_cfg_test_module(&raw_source);
        for marker in forbidden_bottom_write_markers {
            assert!(
                !source.contains(marker),
                "Phase5 product path {path} must route {marker} through MemoryGateway or LifeModelWriteGateway"
            );
        }
    }

    let memory_gateway = read_repo_file("src-tauri/src/memory_gateway.rs");
    for marker in [
        ".save_message(",
        ".save_memory_record(",
        ".record_state_entry(",
        ".insert(&session_id",
        ".insert_batch(",
        ".replace_all_messages(",
        ".replace_all_chunks(",
        ".archive_lifecycle_memory_records(",
        ".rollback_memory_asset(",
        ".rebuild_materialized_view(",
    ] {
        assert!(
            memory_gateway.contains(marker),
            "Phase5 MemoryGateway must own bottom memory write marker {marker}"
        );
    }

    let lifemodel_gateway = read_repo_file("src-tauri/src/life_model_write_gateway.rs");
    assert!(
        lifemodel_gateway.contains("manager.save("),
        "Phase5 LifeModelWriteGateway must own LifeModelManager::save"
    );
    assert!(
        !strip_cfg_test_module(&read_repo_file("src-tauri/src/lib.rs")).contains("manager.save("),
        "Phase5 persist_life_model compatibility wrapper must not save directly"
    );

    let proposal_raw_source = read_repo_file("src-tauri/src/commands/proposal.rs");
    let proposal_source = strip_cfg_test_module(&proposal_raw_source);
    let edit_body = source_between(
        proposal_source,
        "pub(crate) async fn edit_proposal_with_state",
        "pub(crate) async fn postpone_proposal_with_state",
    );
    assert!(
        !edit_body.contains("apply_proposal_to_state"),
        "Phase5 edit_proposal_with_state must edit pending review data only and must not materialize durable writes"
    );

    let memory_materialize_body = source_between(
        &memory_gateway,
        "pub(crate) async fn materialize_memory_proposal_with_state",
        "pub(crate) async fn archive_memory_for_proposal_with_state",
    );
    assert!(
        !memory_materialize_body.contains("MemoryGatewaySubject::Preference"),
        "Phase5 MemoryWrite materialization must classify real proposal payloads, not hard-code Preference"
    );
    assert!(
        memory_materialize_body.contains("MemoryGatewayRequest::from_proposal")
            || memory_materialize_body.contains("memory_gateway_decision_for_proposal"),
        "Phase5 MemoryWrite materialization must route through MemoryGatewayRequest"
    );

    let lifemodel_materialize_body = source_between(
        &lifemodel_gateway,
        "pub(crate) async fn materialize_accepted_lifemodel_proposal_with_state",
        "pub(crate) async fn restore_life_model_with_gateway",
    );
    assert!(
        !lifemodel_materialize_body.contains("Some(before_model_hash.clone()),\n        Some(before_model_hash.clone())"),
        "Phase5 LifeModelWriteGatewayRequest must not pass the current hash as both base_hash and current_hash"
    );
    assert!(
        lifemodel_materialize_body.contains("proposal.base_hash.clone()"),
        "Phase5 accepted proposal materialization must read base_hash from the proposal"
    );
}

#[test]
fn single_system_phase6_product_tool_execution_uses_tool_gateway() {
    let mut direct_executor_files = BTreeSet::new();
    for file in source_files(&["src-tauri/src", "openlife-core/src"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with("_tests.rs") || rel.contains("/tests/") {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        let stripped = strip_cfg_test_module(&source);
        if stripped.contains("ActionExecutor::new(") {
            direct_executor_files.insert(rel);
        }
    }

    let expected = BTreeSet::from(["openlife-core/src/agent/tool_gateway.rs".to_string()]);
    assert_eq!(
        direct_executor_files, expected,
        "Phase6 product execution must instantiate ActionExecutor only behind ToolGateway"
    );

    let gateway = read_repo_file("openlife-core/src/agent/tool_gateway.rs");
    for marker in [
        "validate_manifest_execution_contract",
        "tool_gateway_capability_contract_missing",
        "tool_gateway_risk_contract_missing",
        "tool_gateway_action_type_contract_missing",
        "tool_gateway_permission_contract_missing",
        "tool_gateway_manifest_disabled",
        "tool_gateway_manifest_declarative_only",
        "migration:name_inferred_contract",
    ] {
        assert!(
            gateway.contains(marker),
            "Phase6 ToolGateway must fail closed for explicit manifest contract marker {marker}"
        );
    }

    let lib_source = read_repo_file("src-tauri/src/lib.rs");
    let lib = strip_cfg_test_module(&lib_source);
    assert!(
        !source_between(lib, "tauri::generate_handler![", "])").contains("grant_tool_permission"),
        "Phase6 direct grant_tool_permission IPC must not stay mounted as product authority"
    );
}

#[test]
fn single_system_phase6_final_delivery_is_canonical_and_non_overclaiming() {
    let runtime_source = read_repo_file("src-tauri/src/main_chat_turn_runtime.rs");
    let source = strip_cfg_test_module(&runtime_source);

    assert!(
        source.contains("pub struct CanonicalFinalDeliveryView"),
        "Phase6 final delivery must expose CanonicalFinalDeliveryView"
    );
    for status in [
        "\"completed\"",
        "\"completed_with_pending_items\"",
        "\"blocked\"",
        "\"failed\"",
        "\"cancelled\"",
    ] {
        assert!(
            source.contains(status),
            "CanonicalFinalDeliveryView must preserve allowed status {status}"
        );
    }
    for retired_status in ["\"delivered\"", "\"pending_user_action\""] {
        assert!(
            !source.contains(retired_status),
            "CanonicalFinalDeliveryView must not emit retired overclaim status {retired_status}"
        );
    }
    assert!(
        source.contains("!proposals.is_empty()")
            && source.contains("\"completed_with_pending_items\""),
        "Phase6 proposals must produce completed_with_pending_items instead of completed"
    );
    assert!(
        source.contains("pending_user_actions")
            && source.contains("completed_actions")
            && source.contains("observations_used")
            && source.contains("durable_changes")
            && source.contains("next_steps"),
        "CanonicalFinalDeliveryView must include actions, observations, proposals/blockers, pending actions, durable changes, and next steps"
    );
}

#[test]
fn single_system_phase6_frontend_product_status_reads_life_state_projection() {
    let projection_helper = read_repo_file("frontend/src/utils/lifeStateProjection.ts");
    for marker in [
        "findLifeStateSurface",
        "reviewRequiredCountFromProjection",
        "totalReviewRequiredCount",
    ] {
        assert!(
            projection_helper.contains(marker),
            "Phase6 projection helper must centralize product pending state marker {marker}"
        );
    }

    let product_status_files = [
        "frontend/src/pages/TodayPage.tsx",
        "frontend/src/pages/MailboxPage.tsx",
        "frontend/src/pages/ChatPage.tsx",
        "frontend/src/pages/CompanionPage.tsx",
        "frontend/src/pages/LifeModelPage.tsx",
        "frontend/src/pages/SettingsPage.tsx",
        "frontend/src/pages/settings/tabs/OverviewTab.tsx",
        "frontend/src/pages/settings/tabs/ReviewMemoryTab.tsx",
        "frontend/src/pages/settings/tabs/ToolsPermissionsTab.tsx",
        "frontend/src/pages/settings/tabs/AdvancedTab.tsx",
    ];
    let forbidden_raw_status_markers = [
        "pending_proposal_count",
        "high_risk_pending_proposal_count",
        "pending_builder_review_sessions",
        "unfinished_builder_sessions",
        "isSafeMode(",
        "getSafeModeReason(",
        "diagnosticsUsageReady",
        "diagnosticsUsageReadinessIssues",
    ];

    for path in product_status_files {
        let source = read_repo_file(path);
        if path == "frontend/src/pages/CompanionPage.tsx" {
            assert!(
                source.contains("<ChatPage companionMode"),
                "CompanionPage must inherit ChatPage projection-backed status"
            );
        } else {
            assert!(
                source.contains("LifeStateProjection") || source.contains("getLifeStateProjection"),
                "Phase6 product status file {path} must use LifeStateProjection"
            );
        }
        if matches!(
            path,
            "frontend/src/pages/TodayPage.tsx"
                | "frontend/src/pages/MailboxPage.tsx"
                | "frontend/src/pages/ChatPage.tsx"
                | "frontend/src/pages/LifeModelPage.tsx"
        ) {
            assert!(
                source.contains("reviewRequiredCountFromProjection"),
                "Phase6 product pending state in {path} must use the LifeStateProjection helper"
            );
        }
        for marker in forbidden_raw_status_markers {
            assert!(
                !source.contains(marker),
                "Phase6 product status file {path} must not derive covered state from raw marker {marker}"
            );
        }
    }

    let chat = read_repo_file("frontend/src/pages/ChatPage.tsx");
    let chat_pending_alert = source_between(
        &chat,
        "{/* Pending Proposals Alert */}",
        "{/* Chat mode selector */}",
    );
    assert!(
        chat.contains("const projectionPendingReviewCount = reviewRequiredCountFromProjection("),
        "Chat product pending state must be named and sourced from LifeStateProjection"
    );
    assert!(
        chat_pending_alert.contains("projectionPendingReviewCount"),
        "Chat pending banner must render from projection-backed count"
    );
    for forbidden in [
        "?? pendingProposals.length",
        "|| pendingProposals.length",
        "pendingProposals.length > 0",
    ] {
        assert!(
            !chat.contains(forbidden),
            "Chat must not use raw pendingProposals length as product pending authority: {forbidden}"
        );
    }

    let mailbox = read_repo_file("frontend/src/pages/MailboxPage.tsx");
    assert!(
        mailbox.contains("const mailboxReviewRequiredCount = reviewRequiredCountFromProjection("),
        "Mailbox top-level pending state must be named and sourced from LifeStateProjection"
    );
    assert!(
        !mailbox.contains("folderCounts.pending"),
        "Mailbox global pending badge must not use folderCounts.pending; folderCounts are list-filter details only"
    );

    let lifemodel = read_repo_file("frontend/src/pages/LifeModelPage.tsx");
    let builder_review_counter = source_between(
        &lifemodel,
        "function countBuilderReviewItems",
        "function formatProjectionCount",
    );
    assert!(
        lifemodel.contains(
            "const pendingCount = reviewRequiredCountFromProjection(state.projection, \"life_model\")"
        ),
        "LifeModel product pending count must come from LifeStateProjection helper"
    );
    assert!(
        builder_review_counter.contains("projection.readiness.pendingBuilderReviewSessions"),
        "LifeModel builder review state must come from projection readiness"
    );
    for forbidden in ["pendingProposals", "Math.max", "proposalCount"] {
        assert!(
            !builder_review_counter.contains(forbidden),
            "LifeModel builder review state must not fallback to raw proposal data: {forbidden}"
        );
    }

    let today = read_repo_file("frontend/src/pages/TodayPage.tsx");
    assert!(
        today.contains(
            "const pendingCount = reviewRequiredCountFromProjection(state.projection, \"today\")"
        ),
        "Today pending state must come from LifeStateProjection helper"
    );
    for (path, source) in [
        ("ChatPage", chat.as_str()),
        ("MailboxPage", mailbox.as_str()),
        ("LifeModelPage", lifemodel.as_str()),
        ("TodayPage", today.as_str()),
    ] {
        for forbidden in [
            "?? pendingProposals.length",
            "|| pendingProposals.length",
            "?? folderCounts.pending",
            "|| folderCounts.pending",
            "pending.totalReviewRequiredCount ?? 0",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} must not turn projection-missing pending state into a raw or fake definite count: {forbidden}"
            );
        }
    }
}

#[test]
fn single_system_direct_memory_lifemodel_write_callsites_match_inventory() {
    let expected = expected_count_map("direct_memory_lifemodel_write_surfaces");
    let needles = [
        "persist_life_model(",
        "manager.save(",
        ".save_memory_record(",
        ".record_state_entry(",
        ".insert(&session_id",
        "replace_all_messages(",
        "replace_all_chunks(",
        "archive_lifecycle_memory_records(",
        "rollback_memory_asset(",
        "restore_archived_chunks(",
        "archive_low_access_memories(",
    ];
    let mut actual = BTreeMap::new();
    for file in source_files(&["src-tauri/src", "openlife-core/src"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with("_tests.rs") || rel.contains("/tests/") {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        let stripped = strip_cfg_test_module(&source);
        let count: usize = needles
            .iter()
            .map(|needle| count_occurrences(stripped, needle))
            .sum();
        if count > 0 {
            actual.insert(rel, count);
        }
    }
    assert_eq!(
        actual, expected,
        "direct memory/LifeModel write callsites must be registered in Phase 1 inventory"
    );
}

fn phase7_old_route_marker_allowlist(rel: &str) -> bool {
    rel.ends_with("_tests.rs")
        || rel.ends_with(".test.ts")
        || rel.ends_with(".test.tsx")
        || rel.contains("/tests/")
        || rel.contains("frontend/src/test/")
        || rel == "frontend/src/tauriDev.ts"
        || rel == "src-tauri/src/single_system_authority_tests.rs"
}

#[test]
fn single_system_phase7_forbids_old_route_markers_in_product_source() {
    let markers = [
        "main_chat_agent_beta_v1",
        "main_chat_agent_stage1",
        "main_chat_agent_stage2",
        "main_chat_stage3",
        "main_chat_stage4",
        "main_chat_stage5",
        "main_chat_step6",
        "main_chat_agent_productization",
        "main_chat_live_productization",
        "main_chat_product_maturity",
        "run_multi_strategy_agent_preview",
        "check_runtime_migration_gate",
        "controlled_chat_migration",
        "controlled_chat_cutover",
        "multi_strategy",
        "legacy_write_convergence",
    ];
    let mut violations = Vec::new();
    for file in source_files(&["src-tauri/src", "openlife-core/src", "frontend/src"]) {
        let rel = to_repo_path(&file);
        if phase7_old_route_marker_allowlist(&rel) {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        let stripped = strip_cfg_test_module(&source);
        for marker in markers {
            if stripped.contains(marker) {
                violations.push(format!("{rel}:{marker}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Phase7 contract requires old route markers to be absent from product source; violations: {violations:?}"
    );
}

#[test]
fn single_system_phase7_forbids_old_modules_in_product_module_graph() {
    let module_graph = [
        (
            "src-tauri/src/lib.rs",
            read_repo_file("src-tauri/src/lib.rs"),
        ),
        (
            "src-tauri/src/commands/agent_runtime/mod.rs",
            read_repo_file("src-tauri/src/commands/agent_runtime/mod.rs"),
        ),
        (
            "openlife-core/src/agent/mod.rs",
            read_repo_file("openlife-core/src/agent/mod.rs"),
        ),
    ];
    let forbidden = [
        "main_chat_agent_beta_v1",
        "main_chat_agent_stage1",
        "main_chat_agent_stage2",
        "main_chat_stage3",
        "main_chat_stage4",
        "main_chat_stage5",
        "main_chat_step6",
        "main_chat_agent_productization",
        "main_chat_live_productization",
        "main_chat_product_maturity",
        "migration_ladder",
        "multi_strategy_runtime",
        "react_beta",
        "runtime_migration_gate",
    ];
    let mut violations = Vec::new();
    for (rel, source) in module_graph {
        for marker in forbidden {
            if source.contains(marker) {
                violations.push(format!("{rel}:{marker}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Phase7 contract requires old stage/beta/productization/migration modules to be absent from the product module graph: {violations:?}"
    );
}

#[test]
fn single_system_phase7_frontend_product_pages_do_not_import_dev_bridge_or_legacy_status() {
    let mut violations = Vec::new();
    for file in source_files(&["frontend/src/pages", "frontend/src/components"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with(".test.tsx") || rel.ends_with(".test.ts") {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        for marker in ["tauriDev", "legacyFallbackUsed", "legacy_fallback_used"] {
            if source.contains(marker) {
                violations.push(format!("{rel}:{marker}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Phase7 contract requires product frontend pages/components to avoid dev bridge imports and legacy fallback status fields: {violations:?}"
    );
}

#[test]
fn single_system_phase7_active_docs_do_not_authorize_old_routes() {
    let active_docs = [
        ("README.md", read_repo_file("README.md")),
        ("plans/README.md", read_repo_file("plans/README.md")),
    ];
    let forbidden_active_phrases = [
        "run_multi_strategy_agent_preview",
        "check_runtime_migration_gate",
        "default Chat migration",
        "OpenLife ReAct Beta Roadmap",
        "Main Chat Agent Migration v1 Goal Spec",
        "legacy_stream",
    ];
    let mut violations = Vec::new();
    for (rel, source) in active_docs {
        for phrase in forbidden_active_phrases {
            if source.contains(phrase) {
                violations.push(format!("{rel}:{phrase}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Phase7 contract requires active docs to point at the single-system path, not old Stage/Beta/Migration routes: {violations:?}"
    );
}
