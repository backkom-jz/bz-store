# DevStore 本地开发与云端测试环境拆分实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 本地只运行一套 DevStore 开发栈并执行快速检查；MySQL 集成测试和 Playwright E2E 迁移到不使用 Docker 的云端专用测试环境。

**Architecture:** 本地 `scripts/check.sh` 只运行无需业务服务的验证。云端测试主机使用严格校验过的测试配置，原生启动临时 API、Nuxt 和测试网关，连接隔离的 `bz_ztore_test`、专用 Redis、测试 OSS 桶及测试邮件接收器。测试入口与生产服务、配置和数据卷隔离。

**Tech Stack:** Rust 1.97、Cargo、MySQL 8、Redis、S3-compatible OSS、Node.js 24、Nuxt、Vue 3、Playwright、Docker Compose（仅本地 DevStore 开发栈）、Rocky Linux/systemd（云端测试进程）。

**Spec:** [docs/superpowers/specs/2026-10-12-devstore-local-cloud-test-environments-design.md](../specs/2026-10-12-devstore-local-cloud-test-environments-design.md)

## Global Constraints

- 本地只运行 `devstore` 开发 Compose 项目；本地 gateway 绑定 `127.0.0.1:8088`，避开 LNMP Nginx 的 80/443；本地测试不得启动 `devstore-test`。
- 不使用 PostgreSQL 或 ClamAV；生产服务配置不在本次修改范围内。
- 集成测试数据库必须是 `bz_ztore_test`；集成/E2E 的 Redis、对象存储和邮件接收器必须专用测试资源。
- 测试不得读取或输出 DSN、密码、访问密钥、会话秘密、令牌或签名 URL。
- 移除容器时保留命名卷和匿名卷；不运行 `docker compose down -v` 或 `docker system prune`。
- 云端测试进程以原生进程运行，不依赖 Docker 创建测试 API、Web、Nginx、Redis、对象存储或邮件容器。
- 生产发布不得自动触发会清理测试资源的脚本；生产与测试配置必须分别维护。
- 每个任务完成后运行其验证命令并单独提交；提交不得包含开始前已修改的 `docs/operations/rocky10-ip-release.md`。

## Review Focus

- **测试 DSN 指向开发库、ERP 或错误 schema：** `scripts/test-cloud.sh` 的安全测试覆盖缺失 URL、非 `bz_ztore_test` 库名及错误 `APP_ENV`，全部必须在连接任何服务前失败。
- **云端测试环境误复用生产 Redis/OSS：** 配置校验要求显式测试变量和测试标识；验证缺失测试标识或生产桶名时拒绝启动。
- **测试清理命令泄露密码：** Redis CLI 使用 `REDISCLI_AUTH` 环境变量而非命令参数；测试捕获命令参数和日志，确认不含凭据。
- **E2E 子进程收到退出信号后遗留 API/Web/gateway：** runner 测试覆盖启动失败、SIGTERM 和 SIGINT，只终止本次启动的进程。
- **云端邮件收件器不可用或误指生产邮箱：** 测试要求专用邮箱 API 配置；未配置或无法访问时明确失败，不回退到生产 SMTP/站点。

---

### Task 1: 将本地快速检查与 GitHub CI 限定为快速验证

**Files:**
- Modify: `scripts/check.sh`
- Modify: `.github/workflows/ci.yml`
- Modify: `.env.example`
- Modify: `scripts/test-deploy-config.sh`
- Modify: `scripts/dev.sh`
- Modify: `deploy/docker-compose.yml`
- Test: `scripts/test-deploy-config.sh`

**Interfaces:**
- Produces: `bash scripts/check.sh` 运行格式检查、Clippy、Rust library/binary 单元测试、前端单元测试、类型检查和 web/admin 构建；不运行集成测试、E2E、Docker 或外部服务。
- CI 调用同一快速验证步骤，并保留无需业务服务的部署配置静态检查；不创建 MySQL service，也不运行 Playwright。

- [ ] **Step 1: 为本地检查脚本添加行为断言**

在 `scripts/test-deploy-config.sh` 的独立验证中检查 `scripts/check.sh` 不含 `docker compose`、`test-server.sh` 或 `test-e2e.sh` 调用；检查 `.env.example` 不包含 `MYSQL_TEST_DATABASE_URL`；检查本地 gateway 映射为 `127.0.0.1:8088:80`。

- [ ] **Step 2: 运行检查确认旧行为被捕获**

Run: `bash scripts/test-deploy-config.sh`
Expected: FAIL，原因是当前本地检查仍启动测试 Compose，`.env.example` 暴露测试数据库变量。

- [ ] **Step 3: 改为快速本地检查**

修改 `scripts/check.sh`：保留静态部署检查、备份脚本默认值测试、`cargo fmt --check`、Clippy、`cargo test --workspace --lib`、`cargo test --workspace --bins`、`npm run typecheck`、`npm run test:unit` 及 web/admin 构建。移除 `docker compose ... docker-compose.test.yml` 与 `scripts/test-server.sh all`。从 `.env.example` 移除测试 DSN，只留下本地开发应用配置。

- [ ] **Step 4: 收敛 GitHub Actions**

删除 CI MySQL service、`MYSQL_TEST_DATABASE_URL`、`mysql_database_target` 数据库连接步骤及 Playwright 安装/E2E 步骤。保留 Rust/Node 工具链安装，执行 `npm ci`、静态部署配置检查、备份默认值测试和 `bash scripts/check.sh`。

- [ ] **Step 5: 更新静态检查并验证**

调整 `scripts/test-deploy-config.sh`，只检查仍存在的本地/公网 Compose 与 Nginx 模板，并断言仓库运行配置不含 PostgreSQL/ClamAV 服务定义，同时断言本地端口是 `127.0.0.1:8088:80`。修改 `scripts/dev.sh` 使用带 `:8088` 的健康检查 URL，并从 `http://bz-store.local:8088` 提示本地访问地址。运行 `bash scripts/test-deploy-config.sh`；Expected: `deployment config checks passed`。

- [ ] **Step 6: 运行本地快速验证并提交**

Run: `bash scripts/check.sh`
Expected: 所有本地快速验证成功，且脚本没有启动 Docker 或访问数据库、Redis、OSS。

Commit: `git add scripts/check.sh scripts/test-deploy-config.sh scripts/dev.sh deploy/docker-compose.yml .env.example .github/workflows/ci.yml && git commit -m "build: keep local checks service independent"`

### Task 2: 建立云端测试配置校验入口

**Files:**
- Create: `scripts/test-cloud.sh`
- Create: `scripts/cloud-test-config.mjs`
- Test: `scripts/test-cloud-config.test.mjs`
- Modify: `scripts/test-server.sh`
- Modify: `server/tests/support/mod.rs`

**Interfaces:**
- Consumes: `MYSQL_TEST_DATABASE_URL`、`DEVSTORE_TEST_ENV=cloud`、`REDIS_TEST_URL`、`S3_TEST_ENDPOINT`、`S3_TEST_ACCESS_KEY`、`S3_TEST_SECRET_KEY`、`S3_TEST_BUCKET`、`S3_TEST_REGION`、`S3_TEST_VIRTUAL_HOSTED`、`SMTP_TEST_HOST`、`SMTP_TEST_PORT`、`TEST_MAILBOX_API_URL`。
- Produces: `bash scripts/test-cloud.sh [--check-only|integration|e2e|all]`，只在完成所有目标校验后运行测试；失败信息不包含秘密值。

- [ ] **Step 1: 添加配置拒绝用例**

新增 `scripts/test-cloud-config.test.mjs`，针对导出的 `readCloudTestConfig(env)` 覆盖：合法 `bz_ztore_test` 配置可通过；开发库、ERP 库、空库名、尾斜杠、`APP_ENV=production`、缺少云测试标识/Redis/S3/mailbox 配置必须拒绝。断言错误不含 DSN 用户名或密码。

- [ ] **Step 2: 运行测试确认边界行为**

Run: `node --test scripts/test-cloud-config.test.mjs`
Expected: FAIL，因为配置模块尚未实现。

- [ ] **Step 3: 实现无副作用的云端环境预检**

新增 `scripts/cloud-test-config.mjs` 导出 `readCloudTestConfig(env)`，并由 `scripts/test-cloud.sh` 调用。预检要求 `DEVSTORE_TEST_ENV=cloud`、`APP_ENV=test`、`MYSQL_TEST_DATABASE_URL` 的 URL 路径精确为 `/bz_ztore_test`，测试桶名为 `devstore-test`，且完整提供专用 Redis、S3、SMTP 和 mailbox 配置。`--check-only` 只校验配置、不连接服务。完整验证成功后，脚本才映射测试变量到应用运行时环境并连接或清理资源。

测试变量映射到应用运行时所需的 `DATABASE_URL`、`REDIS_URL`、`S3_*`、`SITE_URL` 和 `SMTP_*`。脚本不从仓库 `.env` 读取测试配置，不打印环境变量值，不允许默认 localhost 测试端口或开发/生产变量回退。

- [ ] **Step 4: 移除测试脚本的本机默认服务**

修改 `scripts/test-server.sh`：只接受已校验的测试环境变量；删除 Redis、S3 本机地址和测试凭据默认值。只在 `APP_ENV=test` 且 URL schema 正确时执行测试。

- [ ] **Step 5: 让集成测试使用显式测试 SMTP 配置**

修改 `server/tests/support/mod.rs`，从环境配置读取专用测试 SMTP 主机/端口与发件人；不硬编码 `127.0.0.1:1025`。保持 MFA/加密测试密钥为进程内测试值，不输出。

- [ ] **Step 6: 覆盖错误配置及验证成功路径**

新增单元测试覆盖合法和拒绝配置，并在脚本中调用 `node --test scripts/test-cloud-config.test.mjs`。执行：

`node --test scripts/test-cloud-config.test.mjs`
`bash scripts/test-cloud.sh --check-only`

Expected: 配置解析单测通过；`--check-only` 对缺失配置返回非零并输出安全错误，不连接任何服务。

- [ ] **Step 7: 提交云端测试安全边界**

Commit: `git add scripts/test-cloud.sh scripts/cloud-test-config.mjs scripts/test-cloud-config.test.mjs scripts/test-server.sh server/tests/support/mod.rs && git commit -m "test: guard cloud integration environment"`

### Task 3: 将 Playwright E2E 运行器改为云端原生模式

**Files:**
- Modify: `scripts/e2e-server.mjs`
- Create: `scripts/e2e-config.mjs`
- Create: `scripts/e2e-gateway.mjs`
- Modify: `scripts/test-e2e.sh`
- Modify: `playwright.config.ts`
- Modify: `tests/e2e/support.ts`
- Test: `scripts/e2e-runner.test.mjs`

**Interfaces:**
- Consumes: Task 2 校验后的测试变量；`E2E_BASE_URL`、`TEST_MAILBOX_API_URL` 和测试 Redis 连接。
- Produces: 云端启动器在回环端口启动临时 API、Nuxt、admin 静态目录网关和测试反向代理；Playwright 使用同一 `E2E_BASE_URL`，退出时停止本次运行器启动的进程。

- [ ] **Step 1: 添加无 Docker 与配置读取测试**

新增 Node `node:test` 用例，针对 E2E runner 的配置模块断言：缺失测试模式或邮箱 API 时拒绝启动；传入合法测试配置时解析站点和邮件接收地址；不得生成含密码的子进程参数。

- [ ] **Step 2: 运行测试确认当前 runner 不满足目标**

Run: `node --test scripts/e2e-runner.test.mjs`
Expected: FAIL，当前代码没有可测试的配置校验模块，且 runner 会调用 Docker。

- [ ] **Step 3: 抽出测试配置解析模块**

在 `scripts/e2e-config.mjs` 导出 `readE2EConfig(env)`；要求 `DEVSTORE_TEST_ENV=cloud`、测试数据库 URL 路径为 `/bz_ztore_test`，读取专用 Redis/S3/SMTP/mailbox 配置。端口固定为 API `28080`、Nuxt `23000`、E2E gateway `28088`，避开生产 API 与本地开发端口；Playwright 来源为 `http://127.0.0.1:28088`，不允许覆盖为生产域名。

- [ ] **Step 4: 添加原生 E2E gateway**

新增 `scripts/e2e-gateway.mjs`，仅绑定 `127.0.0.1:28088`；将 `/api/` 转发到 `127.0.0.1:28080` 测试 API、`/admin/` 安全地提供 `apps/admin/dist` 并支持 SPA fallback、其余请求转发到 `127.0.0.1:23000` 测试 Nuxt。拒绝路径穿越；流式代理请求体和响应，保留 Host/Origin 语义，不记录 Cookie、Authorization、查询参数或请求正文。

- [ ] **Step 5: 移除 Compose 生命周期**

改造 `scripts/e2e-server.mjs`：删除 Docker CLI 和 `deploy/docker-compose.test.yml` 调用；在启动前执行云端预检、构建/准备 admin 产物、初始化测试桶和 fixture，再以明确的环境变量启动 API、Nuxt、gateway 子进程。设置启动超时和健康探测。SIGINT、SIGTERM 或任一子进程失败时，只终止已记录的子进程并等待退出；在所有退出路径删除含测试管理员密码/TOTP 秘密的 `.runtime/e2e.json`。

- [ ] **Step 6: 配置 E2E 来源、邮件和限流隔离**

修改 `playwright.config.ts` 将 HTML 报告写入 `.runtime/playwright-report`，使用 `E2E_BASE_URL` 并让云端启动器负责 webServer 生命周期。修改 `tests/e2e/support.ts`：删除对 Docker `exec` 的调用；从 `TEST_MAILBOX_API_URL` 轮询邮件；每个用例开始前通过本机安装的 `redis-cli` 清理专用测试 Redis，密码只通过 `REDISCLI_AUTH` 传递，CLI 参数和日志不得出现密码。拒绝 Redis URL 与测试标识缺失。

- [ ] **Step 7: 添加 runner 生命周期回归测试**

使用假的子进程命令验证：启动失败会清理先前子进程；SIGTERM/SIGINT 会终止并等待所有 runner-owned 子进程；runner 从不执行 `docker`；命令参数和标准输出不含 DSN/密钥。gateway 测试还需覆盖 `/admin/` SPA fallback 和路径穿越请求拒绝。

- [ ] **Step 8: 运行 E2E 静态验证并提交**

Run: `node --test scripts/e2e-runner.test.mjs`
Run: `npm run typecheck`
Run: `rg -n "docker-compose.test|execFileSync\('docker'|docker compose" scripts/e2e-server.mjs scripts/e2e-gateway.mjs tests/e2e/support.ts playwright.config.ts`
Expected: Node 测试与类型检查通过，最后一条命令无输出（退出码 1 表示无 Docker 引用）。

Commit: `git add scripts/e2e-server.mjs scripts/e2e-config.mjs scripts/e2e-gateway.mjs scripts/test-e2e.sh playwright.config.ts tests/e2e/support.ts scripts/e2e-runner.test.mjs && git commit -m "test: run browser suite against cloud test services"`

### Task 4: 移除本机测试 Compose 并更新快速 CI/开发文档

**Files:**
- Delete: `deploy/docker-compose.test.yml`
- Modify: `scripts/check.sh`
- Modify: `scripts/test-deploy-config.sh`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`
- Create: `docs/operations/cloud-test-environment.md`

**Interfaces:**
- Local developer entry: `bash scripts/dev.sh` 启动一套开发栈；`bash scripts/check.sh` 执行快速检查。
- Cloud test operator entry: `sudo systemctl start devstore-cloud-test.service`；服务单元从 `/etc/devstore/cloud-test.env` 加载测试变量并执行 `scripts/test-cloud.sh all`，具体内容写入运维文档。

- [ ] **Step 1: 删除本机测试 Compose 文件及残余引用**

删除 `deploy/docker-compose.test.yml`，清除 `README.md`、`scripts/check.sh`、`scripts/test-deploy-config.sh`、`.github/workflows/ci.yml` 中对该文件和本地测试端口的引用。保留主网关和生产覆盖层仍使用的 Nginx 模板。

- [ ] **Step 2: 补齐快速检查覆盖**

确认 `scripts/check.sh` 显式执行 `cargo test --workspace --lib`、`cargo test --workspace --bins`、前端单元测试、类型检查与 web/admin 构建；确认没有间接调用集成/E2E 脚本。

- [ ] **Step 3: 编写云端测试操作说明**

新增 `docs/operations/cloud-test-environment.md`，记录 Rocky Linux 原生测试进程、专用环境变量清单、文件权限、`devstore-cloud-test.service` one-shot 单元、浏览器依赖、运行命令、`journalctl` 日志位置、重试方式、失败排查和数据清理边界。示例只用占位值；必须强调 `bz_ztore_test`、专用 Redis/OSS/mailbox 和绝不使用生产配置。

- [ ] **Step 4: 更新 README 与验证**

README 本地开发章节只保留单套 Compose 启停、开发站点、快速检查说明；访问地址使用 `http://bz-store.local:8088`，添加云端测试文档链接。运行 `bash scripts/test-deploy-config.sh`、`bash scripts/check.sh`、`rg -n 'docker-compose.test|devstore-test' README.md scripts .github deploy .env.example` 与 `rg -n 'image: (postgres|clamav/)|CLAMAV_|clamav::' deploy server/src .env.example`。Expected: 主仓库运行文件不再引用本地测试 Compose、PostgreSQL 或 ClamAV；历史计划文档不作为运行引用。

- [ ] **Step 5: 提交环境入口和文档**

Commit: `git add deploy/docker-compose.test.yml scripts/check.sh scripts/test-deploy-config.sh .github/workflows/ci.yml README.md docs/operations/cloud-test-environment.md && git commit -m "docs: split local development and cloud testing"`

### Task 5: 在隔离云端测试环境验证真实集成与 E2E

**Files:**
- Modify: `docs/operations/cloud-test-environment.md`
- Create: `docs/verification/cloud-test-environment.md`
- Test: all `server/tests/*.rs`
- Test: `tests/e2e/*.spec.ts`

**Interfaces:**
- Consumes: Tasks 2–4 的云端测试入口与运维配置；云端测试环境已提供专用 MySQL、Redis、OSS、SMTP sink、邮箱 API 以及 Chromium。
- Produces: 测试结果和 Playwright 报告保存在 `.runtime/`，日志脱敏；失败时不自动修改生产环境或发布版本。

- [ ] **Step 1: 在测试主机安装运行依赖并建立专用配置**

安装与仓库锁定版本一致的 Rust/Node、`redis-cli` 和 Chromium 依赖。创建专用服务用户与仅该用户可读的测试环境文件；将 `APP_ENV=test`、数据库 `/bz_ztore_test`、独立 Redis/OSS/mailbox 写入测试配置，不使用生产 `.env`。

- [ ] **Step 2: 运行云端配置拒绝测试**

先运行 `node --test scripts/test-cloud-config.test.mjs`；Expected: 所有无效/生产配置被拒绝。再运行 `bash scripts/test-cloud.sh integration`。

- [ ] **Step 3: 运行真实 MySQL 集成测试**

Run: `bash scripts/test-server.sh all`
Expected: 所有 `server/tests/*.rs` 通过，测试目标在开始前为 `bz_ztore_test`，测试数据清理仅发生于该库及专用 Redis。

- [ ] **Step 4: 运行云端 Playwright E2E**

Run: `bash scripts/test-cloud.sh e2e`
Expected: 桌面和移动项目通过；确认邮箱验证从测试 mailbox 读取；浏览器访问仅为测试站点；运行日志无 Cookie、密码、DSN、令牌和签名 URL。

- [ ] **Step 5: 执行完整回归并提交**

Run: `bash scripts/test-cloud.sh all`
Expected: 配置校验、集成和 E2E 均通过，报告保存在测试主机 `.runtime/`。

完成后在 `docs/verification/cloud-test-environment.md` 记录通过的测试目标、执行日期、运行时版本和报告位置；不得记录测试端点、账号或密钥。

Commit: `git add docs/operations/cloud-test-environment.md docs/verification/cloud-test-environment.md && git commit -m "test: record cloud integration verification"`

### Task 6: 清理本地旧容器并验收单套开发环境

**Files:**
- No source changes; Docker daemon state only.
- Test: local Docker project/container/volume inventory and local HTTP health.

**Interfaces:**
- Consumes: Task 4 的唯一本地 Compose 文件 `deploy/docker-compose.yml`。
- Produces: DevStore 只保留 `devstore` 一个运行项目；`devstore-test` 项目无容器，DevStore 项目无 PostgreSQL/ClamAV 运行容器；其他独立项目不变，所有现存数据卷保留。

- [ ] **Step 1: 记录清理前容器与卷清单**

运行 `docker ps -a`、`docker compose ls` 与 `docker inspect`，保存 `devstore`、`devstore-test` 的容器名、项目标签、服务名、挂载卷和运行状态；不输出容器环境变量。

- [ ] **Step 2: 移除测试项目容器但保留卷**

从清理前容器清单复核所有 `devstore-test` 容器名和卷名，再运行 `docker ps -aq --filter label=com.docker.compose.project=devstore-test | xargs -r docker rm -f`。该命令只删除带有精确项目标签的容器，不删除卷；核对清理前记录的卷仍存在。

- [ ] **Step 3: 按主 Compose 清理孤儿容器**

运行 `docker compose -p devstore -f deploy/docker-compose.yml up -d --remove-orphans`。该操作移除当前 Compose 未定义的 PostgreSQL/ClamAV 孤儿容器，保留 DevStore API/Web/gateway/Redis/RustFS。不要重建或删除数据卷。

- [ ] **Step 4: 验证运行状态与本地应用**

运行 `docker compose -p devstore -f deploy/docker-compose.yml ps`、`docker compose ls`、`curl -fsS -H 'Host: bz-store.local' http://127.0.0.1:8088/api/v1/health/ready` 和 `curl -I -H 'Host: bz-store.local' http://127.0.0.1:8088/`。Expected: API ready、前端 HTTP 200/正常重定向；DevStore 项目只剩 `devstore`，独立 LNMP 项目不变；DevStore 项目无 PG/ClamAV/`devstore-test` 容器，清理前卷名全部仍存在。

- [ ] **Step 5: 独立审查本轮全部变更**

检查最终 diff、危险清理命令、测试配置默认值、日志脱敏和生产隔离；确认 `docs/operations/rocky10-ip-release.md` 的用户既有改动没有进入提交。运行 `git diff --check` 与完整本地快速检查。

---

## 执行顺序与交付门槛

按 Task 1 → Task 2 → Task 3 → Task 4 → Task 5 → Task 6 顺序执行。Task 5 必须在专用云端测试资源准备完毕后运行；云端资源未就绪时可先完成 Task 1–4，但不得把测试改指向生产环境。Task 6 只在代码、文档和云端验证通过后执行。每个任务独立提交，最终对全分支做一次独立审查。
