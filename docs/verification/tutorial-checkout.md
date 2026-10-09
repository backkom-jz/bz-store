# 教程购买闭环验收记录

日期：2026-10-09。命令在仓库根目录执行。测试用户、收款账号和密钥都是虚构或本机随机值，没有连接真实资金账户。

## 已执行

`bash scripts/check.sh`，实际耗时 118.34 秒，退出码 0。

- `cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` 通过。脚本遇到失败会停止，这两步没有失败。
- Rust 测试 23 个通过、0 个失败。覆盖认证生命周期与管理员显式初始化、CSRF、刷新竞争、限流、目录与付费正文不进入元数据、价格快照与幂等、取消和超时、凭证扫描与并发提交、审核幂等、流水唯一、金额不符、驳回重提、审核回滚、权益与下架、健康和就绪。
- `npm run typecheck` 通过。
- Vitest：5 个文件、15 个测试通过，耗时 786 毫秒。
- `@devstore/web` 生产构建 8.64 秒，`@devstore/admin` 生产构建 665 毫秒。

`bash scripts/test-e2e.sh`，实际耗时 77.85 秒，退出码 0。Playwright 桌面 1440×1000 与移动 390×844 共 8 个测试通过。

- 购买链路：预览可见、付费正文被拒、注册、下单、上传、驳回、重提、核实通过、订单 9900 分且状态 completed、资源数为 1、另一用户章节 403。桌面 15.7 秒，移动 13.8 秒。
- 换用户隔离：购买者可读，另一用户 API 403，两份 SSR HTML 都不含正文；退出后后退不再显示正文。桌面 6.3 秒，移动 6.7 秒。
- 公开目录、预览、本地回跳和空资源库：桌面 3.6 秒与 5.0 秒，移动 3.9 秒与 4.5 秒。

本地 Compose：`bash scripts/dev.sh` 的镜像构建与启动已完成。健康检查 `http://127.0.0.1:8088/api/v1/health/ready` 返回 ready。唯一宿主端口是 `127.0.0.1:8088`，数据库、Redis、对象存储和 ClamAV 没有映射到宿主机。显式创建了管理员 `local-admin@example.test`，没有默认密码；示例教程 slug 为 `rust-axum-starter`。未配置收款时，商品页写明不能购买，下单返回 HTTP 503 与「尚未配置收款信息，暂时不能购买」。`/admin/products/new` 返回后台入口页。浏览器在商品页点击「购买教程」进入带 returnTo 的登录页；390px 宽度没有横向溢出。

## 未验证

GitHub Actions 工作流 `.github/workflows/ci.yml` 已写入，但没有在 GitHub 上执行。本地 `check.sh` 与 `test-e2e.sh` 覆盖了它的命令顺序。

## 设计稿首阶段 14 条

1. 空库迁移、显式管理员和健康检查：本地 Compose 与 `it_admin_bootstrap_is_explicit_and_audited`、`it_ready_depends_on_all_services`。
2. 注册、登录、刷新、退出和密码哈希：`it_auth_lifecycle` 与刷新测试；浏览器注册进入了购买链路。
3. 发布、普通用户不能进后台、下架保留权益：目录与权益集成测试，后台深链接只提供入口，权限仍由 API 执行。
4. 伪造价格与快照：`it_idempotent_checkout_and_price_snapshots`。
5. 幂等键并发、跨用户和载荷冲突：同一订单测试。
6. 预览可读、付费正文不从 API 或 SSR 泄露：目录测试与两条浏览器测试。
7. 凭证类型、大小、所有权和扫描：上传集成测试；浏览器上传经过真实扫描。
8. 驳回原因、重提、旧凭证不能再批：支付集成测试与购买浏览器测试。
9. 并发审核不重复权益和审计：`it_approval_idempotency_and_reference_uniqueness`。
10. 审核注入失败后回滚：`it_review_rollback_reject_and_resubmit`。
11. 流水唯一和金额不符：同一支付测试，金额 9800 分返回 422。
12. 取消、超时与待审核竞争：`it_expiry_workers_preserve_reviewing_orders`。
13. 不能读取他人订单或凭证，私有响应不缓存：上传测试与 Nginx `Cache-Control: private, no-store`。
14. 桌面和移动端走完购买链路：上面的 8 个浏览器测试。
