#!/bin/zsh
set -euo pipefail

repo_root="${0:A:h:h}"
cd "$repo_root"

native_profile="${OPENLIFE_NATIVE_PROFILE:-release}"
case "$native_profile" in
  release)
    expected_bundle_id="ai.openlife.desktop"
    expected_product_name="OpenLife"
    running_profile_pattern='/OpenLife\.app/Contents/MacOS/openlife-tauri($| )'
    profile_config=""
    ;;
  qa)
    expected_bundle_id="ai.openlife.desktop.qa"
    expected_product_name="OpenLife QA"
    running_profile_pattern='/OpenLife QA\.app/Contents/MacOS/openlife-tauri($| )'
    profile_config="$repo_root/src-tauri/tauri.qa.conf.json"
    ;;
  *)
    print -u2 "OPENLIFE_NATIVE_PROFILE must be release or qa"
    exit 64
    ;;
esac
signing_identity="${OPENLIFE_CODESIGN_IDENTITY:-}"
verify_only=0

if [[ "${1:-}" == "--verify-only" ]]; then
  verify_only=1
elif [[ $# -gt 0 ]]; then
  print -u2 "usage: OPENLIFE_CODESIGN_IDENTITY=<identity> $0 [--verify-only]"
  exit 64
fi

if [[ -z "$signing_identity" ]]; then
  print -u2 "OPENLIFE_CODESIGN_IDENTITY is required"
  exit 64
fi

if ! command -v python3 >/dev/null 2>&1; then
  print -u2 "python3 is required to encode the Tauri signing override"
  exit 69
fi

if ! security find-identity -v -p codesigning | grep -Fq '"'"$signing_identity"'"'; then
  print -u2 "the requested macOS code-signing identity is unavailable: $signing_identity"
  exit 69
fi

target_dir="$(cargo metadata --format-version=1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
app_path="$target_dir/release/bundle/macos/$expected_product_name.app"

running_profile_processes="$(pgrep -fl "$running_profile_pattern" || true)"
if [[ -n "$running_profile_processes" ]]; then
  print -u2 "close the running $expected_product_name app before building or verifying its exact native bundle"
  print -u2 "$running_profile_processes"
  exit 73
fi

if [[ $verify_only -eq 0 ]]; then
  signing_config="$(OPENLIFE_CODESIGN_IDENTITY="$signing_identity" python3 -c 'import json,os; print(json.dumps({"bundle":{"macOS":{"signingIdentity":os.environ["OPENLIFE_CODESIGN_IDENTITY"]}}}))')"
  if [[ -n "$profile_config" ]]; then
    OPENLIFE_BUILD_PROFILE="$native_profile" frontend/node_modules/.bin/tauri build \
      --bundles app \
      --config "$profile_config" \
      --config "$signing_config"
  else
    OPENLIFE_BUILD_PROFILE="$native_profile" frontend/node_modules/.bin/tauri build \
      --bundles app \
      --config "$signing_config"
  fi
fi

if [[ ! -d "$app_path" ]]; then
  print -u2 "exact macOS application bundle not found: $app_path"
  exit 66
fi

actual_bundle_id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app_path/Contents/Info.plist")"
if [[ "$actual_bundle_id" != "$expected_bundle_id" ]]; then
  print -u2 "bundle identifier mismatch: expected=$expected_bundle_id actual=$actual_bundle_id"
  exit 65
fi

codesign_output="$(codesign -dv --verbose=4 "$app_path" 2>&1)"
if ! grep -Fq "Identifier=$expected_bundle_id" <<<"$codesign_output"; then
  print -u2 "code signature is not bound to $expected_bundle_id"
  print -u2 "$codesign_output"
  exit 65
fi
if ! grep -Fq "Authority=$signing_identity" <<<"$codesign_output"; then
  print -u2 "code signature does not use the requested authority: $signing_identity"
  print -u2 "$codesign_output"
  exit 65
fi

codesign --verify --deep --strict --verbose=2 "$app_path"
designated_requirement="$(codesign -dr - "$app_path" 2>&1)"
if ! grep -Fq "identifier \"$expected_bundle_id\"" <<<"$designated_requirement"; then
  print -u2 "designated requirement does not bind the expected bundle identifier"
  print -u2 "$designated_requirement"
  exit 65
fi

print "OPENLIFE_EXACT_NATIVE_BUNDLE=$app_path"
print "OPENLIFE_EXACT_NATIVE_PROFILE=$native_profile"
print "OPENLIFE_EXACT_NATIVE_BUNDLE_ID=$actual_bundle_id"
print "OPENLIFE_EXACT_NATIVE_SIGNING_AUTHORITY=$signing_identity"
print "OPENLIFE_EXACT_NATIVE_DESIGNATED_REQUIREMENT=$designated_requirement"
print "OPENLIFE_EXACT_NATIVE_RESOURCE_SEAL=verified"
