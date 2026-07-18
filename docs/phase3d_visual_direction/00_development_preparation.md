# Phase 3D Visual Direction Development Preparation

Status: `HUMAN_APPROVED_VISUAL_BASELINE`.
Date: 2026-07-18.
Scope: real visual-direction study before React implementation.

## Authority And Inputs

This package is subordinate to:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`

Design inputs:

- `docs/phase1_ux_ia/02_product_positioning.md`
- `docs/phase1_ux_ia/03_v2_information_architecture.md`
- `docs/phase1_ux_ia/04_agent_workspace_model.md`
- `docs/phase1_ux_ia/05_review_center_model.md`
- `docs/phase1_ux_ia/06_lifemodel_memory_model.md`
- `docs/phase1_ux_ia/07_chinese_product_language_v1.md`
- `docs/phase1_ux_ia/08_diagnostics_visibility_policy.md`
- `docs/phase1_ux_ia/11_ui_foundation_study.md`
- `docs/phase3c_ui_foundation_shell_mockup/`

Historical visual ideas were used only as reference, especially the earlier
goals of a warm, quiet, orderly personal operating system. They do not
override the current Phase7 and ViewModel boundaries.

## Objective

Create a controlled visual direction and a complete blueprint candidate before
production React work begins. The outcome must be inspectable as real screens,
not a moodboard, prose-only design, or generated bitmap with embedded text.

The target product feel is:

- trustworthy;
- quiet;
- mature;
- visually familiar to current Codex/Cursor light workbenches rather than
  dependent on a custom product-wide palette;
- Chinese-first;
- local-first without claiming local execution when the privacy boundary is
  unknown;
- focused like an agent workbench, but less technical than an IDE.

## Preserved Product Rules

- Unknown, stale, missing evidence, and incomplete permission scope fail closed.
- Provider/privacy certainty comes only from
  `ProviderPrivacyBoundarySummary`.
- Approval is not application, materialization, or completion.
- Product, review, and debug actions stay separate at the contract layer.
- The frontend does not reconstruct product truth from raw diagnostics,
  proposals, or page-local guesses.
- The visual design may show target projections as fixtures, but must label them
  outside the product shell and must not claim backend readiness.

## Non-Goals

- No `ProductShell` replacement.
- No production route or navigation change.
- No React implementation.
- No Rust or Tauri change.
- No claim that proposed Review decision fields already exist.
- No claim that Workspace or Tasks ViewModels are complete Frontend V2
  contracts.

## Deliverables

Phase 3D:

- three controlled visual directions using the same content and hierarchy;
- an explicit selection rationale;
- one editable Figma visual reference, with the account-quota limitation
  recorded explicitly in the Phase 3E QA report.

Phase 3E candidate:

- design tokens and component language;
- desktop blueprints for Today, Workspace, Tasks, Review Center, LifeModel, and
  Settings;
- mobile blueprints for the critical daily, permission, review, and evidence
  flows;
- a standalone responsive HTML/CSS/JS prototype;
- component/state, interaction, accessibility, and field-source matrices;
- browser screenshot and interaction QA.

## React Entry Rule

Human review approved the visual direction on 2026-07-18. This approval freezes
the design baseline, but does not authorize React implementation before the
mainline and contract gates recorded by Phase 3F:

```text
VISUAL_DIRECTION_SELECTED = YES
KEY_SCREEN_BLUEPRINTS_COMPLETE = YES
DESIGN_TOKENS_FROZEN = YES
MOBILE_BLUEPRINTS_COMPLETE = YES
CRITICAL_STATE_DESIGNS_COMPLETE = YES
REACT_PORT_READY = NO
```
