# Sprint 3 Solution RFC: LifeModel Closed Loop

Date: 2026-06-29

Status: ready for Slice 3A implementation after Sprint 1-2 evidence is committed.

## Scope

Raw issues: `OL-002`, `OL-005`, `V4-003`, `V5-001`, `V5-002`, `V5-003`, `V5-004`, `V5-005`, `V5-013`, `V5-018`, `V5-020`, `V5-022`.

Primary source entrypoints:

- `frontend/src/pages/LifeModelPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/components/BuilderPatchReview.tsx`
- `frontend/src/utils/proposalDisplay.ts`
- `frontend/src/utils/reviewDecision.ts`
- `frontend/src/utils/lifeModelTrust.ts`
- `frontend/src/utils/lifeModelQuality.ts`
- proposal, patch, snapshot, and builder commands in `frontend/src/tauri.ts`

Verified source reality before implementation:

- `AgentProposal` already carries `proposal_type`, `source`, `source_detail`, `affected_path`, `before`, `after`, `confidence`, `risk_level`, and `status`.
- `accept_proposal` already applies LifeModel proposal patches through `apply_proposal_to_state`, persists the model, creates an after snapshot, and stores an applied patch when `patch_store` is available.
- `builder_apply_signals` is retired as a direct-write surface; Builder's normal product path is `builder_create_proposals`.
- `proposalDisplay.ts` already creates metadata-safe diff rows and technical rows, so Slice 3A should strengthen canonical path/source visibility instead of replacing Review wholesale.
- `LifeModelPage.tsx` currently builds Overview summaries directly from `LifeModel` fields and display-time quality filtering. It does not yet expose a source/proposal/patch/current-view chain for accepted facts.

## Product Goal

Users can understand and control what OpenLife believes about them: what changed, why, where it came from, whether it was accepted, where it is visible, and how to correct it.

## Non-Goals

- Do not redesign the whole LifeModel schema in this sprint.
- Do not bulk-process old proposals.
- Do not perform destructive rollback by default.
- Do not let Builder write durable current facts without Review confirmation.

## Canonical View Models

`LifeModelChangeView`:

| Field | Meaning |
|---|---|
| `change_id` | Stable id; proposal id for pending changes, patch id after accept. |
| `status` | pending, accepted, rejected, postponed, quarantined, superseded. |
| `affected_path` | exact LifeModel/Memory path. |
| `domain` | identity, goals, capabilities, state, preferences, memory, permission. |
| `before` / `after` | safe display values. |
| `source_excerpt` | bounded source quote or reason summary. |
| `source_refs` | run id, task session id, builder session id, question id. |
| `confidence` | numeric metadata, not model prose. |
| `risk_level` | low, medium, high, critical. |
| `patch_id` | accepted patch id if applied. |
| `snapshot_id` | snapshot id if available. |
| `visible_in_current_view` | boolean. |
| `rollback_available` | boolean or unavailable reason. |

`LifeModelCurrentView`:

| Field | Meaning |
|---|---|
| `facts` | accepted display facts grouped by dimension/path. |
| `pending_changes` | relevant pending `LifeModelChangeView`s. |
| `quality_warnings` | suspicious or quarantined facts. |
| `source_chain` | proposal -> patch -> snapshot -> current fact. |
| `last_updated_at` | current model or snapshot timestamp. |

## First Thin Slice

Use one low-risk path: `preferences.communication_style`.

Slice 3A is the only approved implementation slice from this RFC. It must close the accepted-write visibility loop for `preferences.communication_style`; broader LifeModel schema repair, Today personalization, dedupe for every domain, and rollback execution stay out of scope.

Path canonicalization contract:

| Surface | Canonical value |
|---|---|
| UI/proposal/current-view display path | `preferences.communication_style` |
| LifeModel struct field | `LifeModel.preferences.communication_style` |
| Accepted aliases from older/current code | `/preferences/communication_style`, `preferences.communication`, `/preferences/communication` |
| Dedupe key | normalized canonical display path + normalized value + bounded source digest |

Implementation must normalize inbound patch/proposal paths before display, dedupe, acceptance, and current-view projection. Dot-path and slash-path variants must not create duplicate pending or accepted facts.

Frozen open-question decisions for Slice 3A:

1. Rollback is discoverability-only in this slice: show the linked snapshot/version or unavailable reason, but do not implement a new destructive rollback action.
2. Builder can only create Review proposals for durable LifeModel changes. It must not call retired direct apply surfaces or imply the current model changed before acceptance.
3. Source excerpt minimum is metadata-safe and bounded: prefer `source_detail`, `reason`, or existing evidence summary; if no source excerpt exists, display "source excerpt unavailable" and keep confidence/risk from typed metadata rather than model prose.
4. Current-view proof is required: an accepted proposal is not considered closed until the canonical current view shows the accepted preference and links back to proposal/patch/snapshot or a typed unavailable reason.

Expected flow:

1. User asks OpenLife to remember a low-risk preference.
2. System creates proposal with canonical `affected_path=preferences.communication_style`, normalizing any older slash-path alias before display.
3. Review detail shows before/after/source/confidence/risk.
4. User accepts.
5. Backend applies patch/snapshot or equivalent durable write.
6. LifeModel Overview shows the accepted preference.
7. Detail links back to proposal/run/source.

## Review UI Contract

- Proposal title must include domain and affected path meaning.
- Detail must show before/after diff and not only metadata summaries.
- Accept result must state where the fact is now visible.
- Reject/postpone must prove no write occurred.
- Unsupported action must explain why and what user can do.

## Builder / Extraction Contract

- Builder candidates are proposals, not accepted facts.
- Mixed-field source answers must produce separate proposals or quarantine.
- Source spans or source question ids must be preserved.
- Low-confidence/malformed facts must not enter Today.

## Permission Classification Contract

Read-only builtin operations are not high-risk write permissions unless they:

- expose sensitive data outside local boundary,
- write durable user facts,
- call external provider/tool,
- execute destructive operation.

Risk label must be derived from action semantics, not tool family alone.

## Tests

Backend:

- Accept `preferences.communication_style` creates accepted change and current view entry.
- Reject/postpone does not write current view.
- Duplicate memory intent is deduped by path/value/source digest.
- Slash-path alias `/preferences/communication_style` normalizes to `preferences.communication_style`.
- Dot-path and slash-path variants do not produce duplicate Review proposals or duplicate current-view facts.
- Read-only builtin classification does not become write permission.

Frontend:

- Review detail renders path-specific title, diff, source, confidence, risk.
- LifeModel Overview shows accepted preference and source trace.
- Builder does not claim durable update before acceptance.

Focused command gates for Slice 3A:

- `cargo test -p openlife-tauri lifemodel_closed_loop`
- `cargo test -p openlife-tauri proposal_accept`
- `cd frontend && corepack pnpm test -- LifeModelPage.test.tsx MailboxPage.test.tsx proposalDisplay.test.ts`
- `cd frontend && corepack pnpm typecheck`
- `cargo fmt --check`
- `git diff --check`

If `lifemodel_closed_loop` matches zero tests at first, create focused tests in `src-tauri/src/commands/proposal.rs` or a dedicated module before claiming the gate passed.

Replay:

- v5 real LifeModel write.
- v4 duplicate memory proposal.
- v5 malformed extraction and generic proposal titles.

## Development Slices

1. Define `LifeModelChangeView` for preference path.
2. Expose current view entry for accepted preference.
3. Update Review detail display for this path.
4. Update LifeModel Overview trace rendering.
5. Add focused fixture test for proposal -> accept -> overview.

Exit only when accepted low-risk preference is visible and traceable.

## Slice 3A Implementation Contract

Backend owner paths:

- `src-tauri/src/commands/proposal.rs`
- `src-tauri/src/commands/life_model.rs`
- `openlife-core/src/agent/types.rs`
- `openlife-core/src/life_model/patch.rs`

Frontend owner paths:

- `frontend/src/pages/LifeModelPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/utils/proposalDisplay.ts`
- `frontend/src/tauri.ts`

Implementation steps:

1. Add a small canonical path helper for LifeModel proposal paths. It must normalize `/preferences/communication_style`, `preferences.communication_style`, `preferences.communication`, and `/preferences/communication` to `preferences.communication_style`.
2. Add `LifeModelChangeView` and `LifeModelCurrentView` read models for the approved preference path only. Reuse existing proposal, patch, snapshot, and current `LifeModel` data; do not create a second source of truth.
3. Expose the read model through a Tauri command or extend the existing LifeModel surface in a way that keeps raw proposal values metadata-safe.
4. Update Review/Mailbox display so a communication-style proposal has a path-specific title, before/after diff, source excerpt/unavailable reason, typed confidence, typed risk, proposal id, and run/source link when present.
5. After accepting the proposal, show an accept result that names the current-view path and patch/snapshot availability. Reject/postpone must not show current-view write success.
6. Update LifeModel Overview to render accepted `preferences.communication_style` with trace metadata. If patch/snapshot cannot be linked yet, show an explicit unavailable reason instead of hiding the gap.
7. Add regression tests proving slash and dot path aliases do not create duplicate current-view entries and that a failed/unsupported accept cannot be rendered as visible.

Anti-hallucination checks:

- Do not treat an accept toast, proposal status alone, or generic Overview text as proof. Verify proposal status plus applied patch/snapshot plus current view projection.
- Do not infer source from the proposal title. Source must come from typed fields: `source`, `source_detail`, `run_id`, evidence summaries, patch id, or snapshot id.
- Do not use model-generated confidence wording. Use numeric `proposal.confidence` and `proposal.risk_level`.
- Do not hide missing patch/snapshot evidence behind polished copy. Show a typed unavailable reason and leave a regression note.
- Do not re-enable Builder direct apply or broad batch proposal processing.

Industry bar:

- Match ChatGPT Memory's control expectation: the user can see the accepted preference and know how to change or remove it later.
- Match Notion AI's source expectation: a personal fact must have source/provenance context, not only a summarized claim.
- Match Claude Artifacts' object expectation: the accepted change is a reusable, inspectable object with a stable id, not only an ephemeral message.
