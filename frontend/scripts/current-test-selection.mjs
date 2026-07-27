import { createHash } from "node:crypto";
import { relative, sep } from "node:path";

export const EXPECTED_CURRENT_TEST_COUNT = 193;
export const EXPECTED_CURRENT_TEST_SHA256 =
  "7299bcdb9ff6e7a6ac6988579a32c49e457ad58dbec5597d4f400c6e27ef302e";

const forbiddenPaths = ["src/stage1BrowserEvidence.test.ts", "src/step6ProductAcceptance.test.ts"];

export function currentTestEnv(env = process.env) {
  return { ...env, OPENLIFE_VITEST_SCOPE: "current" };
}

function normalizedPath(path, root) {
  return relative(root, path).split(sep).join("/");
}

export function normalizeTestIds(entries, root) {
  return entries
    .map(entry => `${normalizedPath(entry.file, root)} :: ${entry.name}`)
    .sort((left, right) => left.localeCompare(right));
}

export function testIdDigest(ids) {
  return createHash("sha256")
    .update(`${ids.join("\n")}\n`, "utf8")
    .digest("hex");
}

export function validateCurrentTestSelection(
  entries,
  root,
  expectedCount = EXPECTED_CURRENT_TEST_COUNT,
  expectedDigest = EXPECTED_CURRENT_TEST_SHA256
) {
  if (!Array.isArray(entries) || entries.length === 0) {
    throw new Error("W0-TEST-ZERO-COLLECTION: current Vitest collection is empty");
  }

  const ids = normalizeTestIds(entries, root);
  const forbidden = ids.filter(id => {
    const path = id.split(" :: ", 1)[0];
    return (
      forbiddenPaths.includes(path) ||
      path.startsWith("src/dev/") ||
      path.startsWith("src/test/archive/")
    );
  });
  if (forbidden.length > 0) {
    throw new Error(`W0-TEST-FORBIDDEN-CREDIT: ${forbidden.join(", ")}`);
  }

  const digest = testIdDigest(ids);
  if (ids.length !== expectedCount || digest !== expectedDigest) {
    throw new Error(
      `W0-TEST-ID-DRIFT: expected ${expectedCount}/${expectedDigest}, got ${ids.length}/${digest}`
    );
  }

  return { count: ids.length, digest, ids };
}
