# Phase 3E Visual QA Report

Status: `REVIEW_CANDIDATE_VERIFIED_AFTER_WHITE_BASELINE_RESET`.
Date: 2026-07-18.
Production port gate: `NO`.

## QA Scope

The review candidate was checked as real HTML/CSS/JS, not as generated image
text. QA covered:

- eight desktop product states;
- the same eight states at `1280x800` and `390x844`;
- mobile navigation, evidence sheet, permission and review actions;
- action/evidence fixture contracts;
- fail-closed privacy, freshness, permission, and application semantics;
- keyboard focus, live announcements, target size, and contrast;
- the standalone visual review board.

## Human Visual Correction

The initial pine/mineral custom palette was rejected in human review. The
candidate was rebuilt against the common current Codex/Cursor light-interface
baseline:

- white canvas and main surface;
- `#F5F5F5` sidebar;
- near-black text and primary actions;
- neutral gray selected rows;
- 1px `#E6E6E6` structural dividers;
- system UI sans typography with weights normalized to 400/500/600;
- no product-wide brand accent color;
- semantic color retained only for waiting, concrete errors, verified success,
  and keyboard focus.

The current local Cursor Agents window and official Codex product images were
used as references. Their logos, assets, and product copy were not copied.

## Automated Results

```text
node --check prototype/blueprint-data.js
PASS

node --check prototype/blueprint-app.js
PASS

node prototype/validate-blueprints.mjs
Blueprint validation passed: 8 screens, 22 actions, 21 evidence refs.

node prototype/capture-blueprints.mjs
Visual capture passed: 24 screen/viewport images plus 4 interaction images.

node prototype/interaction-qa.mjs
Interaction QA passed: 35 assertions.

node review/validate-review.mjs
Review board QA passed: 2 viewports, all images and section anchors resolved.

git diff --check
PASS

git diff --no-index --check /tmp/openlife-empty-visual <new-phase-directory>
No whitespace diagnostics. Exit 1 is expected because the compared directory
contains additions.
```

The capture run checked console warnings/errors, page errors, horizontal
overflow, and visible text escaping the viewport. No failure was reported at:

- `1440x900`;
- `1280x800`;
- `390x844`.

## Interaction Results

Verified behavior:

- `查看待审核建议` opens `review-pending`; it does not approve anything.
- `批准变更` first opens confirmation and leaves the item pending.
- confirmed approval enters `已批准，尚未应用`.
- `应用变更` remains disabled without refreshed read-model proof.
- Workspace permission cannot be granted while duration, revocation, or
  transmission scope is unproven.
- permission scope exposes tool, capability, target, data, transmission,
  duration, and revocation.
- stale Today uses amber/fail-closed semantics and no green success state.
- current navigation exposes `aria-current="page"`.
- screen changes are announced through an `aria-live` region.
- desktop and mobile evidence surfaces restore focus to their trigger.
- mobile decision actions remain above the bottom navigation and meet the
  minimum tested target height.

## Visual Inspection

Manually inspected:

- all `1440x900` product screenshots;
- Workspace and Review at `1280x800`;
- Today, Workspace, Review, navigation drawer, and evidence sheet at
  `390x844`;
- full desktop and mobile visual review boards.

Result:

- Today has one primary focus and one decision pressure, without repeated
  metric cards.
- the shell now reads as a white agent workbench rather than a bespoke branded
  dashboard.
- navigation selection is a neutral gray row; custom colored rails were
  removed.
- primary actions are black; evidence and ordinary information are neutral.
- Workspace reads as one execution thread with an inline interruption and
  composer, not a status matrix.
- Review is diff-first and keeps decisions visible.
- LifeModel uses a quieter reading rhythm with provenance.
- Settings leads with privacy boundaries and never shows unknown as local
  success.
- Inspector is closed by default and technical fields are ordered last.
- mobile uses a separate app bar, drawer, bottom navigation, and evidence
  sheet instead of stacking the desktop sidebar.

## Accessibility And Contrast

- Body text is 14px; primary reading copy is 15px; metadata is at least 12px.
- Focus-visible uses a 2px external blue ring.
- Disabled text is not degraded through subtree opacity and has a visible
  reason.
- Status is always communicated with text, not color alone.
- Tested normal-text combinations meet the `4.5:1` target, including muted
  `#666666` text on white/subtle surfaces, secondary text on the gray sidebar,
  and warning/error text on their semantic surfaces.
- `prefers-reduced-motion` removes nonessential review-board transitions.

This is a bounded static accessibility check, not a full VoiceOver or formal
screen-reader certification.

## Figma Result

An editable Figma file was created and one complete Today page was imported:

`https://www.figma.com/design/NncIE0ZWOaxAT9jYsFFKek`

The imported page predates the white-baseline correction. The authenticated
Figma account reached its Starter-plan MCP call limit before it could be
updated. The file is a layout reference only and must not be used as color or
token authority. The repository review board, prototype, and regenerated
screenshots are the complete current visual review package.

## Known Limitations

- Fixtures do not connect to React, Tauri, Rust, or production routes.
- Workspace and Tasks still depend on target/limited ViewModel contracts.
- current `ReviewItem` does not yet project the complete before/after, impact,
  permission scope, duration, or revocation context shown by the blueprint.
- approval and apply commands are intentionally not simulated as production
  writes.
- no external provider, real file permission, durable LifeModel write, or
  backend materialization was executed.
- Figma palette/component-library alignment is blocked by the account quota
  and is not represented as done.

## QA Verdict

The package is ready for human visual review. It is not approved and is not
ready for a React port until the user accepts or revises the visual direction
and the contract blockers are scoped.
