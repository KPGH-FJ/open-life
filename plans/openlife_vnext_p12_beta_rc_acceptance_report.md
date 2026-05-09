# OpenLife vNext P12 Beta RC Acceptance Report

Date: 2026-05-09

Tester: AI Agent (P12 RC fix package)

Build / Commit: dacac16 fix: refresh model router providers before chat routing

Platform: Darwin 22.6.0 arm64 (macOS Apple Silicon)

P12 RC Fix Package: Applied 2026-05-09 addressing P1/P2 review findings. No ChatPage rewrite, no shell enablement, no runtime authority expansion.

Release Artifact:
- `target/aarch64-apple-darwin/release/bundle/macos/OpenLife.app` (25MB DMG)
- `target/aarch64-apple-darwin/release/bundle/dmg/OpenLife_0.1.0_aarch64.dmg`

Decision:

```text
conditional-go
```

## Purpose

This report records whether a P12 Beta Release Candidate can be handed to a
small group of real testers. It should be filled after P12-2 release build drill
and P12-4 acceptance run.

## Build Verification

| Check | Command / Evidence | Result | Notes |
|---|---|---|---|
| CI gate | `make ci` | pass | 735 Rust + 60 Tauri + 214 frontend tests; typecheck + format-check + lockfile-check + frontend build all passed (post P12 RC fix package) |
| Frontend build | `pnpm --dir frontend build` | pass | Vite production build: 1653 modules, ~200KB gzipped main bundle |
| Release build | `cargo tauri build --target aarch64-apple-darwin` | pass | Native aarch64 build succeeded (1m 28s after initial full compile of 4m 46s) |
| Artifact path | `target/aarch64-apple-darwin/release/bundle/` | pass | OpenLife.app + OpenLife_0.1.0_aarch64.dmg (25MB) |
| Signing / notarization | N/A | blocked | No signing certificates configured. macOS requires manual "Open Anyway" in Security & Privacy. This is expected for RC stage. |

Note: Universal binary (`--target universal-apple-darwin`) failed because x86_64-apple-darwin target is not installed. This is a platform setup issue, not a code issue. Rust cross-compilation target is installable via `rustup target add x86_64-apple-darwin`.

Bundle identifier changed from `ai.openlife.app` to `ai.openlife.desktop` to avoid macOS `.app` suffix warning; all hardcoded data directory paths synced in `storage.rs`, `a2a_server.rs`, `mcp_audit.rs`. Existing pre-RC data directories require manual copy if needed.

## Privacy Verification

| Check | Result | Notes |
|---|---|---|
| Diagnostic export excludes API keys | pass | Privacy manifest in export explicitly lists `api_keys: false`. Backend `buildSafeDiagnosticExportPayload` uses whitelist-only fields (boolean flags, numeric counts, non-sensitive provider names). |
| Diagnostic export excludes raw LifeModel | pass | Export includes only `life_model_ready` boolean, not raw content. |
| Diagnostic export excludes raw chat/messages | pass | Export includes only `chat_session_count` numeric, not messages. |
| Diagnostic export excludes raw memory/tool output | pass | Export includes only `memory_chunk_count`, not raw chunks or tool outputs. |
| Diagnostic export redacts local paths | pass | `data_dir` path is exported but local file paths in tool outputs are replaced with `[local-path]` / `[local-file-url]` markers. |
| No automatic diagnostic upload | pass | All exports are user-initiated via Settings → Data → Export Diagnostics button. No background upload exists. |

## Clean Profile Smoke

| Path | Result | Notes |
|---|---|---|
| P11-S1 First Launch and Diagnostics | blocked | Requires manual GUI launch. Code-level: SettingsPage renders, OverviewTab shows readiness items with actionable links to Provider/Builder tabs. Readiness item rendering verified by unit tests. |
| P11-S2 Provider Configuration | blocked | Requires manual GUI + valid API key. Code-level: ProviderConfigSection supports DeepSeek/OpenAI/OpenRouter/Ollama presets, test connection button wired to `testLlmConnection`. Human-readable error guidance added in P12-3. |
| P11-S3 Quick LifeModel Build | blocked | Requires manual GUI + configured model backend. Code-level: BuilderPage Quick Build card is first and prominent, details explain Proposal/Review flow. BuilderPatchReview component renders before/after diffs. |
| P11-S4 Chat to Proposal | blocked | Requires manual GUI + configured model + LifeModel. Code-level: ChatPage readiness banner, empty-state guidance, proposal banner linking to Review Center, AgentRun creation verified by integration tests. |
| P11-S5 Proposal Review and Apply | blocked | Requires manual GUI + pending proposals. Code-level: ProposalReviewPage shows accept/reject/edit/postpone actions with risk level indicators. Apply failure keeps proposal pending. High-risk fields show warning. |
| P11-S6 Run Trace Inspection | blocked | Requires manual GUI + existing runs. Code-level: RunsPage lists runs with status/kind filters. AgentRunDetail shows timeline, tool observations, proposal evidence, model route metadata. |
| P11-S7 Plan Inspection and Legal Operation | blocked | Requires manual GUI + existing plans. Code-level: Plan operations (confirm/reject/cancel/retry) tested via Rust plan_executor and Tauri plan command tests. |
| P11-S8 Backup/Export and Safe Mode Recovery | blocked | Requires manual GUI. Code-level: DataTab distinguishes Export All Data vs Export Diagnostics. Safe Mode banner restricts dangerous operations. Recovery console provides guided actions. |

## Existing Profile Smoke

| Path | Result | Notes |
|---|---|---|
| P11-S1 First Launch and Diagnostics | blocked | Requires manual GUI with existing data directory. Code-level: existing profile paths are tested via Rust integration tests for config migration, LifeModel persistence, and session continuity. |
| P11-S2 Provider Configuration | blocked | Requires manual GUI. Code-level: saved config persists after restart (tested via config round-trip tests). |
| P11-S4 Chat to Proposal | blocked | Requires manual GUI with existing LifeModel + sessions. Code-level: chat session list and resume behavior tested via chat command tests. |
| P11-S5 Proposal Review and Apply | blocked | Requires manual GUI with existing proposals. Code-level: proposal apply with existing model tested via Tauri proposal command tests. |
| P11-S6 Run Trace Inspection | blocked | Requires manual GUI with historical runs. Code-level: run listing/filtering tested via agent store tests. |
| P11-S8 Backup/Export and Safe Mode Recovery | blocked | Requires manual GUI with existing data. Code-level: export/import round-trip tested via memory/vectors tests. Safe Mode blocks tested via integration tests. |

## User Trial Guide Check

| Check | Result | Notes |
|---|---|---|
| Install / launch instructions are clear | pass | BETA_TRIAL_GUIDE.md covers dev launch (`make dev`) and release build artifact paths. Explains RC stage limitation honestly. |
| Model configuration instructions are clear | pass | Step-by-step for cloud providers (DeepSeek/OpenAI/OpenRouter) and local Ollama. Common failure reasons listed. |
| First LifeModel build instructions are clear | pass | Recommends Quick Build. Explains Proposal/Review flow upfront. Tells user where to find pending proposals. |
| First chat instructions are clear | pass | Example prompts provided. Explains what to expect (response, proposal banner, run creation). Links to Settings if blocked. |
| Proposal review instructions are clear | pass | Explains accept/reject/edit/postpone with risk level guidance. Notes apply failure behavior. |
| Diagnostic export instructions are clear | pass | Distinguishes Export All Data (backup) from Export Diagnostics (safe to share). Privacy guarantees listed. |
| Feedback template is clear | pass | "建议包含" and "请勿包含" lists in DataTab and guide. Template in P11 trial path matrix linked. |

## Known Issues

| Severity | Issue | Impact | Workaround | Owner |
|---|---|---|---|---|
| P3 | macOS universal binary requires x86_64 target | Only aarch64 native build available. Intel Mac users need separate build or Rosetta. | Install target: `rustup target add x86_64-apple-darwin` and rebuild. | Dev |
| P3 | Windows/Linux not tested | Trial limited to macOS until platform builds are validated. | Build on respective platforms with `cargo tauri build`. | Dev |
| P3 | DMG/App not code-signed | macOS Gatekeeper blocks first launch. Users must right-click → Open or go to Security & Privacy. | Instructions in trial guide. | Dev |
| P3 | P11-S7 (Plan Inspection) not independently smoke-testable without a completed plan execution | Testers may not encounter plan operations during basic trial. | Plan operations are tested via Rust integration tests. Manual testing requires running plan creation through AgentLoop. | Dev |
| P3 | Streaming error events may carry placeholder run_id before AgentLoop creates the authoritative run | Error copy still displays correctly, but run detail auto-load may fail for pre-run failures (e.g., AgentSpec resolution failure). Known limitation documented in useChatStreaming.ts. | Error details are still shown to the user. Run trace can be inspected post-hoc via RunsPage. | Dev |

No P0 or P1 issues found.

## P12 RC Fix Package (2026-05-09)

| Fix | File(s) | Summary |
|---|---|---|
| 1 | `frontend/.../ProviderTab.tsx` | Prettier formatting applied. |
| 2 | `frontend/.../ProviderTab.tsx` | Removed misleading `use_agent_loop` interactive checkbox. Replaced with read-only text: "AgentLoop/ReAct Runtime 是当前 Beta 主路径，L2/L3 对话默认启用。" Removed false fallback text. |
| 3 | `openlife-core/src/agent/agent_loop.rs` | `generate_response()` and `generate_response_streaming()` now call `scheduler.preview_chat_route()` before each model call, record `AgentRunEventType::ModelRouteSelected` event, and set `run.model_route` on the first successful model call. Both streaming and non-streaming paths covered. |
| 4 | `frontend/.../useChatStreaming.ts` | Removed premature `loadAgentRunForSession` call on `stream-message-start` (which carries a placeholder run_id). `stream-message-done` and `stream-message-error` continue to load with the real run_id. |
| 5 | `openlife-core/src/agent/agent_loop.rs` | Added doc comments to `AgentLoopConfig.allow_writes` / `allow_cloud` clarifying current hardcoded-true behavior and governance notes. Added two unit tests verifying default values and planner-mode configuration. |
| 6 | `plans/.../openlife_vnext_p12_beta_rc_acceptance_report.md` | Updated this report: fresh build/commit, fixes summary, added P3 known issue for placeholder run_id in error events.
| 7 | `tauri.conf.json` + 4 code files + 5 docs | Bundle identifier changed from `ai.openlife.app` to `ai.openlife.desktop` to avoid macOS `.app` suffix warning. All hardcoded data directory paths (`storage.rs`, `a2a_server.rs`, `mcp_audit.rs`, `diagnostics.rs`) synced. `AGENTS.md` migration note reads "一步到位": copy data from `com.openlife.app` or `ai.openlife.app` into `ai.openlife.desktop`. Existing pre-RC data directories require manual copy if needed. Removed P2 known issue. |

## Go / No-Go Criteria

Go requires:

- `make ci` passes. ✅ PASS
- A release build is attempted and the result is recorded. ✅ PASS (aarch64 native DMG and .app produced)
- No P0/P1 issue remains untriaged. ✅ PASS (no P0/P1 issues identified)
- Diagnostic export remains privacy-governed and path-redacted. ✅ PASS (whitelist-only export, redaction markers in place)
- P9 shell remains default-off. ✅ PASS (shell tests confirm default-disabled, not model-callable, blocked for scheduled/proactive/sub-agent)
- Testers can complete the guide without reading source code. ✅ PASS (BETA_TRIAL_GUIDE.md is user-facing, no source code references)

## Final Decision

Decision: conditional-go

Rationale:

- All automated gate checks pass: 733 Rust tests, 60 Tauri tests, 99 frontend tests, typecheck, format check, production build, and release build drill all succeed.
- Privacy governance for diagnostic export is intact: whitelist-only fields, no API keys/raw LifeModel/raw chat/raw memory/raw tool output in export, local paths redacted.
- P9 shell governance remains default-off across all execution paths (normal chat, scheduled, proactive, sub-agent).
- User trial guide (BETA_TRIAL_GUIDE.md) is complete and readable by non-developer testers.
- Release build produces valid macOS `.app` and `.dmg` artifacts (25MB aarch64).
- P11 manual smoke paths (S1-S8) cannot be verified from CLI-only environment — all marked as "blocked - requires manual GUI tester".
- No signing/notarization — macOS users need to allow launch manually (documented in guide).
- No P0/P1 blocking issues identified through static analysis and automated tests.

Next action:

1. Assign a human tester to run P11-S1 through P11-S8 on a clean macOS profile.
2. Assign a human tester to run P11-S1/S2/S4/S5/S6/S8 on an existing profile.
3. If any P0/P1 issue is found during manual smoke, file and triage before upgrading to "go".
4. If all manual smokes pass with no new P0/P1 issues, upgrade to "go" and begin small-scope real-user Beta trial (5-20 testers).
