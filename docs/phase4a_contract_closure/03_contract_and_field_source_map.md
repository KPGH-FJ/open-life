# Phase 4A Contract And Field Source Map

Status: `MERGED_AND_HUMAN_APPROVED`
Date: 2026-07-19

## 1. ReviewItem Decision Context

| Serialized field | Backend source | Frontend rule |
| --- | --- | --- |
| `reviewItemId` | `AgentProposal.id` | Exact dispatch/refreshed-item identity |
| `title` | proposal type mapped by backend | Display only; frontend does not reclassify type |
| `summary` | proposal type + affected path | Main change sentence |
| `before` / `after` | `AgentProposal.before/after` via bounded, redacting serializer | Never render raw proposal JSON as fallback |
| `reasonSummary` | `AgentProposal.reason` | Bounded local-private text |
| `sourceSummary` | `AgentProposal.source` | Product label; raw source ids remain evidence |
| `impactSummary` | backend proposal-type semantics | Must preserve decision vs materialization distinction |
| `affectedObjectLabels` | `affected_path` + proposal domain | Product labels only |
| `expiresAt` | `AgentProposal.expires_at` | No local expiry calculation |
| `permission` | exact permission proposal shape | Required for ToolPermission understanding |
| `evidenceRefs` | proposal/run evidence refs | Inspector entry, not a prose substitute |

Sensitive object keys such as token, secret, password, credential,
authorization, and API key are redacted. ToolPermission raw `after` is never
shown as readable diff; the exact typed permission context replaces it.

## 2. PermissionDecisionContext

| Field | `action_bound` owner | `network_policy` owner | Fail-closed rule |
| --- | --- | --- | --- |
| `scopeKind` | `permission_scope_kind` | same | unknown -> incomplete |
| `policy` | `policy/permission=allow_once` | same | any other value -> incomplete |
| `toolName` | validated `ActionBoundToolPermissionScope` | canonical/top-level tool name | missing -> incomplete |
| `capabilityLabels` | manifest capabilities or validated action type | network capabilities | missing -> incomplete |
| requested/resolved target | validated blocked action scope | blocked target/host | missing target -> incomplete |
| `scopeDigest` | digest of kind + canonical scope + blocked action + policy | same | missing digest -> incomplete |
| request digest/length | input hash/length | input or endpoint digest/length | missing -> incomplete |
| blocked run/step | canonical scope/blocked action | required for Main Chat scoped requests; explicit endpoint consent may omit | partial pair -> incomplete |
| network decision id | not applicable | network policy decision | missing -> incomplete |
| transmission boundary | canonical tool source/capabilities | exact network target | unknown -> incomplete; possible never means sent |
| expiry/duration | proposal expiry + allow-once policy | same | no local duration invention |
| revocation summary | allow-once consumption semantics | same | approval does not imply execution |
| `missingFields` | backend parser | backend parser | non-empty -> status incomplete and Approve disabled |

## 3. ReviewAction

Required fields are `id`, `label`, `kind`, `effect`, `enabled`,
`targetReviewItemId`, and `completionProofAfterDispatch`.

Rules:

- kind/effect mismatch is a Rust contract error;
- enabled actions cannot carry `disabledReason`;
- disabled actions require a non-empty `disabledReason`;
- Approve and Apply require confirmation;
- `completionProofAfterDispatch` must be false;
- expected materialization after dispatch is an expectation, not proof;
- the frontend reducer transitions dispatch success to `refreshing` only;
- a matching refresh that still shows the old decision enters
  `awaiting_projection`, not success;
- only a refreshed matching ReviewItem whose status confirms the requested
  decision may resolve the operation;
- Evidence navigation and task resume use separate handlers/contracts rather
  than this Review decision reducer.

## 4. WorkspaceViewModel

| Field | Source | Rule |
| --- | --- | --- |
| `activeTask` | active `TasksViewModel` item | Only running/waiting-permission/blocked; history is not promoted |
| `recentTaskRefs` | TasksViewModel items | Navigation refs only |
| `pendingReviewItems` | ReviewCenter items linked by canonical terminal-owner relation/refs | Full ReviewItem, no page-local proposal join |
| `activity` | `TaskDetail.evidence_view.event_timeline` | Product-safe metadata only |
| activity status | event kind + normalized lifecycle/failure codes | unknown remains unknown; blocker/error fail closed |
| `providerPrivacyBoundarySummary` | backend provider/privacy owner | No local local-first inference |
| `activityRedactionState` | Workspace builder | Fixed `metadata_only` |
| `sourceRefs` | Tasks + task evidence owners | Inspector/evidence entry |
| `contractLimitations` | backend composer | Visible contract limits, not completion claims |

## 5. Today Adapter

Contract version: `openlife.today-adapter.v1`.

| Product fact | Owner |
| --- | --- |
| readiness, safe mode, task pressure, pending review | `LifeStateProjection` |
| daily goal content/state | `get_daily_goals` compatibility projection |
| provider route/transmission/risk | `ProviderPrivacyBoundarySummary` |
| presentation composition | strict frontend adapter only |

Missing `LifeStateProjection` produces an error envelope even if daily goals
loaded. Missing provider/privacy summary remains route/transmission/risk
unknown. The adapter may not own proposal status, task lifecycle, provider
route, external transmission, or durable completion.

## 6. Settings Orchestration

Refresh order is frozen as:

```text
edit local draft
  -> optional connection test (does not save)
  -> save config command (does not prove boundary)
  -> refresh ProviderPrivacyBoundarySummary
  -> ready only when route, transmission, and risk are all known
```

Boundary refresh failure or an unknown returned boundary remains `unknown`.
The reducer owns transient command lifecycle only; backend config and privacy
summaries remain truth owners.

## 7. Golden And Parity Owners

- Golden JSON: `frontend/src/test/fixtures/phase4a-contract-golden.json`.
- Rust round-trip/invariant test: `review_item.rs`.
- TypeScript parity test: `frontend/src/test/phase4aContractGolden.test.ts`.
- Review dispatch test: `frontend/src/contracts/reviewDispatchContract.test.ts`.
- Settings orchestration test:
  `frontend/src/contracts/settingsOrchestrationContract.test.ts`.
- Production import absence guard: `single_system_authority_tests.rs`.
