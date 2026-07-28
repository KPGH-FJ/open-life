#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, join } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const programPath = "plans/openlife_current_development_program.json";
const programMarkdownPath = "plans/openlife_current_development_program.md";
const ledgerPath = "plans/openlife_problem_ledger.json";
const CURRENT_SCHEMA_VERSION = "1.0.3";
const CURRENT_PROGRAM_ID = "openlife-current-development-program-v1.0.3-receipt-recovery-20260728";
const CURRENT_LEDGER_ID = "openlife-current-problem-ledger-v1.0.3-receipt-recovery-20260728";
const PREDECESSOR_PROGRAM_ID =
  "openlife-current-development-program-v1.0.2-validator-recovery-20260727";
const PREDECESSOR_SCHEMA_VERSION = "1.0.2";
const PREDECESSOR_DRAFT_SHA = "2fd9df02e906f438bb4858422751f7d0cd1d4030";
const PREDECESSOR_ACTIVATION_SHA = "3d17c88b5b81b646cd23c8f2b185eb505ea0dba6";
const PREDECESSOR_HANDOFF_SHA = "8a607bb4f9f392f573c98fa74e8c575d6c2c014d";
const INVALID_W0_S3_RECORD_ID = "W0-S3-INTEGRATION-001";
const INVALID_W0_S3_RECORD_SHA256 =
  "cea939908d35b302d131605e6cd6284e59bc21a905be129a7b95a183c628ef0d";
const RETAINED_EFFECTIVE_RECEIPTS = [
  {
    record_id: "W0-S1-INTEGRATION-001",
    record_canonical_sha256: "85aae971d82c2d20ef9080b61bc09fac869a1217b08e517904b5aeaa9be11363",
  },
  {
    record_id: "W0-S2-INTEGRATION-001",
    record_canonical_sha256: "dbb42bbdbb8bf8ca0596d86042a58c314f8e7296ac63c063ea3b2d3e0a108026",
  },
];
const REVIEW_BASELINE_SHA = "de158ce53018c9c649f7dc0dcb3bdd8271ed4977";
const REVIEW_BASELINE_TREE = "3aa4d4d793ca7a8b687be9e6f21515296db63dff";
const BASELINE_CARD_HASH = "f22a107dd933a38700aa38fb9aa98764a276f50bc006e79740d8d39bca4c6627";
const BASELINE_FACT_HASH = "17cec370fd46971d36a6c3e73ab1a4e53d3f3689749f16842f6ff4048a22d914";
const GUARD_CATALOG_HASH = "838dad6e35bfd1912c04d78299534829a6576d16a943ac9c13ff40945a146670";
const EXPECTED_ABSENT_PATH_HASH =
  "8730120e829ba1a1a74240d4a09afbf831a1b4e376ecbfa512c85216e38963b4";
const SUCCESSOR_DRAFT_PATHS = [
  "plans/openlife_current_development_program.json",
  "plans/openlife_current_development_program.md",
  "plans/openlife_problem_ledger.json",
  "scripts/test-current-development-program-validator.mjs",
  "scripts/validate-current-development-program.mjs",
];
const gitEnvironment = { ...process.env, GIT_NO_REPLACE_OBJECTS: "1" };

const fail = message => {
  throw new Error(message);
};
const assert = (condition, message) => {
  if (!condition) fail(message);
};
const readText = path => readFileSync(join(repositoryRoot, path), "utf8");
const readJson = path => JSON.parse(readText(path));
const readJsonInput = path =>
  JSON.parse(readFileSync(isAbsolute(path) ? path : join(repositoryRoot, path), "utf8"));
const git = (...args) =>
  execFileSync("git", args, {
    cwd: repositoryRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    env: gitEnvironment,
  }).trim();
const gitRaw = (...args) =>
  execFileSync("git", args, {
    cwd: repositoryRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    env: gitEnvironment,
  });
const gitNul = (...args) =>
  execFileSync("git", args, {
    cwd: repositoryRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    env: gitEnvironment,
  })
    .split("\0")
    .filter(Boolean);
const canGit = (...args) => {
  try {
    return git(...args);
  } catch {
    return null;
  }
};
const gitPath = relativePath => {
  const resolved = git("rev-parse", "--git-path", relativePath);
  return isAbsolute(resolved) ? resolved : join(repositoryRoot, resolved);
};
assert(
  git("for-each-ref", "--format=%(refname)", "refs/replace") === "",
  "Git replacement refs are forbidden during Program validation"
);
assert(!existsSync(gitPath("info/grafts")), "Git grafts are forbidden during Program validation");
const validationHeadSha = git("rev-parse", "HEAD");
const validationBranch = git("branch", "--show-current");
const readTextAtCommit = (sha, path) => gitRaw("show", `${sha}:${path}`);
const readJsonAtCommit = (sha, path) => JSON.parse(readTextAtCommit(sha, path));
const isSha = value => typeof value === "string" && /^[0-9a-f]{40}$/.test(value);
const isCommitAncestorOfHead = value =>
  isSha(value) &&
  canGit("cat-file", "-t", value) === "commit" &&
  canGit("merge-base", "--is-ancestor", value, validationHeadSha) !== null;
const clone = value => JSON.parse(JSON.stringify(value));
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
const regexEscape = value => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const newlineIdDigest = values =>
  createHash("sha256")
    .update(`${[...values].sort().join("\n")}\n`, "utf8")
    .digest("hex");
const countBy = (values, selector) =>
  values.reduce((counts, value) => {
    const key = typeof selector === "function" ? selector(value) : value[selector];
    counts[key] = (counts[key] ?? 0) + 1;
    return counts;
  }, {});
const sortedObject = value =>
  Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b)));
const sameJson = (actual, expected, label) =>
  assert(
    JSON.stringify(sortedObject(actual)) === JSON.stringify(sortedObject(expected)),
    `${label}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`
  );
const sameSet = (actual, expected, label) => {
  const normalizedActual = [...new Set(actual)].sort();
  const normalizedExpected = [...new Set(expected)].sort();
  assert(
    JSON.stringify(normalizedActual) === JSON.stringify(normalizedExpected),
    `${label}: expected ${JSON.stringify(normalizedExpected)}, got ${JSON.stringify(normalizedActual)}`
  );
};
const parseArgs = argv => {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) continue;
    const equals = token.indexOf("=");
    if (equals >= 0) {
      parsed[token.slice(2, equals)] = token.slice(equals + 1);
    } else {
      const key = token.slice(2);
      const next = argv[index + 1];
      if (next && !next.startsWith("--")) {
        parsed[key] = next;
        index += 1;
      } else {
        parsed[key] = true;
      }
    }
  }
  return parsed;
};
const splitRef = ref => {
  const match = ref.match(/^(.*):(\d+)(?:-(\d+))?$/);
  if (!match) return { path: ref, start: null, end: null };
  return {
    path: match[1],
    start: Number(match[2]),
    end: Number(match[3] ?? match[2]),
  };
};
const pathAtCommitExists = (sha, path) => canGit("cat-file", "-e", `${sha}:${path}`) !== null;
const validateRefAtCommit = (sha, ref, label) => {
  const parsed = splitRef(ref);
  assert(
    pathAtCommitExists(sha, parsed.path),
    `${label} path does not exist at ${sha}: ${parsed.path}`
  );
  if (parsed.start !== null) {
    const source = readTextAtCommit(sha, parsed.path);
    const lineCount =
      source === ""
        ? 0
        : source.endsWith("\n")
          ? source.slice(0, -1).split("\n").length
          : source.split("\n").length;
    assert(
      parsed.start >= 1 && parsed.end >= parsed.start && parsed.end <= lineCount,
      `${label} line range is outside ${parsed.path} (${lineCount} lines): ${ref}`
    );
  }
};
const globRegex = pattern => {
  const escaped = pattern
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*\*/g, "\u0000")
    .replace(/\*/g, "[^/]*")
    .replace(/\u0000/g, ".*");
  return new RegExp(`^${escaped}$`);
};
const pathMatches = (path, patterns) => patterns.some(pattern => globRegex(pattern).test(path));
const wildcardStaticPrefix = pattern => {
  const wildcardIndex = pattern.search(/[*?[{]/);
  const rawPrefix = wildcardIndex === -1 ? pattern : pattern.slice(0, wildcardIndex);
  return rawPrefix.replace(/\/+$/, "");
};
const assertSafePathPattern = (pattern, label) => {
  assert(
    typeof pattern === "string" &&
      pattern.trim() === pattern &&
      pattern.length > 0 &&
      !pattern.startsWith("/") &&
      !pattern.includes("\\") &&
      !/[?[{\]}]/.test(pattern) &&
      !pattern.split("/").some(segment => segment === "." || segment === "..") &&
      !(pattern.match(/\*+/g) ?? []).some(run => run.length > 2),
    `${label} contains an unsafe path pattern: ${pattern}`
  );
};
const pathIsStateOnly = (path, integrationContract) =>
  integrationContract.state_only_paths.includes(path) ||
  integrationContract.state_only_path_prefixes.some(prefix => path.startsWith(prefix));
const scopedDiffChurn = (executionBaseline, taskUntrackedPaths) => {
  const numstat = execFileSync(
    "git",
    ["diff", "--numstat", "--no-renames", executionBaseline, "--"],
    {
      cwd: repositoryRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }
  );
  let churnLines = 0;
  for (const line of numstat.split("\n").filter(Boolean)) {
    const [added, deleted] = line.split("\t");
    assert(
      /^\d+$/.test(added) && /^\d+$/.test(deleted),
      "Binary or unreadable tracked diff requires a pre-reviewed Program amendment"
    );
    churnLines += Number(added) + Number(deleted);
  }
  for (const path of taskUntrackedPaths) {
    const contents = readFileSync(join(repositoryRoot, path));
    assert(
      !contents.includes(0),
      `Binary untracked file requires a pre-reviewed Program amendment: ${path}`
    );
    if (contents.length > 0) {
      churnLines +=
        contents.reduce((count, byte) => count + (byte === 10 ? 1 : 0), 0) +
        (contents.at(-1) === 10 ? 0 : 1);
    }
  }
  return churnLines;
};
const committedRangeChurn = (baseSha, headSha) => {
  const numstat = execFileSync(
    "git",
    ["diff", "--numstat", "--no-renames", baseSha, headSha, "--"],
    {
      cwd: repositoryRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }
  );
  return numstat
    .split("\n")
    .filter(Boolean)
    .reduce((total, line) => {
      const [added, deleted] = line.split("\t");
      assert(
        /^\d+$/.test(added) && /^\d+$/.test(deleted),
        "Binary or unreadable integrated diff is outside this Program"
      );
      return total + Number(added) + Number(deleted);
    }, 0);
};
const changedPathsFrom = sha => gitNul("diff", "--name-only", "-z", "--no-renames", sha, "--");
const changedPathsBetween = (baseSha, headSha) =>
  gitNul("diff", "--name-only", "-z", "--no-renames", baseSha, headSha, "--");
const uniquePaths = (...pathSets) => [...new Set(pathSets.flat())].sort();
const commitNovelPaths = commitSha => {
  const parents = git("rev-list", "--parents", "-n", "1", commitSha).split(" ").slice(1);
  if (parents.length === 0) return [];
  const changedByParent = parents.map(parent => new Set(changedPathsBetween(parent, commitSha)));
  return [...changedByParent[0]]
    .filter(path => changedByParent.every(paths => paths.has(path)))
    .sort();
};
for (const [sha, label] of [[PREDECESSOR_HANDOFF_SHA, "predecessor handoff merge"]]) {
  sameSet(commitNovelPaths(sha), [], `${label} novel-path replay`);
}
const enforcePathScope = ({
  paths,
  allowedPaths,
  forbiddenPaths = [],
  expectedAbsentPaths = [],
  label,
}) => {
  for (const path of paths) {
    assert(pathMatches(path, allowedPaths), `${label} does not allow ${path}`);
    assert(!pathMatches(path, forbiddenPaths), `${label} explicitly forbids ${path}`);
  }
  for (const path of expectedAbsentPaths) {
    assert(
      !existsSync(join(repositoryRoot, path)),
      `${label} recreated expected-absent path ${path}`
    );
  }
};
const activationSubstantiveProgram = value => {
  const normalized = clone(value);
  const stateMarker = "__ACTIVATION_STATE__";
  normalized.status = stateMarker;
  normalized.execution_authorized = stateMarker;
  for (const field of [
    "status",
    "execution_authority_granted",
    "approved_program_schema_version",
    "approved_draft_commit_sha",
    "approved_by",
    "approved_at",
    "approval_record",
  ]) {
    normalized.program_approval[field] = stateMarker;
  }
  for (const field of [
    "status",
    "reviewed_head_sha",
    "reviewer_ids",
    "outcome",
    "artifact_or_record",
    "blocking_findings",
  ]) {
    normalized.program_approval.independent_challenge[field] = stateMarker;
  }
  normalized.program_activation.status = stateMarker;
  const activationGate = normalized.gates.find(gate => gate.id === "G-PROGRAM-ACTIVATION");
  activationGate.status = stateMarker;
  activationGate.evidence_records = stateMarker;
  normalized.waves.find(wave => wave.id === "WAVE-0").status = stateMarker;
  return normalized;
};
const activationSubstantiveLedger = value => {
  const normalized = clone(value);
  normalized.status = "__ACTIVATION_STATE__";
  normalized.authority.execution_authorized = "__ACTIVATION_STATE__";
  return normalized;
};
const livingSubstantiveProgram = value => {
  const normalized = activationSubstantiveProgram(value);
  for (const gate of normalized.gates) {
    gate.status = "__LIVING_GATE_STATE__";
    gate.evidence_records = "__LIVING_GATE_STATE__";
  }
  for (const wave of normalized.waves) {
    wave.status = "__LIVING_WAVE_STATE__";
  }
  const bounded = normalized.feature_development_gates.bounded_feature_eligibility;
  bounded.status = "__LIVING_FEATURE_STATE__";
  bounded.eligible = "__LIVING_FEATURE_STATE__";
  bounded.eligible_domains = "__LIVING_FEATURE_STATE__";
  bounded.evidence_records = "__LIVING_FEATURE_STATE__";
  const normal = normalized.feature_development_gates.normal_feature_development_reopen;
  normal.status = "__LIVING_FEATURE_STATE__";
  normal.reopened = "__LIVING_FEATURE_STATE__";
  normal.evidence_records = "__LIVING_FEATURE_STATE__";
  return normalized;
};
const initialFactProjection = card => ({
  card_id: card.card_id,
  title: card.title,
  origin: card.origin,
  root_cause_cluster_id: card.root_cause_cluster_id,
  guard_ids: card.guard_ids,
  severity_at_discovery: card.severity_at_discovery,
  initial_current_severity: card.current_severity,
  initial_disposition: card.current_disposition,
  initial_canonical_owner: card.canonical_owner,
  initial_assigned_wave: card.initial_assigned_wave,
  initial_source_evidence: card.source_evidence,
  initial_behavior_evidence: card.behavior_evidence,
});
const stableFactProjection = card => ({
  card_id: card.card_id,
  title: card.title,
  origin: card.origin,
  root_cause_cluster_id: card.root_cause_cluster_id,
  guard_ids: card.guard_ids,
  severity_at_discovery: card.severity_at_discovery,
  initial_assigned_wave: card.initial_assigned_wave,
});
const stableBaselineFactProjection = fact => ({
  card_id: fact.card_id,
  title: fact.title,
  origin: fact.origin,
  root_cause_cluster_id: fact.root_cause_cluster_id,
  guard_ids: fact.guard_ids,
  severity_at_discovery: fact.severity_at_discovery,
  initial_assigned_wave: fact.initial_assigned_wave,
});
const newCardStableFactProjection = card => ({
  card_id: card.card_id,
  title: card.title,
  origin: card.origin,
  root_cause_cluster_id: card.root_cause_cluster_id,
  guard_ids: card.guard_ids,
  severity_at_discovery: card.severity_at_discovery,
  assigned_wave: card.assigned_wave,
  initial_assigned_wave: card.initial_assigned_wave,
  prerequisite_slice_ids: card.prerequisite_slice_ids,
  exact_next_proof: card.exact_next_proof,
});
const livingCardStateProjection = card => ({
  current_severity: card.current_severity,
  current_disposition: card.current_disposition,
  canonical_owner: card.canonical_owner,
  source_evidence: card.source_evidence,
  behavior_evidence: card.behavior_evidence,
  feature_reopen: card.feature_reopen,
  closure_credit: card.closure_credit,
  closure_requirements: card.closure_requirements,
  closure_record: card.closure_record,
  current_wave: card.current_wave,
});
const collectExpectedAbsentPaths = (value, paths = []) => {
  if (Array.isArray(value)) {
    for (const item of value) collectExpectedAbsentPaths(item, paths);
  } else if (value && typeof value === "object") {
    if (value.expected_absent === true && typeof value.former_path === "string") {
      paths.push(value.former_path);
    }
    for (const item of Object.values(value)) {
      collectExpectedAbsentPaths(item, paths);
    }
  }
  return paths;
};
const validateCreditedEvidenceRecords = ({
  records,
  requiredDimensions,
  requiredGuardIds = [],
  expectedScopeId = null,
  requireCurrentFreshness = true,
  label,
}) => {
  assert(Array.isArray(records), `${label} evidence records are not an array`);
  assert(records.length > 0, `${label} has no evidence record`);
  const evidenceRecordIds = new Set();
  for (const record of records) {
    for (const field of program.gate_evidence_contract.required_record_fields) {
      assert(field in record, `${label} evidence record misses ${field}`);
    }
    assert(
      typeof record.record_id === "string" &&
        record.record_id.trim() &&
        !evidenceRecordIds.has(record.record_id) &&
        typeof record.scope_id === "string" &&
        record.scope_id.trim() &&
        (!expectedScopeId || record.scope_id === expectedScopeId) &&
        Array.isArray(record.scope_paths) &&
        record.scope_paths.length > 0 &&
        record.scope_paths.every(path => typeof path === "string" && path.trim()) &&
        Array.isArray(record.guard_ids) &&
        record.guard_ids.every(guardId => typeof guardId === "string" && guardId.trim()),
      `${label} evidence identity/scope is invalid`
    );
    evidenceRecordIds.add(record.record_id);
    assert(
      isCommitAncestorOfHead(record.subject_sha),
      `${label} evidence subject is not a current-chain commit`
    );
    assert(
      typeof record.artifact_or_record === "string" && record.artifact_or_record.trim(),
      `${label} evidence artifact is empty`
    );
    assert(
      Array.isArray(record.dimensions) && record.dimensions.length > 0,
      `${label} evidence dimensions are empty`
    );
    assert(
      ["PASS", "FAIL", "BLOCKED", "UNKNOWN"].includes(record.outcome),
      `${label} evidence outcome is invalid`
    );
    assert(Array.isArray(record.limitations), `${label} evidence limitations must be an array`);
    assert(typeof record.credit_allowed === "boolean", `${label} evidence credit flag is invalid`);
    assert(typeof record.active === "boolean", `${label} evidence active flag is invalid`);
    assert(
      typeof record.producer_id === "string" &&
        record.producer_id.trim() &&
        typeof record.reviewer_id === "string" &&
        record.reviewer_id.trim() &&
        record.producer_id !== record.reviewer_id,
      `${label} evidence lacks an independent reviewer`
    );
    for (const pattern of record.scope_paths) {
      assertSafePathPattern(pattern, `${label} evidence scope`);
      assert(!pattern.includes("*"), `${label} evidence scope must be a literal path: ${pattern}`);
    }
    assert(
      record.guard_ids.every(guardId => guardIds.has(guardId)),
      `${label} evidence references an unknown guard`
    );
    if (
      requireCurrentFreshness &&
      record.outcome === "PASS" &&
      record.credit_allowed === true &&
      record.active === true &&
      record.limitations.length === 0
    ) {
      assert(
        evidenceRecordIsFresh(record),
        `${label} evidence is stale after a later scoped integration`
      );
    }
  }
  const creditedDimensions = new Set(
    records
      .filter(
        record =>
          record.outcome === "PASS" &&
          record.credit_allowed === true &&
          record.active === true &&
          record.limitations.length === 0
      )
      .flatMap(record => record.dimensions)
  );
  for (const dimension of requiredDimensions) {
    assert(creditedDimensions.has(dimension), `${label} PASS lacks credited ${dimension} evidence`);
    assert(
      records
        .filter(record => record.active === true && record.dimensions.includes(dimension))
        .every(
          record =>
            record.outcome === "PASS" &&
            record.credit_allowed === true &&
            record.limitations.length === 0
        ),
      `${label} PASS conflicts with active ${dimension} failure/limitation`
    );
  }
  const creditedGuardIds = new Set(
    records
      .filter(
        record =>
          record.outcome === "PASS" &&
          record.credit_allowed === true &&
          record.active === true &&
          record.limitations.length === 0
      )
      .flatMap(record => record.guard_ids)
  );
  for (const guardId of requiredGuardIds) {
    assert(creditedGuardIds.has(guardId), `${label} PASS lacks credited ${guardId} scope`);
  }
};

const args = parseArgs(process.argv.slice(2));
const profile = args.profile ?? "draft";
assert(
  ["draft", "activation", "ongoing"].includes(profile),
  `Unknown validation profile: ${profile}`
);

const program = readJson(programPath);
const ledger = readJson(ledgerPath);
const programMarkdown = readText(programMarkdownPath);
let invalidatedReceiptIds = new Set();
const predecessorProgram = readJsonAtCommit(PREDECESSOR_HANDOFF_SHA, programPath);
const predecessorLedger = readJsonAtCommit(PREDECESSOR_HANDOFF_SHA, ledgerPath);
const countTrackedAtCommit = (sha, prefix) =>
  git("ls-tree", "-r", "--name-only", sha, "--", prefix).split("\n").filter(Boolean).length;
const predecessorFacts = {
  program_id: predecessorProgram.program_id,
  schema_version: predecessorProgram.schema_version,
  approved_draft_commit_sha: predecessorProgram.program_approval?.approved_draft_commit_sha,
  activation_commit_sha: PREDECESSOR_ACTIVATION_SHA,
  handoff_main_sha: PREDECESSOR_HANDOFF_SHA,
  closure_credit_true: predecessorLedger.cards.filter(card => card.closure_credit === true).length,
  integration_records: predecessorLedger.integration_records.length,
  tracked_task_packet_artifacts: countTrackedAtCommit(
    PREDECESSOR_HANDOFF_SHA,
    predecessorLedger.integration_contract.packet_archive_prefix
  ),
  tracked_attempt_artifacts: countTrackedAtCommit(
    PREDECESSOR_HANDOFF_SHA,
    predecessorLedger.attempt_contract.attempt_artifact_prefix
  ),
  retained_effective_receipts: RETAINED_EFFECTIVE_RECEIPTS,
};
assert(program.schema_version === CURRENT_SCHEMA_VERSION, "Unexpected Program schema version");
assert(ledger.schema_version === CURRENT_SCHEMA_VERSION, "Unexpected ledger schema version");
assert(program.program_id === CURRENT_PROGRAM_ID, "Unexpected Program ID");
assert(ledger.ledger_id === CURRENT_LEDGER_ID, "Unexpected ledger ID");
assert(
  program.predecessor_program?.program_id === predecessorFacts.program_id &&
    program.predecessor_program.schema_version === predecessorFacts.schema_version &&
    program.predecessor_program.approved_draft_commit_sha ===
      predecessorFacts.approved_draft_commit_sha &&
    program.predecessor_program.activation_commit_sha === predecessorFacts.activation_commit_sha &&
    program.predecessor_program.handoff_main_sha === predecessorFacts.handoff_main_sha &&
    program.predecessor_program.closure_credit_true === predecessorFacts.closure_credit_true &&
    program.predecessor_program.integration_records === predecessorFacts.integration_records &&
    program.predecessor_program.tracked_task_packet_artifacts ===
      predecessorFacts.tracked_task_packet_artifacts &&
    program.predecessor_program.tracked_attempt_artifacts ===
      predecessorFacts.tracked_attempt_artifacts &&
    canonicalDigest(program.predecessor_program.retained_effective_receipts) ===
      canonicalDigest(predecessorFacts.retained_effective_receipts) &&
    predecessorFacts.program_id === PREDECESSOR_PROGRAM_ID &&
    predecessorFacts.schema_version === PREDECESSOR_SCHEMA_VERSION &&
    predecessorFacts.approved_draft_commit_sha === PREDECESSOR_DRAFT_SHA &&
    predecessorFacts.closure_credit_true === 0 &&
    predecessorFacts.integration_records === 3 &&
    predecessorFacts.tracked_task_packet_artifacts === 3 &&
    predecessorFacts.tracked_attempt_artifacts === 3,
  "Successor Program predecessor binding drifted"
);
assert(
  ledger.predecessor_snapshot?.program_id === predecessorFacts.program_id &&
    ledger.predecessor_snapshot.schema_version === predecessorFacts.schema_version &&
    ledger.predecessor_snapshot.activation_commit_sha === predecessorFacts.activation_commit_sha &&
    ledger.predecessor_snapshot.handoff_main_sha === predecessorFacts.handoff_main_sha &&
    ledger.predecessor_snapshot.closure_credit_true === predecessorFacts.closure_credit_true &&
    ledger.predecessor_snapshot.integration_records === predecessorFacts.integration_records &&
    ledger.predecessor_snapshot.tracked_task_packet_artifacts ===
      predecessorFacts.tracked_task_packet_artifacts &&
    ledger.predecessor_snapshot.tracked_attempt_artifacts ===
      predecessorFacts.tracked_attempt_artifacts &&
    canonicalDigest(ledger.predecessor_snapshot.retained_effective_receipts) ===
      canonicalDigest(predecessorFacts.retained_effective_receipts),
  "Successor ledger predecessor binding drifted"
);
for (const field of [
  "integration_records",
  "implementation_attempt_records",
  "architecture_review_records",
]) {
  const predecessorRecords = predecessorLedger[field] ?? [];
  const successorRecords = ledger[field] ?? [];
  assert(
    predecessorRecords.length <= successorRecords.length &&
      predecessorRecords.every(
        (record, index) => canonicalDigest(record) === canonicalDigest(successorRecords[index])
      ),
    `Successor mutated predecessor append-only sequence: ${field}`
  );
}
const corruptedPredecessorFacts = { ...predecessorFacts, closure_credit_true: 1 };
assert(
  canonicalDigest(corruptedPredecessorFacts) !== canonicalDigest(program.predecessor_program),
  "Predecessor lineage primitive accepted a corrupted frozen fact"
);
assert(
  canGit("merge-base", "--is-ancestor", PREDECESSOR_HANDOFF_SHA, validationHeadSha) !== null,
  "Current HEAD does not descend from the predecessor handoff"
);
assert(
  program.authority.subordinate_to_phase7 === true &&
    ledger.authority.subordinate_to_phase7 === true,
  "Program and ledger must remain subordinate to Phase7"
);
assert(
  ledger.authority.execution_authorized === program.execution_authorized,
  "Program and ledger execution-authority state disagree"
);

const baseline = program.review_baseline;
assert(baseline.sha === REVIEW_BASELINE_SHA, "Review baseline SHA drifted");
assert(baseline.tree === REVIEW_BASELINE_TREE, "Review baseline tree drifted");
if (canGit("cat-file", "-t", baseline.sha) !== "commit") {
  fail(
    `Review baseline ${baseline.sha} is unavailable. This gate requires a full-history checkout; fetch the baseline object before retrying.`
  );
}
assert(
  git("rev-parse", `${baseline.sha}^{tree}`) === baseline.tree,
  "Baseline tree does not match baseline commit"
);
assert(
  canGit("merge-base", "--is-ancestor", baseline.sha, "HEAD") !== null,
  "Current HEAD does not descend from the formal review baseline"
);
assert(
  ledger.review_baseline.sha === baseline.sha && ledger.review_baseline.tree === baseline.tree,
  "Program and ledger baseline disagree"
);
let invalidLineReferenceRejected = false;
try {
  validateRefAtCommit(baseline.sha, "AGENTS.md:999999", "validator primitive self-check");
} catch (error) {
  invalidLineReferenceRejected =
    error instanceof Error && /line range is outside/.test(error.message);
}
assert(
  invalidLineReferenceRejected,
  "Validator primitive self-check accepted an impossible source line"
);

const worktreeCount = git("worktree", "list", "--porcelain")
  .split("\n")
  .filter(line => line.startsWith("worktree ")).length;
assert(worktreeCount === 1, `Expected one worktree, found ${worktreeCount}`);
const currentBranch = validationBranch;
const localMainTip = canGit("rev-parse", "--verify", "refs/heads/main");
const originMainTip = canGit("rev-parse", "--verify", "refs/remotes/origin/main");
assert(localMainTip || originMainTip, "No local or origin main ref is available");
if (localMainTip && originMainTip) {
  assert(
    localMainTip === originMainTip,
    "Local main and origin/main diverge; refresh the development baseline"
  );
}
const canonicalMainRef = localMainTip !== null ? "main" : "origin/main";
const canonicalMainTip = git("rev-parse", canonicalMainRef);
assert(
  currentBranch === "" || currentBranch === "main" || currentBranch.startsWith("codex/"),
  `Unexpected current branch ${currentBranch}`
);
if (currentBranch !== "") {
  const localBranches = git("for-each-ref", "--format=%(refname:short)", "refs/heads")
    .split("\n")
    .filter(Boolean);
  assert(
    localBranches.every(branch => branch === "main" || branch === currentBranch),
    `Unexpected local branch set: ${localBranches.join(", ")}`
  );
}
const remoteBranches = git("for-each-ref", "--format=%(refname:short)", "refs/remotes/origin")
  .split("\n")
  .filter(branch => branch && branch !== "origin/HEAD");
const allowedRemoteBranches = new Set(["origin/main"]);
if (currentBranch.startsWith("codex/")) {
  allowedRemoteBranches.add(`origin/${currentBranch}`);
}
assert(
  remoteBranches.every(branch => allowedRemoteBranches.has(branch)),
  `Unexpected remote branch set: ${remoteBranches.join(", ")}`
);

const expectedPrecedence = [
  "AGENTS.md",
  "plans/README.md",
  "plans/openlife_single_system_deletion_manifest.md",
  "plans/openlife_single_system_development_preparation.md",
];
assert(
  JSON.stringify(program.authority.precedence.slice(0, 4)) === JSON.stringify(expectedPrecedence),
  "Program Phase7 authority precedence drifted"
);
sameSet(
  program.authority.validator_surfaces,
  [
    "scripts/validate-current-development-program.mjs",
    "scripts/test-current-development-program-validator.mjs",
  ],
  "Program validator surfaces"
);
for (const historical of program.authority.historical_inputs) {
  assert(historical.execution_authority === false, `${historical.id} regained execution authority`);
}
assert(
  program.program_activation.activation_is_not_a_wave_slice === true &&
    program.program_activation.occurs_before_wave_id === "WAVE-0",
  "Program activation is not a pre-Wave transition"
);

const expectedEvidenceVocabulary = [
  "REPRODUCED",
  "SOURCE-CONFIRMED",
  "HISTORICAL-EVIDENCE",
  "UNKNOWN",
];
const expectedCoverageVocabulary = ["COMPLETE", "PARTIAL", "NONE", "UNKNOWN"];
assert(
  JSON.stringify(program.vocabularies.evidence_status) ===
    JSON.stringify(expectedEvidenceVocabulary),
  "Program evidence vocabulary drifted"
);
assert(
  JSON.stringify(ledger.vocabularies.evidence_status) ===
    JSON.stringify(expectedEvidenceVocabulary),
  "Ledger evidence vocabulary drifted"
);
assert(
  JSON.stringify(ledger.vocabularies.evidence_coverage) ===
    JSON.stringify(expectedCoverageVocabulary),
  "Ledger evidence-coverage vocabulary drifted"
);

const expectedArtifactDigests = {
  "r3-source-review.json": "663f26a94b0fdbf0c622fc8a019e13aacc22d1da8df96a751201ca9a6e14466c",
  "r4-core-tests.json": "15996acd656ddb0c00bbe83ddcdcf6f50b913b9b7d2bea2a2c9f1763f55468d9",
  "r4-flaky-test-diagnosis.json":
    "eff41b5f8cad293f739caad80700bbd48efa164875cfdc34a168dd562ea5ff5a",
  "r4-frontend-ci.json": "8ecd7620c17cb5b09572e78b8cdad82c90fda8168247f2fea30604f13813cf7d",
  "r4-native-settings.json": "191e32698559425c96b3bf0451c00365b6826bf266d0559831c01592208ca176",
  "r4-tauri-behavior.json": "9209891f392c341619ecaf7ca354dbee3df258b4c64b927d192b6c81712e01e3",
  "r4-verification.json": "c236813d50284efbe85ee5e4cbfbc6e4b43e548ea8ec6b8cdfea8ddfd1bbe585",
  "r5-code-quality.json": "4ef897fc22350694924c1210c8499fa0a8ea51553b24ac1ad05c774710424be2",
  "r5-retrospective.json": "da95af97dc86138f17fdd21864c789de580b49c17fd8ae0b7cc406ce4142bad6",
  "r6-development-direction-decision.json":
    "28c1af50f3ecf4eee41a27f7fcbdbba6da4fc2d0a991cc52f0deefab6fa719d6",
  "r7-final-handoff.json": "d38172d3fdc39d209601ff6add75cb46e5deffd2c2ce5c9ed96b7d67a7cc1f88",
};
for (const [label, provenance] of [
  ["Program", program.source_inputs],
  ["ledger", ledger.provenance],
]) {
  const observed = Object.fromEntries(provenance.map(entry => [entry.artifact_name, entry.sha256]));
  sameJson(observed, expectedArtifactDigests, `${label} provenance`);
  assert(
    provenance.every(entry => entry.repository_dependency === false),
    `${label} makes ignored review artifacts a repository dependency`
  );
}

assert(
  canonicalDigest(ledger.guard_catalog) === GUARD_CATALOG_HASH,
  "Tracked CQ guard catalog drifted"
);
sameSet(
  ledger.guard_catalog.map(guard => guard.id),
  Array.from({ length: 16 }, (_, index) => `CQ-G${String(index + 1).padStart(2, "0")}`),
  "CQ guard catalog IDs"
);
const guardIds = new Set(ledger.guard_catalog.map(guard => guard.id));
for (const guard of ledger.guard_catalog) {
  assert(guard.mechanism?.trim(), `${guard.id} has no mechanism`);
  assert(guard.machine_check?.trim(), `${guard.id} has no machine check`);
  assert(guard.clusters?.length > 0, `${guard.id} has no target cluster`);
}
const evidenceRecordIsFresh = record =>
  ledger.integration_records.every(integrationRecord => {
    if (
      integrationRecord.changed_paths.every(path =>
        pathIsStateOnly(path, ledger.integration_contract)
      )
    ) {
      return true;
    }
    if (
      !isSha(integrationRecord.range_head_sha) ||
      integrationRecord.range_head_sha === record.subject_sha ||
      canGit(
        "merge-base",
        "--is-ancestor",
        record.subject_sha,
        integrationRecord.range_head_sha
      ) === null
    ) {
      return true;
    }
    const pathOverlap = integrationRecord.changed_paths.some(path =>
      pathMatches(path, record.scope_paths)
    );
    const guardOverlap = integrationRecord.required_guard_ids.some(guardId =>
      record.guard_ids.includes(guardId)
    );
    return !pathOverlap && !guardOverlap;
  });
const predecessorReceiptLineage = (record, programState) => {
  if (
    record.program_approved_draft_sha === programState.program_approval.approved_draft_commit_sha
  ) {
    return {
      kind: "CURRENT_PROGRAM",
      activation_sha:
        programState.program_activation.status === "ACTIVE"
          ? deriveAndValidateActivationCommit(
              programState.program_approval.approved_draft_commit_sha
            ).activationSha
          : null,
    };
  }
  const declaredPredecessor = programState.predecessor_program;
  if (
    declaredPredecessor &&
    record.program_approved_draft_sha === declaredPredecessor.approved_draft_commit_sha &&
    record.program_activation_sha === declaredPredecessor.activation_commit_sha
  ) {
    return {
      kind: "DECLARED_PREDECESSOR",
      activation_sha: declaredPredecessor.activation_commit_sha,
    };
  }
  const retainedReceipt = declaredPredecessor?.retained_effective_receipts?.find(
    candidate =>
      candidate.record_id === record.record_id &&
      candidate.record_canonical_sha256 === canonicalDigest(record)
  );
  if (retainedReceipt) {
    return {
      kind: "DECLARED_RETAINED_PREDECESSOR",
      activation_sha: record.program_activation_sha,
    };
  }
  return null;
};
const recoveryFrozenPacketFields = [
  "canonical_owner",
  "allowed_touched_paths",
  "source_map",
  "forbidden_touched_paths",
  "expected_absent_paths",
  "verification_commands",
  "required_evidence_dimensions",
  "required_guard_ids",
  "red_contract",
];
const recoveryPacketMatchesFrozenScope = (candidatePacket, invalidatedPacket) =>
  candidatePacket.mode === "VERIFICATION" &&
  recoveryFrozenPacketFields.every(
    field =>
      Array.isArray(candidatePacket[field]) &&
      Array.isArray(invalidatedPacket[field]) &&
      canonicalDigest([...candidatePacket[field]].sort()) ===
        canonicalDigest([...invalidatedPacket[field]].sort())
  );
const recoveryReceiptSatisfiesFrozenScope = ({
  candidate,
  invalidatedTarget,
  programState,
  ledgerState,
  snapshotSha,
}) => {
  if (
    !candidate ||
    !invalidatedTarget ||
    candidate.record_id === invalidatedTarget.record_id ||
    candidate.slice_id !== invalidatedTarget.slice_id ||
    candidate.program_approved_draft_sha !==
      programState.program_approval.approved_draft_commit_sha ||
    candidate.changed_paths.some(path => !pathIsStateOnly(path, ledgerState.integration_contract))
  ) {
    return false;
  }
  try {
    const candidatePacket = readJsonAtCommit(snapshotSha, candidate.packet_artifact_path);
    const invalidatedPacket = readJsonAtCommit(snapshotSha, invalidatedTarget.packet_artifact_path);
    if (
      !recoveryPacketMatchesFrozenScope(candidatePacket, invalidatedPacket) ||
      candidate.task_id === invalidatedTarget.task_id ||
      candidate.packet_sha256 === invalidatedTarget.packet_sha256
    ) {
      return false;
    }
    const creditedScope = new Set(
      candidate.task_evidence_records
        .filter(
          evidence =>
            evidence.outcome === "PASS" &&
            evidence.credit_allowed === true &&
            evidence.active === true &&
            evidence.limitations.length === 0
        )
        .flatMap(evidence => evidence.scope_paths)
    );
    const requiredScope = uniquePaths(
      invalidatedTarget.changed_paths,
      (Array.isArray(invalidatedPacket.canonical_owner)
        ? invalidatedPacket.canonical_owner
        : [invalidatedPacket.canonical_owner]
      ).map(ref => splitRef(ref).path)
    );
    return requiredScope.every(path => creditedScope.has(path));
  } catch {
    return false;
  }
};
assert(
  predecessorReceiptLineage(
    {
      program_approved_draft_sha: "0000000000000000000000000000000000000000",
      program_activation_sha: "1111111111111111111111111111111111111111",
    },
    program
  ) === null,
  "Predecessor lineage primitive accepted an unrelated historical Program"
);

const expectedWaveIds = ["WAVE-0", "WAVE-1", "WAVE-2", "WAVE-3", "WAVE-4", "WAVE-5"];
assert(
  JSON.stringify(program.waves.map(wave => wave.id)) === JSON.stringify(expectedWaveIds),
  "Wave ID/order drifted"
);
for (const [index, wave] of program.waves.entries()) {
  assert(wave.order === index, `${wave.id} order field is inconsistent`);
  const expectedDependencies = index === 0 ? [] : [expectedWaveIds[index - 1]];
  assert(
    JSON.stringify(wave.depends_on_wave_ids) === JSON.stringify(expectedDependencies),
    `${wave.id} dependency is not the fail-closed sequential DAG`
  );
  assert(wave.entry_gate_ids.length > 0, `${wave.id} has no entry gate`);
  assert(wave.exit_gate_ids.length > 0, `${wave.id} has no exit gate`);
  assert(wave.feature_credit, `${wave.id} has no feature-credit boundary`);
  assert(
    wave.card_count_semantics === "INITIAL_BASELINE_ASSIGNMENT",
    `${wave.id} card-count semantics are ambiguous`
  );
  if (index === 0) {
    assert(
      wave.refinement_level === "CURRENT_SLICE_PREPARATION_REQUIRED",
      "WAVE-0 does not identify the current packet-preparation boundary"
    );
  } else {
    assert(
      wave.refinement_level === "OUTCOME_ONLY_REQUIRES_WAVE_PREPARATION",
      `${wave.id} is prematurely over-specified`
    );
    assert(wave.slices.length === 0, `${wave.id} contains speculative slices`);
  }
}
const w0 = program.waves[0];
assert(
  w0.entry_gate_ids.includes("G-PROGRAM-ACTIVATION"),
  "WAVE-0 does not depend on pre-Wave Program activation"
);
assert(
  w0.slices.map(slice => slice.id).join(",") === "W0-S1,W0-S2,W0-S3,W0-S4",
  "WAVE-0 slice order drifted"
);
sameJson(
  Object.fromEntries(w0.slices.map(slice => [slice.id, slice.predecessor_slice_ids])),
  {
    "W0-S1": [],
    "W0-S2": ["W0-S1"],
    "W0-S3": ["W0-S2"],
    "W0-S4": ["W0-S1", "W0-S2", "W0-S3"],
  },
  "WAVE-0 predecessor DAG"
);
const w0s1 = w0.slices[0];
const w0s2 = w0.slices[1];
const missingTaskFields = program.agent_task_contract.required_fields.filter(
  field => !(field in w0s1.task_packet_blueprint)
);
assert(
  missingTaskFields.length === 0,
  `W0-S1 packet blueprint misses: ${missingTaskFields.join(", ")}`
);
sameSet(
  Object.keys(w0s1.task_packet_blueprint),
  program.agent_task_contract.required_fields,
  "W0-S1 blueprint fields"
);
assert(
  w0s1.status === "PREDECESSOR_TASK_EVIDENCE_INTEGRATED_NO_CLOSURE" &&
    w0s1.packet_status === "PREDECESSOR_RECEIPT_RETAINED_NOT_DISPATCHABLE" &&
    w0s1.task_packet_blueprint.packet_status === "HISTORICAL_BLUEPRINT_NOT_DISPATCHABLE" &&
    w0s1.task_packet_blueprint.execution_baseline_sha === null &&
    w0s1.task_packet_blueprint.branch === null,
  "W0-S1 predecessor receipt and historical blueprint boundary drifted"
);
assert(
  w0s2.status === "PREDECESSOR_TASK_EVIDENCE_INTEGRATED_NO_CLOSURE" &&
    w0s2.packet_status === "PREDECESSOR_RECEIPT_RETAINED_NOT_DISPATCHABLE",
  "W0-S2 retained predecessor receipt boundary drifted"
);
sameSet(
  w0s1.red_contract,
  [
    "W0-COV-MISSING",
    "W0-COV-NONNUMERIC",
    "W0-COV-ZERO-COLLECTION",
    "W0-COV-BELOW-THRESHOLD",
    "W0-TEST-ZERO-COLLECTION",
    "W0-TEST-FORBIDDEN-CREDIT",
    "W0-TEST-ID-DRIFT",
  ],
  "W0-S1 RED contract"
);
assert(
  w0s1.task_packet_blueprint.program_schema_version === CURRENT_SCHEMA_VERSION &&
    w0s1.task_packet_blueprint.source_map.includes("frontend/src/tauri.test.ts") &&
    w0s1.task_packet_blueprint.allowed_touched_paths.includes("frontend/src/tauri.test.ts") &&
    w0s1.task_packet_blueprint.forbidden_touched_paths.includes("frontend/src/tauriDev.ts") &&
    w0s1.task_packet_blueprint.verification_commands.includes(
      "corepack pnpm --dir frontend test:coverage:checker"
    ) &&
    w0s1.task_packet_blueprint.verification_commands.includes(
      "corepack pnpm --dir frontend test:selection:checker"
    ) &&
    w0s1.task_packet_blueprint.verification_commands.includes(
      "corepack pnpm --dir frontend test:historical"
    ),
  "W0-S1 mixed-owner and truthful-selection boundary drifted"
);
sameSet(w0s1.task_packet_blueprint.red_contract, w0s1.red_contract, "W0-S1 packet RED contract");
const w0s3 = w0.slices.find(slice => slice.id === "W0-S3");
assert(
  w0s3.finding_ids.includes("BR4-D064") &&
    w0s3.evidence_prerequisite_only === true &&
    w0s3.risk_class === "HIGH" &&
    w0s3.status === "PREDECESSOR_RECEIPT_INVALIDATED_RECOVERY_REQUIRED" &&
    w0s3.packet_status === "RECOVERY_VERIFICATION_PACKET_PREPARATION_REQUIRED",
  "W0-S3 is not the bounded high-risk receipt-recovery prerequisite"
);
const w0s4 = w0.slices.find(slice => slice.id === "W0-S4");
assert(
  w0s4.name === "Wave-0 Ledger Reconciliation" &&
    !/activate the approved Program/i.test(w0s4.objective),
  "W0-S4 still owns Program activation"
);

const gateIds = program.gates.map(gate => gate.id);
assert(new Set(gateIds).size === gateIds.length, "Duplicate Program gate ID");
const gateById = new Map(program.gates.map(gate => [gate.id, gate]));
for (const gate of program.gates) {
  assert(program.vocabularies.gate_status.includes(gate.status), `${gate.id} has invalid status`);
  assert(gate.fail_closed === true, `${gate.id} is not fail closed`);
  assert(
    Array.isArray(gate.required_guard_ids) &&
      gate.required_guard_ids.every(guardId => guardIds.has(guardId)),
    `${gate.id} has an invalid required guard set`
  );
  assert(gate.criteria.length > 0, `${gate.id} has no criteria`);
  assert(gate.evidence_dimensions.length > 0, `${gate.id} has no evidence dimension`);
  assert(Array.isArray(gate.evidence_records), `${gate.id} has no evidence-record array`);
  if (gate.status === "PASS") {
    validateCreditedEvidenceRecords({
      records: gate.evidence_records,
      requiredDimensions: gate.evidence_dimensions,
      requiredGuardIds: gate.required_guard_ids,
      expectedScopeId: gate.id,
      label: gate.id,
    });
  }
}
sameSet(
  gateById.get("G-PROGRAM-ACTIVATION").evidence_dimensions,
  ["USER_PROGRAM_APPROVAL", "TRACKED_AUTHORITY"],
  "Program activation evidence dimensions"
);
assert(
  gateById
    .get("G-PROGRAM-ACTIVATION")
    .evidence_records.every(record =>
      record.dimensions.every(dimension =>
        gateById.get("G-PROGRAM-ACTIVATION").evidence_dimensions.includes(dimension)
      )
    ),
  "Program activation evidence contains a surplus self-credited dimension"
);
sameSet(
  gateById.get("G-W0-COVERAGE-TRUTH").evidence_dimensions,
  ["CI_FAULT", "TEST_SIGNAL", "TEST_SELECTION"],
  "W0 truthful frontend evidence dimensions"
);
for (const wave of program.waves) {
  for (const gateId of [...wave.entry_gate_ids, ...wave.exit_gate_ids]) {
    assert(gateById.has(gateId), `${wave.id} references unknown gate ${gateId}`);
  }
}
for (const gateId of [
  "G-W1-P0-ADJUDICATION",
  "G-W2-AUTHORITY-TRUTH",
  "G-W3-ATOMICITY-BOUNDARY",
  "G-W4-CLOSURE-CONVERGENCE",
]) {
  assert(
    gateById.get(gateId).criteria.some(criterion => /every|每/i.test(criterion)),
    `${gateId} can pass without settling every entering card`
  );
}

const featureGates = program.feature_development_gates;
assert(featureGates, "Program has no split feature-development gates");
assert(!("feature_development_reopen_gate" in program), "Legacy broad feature gate still exists");
const boundedFeatureGate = featureGates.bounded_feature_eligibility;
const normalFeatureGate = featureGates.normal_feature_development_reopen;
assert(
  boundedFeatureGate.fail_closed === true && normalFeatureGate.fail_closed === true,
  "Feature-development gates are not fail closed"
);
for (const [label, featureGate] of [
  ["bounded feature", boundedFeatureGate],
  ["normal feature", normalFeatureGate],
]) {
  assert(
    Array.isArray(featureGate.required_guard_ids) &&
      featureGate.required_guard_ids.length > 0 &&
      featureGate.required_guard_ids.every(guardId => guardIds.has(guardId)),
    `${label} gate has an invalid required guard set`
  );
}
assert(
  (boundedFeatureGate.status === "PASS") === (boundedFeatureGate.eligible === true),
  "Bounded feature status and eligibility disagree"
);
assert(
  (normalFeatureGate.status === "PASS") === (normalFeatureGate.reopened === true),
  "Normal feature status and reopen state disagree"
);
if (boundedFeatureGate.eligible) {
  assert(
    Array.isArray(boundedFeatureGate.eligible_domains) &&
      boundedFeatureGate.eligible_domains.length > 0,
    "Bounded feature eligibility has no reviewed domain"
  );
  validateCreditedEvidenceRecords({
    records: boundedFeatureGate.evidence_records,
    requiredDimensions: ["FEATURE_BOUNDARY", "OPEN_CARD_CROSSING", "ABSENCE"],
    requiredGuardIds: boundedFeatureGate.required_guard_ids,
    label: "bounded feature eligibility",
  });
}
if (normalFeatureGate.reopened) {
  validateCreditedEvidenceRecords({
    records: normalFeatureGate.evidence_records,
    requiredDimensions: ["PROGRAM_WIDE_FEATURE_REOPEN", "PHASE7_TRIAL"],
    requiredGuardIds: normalFeatureGate.required_guard_ids,
    label: "normal feature development reopen",
  });
}

const baselineFacts = ledger.baseline_fact_records;
assert(baselineFacts.length === 101, "Expected 101 immutable baseline facts");
for (const record of baselineFacts) {
  const { fact_sha256: observed, ...fact } = record;
  assert(canonicalDigest(fact) === observed, `${record.card_id} baseline fact digest is invalid`);
}
assert(
  canonicalDigest(baselineFacts) === BASELINE_FACT_HASH &&
    ledger.initial_inventory.baseline_fact_set_sha256 === BASELINE_FACT_HASH,
  "Baseline fact set drifted"
);
const baselineCardIds = ledger.initial_inventory.baseline_card_ids;
assert(
  baselineCardIds.length === 101 && newlineIdDigest(baselineCardIds) === BASELINE_CARD_HASH,
  "Baseline card-ID snapshot drifted"
);

const cards = ledger.cards;
assert(cards.length >= 101, "Living ledger deleted a baseline card");
const cardIds = cards.map(card => card.card_id);
assert(new Set(cardIds).size === cards.length, "Problem-card IDs are not unique");
sameSet(
  baselineCardIds.filter(id => cardIds.includes(id)),
  baselineCardIds,
  "Baseline cards retained in living ledger"
);
const baselineFactById = new Map(baselineFacts.map(record => [record.card_id, record]));
const baselineCards = cards.filter(card => baselineFactById.has(card.card_id));
assert(baselineCards.length === 101, "Not all baseline cards remain live");

const expectedWaveCounts = {
  "WAVE-0": 3,
  "WAVE-1": 15,
  "WAVE-2": 41,
  "WAVE-3": 21,
  "WAVE-4": 21,
  "WAVE-5": 0,
};
sameJson(
  {
    ...countBy(baselineFacts, "initial_assigned_wave"),
    "WAVE-5": 0,
  },
  expectedWaveCounts,
  "Immutable initial Wave counts"
);
sameJson(ledger.initial_inventory.wave_counts, expectedWaveCounts, "Ledger inventory Wave counts");
for (const wave of program.waves) {
  assert(
    wave.assigned_card_count === expectedWaveCounts[wave.id],
    `${wave.id} Program count disagrees with immutable baseline assignment`
  );
  const markdownWaveRow = new RegExp(
    `^\\|\\s*\\\`${regexEscape(wave.id)}\\\`\\s+—\\s+${regexEscape(wave.name)}\\s*` +
      `\\|\\s*${expectedWaveCounts[wave.id]}\\s*\\|`,
    "m"
  );
  assert(
    markdownWaveRow.test(programMarkdown),
    `${wave.id} Markdown summary disagrees with Program JSON`
  );
}

const wave0Ids = new Set(["R3-N010", "R3-N020", "R4-FLAKY-001"]);
const wave1Ids = new Set([
  "BR4-D001",
  "BR4-D013",
  "BR4-D014",
  "BR4-D019",
  "BR4-D028",
  "BR4-D029",
  "BR4-D033",
  "BR4-D035",
  "BR4-D037",
  "BR4-D044",
  "BR4-D057",
  "BR4-D064",
  "BR4-D068",
  "BR4-D071",
  "BR4-D072",
]);
const defaultWaveByCluster = {
  "CQ-C01": "WAVE-2",
  "CQ-C02": "WAVE-2",
  "CQ-C03": "WAVE-3",
  "CQ-C04": "WAVE-3",
  "CQ-C05": "WAVE-2",
  "CQ-C06": "WAVE-4",
  "CQ-C07": "WAVE-4",
  "CQ-C08": "WAVE-3",
  "CQ-C09": "WAVE-4",
  "CQ-C10": "WAVE-4",
};
for (const fact of baselineFacts) {
  const expectedWave = wave0Ids.has(fact.card_id)
    ? "WAVE-0"
    : wave1Ids.has(fact.card_id)
      ? "WAVE-1"
      : defaultWaveByCluster[fact.root_cause_cluster_id];
  assert(
    fact.initial_assigned_wave === expectedWave,
    `${fact.card_id} initial Wave mapping is invalid`
  );
}

const expectedSourceUnknown = [
  "BR4-D009",
  "BR4-D010",
  "BR4-D011",
  "BR4-D018",
  "BR4-D038",
  "BR4-D039",
];
sameSet(
  baselineFacts
    .filter(fact => fact.initial_source_evidence.status === "UNKNOWN")
    .map(fact => fact.card_id),
  expectedSourceUnknown,
  "Initial source-UNKNOWN cards"
);
sameJson(
  countBy(baselineFacts, "initial_current_severity"),
  { P0: 1, P1: 17, P2: 22, UNKNOWN: 61 },
  "Initial severity"
);
sameJson(
  countBy(baselineFacts, "initial_disposition"),
  {
    OPEN: 29,
    OPEN_CONFIRMED: 11,
    CLOSURE_CANDIDATE: 54,
    OPEN_UNKNOWN: 7,
  },
  "Initial disposition"
);

const clusterCardIds = ledger.root_cause_clusters.flatMap(cluster => cluster.card_ids);
assert(ledger.root_cause_clusters.length === 10, "Expected ten root-cause clusters");
assert(clusterCardIds.length === 101, "Cluster counts do not total 101");
sameSet(clusterCardIds, baselineCardIds, "Cluster baseline-card partition");
const clusterByCard = new Map();
const approvedClusterIds = new Set(ledger.root_cause_clusters.map(cluster => cluster.id));
for (const cluster of ledger.root_cause_clusters) {
  for (const guardId of cluster.guard_ids) {
    assert(guardIds.has(guardId), `${cluster.id} references unknown guard ${guardId}`);
  }
  for (const cardId of cluster.card_ids) {
    assert(!clusterByCard.has(cardId), `${cardId} has duplicate cluster membership`);
    clusterByCard.set(cardId, cluster.id);
  }
}
const newCards = cards.filter(card => !baselineFactById.has(card.card_id));
const newCardRecords = ledger.new_card_creation_records;
const newCardCreationStateById = new Map();
sameSet(
  newCardRecords.map(record => record.card_id),
  newCards.map(card => card.card_id),
  "New-card creation-record coverage"
);
assert(
  JSON.stringify(ledger.new_card_contract.stable_fact_fields) ===
    JSON.stringify([
      "card_id",
      "title",
      "origin",
      "root_cause_cluster_id",
      "guard_ids",
      "severity_at_discovery",
      "assigned_wave",
      "initial_assigned_wave",
      "prerequisite_slice_ids",
      "exact_next_proof",
    ]),
  "New-card stable-fact contract drifted"
);
for (const record of newCardRecords) {
  for (const field of ledger.new_card_contract.required_record_fields) {
    assert(field in record, `New-card record misses ${field}`);
  }
  const card = newCards.find(candidate => candidate.card_id === record.card_id);
  assert(
    card &&
      /^CUR-N\d{3,}$/.test(card.card_id) &&
      approvedClusterIds.has(card.root_cause_cluster_id) &&
      expectedWaveIds.includes(card.initial_assigned_wave) &&
      card.initial_assigned_wave !== "WAVE-5" &&
      card.assigned_wave === card.initial_assigned_wave &&
      isCommitAncestorOfHead(record.creation_sha) &&
      record.reviewed_head_sha === record.creation_sha &&
      record.review_outcome === "PASS" &&
      /^[0-9a-f]{64}$/.test(record.stable_fact_sha256) &&
      typeof record.producer_id === "string" &&
      record.producer_id.trim() &&
      typeof record.reviewer_id === "string" &&
      record.reviewer_id.trim() &&
      record.producer_id !== record.reviewer_id &&
      typeof record.artifact_or_record === "string" &&
      record.artifact_or_record.trim(),
    `New card ${record.card_id} lacks a valid review bound to its creation SHA`
  );
  const creationLedger = readJsonAtCommit(record.creation_sha, ledgerPath);
  const creationCard = creationLedger.cards.find(candidate => candidate.card_id === record.card_id);
  if (creationCard) {
    newCardCreationStateById.set(record.card_id, livingCardStateProjection(creationCard));
  }
  assert(
    creationCard &&
      creationCard.wave_outcome_history.length === 0 &&
      creationCard.closure_credit === false &&
      creationCard.closure_record === null &&
      canonicalDigest(newCardStableFactProjection(creationCard)) === record.stable_fact_sha256 &&
      canonicalDigest(newCardStableFactProjection(card)) === record.stable_fact_sha256 &&
      (card.wave_outcome_history.length > 0 ||
        canonicalDigest(livingCardStateProjection(card)) ===
          canonicalDigest(livingCardStateProjection(creationCard))),
    `New card ${record.card_id} drifted from its reviewed creation commit`
  );
}

const attemptRecords = ledger.implementation_attempt_records;
const architectureReviewRecords = ledger.architecture_review_records;
assert(
  Array.isArray(attemptRecords) && Array.isArray(architectureReviewRecords),
  "Attempt/review ledgers are not arrays"
);
const attemptById = new Map();
const attemptPacketDigests = new Set();
for (const record of attemptRecords) {
  sameSet(
    Object.keys(record),
    ledger.attempt_contract.required_attempt_fields,
    `Attempt ${record.attempt_id} fields`
  );
  const attemptArtifactText = readTextAtCommit(
    record.artifact_commit_sha,
    record.attempt_artifact_path
  );
  const attemptArtifact = JSON.parse(attemptArtifactText);
  sameSet(
    Object.keys(attemptArtifact),
    ledger.attempt_contract.attempt_artifact_required_fields,
    `Attempt ${record.attempt_id} artifact fields`
  );
  assert(
    typeof record.attempt_id === "string" &&
      record.attempt_id.trim() &&
      !attemptById.has(record.attempt_id) &&
      typeof record.task_id === "string" &&
      record.task_id.trim() &&
      typeof record.slice_id === "string" &&
      record.slice_id.trim() &&
      (approvedClusterIds.has(record.root_cause_cluster_id) ||
        record.root_cause_cluster_id === "PROGRAM-GOVERNANCE") &&
      typeof record.packet_sha256 === "string" &&
      /^[0-9a-f]{64}$/.test(record.packet_sha256) &&
      !attemptPacketDigests.has(record.packet_sha256) &&
      record.packet_artifact_path ===
        `${ledger.integration_contract.packet_archive_prefix}${record.packet_sha256}.json` &&
      isCommitAncestorOfHead(record.execution_baseline_sha) &&
      isCommitAncestorOfHead(record.artifact_commit_sha) &&
      canGit(
        "merge-base",
        "--is-ancestor",
        record.execution_baseline_sha,
        record.artifact_commit_sha
      ) !== null &&
      typeof record.attempt_artifact_sha256 === "string" &&
      /^[0-9a-f]{64}$/.test(record.attempt_artifact_sha256) &&
      record.attempt_artifact_path ===
        `${ledger.attempt_contract.attempt_artifact_prefix}${record.attempt_artifact_sha256}.json` &&
      pathAtCommitExists(record.artifact_commit_sha, record.attempt_artifact_path) &&
      pathAtCommitExists(record.artifact_commit_sha, record.packet_artifact_path) &&
      textDigest(attemptArtifactText) === record.attempt_artifact_sha256 &&
      Buffer.byteLength(attemptArtifactText, "utf8") <=
        ledger.attempt_contract.artifact_budget.max_attempt_artifact_bytes &&
      ledger.attempt_contract.forbidden_artifact_markers.every(
        marker => !attemptArtifactText.includes(marker)
      ) &&
      attemptArtifact.attempt_id === record.attempt_id &&
      attemptArtifact.task_id === record.task_id &&
      attemptArtifact.slice_id === record.slice_id &&
      attemptArtifact.root_cause_cluster_id === record.root_cause_cluster_id &&
      attemptArtifact.packet_sha256 === record.packet_sha256 &&
      attemptArtifact.execution_baseline_sha === record.execution_baseline_sha &&
      attemptArtifact.outcome === record.outcome &&
      attemptArtifact.reason === record.reason &&
      attemptArtifact.producer_id === record.producer_id &&
      typeof attemptArtifact.hypothesis === "string" &&
      attemptArtifact.hypothesis.trim() &&
      Array.isArray(attemptArtifact.change_summary) &&
      attemptArtifact.change_summary.length > 0 &&
      attemptArtifact.change_summary.every(item => typeof item === "string" && item.trim()) &&
      Array.isArray(attemptArtifact.evaluated_gate_ids) &&
      attemptArtifact.evaluated_gate_ids.length > 0 &&
      new Set(attemptArtifact.evaluated_gate_ids).size ===
        attemptArtifact.evaluated_gate_ids.length &&
      attemptArtifact.evaluated_gate_ids.every(
        gateId => typeof gateId === "string" && gateId.trim()
      ) &&
      Array.isArray(attemptArtifact.failed_gate_ids) &&
      new Set(attemptArtifact.failed_gate_ids).size === attemptArtifact.failed_gate_ids.length &&
      attemptArtifact.failed_gate_ids.every(
        gateId =>
          typeof gateId === "string" &&
          gateId.trim() &&
          attemptArtifact.evaluated_gate_ids.includes(gateId)
      ) &&
      (record.outcome === "FAILED_ROOT_CAUSE_ATTEMPT"
        ? attemptArtifact.failed_gate_ids.length > 0 &&
          typeof attemptArtifact.failure_signature === "string" &&
          attemptArtifact.failure_signature.trim()
        : attemptArtifact.failed_gate_ids.length === 0) &&
      typeof attemptArtifact.observed_diff_or_log === "string" &&
      attemptArtifact.observed_diff_or_log.trim() &&
      Buffer.byteLength(attemptArtifact.observed_diff_or_log, "utf8") <=
        ledger.attempt_contract.artifact_budget.max_observed_diff_or_log_bytes &&
      typeof attemptArtifact.observed_diff_or_log_sha256 === "string" &&
      /^[0-9a-f]{64}$/.test(attemptArtifact.observed_diff_or_log_sha256) &&
      textDigest(attemptArtifact.observed_diff_or_log) ===
        attemptArtifact.observed_diff_or_log_sha256 &&
      ledger.attempt_contract.allowed_attempt_outcomes.includes(record.outcome) &&
      typeof record.reason === "string" &&
      record.reason.trim() &&
      typeof record.producer_id === "string" &&
      record.producer_id.trim() &&
      typeof record.integrator_id === "string" &&
      record.integrator_id.trim() &&
      record.producer_id !== record.integrator_id &&
      typeof record.record_note === "string" &&
      record.record_note.trim(),
    `Attempt record is invalid: ${record.attempt_id}`
  );
  attemptById.set(record.attempt_id, record);
  attemptPacketDigests.add(record.packet_sha256);
}
const architectureReviewIds = new Set();
const coveredFailedAttemptIds = new Set();
const latestArchitectureReviewCommitByCluster = new Map();
for (const review of architectureReviewRecords) {
  sameSet(
    Object.keys(review),
    ledger.attempt_contract.required_architecture_review_fields,
    `Architecture review ${review.review_id} fields`
  );
  const priorReviewCommit = latestArchitectureReviewCommitByCluster.get(
    review.root_cause_cluster_id
  );
  assert(
    !priorReviewCommit ||
      (priorReviewCommit !== review.review_artifact_commit_sha &&
        canGit(
          "merge-base",
          "--is-ancestor",
          priorReviewCommit,
          review.review_artifact_commit_sha
        ) !== null),
    `Architecture reviews are not chronological for ${review.root_cause_cluster_id}`
  );
  latestArchitectureReviewCommitByCluster.set(
    review.root_cause_cluster_id,
    review.review_artifact_commit_sha
  );
  const reviewArtifactText = readTextAtCommit(
    review.review_artifact_commit_sha,
    review.review_artifact_path
  );
  const reviewArtifact = JSON.parse(reviewArtifactText);
  sameSet(
    Object.keys(reviewArtifact),
    ledger.attempt_contract.architecture_review_artifact_required_fields,
    `Architecture review ${review.review_id} artifact fields`
  );
  assert(
    typeof review.review_id === "string" &&
      review.review_id.trim() &&
      !architectureReviewIds.has(review.review_id) &&
      (approvedClusterIds.has(review.root_cause_cluster_id) ||
        review.root_cause_cluster_id === "PROGRAM-GOVERNANCE") &&
      Array.isArray(review.covered_failed_attempt_ids) &&
      review.covered_failed_attempt_ids.length >=
        ledger.attempt_contract.failed_attempt_limit_before_review &&
      new Set(review.covered_failed_attempt_ids).size ===
        review.covered_failed_attempt_ids.length &&
      isCommitAncestorOfHead(review.review_artifact_commit_sha) &&
      typeof review.review_artifact_sha256 === "string" &&
      /^[0-9a-f]{64}$/.test(review.review_artifact_sha256) &&
      review.review_artifact_path ===
        `${ledger.attempt_contract.architecture_review_artifact_prefix}${review.review_artifact_sha256}.json` &&
      pathAtCommitExists(review.review_artifact_commit_sha, review.review_artifact_path) &&
      textDigest(reviewArtifactText) === review.review_artifact_sha256 &&
      Buffer.byteLength(reviewArtifactText, "utf8") <=
        ledger.attempt_contract.artifact_budget.max_architecture_review_artifact_bytes &&
      ledger.attempt_contract.forbidden_artifact_markers.every(
        marker => !reviewArtifactText.includes(marker)
      ) &&
      reviewArtifact.review_id === review.review_id &&
      reviewArtifact.root_cause_cluster_id === review.root_cause_cluster_id &&
      canonicalDigest(reviewArtifact.covered_failed_attempt_ids) ===
        canonicalDigest(review.covered_failed_attempt_ids) &&
      canonicalDigest(reviewArtifact.covered_attempt_artifact_sha256s) ===
        canonicalDigest(
          review.covered_failed_attempt_ids.map(
            attemptId => attemptById.get(attemptId)?.attempt_artifact_sha256
          )
        ) &&
      typeof reviewArtifact.shared_failure_pattern === "string" &&
      reviewArtifact.shared_failure_pattern.trim() &&
      typeof reviewArtifact.root_cause_reassessment === "string" &&
      reviewArtifact.root_cause_reassessment.trim() &&
      typeof reviewArtifact.revised_invariant_or_strategy === "string" &&
      reviewArtifact.revised_invariant_or_strategy.trim() &&
      Array.isArray(reviewArtifact.next_attempt_constraints) &&
      reviewArtifact.next_attempt_constraints.length > 0 &&
      reviewArtifact.next_attempt_constraints.every(
        constraint => typeof constraint === "string" && constraint.trim()
      ) &&
      reviewArtifact.outcome === review.outcome &&
      reviewArtifact.reviewer_id === review.reviewer_id &&
      reviewArtifact.next_attempt_authorized === review.next_attempt_authorized &&
      review.outcome === ledger.attempt_contract.architecture_review_pass_outcome &&
      review.next_attempt_authorized === true &&
      typeof review.reviewer_id === "string" &&
      review.reviewer_id.trim(),
    `Architecture review is invalid: ${review.review_id}`
  );
  architectureReviewIds.add(review.review_id);
  for (const attemptId of review.covered_failed_attempt_ids) {
    const attempt = attemptById.get(attemptId);
    assert(
      attempt &&
        attempt.root_cause_cluster_id === review.root_cause_cluster_id &&
        attempt.outcome === ledger.attempt_contract.counted_failure_outcome &&
        !coveredFailedAttemptIds.has(attemptId) &&
        attempt.producer_id !== review.reviewer_id &&
        attempt.integrator_id !== review.reviewer_id &&
        canGit(
          "merge-base",
          "--is-ancestor",
          attempt.artifact_commit_sha,
          review.review_artifact_commit_sha
        ) !== null,
      `Architecture review does not independently cover failed attempt ${attemptId}`
    );
    coveredFailedAttemptIds.add(attemptId);
  }
}

const validateLedgerEvidenceRecords = (
  records,
  requiredFields,
  label,
  requireCredit = false,
  requiredDimensions = [],
  expectedSubjectSha = null,
  requiredGuardIds = [],
  requiredOwnerPath = null,
  expectedScopeId = null
) => {
  assert(Array.isArray(records) && records.length > 0, `${label} has no evidence`);
  const recordIds = new Set();
  for (const record of records) {
    assert(
      record && typeof record === "object" && !Array.isArray(record),
      `${label} contains a non-object evidence record`
    );
    for (const field of requiredFields) {
      assert(field in record, `${label} evidence misses ${field}`);
    }
    assert(
      typeof record.record_id === "string" &&
        record.record_id.trim() &&
        !recordIds.has(record.record_id) &&
        typeof record.scope_id === "string" &&
        record.scope_id.trim() &&
        (!expectedScopeId || record.scope_id === expectedScopeId) &&
        Array.isArray(record.scope_paths) &&
        record.scope_paths.length > 0 &&
        Array.isArray(record.guard_ids) &&
        record.guard_ids.every(guardId => guardIds.has(guardId)),
      `${label} evidence identity/scope is invalid`
    );
    recordIds.add(record.record_id);
    for (const pattern of record.scope_paths) {
      assertSafePathPattern(pattern, `${label} evidence scope`);
      assert(!pattern.includes("*"), `${label} evidence scope must be a literal path: ${pattern}`);
    }
    if (requiredOwnerPath) {
      assert(
        pathMatches(requiredOwnerPath, record.scope_paths),
        `${label} evidence does not cover canonical owner ${requiredOwnerPath}`
      );
    }
    assert(
      typeof record.artifact_or_record === "string" && record.artifact_or_record.trim(),
      `${label} evidence artifact is empty`
    );
    assert(
      Array.isArray(record.dimensions) && record.dimensions.length > 0,
      `${label} evidence dimensions are empty`
    );
    assert(
      ["PASS", "FAIL", "BLOCKED", "UNKNOWN"].includes(record.outcome) &&
        Array.isArray(record.limitations) &&
        typeof record.credit_allowed === "boolean" &&
        typeof record.active === "boolean" &&
        typeof record.producer_id === "string" &&
        record.producer_id.trim() &&
        typeof record.reviewer_id === "string" &&
        record.reviewer_id.trim() &&
        record.producer_id !== record.reviewer_id,
      `${label} evidence status/review contract is invalid`
    );
    assert(
      isCommitAncestorOfHead(record.subject_sha) &&
        (!expectedSubjectSha || record.subject_sha === expectedSubjectSha),
      `${label} evidence is not bound to the expected current-chain SHA`
    );
    if (requireCredit) {
      assert(
        record.outcome === "PASS" &&
          record.credit_allowed === true &&
          record.active === true &&
          record.limitations.length === 0 &&
          evidenceRecordIsFresh(record),
        `${label} contains uncredited or limited evidence`
      );
    }
  }
  if (requireCredit) {
    const creditedDimensions = new Set(
      records
        .filter(
          record =>
            record.active === true &&
            record.outcome === "PASS" &&
            record.credit_allowed === true &&
            record.limitations.length === 0
        )
        .flatMap(record => record.dimensions)
    );
    for (const dimension of requiredDimensions) {
      assert(creditedDimensions.has(dimension), `${label} lacks credited ${dimension} evidence`);
    }
    const creditedGuardIds = new Set(
      records
        .filter(
          record =>
            record.active === true &&
            record.outcome === "PASS" &&
            record.credit_allowed === true &&
            record.limitations.length === 0
        )
        .flatMap(record => record.guard_ids)
    );
    for (const guardId of requiredGuardIds) {
      assert(creditedGuardIds.has(guardId), `${label} lacks credited ${guardId} evidence`);
    }
  }
};
const allowedWaveOutcomes = new Set(ledger.vocabularies.wave_outcome_status);
const approvedDraftLedgerForOutcomes = cards.some(card => card.wave_outcome_history.length > 0)
  ? readJsonAtCommit(program.program_approval.approved_draft_commit_sha, ledgerPath)
  : null;
const waveOutcomeIds = new Set();
for (const card of cards) {
  assert(
    ledger.vocabularies.severity.includes(card.current_severity) &&
      ledger.vocabularies.card_disposition.includes(card.current_disposition) &&
      typeof card.closure_credit === "boolean" &&
      approvedClusterIds.has(card.root_cause_cluster_id),
    `${card.card_id} has an invalid living severity/disposition/closure type`
  );
  assert(expectedWaveIds.includes(card.current_wave), `${card.card_id} has invalid current Wave`);
  assert(
    expectedWaveIds.includes(card.initial_assigned_wave) &&
      card.assigned_wave === card.initial_assigned_wave &&
      Array.isArray(card.prerequisite_slice_ids),
    `${card.card_id} has invalid initial Wave`
  );
  assert(
    card.exact_next_proof?.proof_id === `${card.card_id}-NEXT` &&
      card.exact_next_proof.statement?.trim() &&
      card.exact_next_proof.requires_current_sha === true &&
      Array.isArray(card.exact_next_proof.required_dimensions) &&
      card.exact_next_proof.required_dimensions.length > 0 &&
      card.exact_next_proof.blocks_closure === true,
    `${card.card_id} has no exact next proof`
  );
  assert(
    !/进入 R4|需 R4/.test(card.exact_next_proof.statement),
    `${card.card_id} has obsolete R4 next-proof wording`
  );
  for (const guardId of card.guard_ids) {
    assert(guardIds.has(guardId), `${card.card_id} references unknown guard ${guardId}`);
  }
  assert(
    ledger.vocabularies.owner_status.includes(card.canonical_owner.status) &&
      (card.canonical_owner.status === "UNKNOWN"
        ? card.canonical_owner.path === null
        : typeof card.canonical_owner.path === "string" &&
          pathAtCommitExists("HEAD", card.canonical_owner.path)),
    `${card.card_id} has an invalid or absent current owner`
  );
  assert(
    card.feature_reopen &&
      typeof card.feature_reopen.global_blocker === "boolean" &&
      typeof card.feature_reopen.note === "string" &&
      card.feature_reopen.note.trim(),
    `${card.card_id} has an invalid feature-reopen record`
  );
  const closureRequirementKeys = [
    "current_sha_implementation_or_adjudication",
    "behavior_or_fault_proof",
    "independent_closure_review_same_sha",
    "capability_non_regression",
    "expected_absence_if_applicable",
  ];
  sameSet(
    Object.keys(card.closure_requirements),
    closureRequirementKeys,
    `${card.card_id} closure-requirement keys`
  );
  assert(
    closureRequirementKeys.every(key => typeof card.closure_requirements[key] === "boolean"),
    `${card.card_id} closure requirements are not boolean`
  );
  if (baselineFactById.has(card.card_id)) {
    const fact = baselineFactById.get(card.card_id);
    assert(
      canonicalDigest(stableFactProjection(card)) ===
        canonicalDigest(stableBaselineFactProjection(fact)),
      `${card.card_id} drifted from immutable identity/cluster/guard facts`
    );
    assert(
      clusterByCard.get(card.card_id) === card.root_cause_cluster_id,
      `${card.card_id} card/cluster inventory disagree`
    );
  }
  for (const evidence of [card.source_evidence, card.behavior_evidence]) {
    assert(
      expectedEvidenceVocabulary.includes(evidence.status),
      `${card.card_id} has invalid evidence status`
    );
    assert(
      expectedCoverageVocabulary.includes(evidence.coverage),
      `${card.card_id} has invalid evidence coverage`
    );
  }
  if (card.source_evidence.status === "SOURCE-CONFIRMED") {
    assert(card.source_evidence.refs.length > 0, `${card.card_id} lost source refs`);
    for (const ref of card.source_evidence.refs) {
      validateRefAtCommit("HEAD", ref, `${card.card_id} current source ref`);
    }
  }
  if (card.behavior_evidence.status === "REPRODUCED") {
    assert(
      card.behavior_evidence.records.length > 0,
      `${card.card_id} claims reproduced behavior without a record`
    );
  }
  for (const record of card.behavior_evidence.records) {
    assert(
      expectedEvidenceVocabulary.includes(record.status) &&
        expectedCoverageVocabulary.includes(record.coverage),
      `${card.card_id} has invalid behavior evidence record`
    );
    assert(
      record.finding_closure_credit === false,
      `${card.card_id} behavior record grants closure directly`
    );
  }
  let activeWave = card.initial_assigned_wave;
  let quarantineOutcomeSeen = false;
  let closedOutcomeSeen = false;
  let previousLiveFact =
    card.wave_outcome_history.length === 0
      ? null
      : baselineFactById.has(card.card_id)
        ? livingCardStateProjection(
            approvedDraftLedgerForOutcomes.cards.find(
              candidate => candidate.card_id === card.card_id
            )
          )
        : newCardCreationStateById.get(card.card_id);
  if (card.wave_outcome_history.length > 0) {
    assert(previousLiveFact, `${card.card_id} lacks an initial live-fact snapshot`);
  }
  for (const outcome of card.wave_outcome_history) {
    assert(
      !quarantineOutcomeSeen &&
        (closedOutcomeSeen
          ? outcome.status === "REVALIDATED_CLOSED"
          : outcome.status !== "REVALIDATED_CLOSED"),
      `${card.card_id} has an invalid outcome after a terminal state`
    );
    for (const field of ledger.state_transition_contracts.wave_outcome_required_fields) {
      assert(field in outcome, `${card.card_id} Wave outcome misses ${field}`);
    }
    assert(
      typeof outcome.outcome_id === "string" &&
        outcome.outcome_id.trim() &&
        !waveOutcomeIds.has(outcome.outcome_id),
      `${card.card_id} has an empty or duplicate Wave outcome ID`
    );
    waveOutcomeIds.add(outcome.outcome_id);
    sameSet(
      Object.keys(outcome.resulting_live_fact),
      Object.keys(livingCardStateProjection(card)),
      `${card.card_id} Wave outcome live-fact keys`
    );
    assert(
      /^[0-9a-f]{64}$/.test(outcome.resulting_live_fact_sha256) &&
        canonicalDigest(outcome.resulting_live_fact) === outcome.resulting_live_fact_sha256,
      `${card.card_id} Wave outcome live-fact digest is invalid`
    );
    assert(
      allowedWaveOutcomes.has(outcome.status) && outcome.status !== "NOT_ADJUDICATED_IN_WAVE",
      `${card.card_id} has invalid Wave outcome`
    );
    assert(
      outcome.wave_id === activeWave,
      `${card.card_id} outcome skips or repeats the wrong Wave`
    );
    assert(
      isCommitAncestorOfHead(outcome.execution_sha),
      `${card.card_id} Wave outcome SHA is not a current-chain commit`
    );
    assert(outcome.reason?.trim(), `${card.card_id} Wave outcome lacks reason`);
    validateLedgerEvidenceRecords(
      outcome.evidence_records,
      ledger.state_transition_contracts.wave_outcome_evidence_record_required_fields,
      `${card.card_id} ${outcome.wave_id} outcome`,
      ["ADJUDICATED_AND_REASSIGNED", "REVALIDATED_CLOSED"].includes(outcome.status),
      outcome.status === "ADJUDICATED_AND_REASSIGNED"
        ? ["CURRENT_SHA_ADJUDICATION"]
        : outcome.status === "REVALIDATED_CLOSED"
          ? ["CLOSURE_REVALIDATION", "NON_REGRESSION"]
          : [],
      outcome.execution_sha,
      card.guard_ids,
      outcome.resulting_live_fact.canonical_owner.path,
      card.card_id
    );
    if (
      outcome.status === "ADJUDICATED_AND_REASSIGNED" ||
      outcome.status === "EXPLICIT_CARRY_FORWARD_WITH_REASON"
    ) {
      assert(
        expectedWaveIds.includes(outcome.target_wave) &&
          expectedWaveIds.indexOf(outcome.target_wave) > expectedWaveIds.indexOf(activeWave) &&
          typeof outcome.target_slice_or_preparation_id === "string" &&
          outcome.target_slice_or_preparation_id.trim() &&
          typeof outcome.owner_id === "string" &&
          outcome.owner_id.trim(),
        `${card.card_id} reassignment/carry-forward is not owned and forward-only`
      );
      activeWave = outcome.target_wave;
    } else if (outcome.status === "QUARANTINED_UNREACHABLE") {
      validateLedgerEvidenceRecords(
        outcome.evidence_records,
        ledger.state_transition_contracts.wave_outcome_evidence_record_required_fields,
        `${card.card_id} quarantine`,
        true,
        ["UNREACHABILITY_OR_ISOLATION", "GUARD_OR_ABSENCE"],
        outcome.execution_sha,
        card.guard_ids,
        outcome.resulting_live_fact.canonical_owner.path,
        card.card_id
      );
      for (const field of [
        "quarantine_boundary",
        "unreachability_or_isolation_evidence",
        "guard_or_absence_check",
      ]) {
        assert(
          typeof outcome[field] === "string" && outcome[field].trim(),
          `${card.card_id} quarantine misses ${field}`
        );
      }
      quarantineOutcomeSeen = true;
    } else if (outcome.status === "CLOSED") {
      closedOutcomeSeen = true;
    } else if (outcome.status === "REVALIDATED_CLOSED") {
      assert(
        previousLiveFact.closure_credit === true &&
          previousLiveFact.current_disposition === "CLOSED" &&
          previousLiveFact.closure_record &&
          outcome.execution_sha === outcome.resulting_live_fact.closure_record?.evidence_head_sha &&
          outcome.resulting_live_fact.closure_record?.implementation_sha ===
            previousLiveFact.closure_record.implementation_sha &&
          outcome.resulting_live_fact.closure_record?.implementation_author_id ===
            previousLiveFact.closure_record.implementation_author_id,
        `${card.card_id} closure revalidation is not bound to the prior closure`
      );
      const beforeRevalidation = clone(previousLiveFact);
      const afterRevalidation = clone(outcome.resulting_live_fact);
      for (const field of [
        "source_evidence",
        "behavior_evidence",
        "closure_requirements",
        "closure_record",
      ]) {
        beforeRevalidation[field] = "__REFRESHABLE_CLOSURE_EVIDENCE__";
        afterRevalidation[field] = "__REFRESHABLE_CLOSURE_EVIDENCE__";
      }
      assert(
        canonicalDigest(beforeRevalidation) === canonicalDigest(afterRevalidation),
        `${card.card_id} closure revalidation changed non-evidence live facts`
      );
    }
    if (outcome.status === "EXPLICIT_CARRY_FORWARD_WITH_REASON") {
      const beforeCarry = clone(previousLiveFact);
      const afterCarry = clone(outcome.resulting_live_fact);
      beforeCarry.current_wave = "__FORWARD_WAVE__";
      afterCarry.current_wave = "__FORWARD_WAVE__";
      assert(
        canonicalDigest(beforeCarry) === canonicalDigest(afterCarry),
        `${card.card_id} carry-forward changed live facts without adjudication`
      );
    }
    if (outcome.status === "QUARANTINED_UNREACHABLE") {
      assert(
        outcome.resulting_live_fact.closure_credit === false &&
          outcome.resulting_live_fact.current_disposition !== "CLOSED",
        `${card.card_id} quarantine invented closure credit`
      );
    }
    if (["CLOSED", "REVALIDATED_CLOSED"].includes(outcome.status)) {
      assert(
        outcome.resulting_live_fact.closure_credit === true &&
          outcome.resulting_live_fact.current_disposition === "CLOSED" &&
          outcome.resulting_live_fact.closure_record,
        `${card.card_id} closed outcome lacks a closed live-fact snapshot`
      );
    } else {
      assert(
        outcome.resulting_live_fact.closure_credit === false &&
          outcome.resulting_live_fact.current_disposition !== "CLOSED" &&
          outcome.resulting_live_fact.closure_record === null,
        `${card.card_id} non-CLOSED outcome invented closure state`
      );
    }
    assert(
      outcome.resulting_live_fact.current_wave === activeWave,
      `${card.card_id} Wave outcome snapshot has the wrong resulting Wave`
    );
    previousLiveFact = outcome.resulting_live_fact;
  }
  assert(
    card.current_wave === activeWave,
    `${card.card_id} current Wave disagrees with outcome history`
  );
  if (card.wave_outcome_history.length > 0) {
    const lastOutcome = card.wave_outcome_history.at(-1);
    assert(
      canonicalDigest(livingCardStateProjection(card)) === lastOutcome.resulting_live_fact_sha256,
      `${card.card_id} living state drifted from its latest Wave outcome`
    );
  }
  if (card.closure_credit) {
    assert(
      card.current_disposition === "CLOSED" &&
        card.closure_record &&
        card.canonical_owner.status !== "UNKNOWN" &&
        typeof card.canonical_owner.path === "string" &&
        pathAtCommitExists("HEAD", card.canonical_owner.path),
      `${card.card_id} has closure credit without CLOSED record`
    );
    assert(
      [
        "current_sha_implementation_or_adjudication",
        "behavior_or_fault_proof",
        "independent_closure_review_same_sha",
        "capability_non_regression",
      ].every(key => card.closure_requirements[key] === true),
      `${card.card_id} closure requirements are not satisfied`
    );
    const requiredClosureDimensions = [
      "CURRENT_SHA_IMPLEMENTATION_OR_ADJUDICATION",
      ...card.exact_next_proof.required_dimensions.filter(
        dimension => dimension !== "INDEPENDENT_CLOSURE" && dimension !== "ABSENCE_IF_APPLICABLE"
      ),
    ];
    const closure = card.closure_record;
    for (const field of ledger.state_transition_contracts.closure_record_required_fields) {
      assert(field in closure, `${card.card_id} closure misses ${field}`);
    }
    assert(
      isCommitAncestorOfHead(closure.implementation_sha) &&
        isCommitAncestorOfHead(closure.evidence_head_sha) &&
        card.canonical_owner.status === "SOURCE_CANDIDATE" &&
        pathAtCommitExists(closure.evidence_head_sha, card.canonical_owner.path) &&
        canGit(
          "merge-base",
          "--is-ancestor",
          closure.implementation_sha,
          closure.evidence_head_sha
        ) !== null,
      `${card.card_id} closure implementation/evidence SHA chain is invalid`
    );
    if (card.exact_next_proof.required_dimensions.includes("SOURCE_MAP")) {
      assert(
        card.source_evidence.status === "SOURCE-CONFIRMED" && card.source_evidence.refs.length > 0,
        `${card.card_id} closed without a current source map`
      );
      for (const ref of card.source_evidence.refs) {
        validateRefAtCommit(closure.evidence_head_sha, ref, `${card.card_id} closure source ref`);
      }
    }
    if (card.exact_next_proof.required_dimensions.includes("ABSENCE_IF_APPLICABLE")) {
      const adjudication = closure.absence_applicability;
      for (const field of ledger.state_transition_contracts.absence_applicability_required_fields) {
        assert(field in adjudication, `${card.card_id} absence applicability misses ${field}`);
      }
      assert(
        ["REQUIRED", "NOT_APPLICABLE"].includes(adjudication.decision) &&
          adjudication.subject_sha === closure.evidence_head_sha &&
          isCommitAncestorOfHead(adjudication.subject_sha) &&
          typeof adjudication.reason === "string" &&
          adjudication.reason.trim() &&
          typeof adjudication.producer_id === "string" &&
          adjudication.producer_id.trim() &&
          typeof adjudication.reviewer_id === "string" &&
          adjudication.reviewer_id.trim() &&
          adjudication.producer_id !== adjudication.reviewer_id &&
          typeof adjudication.artifact_or_record === "string" &&
          adjudication.artifact_or_record.trim() &&
          card.closure_requirements.expected_absence_if_applicable === true,
        `${card.card_id} closed without same-SHA independent absence-applicability adjudication`
      );
      if (adjudication.decision === "REQUIRED") {
        requiredClosureDimensions.push("ABSENCE");
      }
    } else {
      assert(
        closure.absence_applicability === null,
        `${card.card_id} invented absence applicability for a non-applicable proof contract`
      );
    }
    const closedOutcome = card.wave_outcome_history.find(outcome => outcome.status === "CLOSED");
    assert(
      closedOutcome?.execution_sha === closure.implementation_sha,
      `${card.card_id} CLOSED outcome and closure SHA disagree`
    );
    validateLedgerEvidenceRecords(
      closure.credited_evidence_records,
      ledger.state_transition_contracts.wave_outcome_evidence_record_required_fields,
      `${card.card_id} credited closure`,
      true,
      requiredClosureDimensions,
      closure.evidence_head_sha,
      card.guard_ids,
      card.canonical_owner.path,
      card.card_id
    );
    validateLedgerEvidenceRecords(
      closure.behavior_or_fault_evidence,
      ledger.state_transition_contracts.wave_outcome_evidence_record_required_fields,
      `${card.card_id} closure behavior/fault`,
      true,
      ["BEHAVIOR_OR_FAULT"],
      closure.evidence_head_sha,
      card.guard_ids,
      card.canonical_owner.path,
      card.card_id
    );
    const review = closure.independent_review;
    for (const field of ledger.state_transition_contracts.independent_review_required_fields) {
      assert(field in review, `${card.card_id} independent review misses ${field}`);
    }
    assert(
      review.outcome === "PASS" &&
        review.reviewed_head_sha === closure.evidence_head_sha &&
        ["INDEPENDENT_AGENT_CHALLENGE", "GITHUB_FORMAL_REVIEW"].includes(review.review_type) &&
        typeof closure.implementation_author_id === "string" &&
        closure.implementation_author_id.trim() &&
        typeof review.reviewer_id === "string" &&
        review.reviewer_id.trim() &&
        review.reviewer_id !== closure.implementation_author_id &&
        typeof review.artifact_or_record === "string" &&
        review.artifact_or_record.trim(),
      `${card.card_id} closure lacks same-SHA independent review`
    );
    validateLedgerEvidenceRecords(
      closure.capability_non_regression,
      ledger.state_transition_contracts.wave_outcome_evidence_record_required_fields,
      `${card.card_id} closure non-regression`,
      true,
      ["NON_REGRESSION"],
      closure.evidence_head_sha,
      card.guard_ids,
      card.canonical_owner.path,
      card.card_id
    );
  } else {
    assert(card.closure_record === null, `${card.card_id} has uncredited closure record`);
    assert(
      card.current_disposition !== "CLOSED" &&
        !card.wave_outcome_history.some(outcome => outcome.status === "CLOSED"),
      `${card.card_id} has an uncredited CLOSED state`
    );
  }
}
sameSet(
  cards.filter(card => card.feature_reopen.global_blocker).map(card => card.card_id),
  [...wave0Ids, ...wave1Ids],
  "Global feature-reopen blockers"
);
sameSet(
  cards.filter(card => card.prerequisite_slice_ids.length > 0).map(card => card.card_id),
  ["BR4-D064"],
  "Prerequisite-bound card set"
);

for (const fact of baselineFacts) {
  for (const ref of fact.initial_source_evidence.refs) {
    validateRefAtCommit(baseline.sha, ref, `${fact.card_id} source ref`);
  }
  if (fact.initial_canonical_owner.path) {
    assert(
      pathAtCommitExists(baseline.sha, fact.initial_canonical_owner.path),
      `${fact.card_id} initial owner path is absent at review baseline`
    );
  }
}

const reproducedPartialIds = [
  "BR4-D032",
  "BR4-D064",
  "BR4-D067",
  "BR4-D068",
  "BR4-D069",
  "BR4-D070",
  "BR4-D071",
  "BR4-D072",
  "R4-FLAKY-001",
];
for (const cardId of reproducedPartialIds) {
  const fact = baselineFactById.get(cardId);
  assert(
    fact.initial_behavior_evidence.status === "REPRODUCED" &&
      fact.initial_behavior_evidence.coverage === "PARTIAL",
    `${cardId} lost bounded R4 behavior evidence`
  );
}
for (const cardId of ["R3-N010", "R3-N020"]) {
  const fact = baselineFactById.get(cardId);
  assert(
    fact.initial_behavior_evidence.status === "REPRODUCED" &&
      fact.initial_behavior_evidence.coverage === "COMPLETE",
    `${cardId} lost reproduced frontend evidence`
  );
}
assert(
  baselineFactById.get("R4-FLAKY-001").origin.discovery_evidence_status === "REPRODUCED",
  "R4-FLAKY-001 discovery evidence is mislabeled"
);
assert(
  !cardIds.includes("R4-NATIVE-001") &&
    !cardIds.includes("R3-N011") &&
    !cardIds.includes("R3-N021"),
  "Merged or withdrawn records entered the living card set"
);
const nativeMerge = ledger.merge_records.find(record => record.record_id === "R4-NATIVE-001");
assert(
  nativeMerge?.target_card_id === "BR4-D064" &&
    nativeMerge.creates_new_card === false &&
    nativeMerge.closure_effect === "NONE",
  "R4-NATIVE-001 merge record is invalid"
);
const d064 = cards.find(card => card.card_id === "BR4-D064");
assert(
  d064.initial_assigned_wave === "WAVE-1" && d064.prerequisite_slice_ids.includes("W0-S3"),
  "BR4-D064 assignment or prerequisite drifted"
);

assert(program.decision.v4_facts.v4_unique_commits === 13, "V4 unique commit count drifted");
sameJson(
  program.decision.v4_facts.classification,
  { integrated: 4, superseded: 8, evidence_only: 1, still_needed_port: 0 },
  "V4 classification"
);
assert(
  program.decision.v4_facts.closure_credit === "UNKNOWN" && !("confidence" in program.decision),
  "V4 closure or Program decision is overclaimed"
);

for (const requiredPath of [
  "AGENTS.md",
  "plans/README.md",
  "plans/openlife_single_system_deletion_manifest.md",
  "plans/openlife_single_system_development_preparation.md",
  "plans/openlife_restart_baseline_cleanup.json",
  programMarkdownPath,
  programPath,
  ledgerPath,
  "plans/openlife_single_system_phase1_inventory.json",
]) {
  assert(existsSync(join(repositoryRoot, requiredPath)), `Missing active path ${requiredPath}`);
}
const phase1Inventory = readJson("plans/openlife_single_system_phase1_inventory.json");
const globalExpectedAbsentPaths = [...new Set(collectExpectedAbsentPaths(phase1Inventory))].sort();
assert(
  globalExpectedAbsentPaths.length === 44 &&
    newlineIdDigest(globalExpectedAbsentPaths) === EXPECTED_ABSENT_PATH_HASH,
  "Global expected-absent path registry drifted"
);
for (const path of globalExpectedAbsentPaths) {
  assert(
    !existsSync(join(repositoryRoot, path)),
    `Global expected-absent path was recreated: ${path}`
  );
}
const activeAuthority = [
  readText("AGENTS.md"),
  readText("plans/README.md"),
  readText("plans/openlife_single_system_deletion_manifest.md"),
  readText("plans/openlife_single_system_development_preparation.md"),
  programMarkdown,
].join("\n");
for (const stalePhrase of [
  "The current entry is the restart-baseline cleanup",
  "next formal full-repo review",
  "next formal full-repository review",
  "After the cleanup PR is merged",
]) {
  assert(!activeAuthority.includes(stalePhrase), `Stale wording remains: ${stalePhrase}`);
}
assert(
  activeAuthority.includes("red-until-trial-green"),
  "Phase7 red-until-trial-green boundary disappeared"
);
assert(
  !programMarkdown.includes("agent-notes/") &&
    !JSON.stringify(program).includes("agent-notes/") &&
    !JSON.stringify(ledger).includes("agent-notes/"),
  "Tracked active Program depends on ignored agent-notes paths"
);
for (const requiredMarkdownFact of [
  "Program activation is a **pre-Wave transition**",
  "W0-S4 — Wave-0 Ledger Reconciliation",
  "### Bounded Feature Eligibility",
  "### Normal Feature Development Reopen",
  "--profile=draft",
  "--profile=activation",
  "--profile=ongoing",
  "--task-packet=<FROZEN_PACKET_PATH>",
  "required_guard_ids:",
  "warning_churn_lines:",
  "hard_stop_churn_lines:",
  "new_worktree: false",
  "risk_class: LOW | MEDIUM | HIGH",
  "PREDECESSOR_RECEIPT_RETAINED_NOT_DISPATCHABLE",
  "W0-S3 receipt recovery is the next packet-preparation subject.",
  "The retained W0-S1 and W0-S2 receipts are replayed against their original",
  "no evidence may be poured back into the old range",
]) {
  assert(
    programMarkdown.includes(requiredMarkdownFact),
    `Program Markdown misses machine fact: ${requiredMarkdownFact}`
  );
}
assert(
  !programMarkdown.includes("dispatches **W0-S1") &&
    !programMarkdown.includes("W0-S1 blueprint ready") &&
    !programMarkdown.includes("W0-S1 must match its inline blueprint"),
  "Program Markdown still instructs Agents to redispatch settled W0-S1"
);
for (const slice of w0.slices) {
  assert(
    programMarkdown.includes(`### ${slice.id} — ${slice.name}`),
    `${slice.id} Markdown/JSON name mismatch`
  );
}

const planningPaths = [
  "AGENTS.md",
  "plans/README.md",
  "plans/openlife_single_system_deletion_manifest.md",
  "plans/openlife_single_system_development_preparation.md",
  programMarkdownPath,
  programPath,
  ledgerPath,
  "scripts/validate-current-development-program.mjs",
  "scripts/test-current-development-program-validator.mjs",
];
const trackedDirtyPaths = uniquePaths(
  gitNul("diff", "--name-only", "-z", "--no-renames", "--"),
  gitNul("diff", "--cached", "--name-only", "-z", "--no-renames", "--")
);
const untrackedPaths = gitNul("ls-files", "--others", "--exclude-standard", "-z");
const dirtyPaths = uniquePaths(trackedDirtyPaths, untrackedPaths);

const deriveAndValidateActivationCommit = approvedDraft => {
  const ancestry = git("rev-list", "--ancestry-path", "--reverse", `${approvedDraft}..HEAD`)
    .split("\n")
    .filter(Boolean);
  assert(ancestry.length > 0, "Approved draft has no activation descendant");
  const activationSha = ancestry[0];
  const activationParents = git("rev-list", "--parents", "-n", "1", activationSha).split(" ");
  assert(
    activationParents.length === 2 && activationParents[1] === approvedDraft,
    "Activation is not a single-parent direct child of the approved draft"
  );
  sameSet(
    changedPathsBetween(approvedDraft, activationSha),
    [programPath, ledgerPath],
    "Derived activation-only changed paths"
  );
  const approvedProgram = readJsonAtCommit(approvedDraft, programPath);
  const approvedLedger = readJsonAtCommit(approvedDraft, ledgerPath);
  const activatedProgram = readJsonAtCommit(activationSha, programPath);
  const activatedLedger = readJsonAtCommit(activationSha, ledgerPath);
  assert(
    approvedProgram.status === "DRAFT_AWAITING_USER_APPROVAL" &&
      approvedProgram.execution_authorized === false &&
      approvedLedger.status === "DRAFT_AWAITING_USER_APPROVAL" &&
      approvedLedger.authority.execution_authorized === false,
    "Derived activation does not start from a fail-closed draft"
  );
  const approval = activatedProgram.program_approval;
  const challenge = approval.independent_challenge;
  assert(
    activatedProgram.status === "APPROVED_FOR_EXECUTION" &&
      activatedProgram.execution_authorized === true &&
      activatedLedger.status === "ACTIVE" &&
      activatedLedger.authority.execution_authorized === true &&
      approval.status === "APPROVED_BY_USER" &&
      approval.execution_authority_granted === true &&
      approval.approved_program_schema_version === activatedProgram.schema_version &&
      approval.approved_draft_commit_sha === approvedDraft &&
      approval.approved_by === "USER" &&
      typeof approval.approved_at === "string" &&
      !Number.isNaN(Date.parse(approval.approved_at)) &&
      typeof approval.approval_record === "string" &&
      approval.approval_record.trim() &&
      challenge.status === "PASS" &&
      challenge.outcome === "PASS" &&
      challenge.reviewed_head_sha === approvedDraft &&
      challenge.reviewer_ids.length > 0 &&
      challenge.reviewer_ids.every(
        reviewerId =>
          typeof reviewerId === "string" &&
          reviewerId.trim() &&
          reviewerId !== approval.program_author_id
      ) &&
      typeof challenge.artifact_or_record === "string" &&
      challenge.artifact_or_record.trim() &&
      challenge.blocking_findings.length === 0,
    "Derived activation lacks exact user approval and independent challenge"
  );
  const activationGate = activatedProgram.gates.find(gate => gate.id === "G-PROGRAM-ACTIVATION");
  assert(
    activatedProgram.program_activation.status === "ACTIVE" &&
      activationGate.status === "PASS" &&
      activationGate.evidence_records.every(record => record.subject_sha === approvedDraft),
    "Derived activation gate is not bound to the approved draft"
  );
  validateCreditedEvidenceRecords({
    records: activationGate.evidence_records,
    requiredDimensions: activationGate.evidence_dimensions,
    requiredGuardIds: activationGate.required_guard_ids,
    expectedScopeId: activationGate.id,
    label: "derived Program activation",
  });
  assert(
    activatedProgram.waves[0].status === "READY" &&
      activatedProgram.waves.slice(1).every(wave => wave.status === "PLANNED_NOT_AUTHORIZED") &&
      activatedProgram.gates
        .filter(gate => gate.id !== "G-PROGRAM-ACTIVATION")
        .every(gate => gate.status === "NOT_RUN" && gate.evidence_records.length === 0),
    "Derived activation granted premature Wave or gate credit"
  );
  assert(
    canonicalDigest(activationSubstantiveProgram(activatedProgram)) ===
      canonicalDigest(activationSubstantiveProgram(approvedProgram)) &&
      canonicalDigest(activationSubstantiveLedger(activatedLedger)) ===
        canonicalDigest(activationSubstantiveLedger(approvedLedger)),
    "Derived activation changed substantive Program or ledger content"
  );
  return { activationSha, activatedProgram, activatedLedger };
};

const assertAuthorizedProgramState = () => {
  assert(
    ["APPROVED_FOR_EXECUTION", "ACTIVE"].includes(program.status) &&
      program.execution_authorized === true &&
      ledger.status === "ACTIVE" &&
      ledger.authority.execution_authorized === true,
    "Authorized state is incomplete"
  );
  const approval = program.program_approval;
  const approvedDraft = approval.approved_draft_commit_sha;
  assert(
    approval.status === "APPROVED_BY_USER" &&
      approval.execution_authority_granted === true &&
      approval.approved_program_schema_version === program.schema_version &&
      approval.approved_by === "USER" &&
      typeof approval.approved_at === "string" &&
      !Number.isNaN(Date.parse(approval.approved_at)) &&
      typeof approval.approval_record === "string" &&
      approval.approval_record.trim() &&
      isCommitAncestorOfHead(approvedDraft),
    "Authorized state lacks exact user approval"
  );
  const challenge = approval.independent_challenge;
  assert(
    challenge.status === "PASS" &&
      challenge.outcome === "PASS" &&
      challenge.reviewed_head_sha === approvedDraft &&
      challenge.reviewer_ids.length > 0 &&
      challenge.reviewer_ids.every(
        reviewerId =>
          typeof reviewerId === "string" &&
          reviewerId.trim() &&
          reviewerId !== approval.program_author_id
      ) &&
      typeof challenge.artifact_or_record === "string" &&
      challenge.artifact_or_record.trim() &&
      challenge.blocking_findings.length === 0 &&
      challenge.github_formal_approval_credit === false,
    "Authorized state lacks an independent exact-draft challenge"
  );
  assert(
    program.program_activation.status === "ACTIVE" &&
      gateById.get("G-PROGRAM-ACTIVATION").status === "PASS",
    "Authorized state lacks active Program gate"
  );
  const derivedActivation = deriveAndValidateActivationCommit(approvedDraft);
  assert(
    canonicalDigest(program.program_approval) ===
      canonicalDigest(derivedActivation.activatedProgram.program_approval) &&
      canonicalDigest(gateById.get("G-PROGRAM-ACTIVATION")) ===
        canonicalDigest(
          derivedActivation.activatedProgram.gates.find(gate => gate.id === "G-PROGRAM-ACTIVATION")
        ),
    "Approval or activation evidence drifted after activation"
  );
  const approvedProgram = readJsonAtCommit(approvedDraft, programPath);
  const approvedLedger = readJsonAtCommit(approvedDraft, ledgerPath);
  assert(
    canonicalDigest(livingSubstantiveProgram(program)) ===
      canonicalDigest(livingSubstantiveProgram(approvedProgram)),
    "Living Program changed an approved substantive contract"
  );
  for (const key of [
    "schema_version",
    "ledger_id",
    "predecessor_snapshot",
    "review_baseline",
    "provenance",
    "vocabularies",
    "state_transition_contracts",
    "integration_contract",
    "receipt_adjudication_contract",
    "attempt_contract",
    "initial_inventory",
    "root_cause_clusters",
    "closure_policy",
    "merge_records",
    "new_card_contract",
  ]) {
    assert(
      canonicalDigest(ledger[key]) === canonicalDigest(approvedLedger[key]),
      `Living ledger changed approved policy section: ${key}`
    );
  }
  const currentLedgerAuthority = clone(ledger.authority);
  const approvedLedgerAuthority = clone(approvedLedger.authority);
  currentLedgerAuthority.execution_authorized = "__LIVING_AUTHORITY_STATE__";
  approvedLedgerAuthority.execution_authorized = "__LIVING_AUTHORITY_STATE__";
  assert(
    canonicalDigest(currentLedgerAuthority) === canonicalDigest(approvedLedgerAuthority),
    "Living ledger changed approved authority policy"
  );
  const approvedCardById = new Map(approvedLedger.cards.map(card => [card.card_id, card]));
  for (const card of cards) {
    const approvedCard = approvedCardById.get(card.card_id);
    if (!approvedCard) continue;
    for (const field of [
      "proof_id",
      "statement",
      "requires_current_sha",
      "required_dimensions",
      "blocks_closure",
    ]) {
      assert(
        canonicalDigest(card.exact_next_proof[field]) ===
          canonicalDigest(approvedCard.exact_next_proof[field]),
        `${card.card_id} weakened an approved exact-next-proof contract`
      );
    }
    if (card.wave_outcome_history.length === 0) {
      assert(
        canonicalDigest(livingCardStateProjection(card)) ===
          canonicalDigest(livingCardStateProjection(approvedCard)),
        `${card.card_id} living state changed without a Wave outcome`
      );
    }
  }
  const historicalNewCardIds = new Set();
  const earliestNewCardRecordById = new Map();
  const earliestAttemptRecordById = new Map();
  const earliestArchitectureReviewById = new Map();
  const earliestIntegrationRecordById = new Map();
  const earliestWaveOutcomeById = new Map();
  for (const ledgerSha of [
    approvedDraft,
    ...git("rev-list", "--reverse", `${approvedDraft}..HEAD`, "--", ledgerPath)
      .split("\n")
      .filter(Boolean),
  ]) {
    const historicalLedger = readJsonAtCommit(ledgerSha, ledgerPath);
    for (const field of [
      "integration_records",
      "implementation_attempt_records",
      "architecture_review_records",
      "receipt_adjudication_records",
    ]) {
      const historicalRecords = historicalLedger[field] ?? [];
      const currentRecords = ledger[field] ?? [];
      assert(
        historicalRecords.length <= currentRecords.length &&
          historicalRecords.every(
            (record, index) => canonicalDigest(record) === canonicalDigest(currentRecords[index])
          ),
        `Append-only ledger sequence drifted: ${field}`
      );
    }
    for (const historicalCard of historicalLedger.cards) {
      const currentCard = cards.find(candidate => candidate.card_id === historicalCard.card_id);
      assert(
        currentCard &&
          historicalCard.wave_outcome_history.length <= currentCard.wave_outcome_history.length &&
          historicalCard.wave_outcome_history.every(
            (outcome, index) =>
              canonicalDigest(outcome) === canonicalDigest(currentCard.wave_outcome_history[index])
          ),
        `Append-only Wave outcome history drifted: ${historicalCard.card_id}`
      );
      for (const historicalOutcome of historicalCard.wave_outcome_history) {
        if (!earliestWaveOutcomeById.has(historicalOutcome.outcome_id)) {
          earliestWaveOutcomeById.set(historicalOutcome.outcome_id, {
            ledgerSha,
            cardId: historicalCard.card_id,
            outcome: historicalOutcome,
          });
        }
      }
      if (/^CUR-N\d{3,}$/.test(historicalCard.card_id)) {
        historicalNewCardIds.add(historicalCard.card_id);
      }
    }
    for (const historicalRecord of historicalLedger.new_card_creation_records ?? []) {
      if (!earliestNewCardRecordById.has(historicalRecord.card_id)) {
        earliestNewCardRecordById.set(historicalRecord.card_id, {
          ledgerSha,
          record: historicalRecord,
        });
      }
    }
    for (const historicalAttempt of historicalLedger.implementation_attempt_records ?? []) {
      if (!earliestAttemptRecordById.has(historicalAttempt.attempt_id)) {
        earliestAttemptRecordById.set(historicalAttempt.attempt_id, {
          ledgerSha,
          record: historicalAttempt,
        });
      }
    }
    for (const historicalReview of historicalLedger.architecture_review_records ?? []) {
      if (!earliestArchitectureReviewById.has(historicalReview.review_id)) {
        earliestArchitectureReviewById.set(historicalReview.review_id, {
          ledgerSha,
          record: historicalReview,
        });
      }
    }
    for (const historicalIntegration of historicalLedger.integration_records ?? []) {
      if (!earliestIntegrationRecordById.has(historicalIntegration.record_id)) {
        earliestIntegrationRecordById.set(historicalIntegration.record_id, {
          ledgerSha,
          record: historicalIntegration,
        });
      }
    }
  }
  for (const [
    attemptId,
    { ledgerSha: introductionSha, record: earliestRecord },
  ] of earliestAttemptRecordById) {
    const currentRecord = ledger.implementation_attempt_records.find(
      record => record.attempt_id === attemptId
    );
    assert(
      currentRecord && canonicalDigest(currentRecord) === canonicalDigest(earliestRecord),
      `Append-only attempt record disappeared/drifted: ${attemptId}`
    );
    assert(
      canGit("merge-base", "--is-ancestor", earliestRecord.artifact_commit_sha, introductionSha) !==
        null && earliestRecord.artifact_commit_sha !== introductionSha,
      `Attempt ${attemptId} record does not follow its artifact commit`
    );
  }
  for (const [reviewId, { record: earliestReview }] of earliestArchitectureReviewById) {
    const currentReview = ledger.architecture_review_records.find(
      record => record.review_id === reviewId
    );
    assert(
      currentReview && canonicalDigest(currentReview) === canonicalDigest(earliestReview),
      `Append-only architecture review disappeared/drifted: ${reviewId}`
    );
  }
  for (const [
    integrationId,
    { ledgerSha: introductionSha, record: earliestIntegration },
  ] of earliestIntegrationRecordById) {
    const currentIntegration = ledger.integration_records.find(
      record => record.record_id === integrationId
    );
    assert(
      currentIntegration &&
        canonicalDigest(currentIntegration) === canonicalDigest(earliestIntegration) &&
        canGit(
          "merge-base",
          "--is-ancestor",
          earliestIntegration.range_head_sha,
          introductionSha
        ) !== null &&
        earliestIntegration.range_head_sha !== introductionSha,
      `Append-only integration record disappeared/drifted: ${integrationId}`
    );
  }
  for (const [
    outcomeId,
    { ledgerSha: introductionSha, cardId, outcome: earliestOutcome },
  ] of earliestWaveOutcomeById) {
    const currentCard = cards.find(card => card.card_id === cardId);
    const currentOutcome = currentCard?.wave_outcome_history.find(
      outcome => outcome.outcome_id === outcomeId
    );
    assert(
      currentOutcome &&
        canonicalDigest(currentOutcome) === canonicalDigest(earliestOutcome) &&
        canGit("merge-base", "--is-ancestor", earliestOutcome.execution_sha, introductionSha) !==
          null &&
        earliestOutcome.execution_sha !== introductionSha &&
        (!earliestOutcome.resulting_live_fact.closure_record ||
          (canGit(
            "merge-base",
            "--is-ancestor",
            earliestOutcome.resulting_live_fact.closure_record.evidence_head_sha,
            introductionSha
          ) !== null &&
            earliestOutcome.resulting_live_fact.closure_record.evidence_head_sha !==
              introductionSha)),
      `Append-only Wave outcome was fabricated, removed or rewritten: ${outcomeId}`
    );
  }
  for (const review of ledger.architecture_review_records) {
    const earliestReview = earliestArchitectureReviewById.get(review.review_id);
    assert(
      earliestReview &&
        canGit(
          "merge-base",
          "--is-ancestor",
          review.review_artifact_commit_sha,
          earliestReview.ledgerSha
        ) !== null &&
        review.review_artifact_commit_sha !== earliestReview.ledgerSha,
      `Architecture review ${review.review_id} is not recorded after its reviewed subject`
    );
    for (const attemptId of review.covered_failed_attempt_ids) {
      const introduction = earliestAttemptRecordById.get(attemptId);
      assert(
        introduction &&
          canGit(
            "merge-base",
            "--is-ancestor",
            introduction.ledgerSha,
            review.review_artifact_commit_sha
          ) !== null,
        `Architecture review ${review.review_id} predates attempt record ${attemptId}`
      );
    }
  }
  for (const cardId of historicalNewCardIds) {
    const currentCard = cards.find(card => card.card_id === cardId);
    const currentRecord = ledger.new_card_creation_records.find(
      record => record.card_id === cardId
    );
    const earliestRecordEntry = earliestNewCardRecordById.get(cardId);
    const earliestRecord = earliestRecordEntry?.record;
    assert(
      currentCard &&
        currentRecord &&
        earliestRecord &&
        canonicalDigest(currentRecord) === canonicalDigest(earliestRecord) &&
        canGit(
          "merge-base",
          "--is-ancestor",
          earliestRecord.creation_sha,
          earliestRecordEntry.ledgerSha
        ) !== null &&
        earliestRecord.creation_sha !== earliestRecordEntry.ledgerSha,
      `Append-only new card or creation record disappeared/drifted: ${cardId}`
    );
  }
  for (const path of [
    "AGENTS.md",
    "plans/README.md",
    "plans/openlife_single_system_deletion_manifest.md",
    "plans/openlife_single_system_development_preparation.md",
    "plans/openlife_single_system_phase1_inventory.json",
    "plans/openlife_restart_baseline_cleanup.json",
    programMarkdownPath,
    "scripts/validate-current-development-program.mjs",
    "scripts/test-current-development-program-validator.mjs",
  ]) {
    assert(
      readText(path) === readTextAtCommit(approvedDraft, path),
      `Approved authority/validator changed without a new Program approval: ${path}`
    );
  }
  return derivedActivation.activationSha;
};
const validateFrozenPacketEnvelope = (packet, label) => {
  const serializedPacket = JSON.stringify(packet);
  assert(
    Buffer.byteLength(serializedPacket, "utf8") <=
      ledger.attempt_contract.artifact_budget.max_packet_bytes &&
      ledger.attempt_contract.forbidden_artifact_markers.every(
        marker => !serializedPacket.includes(marker)
      ),
    `${label} exceeds the packet artifact budget or contains a forbidden secret marker`
  );
  for (const requiredField of program.agent_task_contract.required_fields) {
    assert(requiredField in packet, `${label} misses ${requiredField}`);
  }
  sameSet(
    Object.keys(packet),
    program.agent_task_contract.required_fields,
    `${label} top-level keys`
  );
  const packetPayload = clone(packet);
  packetPayload.packet_sha256 = null;
  packetPayload.packet_payload_sha256 = null;
  packetPayload.packet_freeze_review = null;
  const payloadDigest = canonicalDigest(packetPayload);
  const packetForDigest = clone(packet);
  packetForDigest.packet_sha256 = null;
  const review = packet.packet_freeze_review;
  assert(
    packet.packet_status === "FROZEN_FOR_DISPATCH" &&
      packet.packet_payload_sha256 === payloadDigest &&
      review?.outcome === "PASS" &&
      review.reviewed_payload_sha256 === payloadDigest &&
      typeof review.integrator_id === "string" &&
      review.integrator_id.trim() &&
      typeof review.reviewer_id === "string" &&
      review.reviewer_id.trim() &&
      review.integrator_id !== review.reviewer_id &&
      typeof review.artifact_or_record === "string" &&
      review.artifact_or_record.trim() &&
      packet.packet_sha256 === canonicalDigest(packetForDigest),
    `${label} lacks a canonical independent freeze review`
  );
  return packet.packet_sha256;
};
const validateFrozenPacketSemantics = ({
  packet,
  programState,
  ledgerState,
  executionBaseline,
  expectedSlice,
  requireLiveCheckout = false,
  label,
}) => {
  for (const [field, allowedValues] of [
    ["mode", programState.agent_task_contract.allowed_modes],
    ["risk_class", ["LOW", "MEDIUM", "HIGH"]],
  ]) {
    assert(allowedValues.includes(packet[field]), `${label} has invalid ${field}`);
  }
  for (const field of [
    "task_id",
    "slice_id",
    "slice_exit_contract_id",
    "objective",
    "invariant",
    "minimal_fix_contract",
    "old_path_deletion_contract",
    "ledger_update_contract",
    "handoff_contract",
  ]) {
    assert(
      typeof packet[field] === "string" && packet[field].trim(),
      `${label} has empty ${field}`
    );
  }
  assert(
    Array.isArray(packet.preimplementation_proof_ids) &&
      new Set(packet.preimplementation_proof_ids).size ===
        packet.preimplementation_proof_ids.length &&
      packet.preimplementation_proof_ids.every(
        proofId => typeof proofId === "string" && proofId.trim()
      ),
    `${label} has invalid preimplementation proof IDs`
  );
  assert(
    (packet.architecture_review_id === null ||
      (typeof packet.architecture_review_id === "string" &&
        packet.architecture_review_id.trim())) &&
      Array.isArray(packet.architecture_review_constraints) &&
      new Set(packet.architecture_review_constraints).size ===
        packet.architecture_review_constraints.length &&
      packet.architecture_review_constraints.every(
        constraint => typeof constraint === "string" && constraint.trim()
      ),
    `${label} has invalid architecture-review binding`
  );
  for (const field of [
    "non_goals",
    "source_map",
    "allowed_touched_paths",
    "forbidden_touched_paths",
    "red_contract",
    "verification_commands",
    "required_evidence_dimensions",
    "acceptance_criteria",
    "stop_conditions",
  ]) {
    assert(
      Array.isArray(packet[field]) &&
        packet[field].length > 0 &&
        packet[field].every(value => typeof value === "string" && value.trim()),
      `${label} has empty ${field}`
    );
  }
  assert(
    programState.agent_task_contract.required_stop_conditions.every(condition =>
      packet.stop_conditions.includes(condition)
    ),
    `${label} dropped a mandatory stop condition`
  );
  const verificationCommandContract =
    programState.agent_task_contract.verification_command_contract;
  const allowedVerificationPatterns = verificationCommandContract.allowed_patterns.map(
    pattern => new RegExp(pattern)
  );
  for (const command of packet.verification_commands) {
    assert(
      command.trim() === command &&
        !command.includes("..") &&
        verificationCommandContract.forbidden_fragments.every(
          fragment => !command.includes(fragment)
        ) &&
        allowedVerificationPatterns.some(pattern => pattern.test(command)),
      `${label} has a non-allowlisted or unsafe verification command: ${command}`
    );
  }
  const budget = packet.budget;
  assert(
    budget.warning_files === programState.slice_contract.warning_budget.files &&
      budget.warning_churn_lines === programState.slice_contract.warning_budget.churn_lines &&
      budget.hard_stop_files === programState.slice_contract.hard_stop_budget.files &&
      budget.hard_stop_churn_lines === programState.slice_contract.hard_stop_budget.churn_lines,
    `${label} changed the approved size budget`
  );
  sameSet(
    Object.keys(packet.review_contract),
    Object.keys(programState.agent_task_contract.review_contract_schema),
    `${label} review-contract keys`
  );
  assert(
    packet.review_contract.risk_class === packet.risk_class &&
      packet.review_contract.independent_challenge_required === true &&
      packet.review_contract.reviewed_head_sha === null &&
      packet.review_contract.review_type === "INDEPENDENT_AGENT_CHALLENGE" &&
      packet.review_contract.review_artifact === null &&
      packet.review_contract.github_formal_review_count === 0 &&
      packet.review_contract.github_formal_approval_credit === false,
    `${label} review contract is not fail closed`
  );
  sameSet(
    Object.keys(packet.external_action_policy),
    Object.keys(programState.agent_task_contract.defaults),
    `${label} external-action keys`
  );
  assert(
    Object.entries(programState.agent_task_contract.defaults).every(
      ([key, expected]) => packet.external_action_policy[key] === expected
    ) && packet.closure_claim_allowed === false,
    `${label} changed external-action or closure authority`
  );
  const packetWave = programState.waves.find(wave => wave.id === packet.wave_id);
  assert(
    packet.program_schema_version === programState.schema_version &&
      packet.slice_id === expectedSlice &&
      packetWave &&
      packetWave.id !== "WAVE-5" &&
      ["READY", "IN_PROGRESS"].includes(packetWave.status),
    `${label} Program/Wave/slice binding is invalid`
  );
  const declaredSlice = packetWave.slices.find(candidate => candidate.id === expectedSlice);
  const unresolvedInvalidatedReceipts = ledgerState.integration_records
    .filter(record => invalidatedReceiptIds.has(record.record_id))
    .filter(
      invalidated =>
        !ledgerState.integration_records.some(
          candidate =>
            !invalidatedReceiptIds.has(candidate.record_id) &&
            recoveryReceiptSatisfiesFrozenScope({
              candidate,
              invalidatedTarget: invalidated,
              programState,
              ledgerState,
              snapshotSha: executionBaseline,
            })
        )
    );
  const unresolvedInvalidatedSlices = new Set(
    unresolvedInvalidatedReceipts.map(record => record.slice_id)
  );
  if (unresolvedInvalidatedSlices.size > 0) {
    assert(
      packet.slice_id === "W0-S3" && packet.mode === "VERIFICATION",
      `${label} cannot bypass unresolved receipt recovery`
    );
    const invalidatedTarget = unresolvedInvalidatedReceipts.find(
      record => record.slice_id === packet.slice_id
    );
    const invalidatedPacket = readJsonAtCommit(
      executionBaseline,
      invalidatedTarget.packet_artifact_path
    );
    assert(
      recoveryPacketMatchesFrozenScope(packet, invalidatedPacket),
      `${label} does not reproduce the frozen predecessor packet contract`
    );
  }
  if (declaredSlice) {
    assert(
      Array.isArray(declaredSlice.predecessor_slice_ids),
      `${label} declared slice lacks predecessor metadata`
    );
    for (const predecessorSliceId of declaredSlice.predecessor_slice_ids) {
      const predecessorRecords = ledgerState.integration_records.filter(
        record =>
          record.slice_id === predecessorSliceId &&
          !invalidatedReceiptIds.has(record.record_id) &&
          predecessorReceiptLineage(record, programState)
      );
      assert(
        predecessorRecords.length === 1,
        `${label} predecessor slice lacks exactly one effective receipt: ${predecessorSliceId}`
      );
      const [predecessorRecord] = predecessorRecords;
      const receiptLineage = predecessorReceiptLineage(predecessorRecord, programState);
      validateIntegrationRecordReceipt({
        record: predecessorRecord,
        programState,
        ledgerState,
        snapshotSha: executionBaseline,
        expectedActivationSha:
          receiptLineage.kind === "CURRENT_PROGRAM"
            ? packet.program_activation_sha
            : receiptLineage.activation_sha,
        label: `${label} predecessor ${predecessorSliceId}`,
      });
    }
    if (["IMPLEMENTATION", "VERIFICATION_THEN_CONDITIONAL_IMPLEMENTATION"].includes(packet.mode)) {
      assert(
        !ledgerState.integration_records.some(
          record =>
            record.slice_id === packet.slice_id && !invalidatedReceiptIds.has(record.record_id)
        ),
        `${label} implementation slice was already integrated`
      );
    }
  }
  if (["IMPLEMENTATION", "VERIFICATION_THEN_CONDITIONAL_IMPLEMENTATION"].includes(packet.mode)) {
    const coveredAttemptIds = new Set(
      ledgerState.architecture_review_records.flatMap(review => review.covered_failed_attempt_ids)
    );
    const uncoveredFailedAttempts = ledgerState.implementation_attempt_records.filter(
      attempt =>
        attempt.root_cause_cluster_id === packet.root_cause_cluster_id &&
        attempt.outcome === ledgerState.attempt_contract.counted_failure_outcome &&
        !coveredAttemptIds.has(attempt.attempt_id)
    );
    assert(
      uncoveredFailedAttempts.length <
        ledgerState.attempt_contract.failed_attempt_limit_before_review,
      `${label} cannot dispatch after three failed attempts without architecture review`
    );
    const clusterReviews = ledgerState.architecture_review_records.filter(
      review => review.root_cause_cluster_id === packet.root_cause_cluster_id
    );
    const latestReview = clusterReviews.at(-1);
    const latestReviewArtifact = latestReview
      ? JSON.parse(
          readTextAtCommit(
            latestReview.review_artifact_commit_sha,
            latestReview.review_artifact_path
          )
        )
      : null;
    const latestReviewWasConsumed =
      latestReview &&
      ledgerState.implementation_attempt_records.some(attempt => {
        if (
          attempt.root_cause_cluster_id !== packet.root_cause_cluster_id ||
          attempt.artifact_commit_sha === latestReview.review_artifact_commit_sha ||
          canGit(
            "merge-base",
            "--is-ancestor",
            latestReview.review_artifact_commit_sha,
            attempt.artifact_commit_sha
          ) === null
        ) {
          return false;
        }
        const attemptPacket = readJsonAtCommit(
          attempt.artifact_commit_sha,
          attempt.packet_artifact_path
        );
        return (
          ["IMPLEMENTATION", "VERIFICATION_THEN_CONDITIONAL_IMPLEMENTATION"].includes(
            attemptPacket.mode
          ) &&
          attemptPacket.architecture_review_id === latestReview.review_id &&
          canonicalDigest([...attemptPacket.architecture_review_constraints].sort()) ===
            canonicalDigest([...latestReviewArtifact.next_attempt_constraints].sort())
        );
      });
    const latestReviewIsPending = latestReview && !latestReviewWasConsumed;
    if (latestReviewIsPending) {
      assert(
        packet.architecture_review_id === latestReview.review_id,
        `${label} does not bind the latest architecture review`
      );
      sameSet(
        packet.architecture_review_constraints,
        latestReviewArtifact.next_attempt_constraints,
        `${label} architecture-review constraints`
      );
    } else {
      assert(
        packet.architecture_review_id === null &&
          packet.architecture_review_constraints.length === 0,
        `${label} invented or reused an architecture-review reset`
      );
    }
  } else {
    assert(
      packet.architecture_review_id === null && packet.architecture_review_constraints.length === 0,
      `${label} non-implementation task claimed an architecture-review reset`
    );
  }
  if (declaredSlice?.task_packet_blueprint) {
    const normalizedPacket = clone(packet);
    const normalizedBlueprint = clone(declaredSlice.task_packet_blueprint);
    for (const target of [normalizedPacket, normalizedBlueprint]) {
      target.packet_status = null;
      target.packet_sha256 = null;
      target.packet_payload_sha256 = null;
      target.packet_freeze_review = null;
      target.program_activation_sha = null;
      target.execution_baseline_sha = null;
      target.expected_parent_main_sha = null;
      target.branch = null;
      target.assigned_agent_id = null;
      target.architecture_review_id = null;
      target.architecture_review_constraints = [];
    }
    assert(
      canonicalDigest(normalizedPacket) === canonicalDigest(normalizedBlueprint),
      `${label} diverges from the approved slice blueprint`
    );
  } else if (declaredSlice) {
    sameSet(packet.finding_ids, declaredSlice.finding_ids, `${label} declared finding set`);
    sameSet(
      packet.required_guard_ids,
      declaredSlice.required_guard_ids,
      `${label} declared guard set`
    );
    assert(
      packet.objective === declaredSlice.objective &&
        packet.governance_task === declaredSlice.governance_task &&
        packet.risk_class === declaredSlice.risk_class &&
        packet.root_cause_cluster_id === declaredSlice.root_cause_cluster_id &&
        packet.slice_exit_contract_id === declaredSlice.exit_contract_id &&
        declaredSlice.red_contract.every(redId => packet.red_contract.includes(redId)) &&
        declaredSlice.non_goals.every(nonGoal => packet.non_goals.includes(nonGoal)) &&
        packet.acceptance_criteria.includes(declaredSlice.exit),
      `${label} diverges from the declared slice`
    );
  } else {
    assert(
      packetWave.slices.length === 0 &&
        packet.finding_ids.length > 0 &&
        packet.governance_task === false &&
        packet.slice_exit_contract_id === "GENERIC_FINDING_BOUND",
      `${label} uses an undeclared or unbound slice`
    );
  }
  const packetCards = packet.finding_ids.map(cardId =>
    ledgerState.cards.find(card => card.card_id === cardId)
  );
  if (packet.governance_task === false) {
    assert(
      packetCards.length > 0 && packetCards.every(Boolean),
      `${label} references an unknown finding`
    );
    const prerequisiteException =
      declaredSlice?.evidence_prerequisite_only === true &&
      packet.slice_id === "W0-S3" &&
      packet.finding_ids.length === 1 &&
      packet.finding_ids[0] === "BR4-D064";
    assert(
      prerequisiteException ||
        packetCards.every(
          card =>
            card.current_wave === packet.wave_id &&
            !card.wave_outcome_history.some(outcome =>
              ["CLOSED", "QUARANTINED_UNREACHABLE"].includes(outcome.status)
            )
        ),
      `${label} finding was not active in the Wave at dispatch`
    );
    sameSet(
      packetCards.map(card => card.root_cause_cluster_id),
      [packet.root_cause_cluster_id],
      `${label} root-cause cluster`
    );
    sameSet(
      packet.required_guard_ids,
      packetCards.flatMap(card => card.guard_ids),
      `${label} finding guards`
    );
    const candidateCards = packetCards.filter(
      card => card.current_disposition === "CLOSURE_CANDIDATE"
    );
    if (candidateCards.length > 0) {
      if (packet.mode === "CHALLENGE") {
        assert(
          packet.preimplementation_proof_ids.length === 0,
          `${label} challenge packet invented implementation proof authority`
        );
      } else {
        assert(
          ["VERIFICATION", "VERIFICATION_THEN_CONDITIONAL_IMPLEMENTATION"].includes(packet.mode),
          `${label} bypasses candidate-first verification`
        );
        sameSet(
          packet.preimplementation_proof_ids,
          candidateCards.map(card => card.exact_next_proof.proof_id),
          `${label} candidate proof IDs`
        );
        const requiredCandidateDimensions = new Set(
          candidateCards.flatMap(card => card.exact_next_proof.required_dimensions)
        );
        for (const dimension of requiredCandidateDimensions) {
          assert(
            packet.required_evidence_dimensions.includes(dimension),
            `${label} omits candidate proof dimension ${dimension}`
          );
        }
        if (packet.mode === "VERIFICATION_THEN_CONDITIONAL_IMPLEMENTATION") {
          for (const dimension of [
            "CANDIDATE_PROOF_EXECUTED",
            "CANDIDATE_PROOF_FAILED_BEFORE_IMPLEMENTATION",
          ]) {
            assert(
              packet.required_evidence_dimensions.includes(dimension),
              `${label} omits conditional implementation evidence ${dimension}`
            );
          }
        }
      }
    } else {
      assert(
        packet.preimplementation_proof_ids.length === 0,
        `${label} invented candidate-first proof IDs`
      );
    }
  }
  const stateGuardIds = new Set(ledgerState.guard_catalog.map(guard => guard.id));
  assert(
    packet.required_guard_ids.length > 0 &&
      packet.required_guard_ids.every(guardId => stateGuardIds.has(guardId)) &&
      packet.verification_commands.includes(
        "cargo test -p openlife-tauri single_system -- --nocapture"
      ),
    `${label} lacks valid guards or the canonical single-system command`
  );
  for (const ref of packet.source_map) {
    validateRefAtCommit(executionBaseline, ref, `${label} source map`);
  }
  const ownerPaths = Array.isArray(packet.canonical_owner)
    ? packet.canonical_owner
    : [packet.canonical_owner];
  assert(ownerPaths.length > 0, `${label} canonical owner is empty`);
  for (const ref of ownerPaths) {
    assert(typeof ref === "string", `${label} canonical owner is not a string`);
    validateRefAtCommit(executionBaseline, ref, `${label} canonical owner`);
  }
  if (packet.governance_task === false) {
    const declaredOwnerPaths = new Set(ownerPaths.map(ref => splitRef(ref).path));
    for (const card of packetCards) {
      if (card.canonical_owner.status === "UNKNOWN") {
        assert(
          packet.mode === "VERIFICATION" &&
            packet.required_evidence_dimensions.includes("SOURCE_MAP"),
          `${label} cannot implement before ${card.card_id} owner adjudication`
        );
      } else {
        assert(
          declaredOwnerPaths.has(card.canonical_owner.path) &&
            packet.source_map.some(ref => splitRef(ref).path === card.canonical_owner.path) &&
            (packet.mode !== "IMPLEMENTATION" ||
              pathMatches(card.canonical_owner.path, packet.allowed_touched_paths)),
          `${label} does not bind ${card.card_id} canonical owner`
        );
      }
    }
  }
  for (const [field, patterns] of [
    ["allowed_touched_paths", packet.allowed_touched_paths],
    ["forbidden_touched_paths", packet.forbidden_touched_paths],
  ]) {
    for (const pattern of patterns) {
      assertSafePathPattern(pattern, `${label} ${field}`);
    }
  }
  for (const path of packet.expected_absent_paths) {
    assertSafePathPattern(path, `${label} expected_absent_paths`);
    assert(!path.includes("*"), `${label} has a wildcard expected-absent path`);
  }
  const blueprintAllowed = new Set(
    declaredSlice?.task_packet_blueprint?.allowed_touched_paths ?? []
  );
  for (const pattern of packet.allowed_touched_paths) {
    if (!pattern.includes("*")) continue;
    const staticPrefix = wildcardStaticPrefix(pattern);
    const sourceOwnsPrefix = packet.source_map.some(ref => {
      const sourcePath = splitRef(ref).path;
      return sourcePath === staticPrefix || sourcePath.startsWith(`${staticPrefix}/`);
    });
    assert(
      blueprintAllowed.has(pattern) ||
        (staticPrefix.split("/").filter(Boolean).length >= 3 && sourceOwnsPrefix),
      `${label} wildcard is broader than its source map: ${pattern}`
    );
  }
  if (packet.governance_task === false) {
    for (const statePath of [
      ...ledgerState.integration_contract.state_only_paths,
      ...ledgerState.integration_contract.state_only_path_prefixes.map(
        prefix => `${prefix}probe.json`
      ),
    ]) {
      assert(
        !pathMatches(statePath, packet.allowed_touched_paths),
        `${label} non-governance task can mutate Program state: ${statePath}`
      );
    }
  }
  assert(
    packet.allowed_touched_paths.length > 0 &&
      new Set(packet.allowed_touched_paths).size === packet.allowed_touched_paths.length &&
      packet.allowed_touched_paths.length <= budget.hard_stop_files &&
      packet.execution_baseline_sha === executionBaseline &&
      packet.expected_parent_main_sha === executionBaseline &&
      packet.checkout === programState.slice_contract.writable_checkout &&
      (!requireLiveCheckout || repositoryRoot === programState.slice_contract.writable_checkout) &&
      typeof packet.branch === "string" &&
      packet.branch.startsWith("codex/") &&
      typeof packet.assigned_agent_id === "string" &&
      packet.assigned_agent_id.trim() &&
      packet.assigned_agent_id !== packet.packet_freeze_review.integrator_id &&
      packet.assigned_agent_id !== packet.packet_freeze_review.reviewer_id &&
      isCommitAncestorOfHead(packet.program_activation_sha) &&
      canGit("merge-base", "--is-ancestor", packet.program_activation_sha, executionBaseline) !==
        null &&
      readJsonAtCommit(packet.program_activation_sha, programPath).execution_authorized === true,
    `${label} path, baseline, checkout, branch or activation boundary is invalid`
  );
};
const validateReceiptAdjudications = snapshotSha => {
  const contract = ledger.receipt_adjudication_contract;
  const records = ledger.receipt_adjudication_records;
  assert(contract && Array.isArray(records), "Receipt adjudication authority is missing");
  sameSet(
    contract.allowed_target_record_ids,
    [INVALID_W0_S3_RECORD_ID],
    "Receipt adjudication target allowlist"
  );
  sameSet(contract.allowed_decisions, ["INVALIDATED_NO_CREDIT"], "Receipt adjudication decisions");
  sameSet(
    contract.allowed_reason_codes,
    ["MISSING_REQUIRED_TASK_SCOPE_PATHS"],
    "Receipt adjudication reason codes"
  );
  assert(
    contract.invalidated_credit_scope === "TASK_COMPLETION_AND_INTEGRATED_MAIN_COVERAGE",
    "Receipt adjudication credit boundary drifted"
  );
  const adjudicationIds = new Set();
  const targetIds = new Set();
  for (const adjudication of records) {
    sameSet(
      Object.keys(adjudication),
      contract.required_fields,
      `${adjudication.adjudication_id ?? "Receipt adjudication"} fields`
    );
    assert(
      typeof adjudication.adjudication_id === "string" &&
        adjudication.adjudication_id.trim() &&
        !adjudicationIds.has(adjudication.adjudication_id),
      "Receipt adjudication ID is empty or duplicated"
    );
    adjudicationIds.add(adjudication.adjudication_id);
    assert(
      contract.allowed_target_record_ids.includes(adjudication.target_record_id) &&
        !targetIds.has(adjudication.target_record_id),
      `${adjudication.adjudication_id} targets an unauthorized or duplicate receipt`
    );
    targetIds.add(adjudication.target_record_id);
    const target = ledger.integration_records.find(
      record => record.record_id === adjudication.target_record_id
    );
    assert(target, `${adjudication.adjudication_id} target receipt is missing`);
    assert(
      canonicalDigest(target) === adjudication.target_record_canonical_sha256 &&
        adjudication.target_record_canonical_sha256 === INVALID_W0_S3_RECORD_SHA256,
      `${adjudication.adjudication_id} target canonical digest is invalid`
    );
    assert(
      adjudication.target_record_introduction_sha === "a0004fb8a592d3404d2a92d8ecaf9adf916dd32a" &&
        isCommitAncestorOfHead(adjudication.target_record_introduction_sha),
      `${adjudication.adjudication_id} target introduction SHA is invalid`
    );
    const introductionLedger = readJsonAtCommit(
      adjudication.target_record_introduction_sha,
      ledgerPath
    );
    const introducedTarget = introductionLedger.integration_records.find(
      record => record.record_id === adjudication.target_record_id
    );
    assert(
      introducedTarget &&
        canonicalDigest(introducedTarget) === adjudication.target_record_canonical_sha256,
      `${adjudication.adjudication_id} target was not introduced with the frozen bytes`
    );
    assert(
      adjudication.target_ledger_blob_sha256 ===
        textDigest(readTextAtCommit(PREDECESSOR_HANDOFF_SHA, ledgerPath)) &&
        adjudication.target_ledger_blob_sha256 ===
          "24d5410bf9fb80165780e8742a479a938648c2b569e3a655c610966861329b4d",
      `${adjudication.adjudication_id} predecessor ledger blob binding is invalid`
    );
    assert(
      adjudication.decision === "INVALIDATED_NO_CREDIT" &&
        adjudication.reason_code === "MISSING_REQUIRED_TASK_SCOPE_PATHS" &&
        adjudication.decision_subject_sha === PREDECESSOR_HANDOFF_SHA &&
        canGit("merge-base", "--is-ancestor", adjudication.decision_subject_sha, snapshotSha) !==
          null,
      `${adjudication.adjudication_id} decision or discovery binding is invalid`
    );
    const archivedPacket = readJsonAtCommit(
      adjudication.decision_subject_sha,
      target.packet_artifact_path
    );
    const creditedScope = new Set(
      target.task_evidence_records
        .filter(
          evidence =>
            evidence.outcome === "PASS" &&
            evidence.credit_allowed === true &&
            evidence.active === true &&
            evidence.limitations.length === 0
        )
        .flatMap(evidence => evidence.scope_paths)
    );
    const defectPaths = uniquePaths(
      (Array.isArray(archivedPacket.canonical_owner)
        ? archivedPacket.canonical_owner
        : [archivedPacket.canonical_owner]
      )
        .map(ref => splitRef(ref).path)
        .filter(path => !creditedScope.has(path))
    );
    sameSet(
      adjudication.defect_paths,
      defectPaths,
      `${adjudication.adjudication_id} recomputed defect paths`
    );
    assert(
      defectPaths.length > 0 &&
        adjudication.invalidated_credit_scope === contract.invalidated_credit_scope &&
        typeof adjudication.producer_id === "string" &&
        adjudication.producer_id.trim(),
      `${adjudication.adjudication_id} does not prove a bounded no-credit defect`
    );
    const review = adjudication.independent_review;
    sameSet(
      Object.keys(review),
      contract.independent_review_required_fields,
      `${adjudication.adjudication_id} independent-review fields`
    );
    assert(
      review.outcome === "PASS" &&
        review.reviewed_target_record_sha256 === adjudication.target_record_canonical_sha256 &&
        review.reviewed_decision_subject_sha === adjudication.decision_subject_sha &&
        typeof review.reviewer_id === "string" &&
        review.reviewer_id.trim() &&
        review.reviewer_id !== adjudication.producer_id &&
        typeof review.artifact_or_record === "string" &&
        review.artifact_or_record.trim(),
      `${adjudication.adjudication_id} lacks independent bounded fact review`
    );
  }
  return targetIds;
};

const validateIntegrationRecordReceipt = ({
  record,
  programState,
  ledgerState,
  snapshotSha,
  expectedActivationSha = null,
  requireCanonicalOwnerCoverage = true,
  label,
}) => {
  for (const field of ledgerState.integration_contract.required_fields) {
    assert(field in record, `${label} misses ${field}`);
  }
  sameSet(
    Object.keys(record),
    ledgerState.integration_contract.required_fields,
    `${label} integration-record keys`
  );
  assert(
    typeof record.record_id === "string" &&
      record.record_id.trim() &&
      isSha(record.range_base_sha) &&
      isSha(record.range_head_sha) &&
      canGit("merge-base", "--is-ancestor", record.range_base_sha, record.range_head_sha) !==
        null &&
      canGit("merge-base", "--is-ancestor", record.range_head_sha, snapshotSha) !== null &&
      typeof record.packet_sha256 === "string" &&
      /^[0-9a-f]{64}$/.test(record.packet_sha256),
    `${label} has an invalid identity or committed range`
  );
  const expectedPacketArtifact =
    `${ledgerState.integration_contract.packet_archive_prefix}` + `${record.packet_sha256}.json`;
  assert(
    record.packet_artifact_path === expectedPacketArtifact &&
      pathAtCommitExists(snapshotSha, record.packet_artifact_path),
    `${label} lacks its digest-addressed packet artifact`
  );
  const archivedPacket = readJsonAtCommit(snapshotSha, record.packet_artifact_path);
  assert(
    validateFrozenPacketEnvelope(archivedPacket, `${label} task packet`) === record.packet_sha256,
    `${label} packet digest does not reproduce`
  );
  const dispatchProgram = readJsonAtCommit(record.execution_baseline_sha, programPath);
  const dispatchLedger = readJsonAtCommit(record.execution_baseline_sha, ledgerPath);
  const dispatchActivationSha = deriveAndValidateActivationCommit(
    dispatchProgram.program_approval.approved_draft_commit_sha
  ).activationSha;
  validateFrozenPacketSemantics({
    packet: archivedPacket,
    programState: dispatchProgram,
    ledgerState: dispatchLedger,
    executionBaseline: record.execution_baseline_sha,
    expectedSlice: record.slice_id,
    label: `${label} task packet`,
  });
  assert(
    record.program_approved_draft_sha ===
      dispatchProgram.program_approval.approved_draft_commit_sha &&
      record.program_activation_sha === archivedPacket.program_activation_sha &&
      record.program_activation_sha === dispatchActivationSha &&
      (!expectedActivationSha || record.program_activation_sha === expectedActivationSha) &&
      canGit(
        "merge-base",
        "--is-ancestor",
        record.program_activation_sha,
        record.range_base_sha
      ) !== null &&
      record.execution_baseline_sha === record.range_base_sha &&
      archivedPacket.execution_baseline_sha === record.range_base_sha &&
      archivedPacket.expected_parent_main_sha === record.range_base_sha &&
      archivedPacket.task_id === record.task_id &&
      archivedPacket.slice_id === record.slice_id &&
      archivedPacket.assigned_agent_id === record.producer_id &&
      typeof record.integrator_id === "string" &&
      record.integrator_id.trim() &&
      record.integrator_id === archivedPacket.packet_freeze_review.integrator_id &&
      record.producer_id !== record.integrator_id &&
      archivedPacket.program_schema_version === dispatchProgram.schema_version &&
      archivedPacket.checkout === dispatchProgram.slice_contract.writable_checkout &&
      typeof archivedPacket.branch === "string" &&
      archivedPacket.branch.startsWith("codex/"),
    `${label} packet identity, role, activation or baseline binding is invalid`
  );
  sameSet(
    record.allowed_touched_paths,
    archivedPacket.allowed_touched_paths,
    `${label} frozen allowlist`
  );
  sameSet(
    record.required_guard_ids,
    archivedPacket.required_guard_ids,
    `${label} frozen guard set`
  );
  const rangePaths = changedPathsBetween(record.execution_baseline_sha, record.range_head_sha);
  sameSet(record.changed_paths, rangePaths, `${label} changed paths`);
  assert(record.completion_outcome === "PASS", `${label} is not a completed PASS receipt`);
  for (const path of record.changed_paths) {
    assert(
      pathMatches(path, archivedPacket.allowed_touched_paths) &&
        !pathMatches(path, archivedPacket.forbidden_touched_paths),
      `${label} integrated an out-of-scope path: ${path}`
    );
  }
  for (const path of archivedPacket.expected_absent_paths) {
    assert(
      !pathAtCommitExists(snapshotSha, path),
      `${label} recreated expected-absent path ${path}`
    );
  }
  const integratedChurn = committedRangeChurn(record.execution_baseline_sha, record.range_head_sha);
  assert(
    record.changed_paths.length <= archivedPacket.budget.hard_stop_files &&
      integratedChurn <= archivedPacket.budget.hard_stop_churn_lines,
    `${label} integrated range exceeds the frozen hard-stop budget`
  );
  if (
    ["IMPLEMENTATION", "VERIFICATION_THEN_CONDITIONAL_IMPLEMENTATION"].includes(archivedPacket.mode)
  ) {
    assert(
      record.changed_paths.some(path => !pathIsStateOnly(path, ledgerState.integration_contract)),
      `${label} has no nonempty implementation change`
    );
  }
  assert(
    Array.isArray(record.task_evidence_records) &&
      record.task_evidence_records.every(
        evidence =>
          evidence.subject_sha === record.range_head_sha &&
          Array.isArray(evidence.scope_paths) &&
          evidence.scope_paths.every(path =>
            pathMatches(path, archivedPacket.allowed_touched_paths)
          )
      ),
    `${label} task evidence is not bound to the integrated head and scope`
  );
  validateCreditedEvidenceRecords({
    records: record.task_evidence_records,
    requiredDimensions: archivedPacket.required_evidence_dimensions,
    requiredGuardIds: archivedPacket.required_guard_ids,
    expectedScopeId: archivedPacket.slice_id,
    requireCurrentFreshness: false,
    label: `${label} task completion`,
  });
  const creditedTaskScopePaths = new Set(
    record.task_evidence_records
      .filter(
        evidence =>
          evidence.outcome === "PASS" &&
          evidence.credit_allowed === true &&
          evidence.active === true &&
          evidence.limitations.length === 0
      )
      .flatMap(evidence => evidence.scope_paths)
  );
  const requiredTaskScopePaths = new Set([
    ...record.changed_paths.filter(
      path => !pathIsStateOnly(path, ledgerState.integration_contract)
    ),
    ...(Array.isArray(archivedPacket.canonical_owner)
      ? archivedPacket.canonical_owner
      : [archivedPacket.canonical_owner]
    ).map(ref => splitRef(ref).path),
  ]);
  const invalidatedPredecessorForSlice = (ledgerState.receipt_adjudication_records ?? [])
    .map(adjudication =>
      ledgerState.integration_records.find(
        candidate => candidate.record_id === adjudication.target_record_id
      )
    )
    .find(candidate => candidate?.slice_id === record.slice_id);
  const isCurrentProgramRecoveryReceipt =
    invalidatedPredecessorForSlice &&
    !invalidatedReceiptIds.has(record.record_id) &&
    record.program_approved_draft_sha === programState.program_approval.approved_draft_commit_sha;
  if (isCurrentProgramRecoveryReceipt) {
    assert(
      recoveryReceiptSatisfiesFrozenScope({
        candidate: record,
        invalidatedTarget: invalidatedPredecessorForSlice,
        programState,
        ledgerState,
        snapshotSha,
      }),
      `${label} does not reproduce the frozen predecessor verification scope`
    );
    const invalidatedPacket = readJsonAtCommit(
      snapshotSha,
      invalidatedPredecessorForSlice.packet_artifact_path
    );
    const invalidatedOwnerPaths = (
      Array.isArray(invalidatedPacket.canonical_owner)
        ? invalidatedPacket.canonical_owner
        : [invalidatedPacket.canonical_owner]
    ).map(ref => splitRef(ref).path);
    for (const path of [
      ...invalidatedPredecessorForSlice.changed_paths,
      ...invalidatedOwnerPaths,
    ]) {
      requiredTaskScopePaths.add(path);
    }
    sameSet(
      (Array.isArray(archivedPacket.canonical_owner)
        ? archivedPacket.canonical_owner
        : [archivedPacket.canonical_owner]
      ).map(ref => splitRef(ref).path),
      invalidatedOwnerPaths,
      `${label} recovery canonical-owner set`
    );
    assert(
      archivedPacket.mode === "VERIFICATION" &&
        record.task_id !== invalidatedPredecessorForSlice.task_id &&
        record.packet_sha256 !== invalidatedPredecessorForSlice.packet_sha256,
      `${label} is not a distinct verification recovery receipt`
    );
  }
  if (requireCanonicalOwnerCoverage) {
    for (const path of requiredTaskScopePaths) {
      assert(
        creditedTaskScopePaths.has(path),
        `${label} credited task evidence does not cover ${path}`
      );
    }
  }
  const review = record.independent_review;
  for (const field of ledgerState.integration_contract.independent_review_required_fields) {
    assert(field in review, `${label} review misses ${field}`);
  }
  assert(
    review.outcome === "PASS" &&
      review.reviewed_head_sha === record.range_head_sha &&
      typeof review.reviewer_id === "string" &&
      review.reviewer_id.trim() &&
      review.reviewer_id !== record.integrator_id &&
      review.reviewer_id !== record.producer_id &&
      typeof review.artifact_or_record === "string" &&
      review.artifact_or_record.trim(),
    `${label} lacks same-head independent integration review`
  );
  return archivedPacket;
};
invalidatedReceiptIds = validateReceiptAdjudications(validationHeadSha);

const validateIntegratedMainHistory = () => {
  const approvedDraft = program.program_approval.approved_draft_commit_sha;
  const currentApprovalRecords = [];
  const recordIds = new Set();
  const packetDigests = new Set();
  const packetArtifacts = new Set();
  for (const record of ledger.integration_records) {
    for (const field of ledger.integration_contract.required_fields) {
      assert(field in record, `Integration record misses ${field}`);
    }
    assert(
      typeof record.record_id === "string" &&
        record.record_id.trim() &&
        !recordIds.has(record.record_id),
      `Integration record ID is empty or duplicated: ${record.record_id}`
    );
    recordIds.add(record.record_id);
    assert(
      typeof record.packet_sha256 === "string" &&
        /^[0-9a-f]{64}$/.test(record.packet_sha256) &&
        !packetDigests.has(record.packet_sha256),
      `${record.record_id} has an invalid packet digest`
    );
    packetDigests.add(record.packet_sha256);
    assert(
      !packetArtifacts.has(record.packet_artifact_path),
      `${record.record_id} reuses a packet artifact`
    );
    packetArtifacts.add(record.packet_artifact_path);
    const invalidated = invalidatedReceiptIds.has(record.record_id);
    validateIntegrationRecordReceipt({
      record,
      programState: program,
      ledgerState: ledger,
      snapshotSha: "HEAD",
      requireCanonicalOwnerCoverage: !invalidated,
      label: record.record_id,
    });
    if (!invalidated) currentApprovalRecords.push(record);
  }
  const attemptPacketArtifacts = new Set();
  for (const attempt of ledger.implementation_attempt_records) {
    assert(
      pathAtCommitExists("HEAD", attempt.packet_artifact_path) &&
        pathAtCommitExists("HEAD", attempt.attempt_artifact_path) &&
        textDigest(readText(attempt.attempt_artifact_path)) === attempt.attempt_artifact_sha256 &&
        !attemptPacketArtifacts.has(attempt.packet_artifact_path),
      `${attempt.attempt_id} lacks a unique tracked packet artifact`
    );
    attemptPacketArtifacts.add(attempt.packet_artifact_path);
    const archivedPacket = readJsonAtCommit(
      attempt.artifact_commit_sha,
      attempt.packet_artifact_path
    );
    assert(
      readText(attempt.packet_artifact_path) ===
        readTextAtCommit(attempt.artifact_commit_sha, attempt.packet_artifact_path),
      `${attempt.attempt_id} packet artifact changed after settlement`
    );
    assert(
      validateFrozenPacketEnvelope(archivedPacket, `${attempt.attempt_id} task packet`) ===
        attempt.packet_sha256,
      `${attempt.attempt_id} packet digest does not reproduce`
    );
    const dispatchProgram = readJsonAtCommit(attempt.execution_baseline_sha, programPath);
    const dispatchLedger = readJsonAtCommit(attempt.execution_baseline_sha, ledgerPath);
    validateFrozenPacketSemantics({
      packet: archivedPacket,
      programState: dispatchProgram,
      ledgerState: dispatchLedger,
      executionBaseline: attempt.execution_baseline_sha,
      expectedSlice: attempt.slice_id,
      label: `${attempt.attempt_id} task packet`,
    });
    assert(
      archivedPacket.task_id === attempt.task_id &&
        archivedPacket.slice_id === attempt.slice_id &&
        archivedPacket.root_cause_cluster_id === attempt.root_cause_cluster_id &&
        archivedPacket.execution_baseline_sha === attempt.execution_baseline_sha &&
        archivedPacket.assigned_agent_id === attempt.producer_id &&
        archivedPacket.packet_freeze_review.integrator_id === attempt.integrator_id,
      `${attempt.attempt_id} is not bound to its frozen task packet`
    );
    if (attempt.outcome === "FAILED_ROOT_CAUSE_ATTEMPT") {
      assert(
        ["IMPLEMENTATION", "VERIFICATION_THEN_CONDITIONAL_IMPLEMENTATION"].includes(
          archivedPacket.mode
        ),
        `${attempt.attempt_id} counted a non-implementation failure`
      );
    }
    const matchingIntegration = ledger.integration_records.find(
      record => record.packet_sha256 === attempt.packet_sha256
    );
    assert(
      attempt.outcome === "SUCCEEDED"
        ? matchingIntegration?.completion_outcome === "PASS"
        : !matchingIntegration,
      `${attempt.attempt_id} outcome disagrees with integration history`
    );
  }
  for (const record of ledger.integration_records) {
    const attempt = ledger.implementation_attempt_records.find(
      candidate => candidate.packet_sha256 === record.packet_sha256
    );
    assert(
      attempt &&
        attempt.outcome === "SUCCEEDED" &&
        attempt.task_id === record.task_id &&
        attempt.slice_id === record.slice_id &&
        attempt.producer_id === record.producer_id &&
        attempt.integrator_id === record.integrator_id,
      `${record.record_id} lacks its immutable SUCCEEDED attempt record`
    );
  }
  const trackedPacketArtifacts = gitNul(
    "ls-files",
    "-z",
    "--",
    ledger.integration_contract.packet_archive_prefix
  );
  sameSet(
    trackedPacketArtifacts,
    [...packetArtifacts, ...attemptPacketArtifacts],
    "Tracked task-packet archive"
  );
  sameSet(
    gitNul("ls-files", "-z", "--", ledger.attempt_contract.attempt_artifact_prefix),
    ledger.implementation_attempt_records.map(record => record.attempt_artifact_path),
    "Tracked attempt-artifact archive"
  );
  sameSet(
    gitNul("ls-files", "-z", "--", ledger.attempt_contract.architecture_review_artifact_prefix),
    ledger.architecture_review_records.map(record => record.review_artifact_path),
    "Tracked architecture-review artifact archive"
  );
  for (const review of ledger.architecture_review_records) {
    assert(
      textDigest(readText(review.review_artifact_path)) === review.review_artifact_sha256 &&
        readText(review.review_artifact_path) ===
          readTextAtCommit(review.review_artifact_commit_sha, review.review_artifact_path),
      `${review.review_id} architecture-review artifact changed after settlement`
    );
  }
  const touchedSinceApproval = git(
    "log",
    "--format=",
    "--name-only",
    `${approvedDraft}..HEAD`,
    "--"
  )
    .split("\n")
    .filter(Boolean);
  const touchedProductPaths = touchedSinceApproval.filter(
    path => !pathIsStateOnly(path, ledger.integration_contract)
  );
  const currentProgramRecords = currentApprovalRecords.filter(
    record => record.program_approved_draft_sha === approvedDraft
  );
  const coveredProductPaths = currentProgramRecords.flatMap(record =>
    record.changed_paths.filter(path => !pathIsStateOnly(path, ledger.integration_contract))
  );
  sameSet(coveredProductPaths, touchedProductPaths, "Integrated main task-receipt coverage");
  const commitsSinceApproval = git("rev-list", "--reverse", `${approvedDraft}..HEAD`)
    .split("\n")
    .filter(Boolean);
  for (const commitSha of commitsSinceApproval) {
    const commitProductPaths = commitNovelPaths(commitSha).filter(
      path => !pathIsStateOnly(path, ledger.integration_contract)
    );
    if (commitProductPaths.length === 0) continue;
    const coveringRecords = currentProgramRecords.filter(
      record =>
        record.range_base_sha !== commitSha &&
        canGit("merge-base", "--is-ancestor", record.range_base_sha, commitSha) !== null &&
        canGit("merge-base", "--is-ancestor", commitSha, record.range_head_sha) !== null
    );
    assert(
      coveringRecords.some(record =>
        commitProductPaths.every(path => record.changed_paths.includes(path))
      ),
      `Integrated product commit lacks a verifiable task receipt: ${commitSha}`
    );
  }
};

if (profile === "draft") {
  assert(dirtyPaths.length === 0, "Successor draft must be a clean tracked commit");
  const draftParents = git("rev-list", "--parents", "-n", "1", validationHeadSha).split(" ");
  assert(
    draftParents.length === 2 &&
      draftParents[0] === validationHeadSha &&
      draftParents[1] === PREDECESSOR_HANDOFF_SHA,
    "Successor draft must be a single-parent direct child of the predecessor handoff"
  );
  sameSet(
    changedPathsBetween(PREDECESSOR_HANDOFF_SHA, validationHeadSha),
    SUCCESSOR_DRAFT_PATHS,
    "Successor draft governance-only changed paths"
  );
  assert(
    program.status === "DRAFT_AWAITING_USER_APPROVAL" && program.execution_authorized === false,
    "Draft profile requires fail-closed Program status"
  );
  assert(
    program.program_approval.status === "PENDING_USER_APPROVAL" &&
      program.program_approval.execution_authority_granted === false &&
      program.program_approval.approved_draft_commit_sha === null &&
      program.program_approval.approved_by === null &&
      program.program_approval.approved_at === null &&
      program.program_approval.approval_record === null,
    "Draft profile requires pending user approval"
  );
  assert(
    ledger.status === "DRAFT_AWAITING_USER_APPROVAL" &&
      ledger.authority.execution_authorized === false,
    "Draft ledger is not fail closed"
  );
  assert(
    program.program_activation.status === "DRAFT_VALIDATED_AWAITING_CHALLENGE_AND_USER_APPROVAL",
    "Draft activation state is invalid"
  );
  assert(
    program.waves.every(wave => wave.status === "PLANNED_NOT_AUTHORIZED"),
    "A Wave started in draft profile"
  );
  assert(
    program.gates.every(gate => gate.status === "NOT_RUN" && gate.evidence_records.length === 0),
    "A gate has premature credit in draft profile"
  );
  assert(
    featureGates.bounded_feature_eligibility.status === "BLOCKED" &&
      featureGates.bounded_feature_eligibility.eligible === false &&
      featureGates.bounded_feature_eligibility.eligible_domains.length === 0 &&
      featureGates.bounded_feature_eligibility.evidence_records.length === 0 &&
      featureGates.normal_feature_development_reopen.status === "BLOCKED" &&
      featureGates.normal_feature_development_reopen.reopened === false &&
      featureGates.normal_feature_development_reopen.evidence_records.length === 0,
    "Feature work reopened in draft profile"
  );
  assert(cards.length === 101, "Draft profile must contain exactly 101 cards");
  assert(
    ledger.integration_records.length === 3 &&
      ledger.implementation_attempt_records.length === 3 &&
      canonicalDigest(ledger.integration_records) ===
        canonicalDigest(predecessorLedger.integration_records) &&
      canonicalDigest(ledger.implementation_attempt_records) ===
        canonicalDigest(predecessorLedger.implementation_attempt_records),
    "Successor draft did not preserve the exact predecessor receipt history"
  );
  for (const record of ledger.integration_records) {
    validateIntegrationRecordReceipt({
      record,
      programState: program,
      ledgerState: ledger,
      snapshotSha: validationHeadSha,
      requireCanonicalOwnerCoverage: !invalidatedReceiptIds.has(record.record_id),
      label: `successor predecessor ${record.record_id}`,
    });
  }
  assert(
    invalidatedReceiptIds.size === 1 && invalidatedReceiptIds.has(INVALID_W0_S3_RECORD_ID),
    "Successor draft did not explicitly invalidate the defective W0-S3 receipt"
  );
  for (const card of baselineCards) {
    const fact = baselineFactById.get(card.card_id);
    const { fact_sha256: _factDigest, ...factWithoutDigest } = fact;
    assert(
      canonicalDigest(initialFactProjection(card)) === canonicalDigest(factWithoutDigest) &&
        card.current_wave === fact.initial_assigned_wave &&
        card.closure_credit === false &&
        card.wave_outcome_history.length === 0,
      `${card.card_id} changed before activation`
    );
  }
  for (const path of dirtyPaths) {
    assert(pathMatches(path, planningPaths), `Out-of-scope draft change: ${path}`);
  }
  for (const path of git("diff", "--name-only", PREDECESSOR_HANDOFF_SHA, "--")
    .split("\n")
    .filter(Boolean)) {
    assert(pathMatches(path, planningPaths), `Product changed in draft: ${path}`);
  }
  assert(
    programMarkdown.includes("Initial publication state: `DRAFT_AWAITING_USER_APPROVAL`") &&
      programMarkdown.includes("Live status and execution authority: read"),
    "Program Markdown draft status disagrees with JSON"
  );
} else if (profile === "activation") {
  assert(dirtyPaths.length === 0, "Activation must be a clean tracked commit");
  const approvedDraft = program.program_approval.approved_draft_commit_sha;
  assert(
    isCommitAncestorOfHead(approvedDraft) && git("rev-parse", "HEAD^") === approvedDraft,
    "Activation must be the direct clean child of the approved draft"
  );
  const approvedProgram = readJsonAtCommit(approvedDraft, programPath);
  const approvedLedger = readJsonAtCommit(approvedDraft, ledgerPath);
  assert(
    approvedProgram.status === "DRAFT_AWAITING_USER_APPROVAL" &&
      approvedProgram.execution_authorized === false &&
      approvedLedger.status === "DRAFT_AWAITING_USER_APPROVAL" &&
      approvedLedger.authority.execution_authorized === false,
    "Approved draft was not fail closed"
  );
  assert(
    canonicalDigest(activationSubstantiveProgram(program)) ===
      canonicalDigest(activationSubstantiveProgram(approvedProgram)) &&
      canonicalDigest(activationSubstantiveLedger(ledger)) ===
        canonicalDigest(activationSubstantiveLedger(approvedLedger)),
    "Activation changed substantive Program or ledger content"
  );
  sameSet(
    changedPathsFrom(approvedDraft),
    [programPath, ledgerPath],
    "Activation-only changed paths"
  );
  assertAuthorizedProgramState();
  assert(
    program.status === "APPROVED_FOR_EXECUTION",
    "Activation profile has the wrong Program status"
  );
  assert(
    program.program_approval.github_formal_approval_credit === false,
    "Program activation invented GitHub approval credit"
  );
  assert(
    w0.status === "READY" &&
      program.waves.slice(1).every(wave => wave.status === "PLANNED_NOT_AUTHORIZED"),
    "Activation profile must make only WAVE-0 ready"
  );
  assert(
    program.gates
      .filter(gate => gate.id !== "G-PROGRAM-ACTIVATION")
      .every(gate => gate.status === "NOT_RUN" && gate.evidence_records.length === 0),
    "Activation granted a non-activation gate"
  );
  assert(
    boundedFeatureGate.status === "BLOCKED" &&
      boundedFeatureGate.eligible === false &&
      normalFeatureGate.status === "BLOCKED" &&
      normalFeatureGate.reopened === false,
    "Activation granted feature credit"
  );
  assert(cards.length === 101, "Activation may not add/delete baseline cards");
  assert(
    ledger.integration_records.length === 3 &&
      ledger.implementation_attempt_records.length === 3 &&
      canonicalDigest(ledger.integration_records) ===
        canonicalDigest(predecessorLedger.integration_records) &&
      canonicalDigest(ledger.implementation_attempt_records) ===
        canonicalDigest(predecessorLedger.implementation_attempt_records) &&
      invalidatedReceiptIds.size === 1 &&
      invalidatedReceiptIds.has(INVALID_W0_S3_RECORD_ID),
    "Successor activation changed predecessor receipt or invalidation history"
  );
  for (const record of ledger.integration_records) {
    validateIntegrationRecordReceipt({
      record,
      programState: program,
      ledgerState: ledger,
      snapshotSha: validationHeadSha,
      requireCanonicalOwnerCoverage: !invalidatedReceiptIds.has(record.record_id),
      label: `activated predecessor ${record.record_id}`,
    });
  }
  for (const card of baselineCards) {
    const fact = baselineFactById.get(card.card_id);
    const { fact_sha256: _factDigest, ...factWithoutDigest } = fact;
    assert(
      canonicalDigest(initialFactProjection(card)) === canonicalDigest(factWithoutDigest) &&
        card.current_wave === fact.initial_assigned_wave &&
        card.closure_credit === false &&
        card.wave_outcome_history.length === 0,
      `${card.card_id} changed during activation`
    );
  }
} else {
  const currentActivationSha = assertAuthorizedProgramState();
  const inProgress = program.waves.filter(wave => wave.status === "IN_PROGRESS");
  assert(inProgress.length <= 1, "More than one Wave is IN_PROGRESS");
  for (const wave of program.waves) {
    assert(
      program.vocabularies.wave_status.includes(wave.status),
      `${wave.id} has invalid living status`
    );
    const waveHasStarted = ["READY", "IN_PROGRESS", "COMPLETE"].includes(wave.status);
    if (waveHasStarted) {
      assert(
        wave.depends_on_wave_ids.every(
          dependencyId =>
            program.waves.find(candidate => candidate.id === dependencyId).status === "COMPLETE"
        ),
        `${wave.id} started before its dependency completed`
      );
      assert(
        wave.entry_gate_ids.every(gateId => gateById.get(gateId).status === "PASS"),
        `${wave.id} started before its entry gate passed`
      );
    }
    if (wave.status === "COMPLETE") {
      assert(
        wave.exit_gate_ids.every(gateId => gateById.get(gateId).status === "PASS"),
        `${wave.id} completed before its exit gate passed`
      );
      const enteringCards = cards.filter(
        card =>
          card.initial_assigned_wave === wave.id ||
          card.wave_outcome_history.some(
            outcome =>
              ["ADJUDICATED_AND_REASSIGNED", "EXPLICIT_CARRY_FORWARD_WITH_REASON"].includes(
                outcome.status
              ) && outcome.target_wave === wave.id
          )
      );
      for (const card of enteringCards) {
        assert(
          card.wave_outcome_history.some(outcome => outcome.wave_id === wave.id),
          `${wave.id} completed without settling ${card.card_id}`
        );
      }
    }
  }
  if (boundedFeatureGate.eligible) {
    const boundedPrerequisiteGateIds = [
      "G-W0-COVERAGE-TRUTH",
      "G-W0-LEASE-DETERMINISM",
      "G-W0-EXTERNAL-STATE-ISOLATION",
      "G-W0-LEDGER-RECONCILIATION",
      "G-W1-P0-ADJUDICATION",
    ];
    const priorityUnknownIds = [...wave1Ids].filter(cardId => cardId !== "BR4-D064");
    assert(
      boundedPrerequisiteGateIds.every(gateId => gateById.get(gateId).status === "PASS") &&
        program.waves[0].status === "COMPLETE" &&
        program.waves[1].status === "COMPLETE",
      "Bounded feature eligibility lacks prerequisite gates"
    );
    assert(
      d064.closure_credit === true ||
        d064.wave_outcome_history.some(outcome => outcome.status === "QUARANTINED_UNREACHABLE"),
      "Bounded feature eligibility leaves BR4-D064 reachable and open"
    );
    for (const cardId of priorityUnknownIds) {
      assert(
        cards
          .find(card => card.card_id === cardId)
          .wave_outcome_history.some(
            outcome =>
              outcome.wave_id === "WAVE-1" &&
              ["CLOSED", "ADJUDICATED_AND_REASSIGNED", "QUARANTINED_UNREACHABLE"].includes(
                outcome.status
              )
          ),
        `Bounded feature eligibility skipped ${cardId}`
      );
    }
    assert(
      cards
        .filter(card => card.current_severity === "P0")
        .every(
          card =>
            card.closure_credit === true ||
            card.wave_outcome_history.some(outcome => outcome.status === "QUARANTINED_UNREACHABLE")
        ),
      "Bounded feature eligibility leaves a reachable P0"
    );
    for (const domain of boundedFeatureGate.eligible_domains) {
      for (const field of boundedFeatureGate.eligible_domain_contract.required_fields) {
        assert(field in domain, `Eligible domain misses ${field}`);
      }
      assert(
        isCommitAncestorOfHead(domain.subject_sha) &&
          typeof domain.domain_id === "string" &&
          domain.domain_id.trim() &&
          typeof domain.boundary_review_artifact === "string" &&
          domain.boundary_review_artifact.trim() &&
          Array.isArray(domain.crossing_open_card_ids) &&
          domain.crossing_open_card_ids.length === 0 &&
          Array.isArray(domain.evidence_record_ids) &&
          domain.evidence_record_ids.length > 0 &&
          Array.isArray(domain.scope_paths) &&
          domain.scope_paths.length > 0 &&
          Array.isArray(domain.guard_ids) &&
          domain.guard_ids.length > 0 &&
          domain.guard_ids.every(guardId => guardIds.has(guardId)),
        "Eligible domain lacks a current, closed boundary review"
      );
      for (const pattern of domain.scope_paths) {
        assertSafePathPattern(pattern, `Eligible domain ${domain.domain_id}`);
      }
      const domainEvidence = domain.evidence_record_ids.map(recordId =>
        boundedFeatureGate.evidence_records.find(record => record.record_id === recordId)
      );
      assert(
        domainEvidence.every(
          record =>
            record &&
            record.scope_id === domain.domain_id &&
            record.subject_sha === domain.subject_sha &&
            record.outcome === "PASS" &&
            record.credit_allowed === true &&
            record.active === true &&
            record.limitations.length === 0 &&
            evidenceRecordIsFresh(record)
        ) &&
          domainEvidence.some(
            record => record.artifact_or_record === domain.boundary_review_artifact
          ),
        `Eligible domain ${domain.domain_id} is not bound to current credited evidence`
      );
      sameSet(
        domain.scope_paths,
        domainEvidence.flatMap(record => record.scope_paths),
        `Eligible domain ${domain.domain_id} scope`
      );
      sameSet(
        domain.guard_ids,
        domainEvidence.flatMap(record => record.guard_ids),
        `Eligible domain ${domain.domain_id} guards`
      );
      const domainDimensions = new Set(domainEvidence.flatMap(record => record.dimensions));
      for (const dimension of ["FEATURE_BOUNDARY", "OPEN_CARD_CROSSING", "ABSENCE"]) {
        assert(
          domainDimensions.has(dimension),
          `Eligible domain ${domain.domain_id} lacks ${dimension}`
        );
      }
    }
  }
  const currentProgramW5 = program.waves.find(wave => wave.id === "WAVE-5");
  const currentProgramW5Gate = gateById.get("G-W5-PHASE7-TRIAL");
  assert(
    currentProgramW5.successor_program_required === true &&
      currentProgramW5.current_program_may_complete_wave === false &&
      currentProgramW5.feature_credit === "NONE" &&
      currentProgramW5.status === "PLANNED_NOT_AUTHORIZED" &&
      currentProgramW5Gate.status === "NOT_RUN" &&
      currentProgramW5Gate.evidence_records.length === 0 &&
      normalFeatureGate.status === "BLOCKED" &&
      normalFeatureGate.reopened === false &&
      normalFeatureGate.evidence_records.length === 0,
    "Program schema 1.0.3 cannot execute W5 or reopen normal feature work"
  );
  const scopedOngoingValidation =
    Boolean(args.slice) || Boolean(args["task-packet"]) || Boolean(args["execution-baseline"]);
  if (!scopedOngoingValidation) {
    assert(
      dirtyPaths.length === 0 &&
        (currentBranch === "main" ||
          (currentBranch === "" && git("rev-parse", "HEAD") === canonicalMainTip)),
      "Bare ongoing validation is state-only and requires a clean main checkout"
    );
    validateIntegratedMainHistory();
  } else {
    assert(
      args.slice && args["task-packet"] && args["execution-baseline"],
      "Scoped ongoing validation requires --slice, --task-packet and --execution-baseline together"
    );
    const executionBaseline = args["execution-baseline"];
    assert(
      isCommitAncestorOfHead(executionBaseline),
      "Scoped ongoing validation requires --execution-baseline=<40-hex ancestor>"
    );
    const packet = readJsonInput(args["task-packet"]);
    for (const requiredField of program.agent_task_contract.required_fields) {
      assert(requiredField in packet, `Task packet misses ${requiredField}`);
    }
    sameSet(
      Object.keys(packet),
      program.agent_task_contract.required_fields,
      "Task packet top-level keys"
    );
    const dispatchProgram = readJsonAtCommit(executionBaseline, programPath);
    const dispatchLedger = readJsonAtCommit(executionBaseline, ledgerPath);
    validateFrozenPacketEnvelope(packet, "Task packet");
    validateFrozenPacketSemantics({
      packet,
      programState: dispatchProgram,
      ledgerState: dispatchLedger,
      executionBaseline,
      expectedSlice: args.slice,
      requireLiveCheckout: true,
      label: "Task packet",
    });
    const budget = packet.budget;
    assert(
      packet.execution_baseline_sha === executionBaseline &&
        packet.expected_parent_main_sha === executionBaseline &&
        canonicalMainTip === executionBaseline &&
        currentBranch === packet.branch,
      "Task packet is not bound to the current main tip and task branch"
    );
    assert(
      packet.program_activation_sha === currentActivationSha,
      "Task packet activation does not match the derived Program activation"
    );
    const scopedPaths = uniquePaths(changedPathsFrom(executionBaseline), dirtyPaths);
    enforcePathScope({
      paths: scopedPaths,
      allowedPaths: packet.allowed_touched_paths,
      forbiddenPaths: packet.forbidden_touched_paths,
      expectedAbsentPaths: packet.expected_absent_paths,
      label: `Task packet ${packet.task_id}`,
    });
    const taskUntrackedPaths = untrackedPaths.filter(path => scopedPaths.includes(path));
    const scopedChurnLines = scopedDiffChurn(executionBaseline, taskUntrackedPaths);
    assert(
      scopedPaths.length <= budget.hard_stop_files &&
        scopedChurnLines <= budget.hard_stop_churn_lines,
      `Task packet ${packet.task_id} exceeds hard-stop budget: ` +
        `${scopedPaths.length} files, ${scopedChurnLines} churn lines`
    );
    if (
      scopedPaths.length > budget.warning_files ||
      scopedChurnLines > budget.warning_churn_lines
    ) {
      process.stderr.write(
        `WARNING: task packet ${packet.task_id} exceeds warning budget: ` +
          `${scopedPaths.length} files, ${scopedChurnLines} churn lines\n`
      );
    }
  }
}

const recomputedCurrentInventory = {
  cards: cards.length,
  severity: countBy(cards, "current_severity"),
  disposition: countBy(cards, "current_disposition"),
  current_wave_counts: countBy(cards, "current_wave"),
  closure_credit_true: cards.filter(card => card.closure_credit).length,
};
for (const [key, value] of Object.entries(recomputedCurrentInventory)) {
  if (typeof value === "object") {
    sameJson(ledger.current_inventory[key], value, `Current inventory ${key}`);
  } else {
    assert(ledger.current_inventory[key] === value, `Current inventory ${key} is stale`);
  }
}
const hasReconciledLivingEvidence =
  cards.length > 101 ||
  cards.some(
    card =>
      card.wave_outcome_history.length > 0 ||
      card.closure_credit ||
      card.current_severity !== baselineFactById.get(card.card_id)?.initial_current_severity ||
      card.current_disposition !== baselineFactById.get(card.card_id)?.initial_disposition ||
      canonicalDigest(card.canonical_owner) !==
        canonicalDigest(baselineFactById.get(card.card_id)?.initial_canonical_owner) ||
      canonicalDigest(card.source_evidence) !==
        canonicalDigest(baselineFactById.get(card.card_id)?.initial_source_evidence) ||
      canonicalDigest(card.behavior_evidence) !==
        canonicalDigest(baselineFactById.get(card.card_id)?.initial_behavior_evidence)
  ) ||
  program.gates
    .filter(gate => gate.id !== "G-PROGRAM-ACTIVATION")
    .some(gate => gate.status !== "NOT_RUN" || gate.evidence_records.length > 0) ||
  ledger.integration_records.length > 0 ||
  ledger.implementation_attempt_records.length > 0 ||
  ledger.architecture_review_records.length > 0 ||
  ledger.receipt_adjudication_records.length > 0 ||
  ledger.new_card_creation_records.length > 0 ||
  boundedFeatureGate.evidence_records.length > 0 ||
  boundedFeatureGate.eligible_domains.length > 0 ||
  normalFeatureGate.evidence_records.length > 0;
if (hasReconciledLivingEvidence) {
  const reconciliationSha = ledger.current_inventory.last_reconciled_execution_sha;
  assert(
    isCommitAncestorOfHead(reconciliationSha),
    "Living evidence exists without a resolvable reconciliation SHA"
  );
  const livingSubjectShas = [
    ...cards.flatMap(card => [
      ...card.wave_outcome_history.map(outcome => outcome.execution_sha),
      ...(card.closure_record
        ? [card.closure_record.implementation_sha, card.closure_record.evidence_head_sha]
        : []),
      ...card.behavior_evidence.records
        .map(record => record.source_sha)
        .filter(sha => sha && sha !== baseline.sha),
    ]),
    ...program.gates
      .filter(gate => gate.id !== "G-PROGRAM-ACTIVATION")
      .flatMap(gate => gate.evidence_records.map(record => record.subject_sha)),
    ...ledger.receipt_adjudication_records.map(record => record.decision_subject_sha),
    ...boundedFeatureGate.evidence_records.map(record => record.subject_sha),
    ...normalFeatureGate.evidence_records.map(record => record.subject_sha),
    ...boundedFeatureGate.eligible_domains.map(domain => domain.subject_sha),
    ...ledger.integration_records.map(record => record.range_head_sha),
    ...ledger.implementation_attempt_records.map(record => record.artifact_commit_sha),
    ...ledger.architecture_review_records.map(record => record.review_artifact_commit_sha),
    ...ledger.new_card_creation_records.map(record => record.creation_sha),
  ].filter(Boolean);
  assert(
    livingSubjectShas.length > 0 &&
      livingSubjectShas.every(
        subjectSha =>
          isCommitAncestorOfHead(subjectSha) &&
          canGit("merge-base", "--is-ancestor", subjectSha, reconciliationSha) !== null
      ),
    "Reconciliation SHA does not cover every living evidence subject"
  );
  for (const card of baselineCards) {
    const fact = baselineFactById.get(card.card_id);
    const cardDrifted =
      card.current_severity !== fact.initial_current_severity ||
      card.current_disposition !== fact.initial_disposition ||
      canonicalDigest(card.canonical_owner) !== canonicalDigest(fact.initial_canonical_owner) ||
      canonicalDigest(card.source_evidence) !== canonicalDigest(fact.initial_source_evidence) ||
      canonicalDigest(card.behavior_evidence) !== canonicalDigest(fact.initial_behavior_evidence);
    if (cardDrifted) {
      assert(
        card.wave_outcome_history.length > 0,
        `${card.card_id} living facts changed without a Wave outcome receipt`
      );
    }
  }
} else {
  assert(
    ledger.current_inventory.last_reconciled_execution_sha === null,
    "Ledger claims reconciliation before any living evidence changed"
  );
}

const finalTrackedDirtyPaths = uniquePaths(
  gitNul("diff", "--name-only", "-z", "--no-renames", "--"),
  gitNul("diff", "--cached", "--name-only", "-z", "--no-renames", "--")
);
const finalUntrackedPaths = gitNul("ls-files", "--others", "--exclude-standard", "-z");
const finalDirtyPaths = uniquePaths(finalTrackedDirtyPaths, finalUntrackedPaths);
assert(
  git("rev-parse", "HEAD") === validationHeadSha &&
    git("branch", "--show-current") === validationBranch &&
    canonicalDigest(finalDirtyPaths) === canonicalDigest(dirtyPaths),
  "Repository HEAD, branch or dirty state changed during Program validation"
);

process.stdout.write(
  [
    "Current Development Program validation: PASS",
    `profile=${profile}`,
    `baseline=${baseline.sha}`,
    `branch=${currentBranch || "DETACHED"}`,
    `cards=${cards.length}`,
    `baseline_card_hash=${BASELINE_CARD_HASH}`,
    `baseline_fact_hash=${BASELINE_FACT_HASH}`,
    `execution_authorized=${program.execution_authorized}`,
  ].join("\n") + "\n"
);
