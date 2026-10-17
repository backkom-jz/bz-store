#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

mode="${1:-all}"
case "$mode" in
  --check-only|integration|e2e|all) ;;
  *) echo 'usage: scripts/test-cloud.sh [--check-only|integration|e2e|all]' >&2; exit 2 ;;
esac

node --test scripts/test-cloud-config.test.mjs
node scripts/cloud-test-config.mjs
if [[ "$mode" == --check-only ]]; then
  exit 0
fi

runtime_env="$(node scripts/cloud-test-config.mjs --shell)"
eval "$runtime_env"
if [[ "$mode" == integration || "$mode" == all ]]; then
  bash scripts/test-server.sh all
fi
if [[ "$mode" == e2e || "$mode" == all ]]; then
  bash scripts/test-e2e.sh
fi
