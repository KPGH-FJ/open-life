#!/usr/bin/env zsh
set -euo pipefail

repo_root="${0:A:h:h}"
profile_path="${OPENLIFE_LIVE_EVAL_PROFILE:-$repo_root/.env.live.local}"

if [[ ! -f "$profile_path" ]]; then
  print -u2 "missing live eval profile: $profile_path"
  print -u2 "copy .env.live.example to .env.live.local and fill real provider values"
  exit 2
fi

set -a
source "$profile_path"
set +a

required=(
  OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL
  OPENLIFE_LIVE_EVAL_PROVIDER
  OPENLIFE_LIVE_EVAL_BASE
  OPENLIFE_LIVE_EVAL_MODEL
  OPENLIFE_LIVE_EVAL_API_KEY
)

for key in "${required[@]}"; do
  if [[ -z "${(P)key:-}" ]]; then
    print -u2 "missing required live eval env: $key"
    exit 2
  fi
done

if [[ "$OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL" != "1" ]]; then
  print -u2 "OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL must be 1"
  exit 2
fi

case "$OPENLIFE_LIVE_EVAL_BASE" in
  *localhost*|*127.0.0.1*|*0.0.0.0*|*::1*|*mock*|*fixture*|*synthetic*|*scripted*|*ollama*)
    print -u2 "OPENLIFE_LIVE_EVAL_BASE does not look like an external live provider"
    exit 2
    ;;
esac

if (( $# == 0 )); then
  print -u2 "usage: scripts/live-eval.zsh <gate command> [args...]"
  exit 2
fi

exec "$@"
