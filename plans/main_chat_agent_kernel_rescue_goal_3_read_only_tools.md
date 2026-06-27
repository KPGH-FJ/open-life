# Goal 3: Minimal Read-Only Tools

> Status: prepared for goal mode
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## Objective

Add a minimal governed read-only tool loop to MainChatKernel for workspace file
read, session search, memory search, and explicit web/network blocker behavior,
verified on both send and stream surfaces without silent writes or fake tool
success.

## System Position

This goal turns the kernel from "model answer" into "agent can observe the
world." It must use the existing ActionExecutor/governance foundations without
pulling in the full legacy ReAct selection, provider-ranked MCP, or live gates.

## OpenLife Lessons Applied

- Tool infrastructure exists, but the product loop did not make it feel usable.
- The first tool set must be small and reliable.
- Unsupported tools should fail closed with named blockers instead of falling
  through to a plausible answer.

## Industry Practices Applied

- Tool interfaces need agent-computer-interface design, clear names, examples,
  and mistake-resistant parameters.
- Guardrails belong around tool calls, not only around prompts.
- Read-only tool results should become observations before final synthesis.

## Scope

Allowed implementation scope:

- extend `src-tauri/src/main_chat_kernel.rs`;
- reuse `ActionExecutor` or a narrow adapter into it;
- reuse workspace-scoped file read helpers;
- reuse memory/session search stores where already available;
- add focused read-only tool tests and command-surface parity tests.

Out of scope:

- write tools;
- provider-ranked MCP selection;
- external live-provider proof;
- broad web/MCP restoration;
- UI redesign beyond any minimal test fixture support.

## First Tool Set

| Tool class | Expected behavior |
| --- | --- |
| `file.read` | Read allowed workspace/safe-path file and record observation. |
| `session.search` | Retrieve bounded session context. |
| `memory.search` | Retrieve bounded memory context. |
| `web.read` | Execute only if governed read path exists, otherwise named network blocker. |
| unknown tool | Named unsupported-tool blocker. |

## Runtime Contracts

- Tool decision contract: selected action class, target, governed input, and
  reason.
- Observation contract: tool name, status, bounded output preview, blocker if
  failed.
- Safety contract: model-supplied arguments cannot override governed input.
- Parity contract: send and stream produce equivalent tool outcome semantics.

## Acceptance Checklist

- [ ] File read success case passes.
- [ ] Path traversal blocker case passes.
- [ ] Session search case passes.
- [ ] Memory search case passes.
- [ ] Unknown tool blocker case passes.
- [ ] Send and stream surfaces produce equivalent outcomes.
- [ ] Tool observations feed follow-up synthesis.
- [ ] Model-provided arguments cannot bypass governed executor input.

## Verification

```bash
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_kernel -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
```

## Stop Conditions

- Read tools require enabling broad write permissions.
- File read cannot be scoped to workspace/safe paths.
- The implementation needs provider-ranked MCP logic before simple reads work.
- Tool observation cannot be represented without legacy strategy transcript
  coupling.
