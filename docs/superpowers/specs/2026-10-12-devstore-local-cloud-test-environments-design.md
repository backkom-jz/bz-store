# DevStore 本地开发与云端测试环境拆分设计

日期：2026-10-12

状态：待用户审阅

## 目标

本地开发环境只运行一套 DevStore Compose 项目。PostgreSQL 和 ClamAV 不再使用。单元测试、类型检查和构建在本地运行；需要真实数据库、Redis、对象存储或浏览器端到端流程的集成测试与 E2E 测试，在专用云端测试环境运行。

生产部署不在本次范围内。云端测试环境必须与生产环境隔离，且沿用服务器不使用 Docker 的约束；测试服务使用独立配置和专用资源。

## 已确认的决定与约束

- 本地只保留 `devstore` 开发栈；`devstore-test` 不在本地运行。
- 不使用 PostgreSQL，也不使用 ClamAV。
- 本地 `scripts/check.sh` 只执行快速代码检查：格式/静态检查、Rust 单元测试、前端单元测试、类型检查和构建。
- 数据库集成测试和 E2E 测试在云端专用测试环境执行。
- 云端测试环境使用专用 MySQL 数据库 `bz_ztore_test`、独立 Redis、独立测试对象存储桶和测试邮件接收配置。测试不得访问生产资源。
- 现有本地数据卷在清理孤儿容器时保留；禁止使用 `docker compose down -v`。
- 测试保留现有破坏性清理能力，但启动前必须验证测试数据库名和测试资源配置，避免误连开发或生产资源。

## 当前问题

- 当前 Docker daemon 中仍运行 `devstore` 和 `devstore-test` 两个 Compose 项目。两者都包含不再使用的 PostgreSQL 和 ClamAV 孤儿容器；测试项目还运行重复的 Redis 和 RustFS。
- 仓库当前主 Compose 已定义应用、Redis 和 RustFS，不包含 PostgreSQL 或 ClamAV；`docker-compose.test.yml` 仍服务于本机集成/E2E 测试。
- `scripts/check.sh` 会启动测试 Compose 的 Redis 和 RustFS。`scripts/e2e-server.mjs` 会启动测试 Redis、RustFS、Mailpit 和 Nginx gateway，并在本机启动 API 与 Nuxt。
- 集成测试会清空 `bz_ztore_test` 中的业务表，并执行 Redis `FLUSHDB`；E2E 也会清空 Redis。生产和开发 Redis、数据库、对象存储不能作为测试目标。
- GitHub Actions 当前创建临时 MySQL，并运行完整检查与 Playwright E2E。迭代后云端专用测试环境应成为集成/E2E 的正式执行位置；CI 不应因本次迁移而获得生产密钥或连接生产资源。

## 方案

保留一套本地开发 Compose 项目，提供本地应用服务及其 Redis、RustFS 依赖。Compose 服务和项目名保持简单、稳定；启动、停止、日志命令统一指向 `deploy/docker-compose.yml`。本地开发使用开发库和开发对象存储配置，不启动测试服务。

把本地检查与云端测试拆为不同入口：

- `scripts/check.sh` 作为本地快速验证入口，执行 Rust 格式检查、Clippy、库/二进制单元测试、前端单元测试、类型检查和前端构建；不启动 Docker Compose，不访问测试数据库或云端服务。
- 新增云端测试入口，专供测试服务器执行 MySQL 集成测试及 E2E。它从受限权限的测试环境配置读取连接信息，要求 `APP_ENV=test`、数据库路径严格为 `/bz_ztore_test`，并验证 Redis 与对象存储是测试专用目标。
- 删除只为本地测试容器服务的 `deploy/docker-compose.test.yml`；保留仍由主网关和生产覆盖层使用的 Nginx 模板。测试环境由云端原生服务与专用配置提供，不再有容易误启动的第二套本地 Compose 项目。
- E2E 测试在云端测试主机以原生进程运行，不依赖 Docker 创建 Mailpit、Nginx 或测试 API/Web 容器。运行器通过固定的本机回环端口连接隔离测试服务和临时测试应用；停止时只终止本次启动的子进程。
- 更新 Playwright 配置和测试支持代码，使 E2E 的站点来源、测试邮件接收端和依赖端点由测试环境配置提供。移除测试生命周期内调用 Docker `exec` 清理 Redis 的逻辑，改由云端测试启动器在确认目标为专用测试 Redis 后执行清理。
- 更新 GitHub Actions：保留不依赖业务服务的快速代码检查；数据库集成和 E2E 改由云端测试环境的明确入口运行，不将生产凭据注入 CI。具体触发方式（手动、发布候选或部署后）在实施计划中定义，不由生产发布自动触发破坏性测试。

云端测试服务可以使用专用 RDS 测试库、测试 Redis 实例、测试 OSS 桶及仅供测试的邮件接收器。测试 API 与前端以临时进程运行，避免覆盖正在运行的生产进程。测试云主机与生产共用时，端口、配置文件、进程用户和日志目录必须隔离；优先使用独立测试主机。

## 容器清理与数据保护

- 停止并移除 `devstore-test` Compose 项目的容器，因为测试不再在本机运行。
- 对 `devstore` 项目按当前 Compose 定义更新并移除孤儿容器，使 PostgreSQL 和 ClamAV 不再运行。
- 删除仓库中的本地测试 Compose 定义，避免后来误将测试基础服务重新拉起。
- 不删除任何命名卷或匿名卷；数据卷是否可清理不属于本次自动操作。
- 清理命令必须显式指定项目与 Compose 文件，不能使用未限定项目名的全局清理命令，也不能执行 `docker system prune`。
- 清理前后记录容器名、Compose 项目、服务、镜像、状态及卷名；检查 `devstore` 应用健康状态和本地站点访问。

## 配置与接口

- 本地开发配置只包含开发应用所需项；测试连接信息只存于云端测试主机，不能写入仓库或 `.env.example`。
- 云端测试入口必须拒绝缺少 `MYSQL_TEST_DATABASE_URL` 的执行，并验证目标数据库名为 `bz_ztore_test`。禁止接受 `bz_ztore`、ERP 数据库或生产主机作为测试配置。
- Redis 使用专用测试实例，并在开始清理前验证测试环境标识；OSS 只允许写入专用测试桶。
- 所有敏感连接值通过权限受限的环境文件注入；日志只输出目标类型和校验状态，不输出 DSN、密码、密钥或签名 URL。
- E2E 站点与邮件接收端使用显式测试配置，不能回退到生产域名或 SMTP 服务。

## 验收标准

1. 本地 `docker compose ls` 只显示 DevStore 的一套运行项目；`devstore-test` 不再有运行容器。
2. 本地 DevStore Compose 不启动 PostgreSQL 或 ClamAV；当前 API、前端、Redis、RustFS 健康，站点可访问。
3. 清理前后的 PostgreSQL、ClamAV、Redis、RustFS 数据卷仍存在；未执行任何卷删除命令。
4. 本地 `scripts/check.sh` 不调用 Docker、不连接数据库/Redis/OSS，成功执行格式检查、Clippy、单元测试、类型检查和构建。
5. 云端测试入口对缺失、错误库名或生产配置失败关闭；针对 `bz_ztore_test` 与专用测试服务成功运行集成测试。
6. 云端 E2E 不调用 Docker CLI，不访问本地测试容器端口；站点及邮件配置只能来自测试环境。
7. 云端集成与 E2E 可重复执行，清理操作只影响专用测试数据库、Redis 和对象存储桶。
8. GitHub Actions 不需要生产秘密；快速检查通过，集成/E2E 的结果能通过云端测试执行流程明确获得。
9. 仓库中不再包含可在本地启动的 `devstore-test` Compose 项目或旧的 PostgreSQL/ClamAV 服务定义。

## 不在本次范围内

- 修改生产服务器上的 systemd、Nginx、数据库或对象存储配置。
- 删除本机遗留数据卷或迁移、擦除其中的数据。
- 使用 PostgreSQL、ClamAV 或为其保留兼容配置。
- 在本地访问云端测试数据库、Redis 或对象存储。
- 引入 Kubernetes、微服务拆分或额外的 CI 平台。
- 为真实用户配置邮件服务商账号。
