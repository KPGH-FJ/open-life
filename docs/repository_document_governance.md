# Repository Document Governance

## Keep Tracked

- public entrypoints: `README.md`, `PRODUCT.md`, `AGENTS.md`;
- stable architecture and development documents under `docs/`;
- accepted decisions under `docs/decisions/` and `plans/adr/`;
- at most one current implementation plan.

## Keep Local

Private notes, raw audits, temporary reports, prompts, personal strategy, and
unpublished PRDs belong in ignored local paths such as `agent-notes/` or files
ending in `.local.md` and `.scratch.md`.

## Rules

- Documents must be intentionally public and free of secrets or personal data.
- Do not duplicate current status across multiple files.
- Do not retain completed plans as active working-tree authority.
- Do not create machine-readable ledgers or validators merely to govern
  documentation.
- Git history is the archive for ordinary superseded material.
- History rewriting is reserved for actual secret or privacy removal.

When a document disagrees with source, verify the source, correct the document,
and keep one authority.
