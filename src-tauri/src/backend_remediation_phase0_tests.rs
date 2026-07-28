use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

const FROZEN_SCENARIO_SUITE_SHA256: &str =
    "e969e091777134c62d388c012149c056813ee0c4eb290307c47cf8b439802482";
const FROZEN_FINDING_INVENTORY_SHA256: &str =
    "3126792be60df62df73ca9e574d8dc6726e2d18862495a31bda85104ca1095fe";
const PHASE0_BASELINE_REVISION: &str = "1ca7613bcd25167cf173fa0a21e3baa908f21d94";
const DISCOVERED_FINDING_IMMUTABLE_PREFIX_COUNT: usize = 53;
const DISCOVERED_FINDING_IMMUTABLE_PREFIX_SHA256: &str =
    "f1bd733a0faf7d50e89b67988dec078621c632dd0d1274e5a754d0010974eb12";
const DISCOVERED_CORRECTION_IMMUTABLE_PREFIX_COUNT: usize = 2;
const DISCOVERED_CORRECTION_IMMUTABLE_PREFIX_SHA256: &str =
    "3aee333404641837363c230e5635ccfbec98956acda6ce3e8ff2ee705ee1b9d9";
const DISCOVERED_FINDING_CLASSIFICATIONS: &[&str] = &[
    "net_new_root_cause",
    "frozen_finding_scope_expansion",
    "evidence_gap",
    "evidence_gap_and_scope_correction",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has repo parent")
        .to_path_buf()
}

fn read_repo_file(path: &str) -> String {
    fs::read_to_string(repo_root().join(path)).unwrap_or_else(|err| panic!("read {path}: {err}"))
}

fn canonical_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            let mut canonical = serde_json::Map::new();
            for key in keys {
                canonical.insert(key.clone(), canonical_json(&object[key]));
            }
            serde_json::Value::Object(canonical)
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(canonical_json).collect())
        }
        scalar => scalar.clone(),
    }
}

fn immutable_registry_record_fingerprint(record: &serde_json::Value) -> String {
    let mut immutable = record.clone();
    immutable
        .as_object_mut()
        .expect("discovered finding object")
        .remove("immutable_fingerprint");
    let bytes = serde_json::to_vec(&canonical_json(&immutable))
        .expect("serialize canonical discovered finding");
    format!("{:x}", Sha256::digest(bytes))
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn assert_executable_test_reference(reference: &str) {
    let body = reference
        .strip_prefix("test:")
        .expect("test evidence prefix");
    let (path, test_names) = body
        .rsplit_once("::")
        .map_or((body, None), |(path, name)| (path, Some(name)));
    let source = read_repo_file(path);
    let Some(test_names) = test_names else {
        return;
    };
    for test_name in test_names.split('+') {
        let sync_signature = format!("fn {test_name}(");
        let async_signature = format!("async fn {test_name}(");
        let offset = source
            .find(&async_signature)
            .or_else(|| source.find(&sync_signature))
            .unwrap_or_else(|| panic!("test evidence does not resolve to a function: {reference}"));
        let annotation_window = &source[offset.saturating_sub(500)..offset];
        assert!(
            annotation_window.contains("#[test]")
                || annotation_window.contains("#[tokio::test")
                || annotation_window.contains("#[rstest"),
            "test evidence does not resolve to an annotated test: {reference}"
        );
    }
}

fn assert_source_reference(reference: &str) {
    if reference.starts_with("test:") {
        assert_executable_test_reference(reference);
        return;
    }
    if let Some(body) = reference.strip_prefix("baseline-source:") {
        let path = body.split("::").next().expect("baseline source path");
        let object = format!("{PHASE0_BASELINE_REVISION}:{path}");
        let status = Command::new("git")
            .args(["cat-file", "-e", &object])
            .current_dir(repo_root())
            .status()
            .unwrap_or_else(|err| panic!("resolve baseline source {reference}: {err}"));
        assert!(
            status.success(),
            "baseline source does not exist: {reference}"
        );
        return;
    }
    if let Some(body) = reference.strip_prefix("current-fix:") {
        let path = body.split("::").next().expect("current fix source path");
        assert!(
            repo_root().join(path).is_file(),
            "missing current fix: {reference}"
        );
        return;
    }
    if let Some(body) = reference.strip_prefix("current-authority:") {
        let (path, symbols) = body
            .split_once("::")
            .unwrap_or_else(|| panic!("current authority must identify symbols: {reference}"));
        let source = read_repo_file(path);
        for symbol in symbols.split('+').flat_map(|symbol| symbol.split('.')) {
            assert!(
                source.contains(symbol),
                "current authority symbol is absent: {reference} ({symbol})"
            );
        }
        return;
    }
    if let Some(body) = reference.strip_prefix("deleted-authority:") {
        let (path, symbols) = body
            .split_once("::")
            .unwrap_or_else(|| panic!("deleted authority must identify symbols: {reference}"));
        let path = repo_root().join(path);
        if !path.is_file() {
            return;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read deleted-authority source {reference}: {error}"));
        for symbol in symbols.split('+') {
            assert!(
                !source.contains(symbol),
                "deleted authority symbol is still present: {reference} ({symbol})"
            );
        }
        return;
    }
    if let Some(body) = reference.strip_prefix("baseline-evidence:") {
        let path = body.split('#').next().expect("baseline evidence path");
        assert!(
            repo_root().join(path).is_file(),
            "missing evidence file: {reference}"
        );
        return;
    }
    panic!("unsupported typed source reference: {reference}");
}

#[test]
fn backend_remediation_phase0_inventory_freezes_all_audit_findings() {
    let inventory: serde_json::Value = serde_json::from_str(&read_repo_file(
        "plans/openlife_backend_remediation_v4_inventory.json",
    ))
    .expect("parse backend remediation inventory");
    assert_eq!(
        inventory["schema_version"],
        "openlife-backend-remediation-v4"
    );
    assert_eq!(inventory["status"], "active");
    assert_eq!(inventory["authority"], "subordinate-to-phase7");

    let findings = inventory["findings"]
        .as_array()
        .expect("findings must be an array");
    assert_eq!(findings.len(), 35, "the full audit has 35 frozen findings");

    let ids = findings
        .iter()
        .map(|finding| {
            for field in [
                "id",
                "severity",
                "title",
                "root_cause",
                "phase",
                "owner_module",
                "reproduction",
                "acceptance",
                "deletion_target",
            ] {
                assert!(
                    finding
                        .get(field)
                        .and_then(serde_json::Value::as_str)
                        .is_some(),
                    "finding must include non-null string {field}: {finding}"
                );
            }
            finding["id"].as_str().unwrap().to_string()
        })
        .collect::<BTreeSet<_>>();
    let expected = (1..=3)
        .map(|index| format!("P0-{index:02}"))
        .chain((1..=21).map(|index| format!("P1-{index:02}")))
        .chain((1..=11).map(|index| format!("P2-{index:02}")))
        .collect::<BTreeSet<_>>();
    assert_eq!(ids, expected);

    let scenario_suite = inventory["frozen_scenario_suite"]
        .as_object()
        .expect("frozen scenario suite contract");
    assert_eq!(scenario_suite["scenario_count"], 40);
    assert_eq!(scenario_suite["change_policy"], "versioned-waiver-required");

    let suite: serde_json::Value = serde_json::from_str(&read_repo_file(
        "plans/openlife_backend_remediation_v4_scenarios.json",
    ))
    .expect("parse frozen remediation scenarios");
    assert_eq!(suite["schema_version"], "openlife-backend-scenarios-v1");
    assert_eq!(suite["status"], "frozen");
    assert_eq!(suite["change_policy"], "versioned-waiver-required");
    let scenarios = suite["scenarios"]
        .as_array()
        .expect("scenarios must be an array");
    assert_eq!(scenarios.len(), 40);
    let scenario_ids = scenarios
        .iter()
        .map(|scenario| {
            for field in [
                "id",
                "group",
                "prompt",
                "expected_action",
                "expected_route",
                "proposal_expectation",
                "durable_effect",
                "external_effect",
                "evidence_requirement",
            ] {
                assert!(
                    scenario
                        .get(field)
                        .and_then(serde_json::Value::as_str)
                        .is_some(),
                    "scenario must include string {field}: {scenario}"
                );
            }
            scenario["id"].as_str().unwrap().to_string()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(scenario_ids.len(), 40, "scenario ids must be unique");
    let group_counts = scenarios
        .iter()
        .fold(BTreeMap::new(), |mut counts, scenario| {
            *counts
                .entry(scenario["group"].as_str().expect("scenario group"))
                .or_insert(0usize) += 1;
            counts
        });
    assert_eq!(
        group_counts,
        BTreeMap::from([
            ("ordinary_chat_planning_writing", 8),
            ("explicit_and_inferred_memory", 6),
            ("privacy_and_provider", 6),
            ("web_file_and_local_reads", 6),
            ("tool_permission_and_external_writes", 6),
            ("cancellation_resume_and_concurrency", 4),
            ("realistic_chinese_ambiguity", 4),
        ])
    );

    let rubric = suite["rubric"].as_object().expect("frozen rubric contract");
    assert_eq!(rubric["minimum_executable_task_success_percent"], 90);
    assert_eq!(rubric["ordinary_unexpected_proposal_count"], 0);

    let raw_suite = read_repo_file("plans/openlife_backend_remediation_v4_scenarios.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw_suite.as_bytes())),
        FROZEN_SCENARIO_SUITE_SHA256,
        "the v1 scenario prompts, expectations, execution profiles, and rubric are immutable; create a new suite plus human waiver instead of editing v1"
    );
    let execution = suite["execution_contract"]
        .as_object()
        .expect("scenario execution contract");
    let profiles = execution["group_profiles"]
        .as_object()
        .expect("group execution profiles");
    let scenario_groups = scenarios
        .iter()
        .map(|scenario| scenario["group"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        profiles.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        scenario_groups
    );
    for profile in profiles.values() {
        for field in [
            "seed_state",
            "steps",
            "observations",
            "cleanup",
            "evaluator",
        ] {
            assert!(profile.get(field).is_some(), "profile missing {field}");
        }
    }
    for score in ["1", "2", "3", "4", "5"] {
        assert!(rubric["helpfulness_anchors"][score].is_string());
    }
    let overrides = execution["scenario_overrides"]
        .as_object()
        .expect("scenario overrides");
    assert_eq!(
        overrides
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["RUN-01", "RUN-02", "RUN-03", "RUN-04"])
    );
    assert!(overrides
        .keys()
        .all(|scenario_id| scenario_ids.contains(scenario_id)));

    let waivers: serde_json::Value = serde_json::from_str(&read_repo_file(
        "plans/openlife_backend_remediation_v4_scenario_waivers.json",
    ))
    .expect("parse scenario waiver registry");
    assert_eq!(waivers["suite_id"], suite["suite_id"]);
    assert_eq!(waivers["frozen_suite_sha256"], FROZEN_SCENARIO_SUITE_SHA256);
    assert_eq!(
        waivers["policy"]["implementation_authors_may_self_approve"],
        false
    );
    let required_waiver_fields = waivers["policy"]["required_fields_for_each_waiver"]
        .as_array()
        .expect("required waiver fields");
    for waiver in waivers["waivers"].as_array().expect("waiver registry") {
        for field in required_waiver_fields {
            let field = field.as_str().expect("waiver field name");
            let value = waiver
                .get(field)
                .unwrap_or_else(|| panic!("waiver missing required field {field}: {waiver}"));
            assert!(
                !value.is_null()
                    && value.as_str().is_none_or(|text| !text.trim().is_empty())
                    && value.as_array().is_none_or(|items| !items.is_empty()),
                "waiver field must contain review evidence: {field}"
            );
        }
        assert_ne!(waiver["old_suite_id"], waiver["new_suite_id"]);
    }

    let traceability: serde_json::Value = serde_json::from_str(&read_repo_file(
        "plans/openlife_backend_remediation_v4_traceability.json",
    ))
    .expect("parse remediation traceability matrix");
    let invariant_ids = traceability["invariants"]
        .as_object()
        .expect("invariant catalog")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let rows = traceability["findings"]
        .as_array()
        .expect("traceability findings");
    assert_eq!(rows.len(), 35);
    let allowed_statuses = BTreeSet::from([
        "binary-blob-added-performance-proof-pending",
        "fix-under-verification",
        "fix-under-verification-concurrency-limit-pending",
        "fix-under-verification-full-suite-pending",
        "fix-under-verification-full-symbol-scan-pending",
        "fix-under-verification-independent-review-pending",
        "fix-under-verification-live-redirect-pending",
        "fix-under-verification-real-keychain-smoke-pending",
        "fix-under-verification-restart-parity-pending",
        "fix-under-verification-store-migration-pending",
        "local-claim-fixed-remote-reconciliation-pending",
        "mechanical-gates-green-independent-review-pending",
        "model-route-fixed-migration-proof-pending",
        "network-client-integrated-authentication-pending",
        "phase7-deletion-in-progress",
        "provider-facts-partial-projection-pending",
        "dependency-gate-green-owned-advisories-remediation-open",
        "release-quarantined-authentication-pending",
        "release-quarantined-phase4-pending",
        "reproduced-dependency-gate-pending",
        "reproduced-fix-pending",
        "reproduced-fix-under-verification",
        "reproduced-phase3-pending",
        "reproduced-quality-gate-pending",
        "reproduced-test-architecture-pending",
        "root-fix-focused-verification",
    ]);
    let traceability_ids = rows
        .iter()
        .map(|row| {
            let id = row["id"].as_str().expect("traceability id").to_string();
            for field in [
                "source_refs",
                "reproduction_ref",
                "positive_verification_ref",
                "counterfactual_verification_ref",
                "non_regression_scenarios",
                "status",
            ] {
                assert!(row.get(field).is_some(), "{id} missing {field}");
            }
            for field in [
                "broken_invariants",
                "source_refs",
                "non_regression_scenarios",
            ] {
                assert!(
                    !row[field]
                        .as_array()
                        .expect("traceability array")
                        .is_empty(),
                    "{id} has empty {field}"
                );
            }
            for source_ref in row["source_refs"].as_array().expect("source refs") {
                assert_source_reference(source_ref.as_str().expect("typed source reference"));
            }
            let reproduction_ref = row["reproduction_ref"]
                .as_str()
                .expect("typed reproduction reference");
            if let Some(body) = reproduction_ref.strip_prefix("inventory:") {
                let (path, fragment) = body
                    .split_once('#')
                    .expect("inventory reproduction must identify one finding");
                assert!(repo_root().join(path).is_file());
                assert_eq!(
                    fragment, id,
                    "inventory reproduction points at another finding"
                );
            } else {
                assert_source_reference(reproduction_ref);
            }
            for evidence_ref in [
                row["positive_verification_ref"]
                    .as_str()
                    .expect("positive verification ref"),
                row["counterfactual_verification_ref"]
                    .as_str()
                    .expect("counterfactual verification ref"),
            ] {
                if evidence_ref.starts_with("test:") {
                    assert_executable_test_reference(evidence_ref);
                } else if let Some(gate) = evidence_ref.strip_prefix("gate:") {
                    assert!(
                        !gate.trim().is_empty()
                            && gate
                                .chars()
                                .all(|character| character.is_ascii_alphanumeric()
                                    || character == '-'),
                        "{id} has a malformed mechanical gate reference: {evidence_ref}"
                    );
                } else {
                    assert!(
                        evidence_ref.starts_with("planned:"),
                        "{id} uses unsupported evidence reference: {evidence_ref}"
                    );
                }
            }
            for invariant in row["broken_invariants"]
                .as_array()
                .expect("broken invariants")
            {
                assert!(
                    invariant_ids.contains(invariant.as_str().expect("invariant id")),
                    "{id} references an unknown invariant"
                );
            }
            for scenario_id in row["non_regression_scenarios"]
                .as_array()
                .expect("non-regression scenarios")
            {
                assert!(
                    scenario_ids.contains(scenario_id.as_str().expect("scenario id")),
                    "{id} references an unknown frozen scenario"
                );
            }
            let status = row["status"].as_str().expect("traceability status");
            assert!(
                allowed_statuses.contains(status),
                "{id} uses an unreviewed or completion-like traceability status: {status}"
            );
            id
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(traceability_ids, ids);
}

#[test]
fn backend_remediation_phase0_active_documents_and_adr_amendment_are_present() {
    let plan = read_repo_file("plans/openlife_backend_remediation_v4.md");
    assert!(plan.contains("Status: active implementation work package"));
    assert!(plan.contains("subordinate to the Phase7 contract"));
    assert!(plan.contains("Threat Model"));
    assert!(plan.contains("Rollback And Backout"));
    assert!(plan.contains("Frozen Scenario Suite"));

    let adr = read_repo_file("plans/adr/0014-explicit-user-memory-write-lane.md");
    assert!(adr.contains("Status: accepted"));
    assert!(adr.contains("amends ADR 0013"));
    assert!(adr.contains("current_authenticated_user_message"));
    assert!(adr.contains("must not directly mutate canonical HS assets"));
}

#[test]
fn backend_remediation_phase0_release_capabilities_fail_closed() {
    let release_capability = read_repo_file("src-tauri/capabilities/default.json");
    let release: serde_json::Value =
        serde_json::from_str(&release_capability).expect("parse release capability");
    assert!(
        release.get("remote").is_none(),
        "release capability must not authorize localhost or other remote URLs"
    );
    let release_permissions = release["permissions"]
        .as_array()
        .expect("release permissions are an array")
        .iter()
        .map(|permission| {
            permission.as_str().unwrap_or_else(|| {
                panic!(
                    "release capability contains an unreviewed scoped/object permission: {permission}"
                )
            })
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        release_permissions,
        BTreeSet::from([
            "core:default",
            "dialog:default",
            "fs:allow-read-text-file",
            "fs:allow-write-text-file",
        ]),
        "the release WebView must not receive recursive AppData, shell, store, or HTTP capability"
    );

    let dev_capability = read_repo_file("src-tauri/capabilities/dev-extensions.json");
    let dev: serde_json::Value =
        serde_json::from_str(&dev_capability).expect("parse dev capability");
    assert!(dev.get("remote").is_some());

    let release_config = read_repo_file("src-tauri/tauri.conf.json");
    let config: serde_json::Value =
        serde_json::from_str(&release_config).expect("parse release Tauri config");
    assert!(
        config["app"]["security"]["csp"].is_string(),
        "release CSP must be explicit and non-null"
    );

    let dev_config = read_repo_file("src-tauri/tauri.dev.conf.json");
    let dev_config: serde_json::Value =
        serde_json::from_str(&dev_config).expect("parse dev Tauri config");
    assert_eq!(
        dev_config["app"]["security"]["capabilities"],
        serde_json::json!(["default", "dev-extensions"])
    );
}

#[test]
fn backend_remediation_phase0_high_risk_commands_are_dev_only() {
    let product_manifest = read_repo_file("src-tauri/Cargo.toml");
    assert!(product_manifest.contains("dev-extensions = []"));
    assert!(!product_manifest.contains("openlife-a2a-server"));
    let dev_server_manifest = read_repo_file("tools/openlife-a2a-server/Cargo.toml");
    assert!(dev_server_manifest.contains("required-features = [\"dev-extensions\"]"));

    let source = read_repo_file("src-tauri/src/lib.rs");
    assert!(source.contains(
        "#[cfg(all(feature = \"dev-extensions\", not(debug_assertions)))]\ncompile_error!"
    ));
    for command in [
        "execute_tool_call",
        "register_mcp_server",
        "unregister_mcp_server",
        "inspect_mcp_call",
        "list_mcp_servers",
        "list_mcp_tools",
        "list_mcp_templates",
        "recommend_mcp_manifests",
        "list_mcp_audit_logs",
        "clear_mcp_audit_logs",
        "export_mcp_audit_logs",
        "cleanup_mcp_audit_logs",
        "rotate_mcp_audit_key",
        "list_plugins",
        "reload_plugins",
        "enable_plugin",
        "disable_plugin",
    ] {
        let guarded = format!("#[cfg(feature = \"dev-extensions\")]\n            {command},");
        assert!(
            source.contains(&guarded),
            "{command} must be absent from the release handler and compiled into the handler only with dev-extensions"
        );
    }
    assert!(source.contains("OPENLIFE_DEV_AUTOSTART_A2A"));
    assert!(source.contains("require_authenticated_dev_a2a_opt_in"));
    assert!(!source.contains("OPENLIFE_ENABLE_UNAUTHENTICATED_DEV_A2A"));
    assert!(!source.contains("falling back to embedded a2a server"));
    assert!(!source.contains("OPENLIFE_AUTOSTART_FILESYSTEM_MCP"));
    assert!(source.contains("#[cfg(debug_assertions)]\nfn runtime_dev_url()"));
    assert!(source.contains(
        "#[cfg(feature = \"dev-extensions\")]\n            start_dev_extension_background_workers"
    ));

    let bootstrap = read_repo_file("src-tauri/src/bootstrap.rs");
    assert!(bootstrap.contains(
        "#[cfg(not(feature = \"dev-extensions\"))]\n    let mcp_registry = McpRegistry::new_release_product();"
    ));

    let sidecar = read_repo_file("src-tauri/src/a2a_sidecar.rs");
    assert!(sidecar.contains("A2A_PARENT_PIPE_GUARD_ENV"));
    assert!(sidecar.contains(".stdin(Stdio::piped())"));
    let sidecar_bin = read_repo_file("tools/openlife-a2a-server/src/main.rs");
    assert!(sidecar_bin.contains("wait_for_parent_pipe_close"));
    assert!(sidecar_bin.contains("A2A_PARENT_PIPE_GUARD_ENV"));
    let ci = read_repo_file(".github/workflows/ci.yml");
    assert_eq!(
        ci.matches(
            "cargo test -p openlife-a2a-server --features dev-extensions --test parent_guard --locked"
        )
        .count(),
        3,
        "Linux, macOS, and Windows CI must execute the feature-gated A2A parent lifecycle test"
    );
}

#[test]
fn backend_remediation_phase0_a2a_has_one_router_and_no_embedded_server_owner() {
    let library_server = read_repo_file("src-tauri/src/a2a_server.rs");
    let binary_entrypoint = read_repo_file("tools/openlife-a2a-server/src/main.rs");
    let combined = format!("{library_server}\n{binary_entrypoint}");

    assert!(
        !library_server.contains("pub async fn start("),
        "the callerless embedded A2A server must remain deleted"
    );
    assert_eq!(
        combined.matches(".route(\"/tasks/send\"").count(),
        1,
        "A2A task routing must have one protocol owner"
    );
    assert_eq!(
        combined.matches("fn require_paired_bearer(").count(),
        1,
        "paired authentication must not be copied into the binary entrypoint"
    );
    assert_eq!(
        combined.matches("DefaultBodyLimit::max(").count(),
        1,
        "the A2A request body limit must have one owner"
    );
    assert!(binary_entrypoint.contains("build_a2a_router"));
    assert!(binary_entrypoint.contains("load_persisted_a2a_runtime_state"));
    assert!(!binary_entrypoint.contains("PrivacyEngine::new()"));
}

#[test]
fn backend_remediation_phase0_release_registry_does_not_expose_a2a_execution() {
    let registry = openlife_core::mcp::McpRegistry::new_release_product();
    assert!(!registry
        .list_manifests()
        .iter()
        .any(|manifest| manifest.name == "a2a.call_agent"));
    assert!(!registry
        .list_manifests()
        .iter()
        .any(|manifest| manifest.name == "mcp.call_tool"));
    assert!(!registry
        .list_manifests()
        .iter()
        .any(|manifest| manifest.name == "builtin_echo"));
    assert!(registry
        .list_manifests()
        .iter()
        .any(|manifest| manifest.name == "web.search"));

    let mut registry = openlife_core::mcp::McpRegistry::new();
    registry.register_dev_a2a_tool();
    assert!(registry
        .list_manifests()
        .iter()
        .any(|manifest| manifest.name == "a2a.call_agent"));
}

#[test]
fn backend_remediation_phase0_release_package_owns_no_a2a_binary_target() {
    let product_manifest = read_repo_file("src-tauri/Cargo.toml");
    assert!(
        !product_manifest.contains("openlife-a2a-server")
            && !product_manifest.contains("src/bin/a2a_server.rs"),
        "the Tauri product package must not advertise a development A2A binary to the release bundler"
    );

    let dev_server_manifest = read_repo_file("tools/openlife-a2a-server/Cargo.toml");
    assert!(dev_server_manifest.contains("name = \"openlife-a2a-server\""));
    assert!(dev_server_manifest.contains("openlife-tauri/dev-extensions"));
    assert!(
        read_repo_file("Cargo.toml").contains("tools/openlife-a2a-server"),
        "the quarantined A2A server must remain an explicit workspace development tool"
    );
}

#[test]
fn backend_remediation_phase0_startup_keyring_is_bounded_and_noninteractive() {
    let bootstrap = read_repo_file("src-tauri/src/bootstrap.rs");
    assert!(bootstrap.contains("let secret_store = StartupKeyringSecretStore::default();"));
    assert!(!bootstrap.contains("bootstrap_with_secret_store(data_dir, &KeyringSecretStore)"));

    let secret_store = read_repo_file("src-tauri/src/secret_store.rs");
    assert!(secret_store.contains("STARTUP_SECRET_OPERATION_TIMEOUT"));
    assert!(secret_store.contains("recv_timeout(timeout)"));
    assert!(secret_store.contains("disable_user_interaction()"));
    assert!(secret_store.contains("prior bounded timeout"));
}

fn nkr_s1_product_bootstrap_writer(bootstrap: &str) -> Option<&'static str> {
    let product_bootstrap = bootstrap
        .split_once("\nfn bootstrap_with_secret_store(\n")
        .map(|(_, source)| source)
        .expect("bootstrap product implementation")
        .split_once("\n#[cfg(test)]\nmod tests")
        .map(|(source, _)| source)
        .expect("bootstrap product implementation boundary");
    [
        "hydrate_or_create_integrity_key(",
        "hydrate_or_create_canonical_store_integrity_key(",
        "ensure_write_epoch(secret_store)",
        "save_mcp_audit_keyring_to_path(",
    ]
    .into_iter()
    .find(|forbidden| product_bootstrap.contains(forbidden))
}

#[test]
fn nkr_s1_credential_startup_keyring_is_compile_time_read_only_and_projection_owned() {
    let bootstrap = read_repo_file("src-tauri/src/bootstrap.rs");
    assert_eq!(nkr_s1_product_bootstrap_writer(&bootstrap), None);
    for forbidden in [
        "hydrate_or_create_integrity_key(",
        "hydrate_or_create_canonical_store_integrity_key(",
        "ensure_write_epoch(secret_store)",
        "save_mcp_audit_keyring_to_path(",
    ] {
        let counterexample = format!(
            "header\nfn bootstrap_with_secret_store(\n) {{ {forbidden} }}\n#[cfg(test)]\nmod tests {{}}"
        );
        assert_eq!(
            nkr_s1_product_bootstrap_writer(&counterexample),
            Some(forbidden),
            "source guard accepted forbidden counterexample: {forbidden}"
        );
    }

    let secret_store = read_repo_file("src-tauri/src/secret_store.rs");
    assert!(!secret_store.contains("impl SecretStore for StartupKeyringSecretStore"));
    assert!(secret_store.contains("impl SecretReader for StartupKeyringSecretStore"));
    assert!(!secret_store.contains("OPENLIFE_NATIVE_TAURI_KEYCHAIN_SERVICE={service}"));

    let state = read_repo_file("src-tauri/src/state.rs");
    let projection = read_repo_file("src-tauri/src/life_state_projection.rs");
    assert!(state.contains("CredentialBootstrapSnapshot"));
    assert!(projection.contains("credential_bootstrap"));
}

#[test]
fn native_isolation_keychain_override_is_debug_dev_only_and_fail_closed() {
    let lib = read_repo_file("src-tauri/src/lib.rs");
    assert!(lib.contains(
        "#[cfg(all(feature = \"dev-extensions\", not(debug_assertions)))]\ncompile_error!"
    ));

    let secret_store = read_repo_file("src-tauri/src/secret_store.rs");
    assert!(secret_store.contains(
        "#[cfg(all(feature = \"dev-extensions\", debug_assertions))]\nconst TRIAL_KEYCHAIN_SERVICE_PREFIX"
    ));
    assert!(
        secret_store.contains("#[cfg(not(all(feature = \"dev-extensions\", debug_assertions)))]")
    );
    assert!(secret_store.contains("native isolation trial requires an explicit Keychain service"));
    assert!(secret_store
        .contains("Keychain service override requires the native isolation trial marker"));
    assert!(secret_store.contains("suffix must be at least 32 lowercase hex characters"));
    assert_eq!(
        secret_store
            .matches("OPENLIFE_NATIVE_TAURI_KEYCHAIN_SERVICE={service}")
            .count(),
        0
    );
    assert_eq!(
        secret_store
            .matches("OPENLIFE_NATIVE_TAURI_KEYCHAIN_SERVICE_CLASS=isolated_trial")
            .count(),
        1
    );
    assert!(secret_store.contains("keyring_entry_for_service(&service, secret_ref)"));
    assert!(secret_store.contains("static SELECTED_KEYRING_SERVICE: OnceLock"));
    assert_eq!(secret_store.matches("keyring::Entry::new(").count(), 1);
    assert_eq!(
        secret_store
            .matches("OPENLIFE_KEYCHAIN_SERVICE_OVERRIDE")
            .count(),
        1
    );
}

#[cfg(not(feature = "dev-extensions"))]
#[tokio::test]
async fn backend_remediation_phase0_runtime_projection_reports_release_extensions_disabled() {
    let info = crate::runtime_build_info::collect_runtime_build_info().await;
    assert!(!info.dev_extensions_enabled);
    assert!(!info.authenticated_dev_a2a_enabled);
    assert!(!info.unauthenticated_dev_a2a_enabled);
    assert!(!info.arbitrary_mcp_registration_enabled);
    assert_eq!(info.a2a_status, "disabled_by_build");
}

#[test]
fn backend_remediation_phase0_dev_entrypoints_are_explicit_and_match_dev_capabilities() {
    for path in [
        "scripts/dev.sh",
        "scripts/dev.ps1",
        "scripts/startup.sh",
        "scripts/startup.ps1",
    ] {
        let source = read_repo_file(path);
        assert!(
            source.contains("--features dev-extensions"),
            "{path} must compile the same dev-only command surface its capability advertises"
        );
        assert!(
            source.contains("dev-extensions"),
            "{path} must activate the dev-extensions Tauri capability"
        );
        assert!(
            source.contains("tauri.dev.conf.json"),
            "{path} must merge the reviewed dev configuration before its dynamic URL override"
        );
        assert!(
            source.contains("OPENLIFE_ALLOW_DEV_EXTENSIONS_WITH_CUSTOM_DATA_DIR"),
            "{path} must refuse accidental dev-extension access to a custom/release data directory"
        );
        assert!(
            source.contains("OPENLIFE_PROFILE") && source.contains("dev"),
            "{path} must force dev extensions onto the isolated dev profile"
        );
    }
    for path in ["scripts/startup.sh", "scripts/startup.ps1"] {
        let source = read_repo_file(path);
        assert!(source.contains("OPENLIFE_ENABLE_DEV_A2A"));
        assert!(source.contains("OPENLIFE_A2A_PAIRED_TOKEN"));
        assert!(!source.contains("OPENLIFE_ENABLE_UNAUTHENTICATED_DEV_A2A"));
    }

    let a2a_server = read_repo_file("src-tauri/src/a2a_server.rs");
    assert!(a2a_server.contains("require_authenticated_dev_a2a_opt_in"));
    assert!(a2a_server.contains("OPENLIFE_ENABLE_DEV_A2A"));
    assert!(a2a_server.contains("OPENLIFE_A2A_PAIRED_TOKEN"));
    assert!(!a2a_server.contains("OPENLIFE_ENABLE_UNAUTHENTICATED_DEV_A2A"));
    assert!(a2a_server.contains("OPENLIFE_ALLOW_DEV_EXTENSIONS_WITH_CUSTOM_DATA_DIR"));
    let a2a_bin = read_repo_file("tools/openlife-a2a-server/src/main.rs");
    assert!(a2a_bin.contains("require_authenticated_dev_a2a_opt_in"));
}

#[test]
fn backend_remediation_phase0_discovered_findings_addendum_is_additive_and_fail_closed() {
    let raw_inventory = read_repo_file("plans/openlife_backend_remediation_v4_inventory.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw_inventory.as_bytes())),
        FROZEN_FINDING_INVENTORY_SHA256,
        "the original 35 finding definitions are immutable; new discoveries belong in the additive registry"
    );
    let raw_scenarios = read_repo_file("plans/openlife_backend_remediation_v4_scenarios.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw_scenarios.as_bytes())),
        FROZEN_SCENARIO_SUITE_SHA256,
        "supplemental scenarios must not rewrite the frozen 40-scenario denominator"
    );

    let frozen: serde_json::Value =
        serde_json::from_str(&raw_inventory).expect("parse frozen finding inventory");
    let frozen_ids = frozen["findings"]
        .as_array()
        .expect("frozen findings")
        .iter()
        .map(|finding| finding["id"].as_str().expect("frozen finding id"))
        .collect::<BTreeSet<_>>();
    assert_eq!(frozen_ids.len(), 35);

    let registry: serde_json::Value = serde_json::from_str(&read_repo_file(
        "plans/openlife_backend_remediation_v4_discovered_findings.json",
    ))
    .expect("parse discovered finding registry");
    assert_eq!(
        registry["schema_version"],
        "openlife-backend-remediation-v4-discovered-findings-v1"
    );
    assert_eq!(registry["status"], "append-only");
    assert_eq!(
        registry["protected_baseline"]["frozen_inventory_sha256"],
        FROZEN_FINDING_INVENTORY_SHA256
    );
    assert_eq!(
        registry["protected_baseline"]["frozen_scenario_sha256"],
        FROZEN_SCENARIO_SUITE_SHA256
    );
    assert_eq!(
        registry["append_policy"]["merge_base_append_only_check_required_in_ci"],
        true
    );

    let findings = registry["findings"]
        .as_array()
        .expect("discovered findings array");
    assert!(
        findings.len() >= DISCOVERED_FINDING_IMMUTABLE_PREFIX_COUNT,
        "the trusted discovered-finding prefix must remain present"
    );
    let immutable_finding_prefix =
        serde_json::Value::Array(findings[..DISCOVERED_FINDING_IMMUTABLE_PREFIX_COUNT].to_vec());
    let immutable_finding_prefix = serde_json::to_vec(&canonical_json(&immutable_finding_prefix))
        .expect("serialize immutable discovered-finding prefix");
    assert_eq!(
        format!("{:x}", Sha256::digest(immutable_finding_prefix)),
        DISCOVERED_FINDING_IMMUTABLE_PREFIX_SHA256,
        "the first 53 discovered findings are a trusted immutable prefix; append findings or correction records instead of rewriting them"
    );
    let correction_target_ids = findings[..DISCOVERED_FINDING_IMMUTABLE_PREFIX_COUNT]
        .iter()
        .map(|finding| finding["id"].as_str().expect("trusted finding id"))
        .collect::<BTreeSet<_>>();

    let corrections = registry["definition_corrections"]
        .as_array()
        .expect("append-only definition corrections array");
    assert!(
        corrections.len() >= DISCOVERED_CORRECTION_IMMUTABLE_PREFIX_COUNT,
        "the trusted definition-correction prefix must remain present"
    );
    let immutable_correction_prefix = serde_json::Value::Array(
        corrections[..DISCOVERED_CORRECTION_IMMUTABLE_PREFIX_COUNT].to_vec(),
    );
    let immutable_correction_prefix =
        serde_json::to_vec(&canonical_json(&immutable_correction_prefix))
            .expect("serialize immutable definition-correction prefix");
    assert_eq!(
        format!("{:x}", Sha256::digest(immutable_correction_prefix)),
        DISCOVERED_CORRECTION_IMMUTABLE_PREFIX_SHA256,
        "the first two definition corrections are a trusted immutable prefix; append a superseding correction instead of rewriting them"
    );
    let mut definition_corrections = BTreeMap::new();
    for (index, correction) in corrections.iter().enumerate() {
        let correction_id = correction["id"].as_str().expect("correction id");
        assert_eq!(
            correction_id,
            format!("BR4-C{:03}", index + 1),
            "definition correction ids are append-only and monotonic"
        );
        for field in ["recorded_at", "field", "operation", "reason"] {
            assert!(
                correction[field]
                    .as_str()
                    .is_some_and(|value| !value.trim().is_empty()),
                "{correction_id} missing immutable field {field}"
            );
        }
        assert!(correction["evidence"]
            .as_array()
            .is_some_and(|evidence| !evidence.is_empty()));
        match correction["operation"].as_str().unwrap() {
            "remove_invalid_reference" => {
                assert_eq!(correction["field"], "related_frozen_findings");
                assert!(correction["effective_value"].is_null());
                let finding_id = correction["finding_id"]
                    .as_str()
                    .expect("corrected finding id");
                assert!(
                    correction_target_ids.contains(finding_id),
                    "{correction_id} can only correct a finding in the trusted immutable prefix"
                );
                let invalid_value = correction["invalid_value"]
                    .as_str()
                    .expect("invalid related finding reference");
                assert!(
                    !frozen_ids.contains(invalid_value),
                    "{correction_id} cannot remove a valid frozen finding reference"
                );
                let key = (
                    finding_id.to_string(),
                    "related_frozen_findings".to_string(),
                    invalid_value.to_string(),
                );
                assert!(
                    definition_corrections.insert(key, None).is_none(),
                    "duplicate definition correction for {finding_id}:{invalid_value}"
                );
            }
            "replace_invalid_derived_fingerprints" => {
                assert_eq!(correction["field"], "immutable_fingerprint");
                let replacements = correction["replacements"]
                    .as_array()
                    .filter(|replacements| !replacements.is_empty())
                    .expect("non-empty fingerprint replacements");
                for replacement in replacements {
                    let finding_id = replacement["finding_id"]
                        .as_str()
                        .expect("fingerprint correction finding id");
                    assert!(
                        correction_target_ids.contains(finding_id),
                        "{correction_id} can only correct a finding in the trusted immutable prefix"
                    );
                    let invalid_value = replacement["invalid_value"]
                        .as_str()
                        .expect("invalid stored fingerprint");
                    let effective_value = replacement["effective_value"]
                        .as_str()
                        .expect("effective canonical fingerprint");
                    assert!(is_lower_hex_sha256(invalid_value));
                    assert!(is_lower_hex_sha256(effective_value));
                    assert_ne!(invalid_value, effective_value);
                    let key = (
                        finding_id.to_string(),
                        "immutable_fingerprint".to_string(),
                        invalid_value.to_string(),
                    );
                    assert!(
                        definition_corrections
                            .insert(key, Some(effective_value.to_string()))
                            .is_none(),
                        "duplicate fingerprint correction for {finding_id}:{invalid_value}"
                    );
                }
            }
            operation => panic!("unsupported immutable correction operation: {operation}"),
        }
        assert_eq!(
            correction["immutable_fingerprint"].as_str().unwrap(),
            immutable_registry_record_fingerprint(correction),
            "{correction_id} immutable correction changed; append a superseding correction"
        );
    }

    let mut discovered_ids = BTreeSet::new();
    let mut used_definition_corrections = BTreeSet::new();
    for (index, finding) in findings.iter().enumerate() {
        let id = finding["id"].as_str().expect("discovered finding id");
        assert_eq!(
            id,
            format!("BR4-D{:03}", index + 1),
            "discovered finding ids are append-only, monotonic identities"
        );
        assert!(discovered_ids.insert(id));
        assert!(
            !frozen_ids.contains(id),
            "additive findings cannot reuse a frozen finding identity"
        );
        for field in [
            "discovered_at",
            "severity_at_discovery",
            "classification",
            "why_not_fully_subsumed",
            "title",
            "observed_symptom",
            "root_cause_hypothesis",
            "target_phase",
            "owner_module",
        ] {
            assert!(
                finding[field]
                    .as_str()
                    .is_some_and(|value| !value.trim().is_empty()),
                "{id} missing immutable field {field}"
            );
        }
        for field in [
            "related_frozen_findings",
            "affected_surfaces",
            "broken_invariants",
            "acceptance",
            "deletion_targets",
        ] {
            assert!(
                finding[field]
                    .as_array()
                    .is_some_and(|values| !values.is_empty()),
                "{id} missing non-empty immutable array {field}"
            );
        }
        for related in finding["related_frozen_findings"].as_array().unwrap() {
            let related = related.as_str().expect("related frozen finding id");
            if !frozen_ids.contains(related) {
                let correction = (
                    id.to_string(),
                    "related_frozen_findings".to_string(),
                    related.to_string(),
                );
                assert_eq!(
                    definition_corrections.get(&correction),
                    Some(&None),
                    "{id} references missing frozen finding {related} without an exact append-only correction"
                );
                used_definition_corrections.insert(correction);
            }
        }
        assert!(DISCOVERED_FINDING_CLASSIFICATIONS
            .contains(&finding["classification"].as_str().unwrap()));
        assert!(matches!(
            finding["severity_at_discovery"].as_str().unwrap(),
            "P0" | "P1" | "P2"
        ));
        let stored_fingerprint = finding["immutable_fingerprint"].as_str().unwrap();
        let canonical_fingerprint = immutable_registry_record_fingerprint(finding);
        if stored_fingerprint != canonical_fingerprint {
            let correction = (
                id.to_string(),
                "immutable_fingerprint".to_string(),
                stored_fingerprint.to_string(),
            );
            assert_eq!(
                definition_corrections.get(&correction),
                Some(&Some(canonical_fingerprint)),
                "{id} fingerprint differs from its immutable body without an exact append-only correction"
            );
            used_definition_corrections.insert(correction);
        }
    }
    assert_eq!(
        used_definition_corrections,
        definition_corrections.keys().cloned().collect(),
        "definition corrections must resolve an exact retained invalid value and its canonical effective value"
    );

    let traceability: serde_json::Value = serde_json::from_str(&read_repo_file(
        "plans/openlife_backend_remediation_v4_discovered_traceability.json",
    ))
    .expect("parse discovered traceability");
    let entries = traceability["entries"]
        .as_array()
        .expect("discovered traceability entries");
    let trace_ids = entries
        .iter()
        .map(|entry| entry["id"].as_str().expect("traceability id"))
        .collect::<BTreeSet<_>>();
    assert_eq!(trace_ids, discovered_ids);

    let mut has_open_finding = false;
    for entry in entries {
        let id = entry["id"].as_str().unwrap();
        assert!(matches!(
            entry["triage_state"].as_str().unwrap(),
            "candidate" | "source_reproduced" | "root_cause_confirmed" | "rejected"
        ));
        assert!(matches!(
            entry["implementation_state"].as_str().unwrap(),
            "not_started" | "in_progress" | "implemented" | "reverted"
        ));
        assert!(matches!(
            entry["verification_state"].as_str().unwrap(),
            "none" | "partial" | "complete"
        ));
        let closure = entry["closure_state"].as_str().unwrap();
        assert!(matches!(
            closure,
            "open" | "closure_candidate" | "independently_verified"
        ));
        has_open_finding |= closure != "independently_verified";
        let evidence = entry["evidence"].as_object().expect("evidence object");
        for evidence_kind in [
            "reproduction",
            "failing_test",
            "positive",
            "counterfactual",
            "deletion_or_absence",
            "capability_non_regression",
            "live_boundary",
        ] {
            assert!(
                evidence
                    .get(evidence_kind)
                    .and_then(serde_json::Value::as_array)
                    .is_some(),
                "{id} missing evidence class {evidence_kind}"
            );
        }
        if closure == "independently_verified" {
            assert_eq!(entry["implementation_state"], "implemented");
            assert_eq!(entry["verification_state"], "complete");
            assert!(entry["independent_review"]["reviewer"].is_string());
            assert_eq!(entry["independent_review"]["decision"], "accepted");
            for evidence_kind in ["positive", "counterfactual", "capability_non_regression"] {
                assert!(
                    !evidence[evidence_kind].as_array().unwrap().is_empty(),
                    "{id} cannot close without {evidence_kind} evidence"
                );
            }
            let all_evidence = evidence
                .values()
                .flat_map(|value| value.as_array().into_iter().flatten())
                .filter_map(serde_json::Value::as_str);
            assert!(
                all_evidence
                    .into_iter()
                    .all(|reference| !reference.starts_with("test_definition:")),
                "a test definition alone cannot close {id}"
            );
        }
    }
    if has_open_finding {
        assert_eq!(
            traceability["status"], "fail-closed",
            "the addendum must not claim completion while any discovered finding is open"
        );
    }
    assert!(traceability["supplemental_scenarios"].is_array());
}
