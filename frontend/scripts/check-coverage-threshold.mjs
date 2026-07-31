import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export const COVERAGE_THRESHOLD = 60;

export function validateCoverageSummary(summary) {
  const lines = summary?.total?.lines;
  const total = lines?.total;
  const covered = lines?.covered;
  const skipped = lines?.skipped;
  const percent = lines?.pct;

  if (
    !Number.isInteger(total) ||
    !Number.isInteger(covered) ||
    !Number.isInteger(skipped) ||
    !Number.isFinite(percent)
  ) {
    throw new Error(
      "COVERAGE-NONNUMERIC: total.lines total, covered, skipped and pct must be numeric"
    );
  }
  if (total <= 0) {
    throw new Error("COVERAGE-ZERO-COLLECTION: total.lines.total must be greater than zero");
  }
  const expectedPercent = Math.floor((covered / total) * 10_000) / 100;
  if (
    covered < 0 ||
    covered > total ||
    skipped < 0 ||
    skipped !== 0 ||
    percent < 0 ||
    percent > 100 ||
    percent !== expectedPercent
  ) {
    throw new Error(
      `COVERAGE-INCONSISTENT: total.lines fields disagree (${covered}/${total}, skipped ${skipped}, pct ${percent})`
    );
  }
  if (percent < COVERAGE_THRESHOLD) {
    throw new Error(
      `COVERAGE-BELOW-THRESHOLD: line coverage ${percent}% is below ${COVERAGE_THRESHOLD}%`
    );
  }

  return { total, covered, skipped, percent, threshold: COVERAGE_THRESHOLD };
}

export function checkCoverageFile(path) {
  let raw;
  try {
    raw = readFileSync(path, "utf8");
  } catch {
    throw new Error(`COVERAGE-MISSING: coverage summary is missing at ${path}`);
  }

  let summary;
  try {
    summary = JSON.parse(raw);
  } catch {
    throw new Error(`COVERAGE-NONNUMERIC: coverage summary is not valid JSON at ${path}`);
  }
  return validateCoverageSummary(summary);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const path = process.argv[2] ?? "coverage/coverage-summary.json";
  try {
    const result = checkCoverageFile(path);
    console.log(
      `Frontend coverage: ${result.percent}% across ${result.total} lines (threshold ${result.threshold}%)`
    );
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
