# DevStore

Rust / Nuxt 4 / Vue 3 教程商城。开发中的 A 阶段：人工付款与教程授权闭环。

## 开发环境

Rust 1.97、Docker Compose；前端使用 Node 24。应用使用 MySQL，开发和集成测试连接专用 RDS 数据库，不启动本机 MySQL。

设置 `.env` 中的 `MYSQL_TEST_DATABASE_URL`，必须指向隔离的 `bz_ztore_test`，不得使用运行库 `bz_ztore` 或 ERP 数据库。CI 会提供临时 MySQL 测试服务；本地集成测试不会回退到应用数据库。

当前 MySQL 版只支持在新建的空 `bz_ztore` 数据库中初始化。此前 PostgreSQL 版本的迁移文件已改为 MySQL 方言，不能直接对旧 PostgreSQL 数据库原地运行；如需保留旧数据，必须先制定并验证单独的数据导出、映射、导入及行数/业务校验方案。

启动 Redis、对象存储和文件扫描服务：

```sh
docker compose -f deploy/docker-compose.test.yml up -d redis storage clamav
bash scripts/test-server.sh all
```

测试仅连接 `MYSQL_TEST_DATABASE_URL` 指定的隔离库；每个测试启动时清空 DevStore 业务表，保留迁移记录。测试对象存储桶和凭据不用于生产。
ClamAV 首次启动需要下载病毒库，完全就绪后才能接收付款凭证。

## 本地运行

只监听 `127.0.0.1:80`，通过 `/etc/hosts` 映射 `127.0.0.1 bz-store.local` 后访问 `http://bz-store.local`。数据库、Redis、对象存储和扫描服务不对外发布。这是本机闭环，不能直接当作公网生产环境。

```sh
cp .env.example .env
# 替换密码、对象存储凭据，以及两个互不相同、至少 32 字节的密钥：
# openssl rand -base64 32
bash scripts/dev.sh
```

开发运行时的 `DATABASE_URL` 应指向 DevStore 的 `bz_ztore` 数据库；绝不能指向 ERP 数据库。连接密码里的 `@` 等保留字符需按 URL 规则编码。

`dev.sh` 会构建镜像、创建私有存储桶，并等待健康检查。它不会创建管理员，也没有默认密码。
默认 Docker 网段将网关地址固定为 `172.31.255.2`，API 只信任此反向代理转发的客户端 IP；若该网段与本机网络冲突，请同时调整 Compose 网段、网关地址与 `TRUSTED_PROXIES`。

ClamAV 首次加载病毒库会占用数 GB 内存。小型服务器可能因内存不足而无法健康运行；部署前请先用本机资源验证完整服务栈。

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

前台是 `http://bz-store.local/`，后台是 `http://bz-store.local/admin/`。

检查与浏览器验收：

```sh
bash scripts/check.sh
bash scripts/test-e2e.sh
```

验收记录见 docs/verification/tutorial-checkout.md。生产域名为 `https://store.bengzi.cn`；公网 Compose 覆盖层通过 `deploy/docker-compose.public.yml` 启用，并从 `TLS_CERT_DIR` 挂载证书。DNS、TLS 和公网主机配置仍需在部署环境验收。

邮箱、管理员 MFA 与备份初始化见 [运维说明](docs/operations/auth-email-mfa.md) 和 [MySQL 备份恢复说明](docs/operations/mysql-backups.md)。
