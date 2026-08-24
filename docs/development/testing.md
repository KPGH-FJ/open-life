# Testing OpenLife

Use the smallest check that matches the change, then broaden before merging.

## Fast checks

```sh
git diff --check
cargo fmt --check
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
```

## Product checks

```sh
cargo clippy --all --locked -- -D warnings
cargo test --all --locked
corepack pnpm --dir frontend test
corepack pnpm --dir frontend build
corepack pnpm --dir frontend test:e2e
```

CI runs the Rust and frontend gates, browser-shell smoke, platform compilation,
coverage, and dependency audit as separate checks. It does not repeat the same
unit suites through a second wrapper job.

## Bounded cache cleanup

Use `make clean` for regenerated frontend output and local test reports. Use
`make clean-rust-target` for Cargo development/test artifacts; it is
profile-scoped and leaves release bundles intact. For native UI cleanup,
`scripts/clean-ui-artifacts.sh --dry-run` shows the exact default targets before
removal. The script never targets release bundles or Application Support data;
WebView caches require the separate `--include-webview-cache` option.

Do not use a repository-wide ignored-file clean as a cache cleanup mechanism.
Credentials, local profiles, SQLite databases, QA receipts, user files, and
release bundles are not build caches.

## Controlled Agent behavior matrix

Run the current focused product matrix with:

```sh
scripts/agent-behavior-matrix.zsh
```

The matrix covers canonical Chat and Work, replay and failure, document, Web,
Skill and read-only MCP tools, planning, steering, review, materialization,
cancellation, retry, recovery, Artifact verification and Undo, Personal
Intelligence ports, product diagnostics, and the Workbench projection. Every
row first verifies that its named test exists, so a stale test filter cannot
produce a false pass.

Focused Artifact integration coverage also exercises replacement pre-change
snapshot retention and a separately reviewed restore with exact digest
preconditions. This proves the controlled store/runtime/materializer path, not
the formally installed native product.

Focused revision coverage creates a second Run from one verified current
ArtifactVersion, proves the original file remains unchanged before replacement
Review, materializes v2 only after approval, and checks that v1 plus both Run
FinalResults remain queryable. Migration coverage rebuilds the former
task-keyed FinalResult table into per-Run history. A separate regression keeps
first-decision direct Artifacts behind the same independent semantic verifier
as planned Artifact generation.

This is controlled evidence only. It does not prove native Tauri behavior or a
real external provider/Web route.

## Evidence levels

- Unit and contract tests prove only their named code contract.
- Browser-shell tests prove React routing and rendering with controlled data,
  not native Tauri integration.
- Exact-native evidence proves the exact signed local bundle and isolated
  profile that was exercised.
- Scripted and local HTTP providers are not external-live providers.
- External-live evidence requires an explicitly authorized run against the
  selected real provider or Web route.

Reasoning controls require three separate controlled contracts before native
acceptance: an exact provider/model capability-table test, a composer-to-Turn
binding and restart-persistence test, and an adapter-edge HTTP shape test that
observes `reasoning_effort` and `max_completion_tokens` while legacy
`temperature`/`max_tokens` are absent. A rendered selector alone is not runtime
evidence, and a local HTTP capture is not proof that an external model accepted
the parameter.

New-Conversation admission tests must prove that Project and Memory mode are
persisted before the first Turn, and that an archived or unknown Project leaves
no partial Conversation. Frontend coverage must exercise Memory selection while
no Conversation exists and verify the complete admission passed to the Tauri
bridge.

A blocked prerequisite must not return success, and a passing lower-level test
must not be used as evidence for a broader layer. Tests use synthetic resources
under `test-fixtures/`; they must not read real application data, Keychain
contents, or private user files.

## Exact-native macOS builds

Native review uses the current Tauri bundle and a purpose-specific profile:

```sh
OPENLIFE_CODESIGN_IDENTITY="OpenLife Local Code Signing" \
  scripts/macos-exact-native.zsh

OPENLIFE_NATIVE_PROFILE=qa \
OPENLIFE_CODESIGN_IDENTITY="OpenLife Local Code Signing" \
  scripts/macos-exact-native.zsh
```

Release expects `ai.openlife.desktop`; QA expects
`ai.openlife.desktop.qa`. The verifier checks the selected signing identity,
bundle identifier, Designated Requirement binding, and strict deep resource
seal. It never reads secret values.

Release uses the product Keychain service. Dev and QA use separate identities,
data directories, and atomic `0600` local profile secret files. A Provider or
Search credential is never copied between profiles and must be entered
explicitly in the profile that will use it. This is a development boundary, not
proof that a future distributed release preserves Keychain access across
signed updates.

### Legacy LifeModel migration rehearsal

Run migration QA only against an explicit isolated `OPENLIFE_DATA_DIR`. Before
capturing or restoring a profile, close the exact app using that directory,
checkpoint every SQLite database, require `PRAGMA integrity_check` to return
`ok`, and then take a byte-preserving directory snapshot. A restore rehearsal
must preserve the migrated profile under a separate evidence path and restore
the pre-migration snapshot into an absent target directory; it must not merge
the two profiles or overwrite a running profile.

The native acceptance path is:

1. launch the current QA bundle with the isolated directory and confirm the
   legacy inventory is shown instead of an empty canonical LifeModel;
2. decide every migration candidate and acknowledge every non-LifeModel field;
3. confirm that drafting creates only a Review item;
4. approve through the native high-risk confirmation and require the refreshed
   Review read model to report `applied`;
5. restart the same bundle and confirm canonical version, content, history, and
   Review materialization are recovered;
6. close the app, checkpoint and integrity-check again, restore the
   pre-migration snapshot, and confirm a restart returns to migration-required
   state with no canonical v2 owner.

Never use the release profile for this rehearsal. Migrating real release data
requires the user's item-by-item Review; automated tests must not decide which
personal facts belong in LifeModel.

## External-live evaluation

External-live tests remain opt-in:

```sh
# Canonical document + Web + Review + materialization contract. This wrapper
# fails if the exact ignored test is missing, so a zero-test run cannot count.
scripts/agent-external-live.zsh

# Other explicitly gated live tests:
scripts/live-eval.zsh cargo test -p openlife-tauri --locked \
  <ignored-live-test-name> -- --ignored --nocapture
```

`scripts/live-eval.zsh` requires a separately configured live profile and
rejects localhost, mock, fixture, scripted, and Ollama endpoints. Never retain
credentials, provider payloads, private resource bodies, or generated private
content in plans or test summaries.

Agent capability acceptance prompts must resemble user language. A prompt that
names an internal capability such as `web.search`, prescribes the implementation
plan, and requests every checkpoint is only a narrow transport probe; it cannot
prove semantic task understanding. The external-live Work case therefore asks
for an official-site research deliverable without naming a tool, then verifies
the persisted Plan, exact source-domain constraint, real tool receipts,
request-scoped citations, Review stop condition, and one confirmed
materialization. Ordinary new-file behavior is covered separately and must not
inherit a Review checkpoint unless the user requested it or the target effect
requires it.

Rerun exact-native or external-live evidence only when the corresponding
runtime, identity/profile boundary, provider, network, Review, or materializer
path changed. Otherwise record it as not required rather than upgrading older
evidence to the current build.
