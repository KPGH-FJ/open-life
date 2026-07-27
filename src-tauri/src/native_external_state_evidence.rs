#[cfg(test)]
mod tests {
    use crate::bootstrap::bootstrap_with_secret_store_for_test;
    use crate::secret_store::{
        SecretStore, ACTION_QUEUE_AUTHORITY_KEY_REF, AGENT_RUN_RECEIPT_KEY_REF,
        MAIN_CHAT_EVENT_INTEGRITY_KEY_REF, MCP_AUDIT_KEY_REF_PREFIX, PROVIDER_KEY_REF,
        SEARCH_KEY_REF, TASK_STORE_AUTHORITY_KEY_REF,
    };
    use serde::Serialize;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs::{self, File};
    use std::io;
    use std::net::TcpListener;
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use std::os::fd::AsRawFd;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::Mutex;

    const TEST_NAME: &str =
        "native_external_state_evidence::tests::w0_s3_native_external_state_isolation";
    const EVIDENCE_PREFIX: &str = "W0_S3_NATIVE_EXTERNAL_STATE_EVIDENCE=";
    const ALLOWED_DATABASES: &[&str] = &[
        "agent_runs.db",
        "evidence.db",
        "feedback.db",
        "heuristics.db",
        "life_events.db",
        "main_chat_action_queue.db",
        "main_chat_agent_events.db",
        "main_chat_agent_sessions.db",
        "mcp_audit.db",
        "memory.db",
        "memory_lifecycle.db",
        "patches.db",
        "plan_execute_sessions.db",
        "proposals.db",
        "resources.db",
        "rollout_metrics.db",
        "state.db",
        "tasks.db",
        "tool_permissions.db",
        "vectors.db",
    ];
    const ALLOWED_DATA_FILES: &[&str] = &[
        "agent_runs.db.openlife-owner.lock",
        "builder_sessions.json",
        "config.yaml",
        "life-model/current/hs_asset_authority.db",
        "life-model/current/life_model.yaml",
        "life-model/current/life_model_mutation_journal.db",
        "life-model/current/life_model_mutation_journal.db-shm",
        "life-model/current/life_model_mutation_journal.db-wal",
        "main_chat_action_queue.db.openlife-owner.lock",
        "mcp_audit_keys.json",
        "privacy_policy.yaml",
        "scheduled_tasks.json",
        "tasks.db.openlife-owner.lock",
    ];
    const ALLOWED_DATA_DIRECTORIES: &[&str] = &[
        "data",
        "data/life-model",
        "data/life-model/current",
        "data/life-model/versions",
        "data/plugins",
    ];
    const ALLOWED_FIXED_SECRET_REFS: &[&str] = &[
        PROVIDER_KEY_REF,
        SEARCH_KEY_REF,
        MAIN_CHAT_EVENT_INTEGRITY_KEY_REF,
        ACTION_QUEUE_AUTHORITY_KEY_REF,
        TASK_STORE_AUTHORITY_KEY_REF,
        AGENT_RUN_RECEIPT_KEY_REF,
    ];

    #[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
    #[serde(rename_all = "snake_case")]
    struct FileEntry {
        path: String,
        kind: &'static str,
        len: u64,
    }

    #[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
    struct NativeEndpoint {
        fd: String,
        endpoint_type: String,
        name: String,
        lock: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    enum SecretOperationKind {
        Get,
        Set,
        Delete,
    }

    #[derive(Debug, Clone, Serialize, PartialEq, Eq)]
    struct SecretOperation {
        kind: SecretOperationKind,
        secret_ref: String,
    }

    #[derive(Debug, Clone, Serialize, PartialEq, Eq)]
    struct LockProbe {
        path: String,
        state: &'static str,
    }

    #[derive(Default)]
    struct RecordingSecretStore {
        values: Mutex<BTreeMap<String, String>>,
        operations: Mutex<Vec<SecretOperation>>,
    }

    impl RecordingSecretStore {
        fn operations(&self) -> Vec<SecretOperation> {
            self.operations
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }

        fn record(&self, kind: SecretOperationKind, secret_ref: &str) {
            self.operations
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(SecretOperation {
                    kind,
                    secret_ref: secret_ref.to_string(),
                });
        }
    }

    impl SecretStore for RecordingSecretStore {
        fn get(&self, secret_ref: &str) -> anyhow::Result<Option<String>> {
            self.record(SecretOperationKind::Get, secret_ref);
            Ok(self
                .values
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .get(secret_ref)
                .cloned())
        }

        fn set(&self, secret_ref: &str, value: &str) -> anyhow::Result<()> {
            self.record(SecretOperationKind::Set, secret_ref);
            self.values
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(secret_ref.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, secret_ref: &str) -> anyhow::Result<()> {
            self.record(SecretOperationKind::Delete, secret_ref);
            self.values
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(secret_ref);
            Ok(())
        }
    }

    enum CounterexampleFixture {
        None,
        OpenFile(File),
        LoopbackSocket(TcpListener),
    }

    fn collect_tree(root: &Path) -> io::Result<BTreeSet<FileEntry>> {
        fn visit(root: &Path, current: &Path, entries: &mut BTreeSet<FileEntry>) -> io::Result<()> {
            if !current.exists() {
                return Ok(());
            }
            let mut children = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
            children.sort_by_key(|entry| entry.file_name());
            for child in children {
                let path = child.path();
                let metadata = fs::symlink_metadata(&path)?;
                let relative = path
                    .strip_prefix(root)
                    .expect("inventory path remains below sandbox")
                    .to_string_lossy()
                    .replace('\\', "/");
                let kind = if metadata.file_type().is_dir() {
                    "directory"
                } else if metadata.file_type().is_file() {
                    "file"
                } else if metadata.file_type().is_symlink() {
                    "symlink"
                } else {
                    "other"
                };
                entries.insert(FileEntry {
                    path: relative,
                    kind,
                    len: metadata.len(),
                });
                if metadata.file_type().is_dir() {
                    visit(root, &path, entries)?;
                }
            }
            Ok(())
        }

        let mut entries = BTreeSet::new();
        visit(root, root, &mut entries)?;
        Ok(entries)
    }

    fn collect_lsof(
        pid: u32,
        inspector_output_path: &Path,
    ) -> Result<BTreeSet<NativeEndpoint>, String> {
        let stdout = File::create(inspector_output_path)
            .map_err(|error| format!("create lsof inventory output: {error}"))?;
        let stderr = stdout
            .try_clone()
            .map_err(|error| format!("clone lsof inventory output: {error}"))?;
        let status = Command::new("/usr/sbin/lsof")
            .args(["-nP", "-a", "-p", &pid.to_string(), "-F", "ftnl"])
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .status()
            .map_err(|error| format!("start /usr/sbin/lsof: {error}"))?;
        if !status.success() {
            return Err(format!(
                "lsof exited {}: {}",
                status,
                fs::read_to_string(inspector_output_path)
                    .unwrap_or_default()
                    .trim()
            ));
        }
        let stdout = fs::read_to_string(inspector_output_path)
            .map_err(|error| format!("read lsof inventory: {error}"))?;
        let mut endpoints = BTreeSet::new();
        let mut current_fd: Option<String> = None;
        let mut current_type = String::new();
        let mut current_name = String::new();
        let mut current_lock: Option<String> = None;

        let flush = |endpoints: &mut BTreeSet<NativeEndpoint>,
                     fd: &mut Option<String>,
                     endpoint_type: &mut String,
                     name: &mut String,
                     lock: &mut Option<String>| {
            if let Some(observed_fd) = fd.take() {
                if observed_fd
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_digit())
                {
                    endpoints.insert(NativeEndpoint {
                        fd: observed_fd,
                        endpoint_type: std::mem::take(endpoint_type),
                        name: std::mem::take(name),
                        lock: lock.take(),
                    });
                }
            }
            endpoint_type.clear();
            name.clear();
            *lock = None;
        };

        for line in stdout.lines() {
            if line.is_empty() {
                continue;
            }
            let (field, value) = line.split_at(1);
            match field {
                "f" => {
                    flush(
                        &mut endpoints,
                        &mut current_fd,
                        &mut current_type,
                        &mut current_name,
                        &mut current_lock,
                    );
                    current_fd = Some(value.to_string());
                }
                "t" => current_type = value.to_string(),
                "n" => current_name = value.to_string(),
                "l" if !value.trim().is_empty() => current_lock = Some(value.trim().to_string()),
                _ => {}
            }
        }
        flush(
            &mut endpoints,
            &mut current_fd,
            &mut current_type,
            &mut current_name,
            &mut current_lock,
        );
        Ok(endpoints)
    }

    fn normalize_endpoint(
        endpoint: &NativeEndpoint,
        sandbox_root: &Path,
        data_dir: &Path,
    ) -> NativeEndpoint {
        let sandbox = sandbox_root.to_string_lossy();
        let data = data_dir.to_string_lossy();
        let name = if endpoint.name.starts_with(data.as_ref()) {
            endpoint.name.replacen(data.as_ref(), "$DATA", 1)
        } else if endpoint.name.starts_with(sandbox.as_ref()) {
            endpoint.name.replacen(sandbox.as_ref(), "$SANDBOX", 1)
        } else {
            endpoint.name.clone()
        };
        NativeEndpoint {
            fd: endpoint.fd.clone(),
            endpoint_type: endpoint.endpoint_type.clone(),
            name,
            lock: endpoint.lock.clone(),
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn probe_declared_owner_locks(data_dir: &Path) -> Vec<LockProbe> {
        [
            "agent_runs.db.openlife-owner.lock",
            "main_chat_action_queue.db.openlife-owner.lock",
            "tasks.db.openlife-owner.lock",
        ]
        .into_iter()
        .map(|relative| {
            let path = data_dir.join(relative);
            let file = File::open(&path).expect("open declared owner lock for native probe");
            // SAFETY: flock receives a live descriptor owned by `file`; this
            // probe never shares or stores the descriptor beyond this call.
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            let state = if result == 0 {
                // The bootstrap was expected to hold this exact owner lock.
                // Release the probe immediately before reporting the absence.
                // SAFETY: the descriptor is still live and exclusively held
                // by this probe in this branch.
                unsafe {
                    libc::flock(file.as_raw_fd(), libc::LOCK_UN);
                }
                "not_held"
            } else {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::WouldBlock {
                    "held_by_bootstrap"
                } else {
                    "unknown"
                }
            };
            LockProbe {
                path: format!("$DATA/{relative}"),
                state,
            }
        })
        .collect()
    }

    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    fn probe_declared_owner_locks(_data_dir: &Path) -> Vec<LockProbe> {
        Vec::new()
    }

    fn allowed_database_name(name: &str) -> bool {
        ALLOWED_DATABASES.contains(&name)
            || ["-journal", "-shm", "-wal"].iter().any(|suffix| {
                name.strip_suffix(suffix)
                    .is_some_and(|base| ALLOWED_DATABASES.contains(&base))
            })
    }

    fn allowed_data_file_name(name: &str) -> bool {
        allowed_database_name(name) || ALLOWED_DATA_FILES.contains(&name)
    }

    fn allowed_data_relative_path(relative: &str, kind: &str) -> bool {
        if kind == "directory" {
            return ALLOWED_DATA_DIRECTORIES.contains(&relative);
        }
        let Some(name) = relative.strip_prefix("data/") else {
            return false;
        };
        allowed_data_file_name(name)
    }

    fn allowed_endpoint(endpoint: &NativeEndpoint) -> bool {
        if endpoint.name == "$SANDBOX/lsof-inventory.txt" {
            return true;
        }
        let Some(relative) = endpoint.name.strip_prefix("$DATA/") else {
            return false;
        };
        allowed_data_file_name(relative)
    }

    fn allowed_secret_ref(secret_ref: &str) -> bool {
        ALLOWED_FIXED_SECRET_REFS.contains(&secret_ref)
            || secret_ref
                .strip_prefix(MCP_AUDIT_KEY_REF_PREFIX)
                .is_some_and(|epoch| {
                    !epoch.is_empty() && epoch.chars().all(|character| character.is_ascii_digit())
                })
    }

    fn set_difference<T: Clone + Ord>(current: &BTreeSet<T>, before: &BTreeSet<T>) -> Vec<T> {
        current.difference(before).cloned().collect()
    }

    fn run_child_if_needed() -> bool {
        if std::env::var("OPENLIFE_W0_S3_EVIDENCE_CHILD").as_deref() == Ok("1") {
            return false;
        }
        let sandbox = tempfile::tempdir().expect("create W0-S3 orchestration sandbox");
        let data_dir = sandbox.path().join("data");
        let status =
            Command::new(std::env::current_exe().expect("resolve current test executable"))
                .args(["--exact", TEST_NAME, "--nocapture"])
                .env("OPENLIFE_W0_S3_EVIDENCE_CHILD", "1")
                .env("OPENLIFE_W0_S3_SANDBOX_ROOT", sandbox.path())
                .env("OPENLIFE_DATA_DIR", &data_dir)
                .env("OPENLIFE_W0_S3_SCENARIO", "positive")
                .status()
                .expect("start fresh W0-S3 exact-test process");
        assert!(
            status.success(),
            "fresh W0-S3 exact-test process failed with {status}"
        );
        true
    }

    #[test]
    fn w0_s3_native_external_state_isolation() {
        if run_child_if_needed() {
            return;
        }

        let scenario =
            std::env::var("OPENLIFE_W0_S3_SCENARIO").unwrap_or_else(|_| "positive".into());
        let sandbox_root = PathBuf::from(
            std::env::var("OPENLIFE_W0_S3_SANDBOX_ROOT")
                .expect("fresh process must inherit OPENLIFE_W0_S3_SANDBOX_ROOT"),
        );
        let data_dir = PathBuf::from(
            std::env::var("OPENLIFE_DATA_DIR")
                .expect("fresh process must inherit OPENLIFE_DATA_DIR"),
        );
        assert_eq!(
            data_dir.parent(),
            Some(sandbox_root.as_path()),
            "isolated data directory must be an explicit child of the harness sandbox"
        );
        assert_eq!(
            data_dir.file_name().and_then(|name| name.to_str()),
            Some("data"),
            "isolated data directory must use the declared data endpoint"
        );

        if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            println!(
                "{EVIDENCE_PREFIX}{}",
                serde_json::json!({
                    "schema_version": "openlife.w0_s3.native_external_state_evidence.v1",
                    "status": "UNKNOWN",
                    "scenario": scenario,
                    "unsupported": ["endpoint inventory requires macOS arm64"],
                    "unknown": [
                        "transient sockets between snapshots",
                        "arbitrary filesystem locations outside the isolated sandbox",
                        "real Tauri window, setup, and reconciliation",
                        "Settings and direct Keyring commands",
                        "real OS Keychain contents"
                    ]
                })
            );
            return;
        }

        fs::create_dir_all(&sandbox_root).expect("create isolated W0-S3 sandbox");
        fs::create_dir_all(&data_dir).expect("create isolated W0-S3 data endpoint");
        let canonical_sandbox =
            fs::canonicalize(&sandbox_root).expect("canonicalize isolated W0-S3 sandbox");
        let canonical_data =
            fs::canonicalize(&data_dir).expect("canonicalize isolated W0-S3 data endpoint");
        let inspector_output_path = sandbox_root.join("lsof-inventory.txt");
        fs::write(&inspector_output_path, b"").expect("prepare declared lsof inventory output");
        if scenario == "undeclared_fd" {
            fs::write(
                sandbox_root.join("declared-fd-fixture.txt"),
                b"W0-S3 descriptor fixture",
            )
            .expect("prepare descriptor fixture before baseline inventory");
        }
        let filesystem_before = collect_tree(&sandbox_root).expect("inventory sandbox before");
        let endpoints_before = collect_lsof(std::process::id(), &inspector_output_path)
            .expect("inventory native endpoints before bootstrap");

        let fixture = match scenario.as_str() {
            "positive" => CounterexampleFixture::None,
            "undeclared_file" => {
                fs::write(
                    sandbox_root.join("undeclared-file.txt"),
                    b"W0-S3 undeclared filesystem counterexample",
                )
                .expect("create undeclared filesystem counterexample");
                CounterexampleFixture::None
            }
            "undeclared_fd" => CounterexampleFixture::OpenFile(
                File::open(sandbox_root.join("declared-fd-fixture.txt"))
                    .expect("open undeclared descriptor counterexample"),
            ),
            "undeclared_socket" => CounterexampleFixture::LoopbackSocket(
                TcpListener::bind("127.0.0.1:0")
                    .expect("bind undeclared loopback socket counterexample"),
            ),
            other => panic!("unsupported W0-S3 counterexample scenario: {other}"),
        };

        let secrets = RecordingSecretStore::default();
        let bootstrap = bootstrap_with_secret_store_for_test(data_dir.clone(), &secrets);
        match &fixture {
            CounterexampleFixture::None => {}
            CounterexampleFixture::OpenFile(file) => {
                let _ = file.metadata().expect("descriptor fixture remains open");
            }
            CounterexampleFixture::LoopbackSocket(socket) => {
                let _ = socket.local_addr().expect("loopback fixture remains bound");
            }
        }
        let filesystem_during = collect_tree(&sandbox_root).expect("inventory sandbox during");
        let lock_probes = probe_declared_owner_locks(&data_dir);
        let endpoints_during = collect_lsof(std::process::id(), &inspector_output_path)
            .expect("inventory native endpoints during bootstrap");
        let secret_operations = secrets.operations();
        drop(bootstrap);
        drop(fixture);
        let filesystem_after = collect_tree(&sandbox_root).expect("inventory sandbox after");
        let endpoints_after = collect_lsof(std::process::id(), &inspector_output_path)
            .expect("inventory native endpoints after bootstrap");

        let filesystem_added_during = set_difference(&filesystem_during, &filesystem_before);
        let filesystem_added_after = set_difference(&filesystem_after, &filesystem_before);
        let endpoints_added_during = set_difference(&endpoints_during, &endpoints_before)
            .into_iter()
            .map(|endpoint| normalize_endpoint(&endpoint, &canonical_sandbox, &canonical_data))
            .collect::<Vec<_>>();
        let endpoints_added_after = set_difference(&endpoints_after, &endpoints_before)
            .into_iter()
            .map(|endpoint| normalize_endpoint(&endpoint, &canonical_sandbox, &canonical_data))
            .collect::<Vec<_>>();
        let sockets_added_during = endpoints_added_during
            .iter()
            .filter(|endpoint| matches!(endpoint.endpoint_type.as_str(), "IPv4" | "IPv6" | "unix"))
            .cloned()
            .collect::<Vec<_>>();
        let locks_added_during = endpoints_added_during
            .iter()
            .filter(|endpoint| {
                endpoint.lock.is_some() || endpoint.name.ends_with(".openlife-owner.lock")
            })
            .cloned()
            .collect::<Vec<_>>();

        let mut violations = Vec::new();
        for entry in filesystem_added_during
            .iter()
            .chain(filesystem_added_after.iter())
        {
            if matches!(
                entry.path.as_str(),
                "declared-fd-fixture.txt" | "lsof-inventory.txt"
            ) {
                continue;
            }
            if !allowed_data_relative_path(&entry.path, entry.kind) {
                violations.push(format!("filesystem:{}:{}", entry.kind, entry.path));
            }
        }
        for endpoint in &endpoints_added_during {
            if !allowed_endpoint(endpoint) {
                violations.push(format!(
                    "endpoint:{}:{}:{}",
                    endpoint.fd, endpoint.endpoint_type, endpoint.name
                ));
            }
        }
        for endpoint in &endpoints_added_after {
            if endpoint.name != "$SANDBOX/lsof-inventory.txt" {
                violations.push(format!(
                    "endpoint_after_drop:{}:{}:{}",
                    endpoint.fd, endpoint.endpoint_type, endpoint.name
                ));
            }
        }
        if endpoints_after.len() != endpoints_before.len() {
            violations.push(format!(
                "endpoint_count_after_drop:{}:baseline:{}",
                endpoints_after.len(),
                endpoints_before.len()
            ));
        }
        for operation in &secret_operations {
            if !allowed_secret_ref(&operation.secret_ref) {
                violations.push(format!("secret_reference:{}", operation.secret_ref));
            }
            if operation.kind == SecretOperationKind::Delete {
                violations.push(format!("secret_delete:{}", operation.secret_ref));
            }
        }
        for probe in &lock_probes {
            if probe.state != "held_by_bootstrap" {
                violations.push(format!("lock:{}:{}", probe.state, probe.path));
            }
        }

        for required in [
            MAIN_CHAT_EVENT_INTEGRITY_KEY_REF,
            ACTION_QUEUE_AUTHORITY_KEY_REF,
            TASK_STORE_AUTHORITY_KEY_REF,
            AGENT_RUN_RECEIPT_KEY_REF,
        ] {
            if !secret_operations
                .iter()
                .any(|operation| operation.secret_ref == required)
            {
                violations.push(format!("missing_fixed_secret_reference:{required}"));
            }
        }
        if !secret_operations.iter().any(|operation| {
            operation
                .secret_ref
                .strip_prefix(MCP_AUDIT_KEY_REF_PREFIX)
                .is_some_and(|epoch| {
                    !epoch.is_empty() && epoch.chars().all(|character| character.is_ascii_digit())
                })
        }) {
            violations.push("missing_mcp_epoch_secret_reference".into());
        }

        violations.sort();
        violations.dedup();
        let status = if violations.is_empty() {
            "PASS"
        } else {
            "FAIL"
        };
        let report = serde_json::json!({
            "schema_version": "openlife.w0_s3.native_external_state_evidence.v1",
            "status": status,
            "failure_code": (!violations.is_empty()).then_some("W0-NATIVE-UNDECLARED-EXTERNAL-STATE"),
            "scenario": scenario,
            "process_model": {
                "fresh_exact_test_process": true,
                "openlife_data_dir_inherited_before_process_start": true,
                "data_dir": "$SANDBOX/data",
                "product_bootstrap_wrapper_called": false,
                "selected_bootstrap_path": "bootstrap_with_secret_store_for_test",
                "selected_secret_store": "RecordingSecretStore",
                "os_keychain_implementation_selected": false
            },
            "filesystem": {
                "before": filesystem_before,
                "during": filesystem_during,
                "after": filesystem_after,
                "added_during": filesystem_added_during,
                "added_after": filesystem_added_after
            },
            "secret_references": {
                "operations": secret_operations,
                "values_recorded_in_evidence": false,
                "delete_count": secret_operations.iter().filter(|operation| operation.kind == SecretOperationKind::Delete).count()
            },
            "native_endpoints": {
                "inspector": "/usr/sbin/lsof",
                "before_count": endpoints_before.len(),
                "during_count": endpoints_during.len(),
                "after_count": endpoints_after.len(),
                "added_during": endpoints_added_during,
                "added_after": endpoints_added_after,
                "locks_added_during": locks_added_during,
                "lock_probes": lock_probes,
                "sockets_added_during": sockets_added_during
            },
            "counterexample_fixture": fixture_description(&scenario),
            "declared_allowlist": {
                "data_directories": ALLOWED_DATA_DIRECTORIES,
                "database_files": ALLOWED_DATABASES,
                "sqlite_sidecar_suffixes": ["-journal", "-shm", "-wal"],
                "other_data_files": ALLOWED_DATA_FILES,
                "inspector_artifact": "$SANDBOX/lsof-inventory.txt",
                "fixed_secret_references": ALLOWED_FIXED_SECRET_REFS,
                "mcp_secret_reference_pattern": "keychain://com.openlife.desktop/mcp-audit-key-epoch-<ascii-digits>",
                "sockets": []
            },
            "violations": violations,
            "observed_scope": {
                "filesystem": "$SANDBOX",
                "native_endpoints": "current evidence process at three snapshots",
                "secret_operations": "injected RecordingSecretStore references and operation kinds only",
                "excluded_surfaces": [
                    "real OS Keychain contents",
                    "real OpenLife product data"
                ],
                "secret_delete_observed": secret_operations.iter().any(|operation| operation.kind == SecretOperationKind::Delete),
                "non_allowlisted_socket_added_at_snapshots": sockets_added_during.iter().any(|socket| !allowed_endpoint(socket))
            },
            "evidence_credit": {
                "native_harness": true,
                "real_tauri": false,
                "real_os_keychain": false,
                "finding_closure": false
            },
            "unknown": [
                "transient sockets between snapshots",
                "arbitrary filesystem locations outside the isolated sandbox",
                "real Tauri window, setup, and reconciliation",
                "Settings and direct Keyring commands",
                "real OS Keychain contents"
            ]
        });
        println!("{EVIDENCE_PREFIX}{report}");
        assert!(
            violations.is_empty(),
            "W0-NATIVE-UNDECLARED-EXTERNAL-STATE: scenario={scenario}; violations={violations:?}"
        );
    }

    fn fixture_description(scenario: &str) -> &'static str {
        match scenario {
            "positive" => "none",
            "undeclared_file" => "undeclared temporary-sandbox file",
            "undeclared_fd" => "undeclared open descriptor to declared fixture",
            "undeclared_socket" => "undeclared loopback listener",
            _ => "unknown",
        }
    }
}
