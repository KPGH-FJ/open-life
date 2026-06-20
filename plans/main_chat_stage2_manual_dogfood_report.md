# Main Chat Stage 2 Manual Dogfood Report

> Date: 2026-06-19
> Status: not attempted

## Summary

Stage 2 manual dogfood has not been completed yet.

The Stage 2 readiness gate expects the machine-readable artifact at:

```text
frontend/test-results/main-chat-stage2-manual-dogfood-report.json
```

Operators may start from the non-evidence JSON template at:

```text
plans/main_chat_stage2_manual_dogfood_artifact_template.json
```

That template is intentionally invalid readiness evidence: every row is
`not attempted`, uses placeholder ids, and includes `template_not_evidence`.
Copy its structure only. Before validation, replace every placeholder with real
reviewer/runtime values, set completed rows to the reviewer result, and remove
`template_not_evidence` from rows that are real evidence.

Reviewers or operators can validate that artifact without running the full
readiness gate by calling:

```text
validate_main_chat_agent_stage2_manual_dogfood_artifact
```

Until that artifact contains real reviewer records for every S2-D01 through
S2-D24 P0 scenario from at least two distinct reviewers, the readiness
recommendation must remain:

```text
not_ready_for_limited_internal_trial
```

## Required Reviewer Record Fields

Each machine-readable record must include:

| Field | Required value |
| --- | --- |
| `reviewerId` | Known metadata-safe reviewer id. `unknown`, `none`, and fake labels such as `mock-reviewer` are invalid placeholder evidence. |
| `buildCommit` | Metadata-safe git commit for the build under review. `unknown`, `none`, and fake labels such as `mock-build` are invalid placeholder evidence. |
| `providerMode` | Exact label `deterministic`, `live provider`, or `both`. Whitespace and alternate spellings such as `live_provider` or `live-provider` are invalid. |
| `scenarioId` | Required P0 `S2-D01` through `S2-D24`, or optional P1 `S2-D25` through `S2-D27`. |
| `prompt` | Exact prompt or short prompt summary used by the reviewer. |
| `taskId` | Main Chat task id. `unknown`, `none`, missing-trace placeholders, and fake labels such as `mock-task` are invalid placeholder evidence. |
| `runId` | Main Chat run id. `unknown`, `none`, missing-trace placeholders, and fake labels such as `mock-run` are invalid placeholder evidence. |
| `result` | One of `pass`, `fail`, `blocked`, `confusing`, or `not attempted`. |
| `severity` | One of `P0`, `P1`, or `P2`. Required scenarios must be `P0`; optional scenarios must be `P1`. |
| `notes` | Reviewer notes. Use `none` only when there is truly nothing to add. |
| `userVisibleProblem` | User-visible issue summary, or `none`. |
| `backendRuntimeProblem` | Backend/runtime issue summary, or `none`. |
| `blockers` | Metadata-safe blocker labels. Use an empty array when there are no blockers. |

For `prompt`, `notes`, `userVisibleProblem`, and `backendRuntimeProblem`, the
placeholder `unknown` is treated as missing evidence. Use `none` only after the
reviewer has inspected the scenario and has nothing to add for that field.

Unknown scenario ids are invalid and cause
`stage2_manual_unknown_scenario_id`. Optional P1 rows do not increase the P0
pass count, but if present they are validated with the same field and label
rules as required P0 rows.
Rows with `result: "not attempted"` are valid records, but they do not count
toward `attemptedScenarioCount` and keep the P0 scenario missing/failing until
a real attempt row is recorded.

At least two distinct known metadata-safe reviewers must appear on required P0
rows. `unknown`, `none`, and fake reviewer labels do not identify a real
reviewer and cause `stage2_manual_reviewer_id_invalid`. Optional P1 rows cannot
satisfy the P0 reviewer-count requirement.

Blocker labels must be metadata-safe ids using only letters, numbers, `.`,
`_`, `-`, and `/`. Labels with whitespace, control characters, or free-form
text cause `stage2_manual_blocker_label_invalid`.

## Validator Output

`validate_main_chat_agent_stage2_manual_dogfood_artifact` returns a focused
manual dogfood summary. `missingScenarioIds` lists required P0 scenarios that
still have no real attempted row. `failedScenarioIds` lists required P0
scenarios that are missing, non-passing, invalid, or have row blockers. Use
`missingScenarioIds` first to fill absent rows, then use `failedScenarioIds` and
`blockers` to fix invalid or failing rows.

## Current Blockers

- `stage2_manual_dogfood_evidence_missing`
- `stage2_manual_reviewer_count_below_2`
- `stage2_manual_p0_reviewer_count_below_2`
- `stage2_manual_reviewer_id_invalid`
- `stage2_manual_required_scenarios_missing`
- `stage2_manual_required_scenarios_not_p0`
- `stage2_manual_optional_scenarios_not_p1`
- `stage2_manual_severity_invalid`
- `stage2_manual_trace_ids_missing`
- `stage2_manual_build_commit_missing`
- `stage2_manual_provider_mode_missing`
- `stage2_manual_provider_mode_invalid`
- `stage2_manual_prompt_missing`
- `stage2_manual_notes_missing`
- `stage2_manual_user_visible_problem_missing`
- `stage2_manual_backend_runtime_problem_missing`
- `stage2_manual_blocker_label_invalid`
- `stage2_manual_result_invalid`
- `stage2_manual_unknown_scenario_id`
- `stage2_manual_dogfood_artifact_invalid`
- `stage2_manual_artifact_schema_invalid`
- `stage2_manual_artifact_commit_missing`
- `stage2_manual_artifact_commit_mismatch`
- `stage2_manual_artifact_current_commit_mismatch`

## Machine-Readable Schema

The JSON artifact must use:

```json
{
  "schemaVersion": "stage2-manual-dogfood-v1",
  "commit": "<metadata-safe-git-commit>",
  "reviewerRecords": [
    {
      "reviewerId": "reviewer-a",
      "buildCommit": "<metadata-safe-git-commit>",
      "providerMode": "deterministic",
      "scenarioId": "S2-D01",
      "prompt": "<prompt used>",
      "taskId": "<task-id>",
      "runId": "<run-id>",
      "result": "pass",
      "severity": "P0",
      "notes": "none",
      "userVisibleProblem": "none",
      "backendRuntimeProblem": "none",
      "blockers": []
    }
  ]
}
```

The top-level `commit` must be a known metadata-safe build commit and must
match the current build commit when the readiness gate can determine it.
`unknown`, `none`, fake labels, local/scripted/fixture/synthetic aliases, and
private-network-looking labels fail as missing build provenance. Stale
artifacts fail with `stage2_manual_artifact_current_commit_mismatch`.

## Notes

Do not fill this report with synthetic reviewer data. Local, scripted, or
agent-authored rows are not manual dogfood evidence.
