# Phase 3C Visual And Interaction QA Report

Status: third-pass static mockup QA complete.
Date: 2026-07-10.

## Scope

Checked the standalone HTML/CSS/JS mockup at:

```text
docs/phase3c_ui_foundation_shell_mockup/static_mockup/index.html
```

This report covers static fixtures only. It does not exercise production
ProductShell, React routes, Tauri commands, or live backend data. Browser QA
used a temporary local static server because browser automation does not allow
direct `file://` navigation; the delivered HTML still opens directly.

## Viewport Matrix

Seven states were checked at all required viewports.

| Viewport | States | Horizontal overflow | Clipping | Result |
| --- | ---: | ---: | --- | --- |
| `1440x900` | 7 | `0px` | none detected | pass |
| `1280x800` | 7 | `0px` | none detected | pass |
| `390x844` | 7 | `0px` | none detected | pass |

States:

1. `today-ready-pending-review`
2. `today-stale-unknown`
3. `workspace-waiting-permission`
4. `review-pending-decision`
5. `review-approved-not-materialized`
6. `lifemodel-limited-compat`
7. `settings-provider-privacy-unknown`

All 21 combinations also passed these assertions:

- QA selector remains outside the product shell;
- one primary status is present and is not repeated verbatim in the work surface;
- the product work surface contains no `ReviewItem`, `AgentProposal`,
  `ViewModel`, `EvidenceRef`, `fixture`, or `canonical_scope` copy;
- unknown/possible transmission never renders green;
- every disabled action has a visible reason;
- non-workspace desktop states keep the Inspector in the grid;
- Workspace defaults to a two-column focused layout and opens its Inspector on demand;
- mobile Inspector stays a modal sheet;
- desktop/mobile navigation hierarchy matches its intended placement;
- debug content opacity remains `1`.

## Workspace Density Refinement QA

The Workspace was changed from a dashboard-like composition to a focused task
timeline.

Verified results:

- decorative metrics, the full permission table, the duplicate blocker banner,
  and the separate progress section are absent from the Workspace main surface;
- the main surface shows one task objective and three timeline events;
- only the waiting permission event is expanded;
- the waiting event contains a bounded three-item scope summary and the exact
  Review Action controls;
- desktop Inspector is closed by default and the shell uses two columns;
- `查看任务依据` and `查看访问范围` open the Inspector without changing task or
  review state;
- at `390x844`, all three permission decisions are `44px` high and end at
  `730px`, above the bottom navigation at `781px`;
- the disabled one-time action reason ends at `748px` and remains visible;
- no text clipping or horizontal overflow was detected at any required viewport.

Desktop shell measurements:

| Viewport | Inspector closed columns | Focused content width |
| --- | --- | ---: |
| `1440x900` | `216px 1224px` | `980px` |
| `1280x800` | `204px 1076px` | `980px` |

Opening the Workspace Inspector at `1440x900` changes the shell to
`216px 892px 332px`. Closing it restores the two-column layout and returns
focus to the triggering action.

## Review And Permission QA

| Path | Observed result | Result |
| --- | --- | --- |
| Today `查看待决定建议` | Opens pending decision fixture | pass |
| Pending `批准变更` | Opens confirmation; state remains pending | pass |
| Cancel approval | Closes dialog; state remains pending | pass |
| Confirm approval | Opens `已批准，尚未应用` | pass |
| Approved state | `应用变更` remains disabled with backend-command reason | pass |
| Reject/later/edit | Each opens explicit static result dialog | pass |
| Permission `仅允许本次` | Disabled because duration/revocation contract is missing | pass |
| Permission `查看访问范围` | Opens the on-demand Inspector/sheet and focuses purpose, target, data, transmission, duration, and revoke | pass |
| Close permission sheet | Restores focus to the triggering action | pass |
| Permission `拒绝` | Opens explicit static feedback; task remains blocked and no file read occurs | pass |

The mockup therefore no longer treats a view action as approval and does not
optimistically move from approval to applying or applied.

## Navigation QA

- Mobile bottom navigation contains only 今日, 工作区, 审核中心, LifeModel.
- 任务 remains explicit and unavailable in the drawer/sidebar.
- 设置 is a utility destination, not mobile primary navigation.
- 支持信息 opens the collapsed technical section and is not a product route.
- Drawer Shift+Tab/Tab loops between first and last controls.
- Escape closes the drawer or Inspector and restores focus.
- Covered destinations update visible `aria-current="page"`.

## Accessibility And Type

- Product body text: `14px` with `22px` line height.
- Captions/metadata: at least `12px` in product surfaces.
- Mobile action controls: at least `42px` high.
- Mobile app-bar icon controls: `40px` square.
- Mobile bottom-navigation labels: `11px`.
- Contrast: muted on white `5.78:1`, secondary on white `8.10:1`, disabled
  text on neutral surface `4.51:1`.
- Dynamic state changes use a polite live region.
- Confirmation and mobile Inspector use native modal dialogs.
- No subtree opacity is used for debug degradation.
- Browser error/warning log: empty.

Semantic DOM and keyboard behavior were checked through browser automation.
No full VoiceOver or other physical screen-reader session was run.

## Screenshot Artifacts

The 21-state matrix uses:

```text
artifacts/phase3c_<viewport>_<state>.png
```

Additional interaction captures:

- `artifacts/phase3c_390x844_today-evidence-sheet.png`
- `artifacts/phase3c_390x844_mobile-nav-drawer.png`
- `artifacts/phase3c_390x844_review-confirmation-dialog.png`
- `artifacts/phase3c_1440x900_workspace-permission-scope.png`
- `artifacts/phase3c_390x844_workspace-permission-scope.png`

There are 26 screenshot artifacts. The five refreshed Workspace captures were
re-encoded and verified as real PNG images after the browser capture API
returned JPEG bytes.

## Safety Self-Review

- Stale, unknown, and missing-evidence states remain fail closed.
- Today opens a pending review item; it never jumps directly to approved.
- Approval is distinct from application and completion.
- One-time permission remains disabled until it is enforceable.
- Rich review/permission fields are marked `PROPOSED`, not current ReviewItem truth.
- Product, Review, and Debug actions remain separate in fixture data while
  ordinary users see task-appropriate labels.
- Product fixture values are static and do not claim backend readiness.
- Page code does not derive product truth from raw proposal fragments.

## Known Limitations

- The rich decision and permission contexts are target projections; current
  backend ReviewItem does not own them.
- No production action, route, provider call, or durable write is connected.
- Static reject/later/edit feedback validates interaction shape only.
- The Workspace timeline is a static target presentation; it is not a live
  runtime event stream.
- Inline SVGs remain mockup-only; React must use `lucide-react`.
- React migration remains blocked by `08_review_decision_projection_gap.md`.
