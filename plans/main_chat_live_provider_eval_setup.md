# Main Chat Live Provider Eval Setup

> Date: 2026-06-25
> Status: preparation artifact before external live-provider validation
> Parent: `plans/main_chat_next_6_steps_master_spec.md`

## 1. Purpose

This document defines the local setup required before Step 2 can run real
external live-provider acceptance. It prevents local, scripted, fixture, or
synthetic evidence from being mistaken for external live-provider completion.

## 2. Current Baseline

Code-confirmed current state:

- `main_chat_live_provider_eval_opt_in_from_env` requires
  `OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL` to be set to `1`, `true`, `yes`, or
  `on`.
- live-provider preflight checks provider identity, API key presence, network
  policy, explicit opt-in, scripted-provider response, and local-only policy.
- command-surface evidence initializes live-provider coverage to `0.0` and
  final-completion blockers include missing live-provider scenarios.
- the external live-provider final acceptance test is marked `#[ignore]`; it
  must be selected with `--ignored`, and then still requires opt-in, network, and
  a real external provider API key.

## 3. Required Local Environment

Use environment variables only. Do not write secrets into tracked files.

Required:

```bash
export OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1
export OPENLIFE_LIVE_EVAL_PROVIDER=openai
export OPENLIFE_LIVE_EVAL_BASE=https://api.openai.com/v1
export OPENLIFE_LIVE_EVAL_MODEL=...
export OPENLIFE_LIVE_EVAL_API_KEY=...
```

These names are code-confirmed in the ignored live-provider tests and final
acceptance live setup helper. Do not replace them with generic provider config
names unless the code is changed in the same slice.

Provider-specific runtime key variables such as `OPENAI_API_KEY` may still be
used by normal app configuration, but the current ignored live-provider eval
state copies `OPENLIFE_LIVE_EVAL_API_KEY` into `config.llm.openai_key` before
building the scheduler. The required rule is that the live harness must classify
the provider endpoint as `external_provider`, not `scripted`,
`local_test_http`, `local`, `fixture`, or `synthetic`.

Network policy must be enabled in the app config used by the isolated eval
state. If network is disabled, the expected result is a fail-closed preflight
blocker, not partial credit.

## 4. Scenario Requirements

The opt-in suite must produce four scenario reports:

| Scenario | Required credit |
| --- | --- |
| DirectAnswer | direct provider generation trace, external provider identity, model identity, run id, task session id, normalized response preview |
| WebAgentLoop | provider-backed governed web action, succeeded AgentLoop, no single-step fallback, no overlapping MCP/proposal trace |
| RegisteredMcpAgentLoop | multi-candidate registered MCP set, provider-ranked selection, selected candidate target match, succeeded AgentLoop |
| McpToolPermissionProposal | selected MCP candidate target matches ToolPermission proposal, action status needs confirmation, no read-success overlap |

## 5. Commands

Preflight without invoking external provider:

```bash
cargo test -p openlife-tauri main_chat_live_provider -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance_gate_runner_fails_closed_without_live_provider_opt_in -- --nocapture
```

Opt-in external live run:

```bash
OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1 \
cargo test -p openlife-tauri main_chat_final_acceptance_gate_runner_accepts_external_live_provider_when_opted_in -- --ignored --nocapture
```

Full final gate after live run:

```bash
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
```

## 6. No-Credit Conditions

The following must never receive external live credit:

- scripted scheduler responses;
- local HTTP harness;
- localhost, loopback, private-network alias, or local provider identity;
- mock, fixture, synthetic, or test provider labels;
- malformed provider/model/run/task labels;
- response previews with control characters, wrapping whitespace, or unbounded
  content;
- provider-ranked MCP evidence without complete ranked candidate permutation;
- web evidence that overlaps MCP read-success or ToolPermission proposal traces;
- ToolPermission proposal evidence that also claims MCP read success.

## 7. Secret Handling

- Do not echo API keys in terminal output.
- Do not put keys in `.env`, config, docs, screenshots, or test fixtures unless
  the file is ignored and intentionally local.
- Do not include provider response bodies in docs. Use bounded previews only.
- Run a diff-scoped secret scan before commit if setup changes touch code or
  docs.

Example diff-scoped scan:

```bash
git diff -U0 | rg -n "^\\+.*(sk-[A-Za-z0-9_-]{20,}|OPENAI_API_KEY\\s*=|ANTHROPIC_API_KEY\\s*=|api[_-]?key\\s*[:=])" || true
```

## 8. Failure Classification

If the opt-in live suite fails, classify the failure before editing code:

- `setup_blocker`: missing key, network disabled, no opt-in, unsupported
  provider identity;
- `provider_blocker`: external API unavailable, auth rejected, rate limited;
- `harness_blocker`: report lacks required trace fields even though provider
  invocation happened;
- `agent_blocker`: Main Chat path invoked provider but failed governed action,
  selected unsafe tool, used single-step fallback, or wrote silently;
- `gate_blocker`: evidence exists but final gate rejects it due to contract
  mismatch.

Only `agent_blocker`, `harness_blocker`, or `gate_blocker` should normally
produce code changes.
