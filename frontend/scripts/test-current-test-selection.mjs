import assert from "node:assert/strict";
import { resolve } from "node:path";
import {
  normalizeTestIds,
  testIdDigest,
  validateCurrentTestSelection,
} from "./current-test-selection.mjs";

const root = resolve("/tmp/openlife-test-selection-fixture");
const validEntries = [
  { file: resolve(root, "src/App.test.tsx"), name: "renders current route" },
  { file: resolve(root, "src/tauri.test.ts"), name: "normalizes current wrapper" },
];
const validIds = normalizeTestIds(validEntries, root);
const validDigest = testIdDigest(validIds);

assert.throws(
  () => validateCurrentTestSelection([], root, 0, validDigest),
  /W0-TEST-ZERO-COLLECTION/
);
console.log("PASS W0-TEST-ZERO-COLLECTION");

assert.throws(
  () =>
    validateCurrentTestSelection(
      [
        {
          file: resolve(root, "src/stage1BrowserEvidence.test.ts"),
          name: "retired evidence",
        },
      ],
      root,
      1,
      "unused"
    ),
  /W0-TEST-FORBIDDEN-CREDIT/
);
console.log("PASS W0-TEST-FORBIDDEN-CREDIT");

assert.throws(
  () => validateCurrentTestSelection(validEntries, root, validEntries.length + 1, validDigest),
  /W0-TEST-ID-DRIFT/
);
console.log("PASS W0-TEST-ID-DRIFT");

const valid = validateCurrentTestSelection(validEntries, root, validEntries.length, validDigest);
assert.equal(valid.count, 2);
assert.equal(valid.digest, validDigest);
console.log("PASS W0-TEST-SELECTION-VALID");
