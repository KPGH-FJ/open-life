#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  copyFileSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync, spawnSync } from "node:child_process";

const sourceRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const fixturePrefix = join(realpathSync(tmpdir()), "openlife-program-validator-test-");
const tempRoot = mkdtempSync(fixturePrefix);
const fixtureRoot = join(tempRoot, "fixture");
const baselineSha = "de158ce53018c9c649f7dc0dcb3bdd8271ed4977";
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
  });
const expectValidator = ({ cwd, args, expectedExit, diagnostic = null, label }) => {
  const result = validator(cwd, args);
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  if (result.status !== expectedExit || (diagnostic && !diagnostic.test(output))) {
    throw new Error(
      `${label}: expected exit ${expectedExit}` +
        `${diagnostic ? ` and ${diagnostic}` : ""}, got ${result.status}\n${output}`
    );
  }
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
  execFileSync("git", ["clone", "--local", "--no-hardlinks", sourceRoot, fixtureRoot], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  git(fixtureRoot, "config", "user.name", "OpenLife Validator Selftest");
  git(fixtureRoot, "config", "user.email", "openlife-validator-selftest@example.invalid");
  git(fixtureRoot, "branch", "-f", "main", baselineSha);
  git(fixtureRoot, "checkout", "-B", "codex/current-development-program", baselineSha);
  git(fixtureRoot, "update-ref", "refs/remotes/origin/main", baselineSha);
  removeUnexpectedRemoteRefs(fixtureRoot);

  for (const path of overlayPaths) {
    mkdirSync(dirname(join(fixtureRoot, path)), { recursive: true });
    copyFileSync(join(sourceRoot, path), join(fixtureRoot, path));
  }
  const draftProgram = readJson(fixtureRoot, programPath);
  draftProgram.slice_contract.writable_checkout = fixtureRoot;
  draftProgram.waves[0].slices[0].task_packet_blueprint.checkout = fixtureRoot;
  writeJson(fixtureRoot, programPath, draftProgram);
  const draftSha = commitAll(fixtureRoot, "test: synthetic Program draft");

  expectValidator({
    cwd: fixtureRoot,
    args: ["--profile=draft"],
    expectedExit: 0,
    label: "DRAFT_VALID",
  });

  const activationProgram = readJson(fixtureRoot, programPath);
  const activationLedger = readJson(fixtureRoot, ledgerPath);
  activationProgram.status = "APPROVED_FOR_EXECUTION";
  activationProgram.execution_authorized = true;
  Object.assign(activationProgram.program_approval, {
    status: "APPROVED_BY_USER",
    execution_authority_granted: true,
    approved_program_schema_version: activationProgram.schema_version,
    approved_draft_commit_sha: draftSha,
    approved_by: "USER",
    approved_at: "2026-07-24T00:00:00.000Z",
    approval_record: "selftest:user-approved-exact-draft",
  });
  Object.assign(activationProgram.program_approval.independent_challenge, {
    status: "PASS",
    reviewed_head_sha: draftSha,
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
      subject_sha: draftSha,
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
      subject_sha: draftSha,
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
    {
      subject_sha: draftSha,
      artifact_or_record: "selftest:validator-mutation-suite",
      record_id: "SELFTEST-ACTIVATION-VALIDATOR",
      scope_id: "G-PROGRAM-ACTIVATION",
      scope_paths: [validatorPath, "scripts/test-current-development-program-validator.mjs"],
      guard_ids: [],
      dimensions: ["VALIDATOR_SELF_TEST"],
      outcome: "PASS",
      limitations: [],
      producer_id: "selftest-validator-run",
      reviewer_id: "selftest-validator-run-reviewer",
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
  git(activationNegativeRoot, "checkout", "-B", "codex/current-development-program", draftSha);
  git(activationNegativeRoot, "branch", "-f", "main", baselineSha);
  git(activationNegativeRoot, "update-ref", "refs/remotes/origin/main", baselineSha);
  removeUnexpectedRemoteRefs(activationNegativeRoot);
  const mutatedActivationProgram = JSON.parse(
    git(fixtureRoot, "show", `${activationSha}:${programPath}`)
  );
  const activationLedgerSnapshot = JSON.parse(
    git(fixtureRoot, "show", `${activationSha}:${ledgerPath}`)
  );
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

  process.stdout.write(
    [
      "Current Development Program validator self-test: PASS",
      "scenarios=11",
      "profiles=draft,activation,ongoing,scoped",
      "negative=activation-substantive,role,command,blueprint-tamper,predecessor,w5",
    ].join("\n") + "\n"
  );
} finally {
  if (tempRoot.startsWith(fixturePrefix)) {
    rmSync(tempRoot, { recursive: true, force: true });
  }
}
