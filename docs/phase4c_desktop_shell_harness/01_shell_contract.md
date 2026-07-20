# Desktop Shell Contract

Status: `REVIEW_CANDIDATE`

## Component Owner

- candidate shell: `frontend/src/ui/shell/OpenLifeWorkbenchShell.tsx`;
- visual rules: `frontend/src/ui/shell/openlife.shell.css`;
- semantic foundation: `frontend/src/ui/foundation/**`;
- review-only composition: `frontend/src/dev/phase4c/**`.

The shell owns layout, landmarks, navigation presentation, Settings context,
Inspector structure, focus transitions, and one live region. Product pages own
their read models, content, actions, and refresh state machines.

## Fixed Desktop Layout

| Region | Contract |
| --- | --- |
| sidebar | fixed `232px`, full height, primary product navigation |
| context bar | fixed `56px`, page identity, one primary status, Inspector trigger |
| main work surface | remaining width, independently scrollable, one current user job |
| Inspector | `344px` when open, absent when closed, non-modal desktop region |
| minimum window | `1024x720`; normal review window `1280x800` |
| QA toolbar | outside the product shell and absent from release |

There is no mobile reflow contract. At widths below the desktop minimum, the
window must enforce its minimum size rather than invent a second navigation
system.

## Information Architecture

Primary product navigation has one job per entry:

1. 今日: daily focus and attention items;
2. 工作区: one current task and active execution;
3. 任务: queue and continuity; explicitly unavailable until migrated;
4. 审核中心: decisions for proposals and permissions;
5. LifeModel: current sourced long-term understanding.

Settings is a sidebar utility. Selecting it replaces product navigation with a
dedicated Settings context and Back control. Advanced is not a top-level
product entry; diagnostic data stays in Settings or collapsed Inspector detail.

## Fixed Information Priority

Every fixture follows:

1. current goal or truth;
2. blocker, risk, or important exception;
3. next allowed action;
4. evidence entry;
5. collapsed technical information.

The shell top bar does not repeat every page fact. Main content uses unframed
sections and local dividers rather than dashboard card grids.

## Interaction Contract

- current navigation exposes `aria-current="page"`;
- the skip link is keyboard-reachable and transfers focus to the main work surface;
- navigation moves focus to the new context heading;
- an unavailable entry opens an explicit unavailable page and never redirects;
- opening Inspector focuses its heading;
- closing Inspector restores focus to its trigger;
- Settings Back restores focus to the Settings utility trigger;
- Settings search changes only visible categories and announces result count;
- exactly one polite live region announces dynamic changes;
- every clickable fixture control has visible output;
- unsupported controls are disabled with adjacent `disabledReason`.

## State And Safety Semantics

- unknown, stale, waiting, and Safe Mode use amber protection semantics;
- red is reserved for concrete error or blocked destructive action;
- green requires an explicitly verified success state;
- missing `ProviderPrivacyBoundarySummary` never becomes a green local claim;
- viewing a review item enters `pending_decision` only;
- `approved` and `materializationStatus` remain separate;
- approval confirmation never displays applied or completed;
- absent application commands remain disabled;
- fixtures never infer product truth from page-local values.

## Fixture Action Contract

Every fixture product/review action exposes:

- `id`;
- `kind` (`product`, `review`, or `debug`);
- `enabled`;
- `disabledReason`;
- `targetRef`;
- confirmation semantics;
- materialization semantics.

These attributes verify shape and UX behavior only. They do not dispatch a
Tauri command. Review approval declares `decision_only_refresh_required`, so a
later product migration must dispatch, refresh, and then trust the new read
model rather than setting local success.
