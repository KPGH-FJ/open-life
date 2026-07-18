# Industry Interaction Pattern Study

Status: `REFERENCE_NOT_AUTHORITY`
Goal: reuse proven interaction grammar without copying another product's
business model, brand, or unsupported capability claims.

## 1. Sources

Primary references:

- user-supplied Cursor Settings screenshot from 2026-07-18;
- [OpenAI: Introducing the Codex app](https://openai.com/index/introducing-the-codex-app/);
- [OpenAI: Work with Codex from anywhere](https://openai.com/index/work-with-codex-from-anywhere/);
- [Cursor Agent overview](https://docs.cursor.com/en/agent/overview);
- [Cursor Agent tools](https://docs.cursor.com/en/agent/tools);
- [Cursor permissions reference](https://docs.cursor.com/cli/reference/permissions);
- [Cursor privacy](https://docs.cursor.com/account/privacy);
- [GitHub review proposed changes](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/reviewing-proposed-changes-in-a-pull-request);
- [GitHub deployment review](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/review-deployments);
- [Google OAuth authorization best practices](https://developers.google.com/identity/protocols/oauth2/web-server).

The screenshot is visual evidence only. Official docs are used for product
behavior claims. Neither source overrides OpenLife's backend contract.

## 2. Cursor Settings Grammar

Observed in the user-provided screenshot:

- Settings replaces the work surface with a dedicated settings context.
- A narrow left settings sidebar contains Back, search, stable categories, and
  account/plan utilities.
- The content column is long-form and row-based rather than a grid of summary
  cards.
- Section headers disclose groups such as Models and API Keys.
- Toggles control whether subordinate inputs are relevant.
- API keys, base URLs, model names, and provider-specific fields stay near the
  provider they affect.
- Technical controls use low-contrast separators, restrained radius, and no
  decorative illustration.

OpenLife adopts the structure but changes the semantics:

- provider selection cannot imply privacy;
- a toggle cannot bypass network policy or review;
- secrets are masked and search never indexes secret values;
- connection testing can create a permission/review step;
- saving config must be followed by refreshed provider/privacy truth;
- advanced and dev-only surfaces remain visually and structurally secondary.

## 3. Codex Workbench Grammar

The official Codex material emphasizes project/thread context, visible work,
reviewable changes, tests/diffs, secure defaults, and explicit permission for
elevated actions. The portable principles are:

1. one current task owns the main work surface;
2. execution progress is visible without dumping raw logs;
3. diffs and evidence precede approval;
4. waiting for user input is a first-class task state;
5. approvals change authorization, not historical facts;
6. terminal output, screenshots, diffs, and tests are evidence objects, not
   decorative status copy.

OpenLife extends this grammar with LifeModel, memory, privacy, and proposal-first
durable state rules. It must therefore be stricter than an IDE agent whenever a
change affects personal truth or external transmission.

## 4. Review And Permission Grammar

GitHub's useful sequence is `inspect change -> understand impact -> decide`.
Deployment review also distinguishes approval/rejection from the later
execution state. Google authorization guidance supports narrow, contextual,
on-demand scope.

OpenLife adopts:

- one-sentence proposed outcome;
- current-to-proposed diff;
- reason, source, risk, affected object, expiry;
- permission-specific tool, target, capability, input digest, transmission
  boundary, one-time scope, and what happens next;
- fixed decision bar after the evidence;
- separate `decision`, `application`, and refreshed `current truth` rows.

## 5. Patterns Adopted

| Pattern | Source inspiration | OpenLife use |
|---|---|---|
| Dedicated settings context | Cursor | Back + search + category sidebar + one content column |
| One current task | Codex/Cursor | Workspace is current execution; Tasks owns continuity/history |
| Compact execution timeline | Codex/Cursor | human-readable action/observation/result rows |
| Diff before approval | GitHub/Codex | Review proposal detail |
| Contextual permission | Cursor/Google | exact one-time action approval at the blocker |
| Evidence on demand | Codex | right Inspector / mobile bottom sheet |
| Explicit status after dispatch | Codex/GitHub | refreshing, applying, remote unknown, failed, completed with evidence |
| Low-noise white workbench | Cursor/Codex | selected Phase 3D tokens and component language |

## 6. Patterns Rejected Or Constrained

| Pattern | Why it is not copied directly |
|---|---|
| Model toggle equals availability | OpenLife provider route depends on validation, policy, runtime coherence, and transmission evidence. |
| Auto-run as a general preference | OpenLife requires capability-, risk-, action-, and scope-specific governance. |
| Raw terminal as primary progress | OpenLife users need human-readable personal/task context; raw evidence is advanced. |
| Broad folder permission inferred from a label | Exact action-bound scope is backend-owned and must match the blocked action. |
| IDE project/file mental model everywhere | OpenLife also owns personal state, memory, plans, review, and privacy boundaries. |
| Brand or dark-theme cloning | User selected the white grammar, not another product's identity. |
| “Connected” equals “private” | Cursor's own privacy model demonstrates that provider/backend routing and privacy are separate concerns. |

## 7. Consistency Rules

Across Today, Workspace, Tasks, Review, LifeModel, and Settings:

- one near-black primary action per decision point;
- amber means waiting, unknown, stale, or protective restriction;
- red means a concrete error, rejected dispatch, or blocked destructive action;
- green appears only for current, verified success or `not_sent` local evidence;
- 1px neutral dividers and low radius;
- 14px control/body text, 15px reading text, at least 12px metadata;
- ordinary Chinese labels on the product surface; ids and enum names only in
  Inspector technical disclosure;
- no card wall, no decorative gradients, no oversized dashboard numbers.

## 8. Study Decision

The project should imitate the proven interaction discipline of Cursor and
Codex, not their feature claims. OpenLife's differentiator is the governed
relationship among task execution, evidence, permission, personal truth, and
materialization. Any borrowed pattern that weakens that relationship is
rejected.
