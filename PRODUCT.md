# OpenLife Product Definition

## Purpose

OpenLife is a local-first personal Agent OS for general knowledge work. It lets
a user delegate meaningful, potentially multi-step tasks to a capable Agent,
follow and steer the work, and receive results that can be inspected and
continued. It is not limited to coding work.

OpenLife may use bounded Agent Memory and confirmed LifeModel context to improve
continuity and personalization. Those systems remain optional collaborators of
the Agent harness rather than owners of task execution, permission, or
completion.

## Target Product Loop

1. The user opens or creates a Conversation in the Workbench. Before the first
   send, they can select its Project and Agent Memory use/learning boundary.
   Chat provides a direct answer; Work accepts a meaningful outcome and
   optional files, sources, Project scope, constraints, and desired
   deliverables.
2. Chat and Work share one canonical Conversation, Turn, and typed Item spine.
   Work adds a durable Task and Run with an editable completion contract.
3. OpenLife plans when useful, uses authorized capabilities adaptively, and
   lets the user follow progress, steer, answer questions, pause, resume, or
   approve an important boundary without losing context.
4. OpenLife returns one canonical FinalResult with relevant Artifacts, changes,
   sources, limitations, and verification state.
5. Durable or external effects follow the applicable task scope and risk contract.
   A Review proposal is used only when a governed change needs asynchronous or
   durable review; it is not the container for every task or action.

## Target Core Surfaces

- **Workbench** (`/workspace`): Projects, Conversations, Chat and Work,
  progress, steering, inline decisions, results, and a Needs Attention filter.
- **Personal Intelligence** (`/life-model`): two peer areas with separate
  backend owners: user-owned long-term understanding in LifeModel, and
  user-controlled Agent Memory for work continuity.
- **Settings**: provider/model profiles, privacy and transmission boundaries,
  exact reusable tool-permission inspection and revocation, credential
  recovery, local data controls, and diagnostics. Settings projects permission
  facts from the canonical store; it does not calculate or grant permissions
  in the frontend.

Task, Run, Item, and Approval remain explicit backend facts. They do not each
require a separate top-level product page.

## Non-Negotiable Boundaries

- No silent durable writes.
- Assistant text is not write authorization.
- A task grant authorizes ordinary low-risk, recoverable work inside its
  explicit workspace, resource, provider, and tool scopes. Scope expansion,
  consequential external actions, and destructive or irreversible effects
  require a just-in-time decision.
- External and sensitive actions require a confirmed capability and risk
  contract.
- The provider, model, and any explicitly supported reasoning effort selected
  by the user remain bound to the task. OpenLife may retry that exact route and
  effort, but it must not silently switch model, provider, or reasoning budget.
- Each Work Run also binds one execution ceiling. `scoped_agent` may perform
  ordinary low-risk, recoverable work only inside the separately admitted
  scopes; `observe_only` removes Artifact and personal-intelligence writes.
  Neither mode grants a tool or bypasses just-in-time Review.
- Chat and Work use one provider-agnostic Agent harness. Provider adapters may
  describe authentication, endpoint, streaming, structured-output, reasoning,
  and tool-call transport capabilities, but they must not define a separate
  intent router, planner, tool policy, completion rule, or product flow for a
  vendor or model.
- Missing, stale, or failed evidence must remain visibly unknown or blocked.
- Plans, tool activity, streaming text, and proposal acceptance are progress
  evidence, not proof that the requested result was completed.
- Product state must come from its backend read model when one exists.
- Local, scripted, mock, browser-shell, native-Tauri, and external-live evidence
  are different evidence levels.

## Current Development Baseline

OpenLife has one canonical Chat/Work spine and one Workbench snapshot boundary.
The current source is an engineering baseline, not a claim of market readiness.
Controlled source and browser evidence now closes the foundation loop for
Project folder scope, provider/model selection and readiness, immutable Run
admission, status-specific Stop and new-Run Retry/Continue, steering, Activity,
Results, focused revision, and governed Undo. Work permission Review binds one exact
Task/Run/ToolCall/Action scope: a live approval continues that same Run with a
new Attempt on the waiting ToolCall, while approval after process restart marks
the old Run interrupted instead of pretending that its lost execution context
resumed. Canonical Task history is now visible through one global Activity
surface even when another Conversation is selected. Conversation archive and
restore retain that history; permanent deletion is limited to an archived,
empty Conversation with no Turn, message, or Task references. These remain
engineering facts until native acceptance proves the
shipped product. Exact-native acceptance of the completed loop, desktop
notification activation routing, and user-reviewed formal-profile legacy
LifeModel migration remain open evidence gates. They are gaps under this
product contract, not reasons to introduce another runtime or compatibility
layer.

Artifact Results now show the exact source Run/model, selected local resources,
version relationship, preview, change scope, digest verification, and governed
Undo eligibility supplied by the backend read model. Open, Export, Undo, and
focused-revision failures stay visible on the affected card. A focused revision
creates a new Run bound to one exact verified current ArtifactVersion and the
original provider/model, Skill, resources, execution mode, and Project scope;
it preserves prior FinalResults and versions, keeps the target and media type,
and uses normal verification and replacement Review before a new version can
materialize. Replacement Undo separately retains digest-bound pre-change bytes
and restores them through a reviewed, receipted effect. These remain controlled
source evidence until native acceptance.

Provider capability evidence remains model- and surface-specific. A completed
Chat Turn may validate Chat while a schema-invalid Work AgentStep marks only
Work compatibility failed. The composer refreshes those backend-owned facts
with the terminal Turn; it does not require a restart and does not make one
model failure disable another provider or capability surface.

Earlier development programs remain in Git history. ADR 0018 and ADR 0019
remain the accepted reconstruction and harness contracts. The active
foundation control-loop plan closes each capability vertically through runtime,
persistence, read models, product surfaces, recovery, deletion, and required
evidence before broader Agent capabilities are added.

Repository governance remains small and conventional: at most one active plan,
normal source tests, normal CI, and concise architecture and decision records.
OpenLife must not grow a second internal platform for planning or evaluating
its own development.
