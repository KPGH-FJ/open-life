use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacySurfaceStatus {
    Active,
    CompatReadOnly,
    Deprecated,
    Retired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacySurfaceKind {
    RustTauriCommand,
    FrontendWrapper,
    FrontendExportedType,
    ProductionUiCopy,
    McpManifestResult,
    MockOrFixture,
    Test,
    DiagnosticsField,
    ConfigEnumValue,
    UserVisibleToolText,
    RuntimeReadinessCompat,
    InternalImplementationName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacySurfaceEnforcementMode {
    NoProductionLeakage,
    AllowlistOnly,
    CompatDeserializationOnly,
    InternalReadinessAllowlist,
}

#[derive(Debug, Clone)]
struct LegacySurfaceRegistryEntry {
    token: String,
    status: LegacySurfaceStatus,
    replacement: &'static str,
    allowed_locations: Vec<&'static str>,
    surface_kind: LegacySurfaceKind,
    enforcement_mode: LegacySurfaceEnforcementMode,
}

fn snake(parts: &[&str]) -> String {
    parts.join("_")
}

fn camel(parts: &[&str]) -> String {
    parts.concat()
}

fn beta_readiness_allowlist() -> Vec<&'static str> {
    // Decision: accept ReactBeta/MainChatAgentBetaV1 as internal readiness
    // compat naming debt only. Owner: Main Chat Agent v1 runtime-readiness
    // cleanup. Replacement: ReAct execution readiness / Main Chat Agent
    // readiness. Retirement condition: once compatibility wrappers,
    // deserialization/tests, and historical readiness report names are renamed
    // or removed, delete this allowlist and keep only registry/test fixtures.
    // This allowlist must not authorize production user-visible copy.
    vec![
        "frontend/src/tauri.ts",
        "frontend/src/types.ts",
        "frontend/src/tauri.test.ts",
        "frontend/src/test/ordinaryChatForbiddenCommands.ts",
        "openlife-core/src/agent/action_executor/tool_executor.rs",
        "openlife-core/src/agent/agent_loop.rs",
        "openlife-core/src/agent/lifemodel_backend_completion.rs",
        "openlife-core/src/agent/main_chat_agent_v1.rs",
        "openlife-core/src/agent/mod.rs",
        "openlife-core/src/agent/react_beta.rs",
        "openlife-core/src/agent/tests/mod.rs",
        "openlife-core/src/agent/tests/react_beta.rs",
        "src-tauri/src/commands/agent_runtime/mod.rs",
        "src-tauri/src/lib.rs",
        "src-tauri/src/main_chat_agent_beta_v1_default_experience.rs",
        "src-tauri/src/main_chat_agent_beta_v1_readiness.rs",
        "src-tauri/src/main_chat_agent_beta_v1_real_tasks.rs",
        "src-tauri/src/main_chat_agent_stage1_dogfood.rs",
        "src-tauri/src/main_chat_agent_stage2_readiness.rs",
        "src-tauri/src/main_chat_final_gate.rs",
        "src-tauri/src/main_chat_kernel.rs",
        "src-tauri/src/main_chat_proposal_support.rs",
        "src-tauri/src/main_chat_react_execution.rs",
        "src-tauri/src/main_chat_react_runtime.rs",
        "src-tauri/src/main_chat_react_tool_selection.rs",
        "src-tauri/src/main_chat_task_controls.rs",
    ]
}

fn internal_stub_allowlist() -> Vec<&'static str> {
    vec![
        "frontend/src/pages/SettingsPage.test.tsx",
        "openlife-core/src/agent/action_executor/declarative_stubs.rs",
        "openlife-core/src/agent/action_executor/mod.rs",
        "openlife-core/src/agent/action_executor/tool_executor.rs",
        "openlife-core/src/agent/main_chat_agent_v1.rs",
        "openlife-core/src/agent/tests/main_chat_agent_v1.rs",
        "openlife-core/src/agent/tests/react_beta.rs",
        "openlife-core/src/mcp.rs",
        "src-tauri/src/commands/agent_runtime/mod.rs",
        "src-tauri/src/legacy_write_convergence.rs",
    ]
}

fn registry() -> Vec<LegacySurfaceRegistryEntry> {
    vec![
        LegacySurfaceRegistryEntry {
            token: snake(&["get", "main", "chat", "runtime", "status"]),
            status: LegacySurfaceStatus::Active,
            replacement: "main_chat_runtime_status_v2",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::RustTauriCommand,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["main", "chat", "kernel"]),
            status: LegacySurfaceStatus::Active,
            replacement: "main_chat_kernel",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::ProductionUiCopy,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["get", "default", "chat", "runtime", "boundary", "status"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "get_main_chat_runtime_status",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::RustTauriCommand,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["default", "chat", "adapter"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "main_chat_kernel",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::RustTauriCommand,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: camel(&["get", "Default", "Chat", "Runtime", "Boundary", "Status"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "getMainChatRuntimeStatus",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::FrontendWrapper,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: camel(&["Default", "Chat", "Runtime", "Boundary", "Status"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "MainChatRuntimeStatus",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::FrontendExportedType,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: camel(&["Default", "Chat", "Adapter"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "main_chat_kernel",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::FrontendWrapper,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["legacy", "stream"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "status=failed retired fallback contract",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::MockOrFixture,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["send", "message", "with", "legacy", "generation"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "run_retired_buffered_fallback_delivery",
            allowed_locations: vec!["src-tauri/src/main_chat_runtime_module_tests.rs"],
            surface_kind: LegacySurfaceKind::RustTauriCommand,
            enforcement_mode: LegacySurfaceEnforcementMode::AllowlistOnly,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["beta", "ready"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "readiness_issues/runtime_route_evidence",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::DiagnosticsField,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["beta", "readiness", "issues"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "readiness_issues",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::DiagnosticsField,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["onboarding", "completed"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "first-run readiness projection",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::DiagnosticsField,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["legacy", "data", "dir"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "data_dir/active_data_dir",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::DiagnosticsField,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["has", "completed", "onboarding"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "diagnostics readiness",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::RustTauriCommand,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["mark", "onboarding", "completed"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "diagnostics readiness",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::RustTauriCommand,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: camel(&["Capability", "First", "Beta"]),
            status: LegacySurfaceStatus::Retired,
            replacement: "CapabilityFirst",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::ConfigEnumValue,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["capability", "first", "beta"]),
            status: LegacySurfaceStatus::CompatReadOnly,
            replacement: "capability_first",
            allowed_locations: vec!["openlife-core/src/config.rs"],
            surface_kind: LegacySurfaceKind::ConfigEnumValue,
            enforcement_mode: LegacySurfaceEnforcementMode::CompatDeserializationOnly,
        },
        LegacySurfaceRegistryEntry {
            token: ["Beta"].concat(),
            status: LegacySurfaceStatus::Deprecated,
            replacement: "Capability/readiness naming",
            allowed_locations: beta_readiness_allowlist(),
            surface_kind: LegacySurfaceKind::RuntimeReadinessCompat,
            enforcement_mode: LegacySurfaceEnforcementMode::InternalReadinessAllowlist,
        },
        LegacySurfaceRegistryEntry {
            token: camel(&["React", "Beta"]),
            status: LegacySurfaceStatus::Deprecated,
            replacement: "ReAct execution readiness",
            allowed_locations: beta_readiness_allowlist(),
            surface_kind: LegacySurfaceKind::FrontendWrapper,
            enforcement_mode: LegacySurfaceEnforcementMode::InternalReadinessAllowlist,
        },
        LegacySurfaceRegistryEntry {
            token: camel(&["Main", "Chat", "Agent", "Beta", "V1"]),
            status: LegacySurfaceStatus::Deprecated,
            replacement: "Main Chat Agent readiness",
            allowed_locations: beta_readiness_allowlist(),
            surface_kind: LegacySurfaceKind::RuntimeReadinessCompat,
            enforcement_mode: LegacySurfaceEnforcementMode::InternalReadinessAllowlist,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["react", "beta"]),
            status: LegacySurfaceStatus::Deprecated,
            replacement: "react execution readiness",
            allowed_locations: beta_readiness_allowlist(),
            surface_kind: LegacySurfaceKind::RustTauriCommand,
            enforcement_mode: LegacySurfaceEnforcementMode::InternalReadinessAllowlist,
        },
        LegacySurfaceRegistryEntry {
            token: ["M", "VP"].concat(),
            status: LegacySurfaceStatus::Retired,
            replacement: "capability release/product prototype",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::ProductionUiCopy,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: ["st", "ub"].concat(),
            status: LegacySurfaceStatus::CompatReadOnly,
            replacement: "manifest_only/provider_required/placeholder",
            allowed_locations: internal_stub_allowlist(),
            surface_kind: LegacySurfaceKind::McpManifestResult,
            enforcement_mode: LegacySurfaceEnforcementMode::AllowlistOnly,
        },
        LegacySurfaceRegistryEntry {
            token: snake(&["declarative", "stub"]),
            status: LegacySurfaceStatus::CompatReadOnly,
            replacement: "manifest_only/provider_required",
            allowed_locations: internal_stub_allowlist(),
            surface_kind: LegacySurfaceKind::InternalImplementationName,
            enforcement_mode: LegacySurfaceEnforcementMode::AllowlistOnly,
        },
        LegacySurfaceRegistryEntry {
            token: ["Beta", "MVP"].join(" "),
            status: LegacySurfaceStatus::Retired,
            replacement: "capability status",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::UserVisibleToolText,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: ["Beta", "stub"].join(" "),
            status: LegacySurfaceStatus::Retired,
            replacement: "manifest_only/provider_required",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::UserVisibleToolText,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: ["declarative-only", "stub"].join(" "),
            status: LegacySurfaceStatus::Retired,
            replacement: "manifest_only/provider_required",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::UserVisibleToolText,
            enforcement_mode: LegacySurfaceEnforcementMode::NoProductionLeakage,
        },
        LegacySurfaceRegistryEntry {
            token: "legacyCompatibility".into(),
            status: LegacySurfaceStatus::Active,
            replacement: "explicit legacyCompatibility fixture namespace",
            allowed_locations: vec![],
            surface_kind: LegacySurfaceKind::Test,
            enforcement_mode: LegacySurfaceEnforcementMode::AllowlistOnly,
        },
    ]
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn source_roots() -> Vec<PathBuf> {
    let root = workspace_root();
    vec![
        root.join("src-tauri/src"),
        root.join("openlife-core/src"),
        root.join("frontend/src"),
    ]
}

fn collect_source_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_source_files(&path, out);
            continue;
        }
        if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("rs" | "ts" | "tsx")
        ) {
            out.push(path);
        }
    }
}

fn relative_slash_path(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn token_is_allowed(entry: &LegacySurfaceRegistryEntry, relative_path: &str) -> bool {
    entry
        .allowed_locations
        .iter()
        .any(|suffix| relative_path.ends_with(suffix))
}

fn source_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in source_roots() {
        collect_source_files(&root, &mut files);
    }
    files
}

#[test]
fn legacy_surface_registry_covers_cross_layer_categories() {
    let entries = registry();
    for surface_kind in [
        LegacySurfaceKind::RustTauriCommand,
        LegacySurfaceKind::FrontendWrapper,
        LegacySurfaceKind::FrontendExportedType,
        LegacySurfaceKind::ProductionUiCopy,
        LegacySurfaceKind::McpManifestResult,
        LegacySurfaceKind::MockOrFixture,
        LegacySurfaceKind::Test,
        LegacySurfaceKind::DiagnosticsField,
        LegacySurfaceKind::ConfigEnumValue,
        LegacySurfaceKind::UserVisibleToolText,
        LegacySurfaceKind::RuntimeReadinessCompat,
        LegacySurfaceKind::InternalImplementationName,
    ] {
        assert!(
            entries
                .iter()
                .any(|entry| entry.surface_kind == surface_kind),
            "missing registry surface kind: {surface_kind:?}"
        );
    }
    for status in [
        LegacySurfaceStatus::Active,
        LegacySurfaceStatus::CompatReadOnly,
        LegacySurfaceStatus::Deprecated,
        LegacySurfaceStatus::Retired,
    ] {
        assert!(
            entries.iter().any(|entry| entry.status == status),
            "missing registry status: {status:?}"
        );
    }
}

#[test]
fn legacy_surface_registry_entries_have_governance_metadata() {
    for entry in registry() {
        assert!(!entry.token.is_empty(), "registry entry token is empty");
        assert!(
            !entry.replacement.trim().is_empty(),
            "registry entry `{}` missing replacement",
            entry.token
        );
        match entry.status {
            LegacySurfaceStatus::Deprecated | LegacySurfaceStatus::CompatReadOnly => {
                assert!(
                    !entry.allowed_locations.is_empty(),
                    "{:?} entry `{}` must not default-allow without explicit locations",
                    entry.status,
                    entry.token
                );
                assert_ne!(
                    entry.enforcement_mode,
                    LegacySurfaceEnforcementMode::NoProductionLeakage,
                    "{:?} entry `{}` needs a specific allowlist enforcement mode",
                    entry.status,
                    entry.token
                );
            }
            LegacySurfaceStatus::Retired | LegacySurfaceStatus::Active => {}
        }
    }
}

#[test]
fn legacy_surface_registry_covers_required_audit_terms() {
    let tokens = registry()
        .into_iter()
        .map(|entry| entry.token)
        .collect::<Vec<_>>();
    for required in [
        "Beta",
        "MVP",
        "stub",
        "legacy_stream",
        "default_chat_adapter",
        "get_default_chat_runtime_boundary_status",
        "DefaultChatRuntimeBoundaryStatus",
        "onboarding_completed",
        "beta_ready",
        "beta_readiness_issues",
        "legacy_data_dir",
        "has_completed_onboarding",
        "mark_onboarding_completed",
        "CapabilityFirstBeta",
        "capability_first_beta",
        "send_message_with_legacy_generation",
    ] {
        assert!(
            tokens.iter().any(|token| token == required),
            "registry missing audit term `{required}`"
        );
    }
}

#[test]
fn legacy_surface_retired_and_deprecated_tokens_do_not_leak_across_layers() {
    let entries = registry();
    let mut violations = Vec::new();
    for file in source_files() {
        let relative_path = relative_slash_path(&file);
        if relative_path.ends_with("src-tauri/src/legacy_surface_tests.rs") {
            continue;
        }
        let Ok(source) = fs::read_to_string(&file) else {
            continue;
        };
        for entry in entries.iter() {
            if entry.status == LegacySurfaceStatus::Active || !source.contains(&entry.token) {
                continue;
            }
            if token_is_allowed(entry, &relative_path) {
                continue;
            }
            violations.push(format!(
                "{relative_path} contains {:?} {:?} token `{}` outside {:?} allowlist; replacement={}",
                entry.status,
                entry.surface_kind,
                entry.token,
                entry.enforcement_mode,
                entry.replacement
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "legacy surface leakage:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_handlers_and_frontend_wrappers_do_not_expose_retired_default_chat_commands() {
    let root = workspace_root();
    let lib_rs = fs::read_to_string(root.join("src-tauri/src/lib.rs")).expect("read lib.rs");
    for retired in [
        "get_default_chat_runtime_boundary_status",
        "default_chat_adapter_activate",
        "default_chat_adapter_preview",
        "default_chat_adapter_narrow",
    ] {
        assert!(
            !lib_rs.contains(retired),
            "production Tauri handler still exposes retired command `{retired}`"
        );
    }

    let tauri_ts = fs::read_to_string(root.join("frontend/src/tauri.ts")).expect("read tauri.ts");
    for retired in [
        "getDefaultChatRuntimeBoundaryStatus",
        "DefaultChatRuntimeBoundaryStatus",
        "DefaultChatAdapter",
    ] {
        assert!(
            !tauri_ts.contains(retired),
            "frontend Tauri wrapper/type still exposes retired surface `{retired}`"
        );
    }
}
