# Tauri Capability Audit

> Scope: current default desktop capability surface, IPC command exposure, and Main Chat governance boundary for limited-trial readiness.

## Current Default Capability Surface

`src-tauri/capabilities/default.json` currently grants:

- `core:default`
- `shell:allow-open`
- `fs:allow-app-read`
- `fs:allow-app-write`
- `fs:allow-app-read-recursive`
- `fs:allow-app-write-recursive`
- `dialog:default`
- `store:default`
- `http:default`

This is an app-level permission surface. It is broader than the Main Chat agent policy surface.

## Current Governance Boundary

Main Chat runtime policy is stricter than the Tauri capability file:

- ordinary `send_message` / `start_stream_message` use governed task sessions and typed evidence;
- durable LifeModel and Memory changes are proposal-first or governed manual operations;
- file write, external write, calendar/email/provider/plugin state changes require proposal, permission, or blocker paths;
- ReAct tool execution uses metadata-safe candidate contracts, exact target/action allowlists, and policy blockers;
- external live provider credit requires explicit opt-in, external provider identity, model invocation proof, no silent writes, no legacy fallback, and scenario-specific traces.

The audit risk is therefore not "the agent currently has unrestricted write authority"; the risk is that a frontend or IPC path could use broad app-level permissions outside the governed runtime.

## Capability Findings

### P0: Step6 Real Browser Evidence Is Blocked On macOS

The Step6 Tauri WebDriver runner currently fails closed on macOS with:

- `tauri_webdriver_macos_not_supported_by_tauri_driver`
- `real_tauri_browser_command_surface_unavailable`

Until a supported environment produces a fresh Step6 report with real Tauri UI journeys, app-level capability behavior is not fully user-journey verified.

### P1: Recursive App FS Permissions Need A Narrowing Decision

The app has recursive read/write access inside the app data scope. Backend proposal and import/restore paths are governed, but the capability is still broad.

Required follow-up:

- list all frontend wrapper calls that touch filesystem-related commands;
- confirm each write path is proposal-first, governed import/restore, safe-path constrained, or blocked in Safe Mode;
- consider splitting app capabilities by window or command group if Tauri supports the desired granularity.

### P1: `http:default` Needs Explicit Product Boundary

`http:default` is available at app level. Provider/model calls and web/tool calls have runtime policy, but the app capability itself is broad.

Required follow-up:

- confirm frontend code does not perform direct remote calls outside typed Tauri commands;
- keep provider validation and live eval reports metadata-safe;
- document that cloud model traffic is opt-in and route/policy governed.

### P1: `shell:allow-open` Must Stay UI-Only

`shell:allow-open` is useful for opening local docs or external links, but it must not become an agent execution surface.

Required follow-up:

- keep dangerous shell intent hard-blocked in Main Chat;
- confirm no frontend control turns assistant text into shell/open input without explicit user action;
- document external link opening as user-driven UI behavior, not agent authority.

## Verified Gates In This Audit Pass

- `cargo fmt --check`
- `cargo test -p openlife-tauri --lib legacy_write_convergence_w97 -- --nocapture`
- `cargo test --workspace --lib`
- `corepack pnpm --dir frontend build`
- `corepack pnpm --dir frontend test`
- external live DirectAnswer harness
- external live web/MCP/proposal ReAct harness
- external live final acceptance aggregation

## Remaining Trial Blocker

The major remaining blocker is not the Rust/frontend unit gate or external live provider gate. It is real Step6 Tauri browser evidence in a supported WebDriver environment.

Until Step6 produces a fresh report with `acceptanceReady=true`, OpenLife should be limited to internal engineering dogfood rather than user small-batch trial.
