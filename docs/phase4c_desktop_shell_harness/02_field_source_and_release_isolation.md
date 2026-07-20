# Field Source And Release Isolation

Status: `REVIEW_CANDIDATE`

## Fixture Field Sources

Every visible value is either mapped to a current contract field or explicitly
marked as layout-only. A source name in this table is not evidence that the
harness loaded backend data.

| Visible fixture value | Intended current source | Harness classification |
| --- | --- | --- |
| sidebar primary navigation | approved Phase 3F IA | pure shell contract |
| sidebar transmission boundary | `ProviderPrivacyBoundarySummary` | static unknown fixture |
| Today primary conclusion | `TodayViewModel.dailyStateSummary` | static contract-shaped fixture |
| Today pending badge/status | `TodayViewModel.pendingReviewCount` | static value `1` |
| Today review navigation | `TodayViewModel.reviewCenterLink` | local navigation fixture |
| two timed Today focus rows | no current field owns this exact schedule | `layout_fixture.today.focusList` |
| Workspace waiting state | `WorkspaceViewModel.activeTask.lifecycleStatus` | static contract-shaped fixture |
| Workspace recent steps | `WorkspaceViewModel.activity[]` | static layout rows |
| Workspace permission scope | `WorkspaceViewModel.pendingReviewItems[].decisionContext.permission` | static contract-shaped fixture |
| Workspace task evidence | `WorkspaceViewModel.activeTask.evidenceRefs` | static EvidenceRef-shaped fixture |
| Tasks unavailable conclusion | no migrated Tasks V2 caller | `layout_fixture.routeAvailability` |
| review current/suggested diff | `ReviewItem.decisionContext.before/after` | static contract-shaped fixture |
| review reason/source/impact | `ReviewItem.decisionContext` | static Inspector fixture |
| available review decisions | `ReviewItem.allowedActions[]` | local interaction fixture |
| approved decision | `ReviewItem.status` | local fixture transition only |
| applied/not-applied state | `ReviewItem.materializationStatus` | static `not_materialized` fixture |
| LifeModel summary | `LifeModelViewModel.currentViewSummary` | static contract-shaped fixture |
| LifeModel provenance | `LifeModelViewModel.provenanceRefs` | static EvidenceRef-shaped fixture |
| Settings route/transmission truth | `ProviderPrivacyBoundarySummary` | static unknown fixture |
| Settings test/save flow | `settingsOrchestrationContract` | disabled layout fixture |
| Inspector evidence rows | `EvidenceRef` shape: `id`, `label`, `source`, `sensitivity` | static structure fixture |
| QA labels, selector, feedback | no product field | QA-only outside shell |

Contract tests explicitly reject the nonexistent paths previously found during
review:

- `TodayViewModel.focusItems`;
- `TodayViewModel.reviewAttention`;
- `WorkspaceViewModel.waitingPermission.scope`;
- `LifeStateProjection.applicationStatus`.

## Release Isolation

Phase 4C has a separate multi-page Vite entry:

- canonical review URL: `http://127.0.0.1:4185/dev/phase4c/`;
- `/`, `/index.html`, and `/phase4c/` return `404`;
- compile flag is true only in `vite.phase4c.config.ts`;
- production and Phase 4B Vite configs set the Phase 4C flag false;
- `src-tauri/tauri.phase4c.conf.json` sets `bundle.active=false`;
- Phase 4B and 4C overlays use structured Tauri hooks with explicit
  `cwd: "../frontend"`, avoiding invocation-dependent directory inference;
- its package build command fails intentionally;
- normal production build scans for the harness marker, entry path, and
  stable `ol-workbench-shell` runtime/CSS marker as well as the source export;
- Rust `single_system` tests scan App, ProductShell, route contract, pages, and
  components for forbidden Phase 4C imports.

The reusable shell is therefore source-present but release-absent. It cannot
be described as the current product shell.
