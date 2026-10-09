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

## 本地运行

只监听 `127.0.0.1:8088`。数据库、Redis、对象存储和扫描服务不对外发布。这是本机闭环，不能直接当作公网生产环境。

```sh
cp .env.example .env
# 替换密码、对象存储凭据，以及两个互不相同、至少 32 字节的密钥：
# openssl rand -base64 32
bash scripts/dev.sh
```

`dev.sh` 会构建镜像、创建私有存储桶，并等待健康检查。它不会创建管理员，也没有默认密码。

健康检查：`curl -fsS http://127.0.0.1:8088/api/v1/health/ready`

初始化管理员（密码从标准输入读取，不要写进命令参数）：

```sh
printf '%s\n' '你的密码' | docker compose --env-file .env -f deploy/docker-compose.yml exec -T api admin create --email you@example.com --password-stdin
# 存储桶已由 dev.sh 创建，这条命令可以重复执行
docker compose --env-file .env -f deploy/docker-compose.yml exec -T api admin init-storage
docker compose --env-file .env -f deploy/docker-compose.yml exec -T api admin seed-demo --admin-email you@example.com
```

收款配置是单独的 JSON，字段为 `channel`（bank、wechat 或 alipay）、`payee`、`account`、`instructions`。不要把真实账户写进仓库。

```sh
docker compose --env-file .env -f deploy/docker-compose.yml exec -T api admin payment-config --admin-email you@example.com --file /绝对路径/payment.json
```

未配置收款信息时，商品页会说明不能购买，下单接口返回「尚未配置收款信息，暂时不能购买」。

日志与停止：

```sh
docker compose --env-file .env -f deploy/docker-compose.yml logs -f api web gateway
docker compose --env-file .env -f deploy/docker-compose.yml stop
```

前台是 `http://127.0.0.1:8088/`，后台是 `http://127.0.0.1:8088/admin/`。

检查与浏览器验收：

```sh
bash scripts/check.sh
bash scripts/test-e2e.sh
```

验收记录见 docs/verification/tutorial-checkout.md。完整 V1.0 仍未包括密码重置、管理员额外验证、数字资源下载、服务工单、退款、博客和公网 HTTPS 部署。
