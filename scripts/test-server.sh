#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -z "${MYSQL_TEST_DATABASE_URL:-}" ]]; then
  echo "MYSQL_TEST_DATABASE_URL must target the isolated bz_ztore_test database" >&2
  exit 2
fi
export DATABASE_URL="$MYSQL_TEST_DATABASE_URL"
export REDIS_URL="${REDIS_URL:-redis://127.0.0.1:16379}"
export S3_ENDPOINT="${S3_ENDPOINT:-http://127.0.0.1:19000}"
export S3_ACCESS_KEY="${S3_ACCESS_KEY:-devstore_test}"
export S3_SECRET_KEY="${S3_SECRET_KEY:-devstore_test_only}"
export S3_BUCKET="${S3_BUCKET:-devstore-test}"
export CLAMAV_ADDR="${CLAMAV_ADDR:-127.0.0.1:13310}"
export APP_ENV=test
export SITE_URL=http://127.0.0.1:18088
export JWT_SECRET=devstore_test_jwt_key_at_least_32_bytes_long
export CSRF_SECRET=devstore_test_csrf_key_at_least_32_bytes_long
if [[ "${1:-all}" == all ]]; then
  cargo test --workspace -- --test-threads=1
else
  cargo test --package devstore-server --test "$1" -- --test-threads=1
fi
