# Main Chat Stage 5 Debug Privacy And Redaction Contract

> Date: 2026-06-20
> Stage: Stage 5 - Internal Trial Release and Debug Operations
> Status: preparation contract

## 1. Principle

Debuggability cannot come at the cost of leaking private data. Stage 5 exports
must be metadata-safe by default and explicit about any optional raw-content
mode.

## 2. Default Export Mode

Default export mode is `metadata_safe`.

Allowed by default:

- ids: task/session/run/action/proposal/memory/bundle/report ids;
- low-cardinality labels: strategy, status, failure class, policy decision;
- booleans: key present, network enabled, local-only, redaction applied;
- digests: prompt, response, file, memory, context, notes, safe paths;
- bounded normalized previews where already allowed by existing contracts;
- counts: transcript entries, actions, proposals, memory ids, context assets;
- timestamps;
- build commit/branch/version.

Blocked by default:

- API keys and auth headers;
- full system prompts;
- full user prompts;
- full assistant responses;
- full transcripts;
- raw private memory;
- raw LifeModel materialized view content;
- full `USER.md`, `MEMORY.md`, `SOUL.md`, `AGENTS.md`, or `SKILL.md`;
- provider request/response bodies;
- raw tool arguments/results when they include user content or file content;
- absolute private paths unless already surfaced as safe workspace paths and
  approved by the existing resolver policy.

## 3. Preview Rules

Any preview included by default must be:

- bounded by a fixed character limit;
- single-line or normalized with escaped newlines;
- free of control characters;
- stripped of leading/trailing whitespace;
- scanned for key/token/secret patterns;
- accompanied by a digest of the full original source;
- marked with `truncated=true` when applicable.

## 4. Optional Raw-content Mode

Raw-content export is out of scope for Stage 5 default acceptance. If added as
an optional future mode, it must require:

- explicit user confirmation;
- visible list of raw fields to include;
- local-only export target;
- no automatic upload;
- separate tests proving default mode remains metadata-safe.

## 5. Redaction Report

Every debug bundle must include:

```text
redactionMode
rawContentIncluded
secretsDetected
unsafeFieldCount
unsafeFieldsDropped
previewLimit
promptDigest
responseDigest
contextDigest
```

The bundle must fail closed if a known secret pattern remains in any exported
string field.

Before any bundle or issue report is written to disk, the final serialized
artifact must be recursively scanned across all string fields. If the recursive
scan finds a known secret pattern, raw private content, unapproved absolute
private path, control character, or raw-content field in `metadata_safe` mode,
the artifact write must fail closed or drop the unsafe field before retrying.

Dropping unsafe fields is allowed only for optional preview or notes fields. If
the unsafe field is required identity or evidence, the artifact must be blocked
instead of silently dropping the field. Required identity/evidence includes at
least schema version, bundle/report/artifact id, build commit or its named
blocker, scenario id when supplied, reviewer id when supplied, task session id
for task-attached reports, run id for task-attached reports, failure class,
redaction mode, artifact digest, and created timestamp.

## 6. Special Data Rules

| Data | Default handling |
| --- | --- |
| Provider key | Boolean presence only. |
| Provider identity/model | Metadata-safe raw label, same constraints as live-provider gate. |
| Prompt | Digest and optional bounded preview only. |
| Response | Digest and optional bounded preview only. |
| Tool arguments | Digest plus safe target/action labels; no raw file/user content. |
| Tool results | Observation digest and bounded preview if policy permits. |
| Memory | Memory ids, status, scope/category/risk/confidence, digest; no raw content unless explicit. |
| Knowledge files | Asset id/path/source/digest/truncation/reason; no full content. |
| Tester notes | Digest plus optional bounded preview after secret scan. |
| File paths | Workspace-relative when possible; absolute paths only if already allowed by resolver policy. |

## 7. Acceptance Rules

- Unit tests must inject fake keys/tokens and prove they are not exported.
- Export must fail or drop only optional unsafe preview/notes fields when unsafe
  strings are detected.
- Redaction must be applied before writing any artifact.
- The final serialized artifact must pass recursive string-field scanning before
  atomic write.
- Required identity/evidence fields must never be dropped to make an artifact
  pass; unsafe required fields make the artifact blocked/fail-closed.
- `metadata_safe` bundle must be sufficient for DBG5 evaluation.
- Stage 5 must not require raw-content export to pass.
