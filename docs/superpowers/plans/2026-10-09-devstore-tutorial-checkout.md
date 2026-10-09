# DevStore 脚手架与教程交易闭环 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, or superpowers:subagent-driven-development if the user selects delegation. Steps use checkbox syntax for tracking.

**Goal:** 在本地交付可运行、可验证的 Nuxt 前台、Vue 后台和 Rust API，走通教程注册购买、人工收款审核及授权阅读。

**Architecture:** Axum 模块化单体使用 PostgreSQL 保存交易事实，Redis 共享限流，S3 兼容存储隔离付款凭证，ClamAV 检查上传。Nginx 将前台、后台和 API 合并到同源入口；浏览器使用安全 Cookie 会话，审批和授权使用同一个数据库事务。

**Tech Stack:** Rust、Axum 0.8、SQLx 0.9、Tokio、PostgreSQL、Redis、Nuxt 4、Vue 3、Vite、TypeScript、Tailwind CSS、Playwright、Docker Compose。

**Spec:** [DevStore 设计稿](../specs/2026-10-09-devstore-design.md)，采用其中 A 范围；完整 V1.0 的剩余门槛不计入本轮完成声明。

## Global Constraints

- 人民币金额统一使用整数分，订单保存商品快照。
- 单件商品下单；购物车、自动支付、会员订阅不进入 V1.0。
- 选择 A 时只允许发布和购买 tutorial 类型商品。
- 付款凭证首版仅接收 JPEG、PNG，限制为 5 MiB。
- 付费内容鉴权、文件权限和管理员权限均在后端执行。
- 不提供可用于正式交易的默认收款账户或默认管理员密码。
- payment_review 不允许用户取消，也不被自动超时任务取消。
- 驳回回到 pending_payment，保留审核历史，并将重提窗口延长至至少审核后 24 小时。
- 用户数据、收银台、付费阅读和后台响应使用 private/no-store。
- 主背景 #0B1020，卡片 #172033，主色 #6366F1，强调色 #22D3EE，文字 #F8FAFC，次级文字 #94A3B8，边框 #293449，圆角 12–16px。
- 正式环境不允许跳过安全扫描；依赖不可用不得绕过认证、限流或事务校验。
- 尚未完成的模块不展示可操作入口；没有真实执行的测试不得标记通过。

## Review Focus

1. 两位审核员同时核销同一流水给不同订单：一个成功，一个 409，失败订单不得获得权益（Task 6）。
2. Access Token 到期、多个标签页同时刷新：成功刷新保持会话可用，旧 Refresh Token 不能再被使用（Task 2、Task 7）。
3. 未登录或另一用户读取曾被购买者打开的章节：正文不可从 SSR、客户端缓存或 HTTP 响应泄露（Task 6、Task 7、Task 9）。
4. 有效图片携带超大尺寸、错误 MIME 或恶意内容，或扫描服务掉线：不得进入审核列表；失败上传可清理（Task 5）。
5. 订单接近过期时提交、取消、驳回和审核发生竞争：只有合法状态转换，审核中不超时关闭（Task 4、Task 5、Task 6）。

## 执行边界和环境选择

- 用户已选择 A。当前文档是实施计划，复选框表示待执行工作。
- 建议由当前会话按 Task 1–9 顺序实现。它们共享认证、DTO 和数据库事务接口，顺序执行便于持续联调；完成后进行独立代码审查。
- 本机 Docker 已确认可用；80、443、6379 已有现有容器使用。DevStore 只绑定 `127.0.0.1:8088`，测试入口使用 18088；不停止或复用现有业务容器。
- 本机 Node 为 20.19.6。前端构建使用项目容器中的 Node 24（最低 24.11），不修改系统 Node；实现时将工具链、镜像和依赖解析结果记录在锁文件或固定镜像引用中。
- 使用 npm workspaces 管理 `apps/*` 和 `packages/*`；Rust 根 Cargo workspace 包含 server。构建支持 `npm ci` 和 `cargo --locked`。
- 下文 npm 命令在项目的 Node 24 构建/测试容器内运行。测试脚本和基础 fixture 属于测试搭建步骤，先创建可运行的测试入口，再观察缺少业务行为导致的失败；不把“命令不存在”计作有效的红灯测试。
- 默认 Compose 项目名 devstore；测试使用 devstore-test 及独立数据卷。不得对原有容器或卷执行清理操作。
- 新建仓库没有首个提交。执行开始时先保留设计与计划的基线提交，再创建开发分支；是否需要额外 worktree 由 using-git-worktrees 的环境检查决定。
- 除管理员邮箱、密码和真实收款资料外，常规参数按本文默认值实现，不再逐项询问。真实经营资料可由用户通过本地配置和运维命令提供。

## 文件职责地图

| 路径 | 职责 |
| --- | --- |
| Cargo.toml、server/Cargo.toml、server/src/{main,lib,config,state}.rs | Rust workspace、应用配置与路由入口 |
| server/src/common/{response,error,extractors}.rs | 响应封装、错误映射、统一参数提取 |
| server/src/middleware/{request_id,security,auth,csrf}.rs | 请求追踪、安全响应头、会话与 CSRF |
| server/src/infrastructure/{database,cache,storage,scanner}.rs | 数据库、Redis、私有对象存储、扫描边界 |
| server/src/modules/{auth,catalog,orders,payments,entitlements,tutorials,admin}/ | 业务 handler、service、repository、dto 和 model |
| server/src/jobs/{expire_orders,cleanup_uploads}.rs | 锁定后关闭待付款订单、回收孤立文件 |
| server/src/bin/admin.rs | 管理员初始化、示例教程和收款配置命令 |
| server/migrations/、server/tests/ | 数据约束与真实 PostgreSQL 集成测试 |
| apps/web/ | Nuxt 前台、SSR 和用户购买流程 |
| apps/admin/ | Vue 管理端、商品编辑和付款审核 |
| packages/{shared-types,api-client,ui}/ | 与 Rust DTO 一致的 TS 类型、请求客户端、共享样式与基础组件 |
| deploy/、scripts/、.github/workflows/ | Compose、Nginx、验证入口、CI |
| tests/e2e/、docs/verification/ | 浏览器验收和实际验证记录 |

## 接口约定

- `AppState` 位于 server/src/state.rs，持有 `PgPool`、`Arc<Config>`、`redis::aio::ConnectionManager`、`Arc<dyn object_store::ObjectStore>` 和 `Scanner`。服务通过 `&AppState` 访问依赖。
- `AppError` 位于 common/error.rs，统一实现 IntoResponse；`ApiResult<T> = Result<T, AppError>`。对外响应由中间件附加当前 request_id。
- `CurrentUser { id: Uuid, session_id: Uuid, is_admin: bool }` 为后端提取器，来源只能是已验证 Cookie 和数据库会话。
- 业务函数返回 DTO；handler 负责状态码与 `ApiEnvelope<T> { code, message, data, request_id }`。数据库错误通过 SQLSTATE 映射，冲突不暴露 SQL。
- 所有 JSON 使用 snake_case。TS 金额为 number，但服务端金额范围限定为 `0..=100_000_000` 分，保证前后端精确表达。
- 公共列表参数 `q`（最多 100 字）、`category`（slug）、`page`（从 1 开始）、`page_size`（默认 20，最大 100），返回 `Page<T> { items, page, page_size, total }`，排序 `created_at DESC, id DESC`。
- 测试统一前缀 `it_` 表示依赖真实服务的测试；不以 ignored 测试隐藏未执行用例。集成测试启动时缺少依赖即失败，并说明缺失项。

---

### Task 1: 可启动的基础工程、数据库迁移与测试环境

**Files:** 创建根 Cargo.toml、package.json、.gitignore、.gitattributes、.env.example、server/Cargo.toml、server/build.rs、server/src/{main,lib,config,state}.rs、server/src/common/{mod,response,error,extractors}.rs、server/src/middleware/{mod,request_id,security}.rs、server/src/infrastructure/{mod,database,cache,storage,scanner}.rs、server/migrations/0001_core.sql、server/tests/{support/mod,foundation}.rs、deploy/{docker-compose,docker-compose.test}.yml、server/Dockerfile、scripts/test-server.sh、README.md。

**Interfaces:** `Config::from_env() -> ApiResult<Config>`；`connect(config: &Config) -> ApiResult<AppState>`；`build_router(state: AppState) -> axum::Router`；`migrate(pool: &PgPool) -> ApiResult<()>`；测试支撑 `TestApp::start() -> TestApp` 与 `TestApp::get(path: &str) -> TestResponse`，TestResponse 提供 status、body 和 headers。

- [ ] 编写 `it_health_and_errors`、`it_database_constraints` 测试：存活 200、正常就绪 200、数据库不可用就绪 503、未知 API 404、非法 JSON 400；错误包含 request_id。数据库直接插入非 CNY、负金额、quantity=2、重复活动凭证和缺失外键时必须失败。

```rust
let response = app.get("/api/v1/health/live").await;
assert_eq!(response.status, 200);
assert_eq!(response.body["code"], "OK");
assert!(response.body["request_id"].as_str().is_some_and(|v| !v.is_empty()));
```

- [ ] 运行 `bash scripts/test-server.sh foundation`，确认测试因为尚未实现行为失败；命令启动独立测试依赖并在 Rust 测试容器内执行指定测试文件。
- [ ] 实现基础 Rust 服务、结构化日志、参数提取错误统一响应、请求 ID、安全响应头、敏感字段脱敏及存活/就绪端点。就绪同时检查数据库、Redis、对象存储和扫描服务。
- [ ] 实现设计稿中的 16 张首阶段表、外键、金额和状态约束、幂等/凭证/权益唯一索引、updated_at trigger、审核金额与审核人字段约束；迁移由 SQLx 协调锁执行。build.rs 监听 migrations，SQL 文件固定 LF。
- [ ] 配置 PostgreSQL、Redis、S3 兼容服务、ClamAV 独立网络与健康检查；模板不含真实密钥；产品运行不包含测试管理员、自动种子或内存仓库。补齐配置校验测试，生产缺失密钥、非 HTTPS SITE_URL 或跳过扫描时启动失败。
- [ ] 重跑 foundation 全部通过，再执行 `cargo fmt --all -- --check`。提交本任务文件：`git commit -m "feat: add application foundation and transactional schema"`。

### Task 2: Cookie 会话、认证、管理员初始化与共享限流

**Files:** 创建 server/src/modules/{mod.rs,auth/{mod,dto,model,repository,service,handler}.rs}、server/src/middleware/{auth,csrf}.rs、server/src/bin/admin.rs、server/tests/auth.rs；修改 server/src/infrastructure/cache.rs、server/src/lib.rs。

**Interfaces:** `register(state: &AppState, input: RegisterInput) -> ApiResult<SessionResult>`；`login(state: &AppState, input: LoginInput) -> ApiResult<SessionResult>`；`refresh(state: &AppState, raw_token: &str, csrf: &str) -> ApiResult<SessionResult>`；`revoke(state: &AppState, session_id: Uuid) -> ApiResult<()>`。`SessionResult` 携带服务器内部令牌与 PublicUser，handler 只通过 Set-Cookie 发送令牌。

- [ ] 编写 `it_auth_lifecycle`、`it_csrf_and_rbac`、`it_refresh_race`、`it_shared_rate_limit`：邮箱大小写归一化、重复邮箱 409、密码不明文存储、错误登录统一 401、来源错误和 CSRF 缺失 403、退出后旧 Access Token 401、普通用户后台 403。

```rust
assert_eq!(register_again.status, 409);
assert!(!login_response.body.to_string().contains("refresh_token"));
assert_eq!(old_session_after_logout.status, 401);
assert_eq!(concurrent_refresh_successes, 1);
```

- [ ] 运行 `bash scripts/test-server.sh auth`，确认失败；测试刷新需覆盖 Access Token 已过期但 Refresh Token 仍有效的情形。
- [ ] 实现 12–128 字符密码规则、Argon2id、固定算法/issuer/audience 的 JWT；Access 有效 15 分钟、Refresh 有效 7 天，随机令牌至少 256 bit。哈希与验证放在受并发限制的 blocking 任务中。
- [ ] 实现 HttpOnly、SameSite=Lax、生产 Secure 的 host-only Cookie；Access Path=/，Refresh Path=/api/v1/auth；CSRF 使用服务端专用密钥对 session_id 作 HMAC，随会话保存其哈希。GET /auth/csrf 用有效 Access 或 Refresh 识别会话，返回同一会话稳定的 CSRF Token，避免页面重载及多标签页失配；该接口不开放跨源读取。刷新通过 Refresh Cookie 验证会话及 CSRF，不能要求未过期 Access。重复使用旧 Refresh 返回 401，不撤销刚刷新的合法会话。
- [ ] 实现精确 Origin 校验，认证请求不允许跨站；登录和注册也校验来源。Redis 原子限流：登录每 IP 10 次/分钟和每规范化邮箱 10 次/15 分钟，注册每 IP 5 次/小时，刷新每会话 30 次/分钟；429 携带 Retry-After。仅受信任代理地址可使用其客户端 IP 头，Redis 不可用返回 503。
- [ ] 添加 `admin create --email <email>` 命令，从隐藏交互输入或 stdin 读取密码，不接受 argv 密码、不打印令牌。已有邮箱不静默覆盖密码；运维命令与后台写操作均留审计。
- [ ] 重跑 auth 全部通过，验证 Cookie、并发刷新和多实例限流后提交：`git commit -m "feat: add secure cookie authentication and admin bootstrap"`。

### Task 3: 商品与教程编辑、公开目录和审核可用的种子内容

**Files:** 创建 server/src/modules/catalog/{mod,dto,model,repository,service,handler}.rs、server/src/modules/tutorials/{mod,dto,model,repository,service,handler}.rs、server/src/modules/admin/{mod,audit}.rs、server/tests/catalog.rs；修改 server/src/bin/admin.rs、server/src/lib.rs。

**Interfaces:** `list_products(state: &AppState, query: CatalogQuery) -> ApiResult<Page<ProductSummary>>`；`product_by_slug(state: &AppState, slug: &str) -> ApiResult<ProductDetail>`；`save_product(state: &AppState, actor: &CurrentUser, id: Option<Uuid>, input: ProductInput) -> ApiResult<ProductDetail>`；`tutorial_by_slug(state: &AppState, slug: &str) -> ApiResult<TutorialDetail>`。TutorialDetail 仅含公开介绍与 `ChapterSummary { id, title, sort_order, is_preview }`，不含正文。

- [ ] 编写 `it_catalog_visibility`、`it_admin_editor`、`it_catalog_validation`：未发布商品不公开，目录无 content_md，slug 冲突 409，越权写入 403，搜索 `%`/`_` 作为普通字符处理，页大小不可超过 100，非 tutorial 不能发布。

```rust
assert_eq!(anonymous_draft.status, 404);
assert!(public_tutorial.body["data"].get("content_md").is_none());
assert_eq!(duplicate_slug.status, 409);
assert_eq!(publish_service.status, 422);
```

- [ ] 运行 `bash scripts/test-server.sh catalog` 确认失败。
- [ ] 实现 categories、products、tutorials 公共 GET 及设计稿中的后台创建/修改接口；补充 GET /admin/tutorials 和 GET /admin/tutorials/{id} 供编辑器读取草稿及章节。参数化 SQL，slug 格式 `[a-z0-9]+(?:-[a-z0-9]+)*`、长度不超过 220；付费教程商品必须绑定已发布且至少有一个非预览章节的教程才允许发布。
- [ ] 所有管理写操作与审计在同一事务内；商品 type 不允许在存在历史订单后改变；归档商品不删除教程内容。章节预览标记默认为 false，只有显式操作才公开。
- [ ] 添加显式 `admin seed-demo` 命令，建立一个带预览与付费章节的示例 Rust 教程，slug=rust-axum-starter、price_cents=9900；页面标识示例内容，命令不配置收款账号或默认用户。
- [ ] 重跑 catalog 全部通过后提交：`git commit -m "feat: add tutorial catalog and audited content editing"`。

### Task 4: 幂等下单、历史快照、订单归属与超时取消

**Files:** 创建 server/src/modules/orders/{mod,dto,model,repository,service,handler}.rs、server/src/modules/entitlements/{mod,service}.rs、server/src/jobs/{mod,expire_orders}.rs、server/tests/orders.rs；修改 server/src/bin/admin.rs、server/src/lib.rs。

**Interfaces:** `create_order(state: &AppState, actor: &CurrentUser, key: &str, input: CreateOrderInput) -> ApiResult<OrderDetail>`，输入仅 product_id、quantity；`get_order(state: &AppState, actor: &CurrentUser, id: Uuid) -> ApiResult<OrderDetail>`；`cancel_order(state: &AppState, actor: &CurrentUser, id: Uuid) -> ApiResult<OrderDetail>`；`expire_batch(state: &AppState, now: OffsetDateTime) -> ApiResult<u64>`。OrderDetail 包括订单快照、状态、expires_at 和不含对象 Key 的审核历史。

- [ ] 编写 `it_idempotent_checkout`、`it_snapshot_and_ownership`、`it_cancel_expire_race`：16 个相同键并发请求只建一单，同键不同商品 409，不同用户同键不冲突，未知 amount 字段 422，未配置收款付费下单 503，零元下单立即完成且只授权一次。

```rust
assert_eq!(created_order_count, 1);
assert_eq!(distinct_returned_order_ids.len(), 1);
assert_eq!(after_price_change.item.unit_price_cents, 9900);
assert_eq!(other_user_order.status, 404);
assert_eq!(reviewing_order_after_expiry.status, "payment_review");
```

- [ ] 运行 `bash scripts/test-server.sh orders` 确认失败。
- [ ] 实现用户范围的幂等键（16–128 个 ASCII 可见字符）、载荷指纹、商品行锁、服务端金额及快照、订单明细和请求记录同事务提交。原幂等结果优先返回，不因商品后来下架而生成第二单；数量固定 1。
- [ ] 订单默认 24 小时有效；用户取消、过期任务都先锁订单。每批最多 100 条，任务使用 SKIP LOCKED，仅关闭 pending_payment。所有读取校验归属并返回 no-store。
- [ ] 实现付款配置读取和 `admin payment-config --file <path>`，结构含 channel、payee、account、instructions；文件内容不写日志。GET payment-instructions 返回订单金额与编号及配置，不拼接客户端金额。免费领取不要求收款配置。
- [ ] 零元授权使用 Task 6 将复用的 `grant_tutorial(tx: &mut PgConnection, user_id: Uuid, item_id: Uuid, product_id: Uuid) -> ApiResult<()>`，定义于 modules/entitlements/service.rs；依靠 order_item_id 唯一约束幂等插入。
- [ ] 重跑 orders 全部通过后提交：`git commit -m "feat: add idempotent single-product tutorial checkout"`。

### Task 5: 私有凭证上传、扫描、预览与孤立对象清理

**Files:** 创建 server/src/modules/payments/{mod,dto,model,repository,upload,handler}.rs、server/src/jobs/cleanup_uploads.rs、server/tests/uploads.rs、server/tests/fixtures/{receipt.png,receipt.jpg}；修改 server/src/infrastructure/{storage,scanner}.rs、server/src/lib.rs。

**Interfaces:** `validate_receipt(bytes: &[u8]) -> ApiResult<ValidatedImage>`，返回受验证的 content_type、width、height；`Scanner::scan(&self, bytes: &[u8]) -> ApiResult<()>`；`submit_proof(state: &AppState, actor: &CurrentUser, order_id: Uuid, bytes: bytes::Bytes) -> ApiResult<PaymentSummary>`；`proof_bytes(state: &AppState, actor: &CurrentUser, proof_id: Uuid) -> ApiResult<PrivateFile>`；`cleanup_batch(state: &AppState, cutoff: OffsetDateTime) -> ApiResult<u64>`。

- [ ] 编写 `it_private_uploads`、`it_scanner_failure`、`it_upload_state_race`、`it_upload_cleanup`：JPEG/PNG 成功、伪造扩展名失败、超过 5 MiB 为 413、宽或高超过 8000 或像素总数超过 20,000,000 为 422；扫描失败/超时不能产生 pending 凭证。

```rust
assert_eq!(oversized.status, 413);
assert_eq!(scanner_unavailable.status, 503);
assert_eq!(pending_proofs_after_scan_failure, 0);
assert_eq!(another_users_proof.status, 404);
assert_eq!(concurrent_pending_proofs, 1);
```

- [ ] 运行 `bash scripts/test-server.sh uploads` 确认失败。PNG/JPEG 由测试代码生成或使用明确可分发的最小测试图片，不加入真实付款截图。
- [ ] 上传端点使用单一 multipart 字段 file，完整请求上限 6 MiB、文件上限 5 MiB；先校验用户、订单和上传速率（每用户 10 次/小时），再有界读取、解码和隔离存储。记录 file_objects.status=quarantined，服务端生成不可预测的对象 Key，拒绝客户端指定路径。
- [ ] 调用 ClamAV INSTREAM，超时 30 秒，任何非明确 clean 响应均失败；在后台确认可用前不创建 payment_submission。审阅隔离图片只经鉴权 API，禁止对象公开访问。
- [ ] 扫描通过后另开短事务，重新锁订单并检查归属、有效期和 pending_payment，更新对象 clean 状态、插入凭证并进入 payment_review；避免在数据库锁内等待扫描/上传。若状态已变，返回 409，保留可回收对象而不创建凭证。
- [ ] 实现 GET /admin/payments/{id}/proof 和 GET /orders/{id}/payment-proof/{proofId}；后者仅订单所有者，前者仅管理员。返回正确图片 MIME、nosniff、no-store，不返回对象存储 URL。
- [ ] 每小时清理超过 24 小时的孤立对象；先锁记录并置 deleting，附加凭证必须校验 clean 且未在删除；对象删除失败可重试，已有关联凭证的对象不清理。
- [ ] 重跑 uploads，增加真实 ClamAV 对 EICAR 测试字节的拒绝和真实私有桶匿名读取失败检查后提交：`git commit -m "feat: add scanned private payment receipts"`。

### Task 6: 人工审核、审计、权益与付费章节后端授权

**Files:** 创建 server/src/modules/payments/service.rs、server/src/modules/entitlements/{dto,handler}.rs、server/src/openapi.rs、server/tests/{payments,entitlements}.rs；修改 payments/{dto,handler,repository}.rs、entitlements/{mod,service}.rs、tutorials/{handler,service,repository}.rs、admin/{mod,audit}.rs、server/src/lib.rs。

**Interfaces:** `approve(state: &AppState, actor: &CurrentUser, proof_id: Uuid, input: ApproveInput) -> ApiResult<OrderDetail>`；`reject(state: &AppState, actor: &CurrentUser, proof_id: Uuid, reason: &str) -> ApiResult<OrderDetail>`；`chapter(state: &AppState, actor: Option<&CurrentUser>, tutorial_id: Uuid, chapter_id: Uuid) -> ApiResult<ChapterContent>`；`library(state: &AppState, actor: &CurrentUser) -> ApiResult<Vec<LibraryEntry>>`。`ChapterContent { id, title, content_md }` 只能由已鉴权章节接口返回；引用 Task 4 的 grant_tutorial。

- [ ] 编写 `it_approval_idempotency`、`it_duplicate_bank_reference`、`it_review_rollback`、`it_reject_resubmit`、`it_expiry_review_race`：16 次并发批准只有一次审核审计和一个权益；金额不符 422；复核确认=false 为 422；已拒绝凭证不能通过；重提窗口不少于 24 小时。

```rust
assert_eq!(entitlement_count, 1);
assert_eq!(approval_audit_count, 1);
assert_eq!(second_order_same_bank_reference.status, 409);
assert_eq!(order_after_injected_audit_failure.status, "payment_review");
assert_eq!(entitlements_after_injected_audit_failure, 0);
```

- [ ] 编写 `it_paid_content_access`：未购买 403，未登录付费章节 401，公开预览 200，购买后 200，其他用户仍 403；商品下架后合法购买者仍可读取，下架后的公开购买详情不再提供购买入口；测试中撤销权益后立即 403。
- [ ] 运行 `bash scripts/test-server.sh payments` 和 `bash scripts/test-server.sh entitlements`，确认失败。
- [ ] 实现审批服务：始终先锁订单后锁凭证，重读状态、实际金额、收款渠道和流水；重复 approved 返回当前订单且不修改原审批数据。渠道必须是收款配置允许的值，流水 trim 后 1–128 字符、禁止控制字符。数据库条件唯一索引约束 channel + transaction_reference；唯一冲突回滚并返回 409。
- [ ] 审批事务更新凭证、paid_at、completed、权益和 audit_logs；审计只保存必要业务字段，不保存截图、密码和令牌。故障注入仅通过独立测试数据库的临时约束/trigger，不加入产品后门。
- [ ] 驳回必须提供 1–1000 字原因；订单恢复 pending_payment 并延长 expires_at。后台列表包含超过 24 小时未审核标记；GET /admin/orders、/admin/payments、/admin/audit-logs 使用分页和稳定排序。
- [ ] 章节查询在同一 SQL 授权条件下取正文：对应教程和章节可供访问，且预览或拥有当前 active 权益；章节 ID 必须属于路径中的教程。公共目录和详情严格使用不含正文的 DTO。已下架商品只屏蔽新购买，不影响原权益，尚未发布章节不得因商品权益存在而泄露。
- [ ] 使用 utoipa 提供 /api/v1/openapi.json，记录 Cookie、CSRF、幂等键、上传格式、DTO 与错误状态；仅导出接口结构，不包含任何实际密钥或用户数据。
- [ ] 重跑 payments、entitlements 全部通过并跑 `bash scripts/test-server.sh all` 后提交：`git commit -m "feat: atomically review payments and grant tutorial access"`。

### Task 7: Nuxt 前台、SSR、共享 API 客户端与安全 Markdown

**Files:** 创建 packages/shared-types/{package.json,src/index.ts}、packages/api-client/{package.json,src/index.ts,src/index.test.ts}、packages/ui/{package.json,src/tokens.css,src/MarkdownContent.vue,src/MarkdownContent.test.ts}；创建 apps/web/{package.json,nuxt.config.ts,Dockerfile,app/app.vue,app/layouts/default.vue,app/composables/useApi.ts,app/composables/useSession.ts,app/middleware/auth.ts,app/assets/main.css}；创建 app/pages/{index,login,register}.vue、app/pages/products/{index,[slug]}.vue、app/pages/tutorials/{index,[slug]}.vue、app/pages/checkout/[id].vue、app/pages/user/{orders,library}.vue；创建 tests/e2e/{support.ts,web.spec.ts,content-security.spec.ts}、playwright.config.ts、scripts/test-e2e.sh、deploy/nginx/default.conf；扩展 deploy/docker-compose.test.yml。

**Interfaces:** shared-types 导出与后端同字段的 ApiEnvelope、ApiError、PublicUser、ProductSummary、ProductDetail、TutorialDetail、OrderDetail、PaymentSummary、ChapterContent、LibraryEntry；`createApiClient(options: ApiClientOptions): ApiClient` 暴露 `request<T>(path: string, options?: RequestOptions): Promise<T>`；Nuxt useApi 提供请求级实例，SSR Cookie 只转发到配置的 API_INTERNAL_URL。`MarkdownContent` 接收 `content: string`。

- [ ] 编写客户端和渲染测试：refresh single-flight、刷新失败清理会话、每个请求最多重试一次、支付请求重试保持原 Idempotency-Key；Markdown script、onclick、javascript: 链接均不可执行，代码示例保持文本。

```ts
expect(refreshRequestCount).toBe(1)
expect(retriedOrder.headers.get('Idempotency-Key')).toBe(originalKey)
expect(rendered.querySelector('script')).toBeNull()
expect(rendered.querySelector('a[href^="javascript:"]')).toBeNull()
```

- [ ] 创建 Node 测试容器、Playwright fixture 和最小 Nginx 同源测试入口（/api/ → Axum、/ → Nuxt）；先接通测试 runner。运行 `npm run test:unit` 确认所写用例失败；添加 Playwright 预览未购买、登录返回、空状态和错误状态场景，运行 `npm run test:e2e -- web.spec.ts` 确认页面行为失败。
- [ ] 实现 Nuxt 4 app/ 目录、SEO 标题/描述、深色响应式布局和 Tailwind Token；首页与目录调用真实 API，错误时展示错误和重试入口。共享基础表单、按钮、状态反馈具备 label、focus-visible、禁用和错误说明。
- [ ] API 客户端 credentials=include，写请求从内存 CSRF 加请求头；刷新只针对认证过期 401、最多一次且不对登录接口递归刷新。使用 Web Locks 在支持的浏览器串行跨标签页刷新，锁内先重查当前会话；不支持时对刷新冲突重查共享 Cookie 结果，不立即清除成功轮换后的会话。仅浏览器处理 Cookie 轮换，SSR 遇过期会话返回无私有数据的续登录状态。
- [ ] 实现商品购买、注册/登录和返回路径；returnTo 只接受本站相对路径。下单成功后跳转收银台；网络失败保持幂等键，商品变化后创建新键，阻止重复点击但不依赖按钮禁用保证幂等。
- [ ] 收银台展示实际金额、订单号、收款信息及待付款/审核/驳回/完成状态，支持图片选取、上传反馈和原因展示；待审核每 5 秒查询，页面隐藏时暂停、离开页面取消计时。我的订单可查看明细和取消待付款单，资源页展示已购教程入口。
- [ ] 章节按需请求，未购买只展示预览和购买入口；Markdown 使用禁用 HTML 的解析器、允许协议过滤和 DOMPurify，代码高亮结果经净化。私有 useAsyncData key 包含用户和资源标识；退出/换用户清空私有 Nuxt 数据，SSR 禁止共享私有缓存。
- [ ] 用浏览器测试恶意 next 路径、正文 XSS、刷新竞争、退出后后退和 A/B 用户切换，再执行 `npm run test:unit`、`npm run typecheck`、`npm run build --workspace @devstore/web`。通过后提交：`git commit -m "feat: add storefront checkout and protected tutorial reading"`。

### Task 8: Vue 管理后台、编辑器、审核与审计界面

**Files:** 创建 apps/admin/{package.json,index.html,vite.config.ts,tsconfig.json,Dockerfile,src/main.ts,src/App.vue,src/router.ts,src/style.css}、src/{api.ts,session.ts}、src/views/{Login,Products,ProductEditor,Tutorials,TutorialEditor,Orders,Payments,AuditLogs}.vue、src/components/ReviewPaymentDialog.vue；扩展 packages/shared-types/src/index.ts 的管理员 DTO；创建 tests/e2e/admin.spec.ts；修改 deploy/nginx/default.conf 和 deploy/docker-compose.test.yml，接入 /admin/ 测试入口。

**Interfaces:** 使用 Task 7 的 ApiClient 与共享类型；管理路由 base=/admin/。ReviewPaymentDialog 接收 `payment: AdminPayment`，提交 `ApproveInput`，成功 emit reviewed，失败保留表单并显示服务端错误。

- [ ] 编写后台浏览器测试：未登录跳转后台登录，非管理员不能进入；创建商品和教程、写入章节、发布、修改及下架可见；审核必须填实际金额/渠道/流水/确认到账，驳回原因必填；刷新与返回不会重发批准请求。

```ts
await expect(page.getByRole('button', { name: '确认到账并开通' })).toBeDisabled()
await page.getByLabel('已核实真实收款记录').check()
// 填妥金额、渠道和流水后提交，结果来自 API。
await expect(page.getByText('已完成', { exact: true })).toBeVisible()
```

- [ ] 运行 `npm run test:e2e -- admin.spec.ts` 确认行为失败。
- [ ] 实现独立 Vite SPA、管理员会话守卫和响应式导航；守卫仅改善体验，所有权限仍由 API 执行。后台登录后验证 is_admin，不把普通用户视为管理员。
- [ ] 实现商品列表、价格“元 → 整数分”的严格解析、草稿/发布/归档及教程章节编辑；金额字符串只接受最多两位小数，禁止浮点乘法舍入。编辑器明确标出预览章节，默认未勾选。
- [ ] 实现订单列表与详情、待审核队列、图片 blob 预览及释放、实际收款复核表单、驳回表单；不得自动勾选核实到账，也不得用订单金额预填“实际到账金额”后直接提交。网络错误后从 API 重查，保持幂等行为。
- [ ] 实现审计列表；所有写操作给出成功/失败反馈并刷新权威数据，不使用虚构统计图表。列表支持空态、加载、错误和移动端横向查看。
- [ ] 执行 admin.spec.ts、`npm run typecheck`、`npm run build --workspace @devstore/admin`，通过后提交：`git commit -m "feat: add catalog management and manual payment review UI"`。

### Task 9: 同源 Compose 交付、完整交易验收、CI 与运行文档

**Files:** 创建 deploy/Dockerfile.gateway、.dockerignore、scripts/{check.sh,dev.sh}、tests/e2e/{purchase-flow.spec.ts,tenant-isolation.spec.ts}、.github/workflows/ci.yml、docs/verification/tutorial-checkout.md；修改 deploy/nginx/default.conf、scripts/test-e2e.sh、playwright.config.ts、tests/e2e/support.ts、deploy/docker-compose.yml、deploy/docker-compose.test.yml、README.md 和本计划的实际进度。

**Interfaces:** `bash scripts/dev.sh` 检查本地配置后启动 devstore；`bash scripts/check.sh` 运行 fmt、clippy、单元与真实数据库集成测试、前端类型检查和构建；`bash scripts/test-e2e.sh` 使用独立测试项目运行 Chromium 桌面和移动端场景。

- [x] 编写完整 Playwright 场景：管理员通过 API/页面发布真实测试商品 → 用户注册 → 预览成功且付费正文被拒 → 下单 → 上传 → 管理员驳回 → 用户重提 → 管理员核实通过 → 订单完成 → 用户阅读付费正文。测试独立创建管理员和虚构测试收款配置，绝不连接真实资金账户。

```ts
expect(order.amount_cents).toBe(9900)
expect(order.status).toBe('completed')
expect(grantedEntitlements).toBe(1)
expect(otherUserChapterResponse.status()).toBe(403)
```

- [x] 运行 `bash scripts/test-e2e.sh`，确认缺少同源路由、资源路径或交付连接时失败；不得把 API mock 成功作为此处的通过依据。
- [x] 完成 Nginx 路由：/api/ → Axum、/admin/ → 管理静态文件及 SPA fallback、/ → Nuxt Node SSR；限制请求体、禁用私有响应缓存、配置 CSP 与安全头。覆盖 /admin 深链接刷新、Nuxt 资源路径和 multipart 错误响应。所有服务仅内部网络，唯一宿主入口 127.0.0.1:8088。
- [x] 提供配置、构建和运行文档：生成随机密钥、初始化管理员、显式 seed-demo、设置收款、启动/停止、日志查看和健康检查。收款信息未配置时页面明确说明不可购买；不得宣称可直接用于公网生产经营。
- [x] CI 使用锁定依赖，依次执行 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust 测试、TS 类型检查、前端单元测试、双方构建和购买链路 E2E；只保留诊断所需测试工件，测试数据使用虚构用户和凭证。
- [x] 检查 390px 和 1440px 视口布局、键盘操作、长订单号、代码块、慢请求和断网恢复；测试 A 用户读取后 B 用户登录、SSR 两用户并发请求及退出后浏览器后退，三种情况下都不能看到前用户正文。
- [x] 运行 `bash scripts/check.sh` 和 `bash scripts/test-e2e.sh`，记录实际命令、时间、通过数及失败原因到 verification 文档；没有执行的项明确为未验证。检查 Git diff 无密钥或测试用户上传，并完成独立代码审查与问题修复。
- [x] 仅当设计稿首阶段 14 条验收全部具备证据时，提交 `git commit -m "test: verify tutorial purchase flow and document local deployment"`，报告入口地址、操作方式和完整 V1.0 尚未覆盖的项目。提交不等于推送、合并或公网部署。证据见 docs/verification/tutorial-checkout.md。GitHub Actions 本身未在远端执行。

## 计划自查与覆盖映射

| 设计稿要求 | 负责任务 |
| --- | --- |
| 本地启动、迁移、配置、安全响应与日志 | 1、9 |
| Cookie、会话撤销、CSRF、RBAC、限流 | 2、7 |
| 商品、分类、教程编辑、公开元数据 | 3、8 |
| 商品快照、下单幂等、零元、取消、超时 | 4 |
| 私有凭证、类型/尺寸校验、扫描、清理 | 5 |
| 并发审核、流水唯一、审计、驳回重提 | 6、8 |
| 付费阅读、下架后权益、资源中心 | 6、7 |
| SSR、Markdown 安全、响应式和换用户隔离 | 7、9 |
| 真实 PostgreSQL 并发验证与桌面/移动端 E2E | 1–6、9 |

零元权益函数在 Task 4 首次实现，Task 6 复用；payments 的凭证上传在 Task 5 实现，Task 6 扩展审核。涉及未来数字商品/服务、退款、密码重置和管理员 MFA 的字段可保留，但不将这些功能标记为 A 范围已交付。Task 9 的本地检查、浏览器验收和 Compose 启动已有记录；GitHub Actions 未在远端执行。

## 核实过的官方参考

- [Axum 官方 crate 文档](https://docs.rs/axum/latest/axum/)：核实 0.8 API 路由、提取器、共享状态和 multipart 能力。
- [SQLx 官方 crate 文档](https://docs.rs/sqlx/latest/sqlx/)与[迁移宏说明](https://docs.rs/sqlx/latest/sqlx/macro.migrate.html)：核实 PostgreSQL、Tokio/TLS 和迁移构建监听。
- [Nuxt 官方发布记录](https://github.com/nuxt/nuxt/releases)与[Node SSR 部署](https://nuxt.com/docs/4.x/getting-started/deployment)：核实 CLI Node 版本要求与 SSR 运行入口，项目仍限定 Nuxt 4。
- [Vite 官方入门](https://vite.dev/guide/)：核实工具链 Node 版本下限；项目使用容器 Node 24 避免更改现有系统工具链。
- [object_store S3 构建器](https://docs.rs/object_store/latest/object_store/aws/struct.AmazonS3Builder.html)与[ClamAV 扫描文档](https://docs.clamav.net/manual/Usage/Scanning.html)：核实私有存储适配和扫描接口边界。
