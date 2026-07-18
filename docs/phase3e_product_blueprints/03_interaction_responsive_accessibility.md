# Phase 3E Interaction, Responsive, And Accessibility Spec

Status: blueprint candidate.
Date: 2026-07-18.

## Desktop Shell

- Sidebar is stable and independently scrollable.
- Main surface owns the first focusable page action.
- Inspector is closed by default for focused work and opens from an evidence or
  scope action.
- Opening Inspector must not change product state.
- Closing Inspector restores focus to its trigger.
- Advanced/debug content remains inside Inspector or Settings and is visually
  downgraded by placement, size, and background, never subtree opacity.

## Mobile Shell

- Use a 52px app bar with menu, product identity, and evidence entry.
- Use a 64px bottom navigation for 今日, 工作区, 审核中心, and LifeModel.
- Tasks and Settings remain available from the drawer until Tasks is approved
  as a mature mobile-primary destination.
- The desktop sidebar never stacks above page content.
- Inspector becomes a modal bottom sheet with a visible drag handle, close
  button, focus trap, Escape/back behavior, and focus restoration.
- Review and permission decisions stay above the bottom navigation and remain
  visible while the decision content scrolls.

## Interaction Outcomes

Every enabled control in the standalone prototype must produce one of:

- a visible navigation result;
- a visible Inspector/sheet result;
- a confirmation dialog;
- a static feedback dialog/toast clearly labeled as blueprint feedback.

Controls without an outcome are disabled and show `仅视觉样式` or the actual
contract blocker. There are no silent fake buttons.

## Review Lifecycle

1. `查看待决定建议` opens a pending decision.
2. `批准变更` opens confirmation and does not mutate the current view.
3. Confirming approval opens `已批准，尚未应用`.
4. `应用变更` remains disabled when no command contract exists.
5. `已完成` is not shown without a refreshed applied read model.

## Permission Lifecycle

- Scope must answer purpose, tool, target, data range, transmission boundary,
  grant mode, duration, and revocation.
- Missing duration/revocation or unknown transmission disables unsafe grants.
- `仅允许本次` stays disabled until enforceable one-time semantics exist.
- Reject feedback must state that the task remains paused/blocked and that no
  read or write occurred.

## Keyboard Path

Required order:

1. skip/main entry where applicable;
2. primary navigation;
3. context-bar actions;
4. page content and primary action;
5. review actions;
6. evidence Inspector;
7. collapsed technical detail.

Requirements:

- `aria-current="page"` on active navigation.
- 2px visible focus ring with 2px offset.
- `aria-live="polite"` for fixture, navigation, and state changes.
- modal focus trap and restoration.
- native disabled state plus visible `aria-describedby` reason.
- no keyboard-only dead end.

## Contrast And Motion

- Normal text target: WCAG AA 4.5:1.
- Large text target: 3:1, though primary text should usually exceed 4.5:1.
- Focus indicators target at least 3:1 against adjacent colors.
- `prefers-reduced-motion` removes nonessential transitions.
- No status relies on color alone; every state has text and/or icon semantics.

## Viewport QA

Required:

- `1440x900`
- `1280x800`
- `390x844`

For each critical screen verify:

- no horizontal overflow;
- no clipped text or controls;
- no incoherent overlap;
- primary action remains reachable;
- evidence remains reachable in the first interaction layer;
- mobile fixed actions do not collide with bottom navigation or safe area.
