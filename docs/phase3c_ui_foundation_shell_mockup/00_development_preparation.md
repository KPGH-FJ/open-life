# Phase 3C UI Foundation + ProductShell/Shell Static Mockup Preparation

Status: development-preparation artifact, not implementation completion.
Date: 2026-07-10.
Scope: prepare the Agent implementation slice for UI foundation tokens and a
static ProductShell/workbench-shell mockup.

This package accepts the phase name as assigned:

```text
Phase 3C: UI Foundation + ProductShell/Shell Static Mockup
```

It does not rename the earlier Today and LifeModel limited slices, and it does
not authorize full Frontend V2.

## Authority Stack

Read in this order before development:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`
5. `docs/phase1_ux_ia/11_ui_foundation_study.md`
6. This Phase 3C preparation package.

If an older roadmap, goal, or shell rewrite document conflicts with this stack,
keep it as historical context only.

## Why This Phase Exists Now

The backend read-model repair has moved the product closer to backend-owned
truth for review, LifeModel, tasks/workspace baseline, memory, and provider
privacy. That makes it reasonable to design the next shell around read-model
semantics instead of page-local guesses.

The current frontend still lacks a stable visual foundation:

- `ProductShell` uses centered top tabs, not the long-term desktop workbench
  direction.
- Primary route labels remain English in the navigation while product copy is
  moving Chinese-first.
- Tailwind classes are page-local, with no extracted token layer.
- Colors such as `#f5f6f2` and `#fcfcf8` are hardcoded in components.
- Preview pages can be contract-correct but visually noisy, especially in
  fail-closed states.

Phase 3C exists to make visual quality a first-class engineering input before
more page rewrites happen.

## Development Objective

Create an isolated, realistic static shell mockup and a UI foundation contract
that can guide future React implementation.

The target is not a marketing page. The target is a dense, calm, local-first
personal AI workbench:

- left sidebar for durable product areas;
- main work surface for the active user job;
- right inspector/evidence drawer for evidence, limitations, warnings, and
  debug-only detail;
- Chinese-first product copy, while preserving `LifeModel` as a branded term;
- typography, spacing, radius, color, and state rules that agents can repeat.

## Allowed Deliverables For The Agent Slice

The recommended implementation deliverables are:

1. A standalone static mockup under:

   ```text
   docs/phase3c_ui_foundation_shell_mockup/static_mockup/
   ```

2. Mockup files:

   ```text
   docs/phase3c_ui_foundation_shell_mockup/static_mockup/index.html
   docs/phase3c_ui_foundation_shell_mockup/static_mockup/openlife-ui-foundation.css
   docs/phase3c_ui_foundation_shell_mockup/static_mockup/mockup-data.js
   ```

   `mockup-data.js` is optional, but useful if the Agent wants fixture-driven
   repeated UI states without embedding all data in HTML.

3. A short visual QA report:

   ```text
   docs/phase3c_ui_foundation_shell_mockup/04_visual_qa_report.md
   ```

4. Captured screenshots, if browser tooling is available:

   ```text
   docs/phase3c_ui_foundation_shell_mockup/artifacts/
   ```

5. A completion summary:

   ```text
   docs/phase3c_ui_foundation_shell_mockup/05_summary.md
   ```

The static mockup may use plain HTML/CSS/JS. It should not require Tauri, Rust,
or backend data.

## Explicit Non-Goals

Do not do any of these in Phase 3C:

- replace `frontend/src/components/ProductShell.tsx`;
- change `PRIMARY_PRODUCT_ROUTES`, `SECONDARY_PRODUCT_ROUTES`, or
  `ADVANCED_PRODUCT_ROUTE_GROUPS`;
- change `frontend/src/App.tsx` route wiring;
- add the mockup to primary product navigation;
- rename product routes;
- promote `Memory`/`记忆` to top-level product navigation as a shipped route;
- implement Workspace, Review Center, Tasks, Memory, or Settings V2 as product
  pages;
- change backend Rust/Tauri commands or read-model contracts;
- infer product readiness from mockup copy;
- claim Phase7, desktop trial, live-provider, external transmission, durable
  materialization, Web AgentLoop, or MCP AgentLoop readiness.

## Product Truth Boundary

The mockup may show realistic example data, but every such value must be
visibly fixture/static and must not read as proof that a backend state is ready.

Use these labels when needed:

- `静态样例`
- `后端读模型字段示意`
- `需要真实 read-model 接入`
- `未知`
- `受限`
- `等待确认`
- `需要证据`

Do not use labels like `完成`, `已就绪`, `已同步`, or `已物化` unless the mockup
section is explicitly demonstrating a verified-success visual state.

## Phase 3C Success Criteria

The phase is acceptable when:

- the static mockup demonstrates a left-sidebar workbench shell at realistic
  desktop ratios;
- typography uses real fixed sizes, not image-generation approximations;
- spacing, radius, border, and color are tokenized in CSS variables;
- default, active, loading, empty, stale, warning, danger, safe-mode, disabled,
  and advanced states are all represented;
- everyday product state is visually separated from evidence/debug surfaces;
- fail-closed states are visible but do not visually dominate the whole shell;
- no production route, ProductShell, backend command, or read-model contract is
  changed;
- visual QA screenshots or an equivalent browser inspection report exist;
- `git diff --check` passes.

## Handoff Output Required From The Agent

When the Agent finishes, it must report:

- exact files changed;
- screenshots captured, with paths;
- browser/viewports checked;
- whether any production source file changed;
- whether any route or ProductShell contract changed;
- validation commands run and results;
- known limitations and next recommended phase.
