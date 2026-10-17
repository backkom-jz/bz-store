#!/usr/bin/env bash
set -euo pipefail
"$(dirname "$0")/test-deploy-config.sh"
python3 "$(dirname "$0")/backup/test_mysql_defaults.py"
cd "$(dirname "$0")/.."
if [[ -x .runtime/node24/node_modules/.bin/node ]]; then
  export PATH="$PWD/.runtime/node24/node_modules/.bin:$PATH"
fi
export NUXT_TELEMETRY_DISABLED=1
node --test scripts/test-cloud-config.test.mjs scripts/e2e-runner.test.mjs
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
cargo test --workspace --bins
npm run typecheck
npm run test:unit
npm run build --workspace @devstore/web
npm run build --workspace @devstore/admin
