# Repository Document Governance

This document defines what OpenLife documentation belongs in the public GitHub
repository and what must stay local-only.

It complements `docs/github_repository_governance.md`, which governs GitHub
issues, PRs, branches, and CI. This file governs the document surface itself.

## Goals

- Keep the public repository useful for future humans and Agents.
- Prevent private LifeModel, memory, chat, prompt, API, or product-draft material
  from being committed by default.
- Make old plans clearly historical so they cannot override current authority.
- Preserve enough architecture and decision context to resume development.

This document does not purge anything already pushed to GitHub history. If a
previously pushed document contains secrets or material that must never have
been public, use the history-rewrite workflow below.

## Document Classes

| Class | Git status | Examples | Rule |
| --- | --- | --- | --- |
| Public entry points | Tracked | `README.md`, `AGENTS.md`, `plans/README.md` | Must stay concise, current, and authority-labeled. |
| Stable product, architecture, and development docs | Tracked | `docs/ARCHITECTURE.md`, `docs/architecture/*.md`, `docs/development/testing.md`, `docs/BETA_USER_GUIDE.md`, `docs/decisions/*.md` | Public by intent; include status when stale or scoped. |
| Current execution plans | Tracked, limited | One current goal spec per active area | Track only when it is the active or accepted execution source. |
| Historical plans and PRDs | Tracked archive or removed from tracking | old PRDs, old beta checklists, superseded plans | Must be clearly labeled historical/scoped; do not update as live status logs. |
| Local/private planning | Not tracked | AI scratch notes, private PRDs, raw handoff notes, personal strategy | Keep under ignored local-only paths. |

## Local-Only Material

Use reviewed local-only locations or filename suffixes for drafts and private
material that should not be pushed. The docs/plans private, local, and draft
namespace families are reserved policy labels; Stage4B does not materialize
them as empty directories.

- `ai-notes/`
- `agent-notes/`
- `prd-drafts/`
- files ending in `.private.md`, `.local.md`, or `.scratch.md`

Anything in those locations is local working material unless explicitly moved
into a tracked public location after review.

## Public Document Rules

Before committing a Markdown or HTML document, check all of the following:

- The document is intentionally public.
- It has a clear status when it is historical, scoped, draft, or non-authority.
- It does not contain raw LifeModel, raw memory, sensitive chat, personal files,
  full private prompts, credentials, API keys, private provider endpoints, or
  unpublished personal/commercial strategy.
- It does not duplicate current status that should live in `AGENTS.md`,
  `plans/README.md`, or the current goal spec.
- It has a clear owner surface: entry point, architecture reference, ADR,
  current plan, or archived reference.

Default rule: AI-generated planning notes start local-only. Promote them to the
tracked repository only after a publication review.

## Compression Rules

Avoid turning public docs into append-only execution logs.

- Keep `README.md` focused on product definition, setup, and current user-facing
  status.
- Keep `AGENTS.md` focused on agent constraints, architecture boundaries,
  commands, and active caveats.
- Keep `plans/README.md` as an authority map and compressed status index.
- Put deep proof details in the active goal spec or an archived reference, not
  in every entry point.
- When a plan is complete or superseded, add a short historical status note
  instead of continuing to update it.

## Cleanup Workflow

Use this order when cleaning the repository:

1. Inventory documents with `rg --files -g '*.md' -g '*.html'`.
2. Classify each document as public entry point, stable reference, current plan,
   historical reference, or local/private.
3. For ordinary stale public docs, either archive them in a tracked location or
   remove them in a normal commit.
4. For documents that should stop being tracked but do not require history
   removal, move them to a local-only path and run `git rm --cached <path>` if
   the file should remain on disk.
5. For secrets or material that must be removed from GitHub history, stop and
   perform a dedicated history rewrite.

## History-Rewrite Workflow

Removing a file in a new commit does not remove it from GitHub history. Use a
history rewrite only when the already-pushed content is sensitive enough to
justify force-pushing and coordinating all clones.

Minimum workflow:

1. Rotate any exposed credentials first.
2. Make a fresh backup clone.
3. Use `git filter-repo` or an equivalent reviewed tool to remove the paths or
   replace sensitive text.
4. Inspect the rewritten history locally.
5. Force-push with lease.
6. Ask every clone/fork owner to re-clone or hard-reset to the rewritten branch.
7. If the repository was public, assume the material may already have been
   copied outside GitHub.

Do not mix history rewriting with ordinary cleanup PRs.

## Current Repository Notes

As of 2026-07-07 Stage4B, the current working-tree document baseline is:

- 204 Markdown/HTML files in the non-hidden
  `rg --files -g '*.md' -g '*.html'` scope.
- 191 tracked Markdown/HTML files in `git ls-files '*.md' '*.html'`, including
  hidden GitHub governance docs outside the `rg --files` default scope.
- 14 currently untracked Markdown files in the `rg --files` scope, all from the
  repository cleanup / Stage3-Stage4 documentation work.
- 17 Markdown files under `docs/`, with `docs/architecture/`,
  `docs/development/`, and `docs/decisions/` present. The product-doc namespace
  is still absent by design.
- 169 Markdown/HTML files under `plans/` and 173 plan Markdown/HTML/JSON files
  in the current `rg --files` scope. `plans/adr/` remains the only plans
  subdirectory; no plan-archive namespace move has happened.

Stage3-B / Stage4A classification:

- `docs/ARCHITECTURE.md` is a Stage3-A architecture index and historical
  pointer, not a runtime authority.
- `docs/architecture/agent-runtime.md`,
  `docs/architecture/life-model.md`, `docs/architecture/governance.md`, and
  `docs/architecture/memory.md` are public, stable, source-backed explanatory
  docs beneath the active authority stack.
- `docs/development/testing.md` is a public, stable validation-command
  explainer. It does not convert local/scripted evidence into live-provider
  completion evidence.
- `docs/decisions/README.md` is the Stage4A ADR index. It records ADR 0001,
  0002, 0003, and ADR 0013 without moving ADR 0013.
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` remains the
  canonical ADR 0013 path by Stage4A no-move decision.
- `plans/openlife_repository_document_inventory.json` and
  `plans/openlife_repository_document_link_baseline.json` are the current
  Stage4B inventory and link/path baseline artifacts.

Recommended next cleanup pass:

- Keep public entry points and stable explanatory docs tracked.
- Keep ADR 0013 at its Stage4A no-move canonical path; any future ADR 0013 move
  needs a separate reviewed slice.
- Keep plan-archive namespace creation and broad plan moves deferred until link
  impact and authority guards are reviewed.
- Keep new AI/product planning drafts local-only until explicitly promoted.
