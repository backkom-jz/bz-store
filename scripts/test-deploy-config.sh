#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
bash "$root/scripts/test-local-site-url.sh"
public="$root/deploy/nginx/public.conf.template"
local="$root/deploy/nginx/default.conf.template"
compose="$root/deploy/docker-compose.public.yml"
grep -Fq 'server_name store.bengzi.cn;' "$public"
grep -Fq 'return 308 https://store.bengzi.cn$request_uri;' "$public"
grep -Fq 'listen 443 ssl;' "$public"
grep -Fq 'ssl_certificate /etc/nginx/tls/fullchain.pem;' "$public"
grep -Fq 'proxy_set_header X-Forwarded-Proto https;' "$public"
grep -Fq 'TLS_CERT_DIR' "$compose"
grep -Fq 'S3_ENDPOINT' "$compose"
if grep -Fq 'return 308 https://$host$request_uri;' "$local"; then
  echo 'local gateway must stay HTTP-only for bz-store.local' >&2
  exit 1
fi
grep -Fq 'SITE_URL=http://bz-store.local:8088' "$root/.env.example"
if grep -Eq 'docker compose|test-server\.sh|test-e2e\.sh' "$root/scripts/check.sh"; then
  echo 'local checks must not start Docker or integration/E2E tests' >&2
  exit 1
fi
if grep -q 'MYSQL_TEST_DATABASE_URL' "$root/.env.example"; then
  echo '.env.example must not expose cloud test database configuration' >&2
  exit 1
fi
grep -Fq '127.0.0.1:8088:80' "$root/deploy/docker-compose.yml"
for manifest in "$root"/deploy/docker-compose*.yml; do
  case "${manifest##*/}" in
    docker-compose.yml|docker-compose.public.yml) ;;
    *) echo 'unexpected Compose stack: only development and public overlay are permitted' >&2; exit 1 ;;
  esac
done
if grep -Eq 'image: (postgres|clamav/)' "$root/deploy/docker-compose.yml"; then
  echo 'local runtime must not define PostgreSQL or ClamAV services' >&2
  exit 1
fi
grep -Fq 'access_log off;' "$public"
grep -Fq 'access_log off;' "$local"
if grep -Rqi 'clamav' "$root"/deploy/docker-compose*.yml "$root/server/src" "$root/.env.example"; then
  echo 'DevStore runtime must not require ClamAV' >&2
  exit 1
fi
echo 'deployment config checks passed'
