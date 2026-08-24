#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FRONTEND_DIR="$REPO_ROOT/frontend"

DRY_RUN=0
INCLUDE_WEBVIEW_CACHE=0

usage() {
    cat <<'EOF'
Usage: scripts/clean-ui-artifacts.sh [--dry-run] [--include-webview-cache]

Default clean targets:
  frontend/dist
  target/debug/bundle/macos/OpenLife.app
  target/debug/bundle/macos/OpenLife.app.stale-*

Never cleaned by this script:
  ~/Library/Application Support/ai.openlife.desktop
  *.db files
  LifeModel, memory, proposal, or agent run data
  release bundles
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run)
            DRY_RUN=1
            ;;
        --include-webview-cache)
            INCLUDE_WEBVIEW_CACHE=1
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown flag: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
    shift
done

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is required to resolve the workspace target directory" >&2
    exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
    echo "python3 is required to parse cargo metadata" >&2
    exit 1
fi

TARGET_DIR="$(cd "$REPO_ROOT" && cargo metadata --format-version=1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"

declare -a PATHS=()

add_path() {
    PATHS+=("$1")
}

add_existing_glob() {
    local pattern="$1"
    local matched=0
    while IFS= read -r path; do
        matched=1
        add_path "$path"
    done < <(find "$(dirname "$pattern")" -maxdepth 1 -type d -name "$(basename "$pattern")" -print 2>/dev/null | sort)
    if [ "$matched" -eq 0 ]; then
        add_path "$pattern"
    fi
}

add_path "$FRONTEND_DIR/dist"
add_path "$TARGET_DIR/debug/bundle/macos/OpenLife.app"
add_existing_glob "$TARGET_DIR/debug/bundle/macos/OpenLife.app.stale-*"

if [ "$INCLUDE_WEBVIEW_CACHE" -eq 1 ]; then
    case "$(uname -s)" in
        Darwin*)
            add_path "$HOME/Library/Caches/ai.openlife.desktop"
            add_path "$HOME/Library/WebKit/ai.openlife.desktop"
            add_path "$HOME/Library/Saved Application State/ai.openlife.desktop.savedState"
            ;;
        Linux*)
            add_path "${XDG_CACHE_HOME:-$HOME/.cache}/ai.openlife.desktop"
            ;;
        *)
            echo "No WebView cache paths configured for $(uname -s)" >&2
            ;;
    esac
fi

assert_safe_path() {
    local path="$1"
    if [ -z "$path" ] || [ "$path" = "/" ] || [ "$path" = "$HOME" ] || [ "$path" = "$REPO_ROOT" ]; then
        echo "Refusing unsafe clean path: $path" >&2
        exit 1
    fi
    case "$path" in
        "$HOME/Library/Application Support/ai.openlife.desktop"|"$HOME/Library/Application Support/ai.openlife.desktop/"*)
            echo "Refusing to clean Application Support data: $path" >&2
            exit 1
            ;;
        *.db|*.db/*|*/.db|*/.db/*)
            echo "Refusing to clean database path: $path" >&2
            exit 1
            ;;
    esac
}

for path in "${PATHS[@]}"; do
    assert_safe_path "$path"
    if [ "$DRY_RUN" -eq 1 ]; then
        echo "DRY RUN remove: $path"
    elif [ -e "$path" ]; then
        echo "Removing: $path"
        rm -rf -- "$path"
    else
        echo "Missing: $path"
    fi
done
