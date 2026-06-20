# Main Chat Agent Beta v1 Hardening And Readiness Contract

> Date: 2026-06-18
> Workstream: 5 of 5
> Status: preparation artifact

## 1. Product Goal

Make Beta v1 shippable without hiding gaps.

Hardening does not mean adding more disclaimers. It means the product is
observable, recoverable, testable, and honest about what is complete.

## 2. Benchmark Insight

Codex best practices emphasize tests, checks, and review before accepting work.
Codex and Claude both expose inspectable configuration/instruction surfaces.
OpenClaw filters visible tools through policy/provider/sandbox/channel/plugin
availability. Hermes publicly documents iteration budgets, cancellation,
fallback, compression, and persistence.

OpenLife implication:

- Beta readiness must measure runtime, UI, eval, permission, memory, and live
  evidence together.
- Unsupported actions should fail closed with product states.
- External live proof must remain opt-in and cannot replace deterministic
  readiness.

## 3. Readiness Dimensions

| Dimension | Required proof |
| --- | --- |
| Routing | Work-like prompts create governed task sessions. |
| UI | Main Chat renders task/event/control/final states from runtime evidence. |
| Events | Delta stream replay works after reconnect. |
| Memory | Proposal/accept/reject/edit/rollback lifecycle works. |
| Plan | Draft/edit/confirm/skip/execute/review lifecycle works. |
| Tools | Read-only file/session/memory/web/MCP/skill paths work or block clearly. |
| Permissions | Exact pending permission resumes exact action only. |
| Recovery | retry/cancel/resume/stale guards are tested. |
| Final delivery | Completed/proposed/blocked/skipped/next-action inventory is accurate. |
| Live provider | Opt-in external live evidence is auditable and non-default. |
| No silent writes | Durable writes require proposal/confirmation or explicit policy. |
| No legacy bypass | Legacy fallback is visible and counted. |

## 4. Final Gate Shape

Add or extend a single Beta v1 readiness command/report:

- command name: `run_main_chat_agent_beta_v1_readiness_gate`;
- report name: `MainChatAgentBetaV1ReadinessReport`.

The default readiness command must run in an isolated eval state. It must not
write durable user app-store state, must not mutate real user memory or
knowledge assets, and must not invoke external providers unless live eval is
explicitly opted in. The report must be metadata-safe and must not serialize API
keys, raw secrets, or private provider credentials.

The report must aggregate:

- foundation inventory: verified / partial / missing;
- deterministic runtime scenario results;
- ordinary command-surface results;
- UI state mapping results for every claimed user-visible state;
- real task vertical scenario results;
- memory lifecycle results;
- event replay results;
- plan lifecycle results;
- task continuity results;
- skills/tool surface results;
- final delivery accuracy results;
- external live opt-in results when enabled;
- blockers and unsupported features.

The report must not use a single boolean without structured evidence.
It must also distinguish required default deterministic readiness from opt-in
external live readiness.
If a claimed product state has no UI mapping proof, the corresponding readiness
dimension must be blocked. UI evidence is not optional for Beta v1 because the
stage is product integration, not backend-only maturity.

## 5. Required Blockers

Beta readiness must fail closed for:

- missing or unverified foundation required by a claimed product capability;
- missing UI mapping proof for a claimed user-visible state;
- missing task session for work-like prompt;
- missing action/observation proof for claimed execution;
- memory write without proposal/confirmation;
- rollback not excluding active memory;
- plan execution against stale revision;
- event replay mismatch or duplicate sequence;
- permission approval replaying wrong action;
- external live report using local/mock provider identity;
- final delivery claiming blocked/proposed work as completed;
- hidden legacy fallback;
- silent durable write.

## 6. Test Plan

Minimum local gate:

```bash
git diff --check
cargo test -p openlife-core main_chat_agent -- --nocapture
cargo test -p openlife-tauri main_chat -- --nocapture
pnpm --dir frontend typecheck
pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx
```

The actual implementation may narrow test filters to existing module names, but
the final result must report which runtime, Tauri, and frontend gates were run.
If these existing tests do not cover the final Beta UI states, the implementation
must add a focused `main-chat` UI/component gate and update this command rather
than claiming coverage from unrelated frontend tests. The focused gate must
cover:

- task frame rendering from runtime data;
- timeline event states;
- controls enabled/disabled by runtime state;
- final delivery sections;
- reconnect/event replay state hydration.

External live gate remains opt-in:

```bash
OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1 \
OPENLIFE_LIVE_EVAL_PROVIDER=deepseek \
OPENLIFE_LIVE_EVAL_BASE=https://api.deepseek.com \
OPENLIFE_LIVE_EVAL_MODEL=deepseek-v4-flash \
cargo test -p openlife-tauri main_chat_live_provider -- --ignored --nocapture
```

Never write API keys into repository files.

## 7. Release Notes Requirements

Beta release notes must state:

- what work the agent can do by default;
- what remains proposal-first;
- what is blocked or unsupported;
- how to inspect task evidence;
- how to inspect knowledge assets;
- how to run readiness gates;
- whether external live evidence was run.

## 8. Acceptance

Beta hardening is acceptable when:

- the readiness report fails closed when any required dimension is missing;
- every claimed user-visible state has UI mapping proof backed by runtime
  evidence;
- `run_main_chat_agent_beta_v1_readiness_gate` returns
  `MainChatAgentBetaV1ReadinessReport` with structured default-readiness and
  opt-in-live sections;
- required default dimensions cannot be marked complete by documenting blockers;
  blockers are acceptable only for unsupported, risky, or opt-in live behavior;
- every failure has scenario id, product state, and blocker reason;
- external live scenarios are separated from deterministic readiness;
- all user-visible claims map to runtime evidence;
- test output and final report are enough for a reviewer to decide whether Beta
  v1 is ready.

## 9. Out Of Scope

- Removing all legacy code.
- Solving every future autonomy problem.
- Full external write automation.
- Public marketplace hardening.
- Enterprise sync/security audit.
