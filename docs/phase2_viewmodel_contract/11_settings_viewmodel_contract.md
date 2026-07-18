# SettingsViewModel Contract

Status: proposed/partial contract. No Settings UI implementation.

## Purpose

`DESIGN_DECISION`: `SettingsViewModel` is the product-safe read model for setup readiness, provider/privacy boundary summary, external transmission status, tool permission summary, data controls, safe paths, advanced inspection entry, developer-only gate, MCP/A2A/calibration/versions/metrics visibility status, and support/debug policy.

Backend owner: Proposed `SettingsViewModel` or expanded settings projections
Owner status: `PHASE_2_REQUIRED` for full contract, `PARTIAL` for existing primitives.
Required validation: Phase 3 must separate product settings from support/developer diagnostics before V2 Settings implementation.

## Existing Support

`EXISTING_CODE`: `SettingsPage` currently reads config, diagnostics, `LifeStateProjection`, hot cache, privacy policy, tool permissions, plugins, manifests, router statuses, danger preflight, and data controls.

`VERIFIED_FACT`: Phase 1 says Settings must not become a diagnostic junk drawer or second product truth source.

## Required Field Contract

| Field | Type | Required | Source of truth | Owner status | Evidence | Frontend may infer? | Empty behavior | Error behavior | Stale behavior | Auditability |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `setupReadiness` | `SettingsSetupReadiness` | Yes | Settings read model / projection | `PHASE_2_REQUIRED` | Diagnostics/projection partial | No | Not configured state | Error envelope | Mark stale | Readiness refs |
| `providerPrivacyBoundary` | `ProviderPrivacyBoundarySummary` | Yes | Provider/privacy read model | `PHASE_2_REQUIRED` | Runtime/provider evidence partial | No | Unknown boundary | Error warning | Mark stale; block external actions | Provider/privacy refs |
| `externalTransmissionStatus` | `SettingsExternalTransmissionStatus` | Yes | Privacy/audit/provider owner | `PHASE_2_REQUIRED` | Transmission history exists | No | No transmissions if backend says | Error warning | Mark stale | Audit refs |
| `toolPermissionSummary` | `SettingsToolPermissionSummary` | Yes | `LifeStateProjection.toolPermissions` plus permission owner | `PARTIAL` | Projection/tool store exists | No | Zero permissions | Error warning | Mark stale | Permission refs |
| `dataControls` | `SettingsDataControls` | Yes | Settings/data owner | `PHASE_2_REQUIRED` | Danger preflight/data controls exist | No | No controls | Error warning | Disable risky controls | Data audit refs |
| `safePaths` | string[] | Yes | `LifeStateProjection.safePaths` / config | `EXISTING` | Projection source inspected | No | Empty list | Error warning | Mark stale | Config refs |
| `advancedInspectionEntry` | `ProductAction \| null` | Yes | Visibility policy/read model | `PHASE_2_REQUIRED` | Diagnostics policy | No | Hidden if unavailable | Error warning | Mark stale | Support refs |
| `developerOnlyGate` | `SettingsDeveloperOnlyGate` | Yes | Support/debug policy | `PHASE_2_REQUIRED` | Existing debug gates partial | No | Closed | Closed on error | Closed if stale | Gate refs |
| `mcpA2aVisibility` | `SettingsVisibilityStatus` | Yes | Human-approved policy | `PHASE_2_REQUIRED` | Phase 1 says needs human decision | No | Developer-only default | Developer-only | Developer-only | Policy refs |
| `calibrationVisibility` | `SettingsVisibilityStatus` | Yes | Human-approved policy | `PHASE_2_REQUIRED` | Phase 1 says needs human decision | No | Hidden/advanced | Hidden | Hidden/stale | Policy refs |
| `versionsVisibility` | `SettingsVisibilityStatus` | Yes | Human-approved policy | `PHASE_2_REQUIRED` | Phase 1 says needs human decision | No | Hidden/advanced | Hidden | Hidden/stale | Policy refs |
| `metricsVisibility` | `SettingsVisibilityStatus` | Yes | Human-approved policy | `PHASE_2_REQUIRED` | Metrics developer-only candidate | No | Developer-only | Developer-only | Developer-only | Policy refs |
| `supportDebugPolicy` | `SettingsSupportDebugPolicy` | Yes | Settings/support owner | `PHASE_2_REQUIRED` | Diagnostics visibility policy | No | Default product-only | Product-only | Product-only | Policy refs |

## Settings Nested Contract Types

`PHASE_2_REQUIRED`: These target types keep product settings separate from support/developer diagnostics. They do not create new settings commands or migrations.

```ts
type SettingsSetupReadiness = {
  status: 'not_configured' | 'limited' | 'ready' | 'blocked' | 'unknown'
  missingRequiredItems: string[]
  warnings: ViewModelWarning[]
  nextAction: ProductAction | null
  evidenceRefs: EvidenceRef[]
}

type SettingsExternalTransmissionStatus = {
  lastTransmissionAt: string | null
  totalTransmissionCount: number
  unknownTransmissionCount: number
  externalTransmission: 'none' | 'sent' | 'possible' | 'unknown'
  latestProviderLabel: string | null
  evidenceRefs: EvidenceRef[]
}

type SettingsToolPermissionSummary = {
  grantedCount: number
  pendingCount: number
  blockedCount: number
  highRiskCount: number
  permissionReviewItemRefs: ReviewItemRef[]
  evidenceRefs: EvidenceRef[]
}

type SettingsDataControls = {
  exportAvailable: boolean
  archiveAvailable: boolean
  deleteAvailable: boolean
  dangerPreflightRequired: boolean
  disabledReason?: string
  actions: ProductAction[]
  evidenceRefs: EvidenceRef[]
}

type SettingsDeveloperOnlyGate = {
  open: boolean
  reason: string
  approvedMode: 'product' | 'support' | 'developer'
  expiresAt: string | null
  evidenceRefs: EvidenceRef[]
}

type SettingsVisibilityStatus = {
  defaultVisibility: 'hidden' | 'product' | 'support' | 'developer_only'
  approvedForProductNav: boolean
  humanApprovalRequired: boolean
  reason: string
  debugAction: DebugAction | null
  evidenceRefs: EvidenceRef[]
}

type SettingsSupportDebugPolicy = {
  productVisibleSections: string[]
  supportVisibleSections: string[]
  developerOnlySections: string[]
  defaultMode: 'product'
  advancedEntryAction: ProductAction | null
  evidenceRefs: EvidenceRef[]
}
```

## Product Actions

`ProductAction`: configure provider, manage permissions, manage data, refresh, open advanced inspection, update safe paths where governed.

`ReviewAction`: dangerous settings actions may create or use ReviewItems/confirmation flows.

`DebugAction`: PolicyRouter, ModelRouter, provider health, MCP/A2A internals, metrics, raw debug export.

## UI Cannot Infer

`PHASE_2_REQUIRED`: Settings cannot infer setup readiness, provider trust, external transmission status, support/debug mode, safe-path authority, permission policy truth, or advanced route visibility from local checklist code.

## Visibility Policy

`DESIGN_DECISION`: Default Settings shows product-safe setup, privacy, provider, tools, data controls, and advanced entry.

`PHASE_2_REQUIRED`: MCP/A2A, calibration, versions, and metrics need human classification before ordinary navigation or default display.

`DESIGN_DECISION`: PolicyRouter internals, dev/test wrappers, metrics internals, and historical surfaces are developer-only unless a reviewed support mode says otherwise.

## Empty / Error / Stale Behavior

`DESIGN_DECISION`: Empty means no provider/tool permissions/data history where applicable; show setup actions.

`DESIGN_DECISION`: Error means avoid declaring the system ready.

`DESIGN_DECISION`: Stale means require reload before risky actions.

## Tests Needed

- Settings readiness read-model tests.
- Provider/privacy summary tests.
- External transmission history summary tests.
- Tool permission summary/detail tests.
- Support/debug visibility tests.
- Static guard preventing Settings from becoming second readiness authority.

## Readiness

`READY_WITH_LIMITS`: A limited Settings surface can use existing config, diagnostics, projection, privacy, and tool permission primitives if provider/privacy and support/debug gaps are labeled and risky actions remain guarded.

`PHASE_2_REQUIRED`: Full V2 SettingsViewModel remains required before redesigning Settings as product-safe IA.
