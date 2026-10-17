#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
env_file="${1:-$root/.env}"
expected='http://bz-store.local:8088'

if [[ ! -f "$env_file" ]]; then
  echo '本地 .env 文件不存在；请先复制 .env.example。' >&2
  exit 1
fi

site_url="$(sed -nE 's/^[[:space:]]*SITE_URL[[:space:]]*=[[:space:]]*(.*)[[:space:]]*$/\1/p' "$env_file")"
if [[ "$site_url" != "$expected" ]]; then
  echo "本地 .env 的 SITE_URL 必须设为 ${expected}，以匹配 8088 入口和 Origin 校验；请修改该行后重试。" >&2
  exit 1
fi
