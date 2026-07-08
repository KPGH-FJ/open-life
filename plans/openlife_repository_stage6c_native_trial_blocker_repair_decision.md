# Stage6C Native Trial Blocker Repair Decision

Date: 2026-07-08 Asia/Shanghai
Workspace: `/Users/tw/Desktop/open-life`
Stage: Stage6C native trial blocker repair
Result: P0 repair implemented; overall Phase7 trial remains RED / red-fail-closed

## Boundary

Stage6C repairs the P0 product-path blockers captured by the Stage6B native
Computer Use trial. It does not promote authority, does not close Phase7, and
does not count external live-provider credit.

The Stage6B native evidence remains the native reproduction baseline:

- `frontend/test-results/phase7-computer-use-trial/stage6b-20260708T095147+0800/stage6b-trial-report.md`
- `frontend/test-results/phase7-computer-use-trial/stage6b-20260708T095147+0800/terminal-logs/task-sessions-after-weather-stuck.log`
- `frontend/test-results/phase7-computer-use-trial/stage6b-20260708T095147+0800/terminal-logs/action-queue-after-weather-stuck.log`
- `frontend/test-results/phase7-computer-use-trial/stage6b-20260708T095147+0800/runtime/openlife-data-proposal/main_chat_agent_sessions.db`
- `frontend/test-results/phase7-computer-use-trial/stage6b-20260708T095147+0800/runtime/openlife-data-proposal/proposals.db`

No fresh native Computer Use rerun is recorded in this decision. The repaired
behavior is verified by source-level regression tests and remains subject to a
later native rerun before the product trial can turn green.

## Source Map

### Weather / External Fact Became Local LifeEvent

Stage6B prompt:

`请告诉我今天旧金山的天气。必须使用可审计的 web/weather 读取证据；如果当前没有可用外部读取工具，请明确 fail closed，不要猜。`

Native DB evidence showed:

- `agent_task_sessions.status=completed`
- `final_summary=DirectAnswer completed without tool execution.`
- `action_queue.action_type=life_event.create`
- local `life_event` was created for an external weather request

Root cause:

- `openlife-core/src/agent/main_chat_governance_intent.rs` did not classify the
  Stage6B Chinese request shape `请告诉...天气` as a current external fact read.
- `openlife-core/src/agent/main_chat_memory_candidate.rs` did not exclude the
  same request shape from local LifeEvent capture.
- `src-tauri/src/main_chat_kernel.rs` planned memory governance before read-tool
  routing, so a missed external-read classification could enqueue
  `life_event.create` and complete the task without governed read evidence.

Repair:

- Expanded external-read request terms to include `tell me`, `tell us`,
  `告诉`, `请告诉`, `说一下`, and `说说`.
- External-read classification now suppresses Main Chat memory governance before
  memory materialization.
- The Stage6B weather prompt now routes to governed `web.search` and stays
  blocked / pending permission when no governed read evidence is available.
- The repaired path does not persist local LifeEvents or local Memory /
  LifeModel proposals for the external fact request.

Regression coverage:

- `cargo test -p openlife-core stage6c_native_weather_prompt -- --nocapture`
- `cargo test -p openlife-tauri main_chat_kernel_stage6c_native_weather_prompt_fails_closed_without_life_event -- --nocapture`

### Accepted Memory Proposal Left Main Chat Task Blocked

Stage6B native DB evidence showed:

- `proposals.status=accepted`
- `memory.db` had the accepted `proposal_memory` row
- `agent_task_sessions.pending_blockers_json` still contained
  `proposal:<proposal_id>`
- Companion / Runs / Mailbox / Today disagreed about whether the task was still
  waiting

Root cause:

- `src-tauri/src/commands/proposal.rs::accept_proposal_with_state` applied the
  Review proposal and marked it accepted, but did not synchronize the
  corresponding `AgentTaskSessionStore` blocker.
- The proposal source detail contained the needed task pointer:
  `main_chat_agent_task_session:<task_id>;candidate:<candidate_id>`.

Repair:

- Accepting Review proposals that resolve a Main Chat task approval blocker now
  extracts the originating task id from `after.originatingTaskSessionId`,
  `after.originating_task_session_id`, or `source_detail`.
- It removes only the matching `proposal:<proposal_id>` blocker from the linked
  task session.
- If the task has no remaining blockers and no pending permission action, a
  previously `waiting_permission` task is resumed and completed with an audit
  transcript observation.
- The accept response includes `mainChatTaskSync` metadata when synchronization
  occurred.

Regression coverage:

- `cargo test -p openlife-tauri main_chat_kernel_stage6c_accepting_memory_proposal_clears_task_blocker -- --nocapture`

## Non-P0 Findings Kept Red

These findings were source-mapped but intentionally not fixed in Stage6C:

- Edited proposal accept/reject: `frontend/src/pages/MailboxPage.tsx` disables
  accept unless `proposal.status === "pending"`, while
  `src-tauri/src/commands/proposal.rs` still allows `Edited` proposals through
  the early reject guard before hitting lifecycle conflict behavior. This
  remains red.
- LifeModel quick build step 7: `openlife-core/src/builder/engine.rs` increments
  `step_index` when it emits the step prompt; Stage6B DB showed `step_index=7`,
  `finished=false`, and draft YAML only through step 6, so the native submit path
  did not persist the step 7 answer. This remains red.
- ToolPermission native manual seed path: `frontend/scripts/step6-tauri-webdriver.mjs`
  still defines the Step6 ToolPermission journey as `seeded_control` with
  `prepTaskId=S6_PERMISSION_ACCEPT`, and the manual native trial did not expose
  a visible seed entry. This remains red.

## Verification Status

Passed checks:

- `cargo fmt --check`
- `git diff --check`
- `cargo test -p openlife-core stage6c_native_weather_prompt -- --nocapture`
- `cargo test -p openlife-tauri main_chat_kernel_stage6c_native_weather_prompt_fails_closed_without_life_event -- --nocapture`
- `cargo test -p openlife-tauri main_chat_kernel_stage6c_accepting_memory_proposal_clears_task_blocker -- --nocapture`
- `cargo test -p openlife-tauri single_system -- --nocapture`
- `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture`
- `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture`
- `corepack pnpm --dir frontend typecheck`
- `corepack pnpm --dir frontend format:check`
- `corepack pnpm --dir frontend test -- App.test.tsx ChatPage.test.tsx tauri.test.ts`
- forbidden completion-claim scan over the requested active docs and canonical
  trial report

Fail-closed check:

- `corepack pnpm --dir frontend test:e2e:tauri:step6:local` exited 1 before
  running on macOS with blockers
  `step6_product_acceptance_e2e_blocked`,
  `real_tauri_browser_command_surface_unavailable`,
  `tauri_webdriver_environment_not_ready`, and
  `tauri_webdriver_macos_not_supported_by_tauri_driver`.

## Decision

Stage6C fixes the two prioritized P0 source paths and adds regression tests.
The canonical product trial report must remain RED until a later native rerun
proves the repaired flows end to end and the remaining non-P0 blockers are
either fixed or explicitly accepted as red-fail-closed.
