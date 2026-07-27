import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { currentTestEnv, validateCurrentTestSelection } from "./current-test-selection.mjs";

const frontendRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const vitestCli = resolve(frontendRoot, "node_modules/vitest/vitest.mjs");
const result = spawnSync(process.execPath, [vitestCli, "list", "--json"], {
  cwd: frontendRoot,
  encoding: "utf8",
  env: currentTestEnv(),
});

if (result.status !== 0) {
  console.error(result.stderr || result.stdout || "W0-TEST-ZERO-COLLECTION: Vitest list failed");
  process.exit(1);
}

try {
  const entries = JSON.parse(result.stdout);
  const selection = validateCurrentTestSelection(entries, frontendRoot);
  console.log(`Current frontend test credit: ${selection.count} IDs, sha256:${selection.digest}`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
