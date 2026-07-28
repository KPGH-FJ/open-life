#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync, spawnSync } from "node:child_process";

const sourceRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const fixturePrefix = join(realpathSync(tmpdir()), "openlife-program-validator-test-");
const tempRoot = mkdtempSync(fixturePrefix);
const fixtureRoot = join(tempRoot, "fixture");
const baselineSha = "de158ce53018c9c649f7dc0dcb3bdd8271ed4977";
const predecessorHandoffSha = "8a607bb4f9f392f573c98fa74e8c575d6c2c014d";
const validatorPath = "scripts/validate-current-development-program.mjs";
const programPath = "plans/openlife_current_development_program.json";
const ledgerPath = "plans/openlife_problem_ledger.json";
const overlayPaths = [
  "AGENTS.md",
  "plans/README.md",
  "plans/openlife_single_system_deletion_manifest.md",
  "plans/openlife_single_system_development_preparation.md",
  "plans/openlife_current_development_program.md",
  programPath,
  ledgerPath,
  validatorPath,
  "scripts/test-current-development-program-validator.mjs",
];
const expectedScenarioLabels = [
  "DRAFT_VALID",
  "DRAFT_UNADJUDICATED_MISSING_OWNER_NEGATIVE",
  "DRAFT_PREDECESSOR_IN_PLACE_MUTATION_NEGATIVE",
  "DRAFT_ADJUDICATION_WRONG_DIGEST_NEGATIVE",
  "DRAFT_ADJUDICATION_OUTSIDE_ALLOWLIST_NEGATIVE",
  "DRAFT_ADJUDICATION_DUPLICATE_TARGET_NEGATIVE",
  "DRAFT_ADJUDICATION_CLOSURE_INFLATION_NEGATIVE",
  "DRAFT_REPLACE_REF_NEGATIVE",
  "DRAFT_DIRTY_NEGATIVE",
  "DRAFT_MERGE_NEGATIVE",
  "ACTIVATION_VALID",
  "ACTIVATION_SELF_CREDIT_NEGATIVE",
  "ACTIVATION_SUBSTANTIVE_NEGATIVE",
  "ONGOING_BOOTSTRAP",
  "ONGOING_FRESH_W0_S3_RECOVERY",
  "ONGOING_RECOVERY_MISSING_HISTORICAL_SCOPE_NEGATIVE",
  "ONGOING_UNRECEIPTED_SIDE_COMMIT_NEGATIVE",
  "ONGOING_MERGE_RESOLUTION_NEGATIVE",
  "ONGOING_RELOCATED_DETACHED_CI",
  "PACKET_VALID",
  "PACKET_RECOVERY_NARROWED_OWNER_NEGATIVE",
  "PACKET_RECOVERY_MISSING_HISTORICAL_RED_NEGATIVE",
  "PACKET_RECOVERY_CONDITIONAL_BYPASS_NEGATIVE",
  "PACKET_ROLE_NEGATIVE",
  "PACKET_COMMAND_NEGATIVE",
  "PACKET_SOURCE_TAMPER_NEGATIVE",
  "PREDECESSOR_NEGATIVE",
  "W5_DISPATCH_NEGATIVE",
];
const observedScenarioLabels = [];
const cleanup = () => {
  if (tempRoot.startsWith(fixturePrefix)) {
    rmSync(tempRoot, { recursive: true, force: true });
  }
};
for (const signal of ["SIGINT", "SIGTERM"]) {
  process.once(signal, () => {
    cleanup();
    process.exit(signal === "SIGINT" ? 130 : 143);
  });
}

const canonicalize = value => {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map(key => [key, canonicalize(value[key])])
    );
  }
  return value;
};
const canonicalDigest = value =>
  createHash("sha256")
    .update(JSON.stringify(canonicalize(value)), "utf8")
    .digest("hex");
const textDigest = value => createHash("sha256").update(value, "utf8").digest("hex");
const git = (cwd, ...args) =>
  execFileSync("git", args, {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
const readBlobAtCommit = (cwd, sha, path) =>
  execFileSync("git", ["show", `${sha}:${path}`], {
    cwd,
    stdio: ["ignore", "pipe", "pipe"],
  });
const readJson = (cwd, path) => JSON.parse(readFileSync(join(cwd, path), "utf8"));
const writeJson = (cwd, path, value) =>
  writeFileSync(join(cwd, path), `${JSON.stringify(value, null, 2)}\n`);
const commitAll = (cwd, message) => {
  git(cwd, "add", "-A");
  git(cwd, "commit", "-m", message);
  return git(cwd, "rev-parse", "HEAD");
};
const validator = (cwd, args) =>
  spawnSync(process.execPath, [validatorPath, ...args], {
    cwd,
    encoding: "utf8",
    env: { ...process.env, CI: "1" },
    timeout: 60_000,
  });
const expectValidator = ({ cwd, args, expectedExit, diagnostic = null, label }) => {
  observedScenarioLabels.push(label);
  const ordinal = observedScenarioLabels.length;
  process.stdout.write(`START ${ordinal}/${expectedScenarioLabels.length} ${label}\n`);
  const result = validator(cwd, args);
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  if (result.status !== expectedExit || (diagnostic && !diagnostic.test(output))) {
    throw new Error(
      `${label}: expected exit ${expectedExit}` +
        `${diagnostic ? ` and ${diagnostic}` : ""}, got ${result.status}` +
        `${result.error ? ` (${result.error.message})` : ""}\n${output}`
    );
  }
  process.stdout.write(`PASS ${ordinal}/${expectedScenarioLabels.length} ${label}\n`);
};
const freezePacket = packet => {
  packet.packet_status = "FROZEN_FOR_DISPATCH";
  packet.packet_sha256 = null;
  packet.packet_payload_sha256 = null;
  packet.packet_freeze_review = null;
  const payload = structuredClone(packet);
  payload.packet_sha256 = null;
  payload.packet_payload_sha256 = null;
  payload.packet_freeze_review = null;
  packet.packet_payload_sha256 = canonicalDigest(payload);
  packet.packet_freeze_review = {
    outcome: "PASS",
    reviewed_payload_sha256: packet.packet_payload_sha256,
    integrator_id: "selftest-integrator",
    reviewer_id: "selftest-freeze-reviewer",
    artifact_or_record: "selftest:packet-freeze",
  };
  const packetForDigest = structuredClone(packet);
  packetForDigest.packet_sha256 = null;
  packet.packet_sha256 = canonicalDigest(packetForDigest);
  return packet;
};
const writePacket = (name, packet) => {
  const path = join(tempRoot, `${name}.json`);
  writeFileSync(path, `${JSON.stringify(packet, null, 2)}\n`);
  return path;
};
const removeUnexpectedRemoteRefs = cwd => {
  for (const ref of git(cwd, "for-each-ref", "--format=%(refname)", "refs/remotes/origin")
    .split("\n")
    .filter(Boolean)) {
    if (ref !== "refs/remotes/origin/main" && ref !== "refs/remotes/origin/HEAD") {
      git(cwd, "update-ref", "-d", ref);
    }
  }
};
const removeUnexpectedLocalBranches = (cwd, allowedBranches) => {
  for (const branch of git(cwd, "for-each-ref", "--format=%(refname:short)", "refs/heads")
    .split("\n")
    .filter(Boolean)) {
    if (!allowedBranches.includes(branch)) {
      git(cwd, "branch", "-D", branch);
    }
  }
};

try {
  if (git(sourceRoot, "status", "--porcelain=v1", "--untracked-files=all") !== "") {
    throw new Error(
      "Validator self-test requires a clean source checkout so every fixture byte is bound to HEAD"
    );
  }
  const sourceHeadSha = git(sourceRoot, "rev-parse", "HEAD");
  const makeDraftVariant = ({ name, mutate }) => {
    const variantRoot = join(tempRoot, name);
    execFileSync("git", ["clone", "--local", "--no-hardlinks", sourceRoot, variantRoot], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    git(variantRoot, "config", "user.name", "OpenLife Validator Selftest");
    git(variantRoot, "config", "user.email", "openlife-validator-selftest@example.invalid");
    git(variantRoot, "branch", "-f", "main", predecessorHandoffSha);
    git(variantRoot, "checkout", "-B", `codex/${name}`, predecessorHandoffSha);
    removeUnexpectedLocalBranches(variantRoot, ["main", `codex/${name}`]);
    git(variantRoot, "update-ref", "refs/remotes/origin/main", predecessorHandoffSha);
    removeUnexpectedRemoteRefs(variantRoot);
    for (const path of overlayPaths) {
      mkdirSync(dirname(join(variantRoot, path)), { recursive: true });
      writeFileSync(join(variantRoot, path), readBlobAtCommit(sourceRoot, sourceHeadSha, path));
    }
    const variantProgram = readJson(variantRoot, programPath);
    const variantLedger = readJson(variantRoot, ledgerPath);
    mutate({ program: variantProgram, ledger: variantLedger });
    writeJson(variantRoot, programPath, variantProgram);
    writeJson(variantRoot, ledgerPath, variantLedger);
    commitAll(variantRoot, `test: ${name}`);
    return variantRoot;
  };
  execFileSync("git", ["clone", "--local", "--no-hardlinks", sourceRoot, fixtureRoot], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  git(fixtureRoot, "config", "user.name", "OpenLife Validator Selftest");
  git(fixtureRoot, "config", "user.email", "openlife-validator-selftest@example.invalid");
  git(fixtureRoot, "branch", "-f", "main", predecessorHandoffSha);
  git(fixtureRoot, "checkout", "-B", "codex/current-development-program", predecessorHandoffSha);
  removeUnexpectedLocalBranches(fixtureRoot, ["main", "codex/current-development-program"]);
  git(fixtureRoot, "update-ref", "refs/remotes/origin/main", predecessorHandoffSha);
  removeUnexpectedRemoteRefs(fixtureRoot);

  for (const path of overlayPaths) {
    mkdirSync(dirname(join(fixtureRoot, path)), { recursive: true });
    writeFileSync(join(fixtureRoot, path), readBlobAtCommit(sourceRoot, sourceHeadSha, path));
  }
  const draftSha = commitAll(fixtureRoot, "test: synthetic Program draft");

  expectValidator({
    cwd: fixtureRoot,
    args: ["--profile=draft"],
    expectedExit: 0,
    label: "DRAFT_VALID",
  });
  const unadjudicatedRoot = makeDraftVariant({
    name: "selftest-unadjudicated-missing-owner",
    mutate: ({ ledger }) => {
      ledger.receipt_adjudication_records = [];
    },
  });
  expectValidator({
    cwd: unadjudicatedRoot,
    args: ["--profile=draft"],
    expectedExit: 1,
    diagnostic: /credited task evidence does not cover/,
    label: "DRAFT_UNADJUDICATED_MISSING_OWNER_NEGATIVE",
  });
  const inPlaceMutationRoot = makeDraftVariant({
    name: "selftest-predecessor-in-place-mutation",
    mutate: ({ ledger }) => {
      const target = ledger.integration_records.find(
        record => record.record_id === "W0-S3-INTEGRATION-001"
      );
      target.task_evidence_records[0].scope_paths.push(
        "openlife-core/src/mcp_audit.rs",
        "src-tauri/src/bootstrap.rs",
        "src-tauri/src/secret_store.rs"
      );
      ledger.receipt_adjudication_records[0].target_record_canonical_sha256 =
        canonicalDigest(target);
      ledger.receipt_adjudication_records[0].independent_review.reviewed_target_record_sha256 =
        canonicalDigest(target);
    },
  });
  expectValidator({
    cwd: inPlaceMutationRoot,
    args: ["--profile=draft"],
    expectedExit: 1,
    diagnostic: /mutated predecessor append-only sequence: integration_records/,
    label: "DRAFT_PREDECESSOR_IN_PLACE_MUTATION_NEGATIVE",
  });
  const wrongDigestRoot = makeDraftVariant({
    name: "selftest-adjudication-wrong-digest",
    mutate: ({ ledger }) => {
      ledger.receipt_adjudication_records[0].target_record_canonical_sha256 = "0".repeat(64);
    },
  });
  expectValidator({
    cwd: wrongDigestRoot,
    args: ["--profile=draft"],
    expectedExit: 1,
    diagnostic: /target canonical digest is invalid/,
    label: "DRAFT_ADJUDICATION_WRONG_DIGEST_NEGATIVE",
  });
  const outsideAllowlistRoot = makeDraftVariant({
    name: "selftest-adjudication-outside-allowlist",
    mutate: ({ ledger }) => {
      const alternate = ledger.integration_records.find(
        record => record.record_id === "W0-S2-INTEGRATION-001"
      );
      const adjudication = ledger.receipt_adjudication_records[0];
      adjudication.target_record_id = alternate.record_id;
      adjudication.target_record_canonical_sha256 = canonicalDigest(alternate);
      adjudication.independent_review.reviewed_target_record_sha256 = canonicalDigest(alternate);
    },
  });
  expectValidator({
    cwd: outsideAllowlistRoot,
    args: ["--profile=draft"],
    expectedExit: 1,
    diagnostic: /unauthorized or duplicate receipt/,
    label: "DRAFT_ADJUDICATION_OUTSIDE_ALLOWLIST_NEGATIVE",
  });
  const duplicateTargetRoot = makeDraftVariant({
    name: "selftest-adjudication-duplicate-target",
    mutate: ({ ledger }) => {
      const duplicate = structuredClone(ledger.receipt_adjudication_records[0]);
      duplicate.adjudication_id = "W0-S3-RECEIPT-ADJUDICATION-SELFTEST-DUPLICATE";
      ledger.receipt_adjudication_records.push(duplicate);
    },
  });
  expectValidator({
    cwd: duplicateTargetRoot,
    args: ["--profile=draft"],
    expectedExit: 1,
    diagnostic: /unauthorized or duplicate receipt/,
    label: "DRAFT_ADJUDICATION_DUPLICATE_TARGET_NEGATIVE",
  });
  const closureInflationRoot = makeDraftVariant({
    name: "selftest-adjudication-closure-inflation",
    mutate: ({ ledger }) => {
      ledger.receipt_adjudication_records[0].closure_credit_delta = 1;
    },
  });
  expectValidator({
    cwd: closureInflationRoot,
    args: ["--profile=draft"],
    expectedExit: 1,
    diagnostic: /fields: expected/,
    label: "DRAFT_ADJUDICATION_CLOSURE_INFLATION_NEGATIVE",
  });
  git(fixtureRoot, "replace", draftSha, predecessorHandoffSha);
  expectValidator({
    cwd: fixtureRoot,
    args: ["--profile=draft"],
    expectedExit: 1,
    diagnostic: /replacement refs are forbidden/,
    label: "DRAFT_REPLACE_REF_NEGATIVE",
  });
  git(fixtureRoot, "replace", "-d", draftSha);
  const dirtyProbePath = join(fixtureRoot, "plans/selftest-dirty-probe.json");
  writeFileSync(dirtyProbePath, "dirty\n");
  expectValidator({
    cwd: fixtureRoot,
    args: ["--profile=draft"],
    expectedExit: 1,
    diagnostic: /clean tracked commit/,
    label: "DRAFT_DIRTY_NEGATIVE",
  });
  rmSync(dirtyProbePath);

  git(fixtureRoot, "checkout", "-B", "selftest-merge-side", predecessorHandoffSha);
  git(fixtureRoot, "commit", "--allow-empty", "-m", "test: synthetic merge side");
  git(fixtureRoot, "checkout", "-B", "codex/current-development-program", predecessorHandoffSha);
  git(fixtureRoot, "merge", "--no-ff", "selftest-merge-side", "-m", "test: synthetic merge draft");
  for (const path of overlayPaths) {
    writeFileSync(join(fixtureRoot, path), readBlobAtCommit(sourceRoot, sourceHeadSha, path));
  }
  git(fixtureRoot, "add", "-A");
  git(fixtureRoot, "commit", "--amend", "--no-edit");
  git(fixtureRoot, "branch", "-D", "selftest-merge-side");
  expectValidator({
    cwd: fixtureRoot,
    args: ["--profile=draft"],
    expectedExit: 1,
    diagnostic: /single-parent direct child/,
    label: "DRAFT_MERGE_NEGATIVE",
  });

  git(fixtureRoot, "reset", "--hard", predecessorHandoffSha);
  for (const path of overlayPaths) {
    writeFileSync(join(fixtureRoot, path), readBlobAtCommit(sourceRoot, sourceHeadSha, path));
  }
  const relocatedDraftProgram = readJson(fixtureRoot, programPath);
  relocatedDraftProgram.slice_contract.writable_checkout = fixtureRoot;
  relocatedDraftProgram.waves[0].slices[0].task_packet_blueprint.checkout = fixtureRoot;
  writeJson(fixtureRoot, programPath, relocatedDraftProgram);
  const relocatedDraftSha = commitAll(fixtureRoot, "test: synthetic relocated Program draft");

  const activationProgram = readJson(fixtureRoot, programPath);
  const activationLedger = readJson(fixtureRoot, ledgerPath);
  activationProgram.status = "APPROVED_FOR_EXECUTION";
  activationProgram.execution_authorized = true;
  Object.assign(activationProgram.program_approval, {
    status: "APPROVED_BY_USER",
    execution_authority_granted: true,
    approved_program_schema_version: activationProgram.schema_version,
    approved_draft_commit_sha: relocatedDraftSha,
    approved_by: "USER",
    approved_at: "2026-07-24T00:00:00.000Z",
    approval_record: "selftest:user-approved-exact-draft",
  });
  Object.assign(activationProgram.program_approval.independent_challenge, {
    status: "PASS",
    reviewed_head_sha: relocatedDraftSha,
    reviewer_ids: ["selftest-independent-challenger"],
    outcome: "PASS",
    artifact_or_record: "selftest:exact-draft-challenge",
    blocking_findings: [],
  });
  activationProgram.program_activation.status = "ACTIVE";
  const activationGate = activationProgram.gates.find(gate => gate.id === "G-PROGRAM-ACTIVATION");
  activationGate.status = "PASS";
  activationGate.evidence_records = [
    {
      subject_sha: relocatedDraftSha,
      artifact_or_record: "selftest:user-approval",
      record_id: "SELFTEST-ACTIVATION-USER",
      scope_id: "G-PROGRAM-ACTIVATION",
      scope_paths: [programPath],
      guard_ids: [],
      dimensions: ["USER_PROGRAM_APPROVAL"],
      outcome: "PASS",
      limitations: [],
      producer_id: "selftest-user-record",
      reviewer_id: "selftest-user-record-reviewer",
      credit_allowed: true,
      active: true,
    },
    {
      subject_sha: relocatedDraftSha,
      artifact_or_record: "selftest:tracked-authority",
      record_id: "SELFTEST-ACTIVATION-AUTHORITY",
      scope_id: "G-PROGRAM-ACTIVATION",
      scope_paths: [ledgerPath],
      guard_ids: [],
      dimensions: ["TRACKED_AUTHORITY"],
      outcome: "PASS",
      limitations: [],
      producer_id: "selftest-authority-record",
      reviewer_id: "selftest-authority-record-reviewer",
      credit_allowed: true,
      active: true,
    },
  ];
  activationProgram.waves[0].status = "READY";
  activationLedger.status = "ACTIVE";
  activationLedger.authority.execution_authorized = true;
  writeJson(fixtureRoot, programPath, activationProgram);
  writeJson(fixtureRoot, ledgerPath, activationLedger);
  const activationSha = commitAll(fixtureRoot, "test: synthetic exact-draft activation");

  expectValidator({
    cwd: fixtureRoot,
    args: ["--profile=activation"],
    expectedExit: 0,
    label: "ACTIVATION_VALID",
  });

  const activationNegativeRoot = join(tempRoot, "activation-negative");
  execFileSync("git", ["clone", "--local", "--no-hardlinks", fixtureRoot, activationNegativeRoot], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  git(activationNegativeRoot, "config", "user.name", "OpenLife Validator Selftest");
  git(
    activationNegativeRoot,
    "config",
    "user.email",
    "openlife-validator-selftest@example.invalid"
  );
  git(
    activationNegativeRoot,
    "checkout",
    "-B",
    "codex/current-development-program",
    relocatedDraftSha
  );
  git(activationNegativeRoot, "branch", "-f", "main", baselineSha);
  git(activationNegativeRoot, "update-ref", "refs/remotes/origin/main", baselineSha);
  removeUnexpectedRemoteRefs(activationNegativeRoot);
  const activationProgramSnapshot = JSON.parse(
    git(fixtureRoot, "show", `${activationSha}:${programPath}`)
  );
  const activationLedgerSnapshot = JSON.parse(
    git(fixtureRoot, "show", `${activationSha}:${ledgerPath}`)
  );
  const selfCreditedActivationProgram = structuredClone(activationProgramSnapshot);
  selfCreditedActivationProgram.gates
    .find(gate => gate.id === "G-PROGRAM-ACTIVATION")
    .evidence_records.push({
      subject_sha: relocatedDraftSha,
      artifact_or_record: "selftest:forbidden-self-credit",
      record_id: "SELFTEST-ACTIVATION-FORBIDDEN-SELF-CREDIT",
      scope_id: "G-PROGRAM-ACTIVATION",
      scope_paths: [validatorPath],
      guard_ids: [],
      dimensions: ["VALIDATOR_SELF_TEST"],
      outcome: "PASS",
      limitations: [],
      producer_id: "selftest-validator-run",
      reviewer_id: "selftest-validator-run-reviewer",
      credit_allowed: true,
      active: true,
    });
  writeJson(activationNegativeRoot, programPath, selfCreditedActivationProgram);
  writeJson(activationNegativeRoot, ledgerPath, activationLedgerSnapshot);
  commitAll(activationNegativeRoot, "test: forbidden activation self-credit");
  expectValidator({
    cwd: activationNegativeRoot,
    args: ["--profile=activation"],
    expectedExit: 1,
    diagnostic: /surplus self-credited dimension/,
    label: "ACTIVATION_SELF_CREDIT_NEGATIVE",
  });

  git(activationNegativeRoot, "reset", "--hard", relocatedDraftSha);
  const mutatedActivationProgram = structuredClone(activationProgramSnapshot);
  mutatedActivationProgram.decision.plain_language += " SELFTEST-MUTATION";
  writeJson(activationNegativeRoot, programPath, mutatedActivationProgram);
  writeJson(activationNegativeRoot, ledgerPath, activationLedgerSnapshot);
  commitAll(activationNegativeRoot, "test: forbidden substantive activation mutation");
  expectValidator({
    cwd: activationNegativeRoot,
    args: ["--profile=activation"],
    expectedExit: 1,
    diagnostic: /Activation changed substantive Program or ledger content/,
    label: "ACTIVATION_SUBSTANTIVE_NEGATIVE",
  });

  git(fixtureRoot, "branch", "-f", "main", activationSha);
  git(fixtureRoot, "update-ref", "refs/remotes/origin/main", activationSha);
  git(fixtureRoot, "checkout", "main");
  git(fixtureRoot, "branch", "-D", "codex/current-development-program");
  expectValidator({
    cwd: fixtureRoot,
    args: ["--profile=ongoing"],
    expectedExit: 0,
    label: "ONGOING_BOOTSTRAP",
  });

  const integratedRoot = join(tempRoot, "integrated-w0-s3-recovery");
  execFileSync("git", ["clone", "--local", "--no-hardlinks", fixtureRoot, integratedRoot], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  git(integratedRoot, "config", "user.name", "OpenLife Validator Selftest");
  git(integratedRoot, "config", "user.email", "openlife-validator-selftest@example.invalid");
  git(integratedRoot, "checkout", "-b", "codex/selftest-w0-s3-recovery", activationSha);
  const integratedProgram = readJson(integratedRoot, programPath);
  const predecessorLedgerAtActivation = readJson(integratedRoot, ledgerPath);
  const invalidW0s3Record = predecessorLedgerAtActivation.integration_records.find(
    record => record.record_id === "W0-S3-INTEGRATION-001"
  );
  const integratedW0s3 = integratedProgram.waves[0].slices.find(slice => slice.id === "W0-S3");
  const integratedPacket = structuredClone(
    readJson(integratedRoot, invalidW0s3Record.packet_artifact_path)
  );
  Object.assign(integratedPacket, {
    task_id: "W0-S3-SELFTEST-RECOVERY",
    program_schema_version: integratedProgram.schema_version,
    mode: "VERIFICATION",
    wave_id: "WAVE-0",
    slice_id: "W0-S3",
    slice_exit_contract_id: integratedW0s3.exit_contract_id,
    finding_ids: [...integratedW0s3.finding_ids],
    required_guard_ids: [...integratedW0s3.required_guard_ids],
    governance_task: integratedW0s3.governance_task,
    risk_class: integratedW0s3.risk_class,
    root_cause_cluster_id: integratedW0s3.root_cause_cluster_id,
    objective: integratedW0s3.objective,
    non_goals: [...integratedW0s3.non_goals],
    invariant:
      "A fresh v1.0.3 verification receipt covers all historical changed and canonical-owner paths without changing product behavior.",
    red_contract: [...integratedW0s3.red_contract],
    minimal_fix_contract:
      "Re-run the frozen proof and issue a distinct verification receipt; never edit or re-credit the old receipt.",
    old_path_deletion_contract:
      "The invalid predecessor receipt remains immutable raw history and permanently no-credit.",
    acceptance_criteria: [...integratedPacket.acceptance_criteria, integratedW0s3.exit],
    program_activation_sha: activationSha,
    execution_baseline_sha: activationSha,
    expected_parent_main_sha: activationSha,
    checkout: integratedProgram.slice_contract.writable_checkout,
    branch: "codex/selftest-w0-s3-recovery",
    assigned_agent_id: "selftest-recovery-verifier",
  });
  freezePacket(integratedPacket);
  git(integratedRoot, "commit", "--allow-empty", "-m", "test: current W0-S3 verification subject");
  const integratedTaskHead = git(integratedRoot, "rev-parse", "HEAD");
  git(integratedRoot, "checkout", "main");
  git(
    integratedRoot,
    "merge",
    "--no-ff",
    "codex/selftest-w0-s3-recovery",
    "-m",
    "test: merge current W0-S3 verification subject"
  );
  git(integratedRoot, "branch", "-D", "codex/selftest-w0-s3-recovery");
  const packetArtifactPath = `plans/openlife_task_packets/${integratedPacket.packet_sha256}.json`;
  writeJson(integratedRoot, packetArtifactPath, integratedPacket);
  const attemptArtifact = {
    attempt_id: "W0-S3-SELFTEST-RECOVERY-ATTEMPT",
    task_id: integratedPacket.task_id,
    slice_id: integratedPacket.slice_id,
    root_cause_cluster_id: integratedPacket.root_cause_cluster_id,
    packet_sha256: integratedPacket.packet_sha256,
    execution_baseline_sha: activationSha,
    outcome: "SUCCEEDED",
    reason:
      "Synthetic self-test proves a distinct fresh verification receipt recovers effective W0-S3 credit.",
    producer_id: integratedPacket.assigned_agent_id,
    hypothesis:
      "Fresh same-head verification across all six paths can recover credit without modifying the old receipt.",
    change_summary: ["Ran a synthetic current-SHA verification with no product diff."],
    evaluated_gate_ids: [...integratedPacket.red_contract, ...integratedPacket.required_guard_ids],
    failed_gate_ids: [],
    failure_signature: "NONE",
    observed_diff_or_log: `verification_subject ${integratedTaskHead}\nproduct_changed_paths 0\n`,
    observed_diff_or_log_sha256: null,
  };
  attemptArtifact.observed_diff_or_log_sha256 = textDigest(attemptArtifact.observed_diff_or_log);
  const attemptArtifactText = `${JSON.stringify(attemptArtifact, null, 2)}\n`;
  const attemptArtifactDigest = textDigest(attemptArtifactText);
  const attemptArtifactPath = `plans/openlife_attempt_artifacts/${attemptArtifactDigest}.json`;
  mkdirSync(dirname(join(integratedRoot, attemptArtifactPath)), { recursive: true });
  writeFileSync(join(integratedRoot, attemptArtifactPath), attemptArtifactText);
  const artifactCommitSha = commitAll(integratedRoot, "test: archive W0-S3 recovery artifacts");
  const integratedLedger = readJson(integratedRoot, ledgerPath);
  integratedLedger.integration_records.push({
    record_id: "W0-S3-SELFTEST-RECOVERY-INTEGRATION",
    task_id: integratedPacket.task_id,
    slice_id: integratedPacket.slice_id,
    integrator_id: integratedPacket.packet_freeze_review.integrator_id,
    producer_id: integratedPacket.assigned_agent_id,
    program_approved_draft_sha: relocatedDraftSha,
    program_activation_sha: activationSha,
    execution_baseline_sha: activationSha,
    packet_sha256: integratedPacket.packet_sha256,
    packet_artifact_path: packetArtifactPath,
    range_base_sha: activationSha,
    range_head_sha: integratedTaskHead,
    changed_paths: [],
    allowed_touched_paths: [...integratedPacket.allowed_touched_paths],
    required_guard_ids: [...integratedPacket.required_guard_ids],
    completion_outcome: "PASS",
    task_evidence_records: [
      {
        subject_sha: integratedTaskHead,
        artifact_or_record: "selftest:W0-S3-fresh-recovery-evidence",
        record_id: "W0-S3-SELFTEST-RECOVERY-EVIDENCE",
        scope_id: "W0-S3",
        scope_paths: [...integratedPacket.allowed_touched_paths],
        guard_ids: [...integratedPacket.required_guard_ids],
        dimensions: [...integratedPacket.required_evidence_dimensions],
        outcome: "PASS",
        limitations: [],
        producer_id: integratedPacket.assigned_agent_id,
        reviewer_id: "selftest-task-evidence-reviewer",
        credit_allowed: true,
        active: true,
      },
    ],
    independent_review: {
      outcome: "PASS",
      reviewed_head_sha: integratedTaskHead,
      reviewer_id: "selftest-integration-reviewer",
      artifact_or_record: "selftest:W0-S3-fresh-recovery-independent-review",
    },
  });
  integratedLedger.implementation_attempt_records.push({
    attempt_id: attemptArtifact.attempt_id,
    task_id: integratedPacket.task_id,
    slice_id: integratedPacket.slice_id,
    root_cause_cluster_id: integratedPacket.root_cause_cluster_id,
    packet_sha256: integratedPacket.packet_sha256,
    packet_artifact_path: packetArtifactPath,
    execution_baseline_sha: activationSha,
    artifact_commit_sha: artifactCommitSha,
    attempt_artifact_path: attemptArtifactPath,
    attempt_artifact_sha256: attemptArtifactDigest,
    outcome: "SUCCEEDED",
    reason: attemptArtifact.reason,
    producer_id: integratedPacket.assigned_agent_id,
    integrator_id: integratedPacket.packet_freeze_review.integrator_id,
    record_note: "Synthetic receipt exists only inside the disposable validator fixture.",
  });
  integratedLedger.current_inventory.last_reconciled_execution_sha = artifactCommitSha;
  writeJson(integratedRoot, ledgerPath, integratedLedger);
  const integratedHead = commitAll(integratedRoot, "test: record W0-S3 recovery receipt");
  git(integratedRoot, "update-ref", "refs/remotes/origin/main", integratedHead);
  removeUnexpectedRemoteRefs(integratedRoot);
  expectValidator({
    cwd: integratedRoot,
    args: ["--profile=ongoing"],
    expectedExit: 0,
    label: "ONGOING_FRESH_W0_S3_RECOVERY",
  });

  const missingRecoveryScopeRoot = join(tempRoot, "missing-recovery-historical-scope");
  execFileSync(
    "git",
    ["clone", "--local", "--no-hardlinks", integratedRoot, missingRecoveryScopeRoot],
    {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }
  );
  git(missingRecoveryScopeRoot, "config", "user.name", "OpenLife Validator Selftest");
  git(
    missingRecoveryScopeRoot,
    "config",
    "user.email",
    "openlife-validator-selftest@example.invalid"
  );
  git(missingRecoveryScopeRoot, "checkout", "-B", "main", artifactCommitSha);
  const missingRecoveryScopeLedger = readJson(missingRecoveryScopeRoot, ledgerPath);
  const incompleteRecoveryRecord = structuredClone(integratedLedger.integration_records.at(-1));
  incompleteRecoveryRecord.task_evidence_records[0].scope_paths =
    incompleteRecoveryRecord.task_evidence_records[0].scope_paths.filter(
      path => path !== "scripts/run-w0-s3-native-external-state-evidence.mjs"
    );
  missingRecoveryScopeLedger.integration_records.push(incompleteRecoveryRecord);
  missingRecoveryScopeLedger.implementation_attempt_records.push(
    structuredClone(integratedLedger.implementation_attempt_records.at(-1))
  );
  missingRecoveryScopeLedger.current_inventory.last_reconciled_execution_sha = artifactCommitSha;
  writeJson(missingRecoveryScopeRoot, ledgerPath, missingRecoveryScopeLedger);
  const missingRecoveryScopeHead = commitAll(
    missingRecoveryScopeRoot,
    "test: record incomplete W0-S3 recovery receipt"
  );
  git(missingRecoveryScopeRoot, "update-ref", "refs/remotes/origin/main", missingRecoveryScopeHead);
  removeUnexpectedRemoteRefs(missingRecoveryScopeRoot);
  expectValidator({
    cwd: missingRecoveryScopeRoot,
    args: ["--profile=ongoing"],
    expectedExit: 1,
    diagnostic: /does not reproduce the frozen predecessor verification scope/,
    label: "ONGOING_RECOVERY_MISSING_HISTORICAL_SCOPE_NEGATIVE",
  });

  const unreceiptedRoot = join(tempRoot, "unreceipted-side-commit");
  execFileSync("git", ["clone", "--local", "--no-hardlinks", integratedRoot, unreceiptedRoot], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  git(unreceiptedRoot, "config", "user.name", "OpenLife Validator Selftest");
  git(unreceiptedRoot, "config", "user.email", "openlife-validator-selftest@example.invalid");
  git(unreceiptedRoot, "checkout", "-b", "selftest-unreceipted-side", integratedHead);
  writeFileSync(
    join(unreceiptedRoot, "openlife-core/src/tasks.rs"),
    `${readFileSync(join(unreceiptedRoot, "openlife-core/src/tasks.rs"), "utf8")}\n// selftest unreceipted side\n`
  );
  commitAll(unreceiptedRoot, "test: unreceipted side commit");
  git(unreceiptedRoot, "checkout", "main");
  git(
    unreceiptedRoot,
    "merge",
    "--no-ff",
    "selftest-unreceipted-side",
    "-m",
    "test: clean unreceipted merge"
  );
  git(unreceiptedRoot, "branch", "-D", "selftest-unreceipted-side");
  const unreceiptedHead = git(unreceiptedRoot, "rev-parse", "HEAD");
  git(unreceiptedRoot, "update-ref", "refs/remotes/origin/main", unreceiptedHead);
  removeUnexpectedRemoteRefs(unreceiptedRoot);
  expectValidator({
    cwd: unreceiptedRoot,
    args: ["--profile=ongoing"],
    expectedExit: 1,
    diagnostic:
      /Integrated main task-receipt coverage|Integrated product commit lacks a verifiable task receipt/,
    label: "ONGOING_UNRECEIPTED_SIDE_COMMIT_NEGATIVE",
  });

  const mergeResolutionRoot = join(tempRoot, "merge-resolution");
  execFileSync("git", ["clone", "--local", "--no-hardlinks", integratedRoot, mergeResolutionRoot], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  git(mergeResolutionRoot, "config", "user.name", "OpenLife Validator Selftest");
  git(mergeResolutionRoot, "config", "user.email", "openlife-validator-selftest@example.invalid");
  git(mergeResolutionRoot, "checkout", "-b", "selftest-merge-resolution-side", integratedHead);
  git(mergeResolutionRoot, "commit", "--allow-empty", "-m", "test: empty merge-resolution side");
  git(mergeResolutionRoot, "checkout", "main");
  git(mergeResolutionRoot, "commit", "--allow-empty", "-m", "test: empty merge-resolution main");
  git(mergeResolutionRoot, "merge", "--no-ff", "--no-commit", "selftest-merge-resolution-side");
  writeFileSync(
    join(mergeResolutionRoot, "openlife-core/src/tasks.rs"),
    `${readFileSync(join(mergeResolutionRoot, "openlife-core/src/tasks.rs"), "utf8")}\n// selftest merge-only resolution\n`
  );
  commitAll(mergeResolutionRoot, "test: merge-only resolution");
  git(mergeResolutionRoot, "branch", "-D", "selftest-merge-resolution-side");
  const mergeResolutionHead = git(mergeResolutionRoot, "rev-parse", "HEAD");
  git(mergeResolutionRoot, "update-ref", "refs/remotes/origin/main", mergeResolutionHead);
  removeUnexpectedRemoteRefs(mergeResolutionRoot);
  expectValidator({
    cwd: mergeResolutionRoot,
    args: ["--profile=ongoing"],
    expectedExit: 1,
    diagnostic:
      /Integrated main task-receipt coverage|Integrated product commit lacks a verifiable task receipt/,
    label: "ONGOING_MERGE_RESOLUTION_NEGATIVE",
  });

  const relocatedRoot = join(tempRoot, "relocated-ci");
  execFileSync("git", ["clone", "--local", "--no-hardlinks", fixtureRoot, relocatedRoot], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  git(relocatedRoot, "checkout", "--detach", activationSha);
  git(relocatedRoot, "update-ref", "-d", "refs/heads/main");
  git(relocatedRoot, "update-ref", "refs/remotes/origin/main", activationSha);
  removeUnexpectedRemoteRefs(relocatedRoot);
  expectValidator({
    cwd: relocatedRoot,
    args: ["--profile=ongoing"],
    expectedExit: 0,
    label: "ONGOING_RELOCATED_DETACHED_CI",
  });

  git(fixtureRoot, "checkout", "-b", "codex/selftest-w0-s3", activationSha);
  const activeProgram = readJson(fixtureRoot, programPath);
  const activeLedger = readJson(fixtureRoot, ledgerPath);
  const invalidW0s3 = activeLedger.integration_records.find(
    record => record.record_id === "W0-S3-INTEGRATION-001"
  );
  const blueprint = readJson(fixtureRoot, invalidW0s3.packet_artifact_path);
  const w0s3 = activeProgram.waves[0].slices.find(slice => slice.id === "W0-S3");
  const makeW0Packet = () => {
    const packet = structuredClone(blueprint);
    packet.task_id = "W0-S3-RECOVERY-SELFTEST";
    packet.program_schema_version = activeProgram.schema_version;
    packet.mode = "VERIFICATION";
    packet.wave_id = "WAVE-0";
    packet.slice_id = "W0-S3";
    packet.slice_exit_contract_id = w0s3.exit_contract_id;
    packet.finding_ids = [...w0s3.finding_ids];
    packet.required_guard_ids = [...w0s3.required_guard_ids];
    packet.governance_task = w0s3.governance_task;
    packet.risk_class = w0s3.risk_class;
    packet.root_cause_cluster_id = w0s3.root_cause_cluster_id;
    packet.objective = w0s3.objective;
    packet.non_goals = [...w0s3.non_goals];
    packet.invariant =
      "Fresh current-SHA evidence covers all historical changed and canonical-owner paths.";
    packet.red_contract = [...w0s3.red_contract];
    packet.minimal_fix_contract =
      "Issue a distinct verification receipt and never mutate or re-credit the old receipt.";
    packet.old_path_deletion_contract =
      "The invalid predecessor receipt remains immutable and no-credit.";
    packet.acceptance_criteria = [...packet.acceptance_criteria, w0s3.exit];
    packet.program_activation_sha = activationSha;
    packet.execution_baseline_sha = activationSha;
    packet.expected_parent_main_sha = activationSha;
    packet.checkout = activeProgram.slice_contract.writable_checkout;
    packet.branch = "codex/selftest-w0-s3";
    packet.assigned_agent_id = "selftest-recovery-verifier";
    return freezePacket(packet);
  };
  const validPacketPath = writePacket("w0-s3-recovery-valid", makeW0Packet());
  const scopedArgs = [
    "--profile=ongoing",
    "--slice=W0-S3",
    `--task-packet=${validPacketPath}`,
    `--execution-baseline=${activationSha}`,
  ];
  expectValidator({
    cwd: fixtureRoot,
    args: scopedArgs,
    expectedExit: 0,
    label: "PACKET_VALID",
  });

  const narrowedRecoveryOwnerPacket = makeW0Packet();
  narrowedRecoveryOwnerPacket.canonical_owner = ["openlife-core/src/mcp_audit.rs"];
  const narrowedRecoveryOwnerPacketPath = writePacket(
    "w0-s3-recovery-narrowed-owner",
    freezePacket(narrowedRecoveryOwnerPacket)
  );
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W0-S3",
      `--task-packet=${narrowedRecoveryOwnerPacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /does not reproduce the frozen predecessor packet contract/,
    label: "PACKET_RECOVERY_NARROWED_OWNER_NEGATIVE",
  });

  const missingHistoricalRedPacket = makeW0Packet();
  missingHistoricalRedPacket.red_contract = missingHistoricalRedPacket.red_contract.filter(
    redId => redId !== "W0-NATIVE-DATA-DIR-DOES-NOT-ISOLATE-KEYCHAIN"
  );
  const missingHistoricalRedPacketPath = writePacket(
    "w0-s3-recovery-missing-historical-red",
    freezePacket(missingHistoricalRedPacket)
  );
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W0-S3",
      `--task-packet=${missingHistoricalRedPacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /does not reproduce the frozen predecessor packet contract/,
    label: "PACKET_RECOVERY_MISSING_HISTORICAL_RED_NEGATIVE",
  });

  const conditionalRecoveryPacket = makeW0Packet();
  conditionalRecoveryPacket.mode = "VERIFICATION_THEN_CONDITIONAL_IMPLEMENTATION";
  const conditionalRecoveryPacketPath = writePacket(
    "w0-s3-recovery-conditional-negative",
    freezePacket(conditionalRecoveryPacket)
  );
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W0-S3",
      `--task-packet=${conditionalRecoveryPacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /cannot bypass unresolved receipt recovery/,
    label: "PACKET_RECOVERY_CONDITIONAL_BYPASS_NEGATIVE",
  });

  const rolePacket = makeW0Packet();
  rolePacket.assigned_agent_id = rolePacket.packet_freeze_review.integrator_id;
  const rolePacketPath = writePacket("w0-s2-role-negative", freezePacket(rolePacket));
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W0-S3",
      `--task-packet=${rolePacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /path, baseline, checkout, branch or activation boundary/,
    label: "PACKET_ROLE_NEGATIVE",
  });

  const unsafePacket = makeW0Packet();
  unsafePacket.verification_commands.push("cargo test | tee /tmp/fake-pass");
  const unsafePacketPath = writePacket("w0-s2-command-negative", freezePacket(unsafePacket));
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W0-S3",
      `--task-packet=${unsafePacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /unsafe verification command/,
    label: "PACKET_COMMAND_NEGATIVE",
  });

  const linePacket = makeW0Packet();
  linePacket.source_map[0] = "openlife-core/src/tasks.rs:999999";
  const linePacketPath = writePacket("w0-s2-line-negative", freezePacket(linePacket));
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W0-S3",
      `--task-packet=${linePacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /does not reproduce the frozen predecessor packet contract/,
    label: "PACKET_SOURCE_TAMPER_NEGATIVE",
  });

  const predecessorPacket = makeW0Packet();
  const w0s4 = activeProgram.waves[0].slices.find(slice => slice.id === "W0-S4");
  predecessorPacket.task_id = "W0-S4-SELFTEST";
  predecessorPacket.slice_id = "W0-S4";
  predecessorPacket.slice_exit_contract_id = w0s4.exit_contract_id;
  predecessorPacket.finding_ids = [];
  predecessorPacket.required_guard_ids = [...w0s4.required_guard_ids];
  predecessorPacket.governance_task = true;
  predecessorPacket.risk_class = w0s4.risk_class;
  predecessorPacket.review_contract.risk_class = w0s4.risk_class;
  predecessorPacket.root_cause_cluster_id = w0s4.root_cause_cluster_id;
  predecessorPacket.objective = w0s4.objective;
  predecessorPacket.non_goals = [...w0s4.non_goals];
  predecessorPacket.invariant = "W0-S4 cannot proceed until W0-S3 has fresh effective credit.";
  predecessorPacket.canonical_owner = [ledgerPath];
  predecessorPacket.source_map = [ledgerPath, programPath];
  predecessorPacket.allowed_touched_paths = [ledgerPath, programPath];
  predecessorPacket.forbidden_touched_paths = ["openlife-core/**", "src-tauri/**", "frontend/**"];
  predecessorPacket.red_contract = [...w0s4.red_contract];
  predecessorPacket.acceptance_criteria = [w0s4.exit];
  const predecessorPacketPath = writePacket(
    "w0-s4-before-recovery-negative",
    freezePacket(predecessorPacket)
  );
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W0-S4",
      `--task-packet=${predecessorPacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /cannot bypass unresolved receipt recovery/,
    label: "PREDECESSOR_NEGATIVE",
  });

  const w5Packet = makeW0Packet();
  w5Packet.task_id = "W5-SELFTEST";
  w5Packet.wave_id = "WAVE-5";
  w5Packet.slice_id = "W5-SELFTEST";
  w5Packet.slice_exit_contract_id = "GENERIC_FINDING_BOUND";
  const w5PacketPath = writePacket("w5-dispatch-negative", freezePacket(w5Packet));
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W5-SELFTEST",
      `--task-packet=${w5PacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /Program\/Wave\/slice binding is invalid/,
    label: "W5_DISPATCH_NEGATIVE",
  });

  if (
    observedScenarioLabels.length !== expectedScenarioLabels.length ||
    observedScenarioLabels.some((label, index) => label !== expectedScenarioLabels[index])
  ) {
    throw new Error(
      `Self-test scenario inventory drifted: ${JSON.stringify(observedScenarioLabels)}`
    );
  }
  process.stdout.write(
    [
      "Current Development Program validator self-test: PASS",
      `scenarios=${observedScenarioLabels.length}`,
      `source_head_sha=${sourceHeadSha}`,
      "profiles=draft,activation,ongoing,scoped",
      `scenario_labels_sha256=${createHash("sha256")
        .update(`${observedScenarioLabels.join("\n")}\n`, "utf8")
        .digest("hex")}`,
      "negative=replace-ref,dirty,draft-merge,unreceipted-side,merge-resolution,activation-self-credit,activation-substantive,role,command,source-tamper,predecessor,w5",
    ].join("\n") + "\n"
  );
} finally {
  cleanup();
}
