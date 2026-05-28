# GitHub Repository Governance

This document defines the GitHub-side operating model for OpenLife. It is
especially important for LifeModel-HS work, where issues and PRs are part of
the engineering control system.

## Repository Baseline

- Default branch: `main`
- Integration branch: `dev`
- Normal feature branch prefix: `codex/`
- Issue templates: `.github/ISSUE_TEMPLATE/`
- PR template: `.github/PULL_REQUEST_TEMPLATE.md`
- Label source of truth: `.github/labels.yml`
- Code ownership: `.github/CODEOWNERS`

## Recommended Labels

Configure repository labels from `.github/labels.yml`.

Minimum labels required before LifeModel-HS issues:

- `lifemodel-hs`
- `mvp`
- `epic`
- `needs-plan`
- `governance`
- `privacy`
- `proposal-first`
- `architecture`
- `backend`
- `rust`
- `test`
- `documentation`
- `blocked`
- `do-not-merge`

Dependabot labels:

- `dependencies`
- `github-actions`
- `frontend`

Triage labels:

- `bug`
- `engineering`
- `needs-triage`

## Recommended Milestones

Create these milestones manually in GitHub:

1. `LifeModel-HS MVP`
   - Scope: LMHS-1 through LMHS-10.
   - Exit gate: package criteria in `plans/lifemodel_hs_mvp_task_specs.md`.

2. `LifeModel-HS Foundation`
   - Scope: LMHS-1 through LMHS-5.
   - Exit gate: EvidenceStore, HeuristicStore, policy boundary, selectors,
     and deterministic regression exist with focused tests.

3. `LifeModel-HS Runtime Proof`
   - Scope: LMHS-6 through LMHS-10.
   - Exit gate: narrow runtime behaviors, negative evidence, YAML guardrails,
     trace visibility, and legacy path audit.

## Recommended Branch Protection

Protect `main`:

- Require pull request before merging.
- Require at least one review.
- Require CODEOWNERS review when available.
- Require status checks:
  - `Rust Check`
  - `Frontend Check`
  - `Smoke Test`
- Require branches to be up to date before merging when practical.
- Disallow force pushes.
- Disallow deletions.

Protect `dev`:

- Require pull request before merging for non-maintainer work.
- Require status checks:
  - `Rust Check`
  - `Frontend Check`
  - `Smoke Test`
- Disallow force pushes.

Do not require macOS, Windows, Rust coverage, or security audit checks for
every `dev` PR unless the repository has enough CI budget. Those checks already
run for `main` paths in `.github/workflows/ci.yml`.

## LifeModel-HS Issue Flow

Create issues in this order:

1. `Epic: LifeModel-HS MVP`
2. `LMHS-1: EvidenceStore MVP Skeleton`
3. After LMHS-1 plan review, implementation PR for LMHS-1.
4. Continue one task at a time.

For each LMHS task:

```text
issue -> plan-only Codex pass -> human review -> implementation -> focused tests -> PR -> review -> merge
```

The first Codex pass should be plan-only unless the issue explicitly says
implementation is approved.

## PR Review Rules

Reject or request changes when a PR:

- implements more than one LMHS task without explicit approval,
- changes source-of-truth semantics in one step,
- stores raw sensitive data in evidence, audit, regression, or selector output,
- relaxes privacy policy through a heuristic,
- bypasses Proposal-first governance,
- broadens runtime authority without an accepted ADR,
- rewrites large modules outside the issue scope,
- omits task-specific verification.

## Release Discipline

LifeModel-HS infrastructure and docs can merge first as a small setup PR.

After that:

- merge issue templates before opening the Epic,
- open Epic before child task issues,
- open LMHS-1 before creating implementation branches,
- keep each implementation PR linked to exactly one task issue.

## Manual Setup Checklist

- [ ] Merge GitHub infrastructure PR into `dev`.
- [ ] Merge infrastructure into `main` so issue forms appear in GitHub UI.
- [ ] Configure labels from `.github/labels.yml`.
- [ ] Create `LifeModel-HS MVP` milestone.
- [ ] Optionally create `LifeModel-HS Foundation` and `LifeModel-HS Runtime Proof` milestones.
- [ ] Configure branch protection for `main`.
- [ ] Configure branch protection for `dev`.
- [ ] Create `Epic: LifeModel-HS MVP`.
- [ ] Create `LMHS-1: EvidenceStore MVP Skeleton` with first pass set to plan-only.
