# OpenLife Repository Cleanup

Status: active

Current slice: C4 - Workbench read model. C0-C3 are complete.

## Objective

Remove obsolete repository surfaces and runtime wiring left behind by earlier
product stages while preserving the canonical Chat/Work Agent harness and its
current product behavior. The result must have one production owner per
concern, a strict release/dev/test boundary, and a directory tree whose current
purpose can be explained from source.

## Product Contract

- Shipped routes are `/workspace`, `/life-model`, and `/settings`.
- Chat and Work retain the canonical Conversation and Task/Run/Item/Artifact
  owners defined by ADR 0018 and ADR 0019.
- Tasks and Review remain backend facts presented in the Workbench, not
  duplicate top-level products.
- Memory and LifeModel remain narrow collaborators of the Agent harness.
- No cleanup may turn missing evidence into success or bypass governed durable
  effects.

## In Scope

1. Delete reproducible local build artifacts and isolated QA/dev test data that
   no longer supports an active verification run.
2. Delete proven orphan frontend, Rust, fixture, dependency, command, and
   documentation surfaces.
3. Remove unused metrics, provider-cache, legacy reasoning, evidence-graph,
   proactive, Plugin, and old import/export owners after migrating any narrow
   live consumer.
4. Retain production MCP execution while removing unused management/template
   surfaces and separating release from dev-only IPC.
5. Make one backend-composed Workbench read model the page snapshot and remove
   overlapping frontend reads and migration-era status contracts.
6. Replace stage-named and source-structure tests with current product,
   reachability, and behavior contracts.
7. Align stable documentation, CI, and GitHub required checks with the final
   source tree.

## Out of Scope

- Deleting or rewriting the default Application Support profile or Keychain.
- Building a new canonical backup/import product.
- Restoring retired Today, Tasks, or Review routes.
- Rebuilding Plugin support or expanding MCP capability.
- Unrelated visual redesign or feature development.
- Compatibility for unpublished internal Rust APIs or obsolete test profiles.

## Execution

### C0 - Safety and generated artifacts

- Verify no repository-built application or build process is using generated
  targets.
- Inspect legacy import-journal presence by metadata only.
- Delete reproducible Cargo, frontend, Tauri schema, coverage, and browser-test
  output.
- Preserve ignored credentials and personal notes; restrict local `.env`
  permissions to the owner.

### C1 - Proven orphans

- Remove broken phase development entries, unreachable frontend modules, dead
  mocks, unused dependencies, and unregistered commands.
- Move any still-useful negative fixtures out of repository-root dogfood or
  skill discovery paths, then delete those roots.

### C2 - Retired backend bundles

- Remove rollout metrics and unused provider status/router cache.
- Move the minimum live DTO/helper out of legacy reasoning/proactive modules,
  then delete the retired implementations.
- Remove the unused evidence graph and Plugin vertical.
- Remove legacy import/export recovery and journal wiring without touching
  unknown default-profile files.

### C3 - Release and dev boundaries

- Keep MCP execution, typed manifests, permissions, and audit writes.
- Remove unused MCP templates and management/audit IPC.
- Split product and dev-only frontend clients.
- Require every Tauri command to be release, dev-only, or explicitly internal.

### C4 - Workbench read model

- Compose Conversation, scoped Workspace, and provider-boundary lanes in one
  backend Workbench ViewModel without creating a new store.
- Preserve independent lane state and revision facts.
- Remove duplicate Tasks, Review, Boundary, and empty-conversation frontend
  loads and replace migration status with typed availability.

### C5 - Documents, tests, and CI

- Remove completed stage matrices and duplicate CI jobs after updating external
  required checks.
- Keep and generalize exact macOS bundle verification and authorized live
  evaluation.
- Remove superseded working-tree decisions and update stable source maps.
- Delete merged local development branches while retaining historical safety
  tags.

### C6 - Verification and reverse audit

- Run formatting, lint, Rust, frontend, coverage, security, build, and browser
  shell gates proportional to the final diff.
- Rebuild from the cleaned source tree.
- Verify the three shipped surfaces through an exact native fresh QA profile.
- Run external-live checks only for execution paths materially changed.
- Re-index the final tree and prove that every release command, store, dev
  surface, script, and stable document has a current owner.

## Stop Conditions

- If a proposed deletion still has a live consumer, migrate the consumer before
  deletion.
- If migration requires a new product decision, stop that branch of work rather
  than inventing a compatibility layer.
- Unknown default-profile data is never deleted as repository cleanup.
- A green controlled test is not native or external-live evidence.

## Completion

Complete only when C0-C6 are closed, the worktree contains no active historical
stage machinery, full required checks pass, and the final directory/release
reachability audit finds no unexplained owner.
