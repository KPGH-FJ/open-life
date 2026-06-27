# Goal 7: Web, MCP, And Provider Capability Restoration

> Status: prepared for goal mode
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## Objective

Restore web read, registered MCP read, MCP ToolPermission proposal, and
provider-backed selection/generation on top of the stable MainChatKernel,
without making these advanced capabilities prerequisites for basic chat,
read-only tools, or proposal-only writes.

## System Position

This goal restores high-leverage external capability after the local kernel,
read-only tools, proposal boundaries, UX, and HS context are stable. It is
explicitly not the foundation of basic Main Chat.

## OpenLife Lessons Applied

- Provider-ranked MCP and live-provider proof became too central too early.
- Generic MCP must stay bounded, manifest-backed, and permission-aware.
- External network/provider behavior should enhance a stable agent, not mask
  local execution gaps.

## Industry Practices Applied

- MCP exposes data and arbitrary tool execution paths; consent, access control,
  and strict identity are mandatory.
- MCP security is currently implementation-discipline dependent, so OpenLife
  must enforce allowlists, source identity, and bounded manifests itself.
- Provider-backed ranking should have deterministic fallback and should not be
  required for correctness.

## Scope

Allowed implementation scope:

- integrate governed web read/fetch/search where already supported;
- integrate registered MCP read-only manifests;
- restore MCP permission proposal and replay against kernel actions;
- restore provider-backed selection only after deterministic selection works;
- add opt-in live-provider evidence tests without weakening local deterministic
  tests.

Out of scope:

- making external live provider required in normal CI;
- enabling write-like MCP tools by default;
- broad plugin marketplace execution;
- remote autonomous tool execution without manifest and permission evidence.

## Restoration Order

1. Web read with network-policy blocker.
2. Registered MCP read-only tool.
3. MCP ToolPermission proposal.
4. Permission acceptance replay.
5. Multi-candidate deterministic selection.
6. Provider-ranked preselection.
7. External live-provider harness.

These are hard substages, not a loose checklist. Do not start a later substage
until the earlier substages have passing local evidence or the completion
report records a user-approved deferral.

## Substage Acceptance

| Substage | Required evidence before moving on |
| --- | --- |
| 7A Web read/blocker | Governed read success or explicit network-policy blocker on send and stream. |
| 7B Registered MCP read | Exact manifest/source identity and bounded arguments in runtime evidence. |
| 7C MCP permission proposal | Proposal links to exact pending action identity. |
| 7D Permission replay | Acceptance replays original pending action, not a reinterpreted request. |
| 7E Deterministic selection | Multi-candidate bounded deterministic selection passes without provider ranking. |
| 7F Provider-ranked preselection | Provider ranking is metadata-safe and has deterministic fallback. |
| 7G External live harness | External provider proof is explicit opt-in and never normal local readiness. |

## Runtime Contracts

- Web contract: network policy, source URL, bounded content preview, citation or
  blocker.
- MCP contract: exact manifest identity, source, action type, risk level,
  bounded arguments, and permission decision.
- Ranking contract: deterministic candidate order first; provider-ranked order
  only as accepted metadata-safe enhancement.
- Live-provider contract: opt-in evidence only, never required for local kernel
  pass.

## Acceptance Checklist

- [ ] Web read success or blocker is explicit.
- [ ] MCP read success uses registered manifest evidence.
- [ ] MCP permission proposal links to exact pending action.
- [ ] Accepted permission can replay the original action.
- [ ] Multi-candidate selection is bounded and deterministic before provider ranking.
- [ ] External live-provider proof remains opt-in.

## Verification

```bash
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_kernel -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri main_chat_live_provider -- --nocapture
```

External live-provider ignored tests require explicit operator opt-in and are
not normal completion criteria unless the goal explicitly enters that subtask.

## Stop Conditions

- Basic kernel behavior starts depending on live-provider credentials.
- MCP write-like tools appear executable without permission/proposal boundary.
- Provider-ranked selection replaces deterministic fallback.
- Tool identity depends on ambiguous name matching instead of strict manifest
  identity.
