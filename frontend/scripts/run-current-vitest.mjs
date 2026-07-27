import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { currentTestEnv } from "./current-test-selection.mjs";

const frontendRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const vitestCli = resolve(frontendRoot, "node_modules/vitest/vitest.mjs");
const result = spawnSync(process.execPath, [vitestCli, ...process.argv.slice(2)], {
  cwd: frontendRoot,
  env: currentTestEnv(),
  stdio: "inherit",
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
if (result.signal) {
  console.error(`Current Vitest process terminated by ${result.signal}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
