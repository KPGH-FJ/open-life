#!/usr/bin/env bash
set -euo pipefail

readonly REGISTRY_PATH="plans/openlife_backend_remediation_v4_discovered_findings.json"
readonly REQUESTED_BASE_REVISION="${1:-}"
readonly CURRENT_REGISTRY_FILE="${OPENLIFE_REGISTRY_CURRENT_FILE:-$REGISTRY_PATH}"
readonly TRUSTED_FINDING_PREFIX_COUNT=53
readonly TRUSTED_FINDING_PREFIX_SHA256="f1bd733a0faf7d50e89b67988dec078621c632dd0d1274e5a754d0010974eb12"
readonly TRUSTED_CORRECTION_PREFIX_COUNT=2
readonly TRUSTED_CORRECTION_PREFIX_SHA256="3aee333404641837363c230e5635ccfbec98956acda6ce3e8ff2ee705ee1b9d9"

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

fail() {
  echo "registry append-only check failed: $*" >&2
  exit 1
}

command -v jq >/dev/null 2>&1 || fail "jq is required"
test -f "$CURRENT_REGISTRY_FILE" || fail "missing current registry: $CURRENT_REGISTRY_FILE"

sha256_stdin() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | awk '{print $1}'
  else
    shasum -a 256 | awk '{print $1}'
  fi
}

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

compare_registry_prefix() {
  local baseline_file="$1"
  local label="$2"
  local baseline_finding_count baseline_correction_count current_finding_count current_correction_count

  jq -cS '{schema_version,status,authority,created_at,protected_baseline,append_policy}' \
    "$baseline_file" >"$tmpdir/baseline-header.json"
  jq -cS '{schema_version,status,authority,created_at,protected_baseline,append_policy}' \
    "$CURRENT_REGISTRY_FILE" >"$tmpdir/current-header.json"
  cmp -s "$tmpdir/baseline-header.json" "$tmpdir/current-header.json" \
    || fail "$label changed the immutable registry header"

  baseline_finding_count="$(jq '.findings | length' "$baseline_file")"
  current_finding_count="$(jq '.findings | length' "$CURRENT_REGISTRY_FILE")"
  (( current_finding_count >= baseline_finding_count )) \
    || fail "$label deleted discovered findings"
  jq -cS '.findings' "$baseline_file" >"$tmpdir/baseline-findings.json"
  jq -cS --argjson count "$baseline_finding_count" '.findings[:$count]' \
    "$CURRENT_REGISTRY_FILE" >"$tmpdir/current-finding-prefix.json"
  cmp -s "$tmpdir/baseline-findings.json" "$tmpdir/current-finding-prefix.json" \
    || fail "$label rewrote or reordered an existing discovered finding"

  baseline_correction_count="$(jq '(.definition_corrections // []) | length' "$baseline_file")"
  current_correction_count="$(jq '(.definition_corrections // []) | length' "$CURRENT_REGISTRY_FILE")"
  (( current_correction_count >= baseline_correction_count )) \
    || fail "$label deleted definition corrections"
  jq -cS '(.definition_corrections // [])' "$baseline_file" \
    >"$tmpdir/baseline-corrections.json"
  jq -cS --argjson count "$baseline_correction_count" \
    '(.definition_corrections // [])[:$count]' "$CURRENT_REGISTRY_FILE" \
    >"$tmpdir/current-correction-prefix.json"
  cmp -s "$tmpdir/baseline-corrections.json" "$tmpdir/current-correction-prefix.json" \
    || fail "$label rewrote or reordered an existing definition correction"

  jq -e --argjson offset "$baseline_correction_count" --slurpfile baseline "$baseline_file" '
    ($baseline[0].findings | map({key: .id, value: .}) | from_entries) as $by_id
    | ((.definition_corrections // [])[$offset:]) as $new_corrections
    | ([ $new_corrections[]
        | . as $correction
        | if $correction.operation == "remove_invalid_reference" then
            $correction.field == "related_frozen_findings"
            and ($by_id[$correction.finding_id] != null)
            and (($by_id[$correction.finding_id][$correction.field] // [])
              | index($correction.invalid_value) != null)
          elif $correction.operation == "replace_invalid_derived_fingerprints" then
            $correction.field == "immutable_fingerprint"
            and (($correction.replacements | type) == "array")
            and (($correction.replacements | length) > 0)
            and ([ $correction.replacements[] as $replacement
              | ($by_id[$replacement.finding_id] != null)
                and ($by_id[$replacement.finding_id].immutable_fingerprint
                  == $replacement.invalid_value)
            ] | all)
          else
            false
          end
      ] | all)
  ' "$CURRENT_REGISTRY_FILE" >/dev/null \
    || fail "$label added a correction that is not bound to an exact value in the trusted baseline"
}

finding_prefix_sha256="$(
  jq -cS --argjson count "$TRUSTED_FINDING_PREFIX_COUNT" '.findings[:$count]' \
    "$CURRENT_REGISTRY_FILE" | tr -d '\n' | sha256_stdin
)"
test "$finding_prefix_sha256" = "$TRUSTED_FINDING_PREFIX_SHA256" \
  || fail "the trusted first-53 finding prefix changed"

correction_prefix_sha256="$(
  jq -cS --argjson count "$TRUSTED_CORRECTION_PREFIX_COUNT" \
    '(.definition_corrections // [])[:$count]' "$CURRENT_REGISTRY_FILE" \
    | tr -d '\n' | sha256_stdin
)"
test "$correction_prefix_sha256" = "$TRUSTED_CORRECTION_PREFIX_SHA256" \
  || fail "the trusted first-two correction prefix changed"

jq --argjson finding_count "$TRUSTED_FINDING_PREFIX_COUNT" '
  .findings = .findings[:$finding_count]
  | .definition_corrections = []
' "$CURRENT_REGISTRY_FILE" >"$tmpdir/trusted-prefix.json"
compare_registry_prefix "$tmpdir/trusted-prefix.json" "trusted immutable prefix"

if [[ -n "$REQUESTED_BASE_REVISION" && ! "$REQUESTED_BASE_REVISION" =~ ^0+$ ]]; then
  git rev-parse --verify "$REQUESTED_BASE_REVISION^{commit}" >/dev/null \
    || fail "base revision is unavailable: $REQUESTED_BASE_REVISION"
  if git cat-file -e "$REQUESTED_BASE_REVISION:$REGISTRY_PATH" 2>/dev/null; then
    git show "$REQUESTED_BASE_REVISION:$REGISTRY_PATH" >"$tmpdir/merge-base.json"
    compare_registry_prefix "$tmpdir/merge-base.json" "merge base $REQUESTED_BASE_REVISION"
  fi
fi

echo "registry append-only check passed against trusted prefixes${REQUESTED_BASE_REVISION:+ and requested base}"
