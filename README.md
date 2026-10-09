# DevStore

Rust / Nuxt 4 / Vue 3 教程商城。开发中的 A 阶段：人工付款与教程授权闭环。

## 开发环境

Rust 1.97、Docker Compose；前端使用 Node 24。端口与现有服务隔离。

启动独立测试依赖：

```sh
docker compose -f deploy/docker-compose.test.yml -p devstore-test up -d
bash scripts/test-server.sh all
```

测试仅连接 devstore_test 数据库，每个测试使用独立 schema；测试 S3 桶和凭据不用于生产。
ClamAV 首次启动需要下载病毒库，完全就绪后才能接收付款凭证。

完整启动步骤及验收结果随功能交付补齐。当前实现进度见 docs/superpowers/plans/2026-10-09-devstore-tutorial-checkout.md。
