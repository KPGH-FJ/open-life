# OpenLife vNext P12 Task Specifications

Date: 2026-05-08

Status: next

Package:

```text
Beta Release Candidate and User Trial Delivery
```

P12 starts after P11 / P11.1 Beta Trial Readiness has passed acceptance. P11
made OpenLife diagnosable, recoverable, and feedback-ready. P12's job is to
turn that internal readiness into a small-scope Beta Release Candidate that can
be handed to real testers without requiring source-code knowledge or developer
supervision.

P12 is not a new architecture expansion phase. It should not reopen ChatPage,
increase runtime authority, enable shell by default, or add broad new product
surfaces. The goal is a credible trial delivery package: release build drill,
user-facing trial guide, first-run polish, and a recorded RC acceptance run.

Companion checklist:

- `plans/openlife_vnext_p11_trial_path_matrix.md`
- `plans/openlife_vnext_p12_beta_rc_acceptance_report.md`

## Baseline Review

Before P12:

- P9 shell/sandbox governance is stable and default-off.
- P10 Agent Workspace surfaces passed acceptance.
- P11 Trial Readiness surfaces, trial path matrix, recovery guidance, and
  privacy-governed diagnostic export are in place.
- P11.1 diagnostic path redaction is complete.
- `make ci` passes after P11.1.

## Global Rules

- Execute exactly one P12 task spec at a time.
- Optimize for real tester success, not new framework scope.
- Do not rewrite `ChatPage`.
- Do not enable normal chat shell, terminal UI, scheduled shell, proactive
  shell, or sub-agent shell.
- Do not add new runtime privileges without a separate ADR and task spec.
- Do not add automatic upload of diagnostics, LifeModel, memory, chat, prompt,
  or tool output.
- Keep trial delivery artifacts readable by non-developer testers.
- Add tests for changed UI or command contracts.
- `make ci` remains the final gate.

## P12-0: Documentation And Phase Sync

Goal:

Mark P11 / P11.1 as accepted and make P12 discoverable as the current next
phase.

Expected behavior:

- `README.md` and `AGENTS.md` state that P11 / P11.1 passed acceptance and P12
  Beta Release Candidate is the next phase.
- P12 task specs are linked from standard vNext entry points.
- P12 non-goals are explicit: no runtime authority expansion, no shell
  enablement, no ChatPage rewrite.

Allowed edit areas:

- `AGENTS.md`
- `README.md`
- `plans/openlife_vnext_p12_task_specs.md`
- `plans/openlife_vnext_migration_plan.md`
- `plans/openlife_vnext_test_and_acceptance_matrix.md`
- `plans/openlife_vnext_agent_coding_prompts.md`
- `plans/openlife_vnext_p12_beta_rc_acceptance_report.md`

Verification:

- `rg -n "P12|Beta Release Candidate|openlife_vnext_p12_task_specs|P11.*accepted|P11.*验收" AGENTS.md README.md plans`
- `git diff --name-only` contains documentation files only for this task.

## P12-1: User Trial Guide

Goal:

Create a short user-facing guide that a tester can follow without reading
architecture docs.

Expected behavior:

- A tester can learn how to install or launch, configure a model, build a
  minimum LifeModel, complete one conversation, review proposals, inspect runs,
  export diagnostics, and report feedback.
- The guide explains what not to share: API keys, raw LifeModel content, raw
  chat, raw memory, raw tool output, or private files.
- The guide distinguishes optional local Ollama setup from the recommended
  cloud-provider trial path.

Allowed edit areas:

- `README.md`
- optional `BETA_TRIAL_GUIDE.md`
- `plans/openlife_vnext_p12_task_specs.md`

Verification:

- A non-developer tester can follow the guide from a clean profile.
- The guide links to the P11 trial path matrix for detailed smoke scripts.

## P12-2: Release Build Drill

Goal:

Prove that OpenLife can be built as a desktop trial artifact, not only run in
the dev server.

Expected behavior:

- Run and document the release build command for the local platform.
- Record artifact location, build result, and any known platform limitation.
- Fix build blockers if they are in scope and low risk.
- If build/signing/notarization is blocked by credentials or platform setup,
  document the blocker clearly in the RC report instead of pretending it passed.

Allowed edit areas:

- `README.md`
- `plans/openlife_vnext_p12_beta_rc_acceptance_report.md`
- build scripts only if a genuine build blocker is found
- minimal code/config changes required to make the release build complete

Constraints:

- Do not introduce signing credentials into the repo.
- Do not weaken security capabilities to make a build pass.
- Do not skip `make ci`.

Verification:

- `make ci`
- `pnpm --dir frontend build`
- `cargo tauri build` or the repo's documented release build command
- RC report records exact command results and artifact path.

## P12-3: First-Run Golden Path Polish

Goal:

Remove trial-blocking friction from the first-run path without broad UI
rewrites.

Expected behavior:

- First launch directs the tester toward Settings / Overview readiness.
- Model configuration, Builder, Chat, Review Center, Runs, and diagnostic
  export have understandable empty/error states.
- Readiness actions route to the correct fix surface.
- Failure copy tells the tester what to try next and how to export diagnostics.

Allowed edit areas:

- frontend onboarding/settings/workspace/builder/chat/review/runs surfaces
- mocks and tests
- small read-only diagnostics additions if needed

Constraints:

- No ChatPage rewrite.
- No new runtime authority.
- No new model calls just for onboarding/polish.

Verification:

- frontend tests for changed surfaces
- P11 clean-profile smoke paths still pass
- `pnpm --dir frontend typecheck`
- `make ci`

## P12-4: Beta RC Acceptance Run

Goal:

Record a real release-candidate acceptance run and decide whether the build can
be handed to testers.

Expected behavior:

- Fill `plans/openlife_vnext_p12_beta_rc_acceptance_report.md`.
- Run P11-S1 through P11-S8 on a clean profile.
- Run the existing-profile subset from the P11 matrix.
- Record release build command, artifact path, diagnostics export status, known
  issues, and final go/no-go decision.
- No P0/P1 issue remains untriaged.

Allowed edit areas:

- `plans/openlife_vnext_p12_beta_rc_acceptance_report.md`
- docs if the acceptance run reveals outdated instructions
- focused bug fixes if a P0/P1 issue blocks tester handoff

Verification:

- `make ci`
- release build command attempted and recorded
- P11 trial matrix results recorded
- final RC decision is explicit

## P12 Exit Criteria

P12 is complete when:

- P11 / P11.1 is documented as accepted and P12 is the current delivery phase.
- A user-facing trial guide exists.
- A desktop release build has been attempted and its result is recorded.
- Clean-profile and existing-profile smoke paths are recorded in the RC report.
- Diagnostics export remains privacy-governed and path-redacted.
- P9 shell guarantees remain unchanged.
- No untriaged P0/P1 trial blocker remains.
- `make ci` passes.

P12 completion allows:

```text
Small-scope real-user Beta trial: 5-20 testers, controlled feedback loop.
```

P12 completion does not mean:

```text
Public app-store release, production signing/notarization, broad plugin runtime,
or unrestricted external tool execution.
```

Recommended final verification:

- `rg -n "P12|Beta Release Candidate|Beta RC|trial guide|acceptance report" README.md AGENTS.md plans`
- `pnpm --dir frontend test`
- `pnpm --dir frontend typecheck`
- `cargo test -p openlife-core`
- `cargo test -p openlife-tauri`
- `pnpm --dir frontend build`
- `make ci`
- release build command attempted and recorded
