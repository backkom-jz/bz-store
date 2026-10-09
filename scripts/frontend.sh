#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -x .runtime/node24/node_modules/.bin/node ]]; then
  export PATH="$PWD/.runtime/node24/node_modules/.bin:$PATH"
fi
export NUXT_TELEMETRY_DISABLED=1
npm "$@"
