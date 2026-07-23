# OpenLife Codex / Cursor White Workbench Design System

Status: blueprint candidate, not production tokens.
Date: 2026-07-18.

## Typography

Font stack:

```css
-apple-system, BlinkMacSystemFont, "Segoe UI", "Inter", "PingFang SC",
"Microsoft YaHei", sans-serif
```

| Token | Size / line | Weight | Use |
| --- | --- | ---: | --- |
| `type.caption` | 12 / 18 | 400-500 | metadata, source labels, small status |
| `type.body` | 14 / 22 | 400 | compact product copy |
| `type.bodyReading` | 15 / 24 | 400 | primary Chinese reading text |
| `type.control` | 14 / 20 | 500-600 | navigation and buttons |
| `type.section` | 15 / 22 | 600 | local section title |
| `type.surface` | 20 / 28 | 600 | page title |
| `type.display` | 24 / 32 | 600 | one primary task or proposed change |
| `type.metric` | 24 / 30 | 600-700 | rare, non-repeating metric |

Rules:

- Letter spacing is always `0`.
- Product metadata never drops below 12px.
- Chinese uses the native PingFang/system fallback rather than a decorative
  display face.
- Monospace is restricted to raw ids and collapsed debug fields.
- Long English tokens wrap inside technical disclosure only.

## Spacing

Base grid: 4px.

| Token | Value |
| --- | ---: |
| `space.1` | 4px |
| `space.2` | 8px |
| `space.3` | 12px |
| `space.4` | 16px |
| `space.5` | 20px |
| `space.6` | 24px |
| `space.8` | 32px |
| `space.10` | 40px |
| `space.12` | 48px |

Main content uses 24-32px desktop gutters and 16px mobile gutters. Ordinary
grouping uses spacing and 1px dividers rather than card containers.

## Radius And Elevation

| Token | Value | Use |
| --- | ---: | --- |
| `radius.1` | 4px | rows, status labels, compact controls |
| `radius.2` | 6px | buttons, inputs, decision surfaces |
| `radius.3` | 8px | dialogs and bottom sheets |

Default panels have no shadow. The Workspace composer, dialogs, drawers, and
sheets may use one restrained neutral shadow. No decorative card stack,
nested cards, or pill-heavy layout.

## Neutral Color Roles

| Token | Value | Meaning |
| --- | --- | --- |
| `canvas` | `#FFFFFF` | app background |
| `sidebar` | `#F5F5F5` | stable navigation plane |
| `surface` | `#FFFFFF` | primary work surface |
| `surface.subtle` | `#FAFAFA` | local grouping |
| `surface.sunken` | `#F2F2F2` | selected/quiet control |
| `line` | `#E6E6E6` | standard 1px divider |
| `line.strong` | `#D4D4D4` | control/selected boundary |
| `ink` | `#111111` | primary text and primary action |
| `ink.secondary` | `#4F4F4F` | secondary text |
| `ink.muted` | `#666666` | metadata, minimum 4.5:1 target |
| `accent` | `#111111` | primary action, not a brand hue |
| `accent.hover` | `#000000` | primary hover |
| `accent.soft` | `#F2F2F2` | neutral selection |

## Bounded Semantic Colors

| Token | Value | Meaning |
| --- | --- | --- |
| `amber` | `#805B10` | waiting, stale, unknown, review pressure |
| `amber.soft` | `#FFFAF0` | protective/waiting surface |
| `red` | `#9F3A35` | concrete error or blocked action |
| `red.soft` | `#FFF7F6` | error surface |
| `green` | `#2E7D4F` | verified success only |
| `green.soft` | `#F7FBF8` | verified success surface |
| `information` | `#4F4F4F` | neutral evidence text |
| `information.soft` | `#F7F7F7` | evidence surface |
| `focus` | `#2563EB` | keyboard focus ring only |

Provider/privacy unknown is amber and never green. Evidence is neutral by
default. Safe Mode is amber or neutral unless a concrete action failed.

## Line System

- App and Inspector boundaries: 1px `line`.
- Controls: 1px `line.strong` only when a visible boundary is needed.
- Selection: neutral gray fill; no colored left rail.
- Content sections: spacing plus horizontal rule.
- Warning/error emphasis: bounded semantic border or surface, never a
  product-wide colored band.

## Layout Geometry

| Area | Desktop | Narrow desktop | Mobile |
| --- | ---: | ---: | ---: |
| Sidebar | 232px | 208px | drawer |
| Context bar | 56px | 56px | 52px app bar |
| Inspector | 344px on demand | 320px on demand | bottom sheet |
| Main reading max | 980px | 900px | full width |
| Main dense max | 1180px | 1040px | full width |
| Button | 36px | 36px | 44px minimum |
| Icon button | 36px | 36px | 40px minimum |
| Bottom nav | none | none | 64px + safe area |

## Primitive Set

- `WorkbenchShell`
- `SidebarNav`
- `MobileAppBar`
- `MobileBottomNav`
- `ContextBar`
- `PrimaryStatus`
- `SectionHeader`
- `ActionButton`
- `IconButton`
- `DecisionSurface`
- `ExecutionTimeline`
- `ReviewQueue`
- `ChangeDiff`
- `EvidenceInspector`
- `EvidenceSheet`
- `SourceRow`
- `FailClosedNotice`
- `Composer`
- `Dialog`
- `Toast`

## Component State Matrix

Every interactive primitive must specify:

- default;
- hover;
- focus-visible;
- pressed/selected;
- disabled with visible reason;
- loading without optimistic completion;
- stale;
- blocked;
- unknown;
- safe mode where relevant.

Focus uses a 2px external ring. Disabled controls keep full text opacity and a
separate explanation. Loading never changes an action to completed before a
refreshed read model proves it.
