#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -x .runtime/node24/node_modules/.bin/node ]]; then
  export PATH="$PWD/.runtime/node24/node_modules/.bin:$PATH"
fi
export NUXT_TELEMETRY_DISABLED=1
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
docker compose -f deploy/docker-compose.test.yml up -d --wait --wait-timeout 180 postgres redis storage clamav
bash scripts/test-server.sh all
npm run typecheck
npm run test:unit
npm run build --workspace @devstore/web
npm run build --workspace @devstore/admin
