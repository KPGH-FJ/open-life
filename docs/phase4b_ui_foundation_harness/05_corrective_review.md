# Phase 4B Corrective Review

Status: `CORRECTIVE_PASS_PENDING_HUMAN_REVIEW`
Date: 2026-07-19

## Findings And Root Causes

| Finding                                            | Root cause                                                                                          | Corrective contract                                                                          |
| -------------------------------------------------- | --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `/phase4b/` showed the old product UI              | Vite SPA fallback served `frontend/index.html` for an unknown path                                  | Phase 4B uses MPA mode and rejects every non-harness HTML entry with 404                     |
| Disabled-field contract was overstated             | `FoundationTextField` forwarded `disabledReason` to the DOM instead of enforcing it                 | All disabled interactive primitives require and link a visible reason                        |
| Feedback could be announced twice                  | Visible feedback and a hidden region both owned `aria-live`                                         | `FoundationLiveRegion` is the sole dynamic announcement owner                                |
| Control outlines missed 3:1                        | Divider tokens were reused as interactive boundaries                                                | `--ol-control-boundary` remains above 3:1 on default and hover backgrounds                   |
| Tauri rejection depended on cwd                    | The build hook referenced a relative script path                                                    | The overlay uses an inline hard failure verified from two working directories                |
| QA missed lower content and several keyboard paths | Screenshots captured only the initial internal-scroll position and most actions used pointer clicks | QA now captures top/bottom, exercises keyboard actions, and measures focus/non-text contrast |

## Verification Boundary

The canonical review URL is
`http://127.0.0.1:4184/dev/phase4b/`. The root, `/index.html`, and `/phase4b/`
must return 404. The surface remains a component lab: it is not Shell V2, not a
production route, and not evidence that any product journey is connected to the
backend.

No change in this corrective pass modifies `ProductShell.tsx`,
`productShellContract.ts`, product route authority, or Rust/Tauri business
handlers. Phase 4C remains blocked on human review and merge of Phase 4B.
