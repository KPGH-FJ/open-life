#!/usr/bin/env zsh
set -euo pipefail

repo_root="${0:A:h:h}"
full_test="canonical_work_runtime::tests::external_live_document_web_report_waits_for_review_then_materializes_once"

if [[ "${1:-}" != "--profile-loaded" ]]; then
  exec "$repo_root/scripts/live-eval.zsh" "$0" --profile-loaded
fi

cd "$repo_root"

listed="$({
  cargo test -p openlife-tauri --locked "$full_test" -- --ignored --exact --list
} 2>&1)"
match_count="$(print -r -- "$listed" | grep -F -c "$full_test: test" || true)"
if [[ "$match_count" != "1" ]]; then
  print -u2 "external-live contract missing or ambiguous: expected one exact test, found $match_count"
  exit 3
fi

print "AGENT_EXTERNAL_LIVE_TEST=$full_test"
print "AGENT_EXTERNAL_LIVE_EVIDENCE_LEVEL=external-live"
cargo test -p openlife-tauri --locked "$full_test" -- --ignored --exact --nocapture
