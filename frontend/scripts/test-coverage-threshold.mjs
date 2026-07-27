import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const checker = fileURLToPath(new URL("./check-coverage-threshold.mjs", import.meta.url));
const fixtureRoot = mkdtempSync(join(tmpdir(), "openlife-coverage-checker-"));

function runCase(name, summary, expectedCode, expectedExit) {
  const path = join(fixtureRoot, `${name}.json`);
  if (summary !== null) {
    writeFileSync(path, `${JSON.stringify(summary)}\n`);
  }
  const result = spawnSync(process.execPath, [checker, path], { encoding: "utf8" });
  assert.equal(result.status, expectedExit, `${name} exit status`);
  const output = `${result.stdout}${result.stderr}`;
  assert.match(output, new RegExp(expectedCode), `${name} diagnostic`);
  console.log(`PASS ${expectedCode}`);
}

try {
  runCase("missing", null, "W0-COV-MISSING", 1);
  runCase(
    "nonnumeric",
    { total: { lines: { total: 100, covered: 100, skipped: 0, pct: "unknown" } } },
    "W0-COV-NONNUMERIC",
    1
  );
  runCase(
    "zero",
    { total: { lines: { total: 0, covered: 0, skipped: 0, pct: 100 } } },
    "W0-COV-ZERO-COLLECTION",
    1
  );
  runCase(
    "inconsistent",
    { total: { lines: { total: 100, covered: 0, skipped: 0, pct: 100 } } },
    "W0-COV-INCONSISTENT",
    1
  );
  runCase(
    "below",
    { total: { lines: { total: 1000, covered: 599, skipped: 0, pct: 59.9 } } },
    "W0-COV-BELOW-THRESHOLD",
    1
  );
  runCase(
    "valid",
    { total: { lines: { total: 100, covered: 60, skipped: 0, pct: 60 } } },
    "Frontend coverage",
    0
  );
} finally {
  rmSync(fixtureRoot, { recursive: true, force: true });
}
