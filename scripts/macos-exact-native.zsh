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
action="build-verify"

case "${1:-}" in
  "") ;;
  --verify-only) action="verify-built" ;;
  --install) action="build-install" ;;
  --verify-installed) action="verify-installed" ;;
  *)
    print -u2 "usage: OPENLIFE_CODESIGN_IDENTITY=<identity> $0 [--verify-only|--install|--verify-installed]"
    print -u2 "--install and --verify-installed also require an explicit absolute OPENLIFE_INSTALL_PATH"
    exit 64
    ;;
esac

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
expected_version="$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')"
expected_git_sha="$(git rev-parse --short=12 HEAD)"
install_path="${OPENLIFE_INSTALL_PATH:-}"
build_source_state="unknown"
native_build_nonce=""

if [[ "$action" == "build-verify" || "$action" == "build-install" ]]; then
  if [[ -n "$(git status --porcelain --untracked-files=normal)" ]]; then
    build_source_state="dirty"
  else
    build_source_state="clean"
  fi
  native_build_nonce="$(date -u +%Y%m%dT%H%M%SZ)-$$"
fi

if [[ "$native_profile" == "release" && "$action" == build-* ]]; then
  if [[ "$build_source_state" != "clean" ]]; then
    print -u2 "formal release build requires a clean committed checkout; current source contains uncommitted or untracked files"
    exit 74
  fi
fi

if [[ "$action" == "build-install" || "$action" == "verify-installed" ]]; then
  if [[ -z "$install_path" || "$install_path" != /* || "${install_path:t}" != "$expected_product_name.app" ]]; then
    print -u2 "OPENLIFE_INSTALL_PATH must be an explicit absolute path ending in $expected_product_name.app"
    exit 64
  fi
  if [[ -L "$install_path" ]]; then
    print -u2 "refusing a symlinked install target: $install_path"
    exit 65
  fi
fi

running_profile_processes="$(pgrep -fl "$running_profile_pattern" || true)"
if [[ -n "$running_profile_processes" ]]; then
  print -u2 "close the running $expected_product_name app before building or verifying its exact native bundle"
  print -u2 "$running_profile_processes"
  exit 73
fi

if [[ "$action" == "build-verify" || "$action" == "build-install" ]]; then
  signing_config="$(OPENLIFE_CODESIGN_IDENTITY="$signing_identity" python3 -c 'import json,os; print(json.dumps({"bundle":{"macOS":{"signingIdentity":os.environ["OPENLIFE_CODESIGN_IDENTITY"]}}}))')"
  if [[ -n "$profile_config" ]]; then
    OPENLIFE_BUILD_PROFILE="$native_profile" \
      OPENLIFE_BUILD_SOURCE_STATE="$build_source_state" \
      OPENLIFE_NATIVE_BUILD_NONCE="$native_build_nonce" \
      frontend/node_modules/.bin/tauri build \
      --bundles app \
      --config "$profile_config" \
      --config "$signing_config"
  else
    OPENLIFE_BUILD_PROFILE="$native_profile" \
      OPENLIFE_BUILD_SOURCE_STATE="$build_source_state" \
      OPENLIFE_NATIVE_BUILD_NONCE="$native_build_nonce" \
      frontend/node_modules/.bin/tauri build \
      --bundles app \
      --config "$signing_config"
  fi
fi

if [[ ! -d "$app_path" ]]; then
  print -u2 "exact macOS application bundle not found: $app_path"
  exit 66
fi

verify_bundle() {
  local bundle_path="$1"
  local evidence_prefix="$2"
  local info_plist="$bundle_path/Contents/Info.plist"
  local actual_bundle_id actual_version executable_name executable_path codesign_output designated_requirement executable_hash
  actual_bundle_id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$info_plist")"
  actual_version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$info_plist")"
  executable_name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$info_plist")"
  executable_path="$bundle_path/Contents/MacOS/$executable_name"
  if [[ "$actual_bundle_id" != "$expected_bundle_id" || "$actual_version" != "$expected_version" || ! -x "$executable_path" ]]; then
    print -u2 "bundle identity mismatch for $bundle_path"
    print -u2 "expected_bundle_id=$expected_bundle_id actual_bundle_id=$actual_bundle_id expected_version=$expected_version actual_version=$actual_version executable=$executable_path"
    exit 65
  fi
  codesign_output="$(codesign -dv --verbose=4 "$bundle_path" 2>&1)"
  if ! grep -Fq "Identifier=$expected_bundle_id" <<<"$codesign_output" || ! grep -Fq "Authority=$signing_identity" <<<"$codesign_output"; then
    print -u2 "code signature identity mismatch for $bundle_path"
    print -u2 "$codesign_output"
    exit 65
  fi
  codesign --verify --deep --strict --verbose=2 "$bundle_path"
  designated_requirement="$(codesign -dr - "$bundle_path" 2>&1)"
  if ! grep -Fq "identifier \"$expected_bundle_id\"" <<<"$designated_requirement"; then
    print -u2 "designated requirement does not bind the expected bundle identifier"
    print -u2 "$designated_requirement"
    exit 65
  fi
  if ! strings "$executable_path" | grep -F "$expected_git_sha" >/dev/null; then
    print -u2 "runtime executable does not contain the expected build commit: $expected_git_sha"
    exit 65
  fi
  executable_hash="$(shasum -a 256 "$executable_path" | awk '{print $1}')"
  print "${evidence_prefix}_BUNDLE=$bundle_path"
  print "${evidence_prefix}_BUNDLE_ID=$actual_bundle_id"
  print "${evidence_prefix}_VERSION=$actual_version"
  print "${evidence_prefix}_EXECUTABLE=$executable_path"
  print "${evidence_prefix}_EXECUTABLE_SHA256=$executable_hash"
  print "${evidence_prefix}_BUILD_COMMIT=$expected_git_sha"
  print "${evidence_prefix}_SIGNING_AUTHORITY=$signing_identity"
  print "${evidence_prefix}_DESIGNATED_REQUIREMENT=$designated_requirement"
  print "${evidence_prefix}_RESOURCE_SEAL=verified"
}

verify_bundle "$app_path" "OPENLIFE_BUILT"
built_executable_name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$app_path/Contents/Info.plist")"
built_executable="$app_path/Contents/MacOS/$built_executable_name"
built_executable_hash="$(shasum -a 256 "$built_executable" | awk '{print $1}')"

if [[ "$action" == "build-install" ]]; then
  install_parent="${install_path:h}"
  if [[ ! -d "$install_parent" || -L "$install_parent" ]]; then
    print -u2 "install parent must be an existing non-symlink directory: $install_parent"
    exit 66
  fi
  install_temp="$install_path.installing.$$"
  install_backup=""
  if [[ -e "$install_temp" || -L "$install_temp" ]]; then
    print -u2 "refusing to reuse an existing install staging path: $install_temp"
    exit 73
  fi
  restore_install_on_failure() {
    local status=$?
    if [[ -e "$install_temp" ]]; then
      rm -rf -- "$install_temp"
    fi
    if [[ $status -ne 0 && -n "$install_backup" && ! -e "$install_path" && -e "$install_backup" ]]; then
      mv -- "$install_backup" "$install_path"
    fi
    return $status
  }
  trap restore_install_on_failure EXIT
  ditto "$app_path" "$install_temp"
  verify_bundle "$install_temp" "OPENLIFE_STAGED_INSTALL"
  if [[ -e "$install_path" ]]; then
    install_backup="$install_path.backup.$(date -u +%Y%m%dT%H%M%SZ)"
    if [[ -e "$install_backup" ]]; then
      print -u2 "refusing to overwrite an existing install backup: $install_backup"
      exit 73
    fi
    mv -- "$install_path" "$install_backup"
  fi
  mv -- "$install_temp" "$install_path"
  verify_bundle "$install_path" "OPENLIFE_INSTALLED"
  installed_executable_name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$install_path/Contents/Info.plist")"
  installed_executable="$install_path/Contents/MacOS/$installed_executable_name"
  installed_executable_hash="$(shasum -a 256 "$installed_executable" | awk '{print $1}')"
  if [[ "$installed_executable_hash" != "$built_executable_hash" ]]; then
    print -u2 "installed executable hash differs from the verified build"
    exit 65
  fi
  trap - EXIT
  print "OPENLIFE_INSTALL_BACKUP=${install_backup:-none}"
  print "OPENLIFE_INSTALLED_BUILD_HASH_MATCH=verified"
elif [[ "$action" == "verify-installed" ]]; then
  if [[ ! -d "$install_path" ]]; then
    print -u2 "installed application bundle not found: $install_path"
    exit 66
  fi
  verify_bundle "$install_path" "OPENLIFE_INSTALLED"
  installed_executable_name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$install_path/Contents/Info.plist")"
  installed_executable="$install_path/Contents/MacOS/$installed_executable_name"
  installed_executable_hash="$(shasum -a 256 "$installed_executable" | awk '{print $1}')"
  if [[ "$installed_executable_hash" != "$built_executable_hash" ]]; then
    print -u2 "installed executable hash differs from the verified build"
    exit 65
  fi
  print "OPENLIFE_INSTALLED_BUILD_HASH_MATCH=verified"
fi

print "OPENLIFE_EXACT_NATIVE_BUNDLE=$app_path"
print "OPENLIFE_EXACT_NATIVE_PROFILE=$native_profile"
print "OPENLIFE_EXACT_NATIVE_BUNDLE_ID=$expected_bundle_id"
print "OPENLIFE_EXACT_NATIVE_SOURCE_STATE=$build_source_state"
print "OPENLIFE_EXACT_NATIVE_SIGNING_AUTHORITY=$signing_identity"
print "OPENLIFE_EXACT_NATIVE_RESOURCE_SEAL=verified"
