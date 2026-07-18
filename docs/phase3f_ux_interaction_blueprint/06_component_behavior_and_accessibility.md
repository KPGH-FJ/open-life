# Component Behavior And Accessibility

Status: `REVIEW_CANDIDATE`

## 1. Frozen Visual Baseline

Phase 3F keeps the approved Codex/Cursor white workbench direction:

- canvas/surface `#ffffff`;
- sidebar/subtle region `#f5f5f5`;
- primary text `#111111`;
- secondary text `#4f4f4f`;
- muted text no lighter than `#666666` for 12-14px use;
- divider `#e6e6e6`, strong divider `#d4d4d4`;
- amber `#805b10` with `#fffaf0` for waiting/unknown/stale/protection;
- red `#9f3a35` with `#fff7f6` for concrete error/blocked destructive action;
- green `#2e7d4f` with `#f7fbf8` for verified success only;
- focus ring `#2563eb`;
- system sans/PingFang SC, fixed 14px body, 15px reading, 12px minimum
  metadata, 20/24px surface/page headings;
- production React uses the repository's existing `lucide-react` icons at a
  consistent 18-20px size and 1.75 stroke width; prototype inline SVGs are
  standalone visual placeholders, not a second icon system;
- 4/8/12/16/20/24/32/40/48px spacing scale;
- 4/6/8px radius; no pill-heavy or shadow-heavy UI.

No text scales with viewport width. No whole region uses opacity for visual
de-emphasis.

## 2. Component State Matrix

| Component | Default | Hover | Focus | Disabled | Loading | Stale/blocked/unknown |
|---|---|---|---|---|---|---|
| Primary button | black/white | black + subtle inset | 2px blue ring | gray surface/text + adjacent reason | same width, spinner + verb | unavailable; no fake click |
| Secondary button | white + strong border | subtle gray fill | blue ring | muted + reason | stable label width | inspection may remain enabled |
| Destructive quiet button | red text, neutral background | red soft fill | blue ring | muted | stable pending label | used only for real reject/cancel/delete |
| Nav row | transparent | `#eeeeee` | blue ring | unavailable dialog or true disabled | n/a | current uses `aria-current=page` |
| Toggle | state + label | track emphasis | ring around control | disabled but readable | pending state outside toggle | never represents unknown as off |
| Text/select field | white + line | stronger line | blue ring | gray fill, readable value | group disabled during save | error/unknown message below |
| Status lozenge | neutral semantic label | no required hover | n/a | n/a | `正在...` | amber for waiting/unknown/stale; red for concrete failure |
| Timeline event | compact row | subtle surface | focus when interactive | n/a | active indicator without layout shift | blocker expands as the one active event |
| Evidence item | metadata summary | subtle fill | blue ring | n/a | skeleton only when fetching body | unknown body stays unavailable |
| Inspector | closed | n/a | close button first | n/a | section-level loading | conclusion remains first |
| Dialog | hidden | n/a | initial safe focus, trapped | confirm disabled if invalid | stable pending footer | Escape/cancel never dispatches |
| Attachment chip | imported/ready | detach affordance | visible ring | detach disabled during dispatch | importing progress | failed chip names reason and retry/remove |

## 3. Focus Order

Desktop product page:

1. skip/main focus target when navigating;
2. sidebar current and subsequent navigation;
3. utility controls;
4. top context actions;
5. primary page content in reading order;
6. primary/decision actions;
7. Inspector only after it opens.

Settings:

1. 返回工作台;
2. search;
3. category list;
4. page heading/status;
5. form controls in visible order;
6. test/save actions;
7. advanced disclosure.

Mobile:

1. app bar navigation trigger;
2. page heading/main content;
3. current actions;
4. bottom navigation;
5. drawer or evidence sheet only while open.

## 4. Dialog, Drawer, And Sheet Behavior

- Native `<dialog>` or equivalent accessible modal semantics.
- Initial focus goes to the dialog heading or safest non-destructive action.
- Tab and Shift+Tab remain trapped.
- Escape closes unless a backend dispatch has entered a non-cancellable commit
  boundary; then the dialog stays and explains why.
- Closing restores focus to the opener.
- Background content is inert/hidden from assistive technology while modal.
- Mobile evidence uses a bottom sheet but keeps dialog semantics.
- Inspector on desktop is non-modal only when it does not occlude the work
  surface; on mobile it is modal.

## 5. Dynamic Announcements

Polite live announcements:

- page/category/filter changed;
- evidence opened/closed;
- settings search result count;
- import started/cancelled/committed/detached;
- refresh started/completed/failed;
- review decision recorded;
- task remains waiting, resumes, fails, or becomes remote unknown;
- settings saved but boundary still awaiting refresh.

Assertive announcements are reserved for:

- concrete data-integrity failure;
- external result unknown after a potentially side-effecting dispatch;
- destructive action rejected by policy;
- session-expiring decision that requires immediate user attention.

Announcements describe the new state once. They do not repeat every visual
status label.

## 6. Labels And Error Content

- Icon-only controls have accessible names and tooltips where unfamiliar.
- Buttons are verbs: 查看依据, 仅允许本次并继续, 拒绝, 稍后处理,
  修改, 批准变更, 测试连接, 保存设置.
- Product UI avoids enum names, route ids, `EvidenceRef`, `projectionUpdatedAt`,
  `materialization`, or `canonical_scope`.
- Inspector may show technical fields under a collapsed disclosure.
- Disabled controls always state what evidence or backend action is missing.
- Form errors identify the field and next step; color is never the only cue.

## 7. Target Sizes And Reflow

- Desktop control height: 34-36px minimum; compact icon buttons 32px only where
  pointer density is expected.
- Mobile interactive targets: 44px minimum.
- Bottom navigation label: 11-12px, never 10px.
- Long Chinese/English ids wrap only inside technical details.
- Buttons with long labels wrap or grow; they never clip.
- Fixed-format areas use stable grid tracks and min/max constraints.
- At 390px, decision actions can wrap into two rows; content remains visible
  above the fixed safe-area bar.

## 8. Contrast And Reduced Motion

- Normal text target is WCAG AA 4.5:1.
- Large text and non-text UI target 3:1.
- Focus indicator must remain visible against white, gray, amber, red, and green
  surfaces.
- `prefers-reduced-motion: reduce` disables smooth panel and loading motion.
- Progress never relies solely on animation; a text state is always present.

## 9. Keyboard Acceptance Path

The prototype and later React implementation must verify:

1. keyboard-only page navigation;
2. Workspace attachment and permission decision;
3. confirmation cancel and confirm paths;
4. task resume result and focus placement;
5. Review reject/later/edit/approve paths;
6. Inspector open, technical disclosure, close, and focus restoration;
7. Settings search, category selection, provider form, test, save, and Back;
8. mobile drawer and evidence sheet focus trap;
9. disabled reason announced through `aria-describedby`;
10. no focus loss after re-render or status transition.

## 10. Screen Reader Acceptance

At minimum test VoiceOver on macOS with Safari/WebView-equivalent semantics:

- landmarks and page names;
- `aria-current` on current nav;
- status/alert announcements;
- list/table/diff reading order;
- dialog/sheet title and focus trap;
- field labels, descriptions, errors, and secret masking;
- disabled reason relationship;
- approved-not-applied wording;
- unknown privacy boundary without a misleading green/status-only cue.

Automated checks supplement but do not replace this manual pass.
