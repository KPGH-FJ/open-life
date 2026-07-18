# Phase 3E Product Blueprint Scope And Authority

Status: `FULL_VISUAL_BLUEPRINT_REVIEW_CANDIDATE`.
Date: 2026-07-18.

## Purpose

This package expands the recommended Phase 3D `Codex / Cursor White Workbench`
direction into a complete, responsive product blueprint before React
implementation.

It is a visual and interaction authority candidate. It does not replace the
production shell, routes, ViewModels, or backend contracts.

## Product Surface Ownership

Primary navigation:

- 今日: daily attention and next action.
- 工作区: current collaborative execution.
- 任务: active/history continuity and recovery.
- 审核中心: consequential decisions.
- LifeModel: current long-term understanding, memory/provenance, pending
  suggestions.

Utility navigation:

- 设置: provider, privacy, tools, data, and product setup.
- 支持信息: evidence/debug access, not a primary route.

Companion is expressed as a Workspace interaction mode, not another primary
desktop destination. Memory is represented as a LifeModel sub-surface until a
distinct top-level Memory read model is approved.

## Blueprint Screens

Desktop:

1. Today ready with one pending review.
2. Today stale/unknown fail-closed.
3. Workspace waiting for file permission.
4. Tasks active/history split view.
5. Review pending decision.
6. Review approved, not applied.
7. LifeModel limited current compatibility.
8. Settings provider/privacy unknown.

Mobile critical paths:

1. Today attention view.
2. Workspace permission interruption.
3. Review pending decision with fixed actions.
4. Evidence bottom sheet.
5. Navigation drawer for utility destinations.

## Fixture Boundary

The fixture selector lives outside the product shell. All examples are static
visual fixtures. They do not prove backend readiness, command availability,
materialization, provider routing, external transmission, or product routes.

## React Port Gate

React implementation remains blocked until:

- the user approves the selected visual direction;
- all key review-board and prototype frames are reviewed;
- tokens and responsive rules are frozen;
- the Review decision/permission projection gap is resolved or the React slice
  explicitly excludes those actions;
- action labels and lifecycle transitions match executable contracts;
- the product-shell migration and old-shell deletion plan is approved.
