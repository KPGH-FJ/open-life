#!/usr/bin/env node

import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

const testName =
  "native_external_state_evidence::tests::w0_s3_native_external_state_isolation";
const evidencePrefix = "W0_S3_NATIVE_EXTERNAL_STATE_EVIDENCE=";
const scenarios = [
  { name: "positive", expectSuccess: true },
  { name: "undeclared_file", expectSuccess: false },
  { name: "undeclared_fd", expectSuccess: false },
  { name: "undeclared_socket", expectSuccess: false },
];
const repoRoot = new URL("..", import.meta.url);
const requiredUnknown = [
  "transient sockets between snapshots",
  "arbitrary filesystem locations outside the isolated sandbox",
  "real Tauri window, setup, and reconciliation",
  "Settings and direct Keyring commands",
  "real OS Keychain contents",
];
const expectedLockPaths = [
  "$DATA/agent_runs.db.openlife-owner.lock",
  "$DATA/main_chat_action_queue.db.openlife-owner.lock",
  "$DATA/tasks.db.openlife-owner.lock",
];

function requireSourceFact(path, pattern, label) {
  const bytes = readFileSync(new URL(path, repoRoot));
  const source = bytes.toString("utf8");
  if (!pattern.test(source)) {
    throw new Error(`source fact missing (${label}): ${path}`);
  }
  return {
    path,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    matchedCodePattern: label,
  };
}

const sourceEvidence = {
  openlifeDataDirSelectsFilesystemRoot: requireSourceFact(
    "src-tauri/src/storage.rs",
    /^\s*if let Ok\(path\) = std::env::var\("OPENLIFE_DATA_DIR"\) \{$/m,
    "OPENLIFE_DATA_DIR filesystem selection",
  ),
  productBootstrapSelectsStartupKeyring: requireSourceFact(
    "src-tauri/src/bootstrap.rs",
    /^\s*bootstrap_with_secret_store\(data_dir, &StartupKeyringSecretStore::default\(\)\)$/m,
    "product bootstrap Keychain selection",
  ),
  productEntrypointCallsProductBootstrap: requireSourceFact(
    "src-tauri/src/lib.rs",
    /^\s*let bootstrap = bootstrap::bootstrap\(data_dir\.clone\(\)\);$/m,
    "Tauri product bootstrap path",
  ),
  injectedTestSeamExists: requireSourceFact(
    "src-tauri/src/bootstrap.rs",
    /^\s*pub\(crate\) fn bootstrap_with_secret_store_for_test\($/m,
    "injected SecretStore seam",
  ),
};

function assertRecord(condition, message) {
  if (!condition) throw new Error(message);
}

function sameJson(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function validateCommonRecord(record, scenario) {
  assertRecord(
    record.schema_version === "openlife.w0_s3.native_external_state_evidence.v1",
    `${scenario}: unexpected evidence schema`,
  );
  assertRecord(record.scenario === scenario, `${scenario}: scenario attribution mismatch`);
  assertRecord(
    sameJson(record.unknown, requiredUnknown),
    `${scenario}: required UNKNOWN boundary changed`,
  );
  const processModel = record.process_model;
  assertRecord(
    processModel?.fresh_exact_test_process === true &&
      processModel.openlife_data_dir_inherited_before_process_start === true &&
      processModel.data_dir === "$SANDBOX/data" &&
      processModel.product_bootstrap_wrapper_called === false &&
      processModel.selected_bootstrap_path === "bootstrap_with_secret_store_for_test" &&
      processModel.selected_secret_store === "RecordingSecretStore" &&
      processModel.os_keychain_implementation_selected === false,
    `${scenario}: process-model isolation contract failed`,
  );
  assertRecord(
    record.observed_scope?.filesystem === "$SANDBOX" &&
      sameJson(record.observed_scope.excluded_surfaces, [
        "real OS Keychain contents",
        "real OpenLife product data",
      ]),
    `${scenario}: observed scope exceeded the isolated harness`,
  );
  assertRecord(
    record.evidence_credit?.native_harness === true &&
      record.evidence_credit.real_tauri === false &&
      record.evidence_credit.real_os_keychain === false &&
      record.evidence_credit.finding_closure === false,
    `${scenario}: evidence-credit boundary changed`,
  );
  assertRecord(
    record.native_endpoints.before_count === record.native_endpoints.after_count,
    `${scenario}: endpoint count did not return to baseline after drop`,
  );
  assertRecord(
    record.native_endpoints.added_after.every(
      (endpoint) => endpoint.name === "$SANDBOX/lsof-inventory.txt",
    ) &&
      !record.native_endpoints.added_after.some((endpoint) =>
        endpoint.name.startsWith("$DATA/"),
      ),
    `${scenario}: data endpoint remained open after bootstrap drop`,
  );
  const lockProbes = record.native_endpoints.lock_probes;
  assertRecord(
    Array.isArray(lockProbes) &&
      lockProbes.length === 3 &&
      sameJson(
        lockProbes.map(({ path }) => path),
        expectedLockPaths,
      ) &&
      lockProbes.every(({ state }) => state === "held_by_bootstrap"),
    `${scenario}: exact owner-lock probes were not all held by bootstrap`,
  );
  assertRecord(
    record.secret_references.values_recorded_in_evidence === false &&
      record.secret_references.delete_count === 0 &&
      record.secret_references.operations.every(
        ({ kind }) => kind === "get" || kind === "set",
      ),
    `${scenario}: secret evidence included values or delete operations`,
  );
  const refs = record.secret_references.operations.map(({ secret_ref: ref }) => ref);
  for (const required of [
    "keychain://com.openlife.desktop/main-chat-event-integrity-key-v1",
    "keychain://com.openlife.desktop/action-queue-authority-key-v1",
    "keychain://com.openlife.desktop/task-store-authority-key-v1",
    "keychain://com.openlife.desktop/agent-run-receipt-key-v1",
  ]) {
    assertRecord(refs.includes(required), `${scenario}: missing fixed secret reference ${required}`);
  }
  assertRecord(
    refs.some((ref) =>
      /^keychain:\/\/com\.openlife\.desktop\/mcp-audit-key-epoch-\d+$/.test(ref),
    ),
    `${scenario}: missing MCP epoch secret-reference evidence`,
  );
}

function runScenario({ name, expectSuccess }) {
  const sandbox = mkdtempSync(join(tmpdir(), `openlife-w0-s3-${name}-`));
  const dataDir = join(sandbox, "data");
  try {
    const result = spawnSync(
      "cargo",
      [
        "test",
        "-p",
        "openlife-tauri",
        testName,
        "--",
        "--exact",
        "--nocapture",
      ],
      {
        cwd: repoRoot,
        env: {
          ...process.env,
          OPENLIFE_W0_S3_EVIDENCE_CHILD: "1",
          OPENLIFE_W0_S3_SANDBOX_ROOT: sandbox,
          OPENLIFE_DATA_DIR: dataDir,
          OPENLIFE_W0_S3_SCENARIO: name,
        },
        encoding: "utf8",
        timeout: 15 * 60 * 1000,
      },
    );
    if (result.error) {
      throw new Error(
        `scenario ${name} failed to start or timed out: ${result.error.code ?? "unknown"} ${result.error.message}`,
      );
    }
    if (result.signal !== null) {
      throw new Error(`scenario ${name} terminated by signal ${result.signal}`);
    }
    if (typeof result.status !== "number") {
      throw new Error(`scenario ${name} returned no numeric exit status`);
    }
    const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
    const recordLine = output
      .split(/\r?\n/)
      .find((line) => line.startsWith(evidencePrefix));
    if (!recordLine) {
      throw new Error(`scenario ${name} emitted no structured evidence record\n${output}`);
    }
    const record = JSON.parse(recordLine.slice(evidencePrefix.length));
    if (`${process.platform}-${process.arch}` !== "darwin-arm64") {
      assertRecord(record.status === "UNKNOWN", "unsupported platform did not report UNKNOWN");
      throw new Error("W0-S3 native endpoint inventory requires darwin-arm64; refusing PASS");
    }
    validateCommonRecord(record, name);
    if (expectSuccess) {
      if (
        result.status !== 0 ||
        record.status !== "PASS" ||
        !sameJson(record.violations, []) ||
        record.native_endpoints.sockets_added_during.length !== 0
      ) {
        throw new Error(
          `positive scenario did not pass: exit=${result.status} status=${record.status}\n${output}`,
        );
      }
    } else {
      if (result.status === 0 || record.status !== "FAIL") {
        throw new Error(
          `counterexample ${name} did not fail closed: exit=${result.status} status=${record.status}\n${output}`,
        );
      }
      if (record.failure_code !== "W0-NATIVE-UNDECLARED-EXTERNAL-STATE") {
        throw new Error(`counterexample ${name} emitted the wrong failure code`);
      }
      const exactViolation =
        name === "undeclared_file"
          ? record.violations.length === 1 &&
            record.violations[0] === "filesystem:file:undeclared-file.txt"
          : name === "undeclared_fd"
            ? record.violations.length === 1 &&
              /^endpoint:\d+:REG:\$SANDBOX\/declared-fd-fixture\.txt$/.test(
                record.violations[0],
              )
            : record.violations.length === 1 &&
              /^endpoint:\d+:IPv[46]:127\.0\.0\.1:\d+$/.test(record.violations[0]);
      if (!exactViolation) {
        throw new Error(
          `counterexample ${name} emitted unexpected or additional violations: ${JSON.stringify(record.violations)}`,
        );
      }
    }
    return {
      scenario: name,
      exit: result.status,
      status: record.status,
      failureCode: record.failure_code,
      filesystemSnapshots: {
        before: record.filesystem.before.length,
        during: record.filesystem.during.length,
        after: record.filesystem.after.length,
      },
      filesystemAddedDuring: record.filesystem.added_during.length,
      descriptorSnapshots: {
        before: record.native_endpoints.before_count,
        during: record.native_endpoints.during_count,
        after: record.native_endpoints.after_count,
      },
      descriptorsAddedDuring: record.native_endpoints.added_during.length,
      locksAddedDuring: record.native_endpoints.locks_added_during.length,
      lockProbes: record.native_endpoints.lock_probes,
      socketsAddedDuring: record.native_endpoints.sockets_added_during.length,
      secretOperations: record.secret_references.operations.length,
      secretDeletes: record.secret_references.delete_count,
      violations: record.violations,
      unknown: record.unknown,
    };
  } finally {
    rmSync(sandbox, { recursive: true, force: true });
  }
}

const records = scenarios.map(runScenario);
console.log(
  JSON.stringify(
    {
      schemaVersion: "openlife.w0_s3.native_external_state_runner.v1",
      status: "PASS",
      platform: `${process.platform}-${process.arch}`,
      sourceEvidence,
      redProof: {
        code: "W0-NATIVE-DATA-DIR-DOES-NOT-ISOLATE-KEYCHAIN",
        statement:
          "OPENLIFE_DATA_DIR selects filesystem state, while the product bootstrap independently selects StartupKeyringSecretStore.",
      },
      scenarios: records,
      claims: {
        nativeBootstrapWithInjectedSecretStore: true,
        selectedPathCredit: "injected-test-seam-only",
        realKeychainCredit: false,
        fullTauriOrProductTrialCredit: false,
        findingClosureCredit: false,
      },
    },
    null,
    2,
  ),
);
