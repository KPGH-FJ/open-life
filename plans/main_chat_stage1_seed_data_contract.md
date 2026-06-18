# Main Chat Stage 1 Seed Data Contract

> Date: 2026-06-18
> Scope: deterministic data required for Stage 1 dogfood scenarios
> Status: preparation artifact

## 1. Purpose

Stage 1 dogfood must run against realistic but non-private data. Empty-state
tests are not enough because memory, context, sessions, events, permissions,
plans, and final delivery all need existing objects.

Seed data must be deterministic, local, metadata-safe, and disposable.

## 2. Seed Workspace Layout

The implementation should create a test-only dogfood workspace under an isolated
temporary directory, not under a user's real OpenLife data directory.

Required files:

```text
dogfood_workspace/
  AGENTS.md
  SOUL.md
  USER.md
  MEMORY.md
  project_brief.md
  planning_notes.md
  policy_note.md
  memories/
    USER.md
    MEMORY.md
  skills/
    phase_e_review/
      SKILL.md
    planning_review/
      SKILL.md
    unselected_sensitive/
      SKILL.md
```

Required properties:

- no API keys;
- no real personal data;
- no private URLs;
- all files small enough to avoid context truncation unless a scenario
  intentionally tests truncation;
- every file has a stable digest recorded in the seed manifest.

## 3. Runtime Seed Objects

The isolated eval state must seed:

- one active accepted memory preference;
- one conflicting memory pair with evidence ids;
- one pending memory proposal;
- one accepted memory record eligible for rollback;
- one existing chat session with memory/rollback discussion;
- one blocked task waiting for exact permission;
- one failed read action eligible for retry;
- one non-terminal task with queued action eligible for cancel;
- one terminal task with completed/proposed/blocked/skipped work;
- one PlanExecute session with draft, revision, executable step, unsupported
  step, and review state;
- one registered read-only MCP manifest set;
- one missing MCP target scenario;
- one web fixture source;
- one network-disabled policy state.

## 4. Seed Manifest

The implementation should expose a structured seed manifest in the Stage 1
report.

Required fields:

- `seedWorkspaceRootKind`: `temp_isolated`;
- `knowledgeAssetCount`;
- `skillCount`;
- `sessionSeedCount`;
- `memorySeedCount`;
- `proposalSeedCount`;
- `taskSeedCount`;
- `planSeedCount`;
- `mcpManifestSeedCount`;
- `webFixtureSeedCount`;
- `seedDigest`;
- `secretsDetected`: false.

Digest rules:

- `seedDigest` is `sha256` over a canonical JSON manifest.
- Manifest keys must be sorted lexicographically.
- File paths must be POSIX-style paths relative to the seed workspace root.
- File digests must be `sha256` over raw file bytes.
- Runtime seed object digests must exclude volatile ids and timestamps unless
  the scenario explicitly tests those fields.
- The report must include both `seedDigest` and per-file/per-object digest
  labels in `bytes:<n> hash:sha256:<64-hex>` form.

## 5. Reset And Isolation

Each scenario must either:

- create a fresh isolated state, or
- use a scenario-specific namespace inside a shared isolated state.

No scenario may depend on execution order unless it is explicitly declared as a
multi-step scenario group. A failed scenario must not poison later scenarios.

## 6. Negative Seed Checks

The seed setup must fail closed if:

- any seed file contains an API-key-like pattern;
- canonical file path escapes the seed workspace;
- a selected skill id does not match a seeded skill;
- an unselected skill body appears in prompt/context evidence;
- local/mock provider identity is used as external live evidence;
- a write-like MCP manifest is exposed as read-only.
