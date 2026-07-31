# Security Policy

OpenLife handles sensitive personal context and local credentials.

## Never Commit

- API keys, tokens, or passwords
- raw LifeModel or memory records
- private files, chats, prompts, or model outputs
- unredacted provider payloads
- application databases or Keychain exports

Use metadata-safe summaries, hashes, synthetic fixtures, and redacted
reproduction steps.

## Product Security Boundaries

- No silent durable writes.
- External or sensitive actions require confirmation or proposal flow.
- Missing policy, permission, or evidence fails closed.
- Local/scripted evidence does not prove external-live behavior.
- Keychain and application data must not be touched by ordinary tests.

The accepted source-of-truth and write-lane decisions are:

- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `plans/adr/0014-explicit-user-memory-write-lane.md`
- `plans/adr/0015-transient-state-command-lane.md`

Report sensitive vulnerabilities privately to the repository owner. Public
issues and pull requests must contain only redacted information.
