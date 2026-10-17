# DevStore

Rust / Nuxt 4 / Vue 3 教程商城。支持教程阅读、数字文件交付、基础服务工单、人工收款与退款登记、博客。当前发布验收状态见 [测试与交付回归记录](docs/verification/cloud-test-environment.md)。

## 开发环境

Rust 1.97、Node 24、Docker Compose。本地仅运行一套 `devstore` 开发栈：gateway、API、Web、Redis 和 RustFS；MySQL 使用 RDS 上独立的开发库 `bz_ztore`。不运行本机数据库或病毒扫描容器。

本地和 GitHub CI 只运行单元测试、类型检查和构建。集成测试及 Playwright E2E 在无 Docker 的云端专用测试环境执行，配置、资源、报告及操作步骤见 [云端测试环境](docs/operations/cloud-test-environment.md)。不要将云测试配置放入本地 `.env`。
付款凭证只接受经过完整解码的 PNG/JPEG 图片，限制 5 MiB、8000 边长和 2000 万像素，并保存在私有对象存储中。当前版本不运行恶意软件扫描；管理员应在核对付款时人工检查凭证，不要打开凭证中的外部链接或执行其中内容。

## 本地运行

gateway 只监听 `127.0.0.1:8088`，在 `/etc/hosts` 添加 `127.0.0.1 bz-store.local` 后访问 `http://bz-store.local:8088`。本地端口与其他 Nginx 的 80/443 分开；Redis 和对象存储不对外发布。

```sh
cp .env.example .env
# 替换密码、对象存储凭据，以及两个互不相同、至少 32 字节的密钥：
# openssl rand -base64 32
bash scripts/dev.sh
```

开发运行时的 `DATABASE_URL` 应指向 DevStore 的 `bz_ztore` 数据库；绝不能指向 ERP 数据库。连接密码里的 `@` 等保留字符需按 URL 规则编码。

`dev.sh` 会构建镜像、创建私有存储桶，并等待健康检查。它不会创建管理员，也没有默认密码。
默认 Docker 网段将网关地址固定为 `172.31.255.2`，API 只信任此反向代理转发的客户端 IP；若该网段与本机网络冲突，请同时调整 Compose 网段、网关地址与 `TRUSTED_PROXIES`。

小内存服务器应把 MySQL 和 S3 对象存储放在外部服务，并使用主机 Nginx 反代，不启动容器 gateway。构建前后端镜像应使用 CI 或构建机，避免在 2 GB 主机上编译。

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

前台是 `http://bz-store.local:8088/`，后台是 `http://bz-store.local:8088/admin/`。已有 `.env` 也需将 `SITE_URL` 更新为 `http://bz-store.local:8088`，启动脚本会检查来源是否一致。

本地快速检查（不会启动 Docker、连接业务服务或运行集成/E2E）：

```sh
bash scripts/check.sh
```

`check.sh` 运行脚本/网关单元测试、Rust fmt/Clippy/library/binary 单测、前端和 E2E 支持代码类型检查、前端单测及 web/admin 构建。集成与浏览器验收仅在云端运行；CI 不创建数据库或测试 Compose。

生产域名为 `https://store.bengzi.cn`；无 Docker 的 Rocky Linux 发布见 [IP 上线说明](docs/operations/rocky10-ip-release.md)。已有公网 Compose 覆盖层保留供独立部署使用。本地卷和旧测试容器清理的边界见 [云端测试与本地收敛](docs/operations/cloud-test-environment.md#本地旧容器收敛)。

邮箱、管理员 MFA 与备份初始化见 [运维说明](docs/operations/auth-email-mfa.md) 和 [MySQL 备份恢复说明](docs/operations/mysql-backups.md)。
上传校验与人工审核边界见 [上传文件处理说明](docs/operations/uploads.md)。
