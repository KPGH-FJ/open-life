# ADR 0009: ExecutionSandbox and Bash/Shell Boundary

Date: 2026-05-06
Status: proposed

## Context

Bash and shell execution are powerful Agent Framework capabilities. They are also high risk. OpenLife should not introduce shell execution before it has explicit sandbox policy, deny-read rules, timeout/output limits, environment controls, and trace.

## Decision

Introduce `ExecutionSandbox` before any Bash/Shell executor. Bash is default-off.

## Sandbox Policy

Required fields:

- cwd
- safe_paths
- deny_read_patterns
- deny_write_patterns
- network_policy
- timeout_ms
- max_output_bytes
- env_allowlist
- command_allowlist
- dangerous_command_denylist

## Bash Availability

Initial default:

- disabled in normal chat
- disabled for sub-agents
- disabled for scheduled/proactive tasks
- only enabled behind explicit setting and user-triggered execution

## Permanent Deny Examples

- reading private keys
- reading `.env` unless explicitly scoped and redacted
- destructive filesystem commands outside safe paths
- process-killing broad commands
- privilege escalation
- shell command composition that bypasses allowlist

## Implementation Guardrails

- Bash must go through ToolRuntime.
- Every shell attempt records AgentRunEvent.
- Writes are proposal-first unless an accepted policy says otherwise.
- Shell does not inherit full user environment.
- Output is truncated and stored safely.

## Verification

Tests should prove:

- shell disabled by default
- command allowlist enforced
- dangerous command blocked
- deny-read blocks secret paths
- timeout works
- max output limit works
- env allowlist works

## Open Questions

1. Should OpenLife ever expose interactive shell sessions?
2. Should scheduled tasks be allowed to use shell after explicit user approval?
3. What should be the default safe path set?
