# Agent Development Instructions: Phase 3C UI Foundation + ProductShell/Shell Static Mockup

You are implementing:

```text
Phase 3C: UI Foundation + ProductShell/Shell Static Mockup
```

This is a design-system/static-mockup slice. It is not a ProductShell
replacement and not full Frontend V2.

## Required Reading

Read these files before editing:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`
5. `docs/phase1_ux_ia/11_ui_foundation_study.md`
6. `docs/phase3c_ui_foundation_shell_mockup/00_development_preparation.md`
7. `docs/phase3c_ui_foundation_shell_mockup/01_source_map_and_design_baseline.md`
8. `docs/phase3c_ui_foundation_shell_mockup/02_ui_foundation_and_static_mockup_spec.md`

## Core Instruction

Build a standalone static OpenLife workbench shell mockup under:

```text
docs/phase3c_ui_foundation_shell_mockup/static_mockup/
```

The mockup must use real HTML/CSS layout, real fixed font sizes, real spacing,
and CSS custom properties. Do not use AI-generated bitmap text as the primary
mockup.

Recommended files:

```text
docs/phase3c_ui_foundation_shell_mockup/static_mockup/index.html
docs/phase3c_ui_foundation_shell_mockup/static_mockup/openlife-ui-foundation.css
docs/phase3c_ui_foundation_shell_mockup/static_mockup/mockup-data.js
docs/phase3c_ui_foundation_shell_mockup/04_visual_qa_report.md
docs/phase3c_ui_foundation_shell_mockup/05_summary.md
```

`mockup-data.js` is optional. Use it only if it makes fixture/state switching
cleaner.

## Implementation Scope

Allowed:

- create standalone static mockup files under the Phase 3C docs directory;
- create CSS variables for typography, spacing, radius, color, layout, and
  state tokens;
- use `lucide`-style icon names in comments or simple inline symbols, but do
  not add a production dependency;
- create multiple static states in the same mockup with a simple segmented
  control or static sections;
- capture screenshots into
  `docs/phase3c_ui_foundation_shell_mockup/artifacts/`;
- write a visual QA report and completion summary.

Not allowed:

- modify `frontend/src/components/ProductShell.tsx`;
- modify `frontend/src/productShellContract.ts`;
- modify `frontend/src/App.tsx`;
- modify backend Rust/Tauri code;
- add a production route for the mockup;
- add the mockup to primary navigation;
- replace `/today`, `/companion`, `/mailbox`, `/runs`, `/settings`, or
  `/life-model`;
- claim product readiness from static fixtures;
- describe `WorkspaceViewModel` as complete Frontend V2.

If you believe a production source change is necessary, stop and report the
reason instead of doing it.

## Mockup Content Requirements

Represent these static states:

1. `今日: ready with pending review`
2. `今日: stale/unknown fail-closed`
3. `工作区: active task with waiting permission`
4. `审核中心: review item approved but not materialized`
5. `LifeModel: limited current compatibility`
6. `设置: provider/privacy unknown or possible external transmission`

The shell must include:

- left sidebar primary navigation;
- local/private status area;
- top context bar;
- main work surface;
- right evidence/limitations inspector;
- product action area;
- advanced/debug area that is visually secondary;
- safe-mode or fail-closed visual state.

Use fixture labels such as `静态样例` or `后端读模型字段示意` wherever the mockup
could otherwise be read as live product state.

## Design Requirements

Follow the token values from
`docs/phase3c_ui_foundation_shell_mockup/02_ui_foundation_and_static_mockup_spec.md`.

Pay special attention to:

- 13px/20px body text;
- 12px/16px metadata;
- 18px/24px surface titles;
- 4px spacing grid;
- 260px desktop sidebar;
- 360px desktop inspector;
- 6px or 8px panel radius;
- neutral base palette plus semantic accent, warning, danger, success, and info;
- no nested cards;
- no decorative gradient/orb/bokeh background;
- no viewport-scaled fonts.

## Validation

Run at minimum:

```sh
git diff --check
```

If you create JS that can be linted or formatted by existing tooling, run the
smallest relevant check and document it. Do not invent broad gates for static
docs-only files.

If browser tooling is available, open the static mockup and capture or inspect:

- `1440x900`
- `1280x800`
- `390x844`

Record results in:

```text
docs/phase3c_ui_foundation_shell_mockup/04_visual_qa_report.md
```

## Required Self-Review

Before final handoff, verify:

- no production `ProductShell`, route contract, route wiring, or backend files
  changed;
- every success/ready-looking state is labeled as fixture/static when not live;
- unknown/stale/error states remain fail-closed;
- `已批准，未物化` is visually distinct from `已完成`;
- evidence/debug content is present but secondary;
- screenshots or manual viewport checks are documented;
- `git diff --check` passed.

## Expected Final Handoff From You

Return:

- files changed;
- where to open the static mockup;
- screenshot artifact paths, if produced;
- validation commands and results;
- self-review answers;
- known limitations;
- recommended next phase.

The next phase after a successful Phase 3C should be a reviewed decision on
whether to port the accepted shell foundation into React preview components,
not an automatic global ProductShell replacement.
