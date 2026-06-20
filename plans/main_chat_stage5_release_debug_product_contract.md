# Main Chat Stage 5 Release Debug Product Contract

> Date: 2026-06-20
> Stage: Stage 5 - Internal Trial Release and Debug Operations
> Status: preparation contract

## 1. Product Goal

Make Agent behavior inspectable and reportable enough for internal testing.

Stage 5 should turn "the Agent did something weird" into a structured artifact:

- what build;
- what environment;
- what task/session/run;
- what route/strategy/provider/tool/context/memory;
- what action/observation/final delivery;
- what failed;
- what the tester should do next;
- what evidence can be safely attached to an issue.

## 2. Product Objects

| Object | Meaning | Source of truth |
| --- | --- | --- |
| Release build info | Commit, branch, version, build timestamp, dirty-state flag if available. | Git/build metadata and app config. |
| Environment preflight | Provider, network, key presence, scheduler, workspace root, safe paths, MCP registry, database/readiness blockers. | Existing config/diagnostics/live-provider preflight/state. |
| Debug bundle | Metadata-safe snapshot of one task/session/run plus relevant context and UI evidence. | Agent runtime stores plus Stage 3/4 reports. |
| Failure classification | Stable failure class, severity, scope, recoverability, recovery recommendation. | Failure taxonomy plus runtime blockers/errors. |
| Issue report | Tester-facing report that links build, scenario, task when task-attached, bundle, status, reviewer, and notes. | Local artifact writer or Tauri command. |
| Stage 5 report | DBG5 coverage report proving release/debug mechanics. | Focused Stage 5 eval module. |

## 2.1 Artifact Storage Lifecycle

Debug bundles and issue reports are local product artifacts, not workspace
source files.

Default storage rules:

- write artifacts under the app data directory, not under the git workspace;
- use typed schema versions for bundles and issue reports;
- write through an atomic temp-file-then-rename path;
- return artifact id, created timestamp, schema version, relative storage alias,
  digest, and byte size;
- support list/get for bundle and issue report metadata after app refresh;
- support explicit delete or retention pruning so debug artifacts do not grow
  without bound;
- never require a raw-content export mode for DBG5 acceptance.

Artifact paths shown in UI or reports should use a local artifact alias or
digest. Absolute private host paths should not be exported unless an existing
resolver policy has already approved them.

## 2.2 Build Provenance

Build provenance should be collected from deterministic build/app metadata, not
from ad hoc runtime shell commands.

Preferred sources:

- commit and branch from build-time environment variables or generated metadata;
- app version from Tauri/package metadata;
- build timestamp from build-time metadata when available;
- dirty-state only in dev builds, or a named unavailable blocker when not
  available.

Unknown or stale build provenance must be represented as a named blocker. Do not
fabricate a commit, branch, timestamp, or clean dirty-state claim.

## 3. Debug Bundle Shape

Minimum metadata-safe fields:

```text
bundleId
schemaVersion
createdAt
build: { commit, branch, appVersion, buildTimestamp, dirtyState }
environment: { providerPreflight, network, scheduler, workspaceRootDigest, safePathsDigest, mcpSummary, databaseSummary }
scenario?: { scenarioId, reviewerId, status, notesDigest }
task: { chatSessionId, taskSessionId, runId, strategy, status }
route: { routeType, provider, model, localOnly, liveProviderAttempted }
timeline: [ route_decision | plan | action | observation | blocker | proposal | final_delivery ]
tools: { candidateCount, selectedTool, actionType, targetDigest, policyDecision }
context: { activeMemoryIds, excludedMemoryIds, knowledgeAssetIds, selectedSkillId, contextSourceDigests }
memory: { proposalIds, acceptedMemoryIds, rolledBackMemoryIds, managedKnowledgeVersionIds }
finalDelivery: { completedWork, durableChanges, pendingUserActions, skippedWork, blockers }
failure: { class, severity, recoverability, recoveryRecommendation }
redaction: { mode, rawContentIncluded, secretsDetected, unsafeFieldsDropped }
```

The implementation can use typed structs rather than this exact text shape, but
the product meaning must be preserved.

## 3.1 UI Evidence Boundary

UI evidence is useful only when it is correlated with backend runtime evidence.

Any UI-state debug evidence must include:

- frontend route or surface name;
- visible control/state labels;
- task session id and, when available, backend snapshot id;
- timestamp;
- optional screenshot or DOM evidence digest, not raw private content.

UI evidence may prove that a state or control was visible or missing. It must
not by itself prove that an action executed, a provider was ready, a memory was
used, or a rollback succeeded.

## 4. Required UI States

| State | Required UI behavior |
| --- | --- |
| Preflight ready | Shows build, provider, network, workspace, MCP, and database status. |
| Preflight blocked | Shows named blockers and exact setup action without exposing secrets. |
| Task debuggable | Current or selected task has exportable bundle. |
| Export generating | User sees export in progress and target object ids. |
| Export ready | User sees bundle id, task id, scenario id, redaction mode, and copy/save action. |
| Export blocked | User sees why export is unsafe or impossible. |
| Failure classified | User sees failure class, severity, and recovery recommendation. |
| Issue report draft | User can add scenario id/reviewer/status/notes before exporting. |
| Issue report saved | User sees artifact id, storage alias, digest, and byte size, and can attach it to manual dogfood later. |

## 5. Product Rules

- A debug bundle must not be generated from chat text alone.
- A failure classification must not be generated from model prose alone.
- A bundle may include bounded previews, but raw prompts/responses/memory/files
  are off by default.
- Provider key presence may be exported as boolean only.
- Final delivery must distinguish completed, proposed, blocked, skipped, and
  pending work.
- Stage 5 report must include `notAReadinessGate=true` and
  `readinessClaim=false`.
- Stage 5 may export issue artifacts, but those artifacts are not S2-D manual
  dogfood evidence until reviewed and validated by the Stage 2 contract.
- Task-attached issue reports must include task session id and run id.
  Preflight-only or environment-blocked reports may omit task/run ids only with a
  named blocker and an explicit missing task/run reason, and cannot be marked as
  task behavior pass.
- Required identity/evidence fields must not be dropped during redaction. If a
  required field is unsafe, the artifact is blocked rather than silently
  weakened.

## 6. Required Commands Or APIs

Names are illustrative. Implementation should follow existing naming style:

- `evaluate_main_chat_stage5_release_debug_preflight`
- `export_main_chat_agent_debug_bundle`
- `create_main_chat_internal_issue_report`
- `list_main_chat_debug_bundles`
- `get_main_chat_debug_bundle`
- `delete_main_chat_debug_bundle` or equivalent retention-prune command
- `list_main_chat_internal_issue_reports`
- `get_main_chat_internal_issue_report`
- `delete_main_chat_internal_issue_report` or equivalent retention-prune command
- `run_main_chat_stage5_release_debug_report`

## 7. Non-fake Rules

- Do not show "provider ready" unless preflight proves it.
- Do not show "tool executed" unless an action/observation transcript exists.
- Do not show "memory used" unless context inventory or transcript evidence
  references active memory ids.
- Do not show "rollback worked" unless Stage 4 rollback/context evidence exists.
- Do not show "issue report saved" unless a local artifact exists or the command
  returns a durable artifact id, storage alias, digest, and byte size.
- Do not show "ready for internal trial" from Stage 5.
