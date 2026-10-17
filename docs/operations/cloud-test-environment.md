# DevStore 云端专用测试环境

本地只使用 `scripts/dev.sh` 启动一套开发栈，以及 `scripts/check.sh` 做单元测试、类型检查和构建。集成/E2E 在专用云端 checkout 中运行，采用原生 Rust、Node 和 Chromium 进程；无需 Docker，也无需为测试站点配置域名或开放公网端口。

生产源码与配置保持独立：示例测试目录为 `/opt/devstore-test`，用户为 `devstore-test`，配置为 `/etc/devstore/cloud-test.env`。不要复用生产 `.env`、生产服务用户或生产发布目录。当前真实云端验证状态见 [验收记录](../verification/cloud-test-environment.md)。

## 1. 准备资源与权限

| 资源 | 必须满足 |
| --- | --- |
| MySQL 8 | 库名严格为 `bz_ztore_test`；专用用户仅有此库的迁移、读写和清表权限，无 `bz_ztore`、ERP 或全局权限 |
| Redis | 专用测试实例及显式 DB 索引；用例会执行 `FLUSHDB`，不能使用开发/生产实例；支持密码及 ACL 用户名 |
| OSS/S3 | 私有桶 `devstore-test`；HTTPS endpoint；专用凭据只可操作此桶，不能读写生产桶 |
| SMTP sink | 专门捕获测试邮件，不转发真实邮件；远端必须 TLS；同一测试主机的 localhost sink 可用明文 |
| 邮箱 API | Mailpit 兼容读取 API；远端 HTTPS，localhost 可 HTTP；仅测试主机可访问 |

MySQL、Redis、OSS 必须是显式列入 allowlist 的远端服务，不接受 localhost、127/8、IPv6 loopback/mapped loopback 或未指定地址。allowlist 是配置核对，资源权限仍需在数据库/云平台限制。对象存储与 Redis 不会回退到本地默认地址。

在测试服务器创建用户和目录：

```sh
sudo useradd --system --create-home --home-dir /var/lib/devstore-test --shell /bin/bash devstore-test
sudo install -d -o devstore-test -g devstore-test -m 0750 /opt/devstore-test
sudo install -d -o root -g devstore-test -m 0750 /etc/devstore
sudo -u devstore-test -H git clone https://github.com/backkom-jz/bz-store.git /opt/devstore-test
```

将代码切到本轮已审查的提交；不要在生产目录切分支。`.runtime` 保存私有报告、截图和日志，需限制为测试用户可读，不由 Nginx 托管。

## 2. 安装工具链和浏览器

安装 Rust 1.97（含 rustfmt/Clippy）、Node 24 和 `redis-cli`。Node 位于 `/opt/node/bin` 时必须把该目录加入 PATH，避免 npm 的 `/usr/bin/env node` 找不到解释器。Rust 工具链归测试用户所有，默认位置 `/var/lib/devstore-test/.cargo/bin`。工具链安装参考 [Rocky Linux 发布说明](rocky10-ip-release.md)。

```sh
export PATH=/opt/node/bin:/var/lib/devstore-test/.cargo/bin:/usr/local/bin:/usr/bin:/bin
node --version
npm --version
rustc --version
redis-cli --version
```

Rocky 的仓库包名可能不同，可用 `dnf provides '*/redis-cli'` 确认提供者，再安装 CLI；不需要在测试主机启动 Redis。确认此 CLI 支持 `-u`、`--user` 和 `REDISCLI_AUTH`。

以测试用户安装锁定的 JS 依赖和浏览器：

```sh
sudo -u devstore-test -H env PATH=/opt/node/bin:/var/lib/devstore-test/.cargo/bin:/usr/local/bin:/usr/bin:/bin bash
cd /opt/devstore-test
npm ci
npx --no-install playwright install chromium
node --input-type=module -e 'import { chromium } from "@playwright/test"; const b = await chromium.launch(); await b.close(); console.log("Chromium launch passed");'
exit
```

Playwright 官方 Linux 支持列表为 Debian/Ubuntu，未列出 Rocky；Rocky 10 的浏览器依赖需在本机验证，不能直接将基于 apt 的 `install-deps` 当成 dnf 命令。若启动提示缺少共享库，用 `dnf provides '*/缺少的库名.so'` 查对应包，安装后重跑上面的 smoke test。只有原生 Chromium 启动通过才执行 E2E；无法解决时使用官方支持发行版的专用测试主机。依据：[官方系统要求](https://playwright.dev/docs/intro#system-requirements)、[浏览器安装说明](https://playwright.dev/docs/browsers)。

2 GB 主机使用 `CARGO_BUILD_JOBS=1`、Playwright 单 worker，构建与测试串行执行。仍需观察内存和磁盘；生产繁忙时安排测试窗口或使用独立测试主机。

## 3. 测试邮箱

已提供远端 sink/API 时填入下一节的专用地址。还没有 sink 时，可按 [Mailpit 原生安装说明](https://mailpit.axllent.org/docs/install/) 安装并固定经过验证的版本，测试主机运行以下服务即可，不开放公网端口：

```ini
[Unit]
Description=DevStore test mailbox
After=network.target

[Service]
User=devstore-test
Group=devstore-test
ExecStart=/usr/local/bin/mailpit --smtp 127.0.0.1:1025 --listen 127.0.0.1:8025 --database /var/lib/devstore-test/mailpit.db
Restart=on-failure
UMask=0077
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

保存为 `/etc/systemd/system/devstore-test-mailbox.service` 后运行 `systemctl daemon-reload` 和 `systemctl enable --now devstore-test-mailbox.service`。配置 `SMTP_TEST_HOST=127.0.0.1`、`SMTP_TEST_TLS=false`、`TEST_MAILBOX_API_URL=http://127.0.0.1:8025`，allowlist 加 `127.0.0.1`。远端 SMTP 则必须配置受信证书和 TLS，参考 [Mailpit SMTP TLS](https://mailpit.axllent.org/docs/configuration/smtp/)；测试入口不支持生产邮件账号回退。

邮箱 API URL 可为服务根地址或以 `/api` 结尾的前缀。读取接口为 `/api/v1/messages`（返回 `messages[].ID` 和 `To[].Address`）及 `/api/v1/message/{ID}`（返回 `Text`）。测试只读取匹配随机测试邮箱的验证邮件。当前读取接口不传认证头，因此应只绑定 loopback 或在专用网络按来源 IP 限制访问，不可公开。

## 4. 专用配置文件

用编辑器创建 `/etc/devstore/cloud-test.env`，权限 `root:devstore-test 0640`；不要复制生产配置。以下全部为占位值，URI 密码必须按 URL 规则编码，尤其 `@` 写为 `%40`。SMTP 选用本机 sink 示例：

```dotenv
APP_ENV=test
DEVSTORE_TEST_ENV=cloud
DEVSTORE_TEST_ALLOWED_HOSTS=mysql.test.internal,redis.test.internal,oss-test.example.invalid,127.0.0.1
MYSQL_TEST_DATABASE_URL=mysql://devstore_test:REPLACE_URL_ENCODED_PASSWORD@mysql.test.internal:3306/bz_ztore_test
REDIS_TEST_URL=redis://test_user:REPLACE_URL_ENCODED_PASSWORD@redis.test.internal:6379/0
S3_TEST_ENDPOINT=https://oss-test.example.invalid
S3_TEST_ACCESS_KEY=REPLACE_TEST_ACCESS_KEY
S3_TEST_SECRET_KEY=REPLACE_TEST_SECRET_KEY
S3_TEST_BUCKET=devstore-test
S3_TEST_REGION=oss-cn-shanghai
S3_TEST_VIRTUAL_HOSTED=true
SMTP_TEST_HOST=127.0.0.1
SMTP_TEST_PORT=1025
SMTP_TEST_TLS=false
SMTP_TEST_FROM='DevStore Test <test@example.test>'
TEST_MAILBOX_API_URL=http://127.0.0.1:8025
```

`DEVSTORE_TEST_ALLOWED_HOSTS` 精确匹配每个服务的主机名，不接受通配符。Redis URI 必须显式选择 `/0` 等数字 DB，不可带 query/fragment。Aliyun OSS 使用区域 endpoint、匹配 region 和 virtual-hosted 模式；兼容 S3 服务按其要求设置 `S3_TEST_VIRTUAL_HOSTED=false`。桶必须保持私有。

```sh
sudo chown root:devstore-test /etc/devstore/cloud-test.env
sudo chmod 0640 /etc/devstore/cloud-test.env
```

不要用 `cat`、`set -x`、`env`、`systemctl show --property=Environment` 或连接 URI 命令参数打印配置。配置解析错误只输出变量名，不输出秘密值。测试会自行生成仅本次使用的管理员/MFA 信息，`.runtime/e2e.json` 权限为 0600，正常退出和启动失败时删除；报告和 trace 仍需私有保存。

## 5. 原生 systemd 测试任务

仓库提供 [service 模板](../../deploy/systemd/devstore-cloud-test.service)。测试用户需可写 checkout 的构建目录、`.runtime`、Cargo/npm 缓存；生产目录不能由该用户写入。安装模板：

```sh
sudo install -m 0644 /opt/devstore-test/deploy/systemd/devstore-cloud-test.service /etc/systemd/system/devstore-cloud-test.service
sudo systemctl daemon-reload
```

先通过 systemd 读取同一配置，只运行无连接预检：

```sh
sudo systemd-run --unit=devstore-cloud-preflight --collect --wait --pipe \
  --property=User=devstore-test --property=Group=devstore-test \
  --property=WorkingDirectory=/opt/devstore-test \
  --property=EnvironmentFile=/etc/devstore/cloud-test.env \
  --property=UMask=0077 \
  --setenv=HOME=/var/lib/devstore-test \
  --setenv=PATH=/opt/node/bin:/var/lib/devstore-test/.cargo/bin:/usr/local/bin:/usr/bin:/bin \
  /usr/bin/bash scripts/test-cloud.sh --check-only
```

预检只解析配置并运行配置单测，不连接/清理数据库、Redis 或 OSS。通过后再启动完整测试：

```sh
sudo systemctl start --no-block devstore-cloud-test.service
sudo journalctl -u devstore-cloud-test.service -f
sudo systemctl status devstore-cloud-test.service
```

成功的 one-shot 会回到 inactive/dead，最近一次退出码应为 0；失败为 failed。不要将该任务 enable 为开机服务，也不要挂到生产发布步骤。`flock` 防止同一主机任务重叠；多个测试主机也不能并发清理同一套测试资源。

手动分阶段诊断时，可在相同 `systemd-run` 命令下把最后参数换成 `integration`、`e2e` 或 `all`；先停止完整任务，避免并发。仓库入口：

| 命令 | 内容 |
| --- | --- |
| `bash scripts/test-cloud.sh --check-only` | 配置单测及无连接预检 |
| `bash scripts/test-cloud.sh integration` | 真实 MySQL/Redis/OSS 集成测试 |
| `bash scripts/test-cloud.sh e2e` | 构建、原生临时应用及 Playwright 桌面/移动用例 |
| `bash scripts/test-cloud.sh all` | 顺序执行集成和 E2E |

E2E 临时端口仅绑定 loopback：API `28080`、Nuxt `23000`、gateway `28088`。来源固定 `http://127.0.0.1:28088`，不可改成生产域名。前端构建与 gateway 不接收数据库/Redis/OSS 密钥；API 和 admin CLI 接收经过校验的运行时配置。Rust 测试进程不读取 `.env`，Nuxt 使用空 env 文件，Vite 在测试模式禁用 dotenv。

## 6. 结果与故障排查

报告在 `/opt/devstore-test/.runtime/playwright-report`，trace/截图在 `.runtime/playwright-results`，应用日志为 `.runtime/e2e-api.log`、`e2e-web.log`、`e2e-gateway.log`，任务日志由 `journalctl -u devstore-cloud-test.service` 查看。报告可能包含测试会话和验证链接，不能公开上传。确认脱敏后才分享具体失败信息。

| 现象 | 处理 |
| --- | --- |
| 配置拒绝 | 检查变量名、精确库名、allowlist、TLS 和测试桶，不改为生产配置 |
| MySQL 拒绝/超时 | 检查 RDS 测试用户、schema 权限、测试主机出口 IP 白名单及网络 |
| Redis WRONGPASS/NOPERM | 核对 ACL 用户、URI 编码、所选 DB 和测试专用 FLUSHDB 权限 |
| OSS 403/404/签名失败 | 检查桶、region、virtual-hosted、测试凭据权限和服务器时间 |
| 邮件收不到 | 检查 sink 服务、TLS 模式、API JSON 格式和 `.runtime/e2e-api.log` 的 outbox 错误码 |
| 端口占用 | `ss -lntp` 检查 28080/23000/28088；只停止此前测试任务，不能用宽泛 pkill/killall |
| Chromium 缺库 | 重做第 2 节 smoke test，安装对应发行版库；不跳过浏览器用例宣称验收通过 |
| 超时/OOM | 查看任务和内核日志；调度空闲窗口/扩容测试机，保留串行构建与单 worker |

停止及重试：`sudo systemctl stop devstore-cloud-test.service`；修复后 `sudo systemctl reset-failed devstore-cloud-test.service`，再 `start`。systemd 的 control-group 会回收任务子进程，runner 同时等待本次 API/Web/gateway 退出并删除临时凭据。

集成测试会清空测试库业务表；E2E 用例会 FLUSHDB 专用 Redis、创建测试商品/订单/对象。不会自动清空 OSS 全桶或删除云资源，按测试保留周期单独清理测试桶中的 fixture。不能使用生产身份执行清理；生产发布不调用这些脚本。

## 本地旧容器收敛

按已确认计划，只有云端真实集成和 E2E 通过后才执行旧容器清理。先保存精确 Compose project/service 标签和挂载卷清单，不输出容器环境变量。只移除项目标签严格为 `devstore-test` 的容器，保留全部卷；再使用主 Compose 收敛 `devstore` 的孤儿容器。

禁止 `down -v`、`docker system prune`、全局删容器或删卷。清理后仅保留 `devstore` 一套本地开发项目，其他项目保持原状态；复核原卷仍存在，并检查 `http://bz-store.local:8088/` 和 `/api/v1/health/ready`。当前执行门槛和待办记录在 [验收记录](../verification/cloud-test-environment.md)。
