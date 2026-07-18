# Phase 2 Scope and Constraints

## Goal

Define backend-owned ViewModel / ReadModel contracts for Frontend V2.

## Non-goals

- No React implementation
- No routes
- No CSS
- No backend schema migration
- No new Tauri commands
- No store implementation
- No mock API pretending to be product truth

## Source-of-truth Rule

Pages cannot reconstruct product truth from raw domain reads.

## Product Capability Preservation Rule

Important but incomplete capabilities should be marked CANDIDATE / PHASE_2_REQUIRED, not deleted.

## No Fake Backend Contract Rule

Do not claim a backend owner/read model exists unless current code or Phase 0/0.5/1 evidence verifies it.

## Phase 3 Entry Boundary

Phase 3 begins only after humans approve ViewModel owners, contract gaps, and first vertical slice.
