#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ! -f .env ]]; then
  echo "缺少 .env。请复制 .env.example，并用 openssl rand -base64 32 生成两个不同的密钥。"
  exit 1
fi
if grep -q 'REPLACE' .env; then
  echo ".env 仍包含 REPLACE 占位符。请填入本机密码和密钥后再启动。"
  exit 1
fi
docker compose --env-file .env -f deploy/docker-compose.yml up -d --build
echo "等待 http://127.0.0.1:8088/api/v1/health/ready 。首次启动 ClamAV 可能需要几分钟。"
echo "初始化私有存储桶。"
storage_ready=0
for _ in $(seq 1 30); do
  if docker compose --env-file .env -f deploy/docker-compose.yml exec -T api admin init-storage; then
    storage_ready=1
    break
  fi
  sleep 5
done
if [[ "$storage_ready" -ne 1 ]]; then
  echo "私有存储桶初始化失败。请查看 API 日志后再重试。"
  exit 1
fi
for _ in $(seq 1 90); do
  if curl -fsS http://127.0.0.1:8088/api/v1/health/ready >/dev/null; then
    echo "DevStore 已在 http://127.0.0.1:8088 就绪。这是本地入口，不是公网生产部署。"
    echo "初始化管理员：printf '%s\n' '你的密码' | docker compose --env-file .env -f deploy/docker-compose.yml exec -T api admin create --email you@example.com --password-stdin"
    echo "示例教程：docker compose --env-file .env -f deploy/docker-compose.yml exec -T api admin seed-demo --admin-email you@example.com"
    echo "收款配置：docker compose --env-file .env -f deploy/docker-compose.yml exec -T api admin payment-config --admin-email you@example.com --file /path/payment.json"
    echo "日志：docker compose --env-file .env -f deploy/docker-compose.yml logs -f"
    echo "停止：docker compose --env-file .env -f deploy/docker-compose.yml stop"
    exit 0
  fi
  sleep 5
done
echo "健康检查尚未通过。请查看日志：docker compose --env-file .env -f deploy/docker-compose.yml logs --tail 80"
exit 1
