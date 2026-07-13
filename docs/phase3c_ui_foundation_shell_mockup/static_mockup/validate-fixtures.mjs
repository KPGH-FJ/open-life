import fs from "node:fs";
import vm from "node:vm";

const dataPath = new URL("./mockup-data.js", import.meta.url);
const source = fs.readFileSync(dataPath, "utf8");
const sandbox = { window: {} };
vm.createContext(sandbox);
vm.runInContext(source, sandbox, { filename: dataPath.pathname });

const states = sandbox.window.OPENLIFE_MOCKUP_STATES || [];
const navItems = sandbox.window.OPENLIFE_MOCKUP_NAV || [];
const failures = [];

const requiredStateIds = new Set([
  "today-ready-pending-review",
  "today-stale-unknown",
  "workspace-waiting-permission",
  "review-pending-decision",
  "review-approved-not-materialized",
  "lifemodel-limited-compat",
  "settings-provider-privacy-unknown",
]);

const evidenceSources = new Set([
  "backend-readmodel",
  "audit",
  "task",
  "review",
  "memory",
  "lifemodel",
  "settings",
  "provider",
]);
const evidenceSensitivity = new Set(["public", "local_private", "sensitive", "redacted"]);
const productKinds = new Set([
  "open",
  "start",
  "continue",
  "retry",
  "cancel",
  "refresh",
  "inspect",
  "configure",
]);
const reviewEffects = {
  approve: "decision_only",
  reject: "decision_only",
  edit: "decision_only",
  later: "decision_only",
  revoke: "decision_only",
  apply: "materialization_request",
  resume: "task_resume_request",
  view_evidence: "evidence_only",
};
const debugKinds = new Set([
  "raw_trace",
  "raw_json",
  "export",
  "provider_health",
  "route_evidence",
  "transcript",
]);
const behaviorTypes = new Set(["navigate", "open_inspector", "dialog", "confirm_transition"]);

function fail(path, message) {
  failures.push(`${path}: ${message}`);
}

function requireString(value, path) {
  if (typeof value !== "string" || value.trim() === "") fail(path, "expected non-empty string");
}

function validateSourceRef(item, path) {
  requireString(item.sourceRef, `${path}.sourceRef`);
}

function validateEvidenceRef(item, path) {
  requireString(item.id, `${path}.id`);
  requireString(item.label, `${path}.label`);
  if (!evidenceSources.has(item.source)) fail(`${path}.source`, `unsupported value ${item.source}`);
  if (item.sensitivity && !evidenceSensitivity.has(item.sensitivity)) {
    fail(`${path}.sensitivity`, `unsupported value ${item.sensitivity}`);
  }
}

function validateFixtureBehavior(action, path) {
  if (!action.enabled) return;
  if (!action.fixtureBehavior || !behaviorTypes.has(action.fixtureBehavior.type)) {
    fail(`${path}.fixtureBehavior`, "enabled fixture action needs a verifiable static behavior");
  }
}

function validateBaseAction(action, path) {
  requireString(action.id, `${path}.id`);
  requireString(action.label, `${path}.label`);
  if (typeof action.enabled !== "boolean") fail(`${path}.enabled`, "expected boolean");
  requireString(action.targetRef || action.targetReviewItemId, `${path}.targetRef`);
  validateSourceRef(action, path);
  if (!action.enabled) requireString(action.disabledReason, `${path}.disabledReason`);
  validateFixtureBehavior(action, path);
}

function validateProductAction(action, path) {
  validateBaseAction(action, path);
  if (!productKinds.has(action.kind)) fail(`${path}.kind`, `unsupported ProductAction kind ${action.kind}`);
  requireString(action.targetRef, `${path}.targetRef`);
}

function validateReviewAction(action, path) {
  validateBaseAction(action, path);
  if (!(action.kind in reviewEffects)) fail(`${path}.kind`, `unsupported ReviewAction kind ${action.kind}`);
  if (reviewEffects[action.kind] !== action.effect) {
    fail(`${path}.effect`, `expected ${reviewEffects[action.kind]} for ${action.kind}`);
  }
  if (typeof action.requiresConfirmation !== "boolean") {
    fail(`${path}.requiresConfirmation`, "expected boolean");
  }
  requireString(action.targetReviewItemId, `${path}.targetReviewItemId`);
  if (action.kind === "approve") {
    if (!action.requiresConfirmation) {
      fail(`${path}.requiresConfirmation`, "approve fixture must require confirmation");
    }
    if (action.expectedMaterializationStatusAfterDispatch !== "unknown") {
      fail(
        `${path}.expectedMaterializationStatusAfterDispatch`,
        "current approve contract must remain unknown until backend materialization is proven",
      );
    }
  }
  if (action.kind === "apply") {
    if (!action.requiresConfirmation) {
      fail(`${path}.requiresConfirmation`, "apply fixture must require confirmation");
    }
    if (action.enabled && action.expectedMaterializationStatusAfterDispatch !== "applying") {
      fail(
        `${path}.expectedMaterializationStatusAfterDispatch`,
        "an enabled apply fixture must express the applying transition",
      );
    }
    if (!action.enabled) {
      requireString(action.contractGapRef, `${path}.contractGapRef`);
    }
  }
}

function validateDebugAction(action, path) {
  requireString(action.id, `${path}.id`);
  requireString(action.label, `${path}.label`);
  if (!debugKinds.has(action.kind)) fail(`${path}.kind`, `unsupported DebugAction kind ${action.kind}`);
  if (typeof action.enabled !== "boolean") fail(`${path}.enabled`, "expected boolean");
  if (typeof action.developerOnly !== "boolean") fail(`${path}.developerOnly`, "expected boolean");
  requireString(action.targetRef, `${path}.targetRef`);
  validateSourceRef(action, path);
  validateFixtureBehavior(action, path);
}

const stateIds = new Set(states.map((state) => state.id));
for (const requiredId of requiredStateIds) {
  if (!stateIds.has(requiredId)) fail("states", `missing required state ${requiredId}`);
}

const actionIds = new Set();
for (const [stateIndex, state] of states.entries()) {
  const root = `states[${stateIndex}](${state.id})`;
  requireString(state.id, `${root}.id`);
  requireString(state.navKey, `${root}.navKey`);
  requireString(state.envelope?.status, `${root}.envelope.status`);
  if (state.envelope?.source !== "backend-readmodel") {
    fail(`${root}.envelope.source`, "must preserve backend-readmodel contract shape");
  }
  validateSourceRef(state.primaryStatus || {}, `${root}.primaryStatus`);
  validateSourceRef(state.goal || {}, `${root}.goal`);
  validateSourceRef(state.blocker || {}, `${root}.blocker`);
  for (const key of ["happened", "risk", "next"]) {
    requireString(state.inspectorSummary?.[key], `${root}.inspectorSummary.${key}`);
  }
  if (/Phase 3C|fixture|mockup|视觉 QA/i.test(`${state.goal?.title} ${state.goal?.summary}`)) {
    fail(`${root}.goal`, "product story must not describe mockup self-testing");
  }

  const boundary = state.privacyBoundary || {};
  for (const key of [
    "routeType",
    "externalTransmission",
    "providerLabel",
    "modelLabel",
    "privacyLabel",
    "risk",
  ]) {
    requireString(boundary[key], `${root}.privacyBoundary.${key}`);
  }
  if (typeof boundary.localOnlyRequired !== "boolean") {
    fail(`${root}.privacyBoundary.localOnlyRequired`, "expected boolean");
  }
  if (
    ["unknown", "possible"].includes(boundary.externalTransmission) &&
    state.primaryStatus?.label?.includes("本地")
  ) {
    fail(`${root}.primaryStatus.label`, "unknown/possible transmission cannot claim local certainty");
  }

  for (const [index, metric] of state.metrics.entries()) {
    validateSourceRef(metric, `${root}.metrics[${index}]`);
  }
  for (const [index, event] of (state.timeline || []).entries()) {
    requireString(event.id, `${root}.timeline[${index}].id`);
    requireString(event.status, `${root}.timeline[${index}].status`);
    requireString(event.title, `${root}.timeline[${index}].title`);
    requireString(event.body, `${root}.timeline[${index}].body`);
    validateSourceRef(event, `${root}.timeline[${index}]`);
  }
  for (const [index, item] of (state.permissionContext?.summaryItems || []).entries()) {
    requireString(item.label, `${root}.permissionContext.summaryItems[${index}].label`);
    validateSourceRef(item, `${root}.permissionContext.summaryItems[${index}]`);
  }
  for (const [sectionIndex, section] of state.sections.entries()) {
    for (const [rowIndex, row] of section.rows.entries()) {
      validateSourceRef(row, `${root}.sections[${sectionIndex}].rows[${rowIndex}]`);
    }
  }
  for (const [index, item] of state.evidenceRefs.entries()) {
    validateEvidenceRef(item, `${root}.evidenceRefs[${index}]`);
  }
  for (const [index, item] of (boundary.evidenceRefs || []).entries()) {
    validateEvidenceRef(item, `${root}.privacyBoundary.evidenceRefs[${index}]`);
  }

  const lanes = state.actions || {};
  for (const [index, action] of (lanes.primary || []).entries()) {
    validateProductAction(action, `${root}.actions.primary[${index}]`);
    if (actionIds.has(action.id)) fail(`${root}.actions.primary[${index}].id`, "duplicate action id");
    actionIds.add(action.id);
  }
  for (const [index, action] of (lanes.review || []).entries()) {
    validateReviewAction(action, `${root}.actions.review[${index}]`);
    if (actionIds.has(action.id)) fail(`${root}.actions.review[${index}].id`, "duplicate action id");
    actionIds.add(action.id);
  }
  for (const [index, action] of (lanes.debugOnly || []).entries()) {
    validateDebugAction(action, `${root}.actions.debugOnly[${index}]`);
    if (actionIds.has(action.id)) fail(`${root}.actions.debugOnly[${index}].id`, "duplicate action id");
    actionIds.add(action.id);
  }
}

for (const state of states) {
  for (const lane of ["primary", "review", "debugOnly"]) {
    for (const action of state.actions?.[lane] || []) {
      const behavior = action.fixtureBehavior;
      if (behavior && ["navigate", "confirm_transition"].includes(behavior.type)) {
        if (!stateIds.has(behavior.stateId)) {
          fail(`${state.id}.${action.id}.fixtureBehavior.stateId`, "target state does not exist");
        }
      }
    }
  }
}

const tasksNav = navItems.find((item) => item.key === "tasks");
if (!tasksNav?.unavailable) fail("nav.tasks", "must expose an explicit unavailable state");
if (navItems.some((item) => item.key === "advanced")) {
  fail("nav.advanced", "debug/support must not remain a top-level product destination");
}
const settingsNav = navItems.find((item) => item.key === "settings");
if (settingsNav?.placement !== "utility" || settingsNav.mobilePrimary) {
  fail("nav.settings", "settings must stay in utility navigation, outside mobile primary nav");
}

const todayState = states.find((state) => state.id === "today-ready-pending-review");
const todayReviewAction = todayState?.actions?.primary?.find(
  (action) => action.id === "today:open-pending-review",
);
if (todayReviewAction?.fixtureBehavior?.stateId !== "review-pending-decision") {
  fail("today:open-pending-review", "viewing a pending review must enter the pending decision state");
}
if (
  todayState?.actions?.primary?.some(
    (action) => action.fixtureBehavior?.stateId === "review-approved-not-materialized",
  )
) {
  fail("today.actions", "a view action must not skip directly to approved state");
}

const pendingReviewState = states.find((state) => state.id === "review-pending-decision");
for (const key of ["before", "after", "reason", "source", "risk", "impact", "expires", "target"]) {
  requireString(pendingReviewState?.reviewContext?.[key], `review-pending-decision.reviewContext.${key}`);
}
if (pendingReviewState?.reviewContext?.contractStatus !== "PROPOSED_REVIEW_PROJECTION") {
  fail("review-pending-decision.reviewContext", "must mark the missing projection as PROPOSED");
}
for (const kind of ["reject", "later", "edit", "approve"]) {
  if (!pendingReviewState?.actions?.review?.some((action) => action.kind === kind)) {
    fail("review-pending-decision.actions.review", `missing ${kind} decision action`);
  }
}

const workspaceState = states.find((state) => state.id === "workspace-waiting-permission");
if (workspaceState?.layout !== "workspace_timeline") {
  fail("workspace.layout", "workspace must use the focused timeline presentation");
}
if (workspaceState?.inspectorMode !== "on_demand") {
  fail("workspace.inspectorMode", "workspace Inspector must default to on-demand");
}
if (workspaceState?.metrics?.length !== 0 || workspaceState?.sections?.length !== 0) {
  fail("workspace.density", "workspace must not restore decorative metrics or duplicate progress sections");
}
if (workspaceState?.timeline?.filter((event) => event.status === "waiting").length !== 1) {
  fail("workspace.timeline", "workspace must expose exactly one expanded waiting decision event");
}
for (const key of [
  "tool",
  "capability",
  "target",
  "dataScope",
  "transmission",
  "duration",
  "revocation",
  "currentPolicy",
]) {
  requireString(workspaceState?.permissionContext?.[key], `workspace.permissionContext.${key}`);
}
if ((workspaceState?.permissionContext?.summaryItems || []).length !== 3) {
  fail("workspace.permissionContext.summaryItems", "workspace needs three bounded scope summary items");
}
const allowOnceAction = workspaceState?.actions?.review?.find(
  (action) => action.id === "workspace:allow-once",
);
if (!allowOnceAction || allowOnceAction.enabled || !allowOnceAction.disabledReason) {
  fail("workspace:allow-once", "one-time permission must fail closed until its contract exists");
}

const reviewState = states.find((state) => state.id === "review-approved-not-materialized");
if (!reviewState?.primaryStatus?.label.includes("尚未应用")) {
  fail("review-approved-not-materialized", "must keep approved separate from materialized");
}

if (failures.length) {
  console.error(`Fixture contract validation failed (${failures.length}):`);
  for (const failure of failures) console.error(`- ${failure}`);
  process.exitCode = 1;
} else {
  console.log(
    `Fixture contract validation passed: ${states.length} states, ${actionIds.size} actions, ${navItems.length} navigation entries.`,
  );
}
