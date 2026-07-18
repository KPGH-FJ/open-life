import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const directory = path.dirname(fileURLToPath(import.meta.url));
const source = fs.readFileSync(path.join(directory, "phase3f-data.js"), "utf8");
const context = { window: {} };
vm.createContext(context);
vm.runInContext(source, context);

const model = context.window.OPENLIFE_BLUEPRINT_DATA;
const errors = [];
const requiredScreens = [
  "today-ready",
  "today-stale",
  "workspace",
  "workspace-unknown",
  "workspace-running",
  "workspace-resources-web",
  "tasks",
  "review-pending",
  "review-approved",
  "lifemodel",
  "settings",
];

for (const key of requiredScreens) {
  if (!model.screens[key]) errors.push(`missing screen: ${key}`);
}

for (const [key, screen] of Object.entries(model.screens)) {
  for (const field of [
    "key",
    "selectorLabel",
    "routeKey",
    "layout",
    "title",
    "status",
    "privacy",
    "inspector",
  ]) {
    if (!screen[field]) errors.push(`${key}: missing ${field}`);
  }

  if (!screen.status.sourceRef) errors.push(`${key}: status missing sourceRef`);
  if (!screen.privacy.sourceRef) errors.push(`${key}: privacy missing sourceRef`);
  if (screen.privacy.externalTransmission === "unknown" && screen.privacy.tone === "success") {
    errors.push(`${key}: unknown privacy cannot be success`);
  }

  for (const item of screen.actions || []) {
    for (const field of ["id", "kind", "label", "targetRef", "sourceRef"]) {
      if (!item[field]) errors.push(`${key}/${item.id || "action"}: missing ${field}`);
    }
    if (typeof item.enabled !== "boolean")
      errors.push(`${key}/${item.id}: enabled must be boolean`);
    if (!item.enabled && !item.disabledReason)
      errors.push(`${key}/${item.id}: disabled action missing reason`);
  }

  for (const item of screen.inspector.evidence || []) {
    for (const field of ["id", "label", "source", "sensitivity", "summary"]) {
      if (!item[field]) errors.push(`${key}/${item.id || "evidence"}: missing ${field}`);
    }
  }
}

const todayReviewAction = model.screens["today-ready"].actions.find(
  item => item.id === "today:view-pending-review"
);
if (todayReviewAction?.outcome !== "review-pending") {
  errors.push("Today review view action must open pending decision");
}

const permissionAllow = model.screens.workspace.actions.find(
  item => item.id === "workspace:allow-once"
);
if (!permissionAllow?.enabled) errors.push("known exact one-time permission must be enabled");
if (permissionAllow?.outcome !== "permission-confirm-and-resume") {
  errors.push("known permission must use confirm-refresh-resume fixture flow");
}
for (const field of ["scopeKind", "policy", "inputDigest", "blockedActionId"]) {
  if (!model.screens.workspace.permission[field]) {
    errors.push(`known permission missing ${field}`);
  }
}

const unknownPermissionAllow = model.screens["workspace-unknown"].actions.find(
  item => item.id === "workspace:allow-once"
);
if (unknownPermissionAllow?.enabled) {
  errors.push("unknown permission scope must remain disabled");
}
if (!unknownPermissionAllow?.disabledReason) {
  errors.push("unknown permission must explain why it is disabled");
}

if (model.screens["review-approved"].status.label !== "已批准，尚未应用") {
  errors.push("approved screen must remain distinct from applied/completed");
}

const applyAction = model.screens["review-approved"].actions.find(item => item.kind === "apply");
if (applyAction?.enabled) errors.push("apply must remain disabled without command contract");

const settings = model.screens.settings;
if (!settings.categories?.length || !settings.config) {
  errors.push("settings must define dedicated categories and a draft config fixture");
}
for (const actionId of ["settings:test-provider", "settings:save-provider"]) {
  if (!settings.actions.find(item => item.id === actionId && item.enabled)) {
    errors.push(`settings missing enabled ${actionId} interaction`);
  }
}

if (model.screens["workspace-resources-web"].privacy.externalTransmission !== "sent") {
  errors.push("Web evidence scenario must disclose external transmission");
}

if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}

const actionCount = Object.values(model.screens).reduce(
  (count, screen) => count + (screen.actions?.length || 0),
  0
);
const evidenceCount = Object.values(model.screens).reduce(
  (count, screen) => count + (screen.inspector.evidence?.length || 0),
  0
);

console.log(
  `Phase 3F validation passed: ${Object.keys(model.screens).length} screens, ${actionCount} actions, ${evidenceCount} evidence refs.`
);
