# Phase 3E Blueprint Field Source Map

Status: visual-fixture source classification.
Date: 2026-07-18.

## Classification

- `CURRENT_READ_MODEL`: field exists in a current backend-owned read model.
- `PROPOSED_REVIEW_PROJECTION`: target field required before production Review
  port.
- `LAYOUT_FIXTURE`: content exists only to test hierarchy and density.
- `VISUAL_STATE`: component/rendering state, not backend truth.

## Global Shell

| UI field | Source |
| --- | --- |
| provider/privacy status | `ProviderPrivacyBoundarySummary` |
| surface status | `ViewModelEnvelope.status` or surface-owned read model |
| evidence list | `EvidenceRef[]` or surface provenance refs |
| primary actions | `ViewModelEnvelope.actions.primary` |
| review actions | `ViewModelEnvelope.actions.review` |
| debug actions | `ViewModelEnvelope.actions.debugOnly` |
| QA fixture selector | `VISUAL_STATE`, outside product shell |

## Today

| UI field | Source |
| --- | --- |
| plan freshness/status | `LifeStateProjection` plus bounded Today projection |
| pending review pressure | backend review summary / projection |
| focus title and schedule | `LAYOUT_FIXTURE` until backend Today contract owns it |
| next action | `ViewModelEnvelope.actions.primary` target |
| no automatic external action | privacy/action contract, not local inference |

## Workspace

| UI field | Source |
| --- | --- |
| objective, lifecycle, timeline | limited `WorkspaceViewModel` target |
| waiting permission | `ReviewItem.status` plus task relation |
| purpose/tool/target/data scope | `PROPOSED_REVIEW_PROJECTION` |
| transmission boundary | `ProviderPrivacyBoundarySummary` |
| duration/revocation/grant mode | `PROPOSED_REVIEW_PROJECTION` |
| composer content | `VISUAL_STATE` |

## Tasks

| UI field | Source |
| --- | --- |
| task lifecycle and control | `TasksViewModel` target |
| task titles/result previews | `LAYOUT_FIXTURE` for visual density |
| active/waiting/failed grouping | `TasksViewModel` lifecycle taxonomy |

## Review Center

| UI field | Source |
| --- | --- |
| item id/type/status/risk/expiry | current `ReviewItem` |
| allowed actions | current `ReviewItem.allowedActions` |
| current/proposed diff | `PROPOSED_REVIEW_PROJECTION` from bounded proposal data |
| reason/source/impact/object label | `PROPOSED_REVIEW_PROJECTION` |
| approved not applied | `ReviewItem.status + materializationStatus` |
| applying/applied | refreshed backend read model only |

## LifeModel

| UI field | Source |
| --- | --- |
| truth mode/current summary | `LifeModelViewModel` |
| provenance references | `LifeModelViewModel.provenanceRefs` |
| pending suggestions | `LifeModelViewModel.pendingUpdateCounts` |
| dimension statements | `LAYOUT_FIXTURE` unless current ViewModel owns them |
| compatibility limitation | `LifeModelViewModel.contractLimitations` |

## Settings

| UI field | Source |
| --- | --- |
| provider label | `ProviderPrivacyBoundarySummary.providerLabel` |
| external transmission | `ProviderPrivacyBoundarySummary.externalTransmission` |
| blocked reason | `ProviderPrivacyBoundarySummary.blockedReason` |
| local model row and support copy | `LAYOUT_FIXTURE` unless Settings ViewModel owns it |

## Anti-Inference Rule

The production frontend may format these fields, but it may not combine raw
proposal, diagnostics, provider config, or task fragments to reconstruct the
truth described above. Missing fields remain unknown and unsafe actions remain
disabled.
