# Goal 6: HS Reintegration

> Status: prepared for goal mode
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## Objective

Reintroduce LifeModel-HS into MainChatKernel as bounded read-only context,
proposal policy, and user-reviewed learning flow, without restoring silent
ordinary-chat materialization or making HS packet construction a blocker for
basic agent answers.

## System Position

This goal restores OpenLife's distinctive personal-OS layer after the kernel has
basic execution credibility. HS should make the agent more personally useful
without taking over the turn loop.

## OpenLife Lessons Applied

- HS was introduced too close to the ordinary chat path before the agent loop
  was stable.
- HS should be context and policy first, durable truth only after review.
- Maturation belongs behind proposal/review boundaries, not inside every turn.

## Industry Practices Applied

- Long-term memory belongs in a durable store separate from thread/run state.
- Agent context should be bounded and inspectable.
- Human review is the correct boundary for durable user-model changes.

## Scope

Allowed implementation scope:

- add bounded HS summary context to the kernel;
- add accepted guidance summary context;
- route Memory/LifeModel learning through proposals;
- add tests proving HS context helps answers without direct writes;
- add tests proving rejected/absent HS context does not break basic answers.

Out of scope:

- full maturation loop in ordinary chat;
- background autonomous truth updates;
- raw LifeModel prompt dumping;
- final acceptance/live-provider proof changes.

## Required Behavior

- HS context is bounded and inspectable;
- accepted guidance can influence wording/planning;
- user memory/life changes require proposal acceptance;
- HS policy can block dangerous or write-like behavior;
- missing HS context degrades gracefully.

## Runtime Contracts

- HS context contract: bounded summary, source/provenance reference, freshness
  metadata, and privacy class.
- Guidance contract: accepted guidance can influence generation but cannot
  override safety/write policy.
- Learning contract: new Memory/LifeModel candidates become proposals only.
- Degradation contract: missing or malformed HS context produces warning
  metadata, not turn failure, unless policy genuinely blocks the request.

## Acceptance Checklist

- [ ] HS summary context appears in kernel context assembly.
- [ ] Accepted guidance summary appears when available.
- [ ] HS does not silently materialize truth from ordinary chat.
- [ ] HS policy can produce proposal/blocker outcome.
- [ ] Basic direct answer still works if HS context is unavailable.

## Verification

```bash
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-core main_chat_agent_v1 -- --nocapture
cargo test -p openlife-tauri main_chat_kernel -- --nocapture
```

## Stop Conditions

- HS requires every ordinary chat turn to write source data.
- HS packet construction makes basic direct answer brittle.
- Maturation logic enters the synchronous kernel turn.
- Raw LifeModel or unrestricted memory is injected as an unbounded prompt dump.
