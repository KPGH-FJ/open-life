# Phase 4D Privacy And Configuration Source Map And Field Ownership

Status: `IMPLEMENTED`
Date: 2026-07-21

## Runtime Source Map

```text
dev-only Phase 4D Shell
  -> SettingsPrivacyDataSource.loadSettingsPrivacy
      -> get_config
          -> sanitized AppConfig (masked credentials)
      -> get_provider_privacy_boundary_summary
          -> ProviderPrivacyBoundarySummary envelope
  -> useSettingsPrivacyJourney
      -> local draft only
      -> settingsOrchestrationReducer
  -> SettingsPrivacyView

test path
  -> explicit external-target confirmation when required
  -> test_llm_connection(draft)
      -> network-policy decision
      -> optional consent proposal
      -> optional metadata-only ProviderInvocationReceipt
  -> if reviewProposalId exists
      -> get_review_center_view_model
      -> exact items[].source.proposalId match
  -> ReviewGovernedView
      -> typed ReviewAction decision
  -> return to Settings
  -> no automatic test and no automatic save

save path
  -> save_config(draft)
  -> command return (not boundary proof)
  -> loadSettingsPrivacy again
  -> refreshed sanitized AppConfig + ProviderPrivacyBoundarySummary
```

## Backend Owners

- Config read/write and provider test: `src-tauri/src/commands/settings.rs`.
- Provider/privacy projection:
  `src-tauri/src/read_models/provider_privacy.rs`.
- Provider boundary construction:
  `openlife-core/src/agent/provider_privacy_boundary.rs` and
  `openlife-core/src/agent/product_read_model.rs`.
- Provider terminal receipt:
  `openlife-core/src/llm.rs` plus the scheduler/provider adapter path.
- Rich permission context:
  `openlife-core/src/agent/review_decision_context.rs`.
- Review decision authority: existing Review Center read model and typed review
  commands; the Settings page does not approve directly.

## Field Ownership

| Visible fact | Backend owner and field | Rendering rule |
| --- | --- | --- |
| provider, endpoint, model, local preference | sanitized `AppConfig` from `get_config` | editable draft only; no current-route conclusion |
| stored credential presence | masked `llm.openai_key === "***"` | show only “stored”; never render the value |
| current route | `ProviderPrivacyBoundarySummary.routeType` | never derive from provider, URL, or local preference |
| external transmission | `externalTransmission` | unknown remains unknown; failed provider calls may still prove sent |
| risk and local-only requirement | `risk`, `localOnlyRequired` | render independently from convenience settings |
| boundary evidence and warning | envelope/data `evidenceRefs`, `warnings` | preserve id, label, source, and sensitivity in Inspector |
| test result | `LlmConnectionTestResult.validation_status` | connection evidence only; never a save result |
| network decision | original/effective network policy decision IDs | technical evidence; no page-local allow decision |
| one-time consent | `review_proposal_id`, `permission_id`, `consent_status` | resolve exact ReviewItem; missing resolution disables navigation |
| provider completion | non-simulated receipt with `status=completed` | required together with `ok=true` and `validated` for green test status |
| remote uncertainty | `remote_unknown` result or receipt | no automatic retry and no success treatment |
| saved state | refreshed sanitized config after `save_config` | command return alone is insufficient |
| saved boundary | refreshed ProviderPrivacyBoundarySummary | only refreshed known fields may restore a known boundary |

## Credential Identity Rule

The backend binds a masked credential to the normalized provider plus endpoint
identity. `save_config` preserves a masked or empty credential only when that
identity is unchanged. The candidate UI clears the masked credential as soon as
provider or endpoint changes, so an old secret is not visually carried to a new
destination.

## Fixture Field Source Table

| Fixture value | Contract source or fixture rule |
| --- | --- |
| model/provider form fields | `AppConfig.llm`, `prefer_local_model`, `local_model` |
| local/possible/unknown boundary | exact `ProviderPrivacyBoundarySummary` fields |
| consent-required result | exact `LlmConnectionTestResult` fields |
| pending permission details | exact `ReviewItem.decisionContext.permission` shape |
| before/after, reason, impact, expiry | exact rich `ReviewItem.decisionContext` fields |
| verified test | exact non-simulated completed `ProviderInvocationReceipt` fixture |
| save failure and refresh unknown | orchestration-path fixtures, explicitly QA-only |
| fixture timestamps, IDs, and counts | static QA metadata, never backend state |
| visual spacing and section labels | pure layout sample governed by Foundation tokens |

Every fixture is selected outside the product Shell and visibly labelled
`静态 fixture · 非后端状态`.

## Fail-Closed Reconciliation

1. config read failure disables edit, test, and save;
2. boundary read failure cannot be replaced with config-derived truth;
3. any unsaved draft replaces the effective boundary with unknown;
4. provider test success requires an exact non-simulated completed receipt;
5. consent-required means zero provider dispatch and exactly one matching
   ReviewItem; missing or duplicate proposal matches remain disabled;
6. approval only records permission; the user must explicitly test again;
7. save success enters boundary refresh, not ready;
8. missing, stale, error, or unknown refreshed boundary remains non-green.
