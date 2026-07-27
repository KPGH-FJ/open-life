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
const predecessorActivationSha = "8b5830dd6339572234fb86021735de901c0a84e4";
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
  "DRAFT_REPLACE_REF_NEGATIVE",
  "DRAFT_DIRTY_NEGATIVE",
  "DRAFT_MERGE_NEGATIVE",
  "ACTIVATION_VALID",
  "ACTIVATION_SELF_CREDIT_NEGATIVE",
  "ACTIVATION_SUBSTANTIVE_NEGATIVE",
  "ONGOING_BOOTSTRAP",
  "ONGOING_RELOCATED_DETACHED_CI",
  "PACKET_VALID",
  "PACKET_ROLE_NEGATIVE",
  "PACKET_COMMAND_NEGATIVE",
  "PACKET_BLUEPRINT_TAMPER_NEGATIVE",
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

try {
  if (git(sourceRoot, "status", "--porcelain=v1", "--untracked-files=all") !== "") {
    throw new Error(
      "Validator self-test requires a clean source checkout so every fixture byte is bound to HEAD"
    );
  }
  const sourceHeadSha = git(sourceRoot, "rev-parse", "HEAD");
  execFileSync("git", ["clone", "--local", "--no-hardlinks", sourceRoot, fixtureRoot], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  git(fixtureRoot, "config", "user.name", "OpenLife Validator Selftest");
  git(fixtureRoot, "config", "user.email", "openlife-validator-selftest@example.invalid");
  git(fixtureRoot, "branch", "-f", "main", predecessorActivationSha);
  git(fixtureRoot, "checkout", "-B", "codex/current-development-program", predecessorActivationSha);
  git(fixtureRoot, "update-ref", "refs/remotes/origin/main", predecessorActivationSha);
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
  git(fixtureRoot, "replace", draftSha, predecessorActivationSha);
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

  git(fixtureRoot, "checkout", "-B", "selftest-merge-side", predecessorActivationSha);
  git(fixtureRoot, "commit", "--allow-empty", "-m", "test: synthetic merge side");
  git(fixtureRoot, "checkout", "-B", "codex/current-development-program", predecessorActivationSha);
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

  git(fixtureRoot, "reset", "--hard", predecessorActivationSha);
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

  git(fixtureRoot, "checkout", "-b", "codex/selftest-w0-s1", activationSha);
  const activeProgram = readJson(fixtureRoot, programPath);
  const blueprint = activeProgram.waves[0].slices[0].task_packet_blueprint;
  const makeW0Packet = () => {
    const packet = structuredClone(blueprint);
    packet.program_activation_sha = activationSha;
    packet.execution_baseline_sha = activationSha;
    packet.expected_parent_main_sha = activationSha;
    packet.branch = "codex/selftest-w0-s1";
    packet.assigned_agent_id = "selftest-implementer";
    return freezePacket(packet);
  };
  const validPacketPath = writePacket("w0-s1-valid", makeW0Packet());
  const scopedArgs = [
    "--profile=ongoing",
    "--slice=W0-S1",
    `--task-packet=${validPacketPath}`,
    `--execution-baseline=${activationSha}`,
  ];
  expectValidator({
    cwd: fixtureRoot,
    args: scopedArgs,
    expectedExit: 0,
    label: "PACKET_VALID",
  });

  const rolePacket = makeW0Packet();
  rolePacket.assigned_agent_id = rolePacket.packet_freeze_review.integrator_id;
  const rolePacketPath = writePacket("w0-s1-role-negative", freezePacket(rolePacket));
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W0-S1",
      `--task-packet=${rolePacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /path, baseline, checkout, branch or activation boundary/,
    label: "PACKET_ROLE_NEGATIVE",
  });

  const unsafePacket = makeW0Packet();
  unsafePacket.verification_commands.push("cargo test | tee /tmp/fake-pass");
  const unsafePacketPath = writePacket("w0-s1-command-negative", freezePacket(unsafePacket));
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W0-S1",
      `--task-packet=${unsafePacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /unsafe verification command/,
    label: "PACKET_COMMAND_NEGATIVE",
  });

  const linePacket = makeW0Packet();
  linePacket.source_map[0] = ".github/workflows/ci.yml:999999";
  const linePacketPath = writePacket("w0-s1-line-negative", freezePacket(linePacket));
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W0-S1",
      `--task-packet=${linePacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /diverges from the approved slice blueprint/,
    label: "PACKET_BLUEPRINT_TAMPER_NEGATIVE",
  });

  const predecessorPacket = makeW0Packet();
  predecessorPacket.task_id = "W0-S2-SELFTEST";
  predecessorPacket.slice_id = "W0-S2";
  predecessorPacket.slice_exit_contract_id = "W0-S2-LEASE-DETERMINISM";
  const predecessorPacketPath = writePacket(
    "w0-s2-predecessor-negative",
    freezePacket(predecessorPacket)
  );
  expectValidator({
    cwd: fixtureRoot,
    args: [
      "--profile=ongoing",
      "--slice=W0-S2",
      `--task-packet=${predecessorPacketPath}`,
      `--execution-baseline=${activationSha}`,
    ],
    expectedExit: 1,
    diagnostic: /predecessor slice is not integrated: W0-S1/,
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
      "negative=replace-ref,dirty,merge,activation-self-credit,activation-substantive,role,command,blueprint-tamper,predecessor,w5",
    ].join("\n") + "\n"
  );
} finally {
  cleanup();
}
