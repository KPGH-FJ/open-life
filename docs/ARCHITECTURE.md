# OpenLife Architecture

## Product Path

```text
React Workbench
  -> frontend/src/tauri.ts
  -> Tauri commands in src-tauri/src/lib.rs
  -> Rust runtime and read/write gateways
  -> openlife-core
  -> SQLite, local files, Keychain, models, and governed external tools
```

Main Chat send and stream have separate transport entrypoints and converge on
`OpenLifeTurnRuntime`:

```text
main_chat_send.rs | main_chat_streaming.rs
  -> main_chat_turn_runtime.rs
  -> main_chat_kernel.rs
  -> openlife-core/src/agent/main_chat_agent_v1.rs
```

Product read state is exposed through `LifeStateProjection` and backend
ViewModels. Governed writes pass through proposal, permission, and persistence
owners rather than page-local state.

## Source Maps

- [Agent runtime](architecture/agent-runtime.md)
- [Life Model](architecture/life-model.md)
- [Governance](architecture/governance.md)
- [Memory](architecture/memory.md)
- [Testing](development/testing.md)
- [Decisions](decisions/README.md)

These documents explain source. They do not override runtime code or accepted
ADRs.
