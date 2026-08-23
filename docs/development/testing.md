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
