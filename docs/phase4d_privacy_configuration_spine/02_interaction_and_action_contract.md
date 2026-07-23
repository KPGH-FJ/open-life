# Phase 4D Privacy And Configuration Interaction Contract

Status: `IMPLEMENTED`
Date: 2026-07-21

## Desktop Settings IA

Settings is a Shell utility context, not a primary product route. Its fixed
desktop categories are:

1. 模型与供应商;
2. 隐私与网络;
3. 工具与权限;
4. 数据与恢复;
5. LifeModel 与记忆;
6. 外观;
7. 高级与支持.

Only the first two are implemented in this slice. Every other category opens
an explicit unavailable state instead of a blank page or fake control.

## Information Priority

The Settings work surface uses this order:

1. current backend-owned provider/privacy boundary;
2. the editable configuration group;
3. test, consent, save, or refresh blocker/result;
4. the next explicit action;
5. evidence entry;
6. technical fields in the Inspector disclosure.

The top context bar shows only an active operation or exception. Steady-state
boundary detail stays in the main conclusion and local/private sidebar area.

## Interaction Contract

| User action | Verifiable result | Forbidden interpretation |
| --- | --- | --- |
| edit a field | draft revision increments and current boundary becomes unknown | draft is not backend truth |
| change provider or endpoint | masked credential is cleared | old secret cannot move to a new target |
| test a loopback target | execute the exact draft test | does not save |
| test an external target | show provider, host, model, and possible transmission confirmation | opening the dialog sends nothing |
| consent required | show the exact ReviewItem when resolvable | no guessed review navigation |
| view permission | show before/after, purpose, requested/resolved target, boundary, expiry, revocation | viewing is not approval |
| approve once | record and refresh the exact review decision | does not test, save, or prove transmission |
| test after approval | user explicitly starts a new exact test | no automatic retry |
| successful test | show verified result only with exact receipt | not a saved or generally available provider |
| save | call `save_config`, then re-read both owners | command return is not a known boundary |
| save failure | retain draft and show error | backend state does not become available |
| boundary refresh unknown | show unknown and remain non-green | no local/cloud inference |
| search settings | match static category labels/help terms and announce count | never match credentials or config values |

## Product Action Contract

| ID | kind | targetRef | Enablement |
| --- | --- | --- | --- |
| `settings.provider.test_connection` | `configure` | `settings-draft:<revision>` | valid draft, usable credential, no operation in flight |
| `settings.provider.save_config` | `configure` | `AppConfig` | valid unsaved revision in dirty/tested state |

Both actions always carry `id`, `kind`, `enabled`, optional `disabledReason`,
and `targetRef` into rendered data attributes. Review actions remain a separate
category and preserve confirmation, expected materialization, and completion
proof fields from the backend action contract.

## State Matrix

| State | Main treatment | Actions |
| --- | --- | --- |
| loading | neutral, no local claim | edit/test/save closed |
| idle | refreshed boundary plus sanitized config | test may open; save disabled |
| dirty | unknown protective boundary | test/save follow validation |
| testing | loading on test | all configuration controls disabled |
| tested | exact test result, explicitly not saved | save only if revision changed |
| saving | command in flight | all controls disabled |
| refreshing boundary | neutral loading, no ready claim | all controls disabled |
| ready | refreshed known boundary | save disabled until another edit |
| unknown | amber/unknown | no green local or transmission result |
| blocked/consent required | waiting | exact review only when resolved |
| failed test | result-specific error/unknown | modify before save; no auto retry |
| failed save | red operation failure plus retained draft | modify or retry explicitly |
| stale/error envelope | fail-closed | no action derived from old payload |

Foundation controls retain default, hover, visible focus, disabled, and loading
states. Red is reserved for actual failure; amber covers waiting/protective
states; green is limited to independently verified facts.

## Accessibility And Desktop Scope

- current navigation uses `aria-current="page"`;
- Shell navigation, setting search, primary actions, dialogs, review decisions,
  and Inspector are keyboard reachable;
- focus returns from Settings and Inspector to their opening controls;
- search result and operation changes use polite live regions;
- native password, select, fieldset, button, details, and dialog semantics are
  retained;
- text token pairs meet `4.5:1`; focus/control token pairs meet `3:1`;
- desktop acceptance viewports are `1440x900`, `1280x800`, and `1024x720`;
- mobile remains intentionally outside implementation and acceptance scope.
