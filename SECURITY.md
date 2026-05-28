# Security Policy

OpenLife is local-first and handles sensitive personal context. Security and
privacy issues should be treated with extra care even before public release.

## Sensitive Data Policy

Do not include the following in public issues, PRs, logs, screenshots, test
fixtures, or diagnostics:

- API keys or tokens,
- raw LifeModel content,
- raw memory records,
- raw private file contents,
- raw sensitive chat,
- complete prompts containing private context,
- full model outputs containing private context.

Use redacted summaries, source references, hashes, digests, or metadata-only
audit records instead.

## Reporting

For now, report sensitive security or privacy issues privately to the repository
owner instead of opening a public issue. Public issues should contain only
redacted reproduction steps and metadata.

## LifeModel-HS Security Expectations

LifeModel-HS work must preserve:

- Proposal-first mutation for risky changes,
- privacy as hard Policy,
- no heuristic relaxation of privacy Policy,
- metadata-safe audit and regression records,
- no one-step source-of-truth migration,
- no automatic high-risk LifeModel updates.

See:

- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `plans/lifemodel_hs_mvp_task_specs.md`
