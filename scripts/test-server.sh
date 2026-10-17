#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
runtime_env="$(node scripts/cloud-test-config.mjs --shell)"
eval "$runtime_env"
if [[ "${1:-all}" == all ]]; then
  cargo test --workspace -- --test-threads=1
else
  cargo test --package devstore-server --test "$1" -- --test-threads=1
fi
