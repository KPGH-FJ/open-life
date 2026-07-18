# Review Center Model

## Purpose

审核中心 is the central control surface for consequential changes.

It is not a mailbox.

## ReviewItem Type

```ts
type ReviewItemType =
  | 'proposal'
  | 'permission_request'
  | 'external_write'
  | 'memory_update'
  | 'lifemodel_change'
  | 'policy_change'
  | 'dangerous_action'
```

## ReviewItem Status

```ts
type ReviewItemStatus =
  | 'pending'
  | 'approved'
  | 'rejected'
  | 'expired'
  | 'blocked'
  | 'revoked'
  | 'failed'
```

## ReviewItem Required Fields

| Field | Purpose |
|---|---|
| user-readable title | User knows what decision is being asked |
| risk level | Low / medium / high |
| impact scope | What will change |
| source | Task / workspace / tool that triggered it |
| evidence | Why the request exists |
| default recommendation | What OpenLife recommends |
| available actions | Approve / reject / later / modify / inspect evidence |
| expiration behavior | What happens if user does nothing |
| audit record | How it can be traced later |

## Available Review Actions

These are `ReviewAction`, not generic `ProductAction`.

- 批准
- 拒绝
- 稍后
- 修改
- 查看依据

## Relationship to Workspace

## Relationship to Tasks

## Relationship to Memory

## Relationship to LifeModel

## Relationship to Tool Permissions

## What Should Not Stay in Workspace by Default

## Risk Model

## Auditability

## Human Decisions Needed
