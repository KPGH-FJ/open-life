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
    [
        "\n#[cfg(test)]\nmod tests",
        "\n#[cfg(test)]\nmod bound_content_receipt_tests",
    ]
    .into_iter()
    .filter_map(|marker| source.find(marker))
    .min()
    .map_or(source, |index| &source[..index])
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

#[test]
fn explicit_provider_probe_authority_has_one_opaque_product_issuance_route() {
    let network_client = read_repo_file("openlife-core/src/network_client.rs");
    let grant_fields = source_between(
        &network_client,
        "pub struct ExplicitProviderProbeGrant {",
        "}\n\n/// Non-authorizing",
    );
    assert!(
        !grant_fields.contains("pub ") && !grant_fields.contains("pub("),
        "ExplicitProviderProbeGrant fields must remain private so sibling modules cannot forge a struct literal"
    );
    assert!(
        !network_client.contains("pub fn issue_explicit_provider_probe_grant"),
        "the retired raw policy/decision/string grant constructor must stay absent"
    );
    assert!(
        network_client.contains("pub(crate) struct ExplicitProviderProbeIssuer"),
        "the provider-probe issuer must remain crate-private"
    );
    assert!(
        !network_client.contains("pub struct ExplicitProviderProbeIssuer"),
        "the provider-probe issuer must never become a public capability"
    );
    let tool_permissions = read_repo_file("openlife-core/src/tool_permissions.rs");
    let reviewed_network_grant = source_between(
        &tool_permissions,
        "pub fn grant_reviewed_network_once(",
        "    pub fn grant_action_bound(",
    );
    assert!(
        reviewed_network_grant.contains("ClaimedReviewAcceptanceSnapshot"),
        "reviewed network AllowOnce creation must consume non-serializable ReviewWorkflow proof"
    );
    let probe_issuance = source_between(
        &tool_permissions,
        "pub fn issue_explicit_provider_probe_grant(",
        "    pub fn consume_reviewed_network_once(",
    );
    assert!(
        probe_issuance.contains("Option<ConsumedReviewedNetworkPermission>")
            && !probe_issuance.contains("permission_id: Option<&str>"),
        "provider-probe issuance must consume opaque AllowOnce proof, never a serialized permission id"
    );

    for path in source_files(&["openlife-core/src"]) {
        let repo_path = to_repo_path(&path);
        if repo_path == "openlife-core/src/network_client.rs" {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read core Rust source");
        let production = strip_cfg_test_module(&source);
        if production.contains("create_explicit_provider_probe_authority") {
            assert_eq!(
                repo_path, "openlife-core/src/tool_permissions.rs",
                "only the canonical ToolPermissionStore may own the paired probe authority factory"
            );
        }
        assert!(
            !production.contains("claim_explicit_provider_probe_issuer"),
            "schedulers and core product modules must not expose issuer claims: {repo_path}"
        );
        if production.contains("issue_governed_probe_grant") {
            assert_eq!(
                repo_path, "openlife-core/src/tool_permissions.rs",
                "only the canonical ToolPermissionStore may consume the opaque issuer"
            );
        }
        let has_grant_literal = production.lines().any(|line| {
            line.contains("= ExplicitProviderProbeGrant {")
                || line
                    .trim_start()
                    .starts_with("ExplicitProviderProbeGrant {")
        });
        assert!(
            !has_grant_literal,
            "core sibling modules must not construct an explicit provider probe grant: {repo_path}"
        );
    }

    for path in source_files(&["src-tauri/src"]) {
        let repo_path = to_repo_path(&path);
        if repo_path == "src-tauri/src/single_system_authority_tests.rs" {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read product Rust source");
        let production = strip_cfg_test_module(&source);
        assert!(
            !production.contains("claim_explicit_provider_probe_issuer")
                && !production.contains("issue_governed_probe_grant")
                && !production.contains("create_explicit_provider_probe_authority"),
            "Tauri product code must not own or consume provider-probe issuers: {repo_path}"
        );
        if production.contains("grant_reviewed_network_once(") {
            assert_eq!(
                repo_path, "src-tauri/src/commands/proposal.rs",
                "only accepted Proposal application may materialize reviewed network AllowOnce"
            );
        }
        let has_grant_literal = production.lines().any(|line| {
            line.contains("= ExplicitProviderProbeGrant {")
                || line
                    .trim_start()
                    .starts_with("ExplicitProviderProbeGrant {")
        });
        assert!(
            !has_grant_literal,
            "product source must not construct an explicit provider probe grant: {repo_path}"
        );
    }

    let scheduler_source = read_repo_file("openlife-core/src/scheduler.rs");
    let scheduler = strip_cfg_test_module(&scheduler_source);
    assert!(scheduler.contains(
        "explicit_provider_probe_verifier: Option<crate::network_client::ExplicitProviderProbeVerifier>"
    ));
    assert!(scheduler.contains("explicit_provider_probe_verifier: None"));
    assert!(scheduler.contains("verifier is not bound by ToolPermissionStore"));
    assert!(!scheduler.contains("create_explicit_provider_probe_authority"));

    let governance = read_repo_file("src-tauri/src/provider_network_consent.rs");
    assert!(
        governance.contains(".issue_explicit_provider_probe_grant("),
        "provider network governance must consume the canonical ToolPermissionStore authority"
    );
}

#[test]
fn replay_reconciliation_requires_event_store_attestation_and_one_product_delivery_route() {
    let issuer_call = ".issue_replay_prepared_tool_reconciliation_authority_binding(";
    let apply_call = ".apply_prepared_tool_reconciliation_after_restart(";
    let mut issuer_calls = BTreeMap::new();
    let mut apply_calls = BTreeMap::new();
    for file in source_files(&["openlife-core/src", "src-tauri/src"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with("_tests.rs")
            || rel.contains("/tests/")
            || rel == "src-tauri/src/single_system_authority_tests.rs"
        {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        let stripped = if rel == "openlife-core/src/agent/main_chat_agent_v1.rs" {
            source
                .split("\n#[cfg(test)]\nmod action_queue_replay_claim_tests")
                .next()
                .unwrap_or(&source)
        } else {
            strip_cfg_test_module(&source)
        };
        let issuer_count = count_occurrences(stripped, issuer_call);
        if issuer_count > 0 {
            issuer_calls.insert(rel.clone(), issuer_count);
        }
        let apply_count = count_occurrences(stripped, apply_call);
        if apply_count > 0 {
            apply_calls.insert(rel, apply_count);
        }
    }
    assert_eq!(
        issuer_calls,
        BTreeMap::new(),
        "ActionQueue must not expose a reconciliation issuer over caller-constructible envelopes"
    );
    assert_eq!(
        apply_calls,
        BTreeMap::from([("src-tauri/src/bootstrap.rs".to_string(), 1)]),
        "only trusted bootstrap delivery may apply the signed reconciliation envelope"
    );

    let bootstrap = read_repo_file("src-tauri/src/bootstrap.rs");
    let load = bootstrap
        .find(".pending_tool_queue_reconciliation_projections(")
        .expect("bootstrap loads EventStore-validated projections");
    let apply = bootstrap
        .find("apply_tool_queue_reconciliation_projection(&queue, &projection)")
        .expect("bootstrap calls the sole EventStore-attested reconciliation bridge");
    let acknowledge = bootstrap
        .find(".mark_tool_queue_reconciliation_projection_applied(")
        .expect("bootstrap acknowledges the exact EventStore projection");
    assert!(
        load < apply && apply < acknowledge,
        "release bootstrap must load the EventStore-attested projection, apply it, then acknowledge the outbox in that order"
    );
    assert!(
        bootstrap.contains("event_store_attestation: &projection.event_store_attestation"),
        "bootstrap must pass through the EventStore attestation instead of minting ActionQueue authority"
    );
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
        "backend_readmodel_review_authority_surfaces",
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
        assert!(
            inventory
                .get(category)
                .is_some_and(serde_json::Value::is_array),
            "inventory category {category} must exist as an array"
        );
        let entries = inventory_entries(&inventory, category);
        if category != "direct_proposal_write_surfaces" {
            assert!(
                !entries.is_empty(),
                "inventory category {category} must not be empty"
            );
        }
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
fn single_system_r0_readmodel_authority_map_keeps_frontend_adapters_transitional() {
    let inventory = inventory();
    let entries = inventory_entries(&inventory, "backend_readmodel_review_authority_surfaces");
    let mut by_path = BTreeMap::new();
    let frontend_allowed_classes = BTreeSet::from([
        "frontend_product_bridge",
        "frontend_transitional_adapter",
        "frontend_preview_adapter",
    ]);

    for entry in entries {
        let path = entry_str(entry, "path").to_string();
        let classification = entry_str(entry, "r0_authority_classification").to_string();
        let read_model_status = entry_str(entry, "read_model_status").to_string();
        let backend_owner = entry_bool(entry, "backend_owner");

        if path.starts_with("frontend/") {
            assert!(
                !backend_owner,
                "R0 frontend path {path} must not be classified as a backend read-model owner"
            );
            assert!(
                frontend_allowed_classes.contains(classification.as_str()),
                "R0 frontend path {path} must use a transitional/frontend classification, got {classification}"
            );
            assert_ne!(
                read_model_status, "implemented_backend_view_model",
                "R0 frontend path {path} must not claim implemented backend ViewModel ownership"
            );
        }

        by_path.insert(path, (classification, backend_owner, read_model_status));
    }

    for (path, backend_owner, status) in [
        (
            "src-tauri/src/life_state_projection.rs",
            true,
            "partial_backend_read_model",
        ),
        (
            "openlife-core/src/agent/product_read_model.rs",
            true,
            "implemented_backend_shared_contract",
        ),
        (
            "openlife-core/src/agent/review_item.rs",
            true,
            "implemented_review_item_contract",
        ),
        (
            "src-tauri/src/read_models/review_center.rs",
            true,
            "implemented_review_center_view_model_command",
        ),
        (
            "openlife-core/src/agent/life_model_view_model.rs",
            true,
            "implemented_lifemodel_view_model_contract",
        ),
        (
            "src-tauri/src/read_models/life_model.rs",
            true,
            "implemented_lifemodel_view_model_command",
        ),
        (
            "openlife-core/src/agent/tasks_view_model.rs",
            true,
            "implemented_tasks_workspace_view_model_contract",
        ),
        (
            "src-tauri/src/read_models/tasks.rs",
            true,
            "implemented_tasks_workspace_view_model_commands",
        ),
        (
            "openlife-core/src/agent/review_workflow.rs",
            true,
            "review_governance_boundary_not_review_center",
        ),
        (
            "openlife-core/src/agent/proposal_store.rs",
            false,
            "storage_only_not_read_model_owner",
        ),
        (
            "openlife-core/src/agent/backend_contract_freeze.rs",
            true,
            "partial_proposal_review_read_model",
        ),
        (
            "openlife-core/src/memory_gateway.rs",
            true,
            "memory_gateway_primitives_consumed_by_memory_view_model",
        ),
        (
            "openlife-core/src/agent/memory_view_model.rs",
            true,
            "implemented_memory_view_model_contract",
        ),
        (
            "src-tauri/src/read_models/memory.rs",
            true,
            "implemented_memory_view_model_command",
        ),
        (
            "openlife-core/src/life_model_write_gateway.rs",
            true,
            "materialization_gateway_not_view_model",
        ),
        (
            "openlife-core/src/agent/main_chat_runtime_contract.rs",
            true,
            "task_runtime_primitives_consumed_by_tasks_view_model",
        ),
        (
            "openlife-core/src/tasks.rs",
            true,
            "task_store_primitives_not_tasks_view_model_owner",
        ),
        (
            "src-tauri/src/main_chat_task_controls.rs",
            true,
            "task_control_primitives_consumed_by_tasks_view_model",
        ),
        (
            "src-tauri/src/provider_validation.rs",
            true,
            "provider_validation_primitives_consumed_by_provider_privacy_boundary",
        ),
        (
            "openlife-core/src/agent/provider_privacy_boundary.rs",
            true,
            "implemented_provider_privacy_boundary_contract",
        ),
        (
            "src-tauri/src/read_models/provider_privacy.rs",
            true,
            "implemented_provider_privacy_boundary_command",
        ),
        (
            "frontend/src/tauri.ts",
            false,
            "frontend_bridge_mirror_not_backend_owner",
        ),
        (
            "frontend/src/viewmodels/shared/viewModelEnvelope.ts",
            false,
            "frontend_alias_to_backend_contract_mirror",
        ),
        (
            "frontend/src/viewmodels/today/todayViewModelAdapter.ts",
            false,
            "frontend_only_preview_adapter",
        ),
        (
            "frontend/src/viewmodels/lifemodel/lifeModelViewModelAdapter.ts",
            false,
            "frontend_alias_to_backend_lifemodel_view_model",
        ),
    ] {
        let (_, actual_backend_owner, actual_status) = by_path
            .get(path)
            .unwrap_or_else(|| panic!("R0 read-model authority inventory missing {path}"));
        assert_eq!(
            *actual_backend_owner, backend_owner,
            "R0 read-model authority inventory has wrong backend_owner for {path}"
        );
        assert_eq!(
            actual_status, status,
            "R0 read-model authority inventory has wrong status for {path}"
        );
    }

    {
        let missing_backend_owner = "src-tauri/src/read_models/workspace.rs";
        assert!(
            !repo_root().join(missing_backend_owner).exists(),
            "R0 source map must be updated before treating {missing_backend_owner} as an existing backend read-model owner"
        );
    }
}

#[test]
fn single_system_r1_shared_viewmodel_contract_is_backend_owned() {
    let contract = read_repo_file("openlife-core/src/agent/product_read_model.rs");
    for required in [
        "pub struct ViewModelEnvelope",
        "pub enum ViewModelStatus",
        "pub struct EvidenceRef",
        "pub struct ProductAction",
        "pub struct DebugAction",
        "pub struct ReviewAction",
        "pub enum ReviewItemMaterializationStatus",
        "pub struct ProviderPrivacyBoundarySummary",
        "pub struct BackendEntityRef",
    ] {
        assert!(
            contract.contains(required),
            "R1 backend shared read-model contract must define {required}"
        );
    }
    for invariant in [
        "Self::Apply => ReviewActionEffect::MaterializationRequest",
        "Self::Resume => ReviewActionEffect::TaskResumeRequest",
        "Self::ViewEvidence => ReviewActionEffect::EvidenceOnly",
        "Self::Approve | Self::Reject | Self::Edit | Self::Later | Self::Revoke",
        "ReviewActionEffectMismatch",
    ] {
        assert!(
            contract.contains(invariant),
            "R1 ReviewAction kind/effect invariant must include {invariant}"
        );
    }

    let agent_mod = read_repo_file("openlife-core/src/agent/mod.rs");
    assert!(
        agent_mod.contains("pub mod product_read_model;"),
        "R1 backend shared contract module must be exported from agent::mod"
    );
    assert!(
        agent_mod.contains("ViewModelEnvelope") && agent_mod.contains("ReviewActionEffect"),
        "R1 backend shared contract types must be re-exported for downstream owners"
    );

    let tauri_bridge = read_repo_file("frontend/src/tauri.ts");
    assert!(
        tauri_bridge
            .contains("Canonical Rust owner: openlife-core/src/agent/product_read_model.rs")
            && tauri_bridge.contains("export type ViewModelEnvelope<T>")
            && tauri_bridge.contains("export type ReviewActionKindEffectInvariant")
            && tauri_bridge.contains("export type ProviderPrivacyBoundarySummary"),
        "R1 frontend bridge must mirror backend shared contract types without claiming ownership"
    );

    let shared_frontend = read_repo_file("frontend/src/viewmodels/shared/viewModelEnvelope.ts");
    assert!(
        shared_frontend.contains("Transitional frontend import path")
            && shared_frontend.contains("openlife-core/src/agent/product_read_model.rs")
            && shared_frontend.contains("from \"../../tauri\""),
        "R1 shared frontend ViewModel types must be a transitional alias to the backend mirror"
    );
    assert!(
        !shared_frontend.contains("export type ViewModelStatus = \"loading\""),
        "R1 shared frontend ViewModel file must not keep standalone canonical status definitions"
    );
}

#[test]
fn single_system_r2_review_center_readmodel_owns_review_actions() {
    let review_item = read_repo_file("openlife-core/src/agent/review_item.rs");
    for required in [
        "pub struct ReviewItem",
        "pub struct ReviewCenterViewModel",
        "pub struct ReviewCenterBuildInput",
        "pub struct ReviewItemTaskResumeRelation",
        "pub fn build_review_center_view_model",
        "pub fn build_review_item",
        "pub materialization_status: ReviewItemMaterializationStatus",
        "pub allowed_actions: Vec<ReviewAction>",
        "pub task_resume_relation: Option<ReviewItemTaskResumeRelation>",
        "pub resume_requires_materialization: bool",
        "ReviewActionKind::Approve",
        "ReviewActionKind::Apply",
        "ReviewActionKind::Resume",
    ] {
        assert!(
            review_item.contains(required),
            "R2 backend ReviewItem contract must include {required}"
        );
    }
    assert!(
        review_item
            .contains("ProposalStatus::Accepted => ReviewItemMaterializationStatus::Unknown"),
        "R2 accepted proposal status must not become applied without backend materialization proof"
    );
    assert!(
        review_item.contains("resume_requires_materialization(proposal.proposal_type)")
            && review_item
                .contains("Materialization evidence is unknown; cannot request task resume yet.")
            && !review_item
                .contains("let can_request_resume = status == ReviewItemDecisionStatus::Approved;"),
        "R2 resume eligibility must not be inferred from Approved alone"
    );

    let review_center = read_repo_file("src-tauri/src/read_models/review_center.rs");
    for required in [
        "pub async fn get_review_center_view_model",
        "build_review_center_view_model",
        "list_all_proposals(100, 0)",
        "get_record_by_proposal_id",
        "materialization_status_from_memory_lifecycle",
    ] {
        assert!(
            review_center.contains(required),
            "R2 Tauri ReviewCenterViewModel command must include {required}"
        );
    }

    let lib = read_repo_file("src-tauri/src/lib.rs");
    assert!(
        lib.contains("pub(crate) mod read_models;")
            && lib.contains("use read_models::review_center::get_review_center_view_model;")
            && lib.contains("get_review_center_view_model,"),
        "R2 ReviewCenterViewModel must be an actual registered Tauri command, not only a frontend type"
    );

    let tauri_bridge = read_repo_file("frontend/src/tauri.ts");
    assert!(
        tauri_bridge.contains("export type ReviewItem =")
            && tauri_bridge.contains("export type ReviewCenterViewModel")
            && tauri_bridge.contains("getReviewCenterViewModel()")
            && tauri_bridge.contains("\"get_review_center_view_model\""),
        "R2 frontend bridge must mirror the ReviewCenterViewModel command shape"
    );

    let mailbox = read_repo_file("frontend/src/pages/MailboxPage.tsx");
    for required in [
        "getReviewCenterViewModel",
        "allowedActions.find",
        "actionBlockedReason(selectedReviewItem, \"approve\")",
        "materializationStatus",
        "ReviewCenterViewModel 尚未提供该确认项的后端操作状态",
    ] {
        assert!(
            mailbox.contains(required),
            "R2 Mailbox must consume backend ReviewItem action/materialization authority: {required}"
        );
    }
    for forbidden in [
        "function canAccept(",
        "function isPathInSafePaths(",
        "function acceptBlockedReason(",
        "function appliedNotice(",
        "setProposals(prev =>",
    ] {
        assert!(
            !mailbox.contains(forbidden),
            "R2 Mailbox must not keep local review action/materialization authority pattern: {forbidden}"
        );
    }
}

#[test]
fn single_system_r3_lifemodel_viewmodel_is_backend_owned() {
    let contract = read_repo_file("openlife-core/src/agent/life_model_view_model.rs");
    for required in [
        "pub struct LifeModelViewModel",
        "pub struct LifeModelViewModelBuildInput",
        "pub struct LifeModelPendingUpdateCounts",
        "pub struct LifeModelManualOverrideState",
        "pub struct LifeModelMemoryLinkageSummary",
        "pub fn build_life_model_view_model_envelope",
        "approved_not_applied",
        "has_materialization_proof",
        "patch_status.as_deref() == Some(\"applied\")",
        "!self.snapshot_versions.is_empty()",
        "current_matches_accepted_after",
    ] {
        assert!(
            contract.contains(required),
            "R3 backend LifeModelViewModel contract must include {required}"
        );
    }
    assert!(
        contract.contains("canonical_summary: None")
            && contract.contains("Accepted proposal decisions remain approved-not-applied"),
        "R3 LifeModelViewModel must fail closed on canonical and accepted proposal materialization claims"
    );

    let read_model = read_repo_file("src-tauri/src/read_models/life_model.rs");
    for required in [
        "pub async fn get_life_model_view_model",
        "build_life_model_view_model_envelope",
        "manager.load_existing()",
        "get_life_model_current_view_for_model_with_state",
        "get_review_center_view_model_with_state",
        "count_memory_chunks_with_state",
        "get_memory_tier_stats_with_state",
    ] {
        assert!(
            read_model.contains(required),
            "R3 Tauri LifeModelViewModel command must include {required}"
        );
    }
    assert!(
        !read_model.contains("get_life_model_with_state"),
        "R3 LifeModel read model must not use the older load() helper that can create a default LifeModel during read"
    );

    let lib = read_repo_file("src-tauri/src/lib.rs");
    assert!(
        lib.contains("use read_models::life_model::get_life_model_view_model;")
            && lib.contains("get_life_model_view_model,"),
        "R3 LifeModelViewModel must be an actual registered Tauri command"
    );

    let tauri_bridge = read_repo_file("frontend/src/tauri.ts");
    assert!(
        tauri_bridge.contains("export type LifeModelViewModel =")
            && tauri_bridge.contains("getLifeModelViewModel()")
            && tauri_bridge.contains("\"get_life_model_view_model\""),
        "R3 frontend bridge must mirror the LifeModelViewModel command shape"
    );

    let adapter = read_repo_file("frontend/src/viewmodels/lifemodel/lifeModelViewModelAdapter.ts");
    assert!(
        adapter.contains("getLifeModelViewModel")
            && !adapter.contains("buildLifeModelViewModelEnvelope")
            && !adapter.contains("BuildLifeModelViewModelInput"),
        "R3 frontend adapter must delegate to backend LifeModelViewModel, not rebuild raw truth"
    );

    let page = read_repo_file("frontend/src/pages/LifeModelPage.tsx");
    assert!(
        page.contains("getLifeModelViewModel")
            && page.contains("viewModel?.pendingUpdateCounts.pendingReview")
            && page.contains("viewModel?.memoryLinkage")
            && page.contains("viewModel?.candidateChanges"),
        "R3 LifeModel page must consume backend LifeModelViewModel fields"
    );
    for forbidden in [
        "getLifeModel(",
        "getLifeModelCurrentView(",
        "getSystemDiagnostics(",
        "getModel4DCompletion(",
        "countMemoryChunks(",
        "getMemoryTierStats(",
        "listProposals(",
        "buildLifeModelViewModelEnvelope",
        "getLifeModelQualityIssues",
        "buildLifeModelTrustViews",
    ] {
        assert!(
            !page.contains(forbidden),
            "R3 LifeModel page must not reconstruct backend truth locally via {forbidden}"
        );
    }
}

#[test]
fn single_system_r4_tasks_workspace_viewmodel_owns_task_lifecycle_controls() {
    let contract = read_repo_file("openlife-core/src/agent/tasks_view_model.rs");
    for required in [
        "pub struct TasksViewModel",
        "pub struct WorkspaceViewModel",
        "pub struct TaskViewModelItem",
        "pub struct TaskControl",
        "pub enum TaskLifecycleStatus",
        "pub enum TaskTerminalDeliveryStatus",
        "pub fn build_tasks_view_model",
        "pub fn build_workspace_view_model",
        "pub struct WorkspaceViewModelBuildInput",
        "pub struct WorkspaceActivityItem",
        "pub active_task: Option<TaskViewModelItem>",
        "pub pending_review_items: Vec<ReviewItem>",
        "pub activity: Vec<WorkspaceActivityItem>",
        "activity_redaction_state: \"metadata_only\"",
        "CompletedWithPendingReview",
        "CompletedNeedsEvidence",
        "TaskControlEffect::TaskResumeRequest",
        "TaskControlEffect::TaskRetryRequest",
        "TaskControlEffect::TaskCancelRequest",
        "TaskControlEffect::TaskRefreshRequest",
        "completion_proof_after_dispatch: false",
        "ControlClaimsCompletionProof",
    ] {
        assert!(
            contract.contains(required),
            "R4 backend TasksViewModel contract must include {required}"
        );
    }
    for invariant in [
        "completed_task_without_final_delivery_fails_closed",
        "completed_task_with_final_delivery_missing_status_fails_closed",
        "completed_task_with_completed_status_is_delivered",
        "completed_task_with_completed_with_pending_items_is_not_plain_completed",
        "completed_task_with_pending_review_is_not_plain_completed",
        "request_controls_do_not_claim_completion_after_dispatch",
    ] {
        assert!(
            contract.contains(invariant),
            "R4 backend TasksViewModel tests must lock fail-closed invariant {invariant}"
        );
    }
    assert!(
        !contract.contains(".unwrap_or(true)"),
        "R4 final delivery completion must fail closed when final_delivery_status is missing"
    );

    let read_model = read_repo_file("src-tauri/src/read_models/tasks.rs");
    for required in [
        "pub async fn get_tasks_view_model",
        "pub async fn get_workspace_view_model",
        "list_main_chat_agent_tasks_with_state",
        "get_main_chat_agent_task_detail_with_state",
        "get_review_center_view_model_with_state",
        "get_provider_privacy_boundary_summary_with_state",
        "state.agent_run_store",
        "build_tasks_view_model",
        "Workspace activity is metadata-only",
    ] {
        assert!(
            read_model.contains(required),
            "R4 Tauri TasksViewModel command must include {required}"
        );
    }
    let task_controls = read_repo_file("src-tauri/src/main_chat_task_controls.rs");
    assert!(
        task_controls.contains("\"status\": status")
            && task_controls.contains("final_delivery_status_from_task")
            && !task_controls.contains("\"source\": \"task_session_final_summary\""),
        "R4 TaskDetail final_delivery must carry status evidence and must not treat final_summary alone as delivery proof"
    );

    let lib = read_repo_file("src-tauri/src/lib.rs");
    assert!(
        lib.contains("use read_models::tasks::{get_tasks_view_model, get_workspace_view_model};")
            && lib.contains("get_tasks_view_model,")
            && lib.contains("get_workspace_view_model,"),
        "R4 TasksViewModel and WorkspaceViewModel must be actual registered Tauri commands"
    );

    let tauri_bridge = read_repo_file("frontend/src/tauri.ts");
    assert!(
        tauri_bridge.contains("export type TasksViewModel =")
            && tauri_bridge.contains("export type WorkspaceViewModel =")
            && tauri_bridge.contains("export type TaskControl =")
            && tauri_bridge.contains("getTasksViewModel()")
            && tauri_bridge.contains("getWorkspaceViewModel()")
            && tauri_bridge.contains("\"get_tasks_view_model\""),
        "R4 frontend bridge must mirror backend task/workspace read-model command shapes"
    );

    let runs = read_repo_file("frontend/src/pages/RunsPage.tsx");
    for required in [
        "getTasksViewModel",
        "TaskViewModelItem",
        "TaskControl",
        "item.lifecycleStatus",
        "item.terminalDeliveryStatus",
        "enabledActionControls(item)",
        "control.effect",
        "taskControl.targetActionId",
    ] {
        assert!(
            runs.contains(required),
            "R4 Runs page must consume backend TasksViewModel authority: {required}"
        );
    }
    for forbidden in [
        "listMainChatAgentTasks",
        "getMainChatAgentTaskDetail",
        "taskSummaryByRunId",
        "allowedControlsForSummary",
        "lifecycleForRun",
    ] {
        assert!(
            !runs.contains(forbidden),
            "R4 Runs page must not locally merge raw task lifecycle/control authority via {forbidden}"
        );
    }

    let chat = read_repo_file("frontend/src/pages/ChatPage.tsx");
    for required in [
        "getTasksViewModel",
        "currentTaskViewItem",
        "enabledTaskViewControl",
        "taskViewItem?.lifecycleStatus",
        "taskViewItem?.terminalDeliveryStatus",
        "Backend task read model did not enable resume",
        "canResumeCurrentMainChatTask",
        "canRetryCurrentMainChatTask",
        "canCancelCurrentMainChatTask",
    ] {
        assert!(
            chat.contains(required),
            "R4 Chat page must consume backend TasksViewModel authority: {required}"
        );
    }
    for forbidden in [
        "taskState?.canResume ? [\"resume\"]",
        "taskState?.canRetry ? [\"retry\"]",
        "canCancel ? [\"cancel\"]",
        "taskStatus === \"completed\" ||",
        "runStatus === \"completed\" ||",
        "disabled={!taskState?.canResume",
        "disabled={!currentAgentTaskState.canResume",
    ] {
        assert!(
            !chat.contains(forbidden),
            "R4 Chat page must not grant task control/completion authority from raw fragments: {forbidden}"
        );
    }
}

#[test]
fn single_system_r5_memory_and_provider_privacy_readmodels_own_product_boundaries() {
    let memory_contract = read_repo_file("openlife-core/src/agent/memory_view_model.rs");
    for required in [
        "pub struct MemoryViewModel",
        "pub struct MemoryLifecycleSummary",
        "pub struct MemoryLaneSummary",
        "pub struct MemoryLifeModelLinkageSummary",
        "pub fn build_memory_view_model",
        "accepted_proposal_is_not_materialized_memory_proof",
        "tier_stats_support_storage_but_do_not_create_active_memory_truth",
        "rolled_back_memory_is_not_active",
        "review_items_are_linked_by_proposal_id_without_claiming_materialization",
    ] {
        assert!(
            memory_contract.contains(required),
            "R5 backend MemoryViewModel contract must include {required}"
        );
    }

    let provider_contract = read_repo_file("openlife-core/src/agent/provider_privacy_boundary.rs");
    for required in [
        "pub struct ProviderPrivacyBoundaryBuildInput",
        "pub fn build_provider_privacy_boundary_summary",
        "prefer_local_model_without_route_evidence_keeps_transmission_unknown",
        "local_only_required_without_route_evidence_does_not_claim_not_sent",
        "observed_not_sent_route_can_claim_not_sent",
        "validated_cloud_route_is_possible_not_sent_proof",
        "stale_provider_validation_blocks_cloud_readiness",
        "observed_external_transmission_still_overrides_config",
    ] {
        assert!(
            provider_contract.contains(required),
            "R5 provider/privacy contract must include {required}"
        );
    }

    let memory_read_model = read_repo_file("src-tauri/src/read_models/memory.rs");
    for required in [
        "pub async fn get_memory_view_model",
        "get_memory_view_model_with_state",
        "MemoryLifecycleStore",
        "get_memory_tier_stats_with_state",
        "get_review_center_view_model_with_state",
        "build_memory_view_model",
        "Accepted proposal decisions remain decision state",
    ] {
        assert!(
            memory_read_model.contains(required),
            "R5 Tauri MemoryViewModel command must include {required}"
        );
    }

    let provider_read_model = read_repo_file("src-tauri/src/read_models/provider_privacy.rs");
    for required in [
        "pub async fn get_provider_privacy_boundary_summary",
        "get_provider_privacy_boundary_summary_with_state",
        "summarize_loaded_provider_validation",
        "cloud_api_configured",
        "build_provider_privacy_boundary_summary",
        "external transmission remain fail-closed",
    ] {
        assert!(
            provider_read_model.contains(required),
            "R5 Tauri ProviderPrivacyBoundarySummary command must include {required}"
        );
    }
    assert!(
        !provider_read_model.contains("local_only_required: config.prefer_local_model"),
        "R5 ProviderPrivacyBoundarySummary must not convert local preference into a LocalOnly runtime requirement"
    );
    let provider_contract = read_repo_file("openlife-core/src/agent/provider_privacy_boundary.rs");
    for required in [
        "prefer_local_model_without_route_evidence_keeps_transmission_unknown",
        "local_only_required_without_route_evidence_does_not_claim_not_sent",
        "observed_not_sent_route_can_claim_not_sent",
        "observed_external_transmission_still_overrides_config",
        "no runtime route evidence",
    ] {
        assert!(
            provider_contract.contains(required),
            "R5 provider/privacy contract must lock fail-closed route evidence invariant {required}"
        );
    }
    assert!(
        !provider_contract.contains("unwrap_or(ExternalTransmissionStatus::NotSent)"),
        "R5 provider/privacy must not infer NotSent from config preference"
    );

    let lib = read_repo_file("src-tauri/src/lib.rs");
    assert!(
        lib.contains("use read_models::memory::get_memory_view_model;")
            && lib.contains(
                "use read_models::provider_privacy::get_provider_privacy_boundary_summary;"
            )
            && lib.contains("get_memory_view_model,")
            && lib.contains("get_provider_privacy_boundary_summary,"),
        "R5 MemoryViewModel and ProviderPrivacyBoundarySummary must be actual registered Tauri commands"
    );

    let tauri_bridge = read_repo_file("frontend/src/tauri.ts");
    for required in [
        "export type MemoryViewModel =",
        "export type MemoryLifecycleSummary =",
        "export type MemoryLaneSummary =",
        "getMemoryViewModel()",
        "getProviderPrivacyBoundarySummary()",
        "\"get_memory_view_model\"",
        "\"get_provider_privacy_boundary_summary\"",
    ] {
        assert!(
            tauri_bridge.contains(required),
            "R5 frontend bridge must mirror backend read-model command shape {required}"
        );
    }

    let memory_page = read_repo_file("frontend/src/pages/MemorySearch.tsx");
    assert!(
        memory_page.contains("getMemoryViewModel")
            && memory_page.contains("memoryViewModel?.summary")
            && memory_page.contains("lifecycleSummary")
            && !memory_page.contains("getMemoryTierStats("),
        "R5 MemorySearch must consume MemoryViewModel and must not derive product counts from raw tier stats"
    );

    for (path, required) in [
        (
            "frontend/src/pages/settings/tabs/ProviderTab.tsx",
            "providerPrivacyBoundary",
        ),
        (
            "frontend/src/pages/settings/tabs/OverviewTab.tsx",
            "providerPrivacyBoundary",
        ),
        (
            "frontend/src/pages/settings/tabs/ReviewMemoryTab.tsx",
            "memoryViewModel",
        ),
        (
            "frontend/src/viewmodels/today/todayViewModelAdapter.ts",
            "providerPrivacyBoundary",
        ),
    ] {
        let source = read_repo_file(path);
        assert!(
            source.contains(required),
            "R5 frontend convergence target {path} must consume {required}"
        );
        assert!(
            !source.contains("buildProviderReadinessView"),
            "R5 frontend convergence target {path} must not locally rebuild provider/privacy boundary"
        );
    }
}

#[test]
fn single_system_r6_frontend_convergence_guards_repaired_authority() {
    let frontend_guard =
        read_repo_file("frontend/src/pages/TodayPage.readModelConvergence.test.ts");
    for required in [
        "frontend read-model convergence guards",
        "getReviewCenterViewModel",
        "getTasksViewModel",
        "getLifeModelViewModel",
        "getMemoryViewModel",
        "getProviderPrivacyBoundarySummary",
        "reviewRequiredCountFromProjection",
        "keeps frontend helpers display-only",
    ] {
        assert!(
            frontend_guard.contains(required),
            "R6 frontend static guard must cover {required}"
        );
    }

    let mailbox = read_repo_file("frontend/src/pages/MailboxPage.tsx");
    for required in [
        "getReviewCenterViewModel",
        "allowedActions.find",
        "item.materializationStatus",
        "ReviewCenterViewModel 尚未提供该确认项的后端操作状态",
    ] {
        assert!(
            mailbox.contains(required),
            "R6 Mailbox must keep review action/materialization authority in ReviewCenterViewModel: {required}"
        );
    }
    for forbidden in [
        "function canAccept(",
        "function isPathInSafePaths(",
        "function appliedNotice(",
        "setProposals(prev =>",
    ] {
        assert!(
            !mailbox.contains(forbidden),
            "R6 Mailbox must not restore local review/materialization inference: {forbidden}"
        );
    }

    let chat = read_repo_file("frontend/src/pages/ChatPage.tsx");
    for required in [
        "getTasksViewModel",
        "currentTaskViewItem",
        "enabledTaskViewControl",
        "taskViewItem?.lifecycleStatus",
        "taskViewItem?.terminalDeliveryStatus",
    ] {
        assert!(
            chat.contains(required),
            "R6 Chat must keep task lifecycle/control authority in TasksViewModel: {required}"
        );
    }
    for forbidden in [
        "taskState?.canResume ? [\"resume\"]",
        "taskState?.canRetry ? [\"retry\"]",
        "canCancel ? [\"cancel\"]",
        "taskStatus === \"completed\" ||",
        "runStatus === \"completed\" ||",
    ] {
        assert!(
            !chat.contains(forbidden),
            "R6 Chat must not restore raw task control/completion authority: {forbidden}"
        );
    }

    let runs = read_repo_file("frontend/src/pages/RunsPage.tsx");
    for required in [
        "getTasksViewModel",
        "TaskViewModelItem",
        "item.lifecycleStatus",
        "enabledActionControls(item)",
        "control.effect",
    ] {
        assert!(
            runs.contains(required),
            "R6 Runs must keep lifecycle/control authority in TasksViewModel: {required}"
        );
    }
    for forbidden in [
        "listMainChatAgentTasks",
        "getMainChatAgentTaskDetail",
        "taskSummaryByRunId",
        "allowedControlsForSummary",
        "lifecycleForRun",
    ] {
        assert!(
            !runs.contains(forbidden),
            "R6 Runs must not restore raw task summary lifecycle reconstruction: {forbidden}"
        );
    }

    let lifemodel = read_repo_file("frontend/src/pages/LifeModelPage.tsx");
    assert!(
        lifemodel.contains("getLifeModelViewModel")
            && lifemodel.contains("viewModel?.pendingUpdateCounts.pendingReview")
            && lifemodel.contains("viewModel?.memoryLinkage")
            && lifemodel.contains("viewModel?.candidateChanges"),
        "R6 LifeModel page must keep backend LifeModelViewModel as product truth source"
    );
    for forbidden in [
        "getLifeModel(",
        "getLifeModelCurrentView(",
        "getSystemDiagnostics(",
        "countMemoryChunks(",
        "getMemoryTierStats(",
        "listProposals(",
        "buildLifeModelViewModelEnvelope",
    ] {
        assert!(
            !lifemodel.contains(forbidden),
            "R6 LifeModel page must not restore raw truth reconstruction: {forbidden}"
        );
    }

    let memory_page = read_repo_file("frontend/src/pages/MemorySearch.tsx");
    assert!(
        memory_page.contains("getMemoryViewModel")
            && memory_page.contains("memoryViewModel?.summary")
            && memory_page.contains("lifecycleSummary")
            && memory_page.contains("向量层级只是存储遥测")
            && !memory_page.contains("getMemoryTierStats("),
        "R6 MemorySearch must keep MemoryViewModel as product memory summary source"
    );

    let settings = read_repo_file("frontend/src/pages/SettingsPage.tsx");
    assert!(
        settings.contains("getMemoryViewModel")
            && settings.contains("getProviderPrivacyBoundarySummary")
            && settings.contains("providerPrivacyBoundary"),
        "R6 Settings must consume backend MemoryViewModel and ProviderPrivacyBoundarySummary"
    );
    for path in [
        "frontend/src/pages/settings/tabs/ProviderTab.tsx",
        "frontend/src/pages/settings/tabs/OverviewTab.tsx",
    ] {
        let source = read_repo_file(path);
        assert!(
            source.contains("providerPrivacyBoundary") && !source.contains("buildProviderReadinessView"),
            "R6 Settings tab {path} must use provider/privacy summary and not rebuild provider readiness locally"
        );
    }

    let today = read_repo_file("frontend/src/pages/TodayPage.tsx");
    assert!(
        today.contains("getLifeStateProjection")
            && today.contains("getDailyGoals")
            && today.contains("reviewRequiredCountFromProjection"),
        "R6 Today page must stay projection-backed until a backend TodayViewModel exists"
    );
    for forbidden in [
        "listProposals(",
        "getSystemDiagnostics(",
        "getMemoryTierStats(",
        "buildProviderReadinessView",
        "getProviderPrivacyBoundarySummary",
    ] {
        assert!(
            !today.contains(forbidden),
            "R6 Today limited page must not invent missing backend owners via {forbidden}"
        );
    }

    let runtime_disclosure = read_repo_file("frontend/src/utils/runtimeDisclosure.ts");
    for forbidden in [
        "safeInvoke",
        "getSystemDiagnostics",
        "getProviderPrivacyBoundarySummary",
        "listMainChatAgentTasks",
        "resumeMainChatAgentTask",
        "ReviewCenterViewModel",
        "TasksViewModel",
        "MemoryViewModel",
    ] {
        assert!(
            !runtime_disclosure.contains(forbidden),
            "R6 runtimeDisclosure must remain display-only and not call/own {forbidden}"
        );
    }

    let projection_helper = read_repo_file("frontend/src/utils/lifeStateProjection.ts");
    for forbidden in [
        "safeInvoke",
        "getSystemDiagnostics",
        "listProposals",
        "getProviderPrivacyBoundarySummary",
        "getMemoryTierStats",
    ] {
        assert!(
            !projection_helper.contains(forbidden),
            "R6 lifeStateProjection helper must remain a formatter over backend projection: {forbidden}"
        );
    }
}

#[test]
fn single_system_r0_frontend_raw_reconstruction_hotspots_match_inventory() {
    let raw_read_markers = [
        "getSystemDiagnostics(",
        "listProposals(",
        "listAgentRuns(",
        "listMainChatAgentTasks(",
        "getLifeModel(",
        "getLifeStateProjection(",
        "getDailyGoals(",
        "getSchedulerConfig(",
        "getMemoryTierStats(",
        "getLifeModelCurrentView(",
        "getLifeModelCompletion(",
    ];
    let mut actual = BTreeSet::new();
    for file in source_files(&["frontend/src/pages", "frontend/src/utils"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with(".test.ts")
            || rel.ends_with(".test.tsx")
            || rel.contains("frontend/src/test/")
            || rel == "frontend/src/tauriDev.ts"
        {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        if raw_read_markers
            .iter()
            .any(|marker| source.contains(marker))
        {
            actual.insert(rel);
        }
    }

    let inventory = inventory();
    let entries = inventory_entries(&inventory, "frontend_multi_source_state_surfaces");
    let mut expected = BTreeSet::new();
    let mut classified = BTreeMap::new();
    for entry in entries {
        let disposition = entry_str(entry, "disposition");
        validate_inventory_path_contract(
            "frontend_multi_source_state_surfaces",
            entry,
            disposition,
        );
        if disposition == "deleted" {
            continue;
        }
        let path = entry_str(entry, "path").to_string();
        let classification = entry_str(entry, "r0_surface_classification").to_string();
        assert!(
            !entry_bool(entry, "backend_owner"),
            "frontend multi-source state surface {path} must not be classified as backend owner"
        );
        classified.insert(path.clone(), classification);
        if entry_bool(entry, "r0_raw_source_scan_hit") {
            expected.insert(path);
        }
    }

    assert_eq!(
        actual, expected,
        "R0 frontend raw reconstruction scan must match classified inventory entries"
    );

    for required_helper in [
        "frontend/src/utils/runtimeDisclosure.ts",
        "frontend/src/utils/runDisplaySummary.ts",
        "frontend/src/utils/capabilityStatus.ts",
        "frontend/src/utils/lifeStateProjection.ts",
    ] {
        assert!(
            classified.contains_key(required_helper),
            "R0 source map must classify frontend helper {required_helper}"
        );
    }

    for (path, expected_class) in [
        (
            "frontend/src/pages/chat/useChatContext.ts",
            "product_hook_raw_read_convergence_target",
        ),
        (
            "frontend/src/pages/MemorySearch.tsx",
            "technical_memory_surface_memory_view_model_consumer",
        ),
        ("frontend/src/tauri.ts", "frontend_product_bridge"),
    ] {
        assert_eq!(
            classified.get(path).map(String::as_str),
            Some(expected_class),
            "R0 source map must classify {path} as {expected_class}"
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
fn governed_data_import_recovery_has_one_journal_owner_and_bounded_product_commands() {
    let lib = read_repo_file("src-tauri/src/lib.rs");
    let handler = lib
        .split("tauri::generate_handler![")
        .nth(1)
        .and_then(|rest| rest.split("])").next())
        .expect("Tauri generate_handler body");
    let settings_source = read_repo_file("src-tauri/src/commands/settings.rs");
    let settings = strip_cfg_test_module(&settings_source);
    let bootstrap_source = read_repo_file("src-tauri/src/bootstrap.rs");
    let bootstrap = strip_cfg_test_module(&bootstrap_source);
    let app_state = read_repo_file("src-tauri/src/state.rs");
    let journal_source = read_repo_file("openlife-core/src/persistence_outbox.rs");
    let journal = strip_cfg_test_module(&journal_source);
    let state_projection = read_repo_file("src-tauri/src/state_projection.rs");
    let frontend = read_repo_file("frontend/src/tauri.ts");

    for command in [
        "abandon_governed_data_import_recovery",
        "get_governed_data_import_status",
    ] {
        assert_eq!(
            settings
                .matches(&format!("pub async fn {command}("))
                .count(),
            1,
            "governed import recovery command {command} must have one backend owner"
        );
        assert_eq!(
            handler
                .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
                .filter(|token| *token == command)
                .count(),
            1,
            "governed import recovery command {command} must be shipped exactly once"
        );
        assert!(
            frontend.contains(&format!("\"{command}\"")),
            "frontend contract adapter must expose {command}"
        );
    }
    assert_eq!(
        journal
            .matches("pub fn abandon_preserving_current(")
            .count(),
        1,
        "GovernedDataImportJournal must remain the sole abandonment evidence owner"
    );
    assert_eq!(
        journal.matches("pub fn latest_receipt(").count(),
        1,
        "durable recovery status must reuse the existing journal rather than a second ledger"
    );
    assert_eq!(
        settings.matches("GovernedDataImportJournal::new(").count(),
        0,
        "product Settings paths must reuse the bootstrap-owned journal; Journal::new performs schema migration and is not a read-only status operation"
    );
    assert_eq!(
        bootstrap.matches("GovernedDataImportJournal::new(").count(),
        1,
        "release bootstrap must open and migrate the governed import journal exactly once"
    );
    assert!(app_state.contains("pub(crate) governed_data_import_journal:"));
    assert!(
        app_state
            .contains("Option<Arc<openlife_core::persistence_outbox::GovernedDataImportJournal>>"),
        "AppState must retain the reusable bootstrap-owned governed import journal"
    );
    let status_command = source_between(
        settings,
        "pub async fn get_governed_data_import_status(",
        "pub async fn abandon_governed_data_import_recovery(",
    );
    assert!(status_command.contains("required_governed_data_import_journal(state.inner())"));
    for forbidden in [
        "GovernedDataImportJournal::new(",
        "migrate_",
        "CREATE TABLE",
        "CREATE INDEX",
        "pragma_update",
    ] {
        assert!(
            !status_command.contains(forbidden),
            "read-only governed import status must not execute journal schema work: {forbidden}"
        );
    }
    let journal_accessor = source_between(
        settings,
        "fn required_governed_data_import_journal(",
        "async fn governed_import_recovery_preflight_receipt(",
    );
    assert!(journal_accessor.contains(".governed_data_import_journal"));
    assert!(journal_accessor.contains(".cloned()"));
    assert!(!journal_accessor.contains("mutation_journal_path"));
    assert!(!journal_accessor.contains("GovernedDataImportJournal::new("));
    assert!(state_projection
        .contains("reconcile_state_store_lifemodel_projection_for_import_recovery_event"));
    assert!(!state_projection
        .contains("fn reconcile_state_store_lifemodel_projection_for_import_recovery("));
    let status_adapter = source_between(
        &frontend,
        "export async function getGovernedDataImportStatus",
        "export async function abandonGovernedDataImportRecovery",
    );
    assert!(status_adapter.contains("get_governed_data_import_status"));
    for forbidden in [
        "payload",
        "requestDigest",
        "observedDigest",
        "beforeDigest",
        "targetDigest",
    ] {
        assert!(
            !status_adapter.contains(forbidden),
            "bounded recovery status adapter must not accept {forbidden}"
        );
    }
}

#[test]
fn governed_import_resolution_barrier_covers_all_four_canonical_owners() {
    let coordinator = read_repo_file("src-tauri/src/persistence_coordinator.rs");
    for required in [
        "canonical_write_barrier: tokio::sync::RwLock<()>",
        "admit_normal_or_governed_data_import_writes",
        "admit_startup_reconciliation_writes",
        "acquire_canonical_commit_permit",
        "acquire_governed_data_import_resolution_fence",
        "acquire_governed_data_import_completion_fence",
        "invalidate_canonical_write_admissions",
    ] {
        assert!(
            coordinator.contains(required),
            "PersistenceCoordinator must own canonical recovery barrier marker {required}"
        );
    }

    let lifemodel = read_repo_file("src-tauri/src/life_model_write_gateway.rs");
    let lifemodel_commit = source_between(
        &lifemodel,
        "async fn write_prepared_life_model_compare_and_swap",
        "async fn write_life_model_without_prepare",
    );
    assert!(lifemodel_commit.contains("CanonicalCommitPermit"));
    assert!(lifemodel_commit.contains("manager.save("));
    assert!(lifemodel_commit.contains("observe_canonical_digest"));

    let memory = read_repo_file("src-tauri/src/memory_gateway.rs");
    let memory_admission = source_between(
        &memory,
        "fn admit_memory_vector_writes",
        "async fn exchange_memory_vector_commit_admission",
    );
    assert!(memory_admission.contains("admit_normal_or_governed_data_import_writes"));
    let memory_exchange = source_between(
        &memory,
        "async fn exchange_memory_vector_commit_admission",
        "async fn acquire_memory_vector_commit_permit",
    );
    assert!(memory_exchange.contains("acquire_canonical_commit_permit"));
    let memory_barrier_composition = source_between(
        &memory,
        "async fn acquire_memory_vector_commit_permit",
        "async fn commit_vector_store_mutation",
    );
    assert!(memory_barrier_composition.contains("admit_memory_vector_writes"));
    assert!(memory_barrier_composition.contains("exchange_memory_vector_commit_admission"));
    for (start, end) in [
        (
            "pub(crate) async fn save_conversation_message_idempotent_with_state",
            "pub(crate) async fn save_turn_user_message_idempotent_with_state",
        ),
        (
            "pub(crate) async fn save_turn_user_message_idempotent_with_state",
            "pub(crate) async fn create_chat_session_with_state",
        ),
        (
            "pub(crate) async fn delete_chat_session_with_state",
            "pub(crate) async fn reconcile_canonical_outboxes_with_state",
        ),
        (
            "pub(crate) async fn run_memory_tier_maintenance_with_state",
            "pub(crate) async fn archive_low_access_memories_with_state",
        ),
        (
            "async fn replace_imported_memory_with_state_inner",
            "pub(crate) async fn materialize_memory_proposal_with_state",
        ),
    ] {
        assert!(
            source_between(&memory, start, end).contains("acquire_memory_vector_commit_permit"),
            "Memory/Vector canonical write path {start} must acquire the shared commit permit"
        );
    }
    let rebuild = source_between(
        &memory,
        "pub(crate) async fn rebuild_memory_index_with_state",
        "async fn cancellable_rebuild_embedding",
    );
    assert!(rebuild.contains("commit_vector_store_mutation"));

    let kernel = read_repo_file("src-tauri/src/main_chat_kernel.rs");
    let state_helper = source_between(
        &kernel,
        "async fn acquire_state_store_commit_permit",
        "fn resolve_transient_state_execution_context",
    );
    assert!(state_helper.contains("GovernedDataImportRecoveryOwner::StateStore"));
    assert!(state_helper.contains("acquire_canonical_commit_permit"));
    for start in [
        "async fn build_kernel_transient_state_command_surface_result",
        "async fn build_kernel_resource_daily_task_batch_result",
    ] {
        let body = kernel
            .split(start)
            .nth(1)
            .unwrap_or_else(|| panic!("missing StateGateway owner {start}"));
        assert!(body.contains("acquire_state_store_commit_permit"));
    }

    let settings = read_repo_file("src-tauri/src/commands/settings.rs");
    let state_restore = source_between(
        &settings,
        "if saga.stage == GovernedDataImportStage::VectorApplied",
        "if saga.stage == GovernedDataImportStage::StateCommitted",
    );
    assert!(state_restore.contains("GovernedDataImportRecoveryOwner::StateStore"));
    assert!(state_restore.contains("acquire_canonical_commit_permit"));
    let explicit_abandonment = source_between(
        &settings,
        "pub async fn abandon_governed_data_import_recovery",
        "async fn import_all_data_with_state",
    );
    assert!(explicit_abandonment.contains("acquire_governed_data_import_resolution_fence"));
    let automatic_abandonment = source_between(
        &settings,
        "async fn abandon_governed_import_after_exact_observation",
        "async fn mark_governed_import_owner_unknown",
    );
    assert!(automatic_abandonment.contains("acquire_governed_data_import_resolution_fence"));

    let state_projection = read_repo_file("src-tauri/src/state_projection.rs");
    let state_projection_finalize = source_between(
        &state_projection,
        "async fn acquire_state_projection_finalize_permit",
        "pub(crate) async fn reconcile_state_store_lifemodel_projection",
    );
    assert!(state_projection_finalize.contains("GovernedDataImportRecoveryOwner::StateStore"));
    assert!(state_projection_finalize.contains("acquire_canonical_commit_permit"));
    let state_projection_terminal = source_between(
        &state_projection,
        "\n    match projection_result {\n",
        "fn required_projection_proof",
    );
    assert!(state_projection_terminal.contains("acquire_state_projection_finalize_permit"));
    assert!(state_projection_terminal.contains("mark_projection_applied"));
    assert!(state_projection_terminal.contains("mark_projection_degraded"));

    let successful_completion = source_between(
        &settings,
        "let completion_fence = state",
        "let durable_lifemodel_write =",
    );
    assert!(successful_completion.contains("acquire_governed_data_import_completion_fence"));
    assert!(successful_completion.contains("verify_governed_import_terminal_facts"));
    assert!(successful_completion.contains("GovernedDataImportStage::Completed"));
    assert!(successful_completion.contains("drop(completion_fence)"));

    let lib = read_repo_file("src-tauri/src/lib.rs");
    let dev_workers = source_between(
        &lib,
        "fn start_dev_extension_background_workers",
        "fn runtime_dev_url",
    );
    assert!(dev_workers.contains("run_memory_tier_maintenance_with_state"));
    assert!(!dev_workers.contains(".run_tier_maintenance("));
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
fn single_system_phase4a_review_permission_contract_is_backend_owned_and_fail_closed() {
    let review_item = read_repo_file("openlife-core/src/agent/review_item.rs");
    for required in [
        "pub decision_context: ReviewDecisionContext",
        "build_review_decision_context(proposal, &evidence_refs)",
        "Exact permission scope is incomplete",
        "materialization_status_for",
        "ReviewItemMaterializationStatus::Unknown",
    ] {
        assert!(
            review_item.contains(required),
            "Phase 4A ReviewItem owner must include {required}"
        );
    }

    let decision_context = read_repo_file("openlife-core/src/agent/review_decision_context.rs");
    for required in [
        "pub struct ReviewDecisionContext",
        "pub struct PermissionDecisionContext",
        "PermissionScopeKind::ActionBound",
        "PermissionScopeKind::NetworkPolicy",
        "PermissionDecisionContextStatus::Incomplete",
        "ExternalTransmissionStatus::Unknown",
        "transmissionBoundary",
        "[REDACTED]",
    ] {
        assert!(
            decision_context.contains(required),
            "Phase 4A readable decision projection must include {required}"
        );
    }

    let action_contract = read_repo_file("openlife-core/src/agent/product_read_model.rs");
    for required in [
        "pub completion_proof_after_dispatch: bool",
        "ReviewActionClaimsCompletionProof",
        "DisabledReviewActionMissingReason",
        "ReviewActionConfirmationRequired",
    ] {
        assert!(
            action_contract.contains(required),
            "Phase 4A ReviewAction contract must include {required}"
        );
    }

    let bridge = read_repo_file("frontend/src/tauri.ts");
    for required in [
        "export type ReviewDecisionContext =",
        "export type PermissionDecisionContext =",
        "completionProofAfterDispatch: boolean",
        "decisionContext: ReviewDecisionContext",
        "activeTask?: TaskViewModelItem",
        "pendingReviewItems: ReviewItem[]",
        "activity: WorkspaceActivityItem[]",
    ] {
        assert!(
            bridge.contains(required),
            "Phase 4A TypeScript bridge must mirror {required}"
        );
    }
}

#[test]
fn single_system_phase4a_test_fixtures_and_contract_harnesses_are_absent_from_product_imports() {
    for file in source_files(&["frontend/src/pages", "frontend/src/components"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with(".test.ts") || rel.ends_with(".test.tsx") {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        for forbidden in [
            "phase4a-contract-golden",
            "src/test/fixtures",
            "test/phase4aContractGolden",
        ] {
            assert!(
                !source.contains(forbidden),
                "Phase 4A test contract must not enter product source {rel}: {forbidden}"
            );
        }
    }

    let app = read_repo_file("frontend/src/App.tsx");
    let shell = read_repo_file("frontend/src/components/ProductShell.tsx");
    for source in [&app, &shell] {
        assert!(!source.contains("phase4a-contract-golden"));
        assert!(!source.contains("reviewDispatchContract"));
        assert!(!source.contains("settingsOrchestrationContract"));
    }
}

#[test]
fn single_system_phase4b_foundation_harness_is_dev_only_and_preview_route_stays_absent() {
    let app = read_repo_file("frontend/src/App.tsx");
    let shell = read_repo_file("frontend/src/components/ProductShell.tsx");
    for source in [&app, &shell] {
        for forbidden in [
            "TodayV2PreviewPage",
            "/today-v2-preview",
            "src/dev/phase4b",
            "OPENLIFE_PHASE4B_DEV_HARNESS",
        ] {
            assert!(
                !source.contains(forbidden),
                "Phase 4B dev-only surface must stay absent from product shell authority: {forbidden}"
            );
        }
    }

    assert!(
        !repo_root()
            .join("frontend/src/pages/TodayV2PreviewPage.tsx")
            .exists(),
        "retired production preview page must stay absent"
    );

    let production_vite = read_repo_file("frontend/vite.config.ts");
    let harness_vite = read_repo_file("frontend/vite.phase4b.config.ts");
    let tauri_overlay = read_repo_file("src-tauri/tauri.phase4b.conf.json");
    let package = read_repo_file("frontend/package.json");
    let release_guard = read_repo_file("frontend/scripts/verify-production-absence.mjs");

    assert!(
        production_vite.contains("__OPENLIFE_PHASE4B_HARNESS__: JSON.stringify(false)")
            && harness_vite.contains("__OPENLIFE_PHASE4B_HARNESS__: JSON.stringify(true)"),
        "Phase 4B harness must be compile-time false for production and true only in its Vite entry"
    );
    for required in [
        "http://127.0.0.1:4184/dev/phase4b/",
        "corepack pnpm dev:phase4b --host 127.0.0.1 --port 4184",
        "\"cwd\": \"../frontend\"",
        "Phase 4B is development-only; release and package builds are forbidden.",
        "\"active\": false",
    ] {
        assert!(
            tauri_overlay.contains(required),
            "Phase 4B Tauri dev overlay must contain {required}"
        );
    }
    assert!(
        !tauri_overlay.contains("dev:phase4b -- --host"),
        "Phase 4B beforeDevCommand must pass Vite host options instead of hiding them after a separator"
    );
    assert!(
        package.contains("vite build && node scripts/verify-production-absence.mjs"),
        "normal production build must execute the Phase 4B release absence guard"
    );
    for required in [
        "OPENLIFE_PHASE4B_DEV_HARNESS",
        "TodayV2PreviewPage",
        "/today-v2-preview",
        "dev/phase4b/index.html",
    ] {
        assert!(
            release_guard.contains(required),
            "release bundle guard must scan for {required}"
        );
    }

    for file in source_files(&["frontend/src/pages", "frontend/src/components"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with(".test.ts") || rel.ends_with(".test.tsx") {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        assert!(
            !source.contains("src/dev/phase4b") && !source.contains("OPENLIFE_PHASE4B_DEV_HARNESS"),
            "product source must not import Phase 4B harness: {rel}"
        );
    }
}

#[test]
fn single_system_phase4c_desktop_shell_harness_is_dev_only_and_product_authority_is_unchanged() {
    let app = read_repo_file("frontend/src/App.tsx");
    let shell = read_repo_file("frontend/src/components/ProductShell.tsx");
    let route_contract = read_repo_file("frontend/src/productShellContract.ts");
    for source in [&app, &shell, &route_contract] {
        for forbidden in [
            "src/dev/phase4c",
            "OPENLIFE_PHASE4C_DESKTOP_SHELL_HARNESS",
            "OpenLifeWorkbenchShell",
        ] {
            assert!(
                !source.contains(forbidden),
                "Phase 4C dev-only shell must stay absent from product authority: {forbidden}"
            );
        }
    }

    let production_vite = read_repo_file("frontend/vite.config.ts");
    let phase4b_vite = read_repo_file("frontend/vite.phase4b.config.ts");
    let harness_vite = read_repo_file("frontend/vite.phase4c.config.ts");
    let tauri_overlay = read_repo_file("src-tauri/tauri.phase4c.conf.json");
    let package = read_repo_file("frontend/package.json");
    let release_guard = read_repo_file("frontend/scripts/verify-production-absence.mjs");

    assert!(
        production_vite.contains("__OPENLIFE_PHASE4C_HARNESS__: JSON.stringify(false)")
            && phase4b_vite.contains("__OPENLIFE_PHASE4C_HARNESS__: JSON.stringify(false)")
            && harness_vite.contains("__OPENLIFE_PHASE4C_HARNESS__: JSON.stringify(true)"),
        "Phase 4C harness must be compile-time false outside its dedicated Vite entry"
    );
    assert!(
        harness_vite.contains("__OPENLIFE_PHASE4B_HARNESS__: JSON.stringify(false)"),
        "Phase 4C must not reactivate the Phase 4B harness"
    );
    for required in [
        "http://127.0.0.1:4185/dev/phase4c/",
        "corepack pnpm dev:phase4c --host 127.0.0.1 --port 4185",
        "\"cwd\": \"../frontend\"",
        "Phase 4C is development-only; release and package builds are forbidden.",
        "\"minWidth\": 1024",
        "\"active\": false",
    ] {
        assert!(
            tauri_overlay.contains(required),
            "Phase 4C Tauri dev overlay must contain {required}"
        );
    }
    assert!(
        package.contains("\"dev:phase4c\": \"vite --config vite.phase4c.config.ts\"")
            && package.contains("\"build:phase4c\"")
            && package.contains("\"qa:phase4c\""),
        "Phase 4C scripts must remain explicit dev/review commands"
    );
    for required in [
        "OPENLIFE_PHASE4C_DESKTOP_SHELL_HARNESS",
        "OpenLifeWorkbenchShell",
        "dev/phase4c/index.html",
    ] {
        assert!(
            release_guard.contains(required),
            "release bundle guard must scan for Phase 4C marker {required}"
        );
    }

    for file in source_files(&["frontend/src/pages", "frontend/src/components"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with(".test.ts") || rel.ends_with(".test.tsx") {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        assert!(
            !source.contains("src/dev/phase4c")
                && !source.contains("OPENLIFE_PHASE4C_DESKTOP_SHELL_HARNESS")
                && !source.contains("OpenLifeWorkbenchShell"),
            "product source must not import the Phase 4C shell or harness: {rel}"
        );
    }
}

#[test]
fn single_system_phase4d_read_only_spine_is_dev_only_and_product_authority_is_unchanged() {
    let app = read_repo_file("frontend/src/App.tsx");
    let shell = read_repo_file("frontend/src/components/ProductShell.tsx");
    let route_contract = read_repo_file("frontend/src/productShellContract.ts");
    for source in [&app, &shell, &route_contract] {
        for forbidden in [
            "src/dev/phase4d",
            "OPENLIFE_PHASE4D_READ_ONLY_SPINE_HARNESS",
            "ReadOnlySpineJourney",
        ] {
            assert!(
                !source.contains(forbidden),
                "Phase 4D dev-only journey must stay absent from product authority: {forbidden}"
            );
        }
    }

    let production_vite = read_repo_file("frontend/vite.config.ts");
    let phase4b_vite = read_repo_file("frontend/vite.phase4b.config.ts");
    let phase4c_vite = read_repo_file("frontend/vite.phase4c.config.ts");
    let harness_vite = read_repo_file("frontend/vite.phase4d.config.ts");
    let tauri_overlay = read_repo_file("src-tauri/tauri.phase4d.conf.json");
    let package = read_repo_file("frontend/package.json");
    let release_guard = read_repo_file("frontend/scripts/verify-production-absence.mjs");

    assert!(
        production_vite.contains("__OPENLIFE_PHASE4D_HARNESS__: JSON.stringify(false)")
            && phase4b_vite.contains("__OPENLIFE_PHASE4D_HARNESS__: JSON.stringify(false)")
            && phase4c_vite.contains("__OPENLIFE_PHASE4D_HARNESS__: JSON.stringify(false)")
            && harness_vite.contains("__OPENLIFE_PHASE4D_HARNESS__: JSON.stringify(true)"),
        "Phase 4D harness must be compile-time false outside its dedicated Vite entry"
    );
    assert!(
        harness_vite.contains("__OPENLIFE_PHASE4B_HARNESS__: JSON.stringify(false)")
            && harness_vite.contains("__OPENLIFE_PHASE4C_HARNESS__: JSON.stringify(false)"),
        "Phase 4D must not reactivate earlier harnesses"
    );
    for required in [
        "http://127.0.0.1:4186/dev/phase4d/",
        "corepack pnpm dev:phase4d --host 127.0.0.1 --port 4186",
        "\"cwd\": \"../frontend\"",
        "Phase 4D is development-only; release and package builds are forbidden.",
        "\"minWidth\": 1024",
        "\"active\": false",
    ] {
        assert!(
            tauri_overlay.contains(required),
            "Phase 4D Tauri dev overlay must contain {required}"
        );
    }
    assert!(
        package.contains("\"dev:phase4d\": \"vite --config vite.phase4d.config.ts\"")
            && package.contains("\"build:phase4d\"")
            && package.contains("\"qa:phase4d\""),
        "Phase 4D scripts must remain explicit dev/review commands"
    );
    for required in [
        "OPENLIFE_PHASE4D_READ_ONLY_SPINE_HARNESS",
        "ReadOnlySpineJourney",
        "dev/phase4d/index.html",
    ] {
        assert!(
            release_guard.contains(required),
            "release bundle guard must scan for Phase 4D marker {required}"
        );
    }

    for file in source_files(&["frontend/src/pages", "frontend/src/components"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with(".test.ts") || rel.ends_with(".test.tsx") {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        assert!(
            !source.contains("src/dev/phase4d")
                && !source.contains("OPENLIFE_PHASE4D_READ_ONLY_SPINE_HARNESS")
                && !source.contains("ReadOnlySpineJourney"),
            "product source must not import the Phase 4D journey or harness: {rel}"
        );
    }
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
        "openlife-core/src/agent/plan_execute.rs",
        "src-tauri/src/main_chat_kernel.rs",
        "src-tauri/src/provider_network_consent.rs",
        "src-tauri/src/commands/builder.rs",
        "src-tauri/src/commands/calibration.rs",
    ] {
        assert!(
            product_paths.contains(required_path),
            "Phase4 source map must classify {required_path} as a ReviewWorkflow product path"
        );
        let raw_source = read_repo_file(required_path);
        let source = strip_cfg_test_module(&raw_source);
        if required_path.starts_with("openlife-core/src/agent/action_executor/") {
            assert!(
                source.contains("ctx.submit_review_proposal"),
                "Phase4 ToolGateway product path {required_path} must enter the deep ReviewWorkflow gateway"
            );
            assert!(
                !source.contains("ReviewWorkflow::new"),
                "Phase4 ToolGateway product path {required_path} must not recreate ReviewWorkflow ownership"
            );
        } else {
            assert!(
                source.contains("ReviewWorkflow::new"),
                "Phase4 product path {required_path} must call ReviewWorkflow"
            );
        }
        assert!(
            source.contains("outcome.proposal_id()") || source.contains("outcome.proposal"),
            "Phase4 product path {required_path} must consume ReviewWorkflowOutcome as the authoritative proposal fact"
        );
    }

    assert!(
        product_paths.contains("openlife-core/src/agent/action_executor/mod.rs"),
        "Phase4 source map must expose the deep ActionExecutor ReviewWorkflow gateway"
    );
    let action_executor_owner = read_repo_file("openlife-core/src/agent/action_executor/mod.rs");
    assert_eq!(
        count_occurrences(&action_executor_owner, "ReviewWorkflow::new"),
        1,
        "ActionExecutor must expose one deep ReviewWorkflow owner instead of duplicating it in tool implementations"
    );
    for owner_marker in [
        "pub(crate) fn submit_review_proposal",
        "canonical_write_admission",
        "submit_with_admission(request, admission)",
    ] {
        assert!(
            action_executor_owner.contains(owner_marker),
            "ActionExecutor ReviewWorkflow gateway lost ownership marker {owner_marker}"
        );
    }
    let review_workflow_owner = read_repo_file("openlife-core/src/agent/review_workflow.rs");
    for owner_marker in [
        "pub fn submit_with_admission(",
        "CanonicalWriteAdmissionRequest::new",
        "permit.finish_committed()",
        "permit.finish_noop()",
        "permit.finish_failed()",
    ] {
        assert!(
            review_workflow_owner.contains(owner_marker),
            "ReviewWorkflow lost canonical admission owner marker {owner_marker}"
        );
    }

    let provider_network_consent = read_repo_file("src-tauri/src/provider_network_consent.rs");
    assert!(
        provider_network_consent.contains("submit_main_chat_terminal_review_relation("),
        "Main Chat provider consent must use the typed terminal-owner Review seam"
    );
    assert!(
        provider_network_consent.contains("ProposalTerminalRelationKind::ActionResumePrerequisite"),
        "provider consent must declare its action-resume lifecycle explicitly"
    );
    let task_controls_source = read_repo_file("src-tauri/src/main_chat_task_controls.rs");
    let task_controls = strip_cfg_test_module(&task_controls_source);
    assert!(
        task_controls.contains(".run_provider_network_consent_continuation("),
        "accepted provider consent must resume through the one OpenLifeTurnRuntime owner"
    );
    for forbidden_command_owned_provider_path in [
        ".generate_direct_answer(",
        "CommandSurfaceDirectAnswerModelClient",
        "SchedulerMainChatModelClient",
    ] {
        assert!(
            !task_controls.contains(forbidden_command_owned_provider_path),
            "task controls must not own a parallel provider continuation path: {forbidden_command_owned_provider_path}"
        );
    }
    let turn_runtime = read_repo_file("src-tauri/src/main_chat_turn_runtime.rs");
    for runtime_owner_marker in [
        "pub(crate) async fn run_provider_network_consent_continuation(",
        "OpenLifeTurnExecutionMode::ProviderConsentContinuation",
        "issue_terminal_owner_provider_consent_replay_admission(",
        "terminalized_provider_continuation_preparation_error(",
    ] {
        assert!(
            turn_runtime.contains(runtime_owner_marker),
            "OpenLifeTurnRuntime lost provider continuation owner marker {runtime_owner_marker}"
        );
    }
    let permissions = read_repo_file("openlife-core/src/tool_permissions.rs");
    for exact_grant_marker in [
        "pub fn consume_reviewed_network_once_for_proposal(",
        "pub fn reviewed_network_once_available_for_proposal(",
    ] {
        assert!(
            permissions.contains(exact_grant_marker),
            "provider continuation must bind AllowOnce to the exact accepted Proposal: {exact_grant_marker}"
        );
    }
    let event_store = read_repo_file("src-tauri/src/main_chat_event_stream.rs");
    assert!(
        event_store.contains("PayloadFieldSchema::optional(\"replayCause\"")
            && event_store.contains("latest_provider_event_after_replay_start("),
        "startup recovery must retain replay type and query only provider facts after that replay start"
    );
    assert!(
        !turn_runtime.contains("bind_staged_proposal_to_terminal_owner_origin("),
        "TurnRuntime finalization must validate canonical typed Review relations, never late-bind them"
    );
    assert!(
        !review_workflow_owner.contains("bind_staged_proposal_to_terminal_owner_origin("),
        "ReviewWorkflow must not retain a late-bind escape hatch for Main Chat proposals"
    );
    let main_chat_kernel = read_repo_file("src-tauri/src/main_chat_kernel.rs");
    assert!(
        !main_chat_kernel.contains("AgentRunProposalStagingKind::MainChatReview"),
        "ordinary Main Chat must project typed Review relations at creation instead of staging AgentRun ids later"
    );

    let retired_stage4_memory_knowledge = "src-tauri/src/main_chat_stage4_memory_knowledge.rs";
    assert!(
        !repo_root().join(retired_stage4_memory_knowledge).exists(),
        "Phase7 contract requires the historical Stage4 memory/knowledge shell to be deleted from the product crate"
    );
    let retired_proposal_support = "src-tauri/src/main_chat_proposal_support.rs";
    assert!(
        !repo_root().join(retired_proposal_support).exists(),
        "the dormant parallel Main Chat Proposal route must remain deleted"
    );
    let lib_source = read_repo_file("src-tauri/src/lib.rs");
    for retired_symbol in [
        "mod main_chat_proposal_support;",
        "create_main_chat_agent_proposal(",
        "attach_main_chat_tool_permission_proposal_metadata(",
    ] {
        assert!(
            !lib_source.contains(retired_symbol),
            "retired Proposal symbol {retired_symbol} must remain absent from the product crate"
        );
    }
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
            "src-tauri/src/commands/builder.rs",
            "store.add_generated_proposal(&run_id, &proposal.id)",
        ),
        (
            "src-tauri/src/commands/calibration.rs",
            "created_ids.push(proposal.id",
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
        ".replace_all_messages_guarded(",
        ".replace_all_chunks(",
        ".replace_portable_chunks_guarded(",
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
        ".save_message_idempotent(",
        ".save_message_idempotent_with_proof(",
        ".save_knowledge_note_idempotent_with_outbox(",
        ".replace_all_messages_guarded(",
        ".replace_portable_chunks_guarded(",
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
    for required_owner_marker in [
        "FileMutationJournal",
        "stage_materialization_patches",
        "reconcile_lifemodel_file_mutations_with_state",
        "ensure_projection_snapshot",
    ] {
        assert!(
            lifemodel_gateway.contains(required_owner_marker),
            "Phase5 LifeModelWriteGateway must own durable file mutation/projection marker {required_owner_marker}"
        );
    }
    for retired_imperative_tail in [".create_patch(", "snapshot_for_patch(&after_model"] {
        assert!(
            !lifemodel_gateway.contains(retired_imperative_tail),
            "Phase5 LifeModelWriteGateway must delete post-canonical imperative tail {retired_imperative_tail}"
        );
    }
    let bootstrap = read_repo_file("src-tauri/src/bootstrap.rs");
    assert!(
        bootstrap.contains("reconcile_startup_lifemodel_file_mutations_with_state"),
        "Phase5 startup must use the bootstrap-only admission to reconcile the canonical LifeModel file journal before product use"
    );
    for (path, start, end) in [
        (
            "src-tauri/src/commands/life_model.rs",
            "pub(crate) async fn save_life_model_with_state",
            "#[tauri::command]\npub async fn save_life_model",
        ),
        (
            "src-tauri/src/commands/version.rs",
            "async fn restore_snapshot_governed_operation",
            "#[tauri::command]\npub async fn diff_snapshots",
        ),
        (
            "src-tauri/src/commands/settings.rs",
            "async fn import_all_data_governed_operation_with_fault",
            "let before_digest =",
        ),
    ] {
        let source = read_repo_file(path);
        let body = source_between(&source, start, end);
        assert!(
            body.contains("ensure_projection_snapshot"),
            "governed destructive path {path} must create its required pre-change snapshot"
        );
        assert!(
            !body.contains(".ok()"),
            "governed destructive path {path} must not swallow required pre-change snapshot failure"
        );
    }
    assert!(
        !strip_cfg_test_module(&read_repo_file("src-tauri/src/lib.rs")).contains("manager.save("),
        "Phase5 lib.rs command wiring must not own LifeModel persistence"
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
fn single_system_phase6_workspace_file_execution_has_one_tool_gateway_owner() {
    let resolver = read_repo_file("src-tauri/src/workspace_file_resolver.rs");
    let target_resolution = source_between(
        &resolver,
        "pub(crate) fn resolve_main_chat_workspace_file_target",
        "pub(crate) fn resolve_workspace_root",
    );
    for forbidden in [
        "canonicalize()",
        ".metadata()",
        "std::fs::read",
        "File::open",
    ] {
        assert!(
            !target_resolution.contains(forbidden),
            "workspace candidate resolution must remain lexical before ToolGateway: {forbidden}"
        );
    }

    let kernel = read_repo_file("src-tauri/src/main_chat_kernel.rs");
    let pre_gateway_blocker = source_between(
        &kernel,
        "fn blocked_kernel_read_tool_execution",
        "fn merge_kernel_read_selection_metadata",
    );
    assert!(
        !pre_gateway_blocker.contains("ToolExecutionReceipt::failed_before_dispatch"),
        "a lexical Kernel blocker must not manufacture a ToolGateway execution receipt"
    );
    assert!(pre_gateway_blocker.contains("execution_receipt: None"));

    let filesystem_adapter =
        read_repo_file("openlife-core/src/agent/action_executor/execution_tools.rs");
    assert!(
        filesystem_adapter.contains("tokio::fs::metadata")
            && filesystem_adapter.contains("tokio::fs::read_to_string"),
        "ToolGateway filesystem adapter must own operating-system file observations"
    );
    let executor_context = read_repo_file("openlife-core/src/agent/action_executor/mod.rs");
    assert!(executor_context.contains("struct ToolDispatchAdmission"));
    assert!(executor_context.contains("Result<ToolDispatchAdmission<'a>>"));
    for adapter_path in [
        "openlife-core/src/agent/action_executor/core_os_tools.rs",
        "openlife-core/src/agent/action_executor/execution_tools.rs",
        "openlife-core/src/agent/action_executor/tool_executor.rs",
    ] {
        let adapter = read_repo_file(adapter_path);
        for forbidden in ["mark_local_dispatched", "mark_simulated_dispatched"] {
            assert!(
                !adapter.contains(forbidden),
                "concrete adapter {adapter_path} must consume ToolDispatchAdmission instead of mutating dispatch truth via {forbidden}"
            );
        }
    }
    let remote_helpers = read_repo_file("openlife-core/src/agent/action_executor/helpers.rs");
    for helper in ["fetch_url_async", "search_web_async", "call_a2a_agent"] {
        let marker = format!("async fn {helper}");
        let signature = remote_helpers
            .split(&marker)
            .nth(1)
            .unwrap_or_else(|| panic!("missing remote adapter {helper}"))
            .chars()
            .take(500)
            .collect::<String>();
        assert!(
            signature.contains("ToolDispatchAdmission"),
            "remote adapter {helper} must require a sealed dispatch admission"
        );
    }

    let runtime = read_repo_file("src-tauri/src/main_chat_turn_runtime.rs");
    let final_persistence = source_between(
        &runtime,
        "async fn persist_openlife_turn_final_delivery_receipt",
        "fn canonical_final_owner_digest",
    );
    assert!(final_persistence.contains("toolOwnerBindingsVersion"));
    assert!(final_persistence.contains("toolOwnerBindings"));
    for legacy_parallel_field in [
        "toolReceiptRefs",
        "toolTerminalEventRefs",
        "toolTerminalEventDigests",
    ] {
        assert!(
            !final_persistence.contains(legacy_parallel_field),
            "new finals must not persist the positional v1 tool owner field {legacy_parallel_field}"
        );
    }
    let identity_recovery = source_between(
        &runtime,
        "async fn recover_canonical_tool_facts",
        "async fn recover_openlife_turn_from_durable_final",
    );
    assert!(identity_recovery.contains("binding.action_queue_id"));
    assert!(identity_recovery.contains("binding.receipt_id"));
    assert!(identity_recovery.contains("binding.terminal_event_id"));
}

#[test]
fn single_system_phase6_resource_artifact_and_tool_gateways_are_distinct() {
    let resource_commands_source = read_repo_file("src-tauri/src/resource_commands.rs");
    let resource_commands = strip_cfg_test_module(&resource_commands_source);
    assert!(
        resource_commands.contains("gateway.detach_resource_from_message("),
        "resource detach must enter ResourceGateway instead of mutating ResourceStore from IPC"
    );
    assert!(
        !resource_commands.contains("store.detach_resource_from_message("),
        "resource detach must not retain a command-owned canonical write path"
    );

    let resource_gateway_source = read_repo_file("openlife-core/src/resource_gateway.rs");
    let resource_gateway = strip_cfg_test_module(&resource_gateway_source);
    for lifecycle_owner in [
        "commit_import_batch_guarded(",
        "pub fn detach_resource_from_message(",
    ] {
        assert!(
            resource_gateway.contains(lifecycle_owner),
            "ResourceGateway must own canonical resource lifecycle operation {lifecycle_owner}"
        );
    }
    assert!(
        !resource_gateway.contains("agent::ToolGateway")
            && !resource_gateway.contains("ToolGateway::from_executor_config"),
        "ResourceGateway must not become a second Agent tool executor"
    );

    let proposal_source = read_repo_file("src-tauri/src/commands/proposal.rs");
    let proposal = strip_cfg_test_module(&proposal_source);
    let artifact_apply = source_between(
        proposal,
        "async fn apply_external_write_artifact",
        "pub(crate) fn memory_session_id",
    );
    for required in [
        "claim_id: &str",
        "prepare_artifact_materialization(",
        "stage_artifact_bytes(",
        "commit_staged_artifact(",
        "finish_artifact_confirmed(",
    ] {
        assert!(
            artifact_apply.contains(required),
            "accepted artifact materialization must retain review-claim-bound owner {required}"
        );
    }
    let accept_flow = source_between(
        proposal,
        "pub(crate) async fn accept_proposal_with_state",
        "async fn terminal_owner_relation_kind",
    );
    assert!(
        accept_flow.contains(".claim_dispatch(&proposal_id)")
            && accept_flow.contains(".claimed_acceptance_snapshot(&proposal_id, &dispatch_claim_id)"),
        "artifact bytes must not materialize before ReviewWorkflow proves the claimed accepted decision"
    );
    assert!(
        accept_flow.contains("apply_external_write_artifact(state, &proposal, &dispatch_claim_id)"),
        "artifact materialization must consume the exact accepted dispatch claim"
    );

    let tool_gateway_source = read_repo_file("openlife-core/src/agent/tool_gateway.rs");
    let tool_gateway = strip_cfg_test_module(&tool_gateway_source);
    for foreign_owner in [
        "ResourceGateway",
        "stage_artifact_bytes",
        "commit_staged_artifact",
    ] {
        assert!(
            !tool_gateway.contains(foreign_owner),
            "ToolGateway must not absorb Resource/Artifact canonical ownership: {foreign_owner}"
        );
    }
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
        ) {
            assert!(
                source.contains("reviewRequiredCountFromProjection"),
                "Phase6 product pending state in {path} must use the LifeStateProjection helper"
            );
        }
        if path == "frontend/src/pages/LifeModelPage.tsx" {
            assert!(
                source.contains("viewModel?.pendingUpdateCounts.pendingReview"),
                "R3 LifeModel product pending state must use the backend LifeModelViewModel"
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
        "function BuildSection",
        "function CommunicationStyleCurrentView",
    );
    assert!(
        lifemodel.contains("const pendingCount = viewModel?.pendingUpdateCounts.pendingReview"),
        "LifeModel product pending count must come from LifeModelViewModel"
    );
    assert!(
        builder_review_counter.contains("viewModel?.pendingUpdateCounts.pendingReview"),
        "LifeModel builder review state must come from LifeModelViewModel"
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
fn single_system_statestore_is_the_only_shipped_state_history_read_owner() {
    let raw_source = read_repo_file("src-tauri/src/commands/state.rs");
    let source = strip_cfg_test_module(&raw_source);
    assert!(
        source.contains(".get_product_state_history("),
        "shipped state history and alerts must consume the receipt-gated StateStore product read"
    );
    for forbidden in [
        "state.memory_store",
        ".memory_store.lock()",
        ".get_state_history(&dimension_name",
    ] {
        assert!(
            !source.contains(forbidden),
            "shipped state history must not retain a MemoryStore fallback: {forbidden}"
        );
    }
}

#[test]
fn single_system_statestore_is_the_only_shipped_daily_task_read_owner() {
    let raw_source = read_repo_file("src-tauri/src/commands/state.rs");
    let source = strip_cfg_test_module(&raw_source);
    assert!(
        source.contains(".get_product_daily_tasks("),
        "shipped daily-task reads must consume the receipt-gated StateStore product read"
    );
    assert!(
        source.contains("validate_legacy_yaml_daily_task_cutover_source"),
        "shipped daily-task reads must fail closed on post-cutover legacy YAML drift"
    );
    for forbidden in [
        ".list_daily_tasks(false)",
        ".goals.daily.into_iter()",
        ".filter(|goal| !crate::state_projection::is_state_store_projected_daily_goal(goal))",
        "goals.extend(",
    ] {
        assert!(
            !source.contains(forbidden),
            "shipped daily-task read must not retain YAML/StateStore merge or raw fallback: {forbidden}"
        );
    }

    let bootstrap_raw = read_repo_file("src-tauri/src/bootstrap.rs");
    let bootstrap = strip_cfg_test_module(&bootstrap_raw);
    assert!(
        bootstrap.contains("reconcile_and_import_legacy_yaml_daily_tasks"),
        "startup must stage and atomically import the verified legacy daily-task source"
    );
    let projection_raw = read_repo_file("src-tauri/src/state_projection.rs");
    let projection = strip_cfg_test_module(&projection_raw);
    assert!(
        projection.contains(".get_product_daily_tasks()"),
        "LifeModel compatibility projection must consume receipt-gated canonical daily tasks"
    );
    assert!(
        !projection.contains(".retain(|goal| !is_state_store_projected_daily_goal(goal))"),
        "compatibility projection must not preserve a second unmarked YAML daily-task owner"
    );
}

#[test]
fn single_system_statestore_owned_lifemodel_fields_have_no_shipped_second_writer() {
    let builder_raw = read_repo_file("openlife-core/src/builder/engine.rs");
    let builder = strip_cfg_test_module(&builder_raw);
    assert!(
        builder.contains("life_model_field_authority(&signal.affected_path)"),
        "Builder must consult the shared LifeModel field-authority contract before applying candidates"
    );
    for forbidden in [
        "[\"goals\", \"daily\"] =>",
        "[\"state\", \"alerts\"] =>",
        "goals.daily (merged)",
        "state.alerts (merged)",
    ] {
        assert!(
            !builder.contains(forbidden),
            "Builder must not retain a second StateStore/derived LifeModel writer: {forbidden}"
        );
    }
    assert!(
        builder.contains("\"sig_blocker\"")
            && builder.contains("\"state.open_questions\""),
        "Builder must preserve explicit blocker capability through a reviewed canonical LifeModel field"
    );

    let core_gateway = read_repo_file("openlife-core/src/life_model_write_gateway.rs");
    assert!(
        core_gateway.contains("pub fn life_model_field_authority"),
        "the field-authority classification must have one reusable Core owner"
    );
    let tauri_gateway_raw = read_repo_file("src-tauri/src/life_model_write_gateway.rs");
    let tauri_gateway = strip_cfg_test_module(&tauri_gateway_raw);
    assert!(
        count_occurrences(tauri_gateway, "validate_lifemodel_field_authority(") >= 4,
        "manual/import, accepted Proposal, batch Proposal, and restore writes must enforce field ownership"
    );
    assert!(
        tauri_gateway.contains("STATE_STORE_DAILY_TASK_COMPATIBILITY_MATERIALIZER_ID"),
        "the compatibility exception must be bound to the exact StateStore projector identity"
    );
    let projection_raw = read_repo_file("src-tauri/src/state_projection.rs");
    let projection = strip_cfg_test_module(&projection_raw);
    assert!(
        projection.contains("STATE_STORE_DAILY_TASK_COMPATIBILITY_MATERIALIZER_ID"),
        "the one legitimate compatibility writer must consume the shared projector identity"
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
        "replace_all_messages_guarded(",
        "replace_all_chunks(",
        "replace_portable_chunks_guarded(",
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

#[test]
fn single_system_memory_context_uses_retrieval_truth_not_asset_liveness() {
    let preprocess = read_repo_file("src-tauri/src/main_chat_preprocess.rs");
    let context_loader = read_repo_file("src-tauri/src/main_chat_context_loader.rs");
    let kernel = read_repo_file("src-tauri/src/main_chat_kernel.rs");
    let retired_filter = ["filter_canonical_active", "_memory_results"].concat();

    assert!(!preprocess.contains(&retired_filter));
    assert!(preprocess.contains("filter_canonical_retrievable_memory_results"));
    assert!(preprocess.contains("is_memory_retrievable(memory_id)"));
    assert!(context_loader.contains("retrievable_lifecycle_context_candidates(state).await"));
    assert!(kernel.contains("retrievable_lifecycle_context_candidates(state).await"));
    assert!(context_loader.contains("list_retrievable_records(None, 8)"));

    let action_executor = read_repo_file("openlife-core/src/agent/action_executor/mod.rs");
    let core_os_tools = read_repo_file("openlife-core/src/agent/action_executor/core_os_tools.rs");
    let agent_loop = read_repo_file("openlife-core/src/agent/agent_loop.rs");
    assert!(action_executor.contains("ctx.filter_retrievable_memory_hits("));
    assert!(core_os_tools.contains("ctx.filter_retrievable_memory_hits(hits)"));
    assert!(agent_loop.contains("action_ctx.filter_retrievable_memory_hits("));
}

#[test]
fn single_system_memory_archive_contract_has_no_vector_row_id_authority() {
    let core_os_tools = read_repo_file("openlife-core/src/agent/action_executor/core_os_tools.rs");
    let manifests = read_repo_file("openlife-core/src/mcp.rs");
    assert!(core_os_tools.contains("validated_memory_archive_proposal(args)?"));
    assert!(manifests.contains("memory_archive_owner_parameters()"));
}

#[test]
fn single_system_posthoc_proposal_engine_and_product_consumers_stay_absent() {
    assert!(
        !repo_root()
            .join("openlife-core/src/agent/proposal_engine.rs")
            .exists(),
        "the former product-wired ProposalEngine must not return as a second proposal authority"
    );
    for former_path in [
        "openlife-core/src/agent/proposal_generators/chat.rs",
        "openlife-core/src/agent/proposal_generators/mod.rs",
    ] {
        assert!(
            !repo_root().join(former_path).exists(),
            "the former ProposalEngine generator dependency must stay absent: {former_path}"
        );
    }

    let inventory = inventory();
    let inventoried_ids = inventory_entries(&inventory, "old_runtime_surfaces")
        .into_iter()
        .map(|entry| entry_str(entry, "id"))
        .collect::<BTreeSet<_>>();
    for required_consumer in [
        "core_posthoc_proposal_engine_module",
        "proposal_engine_app_state_owner_symbol",
        "proposal_engine_bootstrap_constructor_symbol",
        "proposal_engine_bootstrap_chat_adapter_registration_symbol",
        "proposal_engine_main_chat_posthoc_generator_symbol",
        "proposal_engine_main_chat_posthoc_gate_symbol",
        "proposal_engine_agent_run_replay_consumer_symbol",
        "proposal_engine_agent_module_reexport_symbol",
    ] {
        assert!(
            inventoried_ids.contains(required_consumer),
            "ProposalEngine deletion inventory must retain the real former product consumer: {required_consumer}"
        );
    }

    let retired_authority_markers = [
        "ProposalEngine",
        "ProposalGenerator",
        "ChatProposalGeneratorAdapter",
        "generate_and_persist_chat_proposals",
        "should_generate_chat_proposals",
        "generate_from_run(&run",
    ];
    for file in source_files(&["openlife-core/src", "src-tauri/src", "frontend/src"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with("_tests.rs")
            || rel.ends_with(".test.ts")
            || rel.ends_with(".test.tsx")
            || rel == "src-tauri/src/single_system_authority_tests.rs"
            || rel.starts_with("openlife-core/src/agent/tests/")
            || rel.starts_with("frontend/src/test/")
        {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        let product_source = strip_cfg_test_module(&source);
        for retired in retired_authority_markers {
            assert!(
                !product_source.contains(retired),
                "former post-hoc proposal authority or product consumer returned as {retired}: {rel}"
            );
        }
    }

    let command_surface = read_repo_file("src-tauri/src/main_chat_command_surface_tests.rs");
    assert!(
        command_surface.contains("ordinary_chat_finalization_never_creates_post_hoc_proposals"),
        "ordinary provider or assistant output must retain a behavioral counterexample against post-hoc Proposal creation"
    );
    let review_memory = read_repo_file("frontend/src/pages/settings/tabs/ReviewMemoryTab.tsx");
    assert!(
        review_memory.contains("后端 PolicyRouter 路由")
            && review_memory.contains("按后端回执显示"),
        "Mailbox & Memory must describe backend-owned governance after inert controls are removed"
    );

    let manifest = read_repo_file("plans/openlife_single_system_deletion_manifest.md");
    let preparation = read_repo_file("plans/openlife_single_system_development_preparation.md");
    for retired in [
        "callerless ProposalEngine",
        "zero-consumer ProposalEngine",
        "ProposalEngine and its feedback, memory, tool, builder, and calibration generators formed a dormant",
    ] {
        assert!(
            !manifest.contains(retired) && !preparation.contains(retired),
            "active deletion authority must not restore the false callerless ProposalEngine history: {retired}"
        );
    }
}

#[test]
fn single_system_d011_conversation_and_retrieval_parallel_routes_stay_absent() {
    let preprocess = read_repo_file("src-tauri/src/main_chat_preprocess.rs");
    let kernel = read_repo_file("src-tauri/src/main_chat_kernel.rs");
    let generation = read_repo_file("src-tauri/src/main_chat_generation_support.rs");
    let runtime = read_repo_file("src-tauri/src/main_chat_turn_runtime.rs");
    let gateway = read_repo_file("src-tauri/src/memory_gateway.rs");
    let memory_store = read_repo_file("openlife-core/src/memory.rs");
    let action_context = read_repo_file("openlife-core/src/agent/action_executor/mod.rs");

    for retired in [
        "preprocess_chat_input_v2",
        "save_turn_message_if_needed",
        "persist_chat_message_if_needed",
    ] {
        assert!(
            !preprocess.contains(retired)
                && !kernel.contains(retired)
                && !generation.contains(retired)
                && !gateway.contains(retired),
            "D011 retired content-last/parallel preprocessing route returned: {retired}"
        );
    }
    assert!(runtime.contains("save_turn_user_message_idempotent_with_state("));
    assert!(generation.contains("main_chat_assistant_message_operation_id("));
    assert!(generation.contains("main_chat_assistant_message:{task_session_id}:{run_id}"));
    assert!(gateway.contains("save_conversation_message_idempotent_with_state("));

    assert!(memory_store.contains("reject_memory_lifecycle_retrieval_insert"));
    assert!(memory_store
        .contains("MemoryLifecycle retrieval disposition is owned by MemoryLifecycleStore"));
    assert!(action_context.contains("MemoryRetrievalAuthorityError"));
    assert!(action_context.contains("memory_lifecycle_reader_unavailable"));
}

#[test]
fn single_system_d049_keyword_conversation_update_routes_stay_absent() {
    let inventory = inventory();
    let conversation_update_entry = inventory_entries(&inventory, "old_runtime_surfaces")
        .into_iter()
        .find(|entry| entry_str(entry, "id") == "main_chat_conversation_updates_module")
        .expect("D049 conversation-update deletion inventory entry");
    let declared_exact_symbols = conversation_update_entry
        .get("former_symbols")
        .and_then(serde_json::Value::as_array)
        .expect("D049 inventory former_symbols")
        .iter()
        .map(|value| value.as_str().expect("D049 former symbol string"))
        .collect::<BTreeSet<_>>();
    let declared_semantic_markers = conversation_update_entry
        .get("semantic_markers")
        .and_then(serde_json::Value::as_array)
        .expect("D049 inventory semantic_markers")
        .iter()
        .map(|value| value.as_str().expect("D049 semantic marker string"))
        .collect::<BTreeSet<_>>();

    let retired_exact_symbols = [
        ["try", "_auto_checkin_daily_goals"].concat(),
        ["build", "_reasoning_trace_prompt"].concat(),
        ["capture", "_conversation_signals"].concat(),
    ];
    let retired_semantic_markers = [["OrdinaryChat", "AutoCheckinSourceData"].concat()];
    assert_eq!(
        declared_exact_symbols,
        retired_exact_symbols
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        "D049 inventory must enumerate every exact retired conversation-update symbol"
    );
    assert_eq!(
        declared_semantic_markers,
        retired_semantic_markers
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        "D049 inventory must enumerate semantic markers that could rename the retired route"
    );

    {
        let former_path = "src-tauri/src/main_chat_conversation_updates.rs";
        assert!(
            !repo_root().join(former_path).exists(),
            "D049 callerless conversation-inference module must stay absent: {former_path}"
        );
    }

    for file in source_files(&["openlife-core/src", "src-tauri/src", "frontend/src"]) {
        let rel = to_repo_path(&file);
        if rel.ends_with("_tests.rs")
            || rel.ends_with(".test.ts")
            || rel.ends_with(".test.tsx")
            || rel == "src-tauri/src/single_system_authority_tests.rs"
            || rel.starts_with("openlife-core/src/agent/tests/")
            || rel.starts_with("frontend/src/test/")
        {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        let production = strip_cfg_test_module(&source);
        for retired in retired_exact_symbols
            .iter()
            .chain(retired_semantic_markers.iter())
        {
            assert!(
                !production.contains(retired.as_str()),
                "D049 retired keyword conversation-update route returned as {retired}: {rel}"
            );
        }
    }

    let command_surface = read_repo_file("src-tauri/src/main_chat_command_surface_tests.rs");
    let auto_checkin_counterexample = [
        "main_chat_kernel_goal_4_ordinary_",
        "auto_checkin_does_not_materialize_truth",
    ]
    .concat();
    assert!(
        command_surface.contains(&auto_checkin_counterexample),
        "ordinary Main Chat must retain the counterexample proving auto-checkin cannot silently materialize truth"
    );
    let auto_checkin_body = source_between(
        &command_surface,
        "async fn main_chat_kernel_goal_4_ordinary_auto_checkin_does_not_materialize_truth()",
        "async fn inferred_memory_review_preserves_direct_answer_and_truthful_proposal_reason()",
    );
    assert!(
        auto_checkin_body.contains("implicit_life_event_ids.is_empty()")
            && auto_checkin_body.contains("list_command_surface_life_events(&send_state)")
            && auto_checkin_body.contains(".is_empty()"),
        "D049 counterexample must assert zero inferred LifeEvent ids and zero canonical LifeEvent rows, not merely keep the old test name"
    );
    assert!(
        !auto_checkin_body.contains("auto-checkin life event ids")
            && !auto_checkin_body.contains("await.len(), 1"),
        "D049 retired auto-checkin expectation must not return behind a green string-only guard"
    );
}
