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

## Domain Ownership

The current boundary is defined by ADR 0016:

- Agent Runtime owns turn and action execution;
- Agent Memory owns working, project, episodic, semantic, procedural, and
  Reflection context;
- LifeModel owns confirmed long-term understanding of the user;
- domain stores own task, transient state, calendar, email, and other business
  facts;
- safety and governance own permissions, privacy, review, and write admission.

Evidence and proposals connect these domains without becoming another fact
owner. Optional personalization failures degrade that capability, not a healthy
base Agent. Every affected read or write gateway still fails closed.

## Source Maps

- [Agent runtime](architecture/agent-runtime.md)
- [Life Model](architecture/life-model.md)
- [Governance](architecture/governance.md)
- [Memory](architecture/memory.md)
- [Testing](development/testing.md)
- [Decisions](decisions/README.md)

These documents explain source. They do not override runtime code or accepted
ADRs.
