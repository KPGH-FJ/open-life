use crate::commands::settings::run_d063_cleanup_orchestration_harness;
use crate::errors::AppError;
use chrono::{Duration, Utc};
use openlife_core::mcp_audit::{McpAuditStore, MCP_AUDIT_RETENTION_MAX_DAYS};
use rusqlite::params;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, Item, ItemFn, Token, UseTree, Visibility};

const D063_CLEANUP_CONTRACT_ADAPTER_VERSION: &str = "d063-cleanup-contract-adapter-v1";

fn parse_rust(source: &str, label: &str) -> syn::File {
    syn::parse_file(source).unwrap_or_else(|error| panic!("parse Rust source {label}: {error}"))
}

fn path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn functions(file: &syn::File) -> impl Iterator<Item = &ItemFn> {
    file.items.iter().filter_map(|item| match item {
        Item::Fn(function) => Some(function),
        _ => None,
    })
}

fn function<'a>(file: &'a syn::File, name: &str) -> &'a ItemFn {
    let matches = functions(file)
        .filter(|function| function.sig.ident == name)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one Rust function named {name}");
    matches[0]
}

fn require_flat_module(file: &syn::File, label: &str) {
    assert!(
        file.items
            .iter()
            .all(|item| !matches!(item, Item::Mod(module) if module.content.is_some() && !has_cfg_test(&module.attrs))),
        "{label} must not hide D063 command or domain authority in an inline module"
    );
}

#[derive(Default)]
struct GenerateHandlerVisitor {
    entries: Vec<syn::ExprPath>,
}

impl<'ast> Visit<'ast> for GenerateHandlerVisitor {
    fn visit_expr_macro(&mut self, expression: &'ast syn::ExprMacro) {
        if expression
            .mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "generate_handler")
        {
            let entries = Punctuated::<Expr, Token![,]>::parse_terminated
                .parse2(expression.mac.tokens.clone())
                .expect("parse tauri::generate_handler entries");
            self.entries
                .extend(entries.into_iter().map(|entry| match entry {
                    Expr::Path(path) => path,
                    _ => panic!("shipped generate_handler entries must be command paths"),
                }));
        }
        visit::visit_expr_macro(self, expression);
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        if item
            .mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "generate_handler")
        {
            panic!("shipped generate_handler must remain an expression macro");
        }
        visit::visit_item_macro(self, item);
    }
}

fn handler_identifiers(file: &syn::File) -> BTreeSet<String> {
    let mut visitor = GenerateHandlerVisitor::default();
    visitor.visit_file(file);
    visitor
        .entries
        .into_iter()
        .map(|entry| {
            assert_eq!(
                entry.path.segments.len(),
                1,
                "shipped generate_handler entries must be unaliased bare identifiers"
            );
            let name = entry.path.segments[0].ident.to_string();
            if name.contains("mcp_audit") {
                assert!(
                    entry.attrs.is_empty(),
                    "MCP audit handlers cannot have cfg-alternate registrations"
                );
            }
            name
        })
        .collect()
}

fn simple_audit_imports(file: &syn::File) -> BTreeMap<String, String> {
    let mut imports = BTreeMap::new();
    for item_use in file.items.iter().filter_map(|item| match item {
        Item::Use(item_use) => Some(item_use),
        _ => None,
    }) {
        let UseTree::Path(root) = &item_use.tree else {
            continue;
        };
        if root.ident != "commands" {
            continue;
        }
        let UseTree::Path(module) = root.tree.as_ref() else {
            panic!("commands imports must not use grouped modules, aliases, or globs");
        };
        if !matches!(module.ident.to_string().as_str(), "mcp" | "settings") {
            continue;
        }
        let names = match module.tree.as_ref() {
            UseTree::Name(name) => vec![name.ident.to_string()],
            UseTree::Group(group) => group
                .items
                .iter()
                .map(|tree| match tree {
                    UseTree::Name(name) => name.ident.to_string(),
                    _ => panic!(
                        "commands::{} imports must use direct names, not aliases, globs, or nested paths",
                        module.ident
                    ),
                })
                .collect(),
            _ => panic!(
                "commands::{} imports must use direct names, not aliases, globs, or nested paths",
                module.ident
            ),
        };
        for name in names.into_iter().filter(|name| name.contains("mcp_audit")) {
            assert!(
                item_use.attrs.is_empty(),
                "MCP audit imports cannot have cfg-alternate bindings"
            );
            assert!(
                imports
                    .insert(name.clone(), module.ident.to_string())
                    .is_none(),
                "duplicate MCP audit import {name}"
            );
        }
    }
    imports
}

#[derive(Default)]
struct RustCodeFacts {
    method_calls: Vec<String>,
    path_calls: Vec<String>,
    direct_delete_sql_calls: usize,
}

impl<'ast> Visit<'ast> for RustCodeFacts {
    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        let method = expression.method.to_string();
        self.method_calls.push(method.clone());
        if matches!(method.as_str(), "execute" | "execute_batch" | "prepare")
            && expression.args.iter().any(|argument| {
                matches!(argument, Expr::Lit(literal) if matches!(&literal.lit, syn::Lit::Str(sql) if sql.value().trim_start().to_ascii_uppercase().starts_with("DELETE FROM MCP_LOG")))
            })
        {
            self.direct_delete_sql_calls += 1;
        }
        visit::visit_expr_method_call(self, expression);
    }

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let Expr::Path(path) = expression.func.as_ref() {
            self.path_calls.push(path_name(&path.path));
        }
        visit::visit_expr_call(self, expression);
    }
}

fn code_facts(function: &ItemFn) -> RustCodeFacts {
    block_facts(&function.block)
}

fn block_facts(block: &syn::Block) -> RustCodeFacts {
    let mut facts = RustCodeFacts::default();
    facts.visit_block(block);
    facts
}

fn store_call_names(facts: &RustCodeFacts) -> BTreeSet<String> {
    let mut calls = facts.method_calls.iter().cloned().collect::<BTreeSet<_>>();
    calls.extend(facts.path_calls.iter().filter_map(|path| {
        let segments = path.split("::").collect::<Vec<_>>();
        (segments.len() >= 2 && matches!(segments[segments.len() - 2], "Self" | "McpAuditStore"))
            .then(|| segments[segments.len() - 1].to_string())
    }));
    calls
}

fn public_mcp_audit_delete_methods(file: &syn::File) -> BTreeSet<String> {
    require_flat_module(file, "McpAuditStore source");
    let methods = inherent_methods(file, "McpAuditStore");
    let sql_owners = methods
        .iter()
        .filter(|method| block_facts(&method.block).direct_delete_sql_calls > 0)
        .map(|method| method.sig.ident.to_string())
        .collect::<BTreeSet<_>>();
    methods
        .into_iter()
        .filter(|method| matches!(method.vis, Visibility::Public(_)))
        .filter(|method| {
            sql_owners.contains(&method.sig.ident.to_string())
                || !store_call_names(&block_facts(&method.block)).is_disjoint(&sql_owners)
        })
        .map(|method| method.sig.ident.to_string())
        .collect()
}

fn type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => Some(path_name(&path.path)),
        syn::Type::Reference(reference) => type_name(&reference.elem),
        _ => None,
    }
}

fn impl_self_name(item: &syn::ItemImpl) -> Option<String> {
    type_name(&item.self_ty)
}

fn inherent_methods<'a>(file: &'a syn::File, owner: &str) -> Vec<&'a syn::ImplItemFn> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item)
                if item.trait_.is_none() && impl_self_name(item).as_deref() == Some(owner) =>
            {
                Some(item)
            }
            _ => None,
        })
        .flat_map(|item| item.items.iter())
        .filter_map(|item| match item {
            syn::ImplItem::Fn(method) => Some(method),
            _ => None,
        })
        .collect()
}

fn has_cfg_test(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .parse_args::<syn::Ident>()
                .is_ok_and(|ident| ident == "test")
    })
}

struct NamedCallVisitor<'ast> {
    target: &'static str,
    calls: Vec<&'ast syn::ExprCall>,
}

impl<'ast> Visit<'ast> for NamedCallVisitor<'ast> {
    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if matches!(expression.func.as_ref(), Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == self.target))
        {
            self.calls.push(expression);
        }
        visit::visit_expr_call(self, expression);
    }
}

fn named_calls<'ast>(function: &'ast ItemFn, target: &'static str) -> Vec<&'ast syn::ExprCall> {
    let mut visitor = NamedCallVisitor {
        target,
        calls: Vec::new(),
    };
    visitor.visit_block(&function.block);
    visitor.calls
}

fn named_calls_in_expr<'ast>(
    expression: &'ast Expr,
    target: &'static str,
) -> Vec<&'ast syn::ExprCall> {
    let mut visitor = NamedCallVisitor {
        target,
        calls: Vec::new(),
    };
    visitor.visit_expr(expression);
    visitor.calls
}

fn module_name(relative: &str) -> String {
    relative
        .strip_prefix("src/")
        .and_then(|path| path.strip_suffix(".rs"))
        .expect("command Rust source path")
        .trim_end_matches("/mod")
        .replace('/', "::")
}

fn source_files(root: &Path) -> Vec<PathBuf> {
    fn collect(path: &Path, output: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(path).unwrap_or_else(|error| panic!("read {path:?}: {error}")) {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                collect(&path, output);
            } else {
                output.push(path);
            }
        }
    }

    let mut files = Vec::new();
    collect(root, &mut files);
    files.sort();
    files
}

fn command_sources() -> Vec<(String, String)> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    source_files(&manifest.join("src/commands"))
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter(|path| !path.to_string_lossy().contains("/tests/"))
        .map(|path| {
            let relative = path
                .strip_prefix(&manifest)
                .expect("command source under Tauri manifest")
                .to_string_lossy()
                .replace('\\', "/");
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {relative}: {error}"));
            (relative, source)
        })
        .collect()
}

fn insert_at(store: &McpAuditStore, path: &Path, tool_name: &str, created_at: &str) {
    store
        .insert_log(
            tool_name,
            &serde_json::json!({"fixture": tool_name}),
            "fixture-result",
            true,
            false,
        )
        .expect("insert D063 audit fixture");
    rusqlite::Connection::open(path)
        .expect("open D063 fixture database")
        .execute(
            "UPDATE mcp_log SET created_at = ?1 WHERE tool_name = ?2",
            params![created_at, tool_name],
        )
        .expect("set deterministic D063 fixture timestamp");
}

fn row_truth(store: &McpAuditStore) -> Vec<(String, String)> {
    let mut rows = store
        .list_logs(100)
        .expect("read D063 audit fixture")
        .into_iter()
        .map(|row| (row.tool_name, row.created_at))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

/// Frozen adapter across the current `cleanup(i64)` and the target
/// `cleanup(McpAuditRetentionDays)` signature. Type inference selects the
/// method argument: identity conversion today, `TryFrom<i64>` after D063 GREEN.
fn d063_cleanup_contract_adapter_v1(
    store: &McpAuditStore,
    retention_days: i64,
) -> anyhow::Result<usize> {
    store.cleanup(retention_days.try_into().map_err(|_| {
        anyhow::anyhow!(
            "{D063_CLEANUP_CONTRACT_ADAPTER_VERSION}: invalid retention {retention_days}"
        )
    })?)
}

fn assert_invalid_retention_is_non_mutating(retention_days: i64) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp-audit.sqlite");
    let store = McpAuditStore::new(&path);
    let now = Utc::now();
    let ancient = now
        .checked_sub_signed(Duration::days(MCP_AUDIT_RETENTION_MAX_DAYS + 2))
        .expect("D063 bounded ancient fixture");
    let future = now
        .checked_add_signed(Duration::days(2))
        .expect("D063 bounded future fixture");
    insert_at(&store, &path, "d063-ancient-row", &ancient.to_rfc3339());
    insert_at(&store, &path, "d063-future-row", &future.to_rfc3339());
    let before = row_truth(&store);

    let result = catch_unwind(AssertUnwindSafe(|| {
        d063_cleanup_contract_adapter_v1(&store, retention_days)
    }));
    let after = row_truth(&store);

    assert_eq!(
        after, before,
        "invalid retention {retention_days} mutated real SQLite rows"
    );
    assert!(
        result.is_ok(),
        "invalid retention {retention_days} must return a typed error, not panic"
    );
    assert!(
        result
            .expect("D063 invalid retention must not panic")
            .is_err(),
        "invalid retention {retention_days} must fail before SQL"
    );
}

async fn assert_valid_retention_boundary(retention_days: i64) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp-audit.sqlite");
    let store = McpAuditStore::new(&path);
    let now = Utc::now();
    let boundary = now
        .checked_sub_signed(Duration::days(retention_days))
        .expect("D063 bounded retention boundary");
    let before_boundary = boundary
        .checked_sub_signed(Duration::hours(2))
        .expect("D063 before-boundary fixture");
    let inside_boundary = boundary
        .checked_add_signed(Duration::hours(2))
        .expect("D063 inside-boundary fixture");
    let future = now
        .checked_add_signed(Duration::days(2))
        .expect("D063 future fixture");
    insert_at(
        &store,
        &path,
        "d063-before-boundary",
        &before_boundary.to_rfc3339(),
    );
    insert_at(
        &store,
        &path,
        "d063-inside-boundary",
        &inside_boundary.to_rfc3339(),
    );
    insert_at(&store, &path, "d063-future-row", &future.to_rfc3339());

    let cleaned = run_d063_cleanup_orchestration_harness(
        retention_days,
        Ok::<i64, AppError>,
        || Ok(()),
        |_| std::future::ready(Ok(())),
        |raw| {
            std::future::ready(
                d063_cleanup_contract_adapter_v1(&store, raw).map_err(AppError::from),
            )
        },
    )
    .await
    .expect("valid D063 retention cleanup");
    assert_eq!(cleaned, 1, "cutoff sign or day boundary is incorrect");
    let remaining = row_truth(&store)
        .into_iter()
        .map(|(tool_name, _)| tool_name)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        remaining,
        BTreeSet::from([
            "d063-future-row".to_string(),
            "d063-inside-boundary".to_string(),
        ]),
        "cleanup must preserve the just-inside and future rows"
    );
}

#[derive(Clone, Copy)]
enum OrchestrationRejection {
    Validation,
    Effects,
    Confirmation(&'static str),
}

#[derive(Default)]
struct OrchestrationCounts {
    effects: AtomicUsize,
    confirmations: AtomicUsize,
    mutations: AtomicUsize,
}

async fn assert_orchestration_rejection(rejection: OrchestrationRejection) {
    let counts = Arc::new(OrchestrationCounts::default());
    let result = run_d063_cleanup_orchestration_harness(
        30,
        move |days| match rejection {
            OrchestrationRejection::Validation => Err(AppError::internal("invalid retention")),
            _ => Ok(days),
        },
        {
            let counts = Arc::clone(&counts);
            move || {
                counts.effects.fetch_add(1, Ordering::SeqCst);
                match rejection {
                    OrchestrationRejection::Effects => {
                        Err(AppError::db_with_hint("degraded", "read_only_degraded"))
                    }
                    _ => Ok(()),
                }
            }
        },
        {
            let counts = Arc::clone(&counts);
            move |_| async move {
                counts.confirmations.fetch_add(1, Ordering::SeqCst);
                match rejection {
                    OrchestrationRejection::Confirmation(reason) => {
                        Err(AppError::permission(reason))
                    }
                    _ => Ok(()),
                }
            }
        },
        {
            let counts = Arc::clone(&counts);
            move |_| async move {
                counts.mutations.fetch_add(1, Ordering::SeqCst);
                Ok(0)
            }
        },
    )
    .await;
    assert!(result.is_err());
    let expected = match rejection {
        OrchestrationRejection::Validation => (0, 0),
        OrchestrationRejection::Effects => (1, 0),
        OrchestrationRejection::Confirmation(_) => (1, 1),
    };
    assert_eq!(counts.effects.load(Ordering::SeqCst), expected.0);
    assert_eq!(counts.confirmations.load(Ordering::SeqCst), expected.1);
    assert_eq!(counts.mutations.load(Ordering::SeqCst), 0);
}

#[test]
fn d063_shipped_handler_matches_the_audit_command_allowlist() {
    for (label, source) in [
        (
            "function rename",
            "use commands::mcp::{clear_mcp_audit_logs as allowed};",
        ),
        ("module alias", "use commands::mcp as audit;"),
        ("glob", "use commands::mcp::*;"),
        (
            "cfg alternate",
            "#[cfg(test)] use commands::mcp::clear_mcp_audit_logs;",
        ),
    ] {
        let probe = parse_rust(source, label);
        assert!(
            catch_unwind(AssertUnwindSafe(|| simple_audit_imports(&probe))).is_err(),
            "{label} must fail closed instead of being partially resolved"
        );
    }
    let handler_alias_probe = parse_rust(
        "fn build() { let _ = tauri::generate_handler![audit::clear_mcp_audit_logs]; }",
        "handler module alias",
    );
    assert!(catch_unwind(AssertUnwindSafe(|| handler_identifiers(
        &handler_alias_probe
    )))
    .is_err());

    let lib = parse_rust(include_str!("lib.rs"), "src-tauri/src/lib.rs");
    let imports = simple_audit_imports(&lib);
    let shipped_audit_commands = handler_identifiers(&lib)
        .into_iter()
        .filter(|name| name.contains("mcp_audit"))
        .collect::<BTreeSet<_>>();
    let shipped_audit_imports = shipped_audit_commands
        .into_iter()
        .map(|command| {
            let module = imports
                .get(&command)
                .unwrap_or_else(|| panic!("missing direct import for audit handler {command}"));
            (command, module.clone())
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        shipped_audit_imports,
        BTreeMap::from([
            ("cleanup_mcp_audit_logs".to_string(), "settings".to_string()),
            ("export_mcp_audit_logs".to_string(), "settings".to_string()),
            ("list_mcp_audit_logs".to_string(), "mcp".to_string()),
            ("rotate_mcp_audit_key".to_string(), "settings".to_string()),
        ]),
        "the shipped product may expose only the governed audit identifiers from their exact modules"
    );
}

#[test]
fn d063_has_one_product_to_domain_cleanup_mutation_call_graph() {
    let test_module_probe = parse_rust(
        "#[cfg(test)] mod tests { fn fixture() {} }",
        "D063 test-only inline module",
    );
    require_flat_module(&test_module_probe, "test-only module probe");
    let product_module_probe = parse_rust(
        "mod hidden_product_authority { fn mutate() {} }",
        "D063 product inline module",
    );
    assert!(catch_unwind(AssertUnwindSafe(|| {
        require_flat_module(&product_module_probe, "product module probe")
    }))
    .is_err());

    let renamed_method_probe = parse_rust(
        r#"
            struct McpAuditStore;
            impl McpAuditStore {
                fn delete_rows(&self) {
                    connection.execute("DELETE FROM mcp_log WHERE created_at < ?1", []);
                }
                pub fn innocuous_renamed_purge(&self) {
                    let fake = ".cleanup( and .clear_old_logs(";
                    Self::delete_rows(self);
                }
                pub fn read_only(&self) {}
            }
        "#,
        "D063 renamed domain mutation counterexample",
    );
    assert_eq!(
        public_mcp_audit_delete_methods(&renamed_method_probe),
        BTreeSet::from(["innocuous_renamed_purge".to_string()]),
        "public mutation entries must be derived from the SQL call graph, not known method names"
    );

    let renamed_probe = parse_rust(
        r#"
            #[tauri::command]
            pub(in crate::commands) async fn innocuous_renamed_endpoint() {
                let fake = ".cleanup( and .clear_old_logs(";
                McpAuditStore::clear_old_logs(&store, days);
            }
        "#,
        "D063 renamed mutation counterexample",
    );
    let renamed_facts = code_facts(function(&renamed_probe, "innocuous_renamed_endpoint"));
    assert_eq!(
        store_call_names(&renamed_facts),
        BTreeSet::from(["clear_old_logs".to_string()]),
        "UFCS mutation calls must remain visible while string literals stay ignored"
    );

    let core = parse_rust(
        include_str!("../../openlife-core/src/mcp_audit.rs"),
        "openlife-core/src/mcp_audit.rs",
    );
    let domain_mutation_methods = public_mcp_audit_delete_methods(&core);
    assert!(
        !domain_mutation_methods.is_empty(),
        "the dynamic cleanup contract requires a discoverable public domain mutation entry"
    );
    let mut mutation_expressions = Vec::new();
    for (relative, source) in command_sources() {
        let module = module_name(&relative);
        let file = parse_rust(&source, &relative);
        require_flat_module(&file, &relative);
        for function in functions(&file) {
            let facts = code_facts(function);
            assert_eq!(
                facts.direct_delete_sql_calls, 0,
                "Tauri command {module}::{} executes direct MCP audit deletion SQL",
                function.sig.ident
            );
            for method in store_call_names(&facts)
                .into_iter()
                .filter(|method| domain_mutation_methods.contains(method.as_str()))
            {
                mutation_expressions.push(format!("{module}::{}::{method}", function.sig.ident));
            }
        }
    }
    mutation_expressions.sort();
    assert_eq!(
        mutation_expressions,
        vec!["commands::settings::cleanup_mcp_audit_logs::cleanup"],
        "all command functions, regardless of name or visibility, must contain one audit cleanup mutation call"
    );
}

#[test]
fn d063_frontend_product_surface_has_no_page_local_cleanup_authority() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let frontend = manifest
        .parent()
        .expect("repository root")
        .join("frontend/src");
    let mut violations = Vec::new();
    for path in source_files(&frontend) {
        let rendered = path.to_string_lossy();
        if rendered.contains("/test/")
            || rendered.contains(".test.")
            || !matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("ts" | "tsx")
            )
        {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read frontend product source");
        for marker in ["clearMcpAuditLogs", "\"clear_mcp_audit_logs\""] {
            if source.contains(marker) {
                violations.push(format!("{}:{marker}", path.display()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "page-local MCP audit cleanup authority remains: {violations:?}"
    );
    let page = include_str!("../../frontend/src/pages/McpPage.tsx");
    assert!(page.contains("to=\"/settings\""));
    assert!(page.contains("在隐私设置中管理审计保留"));
    assert!(!page.contains("确认清理 MCP 审计日志"));
}

#[test]
fn d063_core_has_one_typed_cleanup_mutation_entry() {
    assert_eq!(MCP_AUDIT_RETENTION_MAX_DAYS, 3_650);
    let file = parse_rust(
        include_str!("../../openlife-core/src/mcp_audit.rs"),
        "openlife-core/src/mcp_audit.rs",
    );
    let store_methods = inherent_methods(&file, "McpAuditStore");
    let public_cleanup_methods = store_methods
        .iter()
        .filter(|method| {
            matches!(method.vis, Visibility::Public(_)) && method.sig.ident == "cleanup"
        })
        .collect::<Vec<_>>();
    assert_eq!(public_cleanup_methods.len(), 1);
    assert_eq!(
        public_mcp_audit_delete_methods(&file),
        BTreeSet::from(["cleanup".to_string()]),
        "cleanup must be the only public method that can reach MCP audit deletion SQL"
    );
    let typed_argument = public_cleanup_methods[0]
        .sig
        .inputs
        .iter()
        .find_map(|argument| match argument {
            syn::FnArg::Typed(argument) => Some(type_name(&argument.ty)),
            syn::FnArg::Receiver(_) => None,
        })
        .flatten();
    assert_eq!(typed_argument.as_deref(), Some("McpAuditRetentionDays"));
    assert_eq!(
        store_methods
            .iter()
            .map(|method| block_facts(&method.block).direct_delete_sql_calls)
            .sum::<usize>(),
        1
    );
}

#[test]
fn d063_shipped_command_binds_the_typed_retention_domain() {
    let file = parse_rust(include_str!("commands/settings.rs"), "commands/settings.rs");
    let command = function(&file, "cleanup_mcp_audit_logs");
    let calls = named_calls(command, "orchestrate_mcp_audit_cleanup");
    assert_eq!(calls.len(), 1);
    let validator = calls[0].args.iter().nth(1).expect("retention validator");
    let Expr::Closure(validator) = validator else {
        panic!("retention validator must be an inline closure");
    };
    let parameter = validator
        .inputs
        .first()
        .and_then(|pattern| match pattern {
            syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
            _ => None,
        })
        .expect("validator raw retention parameter");
    let conversions = named_calls_in_expr(&validator.body, "try_from")
        .into_iter()
        .filter(|conversion| {
            matches!(conversion.func.as_ref(), Expr::Path(path) if path_name(&path.path) == "McpAuditRetentionDays::try_from")
                && conversion.args.len() == 1
                && matches!(&conversion.args[0], Expr::Path(path) if path.path.is_ident(&parameter))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        conversions.len(),
        1,
        "the validator must call the core typed conversion exactly once for its raw parameter; outer AppError mapping is allowed"
    );
}

#[test]
fn d063_release_orchestrator_is_private_and_has_one_product_caller() {
    let syntax_probe = parse_rust(
        r#"
            pub fn public_probe() {}
            pub(super) fn parent_probe() {}
            pub(in crate::commands) fn path_probe() {}
            fn private_probe() {
                let fake = "orchestrate_mcp_audit_cleanup(value)";
            }
        "#,
        "D063 private-seam counterexample",
    );
    assert!(matches!(
        function(&syntax_probe, "private_probe").vis,
        Visibility::Inherited
    ));
    for name in ["public_probe", "parent_probe", "path_probe"] {
        assert!(
            !matches!(function(&syntax_probe, name).vis, Visibility::Inherited),
            "{name} must be recognized as release-visible"
        );
    }
    assert!(named_calls(
        function(&syntax_probe, "private_probe"),
        "orchestrate_mcp_audit_cleanup"
    )
    .is_empty());

    let file = parse_rust(include_str!("commands/settings.rs"), "commands/settings.rs");
    let orchestrator = function(&file, "orchestrate_mcp_audit_cleanup");
    assert!(
        matches!(orchestrator.vis, Visibility::Inherited),
        "the injected orchestration seam must be private in release code"
    );
    let harness = function(&file, "run_d063_cleanup_orchestration_harness");
    assert!(has_cfg_test(&harness.attrs));
    assert!(matches!(harness.vis, Visibility::Restricted(_)));
    let callers = functions(&file)
        .filter_map(|candidate| {
            let count = named_calls(candidate, "orchestrate_mcp_audit_cleanup").len();
            (count > 0).then(|| (candidate.sig.ident.to_string(), count))
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        callers,
        BTreeMap::from([
            ("cleanup_mcp_audit_logs".to_string(), 1),
            ("run_d063_cleanup_orchestration_harness".to_string(), 1),
        ]),
        "only the shipped command and cfg(test) forwarding harness may call the private seam"
    );
}

#[tokio::test]
async fn d063_orchestration_validation_failure_stops_all_effects() {
    assert_orchestration_rejection(OrchestrationRejection::Validation).await;
}

#[tokio::test]
async fn d063_orchestration_degraded_effects_stop_confirmation_and_mutation() {
    assert_orchestration_rejection(OrchestrationRejection::Effects).await;
}

#[tokio::test]
async fn d063_orchestration_missing_or_invalid_confirmation_is_non_mutating() {
    for denial in ["missing native authority", "invalid native authority"] {
        assert_orchestration_rejection(OrchestrationRejection::Confirmation(denial)).await;
    }
}

#[test]
fn d063_negative_retention_is_rejected_without_mutation() {
    assert_invalid_retention_is_non_mutating(-1);
}

#[test]
fn d063_zero_retention_is_rejected_without_mutation() {
    assert_invalid_retention_is_non_mutating(0);
}

#[test]
fn d063_above_maximum_retention_is_rejected_without_mutation() {
    assert_invalid_retention_is_non_mutating(MCP_AUDIT_RETENTION_MAX_DAYS + 1);
}

#[test]
fn d063_overflow_retention_is_rejected_without_panic_or_mutation() {
    assert_invalid_retention_is_non_mutating(i64::MAX);
}

#[tokio::test]
async fn d063_one_day_retention_uses_subtracted_day_boundary() {
    assert_valid_retention_boundary(1).await;
}

#[tokio::test]
async fn d063_maximum_retention_is_valid_and_uses_subtracted_day_boundary() {
    assert_valid_retention_boundary(MCP_AUDIT_RETENTION_MAX_DAYS).await;
}
