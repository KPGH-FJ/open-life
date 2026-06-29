# Sprint 3 Solution RFC: LifeModel Closed Loop

Date: 2026-06-29

Status: ready for design review; implement after Sprint 1-2 evidence is stable.

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

Path canonicalization contract:

| Surface | Canonical value |
|---|---|
| UI/proposal/current-view display path | `preferences.communication_style` |
| LifeModel struct field | `LifeModel.preferences.communication_style` |
| Accepted aliases from older/current code | `/preferences/communication_style`, `preferences.communication`, `/preferences/communication` |
| Dedupe key | normalized canonical display path + normalized value + bounded source digest |

Implementation must normalize inbound patch/proposal paths before display, dedupe, acceptance, and current-view projection. Dot-path and slash-path variants must not create duplicate pending or accepted facts.

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
