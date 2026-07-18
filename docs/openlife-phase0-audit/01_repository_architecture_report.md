# Repository Architecture Report

## Structure

Finding: OpenLife is a two-crate Rust workspace plus a React/Tauri frontend.

Evidence:

- Root `Cargo.toml` declares workspace members `src-tauri` and
  `openlife-core`.
- `src-tauri/Cargo.toml` defines package `openlife-tauri` with Tauri 2,
  SQLite, Tokio, Axum, and `openlife-core`.
- `openlife-core/Cargo.toml` defines core dependencies including SQLite,
  tokenizer/vector libraries, AES-GCM, and HTTP clients.
- `frontend/package.json` defines React 18, Vite, TypeScript, Tailwind,
  Vitest, Playwright, and Tauri API dependencies.

File location:

- `Cargo.toml`
- `src-tauri/Cargo.toml`
- `openlife-core/Cargo.toml`
- `frontend/package.json`

Confidence: High.

Impact: The repo is not frontend-only. Any frontend redesign must preserve a
large Rust/Tauri command surface and domain runtime.

## Runtime Entry Points

Finding: Tauri command registration is centralized in `src-tauri/src/lib.rs`,
but ordinary Main Chat execution is delegated to send/stream modules and then
to `OpenLifeTurnRuntime`.

Evidence:

- `send_message` calls `main_chat_send::send_message_with_state`.
- `start_stream_message` calls
  `main_chat_streaming::start_stream_message_with_state`.
- `main_chat_send.rs` uses `OpenLifeTurnRuntime::run_buffered`.
- `main_chat_streaming.rs` uses `OpenLifeTurnRuntime::run_streaming`.
- `main_chat_turn_runtime.rs` defines `OPENLIFE_TURN_RUNTIME_OWNER` and the
  canonical terminal/final-delivery structures.

File location:

- `src-tauri/src/lib.rs`
- `src-tauri/src/main_chat_send.rs`
- `src-tauri/src/main_chat_streaming.rs`
- `src-tauri/src/main_chat_turn_runtime.rs`

Confidence: High.

Impact: A rewrite must not treat `lib.rs` as the runtime owner. It is the Tauri
handler/wiring owner.

## Current Product Route Boundary

Finding: Phase7 old product routes are absent from current product source by
guarded contract, but old command names remain in dev/test/guard surfaces.

Evidence:

- `plans/openlife_single_system_deletion_manifest.md` classifies old routes as
  done, test-only archive, historical-doc-only, or product-valid rename.
- `cargo test -p openlife-tauri single_system -- --nocapture` passed 17 tests.
- Raw `rg` still finds old command names in `frontend/src/tauriDev.ts`,
  frontend mocks/tests, and Rust guard tests.
- Product pages are guarded from importing `tauriDev.ts`.

File location:

- `plans/openlife_single_system_deletion_manifest.md`
- `src-tauri/src/single_system_authority_tests.rs`
- `frontend/src/tauriDev.ts`
- `frontend/src/test/mocks/tauri.ts`

Confidence: High.

Impact: Old-route archaeology must be surface-aware. Raw search hits are not
enough to claim product route presence or absence.

## Documentation Topology

Finding: Current docs are intentionally split between active authority,
source-backed explanatory docs, and historical artifacts.

Evidence:

- `AGENTS.md` states active authority order and marks older Goal/Stage/Beta docs
  as historical unless explicitly named.
- `docs/ARCHITECTURE.md` is described by `AGENTS.md` as an index/historical
  pointer, not Main Chat authority.
- Active architecture docs exist under `docs/architecture/`.

File location:

- `AGENTS.md`
- `plans/README.md`
- `docs/ARCHITECTURE.md`
- `docs/architecture/agent-runtime.md`
- `docs/architecture/life-model.md`
- `docs/architecture/governance.md`
- `docs/architecture/memory.md`

Confidence: High.

Impact: Phase 1 design should reference active authority docs first and use
older plan files as evidence history only.
