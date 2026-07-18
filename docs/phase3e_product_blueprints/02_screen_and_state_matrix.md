# Phase 3E Screen And State Matrix

Status: blueprint candidate.
Date: 2026-07-18.

## Global Information Priority

1. Current goal, task, or proposed change.
2. Blocker, risk, or uncertainty.
3. Next action or decision.
4. Evidence entry.
5. Technical/debug detail.

The same fact appears once as the main conclusion. Supporting sections must add
a different dimension rather than rephrasing it.

## Today

Primary question: `今天我该关注什么？`

Default composition:

- one daily focus statement;
- one compact schedule/continuity section;
- one review-pressure notice when needed;
- one next action;
- evidence on demand.

States:

- ready;
- ready with pending review;
- loading;
- empty;
- stale;
- unknown;
- blocked;
- error.

Today never classifies a state signal as a goal and never turns a pending
proposal into a completed change.

## Workspace

Primary question: `OpenLife 正在做什么，需要我决定什么？`

Default composition:

- task objective and editable clarification entry;
- staged execution timeline;
- only the current interruption expanded;
- in-context permission/review decision;
- compact composer/control bar;
- evidence Inspector on demand.

States:

- idle composer;
- understanding;
- planning;
- running;
- waiting permission;
- blocked;
- failed;
- cancelled;
- completed;
- completed with pending items.

## Tasks

Primary question: `哪些工作正在进行、卡住或可以继续？`

Default composition:

- compact status/filter toolbar;
- task list with lifecycle, next control, and latest result summary;
- selected task detail with timeline and evidence entry;
- no generic todo checkboxes.

States:

- active;
- waiting input;
- blocked;
- failed/retryable;
- completed;
- cancelled;
- empty;
- stale.

The visual blueprint does not prove a complete production Tasks route or read
model.

## Review Center

Primary question: `OpenLife 想改变什么，需要我同意什么？`

Default composition:

- review queue at desktop widths;
- one-sentence proposed change;
- current-to-proposed diff;
- reason, source, risk, impact, and expiry;
- fixed decision actions;
- evidence Inspector on demand.

Decision states:

- pending;
- editing;
- deferred;
- rejected;
- approved decision;
- applying only after acknowledged command;
- applied only after refreshed read-model proof;
- application failed;
- expired;
- stale/unknown.

Product labels:

- `批准变更`
- `拒绝`
- `修改`
- `稍后处理`
- `已批准，尚未应用`
- `应用变更`

`物化`, `EvidenceRef`, and raw action kinds stay in technical detail.

## LifeModel

Primary question: `OpenLife 现在怎样理解我？`

Default composition:

- current understanding summary;
- dimension navigation;
- current statements with provenance and confidence language;
- pending suggestions separated from current truth;
- history/memory as a constrained sub-surface;
- compatibility limitation visible once.

States:

- current canonical view;
- limited compatibility view;
- pending suggestions;
- empty/onboarding;
- stale;
- unknown;
- error.

## Settings

Primary question: `模型、隐私和权限现在是什么边界？`

Default composition:

- provider/privacy summary first;
- local model configuration;
- cloud provider and transmission boundary;
- tool permissions;
- data/export/recovery;
- advanced support details last.

Unknown transmission fails closed. The UI never infers local/private truth from
configured model count or a page-local provider guess.

## Shared Critical States

| State | Color | Copy behavior | Action behavior |
| --- | --- | --- | --- |
| Ready | neutral/verified green | concise verified result | normal actions |
| Loading | neutral/blue | current operation only | no completion claim |
| Waiting | amber | name the user decision | only contract-valid decisions |
| Stale | amber | show last known time | refresh/inspect only |
| Unknown | amber/neutral | say what is missing | unsafe actions disabled |
| Safe mode | amber/neutral | explain protection | risky action disabled |
| Blocked | red only for concrete blocker | explain blocker and recovery | recovery/inspect only |
| Error | red | concrete failure | retry only when supported |
| Approved | amber/neutral | decision recorded | not complete |
| Applied | verified green | refreshed proof | completion may be shown |
