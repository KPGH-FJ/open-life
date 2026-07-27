#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { arch, platform } from "node:process";

const focusedTestId =
  "tasks::tests::scheduled_task_store_cross_process_writer_lease_rejects_then_reopens_after_drop";
const focusedRepetitions = 20;
const workspaceRepetitions = 2;
const focusedMarkers = [
  "task_store_owner_lease_probe:same_process:failure_layer=process_registry:source_kind=none",
  "task_store_cross_process_probe:lease_unavailable:failure_layer=os_owner_lock_would_block:source_kind=would_block",
  "task_store_cross_process_probe:opened_same_canonical_store",
];

const fail = (message, result) => {
  if (result) {
    const combined = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
    const tail = combined.split(/\r?\n/u).slice(-80).join("\n");
    process.stderr.write(`${message}\nraw_failure_tail:\n${tail}\n`);
  } else {
    process.stderr.write(`${message}\n`);
  }
  process.exit(1);
};

if (platform !== "darwin" || arch !== "arm64") {
  fail(
    `W0-LEASE-FORKEXEC requires macOS arm64; observed platform=${platform} arch=${arch}`
  );
}

const runCargo = (args, timeoutMs) =>
  spawnSync("cargo", args, {
    cwd: process.cwd(),
    encoding: "utf8",
    env: {
      ...process.env,
      CARGO_TERM_COLOR: "never",
    },
    maxBuffer: 64 * 1024 * 1024,
    timeout: timeoutMs,
  });

const digestJson = value =>
  createHash("sha256").update(JSON.stringify(value), "utf8").digest("hex");

const countOccurrences = (output, marker) => output.split(marker).length - 1;

const normalizedTestResultSummaries = output =>
  output
    .split(/\r?\n/u)
    .filter(line => line.startsWith("test result:"))
    .map(line => line.replace(/finished in [0-9.]+s$/u, "finished in <elapsed>"));

const focusedSemanticEvidence = output => ({
  marker_counts: Object.fromEntries(
    focusedMarkers.map(marker => [marker, countOccurrences(output, marker)])
  ),
  exact_test_started_line_count: countOccurrences(
    output,
    `test ${focusedTestId} ...`
  ),
  test_result_summaries: normalizedTestResultSummaries(output),
  failed_test_line_count: output
    .split(/\r?\n/u)
    .filter(line => /^test .* \.\.\. FAILED$/u.test(line)).length,
  panic_count: countOccurrences(output, "panicked at"),
});

const workspaceSemanticEvidence = output => ({
  test_result_summaries: normalizedTestResultSummaries(output),
  failed_test_line_count: output
    .split(/\r?\n/u)
    .filter(line => /^test .* \.\.\. FAILED$/u.test(line)).length,
  panic_count: countOccurrences(output, "panicked at"),
});

process.stdout.write(
  `${JSON.stringify({
    event: "W0-S2-CONTRACT",
    contract_id: "W0-LEASE-FORKEXEC",
    platform: "macOS",
    arch: "arm64",
    clean_parent: 1,
    fork_to_exec: 1,
    seed: "N/A",
    process_model: "one clean parent plus one exact exec'd child per focused case",
    concurrency: 1,
    focused_test_id: focusedTestId,
    focused_repetitions: focusedRepetitions,
    workspace_repetitions: workspaceRepetitions,
    semantic_comparison:
      "fixed marker counts, exact test result summaries, failed-test count, and panic count",
    classified_non_semantic_variation: [
      "cargo build activity",
      "cargo build elapsed time",
      "rust test elapsed time",
    ],
  })}\n`
);

let focusedBaselineEvidence;
for (let iteration = 1; iteration <= focusedRepetitions; iteration += 1) {
  const result = runCargo(
    [
      "test",
      "-p",
      "openlife-core",
      "--jobs=1",
      focusedTestId,
      "--",
      "--exact",
      "--nocapture",
      "--test-threads=1",
    ],
    5 * 60 * 1000
  );
  const combined = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  if (result.error || result.status !== 0) {
    fail(
      `W0-S2 focused iteration ${iteration}/${focusedRepetitions} failed: ${
        result.error?.message ?? `exit=${result.status}`
      }`,
      result
    );
  }
  const missingMarkers = focusedMarkers.filter(marker => !combined.includes(marker));
  if (missingMarkers.length > 0) {
    fail(
      `W0-S2 focused iteration ${iteration}/${focusedRepetitions} lacks markers: ${missingMarkers.join(
        ","
      )}`,
      result
    );
  }
  const semanticEvidence = focusedSemanticEvidence(combined);
  if (
    Object.values(semanticEvidence.marker_counts).some(count => count !== 1) ||
    semanticEvidence.exact_test_started_line_count !== 1 ||
    semanticEvidence.failed_test_line_count !== 0 ||
    semanticEvidence.panic_count !== 0
  ) {
    fail(
      `W0-S2 focused iteration ${iteration}/${focusedRepetitions} has invalid semantic evidence: ${JSON.stringify(
        semanticEvidence
      )}`,
      result
    );
  }
  if (focusedBaselineEvidence === undefined) {
    focusedBaselineEvidence = semanticEvidence;
  } else if (
    JSON.stringify(semanticEvidence) !== JSON.stringify(focusedBaselineEvidence)
  ) {
    fail(
      `W0-S2 focused iteration ${iteration}/${focusedRepetitions} has unexplained semantic variation: baseline=${JSON.stringify(
        focusedBaselineEvidence
      )} observed=${JSON.stringify(semanticEvidence)}`,
      result
    );
  }
  process.stdout.write(
    `${JSON.stringify({
      event: "W0-S2-FOCUSED-ITERATION",
      iteration,
      status: "PASS",
      observed_failure_layers: [
        "process_registry",
        "os_owner_lock_would_block",
      ],
      reopened_same_canonical_slot: true,
      semantic_evidence_sha256: digestJson(semanticEvidence),
    })}\n`
  );
}

let workspaceBaselineEvidence;
for (let iteration = 1; iteration <= workspaceRepetitions; iteration += 1) {
  const result = runCargo(
    ["test", "--workspace", "--jobs=1", "--", "--test-threads=1"],
    45 * 60 * 1000
  );
  const combined = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  if (result.error || result.status !== 0) {
    fail(
      `W0-S2 serial workspace iteration ${iteration}/${workspaceRepetitions} failed: ${
        result.error?.message ?? `exit=${result.status}`
      }`,
      result
    );
  }
  const semanticEvidence = workspaceSemanticEvidence(combined);
  if (
    semanticEvidence.test_result_summaries.length === 0 ||
    semanticEvidence.test_result_summaries.some(
      summary => !summary.startsWith("test result: ok.")
    ) ||
    semanticEvidence.failed_test_line_count !== 0 ||
    semanticEvidence.panic_count !== 0
  ) {
    fail(
      `W0-S2 serial workspace iteration ${iteration}/${workspaceRepetitions} has invalid semantic evidence: ${JSON.stringify(
        semanticEvidence
      )}`,
      result
    );
  }
  if (workspaceBaselineEvidence === undefined) {
    workspaceBaselineEvidence = semanticEvidence;
  } else if (
    JSON.stringify(semanticEvidence) !== JSON.stringify(workspaceBaselineEvidence)
  ) {
    fail(
      `W0-S2 serial workspace iteration ${iteration}/${workspaceRepetitions} has unexplained semantic variation: baseline=${JSON.stringify(
        workspaceBaselineEvidence
      )} observed=${JSON.stringify(semanticEvidence)}`,
      result
    );
  }
  process.stdout.write(
    `${JSON.stringify({
      event: "W0-S2-WORKSPACE-ITERATION",
      iteration,
      status: "PASS",
      concurrency: 1,
      semantic_evidence_sha256: digestJson(semanticEvidence),
    })}\n`
  );
}

process.stdout.write(
  `${JSON.stringify({
    event: "W0-S2-SUMMARY",
    status: "PASS",
    focused_passed: focusedRepetitions,
    focused_required: focusedRepetitions,
    workspace_passed: workspaceRepetitions,
    workspace_required: workspaceRepetitions,
    unexplained_variation_count: 0,
  })}\n`
);
