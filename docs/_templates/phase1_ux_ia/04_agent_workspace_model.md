# Agent Workspace Model

## Purpose

The workspace is where the user and OpenLife complete current work together.

## Workspace Is Not Chat

Chat is an input mode inside the workspace, not the product model.

## Required Zones

```text
Workspace
├── Intent Composer
├── Understanding Panel
├── Execution Timeline
└── Control / Review Drawer
```

## Zone Specs

### Intent Composer

Purpose:

Default state:

User actions:

Must not:

### Understanding Panel

Purpose:

Displays:

Missing context:

User correction:

### Execution Timeline

Purpose:

Displays:

States:

Evidence refs:

### Control / Review Drawer

Purpose:

Displays:

Links to Review Center:

Actions:

## State Model

- idle
- understanding
- planning
- running
- waiting_permission
- blocked
- failed
- cancelled
- completed
- completed_with_pending_items

## ChatPage Responsibility Migration

| Existing ChatPage responsibility | V2 destination | Reason |
|---|---|---|
| user input | 工作区 | |
| natural language intent | 工作区 | |
| skill selection | 工作区 / 高级检查 | |
| task session | 工作区 / 任务 | |
| task resume | 工作区 / 任务 / 审核中心 | |
| task cancel | 工作区 / 任务 | |
| retry | 工作区 / 任务 | |
| reasoning trace | 高级检查 | |
| kernel events | 高级检查 / collapsed timeline details | |
| durable agent events | 工作区 timeline / 高级检查 | |
| tool calls | 工作区 summary / 高级检查 detail | |
| blockers | 工作区 / 任务 | |
| generated proposals | 审核中心 | |
| pending review | 审核中心 / global summary | |
| final delivery | 工作区 result / 任务 detail | |
| run history | 任务 | |
| execution transcript | 高级检查 / 任务 detail | |
| memory impact | 审核中心 / 记忆 / LifeModel | |
| LifeModel impact | 审核中心 / LifeModel | |

## Scenario Validation Format

Use the global fixed scenario template for workspace-related scenarios.

## Empty / Loading / Error States

## Evidence and Advanced Inspector

## Human Decisions Needed
