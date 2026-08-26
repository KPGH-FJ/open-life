import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

const fixtureSource = fs.readFileSync(new URL("./fixtures.js", import.meta.url), "utf8");
const appSource = fs.readFileSync(new URL("./app.js", import.meta.url), "utf8");
const htmlSource = fs.readFileSync(new URL("./index.html", import.meta.url), "utf8");
const cssSource = fs.readFileSync(new URL("./styles.css", import.meta.url), "utf8");
const context = { window: {} };
vm.runInNewContext(fixtureSource, context);

const data = context.window.OPENLIFE_FIXTURES;
assert.ok(data, "fixtures must expose OPENLIFE_FIXTURES");
assert.equal(data.journeys.length, 12, "the review surface must retain all 12 journeys");
assert.ok(data.actionBehaviors, "visible product actions need explicit behavior contracts");

for (const journey of data.journeys) {
  assert.equal(journey.states.length, 3, `${journey.id} must retain three deterministic review states`);
  for (const view of journey.states) {
    for (const label of [...(view.decision?.actions ?? []), ...(view.result?.actions ?? [])]) {
      assert.ok(data.actionBehaviors[label], `${journey.id}: action '${label}' has no explicit behavior`);
    }
  }
}

const onboarding = data.journeys.find(journey => journey.id === "onboarding").states[0];
assert.doesNotMatch(onboarding.stateTitle, /没有可用/, "onboarding cannot deny profiles while the picker lists usable profiles");

const readActive = data.journeys.find(journey => journey.id === "read").states[1];
assert.equal(readActive.inspector.kind, "scope", "default scope details must derive from the active view");

const reviewDecision = data.journeys.find(journey => journey.id === "review").states[1].decision;
assert.doesNotMatch(reviewDecision.title, /替换已存在/, "recoverable in-Project edits must not create approval tax");

const explicitMemory = data.journeys.find(journey => journey.id === "memory").states[1];
assert.ok(explicitMemory.result && !explicitMemory.decision, "an explicit 'remember' request should save with undo, not ask twice");

const historySearch = data.journeys.find(journey => journey.id === "history").states[0];
assert.equal(historySearch.composerVariant, "search", "history search must not masquerade as an Agent prompt");

assert.match(htmlSource, /id="resource-picker"/, "the visible Add control needs a resource picker");
assert.match(htmlSource, /id="profile-manager"/, "Settings and Manage Profiles need a real profile surface");
assert.match(appSource, /data\.actionBehaviors/, "runtime actions must use the explicit behavior map");
assert.match(cssSource, /grid-template-rows:\s*auto minmax\(0, 1fr\) auto/, "compact diff rows must not create a blank vertical gap");

console.log("OpenLife prototype contract check passed.");
