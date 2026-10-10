# DevStore 阶段 0/1 实施计划：MySQL 与账号安全

> **执行要求：**后续按本计划逐项实施，每项遵循先写失败测试、确认测试按预期失败，再实现并验证。全部完成后由独立审查者只读审查。
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to execute this plan task by task.

**目标：**将 DevStore 持久层完整迁移到 MySQL，并完成公网发布准备、邮箱验证/密码重置和管理员 TOTP MFA。

**架构：**保留 Axum 模块化单体、Nuxt 前台、Vue 管理后台和现有购买闭环，将 PostgreSQL 数据访问与迁移整体替换为 SQLx MySQL。邮件通过事务 outbox 交给 SMTP 适配器或仅供本地/测试使用的 Mailpit；管理员 TOTP 由 API 中间件强制校验。

**技术栈：**Rust、Axum、SQLx MySQL、MySQL/InnoDB、Redis、S3 兼容存储、Nuxt 4、Vue 3、Docker Compose、GitHub Actions、SMTP、TOTP。

**设计依据：**

- `docs/superpowers/specs/2026-10-10-devstore-mysql-foundation-design.md`
- `docs/superpowers/specs/2026-10-10-devstore-auth-hardening-design.md`

## 全局约束

- DevStore 运行库为 RDS 专用数据库 `bz_ztore`，不得连接 ERP 库。DSN 和密钥仅从未提交的 `.env` 或部署密钥管理读取。
- 有破坏性的集成测试必须使用独立测试库 `bz_ztore_test`。测试在建连前校验目标，禁止清理、迁移运行库或 ERP 库。没有专用测试库时，数据库集成测试明确跳过/失败，不回退到运行库。
- 本地应用不启动或连接 localhost MySQL。CI 使用临时 MySQL 服务；本地数据库集成测试连接专用 RDS 测试库。Redis、对象存储和扫描服务沿用现有隔离测试配置。
- 此前暴露的密码不得用于实现、测试或文档；用户轮换后的凭据仅通过本地 `.env` 配置。计划与代码不得访问用户 ERP 数据。
- 每项完成后运行定向测试；最终运行 Rust lint/集成测试、npm typecheck/unit/build、`bash scripts/check.sh`、`bash scripts/test-e2e.sh`，并独立审查全量改动。

## 文件与接口边界

- `server/Cargo.toml`、`Cargo.lock`：SQLx MySQL feature 与邮件/MFA 依赖。
- `server/src/config.rs`、`server/src/state.rs`、`server/src/main.rs`、`server/src/bin/admin.rs`：MySQL 连接、启动迁移、环境校验与管理 CLI。
- `server/migrations/`：MySQL 核心表和账号安全迁移。
- `server/src/modules/**`、`server/src/jobs/**`、`server/src/infrastructure/**`：运行时 SQL、事务和后台作业。
- `server/tests/**`：测试库安全栅栏、MySQL 测试辅助方法与真实集成测试。
- `deploy/docker-compose*.yml`、`deploy/nginx/**`、`scripts/**`、`.github/workflows/ci.yml`：CI MySQL、域名/HTTPS、审计、备份/恢复。
- `apps/web/**`、`apps/admin/**`、`packages/**`：邮箱验证、密码找回、管理员 TOTP 登录/设置页面与 API 类型。
- `README.md`、`docs/verification/**`：本地开发、运维、密钥轮换及验收说明。

## Review Focus

- DSN 指向 `bz_ztore` 或 ERP 库：`test_database_url_rejects_runtime_and_erp_database_names` 必须在建连前失败（任务 1）。
- UUID 被错误存为文本或 MySQL byte order 不一致：`mysql_binary_uuid_round_trip` 覆盖写入与读取（任务 2）。
- MySQL 字符排序规则导致邮箱大小写/slug 唯一规则变化：迁移测试使用大小写邮箱和规范 slug 验证（任务 2）。
- 部分唯一索引替换后 NULL/状态变化破坏支付流水约束：覆盖重复 pending/approved 与不同订单同流水（任务 2/3）。
- 死锁或并发审核/超时重复交付：InnoDB 真实并发测试断言唯一权益、审计和最终订单状态（任务 3）。
- 邮件服务故障或恶意 Host 导致 token 泄露/邮件枚举：outbox 故障测试与固定 `SITE_URL` 邮件链接测试（任务 5/6/7）。
- TOTP 重放、MFA 首次注册绕过或旧管理员会话残留：重放计数、初始化确认和重注册撤销测试（任务 8）。
- 既有账户或管理员被邮箱验证/MFA 改造锁定：migration 回填、管理员引导与既有账号登录测试（任务 6/8）。

## 实施任务

### Task 1：建立安全的 MySQL 测试目标

**文件：**新增 `deploy/docker-compose.mysql-ci.yml` 或等效 CI 服务配置；修改 `.github/workflows/ci.yml`、`scripts/test-server.sh`；新增 `server/tests/support/database.rs`。

**接口：**测试辅助函数 `test_database_url() -> Result<Url>` 只接受数据库名 `bz_ztore_test` 或 CI 明确指定的隔离测试库；普通运行库名 `bz_ztore` 和 ERP 库名必须在网络建连前拒绝。

- [ ] **先写失败测试：**`test_database_url_rejects_runtime_and_erp_database_names` 断言非法目标在建连前返回错误；`test_database_url_accepts_dedicated_test_database` 断言测试库名可解析。
- [ ] **验证 RED：**运行 `cargo test -p devstore-server --test mysql_database_target`，确认新测试因安全校验接口不存在或行为不符而失败。
- [ ] **实现安全目标解析：**解析 URL 的 database path 并做精确 allowlist；错误中只显示拒绝原因，不打印 DSN、用户名或密码。
- [ ] **增加 CI MySQL service：**配置健康检查、隔离测试库名和仅 CI 使用的随机凭据；新增 `MYSQL_TEST_DATABASE_URL`，保留现有 `DATABASE_URL` 与 PostgreSQL 测试流程，直到任务 2 的整库切换一次完成；本地运行应用仍不使用 localhost MySQL。
- [ ] **验证 GREEN：**运行 `cargo test -p devstore-server --test mysql_database_target`；CI 通过该目标内的集成探针，在 MySQL service 执行 `SELECT 1` 和数据库名断言。
- [ ] 提交本任务。

### Task 2：完整迁移 SQLx、schema、查询与集成测试到 MySQL

**文件：**`server/Cargo.toml`、`Cargo.lock`、`server/src/config.rs`、`server/src/state.rs`、`server/migrations/**`、`server/src/modules/**`、`server/src/jobs/**`、`server/src/infrastructure/**`、`server/tests/**`、`scripts/test-server.sh`、`scripts/check.sh`。

**接口：**`AppState.pool: MySqlPool`；生产与本地配置只接受 MySQL `DATABASE_URL`；测试辅助方法 `isolated_pool() -> MySqlPool` 只能连接任务 1 校验过的测试库，并串行清理测试库中的业务表。

- [ ] **先写 MySQL 行为测试：**新增/迁移 `mysql_binary_uuid_round_trip`、`mysql_migrations_create_core_constraints` 和 `mysql_integration_suite_preserves_purchase_invariants`；确认空 MySQL 环境下当前实现失败。
- [ ] **验证 RED：**通过 CI MySQL service 执行新迁移测试，记录因缺少 MySQL migration/driver 的预期失败；不启动本机 MySQL。
- [ ] **迁移核心 schema：**切换 SQLx 到 MySQL；将 UUID 存为 `BINARY(16)`、JSONB 改 JSON、时间改 UTC `DATETIME(6)`，表使用 InnoDB；由应用生成 UUID 和过期时间。
- [ ] **迁移数据库专属语法：**替换 `$n`、`RETURNING`、`ON CONFLICT`、部分唯一索引、`FILTER`、`pgcrypto`、`search_path`、`to_jsonb`/`jsonb_build_object`、PL/pgSQL 更新时间触发器、正则运算符、advisory lock 和 `FOR UPDATE OF`。分别使用 `?`、应用生成 ID、重复键处理、可空生成列唯一键、条件聚合、显式 JSON 投影、应用维护 `updated_at`、`REGEXP`、幂等 seed/行锁和 MySQL 行锁。
- [ ] **迁移运行时查询：**将 auth/session、catalog/tutorial、orders/payments/entitlements、admin/audit、upload/cleanup/jobs 以及 CLI seed/payment-config 的 pool/transaction 类型与查询全部改为 MySQL；保留现有事务顺序和所有权校验。
- [ ] **迁移测试设施：**每个 `TestApp` 在串行测试前只清理 `bz_ztore_test` 中的业务表；迁移表保留。测试环境明确目标库，不得连接 `bz_ztore` 或 ERP 库。
- [ ] **验证 GREEN：**运行 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings` 及全部 Rust 集成测试；确认购买、核账、回滚、并发、扫描、退款状态和权限现有验收不退化。
- [ ] 删除 PostgreSQL 运行依赖、测试服务与残留查询；全库搜索确认不存在活动 PostgreSQL 专属代码。
- [ ] 提交本任务。

### Task 3：MySQL 并发与完整购买 E2E 验收

**文件：**`.github/workflows/ci.yml`、`deploy/docker-compose.test.yml`、`scripts/test-server.sh`、`scripts/e2e-server.mjs`、`tests/e2e/**`、`server/tests/orders.rs`、`server/tests/payments.rs`。

- [ ] **先写并发回归测试：**`mysql_parallel_approval_grants_one_entitlement`、`mysql_expiry_and_approval_are_mutually_exclusive`，确认重复操作最多创建一条权益和一条审计记录。
- [ ] **验证 RED：**在 CI MySQL service 运行两项测试，确保竞争逻辑有被执行并能暴露不满足的断言。
- [ ] **实现锁与死锁处理：**以 InnoDB 行锁和唯一键保证一致性；仅对可安全重试的死锁/锁等待错误做有限重试，禁止重试外部邮件或存储副作用。
- [ ] **迁移 E2E：**CI 和测试脚本改用 MySQL 测试服务，继续运行桌面/移动端注册、下单、驳回、重提、审批、付费阅读和跨账号隔离。
- [ ] **验证 GREEN：**运行完整 Rust MySQL 集成测试和 `bash scripts/test-e2e.sh`；记录总测试数与失败数。
- [ ] 提交本任务。

### Task 4：公网配置、依赖审计与备份恢复

**文件：**`deploy/docker-compose.yml`、`deploy/nginx/**`、`.env.example`、`.gitignore`、`.github/workflows/ci.yml`、新增 `scripts/backup/**`、`README.md`、`docs/operations/**`。

- [ ] **先写配置/脚本测试：**`production_config_requires_https_and_tls_storage`、`gateway_redirects_http_and_routes_public_host`、`backup_restore_round_trip_preserves_rows`；脚本测试只连接 CI 临时 MySQL，不接触用户 RDS。
- [ ] **验证 RED：**运行对应配置测试和备份脚本测试，确认当前没有公网 Host/TLS 路由及 MySQL 备份恢复实现。
- [ ] **实现域名部署配置：**本地使用 `http://bz-store.local` 并记录 hosts 设置；公网 `SITE_URL` 使用 `https://store.bengzi.cn`，网关支持 HTTP 跳转、证书外部挂载、可信代理与 Secure Cookie。没有 DNS/服务器访问时只验证本地配置，不签发真实证书。
- [ ] **实现密钥与数据库校验：**环境模板仅放占位项；生产校验密钥长度/差异、MySQL DSN、站点来源和 TLS；不打印 DSN 或 secret。
- [ ] **实现备份恢复：**MySQL 加密转储、权限受限临时凭据、保留/异地复制、隔离恢复演练；对象存储备份独立记录。日志和进程参数不得泄露密码。
- [ ] **增加依赖安全检查：**CI 运行 Rust/npm 锁定依赖审计并修复有补丁的高危项；无补丁项逐项记录包名、依赖路径、影响、缓解和复查日，不全局 suppress。
- [ ] **验证 GREEN：**在 CI 运行配置检查、临时 MySQL 备份恢复演练、秘密扫描与依赖审计；DNS、真实证书、RDS 备份和异地恢复列为需用户环境验收项。
- [ ] 提交本任务。

### Task 5：事务邮件 outbox、SMTP 和本地邮件接收器

**文件：**`server/Cargo.toml`、`server/src/config.rs`、新增 `server/src/infrastructure/email.rs`、`server/src/jobs/email_outbox.rs`、MySQL migration、`deploy/docker-compose.test.yml`、测试。

**接口：**异步邮件接口 `async fn EmailSender::send(&self, message: EmailMessage) -> Result<(), EmailError>`；业务事务写入 outbox，短期令牌载荷使用独立 AEAD 密钥加密，后台 worker 负责 SMTP 投递和有限退避重试。

- [ ] **先写失败测试：**`outbox_keeps_message_after_smtp_failure`、`outbox_marks_message_sent_after_delivery`、`production_email_config_never_logs_credentials`。
- [ ] **验证 RED：**运行邮件模块测试，确认接口/表/worker 缺失导致预期失败。
- [ ] **实现 outbox 与 worker：**MySQL 表保存收件人、模板/业务类型、AEAD 加密的短期投递载荷、重试次数和状态；使用 `EMAIL_OUTBOX_ENCRYPTION_KEY`，事务提交后异步投递，成功后清除密文，不记录完整令牌 URL；失败载荷保留供有限重试。
- [ ] **实现配置：**生产 SMTP 参数来自 secret 环境变量且缺失时注册/重置邮件功能关闭并明确报错；开发测试使用 Mailpit，SMTP 与管理 UI 仅绑定 loopback，不加入生产 Compose。
- [ ] **验证 GREEN：**模拟 SMTP 超时/拒绝/恢复、worker 并发领取和重试；确认本地接收器不暴露到公网。
- [ ] 提交本任务。

### Task 6：邮箱验证注册与兼容既有账号

**文件：**MySQL auth migration、`server/src/modules/auth/**`、OpenAPI、`apps/web/**`、`packages/shared-types/**`、认证测试和 Playwright。

- [ ] **先写失败测试：**`unverified_user_cannot_login_or_create_order`、`email_verification_token_is_single_use_and_expires`、`registration_does_not_disclose_existing_email`。
- [ ] **验证 RED：**在 MySQL 测试库运行认证集成测试，确认当前未验证账号可登录/重复邮箱响应暴露账号的断言失败。
- [ ] **迁移既有用户：**增加 `email_verified_at` 和用途/过期/消费字段齐全的 token 表；历史用户在 migration 中标记已验证，避免锁出已有管理员。
- [ ] **实现注册与验证：**注册只创建未验证用户、摘要 token 和同事务 outbox 邮件，不签发会话；验证/重发有 IP/邮箱限流、CSRF 和原子消费逻辑。
- [ ] **实现前台：**注册后展示通用提示，添加验证成功/过期/重发页面；链接基于 `SITE_URL` 生成。
- [ ] **验证 GREEN：**用 Mailpit 完成新账号验证，验证已验证/未验证用户权限、枚举保护、重复消费、过期、CSRF 和浏览器流程。
- [ ] 提交本任务。

### Task 7：密码找回、重置和会话作废

**文件：**MySQL auth migration、`server/src/modules/auth/**`、OpenAPI、`apps/web/app/pages/**`、认证测试与 E2E。

- [ ] **先写失败测试：**`password_forgot_has_same_response_for_known_and_unknown_email`、`password_reset_token_is_single_use`、`password_reset_revokes_all_sessions`。
- [ ] **验证 RED：**在 MySQL 测试库运行认证测试，确认现状缺少端点、令牌与撤销行为。
- [ ] **实现重置 API：**一次性令牌只存摘要并设置短过期；forgot 对任何邮箱返回相同状态/响应体；IP 与邮箱摘要限流；token 和 outbox 在同一事务创建。
- [ ] **实现重置事务：**校验/消费 token、更新 Argon2id hash、撤销该用户全部会话；并发消费只成功一次。邮件链接仅使用配置的规范 `SITE_URL`。
- [ ] **实现前台：**找回表单、重置表单、过期/无效链接与完成状态；不显示邮箱是否注册。
- [ ] **验证 GREEN：**验证正确密码可登录、旧密码和所有旧 Cookie 失效、重复/过期/未知令牌失败，运行浏览器找回流程。
- [ ] 提交本任务。

### Task 8：管理员 TOTP、恢复码和 API 强制授权

**文件：**MySQL MFA migration、`server/src/config.rs`、`server/src/middleware/auth.rs`、auth/admin handler/service、`server/src/bin/admin.rs`、`apps/admin/**`、测试。

- [ ] **先写失败测试：**`admin_api_rejects_session_without_mfa_marker`、`admin_login_requires_valid_totp`、`totp_code_cannot_be_replayed`、`recovery_code_is_consumed_once`。
- [ ] **验证 RED：**运行认证/Admin 测试，确认当前普通管理员会话仍能访问后台且 MFA 断言失败。
- [ ] **实现 seed 与恢复码存储：**独立 `MFA_ENCRYPTION_KEY` 使用 AEAD 加密 seed；恢复码只存 digest；添加 TOTP 最近使用计数，事务内拒绝同一时间步重复码。
- [ ] **实现安全引导：**CLI 只展示一次 TOTP seed 和恢复码，要求 TOTP 确认后启用；未注册 MFA 的管理员不能拿到普通管理员会话；重新注册撤销旧管理员会话和恢复码并审计。
- [ ] **实现登录挑战与服务端授权：**管理员密码登录再校验 TOTP/单次恢复码；session 存 MFA 验证时间/方式；`AdminUser` 对每条 admin API 强制验证新鲜度，普通用户路由不受影响。
- [ ] **实现管理界面：**Vue Admin 展示 TOTP 挑战、初始化和恢复码一次性保存提示；所有安全判断留在 API。
- [ ] **验证 GREEN：**测试未 enrolled、错误码、过期码、重放、恢复码并发、密钥缺失、普通用户、直接 API、CSRF 和管理员会话撤销。
- [ ] 提交本任务。

### Task 9：最终全量验证、运维交接与独立审查

**文件：**全仓库；更新 `README.md`、`docs/verification/**` 和运维文档。

- [ ] **验证 MySQL 全量后端：**CI MySQL 上运行 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings` 和全部集成测试；所有连接目标均为测试库。
- [ ] **验证前后端：**运行 `npm run typecheck`、`npm run test:unit`、前台/管理端生产构建和 `bash scripts/test-e2e.sh`；记录桌面/移动端通过数。
- [ ] **验证安全边界：**检查数据库目标保护、邮件枚举/重放、过期令牌、MFA 绕过、跨用户访问、死锁重试、备份恢复和秘密扫描。
- [ ] **更新验收文档：**分别标注本地/CI 已通过项，以及必须由用户提供 RDS 凭据、RDS 版本/权限、DNS、TLS 和 SMTP 账号后才能完成的外部验收；不得宣称未执行的公网部署已通过。
- [ ] **独立审查：**请求独立只读审查者检查完整改动；Critical/Important 发现先修复，再运行相关测试并复审。
- [ ] 提交最终验收记录。

## 计划自查

- 两份设计的所有范围均映射至任务：MySQL schema/查询/事务、测试隔离、CI、域名/TLS、依赖审计、备份恢复、SMTP/outbox、邮箱验证、密码重置、管理员 TOTP、运维交接与独立审查。
- MySQL 切换需要统一 pool/transaction 类型，故整库 SQL 转换作为同一可编译交付任务；测试服务先独立建立，避免中间提交破坏现有可运行分支。
- 本地开发不连接 localhost MySQL。没有 `bz_ztore_test` 或等效隔离测试目标时，不能运行破坏性集成测试。
- 轮换后的 RDS 测试库凭据、RDS 引擎版本/权限、公网 DNS/服务器、SMTP 账号是外部运行依赖；均不预设已存在。
