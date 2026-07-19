# Phase 4B UI Foundation And Dev Harness Execution Baseline

Status: `TECHNICAL_EXIT_PASS_PENDING_HUMAN_REVIEW`
Date: 2026-07-19

## 1. Entry Gate

Phase 4B started only after the user reviewed and approved Phase 4A. The Phase
4A pull request was merged before this branch was created.

```text
PHASE4A_PR = https://github.com/KPGH-FJ/open-life/pull/56
PHASE4A_MERGED = YES_AT_7f9faf4eb75a086438604a158983a5c127547574
MERGED_MAIN_CI = PASS_RUN_29668642176
MERGED_MAIN_STAGE1_RETIRED_CONTRACT = PASS_RUN_29668642180
MERGED_MAIN_STEP6_RETIRED_CONTRACT = PASS_RUN_29668642172
PHASE4B_BASE_BRANCH = origin/main
PHASE4B_BASE_COMMIT = 7f9faf4eb75a086438604a158983a5c127547574
PHASE4B_WORK_BRANCH = codex/phase4b-ui-foundation-harness
PHASE4B_START_DECISION = APPROVED_2026_07_19
```

The three protected-main workflows above all completed successfully for the
exact base SHA. Backend Remediation v4 remains excluded paused backlog.

## 2. Allowed Scope

Phase 4B may:

- create the semantic CSS token authority selected in Phase 3F;
- create reusable React UI primitives and their state matrix;
- expose semantic Tailwind aliases that resolve to the CSS variables;
- create a compile-time dev-only Vite/Tauri component harness with a separate
  MPA HTML entry;
- delete the production-compiled `/today-v2-preview` route and page;
- add release-bundle, route, import, and Tauri configuration absence guards;
- add component, accessibility, interaction, and browser QA evidence;
- advance the migration/deletion ledger for the new foundation owner.

## 3. Explicit Non-Goals

Phase 4B does not:

- replace `ProductShell` or `productShellContract.ts`;
- add a production route, navigation item, fixture selector, or second shell;
- migrate Today, Workspace, Tasks, Review Center, LifeModel, or Settings;
- connect the harness to Tauri commands or product backend state;
- change Rust/Tauri business handlers, authorization, or durable-write rules;
- treat fixture approval as applied, completed, or written to durable truth;
- authorize Phase 4C without a separate human review.

## 4. Production Change Boundary

React product behavior changes only to remove the retired preview route from
`frontend/src/App.tsx`. Build configuration and test infrastructure also change
to enforce token and release-absence contracts. The normal app still renders
the existing production shell and pages. The new UI foundation is not imported
by `App.tsx`, `ProductShell.tsx`, or any production page in this phase.

The harness has its own HTML entry, Vite config, compile-time flag, Tauri dev
The Vite dev server rejects every HTML navigation except
`/dev/phase4b/`; a mistyped review URL cannot fall through to the production
`src/main.tsx` entry.

## 5. Exit Gate

Phase 4B can be submitted for review only when:

1. token, primitive, state, and accessibility contracts pass;
2. release builds prove the preview and harness absent;
3. the dev harness builds and starts through Vite and real Tauri dev mode;
4. 1440x900, 1280x800, and 390x844 browser QA passes;
5. full repository gates pass;
6. the PR remains unmerged pending human review.
