# Backend / Frontend Contract

## Tauri Bridge

Finding: `frontend/src/tauri.ts` is the product bridge and wraps Tauri invoke
calls through `safeInvoke`.

Evidence:

- `safeInvoke` rejects calls outside the Tauri desktop environment.
- Dev logging redacts secrets, payloads, tool arguments, content, notes, and
  message bodies before logging.
- Argument aliasing supports both snake_case and camelCase command fields.

File location:

- `frontend/src/tauri.ts`

Confidence: High.

Impact: The bridge already has useful safety behavior. Frontend v2 should keep
one typed product bridge and avoid page-local command ad hoc code.

## Chat Contract

Finding: The newer chat path returns structured results, but the old
`sendMessage` wrapper still returns only reply text.

Evidence:

- `sendMessage` is marked deprecated and returns `result.reply`.
- `sendMessageV2` returns full `SendMessageResult`.
- `startStreamMessage` returns `StreamMessageDonePayload` and passes an `args`
  envelope plus aliased top-level fields.
- `SendMessageResult` includes reasoning trace, tool calls, run id,
  agent ingress, agent state, execution transcript, model/tool invocation
  flags, and blockers.

File location:

- `frontend/src/tauri.ts`

Confidence: High.

Impact: Frontend v2 should consume the structured contract only. Reply-only
wrappers are a product-experience liability.

## LifeStateProjection

Finding: A backend read model exists for common product state.

Evidence:

- Backend `LifeStateProjection` aggregates pending review counts, readiness,
  task state, safe mode, tool permissions, safe paths, surfaces, and source
  refs.
- Frontend type mirrors the projection.
- Today, Chat, Mailbox, LifeModel, and Settings read the projection.

File location:

- `src-tauri/src/life_state_projection.rs`
- `frontend/src/tauri.ts`
- `frontend/src/utils/lifeStateProjection.ts`
- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/pages/LifeModelPage.tsx`
- `frontend/src/pages/SettingsPage.tsx`

Confidence: High.

Impact: Frontend v2 should make the projection the product state authority for
shared readiness, pending review, safe mode, and task state.

## Contract Gaps

Finding: Product pages still combine projection reads with raw domain reads.

Evidence:

- Today reads `getLifeStateProjection` and `getDailyGoals`.
- Mailbox reads `getLifeStateProjection` and `listProposals`.
- Chat reads projection, diagnostics, LifeModel, scheduler config, sessions,
  proposals, runs, events, and task state.

File location:

- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/pages/ChatPage.tsx`

Confidence: High.

Impact: This is the main frontend contract debt. The projection helps, but v2
needs a clearer view-model boundary per workflow.

## Dev/Test Bridge

Finding: Old route wrappers still exist in `frontend/src/tauriDev.ts` as
dev/test-only compatibility, not product bridge authority.

Evidence:

- Raw scans find old maturity, beta, migration, and cutover command names in
  `tauriDev.ts`, tests, and mocks.
- `single_system` guards passed and product pages are guarded against importing
  `tauriDev.ts`.

File location:

- `frontend/src/tauriDev.ts`
- `src-tauri/src/single_system_authority_tests.rs`
- `frontend/src/test/mocks/tauri.ts`

Confidence: High.

Impact: Do not delete or restore from raw hits during design. Classify dev/test
surfaces separately.
