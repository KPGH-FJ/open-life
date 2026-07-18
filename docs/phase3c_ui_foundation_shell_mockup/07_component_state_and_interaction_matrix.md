# Phase 3C Component State And Interaction Matrix

Status: static UI foundation acceptance matrix.
Date: 2026-07-10.

## Component State Matrix

| Component | Default | Hover | Focus | Disabled | Loading | Stale | Blocked | Unknown |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Product action | Primary navigation action or neutral inspect action | Neutral surface change | 2px visible ring | Native disabled plus visible reason | Must not imply completion | Risk actions disabled | Concrete reason visible | Inspect-only or disabled |
| Review decision | Reject/later/edit plus one primary approve | Neutral surface change | Same visible ring | Native disabled plus reason | Decision does not imply applied | Decisions disabled when item stale | Red only for concrete failure | Fail closed |
| Approve confirmation | Native modal dialog | Button hover | Focus remains in dialog | Not applicable | No optimistic result | Not shown for stale item | Cancel remains available | Cancel/fail closed |
| Application state | `已批准，尚未应用` | Not interactive | Not applicable | Apply action disabled without command | `applying` only from backend | Refresh required | Failure shown distinctly | Unknown is not complete |
| Permission scope | Human summary plus detailed Inspector view | Not applicable | Section can receive programmatic focus | One-time allow disabled without enforceable scope | Not applicable | Scope must be refreshed | Missing duration/revoke blocks allow | Transmission unknown stays amber |
| Navigation item | Primary or utility placement | Neutral background | Same visible ring | Uncovered task opens explicit state | Not applicable | Current surface remains readable | Unavailable is explicit | Does not invent a page model |
| Primary status | One conclusion | Not interactive | Not applicable | Not applicable | Neutral loading | Amber | Red only for concrete failure | Amber/neutral |
| Safe Mode | Amber/neutral protection | Not interactive | Not applicable | Not applicable | Not applicable | Amber | Specific failed action may be red | Amber/neutral |
| Reference item | Human label, source, sensitivity | No required hover | Visible programmatic focus | Not applicable | Explicit loading copy | Stale label retained | Missing evidence is a blocker | Empty list is not proof |
| Inspector | Persistent desktop third pane except focused Workspace, where it is on demand | Not applicable | Exact item/section focus | Not applicable | Explicit loading | Retains stale state | Risk remains visible | Starts with plain-language uncertainty or the selected Workspace event |
| Mobile Inspector | Closed bottom sheet | Entry hover | Focus trapped and restored | Not applicable | Same as Inspector | Same | Same | Same |
| QA fixture selector | Outside shell | Native hover | Native focus ring | Not applicable | Not applicable | Selects fixture only | Selects fixture only | Never a product mode |

## Information Priority

Every product state follows:

1. Current goal or proposed change.
2. Blocker, risk, or impact.
3. Next decision or action.
4. Reference and privacy entry.
5. Collapsed technical details and debug fixture.

The Inspector follows:

1. What happened.
2. Main risk.
3. What the user can do.
4. Proposal or permission detail.
5. Model/privacy boundary and references.
6. Limitations.
7. Collapsed raw fields, source map, and debug actions.

Workspace is the focused exception: the main surface follows task objective ->
execution timeline -> current permission interruption -> decision. Its
Inspector starts with the selected permission scope and omits the repeated
page-level overview.

## Navigation Matrix

| Destination | Desktop | Mobile bottom | Mobile drawer | Notes |
| --- | --- | --- | --- | --- |
| 今日 | Primary | Yes | Yes | Daily attention only |
| 工作区 | Primary | Yes | Yes | Current execution only |
| 任务 | Primary, unavailable | No | Yes | Remains pending an independent task model |
| 审核中心 | Primary | Yes | Yes | Decisions, not evidence explanation |
| LifeModel | Primary | Yes | Yes | Current long-term understanding |
| 设置 | Utility footer | No | Utility section | Not primary daily workflow |
| 支持信息 | Utility button | No | Utility section | Opens collapsed debug section; not navigation |

## Interaction Matrix

| Interaction | Static outcome | Verification signal |
| --- | --- | --- |
| QA fixture selection | Renders fixed state | Title/status update; live announcement |
| Today `查看待决定建议` | Opens `review-pending-decision` | Pending status remains visible; no approval occurs |
| Review `批准变更` | Opens confirmation dialog | Current state remains pending before confirmation |
| Confirm approval | Opens approved-not-applied fixture | Status becomes `已批准，尚未应用`; apply remains disabled |
| Reject/later/edit | Opens explicit static result dialog | Dialog explains result; no hidden write |
| Permission `查看访问范围` | Opens the on-demand desktop Inspector or mobile sheet | Purpose, target, data, transmission, duration, revocation visible; tool/policy collapsed |
| Permission `仅允许本次` | Native disabled | Reason states that one-time contract is missing |
| Permission `拒绝` | Opens explicit static feedback | Task remains blocked; no file read or state transition occurs |
| Workspace Inspector close | Restores focused two-column shell | Focus returns to the action that opened it |
| Covered navigation | Changes fixture state | Visible `aria-current="page"` follows |
| `任务` | Explicit unavailable surface | No fabricated task read model |
| `设置` | Opens utility destination | Not present in mobile bottom navigation |
| `支持信息` | Opens Inspector technical section | No top-level advanced destination |
| Enabled inspect action | Focuses matching reference or section | Temporary highlight |
| Debug raw JSON | Opens static dialog | Payload visible; dialog states no command/write |
| Mobile menu | Opens modal drawer | Focus trap, Escape, restore |
| Mobile Inspector | Opens modal bottom sheet | Focus trap, Escape/backdrop, restore |
| Mobile pending review | Four fixed decision buttons above bottom nav | 42px controls; content receives bottom padding |
| Dynamic change | Announces state | Polite live region updates |

## Accessibility Rules

- Body text is 14px; captions and metadata are at least 12px.
- Mobile action controls are at least 42px high; app-bar icon controls are 40px.
- Workspace mobile permission decisions are 44px high and stay above the fixed
  bottom navigation in the `390x844` reference viewport.
- Muted text remains `#666660` and must meet 4.5:1 against its surface.
- Debug content is reduced through position, type, and background, not subtree opacity.
- Active navigation uses `aria-current="page"` in each rendered navigation instance.
- Native disabled controls expose a visible reason through `aria-describedby`.
- Mobile drawer and Inspector trap focus, close with Escape, and restore focus.
- Confirmation uses a native modal dialog and does not mutate state before confirm.
- `prefers-reduced-motion` removes nonessential transition duration.

## React Icon Migration

The standalone mockup remains dependency-free and uses inline line SVGs. React
migration must use the existing `lucide-react` system:

| Static key | `lucide-react` target |
| --- | --- |
| `calendar` | `CalendarDays` |
| `workspace` | `Monitor` |
| `tasks` | `ListChecks` |
| `review` | `ShieldCheck` |
| `lifemodel` | `UserRound` |
| `settings` | `Settings` |
| `terminal` | `SquareTerminal` |
| `menu` | `Menu` |
| `close` | `X` |

Do not copy the inline SVG implementation into production React.
