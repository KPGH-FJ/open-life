#!/usr/bin/env bash

set -euo pipefail

task_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
task_repo_dir="$(cd "${task_script_dir}/.." && pwd)"

cd "${task_repo_dir}"

cargo check -p openlife-core --locked
cargo check -p openlife-tauri --locked
cargo test -p openlife-core --lib --locked
cargo test -p openlife-tauri --lib --locked
(
  cd frontend
  corepack pnpm test
)
