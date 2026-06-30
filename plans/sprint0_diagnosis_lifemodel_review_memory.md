# Sprint 0 Diagnosis: LifeModel, Review, Memory Closed Loop

Date: 2026-06-29

Status: Diagnosis packet and RFC outline. Not implemented.

## Raw Issues

Primary issues: `OL-002`, `OL-005`, `V4-003`, `V5-001`, `V5-002`, `V5-003`, `V5-004`, `V5-005`, `V5-013`, `V5-018`, `V5-020`, `V5-022`.

Highest severity:

- `OL-002` P0: read-only builtin operations are misclassified as high-risk/write permissions.
- `OL-005` P1: LifeModel claims are low-quality and hard to source/correct.
- `V4-003` P1: one memory intent creates duplicate weak proposals.
- `V5-001` to `V5-005` P1: Builder/Review/LifeModel write loop is not understandable or traceable.

## Observed Evidence

- v5 real-data write showed accepted low-risk preference persisted in durable data but was not visible in LifeModel Overview.
- Review proposal cards used generic titles such as goal/update-style labels and did not clearly expose affected path, source excerpt, or confidence in a way ordinary users could trust.
- Builder and LifeModel UI could imply update progress before the user had clearly accepted a proposal.
- Existing model summaries included malformed facts and state-like fragments.

## Source Findings

| Area | Finding |
|---|---|
| `frontend/src/pages/LifeModelPage.tsx` | Overview builds four-dimension summaries directly from `LifeModel` fields, then quality-filters short strings. It does not render a canonical proposal/patch/source projection. |
| `frontend/src/utils/lifeModelTrust.ts` | Trust drawer summarizes pending proposals and quality-suppressed items, but accepted source/proposal lineage is still generic. |
| `frontend/src/utils/lifeModelQuality.ts` | Some malformed facts are detected and downgraded in UI, proving a quality guard exists, but it is display-time rather than a full extraction/write quarantine. |
| `frontend/src/pages/MailboxPage.tsx` | Review supports accept/reject/postpone/edit and safe-mode guards. It is a real control point, but the detail contract is not strong enough for source/diff/rollback confidence. |
| `frontend/src/utils/proposalDisplay.ts` | Proposal display already builds before/after diff rows, affected path, source, run link, redaction, and technical rows. This is useful but not enough to close the accepted-write loop. |
| `frontend/src/utils/reviewDecision.ts` | Proposal groups and impact text exist. Titles are still generated from proposal domain/type and can remain too generic for Builder/LifeModel changes. |

## Root-Cause Hypothesis

1. The write path and Review proposal path contain more structure than the LifeModel Overview consumes. Accepted changes are not projected into one canonical user-facing read model.
2. Proposal display is path-aware but not source-span/version-aware enough. Users see "what OpenLife thinks" without enough "why, where from, what changed, can I undo".
3. Builder extraction lacks strict schema boundaries and source-span evidence, so mixed-field facts can become candidate proposals.
4. Low-quality extracted facts are partly caught at display time, but they should be quarantined before becoming Today/LifeModel output.
5. Permission proposal classification for read-only builtin operations is likely too broad or uses risk labels from capability class rather than action semantics.

## Industry Comparison

- ChatGPT Memory exposes memory controls and deletion/disable paths; OpenLife should make every accepted memory/LifeModel fact visible and controllable.
- Notion AI's workspace-style value depends on source confidence; OpenLife should show source and affected path for personal facts.
- Claude Artifacts keeps substantial outputs reusable and editable; LifeModel changes should be durable objects users can inspect and revise, not only ephemeral proposal cards.

## Solution RFC Outline

### Canonical Objects

Create a shared `LifeModelChangeView`:

- proposal id
- affected path
- proposal type
- before value
- after value
- source excerpt or source summary
- source object id/run id/task session id
- confidence
- risk level
- status
- patch id
- snapshot/version id
- rollback/edit availability

Create a canonical `LifeModelCurrentView`:

- current display facts grouped by dimension/path
- source chain per fact
- confidence/risk/last updated
- pending competing proposals
- quality/quarantine state

### UI Contract

- Builder creates candidates, never says a LifeModel fact is updated until accepted.
- Review detail shows path-specific title, before/after diff, source excerpt, confidence, risk, and exact accept result.
- LifeModel Overview shows accepted facts and lets the user trace each fact back to source/proposal/patch/snapshot.
- Versions defaults to latest snapshot and indicates whether it includes the accepted change.
- Today only uses accepted, display-quality facts or clearly labeled pending suggestions.

### Backend Contract

- Accepting a proposal must create or reference a patch/snapshot/change object that can be read back by UI.
- Duplicate memory proposals should be deduped by normalized intent, affected path, source, and value digest.
- Extraction writes should carry source spans and schema validation results.
- Read-only builtin operations must be classified by action type and permission semantics, not just tool family.

## Replay Tests

| Test | Expected |
|---|---|
| Accept `preferences.communication_style` | LifeModel Overview shows the preference with proposal/source trace |
| Builder writes multi-field persona | Proposals use correct affected paths and do not mix identity/goals/state |
| Ask to remember one preference | One deduped proposal appears; title and diff are specific |
| Reject or postpone proposal | No LifeModel/Memory write occurs; status remains traceable |
| Versions after accept | Latest snapshot/patch is visible first or explicitly linked |
| Low-quality extraction | Fact is quarantined or marked pending; Today does not treat it as goal |
| Read-only builtin | Does not appear as high-risk/write permission unless it actually writes or exposes sensitive data |

## Anti-Hallucination Checks

- Do not treat "proposal accepted" toast as proof of LifeModel visibility.
- Verify accepted write through proposal status, patch/snapshot, and current Overview.
- Do not trust extracted fact text without affected path and source span.
- Do not use model-generated confidence text as confidence metadata.
- Do not let UI quality filters hide backend extraction defects without filing regression evidence.

## Thin-Slice Implementation Proposal

1. Pick one low-risk path: `preferences.communication_style`.
2. Define `LifeModelChangeView` for that path from proposal to accepted patch to current view.
3. Update Review detail to show path-specific source/diff/result for this path.
4. Update LifeModel Overview to render the accepted preference and trace link.
5. Add a regression fixture that accepts the preference and asserts it is visible and traceable.

## Open Questions

- Where should rollback live first: Versions, Review history, or LifeModel fact detail?
- Should Builder create only proposals, or can it create a draft snapshot before Review?
- What is the minimum source excerpt needed for privacy-safe personal facts?
