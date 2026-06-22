# Goal 8: Cleanup And Final Gate Realignment

> Status: prepared for goal mode
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## Objective

Reduce legacy Main Chat runtime duplication and realign final/readiness gates so
they validate the new MainChatKernel path instead of preserving the old
over-orchestrated strategy path as the product default.

## System Position

This goal is cleanup and realignment, not initial rescue. It happens only after
the kernel has proved direct answer, send/stream parity, read-only tools,
proposal-only writes, execution UX, HS context, and external capability where
applicable.

## OpenLife Lessons Applied

- Old readiness/productization modules became too influential over runtime
  shape.
- Legacy paths should remain only when they preserve real behavior not yet
  covered by the kernel.
- Final gates must validate product reality, not preserve historical
  complexity.

## Industry Practices Applied

- Evals should follow traces and stable behavior.
- Rich diagnostics are valuable, but ordinary developers should learn the high
  level result surfaces first.
- Safety checks must remain explicit even when legacy complexity is removed.

## Scope

Allowed implementation scope:

- isolate or retire legacy `main_chat_strategy.rs` paths no longer used by
  default Main Chat;
- remove duplicated send/stream logic left behind by the migration;
- update final/readiness gate aggregation to credit kernel evidence;
- keep historical gates available where useful but not product-authoritative;
- update documentation to reflect the new default runtime.

Out of scope:

- deleting useful audit/test history without replacement evidence;
- weakening safety gates;
- claiming completion without live-provider evidence where live-provider
  evidence is explicitly required;
- broad unrelated refactors.

## Required Outcomes

- default Main Chat uses MainChatKernel;
- legacy fallback is explicit and measurable;
- final gates validate kernel direct answer, read-only tools, proposal-only
  writes, HS context, web/MCP/provider evidence as applicable;
- docs no longer point new development at obsolete stage plans first.

## Runtime Contracts

- Migration contract: legacy fallback is explicit, counted, and not the default
  success path.
- Gate contract: final/readiness reports consume kernel evidence fields.
- Documentation contract: `plans/README.md`, AGENTS guidance, and runtime module
  boundaries agree on the default Main Chat path.
- Safety contract: no-silent-write, permission, blocker, and proposal behavior
  remains covered after cleanup.

## Acceptance Checklist

- [ ] Default Main Chat path is kernel-backed.
- [ ] Legacy strategy path is isolated or explicitly marked legacy.
- [ ] Duplicate send/stream code is reduced.
- [ ] Final/readiness gates consume kernel evidence.
- [ ] Documentation authority map is updated.
- [ ] No safety regression in no-silent-write, permission, or blocker behavior.

## Verification

```bash
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-core main_chat_agent_v1 -- --nocapture
cargo test -p openlife-tauri main_chat_kernel -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
npm --prefix frontend test -- --run
```

## Stop Conditions

- Gate realignment requires weakening safety checks.
- Legacy cleanup risks deleting evidence for behavior not yet covered by the
  kernel tests.
- Documentation and runtime disagree about the default Main Chat path.
- A broad cleanup would be easier than proving the new kernel behavior; in that
  case, stop and add missing kernel evidence first.
