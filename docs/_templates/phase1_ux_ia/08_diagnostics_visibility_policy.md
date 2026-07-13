# Diagnostics Visibility Policy

## Visibility Levels

```text
DEFAULT_PRODUCT
EXPANDABLE_DETAILS
ADVANCED_INSPECTOR
DEVELOPER_ONLY
REMOVE_OR_ARCHIVE
NEEDS_HUMAN_DECISION
```

## Classification Matrix

| Surface | V2 visibility | Reason | Risk if too visible | Risk if too hidden |
|---|---|---|---|---|
| Safe Mode | DEFAULT_PRODUCT | | | |
| Usage readiness | DEFAULT_PRODUCT | | | |
| Current task status | DEFAULT_PRODUCT | | | |
| Pending review count | DEFAULT_PRODUCT | | | |
| Tool permission summary | DEFAULT_PRODUCT | | | |
| Runtime disclosure | DEFAULT_PRODUCT / EXPANDABLE_DETAILS | | | |
| Tool call details | EXPANDABLE_DETAILS | | | |
| Reasoning trace | ADVANCED_INSPECTOR | | | |
| Run trace | ADVANCED_INSPECTOR | | | |
| Kernel events | ADVANCED_INSPECTOR | | | |
| Durable events | ADVANCED_INSPECTOR | | | |
| Raw transcript | ADVANCED_INSPECTOR | | | |
| Provider health | ADVANCED_INSPECTOR | | | |
| PolicyRouter | DEVELOPER_ONLY | | | |
| ModelRouter | ADVANCED_INSPECTOR / DEVELOPER_ONLY | | | |
| MCP/A2A | DEVELOPER_ONLY / NEEDS_HUMAN_DECISION | | | |
| Metrics | DEVELOPER_ONLY | | | |
| Calibration | NEEDS_HUMAN_DECISION | | | |
| Versions | NEEDS_HUMAN_DECISION | | | |
| tauriDev/test/historical surfaces | DEVELOPER_ONLY | | | |

## ProductAction vs DebugAction

Default product surfaces may expose `ProductAction`.

Advanced inspector may expose `DebugAction`.

Review Center may expose `ReviewAction`.

Do not mix debug-only actions into default product actions.

## Evidence Preservation Rule

Hide by default does not mean delete evidence.

## Default Product Layer

## Expandable Details

## Advanced Inspector

## Developer-only

## Human Decisions Needed
