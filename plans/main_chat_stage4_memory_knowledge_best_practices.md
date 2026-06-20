# Main Chat Stage 4 Memory And Knowledge Best Practices

> Date: 2026-06-20
> Stage: Stage 4 - Memory and Knowledge Asset Productization
> Status: preparation reference

## 1. Purpose

Stage 4 should learn from first-line agent products without copying private or
unverified internals. Public sources establish the product and architecture
principles. Hermes/OpenClaw remain product benchmarks for perceived capability,
but this document does not claim access to their private implementation.

Stage 4 is about making memory and knowledge assets understandable,
controllable, reversible, and actually useful to the Agent runtime.

## 2. Source-backed Principles

### 2.1 Scoped instruction files work because they are plain, inspectable context

Codex uses `AGENTS.md` as layered repository guidance and skills as modular
instruction/resource bundles. The important pattern is not the filename itself;
it is scope, inspectability, and progressive disclosure.

OpenLife implication:

- keep `AGENTS.md` and selected `SKILL.md` as bounded context surfaces;
- show whether a file was loaded, skipped, truncated, or selected;
- do not let file context override privacy, model routing, tool policy, or
  memory governance;
- use skills for workflow knowledge, not user memory.

References:

- https://developers.openai.com/codex/guides/agents-md
- https://developers.openai.com/codex/skills

### 2.2 Project memory should be readable, scoped, and editable

Claude Code documents two complementary memory mechanisms: user-written
instruction files and auto memory notes derived from corrections/preferences.
It also documents loading limits and audit/edit flows.

OpenLife implication:

- keep human-readable knowledge files short and curated;
- let detailed records live behind the summary surface;
- require source/provenance for agent-written memory;
- make memory audit and edit a first-class product path.

Reference:

- https://code.claude.com/docs/en/memory

### 2.3 File-backed memory is useful, but unrestricted read/write is not enough

Claude's API memory tool stores knowledge in a file directory that can be read,
updated, and deleted across sessions. This validates the file-directory pattern,
but OpenLife has a stricter product requirement: accepted memory must remain
proposal-first, evidence-backed, and reversible.

OpenLife implication:

- `USER.md` and `MEMORY.md` should be managed materialized views or curated
  projections, not independent sources of truth;
- `USER.md` and `MEMORY.md` writes require proposal, confirmation, provenance,
  audit, context reload, and rollback/snapshot;
- `SOUL.md` is higher-risk identity/value context and should remain read-only or
  use an explicitly high-risk confirmation path in Stage 4;
- the runtime store owns accepted memory status.

Reference:

- https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool

### 2.4 Saved memories and inferred chat history must be separated

ChatGPT separates saved memories from reference chat history. Saved memories are
for details a user wants retained; chat history can provide personalization but
is mutable and not exhaustive.

OpenLife implication:

- accepted memory is different from raw transcript retrieval;
- raw chat/session/vector hits can be evidence or search context, not durable
  user truth;
- user-facing memory must show whether it is explicit, inferred, accepted,
  rejected, rolled back, or only a retrieved source.

References:

- https://help.openai.com/en/articles/11146739-how-does-reference-saved-memories-work
- https://help.openai.com/en/articles/8590148-memory-faq

### 2.5 Memory controls must include view, update, disable/reference, and delete/forget semantics

Gemini Enterprise exposes saved-memory creation, view, update, reference
toggle, conversation/data-source controls, and delete flows. The exact product
surface differs from OpenLife, but the control categories are useful.

OpenLife implication:

- users need a memory/knowledge manager, not only inline proposal cards;
- users need to know whether a memory is referenced in runtime context;
- delete/forget/rollback semantics must be explicit and not confused with raw
  transcript deletion.

Reference:

- https://docs.cloud.google.com/gemini/enterprise/docs/configure-personalization

### 2.6 Long-term memory needs namespaces and memory-type separation

LangGraph describes long-term memory as cross-session memory in custom
namespaces, and treats memory as a design problem without a single universal
answer.

OpenLife implication:

- use scopes such as global, workspace, project, and conversation;
- separate preference/fact/workflow/correction/boundary categories;
- do not use one vector store as the whole memory product;
- context retrieval should be selective and policy-aware.

Reference:

- https://docs.langchain.com/oss/python/concepts/memory

### 2.7 Guardrails and tracing are part of memory governance

OpenAI Agents SDK guardrails separate checks on input, output, and tool calls.
Tracing records model generations, tools, handoffs, guardrails, and custom
events.

OpenLife implication:

- memory extraction, proposal creation, acceptance, materialization, rollback,
  and context consumption need traceable events;
- high-risk memory needs confirmation and possibly blocker state;
- final delivery should show durable memory changes, pending changes, and
  rollback availability.

References:

- https://openai.github.io/openai-agents-python/guardrails/
- https://openai.github.io/openai-agents-python/tracing/

## 3. Product Lessons For Stage 4

- Memory should not feel magical or hidden.
- The user must be able to see what OpenLife remembers and why.
- The Agent must use accepted memory in the main task path, not only store it.
- Rejected or rolled-back memory must not re-enter context through old vector or
  transcript retrieval paths.
- Knowledge files are valuable because they are readable, but they are context
  surfaces, not policy authority.
- A good memory product needs correction and rollback at least as much as it
  needs capture.

## 4. OpenLife-specific Best-practice Translation

Stage 4 should build on existing OpenLife strengths:

- `MemoryLifecycleStore` is already closer to product memory truth than
  `MemoryStore` / vector search.
- `ProposalStore` and Review Center already provide confirmation mechanics.
- `main_chat_context_loader` already loads bounded `AGENTS.md`, `SOUL.md`,
  `USER.md`, `MEMORY.md`, and selected `SKILL.md`.
- `AgentControlPlane` already has proposal and rollback affordances.
- Stage 3 already made execution state visible.

The next step is not to invent another memory system. The next step is to make
the existing lifecycle and knowledge surfaces coherent, inspectable, and
consumed by default Main Chat behavior.
