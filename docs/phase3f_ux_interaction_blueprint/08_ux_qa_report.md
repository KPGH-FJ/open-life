# Phase 3F UX And Interaction QA Report

Status: `PASS_FOR_HUMAN_REVIEW`
Date: 2026-07-18
Scope: standalone Phase 3F prototype only; no production frontend or backend
behavior was tested by this package.

## 1. Automated Results

| Check | Result |
|---|---|
| JavaScript syntax: data, app, QA scripts | PASS |
| fixture/action/evidence schema validator | PASS: 11 screens, 31 actions, 28 evidence refs |
| interaction QA | PASS: 57 assertions |
| direct `file://` interaction QA | PASS: same 57 assertions |
| screenshot capture | PASS: 33 screen/viewport images and 7 interaction images |
| browser console errors/warnings | PASS: none observed by the QA harness |
| horizontal overflow at required viewports | PASS |
| focus visibility and focus restoration assertions | PASS |
| unknown/privacy fail-closed assertions | PASS |
| approved-not-applied separation assertions | PASS |

The interaction harness validates navigation feedback, unavailable destinations,
fixture switching, Review decisions, exact permission confirmation, approval
then refreshed resume, attachment lifecycle, settings test/save separation,
mobile navigation, mobile evidence access, and dialog focus restoration.

## 2. Viewport Coverage

Every base scenario was captured at:

- `1440x900`;
- `1280x800`;
- `390x844`.

Base scenarios cover Today ready, Today stale/unknown, Workspace known exact
permission, Workspace unknown permission, Workspace running, Workspace with
resources and governed Web evidence, Tasks, Review pending, Review approved but
not applied, LifeModel limited compatibility, and Settings unknown boundary.

Interaction captures additionally cover mobile navigation, mobile evidence
sheet, mobile Review confirmation, permission Inspector, permission confirmation,
permission resume, and provider connection-test result.

Artifacts: `docs/phase3f_ux_interaction_blueprint/artifacts/`.

## 3. Manual Visual Review

### Desktop

- Workspace uses one dominant task narrative: current goal, progress, blocker,
  next action, then evidence. It no longer reads as a status-card matrix.
- Review shows the proposed change before actions, with current-to-suggested
  comparison, reason, source, risk, impact, and expiry in one decision flow.
- Settings follows a restrained Cursor-like one-column form with a dedicated
  category sidebar. Test and Save remain separate actions.
- Inspector begins with what happened, risk, and required action. Raw ids and
  source fields remain secondary.

### Mobile

- A compact app bar and bottom navigation replace the desktop sidebar.
- Evidence opens as a bottom sheet without forcing the user below the full page.
- Review actions remain fixed above bottom navigation and preserve readable
  labels at 390 px.
- Settings categories collapse into a select control; the form remains a single
  scroll column.

No incoherent overlap, clipped action label, blank region, or unintended
horizontal scrolling was found in the reviewed captures.

## 4. Accessibility Review

Verified by markup and automated browser assertions:

- current navigation uses `aria-current="page"`;
- dynamic status changes use a live status region;
- navigation, scenario selector, primary actions, Review decisions, and
  Inspector have deterministic keyboard order;
- dialogs trap initial intent through explicit focus and restore focus on close;
- disabled controls expose a visible reason and are not fake-clickable;
- focus rings are visible against the white baseline;
- body/caption colors meet the 4.5:1 target in the token combinations checked;
  the lowest checked pair is 5.50:1.

Not verified: a full VoiceOver reading pass, Windows screen readers, browser
zoom above 200%, localization expansion beyond the current Chinese-first copy,
and production Tauri semantics.

## 5. QA Interpretation

This result proves that the interaction proposal is internally consistent as a
static prototype. It does not prove that current backend projections contain
all fields shown by `TARGET_CONTRACT` fixtures, and it does not authorize a
production React migration. The open blockers are recorded in
`07_hallucination_and_contract_risk_audit.md`.
