# OpenLife Product Experience Contract

Status: Accepted design baseline for Gate A
Date: 2026-08-24
Authority: `PRODUCT.md` defines product identity; ADR 0018 defines the canonical
lifecycle. This document fixes the user-facing concepts, interaction states,
and backend ViewModel boundaries used by product design and implementation.

## 1. Product promise

OpenLife is a local-first personal Agent OS for ordinary knowledge work. A
person can start a direct conversation, open a real local folder as a Project,
delegate a multi-step outcome, follow and steer the work, inspect changes and
sources, and recover when something fails.

The first product-quality target is the normal experience of leading desktop
Agent tools. Differentiation, a connector platform, multi-agent productization,
scheduling, Computer Use, and an account system remain out of scope until the
core experience is dependable.

## 2. Canonical user model

```text
OpenLife
├── Provider Profile
│   └── available Model
├── optional Project → one real local folder
│   └── Conversation
│       ├── Chat Turn
│       └── Work Turn → Task → Run → FinalResult → ArtifactVersion
├── History → canonical Conversations and Tasks
├── optional Agent Memory
└── optional LifeModel
```

These are the only primary user concepts:

- **Profile** is a saved route to one provider. It owns endpoint, credential
  reference, protocol, discovered or configured models, default model,
  capabilities, privacy boundary, and last verification state.
- **Model** is selected within a Profile. The selected Profile, model, and any
  supported reasoning effort are bound to the Turn or Run and never change
  silently.
- **Project** is a real local folder selected through the native picker. The
  folder path, permission state, availability, and last verified identity are
  backend facts. A name-only Project is invalid.
- **Conversation** is the durable thread. Chat and Work share it.
- **Task** exists only for Work. A Run is one execution attempt; retry creates
  a new Run and preserves prior evidence.
- **Result** is the user-facing completion surface. It contains outcome,
  changes, Artifacts, sources, limitations, and verification state.
- **Agent Memory** and **LifeModel** are optional personal-intelligence
  collaborators. They never own ordinary task execution or become a prerequisite
  for local-file work.

Task, Run, Review, Activity, and diagnostics remain explicit backend facts but
do not become competing top-level product destinations.

## 3. Primary navigation and shell

The default desktop shell has three regions:

| Region | Persistent responsibility | Must not contain |
| --- | --- | --- |
| Left sidebar | New chat, Open folder, Projects, Conversations, History, Settings | duplicate Task and Activity trees, raw diagnostics |
| Center | one Conversation thread, compact Work progress, steering, composer | permanent settings forms, full technical receipts |
| Right context panel | preview, diff, Review, Result, source, task details when invoked | a second primary navigation or an always-open log stream |

The right panel is closed by default and opens in context. At narrow widths it
becomes an overlay. At 200% zoom the left sidebar may collapse to a drawer and
the context panel becomes a full-width layer; the thread and composer remain
usable without horizontal scrolling.

The first usable screen has three obvious actions: **New chat**, **Open
folder**, and a short list of recent Projects or Conversations. Empty-state
education is concise and secondary to those actions.

## 4. Conversation and composer

Chat and Work use one composer and one thread.

- A conversation without a Project starts in **Chat**.
- A conversation created by **Open folder** starts in **Work**.
- The user may switch mode before sending. Switching never changes Profile,
  model, Project, resources, or Memory boundaries silently.
- The composer exposes current mode, Profile/model, Project or added resources,
  and a send/stop control. Advanced boundaries are progressive disclosure.
- Profile/model selection is available at the point of use. Profile lifecycle,
  credentials, endpoint editing, validation, and defaults live in Settings.
- Attachments and resources show exact scope. A Project folder is a writable
  task scope when authorized; an additional folder is read-only unless the user
  explicitly expands scope.

### Chat

Chat returns a direct answer and can use explicitly available sources. It does
not create a Task merely because a model call occurred.

### Work

Work accepts an outcome, inputs, constraints, and desired deliverables. The
thread shows only the useful execution summary:

1. plan or current intent when planning is useful;
2. active step and evidence-producing actions;
3. questions, steering, stop, resume, or Review when needed;
4. one canonical Result.

Raw tool calls and technical receipts are available in details, not repeated
as the main conversation.

## 5. Provider and model profile contract

A persistent Profile has the following backend-owned projection:

| Field | User-facing behavior |
| --- | --- |
| identity and label | stable saved entry; rename and delete supported |
| provider and protocol | cloud vendor, compatible endpoint, or local Ollama |
| endpoint | editable where applicable; normalized and validated by backend |
| credential reference | masked; never returned as plaintext to UI, logs, screenshots, or diagnostics |
| models | discovered or configured list with per-surface capabilities |
| default model | used for new conversations unless the user chooses another |
| capabilities | Chat, Work structured steps, tools, reasoning, vision, context limits as evidence permits |
| privacy boundary | local or external transmission summary visible before use |
| verification | untested, checking, ready, limited, unavailable, or failed with timestamp and recovery action |

Provider reachability, Chat compatibility, Work compatibility, tool success,
goal completion, and formal release integrity are separate states. One success
does not imply another.

## 6. Project and resource contract

**Open folder** invokes the native folder picker, then creates or reuses a
Project bound to that exact folder. A Project ViewModel includes display name,
canonical path, availability, permission state, last opened time, and recovery
action when stale or missing.

The minimum Project capability set is:

- enumerate folders and files;
- search names and supported text content;
- read one or many supported files;
- create, modify, and rename files inside admitted write scope;
- preview and diff proposed or materialized changes;
- accept, reject, and undo where the effect contract permits;
- restore Project, Conversation, Task, Result, and selection after restart.

Supported reading for the first wave: text, Markdown, code, JSON, YAML, CSV,
PDF, DOCX, XLSX, PPTX, and common images. Supported creation: Markdown, HTML,
CSV, JSON, DOCX, XLSX, PPTX, PDF, and multi-file outputs. A format is not shown
as supported until its read or materialization path has product-level evidence.

Path failures name the affected folder or file and distinguish missing,
permission denied, moved, stale identity, unsupported format, and concurrent
change. Non-ASCII and long paths are normal supported cases.

## 7. State and truth language

All major surfaces use the same state vocabulary:

| State | Meaning | Primary action |
| --- | --- | --- |
| empty | no object exists yet | create or open |
| ready | required evidence is current | continue or send |
| running | a live Run is producing progress | steer or stop |
| waiting-input | the Run needs user information | answer |
| waiting-review | a governed boundary needs a decision | review |
| stopped | user stopped the live Run | resume when resumable or retry |
| failed | a known failure ended the attempt | fix and retry as new Run |
| effect-unknown | an external effect may have occurred | inspect before retry |
| unavailable | provider, model, Project, or resource cannot currently be used | reconnect, reselect, or repair |
| stale | prior evidence no longer proves current truth | refresh or revalidate |
| completed | the original goal and required evidence are satisfied | inspect Result or continue |

The UI never translates unknown, stale, blocked, or partial evidence into
success. “Verified” is reserved for a named check with current evidence.

## 8. Result, preview, diff, and undo

A completed Work Turn opens one Result summary in the thread and may open the
right panel for detail. The Result always answers:

- what outcome was produced;
- which files, sources, or external effects were involved;
- what was changed and where;
- which Profile/model and Run produced it;
- what was verified, what was not, and any limitations;
- which next actions are safe: open, export, revise, compare, undo, or continue.

Preview is content-first. Diff is path- and version-specific. Undo is offered
only when the backend proves the exact version and preconditions. A proposal or
assistant statement never counts as materialization evidence.

## 9. Review and permission behavior

Ordinary low-risk, recoverable work inside the user's selected Project,
resource, provider, and tool scopes may proceed without per-action prompts.

Just-in-time Review is required for scope expansion, sensitive disclosure,
destructive or irreversible effects, consequential external actions, and
LifeModel changes. A Review shows in plain language:

1. the exact action and target;
2. why it is needed for the current goal;
3. data leaving the device or permanent effects;
4. approve once, reject, and any narrower safe alternative.

Technical identifiers and receipts are folded under Details. Approval resumes
only the exact admitted action; it is not a blanket permission.

## 10. Failure and recovery

Every blocking error presents three layers in this order:

1. **What happened** in user language.
2. **What it affects**: message, current Run, Project, Profile, or external
   effect.
3. **What to do next** with one direct recovery action.

Examples include reselect folder, reconnect Profile, choose compatible model,
grant the exact Review, retry as a new Run, or inspect a possible external
effect. Stack traces, protocol payloads, hashes, and internal IDs remain in
copyable technical details.

## 11. Personal intelligence boundary

Agent Memory is opt-in per Conversation and supports explicit remember, source
inspection, edit, forget, and undo. LifeModel is separately user-owned and
requires Review for changes. Reading a Project file, creating an Artifact, or
answering a question never authorizes a Memory or LifeModel write.

Personal-intelligence actions are excluded from the initial Work capability set
unless the user request independently proves that intent. Source text proves
provenance, not permission.

## 12. Fixed product journeys

The design and formal-app acceptance cover these journeys and their empty,
loading, success, failure, cancellation, restart, narrow-window, keyboard,
VoiceOver, reduced-motion, and 200% zoom states:

1. first launch, create Profile, validate, and select a model;
2. no-Project Chat;
3. open a local folder as a Project;
4. enumerate, search, and read Project files;
5. create, modify, rename, preview, diff, and undo;
6. long Work plan, progress, steering, stop, and resume;
7. just-in-time Review;
8. failure diagnosis and recovery;
9. History search, rename, archive, restore, and deletion;
10. optional Agent Memory and LifeModel;
11. public Web research with citations and evidence limitations;
12. standalone files, URLs, and resources with and without a Project.

Natural-language scenarios cannot enumerate every failure. Every journey also
enforces these invariants: goal mismatch cannot complete; required capabilities
need successful receipts; unobserved files or sources cannot be claimed as
read; unmaterialized files cannot be claimed as created; Memory/LifeModel writes
need independent explicit intent; unknown and stale remain visible; formal
delivery requires formal installed-app evidence.

## 13. Visual and accessibility baseline

- Direction: trustworthy, calm, mature, and content-led.
- Preserve the existing neutral palette and restrained status colors; use the
  blue focus color for focus and selection clarity, not decoration.
- UI type follows the production platform system stack. The repository-native
  prototype imports the same CSS token source; monospace is reserved for paths,
  code, hashes, and technical details.
- Base type sizes remain 12, 14, 15, 20, and 24 px; spacing follows the existing
  4 px scale; radii remain 4, 6, and 8 px.
- Primary controls are at least 36 px high in normal desktop density and 44 px
  when touch or zoom conditions require it.
- Focus is always visible. Status never relies on color alone. Reduced motion
  removes nonessential transitions. Keyboard order follows visual order.
- The normal 1440×900 layout, a 1024×768 narrow layout, and 200% zoom are
  explicit design and acceptance targets.

## 14. Backend ViewModel boundary

The frontend renders backend projections for Profile/model readiness, Project
scope, Conversation/Task state, Review, Result, Artifact, permission, and
personal intelligence. It may format and filter those projections, but it does
not recompute truth from raw config, diagnostics, receipts, or database
fragments.

Each production slice must trace a user action from the formal native entry,
through IPC, runtime, persistence and external or filesystem effect, back to
the same product state. Replacement is complete only after the old competing
consumer is deleted and formal native evidence passes.
