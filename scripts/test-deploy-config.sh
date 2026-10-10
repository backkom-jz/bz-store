#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
public="$root/deploy/nginx/public.conf.template"
local="$root/deploy/nginx/default.conf.template"
compose="$root/deploy/docker-compose.public.yml"
grep -Fq 'server_name store.bengzi.cn;' "$public"
grep -Fq 'return 308 https://$host$request_uri;' "$public"
grep -Fq 'listen 443 ssl;' "$public"
grep -Fq 'ssl_certificate /etc/nginx/tls/fullchain.pem;' "$public"
grep -Fq 'proxy_set_header X-Forwarded-Proto https;' "$public"
grep -Fq 'TLS_CERT_DIR' "$compose"
grep -Fq 'S3_ENDPOINT' "$compose"
if grep -Fq 'return 308 https://$host$request_uri;' "$local"; then
  echo 'local gateway must stay HTTP-only for bz-store.local' >&2
  exit 1
fi
grep -Fq 'SITE_URL=http://bz-store.local' "$root/.env.example"
grep -Fq '127.0.0.1:80:80' "$root/deploy/docker-compose.yml"
grep -Fq 'access_log off;' "$public"
grep -Fq 'access_log off;' "$local"
echo 'deployment config checks passed'
