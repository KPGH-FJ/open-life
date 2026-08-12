# Current OpenLife Product Development Plan

Status: complete

## Objective

Complete S3: make the first canonical report Task use a real governed local-
document and Web evidence loop before provider synthesis and Review. Ordinary
knowledge-work language must reach the path without requiring internal tool
names, while every source remains bounded, cited, replayable, and fail-closed.

## Product path

```text
user outcome + bound local documents
  -> canonical Plan
  -> governed document.read ToolCall -> bound Observation
  -> governed web.search / web.fetch ToolCall -> bound Observation
  -> provider synthesis from those exact evidence blocks
  -> cited Markdown ArtifactDraft
  -> ReviewCheckpoint
  -> materialization -> Verification -> FinalResult
```

A report may use only local documents, only Web evidence, or both. When the
user requests a source, missing or failed evidence blocks synthesis; OpenLife
must not silently answer from model knowledge or create a Review proposal.

## In scope

1. Add one bounded production `document.read` capability over resources already
   imported and bound to the current user message/task operation.
2. Persist its exact ToolCall and Observation as canonical Items before
   ProviderGeneration, using metadata-safe identities and digests rather than
   document bodies.
3. Compose document reads with existing governed `web.search` and `web.fetch`
   reads in one deterministic report Run and one evidence contract.
4. Accept ordinary Chinese and English report delegation language without
   requiring users to type `file.read`, `web.search`, or other internal names.
5. Require provider output to cite the request-scoped local-resource and Web
   citation authorities; backend code renders filenames, provenance, and URLs.
6. Preserve exact replay, cancellation fences, provider/model binding, Review,
   ArtifactVersion verification, and canonical FinalResult from S2.
7. Keep local documents as untrusted evidence. Embedded instructions cannot
   expand tool, write, Memory, LifeModel, provider, or network authority.

## Out of scope

- arbitrary filesystem discovery, directory crawling, OCR expansion, or shell;
- new connectors, email, calendar, browser/computer use, or subagents;
- S4 steering, inline approval continuation, recovery redesign, or concurrency;
- S5 Results/Changes/Preview UI redesign;
- provider auto-routing, cross-provider fallback, or background autonomy;
- Memory or LifeModel learning changes;
- deletion of unrelated compatibility owners before their roadmap stage.

## Ownership

- `ResourceStore` and the bounded resource selector own imported document bytes,
  chunks, provenance, and request-scoped local citation issuance.
- ToolGateway/AgentRun evidence owns the actual governed read execution fact.
- `CanonicalTaskRuntimeStore` owns the report Task/Run/Item ordering and binds
  only exact document/Web observation digests.
- Provider output never owns source identity or provenance. Backend citation
  authorities validate citations and render source footers.
- ReviewWorkflow and ArtifactMaterializer retain their S2 responsibilities;
  neither can turn missing read evidence into completion.

## Acceptance

| Scenario | Required result |
| --- | --- |
| Bound document only | one document ToolCall/Observation before ProviderGeneration; cited Markdown draft |
| Web only | governed Web ToolCall/Observation before ProviderGeneration; cited Markdown draft |
| Bound document plus Web | both exact evidence pairs in deterministic execution order; one report Task |
| Ordinary user language | report path works without internal tool-name phrases |
| Multiple bound documents | bounded selection covers relevant files and preserves per-file provenance |
| Missing requested document | no provider synthesis, ArtifactDraft, or Review proposal |
| Empty/failed resource extraction | blocked with no model-knowledge fallback |
| Web permission/challenge/failure | blocked before synthesis and Review |
| Forged or missing local citation | one bounded retry, then blocked with no ArtifactDraft |
| Forged or missing Web citation | one bounded retry, then blocked with no ArtifactDraft |
| Embedded source instruction | treated only as data; no authority expansion or durable write |
| Exact replay/restart | no duplicate read Items, provider generations, proposals, or effects |
| Verified acceptance | same S2 ArtifactVersion reaches Verification and FinalResult |
| Product read model | exposes canonical document/Web Items and never infers missing evidence |

## Checks

```sh
git diff --check
cargo fmt --check
cargo clippy --all --locked -- -D warnings
cargo test --all --locked
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend test
corepack pnpm --dir frontend build
corepack pnpm --dir frontend test:e2e
```

## Stop condition

S3 is complete only when the production report path records real governed
document and Web evidence through the canonical Task owner, ordinary delegation
language reaches it, all negative cases fail before synthesis/Review, full
gates pass, source and stable docs agree, commits are reviewable, and the
working tree is clean. Native and external-live evidence remain S6 unless an
S3 contract cannot be proved below that evidence level.

## Closure

- Production `document.read` now uses the governed ToolGateway path over exact
  task-bound resources; document-only, Web-only, and combined reports share one
  canonical Task/Run/Item lifecycle.
- Missing, failed, or uncited requested evidence stops before ArtifactDraft and
  Review. Citation repair is bounded to one provider retry and never redispatches
  reads.
- Replay and backend read models expose committed document/Web Items without
  duplicating provider generations, proposals, or effects.
- Rust and frontend format, lint, test, production-build, absence, and browser-
  shell gates passed on the closing working tree. Native and external-live
  evidence remain explicitly assigned to S6.
- S4 has not started; this S3 closure is ready for product and code review.

## Next pointer

After S3 review, begin S4: steering, inline approval continuation, recovery,
and controlled concurrency on this same canonical Task lifecycle.
