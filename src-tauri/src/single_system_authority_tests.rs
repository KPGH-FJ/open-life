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
        "direct_memory_lifemodel_write_surfaces",
        "frontend_multi_source_state_surfaces",
        "stage_beta_migration_command_surfaces",
    ];
    let allowed_dispositions = BTreeSet::from([
        "keep",
        "absorb_then_delete",
        "delete",
        "storage_only",
        "test_fixture_only",
        "archive_reference",
    ]);

    for category in required_categories {
        let entries = inventory_entries(&inventory, category);
        assert!(
            !entries.is_empty(),
            "inventory category {category} must not be empty"
        );
        for entry in entries {
            let id = entry_str(entry, "id");
            let path = entry_str(entry, "path");
            let disposition = entry_str(entry, "disposition");
            assert!(!id.trim().is_empty(), "inventory id must be non-empty");
            assert!(
                repo_root().join(path).exists(),
                "inventory path must exist: {path}"
            );
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
fn single_system_handler_old_commands_match_inventory_and_manifest() {
    let lib = read_repo_file("src-tauri/src/lib.rs");
    let handler = lib
        .split("tauri::generate_handler![")
        .nth(1)
        .and_then(|rest| rest.split("])").next())
        .expect("Tauri generate_handler body");
    let old_tokens = ["stage", "beta", "migration", "cutover", "dogfood", "eval"];
    let actual: BTreeSet<String> = handler
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .filter(|token| !token.is_empty())
        .filter(|token| old_tokens.iter().any(|old| token.contains(old)))
        .map(str::to_string)
        .collect();

    let inventory = inventory();
    let expected: BTreeSet<String> =
        inventory_entries(&inventory, "stage_beta_migration_command_surfaces")
            .into_iter()
            .map(|entry| entry_str(entry, "command").to_string())
            .collect();
    assert_eq!(
        actual, expected,
        "stage/beta/migration/cutover/dogfood/eval shipped handler commands must match Phase 1 inventory"
    );

    let manifest = read_repo_file("plans/openlife_single_system_deletion_manifest.md");
    for command in actual {
        assert!(
            manifest.contains(&format!("`{command}`")),
            "deletion manifest must classify shipped old command {command}"
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

#[test]
fn single_system_product_old_route_markers_match_inventory() {
    let inventory = inventory();
    let expected: BTreeMap<String, usize> =
        inventory_entries(&inventory, "product_old_route_markers")
            .into_iter()
            .map(|entry| {
                (
                    entry_str(entry, "marker").to_string(),
                    entry_u64(entry, "expected_file_count") as usize,
                )
            })
            .collect();

    let mut actual = BTreeMap::new();
    for marker in expected.keys() {
        let mut count = 0;
        for file in source_files(&["src-tauri/src", "openlife-core/src", "frontend/src"]) {
            let rel = to_repo_path(&file);
            if rel == "src-tauri/src/single_system_authority_tests.rs" {
                continue;
            }
            let source =
                fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
            if source.contains(marker) {
                count += 1;
            }
        }
        actual.insert(marker.clone(), count);
    }

    assert_eq!(
        actual, expected,
        "old product route markers must stay registered in Phase 1 inventory until their deletion phase"
    );
}
