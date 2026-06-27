# Main Chat Agent Kernel Rescue Acceptance Matrix

> Date: 2026-06-22
> Status: full eight-goal acceptance matrix for the Main Chat kernel rescue
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## 1. Acceptance Principle

The rescue is accepted only when ordinary Main Chat behaves like a small,
reliable, observable agent first, then regains OpenLife product depth through
proposal-reviewed HS, web/MCP/provider integrations, and final gate realignment.

Existing final gates can remain fail-closed during early goals; they are not the
success definition until Goal 8 realigns them to kernel evidence.

Each accepted scenario must prove:

- one shared kernel path for the behavior under test;
- send and stream parity where both surfaces apply;
- user-visible answer, observation, proposal, permission interruption, or
  blocker;
- no legacy fallback success claim;
- no silent durable write;
- runtime evidence for every user-facing claim.

## 2. K0: Baseline And Preparation

| ID | Scenario | Surface | Required evidence |
| --- | --- | --- | --- |
| K0-01 | Repository starts from known branch | Git | `git status --short --branch` recorded. |
| K0-02 | Core and Tauri compile targets are explicit | CLI | Goal specs name both `cargo check -p openlife-core` and `cargo check -p openlife-tauri` where `src-tauri` can change. |
| K0-03 | Kernel rescue docs exist | Plans | Preparation, industry practices, spec-coding contract, index, matrix, and eight goal specs are present. |
| K0-04 | Completion reporting is standardized | Plans | Goal completion template exists and is linked from the index/spec contract. |
| K0-05 | Source-practice freshness is checked | Plans | Industry practice digest names current source links and notes MCP spec freshness. |

## 3. K1: Goal 1 Kernel Foundation

| ID | Scenario | Surface | Expected result |
| --- | --- | --- | --- |
| K1-01 | Kernel module exists | Tauri internal | `MainChatKernel` module compiles without default command migration. |
| K1-02 | Direct answer, isolated kernel | shared kernel | One assistant response, no tools, no durable writes. |
| K1-03 | Empty or invalid user turn | shared kernel | Named validation blocker, no model call if input is invalid. |
| K1-04 | Provider/model route trace | shared kernel | Bounded route metadata is recorded without requiring live eval. |
| K1-05 | Selected skill context | shared kernel | Sanitized selected skill id can influence context, not policy. |
| K1-06 | No final/live dependency | shared kernel | Kernel success does not require final acceptance or live-provider machinery. |

Exit criteria:

- K1-01 through K1-06 pass;
- `direct_writes_executed=false` is asserted on every successful case;
- `legacy_fallback_used=false` is asserted on kernel success;
- Goal 1 completion report is written.

## 4. K2: Goal 2 Send/Stream Convergence

| ID | Scenario | Surface | Expected result |
| --- | --- | --- | --- |
| K2-01 | Direct answer, non-stream | `send_message` adapter | Uses kernel through buffered event sink. |
| K2-02 | Direct answer, stream | `start_stream_message` adapter | Uses kernel through streaming event sink. |
| K2-03 | Send/stream parity | send + stream | Same final semantics, route metadata, blocker semantics, and no-write flags. |
| K2-04 | Invalid input parity | send + stream | Same named blocker on both surfaces. |
| K2-05 | Legacy fallback visibility | send + stream | Any legacy route is explicit and counted, not hidden success. |
| K2-06 | No duplicated strategy rebuild | code review | Adapter code does not recreate intent/layer/strategy execution separately. |

Exit criteria:

- K2-01 through K2-06 pass;
- direct-answer command-surface tests pass;
- Goal 2 completion report is written.

## 5. K3: Goal 3 Read-Only Tools

| ID | Scenario | Surface | Expected result |
| --- | --- | --- | --- |
| K3-01 | Workspace file read | send + stream | Reads an allowed workspace file, records observation, synthesizes answer. |
| K3-02 | Path traversal attempt | send + stream | Explicit filesystem blocker, no read outside allowed root. |
| K3-03 | Session search | send + stream | Retrieves bounded prior session context and cites it in answer. |
| K3-04 | Memory search | send + stream | Retrieves bounded memory context, no memory mutation. |
| K3-05 | Web read unavailable | send + stream | Explicit network-policy blocker if governed web read is unavailable. |
| K3-06 | Unknown tool target | send + stream | Explicit unsupported-tool blocker, no fallback fake success. |
| K3-07 | Governed input enforcement | kernel/tool | Model-supplied arguments cannot bypass governed executor input. |

Exit criteria:

- K3-01 through K3-07 pass on both command surfaces where applicable;
- observations are stored or emitted in a user-inspectable form;
- Goal 3 completion report is written.

## 6. K4: Goal 4 Proposal-Only Writes

| ID | Scenario | Surface | Expected result |
| --- | --- | --- | --- |
| K4-01 | "Remember this" | send + stream | Creates Memory proposal, does not write accepted memory. |
| K4-02 | LifeModel update request | send + stream | Creates LifeModel proposal, does not materialize truth. |
| K4-03 | File write request | send + stream | Creates proposal or permission request; no file write by default. |
| K4-04 | External side effect | send + stream | Confirmation/proposal blocker unless scoped permission exists. |
| K4-05 | Dangerous shell request | send + stream | Hard blocker, no execution, no proposal replay. |
| K4-06 | Auto-checkin isolation | kernel | Ordinary chat auto-checkin does not silently materialize accepted truth. |
| K4-07 | Review Center inspectability | UI + runtime | Proposal source, payload summary, and review status are inspectable. |

Exit criteria:

- K4-01 through K4-07 pass;
- durable Memory and LifeModel updates are proposal-only;
- Goal 4 completion report is written.

## 7. K5: Goal 5 Execution UX

| ID | Scenario | Surface | Expected result |
| --- | --- | --- | --- |
| K5-01 | Direct answer state | UI + runtime | Direct answer appears without readiness/final-gate debug clutter. |
| K5-02 | Tool running state | UI + runtime | User sees the current governed tool action. |
| K5-03 | Tool observation state | UI + runtime | User sees bounded observation or failure reason. |
| K5-04 | Proposal-created state | UI + runtime | User can navigate to inspectable proposal. |
| K5-05 | Blocked/permission-needed state | UI + runtime | User sees reason and next action. |
| K5-06 | Cancel during run | stream + runtime | Kernel stops cleanly and records canceled state. |
| K5-07 | No fake UI state | UI review | UI does not claim execution without kernel/proposal evidence. |

Exit criteria:

- K5-01 through K5-07 pass;
- default Chat prioritizes kernel evidence over readiness noise;
- Goal 5 completion report is written.

## 8. K6: Goal 6 HS Reintegration

| ID | Scenario | Surface | Expected result |
| --- | --- | --- | --- |
| K6-01 | Bounded HS summary context | kernel | HS summary appears with source/provenance, freshness, and privacy metadata. |
| K6-02 | Accepted guidance context | kernel | Accepted guidance can influence answer/planning without overriding policy. |
| K6-03 | HS proposal learning | kernel + proposal | New Memory/LifeModel learning creates proposals only. |
| K6-04 | HS policy blocker/proposal | kernel | HS policy can produce proposal or blocker for write-like/risky request. |
| K6-05 | Missing HS degradation | kernel | Missing/malformed HS context produces warning metadata, not basic answer failure. |
| K6-06 | No raw prompt dump | code review | Raw LifeModel/unbounded memory is not injected into the kernel prompt. |

Exit criteria:

- K6-01 through K6-06 pass;
- ordinary chat still has no silent accepted-truth materialization;
- Goal 6 completion report is written.

## 9. K7: Goal 7 Web, MCP, And Provider Restoration

| ID | Scenario | Surface | Expected result |
| --- | --- | --- | --- |
| K7-01 | Web read or blocker | send + stream | Governed web read succeeds with source evidence or returns network-policy blocker. |
| K7-02 | Registered MCP read | send + stream | MCP read success uses exact registered manifest identity and bounded arguments. |
| K7-03 | MCP permission proposal | send + stream | Permission proposal links to exact pending action. |
| K7-04 | Permission replay | runtime | Accepted permission replays the original action, not a reinterpreted request. |
| K7-05 | Multi-candidate deterministic selection | runtime | Bounded deterministic candidate order works before provider ranking. |
| K7-06 | Provider-ranked preselection | runtime | Provider ranking is metadata-safe, opt-in where required, and has deterministic fallback. |
| K7-07 | External live-provider proof | harness | External live proof remains explicit opt-in and is not normal local completion. |
| K7-08 | MCP strict identity | code review | Tool identity uses strict manifest/source identity, not ambiguous name matching. |

Exit criteria:

- K7-01 through K7-05 pass before K7-06 starts;
- K7-06 can be skipped only if deterministic selection remains complete and the
  completion report records provider-ranking as pending;
- K7-07 remains opt-in and cannot block local kernel readiness;
- Goal 7 completion report is written.

## 10. K8: Goal 8 Cleanup And Final Gate Realignment

| ID | Scenario | Surface | Expected result |
| --- | --- | --- | --- |
| K8-01 | Default Main Chat path | command surface | Default Main Chat is kernel-backed. |
| K8-02 | Legacy fallback isolation | runtime + report | Legacy fallback is explicit, counted, and not default success. |
| K8-03 | Duplicate send/stream reduction | code review | Remaining duplication is transport-only or explicitly justified. |
| K8-04 | Final/readiness gate evidence | eval/report | Gates consume kernel evidence fields. |
| K8-05 | Documentation agreement | docs | `plans/README.md`, AGENTS guidance, and runtime modules agree on default path. |
| K8-06 | Safety regression check | tests/report | No regression in no-silent-write, permission, proposal, or blocker behavior. |
| K8-07 | Historical evidence preservation | code review | Useful legacy audit/test evidence is preserved or replaced before cleanup. |

Exit criteria:

- K8-01 through K8-07 pass;
- final/readiness gates validate product reality, not obsolete paths;
- Goal 8 completion report is written.

## 11. Minimum Verification Commands

Every runtime-changing goal should start with:

```bash
cargo check -p openlife-core
cargo check -p openlife-tauri
```

Then add the focused goal commands, usually:

```bash
cargo test -p openlife-tauri main_chat_kernel -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
```

Frontend-changing goals should also run the repository's active frontend test
command, currently documented as:

```bash
npm --prefix frontend test -- --run
```

Do not add broad final acceptance requirements to early kernel goals. Final
acceptance is realigned in Goal 8 after kernel behavior is covered by focused
tests and command-surface evidence.
